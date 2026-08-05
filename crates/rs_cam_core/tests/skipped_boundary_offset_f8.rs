//! **F-8, reproduced and fixed** — Checkpoint C, D-3b (+ D-3c's choice of
//! semantics). See `planning/review_2026-08-04/ADVERSARIAL_2D_FINDINGS.md`
//! F-8 and `CAVALIER_SHAPE_FAILURE.md` §5.4.
//!
//! Three sites applied `BoundaryConfig::offset` with the same shape:
//!
//! ```ignore
//! let offset_polys = offset_polygon(p, -boundary_config.offset);
//! if let Some(largest) = offset_polys.into_iter().max_by(area) { *p = largest; }
//! ```
//!
//! There is no `else`. On an empty result `p` keeps its **un-offset** value,
//! so the offset the operator dialled silently does not happen. For a
//! NEGATIVE offset — shrinking the machining boundary inwards, the
//! protective direction — the toolpath ends up clipped to a **larger** region
//! than was asked for. That is an over-cut, in the one direction
//! `polygon.rs:295-300`'s safety argument says cannot occur, and both viz
//! sites are on the live worker's boundary path.
//!
//! | test | proves |
//! |---|---|
//! | `the_pre_fix_shape_keeps_the_un_offset_boundary` | **the red.** The exact expression the three sites ran, on a shrink that collapses: the un-offset polygon survives |
//! | `a_collapsed_user_offset_now_drops_the_containment` | D-3b/D-3c: dropped, not un-offset — the multi-region path's semantics |
//! | `a_failed_user_offset_refuses` | the failure arm, distinguished from the collapse |
//! | `a_collapsed_user_offset_reaches_the_clip_as_a_reported_pass_through` | end-to-end through `apply_boundary_clip`: no silent resurrection of the un-offset stock rectangle, and the operator is told |
//!
//! The failure arm is `debug_assertions`-gated for the reason recorded in
//! `boundary_clip_escape_f1.rs`: the `NaN` class trips only `debug_assert!`s
//! (Checkpoint C, Q4 option a). The collapse arms are pure arithmetic and run
//! in every build.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::boundary::{UserOffsetOutcome, apply_user_boundary_offset};
use rs_cam_core::compute::config::{BoundaryConfig, BoundaryContainment, BoundarySource};
use rs_cam_core::compute::execute::GenerationFindings;
use rs_cam_core::geo::{BoundingBox3, P2, P3};
use rs_cam_core::polygon::{Polygon2, offset_polygon};
use rs_cam_core::session::ProjectSession;
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::toolpath_spans::AnnotatedToolpath;

/// A shrink big enough to eat the square whole: the operator asked for a
/// boundary 10 mm inside a 2 mm region.
const TINY: f64 = 2.0;
const SHRINK: f64 = -10.0;

fn non_finite_square(size: f64) -> Polygon2 {
    Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(size, 0.0),
        P2::new(size, f64::NAN),
        P2::new(0.0, size),
    ])
}

// ---------------------------------------------------------------------------
// 1. The red
// ---------------------------------------------------------------------------

/// **The defect, reproduced.** `offset_polygon` and `largest_by_area` are
/// both still public and both still behave exactly as they did, so this runs
/// the pre-fix expression rather than re-enacting it.
#[test]
fn the_pre_fix_shape_keeps_the_un_offset_boundary() {
    let mut p = Polygon2::rectangle(0.0, 0.0, TINY, TINY);
    let area_before = p.area().abs();

    // The exact pre-fix statement. `-SHRINK` is `+10.0`: cavalier's positive
    // distance is an inward shrink, which is why the call sites flipped the
    // sign of the user-facing offset.
    let offset_polys = offset_polygon(&p, -SHRINK);
    assert!(
        offset_polys.is_empty(),
        "a 2 mm square shrunk by 10 mm must collapse — otherwise this \
         fixture is not exercising the branch"
    );
    if let Some(largest) = offset_polys.into_iter().max_by(|a, b| {
        a.area()
            .partial_cmp(&b.area())
            .unwrap_or(std::cmp::Ordering::Equal)
    }) {
        p = largest;
    }

    assert!(
        (p.area().abs() - area_before).abs() < 1e-12,
        "F-8: the requested {SHRINK} mm shrink silently did not happen — the \
         boundary is still the full un-offset {area_before} mm² region, so \
         the toolpath gets clipped to MORE area than was asked for"
    );
}

// ---------------------------------------------------------------------------
// 2. The fix, at the decision
// ---------------------------------------------------------------------------

/// D-3b: nothing-vs-something is now a distinction, and D-3c decided which
/// way "nothing" goes — dropped, matching `RegionSet::processed` on the
/// multi-region path, rather than falling through to the un-offset polygon.
#[test]
fn a_collapsed_user_offset_now_drops_the_containment() {
    let p = Polygon2::rectangle(0.0, 0.0, TINY, TINY);
    match apply_user_boundary_offset(&p, SHRINK) {
        UserOffsetOutcome::Collapsed => {}
        other => panic!("a shrink that eats the region must COLLAPSE: {other:?}"),
    }

    // And the something-case is unchanged: a modest shrink still resolves,
    // and to a smaller polygon than it started with.
    let big = Polygon2::rectangle(0.0, 0.0, 60.0, 60.0);
    match apply_user_boundary_offset(&big, -5.0) {
        UserOffsetOutcome::Resolved(out) => assert!(
            out.area().abs() < big.area().abs(),
            "a -5 mm offset must actually shrink: {} vs {}",
            out.area().abs(),
            big.area().abs()
        ),
        other => panic!("a 60 mm square shrunk 5 mm must resolve: {other:?}"),
    }

    // An outward offset is the direction that basically cannot collapse, and
    // it must not be dragged into the new branch.
    match apply_user_boundary_offset(&big, 5.0) {
        UserOffsetOutcome::Resolved(out) => assert!(out.area().abs() > big.area().abs()),
        other => panic!("a +5 mm offset must grow: {other:?}"),
    }
}

/// The failure arm, kept apart from the collapse arm. Continuing here would
/// mean clipping to the un-offset boundary on the strength of a dependency
/// assertion, which is the F-8 over-cut with an extra step.
#[test]
fn a_failed_user_offset_refuses() {
    if !cfg!(debug_assertions) {
        println!(
            "release build: the NaN class trips only `debug_assert!`s, so \
             there is no contained failure here (Checkpoint C Q4 option a)"
        );
        return;
    }
    match apply_user_boundary_offset(&non_finite_square(60.0), -5.0) {
        UserOffsetOutcome::Failed(f) => println!("failed as expected: {}", f.describe()),
        other => panic!("a NaN vertex must FAIL, not collapse: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 3. End-to-end
// ---------------------------------------------------------------------------

/// Through the shipped clip. The containment mode is `Center`, so
/// `effective_boundary` makes no offset of its own and cannot be the cause —
/// the only thing that can empty this containment is the USER offset, which
/// is what makes this an F-8 test rather than an F-1 one.
///
/// The pre-fix path had a second silent fallback stacked behind the first:
/// `resolve_containment_polygon(..).unwrap_or_else(|| stock rectangle)`. A
/// collapse that returned `None` would have been resurrected as exactly the
/// un-offset rectangle the collapse says is wrong.
#[test]
fn a_collapsed_user_offset_reaches_the_clip_as_a_reported_pass_through() {
    let mut tp = Toolpath::new();
    tp.feed_to(P3::new(1.0, 1.0, -1.0), 1000.0);
    tp.feed_to(P3::new(500.0, 500.0, -1.0), 1000.0);
    let move_count = tp.moves.len();
    let annotated = AnnotatedToolpath::new(tp);

    let boundary = BoundaryConfig {
        enabled: true,
        source: BoundarySource::Stock,
        containment: BoundaryContainment::Center,
        offset: SHRINK,
        ..BoundaryConfig::default()
    };
    let stock_bbox = BoundingBox3::from_points([
        P3::new(0.0, 0.0, -5.0),
        P3::new(TINY, TINY, 0.0),
    ]);

    let recorder =
        rs_cam_core::semantic_trace::ToolpathSemanticRecorder::new("f8-sentry", "Pocket");
    let semantic_ctx = recorder.root_context();
    let mut findings = GenerationFindings::default();

    let clipped = ProjectSession::apply_boundary_clip(
        annotated,
        &boundary,
        &stock_bbox,
        None,
        &[],
        6.0,
        20.0,
        &semantic_ctx,
        &mut rs_cam_core::transform_provenance::ReconcileSet::new(Some(&recorder), None),
        &mut findings,
    )
    .expect("a genuine collapse passes through — only a FAILURE refuses");

    assert_eq!(
        clipped.toolpath.moves.len(),
        move_count,
        "a collapsed containment passes the toolpath through, as ruled"
    );
    let dropped = findings.boundary_clip_dropped.expect(
        "a containment emptied by the USER offset must be reported too — \
         before Checkpoint C this path silently reused the un-offset stock \
         rectangle and said nothing",
    );
    assert_eq!(dropped.containment, BoundaryContainment::Center);
    assert_eq!(dropped.source_region_count, 1);
}
