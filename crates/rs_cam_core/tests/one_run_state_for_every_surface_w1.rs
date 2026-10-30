//! W1 (toolpath inspector programme, 2026-10-30): one run state per
//! toolpath, `session::toolpath_run_state`, which the GUI header and MCP
//! `list_toolpaths` read.
//!
//! # The claim
//!
//! 1. F1: an op with no tool and no feeds is `NotReady` (it names the
//!    blocking findings and asks for a tool), never `Ready`.
//! 2. F2: a refusal for missing feeds does not survive the edit that sets
//!    the feeds. The refusal belongs to the generation-input revision of
//!    the run; the edit moves the revision.
//! 3. A refusal with no blocking finding is `Failed` while no input
//!    changes, and asks for a retry; an edit clears it.
//! 4. A result is `Done`; an edit after it is `OutOfDate`; a switched-off
//!    op is `Disabled`; a compute in flight is `Generating`.
//! 5. W2b: the run state reads a gate-only list (no load report, no
//!    feeds calculator, no Suggest; the GUI reads it on every repaint).
//!    Its blocking ids equal the refusing findings of the full
//!    `diagnose_toolpath` list.
//! 6. W2b: a rest op with no current simulated snapshot is `Waiting` on the
//!    nearest enabled operation before it, and asks for a Generate (the
//!    plan generates and simulates that operation first). Before W2b the
//!    GUI header said "Waiting on upstream stock" and MCP said `ready`.
//!
//! Red proof: drop the revision check in `current_run_refusal`; claim 2
//! reads `Failed` after the feeds are set. Drop the blocking scan; claim 1
//! reads `Ready`.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test one_run_state_for_every_surface_w1
//! ```

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::session::{
    polygon_model, single_op_session_with, square_polygon, stock_over, toolpath_config,
};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::diagnostics::ids;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::session::{
    Command, ForgetResultArgs, NextAction, ProjectSession, RunState, SetToolpathEnabledArgs,
    SetToolpathParamArgs, ToolpathConfig, next_action, refuses_generation, toolpath_run_state,
};

const FEED: f64 = 1200.0;

/// A 40 mm square Pocket on a 6 mm end mill; `tweak` adjusts the config.
fn session(tweak: impl FnOnce(&mut ToolpathConfig)) -> (ProjectSession, ToolpathId) {
    let s = single_op_session_with(
        stock_over(25.0, 10.0),
        endmill_tool_config(6.0),
        polygon_model(vec![square_polygon(20.0)], "w1"),
        "w1 pocket",
        OperationConfig::Pocket(PocketConfig::default()),
        tweak,
    );
    let id = s.toolpath_configs()[0].id;
    (s, id)
}

fn clear_feeds(cfg: &mut ToolpathConfig) {
    let OperationConfig::Pocket(pocket) = &mut cfg.operation else {
        panic!("the fixture is a Pocket");
    };
    pocket.feed_rate = None;
    pocket.plunge_rate = None;
    pocket.spindle_rpm = None;
}

fn state(s: &ProjectSession, id: ToolpathId) -> RunState {
    toolpath_run_state(s, id, false).expect("the toolpath exists")
}

fn set_param(s: &mut ProjectSession, param: &str, value: serde_json::Value) {
    let _ = s
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: param.to_owned(),
            value,
        }))
        .expect("the param applies");
}

#[test]
fn no_tool_and_no_feeds_is_not_ready_f1() {
    let (s, id) = session(|cfg| {
        cfg.tool_id = None;
        clear_feeds(cfg);
    });
    let RunState::NotReady { blocking } = state(&s, id) else {
        panic!(
            "an op with no tool and no feeds is not ready: {:?}",
            state(&s, id)
        );
    };
    let names: Vec<&str> = blocking.iter().map(|b| b.as_str()).collect();
    assert!(names.contains(&ids::PRECOND_NO_TOOL), "{names:?}");
    assert!(names.contains(&ids::PRECOND_NOT_SET), "{names:?}");
    assert_eq!(
        next_action(&state(&s, id)),
        NextAction::ChooseTool,
        "the tool comes first"
    );
}

#[test]
fn a_feeds_refusal_clears_when_the_feeds_are_set_f2() {
    let (mut s, id) = session(clear_feeds);
    let RunState::NotReady { blocking } = state(&s, id) else {
        panic!("unset feeds: not ready, got {:?}", state(&s, id));
    };
    assert_eq!(
        blocking.iter().map(|b| b.as_str()).collect::<Vec<_>>(),
        vec![ids::PRECOND_NOT_SET]
    );
    assert_eq!(next_action(&state(&s, id)), NextAction::FixInputs);

    // The run refuses, and the session records the refusal.
    let cancel = AtomicBool::new(false);
    let refusal = s
        .generate_toolpath(0, &cancel)
        .map(|_| ())
        .expect_err("unset feeds refuse generation");
    assert_eq!(
        s.current_run_refusal(id),
        Some(refusal.to_string().as_str()),
        "the core door records its refusal"
    );

    // The operator sets the feeds. The refusal answered the old inputs.
    set_param(&mut s, "feed_rate", serde_json::json!(FEED));
    set_param(&mut s, "plunge_rate", serde_json::json!(FEED / 2.0));
    set_param(&mut s, "spindle_rpm", serde_json::json!(18_000));
    assert_eq!(
        s.current_run_refusal(id),
        None,
        "the edit clears the refusal"
    );
    assert_eq!(state(&s, id), RunState::Ready);
    assert_eq!(next_action(&state(&s, id)), NextAction::Generate);
}

#[test]
fn a_refusal_is_failed_until_an_input_changes() {
    let (mut s, id) = session(|_| {});
    assert_eq!(state(&s, id), RunState::Ready);
    // The GUI drain records a refusal the diagnostics cannot see.
    let _ = s
        .apply(Command::ForgetResult(ForgetResultArgs {
            index: 0,
            refusal: Some("the worker refused".to_owned()),
        }))
        .expect("index 0 exists");
    assert_eq!(
        state(&s, id),
        RunState::Failed {
            reason: "the worker refused".to_owned()
        }
    );
    assert_eq!(next_action(&state(&s, id)), NextAction::Retry);
    // A cancel records no refusal.
    let _ = s
        .apply(Command::ForgetResult(ForgetResultArgs {
            index: 0,
            refusal: None,
        }))
        .expect("index 0 exists");
    assert_eq!(state(&s, id), RunState::Ready);
    let _ = s
        .apply(Command::ForgetResult(ForgetResultArgs {
            index: 0,
            refusal: Some("again".to_owned()),
        }))
        .expect("index 0 exists");
    set_param(&mut s, "feed_rate", serde_json::json!(FEED + 10.0));
    assert_eq!(state(&s, id), RunState::Ready, "an edit clears the refusal");
}

#[test]
fn done_out_of_date_disabled_and_generating() {
    let (mut s, id) = session(|_| {});
    assert_eq!(
        toolpath_run_state(&s, id, true),
        Some(RunState::Generating),
        "the caller's flag"
    );
    let cancel = AtomicBool::new(false);
    let _ = s
        .generate_toolpath(0, &cancel)
        .expect("the Pocket generates");
    assert_eq!(state(&s, id), RunState::Done);
    assert_eq!(next_action(&state(&s, id)), NextAction::None);

    set_param(&mut s, "feed_rate", serde_json::json!(FEED + 10.0));
    assert_eq!(state(&s, id), RunState::OutOfDate);
    assert_eq!(next_action(&state(&s, id)), NextAction::Regenerate);

    let _ = s
        .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
            index: 0,
            enabled: false,
        }))
        .expect("index 0 exists");
    assert_eq!(state(&s, id), RunState::Disabled);
    assert_eq!(
        toolpath_run_state(&s, ToolpathId::from_raw(9_999), false),
        None,
        "an unknown id has no state"
    );

    // The wire form: one tagged object.
    let json = serde_json::to_value(RunState::NotReady {
        blocking: vec![ids::PRECOND_NO_TOOL.into()],
    })
    .expect("serialise");
    assert_eq!(
        json,
        serde_json::json!({"state": "not_ready", "blocking": ["precondition.no_tool"]})
    );
    assert_eq!(
        serde_json::to_value(NextAction::ChooseTool).expect("serialise"),
        serde_json::json!("choose_tool")
    );
}

#[test]
fn a_rest_op_with_no_snapshot_is_waiting_w2b() {
    // Alone, the rest op has nothing before it: it waits on no operation.
    let (s, id) = session(|cfg| cfg.stock_source = StockSource::FromRemainingStock);
    assert_eq!(state(&s, id), RunState::Waiting { on: Vec::new() });
    assert_eq!(next_action(&state(&s, id)), NextAction::Generate);

    // After a rough, it waits on the rough.
    let (mut s, rough) = session(|_| {});
    let tool = s.tools()[0].id;
    let model = s.models()[0].id;
    let mut rest = toolpath_config(
        "w1 rest",
        OperationConfig::Pocket(PocketConfig::default()),
        tool,
        model,
    );
    rest.stock_source = StockSource::FromRemainingStock;
    let _ = s
        .apply(Command::AddToolpath(
            rs_cam_core::session::AddToolpathArgs {
                setup_index: 0,
                config: Box::new(rest),
            },
        ))
        .expect("the rest op adds");
    let rest_id = s.toolpath_configs()[1].id;
    assert_eq!(state(&s, rest_id), RunState::Waiting { on: vec![rough] });
    assert_eq!(
        serde_json::to_value(state(&s, rest_id)).expect("serialise"),
        serde_json::json!({"state": "waiting", "on": [rough]}),
        "the wire form names the operation it waits on"
    );
    // A switched-off rough is not the stock it waits on.
    let _ = s
        .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
            index: 0,
            enabled: false,
        }))
        .expect("index 0 exists");
    assert_eq!(state(&s, rest_id), RunState::Waiting { on: Vec::new() });
}

/// A change to the fixture config.
type Tweak = fn(&mut ToolpathConfig);

/// Claim 5: the blocking ids of the run state are the refusing findings of
/// the full list, in each case of this file.
#[test]
fn the_gate_list_blocks_as_the_full_list_w2b() {
    let cases: [(&str, Tweak); 4] = [
        ("clean", |_| {}),
        ("no feeds", clear_feeds),
        ("no tool", |cfg| cfg.tool_id = None),
        ("no tool, no feeds", |cfg| {
            cfg.tool_id = None;
            clear_feeds(cfg);
        }),
    ];
    let mut blocked = 0_usize;
    for (name, tweak) in cases {
        let (s, id) = session(tweak);
        let full: Vec<String> = s
            .diagnose_toolpath(0)
            .expect("the list builds")
            .iter()
            .filter(|d| refuses_generation(d))
            .map(|d| d.id.as_str().to_owned())
            .collect();
        let gate: Vec<String> = match state(&s, id) {
            RunState::NotReady { blocking } => {
                blocking.iter().map(|b| b.as_str().to_owned()).collect()
            }
            _ => Vec::new(),
        };
        assert_eq!(
            gate, full,
            "{name}: the gate list and the full list disagree"
        );
        blocked += usize::from(!gate.is_empty());
    }
    assert!(blocked >= 3, "non-vacuity: {blocked} blocked cases");
}
