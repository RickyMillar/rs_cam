//! FM4 — Suggest refuses exactly what the registry's own tool rule refuses.
//!
//! Feeds matrix ruling R1 (2026-09-23): a refusal must rest on a published
//! figure or a NAMED RULE. Each operation's registry row already names one:
//! `OpRegistryEntry::tool_constraints` (`ToolConstraintsDef::allows`). The
//! generator refused on it while Suggest shipped a recipe under a Critical
//! `compat.*` flag (EVIDENCE 4.4-15, 3.1-22, 3.5-18). After R1 the two doors
//! agree: `validate_tool_for_operation` reads the same predicate.
//!
//! The sentry pins:
//!
//! - (a) for every `OperationType x ToolType`, `suggest_params` refuses if and
//!   only if the registry row does not allow the tool's cutter kind;
//! - (b) every refusal text names the operation and carries no Rust `Debug`
//!   output (no `{`);
//! - (c) the pairs the rulings named: EndMill/Pencil, TaperedBallNose/Chamfer
//!   and EndMill/SpiralFinish refuse; BallNose/Pencil, VBit/Chamfer and
//!   BallNose/SpiralFinish ship.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::{
    StockContext, SuggestContext, SuggestParamsInput, SuggestedParams, suggest_params,
};
use rs_cam_core::feeds::{EMBEDDED_LUT, FeedsError, SpindleStrategy, WorkholdingRigidity};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// One common tool per kind, the survey's construction (the tapered ball
/// needs a shank wider than its tip).
fn tool_of(kind: ToolType) -> ToolConfig {
    let diameter = match kind {
        ToolType::TaperedBallNose => 3.175,
        _ => 6.35,
    };
    let mut t = ToolConfig::new_default(ToolId(1), kind);
    t.diameter = diameter;
    t.flute_count = 2;
    t.cutting_length = (diameter * 3.0).max(12.0);
    t.shank_diameter = diameter.max(3.0);
    t.shaft_diameter = diameter.max(3.0);
    t.stickout = t.cutting_length + 8.0;
    if matches!(kind, ToolType::BullNose) {
        t.corner_radius = diameter * 0.15;
        t.corner_radius_mm = diameter * 0.15;
    }
    if matches!(kind, ToolType::TaperedBallNose) {
        t.taper_half_angle = 7.0;
        t.shank_diameter = (diameter + 3.0).max(6.0);
        t.shaft_diameter = t.shank_diameter;
    }
    if matches!(kind, ToolType::VBit) {
        t.included_angle = 60.0;
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

fn suggest(op: OperationType, kind: ToolType) -> Result<SuggestedParams, FeedsError> {
    let tool = tool_of(kind);
    let machine = MachineProfile::default();
    let material = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    let stock = stock_ctx();
    suggest_params(SuggestParamsInput {
        op_type: op,
        tool: &tool,
        machine: &machine,
        material: &material,
        workholding: WorkholdingRigidity::Medium,
        lut: &EMBEDDED_LUT,
        stock_ctx: &stock,
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::default(),
    })
}

fn registry_allows(op: OperationType, kind: ToolType) -> bool {
    op.registry_entry()
        .tool_constraints
        .allows(kind.cutter_kind())
}

/// (a) The two doors agree on every cell of the grid.
#[test]
fn suggest_refuses_exactly_what_the_registry_refuses_fm4() {
    let mut mismatches = Vec::new();
    let mut refusing = Vec::new();
    for &op in OperationType::ALL {
        for &kind in ToolType::ALL {
            let allowed = registry_allows(op, kind);
            // An `Unbacked` refusal (ruling R1, evidence half) is not the
            // tool rule; only `WrongToolForOperation` is.
            let shipped = !matches!(
                suggest(op, kind),
                Err(FeedsError::WrongToolForOperation { .. })
            );
            if allowed != shipped {
                mismatches.push(format!(
                    "{op:?}/{kind:?}: registry allows {allowed}, Suggest shipped {shipped}"
                ));
            }
            if !allowed {
                refusing.push(format!("{op:?}/{kind:?}"));
            }
        }
    }
    println!("registry refusals ({}): {refusing:?}", refusing.len());
    // Non-vacuity: the registry refuses something.
    assert!(
        refusing.len() >= 6,
        "the registry refuses fewer than six cells; the grid no longer exercises the rule"
    );
    assert!(
        mismatches.is_empty(),
        "Suggest and the registry disagree on {} cells:\n  {}",
        mismatches.len(),
        mismatches.join("\n  ")
    );
}

/// (b) Every refusal names the operation as a word and leaks no Debug text.
#[test]
fn the_refusal_text_names_the_operation_without_debug_output_fm4() {
    let mut seen = 0;
    for &op in OperationType::ALL {
        for &kind in ToolType::ALL {
            let Err(err @ FeedsError::WrongToolForOperation { .. }) = suggest(op, kind) else {
                continue;
            };
            seen += 1;
            let text = err.to_string();
            let label = op.spec().label;
            assert!(
                text.contains(label),
                "{op:?}/{kind:?}: the refusal text {text:?} does not name the operation {label:?}"
            );
            assert!(
                !text.contains('{'),
                "{op:?}/{kind:?}: the refusal text {text:?} leaks Debug output"
            );
        }
    }
    assert!(
        seen > 0,
        "non-vacuity: no cell refused, so no text was checked"
    );
}

/// (c) The pairs the rulings named.
#[test]
fn the_named_pairs_refuse_or_ship_fm4() {
    for (op, kind) in [
        (OperationType::Pencil, ToolType::EndMill),
        (OperationType::Chamfer, ToolType::TaperedBallNose),
        (OperationType::SpiralFinish, ToolType::EndMill),
    ] {
        assert!(
            matches!(
                suggest(op, kind),
                Err(FeedsError::WrongToolForOperation { .. })
            ),
            "{op:?}/{kind:?} must refuse: the registry's tool rule does not allow this kind"
        );
    }
    for (op, kind) in [
        (OperationType::Pencil, ToolType::BallNose),
        (OperationType::Chamfer, ToolType::VBit),
        (OperationType::SpiralFinish, ToolType::BallNose),
    ] {
        assert!(
            !matches!(
                suggest(op, kind),
                Err(FeedsError::WrongToolForOperation { .. })
            ),
            "{op:?}/{kind:?} must pass the tool rule: the registry allows this kind"
        );
    }
}
