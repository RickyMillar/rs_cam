//! K5 VTipFlat (feeds contract P7): the flat tip of a V-bit is modelled
//! where the record states it, and a cone that disagrees with the flute is
//! named.
//!
//! The case is op 9 of the blind review (report 06 (a2), research C 1.3):
//! the V20, 3.175 mm, 20 deg, with a 0.3 mm flat tip that its name states.
//! Its record has `cutting_length = 8.152467616`: the cone height of the
//! 0.3 mm flat. A sharp cone of the same V is higher. Before K5 the tool
//! record had no tip, and `build_cutter` built every V-bit sharp.
//!
//! - Plan (Model): `build_cutter` and `geometry_hint` read the tip from
//!   `planning()`; a stated 0.3 gives `flat_tip_diameter() == 0.3`.
//! - Generation: a stated tip whose cone height disagrees with the flute
//!   length warns (`tool.v_tip_unmodelled`).
//! - Only a `ToolType::VBit` has the rule (lead clarification): an end mill
//!   on a V-capable op gives no row, whatever its tip fact holds.
//! - Ruling D12: an `Unknown` tip is a missing input (commit A2): the op
//!   stops, so no row exists.
//!
//! Both cone heights come from `(d/2 - tip/2) / tan(half angle)` in the
//! test; no typed result. Ruling Q6: generation and export give output.
//!
//! Red: make the cone predicate `false` (the mismatch case has no row), or
//! drop the `ToolType::VBit` guard (the end mill case gets a row).

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use common::k5_cases::{V20_TIP_MM, export, export_names, rule_texts, v20};
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::feasibility::CONE_HEIGHT_PRINT_TOLERANCE_MM;
use rs_cam_core::compute::tool_config::{CentreCutting, ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::diagnostics::ids;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::tool::MillingCutter;

/// The cone height (mm) of a V-bit of diameter `d`, included angle `angle`
/// (deg) and flat tip `tip` (mm).
fn cone_height(d: f64, angle: f64, tip: f64) -> f64 {
    (d / 2.0 - tip / 2.0) / (angle / 2.0).to_radians().tan()
}

/// `op` on the flat fixture with `tool`, generated.
fn generated(op: OperationType, tool: ToolConfig) -> (ProjectSession, usize) {
    let (mut session, index) = common::op_fixtures::case_session(common::op_fixtures::CaseSpec {
        name: "V rivers and lakes",
        op: OperationConfig::new_default(op),
        prior: common::op_fixtures::Prior::None,
        stock_source: StockSource::Fresh,
        frame: common::op_fixtures::Frame::TopUp,
        fixture: common::op_fixtures::Fixture::Standard,
        tool: Some(tool),
    });
    common::session::generate(&mut session, index);
    (session, index)
}

#[test]
fn a_stated_tip_is_modelled_and_agrees_with_the_flute() {
    let tool = v20();
    let built = rs_cam_core::compute::cutter::build_cutter(&tool).expect("a valid V-bit");
    assert_eq!(built.flat_tip_diameter(), V20_TIP_MM, "the cutter model");
    let hint = tool.geometry_hint();
    assert!(
        matches!(
            hint,
            rs_cam_core::feeds::ToolGeometryHint::VBit { tip_diameter, .. }
                if tip_diameter == V20_TIP_MM
        ),
        "the feeds hint carries the tip: {hint:?}"
    );
    // The record's flute is the cone height of the 0.3 mm flat.
    let flat_cone = cone_height(tool.diameter, tool.included_angle, V20_TIP_MM);
    assert!(
        (tool.cutting_length_planning_mm() - flat_cone).abs() <= CONE_HEIGHT_PRINT_TOLERANCE_MM,
        "the V20 flute {} is the 0.3 mm cone {flat_cone}",
        tool.cutting_length_planning_mm()
    );
    let (session, index) = generated(OperationType::VCarve, tool);
    assert!(
        !session
            .result_at(index)
            .expect("a result")
            .toolpath()
            .moves
            .is_empty(),
        "the V-Carve generates (Q6)"
    );
    assert!(
        rule_texts(&session, index, ids::TOOL_V_TIP_UNMODELLED).is_empty(),
        "a stated tip that agrees with the flute gives no row"
    );
}

#[test]
fn a_tip_whose_cone_disagrees_with_the_flute_warns() {
    let mut tool = v20();
    // A sharp point stated on the V20 record: its cone is higher than the
    // record's flute, which is the cone of the 0.3 mm flat.
    tool.set_v_tip_diameter(ToolFact::Stated(0.0));
    let sharp_cone = cone_height(tool.diameter, tool.included_angle, 0.0);
    let flat_cone = cone_height(tool.diameter, tool.included_angle, V20_TIP_MM);
    assert!(sharp_cone > flat_cone, "a sharp cone is higher");
    let (session, index) = generated(OperationType::VCarve, tool);
    let row = session
        .result_at(index)
        .and_then(|r| r.stats.feasibility.as_ref())
        .and_then(|f| {
            f.rows()
                .iter()
                .find(|r| r.id() == ids::TOOL_V_TIP_UNMODELLED)
        })
        .expect("the cone-height row");
    let limit = row.limit().expect("the cone height of the stated tip");
    assert!(
        (limit - sharp_cone).abs() < 1e-9,
        "the limit is the sharp cone {sharp_cone}, not {limit}"
    );
    assert!(
        (row.measured() - flat_cone).abs() <= CONE_HEIGHT_PRINT_TOLERANCE_MM,
        "the measure is the record's flute"
    );
    assert!(
        row.text().contains("V tip 0 mm, stated") && row.text().contains("flute"),
        "the row names both facts: {}",
        row.text()
    );
    let exported = export(&session);
    assert!(
        !exported.gcode.is_empty(),
        "the export writes a program (Q6)"
    );
    assert!(
        export_names(&exported, ids::TOOL_V_TIP_UNMODELLED),
        "the export and the header name the warning: {:?}",
        exported.warnings
    );
}

/// Ruling D12: an `Unknown` tip is a missing input (commit A2), not a
/// finding of this rule: the op stops before the rule runs, so no row
/// exists. The stop itself is the D12 sentry's
/// (`an_unstated_centre_cutting_stops_generation_d12`).
#[test]
fn an_unknown_tip_stops_and_gives_no_row() {
    let mut tool = v20();
    tool.set_v_tip_diameter(ToolFact::Unknown);
    let (mut session, index) = common::op_fixtures::case_session(common::op_fixtures::CaseSpec {
        name: "V rivers and lakes",
        op: OperationConfig::new_default(OperationType::VCarve),
        prior: common::op_fixtures::Prior::None,
        stock_source: StockSource::Fresh,
        frame: common::op_fixtures::Frame::TopUp,
        fixture: common::op_fixtures::Fixture::Standard,
        tool: Some(tool),
    });
    let outcome = session.generate_toolpath(index, &std::sync::atomic::AtomicBool::new(false));
    assert!(outcome.is_err(), "an Unknown tip stops the op (D12)");
    assert!(session.result_at(index).is_none(), "no result, so no row");
}

/// Only a V-bit has the rule. An end mill on Trace (an op that takes a
/// V-bit too) gives no row, even with a tip fact whose cone would disagree
/// with its flute.
#[test]
fn a_tool_that_is_not_a_v_bit_gives_no_row() {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.set_centre_cutting(ToolFact::Stated(CentreCutting::Yes));
    tool.set_v_tip_diameter(ToolFact::Stated(0.0));
    let (session, index) = generated(OperationType::Trace, tool);
    assert!(
        !session
            .result_at(index)
            .expect("a result")
            .toolpath()
            .moves
            .is_empty(),
        "the trace generates"
    );
    assert!(
        rule_texts(&session, index, ids::TOOL_V_TIP_UNMODELLED).is_empty(),
        "an end mill has no V tip row"
    );
}
