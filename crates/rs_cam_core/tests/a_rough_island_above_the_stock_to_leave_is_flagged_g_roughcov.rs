//! G-ROUGHCOV / G-THINSTAND (adaptive research 2026-10-23, Phase 1.5).
//!
//! Claim: a 3D Rough that leaves an island of material more than its stock
//! to leave plus one cell above its own target is reported, with the
//! island's area, height and width; an island below that bar is not; and a
//! cell coarser than the stock to leave abstains.
//!
//! The target is the rough's own reachable floor (A3,
//! `stock::standing_islands::measure_rough_coverage`): the planner's
//! per-centre floor (`adaptive3d::intended_floor`: drop-cutter surface +
//! stock to leave, never below the last level), eroded by the tool disc.
//! So the floor a flat tool leaves in a valley or an inside corner is not
//! an island, a column no centre reaches is not compared, a strip left
//! against a wall IS an island (`touches_wall`), and a core left in a hole
//! the tool fits is an island.
//!
//! The stocks are synthetic dexel stocks; no simulation runs. Each
//! expected stock is computed here in closed form (the opening of the
//! model by a 6 mm flat disc), not by the measure. The finding is read
//! through `SimulationTriage::build_with_rest_context`, the one assembly
//! the session's triage entry points call.
//!
//! Falsifiers:
//!
//! * Remove the `rough_coverage` loop in
//!   `SimulationTriage::build_with_rest_context`: arm (a) fails.
//! * Read the target as the model + leave at the column (the A2 target):
//!   arm (e) flags the valley floor and arm (h) flags the hole corners.
//! * Drop the population (every column compared): arm (f) flags the
//!   fringe.
//! * Drop the wall mask: arm (g) reads `touches_wall` false.
//! * Tag a wall by the op's depth per pass (the A3 rule): arm (i) tags the
//!   sliver for one cutter and not for the other; sort tallest first only:
//!   arm (i) lists the sliver first.
//! * Skip off-model columns: arms (d) and (h) fail.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::collections::{BTreeMap, BTreeSet};

use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::diagnostics::ids;
use rs_cam_core::geo::{BoundingBox3, P2, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::stock::sim_measurability::MeasurabilityReport;
use rs_cam_core::stock::sim_triage::{SimulationTriage, TriageClass, TriageInputs};
use rs_cam_core::stock::simulation_cut::SimulationCutTrace;
use rs_cam_core::stock::standing_islands::{
    RoughCoverage, RoughCoverageAbstention, RoughCoverageOutcome, RoughCoverageReading,
    RoughTarget, measure_rough_coverage,
};
use rs_cam_core::tool::FlatEndmill;

const SIZE_MM: f64 = 40.0;
const STOCK_TOP_MM: f64 = 10.0;
const MODEL_Z_MM: f64 = 2.0;
const STL_MM: f64 = 0.5;
const CELL_MM: f64 = 0.25;
const DEPTH_PER_PASS_MM: f64 = 2.0;
/// The rough's cutter: a 6 mm flat end mill, R = 3 mm.
const TOOL_DIAMETER_MM: f64 = 6.0;
const TOOL_RADIUS_MM: f64 = TOOL_DIAMETER_MM / 2.0;

fn cutter() -> FlatEndmill {
    FlatEndmill::new(TOOL_DIAMETER_MM, 25.0)
}

/// The rough's target on `model`; `centre`: the tool-centre polygons, or
/// `None` for the boundary off.
fn rough_target<'a>(
    model: &'a TriangleMesh,
    index: &'a SpatialIndex,
    cutter: &'a FlatEndmill,
    stl: f64,
    centre: Option<&'a [Polygon2]>,
) -> RoughTarget<'a> {
    RoughTarget {
        mesh: model,
        index,
        cutter,
        stock_to_leave_mm: stl,
        pinned_floor_z: None,
        centre,
    }
}

/// No block: a box outside the grid.
const NO_BLOCK: [f64; 4] = [-9.0, -9.0, -8.0, -8.0];

fn flat(x0: f64, y0: f64, x1: f64, y1: f64, z: f64) -> (Vec<P3>, Vec<[u32; 3]>) {
    (
        vec![
            P3::new(x0, y0, z),
            P3::new(x1, y0, z),
            P3::new(x1, y1, z),
            P3::new(x0, y1, z),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
    )
}

fn mesh_of(parts: &[(Vec<P3>, Vec<[u32; 3]>)]) -> TriangleMesh {
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    for (v, t) in parts {
        let base = u32::try_from(vertices.len()).expect("small mesh");
        vertices.extend_from_slice(v);
        triangles.extend(t.iter().map(|[a, b, c]| [a + base, b + base, c + base]));
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// A full-height stock cut to `target(x, y)` everywhere except the cells
/// whose centre is in `block`, which are cut to `block_top` (or left at the
/// stock top when `None`).
fn stock(
    cell: f64,
    target: impl Fn(f64, f64) -> f64,
    block: [f64; 4],
    block_top: Option<f64>,
) -> TriDexelStock {
    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(SIZE_MM, SIZE_MM, STOCK_TOP_MM),
    };
    let mut stock = TriDexelStock::from_bounds(&bbox, cell);
    for row in 0..stock.z_grid.rows {
        for col in 0..stock.z_grid.cols {
            let (x, y) = stock.z_grid.cell_to_world(row, col);
            let in_block = x >= block[0] && x <= block[2] && y >= block[1] && y <= block[3];
            let z = match (in_block, block_top) {
                (true, Some(top)) => top,
                (true, None) => continue,
                (false, _) => target(x, y),
            };
            stock.clear_above_at(row, col, z as f32);
        }
    }
    stock
}

fn measured(outcome: RoughCoverageOutcome) -> RoughCoverage {
    match outcome {
        RoughCoverageOutcome::Measured(c) => c,
        RoughCoverageOutcome::NotMeasurable(why) => panic!("expected a reading, got {why:?}"),
    }
}

fn triage(reading: &RoughCoverageReading) -> SimulationTriage {
    let trace = SimulationCutTrace::from_samples(0.5, vec![]);
    let measurability = MeasurabilityReport::default();
    let steps: BTreeMap<ToolpathId, f64> = [(reading.toolpath_id, DEPTH_PER_PASS_MM)]
        .into_iter()
        .collect();
    let names = BTreeMap::new();
    let diameters = BTreeMap::new();
    let kinematics = BTreeMap::new();
    SimulationTriage::build_with_rest_context(
        &TriageInputs {
            trace: &trace,
            measurability: &measurability,
            diagnostics: &[],
            rapid_collisions: &[],
            toolpath_names: &names,
            holder_collisions: &[],
            obstacle_collisions: &[],
            body_contacts: &[],
            tool_diameters_mm: &diameters,
            commanded_depth_per_pass_mm: &steps,
            kinematic_utilization: &kinematics,
            region_of: None,
        },
        &BTreeSet::new(),
        &BTreeSet::new(),
        &BTreeMap::new(),
        std::slice::from_ref(reading),
    )
}

fn has_finding(t: &SimulationTriage) -> bool {
    t.findings()
        .any(|f| f.diagnostic.id.as_str() == ids::PROJECT_ROUGH_LEAVES_ISLANDS)
}

/// (a) One block left at the stock top over a flat model: one island,
/// 9 × 21 cells, 7.5 mm above the target, 9 cells wide. It stands more
/// than one 2 mm depth per pass above the target, so the class is safety.
#[test]
fn an_island_above_the_bar_is_one_island_with_its_area_height_and_width() {
    let model = mesh_of(&[flat(0.0, 0.0, SIZE_MM, SIZE_MM, MODEL_Z_MM)]);
    let index = SpatialIndex::build_auto(&model);
    let target_z = MODEL_Z_MM + STL_MM;
    let s = stock(CELL_MM, |_, _| target_z, [10.0, 10.0, 12.0, 15.0], None);
    let coverage = measured(measure_rough_coverage(
        &s,
        &rough_target(&model, &index, &cutter(), STL_MM, None),
    ));
    assert!((coverage.bar_mm - (STL_MM + CELL_MM)).abs() < 1e-12);
    assert_eq!(coverage.islands.len(), 1, "{:?}", coverage.islands);
    let island = coverage.islands.first().expect("one island");
    assert_eq!(island.cells, 9 * 21);
    assert!((island.area_mm2 - 189.0 * CELL_MM * CELL_MM).abs() < 1e-9);
    assert!(
        (island.max_standing_mm - (STOCK_TOP_MM - target_z)).abs() < 1e-5,
        "{island:?}"
    );
    assert!(
        (island.min_width_mm - 9.0 * CELL_MM).abs() < 1e-9,
        "{island:?}"
    );
    assert!(
        !island.touches_wall,
        "a block on open floor is not at a wall"
    );

    let reading = RoughCoverageReading {
        toolpath_id: ToolpathId::from_raw(1),
        outcome: RoughCoverageOutcome::Measured(coverage),
    };
    let t = triage(&reading);
    let f = t
        .safety
        .iter()
        .find(|f| f.diagnostic.id.as_str() == ids::PROJECT_ROUGH_LEAVES_ISLANDS)
        .expect("a 7.5 mm island over a 2 mm step is a safety finding");
    assert_eq!(f.class, TriageClass::Safety);
    assert_eq!(t.coverage, vec![reading]);
}

/// (b) The same block cut to 0.6 mm above the target: under the
/// `stock_to_leave + one cell` = 0.75 mm bar, so no island and no finding.
/// The reading is published as a measured zero.
#[test]
fn an_island_below_the_bar_is_not_reported() {
    let model = mesh_of(&[flat(0.0, 0.0, SIZE_MM, SIZE_MM, MODEL_Z_MM)]);
    let index = SpatialIndex::build_auto(&model);
    let target_z = MODEL_Z_MM + STL_MM;
    let s = stock(
        CELL_MM,
        |_, _| target_z,
        [10.0, 10.0, 12.0, 15.0],
        Some(target_z + 0.6),
    );
    let coverage = measured(measure_rough_coverage(
        &s,
        &rough_target(&model, &index, &cutter(), STL_MM, None),
    ));
    assert!(coverage.islands.is_empty(), "{:?}", coverage.islands);
    assert_eq!(coverage.above_spec_cells, 0);
    assert!(coverage.compared_cells > 0);
    let reading = RoughCoverageReading {
        toolpath_id: ToolpathId::from_raw(1),
        outcome: RoughCoverageOutcome::Measured(coverage),
    };
    let t = triage(&reading);
    assert!(!has_finding(&t));
    assert_eq!(t.coverage.len(), 1, "a measured zero is still published");
}

/// (c) A cell coarser than the stock to leave abstains, and the triage
/// raises nothing for it.
#[test]
fn a_cell_coarser_than_the_stock_to_leave_is_not_measurable() {
    let model = mesh_of(&[flat(0.0, 0.0, SIZE_MM, SIZE_MM, MODEL_Z_MM)]);
    let index = SpatialIndex::build_auto(&model);
    let coarse = 0.5;
    let stl = 0.25;
    let s = stock(
        coarse,
        |_, _| MODEL_Z_MM + stl,
        [10.0, 10.0, 12.0, 15.0],
        None,
    );
    let outcome = measure_rough_coverage(&s, &rough_target(&model, &index, &cutter(), stl, None));
    assert_eq!(
        outcome,
        RoughCoverageOutcome::NotMeasurable(RoughCoverageAbstention::CellCoarserThanStockToLeave {
            cell_mm: coarse,
            stock_to_leave_mm: stl,
        })
    );
    let t = triage(&RoughCoverageReading {
        toolpath_id: ToolpathId::from_raw(1),
        outcome,
    });
    assert!(!has_finding(&t));
}

/// (d) Where the model has no surface (a through hole, the band outside
/// the part), the target is the mesh-bbox floor plus the stock to leave.
/// A core left standing there is an island measured from that floor
/// (G-HOLECORE, G-OUTERBAND).
#[test]
fn a_core_where_the_model_has_no_surface_is_measured_from_the_floor() {
    let top_z = 5.0;
    let floor_z = 1.0;
    // The part covers x <= 20.1 (edges off the cell centres); a small low
    // face outside the stock sets the mesh-bbox floor.
    let model = mesh_of(&[
        flat(-1.0, -1.0, 20.1, SIZE_MM + 1.0, top_z),
        flat(50.0, 50.0, 51.0, 51.0, floor_z),
    ]);
    let index = SpatialIndex::build_auto(&model);
    let target = |x: f64, _y: f64| {
        if x <= 20.1 {
            top_z + STL_MM
        } else {
            floor_z + STL_MM
        }
    };
    let s = stock(CELL_MM, target, [28.0, 20.0, 30.0, 22.0], None);
    let coverage = measured(measure_rough_coverage(
        &s,
        &rough_target(&model, &index, &cutter(), STL_MM, None),
    ));
    assert_eq!(coverage.islands.len(), 1, "{:?}", coverage.islands);
    let island = coverage.islands.first().expect("one island");
    assert_eq!(island.cells, 9 * 9);
    assert!(
        (island.max_standing_mm - (STOCK_TOP_MM - floor_z - STL_MM)).abs() < 1e-5,
        "{island:?}"
    );
    assert!(!island.touches_wall, "{island:?}");
}

/// The opening of a V valley `z = 2 + SLOPE·|x − 20|` by the flat disc of
/// radius R: a disc centred on the valley line touches `2 + SLOPE·R`, and
/// no disc goes lower, so the floor is `max(model, 2 + SLOPE·R)`.
const VALLEY_SLOPE: f64 = 0.4;
const VALLEY_BOTTOM_Z: f64 = 2.0;
const VALLEY_X: f64 = 20.0;

fn valley_z(x: f64) -> f64 {
    VALLEY_BOTTOM_Z + VALLEY_SLOPE * (x - VALLEY_X).abs()
}

fn valley_model() -> TriangleMesh {
    let (x0, x1, y0, y1) = (-1.0, SIZE_MM + 1.0, -1.0, SIZE_MM + 1.0);
    let left = (
        vec![
            P3::new(x0, y0, valley_z(x0)),
            P3::new(VALLEY_X, y0, VALLEY_BOTTOM_Z),
            P3::new(VALLEY_X, y1, VALLEY_BOTTOM_Z),
            P3::new(x0, y1, valley_z(x0)),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
    );
    let right = (
        vec![
            P3::new(VALLEY_X, y0, VALLEY_BOTTOM_Z),
            P3::new(x1, y0, valley_z(x1)),
            P3::new(x1, y1, valley_z(x1)),
            P3::new(VALLEY_X, y1, VALLEY_BOTTOM_Z),
        ],
        vec![[0, 1, 2], [0, 2, 3]],
    );
    mesh_of(&[left, right])
}

/// (e) A sloped model cut in two or more draped levels. Every level drapes
/// in this planner, so the plan's floor on the slope is the opening of the
/// model by the tool (the valley bottom stands `SLOPE · R` = 1.2 mm above
/// the model, which the A2 target flagged): no island. A band held at one
/// level Z (6.5) over the slope instead of draping is NOT the plan's floor
/// and reads as ONE island, not at a wall.
#[test]
fn the_plan_floor_on_a_slope_reads_no_island_and_a_held_level_reads_one() {
    let model = valley_model();
    let index = SpatialIndex::build_auto(&model);
    let cutter = cutter();
    let opening = |x: f64| valley_z(x).max(VALLEY_BOTTOM_Z + VALLEY_SLOPE * TOOL_RADIUS_MM);
    let floor = |x: f64, _y: f64| opening(x) + STL_MM;

    let s = stock(CELL_MM, floor, NO_BLOCK, None);
    let coverage = measured(measure_rough_coverage(
        &s,
        &rough_target(&model, &index, &cutter, STL_MM, None),
    ));
    assert!(coverage.compared_cells > 0);
    assert!(
        coverage.islands.is_empty(),
        "{:?}",
        coverage.islands.first()
    );

    // The band x in [26, 30] held at Z 6.5: above spec where
    // 6.5 − (model + leave) > leave + cell, i.e. model < 5.25, x < 28.125.
    let level_z = 6.5;
    let held = |x: f64, y: f64| {
        if (26.0..=30.0).contains(&x) {
            floor(x, y).max(level_z)
        } else {
            floor(x, y)
        }
    };
    let s = stock(CELL_MM, held, NO_BLOCK, None);
    let coverage = measured(measure_rough_coverage(
        &s,
        &rough_target(&model, &index, &cutter, STL_MM, None),
    ));
    assert_eq!(coverage.islands.len(), 1, "{:?}", coverage.islands);
    let island = coverage.islands.first().expect("one island");
    let rows = (SIZE_MM / CELL_MM) as usize + 1;
    assert_eq!(island.cells, 9 * rows, "{island:?}");
    assert!(
        (island.max_standing_mm - (level_z - valley_z(26.0) - STL_MM)).abs() < 1e-4,
        "{island:?}"
    );
    assert!(island.bbox[0] >= 26.0 - 1e-9 && island.bbox[2] <= 28.0 + 1e-9);
    assert!(!island.touches_wall, "{island:?}");
}

/// The tool-centre square of arm (f): cell centres 10.25 ..= 29.75 inside.
fn centre_square() -> Vec<Polygon2> {
    vec![Polygon2::new(vec![
        P2::new(10.1, 10.1),
        P2::new(29.9, 10.1),
        P2::new(29.9, 29.9),
        P2::new(10.1, 29.9),
    ])]
}

/// (f) A column farther than R from every centre the tool may take is
/// outside the population: a full-height fringe at x = 33.0 (3.25 mm from
/// the last centre at 29.75) reads nothing. A full-height strip at
/// x = 32.75 (3.00 mm, inside the reach) reads as an island, and it stands
/// beside the uncut fringe, so it touches a wall (the edge of the reach).
#[test]
fn a_fringe_outside_the_reach_reads_nothing_and_a_strip_inside_it_reads() {
    let model = mesh_of(&[flat(0.0, 0.0, SIZE_MM, SIZE_MM, MODEL_Z_MM)]);
    let index = SpatialIndex::build_auto(&model);
    let cutter = cutter();
    let centre = centre_square();
    let target_z = MODEL_Z_MM + STL_MM;

    let fringe = stock(CELL_MM, |_, _| target_z, [32.9, 15.0, 33.1, 25.0], None);
    let coverage = measured(measure_rough_coverage(
        &fringe,
        &rough_target(&model, &index, &cutter, STL_MM, Some(&centre)),
    ));
    assert!(coverage.compared_cells > 0);
    assert!(coverage.islands.is_empty(), "{:?}", coverage.islands);

    let strip = stock(CELL_MM, |_, _| target_z, [32.7, 15.0, 33.1, 25.0], None);
    let coverage = measured(measure_rough_coverage(
        &strip,
        &rough_target(&model, &index, &cutter, STL_MM, Some(&centre)),
    ));
    assert_eq!(coverage.islands.len(), 1, "{:?}", coverage.islands);
    let island = coverage.islands.first().expect("one island");
    assert_eq!(island.cells, 41, "the column x = 32.75 only: {island:?}");
    assert!(island.touches_wall, "{island:?}");
}

/// The plateau of arms (d) and (g): Z 5 for x <= 20.1, a vertical wall at
/// x = 20.1, and a small low face outside the stock that sets the mesh-bbox
/// floor at Z 1.
const PLATEAU_Z: f64 = 5.0;
const PLATEAU_EDGE_X: f64 = 20.1;
const FLOOR_Z: f64 = 1.0;

/// (g) A 1 mm strip (four cells, x 20.25 ..= 21.0) left at the plateau top
/// against the vertical wall: the tool reaches it (a centre R from the
/// wall puts the cutter edge on the wall), so it is an island, 4 mm high,
/// and it touches the wall (the target rises 4 mm > one 2 mm step).
#[test]
fn a_strip_against_a_vertical_wall_reads_and_touches_the_wall() {
    let model = mesh_of(&[
        flat(-1.0, -1.0, PLATEAU_EDGE_X, SIZE_MM + 1.0, PLATEAU_Z),
        flat(50.0, 50.0, 51.0, 51.0, FLOOR_Z),
    ]);
    let index = SpatialIndex::build_auto(&model);
    let cutter = cutter();
    let target = |x: f64, _y: f64| {
        if x <= PLATEAU_EDGE_X {
            PLATEAU_Z + STL_MM
        } else {
            FLOOR_Z + STL_MM
        }
    };
    let s = stock(
        CELL_MM,
        target,
        [20.2, -1.0, 21.1, SIZE_MM + 1.0],
        Some(PLATEAU_Z + STL_MM),
    );
    let coverage = measured(measure_rough_coverage(
        &s,
        &rough_target(&model, &index, &cutter, STL_MM, None),
    ));
    assert_eq!(coverage.islands.len(), 1, "{:?}", coverage.islands);
    let island = coverage.islands.first().expect("one island");
    let rows = (SIZE_MM / CELL_MM) as usize + 1;
    assert_eq!(island.cells, 4 * rows, "{island:?}");
    assert!(
        (island.max_standing_mm - (PLATEAU_Z - FLOOR_Z)).abs() < 1e-4,
        "{island:?}"
    );
    assert!(island.touches_wall, "{island:?}");
}

/// (h) A plate at Z 5 with a 10 mm square through hole (15.1 ..= 25.1).
/// The 6 mm tool takes the floor only with its centre in 18.1 ..= 22.1, so
/// the plan reaches the floor within R of that square and leaves the hole
/// corners at the plate. A stock cut to that floor with a 1 mm core left
/// at the stock top in the middle reads ONE island, the core, measured
/// from the floor and not at a wall; the corner residue reads nothing.
#[test]
fn a_hole_core_reads_and_the_corner_residue_does_not() {
    let model = plate_with_hole();
    let index = SpatialIndex::build_auto(&model);
    let cutter = cutter();
    let target = |x: f64, y: f64| hole_target(x, y, TOOL_RADIUS_MM);
    let s = stock(CELL_MM, target, [19.5, 19.5, 20.5, 20.5], None);
    let coverage = measured(measure_rough_coverage(
        &s,
        &rough_target(&model, &index, &cutter, STL_MM, None),
    ));
    assert_eq!(coverage.islands.len(), 1, "{:?}", coverage.islands);
    let island = coverage.islands.first().expect("one island");
    assert_eq!(island.cells, 5 * 5, "{island:?}");
    assert!(
        (island.max_standing_mm - (STOCK_TOP_MM - FLOOR_Z - STL_MM)).abs() < 1e-4,
        "{island:?}"
    );
    assert!(!island.touches_wall, "{island:?}");
}

/// The square through hole of arms (h) and (i).
const HOLE_LO: f64 = 15.1;
const HOLE_HI: f64 = 25.1;

/// A plate at Z 5 with the square through hole, and a small low face
/// outside the stock that sets the mesh-bbox floor at Z 1.
fn plate_with_hole() -> TriangleMesh {
    let (h0, h1) = (HOLE_LO, HOLE_HI);
    let (lo, hi) = (-1.0, SIZE_MM + 1.0);
    mesh_of(&[
        flat(lo, lo, h0, hi, PLATEAU_Z),
        flat(h1, lo, hi, hi, PLATEAU_Z),
        flat(h0, lo, h1, h0, PLATEAU_Z),
        flat(h0, h1, h1, hi, PLATEAU_Z),
        flat(50.0, 50.0, 51.0, 51.0, FLOOR_Z),
    ])
}

/// The opening of the plate by a flat disc of `radius`, plus the leave:
/// the floor where the column is in the hole within `radius` of the
/// square of centres whose disc fits the hole, the plate elsewhere.
fn hole_target(x: f64, y: f64, radius: f64) -> f64 {
    let (c0, c1) = (HOLE_LO + radius, HOLE_HI - radius);
    let in_hole = x > HOLE_LO && x < HOLE_HI && y > HOLE_LO && y < HOLE_HI;
    let dx = (c0 - x).max(x - c1).max(0.0);
    let dy = (c0 - y).max(y - c1).max(0.0);
    if in_hole && dx.hypot(dy) <= radius {
        FLOOR_Z + STL_MM
    } else {
        PLATEAU_Z + STL_MM
    }
}

/// (i) A4: the wall tag reads the island, not the op. A one-cell sliver at
/// the hole rim (x = 15.25, y 19 ..= 21) left at the plate top + leave
/// stands 4 mm, as high as the hole wall rises. The 6 mm cutter and the
/// 3 mm cutter both reach the floor there, and both tag the sliver at a
/// wall. A core in the middle of the hole left 1.5 mm proud is SHORTER
/// than the sliver, stays free, and comes first in the list and in the
/// narration.
#[test]
fn the_wall_tag_reads_the_island_and_free_islands_come_first() {
    let model = plate_with_hole();
    let index = SpatialIndex::build_auto(&model);
    let core_top = FLOOR_Z + STL_MM + 1.5;
    for diameter in [TOOL_DIAMETER_MM, TOOL_DIAMETER_MM / 2.0] {
        let cutter = FlatEndmill::new(diameter, 25.0);
        let radius = diameter / 2.0;
        let target = |x: f64, y: f64| {
            let in_core = (19.5..=20.5).contains(&x) && (19.5..=20.5).contains(&y);
            let in_sliver = (15.2..=15.3).contains(&x) && (19.0..=21.0).contains(&y);
            if in_core {
                core_top
            } else if in_sliver {
                PLATEAU_Z + STL_MM
            } else {
                hole_target(x, y, radius)
            }
        };
        let s = stock(CELL_MM, target, NO_BLOCK, None);
        let coverage = measured(measure_rough_coverage(
            &s,
            &rough_target(&model, &index, &cutter, STL_MM, None),
        ));
        assert_eq!(
            coverage.islands.len(),
            2,
            "{diameter} mm: {:?}",
            coverage.islands
        );
        let core = coverage.islands.first().expect("the free core");
        let sliver = coverage.islands.get(1).expect("the sliver at the wall");
        assert_eq!(core.cells, 5 * 5, "{diameter} mm: {core:?}");
        assert!(!core.touches_wall, "{diameter} mm: {core:?}");
        assert_eq!(sliver.cells, 9, "{diameter} mm: {sliver:?}");
        assert!(sliver.touches_wall, "{diameter} mm: {sliver:?}");
        assert!(sliver.max_standing_mm > core.max_standing_mm);
        let reading = RoughCoverageReading {
            toolpath_id: ToolpathId::from_raw(1),
            outcome: RoughCoverageOutcome::Measured(coverage),
        };
        let sentence = reading.narration_sentence();
        assert!(
            sentence.starts_with("coverage: 1 free island(s)")
                && sentence.contains("1 more touch a wall"),
            "{sentence}"
        );
    }
}
