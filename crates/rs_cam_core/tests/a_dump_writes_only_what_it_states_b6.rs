//! B6 — a `$$` import writes only the fields the dump states.
//!
//! `planning/op_architecture_2026-10-14/PLAN.md` backlog B6, seen live on
//! 2026-10-15: `import_machine_settings` replaced the whole controller
//! block with what the dump held. The firmware went from grblHAL to
//! unknown, and the H-100 VFD plugin, the tool-change probe distance, the
//! spindle on delay and the `$30/$31/$32` values were lost. The same write
//! replaced the kinematics block, so the jerk limit and the junction
//! velocity clamp went too.
//!
//! # The claim
//!
//! A profile with every controller field and both unstated kinematics
//! fields set, and a dump with only `$11`, `$110-$112`, `$120-$122` and
//! `$341`: after the import exactly those settings differ, and the post
//! dialect does not move (the dump names no firmware).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::gcode::PostFormat;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::machine::{
    ControllerFirmware, ControllerMode, ControllerSettings, ControllerToolChangeMode,
    MachineProfile, SpindleDrive,
};
use rs_cam_core::session::{
    Command, ImportMachineSettingsArgs, ProjectSessionBuilder, SetMachineArgs, SetPostConfigArgs,
};

const DUMP: &str = "$11=0.020\n\
                    $110=9000.000\n$111=8000.000\n$112=900.000\n\
                    $120=400.000\n$121=350.000\n$122=200.000\n\
                    $341=3\n";

/// Every controller field and both kinematics fields no dump states.
fn full_profile() -> MachineProfile {
    let mut machine = MachineProfile::generic_wood_router();
    machine.kinematics = Some(MachineKinematics {
        acceleration_mm_s2: 300.0,
        acceleration_xyz_mm_s2: Some([300.0, 300.0, 150.0]),
        max_rate_xyz_mm_min: Some([5_000.0, 5_000.0, 500.0]),
        junction_deviation_mm: 0.010,
        jerk_mm_s3: Some(2_000.0),
        max_junction_velocity_mm_min: Some(1_500.0),
    });
    machine.controller = Some(ControllerSettings {
        firmware: ControllerFirmware::GrblHal,
        rpm_max: Some(1_000.0),
        rpm_min: Some(0.0),
        mode: Some(ControllerMode::Normal),
        spindle_at_speed_tolerance_pct: Some(5.0),
        tool_change_mode: Some(ControllerToolChangeMode::Normal),
        tool_change_probing_distance_mm: Some(30.0),
        spindle_on_delay_s: Some(2.5),
        mist_output: Some(true),
        spindle_drive: Some(SpindleDrive::Plugin("H-100 VFD".to_owned())),
    });
    machine
}

#[test]
fn a_dump_writes_only_the_settings_it_states_b6() {
    let mut session = ProjectSessionBuilder::new().build();
    let _ = session
        .apply(Command::SetMachine(SetMachineArgs {
            machine: Box::new(full_profile()),
        }))
        .expect("the machine row refuses nothing");
    let mut post = session.post_config().clone();
    post.format = PostFormat::GrblHal;
    let _ = session
        .apply(Command::SetPostConfig(SetPostConfigArgs {
            post: Box::new(post),
        }))
        .expect("the post row refuses nothing");

    let import = MachineKinematics::from_grbl_settings(DUMP);
    assert!(import.is_recognized());
    assert!(
        import.controller.spindle_drive.is_none()
            && import.controller.firmware == ControllerFirmware::Unknown,
        "precondition: the dump holds no `$I` text and no welcome line"
    );
    let _ = session
        .apply(Command::ImportMachineSettings(ImportMachineSettingsArgs {
            import: Box::new(import),
        }))
        .expect("the import applies");

    // The expected profile: the full profile with the stated settings only.
    let mut expected = full_profile();
    let kinematics = expected.kinematics.as_mut().unwrap();
    kinematics.junction_deviation_mm = 0.020;
    kinematics.acceleration_xyz_mm_s2 = Some([400.0, 350.0, 200.0]);
    kinematics.acceleration_mm_s2 = (400.0 + 350.0 + 200.0) / 3.0;
    kinematics.max_rate_xyz_mm_min = Some([9_000.0, 8_000.0, 900.0]);
    expected.max_feed_mm_min = 9_000.0;
    expected.controller.as_mut().unwrap().tool_change_mode =
        Some(ControllerToolChangeMode::AutomaticTouchOffAtG59_3);

    let json = |machine: &MachineProfile| serde_json::to_value(machine).expect("serialize");
    assert_eq!(json(session.machine()), json(&expected));
    assert_eq!(
        session.post_config().format,
        PostFormat::GrblHal,
        "a dump that names no firmware keeps the post dialect"
    );
}
