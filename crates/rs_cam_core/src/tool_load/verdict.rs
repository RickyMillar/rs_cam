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
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

/// F-039 — Which physical constraint set the modulated feed for a
/// single cutting move (constrained-max solver). One of seven bound
/// types: the chipload band's upper edge, the deflection cap, the
/// power cap, the machine's hard feed cap, the kinematic-reach cap
/// from the F-034 / F-035 integrator, the chipload band's lower
/// edge (applied last), or Phase 3's geometric plunge-rate cap
/// (applied after all of them, on vertical-dominant moves only).
///
/// **This vocabulary describes the MODULATOR only.** It reports which
/// bound the per-move constrained-max solver hit. It deliberately does
/// not name Suggest's Step-9b rubbing-floor clamp, which is a
/// whole-recipe decision taken before any move exists — that lives on
/// [`crate::feeds::CommandedStage::clamped_to`] (Checkpoint K (d2),
/// option d2 over d1). Adding a variant for it here would mean
/// reporting a constraint the modulator never evaluated.
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
    /// Chipload band's lower edge bound the feed: the modulator's own
    /// per-move floor, `band.min × rpm × flutes`, applied AFTER
    /// aggressiveness scaling so feeds never drop below the matched
    /// row's minimum even at low aggressiveness.
    ///
    /// **Corrected at Checkpoint K (d2), 2026-08-13.** This doc used to
    /// call itself "the rubbing floor". It is not: the rubbing floor is
    /// [`crate::feeds::effective_rubbing_floor`], which is
    /// `min(RUBBING_FLOOR_MM_TOOTH, band.max)` — a different quantity,
    /// applied at a different stage, by a different component. On A-6's
    /// B3 reference case the two are **2.000× apart and point at
    /// opposite ends of the same band** (`band.min` 0.005763 vs the
    /// clamp's 0.011525 = `band.max`). A docstring that names a
    /// constant the code does not use is a lie a reader then cites.
    ChiploadMin,
    /// Phase 3 (2026-09-07). A vertical-dominant move was capped at the
    /// operation's own `plunge_rate` by GEOMETRY: the intent skip did not
    /// fire because the move carried no plunge tag.
    ///
    /// The modulator's other five bounds are lateral-cutting physics. This
    /// one is not: the classifier
    /// ([`crate::machine::kinematic_utilization::classify_move`]) reads the move's
    /// own vector, and a descent inside the plunge cone cuts on its centre,
    /// where the chipload band does not apply. The adaptive3d rough emits
    /// its step-down descents as plain cutting moves, so before this guard
    /// they were lifted to the lateral band maximum — 1807 mm/min against a
    /// 512 mm/min plunge rate on the wanaka front rough.
    ///
    /// The guard is a FLOOR UNDER THE LIFT, not a skip. A move that already
    /// sits at or below the plunge rate keeps the strategy's own binding, so
    /// this variant always means "the plunge rate LOWERED this feed".
    PlungeRate,
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
            BindingConstraint::PlungeRate => "plunge-rate",
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
/// the runtime [`crate::dressup::feed_modulation::ModulationStrategy`] enum;
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
    /// **S3 (2026-09-18) — the depth the cut actually took.**
    ///
    /// Post-simulation only. Before a simulation the depth is a value
    /// the engine chose and clamped, and it stays a rationale entry
    /// (`RationaleReason::RigidityFactor`); after one it is a measured
    /// quantity with a real population, like the three gates above it.
    /// Its bound is a rule of thumb, so the row reports and never
    /// refuses an export.
    pub depth: DepthVerdict,
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
    /// T1.1 — the stage-labelled record relating the chipload gate's
    /// number to the commanded one, with every stage's unit named.
    ///
    /// `Some` whenever the chipload gate reached a modelled verdict;
    /// `None` when it refused before matching a vendor row (an
    /// `Unmodeled` verdict has no stages to label) or for drill ops.
    ///
    /// Report-only and additive: no gate, threshold or severity reads
    /// it. See `crates/rs_cam_core/src/feeds/feed_explanation.rs` for the
    /// rule it is written under — it labels stages, it does not pick a
    /// winner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_explanation: Option<Box<crate::feeds::FeedExplanation>>,
    /// Phase 4 (2026-09-07) — the per-toolpath kinematic reading:
    /// utilization of the commanded feed, which constraint binds each
    /// move, the headroom a feed rise would release, and the
    /// plunge-class observation that backs `project.plunge_class_load`.
    ///
    /// `Some` only where the producer had the emitted toolpath, the
    /// machine kinematics and the operation's own `plunge_rate` in
    /// scope — today that is `gcode::project_load_report`, through
    /// [`crate::session::ProjectSession::kinematic_utilization_for`].
    /// The optimizer path leaves it `None`; see the note at
    /// [`crate::tool_load::evaluate_toolpath`].
    ///
    /// Report-only and additive: no gate, threshold or severity reads
    /// it, and nothing in export consumes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kinematic_utilization:
        Option<crate::machine::kinematic_utilization::ToolpathKinematicUtilization>,
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
    ///
    /// S1 (2026-09-18): a row for which
    /// [`CriterionStatus::is_known_absence`] is true does not count
    /// either. The gantry-push row is `Unmodeled` on every milling
    /// toolpath and always will be until a machine-side thrust rating
    /// exists (register T-10). Counting it here would refuse every
    /// export under `accept_unmodeled: false` and would prompt the
    /// operator to re-run a simulation that cannot change the answer.
    pub fn any_unmodeled(&self) -> bool {
        if self.drill_gates.is_some() && all_not_applicable(self) {
            return false;
        }
        self.criteria()
            .iter()
            .any(|s| s.state == LoadState::Unmodeled && !s.is_known_absence())
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
            all.push(d.chip_welding.as_criterion_status(
                CriterionKind::DrillChipWelding,
                d.cycle,
                d.population,
            ));
            all.push(d.peck_adequacy.as_criterion_status(
                CriterionKind::DrillPeckAdequacy,
                d.cycle,
                d.population,
            ));
            all.push(d.plunge_feed.as_criterion_status(
                CriterionKind::DrillPlungeFeed,
                d.cycle,
                d.population,
            ));
        }
        all
    }

    /// The milling gates only — chipload, power, deflection, depth of
    /// cut, and the gantry-push row. Used by `all_not_applicable` to decide the
    /// "drill cycle, milling gates don't apply" partition without the
    /// drill criteria muddying the test.
    ///
    /// S1 (2026-09-18) — the gantry-push row joins here, after
    /// deflection. `feeds::force::lateral_cutting_force` computes the
    /// push; no `MachineProfile` field states the thrust the gantry can
    /// deliver, so the row carries no number and no bound. It is
    /// [`gantry_push_criterion`], and it takes the drill arm when the
    /// three measured gates all took it — a drill has no lateral
    /// engagement, so the partition is unchanged.
    ///
    /// S3 (2026-09-18) — the depth-of-cut row joins after deflection
    /// and before the gantry row. It is a REAL gate with a real
    /// producer, so it counts in every tally the way the three above it
    /// do; only its bound's provenance stops it refusing an export.
    pub(crate) fn milling_criteria(&self) -> Vec<CriterionStatus<'_>> {
        let mut rows = vec![
            self.chipload.as_criterion_status(),
            self.power.as_criterion_status(),
            self.deflection.as_criterion_status(),
            self.depth.as_criterion_status(),
        ];
        // Read the drill answer off the gates that already decided it,
        // not off `drill_gates`: the optimizer path leaves `drill_gates`
        // `None` on a drill toolpath, and the row must still say
        // "doesn't apply" there.
        let plunge_only = rows.iter().all(|s| is_not_applicable(s.unmodeled_reason));
        rows.push(gantry_push_criterion(plunge_only));
        rows
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

    /// **The criterion tier as OWNED rows**, one per
    /// [`Self::criteria`] entry, in the same order.
    ///
    /// S4 (2026-09-18). [`CriterionStatus`] borrows the verdict it came
    /// from, so it cannot serialize, and S1 measured the consequence:
    /// `mcp_get_tool_load_report` serialises the typed verdict struct,
    /// so the criterion list — and with it every bound and every
    /// provenance — never reaches an MCP client. This is the shape that
    /// does. It copies; it derives nothing.
    pub fn criterion_rows(&self) -> Vec<CriterionRow> {
        self.criteria()
            .iter()
            .map(CriterionRow::from_status)
            .collect()
    }
}

/// **The owned form of [`CriterionStatus`], for the wire.**
///
/// Field for field the same row, with the borrows copied out:
/// `unit` becomes a `String`, and `confidence` and `sample_range` are
/// left off because no wire consumer reads them from this shape today
/// — the typed verdicts beside it already carry both.
///
/// Built only by [`ToolpathLoadVerdict::criterion_rows`], so the owned
/// row cannot drift from the borrowed one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
// `ExceededCriterion` holds `&'static str` fields, so its own
// `Deserialize` impl only exists for a `'static` input. Stating the
// bound here keeps the error at this line rather than at every call
// site: a reader deserializing a row needs a `&'static str` of JSON.
#[serde(bound(deserialize = "'de: 'static"))]
pub struct CriterionRow {
    pub kind: CriterionKind,
    pub state: LoadState,
    /// The bound `display_peak` was judged against, in `unit`.
    pub bound: Option<f64>,
    pub bound_source: Option<BoundSource>,
    pub unit: String,
    pub display_peak: Option<f64>,
    /// **X-VAC** — `None` means "not stated", never "zero".
    pub population: Option<GatePopulation>,
    pub unmodeled_reason: Option<UnmodeledReason>,
    pub exceeded: Option<ExceededCriterion>,
}

impl CriterionRow {
    /// Copy a borrowed row into an owned one.
    #[must_use]
    pub fn from_status(status: &CriterionStatus<'_>) -> Self {
        Self {
            kind: status.kind,
            state: status.state,
            bound: status.bound,
            bound_source: status.bound_source.clone(),
            unit: status.unit.to_owned(),
            display_peak: status.display_peak,
            population: status.population,
            unmodeled_reason: status.unmodeled_reason.cloned(),
            exceeded: status.exceeded.clone(),
        }
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

/// **The one operator-facing clause for the gantry-push absence.**
///
/// Held in a single place so the GUI, the CLI and the MCP print the
/// same words. It cites the register item, never a `planning/…` path:
/// those paths rot (`REVIEW_DESIGN` §6.3 counted 117 dead citations of
/// 231), and the register entry is what a reader can act on.
///
/// The blocker behind T-10 is a number, not code. `MachineProfile`
/// carries no axis-thrust field and no maker publishes a rating; the
/// ballpark figures span 20:1 across machine classes, so one constant
/// for every machine is not an option. Until a rating exists the row
/// states that, and states nothing else.
pub const GANTRY_PUSH_UNMODELED_CLAUSE: &str =
    "no machine-side thrust rating is published; register T-10";

/// The gantry row's clause on a plunge-only op. Word for word the one
/// the three milling gates use, so the four rows read as one decision.
pub const GANTRY_PUSH_NOT_APPLICABLE_CLAUSE: &str = "drill cycle — no continuous engagement";

/// `UnmodeledReason` carries a `String` (it deserializes over the MCP
/// wire), so the two reasons cannot be `const`. They are built once and
/// borrowed for `'static`, which coerces to any `CriterionStatus<'a>`.
static GANTRY_PUSH_NOT_IMPLEMENTED: LazyLock<UnmodeledReason> =
    LazyLock::new(|| UnmodeledReason::NotImplemented(GANTRY_PUSH_UNMODELED_CLAUSE.to_owned()));
static GANTRY_PUSH_NOT_APPLICABLE: LazyLock<UnmodeledReason> = LazyLock::new(|| {
    UnmodeledReason::NotApplicableForOp(GANTRY_PUSH_NOT_APPLICABLE_CLAUSE.to_owned())
});

/// The gantry-push row, in the one place that builds it.
///
/// S1 (2026-09-18). Every field that would carry evidence is `None`:
/// no peak, no population, no sample range, no exceedance. A row that
/// printed any of those would claim a judgement the crate cannot make
/// — defect class 3 (`RESUME_PLAN` §9), an absence rendering as a
/// reading.
///
/// `population: None` reads as "not stated", never as zero, so
/// [`CriterionStatus::is_vacuous`] stays false. Vacuity is X-VAC: a
/// gate that measured an EMPTY population. This row has no population
/// at all, and calling it vacuous would put it in the same bucket as a
/// gate whose filters ate its samples — a different finding with a
/// different remedy.
///
/// `plunge_only` is true for a drill cycle, which has no lateral
/// engagement to push against.
fn gantry_push_criterion(plunge_only: bool) -> CriterionStatus<'static> {
    CriterionStatus {
        kind: CriterionKind::GantryPush,
        state: LoadState::Unmodeled,
        confidence: None,
        unmodeled_reason: Some(if plunge_only {
            &GANTRY_PUSH_NOT_APPLICABLE
        } else {
            &GANTRY_PUSH_NOT_IMPLEMENTED
        }),
        sample_range: None,
        population: None,
        display_peak: None,
        unit: CriterionKind::GantryPush.unit(),
        // S4: no machine-side thrust rating exists, so there is no
        // bound and no source. A bound on a row nothing judged would be
        // the same defect the row was added to remove.
        bound: None,
        bound_source: None,
        exceeded: None,
    }
}

// ─────────────────────────────────────────────────────────────────────
// The typed verdicts. `ToolpathLoadVerdict` carries one per gate, and
// the GUI, the export layer and the MCP wire format all read them.
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
    /// S3 (2026-09-18) — the depth the cut actually took, measured as
    /// the peak `axial_engagement_mm` over the toolpath's cutting
    /// samples, against the machine rigidity rule of thumb. The bound
    /// is [`BoundSource::RigidityRuleOfThumb`], which does not gate an
    /// export. See [`crate::tool_load::depth`].
    DepthOfCut,
    /// S1 (2026-09-18) — the force the gantry must push to make the
    /// cut. Always `Unmodeled`; see [`GANTRY_PUSH_UNMODELED_CLAUSE`].
    GantryPush,
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
            CriterionKind::DepthOfCut => "depth of cut",
            CriterionKind::GantryPush => "gantry push",
            CriterionKind::DrillChipWelding => "chip welding",
            CriterionKind::DrillPeckAdequacy => "peck depth",
            CriterionKind::DrillPlungeFeed => "plunge feed",
        }
    }

    pub fn unit(self) -> &'static str {
        match self {
            CriterionKind::Chipload => "mm/tooth",
            CriterionKind::Power => "kW",
            CriterionKind::Deflection | CriterionKind::DepthOfCut => "mm",
            CriterionKind::GantryPush => "N",
            CriterionKind::DrillChipWelding | CriterionKind::DrillPeckAdequacy => "D/d",
            CriterionKind::DrillPlungeFeed => "mm/min per mm Ø",
        }
    }

    /// **True when this crate models the criterion not at all.** S1
    /// (2026-09-18).
    ///
    /// Not "the gate refused this time" — there is no gate, no bound
    /// and no producer, and no project input creates one. Today the
    /// only such kind is [`CriterionKind::GantryPush`]: the force is
    /// computed, and no `MachineProfile` field states the thrust to
    /// judge it against (register T-10).
    ///
    /// [`CriterionStatus::is_known_absence`] reads this to decide
    /// whether an `Unmodeled` row may prompt the operator. Remove a
    /// kind from this list on the day its model lands, and every
    /// counter starts counting it with no other change.
    pub fn is_unmodeled_by_design(self) -> bool {
        matches!(self, CriterionKind::GantryPush)
    }
}

/// **Where a criterion's bound came from, typed.**
///
/// A hover, a refusal line or a CLI row FORMATS this. It never retypes
/// the number. `REVIEW_DESIGN.md` §6.3 measured the reason: doc
/// comments under `crates/` cite 231 `planning/…` paths and 117 of them
/// resolve to nothing, and the same rot has already hit two
/// hard-coded provenance strings in this file — the correction on
/// [`BindingConstraint::ChiploadMin`] and the H4 finding on
/// [`ChipBoundsSource::row_id`]. A typed value cannot drift from the
/// constant it names, because it carries the constant.
///
/// [`Self::gates_export`] lives here, on the SOURCE, and not on
/// [`CriterionKind`]. The reason a bound may not refuse an export is
/// the bound's provenance, not the quantity it measures. The day a
/// rule of thumb is measured, its variant is replaced by a measured
/// one and the row starts gating with no change to any renderer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum BoundSource {
    /// The power gate's ceiling: `MachineProfile::power_at_rpm(rpm)`
    /// times `MachineProfile::safety_factor`, at the sample the
    /// verdict's `available_kw` describes.
    MachinePowerCurve { rpm: f64, safety_factor: f64 },
    /// The matched vendor chip band.
    ///
    /// `floor_mm_per_tooth` is an `Option` because
    /// [`ChipBounds::min_mm_per_tooth`] is one: some LUT rows ship only
    /// an upper bound, and a fabricated `0.0` floor would be an absence
    /// rendered as a reading. The floor travels with the band because
    /// a burn reading sits BELOW its bound, not above it, and the badge
    /// needs the floor to word that case.
    VendorChipBand {
        floor_mm_per_tooth: Option<f64>,
        ceiling_mm_per_tooth: f64,
        source: ChipBoundsSource,
    },
    /// [`crate::tool_load::deflection::EXCEEDS_BOUND_MM`] — the tip
    /// displacement budget the deflection gate judges against. A fixed
    /// finish-quality figure in MILLIMETRES.
    DeflectionBudget,
    /// `RigidityProfile::doc_roughing_factor` (or `adaptive_doc_factor`)
    /// times the tool diameter. A rule of thumb with no published
    /// source, so it does not gate ([`Self::gates_export`]). S3 is its
    /// first producer; nothing in this crate emits it today.
    RigidityRuleOfThumb { factor: f64, diameter_mm: f64 },
    /// The drill gates' own envelopes. All three are material-derived:
    /// `chip_welding_threshold`, `per_peck_max_depth_to_diameter` and
    /// the plunge-feed envelope.
    DrillEnvelope,
}

impl BoundSource {
    /// **The setting an operator changes to move this row**, one of
    /// `"machine"`, `"tool"`, `"material"` or `"vendor row"`.
    ///
    /// The operator's rule 3, and the durable half of provenance
    /// (`REVIEW_DESIGN.md` §6.3): a setting is a live symbol a reader
    /// can find, where a cited document is not.
    #[must_use]
    pub fn setting(&self) -> &'static str {
        match self {
            // The power curve and the safety factor are both
            // `MachineProfile` fields.
            BoundSource::MachinePowerCurve { .. } => "machine",
            BoundSource::VendorChipBand { .. } => "vendor row",
            // The budget is a fixed constant, so no setting moves the
            // BOUND. The tool moves the reading under it: stickout,
            // diameter and the flute profile are the whole cantilever
            // the gate integrates.
            BoundSource::DeflectionBudget => "tool",
            // `RigidityProfile` is a `MachineProfile` field. The
            // diameter is the tool's, but the factor is the machine's,
            // and the factor is what makes this bound a rule of thumb.
            BoundSource::RigidityRuleOfThumb { .. } => "machine",
            BoundSource::DrillEnvelope => "material",
        }
    }

    /// **One clause, formatted from the values this variant carries.**
    /// Never a literal number: every figure below comes from the value
    /// it describes, at render time.
    ///
    /// The clause states the PROVENANCE. For the bound itself, beside
    /// its unit, use [`CriterionStatus::bound_clause`], which formats
    /// the bound and then appends this.
    #[must_use]
    pub fn clause(&self) -> String {
        match self {
            BoundSource::MachinePowerCurve { rpm, safety_factor } => format!(
                "the machine power curve at {rpm:.0} rpm, times the {safety_factor:.2} safety factor"
            ),
            BoundSource::VendorChipBand {
                floor_mm_per_tooth,
                ceiling_mm_per_tooth,
                source,
            } => match floor_mm_per_tooth {
                Some(floor) => format!(
                    "the vendor chip band {floor:.4} to {ceiling_mm_per_tooth:.4} mm/tooth ({})",
                    source.row_id()
                ),
                None => format!(
                    "the vendor chip band, ceiling {ceiling_mm_per_tooth:.4} mm/tooth, \
                     no published floor ({})",
                    source.row_id()
                ),
            },
            BoundSource::DeflectionBudget => format!(
                "the tip deflection budget of {:.3} mm",
                crate::tool_load::deflection::EXCEEDS_BOUND_MM
            ),
            BoundSource::RigidityRuleOfThumb {
                factor,
                diameter_mm,
            } => format!(
                "the machine rigidity factor {factor:.2} times the tool diameter \
                 {diameter_mm:.2} mm, which is {:.2} mm; a rule of thumb with no \
                 published source",
                factor * diameter_mm
            ),
            BoundSource::DrillEnvelope => {
                "the material's drilling envelope for this cycle".to_owned()
            }
        }
    }

    /// **Whether an exceedance of this bound may refuse a g-code
    /// export.** False only for [`BoundSource::RigidityRuleOfThumb`]
    /// today.
    ///
    /// The export gate reads this through
    /// [`CriterionStatus::refuses_export`]. A row with NO source still
    /// refuses: every shipped gate carries one, so an absent source is
    /// a gate that forgot, and that must fail safe.
    #[must_use]
    pub fn gates_export(&self) -> bool {
        !matches!(self, BoundSource::RigidityRuleOfThumb { .. })
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
    /// **X-VAC** — the population behind this verdict. `None` = not
    /// stated (never "zero"). `Some(p)` with `p.is_vacuous()` means the
    /// verdict rests on nothing; see [`CriterionStatus::is_vacuous`].
    ///
    /// This is the single field every verdict-rendering surface reads to
    /// tell a measured pass from a vacuous one. Report-tier: `state` is
    /// unaffected, so a vacuous `Within` still reads `Within`.
    pub population: Option<GatePopulation>,
    pub display_peak: Option<f64>,
    pub unit: &'static str,
    /// **The bound `display_peak` was judged against, in `unit`.**
    /// `None` when the criterion is unmodelled, or when no bound
    /// exists at all (the gantry-push row).
    ///
    /// S4 (2026-09-18). Before this field the GUI built two of the
    /// three caps it drew against, and one of them was an L over D
    /// ratio of 4.0 set beside a gate that judges millimetres against
    /// `deflection::EXCEEDS_BOUND_MM` — defect class 2, a quantity
    /// divided by a fraction of a DIFFERENT quantity. Every bound now
    /// comes from the gate that judged it.
    pub bound: Option<f64>,
    /// **Where `bound` came from, typed.** `None` exactly when `bound`
    /// is `None`. A renderer FORMATS this; it never retypes a number.
    pub bound_source: Option<BoundSource>,
    /// `Some` iff `state == Exceeds` — the typed exceedance label for
    /// this gate, set by each verdict's `as_criterion_status`. The
    /// gating tier (`exceeded_criteria`, `enforce_load_policy`, the
    /// summary breakdown) derives from this field, so including a gate
    /// in `criteria()` is the one decision that makes it gate g-code
    /// export (Phase 6 task 5).
    pub exceeded: Option<ExceededCriterion>,
}

impl CriterionStatus<'_> {
    /// **The X-VAC bar, and it is a POPULATION bar.** True only when the
    /// gate stated its population and that population was empty.
    ///
    /// A verdict bar tests this vacuously — `Within` is exactly what a
    /// vacuous gate returns — so any surface asserting "this gate
    /// exonerated the cut" must consult this first.
    pub fn is_vacuous(&self) -> bool {
        self.population.is_some_and(GatePopulation::is_vacuous)
    }

    /// **True when this row is absent by design and no operator action
    /// can fill it.** S1 (2026-09-18). This is the ONE predicate every
    /// counter reads before it turns an `Unmodeled` tally into an
    /// operator prompt: [`ToolpathLoadVerdict::any_unmodeled`] and,
    /// through it, the export gate, the Readiness triage and the
    /// pre-flight panel.
    ///
    /// **It is NOT `matches!(reason, NotImplemented(_))`.** Measured
    /// 2026-09-18: two shipped gates already refuse with
    /// `NotImplemented` for reasons the operator CAN fix — the power
    /// gate when no `MachineProfile` reached the evaluator
    /// (`power.rs`), and the deflection gate when the tool reports zero
    /// stickout (`deflection.rs`). Both name a missing input. Treating
    /// the bare variant as a known absence would stop the export gate
    /// refusing on either, which is a silent weakening of two gates.
    ///
    /// The distinction is the KIND, not the reason: see
    /// [`CriterionKind::is_unmodeled_by_design`]. The reason must still
    /// be `NotImplemented`, so a drill cycle's `NotApplicableForOp`
    /// gantry row keeps the arm the three milling gates take beside it.
    ///
    /// The row itself stays in [`ToolpathLoadVerdict::criteria`]: it is
    /// a VISIBLE absence, not a hidden one. This predicate suppresses
    /// the prompt, never the row.
    pub fn is_known_absence(&self) -> bool {
        self.kind.is_unmodeled_by_design()
            && matches!(
                self.unmodeled_reason,
                Some(UnmodeledReason::NotImplemented(_))
            )
    }

    /// The one operator-facing vacuity clause, shared by every renderer
    /// so the wording cannot drift between GUI, CLI, MCP and narration.
    /// Empty when not vacuous.
    pub fn vacuity_clause(&self) -> String {
        self.population
            .map(GatePopulation::vacuity_clause)
            .unwrap_or_default()
    }

    /// **The one operator-facing bound clause**, shared by every
    /// renderer so the wording cannot drift between GUI, CLI, MCP and
    /// narration. Empty when the row states no bound.
    ///
    /// The bound is formatted from [`Self::bound`] and the provenance
    /// from [`BoundSource::clause`], both at render time. No number in
    /// this string is typed.
    #[must_use]
    pub fn bound_clause(&self) -> String {
        let Some(bound) = self.bound else {
            return String::new();
        };
        let unit = self.unit;
        match &self.bound_source {
            Some(source) => format!("limit {bound:.4} {unit}, from {}", source.clause()),
            None => format!("limit {bound:.4} {unit}"),
        }
    }

    /// **True when this row exceeded a bound that may refuse a g-code
    /// export.** S4 (2026-09-18).
    ///
    /// The decision keys on the bound's PROVENANCE, never on the
    /// quantity: see [`BoundSource::gates_export`]. A row with no
    /// source still refuses, because every shipped gate carries one and
    /// an absence must fail safe.
    ///
    /// The export gate reads this through
    /// [`crate::gcode::refusing_exceedances`]. A row this predicate
    /// rejects is still REPORTED — it is not refused, and it is not
    /// hidden.
    #[must_use]
    pub fn refuses_export(&self) -> bool {
        self.state == LoadState::Exceeds
            && self
                .bound_source
                .as_ref()
                .is_none_or(BoundSource::gates_export)
    }
}

/// What a gate's verdict was actually computed from — **X-VAC**
/// (`planning/review_2026-08-04/TECH_DEBT_2_CLOSEOUT.md` §4;
/// `planning/review_2026-08-08/XVAC_CENSUS.md`).
///
/// Measured 2026-08-05: three gates returned `Within` with
/// `sample_range 0..0`, no locality and `available_kw 0.0` —
/// indistinguishable on every surface from a measured clean cut. The
/// verdict was *not* wrong; it was **vacuous**, and nothing said so.
/// Every predicate that can shrink a gate's population (the phantom /
/// configured-entry split, the steady-state feed filter, the fresh
/// material floor, the sample-validity predicate, a hole set with no
/// depth) can drive `contributing` to zero while the gate still returns
/// a healthy-looking `Within`.
///
/// This type is **report-tier**: no gate outcome, threshold, severity or
/// export decision reads it. A vacuous `Within` stays `Within` — it just
/// says it is vacuous. Do not derive a refusal from it without a
/// checkpoint ruling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatePopulation {
    /// Units that actually reached the gate's own comparison. Zero means
    /// **vacuous**: the verdict rests on nothing.
    pub contributing: usize,
    /// Units the gate was handed before its own filters ran. Always
    /// `>= contributing`; the difference is what the predicates removed.
    pub offered: usize,
    /// What is being counted, so a consumer can word the marker without
    /// knowing which gate it is looking at.
    pub unit: PopulationUnit,
}

/// What a [`GatePopulation`] counts. Milling gates count simulation
/// samples; the drill trio counts holes (its gates are hole-derived, and
/// the drill path has no per-sample dexel stream at all).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PopulationUnit {
    Samples,
    Holes,
}

impl PopulationUnit {
    pub fn plural(self) -> &'static str {
        match self {
            PopulationUnit::Samples => "samples",
            PopulationUnit::Holes => "holes",
        }
    }
}

impl GatePopulation {
    pub fn new(contributing: usize, offered: usize, unit: PopulationUnit) -> Self {
        Self {
            contributing,
            offered: offered.max(contributing),
            unit,
        }
    }

    /// **The X-VAC predicate.** True when the verdict was computed from
    /// nothing at all.
    pub fn is_vacuous(self) -> bool {
        self.contributing == 0
    }

    /// How many offered units the gate's own predicates removed.
    ///
    /// Test door: `crates/rs_cam_core/tests/gate_population_vacuity_xvac.rs`.
    pub fn filtered_out(self) -> usize {
        self.offered.saturating_sub(self.contributing)
    }

    /// One operator-facing clause, identical on every surface. Empty
    /// string when the population is not vacuous, so a renderer can
    /// append it unconditionally.
    pub fn vacuity_clause(self) -> String {
        if !self.is_vacuous() {
            return String::new();
        }
        format!(
            " — VACUOUS: this verdict rests on 0 of {} {} (all filtered out); \
             it is not a measurement of a clean cut",
            self.offered,
            self.unit.plural(),
        )
    }
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
    /// **X-VAC** — the population this evidence was drawn from.
    ///
    /// `None` means **not stated** (a pre-2026-08-14 wire payload, or a
    /// construction site that has no population to report), never
    /// "measured zero" — the same contract `ToolpathStats`' report-only
    /// findings carry. `Some(p)` with `p.contributing == 0` is the
    /// vacuous case: an `0..0` `sample_range` that means "nothing
    /// contributed", as opposed to the same `0..0` meaning "no single
    /// sample is worth naming".
    ///
    /// Read it through [`SampleEvidence::is_vacuous`]; never infer
    /// vacuity from `sample_range` alone, which is exactly the
    /// ambiguity X-VAC is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub population: Option<GatePopulation>,
}

impl SampleEvidence {
    /// No specific sample recorded: range `0..0`, no statistic.
    pub fn empty() -> Self {
        Self {
            sample_range: 0..0,
            statistic: None,
            locality: None,
            population: None,
        }
    }

    /// Single-sample range at `idx`, no statistic descriptor.
    pub fn at(idx: usize) -> Self {
        Self {
            sample_range: idx..(idx + 1),
            statistic: None,
            locality: None,
            population: None,
        }
    }

    /// Single-sample range at `idx` annotated with a chipload statistic.
    pub fn at_with_stat(idx: usize, statistic: ChiploadStatistic) -> Self {
        Self {
            sample_range: idx..(idx + 1),
            statistic: Some(statistic),
            locality: None,
            population: None,
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

    /// Builder — state the population this evidence was drawn from
    /// (X-VAC). Every per-sample gate calls this on both arms, so a
    /// consumer can always ask "how many samples decided this?".
    #[must_use]
    pub fn with_population(mut self, population: GatePopulation) -> Self {
        self.population = Some(population);
        self
    }

    /// True only when the population is **stated and empty**. An
    /// unstated population is not vacuous — it is unknown.
    pub fn is_vacuous(&self) -> bool {
        self.population.is_some_and(GatePopulation::is_vacuous)
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

    /// Stable identifier for the provenance of these bounds, for use as
    /// the `row_id` of a `LutCitation` and anywhere else a reader has to
    /// tell a calibrated row from a derived one.
    ///
    /// H4 (wave 15): the chipload diagnostic hard-coded `"vendor_lut"`
    /// here regardless of source, so the live validation of 2026-07-30
    /// saw a citation reading `row_id: vendor_lut` and
    /// `extrapolated: true` in the same breath. A citation that names a
    /// row the bounds did not come from is worse than no citation.
    #[must_use]
    pub const fn row_id(self) -> &'static str {
        match self {
            ChipBoundsSource::VendorLut => "vendor_lut",
            ChipBoundsSource::VendorLutExtrapolated => "vendor_lut_extrapolated",
            ChipBoundsSource::VendorLutPointPreset => "vendor_lut_point_preset",
            ChipBoundsSource::VendorLutMissingAe => "vendor_lut_missing_ae",
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
///
/// # The comparison lives on the type — Checkpoint K (b1), 2026-08-13
///
/// Use [`Self::exceeds_high`] / [`Self::below_low`] / [`Self::contains`]
/// rather than reading the fields and writing `>`. That is the whole
/// point of the ruling: a bare comparison against `max_mm_per_tooth` is
/// how G-CHIP-ULP shipped — a verdict decided by the last bit of a
/// multiply/divide round trip. See [`crate::tool_load::boundary`] for
/// the contract and the measurement behind the epsilon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChipBounds {
    pub min_mm_per_tooth: Option<f64>,
    pub max_mm_per_tooth: f64,
    pub source: ChipBoundsSource,
}

impl ChipBounds {
    /// Breakage side. `true` when `observed` is above the band maximum
    /// widened by `breakage_tolerance`
    /// ([`super::ToleranceBands::breakage`]) and the boundary epsilon.
    ///
    /// Pass `0.0` where no tolerance applies (entry-spike advisories,
    /// display comparisons) — the epsilon still does.
    #[must_use]
    pub fn exceeds_high(&self, observed: f64, breakage_tolerance: f64) -> bool {
        super::boundary::exceeds_high(observed, self.max_mm_per_tooth, breakage_tolerance)
    }

    /// Burn side. `None` when the row publishes no minimum — which is
    /// **not** the same as `Some(false)`, and the `Option` is here so no
    /// caller can collapse "unmodelled" into "fine".
    #[must_use]
    pub fn below_low(&self, observed: f64, burn_tolerance: f64) -> Option<bool> {
        self.min_mm_per_tooth
            .map(|min| super::boundary::below_low(observed, min, burn_tolerance))
    }

    /// `true` when `observed` trips neither side at zero tolerance. A
    /// row with no minimum can only fail on the high side.
    #[must_use]
    pub fn contains(&self, observed: f64) -> bool {
        !self.exceeds_high(observed, 0.0) && !self.below_low(observed, 0.0).unwrap_or(false)
    }

    /// `true` when `observed` sits **on** the band ceiling — within the
    /// boundary epsilon, i.e. indistinguishable from it at float
    /// precision.
    ///
    /// This is the proximity half of the Checkpoint K (c2) ceiling
    /// advisory. It is deliberately not a "close to" test: the recipe
    /// this fires for was placed on the ceiling *by the engine's own
    /// rubbing-floor clamp*, so it lands there exactly, modulo the
    /// reconstruction noise that made the verdict unstable in the first
    /// place.
    #[must_use]
    pub fn is_at_max(&self, observed: f64) -> bool {
        super::boundary::is_at_bound(observed, self.max_mm_per_tooth)
    }
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

/// Typed chipload verdict. The chipload gate returns this. It carries
/// both bounds-approach metrics on the within case and the triggering
/// metric on exceed.
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
        /// **Checkpoint K (c2), 2026-08-13 — the recipe is sitting ON
        /// the band ceiling because the engine's own clamp put it
        /// there.**
        ///
        /// Suggest's Step-9b raises an advance below the chip-formation
        /// floor; when the whole derated band sits under that floor,
        /// [`crate::feeds::effective_rubbing_floor`] returns the band
        /// **maximum**, and the recipe is parked exactly on the
        /// breakage-side bound with zero headroom — by design, and
        /// correctly (the alternative measured 3.47× the band maximum).
        ///
        /// A hard `Exceeds(High)` there is the engine failing its own
        /// recipe, so this reports *clamped* instead. It is emitted only
        /// when **both** hold: the observation is at the ceiling within
        /// [`crate::tool_load::boundary::BOUNDARY_EPSILON_REL`], **and**
        /// the commanded advance was placed by that clamp
        /// ([`crate::feeds::recipe_parked_by_rubbing_floor`]). Proximity
        /// alone would demote genuine exceedances on any op whose recipe
        /// happens to sit near the ceiling, which is why (c2) is only
        /// correct with (b1).
        ///
        /// Mirrors [`Self::Within::burn_advisory`]'s shape deliberately:
        /// same pattern, same reason — a bound whose *hard* trip is
        /// indefensible, kept visible instead of hidden.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ceiling_advisory: Option<Box<ChiploadMetric>>,
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
        // S4: `bounds` is the band the gate compared against, carried
        // on the metric that decided the verdict. The row reads against
        // its CEILING — the breakage side a percentage is drawn of —
        // and the floor travels in the source, because a burn reading
        // sits below its bound rather than above it.
        let (state, peak, range, population, exceeded, bounds) = match self {
            ChiploadVerdict::Within {
                approach_to_max, ..
            } => (
                LoadState::Within,
                Some(approach_to_max.observed_mm_per_tooth),
                option_range(&approach_to_max.evidence.sample_range),
                approach_to_max.evidence.population,
                None,
                Some(&approach_to_max.bounds),
            ),
            ChiploadVerdict::Exceeds {
                triggering, side, ..
            } => (
                LoadState::Exceeds,
                Some(triggering.observed_mm_per_tooth),
                option_range(&triggering.evidence.sample_range),
                triggering.evidence.population,
                Some(match side {
                    ChipSide::Low => ExceededCriterion::chipload_burn(),
                    ChipSide::High => ExceededCriterion::chipload_breakage(),
                }),
                Some(&triggering.bounds),
            ),
            ChiploadVerdict::Unmodeled { .. } => {
                (LoadState::Unmodeled, None, None, None, None, None)
            }
        };
        CriterionStatus {
            kind: CriterionKind::Chipload,
            state,
            confidence: self.confidence(),
            unmodeled_reason: self.unmodeled_reason(),
            sample_range: range,
            population,
            display_peak: peak,
            unit: CriterionKind::Chipload.unit(),
            bound: bounds.map(|b| b.max_mm_per_tooth),
            bound_source: bounds.map(|b| BoundSource::VendorChipBand {
                floor_mm_per_tooth: b.min_mm_per_tooth,
                ceiling_mm_per_tooth: b.max_mm_per_tooth,
                source: b.source,
            }),
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
        /// **S4 — where `available_kw` came from.** The two inputs the
        /// gate multiplied: the sample's spindle speed and the
        /// profile's safety factor.
        ///
        /// This is the one gate whose provenance is not already on the
        /// verdict. The band, the deflection budget and the drill
        /// envelopes each ride on a field the verdict already carries;
        /// `available_kw` is a product, and the factors are gone by the
        /// time a consumer reads it.
        ///
        /// `Option` and `#[serde(default)]` so an older serialized
        /// verdict still deserializes. `None` reads as "not stated" and
        /// fails safe at the export gate
        /// ([`CriterionStatus::refuses_export`]).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bound_source: Option<BoundSource>,
    },
    Exceeds {
        peak_kw: f64,
        available_kw: f64,
        evidence: SampleEvidence,
        confidence: Confidence,
        /// **S4 — where `available_kw` came from.** See
        /// [`PowerVerdict::Within::bound_source`].
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bound_source: Option<BoundSource>,
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
        // S4: the bound is `available_kw`, the ceiling this gate judged
        // against, and `bound_source` carries the two inputs that built
        // it. The GUI used to build its own from `max_power_kw ×
        // safety_factor`, which is the ceiling at the RATED speed, not
        // at the speed the toolpath runs.
        let (state, peak, range, population, bound, source) = match self {
            PowerVerdict::Within {
                peak_kw,
                available_kw,
                evidence,
                bound_source,
                ..
            } => (
                LoadState::Within,
                Some(*peak_kw),
                option_range(&evidence.sample_range),
                evidence.population,
                Some(*available_kw),
                bound_source.clone(),
            ),
            PowerVerdict::Exceeds {
                peak_kw,
                available_kw,
                evidence,
                bound_source,
                ..
            } => (
                LoadState::Exceeds,
                Some(*peak_kw),
                option_range(&evidence.sample_range),
                evidence.population,
                Some(*available_kw),
                bound_source.clone(),
            ),
            PowerVerdict::Unmodeled { .. } => (LoadState::Unmodeled, None, None, None, None, None),
        };
        CriterionStatus {
            kind: CriterionKind::Power,
            state,
            confidence: self.confidence(),
            unmodeled_reason: self.unmodeled_reason(),
            sample_range: range,
            population,
            display_peak: peak,
            unit: CriterionKind::Power.unit(),
            bound,
            bound_source: source,
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
        // S4: the bound is `bounds.exceeds_mm`, which the gate sets
        // from `deflection::EXCEEDS_BOUND_MM`. It is MILLIMETRES. The
        // GUI drew this row against `DEFLECTION_SAFE_LD_RATIO = 4.0`,
        // an L over D ratio, which is a different quantity with a
        // similar magnitude — defect class 2 (`RESUME_PLAN` §9).
        let (state, peak, range, population, bound) = match self {
            DeflectionVerdict::Within {
                peak_mm,
                bounds,
                evidence,
                ..
            } => (
                LoadState::Within,
                Some(*peak_mm),
                option_range(&evidence.sample_range),
                evidence.population,
                Some(bounds.exceeds_mm),
            ),
            DeflectionVerdict::Exceeds {
                peak_mm,
                bounds,
                evidence,
                ..
            } => (
                LoadState::Exceeds,
                Some(*peak_mm),
                option_range(&evidence.sample_range),
                evidence.population,
                Some(bounds.exceeds_mm),
            ),
            DeflectionVerdict::Unmodeled { .. } => (LoadState::Unmodeled, None, None, None, None),
        };
        CriterionStatus {
            kind: CriterionKind::Deflection,
            state,
            confidence: self.confidence(),
            unmodeled_reason: self.unmodeled_reason(),
            sample_range: range,
            population,
            display_peak: peak,
            unit: CriterionKind::Deflection.unit(),
            bound,
            bound_source: bound.map(|_| BoundSource::DeflectionBudget),
            exceeded: (state == LoadState::Exceeds).then(ExceededCriterion::deflection),
        }
    }
}

/// **Typed depth-of-cut verdict.** S3 (2026-09-18).
///
/// The measured half of the depth question (`PLAN.md` §11, Reading A).
/// Both decided arms carry the cap they were judged against, so a
/// consumer reads the bound off the verdict and never rebuilds it.
///
/// `bound` is a [`RigidityDepthCap`], which holds the profile's factor
/// and the tool diameter rather than their product. That is the same
/// pair [`BoundSource::RigidityRuleOfThumb`] carries, so the row's
/// bound and the row's provenance are one value seen twice and cannot
/// disagree.
///
/// **This verdict never refuses an export.** The cap is a rule of
/// thumb with no published source; see [`BoundSource::gates_export`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum DepthVerdict {
    Within {
        /// Peak `axial_engagement_mm` over the toolpath's contributing
        /// cutting samples. `0.0` with a vacuous population means the
        /// gate measured nothing — read [`GatePopulation::is_vacuous`],
        /// never this field alone.
        peak_mm: f64,
        bound: crate::machine::RigidityDepthCap,
        evidence: SampleEvidence,
        confidence: Confidence,
    },
    Exceeds {
        peak_mm: f64,
        bound: crate::machine::RigidityDepthCap,
        evidence: SampleEvidence,
        confidence: Confidence,
    },
    Unmodeled {
        reason: UnmodeledReason,
    },
}

impl DepthVerdict {
    pub fn state(&self) -> LoadState {
        match self {
            DepthVerdict::Within { .. } => LoadState::Within,
            DepthVerdict::Exceeds { .. } => LoadState::Exceeds,
            DepthVerdict::Unmodeled { .. } => LoadState::Unmodeled,
        }
    }

    pub fn is_exceeded(&self) -> bool {
        matches!(self, DepthVerdict::Exceeds { .. })
    }

    pub fn is_unmodeled(&self) -> bool {
        matches!(self, DepthVerdict::Unmodeled { .. })
    }

    pub fn confidence(&self) -> Option<&Confidence> {
        match self {
            DepthVerdict::Within { confidence, .. } | DepthVerdict::Exceeds { confidence, .. } => {
                Some(confidence)
            }
            DepthVerdict::Unmodeled { .. } => None,
        }
    }

    pub fn unmodeled_reason(&self) -> Option<&UnmodeledReason> {
        match self {
            DepthVerdict::Unmodeled { reason } => Some(reason),
            _ => None,
        }
    }

    /// **A hand-built depth row for a fixture that ran no simulation.**
    /// S3 (2026-09-18).
    ///
    /// The verdict fixtures in this crate's test modules state their
    /// chipload, power and deflection claims by hand, most of them as a
    /// `Within` over `SampleEvidence::empty()`. This keeps the depth row
    /// beside them in the same posture, so adding the row to
    /// [`ToolpathLoadVerdict`] changes no fixture's meaning. The two
    /// numbers are fixture choices — a 0.25 factor on a Ø6 tool — and
    /// no model reads them.
    #[cfg(test)]
    pub(crate) fn fixture_within() -> Self {
        DepthVerdict::Within {
            peak_mm: 0.0,
            bound: crate::machine::RigidityDepthCap {
                factor: 0.25,
                diameter_mm: 6.0,
            },
            evidence: SampleEvidence::empty(),
            confidence: Confidence::Validated,
        }
    }

    /// The generic row. The bound is `factor × diameter` from the
    /// profile, and the source carries those two numbers, so a renderer
    /// formats the cap and its provenance without multiplying anything
    /// itself.
    pub fn as_criterion_status(&self) -> CriterionStatus<'_> {
        let (state, peak, range, population, bound) = match self {
            DepthVerdict::Within {
                peak_mm,
                bound,
                evidence,
                ..
            } => (
                LoadState::Within,
                Some(*peak_mm),
                option_range(&evidence.sample_range),
                evidence.population,
                Some(*bound),
            ),
            DepthVerdict::Exceeds {
                peak_mm,
                bound,
                evidence,
                ..
            } => (
                LoadState::Exceeds,
                Some(*peak_mm),
                option_range(&evidence.sample_range),
                evidence.population,
                Some(*bound),
            ),
            DepthVerdict::Unmodeled { .. } => (LoadState::Unmodeled, None, None, None, None),
        };
        CriterionStatus {
            kind: CriterionKind::DepthOfCut,
            state,
            confidence: self.confidence(),
            unmodeled_reason: self.unmodeled_reason(),
            sample_range: range,
            population,
            display_peak: peak,
            unit: CriterionKind::DepthOfCut.unit(),
            bound: bound.map(|b| b.cap_mm()),
            bound_source: bound.map(|b| BoundSource::RigidityRuleOfThumb {
                factor: b.factor,
                diameter_mm: b.diameter_mm,
            }),
            exceeded: (state == LoadState::Exceeds).then(ExceededCriterion::depth_of_cut),
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
    /// What the operator should actually do about it.
    ///
    /// R-4 (2026-08-04): this used to live in the GUI, keyed on
    /// `CriterionKind` alone, which is not enough information to word a
    /// drill remedy — the advice depends on the CYCLE. The
    /// peck-adequacy remedy said "reduce peck depth" on `Simple` and
    /// `Dwell` cycles, which have no peck depth (F1 fixed exactly this
    /// defect on the sibling gate in 2026-06 and left this one), and
    /// the chip-welding remedy said "switch to a peck cycle"
    /// unconditionally, including on ops that were already pecking.
    /// Both are reachable on a single hole, so an operator could be
    /// given two contradictory instructions about one number.
    ///
    /// Lives here so the decision is made once, beside the cycle that
    /// determines it, rather than in each consumer.
    pub remedy: &'static str,
}

impl ExceededCriterion {
    pub fn chipload_burn() -> Self {
        Self {
            kind: CriterionKind::Chipload,
            label: "chipload",
            reason_label: "burn risk",
            remedy: "chipload below vendor min — rubbing/burning risk. At low \
                     chipload the tool edge rubs instead of cutting; friction \
                     generates heat that glazes and burns the wood. Increase \
                     feed rate or reduce RPM.",
        }
    }

    pub fn chipload_breakage() -> Self {
        Self {
            kind: CriterionKind::Chipload,
            label: "chipload",
            reason_label: "breakage",
            remedy: "chipload above vendor max — breakage risk. Reduce feed \
                     rate or increase RPM.",
        }
    }

    pub fn power() -> Self {
        Self {
            kind: CriterionKind::Power,
            label: "power",
            reason_label: "spindle power",
            remedy: "predicted spindle power exceeds machine limit",
        }
    }

    pub fn deflection() -> Self {
        Self {
            kind: CriterionKind::Deflection,
            label: "deflection",
            reason_label: "stiffness",
            remedy: "tip deflection exceeds 200 µm — finish/breakage risk",
        }
    }

    /// S3 (2026-09-18). The remedy says the bound is advisory, because
    /// an operator who reads "exceeded" beside an export that went
    /// ahead is owed the reason in the same place.
    pub fn depth_of_cut() -> Self {
        Self {
            kind: CriterionKind::DepthOfCut,
            label: "depth of cut",
            reason_label: "machine rigidity",
            remedy: "the measured depth of cut is deeper than the machine \
                     rigidity factor allows for this operation family. That \
                     factor is a rule of thumb with no published source, so \
                     it reports and does not refuse the export. Reduce the \
                     depth per pass, or take the cut deliberately.",
        }
    }

    /// R-4: on a cycle that is already pecking, "switch to a peck
    /// cycle" is not advice — it is a description of the op. What is
    /// left to change is the peck depth or the hole itself.
    pub fn drill_chip_welding(cycle: crate::tool_load::drill_gates::DrillCycleKind) -> Self {
        Self {
            kind: CriterionKind::DrillChipWelding,
            label: "chip welding",
            reason_label: "deep hole",
            remedy: if cycle.is_pecking() {
                "hole depth-to-diameter exceeds the material chip-welding \
                 threshold even with the evacuation credit this cycle earns — \
                 reduce peck depth, or use a shorter hole or a larger drill."
            } else {
                "hole depth-to-diameter exceeds the material chip-welding \
                 threshold — switch to a peck cycle or reduce depth"
            },
        }
    }

    /// R-4 — F1's defect, on the sibling gate. This trips `Simple` and
    /// `Dwell` cycles on the WHOLE-HOLE ratio (a Ø4 × 30 mm softwood
    /// `Simple` hole reads 7.5 > 6.0), and the shipped remedy then told
    /// the operator to "reduce peck depth" on a cycle that has none.
    pub fn drill_peck_adequacy(cycle: crate::tool_load::drill_gates::DrillCycleKind) -> Self {
        Self {
            kind: CriterionKind::DrillPeckAdequacy,
            label: "peck depth",
            reason_label: if cycle.is_pecking() {
                "peck too deep"
            } else {
                "no peck cycle"
            },
            remedy: if cycle.is_pecking() {
                "single peck too deep for the material — reduce peck depth"
            } else {
                "this cycle cuts the hole in one descent, and the hole is too \
                 deep for the material to clear chips that way — switch to a \
                 peck cycle (there is no peck depth to reduce)"
            },
        }
    }

    pub fn drill_plunge_feed() -> Self {
        Self {
            kind: CriterionKind::DrillPlungeFeed,
            label: "plunge feed",
            reason_label: "breakage",
            remedy: "plunge feed above the material envelope — breakage risk. \
                     Reduce feed rate.",
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
                ceiling_advisory: None,
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
            // S3: a hand-built fixture states no measured depth; the
            // row keeps the posture of the gates beside it.
            depth: DepthVerdict::fixture_within(),
            drill_gates: None,
            modulation_summary: None,
            feed_explanation: None,
            kinematic_utilization: None,
        };
        // S3: three, not two. The fixture's chipload and deflection rows
        // were modelled before, and the depth row joined them. The count
        // moved because a REAL row joined the tier; the gantry row beside
        // it stays `Unmodeled` and still does not count.
        assert_eq!(v.modeled_count(), 3);
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
                    ceiling_advisory: None,
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
                // S3: a hand-built fixture states no measured depth; the
                // row keeps the posture of the gates beside it.
                depth: DepthVerdict::fixture_within(),
                drill_gates: None,
                modulation_summary: None,
                feed_explanation: None,
                kinematic_utilization: None,
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
                        ceiling_advisory: None,
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
                    // S3: a hand-built fixture states no measured depth; the
                    // row keeps the posture of the gates beside it.
                    depth: DepthVerdict::fixture_within(),
                    drill_gates: None,
                    modulation_summary: None,
                    feed_explanation: None,
                    kinematic_utilization: None,
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
                        ceiling_advisory: None,
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
                    // S3: a hand-built fixture states no measured depth; the
                    // row keeps the posture of the gates beside it.
                    depth: DepthVerdict::fixture_within(),
                    drill_gates: None,
                    modulation_summary: None,
                    feed_explanation: None,
                    kinematic_utilization: None,
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
            // S3: the depth row mirrors the gates beside it.
            depth: DepthVerdict::Unmodeled { reason: na() },
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
                // R-7: no hole attribution in a hand-built verdict.
                worst_hole_id: None,
                // X-VAC: not stated by this fixture — it exercises the
                // gate wording, not the population marker. `None` reads as
                // "not stated", never as an empty gate.
                population: None,
                cycle: crate::tool_load::drill_gates::DrillCycleKind::Peck,
            }),
            modulation_summary: None,
            feed_explanation: None,
            kinematic_utilization: None,
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
                    // S3: the depth row mirrors the gates beside it.
                    depth: DepthVerdict::Unmodeled {
                        reason: UnmodeledReason::NotApplicableForOp("drill cycle".to_owned()),
                    },
                    drill_gates: None,
                    modulation_summary: None,
                    feed_explanation: None,
                    kinematic_utilization: None,
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
                    // S3: the depth row mirrors the gates beside it.
                    depth: DepthVerdict::Unmodeled {
                        reason: UnmodeledReason::SimulationRequired,
                    },
                    drill_gates: None,
                    modulation_summary: None,
                    feed_explanation: None,
                    kinematic_utilization: None,
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
                    // S3: the depth row mirrors the gates beside it.
                    depth: DepthVerdict::Unmodeled {
                        reason: UnmodeledReason::NotApplicableForOp("drill cycle".to_owned()),
                    },
                    drill_gates: None,
                    modulation_summary: None,
                    feed_explanation: None,
                    kinematic_utilization: None,
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
                // S3: a hand-built fixture states no measured depth; the
                // row keeps the posture of the gates beside it.
                depth: DepthVerdict::fixture_within(),
                drill_gates: None,
                modulation_summary: None,
                feed_explanation: None,
                kinematic_utilization: None,
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
            ceiling_advisory: None,
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
            ceiling_advisory: None,
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
            bound_source: None,
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
            bound_source: None,
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
            bound_source: None,
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
                    bound_source: None,
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
                // S3: a hand-built fixture states no measured depth; the
                // row keeps the posture of the gates beside it.
                depth: DepthVerdict::fixture_within(),
                drill_gates: None,
                modulation_summary: None,
                feed_explanation: None,
                kinematic_utilization: None,
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
                // S3: a hand-built fixture states no measured depth; the
                // row keeps the posture of the gates beside it.
                depth: DepthVerdict::fixture_within(),
                drill_gates: None,
                modulation_summary: None,
                feed_explanation: None,
                kinematic_utilization: None,
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
                bound_source: None,
            },
            deflection: DeflectionVerdict::Unmodeled {
                reason: UnmodeledReason::SimulationRequired,
            },
            // S3: the depth row mirrors the gates beside it.
            depth: DepthVerdict::Unmodeled {
                reason: UnmodeledReason::SimulationRequired,
            },
            drill_gates: None,
            modulation_summary: None,
            feed_explanation: None,
            kinematic_utilization: None,
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
            deflection: DeflectionVerdict::Unmodeled {
                reason: reason.clone(),
            },
            // S3: the depth row mirrors the gates beside it.
            depth: DepthVerdict::Unmodeled { reason },
            drill_gates: None,
            modulation_summary: None,
            feed_explanation: None,
            kinematic_utilization: None,
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
            // S3: the depth row mirrors the gates beside it.
            depth: DepthVerdict::Unmodeled {
                reason: UnmodeledReason::SimulationRequired,
            },
            drill_gates: None,
            modulation_summary: None,
            feed_explanation: None,
            kinematic_utilization: None,
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
                    ceiling_advisory: None,
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
                // S3: a hand-built fixture states no measured depth; the
                // row keeps the posture of the gates beside it.
                depth: DepthVerdict::fixture_within(),
                drill_gates: None,
                modulation_summary: None,
                feed_explanation: None,
                kinematic_utilization: None,
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
                // S3: a hand-built fixture states no measured depth; the
                // row keeps the posture of the gates beside it.
                depth: DepthVerdict::fixture_within(),
                drill_gates: None,
                modulation_summary: None,
                feed_explanation: None,
                kinematic_utilization: None,
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
