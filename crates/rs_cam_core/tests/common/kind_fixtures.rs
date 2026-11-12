//! The five simulated fixtures of the cut-kind sentries (P29-3 commit C,
//! P29-6 6d): a contour Pocket with a ramp entry, the same Pocket with
//! dogbones, a Profile with tabs and leads, the Geodesic Adaptive 3D Rough
//! of the D65 shelf, and a 2D Adaptive with its default helix entry.
//! `a_sample_states_its_cut_kind_p29_3` and
//! `no_removing_sample_is_outside_the_gates_p29_6` read them.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::sync::atomic::AtomicBool;

use super::meshes::height_field;
use super::op_fixtures::{flat_stock, outline_model};
use super::session::{mesh_model, single_op_session_with, stock_over};
use super::tools::endmill_tool_config;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{DogboneParams, DressupEntryStyle, LeadParams};
use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, Adaptive3dEntryStyle, AdaptiveConfig, ClearingStrategy, PocketConfig,
    PocketPattern, ProfileConfig,
};
use rs_cam_core::session::{ProjectSession, SimulationOptions, SimulationResolution};

/// The simulation cell of the flat fixtures (mm): the cell of
/// `super::op_fixtures::SIM_CELL_MM`.
pub const FLAT_CELL: f64 = super::op_fixtures::SIM_CELL_MM;
/// The tool diameter of every fixture (mm).
pub const TOOL_D: f64 = 6.0;

pub fn generate_and_simulate(session: &mut ProjectSession, cell: f64) {
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("the op generates");
    session
        .run_simulation(
            &SimulationOptions {
                resolution: SimulationResolution::Fixed(cell),
                adaptive_feed_modulation: false,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("the session simulates");
}

/// A contour Pocket of the outline, with a ramp entry. `dogbone`: the
/// dogbone dressup at 90 degrees cuts a relief at each inside corner.
pub fn pocket_session(dogbone: bool) -> ProjectSession {
    let op = OperationConfig::Pocket(PocketConfig {
        pattern: PocketPattern::Contour,
        ..PocketConfig::default()
    });
    let mut session = single_op_session_with(
        flat_stock(),
        endmill_tool_config(TOOL_D),
        outline_model(),
        "pocket",
        op,
        |tc| {
            tc.dressups.entry_style = DressupEntryStyle::Ramp;
            if dogbone {
                tc.dressups.dogbone = Some(DogboneParams { angle: 90.0 });
            }
        },
    );
    generate_and_simulate(&mut session, FLAT_CELL);
    session
}

/// An outside Profile of the outline with tabs and leads. The entry
/// dressup is off: a ramp entry turns the tab re-plunges into ramps.
pub fn profile_session() -> ProjectSession {
    let op = OperationConfig::Profile(ProfileConfig {
        tab_count: 4,
        ..ProfileConfig::default()
    });
    let mut session = single_op_session_with(
        flat_stock(),
        endmill_tool_config(TOOL_D),
        outline_model(),
        "profile",
        op,
        |tc| {
            tc.dressups.entry_style = DressupEntryStyle::None;
            tc.dressups.lead_in_out = Some(LeadParams {
                radius: TOOL_D / 3.0,
                in_feed_rate: None,
                out_feed_rate: None,
            });
        },
    );
    generate_and_simulate(&mut session, FLAT_CELL);
    session
}

/// A 2D Adaptive of the outline with its default entry, the helix (ruling
/// Q6: the fixture that holds `Entry(Helix)`).
pub fn adaptive_session() -> ProjectSession {
    let op = OperationConfig::Adaptive(AdaptiveConfig::default());
    let mut session = single_op_session_with(
        flat_stock(),
        endmill_tool_config(TOOL_D),
        outline_model(),
        "adaptive",
        op,
        |_| {},
    );
    generate_and_simulate(&mut session, FLAT_CELL);
    session
}

// The shelf pocket of the D65 sentry
// (`a_geodesic_step_over_the_ceiling_is_cut_capped_d65`), as P22a builds it
// (`every_wide_step_has_a_named_source_p22a`).
const TOP_Z: f64 = 10.0;
const FLOOR_Z: f64 = 4.0;
const MESH_HALF: f64 = 40.0;
const MESH_STEP: f64 = 0.5;
const POCKET_HALF_X: f64 = 34.0;
const POCKET_HALF_Y: f64 = 24.0;
const ISLAND_RADIUS: f64 = 5.0;
const SHELF_Z: f64 = 6.5;
const SHELF_STEPOVER: f64 = 2.0;
/// The simulation cell of the shelf (mm), P22a arm 1.
pub const SHELF_CELL: f64 = 0.25;

pub fn shelf_session() -> ProjectSession {
    let mesh = height_field(MESH_HALF, MESH_STEP, |x, y| {
        if x.hypot(y) <= ISLAND_RADIUS {
            TOP_Z
        } else if (12.0..=30.0).contains(&x) && y.abs() <= 12.0 {
            SHELF_Z
        } else if x.abs() <= POCKET_HALF_X && y.abs() <= POCKET_HALF_Y {
            FLOOR_Z
        } else {
            TOP_Z
        }
    });
    let op = OperationConfig::Adaptive3d(Adaptive3dConfig {
        stepover: Some(SHELF_STEPOVER),
        depth_per_pass: Some(TOP_Z - FLOOR_Z),
        clearing_strategy: ClearingStrategy::GeodesicAdaptive,
        entry_style: Adaptive3dEntryStyle::Ramp,
        ..Adaptive3dConfig::default()
    });
    let mut session = single_op_session_with(
        stock_over(MESH_HALF, TOP_Z),
        endmill_tool_config(TOOL_D),
        mesh_model(mesh, "shelf"),
        "shelf rough",
        op,
        |_| {},
    );
    generate_and_simulate(&mut session, SHELF_CELL);
    session
}
