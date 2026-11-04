//! P10 (feeds contract, operator ruling D8, lead change 3): a tool change
//! moves a geometry value that its rule wrote and keeps a value the
//! operator set.
//!
//! # The claim
//!
//! On both tool doors (`SetToolpathTool`, and `ReplaceToolpathConfig`, the
//! GUI Tool combo):
//!
//! 1. A Pocket drafted with a 6 mm tool: its rule stepover follows the new
//!    tool (12 mm: ratio 2); a typed depth per pass stays.
//! 2. A Drill: a typed peck (the generic setter, a new stamp slot) stays.
//!
//! Red proof (HEAD b92b78090, before P10): the stepover stayed 2 -> 2
//! (the red run is in the P10 report).
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_tool_change_moves_a_derived_geometry_value_d8
//! ```

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::session::{
    AddToolArgs, AddToolpathArgs, Command, DraftRequest, ProjectSession, ReplaceToolpathConfigArgs,
    SetToolpathParamArgs, SetToolpathToolArgs,
};

fn change_tool(
    s: &mut ProjectSession,
    door: &str,
    index: usize,
    other: rs_cam_core::compute::tool_config::ToolId,
) {
    if door == "set_toolpath_tool" {
        let _ = s
            .apply(Command::SetToolpathTool(SetToolpathToolArgs {
                index,
                tool_id: other,
            }))
            .expect("the tool door");
    } else {
        let mut config = s.toolpath_configs()[index].clone();
        config.tool_id = Some(other);
        let _ = s
            .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
                index,
                config: Box::new(config),
            }))
            .expect("the replace door");
    }
}

fn add(s: &mut ProjectSession, op: OperationType) -> usize {
    let first = s.tools()[0].id;
    let draft = s
        .draft_toolpath(DraftRequest::new(op, 0, "op").tool(first))
        .expect("the draft");
    s.apply(Command::AddToolpath(AddToolpathArgs {
        setup_index: 0,
        config: Box::new(draft.config),
    }))
    .expect("the add")
    .created
    .expect("created")
}

#[test]
fn a_tool_change_keeps_a_typed_peck_d8() {
    for door in ["set_toolpath_tool", "replace_toolpath_config"] {
        let mut s = session();
        let index = add(&mut s, OperationType::Drill);
        let _ = s
            .apply(Command::SetToolpathParam(SetToolpathParamArgs {
                index,
                param: "peck_depth".to_owned(),
                value: serde_json::json!(1.25),
            }))
            .expect("the typed peck");
        let other = s.tools()[1].id;
        change_tool(&mut s, door, index, other);
        let peck = s.toolpath_configs()[index]
            .operation
            .params_value_including_nulls()["peck_depth"]
            .as_f64();
        assert_eq!(peck, Some(1.25), "{door}: the typed peck stays");
    }
}

fn session() -> ProjectSession {
    let mut s = ProjectSession::new_empty();
    for d in [6.0, 12.0] {
        let _ = s
            .apply(Command::AddTool(AddToolArgs {
                tool: Box::new(
                    ToolConfig::builder(ToolId::from_raw(0), ToolType::EndMill)
                        .diameter(d)
                        .build(),
                ),
            }))
            .expect("the tool");
    }
    s
}

#[test]
fn a_tool_change_moves_a_derived_value_and_keeps_a_typed_one_d8() {
    for door in ["set_toolpath_tool", "replace_toolpath_config"] {
        let mut s = session();
        let first = s.tools()[0].id;
        let draft = s
            .draft_toolpath(DraftRequest::new(OperationType::Pocket, 0, "Pocket").tool(first))
            .expect("the draft");
        let index = s
            .apply(Command::AddToolpath(AddToolpathArgs {
                setup_index: 0,
                config: Box::new(draft.config),
            }))
            .expect("the add")
            .created
            .expect("created");
        let _ = s
            .apply(Command::SetToolpathParam(SetToolpathParamArgs {
                index,
                param: "depth_per_pass".to_owned(),
                value: serde_json::json!(0.77),
            }))
            .expect("the typed depth");
        let stepover_before = s.toolpath_configs()[index]
            .operation
            .stepover()
            .expect("a stepover");
        let other = s.tools()[1].id;
        if door == "set_toolpath_tool" {
            let _ = s
                .apply(Command::SetToolpathTool(SetToolpathToolArgs {
                    index,
                    tool_id: other,
                }))
                .expect("the tool door");
        } else {
            let mut config = s.toolpath_configs()[index].clone();
            config.tool_id = Some(other);
            let _ = s
                .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
                    index,
                    config: Box::new(config),
                }))
                .expect("the replace door");
        }
        let op = &s.toolpath_configs()[index].operation;
        let stepover_after = op.stepover().expect("a stepover");
        assert!(
            ((stepover_after / stepover_before) - 2.0).abs() < 1e-9,
            "{door}: the derived stepover follows D 6 -> 12: {stepover_before} -> {stepover_after}"
        );
        assert_eq!(
            op.depth_per_pass(),
            Some(0.77),
            "{door}: the typed depth stays"
        );
    }
}

/// Lead answer Q-F: an old project file holds no `peck_depth` or `z_step`
/// stamp slot. It loads, and a stated value with no stamp is an operator
/// value: a tool change keeps it.
#[test]
fn an_old_file_value_with_no_stamp_stays_on_a_tool_change_d8() {
    // An old file's stamps: no P10 slot.
    let old: rs_cam_core::feeds::FeedsProvenance =
        toml::from_str("stepover = { source = \"manual\" }\n").expect("an old stamp table loads");
    assert!(old.peck_depth.is_none() && old.z_step.is_none());
    for door in ["set_toolpath_tool", "replace_toolpath_config"] {
        let mut s = session();
        let first = s.tools()[0].id;
        let mut config = s
            .draft_toolpath(DraftRequest::new(OperationType::Pocket, 0, "Pocket").tool(first))
            .expect("the draft")
            .config;
        // The file states the value and holds no stamp for it.
        config.feeds_provenance = rs_cam_core::feeds::FeedsProvenance::default();
        let stated = config.operation.stepover().expect("a stated stepover");
        let index = s
            .apply(Command::AddToolpath(AddToolpathArgs {
                setup_index: 0,
                config: Box::new(config),
            }))
            .expect("the add")
            .created
            .expect("created");
        let other = s.tools()[1].id;
        change_tool(&mut s, door, index, other);
        assert_eq!(
            s.toolpath_configs()[index].operation.stepover(),
            Some(stated),
            "{door}: a value with no stamp stays"
        );
    }
}
