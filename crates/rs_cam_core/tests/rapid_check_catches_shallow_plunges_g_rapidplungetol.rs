//! G-RAPIDPLUNGETOL sentries — the live rapid check must see a SHALLOW
//! plunge into material under the footprint.
//!
//! Before this package `RapidClearanceCheck::strikes` flagged a sample only
//! when the tip sat below `max_clearance_tip_z_for_profile − τ`, with
//! `τ = cs·(1/√2 + 0.5 + 1.0)`. A tip less than τ into material under the
//! whole footprint went unflagged. The check now has two channels
//! (`planning/rapid_safety_2026-08-28/RAPIDPLUNGETOL_PLAN.md` §2 A):
//!
//! * LOW — `TriDexelStock::clearance_bounds_for_profile`'s `low`, over the
//!   cells whose whole square is inside the footprint, erring low. A tip
//!   below `low − ε` is flagged; `ε = 2·f32::EPSILON·max(|low|, 1)`.
//! * HIGH — the old test, unchanged: `z < high − τ`.
//!
//! Numbers used below, all from the geometry:
//!
//! * Flat Ø6: `R = 3`, `h(r) = 0` for `r ≤ R`, so every inside cell reads
//!   `low = ray_top` and every visited cell reads `high = conservative_top`.
//! * `τ(cs) = cs·(1/√2 + 1.5)`: `τ(0.5) = 0.5·2.20711 = 1.10355`,
//!   `τ(0.25) = 0.25·2.20711 = 0.55178`. So the old check needed the tip
//!   under `8 − 1.10355 = 6.89645` (0.5 mm cells) or `8 − 0.55178 = 7.44822`
//!   (0.25 mm cells) on a board at `TOP_Z = 8`.
//! * `ε` at `low = 8` is `2 · 1.19e-7 · 8 ≈ 1.9e-6` mm: every depth below is
//!   at least 0.3 mm, five orders of magnitude clear of it.
//!
//! Items 1–3 were red against the high channel alone (the pre-fix check) and
//! are green with both; items 4–5 are the benign classes, silent before and
//! after; 6–7 pin the primitive.
//!
//! ```text
//! cargo test -p rs_cam_core --test rapid_check_catches_shallow_plunges_g_rapidplungetol
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
use rs_cam_core::tool::{FlatEndmill, MillingCutter, TaperedBallEndmill};
use rs_cam_core::toolpath::Toolpath;

/// The rivmap100 tool: a flat 6 mm end mill.
const R: f64 = 3.0;
/// Board top: fresh stock and the top of every block.
const TOP_Z: f64 = 8.0;
/// The cut floor.
const FLOOR_Z: f64 = 0.0;
/// Where every descent starts: 11 mm above the board, clear of everything.
const START_Z: f64 = 19.0;
/// Both resolutions the plan names.
const CELLS: [f64; 2] = [0.5, 0.25];
/// Sub-cell phases of the axis, as fractions of a cell.
const PHASES: [f64; 4] = [0.0, 0.25, 0.5, 0.75];

fn flat() -> FlatEndmill {
    FlatEndmill::new(2.0 * R, 25.0)
}

/// The old check's Z tolerance at `cs`: `cs·(1/√2 + 0.5 + 1.0)`.
fn tau(cs: f64) -> f64 {
    cs * (std::f64::consts::FRAC_1_SQRT_2 + 1.5)
}

/// The low channel's float slack at `low` (the plan's ε).
fn eps(low: f64) -> f64 {
    2.0 * f64::from(f32::EPSILON) * low.abs().max(1.0)
}

/// The state of one cell in a fixture.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Cell {
    /// Cut to the floor over the whole square.
    Floor,
    /// Full height over the whole square.
    Full,
    /// Partly cut: the centre ray reads the floor, `conservative_top` stays
    /// at `TOP_Z`.
    Sliver,
}

/// A 30 × 30 board at `TOP_Z`, each cell set by `state(x, y)` of its centre.
/// The same helper as the G-RAPID6497 sentries.
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
                    let ray = &mut stock.z_grid.rays[row * cols + col];
                    ray_subtract_above(ray, FLOOR_Z as f32);
                }
            }
        }
    }
    stock
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

fn strikes_descent(stock: &TriDexelStock, cx: f64, cy: f64, tip_z: f64) -> bool {
    let cutter = flat();
    let check = RapidClearanceCheck::new(&cutter);
    check.strikes(stock, P3::new(cx, cy, START_Z), P3::new(cx, cy, tip_z), R)
}

/// `(low, high)` of the primitive at `(cx, cy)` with the flat tool.
fn bounds(stock: &TriDexelStock, cx: f64, cy: f64) -> (Option<f64>, f64) {
    let cutter = flat();
    let lut = RadialProfileLUT::from_cutter(&cutter, LUT_SAMPLES);
    stock
        .clearance_bounds_for_profile(cx, cy, R, &cutter, &lut)
        .expect("the disc is on the grid")
}

fn descent_toolpath(cx: f64, cy: f64, tip_z: f64) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(cx, cy, START_Z));
    tp.rapid_to(P3::new(cx, cy, tip_z));
    tp
}

/// The metric walk (the production seam); the flagged move indices.
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

/// The non-metric playback walk (`replay_moves`); the flagged move indices.
fn live_hits_playback(stock: &mut TriDexelStock, tp: &Toolpath) -> Vec<usize> {
    let cutter = flat();
    let lut = RadialProfileLUT::from_cutter(&cutter, LUT_SAMPLES);
    let never_cancel = || false;
    let mut check = RapidClearanceCheck::new(&cutter);
    stock
        .simulate_toolpath_with_lut_cancel_rapid_checked(
            tp,
            &lut,
            cutter.radius(),
            StockCutDirection::FromTop,
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

/// 1. A fresh board, a rapid plunge `e` into it. The axis cell's centre is at
///    most a half-diagonal from the axis, so `d + hd ≤ 2·hd = cs·√2 ≤ 0.708 <
///    R`: it is inside, `ray_top = 8`, `low = 8`, and `8 − e < 8 − ε`.
///    Before: `(0.3, both cs)` and `(1.0, 0.5)` were silent — `7.7 ≥ 6.896`,
///    `7.7 ≥ 7.448`, `7.0 ≥ 6.896`. `(1.0, 0.25)` was already flagged
///    (`7.0 < 7.448`).
#[test]
fn a_shallow_plunge_under_the_whole_footprint_is_a_strike() {
    for cs in CELLS {
        for e in [0.3, 1.0] {
            for phase in PHASES {
                let (cx, cy) = (10.0 + phase * cs, 10.0);
                let stock = board(cs, |_, _| Cell::Full);
                assert!(
                    strikes_descent(&stock, cx, cy, TOP_Z - e),
                    "cs {cs}, phase {phase}: the tip ends {e} mm into a fresh \
                     board under the whole footprint; it must strike"
                );
            }
        }
    }
}

/// 2. Item 1 at `e = 0.3`, `cs = 0.5` through both production walks: the
///    descent is move 1.
#[test]
fn a_shallow_plunge_is_a_strike_in_both_walks() {
    let (cx, cy) = (10.0, 10.0);
    let tp = descent_toolpath(cx, cy, TOP_Z - 0.3);
    let mut stock = board(0.5, |_, _| Cell::Full);
    assert_eq!(live_hits_metric(&mut stock, &tp), vec![1], "metric walk");
    let mut stock = board(0.5, |_, _| Cell::Full);
    assert_eq!(
        live_hits_playback(&mut stock, &tp),
        vec![1],
        "playback walk"
    );
}

/// 3. A block whose face runs through the axis, `e = 0.3`. The axis-row cell
///    whose centre is the first at or past `cx` lies within `cs` of the axis,
///    so `d + hd ≤ cs + cs/√2 ≤ 0.854 < R`: inside and Full, `low = 8`.
///    Before: `high = 8` and `7.7 ≥ 8 − τ` at both resolutions — silent.
#[test]
fn a_shallow_plunge_half_under_a_block_is_a_strike() {
    for cs in CELLS {
        for phase in PHASES {
            let (cx, cy) = (10.0 + phase * cs, 10.0);
            let stock = board(cs, wall_state(cx, cs));
            assert!(
                strikes_descent(&stock, cx, cy, TOP_Z - 0.3),
                "cs {cs}, phase {phase}: half the footprint ends 0.3 mm into \
                 a block; it must strike"
            );
        }
    }
}

/// 4. Benign: every cell cut to the floor, the tip lands on it exactly.
///    `low = high = 0`; `0 < 0 − ε` and `0 < 0 − τ` are both false. Pins the
///    sign of ε: a slack of the wrong sign would flag a tip resting on the
///    floor.
#[test]
fn a_descent_to_a_cut_floor_is_clear() {
    for cs in CELLS {
        for phase in PHASES {
            let (cx, cy) = (10.0 + phase * cs, 10.0);
            let stock = board(cs, |_, _| Cell::Floor);
            assert!(
                !strikes_descent(&stock, cx, cy, FLOOR_Z),
                "cs {cs}, phase {phase}: a tip resting on the cut floor must \
                 not strike"
            );
        }
    }
}

/// 5. Benign: a wall 0.05 mm past the rim, the tip `τ/2` under its top.
///    Every Sliver or Full cell has centre `x > face − cs/2`, so
///    `d > R + 0.05 − cs/2`, and `d + hd > R + 0.05 + cs·(1/√2 − 1/2) > R`:
///    none is inside, every inside cell is Floor, `low = 0 < z`. On the axis
///    row the first Sliver or Full centre lies in `[face − cs/2, face +
///    cs/2)`; at these four phases it falls within the dilated reach
///    `R + cs/2`, so `high = 8` (item 6 asserts it), and `z = 8 − τ/2 ≥
///    8 − τ`: silent before and after. This is the one class τ still exists
///    for.
#[test]
fn a_wall_just_outside_the_rim_is_not_a_plunge_strike() {
    for cs in CELLS {
        for phase in PHASES {
            let (cx, cy) = (10.0 + phase * cs, 10.0);
            let stock = board(cs, wall_state(cx + R + 0.05, cs));
            assert!(
                !strikes_descent(&stock, cx, cy, TOP_Z - 0.5 * tau(cs)),
                "cs {cs}, phase {phase}: a wall 0.05 mm past the rim is the \
                 rim class τ suppresses; it must stay silent"
            );
        }
    }
}

/// 6. The primitive on every fixture above: `low ≤ high`, and each value the
///    derivations above name.
#[test]
fn the_low_bound_never_exceeds_the_high_bound() {
    for cs in CELLS {
        for phase in PHASES {
            let (cx, cy) = (10.0 + phase * cs, 10.0);
            let fixtures: [(&str, TriDexelStock, Option<f64>, f64); 4] = [
                (
                    "fresh board (1, 2)",
                    board(cs, |_, _| Cell::Full),
                    Some(TOP_Z),
                    TOP_Z,
                ),
                (
                    "block at the axis (3)",
                    board(cs, wall_state(cx, cs)),
                    Some(TOP_Z),
                    TOP_Z,
                ),
                (
                    "cut floor (4)",
                    board(cs, |_, _| Cell::Floor),
                    Some(FLOOR_Z),
                    FLOOR_Z,
                ),
                (
                    "wall past the rim (5)",
                    board(cs, wall_state(cx + R + 0.05, cs)),
                    Some(FLOOR_Z),
                    TOP_Z,
                ),
            ];
            for (name, stock, want_low, want_high) in fixtures {
                let (low, high) = bounds(&stock, cx, cy);
                assert_eq!(
                    (low, high),
                    (want_low, want_high),
                    "cs {cs}, phase {phase}, {name}"
                );
                assert!(
                    low.is_none_or(|low| low <= high),
                    "cs {cs}, phase {phase}, {name}: low {low:?} > high {high}"
                );
            }
        }
    }
}

/// 7. The primitive with a CURVED tool: a tapered ball (Ø3 tip, 10°, Ø6
///    shaft) cuts three horizontal passes at `y = 8, 12, 16`, tip at `Z_C`,
///    and the query sits on the middle pass. Every inside cell of the query
///    footprint lies within `R` of `y = 12`, so the middle pass swept it: its
///    stamp left it at `Z_C + h(d_perp)` or lower, where `d_perp ≤ d` is its
///    distance from the pass line and `d` its distance from the query. The
///    low channel charges it `h(d + hd) ≥ h(d_perp)` (the table is
///    non-decreasing), so `low ≤ Z_C + ε`. A cell the flank never reached
///    stands at `TOP_Z ≤ Z_C + h(d_perp)` by the same argument.
#[test]
fn a_curved_tool_reentering_its_own_passes_reads_low_at_or_under_the_cut() {
    const Z_C: f64 = 5.0;
    let cutter = TaperedBallEndmill::new(3.0, 10.0, 6.0, 25.0);
    let radius = cutter.radius();
    let lut = RadialProfileLUT::from_cutter(&cutter, LUT_SAMPLES);
    for cs in CELLS {
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 30.0, 30.0, -5.0, TOP_Z, cs);
        let mut tp = Toolpath::new();
        for y in [8.0, 12.0, 16.0] {
            // Enter and leave off the board (x = −5 and 35), so the only
            // stamps over the query are the horizontal passes.
            tp.rapid_to(P3::new(-5.0, y, START_Z));
            tp.feed_to(P3::new(-5.0, y, Z_C), 1000.0);
            tp.feed_to(P3::new(35.0, y, Z_C), 1000.0);
            tp.rapid_to(P3::new(35.0, y, START_Z));
        }
        let never_cancel = || false;
        stock
            .simulate_toolpath_with_lut_cancel_rapid_checked(
                &tp,
                &lut,
                radius,
                StockCutDirection::FromTop,
                &never_cancel,
                None,
            )
            .expect("never cancelled");
        for phase in PHASES {
            let x_mid = 15.0 + phase * cs;
            let (low, high) = stock
                .clearance_bounds_for_profile(x_mid, 12.0, radius, &cutter, &lut)
                .expect("the disc is on the grid");
            let low = low.expect("the axis cell holds material under the cut");
            assert!(
                low <= Z_C + eps(low),
                "cs {cs}, phase {phase}: low {low} over the cut Z {Z_C}"
            );
            assert!(
                low <= high,
                "cs {cs}, phase {phase}: low {low} > high {high}"
            );
        }
    }
}
