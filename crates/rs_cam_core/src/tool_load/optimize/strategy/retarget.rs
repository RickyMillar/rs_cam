//! Per-gate retargeter strategy — Step 6b, G16.
//!
//! Replaces `run_stage_f_retarget`. For each load-driving gate that's
//! `Exceeds`, runs that gate's [`Retargeter`] and emits one
//! [`CandidatePatch`]. Per design doc §3.5 there is no multi-gate
//! composition — the orchestrator evaluates each candidate
//! independently and ranks. A future `JointRetargetStrategy` may
//! compose retargets if monotonicity tests prove combined responses
//! behave well.
//!
//! **Behaviour change vs. old Stage F.** The legacy
//! `solve_chipload_retarget` produced a `commanded × RCTF` retarget
//! that lowered feed on `BurnRisk`. The new chipload retargeter is
//! sample-driven: target chipload comes from the LUT envelope and the
//! multiplier is `target / observed_peak`, so `BurnRisk` raises feed.
//! Wanaka TP 4 (feed=3150, peak=0.0253, LUT [0.038, 0.07], 1.20×
//! headroom) now produces a feed-up candidate that clamps at the
//! machine 5000 mm/min ceiling — the previous Stage F lowered it.

use crate::tool_load::verdict::ToolpathLoadVerdict;

use super::super::axes::{AxisContext, AxisView};
use super::super::retarget::chipload::ChiploadFeedRetargeter;
use super::super::retarget::deflection::DeflectionDocRetargeter;
use super::super::retarget::power::PowerFeedRetargeter;
use super::super::retarget::{RetargetSolution, Retargeter};
use super::super::space::SearchSpace;
use super::{CandidatePatch, OptimizationStrategy};

const STRATEGY_NAME: &str = "per-gate-retarget";
const CHIPLOAD_SUB: &str = "chipload-retarget";
const POWER_SUB: &str = "power-retarget";
const DEFLECTION_SUB: &str = "deflection-retarget";

/// Wires the three per-gate retargeters into a single strategy. The
/// chipload retargeter is optional: when no LUT row matches we have no
/// chipload envelope to target against, so the chipload arm is skipped
/// rather than emitting a degenerate patch.
///
/// `space` and `ctx` are stored on the struct because the retargeter
/// trait requires them per call but the [`OptimizationStrategy`] trait
/// method only takes the per-call view + verdict.
pub struct PerGateRetargetStrategy<'a> {
    pub chipload: Option<ChiploadFeedRetargeter>,
    pub power: PowerFeedRetargeter,
    pub deflection: DeflectionDocRetargeter,
    pub space: &'a SearchSpace,
    pub ctx: &'a AxisContext<'a>,
}

impl<'a> OptimizationStrategy for PerGateRetargetStrategy<'a> {
    fn name(&self) -> &'static str {
        STRATEGY_NAME
    }

    fn candidates(
        &self,
        baseline: &AxisView<'_>,
        baseline_verdict: &ToolpathLoadVerdict,
    ) -> Vec<CandidatePatch> {
        let mut out: Vec<CandidatePatch> = Vec::new();

        if let Some(cl) = &self.chipload
            && let Some(sol) = cl.target(&baseline_verdict.chipload, self.space, baseline, self.ctx)
        {
            out.push(into_candidate(CHIPLOAD_SUB, sol));
        }

        if let Some(sol) =
            self.power
                .target(&baseline_verdict.power, self.space, baseline, self.ctx)
        {
            out.push(into_candidate(POWER_SUB, sol));
        }

        if let Some(sol) = self.deflection.target(
            &baseline_verdict.deflection,
            self.space,
            baseline,
            self.ctx,
        ) {
            out.push(into_candidate(DEFLECTION_SUB, sol));
        }

        out
    }
}

fn into_candidate(strategy: &'static str, sol: RetargetSolution) -> CandidatePatch {
    CandidatePatch {
        patches: sol.patches,
        strategy,
        rationale: sol.rationale,
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
    use std::ops::Range;

    use super::*;
    use crate::compute::catalog::{OperationConfig, OptimizationSurface};
    use crate::compute::operation_configs::PocketConfig;
    use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use crate::feeds::vendor_lookup::MatchedRow;
    use crate::machine::MachineProfile;
    use crate::material::Material;
    use crate::tool::{FlatEndmill, ToolDefinition};
    use crate::tool_load::optimize::policy::SearchPolicy;
    use crate::tool_load::verdict::{Confidence, ExceedsReason, Verdict};

    struct Env {
        op: OperationConfig,
        machine: MachineProfile,
        material: Material,
        tool: ToolDefinition,
        policy: SearchPolicy,
    }

    impl Env {
        fn new(feed: f64) -> Self {
            let op = OperationConfig::Pocket(PocketConfig {
                feed_rate: feed,
                ..PocketConfig::default()
            });
            let tool_config = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
            let tool = ToolDefinition::new(
                Box::new(FlatEndmill::new(
                    tool_config.diameter,
                    tool_config.cutting_length,
                )),
                tool_config.shank_diameter,
                tool_config.shank_length,
                tool_config.holder_diameter,
                tool_config.stickout,
                tool_config.flute_count,
                tool_config.tool_material,
            );
            Self {
                op,
                machine: MachineProfile::shapeoko_makita(),
                material: Material::default(),
                tool,
                policy: SearchPolicy::default(),
            }
        }

        fn view(&self) -> AxisView<'_> {
            match self.op.optimization_surface() {
                OptimizationSurface::Optimizable(v) => v,
                OptimizationSurface::NotOptimizable { .. } => panic!("must be Optimizable"),
            }
        }

        fn ctx(&self) -> AxisContext<'_> {
            AxisContext {
                project_default_rpm: 18_000,
                machine: &self.machine,
                tool: &self.tool,
                material: &self.material,
            }
        }

        fn space(&self, view: &AxisView<'_>, ctx: &AxisContext<'_>) -> SearchSpace {
            SearchSpace::build(view, ctx, None::<&MatchedRow>, &self.policy)
        }
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
            available_kw: 1.0,
            evidence: SampleEvidence::empty(),
            confidence: Confidence::Validated,
        }
    }

    fn exceeds_power(peak_kw: f64) -> crate::tool_load::verdict::PowerVerdict {
        use crate::tool_load::verdict::{PowerVerdict, SampleEvidence};
        PowerVerdict::Exceeds {
            peak_kw,
            available_kw: 1.0,
            evidence: SampleEvidence::at(0),
            confidence: Confidence::Validated,
        }
    }

    fn exceeds(peak: f64, reason: ExceedsReason) -> Verdict {
        Verdict::Exceeds {
            peak,
            sample_range: Range { start: 0, end: 1 },
            reason,
            confidence: Confidence::Validated,
        }
    }

    fn make_chipload(env: &Env, lut_min: Option<f64>, lut_max: Option<f64>) -> ChiploadFeedRetargeter {
        ChiploadFeedRetargeter {
            lut_chipload_min: lut_min.unwrap_or(f64::NAN),
            lut_chipload_max: lut_max.unwrap_or(f64::NAN),
            low_headroom: env.policy.retarget.chipload_low_headroom.value,
            high_headroom: env.policy.retarget.chipload_high_headroom.value,
            plunge_tracking_threshold: env.policy.feed.plunge_tracking_threshold_fraction.value,
        }
    }

    fn make_power(env: &Env, available_kw: f64) -> PowerFeedRetargeter {
        PowerFeedRetargeter {
            available_kw,
            headroom: env.policy.retarget.power_headroom.value,
            plunge_tracking_threshold: env.policy.feed.plunge_tracking_threshold_fraction.value,
        }
    }

    fn make_deflection(env: &Env) -> DeflectionDocRetargeter {
        DeflectionDocRetargeter::with_headroom(0.200, env.policy.retarget.deflection_headroom.value)
    }

    #[test]
    fn all_within_emits_no_candidates() {
        let env = Env::new(2000.0);
        let view = env.view();
        let ctx = env.ctx();
        let space = env.space(&view, &ctx);
        let strat = PerGateRetargetStrategy {
            chipload: Some(make_chipload(&env, Some(0.05), Some(0.10))),
            power: make_power(&env, 1.0),
            deflection: make_deflection(&env),
            space: &space,
            ctx: &ctx,
        };
        let verdict = ToolpathLoadVerdict {
            toolpath_id: 0,
            chipload: within(0.05),
            power: within_power(0.4),
            deflection: within(0.020),
        };
        assert!(strat.candidates(&view, &verdict).is_empty());
    }

    #[test]
    fn chipload_only_exceeds_emits_one_chipload_candidate() {
        let env = Env::new(2000.0);
        let view = env.view();
        let ctx = env.ctx();
        let space = env.space(&view, &ctx);
        let strat = PerGateRetargetStrategy {
            chipload: Some(make_chipload(&env, Some(0.05), Some(0.10))),
            power: make_power(&env, 1.0),
            deflection: make_deflection(&env),
            space: &space,
            ctx: &ctx,
        };
        let verdict = ToolpathLoadVerdict {
            toolpath_id: 0,
            chipload: exceeds(0.025, ExceedsReason::ChiploadBurnRisk),
            power: within_power(0.4),
            deflection: within(0.020),
        };
        let cps = strat.candidates(&view, &verdict);
        assert_eq!(cps.len(), 1);
        assert_eq!(cps[0].strategy, CHIPLOAD_SUB);
    }

    #[test]
    fn chipload_and_power_exceeds_emits_two_candidates_in_order() {
        let env = Env::new(3000.0);
        let view = env.view();
        let ctx = env.ctx();
        let space = env.space(&view, &ctx);
        let strat = PerGateRetargetStrategy {
            chipload: Some(make_chipload(&env, Some(0.05), Some(0.10))),
            power: make_power(&env, 1.0),
            deflection: make_deflection(&env),
            space: &space,
            ctx: &ctx,
        };
        let verdict = ToolpathLoadVerdict {
            toolpath_id: 0,
            chipload: exceeds(0.025, ExceedsReason::ChiploadBurnRisk),
            power: exceeds_power(1.5),
            deflection: within(0.020),
        };
        let cps = strat.candidates(&view, &verdict);
        assert_eq!(cps.len(), 2);
        assert_eq!(cps[0].strategy, CHIPLOAD_SUB);
        assert_eq!(cps[1].strategy, POWER_SUB);
    }

    #[test]
    fn all_three_gates_exceed_emits_three_candidates() {
        let env = Env::new(3000.0);
        let view = env.view();
        let ctx = env.ctx();
        let space = env.space(&view, &ctx);
        let strat = PerGateRetargetStrategy {
            chipload: Some(make_chipload(&env, Some(0.05), Some(0.10))),
            power: make_power(&env, 1.0),
            deflection: make_deflection(&env),
            space: &space,
            ctx: &ctx,
        };
        let verdict = ToolpathLoadVerdict {
            toolpath_id: 0,
            chipload: exceeds(0.025, ExceedsReason::ChiploadBurnRisk),
            power: exceeds_power(1.5),
            deflection: exceeds(0.32, ExceedsReason::LongToolStiffnessUnsafe),
        };
        let cps = strat.candidates(&view, &verdict);
        assert_eq!(cps.len(), 3);
        assert_eq!(cps[0].strategy, CHIPLOAD_SUB);
        assert_eq!(cps[1].strategy, POWER_SUB);
        assert_eq!(cps[2].strategy, DEFLECTION_SUB);
    }

    #[test]
    fn missing_chipload_retargeter_skips_chipload_candidate() {
        // No matched LUT row → no chipload retargeter; even if chipload
        // is Exceeds, the strategy emits nothing for that gate.
        let env = Env::new(3000.0);
        let view = env.view();
        let ctx = env.ctx();
        let space = env.space(&view, &ctx);
        let strat = PerGateRetargetStrategy {
            chipload: None,
            power: make_power(&env, 1.0),
            deflection: make_deflection(&env),
            space: &space,
            ctx: &ctx,
        };
        let verdict = ToolpathLoadVerdict {
            toolpath_id: 0,
            chipload: exceeds(0.025, ExceedsReason::ChiploadBurnRisk),
            power: within_power(0.4),
            deflection: within(0.020),
        };
        assert!(strat.candidates(&view, &verdict).is_empty());
    }

    #[test]
    fn missing_lut_min_skips_burnrisk_only() {
        // Row with only `chip_load_max_mm` set: BreakageRisk verdicts
        // can retarget; BurnRisk verdicts cannot (NaN target).
        let env = Env::new(3000.0);
        let view = env.view();
        let ctx = env.ctx();
        let space = env.space(&view, &ctx);
        let strat = PerGateRetargetStrategy {
            chipload: Some(make_chipload(&env, None, Some(0.10))),
            power: make_power(&env, 1.0),
            deflection: make_deflection(&env),
            space: &space,
            ctx: &ctx,
        };
        let burn = ToolpathLoadVerdict {
            toolpath_id: 0,
            chipload: exceeds(0.025, ExceedsReason::ChiploadBurnRisk),
            power: within_power(0.4),
            deflection: within(0.020),
        };
        assert!(strat.candidates(&view, &burn).is_empty());

        let breakage = ToolpathLoadVerdict {
            toolpath_id: 0,
            chipload: exceeds(0.20, ExceedsReason::ChiploadBreakageRisk),
            power: within_power(0.4),
            deflection: within(0.020),
        };
        let cps = strat.candidates(&view, &breakage);
        assert_eq!(cps.len(), 1);
        assert_eq!(cps[0].strategy, CHIPLOAD_SUB);
    }

    /// Wanaka TP 4 fixture: BurnRisk on a 3150 mm/min Pocket should now
    /// produce a feed-up candidate (clamped at the 5000 ceiling), not a
    /// feed-down candidate. This regression-pins the Step 6b direction
    /// flip vs. the legacy `solve_chipload_retarget`.
    #[test]
    fn wanaka_tp4_burnrisk_emits_feed_up_candidate() {
        let env = Env::new(3150.0);
        let view = env.view();
        let ctx = env.ctx();
        let space = env.space(&view, &ctx);
        let strat = PerGateRetargetStrategy {
            chipload: Some(make_chipload(&env, Some(0.038), Some(0.07))),
            power: make_power(&env, 1.0),
            deflection: make_deflection(&env),
            space: &space,
            ctx: &ctx,
        };
        let verdict = ToolpathLoadVerdict {
            toolpath_id: 0,
            chipload: exceeds(0.0253, ExceedsReason::ChiploadBurnRisk),
            power: within_power(0.4),
            deflection: within(0.020),
        };
        let cps = strat.candidates(&view, &verdict);
        assert_eq!(cps.len(), 1);
        let primary = cps[0]
            .patches
            .iter()
            .find(|p| matches!(p.source, super::super::super::patches::PatchSource::Primary))
            .expect("primary patch");
        assert!(
            primary.value > 3150.0,
            "feed must rise from baseline; got {}",
            primary.value
        );
        assert!(primary.clamped, "feed should clamp at machine ceiling");
    }
}
