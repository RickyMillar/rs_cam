//! B7 — a machine edit stales exactly the toolpaths whose generation read
//! the machine.
//!
//! `planning/op_architecture_2026-10-14/PLAN.md` backlog B7: the link stage
//! costs each link against a retract with the machine's kinematics
//! (`MachineProfile::link_kinematics`), but a machine edit left every
//! toolpath current.
//!
//! # The rule
//!
//! `Capabilities::reads_link_kinematics` names the ops with a link stage.
//! A machine edit that moves the link envelope drops their results. Every
//! other op reads the machine at simulation time only and keeps its
//! result. An edit that leaves the envelope equal drops nothing.
//!
//! # The claims
//!
//! 1. The capability: a Drop Cutter (raster finish) reads the link
//!    envelope, a Pocket does not.
//! 2. A name change stales nothing.
//! 3. A kinematics edit (`SetMachineKinematics`) stales the Drop Cutter and
//!    keeps the Pocket.
//! 4. A `$$` import that moves the acceleration does the same.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::collections::BTreeSet;

use common::meshes::sawtooth_plate;
use common::session::{
    generate, mesh_model, pinned_heights, polygon_model, square_polygon, stock_over,
    toolpath_config,
};
use common::tools::{ball_tool_config, endmill_tool_config};

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{DropCutterConfig, PocketConfig};
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::session::{
    Command, ImportMachineSettingsArgs, ProjectSession, ProjectSessionBuilder, SetMachineArgs,
    SetMachineKinematicsArgs,
};

const HALF_MM: f64 = 20.0;
const POCKET: usize = 0;
const DROP_CUTTER: usize = 1;

/// A Pocket on a square and a Drop Cutter with a link stage on a
/// corrugated plate (the G-LINKVISIBLE fixture). Both generated.
fn two_ops() -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock_over(HALF_MM, 6.0));
    let end_mill = builder.add_tool(endmill_tool_config(6.0));
    let end_mill = builder.tools()[end_mill].id.0;
    let ball = builder.add_tool(ball_tool_config(3.0));
    let ball = builder.tools()[ball].id.0;
    let square = builder.add_model(polygon_model(vec![square_polygon(HALF_MM)], "square"));
    let plate = builder.add_model(mesh_model(sawtooth_plate(HALF_MM, 6.0, 2.0), "corrugated"));
    let pocket = toolpath_config(
        "Pocket",
        OperationConfig::Pocket(PocketConfig {
            stepover: 2.0,
            depth: 1.0,
            depth_per_pass: 1.0,
            ..PocketConfig::default()
        }),
        end_mill,
        square,
    );
    let mut drop_cutter = toolpath_config(
        "Drop Cutter",
        OperationConfig::DropCutter(DropCutterConfig {
            stepover: 0.5,
            slope_from: 0.0,
            slope_to: 20.0,
            hookup_mm: 3.0,
            ..DropCutterConfig::default()
        }),
        ball,
        plate,
    );
    drop_cutter.heights = pinned_heights(2.0, -1.0);
    let _ = builder.add_toolpath(0, pocket).expect("add the pocket");
    let _ = builder
        .add_toolpath(0, drop_cutter)
        .expect("add the drop cutter");
    let mut session = builder.build();
    generate(&mut session, POCKET);
    generate(&mut session, DROP_CUTTER);
    session
}

fn both_current(session: &ProjectSession) -> bool {
    session.get_result(POCKET).is_some() && session.get_result(DROP_CUTTER).is_some()
}

/// Claim 1.
#[test]
fn the_capability_names_the_link_readers_b7() {
    let session = two_ops();
    let reads = |index: usize| {
        session.toolpath_configs()[index]
            .operation
            .capabilities()
            .reads_link_kinematics()
    };
    assert!(reads(DROP_CUTTER), "a raster finish has a link stage");
    assert!(!reads(POCKET), "a Pocket has no link stage");
}

/// Claim 2.
#[test]
fn a_name_change_stales_nothing_b7() {
    let mut session = two_ops();
    let mut machine = session.machine().clone();
    machine.name = "Renamed".to_owned();
    let effects = session
        .apply(Command::SetMachine(SetMachineArgs {
            machine: Box::new(machine),
        }))
        .expect("the machine row refuses nothing");
    assert!(effects.stale.is_empty(), "stale: {:?}", effects.stale);
    assert!(both_current(&session));
}

/// Claim 3.
#[test]
fn a_kinematics_edit_stales_the_raster_finish_and_keeps_the_pocket_b7() {
    let mut session = two_ops();
    let mut kinematics = session.machine().effective_kinematics();
    kinematics.acceleration_mm_s2 *= 2.0;
    let effects = session
        .apply(Command::SetMachineKinematics(SetMachineKinematicsArgs {
            kinematics: Box::new(kinematics),
        }))
        .expect("a kinematics block");
    assert_eq!(effects.stale, BTreeSet::from([DROP_CUTTER]));
    assert!(session.get_result(DROP_CUTTER).is_none());
    assert!(
        session.get_result(POCKET).is_some(),
        "a Pocket reads the machine at simulation time only"
    );
}

/// Claim 4.
#[test]
fn a_dump_that_moves_the_acceleration_stales_the_raster_finish_b7() {
    let mut session = two_ops();
    let accel = session.machine().effective_kinematics().acceleration_mm_s2 * 2.0;
    let import = MachineKinematics::from_grbl_settings(&format!(
        "$120={accel}\n$121={accel}\n$122={accel}\n"
    ));
    let effects = session
        .apply(Command::ImportMachineSettings(ImportMachineSettingsArgs {
            import: Box::new(import),
        }))
        .expect("the import applies");
    assert_eq!(effects.stale, BTreeSet::from([DROP_CUTTER]));
    assert!(session.get_result(POCKET).is_some());
}
