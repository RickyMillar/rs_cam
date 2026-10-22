//! G-RESTLEVELS: which part of a 2.5D outline still holds material at each
//! Z level of the ladder.
//!
//! A 2.5D operation with `StockSource::FromRemainingStock` receives the
//! machined stock on `ExecutionContext::territory_inputs`. Before this module,
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
//! W3.4 adds two more reads of the same stock:
//!
//! - [`levels_in_reach`] (`RestUse::LevelSkip`, Profile and Trace): a
//!   level is `Whole` or `Empty`, never clipped;
//! - [`column_top`] (`RestUse::StockColumn`, Drill, D8): the highest
//!   material over one point.
//!
//! [`level_masks`] (`RestUse::LevelMask`, 2D Adaptive) reads the same mask
//! per level. [`standing_near`] (`RestUse::ContourFilter`, Waterline, W6.1)
//! reads the same floor around each contour point. It skips a level with no material in the footprint, and it
//! gives each other level the raw mask, which seeds the planner's grid.
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

use crate::compute::catalog::RestUse;
use crate::compute::toolpath_stats::{RestLevelsFallback, RestLevelsFinding};
use crate::dexel_stock::{FRESH_MATERIAL_THRESHOLD_MM, TriDexelStock};
use crate::geo::P2;
use crate::geometry::grid2::Grid2;
use crate::geometry::region_mask::region_polygons_from_mask_reported;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::polygon::{Polygon2, offset_polygon};
use crate::stock::dexel::{LatticeWindow, MaterialMask};

/// The floor of level `z`: material counts when it stands above
/// `z + FRESH_MATERIAL_THRESHOLD_MM`, the floor below which the simulator
/// itself calls material absent. Every rest rule of this module reads it.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the dexel grid holds f32 heights, so the floor is compared in f32"
)]
fn standing_floor(z: f64) -> f32 {
    (z + FRESH_MATERIAL_THRESHOLD_MM) as f32
}

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

/// What a cell off the stock grid holds, for [`regions_per_level`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OffGrid {
    /// The grid holds no answer there, so the cell counts as material and
    /// is cut (Pocket, Zigzag: an outline can reach past the stock box).
    Material,
    /// The outline grows the stock box on purpose, so a cell off the grid
    /// is air (Face: the facing rectangle is the stock box grown by
    /// `stock_offset`). Such a cell is outside the reach mask, so a level
    /// with all the stock standing still cuts the whole rectangle.
    Air,
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
    off_grid: OffGrid,
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
    let pad_cells = crate::util::steps::steps(dilate_mm, cell).unwrap_or(0) + 2;
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

    let mut reach = footprint_mask(stock, &window, outline, tool_radius);
    if off_grid == OffGrid::Air {
        for (i, r) in reach.iter_mut().enumerate() {
            let (row, col) = (i / window.cols, i % window.cols);
            let row = window.row0 + row as isize;
            let col = window.col0 + col as isize;
            #[expect(
                clippy::cast_sign_loss,
                reason = "row >= 0 and col >= 0 are tested before each cast on the same line"
            )]
            let on_grid =
                row >= 0 && col >= 0 && (row as usize) < grid.rows && (col as usize) < grid.cols;
            *r = *r && on_grid;
        }
    }
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
        let mut mask = grid.material_above_mask(&window, standing_floor(z));
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

/// `RestUse::LevelSkip` (W3.4): which levels of a contour op still have
/// material in the reach of the tool. Each level is `Whole` or `Empty`; a
/// contour op never cuts a clipped outline, because a clipped outline cuts
/// a new wall.
///
/// `paths` are the closed tool-centre rings of the op, in the frame of the
/// stock. `open` are its open tool-centre paths (P6.3): their band holds
/// only their own segments, never a chord between their ends, so a level
/// whose material lies only across the mouth of an open path is skipped. `half_width` is the tool radius when the path is the tool
/// centre, and twice the radius when the controller offsets the path (the
/// band then covers both sides). The reach band is every window cell whose
/// centre lies within `half_width - cell / 2` of a ring: the footprint
/// rule of [`regions_per_level`], for the same reason. The simulator
/// blends a cell that a tool covers only in part, so the cells along the
/// wall that the contour cuts keep part of their top. A band of
/// `half_width + cell` holds those wall cells, and a contour beside a
/// standing wall then never skips a level (G-RESTLEVELS, the Profile
/// case: 0 skipped levels of 7). A level is skipped only when no band
/// cell holds material above `z + FRESH_MATERIAL_THRESHOLD_MM`.
///
/// `stock == None` (a fresh operation) gives `Whole` at every level and no
/// finding. The fallbacks of [`regions_per_level`] apply: a stock off the
/// paths and a window too large give `Whole` at every level.
///
/// # Errors
///
/// [`Cancelled`] when `cancel` fires. The function polls once per level.
pub(crate) fn levels_in_reach(
    stock: Option<&TriDexelStock>,
    paths: &[Vec<P2>],
    open: &[Vec<P2>],
    levels: &[f64],
    half_width: f64,
    cancel: &dyn CancelCheck,
) -> Result<LevelPlan, Cancelled> {
    let Some(stock) = stock else {
        return Ok(LevelPlan::whole(levels.len(), None));
    };
    let unread = || {
        LevelPlan::whole(
            levels.len(),
            Some(RestLevelsFinding {
                planned_levels: levels.len(),
                skipped_levels: 0,
                clipped_levels: 0,
                fallback: None,
            }),
        )
    };
    let Some([px_min, py_min, px_max, py_max]) = paths_bbox(paths.iter().chain(open)) else {
        // No path: the op emits nothing at any level.
        return Ok(unread());
    };
    let grid = &stock.z_grid;
    let cell = grid.cell_size;
    let band = (half_width - 0.5 * cell).max(0.0);
    let sb = &stock.stock_bbox;
    let overlaps = sb.min.x <= px_max + band
        && sb.max.x >= px_min - band
        && sb.min.y <= py_max + band
        && sb.max.y >= py_min - band;
    if !overlaps {
        return Ok(LevelPlan::fallback(
            levels.len(),
            RestLevelsFallback::StockOffOutline,
        ));
    }
    let pad_cells = crate::util::steps::steps(band, cell).unwrap_or(0) + 2;
    let window = grid.lattice_window([px_min, py_min, px_max, py_max], pad_cells);
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
    let reach = band_mask(grid, &window, paths, open, band);
    if !reach.contains(&true) {
        // No cell centre lies in the band: the grid cannot see the paths.
        // Cut every level rather than skip a level on no evidence.
        return Ok(unread());
    }

    let mut plan = LevelPlan {
        levels: Vec::with_capacity(levels.len()),
        regions: Vec::new(),
        finding: None,
    };
    for &z in levels {
        check_cancel(cancel)?;
        let material = grid.material_above_mask(&window, standing_floor(z));
        let stands = material.iter().zip(&reach).any(|(&m, &r)| m && r);
        plan.levels.push(if stands {
            LevelRegion::Whole
        } else {
            LevelRegion::Empty
        });
    }
    plan.finding = Some(RestLevelsFinding {
        planned_levels: levels.len(),
        skipped_levels: plan
            .levels
            .iter()
            .filter(|l| **l == LevelRegion::Empty)
            .count(),
        clipped_levels: 0,
        fallback: None,
    });
    Ok(plan)
}

/// What one level of a `RestUse::LevelMask` op starts from.
#[derive(Debug, Clone)]
pub(crate) enum LevelSeed {
    /// Fresh stock: the planner starts from the whole outline.
    Fresh,
    /// No material stands in the footprint of the tool: skip the level.
    Empty,
    /// The material that stands above the level, read raw from the stock.
    Material(std::sync::Arc<MaterialMask>),
}

/// The answer of [`level_masks`] for one outline.
#[derive(Debug, Clone)]
pub(crate) struct SeedPlan {
    /// One entry per level of the ladder, in ladder order.
    pub levels: Vec<LevelSeed>,
    /// `None` when the operation read no machined stock.
    pub finding: Option<RestLevelsFinding>,
}

/// `RestUse::LevelMask` (2D Adaptive): the material each level of `levels`
/// starts from in `outline`, read from the machined stock.
///
/// Per level the function reads the mask of [`regions_per_level`] once,
/// at the same floor and on the stock lattice. It uses the mask twice:
///
/// - the skip: a level is [`LevelSeed::Empty`] when no cell in the
///   footprint of the tool holds material (the footprint rule of
///   [`regions_per_level`]);
/// - the seed: each other level gets the RAW mask, with no footprint
///   filter. The planner's engagement model must see the wood that stands
///   in its own grid, near a wall and in a corner too.
///
/// `stock == None` (a fresh operation) gives `Fresh` at every level and no
/// finding. A stock off the outline and a window too large give `Fresh`
/// at every level with the fallback in the finding. A footprint that holds
/// no cell centre never skips a level, but it still seeds.
///
/// # Errors
///
/// [`Cancelled`] when `cancel` fires. The function polls once per level.
pub(crate) fn level_masks(
    stock: Option<&TriDexelStock>,
    outline: &Polygon2,
    levels: &[f64],
    tool_radius: f64,
    cancel: &dyn CancelCheck,
) -> Result<SeedPlan, Cancelled> {
    let Some(stock) = stock else {
        return Ok(SeedPlan {
            levels: vec![LevelSeed::Fresh; levels.len()],
            finding: None,
        });
    };
    let fallback = |fallback: RestLevelsFallback| SeedPlan {
        levels: vec![LevelSeed::Fresh; levels.len()],
        finding: LevelPlan::fallback(levels.len(), fallback).finding,
    };
    let [ox_min, oy_min, ox_max, oy_max] = outline.bbox();
    let sb = &stock.stock_bbox;
    if !(sb.min.x <= ox_max && sb.max.x >= ox_min && sb.min.y <= oy_max && sb.max.y >= oy_min) {
        return Ok(fallback(RestLevelsFallback::StockOffOutline));
    }
    let grid = &stock.z_grid;
    // The planner's grid holds material only inside the outline, so the
    // outline box holds every cell it seeds. Two cells of pad take up the
    // rounding to the nearest ray. A point past the window reads as
    // material, so it is cut.
    let pad_cells = 2;
    let window = grid.lattice_window([ox_min, oy_min, ox_max, oy_max], pad_cells);
    let limit = (WINDOW_SPAN_OVER_GRID * (grid.rows + 2 * pad_cells))
        * (WINDOW_SPAN_OVER_GRID * (grid.cols + 2 * pad_cells));
    if window.cell_count() > limit {
        return Ok(fallback(RestLevelsFallback::WindowTooLarge {
            cells: window.cell_count(),
            limit,
        }));
    }
    let reach = footprint_mask(stock, &window, outline, tool_radius);
    let reach_seen = reach.contains(&true);

    let mut plan = SeedPlan {
        levels: Vec::with_capacity(levels.len()),
        finding: None,
    };
    for &z in levels {
        check_cancel(cancel)?;
        let mask = grid.material_mask(window, standing_floor(z));
        let stands = mask.cells().iter().zip(&reach).any(|(&m, &r)| m && r);
        plan.levels.push(if reach_seen && !stands {
            LevelSeed::Empty
        } else {
            LevelSeed::Material(std::sync::Arc::new(mask))
        });
    }
    plan.finding = Some(RestLevelsFinding {
        planned_levels: levels.len(),
        skipped_levels: plan
            .levels
            .iter()
            .filter(|l| matches!(l, LevelSeed::Empty))
            .count(),
        clipped_levels: 0,
        fallback: None,
    });
    Ok(plan)
}

/// `RestUse::ContourFilter` (W6.1): which points of a contour at one level
/// pass material. See [`standing_near`].
pub(crate) struct StandingNear<'s> {
    grid: &'s crate::stock::dexel::DexelGrid,
    /// [`standing_floor`] of the level.
    floor: f32,
    /// The reach (mm) from a contour point: `R + cell`.
    band: f64,
}

/// `RestUse::ContourFilter` (W6.1): the rule that keeps a contour point at
/// level `z`, or `None` for a fresh op (`stock == None`), which keeps
/// every point.
///
/// A point is kept when a cell whose centre lies within `R + cell` of it
/// holds material above `z + FRESH_MATERIAL_THRESHOLD_MM`. A finishing
/// pass must never leave material (lead ruling, 2026-10-15), so the band
/// takes in every cell the tool touches, a part-covered one too. The
/// narrower band of [`levels_in_reach`] (`R - cell / 2`) left 0.05 to
/// 0.13 mm in the W4 cap terraces. A point is also kept when the band
/// holds a cell off the grid: the grid holds no answer there, so the op
/// cuts more, never less.
pub(crate) fn standing_near(
    stock: Option<&TriDexelStock>,
    z: f64,
    tool_radius: f64,
) -> Option<StandingNear<'_>> {
    Some(ContourFilterRule::new(stock?, tool_radius, None).at(z))
}

impl StandingNear<'_> {
    /// True when the contour point `p` passes material, or when the grid
    /// holds no answer in its band. See [`standing_near`].
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a far point saturates its cell index, and the range test rejects it; r and c are cast only after `on_grid` proves them not negative"
    )]
    pub(crate) fn keeps(&self, p: P2) -> bool {
        let grid = self.grid;
        let cell = grid.cell_size;
        let index = |u: f64, origin: f64| ((u - origin) / cell).floor() as isize;
        let (c0, c1) = (
            index(p.x - self.band, grid.origin_u),
            index(p.x + self.band, grid.origin_u) + 1,
        );
        let (r0, r1) = (
            index(p.y - self.band, grid.origin_v),
            index(p.y + self.band, grid.origin_v) + 1,
        );
        let mut seen = false;
        for r in r0..=r1 {
            for c in c0..=c1 {
                let (x, y) = (
                    grid.origin_u + c as f64 * cell,
                    grid.origin_v + r as f64 * cell,
                );
                if (x - p.x).hypot(y - p.y) > self.band {
                    continue;
                }
                seen = true;
                let on_grid =
                    r >= 0 && c >= 0 && (r as usize) < grid.rows && (c as usize) < grid.cols;
                if !on_grid || grid.has_material_above(r as usize, c as usize, self.floor) {
                    return true;
                }
            }
        }
        !seen
    }
}

/// `RestUse::ContourFilter` (W6.1, W6.4.6): the rule a runner owns. The
/// Layered runner (`layered::run_layered`, Waterline) and the composed
/// runner (`composed::run_composed`, both halves of Steep and Shallow)
/// resolve it once from the territory ([`Self::resolve`]) and give it to
/// the op. The op folds [`Self::keeps`] into its per-point keep; it does
/// not read the stock itself.
#[derive(Clone, Copy)]
pub(crate) struct ContourFilterRule<'s> {
    /// The remaining stock after the W2.1 guard
    /// (`Territory::reference_stock`, C3).
    stock: &'s TriDexelStock,
    /// The cutter radius (mm), the `R` of [`standing_near`].
    tool_radius: f64,
    /// The machine envelope that times a stretch the rule drops
    /// (`ops::waterline::bridge_air_stretches`). `None`: no time model, so
    /// the rule drops no stretch between two kept points.
    split_timing: Option<&'s crate::machine::kinematics::LinkKinematics>,
}

impl<'s> ContourFilterRule<'s> {
    /// The rule of an op whose rest rule is `rest_use`. `Some` only for
    /// `RestUse::ContourFilter` with a remaining stock that the territory
    /// accepted as the rest reference; `None` for a fresh op, which keeps
    /// every point.
    pub(crate) fn resolve(
        rest_use: RestUse,
        territory: &super::territory::Territory<'s>,
        tool_radius: f64,
        split_timing: Option<&'s crate::machine::kinematics::LinkKinematics>,
    ) -> Option<Self> {
        if rest_use != RestUse::ContourFilter {
            return None;
        }
        territory
            .reference_stock()
            .map(|stock| Self::new(stock, tool_radius, split_timing))
    }

    /// The rule over `stock`. A runner calls [`Self::resolve`];
    /// [`standing_near`] (the Pencil entry check) calls this.
    pub(crate) fn new(
        stock: &'s TriDexelStock,
        tool_radius: f64,
        split_timing: Option<&'s crate::machine::kinematics::LinkKinematics>,
    ) -> Self {
        Self {
            stock,
            tool_radius,
            split_timing,
        }
    }

    /// The rule at level `z` ([`standing_near`]).
    pub(crate) fn at(&self, z: f64) -> StandingNear<'s> {
        StandingNear {
            grid: &self.stock.z_grid,
            floor: standing_floor(z),
            band: self.tool_radius + self.stock.z_grid.cell_size,
        }
    }

    /// True when material stands in the reach of the tool at `p`; `p.z` is
    /// the tool tip Z.
    pub(crate) fn keeps(&self, p: crate::geo::P3) -> bool {
        self.at(p.z).keeps(P2::new(p.x, p.y))
    }

    /// The machine envelope that times a dropped stretch.
    pub(crate) fn split_timing(&self) -> Option<&'s crate::machine::kinematics::LinkKinematics> {
        self.split_timing
    }
}

/// What the stock column over one point holds (`RestUse::StockColumn`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ColumnTop {
    /// No grid cell centre lies in the reach of the point: the grid holds
    /// no answer there.
    NoEvidence,
    /// No material stands in the reach of the point.
    Empty,
    /// The highest material top (mm) in the reach of the point.
    Top(f64),
}

/// D8: the highest material top in the reach of `xy`: every grid cell
/// whose centre lies within `reach + cell` of `xy`. A drill reads it as
/// the top of its hole and rapids down to just above it, so `reach` is
/// the tool radius. The extra cell takes in each cell the disc of the tool
/// covers in part, so no material in the disc stands above the result.
/// This is the safe side for a rapid; [`levels_in_reach`] takes the other
/// side, because a skipped level makes no motion.
#[expect(
    clippy::cast_sign_loss,
    reason = "row and col are cast only after the `row < 0 || col < 0` test skips a negative index"
)]
pub(crate) fn column_top(stock: &TriDexelStock, xy: P2, reach: f64) -> ColumnTop {
    let grid = &stock.z_grid;
    let cell = grid.cell_size;
    let band = reach + cell;
    let pad_cells = crate::util::steps::steps(band, cell).unwrap_or(0) + 1;
    let window = grid.lattice_window([xy.x, xy.y, xy.x, xy.y], pad_cells);
    let mut seen = false;
    let mut top: Option<f32> = None;
    for r in 0..window.rows {
        for c in 0..window.cols {
            let row = window.row0 + r as isize;
            let col = window.col0 + c as isize;
            if row < 0 || col < 0 || row as usize >= grid.rows || col as usize >= grid.cols {
                continue;
            }
            let (x, y) = grid.window_cell_centre(&window, r, c);
            if (x - xy.x).hypot(y - xy.y) > band {
                continue;
            }
            seen = true;
            if let Some(t) = grid.top_z_at(row as usize, col as usize) {
                top = Some(top.map_or(t, |prev| prev.max(t)));
            }
        }
    }
    match (seen, top) {
        (false, _) => ColumnTop::NoEvidence,
        (true, None) => ColumnTop::Empty,
        (true, Some(t)) => ColumnTop::Top(f64::from(t)),
    }
}

/// The XY box of every point of `paths`, or `None` when they hold no point.
fn paths_bbox<'a>(paths: impl IntoIterator<Item = &'a Vec<P2>>) -> Option<[f64; 4]> {
    let mut points = paths.into_iter().flatten();
    let first = points.next()?;
    Some(points.fold(
        [first.x, first.y, first.x, first.y],
        |[x0, y0, x1, y1], p| [x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)],
    ))
}

/// The window cells whose centre lies within `band` of a closed ring of
/// `paths` or of an open path of `open`. Row-major, like the material
/// mask. A closed ring adds its closing segment; an open path does not
/// (P6.3: no chord). Each segment marks only the cells of its own box
/// grown by `band`, so the cost follows the path length, not the window
/// area times the segment count.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a far point saturates its cell index, and the clamp to `0..rows` and `0..cols` bounds it; r and c are at least r0 and c0, which are at least 0"
)]
fn band_mask(
    grid: &crate::stock::dexel::DexelGrid,
    window: &LatticeWindow,
    paths: &[Vec<P2>],
    open: &[Vec<P2>],
    band: f64,
) -> Vec<bool> {
    let cell = grid.cell_size;
    let mut mask = vec![false; window.cell_count()];
    let to_index =
        |u: f64, origin: f64, first: isize| ((u - origin) / cell).floor() as isize - first;
    // (path, is closed): a closed ring has n segments, an open path n - 1.
    let lines = paths
        .iter()
        .map(|p| (p, true))
        .chain(open.iter().map(|p| (p, false)));
    for (ring, closed) in lines {
        let n = ring.len();
        let segments = if closed { n } else { n.saturating_sub(1) };
        for i in 0..segments {
            let (Some(&a), Some(&b)) = (ring.get(i), ring.get((i + 1) % n)) else {
                continue;
            };
            let c0 = to_index(a.x.min(b.x) - band, grid.origin_u, window.col0).max(0);
            let c1 = (to_index(a.x.max(b.x) + band, grid.origin_u, window.col0) + 1)
                .min(window.cols as isize - 1);
            let r0 = to_index(a.y.min(b.y) - band, grid.origin_v, window.row0).max(0);
            let r1 = (to_index(a.y.max(b.y) + band, grid.origin_v, window.row0) + 1)
                .min(window.rows as isize - 1);
            for r in r0..=r1 {
                for c in c0..=c1 {
                    let (x, y) = grid.window_cell_centre(window, r as usize, c as usize);
                    if point_segment_distance(P2::new(x, y), a, b) <= band
                        && let Some(m) = mask.get_mut(r as usize * window.cols + c as usize)
                    {
                        *m = true;
                    }
                }
            }
        }
    }
    mask
}

/// The distance from `p` to the segment `a`-`b`.
fn point_segment_distance(p: P2, a: P2, b: P2) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.x - (a.x + t * dx)).hypot(p.y - (a.y + t * dy))
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

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::geo::{BoundingBox3, P3};
    use crate::interrupt::NeverCancel;

    /// P6.3: the reach band of an OPEN path has no chord between its ends.
    ///
    /// A U path (open at the top, its mouth on Y 0 from X 20 back to X 0)
    /// and a stock that holds material only in a 4 x 2 mm block at the
    /// middle of the mouth. The block is 1 mm or less from the chord and
    /// 8 mm or more from every segment of the U. With the U as an open
    /// path, no band cell holds material, so the level is skipped. The
    /// control: the same points as a closed ring add the chord, and the
    /// level is cut.
    #[test]
    fn an_open_band_has_no_chord_p6_3() {
        let cell = 0.5;
        let tool_radius = 3.0;
        let mut stock = TriDexelStock::from_bounds(
            &BoundingBox3 {
                min: P3::new(-15.0, -35.0, -10.0),
                max: P3::new(35.0, 15.0, 0.0),
            },
            cell,
        );
        // Clear every column outside the block at the mouth.
        let grid = &mut stock.z_grid;
        let mut kept = 0;
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let (x, y) = grid.cell_to_world(row, col);
                if (8.0..=12.0).contains(&x) && (-1.0..=1.0).contains(&y) {
                    kept += 1;
                    continue;
                }
                grid.subtract_interval_at(row * grid.cols + col, -100.0, 100.0);
            }
        }
        assert!(
            kept > 0,
            "non-vacuity: the block at the mouth holds material"
        );
        let u = vec![
            P2::new(0.0, 0.0),
            P2::new(0.0, -20.0),
            P2::new(20.0, -20.0),
            P2::new(20.0, 0.0),
        ];
        let levels = [-5.0];
        let open = levels_in_reach(
            Some(&stock),
            &[],
            std::slice::from_ref(&u),
            &levels,
            tool_radius,
            &NeverCancel,
        )
        .unwrap();
        assert_eq!(
            open.levels,
            vec![LevelRegion::Empty],
            "an open U path read its chord: the material at the mouth kept the level"
        );
        let closed = levels_in_reach(
            Some(&stock),
            std::slice::from_ref(&u),
            &[],
            &levels,
            tool_radius,
            &NeverCancel,
        )
        .unwrap();
        assert_eq!(
            closed.levels,
            vec![LevelRegion::Whole],
            "control: the closed ring holds the chord, so the block keeps the level"
        );
        // The open band still reads material on its own segments.
        let leg = levels_in_reach(
            Some(&stock),
            &[],
            &[vec![P2::new(8.0, 0.0), P2::new(12.0, 0.0)]],
            &levels,
            tool_radius,
            &NeverCancel,
        )
        .unwrap();
        assert_eq!(
            leg.levels,
            vec![LevelRegion::Whole],
            "an open path over the block reads it"
        );
    }
}
