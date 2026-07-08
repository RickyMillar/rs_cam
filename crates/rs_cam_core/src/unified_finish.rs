//! P2.c + P2.d of the unified finishing pass
//! (`planning/unified_finish_planner_design.md`): the per-region generation
//! orchestrator and the region ROUTER.
//!
//! [`unified_finish_toolpath_with_cancel`] composes the P2.b decomposition
//! ([`crate::finish_planner::decompose`]) with three EXISTING region-scoped
//! strategy generators — since P2.d, one call PER REGION (single-polygon
//! [`RegionSet`]) rather than per band, so regions can be ordered freely.
//! With a [`LinkKinematics`] envelope in scope the regions are then routed
//! greedily: each junction's edge cost is
//! `min(retract_link_time, surface_link_time)` integrated with the F-034
//! model (P0 lesson: NEVER distance/feed — it misjudges segmented paths by
//! up to 10×), the route is seeded at the steepest band present
//! (steep-first tool-freshness, `steep_shallow`'s established ordering),
//! and the WINNING candidate is what actually gets emitted between the two
//! region toolpaths: a gouge-checked surface link (Linking feeds, follower's
//! rapid+plunge preamble stripped) or the native retract+rapid+replunge.
//! Without `LinkKinematics` the order falls back to steep-first band-major
//! concatenation (P2.c behavior) with native links only — never a
//! distance-costed guess.
//!
//! Creases stay with the standalone pencil op at this checkpoint —
//! [`crate::finish_planner::decompose`] is called with an empty crease
//! slice here on purpose. Crease routing folds in later.
//!
//! No dressups, no boundary clipping here: the stitched toolpath this
//! module returns flows through the NORMAL session post-passes (boundary
//! clip, `optimize_entry_descents`, feed modulation, F-034 accounting)
//! exactly like every other op's raw generator output.

use crate::debug_trace::ToolpathDebugContext;
use crate::dropcutter::{DropCutterGrid, batch_drop_cutter_with_cancel};
use crate::finish_planner::{FinishBand, FinishPlannerParams, decompose};
use crate::finish_setup::{
    FinishSurface, SLOPE_FILTER_MAX_DEG, SLOPE_FILTER_MIN_DEG,
    build_classification_surface_with_cancel,
};
use crate::geo::{P2, P3};
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::machine_kinematics::{LinkKinematics, retract_link_time, surface_link_time};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::region_set::RegionSet;
use crate::scallop::{
    ScallopDirection, ScallopParams, ScallopRuntimeAnnotation,
    scallop_toolpath_structured_annotated_with_cancel,
};
use crate::surface_link::build_surface_link;
use crate::tool::MillingCutter;
use crate::toolpath::{MoveIntent, Toolpath, raster_toolpath_from_grid};
use crate::waterline::{WaterlineParams, waterline_toolpath_with_cancel};

// ── Types ────────────────────────────────────────────────────────────────

/// Inherited-dial bundle for the unified finish op (one-new-dial rule: the
/// genuinely new dials are the two thresholds + overlap, which live on
/// [`FinishPlannerParams`]; everything here is inherited from the existing
/// per-strategy params).
#[derive(Debug, Clone)]
pub struct UnifiedFinishParams {
    /// Scallop height for mid-steep rings (mm) — scallop's primary dial.
    pub scallop_height: f64,
    /// Path tolerance (also drives classification/generation cell size).
    pub tolerance: f64,
    /// Raster stepover for shallow regions (mm).
    pub raster_stepover: f64,
    /// Waterline Z step for very-steep regions (mm).
    pub z_step: f64,
    /// Waterline contour sampling (mm).
    pub sampling: f64,
    /// Stock to leave on the surface (mm) — scallop path only (raster and
    /// waterline don't take one today; parity with the standalone ops).
    pub stock_to_leave: f64,
    pub feed_rate: f64,
    pub plunge_rate: f64,
    pub safe_z: f64,
}

impl Default for UnifiedFinishParams {
    /// Mirrors the standalone ops' own defaults (`ScallopConfig` /
    /// `WaterlineConfig` / `DropCutterConfig` in
    /// `compute::operation_configs`) so switching between the standalone
    /// three-op stack and this orchestrator doesn't silently change
    /// feeds/quality at the same tool. `safe_z` has no config-struct
    /// equivalent (it's the runtime `ctx.heights.retract_z` at the op-adapter
    /// layer, wave 2) — 30.0 mirrors `ScallopParams::default()`'s
    /// stand-in value.
    fn default() -> Self {
        Self {
            scallop_height: 0.1,
            tolerance: 0.05,
            raster_stepover: 1.0,
            z_step: 1.0,
            sampling: 0.5,
            stock_to_leave: 0.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            safe_z: 30.0,
        }
    }
}

/// Per-band generation telemetry for the report/debug surface.
#[derive(Debug, Clone, Copy, Default)]
pub struct BandGenStats {
    pub region_count: usize,
    pub move_count: usize,
}

/// One routed junction between two consecutive regions in the cut order
/// (P2.d telemetry — lets the A/B harness see which link kind won where).
#[derive(Debug, Clone, Copy)]
pub struct RoutedLink {
    /// Index into the decomposition's `planned.regions` we linked FROM.
    pub from_region: usize,
    /// Index into `planned.regions` we linked TO.
    pub to_region: usize,
    /// True when the gouge-checked surface link won the integrator
    /// comparison and was emitted as Linking feeds; false when the native
    /// retract+rapid+replunge was kept.
    pub surface: bool,
    /// Integrated time (s) of the winning candidate.
    pub cost_s: f64,
    /// Integrated time (s) of the losing candidate — `None` when the
    /// surface candidate was unavailable (off-mesh, outside the machining
    /// boundary, or the follower has no strippable entry preamble), so no
    /// comparison happened.
    pub alt_cost_s: Option<f64>,
}

/// Orchestration report: decomposition stats + what each band generated +
/// the P2.d route.
#[derive(Debug, Clone, Default)]
pub struct UnifiedFinishReport {
    pub decompose: crate::finish_planner::DecomposeStats,
    pub very_steep: BandGenStats,
    pub mid_steep: BandGenStats,
    pub shallow: BandGenStats,
    /// Region cut order (indices into the decomposition's
    /// `planned.regions`). Steep-first band-major when no
    /// [`LinkKinematics`] was supplied; greedy link-costed otherwise.
    pub route: Vec<usize>,
    /// The junction decisions, `route.len().saturating_sub(1)` entries —
    /// empty when routing ran without kinematics (no costing happened).
    pub links: Vec<RoutedLink>,
}

// ── Orchestrator ─────────────────────────────────────────────────────────

/// Decompose the surface into bands, generate each REGION's toolpath with
/// the appropriate EXISTING strategy generator, route the regions (greedy
/// nearest-by-integrated-link-time when `link_kinematics` is `Some`;
/// steep-first band-major otherwise), and stitch them with the winning
/// link candidate emitted at each junction.
#[allow(clippy::too_many_arguments)] // op-generator adapter surface, mirrors the strategy fns it composes
pub fn unified_finish_toolpath_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    top_z: f64,
    bottom_z: f64,
    params: &UnifiedFinishParams,
    planner: &FinishPlannerParams,
    machining_boundary: Option<&RegionSet<'_>>,
    link_kinematics: Option<&LinkKinematics>,
    debug: Option<&ToolpathDebugContext>,
    cancel: &dyn CancelCheck,
) -> Result<(Toolpath, Vec<ScallopRuntimeAnnotation>, UnifiedFinishReport), Cancelled> {
    check_cancel(cancel)?;

    // ── Step 1: classify on the TRUE surface ────────────────────────────
    // P2.b decision (`finish_setup::build_classification_surface_with_cancel`
    // doc): a ball tool's offset (generation) surface geometrically hides
    // steepness at feature scales at/below the ball radius. Classification
    // MUST read the true surface or band assignment collapses to
    // all-shallow on relief at that scale — never swap this for
    // `build_finish_surface_with_cancel`.
    let surface =
        build_classification_surface_with_cancel(mesh, index, cutter, params.tolerance, cancel)?;
    check_cancel(cancel)?;

    // ── Step 2: coverage ∧ machining boundary ───────────────────────────
    let cols = surface.cols();
    let cell = surface.cell_size();
    let origin_x = surface.slope_map.origin_x;
    let origin_y = surface.slope_map.origin_y;
    let covered: Vec<bool> = surface
        .heightmap
        .covered
        .iter()
        .enumerate()
        .map(|(i, &cov)| {
            if !cov {
                return false;
            }
            let Some(boundary) = machining_boundary else {
                return true;
            };
            let row = i / cols;
            let col = i % cols;
            let x = origin_x + col as f64 * cell;
            let y = origin_y + row as f64 * cell;
            boundary.contains(&P2::new(x, y))
        })
        .collect();
    check_cancel(cancel)?;

    // ── Step 3: decompose ────────────────────────────────────────────────
    // Creases empty: P2.c A/B isolates band machinery (module doc). Crease
    // routing is a later increment; narrow/wide creases stay wherever the
    // standalone pencil op finds them today.
    let planned = decompose(&surface.slope_map, &covered, &[], cutter.radius(), planner);
    check_cancel(cancel)?;

    // ── Step 4: per-region generation ───────────────────────────────────
    // P2.d: one strategy call PER REGION (single-polygon RegionSet) so the
    // router can order regions freely. The raster grid is mesh-global and
    // region-independent, so it is computed once (lazily) and shared by
    // every Shallow region; scallop and waterline rebuild their internal
    // surfaces per call — O(1) regions per band on wanaka today, so this
    // duplication is accepted and flagged as a P2.e datapoint if
    // conditioned region counts ever grow.
    let mut report = UnifiedFinishReport {
        decompose: planned.stats,
        ..UnifiedFinishReport::default()
    };
    let mut region_paths: Vec<RegionPath> = Vec::new();
    let mut shallow_grid: Option<DropCutterGrid> = None;

    for (region_index, region) in planned.regions.iter().enumerate() {
        check_cancel(cancel)?;
        let region_set = RegionSet::new(vec![region.polygon.clone()]);
        let (tp, anns) = match region.band {
            FinishBand::VerySteep => {
                let Some((band_min_z, band_max_z)) = band_z_range(&surface, &covered, &region_set)
                else {
                    tracing::warn!(
                        region_index,
                        "unified_finish: VerySteep region has no covered cells inside its own polygon; skipping"
                    );
                    continue;
                };
                let start_z = band_max_z.min(top_z);
                let final_z = band_min_z.max(bottom_z);
                let wp = WaterlineParams {
                    sampling: params.sampling,
                    feed_rate: params.feed_rate,
                    plunge_rate: params.plunge_rate,
                    safe_z: params.safe_z,
                };
                // `waterline_toolpath_with_cancel` ladders start_z..final_z
                // by `z_step` internally — no need to precompute levels.
                // Per-region z-range: each region only ladders the levels
                // its own surface patch spans.
                let tp = waterline_toolpath_with_cancel(
                    mesh,
                    index,
                    cutter,
                    start_z,
                    final_z,
                    params.z_step,
                    &wp,
                    Some(&region_set),
                    cancel,
                )?;
                (tp, Vec::new())
            }
            FinishBand::MidSteep => {
                let sp = ScallopParams {
                    scallop_height: params.scallop_height,
                    tolerance: params.tolerance,
                    direction: ScallopDirection::default(),
                    // Design decision #1: scallop-continuous rings for the
                    // mid-steep band.
                    continuous: true,
                    // Full slope window: the REGION is the confinement now
                    // (the band's polygon already IS the slope-selection),
                    // so scallop's own slope_from/slope_to filter is
                    // deliberately a no-op — see `finish_setup::
                    // {SLOPE_FILTER_MIN_DEG, SLOPE_FILTER_MAX_DEG}`.
                    slope_from: SLOPE_FILTER_MIN_DEG,
                    slope_to: SLOPE_FILTER_MAX_DEG,
                    feed_rate: params.feed_rate,
                    plunge_rate: params.plunge_rate,
                    safe_z: params.safe_z,
                    stock_to_leave: params.stock_to_leave,
                };
                // Generation intentionally uses the ball-center OFFSET
                // surface here (`scallop_toolpath_structured_annotated_
                // with_cancel` builds its own via `finish_setup::
                // build_finish_surface_with_cancel` internally) — only
                // classification (step 1, above) reads the true surface.
                // This is the P2.b "classify true, generate offset" split
                // from the design doc, not an inconsistency.
                scallop_toolpath_structured_annotated_with_cancel(
                    mesh,
                    index,
                    cutter,
                    &sp,
                    debug,
                    Some(&region_set),
                    cancel,
                )?
            }
            FinishBand::Shallow => {
                let grid = match shallow_grid.as_ref() {
                    Some(grid) => grid,
                    None => {
                        // Mesh-bottom floor, mirroring `generate_drop_cutter`'s
                        // `effective_min_z` (compute::execute.rs). The
                        // stock-bbox floor (`ctx.stock_bbox.min.z - 1.0`
                        // there) is the op-adapter's job — this pure-core
                        // function only sees a mesh, not a stock model, so
                        // callers needing that extra floor should pre-max it
                        // into `bottom_z` before calling in.
                        let effective_min_z = mesh.bbox.min.z - 0.1;
                        // `batch_drop_cutter_with_cancel` requires
                        // `&(dyn CancelCheck + Sync)` for its rayon closures;
                        // this function only receives a plain
                        // `&dyn CancelCheck`, matching every finish-op call
                        // site up this chain (see the identical constraint
                        // documented on `slope::SurfaceHeightmap::
                        // from_mesh_with_cancel`). Widening our own signature
                        // to `+ Sync` would ripple through every future
                        // caller for the sake of one internal call, so this
                        // band checks cancellation immediately before and
                        // after the batch call instead of threading `cancel`
                        // through it.
                        let never_cancel = || false;
                        let mut grid = batch_drop_cutter_with_cancel(
                            mesh,
                            index,
                            cutter,
                            params.raster_stepover,
                            0.0,
                            effective_min_z,
                            &never_cancel,
                        )?;
                        // Drop grid points whose vertical ray misses every
                        // triangle in the mesh. Replicated from
                        // `compute::execute::generate_drop_cutter`'s
                        // identical guard: `point_drop_cutter` marks a point
                        // contacted whenever the cutter (which has radius)
                        // touches ANY nearby triangle — including the rim of
                        // a mesh that doesn't cover that XY. Without this
                        // check the tool rides the edge and carves a trench
                        // around the part.
                        for pt in &mut grid.points {
                            let mut over = false;
                            for &tri_idx in &index.query(pt.x, pt.y, 0.0) {
                                // SAFETY: tri_idx comes from `index.query`,
                                // which only ever returns indices into
                                // `mesh.faces` (mirrors `generate_drop_cutter`'s
                                // identical loop in compute::execute.rs).
                                #[allow(clippy::indexing_slicing)]
                                let tri = &mesh.faces[tri_idx];
                                if tri.contains_point_xy(pt.x, pt.y) {
                                    over = true;
                                    break;
                                }
                            }
                            if !over {
                                pt.z = effective_min_z;
                                pt.contacted = false;
                            }
                        }
                        check_cancel(cancel)?;
                        shallow_grid.insert(grid)
                    }
                };
                let effective_min_z = mesh.bbox.min.z - 0.1;
                let tp = raster_toolpath_from_grid(
                    grid,
                    params.feed_rate,
                    params.plunge_rate,
                    params.safe_z,
                    Some(effective_min_z),
                    Some(&region_set),
                );
                (tp, Vec::new())
            }
        };

        let stats = match region.band {
            FinishBand::VerySteep => &mut report.very_steep,
            FinishBand::MidSteep => &mut report.mid_steep,
            FinishBand::Shallow => &mut report.shallow,
        };
        stats.region_count += 1;
        stats.move_count += tp.moves.len();
        if tp.moves.is_empty() {
            continue;
        }

        let (head_strip, entry) = strippable_preamble(&tp).map_or((0, None), |(k, p)| (k, Some(p)));
        let (tail_strip, exit) = trailing_retracts(&tp);
        region_paths.push(RegionPath {
            region_index,
            band: region.band,
            tp,
            anns,
            head_strip,
            entry,
            tail_strip,
            exit,
        });
    }

    if region_paths.is_empty() {
        return Ok((Toolpath::new(), Vec::new(), report));
    }

    // ── Step 5: route ─────────────────────────────────────────────────────
    let (order, junctions) = match link_kinematics {
        Some(lk) => route_greedy(
            &region_paths,
            mesh,
            index,
            cutter,
            params,
            machining_boundary,
            lk,
            cancel,
        )?,
        // No machine envelope in scope: steep-first band-major order
        // (P2.c's naive concat), native links only. NEVER a
        // distance-costed guess — the P0 probe measured distance/feed
        // misjudging segmented finishing paths by up to 10×.
        None => {
            let mut order: Vec<usize> = (0..region_paths.len()).collect();
            order.sort_by_key(|&i| {
                let band = region_paths
                    .get(i)
                    .map_or(FinishBand::Shallow, |rp| rp.band);
                (std::cmp::Reverse(band_rank(band)), i)
            });
            let junctions = (0..order.len().saturating_sub(1)).map(|_| None).collect();
            (order, junctions)
        }
    };

    report.route = order
        .iter()
        .filter_map(|&i| region_paths.get(i).map(|rp| rp.region_index))
        .collect();
    for (j, junction) in junctions.iter().enumerate() {
        let (Some(&from), Some(&to)) = (order.get(j), order.get(j + 1)) else {
            continue;
        };
        let (Some(from_rp), Some(to_rp)) = (region_paths.get(from), region_paths.get(to)) else {
            continue;
        };
        if let Some(link) = junction {
            report.links.push(RoutedLink {
                from_region: from_rp.region_index,
                to_region: to_rp.region_index,
                surface: link.surface,
                cost_s: link.cost_s,
                alt_cost_s: link.alt_cost_s,
            });
        }
    }

    // ── Step 6: stitch in route order, emitting the winning links ───────
    let mut stitched = Toolpath::new();
    let mut annotations: Vec<ScallopRuntimeAnnotation> = Vec::new();
    for (pos, &pi) in order.iter().enumerate() {
        let incoming_surface = pos
            .checked_sub(1)
            .and_then(|j| junctions.get(j))
            .and_then(|l| l.as_ref())
            .filter(|l| l.surface);
        let outgoing_surface = junctions
            .get(pos)
            .and_then(|l| l.as_ref())
            .is_some_and(|l| l.surface);
        let Some(rp) = region_paths.get_mut(pi) else {
            continue;
        };

        // The incoming surface link replaces the follower's rapid+plunge
        // preamble; the outgoing one replaces this region's trailing
        // retract(s). Native junctions keep both untouched.
        let head = if incoming_surface.is_some() {
            rp.head_strip
        } else {
            0
        };
        let tail = if outgoing_surface { rp.tail_strip } else { 0 };

        if let (Some(link), Some(entry)) = (incoming_surface, rp.entry) {
            for p in &link.pts {
                stitched.feed_to_with_intent(*p, params.feed_rate, MoveIntent::Linking);
            }
            stitched.feed_to_with_intent(entry, params.feed_rate, MoveIntent::Linking);
        }

        let offset = stitched.moves.len();
        let kept = rp.tp.moves.len().saturating_sub(head + tail);
        stitched
            .moves
            .extend(rp.tp.moves.iter().skip(head).take(kept).cloned());

        // Annotation `move_index` is local to this region's own toolpath;
        // shift into the stitched frame. An annotation pointing into a
        // stripped preamble clamps to the first kept move (the ring it
        // labels now begins right at the link's landing point).
        let anns = std::mem::take(&mut rp.anns);
        annotations.extend(anns.into_iter().map(|a| {
            let move_index = if a.move_index < head {
                offset
            } else if a.move_index >= head + kept {
                (offset + kept).saturating_sub(1)
            } else {
                offset + (a.move_index - head)
            };
            ScallopRuntimeAnnotation {
                move_index,
                event: a.event,
            }
        }));
    }

    Ok((stitched, annotations, report))
}

// ── P2.d router internals ─────────────────────────────────────────────────

/// One region's generated toolpath plus the stitch metadata the router
/// needs: where the tool ENTERS the surface (its plunge target), where it
/// LEAVES it (last cut before the trailing retracts), and how many moves
/// each end contributes so a surface link can strip them.
struct RegionPath {
    /// Index into the decomposition's `planned.regions`.
    region_index: usize,
    band: FinishBand,
    tp: Toolpath,
    anns: Vec<ScallopRuntimeAnnotation>,
    /// Leading moves (Linking rapid + EntryPlunge) removable when a surface
    /// link lands the tool at `entry` directly. 0 = not strippable.
    head_strip: usize,
    /// Surface entry point — the first run's EntryPlunge target.
    entry: Option<P3>,
    /// Trailing `MoveIntent::Retract` moves removable when the NEXT
    /// junction is a surface link (the tool stays on the surface).
    tail_strip: usize,
    /// Surface exit point — the last move before the trailing retracts.
    exit: Option<P3>,
}

/// A costed junction between two consecutive regions in the route.
struct JunctionChoice {
    surface: bool,
    /// Interior surface-link points (empty for a retract junction, and for
    /// a zero-length surface junction).
    pts: Vec<P3>,
    cost_s: f64,
    alt_cost_s: Option<f64>,
}

fn band_rank(band: FinishBand) -> u8 {
    match band {
        FinishBand::VerySteep => 2,
        FinishBand::MidSteep => 1,
        FinishBand::Shallow => 0,
    }
}

/// Leading strippable preamble: the index `k` of the first
/// `FinishingCut` move plus the surface entry point (the move `k-1`
/// EntryPlunge target). `None` when the toolpath doesn't open with the
/// canonical `Linking rapid(s) → EntryPlunge → FinishingCut` shape every
/// strategy generator in this module emits — an unrecognized preamble is
/// kept verbatim rather than guessed at.
fn strippable_preamble(tp: &Toolpath) -> Option<(usize, P3)> {
    let k = tp
        .moves
        .iter()
        .position(|m| m.intent == MoveIntent::FinishingCut)?;
    if k == 0 {
        return None;
    }
    let head_ok = tp
        .moves
        .iter()
        .take(k)
        .all(|m| matches!(m.intent, MoveIntent::Linking | MoveIntent::EntryPlunge));
    if !head_ok {
        return None;
    }
    let plunge = tp.moves.get(k - 1)?;
    (plunge.intent == MoveIntent::EntryPlunge).then_some((k, plunge.target))
}

/// Trailing retract count + the surface exit point right before them.
fn trailing_retracts(tp: &Toolpath) -> (usize, Option<P3>) {
    let n = tp
        .moves
        .iter()
        .rev()
        .take_while(|m| m.intent == MoveIntent::Retract)
        .count();
    let exit = tp
        .moves
        .len()
        .checked_sub(n + 1)
        .and_then(|i| tp.moves.get(i))
        .map(|m| m.target);
    (n, exit)
}

/// Greedy nearest-by-integrated-link-time route over the generated
/// regions, seeded at the steepest band present (first planned region of
/// that band — deterministic). Returns the order (indices into `paths`)
/// and the winning junction choice for each consecutive pair. 2-opt is
/// deliberately absent: the design doc's P0 discipline is measure first —
/// it only gets built if the A/B says greedy leaves >5% on the table.
#[allow(clippy::too_many_arguments)] // link geometry + machine envelope are irreducible inputs
fn route_greedy(
    paths: &[RegionPath],
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &UnifiedFinishParams,
    machining_boundary: Option<&RegionSet<'_>>,
    lk: &LinkKinematics,
    cancel: &dyn CancelCheck,
) -> Result<(Vec<usize>, Vec<Option<JunctionChoice>>), Cancelled> {
    let seed = (0..paths.len()).max_by_key(|&i| {
        (
            paths.get(i).map_or(0, |rp| band_rank(rp.band)),
            std::cmp::Reverse(i),
        )
    });
    let Some(seed) = seed else {
        return Ok((Vec::new(), Vec::new()));
    };

    let mut visited = vec![false; paths.len()];
    if let Some(v) = visited.get_mut(seed) {
        *v = true;
    }
    let mut order = vec![seed];
    let mut junctions: Vec<Option<JunctionChoice>> = Vec::new();
    let mut current = seed;

    for _ in 1..paths.len() {
        check_cancel(cancel)?;
        let from = paths
            .get(current)
            .and_then(|rp| rp.exit.or_else(|| rp.tp.moves.last().map(|m| m.target)));
        let Some(from) = from else { break };

        let mut best: Option<(usize, JunctionChoice)> = None;
        for (j, candidate) in paths.iter().enumerate() {
            if visited.get(j).copied().unwrap_or(true) {
                continue;
            }
            let choice = choose_link(
                from,
                candidate,
                mesh,
                index,
                cutter,
                params,
                machining_boundary,
                lk,
            );
            // Strict `<` keeps the earliest candidate on exact ties —
            // deterministic (`paths` preserves the planned-region order).
            if best.as_ref().is_none_or(|(_, b)| choice.cost_s < b.cost_s) {
                best = Some((j, choice));
            }
        }
        let Some((next, choice)) = best else { break };
        if let Some(v) = visited.get_mut(next) {
            *v = true;
        }
        order.push(next);
        junctions.push(Some(choice));
        current = next;
    }

    Ok((order, junctions))
}

/// Cost both link candidates from `from` (the previous region's surface
/// exit) into `to` and return the winner. The surface candidate mirrors
/// pencil's emit decision exactly: gouge-checked via [`build_surface_link`]
/// (real cutter → it rides the OFFSET surface), additionally required to
/// stay inside the machining boundary (the selective-scallop gouge rule:
/// never feed across excluded islands), and only available when the
/// follower has a strippable entry. Ties go to the surface link, matching
/// pencil's `surface_t <= retract_t`.
#[allow(clippy::too_many_arguments)] // link geometry + machine envelope are irreducible inputs
fn choose_link(
    from: P3,
    to: &RegionPath,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &UnifiedFinishParams,
    machining_boundary: Option<&RegionSet<'_>>,
    lk: &LinkKinematics,
) -> JunctionChoice {
    let entry_surface = (to.head_strip > 0).then_some(to.entry).flatten();
    let retract_to = entry_surface
        .or(to.entry)
        .or_else(|| to.tp.moves.first().map(|m| m.target))
        .unwrap_or(from);
    let retract_t = retract_link_time(
        from,
        retract_to,
        params.safe_z,
        // No rapid-down-to-clearance: the cost model can't verify the input
        // stock's ceiling from here (that check lives in the post-generation
        // `optimize_entry_descents` pass), so it must not assume a descent
        // it can't guarantee is safe — same reasoning as pencil's emit path.
        None,
        params.plunge_rate,
        &lk.kinematics,
        lk.max_feed_mm_min,
        lk.rapid_feed_mm_min,
    );

    let surface_candidate = entry_surface.and_then(|entry| {
        let pts = build_surface_link(
            from,
            entry,
            mesh,
            index,
            cutter,
            params.stock_to_leave,
            params.sampling,
        )?;
        // Inside the machining boundary only — a surface feed across an
        // excluded island is exactly the gouge class the boundary exists
        // to prevent.
        if let Some(boundary) = machining_boundary
            && !pts.iter().all(|p| boundary.contains(&P2::new(p.x, p.y)))
        {
            return None;
        }
        let mut costed_path = pts.clone();
        costed_path.push(entry);
        let surface_t = surface_link_time(
            from,
            &costed_path,
            params.feed_rate,
            &lk.kinematics,
            lk.max_feed_mm_min,
            lk.rapid_feed_mm_min,
        );
        Some((pts, surface_t))
    });

    match surface_candidate {
        Some((pts, surface_t)) if surface_t <= retract_t => JunctionChoice {
            surface: true,
            pts,
            cost_s: surface_t,
            alt_cost_s: Some(retract_t),
        },
        Some((_, surface_t)) => JunctionChoice {
            surface: false,
            pts: Vec::new(),
            cost_s: retract_t,
            alt_cost_s: Some(surface_t),
        },
        None => JunctionChoice {
            surface: false,
            pts: Vec::new(),
            cost_s: retract_t,
            alt_cost_s: None,
        },
    }
}

/// Z range (min, max) of the classification surface's covered cells whose
/// centers fall inside `regions`. `None` when nothing survives both
/// filters — a genuinely degenerate band (its own polygon shrank to
/// nothing after conditioning, or every covered cell sits just outside it
/// at the mask boundary).
fn band_z_range(
    surface: &FinishSurface,
    covered: &[bool],
    regions: &RegionSet<'_>,
) -> Option<(f64, f64)> {
    let cols = surface.cols();
    if cols == 0 {
        return None;
    }
    let cell = surface.cell_size();
    let origin_x = surface.slope_map.origin_x;
    let origin_y = surface.slope_map.origin_y;

    let mut min_z = f64::INFINITY;
    let mut max_z = f64::NEG_INFINITY;
    for (i, &z) in surface.heightmap.z_values.iter().enumerate() {
        if !covered.get(i).copied().unwrap_or(false) {
            continue;
        }
        let row = i / cols;
        let col = i % cols;
        let x = origin_x + col as f64 * cell;
        let y = origin_y + row as f64 * cell;
        if regions.contains(&P2::new(x, y)) {
            min_z = min_z.min(z);
            max_z = max_z.max(z);
        }
    }
    (min_z.is_finite() && max_z.is_finite()).then_some((min_z, max_z))
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::mesh::{make_test_flat, make_test_hemisphere};
    use crate::polygon::Polygon2;
    use crate::tool::BallEndmill;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn ball(diameter: f64) -> BallEndmill {
        BallEndmill::new(diameter, diameter * 5.0)
    }

    // ── fixture 1: flat plate (Shallow only) ────────────────────────────

    #[test]
    fn flat_plate_generates_raster_only() {
        let mesh = make_test_flat(60.0);
        let index = SpatialIndex::build(&mesh, 10.0);
        let cutter = ball(6.0); // radius 3.0
        let params = UnifiedFinishParams::default();
        let planner = FinishPlannerParams::for_tool(cutter.radius());
        let never_cancel = || false;

        let (tp, anns, report) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            5.0,
            -5.0,
            &params,
            &planner,
            None,
            None,
            None,
            &never_cancel,
        )
        .unwrap();

        assert!(
            report.shallow.move_count > 0,
            "flat plate should produce shallow raster moves"
        );
        assert_eq!(report.mid_steep.move_count, 0);
        assert_eq!(report.very_steep.move_count, 0);
        assert!(!tp.moves.is_empty());
        assert!(
            anns.is_empty(),
            "raster-only run should carry no scallop annotations"
        );
    }

    // ── fixture 2: hemisphere (spans all three bands) ───────────────────
    //
    // Radius/tool-radius/cell-size mirror `finish_planner`'s own
    // `dome_decomposes_into_four_regions` test (radius 30, cell 1.0,
    // `FinishPlannerParams::for_tool(3.0)`) as closely as possible — that
    // test proves the decomposition dials produce a clean
    // shallow/mid/very-steep split at this exact scale. The only new
    // variable here is going through a real triangulated mesh + the
    // classification-surface probe pipeline instead of a synthetic z-grid;
    // R1's hysteresis/close/min-area conditioning is specifically built to
    // absorb the resulting facet noise.
    fn steep_cone_fixture() -> (TriangleMesh, SpatialIndex, BallEndmill, FinishPlannerParams) {
        let mesh = make_test_hemisphere(30.0, 24);
        let index = SpatialIndex::build(&mesh, 10.0);
        let cutter = ball(6.0); // radius 3.0
        let planner = FinishPlannerParams::for_tool(cutter.radius());
        (mesh, index, cutter, planner)
    }

    fn steep_cone_params() -> UnifiedFinishParams {
        UnifiedFinishParams {
            // Forces classification cell_size to exactly 1.0mm
            // (`(tool_radius / 4).max(tolerance)` with tool_radius = 3.0),
            // matching the mirrored `finish_planner` test's grid.
            tolerance: 1.0,
            ..UnifiedFinishParams::default()
        }
    }

    #[test]
    fn steep_cone_generates_multiple_bands() {
        let (mesh, index, cutter, planner) = steep_cone_fixture();
        let params = steep_cone_params();
        let never_cancel = || false;

        let (tp, _anns, report) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            35.0,
            0.0,
            &params,
            &planner,
            None,
            None,
            None,
            &never_cancel,
        )
        .unwrap();

        let populated = [report.very_steep, report.mid_steep, report.shallow]
            .iter()
            .filter(|b| b.move_count > 0)
            .count();
        assert!(
            populated >= 2,
            "hemisphere should span at least two bands, got very_steep={} mid_steep={} shallow={}",
            report.very_steep.move_count,
            report.mid_steep.move_count,
            report.shallow.move_count
        );
        let expected_total =
            report.very_steep.move_count + report.mid_steep.move_count + report.shallow.move_count;
        assert_eq!(tp.moves.len(), expected_total);
    }

    #[test]
    fn annotations_shifted_by_concat_offset() {
        let (mesh, index, cutter, planner) = steep_cone_fixture();
        let params = steep_cone_params();
        let never_cancel = || false;

        let (tp, anns, report) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            35.0,
            0.0,
            &params,
            &planner,
            None,
            None,
            None,
            &never_cancel,
        )
        .unwrap();

        assert!(
            !anns.is_empty(),
            "mid-steep band should carry scallop ring annotations"
        );
        for ann in &anns {
            assert!(
                ann.move_index < tp.moves.len(),
                "annotation move_index {} escaped the stitched toolpath ({} moves)",
                ann.move_index,
                tp.moves.len()
            );
            assert!(
                ann.move_index >= report.very_steep.move_count,
                "annotation move_index {} lands before the mid-steep band's concat offset ({})",
                ann.move_index,
                report.very_steep.move_count
            );
        }
    }

    #[test]
    fn deterministic() {
        let (mesh, index, cutter, planner) = steep_cone_fixture();
        let params = steep_cone_params();
        let never_cancel = || false;

        let (tp_a, _anns_a, report_a) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            35.0,
            0.0,
            &params,
            &planner,
            None,
            None,
            None,
            &never_cancel,
        )
        .unwrap();
        let (tp_b, _anns_b, report_b) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            35.0,
            0.0,
            &params,
            &planner,
            None,
            None,
            None,
            &never_cancel,
        )
        .unwrap();

        assert_eq!(tp_a.moves.len(), tp_b.moves.len());
        assert_eq!(
            report_a.very_steep.move_count,
            report_b.very_steep.move_count
        );
        assert_eq!(report_a.mid_steep.move_count, report_b.mid_steep.move_count);
        assert_eq!(report_a.shallow.move_count, report_b.shallow.move_count);

        let first_a = tp_a
            .moves
            .first()
            .map(|m| (m.target.x, m.target.y, m.target.z));
        let first_b = tp_b
            .moves
            .first()
            .map(|m| (m.target.x, m.target.y, m.target.z));
        assert_eq!(first_a, first_b);
        let last_a = tp_a
            .moves
            .last()
            .map(|m| (m.target.x, m.target.y, m.target.z));
        let last_b = tp_b
            .moves
            .last()
            .map(|m| (m.target.x, m.target.y, m.target.z));
        assert_eq!(last_a, last_b);
    }

    // ── cancellation ─────────────────────────────────────────────────────

    #[test]
    fn cancellation_propagates() {
        let mesh = make_test_flat(60.0);
        let index = SpatialIndex::build(&mesh, 10.0);
        let cutter = ball(6.0);
        let params = UnifiedFinishParams::default();
        let planner = FinishPlannerParams::for_tool(cutter.radius());

        // False on the very first check, true on every check after —
        // guarantees at least one check succeeds (so classification can
        // start) but the run cannot complete without observing cancel.
        let already_checked = AtomicBool::new(false);
        let cancel_after_first = || already_checked.swap(true, Ordering::SeqCst);

        let result = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            5.0,
            -5.0,
            &params,
            &planner,
            None,
            None,
            None,
            &cancel_after_first,
        );
        assert!(
            result.is_err(),
            "expected cancellation to propagate as Err(Cancelled)"
        );
    }

    // ── P2.d router ──────────────────────────────────────────────────────

    #[test]
    fn preamble_and_retract_detection() {
        let mut tp = Toolpath::new();
        tp.rapid_to_with_intent(P3::new(1.0, 2.0, 30.0), MoveIntent::Linking);
        tp.feed_to_with_intent(P3::new(1.0, 2.0, 0.5), 500.0, MoveIntent::EntryPlunge);
        tp.feed_to_with_intent(P3::new(5.0, 2.0, 0.4), 1000.0, MoveIntent::FinishingCut);
        tp.feed_to_with_intent(P3::new(9.0, 2.0, 0.3), 1000.0, MoveIntent::FinishingCut);
        tp.rapid_to_with_intent(P3::new(9.0, 2.0, 30.0), MoveIntent::Retract);

        let (k, entry) = strippable_preamble(&tp).expect("canonical preamble");
        assert_eq!(k, 2);
        assert_eq!((entry.x, entry.y, entry.z), (1.0, 2.0, 0.5));

        let (n, exit) = trailing_retracts(&tp);
        assert_eq!(n, 1);
        let exit = exit.expect("exit point");
        assert_eq!((exit.x, exit.y, exit.z), (9.0, 2.0, 0.3));

        // A toolpath opening with a cut (no preamble) is not strippable.
        let mut bare = Toolpath::new();
        bare.feed_to_with_intent(P3::new(0.0, 0.0, 0.0), 1000.0, MoveIntent::FinishingCut);
        assert!(strippable_preamble(&bare).is_none());

        // An unrecognized preamble shape (e.g. a Retract before the first
        // cut) is kept verbatim rather than guessed at.
        let mut odd = Toolpath::new();
        odd.rapid_to_with_intent(P3::new(0.0, 0.0, 30.0), MoveIntent::Retract);
        odd.feed_to_with_intent(P3::new(0.0, 0.0, 0.0), 500.0, MoveIntent::EntryPlunge);
        odd.feed_to_with_intent(P3::new(1.0, 0.0, 0.0), 1000.0, MoveIntent::FinishingCut);
        assert!(strippable_preamble(&odd).is_none());
    }

    fn test_link_kinematics() -> crate::machine_kinematics::LinkKinematics {
        crate::machine_kinematics::LinkKinematics {
            kinematics: crate::machine_kinematics::MachineKinematics::default(),
            max_feed_mm_min: 3000.0,
            rapid_feed_mm_min: 5000.0,
        }
    }

    #[test]
    fn router_orders_regions_and_reports_links() {
        let (mesh, index, cutter, planner) = steep_cone_fixture();
        let params = steep_cone_params();
        let lk = test_link_kinematics();
        let never_cancel = || false;

        let (tp, anns, report) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            35.0,
            0.0,
            &params,
            &planner,
            None,
            Some(&lk),
            None,
            &never_cancel,
        )
        .unwrap();

        assert!(!tp.moves.is_empty());
        assert!(!report.route.is_empty(), "route must list every cut region");
        assert_eq!(
            report.links.len(),
            report.route.len() - 1,
            "one costed junction per consecutive route pair"
        );
        for link in &report.links {
            assert!(
                link.cost_s.is_finite() && link.cost_s >= 0.0,
                "junction cost must be a real integrated time, got {}",
                link.cost_s
            );
            if let Some(alt) = link.alt_cost_s {
                assert!(
                    link.cost_s <= alt,
                    "winning candidate ({:.3}s) must not cost more than the loser ({alt:.3}s)",
                    link.cost_s
                );
            }
        }
        for ann in &anns {
            assert!(
                ann.move_index < tp.moves.len(),
                "annotation move_index {} escaped the stitched toolpath ({} moves)",
                ann.move_index,
                tp.moves.len()
            );
        }
    }

    #[test]
    fn router_is_deterministic() {
        let (mesh, index, cutter, planner) = steep_cone_fixture();
        let params = steep_cone_params();
        let lk = test_link_kinematics();
        let never_cancel = || false;

        let run = || {
            unified_finish_toolpath_with_cancel(
                &mesh,
                &index,
                &cutter,
                35.0,
                0.0,
                &params,
                &planner,
                None,
                Some(&lk),
                None,
                &never_cancel,
            )
            .unwrap()
        };
        let (tp_a, _, report_a) = run();
        let (tp_b, _, report_b) = run();
        assert_eq!(tp_a.moves.len(), tp_b.moves.len());
        assert_eq!(report_a.route, report_b.route);
        assert_eq!(report_a.links.len(), report_b.links.len());
        for (a, b) in report_a.links.iter().zip(report_b.links.iter()) {
            assert_eq!(a.surface, b.surface);
            assert_eq!(a.from_region, b.from_region);
            assert_eq!(a.to_region, b.to_region);
        }
    }

    /// A synthetic region path at `x_center` on a flat surface (z = 0):
    /// the canonical `Linking rapid → EntryPlunge → cuts → Retract` shape
    /// every strategy in this module emits.
    fn synthetic_region(region_index: usize, band: FinishBand, x_center: f64) -> RegionPath {
        let mut tp = Toolpath::new();
        tp.rapid_to_with_intent(P3::new(x_center - 3.0, 0.0, 30.0), MoveIntent::Linking);
        tp.feed_to_with_intent(
            P3::new(x_center - 3.0, 0.0, 0.0),
            500.0,
            MoveIntent::EntryPlunge,
        );
        tp.feed_to_with_intent(
            P3::new(x_center + 3.0, 0.0, 0.0),
            1000.0,
            MoveIntent::FinishingCut,
        );
        tp.rapid_to_with_intent(P3::new(x_center + 3.0, 0.0, 30.0), MoveIntent::Retract);
        let (head_strip, entry) = strippable_preamble(&tp).map(|(k, p)| (k, Some(p))).unwrap();
        let (tail_strip, exit) = trailing_retracts(&tp);
        RegionPath {
            region_index,
            band,
            tp,
            anns: Vec::new(),
            head_strip,
            entry,
            tail_strip,
            exit,
        }
    }

    #[test]
    fn route_greedy_seeds_at_steepest_and_chains_nearest() {
        let mesh = make_test_flat(60.0);
        let index = SpatialIndex::build(&mesh, 10.0);
        let cutter = ball(6.0);
        let params = UnifiedFinishParams::default();
        let lk = test_link_kinematics();
        let never_cancel = || false;

        // Planned order (Shallow first, as `decompose` emits): a Shallow
        // region on each flank, the lone MidSteep in the middle.
        let paths = vec![
            synthetic_region(0, FinishBand::Shallow, -20.0),
            synthetic_region(1, FinishBand::Shallow, 20.0),
            synthetic_region(2, FinishBand::MidSteep, 0.0),
        ];

        let (order, junctions) = route_greedy(
            &paths,
            &mesh,
            &index,
            &cutter,
            &params,
            None,
            &lk,
            &never_cancel,
        )
        .unwrap();

        assert_eq!(
            order.first(),
            Some(&2),
            "route must seed at the steepest band present, got {order:?}"
        );
        // The MidSteep region exits at x = +3: the east flank's entry
        // (x = 17) is 14 mm away, the west flank's (x = -23) is 26 mm —
        // greedy must take the near one first.
        assert_eq!(order, vec![2, 1, 0]);
        assert_eq!(junctions.len(), 2);
        for j in junctions.iter().flatten() {
            assert!(j.cost_s.is_finite() && j.cost_s > 0.0);
        }
    }

    // ── machining boundary ───────────────────────────────────────────────

    #[test]
    fn boundary_restricts_output() {
        let mesh = make_test_flat(60.0);
        let index = SpatialIndex::build(&mesh, 10.0);
        let cutter = ball(6.0);
        let params = UnifiedFinishParams::default();
        let planner = FinishPlannerParams::for_tool(cutter.radius());
        let never_cancel = || false;

        let (unrestricted, _anns, _report) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            5.0,
            -5.0,
            &params,
            &planner,
            None,
            None,
            None,
            &never_cancel,
        )
        .unwrap();

        let square = Polygon2::rectangle(-10.0, -10.0, 10.0, 10.0);
        let regions = vec![square];
        let boundary = RegionSet::from_slice(&regions);

        let (restricted, _anns2, _report2) = unified_finish_toolpath_with_cancel(
            &mesh,
            &index,
            &cutter,
            5.0,
            -5.0,
            &params,
            &planner,
            Some(&boundary),
            None,
            None,
            &never_cancel,
        )
        .unwrap();

        assert!(
            restricted.moves.len() < unrestricted.moves.len(),
            "boundary-restricted run should produce fewer moves ({} vs {})",
            restricted.moves.len(),
            unrestricted.moves.len()
        );

        // Stepover slack at the region edge: a raster row point can sit up
        // to one stepover past the boundary before the run-splitter drops
        // it.
        let margin = params.raster_stepover.max(cutter.radius());
        let mut saw_cut = false;
        for m in &restricted.moves {
            if let crate::toolpath::MoveType::Linear { .. } = m.move_type
                && m.intent == crate::toolpath::MoveIntent::FinishingCut
            {
                saw_cut = true;
                assert!(
                    m.target.x >= -10.0 - margin
                        && m.target.x <= 10.0 + margin
                        && m.target.y >= -10.0 - margin
                        && m.target.y <= 10.0 + margin,
                    "cutting move ({:.2},{:.2}) escaped the boundary square",
                    m.target.x,
                    m.target.y
                );
            }
        }
        assert!(
            saw_cut,
            "expected at least one cutting move inside the boundary"
        );
    }
}
