//! F189 (tech-debt review 2026-10-19): the Warnings tab reads every triage
//! bucket.
//!
//! The triage class was a choice of `Vec`, and the tab read `safety` and
//! `actions` only. The capped advisories reached the verdict line and no
//! row. Each finding now carries its `TriageClass`, the tab walks
//! `SimulationTriage::findings` and matches the class in one exhaustive
//! match (`triage_row_role`), and the badge counts by class.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use rs_cam_core::diagnostics::{
    Category, Confidence, Diagnostic, DiagnosticId, DiagnosticState, Scope, Severity, Source, ids,
};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::stock::sim_triage::{
    Bounded, DedupKey, Finding, SimulationTriage, TriageClass, WorstEvidence,
};
use rs_cam_viz::ui::components::{Role, TabBadge};
use rs_cam_viz::ui::sim_diagnostics::{triage_row_role, warnings_badge_for};

fn finding(class: TriageClass, id: &str, message: &str) -> Finding {
    Finding {
        class,
        dedup_key: DedupKey {
            id: DiagnosticId::from(id),
            toolpath_id: Some(ToolpathId::from_raw(1)),
            region: None,
            semantic_item_id: None,
            bucket: [0, 0, 0],
        },
        diagnostic: Diagnostic {
            id: DiagnosticId::from(id),
            scope: Scope::Toolpath {
                id: ToolpathId::from_raw(1),
            },
            category: Category::Efficiency,
            severity: Severity::Hint,
            confidence: Confidence::Approximate,
            state: DiagnosticState::Current,
            source: Source::Simulation,
            message: message.to_owned(),
            evidence: None,
            fix: None,
            supersedes: vec![],
            suppressed_diagnostics: vec![],
        },
        occurrences: 1,
        worst: WorstEvidence::default(),
    }
}

fn one_of_each() -> SimulationTriage {
    SimulationTriage {
        safety: vec![finding(
            TriageClass::Safety,
            ids::PROJECT_RAPID_COLLISION,
            "rapid",
        )],
        actions: vec![finding(
            TriageClass::Action,
            ids::PROJECT_GENERATED_EMPTY,
            "empty",
        )],
        advisories: Bounded {
            items: vec![finding(
                TriageClass::Advisory,
                ids::PROJECT_AIR_CUT_HIGH,
                "air",
            )],
            truncated: false,
            total_matching: 1,
        },
        ..SimulationTriage::default()
    }
}

#[test]
fn the_warnings_rows_show_every_class_f189() {
    let triage = one_of_each();
    let rows: Vec<(Role, &str)> = triage
        .findings()
        .map(|f| (triage_row_role(f.class), f.diagnostic.message.as_str()))
        .collect();
    assert_eq!(
        rows,
        vec![
            (Role::Danger, "rapid"),
            (Role::Caution, "empty"),
            (Role::Info, "air"),
        ],
        "the tab must list safety, then actions, then the advisories"
    );
}

#[test]
fn the_badge_counts_safety_and_actions_by_class_f189() {
    let triage = one_of_each();
    assert_eq!(
        warnings_badge_for(&triage),
        Some(TabBadge::Count(2, Role::Danger)),
        "the badge counts what to act on; the advisory is a row, not a count"
    );
    assert_eq!(warnings_badge_for(&SimulationTriage::default()), None);
}
