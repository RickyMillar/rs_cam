//! Performance benchmark suite for rs_cam_core.
//!
//! Run with: cargo bench -p rs_cam_core
//! Results saved to target/criterion/
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::str_to_string,
    clippy::semicolon_if_nothing_returned
)]

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use std::path::Path;

use rs_cam_core::arc_util::linearize_arc;
use rs_cam_core::arcfit::fit_arcs;
use rs_cam_core::contour_extract::weave_contours;
use rs_cam_core::dexel_mesh::dexel_stock_to_mesh;
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::dropcutter::{DropCutterGrid, batch_drop_cutter, point_drop_cutter};
use rs_cam_core::fiber::{Fiber, Interval};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh, make_test_hemisphere};
use rs_cam_core::polygon::{Polygon2, offset_polygon, pocket_offsets};
use rs_cam_core::pushcutter::batch_push_cutter;
use rs_cam_core::radial_profile::RadialProfileLUT;
use rs_cam_core::slope::SlopeMap;
use rs_cam_core::steep_shallow::dilate_grid;
use rs_cam_core::tool::{BallEndmill, CLPoint, FlatEndmill, MillingCutter};
use rs_cam_core::toolpath::{Toolpath, raster_toolpath_from_grid, simplify_path_3d};
use rs_cam_core::waterline::waterline_contours;

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
        b.iter(|| black_box(batch_drop_cutter(&mesh, &index, &ball, 1.0, 0.0, -100.0)))
    });

    // Terrain: real-world mesh
    let (mesh, index) = load_terrain();
    let ball = BallEndmill::new(6.35, 25.0);
    group.bench_function("terrain_ball_6mm_step1", |b| {
        b.iter(|| black_box(batch_drop_cutter(&mesh, &index, &ball, 1.0, 0.0, -100.0)))
    });

    let flat = FlatEndmill::new(6.35, 25.0);
    group.bench_function("terrain_flat_6mm_step1", |b| {
        b.iter(|| black_box(batch_drop_cutter(&mesh, &index, &flat, 1.0, 0.0, -100.0)))
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

// ── 3. Tri-dexel stamping benchmarks ─────────────────────────────────────

fn bench_stamp_tool(c: &mut Criterion) {
    let mut group = c.benchmark_group("stamp_tool");

    let ball = BallEndmill::new(6.35, 25.0);
    let flat = FlatEndmill::new(6.35, 25.0);
    let ball_lut = RadialProfileLUT::from_cutter(&ball, 256);
    let flat_lut = RadialProfileLUT::from_cutter(&flat, 256);

    for cell_size in [0.5, 1.0] {
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 100.0, 100.0, 0.0, 10.0, cell_size);
        group.bench_function(
            BenchmarkId::new("ball_6mm", format!("cs{cell_size}")),
            |b| {
                b.iter(|| {
                    stock.stamp_tool_at(
                        &ball_lut,
                        ball.radius(),
                        50.0,
                        50.0,
                        black_box(-2.0),
                        StockCutDirection::FromTop,
                    )
                })
            },
        );

        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 100.0, 100.0, 0.0, 10.0, cell_size);
        group.bench_function(
            BenchmarkId::new("flat_6mm", format!("cs{cell_size}")),
            |b| {
                b.iter(|| {
                    stock.stamp_tool_at(
                        &flat_lut,
                        flat.radius(),
                        50.0,
                        50.0,
                        black_box(-2.0),
                        StockCutDirection::FromTop,
                    )
                })
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

// ── 7. Simulation benchmarks ────────────────────────────────────────────

fn bench_stamp_linear_segment(c: &mut Criterion) {
    let mut group = c.benchmark_group("stamp_linear_segment");
    group.sample_size(20);

    let ball = BallEndmill::new(6.0, 25.0);
    let lut = RadialProfileLUT::from_cutter(&ball, 256);
    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 60.0, 10.0, 0.0, 10.0, 0.25);
    let start = P3::new(5.0, 5.0, -2.0);
    let end = P3::new(55.0, 5.0, -2.0);

    group.bench_function("50mm_ball6_cs025", |b| {
        b.iter(|| {
            stock.stamp_linear_segment(
                &lut,
                ball.radius(),
                black_box(start),
                black_box(end),
                StockCutDirection::FromTop,
            )
        })
    });

    group.finish();
}

fn bench_simulate_toolpath(c: &mut Criterion) {
    let mut group = c.benchmark_group("simulate_toolpath");
    group.sample_size(10);

    let ball = BallEndmill::new(6.0, 25.0);
    let tp = make_linear_toolpath(2000);
    let fresh = TriDexelStock::from_stock(0.0, 0.0, 1050.0, 20.0, 0.0, 10.0, 0.25);

    group.bench_function("2000moves_ball6_cs025", |b| {
        b.iter(|| {
            let mut stock = fresh.clone();
            stock.simulate_toolpath(&tp, &ball, StockCutDirection::FromTop);
            black_box(&stock);
        })
    });

    group.finish();
}

// ── 8. Fiber interval insertion benchmarks ─────────────────────────────

fn bench_fiber_interval_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("fiber_interval_insert");

    for n in [50, 200, 1000] {
        group.bench_function(BenchmarkId::new("non_overlapping", n), |b| {
            // Pre-compute intervals so only insertion is timed.
            let step = 1.0 / (n as f64 + 1.0);
            let width = step * 0.6;
            let intervals: Vec<Interval> = (0..n)
                .map(|i| {
                    let lo = (i as f64 + 0.5) * step;
                    Interval::new(lo, lo + width)
                })
                .collect();
            b.iter(|| {
                let mut fiber = Fiber::new_x(0.0, 0.0, 0.0, 100.0);
                for iv in &intervals {
                    fiber.add_interval(*iv);
                }
                black_box(fiber.intervals().len());
            })
        });

        group.bench_function(BenchmarkId::new("overlapping", n), |b| {
            // Intervals that all overlap with center region — forces merging.
            let intervals: Vec<Interval> = (0..n)
                .map(|i| {
                    let lo = 0.3 + 0.001 * (i as f64);
                    Interval::new(lo, lo + 0.05)
                })
                .collect();
            b.iter(|| {
                let mut fiber = Fiber::new_x(0.0, 0.0, 0.0, 100.0);
                for iv in &intervals {
                    fiber.add_interval(*iv);
                }
                black_box(fiber.intervals().len());
            })
        });
    }

    group.finish();
}

// ── 9. Simulation with metrics benchmarks ─────────────────────────────

fn bench_simulate_toolpath_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("simulate_toolpath_metrics");
    group.sample_size(10);

    let ball = BallEndmill::new(6.0, 25.0);
    let tp = make_linear_toolpath(500);
    let fresh = TriDexelStock::from_stock(0.0, 0.0, 300.0, 20.0, 0.0, 10.0, 0.5);
    let never_cancel = || false;

    group.bench_function("500moves_ball6_cs05", |b| {
        b.iter(|| {
            let mut stock = fresh.clone();
            let samples = stock
                .simulate_toolpath_with_metrics_with_cancel(
                    &tp,
                    &ball,
                    StockCutDirection::FromTop,
                    0,      // toolpath_id
                    18000,  // spindle_rpm
                    2,      // flute_count
                    5000.0, // rapid_feed
                    1.0,    // sample_step
                    None,   // no semantic trace
                    &never_cancel,
                )
                .expect("no cancel");
            black_box(samples.len());
        })
    });

    group.finish();
}

// ── 8b. Push-cutter batch benchmarks ──────────────────────────────────

fn bench_push_cutter_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("push_cutter_batch");
    group.sample_size(10);

    let (mesh, index) = hemisphere_fixture(20);
    let ball = BallEndmill::new(6.35, 25.0);
    let bbox = &mesh.bbox;
    let z = (bbox.min.z + bbox.max.z) / 2.0;

    group.bench_function("hemisphere_200fibers", |b| {
        b.iter(|| {
            let mut fibers: Vec<Fiber> = (0..200)
                .map(|i| {
                    let y = bbox.min.y + (i as f64 / 200.0) * (bbox.max.y - bbox.min.y);
                    Fiber::new_x(y, z, bbox.min.x, bbox.max.x)
                })
                .collect();
            batch_push_cutter(&mut fibers, &mesh, &index, &ball);
            black_box(fibers.len());
        })
    });

    let (mesh, index) = load_terrain();
    let ball = BallEndmill::new(6.35, 25.0);
    let bbox_t = &mesh.bbox;
    let mid_z = (bbox_t.min.z + bbox_t.max.z) / 2.0;

    group.bench_function("terrain_200fibers", |b| {
        b.iter(|| {
            let mut fibers: Vec<Fiber> = (0..200)
                .map(|i| {
                    let y = bbox_t.min.y + (i as f64 / 200.0) * (bbox_t.max.y - bbox_t.min.y);
                    Fiber::new_x(y, mid_z, bbox_t.min.x, bbox_t.max.x)
                })
                .collect();
            batch_push_cutter(&mut fibers, &mesh, &index, &ball);
            black_box(fibers.len());
        })
    });

    group.finish();
}

// ── 10. Mesh extraction benchmarks ────────────────────────────────────

fn bench_dexel_mesh_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("dexel_mesh_extraction");
    group.sample_size(20);

    // Small grid: 100x100 at cs=1.0
    let ball = BallEndmill::new(6.0, 25.0);
    let lut = RadialProfileLUT::from_cutter(&ball, 256);
    let mut small = TriDexelStock::from_stock(0.0, 0.0, 100.0, 100.0, 0.0, 10.0, 1.0);
    // Stamp some geometry so the mesh isn't trivially uniform.
    for i in 0..5 {
        small.stamp_tool_at(
            &lut,
            ball.radius(),
            20.0 + 15.0 * i as f64,
            50.0,
            -2.0,
            StockCutDirection::FromTop,
        );
    }

    group.bench_function("100x100_cs1", |b| {
        b.iter(|| black_box(dexel_stock_to_mesh(&small)))
    });

    // Medium grid: 200x200 at cs=0.5
    let mut medium = TriDexelStock::from_stock(0.0, 0.0, 100.0, 100.0, 0.0, 10.0, 0.5);
    for i in 0..10 {
        medium.stamp_tool_at(
            &lut,
            ball.radius(),
            10.0 + 8.0 * i as f64,
            50.0,
            -2.0,
            StockCutDirection::FromTop,
        );
    }

    group.bench_function("200x200_cs05", |b| {
        b.iter(|| black_box(dexel_stock_to_mesh(&medium)))
    });

    group.finish();
}

// ── 11. Arc linearization benchmarks ──────────────────────────────────

fn bench_arc_linearize(c: &mut Criterion) {
    let mut group = c.benchmark_group("arc_linearize");

    let start = P3::new(5.0, 0.0, 0.0);
    let end = P3::new(-5.0, 0.0, 0.0);

    for seg_len in [0.1, 0.5, 1.0] {
        group.bench_function(
            BenchmarkId::new("semicircle_r5", format!("seg{seg_len}")),
            |b| b.iter(|| black_box(linearize_arc(start, end, -5.0, 0.0, false, seg_len))),
        );
    }

    group.finish();
}

// ── 9b. Contour weave benchmarks (exercises chain_segments) ───────────

fn bench_weave_contours(c: &mut Criterion) {
    let mut group = c.benchmark_group("weave_contours");
    group.sample_size(10);

    // Build fibers from hemisphere push-cutter, then weave.
    let (mesh, index) = hemisphere_fixture(20);
    let ball = BallEndmill::new(6.35, 25.0);
    let bbox = &mesh.bbox;
    let z = 10.0;

    for n_fibers in [50, 200] {
        let mut x_fibers: Vec<Fiber> = (0..n_fibers)
            .map(|i| {
                let y = bbox.min.y + (i as f64 / n_fibers as f64) * (bbox.max.y - bbox.min.y);
                Fiber::new_x(y, z, bbox.min.x, bbox.max.x)
            })
            .collect();
        let mut y_fibers: Vec<Fiber> = (0..n_fibers)
            .map(|i| {
                let x = bbox.min.x + (i as f64 / n_fibers as f64) * (bbox.max.x - bbox.min.x);
                Fiber::new_y(x, z, bbox.min.y, bbox.max.y)
            })
            .collect();
        batch_push_cutter(&mut x_fibers, &mesh, &index, &ball);
        batch_push_cutter(&mut y_fibers, &mesh, &index, &ball);

        group.bench_function(BenchmarkId::new("hemisphere", n_fibers), |b| {
            b.iter(|| black_box(weave_contours(&x_fibers, &y_fibers, z)))
        });
    }

    group.finish();
}

// ── 10. Grid dilation benchmarks ──────────────────────────────────────

fn bench_dilate_grid(c: &mut Criterion) {
    let mut group = c.benchmark_group("dilate_grid");

    // 200x200 grid with a circular steep region in center.
    let rows = 200;
    let cols = 200;
    let grid: Vec<bool> = (0..rows * cols)
        .map(|i| {
            let r = i / cols;
            let c_col = i % cols;
            let dr = r as f64 - 100.0;
            let dc = c_col as f64 - 100.0;
            dr * dr + dc * dc < 50.0 * 50.0
        })
        .collect();

    for radius in [3, 5, 10] {
        group.bench_function(BenchmarkId::new("200x200", radius), |b| {
            b.iter(|| black_box(dilate_grid(&grid, rows, cols, radius)))
        });
    }

    group.finish();
}

// ── 11. Raster toolpath benchmarks ────────────────────────────────────

fn bench_raster_toolpath(c: &mut Criterion) {
    let mut group = c.benchmark_group("raster_toolpath");

    // Build a synthetic DropCutterGrid.
    for grid_size in [100, 300] {
        let mut points = Vec::with_capacity(grid_size * grid_size);
        for row in 0..grid_size {
            for col in 0..grid_size {
                let x = col as f64 * 0.5;
                let y = row as f64 * 0.5;
                let z = -2.0 + 0.5 * ((x * 0.1).sin() + (y * 0.1).cos());
                points.push(CLPoint {
                    x,
                    y,
                    z,
                    contacted: true,
                });
            }
        }
        let grid = DropCutterGrid {
            points,
            rows: grid_size,
            cols: grid_size,
            x_start: 0.0,
            y_start: 0.0,
            x_step: 0.5,
            y_step: 0.5,
        };

        group.bench_function(BenchmarkId::new("zigzag", grid_size), |b| {
            b.iter(|| black_box(raster_toolpath_from_grid(&grid, 1000.0, 500.0, 10.0)))
        });
    }

    group.finish();
}

// ── 12. Path simplification benchmarks ────────────────────────────────

fn bench_simplify_path(c: &mut Criterion) {
    let mut group = c.benchmark_group("simplify_path");

    for n in [1000, 5000, 10000] {
        // Noisy sine wave path — good for Douglas-Peucker.
        let points: Vec<P3> = (0..n)
            .map(|i| {
                let t = i as f64 / n as f64 * 100.0;
                let noise = (t * 7.3).sin() * 0.01;
                P3::new(t, (t * 0.1).sin() * 10.0 + noise, -2.0 + noise)
            })
            .collect();

        group.bench_function(BenchmarkId::new("noisy_sine", n), |b| {
            b.iter(|| black_box(simplify_path_3d(&points, 0.05)))
        });
    }

    group.finish();
}

// ── 13. Slope map benchmarks ──────────────────────────────────────────

fn bench_slope_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("slope_map");

    for grid_size in [200, 500] {
        let rows = grid_size;
        let cols = grid_size;
        let cs = 0.5;
        let z_values: Vec<f64> = (0..rows * cols)
            .map(|i| {
                let r = i / cols;
                let c_col = i % cols;
                let x = c_col as f64 * cs;
                let y = r as f64 * cs;
                5.0 + 2.0 * (x * 0.1).sin() * (y * 0.1).cos()
            })
            .collect();

        group.bench_function(BenchmarkId::new("from_z_grid", grid_size), |b| {
            b.iter(|| black_box(SlopeMap::from_z_grid(&z_values, rows, cols, 0.0, 0.0, cs)))
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
    bench_stamp_linear_segment,
    bench_simulate_toolpath,
    bench_fiber_interval_insert,
    bench_simulate_toolpath_metrics,
    bench_dexel_mesh_extraction,
    bench_arc_linearize,
    bench_push_cutter_batch,
    bench_weave_contours,
    bench_dilate_grid,
    bench_raster_toolpath,
    bench_simplify_path,
    bench_slope_map,
);
criterion_main!(benches);
