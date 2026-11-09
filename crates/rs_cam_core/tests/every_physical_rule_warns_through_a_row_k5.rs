//! K5 (feeds contract P7): every physical rule of `physical_rules!` warns
//! through a `diag_ids!` row and a presentation row.
//!
//! For each rule of `PhysicalRule::ALL`, each stage that is `Warn` needs
//! the K6 row of the rule's id with `Consequence::Warn` (ruling Q6: no rule
//! refuses), and the W1 `TABLE` row that gives the face and the anchor. A
//! `None` stage states a reason. The table has the four rules of the card
//! (ruling D12 deleted PlungeKnown).
//!
//! Red: remove the `diag_ids!` row or the `TABLE` row of a rule's id, or
//! write a rule whose id is an `INFO` row.

use rs_cam_core::compute::feasibility::{PhysicalRule, Stage};
use rs_cam_core::diagnostics::consequence::{Consequence, RowKind};
use rs_cam_core::diagnostics::{ids, presentation};

#[test]
fn every_warn_stage_has_a_warning_row_and_a_face() {
    assert_eq!(
        PhysicalRule::ALL,
        &[
            PhysicalRule::CentreCut,
            PhysicalRule::HelixCore,
            PhysicalRule::ReachFlute,
            PhysicalRule::ReachHolder,
            PhysicalRule::VTipFlat,
        ],
        "the four rules of the card after ruling D12, and HelixCore (G-SIMCORE, P22-0b), in row order"
    );
    for rule in PhysicalRule::ALL {
        let stages = rule.stages();
        let all = [stages.plan, stages.gen_stage, stages.sim, stages.export];
        let warns = all.iter().filter(|s| **s == Stage::Warn).count();
        assert!(warns > 0, "{rule:?}: a rule warns at some stage");
        for stage in all {
            match stage {
                Stage::None(reason) => {
                    assert!(!reason.trim().is_empty(), "{rule:?}: a None stage says why");
                }
                Stage::Warn => {
                    assert!(
                        matches!(
                            ids::row_kind(stages.id),
                            Some(RowKind::Finding(Consequence::Warn(_)))
                        ),
                        "{rule:?}: the id {} has no warning row",
                        stages.id
                    );
                    assert!(
                        presentation::TABLE
                            .iter()
                            .any(|(id, _, _)| *id == stages.id),
                        "{rule:?}: the id {} has no presentation row",
                        stages.id
                    );
                }
                Stage::Clamp | Stage::Model | Stage::Measure => {}
            }
        }
    }
}

/// Each rule has its own id: a finding names one rule.
#[test]
fn each_rule_has_its_own_id() {
    let mut seen: Vec<&str> = Vec::new();
    for rule in PhysicalRule::ALL {
        let id = rule.stages().id;
        assert!(!seen.contains(&id), "{rule:?}: the id {id} is used twice");
        seen.push(id);
    }
}
