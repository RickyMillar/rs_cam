//! The Machine tab: the library row, the three panel dials, the kinematics
//! editor and the GRBL `$$` import.
//!
//! Every write goes through `super::panel_apply`, which owns the command
//! door.

use super::PanelEdit;
use super::panel_apply::{
    apply_machine, apply_machine_import, apply_machine_kinematics, apply_stock_draft,
    machine_panel_fields_moved,
};
use crate::state::AppState;
use crate::ui::AppEvent;
use crate::ui::components::UiExt as _;
use crate::ui_command::{NoArgs, UiCommand};

/// Machine-library UX (SNAPSHOT model, like the tool library): import a
/// machine *out of* the library (copied into the project's inline machine,
/// no live link) or save the current machine into the library as a
/// reusable starting point. Later edits to a library file never reach
/// existing projects — re-import to pick up a change.
pub(super) fn draw_machine_library_row(
    ui: &mut egui::Ui,
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
) {
    let status_id = egui::Id::new("machine_lib_status");
    let name_id = egui::Id::new("machine_lib_save_name");

    let machines = rs_cam_core::io::machine_library::list();

    ui.horizontal(|ui| {
        ui.label("Library:");
        egui::ComboBox::from_id_salt("machine_library_import")
            .selected_text("Import a machine…")
            .show_ui(ui, |ui| {
                for name in &machines {
                    if ui.selectable_label(false, name).clicked() {
                        match rs_cam_core::io::machine_library::load(name) {
                            Ok(profile) => {
                                // Snapshot copy into the inline machine — no ref.
                                apply_machine(state, profile);
                                ui.data_mut(|d| {
                                    d.insert_temp(status_id, format!("Imported '{name}' (copy)"));
                                });
                            }
                            Err(e) => {
                                tracing::error!("machine library load failed: {e}");
                                ui.data_mut(|d| {
                                    d.insert_temp(status_id, format!("Import failed: {e}"));
                                });
                            }
                        }
                    }
                }
            });
        if machines.is_empty() {
            ui.label(egui::RichText::new("(library empty)").small().weak());
        }
        if ui.button("Manage…").clicked() {
            events.push(AppEvent::Ui(UiCommand::OpenMachineLibrary(NoArgs)));
        }
    });

    ui.horizontal(|ui| {
        ui.label("Save as:");
        let mut name: String = ui.data(|d| d.get_temp::<String>(name_id).unwrap_or_default());
        let resp = ui.add(
            egui::TextEdit::singleline(&mut name)
                .desired_width(140.0)
                .hint_text("machine name"),
        );
        if resp.changed() {
            ui.data_mut(|d| d.insert_temp(name_id, name.clone()));
        }
        let trimmed = name.trim().to_owned();
        if ui
            .add_enabled(!trimmed.is_empty(), egui::Button::new("Save to library"))
            .clicked()
        {
            match rs_cam_core::io::machine_library::save(&trimmed, state.session.machine()) {
                Ok(path) => {
                    state.gui.mark_edited();
                    ui.data_mut(|d| {
                        d.insert_temp(status_id, format!("Saved to {}", path.display()));
                    });
                }
                Err(e) => {
                    tracing::error!("machine library save failed: {e}");
                    ui.data_mut(|d| d.insert_temp(status_id, format!("Save failed: {e}")));
                }
            }
        }
    });

    if let Some(msg) = ui.data(|d| d.get_temp::<String>(status_id)) {
        ui.label(egui::RichText::new(msg).small().weak());
    }
}

// SAFETY: selected_idx from position() within presets; i from enumerate over presets
#[allow(clippy::indexing_slicing)]
pub(super) fn draw_machine_panel(
    ui: &mut egui::Ui,
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
) {
    ui.heading("Machine Setup");
    ui.separator();

    let presets = rs_cam_core::machine::MachineProfile::presets();
    // R6: identity is structural — a library machine or edited preset
    // shows as "Custom" instead of silently claiming the first preset
    // whose name happens to overlap.
    let selected_idx = state.session.machine().matching_preset_index();

    ui.horizontal(|ui| {
        ui.label("Preset:");
        egui::ComboBox::from_id_salt("machine_preset")
            .selected_text(selected_idx.map_or("Custom", |i| presets[i].0))
            .show_ui(ui, |ui| {
                for (i, (label, _)) in presets.iter().enumerate() {
                    if ui
                        .selectable_label(selected_idx == Some(i), *label)
                        .clicked()
                    {
                        // Snapshot: copy the preset into the inline machine.
                        apply_machine(state, presets[i].1.clone());
                    }
                }
            });
    });

    draw_machine_library_row(ui, state, events);

    ui.add_space(8.0);

    // Machine specs — RPM/Power stay read-only (preset/spindle-driven);
    // Max Feed (travel rate, $110-class) and Max Shank are editable.
    // WP6: the widgets below write a DRAFT profile, not the session.
    // The draft survives the frame, so a `DragValue` release applies one
    // command instead of one per frame of the drag.
    let mut draft = match state.history.machine_draft.take() {
        Some(draft) => draft,
        None => state.session.machine().clone(),
    };
    let mut edit = PanelEdit::default();
    ui.param_grid("machine_specs", |ui| {
        let (min_rpm, max_rpm) = state.session.machine().rpm_range();
        ui.label("RPM Range:");
        ui.label(format!("{:.0} - {:.0}", min_rpm, max_rpm));
        ui.end_row();

        let max_power = match state.session.machine().power {
            rs_cam_core::machine::PowerModel::VfdConstantTorque { rated_power_kw, .. } => {
                rated_power_kw
            }
            rs_cam_core::machine::PowerModel::ConstantPower { power_kw } => power_kw,
        };
        ui.label("Power:");
        ui.label(format!("{:.2} kW", max_power));
        ui.end_row();

        ui.label("Max Feed:");
        edit.drag(
            &ui.add(
                egui::DragValue::new(&mut draft.max_feed_mm_min)
                    .speed(50.0)
                    .range(100.0..=30000.0)
                    .suffix(" mm/min"),
            )
            .on_hover_text("Travel/rapid rate ($110-class). Cutting feeds are capped separately."),
        );
        ui.end_row();

        ui.label("Max Shank:");
        edit.drag(
            &ui.add(
                egui::DragValue::new(&mut draft.max_shank_mm)
                    .speed(0.1)
                    .range(1.0..=25.0)
                    .suffix(" mm"),
            ),
        );
        ui.end_row();
    });

    ui.add_space(8.0);
    // The kinematics grid keeps its own edit record: its row is
    // `SetMachineKinematics`, which also drops the library link, and the
    // whole-profile row must not be applied for it.
    let mut kinematics_edit = PanelEdit::default();
    draw_machine_kinematics(ui, state, &mut draft, &mut kinematics_edit);
    if kinematics_edit.committed && draft.kinematics != state.session.machine().kinematics {
        apply_machine_kinematics(state, draft.kinematics);
    }
    edit.in_flight |= kinematics_edit.in_flight;

    ui.add_space(8.0);

    // Safety factor / aggressiveness slider
    ui.horizontal(|ui| {
        ui.label("Aggressiveness:");
        edit.drag(
            &ui.add(
                egui::Slider::new(&mut draft.safety_factor, 0.60..=0.95)
                    .text("")
                    .show_value(true),
            ),
        );
    });
    // The label reads the DRAFT. Reading the session would leave it one
    // release behind the handle for the whole drag (plan section 4 WP6,
    // same-frame reader (a)).
    ui.label(
        egui::RichText::new(if draft.safety_factor < 0.72 {
            "Conservative — safer for new setups"
        } else if draft.safety_factor > 0.85 {
            "Aggressive — experienced operators only"
        } else {
            "Balanced — good for most work"
        })
        .small()
        .color(crate::ui::tokens::TEXT_MUTED),
    );

    // One `SetMachine` for the whole profile. `set_machine` writes the
    // profile alone.
    if edit.committed && machine_panel_fields_moved(&draft, state.session.machine()) {
        apply_machine(state, draft.clone());
    }
    if edit.in_flight {
        state.history.machine_draft = Some(draft);
    }

    ui.add_space(8.0);

    // Workholding rigidity selector. WP6: the combo writes a DRAFT of
    // the stock record, and one `SetStockConfig` lands it.
    //
    // BEHAVIOUR CHANGE, named: the row now drops every toolpath result,
    // because the rigidity derates the feeds of every operation. The
    // panel used to write `stock_mut()` in place and mark the project
    // edited alone, so the cached results kept the previous derating
    // (G-FRESHSTATE).
    use rs_cam_core::feeds::WorkholdingRigidity;
    let mut rigidity = state.session.stock_config().workholding_rigidity;
    let mut rigidity_changed = false;
    ui.horizontal(|ui| {
        ui.label("Workholding:");
        let label = match rigidity {
            WorkholdingRigidity::Low => "Low",
            WorkholdingRigidity::Medium => "Medium",
            WorkholdingRigidity::High => "High",
        };
        egui::ComboBox::from_id_salt("workholding_rigidity")
            .selected_text(label)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_value(&mut rigidity, WorkholdingRigidity::Low, "Low")
                    .changed()
                    || ui
                        .selectable_value(&mut rigidity, WorkholdingRigidity::Medium, "Medium")
                        .changed()
                    || ui
                        .selectable_value(&mut rigidity, WorkholdingRigidity::High, "High")
                        .changed()
                {
                    rigidity_changed = true;
                }
            });
    });
    if rigidity_changed {
        let mut stock = state.session.stock_config().clone();
        stock.workholding_rigidity = rigidity;
        apply_stock_draft(state, stock);
    }
    ui.label(
        egui::RichText::new(match state.session.stock_config().workholding_rigidity {
            rs_cam_core::feeds::WorkholdingRigidity::Low => {
                "Low — tape/CA glue, vacuum table, thin stock"
            }
            rs_cam_core::feeds::WorkholdingRigidity::Medium => {
                "Medium — clamps, toggle clamps, most setups"
            }
            rs_cam_core::feeds::WorkholdingRigidity::High => {
                "High — heavy vise, bolted fixture, thick stock"
            }
        })
        .small()
        .color(crate::ui::tokens::TEXT_MUTED),
    );
}

/// Kinematics editor (per-axis accel + junction deviation + optional
/// jerk) plus the GRBL `$$` import. Editing a value or applying an import
/// materializes the machine's `kinematics: Some(..)`, which opts the live
/// sim into the acceleration-aware cycle-time model (the `None` default
/// is the F-034 feature flag that keeps runtime byte-identical).
pub(super) fn draw_machine_kinematics(
    ui: &mut egui::Ui,
    state: &mut AppState,
    draft: &mut rs_cam_core::machine::MachineProfile,
    edit: &mut PanelEdit,
) {
    ui.label(
        egui::RichText::new("Kinematics (cycle-time model)")
            .strong()
            .color(crate::ui::tokens::TEXT_STRONG),
    );

    if state.session.machine().kinematics.is_none() {
        ui.label(
            egui::RichText::new(
                "Not set — showing defaults. Editing a value or importing $$ enables the \
                 acceleration-aware cycle-time model for this machine.",
            )
            .small()
            .color(crate::ui::tokens::CAUTION),
        );
    }

    // WP6: the block comes off the DRAFT, so a value typed on a
    // previous frame is not re-read from the unchanged session.
    let mut kin = draft
        .kinematics
        .unwrap_or_else(|| state.session.machine().effective_kinematics());
    let scalar = kin.acceleration_mm_s2.max(1.0);
    let mut axes = kin
        .acceleration_xyz_mm_s2
        .unwrap_or([scalar, scalar, scalar]);
    let mut delta = kin.junction_deviation_mm;
    let mut jerk_enabled = kin.jerk_mm_s3.is_some();
    let mut jerk_val = kin.jerk_mm_s3.unwrap_or(500.0);
    let mut changed = false;

    let accel_row = |ui: &mut egui::Ui, label: &str, v: &mut f64, hint: &str| -> egui::Response {
        ui.label(label);
        let response = ui
            .add(
                egui::DragValue::new(v)
                    .speed(5.0)
                    .range(10.0..=20000.0)
                    .suffix(" mm/s²"),
            )
            .on_hover_text(hint);
        ui.end_row();
        response
    };

    ui.param_grid("machine_kinematics", |ui| {
        for (label, index, hint) in [
            ("Accel X:", 0_usize, "GRBL $120"),
            ("Accel Y:", 1, "GRBL $121"),
            ("Accel Z:", 2, "GRBL $122"),
        ] {
            // SAFETY: the three indices are literals into a [f64; 3]
            #[allow(clippy::indexing_slicing)]
            let response = accel_row(ui, label, &mut axes[index], hint);
            changed |= response.changed();
            edit.drag(&response);
        }

        ui.label("Junction dev:");
        let response = ui
            .add(
                egui::DragValue::new(&mut delta)
                    .speed(0.001)
                    .range(0.001..=1.0)
                    .max_decimals(4)
                    .suffix(" mm"),
            )
            .on_hover_text("GRBL $11 — how far the cornering arc may bow from the vertex");
        changed |= response.changed();
        edit.drag(&response);
        ui.end_row();

        ui.label("Jerk limit:");
        ui.horizontal(|ui| {
            let response = ui.checkbox(&mut jerk_enabled, "");
            changed |= response.changed();
            edit.click(&response);
            if jerk_enabled {
                let response = ui.add(
                    egui::DragValue::new(&mut jerk_val)
                        .speed(10.0)
                        .range(1.0..=100_000.0)
                        .suffix(" mm/s³"),
                );
                changed |= response.changed();
                edit.drag(&response);
            } else {
                ui.label(egui::RichText::new("off (trapezoidal)").small().weak());
            }
        });
        ui.end_row();
    });

    if changed {
        kin.acceleration_xyz_mm_s2 = Some(axes);
        kin.acceleration_mm_s2 = (axes[0] + axes[1] + axes[2]) / 3.0;
        kin.junction_deviation_mm = delta;
        kin.jerk_mm_s3 = if jerk_enabled { Some(jerk_val) } else { None };
        draft.kinematics = Some(kin);
    }

    ui.add_space(4.0);
    draw_grbl_import(ui, state, draft, edit);
}

/// "Import GRBL `$$`" — paste or load a settings dump, preview the mapped
/// values, then Apply (the confirm step; reversible via the machine undo
/// snapshot). Applying sets kinematics + Max Feed and breaks any library
/// link, since the values are now inline.
pub(super) fn draw_grbl_import(
    ui: &mut egui::Ui,
    state: &mut AppState,
    draft: &mut rs_cam_core::machine::MachineProfile,
    edit: &mut PanelEdit,
) {
    let buf_id = egui::Id::new("machine_grbl_paste");
    let status_id = egui::Id::new("machine_grbl_status");

    ui.collapsing("Import GRBL $$", |ui| {
        let mut buf: String = ui.data(|d| d.get_temp::<String>(buf_id).unwrap_or_default());

        ui.horizontal(|ui| {
            if ui.button("Load from file…").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("GRBL settings", &["txt", "nc", "gcode", "cfg"])
                    .pick_file()
            {
                match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        buf = content;
                        ui.data_mut(|d| d.insert_temp(buf_id, buf.clone()));
                    }
                    Err(e) => {
                        tracing::error!("read $$ file failed: {e}");
                        ui.data_mut(|d| {
                            d.insert_temp(status_id, format!("Read failed: {e}"));
                        });
                    }
                }
            }
            if ui.button("Clear").clicked() {
                buf.clear();
                ui.data_mut(|d| {
                    d.insert_temp(buf_id, String::new());
                    d.insert_temp(status_id, String::new());
                });
            }
        });

        let resp = ui.add(
            egui::TextEdit::multiline(&mut buf)
                .desired_rows(4)
                .desired_width(f32::INFINITY)
                .hint_text("Paste $$ output here ($11=…, $120=…, …)"),
        );
        if resp.changed() {
            ui.data_mut(|d| d.insert_temp(buf_id, buf.clone()));
        }

        if !buf.trim().is_empty() {
            let imp = rs_cam_core::machine::kinematics::MachineKinematics::from_grbl_settings(&buf);
            let default_delta = rs_cam_core::machine::kinematics::default_junction_deviation_mm();
            let recognized = imp.kinematics.acceleration_xyz_mm_s2.is_some()
                || imp.max_feed_mm_min.is_some()
                || imp.arc_tolerance_mm.is_some()
                || imp.max_spindle_rpm.is_some()
                || (imp.kinematics.junction_deviation_mm - default_delta).abs() > 1e-12;

            if !recognized {
                ui.label(
                    egui::RichText::new("No GRBL settings recognised in this text.")
                        .small()
                        .color(crate::ui::tokens::DANGER),
                );
            } else {
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Will apply:").small().strong());
                if let Some(a) = imp.kinematics.acceleration_xyz_mm_s2 {
                    ui.label(
                        egui::RichText::new(format!(
                            "• Accel X/Y/Z = {:.0}/{:.0}/{:.0} mm/s²",
                            a[0], a[1], a[2]
                        ))
                        .small(),
                    );
                }
                ui.label(
                    egui::RichText::new(format!(
                        "• Junction dev ($11) = {:.3} mm",
                        imp.kinematics.junction_deviation_mm
                    ))
                    .small(),
                );
                if let Some(r) = imp.max_rate_xyz_mm_min {
                    ui.label(
                        egui::RichText::new(format!(
                            "• Max rate X/Y/Z = {:.0}/{:.0}/{:.0} mm/min",
                            r[0], r[1], r[2]
                        ))
                        .small(),
                    );
                }
                if let Some(mf) = imp.max_feed_mm_min {
                    let cur = state.session.machine().max_feed_mm_min;
                    ui.label(
                        egui::RichText::new(format!("• Max Feed: {cur:.0} → {mf:.0} mm/min"))
                            .small(),
                    );
                }
                if let Some(at) = imp.arc_tolerance_mm {
                    ui.label(
                        egui::RichText::new(format!("• Arc tol ($12) = {at:.3} mm (advisory)"))
                            .small()
                            .weak(),
                    );
                }
                if imp.ignored_count > 0 {
                    ui.label(
                        egui::RichText::new(format!(
                            "({} unrelated $ settings ignored)",
                            imp.ignored_count
                        ))
                        .small()
                        .weak(),
                    );
                }

                if ui.button("Apply import").clicked() {
                    let max_feed = imp.max_feed_mm_min;
                    // The parser reports the per-axis rates on the import,
                    // not inside `kinematics` — land them here (P1).
                    let mut kinematics = imp.kinematics;
                    kinematics.max_rate_xyz_mm_min = imp.max_rate_xyz_mm_min;
                    // The draft follows the write, so the panel does not
                    // put the pre-import numbers back on the next commit.
                    draft.kinematics = Some(kinematics);
                    if let Some(mf) = max_feed {
                        draft.max_feed_mm_min = mf;
                    }
                    // The import is its own row: a GRBL dump carries the
                    // travel rate as well as the block, and the row drops
                    // the library link because the numbers are inline now.
                    apply_machine_import(state, kinematics, max_feed);
                    edit.changed = true;
                    ui.data_mut(|d| {
                        d.insert_temp(buf_id, String::new());
                        d.insert_temp(status_id, "Imported $$ settings".to_owned());
                    });
                }
            }
        }

        if let Some(msg) = ui.data(|d| d.get_temp::<String>(status_id))
            && !msg.is_empty()
        {
            ui.label(egui::RichText::new(msg).small().weak());
        }
    });
}
