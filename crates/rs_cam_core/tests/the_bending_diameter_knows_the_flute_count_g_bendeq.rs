//! Sentry: **the deflection model's bending section is an EQUIVALENT diameter
//! and it depends on the flute count** (G-BENDEQ, T-16, 2026-09-17).
//!
//! ## The defect this pins
//!
//! `ENDMILL_CORE_FRACTION = 0.7` was applied to every end mill regardless of
//! flute count, and cited to "Machinery's Handbook stiffness-correction notes"
//! and FSWizard for 2- to 3-flute tools. A literature search found no source
//! giving 0.7 as a 2- to 3-flute core fraction; published 2-flute cores run
//! 0.54 to 0.60.
//!
//! ## The trap this exists to stop
//!
//! Two quantities, conflated by the sources:
//!
//! | | Published | What it is |
//! |---|---|---|
//! | Core diameter | 0.47 – 0.85 D | the geometric root between the flutes |
//! | Equivalent diameter | 0.75 – 0.93 D | the shaft with the same bending compliance |
//!
//! **The flute-count effect reverses sign between them.** More flutes gives a
//! LARGER core and a SMALLER equivalent diameter. A cantilever needs the
//! second, so reaching for a core figure gets the correction backwards — a
//! mistake made once during the work that produced this file, caught only
//! because the research separated the two.
//!
//! That is why arm 2 below asserts the DIRECTION, not just the values. A
//! future edit that swaps in core figures would keep flute-dependence and
//! invert the physics, and only a direction test catches that.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::predict::{
    ENDMILL_EQUIVALENT_FALLBACK, bending_diameter_mm, endmill_equivalent_diameter_fraction,
};

/// Kivanc and Budak, Sabanci MSc thesis 2004, Tables 3.1 and 3.2.
const SOURCED: [(u32, f64); 3] = [(2, 0.889), (3, 0.841), (4, 0.748)];

#[test]
fn the_sourced_flute_counts_carry_their_published_values_g_bendeq() {
    for (flutes, expect) in SOURCED {
        let got = endmill_equivalent_diameter_fraction(flutes);
        assert!(
            (got - expect).abs() < 1e-9,
            "{flutes} flutes gives {got}, not the sourced {expect}. These come \
             from a published deflection table — changing one needs a new \
             citation, not a new opinion."
        );
    }
}

#[test]
fn more_flutes_means_a_smaller_equivalent_diameter_g_bendeq() {
    // The direction, asserted separately from the values. A swap to CORE
    // figures would reverse this while still looking flute-aware.
    let two = endmill_equivalent_diameter_fraction(2);
    let three = endmill_equivalent_diameter_fraction(3);
    let four = endmill_equivalent_diameter_fraction(4);
    assert!(
        two > three && three > four,
        "the equivalent diameter must FALL as flute count rises: got {two}, \
         {three}, {four}. If this reversed, someone has reached for core \
         diameters — those rise with flute count and are the wrong quantity \
         for a bending model."
    );
}

#[test]
fn the_fallback_is_conservative_against_every_sourced_value_g_bendeq() {
    // Outside 2..=4 the historical 0.70 stands. It must stay SMALLER than
    // every sourced figure, because a smaller bending diameter over-states
    // deflection, which is the safe direction for a guard.
    for (flutes, sourced) in SOURCED {
        assert!(
            ENDMILL_EQUIVALENT_FALLBACK < sourced,
            "the fallback {ENDMILL_EQUIVALENT_FALLBACK} is not below the \
             {flutes}-flute value {sourced}, so an unsourced flute count would \
             now UNDER-state deflection"
        );
    }
    for odd in [1_u32, 5, 6, 12] {
        assert!(
            (endmill_equivalent_diameter_fraction(odd) - ENDMILL_EQUIVALENT_FALLBACK).abs() < 1e-9,
            "{odd} flutes is outside the sourced range and must take the \
             fallback rather than an extrapolation"
        );
    }
}

#[test]
fn the_flute_count_actually_reaches_the_bending_diameter_g_bendeq() {
    // Non-vacuity. The values could be right and never consulted: before this
    // change `core_diameter_mm` ignored the flute count entirely.
    let build = |flutes: u32| {
        let mut t = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
        t.diameter = 6.0;
        t.flute_count = flutes;
        bending_diameter_mm(&t)
    };
    let two = build(2);
    let four = build(4);
    assert!(
        (two - four).abs() > 1e-6,
        "a 2-flute and a 4-flute Ø6 end mill give the same bending diameter \
         ({two}). The flute count is not reaching the calculation."
    );
    assert!(
        (two - 0.889 * 6.0).abs() < 1e-9,
        "the Ø6 2-flute bending diameter is {two}, not 0.889 x 6.0"
    );
}

#[test]
fn a_ball_nose_still_bends_on_its_full_diameter_g_bendeq() {
    // The flute-count term is for END MILLS. A ball nose is limited by the
    // ball/shank junction, not by flute relief, and must not pick up the
    // reduction.
    for flutes in [2_u32, 3, 4] {
        let mut t = ToolConfig::new_default(ToolId(1), ToolType::BallNose);
        t.diameter = 6.0;
        t.flute_count = flutes;
        let d = bending_diameter_mm(&t);
        assert!(
            (d - 6.0).abs() < 1e-9,
            "a {flutes}-flute Ø6 ball nose reports a bending diameter of {d}. \
             The equivalent-diameter reduction is an end-mill correction."
        );
    }
}
