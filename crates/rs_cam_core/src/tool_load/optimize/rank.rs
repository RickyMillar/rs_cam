//! Composite scoring for candidate ranking — Layer 2a of §11
//! ([`planning/OPTIMIZER_REFACTOR_G16.md`]).
//!
//! Replaces the cycle-time-only sort with a weighted score that
//! discounts cycle-time savings by three penalty terms read from the
//! typed per-gate verdicts:
//!
//! - **chipload distance** — distance from the LUT-bracket midpoint,
//!   normalised so the bracket bounds sit at 1. Discourages parking a
//!   recommendation right at the breakage / burn edge.
//! - **power overuse** — linear ramp inside the warning band
//!   (`power_warning_fraction × available_kw` → `available_kw`).
//!   Captures the S1 continuous / S6 peak motor-rating convention.
//! - **deflection overuse** — linear ramp across the existing
//!   `validated_within → exceeds` band on `DeflectionBounds`.
//!
//! `Exceeds` candidates that Layer 1's tolerance bands admitted as
//! `Within` carry a high but bounded penalty (the per-gate `Exceeds`
//! arms below); they should still be reachable but never preferred over
//! a strictly-Within sibling at comparable cycle time.
//!
//! Layer 2b (G16 §11) wires this into `select_stage2_candidates` and
//! `build_outcome` — both sort by `composite_score` descending.

use super::OptimizeCandidate;
use super::policy::SearchPolicy;
use crate::tool_load::verdict::{ChiploadVerdict, DeflectionVerdict, PowerVerdict};

/// Composite ranking score, in seconds-of-cycle-time-equivalent units.
/// Higher is better (more cycle-time savings, less penalty).
pub(crate) fn composite_score(
    candidate: &OptimizeCandidate,
    baseline: &OptimizeCandidate,
    policy: &SearchPolicy,
) -> f64 {
    let cycle_savings_s = baseline.cycle_time_s - candidate.cycle_time_s;
    let chipload_pen = chipload_distance_penalty(&candidate.verdict.chipload);
    let power_pen = power_overuse_penalty(
        &candidate.verdict.power,
        policy.ranking.power_warning_fraction.value,
    );
    let defl_pen = deflection_overuse_penalty(&candidate.verdict.deflection);

    let r = &policy.ranking;
    cycle_savings_s
        - r.alpha_chipload_distance.value * chipload_pen
        - r.beta_power_overuse.value * power_pen
        - r.gamma_deflection_overuse.value * defl_pen
}

/// Normalised distance from the LUT-bracket midpoint. 0 at midpoint, 1
/// at either bound. Band-admitted samples can read above 1; clamped at
/// 2 to keep the score finite.
pub(crate) fn chipload_distance_penalty(v: &ChiploadVerdict) -> f64 {
    match v {
        ChiploadVerdict::Within {
            approach_to_min,
            approach_to_max,
            ..
        } => {
            let cl = approach_to_max.observed_mm_per_tooth;
            let max = approach_to_max.bounds.max_mm_per_tooth;
            // Falls back to `max * 0.5` when the matched LUT row has no
            // min bound (burn-side `approach_to_min` is None) — the
            // distance metric still works, just centred on a synthetic
            // midpoint half the bracket below max.
            let min = approach_to_min
                .as_ref()
                .and_then(|m| m.bounds.min_mm_per_tooth)
                .unwrap_or(max * 0.5);
            let mid = 0.5 * (min + max);
            let half = 0.5 * (max - min).max(1e-9);
            ((cl - mid) / half).abs().clamp(0.0, 2.0)
        }
        // Layer 1 may have admitted this via tolerance bands; still
        // penalise so a strictly-Within sibling is preferred.
        ChiploadVerdict::Exceeds { .. } => 2.0,
        // Don't score what we can't measure.
        ChiploadVerdict::Unmodeled { .. } => 0.0,
    }
}

/// Linear ramp from 0 at `warn × available_kw` to 1 at `available_kw`.
/// Below the warning fraction → 0. Above the ceiling → 1 (clamped).
pub(crate) fn power_overuse_penalty(v: &PowerVerdict, warn: f64) -> f64 {
    match v {
        PowerVerdict::Within {
            peak_kw,
            available_kw,
            ..
        } => {
            let frac = (peak_kw / available_kw.max(1e-9)).clamp(0.0, 1.0);
            let span = (1.0 - warn).max(1e-9);
            ((frac - warn) / span).max(0.0)
        }
        PowerVerdict::Exceeds { .. } => 1.5,
        PowerVerdict::Unmodeled { .. } => 0.0,
    }
}

/// Linear ramp across the validated_within → exceeds band on
/// `DeflectionBounds` (50 → 200 µm with default thresholds). 0 below
/// the validated band, 1 at the exceeds threshold.
pub(crate) fn deflection_overuse_penalty(v: &DeflectionVerdict) -> f64 {
    match v {
        DeflectionVerdict::Within {
            peak_mm, bounds, ..
        } => {
            let lo = bounds.validated_within_mm;
            let hi = bounds.exceeds_mm;
            ((peak_mm - lo) / (hi - lo).max(1e-9)).clamp(0.0, 1.0)
        }
        DeflectionVerdict::Exceeds { .. } => 1.5,
        DeflectionVerdict::Unmodeled { .. } => 0.0,
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
    use crate::compute::catalog::OperationConfig;
    use crate::compute::operation_configs::PocketConfig;
    use crate::tool_load::optimize::{ParamDelta, SearchStage};
    use crate::tool_load::verdict::{
        ChipBounds, ChipBoundsSource, ChiploadMetric, ChiploadStatistic, Confidence,
        DeflectionBounds, SampleEvidence, ToolpathLoadVerdict,
    };

    /// LUT bracket with a real lower bound, so the midpoint lookup does
    /// not fall through to the `max * 0.5` synthetic-midpoint branch.
    /// Mid = 0.054 mm/tooth; half-width = 0.016 mm/tooth.
    fn chipload_within(observed: f64) -> ChiploadVerdict {
        let bounds = ChipBounds {
            min_mm_per_tooth: Some(0.038),
            max_mm_per_tooth: 0.070,
            source: ChipBoundsSource::VendorLut,
        };
        ChiploadVerdict::Within {
            approach_to_min: Some(ChiploadMetric {
                observed_mm_per_tooth: observed,
                statistic: ChiploadStatistic::MedianLow,
                evidence: SampleEvidence::empty(),
                bounds: bounds.clone(),
            }),
            approach_to_max: ChiploadMetric {
                observed_mm_per_tooth: observed,
                statistic: ChiploadStatistic::PeakInRange,
                evidence: SampleEvidence::empty(),
                bounds,
            },
            confidence: Confidence::Validated,
        }
    }

    fn power_within(peak_kw: f64, available_kw: f64) -> PowerVerdict {
        PowerVerdict::Within {
            peak_kw,
            available_kw,
            evidence: SampleEvidence::empty(),
            confidence: Confidence::Validated,
        }
    }

    fn deflection_within(peak_mm: f64) -> DeflectionVerdict {
        DeflectionVerdict::Within {
            peak_mm,
            bounds: DeflectionBounds {
                validated_within_mm: 0.050,
                exceeds_mm: 0.200,
            },
            evidence: SampleEvidence::empty(),
            confidence: Confidence::Validated,
        }
    }

    fn candidate(
        cycle_time_s: f64,
        chipload: ChiploadVerdict,
        power: PowerVerdict,
        deflection: DeflectionVerdict,
    ) -> OptimizeCandidate {
        OptimizeCandidate {
            params: OperationConfig::Pocket(PocketConfig::default()),
            delta: ParamDelta::default(),
            cycle_time_s,
            verdict: ToolpathLoadVerdict {
                toolpath_id: 0,
                chipload,
                power,
                deflection,
            },
            stage: SearchStage::Refined,
            reconciled_cycle_time_s: None,
            reconciled_verdict: None,
            gate_deltas: None,
        }
    }

    #[test]
    fn composite_score_prefers_midpoint_when_cycle_times_equal() {
        let policy = SearchPolicy::default();
        let baseline = candidate(
            120.0,
            chipload_within(0.054),
            power_within(0.5, 1.0),
            deflection_within(0.030),
        );
        // Both candidates: same cycle savings vs baseline; differ only
        // in chipload position relative to the LUT midpoint (0.054).
        let midpoint = candidate(
            100.0,
            chipload_within(0.054),
            power_within(0.5, 1.0),
            deflection_within(0.030),
        );
        let bound_edge = candidate(
            100.0,
            chipload_within(0.070), // = max
            power_within(0.5, 1.0),
            deflection_within(0.030),
        );

        let mid_score = composite_score(&midpoint, &baseline, &policy);
        let edge_score = composite_score(&bound_edge, &baseline, &policy);
        assert!(
            mid_score > edge_score,
            "midpoint {mid_score} should outrank bound-edge {edge_score} when cycle times are equal"
        );
        // Midpoint penalty = 0 → score = pure cycle savings.
        assert!((mid_score - 20.0).abs() < 1e-9);
        // Bound-edge penalty = 1 × alpha (5.0) → score = 20 - 5 = 15.
        assert!((edge_score - 15.0).abs() < 1e-9);
    }

    #[test]
    fn composite_score_prefers_faster_when_chipload_equal() {
        let policy = SearchPolicy::default();
        let baseline = candidate(
            120.0,
            chipload_within(0.054),
            power_within(0.5, 1.0),
            deflection_within(0.030),
        );
        // Same chipload position → same penalty; faster cycle wins.
        let faster = candidate(
            90.0,
            chipload_within(0.060),
            power_within(0.5, 1.0),
            deflection_within(0.030),
        );
        let slower = candidate(
            110.0,
            chipload_within(0.060),
            power_within(0.5, 1.0),
            deflection_within(0.030),
        );

        let faster_score = composite_score(&faster, &baseline, &policy);
        let slower_score = composite_score(&slower, &baseline, &policy);
        assert!(
            faster_score > slower_score,
            "faster {faster_score} should outrank slower {slower_score} when chipload equal"
        );
    }

    #[test]
    fn power_penalty_zero_at_below_warning_threshold() {
        let warn = 0.80;
        // 50% of available — well below the 80% warning threshold.
        let p_low = power_overuse_penalty(&power_within(0.50, 1.00), warn);
        assert!((p_low - 0.0).abs() < 1e-9);
        // Exactly at the warning threshold.
        let p_at = power_overuse_penalty(&power_within(0.80, 1.00), warn);
        assert!((p_at - 0.0).abs() < 1e-9);
        // Half-way through the band (90% of available).
        let p_mid = power_overuse_penalty(&power_within(0.90, 1.00), warn);
        assert!((p_mid - 0.5).abs() < 1e-9);
        // At the ceiling.
        let p_max = power_overuse_penalty(&power_within(1.00, 1.00), warn);
        assert!((p_max - 1.0).abs() < 1e-9);
    }

    #[test]
    fn deflection_penalty_ramps_in_band() {
        // Below validated_within (50 µm) → 0.
        let below = deflection_overuse_penalty(&deflection_within(0.030));
        assert!((below - 0.0).abs() < 1e-9);
        // Exactly at the validated bound.
        let at_lo = deflection_overuse_penalty(&deflection_within(0.050));
        assert!((at_lo - 0.0).abs() < 1e-9);
        // Midpoint of band (125 µm = 50 + (200-50)/2).
        let mid = deflection_overuse_penalty(&deflection_within(0.125));
        assert!((mid - 0.5).abs() < 1e-9);
        // At the exceeds threshold.
        let at_hi = deflection_overuse_penalty(&deflection_within(0.200));
        assert!((at_hi - 1.0).abs() < 1e-9);
        // Above exceeds → still 1 (clamped); structural Exceeds would
        // route through the Exceeds arm with penalty 1.5 instead.
        let above = deflection_overuse_penalty(&deflection_within(0.250));
        assert!((above - 1.0).abs() < 1e-9);
    }
}
