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
use crate::finish_setup::FinishResolutionPolicy;
use crate::geo::{P2, P3};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
#[cfg(test)]
use crate::polygon::Polygon2;
use crate::region_set::RegionSet;
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

/// What the reach clamp did to one ramp-finish run (PR-8b).
///
/// Report-only. It is the channel `CHECKPOINT_B_EVIDENCE.md` §8.2 says did
/// not exist: "a genuine reach failure with **no diagnostic channel**; a user
/// would ship it."
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct RampReachClamp {
    /// Ramp points whose Z was raised because the cutter could not hold the
    /// commanded depth at that XY.
    pub clamped_points: usize,
    /// Ramp points examined. `clamped_points == 0` with a non-zero total is a
    /// measured-clean run; both zero means no ramp path was built at all.
    pub ramp_points: usize,
    /// Largest single lift (mm). This is material the ramp intended to remove
    /// and did not.
    pub max_lift_mm: f64,
    /// The Z ladder's bottom before the clamp — `SurfaceHeightmap::min_z`,
    /// which on a padded finish grid is the MESH bbox floor.
    pub requested_bottom_z_mm: f64,
    /// The Z ladder's bottom after the clamp — the deepest tool-centre Z the
    /// surface says this cutter can hold anywhere on this model.
    pub holdable_bottom_z_mm: f64,
}

impl RampReachClamp {
    /// True when the clamp changed nothing: the ladder bottom was already
    /// holdable and no ramp point was lifted.
    #[must_use]
    pub fn is_inert(&self) -> bool {
        self.clamped_points == 0
            && (self.requested_bottom_z_mm - self.holdable_bottom_z_mm).abs() <= 1e-9
    }

    fn record_lift(&mut self, lift_mm: f64) {
        self.clamped_points += 1;
        if lift_mm > self.max_lift_mm {
            self.max_lift_mm = lift_mm;
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

/// The resolution policy ramp-finish generates on (H3 step 2; moved by PR-8a).
///
/// RampFinish selects [`crate::finish_setup::FinishResolutionMode::GeoMeanEnvelopeCusp`] — the
/// geometric mean of `envelope/4` and `cusp/4`. It selected
/// `LegacyEnvelopeQuarter` until PR-8a, under the approved Checkpoint B.
///
/// # Why this op moved and its two siblings did not
///
/// The generation cell reaches ramp-finish's emitted geometry through
/// `step_len = cell_size * 2`, the spacing at which
/// [`ramp_between_contours`] samples the descent. A ramp point is a straight
/// chord between two contour samples, so a coarse cell means long chords
/// across curved walls, and the chord cuts inside the surface it spans.
/// `CHECKPOINT_B_EVIDENCE.md` §3.2 measured that directly: at
/// `envelope/4` (0.750 mm on the shipped Ø1-tip / Ø6-shank taper, i.e. the
/// SHANK) the narrow-valley fixture gouged **2.39 mm** and the narrow ridge
/// **0.16 mm**, and every arm finer than it eliminated both outright —
/// deepest gouge exactly 0.0000 mm, zero samples past 50 µm.
///
/// The win arrives HERE and not deeper. At this cell the two fixtures are
/// already gouge-free for 1.3–2.6× generation time and +11–43% moves;
/// `CuspQuarter` buys nothing further and costs 3.1–11.7×. That is the whole
/// case for a named intermediate mode rather than following the
/// classification grid.
///
/// Scallop and steep/shallow select independently and were NOT moved:
/// scallop's cusp-scaled arm creates 19–33 mm² of standing material the
/// legacy cell does not (gated behind Checkpoint C), and steep/shallow's
/// output is bit-identical across every arm (deferred pending a
/// discriminating fixture — see `steep_shallow_generation_resolution`).
///
/// On a plain ball the geometric mean of two equal cells is that cell, so
/// ball output is byte-identical to pre-PR-8a; only tapered tools move.
#[must_use]
pub fn ramp_finish_generation_resolution(
    cutter: &dyn MillingCutter,
    tolerance: f64,
) -> FinishResolutionPolicy {
    FinishResolutionPolicy::geo_mean_envelope_cusp(cutter, tolerance)
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
    let (tp, _, _) =
        ramp_finish_toolpath_structured_annotated(mesh, index, cutter, params, None, None);
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
    let (tp, _, _) = ramp_finish_toolpath_structured_annotated_with_cancel(
        mesh, index, cutter, params, None, None, cancel,
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
    boundary_regions: Option<&RegionSet<'_>>,
) -> (Toolpath, Vec<RampFinishRuntimeAnnotation>, RampReachClamp) {
    let never_cancel = || false;
    ramp_finish_toolpath_structured_annotated_with_cancel(
        mesh,
        index,
        cutter,
        params,
        debug,
        boundary_regions,
        &never_cancel,
    )
    .expect("non-cancellable ramp finish toolpath should never be cancelled")
}

/// Cancellable variant of [`ramp_finish_toolpath_structured_annotated`].
/// Polls `cancel` once per Z level while building waterline contours, once
/// per terrace (adjacent Z-level pair) while ramping between them, and once
/// per ramp segment during toolpath emission.
///
/// `boundary_regions` (P2.3): folded into the ramp-path keep predicate
/// alongside the slope filter — a point survives only when it's in the
/// slope band (when active) AND inside a machining-boundary region (when
/// given). The split always runs when either filter is active; `None`
/// reproduces today's slope-only (or unfiltered) output byte-for-byte.
#[allow(clippy::too_many_arguments)]
pub fn ramp_finish_toolpath_structured_annotated_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &RampFinishParams,
    debug: Option<&ToolpathDebugContext>,
    boundary_regions: Option<&RegionSet<'_>>,
    cancel: &dyn CancelCheck,
) -> Result<(Toolpath, Vec<RampFinishRuntimeAnnotation>, RampReachClamp), Cancelled> {
    ramp_finish_toolpath_structured_annotated_with_resolution(
        mesh,
        index,
        cutter,
        params,
        debug,
        boundary_regions,
        ramp_finish_generation_resolution(cutter, params.tolerance),
        cancel,
    )
}

/// [`ramp_finish_toolpath_structured_annotated_with_cancel`] with the
/// generation grid resolution supplied by the caller.
///
/// **Research seam, not a production entry point** — see
/// [`crate::scallop::scallop_toolpath_structured_annotated_with_resolution`]
/// for why H3's Checkpoint B harness needs one. Passing
/// `ramp_finish_generation_resolution(cutter, params.tolerance)` reproduces
/// the shipped path exactly.
#[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
pub fn ramp_finish_toolpath_structured_annotated_with_resolution(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &RampFinishParams,
    debug: Option<&ToolpathDebugContext>,
    boundary_regions: Option<&RegionSet<'_>>,
    resolution: FinishResolutionPolicy,
    cancel: &dyn CancelCheck,
) -> Result<(Toolpath, Vec<RampFinishRuntimeAnnotation>, RampReachClamp), Cancelled> {
    check_cancel(cancel)?;
    let bbox = &mesh.bbox;

    // Build surface heightmap and slope map (shared setup, see finish_setup.rs).
    // The RESOLUTION is ramp_finish's own choice (H3 step 2) — see
    // `ramp_finish_generation_resolution`, which the shipped wrapper above
    // passes in.
    let surface = crate::finish_setup::build_finish_surface_with_policy_and_cancel(
        mesh, index, cutter, resolution, cancel,
    )?;
    let surface_hm = surface.heightmap;
    let slope_map = surface.slope_map;
    let cell_size = surface_hm.cell_size;

    // ── Compute Z range, reach-clamped (PR-8b) ───────────────────────────
    //
    // `CHECKPOINT_B_EVIDENCE.md` §8.2: this op gouged 4.2 mm on the
    // `patches + hole` fixture at EVERY resolution, with no diagnostic. Two
    // separate commanded-below-reach errors produce it, and the policy below
    // is ONE rule stated at two scales: **the cutter is never commanded below
    // the depth it can hold there.**
    //
    // GLOBAL form — the ladder bottom. `min_z()` is the minimum over ALL
    // cells, and a finish grid is padded by one envelope radius per side, so
    // uncovered cells carrying the `min_z` clamp are always present: the
    // ladder bottom was therefore the MESH BBOX FLOOR on essentially every
    // ramp-finish run. Measured on that fixture: −3.000 requested against
    // −2.407 holdable, i.e. two whole terraces below anything the profile can
    // reach. `min_covered_z()` is the deepest the tool's reference point can
    // descend anywhere on this surface.
    //
    // LOCAL form — the per-point clamp further down, which is what actually
    // removes the 4.2 mm gouge; see the comment at that site for the measured
    // reason the global form alone does not.
    let z_top = bbox.max.z + params.stock_to_leave;
    let requested_bottom = surface_hm.min_z() + params.stock_to_leave;
    let holdable_bottom = surface_hm
        .min_covered_z()
        .map_or(requested_bottom, |z| z + params.stock_to_leave);
    let z_bottom = requested_bottom.max(holdable_bottom);
    let mut reach_clamp = RampReachClamp {
        requested_bottom_z_mm: requested_bottom,
        holdable_bottom_z_mm: z_bottom,
        ..RampReachClamp::default()
    };
    let z_step = params.max_stepdown;

    // Generate Z levels. `snap_to_bottom = true` guarantees the ladder ends
    // exactly at `z_bottom` (needed so the final terrace's lower contour is
    // the true bottom, not an arbitrary short-of-bottom level); epsilon is
    // half a step, matching the original inline arithmetic exactly.
    let z_levels = crate::finish_setup::z_ladder(z_top, z_bottom, z_step, z_step * 0.5, true);

    if z_levels.len() < 2 {
        info!("Ramp finish: insufficient Z range for ramping");
        return Ok((Toolpath::new(), Vec::new(), reach_clamp));
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
            let mut ramp_path = ramp_between_contours(
                &upper_contours[ui],
                &lower_contours[li],
                params.max_stepdown,
                step_len,
            );
            if ramp_path.len() < 2 {
                continue;
            }

            // ── The LOCAL form of the reach clamp (PR-8b) ────────────────
            //
            // Every point on this path is a BLEND: `ramp_between_contours`
            // pairs the upper and lower contours by arc-length parameter and
            // interpolates XY between them, after `match_contours` paired the
            // two loops by nearest centroid. Neither correspondence is
            // geometric, so a blended point can land anywhere between the two
            // loops — including on ground that has nothing to do with either.
            //
            // MEASURED, and it refuted the §8.2 hypothesis: on the
            // `patches + hole` fixture the deepest gouge (−4.229 mm) is NOT
            // in the 62° pit at all. It sits at (3.981, 4.113) — the flank of
            // a convex 50° DOME — with the path at z = −1.757 and the
            // reachable tool-centre surface at z = +2.472. Clamping only the
            // ladder bottom leaves it unchanged at −4.229 (probed, then
            // reverted); it is a blend artefact, not a ladder-depth artefact,
            // and no valley-reach model has anything to say about a point on
            // a dome. So the clamp has to be per point.
            //
            // The floor is the drop-cutter contact answer AT THIS POINT —
            // `point_drop_cutter`, the same query that builds the generation
            // surface, but asked at the ramp point's own XY instead of read
            // off a grid node. Deliberately NOT a grid lookup: the defect is
            // RESOLUTION-INDEPENDENT (§8.2 measured it identical at all four
            // arms), so a resolution-dependent clamp would leave a
            // resolution-dependent residue. Measured on `patches + hole` with
            // the shipped 0.306 mm cell: the nearest-cell grid lookup lands
            // the worst gouge at −0.225 mm (half a cell across a 50° flank —
            // pure discretisation), the exact query at −0.000. Cost is one
            // drop-cutter query per ramp point against the ~5 000 the surface
            // build already runs.
            //
            // Off the model footprint the query contacts nothing and reads
            // the same `bbox.min.z` floor the heightmap uses, so it declines
            // to clamp — which is right: there is nothing there to gouge.
            //
            // Raise, never drop: a lifted point is a real cut of the surface
            // it now rides, and dropping it would replace a conservative pass
            // with a retract/replunge the operator never asked for. What is
            // NOT removed is the material below it — that is the finding.
            for pt in &mut ramp_path {
                let contact =
                    crate::dropcutter::point_drop_cutter(pt.x, pt.y, mesh, index, cutter).z;
                let floor = contact.max(bbox.min.z) + params.stock_to_leave;
                if pt.z < floor {
                    reach_clamp.record_lift(floor - pt.z);
                    pt.z = floor;
                }
            }
            reach_clamp.ramp_points += ramp_path.len();

            // Apply slope confinement and/or the machining-boundary regions
            // if either is configured; a plain unfiltered push otherwise
            // (byte-identical to pre-P2.3 behavior when neither is active).
            if use_slope_filter || boundary_regions.is_some() {
                let segments = crate::point_runs::split_runs(
                    &ramp_path,
                    |_, pt: &P3| {
                        let slope_ok = !use_slope_filter
                            || slope_map
                                .angle_at_world(pt.x, pt.y)
                                .is_some_and(|a| a >= slope_from_rad && a <= slope_to_rad);
                        let region_ok = boundary_regions
                            .is_none_or(|regions| regions.contains(&P2::new(pt.x, pt.y)));
                        slope_ok && region_ok
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

    Ok((tp, annotations, reach_clamp))
}

pub fn ramp_finish_toolpath_annotated(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &RampFinishParams,
    debug: Option<&ToolpathDebugContext>,
) -> (Toolpath, Vec<(usize, String)>) {
    let (tp, annotations, _clamp) =
        ramp_finish_toolpath_structured_annotated(mesh, index, cutter, params, debug, None);
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

    // ── P2.3: boundary_regions pre-clip ──────────────────────────────

    #[test]
    fn ramp_boundary_regions_none_matches_call_without_param() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = RampFinishParams {
            max_stepdown: 2.0,
            sampling: 3.0,
            tolerance: 0.5,
            ..default_params()
        };
        let never_cancel = || false;

        let tp_default = ramp_finish_toolpath(&mesh, &si, &cutter, &params);
        let (tp_none, _, _) = ramp_finish_toolpath_structured_annotated_with_cancel(
            &mesh,
            &si,
            &cutter,
            &params,
            None,
            None,
            &never_cancel,
        )
        .unwrap();

        assert_eq!(tp_default.moves.len(), tp_none.moves.len());
        for (a, b) in tp_default.moves.iter().zip(tp_none.moves.iter()) {
            assert!((a.target.x - b.target.x).abs() < 1e-9);
            assert!((a.target.y - b.target.y).abs() < 1e-9);
            assert!((a.target.z - b.target.z).abs() < 1e-9);
        }
    }

    #[test]
    fn ramp_boundary_regions_confines_cuts_to_region() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = RampFinishParams {
            max_stepdown: 2.0,
            sampling: 3.0,
            tolerance: 0.5,
            ..default_params()
        };
        let never_cancel = || false;

        let bbox = &mesh.bbox;
        let left_half = Polygon2::new(vec![
            P2::new(bbox.min.x, bbox.min.y),
            P2::new(0.0, bbox.min.y),
            P2::new(0.0, bbox.max.y),
            P2::new(bbox.min.x, bbox.max.y),
        ]);

        let left_half_regions = std::slice::from_ref(&left_half);
        let region_set = RegionSet::from_slice(left_half_regions);
        let (tp, _, _) = ramp_finish_toolpath_structured_annotated_with_cancel(
            &mesh,
            &si,
            &cutter,
            &params,
            None,
            Some(&region_set),
            &never_cancel,
        )
        .unwrap();

        let tol = 1e-6;
        let mut saw_cut = false;
        for m in &tp.moves {
            let is_cut = matches!(m.move_type, crate::toolpath::MoveType::Linear { .. })
                && m.intent == crate::toolpath::MoveIntent::FinishingCut;
            if is_cut {
                saw_cut = true;
                assert!(
                    m.target.x <= tol,
                    "cutting move X={:.3} escaped the left-half boundary region",
                    m.target.x
                );
            }
        }
        assert!(saw_cut, "expected at least one cutting move");
    }
}
