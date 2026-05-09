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
use crate::tool_load::verdict::{PowerVerdict, ToolpathLoadVerdict};

use super::super::axes::{AxisView, SearchAxis};
use super::super::context::machine_max_power_kw;
use super::super::patches::{AxisPatch, PatchSource};
use super::super::policy::SearchPolicy;
use super::super::search_policy;
use super::{CandidatePatch, OptimizationStrategy};

/// Inputs to Stage 0's analytical scaling. Bundled so the function
/// stays terse and the call sites in the orchestrator are explicit
/// about where each value comes from.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Stage0Inputs<'a> {
    /// Baseline spindle RPM the optimizer scales from. Prefer the
    /// trace's actual median RPM over the operation config's
    /// `spindle_rpm` field — the trace is what actually ran.
    pub rpm_baseline: f64,
    /// Baseline commanded feed in mm/min (from the operation config).
    pub feed_baseline_mm_min: f64,
    /// Peak power in kW from the baseline trace, as measured by the
    /// power gate (`power::evaluate` returns this in `Verdict::Within
    /// { peak }` or `Exceeds { peak }`). `None` if the gate refused
    /// (Unmodeled) — Stage 0 cannot scale safely without a power
    /// reference.
    pub peak_power_baseline_kw: Option<f64>,
    pub machine: &'a MachineProfile,
    /// Matched LUT row carrying the calibrated RPM bracket. `None`
    /// drops the LUT-RPM constraint from the scale solve (the optimizer
    /// will rely on machine bounds alone).
    pub lut_row: Option<&'a MatchedRow>,
}

/// Extract peak power from the gate's verdict. `None` for `Unmodeled`.
pub(crate) fn baseline_peak_power_kw(verdict: &ToolpathLoadVerdict) -> Option<f64> {
    match verdict.power {
        PowerVerdict::Within { peak_kw, .. } | PowerVerdict::Exceeds { peak_kw, .. } => {
            Some(peak_kw)
        }
        PowerVerdict::Unmodeled { .. } => None,
    }
}

/// Solve the maximum scale factor `k` that keeps all four limits
/// satisfied at constant chipload. Returns `1.0` if no headroom is
/// available (every cap is already binding at baseline).
///
/// Note: this is a pure arithmetic helper. The orchestrator must
/// independently decide whether to run Stage 0 at all (skip it for
/// `Exceeds`-baseline TPs per Engineering Default 6).
pub(crate) fn solve_headroom_scale(inputs: &Stage0Inputs<'_>) -> f64 {
    let policy = search_policy();
    let min_positive = policy.feed.min_positive_scale_input.value;
    let rpm_baseline = inputs.rpm_baseline.max(min_positive);
    let feed_baseline = inputs.feed_baseline_mm_min.max(min_positive);

    // 1. Machine RPM cap.
    let (_min_rpm, max_rpm) = inputs.machine.rpm_range();
    let k_rpm_machine = max_rpm / rpm_baseline;

    // 2. Machine feed cap.
    let k_feed = inputs.machine.max_feed_mm_min / feed_baseline;

    // 3. Power cap. `machine_max_power_kw × safety / peak_baseline`
    //    works for both `ConstantPower` (rhs is constant) and
    //    `VfdConstantTorque` (below rated_rpm both sides scale with k
    //    so the inequality is invariant; above rated_rpm the rhs caps
    //    at rated_power so the bound is the same scalar).
    let k_power = match inputs.peak_power_baseline_kw {
        Some(peak) if peak > 0.0 => {
            machine_max_power_kw(inputs.machine) * inputs.machine.safety_factor / peak
        }
        _ => f64::INFINITY,
    };

    // 4. LUT-row RPM cap. Use rpm_max if present; fall back to
    //    rpm_nominal × 1.2 (Engineering Default 5's ±20% bracket); fall
    //    back further to the machine ceiling (no LUT constraint).
    let k_lut = match inputs.lut_row {
        Some(row) => {
            let rpm_nominal_headroom = policy.feed.lut_rpm_nominal_headroom.value;
            let lut_rpm_max = row
                .rpm_max
                .or_else(|| row.rpm_nominal.map(|n| n * rpm_nominal_headroom))
                .unwrap_or(max_rpm);
            lut_rpm_max / rpm_baseline
        }
        None => f64::INFINITY,
    };

    let k_unclamped = k_rpm_machine.min(k_feed).min(k_power).min(k_lut);
    // Never propose scaling DOWN from baseline in Stage 0. Down-scaling
    // is Stage 1's job for Exceeds baselines.
    k_unclamped.max(policy.feed.scale_floor.value)
}

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
        if baseline_verdict.chipload.is_exceeded() {
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
        let scaled_rpm = self.machine.clamp_rpm(self.baseline_rpm * k).round();

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
    use crate::tool_load::verdict::{Confidence, UnmodeledReason};

    fn pocket_with_feed_rpm(feed: f64, rpm: u32) -> OperationConfig {
        OperationConfig::Pocket(PocketConfig {
            feed_rate: feed,
            spindle_rpm: Some(rpm),
            ..PocketConfig::default()
        })
    }

    fn within(peak: f64) -> crate::tool_load::verdict::ChiploadVerdict {
        use crate::tool_load::verdict::{
            ChipBounds, ChipBoundsSource, ChiploadMetric, ChiploadStatistic, ChiploadVerdict,
            SampleEvidence,
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

    fn exceeds_chipload(peak: f64) -> crate::tool_load::verdict::ChiploadVerdict {
        use crate::tool_load::verdict::{
            ChipBounds, ChipBoundsSource, ChipSide, ChiploadMetric, ChiploadStatistic,
            ChiploadVerdict, SampleEvidence,
        };
        ChiploadVerdict::Exceeds {
            side: ChipSide::Low,
            triggering: ChiploadMetric {
                observed_mm_per_tooth: peak,
                statistic: ChiploadStatistic::MedianLow,
                evidence: SampleEvidence::at_with_stat(0, ChiploadStatistic::MedianLow),
                bounds: ChipBounds {
                    min_mm_per_tooth: Some(0.038),
                    max_mm_per_tooth: 0.07,
                    source: ChipBoundsSource::VendorLut,
                },
            },
            confidence: Confidence::Validated,
        }
    }

    fn unmodeled() -> crate::tool_load::verdict::ChiploadVerdict {
        crate::tool_load::verdict::ChiploadVerdict::Unmodeled {
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
        assert!(
            rpm_v >= 12_000.0,
            "rpm should not go below baseline: got {rpm_v}"
        );
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod stage0_solve_tests {
    //! Tests for the analytical [`solve_headroom_scale`] solver.
    //! Tests for the strategy wrapper (`HeadroomScaleStrategy`) live in
    //! the `tests` module above.

    use super::*;
    use crate::machine::{
        ChipLoadFormula, MachineProfile, PowerModel, RigidityProfile, SpindleConfig,
    };

    fn synthetic_machine(
        max_rpm: f64,
        max_feed: f64,
        power: PowerModel,
        safety: f64,
    ) -> MachineProfile {
        MachineProfile {
            name: "stage0 test".to_owned(),
            spindle: SpindleConfig::Variable {
                min_rpm: 6000.0,
                max_rpm,
            },
            power,
            chip_load: ChipLoadFormula::default(),
            max_feed_mm_min: max_feed,
            max_shank_mm: 7.0,
            rigidity: RigidityProfile::default(),
            safety_factor: safety,
        }
    }

    #[test]
    fn k_is_one_when_all_caps_already_binding() {
        // rpm_baseline at machine max; feed_baseline at machine max;
        // power_baseline at machine cap × safety. No headroom.
        let machine = synthetic_machine(
            18_000.0,
            5_000.0,
            PowerModel::ConstantPower { power_kw: 1.5 },
            0.8,
        );
        let inputs = Stage0Inputs {
            rpm_baseline: 18_000.0,
            feed_baseline_mm_min: 5_000.0,
            peak_power_baseline_kw: Some(1.2), // 1.5 × 0.8 = 1.2
            machine: &machine,
            lut_row: None,
        };
        let k = solve_headroom_scale(&inputs);
        assert!((k - 1.0).abs() < 1e-6, "expected k = 1.0, got {k}");
    }

    #[test]
    fn k_machine_rpm_binds_when_feed_and_power_have_room() {
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,                                    // generous feed cap
            PowerModel::ConstantPower { power_kw: 5.0 }, // generous power
            0.8,
        );
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: Some(0.5),
            machine: &machine,
            lut_row: None,
        };
        // k_rpm = 24000/12000 = 2.0
        // k_feed = 10000/1500 ≈ 6.67
        // k_power = 5.0 × 0.8 / 0.5 = 8.0
        // k = 2.0 (rpm binds)
        let k = solve_headroom_scale(&inputs);
        assert!((k - 2.0).abs() < 1e-6, "expected k = 2.0, got {k}");
    }

    #[test]
    fn k_feed_binds_when_rpm_and_power_have_room() {
        let machine = synthetic_machine(
            30_000.0, // generous rpm
            1_800.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: Some(0.5),
            machine: &machine,
            lut_row: None,
        };
        // k_rpm = 30000/12000 = 2.5
        // k_feed = 1800/1500 = 1.2
        // k_power = 5.0 × 0.8 / 0.5 = 8.0
        // k = 1.2
        let k = solve_headroom_scale(&inputs);
        assert!((k - 1.2).abs() < 1e-6, "expected k = 1.2, got {k}");
    }

    #[test]
    fn k_power_binds_on_constant_power_machine() {
        let machine = synthetic_machine(
            30_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 0.6 }, // tight
            0.8,
        );
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: Some(0.4),
            machine: &machine,
            lut_row: None,
        };
        // k_power = 0.6 × 0.8 / 0.4 = 1.2
        let k = solve_headroom_scale(&inputs);
        assert!((k - 1.2).abs() < 1e-6, "expected k = 1.2, got {k}");
    }

    #[test]
    fn k_power_uses_rated_for_vfd() {
        // VFD: rated_power is the cap that binds k_power, not the
        // power-at-baseline-rpm value.
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,
            PowerModel::VfdConstantTorque {
                rated_power_kw: 1.0,
                rated_rpm: 18_000.0,
            },
            0.8,
        );
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: Some(0.4), // baseline well under rated.
            machine: &machine,
            lut_row: None,
        };
        // k_power = 1.0 × 0.8 / 0.4 = 2.0
        // k_rpm = 24000/12000 = 2.0 (also 2.0)
        // k_feed plenty
        let k = solve_headroom_scale(&inputs);
        assert!((k - 2.0).abs() < 1e-6, "expected k = 2.0, got {k}");
    }

    #[test]
    fn lut_row_rpm_can_bind() {
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        // LUT row carries an explicit rpm_max.
        let lut_row = MatchedRow {
            chip_load_mm: 0.04,
            chip_load_min_mm: Some(0.025),
            chip_load_max_mm: Some(0.055),
            rpm_nominal: Some(15_000.0),
            rpm_min: Some(14_000.0),
            rpm_max: Some(16_000.0),
            ap_min_mm: None,
            ap_max_mm: None,
            ae_min_mm: None,
            ae_max_mm: None,
            observation_id: "test-row".to_owned(),
            source_vendor: "Test".to_owned(),
            score: 100,
            diameter_match_score: 200,
            row_diameter_mm: 6.0,
            chipload_diameter_scale: 1.0,
            chipload_hardness_scale: 1.0,
            is_extrapolated: false,
        };
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: Some(0.4),
            machine: &machine,
            lut_row: Some(&lut_row),
        };
        // k_lut = 16000/12000 ≈ 1.333
        // Other caps higher. k = 1.333.
        let k = solve_headroom_scale(&inputs);
        assert!(
            (k - 16_000.0 / 12_000.0).abs() < 1e-6,
            "expected k ≈ 1.333, got {k}"
        );
    }

    #[test]
    fn lut_falls_back_to_rpm_nominal_times_1_2() {
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        // No rpm_max, no rpm_min, only rpm_nominal — Engineering
        // Default 5: ±20% bracket from nominal.
        let lut_row = MatchedRow {
            chip_load_mm: 0.04,
            chip_load_min_mm: Some(0.025),
            chip_load_max_mm: Some(0.055),
            rpm_nominal: Some(15_000.0),
            rpm_min: None,
            rpm_max: None,
            ap_min_mm: None,
            ap_max_mm: None,
            ae_min_mm: None,
            ae_max_mm: None,
            observation_id: "test-row".to_owned(),
            source_vendor: "Test".to_owned(),
            score: 100,
            diameter_match_score: 200,
            row_diameter_mm: 6.0,
            chipload_diameter_scale: 1.0,
            chipload_hardness_scale: 1.0,
            is_extrapolated: false,
        };
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: Some(0.4),
            machine: &machine,
            lut_row: Some(&lut_row),
        };
        // k_lut = (15000 × 1.2) / 12000 = 1.5
        let k = solve_headroom_scale(&inputs);
        assert!((k - 1.5).abs() < 1e-6, "expected k = 1.5, got {k}");
    }

    #[test]
    fn power_unmodeled_drops_constraint() {
        let machine = synthetic_machine(
            24_000.0,
            10_000.0,
            PowerModel::ConstantPower { power_kw: 5.0 },
            0.8,
        );
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: None, // gate said Unmodeled
            machine: &machine,
            lut_row: None,
        };
        // Without power data, k constrained only by rpm and feed.
        // k_rpm = 2.0; k_feed ≈ 6.67. k = 2.0.
        let k = solve_headroom_scale(&inputs);
        assert!((k - 2.0).abs() < 1e-6, "expected k = 2.0, got {k}");
    }

    #[test]
    fn k_floors_at_one_never_proposes_down_scaling() {
        // Force every limit to be below baseline; Stage 0 should still
        // return k = 1.0 (downscaling is Stage 1's job for Exceeds).
        let machine = synthetic_machine(
            8_000.0, // baseline 12000 already over machine max
            500.0,   // baseline feed 1500 already over machine cap
            PowerModel::ConstantPower { power_kw: 0.1 },
            0.8,
        );
        let inputs = Stage0Inputs {
            rpm_baseline: 12_000.0,
            feed_baseline_mm_min: 1_500.0,
            peak_power_baseline_kw: Some(1.0),
            machine: &machine,
            lut_row: None,
        };
        let k = solve_headroom_scale(&inputs);
        assert!((k - 1.0).abs() < 1e-9, "expected k floored to 1.0, got {k}");
    }
}
