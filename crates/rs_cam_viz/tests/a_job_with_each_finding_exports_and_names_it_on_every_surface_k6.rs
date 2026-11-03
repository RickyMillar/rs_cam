//! CONTRACT K6 (feeds contract P2, ruling Q6), the viz twin of the core
//! sentry `a_job_with_each_finding_exports_and_names_it_k6`.
//!
//! The same project (`rs_cam_core/tests/common/k6_project.rs`) goes through
//! the GUI export door and the MCP `export_gcode` reply. Each surface
//! exports a program and names each finding. The MCP export takes no
//! override key, and the GUI pre-flight asks for one click (the Danger
//! style of the Export button) only while a `Danger::Machine` warning
//! stands. The CLI door is `ProjectSession::export_gcode_checked`, which the
//! core sentry and the `rs_cam_cli` test `report_export_checks` cover.
//!
//! `cargo test -p rs_cam_viz -q --test a_job_with_each_finding_exports_and_names_it_on_every_surface_k6`

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

#[path = "../../rs_cam_core/tests/common/k6_project.rs"]
mod k6_project;

use rs_cam_core::diagnostics::{Danger, ids};
use rs_cam_viz::io::export::{export_gcode_from_session_reporting, export_reply_text};
use rs_cam_viz::state::runtime::GuiState;
use rs_cam_viz::state::simulation::SimulationState;

/// The ids each surface names for the project.
const IDS: [&str; 5] = [
    ids::GEOM_CLEARANCE_Z_BELOW_RETRACT_Z,
    ids::SAFETY_CUTS_INTO_MODEL,
    ids::SAFETY_NON_CENTRE_CUTTING_PLUNGE,
    ids::SAFETY_NON_CENTRE_CUTTING_DESCENT,
    ids::EXPORT_OP_LEFT_OUT,
];

#[test]
fn every_surface_exports_and_names_each_finding_k6() {
    let session = k6_project::generated_and_simulated();
    let gui = GuiState::new();
    let sim = SimulationState::new();

    // The GUI door (the Export button, the wizard, the MCP tool).
    let exported = export_gcode_from_session_reporting(&session, &gui, &sim)
        .expect("no finding refuses the GUI export");
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
    }

    // The MCP reply names each finding beside the path it wrote.
    let reply = export_reply_text("G-code exported to /tmp/k6.nc".to_owned(), &exported);
    for id in IDS {
        assert!(
            reply.contains(id),
            "the MCP reply does not name {id}: {reply}"
        );
    }

    // The pre-flight: one chip per danger, and the Danger button while a
    // machine warning stands.
    let chips = rs_cam_viz::ui::preflight::danger_counts(&exported.warnings);
    let count = |danger: Danger| {
        chips
            .iter()
            .find(|(d, _)| *d == danger)
            .map_or(0, |(_, n)| *n)
    };
    assert!(count(Danger::Machine) > 0, "{chips:?}");
    assert!(count(Danger::Tool) > 0, "{chips:?}");
    assert!(count(Danger::Part) > 0, "{chips:?}");
    assert_eq!(
        chips.iter().map(|(_, n)| n).sum::<usize>(),
        exported.warnings.len()
    );
    assert!(rs_cam_viz::ui::preflight::export_asks_a_click(
        &exported.warnings
    ));
    let no_machine: Vec<_> = exported
        .warnings
        .iter()
        .filter(|w| w.danger != Danger::Machine)
        .cloned()
        .collect();
    assert!(!rs_cam_viz::ui::preflight::export_asks_a_click(&no_machine));
}

/// The MCP `export_gcode` tool takes no override key: a request with one is
/// refused at the wire, and a request with only the path exports.
#[cfg(feature = "mcp")]
#[test]
fn the_mcp_export_takes_no_override_key_k6() {
    for key in [
        "accept_exceeded_tool_load",
        "accept_unmodeled_tool_load",
        "accept_previous_geometry",
    ] {
        let request = serde_json::json!({ "path": "/tmp/k6.nc", key: true });
        assert!(
            serde_json::from_value::<rs_cam_mcp::server::ExportParam>(request).is_err(),
            "the MCP export still reads `{key}`"
        );
    }
    let plain = serde_json::json!({ "path": "/tmp/k6.nc" });
    assert!(serde_json::from_value::<rs_cam_mcp::server::ExportParam>(plain).is_ok());
}

/// The GUI pre-flight holds no checkbox: no override, no "I understand the
/// risks" step (operator ruling on the mock, 2026-11-03). It draws the
/// chip row and one Export button.
#[test]
fn the_preflight_holds_no_override_checkbox_k6() {
    let source =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ui/preflight.rs"))
            .expect("read the pre-flight source");
    let code: Vec<&str> = source
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect();
    // Non-vacuity: the chip row and the button are in this file.
    assert!(
        code.iter().any(|l| l.contains("danger_counts(")),
        "the anchor moved"
    );
    assert!(
        code.iter().any(|l| l.contains("export_asks_a_click(")),
        "the anchor moved"
    );
    for needle in [
        "ui.checkbox(",
        "Export Anyway",
        "Accept EXCEEDED",
        "Accept unmodeled",
    ] {
        assert!(
            !code.iter().any(|l| l.contains(needle)),
            "the pre-flight still holds `{needle}`"
        );
    }
}
