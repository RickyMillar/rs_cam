//! G-ROUGHCOV / G-THINSTAND (adaptive research 2026-10-23, Phase 1.5).
//!
//! Claim: a 3D Rough that leaves an island of material more than its stock
//! to leave plus one cell above its own target is reported, with the
//! island's area, height and width; an island below that bar is not; and a
//! cell coarser than the stock to leave abstains.
//!
//! The target is the rough's own: `model + stock_to_leave`, and where the
//! model has no surface (a through hole, the band outside the part) the
//! mesh-bbox floor plus the stock to leave, as the 3D Rough planner reads
//! it (`stock::standing_islands::measure_rough_coverage`).
//!
//! The stocks are synthetic dexel stocks; no simulation runs. The finding
//! is read through `SimulationTriage::build_with_rest_context`, the one
//! assembly the session's triage entry points call.
//!
//! Falsifier: remove the `rough_coverage` loop in
//! `SimulationTriage::build_with_rest_context`, and arm (a) fails; change
//! the target to skip off-model columns, and arm (d) fails.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::collections::{BTreeMap, BTreeSet};

use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::diagnostics::ids;
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::stock::sim_measurability::MeasurabilityReport;
use rs_cam_core::stock::sim_triage::{SimulationTriage, TriageClass, TriageInputs};
use rs_cam_core::stock::simulation_cut::SimulationCutTrace;
use rs_cam_core::stock::standing_islands::{
    RoughCoverage, RoughCoverageAbstention, RoughCoverageOutcome, RoughCoverageReading,
    RoughTarget, measure_rough_coverage,
};

const SIZE_MM: f64 = 40.0;
const STOCK_TOP_MM: f64 = 10.0;
const MODEL_Z_MM: f64 = 2.0;
const STL_MM: f64 = 0.5;
const CELL_MM: f64 = 0.25;
const DEPTH_PER_PASS_MM: f64 = 2.0;

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
        &RoughTarget {
            mesh: &model,
            index: &index,
            stock_to_leave_mm: STL_MM,
            pinned_floor_z: None,
            reach: None,
        },
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
    assert!((island.min_width_mm - 9.0 * CELL_MM).abs() < 1e-9, "{island:?}");

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
        &RoughTarget {
            mesh: &model,
            index: &index,
            stock_to_leave_mm: STL_MM,
            pinned_floor_z: None,
            reach: None,
        },
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
    let s = stock(coarse, |_, _| MODEL_Z_MM + stl, [10.0, 10.0, 12.0, 15.0], None);
    let outcome = measure_rough_coverage(
        &s,
        &RoughTarget {
            mesh: &model,
            index: &index,
            stock_to_leave_mm: stl,
            pinned_floor_z: None,
            reach: None,
        },
    );
    assert_eq!(
        outcome,
        RoughCoverageOutcome::NotMeasurable(
            RoughCoverageAbstention::CellCoarserThanStockToLeave {
                cell_mm: coarse,
                stock_to_leave_mm: stl,
            }
        )
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
        &RoughTarget {
            mesh: &model,
            index: &index,
            stock_to_leave_mm: STL_MM,
            pinned_floor_z: None,
            reach: None,
        },
    ));
    assert_eq!(coverage.islands.len(), 1, "{:?}", coverage.islands);
    let island = coverage.islands.first().expect("one island");
    assert_eq!(island.cells, 9 * 9);
    assert!(
        (island.max_standing_mm - (STOCK_TOP_MM - floor_z - STL_MM)).abs() < 1e-5,
        "{island:?}"
    );
}
