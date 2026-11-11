//! CONTRACT K2 (feeds contract P5): Suggest never raises a depth that the
//! operator states.
//!
//! The case is op 1 of the blind review (report 01 (a),
//! `planning/feeds_blind_review_2026-11-03/rootcause/01_op1_op14_bulk_18mm.md`):
//! a 3D Rough with a stated depth per pass of 9.0 mm, an 18 mm 2-flute
//! bit with a 20 mm flute, White Oak, the Shapeoko VFD. Before P5 the add
//! door and "Apply all" wrote 14.047 mm: the rigidity floor (the machine
//! factor 2.0, deleted in P29-4) gave 36 mm, the flute guard cut it to 16 mm
//! and the dial to 14.047 mm.
//!
//! 1. An apply with no pick writes the speeds only: the depth stays 9.0.
//! 2. An apply of the proposal writes a depth at or below 9.0: the
//!    proposal is `min(stated, binding cap)`.
//!
//! Red before P5 (shown in the P5 report).

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::ToolFact;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::envelope::PickRequest;
use rs_cam_core::feeds::suggest::{OperationApplyInput, SuggestContext, apply_to_operation};
use rs_cam_core::feeds::{FeedsProvenance, SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// The operator's depth per pass of op 1 (mm).
const STATED_DPP_MM: f64 = 9.0;

fn bulk_bit() -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 18.0;
    tool.set_flute_count(ToolFact::Stated(2));
    tool.set_cutting_length(ToolFact::Stated(20.0));
    tool.set_stickout(ToolFact::Stated(25.0));
    tool
}

fn rough_3d() -> OperationConfig {
    let mut op = rs_cam_core::compute::fixture_values::fixture_op(OperationType::Adaptive3d);
    assert!(
        op.set_depth_per_pass(STATED_DPP_MM),
        "a 3D Rough holds a depth per pass"
    );
    op
}

fn apply(pick: Option<PickRequest>) -> OperationConfig {
    let tool = bulk_bit();
    let machine = MachineProfile::shapeoko_vfd();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };
    let mut op = rough_3d();
    let mut prov = FeedsProvenance::default();
    apply_to_operation(
        OperationApplyInput {
            tool: &tool,
            machine: &machine,
            material: &material,
            spindle_strategy: SpindleStrategy::default(),
            lut: embedded_vendor_lut(),
            suggest: SuggestContext::default(),
        },
        pick,
        None,
        &mut op,
        &mut prov,
    )
    .expect("an 18 mm end mill on a 3D Rough in oak is backed");
    op
}

#[test]
fn an_apply_of_the_speeds_keeps_the_stated_depth_k2() {
    let op = apply(None);
    assert_eq!(
        op.depth_per_pass(),
        Some(STATED_DPP_MM),
        "the speeds apply moved the stated depth"
    );
}

#[test]
fn an_apply_of_the_proposal_never_raises_the_stated_depth_k2() {
    let op = apply(Some(PickRequest::Proposal));
    let dpp = op
        .depth_per_pass()
        .expect("a 3D Rough holds a depth per pass");
    assert!(
        dpp <= STATED_DPP_MM,
        "the proposal raised the stated depth: {STATED_DPP_MM} -> {dpp}"
    );
}
