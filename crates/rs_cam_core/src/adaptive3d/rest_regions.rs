//! The regions a 3D Rough leaves to a rest pass (integration step 7.5,
//! design §3.1 part H; research D44, D48). Geometry only: a strategy that
//! records the pieces it leaves (`ClearZLevelContext::left_pieces`, today
//! Geodesic Adaptive) hands them here, and the result is the op's
//! `Produced::rest_regions`.

use crate::polygon::{Polygon2, offset_polygon};

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
pub(super) fn left_regions(
    pieces: &[(f64, Polygon2)],
    grow: f64,
    centre_boundary: &[Polygon2],
) -> Option<Vec<Polygon2>> {
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
    (!out.is_empty()).then_some(out)
}
