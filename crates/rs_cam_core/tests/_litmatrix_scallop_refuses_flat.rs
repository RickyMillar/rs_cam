//! Literature-matrix regression — Scallop refuses non-curved tools.
//!
//! Cell: `flat_6mm_scallop_oak_unusable`. Pre-fix, the scallop-stepover
//! block at `feeds/mod.rs:606-622` silently skipped its computation when
//! `ball_r == 0.0` (i.e. `ToolGeometryHint::Flat` or `VBit`), then fell
//! through to `operation_default_profile(OperationFamily::Scallop, …)`
//! and produced a perfectly normal feed / RPM / DOC / WOC tuple for a
//! tool whose tip cannot make a scallop. `calculate()` was infallible,
//! so there was no way for the layer above to detect the refusal — the
//! GUI Suggest button would happily push the meaningless recipe into
//! the operation.
//!
//! Fix: `feeds::validate_tool_for_operation` returns
//! `Err(FeedsError::WrongToolForOperation)` for Scallop + Flat|VBit (and
//! the Parallel + scallop-target combination DropCutter routes through),
//! and `suggest::feeds_result_for_operation` / `suggest_for_operation`
//! / `suggest_params` now return `Result`, propagating the refusal to
//! GUI / CLI / MCP. The shim translates the error into
//! `ShimError::EngineRefused`, flipping the matrix's
//! `(unusable, Err)` branch to Within.
//!
//! This sentry asserts the predicate directly so a future refactor
//! that drops the validation call can't silently regress the matrix
//! cell.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::ToolFact;
use rs_cam_core::compute::{ToolConfig, ToolId};
use rs_cam_core::feeds::{
    CutterKind, FeedsError, FeedsInput, OperationFamily, SpindleStrategy, ToolGeometryHint,
    embedded_vendor_lut, validate_tool_for_operation,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// P29-7 7a: the input of the default op of `kind` at the scallop target
/// `target_scallop_mm` (a Scallop, or a drop cutter on its scallop
/// target; `None` keeps the op's own), on `tool`.
fn base_input<'a>(
    machine: &'a MachineProfile,
    material: &'a Material,
    kind: OperationType,
    tool: &'a ToolConfig,
    target_scallop_mm: Option<f64>,
) -> FeedsInput<'a> {
    let mut op = OperationConfig::new_default(kind);
    if let Some(h) = target_scallop_mm {
        op.set_scallop_height(h);
    }
    FeedsInput::of_operation(
        &op,
        tool,
        material,
        machine,
        embedded_vendor_lut(),
        SpindleStrategy::MatchChart,
    )
}

/// A 2-flute tool of the shape `hint` (6 mm, or the tip of a tapered
/// ball), each fact stated.
fn tool_of(hint: ToolGeometryHint) -> ToolConfig {
    let d = match hint {
        ToolGeometryHint::TaperedBall { tip_radius, .. } => 2.0 * tip_radius,
        ToolGeometryHint::Flat
        | ToolGeometryHint::Ball
        | ToolGeometryHint::Bull { .. }
        | ToolGeometryHint::VBit { .. } => 6.0,
    };
    let builder = ToolConfig::builder(ToolId::from_raw(1), hint.cutter_kind().tool_type())
        .diameter(d)
        .flutes(2)
        .flute_mm(22.0)
        .shank_mm(6.0)
        .stickout_mm(24.0);
    let mut tool = match hint {
        ToolGeometryHint::Flat | ToolGeometryHint::Ball => builder.build(),
        ToolGeometryHint::Bull { corner_radius } => builder.corner_radius(corner_radius).build(),
        ToolGeometryHint::VBit { included_angle, .. } => {
            builder.included_angle_deg(included_angle).build()
        }
        ToolGeometryHint::TaperedBall {
            taper_angle_deg, ..
        } => builder
            .taper_half_angle_deg(taper_angle_deg)
            .shaft_mm(6.0)
            .build(),
    };
    if let ToolGeometryHint::VBit { tip_diameter, .. } = hint {
        tool.set_v_tip_diameter(ToolFact::Stated(tip_diameter));
    }
    assert_eq!(tool.geometry_hint(), hint, "the tool states the hint");
    tool
}

#[test]
fn scallop_refuses_flat_endmill() {
    let machine = MachineProfile::shapeoko_vfd();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };
    let tool = tool_of(ToolGeometryHint::Flat);
    let input = base_input(
        &machine,
        &material,
        OperationType::Scallop,
        &tool,
        Some(0.02),
    );
    // P29-7 7a: the Scallop op's own tool rule (the registry) refuses a
    // flat end mill first, so its message names the op and the tools it
    // takes. The scallop-height message is held on the drop cutter below,
    // where the scallop rule is the rule that refuses.
    let err = validate_tool_for_operation(&input).expect_err(
        "Scallop + Flat should be refused — no tip radius means scallop math is undefined",
    );
    let msg = format!("{err}");
    assert!(
        !msg.contains('{'),
        "refusal message {msg:?} leaks Debug output"
    );
    assert!(matches!(
        err,
        FeedsError::WrongToolForOperation {
            family: OperationFamily::Scallop,
            actual: CutterKind::Flat,
            ..
        }
    ));
}

#[test]
fn scallop_refuses_vbit() {
    let machine = MachineProfile::shapeoko_vfd();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };
    let tool = tool_of(ToolGeometryHint::VBit {
        included_angle: 60.0,
        tip_diameter: 0.1,
    });
    let input = base_input(
        &machine,
        &material,
        OperationType::Scallop,
        &tool,
        Some(0.02),
    );
    assert!(matches!(
        validate_tool_for_operation(&input),
        Err(FeedsError::WrongToolForOperation { .. })
    ));
}

#[test]
fn scallop_accepts_ball_endmill() {
    let machine = MachineProfile::shapeoko_vfd();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };
    let tool = tool_of(ToolGeometryHint::Ball);
    let input = base_input(
        &machine,
        &material,
        OperationType::Scallop,
        &tool,
        Some(0.02),
    );
    assert!(
        validate_tool_for_operation(&input).is_ok(),
        "Scallop + Ball must be accepted — this is the canonical scallop tool",
    );
}

#[test]
fn scallop_accepts_tapered_ball() {
    let machine = MachineProfile::shapeoko_vfd();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };
    let tool = tool_of(ToolGeometryHint::TaperedBall {
        tip_radius: 0.5,
        taper_angle_deg: 10.0,
    });
    let input = base_input(
        &machine,
        &material,
        OperationType::Scallop,
        &tool,
        Some(0.02),
    );
    assert!(validate_tool_for_operation(&input).is_ok());
}

#[test]
fn dropcutter_with_scallop_target_refuses_flat() {
    // DropCutter routes through OperationFamily::Parallel and exposes
    // the scallop-driven stepover via `target_scallop_mm`. When that
    // hint is set the same geometry constraint applies — a flat tool
    // can't satisfy the chord-height formula.
    let machine = MachineProfile::shapeoko_vfd();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };
    let tool = tool_of(ToolGeometryHint::Flat);
    let input = base_input(
        &machine,
        &material,
        OperationType::DropCutter,
        &tool,
        Some(0.02),
    );
    let err = validate_tool_for_operation(&input)
        .expect_err("a drop cutter on a scallop target refuses a flat end mill");
    let msg = format!("{err}");
    assert!(
        msg.contains("scallop height") && msg.contains("curved tip"),
        "refusal message {msg:?} should name the scallop height and the curved tip",
    );
    assert!(matches!(err, FeedsError::WrongToolForOperation { .. }));
}

#[test]
fn parallel_without_scallop_target_accepts_flat() {
    // Plain DropCutter / parallel raster *without* a scallop hint is
    // perfectly fine with a flat endmill — that's the legacy
    // ae_factor stepover path. The refusal must NOT fire here.
    let machine = MachineProfile::shapeoko_vfd();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };
    let tool = tool_of(ToolGeometryHint::Flat);
    let input = base_input(&machine, &material, OperationType::DropCutter, &tool, None);
    assert!(validate_tool_for_operation(&input).is_ok());
}
