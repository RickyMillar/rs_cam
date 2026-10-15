//! Waterline algorithm — generates closed contour toolpaths at constant Z heights.
//!
//! Uses the push-cutter algorithm to find cutter contact intervals on X and Y fibers,
//! then extracts CL boundary points and connects them into closed contours.
//!
//! The algorithm:
//! 1. Generate grids of X-fibers and Y-fibers at the target Z height
//! 2. Run batch push-cutter on both fiber sets
//! 3. Extract CL boundary points from interval endpoints
//! 4. Connect boundary points into closed loops using nearest-neighbor chaining

use crate::geo::{P2, P3};
use crate::geometry::contour_extract::weave_contours;
use crate::geometry::fiber::Fiber;
use crate::geometry::region_set::RegionSet;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::machine::kinematics::{LinkKinematics, retract_link_time, surface_link_time};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::surface::pushcutter::batch_push_cutter_with_cancel;
use crate::tool::MillingCutter;
use crate::toolpath::{MoveIntent, Toolpath};

/// Parameters for waterline toolpath generation.
pub struct WaterlineParams {
    /// Fiber sampling spacing (mm). Smaller = more accurate but slower.
    pub sampling: f64,
    /// Feed rate for cutting moves (mm/min).
    pub feed_rate: f64,
    /// Plunge rate (mm/min).
    pub plunge_rate: f64,
    /// Safe Z for rapid moves.
    pub safe_z: f64,
    /// Stock to leave (mm), applied as a **+Z shift on every emitted contour
    /// point** — the repo-wide finish convention (`scallop.rs`'s
    /// `cl.z + stock_to_leave`, `steep_shallow.rs`'s `z_adjusted = z +
    /// stock_to_leave` for exactly this contour shape).
    ///
    /// Added by F3 / D-16.2 (2026-08-06). Until then `WaterlineParams` had no
    /// such field, so the `UnifiedFinish` VerySteep band **could not** pass
    /// one and silently dropped the operator's dial — the twin of the Shallow
    /// band's defect, found while mapping it
    /// (`planning/review_2026-08-04/FINISHING_OPEN_DEFECTS_EVIDENCE.md` §3.B).
    /// Fixing only Shallow would have put a `stock_to_leave`-sized step at
    /// the shallow↔waterline seam that does not exist today, so both bands
    /// were fixed together.
    ///
    /// `0.0` reproduces the pre-fix toolpath byte-for-byte: the lift is
    /// skipped entirely, not applied as `+0.0`.
    ///
    /// **Known approximation, deliberately shared rather than corrected
    /// here.** A vertical lift leaves `stock_to_leave·cos θ` measured normal
    /// to the surface, which on the near-vertical walls this band exists for
    /// is a small fraction of the dialled value. Every finish op in the repo
    /// makes the same approximation; whether the convention should become
    /// normal-direction is a separate repo-wide question and is NOT coupled
    /// to this field.
    pub stock_to_leave: f64,
}

/// Generate a single waterline contour at a given Z height.
///
/// Returns boundary CL points organized as closed loops.
pub fn waterline_contours(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    z: f64,
    sampling: f64,
) -> Vec<Vec<P3>> {
    crate::interrupt::run_uncancellable(|cancel| {
        waterline_contours_with_cancel(mesh, index, cutter, z, sampling, cancel)
    })
}

/// Inclusive-bounds epsilon for waterline's Z ladder.
///
/// Eight orders of magnitude tighter than
/// [`crate::finish::finish_setup::Z_LADDER_DEFAULT_EPSILON`], which is what
/// `steep_shallow` uses. That is not an oversight on either side: this value
/// reproduces the generator's original `while z >= final_z - 1e-10` loop
/// exactly, and waterline's callers pass ladder bounds taken straight from a
/// mesh bbox, where a 0.01 mm tolerance would silently add a level.
pub const WATERLINE_LADDER_EPSILON: f64 = 1e-10;

/// Z levels a waterline pass will cut at, from `start_z` down to `final_z`.
///
/// Exposed so callers building depth-run spans can pass waterline's REAL
/// level ladder (R2.8) instead of an empty slice; the generator below
/// consumes the same helper, so there is one ladder per operation and — since
/// C3 — one ladder IMPLEMENTATION for the crate.
///
/// C3: this was a private copy of [`crate::ops::depth::z_ladder`]'s
/// `snap_to_bottom = false` arm, differing only in hard-coding its epsilon
/// where the shared version takes one. It is now an adapter that names the
/// epsilon and delegates. `tests/waterline_shared_finish_setup_c3.rs` pins
/// the two against each other across the boundary cases (exact multiples, a
/// remainder, an inverted range, a zero range).
pub fn waterline_z_levels(start_z: f64, final_z: f64, z_step: f64) -> Vec<f64> {
    crate::ops::depth::z_ladder(
        start_z,
        final_z,
        z_step,
        crate::ops::depth::LadderMode::ConstantStep {
            epsilon: WATERLINE_LADDER_EPSILON,
            snap_to_bottom: false,
        },
    )
}

/// A facet whose unit normal has `|n.z|` above this is horizontal for the
/// flat-face nudge in [`waterline_ladder`].
pub const FLAT_FACET_NORMAL_Z: f64 = 0.999;
/// A level within this of a horizontal facet's Z is ON that face.
pub const FLAT_FACE_SNAP_MM: f64 = 1e-3;
/// How far the nudge moves a level DOWN off a horizontal facet.
pub const FLAT_FACE_NUDGE_MM: f64 = 0.01;
/// A level at or below `stock_bottom_z + this` is dropped (R4).
pub const STOCK_BOTTOM_FLOOR_EPS_MM: f64 = 1e-6;

/// What [`waterline_ladder`] did to the requested ladder.
///
/// The counts are the adapter's finding material: `planned_levels` is the
/// raw [`waterline_z_levels`] count, `nudged_levels` how many of those the
/// flat-face nudge moved, `dropped_below_stock` how many the stock-bottom
/// floor removed, and `levels` the ladder that is cut AND spanned.
#[derive(Debug, Clone, PartialEq)]
pub struct WaterlineLadder {
    /// The levels to cut, top first, after the nudge and the floor.
    pub levels: Vec<f64>,
    /// Levels the raw ladder asked for.
    pub planned_levels: usize,
    /// Levels moved down off a horizontal facet.
    pub nudged_levels: usize,
    /// Levels removed because they sat at or below the stock bottom.
    pub dropped_below_stock: usize,
}

/// Unique Z values of the mesh's horizontal facets, rounded to 1e-6 and
/// sorted ascending.
pub fn flat_facet_z_values(mesh: &TriangleMesh) -> Vec<f64> {
    let mut zs: Vec<f64> = mesh
        .faces
        .iter()
        .filter(|f| f.normal.z.abs() > FLAT_FACET_NORMAL_Z)
        // A horizontal facet's bbox has one Z.
        .map(|f| (f.bbox.min.z * 1e6).round() / 1e6)
        .collect();
    zs.sort_by(f64::total_cmp);
    zs.dedup();
    zs
}

/// The ladder a standalone waterline cuts, built ONCE for the generator and
/// the span builder (R3 / R4, Corne case 2026-09-18).
///
/// Three steps, in this order:
///
/// 1. [`waterline_z_levels`]`(start_z, final_z, z_step)` — the raw ladder.
/// 2. The flat-face nudge (§4.5 of the analysis): a level within
///    [`FLAT_FACE_SNAP_MM`] of a horizontal facet's Z moves DOWN by
///    [`FLAT_FACE_NUDGE_MM`], never up. On the Corne case the levels at
///    18.0 (the wall top) and 15.0 (the USB notch floor) wove junk across
///    those faces; a fibre exactly on a flat face reads it as a wall.
/// 3. The stock-bottom floor (R4): every level at or below
///    `stock_bottom_z` is dropped. A level AT the stock bottom cuts the bed
///    by definition; the pinned `-1.37` top on the Corne waterline gave one
///    level below the part that cut the bed.
///
/// Nudge before floor, so a level at a flat model bottom that coincides
/// with the stock bottom nudges to below the stock and is then dropped.
pub fn waterline_ladder(
    mesh: &TriangleMesh,
    start_z: f64,
    final_z: f64,
    z_step: f64,
    stock_bottom_z: f64,
) -> WaterlineLadder {
    let raw = waterline_z_levels(start_z, final_z, z_step);
    let planned_levels = raw.len();
    let flats = flat_facet_z_values(mesh);
    let mut nudged_levels = 0;
    let mut dropped_below_stock = 0;
    let mut levels = Vec::with_capacity(planned_levels);
    for z in raw {
        let on_flat = flats.iter().any(|fz| (fz - z).abs() < FLAT_FACE_SNAP_MM);
        let z = if on_flat {
            nudged_levels += 1;
            z - FLAT_FACE_NUDGE_MM
        } else {
            z
        };
        if z <= stock_bottom_z + STOCK_BOTTOM_FLOOR_EPS_MM {
            dropped_below_stock += 1;
            continue;
        }
        levels.push(z);
    }
    WaterlineLadder {
        levels,
        planned_levels,
        nudged_levels,
        dropped_below_stock,
    }
}

/// PR-8d's minimum-segment floor, inherited by waterline at C3.
///
/// `contour_extract::weave_contours` places each cell-edge vertex at an
/// EXACT fiber interval boundary, so two adjacent cells whose boundaries
/// resolve to the same crossing chain two vertices that are coincident to
/// floating-point noise. `steep_shallow.rs` fixed this at its emission site
/// in PR-8d and left waterline alone to keep that commit one operation wide
/// — but waterline is where the offenders Checkpoint B §8.1 attributed to
/// the steep half actually came from: `steep_shallow` generates its steep
/// passes by calling `waterline_contours`.
///
/// Measured on the §8.1 mixed-slope ribbon: 2 cutting segments of
/// **0.000891 mm** — the same value §8.1 reports, bit for bit — removed at
/// zero cost in cutting length.
///
/// `closed` matches the emitter chosen at the call site: a whole-contour
/// survivor chords back to its start, so a trailing vertex within the floor
/// of the FIRST would make THAT move the degenerate one.
fn floor_contour(points: &[P3], closed: bool) -> Vec<P3> {
    crate::toolpath::drop_sub_minimum_segments(
        points,
        crate::toolpath::MIN_EMITTED_SEGMENT_MM,
        closed,
    )
}

/// Generate waterline toolpaths at multiple Z heights.
///
/// Z heights are generated from start_z down to final_z with the given step.
#[tracing::instrument(skip(mesh, index, cutter, params), fields(
    start_z, final_z, z_step,
    tri_count = mesh.triangles.len(),
))]
pub fn waterline_toolpath(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    start_z: f64,
    final_z: f64,
    z_step: f64,
    params: &WaterlineParams,
) -> Toolpath {
    crate::interrupt::run_uncancellable(|cancel| {
        waterline_toolpath_with_cancel(
            mesh, index, cutter, start_z, final_z, z_step, params, None, cancel,
        )
    })
}

/// `boundary_regions` (P2.3): after each Z level's closed contours come back
/// from [`waterline_contours_with_cancel`], every contour is run through the
/// shared run-splitter with a "point is inside a boundary region" keep
/// predicate. A contour where every point survives is still a genuine
/// closed loop — emitted via [`Toolpath::emit_closed_contour_with_intent`]
/// exactly as before. A contour with only a partial survivor set is an open
/// run: emitted via [`Toolpath::emit_path_segment_with_intent`] instead, so
/// the excluded stretch becomes a retract/replunge gap rather than a
/// straight-line close across material outside the boundary. `None`
/// reproduces today's unconditional closed-contour emission byte-for-byte.
#[allow(clippy::too_many_arguments)]
pub fn waterline_toolpath_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    start_z: f64,
    final_z: f64,
    z_step: f64,
    params: &WaterlineParams,
    boundary_regions: Option<&RegionSet<'_>>,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    // R4: the raw ladder, byte-identical to the pre-2026-09-18 loop. The
    // `UnifiedFinish` very-steep band and the benches ladder through here;
    // the standalone adapter builds its ladder with `waterline_ladder` and
    // calls `waterline_toolpath_at_levels` directly.
    let levels = waterline_z_levels(start_z, final_z, z_step);
    waterline_toolpath_at_levels(
        mesh,
        index,
        cutter,
        &levels,
        params,
        boundary_regions,
        cancel,
    )
}

/// [`waterline_toolpath_with_cancel`] over an EXPLICIT ladder.
///
/// The caller owns the levels, so the ladder it cuts is the ladder it hands
/// to the span builder — one `Vec`, two readers. `levels` is walked in the
/// order given; [`waterline_ladder`] hands them top first.
pub fn waterline_toolpath_at_levels(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    levels: &[f64],
    params: &WaterlineParams,
    boundary_regions: Option<&RegionSet<'_>>,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    let mut toolpath = Toolpath::new();
    let filter = RunFilter {
        boundary_regions,
        keep: None,
        split_timing: None,
    };
    for &z in levels {
        check_cancel(cancel)?;
        let level = waterline_level(mesh, index, cutter, z, params, &filter, cancel)?;
        toolpath.moves.extend(level.toolpath.moves);
    }
    Ok(toolpath)
}

/// Which points of a contour one waterline level keeps.
///
/// With no rule, each contour is a closed loop. With a rule, the shared
/// run-splitter cuts each contour into the runs of kept points: a whole
/// surviving loop stays closed, and a part is an open run, so the dropped
/// stretch becomes a retract and a replunge, not a straight close across
/// it.
#[derive(Clone, Copy, Default)]
pub struct RunFilter<'f> {
    /// P2.3: a point is kept only inside a boundary region. A hard rule:
    /// a stretch outside the boundary is always dropped.
    pub boundary_regions: Option<&'f RegionSet<'f>>,
    /// W6.1 (`RestUse::ContourFilter`): a point is kept only where the rule
    /// says so (the rest stock at the level).
    pub keep: Option<&'f dyn Fn(P2) -> bool>,
    /// W6.1: the machine envelope that times a stretch `keep` drops. A
    /// dropped stretch between two kept points stays dropped only when the
    /// feed through it takes longer than a retract and a re-entry
    /// ([`stretch_is_worth_a_retract`]). `None`: no time model, so `keep`
    /// drops nothing (cut more, never less).
    pub split_timing: Option<&'f LinkKinematics>,
}

impl RunFilter<'_> {
    /// True when no rule drops a point.
    fn keeps_all(&self) -> bool {
        self.boundary_regions.is_none() && self.keep.is_none()
    }

    /// True when the boundary rule keeps `p`.
    fn inside(&self, p: &P3) -> bool {
        self.boundary_regions
            .is_none_or(|r| r.contains(&P2::new(p.x, p.y)))
    }
}

/// One waterline level, as [`waterline_level`] gives it.
pub struct WaterlineLevel {
    /// The cut of the level.
    pub toolpath: Toolpath,
    /// True when [`RunFilter::keep`] kept a point inside the boundary,
    /// after the time rule.
    pub rest_kept: bool,
    /// True when [`RunFilter::keep`] dropped a point inside the boundary,
    /// after the time rule.
    pub rest_dropped: bool,
}

/// W6.1: true when a retract over the stretch `from → path → to` saves
/// time. Both sides come from the one cycle-time model
/// (`machine::kinematics`), as the finishing link stage costs a link:
///
/// - the feed through the stretch: [`surface_link_time`] at the op's feed;
/// - leaving and entering again: [`retract_link_time`], a rapid up to
///   `safe_z`, a rapid across and a plunge at the op's plunge feed, the
///   motion the emitter makes at a run end and a run start.
///
/// A tie keeps the feed: a split that saves no time leaves the cut whole.
pub fn stretch_is_worth_a_retract(
    from: P3,
    path: &[P3],
    params: &WaterlineParams,
    timing: &LinkKinematics,
) -> bool {
    let Some(&to) = path.last() else {
        return false;
    };
    let feed_s = surface_link_time(
        from,
        path,
        params.feed_rate,
        &timing.kinematics,
        timing.max_feed_mm_min,
        timing.rapid_feed_mm_min,
    );
    let retract_s = retract_link_time(
        from,
        to,
        params.safe_z,
        None,
        params.plunge_rate,
        &timing.kinematics,
        timing.max_feed_mm_min,
        timing.rapid_feed_mm_min,
    );
    retract_s < feed_s
}

/// The rest mask of one closed contour: `keep[i]` for each point. A
/// stretch that only the rest rule dropped, between two kept points, comes
/// back as kept when the feed through it is not slower than a retract
/// ([`stretch_is_worth_a_retract`]). A stretch next to a point outside the
/// boundary stays dropped: the boundary ends the run there in any case.
fn bridge_air_stretches(
    contour: &[P3],
    inside: &[bool],
    keep: &mut [bool],
    params: &WaterlineParams,
    timing: Option<&LinkKinematics>,
) {
    let n = contour.len();
    let Some(first_kept) = keep.iter().position(|&k| k) else {
        return;
    };
    let mut k = 1;
    while k < n {
        let i = (first_kept + k) % n;
        if keep.get(i).copied().unwrap_or(true) {
            k += 1;
            continue;
        }
        // A dropped stretch starts at `i`; `from` is the kept point before
        // it. Walk to its end.
        let start = k;
        while k < n && !keep.get((first_kept + k) % n).copied().unwrap_or(true) {
            k += 1;
        }
        let stretch: Vec<usize> = (start..k).map(|j| (first_kept + j) % n).collect();
        let to = (first_kept + k) % n;
        let only_rest = stretch
            .iter()
            .all(|&j| inside.get(j).copied().unwrap_or(false));
        let bounded = inside.get(to).copied().unwrap_or(false);
        let from_i = (first_kept + start - 1) % n;
        let (Some(&from), Some(&end)) = (contour.get(from_i), contour.get(to)) else {
            continue;
        };
        let feed_through = only_rest
            && bounded
            && timing.is_none_or(|t| {
                let mut path: Vec<P3> = stretch
                    .iter()
                    .filter_map(|&j| contour.get(j).copied())
                    .collect();
                path.push(end);
                !stretch_is_worth_a_retract(from, &path, params, t)
            });
        if feed_through {
            for j in stretch {
                if let Some(m) = keep.get_mut(j) {
                    *m = true;
                }
            }
        }
    }
}

/// One waterline level: the contours at `z`, the leave lift, the run split
/// of `filter` and the emit. Every waterline ladder (the standalone op on
/// the Layered runner, and [`waterline_toolpath_at_levels`]) cuts its
/// levels here.
///
/// # Errors
///
/// [`Cancelled`] when `cancel` fires inside the push cutter.
pub fn waterline_level(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    z: f64,
    params: &WaterlineParams,
    filter: &RunFilter<'_>,
    cancel: &dyn CancelCheck,
) -> Result<WaterlineLevel, Cancelled> {
    let mut toolpath = Toolpath::new();
    let (mut rest_kept, mut rest_dropped) = (false, false);
    let mut contours =
        waterline_contours_with_cancel(mesh, index, cutter, z, params.sampling, cancel)?;

    // D-16.2: the whole contour rides up by `stock_to_leave`. Applied to
    // the CL points BEFORE the boundary run-splitter and the
    // minimum-segment floor, so neither the run topology nor the emitted
    // point count can change with the dial — only Z. Skipped entirely at
    // `0.0`, so the pre-fix path is byte-identical there.
    if params.stock_to_leave != 0.0 {
        for contour in &mut contours {
            for p in contour.iter_mut() {
                p.z += params.stock_to_leave;
            }
        }
    }

    for contour in &contours {
        if contour.len() < 3 {
            continue;
        }
        if filter.keeps_all() {
            let path = floor_contour(contour, true);
            if path.len() < 3 {
                continue;
            }
            toolpath.emit_closed_contour_with_intent(
                &path,
                params.safe_z,
                params.feed_rate,
                params.plunge_rate,
                MoveIntent::FinishingCut,
            );
            continue;
        }
        let inside: Vec<bool> = contour.iter().map(|p| filter.inside(p)).collect();
        let mut keep: Vec<bool> = contour
            .iter()
            .zip(&inside)
            .map(|(p, &inside)| inside && filter.keep.is_none_or(|k| k(P2::new(p.x, p.y))))
            .collect();
        if filter.keep.is_some() {
            bridge_air_stretches(contour, &inside, &mut keep, params, filter.split_timing);
            for (&inside, &kept) in inside.iter().zip(&keep) {
                if inside {
                    rest_kept |= kept;
                    rest_dropped |= !kept;
                }
            }
        }
        let runs = crate::geometry::point_runs::split_runs(
            contour,
            |i, _: &P3| keep.get(i).copied().unwrap_or(false),
            crate::geometry::point_runs::RunTopology::Closed,
            2,
        );
        for run in runs {
            if run.len() < 2 {
                continue;
            }
            // Decided on the RAW run: whether this is a whole surviving
            // loop is a question about the filter, not about the
            // degeneracy floor.
            let whole_loop = run.len() == contour.len();
            let path = floor_contour(&run, whole_loop);
            if path.len() < 2 {
                continue;
            }
            if whole_loop {
                // The whole loop survived — safe to close it.
                toolpath.emit_closed_contour_with_intent(
                    &path,
                    params.safe_z,
                    params.feed_rate,
                    params.plunge_rate,
                    MoveIntent::FinishingCut,
                );
            } else {
                // Partial survivor: an open run, not a closed loop —
                // closing it would chord straight across the dropped
                // stretch.
                toolpath.emit_path_segment_with_intent(
                    &path,
                    params.safe_z,
                    params.feed_rate,
                    params.plunge_rate,
                    MoveIntent::FinishingCut,
                );
            }
        }
    }
    Ok(WaterlineLevel {
        toolpath,
        rest_kept,
        rest_dropped,
    })
}

pub fn waterline_contours_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    z: f64,
    sampling: f64,
    cancel: &dyn CancelCheck,
) -> Result<Vec<Vec<P3>>, Cancelled> {
    let bbox = &mesh.bbox;
    let r = cutter.radius();

    // The grid covers `bbox ± r`, padded by one sampling cell on every side
    // (R9, Corne case 2026-09-18). The cutter-location offset of a vertical
    // wall on a bbox face sits exactly at `bbox ± r`. The weave needs one
    // free node outside a blocked node to place a contour crossing, so
    // without the pad the outer loop of such a wall has no crossing on its
    // straight sides and comes back as open corner arcs. The old edge
    // sampler hid this: its bisection left the blocked interval a hair
    // short of the fiber end, so the end node read free. With exact
    // interval ends the pad is the mechanism. One cell exactly: the span
    // grows by `2 · sampling`, so every interior node keeps its coordinate.
    let x_min = bbox.min.x - r - sampling;
    let x_max = bbox.max.x + r + sampling;
    let y_min = bbox.min.y - r - sampling;
    let y_max = bbox.max.y + r + sampling;

    let ny = ((y_max - y_min) / sampling).ceil() as usize + 1;
    let nx = ((x_max - x_min) / sampling).ceil() as usize + 1;
    // The X fibers first, then the Y fibers, in one batch. A fiber is
    // pushed on its own, so one batch gives each fiber the interval two
    // batches give it. W6.1: the batch polls `cancel` between its chunks,
    // so a cancel stops a level inside the push cutter. The two batches of
    // the uncancellable `rayon::join` could not: `&dyn CancelCheck` is not
    // `Sync`, so that join polled only after both batches ended. One batch
    // keeps the parallelism; it runs each chunk of fibers on the pool.
    let mut fibers: Vec<Fiber> = Vec::with_capacity(ny + nx);
    fibers.extend((0..ny).map(|i| {
        let y = y_min + i as f64 * sampling;
        Fiber::new_x(y, z, x_min, x_max)
    }));
    fibers.extend((0..nx).map(|i| {
        let x = x_min + i as f64 * sampling;
        Fiber::new_y(x, z, y_min, y_max)
    }));
    batch_push_cutter_with_cancel(&mut fibers, mesh, index, cutter, cancel)?;
    // The batch polls before each chunk; this poll covers the last one.
    check_cancel(cancel)?;
    let (x_fibers, y_fibers) = fibers.split_at(ny);
    Ok(weave_contours(x_fibers, y_fibers, z))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::mesh::{SpatialIndex, make_test_hemisphere};
    use crate::tool::BallEndmill;
    use crate::toolpath::MoveType;

    #[test]
    fn z_levels_matches_generator_ladder() {
        // R2.8: the exposed ladder must match the generator's own
        // start-down-to-final stepping exactly (same 1e-10 tail epsilon).
        let levels = waterline_z_levels(10.0, 0.0, 2.5);
        assert_eq!(levels, vec![10.0, 7.5, 5.0, 2.5, 0.0]);
    }

    #[test]
    fn z_levels_empty_when_start_below_final() {
        assert!(waterline_z_levels(-1.0, 0.0, 1.0).is_empty());
    }

    #[test]
    fn test_waterline_hemisphere_midheight() {
        let mesh = make_test_hemisphere(20.0, 32);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 25.0);

        let contours = waterline_contours(&mesh, &index, &tool, 10.0, 2.0);
        // At z=10 (midway), should find at least one contour
        assert!(
            !contours.is_empty(),
            "Should find contours at z=10 on hemisphere"
        );
    }

    #[test]
    fn test_waterline_above_mesh() {
        let mesh = make_test_hemisphere(20.0, 16);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 25.0);

        let contours = waterline_contours(&mesh, &index, &tool, 25.0, 2.0);
        assert!(contours.is_empty(), "No contours above mesh");
    }

    #[test]
    fn test_waterline_well_below_mesh() {
        let mesh = make_test_hemisphere(20.0, 16);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 25.0);

        // Well below the mesh — z=-30. G-FLUTETOP: the cutter has no top (the
        // shank and the holder stand above the 25 mm flutes), so the
        // hemisphere still blocks the tool under it. One loop: the base rim
        // (radius 20) offset by the ball radius 3. It used to read free.
        let contours = waterline_contours(&mesh, &index, &tool, -30.0, 2.0);
        assert_eq!(contours.len(), 1, "one loop round the footprint");
        let mean_r = contours[0]
            .iter()
            .map(|p| (p.x * p.x + p.y * p.y).sqrt())
            .sum::<f64>()
            / contours[0].len() as f64;
        assert!(
            (mean_r - 23.0).abs() < 1.0,
            "loop mean radius {mean_r}, expected about 23"
        );
    }

    #[test]
    fn test_waterline_toolpath_multiple_z() {
        let mesh = make_test_hemisphere(20.0, 32);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 25.0);

        let params = WaterlineParams {
            sampling: 2.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 25.0,
            stock_to_leave: 0.0,
        };

        let tp = waterline_toolpath(&mesh, &index, &tool, 15.0, 5.0, 5.0, &params);
        // Should have multiple Z levels: 15, 10, 5
        assert!(!tp.moves.is_empty(), "Waterline toolpath should have moves");

        // Should have rapids (retracts between contours)
        let rapids = tp
            .moves
            .iter()
            .filter(|m| m.move_type == MoveType::Rapid)
            .count();
        assert!(rapids >= 2, "Should have retracts between Z levels");
    }

    #[test]
    fn test_waterline_contour_is_roughly_circular() {
        let mesh = make_test_hemisphere(20.0, 32);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 25.0);

        let contours = waterline_contours(&mesh, &index, &tool, 10.0, 1.5);
        if contours.is_empty() {
            return; // Skip if no contours found (sampling too coarse)
        }

        let contour = &contours[0];
        // Hemisphere at z=10: radius = sqrt(20^2 - 10^2) = sqrt(300) ≈ 17.3
        // With ball cutter r=3, CL radius ≈ 17.3 + 3 = 20.3 (outer) or 17.3 - 3 = 14.3 (inner)
        // Points should be roughly equidistant from center
        let cx: f64 = contour.iter().map(|p| p.x).sum::<f64>() / contour.len() as f64;
        let cy: f64 = contour.iter().map(|p| p.y).sum::<f64>() / contour.len() as f64;

        // Check that points are approximately on a circle
        let radii: Vec<f64> = contour
            .iter()
            .map(|p| {
                let dx = p.x - cx;
                let dy = p.y - cy;
                (dx * dx + dy * dy).sqrt()
            })
            .collect();
        let mean_r = radii.iter().sum::<f64>() / radii.len() as f64;

        // All radii should be within 50% of mean (rough check for circular shape)
        for &r in &radii {
            assert!(
                r > mean_r * 0.5 && r < mean_r * 1.5,
                "Point radius {} far from mean {}, contour may not be circular",
                r,
                mean_r
            );
        }
    }

    // ── P2.3: boundary_regions pre-clip ──────────────────────────────

    #[test]
    fn waterline_boundary_regions_none_matches_call_without_param() {
        let mesh = make_test_hemisphere(20.0, 32);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 25.0);
        let params = WaterlineParams {
            sampling: 2.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 25.0,
            stock_to_leave: 0.0,
        };
        let never_cancel = || false;

        let tp_default = waterline_toolpath(&mesh, &index, &tool, 15.0, 5.0, 5.0, &params);
        let tp_none = waterline_toolpath_with_cancel(
            &mesh,
            &index,
            &tool,
            15.0,
            5.0,
            5.0,
            &params,
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
    fn waterline_boundary_regions_confines_cuts_to_region() {
        let mesh = make_test_hemisphere(20.0, 32);
        let index = SpatialIndex::build(&mesh, 10.0);
        let tool = BallEndmill::new(6.0, 25.0);
        let params = WaterlineParams {
            sampling: 2.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 25.0,
            stock_to_leave: 0.0,
        };
        let never_cancel = || false;

        // Left half of the hemisphere's XY footprint.
        let bbox = &mesh.bbox;
        let left_half = crate::polygon::Polygon2::new(vec![
            crate::geo::P2::new(bbox.min.x, bbox.min.y),
            crate::geo::P2::new(0.0, bbox.min.y),
            crate::geo::P2::new(0.0, bbox.max.y),
            crate::geo::P2::new(bbox.min.x, bbox.max.y),
        ]);

        let left_half_regions = std::slice::from_ref(&left_half);
        let region_set = RegionSet::from_slice(left_half_regions);
        let tp = waterline_toolpath_with_cancel(
            &mesh,
            &index,
            &tool,
            15.0,
            5.0,
            5.0,
            &params,
            Some(&region_set),
            &never_cancel,
        )
        .unwrap();

        let tol = 1e-6;
        let mut saw_cut = false;
        for m in &tp.moves {
            if let MoveType::Linear { .. } = m.move_type {
                saw_cut = true;
                assert!(
                    m.target.x <= tol,
                    "feed move X={:.3} escaped the left-half boundary region",
                    m.target.x
                );
            }
        }
        assert!(saw_cut, "expected at least one feed move");
    }
}
