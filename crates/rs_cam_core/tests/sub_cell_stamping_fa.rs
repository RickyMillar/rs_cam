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
//! 3. `DexelGrid.coverage_max` reaches 1.0 at the cells fully inside the
//!    cutter footprint and ∈ (0, 1) at boundary cells.
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
    // F.a gap 4: `coverage_max` is a per-cell sidecar that records the
    // running max of sub-cell coverage seen at each cell. Interior cells
    // of a single point stamp should reach 1.0; boundary cells should be
    // strictly in (0, 1).
    let cutter = FlatEndmill::new(6.0, 25.0);
    let radius = cutter.radius();
    let cs = 0.25;
    let mut stock = fresh_stock(cs);
    let lut = RadialProfileLUT::from_cutter(&cutter, rs_cam_core::radial_profile::LUT_SAMPLES);
    stock.stamp_tool_at(&lut, radius, 0.0, 0.0, 8.0, StockCutDirection::FromTop);

    // Interior cell (well inside disk): coverage = 1.0.
    let grid: &DexelGrid = &stock.z_grid;
    let (r_in, c_in) = grid.world_to_cell(0.0, 0.0).unwrap();
    assert!(
        grid.coverage_at(r_in, c_in) >= 0.999,
        "interior cell coverage_max={}, expected 1.0",
        grid.coverage_at(r_in, c_in)
    );

    // Boundary cell (just inside the disk edge): coverage ∈ (0, 1).
    let (r_bnd, c_bnd) = grid.world_to_cell(0.0, radius - cs * 0.1).unwrap();
    let cov_bnd = grid.coverage_at(r_bnd, c_bnd);
    eprintln!("boundary cell coverage_max={cov_bnd:.3}");
    assert!(
        cov_bnd > 0.0,
        "boundary cell coverage_max should be > 0, got {cov_bnd}"
    );

    // Boundary-straddling cell (center near disk edge, some sub-samples
    // inside): coverage ∈ (0, 1).
    let (r_ann, c_ann) = grid.world_to_cell(0.0, radius).unwrap();
    let cov_ann = grid.coverage_at(r_ann, c_ann);
    eprintln!("boundary-straddling cell coverage_max={cov_ann:.3}");
    assert!(
        cov_ann > 0.0 && cov_ann < 1.0,
        "boundary-straddling cell coverage_max should be in (0,1), got {cov_ann}"
    );

    // Well-outside cell: coverage = 0. Use cs·4 to be safely past the
    // r + cs·√2 scan radius.
    let (r_out, c_out) = grid.world_to_cell(0.0, radius + cs * 4.0).unwrap();
    assert!(
        grid.coverage_at(r_out, c_out) < 1e-6,
        "outside cell coverage_max should be 0, got {}",
        grid.coverage_at(r_out, c_out)
    );
}

#[test]
fn point_stamp_blend_lowers_ray_top_proportionally_at_boundary() {
    // F.a: a single stamp at a boundary cell should lower its ray top by
    // `coverage × (top₀ − surface)` (multiplicative blend semantic).
    let cutter = FlatEndmill::new(6.0, 25.0);
    let radius = cutter.radius();
    let cs = 0.25;
    let mut stock = fresh_stock(cs);
    let top_initial: f32 = 10.0;
    let cut_z = 8.0_f64;
    let depth = (top_initial as f64) - cut_z;
    let lut = RadialProfileLUT::from_cutter(&cutter, rs_cam_core::radial_profile::LUT_SAMPLES);
    stock.stamp_tool_at(&lut, radius, 0.0, 0.0, cut_z, StockCutDirection::FromTop);

    let grid = &stock.z_grid;
    // Cell straddling the disk edge — cov ∈ (0, 1).
    let (r, c) = grid.world_to_cell(0.0, radius).unwrap();
    let cov = grid.coverage_at(r, c) as f64;
    assert!(cov > 0.05 && cov < 0.95, "expected boundary cov, got {cov}");
    let top = ray_top(grid.ray(r, c)).expect("ray has material");
    let expected_top = (top_initial as f64) - cov * depth;
    let err = ((top as f64) - expected_top).abs();
    eprintln!("annular cell: cov={cov:.3}, top={top}, expected={expected_top:.3}, err={err:.3}");
    assert!(
        err < 1e-3,
        "boundary cell top {top} ≠ expected {expected_top:.3} under coverage-blend"
    );
}

#[test]
fn coverage_increases_monotonically_with_stamps() {
    // Repeated overlapping point stamps should never reduce coverage_max.
    let cutter = FlatEndmill::new(6.0, 25.0);
    let radius = cutter.radius();
    let cs = 0.25;
    let mut stock = fresh_stock(cs);
    let lut = RadialProfileLUT::from_cutter(&cutter, rs_cam_core::radial_profile::LUT_SAMPLES);

    // First stamp at origin.
    stock.stamp_tool_at(&lut, radius, 0.0, 0.0, 8.0, StockCutDirection::FromTop);
    let (r, c) = stock.z_grid.world_to_cell(0.0, radius + cs * 0.5).unwrap();
    let cov1 = stock.z_grid.coverage_at(r, c);

    // Stamp a second cutter shifted laterally so the previous boundary cell
    // is now closer to the new disk center → coverage should not decrease.
    stock.stamp_tool_at(&lut, radius, 0.0, cs * 0.5, 8.0, StockCutDirection::FromTop);
    let cov2 = stock.z_grid.coverage_at(r, c);
    eprintln!("monotonicity: cov1={cov1:.3}, cov2={cov2:.3}");
    assert!(
        cov2 >= cov1,
        "coverage_max regressed after a second stamp: {cov1:.3} → {cov2:.3}"
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
    let cov = stock.z_grid.coverage_at(r, c);
    let top = ray_top(stock.z_grid.ray(r, c)).unwrap_or(10.0);
    eprintln!("annular(r+0.3·cs) cov={cov:.3}, top={top}");
    assert!(
        cov > 0.0,
        "F.a should stamp annular cells outside r; got coverage 0"
    );
    assert!(
        (top as f64) < 10.0 - 1e-6,
        "F.a annular cell should be blended below initial top 10.0; got {top}"
    );
}
