//! G-LADDER (trap D7 of
//! `planning/adaptive3d_step_ladder_roughing_2026-09-24/PLAN.md`): a coarse
//! step of the 3D Rough step ladder is the deepest axial bite, and Suggest
//! reads it as that.
//!
//! Before this sentry the feeds readers read `depth_per_pass` as the deepest
//! bite. A 3D Rough with `depth_per_pass = 3` and `coarse_steps = [20]` then
//! showed the power, the deflection and the axial envelope of a 3 mm cut, and
//! the envelope never clamped the 20 mm step.
//!
//! ## The fixture
//!
//! A Ø6 two-flute flat end mill in generic hardwood. The stickout is 60 mm
//! (10 × D) so that the deflection bound of the axial envelope is well under
//! 20 mm; the arm that needs it asserts that first. The flutes are 30 mm, so
//! the flute-length clamp does not reach 20 mm.
//!
//! ## The arms
//!
//! - `the_power_and_the_deflection_read_the_coarse_step`: an identity. The
//!   ladder `[20]` over a 3 mm base step reads the same power and deflection
//!   as a single 20 mm step, and not the same as a single 3 mm step.
//! - `the_envelope_clamps_the_coarse_step_with_a_record`: the invariant
//!   passes on their own (no calculator point). The envelope cap is computed
//!   here through the same public door the pass uses. The 20 mm step moves to
//!   the cap and carries its own `AxialDocClampedByEnvelope` record.
//! - `the_full_suggest_door_ships_every_step_at_or_below_every_cap`: the
//!   whole funnel. Every step that ships is at or below every clamp that a
//!   record states, and the ladder that ships is valid. The funnel writes the
//!   ladder back from its scratch copy, so a missing write-back fails here.
//! - `an_empty_ladder_moves_no_number`: an empty ladder and a config that
//!   never carried the key give the same Suggest output, and no coarse-step
//!   record.
//!
//! The editor could not derive the envelope cap and the shipped ladder by
//! hand; the arms print them (`eprintln!`) for the orchestrator to record.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    // SAFETY: the sentry prints the cap and the shipped ladder it measured,
    // because the editor could not derive them by hand.
    clippy::print_stderr
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::compute::operation_configs::Adaptive3dConfig;
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::cutter_constraints::{
    DEFAULT_ROUGH_DEFLECTION_LIMIT_UM, cutter_axial_constraints,
};
use rs_cam_core::feeds::suggest::{
    ApplyContext, ApplyScope, SuggestContext, SuggestWarning, feeds_preview_for_operation,
    resolve_operation_invariants,
};
use rs_cam_core::feeds::{
    PassRole, SpindleStrategy, embedded_vendor_lut, power_at_operating_point,
    predict_peak_deflection_um,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

const DIAMETER_MM: f64 = 6.0;
const FLUTES: u32 = 2;
const BASE_STEP_MM: f64 = 3.0;
const COARSE_STEP_MM: f64 = 20.0;
const STEPOVER_MM: f64 = 2.4;
const FEED_MM_MIN: f64 = 1500.0;
const RPM: u32 = 18_000;

/// The `param_name` the envelope gives a coarse-step clamp. It carries the
/// word "depth", so the card routes the record to its DOC row.
const COARSE_PARAM: &str = "coarse step depth";

fn tool() -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    t.diameter = DIAMETER_MM;
    t.shank_diameter = DIAMETER_MM;
    t.flute_count = FLUTES;
    t.cutting_length = 30.0;
    t.stickout = 60.0;
    t
}

fn hardwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    }
}

fn rough(depth_per_pass: f64, coarse_steps: Vec<f64>) -> OperationConfig {
    OperationConfig::Adaptive3d(Adaptive3dConfig {
        stepover: STEPOVER_MM,
        depth_per_pass,
        coarse_steps,
        feed_rate: FEED_MM_MIN,
        plunge_rate: 500.0,
        spindle_rpm: Some(RPM),
        ..Adaptive3dConfig::default()
    })
}

fn coarse_steps(op: &OperationConfig) -> Vec<f64> {
    match op {
        OperationConfig::Adaptive3d(cfg) => cfg.coarse_steps.clone(),
        other => panic!("not a 3D Rough: {other:?}"),
    }
}

/// The ladder rule of the adapter: coarsest first, strictly descending, and
/// each coarse step above the base step.
fn assert_ladder_valid(op: &OperationConfig, ctx: &str) {
    let steps = coarse_steps(op);
    let base = op.depth_per_pass().expect("a 3D Rough has a base step");
    for pair in steps.windows(2) {
        assert!(
            pair[0] > pair[1],
            "{ctx}: the ladder {steps:?} is not strictly descending"
        );
    }
    for s in &steps {
        assert!(
            *s > base,
            "{ctx}: the coarse step {s} is not above the base step {base} ({steps:?})"
        );
    }
}

/// Every clamp that a record states. The deepest step that ships must be at
/// or below each of them.
fn stated_caps(warnings: &[SuggestWarning]) -> Vec<f64> {
    warnings
        .iter()
        .filter_map(|w| match w {
            SuggestWarning::AxialDocClampedByEnvelope { clamped_mm, .. } => Some(*clamped_mm),
            SuggestWarning::AxialEnvelopeSafeBandEmpty {
                max_safe_doc_mm, ..
            } => Some(*max_safe_doc_mm),
            SuggestWarning::RoughingDepthClampedToRigidity { capped, .. }
            | SuggestWarning::DepthClampedToCuttingLength { capped, .. } => Some(*capped),
            SuggestWarning::DppCappedByDeflection { capped_mm, .. } => Some(*capped_mm),
            _ => None,
        })
        .collect()
}

fn coarse_records(warnings: &[SuggestWarning]) -> Vec<(f64, f64, &'static str)> {
    warnings
        .iter()
        .filter_map(|w| match w {
            SuggestWarning::AxialDocClampedByEnvelope {
                param_name,
                commanded_mm,
                clamped_mm,
                binding,
                ..
            } if *param_name == COARSE_PARAM => Some((*commanded_mm, *clamped_mm, *binding)),
            _ => None,
        })
        .collect()
}

/// The axial envelope cap for this fixture, from the same public door that
/// Suggest pass 0 calls for a 3D Rough: rough deflection limit, radial WOC =
/// stepover, no vendor row (the invariant-only arm passes no row).
fn envelope_cap_without_a_row() -> f64 {
    let fz = FEED_MM_MIN / (f64::from(RPM) * f64::from(FLUTES));
    cutter_axial_constraints(
        &build_cutter(&tool()),
        &hardwood(),
        STEPOVER_MM,
        fz,
        None,
        None,
        Some(DEFAULT_ROUGH_DEFLECTION_LIMIT_UM),
    )
    .safe_max_doc_mm()
}

// ── Arm 1: the readers ──────────────────────────────────────────────

#[test]
fn the_power_and_the_deflection_read_the_coarse_step() {
    let machine = MachineProfile::generic_wood_router();
    let material = hardwood();
    let tool = tool();
    let ladder = rough(BASE_STEP_MM, vec![COARSE_STEP_MM]);
    let single_deep = rough(COARSE_STEP_MM, Vec::new());
    let single_base = rough(BASE_STEP_MM, Vec::new());

    assert_eq!(ladder.deepest_axial_step(), Some(COARSE_STEP_MM));

    let p_ladder = power_at_operating_point(&ladder, &tool, &material, &machine, None)
        .expect("a hardwood 3D Rough with feed, RPM and stepover is modelled");
    let p_deep =
        power_at_operating_point(&single_deep, &tool, &material, &machine, None).expect("modelled");
    let p_base =
        power_at_operating_point(&single_base, &tool, &material, &machine, None).expect("modelled");
    assert_eq!(
        p_ladder.ap_mm, COARSE_STEP_MM,
        "the power door read the depth {} mm, not the coarse step",
        p_ladder.ap_mm
    );
    assert_eq!(p_ladder.required_kw.to_bits(), p_deep.required_kw.to_bits());
    assert!(
        p_ladder.required_kw > p_base.required_kw,
        "non-vacuity: the 20 mm step must draw more power than the 3 mm step \
         ({} kW vs {} kW)",
        p_ladder.required_kw,
        p_base.required_kw
    );

    let d_ladder = predict_peak_deflection_um(&ladder, &tool, &material, &machine)
        .expect("a flat end mill in hardwood is modelled");
    let d_deep =
        predict_peak_deflection_um(&single_deep, &tool, &material, &machine).expect("modelled");
    let d_base =
        predict_peak_deflection_um(&single_base, &tool, &material, &machine).expect("modelled");
    assert_eq!(
        d_ladder.predicted_um.to_bits(),
        d_deep.predicted_um.to_bits(),
        "the predictor read {} um on the ladder and {} um on a single 20 mm step",
        d_ladder.predicted_um,
        d_deep.predicted_um
    );
    assert!(
        d_ladder.predicted_um > d_base.predicted_um,
        "non-vacuity: 20 mm must deflect more than 3 mm"
    );
    eprintln!(
        "G-LADDER arm 1: power {:.4} kW (ladder) vs {:.4} kW (3 mm); deflection \
         {:.1} um (ladder) vs {:.1} um (3 mm)",
        p_ladder.required_kw, p_base.required_kw, d_ladder.predicted_um, d_base.predicted_um
    );
}

// ── Arm 2: the envelope, invariant passes only ──────────────────────

#[test]
fn the_envelope_clamps_the_coarse_step_with_a_record() {
    let machine = MachineProfile::generic_wood_router();
    let material = hardwood();
    let tool = tool();
    let cap = envelope_cap_without_a_row();
    eprintln!("G-LADDER arm 2: envelope cap (no vendor row) = {cap:.4} mm");
    assert!(
        cap > 0.0 && cap < COARSE_STEP_MM,
        "fixture is vacuous: the envelope cap {cap} mm is not under the 20 mm \
         coarse step. Lengthen the stickout, do not remove the arm."
    );

    let mut op = rough(BASE_STEP_MM, vec![COARSE_STEP_MM]);
    let warnings = resolve_operation_invariants(
        &mut op,
        &tool,
        &machine,
        &material,
        PassRole::Roughing,
        SuggestContext::default(),
    );
    eprintln!(
        "G-LADDER arm 2: shipped base {:?}, ladder {:?}; warnings {warnings:?}",
        op.depth_per_pass(),
        coarse_steps(&op)
    );

    let records = coarse_records(&warnings);
    assert_eq!(
        records.len(),
        1,
        "the 20 mm step must carry exactly one envelope clamp record; got \
         {records:?} in {warnings:?}"
    );
    let (commanded, clamped, binding) = records[0];
    assert_eq!(
        commanded, COARSE_STEP_MM,
        "the record names the step it moved"
    );
    assert!(
        (clamped - cap).abs() < 1e-9,
        "the record clamps to {clamped} mm, the envelope cap is {cap} mm"
    );
    assert!(!binding.is_empty());
    assert!(
        COARSE_PARAM.contains("depth"),
        "the card routes a record by the word \"depth\""
    );

    let deepest = op.deepest_axial_step().unwrap();
    for c in stated_caps(&warnings) {
        assert!(
            deepest <= c + 1e-9,
            "the deepest step {deepest} mm ships above the stated cap {c} mm"
        );
    }
    assert_ladder_valid(&op, "arm 2");
}

// ── Arm 3: the whole Suggest door ───────────────────────────────────

fn run_suggest(op: &mut OperationConfig, machine: &MachineProfile) -> Vec<SuggestWarning> {
    let tool = tool();
    let material = hardwood();
    let preview = feeds_preview_for_operation(
        op,
        &tool,
        &material,
        machine,
        embedded_vendor_lut(),
        SpindleStrategy::MatchChart,
    );
    let rec = preview
        .applicable()
        .expect("a flat end mill on a 3D Rough in hardwood is a runnable pairing");
    let mut provenance = rs_cam_core::feeds::FeedsProvenance::default();
    rs_cam_core::feeds::suggest::apply(
        &rec,
        ApplyScope::Both,
        op,
        &mut provenance,
        ApplyContext {
            tool: &tool,
            machine,
            material: &material,
            pass_role: PassRole::Roughing,
            suggest: SuggestContext::default(),
        },
    )
}

#[test]
fn the_full_suggest_door_ships_every_step_at_or_below_every_cap() {
    let machine = MachineProfile::generic_wood_router();
    let mut op = rough(BASE_STEP_MM, vec![COARSE_STEP_MM]);
    let warnings = run_suggest(&mut op, &machine);
    eprintln!(
        "G-LADDER arm 3: shipped base {:?}, ladder {:?}, deepest {:?}; coarse \
         records {:?}",
        op.depth_per_pass(),
        coarse_steps(&op),
        op.deepest_axial_step(),
        coarse_records(&warnings)
    );
    let deepest = op.deepest_axial_step().unwrap();
    let caps = stated_caps(&warnings);
    for c in &caps {
        assert!(
            deepest <= c + 1e-9,
            "the deepest step {deepest} mm ships above the stated cap {c} mm; \
             warnings {warnings:?}"
        );
    }
    for (commanded, clamped, _) in coarse_records(&warnings) {
        assert!(commanded > clamped, "a clamp record moves a step down");
    }
    assert_ladder_valid(&op, "arm 3");
}

// ── Arm 4: an empty ladder ──────────────────────────────────────────

#[test]
fn an_empty_ladder_moves_no_number() {
    let machine = MachineProfile::generic_wood_router();

    // A config that never carried the key: serialise an empty ladder (the
    // key is not written) and read it back.
    let explicit = rough(BASE_STEP_MM, Vec::new());
    let OperationConfig::Adaptive3d(cfg) = &explicit else {
        panic!("rough() builds a 3D Rough");
    };
    let text = toml::to_string(cfg).expect("serialise");
    assert!(!text.contains("coarse_steps"), "{text}");
    let parsed: Adaptive3dConfig = toml::from_str(&text).expect("read back");
    let mut without_key = OperationConfig::Adaptive3d(parsed);
    let mut with_empty = explicit.clone();

    let w_key = run_suggest(&mut without_key, &machine);
    let w_empty = run_suggest(&mut with_empty, &machine);

    assert_eq!(format!("{with_empty:?}"), format!("{without_key:?}"));
    assert_eq!(format!("{w_empty:?}"), format!("{w_key:?}"));
    assert!(
        coarse_steps(&with_empty).is_empty(),
        "Suggest added a ladder"
    );
    assert!(coarse_records(&w_empty).is_empty(), "{w_empty:?}");
    assert_eq!(with_empty.deepest_axial_step(), with_empty.depth_per_pass());

    // The readers: an empty ladder reads the base step.
    let tool = tool();
    let material = hardwood();
    let p =
        power_at_operating_point(&explicit, &tool, &material, &machine, None).expect("modelled");
    assert_eq!(p.ap_mm, BASE_STEP_MM);
}
