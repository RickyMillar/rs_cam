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
//!   Operator ruling 1 (2026-09-24, "keep my steps, only cap"): the funnel
//!   keeps the operator's base step 3.0 and does not write the calculator's
//!   depth. The base ships at 3.0, or at a lower value that a record states.
//!   The ladder ships non-empty, or each step that went has a
//!   `CoarseStepRemoved` note. The arm sets the dial to a load share of 1.0
//!   (aggressiveness = 1 / long-tool share), so the dial does not scale the
//!   steps and the base pin is exact.
//! - `an_empty_ladder_moves_no_number`: an empty ladder and a config that
//!   never carried the key give the same Suggest output, and no coarse-step
//!   record.
//! - `a_capped_step_that_meets_the_base_is_removed_with_a_note`: the
//!   invariant passes on their own, with the base step AT the envelope cap.
//!   The envelope lowers the 20 mm step to the cap, which is not above the
//!   base step, so the step goes. Exactly one `CoarseStepRemoved` note says
//!   so, and names 20 mm, the cap, the base step and "axial envelope".
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

/// Each `CoarseStepRemoved` note: `(step, lowered to, next step, next is
/// the base, cap)`.
fn removal_notes(warnings: &[SuggestWarning]) -> Vec<(f64, f64, f64, bool, &'static str)> {
    warnings
        .iter()
        .filter_map(|w| match w {
            SuggestWarning::CoarseStepRemoved {
                step_mm,
                lowered_to_mm,
                next_step_mm,
                next_is_base,
                cap,
            } => Some((*step_mm, *lowered_to_mm, *next_step_mm, *next_is_base, *cap)),
            _ => None,
        })
        .collect()
}

/// Operator ruling 1 (2026-09-24): no step leaves the ladder with no note.
/// Each coarse step of the input either ships or has one removal note.
fn assert_no_silent_removal(
    op: &OperationConfig,
    warnings: &[SuggestWarning],
    input_steps: usize,
    ctx: &str,
) {
    let shipped = coarse_steps(op).len();
    let notes = removal_notes(warnings);
    assert_eq!(
        shipped + notes.len(),
        input_steps,
        "{ctx}: {input_steps} coarse step(s) went in, {shipped} shipped and \
         {} removal note(s) were filed: a step left the ladder with no note. \
         Notes {notes:?}; warnings {warnings:?}",
        notes.len()
    );
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
    assert_no_silent_removal(&op, &warnings, 1, "arm 2");
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

/// The generic router with the dial at a load share of 1.0 for this tool:
/// aggressiveness = 1 / long-tool share. The dial then does not act, so the
/// base step that ships is the operator's value or a stated cap.
fn neutral_dial_machine() -> MachineProfile {
    let mut machine = MachineProfile::generic_wood_router();
    let t = tool();
    let share = rs_cam_core::feeds::long_tool_load_share(t.stickout, t.diameter);
    assert!(share.is_finite() && share > 0.0, "long-tool share {share}");
    machine.aggressiveness = 1.0 / share;
    machine
}

#[test]
fn the_full_suggest_door_ships_every_step_at_or_below_every_cap() {
    let machine = neutral_dial_machine();
    let mut op = rough(BASE_STEP_MM, vec![COARSE_STEP_MM]);
    let warnings = run_suggest(&mut op, &machine);
    eprintln!(
        "G-LADDER arm 3: shipped base {:?}, ladder {:?}, deepest {:?}; coarse \
         records {:?}; removal notes {:?}",
        op.depth_per_pass(),
        coarse_steps(&op),
        op.deepest_axial_step(),
        coarse_records(&warnings),
        removal_notes(&warnings)
    );
    assert!(
        !warnings
            .iter()
            .any(|w| matches!(w, SuggestWarning::EngagementReducedForAggressiveness { .. })),
        "the neutral dial must not act, else the base pin is not exact: {warnings:?}"
    );

    // Ruling 1: the funnel keeps the operator's base step. It ships at 3.0,
    // or lower only where a record states the cap that lowered it. Before
    // the ruling the funnel wrote the calculator's DOC (4.09 mm) here.
    let base = op.depth_per_pass().unwrap();
    assert!(
        base <= BASE_STEP_MM,
        "Suggest raised the operator's base step {BASE_STEP_MM} mm to {base} mm; \
         with a ladder it may only lower a step. Warnings {warnings:?}"
    );
    if base < BASE_STEP_MM {
        assert!(
            stated_caps(&warnings)
                .iter()
                .any(|c| (base - c).abs() < 1e-9),
            "the base step moved {BASE_STEP_MM} -> {base} mm and no record states \
             that cap: {warnings:?}"
        );
    } else {
        assert_eq!(base, BASE_STEP_MM, "the base step is the operator's value");
    }
    // The ladder ships non-empty, or its step went with a note.
    assert_no_silent_removal(&op, &warnings, 1, "arm 3");
    if coarse_steps(&op).is_empty() {
        assert_eq!(
            removal_notes(&warnings).len(),
            1,
            "an empty ladder must carry the note: {warnings:?}"
        );
    }

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

// ── Arm 5: a capped step that meets the base step ───────────────────

#[test]
fn a_capped_step_that_meets_the_base_is_removed_with_a_note() {
    let machine = MachineProfile::generic_wood_router();
    let material = hardwood();
    let tool = tool();
    let cap = envelope_cap_without_a_row();
    assert!(
        cap > 0.0 && cap < COARSE_STEP_MM,
        "fixture is vacuous: the envelope cap {cap} mm is not under the 20 mm \
         coarse step"
    );

    // The base step sits at the cap, so the capped 20 mm step lands on it.
    let mut op = rough(cap, vec![COARSE_STEP_MM]);
    let warnings = resolve_operation_invariants(
        &mut op,
        &tool,
        &machine,
        &material,
        PassRole::Roughing,
        SuggestContext::default(),
    );
    eprintln!(
        "G-LADDER arm 5: cap {cap:.4} mm; shipped base {:?}, ladder {:?}; notes {:?}",
        op.depth_per_pass(),
        coarse_steps(&op),
        removal_notes(&warnings)
    );

    assert!(
        coarse_steps(&op).is_empty(),
        "the 20 mm step capped onto the base step must go: {:?}",
        coarse_steps(&op)
    );
    let notes = removal_notes(&warnings);
    assert_eq!(
        notes.len(),
        1,
        "exactly one removal note for the one step: {warnings:?}"
    );
    let (step, lowered_to, next, next_is_base, why) = notes[0];
    assert_eq!(step, COARSE_STEP_MM, "the note names the step as written");
    assert!(
        (lowered_to - cap).abs() < 1e-9,
        "the note gives the cap {cap} mm as the new value, got {lowered_to} mm"
    );
    assert!(
        (next - cap).abs() < 1e-9,
        "the step it is no longer above is the base step at the cap, got {next} mm"
    );
    assert!(next_is_base, "the next step is Depth/Pass");
    assert_eq!(why, "axial envelope");
    // The clamp record of the step is still filed, before its note.
    let records = coarse_records(&warnings);
    assert_eq!(records.len(), 1, "{warnings:?}");
    assert_eq!(records[0].0, COARSE_STEP_MM);
    assert_no_silent_removal(&op, &warnings, 1, "arm 5");
    // The card text names the step, the cap and Depth/Pass.
    let text = rs_cam_core::feeds::suggest::coarse_step_removed_text(
        step,
        lowered_to,
        next,
        next_is_base,
        why,
    );
    assert!(text.contains("20.00"), "{text}");
    assert!(text.contains("axial envelope"), "{text}");
    assert!(text.contains("Depth/Pass"), "{text}");
}
