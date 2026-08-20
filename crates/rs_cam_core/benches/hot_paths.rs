//! Phase 0A baselines for `planning/perf_review_2026-08-19/PERF_REVIEW.md`.
//!
//! `perf_suite.rs` and `classification.rs` cover primitives; none of the top
//! findings in the 2026-08-19 performance review is visible to them. This
//! target is one bench per finding cluster, deliberately landed **before** any
//! fix so no baseline can be re-derived from a tree that already contains an
//! optimisation.
//!
//! ```text
//! cargo bench -p rs_cam_core --bench hot_paths
//! cargo bench -p rs_cam_core --bench hot_paths -- gen_depth   # one group
//! ```
//!
//! Criterion always builds `--release`, so every number here is a release
//! number. Group names map to findings:
//!
//! | Group | Findings |
//! |---|---|
//! | `sim_kernel_lateral` | S1a, S2, S3, S7, S8 |
//! | `sim_kernel_plunge`  | S1b (the `by_z` 0.02 mm subdivision) |
//! | `sim_e2e_small`      | S4, S6 |
//! | `sim_dispatch_ab`    | S3 wave 4 — per-stamp vs whole-toolpath, PAIRED |
//! | `gen_depth`          | G2 — the L20/L1 **ratio** is the number |
//! | `gen_waterline`      | G1 — likewise the L20/L1 ratio |
//! | `gen_contains_point` | G4 |
//! | `gen_rapid_order`    | G5 |
//! | `gen_vcarve_field`   | G9 |
//! | `viz_triage_build`   | V1 (core side) |
//!
//! Sizing rule: one criterion iteration stays under ~2 s, so the whole target
//! runs in minutes. Where a finding is about repetition (G1, G2) the bench
//! carries BOTH arms of the ratio, because it is the ratio — not the absolute
//! — that the fix is supposed to move.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::str_to_string,
    clippy::semicolon_if_nothing_returned
)]

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

use rs_cam_core::dexel_stock::{StampDispatch, StockCutDirection, TriDexelStock};
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::radial_profile::RadialProfileLUT;
use rs_cam_core::region_set::RegionSet;
use rs_cam_core::simulation_cut::SimulationCutSample;
use rs_cam_core::tool::{BallEndmill, FlatEndmill, MillingCutter};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::toolpath_spans::AnnotatedToolpath;

// ── Shared fixtures ─────────────────────────────────────────────────────

/// A jittered near-circular ring at marching-squares vertex density — the
/// shape class G2 and G4 both fire on (`PERF_REVIEW.md` G4: "rings ~1400
/// verts at 0.5 mm cell").
///
/// The jitter is deterministic (a fixed harmonic sum, no RNG) so every run of
/// this bench measures the same geometry.
fn jittered_ring(radius: f64, verts: usize) -> Vec<P2> {
    (0..verts)
        .map(|i| {
            let t = std::f64::consts::TAU * i as f64 / verts as f64;
            // Two incommensurate harmonics: enough local wobble that the
            // offset cascade cannot short-circuit on a perfect circle, small
            // enough that the ring stays simple (no self-intersection).
            let r = radius + 0.6 * (7.0 * t).sin() + 0.25 * (23.0 * t).cos();
            P2::new(r * t.cos(), r * t.sin())
        })
        .collect()
}

/// The G4 fixture: a 1400-vertex ring with three 200-vertex holes.
fn ring_with_holes(radius: f64, verts: usize, hole_verts: usize) -> Polygon2 {
    let mut poly = Polygon2::new(jittered_ring(radius, verts));
    for k in 0..3 {
        let a = std::f64::consts::TAU * k as f64 / 3.0;
        let (cx, cy) = (0.45 * radius * a.cos(), 0.45 * radius * a.sin());
        poly.holes.push(
            (0..hole_verts)
                .map(|i| {
                    // CW winding, per `Polygon2`'s hole contract.
                    let t = -std::f64::consts::TAU * i as f64 / hole_verts as f64;
                    P2::new(cx + 0.18 * radius * t.cos(), cy + 0.18 * radius * t.sin())
                })
                .collect(),
        );
    }
    poly
}

/// A rolling height field — the same generator shape `classification.rs`
/// uses, at the size the waterline bench can afford.
///
/// `n` vertices per side → `2·(n−1)²` triangles.
fn rolling_field(half: f64, n: usize) -> TriangleMesh {
    let step = 2.0 * half / (n - 1) as f64;
    let mut vertices = Vec::with_capacity(n * n);
    for iy in 0..n {
        let y = -half + iy as f64 * step;
        for ix in 0..n {
            let x = -half + ix as f64 * step;
            let z = 1.6 * (x * 0.9).sin() * (y * 0.7).cos() + 0.9 * (x * 2.3 + y * 1.7).sin()
                - 0.35 * (x * x + y * y).sqrt();
            vertices.push(P3::new(x, y, z));
        }
    }
    let mut triangles = Vec::with_capacity(2 * (n - 1) * (n - 1));
    for iy in 0..n - 1 {
        for ix in 0..n - 1 {
            let a = (iy * n + ix) as u32;
            let b = a + 1;
            let c = a + n as u32;
            let d = c + 1;
            triangles.push([a, c, b]);
            triangles.push([b, c, d]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
}

// ── S1a / S2 / S3 / S7 / S8: lateral stamp kernel ───────────────────────

/// A raster (zigzag) cutting pass: `passes` lines of `span` mm at `depth`,
/// linked by retract + rapid, all inside a stock footprint.
fn raster_pass(passes: usize, span: f64, stepover: f64, depth: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(5.0, 5.0, 10.0));
    for i in 0..passes {
        let y = 5.0 + stepover * i as f64;
        let (x0, x1) = if i % 2 == 0 {
            (5.0, 5.0 + span)
        } else {
            (5.0 + span, 5.0)
        };
        if i == 0 {
            tp.rapid_to(P3::new(x0, y, 10.0));
            tp.feed_to(P3::new(x0, y, depth), 500.0);
        } else {
            tp.feed_to(P3::new(x0, y, depth), 1000.0);
        }
        tp.feed_to(P3::new(x1, y, depth), 1200.0);
    }
    tp.final_retract(10.0);
    tp
}

/// [`run_metric_sim`] with the stamp dispatch forced, for the wave-4 A/B.
#[allow(clippy::too_many_arguments)]
fn run_metric_sim_with(
    fresh: &TriDexelStock,
    tp: &Toolpath,
    lut: &RadialProfileLUT,
    cutter: &dyn MillingCutter,
    radius: f64,
    sample_step_mm: f64,
    dispatch: StampDispatch,
) -> usize {
    let never_cancel = || false;
    let mut stock = fresh.clone();
    stock.stamp_dispatch = dispatch;
    let samples = stock
        .simulate_toolpath_with_lut_metrics_cancel(
            tp,
            lut,
            cutter,
            radius,
            StockCutDirection::FromTop,
            ToolpathId(0),
            18_000,
            2,
            5000.0,
            sample_step_mm,
            None,
            &[],
            &[],
            false,
            &never_cancel,
        )
        .expect("never cancelled");
    samples.len()
}

// ── S3 wave 4: per-stamp vs whole-toolpath dispatch, PAIRED ─────────────

/// The wave-4 deliverable, and it is deliberately **one criterion invocation**.
///
/// `DELTA_sim_w2.md` §1 threw away this wave's first A/B because an unrelated
/// test suite was running at 171 % CPU and the *unmodified* tree read 18–134 %
/// above its own committed numbers — criterion reports p-values against
/// contention exactly as confidently as against a real change. The only defence
/// is to measure both arms in the same session on the same machine, which is
/// what this group does: for each fixture and each thread count, `per_stamp`
/// and `whole_path` sit adjacent in one run, and the number that matters is the
/// RATIO between them, not either absolute.
///
/// Thread counts are pinned with a rayon pool per arm rather than through
/// `RAYON_NUM_THREADS`, so the sweep is inside one process too. The wave-2
/// ceiling to beat is in `DELTA_sim_w2.md` §3c: **2.10× at four threads** on
/// `flat12/cs0.1`, saturating at eight and regressing at twenty-four.
fn bench_sim_dispatch_ab(c: &mut Criterion) {
    let mut group = c.benchmark_group("sim_dispatch_ab");
    group.sample_size(10);

    let lateral = raster_pass(6, 40.0, 4.0, -2.0);
    let plunge = plunge_pass(24, -5.0);

    #[allow(clippy::type_complexity)]
    let fixtures: Vec<(&str, f64, f64, &Toolpath)> = vec![
        ("flat12_cs0.1", 12.0, 0.1, &lateral),
        ("flat6_cs0.1", 6.0, 0.1, &lateral),
        ("flat6_cs0.25_plunge", 6.0, 0.25, &plunge),
    ];

    for (name, diameter, cell_size, tp) in fixtures {
        let flat = FlatEndmill::new(diameter, 25.0);
        let lut = RadialProfileLUT::from_cutter(&flat, rs_cam_core::radial_profile::LUT_SAMPLES);
        let fresh = TriDexelStock::from_stock(0.0, 0.0, 50.0, 32.0, 0.0, 10.0, cell_size);
        // 24 is the box's core count and is in the sweep deliberately: wave 2
        // measured per-stamp dispatch getting *worse* there (108.6 ms against
        // 78.6 at eight), so "does the regression past eight survive?" is a
        // question this group has to be able to answer.
        for threads in [1usize, 2, 4, 8, 24] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .expect("thread pool builds");
            for (mode_name, mode) in [
                ("per_stamp", StampDispatch::PerStamp),
                ("whole_path", StampDispatch::WholeToolpath),
                // S1 wave 5. Metric-CHANGING, and in the same session on
                // purpose: the ratio against `whole_path` is the number the
                // landing decision turns on, and a cross-day absolute on this
                // box is worth nothing (`BASELINES.md`, measurement discipline).
                ("swept_plunge", StampDispatch::SweptPlungeOnly),
                ("swept", StampDispatch::Swept),
            ] {
                group.bench_function(
                    BenchmarkId::new(format!("{name}/{mode_name}"), threads),
                    |b| {
                        pool.install(|| {
                            b.iter(|| {
                                black_box(run_metric_sim_with(
                                    &fresh,
                                    tp,
                                    &lut,
                                    &flat,
                                    flat.radius(),
                                    0.25,
                                    mode,
                                ))
                            })
                        })
                    },
                );
            }
        }
    }

    group.finish();
}

/// The PRODUCTION metric-collecting sim kernel — not
/// `simulate_toolpath_with_metrics_with_cancel` (which rebuilds the LUT per
/// call) and emphatically not the non-metric `simulate_toolpath` that
/// `perf_suite.rs` benches at roughly 1/25th the per-cell cost.
#[allow(clippy::too_many_arguments)]
fn run_metric_sim(
    fresh: &TriDexelStock,
    tp: &Toolpath,
    lut: &RadialProfileLUT,
    cutter: &dyn MillingCutter,
    radius: f64,
    sample_step_mm: f64,
) -> usize {
    let never_cancel = || false;
    let mut stock = fresh.clone();
    let samples = stock
        .simulate_toolpath_with_lut_metrics_cancel(
            tp,
            lut,
            cutter,
            radius,
            StockCutDirection::FromTop,
            ToolpathId(0),
            18_000,
            2,
            5000.0,
            sample_step_mm,
            None,
            &[],
            &[],
            false,
            &never_cancel,
        )
        .expect("never cancelled");
    samples.len()
}

fn bench_sim_kernel_lateral(c: &mut Criterion) {
    let mut group = c.benchmark_group("sim_kernel_lateral");
    group.sample_size(10);

    // 6 passes × 40 mm = 240 mm of cutting at sample_step 0.25 → 960
    // subsegments, each stamping the full radius-inflated footprint. That is
    // the S1a redundancy multiplier, measured.
    let tp = raster_pass(6, 40.0, 4.0, -2.0);

    for diameter in [6.0_f64, 12.0] {
        let flat = FlatEndmill::new(diameter, 25.0);
        let lut = RadialProfileLUT::from_cutter(&flat, rs_cam_core::radial_profile::LUT_SAMPLES);
        for cell_size in [0.25_f64, 0.1] {
            let fresh = TriDexelStock::from_stock(0.0, 0.0, 50.0, 32.0, 0.0, 10.0, cell_size);
            group.bench_function(
                BenchmarkId::new(format!("flat{diameter}"), format!("cs{cell_size}")),
                |b| {
                    b.iter(|| {
                        black_box(run_metric_sim(
                            &fresh,
                            &tp,
                            &lut,
                            &flat,
                            flat.radius(),
                            0.25,
                        ))
                    })
                },
            );
        }
    }

    group.finish();
}

// ── S1b: the `by_z` subdivision on plunge-heavy motion ──────────────────

/// A project-curve / drill-shaped pass: `holes` plunge-and-retract cycles.
///
/// Each 5 mm descent is subdivided at `MAX_SUBSEGMENT_Z_DROP_MM = 0.02`
/// (`dexel_stock/simulation.rs:441`) into 250 identical full-footprint
/// stamps, which is exactly the cost S1b proposes to make analytic.
fn plunge_pass(holes: usize, depth: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(4.0, 4.0, 10.0));
    for i in 0..holes {
        let x = 4.0 + 3.0 * (i % 12) as f64;
        let y = 4.0 + 3.0 * (i / 12) as f64;
        tp.rapid_to(P3::new(x, y, 1.0));
        tp.feed_to(P3::new(x, y, depth), 300.0);
        tp.feed_to(P3::new(x, y, 1.0), 800.0);
    }
    tp.final_retract(10.0);
    tp
}

fn bench_sim_kernel_plunge(c: &mut Criterion) {
    let mut group = c.benchmark_group("sim_kernel_plunge");
    group.sample_size(10);

    let flat = FlatEndmill::new(6.0, 25.0);
    let lut = RadialProfileLUT::from_cutter(&flat, rs_cam_core::radial_profile::LUT_SAMPLES);

    for holes in [24_usize, 60] {
        let tp = plunge_pass(holes, -5.0);
        let fresh = TriDexelStock::from_stock(0.0, 0.0, 44.0, 24.0, 0.0, 10.0, 0.25);
        group.bench_function(BenchmarkId::new("flat6_cs025", holes), |b| {
            b.iter(|| {
                black_box(run_metric_sim(
                    &fresh,
                    &tp,
                    &lut,
                    &flat,
                    flat.radius(),
                    0.25,
                ))
            })
        });
    }

    group.finish();
}

// ── G2: depth-stepped 2.5D geometry recomputed per Z level ──────────────

/// The G2 number is the **ratio** `L20 / L1`. Before the fix it should be
/// ≈ 20 (the whole 2D cascade re-runs per level); after the hoist it should
/// fall towards 1 plus the per-level emission cost.
///
/// **Two families of arms, deliberately.** `pocket/L20` and friends measure
/// the PRE-FIX call shape — `pocket_toolpath` inside the per-level closure, so
/// the whole 2D cascade re-runs per level. Those arms are left exactly as
/// Phase 0 captured them, and they still read the Phase 0 numbers, because the
/// hoist does not make that shape faster; it makes it avoidable. The
/// `*_hoisted` arms measure the POST-FIX shape that `compute/execute.rs` now
/// uses: geometry once, then stamp per level. The G2 deliverable is
/// `pocket/L20_hoisted` against `pocket/L1`, in one run, on one machine.
fn bench_gen_depth(c: &mut Criterion) {
    use rs_cam_core::depth::{toolpath_at_levels, toolpath_at_levels_with_cancel};
    use rs_cam_core::pocket::pocket_toolpath;
    use rs_cam_core::pocket::{PocketParams, pocket_contours, pocket_contours_to_toolpath};
    use rs_cam_core::profile::{
        ProfileParams, ProfileSide, profile_path_reported, profile_path_to_toolpath,
        profile_toolpath,
    };
    use rs_cam_core::zigzag::{ZigzagParams, lines_to_toolpath, zigzag_lines, zigzag_toolpath};

    let mut group = c.benchmark_group("gen_depth");
    group.sample_size(10);

    let poly = Polygon2::new(jittered_ring(40.0, 1400));
    let levels_1: Vec<f64> = vec![-2.0];
    let levels_20: Vec<f64> = (1..=20).map(|i| -0.5 * i as f64).collect();

    for (label, levels) in [("L1", &levels_1), ("L20", &levels_20)] {
        group.bench_function(BenchmarkId::new("pocket", label), |b| {
            b.iter(|| {
                black_box(toolpath_at_levels(levels, 10.0, |z| {
                    pocket_toolpath(
                        &poly,
                        &PocketParams {
                            tool_radius: 3.0,
                            stepover: 4.0,
                            cut_depth: z,
                            feed_rate: 1200.0,
                            plunge_rate: 400.0,
                            safe_z: 10.0,
                            climb: true,
                        },
                    )
                }))
            })
        });

        group.bench_function(BenchmarkId::new("profile", label), |b| {
            b.iter(|| {
                black_box(toolpath_at_levels(levels, 10.0, |z| {
                    profile_toolpath(
                        &poly,
                        &ProfileParams {
                            tool_radius: 3.0,
                            side: ProfileSide::Outside,
                            cut_depth: z,
                            feed_rate: 1200.0,
                            plunge_rate: 400.0,
                            safe_z: 10.0,
                            climb: true,
                            compensate_in_controller: false,
                        },
                    )
                }))
            })
        });

        group.bench_function(BenchmarkId::new("zigzag", label), |b| {
            b.iter(|| {
                black_box(toolpath_at_levels(levels, 10.0, |z| {
                    zigzag_toolpath(
                        &poly,
                        &ZigzagParams {
                            tool_radius: 3.0,
                            stepover: 4.0,
                            cut_depth: z,
                            feed_rate: 1200.0,
                            plunge_rate: 400.0,
                            safe_z: 10.0,
                            angle: 0.0,
                        },
                    )
                }))
            })
        });

        // ── POST-FIX shape (G2 hoist): geometry once, stamp per level ──
        //
        // This is what `compute::execute`'s pocket/profile/zigzag adapters do
        // since 2026-08-20. The composition still runs through
        // `toolpath_at_levels_with_cancel`, so inter-level retracts and the
        // cancellation cadence are identical to the arms above.
        let never = || false;

        group.bench_function(
            BenchmarkId::new("pocket", format!("{label}_hoisted")),
            |b| {
                b.iter(|| {
                    let base = PocketParams {
                        tool_radius: 3.0,
                        stepover: 4.0,
                        cut_depth: 0.0,
                        feed_rate: 1200.0,
                        plunge_rate: 400.0,
                        safe_z: 10.0,
                        climb: true,
                    };
                    let contours = pocket_contours(&poly, base.tool_radius, base.stepover);
                    black_box(
                        toolpath_at_levels_with_cancel(
                            levels,
                            10.0,
                            |z| {
                                Ok(pocket_contours_to_toolpath(
                                    &contours,
                                    &PocketParams {
                                        cut_depth: z,
                                        ..base
                                    },
                                ))
                            },
                            &never,
                        )
                        .expect("never cancelled"),
                    )
                })
            },
        );

        group.bench_function(
            BenchmarkId::new("profile", format!("{label}_hoisted")),
            |b| {
                b.iter(|| {
                    let base = ProfileParams {
                        tool_radius: 3.0,
                        side: ProfileSide::Outside,
                        cut_depth: 0.0,
                        feed_rate: 1200.0,
                        plunge_rate: 400.0,
                        safe_z: 10.0,
                        climb: true,
                        compensate_in_controller: false,
                    };
                    let (contour, _failures) = profile_path_reported(&poly, &base);
                    black_box(
                        toolpath_at_levels_with_cancel(
                            levels,
                            10.0,
                            |z| {
                                Ok(match &contour {
                                    Some(pts) => profile_path_to_toolpath(
                                        pts,
                                        &ProfileParams {
                                            cut_depth: z,
                                            ..base
                                        },
                                    ),
                                    None => rs_cam_core::toolpath::Toolpath::new(),
                                })
                            },
                            &never,
                        )
                        .expect("never cancelled"),
                    )
                })
            },
        );

        group.bench_function(
            BenchmarkId::new("zigzag", format!("{label}_hoisted")),
            |b| {
                b.iter(|| {
                    let base = ZigzagParams {
                        tool_radius: 3.0,
                        stepover: 4.0,
                        cut_depth: 0.0,
                        feed_rate: 1200.0,
                        plunge_rate: 400.0,
                        safe_z: 10.0,
                        angle: 0.0,
                    };
                    let lines = zigzag_lines(&poly, base.tool_radius, base.stepover, base.angle);
                    black_box(
                        toolpath_at_levels_with_cancel(
                            levels,
                            10.0,
                            |z| {
                                Ok(lines_to_toolpath(
                                    &lines,
                                    &ZigzagParams {
                                        cut_depth: z,
                                        ..base
                                    },
                                ))
                            },
                            &never,
                        )
                        .expect("never cancelled"),
                    )
                })
            },
        );
    }

    group.finish();
}

// ── G1: push-cutter query radius = half the part width ──────────────────

/// `push_cutter_batch` in `perf_suite.rs` measures ONE fiber batch at one Z.
/// G1 is about the per-level repetition on top of that: the fiber grid's XY
/// is identical at every Z, and the index prunes nothing, so the cost is
/// `levels × fibers × all_triangles`. Both arms are here so the ratio is
/// readable.
fn bench_gen_waterline(c: &mut Criterion) {
    use rs_cam_core::waterline::{WaterlineParams, waterline_toolpath_with_cancel};

    let mut group = c.benchmark_group("gen_waterline");
    group.sample_size(10);

    let mesh = rolling_field(20.0, 61);
    let index = SpatialIndex::build_auto(&mesh);
    let ball = BallEndmill::new(6.0, 25.0);
    let params = WaterlineParams {
        sampling: 2.0,
        feed_rate: 1200.0,
        plunge_rate: 400.0,
        safe_z: 20.0,
        stock_to_leave: 0.0,
    };
    let never_cancel = || false;

    let top = mesh.bbox.max.z;
    let bottom = mesh.bbox.min.z;
    let span = (top - bottom).max(1e-3);

    for levels in [1_usize, 20] {
        // `waterline_z_levels(start, final, step)` walks down from `start`;
        // pick a step that yields exactly `levels` planes.
        let z_step = span / levels as f64;
        let final_z = top - span * (levels as f64 - 0.5) / levels as f64;
        group.bench_function(
            BenchmarkId::new("rolling61_ball6", format!("L{levels}")),
            |b| {
                b.iter(|| {
                    black_box(
                        waterline_toolpath_with_cancel(
                            &mesh,
                            &index,
                            &ball,
                            top,
                            final_z,
                            z_step,
                            &params,
                            None,
                            &never_cancel,
                        )
                        .expect("never cancelled")
                        .moves
                        .len(),
                    )
                })
            },
        );
    }

    group.finish();
}

// ── G4: `contains_point` with no bbox reject ────────────────────────────

fn bench_gen_contains_point(c: &mut Criterion) {
    let mut group = c.benchmark_group("gen_contains_point");

    let poly = ring_with_holes(40.0, 1400, 200);

    // 10k deterministic query points over a box that overhangs the ring, so
    // a meaningful share of them are OUTSIDE — those are the ones a cached
    // AABB would reject in O(1) and the current code ray-casts in full.
    let queries: Vec<P2> = (0..10_000)
        .map(|i| {
            let t = i as f64;
            P2::new(60.0 * ((t * 0.37).sin()), 60.0 * ((t * 0.61).cos()))
        })
        .collect();

    group.bench_function("polygon_1400v_3holes_10k", |b| {
        b.iter(|| {
            let mut hits = 0usize;
            for q in &queries {
                if poly.contains_point(q) {
                    hits += 1;
                }
            }
            black_box(hits)
        })
    });

    // Multi-region set: the `RegionSet::contains` `.any()` layer on top.
    let regions: Vec<Polygon2> = (0..8)
        .map(|k| {
            let a = std::f64::consts::TAU * k as f64 / 8.0;
            let mut p = Polygon2::new(jittered_ring(12.0, 350));
            for v in &mut p.exterior {
                v.x += 26.0 * a.cos();
                v.y += 26.0 * a.sin();
            }
            p
        })
        .collect();
    let set = RegionSet::from_slice(&regions);

    group.bench_function("regionset_8x350v_10k", |b| {
        b.iter(|| {
            let mut hits = 0usize;
            for q in &queries {
                if set.contains(q) {
                    hits += 1;
                }
            }
            black_box(hits)
        })
    });

    group.finish();
}

// ── G5: O(n²) greedy nearest-neighbour rapid ordering ───────────────────

/// `n` disjoint cutting segments separated by retract + rapid — the shape
/// `optimize_rapid_order` slices into `n` segments and then orders with an
/// uncapped quadratic NN seed (the 2-opt refinement IS capped at
/// `MAX_2OPT_SEGMENTS = 500`; the seed is not).
fn scattered_segments(n: usize) -> Toolpath {
    let mut tp = Toolpath::new();
    for i in 0..n {
        let t = i as f64;
        // Deterministic scatter, deliberately not in a nice raster order.
        let x = 200.0 * (t * 0.9137).sin().abs();
        let y = 200.0 * (t * 0.4271).cos().abs();
        tp.rapid_to(P3::new(x, y, 5.0));
        tp.feed_to(P3::new(x, y, -1.0), 400.0);
        tp.feed_to(P3::new(x + 1.5, y + 0.8, -1.0), 1200.0);
        tp.rapid_to(P3::new(x + 1.5, y + 0.8, 5.0));
    }
    tp
}

fn bench_gen_rapid_order(c: &mut Criterion) {
    use rs_cam_core::tsp::optimize_rapid_order;

    let mut group = c.benchmark_group("gen_rapid_order");
    group.sample_size(10);

    for n in [5_000_usize, 20_000] {
        let tp = scattered_segments(n);
        group.bench_function(BenchmarkId::new("nn_seed", n), |b| {
            b.iter(|| {
                let annotated = AnnotatedToolpath::new(tp.clone());
                black_box(optimize_rapid_order(annotated, 5.0).toolpath.moves.len())
            })
        });
    }

    group.finish();
}

// ── G9: brute-force distance field per v-carve sample ───────────────────

/// A "lettering-ish" fixture: an outer frame with a grid of holes, so the
/// per-sample `point_to_polygon_distance` linear edge scan has a realistic
/// edge count to walk.
fn lettering_polygon(size: f64, hole_verts: usize) -> Polygon2 {
    let h = size / 2.0;
    let mut poly = Polygon2::rectangle(-h, -h, h, h);
    let pitch = size / 4.0;
    for r in 1..=3 {
        for cc in 1..=3 {
            let (cx, cy) = (-h + pitch * cc as f64, -h + pitch * r as f64);
            poly.holes.push(
                (0..hole_verts)
                    .map(|i| {
                        let t = -std::f64::consts::TAU * i as f64 / hole_verts as f64;
                        P2::new(cx + 0.22 * pitch * t.cos(), cy + 0.22 * pitch * t.sin())
                    })
                    .collect(),
            );
        }
    }
    poly
}

fn bench_gen_vcarve_field(c: &mut Criterion) {
    use rs_cam_core::vcarve::{VCarveParams, vcarve_toolpath};

    let mut group = c.benchmark_group("gen_vcarve_field");
    group.sample_size(10);

    let poly = lettering_polygon(60.0, 96);
    let params = VCarveParams {
        half_angle: std::f64::consts::FRAC_PI_4,
        max_depth: 4.0,
        stepover: 1.0,
        feed_rate: 1200.0,
        plunge_rate: 400.0,
        safe_z: 10.0,
        tolerance: 0.05,
        top_z: 0.0,
    };

    group.bench_function("frame60_9holes_tol005", |b| {
        b.iter(|| black_box(vcarve_toolpath(&poly, &params).moves.len()))
    });

    group.finish();
}

// ── G2 (face side): scan rows rebuilt per Z level ───────────────────────

/// Facing a large stock at a fine stepover over many depth levels. The scan
/// rows (inset + slicing + any OneWay normalisation) are Z-independent, so
/// this arm measures exactly what the hoist removed: `levels - 1` redundant
/// rebuilds.
fn bench_gen_face_levels(c: &mut Criterion) {
    use rs_cam_core::face::{FaceDirection, FaceParams, face_toolpath};
    use rs_cam_core::geo::BoundingBox3;

    let mut group = c.benchmark_group("gen_face_levels");
    group.sample_size(20);

    let bounds = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(600.0, 400.0, 25.0),
    };
    for (label, direction) in [
        ("zigzag_20levels", FaceDirection::Zigzag),
        ("oneway_20levels", FaceDirection::OneWay),
    ] {
        let params = FaceParams {
            tool_radius: 6.0,
            stepover: 1.0,
            depth: 10.0,
            depth_per_pass: 0.5,
            feed_rate: 2000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            stock_offset: 5.0,
            direction,
            stock_top_z: 0.0,
        };
        group.bench_function(label, |b| {
            b.iter(|| black_box(face_toolpath(&bounds, &params).moves.len()))
        });
    }

    group.finish();
}

// ── S4 / S6: the end-to-end simulation tail ─────────────────────────────

/// A deterministic three-operation 2.5D project, built through the REAL
/// entry points (`ProjectSession::add_*`) rather than by poking fields, so
/// the per-toolpath clone / mesh-extraction / checkpoint tail S4 is about is
/// inside the measured region.
///
/// Deliberately small: the point is the fixed per-toolpath cost, which
/// scales with the grid, not with how interesting the shape is.
///
/// **The artifact JSON half of S6 is NOT visible here.**
/// `write_simulation_cut_artifact` has exactly one production caller and it
/// lives in `rs_cam_viz` (`compute/worker/execute/mod.rs:428`, commented
/// "viz-only filesystem concern"). Core's `run_simulation` never touches the
/// filesystem, so this bench measures the clone/mesh/checkpoint tail (S4 and
/// S6's per-sample allocation half) only.
fn three_op_session() -> rs_cam_core::session::ProjectSession {
    use rs_cam_core::compute::catalog::OperationConfig;
    use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
    use rs_cam_core::compute::operation_configs::{
        PocketConfig, PocketPattern, ProfileConfig, ZigzagConfig,
    };
    use rs_cam_core::compute::stock_config::StockConfig;
    use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use rs_cam_core::gcode::CoolantMode;
    use rs_cam_core::profile::ProfileSide;
    use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};

    let mut session = ProjectSession::new_empty();

    // 2D ops cut at negative Z, so the stock hangs BELOW z = 0 and its top
    // sits at the world origin plane.
    session.set_stock_config(StockConfig {
        x: 100.0,
        y: 80.0,
        z: 12.0,
        origin_x: -10.0,
        origin_y: -10.0,
        origin_z: -12.0,
        auto_from_model: false,
        ..StockConfig::default()
    });

    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.shank_diameter = 6.35;
    tool.shank_length = 20.0;
    tool.stickout = 45.0;
    tool.flute_count = 2;
    tool.name = "End Mill 6mm".to_owned();
    let tool_idx = session.add_tool(tool);
    let tool_id = session.tools()[tool_idx].id.0;

    let poly = Polygon2::new(vec![
        P2::new(5.0, 5.0),
        P2::new(75.0, 5.0),
        P2::new(75.0, 55.0),
        P2::new(5.0, 55.0),
    ]);
    let model_id = session.add_model(LoadedModel {
        id: 0,
        name: "perf_rect".to_owned(),
        mesh: None,
        polygons: Some(std::sync::Arc::new(vec![poly])),
        drill_targets: std::sync::Arc::new(Vec::new()),
        layers: std::sync::Arc::new(Vec::new()),
        path: std::path::PathBuf::from("synthetic://perf_rect.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    });

    let mut add = |name: &str, op: OperationConfig| {
        let op_type = op.op_type();
        let cfg = ToolpathConfig {
            id: ToolpathId(0),
            name: name.to_owned(),
            enabled: true,
            operation: op,
            dressups: DressupConfig::for_op(op_type),
            heights: HeightsConfig::default(),
            tool_id,
            model_id,
            pre_gcode: None,
            post_gcode: None,
            boundary: BoundaryConfig::default(),
            boundary_inherit: true,
            stock_source: StockSource::default(),
            coolant: CoolantMode::Off,
            face_selection: None,
            debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
            feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
            rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
        };
        session.add_toolpath(0, cfg).expect("add toolpath");
    };

    add(
        "Pocket",
        OperationConfig::Pocket(PocketConfig {
            stepover: 3.0,
            depth: 6.0,
            depth_per_pass: 3.0,
            feed_rate: 1000.0,
            plunge_rate: 400.0,
            climb: true,
            pattern: PocketPattern::Contour,
            angle: 0.0,
            finishing_passes: 0,
            spindle_rpm: Some(18_000),
        }),
    );
    add(
        "Zigzag",
        OperationConfig::Zigzag(ZigzagConfig {
            stepover: 3.0,
            depth: 3.0,
            depth_per_pass: 3.0,
            feed_rate: 1000.0,
            plunge_rate: 400.0,
            angle: 45.0,
            spindle_rpm: Some(18_000),
        }),
    );
    add(
        "Profile",
        OperationConfig::Profile(ProfileConfig {
            side: ProfileSide::Outside,
            depth: 6.0,
            depth_per_pass: 3.0,
            feed_rate: 1000.0,
            plunge_rate: 400.0,
            climb: true,
            ..ProfileConfig::default()
        }),
    );

    session
}

fn bench_sim_e2e_small(c: &mut Criterion) {
    use std::sync::atomic::AtomicBool;

    use rs_cam_core::session::SimulationOptions;

    let mut group = c.benchmark_group("sim_e2e_small");
    group.sample_size(10);

    let cancel = AtomicBool::new(false);
    let mut session = three_op_session();
    for i in 0..session.toolpath_count() {
        session
            .generate_toolpath(i, &cancel)
            .expect("generation must succeed");
    }

    for resolution in [1.0_f64, 0.5] {
        // `adaptive_feed_modulation` defaults to TRUE; pin every dial so the
        // measured work does not move with a default change.
        let opts = SimulationOptions {
            resolution,
            skip_ids: Vec::new(),
            metrics_enabled: true,
            auto_resolution: false,
            use_predicted_feed_in_gates: false,
            adaptive_feed_modulation: false,
            modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
            modulation_aggressiveness: 1.0,
        };
        group.bench_function(
            BenchmarkId::new("3op_2d", format!("res{resolution}")),
            |b| {
                b.iter(|| {
                    let sim = session
                        .run_simulation(&opts, &cancel)
                        .expect("simulation completes");
                    black_box(sim.total_moves)
                })
            },
        );
    }

    group.finish();
}

// ── S5: the fixpoint ladder, memo off vs on ─────────────────────────────

/// One raster pass of a synthetic rest chain: each op is offset in Y and cut
/// deeper, so it both re-passes the previous op's ground and takes fresh
/// material — the shape a `FromRemainingStock` cascade produces.
fn ladder_pass(index: usize) -> std::sync::Arc<AnnotatedToolpath> {
    let y0 = 6.0 + 4.0 * index as f64;
    let depth = -1.0 - 0.6 * index as f64;
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(4.0, y0, 5.0));
    for lane in 0..14 {
        let y = y0 + 0.4 * lane as f64;
        let (a, b) = if lane % 2 == 0 {
            (4.0, 96.0)
        } else {
            (96.0, 4.0)
        };
        tp.feed_to(P3::new(a, y, depth), 1200.0);
        tp.feed_to(P3::new(b, y, depth), 1200.0);
    }
    tp.rapid_to(P3::new(96.0, y0, 5.0));
    std::sync::Arc::new(AnnotatedToolpath::new(tp))
}

fn ladder_request(
    chain: &[std::sync::Arc<AnnotatedToolpath>],
    count: usize,
) -> rs_cam_core::compute::simulate::SimulationRequest {
    use rs_cam_core::compute::simulate::{SimGroupEntry, SimToolpathEntry, SimulationRequest};
    use rs_cam_core::geo::BoundingBox3;

    let tool = || {
        rs_cam_core::tool::ToolDefinition::new(
            Box::new(FlatEndmill::new(6.0, 25.0)),
            6.0,
            20.0,
            25.0,
            45.0,
            2,
            rs_cam_core::compute::tool_config::ToolMaterial::Carbide,
        )
    };
    SimulationRequest {
        groups: vec![SimGroupEntry {
            toolpaths: chain
                .iter()
                .take(count)
                .enumerate()
                .map(|(i, tp)| SimToolpathEntry {
                    id: ToolpathId(i + 1),
                    name: format!("Pass{i}"),
                    annotated: std::sync::Arc::clone(tp),
                    tool: tool(),
                    flute_count: 2,
                    tool_summary: "6mm Flat".to_owned(),
                    semantic_trace: None,
                    spindle_rpm: None,
                    metrics_not_applicable: false,
                    drill_op: None,
                    operation_config_hash: i as u64,
                })
                .collect(),
            direction: StockCutDirection::FromTop,
            local_stock_bbox: None,
            local_to_global: None,
            phantom_prior_stock: None,
        }],
        stock_bbox: BoundingBox3 {
            min: P3::new(0.0, 0.0, -8.0),
            max: P3::new(100.0, 60.0, 0.0),
        },
        stock_top_z: 0.0,
        resolution: 0.4,
        metric_options: rs_cam_core::simulation_cut::SimulationMetricOptions {
            enabled: true,
            capture_arc_engagement: true,
        },
        spindle_rpm: 18_000,
        rapid_feed_mm_min: 5000.0,
        model_mesh: None,
        kinematics: None,
    }
}

/// S5. A **paired same-session A/B**: both arms run the identical three-round
/// ladder (2 → 4 → 6 toolpaths at 0.4 mm, the wanaka reference resolution) in
/// one criterion group. The only difference is whether each round leaves a
/// prefix snapshot for the next one.
///
/// `memo_off` is the pre-S5 behaviour exactly — three full replays — so the
/// ratio is the finding, measured rather than argued. Cross-day absolutes on
/// this box are not comparable (`BASELINES.md`, "Measurement discipline"),
/// which is why both arms are here rather than one arm plus a stored number.
fn bench_sim_fixpoint_ladder(c: &mut Criterion) {
    use std::sync::atomic::AtomicBool;

    use rs_cam_core::compute::sim_prefix::{SimMemo, SimPrefixCache};
    use rs_cam_core::compute::simulate::run_simulation_memoized;

    let mut group = c.benchmark_group("sim_fixpoint_ladder");
    group.sample_size(10);

    let chain: Vec<_> = (0..6).map(ladder_pass).collect();
    let rounds: Vec<_> = [2_usize, 4, 6]
        .into_iter()
        .map(|n| ladder_request(&chain, n))
        .collect();
    let cancel = AtomicBool::new(false);

    for (label, memoize) in [("memo_off", false), ("memo_on", true)] {
        group.bench_function(BenchmarkId::new("3round_6op_res0.4", label), |b| {
            b.iter(|| {
                let mut cache = SimPrefixCache::new();
                let mut moves = 0;
                for request in &rounds {
                    let result = run_simulation_memoized(
                        request,
                        &cancel,
                        |_p| {},
                        Some(SimMemo {
                            cache: &mut cache,
                            store: memoize,
                        }),
                    )
                    .expect("simulation completes");
                    moves += result.total_moves;
                }
                black_box(moves)
            })
        });
    }

    group.finish();
}

// ── V1 (core side): triage + measurability over a whole trace ───────────

/// Synthetic cut samples at trace scale. Deterministic, no RNG.
///
/// One in 17 samples is near-air so the air-cut / low-engagement channels
/// are non-empty; that is what makes the advisory dedup and cap do work
/// rather than short-circuit on an empty vector.
fn synthetic_samples(n_samples: usize, toolpath_count: usize) -> Vec<SimulationCutSample> {
    use rs_cam_core::simulation_cut::{CutKinematics, Engagement, SimulationCutSample};

    (0..n_samples)
        .map(|i| {
            let toolpath_id = i % toolpath_count.max(1);
            let radial = if i % 17 == 0 { 0.01 } else { 0.35 };
            let segment_time_s = 0.01;
            SimulationCutSample {
                toolpath_id: ToolpathId(toolpath_id),
                move_index: i / toolpath_count.max(1),
                sample_index: i,
                position: [
                    (i % 997) as f64 * 0.1,
                    (i % 389) as f64 * 0.1,
                    -1.0 - (i % 7) as f64 * 0.2,
                ],
                cumulative_time_s: i as f64 * segment_time_s,
                segment_time_s,
                is_cutting: true,
                cut_kinematics: CutKinematics::Linear,
                feed_rate_mm_min: 1200.0,
                spindle_rpm: 18_000,
                flute_count: 2,
                axial_doc_mm: 1.5,
                axial_engagement_mm: 1.5,
                plunge_descent_mm: 0.0,
                arc_engagement_radians: Some(std::f64::consts::FRAC_PI_2),
                chipload_mm_per_tooth: 1200.0 / 18_000.0 / 2.0,
                effective_chip_thickness_mm: Some(0.025),
                engagement: Engagement {
                    radial_woc_fraction: radial,
                    ..Engagement::default()
                },
                removed_volume_est_mm3: if radial < 0.02 { 0.0 } else { 0.8 },
                mrr_mm3_s: if radial < 0.02 { 0.0 } else { 80.0 },
                semantic_item_id: None,
                span_path: Vec::new(),
                in_transit_span: false,
                source_intent: None,
            }
        })
        .collect()
}

fn bench_viz_triage_build(c: &mut Criterion) {
    use std::collections::BTreeMap;

    use rs_cam_core::sim_measurability::MeasurabilityReport;
    use rs_cam_core::sim_triage::{SimulationTriage, TriageInputs};
    use rs_cam_core::simulation_cut::SimulationCutTrace;

    let mut group = c.benchmark_group("viz_triage_build");
    group.sample_size(10);

    // The GUI rebuilds BOTH of these every frame from the full trace
    // (`ui/sim_diagnostics.rs:588-589`). Trace assembly itself is already
    // benched in `perf_suite::simulation_cut_trace_aggregation`, so it stays
    // in setup here — what is measured is the per-frame re-derivation.
    for n_samples in [100_000_usize, 600_000] {
        let trace = SimulationCutTrace::from_samples(0.25, synthetic_samples(n_samples, 8));
        let tool_diameters_mm: BTreeMap<ToolpathId, f64> =
            (0..8).map(|i| (ToolpathId(i), 6.0)).collect();

        group.bench_function(BenchmarkId::new("measurability", n_samples), |b| {
            b.iter(|| {
                black_box(
                    MeasurabilityReport::from_trace(&trace, Some(0.25))
                        .entries
                        .len(),
                )
            })
        });

        let measurability = MeasurabilityReport::from_trace(&trace, Some(0.25));
        group.bench_function(BenchmarkId::new("triage", n_samples), |b| {
            b.iter(|| {
                let inputs = TriageInputs {
                    trace: &trace,
                    measurability: &measurability,
                    diagnostics: &[],
                    rapid_collisions: &[],
                    holder_collisions: &[],
                    tool_diameters_mm: &tool_diameters_mm,
                    region_of: None,
                };
                black_box(SimulationTriage::build(&inputs).advisories.total_matching)
            })
        });
    }

    group.finish();
}

// ── Group all benchmarks ────────────────────────────────────────────────

criterion_group!(
    benches,
    bench_sim_kernel_lateral,
    bench_sim_kernel_plunge,
    bench_sim_e2e_small,
    bench_sim_dispatch_ab,
    bench_sim_fixpoint_ladder,
    bench_gen_depth,
    bench_gen_waterline,
    bench_gen_contains_point,
    bench_gen_rapid_order,
    bench_gen_vcarve_field,
    bench_gen_face_levels,
    bench_viz_triage_build,
);
criterion_main!(benches);
