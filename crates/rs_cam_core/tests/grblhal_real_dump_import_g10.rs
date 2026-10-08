//! G10 + G5 against the REAL controller output (2026-10-08).
//!
//! The operator's board is a BTT Scylla on grblHAL with an H-100 VFD over
//! Modbus. Its `$30=1000` is the PWM spindle maximum, which a spindle
//! plugin overrides (grblHAL/core@c3a887e settings.c:2593). The import
//! must not turn that number into an S clamp: the export keeps S18000.
//! Fixture: `tests/fixtures/grblhal_dump_2026-10-08/` (welcome line, `$I`,
//! `$$`, CRLF, as the controller printed them).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::gcode::{PostFormat, WizardOverlay, emitter, program_builder};
use rs_cam_core::geo::P3;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::session::{Command, ImportMachineSettingsArgs, ProjectSessionBuilder};
use rs_cam_core::toolpath::Toolpath;

fn real_dump() -> String {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/grblhal_dump_2026-10-08");
    ["welcome.txt", "dollar_I.txt", "dollar_dollar.txt"]
        .iter()
        .map(|f| std::fs::read_to_string(dir.join(f)).expect("read dump fixture"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_real_dump_selects_grblhal_and_does_not_clamp_s() {
    let dump = real_dump();
    // Non-numeric lines ($74= empty, $76=grblHAL_AP, $312=192.168.5.1)
    // are in the text and must not stop the import.
    assert!(dump.contains("$76=grblHAL_AP") && dump.contains("$312=192.168.5.1"));
    let imp = MachineKinematics::from_grbl_settings(&dump);
    assert!(imp.is_recognized());

    let mut session = ProjectSessionBuilder::new().build();
    assert_eq!(session.post_config().format, PostFormat::Grbl);
    let mut kinematics = imp.kinematics;
    kinematics.max_rate_xyz_mm_min = imp.max_rate_xyz_mm_min;
    let _effects = session
        .apply(Command::ImportMachineSettings(ImportMachineSettingsArgs {
            kinematics: Box::new(kinematics),
            max_feed_mm_min: imp.max_feed_mm_min,
            controller: Some(Box::new(imp.controller)),
        }))
        .expect("the import applies");

    // The profile selects the dialect.
    assert_eq!(session.post_config().format, PostFormat::GrblHal);
    let stored = session.machine().controller.clone().expect("stored facts");
    assert_eq!(
        stored.rpm_max,
        Some(1_000.0),
        "$30 is kept, for information"
    );

    // The export options this profile gives.
    let mut overlay = WizardOverlay::default();
    session
        .post_config()
        .apply_controller_options(session.machine(), &mut overlay);
    assert_eq!(overlay.rpm_range, None, "a VFD plugin: no S clamp");
    assert!(overlay.controller_waits_for_spindle, "$340=5.0: M3 waits");
    assert_eq!(overlay.mist_output, Some(true), "[OPT:...M...]");
    assert!(!overlay.controller_ignores_m6, "$341=0 is normal mode");

    // S18000 reaches the file as S18000.
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 5.0));
    tp.feed_to(P3::new(10.0, 0.0, -1.0), 800.0);
    let gcode = emitter::emit_program_with_overlay(
        &program_builder::build_single(&tp, 18_000),
        PostFormat::GrblHal.definition(),
        &overlay,
    );
    assert!(gcode.contains("M3 S18000"), "{gcode}");
    assert!(!gcode.contains("clamped"), "{gcode}");
}
