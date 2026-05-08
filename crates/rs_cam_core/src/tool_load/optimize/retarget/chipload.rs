//! Chipload retargeter — Step 5, G16.
//!
//! Translates a per-sample chipload `Verdict::Exceeds` (Burn or Breakage) into
//! a feed-rate patch. Geometry-linear math: at fixed RPM and toolpath shape,
//! per-sample chipload scales linearly with feed, so the multiplier needed to
//! land the observed peak inside the LUT envelope (with policy headroom) is
//! `target_chipload / observed_peak`. Applied to baseline feed and clamped to
//! the machine envelope.
//!
//! Plunge tracks feed when `|Δfeed/baseline| > plunge_tracking_threshold`
//! (10% by default). The plunge change is captured as a `PatchSource::Coupled`
//! marker — the apply path in `patches::apply_patches_to_op` skips the marker
//! to avoid double-applying, and the rationale builder reads it to surface the
//! coupled change in candidate explanations.
//!
//! RPM is intentionally frozen — moving RPM also changes chipload non-linearly
//! through the LUT match window, which would invalidate the linear multiplier.
//!
//! See `planning/STEP5_PREP_RETARGETERS.md` §1 for the design rationale and
//! the wanaka TP 4 acceptance trace.

use crate::tool_load::optimize::axes::{AxisContext, AxisView, SearchAxis};
use crate::tool_load::optimize::patches::{AxisPatch, PatchSource};
use crate::tool_load::optimize::retarget::{Retargeter, RetargetSolution};
use crate::tool_load::optimize::space::SearchSpace;
use crate::tool_load::verdict::{ExceedsReason, Verdict};

/// Which side of the LUT envelope the verdict landed on. Local discriminant —
/// Step 7 will replace with typed verdict variants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Side {
    Burn,
    Breakage,
}

const DRIVING_AXES: &[SearchAxis] = &[SearchAxis::FeedRate];

/// Sample-driven feed retargeter for the chipload gate.
///
/// LUT bounds are injected at construction time (the orchestrator at Step 6
/// will plumb them through from the matched LUT row). Headroom factors and the
/// plunge-tracking threshold come from `policy`.
pub struct ChiploadFeedRetargeter {
    /// Vendor LUT row's chipload floor (mm/tooth).
    pub lut_chipload_min: f64,
    /// Vendor LUT row's chipload ceiling (mm/tooth).
    pub lut_chipload_max: f64,
    /// Multiplier (>= 1.0) applied to LUT min for BurnRisk targets so we don't
    /// land exactly on the boundary. From `policy.retarget.chipload_low_headroom`.
    pub low_headroom: f64,
    /// Divisor (>= 1.0) applied to LUT max for BreakageRisk targets so we
    /// don't land exactly on the boundary. From
    /// `policy.retarget.chipload_high_headroom`.
    pub high_headroom: f64,
    /// |Δfeed/baseline| threshold above which plunge must track feed. From
    /// `policy.feed.plunge_tracking_threshold_fraction`.
    pub plunge_tracking_threshold: f64,
}

impl Retargeter for ChiploadFeedRetargeter {
    type Verdict = Verdict;

    fn driving_axes(&self) -> &'static [SearchAxis] {
        DRIVING_AXES
    }

    fn target(
        &self,
        verdict: &Verdict,
        space: &SearchSpace,
        view: &AxisView<'_>,
        ctx: &AxisContext<'_>,
    ) -> Option<RetargetSolution> {
        // Only fires on the chipload-Exceeds variants. Any other verdict
        // (Within / Unmodeled / non-chipload Exceeds) returns None so the
        // orchestrator can dispatch to a different retargeter.
        let (peak, side) = match verdict {
            Verdict::Exceeds {
                peak,
                reason: ExceedsReason::ChiploadBurnRisk,
                ..
            } => (*peak, Side::Burn),
            Verdict::Exceeds {
                peak,
                reason: ExceedsReason::ChiploadBreakageRisk,
                ..
            } => (*peak, Side::Breakage),
            _ => return None,
        };

        // Refuse non-positive peaks — division would blow up. In practice the
        // verdict pipeline filters these before dispatch, but defending here
        // keeps the math total.
        if !peak.is_finite() || peak <= 0.0 {
            return None;
        }

        // Target chipload with headroom margin: pull the peak away from the
        // boundary by `low_headroom` (above LUT min) or by `high_headroom`
        // (below LUT max).
        let target_chipload = match side {
            Side::Burn => self.lut_chipload_min * self.low_headroom,
            Side::Breakage => self.lut_chipload_max / self.high_headroom,
        };
        // The bound for the matched side may be missing (NaN sentinel from the
        // strategy when the LUT row only carries the opposite bound).
        if !target_chipload.is_finite() || target_chipload <= 0.0 {
            return None;
        }
        let multiplier = target_chipload / peak;

        // Apply the multiplier to baseline feed; clamp to the hard feed
        // envelope (machine max_feed).
        let baseline_feed = view.axis_value(SearchAxis::FeedRate, ctx)?;
        if !baseline_feed.is_finite() || baseline_feed <= 0.0 {
            return None;
        }
        let raw_target = baseline_feed * multiplier;
        let feed_bounds = space.axis(SearchAxis::FeedRate)?;
        let clamped_target = feed_bounds.hard.clamp(raw_target);
        let was_clamped = (clamped_target - raw_target).abs() > 1e-6;

        // Primary patch: feed rate set to the (possibly clamped) target.
        let mut patches = vec![AxisPatch {
            axis: SearchAxis::FeedRate,
            value: clamped_target,
            clamped: was_clamped,
            source: PatchSource::Primary,
        }];

        // Coupled plunge marker — emitted when the feed change is large
        // enough that plunge should follow. The apply path treats this as a
        // marker (see `patches::apply_patches_to_op`); the rationale builder
        // reads it to surface the coupled change in candidate explanations.
        let feed_change_fraction = (clamped_target / baseline_feed - 1.0).abs();
        if feed_change_fraction > self.plunge_tracking_threshold {
            patches.push(AxisPatch {
                axis: SearchAxis::FeedRate,
                value: clamped_target,
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
                "{side:?}: scale feed by {multiplier:.2}× to lift sample peak \
                 from {peak:.4} toward LUT × headroom"
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
    use super::*;
    use crate::compute::catalog::{OperationConfig, OptimizationSurface};
    use crate::compute::operation_configs::PocketConfig;
    use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use crate::feeds::vendor_lookup::MatchedRow;
    use crate::machine::MachineProfile;
    use crate::material::Material;
    use crate::tool::{FlatEndmill, ToolDefinition};
    use crate::tool_load::optimize::policy::SearchPolicy;
    use crate::tool_load::optimize::space::SearchSpace;
    use crate::tool_load::verdict::Confidence;

    /// Test fixture bundling everything an `AxisContext` needs to live, plus
    /// the op + view for `axis_value` resolution. Lifetimes thread through
    /// the trait method, so the fixture stores owned values and hands out
    /// borrowed views.
    struct Fixture {
        op: OperationConfig,
        machine: MachineProfile,
        material: Material,
        tool: ToolDefinition,
    }

    impl Fixture {
        fn new(baseline_feed: f64) -> Self {
            let pocket = PocketConfig {
                feed_rate: baseline_feed,
                ..PocketConfig::default()
            };
            let op = OperationConfig::Pocket(pocket);
            let machine = MachineProfile::shapeoko_makita();
            let material = Material::default();
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
                machine,
                material,
                tool,
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

        fn view(&self) -> AxisView<'_> {
            match self.op.optimization_surface() {
                OptimizationSurface::Optimizable(view) => view,
                OptimizationSurface::NotOptimizable { .. } => {
                    panic!("Pocket should be Optimizable in test fixture")
                }
            }
        }

        fn space(&self) -> SearchSpace {
            let view = self.view();
            let ctx = self.ctx();
            let policy = SearchPolicy::default();
            SearchSpace::build(&view, &ctx, None::<&MatchedRow>, &policy)
        }
    }

    fn burn_verdict(peak: f64) -> Verdict {
        Verdict::Exceeds {
            peak,
            sample_range: 0..1,
            reason: ExceedsReason::ChiploadBurnRisk,
            confidence: Confidence::Validated,
        }
    }

    fn breakage_verdict(peak: f64) -> Verdict {
        Verdict::Exceeds {
            peak,
            sample_range: 0..1,
            reason: ExceedsReason::ChiploadBreakageRisk,
            confidence: Confidence::Validated,
        }
    }

    #[test]
    fn burnrisk_doubles_feed_when_peak_is_half_lut_min() {
        // peak=0.025, lut_min=0.05, low_headroom=1.0 → target=0.05, mult=2.0
        // baseline_feed=2000 → raw_target=4000 (within machine 0..5000).
        let fx = Fixture::new(2000.0);
        let space = fx.space();
        let view = fx.view();
        let ctx = fx.ctx();
        let r = ChiploadFeedRetargeter {
            lut_chipload_min: 0.05,
            lut_chipload_max: 0.10,
            low_headroom: 1.0,
            high_headroom: 1.0,
            plunge_tracking_threshold: 0.10,
        };
        let solution = r
            .target(&burn_verdict(0.025), &space, &view, &ctx)
            .expect("BurnRisk should produce a solution");
        let primary = solution
            .patches
            .iter()
            .find(|p| matches!(p.source, PatchSource::Primary))
            .expect("primary patch must be present");
        assert_eq!(primary.axis, SearchAxis::FeedRate);
        assert!(
            (primary.value - 4000.0).abs() < 1e-6,
            "expected feed=4000, got {}",
            primary.value
        );
        assert!(!primary.clamped, "should not be clamped");
    }

    #[test]
    fn breakagerisk_halves_feed_when_peak_is_double_lut_max() {
        // peak=0.20, lut_max=0.10, high_headroom=1.0 → target=0.10, mult=0.5.
        // baseline_feed=2000 → raw_target=1000.
        let fx = Fixture::new(2000.0);
        let space = fx.space();
        let view = fx.view();
        let ctx = fx.ctx();
        let r = ChiploadFeedRetargeter {
            lut_chipload_min: 0.05,
            lut_chipload_max: 0.10,
            low_headroom: 1.0,
            high_headroom: 1.0,
            plunge_tracking_threshold: 0.10,
        };
        let solution = r
            .target(&breakage_verdict(0.20), &space, &view, &ctx)
            .expect("BreakageRisk should produce a solution");
        let primary = solution
            .patches
            .iter()
            .find(|p| matches!(p.source, PatchSource::Primary))
            .expect("primary patch must be present");
        assert!(
            (primary.value - 1000.0).abs() < 1e-6,
            "expected feed=1000, got {}",
            primary.value
        );
    }

    #[test]
    fn coupled_plunge_patch_emitted_when_feed_change_exceeds_10pct() {
        // 50% feed change → exceeds 10% threshold → coupled patch present.
        let fx = Fixture::new(2000.0);
        let space = fx.space();
        let view = fx.view();
        let ctx = fx.ctx();
        let r = ChiploadFeedRetargeter {
            lut_chipload_min: 0.05,
            lut_chipload_max: 0.10,
            low_headroom: 1.0,
            high_headroom: 1.0,
            plunge_tracking_threshold: 0.10,
        };
        let solution = r
            .target(&burn_verdict(0.025), &space, &view, &ctx)
            .expect("solution");
        assert_eq!(
            solution.patches.len(),
            2,
            "expected primary + coupled, got {:?}",
            solution.patches
        );
        let coupled_present = solution
            .patches
            .iter()
            .any(|p| matches!(p.source, PatchSource::Coupled { .. }));
        assert!(coupled_present, "coupled plunge patch missing");
    }

    #[test]
    fn no_coupled_plunge_patch_for_small_feed_change() {
        // peak=0.048, lut_min=0.05 → target=0.05, mult=0.05/0.048=1.0417
        // → feed change ~4.2%, below 10% threshold → no coupled patch.
        let fx = Fixture::new(2000.0);
        let space = fx.space();
        let view = fx.view();
        let ctx = fx.ctx();
        let r = ChiploadFeedRetargeter {
            lut_chipload_min: 0.05,
            lut_chipload_max: 0.10,
            low_headroom: 1.0,
            high_headroom: 1.0,
            plunge_tracking_threshold: 0.10,
        };
        let solution = r
            .target(&burn_verdict(0.048), &space, &view, &ctx)
            .expect("solution");
        assert_eq!(
            solution.patches.len(),
            1,
            "expected only Primary, got {:?}",
            solution.patches
        );
        assert!(matches!(solution.patches[0].source, PatchSource::Primary));
    }

    #[test]
    fn returns_none_for_non_chipload_verdict() {
        let fx = Fixture::new(2000.0);
        let space = fx.space();
        let view = fx.view();
        let ctx = fx.ctx();
        let r = ChiploadFeedRetargeter {
            lut_chipload_min: 0.05,
            lut_chipload_max: 0.10,
            low_headroom: 1.0,
            high_headroom: 1.0,
            plunge_tracking_threshold: 0.10,
        };
        let power_verdict = Verdict::Exceeds {
            peak: 0.5,
            sample_range: 0..1,
            reason: ExceedsReason::SpindlePowerExceeded,
            confidence: Confidence::Validated,
        };
        assert!(r.target(&power_verdict, &space, &view, &ctx).is_none());

        let within = Verdict::Within {
            peak: 0.05,
            confidence: Confidence::Validated,
        };
        assert!(r.target(&within, &space, &view, &ctx).is_none());
    }

    #[test]
    fn target_is_clamped_to_feed_bounds() {
        // baseline=4500, peak=0.025, lut_min=0.05 → mult=2.0 → raw=9000.
        // shapeoko_makita max_feed=5000 → clamped=5000, clamped flag=true.
        let fx = Fixture::new(4500.0);
        let space = fx.space();
        let view = fx.view();
        let ctx = fx.ctx();
        let r = ChiploadFeedRetargeter {
            lut_chipload_min: 0.05,
            lut_chipload_max: 0.10,
            low_headroom: 1.0,
            high_headroom: 1.0,
            plunge_tracking_threshold: 0.10,
        };
        let solution = r
            .target(&burn_verdict(0.025), &space, &view, &ctx)
            .expect("solution");
        let primary = solution
            .patches
            .iter()
            .find(|p| matches!(p.source, PatchSource::Primary))
            .expect("primary patch");
        assert!(
            (primary.value - 5000.0).abs() < 1e-6,
            "expected clamp at 5000, got {}",
            primary.value
        );
        assert!(primary.clamped, "clamped flag must be set when raw > hi");
    }

    /// Wanaka TP 4 acceptance arithmetic: feed=3150, peak=0.0253,
    /// LUT chipload [0.038, 0.07], low_headroom=1.20.
    /// Expected: target_chipload=0.0456, multiplier≈1.802, target_feed≈5677.
    /// Machine envelope is 5000, so the patch clamps at 5000 with clamped=true.
    /// The point of this test is to demonstrate the retargeter raises feed
    /// (the BurnRisk-correct direction) — the previous Stage F lowered it.
    #[test]
    fn wanaka_tp4_burnrisk_raises_feed() {
        let fx = Fixture::new(3150.0);
        let space = fx.space();
        let view = fx.view();
        let ctx = fx.ctx();
        let r = ChiploadFeedRetargeter {
            lut_chipload_min: 0.038,
            lut_chipload_max: 0.07,
            low_headroom: 1.20,
            high_headroom: 1.20,
            plunge_tracking_threshold: 0.10,
        };
        let solution = r
            .target(&burn_verdict(0.0253), &space, &view, &ctx)
            .expect("solution");
        let primary = solution
            .patches
            .iter()
            .find(|p| matches!(p.source, PatchSource::Primary))
            .expect("primary patch");
        // Raw target ≈ 3150 * (0.038*1.20 / 0.0253) ≈ 5677, clamps to 5000.
        assert!(
            primary.value > 3150.0,
            "feed must rise from baseline, got {}",
            primary.value
        );
        assert!(
            (primary.value - 5000.0).abs() < 1e-6,
            "expected clamp at 5000, got {}",
            primary.value
        );
        assert!(primary.clamped);
        // Feed change is large enough to trigger the plunge-tracking marker.
        assert_eq!(solution.patches.len(), 2);
    }
}
