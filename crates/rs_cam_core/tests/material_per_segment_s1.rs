//! S1 (`planning/stock_additions_2026-10-09/PLAN.md`): a material slot on
//! each dexel segment.
//!
//! 1. A cut sample records the slot that the tool removed, in each metric
//!    stamp dispatch (per stamp, whole toolpath, swept, swept plunge only),
//!    and sets the flag where the tool removes two materials.
//! 2. The carry keeps the slots through a Top/Bottom flip.
//! 3. Memory: a plain cell costs the same bytes as before S1; only a cell
//!    with more than one segment pays, on the heap.
//!
//! The ray primitives (union, subtract, merge) have their unit tests in
//! `src/stock/dexel.rs`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::mem::size_of;

use rs_cam_core::budget::estimate::dexel_cell_bytes;
use rs_cam_core::compute::stock_carry::carry_stock_into_group;
use rs_cam_core::compute::transform::{FaceUp, SetupTransformInfo, ZRotation};
use rs_cam_core::dexel_stock::{StampDispatch, StockCutDirection, TriDexelStock};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::material::Material;
use rs_cam_core::stock::dexel::{
    DexelRay, DexelSegment, MaterialSlot, ray_subtract_above, ray_union_interval,
};
use rs_cam_core::stock::simulation_cut::{CutKinematics, SimulationCutSample};
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::Toolpath;

/// A user material from the library. The name is a label only.
fn fill_material() -> Material {
    Material::Custom {
        name: "fill".to_owned(),
        feed_scale_factor: 1.0,
    }
}

const W: f64 = 50.0;
const D: f64 = 50.0;
const H: f64 = 10.0;
/// The added material fills `FILL_X0..FILL_X1` (all Y) from `FILL_FLOOR` up
/// to the stock top.
const FILL_X0: f64 = 20.0;
const FILL_X1: f64 = 30.0;
const FILL_FLOOR: f64 = 6.0;

/// A block with a channel cut to `FILL_FLOOR` and filled again with the
/// added material up to the stock top. The top stays flat at `H`.
fn filled_block(cell: f64) -> (TriDexelStock, MaterialSlot) {
    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(W, D, H),
    };
    let mut stock = TriDexelStock::from_bounds(&bbox, cell);
    let slot = stock.materials.slot_for(&fill_material()).unwrap();
    let grid = &mut stock.z_grid;
    for row in 0..grid.rows {
        for col in 0..grid.cols {
            let (x, _) = grid.cell_to_world(row, col);
            if (FILL_X0..=FILL_X1).contains(&x) {
                let idx = row * grid.cols + col;
                ray_subtract_above(&mut grid.rays[idx], FILL_FLOOR as f32);
                grid.union_interval_at(idx, FILL_FLOOR as f32, H as f32, slot);
            }
        }
    }
    (stock, slot)
}

/// A level pass across the fill at z = 8, then a plunge into the fill.
fn pass_and_plunge() -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(5.0, 25.0, 12.0));
    tp.rapid_to(P3::new(5.0, 25.0, 8.0));
    tp.feed_to(P3::new(45.0, 25.0, 8.0), 600.0);
    tp.rapid_to(P3::new(45.0, 25.0, 12.0));
    tp.rapid_to(P3::new(25.0, 10.0, 12.0));
    tp.feed_to(P3::new(25.0, 10.0, 7.0), 300.0);
    tp
}

fn simulate(stock: &mut TriDexelStock) -> Vec<SimulationCutSample> {
    let cutter = FlatEndmill::new(6.0, 25.0);
    let never_cancel = || false;
    stock
        .simulate_toolpath_with_metrics_with_cancel(
            &pass_and_plunge(),
            &cutter,
            StockCutDirection::FromTop,
            ToolpathId(0),
            12_000,
            2,
            3000.0,
            0.5,
            None,
            &[],
            &[],
            true,
            &never_cancel,
        )
        .expect("simulation succeeds")
}

// ── 1. The cut sample records the slot ──────────────────────────────────

#[test]
fn a_cut_sample_records_the_slot_it_removed_in_every_stamp_dispatch() {
    for dispatch in [
        StampDispatch::PerStamp,
        StampDispatch::WholeToolpath,
        StampDispatch::Swept,
        StampDispatch::SweptPlungeOnly,
    ] {
        let (mut stock, slot) = filled_block(0.25);
        stock.stamp_dispatch = dispatch;
        let samples = simulate(&mut stock);
        let removing = |s: &&SimulationCutSample| s.is_cutting && s.removed_volume_est_mm3 > 1e-6;

        // The tool (R = 3) is wholly over the fill for 23 < x < 27, and
        // wholly over the stock for x < 17 or x > 33. Allow one sample step
        // of lag for the swept dispatch.
        let lateral: Vec<&SimulationCutSample> = samples
            .iter()
            .filter(removing)
            .filter(|s| s.cut_kinematics != CutKinematics::Plunge && s.move_index == 2)
            .collect();
        let inside: Vec<_> = lateral
            .iter()
            .filter(|s| (24.0..26.0).contains(&s.position[0]))
            .collect();
        let outside: Vec<_> = lateral
            .iter()
            .filter(|s| s.position[0] < 15.0 || s.position[0] > 35.0)
            .collect();
        let edge: Vec<_> = lateral
            .iter()
            .filter(|s| (19.0..21.0).contains(&s.position[0]))
            .collect();
        assert!(
            !inside.is_empty() && !outside.is_empty() && !edge.is_empty(),
            "{dispatch:?}: the pass must have samples in each zone"
        );
        for s in &inside {
            assert_eq!(
                s.material_slot, slot,
                "{dispatch:?}: inside x={}",
                s.position[0]
            );
            assert!(
                !s.cuts_several_materials,
                "{dispatch:?}: inside x={}",
                s.position[0]
            );
        }
        for s in &outside {
            assert_eq!(
                s.material_slot,
                MaterialSlot::STOCK,
                "{dispatch:?}: outside"
            );
            assert!(!s.cuts_several_materials, "{dispatch:?}: outside");
        }
        assert!(
            edge.iter().any(|s| s.cuts_several_materials),
            "{dispatch:?}: a sample over the material edge removes both materials"
        );

        // The plunge into the fill: the degenerate (pure vertical) kernel.
        let plunge: Vec<_> = samples
            .iter()
            .filter(removing)
            .filter(|s| s.cut_kinematics == CutKinematics::Plunge)
            .collect();
        assert!(
            !plunge.is_empty(),
            "{dispatch:?}: the plunge removes material"
        );
        for s in &plunge {
            assert_eq!(s.material_slot, slot, "{dispatch:?}: plunge");
            assert!(!s.cuts_several_materials, "{dispatch:?}: plunge");
        }
    }
}

#[test]
fn a_one_material_stock_gives_the_stock_slot_and_the_same_samples() {
    // The same geometry, refilled with the STOCK slot: the grid flag stays
    // off and every sample is as on a plain block that was never filled.
    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(W, D, H),
    };
    let mut plain = TriDexelStock::from_bounds(&bbox, 0.25);
    let mut refilled = TriDexelStock::from_bounds(&bbox, 0.25);
    let grid = &mut refilled.z_grid;
    for idx in 0..grid.rays.len() {
        ray_subtract_above(&mut grid.rays[idx], FILL_FLOOR as f32);
        grid.union_interval_at(idx, FILL_FLOOR as f32, H as f32, MaterialSlot::STOCK);
    }
    assert!(!refilled.z_grid.has_added_material);
    assert_eq!(
        refilled.z_grid.rays, plain.z_grid.rays,
        "a stock union merges back"
    );
    let a = simulate(&mut plain);
    let b = simulate(&mut refilled);
    assert_eq!(a, b);
    assert!(
        a.iter()
            .all(|s| s.material_slot.is_stock() && !s.cuts_several_materials)
    );
}

// ── 2. The carry keeps the slots through a flip ─────────────────────────

#[test]
fn the_carry_keeps_the_slots_through_a_top_bottom_flip() {
    let (w, d, h) = (10.0, 10.0, 5.0);
    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(w, d, h),
    };
    let mut src = TriDexelStock::from_bounds(&bbox, 1.0);
    let slot = src.materials.slot_for(&fill_material()).unwrap();
    // One cell at (x, y) = (3, 2): stock 0..3, the added material 3..5.
    let (row, col) = src.z_grid.world_to_cell(3.0, 2.0).unwrap();
    let idx = row * src.z_grid.cols + col;
    ray_subtract_above(&mut src.z_grid.rays[idx], 3.0);
    src.z_grid.union_interval_at(idx, 3.0, 5.0, slot);

    let bottom = SetupTransformInfo {
        face_up: FaceUp::Bottom,
        z_rotation: ZRotation::Deg0,
        stock_x: w,
        stock_y: d,
        stock_z: h,
        stock_origin_x: 0.0,
        stock_origin_y: 0.0,
        stock_origin_z: 0.0,
    };
    let (dst, map) = carry_stock_into_group(&src, &None, &bbox, &Some(bottom), P3::origin(), 1.0);
    assert!(map.z_flipped);
    assert!(dst.z_grid.has_added_material);
    assert_eq!(dst.materials, src.materials);

    // Bottom: local (x, D - y, H - z). Z reverses, so the added material is
    // now at the bottom of the ray: [0, 2] in slot 1, [2, 5] in the stock.
    let (r, c) = dst.z_grid.world_to_cell(3.0, d - 2.0).unwrap();
    assert_eq!(
        dst.z_grid.ray(r, c).as_slice(),
        &[
            DexelSegment::new(0.0, 2.0, slot),
            DexelSegment::new(2.0, 5.0, MaterialSlot::STOCK),
        ]
    );
    assert_eq!(dst.z_grid.top_material_at(r, c), Some(MaterialSlot::STOCK));
    // No other ray holds the added material.
    let holders = dst
        .z_grid
        .rays
        .iter()
        .filter(|ray| ray.iter().any(|s| s.material == slot))
        .count();
    assert_eq!(holders, 1);

    // A clone (a checkpoint, a snapshot) keeps the slots and the table.
    let snap = dst.checkpoint();
    assert_eq!(snap.z_grid.rays, dst.z_grid.rays);
    assert_eq!(snap.materials, dst.materials);
    assert!(snap.z_grid.has_added_material);
}

// ── 3. Memory ───────────────────────────────────────────────────────────

/// Bytes a ray holds on the heap: zero while its one segment is inline.
fn heap_bytes(ray: &DexelRay) -> usize {
    if ray.spilled() {
        ray.capacity() * size_of::<DexelSegment>()
    } else {
        0
    }
}

#[test]
fn a_plain_cell_costs_the_same_bytes_and_only_a_filled_cell_pays_on_the_heap() {
    // Before S1: segment 8 bytes, ray 24, cell 28 (ray + conservative_top),
    // sample 280. After S1 the segment gains a one-byte slot (12 bytes with
    // align 4); the inline array of one segment (12) still fits under the
    // 16-byte heap pair of SmallVec, so the ray and the cell do not grow.
    assert_eq!(size_of::<DexelSegment>(), 12);
    assert_eq!(size_of::<DexelRay>(), 24);
    assert_eq!(dexel_cell_bytes(), 28);
    // The two sample fields (a slot byte and a flag) fit in the padding.
    assert_eq!(size_of::<SimulationCutSample>(), 280);

    let (stock, slot) = filled_block(1.0);
    let grid = &stock.z_grid;
    let filled_cells = grid
        .rays
        .iter()
        .filter(|ray| ray.iter().any(|s| s.material == slot))
        .count();
    let spilled = grid.rays.iter().filter(|ray| ray.spilled()).count();
    let heap: usize = grid.rays.iter().map(heap_bytes).sum();
    eprintln!(
        "S1 memory: {} cells, {filled_cells} filled, {spilled} spilled, \
         {heap} heap bytes ({} per filled cell), {} bytes per plain cell",
        grid.rays.len(),
        heap / filled_cells.max(1),
        dexel_cell_bytes()
    );
    assert!(filled_cells > 0);
    assert_eq!(
        spilled, filled_cells,
        "only a filled cell leaves the inline slot"
    );
    // A filled cell holds two segments: the stock and the added material.
    assert_eq!(heap, filled_cells * 2 * size_of::<DexelSegment>());

    // A union into a gap of one inline ray: two segments of different
    // slots never merge, so the ray spills; one of the same slot merges.
    let mut ray: DexelRay = [DexelSegment::new(0.0, 1.0, MaterialSlot::STOCK)]
        .into_iter()
        .collect();
    ray_union_interval(&mut ray, 1.0, 2.0, MaterialSlot::STOCK);
    assert!(!ray.spilled());
    ray_union_interval(&mut ray, 2.0, 3.0, slot);
    assert!(ray.spilled());
}
