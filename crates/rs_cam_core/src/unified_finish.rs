//! P2.c of the unified finishing pass
//! (`planning/unified_finish_planner_design.md`): the per-band generation
//! orchestrator.
//!
//! [`unified_finish_toolpath_with_cancel`] composes the P2.b decomposition
//! ([`crate::finish_planner::decompose`]) with three EXISTING region-scoped
//! strategy generators — one call per band, each fed a multi-polygon
//! [`RegionSet`] (per-island fan-out is the strategy's own job, exactly as
//! it already is for `scallop`/`waterline`/raster's `boundary_regions`
//! plumbing today). Bands are concatenated in a naive, fixed STEEP-FIRST
//! order — VerySteep, then MidSteep, then Shallow — the established
//! safer-tool-condition ordering `steep_shallow` already uses (steep,
//! harder-to-reach material is machined while the tool is freshest).
//! Routing WITHIN and BETWEEN bands (nearest-region chaining, 2-opt) is
//! P2.d; this module only concatenates.
//!
//! Creases stay with the standalone pencil op at this checkpoint —
//! [`crate::finish_planner::decompose`] is called with an empty crease
//! slice here on purpose. P2.c isolates the band machinery (build order's
//! A/B checkpoint #1); crease routing folds in later.
//!
//! No dressups, no boundary clipping here: the stitched toolpath this
//! module returns flows through the NORMAL session post-passes (boundary
//! clip, `optimize_entry_descents`, feed modulation, F-034 accounting)
//! exactly like every other op's raw generator output.

use crate::debug_trace::ToolpathDebugContext;
use crate::dropcutter::batch_drop_cutter_with_cancel;
use crate::finish_planner::{FinishBand, FinishPlannerParams, decompose};
use crate::finish_setup::{
    FinishSurface, SLOPE_FILTER_MAX_DEG, SLOPE_FILTER_MIN_DEG,
    build_classification_surface_with_cancel,
};
use crate::geo::P2;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::region_set::RegionSet;
use crate::scallop::{
    ScallopDirection, ScallopParams, ScallopRuntimeAnnotation,
    scallop_toolpath_structured_annotated_with_cancel,
};
use crate::tool::MillingCutter;
use crate::toolpath::{Toolpath, raster_toolpath_from_grid};
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

/// Orchestration report: decomposition stats + what each band generated.
#[derive(Debug, Clone, Default)]
pub struct UnifiedFinishReport {
    pub decompose: crate::finish_planner::DecomposeStats,
    pub very_steep: BandGenStats,
    pub mid_steep: BandGenStats,
    pub shallow: BandGenStats,
}

// ── Orchestrator ─────────────────────────────────────────────────────────

/// Decompose the surface into bands and generate each band's toolpath with
/// the appropriate EXISTING strategy generator, then concatenate
/// steep-first (VerySteep → MidSteep → Shallow). No routing: this is P2.c's
/// naive concatenation checkpoint, not the P2.d router.
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

    // ── Step 4: group planned polygons per band ─────────────────────────
    let mut very_steep_polys = Vec::new();
    let mut mid_steep_polys = Vec::new();
    let mut shallow_polys = Vec::new();
    for region in &planned.regions {
        match region.band {
            FinishBand::VerySteep => very_steep_polys.push(region.polygon.clone()),
            FinishBand::MidSteep => mid_steep_polys.push(region.polygon.clone()),
            FinishBand::Shallow => shallow_polys.push(region.polygon.clone()),
        }
    }

    let mut stitched = Toolpath::new();
    let mut annotations: Vec<ScallopRuntimeAnnotation> = Vec::new();
    let mut report = UnifiedFinishReport {
        decompose: planned.stats,
        ..UnifiedFinishReport::default()
    };

    // ── Step 5: VerySteep → waterline (steep-first) ─────────────────────
    if !very_steep_polys.is_empty() {
        check_cancel(cancel)?;
        let region_count = very_steep_polys.len();
        let region_set = RegionSet::new(very_steep_polys);
        if let Some((band_min_z, band_max_z)) = band_z_range(&surface, &covered, &region_set) {
            let start_z = band_max_z.min(top_z);
            let final_z = band_min_z.max(bottom_z);
            let wp = WaterlineParams {
                sampling: params.sampling,
                feed_rate: params.feed_rate,
                plunge_rate: params.plunge_rate,
                safe_z: params.safe_z,
            };
            // `waterline_toolpath_with_cancel` ladders start_z..final_z by
            // `z_step` internally — no need to precompute levels here.
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
            report.very_steep = BandGenStats {
                region_count,
                move_count: tp.moves.len(),
            };
            stitched.moves.extend(tp.moves);
        } else {
            tracing::warn!(
                region_count,
                "unified_finish: VerySteep band has no covered cells inside its own polygon(s); skipping"
            );
        }
    }

    // ── Step 6: MidSteep → scallop-continuous rings ─────────────────────
    if !mid_steep_polys.is_empty() {
        check_cancel(cancel)?;
        let region_count = mid_steep_polys.len();
        let region_set = RegionSet::new(mid_steep_polys);
        let sp = ScallopParams {
            scallop_height: params.scallop_height,
            tolerance: params.tolerance,
            direction: ScallopDirection::default(),
            // Design decision #1: scallop-continuous rings for the
            // mid-steep band.
            continuous: true,
            // Full slope window: the REGION is the confinement now (the
            // band's polygon already IS the slope-selection), so scallop's
            // own slope_from/slope_to filter is deliberately a no-op — see
            // `finish_setup::{SLOPE_FILTER_MIN_DEG, SLOPE_FILTER_MAX_DEG}`.
            slope_from: SLOPE_FILTER_MIN_DEG,
            slope_to: SLOPE_FILTER_MAX_DEG,
            feed_rate: params.feed_rate,
            plunge_rate: params.plunge_rate,
            safe_z: params.safe_z,
            stock_to_leave: params.stock_to_leave,
        };
        // Generation intentionally uses the ball-center OFFSET surface here
        // (`scallop_toolpath_structured_annotated_with_cancel` builds its
        // own via `finish_setup::build_finish_surface_with_cancel`
        // internally) — only classification (step 1, above) reads the true
        // surface. This is the P2.b "classify true, generate offset" split
        // from the design doc, not an inconsistency.
        let (tp, anns) = scallop_toolpath_structured_annotated_with_cancel(
            mesh,
            index,
            cutter,
            &sp,
            debug,
            Some(&region_set),
            cancel,
        )?;
        // Annotation `move_index` is local to this band's own toolpath;
        // shift by however many moves are already stitched (VerySteep's,
        // if any) so it indexes into the FINAL concatenated toolpath.
        let offset = stitched.moves.len();
        annotations.extend(anns.into_iter().map(|a| ScallopRuntimeAnnotation {
            move_index: a.move_index + offset,
            event: a.event,
        }));
        report.mid_steep = BandGenStats {
            region_count,
            move_count: tp.moves.len(),
        };
        stitched.moves.extend(tp.moves);
    }

    // ── Step 7: Shallow → parallel raster ────────────────────────────────
    if !shallow_polys.is_empty() {
        check_cancel(cancel)?;
        let region_count = shallow_polys.len();
        let region_set = RegionSet::new(shallow_polys);
        // Mesh-bottom floor, mirroring `generate_drop_cutter`'s
        // `effective_min_z` (compute::execute.rs). The stock-bbox floor
        // (`ctx.stock_bbox.min.z - 1.0` there) is the op-adapter's job in
        // wave 2 — this pure-core function only sees a mesh, not a stock
        // model, so wave-2 callers needing that extra floor should pre-max
        // it into `bottom_z` before calling in.
        let effective_min_z = mesh.bbox.min.z - 0.1;
        // `batch_drop_cutter_with_cancel` requires `&(dyn CancelCheck +
        // Sync)` for its rayon closures; this function only receives a
        // plain `&dyn CancelCheck`, matching every finish-op call site up
        // this chain (see the identical constraint documented on
        // `slope::SurfaceHeightmap::from_mesh_with_cancel`). Widening our
        // own signature to `+ Sync` would ripple through every future
        // caller for the sake of one internal call, so this band checks
        // cancellation immediately before and after the batch call instead
        // of threading `cancel` through it.
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
        // Drop grid points whose vertical ray misses every triangle in the
        // mesh. Replicated from `compute::execute::generate_drop_cutter`'s
        // identical guard: `point_drop_cutter` marks a point contacted
        // whenever the cutter (which has radius) touches ANY nearby
        // triangle — including the rim of a mesh that doesn't cover that
        // XY. Without this check the tool rides the edge and carves a
        // trench around the part.
        for pt in &mut grid.points {
            let mut over = false;
            for &tri_idx in &index.query(pt.x, pt.y, 0.0) {
                // SAFETY: tri_idx comes from `index.query`, which only ever
                // returns indices into `mesh.faces` (mirrors
                // `generate_drop_cutter`'s identical loop in
                // compute::execute.rs).
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
        let tp = raster_toolpath_from_grid(
            &grid,
            params.feed_rate,
            params.plunge_rate,
            params.safe_z,
            Some(effective_min_z),
            Some(&region_set),
        );
        report.shallow = BandGenStats {
            region_count,
            move_count: tp.moves.len(),
        };
        stitched.moves.extend(tp.moves);
    }

    Ok((stitched, annotations, report))
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
            &cancel_after_first,
        );
        assert!(
            result.is_err(),
            "expected cancellation to propagate as Err(Cancelled)"
        );
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
