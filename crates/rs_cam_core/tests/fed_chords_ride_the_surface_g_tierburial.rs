//! G-TIERBURIAL (`planning/tiered_finish_2026-09-30/RESULTS.md`, "Fine-tier
//! burial"): after the scallop ring connector, four more sources put a fed
//! move of the rivmap100 fine tier under the drop-cutter surface. Each test
//! here checks one of them on a small synthetic fixture:
//!
//! 1. A Shallow-band raster chord between two lattice points (0.325 mm on
//!    the fine tier). The band now refines each chord against the surface.
//! 2. A VerySteep-band waterline chord between two fiber crossings
//!    (0.158 mm). The band now splits it at a point pushed off the wall at
//!    the contour's own Z.
//! 3. A surface link feeding straight between its 0.5 mm samples
//!    (0.356 mm). `build_surface_link` now refines each feed within the op
//!    tolerance, or refuses the link.
//! 4. The rapid-order pass copying a group whose first move is a fed link
//!    planned from the previous group's last cut, after that group was
//!    reordered (3.453 mm with `intra_region_hookup_mm` 0). The pass now
//!    retracts and rapids over the link's first point instead.
//!
//! Tests 1-3 carry a non-vacuity check: the same fixture's straight chords
//! (the pre-fix emission, built from the public generators) sit deeper than
//! the tolerance. Test 4 checks the invariant directly: no fed move with an
//! XY length starts from another position than it did in the input.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;

use common::meshes::height_field;
use rs_cam_core::dressup::entry_audit::buried_fed_chords;
use rs_cam_core::dressup::{EntrySurfaceProbe, OffMeshEntry};
use rs_cam_core::finish::finish_planner::FinishPlannerParams;
use rs_cam_core::finish::surface_link::build_surface_link;
use rs_cam_core::finish::unified_finish::{
    UnifiedFinishParams, unified_finish_toolpath_with_cancel,
};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::surface::dropcutter::{batch_drop_cutter, point_drop_cutter};
use rs_cam_core::tool::{BallEndmill, MillingCutter};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath, raster_toolpath_from_grid};
use rs_cam_core::trace::toolpath_spans::{AnnotatedToolpath, Span, SpanKind};
use rs_cam_core::trace::transform_provenance::ReconcileSet;

/// The rivmap100 fine tier's tip: an R1.0 ball.
const BALL_D: f64 = 2.0;
const SAMPLE_MM: f64 = 0.05;
const HALF: f64 = 6.0;
const STEP: f64 = 0.1;

fn cutter() -> BallEndmill {
    BallEndmill::new(BALL_D, 25.0)
}

/// The shipped path tolerance (`UnifiedFinishParams::default()`, 0.05 mm,
/// the rivmap100 fine tier's value).
fn tolerance() -> f64 {
    UnifiedFinishParams::default().tolerance
}

fn probe<'a>(
    mesh: &'a TriangleMesh,
    index: &'a SpatialIndex,
    cutter: &'a BallEndmill,
) -> EntrySurfaceProbe<'a> {
    EntrySurfaceProbe {
        mesh,
        index,
        cutter,
        stock_to_leave: 0.0,
        off_mesh: OffMeshEntry::PlungeFallback,
        rest_stock: None,
    }
}

/// The deepest fed sample of `tp` under the drop-cutter surface among the
/// moves `keep` selects: (depth, move index, chord length).
fn worst_burial(
    tp: &Toolpath,
    probe: &EntrySurfaceProbe<'_>,
    keep: impl Fn(usize) -> bool,
) -> (f64, usize, f64) {
    buried_fed_chords(tp, probe, SAMPLE_MM, f64::NEG_INFINITY, |_| true)
        .iter()
        .filter(|r| keep(r.move_index))
        .map(|r| (r.max_burial_mm, r.move_index, r.chord_len_mm))
        .fold(
            (f64::NEG_INFINITY, 0, 0.0),
            |a, b| if b.0 > a.0 { b } else { a },
        )
}

fn unified(mesh: &TriangleMesh, planner: &FinishPlannerParams) -> Toolpath {
    let index = SpatialIndex::build_auto(mesh);
    let cutter = cutter();
    let params = UnifiedFinishParams {
        // No intra-region relink: this file checks each source alone, and
        // test 3 checks the link.
        intra_region_hookup_mm: 0.0,
        ..UnifiedFinishParams::default()
    };
    let never_cancel = || false;
    let (tp, _, _) = unified_finish_toolpath_with_cancel(
        mesh,
        &index,
        &cutter,
        mesh.bbox.max.z,
        mesh.bbox.min.z,
        &params,
        planner,
        None,
        None,
        None,
        None,
        &never_cancel,
    )
    .expect("uncancelled");
    tp
}

/// A terrace: z = 0 for x < 0 and `TERRACE_H` from x = 0, one 0.1 mm grid
/// cell of wall between (about 86 degrees). A raster row across it crosses
/// the ball's shoulder over the terrace edge, where the drop-cutter surface
/// is convex and, below the ball's equator, near vertical.
const TERRACE_H: f64 = 1.5;

fn terrace() -> TriangleMesh {
    height_field(HALF, STEP, |x, _y| if x >= -1e-9 { TERRACE_H } else { 0.0 })
}

#[test]
fn a_shallow_raster_chord_does_not_pass_under_the_surface() {
    let mesh = terrace();
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = cutter();
    let probe = probe(&mesh, &index, &cutter);
    let tol = tolerance();
    let params = UnifiedFinishParams::default();

    // Non-vacuity: the lattice raster at the band's stepover, straight
    // between lattice points (the band's emission before the fix).
    let min_z = mesh.bbox.min.z - 0.1;
    let grid = batch_drop_cutter(&mesh, &index, &cutter, params.raster_stepover, 0.0, min_z);
    let straight = raster_toolpath_from_grid(
        &grid,
        params.feed_rate,
        params.plunge_rate,
        params.safe_z,
        Some(min_z),
        None,
    );
    let (straight_worst, _, straight_chord) = worst_burial(&straight, &probe, |_| true);
    eprintln!("straight lattice raster: worst {straight_worst:.4} mm (chord {straight_chord:.3})");
    assert!(
        straight_worst > tol,
        "non-vacuity: a straight lattice chord passes {straight_worst:.4} mm under the \
         surface, not more than the tolerance {tol}"
    );

    // The whole terrace in the Shallow band: slope thresholds above 90.
    let planner = FinishPlannerParams {
        steep_threshold_deg: 91.0,
        waterline_threshold_deg: 91.0,
        ..FinishPlannerParams::for_tool(cutter.cusp_radius_mm())
    };
    let tp = unified(&mesh, &planner);
    let cuts = tp
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::FinishingCut)
        .count();
    assert!(cuts > 0, "non-vacuity: the Shallow band emitted cuts");
    let (worst, at, chord) = worst_burial(&tp, &probe, |_| true);
    eprintln!(
        "unified Shallow band: {cuts} cuts, worst {worst:.4} mm at move {at} (chord {chord:.3})"
    );
    assert!(
        worst <= tol,
        "move {at} (chord {chord:.3} mm) passes {worst:.4} mm under the drop-cutter surface; \
         the op tolerance is {tol}"
    );
}

/// A square mesa: top `MESA_H` over |x|, |y| <= `MESA_A`, walls at
/// `MESA_WALL_DEG` down to z = 0. Its vertical corner edges are sharp, so a
/// waterline chord across a corner's contour arc cuts into the corner.
const MESA_H: f64 = 2.0;
const MESA_A: f64 = 2.5;
const MESA_WALL_DEG: f64 = 80.0;

fn mesa() -> TriangleMesh {
    let slope = MESA_WALL_DEG.to_radians().tan();
    height_field(HALF, STEP, |x, y| {
        let d = x.abs().max(y.abs()) - MESA_A;
        (MESA_H - d.max(0.0) * slope).max(0.0)
    })
}

#[test]
fn a_waterline_chord_does_not_pass_under_the_surface() {
    let mesh = mesa();
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = cutter();
    let probe = probe(&mesh, &index, &cutter);
    let tol = tolerance();
    let params = UnifiedFinishParams::default();

    // Non-vacuity: the waterline contour at mid-height, straight between
    // fiber crossings (the band's emission before the fix).
    let z = 0.5 * MESA_H;
    let contours =
        rs_cam_core::ops::waterline::waterline_contours(&mesh, &index, &cutter, z, params.sampling);
    let mut straight = Toolpath::new();
    for c in contours.iter().filter(|c| c.len() >= 3) {
        straight.emit_closed_contour_with_intent(
            c,
            params.safe_z,
            params.feed_rate,
            params.plunge_rate,
            MoveIntent::FinishingCut,
        );
    }
    let (straight_worst, _, straight_chord) = worst_burial(&straight, &probe, |_| true);
    eprintln!(
        "straight waterline contour: worst {straight_worst:.4} mm (chord {straight_chord:.3})"
    );
    assert!(
        straight_worst > tol,
        "non-vacuity: a straight contour chord passes {straight_worst:.4} mm under the \
         surface, not more than the tolerance {tol}"
    );

    // The whole mesa in the VerySteep band: slope thresholds at 0. (At the
    // shipped thresholds the walls are VerySteep, but a 0.35 mm wide wall
    // band is conditioned away into the Shallow raster.)
    let planner = FinishPlannerParams {
        steep_threshold_deg: 0.0,
        waterline_threshold_deg: 0.0,
        ..FinishPlannerParams::for_tool(cutter.cusp_radius_mm())
    };
    let tp = unified(&mesh, &planner);
    // The waterline moves: level fed cuts strictly between the base and
    // the top (the Shallow raster on the top and base is level too, at
    // exactly those two heights).
    let level = |i: usize| {
        let (a, b) = (tp.moves[i - 1].target, tp.moves[i].target);
        tp.moves[i].intent == MoveIntent::FinishingCut
            && (a.z - b.z).abs() < 1e-9
            && b.z > 1e-6
            && b.z < MESA_H - 1e-6
    };
    let n_level = (1..tp.moves.len()).filter(|&i| level(i)).count();
    assert!(
        n_level > 0,
        "non-vacuity: the VerySteep band emitted waterline cuts"
    );
    let (worst, at, chord) = worst_burial(&tp, &probe, level);
    let (all, all_at, _) = worst_burial(&tp, &probe, |_| true);
    eprintln!(
        "unified waterline: {n_level} level cuts, worst {worst:.4} mm at move {at} \
         (chord {chord:.3}); all fed moves: worst {all:.4} at move {all_at}"
    );
    assert!(
        worst <= tol,
        "waterline move {at} (chord {chord:.3} mm) passes {worst:.4} mm under the drop-cutter \
         surface; the op tolerance is {tol}"
    );
}

/// A sharp ridge along Y at x = 0: flanks at `RIDGE_DEG`, `RIDGE_H` high.
const RIDGE_H: f64 = 1.5;
const RIDGE_DEG: f64 = 75.0;

fn ridge() -> TriangleMesh {
    let slope = RIDGE_DEG.to_radians().tan();
    height_field(HALF, STEP / 2.0, |x, _y| {
        (RIDGE_H - x.abs() * slope).max(0.0)
    })
}

#[test]
fn a_surface_link_does_not_pass_under_the_surface() {
    let mesh = ridge();
    let index = SpatialIndex::build_auto(&mesh);
    let cutter = cutter();
    let probe = probe(&mesh, &index, &cutter);
    let tol = tolerance();
    let sampling = UnifiedFinishParams::default().sampling;
    let at = |x: f64, y: f64| {
        let cl = point_drop_cutter(x, y, &mesh, &index, &cutter);
        P3::new(x, y, cl.z)
    };
    let as_feeds = |from: P3, pts: &[P3], to: P3| {
        let mut tp = Toolpath::new();
        tp.rapid_to_with_intent(from, MoveIntent::Linking);
        for &p in pts.iter().chain(std::iter::once(&to)) {
            tp.feed_to_with_intent(p, 1000.0, MoveIntent::Linking);
        }
        tp
    };
    // Ten sample phases across the ridge.
    let mut straight_worst = f64::NEG_INFINITY;
    let mut refined_worst = f64::NEG_INFINITY;
    let mut refused = 0usize;
    for k in 0..10 {
        let x0 = -2.5 + k as f64 * sampling / 10.0;
        let (from, to) = (at(x0, 0.3), at(x0 + 5.0, -0.2));
        let straight = build_surface_link(from, to, &mesh, &index, &cutter, 0.0, sampling, None)
            .expect("the ridge is covered: the unrefined link exists");
        straight_worst =
            straight_worst.max(worst_burial(&as_feeds(from, &straight, to), &probe, |_| true).0);
        match build_surface_link(from, to, &mesh, &index, &cutter, 0.0, sampling, Some(tol)) {
            Some(pts) => {
                refined_worst =
                    refined_worst.max(worst_burial(&as_feeds(from, &pts, to), &probe, |_| true).0);
            }
            None => refused += 1,
        }
    }
    eprintln!(
        "surface link over the ridge: straight between samples worst {straight_worst:.4} mm; \
         refined worst {refined_worst:.4} mm; refused {refused} of 10"
    );
    assert!(
        straight_worst > tol,
        "non-vacuity: the link between its {sampling} mm samples passes {straight_worst:.4} mm \
         under the surface, not more than the tolerance {tol}"
    );
    assert!(
        refused < 10,
        "non-vacuity: the refined link exists for some phase"
    );
    assert!(
        refined_worst <= tol,
        "the refined link passes {refined_worst:.4} mm under the drop-cutter surface; \
         the tolerance is {tol}"
    );
}

#[test]
fn a_reordered_group_does_not_feed_its_link_from_elsewhere() {
    const SAFE_Z: f64 = 10.0;
    let mut tp = Toolpath::new();
    let segment = |tp: &mut Toolpath, a: (f64, f64), b: (f64, f64), retract: bool| {
        tp.rapid_to_with_intent(P3::new(a.0, a.1, SAFE_Z), MoveIntent::Linking);
        tp.feed_to_with_intent(P3::new(a.0, a.1, 0.0), 100.0, MoveIntent::EntryPlunge);
        tp.feed_to_with_intent(P3::new(b.0, b.1, 0.0), 1000.0, MoveIntent::FinishingCut);
        if retract {
            tp.rapid_to_with_intent(P3::new(b.0, b.1, SAFE_Z), MoveIntent::Retract);
        }
    };
    // Group A: five segments that alternate near (x 0..5) and far (x
    // 40..43). Nearest-first visits the near three, then the far two, so
    // the group no longer ends where the input did. The last input segment
    // ends at depth: the link continues from there. The reorder saves more
    // than the rapid back to the link costs (the pass's no-regression guard
    // keeps a group, or the whole input, that a reorder would lengthen).
    segment(&mut tp, (0.0, 0.0), (1.0, 0.0), true);
    segment(&mut tp, (40.0, 0.0), (41.0, 0.0), true);
    segment(&mut tp, (2.0, 5.0), (3.0, 5.0), true);
    segment(&mut tp, (42.0, 5.0), (43.0, 5.0), true);
    segment(&mut tp, (4.0, 10.0), (5.0, 10.0), false);
    // Group B, behind a barrier: a fed surface link from (5, 10) into the
    // next region, then that region's cut.
    let barrier = tp.moves.len();
    tp.feed_to_with_intent(P3::new(5.5, 10.0, 0.0), 1000.0, MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(6.0, 10.0, 0.0), 1000.0, MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(8.0, 10.0, 0.0), 1000.0, MoveIntent::FinishingCut);
    tp.rapid_to_with_intent(P3::new(8.0, 10.0, SAFE_Z), MoveIntent::Retract);
    let n = tp.moves.len();
    let input = tp.clone();
    let at = AnnotatedToolpath::with_spans(
        tp,
        vec![
            Span::new(0, n, SpanKind::Operation),
            Span::boundary(barrier, SpanKind::RapidOrderBarrier),
        ],
    );
    let out = rs_cam_core::dressup::tsp::optimize_rapid_order(at, SAFE_Z, None)
        .reconcile(&mut ReconcileSet::new(None, None))
        .into_inner()
        .toolpath;

    // Non-vacuity: the pass reordered group A.
    let order: Vec<f64> = out
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::FinishingCut)
        .map(|m| m.target.x)
        .collect();
    eprintln!("cut order (end x): {order:?}");
    assert_eq!(
        order,
        vec![1.0, 3.0, 5.0, 41.0, 43.0, 8.0],
        "non-vacuity: group A is reordered"
    );

    // Every fed move with an XY length starts where it started in the input.
    for i in 1..out.moves.len() {
        let m = &out.moves[i];
        if !matches!(m.move_type, MoveType::Linear { .. }) {
            continue;
        }
        let from = out.moves[i - 1].target;
        if (m.target.x - from.x).hypot(m.target.y - from.y) < 1e-9 {
            continue; // a vertical descent
        }
        let j = input
            .moves
            .iter()
            .position(|x| x.target == m.target && x.move_type == m.move_type)
            .expect("every fed move comes from the input");
        let planned = input.moves[j - 1].target;
        assert!(
            from == planned,
            "fed {:?} move {i} to {:?} starts at {from:?}; it was planned from {planned:?}",
            m.intent,
            m.target
        );
    }
}
