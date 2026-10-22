//! The Geometry tab of the toolpath inspector (geometry selection GT-1):
//! what the op cuts. It is the first tab (operator ruling 2026-10-22).
//!
//! From top to bottom (design revision 2, R2.5):
//!
//! 1. The op selection block (`selection_section.rs`): the summary, Pick
//!    and Reset, the filter box, the selected list and the notes. An op
//!    with no op selection (every 3D op) gets the "Pick faces" row of its
//!    boundary selection instead.
//! 2. On an op that holds a floor: the Floor line and, under the floor
//!    "A picked face" (UX-8), the floor face slot with "Clear" (L-R5).
//! 3. "Boundary" (operator ruling GT-Q7): the boundary selection block,
//!    with the same rows, while the boundary source is a selection.
//!
//! The tab writes through events only (L-1): `EditToolpathSelection`,
//! `UseAsFloor`, `ClearFloorFace`. It never writes the draft entry.

use rs_cam_core::compute::config::BoundarySource;
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::session::{BOUNDARY_ITEM_USE, SelectionTarget};

use super::ToolpathPanelInputs;
use super::selection_section::{SelectionSectionInputs, draw_selection_section, line_with_actions};
use crate::ui::AppEvent;
use crate::ui::components::SectionHeader;
use crate::ui::tokens;

/// The label of the 3D op face-pick button (geometry selection P4.D).
pub(super) const PICK_FACES_LABEL: &str = "Pick faces";

/// The hover of [`PICK_FACES_LABEL`].
const PICK_FACES_HOVER: &str = "Click flat faces in the viewport to bound this operation. \
    When the boundary source is not Selection, the first face sets the source to Selection \
    with that face. Undo restores the source.";

/// The name of the floor face slot (L-R5).
pub(crate) const FLOOR_FACE: &str = "Floor face";

/// The Geometry tab. `properties::tests` renders it.
pub(super) fn draw_geometry_tab(
    ui: &mut egui::Ui,
    entry: &crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
    selection_ui: &mut crate::state::pick::SelectionUi,
    events: &mut Vec<AppEvent>,
) {
    ui.add_space(4.0);
    // GT-2: a row sets the hover again on each frame it is under the
    // pointer.
    selection_ui.row_hover = None;
    let tool_diameter = inputs
        .tool_configs
        .iter()
        .find(|(id, _)| *id == entry.tool_id)
        .map(|(_, tool)| tool.diameter);
    let floor = entry.operation.floor();

    // ── 1. The op selection ────────────────────────────────────────
    if let Some(selection) = entry.operation.selection() {
        let op_default = entry
            .operation
            .default_selection()
            .unwrap_or_else(rs_cam_core::selection::Selection::empty);
        draw_selection_section(
            ui,
            &SelectionSectionInputs {
                toolpath_id: entry.id,
                toolpath_model: entry.model_id,
                target: SelectionTarget::Op,
                selection,
                op_default: &op_default,
                report: inputs.selection_report.as_ref(),
                item_use: entry.operation.capabilities().item_use,
                models: &inputs.models,
                offers: offers_of(inputs, selection_ui, entry.id, SelectionTarget::Op),
                op_label: entry.operation.op_type().label(),
                floor_op: floor.is_some(),
                note_items: &inputs.selection_note_items,
                model_kinds: &inputs.toolpath_model_kinds,
                tool_diameter,
            },
            selection_ui,
            events,
        );
    } else {
        draw_boundary_pick_row(ui, entry, selection_ui);
    }

    // ── 2. The floor and the floor face slot ───────────────────────
    if let Some(floor) = floor.as_ref() {
        draw_floor_lines(ui, entry, inputs, floor, events);
    }

    // ── 3. The boundary selection (GT-Q7) ──────────────────────────
    if entry.boundary.enabled
        && let BoundarySource::Selection { selection, .. } = &entry.boundary.source
    {
        ui.add_space(6.0);
        SectionHeader::new("Boundary")
            .hover(
                "The items that bound where the tool may go. The source, the containment \
                 and the offset are in the Cut tab.",
            )
            .show(ui);
        draw_selection_section(
            ui,
            &SelectionSectionInputs {
                toolpath_id: entry.id,
                toolpath_model: entry.model_id,
                target: SelectionTarget::Boundary,
                selection,
                op_default: &inputs.boundary_default,
                report: inputs.boundary_report.as_ref(),
                item_use: BOUNDARY_ITEM_USE,
                models: &inputs.models,
                offers: offers_of(inputs, selection_ui, entry.id, SelectionTarget::Boundary),
                op_label: entry.operation.op_type().label(),
                floor_op: false,
                note_items: &inputs.boundary_note_items,
                model_kinds: &inputs.toolpath_model_kinds,
                tool_diameter: None,
            },
            selection_ui,
            events,
        );
    }
}

/// The offers of the open "Rule…" menu when it belongs to `target`.
fn offers_of<'a>(
    inputs: &'a ToolpathPanelInputs,
    selection_ui: &crate::state::pick::SelectionUi,
    id: crate::state::toolpath::ToolpathId,
    target: SelectionTarget,
) -> &'a [rs_cam_core::session::RuleOffer] {
    if selection_ui.offers_for(id, target).is_some() {
        &inputs.rule_offers
    } else {
        &[]
    }
}

/// The Floor line of a floor op, and under "A picked face" the floor face
/// slot with "Clear" (L-R5). The Heights tab owns the other references.
fn draw_floor_lines(
    ui: &mut egui::Ui,
    entry: &crate::state::toolpath::ToolpathEntry,
    inputs: &ToolpathPanelInputs,
    floor: &Floor,
    events: &mut Vec<AppEvent>,
) {
    let answer = inputs
        .toolpath_heights
        .as_ref()
        .and_then(|answer| answer.as_ref().ok());
    let floor_z = answer.and_then(|answer| answer.floor_z(&entry.heights));
    // FR-2 F25: under "Each picked face" each face cuts to its own floor,
    // so the line gives the range of the face floors (the floors that the
    // heights diagram draws, `FloorInputs::face_floors`).
    // R-B: a Drill or a Bore on holes of a STEP model cuts each hole to its
    // own floor, so the line gives the range of the hole floors too.
    let report = inputs
        .selection_report
        .as_ref()
        .and_then(|read| read.as_ref().ok())
        .map(AsRef::as_ref);
    let per_item = matches!(floor, Floor::EachFace { .. } | Floor::EachHole { .. })
        || report.is_some_and(|r| r.hole_rim_bottom_z.is_some());
    let face_floors = match answer {
        Some(answer) if per_item => {
            super::FloorInputs::new(answer, &entry.heights, report).face_floors
        }
        _ => Vec::new(),
    };
    let z_text = match (face_floors.first(), face_floors.last(), &floor_z) {
        // The floors are top first.
        (Some(top), Some(bottom), _) if top.z.to_bits() != bottom.z.to_bits() => {
            format!("Z {:.2} to {:.2}", bottom.z, top.z)
        }
        (Some(only), Some(_), _) => format!("Z {:.2}", only.z),
        (_, _, Some(Ok(z))) => format!("Z {z:.2}"),
        (_, _, Some(Err(refusal))) => format!("no Z: {refusal}"),
        (_, _, None) => "Z \u{2014}".to_owned(),
    };
    let color = match floor_z {
        Some(Err(_)) => tokens::DANGER,
        Some(Ok(_)) | None => tokens::TEXT_BODY,
    };
    let text = format!("{} \u{00B7} {z_text}", floor.reference_label());
    line_with_actions(
        ui,
        "Floor",
        &text,
        color,
        &format!(
            "Floor {text}. The floor of the op. Set the reference and the depth in the Heights \
             tab; \"Use as floor\" on a picked face sets the floor to that face."
        ),
        &[],
    );
    if let Some(key) = floor.face_key() {
        // FR-2 F23: the slot names the face as the list rows do.
        let face_z = answer.and_then(|answer| answer.floor_references(&entry.heights).floor_face_z);
        let label = face_z.map_or_else(
            || super::selection_section::row_label(key, None),
            super::selection_section::face_label,
        );
        let full = super::selection_section::row_full_label(key, face_z);
        if line_with_actions(
            ui,
            FLOOR_FACE,
            &label,
            tokens::TEXT_BODY,
            &format!("{full}. The face that sets the floor. The op does not cut this face."),
            &[(
                "Clear",
                "Set the floor to Each picked face when faces remain, else to the op default.",
            )],
        )
        .is_some()
        {
            events.push(AppEvent::ClearFloorFace {
                toolpath_id: entry.id,
            });
        }
    }
}

/// The "Pick faces" row of an op that holds no selection (every 3D op):
/// the button turns Pick on for the boundary selection, for ANY boundary
/// source (geometry selection P4.D, ruling L-8 revised). The first face
/// click then sets the source to `Selection` with that face
/// (`ProjectSession::face_click_edit`). An op that no boundary moves (D3)
/// draws no row.
fn draw_boundary_pick_row(
    ui: &mut egui::Ui,
    entry: &crate::state::toolpath::ToolpathEntry,
    selection_ui: &mut crate::state::pick::SelectionUi,
) {
    if !entry
        .operation
        .capabilities()
        .territory_xy
        .session_clip_applies()
    {
        return;
    }
    let on = selection_ui.pick_is_on(entry.id, SelectionTarget::Boundary);
    let button = if on {
        crate::ui::components::Button::primary(PICK_FACES_LABEL)
    } else {
        crate::ui::components::Button::new(PICK_FACES_LABEL)
    };
    if ui.add(button).on_hover_text(PICK_FACES_HOVER).clicked() {
        selection_ui.set_pick(if on {
            None
        } else {
            Some(crate::state::pick::PickMode::new(
                entry.id,
                SelectionTarget::Boundary,
                BOUNDARY_ITEM_USE,
            ))
        });
    }
}
