//! Machine Library management modal (SNAPSHOT model).
//!
//! Browse the per-user machine library
//! (`~/.config/rs_cam/machines/*.toml`), preview a machine, and import it
//! as a snapshot COPY into the project's inline machine (no live link), or
//! save / rename / delete library entries. Mirrors the Tool Library modal
//! but simpler: machines are one-file-per-entry, so it reads the library
//! directly each frame (cheap) and routes every mutation through
//! [`AppEvent`]s — it never mutates `AppState` or the filesystem directly.

use super::{AppEvent, theme};
use crate::state::AppState;

/// Ephemeral view state, stashed in egui temp memory.
#[derive(Debug, Clone, Default)]
struct MachineLibraryView {
    selected: Option<String>,
    save_name: String,
    rename_buf: String,
    renaming: bool,
    confirm_delete: bool,
}

/// Top-level draw entry. Short-circuits when the modal is closed.
pub fn draw(ctx: &egui::Context, state: &AppState, events: &mut Vec<AppEvent>) {
    if !state.machine_library_open {
        return;
    }
    let mut still_open = true;
    egui::Window::new("Machine Library")
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(720.0)
        .default_height(460.0)
        .open(&mut still_open)
        .show(ctx, |ui| draw_content(ui, state, events));
    if !still_open {
        events.push(AppEvent::CloseMachineLibrary);
    }
}

fn draw_content(ui: &mut egui::Ui, state: &AppState, events: &mut Vec<AppEvent>) {
    let view_id = egui::Id::new("machinelib_view");
    let mut view: MachineLibraryView = ui.data(|d| d.get_temp(view_id).unwrap_or_default());

    let machines = rs_cam_core::machine_library::list();
    // Drop a selection that was deleted/renamed out from under us.
    if let Some(sel) = view.selected.clone()
        && !machines.contains(&sel)
    {
        view.selected = None;
        view.renaming = false;
        view.confirm_delete = false;
    }

    ui.label(
        egui::RichText::new(
            "Snapshot library — importing COPIES a machine into this project; later library \
             edits don't change existing projects.",
        )
        .small()
        .color(theme::TEXT_MUTED),
    );
    ui.separator();

    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_min_width(210.0);
            ui.set_max_width(240.0);
            draw_list_panel(ui, &machines, &mut view, state, events);
        });
        ui.separator();
        ui.vertical(|ui| {
            draw_detail_panel(ui, &mut view, events);
        });
    });

    ui.data_mut(|d| d.insert_temp(view_id, view));
}

fn draw_list_panel(
    ui: &mut egui::Ui,
    machines: &[String],
    view: &mut MachineLibraryView,
    state: &AppState,
    events: &mut Vec<AppEvent>,
) {
    ui.label(
        egui::RichText::new("Library machines")
            .strong()
            .color(theme::TEXT_HEADING),
    );
    ui.add_space(2.0);

    egui::ScrollArea::vertical()
        .id_salt("machinelib_list")
        .max_height(280.0)
        .show(ui, |ui| {
            if machines.is_empty() {
                ui.label(
                    egui::RichText::new("No machines saved yet")
                        .italics()
                        .color(theme::TEXT_MUTED),
                );
            }
            for name in machines {
                let selected = view.selected.as_deref() == Some(name.as_str());
                if ui.selectable_label(selected, name).clicked() {
                    view.selected = Some(name.clone());
                    view.renaming = false;
                    view.confirm_delete = false;
                    view.rename_buf = name.clone();
                }
            }
        });

    ui.add_space(6.0);
    ui.separator();
    ui.label(
        egui::RichText::new(format!("Save '{}' as:", state.session.machine().name))
            .small()
            .color(theme::TEXT_MUTED),
    );
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut view.save_name)
                .desired_width(130.0)
                .hint_text("machine name"),
        );
        let name = view.save_name.trim().to_owned();
        if ui
            .add_enabled(!name.is_empty(), egui::Button::new("Save"))
            .clicked()
        {
            events.push(AppEvent::SaveMachineToLibrary(name));
            view.save_name.clear();
        }
    });
}

fn draw_detail_panel(ui: &mut egui::Ui, view: &mut MachineLibraryView, events: &mut Vec<AppEvent>) {
    let Some(name) = view.selected.clone() else {
        ui.label(
            egui::RichText::new("Select a machine to preview")
                .italics()
                .color(theme::TEXT_MUTED),
        );
        return;
    };

    let profile = match rs_cam_core::machine_library::load(&name) {
        Ok(p) => p,
        Err(e) => {
            ui.colored_label(
                egui::Color32::from_rgb(220, 120, 120),
                format!("Could not load '{name}': {e}"),
            );
            return;
        }
    };

    ui.heading(&profile.name);
    ui.add_space(2.0);
    egui::Grid::new("machinelib_preview")
        .num_columns(2)
        .spacing([10.0, 3.0])
        .show(ui, |ui| {
            let (min_rpm, max_rpm) = profile.rpm_range();
            row(ui, "RPM range", format!("{min_rpm:.0} – {max_rpm:.0}"));
            row(
                ui,
                "Max feed",
                format!("{:.0} mm/min", profile.max_feed_mm_min),
            );
            row(ui, "Max shank", format!("{:.1} mm", profile.max_shank_mm));
            match &profile.kinematics {
                Some(k) => {
                    let accel = match k.acceleration_xyz_mm_s2 {
                        Some([ax, ay, az]) => format!("{ax:.0} / {ay:.0} / {az:.0} mm/s²"),
                        None => format!("{:.0} mm/s² (isotropic)", k.acceleration_mm_s2),
                    };
                    row(ui, "Accel X/Y/Z", accel);
                    row(
                        ui,
                        "Junction dev",
                        format!("{:.3} mm", k.junction_deviation_mm),
                    );
                }
                None => row(ui, "Kinematics", "not set".to_owned()),
            }
        });

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui
            .button(egui::RichText::new("Import into project").strong())
            .on_hover_text("Copy this machine into the project (snapshot — no live link)")
            .clicked()
        {
            events.push(AppEvent::ImportMachineFromLibrary(name.clone()));
        }
        if ui.button("Rename").clicked() {
            view.renaming = !view.renaming;
            view.confirm_delete = false;
            view.rename_buf = name.clone();
        }
        if view.confirm_delete {
            if ui
                .button(
                    egui::RichText::new("Confirm delete")
                        .color(egui::Color32::from_rgb(220, 90, 90)),
                )
                .clicked()
            {
                events.push(AppEvent::DeleteMachineFromLibrary(name.clone()));
                view.confirm_delete = false;
                view.selected = None;
            }
            if ui.button("Cancel").clicked() {
                view.confirm_delete = false;
            }
        } else if ui
            .button(egui::RichText::new("Delete").color(egui::Color32::from_rgb(200, 100, 100)))
            .clicked()
        {
            view.confirm_delete = true;
        }
    });

    if view.renaming {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("New name:");
            ui.add(
                egui::TextEdit::singleline(&mut view.rename_buf)
                    .desired_width(150.0)
                    .hint_text("new name"),
            );
            let new = view.rename_buf.trim().to_owned();
            if ui
                .add_enabled(!new.is_empty() && new != name, egui::Button::new("Apply"))
                .clicked()
            {
                events.push(AppEvent::RenameMachineInLibrary {
                    old: name.clone(),
                    new: new.clone(),
                });
                view.renaming = false;
                view.selected = Some(new);
            }
        });
    }
}

fn row(ui: &mut egui::Ui, label: &str, value: String) {
    ui.label(egui::RichText::new(label).color(theme::TEXT_MUTED));
    ui.label(value);
    ui.end_row();
}
