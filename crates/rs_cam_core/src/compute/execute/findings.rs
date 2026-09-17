//! The `record_*` writers for [`super::GenerationFindings`].
//!
//! Each one records a fact an operation learned about the geometry while it
//! generated — a truncated core, a dropped band, a clamped ramp — into the
//! execution context's findings cell. Split out of `compute/execute.rs` (P4).

use super::GenerationFindings;

/// Record one cascade's residual on the context's findings cell,
/// accumulating over the several cascades a single operation can run (one
/// per region; one per mid-steep band in a `UnifiedFinish`).
///
/// The first call is what turns "not measured" into "measured" — including
/// when the measurement is zero, which is the distinction A/M9 exists to
/// preserve. Only call it from an adapter that actually ran a cascade.
///
/// M4 §5b: takes all three `ScallopReport` area figures in one call —
/// `uncut_core_mm2` (continuity), `untouched_mm2` (hole-aware) and
/// `standing_mm2` (the dropped-point estimate) — because every call site
/// already has a whole `ScallopReport`/`UnifiedFinishReport` in hand and the
/// three always travel together; three separate `record_*` calls per site
/// would only invite one being forgotten when a fourth cascade figure shows
/// up later.
pub(super) fn record_truncated_core(
    cell: &std::cell::RefCell<GenerationFindings>,
    uncut_core_mm2: f64,
    untouched_mm2: f64,
    standing_mm2: f64,
) {
    let mut findings = cell.borrow_mut();
    let prev = findings.truncated_core_mm2.unwrap_or(0.0);
    findings.truncated_core_mm2 = Some(prev + uncut_core_mm2);
    let prev_untouched = findings.untouched_material_mm2.unwrap_or(0.0);
    findings.untouched_material_mm2 = Some(prev_untouched + untouched_mm2);
    let prev_standing_estimate = findings.reached_uncut_estimate_mm2.unwrap_or(0.0);
    findings.reached_uncut_estimate_mm2 = Some(prev_standing_estimate + standing_mm2);
}

/// Record ONLY the continuity residual, for a cascade that has no hole-aware
/// or estimator sibling to report (Checkpoint C, Q3).
///
/// Pocket's cascade hits this: when a bound stops it, the standing ring area
/// is exactly what
/// [`crate::compute::toolpath_stats::ToolpathStats::truncated_core_mm2`] means, but
/// pocket computes no `untouched_mm2` and no `standing_mm2`. Going through
/// [`record_truncated_core`] with two zeroes would publish "measured zero" for
/// two measures nobody took — the silent-zero trap X-19 exists to prevent —
/// so those two stay `None`.
pub(super) fn record_truncated_core_only(
    cell: &std::cell::RefCell<GenerationFindings>,
    uncut_core_mm2: f64,
) {
    let mut findings = cell.borrow_mut();
    let prev = findings.truncated_core_mm2.unwrap_or(0.0);
    findings.truncated_core_mm2 = Some(prev + uncut_core_mm2);
}

/// Record a dropped-band finding (Wave D1). `None` is a no-op — an adapter
/// that planned bands and dropped none must not overwrite an earlier
/// finding with an absence.
pub(super) fn record_dropped_band(
    cell: &std::cell::RefCell<GenerationFindings>,
    finding: Option<crate::compute::toolpath_stats::DroppedBandFinding>,
) {
    let Some(finding) = finding else { return };
    cell.borrow_mut().dropped_band = Some(finding);
}

/// Record a partial band clip (C8). `None` is a no-op, for the same reason
/// [`record_dropped_band`]'s is: an adapter that planned bands and clipped
/// none must not overwrite an earlier finding with an absence.
pub(super) fn record_clipped_band(
    cell: &std::cell::RefCell<GenerationFindings>,
    finding: Option<crate::compute::toolpath_stats::ClippedBandFinding>,
) {
    let Some(finding) = finding else { return };
    cell.borrow_mut().clipped_band = Some(finding);
}

/// Record the centreline tip-float tally (Wave D1). Unlike the two above
/// this one records a MEASUREMENT, not a defect: `Some` with zero floating
/// points is the honest "a centreline pass ran and nothing floated", and it
/// is exactly what stops a later reader from reading silence as clean.
pub(super) fn record_tip_float(
    cell: &std::cell::RefCell<GenerationFindings>,
    finding: crate::compute::toolpath_stats::TipFloatFinding,
) {
    let mut findings = cell.borrow_mut();
    let mut merged = findings.tip_float.unwrap_or_default();
    merged.merge(finding);
    findings.tip_float = Some(merged);
}

/// Record what the ramp-finish reach clamp did (PR-8b).
///
/// Like [`record_tip_float`], and unlike a defect recorder, this
/// records a MEASUREMENT, not only a defect: an inert clamp is the honest
/// "a descent ran and every commanded depth was holdable", and it is what
/// stops a later reader from reading silence as clean. The diagnostic
/// adapter is what stays quiet when nothing moved.
pub(super) fn record_ramp_reach_clamp(
    cell: &std::cell::RefCell<GenerationFindings>,
    finding: crate::finish::ramp_finish::RampReachClamp,
) {
    cell.borrow_mut().ramp_reach_clamp = Some(finding);
}

/// Record which rest reference the claims pipeline resolved to (A/M6).
///
/// Records a DECISION, not a defect, and is deliberately never suppressed:
/// even the uneventful outcomes say which of two fields the detector read,
/// and that is not derivable from any config field or from the emitted
/// moves. The diagnostic adapter is what decides how loud to be.
pub(super) fn record_claims_reference(
    cell: &std::cell::RefCell<GenerationFindings>,
    finding: crate::compute::toolpath_stats::ClaimsReferenceFinding,
) {
    cell.borrow_mut().claims_reference = Some(finding);
}

/// F4: record that a non-default rest-claims dial steered nothing.
///
/// A no-op at the defaults: an operator who never touched these has
/// nothing to be told, and a
/// notice on every unified-finish toolpath is a notice nobody reads. The C2
/// keeper — `min_rest_depth_mm = 0.02`, `claims_reference = auto`,
/// `territory_clip = false` — is silent by that rule, and must stay silent.
///
/// **Emitting this changes no geometry.** It is recorded from the config
/// alone, before anything is planned, and nothing downstream branches on it.
/// The dial stays inert deliberately — see
/// [`crate::compute::toolpath_stats::InertClaimsDialFinding`] for why applying it or
/// refusing would both break shipped projects.
///
/// The per-dial inertness rules are the finding's, restated here as the
/// construction: `min_rest_depth_mm` needs only `territory_clip == false`
/// (the S4 mask-AND is its sole consumer); `claims_reference` additionally
/// needs `pencil_claims == false`, because with claims running it still
/// chooses which field the crease detector reads.
pub(super) fn record_inert_claims_dial(
    cell: &std::cell::RefCell<GenerationFindings>,
    cfg: &crate::compute::operation_configs::UnifiedFinishConfig,
) {
    if cfg.territory_clip {
        return;
    }
    let defaults = crate::compute::operation_configs::UnifiedFinishConfig::default();
    let min_rest_depth_inert = (cfg.min_rest_depth_mm - defaults.min_rest_depth_mm).abs() > 1e-9;
    let claims_reference_inert =
        !cfg.pencil_claims && cfg.claims_reference != defaults.claims_reference;
    if !min_rest_depth_inert && !claims_reference_inert {
        return;
    }
    cell.borrow_mut().inert_claims_dial =
        Some(crate::compute::toolpath_stats::InertClaimsDialFinding {
            min_rest_depth_mm: cfg.min_rest_depth_mm,
            default_min_rest_depth_mm: defaults.min_rest_depth_mm,
            min_rest_depth_inert,
            claims_reference: cfg.claims_reference,
            claims_reference_inert,
            pencil_claims: cfg.pencil_claims,
        });
}

/// F3: record what the [`crate::geometry::region_mask::MAX_REST_REGIONS`] cap did to a
/// rest-region extraction.
///
/// **Call this even when nothing was truncated.** Like
/// [`record_offset_library_failures`] and unlike the defect-only recorders,
/// this is a MEASUREMENT: `Some` with `truncated() == false` is the honest
/// "an extraction ran and kept everything it found", and it is what stops a
/// later reader taking silence for a complete island list. Only call it from
/// a path that actually ran an extraction.
pub(super) fn record_region_cap(
    cell: &std::cell::RefCell<GenerationFindings>,
    report: crate::geometry::region_mask::RegionCapReport,
) {
    cell.borrow_mut().region_cap = Some(report);
}

/// Record what a finishing link stage did (Phase O item 3; widened to the
/// whole finishing family by G-LINKVISIBLE).
///
/// **Call this even when every counter is zero.** Like
/// [`record_region_cap`], this is a MEASUREMENT, not a defect report: the
/// question it answers is *why* junctions retracted, and "the pass ran and
/// had no junction to act on" is a different answer from "the pass never
/// ran". Only call it from a path that actually ran the relink.
///
/// Four families reach here, each through its own hookup dial:
/// `unified_finish` (`intra_region_hookup_mm`), `scallop`
/// (`intra_pass_hookup_mm`), `drop_cutter` and `waterline` (`hookup_mm`).
/// They share the slot because they share the kernel — every one of them
/// sums a [`crate::finish::surface_link::RelinkReport`] — and one toolpath is one
/// operation, so which dial produced a reading is never ambiguous. The
/// pencil runs a DIFFERENT linker with a different counter set and has its
/// own slot ([`record_pencil_link`]).
pub(super) fn record_relink_totals(
    cell: &std::cell::RefCell<GenerationFindings>,
    totals: crate::finish::unified_finish::RelinkTotals,
) {
    cell.borrow_mut().relink = Some(totals);
}

/// Record what the C2 shallow-band monotone-cell decomposition did.
///
/// **Only call this when the pass actually ran.** The generator returns
/// `None` unless `monotone_cell_decomposition` was on AND at least one
/// Shallow region existed, and that `None` is the honest "not measured" —
/// coercing it to a zeroed struct here would claim a clean measurement of a
/// pass that never happened, which is the exact reading the
/// [`crate::compute::toolpath_stats::ToolpathStats`] contract forbids.
pub(super) fn record_monotone_cells(
    cell: &std::cell::RefCell<GenerationFindings>,
    totals: crate::finish::unified_finish::MonotoneCellTotals,
) {
    cell.borrow_mut().monotone_cells = Some(totals);
}

/// Record what the PENCIL's link stage did (G-LINKVISIBLE).
///
/// Separate from [`record_relink_totals`] because the pencil runs its own
/// linker, whose report carries eight counters against
/// [`crate::finish::unified_finish::RelinkTotals`]' six — including `hop_too_far`
/// and its own at-depth/hop split, which is exactly the pair that names this
/// pass's binding constraint. Folding it into the shared shape would drop
/// them, and a measurement squeezed into another measurement's shape reads
/// clean and means something else.
///
/// **Call this even when every counter is zero**, for the same reason
/// [`record_relink_totals`] says so.
pub(super) fn record_pencil_link(
    cell: &std::cell::RefCell<GenerationFindings>,
    report: crate::finish::pencil::PencilLinkReport,
) {
    cell.borrow_mut().pencil_link = Some(report);
}

/// Record how many 2D offset calls this generation made that came back with
/// a [`crate::polygon::OffsetFailure`] (Checkpoint C, Q1 / D-2).
///
/// **Call this even when the count is zero.** That is what turns "not
/// measured" into "measured clean", and the whole point of the slot is that
/// those are different answers — an operation that runs no offsets at all
/// must keep reading `None`. Only call it from an adapter that actually
/// routed its offsets through
/// [`crate::polygon::offset_polygon_reported`] (or a `_reported` sibling);
/// an adapter still on the plain name has measured nothing and must not
/// claim a zero.
///
/// Accumulates, like `record_cascade_residual`: an operation offsets once
/// per polygon per Z level and each of those is a separate opportunity to
/// fail.
pub(crate) fn record_offset_library_failures(
    cell: &std::cell::RefCell<GenerationFindings>,
    failures: usize,
) {
    let mut findings = cell.borrow_mut();
    findings.offset_library_failures =
        Some(findings.offset_library_failures.unwrap_or(0) + failures);
}

/// Record that a machining-boundary containment collapsed and the clip was
/// not applied (Checkpoint C, Q2).
///
/// Takes `&mut GenerationFindings`, not the `RefCell`: the boundary clip runs
/// after the adapter returned, at a point where both writers own the findings
/// outright.
pub fn record_boundary_clip_dropped(
    findings: &mut GenerationFindings,
    finding: crate::compute::toolpath_stats::BoundaryClipDroppedFinding,
) {
    findings.boundary_clip_dropped = Some(finding);
}

/// The engagement (mm) at or below which a pass is reported as removing
/// nothing (A4) — derived from the REFERENCE's own resolution, not dialled.
///
/// A reference stock is a sampled surface, and a pass riding exactly on
/// ground it already cut still measures a little material above the cutter:
/// the grid snaps each lookup to the nearest ray, and the simulation that
/// built the surface stamped the tool at a finite spacing along its path.
/// Both artefacts have the same shape as a cusp, so the floor is one:
/// `cell² / (2 · tip radius)` — the height of the sampling residual the
/// reference itself can manufacture.
///
/// This is not a tuned number. It was found by the A4 sentry FAILING: the
/// naive tip-vs-top comparison read +21 µm on a pass that removed nothing,
/// and comparing against the cutter's own profile only brought it to
/// +18 µm. A fixed 10 µm floor would have declared that pass "engaged" for
/// the rest of time, which is the one error this report must not make.
///
/// Floored at 1 µm so a flat cutter (no tip sphere) cannot produce an
/// infinite or negative threshold.
fn zero_removal_engagement_floor_mm(
    stock: &crate::dexel_stock::TriDexelStock,
    cutter: &dyn crate::tool::MillingCutter,
) -> f64 {
    let cell = stock.z_grid.cell_size;
    let tip_r = cutter.cusp_radius();
    if tip_r <= 0.0 {
        return 1.0e-3;
    }
    (cell * cell / (2.0 * tip_r)).max(1.0e-3)
}

/// A4: measure the emitted cutting geometry against the reference stock the
/// pass was planned on, and record a finding when it reaches nothing.
///
/// Only called where a rest pass resolved a REAL machined-stock reference —
/// under any other reference the op is not a rest pass in the sense the
/// finding is about, and the question "what did the prior op leave" has no
/// answer in scope.
///
/// Measured PRE-dressup, on the geometry the planner emitted. The air-cut
/// filter that runs later deletes moves that are wholly in air, and a pass
/// riding exactly on the surface it already cut is not in air by that test
/// — it survives, which is precisely how §3.2's rest pass came to spend
/// 1 294 mm and 48 retract trips on nothing.
pub(super) fn record_zero_removal(
    cell: &std::cell::RefCell<GenerationFindings>,
    toolpath: &crate::toolpath::Toolpath,
    stock: &crate::dexel_stock::TriDexelStock,
    cutter: &dyn crate::tool::MillingCutter,
) {
    let engagement = crate::dressup::reference_engagement_of_cutting_moves(toolpath, stock, cutter);
    // Nothing sampled = nothing measured. Not a finding (X-19's rule): an
    // absent measurement is not a defect claim.
    if engagement.sampled_positions == 0 {
        return;
    }
    let floor_mm = zero_removal_engagement_floor_mm(stock, cutter);
    if engagement.deepest_mm > floor_mm {
        return;
    }
    cell.borrow_mut().zero_removal = Some(crate::compute::toolpath_stats::ZeroRemovalFinding {
        deepest_engagement_mm: engagement.deepest_mm,
        sampled_positions: engagement.sampled_positions,
        cutting_distance_mm: toolpath.total_cutting_distance(),
        floor_mm,
    });
}

/// Record an offset stepover an operation derived from the reach policy
/// (PR-6a, H2.3).
///
/// Unlike a defect recorder this is NOT suppressed at the
/// no-change case here — the finding carries both numbers and the reader
/// decides. The diagnostic adapter is what stays quiet when the policy and
/// the retired envelope rule agree (every plain ball), so a test can still
/// assert the derivation ran on a tool it did not move.
///
/// C8: APPENDS. This used to be first-writer-wins against a single slot,
/// justified as "the op's own is the load-bearing one and it always runs
/// first" — true, and beside the point: the post-pass derivation still
/// happened, still steered a report, and was discarded without trace. Both
/// are kept, in the order they fired; each carries its own `site`, and the
/// diagnostic adapter decides which are worth showing.
///
/// This rationale was ORPHANED until C8: the block ran into the next `///`
/// line with no blank between them, so the whole PR-6a justification was
/// attached to `record_ramp_reach_clamp` and THIS function carried no doc
/// at all. A first-writer-wins rule that nobody could find is most of how
/// it survived.
pub(super) fn record_derived_stepover(
    cell: &std::cell::RefCell<GenerationFindings>,
    finding: crate::compute::toolpath_stats::DerivedStepoverFinding,
) {
    cell.borrow_mut().derived_stepovers.push(finding);
}
