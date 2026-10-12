//! Outcome types and the outcome builder (`build_outcome`).
//!
//! - [`OptimizeOutcome`] — what `optimize_toolpath` returns: one struct
//!   with an [`OutcomeKind`] tag and the fields `candidates`,
//!   `narrative`, `recommended_index` and `reason` (Roadmap F.7). MCP
//!   agents and the GUI modal read the same shape for every kind.
//! - [`ProjectOptimizeReport`] — the project rollup over every enabled
//!   toolpath.
//! - [`build_outcome`] — sorts the safe candidates by cycle time and
//!   selects the [`OutcomeKind`]. A candidate is safe when no criterion
//!   row of its verdict refuses an export ([`candidate_is_safe`]).

use serde::{Deserialize, Serialize};

use crate::tool_load::RefuseReason;

use super::candidate::OptimizeCandidate;
use super::narrative::{
    OutcomeNarrative, build_no_safe_narrative, build_no_search_narrative, build_ranked_narrative,
};

/// The kind of an [`OptimizeOutcome`]. The data shape is the same for
/// every kind; only this tag changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeKind {
    /// At least one candidate is safe AND faster than the baseline.
    /// `recommended_index` points at the fastest safe candidate.
    Ranked,
    /// No safe candidate is faster than the baseline, or the search
    /// evaluated no candidate. `reason` holds the classification;
    /// `narrative.limiting_gates` holds the refusing rows of the
    /// fastest candidate.
    NoSafeImprovement,
    /// The optimizer cannot model this toolpath: a drill cycle, a
    /// toolpath with no samples in the trace, or a custom material.
    /// `reason` is set; `candidates` is empty.
    Skipped,
}

/// Outcome of `optimize_toolpath` for one toolpath. Roadmap F.7 made
/// this one struct with an [`OutcomeKind`] tag, so every consumer reads
/// `candidates`, `narrative`, `recommended_index` and `reason` the same
/// way for every kind. The constructors fill the fields as follows:
///
/// | kind | candidates | narrative | recommended_index | reason |
/// |---|---|---|---|---|
/// | Ranked | baseline + safe by cycle time + unsafe | headline + envelope | Some(N), the fastest safe candidate | None |
/// | NoSafeImprovement | baseline + attempted | headline + explanation + envelope + limiting_gates | None | Some(_) |
/// | Skipped | empty | empty | None | Some(_) |
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizeOutcome {
    pub kind: OutcomeKind,
    /// Index 0 is always the baseline (for non-Skipped outcomes). The
    /// safe candidates follow by ascending cycle time, then the unsafe
    /// ones. Empty for `Skipped`.
    pub candidates: Vec<OptimizeCandidate>,
    /// Boxed to keep the outcome's size symmetric and small for use
    /// inside `Vec<OptimizeOutcome>` collections (the narrative carries
    /// strings + Vecs that would otherwise dominate the struct size).
    pub narrative: Box<OutcomeNarrative>,
    /// The recommended index into `candidates`. `Some(N)` only when
    /// `kind` is `Ranked` (the fastest safe candidate). `None` for
    /// `NoSafeImprovement` and `Skipped`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_index: Option<usize>,
    /// Refusal classification. `Some(_)` only when `kind` is
    /// `NoSafeImprovement` or `Skipped`; `None` for `Ranked`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<RefuseReason>,
    /// F4.3 — snapshot of the machine profile the run consumed (name +
    /// caps), populated by [`super::optimize_toolpath`] on every
    /// outcome so narratives are reconcilable with what the search
    /// actually used (the session machine can change between the run
    /// and the readout).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_snapshot: Option<MachineSnapshot>,
    /// **A-8 (F-OPT), 2026-08-13 — the simulation assumptions these
    /// numbers were taken at.**
    ///
    /// Populated by [`super::optimize_toolpath`] on **every** outcome,
    /// refusals and skips included, for the same reason
    /// [`Self::machine_snapshot`] is: a refusal is also a claim about
    /// the world, and it is also taken at an operating point.
    ///
    /// # Measurement contract — the `truncated_core_mm2` family
    ///
    /// `None` means **not stamped**, and never "no assumptions" or
    /// "the defaults". It is reachable exactly two ways: an outcome
    /// built by a constructor and not yet passed through
    /// [`super::optimize_toolpath`] (every `OptimizeOutcome::*`
    /// constructor leaves it `None`), or an outcome deserialized from a
    /// record written before this field existed. A consumer must render
    /// the absence, not substitute a guess — the whole defect this field
    /// addresses is a candidate card that *looked* like it named its
    /// operating point.
    ///
    /// This is the same three-valued discipline
    /// [`crate::compute::toolpath_stats::ToolpathStats::truncated_core_mm2`]
    /// carries (`None` = not measured, a present value = measured), and
    /// deliberately **not**
    /// [`crate::compute::toolpath_stats::ToolpathStats::zero_removal`]'s weaker
    /// contract where `None` conflates "not measured" with "nothing to
    /// report". Here the two are distinguishable and the inner
    /// `Option`s keep them distinguished — see
    /// [`BaselineTraceAssumptions`], where the honest answer to two of
    /// three questions is "the trace does not record it".
    ///
    /// Report-only: no gate consumes it, no candidate is ranked on it,
    /// and no verdict changes on it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assumptions: Option<SimAssumptionStamp>,
}

/// F4.3 — the machine caps an optimize run consumed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MachineSnapshot {
    pub name: String,
    /// Travel rate ($110-class) — rapid/linking cap.
    pub max_feed_mm_min: f64,
    /// Cutting-feed ceiling the search space was bounded by
    /// ([`crate::machine::MachineProfile::cutting_feed_ceiling_mm_min`]).
    pub cutting_feed_ceiling_mm_min: f64,
    pub rpm_min: f64,
    pub rpm_max: f64,
}

impl MachineSnapshot {
    pub fn of(machine: &crate::machine::MachineProfile) -> Self {
        let (rpm_min, rpm_max) = machine.rpm_range();
        Self {
            name: machine.name.clone(),
            max_feed_mm_min: machine.max_feed_mm_min,
            cutting_feed_ceiling_mm_min: machine.cutting_feed_ceiling_mm_min(),
            rpm_min,
            rpm_max,
        }
    }
}

/// **A-8 (F-OPT) — the simulation assumptions an optimize run's numbers
/// were taken at.**
///
/// # The defect this exists for
///
/// The operator's 2026-08-07 feeds/speeds review, `[medium] The optimizer
/// initially evaluates a different feed world than production simulation`:
/// candidate scoring pins `use_predicted_feed_in_gates` and
/// `adaptive_feed_modulation` to `false` while normal simulation can
/// enable either, so *"safe/faster candidate" means safe/faster in the
/// unmodulated commanded-feed candidate model until reconciliation, not
/// necessarily in the live emitted-feed model.* The review's repair was
/// explicitly **not** to unify the two models — it was to stop the
/// isolation being invisible. This type is that disclosure.
///
/// Since the review was written the gap got **wider**, not narrower:
/// Checkpoint J (2026-08-13) flipped `SimulationOptions::default()
/// .adaptive_feed_modulation` to `true` and Checkpoint K (g1) flipped the
/// CLI flag to match, while
/// [`super::candidate::candidate_sim_options`] still pins `false`.
///
/// # What it is not
///
/// It is **not** a claim that every number in the outcome shares one
/// operating point. It records the opposite where that is true: the
/// candidates and the baseline are two different sets, and
/// [`Self::baseline`] is mostly the honest answer *"the trace does not
/// record it"*.
///
/// Report-only. No gate consumes it and no verdict changes on it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimAssumptionStamp {
    /// Options every **non-baseline** candidate's scoring sim ran under.
    pub candidates: CandidateSimAssumptions,
    /// What is knowable about the caller-supplied `baseline_trace` that
    /// candidate 0 was scored against. Deliberately mostly unknown.
    pub baseline: BaselineTraceAssumptions,
    /// Where the machine's kinematics model came from.
    pub kinematics: KinematicsSource,
    /// The vendor-LUT query the chipload band was negotiated through
    /// (Checkpoint K's `lut_query_for`). `None` = **not measured**: no
    /// [`super::context::EvaluationContext`] could be built for this
    /// toolpath (missing toolpath or missing tool), so no query was
    /// formed. Never read it as "unrouted".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lut_query: Option<LutQueryStamp>,
    /// The boundary-comparison contract in force
    /// ([`crate::tool_load::boundary::BOUNDARY_EPSILON_REL`], Checkpoint
    /// K (b1)). Recorded because a verdict's *side* at the band edge is
    /// a function of it, so two outcomes taken under different epsilons
    /// are not comparable at the bound.
    ///
    /// The optimizer makes no comparison of its own against a bound:
    /// its safety test reads [`crate::tool_load::verdict::CriterionStatus::refuses_export`],
    /// which the gates decided with this epsilon.
    pub boundary_epsilon_rel: f64,
}

/// The candidate-scoring sim's operating point. Read straight off
/// [`super::candidate::candidate_sim_options`] so it cannot drift from
/// what the sim ran (§0 rule 5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateSimAssumptions {
    /// Dexel cell for a screening simulation. Phase 1 has one cell, so
    /// this equals [`Self::refined_resolution_mm`]. Phase 2
    /// (`planning/optimize_review_2026-10-12/PLAN.md` §2) states the
    /// final shape.
    pub coarse_resolution_mm: f64,
    /// Dexel cell of every **reported** candidate verdict and cycle
    /// time.
    pub refined_resolution_mm: f64,
    /// `auto_resolution` is pinned off, so the session's own
    /// resolution choice — including whatever the on-screen baseline sim
    /// used — does not reach any candidate.
    pub auto_resolution: bool,
    /// The F-036b pin. `false` today while
    /// `SimulationOptions::default()` ships `true`.
    pub adaptive_feed_modulation: bool,
    /// Stable tag for the modulation strategy the candidate sim carries.
    /// Inert while [`Self::adaptive_feed_modulation`] is `false`, and
    /// recorded anyway so a future flip is legible against this record.
    pub modulation_strategy: crate::tool_load::ModulationStrategyTag,
    /// The feed scale the candidate sim carries. Inert on the same
    /// condition as [`Self::modulation_strategy`].
    pub modulation_feed_scale: f64,
    /// The F-035 pin. `false`, matching `SimulationOptions::default()`.
    /// While this is off, [`SimAssumptionStamp::kinematics`] cannot
    /// influence a candidate verdict.
    pub use_predicted_feed_in_gates: bool,
}

impl CandidateSimAssumptions {
    /// Read the assumptions off the real options builder. The cell is
    /// the library default (`SimulationOptions::default().resolution`);
    /// Phase 1 evaluates no candidate, so no other cell is in use.
    fn observed() -> Self {
        let opts = super::candidate::candidate_sim_options(
            crate::session::SimulationOptions::default().resolution,
        );
        Self {
            coarse_resolution_mm: opts.resolution,
            refined_resolution_mm: opts.resolution,
            auto_resolution: opts.auto_resolution,
            adaptive_feed_modulation: opts.adaptive_feed_modulation,
            modulation_strategy: opts.modulation_strategy.tag(),
            modulation_feed_scale: opts.modulation_feed_scale,
            use_predicted_feed_in_gates: opts.use_predicted_feed_in_gates,
        }
    }

    /// True when the candidate sims and `SimulationOptions::default()`
    /// disagree about feed modulation — i.e. when an optimizer verdict
    /// and a default-path verdict for the same project are taken at
    /// different operating points.
    ///
    /// Report-only helper for renderers; nothing branches on it.
    #[must_use]
    pub fn diverges_from_library_default_modulation(&self) -> bool {
        self.adaptive_feed_modulation
            != crate::session::SimulationOptions::default().adaptive_feed_modulation
    }
}

/// What the caller's `baseline_trace` will admit about how it was made.
///
/// Candidate 0 is scored against that trace directly — no re-sim — so it
/// does **not** share [`CandidateSimAssumptions`]. Two of the three
/// questions below currently have no answer on the wire, and this type
/// says so rather than filling them in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BaselineTraceAssumptions {
    /// Dexel cell the baseline trace was simulated at.
    ///
    /// **Always `None` today.** `SimulationCutTrace` records
    /// `sample_step_mm` (the along-path sampling step) and a
    /// `SimulationProvenance` of geometry hashes; it records no dexel
    /// cell. The cell survives only on `SimulationCutArtifact`, which
    /// the optimizer is not handed. `None` is therefore **not
    /// measured**, and must not be rendered as "same as the
    /// candidates" — the two are known to differ whenever the user's
    /// on-screen sim did not happen to run at
    /// [`CandidateSimAssumptions::refined_resolution_mm`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolution_mm: Option<f64>,
    /// Whether the modulator ran on the baseline trace.
    ///
    /// `Some(true)` is a **positive observation**: the trace carries
    /// per-toolpath `modulation_summaries`, which only
    /// `apply_adaptive_feed_modulation` writes. `None` is **not
    /// measured** and is deliberately never `Some(false)` — an empty
    /// map has two causes that cannot be told apart here (modulation
    /// was off, or the trace was round-tripped through serde, where the
    /// map is `#[serde(skip)]`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adaptive_feed_modulation: Option<bool>,
    /// Along-path sampling step the trace does record. Not the dexel
    /// cell; kept because it is the only sampling scale the baseline
    /// publishes, and it bounds how finely the baseline cycle time
    /// resolves.
    pub sample_step_mm: f64,
}

impl BaselineTraceAssumptions {
    fn observed(trace: &crate::stock::simulation_cut::SimulationCutTrace) -> Self {
        Self {
            // Not recorded on the trace — see the field doc.
            resolution_mm: None,
            adaptive_feed_modulation: if trace.modulation_summaries.is_empty() {
                None
            } else {
                Some(true)
            },
            sample_step_mm: trace.sample_step_mm,
        }
    }
}

/// Where the machine's kinematics model came from.
///
/// Recorded because A-5i's instrument-integrity correction showed the
/// distinction had been documented backwards: `SimulationOptions
/// ::adaptive_feed_modulation` claimed modulation fires only "when the
/// active `MachineProfile` carries `kinematics`", while
/// `MachineProfile::effective_kinematics()` falls back to the generic
/// wood router — so it applies on every machine. Every built-in preset
/// ships `kinematics: None`, which makes
/// [`Self::GenericWoodRouterFallback`] the *common* case, not the
/// exceptional one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KinematicsSource {
    /// `MachineProfile::kinematics` is `Some(_)` — the profile declares
    /// its own accel / junction-deviation model.
    ProfileDeclared,
    /// `MachineProfile::kinematics` is `None`;
    /// `MachineKinematics::generic_wood_router()` is what any consumer
    /// calling `effective_kinematics()` gets. This is what every
    /// built-in preset ships.
    GenericWoodRouterFallback,
}

impl KinematicsSource {
    fn of(machine: &crate::machine::MachineProfile) -> Self {
        if machine.kinematics.is_some() {
            Self::ProfileDeclared
        } else {
            Self::GenericWoodRouterFallback
        }
    }
}

/// The vendor-LUT query the chipload band was negotiated through.
///
/// Checkpoint K (a4) hoisted the reroute into
/// `feeds::vendor_normalize::lut_query_for` so Suggest and the gate can
/// no longer resolve different rows. The optimizer inherits that routing
/// transitively (via `tool_load::chipload::matched_chip_envelope`) but
/// never names it, so nothing on any optimizer surface said which family
/// the band came from. This records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum LutQueryStamp {
    /// A row query was formed. `queried_*` is what the LUT saw;
    /// `declared_*` is the operation's own `feeds_family` /
    /// `feeds_pass_role` before routing. They differ for `Adaptive3d`
    /// (→ `Pocket`), `ProjectCurve` (→ `Parallel`/`Contour` +
    /// `Finish`) and `DropCutter` on a FLAT tool (→ `Pocket` +
    /// `Roughing`, G-DCFLAT).
    Routed {
        declared_family: crate::feeds::vendor_lut::LutOperationFamily,
        declared_pass_role: crate::feeds::vendor_lut::LutPassRole,
        queried_family: crate::feeds::vendor_lut::LutOperationFamily,
        queried_pass_role: crate::feeds::vendor_lut::LutPassRole,
    },
    /// `lut_query_for` **refused**: `ProjectCurve` on a facing bit (a
    /// V-bit routes to the printed Trace rows instead, operator ruling
    /// 2026-09-25), where the LUT publishes no rows and inventing a
    /// family would be worse than saying so. A refusal is not a fallback
    /// to the declared family.
    Refused {
        declared_family: crate::feeds::vendor_lut::LutOperationFamily,
        declared_pass_role: crate::feeds::vendor_lut::LutPassRole,
        tool_family: crate::feeds::vendor_lut::ToolFamily,
    },
}

impl LutQueryStamp {
    /// True when routing rewrote the family or the pass role.
    #[must_use]
    pub fn is_rerouted(&self) -> bool {
        match self {
            Self::Routed {
                declared_family,
                declared_pass_role,
                queried_family,
                queried_pass_role,
            } => declared_family != queried_family || declared_pass_role != queried_pass_role,
            Self::Refused { .. } => false,
        }
    }
}

impl SimAssumptionStamp {
    /// Observe the assumptions for one optimize run.
    ///
    /// Built **outside** `optimize_toolpath_inner`, beside
    /// [`MachineSnapshot::of`], so a `Skipped` outcome that never ran a
    /// sim still names the operating point its refusal was decided at.
    #[must_use]
    pub fn of(
        session: &crate::session::ProjectSession,
        toolpath_index: usize,
        baseline_trace: &crate::stock::simulation_cut::SimulationCutTrace,
    ) -> Self {
        let lut_query = super::context::EvaluationContext::from_session(session, toolpath_index)
            .map(|ctx| {
                let tool_family = ctx.tool.to_geometry_hint().cutter_kind().lut_family();
                match crate::feeds::vendor_normalize::lut_query_for(
                    ctx.operation_kind,
                    tool_family,
                    ctx.lut_op_family,
                    ctx.lut_pass_role,
                ) {
                    Some((queried_family, queried_pass_role)) => LutQueryStamp::Routed {
                        declared_family: ctx.lut_op_family,
                        declared_pass_role: ctx.lut_pass_role,
                        queried_family,
                        queried_pass_role,
                    },
                    None => LutQueryStamp::Refused {
                        declared_family: ctx.lut_op_family,
                        declared_pass_role: ctx.lut_pass_role,
                        tool_family,
                    },
                }
            });
        Self {
            candidates: CandidateSimAssumptions::observed(),
            baseline: BaselineTraceAssumptions::observed(baseline_trace),
            kinematics: KinematicsSource::of(session.machine()),
            lut_query,
            boundary_epsilon_rel: crate::tool_load::boundary::BOUNDARY_EPSILON_REL,
        }
    }
}

impl OptimizeOutcome {
    /// Construct a `Ranked` outcome. Caller is responsible for sorting
    /// `candidates` (baseline at index 0) and computing
    /// `recommended_index` via [`ProjectOptimizeReport::first_safe_index`].
    pub fn ranked(
        candidates: Vec<OptimizeCandidate>,
        recommended_index: Option<usize>,
        narrative: OutcomeNarrative,
    ) -> Self {
        Self {
            kind: OutcomeKind::Ranked,
            candidates,
            narrative: Box::new(narrative),
            recommended_index,
            reason: None,
            machine_snapshot: None,
            assumptions: None,
        }
    }

    /// Construct a `NoSafeImprovement` outcome. `candidates` includes
    /// the baseline at index 0 plus every candidate the optimizer
    /// managed to evaluate; the prior `attempted` field is folded in.
    pub fn no_safe_improvement(
        candidates: Vec<OptimizeCandidate>,
        reason: RefuseReason,
        narrative: OutcomeNarrative,
    ) -> Self {
        Self {
            kind: OutcomeKind::NoSafeImprovement,
            candidates,
            narrative: Box::new(narrative),
            recommended_index: None,
            reason: Some(reason),
            machine_snapshot: None,
            assumptions: None,
        }
    }

    /// Construct a `Skipped` outcome — the optimizer refused before
    /// evaluating any candidate.
    pub fn skipped(reason: RefuseReason) -> Self {
        Self {
            kind: OutcomeKind::Skipped,
            candidates: Vec::new(),
            narrative: Box::default(),
            recommended_index: None,
            reason: Some(reason),
            machine_snapshot: None,
            assumptions: None,
        }
    }

    /// The recommended candidate of a `Ranked` outcome: the first
    /// non-baseline candidate that is safe ([`candidate_is_safe`]) and
    /// faster than the baseline. Returns `None` for any other `kind`.
    ///
    /// A safe candidate that is not faster is information, not a
    /// recommendation.
    pub fn first_safe(&self) -> Option<&OptimizeCandidate> {
        if self.kind != OutcomeKind::Ranked {
            return None;
        }
        ProjectOptimizeReport::first_safe_index(&self.candidates)
            .and_then(|index| self.candidates.get(index))
    }
}

/// Project-level rollup over every enabled toolpath. Surfaced by U3's
/// Optimize-project view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectOptimizeReport {
    /// Baseline project cycle time (seconds), as measured by the sim
    /// already on screen.
    pub baseline_cycle_time_s: f64,
    /// Toolpath index that dominates runtime (the "Bottleneck:"
    /// callout). `None` if no toolpath crosses the threshold (currently
    /// 30% of total runtime — calibrated against wanaka in U3).
    pub bottleneck_index: Option<usize>,
    /// Per-toolpath outcome paired with the toolpath index it relates
    /// to.
    pub per_toolpath: Vec<(usize, OptimizeOutcome)>,
}

impl ProjectOptimizeReport {
    /// Index of the first non-baseline candidate that is safe (no
    /// criterion row refuses an export) and faster than the baseline.
    /// Returns `None` if no such candidate exists. This is the index
    /// version of [`OptimizeOutcome::first_safe`], so a caller can
    /// change the candidate in place (for example the U4 reconciliation
    /// writes the reconciled values).
    pub fn first_safe_index(candidates: &[OptimizeCandidate]) -> Option<usize> {
        let baseline = candidates.first()?;
        candidates
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, c)| candidate_is_safe(c) && c.cycle_time_s < baseline.cycle_time_s)
            .map(|(i, _)| i)
    }

    /// Estimated project cycle time if the operator applied exactly the
    /// rows flagged `true` in `selected` (parallel to `per_toolpath`;
    /// missing trailing entries are treated as unselected).
    ///
    /// Computed as the true project baseline **minus** the realized
    /// per-row savings, rather than re-summing per-row baselines. Only a
    /// selected `Ranked` row with a recommended faster candidate
    /// (`first_safe`) contributes a saving; every other row keeps its
    /// real cost in the total — including `Skipped` rows, which carry no
    /// candidate cycle at all and so can't be re-summed honestly. The
    /// pre-W0.2 header re-summed per-row baselines and folded refused /
    /// skipped rows to `0.0`, understating the optimized time and
    /// inflating the headline savings on every such row (OPT-001).
    pub fn optimized_cycle_time_s(&self, selected: &[bool]) -> f64 {
        let total_saving: f64 = self
            .per_toolpath
            .iter()
            .zip(selected.iter().chain(std::iter::repeat(&false)))
            .filter_map(|((_, outcome), &sel)| {
                if !sel {
                    return None;
                }
                // `first_safe` only resolves for `Ranked` outcomes faster
                // than baseline, so non-applicable rows contribute nothing.
                let baseline = outcome.candidates.first()?.cycle_time_s;
                let improved = outcome.first_safe()?.cycle_time_s;
                Some((baseline - improved).max(0.0))
            })
            .sum();
        (self.baseline_cycle_time_s - total_saving).max(0.0)
    }
}

/// True when no criterion row of the candidate's verdict refuses a
/// g-code export.
///
/// This is the export gate's predicate
/// ([`crate::gcode::refusing_exceedances`] over
/// [`crate::tool_load::verdict::ToolpathLoadVerdict::criteria`]), so the
/// drill rows and the own-material rows count too. A row that exceeds a
/// bound that may not gate an export (for example the depth row, S3)
/// does not make a candidate unsafe; the narrative still names it.
pub(crate) fn candidate_is_safe(candidate: &OptimizeCandidate) -> bool {
    let criteria = candidate.verdict.criteria();
    crate::gcode::refusing_exceedances(&criteria).is_empty()
}

/// Build an `OptimizeOutcome` from the baseline candidate and the
/// evaluated candidates.
///
/// - No candidates → `NoSafeImprovement` with the "no candidate was
///   searched" narrative.
/// - The safe candidates sort by ascending `cycle_time_s`. When the
///   fastest safe candidate is faster than the baseline → `Ranked`.
/// - Otherwise → `NoSafeImprovement`. The narrative names the limiting
///   rows of the fastest candidate.
pub(crate) fn build_outcome(
    baseline: OptimizeCandidate,
    candidates: Vec<OptimizeCandidate>,
) -> OptimizeOutcome {
    if candidates.is_empty() {
        let narrative = build_no_search_narrative(&baseline);
        return OptimizeOutcome::no_safe_improvement(
            vec![baseline],
            RefuseReason::NoImprovementFound,
            narrative,
        );
    }

    let (mut safe, mut unsafe_candidates): (Vec<_>, Vec<_>) =
        candidates.into_iter().partition(candidate_is_safe);
    safe.sort_by(|a, b| a.cycle_time_s.total_cmp(&b.cycle_time_s));
    unsafe_candidates.sort_by(|a, b| a.cycle_time_s.total_cmp(&b.cycle_time_s));

    let baseline_cycle = baseline.cycle_time_s;
    let any_faster_safe = safe
        .first()
        .is_some_and(|c| c.cycle_time_s < baseline_cycle);

    let mut ordered = Vec::with_capacity(safe.len() + unsafe_candidates.len() + 1);
    ordered.push(baseline);
    ordered.extend(safe);
    ordered.extend(unsafe_candidates);

    if any_faster_safe {
        let recommended_index = ProjectOptimizeReport::first_safe_index(&ordered);
        let narrative = build_ranked_narrative(&ordered);
        return OptimizeOutcome::ranked(ordered, recommended_index, narrative);
    }

    let narrative = build_no_safe_narrative(&ordered);
    OptimizeOutcome::no_safe_improvement(ordered, RefuseReason::NoImprovementFound, narrative)
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

    fn report_with(baseline_s: f64, outcomes: Vec<OptimizeOutcome>) -> ProjectOptimizeReport {
        ProjectOptimizeReport {
            baseline_cycle_time_s: baseline_s,
            bottleneck_index: None,
            per_toolpath: outcomes.into_iter().enumerate().collect(),
        }
    }

    #[test]
    fn skipped_and_refused_rows_do_not_deflate_optimized_total() {
        // Two rows the optimizer can't improve: a Skipped drill (no
        // candidates at all) and a NoSafeImprovement. Neither has a
        // recommended faster candidate, so the optimized estimate must
        // equal the baseline. The pre-W0.2 header folded these to zero,
        // understating optimized time and inflating the savings %.
        let report = report_with(
            120.0,
            vec![
                OptimizeOutcome::skipped(RefuseReason::NoImprovementFound),
                OptimizeOutcome::no_safe_improvement(
                    Vec::new(),
                    RefuseReason::NoImprovementFound,
                    OutcomeNarrative::default(),
                ),
            ],
        );
        // Even with both rows checked, nothing is applicable → no saving.
        assert_eq!(report.optimized_cycle_time_s(&[true, true]), 120.0);
    }

    #[test]
    fn empty_selection_returns_exact_baseline() {
        let report = report_with(
            80.0,
            vec![OptimizeOutcome::skipped(RefuseReason::NoImprovementFound)],
        );
        // No selection slice at all — missing entries treated unselected.
        assert_eq!(report.optimized_cycle_time_s(&[]), 80.0);
    }

    // ── build_outcome: one safety predicate, rank by cycle time ──────

    use super::super::{ParamDelta, SearchStage};
    use crate::compute::catalog::OperationConfig;
    use crate::compute::operation_configs::PocketConfig;
    use crate::ids::ToolpathId;
    use crate::tool_load::verdict::{
        ChipBounds, ChipBoundsSource, ChipSide, ChiploadMetric, ChiploadStatistic, ChiploadVerdict,
        Confidence, DeflectionBounds, DeflectionVerdict, DepthVerdict, PowerVerdict,
        SampleEvidence, ToolpathLoadVerdict,
    };

    fn band() -> ChipBounds {
        ChipBounds {
            min_mm_per_tooth: Some(0.038),
            max_mm_per_tooth: 0.07,
            source: ChipBoundsSource::VendorLut,
        }
    }

    fn metric(observed: f64, statistic: ChiploadStatistic) -> ChiploadMetric {
        ChiploadMetric {
            observed_mm_per_tooth: observed,
            statistic,
            evidence: SampleEvidence::at(0),
            bounds: band(),
        }
    }

    fn clean_verdict() -> ToolpathLoadVerdict {
        ToolpathLoadVerdict {
            toolpath_id: ToolpathId(0),
            chipload: ChiploadVerdict::Within {
                approach_to_min: Some(metric(0.05, ChiploadStatistic::MedianLow)),
                approach_to_max: metric(0.05, ChiploadStatistic::PeakInRange),
                confidence: Confidence::Validated,
                entry_spikes: Vec::new(),
                burn_advisory: None,
            },
            power: PowerVerdict::Within {
                peak_kw: 0.05,
                available_kw: 0.6,
                evidence: SampleEvidence::empty(),
                confidence: Confidence::Validated,
                entry_spike: None,
                bound_source: None,
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
            depth: DepthVerdict::fixture_within(),
            drill_gates: None,
            modulation_summary: None,
            feed_explanation: None,
            kinematic_utilization: None,
            material_split: None,
        }
    }

    fn chipload_breach() -> ToolpathLoadVerdict {
        ToolpathLoadVerdict {
            chipload: ChiploadVerdict::Exceeds {
                side: ChipSide::High,
                triggering: metric(0.09, ChiploadStatistic::PeakHigh),
                confidence: Confidence::Validated,
            },
            ..clean_verdict()
        }
    }

    fn depth_breach() -> ToolpathLoadVerdict {
        ToolpathLoadVerdict {
            depth: DepthVerdict::Exceeds {
                peak_mm: 3.0,
                bound: crate::machine::RigidityDepthCap {
                    factor: 0.25,
                    diameter_mm: 6.0,
                },
                evidence: SampleEvidence::at(0),
                confidence: Confidence::Validated,
            },
            ..clean_verdict()
        }
    }

    fn cand(cycle_s: f64, verdict: ToolpathLoadVerdict, stage: SearchStage) -> OptimizeCandidate {
        OptimizeCandidate {
            params: OperationConfig::Pocket(PocketConfig::default()),
            delta: ParamDelta::default(),
            cycle_time_s: cycle_s,
            verdict,
            stage,
            reconciled_cycle_time_s: None,
            reconciled_verdict: None,
            air_cut_fraction_of_total_runtime: None,
        }
    }

    /// Phase 1: no candidate gives the exact "no search" narrative.
    #[test]
    fn no_candidates_give_the_no_search_narrative() {
        let outcome = build_outcome(
            cand(100.0, clean_verdict(), SearchStage::Baseline),
            Vec::new(),
        );
        assert_eq!(outcome.kind, OutcomeKind::NoSafeImprovement);
        assert_eq!(outcome.reason, Some(RefuseReason::NoImprovementFound));
        assert_eq!(outcome.candidates.len(), 1);
        assert_eq!(outcome.narrative.headline, "No candidate was searched.");
        assert_eq!(
            outcome.narrative.explanation,
            "The search has no levers yet; the post-sim tuner is being rebuilt."
        );
        assert!(outcome.narrative.limiting_gates.is_empty());
    }

    /// A depth row over its rule-of-thumb cap does not veto a candidate
    /// (S3); a chipload row over a vendor bound does.
    #[test]
    fn the_depth_row_is_reported_and_the_chipload_row_refuses() {
        assert!(candidate_is_safe(&cand(
            50.0,
            depth_breach(),
            SearchStage::Measured
        )));
        assert!(!candidate_is_safe(&cand(
            50.0,
            chipload_breach(),
            SearchStage::Measured
        )));
    }

    /// The fastest SAFE candidate is recommended; a faster unsafe one is
    /// sorted after the safe ones.
    #[test]
    fn the_fastest_safe_candidate_is_ranked_first() {
        let outcome = build_outcome(
            cand(100.0, clean_verdict(), SearchStage::Baseline),
            vec![
                cand(90.0, clean_verdict(), SearchStage::Measured),
                cand(40.0, chipload_breach(), SearchStage::Measured),
                cand(70.0, depth_breach(), SearchStage::Measured),
            ],
        );
        assert_eq!(outcome.kind, OutcomeKind::Ranked);
        let cycles: Vec<f64> = outcome.candidates.iter().map(|c| c.cycle_time_s).collect();
        assert_eq!(cycles, vec![100.0, 70.0, 90.0, 40.0]);
        assert_eq!(outcome.recommended_index, Some(1));
        assert!((outcome.first_safe().unwrap().cycle_time_s - 70.0).abs() < 1e-12);
    }

    /// No safe candidate is faster: `NoSafeImprovement`, and the
    /// narrative names the rows of the fastest candidate.
    #[test]
    fn no_faster_safe_candidate_names_the_fastest_rows() {
        let outcome = build_outcome(
            cand(100.0, clean_verdict(), SearchStage::Baseline),
            vec![
                cand(120.0, clean_verdict(), SearchStage::Measured),
                cand(60.0, chipload_breach(), SearchStage::Measured),
            ],
        );
        assert_eq!(outcome.kind, OutcomeKind::NoSafeImprovement);
        assert_eq!(outcome.recommended_index, None);
        assert!(outcome.first_safe().is_none());
        let gates = &outcome.narrative.limiting_gates;
        assert_eq!(gates.len(), 1, "{gates:?}");
        assert_eq!(
            gates[0].gate,
            crate::tool_load::optimize::GateKind::Chipload
        );
    }
}
