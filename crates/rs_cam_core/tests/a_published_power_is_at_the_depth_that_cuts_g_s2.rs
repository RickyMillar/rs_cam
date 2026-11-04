//! S2 — the power figure a surface displays is evaluated at the depth that
//! will be cut.
//!
//! Feeds contract P5 (2026-11-03): the history below describes the chain
//! (`enforce_invariants`), which production no longer runs. The arms that
//! read it are retired (see "The arms"). The door, its refusals and its
//! anchor stay; the anchor now ships through the production apply.
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
//! - Retired by feeds contract P5 (2026-11-03): the three arms that read the
//!   rigidity clamp and pass 10 through the chain door
//!   (`the_published_power_describes_a_depth_that_will_not_be_cut`,
//!   `the_rigidity_clamp_moves_the_depth_on_this_fixture`,
//!   `the_door_and_pass_ten_are_one_evaluation`). Production no longer
//!   clamps the depth after the calculator: the calculator sizes its power
//!   at the stated cut or at the envelope pick, which is the cut that ships.
//! - [`every_refusal_names_itself`] — a drill, a material with no force line and a
//!   zero feed each refuse with a distinct, non-empty clause.
//! - [`the_anchor_cut_is_modelled_not_refused`] — non-vacuity for the
//!   refusals: the anchor returns `Ok` with `available_kw > required_kw > 0`.

#![expect(
    clippy::expect_used,
    clippy::print_stderr,
    reason = "a sentry that measures a ratio should print the ratio it measured, the same allowance the sibling power instruments take."
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{DrillConfig, PocketConfig};
use rs_cam_core::compute::tool_config::ToolFact;
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::envelope::PickRequest;
use rs_cam_core::feeds::suggest::{OperationApplyInput, SuggestContext, apply_to_operation};
use rs_cam_core::feeds::{
    PowerUnmodeled, SpindleStrategy, embedded_vendor_lut, power_at_operating_point,
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

/// A Ø12 two-flute flat end mill, long enough that neither the flute guard
/// (0.8 × 50 = 40 mm) nor the cutting-length clamp (50 mm) reaches the
/// requested 36 mm depth. All fixture choices, not model constants.
fn tool() -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    t.diameter = DIAMETER_MM;
    t.set_flute_count(ToolFact::Stated(FLUTES));
    t.set_cutting_length(ToolFact::Stated(50.0));
    // Ruling R4 Q7 (2026-09-24): above 4 x D stickout the long-tool share
    // (0.88) lowers the dial's load target, so even at aggressiveness 1.0
    // pass 6b would scale the depth (measured 24 -> 20 mm here, 2.4 -> 1.8
    // in g_s2). 48 mm = 4.0 x D takes no share, and the dial stays out.
    t.set_stickout(ToolFact::Stated(48.0));
    t
}

fn hardwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    }
}

fn pocket_op(dpp: f64) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        stepover: Some(STEPOVER_MM),
        // Two full passes, so `realised_step_down` snaps the depth to itself
        // and the entry value the invariants see is the requested one.
        floor: rs_cam_core::compute::floor::Floor::top(2.0 * dpp),
        depth_per_pass: Some(dpp),
        feed_rate: Some(1000.0),
        plunge_rate: Some(400.0),
        ..PocketConfig::default()
    })
}

/// The anchor cut as production ships it: `Command::ApplyFeeds` with the
/// envelope proposal (feeds contract P5, K2b). The proposal lowers the
/// requested depth to its binding cap, and the speeds are sized there.
fn apply_all(machine: &MachineProfile, material: &Material, requested_ap: f64) -> OperationConfig {
    let mut operation = pocket_op(requested_ap);
    let mut provenance = rs_cam_core::feeds::FeedsProvenance::default();
    apply_to_operation(
        OperationApplyInput {
            tool: &tool(),
            machine,
            material,
            spindle_strategy: SpindleStrategy::MatchChart,
            lut: embedded_vendor_lut(),
            suggest: SuggestContext::default(),
        },
        Some(PickRequest::Proposal),
        None,
        &mut operation,
        &mut provenance,
    )
    .expect("a roughing pocket with a flat end mill is a runnable pairing");
    operation
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
        floor: rs_cam_core::compute::floor::Floor::Top { depth: 20.0 },
        feed_rate: Some(500.0),
        spindle_rpm: Some(10_000),
        ..DrillConfig::default()
    });
    let drill_refusal = power_at_operating_point(&drill, &tool, &hardwood, &machine, None)
        .expect_err("a drill has no radial engagement, so there is no power model");
    assert_eq!(drill_refusal, PowerUnmodeled::NoRadialEngagement);

    // Acrylic has no force line (ruling B6: no plastic does).
    // `Material::force_line` refuses it rather than predicting force from a
    // fabricated constant.
    let acrylic = Material::Plastic {
        family: PlasticFamily::Acrylic,
    };
    let mut modelled_op = pocket_op(RIGIDITY_CAP_MM);
    modelled_op.set_feed_rate(1000.0);
    let material_refusal = power_at_operating_point(&modelled_op, &tool, &acrylic, &machine, None)
        .expect_err("acrylic has no force line, so there is no power model");
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
    let shipped = apply_all(&machine, &material, REQUESTED_DPP_MM);

    let figure = power_at_operating_point(&shipped, &tool(), &material, &machine, None)
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
