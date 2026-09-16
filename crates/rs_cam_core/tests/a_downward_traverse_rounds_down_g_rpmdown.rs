//! Sentry: **a caller reducing the RPM must land at or below what it asked
//! for** (G-RPMDOWN, 2026-09-16).
//!
//! ## The defect this pins
//!
//! `MachineProfile::clamp_rpm` snaps a discrete spindle to the NEAREST listed
//! speed. That is correct for "what will this machine actually run", and
//! wrong for a caller reducing the RPM to shed load.
//!
//! The power ladder traverses the constant-chipload line downward on a
//! constant-power spindle. The constant-power preset IS the discrete one —
//! the Makita, at 10 000 / 12 000 / 17 000 / 22 000 / 27 000 / 30 000. So the
//! machine where the traverse works is exactly the machine where the naive
//! snap misfires: nearest rounds UP about half the time, and a power-limited
//! traverse that rounds up raises the power it was called to reduce.
//!
//! Measured on the Makita before `next_rpm_at_or_below` existed:
//!
//! ```text
//!   asks for 21 000 -> clamp_rpm gives 22 000   UP
//!   asks for 16 000 -> clamp_rpm gives 17 000   UP
//!   asks for 11 500 -> clamp_rpm gives 12 000   UP
//!   asks for  9 000 -> clamp_rpm gives 10 000   UP
//! ```
//!
//! ## What is asserted
//!
//! 1. For every preset and a sweep of requests, the result never exceeds the
//!    request — unless the spindle simply cannot go that slow, which is
//!    called out separately because the caller has to handle it.
//! 2. `clamp_rpm` DOES round up on the discrete preset. The non-vacuity
//!    partner: without it, arm 1 would pass on a build where the helper is
//!    just an alias for `clamp_rpm` on a spindle that never rounds up.
//! 3. The result is always a speed the spindle can actually run.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::machine::{MachineProfile, SpindleConfig};

/// Requests spanning the gaps between the discrete speeds, so at least some
/// of them fall where "nearest" and "at or below" disagree.
const REQUESTS: [f64; 12] = [
    2_000.0, 9_000.0, 11_500.0, 12_000.0, 14_500.0, 16_000.0, 19_000.0, 21_000.0, 23_000.0,
    26_000.0, 29_000.0, 40_000.0,
];

fn spindle_min_max(m: &MachineProfile) -> (f64, f64) {
    m.rpm_range()
}

#[test]
fn a_downward_traverse_never_lands_above_what_it_asked_for_g_rpmdown() {
    let mut checked = 0;
    for (label, machine) in MachineProfile::presets() {
        let (min_rpm, _) = spindle_min_max(&machine);
        for want in REQUESTS {
            let got = machine.next_rpm_at_or_below(want);
            if want < min_rpm {
                // The spindle cannot go this slow. The helper hands back the
                // slowest it has, and the caller must notice the traverse was
                // partial. Asserted so the behaviour is pinned, not assumed.
                assert!(
                    got >= want,
                    "{label}: asked for {want} below the {min_rpm} floor and got {got}, \
                     which is below the floor"
                );
                continue;
            }
            assert!(
                got <= want + 1e-9,
                "{label}: asked for at-or-below {want} and got {got}. A power-limited \
                 traverse that rounds UP raises the load it was called to reduce."
            );
            checked += 1;
        }
    }
    assert!(
        checked >= 20,
        "only {checked} (machine, request) pairs checked"
    );
}

#[test]
fn clamp_rpm_does_round_up_so_the_helper_is_not_redundant_g_rpmdown() {
    // Non-vacuity partner. If `clamp_rpm` never rounded up, the helper would
    // be pointless and the test above would pass on a plain alias.
    let discrete: Vec<MachineProfile> = MachineProfile::presets()
        .into_iter()
        .map(|(_, m)| m)
        .filter(|m| matches!(m.spindle, SpindleConfig::Discrete { .. }))
        .collect();
    assert!(
        !discrete.is_empty(),
        "no discrete-spindle preset exists, so the rounding hazard this guards \
         cannot be exercised. If the presets changed, this sentry needs rewriting, \
         not deleting."
    );

    let mut saw_round_up = false;
    for machine in &discrete {
        let (min_rpm, max_rpm) = spindle_min_max(machine);
        for want in REQUESTS {
            if want < min_rpm || want > max_rpm {
                continue;
            }
            if machine.clamp_rpm(want) > want + 1e-9 {
                saw_round_up = true;
                // And the helper must not repeat it.
                assert!(
                    machine.next_rpm_at_or_below(want) <= want + 1e-9,
                    "{}: clamp_rpm rounded {want} up, and so did the helper",
                    machine.name
                );
            }
        }
    }
    assert!(
        saw_round_up,
        "clamp_rpm never rounded up across the preset sweep, so arm 1 proves nothing"
    );
}

#[test]
fn the_result_is_a_speed_the_spindle_can_run_g_rpmdown() {
    for (label, machine) in MachineProfile::presets() {
        for want in REQUESTS {
            let got = machine.next_rpm_at_or_below(want);
            match &machine.spindle {
                SpindleConfig::Discrete { speeds } => assert!(
                    speeds.iter().any(|s| (s - got).abs() < 1e-9),
                    "{label}: {got} is not one of the spindle's listed speeds"
                ),
                SpindleConfig::Variable { min_rpm, max_rpm } => assert!(
                    got >= *min_rpm - 1e-9 && got <= *max_rpm + 1e-9,
                    "{label}: {got} is outside the spindle range {min_rpm}..{max_rpm}"
                ),
            }
        }
    }
}
