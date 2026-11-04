//! K8 (feeds contract P5b, ruling Q7): every generated toolpath carries its
//! lineage.
//!
//! # The claim
//!
//! For each op of `OperationType::ALL`, the G-OPGOLDEN fresh arm generates
//! through the session door. The result holds a `Lineage` whose:
//!
//! - `cut` equals the `CutSpec` of the stored op;
//! - stored feed equals the op's feed, with the stored stamp;
//! - tool facts name the tool's diameter;
//! - entry records equal `Produced::entries`;
//! - findings name each generation finding id once.
//!
//! A pick (`ApplyFeeds { pick: Proposal }`) on the Pocket gives a lineage
//! with a `PickRecord`; a speeds-only apply gives none.
//!
//! The compiler holds the rest: `ToolpathComputeResult::lineage` is a
//! required field and `Lineage` has no `Default` (the `compile_fail`
//! doctests of `trace::lineage`).
//!
//! Red before P5b: `trace::lineage` does not exist.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::op_fixtures::{CaseSpec, Fixture, Frame, Prior, case_session};
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::session::{ApplyFeedsArgs, Command};
use rs_cam_core::trace::lineage::ToolFactKind;

fn fresh_session(op: OperationType) -> (rs_cam_core::session::ProjectSession, usize) {
    case_session(CaseSpec {
        name: &format!("{}/fresh", op.kind_str()),
        op: OperationConfig::new_default(op),
        prior: Prior::None,
        stock_source: StockSource::Fresh,
        frame: Frame::TopUp,
        fixture: Fixture::Standard,
        tool: None,
    })
}

#[test]
fn every_generated_toolpath_has_a_lineage_k8() {
    let mut failures = Vec::new();
    let mut generated = 0usize;
    for &op in OperationType::ALL {
        let (mut session, index) = fresh_session(op);
        let cancel = AtomicBool::new(false);
        let Ok(result) = session.generate_toolpath(index, &cancel) else {
            failures.push(format!("{}: the fresh arm refused", op.kind_str()));
            continue;
        };
        generated += 1;
        let lineage = std::sync::Arc::clone(&result.lineage);
        let entries = result.annotated().produced.entries.clone();
        let stats = result.stats.clone();
        let tc = session
            .toolpath_configs()
            .get(index)
            .expect("the case op is stored");
        let label = op.kind_str();
        if lineage.cut != tc.operation.cut_spec() {
            failures.push(format!("{label}: the lineage cut is not the op's CutSpec"));
        }
        if lineage.speeds.feed_rate.value != tc.operation.feed_rate() {
            failures.push(format!("{label}: the lineage feed is not the stored feed"));
        }
        if lineage.speeds.feed_rate.basis != tc.feeds_provenance.feed_rate {
            failures.push(format!(
                "{label}: the lineage feed stamp is not the stored stamp"
            ));
        }
        let diameter = lineage
            .tool_facts
            .iter()
            .find(|f| f.fact == ToolFactKind::Diameter)
            .map(|f| f.value);
        let tool = session
            .toolpath_tool(tc)
            .expect("the case op has a tool")
            .diameter;
        if diameter != Some(tool) {
            failures.push(format!(
                "{label}: the diameter fact {diameter:?} is not {tool}"
            ));
        }
        if lineage.entry.records != entries {
            failures.push(format!(
                "{label}: the entry records are not Produced::entries"
            ));
        }
        if lineage.entry.contract != op.registry_entry().entry {
            failures.push(format!("{label}: the entry contract is not the row's"));
        }
        let mut ids: Vec<String> =
            rs_cam_core::diagnostics::adapters::from_generation::diagnostics_from_generation(
                tc.id, &stats,
            )
            .into_iter()
            .map(|d| d.id.0)
            .collect();
        ids.dedup();
        let names: Vec<String> = lineage.findings.iter().map(|f| f.id.clone()).collect();
        if names != ids {
            failures.push(format!(
                "{label}: findings {names:?}, generation gives {ids:?}"
            ));
        }
        // K5 (P7): each rule outcome is a row of the rule table whose id
        // warns (ruling Q6: a rule never refuses).
        for rule in &lineage.rules {
            if rs_cam_core::diagnostics::ids::warn_danger(rule.id()).is_none() {
                failures.push(format!("{label}: rule {} does not warn", rule.id()));
            }
        }
    }
    assert_eq!(
        generated,
        OperationType::ALL.len(),
        "non-vacuity: {failures:?}"
    );
    assert!(
        failures.is_empty(),
        "K8: {} lineage failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn a_pick_is_in_the_lineage_and_a_speeds_apply_is_not_k8() {
    let (mut session, index) = fresh_session(OperationType::Pocket);
    let cancel = AtomicBool::new(false);
    let _ = session
        .apply(Command::ApplyFeeds(ApplyFeedsArgs {
            index,
            pick: None,
            explored: None,
        }))
        .expect("a speeds apply on the Pocket");
    let speeds = session
        .generate_toolpath(index, &cancel)
        .expect("the Pocket generates");
    assert!(speeds.lineage.pick.is_none(), "a speeds apply is no pick");
    let _ = session
        .apply(Command::ApplyFeeds(ApplyFeedsArgs {
            index,
            pick: Some(rs_cam_core::feeds::envelope::PickRequest::Proposal),
            explored: None,
        }))
        .expect("a proposal pick on the Pocket");
    let picked = session
        .generate_toolpath(index, &cancel)
        .expect("the Pocket generates");
    let pick = picked.lineage.pick.expect("a pick is in the lineage");
    let tc = session
        .toolpath_configs()
        .get(index)
        .expect("the case op is stored");
    assert_eq!(pick.ap_mm, tc.operation.depth_per_pass());
    assert_eq!(pick.ae_mm, tc.operation.stepover());
}
