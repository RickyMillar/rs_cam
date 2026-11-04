//! CONTRACT K5 (feeds contract P7, change 8): the physical rule rows reach
//! every surface with no check of their own.
//!
//! Op 3 of the blind review (`rs_cam_core/tests/common/k5_cases.rs`) warns
//! ReachFlute and ReachHolder. The GUI export door, its G-code
//! header, the MCP `export_gcode` reply, the diagnostics list (MCP
//! `get_toolpath_diagnostics`, the GUI inspector) and the lineage JSON
//! (MCP `get_toolpath_lineage`, CLI `summary.json`) name the same ids. The
//! CLI export door is `export_gcode_checked`, which the core sentries read.
//!
//! `cargo test -p rs_cam_viz -q --test the_k5_rule_rows_reach_every_surface_k5`

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

#[path = "../../rs_cam_core/tests/common/k5_cases.rs"]
mod k5_cases;

use rs_cam_core::diagnostics::ids;
use rs_cam_viz::io::export::{export_gcode_from_session_reporting, export_reply_text};
use rs_cam_viz::state::runtime::GuiState;
use rs_cam_viz::state::simulation::SimulationState;

/// The ids op 3 gives.
const IDS: [&str; 2] = [ids::TOOL_REACH_PAST_FLUTE, ids::TOOL_REACH_TO_HOLDER];

#[test]
fn every_surface_names_the_rule_rows_k5() {
    let session = k5_cases::op3_generated(k5_cases::FLAT_6_STICKOUT_MM);

    // The GUI door (the Export button, the wizard, the MCP tool).
    let exported =
        export_gcode_from_session_reporting(&session, &GuiState::new(), &SimulationState::new())
            .expect("no rule refuses the GUI export (Q6)");
    assert!(!exported.gcode.is_empty(), "the GUI door writes a program");
    let reply = export_reply_text("G-code exported to /tmp/k5.nc".to_owned(), &exported);
    let diagnostics = session.diagnose_toolpath(0).expect("the diagnostics");
    let lineage = session
        .result_at(0)
        .expect("a result")
        .lineage
        .to_json()
        .to_string();
    for id in IDS {
        assert!(
            exported.warnings.iter().any(|w| w.id == id),
            "the GUI export does not name {id}: {:?}",
            exported.warnings
        );
        assert!(
            exported
                .gcode
                .lines()
                .any(|line| line.contains("WARNING") && line.contains(id)),
            "the GUI program header does not name {id}"
        );
        assert!(reply.contains(id), "the MCP reply does not name {id}");
        assert!(
            diagnostics.iter().any(|d| d.id.0 == id),
            "the diagnostics do not name {id}"
        );
        assert!(
            lineage.contains(id),
            "the lineage JSON does not name {id}: {lineage}"
        );
    }
}
