//! R2 (Fusion, operator ruling 2026-10-23): an op may exist with NO tool.
//!
//! # The claim
//!
//! 1. An add that no project tool fits still MAKES the op. The draft door
//!    (`ProjectSession::draft_toolpath`, `ToolChoice::FirstFitting`) gives
//!    `tool_id: None` and the state "No tool fits: needs ...". The op
//!    refuses generation with `SessionError::NoTool` and the fix
//!    (`CHOOSE_A_TOOL_FIX`), and its diagnostics hold `precondition.no_tool`.
//! 2. A project file round-trips an op with no tool: the toolpath section
//!    writes no `tool_id` key, and the load gives `tool_id: None` with no
//!    missing-tool warning.
//! 3. The boundary: the no-tool draft of a 3D op has the default boundary
//!    (no diameter to grow by). The first `SetToolpathTool` applies THE
//!    boundary rule of the draft (`BoundaryConfig::for_3d_op(d)`). A later
//!    tool change keeps the boundary.
//!
//! Red proof: make `draft_toolpath` refuse when `FirstFitting` finds no
//! tool (the old GUI rule); test 1 fails at the draft. Make the file field
//! a required `ToolId` again, with a sentinel id on save; test 2 fails on
//! the key.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test an_op_with_no_tool_is_a_state_r2
//! ```

#![expect(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::session::{mesh_model, stock_over};
use common::tools::{ball_tool_config, endmill_tool_config};
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::config::BoundaryConfig;
use rs_cam_core::diagnostics::ids;
use rs_cam_core::session::{
    AddToolArgs, AddToolpathArgs, CHOOSE_A_TOOL_FIX, Command, DraftRequest, ProjectSession,
    ProjectSessionBuilder, ReplaceToolpathConfigArgs, SessionError, SetToolpathToolArgs,
};

/// A 40 mm flat mesh and one flat end mill. A Scallop needs a ball nose.
fn session() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock_over(25.0, 10.0));
    let _ = builder.add_tool(endmill_tool_config(6.0));
    let _ = builder.add_model(mesh_model(rs_cam_core::mesh::make_test_flat(40.0), "flat"));
    builder.build()
}

/// The state text of a Scallop that no tool fits.
const SCALLOP_STATE: &str = "No tool fits: needs a ball nose or a tapered ball nose";

/// Draft a Scallop with the first fitting tool and add it. The index.
fn add_scallop(s: &mut ProjectSession) -> usize {
    let draft = s
        .draft_toolpath(DraftRequest::new(OperationType::Scallop, 0, "Scallop"))
        .expect("the draft makes the op with no tool");
    assert_eq!(draft.config.tool_id, None, "no tool fits a Scallop");
    assert_eq!(draft.no_tool.as_deref(), Some(SCALLOP_STATE));
    assert_eq!(
        draft.config.boundary,
        BoundaryConfig::default(),
        "with no tool there is no diameter to grow the 3D boundary by"
    );
    s.apply(Command::AddToolpath(AddToolpathArgs {
        setup_index: 0,
        config: Box::new(draft.config),
    }))
    .expect("the add takes an op with no tool")
    .created
    .expect("the add reports the op")
}

#[test]
fn an_add_with_no_fitting_tool_makes_the_op_r2() {
    let mut s = session();
    let index = add_scallop(&mut s);
    assert_eq!(s.toolpath_configs().len(), 1);
    assert_eq!(s.no_tool_state(index).as_deref(), Some(SCALLOP_STATE));
    assert_eq!(s.list_toolpaths()[0].tool_name, "No tool");

    // Generation refuses with the state and the fix.
    let cancel = AtomicBool::new(false);
    let err = s
        .generate_toolpath(index, &cancel)
        .map(|_| ())
        .expect_err("an op with no tool must refuse generation");
    assert!(matches!(err, SessionError::NoTool { .. }), "{err:?}");
    assert_eq!(err.field(), Some("tool_id"));
    let text = err.to_string();
    assert!(
        text.contains(SCALLOP_STATE) && text.contains(CHOOSE_A_TOOL_FIX),
        "the refusal names the state and the fix: {text}"
    );

    // The diagnostics hold the no-tool finding, Severe, with the fix.
    let diags = s
        .diagnose_toolpath_with_trace(index, None)
        .expect("the diagnostics of an op with no tool");
    let finding = diags
        .iter()
        .find(|d| d.id.as_str() == ids::PRECOND_NO_TOOL)
        .unwrap_or_else(|| panic!("no `precondition.no_tool` in {diags:?}"));
    assert_eq!(finding.severity, rs_cam_core::diagnostics::Severity::Severe);
    assert!(
        finding.message.contains(CHOOSE_A_TOOL_FIX),
        "{}",
        finding.message
    );

    // Non-vacuity: a ball nose added to the project binds the waiting
    // Scallop (P3, operator ruling 2026-10-30), so the state goes, and a
    // draft takes the ball nose.
    let _ = s
        .apply(Command::AddTool(AddToolArgs {
            tool: Box::new(ball_tool_config(6.0)),
        }))
        .unwrap();
    assert_eq!(s.toolpath_configs()[index].tool_id, Some(s.tools()[1].id));
    assert_eq!(s.no_tool_state(index), None);
    let draft = s
        .draft_toolpath(DraftRequest::new(OperationType::Scallop, 0, "Scallop 2"))
        .unwrap();
    assert_eq!(draft.config.tool_id, Some(s.tools()[1].id));
    assert_eq!(draft.no_tool, None);
}

#[test]
fn a_project_file_round_trips_an_op_with_no_tool_r2() {
    let mut s = session();
    let index = add_scallop(&mut s);
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("no_tool_r2_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("no_tool.toml");
    s.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let file: toml::Value = toml::from_str(&text).unwrap();
    let toolpaths: Vec<&toml::Value> = file
        .get("setups")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|setup| setup.get("toolpaths").and_then(toml::Value::as_array))
        .flatten()
        .collect();
    assert_eq!(toolpaths.len(), 1, "one toolpath in the file:\n{text}");
    assert!(
        toolpaths[0].get("tool_id").is_none(),
        "an op with no tool writes no `tool_id` key:\n{text}"
    );

    let (loaded, warnings) = ProjectSession::load_with_warnings(&path).unwrap();
    assert_eq!(loaded.toolpath_configs()[index].tool_id, None);
    assert!(
        !warnings.iter().any(|w| matches!(
            w,
            rs_cam_core::session::ProjectLoadWarning::MissingToolReference { .. }
        )),
        "no tool is not a missing tool: {warnings:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_first_tool_sets_the_3d_boundary_r2() {
    let mut s = session();
    let index = add_scallop(&mut s);
    let _ = s
        .apply(Command::AddTool(AddToolArgs {
            tool: Box::new(ball_tool_config(4.0)),
        }))
        .unwrap();
    let ball_id = s
        .tools()
        .iter()
        .find(|t| t.diameter == 4.0)
        .map(|t| t.id)
        .unwrap();
    let _ = s
        .apply(Command::SetToolpathTool(SetToolpathToolArgs {
            index,
            tool_id: ball_id,
        }))
        .unwrap();
    let tc = &s.toolpath_configs()[index];
    assert_eq!(tc.tool_id, Some(ball_id));
    assert_eq!(
        tc.boundary,
        BoundaryConfig::for_3d_op(4.0),
        "the first tool applies the draft boundary rule"
    );

    // A second tool change keeps the boundary: it is no longer the
    // inherited default, and the rule is a creation rule.
    let big = ball_tool_config(8.0);
    let _ = s
        .apply(Command::AddTool(AddToolArgs {
            tool: Box::new(big),
        }))
        .unwrap();
    let big_id = s
        .tools()
        .iter()
        .find(|t| t.diameter == 8.0)
        .map(|t| t.id)
        .unwrap();
    let _ = s
        .apply(Command::SetToolpathTool(SetToolpathToolArgs {
            index,
            tool_id: big_id,
        }))
        .unwrap();
    assert_eq!(
        s.toolpath_configs()[index].boundary,
        BoundaryConfig::for_3d_op(4.0)
    );
}

/// The GUI Tool combo commits the whole config (`ReplaceToolpathConfig`).
/// The first tool through that door applies the same boundary rule.
#[test]
fn the_first_tool_by_the_gui_door_sets_the_3d_boundary_r2() {
    let mut s = session();
    let index = add_scallop(&mut s);
    let _ = s
        .apply(Command::AddTool(AddToolArgs {
            tool: Box::new(ball_tool_config(4.0)),
        }))
        .unwrap();
    let ball_id = s
        .tools()
        .iter()
        .find(|t| t.diameter == 4.0)
        .map(|t| t.id)
        .unwrap();
    let mut config = s.toolpath_configs()[index].clone();
    config.tool_id = Some(ball_id);
    let _ = s
        .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
            index,
            config: Box::new(config),
        }))
        .unwrap();
    assert_eq!(
        s.toolpath_configs()[index].boundary,
        BoundaryConfig::for_3d_op(4.0),
        "the GUI door applies the first-tool boundary rule"
    );
}
