//! Sentry: **a depth the engine writes must be a depth the machine will
//! actually cut** (G-STAIR, T-12 second half, 2026-09-16).
//!
//! ## The defect this pins
//!
//! Generation steps a 2.5D cut as `total / ceil(total / per_pass)`, the one
//! distribution `DepthStepping` has (CUT-15 deleted the other). So the
//! realised depth is a STAIRCASE. On a 12 mm pocket only 4.00, 3.00, 2.40,
//! 2.00, 1.71, 1.50 and 1.33 mm are reachable. Asking for 2.90 cuts 2.40, and
//! asking for 2.60 also cuts 2.40.
//!
//! The apply funnel wrote its proposal through unsnapped. The cut was right —
//! generation applies the same arithmetic either way — but every number the
//! engine then reported was up to 17 % away from what the machine does. The
//! power ladder made that load-bearing: its own account of what it did
//! ("depth 8.40 → 2.90 mm") named a depth that never happens.
//!
//! ## Direction matters, and it is the reassuring one
//!
//! `realised <= requested` always. Verified here by sampling, as it was
//! during the fix over 200 000 random pairs with no counterexample. So a
//! caller reasoning about LOAD stays conservative: the machine cuts no deeper
//! than the engine planned for. This is a reporting defect, not a safety one,
//! and the sentry says so rather than overstating it.
//!
//! ## What is asserted
//!
//! 1. The realised depth is never above the requested one.
//! 2. Snapping is idempotent — a snapped value snaps to itself. Without this,
//!    a "fix" that merely scaled the value would pass arm 1.
//! 3. The staircase is REAL at the sizes we ship: some requests move.
//!    Non-vacuity — if every request were already realisable, the other arms
//!    would prove nothing.
//! 4. Degenerate inputs abstain rather than inventing a depth.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::ops::depth::{DepthStepping, realised_step_down};

/// Totals and requests spanning the sizes this engine actually ships.
const TOTALS: [f64; 6] = [3.0, 6.0, 12.0, 18.0, 25.4, 40.0];
const REQUESTS: [f64; 8] = [0.5, 1.0, 1.71, 2.4, 2.6, 2.9, 4.0, 9.0];

#[test]
fn the_machine_never_cuts_deeper_than_the_engine_asked_g_stair() {
    let mut checked = 0;
    for total in TOTALS {
        for req in REQUESTS {
            let Some(realised) = realised_step_down(total, req) else {
                panic!("{total} / {req} abstained on a valid pair");
            };
            assert!(
                realised <= req + 1e-12,
                "total {total}, requested {req}: realised {realised} is DEEPER than \
                 requested. The load reasoning upstream assumes it never is."
            );
            assert!(
                realised > 0.0,
                "total {total}, requested {req}: realised {realised}"
            );
            checked += 1;
        }
    }
    assert!(checked >= 40, "only {checked} pairs checked");
}

#[test]
fn snapping_is_idempotent_g_stair() {
    // The non-vacuity partner for arm 1. A "fix" that just scaled the value
    // down would satisfy arm 1 and fail here.
    for total in TOTALS {
        for req in REQUESTS {
            let once = realised_step_down(total, req).unwrap();
            let twice = realised_step_down(total, once).unwrap();
            assert!(
                (once - twice).abs() <= 1e-9 * once.max(1.0),
                "total {total}, requested {req}: snapped to {once} then to {twice}. \
                 A realisable depth must snap to itself, or the engine's number \
                 still is not the machine's."
            );
        }
    }
}

#[test]
fn the_staircase_actually_bites_at_shipped_sizes_g_stair() {
    // If nothing ever moved, every other arm here would be vacuous.
    let mut moved = Vec::new();
    for total in TOTALS {
        for req in REQUESTS {
            let realised = realised_step_down(total, req).unwrap();
            if (realised - req).abs() > 1e-9 {
                moved.push((total, req, realised));
            }
        }
    }
    assert!(
        moved.len() >= 20,
        "only {} of {} pairs moved — the staircase would not be worth fixing",
        moved.len(),
        TOTALS.len() * REQUESTS.len()
    );

    // And it moves by an amount worth reporting, not a rounding wobble.
    let worst = moved
        .iter()
        .map(|(_, req, realised)| (req - realised) / req)
        .fold(0.0_f64, f64::max);
    assert!(
        worst > 0.10,
        "the largest snap was {:.1} %, too small to explain the fix",
        worst * 100.0
    );

    // The documented case, stated exactly.
    let twelve = realised_step_down(12.0, 2.9).unwrap();
    assert!(
        (twelve - 2.4).abs() < 1e-9,
        "a 2.9 mm request on a 12 mm pocket must cut 2.4 mm, got {twelve}"
    );
}

#[test]
fn a_depth_that_cannot_be_stepped_abstains_g_stair() {
    // Absence rendered as a reading is this repository's recurring defect.
    // A degenerate pair has no realisable depth, and must not be handed one.
    for (total, req) in [
        (0.0, 3.0),
        (-5.0, 3.0),
        (12.0, 0.0),
        (12.0, -1.0),
        (f64::NAN, 3.0),
        (12.0, f64::NAN),
        (f64::INFINITY, 3.0),
    ] {
        assert!(
            realised_step_down(total, req).is_none(),
            "total {total}, requested {req}: invented a depth instead of abstaining"
        );
    }
    // A request larger than the whole cut is ONE pass of the whole cut, which
    // is a real answer rather than a degenerate one.
    assert!(
        (realised_step_down(6.0, 9.0).unwrap() - 6.0).abs() < 1e-9,
        "a request deeper than the cut must give one pass of the full depth"
    );
}

#[test]
fn generation_cuts_the_depth_the_snap_named_g_stair() {
    // The arm that makes the whole fix worth anything. Snapping is pointless
    // if the thing that actually steps the cut disagrees, and before the two
    // shared one `pass_count` they could: `25.4 / 15` is a hair under the
    // exact third, so dividing back gave `15.000000000000002` and a bare
    // `ceil` produced 16 passes of 1.5875 instead of 15 of 1.6933.
    let mut checked = 0;
    for total in TOTALS {
        for req in REQUESTS {
            let snapped = realised_step_down(total, req).unwrap();
            // What generation will actually do with the snapped value.
            let stepping = DepthStepping::new(0.0, -total, snapped);
            let n = stepping.roughing_pass_count();
            assert!(n >= 1, "total {total}, snapped {snapped}: {n} passes");
            let cut = total / n as f64;
            assert!(
                (cut - snapped).abs() <= 1e-9 * snapped.max(1.0),
                "total {total}, requested {req}: the engine writes {snapped} and \
                 generation cuts {cut} over {n} passes. The number the operator \
                 is shown must be the number the machine cuts."
            );
            checked += 1;
        }
    }
    assert!(checked >= 40, "only {checked} pairs checked");
}
