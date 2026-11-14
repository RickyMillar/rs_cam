//! Review 2026-11-15 F4 (lead ruling): with no current measured verdict for
//! an op, the export warns for EVERY stated row with the gating tone,
//! whatever the limit: one warning per op, `export.stated_cut_over_limit`,
//! that names the worst row (`CheckRow::rank_cmp`) and the count of the
//! others. A warning, never a refusal.
//!
//! The fixture is a Pocket with plunge entries whose stated plunge feed is
//! far over the vertical-rate limit, and whose stated cut feed keeps the
//! chip inside its band: before F4 the export warned only for a chip
//! maximum row, so this op had no "over a limit" line.
//!
//! Review R8 (lead ruling): the vertical rate has no sample gate (row
//! `S-NoSampleGate`), so after a simulation that is fresh for the op the
//! same stated row still warns, with the detail of a fresh simulation.
//!
//! `cargo test -p rs_cam_core -q --test a_stated_cut_over_any_limit_warns_review_f4`

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::DressupEntryStyle;
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern};
use rs_cam_core::diagnostics::ids;
use rs_cam_core::limits::{LimitId, Tone};
use rs_cam_core::session::{ProjectSession, SimulationOptions, SimulationResolution};

mod common;
use common::kind_fixtures::{FLAT_CELL, TOOL_D};
use common::op_fixtures::{flat_stock, outline_model};
use common::session::single_op_session_with;
use common::tools::endmill_tool_config;

/// A stated plunge feed far over any vertical-rate limit of the fixture
/// tool (the machine Z axis rate of the default profile is lower).
const FAST_PLUNGE: f64 = 9000.0;

fn fast_plunge_pocket() -> ProjectSession {
    let op = OperationConfig::Pocket(PocketConfig {
        pattern: PocketPattern::Contour,
        ..PocketConfig::default()
    });
    let mut session = single_op_session_with(
        flat_stock(),
        endmill_tool_config(TOOL_D),
        outline_model(),
        "pocket",
        op,
        |tc| {
            tc.dressups.entry_style = DressupEntryStyle::None;
            tc.dressups.feed_optimization = false;
            tc.operation.set_plunge_rate(FAST_PLUNGE);
        },
    );
    session
        .generate_toolpath(0, &AtomicBool::new(false))
        .expect("the op generates");
    session
}

#[test]
fn a_stated_vertical_rate_over_its_limit_warns_with_no_simulation_review_f4() {
    let session = fast_plunge_pocket();
    let rows = session
        .limit_rows(0, None)
        .expect("the op")
        .envelope()
        .expect("a tool")
        .expect("the envelope builds");
    let gating: Vec<_> = rows
        .iter()
        .filter(|r| r.is_stated() && r.tone() == Tone::Exceeds)
        .collect();
    let faces: Vec<&str> = gating.iter().map(|r| r.face()).collect();
    // Non-vacuity: a vertical-rate row gates, and no chip-maximum row does.
    assert!(
        gating
            .iter()
            .any(|r| r.check().limit == LimitId::PlungeRate),
        "a stated plunge-rate row gates: {faces:?}"
    );
    assert!(
        gating.iter().all(|r| r.check().limit != LimitId::ChipMax),
        "no chip-maximum row gates: {faces:?}"
    );
    let exported = session
        .export_gcode_checked(None)
        .expect("a warning never refuses the export");
    let warnings: Vec<_> = exported
        .warnings
        .iter()
        .filter(|w| w.id == ids::EXPORT_STATED_CUT_OVER_LIMIT)
        .collect();
    assert_eq!(warnings.len(), 1, "one warning per op: {warnings:?}");
    let detail = &warnings.first().expect("one warning").detail;
    assert!(detail.contains("plunge rate"), "{detail}");
    assert!(
        detail.contains("no current simulation measured this op"),
        "{detail}"
    );
    let others = gating.len() - 1;
    match others {
        0 => assert!(!detail.contains("more stated row"), "{detail}"),
        1 => assert!(
            detail.contains("; 1 more stated row over a limit"),
            "{detail}"
        ),
        n => assert!(
            detail.contains(&format!("; {n} more stated rows over a limit")),
            "{detail}"
        ),
    }
    assert!(
        exported
            .gcode
            .lines()
            .any(|l| l.contains("WARNING") && l.contains(ids::EXPORT_STATED_CUT_OVER_LIMIT)),
        "the header names the warning"
    );
}

/// Review R8, the reviewer's case: the F4 fixture after a simulation with
/// modulation off. The simulation is fresh for the op, and no sample gate
/// judges the vertical rate, so the stated plunge-rate row warns. Before
/// R8 this export had no warning.
#[test]
fn a_stated_vertical_rate_over_its_limit_warns_after_a_simulation_review_r8() {
    let mut session = fast_plunge_pocket();
    session
        .run_simulation(
            &SimulationOptions {
                resolution: SimulationResolution::Fixed(FLAT_CELL),
                adaptive_feed_modulation: false,
                ..SimulationOptions::default()
            },
            &AtomicBool::new(false),
        )
        .expect("the session simulates");
    let trace = session
        .simulation_result()
        .and_then(|s| s.cut_trace.clone())
        .expect("the simulation holds a cut trace");
    assert!(
        session.sim_trace_is_fresh_for(&trace, 0),
        "non-vacuity: the simulation is fresh for the op"
    );
    let exported = session
        .export_gcode_checked(Some(&trace))
        .expect("a warning never refuses the export");
    let details: Vec<&str> = exported
        .warnings
        .iter()
        .filter(|w| w.id == ids::EXPORT_STATED_CUT_OVER_LIMIT)
        .map(|w| w.detail.as_str())
        .collect();
    assert_eq!(
        details,
        vec![
            "Toolpath 'pocket': plunge entry · plunge rate 294 % over limit; no sample gate \
             of the simulation judges this limit"
        ]
    );
    assert!(
        exported.gcode.lines().any(|l| l.contains(
            "WARNING Tool export.stated_cut_over_limit pocket: Stated cut over a limit: no \
             measured gate"
        )),
        "the header names the warning"
    );
}
