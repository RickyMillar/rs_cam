//! Sentry: **a refused deflection prediction is not a zero** (G-T4, T-4,
//! 2026-09-18).
//!
//! ## The defect this pins
//!
//! `predict_peak_deflection_um` had nine refusal exits and every one
//! returned `DeflectionPrediction { predicted_um: 0.0, .. }`. Its own
//! module header called that the contract: "no constraint signal", and the
//! caller was to treat it as a pass-through.
//!
//! A modelled zero is unreachable. `tip_deflection_mm` returns 0.0 only
//! when the stickout, the modulus or the force is zero, or when the load
//! point sits at or below the tip; `lateral_cutting_force` returns a
//! strictly positive force for positive inputs. So for any operating point
//! that cleared the nine guards, `predicted_um > 0.0`. **Every 0.0 the
//! function returned was an absence**, written in a notation that cannot
//! say so.
//!
//! The Suggest deflection back-off read that zero. `0.0 > 200.0` is false,
//! the loop body never ran, `iterations` stayed at 0, and no warning, no
//! rationale entry and no trace event followed. A pass-through and a
//! clearance were the same event. The sharp case: an unvalidated plastic
//! on a roughing operation. Nine of the ten shipped plastics carry no
//! measured `Kc`, the stock picker names all ten, and the operator had no
//! way to know which one the model covers.
//!
//! ## What changed
//!
//! The refusal is a type. `predict_peak_deflection_um` returns
//! `Result<DeflectionPrediction, DeflectionUnmodeled>`, so `Ok` always
//! carries a modelled figure and every `Err` names itself. The same change
//! deleted the blanket V-bit guard: its stated reason was that the
//! post-simulation integrator handles V-bits, and this predictor now CALLS
//! that integrator. A V-bit's figure is real but its flute relief is
//! unmodelled, so the prediction carries
//! `DeflectionCaveat::FluteReliefUnmodeled` — a floor, not a bound.
//!
//! Pre-registered in `planning/TECH_DEBT_REGISTER.md` T-4 and
//! `planning/load_model_2026-09-16/T4_REFUSAL.md`.
//!
//! ## What is asserted
//!
//! 1. `a_rigid_cut_still_produces_a_figure` — **the non-vacuity anchor**.
//!    Every other arm asserts an `Err` or a caveat, and a predictor that
//!    refused every input would pass all of them. This arm runs first and
//!    demands `Ok`, strictly positive, finite, `caveat: None`.
//! 2. `no_modelled_operating_point_yields_a_zero` — the second anchor. A
//!    grid of valid operating points, none of which may return an `Ok`
//!    carrying the old sentinel.
//! 3. `every_refusal_names_itself` — the drill case, the unvalidated
//!    material case and the `Material::Custom`-with-a-positive-`kc` case,
//!    each asserting its exact variant.
//! 4. `a_v_bit_is_modelled_and_says_what_is_missing` — `Ok`, positive, and
//!    `Some(FluteReliefUnmodeled)`.
//! 5. `the_two_vocabularies_agree` — every variant maps to an
//!    `UnmodeledReason`, and every `clause()` is non-empty and unique. The
//!    fixture list is checked against an exhaustive match with no wildcard,
//!    so a tenth variant fails to compile rather than passing silently.
//! 6. `the_backoff_states_its_abstention` — Suggest on an unvalidated
//!    plastic roughing pocket emits `DeflectionBackoffUnmodeled` carrying
//!    the reason, and the same operation in hard maple never emits it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::predict::{
    DeflectionCaveat, DeflectionUnmodeled, predict_peak_deflection_um,
};
use rs_cam_core::feeds::suggest::{
    SuggestContext, SuggestForOperationInput, SuggestWarning, suggest_for_operation,
};
use rs_cam_core::feeds::{SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, PlasticFamily, WoodSpecies};
use rs_cam_core::tool_load::verdict::UnmodeledReason;

// ── Fixtures ────────────────────────────────────────────────────────────

/// A stubby carbide end mill: 6 mm, four flutes, 45 mm stickout. The
/// operating point a wood router runs every day.
fn carbide_endmill() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.shaft_diameter = 6.0;
    tool.shank_diameter = 6.0;
    tool.stickout = 45.0;
    tool.flute_count = 4;
    tool
}

/// The same tool as a V-bit. Diameter and stickout match
/// [`carbide_endmill`] so the two differ only in cutter shape.
fn carbide_vbit() -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(2), ToolType::VBit);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.shaft_diameter = 6.0;
    tool.shank_diameter = 6.0;
    tool.stickout = 45.0;
    tool.included_angle = 60.0;
    tool.flute_count = 2;
    tool
}

/// A roughing pocket at a configured depth per pass, stepover, feed and
/// speed. Every input the predictor reads is set.
fn roughing_pocket(depth_per_pass: f64) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        depth: 4.0 * depth_per_pass,
        depth_per_pass,
        stepover: 2.0,
        feed_rate: 1800.0,
        plunge_rate: 500.0,
        spindle_rpm: Some(18_000),
        ..PocketConfig::default()
    })
}

fn hard_maple() -> Material {
    Material::SolidWood {
        species: WoodSpecies::HardMaple,
    }
}

/// Acrylic. One of the nine shipped plastics with no measured `Kc`.
fn acrylic() -> Material {
    Material::Plastic {
        family: PlasticFamily::Acrylic,
    }
}

/// A custom material carrying a POSITIVE `kc`. It clears the early `Kc`
/// guard and then abstains at the delegate — the exit the module header
/// used to leave out.
fn custom_with_a_real_kc() -> Material {
    Material::Custom {
        name: "operator entry".to_owned(),
        feed_scale_factor: 1.0,
        kc: 30.0,
    }
}

// ── 1. The non-vacuity anchor ───────────────────────────────────────────

#[test]
fn a_rigid_cut_still_produces_a_figure() {
    let tool = carbide_endmill();
    let op = roughing_pocket(3.0);
    let machine = MachineProfile::default();

    let prediction = predict_peak_deflection_um(&op, &tool, &hard_maple(), &machine)
        .expect("a 6 mm carbide end mill at 45 mm stickout in hard maple is a modelled cut");

    assert!(
        prediction.predicted_um.is_finite() && prediction.predicted_um > 0.0,
        "the anchor must carry a real figure, got {} µm",
        prediction.predicted_um
    );
    assert_eq!(
        prediction.caveat, None,
        "a flat end mill takes the measured equivalent-diameter fraction, so it carries no caveat"
    );
    assert!(
        prediction.breakdown.f_lateral_n > 0.0,
        "the breakdown must record the force the figure came from"
    );
    assert!(
        (prediction.breakdown.stickout_mm - 45.0).abs() < 1e-9,
        "the breakdown must report the stickout the model used, got {} mm",
        prediction.breakdown.stickout_mm
    );
}

// ── 2. The Ok branch never carries the old sentinel ─────────────────────

#[test]
fn no_modelled_operating_point_yields_a_zero() {
    let machine = MachineProfile::default();
    let material = hard_maple();
    let mut evaluated = 0_usize;

    for stickout in [25.0_f64, 45.0, 70.0] {
        for diameter in [3.0_f64, 6.0, 12.0] {
            for dpp in [0.5_f64, 3.0, 9.0] {
                let mut tool = carbide_endmill();
                tool.diameter = diameter;
                tool.shaft_diameter = diameter;
                tool.shank_diameter = diameter;
                tool.stickout = stickout;
                let op = roughing_pocket(dpp);

                let prediction = predict_peak_deflection_um(&op, &tool, &material, &machine)
                    .expect("every point on this grid is a modelled cut");
                evaluated += 1;
                assert!(
                    prediction.predicted_um > 0.0 && prediction.predicted_um.is_finite(),
                    "Ø{diameter} mm at {stickout} mm stickout and {dpp} mm DPP returned \
                     {} µm — the Ok branch must never carry the refusal sentinel",
                    prediction.predicted_um
                );
            }
        }
    }

    assert_eq!(
        evaluated, 27,
        "the sweep must actually run; a grid that evaluated nothing proves nothing"
    );
}

// ── 3. Each refusal names itself ────────────────────────────────────────

#[test]
fn every_refusal_names_itself() {
    let tool = carbide_endmill();
    let machine = MachineProfile::default();

    // The drill family. Z-only kinematics, no continuous radial
    // engagement — the refusal is correct and the operator needs no
    // action. That is exactly why it must not read like the next two.
    let drill = OperationConfig::new_default(OperationType::Drill);
    assert_eq!(
        predict_peak_deflection_um(&drill, &tool, &hard_maple(), &machine)
            .expect_err("a drill carries no cantilever engagement"),
        DeflectionUnmodeled::NotApplicableForOp(OperationType::Drill)
    );

    // An unvalidated material. The highest-frequency unsafe case: the
    // stock picker offers ten plastics and nine of them refuse.
    assert!(
        acrylic().kc_n_per_mm2().is_none(),
        "fixture premise: Acrylic must carry no measured Kc"
    );
    assert_eq!(
        predict_peak_deflection_um(&roughing_pocket(3.0), &tool, &acrylic(), &machine)
            .expect_err("Acrylic carries no measured cutting coefficient"),
        DeflectionUnmodeled::MaterialUnvalidated
    );

    // A custom material with a POSITIVE kc. It clears the early Kc guard
    // and the delegate refuses it anyway. Before T-4 this exit hid inside
    // a `.map_or(0.0, …)` and the module header did not list it.
    let custom = custom_with_a_real_kc();
    assert_eq!(
        custom.kc_n_per_mm2(),
        Some(30.0),
        "fixture premise: this custom material must PASS the early Kc guard, \
         otherwise the arm proves nothing about the hidden exit"
    );
    assert_eq!(
        predict_peak_deflection_um(&roughing_pocket(3.0), &tool, &custom, &machine)
            .expect_err("a custom material carries no validated force model"),
        DeflectionUnmodeled::MaterialCustom
    );
}

// ── 4. A V-bit is modelled, and says what is missing ────────────────────

#[test]
fn a_v_bit_is_modelled_and_says_what_is_missing() {
    let machine = MachineProfile::default();
    let op = roughing_pocket(3.0);

    let prediction = predict_peak_deflection_um(&op, &carbide_vbit(), &hard_maple(), &machine)
        .expect(
            "T-4 deleted the blanket V-bit guard: the predictor delegates to the integrator, \
             and the integrator reads a V-bit's local diameter through lookup_diameter_at",
        );

    assert!(
        prediction.predicted_um > 0.0 && prediction.predicted_um.is_finite(),
        "the V-bit figure must be a real number, got {} µm",
        prediction.predicted_um
    );
    assert_eq!(
        prediction.caveat,
        Some(DeflectionCaveat::FluteReliefUnmodeled),
        "a V-bit's cone is modelled and its flute relief is not, and that gap runs in the \
         UNSAFE direction — the figure is a floor and must say so"
    );
    assert!(
        !prediction
            .caveat
            .expect("asserted above")
            .clause()
            .is_empty(),
        "the caveat must carry operator-facing wording"
    );
}

// ── 5. One vocabulary, not two ──────────────────────────────────────────

/// Every `DeflectionUnmodeled` variant, as a fixture list.
///
/// The exhaustive match below carries no wildcard arm, so a tenth variant
/// fails to compile here rather than slipping past the list.
fn every_variant() -> Vec<DeflectionUnmodeled> {
    let all = vec![
        DeflectionUnmodeled::NotApplicableForOp(OperationType::Drill),
        DeflectionUnmodeled::MaterialUnvalidated,
        DeflectionUnmodeled::MaterialCustom,
        DeflectionUnmodeled::NoDiameter,
        DeflectionUnmodeled::NoDepthPerPass,
        DeflectionUnmodeled::NoRadialEngagement,
        DeflectionUnmodeled::NoChipload,
        DeflectionUnmodeled::NoStickout,
        DeflectionUnmodeled::DegenerateCantilever,
    ];
    for variant in &all {
        match variant {
            DeflectionUnmodeled::NotApplicableForOp(_)
            | DeflectionUnmodeled::MaterialUnvalidated
            | DeflectionUnmodeled::MaterialCustom
            | DeflectionUnmodeled::NoDiameter
            | DeflectionUnmodeled::NoDepthPerPass
            | DeflectionUnmodeled::NoRadialEngagement
            | DeflectionUnmodeled::NoChipload
            | DeflectionUnmodeled::NoStickout
            | DeflectionUnmodeled::DegenerateCantilever => {}
        }
    }
    all
}

#[test]
fn the_two_vocabularies_agree() {
    let variants = every_variant();
    assert_eq!(
        variants.len(),
        9,
        "the fixture list must cover every variant; the match in every_variant() \
         forces a new one to be visited, and this count forces it to be LISTED"
    );

    let mut clauses: BTreeSet<&'static str> = BTreeSet::new();
    for variant in &variants {
        let clause = variant.clause();
        assert!(
            !clause.trim().is_empty(),
            "{variant:?} carries an empty clause; the operator reads this string"
        );
        assert!(
            clauses.insert(clause),
            "{variant:?} repeats a clause another variant already uses — two findings \
             that print the same words are one finding to the operator"
        );

        // The post-simulation half of the product must word this the same
        // way. `MaterialUnvalidated` is the shared vocabulary's own
        // variant; the input-shaped refusals carry the clause verbatim.
        match variant.as_unmodeled_reason() {
            UnmodeledReason::NotApplicableForOp(detail) => {
                assert!(
                    !detail.trim().is_empty(),
                    "{variant:?} maps to an empty NotApplicableForOp detail"
                );
            }
            UnmodeledReason::MaterialUnvalidated => {
                assert!(
                    matches!(
                        variant,
                        DeflectionUnmodeled::MaterialUnvalidated
                            | DeflectionUnmodeled::MaterialCustom
                    ),
                    "{variant:?} must not claim a material refusal"
                );
            }
            UnmodeledReason::NotImplemented(detail) => {
                assert_eq!(
                    detail, clause,
                    "{variant:?} must carry its own clause into the post-simulation vocabulary"
                );
            }
            other => panic!(
                "{variant:?} maps to {other:?}, which the pre-simulation predictor cannot \
                 claim — it has run no simulation and read no vendor row"
            ),
        }
    }
}

// ── 6. The back-off states its abstention ───────────────────────────────

/// Run Suggest end to end and return the warnings it emitted.
fn suggest_warnings(material: &Material) -> Vec<SuggestWarning> {
    let tool = carbide_endmill();
    let machine = MachineProfile::default();
    let op = roughing_pocket(3.0);

    suggest_for_operation(SuggestForOperationInput {
        operation: &op,
        tool: &tool,
        machine: &machine,
        material,
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::MatchChart,
        context: SuggestContext::default(),
    })
    .expect("a flat end mill on a pocket is a runnable pairing")
    .warnings
}

#[test]
fn the_backoff_states_its_abstention() {
    let unmodelled = suggest_warnings(&acrylic());
    let reason = unmodelled
        .iter()
        .find_map(|w| match w {
            SuggestWarning::DeflectionBackoffUnmodeled { reason, .. } => Some(*reason),
            _ => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "an unvalidated plastic on a roughing pocket must report that the deflection \
                 back-off did not run; got warnings: {unmodelled:?}"
            )
        });
    assert_eq!(
        reason,
        DeflectionUnmodeled::MaterialUnvalidated,
        "the warning must carry the reason, not just the fact of an abstention"
    );

    // The counterpart. A modelled material must never produce the
    // abstention — otherwise the arm above would pass on a build that
    // abstained on everything.
    let modelled = suggest_warnings(&hard_maple());
    assert!(
        !modelled
            .iter()
            .any(|w| matches!(w, SuggestWarning::DeflectionBackoffUnmodeled { .. })),
        "hard maple carries a measured Kc, so the back-off must run; got warnings: {modelled:?}"
    );
}
