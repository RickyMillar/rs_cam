//! Project Curve on Surface — projects 2D polygon paths onto a 3D mesh for engraving.
//!
//! Given a 2D polygon (exterior + holes), resamples each ring at a fine spacing,
//! drops each point onto the mesh via `point_drop_cutter`, and builds a toolpath
//! that follows the projected contour at a specified depth below the surface.

use crate::dropcutter::point_drop_cutter;
use crate::geo::{P2, P3};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::Polygon2;
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;

/// Direction from which the curve is projected onto the mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectDirection {
    /// Project from above (Z-down). Tool contacts the top surface.
    FromAbove,
    /// Project from below (Z-up). Tool contacts the bottom surface.
    FromBelow,
}

/// Parameters for the project-curve-on-surface operation.
pub struct ProjectCurveParams {
    /// Cut depth below (or above) the mesh surface (positive = into material).
    pub depth: f64,
    /// Feed rate for lateral moves (mm/min).
    pub feed_rate: f64,
    /// Plunge rate for Z-descent moves (mm/min).
    pub plunge_rate: f64,
    /// Safe Z height for rapids above the workpiece.
    pub safe_z: f64,
    /// Resample spacing along polygon edges (mm). Smaller = smoother projection.
    pub point_spacing: f64,
    /// Which side of the mesh to project onto.
    pub direction: ProjectDirection,
    /// When true, the mesh has already been Z-inverted by a bottom-facing setup
    /// transform. `FromBelow` should NOT apply its own Z-flip in this case
    /// (it would double-flip, cancelling the setup transform).
    pub setup_z_flipped: bool,
}

impl Default for ProjectCurveParams {
    fn default() -> Self {
        Self {
            depth: 1.0,
            feed_rate: 1000.0,
            plunge_rate: 300.0,
            safe_z: 10.0,
            point_spacing: 0.5,
            direction: ProjectDirection::FromAbove,
            setup_z_flipped: false,
        }
    }
}

/// Resample a polyline (open or closed-irrelevant here) at regular spacing.
///
/// Walks edge-by-edge, accumulating distance. When the accumulated distance
/// exceeds `spacing`, a linearly interpolated point is inserted. The first
/// and last points of the input are always included.
fn resample_polyline(points: &[P2], spacing: f64) -> Vec<P2> {
    if points.len() < 2 || spacing <= 0.0 {
        return points.to_vec();
    }

    let mut result = Vec::with_capacity((polyline_length(points) / spacing) as usize + 2);
    // SAFETY: len >= 2 checked above
    #[allow(clippy::indexing_slicing)]
    result.push(points[0]);

    let mut accumulated = 0.0;

    // SAFETY: i ranges 1..len, so i and i-1 are always valid
    #[allow(clippy::indexing_slicing)]
    for i in 1..points.len() {
        let prev = points[i - 1];
        let curr = points[i];
        let dx = curr.x - prev.x;
        let dy = curr.y - prev.y;
        let seg_len = (dx * dx + dy * dy).sqrt();

        if seg_len < 1e-12 {
            continue;
        }

        let mut remaining = seg_len;

        // How far along this segment before we emit the next sample?
        let mut next_emit = spacing - accumulated;

        while next_emit <= remaining + 1e-12 {
            // Interpolate from the original segment start (prev) by accumulated fraction
            let frac = 1.0 - (remaining - next_emit) / seg_len;
            let pt = P2::new(prev.x + dx * frac, prev.y + dy * frac);
            result.push(pt);
            remaining -= next_emit;
            accumulated = 0.0;
            next_emit = spacing;
        }

        accumulated += remaining;
    }

    // Always include the last point (avoid duplicate if very close)
    // SAFETY: len >= 2 checked at function entry
    #[allow(clippy::indexing_slicing)]
    let last = points[points.len() - 1];
    if let Some(prev) = result.last() {
        let d = ((last.x - prev.x).powi(2) + (last.y - prev.y).powi(2)).sqrt();
        if d > 1e-9 {
            result.push(last);
        }
    }

    result
}

/// Total length of a polyline.
#[allow(clippy::indexing_slicing)] // windows(2) guarantees w[0] and w[1] exist
fn polyline_length(points: &[P2]) -> f64 {
    let mut len = 0.0;
    for w in points.windows(2) {
        let dx = w[1].x - w[0].x;
        let dy = w[1].y - w[0].y;
        len += (dx * dx + dy * dy).sqrt();
    }
    len
}

/// Return true if the vertical ray from (x, y) passes through the 2D
/// footprint of at least one triangle in the mesh. Used by project_curve to
/// reject points that fall inside mesh holes — `point_drop_cutter` would
/// otherwise happily report a contact on the hole's rim, producing a
/// phantom cutting bridge across the hole.
#[allow(clippy::indexing_slicing)]
fn point_over_triangle(x: f64, y: f64, mesh: &TriangleMesh, index: &SpatialIndex) -> bool {
    for &idx in &index.query(x, y, 0.0) {
        let tri = &mesh.faces[idx];
        if tri.contains_point_xy(x, y) {
            return true;
        }
    }
    false
}

/// Close a ring by appending the first point if not already duplicated.
fn close_ring(ring: &[P2]) -> Vec<P2> {
    if ring.len() < 2 {
        return ring.to_vec();
    }
    // SAFETY: len >= 2 checked above
    #[allow(clippy::indexing_slicing)]
    let first = ring[0];
    // SAFETY: ring.len() >= 2 checked above
    #[allow(clippy::expect_used)]
    let last = *ring.last().expect("len >= 2");
    let d = ((first.x - last.x).powi(2) + (first.y - last.y).powi(2)).sqrt();
    if d > 1e-9 {
        let mut closed = ring.to_vec();
        closed.push(first);
        closed
    } else {
        ring.to_vec()
    }
}

/// Project 2D polygon paths onto a 3D mesh and produce an engraving toolpath.
///
/// Each ring of the polygon (exterior and each hole) is treated as a separate
/// chain. Points that fall outside the mesh footprint (no triangle contact) are
/// skipped, splitting the chain into sub-segments.
///
/// When `direction` is `FromBelow`, the mesh is Z-flipped so the drop cutter
/// finds the bottom surface contact, then the result is flipped back.
pub fn project_curve_toolpath(
    polygon: &Polygon2,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ProjectCurveParams,
) -> Toolpath {
    match params.direction {
        ProjectDirection::FromAbove => {
            project_curve_inner(polygon, mesh, index, cutter, params, false)
        }
        ProjectDirection::FromBelow => {
            if params.setup_z_flipped {
                // The mesh is already Z-inverted by a bottom-facing setup transform.
                // The drop cutter finds correct surface contact without an additional
                // flip. Depth goes below the surface in local frame (same as FromAbove).
                project_curve_inner(polygon, mesh, index, cutter, params, false)
            } else {
                // Standalone (no setup transform): flip the mesh Z so the bottom
                // surface becomes the top for the drop cutter.
                let flipped = mesh.z_flipped();
                let flipped_index = SpatialIndex::build_auto(&flipped);
                project_curve_inner(polygon, &flipped, &flipped_index, cutter, params, true)
            }
        }
    }
}

/// Inner projection loop. When `z_flip` is true, the mesh was Z-flipped before
/// calling, so the output Z coordinates are negated back to world space and
/// depth goes upward (into the bottom surface).
fn project_curve_inner(
    polygon: &Polygon2,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ProjectCurveParams,
    z_flip: bool,
) -> Toolpath {
    let mut tp = Toolpath::new();

    // Collect all rings: exterior first, then holes
    let mut rings: Vec<&Vec<P2>> = Vec::with_capacity(1 + polygon.holes.len());
    rings.push(&polygon.exterior);
    for hole in &polygon.holes {
        rings.push(hole);
    }

    for (ring_idx, ring) in rings.iter().enumerate() {
        if ring.len() < 2 {
            continue;
        }

        // Only close the ring for true polygon rings (holes always close, and
        // the exterior closes when polygon.closed is true). Open paths
        // (Polygon2::open_path — unclosed DXF polylines, SVG paths without Z)
        // must stay open, otherwise we emit a phantom segment from the last
        // point back to the first, which carves a straight line across the
        // stock at cut depth.
        let is_hole = ring_idx > 0;
        let ring_points: Vec<P2> = if polygon.closed || is_hole {
            close_ring(ring)
        } else {
            (*ring).clone()
        };
        let resampled = resample_polyline(&ring_points, params.point_spacing);

        // Project each 2D point onto the mesh
        let mut current_chain: Vec<P3> = Vec::new();

        for pt in &resampled {
            let cl = point_drop_cutter(pt.x, pt.y, mesh, index, cutter);
            // `point_drop_cutter` marks `contacted=true` whenever the cutter
            // (which has radius) touches ANY nearby triangle — including the
            // edge of a hole in the mesh. For project_curve we want to trace
            // the surface *directly below* the 2D path, so reject any point
            // whose vertical ray doesn't actually pass through a triangle.
            // Otherwise the tool rides on the hole's rim and produces
            // phantom bridges across the gap, visible as straight cuts
            // crossing the stock.
            let has_surface_below = cl.contacted && point_over_triangle(pt.x, pt.y, mesh, index);
            if has_surface_below {
                let z = if z_flip {
                    // Flip Z back to world space and go depth INTO the bottom surface (upward).
                    -cl.z + params.depth
                } else {
                    cl.z - params.depth
                };
                current_chain.push(P3::new(pt.x, pt.y, z));
            } else {
                // Gap over air (or over a mesh hole) — flush any
                // accumulated chain so the next contiguous stretch starts
                // fresh with a rapid-plunge rather than a feed bridging it.
                if !current_chain.is_empty() {
                    tp.emit_path_segment(
                        &current_chain,
                        params.safe_z,
                        params.feed_rate,
                        params.plunge_rate,
                    );
                    current_chain.clear();
                }
            }
        }

        // Flush remaining chain
        if !current_chain.is_empty() {
            tp.emit_path_segment(
                &current_chain,
                params.safe_z,
                params.feed_rate,
                params.plunge_rate,
            );
        }
    }

    tp.final_retract(params.safe_z);
    tp
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn test_resample_polyline_basic() {
        // A straight line from (0,0) to (10,0) resampled at spacing 3.0
        let pts = vec![P2::new(0.0, 0.0), P2::new(10.0, 0.0)];
        let resampled = resample_polyline(&pts, 3.0);

        // Should get points at 0, 3, 6, 9, 10
        assert_eq!(resampled.len(), 5);
        assert!((resampled[0].x - 0.0).abs() < 1e-9);
        assert!((resampled[1].x - 3.0).abs() < 1e-9);
        assert!((resampled[2].x - 6.0).abs() < 1e-9);
        assert!((resampled[3].x - 9.0).abs() < 1e-9);
        assert!((resampled[4].x - 10.0).abs() < 1e-9);
    }

    #[test]
    fn test_resample_polyline_multi_segment() {
        // L-shaped path: (0,0) -> (4,0) -> (4,4), total length 8, spacing 3
        let pts = vec![P2::new(0.0, 0.0), P2::new(4.0, 0.0), P2::new(4.0, 4.0)];
        let resampled = resample_polyline(&pts, 3.0);

        // Samples at distance 0, 3, 6 along the path, plus endpoint at distance 8
        assert_eq!(resampled.len(), 4);
        assert!((resampled[0].x - 0.0).abs() < 1e-9);
        assert!((resampled[0].y - 0.0).abs() < 1e-9);
        // distance 3 is at (3, 0)
        assert!((resampled[1].x - 3.0).abs() < 1e-9);
        assert!((resampled[1].y - 0.0).abs() < 1e-9);
        // distance 6 is at (4, 2)
        assert!((resampled[2].x - 4.0).abs() < 1e-9);
        assert!((resampled[2].y - 2.0).abs() < 1e-9);
        // endpoint at (4, 4)
        assert!((resampled[3].x - 4.0).abs() < 1e-9);
        assert!((resampled[3].y - 4.0).abs() < 1e-9);
    }

    #[test]
    fn test_resample_single_point() {
        let pts = vec![P2::new(5.0, 5.0)];
        let resampled = resample_polyline(&pts, 1.0);
        assert_eq!(resampled.len(), 1);
    }

    #[test]
    fn test_resample_zero_spacing() {
        let pts = vec![P2::new(0.0, 0.0), P2::new(10.0, 0.0)];
        let resampled = resample_polyline(&pts, 0.0);
        // With zero spacing, returns original points
        assert_eq!(resampled.len(), 2);
    }

    #[test]
    fn test_close_ring() {
        let ring = vec![P2::new(0.0, 0.0), P2::new(1.0, 0.0), P2::new(1.0, 1.0)];
        let closed = close_ring(&ring);
        assert_eq!(closed.len(), 4);
        assert!((closed[3].x - 0.0).abs() < 1e-9);
        assert!((closed[3].y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_close_ring_already_closed() {
        let ring = vec![
            P2::new(0.0, 0.0),
            P2::new(1.0, 0.0),
            P2::new(1.0, 1.0),
            P2::new(0.0, 0.0),
        ];
        let closed = close_ring(&ring);
        assert_eq!(closed.len(), 4); // Should not add duplicate
    }

    #[test]
    fn test_polyline_length() {
        let pts = vec![P2::new(0.0, 0.0), P2::new(3.0, 0.0), P2::new(3.0, 4.0)];
        let len = polyline_length(&pts);
        assert!((len - 7.0).abs() < 1e-9);
    }
}
