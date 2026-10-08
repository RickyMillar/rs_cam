//! M2 + M3 sentries (memory programme 2026-10-01): a checkpoint keeps the
//! inputs of its display mesh, not the mesh, and its local stock snapshot is
//! the next toolpath's prior stock.
//!
//! Before M2, `push_checkpoint` built a marching-cubes mesh for every
//! toolpath (about 96 B per grid cell each) and cloned the group stock twice
//! at the same point in the carve: once for the checkpoint's mesh, once as
//! the next toolpath's `prior_stocks` entry. The checkpoint now holds an
//! `Arc` of the local stock and `SimCheckpointMesh::build_mesh` builds the
//! mesh on demand.
//!
//! The arms:
//!
//! 1. The checkpoint of toolpath k and the prior stock of toolpath k+1 are
//!    ONE allocation (`Arc::ptr_eq`), and the tail phantom shares the last
//!    checkpoint's.
//! 2. `build_mesh` reads the LOCAL stock, not the playback stock. The fixture
//!    stock has a non-zero origin, so the two frames differ; the arm rebuilds
//!    the mesh from `prior_stocks[k+1]` with the public functions and asks
//!    for the same bits.
//! 3. Two builds give the same bits: the scrub cache may drop a mesh and
//!    build it again.
//!
//! The old-versus-new bit check (the eager mesh of `fd06f407` against
//! `build_mesh`) is the ignored instrument in `checkpoint_mesh_ab_m2.rs`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::simulate::{
    SimGroupEntry, SimToolpathEntry, SimulationRequest, SimulationResult, run_simulation,
    transform_stock_mesh_to_global,
};
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::dexel_stock::StockCutDirection;
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::stock::dexel_mesh::dexel_stock_to_mesh;
use rs_cam_core::stock::stock_mesh::StockMesh;
use rs_cam_core::tool::{FlatEndmill, ToolDefinition};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

/// The id of the pending rest operation the tail phantom stands for.
const PHANTOM_ID: ToolpathId = ToolpathId(99);

/// Number of carved toolpaths in the one group.
const PASSES: usize = 3;

/// A stock with a NON-ZERO origin. An identity group's local grid is in the
/// world frame and the playback grid is zero-rooted, so a mesh built from
/// the wrong one lands shifted by this origin.
fn stock() -> BoundingBox3 {
    BoundingBox3 {
        min: P3::new(-20.0, -10.0, -6.0),
        max: P3::new(20.0, 14.0, 0.0),
    }
}

fn tool() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(FlatEndmill::new(6.0, 25.0)),
        6.0,
        20.0,
        25.0,
        45.0,
        2,
        ToolMaterial::Carbide,
    )
}

/// A raster pass in WORLD coordinates, one step deeper and further in Y per
/// index, so each pass cuts new material.
fn pass(pass_index: usize) -> Arc<AnnotatedToolpath> {
    let y0 = -6.0 + 3.0 * pass_index as f64;
    let depth = -1.0 - 0.8 * pass_index as f64;
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(-17.0, y0, 5.0));
    for lane in 0..3 {
        let y = y0 + 2.0 * lane as f64;
        let (x_a, x_b) = if lane % 2 == 0 {
            (-17.0, 17.0)
        } else {
            (17.0, -17.0)
        };
        tp.feed_to(P3::new(x_a, y, depth), 900.0);
        tp.feed_to(P3::new(x_b, y, depth), 900.0);
    }
    tp.rapid_to(P3::new(17.0, y0, 5.0));
    Arc::new(AnnotatedToolpath::new(tp))
}

fn entry(index: usize) -> SimToolpathEntry {
    SimToolpathEntry {
        id: ToolpathId(index + 1),
        name: format!("Pass{index}"),
        annotated: pass(index),
        tool: Arc::new(tool()),
        flute_count: 2,
        tool_summary: "6mm Flat".to_owned(),
        semantic_trace: None,
        spindle_rpm: None,
        metrics_not_applicable: false,
        drill_op: None,
        operation_config_hash: index as u64,
    }
}

fn simulate() -> SimulationResult {
    let request = SimulationRequest {
        groups: vec![SimGroupEntry {
            toolpaths: (0..PASSES).map(entry).collect(),
            direction: StockCutDirection::FromTop,
            local_stock_bbox: None,
            local_to_global: None,
            // The first pending rest operation sits after every carved one.
            phantom_prior_stock: Some((PASSES, PHANTOM_ID)),
            stock_changes: Vec::new(),
        }],
        stock_bbox: stock(),
        stock_top_z: 0.0,
        resolution: 0.5,
        spindle_rpm: 18_000,
        rapid_feed_mm_min: 5000.0,
        model_mesh: None,
        kinematics: None,
        display_stride: 1,
    };
    run_simulation(&request, &AtomicBool::new(false)).expect("simulation completes")
}

fn same_bits(a: &StockMesh, b: &StockMesh) -> bool {
    a.indices == b.indices
        && a.vertices.len() == b.vertices.len()
        && a.colors.len() == b.colors.len()
        && a.vertices
            .iter()
            .zip(&b.vertices)
            .all(|(x, y)| x.to_bits() == y.to_bits())
        && a.colors
            .iter()
            .zip(&b.colors)
            .all(|(x, y)| x.to_bits() == y.to_bits())
}

/// M3: the local stock after toolpath k IS the stock before toolpath k+1,
/// so the result holds it once.
#[test]
fn a_checkpoint_shares_its_stock_with_the_next_prior_stock_m3() {
    let result = simulate();
    assert_eq!(result.checkpoints.len(), PASSES, "one checkpoint per pass");
    for k in 0..PASSES - 1 {
        let next_prior = result
            .prior_stocks
            .get(&ToolpathId(k + 2))
            .expect("every carved toolpath keeps a prior stock");
        assert!(
            Arc::ptr_eq(&result.checkpoints[k].mesh_stock, next_prior),
            "checkpoint {k} and the prior stock of toolpath {} must be one \
             allocation, not two clones of one grid",
            k + 2
        );
    }
    // The first toolpath has no checkpoint before it: a fresh clone.
    assert!(
        result.prior_stocks.contains_key(&ToolpathId(1)),
        "the first toolpath keeps its prior stock too"
    );
}

/// M3: the tail phantom is the fully carved group stock, which is the last
/// checkpoint's local stock.
#[test]
fn the_tail_phantom_shares_the_last_checkpoint_stock_m3() {
    let result = simulate();
    let phantom = result
        .prior_stocks
        .get(&PHANTOM_ID)
        .expect("the tail phantom gets a prior stock");
    assert!(
        Arc::ptr_eq(&result.checkpoints[PASSES - 1].mesh_stock, phantom),
        "the tail phantom must share the last checkpoint's stock"
    );
}

/// M2: the mesh comes from the LOCAL stock, framed by the group's transform
/// and the stock origin — the eager build's inputs.
#[test]
fn a_checkpoint_mesh_is_built_from_the_local_stock_m2() {
    let result = simulate();
    let origin = stock().min;
    for k in 0..PASSES - 1 {
        let built = result.checkpoints[k].build_mesh();
        assert!(
            !built.indices.is_empty(),
            "checkpoint {k} must have a mesh, or the comparison is vacuous"
        );
        let local = &result.prior_stocks[&ToolpathId(k + 2)];
        let expected = transform_stock_mesh_to_global(&dexel_stock_to_mesh(local), &None, origin);
        assert!(
            same_bits(&built, &expected),
            "checkpoint {k}: the mesh must be the local stock's, shifted by \
             the stock origin, bit for bit"
        );
    }
}

/// M2: the scrub cache drops a mesh and builds it again. The two builds must
/// agree bit for bit.
#[test]
fn a_checkpoint_mesh_builds_the_same_twice_m2() {
    let result = simulate();
    for (k, cp) in result.checkpoints.iter().enumerate() {
        assert!(
            same_bits(&cp.build_mesh(), &cp.build_mesh()),
            "checkpoint {k}: two builds must give the same bits"
        );
    }
}
