//! The MCP geometry selection rows (geometry selection P2.7).
//!
//! The wire tools are `edit_toolpath_selection` (the `EditSelection`
//! command row), `selection_status` and `list_model_items` (the
//! `SelectionStatus` and `ListItems` query rows), and `tab_layout` (the
//! `TabLayout` query row, P4.6). Each takes the core door
//! (`ProjectSession::apply` or `ProjectSession::query`); this file only
//! converts the wire type and writes the reply JSON.
//!
//! The `selection_status` reply is [`selection_report_json`] of the core
//! report. The GUI drill panel and the viewport read the same report (the
//! `selection_status` memo), so the three surfaces give one count.

use rs_cam_core::selection::{ItemCounts, MatchKey};
use rs_cam_core::session::{
    Command, EditSelectionArgs, Effects, ListItemsArgs, ModelItem, ProjectSession, Query,
    QueryAnswer, SelectionReport, SelectionStatusArgs, SelectionTarget, TabLayoutAnswer,
    TabLayoutArgs,
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
///
/// P4.4: `item_states` gives one `{key, state}` per item of the toolpath
/// model, in import order, with the key as `list_model_items` gives it
/// (source `Model(id)`); `foreign` gives the other selected items. P3.7:
/// each row also gives `floor_z`: the floor Z of a cut face under the floor
/// "each picked face" (setup frame), else `null`.
pub(crate) fn selection_report_json(
    session: &ProjectSession,
    tc: &rs_cam_core::session::ToolpathConfig,
    report: &SelectionReport,
) -> serde_json::Value {
    let item_states: Vec<serde_json::Value> = session
        .list_model_items(tc.model_id, None, None)
        .map(|items| {
            items
                .iter()
                .zip(&report.item_states)
                .enumerate()
                .map(|(index, (item, state))| {
                    serde_json::json!({
                        "key": key_json(&item.key),
                        "state": state,
                        "floor_z": report.item_floor_z.get(index).copied().flatten(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let foreign: Vec<serde_json::Value> = report
        .foreign
        .iter()
        .enumerate()
        .map(|(index, (key, state))| {
            serde_json::json!({
                "key": key_json(key),
                "state": state,
                "floor_z": report.foreign_floor_z.get(index).copied().flatten(),
            })
        })
        .collect();
    serde_json::json!({
        "item_states": item_states,
        "foreign": foreign,
        "counts": report.counts,
        // P3.4: the faces by facing in the setup frame. A down face
        // refuses in an op and gives its outline to a boundary.
        "faces_by_facing": report.faces_by_facing,
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
        // P4.7: the STL models whose flat faces are not computed yet
        // ("computing faces"); generation computes them itself.
        "faces_not_ready": report.faces_not_ready.iter().map(|m| m.raw()).collect::<Vec<_>>(),
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

/// The reply of the `list_model_items` read (P2.7; P4.5 `near`): the core
/// query row, each item as [`model_item_json`].
pub(crate) fn list_model_items_reply(
    session: &ProjectSession,
    p: &ListModelItemsParam,
) -> serde_json::Value {
    let query = Query::ListItems(ListItemsArgs {
        model_id: rs_cam_core::ids::ModelId::from_wire(p.model_id),
        layer: p.layer.clone(),
        kind: p.kind.map(Into::into),
        near: p.near.map(Into::into),
    });
    match session.query(query) {
        Ok(QueryAnswer::ListItems(answer)) => serde_json::json!({
            "model_id": p.model_id,
            "count": answer.items.len(),
            "items": answer.items.iter().map(model_item_json).collect::<Vec<_>>(),
        }),
        Ok(other) => serde_json::json!({
            "error": format!("the list_model_items read answered {other:?}"),
        }),
        Err(e) => serde_json::json!({ "error": e.to_string() }),
    }
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
    let Some(tc) = session.toolpath_config_at(index) else {
        return serde_json::json!({
            "error": format!("Toolpath index {index} not found"),
        });
    };
    match session.query(Query::SelectionStatus(SelectionStatusArgs {
        index,
        target,
    })) {
        Ok(QueryAnswer::SelectionStatus(report)) => {
            let mut reply = selection_report_json(session, tc, &report);
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

/// The reply of the `tab_layout` read: the core answer, mark by mark. A
/// key is its file JSON; `source` is `"auto"` or `{"hand": i}`.
pub(crate) fn tab_layout_json(answer: &TabLayoutAnswer) -> serde_json::Value {
    serde_json::json!({
        "marks": answer
            .marks
            .iter()
            .map(|m| serde_json::json!({
                "at": m.at,
                "on_loop": m.on_loop,
                "loop_key": key_json(&m.loop_key),
                "source": m.source,
            }))
            .collect::<Vec<_>>(),
        "loops": answer.loops.iter().map(key_json).collect::<Vec<_>>(),
    })
}

/// The `tab_layout` reply for the toolpath at `index`: the answer of the
/// core query row, with the toolpath's name and its `tab_points`.
pub(crate) fn tab_layout_reply(session: &ProjectSession, index: usize) -> serde_json::Value {
    let Some(tc) = session.toolpath_config_at(index) else {
        return serde_json::json!({
            "error": format!("Toolpath index {index} not found"),
        });
    };
    match session.query(Query::TabLayout(TabLayoutArgs { index })) {
        Ok(QueryAnswer::TabLayout(answer)) => {
            let mut reply = tab_layout_json(&answer);
            if let Some(obj) = reply.as_object_mut() {
                obj.insert("index".into(), serde_json::json!(index));
                obj.insert("id".into(), serde_json::json!(tc.id));
                obj.insert("name".into(), serde_json::json!(tc.name));
                if let rs_cam_core::compute::catalog::OperationConfig::Profile(cfg) = &tc.operation
                {
                    obj.insert("tab_points".into(), serde_json::json!(cfg.tab_points));
                }
            }
            reply
        }
        Ok(other) => serde_json::json!({
            "error": format!("the tab_layout read answered {other:?}"),
        }),
        Err(e) => serde_json::json!({ "error": e.to_string() }),
    }
}

impl RsCamApp {
    /// The `tab_layout` read: the core query row (P4.6).
    pub(crate) fn mcp_tab_layout(&self, index: usize) -> String {
        json_str(tab_layout_reply(self.controller.state().session(), index))
    }

    /// The `selection_status` read: the core query row.
    pub(crate) fn mcp_selection_status(&self, index: usize, target: SelectionTarget) -> String {
        json_str(selection_status_reply(
            self.controller.state().session(),
            index,
            target,
        ))
    }

    /// The `list_model_items` read: the core query row.
    pub(crate) fn mcp_list_model_items(&self, p: &ListModelItemsParam) -> String {
        json_str(list_model_items_reply(self.controller.state().session(), p))
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
            let session = self.controller.state().session();
            session
                .toolpath_config_at(index)
                .map_or(serde_json::Value::Null, |tc| {
                    let status = session
                        .selection_status(tc, target)
                        .map_or(serde_json::Value::Null, |r| {
                            selection_report_json(session, tc, &r)
                        });
                    serde_json::json!({
                        "target": target,
                        "selection": tc.selection_of(target).ok().flatten(),
                        "status": status,
                    })
                })
        };
        CoreReply::succeeded(self.mcp_mutation_result(
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
            id: rs_cam_core::ToolpathId::from_raw(0),
            name: op.kind_str().to_owned(),
            enabled: true,
            operation: OperationConfig::new_default(op),
            dressups: DressupConfig::default(),
            heights: HeightsConfig::default(),
            tool_id: rs_cam_core::ids::ToolId::from_raw(tool_id),
            model_id: rs_cam_core::ids::ModelId::from_raw(0),
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
        let model = rs_cam_core::io::load_model_file(
            &path,
            rs_cam_core::ids::ModelId::from_raw(0),
            ModelKind::Dxf,
            ModelUnits::Millimeters,
            Default::default(),
        )
        .expect("the drawing loads");
        let mut s = ProjectSession::new_empty();
        let _ = apply(
            &mut s,
            Command::AddModel(AddModelArgs {
                model: Box::new(model),
            }),
        );
        let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
        tool.diameter = 3.0;
        let tool_index = apply(
            &mut s,
            Command::AddTool(AddToolArgs {
                tool: Box::new(tool),
            }),
        )
        .created
        .expect("the tool is created");
        let tool_id = s.tools()[tool_index].id.raw();

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
            .list_model_items(
                rs_cam_core::ids::ModelId::from_raw(0),
                Some("HOLES"),
                Some(ItemKind::Circle),
            )
            .expect("the model exists");
        assert_eq!(circles.len(), 2, "the drawing holds two circles");
        // P6.5 (O-3): a Drill drills a circle of the tool size only, so the
        // drills get a tool of the picked circle size.
        let mut drill_tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
        drill_tool.diameter = circles[0].key.measure;
        let drill_tool_index = apply(
            &mut s,
            Command::AddTool(AddToolArgs {
                tool: Box::new(drill_tool),
            }),
        )
        .created
        .expect("the drill tool is created");
        let drill_tool_id = s.tools()[drill_tool_index].id.raw();
        let drill = add_op(&mut s, OperationType::Drill, drill_tool_id);
        edit(
            &mut s,
            drill,
            SelectionEdit::Replace(Box::new(Selection::from_picks(vec![
                circles[0].key.clone(),
            ]))),
        );

        let _trace = add_op(&mut s, OperationType::Trace, tool_id);
        let _pocket_default = add_op(&mut s, OperationType::Pocket, tool_id);

        let lost = add_op(&mut s, OperationType::Drill, drill_tool_id);
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
            let tc = s.toolpath_config_at(index).unwrap().clone();
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
            // P4.4: the item states. The wire gives one `{key, state}` per
            // item of the toolpath model, in import order, with the key of
            // `list_model_items`; the viewport colours the same memo.
            let wire_states: Vec<rs_cam_core::session::ItemState> = reply["item_states"]
                .as_array()
                .unwrap()
                .iter()
                .map(|row| serde_json::from_value(row["state"].clone()).unwrap())
                .collect();
            assert_eq!(wire_states, memo.item_states, "op {index} ({name})");
            let listed = s.list_model_items(tc.model_id, None, None).unwrap();
            for (row, item) in reply["item_states"].as_array().unwrap().iter().zip(&listed) {
                assert_eq!(row["key"], key_json(&item.key), "op {index}: the key order");
            }
            assert_eq!(
                reply["foreign"].as_array().unwrap().len(),
                memo.foreign.len(),
                "op {index}"
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
                faces_by_facing: serde_json::from_value(reply["faces_by_facing"].clone()).unwrap(),
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
        let states = |json: &serde_json::Value, state: &str| {
            json["item_states"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|row| row["state"] == state)
                .count()
        };
        assert_eq!(states(&pocket, "cut"), 2, "{pocket}");
        assert_eq!(states(&reply(4), "cut"), 0, "a lost pick cuts nothing");
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
            .list_model_items(
                rs_cam_core::ids::ModelId::from_raw(0),
                Some("HOLES"),
                Some(ItemKind::Circle),
            )
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

        let tc = s.toolpath_config_at(3).unwrap().clone();
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
        let items = s
            .list_model_items(rs_cam_core::ids::ModelId::from_raw(0), None, None)
            .unwrap();
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
    /// The stored selection of op `index`.
    fn stored(s: &ProjectSession, index: usize) -> Selection {
        s.toolpath_config_at(index)
            .unwrap()
            .operation
            .selection()
            .unwrap()
            .clone()
    }

    /// Apply one wire edit (`edit_toolpath_selection` JSON) to op `index`,
    /// as the MCP row does.
    fn wire_edit(s: &mut ProjectSession, index: usize, edit: &serde_json::Value) {
        let wire: EditToolpathSelectionParam =
            serde_json::from_value(serde_json::json!({ "index": index, "edit": edit }))
                .expect("the wire edit parses");
        let _ = apply(
            s,
            Command::EditSelection(EditSelectionArgs {
                index: wire.index,
                target: wire.target.into(),
                edit: wire.edit.into(),
            }),
        );
    }

    /// Geometry selection P4.5, MCP parity of a click: an MCP
    /// `list_model_items { near }` at a circle centre, then the edit its
    /// key gives, stores the same pick as the GUI click on that circle
    /// (`pick_edit`, the `TogglePick` path). Parity of a shift-click: the
    /// layer rule of `layer_rule_edit` (kinds and a circle range) is one
    /// wire `replace` with the same rules.
    #[test]
    fn an_mcp_near_pick_and_a_gui_click_give_one_pick_p4_5() {
        // Toolpath 1 is a Drill with one picked circle; pick the other.
        let circles = project()
            .list_model_items(
                rs_cam_core::ids::ModelId::from_raw(0),
                Some("HOLES"),
                Some(ItemKind::Circle),
            )
            .unwrap();
        let other = circles[1].key.clone();

        // GUI: the click on the drawn circle sends `TogglePick` of the item
        // key; the controller asks `pick_edit`.
        let mut gui = project();
        let tc = gui.toolpath_config_at(1).unwrap().clone();
        let item_key = gui.models()[0]
            .items
            .iter()
            .find(|i| i.key().names_same_geometry(&other))
            .unwrap()
            .key()
            .clone();
        let click = gui.pick_edit(&tc, SelectionTarget::Op, item_key).unwrap();
        edit(&mut gui, 1, click);

        // MCP: `near` at the circle centre lists that circle first; its
        // key goes into `add_pick`.
        let mut mcp = project();
        let p: ListModelItemsParam = serde_json::from_value(serde_json::json!({
            "model_id": 0,
            "near": { "xy": other.anchor, "radius_mm": 0.5 },
        }))
        .unwrap();
        let reply = list_model_items_reply(&mcp, &p);
        let first = &reply["items"][0];
        assert_eq!(first["kind"], serde_json::json!("circle"), "{reply}");
        wire_edit(
            &mut mcp,
            1,
            &serde_json::json!({ "add_pick": first["key"] }),
        );
        assert_eq!(stored(&mcp, 1), stored(&gui, 1), "one item is one pick");
        assert_eq!(stored(&gui, 1).picks().len(), 2);

        // Shift-click on a HOLES circle of the default Pocket (toolpath 3),
        // loops and circles on, Ø range on.
        let mut gui = project();
        let tc = gui.toolpath_config_at(3).unwrap().clone();
        let range = rs_cam_core::selection::DiameterRange {
            nominal_mm: circles[0].measure,
            tolerance_mm: 0.1,
        };
        let layer = gui
            .layer_rule_edit(
                &tc,
                SelectionTarget::Op,
                &circles[0].key,
                &[ItemKind::Loop, ItemKind::Circle],
                Some(range),
            )
            .unwrap();
        edit(&mut gui, 3, layer);
        let mut mcp = project();
        wire_edit(
            &mut mcp,
            3,
            &serde_json::json!({ "replace": { "rules": [
                { "source": "toolpath_model",
                  "filter": { "layers": ["HOLES"], "kinds": { "only": ["loop"] } } },
                { "source": "toolpath_model",
                  "filter": { "layers": ["HOLES"], "kinds": { "only": ["circle"] },
                              "diameter": { "nominal_mm": range.nominal_mm,
                                            "tolerance_mm": 0.1 } } },
            ] } }),
        );
        assert_eq!(
            stored(&mcp, 3),
            stored(&gui, 3),
            "one layer rule, two doors"
        );
        assert_eq!(stored(&gui, 3).rules().len(), 2);
    }

    /// Geometry selection P4.7: an STL face. The MCP door
    /// (`list_model_items { kind: face }`, then `add_pick` of its key) and
    /// the GUI door (the `build_model_faces` job, then the click rule
    /// `face_pick_key_at` and `pick_edit`) store one pick, and the
    /// `selection_status` reply counts the face. Before the faces exist the
    /// reply says `faces_not_ready` and gives no refusal.
    #[test]
    fn an_mcp_face_pick_and_a_gui_click_give_one_pick_on_an_stl_p4_7() {
        use rs_cam_core::geo::P3;
        use rs_cam_core::session::{BuildModelFacesArgs, Job, JobHandle};

        fn stl_project() -> ProjectSession {
            // The stepped mesh of the basis fork face-region test: the
            // Z 5 face round a boss whose top is at Z 8.
            let mut vertices = Vec::new();
            let mut triangles = Vec::new();
            let mut quad = |a: P3, b: P3, c: P3, d: P3| {
                let i = u32::try_from(vertices.len()).unwrap();
                vertices.extend([a, b, c, d]);
                triangles.push([i, i + 1, i + 2]);
                triangles.push([i, i + 2, i + 3]);
            };
            let p = P3::new;
            quad(p(0., 0., 5.), p(20., 0., 5.), p(20., 3., 5.), p(0., 3., 5.));
            quad(
                p(0., 7., 5.),
                p(20., 7., 5.),
                p(20., 10., 5.),
                p(0., 10., 5.),
            );
            quad(p(0., 3., 5.), p(8., 3., 5.), p(8., 7., 5.), p(0., 7., 5.));
            quad(
                p(12., 3., 5.),
                p(20., 3., 5.),
                p(20., 7., 5.),
                p(12., 7., 5.),
            );
            quad(p(8., 3., 8.), p(12., 3., 8.), p(12., 7., 8.), p(8., 7., 8.));
            let mesh = rs_cam_core::mesh::TriangleMesh::from_raw(vertices, triangles);
            let model = rs_cam_core::session::LoadedModel {
                id: rs_cam_core::ids::ModelId::from_raw(0),
                name: "stepped.stl".to_owned(),
                mesh: Some(std::sync::Arc::new(mesh)),
                items: Default::default(),
                path: std::path::PathBuf::from("synthetic://stepped.stl"),
                kind: Some(ModelKind::Stl),
                units: Some(ModelUnits::Millimeters),
                enriched_mesh: None,
                winding_report: None,
                load_error: None,
                import_options: Default::default(),
                import_report: None,
                drawing: Default::default(),
            };
            let mut s = ProjectSession::new_empty();
            let _ = apply(
                &mut s,
                Command::AddModel(AddModelArgs {
                    model: Box::new(model),
                }),
            );
            let tool_index = apply(
                &mut s,
                Command::AddTool(AddToolArgs {
                    tool: Box::new(ToolConfig::new_default(
                        ToolId::from_raw(0),
                        ToolType::EndMill,
                    )),
                }),
            )
            .created
            .unwrap();
            let tool_id = s.tools()[tool_index].id.raw();
            let _ = add_op(&mut s, OperationType::Pocket, tool_id);
            s
        }

        // MCP: the faces of the STL, then `add_pick` of the Z 5 face.
        let mut mcp = stl_project();
        let none: ListModelItemsParam =
            serde_json::from_value(serde_json::json!({ "model_id": 0 })).unwrap();
        assert_eq!(
            list_model_items_reply(&mcp, &none)["count"],
            0,
            "no kind: no face"
        );
        let faces: ListModelItemsParam =
            serde_json::from_value(serde_json::json!({ "model_id": 0, "kind": "face" })).unwrap();
        let reply = list_model_items_reply(&mcp, &faces);
        assert_eq!(reply["count"], 2, "{reply}");
        let z5 = reply["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| (row["measure"].as_f64().unwrap() - 184.0_f64.sqrt()).abs() < 1e-9)
            .expect("the Z 5 face is listed")
            .clone();
        wire_edit(&mut mcp, 0, &serde_json::json!({ "add_pick": z5["key"] }));

        // GUI: before the job, the status says computing faces.
        let mut gui = stl_project();
        let key = rs_cam_core::selection::MatchKey {
            source: SourceRef::ToolpathModel,
            ..rs_cam_core::selection::flat_region_items(gui.models()[0].mesh.as_ref().unwrap())
                .unwrap()
                .iter()
                .next()
                .unwrap()
                .key()
                .clone()
        };
        edit(&mut gui, 0, SelectionEdit::AddPick(key.clone()));
        let cold = selection_status_reply(&gui, 0, SelectionTarget::Op);
        assert_eq!(cold["faces_not_ready"], serde_json::json!([0]), "{cold}");
        assert_eq!(cold["refusal"], serde_json::Value::Null, "{cold}");
        edit(&mut gui, 0, SelectionEdit::RemovePick(key));

        // The job fills the cache; the click rule names the face.
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let Ok(JobHandle::BuildModelFaces(handle)) = gui.start(
            Job::BuildModelFaces(BuildModelFacesArgs {
                model_id: rs_cam_core::ids::ModelId::from_raw(0),
            }),
            &cancel,
        ) else {
            panic!("the build_model_faces row starts");
        };
        let built = rs_cam_core::session::execute_build_model_faces(&handle, &cancel).unwrap();
        assert_eq!(built.faces, 2);
        let tc = gui.toolpath_config_at(0).unwrap().clone();
        let click = gui
            .face_pick_key_at(
                &tc,
                rs_cam_core::ids::ModelId::from_raw(0),
                P3::new(2.0, 2.0, 5.0),
            )
            .expect("the click names the Z 5 face");
        let edit_of_click = gui.pick_edit(&tc, SelectionTarget::Op, click).unwrap();
        edit(&mut gui, 0, edit_of_click);

        assert_eq!(stored(&mcp, 0), stored(&gui, 0), "one face is one pick");
        assert_eq!(stored(&gui, 0).picks().len(), 1);
        let warm = selection_status_reply(&mcp, 0, SelectionTarget::Op);
        assert_eq!(count(&warm, "face"), 1, "{warm}");
        assert_eq!(warm["faces_not_ready"], serde_json::json!([]), "{warm}");
        assert_eq!(
            selection_status_reply(&gui, 0, SelectionTarget::Op)["counts"],
            warm["counts"]
        );
    }

    /// Geometry selection P4.D, MCP parity of a STEP face click: the MCP
    /// door (`list_model_items { kind: face }`, then `add_pick` of the
    /// listed key) and the GUI door (the click rule `face_pick_key` on the
    /// face group under the pointer, then `pick_edit`) store one pick on a
    /// Pocket of a STEP-only project, and the two `selection_status`
    /// replies count one face and no refusal.
    #[test]
    fn an_mcp_face_pick_and_a_gui_click_give_one_pick_on_a_step_p4_d() {
        fn step_project() -> ProjectSession {
            let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../fixtures/gui_step/stepped_block.step");
            let model = rs_cam_core::io::load_model_file(
                &path,
                rs_cam_core::ids::ModelId::from_raw(0),
                ModelKind::Step,
                ModelUnits::Millimeters,
                Default::default(),
            )
            .expect("the STEP fixture loads");
            let mut s = ProjectSession::new_empty();
            let _ = apply(
                &mut s,
                Command::AddModel(AddModelArgs {
                    model: Box::new(model),
                }),
            );
            let tool_index = apply(
                &mut s,
                Command::AddTool(AddToolArgs {
                    tool: Box::new(ToolConfig::new_default(
                        ToolId::from_raw(0),
                        ToolType::EndMill,
                    )),
                }),
            )
            .created
            .unwrap();
            let tool_id = s.tools()[tool_index].id.raw();
            let _ = add_op(&mut s, OperationType::Pocket, tool_id);
            s
        }

        // GUI: the click hits the highest flat face group; the click rule
        // names its item, and `pick_edit` gives the edit.
        let mut gui = step_project();
        let model_id = gui.models()[0].id;
        let enriched = std::sync::Arc::clone(
            gui.models()[0]
                .enriched_mesh
                .as_ref()
                .expect("a STEP model has BREP faces"),
        );
        let tc = gui.toolpath_config_at(0).unwrap().clone();
        let (face_id, click) = enriched
            .face_groups
            .iter()
            .filter(|g| (g.bbox.max.z - g.bbox.min.z).abs() < 1e-9)
            .max_by(|a, b| a.bbox.max.z.total_cmp(&b.bbox.max.z))
            .map(|g| {
                let key = gui
                    .face_pick_key(&tc, model_id, g.id)
                    .expect("a flat face has an item");
                (g.id, key)
            })
            .expect("fixture: the block has a flat face");
        let edit_of_click = gui.pick_edit(&tc, SelectionTarget::Op, click).unwrap();
        edit(&mut gui, 0, edit_of_click);

        // MCP: the faces of the model, then `add_pick` of the listed row
        // of that face.
        let mut mcp = step_project();
        let faces: ListModelItemsParam =
            serde_json::from_value(serde_json::json!({ "model_id": 0, "kind": "face" })).unwrap();
        let reply = list_model_items_reply(&mcp, &faces);
        let row = reply["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["key"]["brep"] == serde_json::json!(face_id.0))
            .unwrap_or_else(|| panic!("the clicked face is listed: {reply}"))
            .clone();
        wire_edit(&mut mcp, 0, &serde_json::json!({ "add_pick": row["key"] }));

        assert_eq!(stored(&mcp, 0), stored(&gui, 0), "one face is one pick");
        assert_eq!(stored(&gui, 0).picks().len(), 1);
        let status = selection_status_reply(&mcp, 0, SelectionTarget::Op);
        assert_eq!(count(&status, "face"), 1, "{status}");
        assert_eq!(status["refusal"], serde_json::Value::Null, "{status}");
        assert_eq!(
            selection_status_reply(&gui, 0, SelectionTarget::Op),
            status,
            "the two doors give one status"
        );
    }

    /// Geometry selection P4.6: the MCP `tab_layout` reply and the marks
    /// the GUI tab editor shows (`state::tab_edit::refresh_layout`, the
    /// layout the viewport draws and the controller edits) are one answer,
    /// mark by mark: `at`, `on_loop`, `loop_key` and `source`. Two arms: a
    /// Profile with auto tabs on a Top setup, and with one hand tab on a
    /// Bottom setup turned 90 degrees.
    #[test]
    fn mcp_and_gui_give_one_tab_layout_p4_6() {
        use rs_cam_core::compute::transform::{FaceUp, ZRotation};
        use rs_cam_core::session::{SetSetupFaceArgs, SetSetupRotationArgs, SetToolpathParamArgs};
        for flipped in [false, true] {
            let mut s = project();
            let tool_id = s.tools()[0].id.raw();
            let profile = add_op(&mut s, OperationType::Profile, tool_id);
            let _ = apply(
                &mut s,
                Command::SetToolpathParam(SetToolpathParamArgs {
                    index: profile,
                    param: "tab_count".to_owned(),
                    value: serde_json::json!(2),
                }),
            );
            if flipped {
                let _ = apply(
                    &mut s,
                    Command::SetSetupFace(SetSetupFaceArgs {
                        setup_index: 0,
                        face_up: FaceUp::Bottom,
                    }),
                );
                let _ = apply(
                    &mut s,
                    Command::SetSetupRotation(SetSetupRotationArgs {
                        setup_index: 0,
                        z_rotation: ZRotation::Deg90,
                    }),
                );
                // One hand tab: the outline point of the first auto mark.
                let first = match s.query(Query::TabLayout(TabLayoutArgs { index: profile })) {
                    Ok(QueryAnswer::TabLayout(answer)) => answer.marks[0].on_loop,
                    other => panic!("tab_layout answered {other:?}"),
                };
                let _ = apply(
                    &mut s,
                    Command::SetToolpathParam(SetToolpathParamArgs {
                        index: profile,
                        param: "tab_points".to_owned(),
                        value: serde_json::json!([first]),
                    }),
                );
            }
            let mcp = tab_layout_reply(&s, profile);
            assert!(mcp.get("error").is_none(), "{mcp}");

            let id = s.toolpath_configs()[profile].id;
            let mut state = crate::state::AppState::new();
            state.replace_session(s);
            state.workspace = crate::state::Workspace::Toolpaths;
            state.selection = crate::state::selection::Selection::Toolpath(id);
            state.gui.selection_ui.set_tab_edit(id, true);
            assert!(crate::state::tab_edit::refresh_layout(&mut state));
            let gui = state
                .gui
                .selection_ui
                .tab_edit(id)
                .and_then(|mode| mode.answer())
                .expect("the GUI editor has a layout");
            assert!(!gui.marks.is_empty(), "the Profile has tabs");
            if flipped {
                assert!(
                    gui.marks
                        .iter()
                        .any(|m| m.source == rs_cam_core::dressup::tabs::TabSource::Hand(0))
                );
            }
            let marks = mcp["marks"].as_array().expect("marks");
            assert_eq!(marks.len(), gui.marks.len(), "{mcp}");
            for (wire, mark) in marks.iter().zip(&gui.marks) {
                assert_eq!(wire["at"], serde_json::json!(mark.at));
                assert_eq!(wire["on_loop"], serde_json::json!(mark.on_loop));
                assert_eq!(wire["loop_key"], key_json(&mark.loop_key));
                assert_eq!(wire["source"], serde_json::json!(mark.source));
            }
            let loops = mcp["loops"].as_array().expect("loops");
            assert_eq!(loops.len(), gui.loops.len());
        }
    }

    /// Geometry selection P3.8 (design section 9, the `selection_status`
    /// row): under the floor "each picked face", the MCP `floor_z` of each
    /// `item_states` row equals the report that the GUI reads (the
    /// Selection section and the floor row), and each cut face gives its
    /// own plane Z less the depth.
    ///
    /// The part is synthetic STEP: three up faces at Z 0, Z -4 and Z -8.
    /// The Pocket picks the faces at Z 0 and Z -4 under
    /// `EachFace { depth: 1 }`; the stock top is Z 5, so neither face is at
    /// the top. The face at Z -8 is not selected and gives `null`.
    ///
    /// Red proof: `selection_report_json` with `"floor_z": null` on the
    /// item rows. The two cut rows then give `null`, and the test fails.
    #[test]
    fn mcp_floor_z_of_each_cut_face_equals_the_gui_report_p3_8() {
        use rs_cam_core::compute::floor::Floor;
        use rs_cam_core::compute::operation_configs::PocketConfig;
        use rs_cam_core::compute::stock_config::StockConfig;
        use rs_cam_core::geo::{P3, V3};
        use rs_cam_core::geometry::enriched_mesh::{
            FaceTessellation, SurfaceParams, SurfaceType, build_enriched_mesh,
        };
        use rs_cam_core::selection::Shape;
        use rs_cam_core::session::{ItemState, LoadedModel, ProjectSessionBuilder};
        use std::sync::Arc;

        const DEPTH: f64 = 1.0;
        let flat = |x0: f64, z: f64| {
            let v = vec![
                P3::new(x0, -10.0, z),
                P3::new(x0 + 20.0, -10.0, z),
                P3::new(x0 + 20.0, 10.0, z),
                P3::new(x0, 10.0, z),
            ];
            FaceTessellation {
                boundary_loops: vec![v.clone()],
                vertices: v,
                triangles: vec![[0, 1, 2], [0, 2, 3]],
                surface_type: SurfaceType::Plane,
                surface_params: SurfaceParams::Plane {
                    normal: V3::new(0.0, 0.0, 1.0),
                    d: z,
                },
            }
        };
        let enriched = build_enriched_mesh(
            vec![flat(-40.0, 0.0), flat(-15.0, -4.0), flat(10.0, -8.0)],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let model = LoadedModel {
            id: rs_cam_core::ids::ModelId::from_raw(0),
            path: std::path::PathBuf::from("synthetic://each_face.step"),
            name: "Each face".to_owned(),
            kind: None,
            mesh: Some(Arc::clone(&enriched.mesh)),
            items: Arc::new(rs_cam_core::io::brep_face_items(&enriched)),
            enriched_mesh: Some(Arc::new(enriched)),
            units: None,
            winding_report: None,
            load_error: None,
            import_options: Default::default(),
            import_report: None,
            drawing: Default::default(),
        };
        let plane_z = |key: &MatchKey| {
            model
                .items
                .iter()
                .find(|item| item.key().names_same_geometry(key))
                .and_then(|item| match item.shape() {
                    Shape::Face(face) => Some(face.plane_z),
                    _ => None,
                })
                .expect("a face of the part")
        };
        let picks: Vec<MatchKey> = model
            .items
            .iter()
            .filter(|item| match item.shape() {
                Shape::Face(face) => face.plane_z > -6.0,
                _ => false,
            })
            .map(|item| MatchKey {
                source: SourceRef::ToolpathModel,
                ..item.key().clone()
            })
            .collect();
        assert_eq!(picks.len(), 2, "the faces at Z 0 and Z -4");

        let mut tool = ToolConfig::new_default(ToolId::from_raw(1), ToolType::EndMill);
        tool.diameter = 3.0;
        let config = ToolpathConfig {
            shoe: Default::default(),
            id: rs_cam_core::ToolpathId::from_raw(0),
            name: "Each face pocket".to_owned(),
            enabled: true,
            operation: OperationConfig::Pocket(PocketConfig {
                floor: Floor::EachFace { depth: DEPTH },
                selection: Selection::empty(),
                ..PocketConfig::default()
            }),
            dressups: DressupConfig::default(),
            heights: HeightsConfig::default(),
            tool_id: rs_cam_core::ids::ToolId::from_raw(1),
            model_id: rs_cam_core::ids::ModelId::from_raw(0),
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
        let mut builder = ProjectSessionBuilder::new()
            .stock(StockConfig {
                x: 80.0,
                y: 40.0,
                z: 20.0,
                origin_x: -40.0,
                origin_y: -20.0,
                origin_z: -15.0,
                auto_from_model: false,
                ..StockConfig::default()
            })
            .tool(tool)
            .model(model.clone());
        builder.add_toolpath(0, config).unwrap();
        let mut s = builder.build();
        for key in &picks {
            edit(&mut s, 0, SelectionEdit::AddPick(key.clone()));
        }

        // MCP: the `selection_status` reply.
        let reply = selection_status_reply(&s, 0, SelectionTarget::Op);
        assert!(reply["refusal"].is_null(), "{reply}");
        let rows = reply["item_states"].as_array().expect("item_states");
        assert_eq!(rows.len(), 3, "{reply}");

        // GUI: the report of the Selection section and the floor row.
        let tc = s.toolpath_config_at(0).unwrap().clone();
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
            .expect("the Pocket holds a selection")
            .expect("the selection resolves");
        assert_eq!(section.item_floor_z.len(), rows.len());
        let mut cut = Vec::new();
        for (index, row) in rows.iter().enumerate() {
            assert_eq!(
                row["floor_z"],
                serde_json::json!(section.item_floor_z[index]),
                "row {index}: MCP floor_z is the GUI report"
            );
            let state: ItemState = serde_json::from_value(row["state"].clone()).unwrap();
            if state == ItemState::Cut {
                let key: MatchKey = serde_json::from_value(row["key"].clone()).unwrap();
                let z = row["floor_z"].as_f64().expect("a cut face has a floor Z");
                assert!(
                    (z - (plane_z(&key) - DEPTH)).abs() < 1e-9,
                    "row {index}: the floor is the plane Z less the depth, got {z}"
                );
                cut.push(z);
            } else {
                assert!(row["floor_z"].is_null(), "row {index}: {row}");
            }
        }
        cut.sort_by(|a, b| b.total_cmp(a));
        assert_eq!(cut.len(), 2, "two cut faces: {reply}");
        assert!((cut[0] - cut[1]).abs() > 1.0, "two floors: {cut:?}");

        // The floor row reads the same floors (`FloorInputs`, top first).
        let answer = s.toolpath_heights(&tc).unwrap();
        let floor_inputs =
            crate::ui::properties::FloorInputs::new(&answer, &tc.heights, Some(&section));
        let row_floors: Vec<f64> = floor_inputs.face_floors.iter().map(|f| f.z).collect();
        assert_eq!(row_floors, cut, "the floor row reads the MCP floors");
    }
}
