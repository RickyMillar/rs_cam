//! G-SUGGEST-POWERSTALE — does the Step 6 power ceiling survive pass 9?
//!
//! `feeds::calculate` Step 6 clamps the feed so the predicted spindle
//! power stays inside the machine envelope. It computes that against the
//! CALCULATOR's operating point: `required = predicted_power_kw(kc,
//! mrr_cross_section(ap_calc, ae_calc), raw_feed)`.
//!
//! `enforce_invariants` then runs, and pass 9
//! (`rescale_feed_to_final_geometry`, added `180c6f26` for
//! G-SUGGEST-NOCLAMP) re-solves the feed against the operation's FINAL
//! stepover and depth_per_pass. Nothing between pass 9 and the write
//! re-checks Step 6. Power scales with `ae · ap · feed`, so a rescale
//! that raises any of the three raises the load, and the clamp that was
//! satisfied at the calculator's geometry may not be satisfied at the
//! shipped one.
//!
//! This is an INSTRUMENT first and a sentry second. The question it has
//! to answer is not only "can the ceiling be outrun" but "is it
//! reachable in production".
//!
//! ## R1 (2026-09-16) — what moved
//!
//! Three things, none of them the structural claim:
//!
//! 1. **The power model.** `predicted_power_kw` below was the pre-R1
//!    linear form; it is now the two-term affine model the engine
//!    actually uses. Left alone it would have kept this instrument
//!    green while measuring a power nothing predicts.
//! 2. **The synthetic spindle: 0.05 kW → 0.58 kW.** Under the two-term
//!    model the Ø12 Ipe slot's feed-free EDGE term alone is 0.344 kW,
//!    so a 0.05 kW spindle cannot make that cut at ANY feed — the
//!    clamp correctly refuses, the feed is left alone, and the fixture
//!    stops exercising the clamp arithmetic. 0.58 kW puts the gate
//!    ceiling at 0.435 kW, above the edge floor and below the 0.522 kW
//!    the unclamped cut draws, so the clamp binds and has a feed
//!    answer.
//! 3. **A defect in this instrument's own arithmetic.**
//!    `shipped_utilisation` multiplied `required` by `safety_factor`
//!    while the ceiling already carried it, reporting every number here
//!    25 % low. Removing it RAISES every figure below; no assertion
//!    changed.
//!
//! Reachability itself moved:
//! `power_ceiling_parity_f2::the_power_ceiling_binds_on_three_shipped_fixtures`
//! now measures peak utilisation at 80 % with three shipped fixtures
//! reaching the branch, against 23.6 % and none before R1. So the "real
//! but unreachable on shipped hardware" framing below no longer holds —
//! the branch is reachable. What did NOT change is the structural
//! verdict: the pass-9 rescale still does not outrun the clamp.
//!
//! ## Verdict, measured 2026-08-21, re-measured 2026-09-16: DOES NOT REPRODUCE
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
//! Measured on the synthetic under-powered spindle (numbers re-taken
//! 2026-09-16 under R1's model, the 0.58 kW fixture and the corrected
//! `shipped_utilisation`; the pre-R1 figures are in brackets):
//!
//! - single heaviest point: shipped load **80.0 %** of the gate ceiling
//!   [59.9 %]
//! - swept 70 requested depths across every tier boundary, **66 of them
//!   power-limited at the calculator** [69]: peak shipped load **96.0 %**
//!   [75.0 %]
//! - all shipped presets x all ten species: peak **100.013 %** [26.6 %],
//!   which is one whole-mm/min feed-rounding step above the ceiling and
//!   not a rescale effect — see
//!   [`shipped_presets_stay_clear_of_the_ceiling_after_the_rescale`]
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
//! ## T-15 (2026-09-18) — the verdict holds for THESE fixtures, and only them
//!
//! No arm here changed, and none needed to. The verdict above is a statement
//! about the fixtures this file builds, and **not one of them crosses a depth
//! tier boundary**. Every fixture asks for a full-width slot, so
//! `SlottingDetected` caps the depth at 0.25 × D before Step 6 ever runs, and
//! `depth_tier_multiplier` reads 1.00 on both sides of every clamp. Pass 9
//! then short-circuits on an unchanged factor, which is why the sweep over 70
//! requested depths found nothing: the requested depth is not the depth the
//! calculator sizes the feed at.
//!
//! T-15 built the crossing case — a 2D `Adaptive` rough at 2.00417 × D that
//! the rigidity clamp takes to exactly 2.0 × D, tier 0.50 → 0.75, feed × 1.5
//! — and it reproduces: 110.67 % of the gate ceiling on a recipe Step 6 had
//! clamped ONTO that ceiling. Pass 10 (`recheck_power_after_rescale`) now
//! re-evaluates the ceiling at the shipped operating point. The fixture and
//! the arithmetic are in
//! `tests/a_rescaled_feed_stays_inside_the_power_ceiling_g_t15.rs`.
//!
//! The general rule the register draws from this stands: **a sweep that does
//! not fire is evidence about the sweep.**
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
/// `tool_load::power::PowerTerms` is `pub(crate)`, and the sibling
/// instrument makes the same call deliberately: a test that imports the
/// expression it checks can only prove the expression equals itself.
/// `GRAIN_ANISOTROPY_FACTOR` is 2.0.
///
/// R1 (2026-09-16) re-baseline. This was
/// `2.0 · kc · cross_section · feed / 60e6` — the pre-R1 linear model.
/// Power is now the two-term affine form:
///
/// ```text
/// P = A · ( Ks · cross_section · feed  +  F_edge · ap · π·D·n · z·ψ/2π ) / 60e6
/// ```
///
/// with `(Ks, F_edge) = (49.95, 5.30) · kc/35.1` — the woodresearch.sk
/// 201905/12 fit `feeds::force` owns — and `cos ψ = 1 − ae/r`. Leaving
/// the old expression here would have kept this instrument green while
/// measuring a power the engine no longer predicts.
#[allow(clippy::too_many_arguments)]
fn predicted_power_kw(
    kc: f64,
    cross_section_mm2: f64,
    feed_mm_min: f64,
    ap_mm: f64,
    ae_mm: f64,
    diameter_mm: f64,
    rpm: f64,
    flutes: f64,
) -> f64 {
    const ANISOTROPY: f64 = 2.0;
    let scale = kc / 35.1;
    let (ks, f_edge) = (49.95 * scale, 5.30 * scale);
    let psi = (1.0 - ae_mm / (diameter_mm / 2.0)).clamp(-1.0, 1.0).acos();
    let duty = flutes * psi / std::f64::consts::TAU;
    let vc = std::f64::consts::PI * diameter_mm * rpm;
    ANISOTROPY * (ks * cross_section_mm2 * feed_mm_min + f_edge * ap_mm * vc * duty) / 60_000_000.0
}

/// The gate's ceiling — `power_at_rpm × safety_factor`, the axis every
/// published power number is quoted against (`power.rs:214`).
fn gate_power_ceiling_kw(machine: &MachineProfile, rpm: f64) -> f64 {
    machine.power_at_rpm(rpm) * machine.safety_factor
}

/// Synthetic under-powered spindle. Not a shipped preset and not a
/// recommendation — it exists to put the Step 6 / pass 9 interaction
/// under a clamp that actually binds, on a cut the clamp can solve.
/// See the R1 note in the module header for why 0.05 kW stopped doing
/// that.
fn underpowered_machine() -> MachineProfile {
    let mut machine = MachineProfile::generic_wood_router();
    machine.name = "SYNTHETIC 0.58 kW (test only)".to_owned();
    machine.power = rs_cam_core::machine::PowerModel::ConstantPower { power_kw: 0.58 };
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
    let required = predicted_power_kw(
        kc,
        cross_section,
        s.feed_mm_min,
        s.ap_mm,
        s.ae_mm,
        DIAMETER_MM,
        s.recommended.rpm,
        2.0,
    );
    let ceiling = gate_power_ceiling_kw(machine, s.recommended.rpm);
    if ceiling <= 0.0 {
        return None;
    }
    // Both terms on the gate's COMMANDED axis, matching every published
    // power number and the sibling instrument: `s.feed_mm_min` is the
    // feed that SHIPS, so Step 9 has already applied `safety_factor` to
    // it, and `ceiling` already carries the factor on the other side.
    //
    // R1 (2026-09-16): this line read `required * machine.safety_factor
    // / ceiling`, multiplying the factor in a second time and reporting
    // every utilisation in this file 25 % low. The bar was therefore
    // looser than the comment above it claimed. Removing it RAISES every
    // number this instrument prints; the assertions are unchanged.
    Some(required / ceiling)
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

/// Reachability. Pre-R1 this passed with a large margin — the power
/// branch was unreachable on shipped hardware, so G-SUGGEST-POWERSTALE
/// was latent rather than live.
///
/// R1 (2026-09-16) re-baseline: the branch is now reachable, and the
/// worst shipped pair sits at **100.013 %** of the gate ceiling. The
/// cause is measured, and it is NOT the pass-9 rescale this file is
/// named for — the rescale leaves the geometry alone here:
///
/// ```text
/// calculator 323.6993 mm/min @ ap 3.0000 -> shipped 324.0000 mm/min @ ap 3.0000
/// ```
///
/// `apply_feeds_subset` writes `round_suggestion_value(feed, 1.0)`
/// (`suggest.rs:874`), a whole mm/min. Step 6's clamp now lands the
/// recommendation EXACTLY on the ceiling, so rounding 323.6993 up to
/// 324 — +0.093 % of feed — ships +0.013 % of power. Pre-R1 nothing sat
/// near the ceiling, so the rounding never showed.
///
/// The bound therefore allows one rounding step and no more: **0.2 %**,
/// ten times tighter than the headline arm's 2 %, and 15× the measured
/// overshoot. A model-scale movement cannot hide inside it. Logged as
/// T-9 in `planning/TECH_DEBT_REGISTER.md`.
#[test]
fn shipped_presets_stay_clear_of_the_ceiling_after_the_rescale() {
    let mut worst = 0.0_f64;
    let mut worst_label = String::new();
    let mut checked = 0usize;

    let mut worst_detail = String::new();
    for (preset_label, machine) in MachineProfile::presets() {
        for species in ALL_SPECIES {
            let material = Material::SolidWood { species };
            let shipped = run_funnel(&machine, &material);
            if let Some(u) = shipped_utilisation(&machine, &material, &shipped) {
                checked += 1;
                if u > worst {
                    worst = u;
                    worst_label = format!("{preset_label} / {species:?}");
                    worst_detail = format!(
                        "calculator {:.4} mm/min @ ap {:.4} -> shipped {:.4} mm/min @ ap {:.4}",
                        shipped.recommended.feed_rate_mm_min,
                        shipped.recommended.axial_depth_mm,
                        shipped.feed_mm_min,
                        shipped.ap_mm,
                    );
                }
            }
        }
    }
    eprintln!("  worst: {worst_detail}");

    assert!(
        checked > 0,
        "sweep population is empty — no preset × species pair produced a \
         measurable utilisation, so this arm is vacuous"
    );
    eprintln!(
        "  G-SUGGEST-POWERSTALE | peak shipped-load utilisation across \
         {checked} shipped preset x species pairs: {:.3}% (worst: {worst_label})",
        100.0 * worst
    );

    assert!(
        worst <= 1.002,
        "a SHIPPED preset exceeds the spindle power ceiling by more than one \
         feed-rounding step after the pass-9 rescale: peak {:.3}% at \
         {worst_label} across {checked} preset × species pairs ({worst_detail}). \
         0.2% is the whole-mm/min rounding allowance; anything above it is a \
         model or ordering movement, not rounding.",
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
