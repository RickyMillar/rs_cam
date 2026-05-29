//! Tool Library management modal.
//!
//! A dedicated screen for browsing, previewing, editing, and organising
//! the per-user tool catalogs (`~/.config/rs_cam/tools/*.toml`). The
//! modal reads from a cached snapshot (`AppState::tool_library_modal`)
//! that the controller refreshes after every mutation, and routes every
//! change through [`AppEvent`]s — it never touches `AppState` mutably or
//! the filesystem directly.
//!
//! Phase 1: browse + preview + add-to-project + delete tool.
//! Phase 2: edit-in-place, move between catalogs, new/delete/rename/dedupe
//! catalogs, search across all catalogs.
//!
//! Ephemeral view state (selection, filter, edit draft, confirm flags)
//! lives in a single [`ToolLibraryView`] held in egui temp memory, the
//! same trick the Save-to-library buffers use.

use super::{AppEvent, theme};
use crate::state::AppState;
use crate::state::job::{ToolConfig, ToolType};

/// All ephemeral view state for the modal, stashed in egui temp memory.
#[derive(Debug, Clone, Default)]
struct ToolLibraryView {
    selected_catalog: Option<String>,
    selected_index: Option<usize>,
    filter: String,
    search_all: bool,
    editing: bool,
    draft: Option<ToolConfig>,
    confirm_delete_tool: bool,
    confirm_delete_catalog: bool,
    new_catalog_name: String,
    rename_catalog_buf: String,
    move_target: String,
}

/// Top-level draw entry. Short-circuits when no modal is open.
pub fn draw(ctx: &egui::Context, state: &AppState, events: &mut Vec<AppEvent>) {
    let Some(modal) = state.tool_library_modal.as_ref() else {
        return;
    };
    let mut still_open = true;
    egui::Window::new("Tool Library")
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .default_width(880.0)
        .default_height(560.0)
        .open(&mut still_open)
        .show(ctx, |ui| draw_content(ui, modal, events));

    if !still_open {
        events.push(AppEvent::CloseToolLibrary);
    }
}

fn draw_content(
    ui: &mut egui::Ui,
    modal: &crate::state::ToolLibraryModalState,
    events: &mut Vec<AppEvent>,
) {
    let view_id = egui::Id::new("toollib_view");
    let mut view: ToolLibraryView = ui.data(|d| d.get_temp(view_id).unwrap_or_default());

    clamp_selection(&mut view, modal);

    egui::SidePanel::left("toollib_left")
        .resizable(true)
        .default_width(340.0)
        .min_width(280.0)
        .show_inside(ui, |ui| {
            draw_catalog_panel(ui, modal, &mut view, events);
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
        draw_detail_panel(ui, modal, &mut view, events);
    });

    ui.data_mut(|d| d.insert_temp(view_id, view));
}

/// Reset stale selection after a snapshot refresh (a catalog or tool the
/// view points at may have been deleted/renamed out from under it).
fn clamp_selection(view: &mut ToolLibraryView, modal: &crate::state::ToolLibraryModalState) {
    let Some(cat) = view.selected_catalog.clone() else {
        view.selected_index = None;
        return;
    };
    match catalog_tools(modal, &cat) {
        Some(tools) => {
            if let Some(i) = view.selected_index
                && i >= tools.len()
            {
                view.selected_index = None;
                view.editing = false;
                view.draft = None;
            }
        }
        None => {
            view.selected_catalog = None;
            view.selected_index = None;
            view.editing = false;
            view.draft = None;
        }
    }
}

fn catalog_tools<'a>(
    modal: &'a crate::state::ToolLibraryModalState,
    name: &str,
) -> Option<&'a [ToolConfig]> {
    modal
        .catalogs
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, c)| c.tools.as_slice())
}

// ── Left panel: catalogs + tool list ────────────────────────────────

fn draw_catalog_panel(
    ui: &mut egui::Ui,
    modal: &crate::state::ToolLibraryModalState,
    view: &mut ToolLibraryView,
    events: &mut Vec<AppEvent>,
) {
    ui.label(
        egui::RichText::new("Catalogs")
            .strong()
            .color(theme::TEXT_STRONG),
    );
    ui.add_space(2.0);

    // New catalog row.
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut view.new_catalog_name)
                .desired_width(160.0)
                .hint_text("new catalog name"),
        );
        let name = view.new_catalog_name.trim().to_owned();
        if ui
            .add_enabled(!name.is_empty(), egui::Button::new("Create"))
            .clicked()
        {
            events.push(AppEvent::CreateToolCatalog(name));
            view.new_catalog_name.clear();
        }
    });
    ui.add_space(2.0);

    // Catalog list.
    egui::ScrollArea::vertical()
        .id_salt("toollib_catalogs")
        .max_height(150.0)
        .show(ui, |ui| {
            if modal.catalogs.is_empty() {
                ui.label(
                    egui::RichText::new("No catalogs yet")
                        .italics()
                        .color(theme::TEXT_DIM),
                );
            }
            for (name, catalog) in &modal.catalogs {
                let selected = view.selected_catalog.as_deref() == Some(name.as_str());
                let label = format!("{name} ({})", catalog.tools.len());
                if ui.selectable_label(selected, label).clicked() {
                    view.selected_catalog = Some(name.clone());
                    view.selected_index = None;
                    view.editing = false;
                    view.draft = None;
                    view.confirm_delete_catalog = false;
                    view.rename_catalog_buf = name.clone();
                }
            }
        });

    // Selected-catalog management.
    if let Some(cat) = view.selected_catalog.clone() {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut view.rename_catalog_buf)
                    .desired_width(120.0)
                    .hint_text("rename to"),
            );
            let new_name = view.rename_catalog_buf.trim().to_owned();
            let can_rename = !new_name.is_empty() && new_name != cat;
            if ui
                .add_enabled(can_rename, egui::Button::new("Rename"))
                .clicked()
            {
                events.push(AppEvent::RenameToolCatalog {
                    old: cat.clone(),
                    new: new_name,
                });
            }
        });
        ui.horizontal(|ui| {
            if ui
                .button("Dedupe")
                .on_hover_text("Remove duplicate tools (same geometry), keeping the first.")
                .clicked()
            {
                events.push(AppEvent::DedupeToolCatalog(cat.clone()));
            }
            if view.confirm_delete_catalog {
                if ui
                    .button(egui::RichText::new("Confirm delete catalog").color(theme::ERROR))
                    .clicked()
                {
                    events.push(AppEvent::DeleteToolCatalog(cat.clone()));
                    view.selected_catalog = None;
                    view.selected_index = None;
                    view.confirm_delete_catalog = false;
                }
                if ui.button("Cancel").clicked() {
                    view.confirm_delete_catalog = false;
                }
            } else if ui.button("Delete catalog").clicked() {
                view.confirm_delete_catalog = true;
            }
        });
    }

    ui.separator();

    // Tool list controls.
    ui.horizontal(|ui| {
        ui.checkbox(&mut view.search_all, "Search all catalogs");
    });
    ui.add(
        egui::TextEdit::singleline(&mut view.filter)
            .desired_width(f32::INFINITY)
            .hint_text("filter by name"),
    );
    ui.add_space(2.0);

    // Tool rows.
    egui::ScrollArea::vertical()
        .id_salt("toollib_tools")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let filter = view.filter.to_lowercase();
            let mut any = false;
            // Collect (catalog, index) so we can mutate `view` after the
            // immutable borrow of `modal` ends per row.
            let rows: Vec<(String, usize)> = if view.search_all {
                modal
                    .catalogs
                    .iter()
                    .flat_map(|(name, c)| {
                        c.tools
                            .iter()
                            .enumerate()
                            .map(move |(i, _)| (name.clone(), i))
                    })
                    .collect()
            } else if let Some(cat) = view.selected_catalog.clone() {
                catalog_tools(modal, &cat)
                    .map(|tools| (0..tools.len()).map(|i| (cat.clone(), i)).collect())
                    .unwrap_or_default()
            } else {
                Vec::new()
            };

            if rows.is_empty() {
                ui.label(
                    egui::RichText::new("Select a catalog to see its tools.")
                        .italics()
                        .color(theme::TEXT_DIM),
                );
            }

            for (cat, idx) in rows {
                let Some(tool) = catalog_tools(modal, &cat).and_then(|t| t.get(idx)) else {
                    continue;
                };
                if !filter.is_empty() && !tool.name.to_lowercase().contains(&filter) {
                    continue;
                }
                any = true;
                let selected = view.selected_catalog.as_deref() == Some(cat.as_str())
                    && view.selected_index == Some(idx);
                let prefix = if view.search_all {
                    format!("{cat} / ")
                } else {
                    String::new()
                };
                let label = format!("{prefix}{}  ·  {}", tool.name, row_summary(tool));
                if ui.selectable_label(selected, label).clicked() {
                    view.selected_catalog = Some(cat.clone());
                    view.selected_index = Some(idx);
                    view.editing = false;
                    view.draft = None;
                    view.confirm_delete_tool = false;
                    view.move_target.clear();
                }
            }

            if !any && !filter.is_empty() {
                ui.label(
                    egui::RichText::new("No tools match the filter.")
                        .italics()
                        .color(theme::TEXT_DIM),
                );
            }
        });
}

/// Compact one-line summary for a tool row: type, diameter, flutes, angle.
fn row_summary(tool: &ToolConfig) -> String {
    let angle = match tool.tool_type {
        ToolType::VBit => format!(" · {:.0}°", tool.included_angle),
        ToolType::TaperedBallNose => format!(" · {:.1}° taper", tool.taper_half_angle),
        _ => String::new(),
    };
    format!(
        "{} · ⌀{:.2}mm · {}F{angle}",
        tool.tool_type.label(),
        tool.diameter,
        tool.flute_count
    )
}

// ── Right panel: preview + detail + actions ─────────────────────────

fn draw_detail_panel(
    ui: &mut egui::Ui,
    modal: &crate::state::ToolLibraryModalState,
    view: &mut ToolLibraryView,
    events: &mut Vec<AppEvent>,
) {
    let (Some(cat), Some(idx)) = (view.selected_catalog.clone(), view.selected_index) else {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(
                egui::RichText::new("Select a tool to preview and edit it.").color(theme::TEXT_DIM),
            );
        });
        return;
    };
    let Some(tool) = catalog_tools(modal, &cat).and_then(|t| t.get(idx)).cloned() else {
        return;
    };

    if view.editing {
        draw_edit_form(ui, &cat, idx, view, events);
        return;
    }

    ui.horizontal(|ui| {
        ui.heading(&tool.name);
        ui.label(
            egui::RichText::new(format!("in {cat}"))
                .small()
                .color(theme::TEXT_DIM),
        );
    });
    ui.separator();

    egui::ScrollArea::vertical().show(ui, |ui| {
        crate::ui::properties::tool::draw_tool_preview(ui, &tool);
        ui.add_space(8.0);
        draw_readonly_grid(ui, &tool);

        ui.add_space(10.0);
        ui.separator();

        // Primary actions.
        ui.horizontal(|ui| {
            if ui
                .button("➕ Add to project")
                .on_hover_text("Copy a snapshot of this tool into the open project.")
                .clicked()
            {
                events.push(AppEvent::AddToolFromLibrary(Box::new(tool.clone())));
            }
            if ui.button("✏ Edit").clicked() {
                view.editing = true;
                view.draft = Some(tool.clone());
            }
            if view.confirm_delete_tool {
                if ui
                    .button(egui::RichText::new("Confirm delete").color(theme::ERROR))
                    .clicked()
                {
                    events.push(AppEvent::DeleteLibraryTool {
                        catalog: cat.clone(),
                        index: idx,
                    });
                    view.selected_index = None;
                    view.confirm_delete_tool = false;
                }
                if ui.button("Cancel").clicked() {
                    view.confirm_delete_tool = false;
                }
            } else if ui.button("🗑 Delete").clicked() {
                view.confirm_delete_tool = true;
            }
        });

        // Move-to-catalog row.
        let targets: Vec<String> = modal
            .catalogs
            .iter()
            .map(|(n, _)| n.clone())
            .filter(|n| n != &cat)
            .collect();
        if !targets.is_empty() {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Move to:");
                egui::ComboBox::from_id_salt("toollib_move_target")
                    .selected_text(if view.move_target.is_empty() {
                        "(choose catalog)".to_owned()
                    } else {
                        view.move_target.clone()
                    })
                    .show_ui(ui, |ui| {
                        for t in &targets {
                            ui.selectable_value(&mut view.move_target, t.clone(), t);
                        }
                    });
                let can_move = targets.contains(&view.move_target);
                if ui
                    .add_enabled(can_move, egui::Button::new("Move"))
                    .clicked()
                {
                    events.push(AppEvent::MoveLibraryTool {
                        from: cat.clone(),
                        index: idx,
                        to: view.move_target.clone(),
                    });
                    view.selected_index = None;
                    view.move_target.clear();
                }
            });
        }
    });
}

fn draw_readonly_grid(ui: &mut egui::Ui, tool: &ToolConfig) {
    egui::Grid::new("toollib_detail_grid")
        .num_columns(2)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            row(ui, "Type", tool.tool_type.label().to_owned());
            row(ui, "Diameter", format!("{:.3} mm", tool.diameter));
            row(
                ui,
                "Cutting length",
                format!("{:.2} mm", tool.cutting_length),
            );
            row(ui, "Flutes", tool.flute_count.to_string());
            match tool.tool_type {
                ToolType::VBit => {
                    row(ui, "Included angle", format!("{:.1}°", tool.included_angle));
                }
                ToolType::TaperedBallNose => {
                    row(
                        ui,
                        "Taper half-angle",
                        format!("{:.2}°", tool.taper_half_angle),
                    );
                    row(
                        ui,
                        "Shaft diameter",
                        format!("{:.3} mm", tool.shaft_diameter),
                    );
                }
                ToolType::BullNose => {
                    row(ui, "Corner radius", format!("{:.2} mm", tool.corner_radius));
                }
                _ => {}
            }
            row(
                ui,
                "Shank diameter",
                format!("{:.3} mm", tool.shank_diameter),
            );
            row(ui, "Material", tool.tool_material.label().to_owned());
            row(ui, "Cut direction", tool.cut_direction.label().to_owned());
            if !tool.vendor.is_empty() {
                row(ui, "Vendor", tool.vendor.clone());
            }
            if !tool.product_id.is_empty() {
                row(ui, "Product ID", tool.product_id.clone());
            }
        });
}

fn row(ui: &mut egui::Ui, label: &str, value: String) {
    ui.label(egui::RichText::new(label).small().color(theme::TEXT_DIM));
    ui.label(value);
    ui.end_row();
}

fn draw_edit_form(
    ui: &mut egui::Ui,
    cat: &str,
    idx: usize,
    view: &mut ToolLibraryView,
    events: &mut Vec<AppEvent>,
) {
    let Some(draft) = view.draft.as_mut() else {
        view.editing = false;
        return;
    };
    ui.horizontal(|ui| {
        ui.heading("Edit tool");
        ui.label(
            egui::RichText::new(format!("in {cat}"))
                .small()
                .color(theme::TEXT_DIM),
        );
    });
    ui.separator();

    let mut save = false;
    let mut cancel = false;
    egui::ScrollArea::vertical().show(ui, |ui| {
        crate::ui::properties::tool::draw_tool_fields(ui, draft);
        ui.add_space(8.0);
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("💾 Save").clicked() {
                save = true;
            }
            if ui.button("Cancel").clicked() {
                cancel = true;
            }
        });
    });

    if save {
        events.push(AppEvent::UpdateLibraryTool {
            catalog: cat.to_owned(),
            index: idx,
            tool: Box::new(draft.clone()),
        });
        view.editing = false;
        view.draft = None;
    } else if cancel {
        view.editing = false;
        view.draft = None;
    }
}
