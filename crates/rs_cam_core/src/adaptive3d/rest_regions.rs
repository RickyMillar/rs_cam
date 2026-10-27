//! The regions a 3D Rough leaves to a rest pass (integration step 7.5,
//! design §3.1 part H; research D44, D48). Geometry only: a strategy that
//! records the pieces it leaves (`ClearZLevelContext::left_pieces`, today
//! Geodesic Adaptive) hands them here, and the result is the op's
//! `Produced::rest_regions`.

use crate::polygon::{Polygon2, offset_polygon};

/// D50: the top Z of the slab a level cuts: the next level above `z` in
/// `levels`, or `level_top` (the level anchor) for the first level. A
/// level cuts the stock from its own Z up to the level above it, so a
/// piece left at `z` leaves the stock up to this top.
pub(super) fn slab_top_z(z: f64, level_top: f64, levels: &[f64]) -> f64 {
    levels
        .iter()
        .copied()
        .filter(|&level| level > z)
        .fold(level_top, f64::min)
}

/// D44: the XY regions a run left to the rest pass: the union over the
/// levels of the pieces it left, grown by `grow` (one planner cell: a
/// piece is a raster outline of the planner's region cells and can lie up
/// to one cell inside the true region). `None` when the run left no piece.
///
/// D48 (G-RESTFRAME): the result is in tool-centre space, inside
/// `centre_boundary` (the run's own containment polygons,
/// `Adaptive3dGeometry::centre_boundary`; empty: no boundary). A piece is
/// a region outline that the 2D planner insets by the tool radius again,
/// and at a boundary the outer-band clip keeps the cells a disc reaches
/// (`outer_band::boundary_reach`), so the piece reaches the boundary line
/// itself. The union is clipped to `centre_boundary`, so a consumer that
/// keeps its tool centre inside a region (`Center`, offset 0) keeps a tool
/// no larger than this run's inside the run's machining boundary.
///
/// D50: each piece carries the top Z of the slab it leaves
/// ([`slab_top_z`]). The second value is the highest top of a piece that
/// lies in the clipped regions: no stock that the run left in them is
/// above it.
pub(super) fn left_regions(
    pieces: &[(f64, Polygon2)],
    grow: f64,
    centre_boundary: &[Polygon2],
) -> Option<(Vec<Polygon2>, f64)> {
    let all: Vec<Polygon2> = pieces.iter().map(|(_, p)| p.clone()).collect();
    let union = Polygon2::union_all(&all);
    let grown: Vec<Polygon2> = union
        .iter()
        .flat_map(|p| offset_polygon(p, -grow.max(0.0)))
        .collect();
    let out = if grown.is_empty() {
        union
    } else {
        Polygon2::union_all(&grown)
    };
    let out = if centre_boundary.is_empty() {
        out
    } else {
        let boundary = Polygon2::union_all(centre_boundary);
        let clipped: Vec<Polygon2> = out
            .iter()
            .flat_map(|region| boundary.iter().flat_map(|b| region.intersection(b)))
            .collect();
        Polygon2::union_all(&clipped)
    };
    if out.is_empty() {
        return None;
    }
    let top = pieces
        .iter()
        .filter(|(_, piece)| {
            out.iter()
                .any(|region| !region.intersection(piece).is_empty())
        })
        .map(|(top, _)| *top)
        .fold(f64::NEG_INFINITY, f64::max);
    top.is_finite().then_some((out, top))
}
