//! **F-1, reproduced end-to-end and then fixed** — Checkpoint C, Q2, option
//! (b) (`planning/review_2026-08-04/ADVERSARIAL_2D_FINDINGS.md` F-1;
//! `CAVALIER_SHAPE_FAILURE.md` §5.3 / §6 D-3a).
//!
//! W4 ranked F-1 first and recorded it honestly as **not reproduced**:
//!
//! > This wave did not drive a panicking boundary polygon through
//! > `apply_boundary_clip` and observe an unclipped toolpath. It is a
//! > code-path finding with all three links cited. Building that
//! > reproduction is the first thing a Checkpoint-C-approved fix should do,
//! > **red-first**.
//!
//! This file is that reproduction, kept permanently rather than thrown away
//! once it went green — because the thing it proves is not "the fix works"
//! but "the defect was real", and the second claim is the one a future reader
//! will doubt.
//!
//! # The defect, restated
//!
//! `boundary::effective_boundary` turns a containment setting into geometry
//! with a single offset. `clip_annotated_to_boundary_set`'s documented
//! contract is that an EMPTY boundary slice *"is not an error and not a clip:
//! the toolpath passes through with an identity mapping."* So an operator who
//! sets `Inside` — "keep the whole cutter inside this boundary", a safety
//! containment — gets **no containment at all** whenever that offset comes
//! back empty, and until Checkpoint C nothing could tell a genuine collapse
//! (correct: the tool is larger than the region, nothing is machinable) from
//! a contained library failure (an unbounded over-cut).
//!
//! # The four arms
//!
//! | test | proves |
//! |---|---|
//! | `the_pre_fix_shape_emits_an_unclipped_path_on_a_failed_containment` | **the red.** The two lines the old code ran, on a boundary whose offset FAILS: the out-of-bounds cut survives untouched |
//! | `a_failed_containment_now_refuses_the_generate` | the fix, at the decision point, through the public API |
//! | `a_failed_region_refuses_end_to_end_through_apply_boundary_clip_multi` | the fix through the whole shipped clip, with the failing geometry injected the way a caller really supplies it |
//! | `a_genuine_collapse_still_passes_through_and_now_says_so` | the case the contract was written for is **preserved** — `boundary.rs`'s tool-larger-than-stock pass-through still works, and is no longer silent |
//!
//! # Why the failing fixture is a `NaN` vertex
//!
//! The R1 capture asset no longer reaches an assertion: R-2b measured that
//! `Polygon2::repaired` (R1.5) splits the self-intersecting asset before
//! cavalier sees it. The `NaN` class (R-4b / F-11) still does — it reaches
//! `static_aabb2d_index`'s `min_x <= max_x` — and it is boundary-shaped in
//! the only sense that matters here: it is a polygon a caller can hand to a
//! containment.
//!
//! **That assertion is a `debug_assert!`**, so the two failure arms are
//! asserted only under `cfg!(debug_assertions)`. In a release build the check
//! does not fire, a corrupt spatial index is built, and there is nothing for
//! the containment to catch. That divergence is accepted and documented
//! (Checkpoint C, Q4 option a) rather than papered over with an assertion
//! that would be false in release. The collapse arm has no such caveat and
//! runs in both.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::boundary::{
    ToolContainment, clip_annotated_to_boundary_set, effective_boundary,
    effective_boundary_reported,
};
use rs_cam_core::compute::config::{BoundaryConfig, BoundaryContainment, BoundarySource};
use rs_cam_core::compute::execute::GenerationFindings;
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::polygon::{OffsetFailure, Polygon2};
use rs_cam_core::session::ProjectSession;
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::toolpath_spans::AnnotatedToolpath;

/// A 60 mm square with one non-finite vertex — `adversarial2d`'s
/// `invalid-nan` fixture, inlined so this sentry owns its own geometry.
fn non_finite_square(size: f64) -> Polygon2 {
    Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(size, 0.0),
        P2::new(size, f64::NAN),
        P2::new(0.0, size),
    ])
}

/// A path with one cut well OUTSIDE any plausible containment. If the clip
/// runs, this move is retracted to `safe_z` and becomes a rapid; if the clip
/// is skipped, it survives as a cut exactly where it was.
fn path_with_an_out_of_bounds_cut() -> (AnnotatedToolpath, usize) {
    let mut tp = Toolpath::new();
    tp.feed_to(P3::new(5.0, 5.0, -1.0), 1000.0);
    tp.feed_to(P3::new(500.0, 500.0, -1.0), 1000.0);
    let n = tp.moves.len();
    (AnnotatedToolpath::new(tp), n)
}

fn is_an_unretracted_cut_at(tp: &Toolpath, x: f64, y: f64) -> bool {
    tp.moves.iter().any(|m| {
        m.move_type != rs_cam_core::toolpath::MoveType::Rapid
            && (m.target.x - x).abs() < 1e-9
            && (m.target.y - y).abs() < 1e-9
    })
}

fn inside_boundary_config() -> BoundaryConfig {
    BoundaryConfig {
        enabled: true,
        source: BoundarySource::Stock,
        containment: BoundaryContainment::Inside,
        ..BoundaryConfig::default()
    }
}

// ---------------------------------------------------------------------------
// 1. The red — the pre-fix shape, on the pre-fix input
// ---------------------------------------------------------------------------

/// **The defect, reproduced.** These are literally the two statements the
/// shipped code ran before Checkpoint C:
///
/// ```ignore
/// let boundaries = effective_boundary(&stock_poly, containment, tool_radius);
/// let clipped = clip_annotated_to_boundary_set(
///     annotated,
///     boundaries.first().map(std::slice::from_ref).unwrap_or(&[]),
///     safe_z,
/// );
/// ```
///
/// Both are still public and still behave exactly as they did, which is what
/// makes this a reproduction rather than a re-enactment. On a boundary whose
/// offset FAILS, `boundaries` is empty, the clipper's documented
/// empty-slice contract applies, and the toolpath comes back with the
/// out-of-bounds cut intact — a `ToolContainment::Inside` request that
/// contained nothing.
#[test]
fn the_pre_fix_shape_emits_an_unclipped_path_on_a_failed_containment() {
    let boundary = non_finite_square(60.0);
    let tool_radius = 3.0;

    // Establish the CAUSE first, so a future reader can see this is the
    // failure arm and not a collapse arm that happens to be empty.
    let (boundaries, cause) =
        effective_boundary_reported(&boundary, ToolContainment::Inside, tool_radius);
    if !cfg!(debug_assertions) {
        println!(
            "release build: the NaN class trips only `debug_assert!`s, so \
             there is no contained failure to reproduce here (Checkpoint C \
             Q4 option a). boundaries = {}",
            boundaries.len()
        );
        return;
    }
    assert!(
        matches!(cause, Some(OffsetFailure::LibraryFailure { .. })),
        "this arm needs a containment whose offset FAILED, got {cause:?}"
    );
    assert!(
        boundaries.is_empty(),
        "a contained failure is mapped to an empty result — that is the \
         whole premise of F-1"
    );
    // And the old name still produces the same empty vec, so the pre-fix
    // call really did see this.
    assert!(effective_boundary(&boundary, ToolContainment::Inside, tool_radius).is_empty());

    let (annotated, move_count) = path_with_an_out_of_bounds_cut();
    let recorder =
        rs_cam_core::semantic_trace::ToolpathSemanticRecorder::new("f1-sentry", "Pocket");
    let clipped = clip_annotated_to_boundary_set(
        annotated,
        boundaries.first().map(std::slice::from_ref).unwrap_or(&[]),
        20.0,
    )
    .reconcile(&mut rs_cam_core::transform_provenance::ReconcileSet::new(
        Some(&recorder),
        None,
    ))
    .into_inner();

    assert_eq!(
        clipped.toolpath.moves.len(),
        move_count,
        "the pre-fix path emits the toolpath UNCHANGED — no clip happened"
    );
    assert!(
        is_an_unretracted_cut_at(&clipped.toolpath, 500.0, 500.0),
        "F-1: a cut 500 mm outside a 60 mm `Inside` containment survives as \
         a CUT. This is the over-cut `polygon.rs` argues cannot happen \
         (\"all under-cut directions, never a gouge\") and it is why Q2 was \
         asked."
    );
}

// ---------------------------------------------------------------------------
// 2. The fix, at the decision point
// ---------------------------------------------------------------------------

/// A containment that could not be COMPUTED refuses, and the message says
/// what to do about it. It does not pass through, because the safety argument
/// for passing through ("nothing here is machinable anyway") rests entirely
/// on the boundary having genuinely run out of geometry.
#[test]
fn a_failed_containment_now_refuses_the_generate() {
    if !cfg!(debug_assertions) {
        println!("release build: no contained failure to refuse on");
        return;
    }
    let (_, cause) =
        effective_boundary_reported(&non_finite_square(60.0), ToolContainment::Inside, 3.0);
    let cause = cause.expect("debug build: the NaN vertex trips the index assertion");

    let mut findings = GenerationFindings::default();
    let err = ProjectSession::resolve_collapsed_containment(
        Some(cause),
        BoundaryContainment::Inside,
        6.0,
        1,
        &mut findings,
    )
    .expect_err("a containment that FAILED must refuse, not pass through");

    let msg = err.to_string();
    assert!(
        msg.contains("Inside"),
        "the refusal must name the containment it could not compute: {msg}"
    );
    assert!(
        msg.to_ascii_lowercase().contains("unclipped"),
        "the refusal must say WHY refusing is safer than continuing: {msg}"
    );
    assert!(
        findings.boundary_clip_dropped.is_none(),
        "a refusal is not a finding — nothing was emitted for a finding to \
         describe"
    );
}

/// The same refusal through the whole shipped clip, with the failing polygon
/// injected the way a real caller supplies one: `apply_boundary_clip_multi`
/// takes its regions as a parameter, so this arm exercises region
/// processing, containment mapping, the per-region aggregation and the
/// decision — not just the decision.
#[test]
fn a_failed_region_refuses_end_to_end_through_apply_boundary_clip_multi() {
    if !cfg!(debug_assertions) {
        println!("release build: no contained failure to refuse on");
        return;
    }
    let regions = vec![non_finite_square(60.0)];
    let (annotated, _) = path_with_an_out_of_bounds_cut();
    let recorder =
        rs_cam_core::semantic_trace::ToolpathSemanticRecorder::new("f1-sentry", "Pocket");
    let semantic_ctx = recorder.root_context();
    let mut findings = GenerationFindings::default();

    let result = ProjectSession::apply_boundary_clip_multi(
        annotated,
        &inside_boundary_config(),
        &regions,
        &[],
        6.0,
        20.0,
        &semantic_ctx,
        &mut rs_cam_core::transform_provenance::ReconcileSet::new(Some(&recorder), None),
        &mut findings,
    );

    let err = result.err().expect(
        "the live clip must refuse a containment it could not compute — \
         passing the path through unclipped is exactly F-1",
    );
    println!("refusal: {err}");
    assert!(
        findings.boundary_clip_dropped.is_none(),
        "a refusal must not also record a dropped-containment finding: the \
         two are alternatives, not a pair"
    );
}

// ---------------------------------------------------------------------------
// 3. The case the contract was written for — preserved
// ---------------------------------------------------------------------------

/// **The legitimate pass-through still works.** `boundary.rs`'s contract
/// names a real case — the tool is larger than the region, nothing there is
/// machinable, and emitting the unclipped path is better than silently
/// deleting it — and option (c) ("clip everything away") was declined
/// precisely because it would break this.
///
/// What changed is that it is no longer silent. Before Checkpoint C the only
/// trace was a `tracing::warn!` in a process that usually installs no
/// subscriber; now a typed finding names the containment that was dropped,
/// how many source regions collapsed, and the tool it collapsed under.
///
/// No `debug_assertions` caveat: a 2 mm region inset by a 6 mm tool collapses
/// by arithmetic, in every build.
#[test]
fn a_genuine_collapse_still_passes_through_and_now_says_so() {
    let regions = vec![Polygon2::rectangle(0.0, 0.0, 2.0, 2.0)];
    let tool_diameter = 6.0;

    // The cause, established: empty, and NOT a failure.
    let (boundaries, cause) =
        effective_boundary_reported(&regions[0], ToolContainment::Inside, tool_diameter / 2.0);
    assert!(boundaries.is_empty(), "a 2 mm square inset by 3 mm is gone");
    assert_eq!(
        cause, None,
        "a genuine geometric collapse is not a failure — if this becomes \
         `Some`, the pass-through case has been reclassified as a refusal \
         and every tool-larger-than-stock job stops generating"
    );

    let (annotated, move_count) = path_with_an_out_of_bounds_cut();
    let recorder =
        rs_cam_core::semantic_trace::ToolpathSemanticRecorder::new("f1-sentry", "Pocket");
    let semantic_ctx = recorder.root_context();
    let mut findings = GenerationFindings::default();

    let clipped = ProjectSession::apply_boundary_clip_multi(
        annotated,
        &inside_boundary_config(),
        &regions,
        &[],
        tool_diameter,
        20.0,
        &semantic_ctx,
        &mut rs_cam_core::transform_provenance::ReconcileSet::new(Some(&recorder), None),
        &mut findings,
    )
    .expect(
        "a GENUINE collapse must still pass through — this is the case \
             `boundary.rs:79-83` documents and option (c) was declined over",
    );

    assert_eq!(
        clipped.toolpath.moves.len(),
        move_count,
        "the collapsed-boundary pass-through is unchanged behaviour"
    );

    let dropped = findings
        .boundary_clip_dropped
        .expect("the pass-through must now be reported, not silent");
    assert_eq!(dropped.containment, BoundaryContainment::Inside);
    assert_eq!(dropped.source_region_count, 1);
    assert!((dropped.tool_diameter_mm - tool_diameter).abs() < 1e-9);

    // And it reaches the operator surface, not just the struct.
    let mut stats = rs_cam_core::compute::config::ToolpathStats::default();
    stats.boundary_clip_dropped = findings.boundary_clip_dropped;
    let diags = rs_cam_core::diagnostics::adapters::from_generation::diagnostics_from_generation(
        rs_cam_core::ids::ToolpathId(1),
        &stats,
    );
    assert!(
        diags
            .iter()
            .any(|d| d.id.as_str() == rs_cam_core::diagnostics::ids::GEOM_BOUNDARY_CLIP_DROPPED),
        "a warning nobody sees is not a warning (programme rule 4): {diags:?}"
    );
}
