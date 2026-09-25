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
//! - (a) Drill with a 15.875 mm flat end mill in hardwood refuses: the G6
//!   drill claim (ruling B5) covers 3.0-12.7 mm only (widened 2026-09-25),
//!   and the text names the group;
//! - (b) Waterline with a ball nose in plywood refuses (no ball plywood row);
//! - (c) Trace with a flat end mill in hardwood ships FormulaOnly (Onsrud
//!   and Amana 1 x D charts back it);
//! - (d) a bull nose on a parallel finish in aluminium ships FormulaOnly (an
//!   unjudged material keeps the formula);
//! - (e) `suggest::default_operation` gives an add door an operation with
//!   the stock defaults and no recipe, so a refused cell still creates its
//!   operation;
//! - (f) extrapolation P2 (G2, 2026-09-24), strict ruling: a V-bit Pocket in
//!   MDF refuses with the Onsrud 37-series text, a V-bit Waterline in MDF
//!   refuses with the contour-finish text, a ball Pocket in plywood refuses
//!   with the ball-nose plywood text, and no V-bit reason in MDF or plywood
//!   says that no V-bit chipload exists there (the Onsrud rows print one);
//! - (g) extrapolation A3 (G3, family transfer): the tapered ball-nose
//!   formula arms are gone. A Ø3.175 tapered Profile, Trace and Waterline
//!   in hardwood ship the Onsrud 77-100 pocket row through the family claim;
//!   a 1.0 mm tapered Trace in softwood (the formula before A3) refuses on
//!   the size rule (orchestrator decision 4); a 0.4 mm tip refuses on the
//!   tip floor (ruling B1).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::{
    StockContext, SuggestContext, SuggestParamsInput, SuggestedParams, default_operation,
    suggest_params,
};
use rs_cam_core::feeds::{EMBEDDED_LUT, FeedsError, FeedsSupport, SpindleStrategy};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{AluminumAlloy, Material, PlywoodGrade, SheetGoodKind, WoodSpecies};

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
    suggest_tool(op, &tool_of(kind), material)
}

fn hardwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    }
}

/// (a) and (b): two CLUELESS cells refuse, with the judgement's reason.
#[test]
fn clueless_cells_refuse_with_the_judgement_reason_fm5() {
    let mut drill_tool = tool_of(ToolType::EndMill);
    drill_tool.diameter = 15.875;
    drill_tool.cutting_length = 38.0;
    drill_tool.shank_diameter = 15.875;
    drill_tool.shaft_diameter = 15.875;
    drill_tool.stickout = drill_tool.cutting_length + 8.0;
    let drill = suggest_tool(OperationType::Drill, &drill_tool, &hardwood())
        .expect_err("a 15.875 mm flat plunge is outside the G6 claim's 3.0-12.7 mm");
    match &drill {
        FeedsError::Unbacked {
            operation, reason, ..
        } => {
            assert_eq!(*operation, OperationType::Drill);
            assert!(
                reason.contains("plunge") && reason.contains("G6") && !reason.contains("2.5"),
                "reason must name the drill gap and the G6 group, and no multiplier: {reason:?}"
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
            FeedsSupport::FormulaOnly { .. }
                | FeedsSupport::VendorBacked
                | FeedsSupport::Extrapolated { .. }
                | FeedsSupport::FamilyTransferred { .. }
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

/// The reason of an `Unbacked` refusal; any other outcome fails the test.
fn unbacked_reason(result: Result<SuggestedParams, FeedsError>, cell: &str) -> String {
    match result {
        Err(FeedsError::Unbacked { reason, .. }) => reason.into_owned(),
        Err(other) => panic!("{cell}: expected Unbacked, got {other:?}"),
        Ok(p) => panic!(
            "{cell}: expected Unbacked, got a recipe on {:?}",
            p.feeds_result.support
        ),
    }
}

/// (f) The G2 judgement (strict): the Onsrud 37-series V-bit rows serve
/// trace passes in MDF and plywood, and the formula cells stay refused with
/// texts that name the figure.
#[test]
fn the_vbit_mdf_and_plywood_cells_refuse_with_the_g2_texts_fm5() {
    let mdf = Material::SheetGood {
        kind: SheetGoodKind::Mdf,
    };
    let plywood = Material::Plywood {
        grade: PlywoodGrade::BalticBirch,
    };

    // A V-bit Pocket in MDF: no pocket row, and the formula is below half of
    // the Onsrud 37-series bands at 1/4 in.
    let pocket = unbacked_reason(
        suggest(OperationType::Pocket, ToolType::VBit, &mdf),
        "V-bit Pocket in MDF",
    );
    assert_eq!(
        pocket,
        "No published figure backs the formula for a V-bit on pocket, contour or trace passes \
         in MDF or plywood: for a 1/4 in V-bit the formula is below half of the Onsrud \
         37-series bands (0.41x to 0.48x)."
    );

    // A V-bit Waterline in MDF: the contour-finish text holds in MDF too.
    let waterline = unbacked_reason(
        suggest(OperationType::Waterline, ToolType::VBit, &mdf),
        "V-bit Waterline in MDF",
    );
    assert_eq!(
        waterline,
        "No published figure backs the formula for a V-bit on waterline or steep-shallow \
         passes: no vendor prints a V-bit 3D finish chart."
    );

    // A ball Pocket in plywood: no V-bit row changes the ball-nose verdict.
    let ball = unbacked_reason(
        suggest(OperationType::Pocket, ToolType::BallNose, &plywood),
        "ball Pocket in plywood",
    );
    assert_eq!(
        ball,
        "No published chipload exists for a ball-nose cutter on adaptive, pocket, contour or \
         trace passes in plywood."
    );

    // No V-bit reason in MDF or plywood says that no V-bit chipload exists.
    for material in [&mdf, &plywood] {
        for op in OperationType::ALL.iter().copied() {
            if let Err(FeedsError::Unbacked { reason, .. }) = suggest(op, ToolType::VBit, material)
            {
                assert!(
                    !reason.contains("No published V-bit chipload exists for MDF"),
                    "{op:?} with a V-bit in {material:?} still says no V-bit chipload exists: \
                     {reason}"
                );
            }
        }
    }
}

/// A tapered ball with the tip `tip_mm` and a shank wider than the tip.
fn tapered(tip_mm: f64) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(1), ToolType::TaperedBallNose);
    t.diameter = tip_mm;
    t.flute_count = 2;
    t.cutting_length = (tip_mm * 3.0).max(12.0);
    t.taper_half_angle = 7.0;
    t.shank_diameter = (tip_mm + 3.0).max(6.0);
    t.shaft_diameter = t.shank_diameter;
    t.stickout = t.cutting_length + 8.0;
    t
}

fn suggest_tool(
    op: OperationType,
    tool: &ToolConfig,
    material: &Material,
) -> Result<SuggestedParams, FeedsError> {
    let machine = MachineProfile::default();
    let stock = stock_ctx();
    suggest_params(SuggestParamsInput {
        op_type: op,
        tool,
        machine: &machine,
        material,
        lut: &EMBEDDED_LUT,
        stock_ctx: &stock,
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::default(),
    })
}

/// (g) The tapered cells after A3 step 2: the family claim ships the
/// printed band, and the micro and sub-floor tips refuse.
#[test]
fn the_tapered_cells_ship_the_family_claim_or_refuse_on_size_fm5() {
    let tool = tapered(3.175);
    for op in [
        OperationType::Profile,
        OperationType::Trace,
        OperationType::Waterline,
    ] {
        let s = suggest_tool(op, &tool, &hardwood())
            .unwrap_or_else(|e| panic!("{op:?}: a tapered ball in hardwood ships, got {e}"));
        let FeedsSupport::FamilyTransferred { family, size } = &s.feeds_result.support else {
            panic!(
                "{op:?}: expected FamilyTransferred, got {:?}",
                s.feeds_result.support
            );
        };
        assert!(size.is_none(), "{op:?}: the tip is the printed size");
        assert_eq!(
            family.source_rows,
            vec!["onsrud-hardwood-77-100-1_8-pocket".to_owned()],
            "{op:?}"
        );
    }

    let softwood = Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    };
    // Decision 4: the 1.0 mm tip reads the 1/4 in pocket row (6.35 mm, the
    // exact flute count), 6.3x the tool, and no chart prints a trace series
    // that brackets the tip.
    let micro = unbacked_reason(
        suggest_tool(OperationType::Trace, &tapered(1.0), &softwood),
        "1.0 mm tapered Trace in softwood",
    );
    assert!(
        micro.starts_with(
            "no published figure for a 1.00 mm tapered ball nose; the nearest chart row is 6.35 \
             mm,"
        ),
        "{micro}"
    );
    let floor = unbacked_reason(
        suggest_tool(OperationType::Trace, &tapered(0.4), &softwood),
        "0.4 mm tapered Trace in softwood",
    );
    assert!(
        floor.starts_with("no published figure for a 0.40 mm tapered ball nose: no wood chart"),
        "{floor}"
    );
}
