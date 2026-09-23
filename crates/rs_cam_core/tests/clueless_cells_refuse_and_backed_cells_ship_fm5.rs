//! FM5 — a formula-only cell ships only where the R1 judgement backs it.
//!
//! Feeds matrix ruling R1, second half (operator, 2026-09-23): "refuse for
//! now! they can be our targets to work on!" The judgement
//! (`planning/feeds_matrix_2026-09-23/FORMULA_BACKING.md`, v2) is encoded as
//! `feeds::support::formula_backing`. A wood cell with no vendor row ships
//! the formula when the judgement is `Backed` and refuses with
//! `FeedsError::Unbacked` when it is `Clueless`. A material the judgement did
//! not cover keeps the formula, as before.
//!
//! The sentry pins four named cells and the door behaviour they imply:
//!
//! - (a) Drill with a flat end mill in hardwood refuses (no drill row for
//!   any tool; the 2.5 multiplier is unsourced);
//! - (b) Waterline with a ball nose in plywood refuses (no ball plywood row);
//! - (c) Trace with a flat end mill in hardwood ships FormulaOnly (Onsrud
//!   and Amana 1 x D charts back it);
//! - (d) a bull nose on a parallel finish in aluminium ships FormulaOnly (an
//!   unjudged material keeps the formula);
//! - (e) `suggest::default_operation` gives an add door an operation with
//!   the stock defaults and no recipe, so a refused cell still creates its
//!   operation.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::{
    StockContext, SuggestContext, SuggestParamsInput, SuggestedParams, default_operation,
    suggest_params,
};
use rs_cam_core::feeds::{
    EMBEDDED_LUT, FeedsError, FeedsSupport, SpindleStrategy, WorkholdingRigidity,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{AluminumAlloy, Material, PlywoodGrade, WoodSpecies};

fn tool_of(kind: ToolType) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(1), kind);
    t.diameter = 6.35;
    t.flute_count = 2;
    t.cutting_length = 19.05;
    t.shank_diameter = 6.35;
    t.shaft_diameter = 6.35;
    t.stickout = 27.05;
    if matches!(kind, ToolType::BullNose) {
        t.corner_radius = 0.95;
        t.corner_radius_mm = 0.95;
    }
    t
}

fn stock_ctx() -> StockContext {
    StockContext {
        stock_top_z: 0.0,
        stock_bottom_z: -18.0,
        stock_z: 18.0,
        stock_padding: 2.0,
    }
}

fn suggest(
    op: OperationType,
    kind: ToolType,
    material: &Material,
) -> Result<SuggestedParams, FeedsError> {
    let tool = tool_of(kind);
    let machine = MachineProfile::default();
    let stock = stock_ctx();
    suggest_params(SuggestParamsInput {
        op_type: op,
        tool: &tool,
        machine: &machine,
        material,
        workholding: WorkholdingRigidity::Medium,
        lut: &EMBEDDED_LUT,
        stock_ctx: &stock,
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::default(),
    })
}

fn hardwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    }
}

/// (a) and (b): two CLUELESS cells refuse, with the judgement's reason.
#[test]
fn clueless_cells_refuse_with_the_judgement_reason_fm5() {
    let drill = suggest(OperationType::Drill, ToolType::EndMill, &hardwood())
        .expect_err("Drill with a flat end mill in hardwood has no published figure");
    match &drill {
        FeedsError::Unbacked {
            operation, reason, ..
        } => {
            assert_eq!(*operation, OperationType::Drill);
            assert!(
                reason.contains("plunge drill") && reason.contains("2.5"),
                "reason must name the drill gap and the unsourced multiplier: {reason:?}"
            );
        }
        other => panic!("expected Unbacked, got {other:?}"),
    }
    let text = drill.to_string();
    assert!(
        text.contains("Drill") && !text.contains('{'),
        "the refusal names the operation as a word and leaks no Debug output: {text:?}"
    );

    let plywood = Material::Plywood {
        grade: PlywoodGrade::BalticBirch,
    };
    let waterline = suggest(OperationType::Waterline, ToolType::BallNose, &plywood)
        .expect_err("Waterline with a ball nose in plywood has no published figure");
    assert!(
        matches!(
            &waterline,
            FeedsError::Unbacked { reason, .. } if reason.contains("plywood")
        ),
        "expected the ball-nose plywood reason, got {waterline:?}"
    );
}

/// (c) and (d): a BACKED wood cell and an unjudged material ship the formula.
#[test]
fn backed_and_unjudged_cells_ship_the_formula_fm5() {
    let trace = suggest(OperationType::Trace, ToolType::EndMill, &hardwood())
        .expect("Trace with a flat end mill in hardwood is BACKED");
    assert!(
        matches!(trace.feeds_result.support, FeedsSupport::FormulaOnly { .. }),
        "a BACKED cell with no row ships FormulaOnly, got {:?}",
        trace.feeds_result.support
    );

    let aluminium = Material::Aluminum {
        alloy: AluminumAlloy::Alloy6061T6,
    };
    let parallel = suggest(OperationType::RadialFinish, ToolType::BullNose, &aluminium)
        .expect("an unjudged material keeps the formula");
    assert!(
        matches!(
            parallel.feeds_result.support,
            FeedsSupport::FormulaOnly { .. } | FeedsSupport::VendorBacked
        ),
        "got {:?}",
        parallel.feeds_result.support
    );
}

/// (e) The add doors' fallback: the registry default with the stock
/// defaults, and no recipe.
#[test]
fn the_default_operation_carries_the_stock_defaults_and_no_recipe_fm5() {
    let stock = stock_ctx();
    let op = default_operation(OperationType::Drill, &stock);
    assert_eq!(op.op_type(), OperationType::Drill);
    // `apply_stock_defaults` sets a drill's depth to the stock height.
    let rs_cam_core::compute::catalog::OperationConfig::Drill(cfg) = &op else {
        panic!("default_operation must build the requested operation kind");
    };
    assert!(
        (cfg.depth - stock.stock_z).abs() < 1e-9,
        "the stock default was not applied: depth {}",
        cfg.depth
    );
}
