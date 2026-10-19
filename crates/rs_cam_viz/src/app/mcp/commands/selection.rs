//! The MCP geometry selection rows (geometry selection P2.7).
//!
//! The wire tools are `edit_toolpath_selection` (the `EditSelection`
//! command row), `selection_status` and `list_model_items` (the
//! `SelectionStatus` and `ListItems` query rows). Each takes the core door
//! (`ProjectSession::apply` or `ProjectSession::query`); this file only
//! converts the wire type and writes the reply JSON.
//!
//! The `selection_status` reply is [`selection_report_json`] of the core
//! report. The GUI drill panel and the viewport read the same report (the
//! `selection_status` memo), so the three surfaces give one count.

use rs_cam_core::selection::{ItemCounts, MatchKey};
use rs_cam_core::session::{
    Command, EditSelectionArgs, Effects, ListItemsArgs, ModelItem, ProjectSession, Query,
    QueryAnswer, SelectionReport, SelectionStatusArgs, SelectionTarget,
};
use rs_cam_mcp::server::{EditToolpathSelectionParam, ListModelItemsParam, json_str};

use crate::app::RsCamApp;

use super::{CoreBefore, CorePlan, CoreReply};

/// The file JSON of a match key: the value an agent copies into
/// `{"add_pick": <key>}`.
fn key_json(key: &MatchKey) -> serde_json::Value {
    serde_json::to_value(key).unwrap_or(serde_json::Value::Null)
}

/// The reply of the `selection_status` read: the core report, field by
/// field. A key is its file JSON; a note, a lost pick and a refusal also
/// carry the core text.
pub(crate) fn selection_report_json(report: &SelectionReport) -> serde_json::Value {
    serde_json::json!({
        "counts": report.counts,
        "islands": report.islands,
        "skipped_by_kind": report.skipped_by_kind,
        "notes": report
            .notes
            .iter()
            .map(|note| serde_json::json!({
                "kind": note.kind,
                "layer": note.layer,
                "count": note.count,
                "first": key_json(&note.first.key),
                "text": note.to_string(),
            }))
            .collect::<Vec<_>>(),
        "lost": report
            .lost
            .iter()
            .map(|lost| serde_json::json!({
                "key": key_json(&lost.key),
                "text": lost.to_string(),
            }))
            .collect::<Vec<_>>(),
        "refusal": report.refusal.as_ref().map(ToString::to_string),
        "holes": report
            .holes
            .iter()
            .map(|hole| serde_json::json!({
                "xy": [hole.xy.x, hole.xy.y],
                "diameter": hole.diameter,
                "layer": hole.layer,
            }))
            .collect::<Vec<_>>(),
        "selected": report.selected.iter().map(key_json).collect::<Vec<_>>(),
    })
}

/// One row of the `list_model_items` read.
pub(crate) fn model_item_json(item: &ModelItem) -> serde_json::Value {
    serde_json::json!({
        "kind": item.kind,
        "layer": item.layer,
        "measure": item.measure,
        "key": key_json(&item.key),
    })
}

/// The item counts of a model (`ItemSet::counts`): the `items` block of
/// `inspect_model`.
pub(crate) fn item_counts_json(counts: &ItemCounts) -> serde_json::Value {
    serde_json::json!({
        "total": counts.by_kind.values().sum::<usize>(),
        "summary": ItemCounts::line(&counts.by_kind),
        "by_kind": counts.by_kind,
        "by_layer": counts.by_layer,
    })
}

/// The `selection_status` reply for the selection `target` of the
/// toolpath at `index`: the answer of the core query row, with the
/// toolpath's name, op, the target and its stored selection.
pub(crate) fn selection_status_reply(
    session: &ProjectSession,
    index: usize,
    target: SelectionTarget,
) -> serde_json::Value {
    let Some(tc) = session.get_toolpath_config(index) else {
        return serde_json::json!({
            "error": format!("Toolpath index {index} not found"),
        });
    };
    match session.query(Query::SelectionStatus(SelectionStatusArgs {
        index,
        target,
    })) {
        Ok(QueryAnswer::SelectionStatus(report)) => {
            let mut reply = selection_report_json(&report);
            if let Some(obj) = reply.as_object_mut() {
                obj.insert("index".into(), serde_json::json!(index));
                obj.insert("target".into(), serde_json::json!(target));
                obj.insert("id".into(), serde_json::json!(tc.id));
                obj.insert("name".into(), serde_json::json!(tc.name));
                obj.insert(
                    "operation".into(),
                    serde_json::json!(tc.operation.op_type().kind_str()),
                );
                obj.insert(
                    "selection".into(),
                    tc.selection_of(target)
                        .ok()
                        .and_then(|selection| serde_json::to_value(selection).ok())
                        .unwrap_or(serde_json::Value::Null),
                );
            }
            reply
        }
        Ok(other) => serde_json::json!({
            "error": format!("the selection_status read answered {other:?}"),
        }),
        Err(e) => serde_json::json!({ "error": e.to_string() }),
    }
}

impl RsCamApp {
    /// The `selection_status` read: the core query row.
    pub(crate) fn mcp_selection_status(&self, index: usize, target: SelectionTarget) -> String {
        json_str(selection_status_reply(
            &self.controller.state().session,
            index,
            target,
        ))
    }

    /// The `list_model_items` read: the core query row.
    pub(crate) fn mcp_list_model_items(&self, p: &ListModelItemsParam) -> String {
        let query = Query::ListItems(ListItemsArgs {
            model_id: p.model_id,
            layer: p.layer.clone(),
            kind: p.kind.map(Into::into),
        });
        match self.controller.state().session.query(query) {
            Ok(QueryAnswer::ListItems(answer)) => json_str(serde_json::json!({
                "model_id": p.model_id,
                "count": answer.items.len(),
                "items": answer.items.iter().map(model_item_json).collect::<Vec<_>>(),
            })),
            Ok(other) => json_str(serde_json::json!({
                "error": format!("the list_model_items read answered {other:?}"),
            })),
            Err(e) => json_str(serde_json::json!({ "error": e.to_string() })),
        }
    }

    /// `edit_toolpath_selection` → `Command::EditSelection`.
    pub(super) fn plan_edit_selection(
        &self,
        p: EditToolpathSelectionParam,
        mut before: CoreBefore,
    ) -> CorePlan {
        let target: SelectionTarget = p.target.into();
        before.index = Some(p.index);
        before.field = Some("edit".to_owned());
        before.extra = serde_json::json!({ "target": target });
        CorePlan::Apply(
            Command::EditSelection(EditSelectionArgs {
                index: p.index,
                target,
                edit: p.edit.into(),
            }),
            Box::new(before),
        )
    }

    /// The reply of an applied selection edit: the stored selection and
    /// its `selection_status` report.
    pub(super) fn describe_edit_selection(
        &mut self,
        effects: &Effects,
        before: &CoreBefore,
    ) -> CoreReply {
        self.controller.state_mut().gui.mark_edited();
        let stale = self.core_stale(effects);
        let index = before.index();
        // `plan_edit_selection` writes the target into `extra`.
        let target: SelectionTarget = before
            .extra
            .get("target")
            .and_then(|t| serde_json::from_value(t.clone()).ok())
            .unwrap_or_default();
        let applied = {
            let session = &self.controller.state().session;
            session
                .get_toolpath_config(index)
                .map_or(serde_json::Value::Null, |tc| {
                    let status = session
                        .selection_status(tc, target)
                        .map_or(serde_json::Value::Null, |r| selection_report_json(&r));
                    serde_json::json!({
                        "target": target,
                        "selection": tc.selection_of(target).ok().flatten(),
                        "status": status,
                    })
                })
        };
        CoreReply::quiet(self.mcp_mutation_result(
            format!("Selection of toolpath {index} edited. Regenerate to apply."),
            applied,
            stale,
            &before.diagnostics,
        ))
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    //! GUI/MCP/core parity of `selection_status` (geometry selection P2.7).
    //!
    //! One project: the P2.7 drawing (two squares on layers `CUT` and
    //! `KEEP`, two circles on `HOLES`, an open line on `TEXT`) and five
    //! ops. For each op the MCP reply, the core query row and the memo the
    //! GUI reads (`ProjectSession::selection_status`, and its drill-panel
    //! projection `hole_selection`) give the same counts, skipped kinds,
    //! notes, lost picks and refusal.

    use super::*;
    use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
    use rs_cam_core::compute::config::{
        BoundaryConfig, DressupConfig, HeightsConfig, RestAnalysisConfig, StockSource,
    };
    use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use rs_cam_core::compute::{ModelKind, ModelUnits};
    use rs_cam_core::selection::{Filter, ItemKind, Rule, Selection, SelectionEdit, SourceRef};
    use rs_cam_core::session::{AddModelArgs, AddToolArgs, AddToolpathArgs, ToolpathConfig};

    fn apply(s: &mut ProjectSession, command: Command) -> Effects {
        s.apply(command).expect("the command applies")
    }

    fn add_op(s: &mut ProjectSession, op: OperationType, tool_id: usize) -> usize {
        let config = ToolpathConfig {
            shoe: Default::default(),
            id: rs_cam_core::ToolpathId(0),
            name: op.kind_str().to_owned(),
            enabled: true,
            operation: OperationConfig::new_default(op),
            dressups: DressupConfig::default(),
            heights: HeightsConfig::default(),
            tool_id,
            model_id: 0,
            pre_gcode: None,
            post_gcode: None,
            boundary: BoundaryConfig::default(),
            boundary_inherit: true,
            rest_analysis: RestAnalysisConfig::default(),
            stock_source: StockSource::default(),
            coolant: Default::default(),
            debug_options: Default::default(),
            feeds_provenance: Default::default(),
            planner_origin: None,
        };
        apply(
            s,
            Command::AddToolpath(AddToolpathArgs {
                setup_index: 0,
                config: Box::new(config),
            }),
        )
        .created
        .expect("the toolpath is created")
    }

    fn edit(s: &mut ProjectSession, index: usize, edit: SelectionEdit) {
        let _ = apply(
            s,
            Command::EditSelection(EditSelectionArgs {
                index,
                target: SelectionTarget::Op,
                edit,
            }),
        );
    }

    /// The five ops: 0 Pocket with a rule on `CUT`; 1 Drill with one
    /// pick (a `list_model_items` key); 2 Trace, default; 3 Pocket, default
    /// (skips the open line); 4 Drill with a lost pick.
    fn project() -> ProjectSession {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../rs_cam_core/tests/fixtures/selection_p2_7/layers.dxf");
        let model =
            rs_cam_core::io::load_model_file(&path, 0, ModelKind::Dxf, ModelUnits::Millimeters)
                .expect("the drawing loads");
        let mut s = ProjectSession::new_empty();
        let _ = apply(
            &mut s,
            Command::AddModel(AddModelArgs {
                model: Box::new(model),
            }),
        );
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.diameter = 3.0;
        let tool_index = apply(
            &mut s,
            Command::AddTool(AddToolArgs {
                tool: Box::new(tool),
            }),
        )
        .created
        .expect("the tool is created");
        let tool_id = s.tools()[tool_index].id.0;

        let pocket = add_op(&mut s, OperationType::Pocket, tool_id);
        edit(
            &mut s,
            pocket,
            SelectionEdit::Replace(Box::new(Selection::from_rules(vec![Rule {
                source: SourceRef::ToolpathModel,
                filter: Filter {
                    layers: vec!["CUT".into()],
                    ..Filter::default()
                },
            }]))),
        );

        let circles = s
            .list_model_items(0, Some("HOLES"), Some(ItemKind::Circle))
            .expect("the model exists");
        assert_eq!(circles.len(), 2, "the drawing holds two circles");
        let drill = add_op(&mut s, OperationType::Drill, tool_id);
        edit(
            &mut s,
            drill,
            SelectionEdit::Replace(Box::new(Selection::from_picks(vec![
                circles[0].key.clone(),
            ]))),
        );

        let _trace = add_op(&mut s, OperationType::Trace, tool_id);
        let _pocket_default = add_op(&mut s, OperationType::Pocket, tool_id);

        let lost = add_op(&mut s, OperationType::Drill, tool_id);
        let mut gone = circles[0].key.clone();
        gone.anchor = [500.0, 500.0];
        edit(
            &mut s,
            lost,
            SelectionEdit::Replace(Box::new(Selection::from_picks(vec![gone]))),
        );
        s
    }

    fn count(json: &serde_json::Value, kind: &str) -> u64 {
        json["counts"][kind].as_u64().unwrap_or(0)
    }

    #[test]
    fn mcp_core_and_gui_give_one_selection_status_p2_7() {
        let mut s = project();
        // P4.3: a rule edit first, so the GUI section reads an edited
        // selection: the `CUT` Pocket also takes the `KEEP` layer.
        edit(
            &mut s,
            0,
            SelectionEdit::AddRule(Rule {
                source: SourceRef::ToolpathModel,
                filter: Filter {
                    layers: vec!["KEEP".into()],
                    ..Filter::default()
                },
            }),
        );
        assert_eq!(s.toolpath_count(), 5);
        for index in 0..5 {
            let tc = s.get_toolpath_config(index).unwrap().clone();
            let memo = s
                .selection_status(&tc, SelectionTarget::Op)
                .expect("the selection resolves");
            let QueryAnswer::SelectionStatus(query) = s
                .query(Query::SelectionStatus(SelectionStatusArgs {
                    index,
                    target: SelectionTarget::Op,
                }))
                .unwrap()
            else {
                panic!("the selection_status row answered another kind");
            };
            assert_eq!(*query, *memo, "op {index}: the query row is the memo");

            let reply = selection_status_reply(&s, index, SelectionTarget::Op);
            let name = &tc.name;
            assert_eq!(
                reply["counts"],
                serde_json::to_value(&memo.counts).unwrap(),
                "op {index} ({name})"
            );
            assert_eq!(reply["islands"], serde_json::json!(memo.islands));
            assert_eq!(
                reply["skipped_by_kind"],
                serde_json::to_value(&memo.skipped_by_kind).unwrap()
            );
            assert_eq!(reply["notes"].as_array().unwrap().len(), memo.notes.len());
            assert_eq!(reply["lost"].as_array().unwrap().len(), memo.lost.len());
            assert_eq!(
                reply["refusal"],
                serde_json::json!(memo.refusal.as_ref().map(ToString::to_string))
            );
            assert_eq!(reply["holes"].as_array().unwrap().len(), memo.holes.len());
            assert_eq!(
                reply["selected"].as_array().unwrap().len(),
                memo.selected.len()
            );
            // P4.3: the Selection section reads the same answer through
            // the panel inputs, and prints the same counts.
            let inputs = crate::ui::properties::toolpath_panel_inputs(
                tc.id,
                &s,
                &crate::state::runtime::GuiState::default(),
                None,
                None,
                false,
            );
            let section = inputs
                .selection_report
                .expect("every op here holds a selection")
                .expect("the selection resolves");
            assert_eq!(*section, *memo, "op {index}: the section reads the memo");
            let wire_counts: std::collections::BTreeMap<ItemKind, usize> =
                serde_json::from_value(reply["counts"].clone()).unwrap();
            let from_wire = SelectionReport {
                counts: wire_counts,
                islands: usize::try_from(reply["islands"].as_u64().unwrap()).unwrap(),
                ..SelectionReport::default()
            };
            assert_eq!(
                crate::ui::properties::selection_counts_text(&section),
                crate::ui::properties::selection_counts_text(&from_wire),
                "op {index} ({name}): the section prints the counts MCP gives"
            );
            // The GUI drill panel and its viewport markers read this
            // projection of the memo.
            if let Some(panel) = s.hole_selection(&tc) {
                assert_eq!(panel.holes, memo.holes, "op {index}");
                assert_eq!(panel.lost_picks, memo.lost.len(), "op {index}");
                assert_eq!(
                    serde_json::json!(panel.refusal),
                    reply["refusal"],
                    "op {index}"
                );
            }
        }

        // The numbers themselves, so a reply that agrees on an empty
        // report cannot pass.
        let reply = |i| selection_status_reply(&s, i, SelectionTarget::Op);
        let pocket = reply(0);
        assert_eq!(
            (count(&pocket, "loop"), count(&pocket, "circle")),
            (2, 0),
            "the CUT rule and the added KEEP rule give two loops"
        );
        assert!(pocket["refusal"].is_null(), "{pocket}");
        let drill = reply(1);
        assert_eq!(count(&drill, "circle"), 1);
        assert_eq!(drill["holes"].as_array().unwrap().len(), 1);
        assert_eq!(
            drill["selection"]["picks"][0]["source"],
            serde_json::json!("toolpath_model"),
            "a list_model_items key on the own model is stored as toolpath_model"
        );
        let trace = reply(2);
        assert_eq!(
            (
                count(&trace, "loop"),
                count(&trace, "circle"),
                count(&trace, "path")
            ),
            (2, 2, 1),
            "{trace}"
        );
        let pocket_default = reply(3);
        assert_eq!(
            pocket_default["skipped_by_kind"]["path"],
            serde_json::json!(1)
        );
        assert_eq!(pocket_default["notes"].as_array().unwrap().len(), 1);
        assert!(
            pocket_default["notes"][0]["text"]
                .as_str()
                .unwrap()
                .contains("TEXT"),
            "{pocket_default}"
        );
        let lost = reply(4);
        assert_eq!(lost["lost"].as_array().unwrap().len(), 1);
        assert!(lost["refusal"].is_string(), "{lost}");
    }

    /// Geometry selection P4.1: the `target` wire field. An absent target
    /// reads `"op"`; `"boundary"` reaches the boundary selection. The MCP
    /// reply, the core query row and the memo the GUI reads (the face
    /// highlight, `pick_edit`) give one boundary report, and a boundary
    /// that is not a selection gives the error that names the fix.
    #[test]
    fn mcp_core_and_gui_give_one_boundary_status_p4_1() {
        use rs_cam_core::compute::config::{BoundaryContainment, BoundarySource};
        use rs_cam_core::session::SetBoundaryConfigArgs;
        use rs_cam_mcp::server::{
            EditToolpathSelectionParam, SelectionStatusParam, SelectionTargetParam,
        };

        // The wire: the default and the two spellings, as the file spells
        // `SelectionTarget`.
        let absent: SelectionStatusParam =
            serde_json::from_value(serde_json::json!({ "index": 3 })).unwrap();
        assert_eq!(absent.target, SelectionTargetParam::Op);
        for target in [SelectionTarget::Op, SelectionTarget::Boundary] {
            let file = serde_json::to_value(target).unwrap();
            let wire: SelectionTargetParam = serde_json::from_value(file.clone()).unwrap();
            assert_eq!(SelectionTarget::from(wire), target, "{file}");
        }

        let mut s = project();
        let circles = s
            .list_model_items(0, Some("HOLES"), Some(ItemKind::Circle))
            .unwrap();
        // Toolpath 3 (a default Pocket) gets a selection boundary with no
        // item, then one circle through the wire edit.
        let _ = apply(
            &mut s,
            Command::SetBoundaryConfig(SetBoundaryConfigArgs {
                index: 3,
                boundary: BoundaryConfig {
                    enabled: true,
                    source: BoundarySource::Selection {
                        selection: Selection::empty(),
                        holes: false,
                    },
                    containment: BoundaryContainment::Center,
                    offset: 0.0,
                },
            }),
        );
        let wire: EditToolpathSelectionParam = serde_json::from_value(serde_json::json!({
            "index": 3,
            "target": "boundary",
            "edit": { "add_pick": key_json(&circles[0].key) },
        }))
        .unwrap();
        let _ = apply(
            &mut s,
            Command::EditSelection(EditSelectionArgs {
                index: wire.index,
                target: wire.target.into(),
                edit: wire.edit.into(),
            }),
        );

        let tc = s.get_toolpath_config(3).unwrap().clone();
        let memo = s.selection_status(&tc, SelectionTarget::Boundary).unwrap();
        let QueryAnswer::SelectionStatus(query) = s
            .query(Query::SelectionStatus(SelectionStatusArgs {
                index: 3,
                target: SelectionTarget::Boundary,
            }))
            .unwrap()
        else {
            panic!("the selection_status row answered another kind");
        };
        assert_eq!(*query, *memo, "the query row is the memo");
        let reply = selection_status_reply(&s, 3, SelectionTarget::Boundary);
        assert_eq!(reply["target"], serde_json::json!("boundary"));
        assert_eq!(count(&reply, "circle"), 1, "{reply}");
        assert_eq!(reply["counts"], serde_json::to_value(&memo.counts).unwrap());
        assert!(reply["refusal"].is_null(), "{reply}");
        assert_eq!(
            reply["selection"]["picks"][0]["source"],
            serde_json::json!("toolpath_model"),
            "a wire key on the own model is stored as toolpath_model"
        );
        // The op selection of the same toolpath is untouched: the default.
        let op = selection_status_reply(&s, 3, SelectionTarget::Op);
        assert_eq!(op["target"], serde_json::json!("op"));
        assert_eq!(
            op["selection"],
            serde_json::to_value(Selection::drawing()).unwrap()
        );

        // A boundary that is not a selection: the reply names the fix.
        let stock = selection_status_reply(&s, 0, SelectionTarget::Boundary);
        assert!(
            stock["error"]
                .as_str()
                .is_some_and(|e| e.contains("set the boundary source to Selection")),
            "{stock}"
        );
    }

    #[test]
    fn the_item_rows_carry_keys_an_agent_can_pick_with() {
        let s = project();
        let items = s.list_model_items(0, None, None).unwrap();
        let rows: Vec<serde_json::Value> = items.iter().map(model_item_json).collect();
        // The key of each row parses as the wire key and names its model.
        for row in &rows {
            let wire: rs_cam_mcp::server::MatchKeyParam =
                serde_json::from_value(row["key"].clone()).expect("a row key is a wire key");
            assert_eq!(wire.source, rs_cam_mcp::server::SourceRefParam::Model(0));
        }
        // inspect_model counts the same items.
        let counts = item_counts_json(&s.models()[0].items.counts());
        assert_eq!(counts["total"], serde_json::json!(rows.len()));
        assert_eq!(counts["by_kind"]["loop"], serde_json::json!(2));
        assert_eq!(counts["by_kind"]["circle"], serde_json::json!(2));
        assert_eq!(counts["by_kind"]["path"], serde_json::json!(1));
        assert_eq!(counts["by_layer"]["HOLES"]["circle"], serde_json::json!(2));
    }
}
