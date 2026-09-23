//! S2 — the power figure a surface displays is evaluated at the depth that
//! will be cut.
//!
//! `feeds::calculate` publishes `FeedsResult::power_kw` at the geometry it was
//! handed. `suggest::enforce_invariants` then runs, and `clamp_dpp_to_rigidity`
//! lowers the depth per pass. The published figure therefore describes a depth
//! that will not be cut — the first of the three defect classes this repository
//! keeps rediscovering: a value computed at one state, consumed at another.
//!
//! `feeds::power_at_operating_point` is the door that answers at the FINAL
//! state. Suggest pass 10 already used that evaluation privately (T-15); S2
//! made it public so a display and the pass read one number.
//!
//! ## The fixture
//!
//! A Ø12 two-flute flat end mill, a roughing pocket in generic hardwood, on
//! the shipped `Generic Wood Router` preset. Its `doc_roughing_factor` is
//! 0.20, so the rigidity cap on this tool is 2.400 mm. The operator asks for
//! 36.000 mm — 3 × D — and the clamp takes it to the cap, a 15× reduction.
//!
//! A Pocket is deliberately NOT an Adaptive-family operation, so the clamp
//! reads `doc_roughing_factor` rather than `adaptive_doc_factor`. The stepover
//! is 4.000 mm (0.333 × D), under the 0.85 × D slotting threshold, so
//! `SlottingDetected` does not cap the depth before Step 6. Both `ap` and `ae`
//! are pinned in the `FeedsInput`, so the Step 6 power ladder's geometry rungs
//! stand down and the fixture measures the clamp, not the ladder.
//!
//! ## The arithmetic the headline arm checks
//!
//! Power is affine in the feed: `P = shear·feed + edge`. On a flat end mill
//! the effective diameter is the nominal diameter at every depth, so between
//! the calculator's point and the shipped point:
//!
//! - the shear slope scales with the cross-section `ap · ae`, and `ae` does
//!   not move here, so it scales with `ap` alone;
//! - the edge term scales with `ap` and with the cutting velocity `π·D·n`, so
//!   it scales with `ap` and the RPM;
//! - neither term carries the depth tier, which is what moves the feed.
//!
//! So with `share_c` the fraction of the calculator-point load the feed
//! carries:
//!
//! ```text
//! P_ship / P_calc = (ap_f / ap_c) · ( share_c · (f_f / f_c)
//!                                     + (1 − share_c) · (n_f / n_c) )
//! ```
//!
//! The arm recovers the two terms at the shipped point from the door's own
//! public surface — `required_kw_at_feed` at a zero feed is the edge floor,
//! and the slope is the shear term — scales them back to the calculator's
//! depth and RPM, then checks that reconstruction against the published
//! `power_kw` before it uses it. Nothing in this file restates the model's
//! coefficients, and nothing reads the terms apart.
//!
//! ## The arms
//!
//! - [`the_published_power_describes_a_depth_that_will_not_be_cut`] — the
//!   claim, with the arithmetic.
//! - [`the_rigidity_clamp_moves_the_depth_on_this_fixture`] — non-vacuity for
//!   the claim: the clamp fires and the door answers.
//! - [`the_door_and_pass_ten_are_one_evaluation`] — on the T-15 sentry's own
//!   fixture, the door's `available_kw` and its `required_kw_at_feed`
//!   reproduce what `PowerRecheckedAfterRescale` reported, bit for bit.
//! - [`every_refusal_names_itself`] — a drill, a material with no `Kc` and a
//!   zero feed each refuse with a distinct, non-empty clause.
//! - [`the_anchor_cut_is_modelled_not_refused`] — non-vacuity for the
//!   refusals: the anchor returns `Ok` with `available_kw > required_kw > 0`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    // SAFETY: a sentry that measures a ratio should print the ratio it
    // measured, the same allowance the sibling power instruments take.
    clippy::print_stderr
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{AdaptiveConfig, DrillConfig, PocketConfig};
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::{
    ApplyContext, ApplyScope, FeedsPreview, SuggestContext, SuggestWarning,
};
use rs_cam_core::feeds::{
    FeedsInput, FeedsResult, OperationFamily, PassRole, PowerUnmodeled, SetupContext,
    SpindleStrategy, ToolGeometryHint, embedded_vendor_lut, power_at_operating_point,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, PlasticFamily, WoodSpecies};

const DIAMETER_MM: f64 = 12.0;
const FLUTES: u32 = 2;

/// 3 × D. What the operator asks for, and what the calculator sizes its feed
/// and its published power at.
const REQUESTED_DPP_MM: f64 = 36.0;

/// `doc_roughing_factor × D` on the shipped `Generic Wood Router` preset:
/// 0.20 × 12 = 2.400 mm. What the rigidity clamp leaves, and what the machine
/// actually cuts.
const RIGIDITY_CAP_MM: f64 = 2.4;

/// 0.333 × D. Under the 0.85 × D slotting threshold and under the preset's
/// 5.0 mm `woc_roughing_max_mm`, so nothing moves it and the shear term's
/// cross-section scales with the depth alone.
const STEPOVER_MM: f64 = 4.0;

/// Relative tolerance for an identity that holds exactly in real arithmetic.
///
/// Both sides are the same affine model evaluated through different
/// expression orderings — a few dozen double-precision operations, so the
/// expected disagreement is a small multiple of `f64::EPSILON` (2.2e-16),
/// about 1e-13 at worst. 1e-9 leaves four orders of margin over that and is
/// still eight orders BELOW the smallest difference a model change could
/// make here: one depth-tier step is a factor of 1.5.
const MODEL_IDENTITY_TOLERANCE: f64 = 1.0e-9;

/// A Ø12 two-flute flat end mill, long enough that neither the flute guard
/// (0.8 × 50 = 40 mm) nor the cutting-length clamp (50 mm) reaches the
/// requested 36 mm depth. All fixture choices, not model constants.
fn tool() -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    t.diameter = DIAMETER_MM;
    t.flute_count = FLUTES;
    t.cutting_length = 50.0;
    // Ruling R4 Q7 (2026-09-24): above 4 x D stickout the long-tool share
    // (0.88) lowers the dial's load target, so even at aggressiveness 1.0
    // pass 6b would scale the depth (measured 24 -> 20 mm here, 2.4 -> 1.8
    // in g_s2). 48 mm = 4.0 x D takes no share, and the dial stays out.
    t.stickout = 48.0;
    t
}

fn hardwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    }
}

fn pocket_op(dpp: f64) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: STEPOVER_MM,
        // Two full passes, so `realised_step_down` snaps the depth to itself
        // and the entry value the invariants see is the requested one.
        depth: 2.0 * dpp,
        depth_per_pass: dpp,
        feed_rate: 1000.0,
        plunge_rate: 400.0,
        ..PocketConfig::default()
    })
}

fn feeds_input<'a>(machine: &'a MachineProfile, material: &'a Material, ap: f64) -> FeedsInput<'a> {
    FeedsInput {
        tool_diameter: DIAMETER_MM,
        flute_count: FLUTES,
        flute_length: 50.0,
        shank_diameter: None,
        tool_geometry: ToolGeometryHint::Flat,
        material,
        machine,
        operation: OperationFamily::Pocket,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        // Pinned, so the Step 6 ladder's geometry rungs stand down and the
        // fixture isolates the rigidity clamp.
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
    operation: OperationConfig,
    warnings: Vec<SuggestWarning>,
}

/// Run the WHOLE Suggest door — `calculate` → `apply_feeds_subset` →
/// `enforce_invariants` — and keep both the calculator's recommendation and
/// the operation that ships. The two are different operating points, and only
/// one of them reaches the machine.
fn run_suggest_door(machine: &MachineProfile, material: &Material, requested_ap: f64) -> Shipped {
    let tool = tool();
    let mut operation = pocket_op(requested_ap);
    let mut provenance = rs_cam_core::feeds::FeedsProvenance::default();
    let input = feeds_input(machine, material, requested_ap);
    let preview = FeedsPreview::build(&input);
    let rec = preview
        .applicable()
        .expect("a roughing pocket with a flat end mill is a runnable pairing");
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
        recommended,
        operation,
        warnings,
    }
}

/// The generic router with the aggressiveness dial at 1.0.
///
/// Ruling R4 (2026-09-24): at the default 0.85 Suggest pass 6b scales the
/// depth 2.4 -> 1.8 mm and the stepover by the same factor (measured,
/// s = 0.75). This file tests the rigidity clamp and the power door, and its
/// headline arithmetic holds the width, so the dial is kept out. FM7 pins
/// the dial.
fn router() -> MachineProfile {
    let mut machine = MachineProfile::generic_wood_router();
    machine.aggressiveness = 1.0;
    machine
}

// ── Non-vacuity for the claim ────────────────────────────────────────

/// Three facts have to hold before the headline arm means anything: the
/// calculator sizes its power at the requested depth, the rigidity clamp
/// lowers that depth, and the door answers at the lowered one. If any fails
/// the headline arm compares two copies of the same number.
#[test]
fn the_rigidity_clamp_moves_the_depth_on_this_fixture() {
    let machine = router();
    let material = hardwood();
    let shipped = run_suggest_door(&machine, &material, REQUESTED_DPP_MM);

    assert!(
        (shipped.recommended.axial_depth_mm - REQUESTED_DPP_MM).abs() < 1.0e-9,
        "fixture is vacuous: the calculator sized its power at {:.4} mm, not \
         the requested {REQUESTED_DPP_MM:.4} mm. Either the flute guard or the \
         Step 6 ladder moved the pinned depth.",
        shipped.recommended.axial_depth_mm,
    );
    assert!(
        shipped.recommended.power_kw > 0.0,
        "fixture is vacuous: the calculator published no power figure, so \
         there is nothing to compare against. warnings: {:?}",
        shipped.recommended.warnings,
    );

    let clamped = shipped
        .warnings
        .iter()
        .any(|w| matches!(w, SuggestWarning::RoughingDepthClampedToRigidity { .. }));
    assert!(
        clamped,
        "fixture is vacuous: the rigidity clamp never fired, so the shipped \
         depth is the requested one. warnings: {:?}",
        shipped.warnings,
    );

    let final_ap = shipped.operation.depth_per_pass().unwrap();
    assert!(
        (final_ap - RIGIDITY_CAP_MM).abs() < 1.0e-9,
        "fixture drifted: the shipped depth is {final_ap:.4} mm, not the \
         {RIGIDITY_CAP_MM:.4} mm rigidity cap (doc_roughing_factor 0.20 x D). \
         A clamp below the file's constants changes the ratio the headline arm \
         predicts; re-derive it, do not widen the tolerance."
    );

    let final_ae = shipped.operation.stepover().unwrap();
    assert!(
        (final_ae - shipped.recommended.radial_width_mm).abs() < 1.0e-9,
        "fixture drifted: the stepover moved, {:.6} -> {final_ae:.6} mm. The \
         headline arm's arithmetic scales the shear term with the depth alone \
         and needs the width held.",
        shipped.recommended.radial_width_mm,
    );
}

// ── The claim ────────────────────────────────────────────────────────

/// The headline. `FeedsResult::power_kw` answers at 36 mm; the machine cuts
/// 2.4 mm; `power_at_operating_point` answers at 2.4 mm, and the two differ by
/// exactly what the model says they should.
#[test]
fn the_published_power_describes_a_depth_that_will_not_be_cut() {
    let machine = router();
    let material = hardwood();
    let shipped = run_suggest_door(&machine, &material, REQUESTED_DPP_MM);

    let figure = power_at_operating_point(
        &shipped.operation,
        &tool(),
        &material,
        &machine,
        // No fallback: a pocket carries its own depth, stepover and speed, so
        // the door reads the operation and nothing else.
        None,
    )
    .expect("a hardwood pocket on a preset router is a modelled cut");

    let ap_c = shipped.recommended.axial_depth_mm;
    let feed_c = shipped.recommended.feed_rate_mm_min;
    let rpm_c = shipped.recommended.rpm;
    let published_kw = shipped.recommended.power_kw;

    let depth_ratio = figure.ap_mm / ap_c;
    let feed_ratio = figure.feed_mm_min / feed_c;
    let rpm_ratio = figure.rpm / rpm_c;

    // Split the shipped point into its two terms through the door's own
    // public surface. Power is affine in the feed, so two evaluations
    // determine the whole line: the reading at a zero feed IS the edge floor,
    // and the rest is the shear term. The probe feed is the calculator's,
    // which keeps the two magnitudes comparable so the subtraction stays well
    // conditioned.
    let edge_f = figure.required_kw_at_feed(0.0);
    let shear_slope_f = (figure.required_kw_at_feed(feed_c) - edge_f) / feed_c;

    // Scale the shipped terms back to the calculator's point. The shear slope
    // carries the cross-section and the width is held, so it scales with the
    // depth; the edge term carries the depth and the cutting velocity, so it
    // scales with the depth and the RPM.
    let shear_c = shear_slope_f / depth_ratio;
    let edge_c = edge_f / (depth_ratio * rpm_ratio);
    let reconstructed_calc_kw = shear_c * feed_c + edge_c;

    // Before the reconstruction is used it is checked against the number the
    // calculator itself published. This is what makes the arm a cross-check of
    // two doors rather than an identity on one.
    let reconstruction_error = (reconstructed_calc_kw - published_kw).abs() / published_kw;
    assert!(
        reconstruction_error < MODEL_IDENTITY_TOLERANCE,
        "the door's terms do not reproduce FeedsResult::power_kw at the \
         calculator's point: reconstructed {reconstructed_calc_kw:.9} kW vs \
         published {published_kw:.9} kW, relative error \
         {reconstruction_error:.3e}. The two doors have stopped assembling the \
         same PowerTerms; find the divergence, do not widen the tolerance."
    );

    let share_c = shear_c * feed_c / reconstructed_calc_kw;
    let predicted_ratio = depth_ratio * (share_c * feed_ratio + (1.0 - share_c) * rpm_ratio);
    let measured_ratio = figure.required_kw / published_kw;

    eprintln!(
        "  G-S2 headline | calculator ap {ap_c:.4} mm, feed {feed_c:.3} mm/min, \
         {rpm_c:.1} rpm -> {published_kw:.4} kW\n                \
         shipped    ap {:.4} mm, feed {:.3} mm/min, {:.1} rpm -> {:.4} kW\n                \
         depth ratio {depth_ratio:.6} x (shear share {share_c:.4} x feed ratio \
         {feed_ratio:.6} + edge share {:.4} x rpm ratio {rpm_ratio:.6}) = \
         {predicted_ratio:.6}; measured {measured_ratio:.6}",
        figure.ap_mm,
        figure.feed_mm_min,
        figure.rpm,
        figure.required_kw,
        1.0 - share_c,
    );

    assert!(
        figure.required_kw < published_kw,
        "S2 does not reproduce: the door reports {:.4} kW at the shipped depth \
         {:.4} mm, which is not BELOW the {published_kw:.4} kW the calculator \
         published at {ap_c:.4} mm. The defect this file protects against is a \
         published figure sized at a depth {:.2}x deeper than the cut.",
        figure.required_kw,
        figure.ap_mm,
        ap_c / figure.ap_mm,
    );

    let ratio_error = (measured_ratio - predicted_ratio).abs() / predicted_ratio;
    assert!(
        ratio_error < MODEL_IDENTITY_TOLERANCE,
        "the shipped figure is not the model's own answer at the shipped \
         point.\n    \
         P_ship / P_calc = (ap_f / ap_c) x (share_c x f_f / f_c + (1 - \
         share_c) x n_f / n_c)\n    \
         = {depth_ratio:.9} x ({share_c:.9} x {feed_ratio:.9} + {:.9} x \
         {rpm_ratio:.9})\n    \
         = {predicted_ratio:.9}, but power_at_operating_point measured \
         {measured_ratio:.9} ({:.4} kW / {published_kw:.4} kW), relative error \
         {ratio_error:.3e}.",
        1.0 - share_c,
        figure.required_kw,
    );
}

// ── The two doors are one evaluation ─────────────────────────────────

/// On the T-15 sentry's own fixture, the public door reproduces what Suggest
/// pass 10 reported. `PowerRecheckedAfterRescale` carries the ceiling it
/// judged against and the required power at the feed pass 9 handed it; the
/// door, called on the operation that ships, must agree on both — bit for
/// bit, because after S2 they ARE the same evaluation.
///
/// The fixture is the T-15 refusal one, restated rather than shared: a 2D
/// Adaptive rough on a Ø12 two-flute end mill, an operator depth of 30 mm
/// (2.5 x D, scale 0.625) and a synthetic 0.6 kW constant-power spindle
/// whose `adaptive_doc_factor` of 2.0 caps the depth at exactly 24.000 mm
/// (scale 0.75). Pass 9 raises the feed 1.2x. Feeds matrix R3 (2026-09-23)
/// put the feed on the continuous published scale, under which a depth
/// clamp alone cannot lift the load over a ceiling the calculator's point
/// satisfied, so pass 10 fires only where the feed-free edge term already
/// exceeds the ceiling: that is this spindle, and pass 10 answers
/// `fits_at_any_feed: false`.
#[test]
fn the_door_and_pass_ten_are_one_evaluation() {
    const T15_ENTRY_DPP_MM: f64 = 30.0;
    const T15_STEPOVER_MM: f64 = 10.0;

    let mut machine = MachineProfile::generic_wood_router();
    // RE-TUNED 0.8 -> 0.6 kW for ruling R4 Q2 (2026-09-24). The ceiling was
    // 0.8 x 0.75 = 0.6 kW; it is now the rated curve, 0.8 kW, and the
    // measured feed-free edge term 0.6975 kW sits UNDER it, so the fixture
    // lost its refusal shape. A 0.6 kW spindle restores the 0.6 kW ceiling:
    // 0.6975 > 0.6, no feed fits.
    machine.name = "SYNTHETIC 0.60 kW (test only)".to_owned();
    machine.power = rs_cam_core::machine::PowerModel::ConstantPower { power_kw: 0.6 };
    machine.rigidity.adaptive_doc_factor = 2.0;
    // Ruling R4 (2026-09-24): this fixture tests the Step 6 / pass 9 / pass 10
    // power interaction, not the aggressiveness dial. The dial at 1.0 keeps
    // pass 6b out of the geometry (it would scale the depth and the stepover
    // before pass 9 runs).
    machine.aggressiveness = 1.0;
    let material = hardwood();

    let mut tool = tool();
    tool.cutting_length = 40.0;

    let mut operation = OperationConfig::Adaptive(AdaptiveConfig {
        stepover: T15_STEPOVER_MM,
        depth: 2.0 * T15_ENTRY_DPP_MM,
        depth_per_pass: T15_ENTRY_DPP_MM,
        feed_rate: 1000.0,
        plunge_rate: 400.0,
        ..AdaptiveConfig::default()
    });
    let mut provenance = rs_cam_core::feeds::FeedsProvenance::default();
    let input = FeedsInput {
        flute_length: 40.0,
        operation: OperationFamily::Adaptive,
        radial_width_mm: Some(T15_STEPOVER_MM),
        ..feeds_input(&machine, &material, T15_ENTRY_DPP_MM)
    };
    let preview = FeedsPreview::build(&input);
    let rec = preview
        .applicable()
        .expect("a 2D adaptive rough with a flat end mill is a runnable pairing");

    let warnings = rs_cam_core::feeds::suggest::apply(
        &rec,
        ApplyScope::Both,
        &mut operation,
        &mut provenance,
        ApplyContext {
            tool: &tool,
            machine: &machine,
            material: &material,
            pass_role: PassRole::Roughing,
            suggest: SuggestContext::default(),
        },
    );

    let (rescaled_feed, reported_required, reported_available) = warnings
        .iter()
        .find_map(|w| match w {
            SuggestWarning::PowerRecheckedAfterRescale {
                rescaled_mm_per_min,
                required_kw_at_rescaled,
                available_kw,
                ..
            } => Some((
                *rescaled_mm_per_min,
                *required_kw_at_rescaled,
                *available_kw,
            )),
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "fixture is vacuous: pass 10 never reported a power re-check, \
                 so there is nothing to compare the door against. warnings: \
                 {warnings:?}"
            )
        });

    let figure = power_at_operating_point(&operation, &tool, &material, &machine, None)
        .expect("the T-15 fixture is a modelled cut");

    assert!(
        figure.available_kw.to_bits() == reported_available.to_bits(),
        "the door and pass 10 disagree about the ceiling: door \
         {:.12} kW, pass 10 {reported_available:.12} kW. Both must be \
         `power_at_rpm(rpm)` at the same RPM (no fraction since ruling R4 Q2).",
        figure.available_kw,
    );

    let door_required_at_rescaled = figure.required_kw_at_feed(rescaled_feed);
    assert!(
        door_required_at_rescaled.to_bits() == reported_required.to_bits(),
        "the door and pass 10 built different PowerTerms: at the rescaled feed \
         {rescaled_feed:.6} mm/min the door says {door_required_at_rescaled:.12} kW \
         and pass 10 reported {reported_required:.12} kW. After S2 these are \
         one evaluation, so they agree bit for bit or the lift was not \
         behaviour-neutral."
    );

    eprintln!(
        "  G-S2 parity | pass 10 ceiling {reported_available:.6} kW, required \
         at the rescaled feed {reported_required:.6} kW; the door reproduces \
         both exactly and reports {:.6} kW at the shipped {:.3} mm/min",
        figure.required_kw, figure.feed_mm_min,
    );

    // No feed fits this cut: the edge term alone is over the ceiling. Pass 10
    // put the feed back to the value pass 9 started from and said so; the
    // door, read at that shipped point, must agree that the ceiling is still
    // exceeded, or the two are not one evaluation.
    let edge = figure.required_kw_at_feed(0.0);
    assert!(
        edge > figure.available_kw * (1.0 - MODEL_IDENTITY_TOLERANCE),
        "the fixture is not the refusal shape: the feed-free edge term {edge:.6} kW is under \
         the {:.6} kW ceiling",
        figure.available_kw,
    );
    assert!(
        figure.required_kw > figure.available_kw * (1.0 - MODEL_IDENTITY_TOLERANCE),
        "pass 10 said no feed fits, but the door reads {:.6} kW under the {:.6} kW ceiling \
         at the shipped point.",
        figure.required_kw,
        figure.available_kw,
    );
}

// ── The refusals ─────────────────────────────────────────────────────

/// An absence refuses, typed, and each refusal says a different thing. A zero
/// would be a reading, and a modelled zero power is unreachable — the third
/// defect class.
#[test]
fn every_refusal_names_itself() {
    let machine = MachineProfile::generic_wood_router();
    let tool = tool();
    let hardwood = hardwood();

    // A drill has no radial width of cut, and the caller offers no fallback.
    let drill = OperationConfig::Drill(DrillConfig {
        depth: 20.0,
        feed_rate: 500.0,
        spindle_rpm: Some(10_000),
        ..DrillConfig::default()
    });
    let drill_refusal = power_at_operating_point(&drill, &tool, &hardwood, &machine, None)
        .expect_err("a drill has no radial engagement, so there is no power model");
    assert_eq!(drill_refusal, PowerUnmodeled::NoRadialEngagement);

    // Acrylic publishes no primary-source Kc. `material/mod.rs` refuses it
    // rather than predicting force from a fabricated constant.
    let acrylic = Material::Plastic {
        family: PlasticFamily::Acrylic,
    };
    let mut modelled_op = pocket_op(RIGIDITY_CAP_MM);
    modelled_op.set_feed_rate(1000.0);
    let material_refusal = power_at_operating_point(&modelled_op, &tool, &acrylic, &machine, None)
        .expect_err("acrylic publishes no Kc, so there is no power model");
    assert_eq!(material_refusal, PowerUnmodeled::MaterialUnvalidated);

    // A zero feed is not a zero-power cut. It is an operation with no feed.
    let mut zero_feed = pocket_op(RIGIDITY_CAP_MM);
    zero_feed.set_feed_rate(0.0);
    let feed_refusal = power_at_operating_point(&zero_feed, &tool, &hardwood, &machine, None)
        .expect_err("an operation with no feed has no operating point");
    assert_eq!(feed_refusal, PowerUnmodeled::NoFeed);

    // Every clause is non-empty, and no two variants say the same thing.
    let all = [
        PowerUnmodeled::MaterialUnvalidated,
        PowerUnmodeled::NoFeed,
        PowerUnmodeled::NoRadialEngagement,
        PowerUnmodeled::NoDepthPerPass,
        PowerUnmodeled::NoSpindleSpeed,
        PowerUnmodeled::NoAvailablePower,
        PowerUnmodeled::NoEngagementDiameter,
    ];
    for (i, a) in all.iter().enumerate() {
        assert!(
            !a.clause().is_empty(),
            "PowerUnmodeled::{a:?} carries an empty clause, so a surface has \
             nothing to paint in place of the number."
        );
        for b in all.iter().skip(i + 1) {
            assert!(
                a.clause() != b.clause(),
                "PowerUnmodeled::{a:?} and PowerUnmodeled::{b:?} share the \
                 clause {:?}. Two different absences must not read the same to \
                 the operator.",
                a.clause(),
            );
        }
    }
}

/// Non-vacuity for the refusals: the anchor cut is modelled, not refused, and
/// the `Ok` branch carries the two positive figures a 0-to-limit bar needs.
#[test]
fn the_anchor_cut_is_modelled_not_refused() {
    let machine = MachineProfile::generic_wood_router();
    let material = hardwood();
    let shipped = run_suggest_door(&machine, &material, REQUESTED_DPP_MM);

    let figure = power_at_operating_point(&shipped.operation, &tool(), &material, &machine, None)
        .expect("the anchor cut is a modelled cut");

    eprintln!(
        "  G-S2 non-vacuity | anchor draws {:.4} kW of a {:.4} kW ceiling \
         ({:.1}%) at ap {:.4} mm, ae {:.4} mm, {:.0} rpm, {:.1} mm/min",
        figure.required_kw,
        figure.available_kw,
        100.0 * figure.required_kw / figure.available_kw,
        figure.ap_mm,
        figure.ae_mm,
        figure.rpm,
        figure.feed_mm_min,
    );

    assert!(
        figure.required_kw > 0.0 && figure.required_kw.is_finite(),
        "the Ok branch must carry a modelled load, got {:.6} kW",
        figure.required_kw,
    );
    assert!(
        figure.available_kw > figure.required_kw,
        "fixture is vacuous: the anchor sits at or above its ceiling ({:.6} kW \
         required, {:.6} kW available), so this arm cannot show that a \
         modelled cut reads as one.",
        figure.required_kw,
        figure.available_kw,
    );
    assert!(
        figure.ap_mm > 0.0 && figure.ae_mm > 0.0 && figure.rpm > 0.0,
        "the figure must carry the point it was evaluated at, so a display can \
         print it: ap {:.4}, ae {:.4}, rpm {:.1}",
        figure.ap_mm,
        figure.ae_mm,
        figure.rpm,
    );
}
