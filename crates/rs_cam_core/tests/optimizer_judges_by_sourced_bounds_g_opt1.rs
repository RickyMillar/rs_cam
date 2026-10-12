//! **G-OPT1 — the optimizer judges a candidate by the sourced bounds.**
//!
//! Pre-registered in `planning/optimize_review_2026-10-12/PLAN.md` §3
//! (review finding F1). The 2026-05 search vetoed every candidate whose
//! depth-of-cut row `Exceeds` the machine rule of thumb
//! (`doc_roughing_factor x D`). S3 ruled that row "reported, never a
//! refusal" for export, and the optimizer now reads the export predicate
//! (`CriterionStatus::refuses_export`): only a bound with a source may
//! refuse a candidate.
//!
//! The arms drive the public ranking door
//! `tool_load::optimize::rank_candidates` with hand-built verdicts. No
//! simulation runs.
//!
//! 1. A faster candidate whose depth row `Exceeds` the rule-of-thumb
//!    bound, with every other row `Within`, is `Ranked`. The narrative
//!    of a slower outcome still names the depth row through
//!    `GateKind::Depth`.
//! 2. A faster candidate whose chipload row `Exceeds` a vendor bound is
//!    refused.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::RigidityDepthCap;
use rs_cam_core::tool_load::optimize::{
    FullCutCheck, FullCutReadings, GateKind, OptimizeCandidate, OutcomeKind, ParamDelta,
    SearchNotes, SearchStage, rank_candidates,
};
use rs_cam_core::tool_load::verdict::{
    ChipBounds, ChipBoundsSource, ChipSide, ChiploadMetric, ChiploadStatistic, ChiploadVerdict,
    Confidence, DeflectionBounds, DeflectionVerdict, DepthVerdict, PowerVerdict, SampleEvidence,
    ToolpathLoadVerdict,
};

/// The rule-of-thumb depth cap of the mini project: 0.25 x 3 mm.
fn rule_of_thumb() -> RigidityDepthCap {
    RigidityDepthCap {
        factor: 0.25,
        diameter_mm: 3.0,
    }
}

fn vendor_band() -> ChipBounds {
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
        bounds: vendor_band(),
    }
}

/// Every row `Within`.
fn clean() -> ToolpathLoadVerdict {
    ToolpathLoadVerdict {
        toolpath_id: ToolpathId(2),
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
        depth: DepthVerdict::Within {
            peak_mm: 0.59,
            bound: rule_of_thumb(),
            evidence: SampleEvidence::at(0),
            confidence: Confidence::Validated,
        },
        drill_gates: None,
        modulation_summary: None,
        feed_explanation: None,
        kinematic_utilization: None,
        material_split: None,
    }
}

/// The depth row over the rule of thumb (1.5 mm against 0.75 mm, the
/// review's measured Z22 cell); every other row `Within`.
fn deep() -> ToolpathLoadVerdict {
    ToolpathLoadVerdict {
        depth: DepthVerdict::Exceeds {
            peak_mm: 1.5,
            bound: rule_of_thumb(),
            evidence: SampleEvidence::at(0),
            confidence: Confidence::Validated,
        },
        ..clean()
    }
}

/// The chipload row over the vendor bound; every other row `Within`.
fn over_chipload() -> ToolpathLoadVerdict {
    ToolpathLoadVerdict {
        chipload: ChiploadVerdict::Exceeds {
            side: ChipSide::High,
            triggering: metric(0.09, ChiploadStatistic::PeakHigh),
            confidence: Confidence::Validated,
        },
        ..clean()
    }
}

fn candidate(cycle_s: f64, verdict: ToolpathLoadVerdict, stage: SearchStage) -> OptimizeCandidate {
    OptimizeCandidate {
        params: OperationConfig::Pocket(PocketConfig::default()),
        dressups: Box::default(),
        delta: ParamDelta::default(),
        cycle_time_s: cycle_s,
        verdict,
        // The full cut holds, so only the gates decide.
        full_cut: FullCutCheck::reference(FullCutReadings {
            removed_volume_mm3: Some(1000.0),
            rapid_collisions: Some(0),
            overcut_peak_mm: Some(0.0),
        }),
        stage,
        reconciled_cycle_time_s: None,
        reconciled_verdict: None,
        air_cut_fraction_of_total_runtime: None,
    }
}

fn notes() -> SearchNotes {
    SearchNotes {
        has_lever: true,
        ..SearchNotes::default()
    }
}

#[test]
fn a_depth_row_over_the_rule_of_thumb_does_not_veto_a_faster_candidate() {
    let outcome = rank_candidates(
        candidate(1315.0, clean(), SearchStage::Baseline),
        vec![candidate(371.0, deep(), SearchStage::Measured)],
        notes(),
    );
    assert_eq!(
        outcome.kind,
        OutcomeKind::Ranked,
        "the depth row is reported, not a veto: {:?}",
        outcome.narrative
    );
    assert_eq!(outcome.recommended_index, Some(1));
    let best = outcome.first_safe().expect("a recommended candidate");
    assert!((best.cycle_time_s - 371.0).abs() < 1e-12);
    assert!(
        outcome
            .narrative
            .explanation
            .contains("depth of cut 1.50 mm")
            && outcome.narrative.explanation.contains("not a limit"),
        "the ranked narrative names the depth row it carries: {}",
        outcome.narrative.explanation
    );
}

#[test]
fn a_chipload_row_over_a_vendor_bound_refuses_a_faster_candidate() {
    let outcome = rank_candidates(
        candidate(1315.0, clean(), SearchStage::Baseline),
        vec![candidate(371.0, over_chipload(), SearchStage::Measured)],
        notes(),
    );
    assert_eq!(outcome.kind, OutcomeKind::NoSafeImprovement);
    assert_eq!(outcome.recommended_index, None);
    let gates = &outcome.narrative.limiting_gates;
    assert!(
        gates.iter().any(|g| g.gate == GateKind::Chipload),
        "the narrative names the chipload row: {gates:?}"
    );
    assert!(
        outcome
            .narrative
            .explanation
            .contains("chipload row refuses an export"),
        "the explanation names the refusing row: {}",
        outcome.narrative.explanation
    );
}

/// When the deep candidate is slower, the outcome is not ranked, and the
/// narrative names the depth row as a depth row, not as "chipload,
/// power, or deflection" (F1).
#[test]
fn a_slower_deep_candidate_names_its_depth_row() {
    let outcome = rank_candidates(
        candidate(300.0, clean(), SearchStage::Baseline),
        vec![candidate(371.0, deep(), SearchStage::Measured)],
        notes(),
    );
    assert_eq!(outcome.kind, OutcomeKind::NoSafeImprovement);
    let gates = &outcome.narrative.limiting_gates;
    assert_eq!(gates.len(), 1, "{gates:?}");
    assert_eq!(gates[0].gate, GateKind::Depth);
    assert!(
        outcome.narrative.headline.contains("rule of thumb"),
        "{}",
        outcome.narrative.headline
    );
    assert!(
        !outcome
            .narrative
            .explanation
            .contains("row refuses an export"),
        "the depth row refuses nothing: {}",
        outcome.narrative.explanation
    );
}
