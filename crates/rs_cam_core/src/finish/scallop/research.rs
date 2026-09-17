//! The scallop research pipeline: the ring-budget seam, the iso-field entry
//! point, and the region-aware research toolpath builder behind them.
//!
//! Split out of `finish/scallop.rs` by P4; every item is unchanged apart
//! from the `use` lines. The parent re-exports each public entry point, so
//! no caller's path changes.

use crate::finish::finish_setup::FinishResolutionPolicy;
use crate::geo::{P2, P3};
use crate::geometry::region_set::RegionSet;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::Polygon2;
use crate::tool::MillingCutter;
use crate::toolpath::{MoveIntent, Toolpath};
use crate::trace::debug_trace::ToolpathDebugContext;

use tracing::info;

use super::ring_generation::{
    closest_kept_point_idx, generate_scallop_rings_with_cancel, rotate_ring,
};
use super::{
    RingSource, ScallopDirection, ScallopParams, ScallopReport, ScallopRuntimeAnnotation,
    ScallopRuntimeEvent, ScallopStepoverPolicy, ScallopStepoverTrace, StepoverGeometry,
};

/// Which stepover the ring cascade's `max_rings` safety cap is budgeted from.
///
/// PR-8c research seam (H3). The shipped budget is
/// [`Self::FlatGroundStepover`] and no production caller passes anything
/// else — every entry point above resolves to it, so this enum adds no
/// behaviour and changes no default. It exists because Checkpoint B's ruling
/// asked for the alternative to be MEASURED before anyone argues about it,
/// and the alternative cannot be measured without a way to select it.
///
/// See `CHECKPOINT_B_EVIDENCE.md` §3.1 finding 3 and the dated
/// "max_rings experiment" addendum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScallopRingBudget {
    /// **Shipped.** `stepover_from_scallop_flat(cusp_r, height)`, floored at
    /// `cusp_r * 0.05` — the WIDEST spacing the cusp target allows, so the
    /// budget under-counts on any sloped ground. The cap then truncates the
    /// cascade and the loop warns.
    FlatGroundStepover,
    /// Budget from [`crate::surface::reach::suggested_offset_stepover_mm`] at the
    /// cutter's own shallow-rest working half-width — the number PR-6a made
    /// canonical for "how far apart may two passes of this cutter sit".
    ///
    /// This is Checkpoint B's "derive the budget from the SELECTED stepover"
    /// read consistently with the reach policy rather than as the loop's
    /// `cusp_r * 0.05` clamp FLOOR, which is the naive cap raise v3 measured
    /// at +92% time and 34× over-cut.
    ReachPolicyStepover,
    /// The naive raise, kept as the CONTROL: budget from the loop's own
    /// clamp floor, the smallest stepover it can ever select. This is what
    /// v3 measured; it is here so the experiment reproduces that result on
    /// the Checkpoint B fixtures instead of citing it.
    LoopClampFloor,
}

/// The iso-field production entry (M8, 2026-09-03) — the first production
/// caller outside [`ScallopStepoverPolicy::SHIPPED`].
///
/// Selects [`RingSource::IsoField`] with [`StepoverGeometry::CosineSlope`]:
/// per-point spacing (no per-ring min-reduction crawl), the SPEC-CORRECT
/// slope law (`scallop_math::variable_stepover`'s inversion does not apply —
/// the historic blocker, the `max_rings` budget, does not exist for the
/// field: ring count is `⌊max D⌋`), and completion by construction (retires
/// the G-SCALLOPBASIN truncation class). The field resolution is
/// [`FinishResolutionPolicy::cusp_quarter`] — field detail scales with the
/// CUSP radius, not the shank; on the R1.5 evidence tool that is 0.375 mm,
/// matching the measured M8b probe. Evidence:
/// `planning/metrology_2026-09-02/FINDINGS.md` §M7–M8.
pub fn scallop_toolpath_iso_field_with_cancel(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ScallopParams,
    debug: Option<&ToolpathDebugContext>,
    boundary_regions: Option<&RegionSet<'_>>,
    cancel: &dyn CancelCheck,
) -> Result<(Toolpath, Vec<ScallopRuntimeAnnotation>, ScallopReport), Cancelled> {
    let policy = ScallopStepoverPolicy {
        ring_source: RingSource::IsoField,
        geometry: StepoverGeometry::CosineSlope,
        ..ScallopStepoverPolicy::SHIPPED
    };
    let (tp, annotations, report, _) = scallop_toolpath_research(
        mesh,
        index,
        cutter,
        params,
        debug,
        boundary_regions,
        FinishResolutionPolicy::cusp_quarter(cutter, params.tolerance),
        // Inert for the field (nothing to truncate); the completing choice
        // is stated anyway so a future cascade fallback cannot silently
        // reintroduce the truncation class.
        ScallopRingBudget::LoopClampFloor,
        policy,
        cancel,
    )?;
    Ok((tp, annotations, report))
}

/// [`scallop_toolpath_structured_annotated_with_resolution`] with the ring
/// budget selected by the caller.
///
/// **Research seam, not a production entry point** (PR-8c). Passing
/// [`ScallopRingBudget::FlatGroundStepover`] reproduces the shipped path
/// exactly — that is what the wrapper above does.
#[allow(clippy::too_many_arguments)]
pub fn scallop_toolpath_structured_annotated_with_resolution_and_ring_budget(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ScallopParams,
    debug: Option<&ToolpathDebugContext>,
    boundary_regions: Option<&RegionSet<'_>>,
    resolution: FinishResolutionPolicy,
    ring_budget: ScallopRingBudget,
    cancel: &dyn CancelCheck,
) -> Result<(Toolpath, Vec<ScallopRuntimeAnnotation>, ScallopReport), Cancelled> {
    let (tp, anns, report, _trace) = scallop_toolpath_research(
        mesh,
        index,
        cutter,
        params,
        debug,
        boundary_regions,
        resolution,
        ring_budget,
        ScallopStepoverPolicy::SHIPPED,
        cancel,
    )?;
    Ok((tp, anns, report))
}

/// The M4 research entry point: ring budget **and** stepover policy selectable,
/// with the cascade's per-iteration decisions returned alongside the toolpath.
///
/// **Research seam, not a production entry point.** Passing
/// [`ScallopRingBudget::FlatGroundStepover`] and
/// [`ScallopStepoverPolicy::SHIPPED`] reproduces the shipped path exactly —
/// that is what the wrapper above does, and
/// `scallop_candidates_m4::shipped_policy_reproduces_the_shipped_fingerprint`
/// asserts it byte for byte.
#[allow(clippy::too_many_arguments)]
pub fn scallop_toolpath_research(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ScallopParams,
    debug: Option<&ToolpathDebugContext>,
    boundary_regions: Option<&RegionSet<'_>>,
    resolution: FinishResolutionPolicy,
    ring_budget: ScallopRingBudget,
    stepover_policy: ScallopStepoverPolicy,
    cancel: &dyn CancelCheck,
) -> Result<
    (
        Toolpath,
        Vec<ScallopRuntimeAnnotation>,
        ScallopReport,
        ScallopStepoverTrace,
    ),
    Cancelled,
> {
    scallop_toolpath_research_with_stage(
        mesh,
        index,
        cutter,
        params,
        debug,
        boundary_regions,
        resolution,
        ring_budget,
        stepover_policy,
        None,
        cancel,
    )
}

/// [`scallop_toolpath_research`] with the shared finishing link stage.
///
/// `link_stage: None` is the legacy intra-pass relink, byte for byte — which
/// is what every research seam and the iso-field entry point pass.
#[allow(clippy::too_many_arguments)]
pub(crate) fn scallop_toolpath_research_with_stage(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    params: &ScallopParams,
    debug: Option<&ToolpathDebugContext>,
    boundary_regions: Option<&RegionSet<'_>>,
    resolution: FinishResolutionPolicy,
    ring_budget: ScallopRingBudget,
    stepover_policy: ScallopStepoverPolicy,
    link_stage: Option<&crate::finish::surface_link::FinishingLinkStage<'_>>,
    cancel: &dyn CancelCheck,
) -> Result<
    (
        Toolpath,
        Vec<ScallopRuntimeAnnotation>,
        ScallopReport,
        ScallopStepoverTrace,
    ),
    Cancelled,
> {
    let mut trace = ScallopStepoverTrace::default();
    check_cancel(cancel)?;
    let mut uncut_core_mm2 = 0.0_f64;
    // M4 §5b: the hole-aware and estimator siblings of `uncut_core_mm2`,
    // accumulated in lockstep with it in the region loop below.
    let mut untouched_mm2 = 0.0_f64;
    let mut standing_mm2 = 0.0_f64;
    // Physical extent (heightmap padding / grid coverage) keeps the FULL
    // tool radius; all cusp/stepover math uses the cusp-forming radius
    // (tip sphere for tapered tools — see `cusp_radius`).
    let tool_radius = cutter.envelope_radius_mm();
    let cusp_r = cutter.cusp_radius_mm();
    let bbox = &mesh.bbox;

    // Build surface heightmap and slope map (shared setup, see finish_setup.rs).
    // The RESOLUTION is scallop's own choice (H3 step 2) — see
    // `scallop_generation_resolution`, which the shipped wrapper above passes
    // in.
    //
    // MEMOISED (`planning/thin_organic_2026-08-27/FINDINGS.md` §1.4). This
    // build is mesh-GLOBAL — one drop-cutter query per grid cell over the whole
    // board — and `unified_finish` calls this function once per REGION, so a
    // tier whose mid-steep band decomposes into k islands used to pay k
    // identical whole-board walks. `cached_finish_surface` returns the SAME
    // `Arc` on a hit, never a recomputed-equal surface; the borrows below read
    // the identical grid the previous region read. The uncached builder is
    // still the one that runs on a miss, so a cached surface and a fresh one
    // cannot diverge.
    let surface = crate::maps::finish_surface_cache::cached_finish_surface(
        mesh, index, cutter, resolution, cancel,
    )?;
    let surface_hm = &surface.heightmap;
    let slope_map = &surface.slope_map;

    // Kept alongside the shared heightmap builder above (which derives the
    // same values internally) because `max_rings` below still needs the raw
    // extent — not just the resulting grid.
    let origin_x = bbox.min.x - tool_radius;
    let origin_y = bbox.min.y - tool_radius;
    let extent_x = bbox.max.x + tool_radius;
    let extent_y = bbox.max.y + tool_radius;

    // Outer boundary: mesh footprint as a rectangle, sampled densely enough
    // for polygon offset to work correctly. Point spacing = stepover.
    let bx0 = bbox.min.x;
    let by0 = bbox.min.y;
    let bx1 = bbox.max.x;
    let by1 = bbox.max.y;
    let flat_so =
        crate::finish::scallop_math::stepover_from_scallop_flat(cusp_r, params.scallop_height)
            .max(cusp_r * 0.1);
    let boundary = {
        let mut pts = Vec::new();
        // Bottom edge
        let mut x = bx0;
        while x < bx1 {
            pts.push(P2::new(x, by0));
            x += flat_so;
        }
        // Right edge
        let mut y = by0;
        while y < by1 {
            pts.push(P2::new(bx1, y));
            y += flat_so;
        }
        // Top edge (reversed)
        let mut x = bx1;
        while x > bx0 {
            pts.push(P2::new(x, by1));
            x -= flat_so;
        }
        // Left edge (reversed)
        let mut y = by1;
        while y > by0 {
            pts.push(P2::new(bx0, y));
            y -= flat_so;
        }
        if pts.len() < 4 {
            // Fallback to simple rectangle
            pts = vec![
                P2::new(bx0, by0),
                P2::new(bx1, by0),
                P2::new(bx1, by1),
                P2::new(bx0, by1),
            ];
        }
        Polygon2::new(pts)
    };

    // Max rings, budgeted from the FLAT-ground stepover — which is the
    // WIDEST spacing the cusp target allows, so this under-counts whenever
    // the terrain has slope. That is a known, deliberate compromise, and
    // the warning at the end of the ring loop reports when it bites.
    //
    // Why it is not simply raised (measured, wanaka ×2, 2026-07-28):
    // budgeting instead from the loop's own `cusp_r * 0.05` clamp floor —
    // the smallest stepover it can select — is correct in principle and
    // does fill the ~28 mm block of standing material this cap was leaving
    // in the middle of the part. But it is far WORSE overall, because the
    // cap was masking a deeper flaw rather than causing one:
    //
    //   * `ring_stepover` takes the MIN across samples on a ring, so a
    //     single steep sample sets the advance for the WHOLE ring. On
    //     terrain every large ring touches steep ground, so the cascade
    //     crawls at the worst-case rate.
    //   * Uncapped, that produced thousands of rings at ~25 µm spacing.
    //     Op B went 38 896 s -> 74 731 s (+92%) while removing LESS
    //     material, and deep over-cut columns went 937 -> 32 221 (34x)
    //     because dense rings mean short chords, and `refine_chord` never
    //     probes a chord shorter than `2 * probe_step`.
    //
    // So the real fix is in `ring_stepover` (per-segment advance instead
    // of min-across-ring, or a physically sensible floor) plus chord
    // refinement — not here. Until then this cap stays, and the loop warns
    // loudly when it truncates so the standing material is a KNOWN defect
    // rather than a silent one.
    //
    // PR-8c: which stepover this is budgeted from is now selectable so the
    // Checkpoint B ruling's alternative can be MEASURED. Production passes
    // `FlatGroundStepover` and the arithmetic below is byte-identical to what
    // it always was.
    let clamp_floor = cusp_r * 0.05;
    let min_stepover = match ring_budget {
        ScallopRingBudget::FlatGroundStepover => {
            crate::finish::scallop_math::stepover_from_scallop_flat(cusp_r, params.scallop_height)
                .max(clamp_floor)
        }
        // The reach policy's own answer for "how far apart may two passes of
        // this cutter sit", evaluated at the shallow-rest depth where its
        // cusp floor binds — the same reference PR-6a uses for a fan it has
        // not measured a depth for yet.
        ScallopRingBudget::ReachPolicyStepover => {
            crate::surface::reach::suggested_offset_stepover_mm(cutter, 0.0).max(clamp_floor)
        }
        ScallopRingBudget::LoopClampFloor => clamp_floor,
    };
    let max_extent = (extent_x - origin_x).max(extent_y - origin_y);
    let max_rings = ((max_extent / min_stepover) * 0.5).ceil() as usize + 10;

    info!(
        max_rings = max_rings,
        min_stepover = format!("{:.3}", min_stepover),
        "Generating scallop rings"
    );

    // P2.3: scan the machining-boundary regions instead of the hardcoded
    // mesh-bbox rectangle when they're available. A single region is used
    // directly as the ring boundary; multiple disjoint regions each get an
    // independent ring set (still bounded by the same `max_rings` safety
    // valve — it's an upper bound on ring count, not an exact prediction, so
    // reusing it per-region is safe). `None`/empty falls back to exactly the
    // one hardcoded-rectangle boundary generated above, so the ring output
    // is byte-identical to pre-P2.3 behavior in that case.
    let region_boundaries: Vec<Polygon2> = match boundary_regions {
        Some(regions) if !regions.is_empty() => regions.as_slice().to_vec(),
        _ => vec![boundary],
    };

    // Generate 3D rings, one region at a time, concatenated in region order.
    let mut rings: Vec<Vec<(P3, bool)>> = Vec::new();
    // C8: which region each ring came from, parallel to `rings`. The ring
    // list is concatenated in region order, so this is contiguous — but it
    // is recorded rather than re-derived, because the direction flip below
    // reverses the list and a re-derivation would have to know that.
    let mut ring_region: Vec<usize> = Vec::new();
    let region_total = region_boundaries.len().max(1);
    for (region_index, region_boundary) in region_boundaries.iter().enumerate() {
        check_cancel(cancel)?;
        let (region_rings, region_metrics) = generate_scallop_rings_with_cancel(
            region_boundary,
            mesh,
            index,
            cutter,
            slope_map,
            surface_hm,
            cusp_r,
            params.scallop_height,
            params.stock_to_leave,
            bbox.min.z,
            max_rings,
            params.tolerance,
            stepover_policy,
            Some(&mut trace),
            cancel,
        )?;
        ring_region.extend(std::iter::repeat_n(region_index, region_rings.len()));
        rings.extend(region_rings);
        uncut_core_mm2 += region_metrics.uncut_core_mm2;
        untouched_mm2 += region_metrics.untouched_mm2;
        standing_mm2 += region_metrics.standing_mm2;
    }

    info!(rings = rings.len(), "Scallop rings generated");

    if rings.is_empty() {
        return Ok((
            Toolpath::new(),
            Vec::new(),
            ScallopReport {
                uncut_core_mm2,
                untouched_mm2,
                standing_mm2,
                cascade_ring_count: 0,
                ring_count: 0,
                // The link stage sits below this early return, so it never
                // ran. `None` = not measured, per the field's contract.
                relink: None,
            },
            trace,
        ));
    }

    // Apply direction. `ring_region` is reversed in lockstep — the whole
    // point of carrying it is that it survives this.
    if matches!(params.direction, ScallopDirection::InsideOut) {
        rings.reverse();
        ring_region.reverse();
    }

    // Slope confinement
    let use_slope_filter =
        crate::finish::finish_setup::slope_filter_active(params.slope_from, params.slope_to);
    let slope_from_rad = params.slope_from.to_radians();
    let slope_to_rad = params.slope_to.to_radians();

    // Combined per-point keep predicate: real mesh coverage (P2.3 bonus
    // fix — see `ring_to_3d`) AND, when active, the slope band AND the
    // machining-boundary regions. Always run points through this (rather
    // than only when a filter is "active") — with every filter a no-op the
    // run-splitter below degenerates to "the whole ring survives as one
    // run", which is byte-identical to the plain unfiltered emission this
    // replaces.
    let region_ok = |p: &P3| -> bool {
        boundary_regions.is_none_or(|regions| regions.contains(&P2::new(p.x, p.y)))
    };
    let keep_point = |pt: &(P3, bool)| -> bool {
        let (p, covered) = pt;
        *covered
            && (!use_slope_filter
                || slope_map
                    .angle_at_world(p.x, p.y)
                    .is_some_and(|a| a >= slope_from_rad && a <= slope_to_rad))
            && region_ok(p)
    };

    // Convert rings to toolpath
    let mut tp = Toolpath::new();
    let mut annotations = Vec::new();
    // G-LINKSTAGE: what each emitted fragment IS, parallel to the fragments
    // `surface_link::relink_fragments` will split back out of `tp`. Built in
    // the SAME loop that emits the moves — a vector derived from
    // `emitted_runs` afterwards would drift the moment a run is skipped, and
    // a mis-aligned kind rotates the wrong fragment. Left empty in the
    // `continuous` branch, which the relink skips.
    let mut fragment_kinds: Vec<crate::finish::surface_link::FragmentKind> = Vec::new();

    if params.continuous && rings.len() >= 2 {
        // Continuous spiral mode: connect adjacent rings at their nearest
        // KEPT points. A ring-to-ring connector is a *cutting* feed only
        // when it is a genuine helical transition — the tool is already
        // down and the hop is no longer than the widest ring spacing the
        // generator can produce. Anything longer — a kept set that
        // shifted to the far side of the ring under the combined keep
        // predicate, or the gap between two disjoint P2.3 boundary regions
        // — gets a retract/rapid/replunge link instead of chording across
        // excluded material at cutting feed (the P0.4 gouge class the
        // no-chord regression tests pin).
        //
        // The bound is RING-SOURCE-AWARE (G-ISOCHANNEL, operator-caught
        // 2026-09-03). The cascade's stepover is clamped at `cusp_r * 3.0`,
        // so that is its genuine widest spacing — byte-identical shipped
        // behaviour. The ISO FIELD spaces rings at the local stepover,
        // which is at most the FLAT-ground stepover anywhere — but its
        // ring LIST is level sets, where consecutive entries can be
        // different loops several mm apart. Under the cascade bound
        // (4.5 mm on the R1.5 evidence tool) those hops chained as
        // "helical" cutting feeds and carved straight channels through
        // standing terrain — the operator saw one in the viewport, and the
        // rapid checker is blind to it because the chord is a FEED. The
        // field bound is 1.5× the flat stepover: a real ring-to-ring hop
        // always passes; a loop-to-loop hop retracts.
        let link_threshold = match stepover_policy.ring_source {
            RingSource::OffsetCascade => cusp_r * 3.0,
            RingSource::IsoField => {
                1.5 * crate::finish::scallop_math::stepover_from_scallop_flat(
                    cusp_r,
                    params.scallop_height,
                )
            }
        };
        // The tool's last emitted position. Seeded from the first ring's
        // geometric end (the pre-existing rotation seed) and updated to the
        // REAL last emitted point after every run — anchoring rotation on
        // the geometric ring end let the connector hop diverge arbitrarily
        // far from where the tool actually stopped.
        // SAFETY: rings.len() >= 2 checked above; rings entries have >= 3 points.
        #[allow(clippy::indexing_slicing)]
        let mut anchor: P3 = rings[0].last().map_or(rings[0][0].0, |&(p, _)| p);
        let mut tool_down = false;

        for (i, ring) in rings.iter().enumerate() {
            check_cancel(cancel)?;
            // Rotate the ring to start at the kept point closest to the
            // tool position. A ring with no kept points emits nothing —
            // pre-P2.3 this still plunged to the ring's rotation start,
            // which for an off-footprint boundary corner meant diving on a
            // rim-contact Z (see `ring_to_3d`).
            let Some(start_idx) = closest_kept_point_idx(ring, &anchor, |pt| keep_point(pt)) else {
                continue;
            };
            let rotated = rotate_ring(ring, start_idx);
            annotations.push(ScallopRuntimeAnnotation {
                move_index: tp.moves.len(),
                event: ScallopRuntimeEvent::Ring {
                    ring_index: i + 1,
                    ring_total: rings.len(),
                    continuous: true,
                    region_index: ring_region.get(i).copied().unwrap_or(0),
                    region_total,
                },
            });

            // Contiguous kept runs across the whole rotated ring (index 0
            // is kept by construction, so the first run starts at 0).
            let idx_runs = crate::geometry::point_runs::split_run_ranges(
                &rotated,
                |_, pt: &(P3, bool)| keep_point(pt),
                1,
            );
            for (run_idx, (s, e)) in idx_runs.into_iter().enumerate() {
                let Some(run) = rotated.get(s..=e) else {
                    continue;
                };
                let Some(&(run_first, _)) = run.first() else {
                    continue;
                };
                let hop =
                    ((run_first.x - anchor.x).powi(2) + (run_first.y - anchor.y).powi(2)).sqrt();
                // Helical transition: only the ring's FIRST run can continue
                // the previous ring's cut, and only when the tool is down
                // and the hop is within one ring spacing.
                let helical_link = run_idx == 0 && s == 0 && tool_down && hop <= link_threshold;
                let mut iter = run.iter();
                if helical_link {
                    if let Some(&(p, _)) = iter.next() {
                        tp.feed_to_with_intent(p, params.feed_rate, MoveIntent::FinishingCut);
                    }
                } else {
                    if tool_down {
                        tp.rapid_to_with_intent(
                            P3::new(anchor.x, anchor.y, params.safe_z),
                            MoveIntent::Retract,
                        );
                        tool_down = false;
                    }
                    if let Some(&(p, _)) = iter.next() {
                        tp.rapid_to_with_intent(
                            P3::new(p.x, p.y, params.safe_z),
                            MoveIntent::Linking,
                        );
                        tp.feed_to_with_intent(p, params.plunge_rate, MoveIntent::EntryPlunge);
                    }
                }
                for &(pt, _) in iter {
                    tp.feed_to_with_intent(pt, params.feed_rate, MoveIntent::FinishingCut);
                }
                if let Some(&(last, _)) = run.last() {
                    anchor = last;
                    tool_down = true;
                }
            }
        }

        // Final retract from wherever the tool actually ended.
        if tool_down {
            tp.rapid_to_with_intent(
                P3::new(anchor.x, anchor.y, params.safe_z),
                MoveIntent::Retract,
            );
        }
    } else {
        // Discrete ring mode: rapid between rings. Each ring is split into
        // contiguous runs that survive the combined keep predicate — a
        // ring is only safe to close back to its own start when EVERY
        // point on it survives (a partial survivor set closing across the
        // excluded gap would chord straight through material this pass
        // must not touch).
        // C8: `(points, close_loop, region_index)` — a ring can split into
        // several emitted runs, and every one of them belongs to the region
        // its parent ring came from.
        let mut emitted_runs: Vec<(Vec<P3>, bool, usize)> = Vec::new();
        for (ring_idx, ring) in rings.iter().enumerate() {
            if ring.len() < 3 {
                continue;
            }
            let region_index = ring_region.get(ring_idx).copied().unwrap_or(0);

            let runs = crate::geometry::point_runs::split_runs(
                ring,
                |_, pt: &(P3, bool)| keep_point(pt),
                crate::geometry::point_runs::RunTopology::Closed,
                3,
            );
            for run in runs {
                let is_closed_loop = run.len() == ring.len();
                let pts: Vec<P3> = run.iter().map(|&(p, _)| p).collect();
                emitted_runs.push((pts, is_closed_loop, region_index));
            }
        }

        for (ring_index, (points, close_loop, region_index)) in emitted_runs.iter().enumerate() {
            check_cancel(cancel)?;
            let Some(&first) = points.first() else {
                continue;
            };
            let move_index = tp.moves.len();
            annotations.push(ScallopRuntimeAnnotation {
                move_index,
                event: ScallopRuntimeEvent::Ring {
                    ring_index: ring_index + 1,
                    ring_total: emitted_runs.len(),
                    continuous: false,
                    region_index: *region_index,
                    region_total,
                },
            });
            // A whole surviving ring closes onto its own start, so the stage
            // may rotate it to begin near the tool. A run the keep predicate
            // split is an open arc: its ends are where the excluded ground
            // begins, and moving them would cut it.
            fragment_kinds.push(if *close_loop {
                crate::finish::surface_link::FragmentKind::ClosedLoop
            } else {
                crate::finish::surface_link::FragmentKind::OpenRun
            });
            tp.rapid_to_with_intent(
                P3::new(first.x, first.y, params.safe_z),
                MoveIntent::Linking,
            );
            tp.feed_to_with_intent(first, params.plunge_rate, MoveIntent::EntryPlunge);
            for pt in points.iter().skip(1) {
                tp.feed_to_with_intent(*pt, params.feed_rate, MoveIntent::FinishingCut);
            }
            let retract_at = if *close_loop {
                // Close the ring: the closing feed brings the cutter back
                // to the first point, so retract from there.
                tp.feed_to_with_intent(first, params.feed_rate, MoveIntent::FinishingCut);
                first
            } else {
                // Open arc: the cutter is at the run's last point — retract
                // there rather than chording back to the run's start.
                points.last().copied().unwrap_or(first)
            };
            tp.rapid_to_with_intent(
                P3::new(retract_at.x, retract_at.y, params.safe_z),
                MoveIntent::Retract,
            );
        }
    }

    if let Some(last) = tp.moves.last()
        && !matches!(last.move_type, crate::toolpath::MoveType::Rapid)
    {
        tp.rapid_to_with_intent(
            P3::new(last.target.x, last.target.y, params.safe_z),
            MoveIntent::Retract,
        );
    }

    // A/M7 — keep the tool DOWN between rings whose ends nearly touch.
    //
    // The discrete branch above emits `retract → rapid → replunge` at EVERY
    // ring junction, unconditionally. `relink_fragments` re-decides each
    // junction on evidence: the link is drop-cutter sampled so it cannot
    // gouge, refused if it would leave `boundary_regions`, and (with
    // kinematics) kept only when it beats the retract on time. Fragment
    // interiors are copied verbatim — only the airborne junctions change.
    //
    // Skipped under `continuous`: spiral mode already chains its contours,
    // so there are no ring-to-ring junctions left to convert.
    //
    // G-LINKVISIBLE: `relink_totals` stays `None` unless the block below
    // runs. That is the "not measured" half of `ScallopReport::relink` —
    // written exactly where the pass is, so the two cannot disagree.
    let mut relink_totals: Option<crate::finish::unified_finish::RelinkTotals> = None;
    if params.intra_pass_hookup_mm > 0.0 && !params.continuous {
        // G-LINKSTAGE. Two configurations, and the caller picks by handing a
        // stage or not.
        //
        // With a stage (the shipped contour scallop): the ONE finishing
        // configuration, built in `FinishingLinkStage::params` — `reorder`
        // and loop rotation, which is what turns a breadth-first ring list
        // into a candidate set at all (of 600 wanaka ring junctions the
        // legacy arm rejected 493 as `too_far` and ZERO on the surface or
        // kinematics tests), plus the stock ceiling, whose absence here was
        // the defect: on a `FromRemainingStock` island pass a surface-riding
        // link rides the MESH, which sits BELOW the standing material, so the
        // link is a lateral cutting feed through rest stock — the
        // G-ISOCLIPRAPID shape — and no lifted hop was ever reachable.
        //
        // Without one (the iso field, every research seam, every existing
        // test): the legacy literal below, byte for byte.
        let geom = crate::finish::surface_link::LinkGeometry {
            stock_to_leave: params.stock_to_leave,
            sampling: params.tolerance.max(0.01),
            feed_rate: params.feed_rate,
            plunge_rate: params.plunge_rate,
            safe_z: params.safe_z,
        };
        let legacy = crate::finish::surface_link::RelinkParams {
            hookup_distance: params.intra_pass_hookup_mm,
            stock_to_leave: geom.stock_to_leave,
            sampling: geom.sampling,
            feed_rate: geom.feed_rate,
            plunge_rate: geom.plunge_rate,
            safe_z: geom.safe_z,
            link_kinematics: params.link_kinematics.as_ref(),
            // Rings are emitted outside-in (or inside-out) and are already
            // in a sane order; reordering them would trade a solved problem
            // for climb/conventional churn. Linking only.
            reorder: false,
            // A ring-to-ring link that leaves the op's territory machines
            // ground the boundary deliberately excluded — the same
            // selective-finishing gouge class `RelinkParams::boundary`
            // documents. On a dendritic rest island a straight line between
            // two rings of the SAME region leaves that region constantly.
            boundary: boundary_regions,
            // Scallop is a FINISHING pass: everything above the mesh has
            // already been cleared, so the mesh IS the material and a link
            // that rides it is riding the workpiece. `None` keeps that
            // behaviour byte-identical.
            link_ceiling: None,
            flush_ride: false,
            // Inert while `link_ceiling` is `None` (the exemption is
            // conjunctive), and `false` is the conservative value regardless:
            // every link this pass emits rides the surface and is a cutting
            // feed, so the territory veto stands.
            airborne_links_may_leave_territory: false,
        };
        // The ON/OFF gate above is `intra_pass_hookup_mm`, and a stage carries
        // its own copy of the same number (the adapter builds it from the same
        // config field, and `finishing_link_stage` returns `None` at `0.0`),
        // so the two cannot disagree about whether the pass runs.
        let (rp, kinds) = match link_stage {
            Some(stage) => (stage.params(&geom), Some(fragment_kinds.as_slice())),
            None => (legacy, None),
        };
        let (linked, rep) = crate::finish::surface_link::relink_fragments_with_kinds(
            crate::trace::toolpath_spans::AnnotatedToolpath::new(tp),
            mesh,
            index,
            cutter,
            &rp,
            kinds,
        );
        info!(
            staged = link_stage.is_some(),
            fragments = rep.fragments,
            surface_links = rep.surface_links,
            // The acceptance measure: only an AT-DEPTH link removes the next
            // fragment's entry. A clearance hop removes the retract and
            // leaves the entry standing.
            at_depth_links = rep.at_depth_links,
            clearance_hops = rep.clearance_hops,
            rotated_loops = rep.rotated_loops,
            retract_links = rep.retract_links,
            too_far = rep.too_far,
            off_surface = rep.off_surface,
            slower_than_retract = rep.slower_than_retract,
            outside_boundary = rep.outside_boundary,
            ceiling_above_safe_z = rep.ceiling_above_safe_z,
            "Scallop intra-pass relink"
        );
        // G-LINKVISIBLE: the same counters the line above logs, on their way
        // to `ToolpathStats::relink`. Recorded even when every one is zero —
        // "the stage ran and had no junction to act on" is a different
        // answer from "the stage never ran".
        let mut totals = crate::finish::unified_finish::RelinkTotals::default();
        totals.add(&rep);
        relink_totals = Some(totals);
        // C1: the ring annotations are this site's index-carrying channel,
        // so they are declared and the type system carries them across —
        // no hand-rolled `old -> new` lookup.
        tp = {
            let mut channels =
                crate::trace::transform_provenance::ReconcileSet::new(None, Some(&mut annotations));
            linked.reconcile(&mut channels).into_inner().toolpath
        };
    }

    info!(
        moves = tp.moves.len(),
        cutting_mm = format!("{:.1}", tp.total_cutting_distance()),
        rapid_mm = format!("{:.1}", tp.total_rapid_distance()),
        "Scallop toolpath complete"
    );

    if let Some(debug_ctx) = debug {
        for annotation in &annotations {
            debug_ctx.add_annotation(annotation.move_index, annotation.event.label());
        }
    }

    let report = ScallopReport {
        uncut_core_mm2,
        untouched_mm2,
        standing_mm2,
        cascade_ring_count: rings.len(),
        // One annotation per emitted ring / kept run, in both the continuous
        // and the discrete branch — so this IS the emitted count, not a
        // proxy for it.
        ring_count: annotations.len(),
        relink: relink_totals,
    };
    Ok((tp, annotations, report, trace))
}
