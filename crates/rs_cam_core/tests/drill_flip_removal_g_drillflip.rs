//! G-DRILLFLIP — analytic drill removal must follow the tool's advance
//! direction in the frame it is being applied to.
//!
//! # How this was found
//!
//! Not by reading the code. The operator watched the viewport during a
//! two-sided job and said: *"I see the tool drill the pins but it's not
//! showing in the sim stock — the other holes do."* Both halves of that
//! sentence are the defect, and the half that looks fine is the dangerous
//! one.
//!
//! `TriDexelStock::apply_drill_op` removed everything **above** the tip
//! envelope, unconditionally — correct in setup-local coordinates, where the
//! tool always advances along −Z. But the global stock is fed holes that
//! `group_drill_op_to_global` has mapped through the setup transform, and a
//! `FaceUp::Bottom` setup maps `z → H − z`. So in a 25 mm blank:
//!
//! * a blind hole bottoming at local 13 arrives at global 12, and clearing
//!   above 12 carves 12..25 — a plausible-looking hole in the **wrong half**
//!   of the stock, which is the quiet failure, and
//! * an alignment-pin hole that breaks through (local `bottom_z` −1, i.e.
//!   1 mm into the spoilboard) arrives at global +26, above the blank, so the
//!   clear is a no-op and no hole appears — the visible failure.
//!
//! **Scope, corrected 2026-08-22.** This header first claimed the global
//! stock is what `StockSource::FromRemainingStock` reads, making the defect a
//! planning one. It is not. Rest generation reads `prior_stocks`, which are
//! clones of the per-setup **local** `group_stock`, and the local stock's
//! drill removal was always correct because setup-local Z is always the tool
//! axis. This is a checkpoint / playback / screenshot defect — what the
//! operator sees. Still worth fixing (it is how the operator caught it), and
//! still worth the sentries, but it never mis-planned a rest pass. The wrong
//! claim came from a stale comment in `dexel_stock/mod.rs` that has also been
//! corrected.
//!
//! # What is pinned here
//!
//! 1. A downward drill still removes the band above the tip (no regression).
//! 2. A flipped drill removes the band **below** the tip, and leaves the
//!    complement standing — asserted on both sides, because "material gone"
//!    and "material still there" fail in opposite directions and the old
//!    kernel would pass a one-sided version of this test.
//! 3. The break-through pin hole is fully removed rather than silently
//!    skipped, with `holes_carved` as the non-vacuity guard: a kernel that
//!    walked no cells at all would otherwise satisfy "no material left".
//! 4. The cone tip points the right way in both directions.
//! 5. A lateral setup abstains explicitly instead of carving a fabricated
//!    Z-axis hole (G-DRILLLATERAL).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::simulate::group_drill_op_to_global;
use rs_cam_core::compute::transform::{FaceUp, SetupTransformInfo, ZRotation};
use rs_cam_core::dexel::{ray_bottom, ray_material_length, ray_top};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::drill::DrillCycle;
use rs_cam_core::drill_op::{DrillHole, DrillOp, HoleSource, ToolProfile};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::material::Material;

const STOCK_X: f64 = 40.0;
const STOCK_Y: f64 = 30.0;
const STOCK_Z: f64 = 25.0;

fn blank() -> TriDexelStock {
    TriDexelStock::from_bounds(
        &BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(STOCK_X, STOCK_Y, STOCK_Z),
        },
        0.25,
    )
}

fn drill(
    profile: ToolProfile,
    diameter_mm: f64,
    xy: [f64; 2],
    top_z: f64,
    bottom_z: f64,
) -> DrillOp {
    DrillOp {
        holes: vec![DrillHole {
            xy,
            top_z,
            bottom_z,
        }],
        hole_source: HoleSource::ModelDerived,
        tool_profile: profile,
        tool_diameter_mm: diameter_mm,
        cycle: DrillCycle::Simple,
        feed_rate_mm_min: 300.0,
        spindle_rpm: 18_000,
        flute_count: 2,
        material: Material::default(),
        retract_z_mm: STOCK_Z + 5.0,
    }
}

/// A `FaceUp::Bottom` setup on this blank — the flip in the operator's
/// two-sided relief job.
fn bottom_flip() -> Option<SetupTransformInfo> {
    Some(SetupTransformInfo {
        face_up: FaceUp::Bottom,
        z_rotation: ZRotation::Deg0,
        stock_x: STOCK_X,
        stock_y: STOCK_Y,
        stock_z: STOCK_Z,
        ..Default::default()
    })
}

/// Material remaining on the ray through `xy`, restricted to `[lo, hi]`.
fn material_between(stock: &TriDexelStock, xy: [f64; 2], lo: f64, hi: f64) -> f64 {
    let (row, col) = stock.z_grid.world_to_cell(xy[0], xy[1]).unwrap();
    let ray = stock.z_grid.ray(row, col);
    let mut total = 0.0_f64;
    for seg in ray.iter() {
        let a = f64::from(seg.enter).max(lo);
        let b = f64::from(seg.exit).min(hi);
        if b > a {
            total += b - a;
        }
    }
    total
}

// ── 1. No regression on the ordinary downward case ──────────────────────

#[test]
fn downward_drill_still_clears_above_the_tip() {
    let mut stock = blank();
    let xy = [20.0, 15.0];
    let report = stock.apply_drill_op(
        &drill(ToolProfile::Flat, 6.0, xy, STOCK_Z, 13.0),
        StockCutDirection::FromTop,
    );

    assert_eq!(report.holes_carved, 1, "the hole centre is on the grid");
    assert!(!report.unrepresentable_axis);
    assert!(
        material_between(&stock, xy, 13.0, STOCK_Z) < 0.05,
        "13..25 is the hole and must be empty"
    );
    assert!(
        material_between(&stock, xy, 0.0, 13.0) > 12.9,
        "0..13 is below the tip and must be untouched"
    );
}

// ── 2. The flipped case, asserted on both sides ─────────────────────────

#[test]
fn flipped_blind_hole_clears_below_the_tip_not_above_it() {
    // Local: enters at the local top (25) and bottoms at 13.
    let local = drill(ToolProfile::Flat, 6.0, [20.0, 15.0], STOCK_Z, 13.0);
    let global = group_drill_op_to_global(&local, &bottom_flip(), P3::new(0.0, 0.0, 0.0));

    // The Bottom flip is z -> 25 - z, so the hole arrives inverted.
    let h = global.holes[0];
    assert!(
        (h.top_z - 0.0).abs() < 1e-9,
        "top_z maps to 0, got {}",
        h.top_z
    );
    assert!(
        (h.bottom_z - 12.0).abs() < 1e-9,
        "bottom_z maps to 12, got {}",
        h.bottom_z
    );
    assert!(
        h.top_z < h.bottom_z,
        "a flipped hole is entered from the low side — the ordering inverts, \
         and this is exactly the fact a min/max would destroy"
    );

    let mut stock = blank();
    let xy = h.xy;
    let report = stock.apply_drill_op(&global, StockCutDirection::FromBottom);
    assert_eq!(report.holes_carved, 1);

    assert!(
        material_between(&stock, xy, 0.0, 12.0) < 0.05,
        "0..12 is the hole in the global frame and must be empty, found {} mm",
        material_between(&stock, xy, 0.0, 12.0)
    );
    // The old kernel removed exactly this band instead. Asserting it is still
    // standing is what tells a flipped hole apart from a mirrored one.
    assert!(
        material_between(&stock, xy, 12.0, STOCK_Z) > 12.9,
        "12..25 is solid stock and must survive — removing it is the \
         pre-fix behaviour, and it looks like a hole",
    );
    let (row, col) = stock.z_grid.world_to_cell(xy[0], xy[1]).unwrap();
    let bottom = ray_bottom(stock.z_grid.ray(row, col)).unwrap();
    assert!(
        (f64::from(bottom) - 12.0).abs() < 0.3,
        "the surviving material starts at the tip, got {bottom}"
    );
}

// ── 3. The break-through pin hole — the visible failure ─────────────────

#[test]
fn flipped_break_through_pin_hole_is_carved_not_skipped() {
    // An alignment pin drilled through the blank and 1 mm into the
    // spoilboard: local bottom_z = -1.
    let local = drill(ToolProfile::Flat, 6.0, [8.0, 8.0], STOCK_Z, -1.0);
    let global = group_drill_op_to_global(&local, &bottom_flip(), P3::new(0.0, 0.0, 0.0));

    let h = global.holes[0];
    assert!(
        h.bottom_z > STOCK_Z,
        "the mapped tip sits ABOVE the blank ({} > {STOCK_Z}) — clearing \
         'above' it is a no-op, which is why no hole appeared",
        h.bottom_z
    );

    let mut stock = blank();
    let xy = h.xy;
    let report = stock.apply_drill_op(&global, StockCutDirection::FromBottom);

    // Non-vacuity: a kernel that walked no cells would also leave "no
    // material in the hole" trivially true if the ray were empty to begin
    // with. It is not, but the guard is the point.
    assert_eq!(
        report.holes_carved, 1,
        "the footprint must actually be walked"
    );
    assert_eq!(report.holes_off_grid, 0);

    let (row, col) = stock.z_grid.world_to_cell(xy[0], xy[1]).unwrap();
    let remaining = ray_material_length(stock.z_grid.ray(row, col));
    assert!(
        f64::from(remaining) < 0.05,
        "a through-hole must clear the full 25 mm ray, {remaining} mm left"
    );

    // And a cell outside the Ø6 footprint is untouched, so this is a hole
    // and not a wholesale clear.
    let away = [8.0 + 6.0, 8.0];
    assert!(
        material_between(&stock, away, 0.0, STOCK_Z) > 24.9,
        "stock 6 mm from the hole centre must be intact"
    );
}

// ── 4. The cone points against the advance in both directions ───────────

#[test]
fn twist_drill_cone_opens_back_toward_the_tool_in_both_directions() {
    let profile = ToolProfile::StandardTwist;
    let diameter = 6.0;
    let xy = [20.0, 15.0];

    // Down: the tip is the deepest point, the rim is shallower (higher).
    let mut down = blank();
    down.apply_drill_op(
        &drill(profile, diameter, xy, STOCK_Z, 13.0),
        StockCutDirection::FromTop,
    );
    let (r, c) = down.z_grid.world_to_cell(xy[0], xy[1]).unwrap();
    let axis_top = f64::from(ray_top(down.z_grid.ray(r, c)).unwrap());
    let (r2, c2) = down.z_grid.world_to_cell(xy[0] + 2.5, xy[1]).unwrap();
    let rim_top = f64::from(ray_top(down.z_grid.ray(r2, c2)).unwrap());
    assert!(
        rim_top > axis_top + 0.5,
        "drilling down, the rim floor sits above the axis floor \
         ({rim_top} vs {axis_top})"
    );

    // Up: mirrored. The rim floor sits BELOW the axis, because the cone
    // still opens back toward the tool and the tool is underneath.
    let local = drill(profile, diameter, xy, STOCK_Z, 13.0);
    let global = group_drill_op_to_global(&local, &bottom_flip(), P3::new(0.0, 0.0, 0.0));
    let mut up = blank();
    up.apply_drill_op(&global, StockCutDirection::FromBottom);
    let gxy = global.holes[0].xy;
    let (r3, c3) = up.z_grid.world_to_cell(gxy[0], gxy[1]).unwrap();
    let axis_bot = f64::from(ray_bottom(up.z_grid.ray(r3, c3)).unwrap());
    let (r4, c4) = up.z_grid.world_to_cell(gxy[0] + 2.5, gxy[1]).unwrap();
    let rim_bot = f64::from(ray_bottom(up.z_grid.ray(r4, c4)).unwrap());
    assert!(
        rim_bot < axis_bot - 0.5,
        "drilling up, the rim ceiling sits below the axis ceiling \
         ({rim_bot} vs {axis_bot})"
    );
}

// ── 5. Lateral setups abstain rather than fabricate ─────────────────────

#[test]
fn lateral_setup_removes_nothing_and_says_so() {
    let local = drill(ToolProfile::Flat, 6.0, [20.0, 15.0], STOCK_Z, 13.0);
    let mut stock = blank();
    let before = material_between(&stock, [20.0, 15.0], 0.0, STOCK_Z);

    let report = stock.apply_drill_op(&local, StockCutDirection::FromFront);

    assert!(
        report.unrepresentable_axis,
        "a Y-axis drill is not expressible as a DrillHole and must be \
         reported, not approximated"
    );
    assert_eq!(report.holes_carved, 0);
    let after = material_between(&stock, [20.0, 15.0], 0.0, STOCK_Z);
    assert!(
        (after - before).abs() < 1e-9,
        "abstaining means the Z grid is untouched, {before} -> {after}"
    );
}
