//! Feeds contract P29-7 7a: the op and the tool of a calculator input.
//!
//! `FeedsInput::of_operation` is the one constructor of a calculator input,
//! so a test states the op and the tool that give the input. These helpers
//! map the old literal fields to them. Include the file with
//! `#[path = "common/feeds_input.rs"] mod feeds_input;`.

#![allow(
    dead_code,
    reason = "each test binary that includes this file uses a subset of the helpers"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::ToolFact;
use rs_cam_core::compute::{ToolConfig, ToolId};
use rs_cam_core::feeds::{OperationFamily, PassRole, ToolGeometryHint};

/// The op that gives the vendor row key (`family`, `role`) with no slot
/// case and no planner width, or `None` when no op gives the key. The
/// choice per key:
///
/// - (Pocket, Roughing): a stock Face (a Pocket declares a slot case).
/// - (Adaptive, Roughing): a 2D Adaptive (it holds the pass ceiling; no op
///   of the key holds no planner width).
/// - (Contour, Roughing): a Profile (a slot); (Contour, SemiFinish): a
///   Waterline; (Contour, Finish): a Steep/Shallow.
/// - (Parallel, Finish): a drop cutter; (Scallop, Finish): a spiral
///   finish; (Trace, Finish): a Pencil (no slot, no stated cut).
/// - (Drill, Roughing): a Drill.
#[must_use]
pub fn op_of_row_key(family: OperationFamily, role: PassRole) -> Option<OperationType> {
    match (family, role) {
        (OperationFamily::Pocket, PassRole::Roughing) => Some(OperationType::Face),
        (OperationFamily::Adaptive, PassRole::Roughing) => Some(OperationType::Adaptive),
        (OperationFamily::Contour, PassRole::Roughing) => Some(OperationType::Profile),
        (OperationFamily::Contour, PassRole::SemiFinish) => Some(OperationType::Waterline),
        (OperationFamily::Contour, PassRole::Finish) => Some(OperationType::SteepShallow),
        (OperationFamily::Parallel, PassRole::Finish) => Some(OperationType::DropCutter),
        (OperationFamily::Scallop, PassRole::Finish) => Some(OperationType::SpiralFinish),
        (OperationFamily::Trace, PassRole::Finish) => Some(OperationType::Pencil),
        (OperationFamily::Drill, PassRole::Roughing) => Some(OperationType::Drill),
        (
            OperationFamily::Pocket | OperationFamily::Adaptive | OperationFamily::Drill,
            PassRole::SemiFinish | PassRole::Finish,
        )
        | (
            OperationFamily::Parallel | OperationFamily::Scallop | OperationFamily::Trace,
            PassRole::Roughing | PassRole::SemiFinish,
        )
        | (OperationFamily::Face, PassRole::Roughing | PassRole::SemiFinish | PassRole::Finish) => {
            None
        }
    }
}

/// [`op_of_row_key`], but the op holds a stated depth: (Parallel, Finish)
/// gives a Ramp Finish (`max_stepdown`), (Scallop, Finish) a Unified
/// Finish (`z_step`, and its own scallop target), (Trace, Finish) an
/// InlayPlug (`depth_per_pass`, no slot). Write the depth with
/// [`op_at_depth`].
#[must_use]
pub fn op_of_row_key_with_depth(family: OperationFamily, role: PassRole) -> Option<OperationType> {
    match (family, role) {
        (OperationFamily::Parallel, PassRole::Finish) => Some(OperationType::RampFinish),
        (OperationFamily::Scallop, PassRole::Finish) => Some(OperationType::UnifiedFinish),
        (OperationFamily::Trace, PassRole::Finish) => Some(OperationType::InlayPlug),
        (
            OperationFamily::Pocket
            | OperationFamily::Adaptive
            | OperationFamily::Contour
            | OperationFamily::Parallel
            | OperationFamily::Scallop
            | OperationFamily::Trace
            | OperationFamily::Face
            | OperationFamily::Drill,
            PassRole::Roughing | PassRole::SemiFinish | PassRole::Finish,
        ) => op_of_row_key(family, role),
    }
}

/// The default op of `kind` at the stated depth `ap`: the depth per pass,
/// or the Z step of a Steep/Shallow or a Unified Finish. An op that holds
/// no side-cut depth panics.
#[must_use]
pub fn op_at_depth(kind: OperationType, ap: f64) -> OperationConfig {
    let mut op = OperationConfig::new_default(kind);
    if let OperationConfig::SteepShallow(c) = &mut op {
        c.z_step = Some(ap);
    } else if let OperationConfig::UnifiedFinish(c) = &mut op {
        c.z_step = Some(ap);
    } else {
        assert!(op.set_depth_per_pass(ap), "{kind:?} holds no depth");
    }
    op
}

/// The default op of `kind` with the stated depth per pass `ap` and the
/// stated stepover `ae`. An op that holds no such field panics: a test
/// states only what an op gives.
#[must_use]
pub fn op_with(kind: OperationType, ap: Option<f64>, ae: Option<f64>) -> OperationConfig {
    let mut op = OperationConfig::new_default(kind);
    if let Some(ap) = ap {
        assert!(
            op.set_depth_per_pass(ap),
            "{kind:?} holds no depth per pass"
        );
    }
    if let Some(ae) = ae {
        assert!(op.set_stepover(ae), "{kind:?} holds no stepover");
    }
    op
}

/// The tool of the shape `hint`: the diameter `d` (the tip of a tapered
/// ball), `flutes`, the flute length `flute_mm` and the shank `shank_mm`
/// (the shaft of a tapered ball too), each fact stated. The stick-out is
/// 4 x D: at that ratio the long-tool share is 1, as for an input with no
/// overhang.
#[must_use]
pub fn tool_of_hint(
    hint: ToolGeometryHint,
    d: f64,
    flutes: u32,
    flute_mm: f64,
    shank_mm: f64,
) -> ToolConfig {
    let builder = ToolConfig::builder(ToolId::from_raw(1), hint.cutter_kind().tool_type())
        .diameter(d)
        .flutes(flutes)
        .flute_mm(flute_mm)
        .shank_mm(shank_mm)
        .stickout_mm(4.0 * d);
    let mut tool = match hint {
        ToolGeometryHint::Flat | ToolGeometryHint::Ball => builder.build(),
        ToolGeometryHint::Bull { corner_radius } => builder.corner_radius(corner_radius).build(),
        ToolGeometryHint::VBit { included_angle, .. } => {
            builder.included_angle_deg(included_angle).build()
        }
        ToolGeometryHint::TaperedBall {
            tip_radius,
            taper_angle_deg,
        } => {
            assert!(
                (2.0 * tip_radius - d).abs() < 1e-12,
                "the diameter of a tapered ball is its tip"
            );
            builder
                .taper_half_angle_deg(taper_angle_deg)
                .shaft_mm(shank_mm)
                .build()
        }
    };
    if let ToolGeometryHint::VBit { tip_diameter, .. } = hint {
        tool.set_v_tip_diameter(ToolFact::Stated(tip_diameter));
    }
    assert_eq!(tool.geometry_hint(), hint, "the tool states the hint");
    tool
}
