//! WP22 sentry (G-FEEDOPTPLUNGE) — the feed-optimisation pass caps a
//! GEOMETRIC plunge at the operation's own plunge rate.
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
//! §27 ruling 3, row WP22.
//!
//! # The defect
//!
//! `feedopt::optimize_feed_rates_inner` writes `nominal_feed_rate * factor`
//! onto EVERY cutting move. It reads no move's own feed and no move's own
//! geometry. A vertical entry descent reads full engagement at its foot, so
//! the factor is about 1.0 and the pass writes the operation's CUTTING feed
//! onto a move the generator commanded at the PLUNGE rate.
//!
//! # The observed shape
//!
//! The shipped pocket defaults command 1000 mm/min of cutting feed against a
//! 500 mm/min plunge rate (`compute/operation_configs.rs`), so the engaged
//! arm lifts a descent by about 2.00x. A descent that reads air takes the
//! other arm and gets the dressup ceiling, 3000 mm/min by default, which is
//! about 6.00x. The exact count and the exact ratio on this fixture are NOT
//! MEASURED here; the red output records them.
//!
//! `feed_modulation::adaptive_feed_modulate` carries the same guard already
//! (P3, 2026-09-07). Both guards read one classifier,
//! `kinematic_utilization::classify_move`, so they cannot disagree about
//! what a plunge is. WP22 adds a second CALLER of that classifier, never a
//! second rule.
//!
//! # The three arms
//!
//! * **(a) The production door.** A fresh-stock pocket generates through
//!   `execute_job` with the dial ON. The arm measures that no move the
//!   classifier calls `Plunge` carries a feed above the operation's plunge
//!   rate.
//! * **(b) The pass itself.** `optimize_feed_rates` takes a hand-built
//!   toolpath and the new `plunge_rate_mm_min` field. The arm measures the
//!   cap, the control with the field absent, and the air arm. It ships in
//!   the FIX commit, because it names a field the pre-fix code has not got
//!   and a compile-red sentry hides the assertion-red arm (a) gives.
//! * **(c) The control.** The same fixture with the dial OFF. It proves arm
//!   (a) measures the PASS and not the generator.
//!
//! # Non-vacuity
//!
//! Arm (a) asserts that the dial is on and that the plunge population is not
//! empty. It reports the over-rate count and the worst ratio in the failure
//! message rather than asserting them, so the arm reads the same on both
//! sides of the fix.
//!
//! The fixture sets `entry_style = DressupEntryStyle::None`. The shipped
//! pocket dressups carry a 3-degree ramp entry, and a 3-degree descent is
//! `MotionClass::Ramp`, not `MotionClass::Plunge`. A ramped pocket may hold
//! no plunge at all, so this fixture takes the entry style whose descents
//! the classifier calls `Plunge`.
//!
//! # Red before the fix
//!
//! Arm (a) is assertion-red. Arm (c) passes on both sides of the fix.
//!
//! # NOT MEASURED
//!
//! * The plunge count and the worst ratio on this fixture. This lane ran no
//!   cargo.
//! * Whether the SHIPPED (ramped) pocket holds any plunge at all. The
//!   fixture moves that dial rather than answering the question.
//! * The `LeadIn` and `LeadOut` population. Those arcs are lateral, so the
//!   geometric cap does not reach them and the pass still overwrites their
//!   operator-tuned feeds. That is a side finding, not WP22 work.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::DressupEntryStyle;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::geo::P3;
use rs_cam_core::kinematic_utilization::{MotionClass, classify_move};
use rs_cam_core::session::{
    GenObserver, GenerateToolpathArgs, GenerateToolpathHandle, Job, JobHandle, ProjectSession,
    ToolpathComputeResult, execute_job,
};
use rs_cam_core::toolpath::Toolpath;

mod common;
use common::session::{polygon_model, single_op_session_with, square_polygon, stock_under};
use common::tools::endmill_tool_config;

/// Half-extent (mm) of the square the pocket clears. The WP18 and WP21
/// pocket fixture, which is measured to run the feed-optimisation pass.
const HALF: f64 = 20.0;

/// Stock height (mm). `stock_under` hangs the board below `z = 0`, the frame
/// a 2D operation cuts in.
const STOCK_Z: f64 = 6.0;

/// Cutter diameter (mm).
const TOOL_D: f64 = 6.0;

// ── Fixture ─────────────────────────────────────────────────────────

fn pocket_op() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: 2.0,
        depth: 3.0,
        depth_per_pass: 1.5,
        ..PocketConfig::default()
    })
}

/// One stock, one tool, one model, one ungenerated fresh-stock pocket whose
/// entry descents are vertical.
///
/// `feed_optimization` writes the dressup dial. `DressupConfig::for_op`
/// leaves it ON, so the `true` arm is the shipped configuration.
fn plunging_pocket_session(feed_optimization: bool) -> ProjectSession {
    single_op_session_with(
        stock_under(HALF, STOCK_Z),
        endmill_tool_config(TOOL_D),
        polygon_model(vec![square_polygon(HALF)], "square"),
        "Pocket",
        pocket_op(),
        |cfg| {
            // A vertical entry descent is what the classifier calls
            // `Plunge`. The shipped ramp entry is `MotionClass::Ramp`.
            cfg.dressups.entry_style = DressupEntryStyle::None;
            cfg.dressups.feed_optimization = feed_optimization;
        },
    )
}

/// Capture the generation inputs, then generate — the two production steps.
fn run_job(session: &mut ProjectSession, index: usize) -> ToolpathComputeResult {
    let cancel = AtomicBool::new(false);
    let JobHandle::GenerateToolpath(handle) = session
        .start(
            Job::GenerateToolpath(GenerateToolpathArgs { index }),
            &cancel,
        )
        .expect("step (i) captures the generation inputs")
    else {
        panic!("the generate_toolpath row answers its own handle variant");
    };
    let handle: GenerateToolpathHandle = *handle;
    let cancel = AtomicBool::new(false);
    execute_job(&handle, &GenObserver::none(), &cancel).expect("step (ii) generates the toolpath")
}

/// Every cutting move the classifier calls `Plunge`, as `(index, feed)`.
///
/// The classifier is the one the guard calls, so the test and the guard
/// cannot disagree about what a plunge is. NOTE: `pocket.rs` identifies a
/// plunge by `feed == plunge_rate`. Do not copy that here — the shipped
/// `link_feed_rate` and `PocketConfig::plunge_rate` are both 500.0, so a
/// feed-value test cannot tell a link move from a plunge.
fn plunges(toolpath: &Toolpath) -> Vec<(usize, f64)> {
    let mut out = Vec::new();
    let mut prev: Option<P3> = None;
    for (i, mv) in toolpath.moves.iter().enumerate() {
        if let Some(p) = prev {
            let delta = [mv.target.x - p.x, mv.target.y - p.y, mv.target.z - p.z];
            if classify_move(mv.move_type, delta) == MotionClass::Plunge
                && let Some(feed) = mv.move_type.feed_rate()
            {
                out.push((i, feed));
            }
        }
        prev = Some(mv.target);
    }
    out
}

// ── (a) the production door ─────────────────────────────────────────

/// With the dial ON, no geometric plunge leaves the pass above the
/// operation's plunge rate.
///
/// Pre-WP22 the pass wrote `nominal * factor` onto every cutting move, so
/// each vertical descent came back at about twice its plunge rate.
#[test]
fn a_generated_pocket_holds_no_plunge_above_the_plunge_rate() {
    let mut session = plunging_pocket_session(true);
    let cfg = session
        .toolpath_configs()
        .first()
        .expect("the fixture holds one toolpath")
        .clone();
    assert!(
        cfg.dressups.feed_optimization,
        "the pass must be on, or this arm measures a dressup that never ran"
    );
    let plunge_rate = cfg.operation.plunge_rate();
    assert!(
        plunge_rate > 0.0,
        "the operation must carry a plunge rate; this arm grades against it"
    );

    let result = run_job(&mut session, 0);
    let found = plunges(result.toolpath());
    assert!(
        !found.is_empty(),
        "no move is classified Plunge, so the cap assertion below measures \
         nothing. The fixture must emit vertical entry descents"
    );

    // Reported, never asserted: the defect shape. An assertion here would
    // fail once the fix lands, so the red output carries the evidence.
    let over = found.iter().filter(|(_, f)| *f > plunge_rate).count();
    let ratios = found.iter().map(|(_, f)| f / plunge_rate);
    let max_ratio = ratios.fold(0.0_f64, f64::max);
    let total = found.len();

    // The cap is exact, so the bar carries no tolerance.
    for (i, feed) in &found {
        assert!(
            *feed <= plunge_rate,
            "G-FEEDOPTPLUNGE: {over} of {total} plunges exceed the plunge \
             rate; worst ratio {max_ratio:.2}x. Move {i} carries {feed} \
             mm/min against a plunge rate of {plunge_rate} mm/min"
        );
    }
}

// ── (c) the control — the dial OFF ──────────────────────────────────

/// With the dial OFF, the generator's own plunges already sit at or below
/// the operation's plunge rate.
///
/// This arm is what makes arm (a) a measurement of the PASS rather than of
/// the generator. It passes on both sides of the fix.
///
/// The bar is `<=` and not equality. A link descent rides
/// `DressupConfig::link_feed_rate`, which equals the pocket's plunge rate on
/// the shipped defaults by coincidence. The arm pins the coincidence out by
/// asking for ONE exact reading instead of demanding it of every move.
#[test]
fn the_generator_alone_emits_no_plunge_above_the_plunge_rate() {
    let mut session = plunging_pocket_session(false);
    let cfg = session
        .toolpath_configs()
        .first()
        .expect("the fixture holds one toolpath")
        .clone();
    assert!(
        !cfg.dressups.feed_optimization,
        "this arm measures the generator, so the pass must be off"
    );
    let plunge_rate = cfg.operation.plunge_rate();

    let result = run_job(&mut session, 0);
    let found = plunges(result.toolpath());
    assert!(
        !found.is_empty(),
        "no move is classified Plunge with the dial off, so arm (a)'s \
         population is the pass's own invention"
    );

    for (i, feed) in &found {
        assert!(
            *feed <= plunge_rate,
            "the generator commands its entry descents at the plunge rate: \
             move {i} carries {feed} mm/min against {plunge_rate} mm/min"
        );
    }

    let total = found.len();
    let at_rate_exactly = |f: &f64| (f - plunge_rate).abs() < 1e-9;
    let at_rate = found.iter().filter(|(_, f)| at_rate_exactly(f)).count();
    assert!(
        at_rate > 0,
        "the population must hold the generator's own entry descents: none \
         of the {total} plunges reads the plunge rate {plunge_rate} mm/min \
         exactly"
    );
}
