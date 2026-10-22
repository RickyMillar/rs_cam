//! Geometry selection P6.1 (L-5): a Profile with side `On` and
//! compensation `InControl` is refused at every door.
//!
//! Side `On` puts the tool centre on the line. The controller offsets the
//! tool to one side of the path (G41/G42), and a cut on the line has no
//! side. `compute::domain::validate_operation` holds the rule, so each door
//! that calls it refuses the pair:
//!
//! - `AddToolpath`,
//! - the op change (`SetToolpathOperation`),
//! - the generic arm of `SetToolpathParam` (the MCP `set_toolpath_param`
//!   tool calls this door), once for `side` and once for `compensation`,
//! - `ReplaceToolpathConfig`,
//! - project load.
//!
//! Each door has a positive control: side `On` with `InComputer` passes.
//! The Profile generator then cuts side `On` on the line (P6.3; P6.1 gave
//! a "not yet" refusal there).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::make_endmill_6mm;
use common::session::toolpath_config;

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::domain::{DomainRefusal, OpDomainRefusal};
use rs_cam_core::compute::operation_configs::{CompensationType, ProfileSide};
use rs_cam_core::session::{
    AddToolpathArgs, Command, ProjectSession, ProjectSessionBuilder, ReplaceToolpathConfigArgs,
    SessionError, SetToolpathOperationArgs, SetToolpathParamArgs, ToolpathConfig,
};
use serde_json::json;

fn profile(side: ProfileSide, compensation: CompensationType) -> OperationConfig {
    let mut config = OperationConfig::new_default(OperationType::Profile);
    let OperationConfig::Profile(c) = &mut config else {
        panic!("the default of the op type")
    };
    c.side = side;
    c.compensation = compensation;
    config
}

fn on_in_control() -> OperationConfig {
    profile(ProfileSide::On, CompensationType::InControl)
}

fn on_in_computer() -> OperationConfig {
    profile(ProfileSide::On, CompensationType::InComputer)
}

fn tp(config: OperationConfig) -> ToolpathConfig {
    toolpath_config(
        "p6_1",
        config,
        rs_cam_core::ids::ToolId::from_raw(0),
        rs_cam_core::ids::ModelId::from_raw(0),
    )
}

/// A session with one tool, one setup and one valid Profile (side
/// Outside, compensation in the computer).
fn session() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new();
    let _ = builder.add_tool(make_endmill_6mm());
    let _ = builder
        .add_toolpath(0, tp(OperationConfig::new_default(OperationType::Profile)))
        .unwrap();
    builder.build()
}

/// The refusal is the L-5 refusal, and its text names the fix.
#[track_caller]
fn assert_side_on_in_control(result: Result<(), SessionError>) {
    match result {
        Err(SessionError::OutOfDomain(DomainRefusal::Operation { refusal, .. })) => {
            assert!(
                matches!(refusal, OpDomainRefusal::SideOnInControl { .. }),
                "expected the side On refusal, got {refusal}"
            );
            let text = refusal.to_string();
            assert!(
                text.contains("side On")
                    && text.contains("G41/G42")
                    && text.contains("In Computer"),
                "{text}"
            );
        }
        other => panic!("expected an operation domain refusal, got {other:?}"),
    }
}

fn stored(s: &ProjectSession, index: usize) -> (ProfileSide, CompensationType) {
    match &s.toolpath_configs()[index].operation {
        OperationConfig::Profile(c) => (c.side, c.compensation),
        other => panic!("expected a Profile, got {:?}", other.op_type()),
    }
}

#[test]
fn add_toolpath_refuses_side_on_in_control() {
    let mut s = session();
    let before = s.toolpath_count();
    let add = |s: &mut ProjectSession, config: OperationConfig| {
        s.apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(tp(config)),
        }))
        .map(|_| ())
    };
    assert_side_on_in_control(add(&mut s, on_in_control()));
    assert_eq!(s.toolpath_count(), before, "nothing added");
    // Positive controls: On in the computer, and a side in the control.
    add(&mut s, on_in_computer()).unwrap();
    add(
        &mut s,
        profile(ProfileSide::Outside, CompensationType::InControl),
    )
    .unwrap();
    assert_eq!(s.toolpath_count(), before + 2);
}

#[test]
fn the_op_change_refuses_side_on_in_control() {
    let mut s = session();
    let change = |s: &mut ProjectSession, operation: OperationConfig| {
        s.apply(Command::SetToolpathOperation(SetToolpathOperationArgs {
            index: 0,
            operation: Box::new(operation),
        }))
        .map(|_| ())
    };
    assert_side_on_in_control(change(&mut s, on_in_control()));
    assert_eq!(
        stored(&s, 0),
        (ProfileSide::Outside, CompensationType::InComputer),
        "nothing written"
    );
    change(&mut s, on_in_computer()).unwrap();
    assert_eq!(
        stored(&s, 0),
        (ProfileSide::On, CompensationType::InComputer)
    );
}

#[test]
fn the_param_setter_refuses_either_half_of_the_pair() {
    let set = |s: &mut ProjectSession, param: &str, value: &str| {
        s.apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: param.to_owned(),
            value: json!(value),
        }))
        .map(|_| ())
    };

    // The side half: compensation is InControl, then side On refuses.
    let mut s = session();
    set(&mut s, "compensation", "in_control").unwrap();
    assert_side_on_in_control(set(&mut s, "side", "on"));
    assert_eq!(
        stored(&s, 0),
        (ProfileSide::Outside, CompensationType::InControl),
        "nothing written"
    );

    // The compensation half: side is On, then InControl refuses.
    let mut s = session();
    set(&mut s, "side", "on").unwrap();
    assert_side_on_in_control(set(&mut s, "compensation", "in_control"));
    assert_eq!(
        stored(&s, 0),
        (ProfileSide::On, CompensationType::InComputer),
        "nothing written"
    );
    // Positive control: back to a side, then InControl is accepted.
    set(&mut s, "side", "inside").unwrap();
    set(&mut s, "compensation", "in_control").unwrap();
    assert_eq!(
        stored(&s, 0),
        (ProfileSide::Inside, CompensationType::InControl)
    );
}

#[test]
fn replace_toolpath_config_refuses_side_on_in_control() {
    let mut s = session();
    let replace = |s: &mut ProjectSession, operation: OperationConfig| {
        s.apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
            index: 0,
            config: Box::new(tp(operation)),
        }))
        .map(|_| ())
    };
    assert_side_on_in_control(replace(&mut s, on_in_control()));
    assert_eq!(
        stored(&s, 0),
        (ProfileSide::Outside, CompensationType::InComputer),
        "nothing written"
    );
    replace(&mut s, on_in_computer()).unwrap();
    assert_eq!(
        stored(&s, 0),
        (ProfileSide::On, CompensationType::InComputer)
    );
}

/// Save a session the raw builder made (it checks nothing), then load it.
fn round_trip(name: &str, config: OperationConfig) -> Result<ProjectSession, SessionError> {
    let mut builder = ProjectSessionBuilder::new();
    let _ = builder.add_tool(make_endmill_6mm());
    let _ = builder.add_toolpath(0, tp(config)).unwrap();
    let raw = builder.build();
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("p6_1_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("project.toml");
    raw.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(
        text.contains("side = \"on\""),
        "the file holds side On:\n{text}"
    );
    let loaded = ProjectSession::load(&path);
    let _ = std::fs::remove_dir_all(&dir);
    loaded
}

#[test]
fn the_loader_refuses_side_on_in_control() {
    let result = round_trip("refused", on_in_control()).map(|_| ());
    assert_side_on_in_control(result);
    let loaded = round_trip("accepted", on_in_computer()).unwrap();
    assert_eq!(
        stored(&loaded, 0),
        (ProfileSide::On, CompensationType::InComputer)
    );
}

/// Side `On` passes every door in the computer, and the Profile generator
/// cuts it on the line (P6.3; before P6.3 it refused with a "not yet"
/// text). The fixture outline is a closed rectangle: every cut point lies
/// on it.
#[test]
fn the_profile_generator_cuts_side_on_p6_3() {
    use common::op_fixtures::{flat_stock, outline_model, tool_for};
    use rs_cam_core::toolpath::MoveIntent;
    let model = outline_model();
    let mut s = common::session::single_op_session_with(
        flat_stock(),
        tool_for(OperationType::Profile),
        model,
        "p6_1",
        on_in_computer(),
        |_| {},
    );
    let cancel = AtomicBool::new(false);
    s.generate_toolpath(0, &cancel)
        .unwrap_or_else(|e| panic!("side On generates (P6.3): {e}"));
    let result = s.result_at(0).expect("a result");
    let cuts: Vec<_> = result
        .toolpath()
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::FinishingCut)
        .map(|m| m.target)
        .collect();
    assert!(!cuts.is_empty(), "side On cuts");
    let (x, y) = (
        common::op_fixtures::OUTLINE_HALF_X,
        common::op_fixtures::OUTLINE_HALF_Y,
    );
    for p in cuts {
        let on_x = (p.x.abs() - x).abs() < 1e-3 && p.y.abs() <= y + 1e-3;
        let on_y = (p.y.abs() - y).abs() < 1e-3 && p.x.abs() <= x + 1e-3;
        assert!(on_x || on_y, "a cut point is off the outline: {p:?}");
    }
}
