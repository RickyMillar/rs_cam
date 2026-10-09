//! Project Curve on Surface — projects 2D polygon paths onto a 3D mesh for engraving.
//!
//! Given a 2D polygon (exterior + holes), resamples each ring at a fine spacing,
//! finds the tip Z of each point and builds a toolpath that follows the
//! projected contour at a specified depth below the surface.
//!
//! Two projection modes find the tip Z ([`ProjectProjection`]):
//! - `Cutter` drops the real cutter (`point_drop_cutter`). The tool body
//!   stops on the surface, so the surface is protected.
//! - `Point` reads the surface Z directly under the point
//!   ([`crate::maps::reach_map::surface_z_at`]) and adds the depth. The tool
//!   body is not tested against the surface.

use crate::geo::{P2, P3, resample_polyline};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::maps::reach_map::surface_z_at;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::{Polygon2, offset_polygon};
use crate::surface::dropcutter::point_drop_cutter;
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;

/// How many resampled points to process between cancel polls in
/// `project_polygon_rings_with_cancel` — frequent enough to stay
/// responsive, coarse enough to avoid an atomic load per point.
const CANCEL_POLL_STRIDE: usize = 64;

/// Direction from which the curve is projected onto the mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectDirection {
    /// Project from above (Z-down). Tool contacts the top surface.
    FromAbove,
    /// Project from below (Z-up). Tool contacts the bottom surface.
    FromBelow,
}

/// Tool-radius compensation side for closed rings. Open rings ignore this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectSide {
    /// Tool centerline rides exactly on the curve.
    #[default]
    Center,
    /// Offset inward by tool radius (closed rings only).
    Inside,
    /// Offset outward by tool radius (closed rings only).
    Outside,
}

/// How the tip Z of a sample is found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProjectProjection {
    /// Drop the real cutter on the surface. The tip stops where the tool
    /// body first touches the surface.
    #[default]
    Cutter,
    /// Tip Z = the surface Z directly under the sample, plus the depth. The
    /// tool body is not tested against the surface, so it can cut a wall
    /// that is steeper than the tool flank.
    ///
    /// Sample rule: every vertex of the curve is a sample, and each curve
    /// segment is split into equal steps of at most `point_spacing`. At a
    /// vertex the tip is exact; between two samples the tool moves on the
    /// chord, so the error is the chord error of the surface over one step.
    Point,
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
    /// Tool radius for compensation (used when `side != Center`).
    pub tool_radius: f64,
    /// Compensation side. Closed rings only.
    pub side: ProjectSide,
    /// How the tip Z of a sample is found.
    pub projection: ProjectProjection,
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
            tool_radius: 0.0,
            side: ProjectSide::Center,
            projection: ProjectProjection::Cutter,
            setup_z_flipped: false,
        }
    }
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

/// Sample a polyline for [`ProjectProjection::Point`]: every input vertex,
/// and each segment split into `ceil(len / spacing)` equal steps, so no
/// step is longer than `spacing`. A segment shorter than 1e-9 mm adds no
/// sample. A `spacing` at or below 1e-6 mm returns the vertices.
fn sample_vertices_and_steps(points: &[P2], spacing: f64) -> Vec<P2> {
    let Some(first) = points.first() else {
        return Vec::new();
    };
    if points.len() < 2 || spacing <= 1e-6 {
        return points.to_vec();
    }
    let mut out = vec![*first];
    for w in points.windows(2) {
        let (Some(a), Some(b)) = (w.first(), w.get(1)) else {
            continue;
        };
        let delta = b - a;
        let len = delta.norm();
        if len < 1e-9 {
            continue;
        }
        // SAFETY: `len / spacing` is finite and positive here (len >= 1e-9,
        // spacing > 1e-6); a curve segment never needs 2^32 steps.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let steps = (len / spacing).ceil().max(1.0) as usize;
        for k in 1..steps {
            #[allow(clippy::cast_precision_loss)] // SAFETY: k < steps, a small count
            let t = k as f64 / steps as f64;
            out.push(a + delta * t);
        }
        out.push(*b);
    }
    out
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
    crate::interrupt::run_uncancellable(|cancel| {
        project_curve_toolpath_with_cancel(polygon, mesh, index, cutter, params, cancel)
    })
}

/// Cancellable variant of [`project_curve_toolpath`]. Polls `cancel` every
/// [`CANCEL_POLL_STRIDE`] resampled points inside the per-ring drop-cutter
/// loop (planning/finishing_stack_review_2026-07.md S.5: "project_curve
/// (drop-cutter per point)" — the worst per-op loop of the flat-2D family).
pub fn project_curve_toolpath_with_cancel(
    polygon: &Polygon2,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ProjectCurveParams,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    check_cancel(cancel)?;
    match params.direction {
        ProjectDirection::FromAbove => {
            project_curve_inner_with_cancel(polygon, mesh, index, cutter, params, false, cancel)
        }
        ProjectDirection::FromBelow => {
            if params.setup_z_flipped {
                // The mesh is already Z-inverted by a bottom-facing setup transform.
                // The drop cutter finds correct surface contact without an additional
                // flip. Depth goes below the surface in local frame (same as FromAbove).
                project_curve_inner_with_cancel(polygon, mesh, index, cutter, params, false, cancel)
            } else {
                // Standalone (no setup transform): flip the mesh Z so the bottom
                // surface becomes the top for the drop cutter.
                let flipped = mesh.z_flipped();
                let flipped_index = SpatialIndex::build_auto(&flipped);
                project_curve_inner_with_cancel(
                    polygon,
                    &flipped,
                    &flipped_index,
                    cutter,
                    params,
                    true,
                    cancel,
                )
            }
        }
    }
}

/// Inner projection loop. When `z_flip` is true, the mesh was Z-flipped before
/// calling, so the output Z coordinates are negated back to world space and
/// depth goes upward (into the bottom surface).
#[allow(clippy::too_many_arguments)]
fn project_curve_inner_with_cancel(
    polygon: &Polygon2,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ProjectCurveParams,
    z_flip: bool,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    let mut tp = Toolpath::new();

    // Apply tool-radius compensation for closed polygons. Open polygons cannot
    // be meaningfully offset (no inside/outside) — fall back to Center.
    // Sign convention in offset_polygon: distance > 0 = inward (shrink),
    // distance < 0 = outward (grow). Center carries no offset distance, so
    // the match stays exhaustive without an unreachable arm.
    let offset_distance = match params.side {
        ProjectSide::Inside => Some(params.tool_radius),
        ProjectSide::Outside => Some(-params.tool_radius),
        ProjectSide::Center => None,
    };
    let offset_polys: Vec<Polygon2> = if polygon.closed {
        match offset_distance {
            Some(distance) => offset_polygon(polygon, distance),
            None => Vec::new(),
        }
    } else {
        Vec::new()
    };
    let polys_iter: Vec<&Polygon2> = if offset_polys.is_empty() {
        // Either Center, open ring, or offset collapsed → use original.
        vec![polygon]
    } else {
        offset_polys.iter().collect()
    };

    for poly in polys_iter {
        project_polygon_rings_with_cancel(
            poly, mesh, index, cutter, params, z_flip, &mut tp, cancel,
        )?;
    }

    tp.final_retract(params.safe_z);
    Ok(tp)
}

#[allow(clippy::too_many_arguments)]
fn project_polygon_rings_with_cancel(
    polygon: &Polygon2,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ProjectCurveParams,
    z_flip: bool,
    tp: &mut Toolpath,
    cancel: &dyn CancelCheck,
) -> Result<(), Cancelled> {
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
        let resampled = match params.projection {
            ProjectProjection::Cutter => resample_polyline(&ring_points, params.point_spacing),
            ProjectProjection::Point => {
                sample_vertices_and_steps(&ring_points, params.point_spacing)
            }
        };

        // Project each 2D point onto the mesh
        let mut current_chain: Vec<P3> = Vec::new();

        for (i, pt) in resampled.iter().enumerate() {
            if i % CANCEL_POLL_STRIDE == 0 {
                check_cancel(cancel)?;
            }
            // The tip Z in the projection frame, or `None` where no surface
            // is directly under the point.
            let surface_tip_z = match params.projection {
                ProjectProjection::Cutter => {
                    let cl = point_drop_cutter(pt.x, pt.y, mesh, index, cutter);
                    // `point_drop_cutter` marks `contacted=true` whenever the cutter
                    // (which has radius) touches ANY nearby triangle — including the
                    // edge of a hole in the mesh. For project_curve we want to trace
                    // the surface *directly below* the 2D path, so reject any point
                    // whose vertical ray doesn't actually pass through a triangle.
                    // Otherwise the tool rides on the hole's rim and produces
                    // phantom bridges across the gap, visible as straight cuts
                    // crossing the stock.
                    let has_surface_below =
                        cl.contacted && point_over_triangle(pt.x, pt.y, mesh, index);
                    has_surface_below.then_some(cl.z)
                }
                // The zero-radius drop: the highest surface under the point
                // in the projection frame. It uses the same containment test
                // as `point_over_triangle`, so a mesh hole is a gap here too.
                ProjectProjection::Point => surface_z_at(pt.x, pt.y, mesh, index),
            };
            if let Some(tip_z) = surface_tip_z {
                let z = if z_flip {
                    // Flip Z back to world space and go depth INTO the bottom surface (upward).
                    -tip_z + params.depth
                } else {
                    tip_z - params.depth
                };
                current_chain.push(P3::new(pt.x, pt.y, z));
            } else {
                // Gap over air (or over a mesh hole) — flush any
                // accumulated chain so the next contiguous stretch starts
                // fresh with a rapid-plunge rather than a feed bridging it.
                if !current_chain.is_empty() {
                    tp.emit_path_segment_with_intent(
                        &current_chain,
                        params.safe_z,
                        params.feed_rate,
                        params.plunge_rate,
                        crate::toolpath::MoveIntent::FinishingCut,
                    );
                    current_chain.clear();
                }
            }
        }

        // Flush remaining chain
        if !current_chain.is_empty() {
            tp.emit_path_segment_with_intent(
                &current_chain,
                params.safe_z,
                params.feed_rate,
                params.plunge_rate,
                crate::toolpath::MoveIntent::FinishingCut,
            );
        }
    }

    Ok(())
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

    // ── Point projection ────────────────────────────────────────────

    /// Valley half-width (mm) of the synthetic surface, and its wall slope.
    const VALLEY_HALF_WIDTH: f64 = 10.0;
    /// Wall slope: tan(60°). The 90° V-bit flank is at 45° from the
    /// horizontal, so the cone touches both walls before the tip reaches
    /// the crease.
    const WALL_SLOPE: f64 = 1.732_050_807_568_877_2;
    /// Crease Z of the original surface; the walls fall away from it.
    const CREASE_Z: f64 = 5.0;

    /// The original surface: `z = CREASE_Z - WALL_SLOPE * |x|`. Seen from
    /// below, this is a sharp V groove whose walls are at 60°. The standalone
    /// `FromBelow` arm flips it, so the drop cutter sees a V valley.
    fn original_surface_z(x: f64) -> f64 {
        CREASE_Z - WALL_SLOPE * x.abs()
    }

    /// A heightfield of the surface above, with a vertex column on the
    /// crease (x = 0) so each cell is planar. 1 mm cells, upward winding.
    fn valley_mesh() -> TriangleMesh {
        let n = 20_u32;
        let mut vertices = Vec::new();
        for j in 0..=n {
            for i in 0..=n {
                let x = -VALLEY_HALF_WIDTH + f64::from(i);
                let y = -VALLEY_HALF_WIDTH + f64::from(j);
                vertices.push(P3::new(x, y, original_surface_z(x)));
            }
        }
        let mut triangles = Vec::new();
        for j in 0..n {
            for i in 0..n {
                let v00 = j * (n + 1) + i;
                let v10 = v00 + 1;
                let v01 = v00 + n + 1;
                let v11 = v01 + 1;
                triangles.push([v00, v10, v11]);
                triangles.push([v00, v11, v01]);
            }
        }
        TriangleMesh::from_raw(vertices, triangles)
    }

    /// The fed points of a toolpath: every `Linear` target. The plunge
    /// target is the first chain point, so the list is the chain points.
    fn fed_points(tp: &Toolpath) -> Vec<P3> {
        tp.moves
            .iter()
            .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Linear { .. }))
            .map(|m| m.target)
            .collect()
    }

    fn valley_params(
        projection: ProjectProjection,
        direction: ProjectDirection,
        depth: f64,
    ) -> ProjectCurveParams {
        ProjectCurveParams {
            depth,
            point_spacing: 0.5,
            safe_z: 30.0,
            direction,
            projection,
            ..ProjectCurveParams::default()
        }
    }

    /// The test curve. It runs along the crease, and its crease vertices
    /// are NOT a whole number of spacings from the start (3.3 mm, then
    /// 5.7 mm), so the arc-length resample of `Cutter` skips them.
    fn valley_curve() -> Polygon2 {
        Polygon2::open_path(vec![
            P2::new(-3.3, -2.0),
            P2::new(0.0, -2.0),
            P2::new(0.0, 3.7),
            P2::new(2.9, 3.7),
        ])
    }

    /// On a steep V valley cut from the far side, Point puts the tip at the
    /// surface Z plus the depth at every sample, and every curve vertex is a
    /// sample. Cutter, on the same curve, stops the cone on the walls, so
    /// its tip is lower in world Z (past the surface by more than the depth)
    /// by the cone interference: for a 90° cone of radius 3 mm in 60° walls,
    /// 3 * (tan 60° - 1) = 2.196 mm.
    #[test]
    fn point_projection_follows_the_surface_where_the_cutter_cannot() {
        let mesh = valley_mesh();
        let index = SpatialIndex::build_auto(&mesh);
        let vbit = crate::tool::VBitEndmill::new(6.0, 90.0, 20.0);
        let depth = -0.4;

        // From below, standalone: the case of the RivMap back-side V lines.
        let point = project_curve_toolpath(
            &valley_curve(),
            &mesh,
            &index,
            &vbit,
            &valley_params(ProjectProjection::Point, ProjectDirection::FromBelow, depth),
        );
        let pts = fed_points(&point);
        assert!(
            pts.len() > 20,
            "Point emits the sampled curve, got {}",
            pts.len()
        );
        for p in &pts {
            let want = original_surface_z(p.x) + depth;
            assert!(
                (p.z - want).abs() < 1e-6,
                "Point tip at ({:.3}, {:.3}) is {:.9}, want surface + depth = {want:.9}",
                p.x,
                p.y,
                p.z
            );
        }
        for v in &valley_curve().exterior {
            assert!(
                pts.iter().any(|p| (p.x - v.x).hypot(p.y - v.y) < 1e-12),
                "curve vertex ({}, {}) is a Point sample",
                v.x,
                v.y
            );
        }
        // No step between samples is longer than the spacing.
        for w in pts.windows(2) {
            assert!((w[1].x - w[0].x).hypot(w[1].y - w[0].y) <= 0.5 + 1e-12);
        }

        let cutter = project_curve_toolpath(
            &valley_curve(),
            &mesh,
            &index,
            &vbit,
            &valley_params(
                ProjectProjection::Cutter,
                ProjectDirection::FromBelow,
                depth,
            ),
        );
        let interference = 3.0 * (WALL_SLOPE - 1.0);
        let crease: Vec<P3> = fed_points(&cutter)
            .into_iter()
            .filter(|p| p.x.abs() < 1e-9 && p.y > -1.0 && p.y < 2.7)
            .collect();
        assert!(!crease.is_empty(), "Cutter has samples on the crease");
        for p in &crease {
            let point_z = CREASE_Z + depth;
            assert!(
                (p.z - (point_z - interference)).abs() < 1e-6,
                "Cutter tip on the crease is {:.6}; Point gives {point_z:.6}; the cone \
                 interference is {interference:.6} mm",
                p.z
            );
        }

        // From above, the same rule: tip = surface - depth at every sample.
        let above = project_curve_toolpath(
            &valley_curve(),
            &mesh,
            &index,
            &vbit,
            &valley_params(ProjectProjection::Point, ProjectDirection::FromAbove, 0.4),
        );
        for p in fed_points(&above) {
            let want = original_surface_z(p.x) - 0.4;
            assert!(
                (p.z - want).abs() < 1e-6,
                "FromAbove Point tip {} != {want}",
                p.z
            );
        }
    }

    /// On a flat surface the two modes agree: with curve vertices on the
    /// spacing grid both modes take the same samples, and the cutter
    /// touches the plane with its tip.
    #[test]
    fn point_and_cutter_agree_on_a_flat_surface() {
        let mesh = crate::mesh::make_test_flat(100.0);
        let index = SpatialIndex::build_auto(&mesh);
        let vbit = crate::tool::VBitEndmill::new(6.0, 20.0, 20.0);
        let curve = Polygon2::open_path(vec![P2::new(0.0, 0.0), P2::new(10.0, 0.0)]);
        for (direction, depth) in [
            (ProjectDirection::FromAbove, 1.0),
            (ProjectDirection::FromBelow, -2.2685),
        ] {
            let run = |projection| {
                fed_points(&project_curve_toolpath(
                    &curve,
                    &mesh,
                    &index,
                    &vbit,
                    &valley_params(projection, direction, depth),
                ))
            };
            let point = run(ProjectProjection::Point);
            let cutter = run(ProjectProjection::Cutter);
            assert_eq!(point.len(), 21, "{direction:?}: 10 mm at 0.5 mm, both ends");
            assert_eq!(point.len(), cutter.len(), "{direction:?}: same samples");
            for (a, b) in point.iter().zip(&cutter) {
                assert!((a - b).norm() < 1e-9, "{direction:?}: {a} != {b}");
            }
        }
    }

    #[test]
    fn point_samples_keep_vertices_and_cap_the_step() {
        let pts = vec![P2::new(0.0, 0.0), P2::new(1.3, 0.0), P2::new(1.3, 0.2)];
        let s = sample_vertices_and_steps(&pts, 0.5);
        // 1.3 mm -> 3 steps of 0.4333; 0.2 mm -> 1 step.
        assert_eq!(s.len(), 5);
        assert!((s[3] - P2::new(1.3, 0.0)).norm() < 1e-12);
        assert!((s[4] - P2::new(1.3, 0.2)).norm() < 1e-12);
        assert!((s[1].x - 1.3 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn default_projection_is_cutter() {
        assert_eq!(
            ProjectCurveParams::default().projection,
            ProjectProjection::Cutter
        );
        assert_eq!(ProjectProjection::default(), ProjectProjection::Cutter);
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
}
