//! P4 sentry (`planning/perf_sweep_2026-11-03/D_REPORT.md`): the model Z
//! query of the deviation passes changes, and the deviation output does not.
//!
//! `compute::simulate::query_model_z_range` reads the model triangles under
//! one point. Before P4 it asked `SpatialIndex::query(x, y, 0.0)`. That
//! query returns a whole row or column band of the index grid for a point
//! left of or below the grid, and the whole mesh for a point off both low
//! sides. A stock that is larger than the model puts many columns there.
//!
//! This file pins the full deviation output of one simulation:
//!
//! - every `ColumnDeviation` field, bit for bit, in order;
//! - every per-vertex deviation, bit for bit.
//!
//! The fixture stock is larger than the model on all four sides, and the
//! model is not at the origin of its frame. So the fixture has columns
//! left of, below, right of and above the index grid, and columns in the
//! corners. The setup is an identity setup with a non-zero stock origin, so
//! the column pass also applies its `-stock_min` frame map.
//!
//! The pinned numbers came from the code before P4 (HEAD 40cf411a7). A
//! change to them is a change to the deviation output, not a re-bless.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::simulate::{
    SimGroupEntry, SimToolpathEntry, SimulationRequest, SimulationResult, run_simulation,
};
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::dexel_stock::StockCutDirection;
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::tool::{BallEndmill, ToolDefinition};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

/// The world stock box. Its origin is not zero, so the identity group's
/// column pass must shift each query point by `-stock_min`.
const STOCK_MIN: [f64; 3] = [10.0, 20.0, -8.0];
const STOCK_MAX: [f64; 3] = [50.0, 48.0, 0.0];

/// The model footprint in the stock-relative frame. The stock-relative
/// stock is `[0, 40] x [0, 28]`, so the stock is larger on all four sides.
const MODEL_X: (f64, f64) = (6.0, 32.0);
const MODEL_Y: (f64, f64) = (5.0, 22.0);

/// The pinned FNV-1a digest of `column_deviations` (HEAD 40cf411a7).
const COLUMN_DIGEST: u64 = 9_523_803_477_485_249_676;
/// The pinned FNV-1a digest of `deviations` (HEAD 40cf411a7).
const VERTEX_DIGEST: u64 = 15_997_056_902_301_517_156;
/// The pinned column count (HEAD 40cf411a7).
const COLUMN_COUNT: usize = 10_772;
/// The pinned vertex count (HEAD 40cf411a7).
const VERTEX_COUNT: usize = 57_368;

/// A height field over the model footprint, in the stock-relative frame.
/// The surface is between z 2.5 and 6.5 (the stock-relative stock is
/// `0..8`), so a column top above it and a cut column both read a value.
fn model() -> TriangleMesh {
    let (nx, ny) = (27_usize, 18_usize);
    let mut vertices = Vec::with_capacity((nx + 1) * (ny + 1));
    for j in 0..=ny {
        for i in 0..=nx {
            let x = MODEL_X.0 + (MODEL_X.1 - MODEL_X.0) * i as f64 / nx as f64;
            let y = MODEL_Y.0 + (MODEL_Y.1 - MODEL_Y.0) * j as f64 / ny as f64;
            let z =
                4.5 + 1.5 * (x * 0.31).sin() * (y * 0.23).cos() + 0.5 * (x * 0.07 + y * 0.11).sin();
            vertices.push(P3::new(x, y, z));
        }
    }
    let mut triangles = Vec::with_capacity(2 * nx * ny);
    let at = |i: usize, j: usize| u32::try_from(j * (nx + 1) + i).unwrap();
    for j in 0..ny {
        for i in 0..nx {
            triangles.push([at(i, j), at(i + 1, j), at(i + 1, j + 1)]);
            triangles.push([at(i, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
}

fn tool() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(BallEndmill::new(4.0, 20.0)),
        4.0,
        40.0,
        2,
        ToolMaterial::Carbide,
    )
}

/// A world-frame raster that cuts over the model and past its edges, so
/// the final stock has cut and uncut columns on each side of the model.
fn raster() -> Arc<AnnotatedToolpath> {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(12.0, 22.0, 5.0));
    let mut lane = 0_usize;
    let mut y = 22.0;
    while y <= 46.0 {
        let (x_a, x_b) = if lane.is_multiple_of(2) {
            (12.0, 48.0)
        } else {
            (48.0, 12.0)
        };
        let depth = -2.0 - 0.1 * lane as f64;
        tp.feed_to(P3::new(x_a, y, depth), 1200.0);
        tp.feed_to(P3::new(x_b, y, depth), 1200.0);
        y += 1.5;
        lane += 1;
    }
    tp.rapid_to(P3::new(12.0, 46.0, 5.0));
    Arc::new(AnnotatedToolpath::new(tp))
}

fn request() -> SimulationRequest {
    SimulationRequest {
        groups: vec![SimGroupEntry {
            toolpaths: vec![SimToolpathEntry {
                head: Default::default(),
                id: ToolpathId::from_raw(1),
                name: "Raster".to_owned(),
                annotated: raster(),
                tool: Arc::new(tool()),
                tool_id: rs_cam_core::compute::tool_config::ToolId::from_raw(0),
                flute_count: 2,
                tool_summary: "4mm Ball".to_owned(),
                semantic_trace: None,
                spindle_rpm: None,
                metrics_not_applicable: false,
                drill_op: None,
                operation_config_hash: 1,
            }],
            direction: StockCutDirection::FromTop,
            local_stock_bbox: None,
            local_to_global: None,
            phantom_prior_stock: None,
            stock_changes: Vec::new(),
            obstacles: Ok(Default::default()),
        }],
        stock_bbox: BoundingBox3 {
            min: P3::new(STOCK_MIN[0], STOCK_MIN[1], STOCK_MIN[2]),
            max: P3::new(STOCK_MAX[0], STOCK_MAX[1], STOCK_MAX[2]),
        },
        stock_top_z: STOCK_MAX[2],
        // 0.2 mm gives 200 x 140 columns: above the 20 000-column
        // threshold, so the column pass runs its parallel branch.
        resolution: 0.2,
        spindle_rpm: 18_000,
        rapid_feed_mm_min: 5000.0,
        model_mesh: Some(Arc::new(model())),
        kinematics: None,
        display_stride: 1,
    }
}

/// FNV-1a over bytes. The digest does not depend on the std hasher.
struct Fnv(u64);

impl Fnv {
    const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
    fn bytes(&mut self, b: &[u8]) {
        for &x in b {
            self.0 ^= u64::from(x);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
    fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }
}

fn column_digest(result: &SimulationResult) -> (usize, u64) {
    let cols = result
        .column_deviations
        .as_ref()
        .expect("column deviations");
    let mut h = Fnv::new();
    for c in cols {
        h.u64(c.x.to_bits());
        h.u64(c.y.to_bits());
        h.u64(u64::from(c.dev.to_bits()));
        h.u64(c.group as u64);
        h.u64(u64::from(c.top_z.to_bits()));
        h.u64(c.row as u64);
        h.u64(c.col as u64);
    }
    (cols.len(), h.0)
}

fn vertex_digest(result: &SimulationResult) -> (usize, u64) {
    let devs = result.deviations.as_ref().expect("vertex deviations");
    let mut h = Fnv::new();
    for d in devs {
        h.u64(u64::from(d.to_bits()));
    }
    (devs.len(), h.0)
}

fn run() -> SimulationResult {
    run_simulation(&request(), &AtomicBool::new(false)).expect("simulation")
}

/// The deviation output is the output of the code before P4, bit for bit.
#[test]
fn the_deviation_output_is_pinned_perf4() {
    let result = run();
    let (n_cols, col_digest) = column_digest(&result);
    let (n_verts, vert_digest) = vertex_digest(&result);
    let cols = result
        .column_deviations
        .as_ref()
        .expect("column deviations");
    let devs = result.deviations.as_ref().expect("vertex deviations");

    // Non-vacuity: the fixture reaches every class of query point. The
    // column grid is 200 x 140 and the model covers about 40 % of it, so
    // most columns read no model and give no `ColumnDeviation`.
    assert!(n_cols > 1000, "too few column deviations: {n_cols}");
    assert!(n_cols < 200 * 140, "every column read the model: {n_cols}");
    assert!(
        cols.iter().any(|c| c.dev.abs() > 0.1),
        "no column deviation is non-zero"
    );
    assert!(
        devs.iter().any(|d| *d != 0.0),
        "no vertex deviation is non-zero"
    );
    assert!(devs.contains(&0.0), "no vertex is outside the model");
    // The reported x/y of an identity group stay in the world frame. So a
    // column over the model reports x near the model x plus the stock origin.
    let min_x = cols.iter().map(|c| c.x).fold(f64::INFINITY, f64::min);
    assert!(
        min_x >= MODEL_X.0 + STOCK_MIN[0] - 0.2,
        "a column left of the model read the model: x {min_x}"
    );

    assert_eq!(
        (n_cols, col_digest, n_verts, vert_digest),
        (COLUMN_COUNT, COLUMN_DIGEST, VERTEX_COUNT, VERTEX_DIGEST),
        "the deviation output changed (columns, column digest, vertices, vertex digest)"
    );
}

/// Two runs give the same output: the parallel branches keep their order.
#[test]
fn the_deviation_output_is_deterministic_perf4() {
    let a = run();
    let b = run();
    assert_eq!(column_digest(&a), column_digest(&b));
    assert_eq!(vertex_digest(&a), vertex_digest(&b));
}
