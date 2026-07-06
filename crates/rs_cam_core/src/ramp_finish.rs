//! Ramp finishing strategy for steep walls.
//!
//! Instead of cutting at discrete constant-Z levels (waterline/contour), the ramp
//! strategy continuously descends along the surface, eliminating Z-level witness
//! marks. The tool is always engaged — no retract-reposition-plunge cycles between
//! Z levels.
//!
//! From Fusion 360 docs: "ramps down walls rather than machines with a constant Z…
//! ensures that the tool is engaged at all times."
//!
//! Algorithm:
//! 1. Generate waterline contours at multiple Z levels
//! 2. Parameterize each contour by arc length
//! 3. Match adjacent Z-level contours by nearest-centroid correspondence
//! 4. Interpolate between matched contours to create continuous helical descent
//! 5. Apply slope confinement to restrict to steep regions

use crate::debug_trace::ToolpathDebugContext;
use crate::geo::P3;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::tool::MillingCutter;
use crate::toolpath::{Toolpath, simplify_path_3d};
use crate::waterline::waterline_contours;

use tracing::info;

/// Cutting direction for ramp finishing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CutDirection {
    /// Climb milling (tool moves with feed direction).
    #[default]
    Climb,
    /// Conventional milling (tool moves against feed direction).
    Conventional,
    /// Alternate directions between passes.
    BothWays,
}

/// Parameters for ramp finishing.
pub struct RampFinishParams {
    /// Maximum Z stepdown per revolution/circuit (mm).
    pub max_stepdown: f64,
    /// Slope confinement: only machine areas steeper than this (degrees from horizontal).
    pub slope_from: f64,
    /// Slope confinement: only machine areas shallower than this (degrees from horizontal).
    pub slope_to: f64,
    /// Cutting direction.
    pub direction: CutDirection,
    /// Order passes bottom-up instead of top-down.
    pub order_bottom_up: bool,
    /// Feed rate for cutting moves (mm/min).
    pub feed_rate: f64,
    /// Plunge rate (mm/min).
    pub plunge_rate: f64,
    /// Safe Z for rapid positioning.
    pub safe_z: f64,
    /// Fiber sampling spacing for waterline contour generation.
    pub sampling: f64,
    /// Stock to leave on the surface (mm).
    pub stock_to_leave: f64,
    /// Path tolerance for simplification.
    pub tolerance: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RampFinishRuntimeEvent {
    Ramp {
        terrace_index: usize,
        terrace_total: usize,
        upper_level_index: usize,
        lower_level_index: usize,
        upper_z: f64,
        lower_z: f64,
        ramp_index: usize,
        ramp_total: usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RampFinishRuntimeAnnotation {
    pub move_index: usize,
    pub event: RampFinishRuntimeEvent,
}

impl RampFinishRuntimeEvent {
    pub fn label(&self) -> String {
        match self {
            Self::Ramp {
                terrace_index,
                ramp_index,
                ..
            } => format!("Terrace {terrace_index} ramp {ramp_index}"),
        }
    }
}

struct RampSegmentRecord {
    path: Vec<P3>,
    terrace_index: usize,
    terrace_total: usize,
    upper_level_index: usize,
    lower_level_index: usize,
    upper_z: f64,
    lower_z: f64,
    ramp_index: usize,
    ramp_total: usize,
}

impl Default for RampFinishParams {
    fn default() -> Self {
        Self {
            max_stepdown: 1.0,
            slope_from: 30.0,
            slope_to: 90.0,
            direction: CutDirection::Climb,
            order_bottom_up: false,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            sampling: 1.0,
            stock_to_leave: 0.0,
            tolerance: 0.05,
        }
    }
}

/// A parameterized contour: points with cumulative arc-length parameter t ∈ [0, 1].
struct ParamContour {
    points: Vec<P3>,
    /// Cumulative arc-length at each point, normalized to [0, 1].
    params: Vec<f64>,
    /// Total arc length.
    total_length: f64,
    /// Centroid for contour matching.
    centroid: (f64, f64),
}

impl ParamContour {
    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    fn from_contour(contour: &[P3]) -> Self {
        let n = contour.len();
        let mut params = Vec::with_capacity(n);
        let mut cum = 0.0;
        params.push(0.0);
        for i in 1..n {
            let dx = contour[i].x - contour[i - 1].x;
            let dy = contour[i].y - contour[i - 1].y;
            cum += (dx * dx + dy * dy).sqrt();
            params.push(cum);
        }
        let total_length = cum.max(1e-10);
        // Normalize to [0, 1]
        for p in &mut params {
            *p /= total_length;
        }

        let cx = contour.iter().map(|p| p.x).sum::<f64>() / n as f64;
        let cy = contour.iter().map(|p| p.y).sum::<f64>() / n as f64;

        Self {
            points: contour.to_vec(),
            params,
            total_length,
            centroid: (cx, cy),
        }
    }

    #[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
    /// Interpolate a point at parameter t ∈ [0, 1].
    fn point_at(&self, t: f64) -> P3 {
        let t = t.clamp(0.0, 1.0);
        if self.points.len() < 2 {
            return self.points[0];
        }
        // Binary search for the segment containing t
        let idx = match self
            .params
            .binary_search_by(|p| p.partial_cmp(&t).unwrap_or(std::cmp::Ordering::Equal))
        {
            Ok(i) => return self.points[i],
            Err(i) => i.saturating_sub(1),
        };
        let idx = idx.min(self.points.len() - 2);
        let t0 = self.params[idx];
        let t1 = self.params[idx + 1];
        let dt = t1 - t0;
        if dt < 1e-15 {
            return self.points[idx];
        }
        let frac = (t - t0) / dt;
        let a = &self.points[idx];
        let b = &self.points[idx + 1];
        P3::new(
            a.x + frac * (b.x - a.x),
            a.y + frac * (b.y - a.y),
            a.z + frac * (b.z - a.z),
        )
    }
}

#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
/// Match contours between adjacent Z levels by nearest centroid.
///
/// Returns pairs of indices (upper_idx, lower_idx) for matched contours.
/// Unmatched contours (new walls appearing/disappearing) are returned separately.
fn match_contours(upper: &[ParamContour], lower: &[ParamContour]) -> Vec<(usize, usize)> {
    let mut matches = Vec::new();
    let mut lower_used = vec![false; lower.len()];

    for (ui, uc) in upper.iter().enumerate() {
        let mut best_dist = f64::INFINITY;
        let mut best_li = None;
        for (li, lc) in lower.iter().enumerate() {
            if lower_used[li] {
                continue;
            }
            let dx = uc.centroid.0 - lc.centroid.0;
            let dy = uc.centroid.1 - lc.centroid.1;
            let dist = dx * dx + dy * dy;
            if dist < best_dist {
                best_dist = dist;
                best_li = Some(li);
            }
        }
        if let Some(li) = best_li {
            // Only match if centroids are reasonably close (within 2× max contour extent)
            let max_extent = uc.total_length.max(lower[li].total_length) * 0.5;
            if best_dist.sqrt() < max_extent {
                matches.push((ui, li));
                lower_used[li] = true;
            }
        }
    }

    matches
}

/// Generate a ramp path between two matched contours.
///
/// Walks along the upper contour, continuously interpolating Z toward the lower
/// contour. The descent rate is limited by `max_stepdown` per revolution.
fn ramp_between_contours(
    upper: &ParamContour,
    lower: &ParamContour,
    max_stepdown: f64,
    step_len: f64,
) -> Vec<P3> {
    let z_upper = upper.points.iter().map(|p| p.z).sum::<f64>() / upper.points.len() as f64;
    let z_lower = lower.points.iter().map(|p| p.z).sum::<f64>() / lower.points.len() as f64;
    let z_drop = (z_upper - z_lower).abs();

    if z_drop < 0.001 || upper.total_length < 1e-6 {
        return Vec::new();
    }

    // Number of revolutions needed to descend at max_stepdown rate
    let n_revs = (z_drop / max_stepdown).ceil().max(1.0);
    // Total distance to cover = n_revs * upper contour length
    let total_dist = n_revs * upper.total_length;
    let n_points = (total_dist / step_len).ceil() as usize;
    if n_points < 2 {
        return Vec::new();
    }

    let mut path = Vec::with_capacity(n_points);

    for i in 0..=n_points {
        let global_frac = i as f64 / n_points as f64; // 0 → 1 over entire ramp
        let t = (global_frac * n_revs).fract(); // Parameter along current revolution

        // Interpolate XY from upper contour at parameter t
        let upper_pt = upper.point_at(t);
        let lower_pt = lower.point_at(t);

        // Z interpolation: linearly descend from z_upper to z_lower
        let z = z_upper + global_frac * (z_lower - z_upper);

        // XY: blend between upper and lower contour shapes as we descend
        let shape_frac = global_frac; // How much to blend toward lower shape
        let x = upper_pt.x + shape_frac * (lower_pt.x - upper_pt.x);
        let y = upper_pt.y + shape_frac * (lower_pt.y - upper_pt.y);

        path.push(P3::new(x, y, z));
    }

    path
}

/// Generate a ramp finishing toolpath.
///
/// Produces continuous helical descent along steep walls instead of discrete
/// Z-level waterline passes. Eliminates Z-level witness marks.
#[tracing::instrument(skip(mesh, index, cutter, params), fields(max_stepdown = params.max_stepdown))]
pub fn ramp_finish_toolpath(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &RampFinishParams,
) -> Toolpath {
    let (tp, _) = ramp_finish_toolpath_structured_annotated(mesh, index, cutter, params, None);
    tp
}

/// Cancellable variant of [`ramp_finish_toolpath`].
#[allow(clippy::expect_used)]
pub fn ramp_finish_toolpath_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &RampFinishParams,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    let (tp, _) = ramp_finish_toolpath_structured_annotated_with_cancel(
        mesh, index, cutter, params, None, cancel,
    )?;
    Ok(tp)
}

fn runtime_annotations_to_labels(
    annotations: &[RampFinishRuntimeAnnotation],
) -> Vec<(usize, String)> {
    annotations
        .iter()
        .map(|annotation| (annotation.move_index, annotation.event.label()))
        .collect()
}

// infallible: cancel closure always returns false, so Cancelled is unreachable
#[allow(clippy::indexing_slicing, clippy::expect_used)]
pub fn ramp_finish_toolpath_structured_annotated(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &RampFinishParams,
    debug: Option<&ToolpathDebugContext>,
) -> (Toolpath, Vec<RampFinishRuntimeAnnotation>) {
    let never_cancel = || false;
    ramp_finish_toolpath_structured_annotated_with_cancel(
        mesh,
        index,
        cutter,
        params,
        debug,
        &never_cancel,
    )
    .expect("non-cancellable ramp finish toolpath should never be cancelled")
}

/// Cancellable variant of [`ramp_finish_toolpath_structured_annotated`].
/// Polls `cancel` once per Z level while building waterline contours, once
/// per terrace (adjacent Z-level pair) while ramping between them, and once
/// per ramp segment during toolpath emission.
#[allow(clippy::indexing_slicing)] // bounded indexing in algorithmic code
pub fn ramp_finish_toolpath_structured_annotated_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &RampFinishParams,
    debug: Option<&ToolpathDebugContext>,
    cancel: &dyn CancelCheck,
) -> Result<(Toolpath, Vec<RampFinishRuntimeAnnotation>), Cancelled> {
    check_cancel(cancel)?;
    let bbox = &mesh.bbox;

    // Build surface heightmap and slope map (shared setup, see finish_setup.rs)
    let surface = crate::finish_setup::build_finish_surface_with_cancel(
        mesh,
        index,
        cutter,
        params.tolerance,
        cancel,
    )?;
    let surface_hm = surface.heightmap;
    let slope_map = surface.slope_map;
    let cell_size = surface_hm.cell_size;

    // Compute Z range
    let z_top = bbox.max.z + params.stock_to_leave;
    let z_bottom = surface_hm.min_z() + params.stock_to_leave;
    let z_step = params.max_stepdown;

    // Generate Z levels. `snap_to_bottom = true` guarantees the ladder ends
    // exactly at `z_bottom` (needed so the final terrace's lower contour is
    // the true bottom, not an arbitrary short-of-bottom level); epsilon is
    // half a step, matching the original inline arithmetic exactly.
    let z_levels = crate::finish_setup::z_ladder(z_top, z_bottom, z_step, z_step * 0.5, true);

    if z_levels.len() < 2 {
        info!("Ramp finish: insufficient Z range for ramping");
        return Ok((Toolpath::new(), Vec::new()));
    }

    info!(
        levels = z_levels.len(),
        z_top = format!("{:.1}", z_top),
        z_bottom = format!("{:.1}", z_bottom),
        "Ramp finish: generating waterline contours"
    );

    // Generate waterline contours at each Z level
    let mut level_contours: Vec<Vec<ParamContour>> = Vec::with_capacity(z_levels.len());
    for &z in &z_levels {
        check_cancel(cancel)?;
        let raw = waterline_contours(mesh, index, cutter, z, params.sampling);
        level_contours.push(
            raw.iter()
                .filter(|c| c.len() >= 3)
                .map(|c| ParamContour::from_contour(c))
                .collect(),
        );
    }

    // Slope confinement bounds
    let slope_from_rad = params.slope_from.to_radians();
    let slope_to_rad = params.slope_to.to_radians();
    let use_slope_filter =
        crate::finish_setup::slope_filter_active(params.slope_from, params.slope_to);

    // Step length for ramp point generation (controls output resolution)
    let step_len = cell_size * 2.0;

    // Generate ramp paths between adjacent Z levels
    let mut all_ramp_segments: Vec<RampSegmentRecord> = Vec::new();

    let level_pairs: Vec<(usize, usize)> = (0..z_levels.len() - 1).map(|i| (i, i + 1)).collect();

    // Optionally reverse for bottom-up ordering
    let level_pairs: Vec<(usize, usize)> = if params.order_bottom_up {
        level_pairs.into_iter().rev().collect()
    } else {
        level_pairs
    };

    for (terrace_pos, &(upper_idx, lower_idx)) in level_pairs.iter().enumerate() {
        check_cancel(cancel)?;
        let upper_contours = &level_contours[upper_idx];
        let lower_contours = &level_contours[lower_idx];

        if upper_contours.is_empty() || lower_contours.is_empty() {
            continue;
        }

        let matches = match_contours(upper_contours, lower_contours);

        let terrace_index = terrace_pos + 1;
        let terrace_total = level_pairs.len();
        let upper_z = z_levels[upper_idx];
        let lower_z = z_levels[lower_idx];
        let mut terrace_segments = Vec::new();

        for &(ui, li) in &matches {
            let ramp_path = ramp_between_contours(
                &upper_contours[ui],
                &lower_contours[li],
                params.max_stepdown,
                step_len,
            );
            if ramp_path.len() < 2 {
                continue;
            }

            // Apply slope confinement if configured
            if use_slope_filter {
                let segments = crate::point_runs::split_runs(
                    &ramp_path,
                    |_, pt: &P3| {
                        slope_map
                            .angle_at_world(pt.x, pt.y)
                            .is_some_and(|a| a >= slope_from_rad && a <= slope_to_rad)
                    },
                    crate::point_runs::RunTopology::Open,
                    2,
                );
                for seg in segments {
                    terrace_segments.push(seg);
                }
            } else {
                terrace_segments.push(ramp_path);
            }
        }

        let ramp_total = terrace_segments.len();
        for (ramp_index, path) in terrace_segments.into_iter().enumerate() {
            all_ramp_segments.push(RampSegmentRecord {
                path,
                terrace_index,
                terrace_total,
                upper_level_index: upper_idx + 1,
                lower_level_index: lower_idx + 1,
                upper_z,
                lower_z,
                ramp_index: ramp_index + 1,
                ramp_total,
            });
        }
    }

    info!(
        segments = all_ramp_segments.len(),
        "Ramp finish: converting to toolpath"
    );

    // Convert to toolpath
    let mut tp = Toolpath::new();
    let mut annotations = Vec::new();

    // Handle direction: for Conventional, reverse each segment
    let should_reverse = matches!(params.direction, CutDirection::Conventional);

    for (i, segment) in all_ramp_segments.iter().enumerate() {
        check_cancel(cancel)?;
        let simplified = simplify_path_3d(&segment.path, params.tolerance);
        if simplified.len() < 2 {
            continue;
        }

        let path = if should_reverse
            || (matches!(params.direction, CutDirection::BothWays) && i % 2 == 1)
        {
            let mut rev = simplified;
            rev.reverse();
            rev
        } else {
            simplified
        };

        let move_index = tp.moves.len();
        tp.emit_path_segment_with_intent(
            &path,
            params.safe_z,
            params.feed_rate,
            params.plunge_rate,
            crate::toolpath::MoveIntent::FinishingCut,
        );
        annotations.push(RampFinishRuntimeAnnotation {
            move_index,
            event: RampFinishRuntimeEvent::Ramp {
                terrace_index: segment.terrace_index,
                terrace_total: segment.terrace_total,
                upper_level_index: segment.upper_level_index,
                lower_level_index: segment.lower_level_index,
                upper_z: segment.upper_z,
                lower_z: segment.lower_z,
                ramp_index: segment.ramp_index,
                ramp_total: segment.ramp_total,
            },
        });
    }

    tp.final_retract(params.safe_z);

    info!(
        moves = tp.moves.len(),
        cutting_mm = format!("{:.1}", tp.total_cutting_distance()),
        rapid_mm = format!("{:.1}", tp.total_rapid_distance()),
        "Ramp finish toolpath complete"
    );

    if let Some(debug_ctx) = debug {
        for annotation in &annotations {
            debug_ctx.add_annotation(annotation.move_index, annotation.event.label());
        }
    }

    Ok((tp, annotations))
}

pub fn ramp_finish_toolpath_annotated(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &RampFinishParams,
    debug: Option<&ToolpathDebugContext>,
) -> (Toolpath, Vec<(usize, String)>) {
    let (tp, annotations) =
        ramp_finish_toolpath_structured_annotated(mesh, index, cutter, params, debug);
    (tp, runtime_annotations_to_labels(&annotations))
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::mesh::SpatialIndex;
    use crate::slope::SlopeMap;
    use crate::tool::BallEndmill;

    fn make_hemisphere() -> (TriangleMesh, SpatialIndex) {
        let mesh = crate::mesh::make_test_hemisphere(20.0, 16);
        let si = SpatialIndex::build(&mesh, 10.0);
        (mesh, si)
    }

    fn ball_cutter() -> BallEndmill {
        BallEndmill::new(6.35, 25.0)
    }

    fn default_params() -> RampFinishParams {
        RampFinishParams {
            max_stepdown: 1.0,
            sampling: 2.0,
            tolerance: 0.2,
            safe_z: 30.0,
            ..RampFinishParams::default()
        }
    }

    // ── ParamContour tests ──────────────────────────────────────────

    #[test]
    fn test_param_contour_arc_length() {
        let contour = vec![
            P3::new(0.0, 0.0, 5.0),
            P3::new(10.0, 0.0, 5.0),
            P3::new(10.0, 10.0, 5.0),
        ];
        let pc = ParamContour::from_contour(&contour);

        assert!(
            (pc.total_length - 20.0).abs() < 0.01,
            "Total length should be ~20"
        );
        assert!((pc.params[0] - 0.0).abs() < 0.01);
        assert!((pc.params[1] - 0.5).abs() < 0.01);
        assert!((pc.params[2] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_param_contour_interpolation() {
        let contour = vec![P3::new(0.0, 0.0, 10.0), P3::new(10.0, 0.0, 10.0)];
        let pc = ParamContour::from_contour(&contour);

        let mid = pc.point_at(0.5);
        assert!(
            (mid.x - 5.0).abs() < 0.01,
            "Midpoint X should be 5, got {:.2}",
            mid.x
        );
        assert!(
            (mid.z - 10.0).abs() < 0.01,
            "Midpoint Z should be 10, got {:.2}",
            mid.z
        );

        let start = pc.point_at(0.0);
        assert!((start.x - 0.0).abs() < 0.01);

        let end = pc.point_at(1.0);
        assert!((end.x - 10.0).abs() < 0.01);
    }

    // ── Contour matching tests ──────────────────────────────────────

    #[test]
    fn test_match_contours_single() {
        let upper = vec![ParamContour::from_contour(&[
            P3::new(0.0, 0.0, 10.0),
            P3::new(10.0, 0.0, 10.0),
            P3::new(10.0, 10.0, 10.0),
        ])];
        let lower = vec![ParamContour::from_contour(&[
            P3::new(0.5, 0.5, 9.0),
            P3::new(9.5, 0.5, 9.0),
            P3::new(9.5, 9.5, 9.0),
        ])];

        let matches = match_contours(&upper, &lower);
        assert_eq!(matches.len(), 1, "Should match the single contour pair");
        assert_eq!(matches[0], (0, 0));
    }

    #[test]
    fn test_match_contours_multiple() {
        // Two contours at each level, should match by nearest centroid
        let upper = vec![
            ParamContour::from_contour(&[
                P3::new(0.0, 0.0, 10.0),
                P3::new(5.0, 0.0, 10.0),
                P3::new(5.0, 5.0, 10.0),
            ]),
            ParamContour::from_contour(&[
                P3::new(20.0, 20.0, 10.0),
                P3::new(25.0, 20.0, 10.0),
                P3::new(25.0, 25.0, 10.0),
            ]),
        ];
        let lower = vec![
            ParamContour::from_contour(&[
                P3::new(20.5, 20.5, 9.0),
                P3::new(24.5, 20.5, 9.0),
                P3::new(24.5, 24.5, 9.0),
            ]),
            ParamContour::from_contour(&[
                P3::new(0.5, 0.5, 9.0),
                P3::new(4.5, 0.5, 9.0),
                P3::new(4.5, 4.5, 9.0),
            ]),
        ];

        let matches = match_contours(&upper, &lower);
        assert_eq!(matches.len(), 2);
        // Upper[0] (near origin) should match lower[1] (near origin)
        assert_eq!(matches[0], (0, 1));
        // Upper[1] (near 20,20) should match lower[0] (near 20,20)
        assert_eq!(matches[1], (1, 0));
    }

    // ── Ramp path generation tests ──────────────────────────────────

    #[test]
    fn test_ramp_continuous_z() {
        let upper = ParamContour::from_contour(&[
            P3::new(0.0, 0.0, 10.0),
            P3::new(10.0, 0.0, 10.0),
            P3::new(10.0, 10.0, 10.0),
            P3::new(0.0, 10.0, 10.0),
        ]);
        let lower = ParamContour::from_contour(&[
            P3::new(0.0, 0.0, 8.0),
            P3::new(10.0, 0.0, 8.0),
            P3::new(10.0, 10.0, 8.0),
            P3::new(0.0, 10.0, 8.0),
        ]);

        let path = ramp_between_contours(&upper, &lower, 1.0, 0.5);
        assert!(path.len() > 10, "Should produce non-trivial path");

        // Z should always decrease (or stay same) along the path
        for window in path.windows(2) {
            let dz = window[0].z - window[1].z;
            assert!(
                dz >= -0.01,
                "Z should never increase: {:.3} -> {:.3}",
                window[0].z,
                window[1].z
            );
        }

        // First point should be near z=10, last near z=8
        assert!(
            (path[0].z - 10.0).abs() < 0.1,
            "Start Z should be ~10, got {:.2}",
            path[0].z
        );
        let last_pt = path.last().expect("path should be non-empty");
        assert!(
            (last_pt.z - 8.0).abs() < 0.1,
            "End Z should be ~8, got {:.2}",
            last_pt.z
        );
    }

    #[test]
    fn test_ramp_stepdown_limit() {
        // With max_stepdown=0.5 and 2mm Z drop, should take >=4 revolutions
        let upper = ParamContour::from_contour(&[
            P3::new(0.0, 0.0, 10.0),
            P3::new(10.0, 0.0, 10.0),
            P3::new(10.0, 10.0, 10.0),
            P3::new(0.0, 10.0, 10.0),
        ]);
        let lower = ParamContour::from_contour(&[
            P3::new(0.0, 0.0, 8.0),
            P3::new(10.0, 0.0, 8.0),
            P3::new(10.0, 10.0, 8.0),
            P3::new(0.0, 10.0, 8.0),
        ]);

        let path = ramp_between_contours(&upper, &lower, 0.5, 0.5);

        // Check that no consecutive segment drops more than max_stepdown
        // over one revolution's worth of points
        let contour_len = upper.total_length;
        let points_per_rev = (contour_len / 0.5).ceil() as usize;
        if path.len() > points_per_rev {
            for i in 0..(path.len() - points_per_rev) {
                let z_drop = path[i].z - path[i + points_per_rev].z;
                assert!(
                    z_drop <= 0.5 + 0.05,
                    "Z drop per revolution should be <= 0.5, got {:.3} at index {}",
                    z_drop,
                    i
                );
            }
        }
    }

    // ── Slope confinement tests ─────────────────────────────────────

    #[test]
    fn test_slope_confinement_filters() {
        // Create a slope map with known values
        let rows = 10;
        let cols = 10;
        let cs = 1.0;
        // Ramp surface: dz/dx=1 → 45° slope
        let mut z_values = vec![0.0; rows * cols];
        for row in 0..rows {
            for col in 0..cols {
                z_values[row * cols + col] = col as f64;
            }
        }
        let slope_map = SlopeMap::from_z_grid(&z_values, rows, cols, 0.0, 0.0, cs);

        // Path through the grid
        let path: Vec<P3> = (0..10)
            .map(|i| P3::new(i as f64, 5.0, 10.0 - i as f64))
            .collect();

        let confined_segments = |from_rad: f64, to_rad: f64| -> Vec<Vec<P3>> {
            crate::point_runs::split_runs(
                &path,
                |_, pt: &P3| {
                    slope_map
                        .angle_at_world(pt.x, pt.y)
                        .is_some_and(|a| a >= from_rad && a <= to_rad)
                },
                crate::point_runs::RunTopology::Open,
                2,
            )
        };

        // slope_from=30, slope_to=90: surface is 45°, should pass
        let segs = confined_segments(30.0_f64.to_radians(), 90.0_f64.to_radians());
        assert!(!segs.is_empty(), "45° surface should pass 30-90° filter");

        // slope_from=50, slope_to=90: surface is 45°, should fail
        let segs = confined_segments(50.0_f64.to_radians(), 90.0_f64.to_radians());
        assert!(segs.is_empty(), "45° surface should fail 50-90° filter");
    }

    // ── Integration tests ───────────────────────────────────────────

    #[test]
    fn test_ramp_produces_toolpath() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = RampFinishParams {
            max_stepdown: 2.0,
            sampling: 3.0,
            tolerance: 0.5,
            ..default_params()
        };

        let tp = ramp_finish_toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.moves.len() > 10,
            "Hemisphere ramp should produce moves, got {}",
            tp.moves.len()
        );
        assert!(
            tp.total_cutting_distance() > 10.0,
            "Should have meaningful cutting distance, got {:.1}",
            tp.total_cutting_distance()
        );
    }

    #[test]
    fn test_ramp_slope_confinement() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();

        // Only machine steep areas (>30°)
        let params = RampFinishParams {
            max_stepdown: 2.0,
            sampling: 3.0,
            tolerance: 0.5,
            slope_from: 30.0,
            slope_to: 90.0,
            ..default_params()
        };

        let tp_confined = ramp_finish_toolpath(&mesh, &si, &cutter, &params);

        // Also run with full range for comparison
        let params_full = RampFinishParams {
            slope_from: 0.0,
            slope_to: 90.0,
            ..params
        };
        let tp_full = ramp_finish_toolpath(&mesh, &si, &cutter, &params_full);

        // Confined should have fewer moves than full
        assert!(
            tp_confined.total_cutting_distance() <= tp_full.total_cutting_distance() + 1.0,
            "Confined ({:.0}mm) should be <= full ({:.0}mm)",
            tp_confined.total_cutting_distance(),
            tp_full.total_cutting_distance()
        );
    }

    #[test]
    fn test_ramp_bottom_up() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = RampFinishParams {
            max_stepdown: 2.0,
            sampling: 3.0,
            tolerance: 0.5,
            order_bottom_up: true,
            ..default_params()
        };

        let tp = ramp_finish_toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.moves.len() > 5,
            "Bottom-up ramp should produce moves, got {}",
            tp.moves.len()
        );
    }

    // ── Path simplification tests ───────────────────────────────────

    #[test]
    fn test_simplify_collinear() {
        let path = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(1.0, 0.0, 0.0),
            P3::new(2.0, 0.0, 0.0),
            P3::new(3.0, 0.0, 0.0),
        ];
        let simplified = simplify_path_3d(&path, 0.01);
        assert_eq!(simplified.len(), 2, "Collinear points should reduce to 2");
    }

    #[test]
    fn test_simplify_preserves_corners() {
        let path = vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(5.0, 5.0, 0.0),
            P3::new(10.0, 0.0, 0.0),
        ];
        let simplified = simplify_path_3d(&path, 0.01);
        assert_eq!(simplified.len(), 3, "Corner should be preserved");
    }
}
