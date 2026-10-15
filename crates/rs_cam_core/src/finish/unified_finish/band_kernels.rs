//! W6.4.3: the three band sub-kernels of the unified finish, one per
//! [`FinishBand`]. Each one cuts ONE region with an existing strategy
//! generator. The region cut ([`super::UnifiedFinishOp`]'s `cut_region`)
//! calls one of them, then runs the intra-region relink and the height-clip
//! bookkeeping.
//!
//! Moved out of the old orchestrator body unchanged: the same arguments,
//! the same order of work, the same accumulations.

use crate::compute::toolpath_stats::{BandHeightClip, HeightClip};
use crate::finish::finish_planner::{FinishBand, FinishPlannerParams, PlannedRegion};
use crate::finish::finish_setup::{FinishSurface, SLOPE_FILTER_MAX_DEG, SLOPE_FILTER_MIN_DEG};
use crate::finish::scallop::{
    ScallopDirection, ScallopParams, ScallopRuntimeAnnotation,
    scallop_toolpath_structured_annotated_with_cancel,
};
use crate::geometry::region_set::RegionSet;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::ops::waterline::{WaterlineParams, waterline_toolpath_with_cancel, waterline_z_levels};
use crate::surface::dropcutter::DropCutterGrid;
use crate::tool::MillingCutter;
use crate::toolpath::{Toolpath, raster_toolpath_from_grid};
use crate::trace::debug_trace::ToolpathDebugContext;

use super::region_routing::{
    SHALLOW_DERATE_MIN_SLOPE_DEG, band_z_range, build_shallow_raster_grid, region_sampling_window,
    shallow_region_max_slope_deg,
};
use super::{ShallowSlopeDerate, UnifiedFinishParams, UnifiedFinishReport, refine_band_chords};

/// What every band kernel reads: the op inputs and the classifier output.
pub(super) struct BandCtx<'a> {
    pub mesh: &'a TriangleMesh,
    pub index: &'a SpatialIndex,
    pub cutter: &'a dyn MillingCutter,
    pub params: &'a UnifiedFinishParams,
    pub planner: &'a FinishPlannerParams,
    /// The classification surface (true surface, `cusp_quarter`).
    pub surface: &'a FinishSurface,
    /// The covered classification cells, after the boundary and the S4
    /// territory clip.
    pub covered: &'a [bool],
    pub top_z: f64,
    pub bottom_z: f64,
    pub debug: Option<&'a ToolpathDebugContext>,
}

/// One band cut: the moves, the ring labels, and the height clip when the
/// resolved heights narrowed the band's Z range (VerySteep only).
pub(super) struct BandCut {
    pub tp: Toolpath,
    pub anns: Vec<ScallopRuntimeAnnotation>,
    pub height_clip: Option<BandHeightClip>,
}

/// VerySteep: waterline contours over the region's own Z span. `None`: the
/// region has no covered cell inside its own polygon, so it is skipped.
pub(super) fn cut_very_steep(
    b: &BandCtx<'_>,
    region_index: usize,
    region_set: &RegionSet<'_>,
    report: &mut UnifiedFinishReport,
    cancel: &dyn CancelCheck,
) -> Result<Option<BandCut>, Cancelled> {
    let params = b.params;
    let Some((band_min_z, band_max_z)) = band_z_range(b.surface, b.covered, region_set) else {
        tracing::warn!(
            region_index,
            "unified_finish: VerySteep region has no covered cells inside its own polygon; skipping"
        );
        return Ok(None);
    };
    let start_z = band_max_z.min(b.top_z);
    let final_z = band_min_z.max(b.bottom_z);
    // Wave D1 instrument (ledger task #15). `UnifiedFinishConfig`
    // declares `DepthSemantics::None`, so an Auto `bottom_z`
    // resolves to `top_z - 0.0` — the STOCK TOP — and the two
    // `.max`/`.min` clamps above then pin this ladder to the rim
    // of a groove that may be centimetres deep. Compare the
    // levels the band's own surface span asks for against the
    // ones the heights allow; if the difference turns out to
    // have cost the whole region its cutting, say so out loud
    // instead of leaving an unmachined feature with a
    // `region_count += 1` for a headstone.
    let planned_levels = waterline_z_levels(band_max_z, band_min_z, params.z_step).len();
    let resolved_levels = waterline_z_levels(start_z, final_z, params.z_step).len();
    // FIN-14: the comparison happened, whatever it found. The
    // record of WHERE the instrument was taken travels with the
    // finding, so a `MidSteep` or `Shallow` region — whose arm
    // reads neither resolved height — reads as "not measured"
    // and never as a clean band.
    report.height_clip_measured.insert(FinishBand::VerySteep);
    let mut height_clip = None;
    if resolved_levels < planned_levels {
        // Attribute to whichever clamp actually bit. When both
        // did, the floor is the one an operator can act on
        // (a raised `bottom_z` removes the DEEP levels).
        let (clip, clip_z_mm) = if final_z > band_min_z {
            (HeightClip::BottomZ, b.bottom_z)
        } else {
            (HeightClip::TopZ, b.top_z)
        };
        // C8: the BOUNDS travel with the level counts. Counts say
        // how much of the ladder was lost; bounds say where, which
        // is what an operator can act on.
        height_clip = Some(BandHeightClip {
            clip,
            clip_z_mm,
            planned_levels,
            resolved_levels,
            requested_top_z_mm: band_max_z,
            requested_bottom_z_mm: band_min_z,
            delivered_top_z_mm: start_z,
            delivered_bottom_z_mm: final_z,
        });
    }
    let wp = WaterlineParams {
        sampling: params.sampling,
        feed_rate: params.feed_rate,
        plunge_rate: params.plunge_rate,
        safe_z: params.safe_z,
        // D-16.2 (F3): the VerySteep band honours the dial too.
        // Before this the field did not exist on
        // `WaterlineParams` at all, so this arm could not pass
        // one — the twin defect of the Shallow band's.
        stock_to_leave: params.stock_to_leave,
    };
    // `waterline_toolpath_with_cancel` ladders start_z..final_z
    // by `z_step` internally — no need to precompute levels.
    // Per-region z-range: each region only ladders the levels
    // its own surface patch spans.
    let tp = waterline_toolpath_with_cancel(
        b.mesh,
        b.index,
        b.cutter,
        start_z,
        final_z,
        params.z_step,
        &wp,
        Some(region_set),
        cancel,
    )?;
    // G-TIERBURIAL: a contour chord between two fiber crossings is
    // straight; where it cuts below the drop-cutter surface it is split
    // at a point pushed off the wall at the contour's own Z.
    Ok(Some(BandCut {
        tp: refine_band_chords(tp, b.mesh, b.index, b.cutter, params, true),
        anns: Vec::new(),
        height_clip,
    }))
}

/// MidSteep: scallop-continuous rings inside the region.
pub(super) fn cut_mid_steep(
    b: &BandCtx<'_>,
    region_set: &RegionSet<'_>,
    report: &mut UnifiedFinishReport,
    cancel: &dyn CancelCheck,
) -> Result<BandCut, Cancelled> {
    let params = b.params;
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
        // A/M7: deliberately OFF here, and not for lack of
        // value. This band runs `continuous: true` (scallop's
        // own spiral already chains the contours), and whatever
        // junctions survive are relinked one level up by
        // `intra_region_hookup_mm` against the REGION's polygon
        // — the correct boundary for a band that owns only part
        // of the surface. Relinking here as well would decide
        // the same junctions twice, against a weaker boundary.
        intra_pass_hookup_mm: 0.0,
        link_kinematics: None,
    };
    // Generation intentionally uses the ball-center OFFSET
    // surface here (`scallop_toolpath_structured_annotated_
    // with_cancel` builds its own under
    // `scallop::scallop_generation_resolution`, mirrored by
    // `unified_finish_mid_steep_generation_resolution`) — only
    // classification (the classifier) reads the true surface.
    // This is the P2.b "classify true, generate offset" split
    // from the design doc, not an inconsistency.
    let (tp, anns, scallop_report) = scallop_toolpath_structured_annotated_with_cancel(
        b.mesh,
        b.index,
        b.cutter,
        &sp,
        b.debug,
        Some(region_set),
        cancel,
    )?;
    // Summed across every mid-steep region, so a truncated
    // cascade in ANY of them reaches the op's report.
    report.uncut_core_mm2 += scallop_report.uncut_core_mm2;
    // M4 §5b: the hole-aware and estimator siblings, summed the
    // same way.
    report.untouched_mm2 += scallop_report.untouched_mm2;
    report.standing_mm2 += scallop_report.standing_mm2;
    Ok(BandCut {
        tp,
        anns,
        height_clip: None,
    })
}

/// Shallow: the honest raster (slope derate, C2 frame and cells) on the
/// whole-mesh lattice. `shared_grid` is the 0 deg memo every non-rotated,
/// non-derated region shares.
pub(super) fn cut_shallow(
    b: &BandCtx<'_>,
    region_index: usize,
    region: &PlannedRegion,
    region_set: &RegionSet<'_>,
    shared_grid: &mut Option<DropCutterGrid>,
    report: &mut UnifiedFinishReport,
    cancel: &dyn CancelCheck,
) -> Result<BandCut, Cancelled> {
    let (mesh, index, cutter, params) = (b.mesh, b.index, b.cutter, b.params);
    // Honest raster (Track B fix, 2026-09-01, ALWAYS ON): the
    // raster spaces its passes in XY projection, so on a slope
    // theta the achieved SURFACE spacing is s_XY / cos(theta) —
    // up to 1.31x the configured scallop spec at 40°
    // (`planning/honest_raster_2026-09-01/FINDINGS.md`). Derate
    // the effective stepover by cos(theta_max) of THIS region,
    // BEFORE any lattice is built, so the derated value flows
    // identically into the undivided raster and the C2 cell
    // decomposition — one frame, one lattice (§0j).
    //
    // theta_max is the total slope, not the cross-feed slope, so
    // the derate is exact on the worst cross-feed slope and
    // conservative elsewhere. On convex ground contact focusing
    // refunds sec(theta) up to ~18°, so the derate is knowingly
    // conservative there too — accepted for v1 (synthesis §1
    // caveat); no curvature-aware refund is built.
    let theta_max_deg = shallow_region_max_slope_deg(
        b.surface,
        b.covered,
        region_set,
        b.planner.steep_threshold_deg,
    );
    let derated = theta_max_deg > SHALLOW_DERATE_MIN_SLOPE_DEG;
    let step_over_mm = if derated {
        params.raster_stepover * theta_max_deg.to_radians().cos()
    } else {
        // cos(0) = 1: a flat region's lattice is bit-identical
        // to the pre-fix one (it IS the shared memo below).
        params.raster_stepover
    };
    if derated {
        report.shallow_slope_derates.push(ShallowSlopeDerate {
            region_index,
            slope_max_deg: theta_max_deg,
            configured_stepover_mm: params.raster_stepover,
            derated_stepover_mm: step_over_mm,
        });
    }
    // C2: choose this region's working FRAME before any lattice
    // is touched. Cheap — polygon-only — and it decides whether
    // the shared 0° memo can serve this region at all.
    //
    // `None` is the dial-off arm and reproduces the pre-C2 band
    // exactly: shared grid, one raster call, whole region.
    let frame = if params.monotone_cell_decomposition {
        Some(crate::geometry::monotone_cells::region_frame(
            &region.polygon,
            step_over_mm,
        ))
    } else {
        None
    };
    // A gate-passing region gets its OWN whole-mesh lattice at
    // its PCA-minor axis, and it cannot share the memo: the
    // decomposition and the emission must sit on ONE frame
    // (§0j measured the mixed-frame candidate as a cost).
    //
    // The lattice deliberately keeps `batch_drop_cutter`'s
    // whole-mesh origin/phase/step rather than being trimmed to
    // the region bbox — trimming would move the lattice ORIGIN,
    // and §0j priced phase/origin alone at 0.954–0.962×, a cost
    // on its own.
    //
    // What IS clipped to the region is the set of lattice points
    // actually sampled (`region_sampling_window`). Same lattice,
    // same phase, fewer points computed — and fewer points then
    // swept by the decomposition, the membership check and the
    // per-cell rasters, all three of which are O(grid × polygon
    // containment). A whole-mesh lattice per gate-passing region
    // measured ~35 min of generation on the 192-region mt2
    // board against ~2 min dial-off (§7 follow-up 1).
    // A rotated OR derated region gets its OWN lattice: the
    // shared memo is built at the configured stepover and 0°,
    // and a derated region's lattice must carry the derated
    // step. Same whole-mesh origin/phase, region-windowed —
    // the construction the rotated C2 arm already uses.
    let direction_deg = frame.as_ref().map_or(0.0, |f| f.direction_deg);
    let private_grid = if frame.as_ref().is_some_and(|f| f.rotated) || derated {
        Some(build_shallow_raster_grid(
            mesh,
            index,
            cutter,
            params,
            step_over_mm,
            direction_deg,
            Some(region_sampling_window(&region.polygon, step_over_mm)),
            cancel,
        )?)
    } else {
        None
    };
    let grid = if let Some(grid) = private_grid.as_ref() {
        grid
    } else {
        match shared_grid.as_ref() {
            Some(grid) => grid,
            // NO window: this lattice is the memo SHARED by every
            // non-rotated, non-derated Shallow region, so it must
            // cover all of them. Windowing it to whichever region
            // happened to build it first would blind the rest.
            None => shared_grid.insert(build_shallow_raster_grid(
                mesh,
                index,
                cutter,
                params,
                params.raster_stepover,
                0.0,
                None,
                cancel,
            )?),
        }
    };
    let effective_min_z = mesh.bbox.min.z - 0.1 + params.stock_to_leave;
    // The undivided arm, kept as a closure so every C2 fallback
    // path emits the identical pre-C2 geometry rather than a
    // second transcription of it.
    let undivided = |grid: &DropCutterGrid| {
        raster_toolpath_from_grid(
            grid,
            params.feed_rate,
            params.plunge_rate,
            params.safe_z,
            Some(effective_min_z),
            Some(region_set),
        )
    };
    let tp = match frame {
        None => undivided(grid),
        Some(f) => {
            let decomposed = crate::geometry::monotone_cells::lattice_monotone_cells(
                grid,
                &region.polygon,
                effective_min_z,
            );
            check_cancel(cancel)?;
            let totals = report.monotone_cells.get_or_insert_default();
            totals.regions = totals.regions.saturating_add(1);
            if f.rotated {
                totals.regions_rotated = totals.regions_rotated.saturating_add(1);
            }
            if decomposed.cells.is_empty() {
                totals.empty_fallbacks = totals.empty_fallbacks.saturating_add(1);
                tracing::warn!(
                    region_index,
                    topology_cells = decomposed.topology_cells,
                    "unified_finish: monotone decomposition produced no cell \
                     polygons; emitting the undivided raster for this region"
                );
                undivided(grid)
            } else {
                // The REFUSE discipline the rig imposed on every
                // cell arm it costed (`FINDINGS.md` §0g, Stage
                // J). Both sides sit on this same grid, so it is
                // an exact population test, not a proxy — and a
                // cell set that lost a lattice point would be
                // uncut material, so the disagreement arm falls
                // back rather than emitting.
                let mismatches = crate::geometry::monotone_cells::cells_select_same_lattice(
                    grid,
                    &region.polygon,
                    &decomposed.cells,
                    effective_min_z,
                );
                if mismatches > 0 {
                    totals.membership_fallbacks = totals.membership_fallbacks.saturating_add(1);
                    tracing::warn!(
                        region_index,
                        mismatches,
                        cells = decomposed.cells.len(),
                        "unified_finish: monotone cells disagree with the \
                         undivided region on emitted lattice points; emitting \
                         the undivided raster for this region"
                    );
                    undivided(grid)
                } else {
                    totals.cells_emitted =
                        totals.cells_emitted.saturating_add(decomposed.cells.len());
                    // One raster call per cell, on the ONE
                    // shared lattice — §0i's construction. The
                    // relinker downstream then sees cell-shaped
                    // fragments; it keeps `reorder: true`, which
                    // is why no cell TSP is emitted here (§0j
                    // measured a greedy cell order as
                    // byte-identical).
                    let mut out = Toolpath::new();
                    for cell in &decomposed.cells {
                        let cell_set = RegionSet::new(vec![cell.clone()]);
                        let cell_tp = raster_toolpath_from_grid(
                            grid,
                            params.feed_rate,
                            params.plunge_rate,
                            params.safe_z,
                            Some(effective_min_z),
                            Some(&cell_set),
                        );
                        out.moves.extend(cell_tp.moves);
                    }
                    out
                }
            }
        }
    };
    // G-TIERBURIAL: a raster chord between two lattice points (and a
    // row turnaround) is straight; where it cuts below the drop-cutter
    // surface it is split at exact surface points.
    Ok(BandCut {
        tp: refine_band_chords(tp, mesh, index, cutter, params, false),
        anns: Vec::new(),
        height_clip: None,
    })
}
