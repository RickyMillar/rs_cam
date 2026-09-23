//! **Cut-metric distributions: the gate's own population, binned.**
//! Package B of `planning/sim_cut_metrics_2026-09-23/PLAN.md` (§3.1, §4 B).
//!
//! A gate verdict states one number: the peak (or the median) against a
//! bound. The operator's first question is different: "how much of the cut
//! is outside the limit?" This module answers it with a histogram of the
//! SAME samples the gate judged, weighted by cutting time.
//!
//! ## The population is the gate's population
//!
//! If the histogram and the gate read different samples, the card shows
//! mass outside the band while the badge says `Within` (§5). So each
//! population below calls the gate's own filters and per-sample functions.
//! It does not copy them:
//!
//! | Metric | Filters and per-sample value |
//! |---|---|
//! | Chipload | [`super::chipload::steady_state_samples_for_toolpath`] (cutting, `radial_woc_fraction >= 0.02`, 95 % of commanded feed), the vestigial `effective_chip_thickness_mm` predicate, [`super::display::achieved_advance_per_tooth`], [`super::locality::is_steady_state_for_gate`] |
//! | Power | [`super::power::sample_power_kw`] at [`super::effective_feed_for_sample`], then [`super::locality::is_steady_state_for_gate`] |
//! | Deflection | [`super::deflection::sample_tip_deflection_mm`] at [`super::deflection::deflection_feed_per_tooth_mm`], then [`super::locality::is_steady_state_for_gate`] |
//! | Depth of cut | the depth gate's `is_depth_sample`, then [`super::locality::is_steady_state_for_gate`]; the value is `axial_engagement_mm` |
//! | Engagement | no gate; the power gate's pre-filter (cutting, not air, arc captured), then [`super::locality::is_steady_state_for_gate`]; the value is the arc in degrees |
//!
//! The sentry `tests/histogram_population_is_the_gate_population_g_cuthist.rs`
//! holds this: the population maximum equals the gate's `display_peak`, and
//! the population size equals the gate's `population.contributing`.
//!
//! ## The bounds are the gate's bounds
//!
//! The histogram reads its bounds from the verdict
//! ([`CriterionStatus::bound`] and [`CriterionStatus::bound_source`]). It
//! adds no bound. The chipload floor is present only when the vendor row
//! publishes one ([`BoundSource::VendorChipBand`] `floor_mm_per_tooth`).
//! Engagement has no bound.
//!
//! The in-band and out-of-band shares use the gate's boundary contract
//! ([`super::boundary::exceeds_high`], [`super::boundary::below_low`]) at
//! zero tolerance. The gate verdict stays the authority on the state: a
//! non-zero `above_s` beside a `Within` verdict is possible when the
//! optimiser widened the trigger through [`super::ToleranceBands`].
//!
//! ## The weight
//!
//! Each sample carries its `segment_time_s`. The Y axis is the share of
//! cutting time (§7 answer 2), because a sample count over-weights short,
//! dense moves.
//!
//! ## What returns what
//!
//! - A criterion that does not apply to the operation
//!   ([`UnmodeledReason::NotApplicableForOp`]), and every kind this module
//!   does not bin (gantry push, the drill trio), returns `None`: no card.
//! - A criterion that is `Unmodeled` for another reason returns
//!   [`NotMeasuredReason::Unmodeled`] with the gate's reason.
//! - A criterion whose population is empty returns
//!   [`NotMeasuredReason::Vacuous`] (X-VAC). It never returns an empty
//!   histogram.

use serde::{Deserialize, Serialize};

use crate::stock::simulation_cut::SimulationCutTrace;

use super::locality::SpanLookup;
use super::verdict::{
    BoundSource, CriterionKind, CriterionStatus, GatePopulation, LoadState, PopulationUnit,
    ToolpathLoadVerdict, UnmodeledReason,
};
use super::{GateEnv, ToolpathLoadContext};

/// The number of inner bins a card draws. The two overflow bins come in
/// addition to these.
pub const DEFAULT_BIN_COUNT: usize = 24;

/// The lower percentile of the inner range. Values below it go to the
/// underflow bin, so one spike does not squash the other bins.
pub const RANGE_PERCENTILE_LOW: f64 = 0.005;

/// The upper percentile of the inner range. Values above it go to the
/// overflow bin.
pub const RANGE_PERCENTILE_HIGH: f64 = 0.995;

/// The quantity a distribution bins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "criterion")]
pub enum DistributionMetric {
    /// A gate quantity, in the gate's own unit ([`CriterionKind::unit`]).
    /// Only `Chipload`, `Power`, `Deflection` and `DepthOfCut` bin.
    Criterion(CriterionKind),
    /// The engagement arc, in degrees. No gate judges it, so it has no
    /// bound and no verdict.
    Engagement,
}

impl DistributionMetric {
    /// The unit of the binned values. For a criterion this is the gate's
    /// unit, so the histogram and the bound use one unit. Deflection is in
    /// millimetres here; a surface that shows micrometres converts both.
    #[must_use]
    pub fn unit(self) -> &'static str {
        match self {
            DistributionMetric::Criterion(kind) => kind.unit(),
            DistributionMetric::Engagement => "deg",
        }
    }
}

/// One member of a gate population.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PopulationSample {
    /// The gate's per-sample value, in [`DistributionMetric::unit`].
    pub value: f64,
    /// The sample's `segment_time_s`: its share of cutting time.
    pub weight_s: f64,
    /// The sample's `move_index`, so a surface can seek the playhead.
    pub move_index: usize,
    /// The sample's index into `SimulationCutTrace::samples`.
    pub sample_index: usize,
}

/// **A time-weighted histogram of one population.**
///
/// # Layout
///
/// `edges` has one more entry than each per-bin vector. Bin `i` covers
/// `edges[i]..=edges[i + 1]`.
///
/// - Bin 0 is the underflow bin. It holds the values below the inner
///   range. Its lower edge is the population minimum.
/// - The last bin is the overflow bin. It holds the values above the
///   inner range. Its upper edge is the population maximum.
/// - The inner bins between them divide the inner range into equal
///   widths. The inner range is the 0.5 to 99.5 percentile of the
///   population. It does not widen to include a bound: a bound far from
///   the data would squash every bar into a few bins. `floor` and
///   `ceiling` carry the bounds, and a surface that draws one outside the
///   inner range marks it at the chart edge.
///
/// An overflow bin can have zero width and zero weight. Its width is not
/// to scale with the inner bins: draw it as a fixed stub, so a spike is
/// never hidden and never stretches the axis.
///
/// # Shares
///
/// `below_s + above_s + in_band_s == total_s`, and the sum of `weights_s`
/// is `total_s`. Values that are not finite are left out of every sum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Histogram {
    /// Bin edges, ascending and finite.
    pub edges: Vec<f64>,
    /// Cutting time (s) in each bin.
    pub weights_s: Vec<f64>,
    /// Samples in each bin.
    pub counts: Vec<usize>,
    /// The `move_index` of the first sample (in trace order) in each bin.
    /// `None` for an empty bin.
    pub first_move_per_bin: Vec<Option<usize>>,
    /// The lower bound the gate has, if any. The chipload floor only.
    pub floor: Option<f64>,
    /// The upper bound the gate has, if any.
    pub ceiling: Option<f64>,
    /// Cutting time (s) below `floor`. Zero when there is no floor.
    pub below_s: f64,
    /// Cutting time (s) above `ceiling`. Zero when there is no ceiling.
    pub above_s: f64,
    /// Cutting time (s) inside the bounds, counted on its own path.
    pub in_band_s: f64,
    /// Cutting time (s) of every finite value.
    pub total_s: f64,
}

impl Histogram {
    /// Bin `samples` into `bin_count` inner bins plus two overflow bins.
    ///
    /// `floor` and `ceiling` are the gate's bounds. Pass `None` for a bound
    /// the gate does not have; this function never makes one up. A
    /// `bin_count` of zero is read as one.
    #[must_use]
    pub fn build(
        samples: &[PopulationSample],
        floor: Option<f64>,
        ceiling: Option<f64>,
        bin_count: usize,
    ) -> Self {
        let inner_bins = bin_count.max(1);
        let mut values: Vec<f64> = samples
            .iter()
            .map(|s| s.value)
            .filter(|v| v.is_finite())
            .collect();
        values.sort_by(f64::total_cmp);

        let bounds = [floor, ceiling];
        let finite_bounds = || bounds.iter().flatten().copied().filter(|b| b.is_finite());
        let (pop_min, pop_max) = match (values.first(), values.last()) {
            (Some(&lo), Some(&hi)) => (lo, hi),
            _ => {
                // No finite value. The range comes from the bounds alone,
                // or is the unit interval when there is no bound.
                let lo = finite_bounds().fold(f64::INFINITY, f64::min);
                let hi = finite_bounds().fold(f64::NEG_INFINITY, f64::max);
                if lo.is_finite() && hi.is_finite() {
                    (lo, hi)
                } else {
                    (0.0, 1.0)
                }
            }
        };
        // The inner range follows the data only. The shares below come
        // from the exact values against the bounds, so a bound outside the
        // range still sorts every sample.
        let mut lo = percentile(&values, RANGE_PERCENTILE_LOW).unwrap_or(pop_min);
        let mut hi = percentile(&values, RANGE_PERCENTILE_HIGH).unwrap_or(pop_max);
        if hi <= lo {
            // Every value is equal and no bound is apart from it. Give the
            // inner range a width so the bins do not divide by zero. This
            // is a display range, not a bound.
            let pad = if lo.abs() > 0.0 {
                lo.abs() * 0.05
            } else {
                1e-6
            };
            lo -= pad;
            hi += pad;
        }

        let inner_width = (hi - lo) / inner_bins as f64;
        let mut edges = Vec::with_capacity(inner_bins + 3);
        edges.push(pop_min.min(lo));
        for k in 0..=inner_bins {
            edges.push(if k == inner_bins {
                hi
            } else {
                lo + inner_width * k as f64
            });
        }
        edges.push(pop_max.max(hi));

        let bin_total = inner_bins + 2;
        let mut weights_s = vec![0.0; bin_total];
        let mut counts = vec![0_usize; bin_total];
        let mut first_move_per_bin: Vec<Option<usize>> = vec![None; bin_total];
        let (mut below_s, mut above_s, mut in_band_s, mut total_s) = (0.0, 0.0, 0.0, 0.0);

        for s in samples {
            if !s.value.is_finite() {
                continue;
            }
            let bin = if s.value < lo {
                0
            } else if s.value > hi {
                inner_bins + 1
            } else {
                let k = ((s.value - lo) / inner_width).floor();
                // `k` is finite and at least zero here; the cast saturates.
                1 + (k as usize).min(inner_bins - 1)
            };
            if let Some(w) = weights_s.get_mut(bin) {
                *w += s.weight_s;
            }
            if let Some(c) = counts.get_mut(bin) {
                *c += 1;
            }
            if let Some(first) = first_move_per_bin.get_mut(bin)
                && first.is_none()
            {
                *first = Some(s.move_index);
            }
            total_s += s.weight_s;
            if floor.is_some_and(|f| super::boundary::below_low(s.value, f, 0.0)) {
                below_s += s.weight_s;
            } else if ceiling.is_some_and(|c| super::boundary::exceeds_high(s.value, c, 0.0)) {
                above_s += s.weight_s;
            } else {
                in_band_s += s.weight_s;
            }
        }

        Self {
            edges,
            weights_s,
            counts,
            first_move_per_bin,
            floor,
            ceiling,
            below_s,
            above_s,
            in_band_s,
            total_s,
        }
    }

    /// The share of cutting time inside the bounds, from 0 to 1. `None`
    /// when the histogram holds no cutting time.
    #[must_use]
    pub fn in_band_share(&self) -> Option<f64> {
        (self.total_s > 0.0).then(|| self.in_band_s / self.total_s)
    }
}

/// Nearest-rank percentile of an ascending slice. `None` when it is empty.
fn percentile(sorted: &[f64], p: f64) -> Option<f64> {
    let last = sorted.len().checked_sub(1)?;
    let idx = (p.clamp(0.0, 1.0) * last as f64).round() as usize;
    sorted.get(idx.min(last)).copied()
}

/// Why a metric has no histogram, although it applies to the operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "detail")]
pub enum NotMeasuredReason {
    /// The gate did not model the criterion. The reason is the gate's own.
    Unmodeled(UnmodeledReason),
    /// The gate modelled the criterion, but no sample reached its
    /// comparison (X-VAC). Word it with [`GatePopulation::vacuity_clause`].
    Vacuous(GatePopulation),
}

/// A measured distribution and what the gate said beside it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricDistribution {
    pub metric: DistributionMetric,
    /// The gate verdict. `None` for engagement, which has no gate. The
    /// card colours its headline by this, not by the in-band share.
    pub state: Option<LoadState>,
    /// Where the bounds came from, as the gate states it. `None` when the
    /// histogram has no bound.
    pub bound_source: Option<BoundSource>,
    /// The gate's own population statement. For engagement, the population
    /// this module counted.
    pub population: GatePopulation,
    pub histogram: Histogram,
}

/// The outcome for one metric that applies to the operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "detail")]
pub enum DistributionOutcome {
    Measured(Box<MetricDistribution>),
    NotMeasured(NotMeasuredReason),
}

/// **The one public door.** Bin one metric of one toolpath.
///
/// `verdict` must be the verdict the gates returned for `ctx` and `env`
/// (for example from [`super::evaluate_toolpath`], or a cached copy of it).
/// This function reads the state and the bounds from the verdict, and it
/// reads the population from `env.sim_trace` through the gate's own
/// filters. When `verdict.toolpath_id` is not `ctx.toolpath_id`, the
/// function returns `None`.
///
/// Returns `None` when the metric has no card for this operation: the
/// criterion is `NotApplicableForOp`, or the kind is one this module does
/// not bin. See the module documentation.
#[must_use]
pub fn metric_distribution(
    metric: DistributionMetric,
    verdict: &ToolpathLoadVerdict,
    ctx: &ToolpathLoadContext<'_>,
    env: &GateEnv<'_>,
) -> Option<DistributionOutcome> {
    if verdict.toolpath_id != ctx.toolpath_id {
        tracing::warn!(
            verdict_toolpath = verdict.toolpath_id.0,
            ctx_toolpath = ctx.toolpath_id.0,
            "metric_distribution: the verdict is for a different toolpath"
        );
        return None;
    }
    match metric {
        DistributionMetric::Criterion(kind) => {
            let status = criterion_status(kind, verdict)?;
            criterion_distribution(kind, &status, ctx, env)
        }
        DistributionMetric::Engagement => engagement_distribution(ctx, env),
    }
}

/// The generic row of the criteria this module bins.
fn criterion_status(
    kind: CriterionKind,
    verdict: &ToolpathLoadVerdict,
) -> Option<CriterionStatus<'_>> {
    match kind {
        CriterionKind::Chipload => Some(verdict.chipload.as_criterion_status()),
        CriterionKind::Power => Some(verdict.power.as_criterion_status()),
        CriterionKind::Deflection => Some(verdict.deflection.as_criterion_status()),
        CriterionKind::DepthOfCut => Some(verdict.depth.as_criterion_status()),
        CriterionKind::GantryPush
        | CriterionKind::DrillChipWelding
        | CriterionKind::DrillPeckAdequacy
        | CriterionKind::DrillPlungeFeed => None,
    }
}

fn criterion_distribution(
    kind: CriterionKind,
    status: &CriterionStatus<'_>,
    ctx: &ToolpathLoadContext<'_>,
    env: &GateEnv<'_>,
) -> Option<DistributionOutcome> {
    if status.state == LoadState::Unmodeled {
        return match status.unmodeled_reason {
            Some(UnmodeledReason::NotApplicableForOp(_)) => None,
            Some(reason) => Some(DistributionOutcome::NotMeasured(
                NotMeasuredReason::Unmodeled(reason.clone()),
            )),
            // Every `Unmodeled` arm carries a reason. Fail visible, not
            // clean, if one does not.
            None => Some(DistributionOutcome::NotMeasured(
                NotMeasuredReason::Unmodeled(UnmodeledReason::NotImplemented(
                    "the gate stated no reason".to_owned(),
                )),
            )),
        };
    }
    let samples = gate_population(DistributionMetric::Criterion(kind), ctx, env);
    let population = status.population.unwrap_or_else(|| {
        let offered = offered_count(ctx, env);
        GatePopulation::new(samples.len(), offered, PopulationUnit::Samples)
    });
    if population.is_vacuous() {
        return Some(DistributionOutcome::NotMeasured(
            NotMeasuredReason::Vacuous(population),
        ));
    }
    if samples.len() != population.contributing {
        // The sentry holds these equal. A difference here means a gate
        // changed its filter and this module did not follow.
        tracing::warn!(
            criterion = kind.label(),
            histogram_samples = samples.len(),
            gate_contributing = population.contributing,
            "cut-metric histogram population differs from the gate population"
        );
    }
    let floor = match &status.bound_source {
        Some(BoundSource::VendorChipBand {
            floor_mm_per_tooth, ..
        }) => *floor_mm_per_tooth,
        _ => None,
    };
    let histogram = Histogram::build(&samples, floor, status.bound, DEFAULT_BIN_COUNT);
    Some(DistributionOutcome::Measured(Box::new(
        MetricDistribution {
            metric: DistributionMetric::Criterion(kind),
            state: Some(status.state),
            bound_source: status.bound_source.clone(),
            population,
            histogram,
        },
    )))
}

fn engagement_distribution(
    ctx: &ToolpathLoadContext<'_>,
    env: &GateEnv<'_>,
) -> Option<DistributionOutcome> {
    // The same short-circuit every milling gate takes: a drill cycle has
    // no continuous engagement.
    if ctx.operation_kind.is_drill_kinematics() {
        return None;
    }
    let Some(trace) = env.sim_trace else {
        return Some(DistributionOutcome::NotMeasured(
            NotMeasuredReason::Unmodeled(UnmodeledReason::SimulationRequired),
        ));
    };
    let any_arc_captured = trace.samples.iter().any(|s| {
        s.toolpath_id == ctx.toolpath_id
            && s.is_cutting
            && s.engagement.radial_woc_fraction >= 0.02
            && s.arc_engagement_radians.is_some()
    });
    if !any_arc_captured {
        return Some(DistributionOutcome::NotMeasured(
            NotMeasuredReason::Unmodeled(UnmodeledReason::ArcEngagementNotCaptured),
        ));
    }
    let samples = gate_population(DistributionMetric::Engagement, ctx, env);
    let population = GatePopulation::new(
        samples.len(),
        offered_count(ctx, env),
        PopulationUnit::Samples,
    );
    if population.is_vacuous() {
        return Some(DistributionOutcome::NotMeasured(
            NotMeasuredReason::Vacuous(population),
        ));
    }
    let histogram = Histogram::build(&samples, None, None, DEFAULT_BIN_COUNT);
    Some(DistributionOutcome::Measured(Box::new(
        MetricDistribution {
            metric: DistributionMetric::Engagement,
            state: None,
            bound_source: None,
            population,
            histogram,
        },
    )))
}

/// The samples this toolpath emitted: the `offered` count the power,
/// deflection and depth gates state.
fn offered_count(ctx: &ToolpathLoadContext<'_>, env: &GateEnv<'_>) -> usize {
    env.sim_trace.map_or(0, |t| {
        t.samples
            .iter()
            .filter(|s| s.toolpath_id == ctx.toolpath_id)
            .count()
    })
}

/// **The gate population of one metric, in trace order.**
///
/// Each member passed the gate's own filters, and its value is the gate's
/// own per-sample value (see the module table). This function does not
/// read the verdict: when the gate refused (`Unmodeled`), the result can
/// be empty or can hold samples the gate never judged. Use
/// [`metric_distribution`] for a surface; this function is the population
/// half, public so a sentry can compare it with the gate.
#[must_use]
pub fn gate_population(
    metric: DistributionMetric,
    ctx: &ToolpathLoadContext<'_>,
    env: &GateEnv<'_>,
) -> Vec<PopulationSample> {
    let Some(trace) = env.sim_trace else {
        return Vec::new();
    };
    let span_lookup = ctx.spans.map(SpanLookup::new);
    let lookup = span_lookup.as_ref();
    match metric {
        DistributionMetric::Criterion(CriterionKind::Chipload) => {
            chipload_population(trace, ctx, lookup)
        }
        DistributionMetric::Criterion(CriterionKind::Power) => {
            // The power gate refuses a `Custom` material and a material
            // with no primary-source Kc; so does this population.
            if matches!(ctx.material, crate::material::Material::Custom { .. }) {
                return Vec::new();
            }
            let Some(kc) = ctx.material.kc_n_per_mm2() else {
                return Vec::new();
            };
            collect(trace, ctx, lookup, |s| {
                let feed = super::effective_feed_for_sample(s, &trace.predicted_feeds);
                super::power::sample_power_kw(ctx.tool, kc, s, feed)
            })
        }
        DistributionMetric::Criterion(CriterionKind::Deflection) => {
            collect(trace, ctx, lookup, |s| {
                let fz = super::deflection::deflection_feed_per_tooth_mm(s, &trace.predicted_feeds);
                super::deflection::sample_tip_deflection_mm(ctx.tool, ctx.material, s, fz)
            })
        }
        DistributionMetric::Criterion(CriterionKind::DepthOfCut) => {
            collect(trace, ctx, lookup, |s| {
                super::depth::is_depth_sample(s).then_some(s.axial_engagement_mm)
            })
        }
        DistributionMetric::Engagement => collect(trace, ctx, lookup, |s| {
            if !s.is_cutting || s.engagement.radial_woc_fraction < 0.02 {
                return None;
            }
            s.arc_engagement_radians.map(f64::to_degrees)
        }),
        DistributionMetric::Criterion(
            CriterionKind::GantryPush
            | CriterionKind::DrillChipWelding
            | CriterionKind::DrillPeckAdequacy
            | CriterionKind::DrillPlungeFeed,
        ) => Vec::new(),
    }
}

/// Walk this toolpath's samples in trace order. Keep each sample that
/// `value` accepts and that the one shared steady-state predicate keeps.
fn collect(
    trace: &SimulationCutTrace,
    ctx: &ToolpathLoadContext<'_>,
    lookup: Option<&SpanLookup<'_>>,
    value: impl Fn(&crate::stock::simulation_cut::SimulationCutSample) -> Option<f64>,
) -> Vec<PopulationSample> {
    trace
        .samples
        .iter()
        .enumerate()
        .filter(|(_, s)| s.toolpath_id == ctx.toolpath_id)
        .filter_map(|(i, s)| {
            let v = value(s)?;
            super::locality::is_steady_state_for_gate(s, lookup).then_some(PopulationSample {
                value: v,
                weight_s: s.segment_time_s,
                move_index: s.move_index,
                sample_index: i,
            })
        })
        .collect()
}

/// The chipload gate's trip set (`burn_samples` in `chipload.rs`): the
/// steady-state feed set, less the samples with no chip model, the
/// samples with no `rpm × flutes` divisor, and the transit samples.
fn chipload_population(
    trace: &SimulationCutTrace,
    ctx: &ToolpathLoadContext<'_>,
    lookup: Option<&SpanLookup<'_>>,
) -> Vec<PopulationSample> {
    let steady = super::chipload::steady_state_samples_for_toolpath(
        trace,
        ctx.toolpath_id,
        ctx.operation_feed_rate_mm_min,
    );
    steady
        .samples
        .into_iter()
        // The gate's vestigial sample-validity predicate: a sample with no
        // chip model does not reach its comparison.
        .filter(|(_, s)| s.effective_chip_thickness_mm.is_some())
        .filter_map(|(i, s)| {
            let advance = super::display::achieved_advance_per_tooth(s, &trace.predicted_feeds)?;
            super::locality::is_steady_state_for_gate(s, lookup).then_some(PopulationSample {
                value: advance.mm(),
                weight_s: s.segment_time_s,
                move_index: s.move_index,
                sample_index: i,
            })
        })
        .collect()
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

    fn pop(values: &[f64]) -> Vec<PopulationSample> {
        values
            .iter()
            .enumerate()
            .map(|(i, &value)| PopulationSample {
                value,
                weight_s: 0.1,
                move_index: i,
                sample_index: i,
            })
            .collect()
    }

    #[test]
    fn edges_are_one_more_than_bins_and_weights_sum_to_total() {
        let h = Histogram::build(
            &pop(&[0.01, 0.02, 0.03, 0.04, 0.05]),
            Some(0.02),
            Some(0.04),
            8,
        );
        assert_eq!(h.edges.len(), h.weights_s.len() + 1);
        assert_eq!(h.weights_s.len(), 10);
        assert_eq!(h.counts.iter().sum::<usize>(), 5);
        let sum: f64 = h.weights_s.iter().sum();
        assert!((sum - h.total_s).abs() < 1e-12);
        assert!((h.below_s - 0.1).abs() < 1e-12, "0.01 is below the floor");
        assert!((h.above_s - 0.1).abs() < 1e-12, "0.05 is above the ceiling");
        assert!((h.in_band_s - 0.3).abs() < 1e-12);
        assert!(h.edges.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn the_range_follows_the_data_and_keeps_a_far_bound_apart() {
        // 2026-09-24 follow-up: the bins cover the data, not the bound. A
        // ceiling four times the peak used to put every sample in one bin.
        let h = Histogram::build(&pop(&[1.0, 1.1, 1.2]), None, Some(5.0), 4);
        assert!(*h.edges.last().unwrap() < 5.0, "the edges stop at the data");
        assert_eq!(h.ceiling, Some(5.0), "the bound stays a separate field");
        assert_eq!(h.below_s, 0.0, "no floor, so nothing is below");
        assert!((h.in_band_s - h.total_s).abs() < 1e-12);
        let occupied = h.counts.iter().filter(|&&c| c > 0).count();
        assert!(occupied >= 3, "three distinct values keep three bins");
    }

    #[test]
    fn a_bound_below_every_value_still_sorts_the_shares() {
        let h = Histogram::build(&pop(&[1.0, 1.1, 1.2]), Some(0.2), None, 4);
        assert_eq!(*h.edges.first().unwrap(), 1.0);
        assert_eq!(h.below_s, 0.0);
        let h = Histogram::build(&pop(&[1.0, 1.1, 1.2]), None, Some(0.2), 4);
        assert!((h.above_s - h.total_s).abs() < 1e-12, "every value is above");
    }

    #[test]
    fn equal_values_do_not_divide_by_zero() {
        let h = Histogram::build(&pop(&[2.0, 2.0, 2.0]), None, None, 24);
        assert!(h.edges.iter().all(|e| e.is_finite()));
        assert_eq!(h.counts.iter().sum::<usize>(), 3);
    }

    #[test]
    fn first_move_is_the_first_in_trace_order() {
        let h = Histogram::build(&pop(&[0.5, 0.5, 0.5, 1.0]), None, None, 2);
        let bin = h.counts.iter().position(|&c| c == 3).unwrap();
        assert_eq!(h.first_move_per_bin[bin], Some(0));
    }
}
