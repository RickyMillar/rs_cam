//! Step 5 (the feed), Step 5b (the long-tool share), the feed basis after
//! the power check, and Step 7 (the machine feed ceiling).

use super::{CalcState, Chip, Cut, Feed, Speed};
use crate::feeds::sourced::{self, Basis};
use crate::feeds::{
    ArmPower, CapArm, CapArmReason, CapArms, FeedsInput, FeedsWarning, OperationFamily,
    RpmFloorSource, SpindleScaleReason, ToolGeometryHint, cap_arm_demand_kw, geometry,
    long_tool_load_share, pick_cap_arm,
};

/// The law of the feed: chip x RPM x Z, and the chip-thinning rule of the
/// op over the depth and the width.
fn feed_law(
    thinning: geometry::RadialThinning,
    chip: &Basis,
    rpm_b: &Basis,
    ap_b: &Basis,
    ae_b: &Basis,
    z_b: &Basis,
) -> Basis {
    let finish_pass = thinning == geometry::RadialThinning::FinishPass;
    let drill_cycle = thinning == geometry::RadialThinning::DrillCycle;
    let base = Basis::derived(
        "chip x RPM x Z",
        vec![chip.clone(), rpm_b.clone(), z_b.clone()],
    );
    if drill_cycle {
        base
    } else if finish_pass {
        Basis::derived(
            "finish pass: no chip thinning (Garr finish row)",
            vec![base, ae_b.clone()],
        )
    } else {
        Basis::derived(
            "radial chip thinning (Garr)",
            vec![base, ae_b.clone(), ap_b.clone()],
        )
    }
}

/// The basis of the flute count Z (P13a, review H3): a tool fact with its
/// kind. An `Unknown` count plans at the legacy row and names it.
pub(super) const fn flute_count_basis(input: &FeedsInput) -> Basis {
    Basis::tool_fact(
        sourced::ToolFactName::FluteCount,
        input.tool_facts.flute_count,
    )
}

/// The spindle power curve of the machine.
pub(super) const fn power_curve() -> Basis {
    Basis::Machine(sourced::MachineField::PowerCurve)
}

pub(super) const fn feed_cap_basis() -> Basis {
    Basis::Machine(sourced::MachineField::CuttingFeedCap)
}

/// Step 5: the feed at the chip-thinning diameter of the shipped depth.
pub(super) fn step5_feed(input: &FeedsInput, speed: &Speed, chip: &Chip, cut: &Cut) -> Feed {
    let d = input.tool_diameter;
    // --- Step 5: Feed rate ---
    //
    // SHADOW POINT (census F-3, T1.4). From here on the diameter is the
    // **chip-thinning** diameter, NOT the LUT-semantics one the chipload
    // band is de-rated by (at the shipped depth, before the rubbing floor). This is the value published as
    // `FeedsResult::effective_diameter_mm`.
    let effective_d = crate::feeds::engagement::EngagedDiameter::of_hint(
        input.tool_geometry,
        d,
        input.cut_body.max_cut_diameter_mm,
        cut.ap,
    );

    // ── Radial chip thinning: APPLIED (feeds contract P11b, L1-L3) ─────
    //
    // Chip first. The band chip is the target PEAK chip, and the feed is
    // sized so that the peak chip equals it: `fz = h_band / sin(psi)` below
    // half the engaged diameter, `fz = h_band` at or above
    // (`geometry::peak_chip_thinning_lift`, Garr). Operator ruling D11
    // (2026-11-04) reverses G-CHIPTHIN-HALFFIX (2026-08-19), which applied
    // no lift because no wood chart prints a radial condition (N-8): the
    // lift is now a model matched to Garr's printed rule, not a correction
    // read from a chart.
    //
    // L2: a finish op (`PassRole::Finish`) does not thin. Garr's finish row
    // (2.5 % D) holds a low chip on purpose (step 0 claim C13). The basis
    // says so.
    //
    // L3: no cap on the lift. The machine cutting-feed ceiling (Step 7,
    // the Q1 arms), the spindle power (Step 6) and the rubbing floor (Step
    // 9b) bound the lifted feed, and each names itself when it binds.
    //
    // The axial term of a ball at a shallow depth stays OBSERVED, NOT
    // APPLIED: the charts print no rule for a depth below 1 x D.
    // A drill cycle has no radial cut (`Radial::Axial`): no lift, and Step
    // 9c sets its feed. P11b2: the one rule (`geometry::RadialThinning`)
    // that the post-sim gate and the modulator also read.
    // P29-7: the rule is a fact of the body cut (`CutFacts::thins_chip`,
    // filled by `OperationConfig::cuts`); the family and the role are the
    // vendor row key only.
    // EXEMPT(P28-18, P28-19)
    let thinning = input.body_facts.thins_chip;
    // P23a: the lift is sized at the widest width that the planner holds at
    // the full feed (the pass ceiling `c D` of an adaptive plan), not at
    // the nominal stepover: the plan may cut that wide at this feed, and
    // the peak chip there must be the band chip. An op with no planner
    // bound reads the nominal width (`CutList::full_feed_width`).
    let radial_lift = thinning.lift(
        input.cuts.list().full_feed_width(cut.ae, d),
        effective_d.mm(),
    );
    let axial_thinning = match input.tool_geometry {
        ToolGeometryHint::Ball | ToolGeometryHint::TaperedBall { .. } => {
            geometry::axial_chip_thinning_factor_for_ball(d, effective_d.mm())
        }
        ToolGeometryHint::Flat | ToolGeometryHint::Bull { .. } | ToolGeometryHint::VBit { .. } => {
            1.0
        }
    };

    // L6 (chip first): the vendor depth table (Onsrud, Freud, Amana: 1 x D
    // full, 2 x D less 25 %, 3 x D less 50 %) no longer scales the feed or
    // the band. Step 0 shows that the table holds neither the force, the
    // chip nor the peak chip. The envelope draws it as the limit "vendor
    // depth ceiling (width not printed)" (`envelope::LimitKind::VendorDepthTable`).
    let raw_feed = speed.rpm * chip.chip_load * input.flute_count as f64 * radial_lift;
    Feed {
        effective_d,
        thinning,
        radial_lift,
        axial_thinning,
        raw_feed,
        feed: raw_feed,
        basis: feed_law(
            thinning,
            &chip.basis,
            &speed.basis,
            &cut.ap_basis,
            &cut.ae_basis,
            &flute_count_basis(input),
        ),
        ld_factor: 1.0,
        power_limited: false,
        power_factor: 1.0,
        feed_power_cut: false,
        feed_clamp_factor: 1.0,
    }
}

/// Step 5b: the long-tool (L/D) share. It is reported; the feed does not
/// take it.
pub(super) fn step5b_long_tool(s: &mut CalcState) {
    let d = s.input.tool_diameter;
    // --- Step 5b: Setup derates ---
    // The long-tool (L/D) share. Ruling R4 Q7 (2026-09-24): a long tool
    // bends more under force. The feed does NOT take the share. The warning
    // is the visible record.
    s.feed.ld_factor = if let Some(overhang) = s.input.setup.tool_overhang_mm {
        let ld_ratio = overhang / d;
        let factor = long_tool_load_share(overhang, d);
        if factor < 1.0 {
            s.warnings.push(FeedsWarning::LongToolDerate {
                stickout_mm: overhang,
                diameter_mm: d,
                ratio: ld_ratio,
                factor,
            });
        }
        factor
    } else {
        1.0
    };
}

/// The feed law over the bases after Step 6, and the power ceiling when
/// the Step 6 feed rung cut the feed.
pub(super) fn feed_basis_now(s: &CalcState) -> Basis {
    let basis = feed_law(
        s.feed.thinning,
        &s.chip.basis,
        &s.speed.basis,
        &s.cut.ap_basis,
        &s.cut.ae_basis,
        &flute_count_basis(s.input),
    );
    if s.feed.feed_power_cut {
        Basis::derived("power ceiling", vec![basis, power_curve()])
    } else {
        basis
    }
}

/// The feed basis after Step 6: the bases that the power ladder moved.
pub(super) fn feed_basis_after_power(s: &mut CalcState) {
    s.feed.basis = feed_basis_now(s);
}

/// Step 7: the machine cutting-feed ceiling and the Q1 arms.
pub(super) fn step7_feed_ceiling(s: &mut CalcState) {
    let input = s.input;
    let machine = input.machine;
    // --- Step 7: Machine feed ceiling ---
    // F4: the calculator emits CUTTING feeds — the ceiling is the cutting
    // ceiling, not the gantry travel rate.
    //
    // Q1 (feeds contract P8, operator ruling 2026-11-03; replaces ruling R4
    // Q10 at the cap): when the ceiling binds, Step 7 builds two arms at the
    // capped feed and compares them on the power curve.
    //   - Arm L, "lower rpm, same chip": the RPM comes down to
    //     `floor(ceiling / chip per rev)` (the highest the spindle runs at or
    //     below it), not under the floor (the row's `rpm_min`, or the
    //     machine's minimum RPM). This is the R4 Q10 descent.
    //   - Arm S, "same rpm, smaller chip": the RPM holds; the feed sits on
    //     the ceiling with a thinner chip.
    // The demand of each arm is `cap_arm_demand_kw` (the worst case of the
    // K7 pre-sim set of the `CutSpec`), the available power
    // `machine.power_at_rpm`. Exactly one arm inside ships; both inside
    // ships L (Q1a); neither inside ships the smaller exceedance and
    // `PowerLimited` names the limit. The R4 bisection walk between the arms
    // is deleted (Q1b): its middle RPM is a third arm the ruling does not
    // name. An arm with a chip under the rubbing floor is inside; Step 9b
    // warns (Q1c). Drill cycles keep their own RPM follow-down (Step 9c).
    // P21c (ruling Q11): THE cutting ceiling of the op
    // (`MachineProfile::op_cutting_ceiling` at the op's `feed_max_rate`),
    // the ceiling that the constant-chip XY limit of Step 5c reads.
    let op_ceiling = super::spindle::op_ceiling(input);
    let machine_cut_ceiling = op_ceiling.mm_min();
    let mut shipped_cap_arm = None;
    let rpm = s.speed.rpm;
    if s.feed.feed > machine_cut_ceiling
        && machine_cut_ceiling > 0.0
        && rpm > 0.0
        // EXEMPT(P28-26)
        && input.operation != OperationFamily::Drill
    {
        let machine_min = machine.rpm_range().0;
        let (rpm_floor, floor_source) = match s.chip.vendor_rpm_min {
            Some(v) if v.is_finite() && v > machine_min => (v, RpmFloorSource::RowRpmMin),
            _ => (machine_min, RpmFloorSource::MachineMinimum),
        };
        let chip_per_rev = s.feed.feed / rpm;
        // The RPM at which the held chip lands on the ceiling, floored to a
        // whole rev/min (the apply funnel writes an integer RPM, and a
        // rounded-up RPM would lift the feed over the ceiling), then the
        // highest RPM the spindle can run at or below it, not under the
        // floor.
        let target = (machine_cut_ceiling / chip_per_rev).floor();
        let mut rpm_l = machine.next_rpm_at_or_below(target.max(rpm_floor));
        if rpm_l < rpm_floor {
            rpm_l = machine.clamp_rpm(rpm_floor);
        }
        if rpm_l < rpm && rpm_l > 0.0 {
            let (ap, ae, effective_d) = (s.cut.ap, s.cut.ae, s.feed.effective_d);
            let arm = |which: CapArm, rpm_c: f64| {
                let feed_c = (chip_per_rev * rpm_c).min(machine_cut_ceiling);
                ArmPower {
                    arm: which,
                    rpm: rpm_c,
                    feed: feed_c,
                    demand: cap_arm_demand_kw(input, ap, ae, effective_d, rpm_c, feed_c),
                    available_kw: machine.power_at_rpm(rpm_c),
                }
            };
            let lower = arm(CapArm::LowerRpmSameChip, rpm_l);
            let same = arm(CapArm::SameRpmSmallerChip, rpm);
            let (shipped, reason) = pick_cap_arm(lower, same);
            let arms = CapArms {
                lower,
                same,
                shipped: shipped.arm,
                reason,
            };
            let rpm_from = rpm;
            s.speed.basis = Basis::derived(
                shipped.arm.law(),
                vec![s.speed.basis.clone(), feed_cap_basis(), power_curve()],
            );
            if shipped.arm == CapArm::LowerRpmSameChip {
                s.speed.spindle_scale *= rpm_l / rpm;
                s.speed.spindle_scale_reason = SpindleScaleReason::FeedCeiling;
                s.speed.rpm = rpm_l;
                s.feed.feed = chip_per_rev * s.speed.rpm;
            }
            s.feed.basis = feed_basis_now(s);
            let held = s.feed.feed <= machine_cut_ceiling * (1.0 + 1e-12);
            s.warnings.push(FeedsWarning::RpmLoweredForFeedCeiling {
                rpm_from,
                rpm_to: s.speed.rpm,
                feed_ceiling_mm_min: machine_cut_ceiling,
                rpm_floor,
                floor_source,
                held,
                arms,
            });
            // Neither arm inside: one `Warn` names the power limit (Q6: a
            // finding warns, never refuses). The Step 6 ladder may have
            // filed it already; the job keeps one.
            if reason == CapArmReason::NeitherInside
                && !s
                    .warnings
                    .iter()
                    .any(|w| matches!(w, FeedsWarning::PowerLimited { .. }))
            {
                s.feed.power_limited = true;
                s.warnings.push(FeedsWarning::PowerLimited {
                    required_kw: shipped.demand_kw().unwrap_or(0.0),
                    available_kw: shipped.available_kw,
                });
            }
            shipped_cap_arm = Some(shipped.arm);
        }
    }
    if s.feed.feed > machine_cut_ceiling {
        s.warnings.push(FeedsWarning::FeedRateClamped {
            requested: s.feed.feed,
            actual: machine_cut_ceiling,
        });
        if s.feed.feed > 0.0 {
            s.feed.feed_clamp_factor = machine_cut_ceiling / s.feed.feed;
        }
        s.feed.feed = machine_cut_ceiling;
        s.feed.basis = Basis::derived("feed cap", vec![s.feed.basis.clone(), feed_cap_basis()]);
    }
    // P16 (D9): an unstated cutting cap that binds the cut warns once.
    let cap_bound = s.warnings.iter().any(|w| {
        matches!(
            w,
            FeedsWarning::RpmLoweredForFeedCeiling { .. } | FeedsWarning::FeedRateClamped { .. }
        )
    });
    if cap_bound && op_ceiling.source() == crate::machine::CuttingCeilingSource::TravelRate {
        s.warnings.push(FeedsWarning::CuttingCapUnstated {
            travel_rate_mm_min: machine_cut_ceiling,
        });
    }
    // K2: the shipped feed names the Q1 arm (its RPM basis names it too).
    if let Some(arm) = shipped_cap_arm {
        s.feed.basis = Basis::derived(arm.law(), vec![s.feed.basis.clone(), power_curve()]);
    }
}
