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
//! P41: the one-pass rule (`one_pass`) holds a tool centre off stock that
//! stands more than one Depth/Pass plus the stay-down clearance above the
//! cutter, and records each such stock cell. That record is cells, not an
//! outline, so it has no [`LeftWhy`].
//! [`left_stock`] reads those cells too ([`LeftStock::held_off_area_mm2`]).
//!
//! P44: the small-region bound is gone, and the record has one more read.
//! At the end of the plan [`left_stock`] takes the stock that stands where
//! the tool fits (`standing::standing_cells`: the material cells of the
//! bottom level and, on remaining stock, the strip cells beside a wall) and
//! names each connected place that has a drop record: its box, its area,
//! its height and the reason ([`LeftPlace`], [`PlaceWhy`]). The places with
//! no record (the margin of a plan at a wall, a strip that made no region)
//! are one count and one area (`LeftStock::unrecorded_places`): they do not
//! raise the finding alone (lead ruling 2026-11-21, [`LeftStock::stands`]).
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
    /// The F-038 forecast of the region is under
    /// `min_region_cut_length_mm`, or (P44) a run of a region contour is.
    ShortForecast,
    /// Geodesic Adaptive: the field does not fit in the piece (research
    /// C38 section 16). P44: a region where no piece gets a plan is cut by
    /// the region contour, and then it is not left (no record); the record
    /// stays for the pieces.
    NoField,
    /// P44b: the F-038 group filter dropped a cut group of a planned region
    /// (its cut is shorter than `min_region_cut_length_mm`). The outline is
    /// the XY box of the cut of the group. Evidence only: it is not a left
    /// piece, and it is not in `LeftStock::standing_pieces`; it gives its
    /// reason to the place on its cells.
    ShortGroup,
}

/// P44: why a place still holds stock at the end of the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceWhy {
    /// A cut there is shorter than `min_region_cut_length_mm` (the operator
    /// dial).
    ShortForecast,
    /// The Geodesic Adaptive field does not fit in the piece.
    NoField,
    /// The plan of the region dropped cut groups there: each is shorter
    /// than `min_region_cut_length_mm` (the operator dial).
    ShortGroup,
    /// The one-pass rule kept the tool off stock that stands more than one
    /// Depth/Pass above the cutter.
    OnePassHold,
}

/// P44: one place where the tool fits and stock stands at the end of the
/// plan. Positions are in the emission frame (mm).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeftPlace {
    /// The XY box of the tool-centre cells of the place.
    pub lo: [f64; 2],
    pub hi: [f64; 2],
    /// The XY area of those cells (mm²).
    pub area_mm2: f64,
    /// The tallest stock above the cutter in the place (mm), and the
    /// tool-centre position that reads it.
    pub tallest_mm: f64,
    pub at: [f64; 2],
    pub why: PlaceWhy,
}

/// The number of places that [`LeftStock::places`] carries (the largest
/// first). It limits the text of the finding only, never a cut:
/// `LeftStock::places_total` and `places_area_mm2` count all the places
/// with a drop record, and the text gives that sum.
pub const PLACES_LISTED: usize = 12;

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
#[derive(Debug, Clone, PartialEq)]
pub struct LeftStock {
    /// P44: the places where the tool fits, stock stands at the end of the
    /// plan and a drop record says why ([`PlaceWhy`]), the largest first,
    /// at most [`PLACES_LISTED`].
    pub places: Vec<LeftPlace>,
    /// The count and the tool-centre area (mm²) of all such places.
    pub places_total: usize,
    pub places_area_mm2: f64,
    /// P44: the count and the tool-centre area (mm²) of the places where
    /// the tool fits and stock stands with no drop record: the margin of a
    /// plan or of a contour at a wall, or a strip that made no region.
    pub unrecorded_places: usize,
    pub unrecorded_area_mm2: f64,
    /// P44: the places include the strip cells beside the walls (remaining
    /// stock, where the planner cell is the simulation cell).
    pub strips_read: bool,
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
    /// P41: the tool centres that the one-pass
    /// rule held off (region cells and wall contour points), summed over
    /// the levels.
    pub held_off_centres: usize,
    /// The XY area of the stock cells that held a centre off and still
    /// stand more than the limit above that cutter at the end of the plan
    /// (mm²).
    pub held_off_area_mm2: f64,
    /// The tallest such stock above the cutter that it held off (mm), its
    /// cell centre and the stock top there.
    pub held_off_tallest_mm: f64,
    pub held_off_at: [f64; 3],
}

impl LeftStock {
    /// True when the operation must state this record: a dropped piece
    /// still holds stock, the one-pass rule held the tool off stock that
    /// still stands, or (P44) a place with a drop record stands. The places
    /// with no record do not raise the finding alone (lead ruling
    /// 2026-11-21): the margin of each plan at a wall would raise it on
    /// every rough. The ONE rule of the finding and of the export gate.
    #[must_use]
    pub fn stands(&self) -> bool {
        self.standing_pieces > 0 || self.held_off_area_mm2 > 0.0 || self.places_total > 0
    }
}

/// Read the recorded outlines on the planner stock at the end of the plan.
/// `None`: the plan recorded nothing, dropped no cut group and held no
/// centre off. `standing` makes the read of the stock that stands where the
/// tool fits (`standing::standing_cells`); it is called only when the plan
/// has a record.
#[expect(
    clippy::too_many_arguments,
    reason = "each parameter is a separate input of the read"
)]
pub(super) fn left_stock(
    unplanned: &[Unplanned],
    dropped_cut_groups: usize,
    one_pass: &super::one_pass::OnePass,
    stock: &TriDexelStock,
    surface_hm: &SurfaceHeightmap,
    stock_to_leave: f64,
    standing: impl FnOnce() -> Option<super::standing::Standing>,
    tool_radius: f64,
) -> Option<LeftStock> {
    let held = one_pass.held.borrow();
    if unplanned.is_empty() && dropped_cut_groups == 0 && held.centres == 0 {
        return None;
    }
    let standing = standing();
    let standing = standing.as_ref();
    let count = |why: LeftWhy| unplanned.iter().filter(|u| u.why == why).count();
    let grid = &stock.z_grid;
    let cell = grid.cell_size;
    let mut seen = vec![false; grid.rows * grid.cols];
    // P44: the reason of each standing cell that lies in a recorded outline.
    let mut cell_why: Vec<Option<LeftWhy>> = vec![None; grid.rows * grid.cols];
    // P44b: a dropped cut group gives its reason to the standing cells of
    // its box and one cell round it (the box of a straight cut has no
    // area). It is not a piece of the standing figures.
    let mark_group =
        |cell_why: &mut [Option<LeftWhy>], rows: (usize, usize), cols: (usize, usize)| {
            for row in rows.0.saturating_sub(1)..=rows.1.min(grid.rows.saturating_sub(1)) {
                for col in cols.0.saturating_sub(1)..=cols.1.min(grid.cols.saturating_sub(1)) {
                    if standing.is_some_and(|s| s.at(row, col))
                        && let Some(why) = cell_why.get_mut(row * grid.cols + col)
                        && why.is_none()
                    {
                        *why = Some(LeftWhy::ShortGroup);
                    }
                }
            }
        };
    let mut out = LeftStock {
        places: Vec::new(),
        places_total: 0,
        places_area_mm2: 0.0,
        unrecorded_places: 0,
        unrecorded_area_mm2: 0.0,
        strips_read: one_pass.on && standing.is_some(),
        short_forecast: count(LeftWhy::ShortForecast),
        no_field: count(LeftWhy::NoField),
        dropped_cut_groups,
        standing_pieces: 0,
        standing_area_mm2: 0.0,
        tallest_mm: 0.0,
        tallest_at: [0.0; 3],
        held_off_centres: held.centres,
        held_off_area_mm2: 0.0,
        held_off_tallest_mm: 0.0,
        held_off_at: [0.0; 3],
    };
    let mut standing_cells = 0usize;
    for piece in unplanned {
        let Some((rows, cols)) = cell_box(&piece.outline, grid.origin_u, grid.origin_v, cell)
        else {
            continue;
        };
        if piece.why == LeftWhy::ShortGroup {
            mark_group(&mut cell_why, rows, cols);
            continue;
        }
        let mut stands = false;
        for row in rows.0..=rows.1.min(grid.rows.saturating_sub(1)) {
            for col in cols.0..=cols.1.min(grid.cols.saturating_sub(1)) {
                let (x, y) = grid.cell_to_world(row, col);
                if !piece.outline.contains_point(&P2::new(x, y)) {
                    continue;
                }
                if standing.is_some_and(|s| s.at(row, col))
                    && let Some(why) = cell_why.get_mut(row * grid.cols + col)
                {
                    *why = Some(piece.why);
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
    // P41: the stock cells that held a centre off and still stand more than
    // the limit above the cutter of that centre. A lower level cannot cut
    // them (the rule holds it off too), so they stand unless a pass at
    // their own level took them.
    let mut held_cells = 0usize;
    for (&(row, col), &cutter_z) in &held.cells {
        if row >= grid.rows || col >= grid.cols {
            continue;
        }
        let top = stock_top_z_at(stock, row, col);
        let over = top - cutter_z;
        if over <= one_pass.limit_mm {
            continue;
        }
        held_cells += 1;
        if over > out.held_off_tallest_mm {
            let (x, y) = grid.cell_to_world(row, col);
            out.held_off_tallest_mm = over;
            out.held_off_at = [x, y, top];
        }
    }
    out.held_off_area_mm2 = held_cells as f64 * cell * cell;
    if let Some(standing) = standing {
        places(
            &mut out,
            standing,
            &cell_why,
            &held,
            one_pass.limit_mm,
            stock,
            tool_radius,
        );
    }
    Some(out)
}

/// P44: the connected places of the standing cells (8-connected), each with
/// its box, area, height and reason. The reason is the record of a drop on
/// a cell of the place; else the one-pass hold, when a stock cell that held
/// a centre off still stands over the limit within one tool radius of the
/// box of the place; else the place has no record and joins
/// `LeftStock::unrecorded_places`.
#[expect(
    clippy::indexing_slicing,
    reason = "each index is row * cols + col with row < rows and col < cols"
)]
fn places(
    out: &mut LeftStock,
    standing: &super::standing::Standing,
    cell_why: &[Option<LeftWhy>],
    held: &super::one_pass::HeldOff,
    limit_mm: f64,
    stock: &TriDexelStock,
    tool_radius: f64,
) {
    let grid = &stock.z_grid;
    let (rows, cols, cell) = (grid.rows, grid.cols, grid.cell_size);
    let held_standing: Vec<(usize, usize)> = held
        .cells
        .iter()
        .filter(|&(&(row, col), &cutter_z)| {
            row < rows && col < cols && stock_top_z_at(stock, row, col) - cutter_z > limit_mm
        })
        .map(|(&cell_at, _)| cell_at)
        .collect();
    let mut seen = vec![false; rows * cols];
    let mut all: Vec<LeftPlace> = Vec::new();
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for row0 in 0..rows {
        for col0 in 0..cols {
            if seen[row0 * cols + col0] || !standing.at(row0, col0) {
                continue;
            }
            seen[row0 * cols + col0] = true;
            stack.push((row0, col0));
            let (mut r_lo, mut r_hi, mut c_lo, mut c_hi) = (row0, row0, col0, col0);
            let mut count = 0usize;
            let (mut tallest, mut tallest_at) = (0.0f32, (row0, col0));
            // The reasons on the cells of the place, by count.
            let mut whys = [0usize; 3];
            while let Some((row, col)) = stack.pop() {
                count += 1;
                r_lo = r_lo.min(row);
                r_hi = r_hi.max(row);
                c_lo = c_lo.min(col);
                c_hi = c_hi.max(col);
                let h = standing.height[row * cols + col];
                if h > tallest {
                    tallest = h;
                    tallest_at = (row, col);
                }
                match cell_why[row * cols + col] {
                    Some(LeftWhy::ShortForecast) => whys[0] += 1,
                    Some(LeftWhy::NoField) => whys[1] += 1,
                    Some(LeftWhy::ShortGroup) => whys[2] += 1,
                    None => {}
                }
                for dr in [-1isize, 0, 1] {
                    for dc in [-1isize, 0, 1] {
                        let (Some(r2), Some(c2)) = (
                            row.checked_add_signed(dr).filter(|&v| v < rows),
                            col.checked_add_signed(dc).filter(|&v| v < cols),
                        ) else {
                            continue;
                        };
                        if !seen[r2 * cols + c2] && standing.at(r2, c2) {
                            seen[r2 * cols + c2] = true;
                            stack.push((r2, c2));
                        }
                    }
                }
            }
            let (x_lo, y_lo) = grid.cell_to_world(r_lo, c_lo);
            let (x_hi, y_hi) = grid.cell_to_world(r_hi, c_hi);
            let reach = tool_radius + cell;
            let area_mm2 = count as f64 * cell * cell;
            let why = if whys.iter().any(|&n| n > 0) {
                if whys[0] >= whys[1] && whys[0] >= whys[2] {
                    PlaceWhy::ShortForecast
                } else if whys[1] >= whys[2] {
                    PlaceWhy::NoField
                } else {
                    PlaceWhy::ShortGroup
                }
            } else if held_standing.iter().any(|&(row, col)| {
                let (x, y) = grid.cell_to_world(row, col);
                x >= x_lo - reach && x <= x_hi + reach && y >= y_lo - reach && y <= y_hi + reach
            }) {
                PlaceWhy::OnePassHold
            } else {
                out.unrecorded_places += 1;
                out.unrecorded_area_mm2 += area_mm2;
                continue;
            };
            let (tx, ty) = grid.cell_to_world(tallest_at.0, tallest_at.1);
            all.push(LeftPlace {
                lo: [x_lo, y_lo],
                hi: [x_hi, y_hi],
                area_mm2,
                tallest_mm: f64::from(tallest),
                at: [tx, ty],
                why,
            });
        }
    }
    out.places_total = all.len();
    out.places_area_mm2 = all.iter().map(|p| p.area_mm2).sum();
    all.sort_by(|a, b| {
        b.area_mm2
            .total_cmp(&a.area_mm2)
            .then(a.lo[0].total_cmp(&b.lo[0]))
            .then(a.lo[1].total_cmp(&b.lo[1]))
    });
    all.truncate(PLACES_LISTED);
    out.places = all;
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
