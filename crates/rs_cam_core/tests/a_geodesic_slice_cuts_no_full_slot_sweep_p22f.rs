//! P22f-1 (feeds contract, `planning/feeds_contract_2026-11-03/phases/P22f_DESIGN.md`):
//! a Geodesic Adaptive slice cuts no full-slot perimeter sweep with a large
//! tool.
//!
//! The fixture is the shelf pocket of the D65 sentry
//! (`a_geodesic_step_over_the_ceiling_is_cut_capped_d65`), every length
//! scaled by `k = D / 6`, with an 18 mm tool: the same plan problem at three
//! times the size. The planner cell is `max(R / 6, tolerance)` = 1.5 mm. On
//! HEAD before P22f-1, the nominal-margin arm of `grown_outlines`
//! (`SIMPLIFY_INSET + m` = 0.55 mm < one cell) refused every region, and the
//! slice cut the region boundary inset by `R + m` as one loop: a full slot.
//!
//! Claim: the decision record (`Produced::region_openings`, P22f-1 ruling
//! 5) holds one opening per region and level, and none keeps the sweep:
//! every decision is `Grown` and neither clip leaves nothing. The slice cuts
//! the sweep exactly when `RegionOpening::cuts_sweep` holds
//! (`clear_one_region`: the sweep rings are empty when the grow is kept), so
//! the record is the statement "no perimeter sweep".
//!
//! Witness (printed, not asserted): the perimeter rule of the P22a readout,
//! on the slice body (the moves before the `WaterlineCleanup` annotation;
//! the waterline pass is a loop by design). It is a heuristic: after
//! P22f-1 it flags one draped open path that is not a ring, and narrowed to
//! flat runs it misses the old sweeps, which drape over the shelf.
//!
//! Red before P22f-1 (2026-11-09, on `36b440b17`; the record did not exist):
//! the P22a perimeter rule found 4 loops, of 555.5, 210.5, 176.0 and
//! 876.2 mm (the last is the waterline pass). Inject check after P22f-1:
//! with the nominal-margin arm put back, the record reads
//! `MeasuredMarginShort` and the test fails.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::print_stderr,
    reason = "test code: an expect is how a test reports a failure; the test prints its measurements for the reader"
)]

mod common;

use common::meshes::height_field;
use rs_cam_core::adaptive3d::{
    Adaptive3dDepth, Adaptive3dGeometry, Adaptive3dLinking, Adaptive3dOutput, Adaptive3dParams,
    Adaptive3dRuntimeEvent, ClearingStrategy3d, EntryStyle3d, KeepDownRoute, OpeningDecision,
    RegionOrdering, StepSplit, adaptive_3d_toolpath_output_with_cancel,
};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

/// The tool diameter of this sentry and of the D65 fixture (mm).
const D: f64 = 18.0;
const D65_D: f64 = 6.0;
/// The scale of every length of the D65 fixture.
const K: f64 = D / D65_D;

// The D65 fixture, as in `a_geodesic_step_over_the_ceiling_is_cut_capped_d65`.
const STEPOVER: f64 = 2.0;
const TOLERANCE: f64 = 0.1;
const STOCK_TO_LEAVE: f64 = 0.5;
const TOP_Z: f64 = 10.0;
const FLOOR_Z: f64 = 4.0;
const MESH_HALF: f64 = 40.0;
const MESH_STEP: f64 = 0.5;
const POCKET_HALF_X: f64 = 34.0;
const POCKET_HALF_Y: f64 = 24.0;
const ISLAND_RADIUS: f64 = 5.0;
const SHELF_Z: f64 = 6.5;
const SHELF_X: (f64, f64) = (12.0, 30.0);
const SHELF_HALF_Y: f64 = 12.0;
const FEED: f64 = 1000.0;

fn shelf() -> TriangleMesh {
    height_field(K * MESH_HALF, K * MESH_STEP, |x, y| {
        if x.hypot(y) <= K * ISLAND_RADIUS {
            K * TOP_Z
        } else if (K * SHELF_X.0..=K * SHELF_X.1).contains(&x) && y.abs() <= K * SHELF_HALF_Y {
            K * SHELF_Z
        } else if x.abs() <= K * POCKET_HALF_X && y.abs() <= K * POCKET_HALF_Y {
            K * FLOOR_Z
        } else {
            K * TOP_Z
        }
    })
}

fn params() -> Adaptive3dParams {
    let r = D / 2.0;
    Adaptive3dParams {
        entry_clearance_mm: K * 0.5,
        ramp_feed_rate: None,
        geometry: Adaptive3dGeometry {
            tool_radius: r,
            envelope_radius: r,
            stepover: K * STEPOVER,
            tolerance: K * TOLERANCE,
            segment_merge_tolerance: None,
            min_cutting_radius: 0.0,
            boundary: None,
            world_stock_xy_bbox: None,
            centre_boundary: Vec::new(),
            gouge_probe: None,
        },
        depth: Adaptive3dDepth {
            depth_per_pass: K * (TOP_Z - FLOOR_Z),
            stock_to_leave: K * STOCK_TO_LEAVE,
            stock_top_z: K * TOP_Z,
            z_floor: None,
            detect_flat_areas: false,
            rest_top_z: None,
        },
        linking: Adaptive3dLinking {
            region_ordering: RegionOrdering::Global,
            min_region_cut_length_mm: 0.0,
            max_stay_down_distance_mm: None,
            stay_down_clearance_mm: K * 0.5,
            step_split: Some(StepSplit::StockDepth),
            keep_down_route: Some(KeepDownRoute::CutFeed),
            level_contour: None,
            rest_region_filter: None,
            one_pass_hold: rs_cam_core::adaptive3d::OnePassHold::Off,
        },
        feed_rate: FEED,
        plunge_rate: 500.0,
        safe_z: K * 15.0,
        entry_style: EntryStyle3d::Plunge,
        initial_stock: None,
        clearing_strategy: ClearingStrategy3d::GeodesicAdaptive,
        trochoid_cap_mult: 1.6,
        engagement_measure: rs_cam_core::adaptive::EngagementMeasure::DiskArea,
    }
}

fn plan() -> Adaptive3dOutput {
    let mesh = shelf();
    let index = SpatialIndex::build(&mesh, K * 10.0);
    let cutter = FlatEndmill::new(D, K * 25.0);
    adaptive_3d_toolpath_output_with_cancel(&mesh, &index, &cutter, &params(), &|| false, None)
        .expect("the plan is not cancelled")
}

/// The P22a perimeter rule on the moves before `body_end` (the witness):
/// each run of uncapped clearing moves that fills a whole entry-to-next
/// non-clearing cut, has at least three moves, and ends within its longest
/// move of its start. Each as (first move, last move, XY length).
fn perimeter_loops(tp: &Toolpath, body_end: usize) -> Vec<(usize, usize, f64)> {
    let moves = &tp.moves[..body_end.min(tp.moves.len())];
    let entry = |i: usize| {
        moves.get(i).is_some_and(|m| {
            matches!(
                m.intent,
                MoveIntent::EntryRamp | MoveIntent::EntryHelix | MoveIntent::EntryPlunge
            )
        })
    };
    let clearing = |i: usize| {
        moves
            .get(i)
            .is_some_and(|m| m.intent == MoveIntent::ClearingCut)
    };
    let xy = |a: P3, b: P3| (b.x - a.x).hypot(b.y - a.y);
    let mut out = Vec::new();
    for s in 1..moves.len() {
        if !entry(s - 1) || !clearing(s) {
            continue;
        }
        let mut e = s;
        while clearing(e + 1) {
            e += 1;
        }
        if moves[s..=e].iter().any(|m| m.feed_cap.is_some()) || e - s < 2 {
            continue;
        }
        let steps: Vec<f64> = (s..=e)
            .map(|k| xy(moves[k - 1].target, moves[k].target))
            .collect();
        let longest = steps.iter().copied().fold(0.0, f64::max);
        if xy(moves[s - 1].target, moves[e].target) <= longest {
            out.push((s, e, steps.iter().sum()));
        }
    }
    out
}

#[test]
fn a_geodesic_slice_cuts_no_full_slot_sweep() {
    let out = plan();
    let body_end = out
        .annotations
        .iter()
        .find(|a| matches!(a.event, Adaptive3dRuntimeEvent::WaterlineCleanup))
        .map_or(out.toolpath.moves.len(), |a| a.move_index);
    let loops = perimeter_loops(&out.toolpath, body_end);
    eprintln!(
        "scaled shelf (D {D}): {} moves; witness (P22a rule, body): {} loops {loops:?}",
        out.toolpath.moves.len(),
        loops.len()
    );
    let openings = &out.produced.region_openings;
    for o in openings {
        eprintln!(
            "opening z {:.2} region {}: cell {:.3}, measured {:.3} -> {:.3} (inset {:.3}), \
             closing {:.2} / {:.2} mm2, {:?}, clips {:?} / {:?}",
            o.z_level,
            o.region,
            o.cell_mm,
            o.measured_before_mm,
            o.measured_after_mm,
            o.shortfall_inset_mm,
            o.excess_mm2,
            o.allowed_mm2,
            o.decision,
            o.stock_box_clip,
            o.centre_reach_clip
        );
    }
    assert!(
        !openings.is_empty(),
        "non-vacuity: the plan records no region opening"
    );
    // The cell of this fixture is over the nominal inset `SIMPLIFY_INSET + m`
    // (0.55 mm): the case P22f-1 opens.
    assert!(
        openings.iter().all(|o| o.cell_mm > K * 0.5 - 1e-9),
        "the fixture's planner cell is not R / 6"
    );
    let sweeps: Vec<_> = openings.iter().filter(|o| o.cuts_sweep()).collect();
    assert!(
        sweeps.is_empty(),
        "regions that keep the perimeter sweep: {sweeps:?}"
    );
    assert!(
        openings
            .iter()
            .all(|o| o.decision == OpeningDecision::Grown),
        "a region did not open on its grown outline"
    );
}
