//! Feeds contract P21c: Step 5c of the calculator, the constant-chip RPM.
//!
//! [`step5c_constant_chip`] is the one RPM-choice door (P21a). Under
//! [`SpindleStrategy::ConstantChip`] the chip per tooth holds and the RPM
//! rises to the first named limit ([`RpmLimit`]). The feed scales with the
//! RPM. No multiplier enters (`P21_DESIGN.md` section 2).
//!
//! The step runs after Step 5, because each limit reads the chip per rev
//! `c = fz x Z x lift`, and before Step 6, because the power check reads
//! the RPM.

use super::feed::{feed_cap_basis, power_curve};
use super::{CalcState, StepLog};
use crate::feeds::sourced::{self, Basis};
use crate::feeds::{
    FeedsInput, OperationFamily, RpmChoice, RpmLimit, RpmLimitValue, SpindleScaleReason,
    SpindleStrategy, cap_arm_demand_kw, constant_chip_gap,
};
use crate::machine::CuttingCeilingMmMin;

/// THE cutting ceiling of the op (ruling Q11): Step 5c, Step 7 and the
/// drill envelope clamp of Step 9c read this one value.
pub(super) fn op_ceiling(input: &FeedsInput) -> CuttingCeilingMmMin {
    input.machine.op_cutting_ceiling(input.feed_max_rate)
}

/// Step 5c: the RPM that [`SpindleStrategy::ConstantChip`] chooses, with
/// its named limits. `None` under [`SpindleStrategy::MatchChart`], and
/// `None` when the policy has a gap ([`constant_chip_gap`]): the Suggest
/// door refuses that input, and a raw `calculate` keeps the chart values.
///
/// Lead ruling 2026-11-10: three limits always bind (the operator ceiling,
/// the spindle maximum and the printed vendor maximum): a chart RPM above
/// one of them comes down to it, and the feed follows to hold the chip. The
/// other limits (the cutting ceiling, the Z axis rate, the power, the chart
/// RPM of an empty ceiling) only stop the rise: the power ladder (Step 6),
/// Step 7 and Step 9c own a move below the chart RPM.
///
/// With no chart RPM (the row prints none and the operator states a
/// ceiling), the RPM is the lowest of all the limits; the surface-speed
/// RPM of Step 1 is not read (ruling Q6).
pub(super) fn step5c_constant_chip(s: &mut CalcState, log: &mut StepLog) -> Option<RpmChoice> {
    let input = s.input;
    let rpm_ceiling = match input.spindle_strategy {
        SpindleStrategy::MatchChart => return None,
        SpindleStrategy::ConstantChip { rpm_ceiling } => rpm_ceiling,
    };
    if constant_chip_gap(input).is_some() {
        return None;
    }
    let before = s.speed.rpm;
    if before <= 0.0 || s.feed.raw_feed <= 0.0 {
        return None;
    }
    // The chip per rev, mm: `fz x Z x lift`. It holds over the step.
    let chip_per_rev = s.feed.raw_feed / before;
    // The RPM that the row prints, after the machine range (Step 2).
    let chart_rpm = s.chip.vendor_rpm.map(|_| before);
    let limits = named_limits(s, rpm_ceiling, chip_per_rev, chart_rpm);
    let lowest_of = |always_binds: Option<bool>| {
        limits
            .iter()
            .filter(|l| always_binds.is_none_or(|want| always_binds_limit(l.limit) == want))
            .min_by(|a, b| a.rpm.total_cmp(&b.rpm))
            .copied()
    };
    let lowest = lowest_of(None)?;
    // The limit that sets the RPM, or that stops the rise at the chart RPM.
    let (binding, limit_rpm) = match (chart_rpm, lowest_of(Some(true))) {
        // A limit that always binds is below the chart RPM, and the lowest
        // limit is one that only stops the rise: the RPM comes down to the
        // first, not to the second.
        (Some(chart), Some(hard)) if lowest.rpm < chart && hard.rpm < chart => {
            (hard.limit, hard.rpm)
        }
        (Some(chart), _) if lowest.rpm < chart => (lowest.limit, chart),
        (Some(_) | None, _) => (lowest.limit, lowest.rpm),
    };
    let machine = input.machine;
    // A whole RPM that the spindle can run, at or below the limit (as
    // Step 7 does): a rounded-up RPM would pass the limit. The chart RPM
    // is such an RPM already.
    let target = if chart_rpm.is_some_and(|chart| limit_rpm.to_bits() == chart.to_bits()) {
        limit_rpm
    } else {
        machine.next_rpm_at_or_below(limit_rpm.floor())
    };
    if target.to_bits() != before.to_bits() && target > 0.0 {
        s.speed.rpm = target;
        if chart_rpm.is_some() {
            s.speed.spindle_scale *= target / before;
        }
        s.speed.spindle_scale_reason = SpindleScaleReason::ConstantChip;
        s.speed.basis = Basis::derived(
            "constant chip to the first limit",
            vec![s.chip.basis.clone(), limit_basis(s, binding)],
        );
        s.feed.raw_feed = chip_per_rev * target;
        s.feed.feed = s.feed.raw_feed;
        s.feed.basis = super::feed::feed_basis_now(s);
    }
    log.state("step5c_constant_chip", s);
    Some(RpmChoice {
        rpm: s.speed.rpm,
        chart_rpm,
        binding,
        limits,
    })
}

/// `true` for a limit that lowers a chart RPM above it (lead ruling
/// 2026-11-10); `false` for a limit that only stops the rise.
const fn always_binds_limit(limit: RpmLimit) -> bool {
    match limit {
        RpmLimit::OperatorCeiling | RpmLimit::SpindleMax | RpmLimit::VendorRpmMax => true,
        RpmLimit::CuttingCeiling(_)
        | RpmLimit::ZAxisRate
        | RpmLimit::PowerAtRpm
        | RpmLimit::ChartRpm => false,
    }
}

/// The basis of the limit that binds.
fn limit_basis(s: &CalcState, limit: RpmLimit) -> Basis {
    match limit {
        RpmLimit::SpindleMax => Basis::Machine(sourced::MachineField::SpindleRpm),
        RpmLimit::CuttingCeiling(_) => feed_cap_basis(),
        RpmLimit::ZAxisRate => Basis::Machine(sourced::MachineField::ZAxisRate),
        RpmLimit::PowerAtRpm => power_curve(),
        RpmLimit::OperatorCeiling => Basis::stated(),
        RpmLimit::ChartRpm | RpmLimit::VendorRpmMax => Basis::Printed(sourced::RowRef(
            s.chip.vendor_source.clone().unwrap_or_default(),
        )),
    }
}

/// Every limit that applies, with the highest RPM it admits
/// (`P21_DESIGN.md` sections 2.1 to 2.6).
fn named_limits(
    s: &CalcState,
    rpm_ceiling: Option<u32>,
    chip_per_rev: f64,
    chart_rpm: Option<f64>,
) -> Vec<RpmLimitValue> {
    let input = s.input;
    let machine = input.machine;
    let (machine_min_rpm, machine_max_rpm) = machine.rpm_range();
    // 2.1: the spindle maximum.
    let mut limits = vec![RpmLimitValue {
        limit: RpmLimit::SpindleMax,
        rpm: machine_max_rpm,
    }];
    // 2.2: the op cutting ceiling over the chip per rev.
    let ceiling = op_ceiling(input);
    limits.push(RpmLimitValue {
        limit: RpmLimit::CuttingCeiling(ceiling.source()),
        rpm: ceiling.mm_min() / chip_per_rev,
    });
    // 2.3: the Z axis rate. A drill cycle only: its one feed is vertical.
    // A side op does not take it; its plunge and ramp go through
    // `VerticalRateLimit::bound` at any RPM (P20b).
    if input.operation == OperationFamily::Drill
        && let Some(z_rate) = machine.z_axis_rate_mm_min()
    {
        limits.push(RpmLimitValue {
            limit: RpmLimit::ZAxisRate,
            rpm: z_rate / chip_per_rev,
        });
    }
    // 2.5: the power at the RPM.
    if let Some(rpm) = power_limit_rpm(
        s,
        chip_per_rev,
        chart_rpm.unwrap_or(machine_min_rpm),
        machine_max_rpm,
    ) {
        limits.push(RpmLimitValue {
            limit: RpmLimit::PowerAtRpm,
            rpm,
        });
    }
    // 2.6: the operator ceiling; with none, the chart RPM.
    match (rpm_ceiling, chart_rpm) {
        (Some(ceiling_rpm), _) => limits.push(RpmLimitValue {
            limit: RpmLimit::OperatorCeiling,
            rpm: f64::from(ceiling_rpm),
        }),
        (None, Some(chart)) => limits.push(RpmLimitValue {
            limit: RpmLimit::ChartRpm,
            rpm: chart,
        }),
        // `constant_chip_gap` refuses this input (`RowPrintsNoRpm`).
        (None, None) => {}
    }
    // Ruling Q9: the RPM maximum that the row prints.
    if let Some(vendor_max) = s.chip.vendor_rpm_max
        && vendor_max.is_finite()
        && vendor_max > 0.0
    {
        limits.push(RpmLimitValue {
            limit: RpmLimit::VendorRpmMax,
            rpm: vendor_max,
        });
    }
    limits
}

/// The highest RPM that the spindle power admits at the held chip, in
/// `[from_rpm, max_rpm]`. Infinite when the power admits `max_rpm`. `None`
/// when the power of the cut is not judged before the simulation: no force
/// line, a depth known after the simulation, or a drill cycle (Step 6
/// does not act on those either).
///
/// At a held chip the required power is linear in the RPM:
/// `P_req(n) = k x n` (`P21_DESIGN.md` section 2.5; both the shear term
/// and the edge term carry one factor n). The available power is
/// `MachineProfile::power_at_rpm(n)`. An RPM is admitted when the ratio
/// `P_req / P_avail` is at or below 1, or at or below its value at
/// `from_rpm`: a rise that does not raise the ratio does not make an
/// over-power cut worse, and the Step 6 ladder then lowers the depth or
/// the width as it does at the chart RPM. On a constant-torque drive below
/// its rated RPM the ratio is constant, so the power sets no RPM there.
fn power_limit_rpm(s: &CalcState, chip_per_rev: f64, from_rpm: f64, max_rpm: f64) -> Option<f64> {
    let input = s.input;
    if input.is_axial_plunge() {
        return None;
    }
    let machine = input.machine;
    let reference_rpm = s.speed.rpm;
    let (_, required_kw) = cap_arm_demand_kw(
        input,
        s.cut.ap,
        s.cut.ae,
        s.feed.effective_d,
        reference_rpm,
        chip_per_rev * reference_rpm,
    )?;
    let k = required_kw / reference_rpm;
    if !(k.is_finite() && k > 0.0) {
        return None;
    }
    let ratio = |rpm: f64| {
        let available = machine.power_at_rpm(rpm);
        if available > 0.0 {
            k * rpm / available
        } else {
            f64::INFINITY
        }
    };
    let admitted_ratio = ratio(from_rpm).max(1.0);
    let admits = |rpm: f64| ratio(rpm) <= admitted_ratio;
    if max_rpm <= from_rpm || admits(max_rpm) {
        return Some(f64::INFINITY);
    }
    // The ratio does not fall as the RPM rises (the required power is
    // linear; the available power is flat or linear), so the highest
    // admitted RPM is a bisection.
    let (mut lo, mut hi) = (from_rpm, max_rpm);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if admits(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(lo)
}
