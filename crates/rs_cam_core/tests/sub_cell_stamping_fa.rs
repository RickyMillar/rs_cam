//! F.a sub-cell stamping regression tests
//! (`planning/DEXEL_Z_ONLY_INVESTIGATION.md` §6.F / §8 Step 4).
//!
//! Locks in the area-weighted coverage behaviour that replaces the prior
//! binary cell-center stamping:
//!
//! 1. A single point-stamp removes ~π·r²·depth of material across the cell
//!    grid (analytical disk area, sub-cell-quantised to within ~1 cell).
//! 2. A single segment stamp removes the stadium area (π·r² + 2·r·L) times
//!    depth.
//! 3. Sub-cell coverage reaches 1.0 at the cells fully inside the cutter
//!    footprint and ∈ (0, 1) at boundary cells.
//!
//!    **Read this before changing tests 3, 5 or 6.** These three used to read
//!    `DexelGrid::coverage_at` — a per-cell `coverage_max: Vec<f32>` sidecar
//!    that recorded the running max of sub-cell coverage. That sidecar was
//!    deleted (PERF_REVIEW S4a): it had **zero production consumers**, its own
//!    docstring said "purely observational", and it cost 4 B per cell in every
//!    full-grid clone plus a third cache stream in the stamp kernel's inner
//!    loop. This file was its only reader in the workspace.
//!
//!    The guarantee did not go with it. On FRESH stock a first stamp's
//!    coverage at a cell is recoverable exactly from the observable it drives:
//!    `ray_blend_above` lowers the top by `coverage × depth`, so
//!    `coverage = (top₀ − top) / depth`. [`observed_coverage`] does that, and
//!    the assertions now run against the blended ray top — the quantity the
//!    simulator actually consumes — rather than against a sidecar nothing
//!    read. Test 3 additionally pins the 1/16 quantisation of the 4×4
//!    sub-sample fan, which the sidecar reading never checked.
//! 4. Drill toolpaths produce identical engagement metrics pre/post F.a —
//!    counter-test, because drill ops bypass the stamping path entirely
//!    (Step 3 PR1, analytical removal kernel).
//! 5. `ray_blend_above` / `ray_blend_below` primitives behave as expected
//!    in volume-invariant fashion (smoke test that the kernel primitives
//!    haven't regressed — full ray_blend unit tests live in
//!    `src/dexel.rs::tests`).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::{
    dexel::{DexelGrid, ray_material_length, ray_top},
    dexel_stock::{StockCutDirection, TriDexelStock},
    geo::{BoundingBox3, P3},
    radial_profile::RadialProfileLUT,
    tool::{FlatEndmill, MillingCutter},
};

fn fresh_stock(cs: f64) -> TriDexelStock {
    let bbox = BoundingBox3 {
        min: P3::new(-10.0, -10.0, 0.0),
        max: P3::new(10.0, 10.0, 10.0),
    };
    TriDexelStock::from_bounds(&bbox, cs)
}

/// Initial top of every ray in [`fresh_stock`].
const FRESH_TOP_MM: f64 = 10.0;

/// Fractional sub-cell coverage a stamp applied at this cell, read back from
/// the blended ray top.
///
/// `ray_blend_above(ray, surface, f)` moves the top from `top₀` to
/// `top₀ − f·(top₀ − surface)`, so on stock still at its fresh height the
/// coverage is `(top₀ − top) / depth` exactly. An untouched cell reads 0; a
/// fully-covered cell reads 1.
///
/// This replaces the deleted `DexelGrid::coverage_at` sidecar — see the module
/// docstring. It observes the value through the channel the simulator uses,
/// which is strictly the stronger reading.
fn observed_coverage(grid: &DexelGrid, row: usize, col: usize, depth: f64) -> f64 {
    let top = ray_top(grid.ray(row, col)).map_or(f64::NEG_INFINITY, f64::from);
    ((FRESH_TOP_MM - top) / depth).clamp(0.0, 1.0)
}

/// Sum of material removed across all cells of the Z-grid, given a
/// pre-stamp stock and a post-stamp stock with identical bounds.
fn total_volume_removed(pre: &TriDexelStock, post: &TriDexelStock) -> f64 {
    let pg = &pre.z_grid;
    let qg = &post.z_grid;
    assert_eq!(pg.rays.len(), qg.rays.len());
    let cell_area = pg.cell_size * pg.cell_size;
    let mut removed = 0.0_f64;
    for (a, b) in pg.rays.iter().zip(qg.rays.iter()) {
        let pre_len = ray_material_length(a) as f64;
        let post_len = ray_material_length(b) as f64;
        removed += (pre_len - post_len).max(0.0) * cell_area;
    }
    removed
}

#[test]
fn point_stamp_total_volume_matches_disk_area() {
    // A single flat-endmill point stamp at known XY removes a cylindrical
    // plug of material. Total volume ≈ π·r² · depth_of_cut, to within the
    // sub-cell quantisation error (≤ ~1 cell of perimeter × depth).
    let cutter = FlatEndmill::new(6.0, 25.0);
    let radius = cutter.radius();
    let cs = 0.25;
    let stock = fresh_stock(cs);
    let stock_top: f64 = 10.0;
    let cut_z: f64 = 8.0;
    let depth = stock_top - cut_z;

    let lut = RadialProfileLUT::from_cutter(&cutter, rs_cam_core::radial_profile::LUT_SAMPLES);
    let mut after = stock.clone();
    after.stamp_tool_at(&lut, radius, 0.0, 0.0, cut_z, StockCutDirection::FromTop);

    let removed = total_volume_removed(&stock, &after);
    let analytical = std::f64::consts::PI * radius * radius * depth;
    let err = (removed - analytical).abs() / analytical;
    eprintln!(
        "point stamp: removed={removed:.3}mm³, analytical={analytical:.3}mm³, err={:.2}%",
        err * 100.0
    );
    // Sub-cell stamping with 4×4 sub-samples is accurate to ~1 % at this
    // radius / cs ratio (24 cells across the disk).
    assert!(
        err < 0.03,
        "point stamp removed {removed:.3} mm³ vs analytical {analytical:.3} mm³ (err {err:.3})"
    );
}

#[test]
fn segment_stamp_total_volume_matches_stadium_area() {
    // A swept-segment stamp of length L removes (π·r² + 2·r·L)·depth.
    let cutter = FlatEndmill::new(6.0, 25.0);
    let radius = cutter.radius();
    let cs = 0.25;
    let stock = fresh_stock(cs);
    let stock_top: f64 = 10.0;
    let cut_z: f64 = 8.0;
    let depth = stock_top - cut_z;
    let length: f64 = 8.0;
    let lut = RadialProfileLUT::from_cutter(&cutter, rs_cam_core::radial_profile::LUT_SAMPLES);

    let mut after = stock.clone();
    after.stamp_linear_segment(
        &lut,
        radius,
        P3::new(-length / 2.0, 0.0, cut_z),
        P3::new(length / 2.0, 0.0, cut_z),
        StockCutDirection::FromTop,
    );

    let removed = total_volume_removed(&stock, &after);
    let stadium_area = std::f64::consts::PI * radius * radius + 2.0 * radius * length;
    let analytical = stadium_area * depth;
    let err = (removed - analytical).abs() / analytical;
    eprintln!(
        "segment stamp: removed={removed:.3}mm³, analytical={analytical:.3}mm³, err={:.2}%",
        err * 100.0
    );
    assert!(
        err < 0.03,
        "segment stamp removed {removed:.3} mm³ vs analytical {analytical:.3} mm³ (err {err:.3})"
    );
}

#[test]
fn point_stamp_coverage_reaches_one_at_disk_interior_and_partial_at_boundary() {
    // F.a gap 4: sub-cell coverage classifies each cell against the disk.
    // Interior cells of a single point stamp reach 1.0; cells straddling the
    // edge land strictly inside (0, 1); cells past the scan radius stay at 0.
    // Read through the blended ray top — see `observed_coverage`.
    let cutter = FlatEndmill::new(6.0, 25.0);
    let radius = cutter.radius();
    let cs = 0.25;
    let mut stock = fresh_stock(cs);
    let cut_z = 8.0_f64;
    let depth = FRESH_TOP_MM - cut_z;
    let lut = RadialProfileLUT::from_cutter(&cutter, rs_cam_core::radial_profile::LUT_SAMPLES);
    stock.stamp_tool_at(&lut, radius, 0.0, 0.0, cut_z, StockCutDirection::FromTop);

    // Interior cell (well inside disk): coverage = 1.0.
    let grid: &DexelGrid = &stock.z_grid;
    let (r_in, c_in) = grid.world_to_cell(0.0, 0.0).unwrap();
    let cov_in = observed_coverage(grid, r_in, c_in, depth);
    assert!(
        cov_in >= 0.999,
        "interior cell coverage={cov_in}, expected 1.0"
    );

    // Boundary cell (just inside the disk edge): coverage > 0.
    let (r_bnd, c_bnd) = grid.world_to_cell(0.0, radius - cs * 0.1).unwrap();
    let cov_bnd = observed_coverage(grid, r_bnd, c_bnd, depth);
    eprintln!("boundary cell coverage={cov_bnd:.3}");
    assert!(cov_bnd > 0.0, "boundary cell coverage should be > 0, got {cov_bnd}");

    // Boundary-straddling cell (center near disk edge, some sub-samples
    // inside): coverage ∈ (0, 1).
    let (r_ann, c_ann) = grid.world_to_cell(0.0, radius).unwrap();
    let cov_ann = observed_coverage(grid, r_ann, c_ann, depth);
    eprintln!("boundary-straddling cell coverage={cov_ann:.3}");
    assert!(
        cov_ann > 0.0 && cov_ann < 1.0,
        "boundary-straddling cell coverage should be in (0,1), got {cov_ann}"
    );

    // The 4×4 sub-sample fan quantises a partial cell's coverage to k/16.
    // The sidecar reading this test used to make never checked that; the
    // blended top does, because the blend is exactly linear in coverage.
    let sixteenths = cov_ann * 16.0;
    assert!(
        (sixteenths - sixteenths.round()).abs() < 1e-3,
        "partial coverage {cov_ann} is not a multiple of 1/16 — the 4×4 \
         sub-sample fan (COVERAGE_SUBSAMPLES_PER_AXIS) is not what ran"
    );

    // Well-outside cell: coverage = 0. Use cs·4 to be safely past the
    // r + cs·√2 scan radius.
    let (r_out, c_out) = grid.world_to_cell(0.0, radius + cs * 4.0).unwrap();
    let cov_out = observed_coverage(grid, r_out, c_out, depth);
    assert!(cov_out < 1e-6, "outside cell coverage should be 0, got {cov_out}");
}

#[test]
fn point_stamp_blend_lowers_ray_top_proportionally_at_boundary() {
    // F.a: a single stamp at a boundary cell lowers its ray top by
    // `coverage × (top₀ − surface)` (multiplicative blend semantic). The
    // boundary cell must land strictly between "untouched" and "fully cut" —
    // a binary cell-center kernel would put it at one end or the other.
    let cutter = FlatEndmill::new(6.0, 25.0);
    let radius = cutter.radius();
    let cs = 0.25;
    let mut stock = fresh_stock(cs);
    let cut_z = 8.0_f64;
    let depth = FRESH_TOP_MM - cut_z;
    let lut = RadialProfileLUT::from_cutter(&cutter, rs_cam_core::radial_profile::LUT_SAMPLES);
    stock.stamp_tool_at(&lut, radius, 0.0, 0.0, cut_z, StockCutDirection::FromTop);

    let grid = &stock.z_grid;
    // Cell straddling the disk edge — cov ∈ (0, 1).
    let (r, c) = grid.world_to_cell(0.0, radius).unwrap();
    let top = ray_top(grid.ray(r, c)).expect("ray has material") as f64;
    eprintln!("annular cell: top={top:.4} (fresh {FRESH_TOP_MM}, surface {cut_z})");
    assert!(
        top < FRESH_TOP_MM - 0.05 * depth,
        "boundary cell top {top} is effectively untouched — the blend did not apply"
    );
    assert!(
        top > cut_z + 0.05 * depth,
        "boundary cell top {top} was cut to the full surface {cut_z} — that is \
         the binary cell-center behaviour F.a replaced"
    );
}

#[test]
fn coverage_increases_monotonically_with_stamps() {
    // Repeated overlapping point stamps must never *restore* material: a
    // second stamp can only lower a ray top further, so observed coverage is
    // non-decreasing. (Blending f₁ then f₂ removes 1 − (1−f₁)(1−f₂), which is
    // ≥ both — the running max the deleted sidecar tracked was a weaker
    // statement about the same thing.)
    let cutter = FlatEndmill::new(6.0, 25.0);
    let radius = cutter.radius();
    let cs = 0.25;
    let mut stock = fresh_stock(cs);
    let cut_z = 8.0_f64;
    let depth = FRESH_TOP_MM - cut_z;
    let lut = RadialProfileLUT::from_cutter(&cutter, rs_cam_core::radial_profile::LUT_SAMPLES);

    // First stamp at origin.
    stock.stamp_tool_at(&lut, radius, 0.0, 0.0, cut_z, StockCutDirection::FromTop);
    let (r, c) = stock.z_grid.world_to_cell(0.0, radius + cs * 0.5).unwrap();
    let cov1 = observed_coverage(&stock.z_grid, r, c, depth);

    // Stamp a second cutter shifted laterally so the previous boundary cell
    // is now closer to the new disk center → coverage should not decrease.
    stock.stamp_tool_at(&lut, radius, 0.0, cs * 0.5, cut_z, StockCutDirection::FromTop);
    let cov2 = observed_coverage(&stock.z_grid, r, c, depth);
    eprintln!("monotonicity: cov1={cov1:.3}, cov2={cov2:.3}");
    assert!(
        cov2 >= cov1,
        "coverage regressed after a second stamp: {cov1:.3} → {cov2:.3}"
    );
}

#[test]
fn extended_scan_radius_catches_annular_cells_old_code_missed() {
    // Under the prior binary kernel, scan_radius = r + cs and the cell-
    // center test left a ring of "annular" cells (centers in
    // (r, r + cs·√2/2)) entirely untouched. With F.a's extended scan
    // radius (r + cs·√2) and sub-cell coverage these are now stamped
    // (with coverage < 1) and contribute to `coverage_max`.
    let cutter = FlatEndmill::new(2.0, 25.0); // small tool, ~4-cell-radius
    let radius = cutter.radius();
    let cs = 0.5;
    let mut stock = fresh_stock(cs);
    let lut = RadialProfileLUT::from_cutter(&cutter, rs_cam_core::radial_profile::LUT_SAMPLES);
    stock.stamp_tool_at(&lut, radius, 0.0, 0.0, 8.0, StockCutDirection::FromTop);

    // A cell whose center sits just outside the disk (annular ring) — pre-
    // F.a binary code skipped these because dist_sq > radius_sq at the
    // cell center.
    let annular_offset = radius + cs * 0.3;
    let (r, c) = stock.z_grid.world_to_cell(0.0, annular_offset).unwrap();
    let top = ray_top(stock.z_grid.ray(r, c)).unwrap_or(10.0);
    eprintln!("annular(r+0.3·cs) top={top}");
    assert!(
        (top as f64) < 10.0 - 1e-6,
        "F.a annular cell should be blended below initial top 10.0; got {top} \
         (pre-F.a this cell was skipped entirely)"
    );
}
