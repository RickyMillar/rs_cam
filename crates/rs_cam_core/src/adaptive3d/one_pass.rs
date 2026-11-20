//! P41: the one-pass rule of a 3D Rough, in the region grid of each level.
//!
//! A stock cell is too tall for a level when one tool centre of the level,
//! at its level floor, would take more than one Depth/Pass plus the
//! stay-down clearance from it in one stamp. Such stock stands because no
//! level above cut it (a region under the cut-length dial, the margin
//! strip at a wall, a strip narrower than the tool radius). No centre of
//! the level may take that much from it: each material cell that would is
//! not a region cell. The 2D plan of the level gets the region without
//! those centres, so its entries, links and order come from the plan as
//! they do for every region.
//!
//! The read is the quantity that one stamp removes from a cell: the covered
//! part of the cell by the tool disc (`TriDexelStock::disc_cell_coverage`,
//! the function of the stamp) times the stock above the cutter surface over
//! the cell. The stock top is `stock_top_z_at`, the read of the material
//! test. The rule has no margin of its own: the 2D plan keeps its centre
//! one planner cell or more inside the raw region boundary.
//!
//! Why the height of a stock cell is read against the LOWEST cutter that
//! can cover it ([`lowest_cutter_over`]), and not against the cutter of each
//! centre: the centres held off a cell are then a disc around the cell.
//! Geodesic Adaptive opens a region on its grown outline, which closes
//! every gap narrower than about two tool radii (`grown_outlines`, the
//! closing guard). A held set that is a union of discs of the tool radius
//! has no such gap, so the closing cannot give a held centre back to the
//! plan. Measured on the Wanaka back with the per-centre read: a centre on
//! a shelf (floor Z 11.0) was not held off a patch that a centre on the
//! pocket floor (Z 8.55) was held off; the closing gave the pocket centres
//! back, and three pieces cut 8.0 mm deep. The lowest cutter is read over
//! the cells where a centre of this level can stand: a material cell of
//! the level grid, inside the boundary clip where one applies.
//!
//! The switch is `Adaptive3dLinking::one_pass_hold` (`OnePassHold`): the
//! session sets `RemainingStock` only on remaining stock, and its text says
//! why. P43: with `FreshStock` this guard does not act (`OnePass::on` is
//! false); a fresh rough keeps the promise with the wall pass of each level
//! alone (`level_contour::WallRule::FreshStock`).
//!
//! `clearing.rs::build_material_bool_grid` calls [`OnePass::hold_off`] on
//! each grid it builds. `waterline_cleanup` masks its contour points with
//! [`OnePass::blocks`]: the wall contour is the one fed path that does not
//! come from the grid.

use std::cell::RefCell;
use std::collections::BTreeMap;

use crate::dexel_stock::TriDexelStock;
use crate::geo::P3;
use crate::stock::radial_profile::RadialProfileLUT;
use crate::surface::slope::SurfaceHeightmap;

use super::intended_floor::level_floor_z;
use super::stock_top_z_at;

/// One stock cell that holds a tool centre off, and the cutter surface over
/// it at that centre.
#[derive(Debug, Clone, Copy)]
pub(super) struct DeepCell {
    pub row: usize,
    pub col: usize,
    /// The Z of the cutter surface over the cell.
    pub cutter_z: f64,
}

/// What the rule held off, for `left_stock`.
#[derive(Debug, Default)]
pub(super) struct HeldOff {
    /// The centre cells held off, summed over the levels, and the wall
    /// contour points that the rule masked.
    pub centres: usize,
    /// Each stock cell that held a centre off, with the lowest cutter
    /// surface that a held centre had over it.
    pub cells: BTreeMap<(usize, usize), f64>,
}

/// The one-pass rule of one operation and its record.
pub(super) struct OnePass {
    /// One Depth/Pass plus the stay-down clearance (mm): the bound of the
    /// deep step (`StepSplit::StockDepth`) and of the held links.
    pub limit_mm: f64,
    /// `Adaptive3dLinking::one_pass_hold` is `RemainingStock`. Off (the
    /// dial `Off` or `FreshStock`): no centre and no contour point is held.
    pub on: bool,
    pub held: RefCell<HeldOff>,
}

/// The floor of a tool centre at one level: the read of the material test
/// (`clearing.rs::cell_has_material`).
#[derive(Clone, Copy)]
pub(super) struct LevelFloor<'a> {
    pub surface_hm: &'a SurfaceHeightmap,
    pub z_level: f64,
    pub stock_to_leave: f64,
    /// The boundary clip of the op (`centre_clip.rs`), or `None`. No centre
    /// stands outside it.
    pub centre_clip: Option<&'a super::centre_clip::CentreClip>,
}

impl LevelFloor<'_> {
    fn at(&self, row: usize, col: usize) -> f64 {
        level_floor_z(
            self.surface_hm.z_or_bbox_floor_at(row, col),
            self.stock_to_leave,
            self.z_level,
        )
    }
}

/// The tool of the read: the table of the cut stamp and the radius of its
/// disc.
#[derive(Clone, Copy)]
pub(super) struct OnePassTool<'a> {
    pub lut: &'a RadialProfileLUT,
    pub radius: f64,
}

impl OnePass {
    pub(super) fn new(depth_per_pass: f64, stay_down_clearance_mm: f64, on: bool) -> Self {
        Self {
            limit_mm: depth_per_pass + stay_down_clearance_mm.max(0.0),
            on,
            held: RefCell::new(HeldOff::default()),
        }
    }

    /// THE height test of the rule: `over_mm` of stock above a cutter
    /// surface is more than one Depth/Pass plus the stay-down clearance.
    /// The level grid ([`deep_cell`]) and the wall pass of a level
    /// (`level_contour::region_wall_pass`: the strip against the cutter of
    /// the next level, P42) both read it.
    pub(super) fn over_limit(&self, over_mm: f64) -> bool {
        over_limit(over_mm, self.limit_mm)
    }

    fn record(&self, cell: DeepCell) {
        let mut held = self.held.borrow_mut();
        held.centres += 1;
        held.cells
            .entry((cell.row, cell.col))
            .and_modify(|z| *z = z.min(cell.cutter_z))
            .or_insert(cell.cutter_z);
    }

    /// True when the tool at `p` takes more than the limit from one stock
    /// cell. The point is recorded as held off.
    pub(super) fn blocks(&self, stock: &TriDexelStock, tool: OnePassTool<'_>, p: P3) -> bool {
        if !self.on {
            return false;
        }
        let grid = &stock.z_grid;
        let found = deep_cell(
            (grid.rows, grid.cols),
            (grid.origin_u, grid.origin_v, grid.cell_size),
            p,
            tool,
            self.limit_mm,
            |row, col| stock_top_z_at(stock, row, col),
            None,
        );
        if let Some(cell) = found {
            self.record(cell);
        }
        found.is_some()
    }

    /// Remove from the padded level grid (`build_material_bool_grid`: one
    /// cell of border, `padded_cols` wide) each material cell whose tool
    /// takes more than the limit from one stock cell. Returns the number of
    /// cells that it removed.
    #[expect(
        clippy::indexing_slicing,
        reason = "each index is inside the box of the material cells, clamped to the grid at the top"
    )]
    pub(super) fn hold_off(
        &self,
        padded: &mut [bool],
        padded_cols: usize,
        stock: &TriDexelStock,
        level: LevelFloor<'_>,
        tool: OnePassTool<'_>,
    ) -> usize {
        let z_level = level.z_level;
        let grid = &stock.z_grid;
        let (rows, cols, cs) = (grid.rows, grid.cols, grid.cell_size);
        if !self.on || rows == 0 || cols == 0 || padded.len() != (rows + 2) * padded_cols {
            return 0;
        }
        let is_material =
            |padded: &[bool], row: usize, col: usize| padded[(row + 1) * padded_cols + (col + 1)];
        // The box of the material cells.
        let mut bound: Option<(usize, usize, usize, usize)> = None;
        for row in 0..rows {
            for col in 0..cols {
                if is_material(padded, row, col) {
                    bound = Some(match bound {
                        None => (row, row, col, col),
                        Some((r0, r1, c0, c1)) => {
                            (r0.min(row), r1.max(row), c0.min(col), c1.max(col))
                        }
                    });
                }
            }
        }
        let Some((r0, r1, c0, c1)) = bound else {
            return 0;
        };
        // The cells that a disc of a material cell can cover, in part too.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a radius over a cell size, positive and small"
        )]
        let reach = (tool.radius / cs).ceil().max(0.0) as usize + 1;
        let (b_r0, b_r1) = (r0.saturating_sub(reach), (r1 + reach).min(rows - 1));
        let (b_c0, b_c1) = (c0.saturating_sub(reach), (c1 + reach).min(cols - 1));
        let (bh, bw) = (b_r1 - b_r0 + 1, b_c1 - b_c0 + 1);
        // A cell can hold a centre off only when it stands more than the
        // limit above the level: the floor of a centre is at or above the
        // level. `tall` holds the top of such a cell, else -inf; `sum` is
        // the count of such cells in the box up to a cell (a summed-area
        // table), so that a centre with none in reach costs four reads.
        let tall_floor = z_level + self.limit_mm;
        let mut tall = vec![f64::NEG_INFINITY; bh * bw];
        let mut sum = vec![0u32; (bh + 1) * (bw + 1)];
        let mut any = false;
        for r in 0..bh {
            let mut row_sum = 0u32;
            for c in 0..bw {
                let top = stock_top_z_at(stock, b_r0 + r, b_c0 + c);
                if top > tall_floor {
                    tall[r * bw + c] = top;
                    row_sum += 1;
                    any = true;
                }
                sum[(r + 1) * (bw + 1) + (c + 1)] = sum[r * (bw + 1) + (c + 1)] + row_sum;
            }
        }
        if !any {
            return 0;
        }
        let count_in = |ra: usize, rb: usize, ca: usize, cb: usize| -> u32 {
            // Inclusive box in box coordinates.
            let at = |r: usize, c: usize| sum[r * (bw + 1) + c];
            at(rb + 1, cb + 1) + at(ra, ca) - at(ra, cb + 1) - at(rb + 1, ca)
        };
        // The cells where a centre of this level can stand: the material
        // cells of the grid as it came in, inside the boundary clip. The
        // loop below collects the held cells and removes them after it, so
        // `padded` is that grid for the whole loop.
        let can_stand = |row: usize, col: usize| {
            is_material(padded, row, col)
                && level.centre_clip.is_none_or(|clip| {
                    let (x, y) = grid.cell_to_world(row, col);
                    clip.contains(x, y)
                })
        };
        // The lowest cutter surface that such a centre can have over a stock
        // cell (`lowest_cutter_over`), read once per cell. NaN: not read.
        let lowest = RefCell::new(vec![f64::NAN; bh * bw]);
        let lowest_over = |t_row: usize, t_col: usize| -> f64 {
            let i = (t_row - b_r0) * bw + (t_col - b_c0);
            let cached = lowest.borrow()[i];
            if !cached.is_nan() {
                return cached;
            }
            let z = lowest_cutter_over(
                (rows, cols),
                cs,
                (t_row, t_col),
                tool,
                z_level,
                |row, col| can_stand(row, col).then(|| level.at(row, col)),
            );
            lowest.borrow_mut()[i] = z;
            z
        };
        let in_box = |t_row: usize, t_col: usize| {
            (b_r0..=b_r1).contains(&t_row) && (b_c0..=b_c1).contains(&t_col)
        };
        let mut held: Vec<usize> = Vec::new();
        for row in r0..=r1 {
            for col in c0..=c1 {
                if !is_material(padded, row, col) {
                    continue;
                }
                let (br, bc) = (row - b_r0, col - b_c0);
                if count_in(
                    br.saturating_sub(reach),
                    (br + reach).min(bh - 1),
                    bc.saturating_sub(reach),
                    (bc + reach).min(bw - 1),
                ) == 0
                {
                    continue;
                }
                let (x, y) = grid.cell_to_world(row, col);
                let floor = level.at(row, col);
                let found = deep_cell(
                    (rows, cols),
                    (grid.origin_u, grid.origin_v, cs),
                    P3::new(x, y, floor),
                    tool,
                    self.limit_mm,
                    |t_row, t_col| {
                        if in_box(t_row, t_col) {
                            tall[(t_row - b_r0) * bw + (t_col - b_c0)]
                        } else {
                            f64::NEG_INFINITY
                        }
                    },
                    Some(&lowest_over),
                );
                if let Some(cell) = found {
                    held.push((row + 1) * padded_cols + (col + 1));
                    self.record(cell);
                }
            }
        }
        for &i in &held {
            padded[i] = false;
        }
        held.len()
    }
}

/// [`OnePass::over_limit`] on its two numbers.
fn over_limit(over_mm: f64, limit_mm: f64) -> bool {
    over_mm > limit_mm
}

/// The lowest cutter surface (Z) that a tool centre of the level can have
/// over the stock cell `cell`: over each grid cell where a centre can stand
/// (`floor_at` gives its level floor, `None`: no centre stands there) and
/// whose disc covers the centre of `cell`, the least of that floor plus the
/// profile height at that distance. `+inf`: no centre covers the cell. No
/// floor is under `z_level`, so the scan stops when it reads the level.
fn lowest_cutter_over(
    dims: (usize, usize),
    cs: f64,
    cell: (usize, usize),
    tool: OnePassTool<'_>,
    z_level: f64,
    floor_at: impl Fn(usize, usize) -> Option<f64>,
) -> f64 {
    let (rows, cols) = dims;
    let (t_row, t_col) = cell;
    let r_sq = tool.radius * tool.radius;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a radius over a cell size, positive and small"
    )]
    let k = (tool.radius / cs).floor().max(0.0) as usize;
    let mut lowest = f64::INFINITY;
    for row in t_row.saturating_sub(k)..=(t_row + k).min(rows.saturating_sub(1)) {
        let dv = (row as f64 - t_row as f64) * cs;
        for col in t_col.saturating_sub(k)..=(t_col + k).min(cols.saturating_sub(1)) {
            let du = (col as f64 - t_col as f64) * cs;
            let d_sq = du * du + dv * dv;
            if d_sq > r_sq {
                continue;
            }
            let profile = tool
                .lut
                .height_at_dist_sq(d_sq.min(tool.lut.radius_sq()))
                .unwrap_or(0.0);
            let Some(floor) = floor_at(row, col) else {
                continue;
            };
            lowest = lowest.min(floor + profile);
            if lowest <= z_level {
                return lowest;
            }
        }
    }
    lowest
}

/// The first stock cell from which the tool at `p` (its tip at `p.z`) takes
/// more than `limit_mm` in one stamp: the covered part of the cell times
/// the stock above the cutter surface over the cell. `top_at` gives the
/// stock top of a cell (`-inf`: the cell cannot count).
///
/// With `lowest_over` the height of a cell is read against the lowest
/// cutter surface that any centre of the grid can have over it, not only
/// the cutter at `p` (the module text says why).
///
/// `dims` is `(rows, cols)` and `frame` is `(origin_u, origin_v, cell
/// size)` of the stock grid.
fn deep_cell(
    dims: (usize, usize),
    frame: (f64, f64, f64),
    p: P3,
    tool: OnePassTool<'_>,
    limit_mm: f64,
    top_at: impl Fn(usize, usize) -> f64,
    lowest_over: Option<&dyn Fn(usize, usize) -> f64>,
) -> Option<DeepCell> {
    let (rows, cols) = dims;
    let (origin_u, origin_v, cs) = frame;
    if cs <= 0.0 || cs.is_nan() || rows == 0 || cols == 0 {
        return None;
    }
    let r = tool.radius;
    let r_sq = r * r;
    // A cell whose centre is more than one cell past the disc edge has no
    // covered part.
    let far_sq = (r + cs) * (r + cs);
    let index = |v: f64, origin: f64, n: usize, up: bool| -> usize {
        let i = (v - origin) / cs;
        let i = if up { i.ceil() } else { i.floor() };
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped to 0..n before the cast"
        )]
        {
            i.clamp(0.0, (n - 1) as f64) as usize
        }
    };
    let (row_lo, row_hi) = (
        index(p.y - r - cs, origin_v, rows, false),
        index(p.y + r + cs, origin_v, rows, true),
    );
    let (col_lo, col_hi) = (
        index(p.x - r - cs, origin_u, cols, false),
        index(p.x + r + cs, origin_u, cols, true),
    );
    for row in row_lo..=row_hi {
        let dv = origin_v + row as f64 * cs - p.y;
        for col in col_lo..=col_hi {
            let top = top_at(row, col);
            // With `lowest_over`, `top_at` gives -inf for a cell that no
            // centre can take the limit from.
            if !top.is_finite() || (lowest_over.is_none() && top <= p.z + limit_mm) {
                continue;
            }
            let du = origin_u + col as f64 * cs - p.x;
            let d_sq = du * du + dv * dv;
            if d_sq > far_sq {
                continue;
            }
            // The profile height over the cell, read at the cutter edge for
            // a cell past it (the stamp does the same).
            let profile = tool
                .lut
                .height_at_dist_sq(d_sq.min(tool.lut.radius_sq()))
                .unwrap_or(0.0);
            let cutter_z = match lowest_over {
                Some(lowest) => lowest(row, col).min(p.z + profile),
                None => p.z + profile,
            };
            let over = top - cutter_z;
            if !over_limit(over, limit_mm) {
                continue;
            }
            let covered = TriDexelStock::disc_cell_coverage(du, dv, r_sq, cs);
            if covered * over > limit_mm {
                return Some(DeepCell { row, col, cutter_z });
            }
        }
    }
    None
}
