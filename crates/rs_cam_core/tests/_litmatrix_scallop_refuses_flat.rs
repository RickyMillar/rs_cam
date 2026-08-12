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

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::feeds::{
    FeedsError, FeedsInput, OperationFamily, PassRole, SetupContext, SpindleStrategy,
    ToolGeometryHint, embedded_vendor_lut, validate_tool_for_operation,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

fn base_input<'a>(
    machine: &'a MachineProfile,
    material: &'a Material,
    operation: OperationFamily,
    tool_geometry: ToolGeometryHint,
    target_scallop_mm: Option<f64>,
) -> FeedsInput<'a> {
    FeedsInput {
        tool_diameter: 6.0,
        flute_count: 2,
        flute_length: 22.0,
        shank_diameter: None,
        tool_geometry,
        material,
        machine,
        operation,
        operation_kind: None,
        pass_role: PassRole::Finish,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm,
        vendor_lut: Some(embedded_vendor_lut()),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MatchChart,
    }
}

#[test]
fn scallop_refuses_flat_endmill() {
    let machine = MachineProfile::shapeoko_vfd();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };
    let input = base_input(
        &machine,
        &material,
        OperationFamily::Scallop,
        ToolGeometryHint::Flat,
        Some(0.02),
    );
    let err = validate_tool_for_operation(&input).expect_err(
        "Scallop + Flat should be refused — no tip radius means scallop math is undefined",
    );
    let msg = format!("{err}");
    assert!(
        msg.contains("scallop") && msg.contains("curved"),
        "refusal message {msg:?} should mention scallop + curved tip",
    );
    assert!(matches!(
        err,
        FeedsError::WrongToolForOperation {
            operation: OperationFamily::Scallop,
            actual_geometry: ToolGeometryHint::Flat,
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
    let input = base_input(
        &machine,
        &material,
        OperationFamily::Scallop,
        ToolGeometryHint::VBit {
            included_angle: 60.0,
            tip_diameter: 0.1,
        },
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
    let input = base_input(
        &machine,
        &material,
        OperationFamily::Scallop,
        ToolGeometryHint::Ball,
        Some(0.02),
    );
    assert!(
        validate_tool_for_operation(&input).is_ok(),
        "Scallop + Ball must be accepted — this is the canonical scallop tool",
    );
}

#[test]
fn scallop_accepts_bull_endmill() {
    let machine = MachineProfile::shapeoko_vfd();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };
    let input = base_input(
        &machine,
        &material,
        OperationFamily::Scallop,
        ToolGeometryHint::Bull { corner_radius: 1.0 },
        Some(0.02),
    );
    assert!(validate_tool_for_operation(&input).is_ok());
}

#[test]
fn scallop_accepts_tapered_ball() {
    let machine = MachineProfile::shapeoko_vfd();
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };
    let input = base_input(
        &machine,
        &material,
        OperationFamily::Scallop,
        ToolGeometryHint::TaperedBall {
            tip_radius: 0.5,
            taper_angle_deg: 10.0,
        },
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
    let input = base_input(
        &machine,
        &material,
        OperationFamily::Parallel,
        ToolGeometryHint::Flat,
        Some(0.02),
    );
    assert!(matches!(
        validate_tool_for_operation(&input),
        Err(FeedsError::WrongToolForOperation { .. })
    ));
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
    let input = base_input(
        &machine,
        &material,
        OperationFamily::Parallel,
        ToolGeometryHint::Flat,
        None,
    );
    assert!(validate_tool_for_operation(&input).is_ok());
}
