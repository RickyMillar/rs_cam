//! G5 (grblHAL support, 2026-10-08): the controller's spindle wait.
//!
//! grblHAL waits `$394` seconds on each M3 that changes the spindle
//! (spindle_control.c:787-790, 828-838 in grblHAL/core@c3a887e). The
//! cycle-time estimate adds that wait at each spindle start, and the
//! export reads the same profile fact to drop its own warm-up dwell.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::session::{polygon_model, square_polygon, stock_under, toolpath_config};
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::gcode::{PostConfig, WizardOverlay};
use rs_cam_core::machine::{ControllerFirmware, ControllerSettings, MachineProfile};
use rs_cam_core::session::{
    ProjectSession, ProjectSessionBuilder, controller_spindle_wait_s, spindle_starts,
};

fn pocket(rpm: u32) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        spindle_rpm: Some(rpm),
        ..PocketConfig::default()
    })
}

/// Three pockets: T1 at 18 000, T1 at 18 000, T2 at 18 000. The program
/// starts the spindle twice: at the start and at the tool change.
fn session(controller: Option<ControllerSettings>) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new()
        .stock(stock_under(20.0, 10.0))
        .machine(MachineProfile {
            controller,
            ..MachineProfile::default()
        });
    let t1 = builder.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    let t1 = builder.tools()[t1].id.0;
    let t2 = builder.add_tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    let t2 = builder.tools()[t2].id.0;
    let model = builder.add_model(polygon_model(vec![square_polygon(10.0)], "square"));
    for (name, tool) in [("a", t1), ("b", t1), ("c", t2)] {
        let _ = builder
            .add_toolpath(0, toolpath_config(name, pocket(18_000), tool, model))
            .expect("add toolpath");
    }
    builder.build()
}

fn hal(on_delay_s: Option<f64>, at_speed_pct: Option<f64>) -> ControllerSettings {
    ControllerSettings {
        firmware: ControllerFirmware::GrblHal,
        spindle_on_delay_s: on_delay_s,
        spindle_at_speed_tolerance_pct: at_speed_pct,
        ..ControllerSettings::default()
    }
}

#[test]
fn the_estimate_adds_the_spindle_wait_at_each_start() {
    let s = session(Some(hal(Some(4.0), None)));
    assert_eq!(spindle_starts(&s), 2, "start + one tool change");
    assert!((controller_spindle_wait_s(&s) - 8.0).abs() < 1e-9);

    // No controller facts: no wait is added.
    assert_eq!(controller_spindle_wait_s(&session(None)), 0.0);
}

#[test]
fn the_profile_fact_turns_off_the_warmup_dwell() {
    let mut overlay = WizardOverlay {
        spindle_warmup_secs: 3,
        ..WizardOverlay::default()
    };
    let post = PostConfig::default();

    post.apply_controller_options(
        &MachineProfile {
            controller: Some(hal(Some(2.0), None)),
            ..MachineProfile::default()
        },
        &mut overlay,
    );
    assert!(overlay.controller_waits_for_spindle, "$394 > 0 waits");

    post.apply_controller_options(
        &MachineProfile {
            controller: Some(hal(None, Some(10.0))),
            ..MachineProfile::default()
        },
        &mut overlay,
    );
    assert!(overlay.controller_waits_for_spindle, "$340 > 0 waits");

    post.apply_controller_options(&MachineProfile::default(), &mut overlay);
    assert!(!overlay.controller_waits_for_spindle, "no facts: no wait");

    // The project's own choice wins over the profile.
    let explicit = PostConfig {
        controller_waits_for_spindle: Some(false),
        ..PostConfig::default()
    };
    explicit.apply_controller_options(
        &MachineProfile {
            controller: Some(hal(Some(2.0), None)),
            ..MachineProfile::default()
        },
        &mut overlay,
    );
    assert!(!overlay.controller_waits_for_spindle);
}
