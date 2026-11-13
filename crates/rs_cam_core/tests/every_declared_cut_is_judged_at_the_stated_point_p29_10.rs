//! Feeds contract P29-10 10i (lead ruling L1,
//! `planning/feeds_contract_2026-11-03/phases/P29-10i_DESIGN.md`): every
//! declared cut is judged at the stated point.
//!
//! Before 10i only the body had rows at the stated point; a first slot, a
//! slot ring, a planner cap or an entry at the STATED depth and feed had
//! none, and that is the cut the machine makes. The envelope now judges
//! every declared cut at two points with ONE judge
//! (`SafeEnvelope::read_declared_cuts`, the point is its one argument):
//! the proposal (part two of the Suggest drawer, `Subject::Planned`) and
//! the stated cut (part one, `Subject::StatedCut`).
//!
//! - Claim 1: for every op kind on four tools, with the registry entry and
//!   with a ramp entry, an envelope with a stated point holds one stated
//!   cut row for every pair (declared cut, planned limit) that holds a
//!   planned row, and no other stated cut row. Each stated cut row is a
//!   value (`Within`, `Exceeds`) or a named `NoNumber` or `NotApplicable`;
//!   a row the scope does not judge names its row of `EXEMPTIONS`.
//! - Claim 2 (one judge, two points): when the op states the proposal (its
//!   depth, width, feed and speed), every stated cut row equals its planned
//!   row in value, bound and state, except the force budget of a cut that
//!   reads the body chip: the proposal judges it at the lower ceiling of
//!   the curve chip and the pick chip (`judged_ceiling`, P29-7 trap 1), the
//!   stated cut at the stated chip only, so the stated ceiling is at or
//!   above the planned ceiling (a smaller or equal chip).
//! - Claim 3 (Q2 open, lead ruling 2): a stated cut row of the chip
//!   maximum never ranks for the face line (`CheckRow::ranks_for_the_face`);
//!   every other row ranks.
//!
//! `cargo test -p rs_cam_core -q --test every_declared_cut_is_judged_at_the_stated_point_p29_10`

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::collections::BTreeMap;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{DressupConfig, DressupEntryStyle};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::SpindleStrategy;
use rs_cam_core::feeds::envelope::{EnvelopeInput, PickRequest, SafeEnvelope};
use rs_cam_core::feeds::suggest::{NoProject, SuggestContext};
use rs_cam_core::limits::{CheckRow, CheckState, EXEMPTIONS, LimitId, NoNumberReason, Subject};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

fn tool(kind: ToolType, d: f64) -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), kind);
    t.diameter = d;
    t.set_flute_count(ToolFact::Stated(2));
    t.set_cutting_length(ToolFact::Stated((d * 3.0).max(12.0)));
    t.set_shank_diameter(ToolFact::Stated(d.max(3.0)));
    t.shaft_diameter = d.max(3.0);
    t.set_stickout(ToolFact::Stated(t.cutting_length_planning_mm() + 8.0));
    t
}

fn envelope(
    op: &OperationConfig,
    tool: &ToolConfig,
    dressups: Option<&DressupConfig>,
) -> Option<SafeEnvelope> {
    SafeEnvelope::for_cut(EnvelopeInput {
        operation: op,
        tool,
        machine: &MachineProfile::default(),
        material: &Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        lut: rs_cam_core::feeds::embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::no_project(NoProject {
            dressups,
            ..NoProject::NONE
        }),
    })
    .ok()
}

/// Every op kind at its fixture values on four tools, with the registry
/// entry and with a ramp entry: (name, op, tool, dressups).
fn cases() -> Vec<(String, OperationConfig, ToolConfig, Option<DressupConfig>)> {
    let ramp = DressupConfig {
        entry_style: DressupEntryStyle::Ramp,
        ..Default::default()
    };
    let mut out = Vec::new();
    for t in [
        tool(ToolType::EndMill, 3.175),
        tool(ToolType::EndMill, 6.0),
        tool(ToolType::BallNose, 6.0),
        tool(ToolType::VBit, 6.35),
    ] {
        for &op_type in OperationType::ALL {
            let op = rs_cam_core::compute::fixture_values::fixture_op(op_type);
            for (label, dressups) in [("registry", None), ("ramp", Some(ramp.clone()))] {
                out.push((
                    format!("{op_type:?}/{label}/{:?}{}", t.tool_type, t.diameter),
                    op.clone(),
                    t.clone(),
                    dressups,
                ));
            }
        }
    }
    out
}

/// The rows of one subject kind by (cut, limit).
fn by_pair(env: &SafeEnvelope, stated: bool) -> BTreeMap<(&'static str, LimitId), &CheckRow> {
    env.rows()
        .iter()
        .filter(|r| {
            if stated {
                matches!(r.subject(), Subject::StatedCut { .. })
            } else {
                r.is_planned()
            }
        })
        .map(|r| ((r.cut().expect("a cut").label(), r.check().limit), r))
        .collect()
}

#[test]
fn claim_1_every_declared_cut_has_a_row_at_the_stated_point() {
    let mut envelopes = 0;
    let mut rows = 0;
    let mut not_judged = 0;
    for (name, op, t, dressups) in cases() {
        let Some(env) = envelope(&op, &t, dressups.as_ref()) else {
            continue;
        };
        let planned = by_pair(&env, false);
        let stated = by_pair(&env, true);
        if env.stated.is_none() {
            assert!(stated.is_empty(), "{name}: no stated point, no stated cut");
            continue;
        }
        envelopes += 1;
        assert_eq!(
            planned.keys().collect::<Vec<_>>(),
            stated.keys().collect::<Vec<_>>(),
            "{name}: the stated cuts are the planned cuts, limit by limit"
        );
        for row in stated.values() {
            rows += 1;
            match row.check().state {
                CheckState::Within | CheckState::Exceeds | CheckState::NotApplicable(_) => {}
                CheckState::NoNumber(reason) => {
                    assert!(!reason.face().is_empty(), "{name}: a named reason");
                    if reason == NoNumberReason::NotJudgedHere {
                        not_judged += 1;
                        let id = row.exemption().expect("a scope cell names its row");
                        assert!(
                            EXEMPTIONS.iter().any(|e| e.id == id),
                            "{name}: {id} is a row of EXEMPTIONS"
                        );
                    }
                }
            }
        }
    }
    assert!(
        envelopes > 50,
        "non-vacuity: {envelopes} envelopes with a stated point"
    );
    assert!(rows > 1000, "non-vacuity: {rows} stated cut rows");
    assert!(not_judged > 0, "non-vacuity: a scope cell (a drill cycle)");
}

#[test]
fn claim_2_one_judge_at_the_proposal_and_at_the_stated_point() {
    let mut compared = 0;
    let mut force_rows = 0;
    for (name, mut op, t, dressups) in cases() {
        let Some(first) = envelope(&op, &t, dressups.as_ref()) else {
            continue;
        };
        let Some(ap) = first.proposal.get().ap_mm() else {
            continue;
        };
        let ae = first.proposal.get().ae_mm;
        let pick = first.pick(PickRequest::Proposal);
        let r = pick.result();
        if r.rpm.fract() != 0.0 || r.feed_rate_mm_min.is_nan() || r.feed_rate_mm_min <= 0.0 {
            continue;
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a whole positive rpm, checked above"
        )]
        let rpm = r.rpm as u32;
        // The op states the proposal: its depth, width, feed and speed.
        let _ = op.set_depth_per_pass(ap);
        let _ = op.set_stepover(ae);
        op.set_feed_rate(r.feed_rate_mm_min);
        op.set_spindle_rpm(Some(rpm));
        let Some(env) = envelope(&op, &t, dressups.as_ref()) else {
            continue;
        };
        let (Some(stated), Some(proposal_ap)) = (env.stated, env.proposal.get().ap_mm()) else {
            continue;
        };
        let same_point = (stated.ap_mm - proposal_ap).abs() < 1e-9
            && (stated.ae_mm - env.proposal.get().ae_mm).abs() < 1e-9;
        let same_speeds = {
            let again = env.pick(PickRequest::Proposal);
            let q = again.result();
            q.rpm == r.rpm && q.feed_rate_mm_min == r.feed_rate_mm_min
        };
        if !(same_point && same_speeds) {
            continue;
        }
        let planned = by_pair(&env, false);
        let stated_rows = by_pair(&env, true);
        for (pair, p) in &planned {
            let s = stated_rows.get(pair).expect("a stated row of every pair");
            compared += 1;
            if pair.1 == LimitId::ForceBudget && p.own_depth_mm().is_none() {
                // One chip at the stated point, the lower ceiling of two at
                // the proposal: the stated ceiling is at or above it.
                force_rows += 1;
                if let (Some(sb), Some(pb)) = (s.check().bound, p.check().bound) {
                    assert!(
                        sb >= pb - 1e-9,
                        "{name} {pair:?}: the stated ceiling {sb} under the planned {pb}"
                    );
                }
                continue;
            }
            assert_eq!(
                (s.check().value, s.check().bound, s.check().state),
                (p.check().value, p.check().bound, p.check().state),
                "{name} {pair:?}: the stated cut at the proposal is the planned cut"
            );
        }
    }
    assert!(compared > 200, "non-vacuity: {compared} compared pairs");
    assert!(force_rows > 0, "non-vacuity: force-budget rows");
}

#[test]
fn claim_3_a_stated_chip_maximum_does_not_rank_for_the_face() {
    let mut held_out = 0;
    let mut ranked = 0;
    for (_, op, t, dressups) in cases() {
        let Some(env) = envelope(&op, &t, dressups.as_ref()) else {
            continue;
        };
        for row in env.rows() {
            let stated_chip_max = matches!(row.subject(), Subject::StatedCut { .. })
                && row.check().limit == LimitId::ChipMax;
            assert_eq!(row.ranks_for_the_face(), !stated_chip_max, "{}", row.face());
            if stated_chip_max {
                held_out += 1;
            } else {
                ranked += 1;
            }
        }
    }
    assert!(
        held_out > 0 && ranked > 0,
        "non-vacuity: {held_out} held out, {ranked} ranked"
    );
}
