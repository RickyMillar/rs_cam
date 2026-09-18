//! R10 (2026-09-18) — the finishing families get no ramp entry by default.
//!
//! ## The defect this closes
//!
//! The Corne waterline (`planning/corne_case_analysis_2026-09-18/ANALYSIS.md`
//! §4.5) ran with the `SemiFinish` role defaults: `entry_style = Ramp` at
//! 3 degrees. A 2 mm drop at 3 degrees is 38 mm of XY. A defect elsewhere
//! put one contour run inside a 3 mm wall as a 2 mm segment, and the ramp
//! fold (G-RAMPCONTAIN) laid that 38 mm as 19 laps over the 2 mm run at
//! every level. The simulation showed the wall sawn to a row of pins.
//!
//! Two fixes share the ruling. The fold refuses a run it would lap more
//! than `RAMP_FOLD_MAX_LAPS` times (`dressup::tests`,
//! `ramp_fold_caps_laps_and_falls_back_to_plunge_r10`). This sentry pins
//! the other half: a finishing contour starts on a 0.5 mm leave, so a
//! plunge there is a 0.5 mm bite and the ramp buys nothing. `Finish` and
//! `SemiFinish` now default to `DressupEntryStyle::None`. `Roughing` keeps
//! `Ramp`.
//!
//! ## What this sentry pins
//!
//! 1. `DressupConfig::for_role`: `Finish` and `SemiFinish` give `None`,
//!    `Roughing` gives `Ramp`.
//! 2. Every operation whose registry role is `Finish` or `SemiFinish`
//!    creates with `entry_style == None` through `for_op`. The population
//!    comes from `OperationType::ALL` and the registry, never from a
//!    literal list, and it must hold at least ten operations with an
//!    unrestricted (`AnyEntry`, no strip) dressup policy — the ones whose
//!    `None` can only come from the role default.
//! 3. No per-op policy promotes an entry: `normalize_for_op` leaves `None`
//!    as `None` on every operation.
//! 4. The roughing family is unmoved: `Pocket` creates with `Ramp`,
//!    `Adaptive` with `Helix`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use rs_cam_core::compute::catalog::{EntryStylePolicy, OperationType, UiProcessRole};
use rs_cam_core::compute::config::{DressupConfig, DressupEntryStyle};

fn is_finishing(role: UiProcessRole) -> bool {
    matches!(role, UiProcessRole::Finish | UiProcessRole::SemiFinish)
}

#[test]
fn the_finishing_roles_default_to_no_entry_and_roughing_keeps_ramp() {
    assert_eq!(
        DressupConfig::for_role(UiProcessRole::Finish).entry_style,
        DressupEntryStyle::None,
        "R10: the Finish role must not ask for a ramp entry"
    );
    assert_eq!(
        DressupConfig::for_role(UiProcessRole::SemiFinish).entry_style,
        DressupEntryStyle::None,
        "R10: the SemiFinish role must not ask for a ramp entry"
    );
    assert_eq!(
        DressupConfig::for_role(UiProcessRole::Roughing).entry_style,
        DressupEntryStyle::Ramp,
        "R10 moves the finishing families only; Roughing keeps Ramp"
    );
}

#[test]
fn every_finishing_operation_creates_with_no_entry() {
    let mut finishing = Vec::new();
    let mut unrestricted = Vec::new();
    for &op in OperationType::ALL {
        let role = op.spec().ui_process_role;
        if !is_finishing(role) {
            continue;
        }
        finishing.push(op);
        let cfg = DressupConfig::for_op(op);
        assert_eq!(
            cfg.entry_style,
            DressupEntryStyle::None,
            "R10: {op:?} ({role:?}) must create with no entry ramp"
        );
        let policy = op.registry_entry().dressup_policy;
        if policy.strip_all_reason.is_none() && policy.entry == EntryStylePolicy::AnyEntry {
            unrestricted.push(op);
        }
    }

    // Non-vacuity. The three operations the case analysis names must be
    // in the population, and the unrestricted subset must be large: those
    // are the operations whose `None` the registry does not force, so the
    // role default is the only thing that puts it there.
    for want in [
        OperationType::Waterline,
        OperationType::Scallop,
        OperationType::Pencil,
    ] {
        assert!(
            unrestricted.contains(&want),
            "population: {want:?} must be a finishing operation with an \
             unrestricted dressup policy; got {unrestricted:?}"
        );
    }
    assert!(
        unrestricted.len() >= 10,
        "population: expected at least ten unrestricted finishing operations, \
         got {} of {} finishing: {unrestricted:?}",
        unrestricted.len(),
        finishing.len()
    );
}

#[test]
fn no_per_op_policy_promotes_an_entry_from_none() {
    for &op in OperationType::ALL {
        let mut cfg = DressupConfig {
            entry_style: DressupEntryStyle::None,
            ..DressupConfig::default()
        };
        cfg.normalize_for_op(op);
        assert_eq!(
            cfg.entry_style,
            DressupEntryStyle::None,
            "R10: normalize_for_op({op:?}) must leave None as None"
        );
    }
}

#[test]
fn the_roughing_family_is_unmoved() {
    assert_eq!(
        DressupConfig::for_op(OperationType::Pocket).entry_style,
        DressupEntryStyle::Ramp,
        "Pocket is Roughing with an unrestricted policy: Ramp"
    );
    assert_eq!(
        DressupConfig::for_op(OperationType::Adaptive).entry_style,
        DressupEntryStyle::Helix,
        "Adaptive is Roughing with PreferHelix: Ramp promotes to Helix"
    );
    assert_eq!(
        DressupConfig::for_op(OperationType::Profile).entry_style,
        DressupEntryStyle::Ramp,
        "Profile is Roughing with an unrestricted policy: Ramp"
    );
}
