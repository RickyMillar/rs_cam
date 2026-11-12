//! Feeds contract P16, items 2 and 6c (review D7, lead ruling B; review D5,
//! operator ruling): the envelope proposes no depth that it cannot model.
//!
//! Item 2: the Project Curve `depth` is a signed offset from the model
//! surface, not a depth into the stock. The claims:
//!
//! 1. The cut spec states `Axial::SurfaceOffset` and a total depth of
//!    `None` with its reason; the registry row states
//!    `AxialRule::SurfaceOffset`.
//! 2. Suggest ships the feed from the chip, with ONE
//!    `DepthJudgedAfterSimulation` warning, no flute-guard warning, and a
//!    power of 0 (no power figure).
//! 3. The shipped power door refuses with `DepthAfterSimulation`, and the
//!    deflection predictor too. The `shipped_power` JSON is `unmodeled`.
//!    Item 2b: the force door and the removal rate refuse with the same
//!    reason (no answer at D x D), and the MRR JSON is `unmodeled`.
//! 4. The envelope proposes no depth (`AfterSimulation`), and a proposal
//!    pick writes no depth.
//!
//! Item 6c: a material with no force line (Baltic birch plywood) gets no
//! depth proposal on a Pocket, and an "Apply all" keeps the stated depth.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test no_depth_is_proposed_that_cannot_be_modelled_p16
//! ```

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{
    Axial, AxialRule, DEPTH_AFTER_SIMULATION, OperationConfig, OperationType, TotalDepth,
};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::envelope::{
    EnvelopeInput, NoDepthProposal, PickRequest, ProposedDepth, SafeEnvelope,
};
use rs_cam_core::feeds::predict::DeflectionUnmodeled;
use rs_cam_core::feeds::suggest::{
    OperationApplyInput, SuggestContext, SuggestForOperationInput, SuggestedParams,
    apply_to_operation, suggest_for_operation,
};
use rs_cam_core::feeds::{
    FeedsProvenance, FeedsWarning, PowerUnmodeled, SpindleStrategy, embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, PlywoodGrade, WoodSpecies};

/// A 6 mm 2F flat end mill.
fn tool() -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    t.diameter = 6.0;
    t.set_flute_count(ToolFact::Stated(2));
    t.set_cutting_length(ToolFact::Stated(24.0));
    t.set_shank_diameter(ToolFact::Stated(6.0));
    t.set_stickout(ToolFact::Stated(32.0));
    t
}

fn oak() -> Material {
    Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }
}

fn birch() -> Material {
    Material::Plywood {
        grade: PlywoodGrade::BalticBirch,
    }
}

/// A Project Curve 1 mm into the surface.
fn project_curve() -> OperationConfig {
    let mut op = OperationConfig::new_default(OperationType::ProjectCurve);
    if let OperationConfig::ProjectCurve(cfg) = &mut op {
        cfg.depth = Some(1.0);
    }
    op
}

fn suggested(op: &OperationConfig, material: &Material) -> SuggestedParams {
    suggest_for_operation(SuggestForOperationInput {
        operation: op,
        tool: &tool(),
        machine: &MachineProfile::generic_wood_router(),
        material,
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::MatchChart,
        context: SuggestContext::default(),
    })
    .unwrap_or_else(|e| panic!("Suggest serves {:?}: {e}", op.op_type()))
}

fn envelope(op: &OperationConfig, material: &Material) -> SafeEnvelope {
    SafeEnvelope::for_cut(EnvelopeInput {
        operation: op,
        tool: &tool(),
        machine: &MachineProfile::generic_wood_router(),
        material,
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::MatchChart,
        context: SuggestContext::default(),
    })
    .expect("an envelope")
}

#[test]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: an unexpected variant panics"
)]
fn the_project_curve_cut_states_an_offset_not_a_depth_p16() {
    let spec = project_curve().cut_spec();
    match spec.axial {
        Axial::SurfaceOffset(Some(offset)) => assert_eq!(offset.mm(), 1.0),
        _ => panic!("the offset is not a depth: {:?}", spec.axial),
    }
    assert_eq!(spec.total, TotalDepth::None(DEPTH_AFTER_SIMULATION));
    assert_eq!(
        OperationType::ProjectCurve.capabilities().cut_rule.axial,
        AxialRule::SurfaceOffset
    );
    // P29-9 c8: the calculator reads its depth at the one depth door.
    assert_eq!(
        rs_cam_core::feeds::planned::body_depth_of(
            &project_curve(),
            tool().geometry_hint(),
            rs_cam_core::feeds::planned::PlanFacts::NONE,
        )
        .speeds_mm(),
        None,
        "the calculator reads no depth"
    );
}

#[test]
fn suggest_ships_the_chip_and_one_warning_for_the_depth_p16() {
    let s = suggested(&project_curve(), &oak());
    let r = &s.feeds_result;
    let after_sim = r
        .warnings
        .iter()
        .filter(|w| matches!(w, FeedsWarning::DepthJudgedAfterSimulation))
        .count();
    assert_eq!(after_sim, 1, "one warning: {:?}", r.warnings);
    assert!(
        !r.warnings.iter().any(|w| matches!(
            w,
            FeedsWarning::DocExceedsFlute { .. }
                | FeedsWarning::PowerLimited { .. }
                | FeedsWarning::PowerLadderReducedCut { .. }
        )),
        "no depth to judge: {:?}",
        r.warnings
    );
    assert_eq!(
        r.power_kw, None,
        "no power figure (P20d: not modelled is None, never 0.0)"
    );
    let feed = s.operation.feed_rate().expect("the feed ships");
    assert!(feed > 0.0, "the feed ships from the chip");

    let power = rs_cam_core::feeds::power_at_shipped_point(
        &s.operation,
        &tool(),
        &oak(),
        &MachineProfile::generic_wood_router(),
        r,
        None,
    );
    assert_eq!(power.err(), Some(PowerUnmodeled::DepthAfterSimulation));
    let json = rs_cam_core::feeds::operating_point::shipped_power_json(
        &rs_cam_core::feeds::power_at_shipped_point(
            &s.operation,
            &tool(),
            &oak(),
            &MachineProfile::generic_wood_router(),
            r,
            None,
        ),
    );
    assert!(json.get("unmodeled").is_some(), "{json}");
    let deflection = rs_cam_core::feeds::predict::predict_peak_deflection_um(
        &s.operation,
        &tool(),
        &oak(),
        &MachineProfile::generic_wood_router(),
    );
    assert_eq!(
        deflection.err(),
        Some(DeflectionUnmodeled::DepthAfterSimulation)
    );
    // Item 2b: no force and no removal rate at the fallback depth D.
    let force = rs_cam_core::feeds::operating_point::force_at_shipped_point(
        &s.operation,
        &tool(),
        &oak(),
        &MachineProfile::generic_wood_router(),
        r,
        None,
    );
    assert_eq!(force.err(), Some(PowerUnmodeled::DepthAfterSimulation));
    assert_eq!(
        r.mrr_mm3_min,
        Err(PowerUnmodeled::DepthAfterSimulation),
        "no removal rate at D x D"
    );
    let mrr = rs_cam_core::feeds::operating_point::mrr_json(&r.mrr_mm3_min);
    assert!(mrr.get("unmodeled").is_some(), "{mrr}");
    // Non-vacuity: a Pocket on the same tool and wood has both.
    let pocket = suggested(&OperationConfig::new_default(OperationType::Pocket), &oak());
    assert!(pocket.feeds_result.mrr_mm3_min.is_ok_and(|v| v > 0.0));
}

#[test]
fn the_envelope_proposes_no_project_curve_depth_p16() {
    let op = project_curve();
    let env = envelope(&op, &oak());
    assert_eq!(
        env.proposal.get().depth,
        ProposedDepth::None(NoDepthProposal::AfterSimulation)
    );
    assert!(
        env.notes
            .iter()
            .any(|n| n == NoDepthProposal::AfterSimulation.clause()),
        "the note: {:?}",
        env.notes
    );
    assert!(!env.pick(PickRequest::Proposal).writes_depth());
}

#[test]
fn a_material_with_no_force_line_gets_no_depth_proposal_p16() {
    let mut op = OperationConfig::new_default(OperationType::Pocket);
    // 20 mm is above the flute guard (0.8 x 24 mm), so a depth proposal
    // would lower it (non-vacuity of the apply claim).
    assert!(op.set_depth_per_pass(20.0), "fixture: a Pocket depth");
    assert!(op.set_stepover(2.0), "fixture: a Pocket stepover");
    let env = envelope(&op, &birch());
    let p = env.proposal.get();
    assert_eq!(p.no_depth(), Some(NoDepthProposal::NoForceLine), "{p:?}");
    assert!(p.ae_mm <= 2.0 + 1e-12, "no raise of the width (K2)");
    // A solid wood with a force line keeps its depth proposal.
    let oak_env = envelope(&op, &oak());
    let oak_ap = oak_env
        .proposal
        .get()
        .ap_mm()
        .expect("oak has a depth proposal");
    assert!(oak_ap < 20.0, "non-vacuity: a depth proposal lowers 20 mm");

    // "Apply all" keeps the stated depth.
    let mut applied = op.clone();
    let mut provenance = FeedsProvenance::default();
    let _ = apply_to_operation(
        OperationApplyInput {
            tool: &tool(),
            machine: &MachineProfile::generic_wood_router(),
            material: &birch(),
            spindle_strategy: SpindleStrategy::MatchChart,
            lut: embedded_vendor_lut(),
            suggest: SuggestContext::default(),
        },
        Some(PickRequest::Proposal),
        None,
        &mut applied,
        &mut provenance,
    )
    .expect("the plywood pocket is backed");
    assert_eq!(
        applied.depth_per_pass(),
        Some(20.0),
        "the apply writes no depth"
    );
}
