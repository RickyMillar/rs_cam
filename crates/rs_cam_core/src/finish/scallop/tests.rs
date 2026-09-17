//! Unit tests for scallop finishing. Moved out of `finish/scallop.rs` by
//! P4; the module body is unchanged.

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use crate::surface::dropcutter::point_drop_cutter;
use crate::toolpath::MoveIntent;

use super::ring_generation::{RingLiftCtx, closest_kept_point_idx, ring_to_3d, rotate_ring};

use super::*;
use crate::mesh::SpatialIndex;
use crate::tool::BallEndmill;

fn make_flat_mesh() -> (TriangleMesh, SpatialIndex) {
    let mesh = crate::mesh::make_test_flat(50.0);
    let si = SpatialIndex::build(&mesh, 10.0);
    (mesh, si)
}

fn make_hemisphere() -> (TriangleMesh, SpatialIndex) {
    let mesh = crate::mesh::make_test_hemisphere(20.0, 16);
    let si = SpatialIndex::build(&mesh, 10.0);
    (mesh, si)
}

fn ball_cutter() -> BallEndmill {
    BallEndmill::new(6.35, 25.0)
}

// ── Ring generation tests ───────────────────────────────────────

#[test]
fn test_scallop_flat_constant_stepover() {
    // On a flat surface, variable stepover should equal the flat formula everywhere
    let (mesh, si) = make_flat_mesh();
    let cutter = ball_cutter();
    let tool_radius = cutter.radius();
    let scallop_height = 0.1;

    let expected_so =
        crate::finish::scallop_math::stepover_from_scallop_flat(tool_radius, scallop_height);

    let cell_size = 1.0;
    let never_cancel = || false;
    // SAFETY: never_cancel always returns false
    let surface = crate::finish::finish_setup::build_finish_surface_with_cell_size_and_cancel(
        &mesh,
        &si,
        &cutter,
        cell_size,
        &never_cancel,
    )
    .unwrap();
    let slope_map = surface.slope_map;

    // Sample stepover from the slope map at the center (min == mean on
    // a uniform flat surface, so the min-selection change is inert here)
    let so = ring_stepover(
        &[P2::new(0.0, 0.0), P2::new(10.0, 0.0), P2::new(10.0, 10.0)],
        &slope_map,
        tool_radius,
        scallop_height,
    );

    assert!(
        (so - expected_so).abs() < expected_so * 0.3,
        "Flat surface stepover ({:.3}) should be near flat formula ({:.3})",
        so,
        expected_so
    );
}

#[test]
fn test_convex_dome_curvature_sign_tightens_stepover() {
    // Regression pin for the SlopeMap/scallop_math sign-convention mismatch:
    // SlopeMap reports NEGATIVE curvature at a physically convex dome peak
    // (see slope.rs::test_curvature_convex), but scallop_math::variable_stepover
    // expects POSITIVE = convex. ring_stepover negates the raw
    // SlopeMap value before calling variable_stepover. If that negation is
    // ever removed, this test fails: a convex dome must produce a TIGHTER
    // stepover than a flat surface, not a wider one.
    fn make_dome_z_grid(rows: usize, cols: usize, cell_size: f64, radius: f64) -> Vec<f64> {
        let cx = (cols - 1) as f64 * cell_size * 0.5;
        let cy = (rows - 1) as f64 * cell_size * 0.5;
        let mut z = vec![0.0; rows * cols];
        for row in 0..rows {
            for col in 0..cols {
                let x = col as f64 * cell_size - cx;
                let y = row as f64 * cell_size - cy;
                let r_sq = radius * radius - x * x - y * y;
                z[row * cols + col] = if r_sq > 0.0 { r_sq.sqrt() } else { 0.0 };
            }
        }
        z
    }

    let z = make_dome_z_grid(20, 20, 1.0, 8.0);
    let slope_map = crate::surface::slope::SlopeMap::from_z_grid(&z, 20, 20, 0.0, 0.0, 1.0);

    let tool_radius = ball_cutter().radius();
    let scallop_height = 0.1;

    // Dome peak, same cell used by slope.rs::test_curvature_convex.
    let raw_curvature = slope_map.curvature_at(10, 10);
    assert!(
        raw_curvature < 0.0,
        "Dome peak curvature should be negative in SlopeMap convention, got {:.6}",
        raw_curvature
    );

    // Same negation applied at the production call site in
    // ring_stepover.
    let curvature = -raw_curvature;
    let angle = slope_map.angle_at(10, 10);

    let so_dome = variable_stepover(tool_radius, scallop_height, angle, curvature);
    let so_flat =
        crate::finish::scallop_math::stepover_from_scallop_flat(tool_radius, scallop_height);

    assert!(
        so_dome < so_flat,
        "Convex dome stepover ({:.4}) should be tighter than flat stepover ({:.4}) \
             once the sign convention is corrected",
        so_dome,
        so_flat
    );
}

/// Regression sentry: a ring cascade that hits `max_rings` before the
/// offsets collapse must REPORT the interior area it left standing.
///
/// The value was previously computed only to build a `tracing::warn!`,
/// which meant nobody saw it: the campaign shipped a 28 mm block of
/// unmachined material for weeks because no harness run installed a
/// subscriber, and the live GUI has no diagnostic channel for it at all
/// (`planning/unified_v3_design.md` §13/§14c). Returning it is what lets
/// it reach one.
///
/// Paired with `test_scallop_rings_converge`, which asserts the same
/// fixture reports 0.0 when its cap is adequate — so this cannot pass by
/// reporting a non-zero area unconditionally.
#[test]
fn ring_cascade_reports_uncut_core_when_capped() {
    let (mesh, si) = make_flat_mesh();
    let cutter = ball_cutter();
    let tool_radius = cutter.radius();

    let bbox = &mesh.bbox;
    let boundary = Polygon2::new(vec![
        P2::new(bbox.min.x, bbox.min.y),
        P2::new(bbox.max.x, bbox.min.y),
        P2::new(bbox.max.x, bbox.max.y),
        P2::new(bbox.min.x, bbox.max.y),
    ]);

    let never_cancel = || false;
    let surface = crate::finish::finish_setup::build_finish_surface_with_cell_size_and_cancel(
        &mesh,
        &si,
        &cutter,
        1.0,
        &never_cancel,
    )
    .unwrap();

    // Three rings on a 50 mm square cannot reach the middle.
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

    // `max_rings` bounds the OFFSET LOOP; the boundary itself is pushed
    // before it, so a bound cap emits `max_rings + 1` rings. The field
    // logs say the same thing (`max_rings=504 rings_emitted=505`).
    assert_eq!(
        rings.len(),
        4,
        "boundary ring + max_rings offset iterations"
    );
    assert!(
        metrics.uncut_core_mm2 > 100.0,
        "a 50 mm square capped at 3 rings leaves a large uncut core, \
             got {} mm²",
        metrics.uncut_core_mm2
    );
}

#[test]
fn test_scallop_rings_converge() {
    // Rings should progressively shrink until the polygon collapses
    let (mesh, si) = make_flat_mesh();
    let cutter = ball_cutter();
    let tool_radius = cutter.radius();

    let bbox = &mesh.bbox;
    let boundary = Polygon2::new(vec![
        P2::new(bbox.min.x, bbox.min.y),
        P2::new(bbox.max.x, bbox.min.y),
        P2::new(bbox.max.x, bbox.max.y),
        P2::new(bbox.min.x, bbox.max.y),
    ]);

    let cell_size = 1.0;
    let never_cancel = || false;
    // SAFETY: never_cancel always returns false
    let surface = crate::finish::finish_setup::build_finish_surface_with_cell_size_and_cancel(
        &mesh,
        &si,
        &cutter,
        cell_size,
        &never_cancel,
    )
    .unwrap();
    let surface_hm = surface.heightmap;
    let slope_map = surface.slope_map;

    let (rings, metrics) = generate_scallop_rings(
        &boundary,
        &mesh,
        &si,
        &cutter,
        &slope_map,
        &surface_hm,
        tool_radius,
        0.1,
        0.0,
        bbox.min.z,
        100,
        0.5,
    );

    assert!(
        rings.len() >= 3,
        "Should produce multiple rings on 50mm flat, got {}",
        rings.len()
    );

    // The cascade collapsed on its own, so nothing was left standing.
    // This is the control for `ring_cascade_reports_uncut_core_when_capped`
    // below: same fixture, adequate cap, zero standing material. M4 §5b:
    // the hole-aware sibling must agree — no truncation, so no residual
    // either way the area is computed.
    assert_eq!(
        metrics.uncut_core_mm2, 0.0,
        "a cascade that collapses within its cap leaves no uncut core"
    );
    assert_eq!(
        metrics.untouched_mm2, 0.0,
        "a cascade that collapses within its cap leaves nothing untouched"
    );

    // Ring count should be bounded (polygon eventually collapses)
    let flat_so = crate::finish::scallop_math::stepover_from_scallop_flat(tool_radius, 0.1);
    let expected_max = (25.0 / flat_so).ceil() as usize + 5; // half extent / stepover
    assert!(
        rings.len() <= expected_max,
        "Too many rings ({}), expected at most ~{}",
        rings.len(),
        expected_max
    );
}

#[test]
fn test_scallop_z_from_dropcutter() {
    // Ring Z values should match drop-cutter queries
    let (mesh, si) = make_flat_mesh();
    let cutter = ball_cutter();

    let params = ScallopParams {
        scallop_height: 0.1,
        tolerance: 0.5,
        ..ScallopParams::default()
    };

    let tp = scallop_toolpath(&mesh, &si, &cutter, &params);

    // On flat mesh (z≈0), all cutting Z should be near 0
    for m in &tp.moves {
        if let crate::toolpath::MoveType::Linear { .. } = m.move_type
            && m.target.z < params.safe_z - 1.0
        {
            assert!(
                m.target.z.abs() < 2.0,
                "Flat mesh cutting Z should be near 0, got {:.2}",
                m.target.z
            );
        }
    }
}

// ── Integration tests ───────────────────────────────────────────

#[test]
fn test_scallop_produces_toolpath() {
    let (mesh, si) = make_hemisphere();
    let cutter = ball_cutter();
    let params = ScallopParams {
        scallop_height: 0.5, // Coarse for speed
        tolerance: 0.5,
        ..ScallopParams::default()
    };

    let tp = scallop_toolpath(&mesh, &si, &cutter, &params);
    assert!(
        tp.moves.len() > 10,
        "Hemisphere scallop should produce moves, got {}",
        tp.moves.len()
    );
    assert!(
        tp.total_cutting_distance() > 10.0,
        "Should have meaningful cutting distance, got {:.1}",
        tp.total_cutting_distance()
    );
}

#[test]
fn test_scallop_continuous_no_rapids_between_rings() {
    let (mesh, si) = make_flat_mesh();
    let cutter = ball_cutter();
    let params = ScallopParams {
        scallop_height: 0.5,
        tolerance: 0.5,
        continuous: true,
        ..ScallopParams::default()
    };

    let tp = scallop_toolpath(&mesh, &si, &cutter, &params);

    // In continuous mode, there should be very few rapids
    // (just the initial approach and final retract)
    let rapid_count = tp
        .moves
        .iter()
        .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Rapid))
        .count();

    assert!(
        rapid_count <= 4,
        "Continuous scallop should have minimal rapids, got {}",
        rapid_count
    );
}

#[test]
fn test_scallop_inside_out() {
    let (mesh, si) = make_hemisphere();
    let cutter = ball_cutter();
    let params = ScallopParams {
        scallop_height: 0.5,
        tolerance: 0.5,
        direction: ScallopDirection::InsideOut,
        ..ScallopParams::default()
    };

    let tp = scallop_toolpath(&mesh, &si, &cutter, &params);
    assert!(
        tp.moves.len() > 5,
        "Inside-out scallop should produce moves, got {}",
        tp.moves.len()
    );
}

// ── Regression: slope-confined passes must not chord across gaps ──

/// Shared assertion: no consecutive pair of cutting (`FinishingCut`)
/// moves may be farther apart than `max_allowed` — a bigger jump means
/// a cutting move chorded across an excluded slope gap instead of
/// retracting. A rapid or plunge in between resets the check, since
/// that's exactly the retract/replunge link the fix introduces.
fn assert_no_chord_across_gap(tp: &Toolpath, max_allowed: f64) {
    let mut prev_cut: Option<P3> = None;
    let mut saw_any_cut = false;
    for m in &tp.moves {
        let is_cut = matches!(m.move_type, crate::toolpath::MoveType::Linear { .. })
            && m.intent == MoveIntent::FinishingCut;
        if is_cut {
            saw_any_cut = true;
            if let Some(prev) = prev_cut {
                let d = ((m.target.x - prev.x).powi(2) + (m.target.y - prev.y).powi(2)).sqrt();
                assert!(
                    d <= max_allowed,
                    "cutting move chorded across the excluded slope gap: {:.2}mm \
                         (allowed {:.2}mm) from ({:.2},{:.2}) to ({:.2},{:.2})",
                    d,
                    max_allowed,
                    prev.x,
                    prev.y,
                    m.target.x,
                    m.target.y
                );
            }
            prev_cut = Some(m.target);
        } else {
            prev_cut = None;
        }
    }
    assert!(saw_any_cut, "expected at least one cutting move");
}

/// Regression for the discrete-ring chording bug: a slope-confined ring
/// used to filter out-of-band points and feed straight between the
/// remaining survivors (and even close the loop back to the first
/// survivor), chording across the excluded stretch. A hemisphere's
/// slope varies continuously with radius, and a ring's own perimeter
/// varies in distance from center (it's an offset of the roughly
/// rectangular mesh-footprint boundary, not a circle), so a slope band
/// like 20-60° is guaranteed to include only part of most rings.
#[test]
fn test_scallop_discrete_no_chord_across_excluded_slope_gap() {
    let (mesh, si) = make_hemisphere();
    let cutter = ball_cutter();
    let params = ScallopParams {
        scallop_height: 0.3,
        tolerance: 0.3,
        continuous: false,
        slope_from: 20.0,
        slope_to: 60.0,
        ..ScallopParams::default()
    };

    let tp = scallop_toolpath(&mesh, &si, &cutter, &params);

    let stepover = crate::finish::scallop_math::stepover_from_scallop_flat(
        cutter.radius(),
        params.scallop_height,
    );
    assert_no_chord_across_gap(&tp, (stepover * 6.0).max(3.0));
}

/// Companion regression for continuous (spiral) mode: same slope band,
/// same hemisphere, `continuous: true` this time.
#[test]
fn test_scallop_continuous_no_chord_across_excluded_slope_gap() {
    let (mesh, si) = make_hemisphere();
    let cutter = ball_cutter();
    let params = ScallopParams {
        scallop_height: 0.3,
        tolerance: 0.3,
        continuous: true,
        slope_from: 20.0,
        slope_to: 60.0,
        ..ScallopParams::default()
    };

    let tp = scallop_toolpath(&mesh, &si, &cutter, &params);

    let stepover = crate::finish::scallop_math::stepover_from_scallop_flat(
        cutter.radius(),
        params.scallop_height,
    );
    assert_no_chord_across_gap(&tp, (stepover * 6.0).max(3.0));
}

// ── Helper tests ────────────────────────────────────────────────

#[test]
fn test_closest_kept_point_idx() {
    let ring = vec![
        (P3::new(0.0, 0.0, 0.0), true),
        (P3::new(10.0, 0.0, 0.0), true),
        (P3::new(10.0, 10.0, 0.0), true),
        (P3::new(0.0, 10.0, 0.0), true),
    ];
    let target = P3::new(9.0, 9.0, 0.0);
    let idx = closest_kept_point_idx(&ring, &target, |pt| pt.1);
    assert_eq!(idx, Some(2), "Closest to (9,9) should be index 2 (10,10)");

    // Excluding the geometrically-closest point must fall through to
    // the next-closest KEPT point, not return the excluded one.
    let mut masked = ring.clone();
    masked[2].1 = false;
    let idx = closest_kept_point_idx(&masked, &target, |pt| pt.1);
    assert_eq!(
        idx,
        Some(1),
        "With (10,10) excluded, closest kept to (9,9) is index 1 (10,0)"
    );

    // Nothing kept -> None (the caller skips the ring entirely).
    let idx = closest_kept_point_idx(&ring, &target, |_| false);
    assert_eq!(idx, None);
}

#[test]
fn test_rotate_ring() {
    let ring = vec![
        (P3::new(0.0, 0.0, 0.0), true),
        (P3::new(1.0, 0.0, 0.0), true),
        (P3::new(2.0, 0.0, 0.0), true),
        (P3::new(3.0, 0.0, 0.0), true),
    ];
    let rotated = rotate_ring(&ring, 2);
    assert!((rotated[0].0.x - 2.0).abs() < 0.01);
    assert!((rotated[1].0.x - 3.0).abs() < 0.01);
    assert!((rotated[2].0.x - 0.0).abs() < 0.01);
    assert!((rotated[3].0.x - 1.0).abs() < 0.01);
}

// ── P2.f: chord refinement ────────────────────────────────────────

/// A flat strip with a sharp triangular ridge running along Y at x=0
/// (apex z=2, base half-width 0.4 mm) — a terrain feature narrower
/// than typical ring point spacing, i.e. the minimal "smooshed
/// mountain" reproducer: exact endpoints either side, a knob between.
fn make_ridge_mesh() -> (TriangleMesh, SpatialIndex) {
    let xs = [-10.0, -0.4, 0.0, 0.4, 10.0];
    let zs = [0.0, 0.0, 2.0, 0.0, 0.0];
    let mut vertices = Vec::new();
    for y in [-10.0, 10.0] {
        for (x, z) in xs.iter().zip(zs.iter()) {
            vertices.push(P3::new(*x, y, *z));
        }
    }
    let mut triangles = Vec::new();
    for i in 0..4u32 {
        triangles.push([i, i + 1, 5 + i]);
        triangles.push([i + 1, 5 + i + 1, 5 + i]);
    }
    let mesh = TriangleMesh::from_raw(vertices, triangles);
    let si = SpatialIndex::build(&mesh, 5.0);
    (mesh, si)
}

fn lift_ctx_for<'a>(
    mesh: &'a TriangleMesh,
    si: &'a SpatialIndex,
    cutter: &'a BallEndmill,
    probe_step: f64,
) -> RingLiftCtx<'a> {
    RingLiftCtx {
        mesh,
        index: si,
        cutter,
        stock_to_leave: 0.0,
        min_z: mesh.bbox.min.z,
        chord_tolerance: 0.05,
        probe_step,
    }
}

#[test]
fn chord_refinement_lifts_path_over_sharp_ridge() {
    let (mesh, si) = make_ridge_mesh();
    let cutter = BallEndmill::new(1.0, 10.0);
    // The ring-lift context no longer carries a heightmap (F2: the
    // coverage guard is the exact point-in-triangle test), so this test
    // no longer builds a finish surface just to hand one over — the
    // probe step it actually needs is passed directly.
    let ctx = lift_ctx_for(&mesh, &si, &cutter, 0.25);

    // Two exact surface points on the flats either side of the ridge —
    // pre-fix, the emitted chord between them cut straight through the
    // ridge at z ≈ 0, beheading it.
    let ring = vec![P2::new(-2.0, 0.0), P2::new(2.0, 0.0)];
    let refined = ring_to_3d(&ring, &ctx);

    assert!(
        refined.len() > 2,
        "refinement must insert points over the ridge"
    );
    let apex = refined
        .iter()
        .filter(|(p, kept)| *kept && p.x.abs() < 0.5)
        .map(|(p, _)| p.z)
        .fold(f64::MIN, f64::max);
    assert!(
        apex > 1.5,
        "refined path must climb over the ridge apex (z≈2), got max z {apex:.3}"
    );
    // Post-refinement, no kept→kept chord may deviate from the surface
    // by more than tolerance + probe aliasing slack.
    for w in refined.windows(2) {
        let (a, ka) = w[0];
        let (b, kb) = w[1];
        if !(ka && kb) {
            continue;
        }
        let mx = (a.x + b.x) * 0.5;
        let my = (a.y + b.y) * 0.5;
        let cl = point_drop_cutter(mx, my, &mesh, &si, &cutter);
        if !cl.z.is_finite() {
            continue;
        }
        let chord_z = (a.z + b.z) * 0.5;
        assert!(
            (cl.z - chord_z).abs() < 0.3,
            "residual chord error {:.3} at ({mx:.2},{my:.2})",
            (cl.z - chord_z).abs()
        );
    }
}

#[test]
fn chord_refinement_no_op_on_flat() {
    let (mesh, si) = make_flat_mesh();
    let cutter = ball_cutter();
    // The ring-lift context no longer carries a heightmap (F2: the
    // coverage guard is the exact point-in-triangle test), so this test
    // no longer builds a finish surface just to hand one over — the
    // probe step it actually needs is passed directly.
    let ctx = lift_ctx_for(&mesh, &si, &cutter, 0.5);

    let ring = vec![
        P2::new(-20.0, -20.0),
        P2::new(20.0, -20.0),
        P2::new(20.0, 20.0),
        P2::new(-20.0, 20.0),
    ];
    let refined = ring_to_3d(&ring, &ctx);
    assert_eq!(
        refined.len(),
        4,
        "flat chords already within tolerance must not gain points (segment-count/runtime guard)"
    );
    assert!(refined.iter().all(|&(p, kept)| kept && p.z.abs() < 0.01));
}

// ── P2.3: boundary_regions pre-clip ──────────────────────────────

/// `boundary_regions = None` reproduces the unrestricted toolpath
/// (behaviorally — the P2.3 bonus covered-fix changes the *baseline*
/// slightly from pre-P2.3 code, but the parameter itself must be a
/// no-op): every cutting move on a flat mesh with no boundary passed
/// should land somewhere on the mesh footprint.
#[test]
fn scallop_boundary_regions_none_is_unrestricted() {
    let (mesh, si) = make_flat_mesh();
    let cutter = ball_cutter();
    let params = ScallopParams {
        scallop_height: 0.5,
        tolerance: 0.5,
        ..ScallopParams::default()
    };
    let never_cancel = || false;

    let (tp, _, _) = scallop_toolpath_structured_annotated_with_cancel(
        &mesh,
        &si,
        &cutter,
        &params,
        None,
        None,
        &never_cancel,
    )
    .unwrap();

    assert!(
        tp.moves.len() > 10,
        "unrestricted scallop should produce moves, got {}",
        tp.moves.len()
    );
}

/// `boundary_regions = Some(&[left-half])` confines every cutting move's
/// XY to that region (a small containment tolerance absorbs the ring's
/// own point spacing landing right on the region edge).
#[test]
fn scallop_boundary_regions_confines_cuts_to_region() {
    let (mesh, si) = make_flat_mesh();
    let cutter = ball_cutter();
    let bbox = &mesh.bbox;
    let left_half = Polygon2::new(vec![
        P2::new(bbox.min.x, bbox.min.y),
        P2::new(0.0, bbox.min.y),
        P2::new(0.0, bbox.max.y),
        P2::new(bbox.min.x, bbox.max.y),
    ]);
    let params = ScallopParams {
        scallop_height: 0.5,
        tolerance: 0.5,
        ..ScallopParams::default()
    };
    let never_cancel = || false;

    let left_half_regions = std::slice::from_ref(&left_half);
    let region_set = RegionSet::from_slice(left_half_regions);
    let (tp, _, _) = scallop_toolpath_structured_annotated_with_cancel(
        &mesh,
        &si,
        &cutter,
        &params,
        None,
        Some(&region_set),
        &never_cancel,
    )
    .unwrap();

    let tol = 1e-6;
    let mut saw_cut = false;
    for m in &tp.moves {
        if let crate::toolpath::MoveType::Linear { .. } = m.move_type
            && m.intent == MoveIntent::FinishingCut
        {
            saw_cut = true;
            assert!(
                m.target.x <= tol,
                "cutting move X={:.3} escaped the left-half boundary region",
                m.target.x
            );
        }
    }
    assert!(saw_cut, "expected at least one cutting move");
}

/// Two disjoint boundary regions each get their own ring set — cuts
/// land in both regions and never in the excluded gap between them.
#[test]
fn scallop_boundary_regions_two_disjoint_regions_both_cut() {
    let (mesh, si) = make_flat_mesh();
    let cutter = ball_cutter();
    let bbox = &mesh.bbox;
    // Left third and right third of the mesh, with a gap in the middle.
    let left = Polygon2::new(vec![
        P2::new(bbox.min.x, bbox.min.y),
        P2::new(-15.0, bbox.min.y),
        P2::new(-15.0, bbox.max.y),
        P2::new(bbox.min.x, bbox.max.y),
    ]);
    let right = Polygon2::new(vec![
        P2::new(15.0, bbox.min.y),
        P2::new(bbox.max.x, bbox.min.y),
        P2::new(bbox.max.x, bbox.max.y),
        P2::new(15.0, bbox.max.y),
    ]);
    let regions = vec![left, right];
    let region_set = RegionSet::from_slice(&regions);
    let params = ScallopParams {
        scallop_height: 0.5,
        tolerance: 0.5,
        ..ScallopParams::default()
    };
    let never_cancel = || false;

    let (tp, _, _) = scallop_toolpath_structured_annotated_with_cancel(
        &mesh,
        &si,
        &cutter,
        &params,
        None,
        Some(&region_set),
        &never_cancel,
    )
    .unwrap();

    let mut saw_left = false;
    let mut saw_right = false;
    for m in &tp.moves {
        if let crate::toolpath::MoveType::Linear { .. } = m.move_type
            && m.intent == MoveIntent::FinishingCut
        {
            assert!(
                m.target.x <= -15.0 + 1e-6 || m.target.x >= 15.0 - 1e-6,
                "cutting move X={:.3} landed in the excluded gap between regions",
                m.target.x
            );
            if m.target.x <= -15.0 + 1e-6 {
                saw_left = true;
            }
            if m.target.x >= 15.0 - 1e-6 {
                saw_right = true;
            }
        }
    }
    assert!(saw_left, "expected at least one cut in the left region");
    assert!(saw_right, "expected at least one cut in the right region");
}
