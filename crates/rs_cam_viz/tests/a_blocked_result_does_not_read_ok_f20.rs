//! F20 (flow review FR-1, SAFETY visibility): a toolpath whose current
//! result holds a BLOCKING finding does not read "OK".
//!
//! # The defect
//!
//! A Profile that the model gouge guard blocked (`safety.cuts_into_model`)
//! showed a green dot in the toolpath list, and "Done" in the inspector
//! header. Only the export refused.
//!
//! # The claim
//!
//! 1. `blocking_finding` reads the stored stats through the one generation
//!    adapter (`diagnostics_from_generation`) and gives the blocking
//!    finding, with its message (the message holds the fix).
//! 2. The row dot of a current result with that finding reads
//!    `BLOCKED` in the danger role, with the message on hover. A current
//!    result with no blocking finding still reads `OK`.
//! 3. A clean measurement (`gouging == 0`) gives no blocking finding.
//!
//! Red proof: make `blocking_finding` return `None`. Claims 1 and 2 fail.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test a_blocked_result_does_not_read_ok_f20
//! ```

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::ModelGougeRule;
use rs_cam_core::compute::model_gouge::ModelGougeFinding;
use rs_cam_core::compute::toolpath_stats::ToolpathStats;
use rs_cam_core::diagnostics::{Severity, ids};
use rs_cam_viz::state::freshness::FreshnessState;
use rs_cam_viz::ui::components::Role;
use rs_cam_viz::ui::toolpath_panel::{BLOCKED_WORD, blocking_finding, row_status};

fn stats(gouging: usize) -> ToolpathStats {
    ToolpathStats {
        model_gouge: Some(ModelGougeFinding {
            rule: ModelGougeRule::Surface {
                fix: "Set the floor to Each picked face",
            },
            judged: 100,
            gouging,
            deepest_mm: if gouging > 0 { 10.0 } else { 0.0 },
            at: [38.9, -2.8, -2.0],
            surface_z: 8.0,
            move_index: 12,
            depth_top_z: None,
        }),
        ..ToolpathStats::default()
    }
}

#[test]
fn a_blocked_result_does_not_read_ok_f20() {
    let id = rs_cam_core::ToolpathId::from_raw(4);

    // 1. The blocking finding, through the generation adapter.
    let gouged = stats(7);
    let finding = blocking_finding(Some(&gouged), id).expect("a gouge is a blocking finding");
    assert_eq!(finding.id.as_str(), ids::SAFETY_CUTS_INTO_MODEL);
    assert_eq!(finding.severity, Severity::Blocking);
    assert!(
        finding
            .message
            .contains("Set the floor to Each picked face"),
        "the message holds the fix: {}",
        finding.message
    );

    // 2. The row dot reads BLOCKED, danger, with the message on hover.
    let current = FreshnessState::Current;
    let (word, role, hover) = row_status(&current, Some(finding.message.as_str()));
    assert_eq!(word, BLOCKED_WORD);
    assert_eq!(role, Role::Danger);
    assert_eq!(hover, Some(finding.message.as_str()));
    let (word, role, _) = row_status(&current, None);
    assert_eq!(
        (word, role),
        ("OK", Role::Ok),
        "a clean result still reads OK"
    );

    // 3. A clean measurement blocks nothing.
    assert!(blocking_finding(Some(&stats(0)), id).is_none());
    assert!(blocking_finding(None, id).is_none());
}
