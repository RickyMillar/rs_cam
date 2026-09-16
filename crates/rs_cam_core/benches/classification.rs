//! M3 baseline instrument: how long the fine true-surface classification grid
//! takes, as a function of grid size and mesh.
//!
//! `planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md` item M3
//! opens with a measurement — "the 849² production row measured 19–47 s" —
//! that no committed instrument reproduces. This file is that instrument, and
//! it lands **before** any candidate so the baseline cannot be re-derived
//! after the fact from a tree that already contains an optimisation.
//!
//! What is measured is exactly what production runs: the tiny-ball
//! (Ø`CLASSIFICATION_PROBE_DIAMETER_MM`) drop-cutter sweep that
//! `finish_setup::build_classification_surface_with_policy_and_cancel` issues,
//! at a caller-pinned grid so the row count is the independent variable rather
//! than a consequence of the tool.
//!
//! ```text
//! cargo bench -p rs_cam_core --bench classification            # CI-speed synthetic rows
//! RS_CAM_M3_HEAVY=1 cargo bench -p rs_cam_core --bench classification
//! ```
//!
//! The heavy rows are **opt-in** rather than always-on: they are the criterion
//! equivalent of `#[ignore]` (a criterion target has no ignore attribute), and
//! at 849² on the 10 MB terrain one sample is tens of seconds — criterion's
//! minimum sample count would put a single run into the tens of minutes and no
//! one would ever run the file. Heavy rows also use a flat sampling mode and a
//! reduced sample count for the same reason.
//!
//! Criterion always builds `--release`, so every number here is a release
//! number.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

use rs_cam_core::finish_setup::CLASSIFICATION_PROBE_DIAMETER_MM;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::slope::SurfaceHeightmap;
use rs_cam_core::tool::BallEndmill;

mod support;
use support::rolling_field;

/// Set `RS_CAM_M3_HEAVY=1` to include the real-fixture rows.
fn heavy_enabled() -> bool {
    std::env::var("RS_CAM_M3_HEAVY").is_ok_and(|v| v != "0" && !v.is_empty())
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

// ── Fixtures ────────────────────────────────────────────────────────────

fn load_terrain(name: &str) -> Option<TriangleMesh> {
    let candidates = [
        repo_root().join("fixtures").join(name),
        repo_root()
            .join("crates/rs_cam_core/tests/fixtures")
            .join(name),
    ];
    candidates
        .iter()
        .find(|p| p.exists())
        .and_then(|p| TriangleMesh::from_stl(p).ok())
}

// ── The measured operation ──────────────────────────────────────────────

/// One classification grid, exactly as production computes it: tiny-ball
/// drop-cutter per cell plus the coverage predicate, at a pinned grid.
fn classify(mesh: &TriangleMesh, index: &SpatialIndex, side: usize) -> SurfaceHeightmap {
    let probe = BallEndmill::new(CLASSIFICATION_PROBE_DIAMETER_MM, 1.0);
    let bbox = &mesh.bbox;
    // Cover the mesh footprint with `side` cells per axis. The grid ORIGIN
    // and floor follow the production builder; only the cell size is solved
    // for, so the row is named by the number production would report.
    let cell = ((bbox.max.x - bbox.min.x).max(bbox.max.y - bbox.min.y)) / (side - 1) as f64;
    let never = || false;
    SurfaceHeightmap::from_mesh_with_cancel(
        mesh, index, &probe, bbox.min.x, bbox.min.y, side, side, cell, bbox.min.z, &never,
    )
    .expect("uncancellable classification never cancels")
}

// ── Benchmarks ──────────────────────────────────────────────────────────

/// CI-speed rows: a synthetic field at the small end of the production range.
fn bench_classification_synthetic(c: &mut Criterion) {
    let mesh = rolling_field(30.0, 121);
    let index = SpatialIndex::build_auto(&mesh);
    let mut group = c.benchmark_group("classification/current/synthetic");
    for side in [64usize, 143] {
        group.throughput(criterion::Throughput::Elements((side * side) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(side), &side, |b, &side| {
            b.iter(|| black_box(classify(&mesh, &index, side)));
        });
    }
    group.finish();
}

/// The spatial-index query itself, isolated from the contact math.
///
/// Every classification cell issues two `SpatialIndex::query` calls, and each
/// one allocates a dedup bitset sized by the WHOLE mesh. Whether that
/// allocation or the drop-cutter contact math dominates decides what a
/// candidate should attack, so the baseline measures them apart as well as
/// together.
fn bench_classification_query_only(c: &mut Criterion) {
    let mesh = rolling_field(30.0, 121);
    let index = SpatialIndex::build_auto(&mesh);
    let bbox = &mesh.bbox;
    let side = 143usize;
    let cell = ((bbox.max.x - bbox.min.x).max(bbox.max.y - bbox.min.y)) / (side - 1) as f64;
    let radius = CLASSIFICATION_PROBE_DIAMETER_MM / 2.0;
    let mut group = c.benchmark_group("classification/current/query_only");
    group.throughput(criterion::Throughput::Elements((side * side) as u64));
    group.bench_function("143", |b| {
        b.iter(|| {
            let mut hits = 0usize;
            for i in 0..side * side {
                let x = bbox.min.x + (i % side) as f64 * cell;
                let y = bbox.min.y + (i / side) as f64 * cell;
                hits += index.query(x, y, radius).len();
                hits += index.query(x, y, 0.0).len();
            }
            black_box(hits)
        });
    });
    group.finish();
}

/// Real-fixture rows at the production sizes. Opt-in; see the module docs.
fn bench_classification_real_fixture(c: &mut Criterion) {
    if !heavy_enabled() {
        eprintln!(
            "classification/current/terrain: skipped (set RS_CAM_M3_HEAVY=1 to run the \
             real-fixture rows; they take minutes each at 849²)"
        );
        return;
    }
    for name in ["terrain_small.stl", "terrain.stl"] {
        let Some(mesh) = load_terrain(name) else {
            eprintln!("classification/current/terrain: {name} not found, skipped");
            continue;
        };
        let index = SpatialIndex::build_auto(&mesh);
        let mut group = c.benchmark_group(format!("classification/current/{name}"));
        group
            .sampling_mode(criterion::SamplingMode::Flat)
            .sample_size(10)
            .measurement_time(Duration::from_secs(30))
            .warm_up_time(Duration::from_secs(3));
        for side in [143usize, 425, 849] {
            group.throughput(criterion::Throughput::Elements((side * side) as u64));
            group.bench_with_input(BenchmarkId::from_parameter(side), &side, |b, &side| {
                b.iter(|| black_box(classify(&mesh, &index, side)));
            });
        }
        group.finish();
    }
}

criterion_group!(
    benches,
    bench_classification_synthetic,
    bench_classification_query_only,
    bench_classification_real_fixture
);
criterion_main!(benches);
