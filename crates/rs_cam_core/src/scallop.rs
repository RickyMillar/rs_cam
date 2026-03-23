//! Scallop finishing strategy — constant scallop height across the surface.
//!
//! Generates concentric offset contours with variable stepover that maintains
//! constant scallop height regardless of surface slope and curvature. On steep
//! walls the stepover is wider (ball endmill has larger effective radius), on
//! shallow convex areas it is tighter.
//!
//! Algorithm:
//! 1. Project mesh boundary onto XY → outer polygon
//! 2. Compute variable stepover from scallop height, local slope, and curvature
//! 3. Iteratively offset polygon inward by stepover
//! 4. Drop-cutter Z at each ring's points → 3D contour
//! 5. Chain rings into toolpath (with optional spiral connection)
//!
//! From Fusion 360 docs: "passes follow sloping and vertical walls to maintain
//! the stepover."

use crate::debug_trace::ToolpathDebugContext;
use crate::dropcutter::point_drop_cutter;
use crate::geo::{P2, P3};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::{Polygon2, offset_polygon};
use crate::scallop_math::variable_stepover;
use crate::slope::SurfaceHeightmap;
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;

use tracing::info;

/// Direction for scallop contouring.
#[derive(Debug, Clone, Copy, Default)]
pub enum ScallopDirection {
    /// Start from boundary, work inward (default).
    #[default]
    OutsideIn,
    /// Start from center, work outward.
    InsideOut,
}

/// Parameters for scallop finishing.
pub struct ScallopParams {
    /// Desired scallop height (mm). This is the PRIMARY parameter.
    pub scallop_height: f64,
    /// Path tolerance for simplification.
    pub tolerance: f64,
    /// Direction of contouring.
    pub direction: ScallopDirection,
    /// Connect contours into a continuous spiral (fewer retracts).
    pub continuous: bool,
    /// Slope confinement: only machine slopes steeper than this (degrees).
    pub slope_from: f64,
    /// Slope confinement: only machine slopes shallower than this (degrees).
    pub slope_to: f64,
    /// Feed rate for cutting moves (mm/min).
    pub feed_rate: f64,
    /// Plunge rate (mm/min).
    pub plunge_rate: f64,
    /// Safe Z for rapid positioning.
    pub safe_z: f64,
    /// Stock to leave on the surface (mm).
    pub stock_to_leave: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScallopRuntimeEvent {
    Ring {
        ring_index: usize,
        ring_total: usize,
        continuous: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScallopRuntimeAnnotation {
    pub move_index: usize,
    pub event: ScallopRuntimeEvent,
}

impl ScallopRuntimeEvent {
    pub fn label(&self) -> String {
        match self {
            Self::Ring { ring_index, .. } => format!("Ring {ring_index}"),
        }
    }
}

impl Default for ScallopParams {
    fn default() -> Self {
        Self {
            scallop_height: 0.01,
            tolerance: 0.05,
            direction: ScallopDirection::OutsideIn,
            continuous: false,
            slope_from: 0.0,
            slope_to: 90.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            stock_to_leave: 0.0,
        }
    }
}

/// Compute the average variable stepover for a ring of 2D points using
/// the slope map and scallop math.
fn average_stepover_for_ring(
    ring: &[P2],
    slope_map: &crate::slope::SlopeMap,
    tool_radius: f64,
    scallop_height: f64,
) -> f64 {
    if ring.is_empty() {
        return crate::scallop_math::stepover_from_scallop_flat(tool_radius, scallop_height);
    }

    let sample_step = 1.max(ring.len() / 20);
    let mut sum = 0.0;
    let mut count = 0;

    for pt in ring.iter().step_by(sample_step) {
        let angle = slope_map.angle_at_world(pt.x, pt.y).unwrap_or(0.0);
        let curvature = slope_map.curvature_at_world(pt.x, pt.y).unwrap_or(0.0);
        let so = variable_stepover(tool_radius, scallop_height, angle, curvature);
        if so > 0.01 {
            sum += so;
            count += 1;
        }
    }

    if count == 0 {
        crate::scallop_math::stepover_from_scallop_flat(tool_radius, scallop_height)
    } else {
        sum / count as f64
    }
}

/// Lift a 2D polygon ring to 3D by drop-cutter Z queries.
/// Points with non-finite Z (outside mesh footprint) get clamped to `min_z`.
fn ring_to_3d(
    ring: &[P2],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    stock_to_leave: f64,
    min_z: f64,
) -> Vec<P3> {
    ring.iter()
        .map(|p| {
            let cl = point_drop_cutter(p.x, p.y, mesh, index, cutter);
            let z = if cl.z.is_finite() {
                cl.z + stock_to_leave
            } else {
                min_z + stock_to_leave
            };
            P3::new(p.x, p.y, z)
        })
        .collect()
}

/// Generate concentric offset rings from the outer boundary inward.
///
/// Uses variable stepover: at each ring, samples the slope map to compute
/// the average stepover that maintains constant scallop height, then offsets
/// by that amount.
#[allow(clippy::too_many_arguments)]
fn generate_scallop_rings(
    boundary: &Polygon2,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    slope_map: &crate::slope::SlopeMap,
    tool_radius: f64,
    scallop_height: f64,
    stock_to_leave: f64,
    min_z: f64,
    max_rings: usize,
) -> Vec<Vec<P3>> {
    let mut rings_3d: Vec<Vec<P3>> = Vec::new();

    // First ring: the boundary itself, lifted to 3D
    let first_ring = ring_to_3d(
        &boundary.exterior,
        mesh,
        index,
        cutter,
        stock_to_leave,
        min_z,
    );
    if first_ring.len() < 3 {
        return rings_3d;
    }
    rings_3d.push(first_ring);

    // Iteratively offset inward
    let mut current_polys = vec![boundary.clone()];

    for _ in 0..max_rings {
        // Compute average stepover from the current ring's slope/curvature
        let avg_stepover = if current_polys.is_empty() {
            crate::scallop_math::stepover_from_scallop_flat(tool_radius, scallop_height)
        } else {
            // Sample from all current polygons
            let mut total_so = 0.0;
            let mut total_count = 0;
            for poly in &current_polys {
                let so = average_stepover_for_ring(
                    &poly.exterior,
                    slope_map,
                    tool_radius,
                    scallop_height,
                );
                total_so += so * poly.exterior.len() as f64;
                total_count += poly.exterior.len();
            }
            if total_count > 0 {
                total_so / total_count as f64
            } else {
                crate::scallop_math::stepover_from_scallop_flat(tool_radius, scallop_height)
            }
        };

        // Clamp stepover to reasonable bounds
        let stepover = avg_stepover
            .max(tool_radius * 0.05) // At least 5% of tool radius
            .min(tool_radius * 3.0); // At most 3× tool radius

        // Offset all current polygons inward
        let mut next_polys = Vec::new();
        for poly in &current_polys {
            let offsets = offset_polygon(poly, stepover);
            next_polys.extend(offsets);
        }

        if next_polys.is_empty() {
            break; // Collapsed to nothing
        }

        // Lift each new polygon ring to 3D
        for poly in &next_polys {
            if poly.exterior.len() < 3 {
                continue;
            }
            let ring_3d = ring_to_3d(&poly.exterior, mesh, index, cutter, stock_to_leave, min_z);
            if ring_3d.len() >= 3 {
                rings_3d.push(ring_3d);
            }
        }

        current_polys = next_polys;
    }

    rings_3d
}

/// Find the closest point index on `ring` to `target`.
fn closest_point_idx(ring: &[P3], target: &P3) -> usize {
    ring.iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let da = (a.x - target.x).powi(2) + (a.y - target.y).powi(2);
            let db = (b.x - target.x).powi(2) + (b.y - target.y).powi(2);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}

/// Reorder a ring to start at the given index.
fn rotate_ring(ring: &[P3], start_idx: usize) -> Vec<P3> {
    let n = ring.len();
    if n == 0 || start_idx == 0 {
        return ring.to_vec();
    }
    let mut result = Vec::with_capacity(n);
    // SAFETY: (start_idx + i) % n is always in 0..n
    #[allow(clippy::indexing_slicing)]
    for i in 0..n {
        result.push(ring[(start_idx + i) % n]);
    }
    result
}

/// Generate a scallop finishing toolpath.
///
/// Produces concentric offset contours with variable stepover that maintains
/// constant scallop height across the surface regardless of slope and curvature.
#[tracing::instrument(skip(mesh, index, cutter, params), fields(scallop_height = params.scallop_height))]
#[allow(clippy::indexing_slicing)] // ring/filtered indexing is guarded by len checks
pub fn scallop_toolpath(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ScallopParams,
) -> Toolpath {
    let (tp, _) = scallop_toolpath_structured_annotated(mesh, index, cutter, params, None);
    tp
}

fn runtime_annotations_to_labels(annotations: &[ScallopRuntimeAnnotation]) -> Vec<(usize, String)> {
    annotations
        .iter()
        .map(|annotation| (annotation.move_index, annotation.event.label()))
        .collect()
}

pub fn scallop_toolpath_structured_annotated(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ScallopParams,
    debug: Option<&ToolpathDebugContext>,
) -> (Toolpath, Vec<ScallopRuntimeAnnotation>) {
    let tool_radius = cutter.radius();
    let bbox = &mesh.bbox;

    // Build surface heightmap and slope map
    let cell_size = (tool_radius / 4.0).max(params.tolerance);
    let origin_x = bbox.min.x - tool_radius;
    let origin_y = bbox.min.y - tool_radius;
    let extent_x = bbox.max.x + tool_radius;
    let extent_y = bbox.max.y + tool_radius;
    let cols = ((extent_x - origin_x) / cell_size).ceil() as usize + 1;
    let rows = ((extent_y - origin_y) / cell_size).ceil() as usize + 1;

    let surface_hm = SurfaceHeightmap::from_mesh(
        mesh, index, cutter, origin_x, origin_y, rows, cols, cell_size, bbox.min.z,
    );
    let slope_map = surface_hm.slope_map();

    // Outer boundary: mesh footprint as a rectangle, sampled densely enough
    // for polygon offset to work correctly. Point spacing = stepover.
    let bx0 = bbox.min.x;
    let by0 = bbox.min.y;
    let bx1 = bbox.max.x;
    let by1 = bbox.max.y;
    let flat_so =
        crate::scallop_math::stepover_from_scallop_flat(tool_radius, params.scallop_height)
            .max(tool_radius * 0.1);
    let boundary = {
        let mut pts = Vec::new();
        // Bottom edge
        let mut x = bx0;
        while x < bx1 {
            pts.push(P2::new(x, by0));
            x += flat_so;
        }
        // Right edge
        let mut y = by0;
        while y < by1 {
            pts.push(P2::new(bx1, y));
            y += flat_so;
        }
        // Top edge (reversed)
        let mut x = bx1;
        while x > bx0 {
            pts.push(P2::new(x, by1));
            x -= flat_so;
        }
        // Left edge (reversed)
        let mut y = by1;
        while y > by0 {
            pts.push(P2::new(bx0, y));
            y -= flat_so;
        }
        if pts.len() < 4 {
            // Fallback to simple rectangle
            pts = vec![
                P2::new(bx0, by0),
                P2::new(bx1, by0),
                P2::new(bx1, by1),
                P2::new(bx0, by1),
            ];
        }
        Polygon2::new(pts)
    };

    // Max rings: bounded by extent / min_stepover
    let min_stepover =
        crate::scallop_math::stepover_from_scallop_flat(tool_radius, params.scallop_height)
            .max(tool_radius * 0.05);
    let max_extent = (extent_x - origin_x).max(extent_y - origin_y);
    let max_rings = ((max_extent / min_stepover) * 0.5).ceil() as usize + 10;

    info!(
        max_rings = max_rings,
        min_stepover = format!("{:.3}", min_stepover),
        "Generating scallop rings"
    );

    // Generate 3D rings
    let mut rings = generate_scallop_rings(
        &boundary,
        mesh,
        index,
        cutter,
        &slope_map,
        tool_radius,
        params.scallop_height,
        params.stock_to_leave,
        bbox.min.z,
        max_rings,
    );

    info!(rings = rings.len(), "Scallop rings generated");

    if rings.is_empty() {
        return (Toolpath::new(), Vec::new());
    }

    // Apply direction
    if matches!(params.direction, ScallopDirection::InsideOut) {
        rings.reverse();
    }

    // Slope confinement
    let use_slope_filter = params.slope_from > 0.01 || params.slope_to < 89.99;
    let slope_from_rad = params.slope_from.to_radians();
    let slope_to_rad = params.slope_to.to_radians();

    // Convert rings to toolpath
    let mut tp = Toolpath::new();
    let mut annotations = Vec::new();

    if params.continuous && rings.len() >= 2 {
        // Continuous spiral mode: connect adjacent rings at their nearest points
        // Start from the first ring
        // SAFETY: rings.len() >= 2 checked above
        #[allow(clippy::indexing_slicing)]
        let mut prev_end = rings[0].last().copied().unwrap_or(rings[0][0]);

        for (i, ring) in rings.iter().enumerate() {
            // Rotate ring to start at the closest point to the previous endpoint
            let start_idx = closest_point_idx(ring, &prev_end);
            let rotated = rotate_ring(ring, start_idx);
            let move_index = tp.moves.len();
            annotations.push(ScallopRuntimeAnnotation {
                move_index,
                event: ScallopRuntimeEvent::Ring {
                    ring_index: i + 1,
                    ring_total: rings.len(),
                    continuous: true,
                },
            });

            // SAFETY: rotated is non-empty (ring is non-empty)
            #[allow(clippy::indexing_slicing)]
            if i == 0 {
                // First ring: rapid to start
                tp.rapid_to(P3::new(rotated[0].x, rotated[0].y, params.safe_z));
                tp.feed_to(rotated[0], params.plunge_rate);
            } else {
                // Connect from previous ring end to this ring start (helical transition)
                tp.feed_to(rotated[0], params.feed_rate);
            }

            // Follow the ring
            #[allow(clippy::indexing_slicing)]
            for pt in &rotated[1..] {
                if use_slope_filter {
                    let in_range = slope_map
                        .angle_at_world(pt.x, pt.y)
                        .is_some_and(|a| a >= slope_from_rad && a <= slope_to_rad);
                    if !in_range {
                        continue;
                    }
                }
                tp.feed_to(*pt, params.feed_rate);
            }

            #[allow(clippy::indexing_slicing)]
            {
                prev_end = rotated.last().copied().unwrap_or(rotated[0]);
            }
        }

        // Final retract
        tp.rapid_to(P3::new(prev_end.x, prev_end.y, params.safe_z));
    } else {
        // Discrete ring mode: rapid between rings
        let mut emitted_rings = Vec::new();
        for ring in &rings {
            if ring.len() < 3 {
                continue;
            }

            // Filter by slope if configured
            let filtered: Vec<P3> = if use_slope_filter {
                ring.iter()
                    .filter(|pt| {
                        slope_map
                            .angle_at_world(pt.x, pt.y)
                            .is_some_and(|a| a >= slope_from_rad && a <= slope_to_rad)
                    })
                    .copied()
                    .collect()
            } else {
                ring.clone()
            };

            if filtered.len() < 3 {
                continue;
            }
            emitted_rings.push(filtered);
        }

        // SAFETY: filtered.len() >= 3 checked before pushing to emitted_rings
        #[allow(clippy::indexing_slicing)]
        for (ring_index, filtered) in emitted_rings.iter().enumerate() {
            let move_index = tp.moves.len();
            annotations.push(ScallopRuntimeAnnotation {
                move_index,
                event: ScallopRuntimeEvent::Ring {
                    ring_index: ring_index + 1,
                    ring_total: emitted_rings.len(),
                    continuous: false,
                },
            });
            tp.rapid_to(P3::new(filtered[0].x, filtered[0].y, params.safe_z));
            tp.feed_to(filtered[0], params.plunge_rate);
            for pt in &filtered[1..] {
                tp.feed_to(*pt, params.feed_rate);
            }
            // Close the ring
            tp.feed_to(filtered[0], params.feed_rate);
            tp.rapid_to(P3::new(filtered[0].x, filtered[0].y, params.safe_z));
        }
    }

    if let Some(last) = tp.moves.last()
        && !matches!(last.move_type, crate::toolpath::MoveType::Rapid)
    {
        tp.rapid_to(P3::new(last.target.x, last.target.y, params.safe_z));
    }

    info!(
        moves = tp.moves.len(),
        cutting_mm = format!("{:.1}", tp.total_cutting_distance()),
        rapid_mm = format!("{:.1}", tp.total_rapid_distance()),
        "Scallop toolpath complete"
    );

    if let Some(debug_ctx) = debug {
        for annotation in &annotations {
            debug_ctx.add_annotation(annotation.move_index, annotation.event.label());
        }
    }

    (tp, annotations)
}

pub fn scallop_toolpath_annotated(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ScallopParams,
    debug: Option<&ToolpathDebugContext>,
) -> (Toolpath, Vec<(usize, String)>) {
    let (tp, annotations) =
        scallop_toolpath_structured_annotated(mesh, index, cutter, params, debug);
    (tp, runtime_annotations_to_labels(&annotations))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::mesh::SpatialIndex;
    use crate::tool::BallEndmill;

    fn make_flat_mesh() -> (TriangleMesh, SpatialIndex) {
        let mesh = crate::mesh::make_test_flat(50.0);
        let si = SpatialIndex::build(&mesh, 10.0);
        (mesh, si)
    }

    fn make_hemisphere() -> (TriangleMesh, SpatialIndex) {
        let mesh = crate::mesh::make_test_hemisphere(20.0, 16);
        let si = SpatialIndex::build(&mesh, 10.0);
        (mesh, si)
    }

    fn ball_cutter() -> BallEndmill {
        BallEndmill::new(6.35, 25.0)
    }

    // ── Ring generation tests ───────────────────────────────────────

    #[test]
    fn test_scallop_flat_constant_stepover() {
        // On a flat surface, variable stepover should equal the flat formula everywhere
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let tool_radius = cutter.radius();
        let scallop_height = 0.1;

        let expected_so =
            crate::scallop_math::stepover_from_scallop_flat(tool_radius, scallop_height);

        let cell_size = 1.0;
        let bbox = &mesh.bbox;
        let origin_x = bbox.min.x - tool_radius;
        let origin_y = bbox.min.y - tool_radius;
        let extent_x = bbox.max.x + tool_radius;
        let extent_y = bbox.max.y + tool_radius;
        let cols = ((extent_x - origin_x) / cell_size).ceil() as usize + 1;
        let rows = ((extent_y - origin_y) / cell_size).ceil() as usize + 1;

        let surface_hm = SurfaceHeightmap::from_mesh(
            &mesh, &si, &cutter, origin_x, origin_y, rows, cols, cell_size, bbox.min.z,
        );
        let slope_map = surface_hm.slope_map();

        // Sample stepover from the slope map at the center
        let so = average_stepover_for_ring(
            &[P2::new(0.0, 0.0), P2::new(10.0, 0.0), P2::new(10.0, 10.0)],
            &slope_map,
            tool_radius,
            scallop_height,
        );

        assert!(
            (so - expected_so).abs() < expected_so * 0.3,
            "Flat surface stepover ({:.3}) should be near flat formula ({:.3})",
            so,
            expected_so
        );
    }

    #[test]
    fn test_scallop_rings_converge() {
        // Rings should progressively shrink until the polygon collapses
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let tool_radius = cutter.radius();

        let bbox = &mesh.bbox;
        let boundary = Polygon2::new(vec![
            P2::new(bbox.min.x, bbox.min.y),
            P2::new(bbox.max.x, bbox.min.y),
            P2::new(bbox.max.x, bbox.max.y),
            P2::new(bbox.min.x, bbox.max.y),
        ]);

        let cell_size = 1.0;
        let origin_x = bbox.min.x - tool_radius;
        let origin_y = bbox.min.y - tool_radius;
        let extent_x = bbox.max.x + tool_radius;
        let extent_y = bbox.max.y + tool_radius;
        let cols = ((extent_x - origin_x) / cell_size).ceil() as usize + 1;
        let rows = ((extent_y - origin_y) / cell_size).ceil() as usize + 1;
        let surface_hm = SurfaceHeightmap::from_mesh(
            &mesh, &si, &cutter, origin_x, origin_y, rows, cols, cell_size, bbox.min.z,
        );
        let slope_map = surface_hm.slope_map();

        let rings = generate_scallop_rings(
            &boundary,
            &mesh,
            &si,
            &cutter,
            &slope_map,
            tool_radius,
            0.1,
            0.0,
            bbox.min.z,
            100,
        );

        assert!(
            rings.len() >= 3,
            "Should produce multiple rings on 50mm flat, got {}",
            rings.len()
        );

        // Ring count should be bounded (polygon eventually collapses)
        let flat_so = crate::scallop_math::stepover_from_scallop_flat(tool_radius, 0.1);
        let expected_max = (25.0 / flat_so).ceil() as usize + 5; // half extent / stepover
        assert!(
            rings.len() <= expected_max,
            "Too many rings ({}), expected at most ~{}",
            rings.len(),
            expected_max
        );
    }

    #[test]
    fn test_scallop_z_from_dropcutter() {
        // Ring Z values should match drop-cutter queries
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();

        let params = ScallopParams {
            scallop_height: 0.1,
            tolerance: 0.5,
            ..ScallopParams::default()
        };

        let tp = scallop_toolpath(&mesh, &si, &cutter, &params);

        // On flat mesh (z≈0), all cutting Z should be near 0
        for m in &tp.moves {
            if let crate::toolpath::MoveType::Linear { .. } = m.move_type
                && m.target.z < params.safe_z - 1.0
            {
                assert!(
                    m.target.z.abs() < 2.0,
                    "Flat mesh cutting Z should be near 0, got {:.2}",
                    m.target.z
                );
            }
        }
    }

    // ── Integration tests ───────────────────────────────────────────

    #[test]
    fn test_scallop_produces_toolpath() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = ScallopParams {
            scallop_height: 0.5, // Coarse for speed
            tolerance: 0.5,
            ..ScallopParams::default()
        };

        let tp = scallop_toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.moves.len() > 10,
            "Hemisphere scallop should produce moves, got {}",
            tp.moves.len()
        );
        assert!(
            tp.total_cutting_distance() > 10.0,
            "Should have meaningful cutting distance, got {:.1}",
            tp.total_cutting_distance()
        );
    }

    #[test]
    fn test_scallop_continuous_no_rapids_between_rings() {
        let (mesh, si) = make_flat_mesh();
        let cutter = ball_cutter();
        let params = ScallopParams {
            scallop_height: 0.5,
            tolerance: 0.5,
            continuous: true,
            ..ScallopParams::default()
        };

        let tp = scallop_toolpath(&mesh, &si, &cutter, &params);

        // In continuous mode, there should be very few rapids
        // (just the initial approach and final retract)
        let rapid_count = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Rapid))
            .count();

        assert!(
            rapid_count <= 4,
            "Continuous scallop should have minimal rapids, got {}",
            rapid_count
        );
    }

    #[test]
    fn test_scallop_inside_out() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = ScallopParams {
            scallop_height: 0.5,
            tolerance: 0.5,
            direction: ScallopDirection::InsideOut,
            ..ScallopParams::default()
        };

        let tp = scallop_toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.moves.len() > 5,
            "Inside-out scallop should produce moves, got {}",
            tp.moves.len()
        );
    }

    // ── Helper tests ────────────────────────────────────────────────

    #[test]
    fn test_closest_point_idx() {
        let ring = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(10.0, 0.0, 0.0),
            P3::new(10.0, 10.0, 0.0),
            P3::new(0.0, 10.0, 0.0),
        ];
        let target = P3::new(9.0, 9.0, 0.0);
        let idx = closest_point_idx(&ring, &target);
        assert_eq!(idx, 2, "Closest to (9,9) should be index 2 (10,10)");
    }

    #[test]
    fn test_rotate_ring() {
        let ring = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(1.0, 0.0, 0.0),
            P3::new(2.0, 0.0, 0.0),
            P3::new(3.0, 0.0, 0.0),
        ];
        let rotated = rotate_ring(&ring, 2);
        assert!((rotated[0].x - 2.0).abs() < 0.01);
        assert!((rotated[1].x - 3.0).abs() < 0.01);
        assert!((rotated[2].x - 0.0).abs() < 0.01);
        assert!((rotated[3].x - 1.0).abs() < 0.01);
    }
}
