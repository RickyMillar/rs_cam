//! The MCP door of the drawing edit ledger (geometry selection P4.9):
//! `edit_model_drawing` (the `EditModelDrawing` row) and the ledger facts
//! that its reply and `inspect_model` give.
//!
//! The GUI model panel reads the same core lists (`open_shapes`, the
//! ledger and its outcomes) and sends the same row. The test below pins
//! that the two surfaces give the same items.

use rs_cam_core::selection::open_shapes;
use rs_cam_core::session::{Command, EditModelDrawingArgs, LoadedModel};
use rs_cam_mcp::server::EditModelDrawingParam;

use super::{CoreBefore, CorePlan};

/// The ledger facts of one model: its open paths (core `open_shapes`)
/// and each stored edit with its outcome. A lost edit gives `"lost"`, the
/// reason as a sentence (the text of the model panel) and the cause. The
/// `edit_model_drawing` reply and `inspect_model` write these keys from
/// this one function.
pub(crate) fn drawing_ledger_json(model: &LoadedModel) -> serde_json::Value {
    let open: Vec<serde_json::Value> = open_shapes(&model.items)
        .into_iter()
        .map(|s| {
            serde_json::json!({
                "index": s.index,
                "key": s.key,
                "layer": s.layer,
                "length_mm": s.length_mm,
                "gap_mm": s.gap_mm,
                "can_close_with_line": s.can_close_with_line,
                "can_move_ends_together": s.can_move_ends_together,
            })
        })
        .collect();
    let outcomes = model.drawing.outcomes();
    let edits: Vec<serde_json::Value> = model
        .drawing
        .edits()
        .iter()
        .enumerate()
        .map(|(index, edit)| {
            let outcome = outcomes.iter().find(|o| o.edit == index);
            let mut row = serde_json::json!({
                "index": index,
                "edit": edit,
                // The stored edit is in file units (the project-file
                // JSON, the `replace` wire form); the label is in model mm.
                "label": edit.clone().scaled(model.drawing_scale()).label(),
            });
            if let Some(map) = row.as_object_mut() {
                match outcome.map(|o| &o.result) {
                    Some(Ok(effect)) => {
                        map.insert("outcome".into(), serde_json::json!("applied"));
                        map.insert("effect".into(), serde_json::json!(effect));
                    }
                    Some(Err(lost)) => {
                        map.insert("outcome".into(), serde_json::json!("lost"));
                        map.insert("reason".into(), serde_json::json!(lost.label()));
                        map.insert("cause".into(), serde_json::json!(lost));
                    }
                    // A model that did not load holds its ledger unreplayed.
                    None => {
                        map.insert("outcome".into(), serde_json::json!("not_replayed"));
                    }
                }
            }
            row
        })
        .collect();
    serde_json::json!({
        "model_id": model.id,
        "open_shapes": open,
        "drawing_edits": edits,
        "lost_edits": model.drawing.lost_count(),
    })
}

/// `edit_model_drawing` → `Command::EditModelDrawing`.
pub(super) fn plan_edit_model_drawing(
    p: EditModelDrawingParam,
    mut before: CoreBefore,
) -> CorePlan {
    before.index = Some(p.model_id);
    before.field = Some("edit".to_owned());
    CorePlan::Apply(
        Command::EditModelDrawing(EditModelDrawingArgs {
            model_id: rs_cam_core::ids::ModelId::from_wire(p.model_id),
            edit: p.edit.into(),
        }),
        Box::new(before),
    )
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    //! GUI/MCP parity of the edit ledger (geometry selection P4.9).
    //!
    //! The GUI arm is the controller door (`edit_model_drawing`, the model
    //! panel and the popover send it). The MCP arm is the wire JSON parsed
    //! as `EditModelDrawingParam` and applied as the row. Both must give
    //! the same ledger, the same items and the same ledger facts.

    use std::path::Path;

    use super::*;
    use crate::app::mcp::tests::U4SilentBackend;
    use crate::controller::AppController;

    use rs_cam_core::compute::stock_config::ModelKind;
    use rs_cam_core::session::{AddModelArgs, LedgerEdit, ProjectSession};
    use rs_cam_mcp::server::ImportModelParam;

    fn mix_dxf() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../rs_cam_core/tests/fixtures/selection_p2_1/mix.dxf")
    }

    fn keys(model: &LoadedModel) -> Vec<String> {
        model
            .items
            .iter()
            .map(|i| format!("{:?}", i.key()))
            .collect()
    }

    #[test]
    fn edit_model_drawing_matches_the_model_panel_p4_9() {
        let path = mix_dxf();
        // GUI: the dialog import, then two acts of the model panel.
        let mut controller = AppController::with_backend(U4SilentBackend);
        controller.open_import_dialog(&path, ModelKind::Dxf);
        controller.confirm_import_dialog().unwrap();
        let id = controller.state().session().models()[0].id;
        let first_open = open_shapes(&controller.state().session().models()[0].items)
            .into_iter()
            .find(|s| s.can_close_with_line)
            .expect("mix.dxf has an open path of 3 or more points");
        let gui_acts = [
            LedgerEdit::Add(rs_cam_core::selection::DrawingEdit::CloseWithLine {
                path: first_open.key,
            }),
            LedgerEdit::Add(rs_cam_core::selection::DrawingEdit::JoinWithin {
                layer: None,
                tolerance_mm: 5.0,
            }),
        ];
        for act in gui_acts {
            controller.edit_drawing_ledger(id, act).unwrap();
        }
        let gui = controller.state().session().models()[0].clone();
        assert_eq!(gui.drawing.edits().len(), 2);

        // MCP: the same import, the same two acts as wire JSON. The path
        // key is the `inspect_model` `open_shapes` key of the MCP model.
        let mut session = ProjectSession::new_empty();
        let (model, _) = super::super::import::import_request_model(&ImportModelParam {
            path: path.display().to_string(),
            units: None,
            join_tolerance_mm: None,
        })
        .unwrap();
        let _ = session
            .apply(Command::AddModel(AddModelArgs {
                model: Box::new(model),
            }))
            .unwrap();
        let door = keys(&session.models()[0]);
        let facts = drawing_ledger_json(&session.models()[0]);
        let key = facts["open_shapes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["can_close_with_line"] == true)
            .unwrap()["key"]
            .clone();
        for edit in [
            serde_json::json!({"add": {"close_with_line": {"path": key}}}),
            serde_json::json!({"add": {"join_within": {"tolerance_mm": 5.0}}}),
        ] {
            let p: EditModelDrawingParam =
                serde_json::from_value(serde_json::json!({"model_id": id, "edit": edit})).unwrap();
            let _ = session
                .apply(Command::EditModelDrawing(EditModelDrawingArgs {
                    model_id: rs_cam_core::ids::ModelId::from_raw(p.model_id),
                    edit: p.edit.into(),
                }))
                .unwrap();
        }
        let mcp = session.models()[0].clone();
        assert_eq!(
            mcp.drawing.edits(),
            gui.drawing.edits(),
            "one stored ledger"
        );
        assert_eq!(keys(&mcp), keys(&gui), "the same items");
        assert_eq!(drawing_ledger_json(&mcp), drawing_ledger_json(&gui));
        assert_ne!(keys(&mcp), door, "non-vacuity: the edits change the items");

        // A refusal: a remove past the end changes nothing on either side.
        assert!(
            controller
                .edit_drawing_ledger(id, LedgerEdit::Remove(9))
                .is_err()
        );
        assert!(
            session
                .apply(Command::EditModelDrawing(EditModelDrawingArgs {
                    model_id: id,
                    edit: LedgerEdit::Remove(9),
                }))
                .is_err()
        );
    }
}
