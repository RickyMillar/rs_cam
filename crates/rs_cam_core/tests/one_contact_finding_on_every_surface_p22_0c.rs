//! P22-0c-3 (design section 9 items 3 and 4): one contact finding, every
//! surface, one frame.
//!
//! The K6 project (`common/k6_project.rs`) holds a pocket whose entry
//! plunges a tool that does not cut at its centre into wood; the
//! simulation's contact kernel records a core contact on it.
//!
//! - Every surface lists `safety.non_centre_cutting_descent` for that
//!   toolpath: the toolpath diagnostics (MCP `get_toolpath_diagnostics`),
//!   the project diagnostics (`get_project_diagnostics`), the simulation
//!   triage (its safety list) and the export. The GUI list reads the same
//!   doors (the viz twin of this sentry). Red before P22-0c-3: the wanaka
//!   read found it only in the export header.
//! - One answer per move: with a fresh simulation the static
//!   `safety.non_centre_cutting_plunge` of the same entry is superseded on
//!   the toolpath list.
//! - One frame: the position the finding states is in the G-code frame of
//!   the setup, on an offset identity setup and on a rotated setup. Red
//!   before: the identity setup printed the world (model) frame.
//!
//! `cargo test -p rs_cam_core -q --features test-support --test one_contact_finding_on_every_surface_p22_0c`

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

#[path = "common/k6_project.rs"]
mod k6_project;

use rs_cam_core::diagnostics::ids;
use rs_cam_core::session::{ProjectEvidence, ProjectSession};

fn evidence(session: &ProjectSession) -> ProjectEvidence<'_> {
    ProjectEvidence::from_simulation_with_holder_collisions(
        session.simulation_result().expect("a simulation"),
        Vec::new(),
    )
}

fn plunge_id(session: &ProjectSession) -> rs_cam_core::ids::ToolpathId {
    session
        .toolpath_configs()
        .get(k6_project::PLUNGE)
        .expect("the plunge op")
        .id
}

#[test]
fn a_core_contact_is_listed_on_every_surface_p22_0c() {
    let session = k6_project::generated_and_simulated();
    let id = plunge_id(&session);
    let sim = session.simulation_result().expect("a simulation");
    assert!(
        sim.non_centre_cutting_descents
            .iter()
            .any(|hit| hit.toolpath_id == id),
        "the fixture: the kernel records a core contact on the plunge op"
    );
    let evidence = evidence(&session);

    // The toolpath diagnostics (MCP `get_toolpath_diagnostics`).
    let own = session
        .diagnose_toolpath_with_evidence(k6_project::PLUNGE, &evidence)
        .expect("the toolpath diagnoses");
    let descents = own
        .iter()
        .filter(|d| d.id.as_str() == ids::SAFETY_NON_CENTRE_CUTTING_DESCENT)
        .count();
    assert_eq!(descents, 1, "the toolpath list names the contact once");
    assert!(
        !own.iter()
            .any(|d| d.id.as_str() == ids::SAFETY_NON_CENTRE_CUTTING_PLUNGE),
        "the fresh simulation supersedes the static plunge row"
    );

    // The project diagnostics (MCP `get_project_diagnostics`).
    let project = session.diagnose_project_with_evidence(&evidence);
    assert!(
        project.iter().any(|d| {
            d.id.as_str() == ids::SAFETY_NON_CENTRE_CUTTING_DESCENT
                && d.scope == (rs_cam_core::diagnostics::Scope::Toolpath { id })
        }),
        "the project list names the contact: {:?}",
        project.iter().map(|d| d.id.as_str()).collect::<Vec<_>>()
    );

    // The simulation triage: a safety finding.
    let triage = session.simulation_triage(&evidence);
    assert!(
        triage
            .safety
            .iter()
            .any(|f| f.diagnostic.id.as_str() == ids::SAFETY_NON_CENTRE_CUTTING_DESCENT),
        "the triage safety list names the contact"
    );

    // The export.
    let exported = session
        .export_gcode_checked(sim.cut_trace.as_deref())
        .expect("no finding refuses the export");
    let row = exported
        .warnings
        .iter()
        .find(|w| w.id == ids::SAFETY_NON_CENTRE_CUTTING_DESCENT)
        .map(|w| w.detail.clone())
        .expect("the export names the contact");
    // Item 5: the text states the dwell rule of the core zone (core 2.4 mm,
    // protrusion 0.5 mm), not the straight-line angle atan(p / d).
    assert!(
        row.contains("at most 0.50 mm of descent along any 2.40 mm of path"),
        "the row states the dwell rule: {row}"
    );
    assert!(
        !row.contains("11.77 deg") && !row.contains("plunge and"),
        "the row no longer states the straight-line limit: {row}"
    );
}

/// The modal positions of a G-code program, in program order.
fn gcode_positions(gcode: &str) -> Vec<[f64; 3]> {
    let mut at = [f64::NAN; 3];
    let mut out = Vec::new();
    for line in gcode.lines() {
        let code = rs_cam_core::gcode::strip_comments(line);
        let mut moved = false;
        for word in code.split_whitespace() {
            let (axis, value) = word.split_at(1);
            let slot = match axis {
                "X" => 0,
                "Y" => 1,
                "Z" => 2,
                _ => continue,
            };
            if let Ok(v) = value.parse::<f64>()
                && let Some(cell) = at.get_mut(slot)
            {
                *cell = v;
                moved = true;
            }
        }
        if moved {
            out.push(at);
        }
    }
    out
}

/// The `X.. Y.. Z.. (G-code frame)` position of a finding text.
fn stated_position(text: &str) -> [f64; 3] {
    let end = text
        .find(" (G-code frame)")
        .unwrap_or_else(|| panic!("the text states its frame: {text}"));
    let head = &text[..end];
    let start = head.rfind(" X").expect("an X word") + 1;
    let mut out = [f64::NAN; 3];
    for (slot, word) in head[start..].split_whitespace().take(3).enumerate() {
        if let Some(cell) = out.get_mut(slot) {
            *cell = word[1..].parse().expect("a number");
        }
    }
    out
}

/// The descent row of the plunge op on the export, and the G-code.
fn descent_row_and_gcode(session: &ProjectSession) -> (String, String) {
    let trace = session
        .simulation_result()
        .and_then(|sim| sim.cut_trace.clone());
    let exported = session
        .export_gcode_checked(trace.as_deref())
        .expect("no finding refuses the export");
    let row = exported
        .warnings
        .iter()
        .find(|w| w.id == ids::SAFETY_NON_CENTRE_CUTTING_DESCENT)
        .map(|w| w.detail.clone())
        .expect("the export names the contact");
    (row, exported.gcode)
}

fn simulated_plunge_only(session: &mut ProjectSession) {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    session
        .generate_toolpath(k6_project::PLUNGE, &cancel)
        .expect("the plunge op generates");
    let skip: Vec<_> = session
        .toolpath_configs()
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != k6_project::PLUNGE)
        .map(|(_, tc)| tc.id)
        .collect();
    let opts = rs_cam_core::session::SimulationOptions {
        resolution: rs_cam_core::session::SimulationResolution::Fixed(0.5),
        skip_ids: skip,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy:
            rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
    };
    session
        .run_simulation(&opts, &cancel)
        .expect("the simulation completes");
}

#[test]
fn a_contact_position_is_in_the_gcode_frame_on_every_setup_p22_0c() {
    use rs_cam_core::compute::transform::ZRotation;
    for rotation in [ZRotation::Deg0, ZRotation::Deg90] {
        let mut session = k6_project::project();
        let _ = session
            .apply(rs_cam_core::session::Command::SetSetupRotation(
                rs_cam_core::session::SetSetupRotationArgs {
                    setup_index: 0,
                    z_rotation: rotation,
                },
            ))
            .expect("the setup takes the rotation");
        simulated_plunge_only(&mut session);
        let (row, gcode) = descent_row_and_gcode(&session);
        let stated = stated_position(&row);
        let positions = gcode_positions(&gcode);
        // The finding states a sample of a move: a point on the segment
        // between two consecutive G-code positions (to the printed 3
        // decimals).
        let on_segment = |a: &[f64; 3], b: &[f64; 3]| {
            let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let len_sq = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
            let t = if len_sq < 1e-12 {
                0.0
            } else {
                (((stated[0] - a[0]) * d[0]
                    + (stated[1] - a[1]) * d[1]
                    + (stated[2] - a[2]) * d[2])
                    / len_sq)
                    .clamp(0.0, 1.0)
            };
            (0..3).all(|k| (a[k] + t * d[k] - stated[k]).abs() < 2e-3)
        };
        let near = positions.windows(2).any(|w| on_segment(&w[0], &w[1]));
        assert!(
            near,
            "{rotation:?}: the stated position {stated:?} is on a G-code move: {row}"
        );
    }
}
