//! Performance benchmark suite for rs_cam_core.
//!
//! Run with: cargo bench -p rs_cam_core
//! Results saved to target/criterion/

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::path::Path;

use rs_cam_core::dropcutter::{batch_drop_cutter, point_drop_cutter};
use rs_cam_core::mesh::{make_test_hemisphere, SpatialIndex, TriangleMesh};
use rs_cam_core::polygon::{offset_polygon, pocket_offsets, Polygon2};
use rs_cam_core::simulation::{stamp_tool_at, Heightmap};
use rs_cam_core::tool::{BallEndmill, FlatEndmill};
use rs_cam_core::waterline::waterline_contours;
use rs_cam_core::arcfit::fit_arcs;
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::geo::P3;

// ── Fixture helpers ──────────────────────────────────────────────────────

fn load_terrain() -> (TriangleMesh, SpatialIndex) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures/terrain_small.stl");
    let mesh = TriangleMesh::from_stl(&path).expect("load terrain_small.stl");
    let index = SpatialIndex::build(&mesh, 10.0);
    (mesh, index)
}

fn hemisphere_fixture(divisions: usize) -> (TriangleMesh, SpatialIndex) {
    let mesh = make_test_hemisphere(25.0, divisions);
    let index = SpatialIndex::build(&mesh, 10.0);
    (mesh, index)
}

fn square_polygon(size: f64) -> Polygon2 {
    let h = size / 2.0;
    Polygon2::rectangle(-h, -h, h, h)
}

fn make_linear_toolpath(n_moves: usize) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 10.0));
    tp.feed_to(P3::new(0.0, 0.0, -1.0), 500.0);
    for i in 0..n_moves {
        let x = (i as f64) * 0.5;
        let y = 5.0 * (i as f64 * 0.05).sin();
        tp.feed_to(P3::new(x, y, -1.0), 1000.0);
    }
    tp
}

// ── 1. Drop-cutter benchmarks ────────────────────────────────────────────

fn bench_batch_drop_cutter(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_drop_cutter");
    group.sample_size(10);

    // Hemisphere: small mesh, predictable geometry
    let (mesh, index) = hemisphere_fixture(20);
    let ball = BallEndmill::new(6.35, 25.0);
    group.bench_function("hemisphere_ball_6mm", |b| {
        b.iter(|| {
            black_box(batch_drop_cutter(
                &mesh, &index, &ball, 1.0, 0.0, -100.0,
            ))
        })
    });

    // Terrain: real-world mesh
    let (mesh, index) = load_terrain();
    let ball = BallEndmill::new(6.35, 25.0);
    group.bench_function("terrain_ball_6mm_step1", |b| {
        b.iter(|| {
            black_box(batch_drop_cutter(
                &mesh, &index, &ball, 1.0, 0.0, -100.0,
            ))
        })
    });

    let flat = FlatEndmill::new(6.35, 25.0);
    group.bench_function("terrain_flat_6mm_step1", |b| {
        b.iter(|| {
            black_box(batch_drop_cutter(
                &mesh, &index, &flat, 1.0, 0.0, -100.0,
            ))
        })
    });

    group.finish();
}

fn bench_point_drop_cutter(c: &mut Criterion) {
    let mut group = c.benchmark_group("point_drop_cutter");

    let (mesh, index) = load_terrain();
    let ball = BallEndmill::new(6.35, 25.0);
    let cx = (mesh.bbox.min.x + mesh.bbox.max.x) / 2.0;
    let cy = (mesh.bbox.min.y + mesh.bbox.max.y) / 2.0;

    group.bench_function("terrain_center_ball", |b| {
        b.iter(|| black_box(point_drop_cutter(cx, cy, &mesh, &index, &ball)))
    });

    group.finish();
}

// ── 2. Spatial index benchmarks ──────────────────────────────────────────

fn bench_spatial_index(c: &mut Criterion) {
    let mut group = c.benchmark_group("spatial_index");

    let (mesh, _) = load_terrain();

    // Build
    group.bench_function("build_terrain", |b| {
        b.iter(|| black_box(SpatialIndex::build(&mesh, 10.0)))
    });

    // Query
    let index = SpatialIndex::build(&mesh, 10.0);
    let cx = (mesh.bbox.min.x + mesh.bbox.max.x) / 2.0;
    let cy = (mesh.bbox.min.y + mesh.bbox.max.y) / 2.0;

    group.bench_function("query_r3", |b| {
        b.iter(|| black_box(index.query(cx, cy, 3.175)))
    });

    group.bench_function("query_r10", |b| {
        b.iter(|| black_box(index.query(cx, cy, 10.0)))
    });

    group.finish();
}

// ── 3. Heightmap stamping benchmarks ─────────────────────────────────────

fn bench_stamp_tool(c: &mut Criterion) {
    let mut group = c.benchmark_group("stamp_tool");

    let ball = BallEndmill::new(6.35, 25.0);
    let flat = FlatEndmill::new(6.35, 25.0);

    for cell_size in [0.5, 1.0] {
        let mut hm = Heightmap::from_stock(0.0, 0.0, 100.0, 100.0, 10.0, cell_size);
        group.bench_function(
            BenchmarkId::new("ball_6mm", format!("cs{cell_size}")),
            |b| {
                b.iter(|| stamp_tool_at(&mut hm, &ball, 50.0, 50.0, black_box(-2.0)))
            },
        );

        let mut hm = Heightmap::from_stock(0.0, 0.0, 100.0, 100.0, 10.0, cell_size);
        group.bench_function(
            BenchmarkId::new("flat_6mm", format!("cs{cell_size}")),
            |b| {
                b.iter(|| stamp_tool_at(&mut hm, &flat, 50.0, 50.0, black_box(-2.0)))
            },
        );
    }

    group.finish();
}

// ── 4. Waterline benchmarks ──────────────────────────────────────────────

fn bench_waterline(c: &mut Criterion) {
    let mut group = c.benchmark_group("waterline");
    group.sample_size(10);

    let (mesh, index) = hemisphere_fixture(20);
    let ball = BallEndmill::new(6.35, 25.0);

    group.bench_function("hemisphere_z10_samp1", |b| {
        b.iter(|| black_box(waterline_contours(&mesh, &index, &ball, 10.0, 1.0)))
    });

    let (mesh, index) = load_terrain();
    let ball = BallEndmill::new(6.35, 25.0);
    let mid_z = (mesh.bbox.min.z + mesh.bbox.max.z) / 2.0;

    group.bench_function("terrain_midz_samp1", |b| {
        b.iter(|| black_box(waterline_contours(&mesh, &index, &ball, mid_z, 1.0)))
    });

    group.finish();
}

// ── 5. Polygon offset & pocket benchmarks ────────────────────────────────

fn bench_polygon_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("polygon_ops");

    // Simple square offset
    let sq = square_polygon(60.0);
    group.bench_function("offset_60mm_square", |b| {
        b.iter(|| black_box(offset_polygon(&sq, 3.0)))
    });

    // Pocket offsets (multiple layers)
    group.bench_function("pocket_offsets_60mm", |b| {
        b.iter(|| black_box(pocket_offsets(&sq, 2.0)))
    });

    // Larger polygon
    let big = square_polygon(200.0);
    group.bench_function("pocket_offsets_200mm", |b| {
        b.iter(|| black_box(pocket_offsets(&big, 3.0)))
    });

    group.finish();
}

// ── 6. Arc fitting benchmarks ────────────────────────────────────────────

fn bench_arc_fitting(c: &mut Criterion) {
    let mut group = c.benchmark_group("arc_fitting");

    for n in [500, 2000, 10000] {
        let tp = make_linear_toolpath(n);
        group.bench_function(BenchmarkId::new("fit_arcs", n), |b| {
            b.iter(|| black_box(fit_arcs(&tp, 0.01)))
        });
    }

    group.finish();
}

// ── Group all benchmarks ─────────────────────────────────────────────────

criterion_group!(
    benches,
    bench_batch_drop_cutter,
    bench_point_drop_cutter,
    bench_spatial_index,
    bench_stamp_tool,
    bench_waterline,
    bench_polygon_ops,
    bench_arc_fitting,
);
criterion_main!(benches);
