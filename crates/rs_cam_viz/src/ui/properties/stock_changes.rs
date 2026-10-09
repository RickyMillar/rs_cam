//! S4 (`planning/stock_additions_2026-10-09/PLAN.md`): the setup panel's
//! "Stock changes" section.
//!
//! The section lists the setup's stock changes in application order. Each
//! row shows the enable checkbox, the name, Add or Remove, the geometry
//! kind, the material and, after a simulation, the volume. The editor
//! below the list edits a DRAFT held in `state.panels.stock_change_editor`.
//!
//! The section writes nothing. Every action is an [`AppEvent::EditStockChanges`]
//! that the controller turns into one of the four S2 commands
//! (`handle_stock_change_intent`), so the panel has no parallel path. A
//! refusal comes back into the editor in the core's words.

use rs_cam_core::compute::stock_change::{
    StockChange, StockChangeOp, StockGeometry, next_stock_change_id,
};
use rs_cam_core::ids::{ModelId, StockChangeId};
use rs_cam_core::material::Material;
use rs_cam_core::session::StockChangeRow;

use crate::state::job::SetupId;
use crate::state::panels::StockChangeEditor;
use crate::ui::AppEvent;
use crate::ui::components::{Banner, Button, ChoiceRow, Role, UiExt as _, ValueRow};
use crate::ui::tokens;

/// One action on a setup's stock-change list. The controller turns each
/// into one S2 command.
#[derive(Debug, Clone, PartialEq)]
pub enum StockChangeIntent {
    /// Apply the editor's draft: `AddStockChange` when `is_new`, else
    /// `ReplaceStockChange`.
    Save {
        /// The draft.
        change: Box<StockChange>,
        /// Add (true) or replace (false).
        is_new: bool,
    },
    /// Flip one change on or off: `ReplaceStockChange` with the stored
    /// record and the new flag.
    SetEnabled {
        /// The change.
        change_id: StockChangeId,
        /// The new flag.
        enabled: bool,
    },
    /// `MoveStockChange`.
    Move {
        /// The change.
        change_id: StockChangeId,
        /// The new position in the list.
        to_position: usize,
    },
    /// `RemoveStockChange`.
    Remove(StockChangeId),
}

/// The geometry kinds, as the editor's closed choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryKind {
    /// [`StockGeometry::Model`].
    Model,
    /// [`StockGeometry::OutlineFill`].
    OutlineFill,
    /// [`StockGeometry::OutlineExtrude`].
    OutlineExtrude,
}

impl GeometryKind {
    /// The kind of a geometry.
    #[must_use]
    pub fn of(geometry: &StockGeometry) -> Self {
        match geometry {
            StockGeometry::Model { .. } => Self::Model,
            StockGeometry::OutlineFill { .. } => Self::OutlineFill,
            StockGeometry::OutlineExtrude { .. } => Self::OutlineExtrude,
        }
    }
}

/// One project model, with the facts the editor filters on.
#[derive(Debug, Clone)]
pub struct ModelChoice {
    /// The model id.
    pub id: ModelId,
    /// The model name.
    pub name: String,
    /// The model holds a mesh (a `Model` geometry can use it).
    pub has_mesh: bool,
    /// The model holds a closed 2D outline (an outline geometry can use it).
    pub has_closed_outline: bool,
}

/// What the section reads. The caller builds it from the session.
pub struct StockChangesInputs<'a> {
    /// The setup.
    pub setup_id: SetupId,
    /// The setup's rows, in application order
    /// (`ProjectSession::stock_change_rows`, filtered to the setup).
    pub rows: &'a [StockChangeRow],
    /// Every project model.
    pub models: &'a [ModelChoice],
    /// The stock Z range in this setup's frame, `(bottom, top)` in mm.
    pub stock_z: (f64, f64),
    /// The stock material: the material a new change starts with.
    pub stock_material: &'a Material,
}

const OP_OPTIONS: &[(StockChangeOp, &str)] = &[
    (StockChangeOp::Add, "Add"),
    (StockChangeOp::Remove, "Remove"),
];

const KIND_OPTIONS: &[(GeometryKind, &str)] = &[
    (GeometryKind::Model, "Model"),
    (GeometryKind::OutlineFill, "Fill"),
    (GeometryKind::OutlineExtrude, "Extrude"),
];

const SECTION_HOVER: &str = "Material that this setup adds to the stock or removes from \
     the stock before its first toolpath. The simulation applies the list from top to \
     bottom. A change after the first setup also changes the stock of the later setups.";

const OP_HOVER: &str = "Add: put the geometry into the stock as the selected material. \
     The geometry can go above the stock top.\nRemove: take the geometry out of every \
     material. A remove cannot use a fill.";

const KIND_HOVER: &str = "Model: a closed mesh model.\nFill: fill every space that is open \
     to the top (this setup's +Z), inside the closed 2D outlines, up to the level Z. A \
     fill fills what the earlier toolpaths cut.\nExtrude: a prism from the closed 2D \
     outlines, from Z from to Z to.";

const FRAME_HOVER: &str = "This value is in this setup's frame: the frame that the \
     toolpaths of this setup use. This setup's +Z is up.";

const MATERIAL_HOVER: &str = "The material that an Add puts into the stock. Pick one from \
     the library, or type a custom name. A Remove ignores the material.";

const VOLUME_HOVER: &str = "The volume that the last simulation added (+) or removed (-) \
     for this change, in ml. Run the simulation to measure it.";

/// The text of one row's second line: Add or Remove, the kind, the material.
#[must_use]
pub fn row_summary(row: &StockChangeRow) -> String {
    let op = match row.change.op {
        StockChangeOp::Add => "Add",
        StockChangeOp::Remove => "Remove",
    };
    let kind = match GeometryKind::of(&row.change.geometry) {
        GeometryKind::Model => "model",
        GeometryKind::OutlineFill => "fill",
        GeometryKind::OutlineExtrude => "extrude",
    };
    match row.change.op {
        StockChangeOp::Add => format!("{op} · {kind} · {}", row.material_label()),
        StockChangeOp::Remove => format!("{op} · {kind}"),
    }
}

/// A new draft for `setup_id`: the next id, the stock material, and a fill
/// up to the stock top.
#[must_use]
pub fn new_draft(
    setup_id: SetupId,
    rows: &[StockChangeRow],
    stock_z: (f64, f64),
    stock_material: &Material,
) -> StockChangeEditor {
    let existing: Vec<StockChange> = rows.iter().map(|r| r.change.clone()).collect();
    let id = next_stock_change_id(&existing);
    StockChangeEditor {
        setup_id,
        is_new: true,
        draft: StockChange {
            id,
            name: format!("Change {}", id.0 + 1),
            enabled: true,
            op: StockChangeOp::Add,
            geometry: StockGeometry::OutlineFill {
                model_ids: Vec::new(),
                level_z: stock_z.1,
            },
            material: stock_material.clone(),
            display_colour: None,
        },
        refusal: None,
    }
}

/// Change the draft's geometry to `kind`. The model ids and the Z values
/// carry over where the new kind can hold them.
pub fn set_geometry_kind(
    geometry: &mut StockGeometry,
    kind: GeometryKind,
    models: &[ModelChoice],
    stock_z: (f64, f64),
) {
    if GeometryKind::of(geometry) == kind {
        return;
    }
    let ids = geometry.model_ids();
    let top = match geometry {
        StockGeometry::OutlineFill { level_z, .. } => *level_z,
        StockGeometry::OutlineExtrude { z_top, .. } => *z_top,
        StockGeometry::Model { .. } => stock_z.1,
    };
    let outline_ids: Vec<ModelId> = ids
        .iter()
        .copied()
        .filter(|id| models.iter().any(|m| m.id == *id && m.has_closed_outline))
        .collect();
    *geometry = match kind {
        GeometryKind::Model => StockGeometry::Model {
            model_id: ids
                .iter()
                .copied()
                .find(|id| models.iter().any(|m| m.id == *id && m.has_mesh))
                .or_else(|| models.iter().find(|m| m.has_mesh).map(|m| m.id))
                .unwrap_or(ModelId(0)),
        },
        GeometryKind::OutlineFill => StockGeometry::OutlineFill {
            model_ids: outline_ids,
            level_z: top,
        },
        GeometryKind::OutlineExtrude => StockGeometry::OutlineExtrude {
            model_ids: outline_ids,
            z_bottom: stock_z.0.min(top - 1.0),
            z_top: top,
        },
    };
}

/// Draw the section. Opens and closes the editor in `editor`; every write
/// is an event.
pub fn draw(
    ui: &mut egui::Ui,
    inputs: &StockChangesInputs<'_>,
    editor: &mut Option<StockChangeEditor>,
    events: &mut Vec<AppEvent>,
) {
    let setup_id = inputs.setup_id;
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new("Stock changes")
            .strong()
            .color(tokens::TEXT_STRONG),
    )
    .on_hover_text(SECTION_HOVER);

    if inputs.rows.is_empty() {
        ui.label(
            egui::RichText::new("No stock changes")
                .italics()
                .color(tokens::TEXT_MUTED),
        );
    }
    let count = inputs.rows.len();
    for (position, row) in inputs.rows.iter().enumerate() {
        draw_row(ui, setup_id, row, position, count, editor, events);
    }

    let editing_here = editor.as_ref().is_some_and(|e| e.setup_id == setup_id);
    if !editing_here
        && ui
            .small_button("+ Add Stock Change")
            .on_hover_text(SECTION_HOVER)
            .clicked()
    {
        *editor = Some(new_draft(
            setup_id,
            inputs.rows,
            inputs.stock_z,
            inputs.stock_material,
        ));
    }

    if let Some(open) = editor.as_mut()
        && open.setup_id == setup_id
    {
        match draw_editor(ui, inputs, open) {
            EditorAction::None => {}
            EditorAction::Apply => events.push(AppEvent::EditStockChanges(
                setup_id,
                StockChangeIntent::Save {
                    change: Box::new(open.draft.clone()),
                    is_new: open.is_new,
                },
            )),
            EditorAction::Cancel => *editor = None,
        }
    }
}

fn draw_row(
    ui: &mut egui::Ui,
    setup_id: SetupId,
    row: &StockChangeRow,
    position: usize,
    count: usize,
    editor: &mut Option<StockChangeEditor>,
    events: &mut Vec<AppEvent>,
) {
    let change_id = row.change.id;
    let mut open_editor = false;
    ui.horizontal(|ui| {
        let mut enabled = row.change.enabled;
        if ui
            .checkbox(&mut enabled, "")
            .on_hover_text("On: the simulation applies this change. Off: it does not.")
            .changed()
        {
            events.push(AppEvent::EditStockChanges(
                setup_id,
                StockChangeIntent::SetEnabled { change_id, enabled },
            ));
        }
        let editing = editor
            .as_ref()
            .is_some_and(|e| e.setup_id == setup_id && !e.is_new && e.draft.id == change_id);
        let name = if row.change.name.is_empty() {
            format!("Change {}", change_id.0 + 1)
        } else {
            row.change.name.clone()
        };
        let response = ui
            .selectable_label(editing, name)
            .on_hover_text("Click to edit this change. Right-click for more actions.");
        if response.clicked() {
            open_editor = true;
        }
        response.context_menu(|ui| {
            if ui.button("Edit").clicked() {
                open_editor = true;
                ui.close();
            }
            if ui
                .add_enabled(position > 0, egui::Button::new("Move Up"))
                .on_hover_text("Apply this change earlier.")
                .clicked()
            {
                events.push(AppEvent::EditStockChanges(
                    setup_id,
                    StockChangeIntent::Move {
                        change_id,
                        to_position: position.saturating_sub(1),
                    },
                ));
                ui.close();
            }
            if ui
                .add_enabled(position + 1 < count, egui::Button::new("Move Down"))
                .on_hover_text("Apply this change later.")
                .clicked()
            {
                events.push(AppEvent::EditStockChanges(
                    setup_id,
                    StockChangeIntent::Move {
                        change_id,
                        to_position: position + 1,
                    },
                ));
                ui.close();
            }
            ui.separator();
            if ui.button("Delete").clicked() {
                events.push(AppEvent::EditStockChanges(
                    setup_id,
                    StockChangeIntent::Remove(change_id),
                ));
                ui.close();
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(row.volume_label()).color(tokens::TEXT_MUTED))
                .on_hover_text(VOLUME_HOVER);
        });
    });
    ui.horizontal(|ui| {
        ui.add_space(tokens::SPACE_5);
        ui.label(
            egui::RichText::new(row_summary(row))
                .small()
                .color(tokens::TEXT_MUTED),
        );
    });
    if open_editor {
        *editor = Some(StockChangeEditor {
            setup_id,
            is_new: false,
            draft: row.change.clone(),
            refusal: None,
        });
    }
}

enum EditorAction {
    None,
    Apply,
    Cancel,
}

fn draw_editor(
    ui: &mut egui::Ui,
    inputs: &StockChangesInputs<'_>,
    editor: &mut StockChangeEditor,
) -> EditorAction {
    let mut action = EditorAction::None;
    let before = editor.draft.clone();
    let draft = &mut editor.draft;
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(if editor.is_new {
            "New stock change"
        } else {
            "Edit stock change"
        })
        .strong()
        .color(tokens::TEXT_MUTED),
    );
    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.text_edit_singleline(&mut draft.name);
    });
    ui.add(ChoiceRow::new("Change", &mut draft.op, OP_OPTIONS).hover(OP_HOVER));
    let mut kind = GeometryKind::of(&draft.geometry);
    if ui
        .add(ChoiceRow::new("Geometry", &mut kind, KIND_OPTIONS).hover(KIND_HOVER))
        .changed()
    {
        set_geometry_kind(&mut draft.geometry, kind, inputs.models, inputs.stock_z);
    }

    draw_model_pick(ui, &mut draft.geometry, inputs.models);

    ui.label(
        egui::RichText::new(format!(
            "Stock in this setup: Z {:.2} to {:.2} mm",
            inputs.stock_z.0, inputs.stock_z.1
        ))
        .small()
        .color(tokens::TEXT_MUTED),
    )
    .on_hover_text(FRAME_HOVER);
    ui.param_grid("stock_change_z", |ui| match &mut draft.geometry {
        StockGeometry::Model { .. } => {}
        StockGeometry::OutlineFill { level_z, .. } => {
            let _ = ValueRow::new("Level Z", level_z, " mm", 0.1, f64::MIN..=f64::MAX)
                .tooltip(Some(FRAME_HOVER))
                .show(ui);
        }
        StockGeometry::OutlineExtrude {
            z_bottom, z_top, ..
        } => {
            let _ = ValueRow::new("Z from", z_bottom, " mm", 0.1, f64::MIN..=f64::MAX)
                .tooltip(Some(FRAME_HOVER))
                .show(ui);
            let _ = ValueRow::new("Z to", z_top, " mm", 0.1, f64::MIN..=f64::MAX)
                .tooltip(Some(FRAME_HOVER))
                .show(ui);
        }
    });

    if draft.op == StockChangeOp::Add {
        draw_material_pick(ui, &mut draft.material);
    }

    if editor.draft != before {
        editor.refusal = None;
    }
    if let Some(refusal) = &editor.refusal {
        let _ = Banner::new(Role::Danger, refusal.clone()).show(ui);
    }
    ui.horizontal(|ui| {
        if ui
            .add(Button::primary("Apply"))
            .on_hover_text("Write this change to the setup.")
            .clicked()
        {
            action = EditorAction::Apply;
        }
        if ui
            .add(Button::quiet("Cancel"))
            .on_hover_text("Close the editor. The setup keeps its changes as they are.")
            .clicked()
        {
            action = EditorAction::Cancel;
        }
    });
    action
}

/// The model pick: one mesh model for `Model`, a set of outline models for
/// the outline kinds. Only the models the kind can use are offered.
fn draw_model_pick(ui: &mut egui::Ui, geometry: &mut StockGeometry, models: &[ModelChoice]) {
    match geometry {
        StockGeometry::Model { model_id } => {
            let meshes: Vec<&ModelChoice> = models.iter().filter(|m| m.has_mesh).collect();
            if meshes.is_empty() {
                ui.label(
                    egui::RichText::new("No mesh models loaded")
                        .italics()
                        .color(tokens::TEXT_MUTED),
                );
                return;
            }
            let selected = meshes
                .iter()
                .find(|m| m.id == *model_id)
                .map_or_else(|| "Pick a model".to_owned(), |m| m.name.clone());
            ui.horizontal(|ui| {
                ui.label("Model:");
                egui::ComboBox::from_id_salt("stock_change_mesh")
                    .selected_text(selected)
                    .show_ui(ui, |ui| {
                        for m in &meshes {
                            if ui.selectable_label(m.id == *model_id, &m.name).clicked() {
                                *model_id = m.id;
                            }
                        }
                    })
                    .response
                    .on_hover_text("A closed mesh model, in this setup's frame.");
            });
        }
        StockGeometry::OutlineFill { model_ids, .. }
        | StockGeometry::OutlineExtrude { model_ids, .. } => {
            let outlines: Vec<&ModelChoice> =
                models.iter().filter(|m| m.has_closed_outline).collect();
            ui.label("Outlines:")
                .on_hover_text("The 2D models whose closed outlines bound the change.");
            if outlines.is_empty() {
                ui.label(
                    egui::RichText::new("No 2D models with a closed outline")
                        .italics()
                        .color(tokens::TEXT_MUTED),
                );
            }
            for m in outlines {
                let mut checked = model_ids.contains(&m.id);
                if ui.checkbox(&mut checked, m.name.as_str()).changed() {
                    if checked {
                        model_ids.push(m.id);
                    } else {
                        model_ids.retain(|id| *id != m.id);
                    }
                }
            }
        }
    }
}

/// The material pick: the library picker, and a custom name.
fn draw_material_pick(ui: &mut egui::Ui, material: &mut Material) {
    let _ = super::stock::draw_hierarchical_material_picker(ui, material);
    let mut custom = match material {
        Material::Custom { name, .. } => name.clone(),
        _ => String::new(),
    };
    ui.horizontal(|ui| {
        ui.label("Custom:");
        let response = ui
            .add(
                egui::TextEdit::singleline(&mut custom)
                    .hint_text("name")
                    .desired_width(140.0),
            )
            .on_hover_text(MATERIAL_HOVER);
        if response.changed() {
            let trimmed = custom.trim();
            if !trimmed.is_empty() {
                let feed_scale_factor = match material {
                    Material::Custom {
                        feed_scale_factor, ..
                    } => *feed_scale_factor,
                    _ => 1.0,
                };
                *material = Material::Custom {
                    name: custom.clone(),
                    feed_scale_factor,
                };
            }
        }
    });
}

#[cfg(test)]
#[allow(
    // SAFETY: test code; a failed lookup is a failed test.
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use rs_cam_core::session::StockChangeVolumeAbsence;

    fn models() -> Vec<ModelChoice> {
        vec![
            ModelChoice {
                id: ModelId(1),
                name: "part.stl".to_owned(),
                has_mesh: true,
                has_closed_outline: false,
            },
            ModelChoice {
                id: ModelId(2),
                name: "channels.dxf".to_owned(),
                has_mesh: false,
                has_closed_outline: true,
            },
        ]
    }

    fn row(change: StockChange) -> StockChangeRow {
        StockChangeRow {
            setup_index: 0,
            setup_id: 0,
            setup_name: "Setup 1".to_owned(),
            position: 0,
            change,
            volume: Err(StockChangeVolumeAbsence::NotSimulated),
        }
    }

    #[test]
    fn a_new_draft_takes_the_next_id_the_stock_material_and_the_stock_top() {
        let material = Material::Custom {
            name: "Resin".to_owned(),
            feed_scale_factor: 1.0,
        };
        let first = new_draft(SetupId(0), &[], (-20.0, 0.0), &material);
        assert!(first.is_new);
        assert_eq!(first.draft.id, StockChangeId(0));
        assert_eq!(first.draft.material, material);
        assert_eq!(
            first.draft.geometry,
            StockGeometry::OutlineFill {
                model_ids: Vec::new(),
                level_z: 0.0
            }
        );
        let mut held = first.draft;
        held.id = StockChangeId(3);
        let second = new_draft(SetupId(0), &[row(held)], (-20.0, 0.0), &material);
        assert_eq!(second.draft.id, StockChangeId(4));
    }

    #[test]
    fn a_kind_switch_keeps_the_models_the_new_kind_can_use() {
        let models = models();
        let mut geometry = StockGeometry::OutlineFill {
            model_ids: vec![ModelId(2)],
            level_z: 3.0,
        };
        set_geometry_kind(
            &mut geometry,
            GeometryKind::OutlineExtrude,
            &models,
            (-20.0, 0.0),
        );
        assert_eq!(
            geometry,
            StockGeometry::OutlineExtrude {
                model_ids: vec![ModelId(2)],
                z_bottom: -20.0,
                z_top: 3.0
            }
        );
        set_geometry_kind(&mut geometry, GeometryKind::Model, &models, (-20.0, 0.0));
        assert_eq!(
            geometry,
            StockGeometry::Model {
                model_id: ModelId(1)
            },
            "the outline model is no mesh, so the first mesh model is picked"
        );
        set_geometry_kind(
            &mut geometry,
            GeometryKind::OutlineFill,
            &models,
            (-20.0, 0.0),
        );
        assert_eq!(
            geometry,
            StockGeometry::OutlineFill {
                model_ids: Vec::new(),
                level_z: 0.0
            },
            "the mesh model has no outline, so the fill starts empty at the stock top"
        );
    }

    #[test]
    fn the_row_summary_names_the_op_the_kind_and_an_add_material() {
        let material = Material::Custom {
            name: "Resin".to_owned(),
            feed_scale_factor: 1.0,
        };
        let mut change = new_draft(SetupId(0), &[], (0.0, 10.0), &material).draft;
        assert_eq!(row_summary(&row(change.clone())), "Add · fill · Resin");
        change.op = StockChangeOp::Remove;
        change.geometry = StockGeometry::Model {
            model_id: ModelId(1),
        };
        assert_eq!(row_summary(&row(change)), "Remove · model");
    }
}
