//! K4 (feeds contract P3): every op states how it enters material, and the
//! entries it emits are of the kinds its contract states.
//!
//! # The claim
//!
//! For each op of `OperationType::ALL`, the G-OPGOLDEN fixture arms
//! (`fresh`, and `rest` for an op with a rest rule) generate through the
//! session door. `Produced::entries` holds one `EntryRecord` per entry
//! (`JobPhases::run_stages`). Each record kind is in the set of the op's
//! contract (`EntryContract::record_kinds`):
//!
//! - `Dressup`: `allowed`, `own` and the plunge of `none_is`.
//! - `Planner`: `kinds`.
//! - `DescentIsCut`: no record. Drill and Pin Drill emit `Drilling` moves;
//!   Bore emits its cut helix (`ClearingCut`).
//! - `StraightPlunge`: `Plunge` only.
//!
//! G-ISOCLIPENTRY is a session stage, not a contract kind (P3 Appendix A,
//! "+ G-ISO"): on a remaining-stock pass of an op with a surface probe
//! (`Capabilities::entry_probe`), `optimize_entry_descents` can turn a
//! plunge into the rest ramp ladder. So a `rest` arm of such an op may also
//! hold a `Ramp` record.
//!
//! Non-vacuity: every op whose contract is not `DescentIsCut` has at least
//! one record over its arms, and the Pocket fresh arm (Roughing: Ramp)
//! holds a `Ramp` record.
//!
//! Red before P3: `EntryContract` and `Produced::entries` do not exist.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;

use common::op_fixtures::{CaseSpec, Fixture, Frame, Prior, case_session, run_prior};
use rs_cam_core::compute::catalog::{
    EntryContract, EntryKind, EntryRecord, EntrySet, OperationConfig, OperationType, RestUse,
};
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::toolpath::MoveIntent;

/// One generated arm: the entry records and the move intents.
struct Arm {
    records: Vec<EntryRecord>,
    intents: Vec<MoveIntent>,
}

fn generate(op: OperationType, rest: bool) -> Option<Arm> {
    let (mut session, index) = case_session(CaseSpec {
        name: &format!("{}/{}", op.kind_str(), if rest { "rest" } else { "fresh" }),
        op: OperationConfig::new_default(op),
        prior: if rest { Prior::RestPrior } else { Prior::None },
        stock_source: if rest {
            StockSource::FromRemainingStock
        } else {
            StockSource::Fresh
        },
        frame: Frame::TopUp,
        fixture: Fixture::Standard,
        tool: None,
    });
    if rest {
        run_prior(&mut session);
    }
    let cancel = AtomicBool::new(false);
    let result = session.generate_toolpath(index, &cancel).ok()?;
    Some(Arm {
        records: result.annotated().produced.entries.clone(),
        intents: result.toolpath().moves.iter().map(|m| m.intent).collect(),
    })
}

#[test]
fn every_op_states_its_entry_contract_k4() {
    let mut failures = Vec::new();
    let mut seen: BTreeMap<&str, Vec<EntryKind>> = BTreeMap::new();
    for &op in OperationType::ALL {
        let contract = op.registry_entry().entry;
        let has_surface_probe = OperationConfig::new_default(op)
            .capabilities()
            .entry_probe
            .leave_mm()
            .is_some();
        let mut arms = vec![false];
        if op.capabilities().rest_use != RestUse::None {
            arms.push(true);
        }
        let mut any_record = false;
        for rest in arms {
            let label = format!("{}/{}", op.kind_str(), if rest { "rest" } else { "fresh" });
            // G-ISOCLIPENTRY (module doc): the rest ramp of the session.
            let allowed = if rest && has_surface_probe {
                contract
                    .record_kinds()
                    .union(EntrySet::of(&[EntryKind::Ramp]))
            } else {
                contract.record_kinds()
            };
            let Some(arm) = generate(op, rest) else {
                failures.push(format!("{label}: the arm refused"));
                continue;
            };
            any_record |= !arm.records.is_empty();
            let kinds = seen.entry(op.kind_str()).or_default();
            for record in &arm.records {
                if !kinds.contains(&record.kind()) {
                    kinds.push(record.kind());
                }
                if !allowed.contains(record.kind()) {
                    failures.push(format!(
                        "{label}: an entry of kind {:?} at {:?}; the contract {contract:?} allows {:?}",
                        record.kind(),
                        record.to(),
                        allowed.kinds().collect::<Vec<_>>()
                    ));
                }
            }
            match contract {
                EntryContract::DescentIsCut => {
                    let cuts = arm
                        .intents
                        .iter()
                        .any(|i| matches!(i, MoveIntent::Drilling | MoveIntent::ClearingCut));
                    if !cuts {
                        failures.push(format!("{label}: a descent-is-cut op emits no descent cut"));
                    }
                }
                EntryContract::Dressup { .. }
                | EntryContract::Planner { .. }
                | EntryContract::StraightPlunge { .. } => {}
            }
        }
        let needs_a_record = match contract {
            EntryContract::DescentIsCut => false,
            EntryContract::Dressup { .. }
            | EntryContract::Planner { .. }
            | EntryContract::StraightPlunge { .. } => true,
        };
        if needs_a_record && !any_record {
            failures.push(format!(
                "{}: no entry record over its arms (non-vacuity)",
                op.kind_str()
            ));
        }
    }
    // Non-vacuity of the kind: the Pocket ramps (Roughing role default).
    let pocket = generate(OperationType::Pocket, false).expect("the Pocket generates");
    if !pocket.records.iter().any(|r| r.kind() == EntryKind::Ramp) {
        failures.push("pocket/fresh: no Ramp entry record".to_owned());
    }
    assert!(
        failures.is_empty(),
        "K4: {} entry contract failures:\n{}\nkinds seen per op: {seen:?}",
        failures.len(),
        failures.join("\n")
    );
}
