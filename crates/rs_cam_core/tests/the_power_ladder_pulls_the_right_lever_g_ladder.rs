//! Sentry: **a power limit is answered by the dial that constraint responds
//! to, and never by thinning the chip** (G-LADDER, 2026-09-16).
//!
//! ## The defect this pins
//!
//! Before the ladder, every power limit scaled the feed and nothing else.
//! That is the wrong dial twice over.
//!
//! The cutting-force model is affine — `Fc/ap = Ks·h + F_edge` — so the
//! ploughing term carries no feed at all. Thinning the chip sheds only part
//! of the load, while pushing the chipload toward the rubbing floor and
//! raising the energy spent per mm³. On the motivating case the feed derate
//! came out at 0.058, a 17x cut, and the recipe was STILL over budget
//! afterwards.
//!
//! And what sheds spindle load depends on the spindle. On a constant-torque
//! VFD below its rated speed, available power falls with RPM at exactly the
//! rate required power falls, so walking the constant-chipload line changes
//! nothing: 120 % utilisation at 9 000, 6 000, 4 500 and 3 000 rpm alike. On
//! a constant-power router the same walk takes 95 % to 32 %.
//!
//! ## What is asserted
//!
//! 1. On a CONSTANT-POWER spindle the traverse fires: the RPM comes down and
//!    the chipload is held.
//! 2. On a CONSTANT-TORQUE VFD below rated it does NOT: the RPM is left
//!    alone, because moving it would buy nothing. This is the arm that must
//!    fail if anyone collapses the branch back to one case.
//! 3. While a dial above the feed can still do the job, the ladder holds the
//!    advance per tooth: it makes the cut smaller, not thinner. The ladder
//!    does keep a LAST-RESORT feed rung — shipping a recipe the spindle
//!    cannot turn is worse than a thinner chip — so the test asserts that
//!    rung did not fire on these fixtures, which is what makes the claim
//!    exact rather than merely true here by luck.
//! 4. A user-pinned depth is not reduced. Pinning a depth is a statement
//!    about the part, not about how fast to cut it.
//!
//! The literature matrix covers arm 1 through `bull_12mm_pocket_oak`, whose
//! machine routes to `generic_wood_router` (constant power). It covers arm 2
//! nowhere, because no matrix cell routes to a VFD. That is why this file
//! builds its machines directly.
//!
//! See `planning/load_model_2026-09-16/DERATE_SPEC.md` and `derate_levers.py`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::feeds::{
    FeedsInput, FeedsWarning, OperationFamily, PassRole, SetupContext, SpindleStrategy,
    ToolGeometryHint, calculate, embedded_vendor_lut,
};
use rs_cam_core::machine::{MachineProfile, PowerModel, SpindleConfig};
use rs_cam_core::material::{Material, WoodSpecies};

/// A heavy cut in a hard wood on a small spindle, so the power limit binds.
fn recipe(machine: &MachineProfile, pinned_depth: Option<f64>) -> rs_cam_core::feeds::FeedsResult {
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };
    calculate(&FeedsInput {
        tool_diameter: 12.0,
        flute_count: 4,
        flute_length: 38.0,
        shank_diameter: None,
        tool_geometry: ToolGeometryHint::Flat,
        material: &material,
        machine,
        operation: OperationFamily::Pocket,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: pinned_depth,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(embedded_vendor_lut()),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MatchChart,
    })
}

/// A variable-speed spindle over the same range, so the two machines differ
/// ONLY in their power model. Anything else would confound the comparison.
///
/// The 3 000 rpm floor is load-bearing. `generic_wood_router` floors at
/// 6 000, which is exactly where this cut's chart RPM lands, so there is no
/// room to traverse and the RPM rung could never fire — the fixture would
/// have proved nothing while looking like it passed.
fn machine_with(power: PowerModel) -> MachineProfile {
    let mut m = MachineProfile::generic_wood_router();
    m.spindle = SpindleConfig::Variable {
        min_rpm: 3_000.0,
        max_rpm: 24_000.0,
    };
    m.power = power;
    m
}

/// Measured band for this cut: at and above 0.8 kW nothing binds; between
/// about 0.35 and 0.6 kW the traverse alone closes the gap; below about
/// 0.3 kW it cannot and the depth rung takes over.
fn router(power_kw: f64) -> MachineProfile {
    machine_with(PowerModel::ConstantPower { power_kw })
}

/// Enough power that the traverse alone closes the gap.
const ROUTER_TRAVERSE_KW: f64 = 0.5;
/// Little enough that the traverse cannot, so the depth rung must act.
const ROUTER_DEPTH_KW: f64 = 0.25;

fn vfd() -> MachineProfile {
    machine_with(PowerModel::VfdConstantTorque {
        rated_power_kw: 1.5,
        rated_rpm: 24_000.0,
    })
}

fn ladder_fired(r: &rs_cam_core::feeds::FeedsResult) -> bool {
    r.warnings
        .iter()
        .any(|w| matches!(w, FeedsWarning::PowerLadderReducedCut { .. }))
}

fn rpm_moved(r: &rs_cam_core::feeds::FeedsResult) -> bool {
    r.warnings.iter().any(|w| {
        matches!(
            w,
            FeedsWarning::PowerLadderReducedCut {
                rpm_from: Some(_),
                ..
            }
        )
    })
}

fn advance_per_tooth(r: &rs_cam_core::feeds::FeedsResult, flutes: f64) -> f64 {
    if r.rpm > 0.0 {
        r.feed_rate_mm_min / (r.rpm * flutes)
    } else {
        0.0
    }
}

#[test]
fn a_constant_power_spindle_slows_down_g_ladder() {
    let r = recipe(&router(ROUTER_TRAVERSE_KW), None);
    assert!(
        ladder_fired(&r),
        "the cut was not power limited on a 0.8 kW router, so this fixture \
         proves nothing. Make the cut heavier."
    );
    assert!(
        rpm_moved(&r),
        "a constant-power spindle keeps its full power as the RPM drops, so \
         the traverse DOES shed load here and the ladder should have used it. \
         Warnings: {:?}",
        r.warnings
    );
}

#[test]
fn a_constant_torque_vfd_does_not_slow_down_g_ladder() {
    // The arm that must fail if the branch is ever collapsed to one case.
    let r = recipe(&vfd(), None);
    assert!(
        ladder_fired(&r),
        "the cut was not power limited on the VFD, so this fixture proves \
         nothing. Make the cut heavier."
    );
    assert!(
        !rpm_moved(&r),
        "the RPM was lowered on a constant-torque VFD below its rated speed. \
         Available power falls with the RPM exactly as fast as required power \
         does, so utilisation does not move and the operator lost spindle \
         speed for nothing. Warnings: {:?}",
        r.warnings
    );
}

#[test]
fn the_ladder_holds_the_advance_per_tooth_g_ladder() {
    // The defining property. The dial it replaced was the feed, and the whole
    // argument for the ladder is that thinning the chip is the wrong answer.
    for (label, machine) in [("router", router(ROUTER_TRAVERSE_KW)), ("vfd", vfd())] {
        let limited = recipe(&machine, None);
        assert!(
            ladder_fired(&limited),
            "{label}: not power limited, so this proves nothing"
        );

        // The same cut on a machine with power to spare — the chipload the
        // calculator wanted before any power constraint.
        let mut roomy = machine.clone();
        roomy.power = PowerModel::ConstantPower { power_kw: 100.0 };
        let free = recipe(&roomy, None);
        assert!(
            !ladder_fired(&free),
            "{label}: the 100 kW reference cut was itself power limited"
        );

        // Be exact about what this covers. The ladder HAS a last-resort feed
        // rung, and that rung does thin the chip — it exists because shipping
        // a recipe the spindle cannot turn is worse than a thinner chip. The
        // claim here is narrower and is the one that matters: while a dial
        // above the feed can still do the job, the feed is not touched.
        let feed_rung_fired = limited.warnings.iter().any(|w| {
            matches!(
                w,
                FeedsWarning::PowerLadderReducedCut {
                    feed_factor: Some(_),
                    ..
                }
            )
        });
        assert!(
            !feed_rung_fired,
            "{label}: the last-resort feed rung fired on a fixture chosen so the              rungs above it suffice. Either the fixture drifted or the ladder              reached for the feed too early."
        );

        let a = advance_per_tooth(&limited, 4.0);
        let b = advance_per_tooth(&free, 4.0);
        assert!(
            (a - b).abs() <= 1e-6 * b.max(1.0),
            "{label}: the ladder moved the advance per tooth from {b:.6} to \
             {a:.6} mm/tooth. It must make the cut SMALLER, not THINNER — a \
             thinner chip is what it replaced, and it did not work."
        );
    }
}

#[test]
fn a_pinned_depth_is_not_reduced_g_ladder() {
    // Pinning a depth is a statement about the part. The engine may cut it
    // slower or narrower, but quietly cutting it shallower would ship a
    // different part than the one that was asked for.
    let pinned = 6.0;
    // Deliberately the LOW-power router: at this budget the ladder wants
    // the depth rung, so the pin has something to refuse.
    let r = recipe(&router(ROUTER_DEPTH_KW), Some(pinned));
    assert!(
        ladder_fired(&r)
            || r.warnings
                .iter()
                .any(|w| matches!(w, FeedsWarning::PowerLimited { .. })),
        "the pinned-depth cut was not power constrained, so this proves nothing"
    );
    let reduced_depth = r.warnings.iter().any(|w| {
        matches!(
            w,
            FeedsWarning::PowerLadderReducedCut {
                axial_from: Some(_),
                ..
            }
        )
    });
    assert!(
        !reduced_depth,
        "the ladder reduced a depth the user pinned at {pinned} mm. \
         Warnings: {:?}",
        r.warnings
    );
}
