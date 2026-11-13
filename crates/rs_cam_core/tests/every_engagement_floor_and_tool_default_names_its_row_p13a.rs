//! Feeds contract P13a (architecture review 2026-11-05, H3b, L3, L4): four
//! numbers that shipped with no F201 row now name one.
//!
//! - `feeds::MIN_AP_MM` and `feeds::MIN_AE_MM`, the engagement floor of the
//!   calculator and of the envelope. Before P13a their basis was a law with
//!   no input (`Basis::derived("MIN_AP_MM", [])`).
//! - The 30 degree helix that five cutter constructors typed
//!   (`tool/flat.rs`, `ball.rs`, `tapered_ball.rs`, `bullnose.rs`,
//!   `vbit.rs`). P13a makes it one constant, `tool::DEFAULT_HELIX_DEG`.
//! - `compute::operation_configs::DRILL_DWELL_S`, the G82 dwell.
//!
//! The claims:
//!
//! - `feeds::formula_constants()` holds one row per number, and the row
//!   value is the value that the code reads.
//! - The basis of a depth or a width that the engagement floor raised reads
//!   the row (`Basis::Rule`).
//! - The two envelope engagement-floor limits read the rows.

#![expect(
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::operation_configs::DRILL_DWELL_S;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::FeedsInput;
use rs_cam_core::feeds::envelope::{EnvelopeInput, LimitKind, SafeEnvelope};
use rs_cam_core::feeds::suggest::{NoProject, SuggestContext};
use rs_cam_core::feeds::{
    Basis, SpindleStrategy, calculate, embedded_vendor_lut, formula_constants,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{FormulaConstant, Material, WoodSpecies};
use rs_cam_core::tool::{
    BallEndmill, BullNoseEndmill, FlatEndmill, TaperedBallEndmill, VBitEndmill,
};

const MIN_AP_ROW: &str = "feeds.MIN_AP_MM";
const MIN_AE_ROW: &str = "feeds.MIN_AE_MM";
const HELIX_ROW: &str = "tool.DEFAULT_HELIX_DEG";
const DWELL_ROW: &str = "compute.operation_configs.DRILL_DWELL_S";

fn row(name: &str) -> FormulaConstant {
    formula_constants()
        .into_iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("F201: formula_constants() holds no row {name}"))
}

fn rule_names(b: &Basis) -> Vec<String> {
    b.unsourced_rules()
        .iter()
        .map(|c| c.name.to_string())
        .collect()
}

fn softwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    }
}

fn flat() -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    t.diameter = 6.0;
    t.set_flute_count(ToolFact::Stated(2));
    t.set_cutting_length(ToolFact::Stated(20.0));
    t.set_shank_diameter(ToolFact::Stated(6.0));
    t.shaft_diameter = 6.0;
    t.set_stickout(ToolFact::Stated(28.0));
    t
}

#[test]
fn the_four_numbers_have_f201_rows_p13a() {
    let ap = row(MIN_AP_ROW);
    let ae = row(MIN_AE_ROW);
    let helix = row(HELIX_ROW);
    let dwell = row(DWELL_ROW);
    assert!(ap.value > 0.0 && ae.value > 0.0, "the floors are positive");
    assert!(
        (dwell.value - DRILL_DWELL_S).abs() < 1e-12,
        "the dwell row holds DRILL_DWELL_S"
    );
    // The five constructors read the one helix of the row.
    let helices = [
        ("flat", FlatEndmill::new(6.0, 20.0).helix_deg),
        ("ball", BallEndmill::new(6.0, 20.0).helix_deg),
        (
            "bull",
            BullNoseEndmill::new(6.0, 1.0, 20.0)
                .unwrap_or_else(|e| panic!("a valid bull nose: {e:?}"))
                .helix_deg,
        ),
        (
            "tapered",
            TaperedBallEndmill::new(1.0, 5.0, 6.0, 20.0)
                .unwrap_or_else(|e| panic!("a valid tapered ball: {e:?}"))
                .helix_deg,
        ),
        (
            "vbit",
            VBitEndmill::new(6.0, 60.0, 20.0)
                .unwrap_or_else(|e| panic!("a valid V-bit: {e:?}"))
                .helix_deg,
        ),
    ];
    for (name, h) in helices {
        assert!(
            (h - helix.value).abs() < 1e-12,
            "the {name} constructor helix {h} must be the row {HELIX_ROW} = {}",
            helix.value
        );
    }
}

#[test]
fn a_floored_depth_and_width_read_the_floor_rows_p13a() {
    let mut op = rs_cam_core::compute::fixture_values::fixture_op(OperationType::Pocket);
    // A stated depth and width under the floors: the calculator raises both.
    assert!(op.set_depth_per_pass(row(MIN_AP_ROW).value * 0.5));
    assert!(op.set_stepover(row(MIN_AE_ROW).value * 0.5));
    let machine = MachineProfile::generic_wood_router();
    let material = softwood();
    let tool = flat();
    let input = FeedsInput::of_operation(
        &op,
        &tool,
        &material,
        &machine,
        embedded_vendor_lut(),
        SpindleStrategy::MatchChart,
    );
    let r = calculate(&input);
    assert!(
        (r.axial_depth_mm - row(MIN_AP_ROW).value).abs() < 1e-12,
        "the depth sits on the floor"
    );
    let ap = rule_names(&r.bases.ap);
    assert!(ap.iter().any(|n| n == MIN_AP_ROW), "depth basis: {ap:?}");
    let ae = rule_names(&r.bases.ae);
    assert!(ae.iter().any(|n| n == MIN_AE_ROW), "width basis: {ae:?}");
}

#[test]
fn the_envelope_engagement_floors_read_the_rows_p13a() {
    let mut op = rs_cam_core::compute::fixture_values::fixture_op(OperationType::Pocket);
    let machine = MachineProfile::generic_wood_router();
    let material = softwood();
    let tool = flat();
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
        modulation: rs_cam_core::dressup::feed_modulation::RunModulation::Off,
    })
    .unwrap_or_else(|e| panic!("the Pocket envelope is backed: {e}"));
    let floors: Vec<Vec<String>> = env
        .boundary
        .iter()
        .filter(|l| l.kind == LimitKind::EngagementFloor)
        .map(|l| rule_names(&l.basis))
        .collect();
    assert_eq!(floors.len(), 2, "the depth floor and the width floor");
    assert!(
        floors.iter().any(|n| n.iter().any(|x| x == MIN_AP_ROW)),
        "{floors:?}"
    );
    assert!(
        floors.iter().any(|n| n.iter().any(|x| x == MIN_AE_ROW)),
        "{floors:?}"
    );
}
