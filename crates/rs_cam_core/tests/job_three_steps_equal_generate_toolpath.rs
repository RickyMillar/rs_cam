//! WP10 — the three `Job` steps generate what `generate_toolpath`
//! generates, and a stale handle is refused.
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
//! §1 (the `Job` kind), §4 WP10, and §16 rulings 2, 3, 4 and 8.
//!
//! # The shape this file pins
//!
//! A `Job` row runs three synchronous steps, never an `async fn`:
//!
//! - step (i) is [`ProjectSession::start`]. It runs on the frame loop,
//!   holds `&mut self`, drops the cached result, checks the rest and
//!   boundary preconditions, resolves the generation inputs and captures
//!   the session reads the tail needs. It hands back a handle.
//! - step (ii) is `rs_cam_core::session::execute_job`. It is a free
//!   function. It holds no session, so it runs off the frame loop.
//! - step (iii) is `apply(Command::AdoptResult { .. })` with the
//!   revision the handle carries.
//!
//! `generate_toolpath` runs those same three functions inline, so the
//! monolith is split and not wrapped. This file measures that split from
//! the outside: the three steps and `generate_toolpath` must produce one
//! geometry.
//!
//! # What the arms compare
//!
//! Arm (a) compares the MOVE LIST and the SPANS of the adopted result
//! against the move list and the spans `generate_toolpath` caches on a
//! second, identically-built session: the move count, the canonical
//! FNV-1a fingerprint of `Debug` over `toolpath.moves` (which
//! round-trips every `f64` bit, so it is a byte-level identity check),
//! the span count and `spans_valid`.
//!
//! It compares NEITHER `ToolpathStats` NOR the two traces. A trace
//! records wall-clock spans and recorder identities, so an equality over
//! it would measure the recorder rather than the geometry.
//!
//! A non-vacuity guard runs first: an empty move list would make the
//! whole comparison pass on two empty answers.
//!
//! Arm (b) is the staleness contract. The handle carries the revision
//! `start` read. An edit between step (i) and step (iii) moves that
//! revision, and the adopt must then refuse with
//! [`SessionError::StaleCompletion`] and insert nothing.
//!
//! Arm (c) is a function-pointer coercion. A `fn` pointer captures
//! nothing, so a step (ii) that coerces to
//! `fn(&GenerateToolpathHandle, &GenObserver<'_>, &AtomicBool) ->
//! Result<ToolpathComputeResult, SessionError>` cannot hold a session
//! borrow. The coercion is the proof; the arm exists to stop the
//! signature widening back. WP11b added the observer argument; it carries
//! the debug-options gate and the phase sink, and no session.
//!
//! Arm (d) reads the registry column: the `generate_toolpath` row
//! declares [`CommandKind::Job`].
//!
//! # The fixture
//!
//! One 40 mm square polygon model, one Ø6 mm end mill, a 6 mm board
//! hanging below world Z0 (2D operations cut at negative Z), and one
//! pocket that clears the square in a single 3 mm pass. The same recipe
//! `generated_empty_refusal_g_entryempty.rs` uses for its non-empty
//! control, so the generation is known to emit cutting motion.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::session::{
    AdoptResultArgs, Command, CommandId, CommandKind, GenObserver, GenerateToolpathArgs,
    GenerateToolpathHandle, Job, JobHandle, ProjectSession, SessionError, SetToolpathParamArgs,
    ToolpathComputeResult, execute_job,
};

mod common;
use common::fingerprint::move_fingerprint;
use common::session::{polygon_model, single_op_session_with, square_polygon, stock_under};
use common::tools::endmill_tool_config;

/// Half-extent (mm) of the square the pocket clears — a 40 x 40 mm
/// region.
const HALF: f64 = 20.0;

/// Stock height (mm). `stock_under` hangs the board below `z = 0`, the
/// frame a 2D operation cuts in.
const STOCK_Z: f64 = 6.0;

/// Cutter diameter (mm). Radius 3 mm against a 20 mm half-extent, so the
/// inward offset cascade has room to emit.
const TOOL_D: f64 = 6.0;

/// The feed the staleness arm writes, in mm/min. Any value the config
/// does not already carry moves the revision.
const EDITED_FEED_RATE: f64 = 4321.0;

// ── Fixture ─────────────────────────────────────────────────────────

fn pocket_op() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: 2.0,
        depth: 3.0,
        depth_per_pass: 3.0,
        ..PocketConfig::default()
    })
}

/// One stock, one tool, one model, one ungenerated pocket.
fn pocket_session() -> ProjectSession {
    single_op_session_with(
        stock_under(HALF, STOCK_Z),
        endmill_tool_config(TOOL_D),
        polygon_model(vec![square_polygon(HALF)], "square"),
        "Pocket",
        pocket_op(),
        |_| {},
    )
}

/// The geometry of a cached result: the move count, the byte-level move
/// fingerprint, the span count, and whether the spans survived.
fn geometry(session: &ProjectSession, index: usize) -> (usize, u64, usize, bool) {
    let result = session
        .get_result(index)
        .expect("the fixture must hold a cached result to compare");
    let annotated = result.annotated();
    let (moves, hash) = move_fingerprint(&annotated.toolpath);
    (moves, hash, annotated.spans.len(), annotated.spans_valid)
}

/// Run the three steps on `session` and adopt the answer.
fn run_three_steps(session: &mut ProjectSession, index: usize) {
    let cancel = AtomicBool::new(false);
    let JobHandle::GenerateToolpath(handle) = session
        .start(
            Job::GenerateToolpath(GenerateToolpathArgs { index }),
            &cancel,
        )
        .expect("step (i) captures the generation inputs");
    let result = execute_job(&handle, &GenObserver::none(), &cancel)
        .expect("step (ii) generates the toolpath");
    let _ = session
        .apply(Command::AdoptResult(AdoptResultArgs {
            index,
            revision: handle.revision,
            result: Box::new(result),
        }))
        .expect("step (iii) adopts at the revision the handle carries");
}

// ── (a) one geometry ────────────────────────────────────────────────

/// The three steps cache the geometry `generate_toolpath` caches.
#[test]
fn the_three_job_steps_generate_what_generate_toolpath_generates() {
    let mut monolith = pocket_session();
    let cancel = AtomicBool::new(false);
    monolith
        .generate_toolpath(0, &cancel)
        .expect("the pocket generates");

    let mut steps = pocket_session();
    run_three_steps(&mut steps, 0);

    let expected = geometry(&monolith, 0);
    assert!(
        expected.0 > 0,
        "the fixture generated no move at all, so every comparison below \
         would pass on two empty answers"
    );
    assert!(
        expected.2 > 0,
        "the fixture generated no span, so the span comparison below \
         asserts nothing"
    );
    assert_eq!(
        geometry(&steps, 0),
        expected,
        "start / execute_job / AdoptResult must cache the move list and \
         the spans generate_toolpath caches. The tuple is (move count, \
         FNV-1a over Debug of the moves, span count, spans_valid)."
    );
}

/// `generate_toolpath` itself runs the three steps, so it answers the
/// same geometry twice in a row.
///
/// Without this arm, arm (a) would still pass if `generate_toolpath`
/// kept a private second pipeline that happened to agree once.
#[test]
fn generate_toolpath_repeats_its_own_answer() {
    let mut session = pocket_session();
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("the pocket generates");
    let first = geometry(&session, 0);
    session
        .generate_toolpath(0, &cancel)
        .expect("the pocket generates a second time");
    assert_eq!(
        geometry(&session, 0),
        first,
        "one configuration must generate one geometry"
    );
}

// ── (b) a stale handle is refused ───────────────────────────────────

/// An edit between step (i) and step (iii) makes the handle stale, and
/// the adopt refuses.
#[test]
fn an_edit_after_start_refuses_the_handles_result() {
    let mut session = pocket_session();
    let cancel = AtomicBool::new(false);
    let JobHandle::GenerateToolpath(handle) = session
        .start(
            Job::GenerateToolpath(GenerateToolpathArgs { index: 0 }),
            &cancel,
        )
        .expect("step (i) captures the generation inputs");
    let captured = handle.revision;

    let _ = session
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "feed_rate".to_owned(),
            value: serde_json::json!(EDITED_FEED_RATE),
        }))
        .expect("feed_rate is a Pocket parameter");
    assert_ne!(
        session.toolpath_revision(0),
        captured,
        "the edit must move the revision, or the refusal below cannot \
         fire and this arm measures nothing"
    );

    let result = execute_job(&handle, &GenObserver::none(), &cancel)
        .expect("step (ii) generates the toolpath");
    let outcome = session.apply(Command::AdoptResult(AdoptResultArgs {
        index: 0,
        revision: captured,
        result: Box::new(result),
    }));
    assert!(
        matches!(outcome, Err(SessionError::StaleCompletion { .. })),
        "the handle describes inputs the project no longer has, so the \
         adopt must refuse with StaleCompletion; got {outcome:?}"
    );
    assert!(
        session.get_result(0).is_none(),
        "a refused adopt inserts nothing"
    );
}

// ── (c) step (ii) holds no session ──────────────────────────────────

/// `execute_job` coerces to a plain function pointer.
///
/// A `fn` pointer captures no environment. A step (ii) that held a
/// session borrow, or that took `&ProjectSession`, would not coerce to
/// this type at all.
#[test]
fn execute_job_holds_no_session() {
    // WP11b added the observer. It carries the debug-options gate and the
    // phase sink, and it holds no session either, so the claim this arm
    // makes is unchanged.
    let step_two: fn(
        &GenerateToolpathHandle,
        &GenObserver<'_>,
        &AtomicBool,
    ) -> Result<ToolpathComputeResult, SessionError> = execute_job;

    let mut session = pocket_session();
    let cancel = AtomicBool::new(false);
    let JobHandle::GenerateToolpath(handle) = session
        .start(
            Job::GenerateToolpath(GenerateToolpathArgs { index: 0 }),
            &cancel,
        )
        .expect("step (i) captures the generation inputs");
    drop(session);

    let result = step_two(&handle, &GenObserver::none(), &cancel);
    assert!(
        result.is_ok(),
        "step (ii) must generate after the session is dropped; got \
         {result:?}"
    );
}

// ── (d) the registry column ─────────────────────────────────────────

/// The `generate_toolpath` row declares the `Job` kind and its own wire
/// name.
#[test]
fn the_generate_toolpath_row_is_a_job() {
    let job = Job::GenerateToolpath(GenerateToolpathArgs { index: 0 });
    assert_eq!(job.id(), CommandId::GenerateToolpath);
    assert_eq!(
        CommandId::GenerateToolpath.kind(),
        CommandKind::Job,
        "three synchronous steps are a Job, not a Command and not a Query"
    );
    assert_eq!(
        CommandId::GenerateToolpath.wire_name(),
        "generate_toolpath",
        "the wire name is the MCP tool name, and the viz surfaces sentry \
         scans mcp_server.rs for it"
    );
}
