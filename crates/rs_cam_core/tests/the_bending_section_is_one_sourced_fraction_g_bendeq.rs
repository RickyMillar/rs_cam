//! Sentry: **the bending section is ONE sourced fraction, applied to every
//! fluted shape, with no flute-count term** (G-BENDEQ, T-17, 2026-09-17).
//!
//! ## What this replaces, and why it inverted
//!
//! This file supersedes `the_bending_diameter_knows_the_flute_count_g_bendeq`,
//! which asserted the opposite: a per-flute table (2 -> 0.889, 3 -> 0.841,
//! 4 -> 0.748) from Kivanc and Budak, and a DIRECTION arm requiring the
//! fraction to FALL as flute count rises.
//!
//! A first-principles derivation of the fluted section
//! (`planning/load_model_2026-09-16/FLUTE_SECTION_MATH.md`) removed both:
//!
//! 1. **The N-trend is an artefact.** The derivation reproduces those three
//!    values, but only under the "one flute shape, copied N times" family the
//!    thesis models. Real end mills have a core that RISES with flute count
//!    and a land fraction that RISES with it, and both push the fraction UP.
//!    Under a realistic family the trend REVERSES. Kops and Vo measured about
//!    0.80 flat across two- and four-flute tools, from compliance.
//! 2. **The two-flute value was the wrong statistic.** A two-flute section is
//!    the only one that is not isotropic — for N >= 3 the deviatoric part of
//!    the inertia tensor vanishes by symmetry, but two flutes leave principal
//!    values differing by 2.4x to 2.7x. The published 0.889 is their
//!    ARITHMETIC mean. A rotating tool under a fixed-direction load averages
//!    COMPLIANCE, so the correct statistic is the harmonic mean, about 0.84.
//!
//! The lesson worth keeping: the old file's direction arm was written to stop
//! someone swapping in CORE diameters, which rise with flute count. It would
//! have done that. It also encoded a law that no longer survives contact with
//! the geometry. **A guard against one error became an assertion of another.**
//!
//! ## The trap this exists to stop
//!
//! Re-introducing a flute-count term. It is a tempting edit: published tables
//! exist, they look authoritative, and a per-flute function looks more precise
//! than one constant. Arm 2 fails on any such change.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolMaterial, ToolType};
use rs_cam_core::feeds::predict::{ENDMILL_EQUIVALENT_DIAMETER_FRACTION, bending_diameter_mm};
use rs_cam_core::tool::{FlatEndmill, ToolDefinition};

#[test]
fn the_fraction_is_the_sourced_value_g_bendeq() {
    assert!(
        (ENDMILL_EQUIVALENT_DIAMETER_FRACTION - 0.80).abs() < 1e-9,
        "the equivalent-diameter fraction is {ENDMILL_EQUIVALENT_DIAMETER_FRACTION}, not the \
         sourced 0.80. This comes from Kops and Vo, Annals of the CIRP 39(1):93-96 (1990), who \
         measured it from COMPLIANCE — the quantity a cantilever model needs. Changing it needs \
         a new citation, not a new opinion."
    );
}

#[test]
fn the_bending_section_does_not_depend_on_the_flute_count_g_bendeq() {
    // The anti-artefact guard, and the reason this file exists.
    // `I` goes as the 4th power, so a re-introduced flute term would move
    // every deflection number by up to 3x while looking better sourced.
    let deflection_at = |flutes: u32| {
        let td = ToolDefinition::new(
            Box::new(FlatEndmill::new(6.0, 60.0)),
            6.0,
            100.0,
            25.0,
            45.0,
            flutes,
            ToolMaterial::Carbide,
        );
        td.tip_deflection_mm(100.0, 6.0, 600_000.0)
    };
    let two = deflection_at(2);
    let four = deflection_at(4);
    assert!(
        (two - four).abs() < 1e-12,
        "a 2-flute and a 4-flute Ø6 end mill deflect differently ({two} vs {four}). A \
         flute-count term has come back. The published 2/3/4 table reproduces only under a \
         'one flute shape, copied N times' family that real end mills do not follow — under a \
         realistic family the trend REVERSES. See FLUTE_SECTION_MATH.md section 4.3."
    );
}

#[test]
fn every_fluted_shape_takes_the_fraction_g_bendeq() {
    // The relief is a property of the CROSS-SECTION, so the end of the tool
    // does not change it. Before T-17 a ball and a bull nose were modelled as
    // solid at full diameter.
    for tool_type in [ToolType::EndMill, ToolType::BallNose, ToolType::BullNose] {
        let mut t = ToolConfig::new_default(ToolId(1), tool_type);
        t.diameter = 6.0;
        t.flute_count = 2;
        let got = bending_diameter_mm(&t);
        let want = ENDMILL_EQUIVALENT_DIAMETER_FRACTION * 6.0;
        assert!(
            (got - want).abs() < 1e-9,
            "{tool_type:?} reports a bending diameter of {got}, not {want}. A bull nose has an \
             identical section above its corner radius, and a ball nose's ball region carries \
             under 1 % of the compliance at L/D >= 3, so both take the fraction."
        );
    }
}

#[test]
fn the_fraction_actually_reaches_the_integrator_g_bendeq() {
    // Non-vacuity. Arm 2 would pass if the fraction were never consulted at
    // all, because 1.0 is also flute-independent. This arm pins that the
    // section really is reduced: the integrator must read SOFTER than the
    // solid-diameter closed form, by exactly (1/f)^4.
    let td = ToolDefinition::new(
        Box::new(FlatEndmill::new(6.0, 60.0)),
        6.0,
        100.0,
        25.0,
        45.0,
        2,
        ToolMaterial::Carbide,
    );
    let (force, axial_doc, l, e) = (270.0_f64, 6.0_f64, 45.0_f64, 600_000.0_f64);
    let a = l - axial_doc * 0.5;
    let i_solid = std::f64::consts::PI * 6.0_f64.powi(4) / 64.0;
    let solid = force * a * a * (3.0 * l - a) / (6.0 * e * i_solid);
    let got = td.tip_deflection_mm(force, axial_doc, e);
    let want_ratio = 1.0 / ENDMILL_EQUIVALENT_DIAMETER_FRACTION.powi(4);
    let got_ratio = got / solid;
    assert!(
        (got_ratio - want_ratio).abs() / want_ratio < 0.01,
        "the integrator reads {got_ratio:.4}x the solid-diameter closed form, not the \
         {want_ratio:.4}x the fraction demands. Either the fraction stopped reaching \
         `tip_deflection_mm`, or something else changed the section."
    );
}

#[test]
fn a_vbit_is_the_documented_exception_g_bendeq() {
    // A V-bit's flute is not an end-mill flute and no source gives a figure,
    // so it stays unmodelled. This arm exists so the gap stays DELIBERATE: it
    // fails if someone quietly extends the fraction to V-bits without a
    // citation, which would be a fabricated constant inside a safety guard.
    let mut t = ToolConfig::new_default(ToolId(1), ToolType::VBit);
    t.diameter = 6.0;
    t.flute_count = 2;
    let got = bending_diameter_mm(&t);
    assert!(
        got.abs() < 1e-9,
        "a V-bit now reports a bending diameter of {got}. The closed-form path refuses V-bits, \
         so this must stay 0.0. Note the gap is NON-conservative wherever a V-bit IS modelled \
         as a solid cone — it under-states deflection by (1/f_V)^4. See T-4."
    );
}
