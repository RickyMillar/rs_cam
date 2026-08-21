//! G-SUGGEST-POWERSTALE — does the Step 6 power ceiling survive pass 9?
//!
//! `feeds::calculate` Step 6 clamps the feed so the predicted spindle
//! power stays inside the machine envelope. It computes that against the
//! CALCULATOR's operating point: `required = predicted_power_kw(kc,
//! mrr_cross_section(ap_calc, ae_calc), raw_feed)`.
//!
//! `enforce_invariants` then runs, and pass 9
//! (`rescale_feed_to_final_geometry`, added `a1bb964b` for
//! G-SUGGEST-NOCLAMP) re-solves the feed against the operation's FINAL
//! stepover and depth_per_pass. Nothing between pass 9 and the write
//! re-checks Step 6. Power scales with `ae · ap · feed`, so a rescale
//! that raises any of the three raises the load, and the clamp that was
//! satisfied at the calculator's geometry may not be satisfied at the
//! shipped one.
//!
//! This is an INSTRUMENT first and a sentry second. The question it has
//! to answer is not only "can the ceiling be outrun" but "is it
//! reachable in production", because
//! `power_ceiling_parity_f2::the_power_ceiling_does_not_bind_on_shipped_presets`
//! measured the power branch as never firing on any shipped preset —
//! peak utilisation 23.6 %, with rigidity and the machine cutting
//! ceiling binding first. If the answer is "real but unreachable on
//! shipped hardware" that is a latent defect, not an active one, and
//! this file should say so rather than imply a live hazard.
//!
//! ## Verdict, measured 2026-08-21: DOES NOT REPRODUCE
//!
//! The structural gap is real — nothing between pass 9 and the write
//! re-checks Step 6 — but the rescale cannot outrun the ceiling, and
//! that is not luck. Pass 9 holds the operation's IMPLIED CHIPLOAD
//! fixed and re-multiplies at the final geometry; that chipload was
//! derived from the feed Step 6 had already clamped, so the power
//! clamp rides through the rescale proportionally. Meanwhile
//! `enforce_invariants` clamps geometry DOWNWARD, shrinking the
//! cross-section that power scales with.
//!
//! Measured, on the synthetic under-powered spindle that is the only
//! place the branch is reachable at all:
//!
//! - single heaviest point: shipped load **59.9 %** of the gate ceiling
//!   (calculator 413.4 mm/min @ ap 3.000 -> shipped 413.0 @ ap 2.400)
//! - swept 70 requested depths across every tier boundary, **69 of them
//!   power-limited at the calculator**: peak shipped load **75.0 %**
//! - all shipped presets x all ten species: peak **26.6 %**
//!
//! The sweep matters more than the single point. `depth_tier_multiplier`
//! is STEPPED (1.0 / 0.75 / 0.50 / 0.45 at ap/D of 1 / 2 / 3), so a
//! clamp crossing a boundary downward buys up to 1.33x feed for an
//! arbitrarily small loss of cross-section — the one shape that could
//! outrun a ceiling checked before the rescale. It was searched
//! explicitly and does not.
//!
//! These arms stay as sentries rather than being deleted: they are the
//! thing that fails if a future change makes pass 9 re-solve from
//! something other than the clamped feed.
//!
//! NON-VACUITY IS LOAD-BEARING HERE. This project has already measured
//! three gates returning `Within` on `sample_range 0..0` — a bar written
//! as a verdict comparison is worthless until its population is checked.
//! Two arms below exist only to prove the headline arm is not vacuous:
//! that the fixture genuinely reaches the power branch, and that pass 9
//! genuinely moves the geometry. If either fails, the headline arm
//! proves nothing regardless of its verdict.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    // Same allowance the sibling power instrument takes: an instrument
    // that cannot report its measurement is not an instrument, and a
    // green run here should still print the margin it measured.
    clippy::print_stderr
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::{ApplyContext, ApplyScope, FeedsPreview, SuggestContext};
use rs_cam_core::feeds::{
    FeedsInput, FeedsResult, FeedsWarning, OperationFamily, PassRole, SetupContext,
    SpindleStrategy, ToolGeometryHint, calculate, embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// Every shipped wood species — the sweep population, same list the
/// sibling power instrument uses.
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

/// Ø12, not Ø6.
///
/// Measured while building this instrument: at Ø6 the fixture never
/// reaches the power branch at all. Asking for a full-width slot trips
/// `SlottingDetected`, which cuts the DOC to 1.5 mm BEFORE the Step 6
/// check, and the collapsed cross-section leaves the load far under the
/// ceiling — the headline arm passed while proving nothing. The sibling
/// instrument reaches the branch at Ø12 for the same reason, and the
/// non-vacuity arm below is what caught it.
const DIAMETER_MM: f64 = 12.0;

/// The power model, restated rather than imported.
///
/// `tool_load::power::predicted_power_kw` is `pub(crate)`, and the
/// sibling instrument makes the same call deliberately: a test that
/// imports the expression it checks can only prove the expression equals
/// itself. `GRAIN_ANISOTROPY_FACTOR` is 2.0.
fn predicted_power_kw(kc: f64, cross_section_mm2: f64, feed_mm_min: f64) -> f64 {
    2.0 * kc * cross_section_mm2 * feed_mm_min / 60_000_000.0
}

/// The gate's ceiling — `power_at_rpm × safety_factor`, the axis every
/// published power number is quoted against (`power.rs:214`).
fn gate_power_ceiling_kw(machine: &MachineProfile, rpm: f64) -> f64 {
    machine.power_at_rpm(rpm) * machine.safety_factor
}

/// Synthetic under-powered spindle. Not a shipped preset and not a
/// recommendation — it exists because no shipped preset reaches the
/// power branch at all, so it is the only place the interaction between
/// Step 6 and pass 9 is observable.
fn underpowered_machine() -> MachineProfile {
    let mut machine = MachineProfile::generic_wood_router();
    machine.name = "SYNTHETIC 0.05 kW (test only)".to_owned();
    machine.power = rs_cam_core::machine::PowerModel::ConstantPower { power_kw: 0.05 };
    machine
}

fn endmill() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = DIAMETER_MM;
    tool.flute_count = 2;
    tool
}

/// A heavy 2.5D rough: full-width slot at a deliberately aggressive DOC,
/// so the rigidity clamp inside `enforce_invariants` HAS something to
/// take away — which is what makes pass 9 fire.
fn heavy_pocket() -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: DIAMETER_MM,
        depth: 4.0 * DIAMETER_MM,
        depth_per_pass: 2.0 * DIAMETER_MM,
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        climb: true,
        ..PocketConfig::default()
    })
}

fn feeds_input<'a>(
    machine: &'a MachineProfile,
    material: &'a Material,
    ap_mm: f64,
    ae_mm: f64,
) -> FeedsInput<'a> {
    FeedsInput {
        tool_diameter: DIAMETER_MM,
        flute_count: 2,
        flute_length: 3.0 * DIAMETER_MM,
        shank_diameter: None,
        tool_geometry: ToolGeometryHint::Flat,
        material,
        machine,
        operation: OperationFamily::Pocket,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: Some(ap_mm),
        radial_width_mm: Some(ae_mm),
        target_scallop_mm: None,
        vendor_lut: Some(embedded_vendor_lut()),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MatchChart,
    }
}

struct Shipped {
    recommended: FeedsResult,
    feed_mm_min: f64,
    ap_mm: f64,
    ae_mm: f64,
}

/// Run the WHOLE funnel — `calculate` → `apply_feeds_subset` →
/// `enforce_invariants` (which is where pass 9 lives) — and report what
/// actually lands on the operation. Measuring the shipped operation, not
/// the calculator's recommendation, is the entire point: the two are
/// different numbers and only one of them reaches the machine.
fn run_funnel(machine: &MachineProfile, material: &Material) -> Shipped {
    run_funnel_at(machine, material, 2.0 * DIAMETER_MM)
}

fn run_funnel_at(machine: &MachineProfile, material: &Material, requested_ap: f64) -> Shipped {
    let tool = endmill();
    let mut operation = heavy_pocket();
    if let OperationConfig::Pocket(cfg) = &mut operation {
        cfg.depth_per_pass = requested_ap;
        cfg.depth = 2.0 * requested_ap;
    }
    let mut provenance = rs_cam_core::feeds::FeedsProvenance::default();

    let input = feeds_input(machine, material, requested_ap, DIAMETER_MM);
    let preview = FeedsPreview::build(&input);
    let rec = preview
        .applicable()
        .expect("heavy pocket with a flat endmill is a runnable pairing");
    let recommended = rec.result().clone();

    let _warnings = rs_cam_core::feeds::suggest::apply(
        &rec,
        ApplyScope::Both,
        &mut operation,
        &mut provenance,
        ApplyContext {
            tool: &tool,
            machine,
            material,
            pass_role: PassRole::Roughing,
            suggest: SuggestContext::default(),
        },
    );

    Shipped {
        feed_mm_min: operation.feed_rate(),
        ap_mm: operation.depth_per_pass().unwrap_or(0.0),
        ae_mm: operation.stepover().unwrap_or(0.0),
        recommended,
    }
}

/// Utilisation of the gate ceiling implied by the SHIPPED operating
/// point. 1.0 means exactly at the ceiling; above 1.0 means the funnel
/// commanded a cut the spindle cannot deliver.
fn shipped_utilisation(machine: &MachineProfile, material: &Material, s: &Shipped) -> Option<f64> {
    let kc = material.kc_n_per_mm2()?;
    let cross_section = ToolGeometryHint::Flat.mrr_cross_section_mm2(s.ap_mm, s.ae_mm);
    let required = predicted_power_kw(kc, cross_section, s.feed_mm_min);
    let ceiling = gate_power_ceiling_kw(machine, s.recommended.rpm);
    if ceiling <= 0.0 {
        return None;
    }
    // Both terms on the gate's COMMANDED axis, matching every published
    // power number and the sibling instrument.
    Some(required * machine.safety_factor / ceiling)
}

// ── Non-vacuity guards ───────────────────────────────────────────────

/// If the calculator never power-limits this fixture, the headline arm
/// is comparing against a ceiling that was never engaged and would pass
/// no matter what pass 9 did.
#[test]
fn the_fixture_actually_reaches_the_power_branch() {
    let machine = underpowered_machine();
    let material = Material::SolidWood {
        species: WoodSpecies::Ipe,
    };
    let input = feeds_input(&machine, &material, 2.0 * DIAMETER_MM, DIAMETER_MM);
    let result = calculate(&input);

    assert!(
        result
            .warnings
            .iter()
            .any(|w| matches!(w, FeedsWarning::PowerLimited { .. })),
        "fixture is vacuous: the calculator never engaged the Step 6 power \
         clamp, so the headline arm would pass regardless of pass 9. \
         warnings: {:?}",
        result.warnings
    );
}

/// If `enforce_invariants` leaves the geometry alone, pass 9
/// short-circuits (it only re-solves when a pass actually moved the
/// stepover or depth_per_pass) and the headline arm proves nothing.
#[test]
fn pass_nine_actually_moves_the_geometry() {
    let machine = underpowered_machine();
    let material = Material::SolidWood {
        species: WoodSpecies::Ipe,
    };
    let shipped = run_funnel(&machine, &material);

    let moved = (shipped.ap_mm - 2.0 * DIAMETER_MM).abs() > 1e-6
        || (shipped.ae_mm - DIAMETER_MM).abs() > 1e-6;
    assert!(
        moved,
        "fixture is vacuous: enforce_invariants left the requested geometry \
         untouched (ap {:.4} mm, ae {:.4} mm), so pass 9 never re-solved and \
         the headline arm cannot observe the interaction it exists to test",
        shipped.ap_mm, shipped.ae_mm
    );
}

// ── The claim ────────────────────────────────────────────────────────

/// The headline. Whatever pass 9 does to the feed, the operating point
/// that reaches the machine must still fit inside the spindle envelope.
#[test]
fn the_shipped_feed_respects_the_power_ceiling_after_the_rescale() {
    let machine = underpowered_machine();
    let material = Material::SolidWood {
        species: WoodSpecies::Ipe,
    };
    let shipped = run_funnel(&machine, &material);
    let utilisation =
        shipped_utilisation(&machine, &material, &shipped).expect("Ipe publishes a Kc");

    eprintln!(
        "  G-SUGGEST-POWERSTALE | {} / Ipe Ø{DIAMETER_MM}: calculator {:.1} mm/min \
         @ ap {:.3} ae {:.3} -> shipped {:.1} mm/min @ ap {:.3} ae {:.3}; \
         shipped load = {:.1}% of the gate ceiling",
        machine.name,
        shipped.recommended.feed_rate_mm_min,
        shipped.recommended.axial_depth_mm,
        shipped.recommended.radial_width_mm,
        shipped.feed_mm_min,
        shipped.ap_mm,
        shipped.ae_mm,
        100.0 * utilisation,
    );

    assert!(
        utilisation <= 1.02,
        "G-SUGGEST-POWERSTALE: the SHIPPED operating point implies {:.1}% of \
         the spindle's gate ceiling.\n    \
         calculator recommended feed {:.1} mm/min at ap {:.3} / ae {:.3}; \
         the operation ships feed {:.1} mm/min at ap {:.3} / ae {:.3}.\n    \
         Step 6 clamped the feed against the CALCULATOR's geometry, pass 9 \
         re-solved it against the FINAL geometry, and nothing re-checked the \
         power envelope in between.",
        100.0 * utilisation,
        shipped.recommended.feed_rate_mm_min,
        shipped.recommended.axial_depth_mm,
        shipped.recommended.radial_width_mm,
        shipped.feed_mm_min,
        shipped.ap_mm,
        shipped.ae_mm,
    );
}

/// Reachability. If this passes with a large margin, G-SUGGEST-POWERSTALE
/// is latent — real in the code, unreachable on hardware anyone ships —
/// and should be recorded as such rather than as a live hazard.
#[test]
fn shipped_presets_stay_clear_of_the_ceiling_after_the_rescale() {
    let mut worst = 0.0_f64;
    let mut worst_label = String::new();
    let mut checked = 0usize;

    for (preset_label, machine) in MachineProfile::presets() {
        for species in ALL_SPECIES {
            let material = Material::SolidWood { species };
            let shipped = run_funnel(&machine, &material);
            if let Some(u) = shipped_utilisation(&machine, &material, &shipped) {
                checked += 1;
                if u > worst {
                    worst = u;
                    worst_label = format!("{preset_label} / {species:?}");
                }
            }
        }
    }

    assert!(
        checked > 0,
        "sweep population is empty — no preset × species pair produced a \
         measurable utilisation, so this arm is vacuous"
    );
    eprintln!(
        "  G-SUGGEST-POWERSTALE | peak shipped-load utilisation across \
         {checked} shipped preset x species pairs: {:.1}% (worst: {worst_label})",
        100.0 * worst
    );

    assert!(
        worst <= 1.0,
        "a SHIPPED preset now exceeds the spindle power ceiling after the \
         pass-9 rescale: peak {:.1}% at {worst_label} across {checked} \
         preset × species pairs. This moves G-SUGGEST-POWERSTALE from latent \
         to live.",
        100.0 * worst
    );
}

/// Search, not a single point.
///
/// The headline arm above measures ONE operating point, and on that
/// point the load falls rather than rises — `enforce_invariants` clamped
/// ap 3.000 -> 2.400 while pass 9 left the feed alone, so utilisation
/// went 100% -> 59.9%. A negative result from one fixture is a weak
/// negative, and the mechanism names the regime where it should NOT
/// hold: `depth_tier_multiplier` is STEPPED (1.0 / 0.75 / 0.50 / 0.45 at
/// ap/D of 1 / 2 / 3), so an ap clamp that crosses a boundary DOWNWARD
/// buys up to a 1.33x feed rise for an arbitrarily small reduction in
/// cross-section. Power goes as `ap · ae · feed`, so that is the shape
/// that can outrun a ceiling checked before the rescale.
///
/// This arm sweeps requested ap across every tier boundary at a fine
/// step and reports the PEAK shipped load. It is the arm that decides
/// whether G-SUGGEST-POWERSTALE is unreachable or merely unreached.
#[test]
fn no_requested_depth_lets_the_rescale_outrun_the_ceiling() {
    let machine = underpowered_machine();
    let material = Material::SolidWood {
        species: WoodSpecies::Ipe,
    };

    let mut worst = 0.0_f64;
    let mut worst_ap = 0.0_f64;
    let mut worst_shipped = 0.0_f64;
    let mut reached_branch = 0usize;
    let mut checked = 0usize;

    // ap/D from 0.05 to 3.5 in 0.05 steps — straddles all three tier
    // boundaries (1, 2, 3) with plenty of points either side.
    for step in 1..=70 {
        let requested_ap = DIAMETER_MM * (step as f64) * 0.05;
        let probe = feeds_input(&machine, &material, requested_ap, DIAMETER_MM);
        if calculate(&probe).power_limited {
            reached_branch += 1;
        }
        let shipped = run_funnel_at(&machine, &material, requested_ap);
        if let Some(u) = shipped_utilisation(&machine, &material, &shipped) {
            checked += 1;
            if u > worst {
                worst = u;
                worst_ap = requested_ap;
                worst_shipped = shipped.feed_mm_min;
            }
        }
    }

    eprintln!(
        "  G-SUGGEST-POWERSTALE | swept {checked} requested depths, {reached_branch} \
         of them power-limited at the calculator; peak shipped load {:.1}% of the \
         gate ceiling at requested ap {:.3} mm (shipped feed {:.1} mm/min)",
        100.0 * worst,
        worst_ap,
        worst_shipped,
    );

    assert!(
        reached_branch > 0,
        "sweep is vacuous: no requested depth reached the Step 6 power clamp, \
         so nothing here constrains the rescale"
    );
    assert!(
        worst <= 1.02,
        "G-SUGGEST-POWERSTALE reproduces: peak shipped load {:.1}% of the gate \
         ceiling at requested ap {:.3} mm (shipped feed {:.1} mm/min). Step 6 \
         checked power at the calculator geometry; pass 9 re-solved the feed at \
         the final geometry; nothing re-checked in between.",
        100.0 * worst,
        worst_ap,
        worst_shipped,
    );
}
