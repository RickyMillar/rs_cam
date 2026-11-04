//! Ruling D12 (feeds contract P7 commit A2): centre cutting, and the tip of
//! a V-bit, are required inputs.
//!
//! The operator: "can we just make them have to state it?", then "Stated
//! only". An op whose tool holds `centre_cutting` `Unknown` or `Estimated`
//! does not generate: it stops with a reason that names the tool and the
//! fact, as an unset depth does (S11, ruling Q10). `Stated` and `Catalogue`
//! count. A V-bit also needs its `v_tip_diameter`; a tool that is not a
//! V-bit needs no tip. The diagnostics give the same stop before
//! generation (`precondition.not_set`, a missing input).
//!
//! Red: make `compute::not_set::tool_facts_not_set` give `None`. The
//! Unknown and Estimated cases then generate.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::op_fixtures::{CaseSpec, Fixture, Frame, Prior, case_session};
use rs_cam_core::compute::OperationError;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::not_set::NotSetKind;
use rs_cam_core::compute::tool_config::{CentreCutting, ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::diagnostics::ids;
use rs_cam_core::session::SessionError;

/// `op` on its fixture with `tool`: the generation outcome and the
/// diagnostics of the op.
fn run(
    op: OperationType,
    tool: ToolConfig,
) -> (
    Result<(), SessionError>,
    Vec<rs_cam_core::diagnostics::Diagnostic>,
) {
    let (mut session, index) = case_session(CaseSpec {
        name: "d12",
        op: OperationConfig::new_default(op),
        prior: Prior::None,
        stock_source: StockSource::Fresh,
        frame: Frame::TopUp,
        fixture: Fixture::Standard,
        tool: Some(tool),
    });
    let diagnostics = session.diagnose_toolpath(index).expect("the diagnostics");
    let outcome = session
        .generate_toolpath(index, &AtomicBool::new(false))
        .map(|_| ());
    (outcome, diagnostics)
}

/// A production tool: `new_default` states neither fact (D12: the operator
/// must).
fn production_tool(tool_type: ToolType) -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId::from_raw(0), tool_type);
    tool.tool_number = 6;
    tool.name = "test end mill".to_owned();
    tool
}

/// The stop names the fact keys `fields`, the tool, and the fix.
#[track_caller]
fn assert_stops(
    outcome: &Result<(), SessionError>,
    diagnostics: &[rs_cam_core::diagnostics::Diagnostic],
    fields: &[&str],
    sentence: &str,
) {
    match outcome {
        Err(SessionError::Operation(OperationError::NotSet(not_set))) => {
            assert_eq!(not_set.kind, NotSetKind::ToolFact, "{not_set:?}");
            assert_eq!(not_set.fields, fields, "{not_set:?}");
            let text = not_set.to_string();
            assert!(
                text.contains(sentence) && text.contains("T6"),
                "the reason names the tool and the fact: {text}"
            );
        }
        Err(other) => panic_with(&format!("a NotSet stop, not {other:?}")),
        Ok(()) => panic_with("the op must stop"),
    }
    let row = diagnostics
        .iter()
        .find(|d| d.id.0 == ids::PRECOND_NOT_SET)
        .expect("the precondition row");
    assert!(
        row.message.contains(sentence),
        "the diagnostics give the same stop: {}",
        row.message
    );
}

#[expect(
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]
#[track_caller]
fn panic_with(text: &str) -> ! {
    panic!("{text}")
}

#[test]
fn an_unknown_or_estimated_centre_cutting_stops() {
    let unknown = production_tool(ToolType::EndMill);
    let (outcome, diagnostics) = run(OperationType::Pocket, unknown.clone());
    assert_stops(
        &outcome,
        &diagnostics,
        &["centre_cutting"],
        "state centre cutting for tool T6",
    );

    let mut estimated = unknown;
    estimated.set_centre_cutting(ToolFact::estimated(CentreCutting::Yes, "a guess"));
    let (outcome, diagnostics) = run(OperationType::Pocket, estimated);
    assert_stops(
        &outcome,
        &diagnostics,
        &["centre_cutting"],
        "state centre cutting for tool T6",
    );
}

#[test]
fn a_stated_or_catalogue_centre_cutting_generates() {
    for fact in [
        ToolFact::Stated(CentreCutting::Yes),
        ToolFact::Catalogue {
            value: CentreCutting::Yes,
            source: "a vendor row".to_owned(),
        },
    ] {
        let mut tool = production_tool(ToolType::EndMill);
        tool.set_centre_cutting(fact.clone());
        let (outcome, diagnostics) = run(OperationType::Pocket, tool);
        assert!(outcome.is_ok(), "{fact:?}: {outcome:?}");
        assert!(
            !diagnostics.iter().any(|d| d.id.0 == ids::PRECOND_NOT_SET),
            "{fact:?}: no stop row"
        );
    }
}

/// A tool that is not a V-bit needs no tip, whatever its tip fact holds.
#[test]
fn a_tool_that_is_not_a_v_bit_needs_no_tip() {
    let mut tool = production_tool(ToolType::EndMill);
    tool.set_centre_cutting(ToolFact::Stated(CentreCutting::Yes));
    assert!(tool.v_tip_diameter_fact().is_unknown());
    let (outcome, _) = run(OperationType::Pocket, tool);
    assert!(outcome.is_ok(), "{outcome:?}");
}

#[test]
fn a_v_bit_with_an_unknown_tip_stops() {
    let mut tool = production_tool(ToolType::VBit);
    tool.set_centre_cutting(ToolFact::Stated(CentreCutting::Yes));
    let (outcome, diagnostics) = run(OperationType::VCarve, tool.clone());
    assert_stops(
        &outcome,
        &diagnostics,
        &["v_tip_diameter"],
        "state the tip diameter of V-bit T6",
    );

    // Both facts unknown: one stop names both.
    let mut both = tool.clone();
    both.set_centre_cutting(ToolFact::Unknown);
    let (outcome, diagnostics) = run(OperationType::VCarve, both);
    assert_stops(
        &outcome,
        &diagnostics,
        &["centre_cutting", "v_tip_diameter"],
        "state the tip diameter of V-bit T6",
    );

    tool.set_v_tip_diameter(ToolFact::Stated(0.0));
    let (outcome, _) = run(OperationType::VCarve, tool);
    assert!(outcome.is_ok(), "a stated tip generates: {outcome:?}");
}
