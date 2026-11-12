//! Feeds contract P23b (`planning/feeds_contract_2026-11-03/phases/P23b_survey.md`
//! section 1): MCP `get_suggest_rationale` prints the Suggest feed and
//! plunge beside the RPM on every op, with or without a power figure.
//!
//! Before P23b the reply printed the feed only inside `basis.shipped_power`.
//! A Project Curve has no power figure before the simulation
//! (`{"unmodeled": "DepthAfterSimulation"}`), so its reply held an RPM
//! (`basis.rpm`, P21c) and no feed (wanaka idx 5). The engine always shipped
//! the three together; only the report was short.
//!
//! The fixture is the wanaka idx 5 cut: a Project Curve with a 6 mm 2-flute
//! flat end mill in white oak, under the constant-chip policy with a ceiling
//! of 24000, on the Shapeoko VFD preset with the machine values that
//! `planning/rivmap_block_job/nz_wanaka_21_operator.toml` prints.
//!
//! The claims:
//!
//! 1. the reply holds an RPM, a feed and a plunge, and `shipped_power` is
//!    unmodeled (non-vacuity: the old feed field is absent);
//! 2. the feed is `chip_load_mm x flutes x rpm x radial_lift` of the same
//!    reply, rounded down to a whole mm/min;
//! 3. the values are the ones an apply writes (the core record
//!    `CutterOpProfile::suggested_speeds`, which the CLI prints too);
//! 4. a Pocket (a modelled power) prints the same fields.
//!
//! Red: drop the `feed_mm_min` insert in `app/mcp/generation.rs`.

#![cfg(feature = "mcp")]
#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::fixture_values::fixture_op;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::profile::{CutterOpProfile, CutterOpProfileInput};
use rs_cam_core::feeds::suggest::{NoProject, SuggestContext};
use rs_cam_core::feeds::{SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

fn wanaka_like() -> MachineProfile {
    MachineProfile {
        max_feed_mm_min: 15_000.0,
        ..MachineProfile::shapeoko_vfd()
    }
    .with_axis_limits([15_000.0, 15_000.0, 2500.0], [1200.0, 1200.0, 550.0])
}

fn flat_6_2f() -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    t.diameter = 6.0;
    t.set_flute_count(ToolFact::Stated(2));
    t.set_cutting_length(ToolFact::Stated(25.0));
    t.set_shank_diameter(ToolFact::Stated(6.0));
    t.shaft_diameter = 6.0;
    t.set_stickout(ToolFact::Stated(30.0));
    t
}

/// A Project Curve 1 mm into the surface.
fn project_curve() -> OperationConfig {
    let mut op = OperationConfig::new_default(OperationType::ProjectCurve);
    let OperationConfig::ProjectCurve(cfg) = &mut op else {
        panic!("the default is a Project Curve");
    };
    cfg.depth = Some(1.0);
    op
}

/// The MCP basis of `op`, and the core speeds record of the same profile.
fn basis_of(
    op: &OperationConfig,
) -> (
    serde_json::Value,
    rs_cam_core::feeds::profile::SuggestedSpeeds,
) {
    let (tool, machine) = (flat_6_2f(), wanaka_like());
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };
    let profile = CutterOpProfile::for_combo(CutterOpProfileInput {
        operation: op,
        tool: &tool,
        machine: &machine,
        material: &material,
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::ConstantChip {
            rpm_ceiling: Some(24_000),
        },
        context: SuggestContext::no_project(NoProject::NONE),
    })
    .expect("the tool builds a cutter");
    assert!(
        profile.feasibility.is_ok(),
        "Suggest serves {:?}: {:?}",
        op.op_type(),
        profile.feasibility
    );
    let speeds = profile
        .suggested_speeds()
        .expect("a served op has its speeds");
    (rs_cam_viz::app::suggest_basis_json(&profile), speeds)
}

fn number(value: &serde_json::Value, key: &str) -> f64 {
    value[key]
        .as_f64()
        .unwrap_or_else(|| panic!("`{key}` is not a number in {value}"))
}

/// The feed of the reply is its own chip x flutes x rpm x lift, rounded
/// down, and the reply equals the core record.
fn assert_speeds(basis: &serde_json::Value, speeds: rs_cam_core::feeds::profile::SuggestedSpeeds) {
    let rpm_block = &basis["rpm"];
    let (rpm, feed, plunge) = (
        number(rpm_block, "rpm"),
        number(rpm_block, "feed_mm_min"),
        number(rpm_block, "plunge_mm_min"),
    );
    assert!(rpm > 0.0 && feed > 0.0 && plunge > 0.0, "{rpm_block}");
    let from_terms = number(rpm_block, "chip_load_mm")
        * number(rpm_block, "flutes")
        * rpm
        * number(rpm_block, "radial_lift");
    assert_eq!(
        feed,
        from_terms.floor(),
        "the feed is chip x flutes x rpm x lift, rounded down: {rpm_block}"
    );
    // Claim 3: the values an apply writes.
    assert_eq!(Some(rpm), speeds.rpm.map(f64::from));
    assert_eq!(Some(feed), speeds.feed_mm_min);
    assert_eq!(Some(plunge), speeds.plunge_mm_min);
}

/// Claims 1 to 3 on the wanaka idx 5 cut.
#[test]
fn a_project_curve_reply_holds_the_rpm_the_feed_and_the_plunge_p23b() {
    let (basis, speeds) = basis_of(&project_curve());
    // Non-vacuity: this op has no power figure, so the old feed field
    // (`shipped_power.feed_mm_min`) is absent.
    assert!(
        basis["shipped_power"]["unmodeled"].is_string(),
        "the Project Curve power is unmodeled: {}",
        basis["shipped_power"]
    );
    assert!(basis["shipped_power"]["feed_mm_min"].is_null());
    assert_speeds(&basis, speeds);
    // The constant-chip RPM: the spindle maximum of the fixture.
    assert_eq!(number(&basis["rpm"], "rpm"), 24_000.0);
    assert_eq!(number(&basis["rpm"], "value"), 24_000.0);
    // A Project Curve cuts the full width: no lift.
    assert_eq!(number(&basis["rpm"], "radial_lift"), 1.0);
}

/// Claim 4: an op with a modelled power prints the same fields.
#[test]
fn a_pocket_reply_holds_the_same_fields_p23b() {
    let (basis, speeds) = basis_of(&fixture_op(OperationType::Pocket));
    assert!(
        basis["shipped_power"]["feed_mm_min"].is_number(),
        "the Pocket power is modelled: {}",
        basis["shipped_power"]
    );
    assert_speeds(&basis, speeds);
}
