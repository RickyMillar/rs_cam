//! G-PENCILHOP — the pencil's clearance-hop cap is an OPERATOR dial, and it
//! moves only the hop tier.
//!
//! # The defect
//!
//! `PencilParams::link_hop_distance_mm` split the pencil's two link tiers —
//! the at-depth join that removes the next fragment's ENTRY, and the
//! clearance hop that lifts clear of standing material and descends again.
//! `compute/execute.rs` then hardcoded `None`. There was no `PencilConfig`
//! field and no `ParamDef`, so no project file, no GUI and no MCP
//! `set_toolpath_param` call could reach the split. The measurement that
//! decides whether the two tiers deserve two numbers — the pencil is the
//! one family where the at-depth tier fires at all — could not be run.
//!
//! # What `None` is NOT
//!
//! `None` does not switch the hop tier off, and a reader who assumes it
//! does will "fix" a working feature. `hop_cap` falls back to
//! `hookup_distance`, and the earlier `gap > hookup_distance` test already
//! returned, so the `gap > hop_cap` test cannot run at `None`. An earlier
//! ledger row called this a shipped regression and was RETRACTED
//! (`0b5f1cd2`); the hop tier itself predates the link stage
//! (`git log -S "PencilJunction::Lifted"` reaches `fb5339da`). This file
//! pins the retraction: `None` and `Some(hookup_distance)` emit the same
//! toolpath, move for move.
//!
//! # What the dial buys
//!
//! `Some(0.0)` refuses every hop and leaves the at-depth tier alone. That
//! is the control arm: it separates what each tier is worth on one fixture,
//! which no other arrangement of the shipped dials can do.
//!
//! The refusal itself is pinned in the crate, not here:
//! `pencil::tests::a_zero_hop_cap_refuses_a_lifted_link_and_keeps_the_at_depth_tier`.
//! A hop needs STANDING MATERIAL to clear, so it needs a machined input
//! stock; a single-op session over fresh stock hands the emitter
//! `entry_stock: None`, which cannot reach `plan_link_lift` at all and joins
//! every junction at depth. This file pins what the crate arm cannot see:
//! that the dial reaches the emitter from a project file and from MCP.
//!
//! # No default moved
//!
//! `PencilConfig::default()` carries `None`, so every saved project loads
//! and emits exactly what it emitted before.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

mod common;

use common::meshes::sawtooth_plate;
use common::session::{generate, mesh_model, pinned_heights, single_op_session_with, stock_over};
use common::tools::ball_tool_config;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::operation_configs::PencilConfig;
use rs_cam_core::session::ProjectSession;

/// The corrugated plate `link_counters_visible_g_linkvisible` uses for its
/// pencil arm — many short cut runs whose ends nearly touch, which is the
/// topology a link stage exists for.
const HALF_MM: f64 = 20.0;
const PERIOD_MM: f64 = 6.0;
const AMPLITUDE_MM: f64 = 2.0;
const BALL_DIAMETER_MM: f64 = 3.0;
/// One period plus slack, so the run-to-run gap sits inside the at-depth cap.
const HOOKUP_MM: f64 = PERIOD_MM * 1.5;

fn pencil_config(hop_cap: Option<f64>) -> OperationConfig {
    OperationConfig::Pencil(PencilConfig {
        hookup_distance: HOOKUP_MM,
        link_hop_distance_mm: hop_cap,
        ..PencilConfig::default()
    })
}

fn pencil_session(hop_cap: Option<f64>) -> ProjectSession {
    let mut session = single_op_session_with(
        stock_over(HALF_MM, 6.0),
        ball_tool_config(BALL_DIAMETER_MM),
        mesh_model(
            sawtooth_plate(HALF_MM, PERIOD_MM, AMPLITUDE_MM),
            "corrugated",
        ),
        "Pencil",
        pencil_config(hop_cap),
        |cfg| cfg.heights = pinned_heights(2.0, -2.0),
    );
    generate(&mut session, 0);
    session
}

fn move_targets(session: &ProjectSession, arm: &str) -> Vec<(f64, f64, f64)> {
    session
        .get_result(0)
        .unwrap_or_else(|| panic!("{arm}: the pencil produced no result"))
        .toolpath()
        .moves
        .iter()
        .map(|m| (m.target.x, m.target.y, m.target.z))
        .collect()
}

/// The retraction of the G-PENCILHOP defect claim, pinned as motion.
///
/// `None` is "the same cap as `hookup_distance`". If a later reader makes
/// `None` mean OFF, this fails.
#[test]
fn an_unset_cap_emits_the_same_toolpath_as_the_at_depth_cap() {
    let unset = move_targets(&pencil_session(None), "unset");
    let explicit = move_targets(&pencil_session(Some(HOOKUP_MM)), "explicit");

    assert!(
        !unset.is_empty(),
        "population: the unset arm emitted no move at all"
    );
    assert_eq!(
        unset.len(),
        explicit.len(),
        "`None` must mean the same cap as `hookup_distance`: the two arms \
         emitted {} and {} moves",
        unset.len(),
        explicit.len()
    );
    assert_eq!(
        unset, explicit,
        "`None` must mean the same cap as `hookup_distance`, move for move"
    );
}

/// The dial has to be reachable from the surfaces an operator and an agent
/// actually use. This is what the hardcoded `None` denied.
#[test]
fn the_dial_is_reachable_from_set_toolpath_param() {
    let mut session = single_op_session_with(
        stock_over(HALF_MM, 6.0),
        ball_tool_config(BALL_DIAMETER_MM),
        mesh_model(
            sawtooth_plate(HALF_MM, PERIOD_MM, AMPLITUDE_MM),
            "corrugated",
        ),
        "Pencil",
        pencil_config(None),
        |cfg| cfg.heights = pinned_heights(2.0, -2.0),
    );

    assert!(
        OperationType::Pencil
            .registry_entry()
            .param_defs
            .iter()
            .any(|d| d.name == "link_hop_distance_mm"),
        "the registry must publish the dial, or `get_operation_schema` and \
         the wildcard set path never see the name"
    );

    session
        .set_toolpath_param(0, "link_hop_distance_mm", serde_json::json!(0.0))
        .expect("MCP must accept the dial by name");
    assert_eq!(read_hop_cap(&session), Some(0.0));

    session
        .set_toolpath_param(0, "link_hop_distance_mm", serde_json::json!(null))
        .expect("null must reset the dial");
    assert_eq!(
        read_hop_cap(&session),
        None,
        "null must restore the shipped one-cap behaviour"
    );
}

/// The saved default did not move — every project file loads unchanged.
#[test]
fn the_shipped_default_is_unset() {
    assert_eq!(PencilConfig::default().link_hop_distance_mm, None);
}

fn read_hop_cap(session: &ProjectSession) -> Option<f64> {
    match &session.toolpath_configs()[0].operation {
        OperationConfig::Pencil(cfg) => cfg.link_hop_distance_mm,
        other => panic!("expected a pencil op, found {other:?}"),
    }
}
