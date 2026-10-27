//! Geodesic Adaptive, the edges of its plan (research C38,
//! `planning/adaptive_research_2026-10-23/P3_C38_outer_band.md`).
//!
//! The arm clears the body only (§16, operator ruling 2026-10-27). The outer
//! strip and the stock-edge band go to a separate rest pass. The parts here
//! keep the body's plan correct at its edges:
//! - Item B: the grown outline stops at the stock box
//!   ([`clip_to_stock_box`]). Beyond the box edge no material stands, so
//!   the grow there would put phantom stock in the 2D grid.
//! - §13.7: the grown outline also stops at the reach of the boundary clip
//!   ([`clip_to_centre_reach`]), so nothing is planned where the clip
//!   would drop it.
//! - G-CLIPAIR (§11): the boundary clear keeps the stock a disc with its
//!   centre inside the boundary can reach ([`boundary_reach`]), so the plan
//!   reads the band it ploughs, and the level's regions stay inside the
//!   boundary ([`RegionClip::restrict_regions`]). Global ordering only.
//!
//! §13.8: each helper takes geometry only; the strategy test sits at the
//! call site, so a shared layer can adopt it unchanged.

use crate::dexel_stock::TriDexelStock;
use crate::polygon::{Polygon2, offset_polygon};

use super::centre_clip::CentreClip;

/// The C31 grown outlines of a region clipped to the XY box of the planner
/// stock, or `None` when nothing is left (the region then keeps the
/// shipped path, as when the grow gives no outline).
///
/// A polygon inside the box is returned unchanged, so a region away from
/// the stock edge plans exactly as before.
pub(super) fn clip_to_stock_box(
    grown: Vec<Polygon2>,
    stock: &TriDexelStock,
) -> Option<Vec<Polygon2>> {
    let (lo, hi) = (stock.stock_bbox.min, stock.stock_bbox.max);
    let inside = |poly: &Polygon2| {
        poly.exterior
            .iter()
            .all(|p| p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y)
    };
    let stock_box = Polygon2::rectangle(lo.x, lo.y, hi.x, hi.y);
    let out: Vec<Polygon2> = grown
        .into_iter()
        .flat_map(|poly| {
            if inside(&poly) {
                vec![poly]
            } else {
                poly.intersection(&stock_box)
            }
        })
        .filter(|poly| poly.exterior.len() >= 3)
        .collect();
    (!out.is_empty()).then_some(out)
}

/// G-CLIPAIR (§11 a): the boundary grown by the tool radius, the reach of a
/// disc whose centre is inside `boundary`, or `None` when the radius is not
/// usable or the grow is empty. The boundary clear then empties only the
/// planner-stock cells outside this reach. The cells between the boundary
/// and the reach keep their stock, which a cut from inside the boundary
/// meets (P3_C38 §10.7, §11.1b). Geometry only (§13.8): the caller decides
/// which strategies use it.
pub(super) fn boundary_reach(boundary: &Polygon2, tool_radius: f64) -> Option<Vec<Polygon2>> {
    if !tool_radius.is_finite() || tool_radius <= 0.0 {
        return None;
    }
    let reach = offset_polygon(boundary, -tool_radius);
    (!reach.is_empty()).then_some(reach)
}

/// G-CLIPAIR (§11 b): which planner-stock cells have their centre inside
/// the boundary. Built by the boundary clear (`path.rs`) when
/// [`boundary_reach`] gives a reach.
pub(super) struct RegionClip {
    /// One flag per planner-stock cell (row-major, `rows × cols`).
    inside: Vec<bool>,
    rows: usize,
    cols: usize,
}

impl RegionClip {
    pub(super) fn new(inside: Vec<bool>, rows: usize, cols: usize) -> Option<Self> {
        (inside.len() == rows * cols).then_some(Self { inside, rows, cols })
    }

    /// Restrict a level's padded material grid (`build_material_bool_grid`:
    /// cell `(r, c)` of the stock is padded cell `(r + 1, c + 1)`) to the
    /// region cells of Geodesic Adaptive: a cell whose centre is outside the
    /// boundary is not a region cell, whatever its stock (the boundary
    /// clear gave this before). §16: the band stock the reach keeps is read
    /// by the plan, but no region is made to cut it; the edge contour pass
    /// cuts it.
    pub(super) fn restrict_regions(
        &self,
        grid: &mut [bool],
        padded_rows: usize,
        padded_cols: usize,
    ) {
        if padded_rows != self.rows + 2 || padded_cols != self.cols + 2 {
            return;
        }
        let at = |r: usize, c: usize| (r + 1) * padded_cols + (c + 1);
        for r in 0..self.rows {
            for c in 0..self.cols {
                let inside = self.inside.get(r * self.cols + c).copied().unwrap_or(false);
                if let Some(cell) = grid.get_mut(at(r, c))
                    && !inside
                {
                    *cell = false;
                }
            }
        }
    }
}

/// §13.7: the C31 grown outlines of a region clipped to the reach of the
/// boundary clip: each clip polygon grown by `tool_radius - margin`. The 2D
/// planner insets its outline by the tool radius, so its tool-centre region
/// then lies within the clip shrunk by `margin` (up to a concave notch of
/// the clip narrower than twice the grow), and every turn, held link and
/// wrap end it plans is one the clip keeps. `margin` is the C31 margin
/// (`PERIMETER_INSET_MARGIN_MM`). With no clip the outlines pass unchanged;
/// a polygon inside the reach is returned unchanged; `None` when nothing is
/// left (the region then keeps the shipped path). Geometry only (§13.8).
pub(super) fn clip_to_centre_reach(
    grown: Vec<Polygon2>,
    clip: Option<&CentreClip>,
    tool_radius: f64,
    margin: f64,
) -> Option<Vec<Polygon2>> {
    let Some(clip) = clip else {
        return Some(grown);
    };
    let grow = tool_radius - margin;
    if !grow.is_finite() || grow <= 0.0 {
        return Some(grown);
    }
    let reach: Vec<Polygon2> = clip
        .polygons()
        .iter()
        .flat_map(|poly| offset_polygon(poly, -grow))
        .collect();
    if reach.is_empty() {
        return None;
    }
    let inside = |poly: &Polygon2| {
        reach
            .iter()
            .any(|r| poly.exterior.iter().all(|p| r.contains_point(p)))
    };
    let out: Vec<Polygon2> = grown
        .into_iter()
        .flat_map(|poly| {
            if inside(&poly) {
                vec![poly]
            } else {
                reach.iter().flat_map(|r| poly.intersection(r)).collect()
            }
        })
        .filter(|poly| poly.exterior.len() >= 3)
        .collect();
    (!out.is_empty()).then_some(out)
}
