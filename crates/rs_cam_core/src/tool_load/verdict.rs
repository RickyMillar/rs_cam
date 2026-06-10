//! Verdict and confidence types for the tool-load monitor.
//!
//! Each guardrail criterion (chipload, power, deflection, ...) reports an
//! independent `Verdict`. There is no scalar "load %" — a project-wide
//! report is a vector of per-criterion verdicts per toolpath. A criterion that
//! cannot be evaluated honestly returns `Unmodeled` with a typed reason; it
//! never silently falls back to a passing or failing value.

use crate::ids::ToolpathId;
use std::collections::BTreeMap;
use std::ops::Range;

use serde::{Deserialize, Serialize};

/// F-039 — Which physical constraint set the modulated feed for a
/// single cutting move (constrained-max solver). One of six bound
/// types: the chipload band's upper edge, the deflection cap, the
/// power cap, the machine's hard feed cap, the kinematic-reach cap
/// from the F-034 / F-035 integrator, or the chipload band's lower
/// edge (rubbing floor, applied last).
///
/// The variants are `Ord` so consumer code can build histograms and
/// `BTreeMap` keyed views without extra glue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingConstraint {
    /// Chipload band's upper edge bound the feed (commanded chipload
    /// would have exceeded `band.max`).
    ChiploadMax,
    /// Tip-deflection cap bound the feed (cantilever displacement
    /// would have exceeded `EXCEEDS_BOUND_MM`).
    DeflectionMax,
    /// Spindle power cap bound the feed (`Kc × DOC × WOC × feed`
    /// would have exceeded `available_kw × safety_factor`).
    PowerMax,
    /// Machine's `max_feed_mm_min` hard cap (`$110`/`$111`/`$112`).
    MachineMaxFeed,
    /// Kinematic-reach cap from the per-move accel/junction
    /// integrator (F-034 / F-035) — the move was too short to reach
    /// the requested feed in the available distance.
    KinematicReach,
    /// Chipload band's lower edge bound the feed (rubbing floor —
    /// applied AFTER aggressiveness scaling so feeds never drop
    /// below `band.min × rpm × flutes` even at low aggressiveness).
    ChiploadMin,
}

impl BindingConstraint {
    pub fn label(self) -> &'static str {
        match self {
            BindingConstraint::ChiploadMax => "chipload-max",
            BindingConstraint::DeflectionMax => "deflection-max",
            BindingConstraint::PowerMax => "power-max",
            BindingConstraint::MachineMaxFeed => "machine-max-feed",
            BindingConstraint::KinematicReach => "kinematic-reach",
            BindingConstraint::ChiploadMin => "chipload-min",
        }
    }
}

/// F-039 — Per-toolpath transparency rollup for the constrained-max
/// modulator. Surfaced on [`ToolpathLoadVerdict::modulation_summary`].
///
/// Fields:
/// - `moves_touched`: number of cutting moves whose `feed_rate` was
///   actually rewritten by the modulator (skipped intents, rapids,
///   and no-engagement moves don't count).
/// - `moves_total`: total cutting-eligible moves the modulator
///   considered (excludes rapids).
/// - `median_feed_delta_pct`: median of
///   `(modulated - commanded) / commanded × 100` across touched
///   moves. Negative when modulation backed off, positive when it
///   raised feed.
/// - `binding_constraint_distribution`: fraction (0.0–1.0) of
///   touched moves whose binding constraint matched each variant.
///   Sums to ≤ 1.0 (rounding); variants with zero share are omitted.
/// - `aggressiveness`: the scalar the run used (1.0 = at-limit;
///   0.7 = 70 %; etc.).
/// - `strategy`: which algorithm produced this summary (band-mid or
///   constrained-max). `BandMid` rolls up `binding_constraint_distribution`
///   from the same six-variant enum but its bindings collapse to the
///   single legacy "chipload" clamp band; F-036c-style retro reads
///   stay possible.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModulationSummary {
    pub moves_touched: usize,
    pub moves_total: usize,
    pub median_feed_delta_pct: f64,
    pub binding_constraint_distribution: BTreeMap<BindingConstraint, f64>,
    pub aggressiveness: f64,
    pub strategy: ModulationStrategyTag,
}

/// Stable serialised tag for the modulation strategy used. Mirrors
/// the runtime [`crate::feed_modulation::ModulationStrategy`] enum;
/// duplicated here to keep `verdict.rs` free of feed-modulation
/// dependencies (the verdict types live closer to the public
/// reporting surface).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModulationStrategyTag {
    BandMid,
    ConstrainedMax,
}

/// Why a criterion could not be evaluated. Typed (not free-form strings) so
/// callers can branch and the UI can localize.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "detail")]
pub enum UnmodeledReason {
    /// No simulation has been run, or the cached trace doesn't cover this
    /// toolpath. The criterion needs per-sample metrics.
    SimulationRequired,
    /// A simulation trace exists, but its provenance hashes don't match the
    /// current project state — the toolpath, tool, stock, or machine has
    /// changed since it was captured.
    StaleSimulation,
    /// The simulation was run without arc-engagement capture enabled.
    /// Re-run with `MetricOptions::capture_arc_engagement = true`.
    ArcEngagementNotCaptured,
    /// No vendor LUT row matches the (tool family, material family) tuple
    /// for this toolpath. The chipload bounds are unknown.
    NoVendorData,
    /// The simulation trace exists, but no samples for this toolpath are
    /// running at the operation's commanded feed rate (steady-state
    /// cutting). Typically a pure-plunge drill cycle, or a toolpath where
    /// every sample is a ramp/entry move at a different feed. The
    /// chipload-vs-LUT comparison is calibrated for steady-state cutting,
    /// so we refuse rather than flag transient feeds against it.
    SteadyStateSamplesNotPresent,
    /// The material is `Custom` without an explicitly-validated `kc`.
    /// We refuse to compute a force-derived envelope from a guessed Kc.
    MaterialUnvalidated,
    /// The cutter shape cannot model the engagement mode in this region
    /// (e.g. V-bit at the tip, ball nose past the hemisphere pole).
    /// The free-form `String` carries the cutter-supplied reason.
    CutterModeUnsupported(String),
    /// The criterion is intentionally not implemented yet (deferred to a
    /// later phase). The string names the phase or follow-up.
    NotImplemented(String),
    /// The gate is geometrically not applicable to this operation. The
    /// simulator isn't failing — the criterion just has no meaning for
    /// the op type. The carried `String` is the operator-facing
    /// explanation (e.g. `"drill cycle — no continuous engagement"`).
    ///
    /// Roadmap F.8: separates "couldn't measure" (re-run sim, supply
    /// better data) from "doesn't apply" (no action needed). Surfaced
    /// for drill / alignment-pin-drill cycles which are plunge-only.
    /// `String` (not `&'static str`) because the verdict deserializes
    /// over the MCP wire.
    NotApplicableForOp(String),
    /// Simulation ran end-to-end but the toolpath made no contact with
    /// material: every sample is either a rapid move or an air-cut
    /// pass. Distinct from `SimulationRequired` (no trace exists) —
    /// surfaces the user-facing finding that the toolpath generated
    /// zero in-material work (UX dial-in finding A1 / A10).
    AllSamplesAirCutOrRapid,
}

/// What a "Within" or "Exceeds" verdict claims about its inputs.
///
/// `Validated` is rare — it means every input was independently checked.
/// Most useful results are `Approximate` with a typed reason; UI must render
/// `Approximate` differently from `Validated` so users don't anchor on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "detail")]
pub enum Confidence {
    /// All inputs validated; the verdict is trustworthy.
    Validated,
    /// Verdict is best-effort given known input limitations. The string
    /// describes which input is approximate (e.g. "isotropic Kc only",
    /// "slot-engagement decomposition").
    Approximate(String),
}

/// Per-toolpath outcome across all criteria.
///
/// `toolpath_id` is the core `usize` index into the project's enabled
/// toolpath list (matches `SimulationCutSample::toolpath_id` semantics).
///
/// All three gates use typed verdicts (G16 Step 7).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolpathLoadVerdict {
    pub toolpath_id: ToolpathId,
    pub chipload: ChiploadVerdict,
    pub power: PowerVerdict,
    pub deflection: DeflectionVerdict,
    /// Drill-specific gate trio (§6.E / Step 3 PR2). Populated only for
    /// drill toolpaths — `None` for milling ops. Lives alongside the
    /// existing three gates so consumers iterating
    /// [`ToolpathLoadVerdict::criteria`] keep working unchanged, while
    /// drill-aware UI can fan out the drill-specific gates when the field
    /// is `Some`. The existing chipload/power/deflection gates remain
    /// `Unmodeled(NotApplicableForOp)` for drill ops.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drill_gates: Option<crate::tool_load::drill_gates::DrillGatesVerdict>,
    /// F-039 — per-toolpath constrained-max modulation rollup.
    /// `Some` when adaptive feed modulation ran on this toolpath and
    /// rewrote at least one move's feed; `None` when modulation was
    /// off, was a no-op (no LUT band / no kinematics / drill cycle),
    /// or hadn't been computed yet. The field is additive — existing
    /// consumers that iterate criteria stay byte-stable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modulation_summary: Option<ModulationSummary>,
}

impl ToolpathLoadVerdict {
    /// Count criteria with a non-`Unmodeled` verdict (i.e. actually evaluated).
    pub fn modeled_count(&self) -> usize {
        self.criteria()
            .iter()
            .filter(|s| s.state != LoadState::Unmodeled)
            .count()
    }

    /// True if any criterion is `Exceeds`.
    pub fn any_exceeded(&self) -> bool {
        self.criteria()
            .iter()
            .any(|s| s.state == LoadState::Exceeds)
    }

    /// True if any milling criterion is `Unmodeled` and needs operator
    /// action. Drill cycles carry drill-native gates; when all milling
    /// criteria are `NotApplicableForOp` and drill gates exist, that is
    /// neutral rather than scary/missing evidence.
    pub fn any_unmodeled(&self) -> bool {
        if self.drill_gates.is_some() && all_not_applicable(self) {
            return false;
        }
        self.criteria()
            .iter()
            .any(|s| s.state == LoadState::Unmodeled)
    }

    /// Generic per-criterion summaries — chipload, power, deflection in
    /// that order. Lets UI / export / timeline iterate over the gates
    /// without knowing each typed verdict's internals.
    ///
    /// Returns `Vec` rather than a fixed-arity array (Phase 6 task 4)
    /// so a future fourth gate extends the list without breaking
    /// `for status in verdict.criteria()` consumers. This is **the**
    /// single inclusion point for the gating tier: `modeled_count`,
    /// `any_exceeded`, `any_unmodeled`, `exceeded_criteria` (and through
    /// it `enforce_load_policy` + the summary breakdown) all derive from
    /// this list, so a gate added here automatically participates in
    /// export gating — a forgotten gate is structurally impossible.
    pub fn criteria(&self) -> Vec<CriterionStatus<'_>> {
        let mut all = self.milling_criteria();
        // F1.7: drill gates join the criterion tier when present, so a
        // Critical drill exceedance blocks export exactly like a
        // milling trip (pre-fix `drill_gates` was display-only and
        // `enforce_load_policy` never saw it).
        if let Some(d) = &self.drill_gates {
            all.push(
                d.chip_welding
                    .as_criterion_status(CriterionKind::DrillChipWelding),
            );
            all.push(
                d.peck_adequacy
                    .as_criterion_status(CriterionKind::DrillPeckAdequacy),
            );
            all.push(
                d.plunge_feed
                    .as_criterion_status(CriterionKind::DrillPlungeFeed),
            );
        }
        all
    }

    /// The three milling gates only — chipload, power, deflection.
    /// Used by `all_not_applicable` to decide the "drill cycle, milling
    /// gates don't apply" partition without the drill criteria muddying
    /// the test.
    pub fn milling_criteria(&self) -> Vec<CriterionStatus<'_>> {
        vec![
            self.chipload.as_criterion_status(),
            self.power.as_criterion_status(),
            self.deflection.as_criterion_status(),
        ]
    }

    /// Per-criterion exceedance labels for this toolpath. Empty when no
    /// criterion is `Exceeds`. Used by the export gate. Derived from
    /// `criteria()` (Phase 6 task 5) — each gate's `as_criterion_status`
    /// carries its own typed `ExceededCriterion` on `Exceeds`.
    pub fn exceeded_criteria(&self) -> Vec<ExceededCriterion> {
        self.criteria()
            .into_iter()
            .filter_map(|s| s.exceeded)
            .collect()
    }
}

/// Project-level report: one verdict per toolpath.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolLoadReport {
    pub per_toolpath: Vec<ToolpathLoadVerdict>,
}

/// F5 — first-look summary surfaced alongside the per-toolpath verdicts.
/// Lets a caller answer "what's broken in this project?" from a single
/// read instead of folding the per-toolpath array themselves.
///
/// Computed from `ToolLoadReport::summary()`; not serialized as part of
/// `ToolLoadReport` itself so existing consumers stay byte-stable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolLoadReportSummary {
    pub total_toolpaths: usize,
    /// Toolpaths with at least one criterion in `Within` and none in `Exceeds`.
    pub within: usize,
    /// Toolpaths with at least one criterion in `Exceeds`.
    pub exceeds: usize,
    /// Toolpaths whose every criterion is `Unmodeled` because the gate
    /// inputs are missing or stale (e.g. simulation not run, arc
    /// engagement not captured, vendor LUT row not found). These need
    /// operator action (re-sim, calibrate, supply tool data).
    pub fully_unmodeled: usize,
    /// Roadmap F.8: toolpaths whose every criterion is `Unmodeled`
    /// because the gate genuinely doesn't apply to the operation type
    /// (drill cycles, alignment-pin drills — no continuous engagement
    /// to measure). No operator action needed. Buckets separately from
    /// `fully_unmodeled` so a project summary can answer "what's
    /// broken?" without flagging plunge-only ops.
    #[serde(default)]
    pub not_applicable: usize,
    /// One entry per `Exceeds` criterion across the project, in toolpath
    /// order.
    pub exceeds_breakdown: Vec<ExceedsEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExceedsEntry {
    pub toolpath_id: ToolpathId,
    /// Operator-facing toolpath name resolved at summary time. Empty
    /// string when the caller didn't supply a name resolver (e.g. unit
    /// tests). Roadmap F.11.
    #[serde(default)]
    pub toolpath_name: String,
    /// `"chipload"` / `"power"` / `"deflection"`.
    pub gate: String,
    /// `"low"` / `"high"` for chipload; `None` for power / deflection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side: Option<String>,
}

impl ToolLoadReport {
    pub fn any_exceeded(&self) -> bool {
        self.per_toolpath
            .iter()
            .any(ToolpathLoadVerdict::any_exceeded)
    }

    pub fn any_unmodeled(&self) -> bool {
        self.per_toolpath
            .iter()
            .any(ToolpathLoadVerdict::any_unmodeled)
    }

    /// All toolpath **ids** (not indices) that have at least one `Exceeds`
    /// verdict, with per-criterion `ExceededCriterion` entries. Used by the
    /// export gate to produce the blocking error message.
    pub fn exceeded_criteria(&self) -> Vec<(ToolpathId, Vec<ExceededCriterion>)> {
        self.per_toolpath
            .iter()
            .filter_map(|v| {
                let crits = v.exceeded_criteria();
                if crits.is_empty() {
                    None
                } else {
                    Some((v.toolpath_id, crits))
                }
            })
            .collect()
    }

    /// F5 — project-level rollup of per-toolpath verdicts. Cheap to call;
    /// folds the `per_toolpath` array once. `name_for` resolves
    /// `toolpath_id → display name` for `ExceedsEntry.toolpath_name`
    /// (Roadmap F.11); pass `|_| None` from contexts that don't have a
    /// `ProjectSession` handy and the field collapses to an empty
    /// string.
    pub fn summary<F>(&self, name_for: F) -> ToolLoadReportSummary
    where
        F: Fn(ToolpathId) -> Option<String>,
    {
        let mut within = 0usize;
        let mut exceeds = 0usize;
        let mut fully_unmodeled = 0usize;
        let mut not_applicable = 0usize;
        let mut exceeds_breakdown: Vec<ExceedsEntry> = Vec::new();
        for v in &self.per_toolpath {
            if v.any_exceeded() {
                exceeds += 1;
            } else if v.modeled_count() == 0 {
                // Roadmap F.8: bucket "doesn't apply" separately from
                // "couldn't measure". A toolpath is `not_applicable`
                // only when every gate reports `NotApplicableForOp` —
                // if any gate is `Unmodeled` for a measurable reason
                // (sim required, arc engagement not captured, ...)
                // the toolpath needs operator action and rolls up as
                // `fully_unmodeled`.
                if all_not_applicable(v) {
                    not_applicable += 1;
                } else {
                    fully_unmodeled += 1;
                }
            } else {
                within += 1;
            }
            let name = name_for(v.toolpath_id).unwrap_or_default();
            // Derived from `criteria()` via `exceeded_criteria()` (Phase 6
            // task 5) — a gate included in `criteria()` automatically
            // appears in the breakdown; order stays chipload, power,
            // deflection.
            for ec in v.exceeded_criteria() {
                exceeds_breakdown.push(ExceedsEntry {
                    toolpath_id: v.toolpath_id,
                    toolpath_name: name.clone(),
                    gate: ec.label.to_owned(),
                    side: ec.side_label().map(str::to_owned),
                });
            }
        }
        ToolLoadReportSummary {
            total_toolpaths: self.per_toolpath.len(),
            within,
            exceeds,
            fully_unmodeled,
            not_applicable,
            exceeds_breakdown,
        }
    }
}

/// True when every criterion on the verdict is
/// `Unmodeled(NotApplicableForOp)` — the gate genuinely doesn't apply
/// to the op type (drill cycles, alignment-pin drills). Used by
/// `summary()` to partition `fully_unmodeled` from `not_applicable`.
fn all_not_applicable(v: &ToolpathLoadVerdict) -> bool {
    // Milling criteria only — drill criteria are always modeled, so
    // including them here would break the "drill cycle, milling gates
    // don't apply" partition (and with it `any_unmodeled`'s drill
    // special case, re-blocking export on accept_unmodeled policies).
    v.milling_criteria()
        .iter()
        .all(|s| is_not_applicable(s.unmodeled_reason))
}

fn is_not_applicable(reason: Option<&UnmodeledReason>) -> bool {
    matches!(reason, Some(UnmodeledReason::NotApplicableForOp(_)))
}

// ─────────────────────────────────────────────────────────────────────
// Typed verdict scaffolding (G16 Step 7a). Lives alongside the legacy
// flat `Verdict` until the per-gate evaluators migrate. No consumers
// read these yet.
// ─────────────────────────────────────────────────────────────────────

/// Coarse outcome shared across all gate verdicts. Lets UI / export /
/// timeline iterate without knowing each typed verdict's internals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadState {
    Within,
    Exceeds,
    Unmodeled,
}

/// Identifies which gate produced a verdict. Used by the helper layer
/// to label criteria for UI, export, and the MCP wire format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CriterionKind {
    Chipload,
    Power,
    Deflection,
    // F1.7 (2026-06-10): drill-native gates join the criterion tier so
    // they participate in export gating like the milling gates.
    DrillChipWelding,
    DrillPeckAdequacy,
    DrillPlungeFeed,
}

impl CriterionKind {
    pub fn label(self) -> &'static str {
        match self {
            CriterionKind::Chipload => "chipload",
            CriterionKind::Power => "power",
            CriterionKind::Deflection => "deflection",
            CriterionKind::DrillChipWelding => "chip welding",
            CriterionKind::DrillPeckAdequacy => "peck depth",
            CriterionKind::DrillPlungeFeed => "plunge feed",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            CriterionKind::Chipload => "mm/tooth",
            CriterionKind::Power => "kW",
            CriterionKind::Deflection => "mm",
            CriterionKind::DrillChipWelding | CriterionKind::DrillPeckAdequacy => "D/d",
            CriterionKind::DrillPlungeFeed => "mm/min per mm Ø",
        }
    }
}

/// Generic per-criterion summary used by UI / export / timeline. A
/// typed verdict produces one of these via `as_criterion_status`,
/// hiding its internals from consumers that just want
/// "what kind, what state, what peak, what range".
#[derive(Debug, Clone)]
pub struct CriterionStatus<'a> {
    pub kind: CriterionKind,
    pub state: LoadState,
    pub confidence: Option<&'a Confidence>,
    pub unmodeled_reason: Option<&'a UnmodeledReason>,
    pub sample_range: Option<Range<usize>>,
    pub display_peak: Option<f64>,
    pub unit: &'static str,
    /// `Some` iff `state == Exceeds` — the typed exceedance label for
    /// this gate, set by each verdict's `as_criterion_status`. The
    /// gating tier (`exceeded_criteria`, `enforce_load_policy`, the
    /// summary breakdown) derives from this field, so including a gate
    /// in `criteria()` is the one decision that makes it gate g-code
    /// export (Phase 6 task 5).
    pub exceeded: Option<ExceededCriterion>,
}

/// Sample-range evidence behind a peak metric. Empty range (`0..0`)
/// means the criterion has no per-sample resolution (or no specific
/// triggering sample is recorded). `statistic` is `Some` only on the
/// chipload gate, which uses different statistics for the burn vs.
/// breakage sides.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SampleEvidence {
    pub sample_range: Range<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statistic: Option<ChiploadStatistic>,
    /// G17 A3 — short operator-facing label for *where* the triggering
    /// sample sits in the cut: "slot section", "heavy engagement",
    /// "plunge entry", "helix entry". `None` for plain steady-state
    /// samples or when arc engagement was not captured. Set by the
    /// per-gate evaluators via [`crate::tool_load::locality`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locality: Option<String>,
}

impl SampleEvidence {
    /// No specific sample recorded: range `0..0`, no statistic.
    pub fn empty() -> Self {
        Self {
            sample_range: 0..0,
            statistic: None,
            locality: None,
        }
    }

    /// Single-sample range at `idx`, no statistic descriptor.
    pub fn at(idx: usize) -> Self {
        Self {
            sample_range: idx..(idx + 1),
            statistic: None,
            locality: None,
        }
    }

    /// Single-sample range at `idx` annotated with a chipload statistic.
    pub fn at_with_stat(idx: usize, statistic: ChiploadStatistic) -> Self {
        Self {
            sample_range: idx..(idx + 1),
            statistic: Some(statistic),
            locality: None,
        }
    }

    /// Builder — attach a locality label after construction. Pass
    /// `Some(label)` from a `classify_sample_locality` call; `None`
    /// leaves the field unset.
    #[must_use]
    pub fn with_locality(mut self, locality: Option<String>) -> Self {
        self.locality = locality;
        self
    }
}

/// Which statistic produced a reported chipload value. Burn-risk uses
/// the *median* of in-cut chip thicknesses (robust to transient low-arc
/// samples); breakage uses the per-sample peak. `PeakInRange` is the
/// within-bounds reporting case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChiploadStatistic {
    /// Median of in-cut sample chip thicknesses, vs LUT min.
    MedianLow,
    /// Per-sample peak above LUT max.
    PeakHigh,
    /// Per-sample peak inside the LUT envelope (Within reporting).
    PeakInRange,
}

/// Where the chipload bounds came from. Distinct from
/// `optimize::bounds::BoundsSource` (axis-bound source).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChipBoundsSource {
    /// Direct vendor LUT row match for the (tool, material, op) tuple.
    VendorLut,
    /// Vendor LUT row extrapolated to the query's diameter / hardness.
    /// The diagnostic detail (scale factors, calibrated row id) lives
    /// on the verdict's `Confidence::Approximate` payload.
    VendorLutExtrapolated,
    /// F3.3 (defect class C1) — the matched row publishes a
    /// single-point "range" (raw min == max): a nominal preset, not a
    /// calibrated envelope. The implied burn floor is fabricated
    /// precision.
    VendorLutPointPreset,
    /// F3.3 — the matched row carries no `ae_min`/`ae_max`
    /// calibration, so the engagement-arc normalization was skipped
    /// and the bound comparison is arc-uncompensated.
    VendorLutMissingAe,
}

impl ChipBoundsSource {
    /// F3.3 — true when the LOW (burn) side of these bounds is too
    /// weakly provenanced to hard-refuse on: a low-side trip downgrades
    /// to a structured burn advisory
    /// ([`ChiploadVerdict::Within::burn_advisory`]) instead of
    /// `Exceeds(Low)`. The HIGH (breakage) side stays hard for every
    /// source — over-thick chips break teeth regardless of how the
    /// bound was derived.
    pub fn low_side_is_advisory(self) -> bool {
        match self {
            ChipBoundsSource::VendorLut => false,
            ChipBoundsSource::VendorLutExtrapolated
            | ChipBoundsSource::VendorLutPointPreset
            | ChipBoundsSource::VendorLutMissingAe => true,
        }
    }
}

/// G17 C2 — informational entry-sample spike that exceeded the
/// gate's bound but was *excluded* from the trip decision because it
/// sat in a transient entry move (helix / plunge). Carries the worst
/// reading observed in the entry samples so the UI can surface it as
/// a "Note:" advisory without flipping the verdict to `Exceeds`.
///
/// Only present on the `Within` arm of each gate verdict. When the
/// gate trips on a steady-state sample (`Exceeds`), the trip itself
/// is the loud signal and the entry spike is omitted to keep the
/// payload focused.
///
/// `side` is meaningful only on chipload spikes (which can spike
/// independently on the burn / breakage sides). Power and deflection
/// always set `side: None`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntrySpike {
    /// Worst observed value in the entry samples for this gate's
    /// concerning direction.
    pub observed: f64,
    /// The bound the entry sample exceeded. Same semantics as
    /// `LimitingGate.bound` — chipload max for breakage, chipload
    /// min for burn, `available_kw` for power, `bounds.exceeds_mm`
    /// for deflection.
    pub bound: f64,
    /// Locality label for the entry sample
    /// (`"helix entry"` / `"plunge entry"` /
    /// occasionally `"heavy engagement"`). Mirrors
    /// [`SampleEvidence::locality`].
    pub locality: String,
    /// For chipload: which side spiked. `None` for power / deflection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub side: Option<ChipSide>,
}

/// Vendor LUT chip-load bounds. `min_mm_per_tooth` is `Option` because
/// some LUT rows ship only an upper bound — the chipload evaluator can
/// still flag breakage from `PeakHigh` while burn-risk falls back to
/// `Unmodeled` for that row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChipBounds {
    pub min_mm_per_tooth: Option<f64>,
    pub max_mm_per_tooth: f64,
    pub source: ChipBoundsSource,
}

/// Which side of the LUT envelope a chipload exceedance landed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChipSide {
    /// Below LUT min — burn / rubbing risk. Statistic is `MedianLow`.
    Low,
    /// Above LUT max — breakage risk. Statistic is `PeakHigh`.
    High,
}

/// One per-side chipload reading: the observed value, the statistic
/// that produced it, the supporting sample-range evidence, and the
/// LUT bounds it was compared against.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChiploadMetric {
    pub observed_mm_per_tooth: f64,
    pub statistic: ChiploadStatistic,
    pub evidence: SampleEvidence,
    pub bounds: ChipBounds,
}

/// Typed chipload verdict. Replaces the flat `Verdict` once the
/// chipload evaluator migrates (Step 7d). Carries both bounds-approach
/// metrics on the within case and the triggering metric on exceed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ChiploadVerdict {
    Within {
        /// Distance-to-min metric. `None` when the matched row has no
        /// `chip_load_min_mm` — burn-risk arm is `Unmodeled` for that
        /// row.
        approach_to_min: Option<ChiploadMetric>,
        /// Distance-to-max metric. Always present (the evaluator
        /// rejects rows with no max).
        approach_to_max: ChiploadMetric,
        confidence: Confidence,
        /// G17 C2 — entry-sample spike(s) that exceeded the LUT
        /// bounds but were excluded from the gate trip. Up to two
        /// entries (one per side: high / low). Empty when no entry
        /// sample exceeded its bound.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        entry_spikes: Vec<EntrySpike>,
        /// F3.3 — the median chip thickness sat below the row's burn
        /// floor, but the floor's provenance is too weak to refuse on
        /// ([`ChipBoundsSource::low_side_is_advisory`]): surfaced as a
        /// structured advisory instead of flipping to `Exceeds(Low)`.
        /// Candidates carrying this land in the MarginalSafe tier
        /// rather than auto-recommending.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        burn_advisory: Option<Box<ChiploadMetric>>,
    },
    Exceeds {
        side: ChipSide,
        triggering: ChiploadMetric,
        confidence: Confidence,
    },
    Unmodeled {
        reason: UnmodeledReason,
    },
}

impl ChiploadVerdict {
    pub fn state(&self) -> LoadState {
        match self {
            ChiploadVerdict::Within { .. } => LoadState::Within,
            ChiploadVerdict::Exceeds { .. } => LoadState::Exceeds,
            ChiploadVerdict::Unmodeled { .. } => LoadState::Unmodeled,
        }
    }

    pub fn is_exceeded(&self) -> bool {
        matches!(self, ChiploadVerdict::Exceeds { .. })
    }

    pub fn is_unmodeled(&self) -> bool {
        matches!(self, ChiploadVerdict::Unmodeled { .. })
    }

    pub fn confidence(&self) -> Option<&Confidence> {
        match self {
            ChiploadVerdict::Within { confidence, .. }
            | ChiploadVerdict::Exceeds { confidence, .. } => Some(confidence),
            ChiploadVerdict::Unmodeled { .. } => None,
        }
    }

    pub fn unmodeled_reason(&self) -> Option<&UnmodeledReason> {
        match self {
            ChiploadVerdict::Unmodeled { reason } => Some(reason),
            _ => None,
        }
    }

    pub fn as_criterion_status(&self) -> CriterionStatus<'_> {
        let (state, peak, range, exceeded) = match self {
            ChiploadVerdict::Within {
                approach_to_max, ..
            } => (
                LoadState::Within,
                Some(approach_to_max.observed_mm_per_tooth),
                option_range(&approach_to_max.evidence.sample_range),
                None,
            ),
            ChiploadVerdict::Exceeds {
                triggering, side, ..
            } => (
                LoadState::Exceeds,
                Some(triggering.observed_mm_per_tooth),
                option_range(&triggering.evidence.sample_range),
                Some(match side {
                    ChipSide::Low => ExceededCriterion::chipload_burn(),
                    ChipSide::High => ExceededCriterion::chipload_breakage(),
                }),
            ),
            ChiploadVerdict::Unmodeled { .. } => (LoadState::Unmodeled, None, None, None),
        };
        CriterionStatus {
            kind: CriterionKind::Chipload,
            state,
            confidence: self.confidence(),
            unmodeled_reason: self.unmodeled_reason(),
            sample_range: range,
            display_peak: peak,
            unit: CriterionKind::Chipload.unit(),
            exceeded,
        }
    }
}

/// Typed power verdict. Both `Within` and `Exceeds` carry
/// `available_kw` so UI / MCP can render the headroom band.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum PowerVerdict {
    Within {
        peak_kw: f64,
        available_kw: f64,
        evidence: SampleEvidence,
        confidence: Confidence,
        /// G17 C2 — entry-sample power spike that exceeded
        /// `available_kw` but was excluded from the gate trip.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        entry_spike: Option<EntrySpike>,
    },
    Exceeds {
        peak_kw: f64,
        available_kw: f64,
        evidence: SampleEvidence,
        confidence: Confidence,
    },
    Unmodeled {
        reason: UnmodeledReason,
    },
}

impl PowerVerdict {
    pub fn state(&self) -> LoadState {
        match self {
            PowerVerdict::Within { .. } => LoadState::Within,
            PowerVerdict::Exceeds { .. } => LoadState::Exceeds,
            PowerVerdict::Unmodeled { .. } => LoadState::Unmodeled,
        }
    }

    pub fn is_exceeded(&self) -> bool {
        matches!(self, PowerVerdict::Exceeds { .. })
    }

    pub fn is_unmodeled(&self) -> bool {
        matches!(self, PowerVerdict::Unmodeled { .. })
    }

    pub fn confidence(&self) -> Option<&Confidence> {
        match self {
            PowerVerdict::Within { confidence, .. } | PowerVerdict::Exceeds { confidence, .. } => {
                Some(confidence)
            }
            PowerVerdict::Unmodeled { .. } => None,
        }
    }

    pub fn unmodeled_reason(&self) -> Option<&UnmodeledReason> {
        match self {
            PowerVerdict::Unmodeled { reason } => Some(reason),
            _ => None,
        }
    }

    pub fn as_criterion_status(&self) -> CriterionStatus<'_> {
        let (state, peak, range) = match self {
            PowerVerdict::Within {
                peak_kw, evidence, ..
            } => (
                LoadState::Within,
                Some(*peak_kw),
                option_range(&evidence.sample_range),
            ),
            PowerVerdict::Exceeds {
                peak_kw, evidence, ..
            } => (
                LoadState::Exceeds,
                Some(*peak_kw),
                option_range(&evidence.sample_range),
            ),
            PowerVerdict::Unmodeled { .. } => (LoadState::Unmodeled, None, None),
        };
        CriterionStatus {
            kind: CriterionKind::Power,
            state,
            confidence: self.confidence(),
            unmodeled_reason: self.unmodeled_reason(),
            sample_range: range,
            display_peak: peak,
            unit: CriterionKind::Power.unit(),
            exceeded: (state == LoadState::Exceeds).then(ExceededCriterion::power),
        }
    }
}

/// Both deflection thresholds visible on the verdict so UI can render
/// the validated-within / approximate-within / exceeds bands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeflectionBounds {
    /// Below this peak tip deflection (mm), `Within(Validated)`.
    /// 0.050 mm in `tool_load::deflection`.
    pub validated_within_mm: f64,
    /// Above this peak tip deflection (mm), `Exceeds`. 0.200 mm.
    /// Between the two thresholds, `Within(Approximate)` with
    /// finish-degradation warning.
    pub exceeds_mm: f64,
}

/// Typed deflection verdict. Both `Within` and `Exceeds` carry the
/// bounds so consumers can render the threshold band without hardcoding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum DeflectionVerdict {
    Within {
        peak_mm: f64,
        bounds: DeflectionBounds,
        evidence: SampleEvidence,
        confidence: Confidence,
        /// G17 C2 — entry-sample deflection spike that exceeded
        /// `bounds.exceeds_mm` but was excluded from the gate trip.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        entry_spike: Option<EntrySpike>,
    },
    Exceeds {
        peak_mm: f64,
        bounds: DeflectionBounds,
        evidence: SampleEvidence,
        confidence: Confidence,
    },
    Unmodeled {
        reason: UnmodeledReason,
    },
}

impl DeflectionVerdict {
    pub fn state(&self) -> LoadState {
        match self {
            DeflectionVerdict::Within { .. } => LoadState::Within,
            DeflectionVerdict::Exceeds { .. } => LoadState::Exceeds,
            DeflectionVerdict::Unmodeled { .. } => LoadState::Unmodeled,
        }
    }

    pub fn is_exceeded(&self) -> bool {
        matches!(self, DeflectionVerdict::Exceeds { .. })
    }

    pub fn is_unmodeled(&self) -> bool {
        matches!(self, DeflectionVerdict::Unmodeled { .. })
    }

    pub fn confidence(&self) -> Option<&Confidence> {
        match self {
            DeflectionVerdict::Within { confidence, .. }
            | DeflectionVerdict::Exceeds { confidence, .. } => Some(confidence),
            DeflectionVerdict::Unmodeled { .. } => None,
        }
    }

    pub fn unmodeled_reason(&self) -> Option<&UnmodeledReason> {
        match self {
            DeflectionVerdict::Unmodeled { reason } => Some(reason),
            _ => None,
        }
    }

    pub fn as_criterion_status(&self) -> CriterionStatus<'_> {
        let (state, peak, range) = match self {
            DeflectionVerdict::Within {
                peak_mm, evidence, ..
            } => (
                LoadState::Within,
                Some(*peak_mm),
                option_range(&evidence.sample_range),
            ),
            DeflectionVerdict::Exceeds {
                peak_mm, evidence, ..
            } => (
                LoadState::Exceeds,
                Some(*peak_mm),
                option_range(&evidence.sample_range),
            ),
            DeflectionVerdict::Unmodeled { .. } => (LoadState::Unmodeled, None, None),
        };
        CriterionStatus {
            kind: CriterionKind::Deflection,
            state,
            confidence: self.confidence(),
            unmodeled_reason: self.unmodeled_reason(),
            sample_range: range,
            display_peak: peak,
            unit: CriterionKind::Deflection.unit(),
            exceeded: (state == LoadState::Exceeds).then(ExceededCriterion::deflection),
        }
    }
}

/// Per-criterion exceedance label suitable for the export-gate error
/// message and MCP wire output. Replaces the
/// `(&'static str, ExceedsReason)` pair returned by the legacy
/// `ToolLoadReport::exceeded_toolpaths`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExceededCriterion {
    pub kind: CriterionKind,
    pub label: &'static str,
    pub reason_label: &'static str,
}

impl ExceededCriterion {
    pub fn chipload_burn() -> Self {
        Self {
            kind: CriterionKind::Chipload,
            label: "chipload",
            reason_label: "burn risk",
        }
    }

    pub fn chipload_breakage() -> Self {
        Self {
            kind: CriterionKind::Chipload,
            label: "chipload",
            reason_label: "breakage",
        }
    }

    pub fn power() -> Self {
        Self {
            kind: CriterionKind::Power,
            label: "power",
            reason_label: "spindle power",
        }
    }

    pub fn deflection() -> Self {
        Self {
            kind: CriterionKind::Deflection,
            label: "deflection",
            reason_label: "stiffness",
        }
    }

    pub fn drill_chip_welding() -> Self {
        Self {
            kind: CriterionKind::DrillChipWelding,
            label: "chip welding",
            reason_label: "deep hole",
        }
    }

    pub fn drill_peck_adequacy() -> Self {
        Self {
            kind: CriterionKind::DrillPeckAdequacy,
            label: "peck depth",
            reason_label: "peck too deep",
        }
    }

    pub fn drill_plunge_feed() -> Self {
        Self {
            kind: CriterionKind::DrillPlungeFeed,
            label: "plunge feed",
            reason_label: "breakage",
        }
    }

    /// `"low"` / `"high"` for the chipload burn / breakage sides,
    /// `None` for the single-sided gates. Feeds `ExceedsEntry.side`
    /// in the report summary.
    pub fn side_label(&self) -> Option<&'static str> {
        if *self == Self::chipload_burn() {
            Some("low")
        } else if *self == Self::chipload_breakage() {
            Some("high")
        } else {
            None
        }
    }
}

fn option_range(range: &Range<usize>) -> Option<Range<usize>> {
    if range.is_empty() {
        None
    } else {
        Some(range.clone())
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

    #[test]
    fn modeled_count_ignores_unmodeled() {
        let v = ToolpathLoadVerdict {
            toolpath_id: ToolpathId(0),
            chipload: ChiploadVerdict::Within {
                approach_to_min: None,
                approach_to_max: ChiploadMetric {
                    observed_mm_per_tooth: 0.05,
                    statistic: ChiploadStatistic::PeakInRange,
                    evidence: SampleEvidence::empty(),
                    bounds: ChipBounds {
                        min_mm_per_tooth: Some(0.038),
                        max_mm_per_tooth: 0.07,
                        source: ChipBoundsSource::VendorLut,
                    },
                },
                confidence: Confidence::Validated,
                entry_spikes: Vec::new(),
                burn_advisory: None,
            },
            power: PowerVerdict::Unmodeled {
                reason: UnmodeledReason::SimulationRequired,
            },
            deflection: DeflectionVerdict::Within {
                peak_mm: 3.5,
                bounds: DeflectionBounds {
                    validated_within_mm: 0.050,
                    exceeds_mm: 0.200,
                },
                evidence: SampleEvidence::empty(),
                confidence: Confidence::Validated,
                entry_spike: None,
            },
            drill_gates: None,
            modulation_summary: None,
        };
        assert_eq!(v.modeled_count(), 2);
        assert!(!v.any_exceeded());
        assert!(v.any_unmodeled());
    }

    /// Regression: `Confidence::Approximate(String)` and
    /// `UnmodeledReason::CutterModeUnsupported(String)` are newtype variants
    /// inside internally-tagged enums; without `content = "detail"` serde
    /// fails at runtime and the MCP layer silently returned `null`.
    #[test]
    fn report_serializes_with_string_carrying_variants() {
        let r = ToolLoadReport {
            per_toolpath: vec![ToolpathLoadVerdict {
                toolpath_id: ToolpathId(0),
                chipload: ChiploadVerdict::Within {
                    approach_to_min: None,
                    approach_to_max: ChiploadMetric {
                        observed_mm_per_tooth: 0.05,
                        statistic: ChiploadStatistic::PeakInRange,
                        evidence: SampleEvidence::empty(),
                        bounds: ChipBounds {
                            min_mm_per_tooth: Some(0.038),
                            max_mm_per_tooth: 0.07,
                            source: ChipBoundsSource::VendorLut,
                        },
                    },
                    confidence: Confidence::Approximate("isotropic Kc only".to_owned()),
                    entry_spikes: Vec::new(),
                    burn_advisory: None,
                },
                power: PowerVerdict::Unmodeled {
                    reason: UnmodeledReason::CutterModeUnsupported("v-bit tip".to_owned()),
                },
                deflection: DeflectionVerdict::Within {
                    peak_mm: 4.5,
                    bounds: DeflectionBounds {
                        validated_within_mm: 0.050,
                        exceeds_mm: 0.200,
                    },
                    evidence: SampleEvidence::empty(),
                    confidence: Confidence::Approximate("L/D in 4-6 range".to_owned()),
                    entry_spike: None,
                },
                drill_gates: None,
                modulation_summary: None,
            }],
        };
        let v = serde_json::to_value(&r).expect("must round-trip");
        let s = serde_json::to_string(&v).unwrap();
        assert!(s.contains("isotropic Kc only"), "lost detail string: {s}");
        assert!(s.contains("v-bit tip"), "lost detail string: {s}");
        assert!(s.contains("L/D in 4-6 range"), "lost detail string: {s}");
    }

    #[test]
    fn report_collects_exceeded_reasons() {
        let r = ToolLoadReport {
            per_toolpath: vec![
                ToolpathLoadVerdict {
                    toolpath_id: ToolpathId(0),
                    chipload: ChiploadVerdict::Within {
                        approach_to_min: None,
                        approach_to_max: ChiploadMetric {
                            observed_mm_per_tooth: 0.05,
                            statistic: ChiploadStatistic::PeakInRange,
                            evidence: SampleEvidence::empty(),
                            bounds: ChipBounds {
                                min_mm_per_tooth: Some(0.038),
                                max_mm_per_tooth: 0.07,
                                source: ChipBoundsSource::VendorLut,
                            },
                        },
                        confidence: Confidence::Validated,
                        entry_spikes: Vec::new(),
                        burn_advisory: None,
                    },
                    power: PowerVerdict::Unmodeled {
                        reason: UnmodeledReason::NotImplemented("phase 1b".to_owned()),
                    },
                    deflection: DeflectionVerdict::Exceeds {
                        peak_mm: 8.5,
                        bounds: DeflectionBounds {
                            validated_within_mm: 0.050,
                            exceeds_mm: 0.200,
                        },
                        evidence: SampleEvidence::empty(),
                        confidence: Confidence::Validated,
                    },
                    drill_gates: None,
                    modulation_summary: None,
                },
                ToolpathLoadVerdict {
                    toolpath_id: ToolpathId(1),
                    chipload: ChiploadVerdict::Within {
                        approach_to_min: None,
                        approach_to_max: ChiploadMetric {
                            observed_mm_per_tooth: 0.04,
                            statistic: ChiploadStatistic::PeakInRange,
                            evidence: SampleEvidence::empty(),
                            bounds: ChipBounds {
                                min_mm_per_tooth: Some(0.038),
                                max_mm_per_tooth: 0.07,
                                source: ChipBoundsSource::VendorLut,
                            },
                        },
                        confidence: Confidence::Validated,
                        entry_spikes: Vec::new(),
                        burn_advisory: None,
                    },
                    power: PowerVerdict::Unmodeled {
                        reason: UnmodeledReason::NotImplemented("phase 1b".to_owned()),
                    },
                    deflection: DeflectionVerdict::Within {
                        peak_mm: 2.5,
                        bounds: DeflectionBounds {
                            validated_within_mm: 0.050,
                            exceeds_mm: 0.200,
                        },
                        evidence: SampleEvidence::empty(),
                        confidence: Confidence::Validated,
                        entry_spike: None,
                    },
                    drill_gates: None,
                    modulation_summary: None,
                },
            ],
        };
        assert!(r.any_exceeded());
        let exceeded = r.exceeded_criteria();
        assert_eq!(exceeded.len(), 1);
        assert_eq!(exceeded[0].0, ToolpathId(0));
        assert_eq!(exceeded[0].1.len(), 1);
        assert_eq!(exceeded[0].1[0], ExceededCriterion::deflection());
    }

    /// F1.7 (2026-06-10) — drill gates join `criteria()` and therefore
    /// export gating: a Critical drill exceedance trips `any_exceeded`
    /// (blocking export like a milling trip), an all-within drill
    /// verdict buckets `within` (gates WERE evaluated), and the
    /// milling-N/A drill special case in `any_unmodeled` still holds.
    #[test]
    fn drill_gates_participate_in_criteria_and_export_gating() {
        use crate::tool_load::drill_gates::{
            DrillGateOutcome, DrillGateSeverity, DrillGatesVerdict,
        };
        let na = || UnmodeledReason::NotApplicableForOp("drill cycle".to_owned());
        let drill_verdict = |plunge: DrillGateOutcome| ToolpathLoadVerdict {
            toolpath_id: ToolpathId(0),
            chipload: ChiploadVerdict::Unmodeled { reason: na() },
            power: PowerVerdict::Unmodeled { reason: na() },
            deflection: DeflectionVerdict::Unmodeled { reason: na() },
            drill_gates: Some(DrillGatesVerdict {
                chip_welding: DrillGateOutcome::Within {
                    observed: 0.5,
                    threshold: 8.0,
                    envelope_lo: None,
                    envelope_hi: None,
                },
                peck_adequacy: DrillGateOutcome::Within {
                    observed: 0.3,
                    threshold: 6.0,
                    envelope_lo: None,
                    envelope_hi: None,
                },
                plunge_feed: plunge,
            }),
            modulation_summary: None,
        };

        let healthy = drill_verdict(DrillGateOutcome::Within {
            observed: 100.0,
            threshold: 50.0,
            envelope_lo: Some(50.0),
            envelope_hi: Some(400.0),
        });
        assert!(!healthy.any_exceeded());
        assert!(
            !healthy.any_unmodeled(),
            "milling-N/A + drill gates stays neutral, not 'missing evidence'"
        );
        assert_eq!(healthy.modeled_count(), 3, "three drill criteria modeled");

        let critical = drill_verdict(DrillGateOutcome::Exceeds {
            observed: 500.0,
            threshold: 400.0,
            severity: DrillGateSeverity::Critical,
            envelope_lo: Some(50.0),
            envelope_hi: Some(400.0),
        });
        assert!(critical.any_exceeded(), "Critical drill gate must block");
        let labels: Vec<&str> = critical
            .exceeded_criteria()
            .iter()
            .map(|e| e.label)
            .collect();
        assert_eq!(labels, vec!["plunge feed"]);

        let elevated = drill_verdict(DrillGateOutcome::Exceeds {
            observed: 20.0,
            threshold: 50.0,
            severity: DrillGateSeverity::Elevated,
            envelope_lo: Some(50.0),
            envelope_hi: Some(400.0),
        });
        assert!(
            !elevated.any_exceeded(),
            "Elevated is a warning band, not an export blocker"
        );

        // Summary buckets: healthy drill TP rolls up `within` (the
        // gates were evaluated), critical rolls up `exceeds`.
        let report = ToolLoadReport {
            per_toolpath: vec![healthy, critical],
        };
        let s = report.summary(|_| None);
        assert_eq!(s.within, 1);
        assert_eq!(s.exceeds, 1);
        assert_eq!(s.not_applicable, 0);
    }

    /// F.8 — toolpaths whose every gate reports `NotApplicableForOp`
    /// bucket into `summary.not_applicable`, not `fully_unmodeled`.
    /// Mixed-reason toolpaths (one gate N/A, one gate `Unmodeled` for a
    /// measurable reason) still roll up as `fully_unmodeled` so the
    /// "what needs operator action?" question gets the right answer.
    #[test]
    fn summary_buckets_not_applicable_separately_from_fully_unmodeled() {
        let r = ToolLoadReport {
            per_toolpath: vec![
                // Drill cycle — every gate N/A.
                ToolpathLoadVerdict {
                    toolpath_id: ToolpathId(0),
                    chipload: ChiploadVerdict::Unmodeled {
                        reason: UnmodeledReason::NotApplicableForOp("drill cycle".to_owned()),
                    },
                    power: PowerVerdict::Unmodeled {
                        reason: UnmodeledReason::NotApplicableForOp("drill cycle".to_owned()),
                    },
                    deflection: DeflectionVerdict::Unmodeled {
                        reason: UnmodeledReason::NotApplicableForOp("drill cycle".to_owned()),
                    },
                    drill_gates: None,
                    modulation_summary: None,
                },
                // Sim wasn't run yet — every gate `SimulationRequired`.
                // Operator action: run the sim.
                ToolpathLoadVerdict {
                    toolpath_id: ToolpathId(1),
                    chipload: ChiploadVerdict::Unmodeled {
                        reason: UnmodeledReason::SimulationRequired,
                    },
                    power: PowerVerdict::Unmodeled {
                        reason: UnmodeledReason::SimulationRequired,
                    },
                    deflection: DeflectionVerdict::Unmodeled {
                        reason: UnmodeledReason::SimulationRequired,
                    },
                    drill_gates: None,
                    modulation_summary: None,
                },
                // Mixed: one gate N/A, one needs sim. Operator still
                // has an action item, so this rolls up as
                // `fully_unmodeled` (not `not_applicable`).
                ToolpathLoadVerdict {
                    toolpath_id: ToolpathId(2),
                    chipload: ChiploadVerdict::Unmodeled {
                        reason: UnmodeledReason::NotApplicableForOp("drill cycle".to_owned()),
                    },
                    power: PowerVerdict::Unmodeled {
                        reason: UnmodeledReason::SimulationRequired,
                    },
                    deflection: DeflectionVerdict::Unmodeled {
                        reason: UnmodeledReason::NotApplicableForOp("drill cycle".to_owned()),
                    },
                    drill_gates: None,
                    modulation_summary: None,
                },
            ],
        };
        let s = r.summary(|_| None);
        assert_eq!(s.total_toolpaths, 3);
        assert_eq!(s.within, 0);
        assert_eq!(s.exceeds, 0);
        assert_eq!(
            s.not_applicable, 1,
            "only the all-N/A toolpath should land in not_applicable"
        );
        assert_eq!(
            s.fully_unmodeled, 2,
            "both the all-SimulationRequired and the mixed toolpath should land in fully_unmodeled"
        );
    }

    /// F.11 — `summary()` resolves `toolpath_name` for every
    /// `ExceedsEntry` via the supplied closure. Empty string when the
    /// closure returns `None` (e.g. an id that's been deleted between
    /// the report's capture and the summary fold).
    #[test]
    fn summary_resolves_toolpath_name_into_exceeds_breakdown() {
        let r = ToolLoadReport {
            per_toolpath: vec![ToolpathLoadVerdict {
                toolpath_id: ToolpathId(42),
                chipload: ChiploadVerdict::Exceeds {
                    side: ChipSide::High,
                    triggering: ChiploadMetric {
                        observed_mm_per_tooth: 0.20,
                        statistic: ChiploadStatistic::PeakHigh,
                        evidence: SampleEvidence::at(0),
                        bounds: ChipBounds {
                            min_mm_per_tooth: Some(0.038),
                            max_mm_per_tooth: 0.07,
                            source: ChipBoundsSource::VendorLut,
                        },
                    },
                    confidence: Confidence::Validated,
                },
                power: PowerVerdict::Unmodeled {
                    reason: UnmodeledReason::SimulationRequired,
                },
                deflection: DeflectionVerdict::Within {
                    peak_mm: 0.020,
                    bounds: DeflectionBounds {
                        validated_within_mm: 0.050,
                        exceeds_mm: 0.200,
                    },
                    evidence: SampleEvidence::empty(),
                    confidence: Confidence::Validated,
                    entry_spike: None,
                },
                drill_gates: None,
                modulation_summary: None,
            }],
        };
        // Resolver hit — name flows into the entry.
        let s = r.summary(|id| (id == ToolpathId(42)).then(|| "TP3 Adaptive Rough".to_owned()));
        assert_eq!(s.exceeds_breakdown.len(), 1);
        let e = &s.exceeds_breakdown[0];
        assert_eq!(e.toolpath_id, ToolpathId(42));
        assert_eq!(e.toolpath_name, "TP3 Adaptive Rough");
        assert_eq!(e.gate, "chipload");
        assert_eq!(e.side.as_deref(), Some("high"));

        // Resolver returns None — name collapses to empty (matches the
        // `#[serde(default)]` round-trip behavior).
        let s = r.summary(|_| None);
        assert_eq!(s.exceeds_breakdown[0].toolpath_name, "");
    }

    // ── Typed verdict scaffolding (Step 7a) ─────────────────────────

    fn chip_bounds_with_min() -> ChipBounds {
        ChipBounds {
            min_mm_per_tooth: Some(0.038),
            max_mm_per_tooth: 0.07,
            source: ChipBoundsSource::VendorLut,
        }
    }

    fn chip_bounds_no_min() -> ChipBounds {
        ChipBounds {
            min_mm_per_tooth: None,
            max_mm_per_tooth: 0.07,
            source: ChipBoundsSource::VendorLut,
        }
    }

    fn deflection_bounds() -> DeflectionBounds {
        DeflectionBounds {
            validated_within_mm: 0.050,
            exceeds_mm: 0.200,
        }
    }

    #[test]
    fn chipload_verdict_state_maps_to_load_state() {
        let within = ChiploadVerdict::Within {
            approach_to_min: None,
            approach_to_max: ChiploadMetric {
                observed_mm_per_tooth: 0.06,
                statistic: ChiploadStatistic::PeakInRange,
                evidence: SampleEvidence::at(7),
                bounds: chip_bounds_with_min(),
            },
            confidence: Confidence::Validated,
            entry_spikes: Vec::new(),
            burn_advisory: None,
        };
        assert_eq!(within.state(), LoadState::Within);
        assert!(!within.is_exceeded());
        assert!(!within.is_unmodeled());

        let exceeds = ChiploadVerdict::Exceeds {
            side: ChipSide::Low,
            triggering: ChiploadMetric {
                observed_mm_per_tooth: 0.012,
                statistic: ChiploadStatistic::MedianLow,
                evidence: SampleEvidence::at_with_stat(3, ChiploadStatistic::MedianLow),
                bounds: chip_bounds_with_min(),
            },
            confidence: Confidence::Validated,
        };
        assert_eq!(exceeds.state(), LoadState::Exceeds);
        assert!(exceeds.is_exceeded());

        let unmodeled = ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        };
        assert_eq!(unmodeled.state(), LoadState::Unmodeled);
        assert!(unmodeled.is_unmodeled());
    }

    #[test]
    fn chipload_within_with_no_min_round_trips() {
        // Anchors the "LUT row has only `chip_load_max_mm`" case the
        // chipload evaluator carries through to Step 7d.
        let v = ChiploadVerdict::Within {
            approach_to_min: None,
            approach_to_max: ChiploadMetric {
                observed_mm_per_tooth: 0.06,
                statistic: ChiploadStatistic::PeakInRange,
                evidence: SampleEvidence::empty(),
                bounds: chip_bounds_no_min(),
            },
            confidence: Confidence::Validated,
            entry_spikes: Vec::new(),
            burn_advisory: None,
        };
        let json = serde_json::to_string(&v).expect("ser");
        let back: ChiploadVerdict = serde_json::from_str(&json).expect("de");
        assert_eq!(back, v);
    }

    #[test]
    fn chipload_exceeds_low_carries_median_statistic_in_status() {
        // Pins the design corollary that BurnRisk (Low side) is driven
        // by MedianLow today, not PeakLow. Step 7d's evaluator must
        // populate `triggering.statistic` accordingly.
        let v = ChiploadVerdict::Exceeds {
            side: ChipSide::Low,
            triggering: ChiploadMetric {
                observed_mm_per_tooth: 0.012,
                statistic: ChiploadStatistic::MedianLow,
                evidence: SampleEvidence::at_with_stat(11, ChiploadStatistic::MedianLow),
                bounds: chip_bounds_with_min(),
            },
            confidence: Confidence::Validated,
        };
        let status = v.as_criterion_status();
        assert_eq!(status.kind, CriterionKind::Chipload);
        assert_eq!(status.state, LoadState::Exceeds);
        assert_eq!(status.unit, "mm/tooth");
        assert_eq!(status.display_peak, Some(0.012));
        assert_eq!(status.sample_range, Some(11..12));
    }

    #[test]
    fn power_verdict_within_carries_available_kw() {
        let v = PowerVerdict::Within {
            peak_kw: 0.4,
            available_kw: 0.71,
            evidence: SampleEvidence::at(2),
            confidence: Confidence::Approximate("isotropic Kc".to_owned()),
            entry_spike: None,
        };
        match &v {
            PowerVerdict::Within { available_kw, .. } => {
                assert!((*available_kw - 0.71).abs() < 1e-9);
            }
            other => panic!("expected Within, got {other:?}"),
        }
        let status = v.as_criterion_status();
        assert_eq!(status.kind, CriterionKind::Power);
        assert_eq!(status.state, LoadState::Within);
        assert_eq!(status.unit, "kW");
        assert_eq!(status.display_peak, Some(0.4));
        assert_eq!(status.sample_range, Some(2..3));
    }

    #[test]
    fn power_exceeds_carries_available_kw_and_round_trips() {
        let v = PowerVerdict::Exceeds {
            peak_kw: 1.5,
            available_kw: 0.5,
            evidence: SampleEvidence::at(0),
            confidence: Confidence::Validated,
        };
        let json = serde_json::to_string(&v).expect("ser");
        let back: PowerVerdict = serde_json::from_str(&json).expect("de");
        assert_eq!(back, v);
        match back {
            PowerVerdict::Exceeds { available_kw, .. } => {
                assert!((available_kw - 0.5).abs() < 1e-9);
            }
            other => panic!("expected Exceeds, got {other:?}"),
        }
    }

    #[test]
    fn deflection_bounds_round_trip_keeps_both_thresholds() {
        let v = DeflectionVerdict::Within {
            peak_mm: 0.080,
            bounds: deflection_bounds(),
            evidence: SampleEvidence::at(5),
            confidence: Confidence::Approximate("approximate band".to_owned()),
            entry_spike: None,
        };
        let json = serde_json::to_string(&v).expect("ser");
        let back: DeflectionVerdict = serde_json::from_str(&json).expect("de");
        assert_eq!(back, v);
        match back {
            DeflectionVerdict::Within { bounds, .. } => {
                assert!((bounds.validated_within_mm - 0.050).abs() < 1e-9);
                assert!((bounds.exceeds_mm - 0.200).abs() < 1e-9);
            }
            other => panic!("expected Within, got {other:?}"),
        }
    }

    #[test]
    fn empty_sample_range_yields_none_in_status() {
        let v = PowerVerdict::Within {
            peak_kw: 0.2,
            available_kw: 0.71,
            evidence: SampleEvidence::empty(),
            confidence: Confidence::Validated,
            entry_spike: None,
        };
        let s = v.as_criterion_status();
        assert!(s.sample_range.is_none());
    }

    #[test]
    fn unmodeled_status_has_no_peak_no_range_no_confidence() {
        let v = DeflectionVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        };
        let s = v.as_criterion_status();
        assert_eq!(s.kind, CriterionKind::Deflection);
        assert_eq!(s.state, LoadState::Unmodeled);
        assert!(s.display_peak.is_none());
        assert!(s.sample_range.is_none());
        assert!(s.confidence.is_none());
    }

    #[test]
    fn criterion_kind_label_and_unit() {
        assert_eq!(CriterionKind::Chipload.label(), "chipload");
        assert_eq!(CriterionKind::Power.label(), "power");
        assert_eq!(CriterionKind::Deflection.label(), "deflection");
        assert_eq!(CriterionKind::Chipload.unit(), "mm/tooth");
        assert_eq!(CriterionKind::Power.unit(), "kW");
        assert_eq!(CriterionKind::Deflection.unit(), "mm");
    }

    /// Wire-format snapshot for `ToolLoadReport` after the typed-verdict
    /// migration (G16 Step 7). The MCP `get_tool_load_report` tool serves
    /// this JSON; consumers should be able to read the kind tags and the
    /// typed payloads without surprises.
    #[test]
    fn report_serializes_typed_verdict_wire_format() {
        let r = ToolLoadReport {
            per_toolpath: vec![ToolpathLoadVerdict {
                toolpath_id: ToolpathId(7),
                chipload: ChiploadVerdict::Exceeds {
                    side: ChipSide::Low,
                    triggering: ChiploadMetric {
                        observed_mm_per_tooth: 0.012,
                        statistic: ChiploadStatistic::MedianLow,
                        evidence: SampleEvidence::at_with_stat(11, ChiploadStatistic::MedianLow),
                        bounds: ChipBounds {
                            min_mm_per_tooth: Some(0.038),
                            max_mm_per_tooth: 0.07,
                            source: ChipBoundsSource::VendorLut,
                        },
                    },
                    confidence: Confidence::Validated,
                },
                power: PowerVerdict::Within {
                    peak_kw: 0.4,
                    available_kw: 0.71,
                    evidence: SampleEvidence::at(2),
                    confidence: Confidence::Approximate("isotropic Kc".to_owned()),
                    entry_spike: None,
                },
                deflection: DeflectionVerdict::Within {
                    peak_mm: 0.080,
                    bounds: DeflectionBounds {
                        validated_within_mm: 0.050,
                        exceeds_mm: 0.200,
                    },
                    evidence: SampleEvidence::at(5),
                    confidence: Confidence::Approximate("approximate band".to_owned()),
                    entry_spike: None,
                },
                drill_gates: None,
                modulation_summary: None,
            }],
        };
        let s = serde_json::to_string(&r).expect("serialize");
        // Chipload payload — ChipSide + ChiploadStatistic + bounds.
        assert!(s.contains("\"side\":\"low\""), "missing chipload side: {s}");
        assert!(
            s.contains("\"statistic\":\"median_low\""),
            "missing chipload statistic: {s}"
        );
        assert!(
            s.contains("\"min_mm_per_tooth\":0.038"),
            "missing LUT min: {s}"
        );
        // Power payload — both arms must surface available_kw.
        assert!(
            s.contains("\"available_kw\":0.71"),
            "missing power available_kw: {s}"
        );
        // Deflection payload — both thresholds present.
        assert!(
            s.contains("\"validated_within_mm\":0.05"),
            "missing deflection within bound: {s}"
        );
        assert!(
            s.contains("\"exceeds_mm\":0.2"),
            "missing deflection exceeds bound: {s}"
        );
        // Round-trips.
        let back: ToolLoadReport = serde_json::from_str(&s).expect("round-trip");
        assert_eq!(back.per_toolpath.len(), 1);
    }

    #[test]
    fn exceeded_criteria_returns_typed_labels() {
        let r = ToolLoadReport {
            per_toolpath: vec![ToolpathLoadVerdict {
                toolpath_id: ToolpathId(0),
                chipload: ChiploadVerdict::Exceeds {
                    side: ChipSide::High,
                    triggering: ChiploadMetric {
                        observed_mm_per_tooth: 0.20,
                        statistic: ChiploadStatistic::PeakHigh,
                        evidence: SampleEvidence::at(0),
                        bounds: ChipBounds {
                            min_mm_per_tooth: Some(0.038),
                            max_mm_per_tooth: 0.07,
                            source: ChipBoundsSource::VendorLut,
                        },
                    },
                    confidence: Confidence::Validated,
                },
                power: PowerVerdict::Unmodeled {
                    reason: UnmodeledReason::SimulationRequired,
                },
                deflection: DeflectionVerdict::Within {
                    peak_mm: 0.020,
                    bounds: DeflectionBounds {
                        validated_within_mm: 0.050,
                        exceeds_mm: 0.200,
                    },
                    evidence: SampleEvidence::empty(),
                    confidence: Confidence::Validated,
                    entry_spike: None,
                },
                drill_gates: None,
                modulation_summary: None,
            }],
        };
        let exceeded = r.exceeded_criteria();
        assert_eq!(exceeded.len(), 1);
        assert_eq!(exceeded[0].0, ToolpathId(0));
        assert_eq!(exceeded[0].1.len(), 1);
        assert_eq!(exceeded[0].1[0], ExceededCriterion::chipload_breakage());
    }

    #[test]
    fn exceeded_criterion_constructors_label_and_reason() {
        assert_eq!(
            ExceededCriterion::chipload_burn().kind,
            CriterionKind::Chipload
        );
        assert_eq!(ExceededCriterion::chipload_burn().reason_label, "burn risk");
        assert_eq!(
            ExceededCriterion::chipload_breakage().reason_label,
            "breakage"
        );
        assert_eq!(ExceededCriterion::power().reason_label, "spindle power");
        assert_eq!(ExceededCriterion::deflection().reason_label, "stiffness");
    }

    #[test]
    fn side_label_maps_chipload_sides_only() {
        assert_eq!(ExceededCriterion::chipload_burn().side_label(), Some("low"));
        assert_eq!(
            ExceededCriterion::chipload_breakage().side_label(),
            Some("high")
        );
        assert_eq!(ExceededCriterion::power().side_label(), None);
        assert_eq!(ExceededCriterion::deflection().side_label(), None);
    }

    /// Phase 6 task 5 sentry — the gating tier derives from
    /// `criteria()`: every `CriterionStatus` carries `exceeded: Some`
    /// exactly when its state is `Exceeds`, and `exceeded_criteria()`
    /// is precisely the `Some`s in criteria order. If a gate ever sets
    /// `Exceeds` without an `exceeded` label (or vice versa), export
    /// gating silently diverges from the displayed state — fail here.
    #[test]
    fn gating_tier_derives_from_criteria() {
        let v = ToolpathLoadVerdict {
            toolpath_id: ToolpathId(7),
            chipload: ChiploadVerdict::Exceeds {
                side: ChipSide::Low,
                triggering: ChiploadMetric {
                    observed_mm_per_tooth: 0.01,
                    statistic: ChiploadStatistic::MedianLow,
                    evidence: SampleEvidence::at(3),
                    bounds: ChipBounds {
                        min_mm_per_tooth: Some(0.038),
                        max_mm_per_tooth: 0.07,
                        source: ChipBoundsSource::VendorLut,
                    },
                },
                confidence: Confidence::Validated,
            },
            power: PowerVerdict::Exceeds {
                peak_kw: 2.5,
                available_kw: 1.5,
                evidence: SampleEvidence::at(4),
                confidence: Confidence::Validated,
            },
            deflection: DeflectionVerdict::Unmodeled {
                reason: UnmodeledReason::SimulationRequired,
            },
            drill_gates: None,
            modulation_summary: None,
        };
        for status in v.criteria() {
            assert_eq!(
                status.exceeded.is_some(),
                status.state == LoadState::Exceeds,
                "criterion {:?}: exceeded label must track Exceeds state",
                status.kind
            );
            if let Some(ec) = &status.exceeded {
                assert_eq!(ec.kind, status.kind);
            }
        }
        assert_eq!(
            v.exceeded_criteria(),
            vec![
                ExceededCriterion::chipload_burn(),
                ExceededCriterion::power()
            ]
        );
    }

    // ── F.8 + F.11 — summary() bucketing and name resolution ────────

    /// Helper: build a `ToolpathLoadVerdict` whose every gate is
    /// `NotApplicableForOp` — the F.8 drill-cycle shape.
    fn vd_all_not_applicable(id: usize) -> ToolpathLoadVerdict {
        let reason = UnmodeledReason::NotApplicableForOp(
            "drill cycle — no continuous engagement".to_owned(),
        );
        ToolpathLoadVerdict {
            toolpath_id: ToolpathId(id),
            chipload: ChiploadVerdict::Unmodeled {
                reason: reason.clone(),
            },
            power: PowerVerdict::Unmodeled {
                reason: reason.clone(),
            },
            deflection: DeflectionVerdict::Unmodeled { reason },
            drill_gates: None,
            modulation_summary: None,
        }
    }

    /// Helper: every gate `Unmodeled(SimulationRequired)` — the
    /// fully-unmodeled bucket where operator action *would* help.
    fn vd_all_sim_required(id: usize) -> ToolpathLoadVerdict {
        ToolpathLoadVerdict {
            toolpath_id: ToolpathId(id),
            chipload: ChiploadVerdict::Unmodeled {
                reason: UnmodeledReason::SimulationRequired,
            },
            power: PowerVerdict::Unmodeled {
                reason: UnmodeledReason::SimulationRequired,
            },
            deflection: DeflectionVerdict::Unmodeled {
                reason: UnmodeledReason::SimulationRequired,
            },
            drill_gates: None,
            modulation_summary: None,
        }
    }

    #[test]
    fn summary_separates_not_applicable_from_fully_unmodeled() {
        let r = ToolLoadReport {
            per_toolpath: vec![
                vd_all_not_applicable(0), // drill — doesn't apply
                vd_all_not_applicable(1), // drill — doesn't apply
                vd_all_sim_required(2),   // failed sim — fully unmodeled
            ],
        };
        let s = r.summary(|_| None);
        assert_eq!(s.total_toolpaths, 3);
        assert_eq!(s.not_applicable, 2);
        assert_eq!(s.fully_unmodeled, 1);
        assert_eq!(s.within, 0);
        assert_eq!(s.exceeds, 0);
    }

    /// Verdicts with at least one gate genuinely modeled still roll
    /// up as `Within` — the not-applicable bucket is the all-gate case.
    #[test]
    fn summary_not_applicable_requires_all_gates_to_not_apply() {
        let mut v = vd_all_not_applicable(0);
        // Override one gate with a real modeled reading.
        v.deflection = DeflectionVerdict::Within {
            peak_mm: 0.02,
            bounds: DeflectionBounds {
                validated_within_mm: 0.050,
                exceeds_mm: 0.200,
            },
            evidence: SampleEvidence::empty(),
            confidence: Confidence::Validated,
            entry_spike: None,
        };
        let r = ToolLoadReport {
            per_toolpath: vec![v],
        };
        let s = r.summary(|_| None);
        assert_eq!(s.not_applicable, 0);
        assert_eq!(s.fully_unmodeled, 0);
        assert_eq!(s.within, 1);
    }

    #[test]
    fn summary_populates_exceeds_breakdown_toolpath_name() {
        let r = ToolLoadReport {
            per_toolpath: vec![ToolpathLoadVerdict {
                toolpath_id: ToolpathId(42),
                chipload: ChiploadVerdict::Within {
                    approach_to_min: None,
                    approach_to_max: ChiploadMetric {
                        observed_mm_per_tooth: 0.05,
                        statistic: ChiploadStatistic::PeakInRange,
                        evidence: SampleEvidence::empty(),
                        bounds: ChipBounds {
                            min_mm_per_tooth: Some(0.038),
                            max_mm_per_tooth: 0.07,
                            source: ChipBoundsSource::VendorLut,
                        },
                    },
                    confidence: Confidence::Validated,
                    entry_spikes: Vec::new(),
                    burn_advisory: None,
                },
                power: PowerVerdict::Unmodeled {
                    reason: UnmodeledReason::SimulationRequired,
                },
                deflection: DeflectionVerdict::Exceeds {
                    peak_mm: 0.300,
                    bounds: DeflectionBounds {
                        validated_within_mm: 0.050,
                        exceeds_mm: 0.200,
                    },
                    evidence: SampleEvidence::empty(),
                    confidence: Confidence::Validated,
                },
                drill_gates: None,
                modulation_summary: None,
            }],
        };
        let s = r.summary(|id| {
            if id == ToolpathId(42) {
                Some("TP 5: Front pocket".to_owned())
            } else {
                None
            }
        });
        assert_eq!(s.exceeds_breakdown.len(), 1);
        let entry = &s.exceeds_breakdown[0];
        assert_eq!(entry.toolpath_id, ToolpathId(42));
        assert_eq!(entry.gate, "deflection");
        assert_eq!(entry.toolpath_name, "TP 5: Front pocket");
    }

    /// Callers without a name resolver (legacy test fixtures, headless
    /// gcode export) get an empty `toolpath_name` rather than failing.
    #[test]
    fn summary_without_resolver_yields_empty_name() {
        let r = ToolLoadReport {
            per_toolpath: vec![ToolpathLoadVerdict {
                toolpath_id: ToolpathId(0),
                chipload: ChiploadVerdict::Exceeds {
                    side: ChipSide::High,
                    triggering: ChiploadMetric {
                        observed_mm_per_tooth: 0.20,
                        statistic: ChiploadStatistic::PeakHigh,
                        evidence: SampleEvidence::at(0),
                        bounds: ChipBounds {
                            min_mm_per_tooth: Some(0.038),
                            max_mm_per_tooth: 0.07,
                            source: ChipBoundsSource::VendorLut,
                        },
                    },
                    confidence: Confidence::Validated,
                },
                power: PowerVerdict::Unmodeled {
                    reason: UnmodeledReason::SimulationRequired,
                },
                deflection: DeflectionVerdict::Within {
                    peak_mm: 0.02,
                    bounds: DeflectionBounds {
                        validated_within_mm: 0.050,
                        exceeds_mm: 0.200,
                    },
                    evidence: SampleEvidence::empty(),
                    confidence: Confidence::Validated,
                    entry_spike: None,
                },
                drill_gates: None,
                modulation_summary: None,
            }],
        };
        let s = r.summary(|_| None);
        assert_eq!(s.exceeds_breakdown.len(), 1);
        assert_eq!(s.exceeds_breakdown[0].toolpath_name, "");
    }

    /// `UnmodeledReason::NotApplicableForOp` carries the operator-facing
    /// explanation string and round-trips through serde.
    #[test]
    fn not_applicable_for_op_round_trips() {
        let r = UnmodeledReason::NotApplicableForOp(
            "drill cycle — no continuous engagement".to_owned(),
        );
        let s = serde_json::to_string(&r).expect("ser");
        assert!(s.contains("not_applicable_for_op"), "tag missing: {s}");
        assert!(s.contains("drill cycle"), "detail missing: {s}");
        let back: UnmodeledReason = serde_json::from_str(&s).expect("de");
        assert_eq!(back, r);
    }
}
