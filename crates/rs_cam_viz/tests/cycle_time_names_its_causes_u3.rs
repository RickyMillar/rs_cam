//! U3 (memory programme 2026-10-01): a weak cycle time names each cause,
//! and each remedy names the control that removes it.
//!
//! The operator simulated with "Capture cutting metrics" off, on a machine
//! profile with no kinematics. The GUI showed "8:13:19 (cutting only, no
//! accel)" and the remedy "Run a simulation to get a modelled estimate"
//! directly after a simulation ran. The basis alone folds both causes into
//! one word. These tests pin the label and the remedies that the viz
//! surfaces now read from `CycleTime::missing`.
//!
//! The per-cause decision itself is unit-tested in core
//! (`session/cycle_time.rs`); this file pins the viz half: the label text
//! on the row and the remedy text on hover and below the row.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::session::{CycleTime, CycleTimeBasis, CycleTimeEvidence, MissingInput};
use rs_cam_viz::ui::readiness::{
    MissingInputExt, cycle_time_hover, cycle_time_remedies, format_cycle_time,
};

fn evidence(sim: bool, trace: bool, kin: bool) -> CycleTimeEvidence {
    CycleTimeEvidence {
        simulation_present: sim,
        cut_trace_present: trace,
        machine_kinematics_present: kin,
    }
}

/// The operator's row, as the timeline and the readiness panel print it.
#[test]
fn the_operators_row_names_both_causes() {
    let cycle = CycleTime::of(29_599.0, CycleTimeBasis::CuttingOnly)
        .with_evidence(evidence(true, false, false));
    let row = format!("{} ({})", format_cycle_time(cycle.seconds), cycle.label());
    assert_eq!(
        row,
        "8:13:19 (cutting only: metrics not captured, no machine kinematics)"
    );
}

/// After a trace-less run, no remedy says "Run a simulation". The trace
/// remedy names the control path and says to re-run.
#[test]
fn a_trace_less_run_never_says_run_a_simulation() {
    let cycle = CycleTime::of(29_599.0, CycleTimeBasis::CuttingOnly)
        .with_evidence(evidence(true, false, false));
    let remedies = cycle_time_remedies(&cycle);
    assert_eq!(remedies.len(), 2, "{remedies:?}");
    for remedy in &remedies {
        assert!(
            !remedy.contains("Run a simulation"),
            "a simulation ran; the remedy must not tell the operator to run one: {remedy}"
        );
    }

    let trace = remedies[0];
    assert!(
        trace.contains("Simulation \u{25B8} Setup & run \u{25B8} \"Capture cutting metrics\""),
        "the remedy must name the control as the GUI shows it (sim_op_list.rs): {trace}"
    );
    assert!(trace.contains("re-run the simulation"), "{trace}");

    let kinematics = remedies[1];
    assert!(
        kinematics.contains("Machine properties \u{25B8} Kinematics")
            && kinematics.contains("Import GRBL $$"),
        "{kinematics}"
    );

    // The hover carries the caveat, then every remedy.
    let hover = cycle_time_hover(&cycle);
    assert!(
        hover.starts_with(CycleTimeBasis::CuttingOnly.caveat()),
        "{hover}"
    );
    for remedy in remedies {
        assert!(hover.contains(remedy), "{hover}");
    }
}

/// Every cause carries a remedy, and only the never-simulated cause tells
/// the operator to run a first simulation.
#[test]
fn every_cause_has_a_remedy_and_only_no_simulation_asks_for_a_first_run() {
    for input in MissingInput::ALL {
        let remedy = input.remedy();
        assert!(!remedy.is_empty(), "{input:?}");
        assert!(!input.short_label().is_empty(), "{input:?}");
        let asks_for_first_run = remedy.contains("Run the simulation");
        assert_eq!(
            asks_for_first_run,
            input == MissingInput::NoSimulation,
            "{input:?}: {remedy}"
        );
    }
}

/// A wall-clock estimate carries no remedy; no estimate carries none
/// either.
#[test]
fn a_complete_estimate_has_no_remedy() {
    let cycle = CycleTime::of(100.0, CycleTimeBasis::MachineModel)
        .with_evidence(evidence(true, true, true));
    assert!(cycle_time_remedies(&cycle).is_empty());
    assert_eq!(cycle.label(), "wall clock");
    assert!(cycle_time_remedies(&CycleTime::NONE).is_empty());
}
