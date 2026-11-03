//! The MCP floor row (geometry selection P3.3).
//!
//! The wire tool `set_toolpath_floor` is the `SetToolpathFloor` command row.
//! The GUI writes the floor through the whole config
//! (`replace_toolpath_config`, L-17); both routes take
//! `ProjectSession::apply`, so they give one config. This file converts the
//! wire type and writes the heights JSON that `get_toolpath_params` and the
//! `set_toolpath_floor` reply give. The heights come from the one heights
//! rule (`ProjectSession::toolpath_heights`, P3.1).

use rs_cam_core::session::{ProjectSession, SetToolpathFloorArgs, ToolpathConfig};
use rs_cam_mcp::server::SetToolpathFloorParam;

/// The core payload of one `set_toolpath_floor` request.
pub(super) fn floor_args(p: &SetToolpathFloorParam) -> SetToolpathFloorArgs {
    SetToolpathFloorArgs {
        index: p.index,
        floor: p.floor.clone().into(),
    }
}

/// The heights of `tc` in its setup frame, as the MCP replies give them.
///
/// `top_from` says where the Top Z comes from. For Pocket, Profile, Zigzag
/// and Adaptive 2D, `floor` is the file JSON of the floor, `floor_label`
/// names its reference, and `floor_z` is the resolved floor Z (the
/// `bottom_z` of these ops). A floor that does not resolve gives
/// `floor_z: null` and `floor_refusal` (the text generation refuses with).
/// Every other op gives `null` in the four floor keys.
pub(crate) fn toolpath_heights_json(
    session: &ProjectSession,
    tc: &ToolpathConfig,
) -> serde_json::Value {
    let answer = match session.toolpath_heights(tc) {
        Ok(answer) => answer,
        Err(e) => return serde_json::json!({ "error": e.to_string() }),
    };
    let heights = &answer.heights;
    let floor = tc.operation.floor();
    let floor_z = answer.floor_z(&tc.heights);
    serde_json::json!({
        "clearance_z": heights.clearance_z,
        "retract_z": heights.retract_z,
        "feed_z": heights.feed_z,
        "top_z": heights.top_z,
        "bottom_z": heights.bottom_z,
        "top_from": answer.top_from.label(),
        "floor": floor,
        "floor_label": floor.as_ref().map(|f| f.reference_label()),
        "floor_z": floor_z.and_then(Result::ok),
        "floor_refusal": floor_z.and_then(Result::err).map(|r| r.to_string()),
    })
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    //! GUI/MCP parity of the floor (geometry selection P3.3, L-17).

    use super::*;
    use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
    use rs_cam_core::compute::config::{
        BoundaryConfig, DressupConfig, HeightsConfig, RestAnalysisConfig, StockSource,
    };
    use rs_cam_core::compute::floor::Floor;
    use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use rs_cam_core::compute::{ModelKind, ModelUnits};
    use rs_cam_core::session::{AddModelArgs, AddToolArgs, AddToolpathArgs, Command, Effects};

    /// The absolute floor Z both routes write (mm, setup frame).
    const FLOOR_Z: f64 = -2.5;

    fn apply(s: &mut ProjectSession, command: Command) -> Effects {
        s.apply(command).expect("the command applies")
    }

    /// The P2.7 drawing, one end mill and one default Pocket (index 0).
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
        let config = ToolpathConfig {
            shoe: Default::default(),
            feeds_lock: Default::default(),
            id: rs_cam_core::ToolpathId::from_raw(0),
            name: "Pocket".to_owned(),
            enabled: true,
            operation: OperationConfig::new_default(OperationType::Pocket),
            dressups: DressupConfig::default(),
            heights: HeightsConfig::default(),
            tool_id: Some(rs_cam_core::ids::ToolId::from_raw(tool_id)),
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
        let _ = apply(
            &mut s,
            Command::AddToolpath(AddToolpathArgs {
                setup_index: 0,
                config: Box::new(config),
            }),
        );
        s
    }

    /// One `set_toolpath_floor` request, as the wire sends it, through the
    /// conversion the MCP arm uses.
    fn wire_floor(s: &mut ProjectSession, floor: &serde_json::Value) -> Result<Effects, String> {
        let wire: SetToolpathFloorParam =
            serde_json::from_value(serde_json::json!({ "index": 0, "floor": floor }))
                .expect("the wire request parses");
        s.apply(Command::SetToolpathFloor(floor_args(&wire)))
            .map_err(|e| e.to_string())
    }

    /// An MCP `set_toolpath_floor` and a GUI draft write
    /// (`write_entry_config_to_session`, the inspector's route) give one
    /// config and one `toolpath_heights`. The floor Z is the absolute Z the
    /// request names, and the MCP write stales the toolpath. A refused
    /// wire floor writes nothing.
    #[test]
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
    fn mcp_and_gui_give_one_floor_p3_3() {
        let floor = Floor::Z { z: FLOOR_Z };
        let file_json = serde_json::to_value(&floor).unwrap();

        // MCP: the wire request through the core door.
        let mut mcp = project();
        let before = mcp.toolpath_config_at(0).unwrap().operation.floor();
        assert_ne!(
            before,
            Some(floor.clone()),
            "the default is not the test floor"
        );
        let effects = wire_floor(&mut mcp, &file_json).expect("the wire floor applies");
        assert!(effects.stale.contains(&0), "the floor stales the toolpath");

        // GUI: the inspector edits the draft and writes the whole config.
        let mut state = crate::state::AppState::new();
        state.replace_session(project());
        let id = state.session().toolpath_configs()[0].id;
        let mut entry = crate::ui::properties::build_entry_from_session_and_gui(
            id,
            state.session(),
            &state.gui,
        )
        .expect("the toolpath has an entry");
        match &mut entry.operation {
            OperationConfig::Pocket(c) => c.floor = floor.clone(),
            other => panic!("the fixture op is a Pocket, not {}", other.label()),
        }
        let _ = crate::ui::properties::write_entry_config_to_session(&entry, &mut state)
            .expect("the GUI write applies");

        // One config.
        let mcp_tc = mcp.toolpath_config_at(0).unwrap();
        let gui_tc = state.session().toolpath_config_at(0).unwrap();
        assert_eq!(
            serde_json::to_value(&mcp_tc.operation).unwrap(),
            serde_json::to_value(&gui_tc.operation).unwrap(),
            "the two routes store one operation config"
        );
        assert_eq!(
            serde_json::to_value(&mcp_tc.heights).unwrap(),
            serde_json::to_value(&gui_tc.heights).unwrap(),
            "the two routes store one heights config"
        );
        assert_eq!(mcp_tc.operation.floor(), Some(floor.clone()));

        // One `toolpath_heights`.
        let mcp_heights = toolpath_heights_json(&mcp, mcp_tc);
        let gui_heights = toolpath_heights_json(state.session(), gui_tc);
        assert_eq!(mcp_heights, gui_heights, "one toolpath_heights");
        assert_eq!(mcp_heights["floor"], file_json, "{mcp_heights}");
        assert_eq!(mcp_heights["floor_z"], serde_json::json!(FLOOR_Z));
        assert_eq!(mcp_heights["bottom_z"], serde_json::json!(FLOOR_Z));
        assert!(mcp_heights["floor_refusal"].is_null(), "{mcp_heights}");
        let answer = mcp.toolpath_heights(mcp_tc).unwrap();
        assert_eq!(answer.floor_z(&mcp_tc.heights), Some(Ok(FLOOR_Z)));

        // A refused wire floor writes nothing.
        for refused in [
            serde_json::json!({ "from": "top", "depth": -1.0 }),
            serde_json::json!({ "from": "top", "depth": 0.0 }),
        ] {
            let error = wire_floor(&mut mcp, &refused).expect_err("the door refuses");
            assert!(
                error.contains("Pocket"),
                "the refusal names the toolpath: {error}"
            );
            assert_eq!(
                mcp.toolpath_config_at(0).unwrap().operation.floor(),
                Some(floor.clone()),
                "{refused} wrote the config"
            );
        }
    }

    /// The STEP stepped block and one Pocket that picks its two lowest up
    /// faces. Returns the session and the keys of the two faces (the
    /// lower first), as a click names them (source `Model(id)`).
    fn step_pocket() -> (ProjectSession, [rs_cam_core::selection::MatchKey; 2]) {
        use rs_cam_core::selection::{SelectionEdit, Shape};
        use rs_cam_core::session::{EditSelectionArgs, SelectionTarget};
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/gui_step/stepped_block.step");
        let model = rs_cam_core::io::load_model_file(
            &path,
            rs_cam_core::ids::ModelId::from_raw(0),
            ModelKind::Step,
            ModelUnits::Millimeters,
            Default::default(),
        )
        .expect("the STEP fixture loads");
        let mut s = project();
        let id = apply(
            &mut s,
            Command::AddModel(AddModelArgs {
                model: Box::new(model),
            }),
        );
        let _ = id;
        let model = s.models().last().expect("the STEP model").clone();
        let mut faces: Vec<(f64, rs_cam_core::selection::MatchKey)> = model
            .items
            .iter()
            .filter_map(|item| match item.shape() {
                Shape::Face(face) if face.normal.z > 0.99 => {
                    Some((face.plane_z, item.key().clone()))
                }
                Shape::Loop(_)
                | Shape::Path(_)
                | Shape::Point(_)
                | Shape::Circle { .. }
                | Shape::Face(_) => None,
            })
            .collect();
        faces.sort_by(|a, b| a.0.total_cmp(&b.0));
        assert!(faces.len() >= 2, "fixture: two up faces");
        let lower = faces[0].1.clone();
        let upper = faces[1].1.clone();
        let mut tc = s.toolpath_config_at(0).unwrap().clone();
        tc.model_id = model.id;
        let _ = apply(
            &mut s,
            Command::ReplaceToolpathConfig(rs_cam_core::session::ReplaceToolpathConfigArgs {
                index: 0,
                config: Box::new(tc),
            }),
        );
        for key in [&lower, &upper] {
            let _ = apply(
                &mut s,
                Command::EditSelection(EditSelectionArgs {
                    index: 0,
                    target: SelectionTarget::Op,
                    edit: SelectionEdit::AddPick(key.clone()),
                }),
            );
        }
        (s, [lower, upper])
    }

    /// Geometry selection GT-4 (L-R5): "Use as floor" in the Geometry tab
    /// (`AppEvent::UseAsFloor`, the controller door) and MCP
    /// `set_toolpath_floor` with the face form store ONE floor and ONE op
    /// config: the floor `Face { key, depth: 0 }` in the toolpath form,
    /// and the face out of the picks. "Clear" then gives "Each picked face"
    /// while a face pick remains, else the op default.
    ///
    /// Red proof: drop the pick removal of the `SetToolpathFloor` door
    /// (`heights.rs`): the face stays in the picks on both arms, and the
    /// pick assertion fails.
    #[test]
    fn use_as_floor_matches_set_toolpath_floor_gt4() {
        use crate::app::mcp::tests::U4SilentBackend;
        use crate::controller::AppController;
        use crate::ui::AppEvent;

        let (session, [lower, upper]) = step_pocket();
        // MCP: the wire face form, the key as `list_model_items` gives it.
        let mut mcp = session;
        let model_id = mcp.toolpath_config_at(0).unwrap().model_id;
        let wire_key = serde_json::json!({
            "source": { "model": model_id.raw() },
            "kind": "face",
            "anchor": lower.anchor,
            "measure": lower.measure,
            "brep": lower.brep,
        });
        let _ = wire_floor(
            &mut mcp,
            &serde_json::json!({ "from": "face", "depth": 0.0, "key": wire_key }),
        )
        .expect("the wire face floor applies");

        // GUI: the controller door of "Use as floor".
        let (gui_session, _) = step_pocket();
        let mut controller = AppController::with_backend(U4SilentBackend);
        controller.state_mut().replace_session(gui_session);
        let id = controller.state().session().toolpath_configs()[0].id;
        controller.handle_internal_event(AppEvent::UseAsFloor {
            toolpath_id: id,
            key: lower.clone(),
        });

        let mcp_tc = mcp.toolpath_config_at(0).unwrap();
        let gui_tc = controller.state().session().toolpath_config_at(0).unwrap();
        assert_eq!(
            serde_json::to_value(&mcp_tc.operation).unwrap(),
            serde_json::to_value(&gui_tc.operation).unwrap(),
            "the two routes store one operation config"
        );
        let Some(Floor::Face { key, depth }) = gui_tc.operation.floor() else {
            panic!(
                "the floor is the picked face: {:?}",
                gui_tc.operation.floor()
            );
        };
        assert_eq!(depth, 0.0);
        assert_eq!(key.source, rs_cam_core::selection::SourceRef::ToolpathModel);
        let picks = gui_tc.operation.selection().unwrap().picks().to_vec();
        assert!(
            !picks.iter().any(|p| p.names_same_geometry(&lower)),
            "the floor face leaves the picks: {picks:?}"
        );
        assert!(
            picks.iter().any(|p| p.names_same_geometry(&upper)),
            "the other face stays: {picks:?}"
        );
        let heights = toolpath_heights_json(controller.state().session(), gui_tc);
        assert!(heights["floor_refusal"].is_null(), "{heights}");
        assert_eq!(heights["floor_label"], "A picked face");

        // Clear: a face pick remains, so Each picked face, depth 0.
        controller.handle_internal_event(AppEvent::ClearFloorFace { toolpath_id: id });
        assert_eq!(
            controller
                .state()
                .session()
                .toolpath_config_at(0)
                .unwrap()
                .operation
                .floor(),
            Some(Floor::EachFace { depth: 0.0 })
        );
    }

    /// Geometry selection P3.8 (design section 9): the two refusal rows
    /// and the schema row of the floor name the floor door. P6.8 adds the
    /// Bore row and the Bore and `open_paths` schema rows.
    ///
    /// - `set_toolpath_param depth` on a Pocket refuses and names
    ///   `set_toolpath_floor` (the four ops hold no `depth`).
    /// - `get_operation_schema` gives `floor` on the four ops, written
    ///   through `set_toolpath_floor`, with each reference; an op with no
    ///   floor gives none.
    ///
    /// Red proof: the `depth` arm of `unknown_param_error` removed (the
    /// generic "unknown parameter" text, with no floor door): the first
    /// assertion fails.
    #[test]
    fn the_floor_wire_rows_name_the_floor_door_p3_8() {
        use rs_cam_core::session::SetToolpathParamArgs;
        let mut s = project();
        let before = s.toolpath_config_at(0).unwrap().operation.floor();
        let error = s
            .apply(Command::SetToolpathParam(SetToolpathParamArgs {
                index: 0,
                param: "depth".to_owned(),
                value: serde_json::json!(6.0),
            }))
            .expect_err("a Pocket holds no depth");
        let text = error.to_string();
        assert!(
            text.contains("set_toolpath_floor"),
            "the refusal names the floor door: {text}"
        );
        assert_eq!(
            s.toolpath_config_at(0).unwrap().operation.floor(),
            before,
            "the refused write keeps the floor"
        );

        // P6.8: the Bore (P6.6) is an `OpFloor` op too.
        for op in [
            OperationType::Pocket,
            OperationType::Profile,
            OperationType::Zigzag,
            OperationType::Adaptive,
            OperationType::Bore,
            OperationType::Drill,
        ] {
            let schema = OperationConfig::schema_for_type(op);
            let floor = schema
                .floor
                .unwrap_or_else(|| panic!("{} gives a floor schema", op.label()));
            assert_eq!(floor["set_through"], "set_toolpath_floor");
            // R-B: the schema lists the references that the op admits
            // (`Floor::admits`): each hole floor on a Drill and a Bore, no
            // face floor on a Drill.
            let admitted = Floor::EACH_REFERENCE
                .iter()
                .filter(|f| f.admits(op.capabilities().item_use))
                .count();
            // Non-vacuity: a Pocket has no hole floor, a Drill no face
            // floor, a Bore every reference.
            let want = match op {
                OperationType::Drill => Floor::EACH_REFERENCE.len() - 2,
                OperationType::Bore => Floor::EACH_REFERENCE.len(),
                OperationType::Face
                | OperationType::Pocket
                | OperationType::Profile
                | OperationType::Adaptive
                | OperationType::VCarve
                | OperationType::Inlay
                | OperationType::InlayPlug
                | OperationType::Zigzag
                | OperationType::Trace
                | OperationType::Chamfer
                | OperationType::DropCutter
                | OperationType::Adaptive3d
                | OperationType::Waterline
                | OperationType::Pencil
                | OperationType::Scallop
                | OperationType::UnifiedFinish
                | OperationType::SteepShallow
                | OperationType::RampFinish
                | OperationType::SpiralFinish
                | OperationType::RadialFinish
                | OperationType::HorizontalFinish
                | OperationType::ProjectCurve
                | OperationType::AlignmentPinDrill => Floor::EACH_REFERENCE.len() - 1,
            };
            assert_eq!(admitted, want, "{}", op.label());
            assert_eq!(
                floor["variants"].as_array().map(Vec::len),
                Some(admitted),
                "{floor}"
            );
            assert!(
                schema
                    .params
                    .iter()
                    .all(|p| p.name != "floor" && p.name != "depth"),
                "{}: the floor is driver-set and no depth is published",
                op.label()
            );
        }
        assert!(
            OperationConfig::schema_for_type(OperationType::Face)
                .floor
                .is_none(),
            "Face sets its depth with its own dial"
        );

        // P6.8 (design section 8, the `get_operation_schema` row): the Bore
        // publishes its dials, and the Pocket publishes `open_paths` with
        // its three tokens (a string enum, not the object of the design).
        let bore = OperationConfig::schema_for_type(OperationType::Bore);
        for name in [
            "pitch",
            "climb",
            "finish_allowance",
            "feed_rate",
            "plunge_rate",
            "ramp_feed_rate",
            "spindle_rpm",
        ] {
            assert!(
                bore.params.iter().any(|p| p.name == name),
                "the Bore schema gives `{name}`"
            );
        }
        let pocket = OperationConfig::schema_for_type(OperationType::Pocket);
        let open_paths = pocket
            .params
            .iter()
            .find(|p| p.name == "open_paths")
            .expect("the Pocket schema gives open_paths");
        assert_eq!(open_paths.default, serde_json::json!("skip"));
        let text = serde_json::to_string(open_paths).unwrap();
        for token in ["skip", "extend_left", "extend_right"] {
            assert!(text.contains(token), "open_paths names `{token}`: {text}");
        }
    }
}
