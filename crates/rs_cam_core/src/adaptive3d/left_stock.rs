//! P37: the stock that a 3D Rough holds as material and does not plan.
//!
//! The level plan drops a region or a piece at three places
//! ([`LeftWhy`]). Each drop is recorded with its level and its outline
//! ([`Unplanned`]). At the end of the plan [`left_stock`] reads each
//! outline on the planner stock: a lower level can still cut a piece, so
//! only a piece whose cells still hold material above its level floor
//! counts as standing. The compute runner records the result as the
//! finding `geom.rough_left_stock`.
//!
//! The record is evidence only. It does not feed `Produced::rest_regions`
//! or the wall-contour skip (`ClearZLevelContext::left_pieces`), so it
//! moves no tool.

use crate::dexel_stock::TriDexelStock;
use crate::geo::P2;
use crate::polygon::Polygon2;
use crate::surface::slope::SurfaceHeightmap;

use super::intended_floor::level_floor_z;
use super::stock_top_z_at;

/// Why the level plan did not plan a region or a piece that holds material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftWhy {
    /// The region is under the small-region bound of its level
    /// (`detect_and_order_regions`).
    SmallRegion,
    /// The F-038 forecast of the region is under
    /// `min_region_cut_length_mm`.
    ShortForecast,
    /// Geodesic Adaptive: the field does not fit in the piece (research
    /// C38 section 16).
    NoField,
}

/// One region or piece that the plan held as material and did not plan, at
/// its level.
#[derive(Debug, Clone)]
pub(super) struct Unplanned {
    pub z_level: f64,
    pub outline: Polygon2,
    pub why: LeftWhy,
}

/// What a 3D Rough held as material and did not plan, read on the planner
/// stock at the end of the plan. Positions are in the emission frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeftStock {
    /// The regions under the small-region bound, summed over the levels.
    pub small_regions: usize,
    /// The regions the F-038 forecast skipped, summed over the levels.
    pub short_forecast: usize,
    /// The pieces where the geodesic field does not fit, summed over the
    /// levels.
    pub no_field: usize,
    /// The cut groups the F-038 group filter dropped. A group has no
    /// outline, so it is not in the standing figures.
    pub dropped_cut_groups: usize,
    /// The recorded regions and pieces that still hold material above
    /// their level floor at the end of the plan.
    pub standing_pieces: usize,
    /// The XY area of the planner cells that hold that material (mm²).
    /// Each cell counts once.
    pub standing_area_mm2: f64,
    /// The tallest such material above its level floor (mm), its cell
    /// centre and the stock top there.
    pub tallest_mm: f64,
    pub tallest_at: [f64; 3],
}

/// Read the recorded outlines on the planner stock at the end of the plan.
/// `None`: the plan recorded nothing and dropped no cut group.
pub(super) fn left_stock(
    unplanned: &[Unplanned],
    dropped_cut_groups: usize,
    stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    stock_to_leave: f64,
) -> Option<LeftStock> {
    if unplanned.is_empty() && dropped_cut_groups == 0 {
        return None;
    }
    let count = |why: LeftWhy| unplanned.iter().filter(|u| u.why == why).count();
    let grid = &stock.z_grid;
    let cell = grid.cell_size;
    let mut seen = vec![false; grid.rows * grid.cols];
    let mut out = LeftStock {
        small_regions: count(LeftWhy::SmallRegion),
        short_forecast: count(LeftWhy::ShortForecast),
        no_field: count(LeftWhy::NoField),
        dropped_cut_groups,
        standing_pieces: 0,
        standing_area_mm2: 0.0,
        tallest_mm: 0.0,
        tallest_at: [0.0; 3],
    };
    let mut standing_cells = 0usize;
    for piece in unplanned {
        let Some((rows, cols)) = cell_box(&piece.outline, grid.origin_u, grid.origin_v, cell)
        else {
            continue;
        };
        let mut stands = false;
        for row in rows.0..=rows.1.min(grid.rows.saturating_sub(1)) {
            for col in cols.0..=cols.1.min(grid.cols.saturating_sub(1)) {
                let (x, y) = grid.cell_to_world(row, col);
                if !piece.outline.contains_point(&P2::new(x, y)) {
                    continue;
                }
                let floor = level_floor_z(
                    surface_hm.z_or_bbox_floor_at(row, col),
                    stock_to_leave,
                    piece.z_level,
                );
                let top = stock_top_z_at(stock, row, col);
                let above = top - floor;
                if above <= super::level_contour::MATERIAL_MARGIN_MM {
                    continue;
                }
                stands = true;
                if let Some(cell_seen) = seen.get_mut(row * grid.cols + col)
                    && !*cell_seen
                {
                    *cell_seen = true;
                    standing_cells += 1;
                }
                if above > out.tallest_mm {
                    out.tallest_mm = above;
                    out.tallest_at = [x, y, top];
                }
            }
        }
        if stands {
            out.standing_pieces += 1;
        }
    }
    out.standing_area_mm2 = standing_cells as f64 * cell * cell;
    Some(out)
}

/// The inclusive row and column ranges of the grid cells in the XY box of
/// `outline`. `None`: the outline has no point, or its box is left of or
/// below the grid.
fn cell_box(
    outline: &Polygon2,
    origin_u: f64,
    origin_v: f64,
    cell: f64,
) -> Option<((usize, usize), (usize, usize))> {
    let (mut lo, mut hi) = (
        P2::new(f64::INFINITY, f64::INFINITY),
        P2::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
    );
    for p in &outline.exterior {
        lo = P2::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = P2::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    if !(lo.x.is_finite() && lo.y.is_finite() && hi.x.is_finite() && hi.y.is_finite()) {
        return None;
    }
    let index = |v: f64, origin: f64| -> Option<usize> {
        let i = ((v - origin) / cell).floor();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "floored, checked finite and clamped at 0"
        )]
        i.is_finite().then(|| i.max(0.0) as usize)
    };
    if hi.x < origin_u || hi.y < origin_v {
        return None;
    }
    Some((
        (index(lo.y, origin_v)?, index(hi.y, origin_v)? + 1),
        (index(lo.x, origin_u)?, index(hi.x, origin_u)? + 1),
    ))
}

/// The XY box of an outline as text, for the `info` line of a left piece.
pub(super) fn box_text(outline: &Polygon2) -> String {
    let (mut lo, mut hi) = (
        P2::new(f64::INFINITY, f64::INFINITY),
        P2::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
    );
    for p in &outline.exterior {
        lo = P2::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = P2::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    format!("x {:.2}..{:.2} y {:.2}..{:.2}", lo.x, hi.x, lo.y, hi.y)
}
