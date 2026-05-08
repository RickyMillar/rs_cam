//! Headroom scale-up strategy — closed-form `(feed, rpm)` solve at
//! constant chipload until a machine, tool, or LUT-row limit binds.
//!
//! Replaces the old `run_stage_0` helper. Same arithmetic
//! ([`super::super::solve_headroom_scale`]); the wrapping is what's
//! new — the strategy returns a [`CandidatePatch`] with two axis
//! patches (feed, rpm) instead of mutating an op directly.
//!
//! Skipped when the baseline trips chipload — proportional scaling
//! preserves chipload, so it can't fix `Exceeds`. Skipped when
//! `solve_headroom_scale` returns ≤ 1.0 (no headroom).

use crate::feeds::vendor_lookup::MatchedRow;
use crate::machine::MachineProfile;
use crate::tool_load::verdict::{ToolpathLoadVerdict, Verdict};

use super::super::axes::{AxisView, SearchAxis};
use super::super::patches::{AxisPatch, PatchSource};
use super::super::policy::SearchPolicy;
use super::super::{Stage0Inputs, baseline_peak_power_kw, solve_headroom_scale};
use super::{CandidatePatch, OptimizationStrategy};

const STRATEGY_NAME: &str = "headroom";

/// Closed-form headroom scale-up. Bound to a single toolpath via
/// constructor inputs so the trait method stays argument-light.
pub struct HeadroomScaleStrategy<'a> {
    pub machine: &'a MachineProfile,
    pub lut_row: Option<&'a MatchedRow>,
    /// Baseline RPM the trace actually ran at — preferred over the op's
    /// commanded `spindle_rpm` (which may be `None` for "use project
    /// default") because Stage 0 scales from the measured value.
    pub baseline_rpm: f64,
    pub policy: &'a SearchPolicy,
}

impl<'a> OptimizationStrategy for HeadroomScaleStrategy<'a> {
    fn name(&self) -> &'static str {
        STRATEGY_NAME
    }

    fn candidates(
        &self,
        baseline: &AxisView<'_>,
        baseline_verdict: &ToolpathLoadVerdict,
    ) -> Vec<CandidatePatch> {
        // Skip when the baseline trips chipload — proportional scaling
        // preserves chipload, so it can never fix Burn/Breakage.
        if matches!(baseline_verdict.chipload, Verdict::Exceeds { .. }) {
            return Vec::new();
        }

        let inputs = Stage0Inputs {
            rpm_baseline: self.baseline_rpm,
            feed_baseline_mm_min: baseline.op.feed_rate(),
            peak_power_baseline_kw: baseline_peak_power_kw(baseline_verdict),
            machine: self.machine,
            lut_row: self.lut_row,
        };
        let k = solve_headroom_scale(&inputs);
        let feed_policy = &self.policy.feed;
        if k <= feed_policy.scale_floor.value + feed_policy.scale_epsilon.value {
            return Vec::new();
        }

        // Scaled (feed, rpm). RPM goes through the machine's clamp/snap
        // so Discrete spindles land on a real dial position; `apply_axis_patch_to_op`
        // does the round-to-u32 conversion downstream.
        let scaled_feed = baseline.op.feed_rate() * k;
        let scaled_rpm = self
            .machine
            .clamp_rpm(self.baseline_rpm * k)
            .round();

        let patches = vec![
            AxisPatch {
                axis: SearchAxis::FeedRate,
                value: scaled_feed,
                clamped: false,
                source: PatchSource::Strategy {
                    strategy: STRATEGY_NAME,
                },
            },
            AxisPatch {
                axis: SearchAxis::SpindleRpm,
                value: scaled_rpm,
                clamped: false,
                source: PatchSource::Strategy {
                    strategy: STRATEGY_NAME,
                },
            },
        ];

        vec![CandidatePatch {
            patches,
            strategy: STRATEGY_NAME,
            rationale: format!(
                "scale (feed, rpm) by {k:.3}× to the closest binding limit (machine envelope, power, LUT RPM)",
            ),
        }]
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
    use crate::compute::catalog::{OperationConfig, OptimizationSurface};
    use crate::compute::operation_configs::PocketConfig;
    use crate::tool_load::verdict::{Confidence, ExceedsReason, UnmodeledReason};
    use std::ops::Range;

    fn pocket_with_feed_rpm(feed: f64, rpm: u32) -> OperationConfig {
        OperationConfig::Pocket(PocketConfig {
            feed_rate: feed,
            spindle_rpm: Some(rpm),
            ..PocketConfig::default()
        })
    }

    fn within(peak: f64) -> Verdict {
        Verdict::Within {
            peak,
            confidence: Confidence::Validated,
        }
    }

    fn within_power(peak_kw: f64) -> crate::tool_load::verdict::PowerVerdict {
        use crate::tool_load::verdict::{PowerVerdict, SampleEvidence};
        PowerVerdict::Within {
            peak_kw,
            available_kw: 0.71,
            evidence: SampleEvidence::empty(),
            confidence: Confidence::Validated,
        }
    }

    fn within_deflection(peak_mm: f64) -> crate::tool_load::verdict::DeflectionVerdict {
        use crate::tool_load::verdict::{DeflectionBounds, DeflectionVerdict, SampleEvidence};
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

    fn exceeds_chipload(peak: f64) -> Verdict {
        Verdict::Exceeds {
            peak,
            sample_range: Range { start: 0, end: 1 },
            reason: ExceedsReason::ChiploadBurnRisk,
            confidence: Confidence::Validated,
        }
    }

    fn unmodeled() -> Verdict {
        Verdict::Unmodeled {
            reason: UnmodeledReason::NotImplemented("test".to_owned()),
        }
    }

    #[test]
    fn within_baseline_emits_one_candidate_with_feed_and_rpm_patches() {
        let machine = MachineProfile::shapeoko_makita();
        let policy = SearchPolicy::default();
        let op = pocket_with_feed_rpm(1500.0, 12_000);
        let OptimizationSurface::Optimizable(view) = op.optimization_surface() else {
            panic!("Pocket should be Optimizable");
        };
        let strategy = HeadroomScaleStrategy {
            machine: &machine,
            lut_row: None,
            baseline_rpm: 12_000.0,
            policy: &policy,
        };
        let verdict = ToolpathLoadVerdict {
            toolpath_id: 0,
            chipload: within(0.05),
            power: within_power(0.4),
            deflection: within_deflection(0.020),
        };

        let candidates = strategy.candidates(&view, &verdict);
        assert_eq!(candidates.len(), 1, "expected one headroom candidate");
        let cp = &candidates[0];
        assert_eq!(cp.strategy, STRATEGY_NAME);
        assert_eq!(cp.patches.len(), 2);
        let axes: Vec<_> = cp.patches.iter().map(|p| p.axis).collect();
        assert!(axes.contains(&SearchAxis::FeedRate));
        assert!(axes.contains(&SearchAxis::SpindleRpm));
        // Both patches scale up, not down.
        let feed_v = cp
            .patches
            .iter()
            .find(|p| p.axis == SearchAxis::FeedRate)
            .unwrap()
            .value;
        let rpm_v = cp
            .patches
            .iter()
            .find(|p| p.axis == SearchAxis::SpindleRpm)
            .unwrap()
            .value;
        assert!(feed_v > 1500.0, "feed should scale up: got {feed_v}");
        assert!(rpm_v >= 12_000.0, "rpm should not go below baseline: got {rpm_v}");
    }

    #[test]
    fn exceeds_chipload_baseline_returns_no_candidates() {
        let machine = MachineProfile::shapeoko_makita();
        let policy = SearchPolicy::default();
        let op = pocket_with_feed_rpm(1500.0, 12_000);
        let OptimizationSurface::Optimizable(view) = op.optimization_surface() else {
            panic!();
        };
        let strategy = HeadroomScaleStrategy {
            machine: &machine,
            lut_row: None,
            baseline_rpm: 12_000.0,
            policy: &policy,
        };
        let verdict = ToolpathLoadVerdict {
            toolpath_id: 0,
            chipload: exceeds_chipload(0.005),
            power: within_power(0.4),
            deflection: within_deflection(0.020),
        };
        assert!(strategy.candidates(&view, &verdict).is_empty());
    }

    #[test]
    fn unmodeled_chipload_does_not_block_headroom() {
        // Unmodeled chipload (e.g. ramp/profile pre-load gate) must not
        // suppress headroom — only Exceeds does.
        let machine = MachineProfile::shapeoko_makita();
        let policy = SearchPolicy::default();
        let op = pocket_with_feed_rpm(1500.0, 12_000);
        let OptimizationSurface::Optimizable(view) = op.optimization_surface() else {
            panic!();
        };
        let strategy = HeadroomScaleStrategy {
            machine: &machine,
            lut_row: None,
            baseline_rpm: 12_000.0,
            policy: &policy,
        };
        let verdict = ToolpathLoadVerdict {
            toolpath_id: 0,
            chipload: unmodeled(),
            power: within_power(0.4),
            deflection: within_deflection(0.020),
        };
        assert_eq!(strategy.candidates(&view, &verdict).len(), 1);
    }

    #[test]
    fn no_headroom_when_baseline_already_at_machine_max() {
        // Baseline at machine ceiling (max_feed_mm_min for shapeoko_makita = 5000).
        // Any scale > 1 violates the feed cap, so solve returns 1.0 and the strategy
        // emits no candidate.
        let machine = MachineProfile::shapeoko_makita();
        let policy = SearchPolicy::default();
        let op = pocket_with_feed_rpm(machine.max_feed_mm_min, 30_000);
        let OptimizationSurface::Optimizable(view) = op.optimization_surface() else {
            panic!();
        };
        let strategy = HeadroomScaleStrategy {
            machine: &machine,
            lut_row: None,
            baseline_rpm: 30_000.0,
            policy: &policy,
        };
        let verdict = ToolpathLoadVerdict {
            toolpath_id: 0,
            chipload: within(0.05),
            power: within_power(0.4),
            deflection: within_deflection(0.020),
        };
        assert!(strategy.candidates(&view, &verdict).is_empty());
    }
}
