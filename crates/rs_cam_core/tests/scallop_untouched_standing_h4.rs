//! M4 §5b sentry — `ScallopReport` gains the untouched/standing split the
//! M4 research oracle (`tests/common/scallop_oracle.rs::OracleReport`)
//! already draws, so a ring cascade's truncation is visible in production
//! telemetry (`ToolpathStats`, narration) rather than only in a research
//! harness. Ruled YES at Checkpoint C
//! (`planning/review_2026-07-29/CHECKPOINT_C_EVIDENCE.md:488`).
//!
//! `uncut_core_mm2` — the pre-existing field — is HOLE-BLIND: it sums each
//! truncated cascade polygon's EXTERIOR shoelace area only
//! (`MEASUREMENT_DOMAINS.md` X-5), so an island inside the truncated core
//! over-reports as uncut even though it is not part of the region. This
//! file proves three things about the fix, using the (now `pub`, test/
//! research seam) `generate_scallop_rings` directly rather than a full
//! mesh/toolpath generation:
//!
//! 1. A cascade that collapses normally reports `untouched_mm2 == 0.0`
//!    (same as `uncut_core_mm2`), and `standing_mm2 == 0.0` when every ring
//!    point is kept.
//! 2. A cascade that hits `max_rings` reports `untouched_mm2 > 0.0` — and,
//!    with no holes in that fixture, `untouched_mm2` equals `uncut_core_mm2`
//!    exactly: the fix changes nothing when there is nothing to correct.
//! 3. **The hole-blindness is actually fixed**: on a truncated region built
//!    from a square boundary with a square hole, `untouched_mm2 <
//!    uncut_core_mm2`, and the gap tracks a closed-form prediction of the
//!    grown hole's area — computed independently of
//!    `ScallopReport`/`Polygon2::area()`, from the known stepover, hole
//!    dimensions and ring count. Without this assertion the split is
//!    decoration: it would pass even if `untouched_mm2` were computed some
//!    other, unrelated way.
//!
//! Report-only, per the ruling: nothing about which moves are emitted
//! changes here — only what the report says.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::finish_setup::build_finish_surface_with_cell_size_and_cancel;
use rs_cam_core::geo::P2;
use rs_cam_core::mesh::{SpatialIndex, make_test_flat};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::scallop::{ScallopReport, generate_scallop_rings};
use rs_cam_core::scallop_math::stepover_from_scallop_flat;
use rs_cam_core::tool::{BallEndmill, MillingCutter};

fn never_cancel() -> bool {
    false
}

fn ball_cutter() -> BallEndmill {
    // radius() == cusp_radius_mm() for a plain ball nose (no taper), so the
    // stepover law reduces to `stepover_from_scallop_flat` exactly — see
    // `MillingCutter::cusp_radius`'s doc.
    BallEndmill::new(6.0, 25.0)
}

fn rectangle_boundary(mesh: &rs_cam_core::mesh::TriangleMesh) -> Polygon2 {
    let bbox = &mesh.bbox;
    Polygon2::new(vec![
        P2::new(bbox.min.x, bbox.min.y),
        P2::new(bbox.max.x, bbox.min.y),
        P2::new(bbox.max.x, bbox.max.y),
        P2::new(bbox.min.x, bbox.max.y),
    ])
}

/// A CCW square ring, `half`-width, centred at the origin.
fn ccw_square(half: f64) -> Vec<P2> {
    vec![
        P2::new(-half, -half),
        P2::new(half, -half),
        P2::new(half, half),
        P2::new(-half, half),
    ]
}

/// 1. A cascade that collapses well inside its `max_rings` cap reports zero
///    on both `uncut_core_mm2` and its hole-aware sibling `untouched_mm2`, and
///    zero `standing_mm2` too — every ring point sits over real, covered mesh
///    on this flat fixture, so nothing is dropped (requirement 4: `standing_mm2`
///    is `0.0` when every ring point is kept).
#[test]
fn collapsing_cascade_reports_zero_untouched_and_standing() {
    let mesh = make_test_flat(50.0);
    let si = SpatialIndex::build(&mesh, 10.0);
    let cutter = ball_cutter();
    let tool_radius = cutter.radius();
    let boundary = rectangle_boundary(&mesh);

    let surface =
        build_finish_surface_with_cell_size_and_cancel(&mesh, &si, &cutter, 1.0, &never_cancel)
            .expect("finish surface");

    let (rings, metrics) = generate_scallop_rings(
        &boundary,
        &mesh,
        &si,
        &cutter,
        &surface.slope_map,
        &surface.heightmap,
        tool_radius,
        0.1,
        0.0,
        mesh.bbox.min.z,
        100,
        0.5,
    );

    assert!(
        rings.len() >= 3,
        "should produce multiple rings on a 50mm flat plate, got {}",
        rings.len()
    );
    assert_eq!(
        metrics.uncut_core_mm2, 0.0,
        "a cascade that collapses within its cap leaves no uncut core"
    );
    assert_eq!(
        metrics.untouched_mm2, 0.0,
        "the hole-aware sibling must agree when there is no truncation"
    );
    assert!(
        metrics.standing_mm2 >= 0.0,
        "standing_mm2 must never be negative, got {}",
        metrics.standing_mm2
    );
    assert_eq!(
        metrics.standing_mm2, 0.0,
        "every ring point sits over covered, real mesh on this fixture, so \
         nothing should be dropped; got {} mm²",
        metrics.standing_mm2
    );
}

/// 2. A cascade that hits `max_rings` before collapsing reports a non-zero
///    `untouched_mm2` — and, since this fixture carries no holes, it must equal
///    `uncut_core_mm2` exactly: the hole-aware fix changes nothing when there
///    is nothing to correct. Same fixture as `scallop.rs`'s internal
///    `ring_cascade_reports_uncut_core_when_capped` sentry.
#[test]
fn truncated_cascade_without_holes_reports_matching_untouched_area() {
    let mesh = make_test_flat(50.0);
    let si = SpatialIndex::build(&mesh, 10.0);
    let cutter = ball_cutter();
    let tool_radius = cutter.radius();
    let boundary = rectangle_boundary(&mesh);

    let surface =
        build_finish_surface_with_cell_size_and_cancel(&mesh, &si, &cutter, 1.0, &never_cancel)
            .expect("finish surface");

    // Three rings on a 50 mm square cannot reach the middle (same budget as
    // the internal sentry this mirrors).
    let (rings, metrics) = generate_scallop_rings(
        &boundary,
        &mesh,
        &si,
        &cutter,
        &surface.slope_map,
        &surface.heightmap,
        tool_radius,
        0.1,
        0.0,
        mesh.bbox.min.z,
        3,
        0.5,
    );

    assert_eq!(
        rings.len(),
        4,
        "boundary ring + max_rings offset iterations"
    );
    assert!(
        metrics.uncut_core_mm2 > 100.0,
        "a 50 mm square capped at 3 rings leaves a large uncut core, got {} mm²",
        metrics.uncut_core_mm2
    );
    assert!(
        metrics.untouched_mm2 > 100.0,
        "the hole-aware sibling must also report the truncation, got {} mm²",
        metrics.untouched_mm2
    );
    assert!(
        (metrics.untouched_mm2 - metrics.uncut_core_mm2).abs() < 1e-6,
        "no holes in this fixture, so untouched_mm2 ({}) must equal \
         uncut_core_mm2 ({}) exactly",
        metrics.untouched_mm2,
        metrics.uncut_core_mm2
    );
}

/// 3. THE assertion that proves the hole-blindness is fixed, not decorated.
///
/// A 40x40 mm square region with a 10x10 mm square hole, capped at
/// `max_rings = 1` so the cascade runs exactly one offset step and the
/// hole survives into the truncated core. On a flat mesh the loop's
/// per-ring stepover reduces to the flat-ground formula exactly (slope and
/// curvature both read `0.0`, bit-for-bit, off a mesh whose height is a
/// single constant) — verified independently by the existing
/// `test_scallop_flat_constant_stepover` unit test in `scallop.rs` — so the
/// offset distance is knowable in closed form without reading anything the
/// cascade itself produced:
///
/// * the EXTERIOR shrinks by `s` on every side (a rectangle offset inward
///   keeps sharp corners): `expected_ext_area = (40 - 2s)²`.
/// * the HOLE grows by `s` on every side. Growing a CONVEX polygon outward
///   by a constant distance is a Minkowski sum with a disk of radius `s`,
///   whose area has the standard closed form `A + P·s + π·s²` (original
///   area, plus perimeter times `s`, plus the four quarter-circle corner
///   fillets) — this is the textbook formula, not anything this test reads
///   off the cascade.
///
/// The difference `uncut_core_mm2 − untouched_mm2` must track the predicted
/// grown-hole area, independently derived. If the split were computed some
/// other, unrelated way, this assertion — not just "the numbers differ" —
/// is what would catch it.
#[test]
fn truncated_region_with_a_hole_is_untouched_short_by_the_holes_area() {
    let mesh = make_test_flat(100.0);
    let si = SpatialIndex::build(&mesh, 10.0);
    let cutter = ball_cutter();
    let cusp_r = cutter.cusp_radius_mm();
    let scallop_height = 0.05;

    const EXT_HALF: f64 = 20.0;
    const HOLE_HALF: f64 = 5.0;

    let mut boundary = Polygon2::with_holes(ccw_square(EXT_HALF), vec![ccw_square(HOLE_HALF)]);
    boundary.ensure_winding();
    assert!(
        boundary.has_correct_winding(),
        "fixture bug: exterior must be CCW and the hole CW before offsetting"
    );

    let surface =
        build_finish_surface_with_cell_size_and_cancel(&mesh, &si, &cutter, 1.0, &never_cancel)
            .expect("finish surface");

    let (rings, metrics) = generate_scallop_rings(
        &boundary,
        &mesh,
        &si,
        &cutter,
        &surface.slope_map,
        &surface.heightmap,
        cusp_r,
        scallop_height,
        0.0,
        mesh.bbox.min.z,
        1, // exactly one offset iteration
        0.01,
    );

    // Boundary ring + exactly one offset ring — confirms the loop ran
    // through its single iteration rather than collapsing early (a
    // collapse would zero BOTH area fields and make the assertions below
    // pass vacuously at 0.0 == 0.0, which is why this is checked first).
    assert_eq!(rings.len(), 2, "boundary ring + one offset iteration");

    let s = stepover_from_scallop_flat(cusp_r, scallop_height);
    assert!(
        s > 0.0 && s.is_finite(),
        "sanity: stepover must be positive"
    );

    let ext_side_after = 2.0 * EXT_HALF - 2.0 * s;
    let expected_ext_area = ext_side_after * ext_side_after;

    let hole_side_before = 2.0 * HOLE_HALF;
    let hole_area_before = hole_side_before * hole_side_before;
    let hole_perimeter_before = 4.0 * hole_side_before;
    let expected_hole_area_after =
        hole_area_before + hole_perimeter_before * s + std::f64::consts::PI * s * s;

    eprintln!(
        "s={s:.6} expected_ext_area={expected_ext_area:.3} \
         expected_hole_area_after={expected_hole_area_after:.3} \
         uncut_core_mm2={:.3} untouched_mm2={:.3}",
        metrics.uncut_core_mm2, metrics.untouched_mm2
    );

    // The hole-blind figure should track the exterior-only prediction...
    let ext_rel_err = (metrics.uncut_core_mm2 - expected_ext_area).abs() / expected_ext_area;
    assert!(
        ext_rel_err < 0.02,
        "uncut_core_mm2 ({}) should track the closed-form exterior area \
         ({expected_ext_area}) to within 2%, got {:.4}%",
        metrics.uncut_core_mm2,
        ext_rel_err * 100.0
    );

    // ...and this is the load-bearing assertion: the hole-aware figure must
    // be SMALLER, by approximately the grown hole's own area.
    assert!(
        metrics.untouched_mm2 < metrics.uncut_core_mm2,
        "untouched_mm2 ({}) must be strictly less than uncut_core_mm2 ({}) \
         once a hole survives into the truncated core — this is the \
         hole-blindness fix",
        metrics.untouched_mm2,
        metrics.uncut_core_mm2
    );
    let observed_gap = metrics.uncut_core_mm2 - metrics.untouched_mm2;
    let gap_rel_err = (observed_gap - expected_hole_area_after).abs() / expected_hole_area_after;
    assert!(
        gap_rel_err < 0.05,
        "the uncut_core_mm2/untouched_mm2 gap ({observed_gap}) should track the \
         independently-derived grown-hole area ({expected_hole_area_after}) to \
         within 5%, got {:.4}%",
        gap_rel_err * 100.0
    );
}

/// 5. The three provenance constants must be pairwise distinct — and
///    `STANDING_PROVENANCE` in particular must not be `comparable_to` either
///    exact-area sibling, since it is an estimator, not a polygon area (M1;
///    `MeasurementProvenance::comparable_to` gates whether two values may
///    legally be summed or ratio'd).
#[test]
fn provenance_constants_are_pairwise_distinct() {
    assert_ne!(
        ScallopReport::PROVENANCE,
        ScallopReport::UNTOUCHED_PROVENANCE,
        "uncut_core_mm2 and untouched_mm2 carry different resolution notes \
         and must not collapse to the same provenance value"
    );
    assert_ne!(
        ScallopReport::PROVENANCE,
        ScallopReport::STANDING_PROVENANCE,
        "an exact polygon area and an estimator must not share a provenance"
    );
    assert_ne!(
        ScallopReport::UNTOUCHED_PROVENANCE,
        ScallopReport::STANDING_PROVENANCE,
        "the hole-aware exact area and the estimator must not share a provenance"
    );

    // The estimator must not be treated as ratio/sum-legal against either
    // exact-area sibling, even though all three share the same
    // MeasurementDomain::ProjectedXyArea label.
    assert!(
        !ScallopReport::STANDING_PROVENANCE.comparable_to(&ScallopReport::PROVENANCE),
        "comparable_to must refuse the estimator against uncut_core_mm2's provenance"
    );
    assert!(
        !ScallopReport::STANDING_PROVENANCE.comparable_to(&ScallopReport::UNTOUCHED_PROVENANCE),
        "comparable_to must refuse the estimator against untouched_mm2's provenance"
    );

    // The two exact-area siblings share domain AND stage on purpose (M4 §5b
    // ruling: "same domain and stage, different resolution statement") —
    // `comparable_to` does not inspect `resolution_note`, so it legitimately
    // reads these two as comparable. Pinned here so a future change to
    // `comparable_to` that starts checking the note does not silently flip
    // this without a reader noticing.
    assert!(
        ScallopReport::PROVENANCE.comparable_to(&ScallopReport::UNTOUCHED_PROVENANCE),
        "PROVENANCE and UNTOUCHED_PROVENANCE share domain and stage by design"
    );
}
