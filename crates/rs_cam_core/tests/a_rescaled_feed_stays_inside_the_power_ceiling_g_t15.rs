//! T-15 — the feed pass 9 re-derives must still fit the spindle.
//!
//! `feeds::calculate` Step 6 checks the spindle power ceiling at the geometry
//! it was handed. `enforce_invariants` then runs, and pass 9
//! (`rescale_feed_to_final_geometry`) re-multiplies the feed by
//! `depth_tier_multiplier` at the FINAL depth. That multiplier RISES as the
//! depth falls, so a clamp that lowers the depth makes pass 9 RAISE the feed.
//! Until T-15 nothing re-checked Step 6 afterwards.
//!
//! ## Re-derived for one continuous scale (feeds matrix R3, 2026-09-23)
//!
//! The multiplier used to be a step: 1.00 / 0.75 / 0.50 / 0.45 at `ap/D` of
//! 1 / 2 / 3. A clamp of 0.21 % across the 2 x D boundary then lifted the
//! feed 1.5x, and the shipped load outran the ceiling. R3 unified the feed
//! and the chipload band on the published, piecewise-linear
//! `feeds::geometry::doc_derating_scale` (1.00 at 1 x D, 0.75 at 2 x D,
//! 0.50 at 3 x D, linear between, held at 0.50 above). The lift is now the
//! ratio of the scale at the two depths. Power is affine in the feed, so
//! for a clamp from `a x D` to `b x D` (`a > b`, both in 1..3) the shipped
//! load against the calculator's point is at most
//!
//! ```text
//! (b / a) · s(b) / s(a)  =  b (5 − b) / (a (5 − a))
//! ```
//!
//! and `x (5 − x)` rises up to `x = 2.5`, so a clamp that starts at or
//! below 2.5 x D can never lift the shipped load above a ceiling the
//! calculator's point satisfied. The headline arm pins that fact on the
//! fixture; the refusal arm still exercises pass 10, because a ceiling the
//! feed-free edge term already exceeds stays exceeded at any feed.
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
//! - an operator-set depth of 30.000 mm, 2.5 × D, where the scale is 0.625.
//!
//! The rigidity clamp then takes the depth 30.000 → 24.000 mm, a loss of
//! 20 %, and the scale goes 0.625 → 0.75, a feed rise of exactly 1.2×.
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
//! with `tier_ratio = 0.75 / 0.625 = 1.2` and `shear_share = shear / (shear +
//! edge)` at the calculator's point. The edge term carries no feed, so the
//! real figure is lower than the pure-shear bound and the arms below quote
//! the measured split.
//!
//! ## The arms
//!
//! - [`the_shipped_feed_fits_the_spindle_after_a_tier_crossing_rescale`] — on
//!   a recipe Step 6 clamped ONTO the ceiling, the rescale stays under it
//!   and pass 10 has nothing to do.
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
use rs_cam_core::feeds::geometry::doc_derating_scale;
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

/// 2.5 × D, where the published scale is 0.625, so the calculator sizes the
/// feed with `depth_tier_multiplier = 0.625`.
const ENTRY_DPP_MM: f64 = 30.0;

/// The rigidity cap: `adaptive_doc_factor × D = 2.0 × 12 = 24.0`, exactly
/// 2.0 × D, where the published scale is 0.75.
const CLAMPED_DPP_MM: f64 = 24.0;

/// The lift pass 9 applies: the scale at the clamped depth over the scale at
/// the entry depth, 0.75 / 0.625 = 1.2. Derived from the one function so the
/// arms state the rule, not a number.
fn tier_ratio() -> f64 {
    doc_derating_scale(CLAMPED_DPP_MM / DIAMETER_MM)
        / doc_derating_scale(ENTRY_DPP_MM / DIAMETER_MM)
}

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
/// with `(Ks, F_edge)` and the grain factor `A` from the material's force
/// line (`Material::force_line`, ruling B6; `A = 1.0` on every shipped
/// line), and `cos ψ = 1 − ae/r`. The test reads the two coefficients from
/// the line and restates the expression. Returns the two terms apart,
/// because the whole mechanism turns on which of them carries the feed.
fn power_terms_kw(
    material: &Material,
    ap_mm: f64,
    ae_mm: f64,
    feed_mm_min: f64,
    rpm: f64,
) -> (f64, f64) {
    let line = material
        .force_line()
        .expect("fixture species has a force line");
    let (ks, f_edge, a) = (
        line.ks_n_per_mm2(),
        line.f_edge_n_per_mm(),
        line.grain_factor(),
    );
    let cross_section = ToolGeometryHint::Flat.mrr_cross_section_mm2(ap_mm, ae_mm);
    let psi = (1.0 - ae_mm / (DIAMETER_MM / 2.0)).clamp(-1.0, 1.0).acos();
    let duty = f64::from(FLUTES) * psi / std::f64::consts::TAU;
    let vc = std::f64::consts::PI * DIAMETER_MM * rpm;
    let shear = a * ks * cross_section * feed_mm_min / 60_000_000.0;
    let edge = a * f_edge * ap_mm * vc * duty / 60_000_000.0;
    (shear, edge)
}

/// The gate's ceiling — the rated curve `power_at_rpm`, the axis every
/// published power number is quoted against. Ruling R4 Q2 (2026-09-24)
/// removed the `safety_factor` fraction.
fn gate_ceiling_kw(machine: &MachineProfile, rpm: f64) -> f64 {
    machine.power_at_rpm(rpm)
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
    // Ruling R4 Q7 (2026-09-24): above 4 x D stickout the long-tool share
    // (0.88) lowers the dial's load target, so even at aggressiveness 1.0
    // pass 6b would scale the depth (measured 24 -> 20 mm here, 2.4 -> 1.8
    // in g_s2). 48 mm = 4.0 x D takes no share, and the dial stays out.
    t.stickout = 48.0;
    t
}

/// Synthetic spindle. Not a shipped preset and not a recommendation — it
/// exists to put the Step 6 / pass 9 interaction under a ceiling that binds.
/// `adaptive_doc_factor` is set to 2.0, the shipped `RigidityProfile::default()`
/// value; the generic router ships 1.5, which caps at 1.5 × D and lands inside
/// the same tier the entry depth is above.
///
/// Ruling R4 Q2 (2026-09-24) RE-TUNE. The ceiling was `power_at_rpm x 0.75`
/// and is now `power_at_rpm`. An arm whose recipe Step 6 clamped onto the
/// ceiling reproduces its old operating point exactly on a spindle of 0.75 x
/// its old rating: the clamp solves the same feed against the same kW. So
/// 2.0 -> 1.5, 1.6 -> 1.2 (the crossing arm) and 0.8 -> 0.6 kW (the refusal
/// arm: the measured edge term 0.6975 kW must exceed the ceiling). The
/// un-clamped arm is re-tuned on its own measurement, see there.
///
/// Ruling B6 RE-TUNE (2026-09-25). The force line replaced the scaled
/// woodresearch.sk anchor and the 2.0 grain factor. On `GenericHardwood`
/// the shear term is x0.5197 (51.92 / 99.9 N/mm²) and the edge term is
/// x0.3846 (4.077 / 10.6 N/mm) of the old one. Each arm's ceiling is the
/// old ceiling re-evaluated at the old operating point with the new line,
/// so each recipe keeps its old feed and its old clamp state.
fn machine_at(power_kw: f64) -> MachineProfile {
    let mut m = MachineProfile::generic_wood_router();
    m.name = format!("SYNTHETIC {power_kw:.2} kW (test only)");
    m.power = rs_cam_core::machine::PowerModel::ConstantPower { power_kw };
    m.rigidity.adaptive_doc_factor = 2.0;
    // Ruling R4 (2026-09-24): this fixture tests the Step 6 / pass 9 / pass 10
    // power interaction, not the aggressiveness dial. The dial at 1.0 keeps
    // pass 6b out of the geometry (it would scale the depth and the stepover
    // before pass 9 runs).
    m.aggressiveness = 1.0;
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
    /// Both sides sit on one axis: `feed_mm_min` is the feed that ships and
    /// the ceiling is the rated curve. Since ruling R4 neither carries a
    /// factor.
    fn shipped_utilisation(&self, machine: &MachineProfile, material: &Material) -> f64 {
        let (shear, edge) = power_terms_kw(
            material,
            self.ap_mm,
            self.ae_mm,
            self.feed_mm_min,
            self.recommended.rpm,
        );
        (shear + edge) / gate_ceiling_kw(machine, self.recommended.rpm)
    }

    /// The same, at the point the CALCULATOR derived its feed at.
    fn calculator_utilisation(&self, machine: &MachineProfile, material: &Material) -> f64 {
        let (shear, edge) = power_terms_kw(
            material,
            self.recommended.axial_depth_mm,
            self.recommended.radial_width_mm,
            self.recommended.feed_rate_mm_min,
            self.recommended.rpm,
        );
        (shear + edge) / gate_ceiling_kw(machine, self.recommended.rpm)
    }

    /// Fraction of the calculator-point load that the feed carries.
    fn calculator_shear_share(&self, material: &Material) -> f64 {
        let (shear, edge) = power_terms_kw(
            material,
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
    // R4 re-tune: 1.6 -> 1.2 kW (the old 1.6 x 0.75 ceiling).
    // B6 re-tune: 1.2 -> 0.5 kW. At the old point (30 mm, the edge term
    // about 1.17 kW and the shear term about 0.27 kW at 544 mm/min) the new
    // line draws 0.3846 x 1.17 + 0.5197 x 0.27 = 0.59 kW. A ceiling under
    // that keeps Step 6 on the feed, so the calculator's feed stays far
    // below the 4 000 mm/min cap and the 1.2x lift is not truncated.
    let machine = machine_at(0.5);
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
         test is {ENTRY_DPP_MM:.4} mm (ap/D {:.5}, scale 0.625) down to \
         {CLAMPED_DPP_MM:.4} mm (ap/D {:.5}, scale 0.75).",
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
         (scale 0.625 -> 0.75); pass 9 feed {requested:.3} -> {rescaled:.3} mm/min \
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
    // 0.75 / 0.625 = 1.2 exactly. Pass 9 holds the implied chipload fixed and
    // re-multiplies by the depth scale alone, so the lift IS the scale ratio
    // unless the cutting-feed ceiling truncates it.
    let ratio = tier_ratio();
    assert!(
        (lift_on_calculator - ratio).abs() < 1.0e-9,
        "the lift is {lift_on_calculator:.9} of the calculator's feed, not the \
         0.75 / 0.625 = {ratio:.4} scale ratio. Either a cap truncated the \
         re-derivation or the scale moved; re-derive this arm, do not widen it."
    );
}

// ── The claim ────────────────────────────────────────────────────────

/// The headline. Whatever pass 9 does to the feed, the operating point that
/// reaches the machine must still fit inside the spindle envelope.
///
/// Step 6 clamps this recipe exactly ONTO the ceiling (utilisation 1.000 at
/// the calculator's point), so every bit of the movement below is the
/// rescale. Under the one continuous scale the rescale cannot breach: the
/// depth falls to 0.8 and the feed rises 1.2, so the shipped load is
/// `0.8 · (1 + 0.2 · shear_share) < 1`. Pass 10 finds nothing over the
/// ceiling and files no warning; that silence is the pin.
#[test]
fn the_shipped_feed_fits_the_spindle_after_a_tier_crossing_rescale() {
    // R4 re-tune: 2.0 -> 1.5 kW spindle (the old 2.0 x 0.75 ceiling).
    // B6 re-tune: 1.5 -> 0.65 kW. At the 30 mm entry the feed-free edge
    // term is now about 0.45 kW (0.3846 x the old 1.17 kW). The old 1.5 kW
    // point (shear about 0.33 kW) re-evaluates to 0.45 + 0.5197 x 0.33 =
    // 0.62 kW. 0.65 kW leaves a shear budget above the rubbing floor, so
    // Step 6 still lands the recipe on the ceiling.
    let machine = machine_at(0.65);
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
    // P = shear·feed + edge. The feed rises by the scale ratio, both terms
    // scale with the depth, and the edge term carries no feed:
    //   P_ship / P_calc = depth_ratio · (1 + (tier_ratio − 1) · shear_share)
    let predicted = depth_ratio * (1.0 + (tier_ratio() - 1.0) * shear_share);
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
         {CLAMPED_DPP_MM:.3} mm, a loss of {:.3}%, which moved the depth scale \
         0.625 -> 0.75 and made pass 9 raise the feed {:.3}x.\n    \
         Predicted load = depth ratio {depth_ratio:.6} x (1 + {:.3} x shear \
         share {shear_share:.4}) = {predicted:.4}x, against a recipe Step 6 had \
         clamped to exactly 1.000 of the ceiling.\n    \
         calculator {:.3} mm/min at ap {:.4}; the operation ships {:.3} mm/min \
         at ap {:.4}.",
        100.0 * utilisation,
        100.0 * (1.0 - depth_ratio),
        tier_ratio(),
        tier_ratio() - 1.0,
        shipped.recommended.feed_rate_mm_min,
        shipped.recommended.axial_depth_mm,
        shipped.feed_mm_min,
        shipped.ap_mm,
    );
    // The continuous scale keeps the rescaled point under the ceiling by
    // construction (module doc), and the measured load agrees with the
    // affine prediction. Pass 10 re-checks and finds nothing over the
    // ceiling, so it files no warning and moves no number.
    assert!(
        predicted < 1.0 - 1.0e-6,
        "the affine prediction {predicted:.4}x is not below the ceiling; the scale or the \
         fixture moved — re-derive the module doc's inequality before widening this"
    );
    assert!(
        (utilisation - predicted).abs() < 5.0e-3,
        "the shipped load {utilisation:.4} does not match the affine prediction {predicted:.4}"
    );
    assert!(
        shipped.power_recheck().is_none(),
        "pass 10 filed a power re-check on a rescale that stays under the ceiling: {:?}",
        shipped.warnings
    );
    assert!(
        (shipped.feed_mm_min - shipped.rescale().map(|(_, r)| r).unwrap_or(f64::NAN)).abs()
            < 1.0 + 1.0e-9,
        "the operation must carry pass 9's rescaled feed (to the 1 mm/min rounding); it \
         carries {:.3} mm/min",
        shipped.feed_mm_min,
    );
}

/// Pass 10's gate. An operation whose geometry no pass moves must keep its
/// feed EXACTLY, so the re-check cannot un-round a value it has no reason to
/// touch. Pass 9 holds the same contract and
/// `suggest_feed_matches_final_geometry` pins it from the other side.
///
/// The machine here is a 0.7 kW spindle, so the power branch is live:
/// the arm proves the gate, not an inactive ceiling. B6 re-tune: 1.6 ->
/// 0.7 kW, about the x0.45 that the new line puts on hardwood power.
#[test]
fn an_untouched_operation_keeps_its_feed_byte_identical() {
    let machine = machine_at(0.7);
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
/// The same 0.23 kW ceiling makes Step 6 refuse for the same reason, so the
/// calculator ships its unclamped feed with a `PowerLimited` warning and no
/// derate. That is the shape this arm needs.
#[test]
fn no_feed_fits_when_the_edge_term_alone_is_over_budget() {
    // R4 re-tune: 0.8 -> 0.6 kW (the old 0.8 x 0.75 ceiling), under the
    // measured 0.6975 kW edge term, so no feed fits.
    // B6 re-tune: 0.6 -> 0.23 kW. The edge term is x0.3846 of the old one,
    // 0.6975 -> 0.268 kW, and 0.23 kW keeps the old 0.86 ratio under it.
    let mut machine = machine_at(0.23);
    // Step 6 ships this recipe unclamped (no feed fits), and at the 30 mm
    // entry the unclamped feed reaches the default 4 000 cutting-feed ceiling
    // (4 000 x 0.75 before ruling R4), which would hide pass 9's lift behind the cap. A higher
    // ceiling keeps the lift visible so pass 10 has a raise to withdraw.
    machine.max_feed_mm_min = 8000.0;
    let material = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    let shipped = run_funnel(&machine, &material, ENTRY_DPP_MM);

    let (_, edge) = power_terms_kw(
        &material,
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
/// fixture is one Step 6 never touched: 80 to 90 % of the ceiling at the
/// calculator's point, the band that straddles the 89.4 % p90 the register
/// re-measured across 162 recipes after R1. Under the continuous scale the
/// 1.2× lift rides on a 0.8× depth, so the shipped load stays under the
/// ceiling; the arm pins that the un-clamped case is not lifted over it.
#[test]
fn an_unclamped_recipe_is_not_lifted_over_the_ceiling() {
    // R4 re-tune: 1.75 -> 1.53 kW. Measured after WP3 at 1.75 kW the
    // calculator's point drew 74.27 % of the ceiling (the feed lost its 0.75
    // factor). The recipe is un-clamped, so its power does not move with the
    // spindle: 1.75 x 0.7427 / 1.53 = 84.95 %, inside the 80-90 % band.
    // Extrapolation P2 re-tune (a32ea149, one Janka table): the row's
    // softwood default moved 500 -> 600 lbf, so the band rose x1.061 and the
    // point drew 90.14 % at 1.53 kW. 1.53 x 0.9014 / 1.62 = 85.13 %.
    // B6 re-tune: 1.62 -> 1.2 kW. Radiata pine now reads the Curti line at
    // 504.1 kg/m³ (FPL Table 5-5a Gb 0.42 by Eq. 4-11): Ks 38.73, F_edge
    // 3.041, grain factor 1.0, against the old 2 x (23.05, 2.446). The
    // shear term is x0.840 and the edge term x0.622 of the old one. At the
    // old 1.379 kW point (8 000 rev/min: edge 0.54 kW, shear 0.84 kW) the
    // new line draws about 1.04 kW, 86.7 % of 1.2 kW. A higher spindle
    // speed moves more of the load to the edge term; at 12 000 rev/min the
    // point is about 81.8 %. Both are inside the 80-90 % band.
    let machine = machine_at(1.2);
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
