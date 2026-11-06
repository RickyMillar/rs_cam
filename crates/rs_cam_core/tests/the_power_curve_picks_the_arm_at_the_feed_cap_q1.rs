//! Q1 (feeds contract P8, operator ruling 2026-11-03): at the cutting-feed
//! cap, Step 7 of `feeds::calculate` builds two arms at the capped feed,
//! "lower rpm, same chip" (L) and "same rpm, smaller chip" (S), and ships
//! the arm inside the power curve of the spindle.
//!
//! - (a) L inside, S inside: L ships (lead decision Q1a).
//! - (b) L outside, S inside: S ships and the basis names S. Before P8 the
//!   R4 Q10 descent walked up from L until the cut fitted and shipped a
//!   middle RPM (the bisection walk, deleted by Q1b).
//! - (c) neither inside: the smaller exceedance ships and one
//!   `FeedsWarning::PowerLimited` names the power limit.
//!
//! The tool is the op 1 tool of report 01 (b): an 18 mm 2F flat end mill.
//! The cut is a Pocket, whose first pass is a slot (`PowerCases` Slot), on
//! the Shapeoko VFD (constant torque below its rated RPM).
//!
//! Every expected value comes from the doors under test: arm L's RPM from
//! the unbound chip per rev and `MachineProfile::next_rpm_at_or_below`, the
//! available power from `MachineProfile::power_at_rpm`, and the demand from
//! `power_at_operating_point` (the `PowerTerms::of` door through
//! `Engagement`) on a Face that states the slot width at the arm's RPM and
//! feed.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{FaceConfig, PocketConfig};
use rs_cam_core::compute::tool_config::ToolFact;
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::engagement::PowerCase;
use rs_cam_core::feeds::sourced::Basis;
use rs_cam_core::feeds::suggest::feeds_result_for_operation;
use rs_cam_core::feeds::{
    ArmPower, CapArm, CapArmReason, CapArms, FeedsResult, FeedsWarning, SpindleStrategy,
    embedded_vendor_lut, power_at_operating_point,
};
use rs_cam_core::machine::{MachineProfile, PowerModel};
use rs_cam_core::material::{Material, WoodSpecies};

/// Report 01 (b): the 18 mm 2F flat end mill of op 1.
fn op1_tool() -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    t.diameter = 18.0;
    t.set_flute_count(ToolFact::Stated(2));
    t
}

fn pocket(ap: f64) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        depth_per_pass: Some(ap),
        stepover: Some(3.6),
        ..PocketConfig::default()
    })
}

fn result(machine: &MachineProfile, material: &Material, ap: f64) -> FeedsResult {
    feeds_result_for_operation(
        &pocket(ap),
        &op1_tool(),
        material,
        machine,
        embedded_vendor_lut(),
        SpindleStrategy::MatchChart,
    )
    .expect("an 18 mm flat pocket ships")
}

fn arms(r: &FeedsResult) -> CapArms {
    r.warnings
        .iter()
        .find_map(|w| match w {
            FeedsWarning::RpmLoweredForFeedCeiling { arms, .. } => Some(*arms),
            FeedsWarning::FeedRateClamped { .. }
            | FeedsWarning::PowerLimited { .. }
            | FeedsWarning::PowerLadderReducedCut { .. }
            | FeedsWarning::ShankTooLarge { .. }
            | FeedsWarning::DepthJudgedAfterSimulation
            | FeedsWarning::DocExceedsFlute { .. }
            | FeedsWarning::CuttingCapUnstated { .. }
            | FeedsWarning::ScallopInvalid { .. }
            | FeedsWarning::ChiploadBelowRubbingFloor { .. }
            | FeedsWarning::LongToolDerate { .. }
            | FeedsWarning::VendorRowPublishesNoChipload { .. }
            | FeedsWarning::NoVendorRowsForRoutedOperation { .. }
            | FeedsWarning::FormulaChipNoPrintedRow { .. }
            | FeedsWarning::DrillFeedClampedToEnvelope { .. } => None,
        })
        .expect("the feed cap binds, so Step 7 compares the arms")
}

fn power_limited(r: &FeedsResult) -> usize {
    r.warnings
        .iter()
        .filter(|w| matches!(w, FeedsWarning::PowerLimited { .. }))
        .count()
}

fn same(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(1.0)
}

/// The arms as the doors under test give them, independent of Step 7:
/// (rpm, feed, demand, available) for L and for S.
struct Expected {
    lower: (f64, f64, f64, f64),
    same: (f64, f64, f64, f64),
}

fn expected(machine: &MachineProfile, material: &Material, ap: f64) -> Expected {
    // The same cell with the cap out of reach: the RPM and the chip per
    // rev before Step 7.
    let mut unbound = machine.clone();
    unbound.max_feed_mm_min = 1.0e9;
    unbound.max_cutting_feed_mm_min = Some(1.0e9);
    let free = result(&unbound, material, ap);
    let cap = machine.cutting_feed_ceiling_mm_min();
    assert!(free.feed_rate_mm_min > cap, "non-vacuity: the cap binds");
    let rpm_s = free.rpm;
    let chip_per_rev = free.feed_rate_mm_min / free.rpm;
    let rpm_l = machine.next_rpm_at_or_below((cap / chip_per_rev).floor());
    let feed_l = (chip_per_rev * rpm_l).min(cap);
    let tool = op1_tool();
    let demand = |rpm: f64, feed: f64| {
        let face = OperationConfig::Face(FaceConfig {
            stepover: Some(tool.diameter),
            depth: ap,
            depth_per_pass: Some(ap),
            feed_rate: Some(feed),
            plunge_rate: Some(300.0),
            spindle_rpm: Some(rpm.round() as u32),
            ..FaceConfig::default()
        });
        let figure = power_at_operating_point(&face, &tool, material, machine, None)
            .expect("the slot face is modelled");
        assert_eq!(figure.case, PowerCase::Nominal, "a face has no slot case");
        figure.required_kw
    };
    Expected {
        lower: (
            rpm_l,
            feed_l,
            demand(rpm_l, feed_l),
            machine.power_at_rpm(rpm_l),
        ),
        same: (rpm_s, cap, demand(rpm_s, cap), machine.power_at_rpm(rpm_s)),
    }
}

fn check_arm(arm: &ArmPower, want: (f64, f64, f64, f64)) {
    let (rpm, feed, demand, available) = want;
    assert!(same(arm.rpm, rpm), "rpm {} vs {rpm}", arm.rpm);
    assert!(same(arm.feed, feed), "feed {} vs {feed}", arm.feed);
    let (case, kw) = arm.demand.expect("oak and jarrah have a force line");
    assert_eq!(case, PowerCase::Slot, "the pocket's worst case is its slot");
    assert!(same(kw, demand), "demand {kw} vs {demand}");
    assert!(
        same(arm.available_kw, available),
        "available {} vs {available}",
        arm.available_kw
    );
}

fn law(b: &Basis) -> &'static str {
    match b {
        Basis::Derived { law, .. } => law,
        Basis::Operator { .. }
        | Basis::Printed(_)
        | Basis::Claim(_)
        | Basis::Rule(_)
        | Basis::Machine(_)
        | Basis::ToolFact(_) => "",
    }
}

#[test]
fn a_both_inside_ships_the_lower_rpm_q1a() {
    let machine = MachineProfile::shapeoko_vfd();
    let material = Material::SolidWood {
        species: WoodSpecies::Jarrah,
    };
    let ap = 5.0;
    let want = expected(&machine, &material, ap);
    assert!(want.lower.2 <= want.lower.3, "precondition: L inside");
    assert!(want.same.2 <= want.same.3, "precondition: S inside");

    let r = result(&machine, &material, ap);
    let arms = arms(&r);
    check_arm(&arms.lower, want.lower);
    check_arm(&arms.same, want.same);
    assert_eq!(arms.shipped, CapArm::LowerRpmSameChip);
    assert_eq!(arms.reason, CapArmReason::BothInside);
    assert!(same(r.rpm, want.lower.0), "{} vs {}", r.rpm, want.lower.0);
    assert!(same(r.feed_rate_mm_min, want.lower.1));
    assert_eq!(law(&r.bases.rpm), CapArm::LowerRpmSameChip.law());
    assert_eq!(law(&r.bases.feed), CapArm::LowerRpmSameChip.law());
    assert_eq!(power_limited(&r), 0);
}

#[test]
fn a_lower_rpm_outside_the_curve_ships_the_same_rpm_q1() {
    let machine = MachineProfile::shapeoko_vfd();
    let material = Material::SolidWood {
        species: WoodSpecies::Jarrah,
    };
    let ap = 6.0;
    let want = expected(&machine, &material, ap);
    assert!(want.lower.2 > want.lower.3, "precondition: L outside");
    assert!(want.same.2 <= want.same.3, "precondition: S inside");

    let r = result(&machine, &material, ap);
    let arms = arms(&r);
    check_arm(&arms.lower, want.lower);
    check_arm(&arms.same, want.same);
    assert_eq!(arms.shipped, CapArm::SameRpmSmallerChip);
    assert_eq!(arms.reason, CapArmReason::OnlyInside);
    // Before P8 the walk shipped an RPM between L and S.
    assert!(same(r.rpm, want.same.0), "{} vs {}", r.rpm, want.same.0);
    assert!(same(r.feed_rate_mm_min, want.same.1));
    assert_eq!(law(&r.bases.rpm), CapArm::SameRpmSmallerChip.law());
    assert_eq!(law(&r.bases.feed), CapArm::SameRpmSmallerChip.law());
    // Step 6 may file `PowerLimited` for the feed before the cap; Step 7
    // adds none when an arm is inside.
    assert!(power_limited(&r) <= 1);
}

#[test]
fn neither_inside_ships_the_smaller_exceedance_and_warns_once_q1() {
    let mut machine = MachineProfile::shapeoko_vfd();
    let PowerModel::VfdConstantTorque {
        rated_power_kw,
        rated_rpm,
    } = machine.power
    else {
        panic!("the Shapeoko VFD is constant torque");
    };
    // A spindle with a fiftieth of the power: no arm fits.
    machine.power = PowerModel::VfdConstantTorque {
        rated_power_kw: rated_power_kw / 50.0,
        rated_rpm,
    };
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };
    let ap = 3.0;
    // P16 (review D8): with no feed that fits, power rung 4 ships the
    // rubbing-floor feed, under the travel rate of the preset. So the cap
    // sits at half of the uncapped feed, derived from the cell itself, and
    // binds again.
    let mut unbound = machine.clone();
    unbound.max_feed_mm_min = 1.0e9;
    unbound.max_cutting_feed_mm_min = Some(1.0e9);
    machine.max_feed_mm_min = 0.5 * result(&unbound, &material, ap).feed_rate_mm_min;
    let want = expected(&machine, &material, ap);
    assert!(want.lower.2 > want.lower.3, "precondition: L outside");
    assert!(want.same.2 > want.same.3, "precondition: S outside");
    let smaller = if want.same.2 - want.same.3 < want.lower.2 - want.lower.3 {
        CapArm::SameRpmSmallerChip
    } else {
        CapArm::LowerRpmSameChip
    };

    let r = result(&machine, &material, ap);
    let arms = arms(&r);
    check_arm(&arms.lower, want.lower);
    check_arm(&arms.same, want.same);
    assert_eq!(arms.reason, CapArmReason::NeitherInside);
    assert_eq!(arms.shipped, smaller);
    assert_eq!(
        power_limited(&r),
        1,
        "one Warn names the power limit: {:?}",
        r.warnings
    );
}
