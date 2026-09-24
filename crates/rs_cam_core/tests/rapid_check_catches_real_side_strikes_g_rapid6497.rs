//! G-RAPID6497 characterisation sentries — what the live rapid check
//! (`RapidClearanceCheck::strikes`) must go on catching.
//!
//! A diagnosis of rivmap100 move 6497 first read one strike as a check
//! artefact: a steep wall just outside the 3 mm radius, read at full height
//! through the sliver-safe `conservative_top` of a rim cell. A later
//! measurement (G-PLANSIMGAP) showed the wall is real. The segment-merge
//! dressup moved a ring after the planner stamped it, and the wall stands
//! about 0.05 mm from the tool edge. The strike is a true positive, so the
//! check stays as it is.
//!
//! These tests pin the strike classes that any later change to the check must
//! keep. Each one passes on the check as it is:
//!
//! * (b) the cutter's side enters a solid block 0.3 mm and 1 mm deep;
//! * (c) a sub-cell sliver stands under the footprint, where only
//!   `conservative_top` knows it is there;
//! * (d) a steep wall stands inside the rim.
//!
//! Each case runs at 0.5 mm and 0.25 mm cells. (b) and (d) run at four
//! sub-cell phases of the axis, so the verdict does not depend on grid
//! alignment. (b) also runs through the production metric walk.
//!
//! Known limit, not pinned here: the check subtracts a Z tolerance of
//! 2.207 cells (1.10 mm at 0.5 mm cells). A tip that goes less than that
//! into material with the whole footprint over it is not flagged.
//!
//! ```text
//! cargo test -p rs_cam_core --test rapid_check_catches_real_side_strikes_g_rapid6497
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::stock::collision::RapidClearanceCheck;
use rs_cam_core::stock::dexel::ray_subtract_above;
use rs_cam_core::stock::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use rs_cam_core::tool::{FlatEndmill, MillingCutter};
use rs_cam_core::toolpath::Toolpath;

/// The rivmap100 tool: a flat 6 mm end mill.
const R: f64 = 3.0;
/// The top of every standing wall, block or sliver.
const TOP_Z: f64 = 8.0;
/// The cut floor around it.
const FLOOR_Z: f64 = 0.0;
/// The descent ends 1 mm above the floor: clear of every floor cell, and
/// 7 mm under every standing top.
const TIP_Z: f64 = 1.0;
/// The Y of the sliver descents: a row of cell centres at both resolutions.
const AXIS_ROW: usize = 40;

fn flat() -> FlatEndmill {
    FlatEndmill::new(2.0 * R, 25.0)
}

/// The state of one cell in a fixture.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cell {
    /// Cut to the floor over the whole square.
    Floor,
    /// Full height over the whole square.
    Full,
    /// Partly cut: the centre ray reads the floor, but material may still
    /// stand at full height in part of the square, so `conservative_top`
    /// stays at `TOP_Z`. This is the sliver case.
    Sliver,
}

/// A 30 × 30 board at `TOP_Z`, with each cell set by `state(x, y)` of its
/// centre.
fn board(cs: f64, state: impl Fn(f64, f64) -> Cell) -> TriDexelStock {
    let mut stock = TriDexelStock::from_stock(0.0, 0.0, 30.0, 30.0, -5.0, TOP_Z, cs);
    let rows = stock.z_grid.rows;
    let cols = stock.z_grid.cols;
    let ou = stock.z_grid.origin_u;
    let ov = stock.z_grid.origin_v;
    for row in 0..rows {
        for col in 0..cols {
            let x = ou + col as f64 * cs;
            let y = ov + row as f64 * cs;
            match state(x, y) {
                Cell::Full => {}
                Cell::Floor => stock.clear_above_at(row, col, FLOOR_Z as f32),
                Cell::Sliver => {
                    // Lower the ray only. `conservative_top` stays high.
                    let ray = &mut stock.z_grid.rays[row * cols + col];
                    ray_subtract_above(ray, FLOOR_Z as f32);
                }
            }
        }
    }
    stock
}

/// The X of cell column `col` and the Y of the axis row.
fn cell_centre(stock: &TriDexelStock, col: usize) -> (f64, f64) {
    let g = &stock.z_grid;
    (
        g.origin_u + col as f64 * g.cell_size,
        g.origin_v + AXIS_ROW as f64 * g.cell_size,
    )
}

/// A wall face parallel to Y at `face_x`: full height at and past the face,
/// a partly cut cell where the face crosses the square, floor elsewhere.
fn wall_state(face_x: f64, cs: f64) -> impl Fn(f64, f64) -> Cell {
    move |x, _y| {
        if x >= face_x {
            Cell::Full
        } else if x + 0.5 * cs > face_x {
            Cell::Sliver
        } else {
            Cell::Floor
        }
    }
}

fn strikes_descent(stock: &TriDexelStock, cx: f64, cy: f64) -> bool {
    let cutter = flat();
    let check = RapidClearanceCheck::new(&cutter);
    check.strikes(stock, P3::new(cx, cy, 19.0), P3::new(cx, cy, TIP_Z), R)
}

/// Run the metric walk (the production seam) and return the flagged moves.
fn live_hits_metric(stock: &mut TriDexelStock, tp: &Toolpath) -> Vec<usize> {
    let cutter = flat();
    let lut = RadialProfileLUT::from_cutter(&cutter, LUT_SAMPLES);
    let never_cancel = || false;
    let mut check = RapidClearanceCheck::new(&cutter);
    stock
        .simulate_toolpath_with_lut_metrics_rapid_checked(
            tp,
            &lut,
            &cutter,
            cutter.radius(),
            StockCutDirection::FromTop,
            ToolpathId(0),
            18_000,
            2,
            5000.0,
            0.25,
            None,
            &[],
            &[],
            true,
            &never_cancel,
            Some(&mut check),
        )
        .expect("never cancelled");
    check
        .into_hits()
        .into_iter()
        .map(|c| c.move_index)
        .collect()
}

fn descent_toolpath(cx: f64, cy: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(cx, cy, 19.0));
    tp.rapid_to(P3::new(cx, cy, TIP_Z));
    tp
}

/// (b) A REAL CROSSING — a solid block whose face lies `e` inside the radius.
/// The descent drives the cutter's side 0.3 mm (and 1 mm) into a block that
/// stands 7 mm above the tip. Four sub-cell phases of the axis prove the
/// verdict does not depend on grid alignment.
#[test]
fn a_footprint_that_enters_a_block_is_a_strike() {
    for cs in [0.5, 0.25] {
        for e in [0.3, 1.0] {
            for phase in [0.0, 0.25, 0.5, 0.75] {
                let cx = 10.0 + phase * cs;
                let cy = 10.0;
                let face_x = cx + R - e;
                let stock = board(cs, wall_state(face_x, cs));
                assert!(
                    strikes_descent(&stock, cx, cy),
                    "cs {cs}, phase {phase}: the cutter's side enters a block \
                     {e} mm deep and {} mm tall; it must strike",
                    TOP_Z - TIP_Z
                );
            }
        }
    }
}

/// (b) through the production walk.
#[test]
fn a_footprint_that_enters_a_block_is_a_strike_in_the_walk() {
    let (cx, cy) = (10.0, 10.0);
    let mut stock = board(0.5, wall_state(cx + R - 0.3, 0.5));
    assert_eq!(
        live_hits_metric(&mut stock, &descent_toolpath(cx, cy)),
        vec![1]
    );
}

/// (c) A SLIVER — a rib narrower than one cell under the footprint, standing
/// above the tip. Its centre ray reads the floor; only `conservative_top`
/// knows it is there. Two positions: 1.5 mm off the axis, and in the
/// outermost cell column whose whole square is inside the radius (the edge of
/// the square cells that lie wholly inside the radius).
#[test]
fn a_sliver_under_the_footprint_is_a_strike() {
    for cs in [0.5, 0.25] {
        let probe = TriDexelStock::from_stock(0.0, 0.0, 30.0, 30.0, -5.0, TOP_Z, cs);
        let axis_col = (10.0 / cs).round() as usize;
        let (cx, cy) = cell_centre(&probe, axis_col);
        let near = cx + 1.5;
        // The outermost column whose square lies wholly inside the radius on
        // the axis row.
        let mut edge_col = axis_col;
        while {
            let (x, _) = cell_centre(&probe, edge_col + 1);
            let far_x = x - cx + 0.5 * cs;
            (far_x * far_x + (0.5 * cs) * (0.5 * cs)).sqrt() <= R
        } {
            edge_col += 1;
        }
        let (edge_x, _) = cell_centre(&probe, edge_col);
        for rib_x in [near, edge_x] {
            let stock = board(cs, move |x, _y| {
                if (x - rib_x).abs() < 0.5 * cs {
                    Cell::Sliver
                } else {
                    Cell::Floor
                }
            });
            assert!(
                strikes_descent(&stock, cx, cy),
                "cs {cs}: a sub-cell rib {:.3} mm off the axis stands {} mm \
                 above the tip; it must strike",
                rib_x - cx,
                TOP_Z - TIP_Z
            );
        }
    }
}

/// (d) A STEEP WALL INSIDE THE RADIUS near the rim, above the tip. The face
/// lies 0.05 mm more than one cell diagonal (`cs * sqrt(2)`) inside the rim,
/// so at least one square of the wall lies wholly inside the radius.
#[test]
fn a_steep_wall_inside_the_rim_is_a_strike() {
    for cs in [0.5, 0.25] {
        for phase in [0.0, 0.25, 0.5, 0.75] {
            let cx = 10.0 + phase * cs;
            let cy = 10.0;
            let inset = cs * std::f64::consts::SQRT_2 + 0.05;
            let stock = board(cs, wall_state(cx + R - inset, cs));
            assert!(
                strikes_descent(&stock, cx, cy),
                "cs {cs}, phase {phase}: a wall {inset:.3} mm inside the rim \
                 stands {} mm above the tip; it must strike",
                TOP_Z - TIP_Z
            );
        }
    }
}
