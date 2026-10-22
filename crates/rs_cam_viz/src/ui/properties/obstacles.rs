//! K2 (`planning/op_architecture_2026-10-14/K2_PLAN.md`): the obstacle list
//! on the setup page and the obstacle editor on the `Selection::Obstacle`
//! page.
//!
//! An obstacle is a solid the tool must not enter: a clamp, a vise jaw, a
//! keep-out zone (K1: one concept). Every value is in the setup frame, and
//! the obstacle is fixed to the table, not to the part.
//!
//! The list writes nothing. Each row action is an `AppEvent` that the
//! controller turns into one core command. The editor edits a DRAFT held in
//! `state.panels.obstacle_editor`; Apply goes through
//! `panel_apply::apply_obstacle_draft`, and a refusal comes back into the
//! editor in the core's words (the S4 contract).

use rs_cam_core::compute::obstacle::{DEFAULT_OBSTACLE_BOX_MM, Obstacle, ObstacleGeometry};
use rs_cam_core::ids::{ModelId, ObstacleId};

use super::stock_changes::{ModelChoice, draw_mesh_pick, draw_outline_pick};
use crate::state::job::SetupId;
use crate::state::panels::ObstacleEditor;
use crate::state::selection::Selection;
use crate::ui::AppEvent;
use crate::ui::components::{Banner, Button, ChoiceRow, Role, SectionHeader, UiExt as _, ValueRow};
use crate::ui::tokens;
use crate::ui_command::UiCommand;

/// The geometry kinds, as the editor's closed choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObstacleKind {
    /// [`ObstacleGeometry::Box`].
    Box,
    /// [`ObstacleGeometry::OutlineExtrude`].
    OutlineExtrude,
    /// [`ObstacleGeometry::Model`].
    Model,
}

impl ObstacleKind {
    /// The kind of a geometry.
    #[must_use]
    pub fn of(geometry: &ObstacleGeometry) -> Self {
        match geometry {
            ObstacleGeometry::Box { .. } => Self::Box,
            ObstacleGeometry::OutlineExtrude { .. } => Self::OutlineExtrude,
            ObstacleGeometry::Model { .. } => Self::Model,
        }
    }
}

const KIND_OPTIONS: &[(ObstacleKind, &str)] = &[
    (ObstacleKind::Box, "Box"),
    (ObstacleKind::OutlineExtrude, "Extrude"),
    (ObstacleKind::Model, "Model"),
];

const SECTION_HOVER: &str = "Solids the tool must not enter: clamps, vise jaws and \
     keep-out zones. Every toolpath of this setup keeps out of each enabled obstacle, \
     grown by its clearance. An obstacle is fixed to the table: a flip of the part \
     does not move it.";

const KIND_HOVER: &str = "Box: an axis-aligned box.\nExtrude: a prism from the closed 2D \
     outlines, from Z from to Z to.\nModel: a mesh model. The tool keeps out of its \
     outline seen from above, from its top down.";

const FRAME_HOVER: &str = "This value is in this setup's frame: the frame that the \
     toolpaths of this setup use. This setup's +Z is up. The obstacle is fixed to the \
     table.";

const CLEARANCE_HOVER: &str = "The distance that the tool keeps from the obstacle, on \
     every side and above it.";

const ENABLE_HOVER: &str = "On: every toolpath of this setup keeps out of this obstacle. \
     Off: the obstacle has no effect.";

const REMOVE_HOVER: &str = "Remove this obstacle from the setup. Undo puts it back.";

/// What the list reads. The caller builds it from the session.
pub struct ObstacleListInputs<'a> {
    /// The setup.
    pub setup_id: SetupId,
    /// The setup's obstacles, in list order.
    pub obstacles: &'a [Obstacle],
    /// Every project model.
    pub models: &'a [ModelChoice],
    /// The obstacles that do not resolve, with the core text (D10).
    pub errors: &'a [(ObstacleId, String)],
    /// The selected obstacle of this setup, if any.
    pub selected: Option<ObstacleId>,
}

/// What the editor reads.
pub struct ObstacleEditorInputs<'a> {
    /// Every project model.
    pub models: &'a [ModelChoice],
    /// The stock Z range in this setup's frame, `(bottom, top)` in mm.
    pub stock_z: (f64, f64),
}

/// What the operator asked the editor to do this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorAction {
    /// Nothing.
    None,
    /// Write the draft to the setup.
    Apply,
    /// Put the draft back to the stored record.
    Cancel,
    /// Remove the obstacle.
    Remove,
}

/// The text of one row's second line: the kind and its main facts.
#[must_use]
pub fn row_summary(obstacle: &Obstacle, models: &[ModelChoice]) -> String {
    let clearance = format!("clearance {:.1} mm", obstacle.clearance);
    match &obstacle.geometry {
        ObstacleGeometry::Box { .. } => format!("box · {clearance}"),
        ObstacleGeometry::OutlineExtrude {
            model_ids,
            z_bottom,
            z_top,
        } => {
            let count = model_ids.len();
            let noun = if count == 1 { "outline" } else { "outlines" };
            format!("extrude · {count} {noun} · Z {z_bottom:.1} to {z_top:.1}")
        }
        ObstacleGeometry::Model { model_id } => {
            let name = models.iter().find(|m| m.id == *model_id).map_or_else(
                || format!("missing model {}", model_id.raw()),
                |m| m.name.clone(),
            );
            format!("model · {name}")
        }
    }
}

/// Change the geometry to `kind`. The Z range and the model ids carry over
/// where the new kind can hold them:
///
/// - Box to Extrude keeps the box Z range.
/// - Extrude to Box takes the default spans at the old bottom Z.
/// - Any kind to Model takes a mesh model the geometry reads, else the
///   first mesh model.
/// - Any kind to Extrude keeps the model ids that have a closed outline.
pub fn set_obstacle_kind(
    geometry: &mut ObstacleGeometry,
    kind: ObstacleKind,
    models: &[ModelChoice],
    stock_z: (f64, f64),
) {
    if ObstacleKind::of(geometry) == kind {
        return;
    }
    let ids = geometry.model_ids();
    let (bottom, top) = match geometry {
        ObstacleGeometry::Box { min, max } => (min[2], max[2]),
        ObstacleGeometry::OutlineExtrude {
            z_bottom, z_top, ..
        } => (*z_bottom, *z_top),
        ObstacleGeometry::Model { .. } => stock_z,
    };
    *geometry = match kind {
        ObstacleKind::Box => {
            let [w, d, h] = DEFAULT_OBSTACLE_BOX_MM;
            ObstacleGeometry::Box {
                min: [0.0, 0.0, bottom],
                max: [w, d, bottom + h],
            }
        }
        ObstacleKind::OutlineExtrude => ObstacleGeometry::OutlineExtrude {
            model_ids: ids
                .iter()
                .copied()
                .filter(|id| models.iter().any(|m| m.id == *id && m.has_closed_outline))
                .collect(),
            z_bottom: bottom,
            z_top: top,
        },
        ObstacleKind::Model => ObstacleGeometry::Model {
            model_id: ids
                .iter()
                .copied()
                .find(|id| models.iter().any(|m| m.id == *id && m.has_mesh))
                .or_else(|| models.iter().find(|m| m.has_mesh).map(|m| m.id))
                .unwrap_or(ModelId::placeholder()),
        },
    };
}

/// Draw the setup page's obstacle list. Every write is an event.
pub fn draw_list(ui: &mut egui::Ui, inputs: &ObstacleListInputs<'_>, events: &mut Vec<AppEvent>) {
    let setup_id = inputs.setup_id;
    ui.add_space(8.0);
    SectionHeader::new("Obstacles")
        .hover(SECTION_HOVER)
        .show(ui);
    for obstacle in inputs.obstacles {
        draw_row(ui, inputs, obstacle, events);
    }
    if ui
        .small_button("+ Add Obstacle")
        .on_hover_text(SECTION_HOVER)
        .clicked()
    {
        events.push(AppEvent::AddObstacle(setup_id));
    }
}

fn draw_row(
    ui: &mut egui::Ui,
    inputs: &ObstacleListInputs<'_>,
    obstacle: &Obstacle,
    events: &mut Vec<AppEvent>,
) {
    let setup_id = inputs.setup_id;
    let obstacle_id = obstacle.id;
    let select = || {
        AppEvent::Ui(UiCommand::Select(Selection::Obstacle(
            setup_id,
            obstacle_id,
        )))
    };
    ui.horizontal(|ui| {
        let mut enabled = obstacle.enabled;
        if ui
            .checkbox(&mut enabled, "")
            .on_hover_text(ENABLE_HOVER)
            .changed()
        {
            events.push(AppEvent::SetObstacleEnabled(setup_id, obstacle_id, enabled));
        }
        let name = if obstacle.name.is_empty() {
            format!("Obstacle {}", obstacle_id.raw() + 1)
        } else {
            obstacle.name.clone()
        };
        let response = ui
            .selectable_label(inputs.selected == Some(obstacle_id), name)
            .on_hover_text("Click to edit this obstacle. Right-click for more actions.");
        if response.clicked() {
            events.push(select());
        }
        response.context_menu(|ui| {
            if ui.button("Edit").clicked() {
                events.push(select());
                ui.close();
            }
            ui.separator();
            if ui.button("Delete").clicked() {
                events.push(AppEvent::RemoveObstacle(setup_id, obstacle_id));
                ui.close();
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(Button::quiet("Remove"))
                .on_hover_text(REMOVE_HOVER)
                .clicked()
            {
                events.push(AppEvent::RemoveObstacle(setup_id, obstacle_id));
            }
        });
    });
    ui.horizontal(|ui| {
        ui.add_space(tokens::SPACE_5);
        ui.label(
            egui::RichText::new(row_summary(obstacle, inputs.models))
                .small()
                .color(tokens::TEXT_MUTED),
        );
    });
    if let Some((_, error)) = inputs.errors.iter().find(|(id, _)| *id == obstacle_id) {
        let _ = Banner::new(Role::Danger, error.clone()).show(ui);
    }
}

/// Draw the obstacle editor over the draft in `editor`. The caller applies,
/// resets or removes on the returned [`EditorAction`].
///
/// The editor has no enabled checkbox: the list-row checkbox is the one
/// control for that field.
pub fn draw_editor(
    ui: &mut egui::Ui,
    inputs: &ObstacleEditorInputs<'_>,
    editor: &mut ObstacleEditor,
) -> EditorAction {
    let mut action = EditorAction::None;
    let before = editor.draft.clone();
    let draft = &mut editor.draft;
    ui.heading("Obstacle");
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.text_edit_singleline(&mut draft.name);
    });
    let mut kind = ObstacleKind::of(&draft.geometry);
    if ui
        .add(ChoiceRow::new("Geometry", &mut kind, KIND_OPTIONS).hover(KIND_HOVER))
        .changed()
    {
        set_obstacle_kind(&mut draft.geometry, kind, inputs.models, inputs.stock_z);
    }

    match &mut draft.geometry {
        ObstacleGeometry::Model { model_id } => draw_mesh_pick(
            ui,
            "obstacle_mesh",
            model_id,
            inputs.models,
            "A mesh model. Its coordinates are in this setup's frame.",
        ),
        ObstacleGeometry::OutlineExtrude { model_ids, .. } => draw_outline_pick(
            ui,
            model_ids,
            inputs.models,
            "The 2D models whose closed outlines give the obstacle section.",
        ),
        ObstacleGeometry::Box { .. } => {}
    }

    ui.label(
        egui::RichText::new(format!(
            "Setup frame, fixed to the table. Stock: Z {:.2} to {:.2} mm",
            inputs.stock_z.0, inputs.stock_z.1
        ))
        .small()
        .color(tokens::TEXT_MUTED),
    )
    .on_hover_text(FRAME_HOVER);
    ui.param_grid("obstacle_geometry", |ui| {
        match &mut draft.geometry {
            ObstacleGeometry::Box { min, max } => {
                let labels = [("X min", "X max"), ("Y min", "Y max"), ("Z min", "Z max")];
                for ((lo, hi), (label_min, label_max)) in
                    min.iter_mut().zip(max.iter_mut()).zip(labels)
                {
                    let _ = ValueRow::new(label_min, lo, " mm", 0.5, f64::MIN..=f64::MAX)
                        .tooltip(Some(FRAME_HOVER))
                        .show(ui);
                    let _ = ValueRow::new(label_max, hi, " mm", 0.5, f64::MIN..=f64::MAX)
                        .tooltip(Some(FRAME_HOVER))
                        .show(ui);
                }
            }
            ObstacleGeometry::OutlineExtrude {
                z_bottom, z_top, ..
            } => {
                let _ = ValueRow::new("Z from", z_bottom, " mm", 0.1, f64::MIN..=f64::MAX)
                    .tooltip(Some(FRAME_HOVER))
                    .show(ui);
                let _ = ValueRow::new("Z to", z_top, " mm", 0.1, f64::MIN..=f64::MAX)
                    .tooltip(Some(FRAME_HOVER))
                    .show(ui);
            }
            ObstacleGeometry::Model { .. } => {}
        }
        let _ = ValueRow::new("Clearance", &mut draft.clearance, " mm", 0.1, 0.0..=100.0)
            .tooltip(Some(CLEARANCE_HOVER))
            .show(ui);
    });

    if editor.draft != before {
        editor.refusal = None;
    }
    if let Some(refusal) = &editor.refusal {
        let _ = Banner::new(Role::Danger, refusal.clone()).show(ui);
    }
    ui.horizontal(|ui| {
        if ui
            .add(Button::primary("Apply"))
            .on_hover_text("Write this obstacle to the setup.")
            .clicked()
        {
            action = EditorAction::Apply;
        }
        if ui
            .add(Button::quiet("Cancel"))
            .on_hover_text("Put the editor back to the obstacle that the setup holds.")
            .clicked()
        {
            action = EditorAction::Cancel;
        }
        if ui
            .add(Button::danger("Remove"))
            .on_hover_text(REMOVE_HOVER)
            .clicked()
        {
            action = EditorAction::Remove;
        }
    });
    action
}

#[cfg(test)]
mod tests {
    use super::*;

    fn models() -> Vec<ModelChoice> {
        vec![
            ModelChoice {
                id: ModelId::from_raw(1),
                name: "clamp.stl".to_owned(),
                has_mesh: true,
                has_closed_outline: false,
            },
            ModelChoice {
                id: ModelId::from_raw(2),
                name: "jaws.dxf".to_owned(),
                has_mesh: false,
                has_closed_outline: true,
            },
        ]
    }

    #[test]
    fn a_kind_switch_carries_the_z_range_and_the_usable_models_k2() {
        let models = models();
        let mut geometry = ObstacleGeometry::Box {
            min: [0.0, 0.0, -4.0],
            max: [10.0, 10.0, 12.0],
        };
        set_obstacle_kind(
            &mut geometry,
            ObstacleKind::OutlineExtrude,
            &models,
            (-20.0, 0.0),
        );
        assert_eq!(
            geometry,
            ObstacleGeometry::OutlineExtrude {
                model_ids: Vec::new(),
                z_bottom: -4.0,
                z_top: 12.0
            },
            "box to extrude keeps the box Z range"
        );
        set_obstacle_kind(&mut geometry, ObstacleKind::Model, &models, (-20.0, 0.0));
        assert_eq!(
            geometry,
            ObstacleGeometry::Model {
                model_id: ModelId::from_raw(1)
            },
            "extrude to model picks the first mesh model"
        );
        set_obstacle_kind(
            &mut geometry,
            ObstacleKind::OutlineExtrude,
            &models,
            (-20.0, 0.0),
        );
        assert_eq!(
            geometry,
            ObstacleGeometry::OutlineExtrude {
                model_ids: Vec::new(),
                z_bottom: -20.0,
                z_top: 0.0
            },
            "model to extrude keeps no model without a closed outline"
        );
        if let ObstacleGeometry::OutlineExtrude { model_ids, .. } = &mut geometry {
            model_ids.push(ModelId::from_raw(2));
        }
        set_obstacle_kind(&mut geometry, ObstacleKind::Box, &models, (-20.0, 0.0));
        assert_eq!(
            geometry,
            ObstacleGeometry::Box {
                min: [0.0, 0.0, -20.0],
                max: [
                    DEFAULT_OBSTACLE_BOX_MM[0],
                    DEFAULT_OBSTACLE_BOX_MM[1],
                    -20.0 + DEFAULT_OBSTACLE_BOX_MM[2]
                ]
            },
            "extrude to box takes the default spans at the old bottom Z"
        );
    }

    #[test]
    fn the_row_summary_names_the_kind_and_its_facts_k2() {
        let models = models();
        let mut obstacle = Obstacle::new_default_box(ObstacleId::from_raw(0));
        assert_eq!(row_summary(&obstacle, &models), "box · clearance 3.0 mm");
        obstacle.geometry = ObstacleGeometry::OutlineExtrude {
            model_ids: vec![ModelId::from_raw(2), ModelId::from_raw(3)],
            z_bottom: -20.0,
            z_top: 0.0,
        };
        assert_eq!(
            row_summary(&obstacle, &models),
            "extrude · 2 outlines · Z -20.0 to 0.0"
        );
        obstacle.geometry = ObstacleGeometry::Model {
            model_id: ModelId::from_raw(1),
        };
        assert_eq!(row_summary(&obstacle, &models), "model · clamp.stl");
        obstacle.geometry = ObstacleGeometry::Model {
            model_id: ModelId::from_raw(9),
        };
        assert_eq!(row_summary(&obstacle, &models), "model · missing model 9");
    }
}
