//! Unit tests for the optimizer orchestration layer.
//!
//! Moved out of `tool_load/optimize/mod.rs` by P4; the module body is
//! unchanged.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use super::delta::{classify_one_gate_chipload, classify_one_gate_power};
use super::*;
use crate::tool_load::verdict::{DeflectionVerdict, PowerVerdict};

#[test]
fn param_delta_has_changes_detects_any_field() {
    assert!(!ParamDelta::default().has_changes());
    assert!(
        ParamDelta {
            feed_mm_min: Some(2100.0),
            ..Default::default()
        }
        .has_changes()
    );
    assert!(
        ParamDelta {
            stepover_mm: Some(0.8),
            ..Default::default()
        }
        .has_changes()
    );
}

#[test]
fn first_safe_skips_index_zero_baseline() {
    // Skipped/NoSafeImprovement outcomes never recommend.
    let skipped = OptimizeOutcome::skipped(RefuseReason::SimulationRequired);
    assert!(skipped.first_safe().is_none());

    let nsi = OptimizeOutcome::no_safe_improvement(
        Vec::new(),
        RefuseReason::NoFeasibleRow,
        OutcomeNarrative {
            explanation: "test".to_owned(),
            ..OutcomeNarrative::default()
        },
    );
    assert!(nsi.first_safe().is_none());
}

fn synthetic_candidate(
    feed: f64,
    cycle_time: f64,
    verdict: ToolpathLoadVerdict,
) -> OptimizeCandidate {
    use crate::compute::operation_configs::PocketConfig;
    OptimizeCandidate {
        params: OperationConfig::Pocket(PocketConfig {
            feed_rate: feed,
            ..PocketConfig::default()
        }),
        delta: ParamDelta {
            feed_mm_min: Some(feed),
            ..Default::default()
        },
        cycle_time_s: cycle_time,
        verdict,
        stage: SearchStage::Refined,
        reconciled_cycle_time_s: None,
        reconciled_verdict: None,
        gate_deltas: None,
        air_cut_fraction_of_total_runtime: None,
    }
}

fn within_power_verdict() -> PowerVerdict {
    use super::super::verdict::{Confidence, SampleEvidence};
    PowerVerdict::Within {
        peak_kw: 0.5,
        available_kw: 0.71,
        evidence: SampleEvidence::empty(),
        confidence: Confidence::Validated,
        entry_spike: None,
        bound_source: None,
    }
}

fn within_deflection_verdict(peak_mm: f64) -> DeflectionVerdict {
    use super::super::verdict::{Confidence, DeflectionBounds, SampleEvidence};
    DeflectionVerdict::Within {
        peak_mm,
        bounds: DeflectionBounds {
            validated_within_mm: 0.050,
            exceeds_mm: 0.200,
        },
        evidence: SampleEvidence::empty(),
        confidence: Confidence::Validated,
        entry_spike: None,
    }
}

fn within_chipload_verdict(peak: f64) -> ChiploadVerdict {
    use super::super::verdict::{
        ChipBounds, ChipBoundsSource, ChiploadMetric, ChiploadStatistic, Confidence, SampleEvidence,
    };
    ChiploadVerdict::Within {
        approach_to_min: None,
        approach_to_max: ChiploadMetric {
            observed_mm_per_tooth: peak,
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
    }
}

fn exceeds_chipload_verdict_high(peak: f64) -> ChiploadVerdict {
    use super::super::verdict::{
        ChipBounds, ChipBoundsSource, ChipSide, ChiploadMetric, ChiploadStatistic, Confidence,
        SampleEvidence,
    };
    ChiploadVerdict::Exceeds {
        side: ChipSide::High,
        triggering: ChiploadMetric {
            observed_mm_per_tooth: peak,
            statistic: ChiploadStatistic::PeakHigh,
            evidence: SampleEvidence::at(0),
            bounds: ChipBounds {
                min_mm_per_tooth: Some(0.038),
                max_mm_per_tooth: 0.07,
                source: ChipBoundsSource::VendorLut,
            },
        },
        confidence: Confidence::Validated,
    }
}

fn within_verdict() -> ToolpathLoadVerdict {
    ToolpathLoadVerdict {
        toolpath_id: ToolpathId(0),
        chipload: within_chipload_verdict(0.04),
        power: within_power_verdict(),
        deflection: within_deflection_verdict(0.030),
        // S3: a hand-built fixture states no measured depth; the
        // row keeps the posture of the gates beside it.
        depth: crate::tool_load::verdict::DepthVerdict::fixture_within(),
        drill_gates: None,
        modulation_summary: None,
        feed_explanation: None,
        kinematic_utilization: None,
    }
}

fn exceeds_chipload_verdict() -> ToolpathLoadVerdict {
    ToolpathLoadVerdict {
        toolpath_id: ToolpathId(0),
        chipload: exceeds_chipload_verdict_high(0.08),
        power: within_power_verdict(),
        deflection: within_deflection_verdict(0.030),
        // S3: a hand-built fixture states no measured depth; the
        // row keeps the posture of the gates beside it.
        depth: crate::tool_load::verdict::DepthVerdict::fixture_within(),
        drill_gates: None,
        modulation_summary: None,
        feed_explanation: None,
        kinematic_utilization: None,
    }
}

fn ranked_test(candidates: Vec<OptimizeCandidate>) -> OptimizeOutcome {
    OptimizeOutcome::ranked(candidates, None, OutcomeNarrative::default())
}

#[test]
fn first_safe_finds_faster_safe_candidate() {
    let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
    let faster_safe = synthetic_candidate(2100.0, 70.0, within_verdict());
    let outcome = ranked_test(vec![baseline, faster_safe]);
    let recommended = outcome.first_safe().expect("should recommend");
    assert!((recommended.cycle_time_s - 70.0).abs() < 1e-9);
}

#[test]
fn first_safe_skips_unsafe_candidates() {
    let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
    let faster_unsafe = synthetic_candidate(2500.0, 60.0, exceeds_chipload_verdict());
    let faster_safe = synthetic_candidate(2100.0, 70.0, within_verdict());
    // Unsafe is faster but is not safe; recommendation is the safe one.
    let outcome = ranked_test(vec![baseline, faster_unsafe, faster_safe]);
    let recommended = outcome.first_safe().expect("should recommend");
    assert!((recommended.cycle_time_s - 70.0).abs() < 1e-9);
}

#[test]
fn first_safe_returns_none_when_only_slower_candidates() {
    let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
    let slower_safe = synthetic_candidate(1200.0, 110.0, within_verdict());
    let outcome = ranked_test(vec![baseline, slower_safe]);
    assert!(outcome.first_safe().is_none());
}

#[test]
fn first_safe_returns_none_when_all_candidates_unsafe() {
    let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
    let faster_unsafe = synthetic_candidate(2500.0, 60.0, exceeds_chipload_verdict());
    let outcome = ranked_test(vec![baseline, faster_unsafe]);
    assert!(outcome.first_safe().is_none());
}

#[test]
fn first_safe_index_returns_position_of_recommended() {
    let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
    let faster_unsafe = synthetic_candidate(2500.0, 60.0, exceeds_chipload_verdict());
    let faster_safe = synthetic_candidate(2100.0, 70.0, within_verdict());
    let candidates = vec![baseline, faster_unsafe, faster_safe];
    // Index 1 is unsafe, so the recommendation is index 2.
    assert_eq!(
        ProjectOptimizeReport::first_safe_index(&candidates),
        Some(2)
    );
}

#[test]
fn first_safe_index_returns_none_when_no_recommendation() {
    let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
    let slower_safe = synthetic_candidate(1200.0, 110.0, within_verdict());
    let candidates = vec![baseline, slower_safe];
    assert_eq!(ProjectOptimizeReport::first_safe_index(&candidates), None);
}

#[test]
fn first_safe_index_returns_none_when_only_baseline() {
    let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
    let candidates = vec![baseline];
    assert_eq!(ProjectOptimizeReport::first_safe_index(&candidates), None);
}

#[test]
fn select_stage2_keeps_top_n_by_composite_score() {
    // Baseline cycle 200s; all candidates use chipload 0.04 mm/tooth
    // against a 0.038-0.07 LUT bracket, so the within penalties are
    // identical. Cycle savings dominate ordering for the within
    // candidates; the exceeds candidate carries a fixed 2.0 chipload
    // penalty (~10s at α=5.0) but its 140s savings still wins overall.
    let baseline = synthetic_candidate(1500.0, 200.0, within_verdict());
    let candidates = vec![
        synthetic_candidate(2000.0, 80.0, within_verdict()),
        synthetic_candidate(2100.0, 70.0, within_verdict()),
        synthetic_candidate(1900.0, 90.0, within_verdict()),
        synthetic_candidate(2200.0, 60.0, exceeds_chipload_verdict()),
        synthetic_candidate(1800.0, 100.0, within_verdict()),
    ];
    let policy = SearchPolicy::default();
    let top3 = select_stage2_candidates(candidates, &baseline, &policy, 3);
    assert_eq!(top3.len(), 3);
    // Top three by composite score: 60 (exceeds, savings 140 - pen 10),
    // then 70 and 80 (within, equal penalty so cycle dominates).
    assert!((top3[0].cycle_time_s - 60.0).abs() < 1e-9);
    assert!((top3[1].cycle_time_s - 70.0).abs() < 1e-9);
    assert!((top3[2].cycle_time_s - 80.0).abs() < 1e-9);
}

#[test]
fn select_stage2_prefers_midpoint_over_band_edge_at_close_cycle_time() {
    // Demonstrates the layer 2b reorder relative to a pure-cycle-time
    // sort. Two within candidates with similar cycle times but
    // different chipload positions: the midpoint candidate wins
    // despite being slightly slower.
    let baseline = synthetic_candidate(1500.0, 200.0, within_verdict());
    // Build chipload verdicts with both approach_to_min AND
    // approach_to_max so the rank-side midpoint comes from the
    // real LUT bracket (0.038 / 0.070, mid = 0.054, half = 0.016)
    // and not from the synthetic `max * 0.5` fallback.
    let with_chipload = |cl: f64| {
        use super::super::verdict::{
            ChipBounds, ChipBoundsSource, ChiploadMetric, ChiploadStatistic, Confidence,
            SampleEvidence,
        };
        let bounds = ChipBounds {
            min_mm_per_tooth: Some(0.038),
            max_mm_per_tooth: 0.070,
            source: ChipBoundsSource::VendorLut,
        };
        let chipload = ChiploadVerdict::Within {
            approach_to_min: Some(ChiploadMetric {
                observed_mm_per_tooth: cl,
                statistic: ChiploadStatistic::MedianLow,
                evidence: SampleEvidence::empty(),
                bounds: bounds.clone(),
            }),
            approach_to_max: ChiploadMetric {
                observed_mm_per_tooth: cl,
                statistic: ChiploadStatistic::PeakInRange,
                evidence: SampleEvidence::empty(),
                bounds,
            },
            confidence: Confidence::Validated,
            entry_spikes: Vec::new(),
            burn_advisory: None,
        };
        ToolpathLoadVerdict {
            toolpath_id: ToolpathId(0),
            chipload,
            power: within_power_verdict(),
            deflection: within_deflection_verdict(0.030),
            // S3: a hand-built fixture states no measured depth; the
            // row keeps the posture of the gates beside it.
            depth: crate::tool_load::verdict::DepthVerdict::fixture_within(),
            drill_gates: None,
            modulation_summary: None,
            feed_explanation: None,
            kinematic_utilization: None,
        }
    };
    // Faster but parked at LUT max (chipload 0.07 → distance 1.0
    // from midpoint 0.054 → penalty 5.0 at α=5.0).
    let edge = synthetic_candidate(2200.0, 76.0, with_chipload(0.07));
    // Slower by 4s but at midpoint → penalty 0.
    let mid = synthetic_candidate(2000.0, 80.0, with_chipload(0.054));
    let policy = SearchPolicy::default();
    let top = select_stage2_candidates(vec![edge, mid], &baseline, &policy, 2);
    // Edge: savings 124, penalty 5 → score 119.
    // Mid:  savings 120, penalty 0 → score 120.
    // Midpoint wins.
    assert!((top[0].cycle_time_s - 80.0).abs() < 1e-9);
    assert!((top[1].cycle_time_s - 76.0).abs() < 1e-9);
}

#[test]
fn select_stage2_does_not_panic_when_fewer_candidates_than_n() {
    let baseline = synthetic_candidate(1500.0, 200.0, within_verdict());
    let candidates = vec![synthetic_candidate(2000.0, 80.0, within_verdict())];
    let policy = SearchPolicy::default();
    let result = select_stage2_candidates(candidates, &baseline, &policy, 3);
    assert_eq!(result.len(), 1);
}

#[test]
fn build_outcome_empty_candidates_yields_no_safe_improvement() {
    let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
    let outcome = build_outcome(
        baseline,
        Vec::new(),
        &crate::machine::MachineProfile::default(),
    );
    assert_eq!(
        outcome.kind,
        OutcomeKind::NoSafeImprovement,
        "got {outcome:?}"
    );
    assert_eq!(outcome.reason, Some(RefuseReason::NoImprovementFound));
    let explanation = &outcome.narrative.explanation;
    assert!(
        explanation.contains("no candidates"),
        "explanation should say no candidates were produced: {explanation}"
    );
    // Even with no candidates, the baseline is preserved so the
    // modal/rollup can show "this is what you had".
    assert_eq!(outcome.candidates.len(), 1);
}

#[test]
fn build_outcome_all_slower_yields_no_safe_improvement() {
    let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
    let candidates = vec![
        synthetic_candidate(1300.0, 105.0, within_verdict()),
        synthetic_candidate(1200.0, 110.0, within_verdict()),
    ];
    let outcome = build_outcome(
        baseline,
        candidates,
        &crate::machine::MachineProfile::default(),
    );
    assert_eq!(
        outcome.kind,
        OutcomeKind::NoSafeImprovement,
        "got {outcome:?}"
    );
    let explanation = &outcome.narrative.explanation;
    assert!(
        explanation.contains("no candidate beat the baseline"),
        "explanation should mention slower-than-baseline: {explanation}"
    );
    // Baseline at index 0 + the two attempted candidates.
    assert_eq!(
        outcome.candidates.len(),
        3,
        "candidates must include baseline + 2 candidates"
    );
}

#[test]
fn build_outcome_all_unsafe_yields_no_safe_improvement() {
    let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
    let candidates = vec![
        synthetic_candidate(2500.0, 60.0, exceeds_chipload_verdict()),
        synthetic_candidate(2300.0, 65.0, exceeds_chipload_verdict()),
    ];
    let outcome = build_outcome(
        baseline,
        candidates,
        &crate::machine::MachineProfile::default(),
    );
    assert_eq!(
        outcome.kind,
        OutcomeKind::NoSafeImprovement,
        "got {outcome:?}"
    );
    let explanation = &outcome.narrative.explanation;
    assert!(
        explanation.contains("gate limit"),
        "explanation should mention gate limit: {explanation}"
    );
    assert_eq!(outcome.candidates.len(), 3);
    // Sorted by ascending cycle time means index 1 has
    // the lower cycle (60s), index 2 the higher (65s).
    assert!(outcome.candidates[1].cycle_time_s <= outcome.candidates[2].cycle_time_s);
}

#[test]
fn build_outcome_with_safe_faster_candidate_yields_ranked() {
    let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
    let faster_safe = synthetic_candidate(2100.0, 70.0, within_verdict());
    let faster_unsafe = synthetic_candidate(2500.0, 60.0, exceeds_chipload_verdict());
    let candidates = vec![faster_unsafe, faster_safe];
    let outcome = build_outcome(
        baseline,
        candidates,
        &crate::machine::MachineProfile::default(),
    );
    assert_eq!(outcome.kind, OutcomeKind::Ranked, "got {outcome:?}");
    let ranked = &outcome.candidates;
    // Baseline at index 0.
    assert!((ranked[0].cycle_time_s - 100.0).abs() < 1e-9);
    // Sorted ascending after baseline: 60.0 (unsafe), 70.0 (safe).
    assert!((ranked[1].cycle_time_s - 60.0).abs() < 1e-9);
    assert!((ranked[2].cycle_time_s - 70.0).abs() < 1e-9);
}

#[test]
fn refuse_reason_explanations_cover_every_variant() {
    // Smoke test: every variant gives a non-empty explanation. If
    // someone adds a variant without an explanation arm, this fails.
    let variants = [
        RefuseReason::SimulationRequired,
        RefuseReason::ArcEngagementNotCaptured,
        RefuseReason::MaterialUnvalidated,
        RefuseReason::NoVendorData,
        RefuseReason::SteadyStateSamplesNotPresent,
        RefuseReason::BipolarEngagement,
        RefuseReason::DeflectionSetupLocked,
        RefuseReason::NoFeasibleRow,
        RefuseReason::RpmBracketEmpty,
        RefuseReason::DiameterExtrapolationTooPoor,
        RefuseReason::NoImprovementFound,
    ];
    for v in variants {
        let s = v.explanation_for_optimize();
        assert!(!s.is_empty(), "variant {v:?} returned empty explanation");
    }
}

// ── Per-candidate gate deltas + tier dispatcher (Commit #3) ──────

fn exceeds_power_verdict() -> ToolpathLoadVerdict {
    use super::super::verdict::{Confidence, SampleEvidence};
    let mut v = within_verdict();
    v.power = PowerVerdict::Exceeds {
        peak_kw: 2.5,
        available_kw: 0.71,
        evidence: SampleEvidence::at(0),
        confidence: Confidence::Validated,
        bound_source: None,
    };
    v
}

fn unmodeled_chipload_verdict() -> ToolpathLoadVerdict {
    use super::super::verdict::UnmodeledReason;
    let mut v = within_verdict();
    v.chipload = ChiploadVerdict::Unmodeled {
        reason: UnmodeledReason::NoVendorData,
    };
    v
}

/// F2.3 — the Stage F dispatch predicate fires on ANY Exceeds load
/// gate, not chipload alone. A power-only or deflection-only
/// Exceeds baseline must route to the per-gate retargeters (pre-
/// F2.3 it routed to the headroom scale-up).
#[test]
fn retarget_dispatch_fires_on_any_exceeds_gate() {
    use super::super::verdict::{Confidence, DeflectionBounds, DeflectionVerdict, SampleEvidence};
    assert!(!any_load_gate_exceeds(&within_verdict()));
    assert!(!any_load_gate_exceeds(&unmodeled_chipload_verdict()));
    assert!(any_load_gate_exceeds(&exceeds_chipload_verdict()));
    assert!(any_load_gate_exceeds(&exceeds_power_verdict()));

    let mut v = within_verdict();
    v.deflection = DeflectionVerdict::Exceeds {
        peak_mm: 0.35,
        bounds: DeflectionBounds {
            validated_within_mm: 0.050,
            exceeds_mm: 0.200,
        },
        evidence: SampleEvidence::at(0),
        confidence: Confidence::Validated,
    };
    assert!(any_load_gate_exceeds(&v));
}

#[test]
fn classify_one_gate_within_to_within_is_same() {
    let b = within_verdict();
    let mut c = within_verdict();
    if let PowerVerdict::Within { peak_kw, .. } = &mut c.power {
        *peak_kw = 0.55;
    }
    assert_eq!(classify_one_gate_power(&b.power, &c.power), GateDelta::Same);
}

#[test]
fn classify_one_gate_exceeds_to_within_is_improved() {
    let b = exceeds_chipload_verdict();
    let c = within_verdict();
    assert_eq!(
        classify_one_gate_chipload(&b.chipload, &c.chipload),
        GateDelta::Improved
    );
}

#[test]
fn classify_one_gate_within_to_exceeds_is_worsened() {
    let b = within_verdict();
    let c = exceeds_chipload_verdict();
    assert_eq!(
        classify_one_gate_chipload(&b.chipload, &c.chipload),
        GateDelta::Worsened
    );
}

#[test]
fn classify_one_gate_exceeds_to_smaller_exceeds_is_improved() {
    let b_v = exceeds_chipload_verdict_high(0.10);
    let c_v = exceeds_chipload_verdict_high(0.08); // strictly smaller, > 5% threshold
    assert_eq!(classify_one_gate_chipload(&b_v, &c_v), GateDelta::Improved);
}

#[test]
fn classify_one_gate_exceeds_to_larger_exceeds_is_worsened() {
    let b_v = exceeds_chipload_verdict_high(0.08);
    let c_v = exceeds_chipload_verdict_high(0.10);
    assert_eq!(classify_one_gate_chipload(&b_v, &c_v), GateDelta::Worsened);
}

#[test]
fn classify_one_gate_unmodeled_either_side_is_unmodeled() {
    let b = unmodeled_chipload_verdict();
    let c = within_verdict();
    assert_eq!(
        classify_one_gate_chipload(&b.chipload, &c.chipload),
        GateDelta::Unmodeled
    );
    assert_eq!(
        classify_one_gate_chipload(&c.chipload, &b.chipload),
        GateDelta::Unmodeled
    );
}

#[test]
fn gate_deltas_helpers() {
    let pure = GateDeltas {
        chipload: GateDelta::Improved,
        power: GateDelta::Same,
        deflection: GateDelta::Same,
    };
    assert!(pure.no_regression());
    assert!(pure.any_improved());
    assert!(!pure.any_worsened());

    let tradeoff = GateDeltas {
        chipload: GateDelta::Improved,
        power: GateDelta::Worsened,
        deflection: GateDelta::Same,
    };
    assert!(!tradeoff.no_regression());
    assert!(tradeoff.any_improved());
    assert!(tradeoff.any_worsened());
}

#[test]
fn build_outcome_pure_improvement_yields_ranked_with_deltas() {
    // Baseline has chipload Exceeds. Candidate fixes it (Improved
    // on chipload, Same on others) and is faster. Should land in
    // Ranked, with gate_deltas populated on the candidate.
    let baseline = synthetic_candidate(1500.0, 100.0, exceeds_chipload_verdict());
    let pure = synthetic_candidate(2100.0, 70.0, within_verdict());
    let outcome = build_outcome(
        baseline,
        vec![pure],
        &crate::machine::MachineProfile::default(),
    );
    assert_eq!(outcome.kind, OutcomeKind::Ranked, "got {outcome:?}");
    let ranked = &outcome.candidates;
    assert!(ranked[0].gate_deltas.is_none(), "baseline has no deltas");
    let deltas = ranked[1].gate_deltas.expect("candidate has deltas");
    assert_eq!(deltas.chipload, GateDelta::Improved);
    assert_eq!(deltas.power, GateDelta::Same);
    assert_eq!(deltas.deflection, GateDelta::Same);
}

#[test]
fn build_outcome_tradeoff_yields_tradeoff_variant() {
    // Baseline trips chipload. Candidate fixes chipload (Improved)
    // but pushes power into Exceeds (Worsened). Faster overall.
    // Pure-improvement check fails; trade-off check fires.
    let baseline = synthetic_candidate(1500.0, 100.0, exceeds_chipload_verdict());
    let mut tradeoff_verdict = within_verdict();
    // Fix chipload but trip power.
    tradeoff_verdict.chipload = within_verdict().chipload;
    tradeoff_verdict.power = exceeds_power_verdict().power;
    let candidate = synthetic_candidate(2200.0, 80.0, tradeoff_verdict);
    let outcome = build_outcome(
        baseline,
        vec![candidate],
        &crate::machine::MachineProfile::default(),
    );
    assert_eq!(outcome.kind, OutcomeKind::TradeOff, "got {outcome:?}");
    let tradeoffs = &outcome.candidates;
    assert_eq!(tradeoffs.len(), 2, "baseline + 1 trade-off");
    let deltas = tradeoffs[1].gate_deltas.expect("populated");
    assert_eq!(deltas.chipload, GateDelta::Improved);
    assert_eq!(deltas.power, GateDelta::Worsened);
}

#[test]
fn build_outcome_prefers_pure_improvement_over_tradeoff() {
    // Two candidates: one is a trade-off, one is pure improvement.
    // Pure should win — Ranked outcome.
    let baseline = synthetic_candidate(1500.0, 100.0, exceeds_chipload_verdict());
    let pure = synthetic_candidate(2100.0, 75.0, within_verdict());
    let mut tradeoff_verdict = within_verdict();
    tradeoff_verdict.power = exceeds_power_verdict().power;
    let tradeoff_cand = synthetic_candidate(2200.0, 70.0, tradeoff_verdict);
    let outcome = build_outcome(
        baseline,
        vec![tradeoff_cand, pure],
        &crate::machine::MachineProfile::default(),
    );
    assert_eq!(
        outcome.kind,
        OutcomeKind::Ranked,
        "pure improvement must win, got {outcome:?}"
    );
}

#[test]
fn first_safe_returns_none_for_tradeoff_outcome() {
    // first_safe only auto-recommends from Ranked outcomes.
    // TradeOff candidates need explicit user acceptance.
    let baseline = synthetic_candidate(1500.0, 100.0, exceeds_chipload_verdict());
    let mut tradeoff_verdict = within_verdict();
    tradeoff_verdict.power = exceeds_power_verdict().power;
    let candidate = synthetic_candidate(2200.0, 70.0, tradeoff_verdict);
    let outcome = build_outcome(
        baseline,
        vec![candidate],
        &crate::machine::MachineProfile::default(),
    );
    assert_eq!(outcome.kind, OutcomeKind::TradeOff);
    assert!(
        outcome.first_safe().is_none(),
        "TradeOff outcomes should not auto-recommend"
    );
}

/// Build a `Within` chipload verdict whose `approach_to_max`
/// observed value exceeds the strict LUT max — i.e. a band-admitted
/// reading that `candidate_is_marginally_safe` should flag. Strict
/// LUT max stays at 0.07; observed comes in at `peak`. For the
/// classifier to fire, callers pass `peak > 0.07`.
fn band_admitted_chipload_verdict(peak: f64) -> ChiploadVerdict {
    use super::super::verdict::{
        ChipBounds, ChipBoundsSource, ChiploadMetric, ChiploadStatistic, Confidence, SampleEvidence,
    };
    ChiploadVerdict::Within {
        approach_to_min: None,
        approach_to_max: ChiploadMetric {
            observed_mm_per_tooth: peak,
            statistic: ChiploadStatistic::PeakInRange,
            evidence: SampleEvidence::empty(),
            bounds: ChipBounds {
                min_mm_per_tooth: Some(0.038),
                max_mm_per_tooth: 0.070,
                source: ChipBoundsSource::VendorLut,
            },
        },
        confidence: Confidence::Validated,
        entry_spikes: Vec::new(),
        burn_advisory: None,
    }
}

fn band_admitted_verdict() -> ToolpathLoadVerdict {
    // 0.072 > 0.07 strict max but inside 0.07 × (1 + 0.05) = 0.0735
    // tolerance band → Within with strict-bound breach.
    ToolpathLoadVerdict {
        toolpath_id: ToolpathId(0),
        chipload: band_admitted_chipload_verdict(0.072),
        power: within_power_verdict(),
        deflection: within_deflection_verdict(0.030),
        // S3: a hand-built fixture states no measured depth; the
        // row keeps the posture of the gates beside it.
        depth: crate::tool_load::verdict::DepthVerdict::fixture_within(),
        drill_gates: None,
        modulation_summary: None,
        feed_explanation: None,
        kinematic_utilization: None,
    }
}

/// F3.3 — a `Within` chipload verdict carrying a weak-provenance
/// `burn_advisory` routes the candidate to MarginalSafe (safe to
/// scrap-test, not auto-recommend). Pre-F3.3 such a candidate
/// either hard-refused on the fabricated burn floor or
/// auto-recommended with full authority.
#[test]
fn burn_advisory_candidate_lands_marginal_safe() {
    use super::super::verdict::{
        ChipBounds, ChipBoundsSource, ChiploadMetric, ChiploadStatistic, Confidence, SampleEvidence,
    };
    let advisory_metric = ChiploadMetric {
        observed_mm_per_tooth: 0.005,
        statistic: ChiploadStatistic::MedianLow,
        evidence: SampleEvidence::empty(),
        bounds: ChipBounds {
            min_mm_per_tooth: Some(0.02),
            max_mm_per_tooth: 0.070,
            source: ChipBoundsSource::VendorLutExtrapolated,
        },
    };
    let chipload = match band_admitted_chipload_verdict(0.05) {
        ChiploadVerdict::Within {
            approach_to_min,
            approach_to_max,
            entry_spikes,
            ..
        } => ChiploadVerdict::Within {
            approach_to_min,
            approach_to_max,
            confidence: Confidence::Approximate("extrapolated".to_owned()),
            entry_spikes,
            burn_advisory: Some(Box::new(advisory_metric)),
        },
        other => panic!("fixture must be Within, got {other:?}"),
    };
    let verdict = ToolpathLoadVerdict {
        toolpath_id: ToolpathId(0),
        chipload,
        power: within_power_verdict(),
        deflection: within_deflection_verdict(0.030),
        // S3: a hand-built fixture states no measured depth; the
        // row keeps the posture of the gates beside it.
        depth: crate::tool_load::verdict::DepthVerdict::fixture_within(),
        drill_gates: None,
        modulation_summary: None,
        feed_explanation: None,
        kinematic_utilization: None,
    };
    let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
    let advisory_candidate = synthetic_candidate(2100.0, 75.0, verdict);
    let outcome = build_outcome(
        baseline,
        vec![advisory_candidate],
        &crate::machine::MachineProfile::default(),
    );
    assert_eq!(outcome.kind, OutcomeKind::MarginalSafe, "got {outcome:?}");
    assert!(
        outcome.first_safe().is_none(),
        "burn-advisory candidates must not auto-recommend"
    );
}

#[test]
fn build_outcome_emits_marginal_safe_when_inside_tolerance_band() {
    // G16 §11.4 Layer 3: a Within candidate whose chipload reading
    // is over the strict LUT bound but inside the tolerance band
    // should land in MarginalSafe, not Ranked.
    let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
    let band_admitted = synthetic_candidate(2100.0, 75.0, band_admitted_verdict());
    let outcome = build_outcome(
        baseline,
        vec![band_admitted],
        &crate::machine::MachineProfile::default(),
    );
    assert_eq!(outcome.kind, OutcomeKind::MarginalSafe, "got {outcome:?}");
    assert_eq!(outcome.candidates.len(), 2, "baseline + 1 band-admitted");
    let explanation = &outcome.narrative.explanation;
    assert!(
        explanation.contains("tolerance band") || explanation.contains("scrap"),
        "explanation should mention tolerance band / scrap test: {explanation}"
    );
}

#[test]
fn first_safe_returns_none_for_marginal_safe_outcome() {
    // first_safe is the auto-Apply target — it must NOT include
    // band-admitted candidates. The user picks them via the modal.
    let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
    let band_admitted = synthetic_candidate(2100.0, 75.0, band_admitted_verdict());
    let outcome = build_outcome(
        baseline,
        vec![band_admitted],
        &crate::machine::MachineProfile::default(),
    );
    assert_eq!(outcome.kind, OutcomeKind::MarginalSafe);
    assert!(
        outcome.first_safe().is_none(),
        "MarginalSafe outcomes should not auto-recommend via first_safe"
    );
}

#[test]
fn first_marginal_safe_recommends_from_marginal_outcome() {
    let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
    let band_admitted = synthetic_candidate(2100.0, 75.0, band_admitted_verdict());
    let outcome = build_outcome(
        baseline,
        vec![band_admitted],
        &crate::machine::MachineProfile::default(),
    );
    let recommended = outcome
        .first_marginal_safe()
        .expect("MarginalSafe outcome must surface a recommendation");
    assert!((recommended.cycle_time_s - 75.0).abs() < 1e-9);
}

#[test]
fn build_outcome_prefers_strict_safe_over_marginal_safe() {
    // Pure (strict-LUT) improvement must outrank a band-admitted
    // candidate at comparable cycle time. Outcome lands in Ranked,
    // and first_safe picks the strict candidate.
    let baseline = synthetic_candidate(1500.0, 100.0, within_verdict());
    let band_admitted = synthetic_candidate(2100.0, 70.0, band_admitted_verdict());
    let strict = synthetic_candidate(2000.0, 75.0, within_verdict());
    let outcome = build_outcome(
        baseline,
        vec![band_admitted, strict],
        &crate::machine::MachineProfile::default(),
    );
    assert_eq!(
        outcome.kind,
        OutcomeKind::Ranked,
        "strict-safe pure improvement must win the tier dispatch, got {outcome:?}"
    );
    let recommended = outcome.first_safe().expect("strict candidate present");
    assert!((recommended.cycle_time_s - 75.0).abs() < 1e-9);
}
