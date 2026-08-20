//! G8 — spatial index / silhouette / transformed-mesh reuse.
//!
//! Two jobs:
//!
//! 1. **Equivalence.** The CSR conversion of [`SpatialIndex`] must return the
//!    exact same candidate sets, in the same order, as the historical
//!    `Vec<Vec<usize>>` form. Several downstream consumers are order-sensitive
//!    in their tie-breaking, so a reordered candidate list can change an
//!    emitted toolpath. The reference builder below is a verbatim transcript of
//!    the pre-CSR `SpatialIndex::build` body, kept here so the assertion is
//!    against the old algorithm rather than against the new one restated.
//!
//! 2. **Measurement.** `#[ignore]`d timing probes (run with `--ignored
//!    --nocapture`) for the three repeated units the G8 row names: index build,
//!    silhouette rasterisation, setup mesh transform. Set `G8_TERRAIN` to an
//!    STL path to measure the review's reference workload; otherwise the
//!    in-repo `fixtures/terrain_small.stl` is used.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

mod common;

use rs_cam_core::mesh::{SpatialIndex, TriangleMesh, make_test_hemisphere};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Instant;

/// The cache counters are process-global, so every test that reads a *delta*
/// off them must not run concurrently with another that builds. Cargo runs
/// the tests in one binary on a thread pool; this serialises just those.
fn counter_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    #[allow(clippy::unwrap_used)] // poisoning would mean another test panicked
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}

// ── The pre-CSR reference implementation ─────────────────────────────────

/// Verbatim transcript of the `Vec<Vec<usize>>` `SpatialIndex::build` that
/// shipped before the CSR conversion, reduced to the parts that determine
/// the candidate sets. Returns `(cells, cell_count_x, cell_count_y)`.
fn reference_cells(mesh: &TriangleMesh, cell_size: f64) -> (Vec<Vec<usize>>, usize, usize) {
    let bbox = &mesh.bbox;
    let origin_x = bbox.min.x;
    let origin_y = bbox.min.y;

    let extent_x = bbox.max.x - bbox.min.x;
    let extent_y = bbox.max.y - bbox.min.y;
    let max_extent = extent_x.max(extent_y);
    let cell_size = if max_extent > 0.0 {
        cell_size.min(max_extent / 4.0)
    } else {
        cell_size
    };

    let cell_count_x = ((bbox.max.x - bbox.min.x) / cell_size).ceil() as usize + 1;
    let cell_count_y = ((bbox.max.y - bbox.min.y) / cell_size).ceil() as usize + 1;
    let total_cells = cell_count_x * cell_count_y;

    let mut cells = vec![Vec::new(); total_cells];

    for (i, face) in mesh.faces.iter().enumerate() {
        let x0 = ((face.bbox.min.x - origin_x) / cell_size).floor() as isize;
        let x1 = ((face.bbox.max.x - origin_x) / cell_size).floor() as isize;
        let y0 = ((face.bbox.min.y - origin_y) / cell_size).floor() as isize;
        let y1 = ((face.bbox.max.y - origin_y) / cell_size).floor() as isize;

        let x0 = x0.max(0) as usize;
        let x1 = (x1 as usize).min(cell_count_x - 1);
        let y0 = y0.max(0) as usize;
        let y1 = (y1 as usize).min(cell_count_y - 1);

        for cy in y0..=y1 {
            for cx in x0..=x1 {
                cells[cy * cell_count_x + cx].push(i);
            }
        }
    }

    (cells, cell_count_x, cell_count_y)
}

/// The auto cell size, duplicated so the reference can be built at the same
/// resolution `build_auto` picks. Mirrors `SpatialIndex::build_auto`.
fn reference_auto_cell(mesh: &TriangleMesh) -> f64 {
    let bbox = &mesh.bbox;
    let extent_x = bbox.max.x - bbox.min.x;
    let extent_y = bbox.max.y - bbox.min.y;
    let max_extent = extent_x.max(extent_y);
    let tri_count = (mesh.triangles.len().max(1)) as f64;
    let area = (extent_x * extent_y).max(1e-6);
    let density_cell = (8.0 * area / tri_count).sqrt();
    let coarse_cell = (max_extent / 50.0).max(1.0);
    let min_cell = (area / 1_000_000.0).sqrt();
    density_cell.min(coarse_cell).max(min_cell).max(0.1)
}

// ── Fixtures ─────────────────────────────────────────────────────────────

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures")
}

fn terrain() -> TriangleMesh {
    TriangleMesh::from_stl(&fixtures_dir().join("terrain_small.stl")).expect("terrain_small.stl")
}

/// A ragged mesh whose triangles straddle many cells, so multi-cell fill and
/// the clamp branches are exercised rather than a uniform grid.
fn ragged() -> TriangleMesh {
    let mut m = make_test_hemisphere(25.0, 24);
    // Stretch X so triangle bboxes span several cells in one axis only.
    for v in &mut m.vertices {
        v.x *= 7.0;
    }
    TriangleMesh::from_raw(m.vertices.clone(), m.triangles.clone())
}

// ── 1. CSR equivalence ───────────────────────────────────────────────────

fn assert_cells_identical(mesh: &TriangleMesh, cell_size: f64, label: &str) {
    let (want, nx, ny) = reference_cells(mesh, cell_size);
    let got = SpatialIndex::build(mesh, cell_size);

    assert_eq!(got.cell_count_x(), nx, "{label}: cell_count_x");
    assert_eq!(got.cell_count_y(), ny, "{label}: cell_count_y");
    assert_eq!(got.cell_count(), want.len(), "{label}: total cells");

    for (idx, expected) in want.iter().enumerate() {
        let actual = got.cell_slice(idx);
        assert_eq!(
            actual,
            expected.as_slice(),
            "{label}: cell {idx} differs — CSR must be element-wise identical, \
             not merely set-equal (downstream tie-breaking is order-sensitive)"
        );
    }
}

#[test]
fn csr_cells_are_elementwise_identical_to_vec_of_vec() {
    let terrain = terrain();
    let ragged = ragged();
    let hemi = make_test_hemisphere(25.0, 16);
    let flat = rs_cam_core::mesh::make_test_flat(40.0);

    for &cell in &[0.5, 1.0, 2.0, 5.0, 10.0, 37.5] {
        assert_cells_identical(&terrain, cell, &format!("terrain@{cell}"));
        assert_cells_identical(&ragged, cell, &format!("ragged@{cell}"));
        assert_cells_identical(&hemi, cell, &format!("hemisphere@{cell}"));
        assert_cells_identical(&flat, cell, &format!("flat@{cell}"));
    }

    // And at the auto resolution, which is the one the generation path uses.
    for m in [&terrain, &ragged, &hemi, &flat] {
        assert_cells_identical(m, reference_auto_cell(m), "auto");
    }
}

#[test]
fn csr_queries_match_the_reference_cell_union() {
    // A query is a dedup'd concatenation of the covered cells in (y, x) scan
    // order. Rebuild that from the reference and compare to `query`, so the
    // assertion covers the query path and not just the storage.
    let mesh = terrain();
    let cell = 10.0;
    let (want, nx, ny) = reference_cells(&mesh, cell);
    let index = SpatialIndex::build(&mesh, cell);

    let origin_x = mesh.bbox.min.x;
    let origin_y = mesh.bbox.min.y;
    // `build` clamps cell_size; recompute the clamped value the same way.
    let max_extent = (mesh.bbox.max.x - mesh.bbox.min.x).max(mesh.bbox.max.y - mesh.bbox.min.y);
    let cell = if max_extent > 0.0 {
        cell.min(max_extent / 4.0)
    } else {
        cell
    };

    let mut checked = 0usize;
    for step in 0..12 {
        let t = step as f64 / 11.0;
        let cx = mesh.bbox.min.x + t * (mesh.bbox.max.x - mesh.bbox.min.x);
        let cy = mesh.bbox.min.y + (1.0 - t) * (mesh.bbox.max.y - mesh.bbox.min.y);
        for &radius in &[0.0, 3.175, 12.5] {
            let x0 = (((cx - radius - origin_x) / cell).floor() as isize).max(0) as usize;
            let x1 = ((((cx + radius - origin_x) / cell).floor() as isize).max(0) as usize)
                .min(nx.saturating_sub(1));
            let y0 = (((cy - radius - origin_y) / cell).floor() as isize).max(0) as usize;
            let y1 = ((((cy + radius - origin_y) / cell).floor() as isize).max(0) as usize)
                .min(ny.saturating_sub(1));

            let mut expected: Vec<usize> = Vec::new();
            let mut seen = vec![false; mesh.faces.len()];
            for yy in y0..=y1 {
                for xx in x0..=x1 {
                    for &tri in &want[yy * nx + xx] {
                        if !seen[tri] {
                            seen[tri] = true;
                            expected.push(tri);
                        }
                    }
                }
            }

            assert_eq!(
                index.query(cx, cy, radius),
                expected,
                "query({cx}, {cy}, {radius}) diverged from the reference union"
            );
            checked += 1;
        }
    }
    assert!(checked >= 30, "expected a real sample of queries");
}

// ── 2. Cache observational invisibility ──────────────────────────────────

#[test]
fn cached_index_equals_a_fresh_build() {
    let _guard = counter_lock();
    use rs_cam_core::geom_cache::cached_auto_index;
    use std::sync::Arc;

    let mesh = Arc::new(terrain());
    let fresh = SpatialIndex::build_auto(&mesh);

    let first = cached_auto_index(&mesh);
    let second = cached_auto_index(&mesh);

    // Same allocation on the second call — that is the whole point.
    assert!(
        Arc::ptr_eq(&first, &second),
        "second lookup rebuilt instead of reusing"
    );

    // And the cached value is what a fresh build would have produced, cell for
    // cell, in order. Asserted rather than assumed.
    assert_eq!(first.cell_count(), fresh.cell_count());
    assert_eq!(first.cell_count_x(), fresh.cell_count_x());
    assert_eq!(first.cell_count_y(), fresh.cell_count_y());
    for idx in 0..fresh.cell_count() {
        assert_eq!(
            first.cell_slice(idx),
            fresh.cell_slice(idx),
            "cached cell {idx} diverged from a fresh build"
        );
    }
    assert_eq!(first.cell_size().to_bits(), fresh.cell_size().to_bits());
}

#[test]
fn a_different_mesh_at_the_same_address_is_not_a_hit() {
    let _guard = counter_lock();
    use rs_cam_core::geom_cache::cached_auto_index;
    use std::sync::Arc;

    // Fill, drop, refill. Without a liveness-checked key this is the ABA case:
    // the allocator may hand the second `Arc` the address the first one just
    // released, and a bare-pointer cache would answer with the first mesh's
    // index for the second mesh's content.
    let mut seen_reuse = false;
    let first_ptr = {
        let a = Arc::new(make_test_hemisphere(25.0, 12));
        let idx = cached_auto_index(&a);
        assert_eq!(idx.total_triangles(), a.faces.len());
        Arc::as_ptr(&a)
    };

    for _ in 0..64 {
        let b = Arc::new(rs_cam_core::mesh::make_test_flat(40.0));
        if Arc::as_ptr(&b) == first_ptr {
            seen_reuse = true;
        }
        let idx = cached_auto_index(&b);
        assert_eq!(
            idx.total_triangles(),
            b.faces.len(),
            "cache returned an index built for a different mesh"
        );
    }
    // Whether or not the allocator actually reused the address, the assertion
    // above is the one that matters; report the outcome so the test's value is
    // legible when it passes trivially.
    println!("address reuse observed: {seen_reuse}");
}

#[test]
fn cache_is_bounded() {
    let _guard = counter_lock();
    use rs_cam_core::geom_cache::{cache_len, cached_auto_index};
    use std::sync::Arc;

    let mut live = Vec::new();
    for d in 4..24 {
        let m = Arc::new(make_test_hemisphere(10.0 + d as f64, d));
        let _ = cached_auto_index(&m);
        live.push(m);
    }
    assert!(
        cache_len() <= rs_cam_core::geom_cache::CAPACITY,
        "cache grew past its stated capacity: {} > {}",
        cache_len(),
        rs_cam_core::geom_cache::CAPACITY
    );
}

// ── 2b. The count, end to end through the session ────────────────────────

/// The G8 claim, measured on the real generation entry point rather than
/// argued: N toolpaths over one model resolve N times and build the spatial
/// index **once**.
///
/// Before this change `resolve_generation_inputs` called
/// `SpatialIndex::build_auto` unconditionally on every call, so this count was
/// exactly N — and N again on each fixpoint round of `generate_all`.
#[test]
fn eight_toolpaths_over_one_model_build_one_index() {
    use common::session::{
        generate, mesh_model, pinned_heights, single_op_session_with, stock_over, toolpath_config,
    };
    use common::tools::ball_tool_config;
    use rs_cam_core::compute::catalog::OperationConfig;
    use rs_cam_core::compute::operation_configs::DropCutterConfig;

    let _guard = counter_lock();
    rs_cam_core::geom_cache::clear();
    rs_cam_core::geom_cache::reset_stats();

    const N: usize = 8;
    let half = 20.0;
    let mut session = single_op_session_with(
        stock_over(half, 6.0),
        ball_tool_config(3.0),
        mesh_model(make_test_hemisphere(15.0, 20), "hemi"),
        "DropCutter 0",
        OperationConfig::DropCutter(DropCutterConfig::default()),
        |cfg| cfg.heights = pinned_heights(0.0, -6.0),
    );
    let tool_id = session.tools()[0].id.0;
    let model_id = session.models()[0].id;
    for i in 1..N {
        let mut cfg = toolpath_config(
            &format!("DropCutter {i}"),
            OperationConfig::DropCutter(DropCutterConfig::default()),
            tool_id,
            model_id,
        );
        cfg.heights = pinned_heights(0.0, -6.0);
        session.add_toolpath(0, cfg).expect("add toolpath");
    }
    assert_eq!(session.toolpath_configs().len(), N);

    for i in 0..N {
        generate(&mut session, i);
    }

    let stats = rs_cam_core::geom_cache::stats();
    println!(
        "{N} toolpaths, one model: index_builds={} index_hits={}",
        stats.index_builds, stats.index_hits
    );
    assert_eq!(
        stats.index_builds, 1,
        "one model, one auto cell size — the index must be built exactly once \
         across {N} generations (it was built {N} times before G8)"
    );
    assert_eq!(
        stats.index_hits,
        (N - 1) as u64,
        "every generation after the first must have hit the memo"
    );
}

// ── 3. Measurement (ignored) ─────────────────────────────────────────────

fn measurement_mesh() -> (TriangleMesh, String) {
    match std::env::var("G8_TERRAIN") {
        Ok(p) if !p.is_empty() => {
            let mesh = TriangleMesh::from_stl(std::path::Path::new(&p)).expect("G8_TERRAIN stl");
            (mesh, p)
        }
        _ => (terrain(), "fixtures/terrain_small.stl".to_owned()),
    }
}

#[test]
#[ignore = "timing probe; run with --ignored --nocapture"]
fn g8_unit_costs() {
    let (mesh, label) = measurement_mesh();
    println!("mesh: {label}");
    println!(
        "  triangles={} vertices={} auto_cell={:.4}",
        mesh.faces.len(),
        mesh.vertices.len(),
        reference_auto_cell(&mesh)
    );

    // Enough repetitions that the smallest unit still spans several
    // milliseconds — the in-repo fixture is 16x smaller than the reference
    // terrain and 3 reps of it round to one significant figure.
    let reps = if mesh.faces.len() < 100_000 { 60 } else { 3 };

    let t = Instant::now();
    for _ in 0..reps {
        std::hint::black_box(SpatialIndex::build_auto(&mesh));
    }
    println!(
        "  SpatialIndex::build_auto  {:.3} ms",
        t.elapsed().as_secs_f64() * 1000.0 / reps as f64
    );

    let t = Instant::now();
    for _ in 0..reps {
        std::hint::black_box(SpatialIndex::build(&mesh, 10.0));
    }
    println!(
        "  SpatialIndex::build(10.0) {:.3} ms",
        t.elapsed().as_secs_f64() * 1000.0 / reps as f64
    );

    let t = Instant::now();
    for _ in 0..reps {
        std::hint::black_box(reference_cells(&mesh, 10.0));
    }
    println!(
        "  reference Vec<Vec>(10.0)  {:.3} ms",
        t.elapsed().as_secs_f64() * 1000.0 / reps as f64
    );

    let t = Instant::now();
    for _ in 0..reps {
        std::hint::black_box(reference_cells(&mesh, reference_auto_cell(&mesh)));
    }
    println!(
        "  reference Vec<Vec>(auto)  {:.3} ms",
        t.elapsed().as_secs_f64() * 1000.0 / reps as f64
    );

    let t = Instant::now();
    for _ in 0..reps {
        std::hint::black_box(rs_cam_core::boundary::model_silhouette(&mesh, None));
    }
    println!(
        "  model_silhouette(None)    {:.3} ms",
        t.elapsed().as_secs_f64() * 1000.0 / reps as f64
    );

    // The setup transform's cost: a full re-derivation of `faces` (normal +
    // bbox per triangle) plus the ~144 B/triangle allocation.
    let t = Instant::now();
    for _ in 0..reps {
        let verts: Vec<rs_cam_core::geo::P3> = mesh
            .vertices
            .iter()
            .map(|v| rs_cam_core::geo::P3::new(v.x, -v.y, -v.z))
            .collect();
        std::hint::black_box(TriangleMesh::from_raw(verts, mesh.triangles.clone()));
    }
    println!(
        "  transform_mesh_to_setup   {:.3} ms  ({:.1} MB faces)",
        t.elapsed().as_secs_f64() * 1000.0 / reps as f64,
        (mesh.faces.len() * std::mem::size_of::<rs_cam_core::geo::Triangle>()) as f64 / 1.0e6
    );
}
