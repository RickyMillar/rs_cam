//! CONTRACT K6 (feeds contract P2, ruling Q6): no finding refuses a job.
//!
//! Before P2 each of these findings stopped the job: the height planes out
//! of order, a cut into the model, a plunge of a tool that does not cut at
//! its centre and its strike in the simulation, an op with no current
//! result, a load exceedance, a drill `Critical` and a stale simulation.
//! Now every op that has its inputs generates, both session doors return a
//! program, and the export result and the G-code header name each finding.
//!
//! The project is `common/k6_project.rs`. The load rows come from a
//! hand-built report through `export_gcode_with_report`, the door that
//! `export_gcode_checked` takes with the report of the session.
//!
//! `cargo test -p rs_cam_core -q --test a_job_with_each_finding_exports_and_names_it_k6`

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

#[path = "common/k6_project.rs"]
mod k6_project;

use std::collections::BTreeSet;

use rs_cam_core::diagnostics::ids;
use rs_cam_core::gcode::{ExportWarning, ExportedGcode};
use rs_cam_core::session::ProjectSession;
use rs_cam_core::session::run_state::{RunState, toolpath_run_state};
use rs_cam_core::tool_load::drill_gates::{
    DrillCycleKind, DrillGateOutcome, DrillGateSeverity, DrillGatesVerdict,
};
use rs_cam_core::tool_load::verdict::{
    DeflectionBounds, DepthVerdict, PowerVerdict, SampleEvidence, ToolLoadReport,
    ToolpathLoadVerdict,
};
use rs_cam_core::tool_load::{ChiploadVerdict, Confidence, DeflectionVerdict, UnmodeledReason};

/// The ids of the findings the session doors raise from the project with
/// a fresh simulation. P22-0c-3 (one answer per move): the simulation's
/// core answer (`safety.non_centre_cutting_descent`) supersedes the static
/// `safety.non_centre_cutting_plunge` ([`STATIC_ONLY_IDS`]).
const PROJECT_IDS: [&str; 4] = [
    ids::GEOM_CLEARANCE_Z_BELOW_RETRACT_Z,
    ids::SAFETY_CUTS_INTO_MODEL,
    ids::SAFETY_NON_CENTRE_CUTTING_DESCENT,
    ids::EXPORT_OP_LEFT_OUT,
];

/// The ids that stand only while no fresh simulation measured the core:
/// an export with no trace names them; a fresh trace supersedes them.
const STATIC_ONLY_IDS: [&str; 1] = [ids::SAFETY_NON_CENTRE_CUTTING_PLUNGE];

/// The ids the hand-built report adds.
const LOAD_IDS: [&str; 3] = [
    ids::LOAD_DEFLECTION_EXCEEDS,
    ids::DRILL_CHIP_WELDING,
    ids::LOAD_CHIPLOAD_UNMODELED,
];

fn ids_of(warnings: &[ExportWarning]) -> BTreeSet<&'static str> {
    warnings.iter().map(|w| w.id).collect()
}

fn toolpath_id(session: &ProjectSession, index: usize) -> rs_cam_core::ids::ToolpathId {
    session
        .toolpath_configs()
        .get(index)
        .map(|tc| tc.id)
        .expect("the op exists")
}

/// A verdict with every milling row unmodelled for want of a simulation.
fn unmodelled(id: rs_cam_core::ids::ToolpathId) -> ToolpathLoadVerdict {
    let none = || UnmodeledReason::SimulationRequired;
    ToolpathLoadVerdict {
        toolpath_id: id,
        chipload: ChiploadVerdict::Unmodeled { reason: none() },
        power: PowerVerdict::Unmodeled { reason: none() },
        deflection: DeflectionVerdict::Unmodeled { reason: none() },
        depth: DepthVerdict::Unmodeled { reason: none() },
        drill_gates: None,
        modulation_summary: None,
        feed_explanation: None,
        kinematic_utilization: None,
        material_split: None,
        cut_force: None,
    }
}

/// The hand-built report: a deflection exceedance on op 1, a drill
/// `Critical` on op 0, and a stale chipload row on op 2.
fn load_report(session: &ProjectSession) -> ToolLoadReport {
    let mut deflection = unmodelled(toolpath_id(session, k6_project::GOUGE));
    deflection.deflection = DeflectionVerdict::Exceeds {
        peak_mm: 0.4,
        bounds: DeflectionBounds {
            validated_within_mm: 0.050,
            exceeds_mm: 0.200,
            basis: rs_cam_core::tool_load::verdict::DeflectionBasis::FinishTarget,
        },
        evidence: SampleEvidence::at(0),
        confidence: Confidence::Validated,
    };
    let within = |observed: f64, threshold: f64| DrillGateOutcome::Within {
        observed,
        threshold,
        envelope_lo: None,
        envelope_hi: None,
    };
    let mut drill = unmodelled(toolpath_id(session, k6_project::INVERTED));
    drill.drill_gates = Some(DrillGatesVerdict {
        chip_welding: DrillGateOutcome::Exceeds {
            observed: 12.0,
            threshold: 8.0,
            severity: DrillGateSeverity::Critical,
            envelope_lo: None,
            envelope_hi: None,
        },
        peck_adequacy: within(0.3, 6.0),
        plunge_feed: within(100.0, 50.0),
        worst_hole_id: None,
        population: None,
        cycle: DrillCycleKind::Peck,
    });
    let mut stale = unmodelled(toolpath_id(session, k6_project::PLUNGE));
    stale.chipload = ChiploadVerdict::Unmodeled {
        reason: UnmodeledReason::StaleSimulation,
    };
    ToolLoadReport {
        per_toolpath: vec![deflection, drill, stale],
    }
}

/// The claims every door makes on its export: each id is named in the
/// result and in the header, and the header is comment lines only.
fn assert_names_each(exported: &ExportedGcode, expected: &[&str], door: &str) {
    let named = ids_of(&exported.warnings);
    for id in expected {
        assert!(
            named.contains(id),
            "{door}: the result does not name {id}: {named:?}"
        );
        let line = exported
            .gcode
            .lines()
            .find(|line| line.contains("WARNING") && line.contains(id))
            .unwrap_or_else(|| panic!("{door}: the header does not name {id}"));
        assert!(
            rs_cam_core::gcode::strip_comments(line).trim().is_empty(),
            "{door}: the header line is not a comment: {line}"
        );
    }
    // The header: one comment per warning, in one block directly after
    // the post preamble (its first line "(Generated by rs_cam)", its last
    // the spindle start), before the first motion.
    let lines: Vec<&str> = exported.gcode.lines().collect();
    assert_eq!(
        lines.first().copied(),
        Some("(Generated by rs_cam)"),
        "{door}"
    );
    let first = lines
        .iter()
        .position(|line| line.contains("WARNING"))
        .expect("the header holds the warnings");
    let header = lines[first..]
        .iter()
        .take_while(|line| line.contains("WARNING"))
        .count();
    assert_eq!(
        header,
        exported.warnings.len(),
        "{door}: one header line per warning, in one block"
    );
    assert!(
        lines[first - 1].starts_with("M3"),
        "{door}: the header follows the preamble: {:?}",
        &lines[..=first]
    );
    let motion = lines
        .iter()
        .position(|line| {
            matches!(
                line.split_whitespace().next(),
                Some(word) if word == "G0" || word == "G1"
            )
        })
        .expect("the program moves");
    assert!(
        first < motion,
        "{door}: the header comes before the first motion"
    );
    for warning in &exported.warnings {
        assert!(
            k6_project::NAMES.contains(&warning.toolpath.as_str()),
            "{door}: the warning names no toolpath of the project: {warning:?}"
        );
    }
}

/// The sentry: every op with its inputs generates, no finding sets
/// `NotReady`, and both doors export and name each finding.
#[test]
fn a_job_with_each_finding_exports_and_names_it_k6() {
    let session = k6_project::generated_and_simulated();
    for index in [k6_project::INVERTED, k6_project::GOUGE, k6_project::PLUNGE] {
        assert!(session.result_at(index).is_some(), "op {index} generates");
        let state = toolpath_run_state(&session, toolpath_id(&session, index), false);
        assert!(
            !matches!(state, Some(RunState::NotReady { .. })),
            "op {index}: a finding sets {state:?}"
        );
    }
    assert!(session.result_at(k6_project::LEFT_OUT).is_none());

    let trace = session
        .simulation_result()
        .and_then(|sim| sim.cut_trace.clone());
    let checked = session
        .export_gcode_checked(trace.as_deref())
        .expect("no finding refuses the export");
    assert_names_each(&checked, &PROJECT_IDS, "export_gcode_checked");
    for id in STATIC_ONLY_IDS {
        assert!(
            !ids_of(&checked.warnings).contains(id),
            "export_gcode_checked: a fresh simulation supersedes {id}"
        );
    }
    // With no fresh trace the static answer stands (P22-0c-3).
    let no_trace = session
        .export_gcode_checked(None)
        .expect("no finding refuses the export");
    let before: Vec<&str> = PROJECT_IDS
        .iter()
        .chain(&STATIC_ONLY_IDS)
        .copied()
        .collect();
    assert_names_each(&no_trace, &before, "export_gcode_checked (no trace)");

    let with_report = session
        .export_gcode_with_report(&load_report(&session))
        .expect("no load row refuses the export");
    let all: Vec<&str> = PROJECT_IDS.iter().chain(&LOAD_IDS).copied().collect();
    assert_names_each(&with_report, &all, "export_gcode_with_report");
    assert_eq!(
        rs_cam_core::gcode::program_without_comments(&checked.gcode),
        rs_cam_core::gcode::program_without_comments(&with_report.gcode),
        "the load rows change comment lines only"
    );
}

/// A project with no `Warn` finding writes no header line.
#[test]
fn a_clean_job_writes_no_warning_line_k6() {
    let mut session = k6_project::generated_and_simulated();
    // Keep only the gouge-free, plunge-free, ordered op: disable the rest.
    for index in [
        k6_project::INVERTED,
        k6_project::GOUGE,
        k6_project::PLUNGE,
        k6_project::LEFT_OUT,
    ] {
        let _ = session
            .apply(rs_cam_core::session::Command::SetToolpathEnabled(
                rs_cam_core::session::SetToolpathEnabledArgs {
                    index,
                    enabled: index == k6_project::INVERTED,
                },
            ))
            .expect("the op takes the switch");
    }
    let _ = session
        .apply(rs_cam_core::session::Command::SetToolpathHeights(
            rs_cam_core::session::SetToolpathHeightsArgs {
                index: k6_project::INVERTED,
                heights: rs_cam_core::compute::config::HeightsConfig::default(),
            },
        ))
        .expect("the default heights are in order");
    session
        .generate_toolpath(
            k6_project::INVERTED,
            &std::sync::atomic::AtomicBool::new(false),
        )
        .expect("the op generates");
    let report = ToolLoadReport {
        per_toolpath: Vec::new(),
    };
    let exported = session
        .export_gcode_with_report(&report)
        .expect("the export runs");
    assert!(exported.warnings.is_empty(), "{:?}", exported.warnings);
    assert!(!exported.gcode.contains("WARNING"), "{}", exported.gcode);
}
