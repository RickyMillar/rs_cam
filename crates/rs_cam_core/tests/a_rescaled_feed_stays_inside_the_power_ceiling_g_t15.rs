//! T-15 — the feed pass 9 re-derives must still fit the spindle.
//!
//! `feeds::calculate` Step 6 checks the spindle power ceiling at the geometry
//! it was handed. `enforce_invariants` then runs, and pass 9
//! (`rescale_feed_to_final_geometry`) re-multiplies the feed by
//! `depth_tier_multiplier` at the FINAL depth. That multiplier RISES as the
//! depth falls — 1.00 / 0.75 / 0.50 / 0.45 at `ap/D` of 1 / 2 / 3 — so a clamp
//! that lowers the depth across a tier boundary makes pass 9 RAISE the feed,
//! by up to 2.22×, for a depth loss that can be arbitrarily small. Until T-15
//! nothing re-checked Step 6 afterwards.
//!
//! ## The fixture, and why the older instrument missed it
//!
//! `suggest_power_ceiling_after_pass9_g_suggest_powerstale` searched for this
//! and reported "does not reproduce". Its verdict holds for ITS fixtures: it
//! asks for a full-width slot, `SlottingDetected` caps the depth at 0.25 × D
//! before Step 6, and every point it measures sits inside the first tier, so
//! the multiplier is 1.00 before and after every clamp and pass 9 short-
//! circuits on an unchanged factor.
//!
//! The crossing case needs a depth ABOVE a tier boundary that a clamp brings
//! just below it. The fixture here is the register's own proposal:
//!
//! - an `Adaptive` (2D) roughing operation, which is Adaptive feeds family, so
//!   the rigidity clamp reads `rigidity.adaptive_doc_factor`, and which is
//!   outside the axial-envelope pass's routing, so pass 0 leaves the depth
//!   alone;
//! - `adaptive_doc_factor = 2.0` — the shipped `RigidityProfile::default()`
//!   value — on a Ø12 tool, so the rigidity cap lands on 24.000 mm, exactly
//!   2.0 × D;
//! - an operator-set depth of 24.050 mm, 2.00417 × D, one step above the
//!   boundary.
//!
//! The rigidity clamp then takes the depth 24.050 → 24.000 mm, a loss of
//! 0.21 %, and the depth tier goes 0.50 → 0.75, a feed rise of exactly 1.5×.
//!
//! ## What that does to spindle power
//!
//! Power is affine in the feed: `P = shear·feed + edge`. The shear term
//! carries the cross-section `ap · ae`, the edge term carries `ap` alone, and
//! neither carries the depth tier. So the shipped load is
//!
//! ```text
//! P_ship / P_calc = (ap_final / ap_calc) · (1 + (tier_ratio − 1) · shear_share)
//! ```
//!
//! with `tier_ratio = 0.75 / 0.50 = 1.5` and `shear_share = shear / (shear +
//! edge)` at the calculator's point. The register's "about 1.46×" is the
//! pure-shear bound; the edge term carries no feed, so the real figure is
//! lower and the arms below quote the measured split.
//!
//! ## The arms
//!
//! - [`the_shipped_feed_fits_the_spindle_after_a_tier_crossing_rescale`] — the
//!   breach, on a recipe Step 6 clamped ONTO the ceiling.
//! - [`pass_nine_raises_the_feed_across_the_tier_boundary`] — non-vacuity: the
//!   mechanism fires. Without it the headline arm proves an absence.
//! - [`an_untouched_operation_keeps_its_feed_byte_identical`] — pass 10's gate.
//! - [`no_feed_fits_when_the_edge_term_alone_is_over_budget`] — the refusal.
//! - [`an_unclamped_recipe_is_not_lifted_over_the_ceiling`] — the case the
//!   register's cheaper proposal would have missed. That proposal was to
//!   refuse the raise when `FeedsDerates::power_limit < 1.0`, i.e. only when
//!   Step 6 had already clamped. This fixture is one Step 6 never clamped —
//!   85 % of the ceiling, under the 89.4 % p90 the register re-measured after
//!   R1 — and pass 9 lifts it over.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    // SAFETY: a sentry that measures a utilisation should print the margin it
    // measured, the same allowance the sibling power instruments take.
    clippy::print_stderr
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::AdaptiveConfig;
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::{
    ApplyContext, ApplyScope, FeedsPreview, SuggestContext, SuggestWarning,
};
use rs_cam_core::feeds::{
    FeedsInput, FeedsResult, FeedsWarning, OperationFamily, PassRole, SetupContext,
    SpindleStrategy, ToolGeometryHint, embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

const DIAMETER_MM: f64 = 12.0;
const FLUTES: u32 = 2;

/// 2.00417 × D — one step above the `ap/D > 2.0` tier boundary, so the
/// calculator sizes the feed with `depth_tier_multiplier = 0.50`.
const ENTRY_DPP_MM: f64 = 24.05;

/// The rigidity cap: `adaptive_doc_factor × D = 2.0 × 12 = 24.0`, exactly
/// 2.0 × D, so the clamped depth sits in the 0.75 tier.
const CLAMPED_DPP_MM: f64 = 24.0;

/// 0.833 × D. Below the 0.85 × D slotting threshold, so `SlottingDetected`
/// does not cap the depth before Step 6 — the failure that kept the older
/// instrument inside the first tier.
const STEPOVER_MM: f64 = 10.0;

/// A depth no pass moves: under the 24.0 mm rigidity cap, under the 40 mm
/// cutting length, under the 32 mm flute guard.
const UNTOUCHED_DPP_MM: f64 = 12.0;

/// The power model, restated rather than imported.
///
/// `tool_load::power::PowerTerms` is `pub(crate)`, and both sibling power
/// instruments make the same call deliberately: a test that imports the
/// expression it checks can only prove the expression equals itself.
///
/// ```text
/// P = A · ( Ks · cross_section · feed  +  F_edge · ap · π·D·n · z·ψ/2π ) / 60e6
/// ```
///
/// with `A = GRAIN_ANISOTROPY_FACTOR = 2.0`, `(Ks, F_edge) = (49.95, 5.30) ·
/// kc/35.1` — the woodresearch.sk 201905/12 fit `feeds::force` owns — and
/// `cos ψ = 1 − ae/r`. Returns the two terms apart, because the whole
/// mechanism turns on which of them carries the feed.
fn power_terms_kw(kc: f64, ap_mm: f64, ae_mm: f64, feed_mm_min: f64, rpm: f64) -> (f64, f64) {
    const ANISOTROPY: f64 = 2.0;
    let scale = kc / 35.1;
    let (ks, f_edge) = (49.95 * scale, 5.30 * scale);
    let cross_section = ToolGeometryHint::Flat.mrr_cross_section_mm2(ap_mm, ae_mm);
    let psi = (1.0 - ae_mm / (DIAMETER_MM / 2.0)).clamp(-1.0, 1.0).acos();
    let duty = f64::from(FLUTES) * psi / std::f64::consts::TAU;
    let vc = std::f64::consts::PI * DIAMETER_MM * rpm;
    let shear = ANISOTROPY * ks * cross_section * feed_mm_min / 60_000_000.0;
    let edge = ANISOTROPY * f_edge * ap_mm * vc * duty / 60_000_000.0;
    (shear, edge)
}

/// The gate's ceiling — `power_at_rpm × safety_factor`, the axis every
/// published power number is quoted against (`power.rs:214`).
fn gate_ceiling_kw(machine: &MachineProfile, rpm: f64) -> f64 {
    machine.power_at_rpm(rpm) * machine.safety_factor
}

/// A Ø12 two-flute carbide end mill long enough that neither the flute guard
/// (0.8 × flute length = 32 mm) nor the cutting-length clamp reaches the
/// 24.05 mm depth, and stiff enough that the deflection back-off does not
/// fire. All four are fixture choices, not model constants.
fn tool() -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    t.diameter = DIAMETER_MM;
    t.flute_count = FLUTES;
    t.cutting_length = 40.0;
    t.stickout = 50.0;
    t
}

/// Synthetic spindle. Not a shipped preset and not a recommendation — it
/// exists to put the Step 6 / pass 9 interaction under a ceiling that binds.
/// `adaptive_doc_factor` is set to 2.0, the shipped `RigidityProfile::default()`
/// value; the generic router ships 1.5, which caps at 1.5 × D and lands inside
/// the same tier the entry depth is above.
fn machine_at(power_kw: f64) -> MachineProfile {
    let mut m = MachineProfile::generic_wood_router();
    m.name = format!("SYNTHETIC {power_kw:.2} kW (test only)");
    m.power = rs_cam_core::machine::PowerModel::ConstantPower { power_kw };
    m.rigidity.adaptive_doc_factor = 2.0;
    m
}

fn adaptive_op(dpp: f64) -> OperationConfig {
    OperationConfig::Adaptive(AdaptiveConfig {
        stepover: STEPOVER_MM,
        // Two full passes, so `realised_step_down` snaps the depth to itself
        // and the entry value the invariants see is the requested one.
        depth: 2.0 * dpp,
        depth_per_pass: dpp,
        feed_rate: 1000.0,
        plunge_rate: 400.0,
        ..AdaptiveConfig::default()
    })
}

fn feeds_input<'a>(machine: &'a MachineProfile, material: &'a Material, ap: f64) -> FeedsInput<'a> {
    FeedsInput {
        tool_diameter: DIAMETER_MM,
        flute_count: FLUTES,
        flute_length: 40.0,
        shank_diameter: None,
        tool_geometry: ToolGeometryHint::Flat,
        material,
        machine,
        operation: OperationFamily::Adaptive,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        // Pinned, so the Step 6 ladder's geometry rungs stand down and the
        // feed rung is the only one that can move: the fixture isolates the
        // pass-9 interaction rather than the ladder.
        axial_depth_mm: Some(ap),
        radial_width_mm: Some(STEPOVER_MM),
        target_scallop_mm: None,
        vendor_lut: Some(embedded_vendor_lut()),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MatchChart,
    }
}

struct Shipped {
    recommended: FeedsResult,
    warnings: Vec<SuggestWarning>,
    feed_mm_min: f64,
    ap_mm: f64,
    ae_mm: f64,
}

impl Shipped {
    /// Utilisation of the gate ceiling at the operating point that REACHES the
    /// machine. 1.0 is exactly on the ceiling; above 1.0 the funnel commanded
    /// a cut the spindle cannot deliver.
    ///
    /// Both sides sit on the gate's COMMANDED axis: `feed_mm_min` is the feed
    /// that ships, so Step 9 has already applied `safety_factor` to it, and
    /// the ceiling carries the factor on the other side. Applying it twice is
    /// the defect the sibling instrument corrected in its own arithmetic.
    fn shipped_utilisation(&self, machine: &MachineProfile, material: &Material) -> f64 {
        let kc = material
            .kc_n_per_mm2()
            .expect("fixture species publishes a Kc");
        let (shear, edge) = power_terms_kw(
            kc,
            self.ap_mm,
            self.ae_mm,
            self.feed_mm_min,
            self.recommended.rpm,
        );
        (shear + edge) / gate_ceiling_kw(machine, self.recommended.rpm)
    }

    /// The same, at the point the CALCULATOR derived its feed at.
    fn calculator_utilisation(&self, machine: &MachineProfile, material: &Material) -> f64 {
        let kc = material
            .kc_n_per_mm2()
            .expect("fixture species publishes a Kc");
        let (shear, edge) = power_terms_kw(
            kc,
            self.recommended.axial_depth_mm,
            self.recommended.radial_width_mm,
            self.recommended.feed_rate_mm_min,
            self.recommended.rpm,
        );
        (shear + edge) / gate_ceiling_kw(machine, self.recommended.rpm)
    }

    /// Fraction of the calculator-point load that the feed carries.
    fn calculator_shear_share(&self, material: &Material) -> f64 {
        let kc = material
            .kc_n_per_mm2()
            .expect("fixture species publishes a Kc");
        let (shear, edge) = power_terms_kw(
            kc,
            self.recommended.axial_depth_mm,
            self.recommended.radial_width_mm,
            self.recommended.feed_rate_mm_min,
            self.recommended.rpm,
        );
        shear / (shear + edge)
    }

    fn rescale(&self) -> Option<(f64, f64)> {
        self.warnings.iter().find_map(|w| match w {
            SuggestWarning::FeedRescaledToFinalGeometry {
                requested_mm_per_min,
                rescaled_mm_per_min,
                ..
            } => Some((*requested_mm_per_min, *rescaled_mm_per_min)),
            _ => None,
        })
    }

    fn power_recheck(&self) -> Option<(f64, f64, bool)> {
        self.warnings.iter().find_map(|w| match w {
            SuggestWarning::PowerRecheckedAfterRescale {
                rescaled_mm_per_min,
                shipped_mm_per_min,
                fits_at_any_feed,
                ..
            } => Some((*rescaled_mm_per_min, *shipped_mm_per_min, *fits_at_any_feed)),
            _ => None,
        })
    }
}

/// Run the WHOLE funnel — `calculate` → `apply_feeds_subset` →
/// `enforce_invariants` — and report what lands on the operation. Measuring
/// the shipped operation, not the calculator's recommendation, is the point:
/// the two are different numbers and only one of them reaches the machine.
fn run_funnel(machine: &MachineProfile, material: &Material, requested_ap: f64) -> Shipped {
    let tool = tool();
    let mut operation = adaptive_op(requested_ap);
    let mut provenance = rs_cam_core::feeds::FeedsProvenance::default();
    let input = feeds_input(machine, material, requested_ap);
    let preview = FeedsPreview::build(&input);
    let rec = preview
        .applicable()
        .expect("a 2D adaptive rough with a flat end mill is a runnable pairing");
    let recommended = rec.result().clone();

    let warnings = rs_cam_core::feeds::suggest::apply(
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
        warnings,
        recommended,
    }
}

// ── Non-vacuity ──────────────────────────────────────────────────────

/// The mechanism, not an absence.
///
/// Three facts have to hold before the headline arm means anything: the
/// rigidity clamp crosses the tier boundary, pass 9 fires on that crossing,
/// and it RAISES the feed. If any one fails the headline arm passes while
/// proving nothing.
#[test]
fn pass_nine_raises_the_feed_across_the_tier_boundary() {
    let machine = machine_at(1.6);
    let material = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    let shipped = run_funnel(&machine, &material, ENTRY_DPP_MM);

    assert!(
        (shipped.recommended.axial_depth_mm - ENTRY_DPP_MM).abs() < 1.0e-9,
        "fixture is vacuous: the calculator did not size its feed at the \
         requested depth (got {:.4} mm, wanted {ENTRY_DPP_MM:.4} mm), so the \
         entry point is not above the tier boundary",
        shipped.recommended.axial_depth_mm,
    );
    assert!(
        (shipped.ap_mm - CLAMPED_DPP_MM).abs() < 1.0e-9,
        "fixture is vacuous: the shipped depth is {:.4} mm, not the \
         {CLAMPED_DPP_MM:.4} mm rigidity cap. The crossing this file exists to \
         test is {ENTRY_DPP_MM:.4} mm (ap/D {:.5}, tier 0.50) down to \
         {CLAMPED_DPP_MM:.4} mm (ap/D {:.5}, tier 0.75).",
        shipped.ap_mm,
        ENTRY_DPP_MM / DIAMETER_MM,
        CLAMPED_DPP_MM / DIAMETER_MM,
    );

    let (requested, rescaled) = shipped.rescale().unwrap_or_else(|| {
        panic!(
            "fixture is vacuous: pass 9 never fired, so no feed was re-derived \
             at the final geometry. warnings: {:?}",
            shipped.warnings
        )
    });
    // Pass 9 re-derives from the calculator's UNROUNDED feed (its
    // `CalculatorOperatingPoint`), while `requested` is the value
    // `apply_feeds_subset` floored onto the operation. The tier ratio is
    // therefore exact against the calculator's number and a hair above it
    // against the floored one — 653.403 vs 653.000 here.
    let lift_on_calculator = rescaled / shipped.recommended.feed_rate_mm_min;
    let lift_on_operation = rescaled / requested;
    eprintln!(
        "  G-T15 non-vacuity | depth {ENTRY_DPP_MM:.3} -> {CLAMPED_DPP_MM:.3} mm \
         (tier 0.50 -> 0.75); pass 9 feed {requested:.3} -> {rescaled:.3} mm/min \
         (x{lift_on_operation:.4} on the floored feed, \
         x{lift_on_calculator:.4} on the calculator's {:.3})",
        shipped.recommended.feed_rate_mm_min,
    );
    assert!(
        rescaled > requested,
        "fixture is vacuous: pass 9 fired but did not RAISE the feed \
         ({requested:.3} -> {rescaled:.3} mm/min). T-15 is about a raise; a \
         drop cannot breach a ceiling the calculator already satisfied."
    );
    // 0.75 / 0.50 = 1.5 exactly. Pass 9 holds the implied chipload fixed and
    // re-multiplies by the depth tier alone, so the lift IS the tier ratio
    // unless the cutting-feed ceiling truncates it.
    assert!(
        (lift_on_calculator - 1.5).abs() < 1.0e-9,
        "the lift is {lift_on_calculator:.9} of the calculator's feed, not the \
         0.75 / 0.50 = 1.5 tier ratio. Either a cap truncated the \
         re-derivation or the tier table moved; re-derive this arm, do not \
         widen it."
    );
}

// ── The claim ────────────────────────────────────────────────────────

/// The headline. Whatever pass 9 does to the feed, the operating point that
/// reaches the machine must still fit inside the spindle envelope.
///
/// Step 6 clamps this recipe exactly ONTO the ceiling (utilisation 1.000 at
/// the calculator's point), so every bit of the breach below is the rescale.
#[test]
fn the_shipped_feed_fits_the_spindle_after_a_tier_crossing_rescale() {
    let machine = machine_at(1.6);
    let material = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    let shipped = run_funnel(&machine, &material, ENTRY_DPP_MM);

    let at_calculator = shipped.calculator_utilisation(&machine, &material);
    assert!(
        (at_calculator - 1.0).abs() < 1.0e-6,
        "fixture drifted: Step 6 no longer lands this recipe ON the ceiling \
         (utilisation {at_calculator:.6} at the calculator's point). The arm \
         below attributes the whole breach to the rescale and needs that \
         starting point."
    );
    assert!(
        shipped.recommended.power_limited,
        "fixture is vacuous: the calculator never engaged the Step 6 power \
         clamp. warnings: {:?}",
        shipped.recommended.warnings,
    );

    let shear_share = shipped.calculator_shear_share(&material);
    let depth_ratio = shipped.ap_mm / shipped.recommended.axial_depth_mm;
    // P = shear·feed + edge. The feed rises by the tier ratio, both terms
    // scale with the depth, and the edge term carries no feed:
    //   P_ship / P_calc = depth_ratio · (1 + (tier_ratio − 1) · shear_share)
    let predicted = depth_ratio * (1.0 + 0.5 * shear_share);
    let utilisation = shipped.shipped_utilisation(&machine, &material);

    eprintln!(
        "  G-T15 headline | calc {:.3} mm/min @ ap {:.4} -> shipped {:.3} mm/min \
         @ ap {:.4}; shear share {shear_share:.4}, depth ratio {depth_ratio:.6}; \
         predicted breach {predicted:.4}x, shipped load {:.2}% of the ceiling",
        shipped.recommended.feed_rate_mm_min,
        shipped.recommended.axial_depth_mm,
        shipped.feed_mm_min,
        shipped.ap_mm,
        100.0 * utilisation,
    );

    assert!(
        utilisation <= 1.0 + 1.0e-9,
        "T-15 reproduces: the SHIPPED operating point draws {:.2}% of the \
         spindle's gate ceiling.\n    \
         The rigidity clamp took the depth {ENTRY_DPP_MM:.3} -> \
         {CLAMPED_DPP_MM:.3} mm, a loss of {:.3}%, which moved the depth tier \
         0.50 -> 0.75 and made pass 9 raise the feed 1.5x.\n    \
         Predicted breach = depth ratio {depth_ratio:.6} x (1 + 0.5 x shear \
         share {shear_share:.4}) = {predicted:.4}x, against a recipe Step 6 had \
         clamped to exactly 1.000 of the ceiling.\n    \
         calculator {:.3} mm/min at ap {:.4}; the operation ships {:.3} mm/min \
         at ap {:.4}.",
        100.0 * utilisation,
        100.0 * (1.0 - depth_ratio),
        shipped.recommended.feed_rate_mm_min,
        shipped.recommended.axial_depth_mm,
        shipped.feed_mm_min,
        shipped.ap_mm,
    );

    let (rescaled, ship_feed, fits) = shipped.power_recheck().unwrap_or_else(|| {
        panic!(
            "the shipped point fits, but pass 10 filed no warning — the \
             operator is not told the feed moved. warnings: {:?}",
            shipped.warnings
        )
    });
    assert!(
        fits,
        "a feed does fit this cut: the edge term is under the ceiling"
    );
    assert!(
        ship_feed < rescaled && (ship_feed - shipped.feed_mm_min).abs() < 1.0e-9,
        "the warning reports {rescaled:.3} -> {ship_feed:.3} mm/min but the \
         operation carries {:.3} mm/min",
        shipped.feed_mm_min,
    );
}

/// Pass 10's gate. An operation whose geometry no pass moves must keep its
/// feed EXACTLY, so the re-check cannot un-round a value it has no reason to
/// touch. Pass 9 holds the same contract and
/// `suggest_feed_matches_final_geometry` pins it from the other side.
///
/// The machine here is the same 1.6 kW spindle, so the power branch is live:
/// the arm proves the gate, not an inactive ceiling.
#[test]
fn an_untouched_operation_keeps_its_feed_byte_identical() {
    let machine = machine_at(1.6);
    let material = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    let shipped = run_funnel(&machine, &material, UNTOUCHED_DPP_MM);

    assert!(
        (shipped.ap_mm - UNTOUCHED_DPP_MM).abs() < 1.0e-12
            && (shipped.ae_mm - STEPOVER_MM).abs() < 1.0e-12,
        "fixture is vacuous: a pass moved the geometry (ap {:.6}, ae {:.6}), so \
         pass 9 is entitled to re-derive and this arm tests nothing",
        shipped.ap_mm,
        shipped.ae_mm,
    );
    assert!(
        shipped.rescale().is_none() && shipped.power_recheck().is_none(),
        "pass 9 or pass 10 acted on an operation no pass moved. warnings: {:?}",
        shipped.warnings,
    );
    // `apply_feeds_subset` writes `round_suggestion_value_down(feed, 1.0)`.
    let expected = shipped.recommended.feed_rate_mm_min.floor();
    assert!(
        shipped.feed_mm_min == expected,
        "the feed is not byte-identical: the funnel wrote {:.12} where the \
         calculator's value floors to {expected:.12}",
        shipped.feed_mm_min,
    );
}

/// The refusal. When the feed-free edge term alone is at or over the ceiling,
/// no feed fits: thinning the chip leaves the ploughing power exactly where it
/// was. Pass 10 must not raise the feed on such a cut — it puts it back to the
/// value pass 9 started from, which Step 6 validated at the calculator's
/// geometry, and says that no feed fits.
///
/// The same 0.6 kW ceiling makes Step 6 refuse for the same reason, so the
/// calculator ships its unclamped feed with a `PowerLimited` warning and no
/// derate. That is the shape this arm needs.
#[test]
fn no_feed_fits_when_the_edge_term_alone_is_over_budget() {
    let machine = machine_at(0.8);
    let material = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    let shipped = run_funnel(&machine, &material, ENTRY_DPP_MM);

    let kc = material.kc_n_per_mm2().unwrap();
    let (_, edge) = power_terms_kw(
        kc,
        shipped.ap_mm,
        shipped.ae_mm,
        0.0,
        shipped.recommended.rpm,
    );
    let ceiling = gate_ceiling_kw(&machine, shipped.recommended.rpm);
    assert!(
        edge >= ceiling,
        "fixture is vacuous: the edge term is {edge:.4} kW against a \
         {ceiling:.4} kW ceiling, so a feed DOES fit and this arm does not \
         exercise the refusal"
    );

    let (requested, rescaled) = shipped
        .rescale()
        .expect("pass 9 fires on this fixture — same crossing as the headline");
    let (_, ship_feed, fits) = shipped.power_recheck().unwrap_or_else(|| {
        panic!(
            "pass 10 did not re-check a cut no feed can make. warnings: {:?}",
            shipped.warnings
        )
    });

    eprintln!(
        "  G-T15 refusal | edge {edge:.4} kW vs ceiling {ceiling:.4} kW; pass 9 \
         {requested:.3} -> {rescaled:.3} mm/min, shipped {ship_feed:.3} mm/min"
    );
    assert!(
        !fits,
        "pass 10 reports a feed fits, but the edge term {edge:.4} kW is already \
         at or over the {ceiling:.4} kW ceiling"
    );
    assert!(
        (ship_feed - requested).abs() < 1.0e-9 && (shipped.feed_mm_min - requested).abs() < 1.0e-9,
        "no feed fits, so the operation must ship the pre-rescale feed \
         {requested:.3} mm/min; it ships {:.3} mm/min (warning says \
         {ship_feed:.3})",
        shipped.feed_mm_min,
    );
    assert!(
        shipped.feed_mm_min < rescaled,
        "the shipped feed {:.3} mm/min is not below the {rescaled:.3} mm/min \
         pass 9 wrote — the raise was not withdrawn",
        shipped.feed_mm_min,
    );
}

/// The case the register's cheaper proposal would have missed.
///
/// That proposal was to refuse the raise when `FeedsDerates::power_limit`
/// is below 1.0 — i.e. only on a recipe Step 6 had already clamped. This
/// fixture is one Step 6 never touched: 85 % of the ceiling at the
/// calculator's point, below the 89.4 % p90 the register re-measured across
/// 162 recipes after R1. Pass 9's 1.5× lift takes it over the ceiling all the
/// same, and only a re-evaluation at the final state catches it.
#[test]
fn an_unclamped_recipe_is_not_lifted_over_the_ceiling() {
    let machine = machine_at(1.25);
    let material = Material::SolidWood {
        species: WoodSpecies::RadiataPine,
    };
    let shipped = run_funnel(&machine, &material, ENTRY_DPP_MM);

    assert!(
        !shipped.recommended.power_limited
            && !shipped
                .recommended
                .warnings
                .iter()
                .any(|w| matches!(w, FeedsWarning::PowerLimited { .. })),
        "fixture drifted: Step 6 clamped this recipe, so it is no longer the \
         un-clamped case. warnings: {:?}",
        shipped.recommended.warnings,
    );
    assert!(
        (shipped.recommended.derates.power_limit - 1.0).abs() < 1.0e-12,
        "fixture drifted: `power_limit` is {:.6}, not 1.0 — the register's \
         cheaper fix would have caught this case after all",
        shipped.recommended.derates.power_limit,
    );

    let at_calculator = shipped.calculator_utilisation(&machine, &material);
    assert!(
        (0.80..0.90).contains(&at_calculator),
        "fixture drifted: the calculator's point sits at {:.2}% of the ceiling. \
         This arm needs an ORDINARY un-clamped load — the band is 80-90%, \
         which straddles the 89.4% p90 the register re-measured.",
        100.0 * at_calculator,
    );
    let (requested, rescaled) = shipped
        .rescale()
        .expect("pass 9 fires on this fixture — same crossing as the headline");
    assert!(
        rescaled > requested,
        "fixture is vacuous: pass 9 did not raise the feed"
    );

    let utilisation = shipped.shipped_utilisation(&machine, &material);
    eprintln!(
        "  G-T15 un-clamped | Step 6 did not clamp ({:.2}% of the ceiling); pass 9 \
         {requested:.3} -> {rescaled:.3} mm/min; shipped load {:.2}%",
        100.0 * at_calculator,
        100.0 * utilisation,
    );
    assert!(
        utilisation <= 1.0 + 1.0e-9,
        "T-15 reproduces on a recipe Step 6 never clamped: the calculator's \
         point draws {:.2}% of the ceiling and the SHIPPED point draws {:.2}%. \
         Refusing the raise only when `power_limit < 1.0` would not have seen \
         this one.",
        100.0 * at_calculator,
        100.0 * utilisation,
    );
}
