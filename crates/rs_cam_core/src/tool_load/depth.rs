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
//! The cap is [`RigidityProfile::depth_cap_mm`]: the profile's own
//! factor for the operation family, times the diameter that
//! `feeds::geometry::depth_cap_diameter_mm` gives at the measured peak
//! (the engaged cone diameter on a tapered ball, the nominal diameter on
//! every other shape). It is a
//! RULE OF THUMB with no published source, so it carries
//! [`BoundSource::RigidityRuleOfThumb`], whose `gates_export()` is
//! false. An exceedance is reported; the export goes ahead. The day the
//! machine is measured, the variant is replaced by a measured one and
//! the row gates with no change to any renderer (`PLAN.md` §11).
//!
//! **This gate adds no number.** The two helpers it calls are the ones
//! the Suggest clamp calls, so the cap the recipe was lowered to and the
//! cap this row draws against cannot disagree.
//!
//! ## A finishing pass has no bound (feeds matrix R2, 2026-09-23)
//!
//! No vendor publishes an axial finishing cap as a fraction of D for
//! wood. For a `Finish` or `SemiFinish` pass the profile gives no cap,
//! and this gate returns [`DepthVerdict::Reported`]: the peak and its
//! population, with no bound. The deflection gate is the limit on that
//! pass.
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
//! the same contract `power.rs` and `deflection.rs` carry.

use crate::stock::simulation_cut::SimulationCutSample;

use super::locality::SpanLookup;
use super::verdict::{
    ChiploadStatistic, Confidence, DepthVerdict, GatePopulation, PopulationUnit, SampleEvidence,
    UnmodeledReason,
};

/// The one operator-facing clause for "this gate has no meaning on a
/// plunge-only cycle". Identical to the three milling gates', so a
/// drill toolpath's rows read the same way across the whole tier.
const DRILL_NOT_APPLICABLE: &str = "drill cycle — no continuous engagement";

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

#[tracing::instrument(level = "debug", skip_all, fields(toolpath_id = ctx.toolpath_id.0, op = ?ctx.operation_kind))]
pub fn evaluate(ctx: &super::ToolpathLoadContext<'_>, env: &super::GateEnv<'_>) -> DepthVerdict {
    let &super::ToolpathLoadContext {
        toolpath_id,
        tool,
        operation_kind,
        spans,
        ..
    } = ctx;
    let &super::GateEnv {
        sim_trace,
        machine,
        tolerance,
        ..
    } = env;
    let _ = tolerance;

    // Same short-circuit as chipload / power / deflection. A drill
    // cycle is Z-only: there is no lateral engagement to measure and no
    // axial rule of thumb to measure it against.
    if operation_kind.is_drill_kinematics() {
        tracing::debug!(
            reason = "NotApplicableForOp",
            "depth gate refuses: plunge-only op has no continuous engagement"
        );
        return DepthVerdict::Unmodeled {
            reason: UnmodeledReason::NotApplicableForOp(DRILL_NOT_APPLICABLE.to_owned()),
        };
    }
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

    // X-VAC — the same population contract `power.rs` carries. With
    // `peak_mm == 0.0` and an empty population the verdict below is a
    // `Within` at zero depth, which is the most reassuring thing this
    // gate can print, so the population has to say it rests on nothing.
    //
    // Feeds matrix R2 (2026-09-23): the walk comes BEFORE the bound,
    // because the bound's diameter is read at the measured peak.
    let span_lookup = spans.map(SpanLookup::new);
    let mut offered: usize = 0;
    let mut contributing: usize = 0;
    let mut peak_mm = 0.0_f64;
    let mut peak_idx: Option<usize> = None;

    for (i, s) in trace.samples.iter().enumerate() {
        if s.toolpath_id != toolpath_id {
            continue;
        }
        offered += 1;
        if !is_depth_sample(s) {
            continue;
        }
        // The one steady-state predicate every gate in this folder
        // shares. It matters more here than anywhere else: a phantom
        // transit sample's dexel reads `stock_top − cutter_z` over
        // NEIGHBOURING uncleared stock, so its `axial_engagement_mm` is
        // exactly the inflated reading this gate would otherwise report
        // as the cut's depth.
        if !super::locality::is_steady_state_for_gate(s, span_lookup.as_ref()) {
            continue;
        }
        contributing += 1;
        if s.axial_engagement_mm > peak_mm {
            peak_mm = s.axial_engagement_mm;
            peak_idx = Some(i);
        }
    }

    let population = GatePopulation::new(contributing, offered, PopulationUnit::Samples);
    let evidence_with =
        |statistic: ChiploadStatistic| match peak_idx {
            Some(idx) => SampleEvidence::at_with_stat(idx, statistic)
                .with_locality(trace.samples.get(idx).and_then(|s| {
                    super::locality::classify_sample_locality(s, span_lookup.as_ref())
                }))
                .with_population(population),
            None => SampleEvidence::empty().with_population(population),
        };

    // The family and the pass role the Suggest clamp keys on, read off
    // the operation's own registry spec — the same pair, from the same
    // place, so the two readers cannot classify one operation
    // differently. The diameter is `depth_cap_diameter_mm` at the
    // measured peak: the function the Suggest clamp calls at the depth
    // it ships. On a tapered ball it is the engaged cone diameter at the
    // peak, capped at the shank; on every other shape it is the nominal
    // diameter.
    let spec = operation_kind.spec();
    let cap_diameter_mm = crate::feeds::geometry::depth_cap_diameter_mm(tool, peak_mm);
    let bound =
        machine
            .rigidity
            .depth_cap_mm(spec.feeds_family, spec.feeds_pass_role, cap_diameter_mm);
    let Some(bound) = bound else {
        // Feeds matrix R2 (2026-09-23): a finishing or semi-finishing
        // pass has no axial cap. The drill family also has none, but the
        // drill arm above already took it. The gate reports the peak and
        // judges nothing; the deflection gate is the limit on this pass.
        tracing::debug!(
            verdict = "Reported",
            peak_mm,
            "depth gate reports: this pass role carries no axial cap"
        );
        return DepthVerdict::Reported {
            peak_mm,
            evidence: evidence_with(ChiploadStatistic::PeakInRange),
            confidence: Confidence::Approximate(format!(
                "peak engaged depth {peak_mm:.3} mm; a finishing pass has no \
                 axial cap, and the deflection gate is its limit"
            )),
        };
    };
    let cap_mm = bound.cap_mm();
    if !cap_mm.is_finite() || cap_mm <= 0.0 {
        tracing::debug!(
            reason = "NotImplemented",
            diameter = cap_diameter_mm,
            factor = bound.factor,
            "depth gate refuses: the rigidity cap is not a usable depth"
        );
        return DepthVerdict::Unmodeled {
            reason: UnmodeledReason::NotImplemented(
                "the tool reports no usable diameter for the rigidity cap".to_owned(),
            ),
        };
    }

    // EVIDENCE 5.1-6 / 5.2-16: the dexel stock stores its segment ends
    // in `f32`, so a removed column is an `f32` difference. A Profile
    // cut at exactly the cap read 7.6e-7 mm above it and printed
    // `Exceeds` at 1.2000 against 1.2000. The tolerance is the dexel
    // resolution, as a fraction of the cap, in the form the other gates
    // pass their own tolerance.
    let exceeds =
        super::boundary::exceeds_high(peak_mm, cap_mm, DEXEL_DEPTH_RESOLUTION_MM / cap_mm);
    // The statistic is the chipload gate's own vocabulary, and it is
    // the right one: a peak read against a high bound, `PeakHigh` when
    // it trips and `PeakInRange` when it does not. One deep excursion is
    // the finding, as it is for deflection — a median would hide it.
    let statistic = if exceeds {
        ChiploadStatistic::PeakHigh
    } else {
        ChiploadStatistic::PeakInRange
    };
    let evidence = evidence_with(statistic);
    // Every figure here is formatted from the value it describes. The
    // confidence is `Approximate` on both decided arms because the
    // bound is a rule of thumb, and that is a property of the bound,
    // not of this cut.
    let confidence = Confidence::Approximate(format!(
        "peak engaged depth {peak_mm:.3} mm against the machine rigidity \
         factor {:.2} times the engaged diameter {:.2} mm at that depth",
        bound.factor, bound.diameter_mm
    ));

    if exceeds {
        tracing::warn!(
            verdict = "Exceeds",
            peak_mm,
            cap_mm,
            factor = bound.factor,
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

/// A sample this gate can read a depth from: it is cutting, and it
/// reports a positive engaged depth. A cutting sample at zero depth
/// contributes nothing to a maximum, and counting it would make an
/// air pass look like a measured population.
pub(super) fn is_depth_sample(sample: &SimulationCutSample) -> bool {
    sample.is_cutting && sample.axial_engagement_mm > 0.0
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
    use crate::compute::catalog::OperationType;
    use crate::compute::tool_config::ToolMaterial;
    use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
    use crate::ids::ToolpathId;
    use crate::machine::MachineProfile;
    use crate::material::Material;
    use crate::stock::simulation_cut::{CutKinematics, Engagement, SimulationCutTrace};
    use crate::tool::{FlatEndmill, ToolDefinition};
    use crate::tool_load::{GateEnv, ToleranceBands, ToolpathLoadContext};

    const TP: ToolpathId = ToolpathId(0);

    fn tool() -> ToolDefinition {
        ToolDefinition::new(
            Box::new(FlatEndmill::new(6.0, 20.0)),
            6.0,
            30.0,
            20.0,
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
            in_transit_span: false,
            ..SimulationCutSample::test_fixture()
        }
    }

    fn trace(samples: Vec<SimulationCutSample>) -> SimulationCutTrace {
        SimulationCutTrace {
            samples,
            ..SimulationCutTrace::test_fixture()
        }
    }

    fn run(op: OperationType, t: Option<&SimulationCutTrace>) -> DepthVerdict {
        let tool = tool();
        let material = Material::default();
        let machine = MachineProfile::shapeoko_makita();
        let tolerance = ToleranceBands::default();
        let ctx = ToolpathLoadContext {
            toolpath_id: TP,
            tool: &tool,
            material: &material,
            operation_family: LutOperationFamily::Pocket,
            pass_role: LutPassRole::Roughing,
            operation_feed_rate_mm_min: 1500.0,
            operation_kind: op,
            spans: None,
            drill_op: None,
        };
        let env = GateEnv {
            sim_trace: t,
            machine: Some(&machine),
            tolerance: &tolerance,
        };
        evaluate(&ctx, &env)
    }

    /// `doc_roughing_factor` 0.25 × Ø6 = 1.5 mm. A 1.2 mm peak is under
    /// it, and the peak is the MAXIMUM of the three depths.
    #[test]
    fn the_peak_is_the_maximum_engaged_depth() {
        let t = trace(vec![
            sample(0, 0.4, true),
            sample(1, 1.2, true),
            sample(2, 0.9, true),
        ]);
        match run(OperationType::Pocket, Some(&t)) {
            DepthVerdict::Within {
                peak_mm,
                bound,
                evidence,
                ..
            } => {
                assert!((peak_mm - 1.2).abs() < 1e-12);
                assert!((bound.cap_mm() - 1.5).abs() < 1e-12);
                assert_eq!(evidence.population.unwrap().contributing, 3);
            }
            other => panic!("expected Within, got {other:?}"),
        }
    }

    #[test]
    fn a_depth_past_the_cap_exceeds() {
        let t = trace(vec![sample(0, 0.5, true), sample(1, 2.4, true)]);
        match run(OperationType::Pocket, Some(&t)) {
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
        let t = trace(vec![sample(0, 1.5, true)]);
        assert!(matches!(
            run(OperationType::Pocket, Some(&t)),
            DepthVerdict::Within { .. }
        ));
    }

    #[test]
    fn a_trace_with_no_cutting_samples_is_vacuous() {
        let t = trace(vec![sample(0, 0.0, false)]);
        match run(OperationType::Pocket, Some(&t)) {
            DepthVerdict::Within {
                peak_mm, evidence, ..
            } => {
                assert_eq!(peak_mm, 0.0);
                let p = evidence.population.unwrap();
                assert!(p.is_vacuous());
                assert_eq!(p.offered, 1);
            }
            other => panic!("expected a vacuous Within, got {other:?}"),
        }
    }

    #[test]
    fn a_transit_sample_never_sets_the_peak() {
        // With no spans, `is_phantom_transit` falls back to the
        // conservative `in_transit_span` flag, and the inflated dexel
        // reading a bridge carries must not become the cut's depth.
        let t = trace(vec![
            sample(0, 0.8, true),
            SimulationCutSample {
                in_transit_span: true,
                ..sample(1, 9.0, true)
            },
        ]);
        match run(OperationType::Pocket, Some(&t)) {
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
            run(OperationType::Pocket, None),
            DepthVerdict::Unmodeled {
                reason: UnmodeledReason::SimulationRequired
            }
        ));
    }

    #[test]
    fn a_drill_cycle_does_not_apply() {
        let t = trace(vec![sample(0, 1.0, true)]);
        match run(OperationType::Drill, Some(&t)) {
            DepthVerdict::Unmodeled {
                reason: UnmodeledReason::NotApplicableForOp(clause),
            } => assert_eq!(clause, DRILL_NOT_APPLICABLE),
            other => panic!("expected NotApplicableForOp, got {other:?}"),
        }
    }

    #[test]
    fn no_machine_profile_is_not_implemented() {
        let tool = tool();
        let material = Material::default();
        let tolerance = ToleranceBands::default();
        let t = trace(vec![sample(0, 1.0, true)]);
        let ctx = ToolpathLoadContext {
            toolpath_id: TP,
            tool: &tool,
            material: &material,
            operation_family: LutOperationFamily::Pocket,
            pass_role: LutPassRole::Roughing,
            operation_feed_rate_mm_min: 1500.0,
            operation_kind: OperationType::Pocket,
            spans: None,
            drill_op: None,
        };
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

    /// Feeds matrix R2 (2026-09-23): a finishing operation has no axial
    /// cap. Before R2 this 1.2 mm Trace cut read `Exceeds` against
    /// `doc_finishing_factor` (0.10 × Ø6 = 0.6 mm). Now the gate reports
    /// the peak with no bound, and the state is `Within`.
    #[test]
    fn a_finishing_pass_reports_its_depth_with_no_bound() {
        let t = trace(vec![sample(0, 0.4, true), sample(1, 1.2, true)]);
        let v = run(OperationType::Trace, Some(&t));
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

    /// EVIDENCE 5.1-6: a Profile cut at exactly the cap read 7.6e-7 mm
    /// above it, because the dexel stock stores lengths in `f32`. That
    /// noise is inside [`DEXEL_DEPTH_RESOLUTION_MM`], so it is `Within`.
    #[test]
    fn f32_noise_at_the_cap_is_within() {
        let t = trace(vec![sample(0, 1.5 + 7.6e-7, true)]);
        assert!(matches!(
            run(OperationType::Pocket, Some(&t)),
            DepthVerdict::Within { .. }
        ));
    }

    /// A reading one tenth of a micrometre past the dexel resolution is
    /// a real exceedance.
    #[test]
    fn a_depth_past_the_dexel_resolution_exceeds() {
        let t = trace(vec![sample(0, 1.5 + 2.0 * DEXEL_DEPTH_RESOLUTION_MM, true)]);
        assert!(matches!(
            run(OperationType::Pocket, Some(&t)),
            DepthVerdict::Exceeds { .. }
        ));
    }
}
