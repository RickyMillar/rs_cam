//! M2 A/B instrument (memory programme 2026-10-01): the on-demand checkpoint
//! mesh equals the eager checkpoint mesh bit for bit.
//!
//! This is an `#[ignore]` instrument, not a gate. The eager mesh exists only
//! on the commit before M2, so the comparison needs two builds:
//!
//! 1. On `fd06f407` (the eager build), copy this file into
//!    `crates/rs_cam_core/tests/`, replace the one token `cp.build_mesh()`
//!    with `cp.mesh.clone()`, and run
//!    `cargo test -p rs_cam_core -q --test checkpoint_mesh_ab_m2 -- --ignored`.
//!    The test fails and its message gives the fingerprint.
//! 2. On the M2 branch, run the same command with
//!    `RS_CAM_M2_CHECKPOINT_MESH_HASH=<that fingerprint>`. It must pass.
//!
//! The fixture is one identity group with a non-zero stock origin and a
//! drill-free raster chain, so the identity shift of
//! `transform_stock_mesh_to_global` is in the hash. The `local_to_global` arm
//! and the drill cylinders are not in this fixture.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::simulate::{
    SimGroupEntry, SimToolpathEntry, SimulationRequest, run_simulation,
};
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::dexel_stock::StockCutDirection;
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::stock::stock_mesh::StockMesh;
use rs_cam_core::tool::{FlatEndmill, ToolDefinition};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

/// The environment variable that carries the eager build's fingerprint.
const EXPECTED_ENV: &str = "RS_CAM_M2_CHECKPOINT_MESH_HASH";

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

/// A raster pass in the group's emission frame.
fn pass(pass_index: usize, x0: f64, x1: f64, y_base: f64) -> Arc<AnnotatedToolpath> {
    let y0 = y_base + 3.0 * pass_index as f64;
    let depth = -1.0 - 0.8 * pass_index as f64;
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(x0, y0, 5.0));
    for lane in 0..3 {
        let y = y0 + 2.0 * lane as f64;
        let (x_a, x_b) = if lane % 2 == 0 { (x0, x1) } else { (x1, x0) };
        tp.feed_to(P3::new(x_a, y, depth), 900.0);
        tp.feed_to(P3::new(x_b, y, depth), 900.0);
    }
    tp.rapid_to(P3::new(x1, y0, 5.0));
    Arc::new(AnnotatedToolpath::new(tp))
}

fn entry(id: usize, annotated: Arc<AnnotatedToolpath>) -> SimToolpathEntry {
    SimToolpathEntry {
        id: ToolpathId(id),
        name: format!("Pass{id}"),
        annotated,
        tool: Arc::new(tool()),
        flute_count: 2,
        tool_summary: "6mm Flat".to_owned(),
        semantic_trace: None,
        spindle_rpm: None,
        metrics_not_applicable: false,
        drill_op: None,
        operation_config_hash: id as u64,
    }
}

fn hash_mesh(h: &mut std::collections::hash_map::DefaultHasher, mesh: &StockMesh) {
    mesh.vertices.len().hash(h);
    for v in &mesh.vertices {
        v.to_bits().hash(h);
    }
    mesh.indices.hash(h);
    for c in &mesh.colors {
        c.to_bits().hash(h);
    }
}

#[test]
#[ignore = "A/B instrument: compare against the eager build of fd06f407"]
fn the_on_demand_checkpoint_mesh_equals_the_eager_mesh_m2() {
    // The identity group emits in WORLD coordinates (F-024).
    let top: Vec<SimToolpathEntry> = (0..3)
        .map(|i| entry(i + 1, pass(i, -17.0, 17.0, -6.0)))
        .collect();
    let request = SimulationRequest {
        groups: vec![SimGroupEntry {
            toolpaths: top,
            direction: StockCutDirection::FromTop,
            local_stock_bbox: None,
            local_to_global: None,
            phantom_prior_stock: None,
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
    let result = run_simulation(&request, &AtomicBool::new(false)).expect("simulation");
    assert_eq!(result.checkpoints.len(), 3, "one checkpoint per pass");

    let mut h = std::collections::hash_map::DefaultHasher::new();
    for cp in &result.checkpoints {
        cp.boundary_index.hash(&mut h);
        // The one token the A/B swaps: `cp.mesh.clone()` on fd06f407.
        let mesh = cp.build_mesh();
        assert!(
            !mesh.indices.is_empty(),
            "a checkpoint mesh must not be empty"
        );
        hash_mesh(&mut h, &mesh);
    }
    let got = format!("{:016x}", h.finish());

    match std::env::var(EXPECTED_ENV) {
        Ok(expected) => assert_eq!(
            got,
            expected.trim(),
            "the on-demand checkpoint meshes differ from the eager build"
        ),
        Err(_) => panic!("{EXPECTED_ENV} is not set; checkpoint mesh fingerprint = {got}"),
    }
}
