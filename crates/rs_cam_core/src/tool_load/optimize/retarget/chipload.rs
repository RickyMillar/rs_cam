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
//!
//! # A band narrower than the headroom is refused — Q-NARROW (c), 2026-08-14
//!
//! Reading the gate's band is not sufficient: the retarget then pulls the
//! target *off* that band's edge by a repo-authored headroom factor (1.20).
//! On **86 of the 235** two-sided shipped LUT rows the band is narrower than
//! that factor — 48 of them single-point rows where `max == min`, which the
//! bounds already label `ChipBoundsSource::VendorLutPointPreset` — so the
//! headroom target falls outside the band the gate judges by and the
//! candidate is rejected by construction, at the price of a full generate +
//! simulate.
//!
//! A-8i measured that and reported it in the rationale (P-(1b), report-only).
//! Q-NARROW ruled (c): **refuse, with a typed reason**
//! ([`crate::tool_load::optimize::retarget::NarrowChipBandRefusal`]) — and
//! explicitly rejected clamping the target into the band, because a clamped
//! target is a number the vendor row does not support. The refusal carries
//! the band, both headroom targets and the observed peak, so it is
//! self-explanatory on the wire and doubles as the census instrument for the
//! ledgered follow-up on the headroom policy itself (Q-NARROW (d)).

use crate::tool_load::optimize::axes::{AxisContext, AxisView, SearchAxis};
use crate::tool_load::optimize::patches::{AxisPatch, PatchSource};
use crate::tool_load::optimize::retarget::{
    NarrowChipBandRefusal, RetargetOutcome, RetargetRefusal, RetargetSolution, Retargeter,
};
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
    ) -> RetargetOutcome {
        // Only fires on the chipload-Exceeds variants. Within and
        // Unmodeled are NotApplicable.
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
            _ => return RetargetOutcome::NotApplicable,
        };

        // Non-positive peaks would blow the division up. In practice the
        // verdict pipeline filters these before dispatch, but defending here
        // keeps the math total. `NotApplicable`, not `Refused` — there is no
        // evidence to explain, the input is unmodellable.
        if !peak.is_finite() || peak <= 0.0 {
            return RetargetOutcome::NotApplicable;
        }

        // Target chipload with headroom margin: pull the peak away from the
        // boundary by `low_headroom` (above the band's floor) or by
        // `high_headroom` (below the band's ceiling).
        //
        // The floor is `Option` because a row may publish no minimum
        // (`ChiploadBoundPolicy::AllowHalfBand`). `None` is *unmodelled*, not
        // zero — refuse the burn retarget rather than invent a floor.
        let low_target = bounds.min_mm_per_tooth.map(|m| m * self.low_headroom);
        let high_target = bounds.max_mm_per_tooth / self.high_headroom;
        let target_chipload = match side {
            Side::Burn => match low_target {
                Some(t) => t,
                None => return RetargetOutcome::NotApplicable,
            },
            Side::Breakage => high_target,
        };
        if !target_chipload.is_finite() || target_chipload <= 0.0 {
            return RetargetOutcome::NotApplicable;
        }

        // ── Q-NARROW (c) — the branch A-8i measured is now TAKEN ─────────
        //
        // P-(1a) made the retargeter read the gate's band. P-(1b) asked the
        // gate's *own predicate* whether the headroom target actually lands
        // inside it — `ChipBounds::contains`, the inclusive-with-epsilon
        // comparison this very `Exceeds` was decided by (Checkpoint K b1), so
        // retargeter and gate cannot disagree about the edge either — and
        // **reported** the answer without acting on it, because acting moves
        // a measured **86 of the 235** two-sided shipped LUT rows (36.6 %; 48
        // of them single-point rows where max == min) and that is a ruling,
        // not an implementation detail.
        //
        // Q-NARROW ruled (c), 2026-08-14, BINDING: refuse, with a typed
        // reason. Option (b) — clamp the target into the band — was
        // explicitly rejected as the invent-a-number failure mode. So on a
        // band narrower than the headroom the retargeter now declines instead
        // of emitting a candidate its own gate has already rejected, and the
        // full generate + simulate that candidate would have cost is not
        // spent finding that out.
        //
        // The ratio is scale-invariant (vendor scaling and DOC derating each
        // multiply both bounds by one factor), so the census answers the
        // population exactly:
        // `planning/review_2026-08-08/artifacts/a8i/narrow_band_census.{py,txt}`.
        if !bounds.contains(target_chipload) {
            return RetargetOutcome::Refused(RetargetRefusal::ChiploadBandNarrowerThanHeadroom(
                NarrowChipBandRefusal {
                    side: match side {
                        Side::Burn => ChipSide::Low,
                        Side::Breakage => ChipSide::High,
                    },
                    band_min_mm_per_tooth: bounds.min_mm_per_tooth,
                    band_max_mm_per_tooth: bounds.max_mm_per_tooth,
                    band_source: bounds.source,
                    low_headroom: self.low_headroom,
                    high_headroom: self.high_headroom,
                    low_target_mm_per_tooth: low_target,
                    high_target_mm_per_tooth: high_target,
                    observed_mm_per_tooth: peak,
                },
            ));
        }

        let multiplier = target_chipload / peak;

        // Apply the multiplier to baseline feed; clamp to the hard feed
        // envelope (machine max_feed).
        let Some(baseline_feed) = view.axis_value(SearchAxis::FeedRate, ctx) else {
            return RetargetOutcome::NotApplicable;
        };
        if !baseline_feed.is_finite() || baseline_feed <= 0.0 {
            return RetargetOutcome::NotApplicable;
        }
        let raw_target = baseline_feed * multiplier;
        let Some(feed_bounds) = space.axis(SearchAxis::FeedRate) else {
            return RetargetOutcome::NotApplicable;
        };
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

        // Every solution that reaches here aims at a target the gate's own
        // predicate has already admitted — the out-of-band case returned a
        // typed refusal above, so this rationale can no longer be a warning
        // dressed as a plan.
        let rationale = format!(
            "{side:?}: scale feed by {multiplier:.2}× to move sample peak \
             from {peak:.4} to {target_chipload:.4} — the gate's own \
             DOC-derated band with headroom"
        );

        RetargetOutcome::Solved(RetargetSolution { patches, rationale })
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
            .solution()
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
            .solution()
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
            .solution()
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
            .solution()
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
        assert!(r.target(&within, &space, &view, &ctx).solution().is_none());

        let unmodeled = ChiploadVerdict::Unmodeled {
            reason: crate::tool_load::verdict::UnmodeledReason::SimulationRequired,
        };
        assert!(
            r.target(&unmodeled, &space, &view, &ctx)
                .solution()
                .is_none()
        );
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
            .solution()
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
            .solution()
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
            .solution()
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
        let solution = r
            .target(&breakage, &space, &view, &ctx)
            .solution()
            .expect("solution");
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
                .solution()
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
                .solution()
                .is_none(),
            "no published floor means burn is unmodelled, not zero"
        );
        assert!(
            r.target(&breakage_verdict(0.20, half), &space, &view, &ctx)
                .solution()
                .is_some(),
            "the ceiling is still actionable on a half band"
        );
    }

    /// **Q-NARROW (c) — the branch P-(1b) measured, now TAKEN.**
    ///
    /// Inverted from `a_target_outside_a_narrow_band_is_reported_and_the_feed_
    /// is_unchanged`, which pinned the pre-ruling behaviour on the identical
    /// recipe. What it asserted, and what this asserts now:
    ///
    /// | | P-(1b), report-only | Q-NARROW (c), refusal |
    /// |---|---|---|
    /// | outcome | `Solved` | `Refused` |
    /// | primary feed | 833.333 mm/min | none emitted |
    /// | operator text | rationale contains "OUTSIDE" | typed reason + explanation |
    /// | cost of finding out | full generate + simulate | zero |
    ///
    /// The band is `[0.09, 0.10]` — `max/min = 1.111 < 1.20`, so `max / 1.20
    /// = 0.0833` falls under the floor and `min × 1.20 = 0.108` clears the
    /// ceiling. Both headroom targets are outside the band; `ChipBounds
    /// ::contains`, the gate's own epsilon-inclusive predicate, is what says
    /// so — not a bare comparison written here.
    #[test]
    fn a_narrow_band_refuses_the_retarget_with_a_typed_reason() {
        let fx = Fixture::new(2000.0);
        let space = fx.space();
        let view = fx.view();
        let ctx = fx.ctx();
        let r = retargeter(1.20, 1.20);
        let narrow = chip_bounds_of(Some(0.09), 0.10);
        let answer = r.target(&breakage_verdict(0.20, narrow), &space, &view, &ctx);
        let refusal = answer
            .refusal()
            .expect("a band narrower than the headroom must refuse, not propose");
        let RetargetRefusal::ChiploadBandNarrowerThanHeadroom(detail) = refusal;
        assert_eq!(detail.side, ChipSide::High);
        assert_eq!(detail.band_min_mm_per_tooth, Some(0.09));
        assert!((detail.band_max_mm_per_tooth - 0.10).abs() < 1e-12);
        // Both headroom targets are recorded, and both are outside [0.09, 0.10]
        // — the ruling's two-sided condition, asserted from the record rather
        // than restated as prose.
        assert!((detail.high_target_mm_per_tooth - 0.10 / 1.20).abs() < 1e-12);
        assert!(
            (detail.low_target_mm_per_tooth.expect("floor published") - 0.09 * 1.20).abs() < 1e-12
        );
        assert!((detail.observed_mm_per_tooth - 0.20).abs() < 1e-12);
        assert_eq!(
            detail.refused_target_mm_per_tooth(),
            Some(detail.high_target_mm_per_tooth)
        );
        // And no patch survives: this is the whole behavioural point — the
        // 833.333 mm/min candidate the pre-ruling code emitted is gone.
        assert!(
            answer.clone().solution().is_none(),
            "a refusal must not also carry a solution"
        );
        assert_eq!(
            refusal.reason(),
            crate::tool_load::RefuseReason::ChiploadBandNarrowerThanHeadroom
        );
        let text = refusal.explanation();
        for needle in ["0.0900", "0.1000", "1.20", "0.0833"] {
            assert!(
                text.contains(needle),
                "the refusal must be self-explanatory — {needle} missing from: {text}"
            );
        }
    }

    /// **Q-NARROW (c) — the single-point row, the cleanest case.**
    ///
    /// 48 of the 86 affected shipped rows publish `max == min`: a nominal
    /// preset, which the bounds already label
    /// `ChipBoundsSource::VendorLutPointPreset`. *Any* multiplicative
    /// headroom is unsatisfiable on a point — which is the observation
    /// Q-NARROW (d) is ledgered to revisit. Both sides refuse.
    #[test]
    fn a_single_point_row_refuses_both_sides() {
        let fx = Fixture::new(2000.0);
        let space = fx.space();
        let view = fx.view();
        let ctx = fx.ctx();
        let r = retargeter(1.20, 1.20);
        let point = ChipBounds {
            min_mm_per_tooth: Some(0.05),
            max_mm_per_tooth: 0.05,
            source: ChipBoundsSource::VendorLutPointPreset,
        };
        let burn = r.target(&burn_verdict(0.02, point.clone()), &space, &view, &ctx);
        let breakage = r.target(&breakage_verdict(0.20, point), &space, &view, &ctx);
        for (label, answer, side) in [
            ("burn", &burn, ChipSide::Low),
            ("breakage", &breakage, ChipSide::High),
        ] {
            let refusal = answer
                .refusal()
                .unwrap_or_else(|| panic!("{label} side must refuse on a single-point row"));
            let RetargetRefusal::ChiploadBandNarrowerThanHeadroom(detail) = refusal;
            assert_eq!(detail.side, side);
            assert_eq!(detail.band_source, ChipBoundsSource::VendorLutPointPreset);
        }
    }

    /// The control: a band wider than the headroom hosts the target, so the
    /// retarget still happens — and lands on the **same number** it did
    /// before Q-NARROW (c). Without this, "the narrow band refuses" above
    /// could be true of every recipe.
    #[test]
    fn a_band_that_hosts_its_target_still_retargets_bit_identically() {
        let fx = Fixture::new(2000.0);
        let space = fx.space();
        let view = fx.view();
        let ctx = fx.ctx();
        let r = retargeter(1.20, 1.20);
        // 0.10 / 0.05 = 2.0, comfortably wider than the 1.20 headroom.
        let answer = r.target(&breakage_verdict(0.20, chip_bounds()), &space, &view, &ctx);
        assert!(
            answer.refusal().is_none(),
            "a band that hosts its own target must not refuse"
        );
        let solution = answer.solution().expect("solution");
        assert!(
            !solution.rationale.contains("OUTSIDE"),
            "the report-only warning is gone with the branch it belonged to: {}",
            solution.rationale
        );
        let primary = solution
            .patches
            .iter()
            .find(|p| matches!(p.source, PatchSource::Primary))
            .expect("primary patch");
        // target = 0.10 / 1.20 = 0.08333; multiplier = target / 0.20;
        // feed = 2000 × that = 833.333… — bit-identical to the pre-ruling
        // number on a band the ruling does not touch.
        assert!(
            (primary.value - 2000.0 * (0.10 / 1.20) / 0.20).abs() < 1e-9,
            "the control arm's feed must not move; got {}",
            primary.value
        );
    }
}
