//! F029 step 1: a tool record outside its domain is refused at every door.
//!
//! The defect (tech-debt review 2026-10-19, F029): MCP `set_tool_param
//! {included_angle: 0}` wrote the value. The next tool-panel draw called
//! `build_cutter`, and `VBitEndmill::new` asserted `0 < angle < 180`. The
//! panic took the GUI thread down, and the unsaved project with it.
//!
//! Each test below takes one door and gives it a V-bit with an included
//! angle of 0. The door must refuse with a typed error and write nothing.
//! The domain is `rs_cam_core::tool::ToolDimension`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::build_cutter;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::io::tool_library::{self, ToolCatalog, ToolLibraryError};
use rs_cam_core::session::{
    AddToolArgs, Command, ProjectSession, ProjectSessionBuilder, ReplaceToolArgs, ReplaceToolsArgs,
    SessionError, SetToolParamArgs,
};
use rs_cam_core::tool::{
    BullNoseEndmill, TaperedBallEndmill, ToolDimension, ToolGeometryError, VBitEndmill,
};

fn vbit(id: usize) -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(id), ToolType::VBit);
    tool.name = "60 V".to_owned();
    tool.included_angle = 60.0;
    tool
}

fn flat_vbit(id: usize) -> ToolConfig {
    let mut tool = vbit(id);
    tool.included_angle = 0.0;
    tool
}

fn session_with_vbit() -> ProjectSession {
    ProjectSessionBuilder::new().tool(vbit(0)).build()
}

fn assert_refused_angle(result: Result<rs_cam_core::session::Effects, SessionError>) {
    match result {
        Err(SessionError::ToolOutOfDomain { error, .. }) => {
            assert_eq!(error.dimension, ToolDimension::IncludedAngle);
            assert_eq!(error.value, 0.0);
        }
        other => panic!("expected ToolOutOfDomain, got {other:?}"),
    }
}

#[test]
fn the_cutter_constructors_refuse_instead_of_panic() {
    assert!(VBitEndmill::new(6.0, 0.0, 10.0).is_err());
    assert!(VBitEndmill::new(6.0, 180.0, 10.0).is_err());
    assert!(VBitEndmill::new(6.0, 60.0, 10.0).is_ok());
    assert!(TaperedBallEndmill::new(1.0, 0.0, 6.0, 10.0).is_err());
    // A shaft below the tip is not a tapered ball.
    assert!(TaperedBallEndmill::new(2.0, 5.0, 1.0, 10.0).is_err());
    assert!(BullNoseEndmill::new(6.0, 3.5, 10.0).is_err());
    assert!(BullNoseEndmill::new(6.0, 1.0, 10.0).is_ok());
    assert!(build_cutter(&flat_vbit(0)).is_err());
    assert!(build_cutter(&vbit(0)).is_ok());
}

#[test]
fn set_tool_param_refuses_and_keeps_the_stored_tool() {
    let mut session = session_with_vbit();
    assert_refused_angle(session.apply(Command::SetToolParam(SetToolParamArgs {
        index: 0,
        param: "included_angle".to_owned(),
        value: serde_json::json!(0.0),
    })));
    assert_eq!(session.tools()[0].included_angle, 60.0, "nothing written");
    // The same door still writes a value inside the domain.
    let _ = session
        .apply(Command::SetToolParam(SetToolParamArgs {
            index: 0,
            param: "included_angle".to_owned(),
            value: serde_json::json!(90.0),
        }))
        .unwrap();
    assert_eq!(session.tools()[0].included_angle, 90.0);
}

#[test]
fn add_tool_and_add_from_library_refuse() {
    let mut session = session_with_vbit();
    assert_refused_angle(session.apply(Command::AddTool(AddToolArgs {
        tool: Box::new(flat_vbit(1)),
    })));
    assert_refused_angle(session.apply(Command::AddToolFromLibrary(AddToolArgs {
        tool: Box::new(flat_vbit(1)),
    })));
    assert_eq!(session.tools().len(), 1, "nothing added");
}

#[test]
fn replace_tool_and_replace_tools_refuse() {
    let mut session = session_with_vbit();
    assert_refused_angle(session.apply(Command::ReplaceTool(ReplaceToolArgs {
        tool_id: 0,
        config: Box::new(flat_vbit(0)),
    })));
    assert_refused_angle(session.apply(Command::ReplaceTools(ReplaceToolsArgs {
        tools: vec![vbit(0), flat_vbit(1)],
    })));
    assert_eq!(session.tools().len(), 1);
    assert_eq!(session.tools()[0].included_angle, 60.0, "nothing written");
}

#[test]
fn the_project_loader_refuses() {
    // The builder is a fixture door and checks nothing, so it can write
    // the bad record to a file for the loader to read.
    let session = ProjectSessionBuilder::new().tool(flat_vbit(0)).build();
    let dir = std::env::temp_dir().join("rs_cam_f029_project_load");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bad_tool.toml");
    session.save(&path).unwrap();
    match ProjectSession::load(&path) {
        Err(SessionError::ToolOutOfDomain { tool, error }) => {
            assert_eq!(tool, "60 V");
            assert_eq!(error.dimension, ToolDimension::IncludedAngle);
        }
        Err(other) => panic!("expected ToolOutOfDomain, got {other}"),
        Ok(_) => panic!("the loader accepted a V-bit with no angle"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_tool_library_refuses_on_load_and_on_write() {
    let dir = std::env::temp_dir().join("rs_cam_f029_tool_library");
    let _ = std::fs::remove_dir_all(&dir);
    // A write door refuses the bad record, so a Save cannot make a catalog
    // that the loader refuses.
    assert!(matches!(
        tool_library::append_to(&dir, "vbits", flat_vbit(0)),
        Err(ToolLibraryError::InvalidTool { .. })
    ));
    assert!(matches!(
        tool_library::add_or_replace_to(&dir, "vbits", flat_vbit(0)),
        Err(ToolLibraryError::InvalidTool { .. })
    ));
    // A catalog written by another program is refused on load.
    let catalog = ToolCatalog {
        tools: vec![vbit(0), flat_vbit(1)],
    };
    tool_library::save_to(&dir, "vbits", &catalog).unwrap();
    match tool_library::load_from(&dir, "vbits") {
        Err(ToolLibraryError::InvalidTool { source, .. }) => {
            let ToolGeometryError { dimension, .. } = source;
            assert_eq!(dimension, ToolDimension::IncludedAngle);
        }
        other => panic!("expected InvalidTool, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}
