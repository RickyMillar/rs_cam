//! CONTRACT K2b (feeds contract P5, ruling Q4): Suggest never writes
//! geometry by itself.
//!
//! An apply with no pick writes the speeds (feed, plunge, RPM, ramp feed)
//! for the geometry that the op keeps. This sentry walks
//! `OperationType::ALL`: after an apply with no pick, every field of the op
//! except the four speed fields is equal to the field before the apply.
//!
//! Report 01 (e) of the blind review: a Face with a stated stepover of
//! 14.4 mm on the 18 mm bit. Before P5 "Apply all" wrote the calculator's
//! width over it. Now the stepover stays.
//!
//! Red before P5 (shown in the P5 report).

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::{
    OperationApplyInput, StockContext, SuggestContext, apply_to_operation,
};
use rs_cam_core::feeds::{FeedsProvenance, SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// The fields that a speeds apply writes. Every other field must stay.
const SPEED_KEYS: [&str; 4] = ["feed_rate", "plunge_rate", "spindle_rpm", "ramp_feed_rate"];

fn tool(kind: ToolType) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId::from_raw(0), kind);
    t.diameter = 6.0;
    if matches!(kind, ToolType::VBit) {
        t.included_angle = 60.0;
    }
    t
}

fn stock() -> StockContext {
    StockContext {
        stock_top_z: 0.0,
        stock_bottom_z: -18.0,
        stock_z: Some(18.0),
        stock_padding: 2.0,
    }
}

/// The op's JSON with the speed fields taken out.
fn geometry_of(op: &OperationConfig) -> serde_json::Value {
    let mut v = serde_json::to_value(op).expect("an op serialises");
    if let Some(params) = v
        .get_mut("params")
        .and_then(serde_json::Value::as_object_mut)
    {
        for key in SPEED_KEYS {
            params.remove(key);
        }
    }
    v
}

/// Apply with no pick; `None` when the tool is refused for the op.
fn apply_speeds(op: &OperationConfig, tool: &ToolConfig) -> Option<OperationConfig> {
    let machine = MachineProfile::default();
    let material = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    let stock = stock();
    let mut out = op.clone();
    let mut prov = FeedsProvenance::default();
    apply_to_operation(
        OperationApplyInput {
            tool,
            machine: &machine,
            material: &material,
            spindle_strategy: SpindleStrategy::default(),
            lut: embedded_vendor_lut(),
            suggest: SuggestContext {
                stock: Some(&stock),
                ..SuggestContext::default()
            },
        },
        None,
        None,
        &mut out,
        &mut prov,
    )
    .ok()?;
    Some(out)
}

#[test]
fn an_apply_with_no_pick_leaves_every_geometry_field_k2b() {
    let mut applied = 0_usize;
    for &op_type in OperationType::ALL {
        let op = rs_cam_core::compute::fixture_values::fixture_op(op_type);
        let Some((kind, out)) = ToolType::ALL
            .iter()
            .find_map(|&k| apply_speeds(&op, &tool(k)).map(|out| (k, out)))
        else {
            panic!("{op_type:?}: no tool kind is backed; the walk needs one");
        };
        assert_eq!(
            geometry_of(&out),
            geometry_of(&op),
            "{op_type:?} with {kind:?}: an apply with no pick moved a geometry field"
        );
        applied += 1;
    }
    assert_eq!(
        applied,
        OperationType::ALL.len(),
        "the walk covers every op"
    );
}

#[test]
fn a_stated_face_stepover_stays_after_an_apply_k2b() {
    let mut op = rs_cam_core::compute::fixture_values::fixture_op(OperationType::Face);
    assert!(op.set_stepover(14.4), "a Face holds a stepover");
    let mut bit = tool(ToolType::EndMill);
    bit.diameter = 18.0;
    bit.cutting_length = 20.0;
    bit.stickout = 25.0;
    let out = apply_speeds(&op, &bit).expect("an 18 mm end mill on a Face is backed");
    assert_eq!(
        out.stepover(),
        Some(14.4),
        "the apply moved the stated Face stepover"
    );
}
