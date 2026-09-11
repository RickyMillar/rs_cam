//! WP11b sentry — one input assembly answers both generation doors (N12).
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
//! §4 WP11b and §22.
//!
//! Tracker row N12 lists the generation-input divergences between the core
//! session door and the GUI worker door. WP10 gave the core door three
//! steps — `start`, [`execute_job`], `AdoptResult`. WP11b makes the GUI
//! worker run those same three steps, so the worker's own assembly goes
//! away and one resolver answers for both doors.
//!
//! This file measures three things a compiler cannot:
//!
//! - **(a) The feed-optimisation input reaches the core door.** N12 item 3
//!   said the GUI door built a feed-optimisation stock and the core door
//!   passed `None`, so the two doors emitted different feed rates from one
//!   configuration. The measurement is BEHAVIOURAL, not an identity: an
//!   identity between two routes that call one function is true by
//!   construction and says nothing. The quantity is the number of cutting
//!   moves whose feed rate is not the operation's commanded feed. Before
//!   WP11b that count is zero on the core door.
//! - **(b) The worker names no loose executor.** No file under
//!   `crates/rs_cam_viz/src/` names `execute_operation_annotated`, comment
//!   lines excluded. A viz-side call to the loose entry is a second
//!   assembly, whatever else the worker does.
//! - **(c) The bundle keeps one producer.** The `-> Result<ResolvedGenInputs`
//!   spelling appears once in the core crate. WP11a's own sentry scans three
//!   spellings; this arm pins the one WP11b must not multiply.
//! - **(d) The narrowed executor exists and takes the bundle.** A function
//!   pointer coercion, so the argument list is checked at compile time.
//!
//! If this test fails because a symbol moved, retarget it — do not delete it.
//!
//! NOT MEASURED here: the viz drain, the lane and the artifact write. Those
//! are viz-side and their sentries live in `crates/rs_cam_viz/`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::execute::{GeneratedToolpath, GenerationFindings, OperationError};
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::session::{
    AdoptResultArgs, Command, GenContext, GenObserver, GenerateToolpathArgs, Job, JobHandle,
    ProjectSession, ResolvedGenInputs, SessionError, ToolpathComputeResult, execute_generation,
    execute_job,
};
use rs_cam_core::toolpath::Toolpath;

mod common;
use common::session::{polygon_model, single_op_session_with, square_polygon, stock_under};
use common::tools::endmill_tool_config;

/// Half-extent (mm) of the square the pocket clears — a 40 x 40 mm region.
/// The same fixture WP10's sentry uses.
const HALF: f64 = 20.0;

/// Stock height (mm). `stock_under` hangs the board below `z = 0`, the frame
/// a 2D operation cuts in, so the pocket's levels sit inside material. A
/// pocket that cuts air reads no feed modulation at all and the arm below
/// would be vacuous.
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

/// One stock, one tool, one model, one ungenerated pocket.
///
/// `toolpath_config` takes the dressups from `DressupConfig::for_op`, whose
/// base is `DressupConfig::default()` — and that carries
/// `feed_optimization: true`. The pass under test is therefore LIVE on this
/// fixture without the test setting a dial.
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

/// The operation's commanded cutting feed, in mm/min.
fn commanded_feed(session: &ProjectSession) -> f64 {
    session.toolpath_configs()[0].operation.feed_rate()
}

/// How many cutting moves carry a feed the operation did not command.
///
/// The feed-optimisation pass rewrites the feed rate in place and touches
/// nothing else, so this count is exactly what that pass changed.
fn modulated_move_count(toolpath: &Toolpath, commanded: f64) -> usize {
    toolpath
        .moves
        .iter()
        .filter(|mv| mv.move_type.is_cutting())
        .filter(|mv| {
            mv.move_type
                .feed_rate()
                .is_some_and(|feed| (feed - commanded).abs() > 1e-9)
        })
        .count()
}

/// Run the three `Job` steps and adopt the answer.
fn run_three_steps(session: &mut ProjectSession, index: usize) {
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

/// The cached move list of toolpath `index`.
fn cached_moves(session: &ProjectSession, index: usize) -> Toolpath {
    session
        .get_result(index)
        .expect("the fixture must hold a cached result")
        .annotated()
        .toolpath
        .clone()
}

// ── (a) the feed-optimisation input reaches the core door ───────────

/// The `start` + `execute_job` route reads the feed-optimisation stock.
///
/// N12 item 3. Before WP11b the core door passed `None` where the GUI door
/// passed a stock, so every cutting move on this route kept its commanded
/// feed and the count below is zero.
#[test]
fn the_job_route_modulates_feeds_from_the_feed_optimisation_stock() {
    let mut session = pocket_session();
    let commanded = commanded_feed(&session);
    run_three_steps(&mut session, 0);
    let toolpath = cached_moves(&session, 0);

    let cutting = toolpath
        .moves
        .iter()
        .filter(|mv| mv.move_type.is_cutting())
        .count();
    assert!(
        cutting > 0,
        "the fixture generated no cutting motion, so the feed measurement \
         below is vacuous"
    );

    let modulated = modulated_move_count(&toolpath, commanded);
    assert!(
        modulated > 0,
        "N12 item 3: the core door must read a feed-optimisation stock, so \
         at least one of the {cutting} cutting moves must carry a feed the \
         operation did not command ({commanded} mm/min); none did"
    );
}

/// `generate_toolpath` reads the same input, and reports the same count.
///
/// The two routes run one set of steps after WP11b, so this arm is an
/// agreement check over a quantity the arm above proves is non-zero — never
/// an identity standing on its own.
#[test]
fn both_core_routes_report_one_feed_optimisation_answer() {
    let mut steps = pocket_session();
    let commanded = commanded_feed(&steps);
    run_three_steps(&mut steps, 0);

    let mut monolith = pocket_session();
    let cancel = AtomicBool::new(false);
    monolith
        .generate_toolpath(0, &cancel)
        .expect("the session door generates a toolpath");

    let by_steps = modulated_move_count(&cached_moves(&steps, 0), commanded);
    let by_monolith = modulated_move_count(&cached_moves(&monolith, 0), commanded);
    assert!(
        by_steps > 0,
        "the three-step route modulated no feed, so this agreement is vacuous"
    );
    assert_eq!(
        by_steps, by_monolith,
        "the three steps and generate_toolpath must read one \
         feed-optimisation input; the steps modulated {by_steps} moves and \
         generate_toolpath modulated {by_monolith}"
    );
}

// ── (b) the worker names no loose executor ──────────────────────────

/// A line comment, a doc comment, or a module doc comment.
fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with("//")
}

/// Collect every `.rs` file under `dir`, recursively.
fn rust_sources_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let listing = std::fs::read_dir(dir);
    let listing = listing.unwrap_or_else(|e| panic!("read dir {}: {e}", dir.display()));
    for entry in listing {
        let entry = entry.unwrap_or_else(|e| panic!("entry in {}: {e}", dir.display()));
        let path = entry.path();
        if path.is_dir() {
            rust_sources_under(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Every `.rs` file under the viz crate's `src/`, in a stable order.
fn viz_sources() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("rs_cam_viz")
        .join("src");
    assert!(
        root.is_dir(),
        "{} is not a directory; retarget this test",
        root.display()
    );
    let mut files = Vec::new();
    rust_sources_under(&root, &mut files);
    files.sort();
    files
}

/// The GUI worker calls no loose executor.
///
/// The loose entry takes 21 arguments and builds nothing, so a viz-side call
/// to it is a second input assembly however the request is shaped. Comment
/// lines are excluded: several comments name the function to explain the
/// history, and a comment is not a call.
#[test]
fn no_viz_source_names_the_loose_executor() {
    let files = viz_sources();
    assert!(
        !files.is_empty(),
        "the viz source scan read no file; an empty population passes and \
         looks healthy"
    );

    let mut hits: Vec<String> = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for (index, line) in text.lines().enumerate() {
            if is_comment(line) {
                continue;
            }
            if line.contains("execute_operation_annotated") {
                hits.push(format!("{}:{}", path.display(), index + 1));
            }
        }
    }

    assert!(
        hits.is_empty(),
        "the GUI worker must reach the executor through \
         rs_cam_core::session::execute_job, never the loose entry; these \
         lines name it: {}",
        hits.join(", ")
    );
}

// ── (c) the bundle keeps one producer ───────────────────────────────

/// One core function returns `Result<ResolvedGenInputs, _>`.
///
/// WP11b moves fields into the bundle and adds a narrowed executor over it.
/// Neither may add a producer. WP11a's sentry scans three return spellings;
/// this arm pins the one the resolver uses.
#[test]
fn one_core_function_returns_the_resolved_bundle() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    assert!(root.is_dir(), "{} is not a directory", root.display());
    let mut files = Vec::new();
    rust_sources_under(&root, &mut files);
    files.sort();
    assert!(!files.is_empty(), "the core source scan read no file");

    let mut hits: Vec<String> = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        for (index, line) in text.lines().enumerate() {
            if is_comment(line) {
                continue;
            }
            if line.contains("-> Result<ResolvedGenInputs") {
                hits.push(format!("{}:{}", path.display(), index + 1));
            }
        }
    }

    assert_eq!(
        hits.len(),
        1,
        "exactly one core function returns Result<ResolvedGenInputs, _>; \
         the scan found: [{}]",
        hits.join(", ")
    );
}

// ── (d) the narrowed executor takes the bundle ──────────────────────

/// The narrowed executor reads the resolved bundle and the captured context.
///
/// A function-pointer coercion, so the argument list is checked by the
/// compiler. The loose 21-argument entry stays for the in-module strategy
/// advisor until WP12; what this arm pins is that the narrow one exists and
/// that its inputs are the two unconstructible bundles.
// The explicit argument list IS the claim, so it does not factor into a
// type alias.
#[allow(clippy::type_complexity)]
#[test]
fn the_narrowed_executor_reads_the_resolved_bundle() {
    let _narrowed: fn(
        &ResolvedGenInputs,
        &GenContext,
        &GenObserver<'_>,
        &AtomicBool,
    ) -> Result<(GeneratedToolpath, GenerationFindings), OperationError> = execute_generation;

    let _step_two: fn(
        &rs_cam_core::session::GenerateToolpathHandle,
        &GenObserver<'_>,
        &AtomicBool,
    ) -> Result<ToolpathComputeResult, SessionError> = execute_job;
}
