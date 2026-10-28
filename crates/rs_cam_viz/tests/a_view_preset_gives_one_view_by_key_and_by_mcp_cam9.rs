//! CAM-9 (design A, commit C5): a view preset, a fit and a pose give one
//! view by key and by MCP.
//!
//! # The claim
//!
//! 1. For each preset and each projection, the key path
//!    (`CameraState::preset`) and the MCP door (`apply_camera_request` with
//!    `preset`) give a bitwise-equal `view_proj(16/9)`.
//! 2. `fit: model` and Reset View (`fit_to_first_model`) give an equal
//!    camera.
//! 3. The pose echo replays: degrees on the wire become radians in the
//!    crate, and the echo of a replayed echo equals the first echo within
//!    1e-9 degrees. The first trip from a typed degree value is within
//!    1e-4 degrees, because the crate holds the angle as an `f32`.

#![cfg(feature = "mcp")]
#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::selection::ItemSet;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder};
use rs_cam_mcp::server::{CameraParam, CameraPoseParam};
use rs_cam_viz::render::camera::{ProjectionMode, ViewPreset};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::camera::{
    CameraEcho, CameraRequest, FitTarget, apply_camera_request, fit_to_first_model,
};

fn bits(m: [[f32; 4]; 4]) -> [[u32; 4]; 4] {
    m.map(|row| row.map(f32::to_bits))
}

/// A state with one model: a triangle 40 x 30 x 0 mm.
fn state_with_a_model() -> AppState {
    let mesh = TriangleMesh::from_raw(
        vec![
            P3::new(0.0, 0.0, 0.0),
            P3::new(40.0, 0.0, 0.0),
            P3::new(0.0, 30.0, 5.0),
        ],
        vec![[0, 1, 2]],
    );
    let model = LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(0),
        name: "triangle.stl".to_owned(),
        mesh: Some(Arc::new(mesh)),
        items: Arc::new(ItemSet::default()),
        path: PathBuf::from("triangle.stl"),
        kind: Some(ModelKind::Stl),
        units: Some(ModelUnits::Millimeters),
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    };
    let mut builder = ProjectSessionBuilder::new();
    let _ = builder.add_model(model);
    let mut state = AppState::new();
    state.replace_session(builder.build());
    state
}

#[test]
fn a_view_preset_gives_one_view_by_key_and_by_mcp_cam9() {
    let mut checked = 0;
    for projection in [ProjectionMode::Perspective, ProjectionMode::Orthographic] {
        for preset in [
            ViewPreset::Top,
            ViewPreset::Front,
            ViewPreset::Right,
            ViewPreset::Isometric,
        ] {
            let mut by_key = AppState::new();
            by_key.camera.set_projection(projection);
            by_key.camera.preset(preset);
            let mut by_mcp = AppState::new();
            let echo = apply_camera_request(
                &mut by_mcp,
                &CameraRequest {
                    preset: Some(preset),
                    projection: Some(projection),
                    ..CameraRequest::default()
                },
            )
            .expect("a preset is never refused");
            assert_eq!(
                bits(by_key.camera.pose().view_proj(16.0 / 9.0)),
                bits(by_mcp.camera.pose().view_proj(16.0 / 9.0)),
                "{projection:?} {preset:?}"
            );
            assert_eq!(echo.view_preset, Some(preset), "the echo names the preset");
            checked += 1;
        }
    }
    assert_eq!(checked, 8, "every arm ran");
}

#[test]
fn fit_model_is_reset_view_cam9() {
    let mut by_key = state_with_a_model();
    fit_to_first_model(&mut by_key);
    let mut by_mcp = state_with_a_model();
    let echo = apply_camera_request(
        &mut by_mcp,
        &CameraRequest {
            fit: Some(FitTarget::Model),
            ..CameraRequest::default()
        },
    )
    .expect("the project has a model");
    assert_eq!(by_key.camera.pose(), by_mcp.camera.pose());
    let framed = echo.framed.expect("the echo carries the framed box");
    assert_eq!(
        [framed.max.x, framed.max.y, framed.max.z],
        [40.0, 30.0, 5.0]
    );
}

/// The degrees of an echo, as the reply writes them.
fn degrees(echo: &CameraEcho) -> (f64, f64) {
    (
        f64::from(echo.pose.yaw).to_degrees(),
        f64::from(echo.pose.pitch).to_degrees(),
    )
}

fn pose_request(yaw_deg: f64, pitch_deg: f64) -> CameraRequest {
    CameraParam {
        pose: Some(CameraPoseParam {
            yaw_deg: Some(yaw_deg),
            pitch_deg: Some(pitch_deg),
            ..CameraPoseParam::default()
        }),
        ..CameraParam::default()
    }
    .into()
}

#[test]
fn the_pose_echo_replays_cam9() {
    for (yaw, pitch) in [(30.0, 20.0), (-135.5, -45.25), (0.1, 89.4)] {
        let mut state = AppState::new();
        let first = apply_camera_request(&mut state, &pose_request(yaw, pitch))
            .expect("a pose in range is never refused");
        let (yaw1, pitch1) = degrees(&first);
        assert!(
            (yaw1 - yaw).abs() < 1e-4 && (pitch1 - pitch).abs() < 1e-4,
            "({yaw}, {pitch}) came back as ({yaw1}, {pitch1})"
        );
        let second = apply_camera_request(&mut state, &pose_request(yaw1, pitch1))
            .expect("the echo replays");
        let (yaw2, pitch2) = degrees(&second);
        assert!(
            (yaw2 - yaw1).abs() < 1e-9 && (pitch2 - pitch1).abs() < 1e-9,
            "the replayed echo moved: ({yaw1}, {pitch1}) -> ({yaw2}, {pitch2})"
        );
    }
}
