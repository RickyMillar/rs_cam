//! WP21 sentry — the feed-optimisation clamp never panics when the derived
//! floor exceeds the dressup's own ceiling.
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`,
//! row WP21.
//!
//! # The defect
//!
//! `compute::execute::apply_dressups` builds the feed-optimisation
//! parameters. It reads the ceiling from the operator's own dial,
//! `DressupConfig::feed_max_rate`, and it DERIVES the floor as half the
//! operation's nominal feed rate. The two dials are independent, so a
//! nominal feed above twice the ceiling puts the floor above the ceiling.
//!
//! `feedopt::optimize_feed_rates_inner` then handed that inverted pair to
//! `f64::clamp`, and `clamp` panics when `min > max`. The pass is default
//! ON for a pocket, so the panic reached the operator.
//!
//! # The observed shape
//!
//! `tool_load::optimize::retarget_reconciliation_a8` commands 12 000 mm/min
//! (`BASELINE_FEED_MM_MIN`) against the default ceiling of 3 000 mm/min.
//! Both of its arms panic with `min > max, or either was NaN. min = 6000.0,
//! max = 3000.0`. That fixture raises its MACHINE feed ceiling to 24 000
//! mm/min, which proves the binding number is the dressup dial and not the
//! machine profile.
//!
//! # The two arms
//!
//! * **(a) The production door.** A fresh-stock pocket whose commanded feed
//!   is four times the dressup ceiling generates through `execute_job`. The
//!   arm measures that the job completes and that no cutting move carries a
//!   feed above the ceiling.
//! * **(b) The pass itself.** `optimize_feed_rates` takes a hand-built
//!   inverted pair over a full block of material. The arm measures that
//!   every emitted cutting feed sits inside the capped range. Arm (a)
//!   covers the construction site; arm (b) covers the clamp site, so a
//!   later inversion from any other caller stays caught.
//!
//! # Non-vacuity
//!
//! The air-cut branch of the pass never reaches the clamp. The pre-fix red
//! is the `clamp` panic itself, so the red output proves both arms drive
//! the ENGAGED branch. Arm (a) also asserts that the dial is on and that
//! the toolpath holds cutting motion.
//!
//! # NOT MEASURED
//!
//! * A sane pair. `feedopt`'s own `test_optimize_full_engagement_gets_nominal`
//!   measures that case, and this file leaves it there.
//! * The floor on arm (a). The boundary clip re-emits a re-entry descent at
//!   the operation's plunge rate (G-BOUNDARYPLUNGE), which sits below the
//!   capped floor for reasons that have nothing to do with WP21. Arm (b)
//!   measures the floor on the pass's own output instead.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::feedopt::{FeedOptParams, optimize_feed_rates};
use rs_cam_core::geo::P3;
use rs_cam_core::session::{
    GenObserver, GenerateToolpathArgs, GenerateToolpathHandle, Job, JobHandle, ProjectSession,
    ToolpathComputeResult, execute_job,
};
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::{MoveType, Toolpath};
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

mod common;
use common::session::{polygon_model, single_op_session_with, square_polygon, stock_under};
use common::tools::endmill_tool_config;

/// Half-extent (mm) of the square the pocket clears. WP18's pocket fixture,
/// which is measured to run the feed-optimisation pass.
const HALF: f64 = 20.0;

/// Stock height (mm). `stock_under` hangs the board below `z = 0`, the frame
/// a 2D operation cuts in.
const STOCK_Z: f64 = 6.0;

/// Cutter diameter (mm).
const TOOL_D: f64 = 6.0;

/// How far above the dressup ceiling the commanded feed sits.
///
/// The floor is half the nominal feed, so any multiple above two inverts the
/// pair. The value is read off the dial rather than written down, so a later
/// change to the dial's default keeps this arm exercising the inversion.
const FEED_MULTIPLE_OF_CEILING: f64 = 4.0;

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
/// commanded feed sits above twice the dressup ceiling.
fn inverted_pocket_session() -> ProjectSession {
    single_op_session_with(
        stock_under(HALF, STOCK_Z),
        endmill_tool_config(TOOL_D),
        polygon_model(vec![square_polygon(HALF)], "square"),
        "Pocket",
        pocket_op(),
        |cfg| {
            let ceiling = cfg.dressups.feed_max_rate;
            cfg.operation
                .set_feed_rate(FEED_MULTIPLE_OF_CEILING * ceiling);
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

/// Every cutting move's feed rate, in mm/min.
fn cutting_feeds(toolpath: &Toolpath) -> Vec<f64> {
    toolpath
        .moves
        .iter()
        .filter(|mv| mv.move_type.is_cutting())
        .filter_map(|mv| mv.move_type.feed_rate())
        .collect()
}

// ── (a) the production door ─────────────────────────────────────────

/// A pocket commanded above twice the dressup ceiling generates, and every
/// cutting move stays at or below that ceiling.
///
/// Pre-WP21 this call panicked inside `f64::clamp`, because the derived
/// floor (half of 12 000 mm/min) sat above the ceiling (3 000 mm/min).
#[test]
fn a_pocket_above_twice_the_ceiling_generates_without_a_panic() {
    let mut session = inverted_pocket_session();
    let cfg = session
        .toolpath_configs()
        .first()
        .expect("the fixture holds one toolpath")
        .clone();
    let ceiling = cfg.dressups.feed_max_rate;
    let commanded = cfg.operation.feed_rate();
    assert!(
        cfg.dressups.feed_optimization,
        "the pass must be on, or this arm measures a dressup that never ran"
    );
    assert!(
        commanded > 2.0 * ceiling,
        "the fixture must invert the pair: the commanded feed is {commanded} \
         mm/min and the dressup ceiling is {ceiling} mm/min, so the derived \
         floor of {} mm/min does not exceed it",
        commanded * 0.5
    );

    let result = run_job(&mut session, 0);
    let feeds = cutting_feeds(result.toolpath());
    assert!(
        !feeds.is_empty(),
        "the fixture generated no cutting motion, so the ceiling assertion \
         below is vacuous"
    );
    for feed in &feeds {
        assert!(
            *feed <= ceiling,
            "the dressup ceiling is the hard limit: a cutting move carries \
             {feed} mm/min against a ceiling of {ceiling} mm/min"
        );
    }
}

// ── (b) the pass itself ─────────────────────────────────────────────

/// The pass takes an inverted pair and emits feeds inside the capped range.
///
/// The parameters are built the way `apply_dressups` builds them: the
/// ceiling is the dial, and the floor is half the nominal feed. The capped
/// floor is the ceiling, so every emitted cutting feed reads the ceiling.
#[test]
fn the_pass_caps_the_floor_at_the_ceiling_instead_of_panicking() {
    let tool = FlatEndmill::new(10.0, 25.0);
    let ceiling = 3000.0;
    let nominal = FEED_MULTIPLE_OF_CEILING * ceiling;
    let params = FeedOptParams {
        nominal_feed_rate: nominal,
        max_feed_rate: ceiling,
        min_feed_rate: nominal * 0.5,
        ramp_rate: 500.0,
        air_cut_threshold: 0.05,
        // WP22: this arm measures the WP21 clamp, not the WP22 plunge cap.
        // The fixture's vertical descent must stay uncapped, or the floor
        // assertion below reads the plunge rate instead of the ceiling.
        plunge_rate_mm_min: None,
    };
    assert!(
        params.min_feed_rate > params.max_feed_rate,
        "the arm measures an inverted pair; it must build one"
    );

    // A full block of material, so the engaged branch of the pass runs.
    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 50.0, 50.0, 0.0, 10.0, 1.0);

    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(10.0, 10.0, 15.0));
    tp.feed_to(P3::new(10.0, 10.0, 5.0), nominal);
    tp.feed_to(P3::new(20.0, 10.0, 5.0), nominal);

    let result =
        optimize_feed_rates(AnnotatedToolpath::new(tp), &tool, &mut stock, &params).toolpath;

    // The floor takes the ceiling's value, so the capped range is one point.
    let capped_floor = params.min_feed_rate.min(params.max_feed_rate);
    let mut cutting = 0;
    for mv in &result.moves {
        let MoveType::Linear { feed_rate } = mv.move_type else {
            continue;
        };
        cutting += 1;
        assert!(
            feed_rate <= ceiling,
            "the ceiling is the hard limit: a move carries {feed_rate} \
             mm/min against {ceiling} mm/min"
        );
        assert!(
            feed_rate >= capped_floor,
            "the floor takes the ceiling's value: a move carries \
             {feed_rate} mm/min against a capped floor of {capped_floor} \
             mm/min"
        );
    }
    assert_eq!(
        cutting, 2,
        "the fixture emits two cutting moves; a different count makes the \
         bounds above vacuous"
    );
}
