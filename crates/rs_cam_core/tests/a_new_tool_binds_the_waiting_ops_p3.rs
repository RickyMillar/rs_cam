//! P3 (operator ruling 2026-10-30): a tool added to the project binds every
//! toolpath that has no tool and accepts the tool.
//!
//! # The claim
//!
//! 1. A Scallop and a 3D Rough have no tool (the project has none). A flat
//!    end mill arrives through `Command::AddTool`. The 3D Rough takes it;
//!    the Scallop needs a ball nose, refuses the end mill and keeps no
//!    tool. `Effects::stale` names the 3D Rough only.
//! 2. A ball nose arrives through `Command::AddToolFromLibrary`. The
//!    Scallop takes it; the 3D Rough keeps its end mill.
//!
//! Red proof: remove the `bind_waiting_toolpaths` call from
//! `ProjectSession::add_tool`. The first assertion after the add fails.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_new_tool_binds_the_waiting_ops_p3
//! ```

#![expect(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use common::session::{mesh_model, stock_over};
use common::tools::{ball_tool_config, endmill_tool_config};
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::session::{
    AddToolArgs, AddToolpathArgs, Command, DraftRequest, ProjectSession, ProjectSessionBuilder,
};

/// A 40 mm flat mesh and no tool.
fn session() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock_over(25.0, 10.0));
    let _ = builder.add_model(mesh_model(rs_cam_core::mesh::make_test_flat(40.0), "flat"));
    builder.build()
}

/// Draft an op of `op_type` with the first fitting tool and add it. The
/// index.
fn add(s: &mut ProjectSession, op_type: OperationType, name: &str) -> usize {
    let draft = s
        .draft_toolpath(DraftRequest::new(op_type, 0, name))
        .expect("the draft makes the op");
    s.apply(Command::AddToolpath(AddToolpathArgs {
        setup_index: 0,
        config: Box::new(draft.config),
    }))
    .expect("the add takes the op")
    .created
    .expect("the add reports the op")
}

#[test]
fn a_new_tool_binds_the_waiting_ops_p3() {
    let mut s = session();
    let scallop = add(&mut s, OperationType::Scallop, "Scallop");
    let rough = add(&mut s, OperationType::Adaptive3d, "3D Rough");
    assert_eq!(s.toolpath_configs()[scallop].tool_id, None, "no tool yet");
    assert_eq!(s.toolpath_configs()[rough].tool_id, None, "no tool yet");

    let effects = s
        .apply(Command::AddTool(AddToolArgs {
            tool: Box::new(endmill_tool_config(6.0)),
        }))
        .unwrap();
    let end_mill_id = s.tools()[0].id;
    assert_eq!(
        s.toolpath_configs()[rough].tool_id,
        Some(end_mill_id),
        "the 3D Rough takes the end mill"
    );
    assert!(effects.stale.contains(&rough), "{:?}", effects.stale);
    assert_eq!(
        s.toolpath_configs()[scallop].tool_id,
        None,
        "the Scallop refuses the end mill and keeps no tool"
    );
    assert!(!effects.stale.contains(&scallop), "{:?}", effects.stale);

    let effects = s
        .apply(Command::AddToolFromLibrary(AddToolArgs {
            tool: Box::new(ball_tool_config(6.0)),
        }))
        .unwrap();
    let ball_id = s.tools()[1].id;
    assert_eq!(
        s.toolpath_configs()[scallop].tool_id,
        Some(ball_id),
        "the Scallop takes the ball nose"
    );
    assert!(effects.stale.contains(&scallop), "{:?}", effects.stale);
    assert_eq!(
        s.toolpath_configs()[rough].tool_id,
        Some(end_mill_id),
        "a bound op keeps its tool"
    );
}
