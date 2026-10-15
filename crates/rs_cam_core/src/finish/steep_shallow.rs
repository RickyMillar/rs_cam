//! Steep and Shallow finishing strategy.
//!
//! Automatically splits the surface into steep and shallow regions based on a
//! threshold angle, then applies waterline (contour) passes to steep areas and
//! parallel (raster) passes to shallow areas.
//!
//! From Fusion 360 docs: "machines steep areas using Contour passes and shallow
//! areas using Parallel or Scallop passes."
//!
//! Key features:
//! - Threshold angle classification (default 40° from horizontal)
//! - Overlap distance: both strategies extend into each other's regions
//! - Wall clearance: shallow passes stay clear of steep walls
//! - Steep-first ordering for safer tool conditions
//! - Scallop height support for variable stepover in shallow regions

use std::ops::Range;

use crate::compute::spans::RegionNode;
use crate::finish::composed::{ComposedRest, ComposedSurfaceOp, Routed, compose};
use crate::finish::finish_setup::FinishResolutionPolicy;
use crate::finish::surface_link::{FinishingLinkStage, LinkGeometry, relink_fragments};
use crate::finish::unified_finish::RelinkTotals;
use crate::geo::{P2, P3};
use crate::geometry::region_set::RegionSet;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::ops::waterline::{AirBridge, bridge_air_stretches, waterline_contours};
use crate::surface::dropcutter::batch_drop_cutter;
use crate::surface::slope::{SlopeMap, classify_steep_shallow};
use crate::tool::MillingCutter;
use crate::toolpath::Toolpath;
use crate::trace::toolpath_spans::AnnotatedToolpath;
use crate::trace::transform_provenance::ReconcileSet;

use tracing::{debug, info};

/// Parameters for steep-and-shallow finishing.
pub struct SteepShallowParams {
    /// Threshold angle (degrees from horizontal). Areas steeper than this get
    /// waterline passes; shallower areas get parallel passes. Default: 40.
    pub threshold_angle: f64,
    /// Overlap distance: both strategies extend into each other's region (mm).
    /// Eliminates visible transition marks. Default: 2× stepover.
    pub overlap_distance: f64,
    /// Wall clearance: shallow passes stay this far from steep walls (mm).
    /// Prevents tool rubbing on steep walls during parallel passes.
    pub wall_clearance: f64,
    /// Machine steep regions first for safer tool conditions.
    pub steep_first: bool,
    /// Stepover for parallel passes in shallow regions (mm).
    pub stepover: f64,
    /// Z step for waterline passes in steep regions (mm).
    pub z_step: f64,
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

impl Default for SteepShallowParams {
    fn default() -> Self {
        Self {
            threshold_angle: 45.0,
            overlap_distance: 2.0,
            wall_clearance: 1.0,
            steep_first: true,
            stepover: 1.0,
            z_step: 1.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
            sampling: 1.0,
            stock_to_leave: 0.0,
            tolerance: 0.05,
        }
    }
}

/// Dilate a boolean grid by `radius_cells`, setting additional cells to `true`.
/// 8-connected expansion.
#[allow(clippy::indexing_slicing)] // bounded indexing in grid morphology
pub fn dilate_grid(grid: &[bool], rows: usize, cols: usize, radius_cells: usize) -> Vec<bool> {
    if radius_cells == 0 {
        return grid.to_vec();
    }
    let mut result = grid.to_vec();
    for _ in 0..radius_cells {
        let prev = result.clone();
        for row in 0..rows {
            for col in 0..cols {
                if prev[row * cols + col] {
                    continue; // Already true
                }
                // Check 8-connected neighbors
                let has_neighbor = (-1i32..=1).any(|dr| {
                    (-1i32..=1).any(|dc| {
                        if dr == 0 && dc == 0 {
                            return false;
                        }
                        let nr = row as i32 + dr;
                        let nc = col as i32 + dc;
                        if nr < 0 || nr >= rows as i32 || nc < 0 || nc >= cols as i32 {
                            return false;
                        }
                        prev[nr as usize * cols + nc as usize]
                    })
                });
                if has_neighbor {
                    result[row * cols + col] = true;
                }
            }
        }
    }
    result
}

/// Erode a boolean grid by `radius_cells`, clearing cells near the boundary.
fn erode_grid(grid: &[bool], rows: usize, cols: usize, radius_cells: usize) -> Vec<bool> {
    if radius_cells == 0 {
        return grid.to_vec();
    }
    // Erode = dilate the inverse, then invert back
    let inv: Vec<bool> = grid.iter().map(|&v| !v).collect();
    let dilated_inv = dilate_grid(&inv, rows, cols, radius_cells);
    dilated_inv.iter().map(|&v| !v).collect()
}

/// The steep sub-kernel: waterline contours filtered to the steep+overlap
/// cells. Polls `cancel` once per Z level (the outer waterline-contour
/// loop). Gives the toolpath and the index of the first move of each Z
/// level (W6.4.6: the link stage links inside one level only).
///
/// W6.4.4: this is the op's OWN emitter, not the waterline kernel of
/// Unified Finish: the 30° contour-majority filter, the per-point keep on
/// the steep GRID, the closed-run split, the PR-8d minimum-segment floor and
/// `snap_to_bottom: false` are this op's output.
///
/// `boundary_regions` (P2.3): folded into the existing steep-grid keep
/// predicate below — a contour point survives only when it's both in the
/// expanded steep grid AND inside a machining-boundary region (when one is
/// given). `None` reproduces today's steep-grid-only filtering byte-for-byte.
///
/// `rest` (W6.4.6): a kept point must also pass material at the level
/// ([`ComposedRest`]). `None` (a fresh pass) adds no rule.
#[allow(clippy::indexing_slicing, clippy::too_many_arguments)]
fn emit_steep_passes(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    slope_map: &SlopeMap,
    steep_expanded: &[bool],
    z_top: f64,
    z_bottom: f64,
    z_step: f64,
    sampling: f64,
    stock_to_leave: f64,
    feed_rate: f64,
    plunge_rate: f64,
    safe_z: f64,
    boundary_regions: Option<&RegionSet<'_>>,
    rest: Option<&ComposedRest<'_>>,
    cancel: &dyn CancelCheck,
) -> Result<(Toolpath, Vec<usize>), Cancelled> {
    let mut tp = Toolpath::new();
    let mut level_starts = Vec::new();

    let steep_threshold = 30.0_f64.to_radians(); // Filter out contours that are mostly shallow

    // `snap_to_bottom = false`: matches the prior inline loop exactly — the
    // ladder is not guaranteed to land exactly on `z_bottom` when the range
    // isn't a whole multiple of `z_step` (see ops::depth::z_ladder docs).
    let z_levels = crate::ops::depth::z_ladder(
        z_top,
        z_bottom,
        z_step,
        crate::ops::depth::LadderMode::ConstantStep {
            epsilon: crate::ops::depth::Z_LADDER_DEFAULT_EPSILON,
            snap_to_bottom: false,
        },
    );
    for z in z_levels {
        check_cancel(cancel)?;
        level_starts.push(tp.moves.len());
        let contours = waterline_contours(mesh, index, cutter, z, sampling);

        for contour in &contours {
            if contour.len() < 3 {
                continue;
            }

            // Filter: keep contours that are predominantly in the steep+overlap region.
            // Sample points and check the expanded steep grid.
            let sample_step = 1.max(contour.len() / 10);
            let in_steep = contour
                .iter()
                .step_by(sample_step)
                .filter(|p| {
                    slope_map
                        .angle_at_world(p.x, p.y)
                        .is_some_and(|a| a >= steep_threshold)
                })
                .count();
            let total = contour.len().div_ceil(sample_step);
            if total > 0 && in_steep * 3 < total {
                continue; // Mostly shallow, skip
            }

            // Further filter: mark which points fall within the expanded
            // steep grid AND (when given) a machining-boundary region,
            // preserving contour ordering so we can tell which survivors
            // are contiguous.
            let mut keep: Vec<bool> = contour
                .iter()
                .map(|p| {
                    let in_steep_grid = slope_map
                        .world_to_cell(p.x, p.y)
                        .is_some_and(|(row, col)| steep_expanded[row * slope_map.cols + col]);
                    let in_region =
                        boundary_regions.is_none_or(|regions| regions.contains(&P2::new(p.x, p.y)));
                    in_steep_grid && in_region
                })
                .collect();
            // W6.4.6: the rest rule, then the air-stretch time rule.
            if let Some(rest) = rest {
                let inside = keep.clone();
                for (kept, p) in keep.iter_mut().zip(contour) {
                    *kept = *kept && (rest.standing)(P3::new(p.x, p.y, z));
                }
                bridge_air_stretches(
                    contour,
                    &inside,
                    &mut keep,
                    true,
                    &AirBridge {
                        feed_rate,
                        plunge_rate,
                        safe_z,
                        timing: rest.split_timing,
                    },
                );
            }

            let kept_count = keep.iter().filter(|&&k| k).count();
            if kept_count < 3 {
                continue;
            }

            use crate::toolpath::MoveIntent;
            let z_adjusted = z + stock_to_leave;
            // Only a contour where every point survives is a genuinely
            // closed loop — safe to close back to its start. Partial
            // survivors are non-contiguous with the excluded shallow
            // region: chording straight across that gap at cutting feed
            // would gouge material that sits above this Z level. Split
            // into contiguous runs and emit each as its own open pass with
            // its own rapid/plunge/retract envelope.
            let whole_contour_kept = kept_count == contour.len();
            let runs = crate::geometry::point_runs::split_runs(
                contour,
                |i, _p| keep.get(i).copied().unwrap_or(false),
                crate::geometry::point_runs::RunTopology::Closed,
                2,
            );

            for run in &runs {
                if run.len() < 2 {
                    continue;
                }
                // ── Minimum-segment floor (PR-8d) ──────────────────────────
                //
                // `CHECKPOINT_B_EVIDENCE.md` §8.1: this op emits 0.9 µm
                // cutting segments on the mixed-slope ribbon at EVERY
                // resolution. Measured here: `min seg 0.000891 mm`, exactly
                // two of them, both in THIS (steep/waterline) half, bit
                // identical across all four A/B arms — so the cell is not
                // the cause and a finer grid is not the cure.
                //
                // SOURCE: `contour_extract::weave_contours`. Marching squares
                // places each cell-edge vertex at an EXACT fiber interval
                // boundary (`find_interval_boundary_x`/`_y` return the
                // interval endpoint that falls inside the edge span, not the
                // edge midpoint), so two adjacent cells whose boundaries
                // resolve to the same crossing produce two chained vertices
                // that are coincident to floating-point noise.
                //
                // WHY IT IS FIXED HERE and not there: of the three consumers
                // of `weave_contours`, `ramp_finish` already RDP-simplifies
                // its contour points, and this op and `waterline.rs` both
                // emit them RAW — this op carries a `tolerance` field
                // documented as "Path tolerance for simplification" and has
                // never used it for anything but the resolution policy. The
                // missing filter/merge decision is at the emission site, and
                // that is the decision being added. `waterline.rs` had the
                // same defect from the same source and was deliberately left
                // alone to keep PR-8d's blast radius one operation wide;
                // C3 (2026-08-02) closed it — `waterline::floor_contour`
                // applies the same floor at waterline's own emission sites.
                //
                // Note which way round the source actually runs: the two
                // 0.000891 mm segments Checkpoint B §8.1 observed in the
                // STEEP half are manufactured by `waterline_contours`, which
                // the steep half calls. This filter removes them from the
                // steep/shallow output; C3 stops them being emitted by
                // direct waterline ops too.
                //
                // `closed` matches the emitter chosen below: a whole-contour
                // survivor chords back to its start, so a trailing vertex
                // within the floor of the first would make THAT move the
                // degenerate one.
                let path: Vec<P3> = crate::toolpath::drop_sub_minimum_segments(
                    &run.iter()
                        .map(|pt| P3::new(pt.x, pt.y, z_adjusted))
                        .collect::<Vec<P3>>(),
                    crate::toolpath::MIN_EMITTED_SEGMENT_MM,
                    whole_contour_kept,
                );
                if path.len() < 2 {
                    continue;
                }

                // Whole-contour survivors are a genuine closed loop — use
                // the shared closed-contour emitter (rapid/plunge/feed/
                // close/retract) instead of hand-closing the point list.
                // Partial survivors are an open run: emit as a plain path
                // segment with its own rapid/plunge/retract envelope.
                if whole_contour_kept {
                    tp.emit_closed_contour_with_intent(
                        &path,
                        safe_z,
                        feed_rate,
                        plunge_rate,
                        MoveIntent::FinishingCut,
                    );
                } else {
                    tp.emit_path_segment_with_intent(
                        &path,
                        safe_z,
                        feed_rate,
                        plunge_rate,
                        MoveIntent::FinishingCut,
                    );
                }
            }
        }
    }

    Ok((tp, level_starts))
}

/// The shallow sub-kernel: a zigzag raster filtered to the shallow+overlap
/// cells. Polls `cancel` once per raster row.
///
/// W6.4.4: the op's OWN emitter (its own drop-cutter grid and hand-walked
/// zigzag, no chord refinement), not the raster kernel of Unified Finish.
///
/// `boundary_regions` (P2.3): folded into the existing `is_shallow`
/// per-point keep check below (the drop-cutter batch already ran before
/// this loop, so there's no query left to pre-skip — the region check is
/// still cheap since it only gates which precomputed grid points get
/// emitted). `None` reproduces today's shallow-grid-only filtering
/// byte-for-byte.
///
/// `rest` (W6.4.6): a kept point must also pass material at its drop
/// ([`ComposedRest`]). `None` (a fresh pass) adds no rule.
#[allow(clippy::too_many_arguments)]
fn emit_shallow_passes(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    shallow_eroded: &[bool],
    slope_map: &SlopeMap,
    stepover: f64,
    stock_to_leave: f64,
    feed_rate: f64,
    plunge_rate: f64,
    safe_z: f64,
    boundary_regions: Option<&RegionSet<'_>>,
    rest: Option<&ComposedRest<'_>>,
    cancel: &dyn CancelCheck,
) -> Result<Toolpath, Cancelled> {
    let mut tp = Toolpath::new();

    // Generate drop-cutter raster grid
    let grid = batch_drop_cutter(mesh, index, cutter, stepover, 0.0, mesh.bbox.min.z);
    check_cancel(cancel)?;

    // Walk rows in zigzag pattern, clipping to the shallow region
    for row in 0..grid.rows {
        check_cancel(cancel)?;
        let reverse = row % 2 == 1;

        let cols: Box<dyn Iterator<Item = usize>> = if reverse {
            Box::new((0..grid.cols).rev())
        } else {
            Box::new(0..grid.cols)
        };

        // The CL points of the row in walk order, and which of them lie in
        // the shallow region and (when given) a machining-boundary region.
        let mut points: Vec<P3> = Vec::with_capacity(grid.cols);
        let mut inside: Vec<bool> = Vec::with_capacity(grid.cols);
        for col in cols {
            let cl = grid.get(row, col);
            // Read world coordinates straight off the CL point rather than
            // reconstructing them from the grid origin/step — the grid's
            // `u_start`/`v_start` are rotated-frame minima that only read as
            // world X/Y when `direction_deg == 0.0` (which the call above
            // hardcodes, but `cl.x`/`cl.y` are always world-frame regardless
            // of sampling direction, so this removes that trap entirely).
            let (x, y) = (cl.x, cl.y);
            let is_shallow = slope_map
                .world_to_cell(x, y)
                .is_some_and(|(r, c)| shallow_eroded.get(r * slope_map.cols + c) == Some(&true))
                && boundary_regions.is_none_or(|regions| regions.contains(&P2::new(x, y)));
            points.push(P3::new(x, y, cl.z));
            inside.push(is_shallow);
        }

        // W6.4.6: the rest rule, then the air-stretch time rule.
        let mut keep = inside.clone();
        if let Some(rest) = rest {
            for (kept, p) in keep.iter_mut().zip(&points) {
                *kept = *kept && (rest.standing)(*p);
            }
            bridge_air_stretches(
                &points,
                &inside,
                &mut keep,
                false,
                &AirBridge {
                    feed_rate,
                    plunge_rate,
                    safe_z,
                    timing: rest.split_timing,
                },
            );
        }

        // Emit each run of kept points.
        let mut run: Vec<P3> = Vec::new();
        for (p, &kept) in points.iter().zip(&keep) {
            if kept {
                run.push(P3::new(p.x, p.y, p.z + stock_to_leave));
            } else {
                emit_shallow_run(&mut tp, &run, feed_rate, plunge_rate, safe_z);
                run.clear();
            }
        }
        emit_shallow_run(&mut tp, &run, feed_rate, plunge_rate, safe_z);
    }

    Ok(tp)
}

/// Emit one shallow raster run: a rapid over its start, a plunge, the cut,
/// and a rapid retract. A run of fewer than two points emits nothing.
fn emit_shallow_run(tp: &mut Toolpath, run: &[P3], feed_rate: f64, plunge_rate: f64, safe_z: f64) {
    use crate::toolpath::MoveIntent;
    let (Some(&first), Some(&last)) = (run.first(), run.last()) else {
        return;
    };
    if run.len() < 2 {
        return;
    }
    tp.rapid_to_with_intent(P3::new(first.x, first.y, safe_z), MoveIntent::Linking);
    tp.feed_to_with_intent(first, plunge_rate, MoveIntent::EntryPlunge);
    for pt in run.iter().skip(1) {
        tp.feed_to_with_intent(*pt, feed_rate, MoveIntent::FinishingCut);
    }
    tp.rapid_to_with_intent(P3::new(last.x, last.y, safe_z), MoveIntent::Retract);
}

/// The resolution policy steep/shallow generates on (H3 step 2).
///
/// SteepShallow selects `FinishResolutionMode::LegacyEnvelopeQuarter` — the
/// same `(envelope_radius/4).max(tolerance)` cell it has always used, now
/// stated at its own call site instead of inherited from a shared builder.
///
/// # Why this op STAYS on the legacy cell (PR-8c, written deferral)
///
/// H3's wave moved `ramp_finish` off this mode
/// (`ramp_finish::ramp_finish_generation_resolution`) and deliberately left
/// this one on it. Checkpoint B approved a **deferral with a written reason**
/// rather than a silent hold, and this is it.
///
/// `CHECKPOINT_B_EVIDENCE.md` §3.3 ran the same four-arm A/B over this op on
/// four fixtures and measured **zero quality delta**: every residual quantile
/// is bit-identical across `envelope/4`, the geo-mean cell, `cusp/4` and the
/// tolerance floor, on every fixture. Move counts move 1–5%. The emitted
/// output CONVERGES well above `cusp/4` — fingerprints are byte-identical
/// between the intermediate and finer arms on the narrow valley, and between
/// `cusp/4` and the tolerance arm on two more. Going to `cusp/4` costs
/// **2.9–7.6× generation time** for nothing measurable, which fails the
/// repository's performance policy outright.
///
/// **That is not a clean bill of health, and must not be read as one.** This
/// op CLASSIFIES steep vs shallow on this same grid (its own offset surface,
/// not the true-surface classification builder), and §2.2 measured that
/// classification **10.6–13.3% wrong at `envelope/4`** on these fixtures,
/// with the VerySteep class entirely ABSENT on the two narrow ones where the
/// truth has 8 and 4 disconnected regions. Labels that wrong with output that
/// unchanged is a dissociation, not an absolution. The two readings that fit:
/// these fixtures all carry contiguous steep territory far wider than a
/// 0.75 mm cell, so the label error lands on boundaries the op's
/// `overlap_distance` dilation and `wall_clearance` erosion then swamp; or
/// the op is genuinely insensitive. Nothing here distinguishes them.
///
/// **What would**: a fixture whose steep/shallow BOUNDARY has structure at
/// the 0.5–1.5 mm scale — interdigitated steep fingers narrower than the
/// overlap distance, so a mislabelled cell cannot be absorbed by dilation.
/// Until such a fixture exists and is run, moving this op would be changing
/// a default on the strength of an experiment that could not have detected
/// the change. `finish_setup::build_classification_surface_with_cancel`'s
/// doc records the measured blind spot this shares.
#[must_use]
pub fn steep_shallow_generation_resolution(
    cutter: &dyn MillingCutter,
    tolerance: f64,
) -> FinishResolutionPolicy {
    FinishResolutionPolicy::legacy_envelope_quarter(cutter, tolerance)
}

/// Where each half of the merged toolpath ended up.
///
/// The op is two independently generated passes concatenated — a Z-laddered
/// waterline over the steep territory and a raster over the shallow — not
/// one continuous trace. The adapter needs the split to place rapid-order
/// barriers (`compute::spans::region_node_barriers`): the halves must keep
/// their emitted order relative to each other, the steep half must keep its
/// Z ladder, and everything inside those constraints is reorderable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteepShallowSplit {
    pub steep: Range<usize>,
    pub shallow: Range<usize>,
}

/// Label on the steep half's `Region` span — the handle sentries use to
/// find the waterline pass again after a reorder remaps spans.
pub const STEEP_HALF_LABEL: &str = "Steep pass";
/// Label on the shallow half's `Region` span — the sentries' handle for
/// the raster pass, beside [`STEEP_HALF_LABEL`].
pub const SHALLOW_HALF_LABEL: &str = "Shallow pass";

/// Generate a steep-and-shallow finishing toolpath, and the
/// [`SteepShallowSplit`] describing where each half landed.
///
/// Splits the surface into steep and shallow regions based on slope angle,
/// then generates waterline passes for steep areas and parallel raster passes
/// for shallow areas, with configurable overlap and wall clearance. Polls
/// `cancel` in the heightmap construction and in both pass generators
/// (per-Z-level for steep, per-raster-row for shallow).
///
/// `boundary_regions` (P2.3): forwarded to both pass generators, which fold
/// it into their existing per-point keep checks (steep grid / shallow grid
/// respectively). `None` reproduces today's output byte-for-byte.
#[tracing::instrument(skip(mesh, index, cutter, params, boundary_regions, cancel), fields(threshold = params.threshold_angle))]
pub fn steep_shallow_toolpath_split_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &SteepShallowParams,
    boundary_regions: Option<&RegionSet<'_>>,
    cancel: &dyn CancelCheck,
) -> Result<(Toolpath, SteepShallowSplit), Cancelled> {
    steep_shallow_toolpath_split_with_resolution(
        mesh,
        index,
        cutter,
        params,
        boundary_regions,
        steep_shallow_generation_resolution(cutter, params.tolerance),
        cancel,
    )
}

/// [`steep_shallow_toolpath_split_with_cancel`] with the generation grid
/// resolution supplied by the caller.
///
/// **Research seam, not a production entry point** — see
/// [`crate::finish::scallop::scallop_toolpath_structured_annotated_with_resolution`]
/// for why H3's Checkpoint B harness needs one. This op is the one where the
/// cell also decides CLASSIFICATION (steep vs shallow is read off this same
/// grid), so it is the sharpest of the three arms. Passing
/// `steep_shallow_generation_resolution(cutter, params.tolerance)` reproduces
/// the shipped path exactly.
pub fn steep_shallow_toolpath_split_with_resolution(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &SteepShallowParams,
    boundary_regions: Option<&RegionSet<'_>>,
    resolution: FinishResolutionPolicy,
    cancel: &dyn CancelCheck,
) -> Result<(Toolpath, SteepShallowSplit), Cancelled> {
    let op = SteepShallowOp {
        mesh,
        index,
        cutter,
        params,
        boundary_regions,
        resolution,
        rest: None,
        link_stage: None,
    };
    let composed = compose(&op, cancel)?;
    Ok((composed.toolpath, composed.report.split))
}

// ── The composed op (W6.4.4) ─────────────────────────────────────────────

/// Steep and Shallow as a composed-surface op ([`crate::finish::composed`]):
/// the slope classifier on its own offset surface (`legacy_envelope_quarter`,
/// PR-8c), two cell-mask regions (steep, shallow) cut by the op's own two
/// emitters, and a fixed order (`steep_first`).
pub(crate) struct SteepShallowOp<'a> {
    pub mesh: &'a TriangleMesh,
    pub index: &'a SpatialIndex,
    pub cutter: &'a dyn MillingCutter,
    pub params: &'a SteepShallowParams,
    /// The territory regions (P2.3). Each emitter folds them into its
    /// per-point keep.
    pub boundary_regions: Option<&'a RegionSet<'a>>,
    /// The generation and classification grid
    /// ([`steep_shallow_generation_resolution`] in production).
    pub resolution: FinishResolutionPolicy,
    /// W6.4.6: the rest inputs. `Some` only on a rest pass (the territory
    /// accepted the remaining stock as the rest reference).
    pub rest: Option<ComposedRest<'a>>,
    /// W6.4.6: the shared finishing link stage, `Some` when `hookup_mm` is
    /// above zero. Its ceiling comes from `link_ceiling_for`.
    pub link_stage: Option<FinishingLinkStage<'a>>,
}

/// What the composed op reports: the split of the two halves and the link
/// stage totals.
pub(crate) struct SteepShallowReport {
    pub split: SteepShallowSplit,
    /// G-LINKVISIBLE: `Some` exactly when the link stage ran.
    pub relink: Option<RelinkTotals>,
}

/// The two regions of the op. Each one is a cell mask on the classifier
/// grid, not a polygon: a polygon form changes the contour run splits and
/// the raster run ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SteepShallowRegion {
    /// Waterline passes over the steep cells, dilated by the overlap.
    Steep,
    /// Raster passes over the shallow cells, eroded by the wall clearance
    /// and dilated by the overlap.
    Shallow,
}

/// What the steep/shallow classifier gives the two emitters.
pub(crate) struct SteepShallowClassified {
    surface: std::sync::Arc<crate::finish::finish_setup::FinishSurface>,
    steep_expanded: Vec<bool>,
    shallow_expanded: Vec<bool>,
    z_top: f64,
    z_bottom: f64,
}

impl ComposedSurfaceOp for SteepShallowOp<'_> {
    type Classified = SteepShallowClassified;
    type Fragment = (SteepShallowRegion, Toolpath);
    type Memo = ();
    type Annotation = ();
    type Report = SteepShallowReport;

    fn classify(
        &self,
        cancel: &dyn CancelCheck,
    ) -> Result<(SteepShallowClassified, SteepShallowReport), Cancelled> {
        let (mesh, index, cutter, params, resolution) = (
            self.mesh,
            self.index,
            self.cutter,
            self.params,
            self.resolution,
        );
        let bbox = &mesh.bbox;

        // Build surface heightmap and slope map (shared setup, see finish_setup.rs).
        // The RESOLUTION is steep/shallow's own choice (H3 step 2) — see
        // `steep_shallow_generation_resolution`, which the shipped entry passes
        // in.
        //
        // MEMOISED — see `finish_surface_cache`'s module doc. Like `ramp_finish`,
        // this op builds one surface per call rather than per region; it shares the
        // entry so every generation consumer reads one memo.
        let surface = crate::maps::finish_surface_cache::cached_finish_surface(
            mesh, index, cutter, resolution, cancel,
        )?;
        let surface_hm = &surface.heightmap;
        let slope_map = &surface.slope_map;
        let cell_size = surface_hm.cell_size;
        let rows = surface_hm.rows;
        let cols = surface_hm.cols;

        // Classify steep vs shallow
        let steep_grid = classify_steep_shallow(slope_map, params.threshold_angle);

        let steep_count = steep_grid.iter().filter(|&&s| s).count();
        let shallow_count = steep_grid.iter().filter(|&&s| !s).count();
        info!(
            steep = steep_count,
            shallow = shallow_count,
            threshold = params.threshold_angle,
            "Steep/shallow classification"
        );

        // Expand steep region by overlap_distance for waterline pass extension
        let overlap_cells = (params.overlap_distance / cell_size).ceil() as usize;
        let steep_expanded = dilate_grid(&steep_grid, rows, cols, overlap_cells);

        // Create shallow region: invert steep, then erode by wall_clearance
        let shallow_grid: Vec<bool> = steep_grid.iter().map(|&s| !s).collect();
        let clearance_cells = (params.wall_clearance / cell_size).ceil() as usize;
        let shallow_eroded = erode_grid(&shallow_grid, rows, cols, clearance_cells);

        // Expand shallow region by overlap_distance for raster pass extension
        let shallow_expanded = dilate_grid(&shallow_eroded, rows, cols, overlap_cells);

        // Z range.
        //
        // C2 AUDIT (2026-07-30), the consumer PR-8b flagged as unaudited: the
        // ladder bottom WANTS the bbox floor, and `min_covered_z()` would be a
        // regression here. The steep half is a WATERLINE pass — it slices the
        // mesh with `waterline_contours` at each level, and a vertical wall
        // extends from the top face all the way down while the drop-cutter grid
        // only ever sees the top face. On a plateau (a box with vertical sides)
        // `min_covered_z()` IS the top face, so a covered-only bottom would
        // collapse the ladder to a single level and drop every wall pass — the
        // exact passes the operation exists to make. Levels below real material
        // are free: `waterline_contours` returns nothing there.
        // Sentry: `steep_ladder_bottom_is_the_bbox_floor_not_the_covered_minimum`.
        // (This differs from `ramp_finish`, whose ladder bottom is a DEPTH THE
        // CUTTER MUST HOLD at a sampled surface point — that one is clamped to
        // `min_covered_z()`, PR-8b.)
        let z_top = bbox.max.z;
        let z_bottom = surface_hm.min_z_or_bbox_floor();

        debug!(
            z_top = format!("{:.1}", z_top),
            z_bottom = format!("{:.1}", z_bottom),
            overlap_cells = overlap_cells,
            clearance_cells = clearance_cells,
            "Generating steep and shallow passes"
        );

        Ok((
            SteepShallowClassified {
                surface,
                steep_expanded,
                shallow_expanded,
                z_top,
                z_bottom,
            },
            SteepShallowReport {
                split: SteepShallowSplit {
                    steep: 0..0,
                    shallow: 0..0,
                },
                relink: self.link_stage.as_ref().map(|_| RelinkTotals::default()),
            },
        ))
    }

    /// Two regions, always: 0 is the steep half, 1 the shallow half. The
    /// emit order is fixed; `route` applies `steep_first`.
    fn region_count(&self, _classified: &SteepShallowClassified) -> usize {
        2
    }

    fn cut_region(
        &self,
        classified: &SteepShallowClassified,
        region: usize,
        _memo: &mut (),
        report: &mut SteepShallowReport,
        cancel: &dyn CancelCheck,
    ) -> Result<Option<(SteepShallowRegion, Toolpath)>, Cancelled> {
        let params = self.params;
        let slope_map = &classified.surface.slope_map;
        match region {
            // Generate steep (waterline) passes
            0 => {
                let (tp, level_starts) = emit_steep_passes(
                    self.mesh,
                    self.index,
                    self.cutter,
                    slope_map,
                    &classified.steep_expanded,
                    classified.z_top,
                    classified.z_bottom,
                    params.z_step,
                    params.sampling,
                    params.stock_to_leave,
                    params.feed_rate,
                    params.plunge_rate,
                    params.safe_z,
                    self.boundary_regions,
                    self.rest.as_ref(),
                    cancel,
                )?;
                Ok(Some((
                    SteepShallowRegion::Steep,
                    self.link_levels(tp, &level_starts, report),
                )))
            }
            // Generate shallow (parallel) passes
            1 => {
                let tp = emit_shallow_passes(
                    self.mesh,
                    self.index,
                    self.cutter,
                    &classified.shallow_expanded,
                    slope_map,
                    params.stepover,
                    params.stock_to_leave,
                    params.feed_rate,
                    params.plunge_rate,
                    params.safe_z,
                    self.boundary_regions,
                    self.rest.as_ref(),
                    cancel,
                )?;
                Ok(Some((SteepShallowRegion::Shallow, self.link(tp, report))))
            }
            _ => Ok(None),
        }
    }

    /// Concatenate the two halves; `steep_first` puts the steep half first.
    fn route(
        &self,
        _classified: SteepShallowClassified,
        fragments: Vec<(SteepShallowRegion, Toolpath)>,
        report: &mut SteepShallowReport,
        _cancel: &dyn CancelCheck,
    ) -> Result<Routed<()>, Cancelled> {
        let mut steep_tp = Toolpath::new();
        let mut shallow_tp = Toolpath::new();
        for (region, tp) in fragments {
            match region {
                SteepShallowRegion::Steep => steep_tp = tp,
                SteepShallowRegion::Shallow => shallow_tp = tp,
            }
        }
        info!(
            steep_moves = steep_tp.moves.len(),
            shallow_moves = shallow_tp.moves.len(),
            steep_cut_mm = format!("{:.0}", steep_tp.total_cutting_distance()),
            shallow_cut_mm = format!("{:.0}", shallow_tp.total_cutting_distance()),
            "Steep and shallow passes generated"
        );

        // Merge: steep_first means steep toolpath comes first
        let (n_steep, n_shallow) = (steep_tp.moves.len(), shallow_tp.moves.len());
        let mut tp = Toolpath::new();
        report.split = if self.params.steep_first {
            tp.moves.extend(steep_tp.moves);
            tp.moves.extend(shallow_tp.moves);
            SteepShallowSplit {
                steep: 0..n_steep,
                shallow: n_steep..n_steep + n_shallow,
            }
        } else {
            tp.moves.extend(shallow_tp.moves);
            tp.moves.extend(steep_tp.moves);
            SteepShallowSplit {
                shallow: 0..n_shallow,
                steep: n_shallow..n_shallow + n_steep,
            }
        };

        info!(
            moves = tp.moves.len(),
            cutting_mm = format!("{:.1}", tp.total_cutting_distance()),
            rapid_mm = format!("{:.1}", tp.total_rapid_distance()),
            "Steep and shallow toolpath complete"
        );
        Ok(Routed {
            toolpath: tp,
            annotations: Vec::new(),
        })
    }

    /// The two halves. The steep half is a waterline: it ladders down in
    /// Z, so its runs must keep their relative depth order. The shallow
    /// half is a raster over a height field: its runs are materially
    /// independent of visiting order.
    fn order_constraints(report: &SteepShallowReport) -> Vec<RegionNode> {
        let split = &report.split;
        vec![
            RegionNode {
                move_range: split.steep.clone(),
                depth_ordered: true,
            },
            RegionNode {
                move_range: split.shallow.clone(),
                depth_ordered: false,
            },
        ]
    }

    /// The cutting-run spans and one labelled `Region` span per half.
    fn label_spans(
        toolpath: &Toolpath,
        _annotations: &[()],
        report: &SteepShallowReport,
    ) -> Vec<crate::trace::toolpath_spans::Span> {
        use crate::compute::spans::spans_from_cutting_runs;
        let split = &report.split;
        use crate::trace::toolpath_spans::{Span, SpanKind};

        let mut spans = spans_from_cutting_runs(toolpath, "Steep/shallow run");
        let insert_at = 1.min(spans.len());
        let mut halves: Vec<Span> = Vec::new();
        if !split.steep.is_empty() {
            halves.push(
                Span::new(split.steep.start, split.steep.end, SpanKind::Region)
                    .with_label(STEEP_HALF_LABEL),
            );
        }
        if !split.shallow.is_empty() {
            halves.push(
                Span::new(split.shallow.start, split.shallow.end, SpanKind::Region)
                    .with_label(SHALLOW_HALF_LABEL),
            );
        }
        halves.sort_by_key(|s| s.start_move);
        spans.splice(insert_at..insert_at, halves);
        spans
    }
}

impl SteepShallowOp<'_> {
    /// Run the shared link stage over `tp`, when it is on. `tp` unchanged
    /// when it is off.
    fn link(&self, tp: Toolpath, report: &mut SteepShallowReport) -> Toolpath {
        let Some(stage) = &self.link_stage else {
            return tp;
        };
        let params = self.params;
        let rp = stage.params(&LinkGeometry {
            stock_to_leave: params.stock_to_leave,
            sampling: params.sampling,
            feed_rate: params.feed_rate,
            plunge_rate: params.plunge_rate,
            safe_z: params.safe_z,
            // G-TIERBURIAL: a link feed is refined against the surface
            // within the op tolerance, or refused.
            link_tolerance: Some(params.tolerance),
        });
        let (linked, rep) = relink_fragments(
            AnnotatedToolpath::new(tp),
            self.mesh,
            self.index,
            self.cutter,
            &rp,
        );
        if let Some(totals) = report.relink.as_mut() {
            totals.add(&rep);
        }
        linked
            .reconcile(&mut ReconcileSet::empty())
            .into_inner()
            .toolpath
    }

    /// [`Self::link`] for the steep half, one Z level at a time: the stage
    /// reorders the runs it links, and the steep half must keep its Z
    /// ladder (its `depth_ordered` node).
    fn link_levels(
        &self,
        tp: Toolpath,
        level_starts: &[usize],
        report: &mut SteepShallowReport,
    ) -> Toolpath {
        if self.link_stage.is_none() {
            return tp;
        }
        let mut out = Toolpath::new();
        let mut moves = tp.moves;
        for &start in level_starts.iter().rev() {
            let level = Toolpath {
                moves: moves.split_off(start.min(moves.len())),
            };
            if level.moves.is_empty() {
                continue;
            }
            let mut linked = self.link(level, report).moves;
            linked.append(&mut out.moves);
            out.moves = linked;
        }
        out
    }
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
    use crate::surface::slope::SurfaceHeightmap;
    use crate::tool::BallEndmill;

    fn make_hemisphere() -> (TriangleMesh, SpatialIndex) {
        let mesh = crate::mesh::make_test_hemisphere(20.0, 16);
        let si = SpatialIndex::build(&mesh, 10.0);
        (mesh, si)
    }

    fn ball_cutter() -> BallEndmill {
        BallEndmill::new(6.35, 25.0)
    }

    /// The whole op, never cancelled, with no boundary.
    fn toolpath(
        mesh: &TriangleMesh,
        index: &SpatialIndex,
        cutter: &dyn MillingCutter,
        params: &SteepShallowParams,
    ) -> Toolpath {
        crate::interrupt::run_uncancellable(|c| {
            steep_shallow_toolpath_split_with_cancel(mesh, index, cutter, params, None, c)
        })
        .0
    }

    // ── Grid morphology tests ───────────────────────────────────────

    #[test]
    fn test_dilate_grid_single_cell() {
        // Single true cell in center, dilate by 1 → 3×3 block
        let mut grid = vec![false; 25]; // 5×5
        grid[2 * 5 + 2] = true; // Center
        let dilated = dilate_grid(&grid, 5, 5, 1);

        for dr in -1i32..=1 {
            for dc in -1i32..=1 {
                let r = (2 + dr) as usize;
                let c = (2 + dc) as usize;
                assert!(
                    dilated[r * 5 + c],
                    "Cell ({},{}) should be true after dilation",
                    r,
                    c
                );
            }
        }
        // Corner should still be false
        assert!(!dilated[0], "Corner should remain false");
    }

    #[test]
    fn test_erode_grid() {
        // 7×7 grid: true in center 5×5 block, false border → erode by 1 shrinks to 3×3
        let rows = 7;
        let cols = 7;
        let mut grid = vec![false; rows * cols];
        for r in 1..6 {
            for c in 1..6 {
                grid[r * cols + c] = true;
            }
        }
        let eroded = erode_grid(&grid, rows, cols, 1);

        // Border of the original block (row/col 1 and 5) should be eroded
        assert!(!eroded[cols + 1], "Edge of block should be eroded");
        assert!(!eroded[5 * cols + 5], "Edge of block should be eroded");
        // Interior (rows 2-4, cols 2-4) should survive
        assert!(eroded[3 * cols + 3], "Center should survive erosion");
        assert!(eroded[2 * cols + 2], "Inner cell should survive erosion");
    }

    #[test]
    fn test_dilate_erode_identity() {
        // Dilate then erode by same amount should approximately preserve shape
        // (not exact due to morphological properties, but close)
        let rows = 10;
        let cols = 10;
        let mut grid = vec![false; rows * cols];
        // Create a 4×4 block in center
        for r in 3..7 {
            for c in 3..7 {
                grid[r * cols + c] = true;
            }
        }
        let dilated = dilate_grid(&grid, rows, cols, 1);
        let restored = erode_grid(&dilated, rows, cols, 1);

        // Original block should be present
        for r in 3..7 {
            for c in 3..7 {
                assert!(
                    restored[r * cols + c],
                    "Original block cell ({},{}) should survive dilate+erode",
                    r,
                    c
                );
            }
        }
    }

    // ── Classification + region tests ───────────────────────────────

    #[test]
    fn test_steep_shallow_hemisphere_split() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let tool_radius = cutter.radius();
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
        let steep = classify_steep_shallow(&slope_map, 40.0);

        let steep_count = steep.iter().filter(|&&s| s).count();
        let shallow_count = steep.iter().filter(|&&s| !s).count();

        assert!(steep_count > 0, "Hemisphere should have steep cells, got 0");
        assert!(
            shallow_count > 0,
            "Hemisphere should have shallow cells, got 0"
        );
        // Outer ring should be steep, inner area shallow
        assert!(
            shallow_count > steep_count / 2,
            "Should have a significant shallow region: {} steep, {} shallow",
            steep_count,
            shallow_count
        );
    }

    // ── Integration tests ───────────────────────────────────────────

    #[test]
    fn test_steep_shallow_produces_toolpath() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = SteepShallowParams {
            stepover: 2.0,
            z_step: 2.0,
            sampling: 3.0,
            tolerance: 0.5,
            ..SteepShallowParams::default()
        };

        let tp = toolpath(&mesh, &si, &cutter, &params);
        assert!(
            tp.moves.len() > 10,
            "Should produce non-trivial toolpath, got {} moves",
            tp.moves.len()
        );
    }

    #[test]
    fn test_steep_has_waterline_passes() {
        // Steep passes should be at constant Z levels
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let tool_radius = cutter.radius();
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
        let steep_grid = classify_steep_shallow(&slope_map, 40.0);
        let steep_expanded = dilate_grid(&steep_grid, rows, cols, 2);

        let (steep_tp, _) = crate::interrupt::run_uncancellable(|c| {
            emit_steep_passes(
                &mesh,
                &si,
                &cutter,
                &slope_map,
                &steep_expanded,
                bbox.max.z,
                surface_hm.min_z_or_bbox_floor(),
                2.0,
                3.0,
                0.0,
                1000.0,
                500.0,
                30.0,
                None,
                None,
                c,
            )
        });

        // Steep waterline passes should have constant Z within each contour
        // (between rapids). Collect Z values of feed moves between rapids.
        let mut contour_z_values: Vec<f64> = Vec::new();
        let mut in_contour = false;
        for m in &steep_tp.moves {
            match m.move_type {
                crate::toolpath::MoveType::Rapid => {
                    if in_contour && contour_z_values.len() >= 2 {
                        let z_min = contour_z_values
                            .iter()
                            .copied()
                            .fold(f64::INFINITY, f64::min);
                        let z_max = contour_z_values
                            .iter()
                            .copied()
                            .fold(f64::NEG_INFINITY, f64::max);
                        assert!(
                            (z_max - z_min) < 0.1,
                            "Steep contour Z should be constant, got range {:.3}",
                            z_max - z_min
                        );
                    }
                    contour_z_values.clear();
                    in_contour = false;
                }
                crate::toolpath::MoveType::Linear { .. } if m.target.z < 29.0 => {
                    // Below safe_z → cutting
                    contour_z_values.push(m.target.z);
                    in_contour = true;
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_shallow_has_raster_passes() {
        // Shallow passes should have varying Z (following the surface)
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let tool_radius = cutter.radius();
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
        let steep_grid = classify_steep_shallow(&slope_map, 40.0);
        let shallow_grid: Vec<bool> = steep_grid.iter().map(|&s| !s).collect();
        let shallow_expanded = dilate_grid(&shallow_grid, rows, cols, 2);

        let shallow_tp = crate::interrupt::run_uncancellable(|c| {
            emit_shallow_passes(
                &mesh,
                &si,
                &cutter,
                &shallow_expanded,
                &slope_map,
                2.0,
                0.0,
                1000.0,
                500.0,
                30.0,
                None,
                None,
                c,
            )
        });

        // Shallow raster should have variable Z across the surface
        let cutting_z: Vec<f64> = shallow_tp
            .moves
            .iter()
            .filter(|m| {
                matches!(m.move_type, crate::toolpath::MoveType::Linear { .. }) && m.target.z < 29.0
            })
            .map(|m| m.target.z)
            .collect();

        if cutting_z.len() > 2 {
            let z_min = cutting_z.iter().copied().fold(f64::INFINITY, f64::min);
            let z_max = cutting_z.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            assert!(
                z_max - z_min > 1.0,
                "Shallow raster Z should vary (following surface), range was only {:.2}",
                z_max - z_min
            );
        }
    }

    #[test]
    fn test_steep_first_ordering() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = SteepShallowParams {
            stepover: 2.0,
            z_step: 2.0,
            sampling: 3.0,
            tolerance: 0.5,
            steep_first: true,
            ..SteepShallowParams::default()
        };

        let tp = toolpath(&mesh, &si, &cutter, &params);
        // With steep_first, waterline passes (constant Z) should come before
        // raster passes (variable Z). We can't perfectly distinguish in the
        // merged toolpath, but at minimum it should produce output.
        assert!(
            tp.moves.len() > 5,
            "Steep-first should produce moves, got {}",
            tp.moves.len()
        );
    }

    #[test]
    fn test_overlap_increases_coverage() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();

        // Without overlap
        let params_no_overlap = SteepShallowParams {
            stepover: 2.0,
            z_step: 2.0,
            sampling: 3.0,
            tolerance: 0.5,
            overlap_distance: 0.0,
            ..SteepShallowParams::default()
        };
        let tp_no = toolpath(&mesh, &si, &cutter, &params_no_overlap);

        // With overlap
        let params_overlap = SteepShallowParams {
            overlap_distance: 5.0,
            ..params_no_overlap
        };
        let tp_yes = toolpath(&mesh, &si, &cutter, &params_overlap);

        // Overlap should produce more or equal cutting distance
        assert!(
            tp_yes.total_cutting_distance() >= tp_no.total_cutting_distance() - 1.0,
            "Overlap should increase coverage: with={:.0} without={:.0}",
            tp_yes.total_cutting_distance(),
            tp_no.total_cutting_distance()
        );
    }

    // ── Regression: steep passes must not chord across excluded regions ──

    /// Regression test for the gouge bug: `generate_steep_passes` used to
    /// filter waterline contour points by the steep mask, then feed ALL
    /// survivors sequentially and unconditionally close the loop back to
    /// the first survivor. When survivors are non-contiguous (a shallow
    /// gap sits between them), that closing/connecting move chords
    /// straight across the excluded region at cutting feed, into material
    /// that sits above the pass's Z level.
    ///
    /// Uses a real hemisphere waterline contour (a genuine closed circular
    /// loop) with a hand-built steep mask (steep iff world X < 0) that has
    /// nothing to do with the hemisphere's real slope — it exists purely to
    /// force a deterministic half-kept / half-excluded split so the test
    /// doesn't depend on the real classifier's grid resolution.
    #[test]
    fn test_steep_passes_no_chord_across_excluded_gap() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let tool_radius = cutter.radius();
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

        // Hand-built mask: steep iff the cell's world X coordinate is
        // negative — a hard half-plane split unrelated to real slope.
        let mut steep_mask = vec![false; rows * cols];
        for row in 0..rows {
            for col in 0..cols {
                let x = origin_x + col as f64 * cell_size;
                if x < 0.0
                    && let Some(slot) = steep_mask.get_mut(row * cols + col)
                {
                    *slot = true;
                }
            }
        }

        let z_test = 8.0;
        let sampling = 1.0;

        // Capture the raw contour to establish the expected point spacing.
        let raw_contours = waterline_contours(&mesh, &si, &cutter, z_test, sampling);
        let contour = raw_contours
            .iter()
            .max_by_key(|c| c.len())
            .expect("hemisphere should produce a contour at z=8");
        assert!(
            contour.len() >= 8,
            "need a non-trivial contour to exercise the split, got {}",
            contour.len()
        );

        let mut max_adjacent_spacing = 0.0_f64;
        for i in 0..contour.len() {
            let a = contour[i];
            let b = contour[(i + 1) % contour.len()];
            let d = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
            max_adjacent_spacing = max_adjacent_spacing.max(d);
        }

        let (steep_tp, _) = crate::interrupt::run_uncancellable(|c| {
            emit_steep_passes(
                &mesh,
                &si,
                &cutter,
                &slope_map,
                &steep_mask,
                z_test,
                z_test - 0.5, // z_bottom: keeps the loop to a single Z level
                100.0,        // z_step larger than the range: exactly one pass
                sampling,
                0.0,
                1000.0,
                500.0,
                30.0,
                None,
                None,
                c,
            )
        });

        // No consecutive pair of cutting (FinishingCut) moves may be farther
        // apart than a small multiple of the contour's own point spacing —
        // that would mean a cutting move chorded across the excluded half.
        let max_allowed = (max_adjacent_spacing * 3.0).max(3.0);
        let mut prev_cut: Option<P3> = None;
        let mut saw_any_cut = false;
        for m in &steep_tp.moves {
            let is_cut = matches!(m.move_type, crate::toolpath::MoveType::Linear { .. })
                && m.intent == crate::toolpath::MoveIntent::FinishingCut;
            if is_cut {
                saw_any_cut = true;
                if let Some(prev) = prev_cut {
                    let d = ((m.target.x - prev.x).powi(2) + (m.target.y - prev.y).powi(2)).sqrt();
                    assert!(
                        d <= max_allowed,
                        "cutting move chorded across the excluded region: {:.2}mm \
                         (allowed {:.2}mm) from ({:.2},{:.2}) to ({:.2},{:.2})",
                        d,
                        max_allowed,
                        prev.x,
                        prev.y,
                        m.target.x,
                        m.target.y
                    );
                }
                prev_cut = Some(m.target);
            } else {
                // A rapid or plunge breaks the run — the next cutting move
                // starts a fresh pass and shouldn't be distance-checked
                // against whatever preceded the break.
                prev_cut = None;
            }
        }
        assert!(saw_any_cut, "expected at least one cutting move");
    }

    /// Companion to the gouge regression above: when every contour point
    /// survives the steep filter (no excluded gap at all), each pass must
    /// still close back to its own start — the closed-loop behavior must
    /// be preserved for genuinely closed contours.
    #[test]
    fn test_steep_passes_still_close_loop_when_fully_steep() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let tool_radius = cutter.radius();
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

        // Every cell steep: the whole contour should survive the filter.
        let all_steep = vec![true; rows * cols];

        let (steep_tp, _) = crate::interrupt::run_uncancellable(|c| {
            emit_steep_passes(
                &mesh, &si, &cutter, &slope_map, &all_steep, 8.0, 7.5, 100.0, 1.0, 0.0, 1000.0,
                500.0, 30.0, None, None, c,
            )
        });

        // Reconstruct each pass (a run of plunge+cut moves between rapids)
        // and assert every pass returns to its own starting XY.
        let mut passes: Vec<Vec<P3>> = Vec::new();
        let mut current: Vec<P3> = Vec::new();
        for m in &steep_tp.moves {
            match m.move_type {
                crate::toolpath::MoveType::Rapid => {
                    if current.len() >= 2 {
                        passes.push(std::mem::take(&mut current));
                    } else {
                        current.clear();
                    }
                }
                crate::toolpath::MoveType::Linear { .. }
                    if m.intent == crate::toolpath::MoveIntent::EntryPlunge
                        || m.intent == crate::toolpath::MoveIntent::FinishingCut =>
                {
                    current.push(m.target);
                }
                _ => {}
            }
        }
        if current.len() >= 2 {
            passes.push(current);
        }

        assert!(!passes.is_empty(), "expected at least one steep pass");
        for pass in &passes {
            let first = pass.first().copied();
            let last = pass.last().copied();
            if let (Some(first), Some(last)) = (first, last) {
                let d = ((first.x - last.x).powi(2) + (first.y - last.y).powi(2)).sqrt();
                assert!(
                    d < 1e-6,
                    "fully-steep contour pass should close its loop, gap was {d:.4}mm"
                );
            }
        }
    }

    // ── P2.3: boundary_regions pre-clip ──────────────────────────────

    #[test]
    fn steep_shallow_boundary_regions_confines_cuts_to_region() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = SteepShallowParams {
            stepover: 2.0,
            z_step: 2.0,
            sampling: 3.0,
            tolerance: 0.5,
            ..SteepShallowParams::default()
        };
        let never_cancel = || false;

        // Left half of the hemisphere's XY footprint (bbox roughly [-20,20]).
        let bbox = &mesh.bbox;
        let left_half = crate::polygon::Polygon2::new(vec![
            crate::geo::P2::new(bbox.min.x, bbox.min.y),
            crate::geo::P2::new(0.0, bbox.min.y),
            crate::geo::P2::new(0.0, bbox.max.y),
            crate::geo::P2::new(bbox.min.x, bbox.max.y),
        ]);

        let left_half_regions = std::slice::from_ref(&left_half);
        let region_set = RegionSet::from_slice(left_half_regions);
        let (tp, _) = steep_shallow_toolpath_split_with_cancel(
            &mesh,
            &si,
            &cutter,
            &params,
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

    // ── W6.4.6: the rest rule ────────────────────────────────────────

    /// The op with a rest rule `standing`, timed by the default machine.
    fn rest_toolpath(standing: &dyn Fn(P3) -> bool) -> (Toolpath, SteepShallowSplit) {
        let timing = crate::machine::MachineProfile::default().link_kinematics();
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = SteepShallowParams {
            stepover: 2.0,
            z_step: 2.0,
            sampling: 3.0,
            tolerance: 0.5,
            ..SteepShallowParams::default()
        };
        let op = SteepShallowOp {
            mesh: &mesh,
            index: &si,
            cutter: &cutter,
            params: &params,
            boundary_regions: None,
            resolution: steep_shallow_generation_resolution(&cutter, params.tolerance),
            rest: Some(ComposedRest {
                standing: Box::new(standing),
                split_timing: Some(&timing),
            }),
            link_stage: None,
        };
        let composed = crate::interrupt::run_uncancellable(|c| compose(&op, c));
        (composed.toolpath, composed.report.split)
    }

    /// The X of each cutting move in `range`.
    fn cut_xs(tp: &Toolpath, range: Range<usize>) -> Vec<f64> {
        tp.moves[range]
            .iter()
            .filter(|m| {
                matches!(m.move_type, crate::toolpath::MoveType::Linear { .. })
                    && m.intent == crate::toolpath::MoveIntent::FinishingCut
            })
            .map(|m| m.target.x)
            .collect()
    }

    /// A rule that sees material only at `x < 0`:
    ///
    /// - confines the shallow half to `x < 0`: a dropped stretch at the end
    ///   of a raster row has no kept point after it;
    /// - drops a part of the steep half at `x > 0`: on a high contour the
    ///   retract is shorter than the feed through the dropped half loop,
    ///   and on a low one the time rule feeds through it.
    ///
    /// A rule that sees material everywhere gives the fresh toolpath, so
    /// the rule alone makes the difference.
    #[test]
    fn the_rest_rule_confines_both_halves_to_the_standing_material() {
        let (fresh, _) = rest_toolpath(&|_| true);
        let (mesh, si) = make_hemisphere();
        let params = SteepShallowParams {
            stepover: 2.0,
            z_step: 2.0,
            sampling: 3.0,
            tolerance: 0.5,
            ..SteepShallowParams::default()
        };
        assert_eq!(
            format!("{:?}", fresh.moves),
            format!("{:?}", toolpath(&mesh, &si, &ball_cutter(), &params).moves),
            "a rule that keeps every point must give the fresh toolpath"
        );
        let (_, fresh_split) = rest_toolpath(&|_| true);
        let (rest, split) = rest_toolpath(&|p| p.x < 0.0);
        let shallow = cut_xs(&rest, split.shallow);
        assert!(
            !shallow.is_empty(),
            "the shallow half cuts nothing at x < 0"
        );
        let worst = shallow.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        assert!(
            worst < 0.0,
            "the shallow half cuts at x = {worst:.3}, where the rule sees no material"
        );
        let right = |xs: Vec<f64>| xs.iter().filter(|&&x| x > 0.0).count();
        let (fresh_right, rest_right) = (
            right(cut_xs(&fresh, fresh_split.steep)),
            right(cut_xs(&rest, split.steep)),
        );
        assert!(
            rest_right < fresh_right,
            "the steep half keeps {rest_right} cutting moves at x > 0, the fresh one \
             {fresh_right}: the rule drops no stretch"
        );
    }

    /// G-LINKVISIBLE: the report holds link totals exactly when the
    /// `hookup_mm` stage ran.
    #[test]
    fn the_report_holds_link_totals_only_when_the_stage_ran() {
        let (mesh, si) = make_hemisphere();
        let cutter = ball_cutter();
        let params = SteepShallowParams {
            stepover: 2.0,
            z_step: 2.0,
            sampling: 3.0,
            tolerance: 0.5,
            ..SteepShallowParams::default()
        };
        let report = |link_stage: Option<FinishingLinkStage<'_>>| {
            let op = SteepShallowOp {
                mesh: &mesh,
                index: &si,
                cutter: &cutter,
                params: &params,
                boundary_regions: None,
                resolution: steep_shallow_generation_resolution(&cutter, params.tolerance),
                rest: None,
                link_stage,
            };
            crate::interrupt::run_uncancellable(|c| compose(&op, c)).report
        };
        assert!(report(None).relink.is_none(), "no stage, no totals");
        let on = report(Some(FinishingLinkStage {
            hookup_distance: 5.0,
            link_ceiling: None,
            boundary: None,
            link_kinematics: None,
        }));
        let totals = on.relink.expect("a stage that ran records its totals");
        assert!(totals.fragments > 0, "the stage saw the runs: {totals:?}");
    }
}
