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
pub(crate) mod path;
mod search;
mod spiral;

pub(crate) use material_grid::MaterialGrid;
pub(crate) use path::{AdaptiveSegment, adaptive_segments_with_debug};
use path::{apply_residue_mop_cleanup, runtime_annotations_to_labels, segments_to_toolpath};

pub(crate) use crate::adaptive_shared::{
    angle_diff, average_angles, blend_corners_to_moves, refine_angle_bracket,
    target_engagement_fraction,
};
use crate::debug_trace::ToolpathDebugContext;
use crate::dexel_stock::TriDexelStock;
use crate::interrupt::{CancelCheck, Cancelled};
use crate::polygon::Polygon2;
use crate::toolpath::Toolpath;

/// How the planner cleans up residue left by the main adaptive spiral.
///
/// - `Legacy`: pre-2026-05 behaviour. After the main spiral, the planner
///   runs many short cleanup passes (each entered via a fresh boundary
///   walk) until material is < 1%. Each pass costs a full retract-rapid-
///   plunge cycle, fragmenting the toolpath visually and producing
///   dozens of inter-pass Rapids on shapes with patchy residue.
/// - `ResidueMop`: 2026-05 follow-up. Short cleanup passes are dropped;
///   instead, after the long adaptive sweeps + boundary cleanup, a
///   grid-walking mop visits remaining residue patches directly,
///   feed-linking between adjacent patches and Rapid-ing only when
///   patches are > 6R apart. Substantially fewer Rapids and arcs;
///   matches BobCAD/Fusion offset-pocket finisher aesthetic.
/// - `ContourParallelNarrow`: 2026-05-28 follow-up. Same as `ResidueMop`
///   for regions wider than the engagement-target spiral can stably
///   handle. For *narrow* regions — where the largest inscribed disk
///   inside the machinable mask is ≤ 3 × stepover — the engagement
///   spiral degenerates into a sawtooth wiggle (no room to swing the
///   ~21-candidate-angle search). For those regions the planner skips
///   the spiral entirely and emits concentric contour-parallel offset
///   loops, then runs the standard residue mop. Targets annular /
///   ring-shaped pockets (donut-topology with a hole near the bounds).
/// - `ContourParallelHybrid`: 2026-05-28 follow-up. Mixes spiral and
///   contour-parallel per *sub-region*. The whole-region narrow gate
///   still applies (uniformly narrow regions skip the spiral and use
///   contour-parallel from the start). For regions that pass the gate
///   as wide, the engagement-target spiral runs as normal — but the
///   residue cleanup phase walks `machinable` inward at stepover
///   offsets and emits only contours that pass through residue. So a
///   shape with a wide bulb and a narrow tail (tadpole, key) gets a
///   spiral in the bulb and concentric offset loops in the tail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum CleanupStrategy {
    Legacy,
    ResidueMop,
    ContourParallelNarrow,
    #[default]
    ContourParallelHybrid,
}

/// Which engagement quantity the direction search measures and compares
/// against `target_engagement_fraction` (contact angle α / 2π).
///
/// `DiskArea` is the historical measure: the fraction of the cutter
/// *disk area* lying in material. That is a different physical quantity
/// from the angle-fraction target — the two only coincide at full slot —
/// so the effective stepover the controller converges on deviates from
/// the commanded one (algorithm review 2026-06-12, finding F1).
/// `LeadingArc` samples the leading half of the flute circle and reads
/// α/2π directly, matching the target's units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum EngagementMeasure {
    #[default]
    DiskArea,
    LeadingArc,
}

/// How the main clearing passes are generated (Stage 1 of the adaptive
/// algorithm review).
///
/// `Agent` is the historical reactive per-step engagement search.
/// `ContourSpiral` is constructive: iso-contours of the machinable-region
/// EDT at stepover increments, traced inside-out from the helical starter
/// pocket as one continuous stay-down pass — engagement bounded by wrap
/// spacing, one plunge per region. Shares the narrow gate, starter
/// pocket, residue cleanup and toolpath emission with the agent path;
/// falls back to the agent when no starter-pocket position exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum PathStrategy2d {
    #[default]
    Agent,
    ContourSpiral,
}

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
    /// How residue is mopped up after the main spiral. See `CleanupStrategy`.
    pub cleanup_strategy: CleanupStrategy,
    /// Which engagement quantity the direction search measures. See
    /// `EngagementMeasure`.
    pub engagement_measure: EngagementMeasure,
    /// How the main clearing passes are generated. See `PathStrategy2d`.
    pub path_strategy: PathStrategy2d,
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
    let segments = adaptive_segments_with_debug(polygon, params, cancel, debug, None)?;
    let segments = match params.cleanup_strategy {
        CleanupStrategy::Legacy => segments,
        CleanupStrategy::ResidueMop | CleanupStrategy::ContourParallelNarrow => {
            apply_residue_mop_cleanup(polygon, params, &segments)
        }
        CleanupStrategy::ContourParallelHybrid => {
            path::apply_contour_parallel_residue_cleanup(polygon, params, &segments)
        }
    };
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
            cleanup_strategy: CleanupStrategy::Legacy,
            engagement_measure: EngagementMeasure::DiskArea,
            path_strategy: PathStrategy2d::Agent,
        }
    }

    /// Stage 4 — the ContourSpiral path records a planner-engagement
    /// sample for every emitted cut point, the values are valid
    /// leading-arc fractions in `[0, 0.5]`, and the median sits near the
    /// commanded target. This is the per-move signal the feed modulator
    /// looks up positionally; if the sink ever comes back empty the
    /// modulator silently falls back to the uniform floor, so this test
    /// guards the wiring end-to-end through the 2D engine.
    #[test]
    fn contour_spiral_populates_planner_engagement_sink() {
        let poly = square_polygon(60.0);
        let params = AdaptiveParams {
            path_strategy: PathStrategy2d::ContourSpiral,
            cleanup_strategy: CleanupStrategy::ContourParallelHybrid,
            engagement_measure: EngagementMeasure::LeadingArc,
            ..default_params(3.0, 1.2)
        };
        let never_cancel = || false;
        let mut sink: Vec<(P2, f64)> = Vec::new();
        let segs =
            adaptive_segments_with_debug(&poly, &params, &never_cancel, None, Some(&mut sink))
                .expect("spiral should not cancel");
        assert!(
            segs.iter().any(|s| matches!(s, AdaptiveSegment::Cut(_))),
            "spiral should emit a Cut segment"
        );
        assert!(
            !sink.is_empty(),
            "spiral must record planner engagement samples"
        );
        for (_, f) in &sink {
            assert!(
                (0.0..=0.5 + 1e-9).contains(f),
                "leading-arc fraction {f} outside [0, 0.5]"
            );
        }
        let mut vals: Vec<f64> = sink.iter().map(|(_, f)| *f).collect();
        vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = vals[vals.len() / 2];
        let target = crate::adaptive_shared::target_engagement_fraction(1.2, 3.0);
        assert!(
            median > 0.3 * target && median < 2.5 * target,
            "median engagement {median} far from target {target}"
        );
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
    fn boundary_distances_are_euclidean_not_manhattan() {
        // F4 (algorithm review 2026-06-12): the boundary-distance field
        // is consumed by the wall bias, the gradient-mode switch and the
        // strip-centerline follower. A diamond's edges are all at 45° to
        // the grid, so its center reads h/√2 ≈ 7.07 under the Euclidean
        // metric but ≈ h = 10 under the old 4-connected BFS (Manhattan).
        let h = 10.0;
        let diamond = Polygon2::new(vec![
            P2::new(0.0, -h),
            P2::new(h, 0.0),
            P2::new(0.0, h),
            P2::new(-h, 0.0),
        ]);
        let grid = MaterialGrid::from_polygon(&diamond, 0.5);
        for (x, y) in [
            (3.0, 5.0),
            (-3.0, 5.0),
            (3.0, -5.0),
            (-3.0, -5.0),
            (5.0, 3.0),
            (-5.0, 3.0),
            (5.0, -3.0),
            (-5.0, -3.0),
            (0.0, 9.5),
        ] {
            assert!(
                grid.is_material(x, y),
                "interior lattice point ({x},{y}) rasterised as air"
            );
        }
        let dist = grid.compute_boundary_distances();
        let center = grid.boundary_distance_at(&dist, 0.0, 0.0);
        let expected = h / std::f64::consts::SQRT_2;
        assert!(
            (center - expected).abs() < 0.8,
            "diamond-center boundary distance must be Euclidean \
             (expected ≈{expected:.2}, Manhattan would read ≈{h:.1}); got {center:.2}"
        );
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
        let empty = super::search::EndpointGrid::new(tool_radius * 3.0);
        let e1 = find_entry_point(&grid, &mask, &machinable[0], tool_radius, None, &empty);
        assert!(e1.is_some());
        let e1 = e1.unwrap();

        // Second entry: should avoid being close to the first
        let mut visited = super::search::EndpointGrid::new(tool_radius * 3.0);
        visited.insert(e1);
        let e2 = find_entry_point(
            &grid,
            &mask,
            &machinable[0],
            tool_radius,
            Some(e1),
            &visited,
        );
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

    // ── Corner-burrow investigation instrumentation ────────────────────
    //
    // Diagnoses the "arcs disappear in the corner" symptom reported on
    // adaptive3d AgentSearch. Runs the 2D planner on a square (the same
    // shape the operator clears) and dumps:
    //   - Cut / Rapid / Link segment counts
    //   - PassSummary `exit_reason` histogram
    //   - ForcedClear positions, classified by proximity to a corner
    //   - Per-pass step / idle / search-evaluation counters
    //
    // Hypothesis from the code-read: most passes near the corner exit
    // via `idle_count > 15`, then `forced_clear` zaps a 2R disc at the
    // endpoint, and the next entry — picked by walking the *original*
    // polygon boundary while excluding (3R)² around prior endpoints —
    // lands far from the residual sliver. Result: many short Cut groups
    // separated by Rapids, which arcfit cannot bridge.
    //
    // Run with `cargo test -p rs_cam_core --lib adaptive::tests::instrument_corner_burrow -- --nocapture`.
    #[test]
    #[allow(clippy::print_stderr)]
    fn instrument_corner_burrow_50mm_square() {
        // 50mm × 50mm square, 6mm tool (radius 3mm), 50% stepover.
        // Matches the user's "burrowing into a corner" report shape.
        let polygon = square_polygon(50.0);
        let tool_radius = 3.0;
        let stepover = tool_radius; // 50% stepover (radial WOC = R)
        let params = AdaptiveParams {
            slot_clearing: false, // study pure adaptive search (no seed slots)
            ..default_params(tool_radius, stepover)
        };
        let never_cancel = || false;
        let segments = adaptive_segments_with_debug(&polygon, &params, &never_cancel, None, None)
            .expect("adaptive should not cancel");

        let mut cut_count = 0usize;
        let mut cut_steps_total = 0usize;
        let mut rapid_count = 0usize;
        let mut link_count = 0usize;
        let mut pass_summaries: Vec<(usize, usize, usize, usize, String)> = Vec::new();
        let mut forced_clears: Vec<(usize, f64, f64)> = Vec::new();
        let mut boundary_cleanup_contours = 0usize;
        let mut pass_entries: Vec<(usize, f64, f64)> = Vec::new();

        for seg in &segments {
            match seg {
                AdaptiveSegment::Cut(path) => {
                    cut_count += 1;
                    cut_steps_total += path.len();
                }
                AdaptiveSegment::Rapid(_) => rapid_count += 1,
                AdaptiveSegment::Link(_) => link_count += 1,
                AdaptiveSegment::Marker(ev) => match ev {
                    AdaptiveRuntimeEvent::PassSummary {
                        pass_index,
                        step_count,
                        idle_count,
                        search_evaluations,
                        exit_reason,
                    } => {
                        pass_summaries.push((
                            *pass_index,
                            *step_count,
                            *idle_count,
                            *search_evaluations,
                            exit_reason.clone(),
                        ));
                    }
                    AdaptiveRuntimeEvent::ForcedClear {
                        pass_index,
                        center_x,
                        center_y,
                        ..
                    } => {
                        forced_clears.push((*pass_index, *center_x, *center_y));
                    }
                    AdaptiveRuntimeEvent::BoundaryCleanup { .. } => {
                        boundary_cleanup_contours += 1;
                    }
                    AdaptiveRuntimeEvent::PassEntry {
                        pass_index,
                        entry_x,
                        entry_y,
                    } => {
                        pass_entries.push((*pass_index, *entry_x, *entry_y));
                    }
                    AdaptiveRuntimeEvent::SlotClearing { .. } => {}
                },
            }
        }

        let mut reason_hist: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        for (_, _, _, _, r) in &pass_summaries {
            *reason_hist.entry(r.clone()).or_insert(0) += 1;
        }

        // Corner-zone classification: machinable region is offset_polygon(square, 3.0),
        // so its corners are at (±22, ±22). "Near a corner" = within 2 tool diameters
        // (= 4R = 12mm) of one of those four points.
        let machinable_corners: [(f64, f64); 4] =
            [(22.0, 22.0), (22.0, -22.0), (-22.0, 22.0), (-22.0, -22.0)];
        let near_corner_threshold_sq = (4.0 * tool_radius).powi(2);
        let in_corner = |x: f64, y: f64| {
            machinable_corners.iter().any(|(cx, cy)| {
                let dx = x - cx;
                let dy = y - cy;
                dx * dx + dy * dy < near_corner_threshold_sq
            })
        };

        let corner_forced_clears = forced_clears
            .iter()
            .filter(|(_, x, y)| in_corner(*x, *y))
            .count();
        let corner_pass_entries = pass_entries
            .iter()
            .filter(|(_, x, y)| in_corner(*x, *y))
            .count();

        eprintln!();
        eprintln!("════════════════════════════════════════════════════════════════");
        eprintln!("Adaptive corner-burrow instrumentation: 50mm square, 6mm tool");
        eprintln!("════════════════════════════════════════════════════════════════");
        eprintln!(
            "Segments: {} Cut groups ({} total cut-step points), {} Rapids, {} Links",
            cut_count, cut_steps_total, rapid_count, link_count
        );
        eprintln!(
            "Pass count: {} (boundary cleanup contours: {})",
            pass_summaries.len(),
            boundary_cleanup_contours
        );
        eprintln!("Exit reason histogram:");
        for (r, c) in &reason_hist {
            eprintln!("  {:>14} : {}", r, c);
        }
        eprintln!(
            "ForcedClear sites: {} total, {} near a machinable corner (within 4R = 12mm)",
            forced_clears.len(),
            corner_forced_clears
        );
        eprintln!(
            "PassEntry sites:   {} total, {} near a machinable corner",
            pass_entries.len(),
            corner_pass_entries
        );
        eprintln!();
        eprintln!("Per-pass detail (idx | steps | idle | search-evals | exit):");
        for (idx, steps, idle, evals, reason) in &pass_summaries {
            let entry_xy = pass_entries
                .iter()
                .find(|(i, _, _)| i == idx)
                .map(|(_, x, y)| format!("({:>6.2}, {:>6.2})", x, y))
                .unwrap_or_else(|| "(   ?  ,   ?  )".into());
            let corner_tag = pass_entries
                .iter()
                .find(|(i, _, _)| i == idx)
                .map(|(_, x, y)| if in_corner(*x, *y) { "CORNER" } else { "" })
                .unwrap_or("");
            eprintln!(
                "  pass {:>3} | steps {:>4} | idle {:>3} | evals {:>5} | exit '{}' | entry {} {}",
                idx, steps, idle, evals, reason, entry_xy, corner_tag
            );
        }
        eprintln!();
        eprintln!("Forced-clear positions:");
        for (idx, x, y) in &forced_clears {
            let tag = if in_corner(*x, *y) { "CORNER" } else { "" };
            eprintln!("  pass {:>3} | ({:>7.2}, {:>7.2}) {}", idx, x, y, tag);
        }
        eprintln!("════════════════════════════════════════════════════════════════");

        // Sanity bound: the test exists to dump data, not to gate behaviour.
        // But assert that at least one pass ran so a silent regression
        // (no passes at all) gets caught.
        assert!(!pass_summaries.is_empty(), "no adaptive passes ran");
    }

    // Render the corner-burrow segments as an SVG for visual inspection.
    // Cut groups in colour-cycled hues (numbered by emission order), Rapids
    // in dashed red, Links in dashed orange.
    //
    // Output: target/adaptive_corner_burrow_50mm.svg (relative to repo root)
    #[test]
    #[allow(clippy::print_stderr)]
    fn render_corner_burrow_svg() {
        let polygon = square_polygon(50.0);
        let tool_radius = 3.0;
        let stepover = tool_radius;
        let params = AdaptiveParams {
            slot_clearing: false,
            ..default_params(tool_radius, stepover)
        };
        let never_cancel = || false;
        let segments = adaptive_segments_with_debug(&polygon, &params, &never_cancel, None, None)
            .expect("adaptive should not cancel");
        write_segments_svg(
            &segments,
            &polygon,
            tool_radius,
            "adaptive_corner_burrow_50mm.svg",
            "baseline (current planner)",
        );
    }

    // ── Shared SVG renderer ────────────────────────────────────────────
    //
    // Writes a 600×600 SVG under <workspace>/target/<filename> showing all
    // Cut groups in colour-cycled hues, Rapids as dashed red, Links as
    // dashed orange. Cut groups numbered by emission order. `label`
    // appears at the top of the legend so different variants are easy
    // to tell apart in side-by-side viewing.
    #[allow(clippy::print_stderr)]
    fn write_segments_svg(
        segments: &[AdaptiveSegment],
        polygon: &Polygon2,
        tool_radius: f64,
        filename: &str,
        label: &str,
    ) {
        use std::io::Write;

        // Compute polygon bbox for viewBox.
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &polygon.exterior {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
        let pad = 5.0_f64;
        let view_min_x = min_x - pad;
        let view_min_y = min_y - pad;
        let view_w = (max_x - min_x) + 2.0 * pad;
        let view_h = (max_y - min_y) + 2.0 * pad;
        const SCALE: f64 = 10.0;
        let size_px_w = (view_w * SCALE).round() as i32;
        let size_px_h = (view_h * SCALE).round() as i32;
        let view_box = format!(
            "{} {} {} {}",
            view_min_x,
            -(view_min_y + view_h),
            view_w,
            view_h
        );

        let mut svg = String::new();
        svg.push_str(&format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='{}' height='{}' viewBox='{}'>\n",
            size_px_w, size_px_h, view_box
        ));
        svg.push_str("<g transform='scale(1,-1)'>\n");

        svg.push_str(&format!(
            "<rect x='{}' y='{}' width='{}' height='{}' fill='#fafafa'/>\n",
            view_min_x, view_min_y, view_w, view_h
        ));

        // Stock polygon exterior + holes.
        let ring_to_path = |ring: &[P2]| -> String {
            if ring.is_empty() {
                return String::new();
            }
            let mut s = format!("M{:.3} {:.3}", ring[0].x, ring[0].y);
            for p in &ring[1..] {
                s.push_str(&format!(" L{:.3} {:.3}", p.x, p.y));
            }
            s.push_str(" Z");
            s
        };
        svg.push_str(&format!(
            "<path d='{}' fill='none' stroke='#888' stroke-width='0.15'/>\n",
            ring_to_path(&polygon.exterior)
        ));
        for hole in &polygon.holes {
            svg.push_str(&format!(
                "<path d='{}' fill='none' stroke='#888' stroke-width='0.15'/>\n",
                ring_to_path(hole)
            ));
        }

        // Machinable boundary (inset by tool radius).
        let machinable = crate::polygon::offset_polygon(polygon, tool_radius);
        for inset in &machinable {
            svg.push_str(&format!(
                "<path d='{}' fill='none' stroke='#aaa' stroke-width='0.1' stroke-dasharray='0.5,0.5'/>\n",
                ring_to_path(&inset.exterior)
            ));
            for hole in &inset.holes {
                svg.push_str(&format!(
                    "<path d='{}' fill='none' stroke='#aaa' stroke-width='0.1' stroke-dasharray='0.5,0.5'/>\n",
                    ring_to_path(hole)
                ));
            }
        }

        let mut last_end: Option<P2> = None;
        let mut pass_counter = 0usize;
        let mut label_positions: Vec<(usize, P2)> = Vec::new();
        let mut cut_groups = 0usize;
        let mut rapid_count = 0usize;
        let mut link_count = 0usize;

        for seg in segments {
            match seg {
                AdaptiveSegment::Cut(path) => {
                    cut_groups += 1;
                    pass_counter += 1;
                    if let Some(first) = path.first() {
                        label_positions.push((pass_counter, *first));
                    }
                    let pts = path
                        .iter()
                        .map(|p| format!("{:.3},{:.3}", p.x, p.y))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let hue = (pass_counter * 47) % 360;
                    svg.push_str(&format!(
                        "<polyline points='{}' fill='none' stroke='hsl({},70%,45%)' \
                         stroke-width='0.4' stroke-linecap='round' stroke-linejoin='round'/>\n",
                        pts, hue
                    ));
                    last_end = path.last().copied();
                }
                AdaptiveSegment::Rapid(p) => {
                    rapid_count += 1;
                    if let Some(from) = last_end {
                        svg.push_str(&format!(
                            "<line x1='{:.3}' y1='{:.3}' x2='{:.3}' y2='{:.3}' \
                             stroke='#d22' stroke-width='0.25' stroke-dasharray='0.6,0.6'/>\n",
                            from.x, from.y, p.x, p.y
                        ));
                    }
                    svg.push_str(&format!(
                        "<circle cx='{:.3}' cy='{:.3}' r='0.4' fill='#d22'/>\n",
                        p.x, p.y
                    ));
                    last_end = Some(*p);
                }
                AdaptiveSegment::Link(p) => {
                    link_count += 1;
                    if let Some(from) = last_end {
                        svg.push_str(&format!(
                            "<line x1='{:.3}' y1='{:.3}' x2='{:.3}' y2='{:.3}' \
                             stroke='#e80' stroke-width='0.25' stroke-dasharray='0.4,0.4'/>\n",
                            from.x, from.y, p.x, p.y
                        ));
                    }
                    svg.push_str(&format!(
                        "<circle cx='{:.3}' cy='{:.3}' r='0.3' fill='#e80'/>\n",
                        p.x, p.y
                    ));
                    last_end = Some(*p);
                }
                AdaptiveSegment::Marker(_) => {}
            }
        }

        svg.push_str("<g font-family='monospace' font-size='1.5' fill='#222'>\n");
        for (idx, p) in &label_positions {
            svg.push_str(&format!(
                "<text x='{:.3}' y='{:.3}' transform='scale(1,-1)'>{}</text>\n",
                p.x + 0.5,
                -(p.y + 0.5),
                idx
            ));
        }
        svg.push_str("</g>\n");

        svg.push_str("</g>\n");
        svg.push_str(&format!(
            "<g font-family='monospace' font-size='1.8' fill='#222' transform='translate({},{})'>\n",
            view_min_x + 1.0,
            -(view_min_y + view_h) + 3.0
        ));
        svg.push_str(&format!("<text x='0' y='0'>{}</text>\n", label));
        svg.push_str(&format!(
            "<text x='0' y='2.2'>cuts {} | rapids {} | links {}</text>\n",
            cut_groups, rapid_count, link_count
        ));
        svg.push_str("<text x='0' y='4.4'>red dashed = Rapid, orange dashed = Link</text>\n");
        svg.push_str("</g>\n");

        svg.push_str("</svg>\n");

        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let target_dir = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("target"))
            .expect("locate workspace target dir");
        std::fs::create_dir_all(&target_dir).expect("create target dir");
        let out = target_dir.join(filename);
        let mut f = std::fs::File::create(&out).expect("create svg");
        f.write_all(svg.as_bytes()).expect("write svg");
        eprintln!(
            "wrote {} ({} cuts / {} rapids / {} links)",
            out.display(),
            cut_groups,
            rapid_count,
            link_count
        );
    }

    // ── Cheap fix: boundary-extension post-process ─────────────────────
    //
    // Walks the baseline segments. Wherever we see `Cut → Rapid → Cut` and
    // the Rapid is short (< 6R, the same 2D-link distance gate) AND the
    // straight line between the previous Cut endpoint and the Rapid target
    // both (a) stays inside the machinable region and (b) actually clears
    // material along the way → absorb the Rapid into the previous Cut group
    // as a feed extension. Otherwise leave the Rapid alone.
    //
    // The 2D planner already does this in spirit at path.rs:248-264, but only
    // for is_clear_path (≤ 20% material along the line). The "cheap fix"
    // INVERTS that test: extend when the line DOES cross material, so the
    // cutter cleans residue along the way instead of skipping over it.
    #[allow(clippy::print_stderr, dead_code)]
    fn apply_boundary_extend_fix(
        polygon: &Polygon2,
        tool_radius: f64,
        machinable: &Polygon2,
        machinable_mask: &[bool],
        cell_size: f64,
        step_len: f64,
        original: &[AdaptiveSegment],
    ) -> Vec<AdaptiveSegment> {
        // Run with threshold 20R so we can see whether the post-process
        // logic itself is doing anything, even though the 2D planner's
        // gate is 6R. Diagnostic printouts below record what happens.
        let max_link_dist = tool_radius * 20.0;
        eprintln!(
            "cheap-fix: scanning {} segments, link-absorption threshold {:.1}mm",
            original.len(),
            max_link_dist
        );
        let mut rapid_dists: Vec<f64> = Vec::new();
        let mut absorbed = 0usize;
        let mut rejected_too_far = 0usize;
        let mut rejected_no_clearing = 0usize;
        let mut rejected_off_machinable = 0usize;
        let mut grid = MaterialGrid::from_polygon(polygon, cell_size);
        let mut out: Vec<AdaptiveSegment> = Vec::new();
        let mut last_cut_end: Option<P2> = None;

        let mut i = 0;
        while i < original.len() {
            match &original[i] {
                AdaptiveSegment::Cut(path) => {
                    for p in path {
                        grid.clear_circle(p.x, p.y, tool_radius);
                    }
                    last_cut_end = path.last().copied();
                    out.push(AdaptiveSegment::Cut(path.clone()));
                    i += 1;
                }
                AdaptiveSegment::Rapid(target) => {
                    let from = last_cut_end;
                    if let Some(from) = from {
                        let dx = target.x - from.x;
                        let dy = target.y - from.y;
                        let dist = (dx * dx + dy * dy).sqrt();
                        rapid_dists.push(dist);
                        if dist >= max_link_dist {
                            rejected_too_far += 1;
                        } else if dist > 1e-6 {
                            let n_steps = (dist / step_len).ceil().max(1.0) as usize;
                            let mut extension: Vec<P2> = Vec::new();
                            let mut cleared_any = false;
                            let mut still_machinable = true;
                            let mut trial_grid = grid.clone();
                            for k in 1..=n_steps {
                                let t = k as f64 / n_steps as f64;
                                let nx = from.x + t * dx;
                                let ny = from.y + t * dy;
                                if !trial_grid.is_machinable(machinable_mask, nx, ny) {
                                    still_machinable = false;
                                    break;
                                }
                                let before = trial_grid.material_count;
                                trial_grid.clear_circle(nx, ny, tool_radius);
                                if trial_grid.material_count != before {
                                    cleared_any = true;
                                }
                                extension.push(P2::new(nx, ny));
                            }
                            if !still_machinable {
                                rejected_off_machinable += 1;
                            } else if !cleared_any {
                                rejected_no_clearing += 1;
                            } else {
                                grid = trial_grid;
                                if let Some(AdaptiveSegment::Cut(p)) = out
                                    .iter_mut()
                                    .rev()
                                    .find(|s| matches!(s, AdaptiveSegment::Cut(_)))
                                {
                                    p.extend(extension.iter().copied());
                                }
                                last_cut_end = extension.last().copied();
                                absorbed += 1;
                                i += 1;
                                continue;
                            }
                        }
                    }
                    out.push(AdaptiveSegment::Rapid(*target));
                    last_cut_end = Some(*target);
                    i += 1;
                }
                AdaptiveSegment::Link(target) => {
                    out.push(AdaptiveSegment::Link(*target));
                    last_cut_end = Some(*target);
                    i += 1;
                }
                AdaptiveSegment::Marker(m) => {
                    out.push(AdaptiveSegment::Marker(m.clone()));
                    i += 1;
                }
            }
        }
        let _ = machinable; // kept in signature for future extension along contour
        let mut dists_sorted = rapid_dists.clone();
        dists_sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        eprintln!(
            "cheap-fix: {} rapids scanned, absorbed {}, too-far {}, no-clearing {}, off-machinable {}",
            rapid_dists.len(),
            absorbed,
            rejected_too_far,
            rejected_no_clearing,
            rejected_off_machinable
        );
        if !dists_sorted.is_empty() {
            let median = dists_sorted[dists_sorted.len() / 2];
            let p90 = dists_sorted[(dists_sorted.len() * 9 / 10).min(dists_sorted.len() - 1)];
            eprintln!(
                "  rapid-distance: min {:.1}mm, median {:.1}mm, p90 {:.1}mm, max {:.1}mm",
                dists_sorted.first().copied().unwrap_or(0.0),
                median,
                p90,
                dists_sorted.last().copied().unwrap_or(0.0)
            );
        }
        out
    }

    // ── Variant renders ────────────────────────────────────────────────

    #[test]
    #[allow(clippy::print_stderr)]
    fn render_corner_burrow_cheap_fix_svg() {
        let polygon = square_polygon(50.0);
        let tool_radius = 3.0;
        let stepover = tool_radius;
        let params = AdaptiveParams {
            slot_clearing: false,
            ..default_params(tool_radius, stepover)
        };
        let never_cancel = || false;
        let baseline = adaptive_segments_with_debug(&polygon, &params, &never_cancel, None, None)
            .expect("adaptive should not cancel");

        let machinable = crate::polygon::offset_polygon(&polygon, tool_radius)
            .into_iter()
            .next()
            .expect("machinable region exists");
        let cell_size = (tool_radius / 6.0).max(params.tolerance);
        let machinable_mask = MaterialGrid::build_machinable_mask(
            &machinable,
            polygon
                .exterior
                .iter()
                .map(|p| p.x)
                .fold(f64::INFINITY, f64::min)
                - 1.0,
            polygon
                .exterior
                .iter()
                .map(|p| p.y)
                .fold(f64::INFINITY, f64::min)
                - 1.0,
            ((50.0 + 4.0) / cell_size).ceil() as usize,
            ((50.0 + 4.0) / cell_size).ceil() as usize,
            cell_size,
        );
        // ^ above mask is approximate (sufficient for in-machinable lookups in the test).
        let _ = machinable_mask;
        // Re-derive precisely the same mask the planner used.
        let test_grid = MaterialGrid::from_polygon(&polygon, cell_size);
        let mask = MaterialGrid::build_machinable_mask(
            &machinable,
            test_grid.origin_x,
            test_grid.origin_y,
            test_grid.rows,
            test_grid.cols,
            test_grid.cell_size,
        );

        let step_len = cell_size * 3.0;
        let fixed = apply_boundary_extend_fix(
            &polygon,
            tool_radius,
            &machinable,
            &mask,
            cell_size,
            step_len,
            &baseline,
        );

        write_segments_svg(
            &fixed,
            &polygon,
            tool_radius,
            "adaptive_corner_burrow_50mm_cheap.svg",
            "cheap fix (boundary-extension post-process)",
        );
    }

    #[test]
    #[allow(clippy::print_stderr)]
    fn render_corner_burrow_proper_fix_svg() {
        let polygon = square_polygon(50.0);
        let tool_radius = 3.0;
        let stepover = tool_radius;
        let params = AdaptiveParams {
            slot_clearing: false,
            ..default_params(tool_radius, stepover)
        };
        let never_cancel = || false;
        let baseline = adaptive_segments_with_debug(&polygon, &params, &never_cancel, None, None)
            .expect("adaptive should not cancel");

        let machinable = crate::polygon::offset_polygon(&polygon, tool_radius)
            .into_iter()
            .next()
            .expect("machinable region exists");
        let cell_size = (tool_radius / 6.0).max(params.tolerance);

        let _ = (machinable, cell_size, stepover);
        let mop_params = AdaptiveParams {
            cleanup_strategy: CleanupStrategy::ResidueMop,
            ..default_params(tool_radius, stepover)
        };
        let fixed = path::apply_residue_mop_cleanup(&polygon, &mop_params, &baseline);
        write_segments_svg(
            &fixed,
            &polygon,
            tool_radius,
            "adaptive_corner_burrow_50mm_proper.svg",
            "proper fix (planner ResidueMop)",
        );
    }

    // ── Shape matrix ───────────────────────────────────────────────────
    //
    // Run baseline + offset-pocket-mop on a variety of shapes, render each
    // pair side-by-side as SVG. Validates that the proper fix doesn't
    // regress on non-square geometry (circles, concave corners, holes).
    //
    // Output: target/adaptive_shape_<name>_{baseline,proper}.svg
    #[test]
    #[allow(clippy::print_stderr)]
    fn render_shape_matrix_svg() {
        // Shape definitions. Each returns a (name, polygon) pair sized to
        // ~50mm extent so they're directly comparable to the square test.
        let shapes: Vec<(&str, Polygon2)> = vec![
            ("square_50", Polygon2::rectangle(-25.0, -25.0, 25.0, 25.0)),
            ("rect_60x30", Polygon2::rectangle(-30.0, -15.0, 30.0, 15.0)),
            ("circle_50", {
                // 50mm diameter circle, 64-gon approximation
                let r = 25.0;
                let n = 64;
                let pts: Vec<P2> = (0..n)
                    .map(|i| {
                        let t = (i as f64 / n as f64) * std::f64::consts::TAU;
                        P2::new(r * t.cos(), r * t.sin())
                    })
                    .collect();
                Polygon2::new(pts)
            }),
            ("l_shape", {
                // L-shape: 50mm × 50mm with a 25mm × 25mm notch removed
                // from the top-right corner. Concave inner corner exposes
                // the offset-polygon arc rounding behaviour.
                Polygon2::new(vec![
                    P2::new(-25.0, -25.0),
                    P2::new(25.0, -25.0),
                    P2::new(25.0, 0.0),
                    P2::new(0.0, 0.0),
                    P2::new(0.0, 25.0),
                    P2::new(-25.0, 25.0),
                ])
            }),
            ("square_with_hole", {
                // 50mm × 50mm square with a 15mm × 15mm hole centred at origin.
                // Hole is CW (opposite winding from exterior).
                let outer = Polygon2::rectangle(-25.0, -25.0, 25.0, 25.0).exterior;
                let hole = vec![
                    P2::new(-7.5, -7.5),
                    P2::new(-7.5, 7.5),
                    P2::new(7.5, 7.5),
                    P2::new(7.5, -7.5),
                ];
                Polygon2::with_holes(outer, vec![hole])
            }),
            ("tadpole", {
                // Wide bulb (r=30mm at x=-15) tangent-merged with a
                // narrow tail (4.5mm half-height) extending to x=50.
                // Bulb is 10 cutter-diameters wide — large enough for
                // the engagement-target spiral to settle into clean
                // constant-engagement arcs without convergence
                // wiggle. Tail is too narrow for the spiral search
                // ring, so the contour-parallel sweep handles it.
                let cx = -15.0_f64;
                let r = 30.0_f64;
                let tail_h = 4.5_f64;
                let tail_x = 50.0_f64;
                let theta_attach = (tail_h / r).asin();
                let mut pts: Vec<P2> = Vec::new();
                pts.push(P2::new(tail_x, tail_h));
                let n_arc = 48;
                for i in 0..=n_arc {
                    let t = i as f64 / n_arc as f64;
                    let theta = theta_attach + t * (std::f64::consts::TAU - 2.0 * theta_attach);
                    pts.push(P2::new(cx + r * theta.cos(), r * theta.sin()));
                }
                pts.push(P2::new(tail_x, -tail_h));
                Polygon2::new(pts)
            }),
        ];

        let tool_radius = 3.0;
        let stepover = tool_radius;
        let params = AdaptiveParams {
            slot_clearing: false,
            ..default_params(tool_radius, stepover)
        };
        let never_cancel = || false;

        eprintln!();
        eprintln!(
            "════════════════════════════════════════════════════════════════════════════════════════════════════════"
        );
        eprintln!(
            "Shape matrix — baseline / ResidueMop / ContourParallelNarrow / ContourParallelHybrid"
        );
        eprintln!(
            "════════════════════════════════════════════════════════════════════════════════════════════════════════"
        );
        eprintln!(
            "{:>18} | {:>14} | {:>14} | {:>14} | {:>14}",
            "shape", "baseline", "mop", "contour-narrow", "hybrid"
        );

        let count_segs = |segs: &[AdaptiveSegment]| -> (usize, usize, usize) {
            let mut c = 0;
            let mut r = 0;
            let mut l = 0;
            for s in segs {
                match s {
                    AdaptiveSegment::Cut(_) => c += 1,
                    AdaptiveSegment::Rapid(_) => r += 1,
                    AdaptiveSegment::Link(_) => l += 1,
                    AdaptiveSegment::Marker(_) => {}
                }
            }
            (c, r, l)
        };

        for (name, polygon) in &shapes {
            let machinable_vec = crate::polygon::offset_polygon(polygon, tool_radius);
            if machinable_vec.is_empty() {
                eprintln!("{:>18} | (no machinable region)", name);
                continue;
            }
            let _machinable = machinable_vec[0].clone();
            let baseline =
                adaptive_segments_with_debug(polygon, &params, &never_cancel, None, None)
                    .expect("adaptive should not cancel");
            let mop_params = AdaptiveParams {
                cleanup_strategy: CleanupStrategy::ResidueMop,
                ..default_params(tool_radius, stepover)
            };
            let fixed = path::apply_residue_mop_cleanup(polygon, &mop_params, &baseline);
            // ContourParallelNarrow: branches inside adaptive_segments_with_debug
            // based on max-DT-in-mask gate, then runs the residue mop on the
            // output. For wide regions this should be byte-identical to the
            // ResidueMop column; for narrow regions (donut) it emits
            // concentric offset loops instead.
            let narrow_params = AdaptiveParams {
                cleanup_strategy: CleanupStrategy::ContourParallelNarrow,
                slot_clearing: false,
                ..default_params(tool_radius, stepover)
            };
            let narrow_segs =
                adaptive_segments_with_debug(polygon, &narrow_params, &never_cancel, None, None)
                    .expect("adaptive should not cancel");
            let narrow = path::apply_residue_mop_cleanup(polygon, &narrow_params, &narrow_segs);
            // ContourParallelHybrid: spiral runs on whole machinable
            // (unless the narrow gate fires for the whole region —
            // then it also short-circuits to contour-parallel like
            // Narrow). Cleanup phase uses contour-parallel sweep with
            // material-presence filter, then cell-walking mop fallback.
            let hybrid_params = AdaptiveParams {
                cleanup_strategy: CleanupStrategy::ContourParallelHybrid,
                slot_clearing: false,
                ..default_params(tool_radius, stepover)
            };
            let hybrid_segs =
                adaptive_segments_with_debug(polygon, &hybrid_params, &never_cancel, None, None)
                    .expect("adaptive should not cancel");
            let hybrid =
                path::apply_contour_parallel_residue_cleanup(polygon, &hybrid_params, &hybrid_segs);
            let (bc, br, bl) = count_segs(&baseline);
            let (fc, fr, fl) = count_segs(&fixed);
            let (nc, nr, nl) = count_segs(&narrow);
            let (hc, hr, hl) = count_segs(&hybrid);
            eprintln!(
                "{:>18} | {:>2}C {:>2}R {:>2}L | {:>2}C {:>2}R {:>2}L | {:>2}C {:>2}R {:>2}L | {:>2}C {:>2}R {:>2}L",
                name, bc, br, bl, fc, fr, fl, nc, nr, nl, hc, hr, hl
            );
            write_segments_svg(
                &baseline,
                polygon,
                tool_radius,
                &format!("adaptive_shape_{}_baseline.svg", name),
                &format!("{}: baseline", name),
            );
            write_segments_svg(
                &fixed,
                polygon,
                tool_radius,
                &format!("adaptive_shape_{}_proper.svg", name),
                &format!("{}: offset-pocket mop-up", name),
            );
            write_segments_svg(
                &narrow,
                polygon,
                tool_radius,
                &format!("adaptive_shape_{}_narrow.svg", name),
                &format!("{}: contour-parallel narrow", name),
            );
            write_segments_svg(
                &hybrid,
                polygon,
                tool_radius,
                &format!("adaptive_shape_{}_hybrid.svg", name),
                &format!("{}: contour-parallel hybrid", name),
            );
        }
        eprintln!(
            "════════════════════════════════════════════════════════════════════════════════════════════════════════"
        );
    }

    // ── Wiggle zoom ────────────────────────────────────────────────────
    //
    // Renders just the first ~30 steps of pass 1 at high zoom, with every
    // step numbered. Lets us see whether the initial direction-search
    // takes a wandering path before settling into the main spiral.
    #[test]
    #[allow(clippy::print_stderr)]
    fn render_corner_burrow_initial_wiggle_svg() {
        use std::io::Write;

        let polygon = square_polygon(50.0);
        let tool_radius = 3.0;
        let stepover = tool_radius;
        let params = AdaptiveParams {
            slot_clearing: false,
            ..default_params(tool_radius, stepover)
        };
        let never_cancel = || false;
        let segments = adaptive_segments_with_debug(&polygon, &params, &never_cancel, None, None)
            .expect("adaptive should not cancel");

        // Find the first Cut group; take its first N points.
        let first_cut = segments
            .iter()
            .find_map(|s| {
                if let AdaptiveSegment::Cut(p) = s {
                    Some(p.clone())
                } else {
                    None
                }
            })
            .expect("at least one Cut");
        let n_steps = 40.min(first_cut.len());
        let head: Vec<P2> = first_cut.iter().take(n_steps).copied().collect();

        // Compute bbox for zoom
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &head {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
        let pad = 2.0_f64;
        min_x -= pad;
        min_y -= pad;
        max_x += pad;
        max_y += pad;
        let width = max_x - min_x;
        let height = max_y - min_y;

        let svg_size = 800.0;
        let view_box = format!("{} {} {} {}", min_x, -max_y, width, height);
        let mut svg = String::new();
        svg.push_str(&format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='{}' height='{}' viewBox='{}'>\n",
            svg_size, svg_size, view_box
        ));
        svg.push_str("<g transform='scale(1,-1)'>\n");

        svg.push_str(&format!(
            "<rect x='{}' y='{}' width='{}' height='{}' fill='#fafafa'/>\n",
            min_x, min_y, width, height
        ));

        // Machinable boundary (inset square corner).
        let mh = 25.0 - tool_radius;
        svg.push_str(&format!(
            "<rect x='-{mh}' y='-{mh}' width='{mw}' height='{mw}' fill='none' \
             stroke='#aaa' stroke-width='0.04' stroke-dasharray='0.2,0.2'/>\n",
            mh = mh,
            mw = mh * 2.0
        ));

        // Cutter circles at each step (translucent) so the swept area is visible
        for p in &head {
            svg.push_str(&format!(
                "<circle cx='{:.3}' cy='{:.3}' r='{:.3}' fill='#88aaff' fill-opacity='0.08' stroke='none'/>\n",
                p.x, p.y, tool_radius
            ));
        }

        // Polyline through cutter centers
        let pts = head
            .iter()
            .map(|p| format!("{:.3},{:.3}", p.x, p.y))
            .collect::<Vec<_>>()
            .join(" ");
        svg.push_str(&format!(
            "<polyline points='{}' fill='none' stroke='#1144aa' stroke-width='0.06' \
             stroke-linecap='round' stroke-linejoin='round'/>\n",
            pts
        ));

        // Numbered step markers
        svg.push_str("<g font-family='monospace' font-size='0.35' fill='#222'>\n");
        for (i, p) in head.iter().enumerate() {
            svg.push_str(&format!(
                "<circle cx='{:.3}' cy='{:.3}' r='0.08' fill='#aa1144'/>\n",
                p.x, p.y
            ));
            svg.push_str(&format!(
                "<text x='{:.3}' y='{:.3}' transform='scale(1,-1)'>{}</text>\n",
                p.x + 0.1,
                -(p.y + 0.1),
                i
            ));
        }
        svg.push_str("</g>\n");

        svg.push_str("</g>\n");
        svg.push_str("</svg>\n");

        let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let target_dir = manifest_dir
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("target"))
            .expect("locate workspace target dir");
        std::fs::create_dir_all(&target_dir).expect("create target dir");
        let out = target_dir.join("adaptive_corner_burrow_50mm_wiggle.svg");
        let mut f = std::fs::File::create(&out).expect("create svg");
        f.write_all(svg.as_bytes()).expect("write svg");
        eprintln!(
            "wrote {} (first {} steps of pass 1)",
            out.display(),
            n_steps
        );
    }
}
