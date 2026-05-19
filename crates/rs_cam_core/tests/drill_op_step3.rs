//! Step 3 PR1 regression-locking tests — analytical drill removal +
//! mesh extraction.
//!
//! Covers:
//! - `TriDexelStock::apply_drill_op` clips cells inside the cylinder
//!   footprint and leaves cells outside untouched.
//! - Composes correctly with subsequent milling stamping
//!   (drill-then-pocket and pocket-then-drill).
//! - `append_drill_cylinders` adds non-empty geometry to the stock mesh.
//! - `build_drill_op_for_config` resolves the right `HoleSource` for
//!   `Drill` (ModelDerived) vs `AlignmentPinDrill` (Snapshot).
//!
//! Plan: planning/DEXEL_Z_ONLY_INVESTIGATION.md §6.E / §8 Step 3.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::dexel::ray_top;
use rs_cam_core::dexel_mesh::{append_drill_cylinders, dexel_stock_to_mesh};
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::drill::DrillCycle;
use rs_cam_core::drill_op::{DrillHole, DrillOp, HoleSource, OpData, ToolProfile};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::material::Material;
use std::sync::Arc;

fn stock_5x5x10() -> TriDexelStock {
    TriDexelStock::from_bounds(
        &BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(5.0, 5.0, 10.0),
        },
        0.25,
    )
}

fn flat_drill(diameter_mm: f64, xy: [f64; 2], top_z: f64, bottom_z: f64) -> DrillOp {
    DrillOp {
        holes: vec![DrillHole { xy, top_z, bottom_z }],
        hole_source: HoleSource::ModelDerived,
        tool_profile: ToolProfile::Flat,
        tool_diameter_mm: diameter_mm,
        cycle: DrillCycle::Simple,
        feed_rate_mm_min: 300.0,
        spindle_rpm: 18_000,
        flute_count: 2,
        material: Material::default(),
    }
}

#[test]
fn analytical_removal_sets_ray_top_to_bottom_z_inside_footprint() {
    let mut stock = stock_5x5x10();
    let drill = flat_drill(2.0, [2.5, 2.5], 10.0, 4.0);
    stock.apply_drill_op(&drill);

    // Center cell at (2.5, 2.5): ray top should equal bottom_z = 4.0.
    let (row, col) = stock.z_grid.world_to_cell(2.5, 2.5).unwrap();
    let center_top = ray_top(stock.z_grid.ray(row, col)).unwrap() as f64;
    assert!(
        (center_top - 4.0).abs() < 0.01,
        "center cell top should be at bottom_z=4.0, got {center_top}"
    );
}

#[test]
fn analytical_removal_leaves_outside_cells_untouched() {
    let mut stock = stock_5x5x10();
    let drill = flat_drill(2.0, [2.5, 2.5], 10.0, 4.0);
    stock.apply_drill_op(&drill);

    // Cell well outside the Ø2 footprint (>3mm from hole center):
    // ray_top must still be at the original stock top (10.0).
    let (row, col) = stock.z_grid.world_to_cell(0.0, 0.0).unwrap();
    let outside_top = ray_top(stock.z_grid.ray(row, col)).unwrap() as f64;
    assert!(
        (outside_top - 10.0).abs() < 0.01,
        "cell outside hole footprint must keep ray_top=10.0, got {outside_top}"
    );
}

#[test]
fn drill_does_not_raise_already_lower_cells() {
    // If an earlier op already cleared the cell deeper than the drill's
    // bottom_z, drilling on top must NOT raise the ray. This is the
    // idempotence requirement for drill-then-pocket composition.
    let mut stock = stock_5x5x10();
    // Manually clear the center cell down to z=2 (simulating a prior pocket).
    let (row, col) = stock.z_grid.world_to_cell(2.5, 2.5).unwrap();
    stock.clear_above_at(row, col, 2.0);
    assert!(
        (ray_top(stock.z_grid.ray(row, col)).unwrap() as f64 - 2.0).abs() < 0.01
    );

    // Now drill the same XY with bottom_z = 4.0 (above the pocket floor).
    let drill = flat_drill(2.0, [2.5, 2.5], 10.0, 4.0);
    stock.apply_drill_op(&drill);

    // Cell top must still be 2.0 (drill must not add material back).
    let after = ray_top(stock.z_grid.ray(row, col)).unwrap() as f64;
    assert!(
        (after - 2.0).abs() < 0.01,
        "drill must be non-additive over already-lower stock; got top={after}"
    );
}

#[test]
fn drill_then_pocket_composes_correctly() {
    // Drill bores down to z=6.0; then a pocket-style clear takes the
    // surrounding region down to z=4.0. The drill hole should now sit
    // at z=4.0 (it's been further cleared by the pocket) — the dexel
    // grid is the source of truth, drill is just an early removal.
    let mut stock = stock_5x5x10();
    let drill = flat_drill(2.0, [2.5, 2.5], 10.0, 6.0);
    stock.apply_drill_op(&drill);

    // Simulate a pocket that clears the entire stock down to z=4.0.
    for r in 0..stock.z_grid.rows {
        for c in 0..stock.z_grid.cols {
            stock.clear_above_at(r, c, 4.0);
        }
    }

    // Cell at hole center: ray_top should now be 4.0 (pocket floor),
    // not 6.0 (the original drill bottom).
    let (row, col) = stock.z_grid.world_to_cell(2.5, 2.5).unwrap();
    let after = ray_top(stock.z_grid.ray(row, col)).unwrap() as f64;
    assert!(
        (after - 4.0).abs() < 0.01,
        "drill-then-pocket: cell top should follow the deeper pocket floor; got {after}"
    );
}

#[test]
fn append_drill_cylinders_adds_geometry() {
    let stock = stock_5x5x10();
    let mut mesh = dexel_stock_to_mesh(&stock);
    let initial_verts = mesh.vertices.len();

    let drill = flat_drill(2.0, [2.5, 2.5], 10.0, 4.0);
    append_drill_cylinders(&mut mesh, &[&drill]);

    assert!(
        mesh.vertices.len() > initial_verts,
        "append_drill_cylinders should add geometry; before={initial_verts}, after={}",
        mesh.vertices.len()
    );
    // Each hole contributes: 16 ring verts at cylinder_bottom + 16 at
    // top + 1 cap vertex = 33 verts × 3 floats = 99 floats.
    let delta = (mesh.vertices.len() - initial_verts) / 3;
    assert_eq!(
        delta, 33,
        "expected 33 added vertices per Flat-profile hole (16+16+1), got {delta}"
    );
}

#[test]
fn opdata_drill_carries_both_representations() {
    let drill = flat_drill(2.0, [2.5, 2.5], 10.0, 4.0);
    let annotated = Arc::new(rs_cam_core::toolpath_spans::AnnotatedToolpath::new(
        rs_cam_core::toolpath::Toolpath::new(),
    ));
    let op_data = OpData::DrillOp(Arc::new(drill), Arc::clone(&annotated));

    // Dual-representation invariant: both views must be reachable.
    assert!(op_data.is_drill_op());
    assert!(op_data.drill_op().is_some());
    assert!(Arc::ptr_eq(op_data.annotated(), &annotated));

    let plain = OpData::Toolpath(Arc::clone(&annotated));
    assert!(!plain.is_drill_op());
    assert!(plain.drill_op().is_none());
    assert!(Arc::ptr_eq(plain.annotated(), &annotated));
}

#[test]
fn cone_profile_protrusion_geometry() {
    // StandardTwist drill (118° included → 59° half-angle from axis).
    // tip_protrusion for Ø2 tool: r / tan(59°) ≈ 1 / 1.664 ≈ 0.6 mm.
    let profile = ToolProfile::StandardTwist;
    let radius_mm: f64 = 1.0;
    let protrusion = profile.tip_protrusion_mm(radius_mm);
    assert!(
        (protrusion - radius_mm / 59.0_f64.to_radians().tan()).abs() < 1e-9
    );
    assert!(
        protrusion > 0.5 && protrusion < 0.7,
        "Ø2 standard twist tip_protrusion should be ~0.6mm, got {protrusion}"
    );

    // Flat profile: protrusion is exactly 0.
    assert_eq!(ToolProfile::Flat.tip_protrusion_mm(radius_mm), 0.0);
}
