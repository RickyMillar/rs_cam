//! P39: the ramp entry of a 3D Rough is the shape that the planner chose.
//!
//! Record: `planning/feeds_contract_2026-11-03/phases/P39_RAMP_ENTRY_STOCK_2026-11-17.md`.
//!
//! ## The defect
//!
//! The planner read the stock on the tool disc at the entry XY. The emitter
//! shipped a zigzag along +X, up to 19 mm long at 3 degrees, and no part of
//! the path read the stock, the model or the centre boundary along the legs
//! before the model clip. On the Wanaka back (toolpath id 2):
//!
//! - a leg met stock outside the read disc at its full height (7.2 mm);
//! - the model clip lifted a leg onto a ledge of the model, and the leg rode
//!   the ledge under 14.5 mm of stock (the `project.entry_load` reading).
//!
//! `adaptive3d_entry_stock_aware` did not see it: it reads the material
//! under the tool at the END of each fed descent, never under a flat piece
//! and never along the leg.
//!
//! ## What this sentry holds
//!
//! It replays the EMITTED path on a fresh dexel stock, move by move. For
//! each `EntryRamp` move:
//!
//! - the stock under the tool along the move stands at most one lap above
//!   the cutter (`ENTRY_CLEARANCE`, the descent of one lap of the zigzag);
//! - the tool centre stays inside the centre boundary of the op;
//! - where a direction has a run that the model clip does not lift (the two
//!   first fixtures), the move goes down: the planner took that direction.
//!
//! On the terrain fixture an entry can stand on a slope, where every
//! direction lifts. The leg then ships its whole clipped profile, and the
//! first rule still holds.
//!
//! Red without the rule (`adaptive3d::entry_ramp::plan`): the boundary
//! fixture read 6.02 mm (Contour Parallel) and the ledge fixture 8.00 mm
//! with 3 530 flat or rising moves (Geodesic Adaptive).

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the test prints its measurements for the reader"
)]

mod common;

use rs_cam_core::adaptive3d::{
    Adaptive3dDepth, Adaptive3dGeometry, Adaptive3dLinking, Adaptive3dParams, ClearingStrategy3d,
    EntryStyle3d, RegionOrdering,
};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh, make_test_flat};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::stock::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use rs_cam_core::tool::{FlatEndmill, MillingCutter};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};

const PLATE: f64 = 40.0;
const STOCK_TOP_Z: f64 = 16.0;
const SAFE_Z: f64 = 21.0;
const DPP: f64 = 8.0;
const CELL: f64 = 0.25;
/// The replay reads material a little inside the tool footprint, so a
/// dexel cell on the footprint edge is not read as a bite (the inset of
/// `adaptive3d_entry_stock_aware`).
const PROBE_INSET: f64 = 2.0 * CELL;
/// Dexel noise on a depth reading (as `adaptive3d_entry_stock_aware`).
const DEPTH_TOL: f64 = 0.25;
/// One lap of the ramp zigzag descends this (`dressup::ENTRY_CLEARANCE`).
const ONE_LAP_MM: f64 = 2.0;
/// Half the side of the centre boundary of the boundary fixture.
const CENTRE_HALF: f64 = 10.0;
/// The top of the ledge of the ledge fixture.
const LEDGE_Z: f64 = 10.0;

fn params(
    strategy: ClearingStrategy3d,
    centre_boundary: Vec<Polygon2>,
    gouge_probe: Option<std::sync::Arc<dyn MillingCutter>>,
) -> Adaptive3dParams {
    Adaptive3dParams {
        entry_clearance_mm: 0.5,
        ramp_feed_rate: None,
        geometry: Adaptive3dGeometry {
            tool_radius: 3.0,
            envelope_radius: 3.0,
            stepover: 1.8,
            tolerance: 0.1,
            segment_merge_tolerance: None,
            min_cutting_radius: 0.0,
            boundary: None,
            world_stock_xy_bbox: None,
            centre_boundary,
            gouge_probe,
        },
        depth: Adaptive3dDepth {
            depth_per_pass: DPP,
            stock_to_leave: 0.0,
            stock_top_z: STOCK_TOP_Z,
            z_floor: None,
            detect_flat_areas: false,
            rest_top_z: None,
        },
        linking: Adaptive3dLinking {
            region_ordering: RegionOrdering::ByArea,
            min_region_cut_length_mm: 0.0,
            max_stay_down_distance_mm: None,
            stay_down_clearance_mm: 0.5,
            step_split: None,
            keep_down_route: None,
            level_contour: None,
            rest_region_filter: None,
        },
        trochoid_cap_mult: 1.6,
        engagement_measure: rs_cam_core::adaptive::EngagementMeasure::DiskArea,
        feed_rate: 2400.0,
        plunge_rate: 500.0,
        safe_z: SAFE_Z,
        entry_style: EntryStyle3d::Ramp { max_angle_deg: 3.0 },
        initial_stock: None,
        clearing_strategy: strategy,
    }
}

/// A floor at Z 0 for X under 0 and a ledge at `LEDGE_Z` for X over 0, with
/// the wall between them.
fn ledge_mesh() -> TriangleMesh {
    let h = PLATE / 2.0;
    let vertices = vec![
        P3::new(-h, -h, 0.0),
        P3::new(0.0, -h, 0.0),
        P3::new(0.0, h, 0.0),
        P3::new(-h, h, 0.0),
        P3::new(0.0, -h, LEDGE_Z),
        P3::new(h, -h, LEDGE_Z),
        P3::new(h, h, LEDGE_Z),
        P3::new(0.0, h, LEDGE_Z),
    ];
    let triangles = vec![
        [0, 1, 2],
        [0, 2, 3],
        [1, 4, 7],
        [1, 7, 2],
        [4, 5, 6],
        [4, 6, 7],
    ];
    TriangleMesh::from_raw(vertices, triangles)
}

fn generate(
    mesh: &TriangleMesh,
    strategy: ClearingStrategy3d,
    centre: Vec<Polygon2>,
) -> (Toolpath, FlatEndmill) {
    generate_with_probe(mesh, strategy, centre, None)
}

/// [`generate`] with the reduced tool of the model gouge guard
/// (`Adaptive3dGeometry::gouge_probe`): with it the chord lift of the cuts
/// and, P40, of the entries is on.
fn generate_with_probe(
    mesh: &TriangleMesh,
    strategy: ClearingStrategy3d,
    centre: Vec<Polygon2>,
    gouge_probe: Option<std::sync::Arc<dyn MillingCutter>>,
) -> (Toolpath, FlatEndmill) {
    let index = SpatialIndex::build(mesh, 8.0);
    let tool = FlatEndmill::new(6.0, 25.0);
    let tp = rs_cam_core::adaptive3d::adaptive_3d_toolpath_output_with_cancel(
        mesh,
        &index,
        &tool,
        &params(strategy, centre, gouge_probe),
        &rs_cam_core::interrupt::NeverCancel,
        None,
    )
    .expect("NeverCancel never cancels")
    .toolpath;
    assert!(!tp.moves.is_empty(), "the rough emitted nothing");
    (tp, tool)
}

/// What the replay reads of the ramp moves of one toolpath.
#[derive(Debug, Default)]
struct RampAudit {
    ramp_moves: usize,
    /// Ramp moves that are flat or go up (move index, start, end).
    lifted: Vec<(usize, P3, P3)>,
    /// The most stock above the cutter along one ramp move (mm), and the move.
    worst_bite: f64,
    worst_at: Option<(usize, P3, P3)>,
    /// How far a ramp point lies outside the square of `half` (mm).
    worst_outside: f64,
}

fn audit(tp: &Toolpath, tool: &FlatEndmill, centre_half: Option<f64>) -> RampAudit {
    let h = PLATE / 2.0;
    let mut stock = TriDexelStock::from_bounds(
        &BoundingBox3 {
            min: P3::new(-h, -h, -2.0),
            max: P3::new(h, h, STOCK_TOP_Z),
        },
        CELL,
    );
    let r = tool.radius();
    let lut = RadialProfileLUT::from_cutter(tool, LUT_SAMPLES);
    let mut out = RampAudit::default();
    for i in 1..tp.moves.len() {
        let a = tp.moves[i - 1].target;
        let m = &tp.moves[i];
        let b = m.target;
        if matches!(m.move_type, MoveType::Rapid) {
            continue;
        }
        if m.intent == MoveIntent::EntryRamp {
            out.ramp_moves += 1;
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let xy = dx.hypot(dy);
            if xy > 1e-6 && b.z >= a.z - 1e-9 {
                out.lifted.push((i, a, b));
            }
            let n = (xy / (CELL * 0.5)).ceil().max(1.0) as usize;
            for k in 0..=n {
                let t = k as f64 / n as f64;
                let p = P3::new(a.x + dx * t, a.y + dy * t, a.z + (b.z - a.z) * t);
                let bite = stock
                    .max_top_z_in_disc(p.x, p.y, r - PROBE_INSET)
                    .map_or(0.0, |top| top - p.z);
                if bite > out.worst_bite {
                    out.worst_bite = bite;
                    out.worst_at = Some((i, a, b));
                }
            }
            if let Some(half) = centre_half {
                out.worst_outside = out
                    .worst_outside
                    .max(b.x.abs() - half)
                    .max(b.y.abs() - half);
            }
        }
        stock.stamp_linear_segment(&lut, r, a, b, StockCutDirection::FromTop);
    }
    out
}

fn check(label: &str, a: &RampAudit, a_free_direction_exists: bool) {
    eprintln!(
        "{label}: {} ramp moves, {} flat or rising, worst bite {:.2} mm at {:?}, outside {:.2} mm",
        a.ramp_moves,
        a.lifted.len(),
        a.worst_bite,
        a.worst_at,
        a.worst_outside
    );
    assert!(a.ramp_moves > 0, "{label}: the fixture ships no ramp move");
    assert!(
        !a_free_direction_exists || a.lifted.is_empty(),
        "{label}: {} ramp moves are flat or go up (the model clip lifted the leg); the first: {:?}",
        a.lifted.len(),
        a.lifted.first()
    );
    assert!(
        a.worst_bite <= ONE_LAP_MM + DEPTH_TOL,
        "{label}: a ramp move meets stock {:.2} mm above the cutter (one lap is {ONE_LAP_MM} mm) at {:?}",
        a.worst_bite,
        a.worst_at
    );
    assert!(
        a.worst_outside <= 0.02,
        "{label}: a ramp point is {:.2} mm outside the centre boundary",
        a.worst_outside
    );
}

const STRATEGIES: [(ClearingStrategy3d, &str); 2] = [
    (ClearingStrategy3d::ContourParallel, "contour parallel"),
    (ClearingStrategy3d::GeodesicAdaptive, "geodesic adaptive"),
];

/// A flat plate under 16 mm of stock, with a centre boundary 20 mm square.
/// The stock outside the boundary stands at 16 mm while the level at Z 0
/// enters from a top at Z 8. Before P39 a leg along +X left the boundary
/// and met that stock.
#[test]
fn a_ramp_leg_stays_inside_the_centre_boundary_and_under_no_taller_stock() {
    let square = Polygon2::rectangle(-CENTRE_HALF, -CENTRE_HALF, CENTRE_HALF, CENTRE_HALF);
    for (strategy, name) in STRATEGIES {
        let (tp, tool) = generate(&make_test_flat(PLATE), strategy, vec![square.clone()]);
        check(
            &format!("boundary, {name}"),
            &audit(&tp, &tool, Some(CENTRE_HALF)),
            true,
        );
    }
}

/// A floor at Z 0 beside a ledge at Z 10, under 16 mm of stock. Before P39
/// a leg along +X went over the ledge: the model clip lifted it, and it rode
/// the ledge under the stock that no lap had lowered.
#[test]
fn a_ramp_leg_does_not_ride_a_ledge_of_the_model() {
    for (strategy, name) in STRATEGIES {
        let (tp, tool) = generate(&ledge_mesh(), strategy, Vec::new());
        check(&format!("ledge, {name}"), &audit(&tp, &tool, None), true);
    }
}

/// A rolling terrain (the terrain of `adaptive3d_entry_stock_aware`) under
/// 16 mm of stock. An entry can stand on a slope, so a leg can be lifted;
/// no ramp move meets more than one lap of stock.
#[test]
fn a_lifted_ramp_leg_still_takes_one_lap_at_most() {
    let mesh = common::meshes::height_field(PLATE / 2.0, 1.0, |x, y| {
        2.0 + 11.0 * ((x / 3.5).sin() * (y / 3.5).sin()).max(0.0)
    });
    for (strategy, name) in STRATEGIES {
        let (tp, tool) = generate(&mesh, strategy, Vec::new());
        check(&format!("terrain, {name}"), &audit(&tp, &tool, None), false);
    }
}

/// P40 A: a ramp leg that the model clip lifts over a wall of the model
/// holds the model on each chord (the 3D Rough door of
/// `dressup::entry_descent::hold_entry_chords`).
///
/// Record: `planning/feeds_contract_2026-11-03/phases/P40_WALL_CHORD_ONE_PASS_2026-11-17.md`.
///
/// The fixture is the ledge of this file with a centre boundary that is a
/// strip along X: X -5.9 to 15, Y -1 to 1. The tool stands on the floor
/// while its centre is under X -3. Inside the strip no direction has a run
/// of 3 mm (the shortest run, the tool radius) that the clip does not lift,
/// so the planner takes the longest run that the centre may take: +X, over
/// the ledge. The clip lifts the leg onto the ledge, and a chord of each
/// lap goes over the wall line.
///
/// The op states the reduced tool of the model gouge guard, as the session
/// does (`gouge_probe`), so the chord lift is on. The judge is the rule of
/// the guard (`compute::model_gouge`, `Allowance`: the reduced tool, the
/// limit `CL Z + leave - tolerance`, the bar `MODEL_GOUGE_Z_TOL_MM`), read
/// at `MODEL_GOUGE_TOL_MM` along each `EntryRamp` move. The guard itself
/// reads a move at a quarter of the diameter.
///
/// Contour Parallel only: Geodesic Adaptive emits no move inside this strip.
///
/// Red with `adaptive3d::path::EntryFrame::chord_lift` returning `None`.
#[test]
fn a_lifted_ramp_leg_over_a_wall_holds_the_model_p40() {
    use rs_cam_core::compute::model_gouge::{MODEL_GOUGE_TOL_MM, MODEL_GOUGE_Z_TOL_MM};
    /// `params`: the chord tolerance of the op.
    const TOL: f64 = 0.1;
    let mesh = ledge_mesh();
    let index = SpatialIndex::build(&mesh, 8.0);
    let strip = Polygon2::rectangle(-5.9, -1.0, 15.0, 1.0);
    for (strategy, name) in STRATEGIES
        .into_iter()
        .filter(|(s, _)| *s == ClearingStrategy3d::ContourParallel)
    {
        let reduced = FlatEndmill::new(6.0 - 2.0 * TOL, 25.0);
        let (tp, tool) = generate_with_probe(
            &mesh,
            strategy,
            vec![strip.clone()],
            Some(std::sync::Arc::new(reduced)),
        );
        let judge = FlatEndmill::new(6.0 - 2.0 * TOL, 25.0);
        let (mut worst, mut worst_at) = (0.0_f64, None);
        let (mut ramp_moves, mut on_ledge) = (0_usize, 0_usize);
        for i in 1..tp.moves.len() {
            let (a, m) = (tp.moves[i - 1].target, &tp.moves[i]);
            let b = m.target;
            if m.intent != MoveIntent::EntryRamp {
                continue;
            }
            ramp_moves += 1;
            if b.x > -3.0 && (b.z - LEDGE_Z).abs() < 1e-6 {
                on_ledge += 1;
            }
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let n = (dx.hypot(dy) / MODEL_GOUGE_TOL_MM).ceil().max(1.0) as usize;
            for k in 0..=n {
                let t = k as f64 / n as f64;
                let cl = rs_cam_core::surface::dropcutter::point_drop_cutter(
                    a.x + dx * t,
                    a.y + dy * t,
                    &mesh,
                    &index,
                    &judge,
                );
                let under = cl.z - TOL - (a.z + (b.z - a.z) * t);
                if cl.contacted && under > worst {
                    worst = under;
                    worst_at = Some((i, a, b));
                }
            }
        }
        eprintln!(
            "wall, {name}: {ramp_moves} ramp moves, {on_ledge} end on the ledge, deepest chord \
             {worst:.3} mm under the limit at {worst_at:?}"
        );
        assert!(
            on_ledge > 0,
            "wall, {name}: no ramp move ends on the ledge ({ramp_moves} ramp moves): the fixture \
             does not lift a leg"
        );
        assert!(
            worst <= MODEL_GOUGE_Z_TOL_MM,
            "wall, {name}: a ramp chord is {worst:.3} mm under the model plus the leave less the \
             tolerance at {worst_at:?}"
        );
        // The one-lap rule of P39 holds with the lift on.
        check(&format!("wall, {name}"), &audit(&tp, &tool, None), false);
    }
}
