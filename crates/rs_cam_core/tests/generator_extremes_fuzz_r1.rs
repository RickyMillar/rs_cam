//! R1 (tech-debt review 2026-06-10) — generator robustness at
//! optimizer search-space extremes.
//!
//! The optimizer's candidate evaluation regenerates toolpaths at
//! parameters no interactive flow produces: the search space's hard
//! floors (0.05 mm depth-per-pass / stepover, `SearchPolicy`) and the
//! stepover ceiling (full tool diameter). The `param_sweep` harness
//! sweeps sensible values only, so generator panics at these corners
//! (e.g. cavalier_contours' `parallel_offset` asserts on degenerate
//! offsets — the WANAKA Back Rough optimize crash) shipped undetected.
//!
//! Bar: every op family the optimizer's axis grid actually sweeps must
//! GENERATE without panicking at the hard floor and ceiling corners.
//! Generation only — no sim. Two tiers:
//!
//! - CI tier (`fast_two_d_generators_survive_search_space_corners`):
//!   the cheap corners — DOC floor everywhere, stepover *ceiling* for
//!   clearing ops (few passes), zigzag at the stepover floor (linear
//!   move count, no offset chaining).
//! - Manual tier (`#[ignore]`): tiny-stepover full clearing (pocket /
//!   adaptive at the 0.05 mm floor, adaptive3d over terrain) — tens of
//!   minutes in debug. Run when touching pocket/adaptive clearing or
//!   polygon offsetting. The crash classes these found in development
//!   are pinned cheaply by `offset_polygon_degenerate_inputs_r1.rs`
//!   (captured WANAKA asset + synthetic repeat-vertex contract test).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::repo_root;
use rs_cam_core::ids::ToolpathId;

use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, AdaptiveConfig, PocketConfig, ProfileConfig, TraceConfig, ZigzagConfig,
};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::session::{ProjectSession, ToolpathConfig};

/// `SearchPolicy` hard floor for DOC and stepover axes.
const FLOOR_MM: f64 = 0.05;

/// Run one extreme-corner op config through the production generate
/// funnel under `catch_unwind`; returns a failure description on
/// panic or generation error.
fn generate_isolated(session: &mut ProjectSession, index: usize, label: &str) -> Option<String> {
    let cancel = AtomicBool::new(false);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        session.generate_toolpath(index, &cancel).map(|_| ())
    }));
    match outcome {
        Ok(Ok(())) => None,
        // A typed error at an extreme corner is acceptable behavior —
        // the optimizer skips the candidate. Only panics fail the bar.
        Ok(Err(_)) => None,
        Err(payload) => {
            let msg = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "non-string panic payload".to_owned());
            Some(format!("{label}: generator panicked: {msg}"))
        }
    }
}

fn toolpath_config(
    id: usize,
    name: &str,
    op: OperationConfig,
    tool_id: usize,
    model_id: usize,
) -> ToolpathConfig {
    let op_type = op.op_type();
    ToolpathConfig {
        id: ToolpathId(id),
        name: name.to_owned(),
        enabled: true,
        operation: op,
        dressups: DressupConfig::for_op(op_type),
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::default(),
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
    }
}

fn run_2d_matrix(matrix: &[(&str, OperationConfig)]) {
    let toml_path = repo_root().join("test_data/ux_2d_pocket.toml");
    let mut session = ProjectSession::load(&toml_path).expect("load ux_2d_pocket");

    let tool_id = session
        .tools()
        .iter()
        .find(|t| (t.diameter - 6.0).abs() < 1e-6)
        .map(|t| t.id.0)
        .expect("ux_2d_pocket.toml must define a 6 mm end mill");
    let model_id = session
        .models()
        .first()
        .map(|m| m.id)
        .expect("ux_2d_pocket.toml must load demo_pocket.svg");

    for (i, (name, op)) in matrix.iter().enumerate() {
        session
            .add_toolpath(0, toolpath_config(i, name, op.clone(), tool_id, model_id))
            .expect("add extreme toolpath");
    }

    let mut failures = Vec::new();
    for (i, (name, _)) in matrix.iter().enumerate() {
        if let Some(f) = generate_isolated(&mut session, i, name) {
            failures.push(f);
        }
    }
    assert!(
        failures.is_empty(),
        "2D generators panicked at search-space corners:\n{}",
        failures.join("\n")
    );
}

/// 6 mm end mill in ux_2d_pocket.toml — used for the stepover ceiling.
const TOOL_DIAMETER_MM: f64 = 6.0;

/// Shallow depth keeps level counts (depth / dpp at the 0.05 floor)
/// small — the extreme under test is the per-level parameter, not the
/// level count.
const DEPTH_MM: f64 = 0.1;

#[test]
fn fast_two_d_generators_survive_search_space_corners() {
    run_2d_matrix(&[
        (
            "pocket@stepover-ceiling",
            OperationConfig::Pocket(PocketConfig {
                depth: DEPTH_MM,
                depth_per_pass: FLOOR_MM,
                stepover: TOOL_DIAMETER_MM,
                ..PocketConfig::default()
            }),
        ),
        (
            "adaptive@stepover-ceiling",
            OperationConfig::Adaptive(AdaptiveConfig {
                depth: DEPTH_MM,
                depth_per_pass: FLOOR_MM,
                stepover: TOOL_DIAMETER_MM,
                ..AdaptiveConfig::default()
            }),
        ),
        (
            "profile@floors",
            OperationConfig::Profile(ProfileConfig {
                depth: DEPTH_MM,
                depth_per_pass: FLOOR_MM,
                ..ProfileConfig::default()
            }),
        ),
        (
            "zigzag@floors",
            OperationConfig::Zigzag(ZigzagConfig {
                depth: DEPTH_MM,
                depth_per_pass: FLOOR_MM,
                stepover: FLOOR_MM,
                ..ZigzagConfig::default()
            }),
        ),
        (
            "trace@floors",
            OperationConfig::Trace(TraceConfig {
                depth: DEPTH_MM,
                depth_per_pass: FLOOR_MM,
                ..TraceConfig::default()
            }),
        ),
    ]);
}

#[test]
#[ignore = "tens of minutes in debug: full pocket/adaptive clearing at the 0.05 mm \
            stepover floor (hundreds of offset rings / spiral passes). Run manually \
            when touching pocket/adaptive clearing or polygon offsetting."]
fn slow_two_d_clearing_at_stepover_floor() {
    run_2d_matrix(&[
        (
            "pocket@floors",
            OperationConfig::Pocket(PocketConfig {
                depth: DEPTH_MM,
                depth_per_pass: FLOOR_MM,
                stepover: FLOOR_MM,
                ..PocketConfig::default()
            }),
        ),
        (
            "adaptive@floors",
            OperationConfig::Adaptive(AdaptiveConfig {
                depth: DEPTH_MM,
                depth_per_pass: FLOOR_MM,
                stepover: FLOOR_MM,
                ..AdaptiveConfig::default()
            }),
        ),
    ]);
}

#[test]
#[ignore = "~40 min in debug: full adaptive3d clearing at the 0.05 mm stepover floor \
            over terrain_small. Run manually when touching adaptive3d/clearing or \
            polygon offsetting. The cheap CI teeth for the same crash class are \
            offset_polygon_degenerate_inputs_r1.rs (captured WANAKA asset) and the \
            fast 2D matrix above."]
fn adaptive3d_generator_survives_search_space_floors() {
    let toml_path = repo_root().join("test_data/ux_3d_terrain.toml");
    let mut session = ProjectSession::load(&toml_path).expect("load ux_3d_terrain");

    let tool = session
        .tools()
        .first()
        .cloned()
        .expect("ux_3d_terrain.toml must define a tool");
    let mesh_model_id = session
        .models()
        .iter()
        .find(|m| m.mesh.is_some())
        .map(|m| m.id)
        .expect("ux_3d_terrain.toml must load terrain_small.stl");

    // The WANAKA optimize crash came from exactly this family: the 3D
    // clearing planner hands terrain-slice polygons (with holes) to the
    // 2D adaptive machinability probe, whose tiny inward offsets at the
    // stepover floor hit cavalier_contours' degenerate-input asserts.
    // depth_per_pass stays sensible — the DOC floor only multiplies the
    // Z-level count (runtime, not a panic class); stepover is the axis
    // that produces degenerate offsets.
    let op = OperationConfig::Adaptive3d(Adaptive3dConfig {
        stepover: FLOOR_MM,
        ..Adaptive3dConfig::default()
    });
    session
        .add_toolpath(
            0,
            toolpath_config(0, "adaptive3d@floors", op, tool.id.0, mesh_model_id),
        )
        .expect("add adaptive3d toolpath");

    if let Some(f) = generate_isolated(&mut session, 0, "adaptive3d@floors") {
        panic!("{f}");
    }
}
