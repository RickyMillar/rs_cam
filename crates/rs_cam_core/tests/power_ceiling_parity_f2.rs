//! **F-2 — Suggest's spindle-power ceiling omitted `machine.safety_factor`
//! while the gate applied it.**
//!
//! Census: `planning/review_2026-08-04/FEEDS_CENSUS.md` §2.6 (C-8), §6.2
//! P-2, §6.3 F-2, tier-3 item T3.2. Ruled at Checkpoint B Q3:
//! *"power safety-factor parity NOW (Suggest gains
//! `machine.safety_factor`, before/after recorded)"*.
//!
//! ## The two ceilings
//!
//! | | site | ceiling |
//! |---|---|---|
//! | gate | `tool_load::power::evaluate` (`power.rs:214`) | `machine.power_at_rpm(rpm) · machine.safety_factor` |
//! | Suggest | `feeds::calculate` Step 6 (`feeds/mod.rs:1233`) | `machine.power_at_rpm(rpm)` |
//!
//! Suggest both *clamped* against the unfactored ceiling and *published*
//! it as [`FeedsResult::available_power_kw`], which is the denominator of
//! the headroom the Feeds & Speeds modal renders. An operator reading
//! that headroom saw `1/safety_factor` = **1.25×–1.33×** more room than
//! the gate would allow for the same cut.
//!
//! ## What these tests pin
//!
//! 1. [`suggest_power_ceiling_equals_the_gate_power_ceiling`] — the
//!    parity claim itself, across every shipped machine preset. **Fails
//!    on the parent revision** for every preset (all ship
//!    `safety_factor < 1.0`).
//! 2. [`the_published_headroom_denominator_is_the_gate_ceiling`] — the
//!    consumer-facing consequence: `available_power_kw` is what the UI
//!    divides by, so it must be the bound the verdict will use.
//! 3. [`the_recommended_feed_never_implies_power_above_the_gate_ceiling`]
//!    — direction check over presets × 10 species × Ø3/Ø6/Ø12.
//! 4. [`the_power_ceiling_does_not_bind_on_shipped_presets`] — the
//!    measurement that **narrows the census's claim** (below).
//! 5. [`a_power_limited_feed_lands_exactly_on_the_gate_ceiling`] — the
//!    no-double-count guard, on a synthetic under-powered spindle
//!    because no shipped preset reaches the branch.
//! 6. [`the_power_limited_warning_compares_like_with_like`] — the
//!    warning's two terms sit on the gate's axis too.
//! 7. [`report_before_after_feeds_on_representative_fixtures`] — not an
//!    assertion, a **record**, reproducible with `--nocapture`.
//!
//! ## What the measurement narrowed
//!
//! The census rated P-2 **HIGH** on the reasoning that "Suggest can ship
//! a feed the gate calls `Exceeds`". Swept across all three shipped
//! presets × all ten wood species × Ø3/Ø6/Ø12 full-width slots, the
//! `power_limited` branch **never fires**: `feeds::calculate` clamps
//! axial DOC (rigidity) and feed (machine cutting ceiling) long before
//! spindle power binds. So on shipped profiles F-2 moved **no
//! recommended feed at all** — what it moved is the published
//! `available_power_kw`, i.e. the denominator of the headroom the Feeds
//! & Speeds modal renders, by `safety_factor` (−25 % / −20 %).
//!
//! ## Two axes, and why the clamp is NOT multiplied
//!
//! `feeds::calculate` already applies `machine.safety_factor` to the
//! feed at Step 9. So there are two internally-consistent axes:
//!
//! | axis | feed | ceiling |
//! |---|---|---|
//! | RAW | `raw_feed` (pre-Step-9) | `power_at_rpm(rpm)` |
//! | COMMANDED (the gate's) | final feed | `power_at_rpm(rpm) · safety_factor` |
//!
//! The Step 6 clamp lives on the RAW axis and is correct there: Step 9
//! then scales the feed by `safety_factor`, so the commanded result
//! satisfies the gate's bound by construction, landing exactly on it
//! (test 5).
//!
//! Multiplying that clamp's ceiling by `safety_factor` as well — the
//! naive reading of "Suggest gains `machine.safety_factor`" — applies
//! the factor **twice**. Measured: a power-limited feed drops a further
//! 25 % (723.4 → 542.5 mm/min on the synthetic fixture) and the
//! literature-matrix cell `flat_6mm_pocket_al6061_lut` goes `major`,
//! chipload falling to 0.0269 mm/tooth — within 10 % of the 0.025
//! rubbing floor. That is a feed-moving recalibration Q3 did not
//! authorise, and §7 forbids absorbing it by re-pinning the cell.
//!
//! What actually lacked parity is the PUBLISHED pair: `power_kw` is a
//! COMMANDED-axis number and `available_power_kw`, its denominator in
//! the modal's headroom bar, was a RAW-axis one. Both now sit on the
//! gate's axis, as does the `PowerLimited` warning.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::feeds::{
    FeedsInput, FeedsResult, OperationFamily, PassRole, SetupContext, SpindleStrategy,
    ToolGeometryHint, calculate, embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// The heaviest 2.5D roughing cut the calculator will entertain for a
/// given tool: full-width slot, deep commanded DOC. Used as the sweep
/// fixture — if the power ceiling binds anywhere, it binds here.
fn slot_cut(machine: &MachineProfile, species: WoodSpecies, diameter: f64) -> FeedsResult {
    let material = Material::SolidWood { species };
    calculate(&FeedsInput {
        tool_diameter: diameter,
        flute_count: 2,
        flute_length: 3.0 * diameter,
        shank_diameter: None,
        tool_geometry: ToolGeometryHint::Flat,
        material: &material,
        machine,
        operation: OperationFamily::Pocket,
        pass_role: PassRole::Roughing,
        // Ask for a full-width slot at a deep DOC — the heaviest cut the
        // calculator will entertain for this tool. Both are clamped
        // downstream (rigidity / LUT ap band); the point is to request
        // the maximum so nothing but those clamps limits the load.
        axial_depth_mm: Some(2.0 * diameter),
        radial_width_mm: Some(diameter),
        target_scallop_mm: None,
        vendor_lut: Some(embedded_vendor_lut()),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MatchChart,
    })
}

/// A light finishing cut — the control: whatever the heavy fixture does,
/// this must never be power-limited.
fn light_finish(machine: &MachineProfile) -> FeedsResult {
    let material = Material::SolidWood {
        species: WoodSpecies::RadiataPine,
    };
    calculate(&FeedsInput {
        tool_diameter: 3.0,
        flute_count: 2,
        flute_length: 18.0,
        shank_diameter: None,
        tool_geometry: ToolGeometryHint::Flat,
        material: &material,
        machine,
        operation: OperationFamily::Contour,
        pass_role: PassRole::Finish,
        axial_depth_mm: Some(1.0),
        radial_width_mm: Some(0.5),
        target_scallop_mm: None,
        vendor_lut: Some(embedded_vendor_lut()),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MatchChart,
    })
}

/// Every shipped wood species — the sweep population.
const ALL_SPECIES: [WoodSpecies; 10] = [
    WoodSpecies::GenericSoftwood,
    WoodSpecies::RadiataPine,
    WoodSpecies::LongleafPine,
    WoodSpecies::GenericHardwood,
    WoodSpecies::HardMaple,
    WoodSpecies::Walnut,
    WoodSpecies::Birch,
    WoodSpecies::WhiteOak,
    WoodSpecies::Jarrah,
    WoodSpecies::Ipe,
];

/// The gate's ceiling, spelled out here rather than imported so the test
/// states the claim independently of the production expression it
/// checks. Mirrors `tool_load::power::evaluate` (`power.rs:214`).
fn gate_power_ceiling_kw(machine: &MachineProfile, rpm: f64) -> f64 {
    machine.power_at_rpm(rpm) * machine.safety_factor
}

#[test]
fn suggest_power_ceiling_equals_the_gate_power_ceiling() {
    // The parity claim. Every shipped preset, because the ceiling is a
    // per-machine quantity and a single fixture could pass by accident
    // on a profile whose safety_factor happened to be 1.0.
    let mut checked = 0usize;
    for (_preset_label, machine) in MachineProfile::presets() {
        assert!(
            machine.safety_factor < 1.0,
            "preset {} ships safety_factor {} — a 1.0 factor would make this \
             test vacuous for that preset",
            machine.name,
            machine.safety_factor
        );
        for species in [
            WoodSpecies::HardMaple,
            WoodSpecies::WhiteOak,
            WoodSpecies::RadiataPine,
        ] {
            let result = slot_cut(&machine, species, 12.0);
            let expected = gate_power_ceiling_kw(&machine, result.rpm);
            checked += 1;
            assert!(
                (result.available_power_kw - expected).abs() < 1e-9,
                "{}/{:?}: Suggest's available power {:.6} kW != the gate's ceiling \
                 {:.6} kW (= power_at_rpm({:.0}) {:.6} × safety_factor {:.3}). \
                 F-2: feeds/mod.rs Step 6 omitted the safety factor that \
                 tool_load::power::evaluate applies.",
                machine.name,
                species,
                result.available_power_kw,
                expected,
                result.rpm,
                machine.power_at_rpm(result.rpm),
                machine.safety_factor,
            );
        }
    }
    assert!(checked > 0, "no preset exercised — assertion never ran");
}

#[test]
fn the_published_headroom_denominator_is_the_gate_ceiling() {
    // `available_power_kw` is not an internal: it is the denominator the
    // Feeds & Speeds modal renders headroom against
    // (`feeds::explain::MachineEnvelope` / the modal's power row). If it
    // is not the gate's bound then the modal's "headroom" and the
    // verdict's "Exceeds" describe different machines.
    let machine = MachineProfile::shapeoko_vfd();
    let result = slot_cut(&machine, WoodSpecies::HardMaple, 12.0);
    let ceiling = gate_power_ceiling_kw(&machine, result.rpm);
    assert!(
        (result.available_power_kw - ceiling).abs() < 1e-9,
        "published headroom denominator {:.6} kW must be the gate ceiling {:.6} kW",
        result.available_power_kw,
        ceiling
    );
    assert!(
        result.available_power_kw > 0.0,
        "fixture must produce a positive ceiling, else the ratio below is meaningless"
    );
}

#[test]
fn the_recommended_feed_never_implies_power_above_the_gate_ceiling() {
    // Direction check, and the only test here that looks at the feed
    // rather than the ceiling: a recommendation must never imply a
    // predicted power above the bound the gate will judge it by. Swept
    // over every preset × every species × three tool sizes, so the claim
    // is not one fixture's luck.
    //
    // This test deliberately does NOT require that anything be
    // power-limited — measurement (see
    // `the_power_ceiling_does_not_bind_on_shipped_presets`) shows nothing
    // is, and a bar that demanded otherwise would be asserting a fixture
    // property rather than a code property.
    for (_preset_label, machine) in MachineProfile::presets() {
        for species in ALL_SPECIES {
            for diameter in [3.0, 6.0, 12.0] {
                let result = slot_cut(&machine, species, diameter);
                let ceiling = gate_power_ceiling_kw(&machine, result.rpm);
                assert!(
                    result.power_kw <= ceiling + 1e-9,
                    "{} / {:?} / Ø{diameter}: recommended feed implies {:.6} kW, \
                     above the gate ceiling {:.6} kW",
                    machine.name,
                    species,
                    result.power_kw,
                    ceiling
                );
            }
        }
    }
}

#[test]
fn the_power_ceiling_does_not_bind_on_shipped_presets() {
    // MEASUREMENT, pinned so it cannot silently stop being true.
    //
    // The census rated P-2 HIGH on the reasoning that "Suggest can ship a
    // feed the gate calls Exceeds". Swept across every shipped preset,
    // every wood species and Ø3/Ø6/Ø12, the power branch never fires:
    // peak utilisation of the *unfactored* ceiling stays far below 1.0,
    // because `feeds::calculate` clamps axial DOC (rigidity) and feed
    // (machine cutting ceiling) long before spindle power binds.
    //
    // The operator-visible consequence of F-2 is therefore in the
    // PUBLISHED HEADROOM (`available_power_kw`, the modal's denominator),
    // not in any recommended feed on these profiles. That is a narrower
    // claim than the census made, and it is the honest one.
    let mut worst = 0.0_f64;
    let mut worst_label = String::new();
    let mut limited_count = 0usize;
    for (_preset_label, machine) in MachineProfile::presets() {
        for species in ALL_SPECIES {
            for diameter in [3.0, 6.0, 12.0] {
                let result = slot_cut(&machine, species, diameter);
                if result.power_limited {
                    limited_count += 1;
                }
                let unfactored = machine.power_at_rpm(result.rpm);
                if unfactored <= 0.0 {
                    continue;
                }
                let utilisation = result.power_kw / unfactored;
                if utilisation > worst {
                    worst = utilisation;
                    worst_label = format!("{} / {:?} / Ø{diameter}", machine.name, species);
                }
            }
        }
    }
    eprintln!(
        "  peak spindle-power utilisation across the sweep: {:.1}% of the \
         unfactored ceiling ({worst_label}); power-limited fixtures: {limited_count}",
        100.0 * worst
    );
    assert_eq!(
        limited_count,
        0,
        "a shipped preset became power-limited ({worst_label} at {:.1}%). That is \
         not a failure of the fix — it means the module header's claim that the \
         ceiling never binds on shipped presets is now stale and the before/after \
         record must be re-taken.",
        100.0 * worst
    );
}

/// Synthetic under-powered spindle, used ONLY to reach the
/// `power_limited` branch. Not a shipped preset and not a
/// recommendation — see
/// [`a_power_limited_feed_lands_exactly_on_the_gate_ceiling`].
fn underpowered_machine() -> MachineProfile {
    let mut machine = MachineProfile::generic_wood_router();
    machine.name = "SYNTHETIC 0.05 kW (test only)".to_owned();
    machine.power = rs_cam_core::machine::PowerModel::ConstantPower { power_kw: 0.05 };
    machine
}

#[test]
fn a_power_limited_feed_lands_exactly_on_the_gate_ceiling() {
    // The no-double-count guard, and the only place the feed behaviour
    // is measurable at all — no shipped preset is power-limited (test
    // above), so a synthetic under-powered spindle reaches the branch.
    //
    // The Step 6 clamp works on the RAW axis (`required(raw_feed)` vs
    // `power_at_rpm`), and Step 9 then applies `safety_factor` to the
    // feed. The two compose so a power-limited op lands at exactly
    // `power_at_rpm · safety_factor` — the gate's own bound, 100 %
    // utilisation, no headroom wasted and none borrowed.
    //
    // Multiplying the Step 6 ceiling by `safety_factor` as well (the
    // naive reading of "Suggest gains machine.safety_factor") lands it
    // at `safety_factor²` instead: utilisation 75 %, feed 723.4 → 542.5
    // mm/min on this fixture. That extra derate is what drove
    // `flat_6mm_pocket_al6061_lut` to `major` in the literature matrix,
    // and it is a feed-moving recalibration Q3 did not authorise. This
    // assertion is what stops it being reintroduced by accident.
    let machine = underpowered_machine();
    let result = slot_cut(&machine, WoodSpecies::HardMaple, 12.0);
    assert!(
        result.power_limited,
        "the synthetic profile must reach the power-limited branch, else this \
         record is empty (power {:.4} kW vs ceiling {:.4} kW)",
        result.power_kw, result.available_power_kw
    );
    let gate_ceiling = gate_power_ceiling_kw(&machine, result.rpm);
    let utilisation = result.power_kw / gate_ceiling;
    eprintln!(
        "  {} | Ø12 slot maple: feed {:.1} mm/min, {:.5} kW of {:.5} kW gate \
         ceiling = {:.1}% (safety_factor {:.2})",
        machine.name,
        result.feed_rate_mm_min,
        result.power_kw,
        gate_ceiling,
        100.0 * utilisation,
        machine.safety_factor,
    );
    assert!(
        (utilisation - 1.0).abs() < 0.02,
        "a power-limited op must land ON the gate ceiling (100 %), not below it. \
         Observed utilisation {:.4}. Below 1.0 by roughly safety_factor ({:.3}) \
         means the Step 6 clamp is double-applying it.",
        utilisation,
        machine.safety_factor
    );
}

#[test]
fn the_power_limited_warning_compares_like_with_like() {
    // F-2's sibling: `FeedsWarning::PowerLimited` is rendered verbatim
    // by the modal and the properties panel. Its two terms must sit on
    // the same axis as each other and as the published pair, otherwise
    // the warning restates the defect it is reporting.
    let machine = underpowered_machine();
    let result = slot_cut(&machine, WoodSpecies::HardMaple, 12.0);
    let warning = result
        .warnings
        .iter()
        .find_map(|w| match w {
            rs_cam_core::feeds::FeedsWarning::PowerLimited {
                required_kw,
                available_kw,
            } => Some((*required_kw, *available_kw)),
            _ => None,
        })
        .expect("the synthetic profile must emit a PowerLimited warning");
    let gate_ceiling = gate_power_ceiling_kw(&machine, result.rpm);
    assert!(
        (warning.1 - gate_ceiling).abs() < 1e-9,
        "PowerLimited.available_kw {:.6} must be the gate ceiling {:.6}",
        warning.1,
        gate_ceiling
    );
    assert!(
        (warning.1 - result.available_power_kw).abs() < 1e-9,
        "PowerLimited.available_kw {:.6} must agree with the published \
         available_power_kw {:.6}",
        warning.1,
        result.available_power_kw
    );
    assert!(
        warning.0 > warning.1,
        "a PowerLimited warning must show required ({:.6}) above available \
         ({:.6}), else it does not explain the derate it accompanies",
        warning.0,
        warning.1
    );
}

#[test]
fn report_before_after_feeds_on_representative_fixtures() {
    // The ruling asked for before/after feed values on representative
    // fixtures. This prints them from live code so the commit body's
    // numbers are reproducible with
    // `cargo test -p rs_cam_core --test power_ceiling_parity_f2 -- \
    //  report_before_after_feeds_on_representative_fixtures --nocapture`
    // rather than being a quotation.
    eprintln!("\n── F-2 power-ceiling parity: representative fixtures ──────────");
    eprintln!(
        "{:<44} {:>7} {:>9} {:>6} {:>6} {:>8} {:>8} {:>8}",
        "machine / fixture", "rpm", "feed", "ap", "ae", "power kW", "ceil kW", "limited"
    );
    for (_preset_label, machine) in MachineProfile::presets() {
        for (label, result) in [
            (
                "slot Ø12x20 maple",
                slot_cut(&machine, WoodSpecies::HardMaple, 12.0),
            ),
            (
                "slot Ø12x20 white oak",
                slot_cut(&machine, WoodSpecies::WhiteOak, 12.0),
            ),
            ("light finish Ø3 pine", light_finish(&machine)),
        ] {
            eprintln!(
                "{:<44} {:>7.0} {:>9.1} {:>6.1} {:>6.1} {:>8.4} {:>8.4} {:>8}",
                format!("{} | {}", machine.name, label),
                result.rpm,
                result.feed_rate_mm_min,
                result.axial_depth_mm,
                result.radial_width_mm,
                result.power_kw,
                result.available_power_kw,
                result.power_limited,
            );
        }
    }
    eprintln!("───────────────────────────────────────────────────────────────\n");
}
