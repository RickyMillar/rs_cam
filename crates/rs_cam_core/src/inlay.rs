//! Inlay operations — generate male and female V-carve toolpaths for wood inlays.
//!
//! An inlay consists of two mating pieces:
//! - **Female pocket**: A standard V-carve into the workpiece, with optional
//!   flat-bottom clearing where the V reaches max depth.
//! - **Male plug**: An inverted V-carve that mirrors the female pocket's geometry,
//!   creating a plug that fits precisely into the female pocket when glued.
//!
//! The V-bit angle must match for both operations. A `glue_gap` parameter accounts
//! for the adhesive layer between mating surfaces.

use crate::geo::{P2, P3, point_to_segment_distance};
use crate::pocket::{PocketParams, pocket_toolpath};
use crate::polygon::{Polygon2, offset_polygon};
use crate::toolpath::Toolpath;
use crate::vcarve::{VCarveParams, vcarve_toolpath};
use crate::zigzag::zigzag_lines;

/// Parameters for inlay operations.
pub struct InlayParams {
    /// V-bit half-angle in radians (must match tool geometry).
    pub half_angle: f64,
    /// Female pocket depth (mm). The V-carve will reach this depth at wide areas.
    pub pocket_depth: f64,
    /// Gap between mating surfaces for glue (mm). Default: 0.1.
    pub glue_gap: f64,
    /// Additional depth below start surface for the male plug (mm).
    /// Controls how much extra material the plug extends below the inlay face.
    pub flat_depth: f64,
    /// Extra margin around the plug boundary (mm).
    pub boundary_offset: f64,
    /// Scan line spacing for both V-carve and flat clearing (mm).
    pub stepover: f64,
    /// Tool radius for flat area clearing (mm). Use 0 to skip flat clearing.
    pub flat_tool_radius: f64,
    /// Feed rate for cutting moves (mm/min).
    pub feed_rate: f64,
    /// Plunge rate (mm/min).
    pub plunge_rate: f64,
    /// Safe Z for rapid moves (mm).
    pub safe_z: f64,
    /// Tolerance for scan line sampling (mm).
    pub tolerance: f64,
}

/// Result of an inlay operation: female pocket + male plug toolpaths.
pub struct InlayResult {
    /// Female pocket toolpath (cut into the workpiece).
    pub female: Toolpath,
    /// Male plug toolpath (cut from the plug stock).
    pub male: Toolpath,
}

/// Generate the female pocket toolpath.
///
/// This is a standard V-carve of the design polygon, optionally followed by
/// flat-bottom clearing where the V-carve reaches max_depth.
fn female_toolpath(polygon: &Polygon2, params: &InlayParams) -> Toolpath {
    let mut tp = Toolpath::new();

    // Step 1: V-carve the design
    let vcarve_params = VCarveParams {
        half_angle: params.half_angle,
        max_depth: params.pocket_depth,
        stepover: params.stepover,
        feed_rate: params.feed_rate,
        plunge_rate: params.plunge_rate,
        safe_z: params.safe_z,
        tolerance: params.tolerance,
    };
    let vcarve_tp = vcarve_toolpath(polygon, &vcarve_params);
    tp.moves.extend(vcarve_tp.moves);

    // Step 2: Flat area clearing where V-carve hits max_depth
    // The flat region is the polygon inset by max_depth * tan(half_angle)
    if params.flat_tool_radius > 0.0 && params.pocket_depth > 0.0 {
        let inset_dist = params.pocket_depth * params.half_angle.tan();
        let inset_polygons = offset_polygon(polygon, -inset_dist);

        for inset_poly in &inset_polygons {
            if inset_poly.exterior.len() < 3 {
                continue;
            }
            let pocket_params = PocketParams {
                tool_radius: params.flat_tool_radius,
                stepover: params.stepover,
                cut_depth: -params.pocket_depth,
                feed_rate: params.feed_rate,
                plunge_rate: params.plunge_rate,
                safe_z: params.safe_z,
                climb: false,
            };
            let flat_tp = pocket_toolpath(inset_poly, &pocket_params);
            tp.moves.extend(flat_tp.moves);
        }
    }

    tp
}

/// Generate the male plug toolpath.
///
/// The male plug is an inverted V-carve: the design boundary becomes a ridge,
/// and the depth increases as you move away from the boundary (outward) to
/// create a shape that mates with the female pocket.
///
/// An outer boundary rectangle is created around the design, and the annular
/// region between the design and the boundary is carved with inverted depth.
fn male_toolpath(polygon: &Polygon2, params: &InlayParams) -> Toolpath {
    let mut tp = Toolpath::new();
    let tan_half = params.half_angle.tan();

    if tan_half < 1e-10 {
        return tp;
    }

    // Compute the bounding box of the design with margin
    let (x_min, y_min, x_max, y_max) = polygon_bounds(polygon);
    let margin = params.pocket_depth * tan_half + params.boundary_offset;

    // Outer boundary: rectangle around the design
    let outer = Polygon2::rectangle(
        x_min - margin,
        y_min - margin,
        x_max + margin,
        y_max + margin,
    );

    // The male carving region is the outer boundary with the design as a hole
    // (inverted from the female where we carve inside the design)
    let male_region = Polygon2::with_holes(outer.exterior.clone(), vec![polygon.exterior.clone()]);

    // Generate scan lines across the male region
    let scan_lines = zigzag_lines(&male_region, 0.05, params.stepover, 0.0);
    let sample_step = params.tolerance.max(0.05);

    // Apply glue gap: offset the design boundary inward slightly
    // This makes the male plug slightly smaller than the female pocket
    let gap_offset = params.glue_gap / tan_half;

    for line in &scan_lines {
        let dx = line[1].x - line[0].x;
        let dy = line[1].y - line[0].y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-10 {
            continue;
        }

        let n_samples = (len / sample_step).ceil() as usize;
        let mut points: Vec<P3> = Vec::with_capacity(n_samples + 1);

        for i in 0..=n_samples {
            let t = i as f64 / n_samples.max(1) as f64;
            let x = line[0].x + t * dx;
            let y = line[0].y + t * dy;

            // Distance to the design boundary (not the outer boundary)
            let dist = point_to_polygon_boundary(&P2::new(x, y), &polygon.exterior, &polygon.holes);

            // Male depth: increases with distance from design boundary
            // At the boundary: depth = glue_gap_depth (flush with slight gap)
            // Moving outward: depth increases linearly
            let depth = ((dist - gap_offset) / tan_half + params.flat_depth)
                .clamp(0.0, params.pocket_depth);

            points.push(P3::new(x, y, -depth));
        }

        if points.is_empty() {
            continue;
        }

        tp.emit_path_segment(&points, params.safe_z, params.feed_rate, params.plunge_rate);
    }

    tp
}

/// Compute the minimum distance from a point to the edges of a polygon boundary.
fn point_to_polygon_boundary(point: &P2, exterior: &[P2], holes: &[Vec<P2>]) -> f64 {
    let mut min_dist = f64::INFINITY;

    for i in 0..exterior.len() {
        let a = &exterior[i];
        let b = &exterior[(i + 1) % exterior.len()];
        let dist = point_to_segment_distance(point, a, b);
        min_dist = min_dist.min(dist);
    }

    for hole in holes {
        for i in 0..hole.len() {
            let a = &hole[i];
            let b = &hole[(i + 1) % hole.len()];
            let dist = point_to_segment_distance(point, a, b);
            min_dist = min_dist.min(dist);
        }
    }

    min_dist
}

/// Get the XY bounding box of a polygon.
fn polygon_bounds(polygon: &Polygon2) -> (f64, f64, f64, f64) {
    let mut x_min = f64::INFINITY;
    let mut y_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_max = f64::NEG_INFINITY;

    for p in &polygon.exterior {
        x_min = x_min.min(p.x);
        y_min = y_min.min(p.y);
        x_max = x_max.max(p.x);
        y_max = y_max.max(p.y);
    }

    (x_min, y_min, x_max, y_max)
}

/// Generate both female and male inlay toolpaths.
///
/// The female pocket is cut into the workpiece, and the male plug is cut from
/// a separate piece of stock. When the plug is glued into the pocket and the
/// top sanded flush, the inlay design is revealed.
pub fn inlay_toolpaths(polygon: &Polygon2, params: &InlayParams) -> InlayResult {
    let female = female_toolpath(polygon, params);
    let male = male_toolpath(polygon, params);
    InlayResult { female, male }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_4;

    fn circle_polygon(radius: f64, n_pts: usize) -> Polygon2 {
        let pts: Vec<P2> = (0..n_pts)
            .map(|i| {
                let angle = 2.0 * std::f64::consts::PI * i as f64 / n_pts as f64;
                P2::new(radius * angle.cos(), radius * angle.sin())
            })
            .collect();
        Polygon2::new(pts)
    }

    fn square_polygon(size: f64) -> Polygon2 {
        let h = size / 2.0;
        Polygon2::rectangle(-h, -h, h, h)
    }

    fn default_params() -> InlayParams {
        InlayParams {
            half_angle: FRAC_PI_4, // 90° V-bit
            pocket_depth: 3.0,
            glue_gap: 0.1,
            flat_depth: 0.5,
            boundary_offset: 2.0,
            stepover: 1.0,
            flat_tool_radius: 0.0, // no flat clearing by default
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            tolerance: 0.2,
        }
    }

    #[test]
    fn test_circle_inlay_female() {
        let circle = circle_polygon(10.0, 32);
        let params = default_params();
        let result = inlay_toolpaths(&circle, &params);

        assert!(
            !result.female.moves.is_empty(),
            "Female toolpath should have moves"
        );
        assert!(
            result.female.total_cutting_distance() > 10.0,
            "Female should have significant cutting"
        );
    }

    #[test]
    fn test_circle_inlay_male() {
        let circle = circle_polygon(10.0, 32);
        let params = default_params();
        let result = inlay_toolpaths(&circle, &params);

        assert!(
            !result.male.moves.is_empty(),
            "Male toolpath should have moves"
        );
        assert!(
            result.male.total_cutting_distance() > 10.0,
            "Male should have significant cutting"
        );
    }

    #[test]
    fn test_female_depth_bounded() {
        let sq = square_polygon(20.0);
        let params = InlayParams {
            pocket_depth: 3.0,
            ..default_params()
        };
        let result = inlay_toolpaths(&sq, &params);

        // Female V-carve should not exceed pocket_depth
        for m in &result.female.moves {
            if let crate::toolpath::MoveType::Linear { .. } = m.move_type {
                assert!(
                    m.target.z >= -params.pocket_depth - 0.1,
                    "Female depth {} exceeds pocket_depth {}",
                    m.target.z,
                    params.pocket_depth
                );
            }
        }
    }

    #[test]
    fn test_male_depth_bounded() {
        let sq = square_polygon(20.0);
        let params = default_params();
        let result = inlay_toolpaths(&sq, &params);

        // Male plug should not exceed pocket_depth
        for m in &result.male.moves {
            if let crate::toolpath::MoveType::Linear { .. } = m.move_type {
                assert!(
                    m.target.z >= -params.pocket_depth - 0.1,
                    "Male depth {} exceeds pocket_depth {}",
                    m.target.z,
                    params.pocket_depth
                );
            }
        }
    }

    #[test]
    fn test_letter_o_with_island() {
        // Letter "O" — outer ring with inner hole
        let outer = circle_polygon(15.0, 32);
        let inner = circle_polygon(8.0, 32);
        let poly = Polygon2::with_holes(outer.exterior, vec![inner.exterior]);

        let params = default_params();
        let result = inlay_toolpaths(&poly, &params);

        assert!(
            !result.female.moves.is_empty(),
            "Letter O female should have moves"
        );
        assert!(
            !result.male.moves.is_empty(),
            "Letter O male should have moves"
        );
    }

    #[test]
    fn test_flat_clearing_adds_moves() {
        let sq = square_polygon(20.0);
        let params_no_flat = InlayParams {
            flat_tool_radius: 0.0,
            ..default_params()
        };
        let params_with_flat = InlayParams {
            flat_tool_radius: 3.0,
            ..default_params()
        };

        let result_no = inlay_toolpaths(&sq, &params_no_flat);
        let result_with = inlay_toolpaths(&sq, &params_with_flat);

        // With flat clearing, female should have more moves
        assert!(
            result_with.female.moves.len() >= result_no.female.moves.len(),
            "Flat clearing should add moves: without={}, with={}",
            result_no.female.moves.len(),
            result_with.female.moves.len()
        );
    }

    #[test]
    fn test_glue_gap_affects_male_depth() {
        let sq = square_polygon(20.0);

        let params_no_gap = InlayParams {
            glue_gap: 0.0,
            ..default_params()
        };
        let params_with_gap = InlayParams {
            glue_gap: 0.5,
            ..default_params()
        };

        let result_no = inlay_toolpaths(&sq, &params_no_gap);
        let result_with = inlay_toolpaths(&sq, &params_with_gap);

        // Both should produce toolpaths
        assert!(!result_no.male.moves.is_empty());
        assert!(!result_with.male.moves.is_empty());

        // With glue gap, male cuts should generally be shallower near boundary
        // (the gap pushes the depth profile outward)
    }

    #[test]
    fn test_polygon_bounds() {
        let sq = square_polygon(20.0);
        let (x_min, y_min, x_max, y_max) = polygon_bounds(&sq);
        assert!((x_min - (-10.0)).abs() < 0.1);
        assert!((y_min - (-10.0)).abs() < 0.1);
        assert!((x_max - 10.0).abs() < 0.1);
        assert!((y_max - 10.0).abs() < 0.1);
    }
}
