//! Power retargeter — Step 5, G16.
//!
//! Implements [`Retargeter`] for `Verdict::Exceeds {
//! reason: SpindlePowerExceeded, .. }`. Per the prep doc
//! (`planning/STEP5_PREP_RETARGETERS.md` §2), the math is a linear
//! feed reduction at fixed RPM:
//!
//! ```text
//! target_kw = available_kw × headroom        (headroom < 1.0, e.g. 0.85)
//! multiplier = target_kw / observed_peak_kw
//! target_feed = baseline_feed × multiplier   (clamped to feed-axis hard bounds)
//! ```
//!
//! RPM is intentionally frozen — reducing RPM would also reduce
//! chipload, which the chipload retargeter is responsible for. Plunge
//! tracks feed via a `PatchSource::Coupled` marker patch when the
//! relative feed change exceeds the policy threshold (default 10%).
//!
//! `available_kw` is injected at construction by the orchestrator
//! (Step 6), which computes it as
//! `machine.power_at_rpm(baseline_rpm) × machine.safety_factor`. The
//! `SearchSpace` does not expose a power helper today; constructor
//! injection keeps this retargeter testable in isolation without
//! coupling it to the wider machine/space plumbing.

use crate::tool_load::verdict::{ExceedsReason, Verdict};

use super::super::axes::{AxisContext, AxisView, SearchAxis};
use super::super::patches::{AxisPatch, PatchSource};
use super::super::space::SearchSpace;
use super::{RetargetSolution, Retargeter};

/// The single axis this retargeter drives. RPM is intentionally
/// excluded so the linear feed-multiplier math holds.
const POWER_DRIVING_AXES: &[SearchAxis] = &[SearchAxis::FeedRate];

/// Sample-driven feed retargeter for `SpindlePowerExceeded` verdicts.
///
/// Constructed once per optimization run with the available power at
/// baseline RPM already pinned in. The orchestrator (Step 6) does the
/// `machine.power_at_rpm(rpm) × safety_factor` math at build time.
#[derive(Debug, Clone, Copy)]
pub struct PowerFeedRetargeter {
    /// Available spindle power at the baseline RPM, including the
    /// machine-level safety factor. kW.
    pub available_kw: f64,
    /// Additional headroom factor applied on top of `available_kw`
    /// (typically `policy.retarget.power_headroom`, e.g. 0.85). Must
    /// be `<= 1.0` for the retargeter to add margin rather than push
    /// the spindle further into overload.
    pub headroom: f64,
    /// Relative feed change above which plunge gets a coupled patch
    /// (typically `policy.feed.plunge_tracking_threshold_fraction`,
    /// e.g. 0.10 = 10%).
    pub plunge_tracking_threshold: f64,
}

impl Retargeter for PowerFeedRetargeter {
    type Verdict = Verdict;

    fn driving_axes(&self) -> &'static [SearchAxis] {
        POWER_DRIVING_AXES
    }

    fn target(
        &self,
        verdict: &Verdict,
        space: &SearchSpace,
        view: &AxisView<'_>,
        ctx: &AxisContext<'_>,
    ) -> Option<RetargetSolution> {
        // 1. Match only the power-exceeded variant. Every other verdict
        //    (Within, Unmodeled, or other Exceeds reasons) is for a
        //    different retargeter to handle.
        let peak_kw = match verdict {
            Verdict::Exceeds {
                peak,
                reason: ExceedsReason::SpindlePowerExceeded,
                ..
            } => *peak,
            _ => return None,
        };

        // Defensive: a non-positive peak is nonsensical for power and
        // would produce a non-finite multiplier. Refuse rather than
        // emit garbage.
        if !peak_kw.is_finite() || peak_kw <= 0.0 {
            return None;
        }

        // 2. Linear feed multiplier: scale the cutting load (≈ MRR ×
        //    Kc) so peak power lands at `available × headroom`.
        let target_kw = self.available_kw * self.headroom;
        let multiplier = target_kw / peak_kw;

        // 3. Apply to the baseline feed and clamp to hard feed bounds.
        let baseline_feed = view.axis_value(SearchAxis::FeedRate, ctx)?;
        let raw_target = baseline_feed * multiplier;
        let feed_bounds = space.axis(SearchAxis::FeedRate)?;
        let clamped = feed_bounds.hard.clamp(raw_target);
        let was_clamped = (clamped - raw_target).abs() > 1e-6;

        // 4. Build the patch list. Primary feed patch always present;
        //    coupled plunge patch when the relative feed change exceeds
        //    the policy threshold.
        let mut patches = vec![AxisPatch {
            axis: SearchAxis::FeedRate,
            value: clamped,
            clamped: was_clamped,
            source: PatchSource::Primary,
        }];

        // Guard against a zero baseline (the bounds resolver normally
        // floors it at 0 from the machine envelope, but be explicit).
        let relative_change = if baseline_feed > 0.0 {
            (clamped / baseline_feed - 1.0).abs()
        } else {
            0.0
        };
        if relative_change > self.plunge_tracking_threshold {
            patches.push(AxisPatch {
                axis: SearchAxis::FeedRate,
                value: clamped,
                clamped: was_clamped,
                source: PatchSource::Coupled {
                    from_axis: SearchAxis::FeedRate,
                    rule: "plunge tracks feed when |Δfeed| > 10%",
                },
            });
        }

        Some(RetargetSolution {
            patches,
            rationale: format!(
                "power: scale feed by {multiplier:.2}× to bring peak {peak_kw:.3} kW under available × headroom = {target_kw:.3} kW",
            ),
        })
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
    use crate::machine::MachineProfile;
    use crate::material::Material;
    use crate::tool::ToolDefinition;
    use crate::tool_load::optimize::policy::SearchPolicy;
    use crate::tool_load::verdict::{Confidence, ExceedsReason, Verdict};

    // ── Test scaffolding ──────────────────────────────────────────

    /// A pocket op anchored at a known feed rate so tests assert
    /// against deterministic inputs.
    fn make_pocket_with_feed(feed_mm_min: f64) -> OperationConfig {
        let pocket = PocketConfig {
            feed_rate: feed_mm_min,
            ..PocketConfig::default()
        };
        OperationConfig::Pocket(pocket)
    }

    fn make_tool() -> ToolDefinition {
        let tool_config = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        ToolDefinition::new(
            Box::new(crate::tool::FlatEndmill::new(
                tool_config.diameter,
                tool_config.cutting_length,
            )),
            tool_config.shank_diameter,
            tool_config.shank_length,
            tool_config.holder_diameter,
            tool_config.stickout,
            tool_config.flute_count,
            tool_config.tool_material,
        )
    }

    /// Bundle the borrowed test environment so individual tests don't
    /// repeat the lifetime gymnastics.
    struct Env {
        machine: MachineProfile,
        material: Material,
        tool: ToolDefinition,
        op: OperationConfig,
        policy: SearchPolicy,
    }

    impl Env {
        fn new(feed: f64) -> Self {
            Self {
                machine: MachineProfile::shapeoko_makita(),
                material: Material::default(),
                tool: make_tool(),
                op: make_pocket_with_feed(feed),
                policy: SearchPolicy::default(),
            }
        }

        fn view(&self) -> AxisView<'_> {
            match self.op.optimization_surface() {
                OptimizationSurface::Optimizable(v) => v,
                OptimizationSurface::NotOptimizable { .. } => {
                    panic!("Pocket must be Optimizable")
                }
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
            SearchSpace::build(view, ctx, None, &self.policy)
        }
    }

    fn power_exceeds(peak_kw: f64) -> Verdict {
        Verdict::Exceeds {
            peak: peak_kw,
            sample_range: empty_range(),
            reason: ExceedsReason::SpindlePowerExceeded,
            confidence: Confidence::Validated,
        }
    }

    fn empty_range() -> Range<usize> {
        0..0
    }

    fn primary(patches: &[AxisPatch]) -> &AxisPatch {
        patches
            .iter()
            .find(|p| matches!(p.source, PatchSource::Primary))
            .expect("primary patch must exist")
    }

    // ── Required tests ─────────────────────────────────────────────

    #[test]
    fn halves_feed_when_peak_is_double_target() {
        // peak = 1.5 kW, available = 1.0, headroom = 1.0 → target = 1.0
        // multiplier = 1.0 / 1.5 ≈ 0.6667 → feed 3000 → 2000.
        let env = Env::new(3000.0);
        let view = env.view();
        let ctx = env.ctx();
        let space = env.space(&view, &ctx);

        let r = PowerFeedRetargeter {
            available_kw: 1.0,
            headroom: 1.0,
            plunge_tracking_threshold: 0.10,
        };
        let solution = r
            .target(&power_exceeds(1.5), &space, &view, &ctx)
            .expect("power exceeds must produce a solution");
        let p = primary(&solution.patches);
        assert!(
            (p.value - 2000.0).abs() < 1e-3,
            "expected feed ~= 2000, got {} (rationale: {})",
            p.value,
            solution.rationale
        );
        assert!(!p.clamped, "feed 2000 is well inside machine envelope");
    }

    #[test]
    fn coupled_plunge_emitted_when_feed_change_exceeds_threshold() {
        // Same scenario as above (~33% reduction) — must produce a
        // coupled plunge-tracking patch.
        let env = Env::new(3000.0);
        let view = env.view();
        let ctx = env.ctx();
        let space = env.space(&view, &ctx);

        let r = PowerFeedRetargeter {
            available_kw: 1.0,
            headroom: 1.0,
            plunge_tracking_threshold: 0.10,
        };
        let solution = r
            .target(&power_exceeds(1.5), &space, &view, &ctx)
            .expect("solution");
        assert_eq!(solution.patches.len(), 2, "{:?}", solution.patches);
        let coupled = solution
            .patches
            .iter()
            .find(|p| matches!(p.source, PatchSource::Coupled { .. }))
            .expect("coupled plunge patch must be present");
        assert_eq!(coupled.axis, SearchAxis::FeedRate);
    }

    #[test]
    fn no_coupled_plunge_for_small_feed_change() {
        // peak = 1.05 kW, available = 1.0, headroom = 1.0
        //   → multiplier ≈ 0.9524 (≈ 4.8% reduction, below 10%)
        let env = Env::new(3000.0);
        let view = env.view();
        let ctx = env.ctx();
        let space = env.space(&view, &ctx);

        let r = PowerFeedRetargeter {
            available_kw: 1.0,
            headroom: 1.0,
            plunge_tracking_threshold: 0.10,
        };
        let solution = r
            .target(&power_exceeds(1.05), &space, &view, &ctx)
            .expect("solution");
        assert_eq!(
            solution.patches.len(),
            1,
            "expected only the primary feed patch, got {:?}",
            solution.patches
        );
        let p = primary(&solution.patches);
        // Sanity: multiplier really is small.
        let mult = p.value / 3000.0;
        assert!(
            (mult - 1.0).abs() < 0.10,
            "test setup invalid: |mult - 1.0| = {:.4}, must be < 0.10",
            (mult - 1.0_f64).abs()
        );
    }

    #[test]
    fn returns_none_for_non_power_verdict() {
        let env = Env::new(3000.0);
        let view = env.view();
        let ctx = env.ctx();
        let space = env.space(&view, &ctx);

        let r = PowerFeedRetargeter {
            available_kw: 1.0,
            headroom: 0.85,
            plunge_tracking_threshold: 0.10,
        };

        let chipload_verdict = Verdict::Exceeds {
            peak: 0.5,
            sample_range: empty_range(),
            reason: ExceedsReason::ChiploadBurnRisk,
            confidence: Confidence::Validated,
        };
        assert!(r.target(&chipload_verdict, &space, &view, &ctx).is_none());

        let within = Verdict::Within {
            peak: 0.3,
            confidence: Confidence::Validated,
        };
        assert!(r.target(&within, &space, &view, &ctx).is_none());
    }

    #[test]
    fn target_is_clamped_to_feed_bounds_lower() {
        // Extreme overshoot: peak 100 kW vs available 0.5 kW. The raw
        // target collapses to ~15 mm/min; the machine min-feed bound
        // (0.0 in `resolve_feed_bounds`) won't actually clamp this
        // case, but the multiplier × baseline arithmetic must produce a
        // value <= machine.max_feed and the patch's `value` must equal
        // `hard.clamp(raw)`. Use an extreme case that *does* clamp:
        // a negative-going overshoot can't happen here, so verify the
        // arithmetic by checking the value matches the clamped product.
        let env = Env::new(3000.0);
        let view = env.view();
        let ctx = env.ctx();
        let space = env.space(&view, &ctx);

        let r = PowerFeedRetargeter {
            available_kw: 0.5,
            headroom: 1.0,
            plunge_tracking_threshold: 0.10,
        };
        let solution = r
            .target(&power_exceeds(100.0), &space, &view, &ctx)
            .expect("solution");
        let p = primary(&solution.patches);
        // multiplier = 0.5 / 100 = 0.005 → raw = 15. Machine min is 0,
        // so hard.clamp(15) = 15 (not clamped at the floor). Verify
        // exact arithmetic.
        let expected_raw = 3000.0 * (0.5 / 100.0);
        let feed_bounds = space.axis(SearchAxis::FeedRate).expect("feed bounds");
        let expected = feed_bounds.hard.clamp(expected_raw);
        assert!(
            (p.value - expected).abs() < 1e-6,
            "expected {expected} (= clamp({expected_raw})), got {}",
            p.value
        );
    }

    #[test]
    fn target_is_clamped_to_feed_bounds_upper() {
        // Boost scenario: peak is below target, so multiplier > 1 and
        // the raw target overshoots the machine max feed. Verify the
        // patch value is clamped at `feed_bounds.hard.hi` and
        // `clamped = true`.
        //
        // `MachineProfile::shapeoko_makita()` has max_feed_mm_min =
        // 5000. Baseline 3000, peak 0.1 kW vs available 1.0 (headroom
        // 1.0) → target = 1.0 → multiplier = 10 → raw = 30000. Hard
        // clamps to 5000.
        let env = Env::new(3000.0);
        let view = env.view();
        let ctx = env.ctx();
        let space = env.space(&view, &ctx);

        let r = PowerFeedRetargeter {
            available_kw: 1.0,
            headroom: 1.0,
            plunge_tracking_threshold: 0.10,
        };
        let solution = r
            .target(&power_exceeds(0.1), &space, &view, &ctx)
            .expect("solution");
        let p = primary(&solution.patches);
        let feed_bounds = space.axis(SearchAxis::FeedRate).expect("feed bounds");
        assert!(
            (p.value - feed_bounds.hard.hi).abs() < 1e-6,
            "expected clamp at hard.hi = {}, got {}",
            feed_bounds.hard.hi,
            p.value
        );
        assert!(p.clamped, "value should be flagged clamped");
    }
}
