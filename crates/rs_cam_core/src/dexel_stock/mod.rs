//! Tri-dexel stock representation with tool stamping and toolpath simulation.
//!
//! Replaces the 2.5-D heightmap for volumetric material removal.  The Z-grid
//! is always present; X and Y grids are created lazily when side-face cuts are
//! needed (future work).

mod band;
mod cut_direction;
mod playback;
mod simulation;
mod stamping;
mod swept;
mod tile_mip;
mod whole_path;

pub use cut_direction::StockCutDirection;
pub use playback::{PlaybackDispatch, PlaybackDispatchStats};
pub use simulation::{
    ChipThicknessStats, chip_thickness_stats, effective_chip_thickness_mm, peak_chip_thickness_mm,
};
/// The dexel engagement channel's two measurement floors. Re-exported from
/// the (private) stamping kernel because [`crate::sim_measurability`] and its
/// consumers need to cite the numbers they abstain on.
pub use stamping::FRESH_MATERIAL_THRESHOLD_MM;
pub use whole_path::{StampDispatch, StampDispatchStats};

use stamping::{stamp_point_on_grid, stamp_segment_on_grid};

use crate::dexel::{DexelAxis, DexelGrid};
use crate::geo::{BoundingBox3, P3};
use crate::radial_profile::RadialProfileLUT;

/// What one [`TriDexelStock::apply_drill_op`] call actually removed.
///
/// Exists because the analytic drill kernel has three ways to remove nothing
/// while returning normally — an off-grid hole centre, a zero-diameter tool,
/// and a drill axis this kernel cannot represent — and G-DRILLFLIP showed
/// that "the stock looks unchanged" is a symptom a person has to be watching
/// the viewport to catch. A test can assert `holes_carved` instead, which is
/// the non-vacuity handle the fix's own sentries use.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DrillRemovalReport {
    /// Holes whose centre landed on the grid and whose footprint was walked.
    pub holes_carved: usize,
    /// Holes whose XY centre fell outside the grid entirely.
    pub holes_off_grid: usize,
    /// The drill axis was not this grid's Z axis (a lateral `FaceUp`), so
    /// nothing was removed. See `apply_drill_op`'s "Lateral setups abstain".
    pub unrepresentable_axis: bool,
}

// ── TriDexelStock ───────────────────────────────────────────────────────

/// Volumetric stock representation using three orthogonal dexel grids.
///
/// For the common top/bottom workflow only the Z-grid is needed.
pub struct TriDexelStock {
    pub z_grid: DexelGrid,
    pub x_grid: Option<DexelGrid>,
    pub y_grid: Option<DexelGrid>,
    pub stock_bbox: BoundingBox3,
    /// How the metric simulator schedules its stamp kernel (`PERF_REVIEW.md`
    /// S3). A **schedule only** — every shape produces bit-identical grids,
    /// samples and metrics, so this is safe to set from a bench harness or a
    /// determinism sentry. Defaults to [`StampDispatch::Auto`].
    pub stamp_dispatch: StampDispatch,
    /// What the last metric simulation's whole-toolpath dispatcher did.
    /// Diagnostics for the wave-4 non-vacuity sentries; reset at the start of
    /// every `simulate_toolpath_with_lut_metrics_cancel` and left all-zero when
    /// that run used per-stamp dispatch.
    pub last_stamp_dispatch: StampDispatchStats,
    /// How the **non-metric playback** replay schedules its stamp kernel
    /// (SIM w6). A schedule only — both shapes produce bit-identical grids, and
    /// that matters more here than on the metric side: this is the kernel that
    /// builds `global_stock`.
    ///
    /// **Corrected 2026-08-22:** this sentence used to end "which
    /// `StockSource::FromRemainingStock` generation reads". It does not.
    /// Rest generation reads `SimulationResult::prior_stocks`, and those
    /// snapshots are clones of the **per-setup local** `group_stock`
    /// (`compute/simulate.rs:917`, consumed at `session/compute.rs:1434`).
    /// `global_stock` feeds checkpoints, playback and the S5 prefix memo, and
    /// nothing else. The old wording made this schedule look like it had
    /// generation consequences it does not have — and it was cited as
    /// evidence in a defect write-up before anyone checked it. Defaults to [`PlaybackDispatch::Auto`].
    pub playback_dispatch: PlaybackDispatch,
    /// What the last non-metric replay's dispatcher did. Diagnostics for the
    /// w6 non-vacuity sentries; reset at the start of every
    /// `simulate_toolpath_with_lut_cancel` and left all-zero when that run was
    /// serial.
    pub last_playback_dispatch: PlaybackDispatchStats,
}

impl Clone for TriDexelStock {
    fn clone(&self) -> Self {
        Self {
            z_grid: self.z_grid.clone(),
            x_grid: self.x_grid.clone(),
            y_grid: self.y_grid.clone(),
            stock_bbox: self.stock_bbox,
            stamp_dispatch: self.stamp_dispatch,
            last_stamp_dispatch: self.last_stamp_dispatch,
            playback_dispatch: self.playback_dispatch,
            last_playback_dispatch: self.last_playback_dispatch,
        }
    }
}

impl TriDexelStock {
    /// Create a stock from a bounding box (Z-grid only).
    pub fn from_bounds(bbox: &BoundingBox3, cell_size: f64) -> Self {
        Self {
            z_grid: DexelGrid::z_grid_from_bounds(bbox, cell_size),
            x_grid: None,
            y_grid: None,
            stock_bbox: *bbox,
            stamp_dispatch: StampDispatch::default(),
            last_stamp_dispatch: StampDispatchStats::default(),
            playback_dispatch: PlaybackDispatch::default(),
            last_playback_dispatch: PlaybackDispatchStats::default(),
        }
    }

    /// Create from explicit stock dimensions (matches `Heightmap::from_stock`).
    pub fn from_stock(
        x_min: f64,
        y_min: f64,
        x_max: f64,
        y_max: f64,
        z_min: f64,
        z_max: f64,
        cell_size: f64,
    ) -> Self {
        let bbox = BoundingBox3 {
            min: P3::new(x_min, y_min, z_min),
            max: P3::new(x_max, y_max, z_max),
        };
        Self::from_bounds(&bbox, cell_size)
    }

    /// Clone the stock state for checkpointing.
    pub fn checkpoint(&self) -> Self {
        self.clone()
    }

    // ── Lazy grid initialization ────────────────────────────────────────

    /// Ensure the grid for `direction` exists, creating it lazily if needed.
    /// Returns a mutable reference to the appropriate grid.
    fn ensure_grid(&mut self, direction: StockCutDirection) -> &mut DexelGrid {
        let bbox = self.stock_bbox;
        let cell_size = self.z_grid.cell_size;
        match direction.grid_axis() {
            DexelAxis::Z => &mut self.z_grid,
            DexelAxis::Y => self
                .y_grid
                .get_or_insert_with(|| DexelGrid::y_grid_from_bounds(&bbox, cell_size)),
            DexelAxis::X => self
                .x_grid
                .get_or_insert_with(|| DexelGrid::x_grid_from_bounds(&bbox, cell_size)),
        }
    }

    // ── Single-position stamp ───────────────────────────────────────────

    /// Stamp a tool at a 3-D position into the grid determined by `direction`.
    ///
    /// The position `(cx, cy, tip_z)` is in global stock coordinates.
    /// For Z-grid (FromTop/FromBottom): the tool footprint is in XY, ray depth is Z.
    /// For Y-grid (FromFront/FromBack): footprint in XZ, depth is Y.
    /// For X-grid (FromLeft/FromRight): footprint in YZ, depth is X.
    pub fn stamp_tool_at(
        &mut self,
        lut: &RadialProfileLUT,
        radius: f64,
        cx: f64,
        cy: f64,
        tip_z: f64,
        direction: StockCutDirection,
    ) {
        let (cu, cv, cd) = direction.decompose(cx, cy, tip_z);
        let from_high = direction.cuts_from_high_side();
        let grid = self.ensure_grid(direction);
        stamp_point_on_grid(grid, lut, radius, cu, cv, cd, from_high, None);
    }

    // ── Swept linear segment ────────────────────────────────────────────

    /// Stamp the tool along a linear segment from `start` to `end`.
    ///
    /// Uses closest-point-on-segment to find the cutter height at each cell,
    /// matching the existing heightmap `stamp_linear_segment_lut`.
    pub fn stamp_linear_segment(
        &mut self,
        lut: &RadialProfileLUT,
        radius: f64,
        start: P3,
        end: P3,
        direction: StockCutDirection,
    ) {
        self.stamp_linear_segment_with_mip(lut, radius, start, end, direction, &mut None);
    }

    /// [`Self::stamp_linear_segment`] with the S2 air-skip mip threaded in.
    ///
    /// Private because the mip is an optimisation artefact with a lifetime tied
    /// to one replay of one toolpath, not part of the stock's public surface.
    /// The public entry point passes `None` and behaves exactly as before.
    fn stamp_linear_segment_with_mip(
        &mut self,
        lut: &RadialProfileLUT,
        radius: f64,
        start: P3,
        end: P3,
        direction: StockCutDirection,
        mip: &mut Option<tile_mip::TileMaxTop>,
    ) {
        let s = direction.decompose(start.x, start.y, start.z);
        let e = direction.decompose(end.x, end.y, end.z);
        let from_high = direction.cuts_from_high_side();
        let grid = self.ensure_grid(direction);
        stamp_segment_on_grid(grid, lut, radius, s, e, from_high, mip.as_mut());
    }

    /// Sum of material top-Z values in a circular window around (cx, cy).
    ///
    /// Iterates all Z-grid cells within `radius` of (cx, cy) and sums their
    /// top-Z values (or 0.0 if the ray is empty). This is the tri-dexel
    /// equivalent of adaptive3d's `local_material_sum()` which sums heightmap
    /// cell values in a local radius for engagement tracking.
    pub fn local_material_sum(&self, cx: f64, cy: f64, radius: f64) -> f64 {
        let grid = &self.z_grid;
        let cs = grid.cell_size;
        let r_cells = (radius / cs).ceil() as isize;

        // Convert world (cx, cy) to grid cell
        let center_col = ((cx - grid.origin_u) / cs).round() as isize;
        let center_row = ((cy - grid.origin_v) / cs).round() as isize;

        // Same clamp defect as the stamp kernels (see `clamped_cell_bbox`): a
        // disc entirely LEFT of or BELOW the grid gave a negative `center + r`
        // that `as usize` wrapped and `.min` then clamped to the last cell, so
        // the walk covered the whole grid instead of nothing. The distance test
        // rejected every cell, so the sum was right and the work was not.
        let Some((col_min, col_max, row_min, row_max)) = stamping::clamped_cell_bbox(
            center_col - r_cells,
            center_col + r_cells,
            center_row - r_cells,
            center_row + r_cells,
            grid.cols,
            grid.rows,
        ) else {
            return 0.0;
        };

        let r_sq = radius * radius;
        let mut sum = 0.0;

        for row in row_min..=row_max {
            let cell_y = grid.origin_v + row as f64 * cs;
            let dy = cell_y - cy;
            let dy_sq = dy * dy;
            if dy_sq > r_sq {
                continue;
            }
            for col in col_min..=col_max {
                let cell_x = grid.origin_u + col as f64 * cs;
                let dx = cell_x - cx;
                let dist_sq = dx * dx + dy_sq;
                if dist_sq > r_sq {
                    continue;
                }
                if let Some(top) = grid.top_z_at(row, col) {
                    sum += top as f64;
                }
            }
        }
        sum
    }

    /// Highest material top over all Z-grid cells intersecting the disc of
    /// `radius` around `(cx, cy)` (world frame). Mirrors the collision
    /// checker's view: a tool descending at this XY can touch material in
    /// any column within its radius. `None` when no intersecting column
    /// holds material.
    ///
    /// Same cell-walk and distance convention as [`Self::local_material_sum`]
    /// (max instead of sum) — see that method's comment for the shared
    /// tri-dexel-vs-heightmap correspondence.
    pub fn max_top_z_in_disc(&self, cx: f64, cy: f64, radius: f64) -> Option<f64> {
        let grid = &self.z_grid;
        let cs = grid.cell_size;
        let r_cells = (radius / cs).ceil() as isize;

        // Convert world (cx, cy) to grid cell
        let center_col = ((cx - grid.origin_u) / cs).round() as isize;
        let center_row = ((cy - grid.origin_v) / cs).round() as isize;

        // See `local_material_sum` — the same wrapping clamp, same fix.
        let (col_min, col_max, row_min, row_max) = stamping::clamped_cell_bbox(
            center_col - r_cells,
            center_col + r_cells,
            center_row - r_cells,
            center_row + r_cells,
            grid.cols,
            grid.rows,
        )?;

        let r_sq = radius * radius;
        let mut max_top: Option<f64> = None;

        for row in row_min..=row_max {
            let cell_y = grid.origin_v + row as f64 * cs;
            let dy = cell_y - cy;
            let dy_sq = dy * dy;
            if dy_sq > r_sq {
                continue;
            }
            for col in col_min..=col_max {
                let cell_x = grid.origin_u + col as f64 * cs;
                let dx = cell_x - cx;
                let dist_sq = dx * dx + dy_sq;
                if dist_sq > r_sq {
                    continue;
                }
                if let Some(top) = grid.top_z_at(row, col) {
                    let top = top as f64;
                    max_top = Some(max_top.map_or(top, |m: f64| m.max(top)));
                }
            }
        }
        max_top
    }

    /// **The reading a clearance ceiling must use.** Highest Z at which
    /// material may stand anywhere under a disc of `radius` around
    /// `(cx, cy)` — see [`crate::dexel::DexelGrid::conservative_top`].
    ///
    /// Differs from [`Self::max_top_z_in_disc`] in both of the ways that
    /// made descent planning resolution-dependent (A/M10):
    ///
    /// 1. it reads the sliver-safe bound rather than the cell-centre column,
    ///    so a rib narrower than one cell cannot be blended out of sight;
    /// 2. it visits every cell whose SQUARE overlaps the disc, not every
    ///    cell whose CENTRE lies inside it — a cell half under the tool
    ///    still holds material under the tool.
    ///
    /// Returns `None` only when the disc lies entirely outside the grid, in
    /// which case the caller has no stock information here and should fall
    /// back to the analytic fresh-stock top.
    pub fn max_conservative_top_z_in_disc(&self, cx: f64, cy: f64, radius: f64) -> Option<f64> {
        let grid = &self.z_grid;
        let cs = grid.cell_size;
        // Half a cell of dilation turns "centre inside the disc" into
        // "square overlaps the disc"; the `ceil` then rounds out to whole
        // cells. Both are deliberate over-reach — this query may only ever
        // err high.
        let reach = radius + cs * 0.5;
        let r_cells = (reach / cs).ceil() as isize;

        let center_col = ((cx - grid.origin_u) / cs).round() as isize;
        let center_row = ((cy - grid.origin_v) / cs).round() as isize;

        // This one already had the off-grid guard the other two lacked; it now
        // shares their helper so there is one place the clamp is written.
        let (col_min, col_max, row_min, row_max) = stamping::clamped_cell_bbox(
            center_col - r_cells,
            center_col + r_cells,
            center_row - r_cells,
            center_row + r_cells,
            grid.cols,
            grid.rows,
        )?;

        let reach_sq = reach * reach;
        let mut max_top: Option<f64> = None;

        for row in row_min..=row_max {
            let cell_y = grid.origin_v + row as f64 * cs;
            let dy = cell_y - cy;
            let dy_sq = dy * dy;
            if dy_sq > reach_sq {
                continue;
            }
            for col in col_min..=col_max {
                let cell_x = grid.origin_u + col as f64 * cs;
                let dx = cell_x - cx;
                if dx * dx + dy_sq > reach_sq {
                    continue;
                }
                let top = f64::from(grid.conservative_top_at(row, col));
                max_top = Some(max_top.map_or(top, |m: f64| m.max(top)));
            }
        }
        max_top
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Clear all material above `z` at the given cell on the Z-grid.
    ///
    /// After this call, no material exists above `z` at (row, col).
    /// Used for border clearing in adaptive3d where cells outside the mesh
    /// footprint are set to the surface height.
    pub fn clear_above_at(&mut self, row: usize, col: usize, z: f32) {
        let idx = row * self.z_grid.cols + col;
        let ray = &mut self.z_grid.rays[idx];
        crate::dexel::ray_subtract_above(ray, z);
        // A/M10: a whole-cell clear is exactly the case the sliver-safe
        // bound trusts — no partial coverage, no sub-cell remainder.
        self.z_grid.lower_conservative_top(idx, z);
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Clear all material **below** `z` at the given cell on the Z-grid.
    ///
    /// The mirror of [`Self::clear_above_at`], for a tool entering from the
    /// low side of the ray axis (`StockCutDirection::cuts_from_high_side()`
    /// is false). After this call, no material exists below `z` at
    /// (row, col).
    ///
    /// # Why this does not touch `conservative_top`
    ///
    /// `conservative_top` is a sliver-safe **upper** bound on material height
    /// anywhere in the cell. Subtracting from below cannot raise any ray's
    /// top, so the existing bound stays valid without being rewritten. When
    /// the subtraction empties the ray outright the bound is left loose — it
    /// still reports the pre-clear height for a cell that now holds nothing.
    /// That errs toward *more* material than is really there, which is the
    /// safe direction for every consumer of the bound (the rapid-collision
    /// scan and the air-skip test both treat a higher bound as "check this
    /// cell"), so it costs work, never safety. Lowering it to the surviving
    /// `ray_top` would be tighter but is **not** sliver-safe: that is a
    /// cell-centre sample, and the bound's contract is over the whole cell.
    pub fn clear_below_at(&mut self, row: usize, col: usize, z: f32) {
        let idx = row * self.z_grid.cols + col;
        let ray = &mut self.z_grid.rays[idx];
        crate::dexel::ray_subtract_below(ray, z);
    }

    /// Analytical drill removal — DEXEL roadmap §6.E Step 3.
    ///
    /// Bypasses per-segment stamping for drilling cycles. For each hole,
    /// walks cells inside the tool's XY footprint and clips each ray's
    /// material to the tip envelope:
    ///
    /// `z_cut(r) = bottom_z ± h(r)` where `h(r)` is the cone-tip
    /// protrusion beyond the deepest point. `ToolProfile::Flat` uses
    /// `h(r) = 0`; coned profiles use `h(r) = r / tan(half_angle)`.
    ///
    /// Idempotent and composable with prior stamping: any cell whose
    /// existing material is already clear of `z_cut(r)` is left untouched.
    /// A subsequent milling op sees the post-drill ray state because the
    /// kernel mutates the dexel in place.
    ///
    /// # `direction` is not decoration — G-DRILLFLIP (2026-08-21)
    ///
    /// A [`crate::drill_op::DrillHole`] carries **no axis**: it is an XY
    /// centre plus a `top_z`/`bottom_z` pair, which describes a hole only
    /// relative to whatever frame the caller is holding. In setup-local
    /// coordinates the tool always advances along −Z, so this kernel used to
    /// hardcode "remove everything above the tip envelope" and take no
    /// direction at all.
    ///
    /// That is wrong for the **global** stock. `group_drill_op_to_global`
    /// maps a `FaceUp::Bottom` setup's holes through `z → H − z`, which
    /// inverts them: a hole entered at local `top_z` 10 and bottomed at 4
    /// arrives as `top_z` 0, `bottom_z` 6 in a 10 mm blank. Removing above
    /// `bottom_z` then clears 6..10 — the exact **complement** of the 0..6
    /// the hole occupies. The failure has two faces, and the quiet one is
    /// worse:
    ///
    /// * A pin drill that breaks through (`bottom_z` below the far face, to
    ///   penetrate the spoilboard) maps to a `bottom_z` *above* the blank, so
    ///   the clear is a no-op and no hole appears at all. This is the visible
    ///   face — it is how the defect was found, by watching the viewport.
    /// * A blind hole maps to a `bottom_z` still inside the blank, so a
    ///   plausible-looking hole appears — in the wrong half of the stock.
    ///
    /// **Scope, corrected 2026-08-22.** The first version of this note said
    /// the global stock is what `StockSource::FromRemainingStock` reads and
    /// that a rest pass would therefore plan against fiction. **That is
    /// wrong.** Rest generation reads `prior_stocks`, which are clones of the
    /// per-setup **local** `group_stock` — and the local stock's drill removal
    /// was always correct, because setup-local Z is always the tool axis.
    /// G-DRILLFLIP was a checkpoint / playback / screenshot defect: what the
    /// operator sees, not what the next operation plans against. The claim was
    /// inherited from a stale comment on `playback_dispatch` above, repeated
    /// without checking, and both are now fixed.
    ///
    /// So the axis has to be supplied by whoever knows the frame.
    /// [`StockCutDirection::cuts_from_high_side`] is exactly that fact and
    /// already rides along both call paths (the per-setup stock is always
    /// `FromTop`; the global stock and playback both carry the group's
    /// `cut_direction()`), so it is the parameter rather than a new field on
    /// `DrillOp` — the direction belongs to the frame, not to the operation.
    ///
    /// # Lateral setups abstain
    ///
    /// `FaceUp::{Front,Back,Left,Right}` put the drill axis along global X or
    /// Y, and `DrillHole` cannot express that: `group_drill_op_to_global`
    /// keeps the mapped `top`'s XY and the mapped `bottom`'s Z, so for a
    /// lateral transform the hole's real axis is discarded before it ever
    /// reaches this kernel. Rather than carve a fabricated Z-axis hole, those
    /// directions remove nothing and report it in the returned
    /// [`DrillRemovalReport`]. Tracked as G-DRILLLATERAL; fixing it means
    /// giving `DrillHole` two 3-D endpoints, not patching this function.
    pub fn apply_drill_op(
        &mut self,
        drill_op: &crate::drill_op::DrillOp,
        direction: StockCutDirection,
    ) -> DrillRemovalReport {
        let mut report = DrillRemovalReport::default();
        if direction.grid_axis() != DexelAxis::Z {
            report.unrepresentable_axis = true;
            return report;
        }
        let from_high = direction.cuts_from_high_side();
        let radius_mm = drill_op.tool_diameter_mm * 0.5;
        if radius_mm <= 0.0 {
            return report;
        }
        let half_angle = drill_op.tool_profile.cone_half_angle_rad();
        let inv_tan = if matches!(drill_op.tool_profile, crate::drill_op::ToolProfile::Flat)
            || half_angle <= 0.0
        {
            None
        } else {
            // h(r) = r / tan(half_angle) = r * (1 / tan(α))
            Some(1.0 / half_angle.tan())
        };

        let radius_sq = radius_mm * radius_mm;
        let cell_size = self.z_grid.cell_size;
        let cell_radius = (radius_mm / cell_size).ceil() as isize + 1;

        for hole in &drill_op.holes {
            let Some((center_row, center_col)) = self.z_grid.world_to_cell(hole.xy[0], hole.xy[1])
            else {
                report.holes_off_grid += 1;
                continue;
            };
            report.holes_carved += 1;
            let center_row = center_row as isize;
            let center_col = center_col as isize;

            for dr in -cell_radius..=cell_radius {
                let row = center_row + dr;
                if row < 0 || row >= self.z_grid.rows as isize {
                    continue;
                }
                for dc in -cell_radius..=cell_radius {
                    let col = center_col + dc;
                    if col < 0 || col >= self.z_grid.cols as isize {
                        continue;
                    }
                    let (cell_x, cell_y) = self.z_grid.cell_to_world(row as usize, col as usize);
                    let dx = cell_x - hole.xy[0];
                    let dy = cell_y - hole.xy[1];
                    let r_sq = dx * dx + dy * dy;
                    if r_sq > radius_sq {
                        continue;
                    }
                    // The cone opens back toward the tool, so its
                    // protrusion is measured *against* the advance
                    // direction: above the tip when drilling down, below it
                    // when drilling up.
                    let z_cut = match inv_tan {
                        None => hole.bottom_z,
                        Some(inv_tan) => {
                            let r = r_sq.sqrt();
                            if from_high {
                                hole.bottom_z + r * inv_tan
                            } else {
                                hole.bottom_z - r * inv_tan
                            }
                        }
                    };
                    if from_high {
                        self.clear_above_at(row as usize, col as usize, z_cut as f32);
                    } else {
                        self.clear_below_at(row as usize, col as usize, z_cut as f32);
                    }
                }
            }
        }
        report
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::dexel::{ray_bottom, ray_top};
    use crate::ids::ToolpathId;
    use crate::radial_profile::RadialProfileLUT;
    use crate::tool::{BallEndmill, FlatEndmill, MillingCutter};
    use crate::toolpath::Toolpath;

    /// Helper: create a TriDexelStock with the given dimensions.
    fn make_stock(
        x_min: f64,
        y_min: f64,
        x_max: f64,
        y_max: f64,
        z_min: f64,
        z_max: f64,
        cell_size: f64,
    ) -> TriDexelStock {
        TriDexelStock::from_stock(x_min, y_min, x_max, y_max, z_min, z_max, cell_size)
    }

    // ── Basic construction ──────────────────────────────────────────────

    #[test]
    fn from_bounds_dimensions_correct() {
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, -5.0),
            max: P3::new(10.0, 10.0, 5.0),
        };
        let stock = TriDexelStock::from_bounds(&bbox, 1.0);
        // 10mm / 1mm cell + 1 = 11 rows and cols
        assert_eq!(stock.z_grid.rows, 11);
        assert_eq!(stock.z_grid.cols, 11);
    }

    // ── Single stamp equivalence ────────────────────────────────────────

    #[test]
    fn stamp_flat_endmill_cuts_correctly() {
        let tool = FlatEndmill::new(10.0, 25.0); // radius 5
        let mut stock = make_stock(-10.0, -10.0, 10.0, 10.0, 0.0, 5.0, 0.5);

        let lut = RadialProfileLUT::from_cutter(&tool, crate::radial_profile::LUT_SAMPLES);
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            0.0,
            0.0,
            2.0,
            StockCutDirection::FromTop,
        );

        // Center cell: flat endmill tip at z=2, so top should be 2.0.
        let (cr, cc) = stock.z_grid.world_to_cell(0.0, 0.0).unwrap();
        let center_z = ray_top(stock.z_grid.ray(cr, cc)).unwrap() as f64;
        assert!((center_z - 2.0).abs() < 0.01, "center z={center_z:.4}");

        // Cell outside tool radius: should still be at stock top (5.0).
        let (or, oc) = stock.z_grid.world_to_cell(-8.0, -8.0).unwrap();
        let outer_z = ray_top(stock.z_grid.ray(or, oc)).unwrap() as f64;
        assert!((outer_z - 5.0).abs() < 0.01, "outer z={outer_z:.4}");
    }

    #[test]
    fn stamp_ball_endmill_cuts_correctly() {
        let tool = BallEndmill::new(6.0, 25.0); // radius 3
        let mut stock = make_stock(-10.0, -10.0, 10.0, 10.0, 0.0, 5.0, 0.5);

        let lut = RadialProfileLUT::from_cutter(&tool, crate::radial_profile::LUT_SAMPLES);
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            0.0,
            0.0,
            1.0,
            StockCutDirection::FromTop,
        );

        // Center cell: ball tip at z=1, so top should be 1.0.
        let (cr, cc) = stock.z_grid.world_to_cell(0.0, 0.0).unwrap();
        let center_z = ray_top(stock.z_grid.ray(cr, cc)).unwrap() as f64;
        assert!((center_z - 1.0).abs() < 0.02, "center z={center_z:.4}");

        // Cell at tool radius edge (3mm away): ball profile rises to tip_z + r = 4.0,
        // but clipped to stock top 5.0, so should be near 4.0.
        let (er, ec) = stock.z_grid.world_to_cell(3.0, 0.0).unwrap();
        let edge_z = ray_top(stock.z_grid.ray(er, ec)).unwrap() as f64;
        assert!(edge_z > 3.5 && edge_z <= 5.0, "edge z={edge_z:.4}");

        // Cell outside tool radius: still at stock top.
        let (or, oc) = stock.z_grid.world_to_cell(-8.0, -8.0).unwrap();
        let outer_z = ray_top(stock.z_grid.ray(or, oc)).unwrap() as f64;
        assert!((outer_z - 5.0).abs() < 0.01, "outer z={outer_z:.4}");
    }

    // ── Linear segment equivalence ──────────────────────────────────────

    #[test]
    fn linear_segment_flat_cuts_correctly() {
        let tool = FlatEndmill::new(4.0, 20.0); // radius 2
        let mut stock = make_stock(-5.0, -5.0, 15.0, 5.0, 0.0, 5.0, 0.5);

        let start = P3::new(0.0, 0.0, 2.0);
        let end = P3::new(10.0, 0.0, 2.0);

        let lut = RadialProfileLUT::from_cutter(&tool, crate::radial_profile::LUT_SAMPLES);
        stock.stamp_linear_segment(&lut, tool.radius(), start, end, StockCutDirection::FromTop);

        // Along the path center (y=0): z should be at tip_z = 2.0.
        for x in [0.0, 5.0, 10.0] {
            let (r, c) = stock.z_grid.world_to_cell(x, 0.0).unwrap();
            let z = ray_top(stock.z_grid.ray(r, c)).unwrap() as f64;
            assert!((z - 2.0).abs() < 0.02, "x={x} z={z:.4}");
        }

        // Outside tool radius (y=4): should still be stock top (5.0).
        let (r, c) = stock.z_grid.world_to_cell(5.0, 4.0).unwrap();
        let z = ray_top(stock.z_grid.ray(r, c)).unwrap() as f64;
        assert!((z - 5.0).abs() < 0.01, "outside z={z:.4}");
    }

    #[test]
    fn linear_segment_ball_diagonal_cuts_correctly() {
        let tool = BallEndmill::new(6.0, 25.0); // radius 3
        let mut stock = make_stock(0.0, 0.0, 30.0, 30.0, -5.0, 5.0, 0.25);

        let start = P3::new(5.0, 5.0, -1.0);
        let end = P3::new(25.0, 25.0, -1.0);

        let lut = RadialProfileLUT::from_cutter(&tool, crate::radial_profile::LUT_SAMPLES);
        stock.stamp_linear_segment(&lut, tool.radius(), start, end, StockCutDirection::FromTop);

        // Midpoint of the diagonal (15,15): ball tip at z=-1, so center z = -1.0.
        let (r, c) = stock.z_grid.world_to_cell(15.0, 15.0).unwrap();
        let z = ray_top(stock.z_grid.ray(r, c)).unwrap() as f64;
        assert!((z - (-1.0)).abs() < 0.02, "midpoint z={z:.4}");

        // Far from the path (0,0): should still be stock top (5.0).
        let (r, c) = stock.z_grid.world_to_cell(0.0, 0.0).unwrap();
        let z = ray_top(stock.z_grid.ray(r, c)).unwrap() as f64;
        assert!((z - 5.0).abs() < 0.01, "corner z={z:.4}");
    }

    // ── Toolpath simulation equivalence ─────────────────────────────────

    #[test]
    fn simulate_toolpath_cuts_correctly() {
        let tool = FlatEndmill::new(4.0, 20.0); // radius 2
        let mut stock = make_stock(-5.0, -5.0, 15.0, 5.0, -5.0, 0.0, 0.5);

        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(0.0, 0.0, -3.0), 500.0);
        tp.feed_to(P3::new(10.0, 0.0, -3.0), 1000.0);

        stock.simulate_toolpath(&tp, &tool, StockCutDirection::FromTop);

        // Along the path (y=0): plunge to z=-3, then cut at z=-3 to x=10.
        for x in [0.0, 5.0, 10.0] {
            let (r, c) = stock.z_grid.world_to_cell(x, 0.0).unwrap();
            let z = ray_top(stock.z_grid.ray(r, c)).unwrap() as f64;
            assert!((z - (-3.0)).abs() < 0.02, "x={x} z={z:.4}");
        }

        // Far from the path: should still be stock top (0.0).
        let (r, c) = stock.z_grid.world_to_cell(-4.0, -4.0).unwrap();
        let z = ray_top(stock.z_grid.ray(r, c)).unwrap() as f64;
        assert!((z - 0.0).abs() < 0.01, "outside z={z:.4}");
    }

    // ── Bottom cuts ─────────────────────────────────────────────────────

    #[test]
    fn bottom_cut_removes_from_below() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut stock = TriDexelStock::from_stock(-10.0, -10.0, 10.0, 10.0, 0.0, 10.0, 0.5);

        let lut = RadialProfileLUT::from_cutter(&tool, crate::radial_profile::LUT_SAMPLES);
        // Tip at z=3 from below: flat endmill surface at z=3, remove below.
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            0.0,
            0.0,
            3.0,
            StockCutDirection::FromBottom,
        );

        let ray = stock.z_grid.ray(
            stock.z_grid.world_to_cell(0.0, 0.0).unwrap().0,
            stock.z_grid.world_to_cell(0.0, 0.0).unwrap().1,
        );
        // Bottom should now be at 3.0, top still at 10.0.
        assert!((ray_bottom(ray).unwrap() - 3.0).abs() < 0.01);
        assert!((ray_top(ray).unwrap() - 10.0).abs() < 0.01);
    }

    #[test]
    fn top_then_bottom_cut() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut stock = TriDexelStock::from_stock(-10.0, -10.0, 10.0, 10.0, 0.0, 10.0, 0.5);

        let lut = RadialProfileLUT::from_cutter(&tool, crate::radial_profile::LUT_SAMPLES);
        // Top cut: remove above z=7
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            0.0,
            0.0,
            7.0,
            StockCutDirection::FromTop,
        );
        // Bottom cut: remove below z=3
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            0.0,
            0.0,
            3.0,
            StockCutDirection::FromBottom,
        );

        let (r, c) = stock.z_grid.world_to_cell(0.0, 0.0).unwrap();
        let ray = stock.z_grid.ray(r, c);
        assert!((ray_bottom(ray).unwrap() - 3.0).abs() < 0.01);
        assert!((ray_top(ray).unwrap() - 7.0).abs() < 0.01);
    }

    // ── Rapids don't cut ────────────────────────────────────────────────

    #[test]
    fn rapids_dont_cut() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut stock = TriDexelStock::from_stock(-10.0, -10.0, 10.0, 10.0, 0.0, 5.0, 1.0);

        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.rapid_to(P3::new(5.0, 5.0, 0.0));

        stock.simulate_toolpath(&tp, &tool, StockCutDirection::FromTop);

        // All rays should still have full stock.
        for ray in &stock.z_grid.rays {
            assert_eq!(ray.len(), 1);
            assert!((ray[0].exit - 5.0).abs() < 1e-4);
        }
    }

    // ── Range simulation ────────────────────────────────────────────────

    #[test]
    fn simulate_range_partial() {
        let tool = FlatEndmill::new(4.0, 20.0);
        let mut stock = TriDexelStock::from_stock(-5.0, -5.0, 15.0, 5.0, -5.0, 0.0, 0.5);

        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(0.0, 0.0, -3.0), 500.0);
        tp.feed_to(P3::new(5.0, 0.0, -3.0), 1000.0);
        tp.feed_to(P3::new(10.0, 0.0, -3.0), 1000.0);

        // Simulate only the first two cutting moves.
        stock.simulate_toolpath_range(&tp, &tool, StockCutDirection::FromTop, 0, 3);

        // x=2.5 should be cut (in first segment)
        let (r, c) = stock.z_grid.world_to_cell(2.5, 0.0).unwrap();
        assert!(ray_top(stock.z_grid.ray(r, c)).unwrap() < 0.0);

        // x=7.5 should be uncut (in third segment, not simulated)
        let (r, c) = stock.z_grid.world_to_cell(7.5, 0.0).unwrap();
        assert!((ray_top(stock.z_grid.ray(r, c)).unwrap() - 0.0).abs() < 1e-4);
    }

    // ── Checkpoint ──────────────────────────────────────────────────────

    #[test]
    fn checkpoint_is_independent_copy() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut stock = TriDexelStock::from_stock(-10.0, -10.0, 10.0, 10.0, 0.0, 5.0, 0.5);

        let saved = stock.checkpoint();

        // Cut the original.
        let lut = RadialProfileLUT::from_cutter(&tool, crate::radial_profile::LUT_SAMPLES);
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            0.0,
            0.0,
            2.0,
            StockCutDirection::FromTop,
        );

        // Saved should still be at stock top.
        let (r, c) = saved.z_grid.world_to_cell(0.0, 0.0).unwrap();
        assert!((ray_top(saved.z_grid.ray(r, c)).unwrap() - 5.0).abs() < 1e-4);
    }

    // ── Phase 6: Side-face grid tests ──────────────────────────────────

    #[test]
    fn stamp_from_back_creates_y_grid_and_cuts() {
        let tool = FlatEndmill::new(10.0, 25.0); // radius 5
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, 0.0, 20.0, 1.0);

        assert!(stock.y_grid.is_none(), "Y-grid should not exist yet");

        let lut = RadialProfileLUT::from_cutter(&tool, crate::radial_profile::LUT_SAMPLES);
        // Stamp from back (+Y side): tool center at global (10, ?, 10)
        // decompose for Y-grid: u=x=10, v=z=10, depth=y
        // FromBack = subtract_above (high-Y side), tip_y = 15
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            10.0,
            15.0, // global Y: tool tip Y
            10.0, // global Z
            StockCutDirection::FromBack,
        );

        assert!(stock.y_grid.is_some(), "Y-grid should be lazily created");
        let y_grid = stock.y_grid.as_ref().unwrap();

        // Y-grid: u=X, v=Z. Cell at (x=10, z=10) should be shortened from above.
        let (row, col) = y_grid.world_to_cell(10.0, 10.0).unwrap();
        let ray = y_grid.ray(row, col);
        // Original ray: [0, 20]. After subtract_above at y=15+0 (flat endmill h=0 at center),
        // ray should be [0, 15].
        assert!(
            ray_top(ray).unwrap() < 20.0,
            "Y-grid ray should be shortened"
        );
        assert!((ray_top(ray).unwrap() - 15.0).abs() < 0.1);

        // Z-grid should be untouched.
        let (zr, zc) = stock.z_grid.world_to_cell(10.0, 10.0).unwrap();
        assert!((ray_top(stock.z_grid.ray(zr, zc)).unwrap() - 20.0).abs() < 0.01);
    }

    #[test]
    fn stamp_from_front_creates_y_grid_and_cuts() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, 0.0, 20.0, 1.0);

        let lut = RadialProfileLUT::from_cutter(&tool, crate::radial_profile::LUT_SAMPLES);
        // FromFront: tool enters from -Y (low Y). subtract_below.
        // Tool tip at global y=5, center at (10, 5, 10).
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            10.0,
            5.0,  // global Y: tool tip
            10.0, // global Z
            StockCutDirection::FromFront,
        );

        let y_grid = stock.y_grid.as_ref().unwrap();
        let (row, col) = y_grid.world_to_cell(10.0, 10.0).unwrap();
        let ray = y_grid.ray(row, col);
        // subtract_below at y=5: ray bottom should be at 5.
        assert!((ray_bottom(ray).unwrap() - 5.0).abs() < 0.1);
        assert!((ray_top(ray).unwrap() - 20.0).abs() < 0.01); // top unchanged
    }

    #[test]
    fn stamp_from_left_creates_x_grid_and_cuts() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, 0.0, 20.0, 1.0);

        assert!(stock.x_grid.is_none());

        let lut = RadialProfileLUT::from_cutter(&tool, crate::radial_profile::LUT_SAMPLES);
        // FromLeft: tool enters from -X. subtract_below on X-grid.
        // decompose: u=Y, v=Z, depth=X. Tool at global (5, 10, 10).
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            5.0,  // global X: tool tip
            10.0, // global Y
            10.0, // global Z
            StockCutDirection::FromLeft,
        );

        assert!(stock.x_grid.is_some(), "X-grid should be lazily created");
        let x_grid = stock.x_grid.as_ref().unwrap();

        // X-grid: u=Y, v=Z. Cell at (y=10, z=10).
        let (row, col) = x_grid.world_to_cell(10.0, 10.0).unwrap();
        let ray = x_grid.ray(row, col);
        // subtract_below at x=5: ray bottom at 5, top at 20.
        assert!((ray_bottom(ray).unwrap() - 5.0).abs() < 0.1);
        assert!((ray_top(ray).unwrap() - 20.0).abs() < 0.01);
    }

    #[test]
    fn stamp_from_right_creates_x_grid_and_cuts() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, 0.0, 20.0, 1.0);

        let lut = RadialProfileLUT::from_cutter(&tool, crate::radial_profile::LUT_SAMPLES);
        // FromRight: tool enters from +X. subtract_above on X-grid.
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            15.0, // global X: tool tip
            10.0, // global Y
            10.0, // global Z
            StockCutDirection::FromRight,
        );

        let x_grid = stock.x_grid.as_ref().unwrap();
        let (row, col) = x_grid.world_to_cell(10.0, 10.0).unwrap();
        let ray = x_grid.ray(row, col);
        // subtract_above at x=15: ray top at 15, bottom at 0.
        assert!((ray_top(ray).unwrap() - 15.0).abs() < 0.1);
        assert!((ray_bottom(ray).unwrap() - 0.0).abs() < 0.01);
    }

    #[test]
    fn linear_segment_on_y_grid() {
        let tool = FlatEndmill::new(4.0, 20.0); // radius 2
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, 0.0, 20.0, 0.5);

        let lut = RadialProfileLUT::from_cutter(&tool, crate::radial_profile::LUT_SAMPLES);
        // Sweep along X at global (x, y=15, z=10) from x=2 to x=18.
        // FromBack stamps on Y-grid. decompose: u=x, v=z, depth=y
        let start = P3::new(2.0, 15.0, 10.0);
        let end = P3::new(18.0, 15.0, 10.0);
        stock.stamp_linear_segment(&lut, tool.radius(), start, end, StockCutDirection::FromBack);

        let y_grid = stock.y_grid.as_ref().unwrap();
        // Check a cell along the swept path: (x=10, z=10).
        let (row, col) = y_grid.world_to_cell(10.0, 10.0).unwrap();
        let ray = y_grid.ray(row, col);
        assert!(ray_top(ray).unwrap() < 20.0, "Y-grid ray should be cut");
        assert!((ray_top(ray).unwrap() - 15.0).abs() < 0.1);
    }

    #[test]
    fn multi_grid_simulation_preserves_z_grid() {
        // Simulate a Top setup, then a Front setup.
        // The Z-grid cuts from setup 1 should be unaffected by setup 2.
        let tool = FlatEndmill::new(6.0, 20.0); // radius 3

        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 30.0, 30.0, 0.0, 20.0, 1.0);

        // Setup 1: Top cut — stamp at center.
        let lut = RadialProfileLUT::from_cutter(&tool, crate::radial_profile::LUT_SAMPLES);
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            15.0,
            15.0,
            12.0, // cut Z-grid to z=12
            StockCutDirection::FromTop,
        );

        let (zr, zc) = stock.z_grid.world_to_cell(15.0, 15.0).unwrap();
        let z_top_before = ray_top(stock.z_grid.ray(zr, zc)).unwrap();
        assert!((z_top_before - 12.0).abs() < 0.1);

        // Setup 2: FromBack cut — stamp on Y-grid.
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            15.0,
            25.0, // global Y (depth for Y-grid)
            10.0, // global Z
            StockCutDirection::FromBack,
        );

        // Z-grid should be unchanged.
        let z_top_after = ray_top(stock.z_grid.ray(zr, zc)).unwrap();
        assert!(
            (z_top_after - z_top_before).abs() < 1e-6,
            "Z-grid should not be affected by Y-grid stamping"
        );

        // Y-grid should have cuts.
        let y_grid = stock.y_grid.as_ref().unwrap();
        let (yr, yc) = y_grid.world_to_cell(15.0, 10.0).unwrap();
        assert!(ray_top(y_grid.ray(yr, yc)).unwrap() < 30.0);
    }

    #[test]
    fn checkpoint_preserves_side_grids() {
        let tool = FlatEndmill::new(10.0, 25.0);
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, 0.0, 20.0, 1.0);

        let lut = RadialProfileLUT::from_cutter(&tool, crate::radial_profile::LUT_SAMPLES);
        // Create Y-grid via stamp.
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            10.0,
            15.0,
            10.0,
            StockCutDirection::FromBack,
        );

        let saved = stock.checkpoint();

        // Cut more on the original.
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            10.0,
            10.0,
            10.0,
            StockCutDirection::FromBack,
        );

        // Saved Y-grid should be unaffected by the second cut.
        let saved_y = saved.y_grid.as_ref().unwrap();
        let (row, col) = saved_y.world_to_cell(10.0, 10.0).unwrap();
        assert!(
            (ray_top(saved_y.ray(row, col)).unwrap() - 15.0).abs() < 0.1,
            "Checkpoint Y-grid should reflect only the first cut"
        );
    }

    #[test]
    fn simulate_toolpath_on_y_grid() {
        let tool = FlatEndmill::new(4.0, 20.0);
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 20.0, 20.0, 0.0, 20.0, 0.5);

        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(5.0, 25.0, 10.0)); // approach from outside
        tp.feed_to(P3::new(5.0, 15.0, 10.0), 1000.0); // plunge into stock
        tp.feed_to(P3::new(15.0, 15.0, 10.0), 1000.0); // cut along X

        stock.simulate_toolpath(&tp, &tool, StockCutDirection::FromBack);

        assert!(stock.y_grid.is_some());
        let y_grid = stock.y_grid.as_ref().unwrap();
        // Cell at (x=10, z=10) should be cut.
        let (row, col) = y_grid.world_to_cell(10.0, 10.0).unwrap();
        assert!(ray_top(y_grid.ray(row, col)).unwrap() < 20.0);
    }

    // ── Query helper tests ─────────────────────────────────────────────

    #[test]
    fn test_local_material_sum() {
        // 10x10 stock, z 0..5, cell_size=1. All cells have top=5.
        let stock = TriDexelStock::from_stock(0.0, 0.0, 10.0, 10.0, 0.0, 5.0, 1.0);

        // Sum in a radius of 1.5 around the center (5, 5).
        // Cells within radius 1.5 of (5,5): the center plus 4 axis-neighbors
        // plus 4 diagonal neighbors (dist = sqrt(2) ~= 1.414 < 1.5).
        // That's 9 cells, each with top=5 => sum = 45.
        let sum = stock.local_material_sum(5.0, 5.0, 1.5);
        assert!((sum - 45.0).abs() < 1e-6, "Expected 45.0, got {sum}");
    }

    #[test]
    fn test_local_material_sum_after_stamp() {
        let tool = FlatEndmill::new(4.0, 20.0); // radius 2
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 10.0, 10.0, 0.0, 5.0, 1.0);

        // Before stamping: sum around center should reflect full stock.
        let sum_before = stock.local_material_sum(5.0, 5.0, 3.0);

        // Stamp tool at center, cutting to z=2.
        let lut = RadialProfileLUT::from_cutter(&tool, crate::radial_profile::LUT_SAMPLES);
        stock.stamp_tool_at(
            &lut,
            tool.radius(),
            5.0,
            5.0,
            2.0,
            StockCutDirection::FromTop,
        );

        let sum_after = stock.local_material_sum(5.0, 5.0, 3.0);

        // The stamp removed material, so sum should decrease.
        assert!(
            sum_after < sum_before,
            "Sum should decrease after stamp: before={sum_before}, after={sum_after}"
        );
    }

    #[test]
    fn test_max_top_z_in_disc_stepped_stock() {
        // 10x10 stock, z 0..10, cell_size=1. Step: the right half
        // (col >= cols/2) is lowered to top=3; the left half stays at
        // top=10 (fresh, unstamped).
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 10.0, 10.0, 0.0, 10.0, 1.0);
        let rows = stock.z_grid.rows;
        let cols = stock.z_grid.cols;
        for row in 0..rows {
            for col in (cols / 2)..cols {
                stock.clear_above_at(row, col, 3.0);
            }
        }

        // Querying at the midpoint between the last tall column and the
        // first lowered column, with a disc wide enough to reach both,
        // must report the TALL side's top, not the local (lowered)
        // column's top — mirroring the collision checker's view that a
        // tool can touch material anywhere within its footprint, not just
        // at its center.
        let cs = stock.z_grid.cell_size;
        let last_tall_col = cols / 2 - 1;
        let midpoint_x = stock.z_grid.origin_u + (last_tall_col as f64 + 0.5) * cs;
        let at_boundary = stock.max_top_z_in_disc(midpoint_x, 5.0, 0.6);
        assert!(
            (at_boundary.unwrap_or(0.0) - 10.0).abs() < 1e-6,
            "expected the disc straddling the step to see the tall side's top (10.0), got {at_boundary:?}"
        );

        // Deep inside the lowered side (disc doesn't reach the step),
        // the query should report the lowered top.
        let deep_low = stock.max_top_z_in_disc(9.0, 5.0, 0.4);
        assert!(
            (deep_low.unwrap_or(0.0) - 3.0).abs() < 1e-6,
            "expected the lowered side's top (3.0), got {deep_low:?}"
        );

        // Deep inside the tall side, the query should report the tall top.
        let deep_tall = stock.max_top_z_in_disc(1.0, 5.0, 0.4);
        assert!(
            (deep_tall.unwrap_or(0.0) - 10.0).abs() < 1e-6,
            "expected the tall side's top (10.0), got {deep_tall:?}"
        );
    }

    #[test]
    fn test_clear_above_at() {
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 4.0, 4.0, 0.0, 10.0, 1.0);

        // Clear above z=6 at cell (2, 2).
        stock.clear_above_at(2, 2, 6.0);

        let ray = stock.z_grid.ray(2, 2);
        assert_eq!(ray_top(ray), Some(6.0));

        // Neighboring cell should be untouched.
        let neighbor = stock.z_grid.ray(2, 1);
        assert_eq!(ray_top(neighbor), Some(10.0));
    }

    #[test]
    fn test_clear_above_at_empty_ray() {
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 4.0, 4.0, 0.0, 10.0, 1.0);

        // Clear the ray entirely first.
        stock.z_grid.ray_mut(1, 1).clear();
        assert!(stock.z_grid.ray(1, 1).is_empty());

        // clear_above_at on an empty ray should not panic.
        stock.clear_above_at(1, 1, 5.0);
        assert!(stock.z_grid.ray(1, 1).is_empty());
    }

    #[test]
    fn test_fused_metrics_positive_values() {
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 50.0, 20.0, 0.0, 10.0, 0.5);
        let flat = FlatEndmill::new(6.35, 25.0);
        let never_cancel = || false;
        let mut tp = crate::toolpath::Toolpath::new();
        tp.rapid_to(P3::new(5.0, 10.0, 15.0));
        tp.feed_to(P3::new(5.0, 10.0, -2.0), 500.0);
        tp.feed_to(P3::new(40.0, 10.0, -2.0), 1000.0);

        let samples = stock
            .simulate_toolpath_with_metrics_with_cancel(
                &tp,
                &flat,
                StockCutDirection::FromTop,
                ToolpathId(0),
                18000,
                2,
                5000.0,
                1.0,
                None,
                &[],
                &[],
                true,
                &never_cancel,
            )
            .unwrap();

        let cutting: Vec<_> = samples.iter().filter(|s| s.is_cutting).collect();
        assert!(!cutting.is_empty(), "should have cutting samples");
        for s in &cutting {
            assert!(s.axial_doc_mm >= 0.0, "axial_doc must be non-negative");
            assert!(
                s.engagement.radial_woc_fraction >= 0.0,
                "engagement must be non-negative"
            );
            assert!(
                s.removed_volume_est_mm3 >= 0.0,
                "volume must be non-negative"
            );
        }
        // At least the first cutting move should have positive engagement.
        let first_cut = cutting.iter().find(|s| s.removed_volume_est_mm3 > 0.0);
        assert!(
            first_cut.is_some(),
            "should have at least one sample with material removal"
        );
    }

    #[test]
    fn test_fused_metrics_ball_endmill() {
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 30.0, 10.0, 0.0, 10.0, 0.5);
        let ball = BallEndmill::new(6.0, 25.0);
        let never_cancel = || false;
        let mut tp = crate::toolpath::Toolpath::new();
        tp.rapid_to(P3::new(3.0, 5.0, 15.0));
        tp.feed_to(P3::new(3.0, 5.0, -2.0), 500.0);
        tp.feed_to(P3::new(25.0, 5.0, -2.0), 1000.0);

        let samples = stock
            .simulate_toolpath_with_metrics_with_cancel(
                &tp,
                &ball,
                StockCutDirection::FromTop,
                ToolpathId(0),
                18000,
                2,
                5000.0,
                1.0,
                None,
                &[],
                &[],
                true,
                &never_cancel,
            )
            .unwrap();

        // Should have some cutting samples with positive metrics.
        let has_material_removal = samples.iter().any(|s| s.removed_volume_est_mm3 > 0.0);
        assert!(has_material_removal, "ball endmill should remove material");
    }

    #[test]
    fn test_fused_metrics_degenerate_segment() {
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 10.0, 10.0, 0.0, 10.0, 1.0);
        let flat = FlatEndmill::new(6.0, 25.0);
        let never_cancel = || false;

        // Toolpath with zero-length cutting move (same start and end).
        let mut tp = crate::toolpath::Toolpath::new();
        tp.rapid_to(P3::new(5.0, 5.0, 15.0));
        tp.feed_to(P3::new(5.0, 5.0, -2.0), 500.0);
        tp.feed_to(P3::new(5.0, 5.0, -2.0), 1000.0); // zero-length

        // Should not panic.
        let samples = stock
            .simulate_toolpath_with_metrics_with_cancel(
                &tp,
                &flat,
                StockCutDirection::FromTop,
                ToolpathId(0),
                18000,
                2,
                5000.0,
                1.0,
                None,
                &[],
                &[],
                true,
                &never_cancel,
            )
            .unwrap();

        for s in &samples {
            assert!(s.removed_volume_est_mm3 >= 0.0);
        }
    }
}
