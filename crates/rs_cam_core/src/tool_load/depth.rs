//! **Depth-of-cut guardrail — the depth the cut actually took.**
//! S3 (2026-09-18), `SURFACE_IMPL.md` §1 and `PLAN.md` §11 Reading A.
//!
//! Depth of cut sets the depth on nearly every recipe this engine
//! ships, and until this gate existed the operator could not see it as
//! a limit. The cap lived in one place: `feeds::suggest::invariants`,
//! where `clamp_dpp_to_rigidity` lowered the depth per pass and pushed
//! a `RoughingDepthClampedToRigidity` warning. After the clamp the
//! number disappeared.
//!
//! ## Why this is a criterion only AFTER a simulation
//!
//! Before a simulation the depth is a value the engine chose and then
//! clamped, so a row would sit pinned at 100 % and say nothing. It
//! stays a rationale entry there.
//!
//! After a simulation it is different in kind. The engaged depth VARIES
//! per sample — entry ramps, curved surfaces and arcs all cut
//! shallower or deeper than the parameter says — so the peak over the
//! population is a measurement, exactly like the peak chipload and the
//! peak tip deflection beside it.
//!
//! ## The quantity: `axial_engagement_mm`, not `axial_doc_mm`
//!
//! [`SimulationCutSample`] carries both. `axial_doc_mm` is the LEGACY
//! wire name and reads `0.0` on a pure-vertical plunge, where the
//! descent goes to `plunge_descent_mm` instead.
//! [`SimulationCutSample::axial_engagement_mm`] is the axis the emitter
//! documents as "maximum material height engaged by lateral/arc/helix
//! cutting at this sample", and it is the axis the deflection and
//! chip-geometry gates already consume. The rigidity cap is a bound on
//! how much cutting edge is buried in the work, so this is the reading
//! it judges — and it is the same field `session::compute` folds to a
//! maximum for the toolpath's own axial figure, so the criterion and
//! the toolpath statistic describe one quantity.
//!
//! ## The bound, and why it does not gate
//!
//! The cap is [`crate::feeds::force_budget::rigidity_depth_cap`] (P13c, O1;
//! the Adaptive family since P29-4; a slot or a groove under every role
//! since P29-5): on every roughing cut the depth where the peak cutting
//! force at a sample's own width and chip meets the machine force budget.
//! The budget stands in for an unmeasured
//! frame, so the cap carries
//! [`BoundSource::MachineForceBudget`](crate::tool_load::verdict::BoundSource::MachineForceBudget), whose `gates_export()` is
//! false. An exceedance is reported; the export goes ahead. The day the
//! machine is measured, the variant is replaced by a measured one and
//! the row gates with no change to any renderer (`PLAN.md` §11).
//!
//! ## Every sample is judged against its own cap (P29-6 6c)
//!
//! The population is every sample of the toolpath that removes material
//! and has a depth. Each sample with a width is judged: its depth against
//! the cap at its own width and chip. The verdict names the sample with
//! the largest depth over its own cap (the judged sample), and the row
//! prints that sample's depth and cap (ruling Q3); the text names the
//! deepest depth too. A sample with a depth and no width (no arc, or an arc
//! of 0) gets no number: the full-slot fallback is deleted. A screen on the
//! force at `depth / r*` solves the cap only for a sample that can beat the
//! worst ratio `r*` so far.
//!
//! ## One engagement door with the force reading (P29-11 11-5, F6-a)
//!
//! The width and the chip of a sample come from the doors of the force
//! reading (`super::cut_force`) and the deflection gate:
//! [`super::deflection::sample_engagement`] (the arc at the engaged depth)
//! and [`super::deflection::deflection_feed_per_tooth_mm`] (the one
//! operating point of the sample). The width share `radial_woc_fraction`
//! is not read. So the population of this gate is the population of the
//! force reading, and the row `Exceeds` exactly when a sample's force at
//! its depth is over the budget. When every judged sample has no
//! force-budget root below the flute, the force at the flute length is at
//! or under the budget: the gate gives `Within` with the flute length as
//! the bound (before 11-5: `Reported`, a row with no number).
//!
//! **This gate adds no number.** The two helpers it calls are the ones
//! the Suggest clamp calls, so the cap the recipe was lowered to and the
//! cap this row draws against cannot disagree.
//!
//! ## A one-sided finishing pass has no bound (feeds matrix R2, 2026-09-23)
//!
//! No vendor publishes an axial finishing cap as a fraction of D for
//! wood. For a `Finish` or `SemiFinish` pass that cuts on one side the
//! profile gives no cap, and this gate returns [`DepthVerdict::Reported`]:
//! the peak and its population, with no bound. The deflection gate is the
//! limit on that pass. A slot or a groove reads the force budget under
//! every role (P29-5, ruling R3).
//!
//! ## The comparison tolerance
//!
//! The dexel stock stores lengths in `f32`, so the peak carries `f32`
//! noise. The comparison widens the cap by
//! [`DEXEL_DEPTH_RESOLUTION_MM`] (EVIDENCE 5.1-6).
//!
//! ## Refusal cases
//!
//! - Drill cycle → `Unmodeled(NotApplicableForOp)`, the same clause the
//!   three milling gates use: a Z-only cycle has no continuous
//!   engagement to measure.
//! - No simulation trace → `Unmodeled(SimulationRequired)`.
//! - No `MachineProfile` reached the evaluator → `Unmodeled(NotImplemented)`,
//!   mirroring `power.rs`. There is no profile, so there is no factor.
//! - No usable tool diameter → `Unmodeled(NotImplemented)`.
//!
//! A trace whose samples all filter out is **not** a refusal: it is a
//! vacuous `Within` that states its empty [`GatePopulation`] (X-VAC),
//! the same contract `power.rs` and `deflection.rs` carry. Its cap is read
//! at the first sample that states a feed, a speed and a width; with no
//! such sample the gate reports with no cap.

use crate::stock::simulation_cut::SimulationCutSample;
use crate::tool::MillingCutter as _;

use super::locality::SpanLookup;
use super::verdict::{
    ChiploadStatistic, Confidence, DepthVerdict, GatePopulation, PopulationUnit, SampleEvidence,
    UnmodeledReason,
};

/// The depth resolution of the dexel stock, in mm. EVIDENCE 5.1-6.
///
/// The stock stores each segment end as an `f32`, so a removed column
/// is the difference of two `f32` lengths, with an error of about two
/// `f32` ulp of the ray length. One ulp at 256 to 512 mm is about
/// 3.1e-5 mm, so 1e-4 mm covers a column in stock up to 512 mm deep.
/// The 18 mm fixture of EVIDENCE 5.1-6 read 7.6e-7 mm. The 8-ulp `f64`
/// slack in `boundary` does not cover it: that slack is about 2e-15 mm
/// at a 1.2 mm cap.
pub const DEXEL_DEPTH_RESOLUTION_MM: f64 = 1.0e-4;

/// The headline depth verdict of the toolpath (the one driver,
/// `groups::judge_by_group`).
pub fn evaluate(ctx: &super::ToolpathLoadContext<'_>, env: &super::GateEnv<'_>) -> DepthVerdict {
    super::groups::headline::<super::DepthGate>(ctx, env)
}

/// The depth verdict of one group: each sample with a depth against the
/// force-budget depth at its own width and chip.
#[tracing::instrument(level = "debug", skip_all, fields(toolpath_id = ctx.toolpath_id().raw(), group = ?population.group))]
pub(super) fn judge(
    ctx: &super::ToolpathLoadContext<'_>,
    env: &super::GateEnv<'_>,
    population: super::groups::GroupPopulation<'_>,
) -> DepthVerdict {
    let (toolpath_id, tool, material, spans) =
        (ctx.toolpath_id(), ctx.tool(), ctx.material(), ctx.spans());
    let &super::GateEnv {
        sim_trace,
        machine,
        tolerance,
        ..
    } = env;
    let _ = tolerance;

    let Some(trace) = sim_trace else {
        tracing::debug!(
            reason = "SimulationRequired",
            "depth gate refuses: no simulation trace"
        );
        return DepthVerdict::Unmodeled {
            reason: UnmodeledReason::SimulationRequired,
        };
    };
    let Some(machine) = machine else {
        tracing::debug!(
            reason = "NotImplemented",
            "depth gate refuses: no machine profile reached the evaluator"
        );
        return DepthVerdict::Unmodeled {
            reason: UnmodeledReason::NotImplemented(
                "no machine profile reached the load evaluator".to_owned(),
            ),
        };
    };

    // P29-11 S10: the facts of this group's cuts
    // (`ToolpathLoadContext::group_facts`, filled from the cut list of the
    // op; the sentry `every_op_declares_its_cuts_p29_3` holds the depth
    // limit equal to the registry row). P29-6 6c: the gate reads the
    // facts, not the op kind.
    let facts = ctx.group_facts(population.group);
    let span_lookup = spans.map(SpanLookup::new);

    // The population (P29-6 6c, 6d): every sample of the group that removes
    // material and has a depth. X-VAC: `offered` counts every sample of the
    // toolpath, so an empty population says it rests on nothing.
    let offered = population.toolpath.toolpath_samples;
    let group_samples = population.samples;
    let mut population: Vec<(usize, &SimulationCutSample)> = Vec::new();
    let mut deepest: Option<(usize, f64)> = None;
    for &(i, s) in group_samples {
        if !is_depth_sample(s) {
            continue;
        }
        if deepest.is_none_or(|(_, d)| s.axial_engagement_mm > d) {
            deepest = Some((i, s.axial_engagement_mm));
        }
        population.push((i, s));
    }
    let deepest_mm = deepest.map_or(0.0, |(_, d)| d);
    let evidence_at = |idx: Option<usize>, contributing: usize, statistic: ChiploadStatistic| {
        let population = GatePopulation::new(contributing, offered, PopulationUnit::Samples);
        match idx {
            Some(idx) => SampleEvidence::at_with_stat(idx, statistic)
                .with_locality(trace.samples.get(idx).and_then(|s| {
                    super::locality::classify_sample_locality(s, span_lookup.as_ref())
                }))
                .with_population(population),
            None => SampleEvidence::empty().with_population(population),
        }
    };

    // EXEMPT(P28-02)
    let Ok(crate::machine::DepthCapRule { budget_n }) = machine.rigidity.force_budget_of(facts)
    else {
        // Feeds matrix R2 (2026-09-23): a finishing or semi-finishing
        // pass that cuts on one side has no axial cap; a slot or a groove
        // reads the force budget (P29-5, ruling R3). The drill family also
        // has none, but `limits::scope` does not judge the drill group. The gate
        // reports the deepest depth and judges nothing; the deflection
        // gate is the limit on this pass.
        tracing::debug!(
            verdict = "Reported",
            deepest_mm,
            "depth gate reports: this cut carries no axial cap"
        );
        return DepthVerdict::Reported {
            peak_mm: deepest_mm,
            evidence: evidence_at(
                deepest.map(|(i, _)| i),
                population.len(),
                ChiploadStatistic::PeakInRange,
            ),
            confidence: Confidence::Approximate(format!(
                "peak engaged depth {deepest_mm:.3} mm; a finishing pass that cuts \
                 on one side has no depth limit, and the deflection gate is its limit"
            )),
        };
    };

    // P29-6 6c (row `M5` closed): each sample is judged against its OWN
    // cap, the force-budget depth at its own width and chip
    // (`force_budget::rigidity_depth_cap`). The verdict names the sample
    // with the largest depth over its cap. A sample with no width gets no
    // number: no full-slot fallback.
    //
    // The screen: the gate keeps the worst ratio `r*` so far. A sample can
    // beat it only when its cap is below `depth / r*`, that is when the
    // force at `depth / r*` is over the budget (the force does not fall
    // with depth: one root, P29-4 finding Q5). Past the flute length the
    // screen reads the force at the flute: with no force over the budget
    // there, the sample has no root below the flute. Only a sample that
    // passes the screen pays for a solve.
    let flute_mm = tool.length();
    let mut contributing: usize = 0;
    let mut no_width: usize = 0;
    let mut no_chip: usize = 0;
    let mut unmodelled: Option<&'static str> = None;
    let mut judged: Option<JudgedSample> = None;
    // P29-11 11-5 (F6-a): the deepest sample that the force budget holds
    // with no root below the flute; it is the judged sample when no sample
    // has a cap.
    let mut deepest_no_root: Option<NoRootSample> = None;
    for &(i, s) in &population {
        let Some(SampleCut { ae_mm, fz_mm }) = sample_cut(s, tool, &trace.predicted_feeds) else {
            no_width += 1;
            continue;
        };
        if !(fz_mm.is_finite() && fz_mm > 0.0) {
            no_chip += 1;
            continue;
        }
        let depth_mm = s.axial_engagement_mm;
        let no_root = NoRootSample {
            idx: i,
            depth_mm,
            ae_mm,
            fz_mm,
        };
        let screen_ap = judged
            .as_ref()
            .map_or(flute_mm, |j| (depth_mm / j.ratio).min(flute_mm));
        let cannot_beat =
            crate::feeds::force_budget::peak_force_n(tool, material, screen_ap, ae_mm, fz_mm)
                .is_some_and(|f| f <= budget_n);
        if cannot_beat {
            contributing += 1;
            if screen_ap >= flute_mm {
                // The screen read the force at the flute: no root below it.
                match no_root_sample(no_root, tool, material, flute_mm, budget_n, &mut judged) {
                    Ok(Some(n)) => n.keep_deepest(&mut deepest_no_root),
                    Ok(None) => {}
                    Err(reason) => unmodelled = Some(reason),
                }
            }
            continue;
        }
        match crate::feeds::force_budget::rigidity_depth_cap(
            machine, facts, tool, material, ae_mm, fz_mm,
        ) {
            Some(crate::feeds::force_budget::RigidityCapOutcome::Cap(bound)) => {
                let cap_mm = bound.cap_mm();
                if !cap_mm.is_finite() || cap_mm <= 0.0 {
                    unmodelled = Some("the tool reports no usable diameter for the rigidity cap");
                    continue;
                }
                contributing += 1;
                let ratio = depth_mm / cap_mm;
                if judged.as_ref().is_none_or(|j| ratio > j.ratio) {
                    judged = Some(JudgedSample {
                        idx: i,
                        depth_mm,
                        ratio,
                        bound,
                    });
                }
            }
            Some(crate::feeds::force_budget::RigidityCapOutcome::NoLimitBelowFlute) => {
                contributing += 1;
                match no_root_sample(no_root, tool, material, flute_mm, budget_n, &mut judged) {
                    Ok(Some(n)) => n.keep_deepest(&mut deepest_no_root),
                    Ok(None) => {}
                    Err(reason) => unmodelled = Some(reason),
                }
            }
            Some(crate::feeds::force_budget::RigidityCapOutcome::Unmodelled(reason)) => {
                unmodelled = Some(reason);
            }
            // The rule above holds a budget, so the producer answers.
            None => {}
        }
    }
    // The samples that removed material at a depth and got no number.
    let no_number_clause = match (no_width, no_chip) {
        (0, 0) => String::new(),
        (w, 0) => format!("; {w} samples with a depth have no measured width (no number)"),
        (0, c) => format!("; {c} samples with a depth state no feed or speed (no number)"),
        (w, c) => format!(
            "; {w} samples with a depth have no measured width and {c} state no feed or \
             speed (no number)"
        ),
    };

    let Some(JudgedSample {
        idx,
        depth_mm: peak_mm,
        bound,
        ..
    }) = judged
    else {
        if let Some(reason) = unmodelled {
            tracing::debug!(
                reason,
                "depth gate refuses: the force budget cannot set a depth"
            );
            return DepthVerdict::Unmodeled {
                reason: UnmodeledReason::NotImplemented(reason.to_owned()),
            };
        }
        if let Some(no_root) = deepest_no_root {
            // P29-11 11-5 (F6-a): every judged sample has no force-budget
            // root below the flute. The force at the flute length is at or
            // under the budget, so no depth the tool can cut reaches it:
            // that is a proof of `Within`, with the flute length as the
            // bound (the force reading shows the same force).
            return no_root_verdict(
                no_root,
                flute_mm,
                budget_n,
                deepest_mm,
                &no_number_clause,
                |idx, statistic| evidence_at(Some(idx), contributing, statistic),
            );
        }
        if !population.is_empty() {
            // Every sample with a depth has no number: no cut to judge.
            return DepthVerdict::Reported {
                peak_mm: deepest_mm,
                evidence: evidence_at(deepest.map(|(i, _)| i), 0, ChiploadStatistic::PeakInRange),
                confidence: Confidence::Approximate(format!(
                    "peak engaged depth {deepest_mm:.3} mm; no sample with a depth has a \
                     number, so the force budget judges none{no_number_clause}"
                )),
            };
        }
        // X-VAC: the cut of the first sample of the toolpath that states a
        // feed, a speed and a width.
        let first_cut = trace.samples.iter().find_map(|s| {
            if s.toolpath_id != toolpath_id {
                return None;
            }
            let SampleCut { ae_mm, fz_mm } = sample_cut(s, tool, &trace.predicted_feeds)?;
            (fz_mm.is_finite() && fz_mm > 0.0).then_some((ae_mm, fz_mm))
        });
        return vacuous_verdict(ctx, machine, facts, first_cut, || {
            evidence_at(None, 0, ChiploadStatistic::PeakInRange)
        });
    };

    // EVIDENCE 5.1-6 / 5.2-16: the dexel stock stores its segment ends
    // in `f32`, so a removed column is an `f32` difference. A Profile
    // cut at exactly the cap read 7.6e-7 mm above it and printed
    // `Exceeds` at 1.2000 against 1.2000. The tolerance is the dexel
    // resolution, as a fraction of the cap, in the form the other gates
    // pass their own tolerance.
    let cap_mm = bound.cap_mm();
    let exceeds =
        super::boundary::exceeds_high(peak_mm, cap_mm, DEXEL_DEPTH_RESOLUTION_MM / cap_mm);
    // The statistic is the chipload gate's own vocabulary: a peak read
    // against a high bound, `PeakHigh` when it trips and `PeakInRange`
    // when it does not. The judged sample is the worst depth over its own
    // cap; a median would hide it.
    let statistic = if exceeds {
        ChiploadStatistic::PeakHigh
    } else {
        ChiploadStatistic::PeakInRange
    };
    let evidence = evidence_at(Some(idx), contributing, statistic);
    // Ruling Q3: the row prints the depth and the cap of the judged
    // sample; the text names the deepest depth too. The confidence is
    // `Approximate` on both decided arms because the budget stands in for
    // an unmeasured frame, and that is a property of the bound, not of
    // this cut.
    let confidence = Confidence::Approximate(match bound {
        crate::machine::RigidityDepthCap::ForceBudget {
            budget_n,
            ae_mm,
            fz_mm,
            ..
        } => format!(
            "judged sample: engaged depth {peak_mm:.3} mm against the depth {cap_mm:.3} mm \
             where the peak cutting force at width {ae_mm:.2} mm and {fz_mm:.4} mm/tooth \
             meets the machine force budget {budget_n:.1} N (the largest depth over its own \
             cap); deepest engaged depth {deepest_mm:.3} mm{no_number_clause}"
        ),
    });

    if exceeds {
        tracing::warn!(
            verdict = "Exceeds",
            peak_mm,
            cap_mm,
            "depth gate Exceeds: the measured depth is past the machine rigidity cap"
        );
        return DepthVerdict::Exceeds {
            peak_mm,
            bound,
            evidence,
            confidence,
        };
    }
    DepthVerdict::Within {
        peak_mm,
        bound,
        evidence,
        confidence,
    }
}

/// The depth histogram's value of a sample: its engaged depth when the
/// gate gives the sample a number, else `None`. The histogram calls it
/// after the gate's span filter, so it bins the gate's own population: on
/// a cut with no depth limit every sample with a depth; on a force-budget
/// cut the samples with a width and a chip (P29-6 6c). The histogram bins
/// the headline group, so the rule is that group's (P29-11 S10).
pub(super) fn depth_number(
    s: &SimulationCutSample,
    ctx: &super::ToolpathLoadContext<'_>,
    machine: Option<&crate::machine::MachineProfile>,
    predicted_feeds: &crate::machine::kinematics::PredictedFeedMap,
) -> Option<f64> {
    if !is_depth_sample(s) {
        return None;
    }
    // EXEMPT(P28-02)
    let has_rule = machine.is_some_and(|m| {
        m.rigidity
            .force_budget_of(ctx.group_facts(ctx.headline_group()))
            .is_ok()
    });
    if !has_rule {
        return Some(s.axial_engagement_mm);
    }
    let SampleCut { fz_mm, .. } = sample_cut(s, ctx.tool(), predicted_feeds)?;
    (fz_mm.is_finite() && fz_mm > 0.0).then_some(s.axial_engagement_mm)
}

/// The sample with the largest depth over its own cap so far.
struct JudgedSample {
    idx: usize,
    depth_mm: f64,
    /// `depth_mm / cap`.
    ratio: f64,
    bound: crate::machine::RigidityDepthCap,
}

/// The deepest sample so far that the force budget holds with no root
/// below the flute (P29-11 11-5).
#[derive(Clone, Copy)]
struct NoRootSample {
    idx: usize,
    depth_mm: f64,
    ae_mm: f64,
    fz_mm: f64,
}

impl NoRootSample {
    fn keep_deepest(self, deepest: &mut Option<Self>) {
        if deepest.is_none_or(|d| self.depth_mm > d.depth_mm) {
            *deepest = Some(self);
        }
    }
}

/// P29-11 11-5 (F6-a): a sample with no force-budget root below the flute.
/// At or under the flute length it holds (`Some`, a candidate of
/// [`no_root_verdict`]). The producer solves the root only up to the flute,
/// so a sample DEEPER than the flute is judged by its own force at its own
/// depth (`force_budget::peak_force_n`, the number of the force reading):
/// over the budget it becomes the judged sample against the flute length
/// when it beats the worst ratio (`None`); at or under, it holds (`Some`).
/// `Err`: the force model gives no number at its depth.
fn no_root_sample(
    no_root: NoRootSample,
    tool: &crate::tool::ToolDefinition,
    material: &crate::material::Material,
    flute_mm: f64,
    budget_n: f64,
    judged: &mut Option<JudgedSample>,
) -> Result<Option<NoRootSample>, &'static str> {
    if no_root.depth_mm <= flute_mm {
        return Ok(Some(no_root));
    }
    let force_n = crate::feeds::force_budget::peak_force_n(
        tool,
        material,
        no_root.depth_mm,
        no_root.ae_mm,
        no_root.fz_mm,
    )
    .ok_or(crate::feeds::force_budget::NO_BUDGET_INPUT)?;
    if force_n <= budget_n {
        return Ok(Some(no_root));
    }
    let ratio = no_root.depth_mm / flute_mm;
    if judged.as_ref().is_none_or(|j| ratio > j.ratio) {
        *judged = Some(JudgedSample {
            idx: no_root.idx,
            depth_mm: no_root.depth_mm,
            ratio,
            bound: crate::machine::RigidityDepthCap::ForceBudget {
                budget_n,
                ae_mm: no_root.ae_mm,
                fz_mm: no_root.fz_mm,
                depth_mm: flute_mm,
                below_floor: false,
            },
        });
    }
    Ok(None)
}

/// P29-11 11-5 (F6-a): the verdict when every judged sample has no
/// force-budget root below the flute: `Within`, at the deepest such sample,
/// against the flute length. Every such sample has its force at its own
/// depth at or under the budget (a sample deeper than the flute is judged
/// by that force in the loop), so the force reading agrees.
fn no_root_verdict(
    no_root: NoRootSample,
    flute_mm: f64,
    budget_n: f64,
    deepest_mm: f64,
    no_number_clause: &str,
    evidence_at: impl Fn(usize, ChiploadStatistic) -> SampleEvidence,
) -> DepthVerdict {
    let NoRootSample {
        idx,
        depth_mm: peak_mm,
        ae_mm,
        fz_mm,
    } = no_root;
    let bound = crate::machine::RigidityDepthCap::ForceBudget {
        budget_n,
        ae_mm,
        fz_mm,
        depth_mm: flute_mm,
        below_floor: false,
    };
    let evidence = evidence_at(idx, ChiploadStatistic::PeakInRange);
    let confidence = Confidence::Approximate(format!(
        "judged sample: engaged depth {peak_mm:.3} mm; {}: the peak cutting force at the \
         flute length {flute_mm:.3} mm, width {ae_mm:.2} mm and {fz_mm:.4} mm/tooth is at or \
         under the machine force budget {budget_n:.1} N, so the flute length is the bound; \
         deepest engaged depth {deepest_mm:.3} mm{no_number_clause}",
        crate::feeds::force_budget::NO_BUDGET_LIMIT_BELOW_THE_FLUTE
    ));
    DepthVerdict::Within {
        peak_mm,
        bound,
        evidence,
        confidence,
    }
}

/// The width and the chip of one sample: the cut the force reading reads.
#[derive(Clone, Copy)]
struct SampleCut {
    ae_mm: f64,
    fz_mm: f64,
}

/// P29-11 11-5 (F6-a): the cut of a sample, from the one engagement door of
/// the force reading and the deflection gate,
/// [`super::deflection::sample_engagement`] (the arc at the engaged depth,
/// the depth this gate judges), and the one operating point,
/// [`super::deflection::deflection_feed_per_tooth_mm`]. `None` when the
/// sample has no arc (or an arc of 0, ruling Q7): no measured width, no
/// number. The width share `radial_woc_fraction` is not read (P29-6 6c
/// deleted the full-slot fallback; 11-5 the width-share fallback).
fn sample_cut(
    s: &SimulationCutSample,
    tool: &crate::tool::ToolDefinition,
    predicted_feeds: &crate::machine::kinematics::PredictedFeedMap,
) -> Option<SampleCut> {
    let ae_mm = super::deflection::sample_engagement(tool, s)?.ae_mm();
    (ae_mm.is_finite() && ae_mm > 0.0).then(|| SampleCut {
        ae_mm,
        fz_mm: super::deflection::deflection_feed_per_tooth_mm(s, tool, predicted_feeds),
    })
}

/// X-VAC: with no sample that has a depth, the gate reads its bound off
/// `first_cut`, the width and chip of the first sample of the toolpath
/// that states a feed, a speed and a width, so a vacuous `Within` still
/// prints a cap. The population says the reading rests on nothing. With no
/// such sample there is no cut to set a depth at, and the gate reports.
fn vacuous_verdict(
    ctx: &super::ToolpathLoadContext<'_>,
    machine: &crate::machine::MachineProfile,
    facts: crate::limits::CutFacts,
    first_cut: Option<(f64, f64)>,
    evidence: impl Fn() -> SampleEvidence,
) -> DepthVerdict {
    let tool = ctx.tool();
    let Some((ae_mm, fz_mm)) = first_cut else {
        return DepthVerdict::Reported {
            peak_mm: 0.0,
            evidence: evidence(),
            confidence: Confidence::Approximate(
                "no sample states a feed and a width: the force budget has no cut to set a \
                 depth at"
                    .to_owned(),
            ),
        };
    };
    match crate::feeds::force_budget::rigidity_depth_cap(
        machine,
        facts,
        tool,
        ctx.material(),
        ae_mm,
        fz_mm,
    ) {
        Some(crate::feeds::force_budget::RigidityCapOutcome::Cap(bound))
            if bound.cap_mm().is_finite() && bound.cap_mm() > 0.0 =>
        {
            DepthVerdict::Within {
                peak_mm: 0.0,
                bound,
                evidence: evidence(),
                confidence: Confidence::Approximate(
                    "no sample removed material at a depth: the cap is read at the first \
                     sample that states a feed and a width"
                        .to_owned(),
                ),
            }
        }
        Some(crate::feeds::force_budget::RigidityCapOutcome::Cap(_)) => DepthVerdict::Unmodeled {
            reason: UnmodeledReason::NotImplemented(
                "the tool reports no usable diameter for the rigidity cap".to_owned(),
            ),
        },
        Some(crate::feeds::force_budget::RigidityCapOutcome::Unmodelled(reason)) => {
            DepthVerdict::Unmodeled {
                reason: UnmodeledReason::NotImplemented(reason.to_owned()),
            }
        }
        Some(crate::feeds::force_budget::RigidityCapOutcome::NoLimitBelowFlute) | None => {
            DepthVerdict::Reported {
                peak_mm: 0.0,
                evidence: evidence(),
                confidence: Confidence::Approximate(format!(
                    "peak engaged depth 0.000 mm; {}",
                    crate::feeds::force_budget::NO_BUDGET_LIMIT_BELOW_THE_FLUTE
                )),
            }
        }
    }
}

/// A sample this gate can read a depth from: it removes material
/// ([`SimulationCutSample::removes_material`], P29-6 ruling Q2), and it
/// reports a positive engaged depth. A removing sample at zero depth
/// contributes nothing to a maximum, and counting it would make an air
/// pass look like a measured population.
pub(super) fn is_depth_sample(sample: &SimulationCutSample) -> bool {
    sample.removes_material() && sample.axial_engagement_mm > 0.0
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::compute::catalog::OperationType;
    use crate::compute::tool_config::ToolMaterial;
    use crate::ids::ToolpathId;
    use crate::machine::MachineProfile;
    use crate::material::Material;
    use crate::stock::simulation_cut::{CutKinematics, Engagement, SimulationCutTrace};
    use crate::tool::{FlatEndmill, ToolDefinition};
    use crate::tool_load::{GateEnv, ToleranceBands, ToolpathLoadContext};

    const TP: ToolpathId = ToolpathId::from_raw(0);

    /// The facts of the body cut of a default op of `op` (P29-11 S10: the
    /// cap producer takes the facts of a cut).
    fn body_facts(op: OperationType) -> crate::limits::CutFacts {
        let config = crate::compute::catalog::OperationConfig::new_default(op);
        config
            .cuts(&crate::feeds::entry_rule::ConfiguredEntry::registry_default(&config))
            .body_facts()
    }

    fn tool() -> ToolDefinition {
        ToolDefinition::new(
            Box::new(FlatEndmill::new(6.0, 20.0)),
            6.0,
            30.0,
            2,
            ToolMaterial::Carbide,
        )
    }

    fn sample(idx: usize, depth: f64, cutting: bool) -> SimulationCutSample {
        SimulationCutSample {
            toolpath_id: TP,
            move_index: idx,
            sample_index: idx,
            is_cutting: cutting,
            cut_kinematics: CutKinematics::Linear,
            feed_rate_mm_min: 1500.0,
            spindle_rpm: 18_000,
            flute_count: 2,
            axial_doc_mm: 0.0,
            axial_engagement_mm: depth,
            engagement: Engagement::with_radial_woc(0.4),
            // P29-11 11-5: the gate reads the width of the arc, not the
            // share: this arc gives the width 0.4 D of the flat tool.
            arc_engagement_radians: Some(arc_of_share(0.4)),
            // P29-6 6c: the gate reads the samples that remove material.
            removed_volume_est_mm3: if cutting { 0.1 } else { 0.0 },
            in_transit_span: false,
            ..SimulationCutSample::test_fixture()
        }
    }

    /// The arc (rad) whose width `r (1 - cos psi)` is the share `share` of
    /// the diameter `2 r` of a flat tool: `cos psi = 1 - 2 share`.
    fn arc_of_share(share: f64) -> f64 {
        (1.0 - 2.0 * share).acos()
    }

    /// The width (mm) of the arc of [`arc_of_share`] on the 6 mm flat tool,
    /// by the gate's own door (`deflection::sample_engagement`).
    fn width_of_share(share: f64) -> f64 {
        3.0 * (1.0 - arc_of_share(share).cos())
    }

    fn trace(samples: Vec<SimulationCutSample>) -> SimulationCutTrace {
        SimulationCutTrace {
            samples,
            ..SimulationCutTrace::test_fixture()
        }
    }

    fn run(op: OperationType, t: Option<&SimulationCutTrace>) -> DepthVerdict {
        run_in(op, t, &Material::default())
    }

    /// The mechanics material. P29-4 (ruling R1): the Adaptive family reads
    /// the force budget. In generic softwood (`Material::default()`) the
    /// budget has no root below the flute at the fixture cut, so the gate
    /// gives `Within` at the flute length (P29-11 11-5); white oak has a
    /// root there.
    fn oak() -> Material {
        Material::SolidWood {
            species: crate::material::WoodSpecies::WhiteOak,
        }
    }

    /// The force-budget cap (mm) of the mechanics arms, from the one
    /// producer at the fixture cut: the width of the arc of the share 0.4
    /// of the 6 mm tool (`sample`) and the chip feed / (rpm x Z).
    fn oak_cap_mm() -> f64 {
        let cut = crate::feeds::force_budget::rigidity_depth_cap(
            &MachineProfile::shapeoko_makita(),
            body_facts(OperationType::Adaptive),
            &tool(),
            &oak(),
            width_of_share(0.4),
            1500.0 / (18_000.0 * 2.0),
        );
        match cut {
            Some(crate::feeds::force_budget::RigidityCapOutcome::Cap(c)) => c.cap_mm(),
            other => panic!("the oak fixture cut has a force-budget cap: {other:?}"),
        }
    }

    fn run_in(
        op: OperationType,
        t: Option<&SimulationCutTrace>,
        material: &Material,
    ) -> DepthVerdict {
        let tool = tool();
        // P29-4: no family reads a factor rule. The mechanics arms run the
        // Adaptive family on the force budget in oak (`oak_cap_mm`).
        let machine = MachineProfile::shapeoko_makita();
        let tolerance = ToleranceBands::default();
        let ctx = ToolpathLoadContext::of_operation(
            TP,
            &crate::compute::catalog::OperationConfig::new_default(op),
            &tool,
            material,
            crate::machine::SteadyFeedMmMin::op_feed_only(1500.0),
            None,
            crate::limits::CapTable::empty(),
            None,
        );
        let env = GateEnv {
            sim_trace: t,
            machine: Some(&machine),
            tolerance: &tolerance,
        };
        evaluate(&ctx, &env)
    }

    /// P13c (O1): a roughing Pocket reads the force budget at the judged
    /// sample's cut: the arc pi/2 gives ae = r = 3 mm, the chip is the
    /// sample feed / (rpm x Z). The bound is the one producer's number.
    #[test]
    fn the_pocket_reads_the_force_budget_p13c() {
        let mut s0 = sample(0, 0.8, true);
        s0.arc_engagement_radians = Some(std::f64::consts::FRAC_PI_2);
        let t = trace(vec![s0]);
        let machine = MachineProfile::shapeoko_makita();
        let want = crate::feeds::force_budget::rigidity_depth_cap(
            &machine,
            body_facts(OperationType::Pocket),
            &tool(),
            &Material::default(),
            3.0,
            1500.0 / (18_000.0 * 2.0),
        );
        match (run(OperationType::Pocket, Some(&t)), want) {
            (
                DepthVerdict::Within { bound, .. } | DepthVerdict::Exceeds { bound, .. },
                Some(crate::feeds::force_budget::RigidityCapOutcome::Cap(c)),
            ) => {
                // The sample arc gives r (1 - cos pi/2), equal to 3 mm to one ulp.
                assert!(
                    (bound.cap_mm() - c.cap_mm()).abs() < 1e-9,
                    "{bound:?} {c:?}"
                );
            }
            // P29-11 11-5 (F6-a): no root below the flute is `Within` at
            // the flute length.
            (
                DepthVerdict::Within { bound, .. },
                Some(crate::feeds::force_budget::RigidityCapOutcome::NoLimitBelowFlute),
            ) => {
                assert!(
                    (bound.cap_mm() - tool().length()).abs() < 1e-12,
                    "{bound:?}"
                );
            }
            other => panic!("the Pocket gate and the producer disagree: {other:?}"),
        }
    }

    /// The cap `c` is the oak force-budget depth (P29-4; before, the
    /// fixture factor 0.25 x 6 = 1.5 mm). The three samples share one width
    /// and one chip, so they share one cap: the judged sample (the largest
    /// depth over its own cap) is the deepest one, 0.8 c, under the cap.
    #[test]
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
    fn at_one_width_the_judged_sample_is_the_deepest() {
        let c = oak_cap_mm();
        let t = trace(vec![
            sample(0, 0.25 * c, true),
            sample(1, 0.8 * c, true),
            sample(2, 0.6 * c, true),
        ]);
        match run_in(OperationType::Adaptive, Some(&t), &oak()) {
            DepthVerdict::Within {
                peak_mm,
                bound,
                evidence,
                ..
            } => {
                assert!((peak_mm - 0.8 * c).abs() < 1e-12);
                assert!((bound.cap_mm() - c).abs() < 1e-12);
                assert_eq!(evidence.population.unwrap().contributing, 3);
            }
            other => panic!("expected Within, got {other:?}"),
        }
    }

    /// The oak force-budget cap (mm) at the width of the arc of the share
    /// `share` of the 6 mm tool and the fixture chip.
    fn oak_cap_at_share_mm(share: f64) -> f64 {
        let cut = crate::feeds::force_budget::rigidity_depth_cap(
            &MachineProfile::shapeoko_makita(),
            body_facts(OperationType::Adaptive),
            &tool(),
            &oak(),
            width_of_share(share),
            1500.0 / (18_000.0 * 2.0),
        );
        match cut {
            Some(crate::feeds::force_budget::RigidityCapOutcome::Cap(c)) => c.cap_mm(),
            other => panic!("the oak fixture cut has a force-budget cap: {other:?}"),
        }
    }

    /// P29-6 6c (ruling Q3): each sample is judged against its own cap. A
    /// deep narrow sample (0.8 of its cap) and a shallower wide sample (0.9
    /// of its smaller cap): the verdict names the shallower wide sample,
    /// with its depth and its cap, not the deepest sample.
    #[test]
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
    fn the_judged_sample_is_the_largest_depth_over_its_own_cap() {
        let narrow_cap = oak_cap_at_share_mm(0.4);
        let wide_cap = oak_cap_at_share_mm(1.0);
        let deep = 0.8 * narrow_cap;
        let wide_depth = 0.9 * wide_cap;
        assert!(
            wide_depth < deep,
            "vacuous: the wide sample must be the shallower one ({wide_depth} vs {deep})"
        );
        let wide = SimulationCutSample {
            engagement: Engagement::with_radial_woc(1.0),
            arc_engagement_radians: Some(arc_of_share(1.0)),
            ..sample(1, wide_depth, true)
        };
        let t = trace(vec![sample(0, deep, true), wide]);
        match run_in(OperationType::Adaptive, Some(&t), &oak()) {
            DepthVerdict::Within {
                peak_mm,
                bound,
                evidence,
                ..
            } => {
                assert!((peak_mm - wide_depth).abs() < 1e-12, "{peak_mm}");
                assert!((bound.cap_mm() - wide_cap).abs() < 1e-12);
                assert_eq!(evidence.sample_range.start, 1);
                assert_eq!(evidence.population.unwrap().contributing, 2);
            }
            other => panic!("expected Within at the wide sample, got {other:?}"),
        }
    }

    /// P29-6 6c: a sample with a depth and no width gets no number. Before
    /// 6c it took a full-slot cap. P29-11 11-5 (F6-a): the gate reads the
    /// arc only, so a sample with a width share (0.4) and no arc has no
    /// width either (before 11-5 the share gave it one). Here it is five
    /// caps deep, and it neither sets the bound nor counts.
    #[test]
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
    fn a_sample_with_no_width_gets_no_number() {
        let c = oak_cap_mm();
        let no_width = SimulationCutSample {
            arc_engagement_radians: None,
            ..sample(1, 5.0 * c, true)
        };
        let t = trace(vec![sample(0, 0.5 * c, true), no_width]);
        match run_in(OperationType::Adaptive, Some(&t), &oak()) {
            DepthVerdict::Within {
                peak_mm,
                bound,
                evidence,
                ..
            } => {
                assert!((peak_mm - 0.5 * c).abs() < 1e-12, "{peak_mm}");
                assert!((bound.cap_mm() - c).abs() < 1e-12);
                let p = evidence.population.unwrap();
                assert_eq!((p.contributing, p.offered), (1, 2));
            }
            other => panic!("expected Within at the sample with a width, got {other:?}"),
        }
    }

    /// P29-6 6c: when every sample with a depth has no width, no sample
    /// has a number. The gate reports the deepest depth with no bound, and
    /// the population is vacuous. P29-11 11-5: a width share with no arc is
    /// no width (the share 0.4 of `sample`, the arc removed).
    #[test]
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
    fn only_samples_with_no_width_report_with_no_bound() {
        let no_width = |idx: usize, depth: f64| SimulationCutSample {
            arc_engagement_radians: None,
            ..sample(idx, depth, true)
        };
        let t = trace(vec![no_width(0, 0.4), no_width(1, 0.9)]);
        match run_in(OperationType::Adaptive, Some(&t), &oak()) {
            DepthVerdict::Reported {
                peak_mm, evidence, ..
            } => {
                assert!((peak_mm - 0.9).abs() < 1e-12, "{peak_mm}");
                let p = evidence.population.unwrap();
                assert!(p.is_vacuous());
                assert_eq!(p.offered, 2);
            }
            other => panic!("expected Reported with no bound, got {other:?}"),
        }
    }

    #[test]
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
    fn a_depth_past_the_cap_exceeds() {
        let c = oak_cap_mm();
        let t = trace(vec![sample(0, c / 3.0, true), sample(1, 1.6 * c, true)]);
        match run_in(OperationType::Adaptive, Some(&t), &oak()) {
            DepthVerdict::Exceeds { peak_mm, bound, .. } => {
                assert!(peak_mm > bound.cap_mm());
            }
            other => panic!("expected Exceeds, got {other:?}"),
        }
    }

    /// A cut that lands exactly on the cap is `Within`: the boundary
    /// contract is inclusive on every gate in this folder.
    #[test]
    fn a_cut_exactly_on_the_cap_is_within() {
        let t = trace(vec![sample(0, oak_cap_mm(), true)]);
        assert!(matches!(
            run_in(OperationType::Adaptive, Some(&t), &oak()),
            DepthVerdict::Within { .. }
        ));
    }

    /// X-VAC: a trace with no cutting sample gives a vacuous population.
    /// P29-11 11-5 (F6-a): the X-VAC cut reads the gate's one door
    /// (`deflection::sample_engagement`), and a sample that does not cut
    /// has no engagement, so no sample states a width: the gate reports
    /// with no cap (before 11-5 the width share of the rapid gave a vacuous
    /// `Within` with a cap).
    #[test]
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
    fn a_trace_with_no_cutting_samples_is_vacuous() {
        let t = trace(vec![sample(0, 0.0, false)]);
        match run_in(OperationType::Adaptive, Some(&t), &oak()) {
            DepthVerdict::Reported {
                peak_mm, evidence, ..
            } => {
                assert_eq!(peak_mm, 0.0);
                let p = evidence.population.unwrap();
                assert!(p.is_vacuous());
                assert_eq!(p.offered, 1);
            }
            other => panic!("expected a vacuous Reported, got {other:?}"),
        }
    }

    /// X-VAC: a group with no sample at a depth reads its cap at the first
    /// cutting sample of the toolpath with an arc (the gate's door): here a
    /// link sample with a depth in the Link group, so the Body group is
    /// vacuous and still prints a cap.
    #[test]
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
    fn a_vacuous_group_reads_its_cap_at_the_first_cutting_sample() {
        let t = trace(vec![
            sample(0, 0.0, false),
            SimulationCutSample {
                in_transit_span: true,
                source_intent: Some(crate::toolpath::MoveIntent::Linking),
                ..sample(1, 0.5, true)
            },
        ]);
        match run_in(OperationType::Adaptive, Some(&t), &oak()) {
            DepthVerdict::Within {
                peak_mm,
                bound,
                evidence,
                ..
            } => {
                assert_eq!(peak_mm, 0.0);
                assert!((bound.cap_mm() - oak_cap_mm()).abs() < 1e-12, "{bound:?}");
                assert!(evidence.population.unwrap().is_vacuous());
            }
            other => panic!("expected a vacuous Within with a cap, got {other:?}"),
        }
    }

    #[test]
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
    fn a_link_sample_never_sets_the_headline_peak() {
        // P29-6 6d: a link sample is judged in the Link group, so the deep
        // reading a bridge carries never becomes the body cut's depth.
        let t = trace(vec![
            sample(0, 0.8, true),
            SimulationCutSample {
                in_transit_span: true,
                source_intent: Some(crate::toolpath::MoveIntent::Linking),
                ..sample(1, 9.0, true)
            },
        ]);
        match run_in(OperationType::Adaptive, Some(&t), &oak()) {
            DepthVerdict::Within {
                peak_mm, evidence, ..
            } => {
                assert!((peak_mm - 0.8).abs() < 1e-12);
                let p = evidence.population.unwrap();
                assert_eq!((p.contributing, p.offered), (1, 2));
            }
            other => panic!("expected Within at 0.8 mm, got {other:?}"),
        }
    }

    #[test]
    fn no_trace_asks_for_a_simulation() {
        assert!(matches!(
            run(OperationType::Adaptive, None),
            DepthVerdict::Unmodeled {
                reason: UnmodeledReason::SimulationRequired
            }
        ));
    }

    #[test]
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
    fn a_drill_cycle_does_not_apply() {
        let t = trace(vec![sample(0, 1.0, true)]);
        match run(OperationType::Drill, Some(&t)) {
            DepthVerdict::Unmodeled {
                reason: UnmodeledReason::NotApplicableForOp(clause),
            } => assert_eq!(clause, "drill cycle — no continuous engagement"),
            other => panic!("expected NotApplicableForOp, got {other:?}"),
        }
    }

    #[test]
    fn no_machine_profile_is_not_implemented() {
        let tool = tool();
        let material = Material::default();
        let tolerance = ToleranceBands::default();
        let t = trace(vec![sample(0, 1.0, true)]);
        let ctx = ToolpathLoadContext::of_operation(
            TP,
            &crate::compute::catalog::OperationConfig::new_default(OperationType::Pocket),
            &tool,
            &material,
            crate::machine::SteadyFeedMmMin::op_feed_only(1500.0),
            None,
            crate::limits::CapTable::empty(),
            None,
        );
        let env = GateEnv {
            sim_trace: Some(&t),
            machine: None,
            tolerance: &tolerance,
        };
        assert!(matches!(
            evaluate(&ctx, &env),
            DepthVerdict::Unmodeled {
                reason: UnmodeledReason::NotImplemented(_)
            }
        ));
    }

    /// Feeds matrix R2 (2026-09-23): a one-sided finishing operation has
    /// no axial cap. Before R2 a 1.2 mm Trace cut read `Exceeds` against
    /// `doc_finishing_factor` (0.10 × Ø6 = 0.6 mm). P29-5 (ruling R3): a
    /// Trace is a slot and reads the force budget, so the fixture is a
    /// Scallop finish (material on one side). The gate reports the peak
    /// with no bound, and the state is `Within`.
    #[test]
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
    fn a_finishing_pass_reports_its_depth_with_no_bound() {
        let t = trace(vec![sample(0, 0.4, true), sample(1, 1.2, true)]);
        let v = run(OperationType::Scallop, Some(&t));
        match &v {
            DepthVerdict::Reported {
                peak_mm, evidence, ..
            } => {
                assert!((peak_mm - 1.2).abs() < 1e-12);
                assert_eq!(evidence.population.unwrap().contributing, 2);
            }
            other => panic!("expected Reported, got {other:?}"),
        }
        let status = v.as_criterion_status();
        assert_eq!(status.state, crate::tool_load::verdict::LoadState::Within);
        assert_eq!(status.bound, None);
        assert_eq!(status.bound_source, None);
        assert_eq!(status.display_peak, Some(1.2));
    }

    /// P29-11 11-5 (F6-a): in generic softwood the fixture cut has no
    /// force-budget root below the flute, on every sample. The force at the
    /// flute length is at or under the budget, so the gate gives `Within`
    /// with the flute length as the bound (before 11-5: `Reported`), at the
    /// deepest judged sample, with every judged sample counted.
    #[test]
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "test: an unexpected variant panics"
    )]
    fn no_root_below_the_flute_is_within_at_the_flute_length() {
        let fz = 1500.0 / (18_000.0 * 2.0);
        assert!(
            matches!(
                crate::feeds::force_budget::rigidity_depth_cap(
                    &MachineProfile::shapeoko_makita(),
                    body_facts(OperationType::Adaptive),
                    &tool(),
                    &Material::default(),
                    width_of_share(0.4),
                    fz,
                ),
                Some(crate::feeds::force_budget::RigidityCapOutcome::NoLimitBelowFlute)
            ),
            "non-vacuity: the fixture cut has no root below the flute"
        );
        let t = trace(vec![sample(0, 0.4, true), sample(1, 1.2, true)]);
        match run(OperationType::Adaptive, Some(&t)) {
            DepthVerdict::Within {
                peak_mm,
                bound,
                evidence,
                ..
            } => {
                assert!((peak_mm - 1.2).abs() < 1e-12, "{peak_mm}");
                assert!(
                    (bound.cap_mm() - tool().length()).abs() < 1e-12,
                    "{bound:?}"
                );
                assert_eq!(evidence.sample_range.start, 1);
                assert_eq!(evidence.population.unwrap().contributing, 2);
            }
            other => panic!("expected Within at the flute length, got {other:?}"),
        }
    }

    /// P29-11 11-5 (F6-a): a sample deeper than the flute length is judged
    /// by its own force at its own depth (the producer solves only up to
    /// the flute). In softwood the fixture cut has no root below the flute;
    /// at 1.5 flute lengths at the fixture feed the force reading is over
    /// the budget and the row exceeds against the flute length; at a fifth
    /// of that feed it is under the budget and the row holds. Both sides
    /// agree with the force of the reading (`cut_force::sample_peak_force_n`).
    #[test]
    fn a_sample_past_the_flute_is_judged_by_its_own_force() {
        let flute = tool().length();
        let budget = MachineProfile::shapeoko_makita().rigidity.force_budget_n;
        let mut fast_over = 0;
        for feed in [1500.0, 300.0] {
            let s = SimulationCutSample {
                feed_rate_mm_min: feed,
                ..sample(0, 1.5 * flute, true)
            };
            let fz = feed / (18_000.0 * 2.0);
            let force =
                super::super::cut_force::sample_peak_force_n(&tool(), &Material::default(), &s, fz)
                    .unwrap();
            let v = run(OperationType::Adaptive, Some(&trace(vec![s])));
            assert_eq!(
                v.is_exceeded(),
                force > budget,
                "feed {feed}: force {force} N, budget {budget} N, {v:?}"
            );
            if force > budget {
                fast_over += 1;
            }
            match v {
                DepthVerdict::Within { bound, .. } | DepthVerdict::Exceeds { bound, .. } => {
                    assert!((bound.cap_mm() - flute).abs() < 1e-12, "{bound:?}");
                }
                other @ (DepthVerdict::Reported { .. } | DepthVerdict::Unmodeled { .. }) => {
                    panic!("feed {feed}: a bound at the flute length, got {other:?}")
                }
            }
        }
        assert_eq!(
            fast_over, 1,
            "non-vacuity: one side over the budget, one under"
        );
    }

    /// EVIDENCE 5.1-6: a Profile cut at exactly the cap read 7.6e-7 mm
    /// above it, because the dexel stock stores lengths in `f32`. That
    /// noise is inside [`DEXEL_DEPTH_RESOLUTION_MM`], so it is `Within`.
    #[test]
    fn f32_noise_at_the_cap_is_within() {
        let t = trace(vec![sample(0, oak_cap_mm() + 7.6e-7, true)]);
        assert!(matches!(
            run_in(OperationType::Adaptive, Some(&t), &oak()),
            DepthVerdict::Within { .. }
        ));
    }

    /// A reading one tenth of a micrometre past the dexel resolution is
    /// a real exceedance.
    #[test]
    fn a_depth_past_the_dexel_resolution_exceeds() {
        let t = trace(vec![sample(
            0,
            oak_cap_mm() + 2.0 * DEXEL_DEPTH_RESOLUTION_MM,
            true,
        )]);
        assert!(matches!(
            run_in(OperationType::Adaptive, Some(&t), &oak()),
            DepthVerdict::Exceeds { .. }
        ));
    }
}
