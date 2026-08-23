//! **G-ENTRYEMPTY / G-STICKYEMPTY** — an empty generation must be a typed
//! refusal, and must not poison the operation that produced it.
//!
//! # The two defects
//!
//! Observed live 2026-08-23 on `wanaka200`:
//!
//! 1. **G-ENTRYEMPTY.** An `adaptive3d` back-rough with `entry_style = helix`
//!    generated a **0-move toolpath and reported success**. Nothing gated it
//!    — [`rs_cam_core::compute::config::ToolpathStats::zero_removal`] is
//!    report-only by operator ruling *and* cannot fire on an empty toolpath
//!    at all (it measures engagement of emitted cutting geometry; with no
//!    cutting moves it samples nothing and records nothing). The same gap let
//!    a heights-tab mishap silently empty a healthy operation, and it took
//!    262 downstream rapid collisions and a human to notice.
//! 2. **G-STICKYEMPTY.** Once an operation had generated empty, putting its
//!    parameters back did not bring it back: it kept generating nothing until
//!    the project was reloaded.
//!
//! # What is pinned here
//!
//! | test | pins |
//! |---|---|
//! | `a_pocket_the_tool_fits_cuts_something` | **non-vacuity** — the fixture cuts before anything is asked of the gate |
//! | `a_pocket_whose_offsets_collapse_is_refused_not_reported_as_done` | the refusal, and that it is [`SessionError::GeneratedEmpty`] rather than prose in `OperationFailed` |
//! | `a_refused_generation_leaves_no_cached_result` | the half of the refusal that de-poisons: nothing is cached |
//! | `a_rest_operation_that_finds_nothing_still_succeeds` | the exemption, without which rest chains break |
//! | `an_empty_generation_does_not_stop_the_next_one` | G-STICKYEMPTY on the fresh-stock path: empty → known-good params → cuts again, no reload |
//! | `an_empty_rest_pass_keeps_its_prior_stock_snapshot` | G-STICKYEMPTY's **mechanism**: the phantom-prior-stock scan vs. the short-toolpath filter |
//!
//! # Why a pocket, and why the tool is the dial
//!
//! The emptiness has to be produced by the shipped pipeline, not staged. A
//! contour pocket whose first inward offset collapses returns an **empty
//! toolpath with `Ok`** — `pocket_toolpath_at_levels_reported_with_cancel`
//! maps only cancellation to an error — which is the exact "silent empty
//! success" shape `tests/adversarial_2d_campaign_r2.rs` names as a failure
//! condition and the exact shape the live defect had. Offsetting a 40 mm
//! square inward by a 30 mm tool radius collapses it by construction, so the
//! trigger is arithmetic rather than a tuned fixture; the same square with a
//! Ø6 tool cuts, which is what the non-vacuity test measures.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::session::{
    polygon_model, single_op_session_with, square_polygon, stock_under, toolpath_config,
};
use common::tools::endmill_tool_config;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::operation_configs::{PocketConfig, RestConfig};
use rs_cam_core::session::{ProjectSession, SessionError, SimulationOptions};
use rs_cam_core::toolpath::MoveType;

/// Half-extent (mm) of the square the pocket clears — a 40 × 40 mm region.
const HALF: f64 = 20.0;

/// Stock height (mm). `stock_under` hangs it below `z = 0`, the frame 2D ops
/// cut in.
const STOCK_Z: f64 = 6.0;

/// A tool that comfortably fits the region: radius 3 mm against a 20 mm
/// half-extent.
const FITTING_TOOL_D: f64 = 6.0;

/// A tool that cannot: radius 30 mm against a 20 mm half-extent, so the very
/// first inward offset collapses and the cascade has nothing to emit.
const OVERSIZE_TOOL_D: f64 = 60.0;

/// Simulation cell (mm). Coarse on purpose — these sentries read *whether* a
/// prior-stock snapshot exists, never a measurement taken from it.
const SIM_CELL_MM: f64 = 0.5;

// ── Fixture ─────────────────────────────────────────────────────────────

fn pocket_op() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: 2.0,
        depth: 3.0,
        depth_per_pass: 3.0,
        ..PocketConfig::default()
    })
}

/// One pocket over a 40 mm square, cut by a tool of `tool_d`.
fn pocket_session(tool_d: f64) -> ProjectSession {
    single_op_session_with(
        stock_under(HALF, STOCK_Z),
        endmill_tool_config(tool_d),
        polygon_model(vec![square_polygon(HALF)], "square"),
        "Pocket",
        pocket_op(),
        |_| {},
    )
}

fn generate(session: &mut ProjectSession, index: usize) -> Result<(), SessionError> {
    let cancel = AtomicBool::new(false);
    session.generate_toolpath(index, &cancel).map(|_| ())
}

/// Moves that actually cut — the same population the gate counts.
fn cutting_moves(session: &ProjectSession, index: usize) -> usize {
    session.get_result(index).map_or(0, |r| {
        r.toolpath()
            .moves
            .iter()
            .filter(|m| !matches!(m.move_type, MoveType::Rapid))
            .count()
    })
}

fn simulate(session: &mut ProjectSession) {
    let cancel = AtomicBool::new(false);
    session
        .run_simulation(
            &SimulationOptions {
                resolution: SIM_CELL_MM,
                auto_resolution: false,
                metrics_enabled: false,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("simulation of the fixture");
}

// ── Non-vacuity ─────────────────────────────────────────────────────────

/// The fixture must CUT before it is allowed to gate anything. Without this,
/// every assertion below would be satisfied by a pocket that never worked.
#[test]
fn a_pocket_the_tool_fits_cuts_something() {
    let mut session = pocket_session(FITTING_TOOL_D);
    generate(&mut session, 0).expect("a Ø6 tool clears a 40 mm square");
    assert!(
        cutting_moves(&session, 0) > 0,
        "fixture is vacuous: the Ø{FITTING_TOOL_D} pocket emitted no cutting moves, so the \
         empty-generation tests below would prove nothing"
    );
}

// ── G-ENTRYEMPTY: the refusal ───────────────────────────────────────────

#[test]
fn a_pocket_whose_offsets_collapse_is_refused_not_reported_as_done() {
    let mut session = pocket_session(OVERSIZE_TOOL_D);
    let err = generate(&mut session, 0)
        .expect_err("0 cutting moves from a non-empty region must not report success");

    // The TYPE is the point, not the prose: a consumer must be able to tell
    // this apart from a missing-geometry error and from AwaitingPriorStock
    // without string-matching.
    let SessionError::GeneratedEmpty(msg) = &err else {
        panic!("expected SessionError::GeneratedEmpty, got {err:?}");
    };
    assert!(
        msg.contains("Pocket"),
        "the refusal must name the toolpath so an operator knows which op to look at: {msg}"
    );
    // The cancellation sentry in `adversarial_2d_campaign_r2.rs` classifies an
    // outcome by searching the error text for "cancel"; a refusal that
    // borrowed the word would be read as an honoured cancel flag.
    assert!(
        !msg.to_ascii_lowercase().contains("cancel"),
        "the refusal text must not be mistakable for a cancellation: {msg}"
    );
}

#[test]
fn a_refused_generation_leaves_no_cached_result() {
    let mut session = pocket_session(OVERSIZE_TOOL_D);
    let _ = generate(&mut session, 0);
    assert!(
        session.get_result(0).is_none(),
        "a refused generation must cache nothing — a 0-move result left in \
         `session.results` is exportable, simulatable, and counts as \
         'generated' to the phantom-prior-stock scan"
    );
}

// ── The exemption that keeps rest chains working ────────────────────────

/// A `Rest` operation returns empty **by contract** when its tool is not
/// finer than the previous one, and rest machining generally is empty
/// whenever the upstream genuinely left nothing. Refusing that would wedge
/// the chains `generate_all(fixpoint: true)` exists to walk, so the gate
/// exempts it — the same carve-out `adversarial_2d_campaign_r2.rs` already
/// makes in its silent-empty gate, for the same geometric reason.
///
/// Note what is NOT asserted: that `zero_removal` fires. It cannot. That
/// finding measures engagement of *emitted cutting geometry* against a
/// reference stock, and an empty toolpath samples zero positions — which
/// X-19 rules is "not measured", not "measured zero". An empty rest pass is
/// reported by its move count and by the generator's log line, not by
/// `zero_removal`.
#[test]
fn a_rest_operation_that_finds_nothing_still_succeeds() {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock_under(HALF, STOCK_Z));
    let prev_idx = session.add_tool(endmill_tool_config(OVERSIZE_TOOL_D));
    let prev_id = session.tools()[prev_idx].id;
    let cur_idx = session.add_tool(endmill_tool_config(OVERSIZE_TOOL_D));
    let cur_tool = session.tools()[cur_idx].id.0;
    let model_id = session.add_model(polygon_model(vec![square_polygon(HALF)], "square"));
    let op = OperationConfig::Rest(RestConfig {
        prev_tool_id: Some(prev_id),
        stepover: 2.0,
        depth: 3.0,
        depth_per_pass: 3.0,
        ..RestConfig::default()
    });
    session
        .add_toolpath(0, toolpath_config("Rest", op, cur_tool, model_id))
        .expect("add rest toolpath");

    generate(&mut session, 0).expect("an empty rest pass is a legitimate result, not a refusal");
    assert_eq!(
        cutting_moves(&session, 0),
        0,
        "fixture is vacuous: this rest pass was supposed to find nothing"
    );
}

// ── G-STICKYEMPTY ───────────────────────────────────────────────────────

/// The discriminating repro, on the fresh-stock path: good params cut, the
/// empty generation is refused, and the SAME good params cut again with no
/// reload in between.
///
/// The live symptom was the third step returning nothing. What made it stick
/// was that the failed generation left state behind; `generate_toolpath` now
/// drops the cached result on the way IN, so no exit — precondition, `?`,
/// generator error or this refusal — can leave a previous parameter set's
/// toolpath behind to be read as the current one's answer.
#[test]
fn an_empty_generation_does_not_stop_the_next_one() {
    let mut session = pocket_session(FITTING_TOOL_D);
    generate(&mut session, 0).expect("baseline generate");
    let baseline = cutting_moves(&session, 0);
    assert!(baseline > 0, "fixture is vacuous");

    // Empty it, exactly as switching a dial to a value the geometry cannot
    // satisfy does.
    session
        .set_tool_param(0, "diameter", &serde_json::json!(OVERSIZE_TOOL_D))
        .expect("set tool diameter");
    let err = generate(&mut session, 0).expect_err("the oversize tool must be refused");
    assert!(matches!(err, SessionError::GeneratedEmpty(_)), "{err:?}");

    // Put it back. This is the step that used to keep returning nothing.
    session
        .set_tool_param(0, "diameter", &serde_json::json!(FITTING_TOOL_D))
        .expect("restore tool diameter");
    generate(&mut session, 0)
        .expect("restoring known-good parameters must regenerate without a project reload");
    assert_eq!(
        cutting_moves(&session, 0),
        baseline,
        "the restored generation must reproduce the baseline exactly — anything else means the \
         empty generation left state behind"
    );
}

/// G-STICKYEMPTY's mechanism, on the path where the empty result is
/// *legitimate* and therefore still cached.
///
/// `ProjectSession::run_simulation` drops toolpaths below
/// `MIN_SIMULATED_MOVES` from the entries it carves, and only a carved entry
/// receives a `prior_stocks` snapshot. The phantom-prior-stock scan used to
/// ask a different question — "does a result object exist" — so an operation
/// that generated empty was counted as generated (no phantom) *and* dropped
/// from the entries (no snapshot). Missing both, a `FromRemainingStock` op
/// hits its own generate-time precondition — "no simulated remaining-stock
/// snapshot is available" — and can never generate again, whatever its
/// parameters are put back to, until a project reload clears its result.
///
/// Nothing is mutated between the two generates here: the sentry's whole
/// claim is that generating an empty rest pass, simulating, and generating it
/// again is not allowed to turn into an error.
#[test]
fn an_empty_rest_pass_keeps_its_prior_stock_snapshot() {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(stock_under(HALF, STOCK_Z));
    let rough_idx = session.add_tool(endmill_tool_config(FITTING_TOOL_D));
    let rough_tool = session.tools()[rough_idx].id.0;
    let rest_idx = session.add_tool(endmill_tool_config(OVERSIZE_TOOL_D));
    let rest_tool = session.tools()[rest_idx].id.0;
    let model_id = session.add_model(polygon_model(vec![square_polygon(HALF)], "square"));

    session
        .add_toolpath(
            0,
            toolpath_config("Rough", pocket_op(), rough_tool, model_id),
        )
        .expect("add rough");
    let mut rest_cfg = toolpath_config("Rest pocket", pocket_op(), rest_tool, model_id);
    rest_cfg.stock_source = StockSource::FromRemainingStock;
    session.add_toolpath(0, rest_cfg).expect("add rest pocket");

    generate(&mut session, 0).expect("rough generates");
    assert!(cutting_moves(&session, 0) > 0, "fixture is vacuous");

    // Round 1: the phantom snapshot unblocks the pending rest op, which then
    // generates empty — legitimately, because it reads remaining stock.
    simulate(&mut session);
    generate(&mut session, 1).expect("first rest generate (phantom snapshot)");
    assert_eq!(
        cutting_moves(&session, 1),
        0,
        "fixture is vacuous: this rest pass was supposed to find nothing"
    );

    // Round 2: nothing changed. The op must still be able to generate.
    simulate(&mut session);
    generate(&mut session, 1).expect(
        "an operation that generated empty must still have a prior-stock snapshot on the next \
         round — otherwise it is stuck until the project is reloaded (G-STICKYEMPTY)",
    );
}
