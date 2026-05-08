//! Per-axis search bounds — split into hard / preferred / warm-start
//! intervals so the optimizer can probe outside the vendor envelope when
//! every candidate is sim-verified.
//!
//! Step 4 of G16. Replaces the previous single-envelope intersection
//! between baseline-multiplier and LUT bounds (the §1.3 wanaka TP 4 bug
//! where LUT ae_max=0.95 capped a search that should have explored
//! 2.5–3.0). Now the LUT row is the *preferred* envelope; warm-start is
//! a baseline-anchored multiplier sweep clamped only by hard limits;
//! `outside_preferred_probes` adds policy-controlled probes beyond the
//! vendor envelope.

use std::cmp::Ordering;

use crate::compute::catalog::{OperationConfig, OperationType};
use crate::feeds::vendor_lookup::MatchedRow;

use super::axes::{AxisContext, AxisView, SearchAxis};
use super::policy::{AxisPolicy, SearchPolicy};

/// Closed numeric interval with inclusive endpoints.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval {
    pub lo: f64,
    pub hi: f64,
}

impl Interval {
    pub fn new(lo: f64, hi: f64) -> Self {
        Self { lo, hi }
    }

    pub fn contains(&self, v: f64) -> bool {
        self.lo <= v && v <= self.hi
    }

    pub fn clamp(&self, v: f64) -> f64 {
        v.clamp(self.lo, self.hi)
    }

    pub fn intersect(&self, other: &Self) -> Option<Self> {
        let lo = self.lo.max(other.lo);
        let hi = self.hi.min(other.hi);
        if lo <= hi {
            Some(Self { lo, hi })
        } else {
            None
        }
    }

    pub fn width(&self) -> f64 {
        (self.hi - self.lo).max(0.0)
    }
}

/// Provenance for a contributing source on an [`AxisBounds`]. Multiple
/// sources may be present — the LUT row narrows the preferred interval
/// while machine envelope or policy floor narrows the hard interval.
#[derive(Debug, Clone, PartialEq)]
pub enum BoundsSource {
    /// LUT row's calibrated envelope contributed to `preferred`.
    LutPreferred {
        row_id: String,
        lo: f64,
        hi: f64,
    },
    /// Machine envelope contributed to `hard`.
    MachineEnvelope { lo: f64, hi: f64 },
    /// Policy hard floor contributed to `hard.lo`.
    HardFloor {
        floor: f64,
        source: &'static str,
    },
    /// Policy hard ceiling contributed to `hard.hi`.
    HardCeiling {
        ceiling: f64,
        source: &'static str,
    },
    /// Baseline × multipliers contributed to `warm_start`.
    BaselineMultiplier {
        mult_lo: f64,
        mult_hi: f64,
        baseline: f64,
    },
}

/// Resolved bounds for a single search axis. Search MUST stay inside
/// `hard`; `preferred` reflects the vendor LUT envelope (search prefers
/// to stay inside but may probe outside per policy); `warm_start` is the
/// routine sweep range anchored on baseline.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisBounds {
    pub axis: SearchAxis,
    pub baseline: f64,
    pub hard: Interval,
    pub preferred: Option<Interval>,
    pub warm_start: Interval,
    pub sources: Vec<BoundsSource>,
}

impl AxisBounds {
    /// Generate `n_points` warm-start grid values, always including
    /// baseline. n=3: `[lo, baseline, hi]`. n=4: insert a midpoint
    /// between baseline and hi at fraction `midpoint_weight`. Returns
    /// sorted values; caller is responsible for dedup.
    pub fn warm_start_grid(&self, n_points: usize, midpoint_weight: f64) -> Vec<f64> {
        let lo = self.warm_start.lo;
        // Hi must be at least baseline so a degenerate (lo==hi) interval
        // still produces a usable grid.
        let hi = self.warm_start.hi.max(self.baseline);
        let mut v = vec![lo, self.baseline];
        if n_points >= 4 {
            v.push(self.baseline + (hi - self.baseline) * midpoint_weight);
        }
        v.push(hi);
        v.sort_by(f64::total_cmp);
        v
    }

    /// Generate probes just outside the preferred envelope, capped at
    /// hard. Returns empty when `policy_allows` is false or `preferred`
    /// is None. Up to two probes — one each side that's strictly outside
    /// preferred and inside hard.
    pub fn outside_preferred_probes(&self, policy: &AxisPolicy) -> Vec<f64> {
        if !policy.allow_outside_preferred.value {
            return Vec::new();
        }
        let Some(pref) = self.preferred.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let above = pref.hi * policy.outside_preferred_hi_mult.value;
        if above > pref.hi && above <= self.hard.hi {
            out.push(above);
        }
        let below = pref.lo * policy.outside_preferred_lo_mult.value;
        if below < pref.lo && below >= self.hard.lo {
            out.push(below);
        }
        out.sort_by(f64::total_cmp);
        out
    }

    /// Multi-line debug summary suitable for tracing / refusal text.
    pub fn summary(&self) -> String {
        let pref = match &self.preferred {
            Some(p) => format!("[{:.4}, {:.4}]", p.lo, p.hi),
            None => "none".to_owned(),
        };
        format!(
            "{:?}: baseline={:.4}, hard=[{:.4}, {:.4}], preferred={}, warm_start=[{:.4}, {:.4}], sources={}",
            self.axis,
            self.baseline,
            self.hard.lo,
            self.hard.hi,
            pref,
            self.warm_start.lo,
            self.warm_start.hi,
            self.sources.len(),
        )
    }
}

// ── Per-axis resolvers ────────────────────────────────────────────────

/// Resolve bounds for `DepthPerPass`. `op_type` distinguishes the
/// 4-variant clearing ops (Pocket/Adaptive use `baseline_mult_hi_four_point`)
/// from the 3-variant default. Reads `ap_min_mm` / `ap_max_mm` from the
/// LUT row when present.
pub fn resolve_doc_bounds(
    baseline_doc_mm: f64,
    lut_row: Option<&MatchedRow>,
    op_type: OperationType,
    policy: &SearchPolicy,
) -> AxisBounds {
    resolve_geometry_bounds(
        SearchAxis::DepthPerPass,
        baseline_doc_mm,
        lut_row.and_then(|row| match (row.ap_min_mm, row.ap_max_mm) {
            (Some(lo), Some(hi)) => Some((lo, hi, row.observation_id.clone())),
            _ => None,
        }),
        op_type,
        &policy.axes.doc,
    )
}

/// Resolve bounds for `Stepover`. Reads `ae_min_mm` / `ae_max_mm` from
/// the LUT row when present.
pub fn resolve_stepover_bounds(
    baseline_stepover_mm: f64,
    lut_row: Option<&MatchedRow>,
    op_type: OperationType,
    policy: &SearchPolicy,
) -> AxisBounds {
    resolve_geometry_bounds(
        SearchAxis::Stepover,
        baseline_stepover_mm,
        lut_row.and_then(|row| match (row.ae_min_mm, row.ae_max_mm) {
            (Some(lo), Some(hi)) => Some((lo, hi, row.observation_id.clone())),
            _ => None,
        }),
        op_type,
        &policy.axes.stepover,
    )
}

/// Resolve bounds for `ScallopHeight`. Quality target — no LUT envelope
/// today; multiplicative-only sweep.
pub fn resolve_scallop_height_bounds(
    baseline_scallop_mm: f64,
    op_type: OperationType,
    policy: &SearchPolicy,
) -> AxisBounds {
    resolve_geometry_bounds(
        SearchAxis::ScallopHeight,
        baseline_scallop_mm,
        None,
        op_type,
        &policy.axes.scallop_height,
    )
}

/// Resolve bounds for `FeedRate`. Hard interval comes from the machine
/// envelope; preferred and warm-start are baseline-anchored. Step 4
/// scaffold — Step 5 retargeters refine via LUT chipload × rpm × flutes.
pub fn resolve_feed_bounds(
    baseline_feed_mm_min: f64,
    ctx: &AxisContext<'_>,
    _lut_row: Option<&MatchedRow>,
    _policy: &SearchPolicy,
) -> AxisBounds {
    let baseline = baseline_feed_mm_min.max(0.0);
    let mut sources = Vec::new();

    let machine_min: f64 = 0.0;
    let machine_max = ctx.machine.max_feed_mm_min.max(machine_min);
    sources.push(BoundsSource::MachineEnvelope {
        lo: machine_min,
        hi: machine_max,
    });

    // Warm-start: ±50% around baseline, clamped to machine envelope.
    // Retargeters in Step 5 will replace this with sample-driven targets;
    // for Step 4 we just need plausible defaults.
    let raw_lo = (baseline * 0.5).max(machine_min);
    let raw_hi = (baseline * 1.5).min(machine_max).max(baseline);
    sources.push(BoundsSource::BaselineMultiplier {
        mult_lo: 0.5,
        mult_hi: 1.5,
        baseline,
    });

    AxisBounds {
        axis: SearchAxis::FeedRate,
        baseline,
        hard: Interval::new(machine_min, machine_max),
        preferred: None,
        warm_start: Interval::new(raw_lo, raw_hi),
        sources,
    }
}

/// Resolve bounds for `SpindleRpm`. Hard interval is the machine RPM
/// envelope. Preferred is `[rpm_min_rpm, rpm_max_rpm]` from the LUT row
/// when both are present. Warm-start defaults to ±20% around baseline.
pub fn resolve_rpm_bounds(
    baseline_rpm: f64,
    ctx: &AxisContext<'_>,
    lut_row: Option<&MatchedRow>,
    _policy: &SearchPolicy,
) -> AxisBounds {
    let baseline = baseline_rpm.max(0.0);
    let mut sources = Vec::new();

    let (machine_min, machine_max) = ctx.machine.rpm_range();
    sources.push(BoundsSource::MachineEnvelope {
        lo: machine_min,
        hi: machine_max,
    });

    let preferred = lut_row.and_then(|row| {
        match (row.rpm_min, row.rpm_max) {
            (Some(lo), Some(hi)) if lo > 0.0 && hi >= lo => {
                sources.push(BoundsSource::LutPreferred {
                    row_id: row.observation_id.clone(),
                    lo,
                    hi,
                });
                Some(Interval::new(lo, hi))
            }
            _ => None,
        }
    });

    let raw_lo = (baseline * 0.8).max(machine_min);
    let raw_hi = (baseline * 1.2).min(machine_max).max(baseline);
    sources.push(BoundsSource::BaselineMultiplier {
        mult_lo: 0.8,
        mult_hi: 1.2,
        baseline,
    });

    AxisBounds {
        axis: SearchAxis::SpindleRpm,
        baseline,
        hard: Interval::new(machine_min, machine_max),
        preferred,
        warm_start: Interval::new(raw_lo, raw_hi),
        sources,
    }
}

/// Build all bounds an `AxisView` exposes. Used by [`super::space::SearchSpace::build`].
pub(crate) fn resolve_axis_bounds(
    axis: SearchAxis,
    view: &AxisView<'_>,
    ctx: &AxisContext<'_>,
    lut_row: Option<&MatchedRow>,
    policy: &SearchPolicy,
) -> Option<AxisBounds> {
    let baseline = view.axis_value(axis, ctx)?;
    Some(match axis {
        SearchAxis::FeedRate => resolve_feed_bounds(baseline, ctx, lut_row, policy),
        SearchAxis::SpindleRpm => resolve_rpm_bounds(baseline, ctx, lut_row, policy),
        SearchAxis::DepthPerPass => resolve_doc_bounds(baseline, lut_row, view.op_type, policy),
        SearchAxis::Stepover => resolve_stepover_bounds(baseline, lut_row, view.op_type, policy),
        SearchAxis::ScallopHeight => {
            resolve_scallop_height_bounds(baseline, view.op_type, policy)
        }
        // Reserved axes have no resolver yet.
        SearchAxis::AngularStep | SearchAxis::HelixPitch | SearchAxis::RampAngle => {
            return None;
        }
    })
}

// ── Internal: shared geometry-axis resolver (DOC, Stepover, Scallop) ──

/// Common shape for axes whose hard interval is just `[hard_floor, hard_ceiling]`
/// from policy and whose warm-start is `baseline × [mult_lo, mult_hi]`.
/// Caller passes the LUT-preferred interval when present.
fn resolve_geometry_bounds(
    axis: SearchAxis,
    baseline_raw: f64,
    lut_preferred: Option<(f64, f64, String)>,
    op_type: OperationType,
    p: &AxisPolicy,
) -> AxisBounds {
    let mut sources = Vec::new();

    let hard_lo = p.hard_floor.value;
    let hard_hi = p.hard_ceiling.value.max(hard_lo);
    sources.push(BoundsSource::HardFloor {
        floor: hard_lo,
        source: hard_floor_label(axis),
    });
    sources.push(BoundsSource::HardCeiling {
        ceiling: hard_hi,
        source: hard_ceiling_label(axis),
    });

    let baseline = baseline_raw.max(hard_lo);

    let four_variant = matches!(op_type, OperationType::Pocket | OperationType::Adaptive);
    let mult_lo = p.baseline_mult_lo.value;
    let mult_hi = if four_variant {
        p.baseline_mult_hi_four_point.value
    } else {
        p.baseline_mult_hi_three_point.value
    };
    sources.push(BoundsSource::BaselineMultiplier {
        mult_lo,
        mult_hi,
        baseline,
    });

    let preferred = lut_preferred.map(|(lo, hi, row_id)| {
        sources.push(BoundsSource::LutPreferred {
            row_id,
            lo,
            hi,
        });
        Interval::new(lo, hi)
    });

    // Warm-start: baseline × multipliers. When baseline is OUTSIDE the
    // preferred envelope, expand toward preferred so the routine sweep
    // still covers a useful range. Always clamp into hard.
    let raw_lo = (baseline * mult_lo).max(hard_lo);
    let raw_hi = (baseline * mult_hi).min(hard_hi).max(baseline);
    let warm_start = match &preferred {
        Some(pref) if !pref.contains(baseline) => {
            // Baseline is outside the vendor envelope — extend toward
            // preferred so the warm-start grid spans the gap.
            let lo = raw_lo.min(pref.lo).max(hard_lo);
            let hi = raw_hi.max(pref.hi).min(hard_hi).max(baseline);
            Interval::new(lo, hi)
        }
        _ => Interval::new(raw_lo, raw_hi),
    };

    // Defensive: enforce lo <= baseline <= hi.
    let warm_start = match warm_start.lo.partial_cmp(&warm_start.hi) {
        Some(Ordering::Greater) => Interval::new(warm_start.hi, warm_start.lo),
        _ => warm_start,
    };

    AxisBounds {
        axis,
        baseline,
        hard: Interval::new(hard_lo, hard_hi),
        preferred,
        warm_start,
        sources,
    }
}

const fn hard_floor_label(axis: SearchAxis) -> &'static str {
    match axis {
        SearchAxis::DepthPerPass => "policy.axes.doc.hard_floor",
        SearchAxis::Stepover => "policy.axes.stepover.hard_floor",
        SearchAxis::ScallopHeight => "policy.axes.scallop_height.hard_floor",
        SearchAxis::FeedRate => "machine.min_feed_rate",
        SearchAxis::SpindleRpm => "machine.min_spindle_rpm",
        SearchAxis::AngularStep | SearchAxis::HelixPitch | SearchAxis::RampAngle => {
            "reserved axis hard floor"
        }
    }
}

const fn hard_ceiling_label(axis: SearchAxis) -> &'static str {
    match axis {
        SearchAxis::DepthPerPass => "policy.axes.doc.hard_ceiling",
        SearchAxis::Stepover => "policy.axes.stepover.hard_ceiling",
        SearchAxis::ScallopHeight => "policy.axes.scallop_height.hard_ceiling",
        SearchAxis::FeedRate => "machine.max_feed_rate",
        SearchAxis::SpindleRpm => "machine.max_spindle_rpm",
        SearchAxis::AngularStep | SearchAxis::HelixPitch | SearchAxis::RampAngle => {
            "reserved axis hard ceiling"
        }
    }
}

/// Factory-default anchor: the operation's out-of-the-box value for an
/// axis. Used by the variant builders to ensure operators with baselines
/// far from the well-tested defaults still get a candidate near the
/// canonical setup. Returns None when the default is unavailable, equal
/// to the dedup-tolerance neighbourhood of baseline, or below the hard
/// floor.
pub(crate) fn factory_default_for_axis(
    axis: SearchAxis,
    op_type: OperationType,
    policy: &SearchPolicy,
) -> Option<f64> {
    let default_op = OperationConfig::new_default(op_type);
    let (val, hard_floor) = match axis {
        SearchAxis::DepthPerPass => {
            (default_op.depth_per_pass()?, policy.axes.doc.hard_floor.value)
        }
        SearchAxis::Stepover => (default_op.stepover()?, policy.axes.stepover.hard_floor.value),
        SearchAxis::ScallopHeight => (
            default_op.scallop_height()?,
            policy.axes.scallop_height.hard_floor.value,
        ),
        _ => return None,
    };
    if val.is_finite() && val > hard_floor {
        Some(val)
    } else {
        None
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
    use crate::feeds::vendor_lookup::MatchedRow;

    fn synthetic_lut_row(
        ap: Option<(f64, f64)>,
        ae: Option<(f64, f64)>,
        rpm: Option<(f64, f64)>,
    ) -> MatchedRow {
        MatchedRow {
            chip_load_mm: 0.10,
            chip_load_min_mm: Some(0.05),
            chip_load_max_mm: Some(0.20),
            rpm_nominal: rpm.map(|(lo, hi)| (lo + hi) / 2.0),
            rpm_min: rpm.map(|x| x.0),
            rpm_max: rpm.map(|x| x.1),
            ap_min_mm: ap.map(|x| x.0),
            ap_max_mm: ap.map(|x| x.1),
            ae_min_mm: ae.map(|x| x.0),
            ae_max_mm: ae.map(|x| x.1),
            observation_id: "test-row".to_owned(),
            source_vendor: "Test".to_owned(),
            score: 100,
            diameter_match_score: 200,
            row_diameter_mm: 6.0,
            chipload_diameter_scale: 1.0,
            chipload_hardness_scale: 1.0,
            is_extrapolated: false,
        }
    }

    #[test]
    fn interval_intersect_overlap() {
        let a = Interval::new(1.0, 5.0);
        let b = Interval::new(3.0, 7.0);
        assert_eq!(a.intersect(&b), Some(Interval::new(3.0, 5.0)));
    }

    #[test]
    fn interval_intersect_disjoint() {
        let a = Interval::new(1.0, 2.0);
        let b = Interval::new(3.0, 4.0);
        assert_eq!(a.intersect(&b), None);
    }

    #[test]
    fn interval_clamp() {
        let i = Interval::new(1.0, 5.0);
        assert!((i.clamp(0.5) - 1.0).abs() < 1e-9);
        assert!((i.clamp(3.0) - 3.0).abs() < 1e-9);
        assert!((i.clamp(10.0) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn doc_bounds_baseline_inside_preferred_does_not_clamp_to_lut() {
        // Baseline 0.84 is inside LUT [0.4, 0.95]. Old behaviour
        // intersected mult-envelope with LUT, capping hi at 0.95. New
        // behaviour: warm_start uses baseline × multipliers only, LUT
        // becomes preferred, and probes extend beyond LUT.
        let policy = SearchPolicy::default();
        let row = synthetic_lut_row(None, Some((0.4, 0.95)), None);
        let bounds = resolve_stepover_bounds(0.84, Some(&row), OperationType::Adaptive3d, &policy);

        // Warm start uses 0.7 / 1.3 multipliers (Adaptive3d is 3-variant).
        assert!((bounds.warm_start.lo - 0.84 * 0.7).abs() < 1e-9, "{bounds:?}");
        assert!((bounds.warm_start.hi - 0.84 * 1.3).abs() < 1e-9, "{bounds:?}");

        // Preferred carries the LUT.
        let pref = bounds.preferred.expect("preferred must be set");
        assert!((pref.lo - 0.4).abs() < 1e-9);
        assert!((pref.hi - 0.95).abs() < 1e-9);

        // hard.lo is policy floor (0.05), hard.hi is ceiling (100).
        assert!(bounds.hard.lo > 0.0);
        assert!(bounds.hard.hi >= bounds.warm_start.hi);
    }

    #[test]
    fn doc_bounds_baseline_outside_preferred_expands_warm_start() {
        // Baseline 2.0, LUT [5.0, 8.0] — baseline is BELOW preferred.
        // Warm start should expand toward preferred so the grid spans
        // the gap.
        let policy = SearchPolicy::default();
        let row = synthetic_lut_row(Some((5.0, 8.0)), None, None);
        let bounds = resolve_doc_bounds(2.0, Some(&row), OperationType::Adaptive3d, &policy);

        // Warm start lo from multiplier (0.7 * 2.0 = 1.4), hi extended
        // toward preferred.hi (8.0).
        assert!(bounds.warm_start.lo <= 1.4 + 1e-9);
        assert!(bounds.warm_start.hi >= 8.0 - 1e-9);
    }

    #[test]
    fn outside_preferred_probes_returns_two_when_policy_allows() {
        let policy = SearchPolicy::default();
        let row = synthetic_lut_row(None, Some((0.4, 0.95)), None);
        let bounds = resolve_stepover_bounds(0.84, Some(&row), OperationType::Adaptive3d, &policy);

        let probes = bounds.outside_preferred_probes(&policy.axes.stepover);
        assert_eq!(probes.len(), 2, "got {probes:?}");
        // Above probe should sit above 0.95.
        assert!(probes.iter().any(|p| *p > 0.95));
        // Below probe should sit below 0.40.
        assert!(probes.iter().any(|p| *p < 0.40));
        // All probes within hard.
        for p in &probes {
            assert!(bounds.hard.contains(*p), "probe {p} outside hard");
        }
    }

    #[test]
    fn outside_preferred_probes_empty_when_no_preferred() {
        let policy = SearchPolicy::default();
        let bounds = resolve_doc_bounds(2.0, None, OperationType::Adaptive3d, &policy);
        let probes = bounds.outside_preferred_probes(&policy.axes.doc);
        assert!(probes.is_empty(), "got {probes:?}");
    }

    #[test]
    fn outside_preferred_probes_empty_when_policy_disallows() {
        let mut policy = SearchPolicy::default();
        policy.axes.stepover.allow_outside_preferred.value = false;
        let row = synthetic_lut_row(None, Some((0.4, 0.95)), None);
        let bounds = resolve_stepover_bounds(0.84, Some(&row), OperationType::Adaptive3d, &policy);
        let probes = bounds.outside_preferred_probes(&policy.axes.stepover);
        assert!(probes.is_empty(), "got {probes:?}");
    }

    #[test]
    fn warm_start_grid_three_point() {
        let policy = SearchPolicy::default();
        let bounds = resolve_doc_bounds(3.0, None, OperationType::Adaptive3d, &policy);
        let grid = bounds.warm_start_grid(3, policy.axes.doc.midpoint_weight.value);
        assert_eq!(grid.len(), 3, "got {grid:?}");
        assert!((grid[0] - 2.1).abs() < 1e-6);
        assert!((grid[1] - 3.0).abs() < 1e-6);
        assert!((grid[2] - 3.9).abs() < 1e-6);
    }

    #[test]
    fn warm_start_grid_four_point_with_midpoint() {
        let policy = SearchPolicy::default();
        let bounds = resolve_doc_bounds(1.5, None, OperationType::Pocket, &policy);
        // Pocket is 4-variant: hi multiplier = 1.4, so hi = 2.1.
        // Midpoint at 0.5 between 1.5 and 2.1 = 1.8.
        let grid = bounds.warm_start_grid(4, policy.axes.doc.midpoint_weight.value);
        assert_eq!(grid.len(), 4, "got {grid:?}");
        assert!((grid[0] - 1.05).abs() < 1e-6);
        assert!((grid[1] - 1.5).abs() < 1e-6);
        assert!((grid[2] - 1.8).abs() < 1e-6);
        assert!((grid[3] - 2.1).abs() < 1e-6);
    }

    #[test]
    fn baseline_floors_at_hard_minimum() {
        let policy = SearchPolicy::default();
        // Tiny baseline 0.01 — below hard floor 0.05 for DOC.
        let bounds = resolve_doc_bounds(0.01, None, OperationType::Pocket, &policy);
        assert!(bounds.baseline >= policy.axes.doc.hard_floor.value);
        // All warm-start values respect floor.
        for v in [bounds.warm_start.lo, bounds.warm_start.hi] {
            assert!(v >= policy.axes.doc.hard_floor.value - 1e-9);
        }
    }

    // ── Property tests ────────────────────────────────────────────

    #[test]
    fn property_hard_contains_warm_start_for_doc() {
        let policy = SearchPolicy::default();
        for baseline in [0.1_f64, 0.5, 1.0, 3.0, 10.0, 50.0] {
            for op in [OperationType::Pocket, OperationType::Adaptive3d] {
                let bounds = resolve_doc_bounds(baseline, None, op, &policy);
                assert!(
                    bounds.hard.lo <= bounds.warm_start.lo + 1e-9,
                    "hard.lo > warm_start.lo for baseline={baseline} op={op:?}: {bounds:?}"
                );
                assert!(
                    bounds.hard.hi + 1e-9 >= bounds.warm_start.hi,
                    "hard.hi < warm_start.hi for baseline={baseline} op={op:?}: {bounds:?}"
                );
            }
        }
    }

    #[test]
    fn property_baseline_inside_warm_start() {
        let policy = SearchPolicy::default();
        for baseline in [0.1_f64, 0.84, 1.0, 3.0, 10.0] {
            let bounds =
                resolve_stepover_bounds(baseline, None, OperationType::Adaptive3d, &policy);
            assert!(
                bounds.warm_start.lo <= bounds.baseline + 1e-9,
                "warm_start.lo > baseline for {baseline}: {bounds:?}"
            );
            assert!(
                bounds.warm_start.hi + 1e-9 >= bounds.baseline,
                "warm_start.hi < baseline for {baseline}: {bounds:?}"
            );
        }
    }

    #[test]
    fn property_resolvers_never_produce_non_finite() {
        let policy = SearchPolicy::default();
        for baseline in [0.001_f64, 0.84, 100.0, 10_000.0] {
            let bounds = resolve_doc_bounds(baseline, None, OperationType::Pocket, &policy);
            for v in [
                bounds.baseline,
                bounds.hard.lo,
                bounds.hard.hi,
                bounds.warm_start.lo,
                bounds.warm_start.hi,
            ] {
                assert!(v.is_finite(), "non-finite for baseline={baseline}: {v}");
            }
        }
    }

    #[test]
    fn property_preferred_when_present_within_hard() {
        let policy = SearchPolicy::default();
        let row = synthetic_lut_row(Some((1.0, 5.0)), Some((0.5, 2.0)), None);
        for op in [
            OperationType::Pocket,
            OperationType::Adaptive,
            OperationType::Adaptive3d,
        ] {
            let bounds = resolve_doc_bounds(2.0, Some(&row), op, &policy);
            let pref = bounds.preferred.expect("preferred must be set");
            assert!(bounds.hard.lo <= pref.lo + 1e-9, "{bounds:?}");
            assert!(bounds.hard.hi + 1e-9 >= pref.hi, "{bounds:?}");
        }
    }

    #[test]
    fn factory_default_anchor_recovered_for_low_baseline() {
        // The 'default brings search back to safe envelope' property.
        let policy = SearchPolicy::default();
        let anchor =
            factory_default_for_axis(SearchAxis::Stepover, OperationType::Adaptive3d, &policy);
        assert!(anchor.is_some());
        // Adaptive3d default stepover is 2.0mm.
        let v = anchor.unwrap();
        assert!((v - 2.0).abs() < 0.5, "expected ~2.0, got {v}");
    }
}
