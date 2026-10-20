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
        floor: p.floor.into(),
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
        "floor_label": floor.map(|f| f.reference_label()),
        "floor_z": floor_z.and_then(Result::ok),
        "floor_refusal": floor_z.and_then(Result::err).map(|r| r.to_string()),
    })
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
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
            0,
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
        let config = ToolpathConfig {
            shoe: Default::default(),
            id: rs_cam_core::ToolpathId(0),
            name: "Pocket".to_owned(),
            enabled: true,
            operation: OperationConfig::new_default(OperationType::Pocket),
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
    fn mcp_and_gui_give_one_floor_p3_3() {
        let floor = Floor::Z { z: FLOOR_Z };
        let file_json = serde_json::to_value(floor).unwrap();

        // MCP: the wire request through the core door.
        let mut mcp = project();
        let before = mcp.get_toolpath_config(0).unwrap().operation.floor();
        assert_ne!(before, Some(floor), "the default is not the test floor");
        let effects = wire_floor(&mut mcp, &file_json).expect("the wire floor applies");
        assert!(effects.stale.contains(&0), "the floor stales the toolpath");

        // GUI: the inspector edits the draft and writes the whole config.
        let mut state = crate::state::AppState::new();
        state.session = project();
        let id = state.session.toolpath_configs()[0].id;
        let mut entry =
            crate::ui::properties::build_entry_from_session_and_gui(id, &state.session, &state.gui)
                .expect("the toolpath has an entry");
        match &mut entry.operation {
            OperationConfig::Pocket(c) => c.floor = floor,
            other => panic!("the fixture op is a Pocket, not {}", other.label()),
        }
        let _ = crate::ui::properties::write_entry_config_to_session(&entry, &mut state)
            .expect("the GUI write applies");

        // One config.
        let mcp_tc = mcp.get_toolpath_config(0).unwrap();
        let gui_tc = state.session.get_toolpath_config(0).unwrap();
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
        assert_eq!(mcp_tc.operation.floor(), Some(floor));

        // One `toolpath_heights`.
        let mcp_heights = toolpath_heights_json(&mcp, mcp_tc);
        let gui_heights = toolpath_heights_json(&state.session, gui_tc);
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
                mcp.get_toolpath_config(0).unwrap().operation.floor(),
                Some(floor),
                "{refused} wrote the config"
            );
        }
    }

    /// Geometry selection P3.8 (design section 9): the two refusal rows
    /// and the schema row of the floor name the floor door.
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
        let before = s.get_toolpath_config(0).unwrap().operation.floor();
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
            s.get_toolpath_config(0).unwrap().operation.floor(),
            before,
            "the refused write keeps the floor"
        );

        for op in [
            OperationType::Pocket,
            OperationType::Profile,
            OperationType::Zigzag,
            OperationType::Adaptive,
        ] {
            let schema = OperationConfig::schema_for_type(op);
            let floor = schema
                .floor
                .unwrap_or_else(|| panic!("{} gives a floor schema", op.label()));
            assert_eq!(floor["set_through"], "set_toolpath_floor");
            assert_eq!(
                floor["variants"].as_array().map(Vec::len),
                Some(Floor::EACH_REFERENCE.len()),
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
    }
}
