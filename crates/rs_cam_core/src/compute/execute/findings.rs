//! The one door into [`super::GenerationFindings`] (W2.3).
//!
//! A kernel returns a report. The adapter hands that report to
//! [`super::ExecutionContext::record`] (or [`GenerationFindings::record`]
//! where the writer owns the findings), and the report's
//! [`RecordFindings`] impl below writes every field it carries. An adapter
//! does not pick fields out of a report, so it cannot forget one of them.
//!
//! Each impl keeps the rule its field had as a hand-written `record_*`
//! function: some fields add, some merge, some overwrite, one appends. The
//! three-valued contract (A/M9, X-19) holds everywhere: `None` = not
//! measured, `Some(0)` = measured clean. An impl writes `Some` only where
//! its report says a measurement ran.

use super::GenerationFindings;
use super::dressup_apply::SHALLOW_SLOPE_DERATE_BASIS;
use crate::compute::toolpath_stats::{
    BoundaryClipDroppedFinding, ClaimsReferenceFinding, ClippedBandFinding, DerivedStepoverFinding,
    DroppedBandFinding, InertClaimsDialFinding, RestLevelsFinding, SlopeDerateDetail,
    TipFloatFinding, WaterlineLadderFinding, ZeroRemovalFinding,
};

/// A report that writes itself into [`GenerationFindings`].
///
/// Implement it on the type a kernel returns, not on a field the adapter
/// copies out. Then one `record` call carries the whole report.
pub(crate) trait RecordFindings {
    /// Write this report into `findings`, with this field's own rule.
    fn record_into(&self, findings: &mut GenerationFindings);
}

impl GenerationFindings {
    /// THE door: write one report into these findings.
    pub(crate) fn record<R: RecordFindings + ?Sized>(&mut self, report: &R) {
        report.record_into(self);
    }
}

/// `None` writes nothing. An adapter that planned bands and dropped none,
/// or a pass that never ran, must not overwrite an earlier finding with an
/// absence.
impl<T: RecordFindings> RecordFindings for Option<T> {
    fn record_into(&self, findings: &mut GenerationFindings) {
        if let Some(report) = self {
            report.record_into(findings);
        }
    }
}

/// A prepare that measures nothing (Waterline: its Z-free geometry is the
/// surface itself) reports `()`, which writes nothing.
impl RecordFindings for () {
    fn record_into(&self, _findings: &mut GenerationFindings) {}
}

/// Each report of the list, in order. A Layered prepare that runs one
/// kernel per piece of a region returns one report per piece.
impl<T: RecordFindings> RecordFindings for Vec<T> {
    fn record_into(&self, findings: &mut GenerationFindings) {
        for report in self {
            report.record_into(findings);
        }
    }
}

/// How many 2D offset calls came back with a
/// [`crate::polygon::OffsetFailure`] (Checkpoint C, Q1 / D-2).
///
/// Record it even when the count is zero. That turns "not measured" into
/// "measured clean". Only an adapter that routes its offsets through
/// [`crate::polygon::offset_polygon_reported`] (or a `_reported` sibling)
/// records it; an adapter on the plain name measured nothing and must not
/// claim a zero. The count adds: an operation offsets once per polygon and
/// each call is a separate chance to fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OffsetFailures(pub usize);

impl RecordFindings for OffsetFailures {
    fn record_into(&self, findings: &mut GenerationFindings) {
        findings.offset_library_failures =
            Some(findings.offset_library_failures.unwrap_or(0) + self.0);
    }
}

/// One cascade's residual: the three `ScallopReport` area figures, which
/// always travel together (M4 §5b). The sums add over the cascades of one
/// operation (one per region; one per mid-steep band in a `UnifiedFinish`).
/// The first record turns "not measured" into "measured", a zero included.
struct CascadeResidual {
    uncut_core_mm2: f64,
    untouched_mm2: f64,
    standing_mm2: f64,
}

impl RecordFindings for CascadeResidual {
    fn record_into(&self, findings: &mut GenerationFindings) {
        let prev = findings.truncated_core_mm2.unwrap_or(0.0);
        findings.truncated_core_mm2 = Some(prev + self.uncut_core_mm2);
        let prev_untouched = findings.untouched_material_mm2.unwrap_or(0.0);
        findings.untouched_material_mm2 = Some(prev_untouched + self.untouched_mm2);
        let prev_standing = findings.reached_uncut_estimate_mm2.unwrap_or(0.0);
        findings.reached_uncut_estimate_mm2 = Some(prev_standing + self.standing_mm2);
    }
}

/// A pocket ring cascade: its offset failures, and the standing area when a
/// bound stopped it (Checkpoint C, Q1 and Q3).
///
/// Only the continuity residual: pocket computes no `untouched_mm2` and no
/// `standing_mm2`, and two zeroes there would publish "measured zero" for
/// two measures nobody took (X-19). `None` on `truncated_core_mm2` = no
/// bound fired, so nothing is written.
impl RecordFindings for crate::ops::pocket::PocketCascadeReport {
    fn record_into(&self, findings: &mut GenerationFindings) {
        findings.record(&OffsetFailures(self.offset_failures));
        if let Some(standing) = self.truncated_core_mm2 {
            let prev = findings.truncated_core_mm2.unwrap_or(0.0);
            findings.truncated_core_mm2 = Some(prev + standing);
        }
    }
}

/// A scallop pass: its cascade residual (always), and its link-stage
/// totals when the stage ran (G-LINKVISIBLE: the generator decided, so its
/// `Option` goes straight through).
impl RecordFindings for crate::finish::scallop::ScallopReport {
    fn record_into(&self, findings: &mut GenerationFindings) {
        findings.record(&CascadeResidual {
            uncut_core_mm2: self.uncut_core_mm2,
            untouched_mm2: self.untouched_mm2,
            standing_mm2: self.standing_mm2,
        });
        findings.record(&self.relink);
    }
}

/// A unified finish: every finding its orchestrator reports.
///
/// - the link totals, only when the intra-region pass ran;
/// - the C2 monotone-cell totals, only when that pass ran;
/// - the mid-steep cascade residual (always);
/// - the dropped band (Wave D1) and the clipped band (C8);
/// - the crease node's tip float, `None` when claims never ran;
/// - the derived stepovers: one per slope-derated Shallow region (the
///   operator's `raster_stepover` is not rewritten, so this is the only
///   surface the derived value appears on), then the claims fan's (PR-6a),
///   in that order.
impl RecordFindings for crate::finish::unified_finish::UnifiedFinishReport {
    fn record_into(&self, findings: &mut GenerationFindings) {
        findings.record(&self.relink);
        findings.record(&self.monotone_cells);
        findings.record(&CascadeResidual {
            uncut_core_mm2: self.uncut_core_mm2,
            untouched_mm2: self.untouched_mm2,
            standing_mm2: self.standing_mm2,
        });
        findings.record(&crate::finish::unified_finish::dropped_band_finding(self));
        findings.record(&crate::finish::unified_finish::clipped_band_finding(self));
        findings.record(&self.tip_float);
        for d in &self.shallow_slope_derates {
            findings.record(&DerivedStepoverFinding {
                site: "UnifiedFinish shallow raster slope derate",
                stepover_mm: d.derated_stepover_mm,
                // Not a depth-keyed derivation: the derate is slope-keyed.
                reference_depth_mm: 0.0,
                reference_depth_basis: SHALLOW_SLOPE_DERATE_BASIS,
                envelope_rule_mm: d.configured_stepover_mm,
                slope_derate: Some(SlopeDerateDetail {
                    region_index: d.region_index,
                    slope_max_deg: d.slope_max_deg,
                }),
            });
        }
        if let Some(claims) = self.claims {
            findings.record(&DerivedStepoverFinding {
                site: "UnifiedFinish crease/pencil claims",
                stepover_mm: claims.offset_stepover_mm,
                reference_depth_mm: claims.offset_stepover_reference_depth_mm,
                reference_depth_basis: crate::finish::unified_finish::CLAIMS_STEPOVER_DEPTH_BASIS,
                envelope_rule_mm: claims.envelope_rule_stepover_mm,
                slope_derate: None,
            });
        }
    }
}

/// A pencil pass: its tip float (a centreline op always measures it, zero
/// included) and its own link-stage report, `Some` exactly when the
/// emitter ran.
impl RecordFindings for crate::finish::pencil::PencilReport {
    fn record_into(&self, findings: &mut GenerationFindings) {
        findings.record(&self.tip_float);
        findings.record(&self.link);
    }
}

/// A dropped band (Wave D1): an unmachined feature.
impl RecordFindings for DroppedBandFinding {
    fn record_into(&self, findings: &mut GenerationFindings) {
        findings.dropped_band = Some(*self);
    }
}

/// A partial band clip (C8).
impl RecordFindings for ClippedBandFinding {
    fn record_into(&self, findings: &mut GenerationFindings) {
        findings.clipped_band = Some(*self);
    }
}

/// The centreline tip-float tally (Wave D1). A MEASUREMENT, not a defect:
/// `Some` with zero floating points is the honest "a centreline pass ran
/// and nothing floated". Merges over the passes of one operation.
impl RecordFindings for TipFloatFinding {
    fn record_into(&self, findings: &mut GenerationFindings) {
        let mut merged = findings.tip_float.unwrap_or_default();
        merged.merge(*self);
        findings.tip_float = Some(merged);
    }
}

/// What the ramp-finish reach clamp did (PR-8b). A MEASUREMENT: an inert
/// clamp is the honest "a descent ran and every depth was holdable". The
/// diagnostic adapter stays quiet when nothing moved.
impl RecordFindings for crate::finish::ramp_finish::RampReachClamp {
    fn record_into(&self, findings: &mut GenerationFindings) {
        findings.ramp_reach_clamp = Some(*self);
    }
}

/// Which rest reference the claims pipeline resolved to (A/M6). A
/// DECISION, never suppressed: even the uneventful outcomes say which of
/// two fields the detector read, and no config field or move shows that.
impl RecordFindings for ClaimsReferenceFinding {
    fn record_into(&self, findings: &mut GenerationFindings) {
        findings.claims_reference = Some(*self);
    }
}

/// F4: a non-default rest-claims dial that steered nothing. Build it with
/// [`InertClaimsDialFinding::of_config`], which is `None` at the defaults.
impl RecordFindings for InertClaimsDialFinding {
    fn record_into(&self, findings: &mut GenerationFindings) {
        findings.inert_claims_dial = Some(*self);
    }
}

impl InertClaimsDialFinding {
    /// F4: the inert dials of `cfg`, or `None` when nothing inert is set.
    ///
    /// `None` at the defaults: an operator who never touched these has
    /// nothing to be told. The C2 keeper (`min_rest_depth_mm = 0.02`,
    /// `claims_reference = auto`, `territory_clip = false`) is silent by
    /// that rule, and must stay silent.
    ///
    /// **This changes no geometry.** It reads the config alone, before
    /// anything is planned, and nothing downstream branches on it. The dial
    /// stays inert on purpose; the finding's own doc says why.
    ///
    /// The per-dial rules: `min_rest_depth_mm` needs only
    /// `territory_clip == false` (the S4 mask-AND is its sole consumer);
    /// `claims_reference` also needs `pencil_claims == false`, because with
    /// claims running it still chooses which field the crease detector
    /// reads.
    pub(crate) fn of_config(
        cfg: &crate::compute::operation_configs::UnifiedFinishConfig,
    ) -> Option<Self> {
        if cfg.territory_clip {
            return None;
        }
        let defaults = crate::compute::operation_configs::UnifiedFinishConfig::default();
        let min_rest_depth_inert =
            (cfg.min_rest_depth_mm - defaults.min_rest_depth_mm).abs() > 1e-9;
        let claims_reference_inert =
            !cfg.pencil_claims && cfg.claims_reference != defaults.claims_reference;
        if !min_rest_depth_inert && !claims_reference_inert {
            return None;
        }
        Some(Self {
            min_rest_depth_mm: cfg.min_rest_depth_mm,
            default_min_rest_depth_mm: defaults.min_rest_depth_mm,
            min_rest_depth_inert,
            claims_reference: cfg.claims_reference,
            claims_reference_inert,
            pencil_claims: cfg.pencil_claims,
        })
    }
}

/// F3: what the [`crate::geometry::region_mask::MAX_REST_REGIONS`] cap did
/// to a rest-region extraction. A MEASUREMENT: `Some` with
/// `truncated() == false` is "an extraction ran and kept everything".
/// Record it only from a path that ran an extraction.
impl RecordFindings for crate::geometry::region_mask::RegionCapReport {
    fn record_into(&self, findings: &mut GenerationFindings) {
        findings.region_cap = Some(*self);
    }
}

/// What a finishing link stage did (Phase O item 3, G-LINKVISIBLE). A
/// MEASUREMENT: record it even when every counter is zero, because "the
/// pass ran and had no junction" differs from "the pass never ran".
///
/// Four families reach here, each through its own hookup dial:
/// `unified_finish`, `scallop`, `drop_cutter` and `waterline`. They share
/// the slot because they share the kernel, and one toolpath is one
/// operation. The pencil runs a different linker and has its own slot.
impl RecordFindings for crate::finish::unified_finish::RelinkTotals {
    fn record_into(&self, findings: &mut GenerationFindings) {
        findings.relink = Some(*self);
    }
}

/// What the C2 shallow-band monotone-cell decomposition did. The
/// generator returns `None` unless the pass ran; a zeroed struct in its
/// place would claim a clean measurement of a pass that never happened.
impl RecordFindings for crate::finish::unified_finish::MonotoneCellTotals {
    fn record_into(&self, findings: &mut GenerationFindings) {
        findings.monotone_cells = Some(*self);
    }
}

/// What the PENCIL's link stage did (G-LINKVISIBLE). Its own slot: its
/// counter set (with `hop_too_far` and its own at-depth/hop split) does not
/// fit the shared shape, and a squeezed measurement reads clean and means
/// something else.
impl RecordFindings for crate::finish::pencil::PencilLinkReport {
    fn record_into(&self, findings: &mut GenerationFindings) {
        findings.pencil_link = Some(*self);
    }
}

/// A machining-boundary containment collapsed and the clip was not applied
/// (Checkpoint C, Q2). The boundary clip runs after the adapter returned,
/// where the writer owns the findings, so it records through
/// [`GenerationFindings::record`].
impl RecordFindings for BoundaryClipDroppedFinding {
    fn record_into(&self, findings: &mut GenerationFindings) {
        findings.boundary_clip_dropped = Some(*self);
    }
}

/// What the standalone waterline's ladder builder did (R3 / R4). Written
/// once per `generate_waterline` run, clean or not: `None` on the stats
/// side means "no ladder was built", never "nothing was dropped".
impl RecordFindings for WaterlineLadderFinding {
    fn record_into(&self, findings: &mut GenerationFindings) {
        findings.waterline_ladder = Some(*self);
    }
}

/// A4: a rest pass that reaches nothing. Build it with
/// [`ZeroRemovalFinding::measure`].
impl RecordFindings for ZeroRemovalFinding {
    fn record_into(&self, findings: &mut GenerationFindings) {
        findings.zero_removal = Some(*self);
    }
}

/// An offset stepover derived from the reach policy (PR-6a, H2.3).
///
/// Not suppressed at the no-change case: the finding carries both numbers
/// and the diagnostic adapter decides. C8: APPENDS. One toolpath can derive
/// a stepover twice (the op's own site, then the generic rest-analysis
/// post-pass); both are kept in the order they fired, each with its own
/// `site`.
impl RecordFindings for DerivedStepoverFinding {
    fn record_into(&self, findings: &mut GenerationFindings) {
        findings.derived_stepovers.push(*self);
    }
}

/// One outline's rest-stock level clip (G-RESTLEVELS). Merges over the
/// outlines of one operation.
impl RecordFindings for RestLevelsFinding {
    fn record_into(&self, findings: &mut GenerationFindings) {
        match findings.rest_levels.as_mut() {
            Some(sum) => sum.merge(*self),
            None => findings.rest_levels = Some(*self),
        }
    }
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

impl ZeroRemovalFinding {
    /// A4: measure the emitted cutting geometry against the reference stock
    /// the pass was planned on. `Some` when it reaches nothing.
    ///
    /// Call it only where a rest pass resolved a REAL machined-stock
    /// reference. Under any other reference the op is not a rest pass in
    /// the sense the finding is about.
    ///
    /// Measured PRE-dressup, on the geometry the planner emitted. The
    /// air-cut filter that runs later deletes moves wholly in air, and a
    /// pass riding exactly on the surface it already cut is not in air by
    /// that test: it survives, which is how §3.2's rest pass came to spend
    /// 1 294 mm and 48 retract trips on nothing.
    pub(crate) fn measure(
        toolpath: &crate::toolpath::Toolpath,
        stock: &crate::dexel_stock::TriDexelStock,
        cutter: &dyn crate::tool::MillingCutter,
    ) -> Option<Self> {
        let engagement =
            crate::dressup::reference_engagement_of_cutting_moves(toolpath, stock, cutter);
        // Nothing sampled = nothing measured. Not a finding (X-19's rule):
        // an absent measurement is not a defect claim.
        if engagement.sampled_positions == 0 {
            return None;
        }
        let floor_mm = zero_removal_engagement_floor_mm(stock, cutter);
        if engagement.deepest_mm > floor_mm {
            return None;
        }
        Some(Self {
            deepest_engagement_mm: engagement.deepest_mm,
            sampled_positions: engagement.sampled_positions,
            cutting_distance_mm: toolpath.total_cutting_distance(),
            floor_mm,
        })
    }
}
