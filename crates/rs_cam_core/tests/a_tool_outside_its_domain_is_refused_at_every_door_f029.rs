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

#![expect(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::build_cutter;
use rs_cam_core::compute::tool_config::{NonCentreCuttingGeometry, ToolConfig, ToolId, ToolType};
use rs_cam_core::io::tool_library::{self, ToolCatalog, ToolLibraryError};
use rs_cam_core::session::{
    AddToolArgs, Command, ProjectSession, ProjectSessionBuilder, ReplaceToolArgs, ReplaceToolsArgs,
    SessionError, SetToolParamArgs,
};
use rs_cam_core::tool::{
    BullNoseEndmill, TaperedBallEndmill, ToolDimension, ToolGeometryError, VBitEndmill,
};

fn vbit(id: usize) -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(id), ToolType::VBit);
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
fn invalid_non_centre_cutting_param_rolls_back() {
    let mut session = ProjectSessionBuilder::new()
        .tool(ToolConfig::test_default(
            ToolId::from_raw(0),
            ToolType::EndMill,
        ))
        .build();
    let valid = serde_json::json!({
        "centre_core_diameter_mm": 3.0,
        "blade_protrusion_mm": 1.0,
    });
    let _ = session
        .apply(Command::SetToolParam(SetToolParamArgs {
            index: 0,
            param: "centre_cutting".to_owned(),
            value: valid,
        }))
        .unwrap();
    let before = session.tools()[0].non_centre_cutting_geometry();
    let invalid = serde_json::json!({
        "centre_core_diameter_mm": 7.0,
        "blade_protrusion_mm": 1.0,
    });
    assert!(
        session
            .apply(Command::SetToolParam(SetToolParamArgs {
                index: 0,
                param: "centre_cutting".to_owned(),
                value: invalid,
            }))
            .is_err()
    );
    assert_eq!(session.tools()[0].non_centre_cutting_geometry(), before);
    assert_eq!(
        before,
        Some(NonCentreCuttingGeometry {
            centre_core_diameter_mm: 3.0,
            blade_protrusion_mm: 1.0,
        })
    );
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
        tool_id: rs_cam_core::ids::ToolId::from_raw(0),
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

/// P7 A2: the flat tip of a V-bit is in `0 <= tip < diameter` (at the
/// diameter the cone has no height). A tip outside it is refused at every
/// door, as the angle is: the cutter, `set_tool_param`, `AddTool`,
/// `ReplaceTool`, the project loader and the tool library.
#[test]
fn a_v_tip_outside_its_domain_is_refused_at_every_door() {
    use rs_cam_core::compute::tool_config::ToolFact;
    let with_tip = |id: usize, tip: f64| {
        let mut tool = vbit(id);
        tool.set_v_tip_diameter(ToolFact::Stated(tip));
        tool
    };
    let diameter = vbit(0).diameter;
    let bad = with_tip(1, diameter);
    let assert_refused_tip =
        |result: Result<rs_cam_core::session::Effects, SessionError>| match result {
            Err(SessionError::ToolOutOfDomain { error, .. }) => {
                assert_eq!(error.dimension, ToolDimension::VTipDiameter);
            }
            other => panic!("expected the V tip refusal, got {other:?}"),
        };

    // The domain ends: 0 and a tip below the diameter are in; the diameter
    // and a negative tip are out.
    assert!(build_cutter(&with_tip(0, 0.0)).is_ok());
    assert!(build_cutter(&with_tip(0, diameter / 2.0)).is_ok());
    assert!(build_cutter(&bad).is_err());
    assert!(build_cutter(&with_tip(0, -0.1)).is_err());

    let mut session = session_with_vbit();
    assert_refused_tip(session.apply(Command::SetToolParam(SetToolParamArgs {
        index: 0,
        param: "v_tip_diameter".to_owned(),
        value: serde_json::json!(diameter),
    })));
    assert_eq!(
        session.tools()[0].v_tip_diameter_fact(),
        vbit(0).v_tip_diameter_fact(),
        "nothing written"
    );
    assert_refused_tip(session.apply(Command::AddTool(AddToolArgs {
        tool: Box::new(bad.clone()),
    })));
    assert_refused_tip(session.apply(Command::ReplaceTool(ReplaceToolArgs {
        tool_id: rs_cam_core::ids::ToolId::from_raw(0),
        config: Box::new(with_tip(0, diameter)),
    })));
    assert_eq!(session.tools().len(), 1, "nothing added");

    let dir = std::env::temp_dir().join("rs_cam_f029_v_tip");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bad_tip.toml");
    ProjectSessionBuilder::new()
        .tool(bad.clone())
        .build()
        .save(&path)
        .unwrap();
    match ProjectSession::load(&path) {
        Err(SessionError::ToolOutOfDomain { error, .. }) => {
            assert_eq!(error.dimension, ToolDimension::VTipDiameter);
        }
        Err(other) => panic!("expected the V tip refusal, got {other}"),
        Ok(_) => panic!("the loader accepted a V tip as wide as the bit"),
    }
    assert!(matches!(
        tool_library::append_to(&dir, "vbits", bad.clone()),
        Err(ToolLibraryError::InvalidTool { .. })
    ));
    tool_library::save_to(
        &dir,
        "vbits",
        &ToolCatalog {
            tools: vec![vbit(0), bad],
        },
    )
    .unwrap();
    match tool_library::load_from(&dir, "vbits") {
        Err(ToolLibraryError::InvalidTool { source, .. }) => {
            assert_eq!(source.dimension, ToolDimension::VTipDiameter);
        }
        other => panic!("expected InvalidTool, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// P22-0c-6a (ruling 6): a shank is a diameter, so `> 0`. A stated shank
/// of 0 is not a fact: every door refuses it, as it refuses the V tip
/// (the cutter door, `set_tool_param`, `AddTool`, `AddToolFromLibrary`,
/// `ReplaceTool`, `ReplaceTools`, the project loader and the tool library,
/// on write and on load). A tool that does not know its shank states
/// `Unknown`; that is in, and plans on the legacy default.
#[test]
fn a_stated_shank_of_zero_is_refused_at_every_door_p22_0c6() {
    use rs_cam_core::compute::tool_config::ToolFact;
    let flat = |id: usize, shank: ToolFact<f64>| {
        let mut tool = ToolConfig::test_default(ToolId::from_raw(id), ToolType::EndMill);
        tool.name = "6 flat".to_owned();
        tool.set_shank_diameter(shank);
        tool
    };
    let bad = |id: usize| flat(id, ToolFact::Stated(0.0));
    let assert_refused_shank =
        |result: Result<rs_cam_core::session::Effects, SessionError>| match result {
            Err(SessionError::ToolOutOfDomain { error, .. }) => {
                assert_eq!(error.dimension, ToolDimension::ShankDiameter);
                assert_eq!(error.value, 0.0);
            }
            other => panic!("expected the shank refusal, got {other:?}"),
        };

    // The domain ends: a shank above 0 and an Unknown shank are in; 0 and
    // a negative shank are out, in every fact form.
    assert!(build_cutter(&flat(0, ToolFact::Stated(6.0))).is_ok());
    assert!(build_cutter(&flat(0, ToolFact::Unknown)).is_ok());
    assert!(build_cutter(&bad(0)).is_err());
    assert!(build_cutter(&flat(0, ToolFact::Stated(-0.1))).is_err());
    assert!(
        build_cutter(&flat(
            0,
            ToolFact::Estimated {
                value: 0.0,
                basis: "a test".to_owned(),
            }
        ))
        .is_err()
    );
    assert!(
        build_cutter(&flat(
            0,
            ToolFact::Catalogue {
                value: 0.0,
                source: "a test".to_owned(),
            }
        ))
        .is_err()
    );

    let good = flat(0, ToolFact::Stated(6.0));
    let mut session = ProjectSessionBuilder::new().tool(good.clone()).build();
    assert_refused_shank(session.apply(Command::SetToolParam(SetToolParamArgs {
        index: 0,
        param: "shank_diameter".to_owned(),
        value: serde_json::json!(0.0),
    })));
    assert_eq!(
        session.tools()[0].shank_diameter_fact(),
        good.shank_diameter_fact(),
        "nothing written"
    );
    assert_refused_shank(session.apply(Command::AddTool(AddToolArgs {
        tool: Box::new(bad(1)),
    })));
    assert_refused_shank(session.apply(Command::AddToolFromLibrary(AddToolArgs {
        tool: Box::new(bad(1)),
    })));
    assert_refused_shank(session.apply(Command::ReplaceTool(ReplaceToolArgs {
        tool_id: rs_cam_core::ids::ToolId::from_raw(0),
        config: Box::new(bad(0)),
    })));
    assert_refused_shank(session.apply(Command::ReplaceTools(ReplaceToolsArgs {
        tools: vec![good.clone(), bad(1)],
    })));
    assert_eq!(session.tools().len(), 1, "nothing added");
    assert_eq!(
        session.tools()[0].shank_diameter_fact(),
        good.shank_diameter_fact(),
        "nothing replaced"
    );

    let dir = std::env::temp_dir().join("rs_cam_f029_shank_zero");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bad_shank.toml");
    ProjectSessionBuilder::new()
        .tool(bad(0))
        .build()
        .save(&path)
        .unwrap();
    match ProjectSession::load(&path) {
        Err(SessionError::ToolOutOfDomain { tool, error }) => {
            assert_eq!(tool, "6 flat");
            assert_eq!(error.dimension, ToolDimension::ShankDiameter);
        }
        Err(other) => panic!("expected the shank refusal, got {other}"),
        Ok(_) => panic!("the loader accepted a shank of 0"),
    }
    assert!(matches!(
        tool_library::append_to(&dir, "flats", bad(0)),
        Err(ToolLibraryError::InvalidTool { .. })
    ));
    assert!(matches!(
        tool_library::add_or_replace_to(&dir, "flats", bad(0)),
        Err(ToolLibraryError::InvalidTool { .. })
    ));
    tool_library::save_to(
        &dir,
        "flats",
        &ToolCatalog {
            tools: vec![good, bad(1)],
        },
    )
    .unwrap();
    match tool_library::load_from(&dir, "flats") {
        Err(ToolLibraryError::InvalidTool { source, .. }) => {
            assert_eq!(source.dimension, ToolDimension::ShankDiameter);
        }
        other => panic!("expected InvalidTool, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&dir);
}
