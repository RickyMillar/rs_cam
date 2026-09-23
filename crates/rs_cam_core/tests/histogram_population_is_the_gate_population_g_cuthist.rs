//! **G-CUTHIST — the cut-metric histogram bins the gate's own population.**
//!
//! Pre-registration: `planning/sim_cut_metrics_2026-09-23/PLAN.md` §4 B and
//! §5. If the histogram and the gate read different samples, the card shows
//! mass outside the band while the badge says `Within`.
//!
//! # The claim
//!
//! For each banded criterion (chipload, power, deflection, depth of cut), on
//! one small synthetic trace:
//!
//! 1. the maximum of `distribution::gate_population` equals the gate's
//!    `display_peak`;
//! 2. the population size equals the gate's `population.contributing`, and
//!    the histogram counts sum to the same number;
//! 3. the histogram weights sum to `total_s`, `total_s` is positive, and
//!    `below_s + above_s + in_band_s == total_s`. The test computes
//!    `below_s` and `above_s` again from the fixture and the gate's bounds.
//!
//! # The guarded defect, and which assertion goes red
//!
//! The fixture holds five decoy samples that the gates must NOT count:
//!
//! - a transit sample (`in_transit_span`, no spans, so phantom) with the
//!   highest value in the trace. A population that skips
//!   `locality::is_steady_state_for_gate` holds it: claim 1 goes red on
//!   every criterion (its value is the new maximum), and claim 2 goes red.
//! - an air-cut sample (`radial_woc_fraction` 0.01). A population that skips
//!   the `< 0.02` air rule holds it: claim 2 goes red for chipload, power and
//!   deflection. The depth gate has no air rule, so it counts this sample,
//!   and its population must count it too.
//! - a sample at 80 % of the commanded feed. A chipload population that
//!   skips `steady_state_samples_for_toolpath` holds it: claim 2 goes red
//!   for chipload.
//! - a sample with no `effective_chip_thickness_mm`. A chipload population
//!   that skips the gate's vestigial chip-model predicate holds it: claim 2
//!   goes red for chipload.
//! - a sample of a different toolpath: claim 2 goes red if the toolpath
//!   filter goes.
//!
//! A population that resolves the feed differently from the gate (for
//! example the commanded feed, not the predicted feed) reads a different
//! value on every sample: claim 1 goes red, because the fixture steers each
//! sample's value only through the F-035 predicted-feed map.
//!
//! Fast: no simulation, one hand-built trace of about twenty samples.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::machine::kinematics::PredictedFeedMap;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::stock::simulation_cut::{
    CutKinematics, Engagement, SimulationCutSample, SimulationCutSummary, SimulationCutTrace,
};
use rs_cam_core::tool::{FlatEndmill, ToolDefinition};
use rs_cam_core::tool_load::distribution::{
    DistributionMetric, DistributionOutcome, gate_population, metric_distribution,
};
use rs_cam_core::tool_load::verdict::{
    BoundSource, CriterionKind, CriterionStatus, LoadState, ToolpathLoadVerdict,
};
use rs_cam_core::tool_load::{
    GateEnv, ToleranceBands, ToolpathLoadContext, boundary, evaluate_toolpath,
};

const TP: ToolpathId = ToolpathId(0);
const OTHER_TP: ToolpathId = ToolpathId(1);
const RPM: u32 = 18_000;
const FLUTES: u32 = 2;
const OP_FEED: f64 = 1000.0;
const SEGMENT_S: f64 = 0.1;

fn tool() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(FlatEndmill::new(6.35, 20.0)),
        6.35,
        30.0,
        20.0,
        30.0,
        FLUTES,
        ToolMaterial::Carbide,
    )
}

fn material() -> Material {
    Material::SolidWood {
        species: WoodSpecies::HardMaple,
    }
}

/// One cutting sample. `advance` is the advance per tooth the gate is to
/// observe; the trace realises it through the predicted-feed map, so the
/// commanded feed (which steers the steady-state filter) stays separate.
/// `depth` and `arc` vary so the power, deflection and depth populations
/// have distinct values and a distinct maximum.
fn sample(idx: usize, advance: f64, depth: f64, arc: f64) -> (SimulationCutSample, f64) {
    let s = SimulationCutSample {
        toolpath_id: TP,
        move_index: idx,
        sample_index: idx,
        position: [0.0, 0.0, -depth],
        cumulative_time_s: SEGMENT_S * idx as f64,
        segment_time_s: SEGMENT_S,
        is_cutting: true,
        cut_kinematics: CutKinematics::Linear,
        feed_rate_mm_min: OP_FEED,
        spindle_rpm: RPM,
        flute_count: FLUTES,
        axial_doc_mm: depth,
        axial_engagement_mm: depth,
        arc_engagement_radians: Some(arc),
        chipload_mm_per_tooth: OP_FEED / (f64::from(RPM) * f64::from(FLUTES)),
        effective_chip_thickness_mm: Some(advance),
        engagement: Engagement::with_radial_woc(0.4),
        removed_volume_est_mm3: 0.1,
        mrr_mm3_s: 1.0,
        ..SimulationCutSample::test_fixture()
    };
    (s, advance)
}

/// The fixture: twelve counted samples, one far below any chip floor, and
/// the five decoys named in the module header.
///
/// Trace indices: 0..=12 are counted by every gate. 13 (transit) and 17
/// (other toolpath) are counted by no gate. 14 (air) is counted by the depth
/// gate only, which has no air rule. 15 (80 % feed) and 16 (no chip model)
/// are decoys for chipload only: the power, deflection and depth gates count
/// them, because those gates have no feed filter and no chip-model
/// predicate.
fn fixture() -> SimulationCutTrace {
    let mut rows: Vec<(SimulationCutSample, f64)> = Vec::new();
    // Counted: advance 0.028..0.0394 mm/tooth (the chipload unit tests
    // show 0.040 is inside this row's band), depth 0.6..1.7 mm, arc
    // 0.8..1.35 rad.
    for k in 0..12 {
        let kf = k as f64;
        rows.push(sample(
            k,
            0.028 + 0.001 * kf + 0.0002 * (kf % 3.0),
            0.6 + 0.1 * kf,
            0.8 + 0.05 * kf,
        ));
    }
    // Counted: one sample far below any vendor floor. The median stays in
    // the band, so the gate stays `Within`; `below_s` gets one segment.
    rows.push(sample(12, 0.004, 0.5, 0.7));
    // Decoy: phantom transit, the highest value of every quantity.
    let (mut s, a) = sample(13, 0.058, 2.8, 2.0);
    s.in_transit_span = true;
    rows.push((s, a));
    // Decoy: air cut.
    let (mut s, a) = sample(14, 0.050, 2.5, 1.9);
    s.engagement = Engagement::with_radial_woc(0.01);
    rows.push((s, a));
    // Decoy: 80 % of the commanded feed (chipload only).
    let (mut s, a) = sample(15, 0.049, 1.0, 1.0);
    s.feed_rate_mm_min = 0.8 * OP_FEED;
    rows.push((s, a));
    // Decoy: no chip model (chipload only).
    let (mut s, a) = sample(16, 0.048, 1.0, 1.0);
    s.effective_chip_thickness_mm = None;
    rows.push((s, a));
    // Decoy: another toolpath.
    let (mut s, a) = sample(17, 0.057, 2.7, 2.0);
    s.toolpath_id = OTHER_TP;
    rows.push((s, a));

    let mut predicted_feeds = PredictedFeedMap::new();
    for (s, advance) in &rows {
        let divisor = f64::from(s.spindle_rpm) * f64::from(s.flute_count);
        predicted_feeds.insert((s.toolpath_id, s.move_index), advance * divisor);
    }
    let samples: Vec<SimulationCutSample> = rows.into_iter().map(|(s, _)| s).collect();
    SimulationCutTrace {
        predicted_feeds,
        sample_step_mm: 1.0,
        summary: SimulationCutSummary {
            sample_count: samples.len(),
            toolpath_count: 2,
            issue_count: 0,
            hotspot_count: 0,
            total_runtime_s: 2.0,
            cutting_runtime_s: 2.0,
            rapid_runtime_s: 0.0,
            air_cut_time_s: 0.0,
            low_engagement_time_s: 0.0,
            average_engagement: 0.4,
            peak_chipload_mm_per_tooth: 0.058,
            peak_axial_doc_mm: 2.8,
            peak_plunge_descent_mm: 0.0,
            total_removed_volume_est_mm3: 1.0,
            average_mrr_mm3_s: 1.0,
            per_kinematics: std::collections::BTreeMap::new(),
            runtime_by_intent: None,
        },
        samples,
        ..SimulationCutTrace::test_fixture()
    }
}

fn status_for(kind: CriterionKind, v: &ToolpathLoadVerdict) -> CriterionStatus<'_> {
    match kind {
        CriterionKind::Chipload => v.chipload.as_criterion_status(),
        CriterionKind::Power => v.power.as_criterion_status(),
        CriterionKind::Deflection => v.deflection.as_criterion_status(),
        CriterionKind::DepthOfCut => v.depth.as_criterion_status(),
        other => panic!("{other:?} does not bin"),
    }
}

#[test]
fn histogram_population_is_the_gate_population() {
    let trace = fixture();
    let tool = tool();
    let material = material();
    let machine = MachineProfile::shapeoko_makita();
    let tolerance = ToleranceBands::default();
    let ctx = ToolpathLoadContext {
        toolpath_id: TP,
        tool: &tool,
        material: &material,
        operation_family: LutOperationFamily::Pocket,
        pass_role: LutPassRole::Roughing,
        operation_feed_rate_mm_min: OP_FEED,
        operation_kind: OperationType::Pocket,
        spans: None,
        drill_op: None,
    };
    let env = GateEnv {
        sim_trace: Some(&trace),
        machine: Some(&machine),
        tolerance: &tolerance,
    };
    let verdict = evaluate_toolpath(&ctx, Some(&trace), Some(&machine), &tolerance);

    // `display_peak` is the population maximum on `Within` and on
    // `Exceeds(High)`. On `Exceeds(Low)` it is the MEDIAN, so the fixture
    // must hold the chipload gate at `Within`.
    assert_eq!(
        verdict.chipload.state(),
        LoadState::Within,
        "fixture must keep chipload Within: {:?}",
        verdict.chipload
    );

    for kind in [
        CriterionKind::Chipload,
        CriterionKind::Power,
        CriterionKind::Deflection,
        CriterionKind::DepthOfCut,
    ] {
        let status = status_for(kind, &verdict);
        assert_ne!(
            status.state,
            LoadState::Unmodeled,
            "{kind:?} must be modelled on this fixture: {:?}",
            status.unmodeled_reason
        );
        let gate_pop = status
            .population
            .expect("a decided gate states its population");
        let peak = status.display_peak.expect("a decided gate states its peak");

        let population = gate_population(DistributionMetric::Criterion(kind), &ctx, &env);

        // Claim 2 — the size.
        assert_eq!(
            population.len(),
            gate_pop.contributing,
            "{kind:?}: histogram population size != gate contributing"
        );
        assert!(
            gate_pop.contributing > 0,
            "{kind:?}: the fixture is not vacuous"
        );
        let decoys: &[usize] = match kind {
            CriterionKind::Chipload => &[13, 14, 15, 16, 17],
            CriterionKind::DepthOfCut => &[13, 17],
            _ => &[13, 14, 17],
        };
        assert!(
            population.iter().all(|p| !decoys.contains(&p.sample_index)),
            "{kind:?}: a decoy sample entered the population"
        );

        // Claim 1 — the maximum.
        let max = population
            .iter()
            .map(|p| p.value)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(
            (max - peak).abs() <= 1e-12 * peak.abs().max(1.0),
            "{kind:?}: population max {max} != gate display_peak {peak}"
        );

        // Claim 3 — the shares, through the public door.
        let outcome =
            metric_distribution(DistributionMetric::Criterion(kind), &verdict, &ctx, &env)
                .expect("a milling criterion has a card");
        let DistributionOutcome::Measured(dist) = outcome else {
            panic!("{kind:?}: expected a measured distribution, got {outcome:?}");
        };
        let h = &dist.histogram;
        assert_eq!(dist.state, Some(status.state));
        assert_eq!(dist.population, gate_pop);
        assert_eq!(h.counts.iter().sum::<usize>(), gate_pop.contributing);
        assert_eq!(h.edges.len(), h.weights_s.len() + 1);
        assert!(h.total_s > 0.0, "{kind:?}: total_s must not be zero");
        let bins_s: f64 = h.weights_s.iter().sum();
        assert!(
            (bins_s - h.total_s).abs() < 1e-9,
            "{kind:?}: bins {bins_s} != total {}",
            h.total_s
        );
        assert!(
            (h.below_s + h.above_s + h.in_band_s - h.total_s).abs() < 1e-9,
            "{kind:?}: below + above + in band != total"
        );

        // The bounds are the gate's, and only the gate's.
        assert_eq!(
            h.ceiling, status.bound,
            "{kind:?}: ceiling is the gate bound"
        );
        let floor = match &status.bound_source {
            Some(BoundSource::VendorChipBand {
                floor_mm_per_tooth, ..
            }) => *floor_mm_per_tooth,
            _ => None,
        };
        assert_eq!(h.floor, floor, "{kind:?}: floor only from the vendor band");

        // below_s and above_s again, on a separate path.
        let expected_below: f64 = population
            .iter()
            .filter(|p| floor.is_some_and(|f| boundary::below_low(p.value, f, 0.0)))
            .map(|p| p.weight_s)
            .sum();
        let expected_above: f64 = population
            .iter()
            .filter(|p| {
                !floor.is_some_and(|f| boundary::below_low(p.value, f, 0.0))
                    && status
                        .bound
                        .is_some_and(|c| boundary::exceeds_high(p.value, c, 0.0))
            })
            .map(|p| p.weight_s)
            .sum();
        assert!(
            (h.below_s - expected_below).abs() < 1e-12,
            "{kind:?}: below_s"
        );
        assert!(
            (h.above_s - expected_above).abs() < 1e-12,
            "{kind:?}: above_s"
        );
        // Non-vacuity anchor: sample 12 (0.004 mm/tooth) is below any
        // plausible vendor floor, so a floor that exists holds some time.
        if kind == CriterionKind::Chipload && floor.is_some() {
            assert!(
                h.below_s >= SEGMENT_S - 1e-12,
                "the sample below the vendor floor must land in below_s"
            );
        }
    }

    // Engagement has no gate. It uses the power gate's pre-filter and the
    // shared steady-state predicate, so the decoys stay out here too.
    let outcome = metric_distribution(DistributionMetric::Engagement, &verdict, &ctx, &env)
        .expect("a milling op has an engagement card");
    let DistributionOutcome::Measured(dist) = outcome else {
        panic!("expected a measured engagement distribution, got {outcome:?}");
    };
    // 0..=12 plus 15 and 16: engagement has no feed filter and no
    // chip-model predicate.
    assert_eq!(dist.population.contributing, 15);
    assert!(dist.histogram.floor.is_none() && dist.histogram.ceiling.is_none());
}

/// A drill cycle has no card, and a missing trace is not measured. Neither
/// draws an empty histogram (X-VAC).
#[test]
fn not_applicable_has_no_card_and_unmodelled_is_not_measured() {
    let tool = tool();
    let material = material();
    let machine = MachineProfile::shapeoko_makita();
    let tolerance = ToleranceBands::default();
    let mut ctx = ToolpathLoadContext {
        toolpath_id: TP,
        tool: &tool,
        material: &material,
        operation_family: LutOperationFamily::Pocket,
        pass_role: LutPassRole::Roughing,
        operation_feed_rate_mm_min: OP_FEED,
        operation_kind: OperationType::Pocket,
        spans: None,
        drill_op: None,
    };
    let env = GateEnv {
        sim_trace: None,
        machine: Some(&machine),
        tolerance: &tolerance,
    };
    let verdict = evaluate_toolpath(&ctx, None, Some(&machine), &tolerance);
    for metric in [
        DistributionMetric::Criterion(CriterionKind::Chipload),
        DistributionMetric::Criterion(CriterionKind::Power),
        DistributionMetric::Engagement,
    ] {
        let outcome = metric_distribution(metric, &verdict, &ctx, &env);
        assert!(
            matches!(outcome, Some(DistributionOutcome::NotMeasured(_))),
            "{metric:?} with no trace must be NotMeasured, got {outcome:?}"
        );
    }
    assert!(
        metric_distribution(
            DistributionMetric::Criterion(CriterionKind::GantryPush),
            &verdict,
            &ctx,
            &env
        )
        .is_none()
    );

    ctx.operation_kind = OperationType::Drill;
    let verdict = evaluate_toolpath(&ctx, None, Some(&machine), &tolerance);
    for metric in [
        DistributionMetric::Criterion(CriterionKind::Chipload),
        DistributionMetric::Criterion(CriterionKind::Deflection),
        DistributionMetric::Engagement,
    ] {
        assert!(
            metric_distribution(metric, &verdict, &ctx, &env).is_none(),
            "{metric:?} on a drill cycle must have no card"
        );
    }
}
