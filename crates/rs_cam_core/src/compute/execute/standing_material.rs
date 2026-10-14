//! G-RESTLEVELS: which part of a 2.5D outline still holds material at each
//! Z level of the ladder.
//!
//! A 2.5D operation with `StockSource::FromRemainingStock` receives the
//! machined stock as `ExecutionContext::initial_stock`. Before this module,
//! Pocket planned every level from `top_z` down and cut air where an
//! upstream operation had already cleared the material (op Z23 re-cut 112
//! levels that op Z22 had cleared; 93.5 % of its cut time was air).
//!
//! [`regions_per_level`] tests each level against the stock and gives one of
//! three answers:
//!
//! - [`LevelRegion::Whole`]: cut the whole outline, as a fresh operation does;
//! - [`LevelRegion::Empty`]: no material in the reach of the tool, skip the level;
//! - [`LevelRegion::Clipped`]: cut only the outline ∩ (material ⊕ reach margin).
//!
//! The Layered runner (`layered.rs`, `RestUse::LevelClip`) calls it and
//! clips its INPUT polygon, never the emitted path. The design
//! ladder stays as it is: a skipped level is removed, and no level moves.
//!
//! # The geometry
//!
//! - A cell counts only when its centre is inside the FOOTPRINT of the tool
//!   in the outline, `(outline ⊖ R) ⊕ R`. Material outside the footprint
//!   (the corner fillets an earlier cutter of the same radius left, the
//!   stock outside the wall) is material this operation cannot reach in
//!   any case. The footprint is shrunk by half a cell, because the
//!   simulator blends a cell that a tool covers only in part, and a cell
//!   whose centre is that close to the edge of a cut keeps part of its top.
//! - Material counts when it stands above `z + FRESH_MATERIAL_THRESHOLD_MM`,
//!   the floor below which the simulator itself calls material absent.
//! - The material mask is grown by `2R + cell`. The operation insets its
//!   input by `R` again (tool compensation), so the tool centre then covers
//!   every position whose disc comes within one cell of the material. A
//!   margin of `R + cell` is too small: a sliver along the wall that is
//!   thinner than `R - cell` then gives a region the tool cannot enter.
//!
//! # Fallbacks
//!
//! Each fallback gives `Whole` at every level, which cuts more, never less:
//! a stock box that does not overlap the outline (a frame mismatch), a
//! region extraction that hits `MAX_REST_REGIONS`, and a window too large
//! for the grid. The finding records which one.

use crate::compute::toolpath_stats::{RestLevelsFallback, RestLevelsFinding};
use crate::dexel_stock::{FRESH_MATERIAL_THRESHOLD_MM, TriDexelStock};
use crate::geometry::grid2::Grid2;
use crate::geometry::region_mask::region_polygons_from_mask_reported;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::polygon::{Polygon2, offset_polygon};
use crate::stock::dexel::LatticeWindow;

/// What one Z level of the ladder cuts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LevelRegion {
    /// The whole outline, as a fresh operation cuts it.
    Whole,
    /// Nothing: no material stands in the reach of the tool.
    Empty,
    /// The outline ∩ the material region `LevelPlan::regions[i]`.
    Clipped(usize),
}

/// The answer of [`regions_per_level`] for one outline.
#[derive(Debug, Clone)]
pub(crate) struct LevelPlan {
    /// One entry per level of the ladder, in ladder order.
    pub levels: Vec<LevelRegion>,
    /// The distinct clipped regions. Each is already intersected with the
    /// outline. Levels with an equal mask share one entry.
    pub regions: Vec<Vec<Polygon2>>,
    /// `None` when the operation read no machined stock.
    pub finding: Option<RestLevelsFinding>,
}

impl LevelPlan {
    /// `Whole` at every level. `finding` is `None` when the operation read
    /// no machined stock.
    pub(super) fn whole(level_count: usize, finding: Option<RestLevelsFinding>) -> Self {
        Self {
            levels: vec![LevelRegion::Whole; level_count],
            regions: Vec::new(),
            finding,
        }
    }

    fn fallback(level_count: usize, fallback: RestLevelsFallback) -> Self {
        Self::whole(
            level_count,
            Some(RestLevelsFinding {
                planned_levels: level_count,
                skipped_levels: 0,
                clipped_levels: 0,
                fallback: Some(fallback),
            }),
        )
    }

    /// True when at least one level cuts the whole outline.
    pub(crate) fn needs_whole(&self) -> bool {
        self.levels.contains(&LevelRegion::Whole)
    }
}

/// The largest window, per axis, as a multiple of the grid span plus its
/// pad. An outline more than twice the span of the stock on an axis is not
/// a rest pocket on this stock; the clip then falls back to `Whole`.
const WINDOW_SPAN_OVER_GRID: usize = 2;

/// G-RESTLEVELS: the region each level of `levels` cuts in `outline`, read
/// from the machined stock.
///
/// `stock == None` (a fresh operation) gives `Whole` at every level and no
/// finding, so a fresh toolpath stays byte-identical. `levels` and `outline`
/// must be in the frame of the stock (the emission frame).
///
/// # Errors
///
/// [`Cancelled`] when `cancel` fires. The function polls once per level.
pub(crate) fn regions_per_level(
    stock: Option<&TriDexelStock>,
    outline: &Polygon2,
    levels: &[f64],
    tool_radius: f64,
    cancel: &dyn CancelCheck,
) -> Result<LevelPlan, Cancelled> {
    let Some(stock) = stock else {
        return Ok(LevelPlan::whole(levels.len(), None));
    };
    let [ox_min, oy_min, ox_max, oy_max] = outline.bbox();
    let sb = &stock.stock_bbox;
    let overlaps =
        sb.min.x <= ox_max && sb.max.x >= ox_min && sb.min.y <= oy_max && sb.max.y >= oy_min;
    if !overlaps {
        return Ok(LevelPlan::fallback(
            levels.len(),
            RestLevelsFallback::StockOffOutline,
        ));
    }

    let grid = &stock.z_grid;
    let cell = grid.cell_size;
    let dilate_mm = 2.0 * tool_radius + cell;
    // Two cells more than the dilation, so no grown region touches the
    // window edge (marching squares drops a loop that touches it).
    let pad_cells = (dilate_mm / cell).ceil() as usize + 2;
    let window = grid.lattice_window([ox_min, oy_min, ox_max, oy_max], pad_cells);
    let limit = (WINDOW_SPAN_OVER_GRID * (grid.rows + 2 * pad_cells))
        * (WINDOW_SPAN_OVER_GRID * (grid.cols + 2 * pad_cells));
    if window.cell_count() > limit {
        return Ok(LevelPlan::fallback(
            levels.len(),
            RestLevelsFallback::WindowTooLarge {
                cells: window.cell_count(),
                limit,
            },
        ));
    }

    let reach = footprint_mask(stock, &window, outline, tool_radius);
    let reach_count = reach.iter().filter(|&&r| r).count();
    if reach_count == 0 {
        // The footprint holds no cell centre: the grid cannot see this
        // outline. Cut it whole rather than skip a level on no evidence.
        return Ok(LevelPlan::whole(
            levels.len(),
            Some(RestLevelsFinding {
                planned_levels: levels.len(),
                skipped_levels: 0,
                clipped_levels: 0,
                fallback: None,
            }),
        ));
    }

    let (origin_x, origin_y) = grid.window_cell_centre(&window, 0, 0);
    let outline_area = outline.area();
    let mut plan = LevelPlan {
        levels: Vec::with_capacity(levels.len()),
        regions: Vec::new(),
        finding: None,
    };
    let mut previous: Option<(Vec<bool>, LevelRegion)> = None;
    for &z in levels {
        check_cancel(cancel)?;
        let floor = (z + FRESH_MATERIAL_THRESHOLD_MM) as f32;
        let mut mask = grid.material_above_mask(&window, floor);
        for (m, &r) in mask.iter_mut().zip(reach.iter()) {
            *m = *m && r;
        }
        // On a descending ladder the mask can only grow, so equal masks are
        // neighbours: a comparison with the previous level is the cache.
        if let Some((prev_mask, prev_region)) = &previous
            && *prev_mask == mask
        {
            plan.levels.push(*prev_region);
            continue;
        }
        let count = mask.iter().filter(|&&m| m).count();
        let region = if count == 0 {
            LevelRegion::Empty
        } else if count == reach_count {
            LevelRegion::Whole
        } else {
            let Ok(grid_mask) = Grid2::from_vec(window.cols, window.rows, mask.clone()) else {
                // The mask has `rows * cols` cells by construction. If the
                // shape is wrong all the same, cut the whole outline.
                plan.levels.push(LevelRegion::Whole);
                continue;
            };
            let extraction = region_polygons_from_mask_reported(
                &grid_mask, origin_x, origin_y, cell, dilate_mm, None,
            );
            if extraction.cap.truncated() {
                return Ok(LevelPlan::fallback(
                    levels.len(),
                    RestLevelsFallback::RegionCap {
                        total_before_cap: extraction.cap.total_before_cap,
                    },
                ));
            }
            let pieces: Vec<Polygon2> = extraction
                .polygons
                .iter()
                .flat_map(|region| outline.intersection(region))
                .collect();
            let area: f64 = pieces.iter().map(Polygon2::area).sum();
            if pieces.is_empty() {
                LevelRegion::Empty
            } else if area >= outline_area - cell * cell {
                LevelRegion::Whole
            } else {
                plan.regions.push(pieces);
                LevelRegion::Clipped(plan.regions.len() - 1)
            }
        };
        plan.levels.push(region);
        previous = Some((mask, region));
    }
    plan.finding = Some(RestLevelsFinding {
        planned_levels: levels.len(),
        skipped_levels: plan
            .levels
            .iter()
            .filter(|l| **l == LevelRegion::Empty)
            .count(),
        clipped_levels: plan
            .levels
            .iter()
            .filter(|l| matches!(l, LevelRegion::Clipped(_)))
            .count(),
        fallback: None,
    });
    Ok(plan)
}

/// The window cells whose centre lies inside the footprint of the tool in
/// `outline`, `(outline ⊖ R) ⊕ (R - cell / 2)`. Row-major, like the
/// material mask.
fn footprint_mask(
    stock: &TriDexelStock,
    window: &LatticeWindow,
    outline: &Polygon2,
    tool_radius: f64,
) -> Vec<bool> {
    let grid = &stock.z_grid;
    let grow = (tool_radius - 0.5 * grid.cell_size).max(0.0);
    // A negative distance grows the polygon (`offset_polygon` sign rule).
    let footprint: Vec<Polygon2> = offset_polygon(outline, tool_radius)
        .iter()
        .flat_map(|core| offset_polygon(core, -grow))
        .collect();
    let mut mask = Vec::with_capacity(window.cell_count());
    for r in 0..window.rows {
        for c in 0..window.cols {
            let (x, y) = grid.window_cell_centre(window, r, c);
            let p = crate::geo::P2::new(x, y);
            mask.push(footprint.iter().any(|f| f.contains_point(&p)));
        }
    }
    mask
}
