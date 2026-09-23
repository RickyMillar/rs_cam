//! FM0 — every feeds cell declares its support arm.
//!
//! Programme: feeds matrix 2026-09-23, Phase 0
//! (`planning/feeds_matrix_2026-09-23/PLAN.md` §5).
//!
//! A cell is one operation, one tool family and one material family.
//! `feeds::feeds_support` joins two halves:
//!
//! - the static half, `OperationSpec::feeds_formula_source` on each
//!   registry row;
//! - the dynamic half, the recipe row lookup that `feeds::calculate` does.
//!
//! Phase 0 is behaviour-preserving. Every registry row declares a formula
//! source, so no cell that shipped a recipe before FM0 refuses after it.
//!
//! ## The arms
//!
//! - [`every_operation_declares_its_formula_source`] (a) — an exhaustive
//!   `match` with no wildcard. A new operation cannot join without a
//!   decision.
//! - [`a_cell_with_a_vendor_row_is_vendor_backed`] (b) — over
//!   `OperationType::ALL × ToolType::ALL × four wood families`, the
//!   resolver does not panic, it agrees with the arm `calculate` records,
//!   and a cell whose lookup found a row is `VendorBacked`.
//! - [`no_cell_refuses_today`] (c) — the set of `Refuse` cells is empty.
//! - [`the_unbacked_refusal_names_its_cell`] (d) — the `Display` text of
//!   `FeedsError::Unbacked` names the operation, the tool family and the
//!   reason.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::feeds_input_for_operation;
use rs_cam_core::feeds::support::{DRILL_FORMULA_SOURCE, MILLING_FORMULA_SOURCE, NO_BASIS_REASON};
use rs_cam_core::feeds::vendor_lut::{MaterialFamily, ToolFamily};
use rs_cam_core::feeds::{
    FeedsError, FeedsSupport, SpindleStrategy, WorkholdingRigidity, calculate, embedded_vendor_lut,
    feeds_support, validate_tool_for_operation,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, PlywoodGrade, SheetGoodKind, WoodSpecies};

/// The four wood material families of the matrix, one material each.
fn wood_materials() -> [(&'static str, Material); 4] {
    [
        (
            "hardwood",
            Material::SolidWood {
                species: WoodSpecies::GenericHardwood,
            },
        ),
        (
            "softwood",
            Material::SolidWood {
                species: WoodSpecies::GenericSoftwood,
            },
        ),
        (
            "mdf",
            Material::SheetGood {
                kind: SheetGoodKind::Mdf,
            },
        ),
        (
            "plywood_hardwood",
            Material::Plywood {
                grade: PlywoodGrade::BalticBirch,
            },
        ),
    ]
}

/// The tool at its default geometry. `ToolConfig::new_default` gives each
/// type a diameter that the LUT carries: Ø6.35 end mill and ball nose,
/// Ø12.7 bull nose and V-bit, Ø3.175 tapered ball nose.
fn tool(tool_type: ToolType) -> ToolConfig {
    ToolConfig::new_default(ToolId(0), tool_type)
}

/// One resolved cell.
struct CellOutcome {
    label: String,
    support: FeedsSupport,
    recorded: FeedsSupport,
    has_row: bool,
    declared: Option<&'static str>,
    validation: Result<(), FeedsError>,
}

/// Resolve every cell through the production input builder.
fn every_cell() -> Vec<CellOutcome> {
    let machine = MachineProfile::generic_wood_router();
    let lut = embedded_vendor_lut();
    let mut out = Vec::new();
    for &op in OperationType::ALL {
        let operation = OperationConfig::new_default(op);
        for &tool_type in ToolType::ALL {
            let tool = tool(tool_type);
            for (material_name, material) in &wood_materials() {
                let input = feeds_input_for_operation(
                    &operation,
                    &tool,
                    material,
                    &machine,
                    WorkholdingRigidity::Medium,
                    lut,
                    SpindleStrategy::MatchChart,
                );
                let result = calculate(&input);
                out.push(CellOutcome {
                    label: format!("{op:?} × {tool_type:?} × {material_name}"),
                    support: feeds_support(&input),
                    recorded: result.support,
                    has_row: result.matched_lut_row.is_some(),
                    declared: op.spec().feeds_formula_source,
                    validation: validate_tool_for_operation(&input),
                });
            }
        }
    }
    out
}

/// (a) Each operation's formula-source decision, written as an exhaustive
/// `match`. A new `OperationType` variant does not compile here until it
/// has a decision.
#[test]
fn every_operation_declares_its_formula_source() {
    for &op in OperationType::ALL {
        let expected: Option<&str> = match op {
            OperationType::Drill | OperationType::AlignmentPinDrill => Some(DRILL_FORMULA_SOURCE),
            OperationType::Face
            | OperationType::Pocket
            | OperationType::Profile
            | OperationType::Adaptive
            | OperationType::VCarve
            | OperationType::Rest
            | OperationType::Inlay
            | OperationType::Zigzag
            | OperationType::Trace
            | OperationType::Chamfer
            | OperationType::DropCutter
            | OperationType::Adaptive3d
            | OperationType::Waterline
            | OperationType::Pencil
            | OperationType::Scallop
            | OperationType::UnifiedFinish
            | OperationType::SteepShallow
            | OperationType::RampFinish
            | OperationType::SpiralFinish
            | OperationType::RadialFinish
            | OperationType::HorizontalFinish
            | OperationType::ProjectCurve => Some(MILLING_FORMULA_SOURCE),
        };
        assert_eq!(
            op.spec().feeds_formula_source,
            expected,
            "{op:?}: the registry's feeds_formula_source moved. Decide it here and in \
             the registry row together."
        );
    }
    assert_eq!(OperationType::ALL.len(), 24, "the operation count changed");
}

/// (b) The resolver runs on every cell, agrees with the arm `calculate`
/// records, and a cell whose lookup found a row is `VendorBacked`.
#[test]
fn a_cell_with_a_vendor_row_is_vendor_backed() {
    let cells = every_cell();
    assert_eq!(cells.len(), 24 * 5 * 4, "the matrix is not the full grid");

    let mut vendor_backed = 0usize;
    let mut formula_only = 0usize;
    for cell in &cells {
        assert_eq!(
            cell.support, cell.recorded,
            "{}: feeds_support and calculate disagree on the arm",
            cell.label
        );
        if cell.has_row {
            assert_eq!(
                cell.support,
                FeedsSupport::VendorBacked,
                "{}: the lookup found a row, so the arm must be VendorBacked",
                cell.label
            );
            vendor_backed += 1;
        } else {
            match (cell.support, cell.declared) {
                (FeedsSupport::FormulaOnly { source }, Some(declared)) => {
                    assert_eq!(source, declared, "{}: wrong formula source", cell.label);
                    formula_only += 1;
                }
                (other, declared) => panic!(
                    "{}: no row and declared source {declared:?}, but the arm is {other:?}",
                    cell.label
                ),
            }
        }
        assert!(
            !matches!(cell.validation, Err(FeedsError::Unbacked { .. })),
            "{}: validation raised Unbacked in Phase 0",
            cell.label
        );
    }
    // Non-vacuity: the grid holds both arms.
    assert!(vendor_backed > 0, "no cell found a vendor row");
    assert!(formula_only > 0, "no cell fell back to the formula");
}

/// (c) No cell refuses today.
///
/// Phase 4 re-blesses this pin with ruling R1 as the cause: when a
/// registry row changes to `None`, the cells of that operation with no
/// vendor row join this set.
#[test]
fn no_cell_refuses_today() {
    let refused: Vec<String> = every_cell()
        .into_iter()
        .filter(|cell| matches!(cell.support, FeedsSupport::Refuse { .. }))
        .map(|cell| cell.label)
        .collect();
    assert!(
        refused.is_empty(),
        "Phase 0 refuses no cell; these refuse: {refused:?}"
    );
}

/// (d) The refusal text names the operation, the tool family, the
/// material and the reason.
#[test]
fn the_unbacked_refusal_names_its_cell() {
    let err = FeedsError::Unbacked {
        operation: OperationType::Pencil,
        tool_family: ToolFamily::FlatEnd,
        material: MaterialFamily::Hardwood,
        reason: NO_BASIS_REASON,
    };
    let text = err.to_string();
    for needle in [
        OperationType::Pencil.label(),
        "FlatEnd",
        "Hardwood",
        NO_BASIS_REASON,
    ] {
        assert!(text.contains(needle), "{text:?} does not name {needle:?}");
    }
}
