//! Feeds contract P13a (architecture review 2026-11-05, H3): the basis of a
//! tool fact carries the kind of the fact.
//!
//! A tool fact that is `Unknown` plans at its legacy loader default
//! (`compute::tool_config::LEGACY_DEFAULT_*`), and each default is an F201
//! row marked Unsourced. Before P13a the basis of a tool fact was a bare
//! "tool library value" with no input, so the legacy row did not show in
//! the Unsourced rows of a Suggest (`SuggestWarning::UnsourcedRowsAccepted`).
//!
//! The claims:
//!
//! - A flat end mill with an `Unknown` flute count lists the legacy flute
//!   count row in the Unsourced rows of its Suggest.
//! - The same tool with a stated flute count does not list that row.
//! - A tool with an `Unknown` cutting length gives an envelope whose flute
//!   guard and cutting-length limits read the legacy cutting length row.

#![expect(
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::FeedsInput;
use rs_cam_core::feeds::envelope::{EnvelopeInput, LimitKind, SafeEnvelope};
use rs_cam_core::feeds::suggest::{
    NoProject, SuggestContext, SuggestForOperationInput, SuggestWarning, suggest_for_operation,
};
use rs_cam_core::feeds::{SpindleStrategy, calculate, embedded_vendor_lut};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

const LEGACY_FLUTE_ROW: &str = "compute.tool_config.LEGACY_DEFAULT_FLUTE_COUNT";
const LEGACY_LENGTH_ROW: &str = "compute.tool_config.LEGACY_DEFAULT_CUTTING_LENGTH_MM";

fn softwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    }
}

/// A 6 mm flat end mill with every fact stated.
fn flat() -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    t.diameter = 6.0;
    t.set_flute_count(ToolFact::Stated(2));
    t.set_cutting_length(ToolFact::Stated(22.0));
    t.set_shank_diameter(ToolFact::Stated(6.0));
    t.shaft_diameter = 6.0;
    t.set_stickout(ToolFact::Stated(30.0));
    t
}

/// The Unsourced rows of the Suggest of a Pocket with `tool`.
fn unsourced_rows(tool: &ToolConfig) -> Vec<String> {
    let op = OperationConfig::new_default(OperationType::Pocket);
    let machine = MachineProfile::generic_wood_router();
    let material = softwood();
    let s = suggest_for_operation(SuggestForOperationInput {
        operation: &op,
        tool,
        machine: &machine,
        material: &material,
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::MatchChart,
        context: SuggestContext::no_project(NoProject::NONE),
    })
    .unwrap_or_else(|e| panic!("Suggest must serve the Pocket: {e}"));
    s.warnings
        .iter()
        .find_map(|w| {
            if let SuggestWarning::UnsourcedRowsAccepted { rows } = w {
                Some(rows.clone())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

#[test]
fn an_unknown_flute_count_lists_its_legacy_row_p13a() {
    let mut tool = flat();
    tool.set_flute_count(ToolFact::Unknown);
    let rows = unsourced_rows(&tool);
    assert!(
        rows.iter().any(|r| r == LEGACY_FLUTE_ROW),
        "an Unknown flute count plans at the legacy default; the Unsourced rows must name \
         {LEGACY_FLUTE_ROW}. Rows: {rows:?}"
    );
}

#[test]
fn a_stated_flute_count_lists_no_legacy_row_p13a() {
    let rows = unsourced_rows(&flat());
    assert!(
        !rows.iter().any(|r| r == LEGACY_FLUTE_ROW),
        "a stated flute count reads no legacy row. Rows: {rows:?}"
    );
}

#[test]
fn an_unknown_cutting_length_puts_its_legacy_row_in_the_flute_guard_p13a() {
    let mut tool = flat();
    tool.set_cutting_length(ToolFact::Unknown);
    let mut op = rs_cam_core::compute::fixture_values::fixture_op(OperationType::Pocket);
    let machine = MachineProfile::generic_wood_router();
    let material = softwood();
    let r = calculate(&FeedsInput::of_operation(
        &op,
        &tool,
        &material,
        &machine,
        embedded_vendor_lut(),
        SpindleStrategy::MatchChart,
    ));
    op.set_feed_rate(r.feed_rate_mm_min);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a positive RPM below 100 000"
    )]
    op.set_spindle_rpm(Some(r.rpm as u32));
    let env = SafeEnvelope::for_cut(EnvelopeInput {
        operation: &op,
        tool: &tool,
        machine: &machine,
        material: &material,
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::MatchChart,
        context: SuggestContext::no_project(NoProject::NONE),
    })
    .unwrap_or_else(|e| panic!("the Pocket envelope is backed: {e}"));
    let guard = env
        .limit(LimitKind::FluteGuard)
        .unwrap_or_else(|| panic!("the envelope keeps the flute guard"));
    let names: Vec<String> = guard
        .basis
        .unsourced_rules()
        .iter()
        .map(|c| c.name.to_string())
        .collect();
    assert!(
        names.iter().any(|n| n == LEGACY_LENGTH_ROW),
        "the flute guard of an Unknown cutting length reads {LEGACY_LENGTH_ROW}. Rows: {names:?}"
    );
    let length = env
        .limit(LimitKind::CuttingLength)
        .unwrap_or_else(|| panic!("the envelope keeps the cutting length"));
    let names: Vec<String> = length
        .basis
        .unsourced_rules()
        .iter()
        .map(|c| c.name.to_string())
        .collect();
    assert!(
        names.iter().any(|n| n == LEGACY_LENGTH_ROW),
        "the cutting-length limit of an Unknown cutting length reads {LEGACY_LENGTH_ROW}. \
         Rows: {names:?}"
    );
}
