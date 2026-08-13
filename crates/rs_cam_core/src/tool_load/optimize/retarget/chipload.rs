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
//!
//! # The band comes off the verdict — Checkpoint P (1a), 2026-08-14
//!
//! The retargeter used to be handed the **raw** matched LUT row at
//! construction time (`MatchedRow::chip_load_{min,max}_mm` — diameter- and
//! hardness-scaled, but *not* DOC-derated) while the chipload gate judges
//! against `geometry::derate_chipload_bounds(...)` of that same row at the
//! measured peak axial DOC. Two instruments, one comparison.
//!
//! A-8 measured the consequence end to end
//! (`planning/review_2026-08-08/OPTIMIZER_ASSUMPTIONS.md` §2): the retargeter
//! emitted the **identical** feed for a 3 mm and a 20 mm single pass while the
//! gate's ceiling halved between them, so past `DOC/Ø ≈ 1.67` it was aiming at
//! a number above the bar it would be judged by and the retargeted candidate
//! came back `Exceeds(High)` — a guaranteed-rejected candidate costing a full
//! generate + simulate.
//!
//! The band the gate used is already in the retargeter's hand: it is
//! `ChiploadMetric::bounds` on the very `ChiploadVerdict` it is passed. It
//! reads that now, and nothing else. There is deliberately **no second band**
//! on this struct for the two to drift apart again.

use crate::tool_load::optimize::axes::{AxisContext, AxisView, SearchAxis};
use crate::tool_load::optimize::patches::{AxisPatch, PatchSource};
use crate::tool_load::optimize::retarget::{RetargetSolution, Retargeter};
use crate::tool_load::optimize::space::SearchSpace;
use crate::tool_load::verdict::{ChipSide, ChiploadVerdict};

/// Local label for rationale strings. Mirrors `ChipSide` 1:1; kept
/// distinct so rationale stays human-readable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Side {
    Burn,
    Breakage,
}

const DRIVING_AXES: &[SearchAxis] = &[SearchAxis::FeedRate];

/// Sample-driven feed retargeter for the chipload gate.
///
/// **Carries no band.** Checkpoint P (1a): the band is read off the
/// `ChiploadVerdict` handed to [`Retargeter::target`] — `ChiploadMetric
/// ::bounds`, the DOC-derated band the gate actually judged by. Only the
/// policy dials live here.
pub struct ChiploadFeedRetargeter {
    /// Multiplier (>= 1.0) applied to the band's floor for BurnRisk targets so
    /// we don't land exactly on the boundary. From
    /// `policy.retarget.chipload_low_headroom`.
    pub low_headroom: f64,
    /// Divisor (>= 1.0) applied to the band's ceiling for BreakageRisk targets
    /// so we don't land exactly on the boundary. From
    /// `policy.retarget.chipload_high_headroom`.
    pub high_headroom: f64,
    /// |Δfeed/baseline| threshold above which plunge must track feed. From
    /// `policy.feed.plunge_tracking_threshold_fraction`.
    pub plunge_tracking_threshold: f64,
}

impl Retargeter for ChiploadFeedRetargeter {
    type Verdict = ChiploadVerdict;

    fn driving_axes(&self) -> &'static [SearchAxis] {
        DRIVING_AXES
    }

    fn target(
        &self,
        verdict: &ChiploadVerdict,
        space: &SearchSpace,
        view: &AxisView<'_>,
        ctx: &AxisContext<'_>,
    ) -> Option<RetargetSolution> {
        // Only fires on the chipload-Exceeds variants. Within and
        // Unmodeled return None.
        //
        // Checkpoint P (1a): `bounds` travels with the peak. It is the
        // DOC-derated band this very verdict was decided against, so the
        // target below is aimed at the bar the re-simulation will judge it by.
        let (peak, side, bounds) = match verdict {
            ChiploadVerdict::Exceeds {
                side: ChipSide::Low,
                triggering,
                ..
            } => (
                triggering.observed_mm_per_tooth,
                Side::Burn,
                &triggering.bounds,
            ),
            ChiploadVerdict::Exceeds {
                side: ChipSide::High,
                triggering,
                ..
            } => (
                triggering.observed_mm_per_tooth,
                Side::Breakage,
                &triggering.bounds,
            ),
            _ => return None,
        };

        // Refuse non-positive peaks — division would blow up. In practice the
        // verdict pipeline filters these before dispatch, but defending here
        // keeps the math total.
        if !peak.is_finite() || peak <= 0.0 {
            return None;
        }

        // Target chipload with headroom margin: pull the peak away from the
        // boundary by `low_headroom` (above the band's floor) or by
        // `high_headroom` (below the band's ceiling).
        //
        // The floor is `Option` because a row may publish no minimum
        // (`ChiploadBoundPolicy::AllowHalfBand`). `None` is *unmodelled*, not
        // zero — refuse the burn retarget rather than invent a floor.
        let target_chipload = match side {
            Side::Burn => bounds.min_mm_per_tooth? * self.low_headroom,
            Side::Breakage => bounds.max_mm_per_tooth / self.high_headroom,
        };
        if !target_chipload.is_finite() || target_chipload <= 0.0 {
            return None;
        }

        // ── Checkpoint P (1b) — the same COMPARISON, not just the same band ──
        //
        // P-(1a) made the retargeter read the gate's band. This asks the
        // gate's *own predicate* whether the headroom target actually lands
        // inside it. `ChipBounds::contains` is the inclusive-with-epsilon
        // comparison this very `Exceeds` was decided by (Checkpoint K b1), so
        // retargeter and gate can no longer disagree about the edge either.
        //
        // **Report-only, deliberately — the branch is NOT taken.** Turning
        // this into a refusal or a clamp would move a measured **86 of the 235**
        // two-sided shipped LUT rows (36.6 %; 48 of them single-point rows
        // where max == min). On any row narrower than the headroom factor both
        // `min * 1.20 > max` and `max / 1.20 < min`, so BOTH headroom targets
        // fall outside the band the gate judges by — P-(1a)'s defect class
        // reached through the *headroom policy* instead of the DOC derate, on
        // a third of shipped rows. The ratio is scale-invariant (vendor
        // scaling and DOC derating each multiply both bounds by one factor),
        // so the census answers it exactly. That population is far outside
        // this wave's authorised movement, so A-8i measures it, says it in the
        // rationale the operator reads, and hands the branch to a checkpoint.
        //
        // Evidence: `planning/review_2026-08-08/artifacts/a8i/narrow_band_census.{py,txt}`.
        let headroom = match side {
            Side::Burn => self.low_headroom,
            Side::Breakage => self.high_headroom,
        };
        let target_in_band = bounds.contains(target_chipload);

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

        // F1 — RPM-down compensation when feed clamped on the burn
        // side. The feed multiplier got clipped by the machine's
        // max_feed envelope, so the *observed* chipload at the clamped
        // feed falls short of the LUT-min target. Both observed peak
        // and feed scale linearly together, so:
        //   achieved_observed = peak × (clamped_target / baseline_feed)
        // To make up the remaining shortfall via RPM (chipload ∝
        // 1/rpm at fixed feed):
        //   rpm_target = rpm_baseline × (achieved_observed / target_chipload)
        // Emitted only on the burn side — raising RPM to fix breakage
        // would be backwards (high-side trips reduce feed instead, no
        // compensation needed). Apply path snaps RPM to a real dial
        // position via `machine.clamp_rpm` downstream.
        if was_clamped
            && matches!(side, Side::Burn)
            && let Some(rpm_baseline) = view.axis_value(SearchAxis::SpindleRpm, ctx)
            && rpm_baseline > 0.0
            && let Some(rpm_bounds) = space.axis(SearchAxis::SpindleRpm)
        {
            let achieved_observed = peak * (clamped_target / baseline_feed);
            // Checkpoint P (1b): the contract's low-side comparison, not a
            // bare `<`. STATED so it is not over-claimed — this one is
            // provably a no-op on reachable inputs. `achieved / target`
            // equals `clamped_target / raw_target` exactly (both are
            // `peak * clamped / baseline` over `target = peak * raw /
            // baseline`), so `achieved` sits within the 8-ulp boundary slack
            // of `target` only when `raw_target` is within ~1e-11 of the feed
            // cap — while `was_clamped` needs `|clamped - raw| > 1e-6` to be
            // true at all. The two epsilons cannot both bind. Routed anyway:
            // "nothing re-deciding a gate writes a bare bound comparison" is
            // the contract, and an unreachable exception is still an exception.
            if achieved_observed > 0.0
                && crate::tool_load::boundary::below_low(achieved_observed, target_chipload, 0.0)
            {
                let rpm_target_raw = rpm_baseline * (achieved_observed / target_chipload);
                let rpm_clamped = rpm_bounds.hard.clamp(rpm_target_raw);
                // Only emit when the RPM target actually moves *down* —
                // a no-op patch (or worse, an upward bump from a clamp
                // hitting min_rpm) wouldn't help.
                if rpm_clamped < rpm_baseline - 1.0 {
                    patches.push(AxisPatch {
                        axis: SearchAxis::SpindleRpm,
                        value: rpm_clamped,
                        clamped: (rpm_clamped - rpm_target_raw).abs() > 1e-6,
                        source: PatchSource::Coupled {
                            from_axis: SearchAxis::FeedRate,
                            rule: "lower RPM to lift chipload when feed at machine cap",
                        },
                    });
                }
            }
        }

        let mut rationale = format!(
            "{side:?}: scale feed by {multiplier:.2}× to move sample peak \
             from {peak:.4} to {target_chipload:.4} — the gate's own \
             DOC-derated band with headroom"
        );
        if !target_in_band {
            // The gate's own predicate says this target would trip. Say so
            // where the operator reads it, rather than emitting a candidate
            // that cannot reconcile and letting the re-simulation discover it.
            rationale.push_str(&format!(
                ". WARNING: {target_chipload:.4} is OUTSIDE that band \
                 [{min}, {max:.4}] — the band is narrower than the \
                 {headroom:.2}× headroom, so no feed can both clear the \
                 headroom and stay inside the band",
                min = bounds
                    .min_mm_per_tooth
                    .map_or_else(|| "none".to_owned(), |m| format!("{m:.4}")),
                max = bounds.max_mm_per_tooth,
            ));
        }

        Some(RetargetSolution { patches, rationale })
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
    use crate::tool_load::verdict::{
        ChipBounds, ChipBoundsSource, ChiploadMetric, ChiploadStatistic, Confidence, SampleEvidence,
    };

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

    fn chip_bounds_of(min: Option<f64>, max: f64) -> ChipBounds {
        ChipBounds {
            min_mm_per_tooth: min,
            max_mm_per_tooth: max,
            source: ChipBoundsSource::VendorLut,
        }
    }

    fn chip_bounds() -> ChipBounds {
        chip_bounds_of(Some(0.05), 0.10)
    }

    /// **Checkpoint P (1a) re-pin.** These builders now take the band
    /// explicitly. Before P-(1a) the band lived on the retargeter and the
    /// verdict carried its own, so a fixture could — and several did — state
    /// two different bands for one recipe. That divergence *was* the defect
    /// (`OPTIMIZER_ASSUMPTIONS.md` §2.5); a fixture is no longer able to
    /// express it.
    fn burn_verdict(peak: f64, bounds: ChipBounds) -> ChiploadVerdict {
        ChiploadVerdict::Exceeds {
            side: ChipSide::Low,
            triggering: ChiploadMetric {
                observed_mm_per_tooth: peak,
                statistic: ChiploadStatistic::MedianLow,
                evidence: SampleEvidence::at_with_stat(0, ChiploadStatistic::MedianLow),
                bounds,
            },
            confidence: Confidence::Validated,
        }
    }

    fn breakage_verdict(peak: f64, bounds: ChipBounds) -> ChiploadVerdict {
        ChiploadVerdict::Exceeds {
            side: ChipSide::High,
            triggering: ChiploadMetric {
                observed_mm_per_tooth: peak,
                statistic: ChiploadStatistic::PeakHigh,
                evidence: SampleEvidence::at_with_stat(0, ChiploadStatistic::PeakHigh),
                bounds,
            },
            confidence: Confidence::Validated,
        }
    }

    fn retargeter(low_headroom: f64, high_headroom: f64) -> ChiploadFeedRetargeter {
        ChiploadFeedRetargeter {
            low_headroom,
            high_headroom,
            plunge_tracking_threshold: 0.10,
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
        let r = retargeter(1.0, 1.0);
        let solution = r
            .target(&burn_verdict(0.025, chip_bounds()), &space, &view, &ctx)
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
        let r = retargeter(1.0, 1.0);
        let solution = r
            .target(&breakage_verdict(0.20, chip_bounds()), &space, &view, &ctx)
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
        let r = retargeter(1.0, 1.0);
        let solution = r
            .target(&burn_verdict(0.025, chip_bounds()), &space, &view, &ctx)
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
        let r = retargeter(1.0, 1.0);
        let solution = r
            .target(&burn_verdict(0.048, chip_bounds()), &space, &view, &ctx)
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
    fn returns_none_for_non_exceeds_chipload_verdict() {
        // The retargeter's `Verdict` associated type is now
        // `ChiploadVerdict`, so non-chipload verdicts can't reach it.
        // Verify Within / Unmodeled chipload variants are no-ops.
        let fx = Fixture::new(2000.0);
        let space = fx.space();
        let view = fx.view();
        let ctx = fx.ctx();
        let r = retargeter(1.0, 1.0);
        let within = ChiploadVerdict::Within {
            approach_to_min: None,
            approach_to_max: ChiploadMetric {
                observed_mm_per_tooth: 0.05,
                statistic: ChiploadStatistic::PeakInRange,
                evidence: SampleEvidence::empty(),
                bounds: chip_bounds(),
            },
            confidence: Confidence::Validated,
            entry_spikes: Vec::new(),
            burn_advisory: None,
            ceiling_advisory: None,
        };
        assert!(r.target(&within, &space, &view, &ctx).is_none());

        let unmodeled = ChiploadVerdict::Unmodeled {
            reason: crate::tool_load::verdict::UnmodeledReason::SimulationRequired,
        };
        assert!(r.target(&unmodeled, &space, &view, &ctx).is_none());
    }

    #[test]
    fn target_is_clamped_to_feed_bounds() {
        // baseline=4500, peak=0.025, lut_min=0.05 → mult=2.0 → raw=9000.
        // shapeoko_makita max_feed=5000 → clamped=5000, clamped flag=true.
        let fx = Fixture::new(4500.0);
        let space = fx.space();
        let view = fx.view();
        let ctx = fx.ctx();
        let r = retargeter(1.0, 1.0);
        let solution = r
            .target(&burn_verdict(0.025, chip_bounds()), &space, &view, &ctx)
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
        let r = retargeter(1.20, 1.20);
        let solution = r
            .target(
                &burn_verdict(0.0253, chip_bounds_of(Some(0.038), 0.07)),
                &space,
                &view,
                &ctx,
            )
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
        // Patches: primary feed + plunge-tracking coupled marker +
        // F1 RPM-down coupled patch (feed clamped at machine cap → RPM
        // drops to lift chipload back into bounds).
        assert_eq!(
            solution.patches.len(),
            3,
            "expected 3 patches (primary feed, plunge marker, F1 RPM-down), got axes {:?}",
            solution.patches.iter().map(|p| p.axis).collect::<Vec<_>>(),
        );
    }

    /// F1 — burn-side trip with feed at machine cap should emit a
    /// coupled SpindleRpm patch that lowers RPM to bring observed
    /// chipload back to the LUT-min × headroom target. Wanaka TP 1
    /// arithmetic: baseline feed=4000 (already at shapeoko_makita's
    /// 5000 cap... actually the fixture's space caps closer to 5000).
    /// Using the same 0.0253 / 0.038 / headroom 1.20 setup as
    /// `wanaka_tp4_burnrisk_raises_feed`, the feed clamps at 5000 and
    /// the RPM-down patch should fire.
    #[test]
    fn f1_burnrisk_emits_rpm_down_patch_when_feed_clamps() {
        let fx = Fixture::new(3150.0);
        let space = fx.space();
        let view = fx.view();
        let ctx = fx.ctx();
        let r = retargeter(1.20, 1.20);
        let solution = r
            .target(
                &burn_verdict(0.0253, chip_bounds_of(Some(0.038), 0.07)),
                &space,
                &view,
                &ctx,
            )
            .expect("solution");
        let rpm_patch = solution
            .patches
            .iter()
            .find(|p| matches!(p.axis, SearchAxis::SpindleRpm))
            .expect("F1 should emit a SpindleRpm patch when feed clamps");
        // RPM target = 18000 × (achieved_observed / target_chipload)
        // achieved_observed = 0.0253 × (5000/3150) = 0.04016
        // target_chipload = 0.038 × 1.20 = 0.0456
        // rpm_target = 18000 × (0.04016 / 0.0456) = 15850
        // Then clamped against shapeoko_makita's RPM bounds (10000-30000).
        assert!(
            rpm_patch.value < 18_000.0 && rpm_patch.value > 10_000.0,
            "expected RPM in (10000, 18000), got {}",
            rpm_patch.value
        );
        assert!(
            matches!(
                rpm_patch.source,
                PatchSource::Coupled {
                    from_axis: SearchAxis::FeedRate,
                    ..
                }
            ),
            "RPM patch should be coupled to FeedRate, got {:?}",
            rpm_patch.source
        );
    }

    /// F1 — high-side (breakage) trips reduce feed instead of raising
    /// it, so the RPM-down compensation must NOT fire. Raising RPM on a
    /// breakage trip would be the wrong direction.
    #[test]
    fn f1_breakage_does_not_emit_rpm_patch() {
        // baseline=4500, peak=0.20, lut_max=0.10, headroom=1.0:
        // multiplier = (0.10/1.0)/0.20 = 0.5 → target_feed=2250 (no clamp).
        let fx = Fixture::new(4500.0);
        let space = fx.space();
        let view = fx.view();
        let ctx = fx.ctx();
        let r = retargeter(1.0, 1.0);
        let breakage = ChiploadVerdict::Exceeds {
            side: ChipSide::High,
            triggering: ChiploadMetric {
                observed_mm_per_tooth: 0.20,
                statistic: ChiploadStatistic::PeakHigh,
                evidence: SampleEvidence::empty(),
                bounds: chip_bounds(),
            },
            confidence: Confidence::Validated,
        };
        let solution = r.target(&breakage, &space, &view, &ctx).expect("solution");
        assert!(
            !solution
                .patches
                .iter()
                .any(|p| matches!(p.axis, SearchAxis::SpindleRpm)),
            "breakage retarget must not emit an RPM patch, got {:?}",
            solution.patches.iter().map(|p| p.axis).collect::<Vec<_>>()
        );
    }

    /// **Checkpoint P (1a) — the property, stated as a unit.**
    ///
    /// One retargeter, two verdicts whose bands differ by exactly the DOC
    /// derate (`doc_derating_scale(3.15) = 0.50`), same observed peak. The
    /// targets must differ by the same 0.50.
    ///
    /// Before P-(1a) this was impossible to express: the band lived on the
    /// retargeter, so one retargeter had exactly one target regardless of what
    /// the gate measured. That is the whole of A-8 §2.4's "read the retarget
    /// feed column — it is 1708.1 on both arms".
    #[test]
    fn the_band_the_retargeter_aims_at_comes_from_the_verdict() {
        let fx = Fixture::new(2000.0);
        let space = fx.space();
        let view = fx.view();
        let ctx = fx.ctx();
        let r = retargeter(1.20, 1.20);

        // Raw row band, as the gate would report it at DOC/Ø <= 1.
        let raw_band = breakage_verdict(0.20, chip_bounds_of(Some(0.032), 0.055));
        // The same row at DOC/Ø = 3.15: `derate_chipload_bounds` scales BOTH
        // bounds by 0.50, which is why the band's shape is preserved.
        let derated_band = breakage_verdict(0.20, chip_bounds_of(Some(0.016), 0.0275));

        let feed_of = |v: &ChiploadVerdict| {
            r.target(v, &space, &view, &ctx)
                .expect("breakage verdict must retarget")
                .patches
                .iter()
                .find(|p| matches!(p.source, PatchSource::Primary))
                .expect("primary patch")
                .value
        };
        let shallow = feed_of(&raw_band);
        let deep = feed_of(&derated_band);
        assert!(
            (deep / shallow - 0.5).abs() < 1e-9,
            "the retargeted feed must track the band the GATE used: shallow \
             {shallow:.4} vs deep {deep:.4} (ratio {:.6}, expected 0.5)",
            deep / shallow
        );
    }

    /// A row that publishes no floor gives a half-band verdict
    /// (`ChiploadBoundPolicy::AllowHalfBand`). Burn cannot be modelled
    /// against a band with no floor, so the retarget refuses rather than
    /// inventing one — and the high side is unaffected.
    #[test]
    fn a_half_band_verdict_refuses_the_burn_retarget_only() {
        let fx = Fixture::new(2000.0);
        let space = fx.space();
        let view = fx.view();
        let ctx = fx.ctx();
        let r = retargeter(1.20, 1.20);
        let half = chip_bounds_of(None, 0.10);
        assert!(
            r.target(&burn_verdict(0.025, half.clone()), &space, &view, &ctx)
                .is_none(),
            "no published floor means burn is unmodelled, not zero"
        );
        assert!(
            r.target(&breakage_verdict(0.20, half), &space, &view, &ctx)
                .is_some(),
            "the ceiling is still actionable on a half band"
        );
    }

    /// **Checkpoint P (1b) — the branch NOT taken, pinned as a report.**
    ///
    /// A band narrower than the headroom factor cannot host either headroom
    /// target: at `max/min = 1.111 < 1.20`, `max / 1.20 = 0.0833` falls under
    /// the floor and `min * 1.20 = 0.108` clears the ceiling. `ChipBounds
    /// ::contains` — the gate's own predicate — says so, and the retargeter
    /// now says so in the rationale.
    ///
    /// **It still emits the patch, unchanged.** That is the whole point of the
    /// slice: 86 of the 235 two-sided shipped rows are this shape, so
    /// branching here would move a third of shipped rows and is a checkpoint
    /// question, not a fix. This test pins BOTH halves — the report appears,
    /// and the number does not move.
    #[test]
    fn a_target_outside_a_narrow_band_is_reported_and_the_feed_is_unchanged() {
        let fx = Fixture::new(2000.0);
        let space = fx.space();
        let view = fx.view();
        let ctx = fx.ctx();
        let r = retargeter(1.20, 1.20);
        let narrow = chip_bounds_of(Some(0.09), 0.10);
        let solution = r
            .target(&breakage_verdict(0.20, narrow), &space, &view, &ctx)
            .expect("a narrow band still retargets — it only reports");
        assert!(
            solution.rationale.contains("OUTSIDE"),
            "the gate's own predicate rejects this target; the rationale must \
             say so. got: {}",
            solution.rationale
        );
        let primary = solution
            .patches
            .iter()
            .find(|p| matches!(p.source, PatchSource::Primary))
            .expect("primary patch");
        // target = 0.10 / 1.20 = 0.08333; multiplier = 0.08333 / 0.20;
        // feed = 2000 * that = 833.33. Bare arithmetic, unmoved by the report.
        assert!(
            (primary.value - 2000.0 * (0.10 / 1.20) / 0.20).abs() < 1e-9,
            "P-(1b) is report-only — the emitted feed must be the same number \
             it was before the check existed; got {}",
            primary.value
        );
    }

    /// The control: a band wider than the headroom hosts the target, so the
    /// rationale carries no warning. Without this, "the rationale contains
    /// OUTSIDE" above could be true of every recipe.
    #[test]
    fn a_target_inside_the_band_says_nothing_extra() {
        let fx = Fixture::new(2000.0);
        let space = fx.space();
        let view = fx.view();
        let ctx = fx.ctx();
        let r = retargeter(1.20, 1.20);
        // 0.10 / 0.05 = 2.0, comfortably wider than the 1.20 headroom.
        let solution = r
            .target(&breakage_verdict(0.20, chip_bounds()), &space, &view, &ctx)
            .expect("solution");
        assert!(
            !solution.rationale.contains("OUTSIDE"),
            "a band that hosts its own target must not be reported as narrow: \
             {}",
            solution.rationale
        );
    }
}
