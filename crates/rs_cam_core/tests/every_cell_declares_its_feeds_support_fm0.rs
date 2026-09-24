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
//!   and a cell whose lookup found a row is `VendorBacked`, `Extrapolated`
//!   (a G1 size claim, extrapolation P1), `FamilyTransferred` (a G3 family
//!   claim, A3) or a size refusal.
//! - [`no_cell_refuses_today`] (c) — the set of `Refuse` cells is empty.
//! - [`the_unbacked_refusal_names_its_cell`] (d) — the `Display` text of
//!   `FeedsError::Unbacked` names the operation, the tool family and the
//!   reason.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::feeds_input_for_operation;
use rs_cam_core::feeds::support::{
    DRILL_FORMULA_SOURCE, FormulaBacking, MILLING_FORMULA_SOURCE, NO_BASIS_REASON, formula_backing,
};
use rs_cam_core::feeds::vendor_lut::{MaterialFamily, ToolFamily};
use rs_cam_core::feeds::{
    FeedsError, FeedsSupport, SpindleStrategy, calculate, embedded_vendor_lut, feeds_support,
    validate_tool_for_operation,
};
use rs_cam_core::feeds::{OperationFamily, PassRole};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, PlywoodGrade, SheetGoodKind, WoodSpecies};

/// The four wood material families of the matrix, one material each.
fn wood_materials() -> [(&'static str, MaterialFamily, Material); 4] {
    [
        (
            "hardwood",
            MaterialFamily::Hardwood,
            Material::SolidWood {
                species: WoodSpecies::GenericHardwood,
            },
        ),
        (
            "softwood",
            MaterialFamily::Softwood,
            Material::SolidWood {
                species: WoodSpecies::GenericSoftwood,
            },
        ),
        (
            "mdf",
            MaterialFamily::Mdf,
            Material::SheetGood {
                kind: SheetGoodKind::Mdf,
            },
        ),
        (
            "plywood_hardwood",
            MaterialFamily::PlywoodHardwood,
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
    /// The R1 judgement's key for this cell.
    tool_family: ToolFamily,
    family: OperationFamily,
    role: PassRole,
    material_family: MaterialFamily,
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
            for (material_name, material_family, material) in &wood_materials() {
                let (family, role) = operation.feeds_style();
                let input = feeds_input_for_operation(
                    &operation,
                    &tool,
                    material,
                    &machine,
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
                    tool_family: tool_type.cutter_kind().lut_family(),
                    family,
                    role,
                    material_family: *material_family,
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
    let mut extrapolated = 0usize;
    let mut family_transferred = 0usize;
    let mut formula_only = 0usize;
    let mut refused = 0usize;
    let mut size_refused = 0usize;
    for cell in &cells {
        assert_eq!(
            cell.support, cell.recorded,
            "{}: feeds_support and calculate disagree on the arm",
            cell.label
        );
        if cell.has_row {
            // Operator ruling 2026-09-24 (R1 applied to size): a row found
            // for a micro tool more than 2x off its diameter refuses, and the
            // reason names both sizes.
            // Extrapolation P1 step 3: an off-size row answers through a
            // stated G1 size claim (`Extrapolated`), and the claim names
            // the row's diameter and the query key.
            match &cell.support {
                FeedsSupport::VendorBacked => vendor_backed += 1,
                FeedsSupport::Extrapolated { claim } => {
                    assert!(
                        claim.scale.is_finite() && claim.scale > 0.0,
                        "{}: a claim must carry a positive scale: {claim:?}",
                        cell.label
                    );
                    assert!(
                        claim.range_mm.contains(&claim.query_diameter_mm),
                        "{}: the claim's range must hold the query key: {claim:?}",
                        cell.label
                    );
                    extrapolated += 1;
                }
                // A3 (G3): a family rule carries a home row into this
                // family. The cell ships, so the evidence half must not
                // raise `Unbacked`.
                FeedsSupport::FamilyTransferred { family, size } => {
                    assert!(
                        family.source_rows.len() == 1 && family.home.0 != family.query.0,
                        "{}: a family claim names one row and two families: {family:?}",
                        cell.label
                    );
                    if let Some(claim) = size {
                        assert!(
                            claim.range_mm.contains(&claim.query_diameter_mm),
                            "{}: the size claim's range must hold the query key: {claim:?}",
                            cell.label
                        );
                    }
                    assert!(
                        !matches!(cell.validation, Err(FeedsError::Unbacked { .. })),
                        "{}: a transferred cell must not raise Unbacked",
                        cell.label
                    );
                    family_transferred += 1;
                }
                FeedsSupport::Refuse { reason } if is_size_refusal(reason) => {
                    assert!(
                        matches!(
                            cell.validation,
                            Err(FeedsError::Unbacked { .. })
                                | Err(FeedsError::WrongToolForOperation { .. })
                        ),
                        "{}: the size rule refuses but validation did not: {:?}",
                        cell.label,
                        cell.validation
                    );
                    size_refused += 1;
                }
                other => panic!(
                    "{}: the lookup found a row, so the arm must be VendorBacked, \
                     Extrapolated, FamilyTransferred or a size refusal, got {other:?}",
                    cell.label
                ),
            }
        } else {
            let judged = formula_backing(
                cell.tool_family,
                cell.family,
                cell.role,
                cell.material_family,
            );
            match (cell.support.clone(), cell.declared) {
                (FeedsSupport::FormulaOnly { source }, Some(declared)) => {
                    assert_eq!(source, declared, "{}: wrong formula source", cell.label);
                    assert_eq!(
                        judged,
                        FormulaBacking::Backed,
                        "{}: ships the formula but the R1 judgement is {judged:?}",
                        cell.label
                    );
                    // The registry tool rule (FM4) may still refuse the
                    // cell; the evidence half must not.
                    assert!(
                        !matches!(cell.validation, Err(FeedsError::Unbacked { .. })),
                        "{}: a BACKED cell must not raise Unbacked",
                        cell.label
                    );
                    formula_only += 1;
                }
                // Ruling R1 (2026-09-23): a wood cell with no row that the
                // judgement calls CLUELESS refuses, with the judgement's reason.
                (FeedsSupport::Refuse { reason }, Some(_)) => {
                    assert!(
                        matches!(judged, FormulaBacking::Clueless { reason: r } if r == reason.as_ref()),
                        "{}: refuses with {reason:?} but the judgement says {judged:?}",
                        cell.label
                    );
                    // The validator reads the registry tool rule first, so
                    // a cell both doors refuse carries `WrongToolForOperation`.
                    assert!(
                        matches!(
                            cell.validation,
                            Err(FeedsError::Unbacked { .. }
                                | FeedsError::WrongToolForOperation { .. })
                        ),
                        "{}: the arm refuses but validation did not refuse: {:?}",
                        cell.label,
                        cell.validation
                    );
                    refused += 1;
                }
                (other, declared) => panic!(
                    "{}: no row and declared source {declared:?}, but the arm is {other:?}",
                    cell.label
                ),
            }
        }
    }
    // Non-vacuity: the grid holds all three arms.
    assert!(vendor_backed > 0, "no cell found a vendor row");
    assert!(formula_only > 0, "no cell fell back to the formula");
    // Extrapolation P1 step 3: the default Ø6.35 ball nose sits on the Ø6.0
    // ball rows, so the grid holds form C claims.
    assert!(extrapolated > 0, "no cell shipped through a G1 size claim");
    // A3 step 2: the default Ø3.175 tapered ball reads the Onsrud 77-100
    // pocket row on Profile, Trace, Pencil, Waterline and SteepShallow.
    assert!(
        family_transferred > 0,
        "no cell shipped through a G3 family claim"
    );
    assert!(
        refused > 0,
        "no cell refuses; ruling R1 encodes a CLUELESS set"
    );
    println!(
        "the size rule refuses {size_refused} cells that found a row; \
         {extrapolated} cells ship through a G1 size claim; \
         {family_transferred} cells ship through a G3 family claim"
    );
}

/// The size refusal's text starts with this (`support::micro_extrapolation_refusal`).
fn is_size_refusal(reason: &str) -> bool {
    reason.starts_with("no published figure for a ")
}

/// (c) The refusing cells are exactly the cells the R1 judgement calls
/// CLUELESS and that no vendor row answers.
///
/// Re-blessed 2026-09-23 with ruling R1 as the cause. Phase 0 pinned an
/// empty set; the operator ruled "refuse for now, they can be our targets
/// to work on", so the set is now the judgement table, and a new vendor
/// row or a new judgement moves a cell out of it with its cause named.
#[test]
fn the_refusing_cells_are_the_clueless_cells() {
    let cells = every_cell();
    // The size rule (2026-09-24) refuses cells that DID find a row; this
    // test is about the no-row CLUELESS set, so those are left out here and
    // counted in `a_cell_with_a_vendor_row_is_vendor_backed`.
    let refused: Vec<&CellOutcome> = cells
        .iter()
        .filter(|cell| {
            matches!(&cell.support, FeedsSupport::Refuse { reason } if !is_size_refusal(reason))
        })
        .collect();
    let clueless: Vec<&CellOutcome> = cells
        .iter()
        .filter(|cell| {
            !cell.has_row
                && matches!(
                    formula_backing(
                        cell.tool_family,
                        cell.family,
                        cell.role,
                        cell.material_family
                    ),
                    FormulaBacking::Clueless { .. }
                )
        })
        .collect();
    let refused_labels: Vec<&str> = refused.iter().map(|c| c.label.as_str()).collect();
    let clueless_labels: Vec<&str> = clueless.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        refused_labels, clueless_labels,
        "the refusing set and the judgement's CLUELESS set differ"
    );
    // Non-vacuity, and the count the operator saw (FORMULA_BACKING_v2: 270
    // matrix cells at two diameters; this grid walks one diameter).
    assert!(!refused.is_empty(), "ruling R1 refuses at least one cell");
    println!("R1 refuses {} of {} grid cells", refused.len(), cells.len());
}

/// (d) The refusal text names the operation, the tool family, the
/// material and the reason.
#[test]
fn the_unbacked_refusal_names_its_cell() {
    let err = FeedsError::Unbacked {
        operation: OperationType::Pencil,
        tool_family: ToolFamily::FlatEnd,
        material: MaterialFamily::Hardwood,
        reason: NO_BASIS_REASON.into(),
    };
    let text = err.to_string();
    for needle in [
        OperationType::Pencil.label(),
        ToolFamily::FlatEnd.label(),
        MaterialFamily::Hardwood.label(),
        NO_BASIS_REASON,
    ] {
        assert!(text.contains(needle), "{text:?} does not name {needle:?}");
    }
}
