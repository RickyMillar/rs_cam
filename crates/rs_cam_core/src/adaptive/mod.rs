//! Adaptive clearing with constant engagement.
//!
//! Generates toolpaths that maintain a target engagement angle by making
//! local decisions about direction at each step. Unlike pocket (contour-parallel)
//! or zigzag (scan-line), adaptive dynamically adjusts the path to keep
//! constant tool load.
//!
//! Algorithm overview (Freesteel/Adaptive2d inspired):
//! 1. Build a material grid from the input polygon
//! 2. Find an entry point on the boundary of uncut material
//! 3. At each step, search for a direction producing target engagement
//! 4. When blocked, find the next uncut region and re-enter
//! 5. Repeat until all material is cleared
//!
//! Reference: research/02_algorithms.md §5

mod material_grid;
mod path;
mod search;

pub(crate) use material_grid::MaterialGrid;
use path::{adaptive_segments_with_debug, runtime_annotations_to_labels, segments_to_toolpath};

pub(crate) use crate::adaptive_shared::{
    angle_diff, average_angles, blend_corners_to_moves, refine_angle_bracket,
    target_engagement_fraction,
};
use crate::debug_trace::ToolpathDebugContext;
use crate::dexel_stock::TriDexelStock;
use crate::interrupt::{CancelCheck, Cancelled};
use crate::polygon::Polygon2;
use crate::toolpath::Toolpath;

/// Parameters for adaptive clearing.
pub struct AdaptiveParams {
    pub tool_radius: f64,
    pub stepover: f64,
    pub cut_depth: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub safe_z: f64,
    pub tolerance: f64,
    /// Enable slot clearing: cut a center slot before adaptive spiral.
    /// Reduces linking motion at corners for some pockets.
    pub slot_clearing: bool,
    /// Minimum cutting radius: blend sharp inside corners with arcs of at
    /// least this radius. Prevents chatter on sharp corners. 0.0 = disabled.
    pub min_cutting_radius: f64,
    /// Optional prior stock state. When provided, the material grid is
    /// initialized from the tri-dexel stock so that cells already cleared
    /// by earlier operations are not re-cut.
    pub initial_stock: Option<TriDexelStock>,
}

/// A segment of the adaptive path: cutting, rapid reposition, or link (tool-down reposition).
#[derive(Debug, Clone, PartialEq)]
pub enum AdaptiveRuntimeEvent {
    SlotClearing {
        line_index: usize,
        line_total: usize,
    },
    PassEntry {
        pass_index: usize,
        entry_x: f64,
        entry_y: f64,
    },
    PassSummary {
        pass_index: usize,
        step_count: usize,
        idle_count: usize,
        search_evaluations: usize,
        exit_reason: String,
    },
    ForcedClear {
        pass_index: usize,
        center_x: f64,
        center_y: f64,
        radius: f64,
    },
    BoundaryCleanup {
        contour_index: usize,
        contour_total: usize,
    },
}

impl AdaptiveRuntimeEvent {
    pub fn label(&self) -> String {
        match self {
            Self::SlotClearing {
                line_index,
                line_total,
            } => format!("Slot clearing {line_index}/{line_total}"),
            Self::PassEntry {
                pass_index,
                entry_x,
                entry_y,
            } => format!("Pass {pass_index} — entry at ({entry_x:.1}, {entry_y:.1})"),
            Self::PassSummary {
                pass_index,
                step_count,
                idle_count,
                search_evaluations,
                exit_reason,
            } => format!(
                "Pass {pass_index} — {step_count} steps ({exit_reason}, idle {idle_count}, search {search_evaluations})"
            ),
            Self::ForcedClear {
                pass_index,
                center_x,
                center_y,
                radius,
            } => format!(
                "Pass {pass_index} — forced clear at ({center_x:.1}, {center_y:.1}) r {radius:.1}"
            ),
            Self::BoundaryCleanup {
                contour_index,
                contour_total,
            } => format!("Boundary cleanup {contour_index}/{contour_total}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AdaptiveRuntimeAnnotation {
    pub move_index: usize,
    pub event: AdaptiveRuntimeEvent,
}

/// Generate an adaptive clearing toolpath for a 2D polygon region.
///
/// The toolpath maintains approximately constant engagement by dynamically
/// adjusting direction at each step. Returns a Toolpath with rapids,
/// plunges, and feeds at the specified cut_depth.
// infallible: cancel closure always returns false, so Cancelled is unreachable
#[allow(clippy::expect_used)]
#[tracing::instrument(skip(polygon, params), fields(
    tool_radius = params.tool_radius,
    stepover = params.stepover,
    cut_depth = params.cut_depth,
))]
pub fn adaptive_toolpath(polygon: &Polygon2, params: &AdaptiveParams) -> Toolpath {
    let never_cancel = || false;
    adaptive_toolpath_with_cancel(polygon, params, &never_cancel)
        .expect("non-cancellable adaptive should never be cancelled")
}

pub fn adaptive_toolpath_with_cancel(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    adaptive_toolpath_traced_with_cancel(polygon, params, cancel, None)
}

pub fn adaptive_toolpath_traced_with_cancel(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    cancel: &dyn CancelCheck,
    debug: Option<&ToolpathDebugContext>,
) -> Result<Toolpath, Cancelled> {
    let (tp, _) =
        adaptive_toolpath_structured_annotated_traced_with_cancel(polygon, params, cancel, debug)?;
    Ok(tp)
}

pub fn adaptive_toolpath_structured_annotated_traced_with_cancel(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    cancel: &dyn CancelCheck,
    debug: Option<&ToolpathDebugContext>,
) -> Result<(Toolpath, Vec<AdaptiveRuntimeAnnotation>), Cancelled> {
    let segments = adaptive_segments_with_debug(polygon, params, cancel, debug)?;
    let (tp, annotations) = segments_to_toolpath(&segments, params);
    if let Some(debug_ctx) = debug {
        for annotation in &annotations {
            debug_ctx.add_annotation(annotation.move_index, annotation.event.label());
        }
    }
    Ok((tp, annotations))
}

pub fn adaptive_toolpath_annotated_traced_with_cancel(
    polygon: &Polygon2,
    params: &AdaptiveParams,
    cancel: &dyn CancelCheck,
    debug: Option<&ToolpathDebugContext>,
) -> Result<(Toolpath, Vec<(usize, String)>), Cancelled> {
    let (tp, annotations) =
        adaptive_toolpath_structured_annotated_traced_with_cancel(polygon, params, cancel, debug)?;
    Ok((tp, runtime_annotations_to_labels(&annotations)))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use std::f64::consts::PI;

    use super::material_grid::{CELL_CLEARED, CELL_MATERIAL};
    use super::path::{AdaptiveSegment, adaptive_segments, is_clear_path, simplify_path};
    use super::search::{compute_engagement, find_entry_point, search_direction};
    use super::*;
    use crate::adaptive_shared::blend_corners;
    use crate::geo::P2;
    use crate::polygon::offset_polygon;

    fn square_polygon(size: f64) -> Polygon2 {
        let h = size / 2.0;
        Polygon2::rectangle(-h, -h, h, h)
    }

    fn default_params(tool_radius: f64, stepover: f64) -> AdaptiveParams {
        AdaptiveParams {
            tool_radius,
            stepover,
            cut_depth: -3.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 10.0,
            tolerance: 0.2,
            slot_clearing: false,
            min_cutting_radius: 0.0,
            initial_stock: None,
        }
    }

    // ── MaterialGrid tests ─────────────────────────────────────────────

    #[test]
    fn test_material_grid_from_square() {
        let sq = square_polygon(20.0);
        let grid = MaterialGrid::from_polygon(&sq, 1.0);

        // Center should be material
        assert!(grid.is_material(0.0, 0.0));
        // Outside should be air
        assert!(!grid.is_material(15.0, 0.0));
        assert!(!grid.is_material(0.0, 15.0));
    }

    #[test]
    fn test_material_grid_with_hole() {
        let hole = vec![
            P2::new(-3.0, -3.0),
            P2::new(-3.0, 3.0),
            P2::new(3.0, 3.0),
            P2::new(3.0, -3.0),
        ]; // CW
        let poly = Polygon2::with_holes(square_polygon(20.0).exterior, vec![hole]);
        let grid = MaterialGrid::from_polygon(&poly, 0.5);

        // Outside should be air
        assert!(!grid.is_material(15.0, 0.0));
        // Inside hole should be air
        assert!(!grid.is_material(0.0, 0.0));
        // Between hole and exterior should be material
        assert!(grid.is_material(7.0, 0.0));
    }

    #[test]
    fn test_material_grid_clear_circle() {
        let sq = square_polygon(20.0);
        let mut grid = MaterialGrid::from_polygon(&sq, 0.5);

        assert!(grid.is_material(0.0, 0.0));
        grid.clear_circle(0.0, 0.0, 3.0);
        assert!(!grid.is_material(0.0, 0.0));
        assert!(!grid.is_material(2.0, 0.0));

        // Far away should still be material
        assert!(grid.is_material(7.0, 7.0));
    }

    #[test]
    fn test_material_fraction_starts_at_one() {
        let sq = square_polygon(20.0);
        let grid = MaterialGrid::from_polygon(&sq, 1.0);
        assert!(grid.material_fraction() > 0.95);
    }

    #[test]
    fn test_material_fraction_decreases_after_clear() {
        let sq = square_polygon(20.0);
        let mut grid = MaterialGrid::from_polygon(&sq, 0.5);

        let before = grid.material_fraction();
        grid.clear_circle(0.0, 0.0, 5.0);
        let after = grid.material_fraction();
        assert!(
            after < before,
            "Material fraction should decrease: {} -> {}",
            before,
            after
        );
    }

    #[test]
    fn test_find_nearest_material() {
        let sq = square_polygon(20.0);
        let mut grid = MaterialGrid::from_polygon(&sq, 0.5);

        // Clear center
        grid.clear_circle(0.0, 0.0, 5.0);

        // Nearest material from center should be ~5mm away
        let (mx, my) = grid.find_nearest_material(0.0, 0.0).unwrap();
        let dist = (mx * mx + my * my).sqrt();
        assert!(
            dist > 4.0 && dist < 7.0,
            "Nearest material should be ~5mm away, got {}",
            dist
        );
    }

    // ── Boundary distance tests ───────────────────────────────────────

    #[test]
    fn test_boundary_distance_center_vs_edge() {
        let sq = square_polygon(20.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);
        let dist = grid.compute_boundary_distances();

        let center_dist = grid.boundary_distance_at(&dist, 0.0, 0.0);
        let edge_dist = grid.boundary_distance_at(&dist, 9.0, 0.0);

        assert!(
            center_dist > edge_dist,
            "Center ({:.1}) should be farther from boundary than edge ({:.1})",
            center_dist,
            edge_dist
        );
        // Center of 20x20 square: ~10 cells from boundary at 0.5 cell_size = ~5.0
        assert!(
            center_dist > 4.0,
            "Center distance should be significant, got {:.1}",
            center_dist
        );
        // Near edge (9.0 from center, wall at 10.0): ~1mm from boundary
        assert!(
            edge_dist < 3.0,
            "Edge distance should be small, got {:.1}",
            edge_dist
        );
    }

    #[test]
    fn test_boundary_distance_air_is_zero() {
        let sq = square_polygon(10.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);
        let dist = grid.compute_boundary_distances();

        // Well outside the polygon → AIR → distance 0
        let air_dist = grid.boundary_distance_at(&dist, 20.0, 20.0);
        assert!(
            air_dist < 0.01,
            "AIR cell should have distance 0, got {}",
            air_dist
        );
    }

    #[test]
    fn test_boundary_gradient_points_inward() {
        let sq = square_polygon(20.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);
        let dist = grid.compute_boundary_distances();

        // Near the right wall (x ≈ 9): gradient should point left (negative x)
        let (gx, _gy) = grid.boundary_gradient(&dist, 9.0, 0.0);
        // Gradient points toward increasing distance = away from wall = inward
        // But we're near the right wall, so inward = negative x? Actually no:
        // gradient points in the direction of increasing distance, which is toward
        // the interior. At x=9 (near right wall at x=10), increasing distance is
        // toward the left (negative x direction).
        // Wait - the boundary distance increases as you move AWAY from the wall.
        // So the gradient points away from the wall = toward interior.
        // At x=9 near the right wall: gradient x should be negative (pointing left = inward).
        // Actually let me think again. The wall is air at x>10. Distance increases as you
        // go from x=10 toward x=0 (away from the air boundary). So at x=9, the gradient
        // should point toward x=0, which is negative x.
        assert!(
            gx < -0.1,
            "Near right wall, gradient x should be negative (inward), got {:.2}",
            gx
        );
    }

    // ── Engagement computation tests ───────────────────────────────────

    #[test]
    fn test_engagement_full_material() {
        let sq = square_polygon(40.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);

        // Center of large square, small tool → should be ~1.0
        let eng = compute_engagement(&grid, 0.0, 0.0, 3.0);
        assert!(
            eng > 0.9,
            "Fully surrounded should have near-1.0 engagement, got {}",
            eng
        );
    }

    #[test]
    fn test_engagement_no_material() {
        let sq = square_polygon(10.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);

        // Far outside
        let eng = compute_engagement(&grid, 50.0, 50.0, 3.0);
        assert!(
            eng < 0.01,
            "No material should have 0 engagement, got {}",
            eng
        );
    }

    #[test]
    fn test_engagement_partial() {
        let sq = square_polygon(20.0);
        let mut grid = MaterialGrid::from_polygon(&sq, 0.5);

        // Clear a channel through center
        for i in -20..=20 {
            let x = i as f64 * 0.5;
            grid.clear_circle(x, 0.0, 2.0);
        }

        // Engagement at the edge of the channel should be partial
        let eng = compute_engagement(&grid, 0.0, 2.5, 3.0);
        assert!(
            eng > 0.1 && eng < 0.9,
            "Edge of channel should have partial engagement, got {}",
            eng
        );
    }

    #[test]
    fn test_target_engagement_fraction() {
        // 20% stepover on 3.175mm radius tool
        let frac = target_engagement_fraction(1.27, 3.175);
        assert!(
            frac > 0.05 && frac < 0.25,
            "20% stepover should give small engagement fraction, got {}",
            frac
        );

        // Full slot (WOC = diameter) → engagement should be 0.5 (half circle)
        let frac_full = target_engagement_fraction(6.35, 3.175);
        assert!(
            (frac_full - 0.5).abs() < 0.01,
            "Full slot should give 0.5 engagement fraction, got {}",
            frac_full
        );
    }

    // ── Direction search tests ─────────────────────────────────────────

    #[test]
    fn test_search_direction_finds_material() {
        let sq = square_polygon(40.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);
        let boundary_dist = grid.compute_boundary_distances();

        // Machinable = inset by tool radius
        let machinable = offset_polygon(&sq, 3.0);
        assert!(!machinable.is_empty());
        let mask = MaterialGrid::build_machinable_mask(
            &machinable[0],
            grid.origin_x,
            grid.origin_y,
            grid.rows,
            grid.cols,
            grid.cell_size,
        );

        let target = target_engagement_fraction(1.5, 3.0);
        let angle = search_direction(
            &grid,
            &mask,
            0.0,
            0.0,
            3.0,
            1.0,
            target,
            0.0,
            &boundary_dist,
        );
        assert!(angle.is_some(), "Should find a direction in open material");
    }

    #[test]
    fn test_search_direction_blocked_outside() {
        let sq = square_polygon(10.0);
        let mut grid = MaterialGrid::from_polygon(&sq, 0.5);
        let boundary_dist = grid.compute_boundary_distances();

        // Clear everything
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                if grid.cells[row * grid.cols + col] == CELL_MATERIAL {
                    grid.cells[row * grid.cols + col] = CELL_CLEARED;
                }
            }
        }

        let machinable = offset_polygon(&sq, 2.0);
        if machinable.is_empty() {
            return; // polygon too small for tool
        }
        let mask = MaterialGrid::build_machinable_mask(
            &machinable[0],
            grid.origin_x,
            grid.origin_y,
            grid.rows,
            grid.cols,
            grid.cell_size,
        );
        let target = target_engagement_fraction(1.0, 2.0);
        let angle = search_direction(
            &grid,
            &mask,
            0.0,
            0.0,
            2.0,
            0.5,
            target,
            0.0,
            &boundary_dist,
        );
        assert!(
            angle.is_none(),
            "Should be blocked when no material remains"
        );
    }

    #[test]
    fn test_search_direction_wall_tangent_bias_applied() {
        // Verify that the wall-tangent bias adds a scoring penalty for
        // perpendicular movement near walls. We test the boundary distance
        // and gradient mechanics rather than the full search outcome
        // (which depends on engagement differences too).
        let sq = square_polygon(20.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);
        let boundary_dist = grid.compute_boundary_distances();

        // Near the left wall at x=-9 (wall at x=-10): boundary_distance < 2*tool_radius
        let bd = grid.boundary_distance_at(&boundary_dist, -9.0, 0.0);
        assert!(
            bd < 4.0,
            "Near wall, boundary distance should be small, got {:.1}",
            bd
        );

        // Gradient should point away from the wall (positive x = inward)
        let (gx, _gy) = grid.boundary_gradient(&boundary_dist, -9.0, 0.0);
        assert!(
            gx > 0.1,
            "Near left wall, gradient should point right (inward), got gx={:.2}",
            gx
        );

        // Verify search_direction works near a wall (finds a direction)
        let machinable = offset_polygon(&sq, 2.0);
        if machinable.is_empty() {
            return;
        }
        let mask = MaterialGrid::build_machinable_mask(
            &machinable[0],
            grid.origin_x,
            grid.origin_y,
            grid.rows,
            grid.cols,
            grid.cell_size,
        );
        let target = target_engagement_fraction(1.5, 2.0);
        let angle = search_direction(
            &grid,
            &mask,
            -7.0,
            0.0,
            2.0,
            1.0,
            target,
            0.0,
            &boundary_dist,
        );
        assert!(angle.is_some(), "Should find a direction near wall");
    }

    // ── Entry point spreading tests ───────────────────────────────────

    #[test]
    fn test_entry_points_spread() {
        let sq = square_polygon(20.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);
        let tool_radius = 2.5;

        let machinable = offset_polygon(&sq, tool_radius);
        if machinable.is_empty() {
            return;
        }
        let mask = MaterialGrid::build_machinable_mask(
            &machinable[0],
            grid.origin_x,
            grid.origin_y,
            grid.rows,
            grid.cols,
            grid.cell_size,
        );

        // First entry: no previous endpoints
        let e1 = find_entry_point(&grid, &mask, &machinable[0], tool_radius, None, &[]);
        assert!(e1.is_some());
        let e1 = e1.unwrap();

        // Second entry: should avoid being close to the first
        let e2 = find_entry_point(&grid, &mask, &machinable[0], tool_radius, Some(e1), &[e1]);
        assert!(e2.is_some());
        let e2 = e2.unwrap();

        let dx = e2.x - e1.x;
        let dy = e2.y - e1.y;
        let dist = (dx * dx + dy * dy).sqrt();
        // The second entry should be at least some distance from the first
        // (not right on top of it, though it may still be nearby if material is concentrated)
        assert!(
            dist > 0.1,
            "Second entry should be spread from first, dist={:.1}",
            dist
        );
    }

    // ── Path simplification tests ──────────────────────────────────────

    #[test]
    fn test_simplify_straight_line() {
        let pts: Vec<P2> = (0..=10).map(|i| P2::new(i as f64, 0.0)).collect();
        let simplified = simplify_path(&pts, 0.01);
        assert_eq!(
            simplified.len(),
            2,
            "Straight line should simplify to 2 points"
        );
    }

    #[test]
    fn test_simplify_preserves_corners() {
        let pts = vec![
            P2::new(0.0, 0.0),
            P2::new(5.0, 0.0),
            P2::new(5.0, 5.0),
            P2::new(10.0, 5.0),
        ];
        let simplified = simplify_path(&pts, 0.1);
        assert!(simplified.len() >= 3, "L-shape should preserve the corner");
    }

    // ── Blend corners tests ────────────────────────────────────────────

    #[test]
    fn test_blend_corners_sharp_turn() {
        // L-shape: 90° turn
        let path = vec![P2::new(0.0, 0.0), P2::new(10.0, 0.0), P2::new(10.0, 10.0)];
        let blended = blend_corners(&path, 2.0);
        // Should add arc points at the corner
        assert!(
            blended.len() > 3,
            "90° corner should get blend points, got {} points",
            blended.len()
        );
        // First and last points should be preserved
        assert!((blended[0].x - 0.0).abs() < 1e-10);
        assert!((blended.last().unwrap().y - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_blend_corners_straight_line_unchanged() {
        let path = vec![P2::new(0.0, 0.0), P2::new(5.0, 0.0), P2::new(10.0, 0.0)];
        let blended = blend_corners(&path, 2.0);
        // Nearly straight → no blending, should be 3 points (start, corner, end)
        assert_eq!(blended.len(), 3, "Straight line should not be blended");
    }

    #[test]
    fn test_blend_corners_disabled_when_zero() {
        let path = vec![P2::new(0.0, 0.0), P2::new(10.0, 0.0), P2::new(10.0, 10.0)];
        let blended = blend_corners(&path, 0.0);
        assert_eq!(blended.len(), path.len(), "Zero radius should not blend");
    }

    #[test]
    fn test_blend_corners_radius_too_large() {
        // Very short segments, large radius → setback won't fit
        let path = vec![P2::new(0.0, 0.0), P2::new(1.0, 0.0), P2::new(1.0, 1.0)];
        let blended = blend_corners(&path, 10.0);
        // Radius too large for the segments → corner preserved unblended
        assert_eq!(
            blended.len(),
            3,
            "Too-large radius should not blend short segments"
        );
    }

    // ── Blend corners to moves (arc emission) tests ───────────────────

    #[test]
    fn test_blend_corners_to_moves_emits_arc() {
        use crate::adaptive_shared::BlendedMove;
        // L-shape: 90° turn
        let path = vec![P2::new(0.0, 0.0), P2::new(10.0, 0.0), P2::new(10.0, 10.0)];
        let moves = blend_corners_to_moves(&path, 2.0);

        // Should contain at least one Arc move
        let arc_count = moves
            .iter()
            .filter(|m| matches!(m, BlendedMove::Arc { .. }))
            .count();
        assert!(
            arc_count > 0,
            "90° corner should produce at least one Arc move, got {arc_count}"
        );

        // First move should be Linear(start), last should be Linear(end)
        assert!(matches!(&moves[0], BlendedMove::Linear(p) if p.x.abs() < 1e-10));
        assert!(
            matches!(moves.last().unwrap(), BlendedMove::Linear(p) if (p.y - 10.0).abs() < 1e-10)
        );
    }

    #[test]
    fn test_blend_corners_to_moves_arc_center_on_radius() {
        use crate::adaptive_shared::BlendedMove;
        let path = vec![P2::new(0.0, 0.0), P2::new(10.0, 0.0), P2::new(10.0, 10.0)];
        let min_r = 2.0;
        let moves = blend_corners_to_moves(&path, min_r);

        for (i, m) in moves.iter().enumerate() {
            if let BlendedMove::Arc { end, center, .. } = m {
                // Arc endpoint should be at min_radius from center
                let dx = end.x - center.x;
                let dy = end.y - center.y;
                let dist = (dx * dx + dy * dy).sqrt();
                assert!(
                    (dist - min_r).abs() < 0.01,
                    "Arc move {i}: endpoint should be {min_r} from center, got {dist:.4}"
                );
            }
        }
    }

    #[test]
    fn test_blend_corners_to_moves_straight_no_arc() {
        use crate::adaptive_shared::BlendedMove;
        let path = vec![P2::new(0.0, 0.0), P2::new(5.0, 0.0), P2::new(10.0, 0.0)];
        let moves = blend_corners_to_moves(&path, 2.0);
        let arc_count = moves
            .iter()
            .filter(|m| matches!(m, BlendedMove::Arc { .. }))
            .count();
        assert_eq!(arc_count, 0, "Straight line should produce no arcs");
    }

    // ── Slot clearing tests ────────────────────────────────────────────

    #[test]
    fn test_slot_clearing_reduces_material() {
        let sq = square_polygon(20.0);
        let tool_radius = 2.5;
        let cell_size = 0.5;

        // Without slot clearing
        let grid_no_slot = MaterialGrid::from_polygon(&sq, cell_size);
        let frac_before = grid_no_slot.material_fraction();

        // With slot clearing: run adaptive_segments and check material after slot pass
        let never_cancel = || false;
        let segs = adaptive_segments(&sq, tool_radius, 1.2, 0.2, true, &never_cancel)
            .expect("test helper should not cancel");

        // Verify we got at least one cut segment (the slot)
        let cut_count = segs
            .iter()
            .filter(|s| matches!(s, AdaptiveSegment::Cut(_)))
            .count();
        assert!(
            cut_count >= 1,
            "Slot clearing should produce at least one cut segment"
        );

        // Replay just the first cut segment to verify it clears material
        let mut grid = MaterialGrid::from_polygon(&sq, cell_size);
        if let Some(AdaptiveSegment::Cut(path)) =
            segs.iter().find(|s| matches!(s, AdaptiveSegment::Cut(_)))
        {
            for p in path {
                grid.clear_circle(p.x, p.y, tool_radius);
            }
        }
        let frac_after_slot = grid.material_fraction();
        assert!(
            frac_after_slot < frac_before,
            "Slot should clear material: {:.1}% → {:.1}%",
            frac_before * 100.0,
            frac_after_slot * 100.0
        );
    }

    // ── Full adaptive toolpath tests ───────────────────────────────────

    #[test]
    fn test_adaptive_toolpath_basic() {
        let sq = square_polygon(16.0);
        let params = default_params(2.5, 1.2);

        let tp = adaptive_toolpath(&sq, &params);

        // Should have moves
        assert!(
            tp.moves.len() > 10,
            "Adaptive should generate moves, got {}",
            tp.moves.len()
        );

        // Should have some cutting distance
        assert!(
            tp.total_cutting_distance() > 20.0,
            "Should have significant cutting, got {}",
            tp.total_cutting_distance()
        );
    }

    #[test]
    fn test_adaptive_toolpath_all_at_cut_depth() {
        let sq = square_polygon(16.0);
        let mut params = default_params(2.5, 1.2);
        params.cut_depth = -5.0;

        let tp = adaptive_toolpath(&sq, &params);

        // All feed moves should be at cut_depth
        for m in &tp.moves {
            if let crate::toolpath::MoveType::Linear { feed_rate } = m.move_type
                && feed_rate > 500.0
            {
                // cutting move (not plunge)
                assert!(
                    (m.target.z - (-5.0)).abs() < 1e-10,
                    "Cutting move should be at cut_depth, got z={}",
                    m.target.z
                );
            }
        }
    }

    #[test]
    fn test_adaptive_too_small_polygon() {
        // Polygon smaller than tool
        let sq = square_polygon(3.0);
        let params = default_params(3.0, 1.5);

        let tp = adaptive_toolpath(&sq, &params);
        // Should gracefully return empty or minimal toolpath
        assert!(
            tp.moves.len() <= 2,
            "Too-small polygon should produce minimal toolpath"
        );
    }

    #[test]
    fn test_adaptive_clears_most_material() {
        let sq = square_polygon(16.0);
        let cell_size = 0.5;
        let tool_radius = 2.5;

        let never_cancel = || false;
        let segments = adaptive_segments(&sq, tool_radius, 1.2, 0.2, false, &never_cancel)
            .expect("test helper should not cancel");

        // Build a material grid and replay the segments to check coverage
        let mut grid = MaterialGrid::from_polygon(&sq, cell_size);
        for seg in &segments {
            if let AdaptiveSegment::Cut(path) = seg {
                for p in path {
                    grid.clear_circle(p.x, p.y, tool_radius);
                }
            }
        }

        let remaining = grid.material_fraction();
        assert!(
            remaining < 0.15,
            "Adaptive should clear most material, {:.1}% remaining",
            remaining * 100.0
        );
    }

    #[test]
    fn test_adaptive_with_slot_clearing() {
        let sq = square_polygon(16.0);
        let mut params = default_params(2.5, 1.2);
        params.slot_clearing = true;

        let tp = adaptive_toolpath(&sq, &params);

        assert!(
            tp.moves.len() > 10,
            "Adaptive+slot should generate moves, got {}",
            tp.moves.len()
        );
        assert!(
            tp.total_cutting_distance() > 20.0,
            "Should have significant cutting with slot, got {}",
            tp.total_cutting_distance()
        );
    }

    #[test]
    fn test_adaptive_with_min_cutting_radius() {
        let sq = square_polygon(16.0);
        let mut params = default_params(2.5, 1.2);
        params.min_cutting_radius = 1.0;

        let tp = adaptive_toolpath(&sq, &params);

        assert!(
            tp.moves.len() > 10,
            "Adaptive+blend should generate moves, got {}",
            tp.moves.len()
        );
    }

    // ── Link vs retract tests ──────────────────────────────────────────

    #[test]
    fn test_is_clear_path_cleared_area() {
        let sq = square_polygon(20.0);
        let mut grid = MaterialGrid::from_polygon(&sq, 0.5);
        let tool_radius = 2.5;

        let machinable = offset_polygon(&sq, tool_radius);
        assert!(!machinable.is_empty());
        let mask = MaterialGrid::build_machinable_mask(
            &machinable[0],
            grid.origin_x,
            grid.origin_y,
            grid.rows,
            grid.cols,
            grid.cell_size,
        );

        // Clear a corridor through the center
        for i in -20..=20 {
            let x = i as f64 * 0.5;
            grid.clear_circle(x, 0.0, tool_radius);
        }

        // Path through the cleared corridor should be safe
        let from = P2::new(-5.0, 0.0);
        let to = P2::new(5.0, 0.0);
        assert!(
            is_clear_path(&grid, &mask, from, to, tool_radius),
            "Path through cleared corridor should be safe"
        );
    }

    #[test]
    fn test_is_clear_path_blocked_by_material() {
        let sq = square_polygon(20.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);
        let tool_radius = 2.5;

        let machinable = offset_polygon(&sq, tool_radius);
        assert!(!machinable.is_empty());
        let mask = MaterialGrid::build_machinable_mask(
            &machinable[0],
            grid.origin_x,
            grid.origin_y,
            grid.rows,
            grid.cols,
            grid.cell_size,
        );

        // Uncleared grid — path through material should be blocked
        let from = P2::new(-5.0, 0.0);
        let to = P2::new(5.0, 0.0);
        assert!(
            !is_clear_path(&grid, &mask, from, to, tool_radius),
            "Path through uncut material should be blocked"
        );
    }

    #[test]
    fn test_link_reduces_rapids() {
        let sq = square_polygon(16.0);
        let params = default_params(2.5, 1.2);

        let tp = adaptive_toolpath(&sq, &params);

        // Count rapid moves (retract + reposition)
        let _rapid_count = tp
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, crate::toolpath::MoveType::Rapid))
            .count();

        // With linking, there should be fewer rapids than passes * 2
        // (each retract+reposition pair = 2 rapids; links eliminate both)
        let never_cancel = || false;
        let segments = adaptive_segments(&sq, 2.5, 1.2, 0.2, false, &never_cancel)
            .expect("test helper should not cancel");
        let total_entries = segments
            .iter()
            .filter(|s| matches!(s, AdaptiveSegment::Rapid(_) | AdaptiveSegment::Link(_)))
            .count();
        let link_count = segments
            .iter()
            .filter(|s| matches!(s, AdaptiveSegment::Link(_)))
            .count();

        // Should have at least some links (nearby passes in cleared area)
        assert!(
            link_count > 0 || total_entries <= 2,
            "Should produce links between nearby passes, got {} links / {} entries",
            link_count,
            total_entries
        );
    }

    // ── Coarse scan direction search tests ────────────────────────────

    #[test]
    fn test_search_coarse_finds_uturn() {
        // Full material square, tool at center, prev_angle pointing +X.
        // Coarse 360° scan must find a valid direction (since material is everywhere).
        let sq = square_polygon(30.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);
        let boundary_dist = grid.compute_boundary_distances();

        let machinable = offset_polygon(&sq, 2.5);
        assert!(!machinable.is_empty());
        let mask = MaterialGrid::build_machinable_mask(
            &machinable[0],
            grid.origin_x,
            grid.origin_y,
            grid.rows,
            grid.cols,
            grid.cell_size,
        );

        let target = target_engagement_fraction(1.2, 2.5);
        // prev_angle = PI (pointing -X) — narrow search should fail on some configs,
        // coarse scan covers full 360°
        let angle = search_direction(
            &grid,
            &mask,
            0.0,
            0.0,
            2.5,
            0.75,
            target,
            PI,
            &boundary_dist,
        );
        assert!(
            angle.is_some(),
            "Coarse scan should find a direction in full material"
        );
    }

    #[test]
    fn test_search_coarse_engagement_result() {
        // Verify that the direction found by the coarse scan actually
        // leads to a position with engagement within the target tolerance.
        let sq = square_polygon(40.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);
        let boundary_dist = grid.compute_boundary_distances();

        let tool_radius = 3.0;
        let machinable = offset_polygon(&sq, tool_radius);
        assert!(!machinable.is_empty());
        let mask = MaterialGrid::build_machinable_mask(
            &machinable[0],
            grid.origin_x,
            grid.origin_y,
            grid.rows,
            grid.cols,
            grid.cell_size,
        );

        let step_len = grid.cell_size * 1.5;
        let target = target_engagement_fraction(1.5, tool_radius);
        let angle = search_direction(
            &grid,
            &mask,
            0.0,
            0.0,
            tool_radius,
            step_len,
            target,
            0.0,
            &boundary_dist,
        );
        assert!(angle.is_some(), "Should find direction in open material");

        // Verify engagement at destination
        let a = angle.unwrap();
        let nx = step_len * a.cos();
        let ny = step_len * a.sin();
        let eng = compute_engagement(&grid, nx, ny, tool_radius);
        assert!(
            eng > 0.005,
            "Destination should have non-zero engagement, got {:.4}",
            eng
        );
    }

    // ── Growing-radius entry point tests ──────────────────────────────

    #[test]
    fn test_find_material_radius_finds_cluster() {
        // Material in one corner only, search from far away.
        let sq = Polygon2::rectangle(0.0, 0.0, 40.0, 40.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);

        // Clear everything except a 5×5 cluster in the top-right corner
        // by creating a new grid and keeping only the corner
        let mut grid2 = MaterialGrid::from_polygon(&sq, 0.5);
        for r in 0..grid2.rows {
            let y = grid2.origin_y + r as f64 * grid2.cell_size;
            for c in 0..grid2.cols {
                let x = grid2.origin_x + c as f64 * grid2.cell_size;
                if !(x > 33.0 && y > 33.0) && grid2.cells[r * grid2.cols + c] == CELL_MATERIAL {
                    grid2.cells[r * grid2.cols + c] = CELL_CLEARED;
                    grid2.material_count -= 1;
                }
            }
        }

        // Search from (5, 5) — far from the cluster
        let result = grid2.find_nearest_material(5.0, 5.0);
        assert!(
            result.is_some(),
            "Growing-radius search should find distant material"
        );
        let (mx, my) = result.unwrap();
        assert!(
            mx > 30.0 && my > 30.0,
            "Found material should be in the cluster at ({}, {})",
            mx,
            my
        );

        // Verify the original grid still works (regression)
        let result2 = grid.find_nearest_material(5.0, 5.0);
        assert!(result2.is_some(), "Full grid should find nearby material");
        let (mx2, my2) = result2.unwrap();
        let dist = ((mx2 - 5.0).powi(2) + (my2 - 5.0).powi(2)).sqrt();
        assert!(
            dist < 2.0,
            "Nearby material should be very close, got dist={:.1}",
            dist
        );
    }

    #[test]
    fn test_find_material_radius_nearby() {
        // Full material grid — nearest should be found immediately with small radius.
        let sq = square_polygon(20.0);
        let grid = MaterialGrid::from_polygon(&sq, 0.5);

        let result = grid.find_nearest_material(0.0, 0.0);
        assert!(result.is_some(), "Should find nearby material");
        let (mx, my) = result.unwrap();
        let dist = (mx * mx + my * my).sqrt();
        assert!(
            dist < 1.0,
            "Center of full grid should find material right there, got dist={:.1}",
            dist
        );
    }

    #[test]
    fn traced_adaptive_emits_pass_spans_and_hotspots() {
        let poly = square_polygon(20.0);
        let params = AdaptiveParams {
            slot_clearing: true,
            ..default_params(2.0, 1.5)
        };
        let recorder = crate::debug_trace::ToolpathDebugRecorder::new("Adaptive", "2D Rough");
        let ctx = recorder.root_context();
        let never_cancel = || false;

        let tp = adaptive_toolpath_traced_with_cancel(&poly, &params, &never_cancel, Some(&ctx))
            .expect("debug run should complete");
        let trace = recorder.finish();

        assert!(!tp.moves.is_empty(), "expected a non-empty toolpath");
        assert!(trace.spans.iter().any(|span| span.kind == "slot_clearing"));
        assert!(trace.spans.iter().any(|span| span.kind == "adaptive_pass"));
        assert!(
            trace
                .spans
                .iter()
                .any(|span| span.kind == "boundary_cleanup")
        );
        assert!(
            trace
                .spans
                .iter()
                .filter(|span| span.kind == "adaptive_pass")
                .any(|span| span.exit_reason.is_some()),
            "adaptive pass spans should record exit reasons"
        );
        assert!(
            trace
                .hotspots
                .iter()
                .any(|hotspot| hotspot.kind == "adaptive_pass"),
            "adaptive trace should record at least one hotspot"
        );
    }

    #[test]
    fn initial_stock_reduces_adaptive_moves() {
        use crate::geo::{BoundingBox3, P3};

        let poly = square_polygon(20.0);
        let tool_radius = 2.0;
        let stepover = 1.5;

        // Run without initial stock (full material).
        let params_full = default_params(tool_radius, stepover);
        let tp_full = adaptive_toolpath(&poly, &params_full);
        assert!(!tp_full.moves.is_empty(), "full run should produce moves");

        // Build a stock that covers the polygon, with the left half cleared.
        // Stock: x=-10..10, y=-10..10, z=-10..0
        let bbox = BoundingBox3 {
            min: P3::new(-10.0, -10.0, -10.0),
            max: P3::new(10.0, 10.0, 0.0),
        };
        let cell_size = 0.5;
        let mut stock = TriDexelStock::from_bounds(&bbox, cell_size);

        // Clear the left half (x < 0) by subtracting above z = -10 (removes
        // all material in those cells).
        let grid = &mut stock.z_grid;
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let world_x = grid.origin_u + col as f64 * grid.cell_size;
                if world_x < 0.0 {
                    crate::dexel::ray_subtract_above(&mut grid.rays[row * grid.cols + col], -10.0);
                }
            }
        }

        // Run with the half-cleared stock.
        let params_stock = AdaptiveParams {
            initial_stock: Some(stock),
            ..default_params(tool_radius, stepover)
        };
        let tp_stock = adaptive_toolpath(&poly, &params_stock);
        assert!(
            !tp_stock.moves.is_empty(),
            "stock-aware run should still produce moves for remaining material"
        );

        // The stock-aware run should produce fewer moves because half
        // the material is already gone.
        assert!(
            tp_stock.moves.len() < tp_full.moves.len(),
            "stock-aware ({} moves) should be fewer than full ({} moves)",
            tp_stock.moves.len(),
            tp_full.moves.len(),
        );
    }
}
