//! CAM-9 (design A, commit C5): an MCP camera pose lands in one call, and
//! a refused request changes nothing.
//!
//! # The claim
//!
//! 1. After `apply_camera_request` with a full pose, the camera holds that
//!    pose. No frame step runs: the write is instant.
//! 2. Each refusal leaves the camera as it was: a NaN, `distance = 0`,
//!    `pitch_deg = 91`, `fit: model` with no model, `fit: selection` with
//!    no selected toolpath, and `fit` together with `frame_items`.

#![cfg(feature = "mcp")]
#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use nalgebra::Point3;
use rs_cam_mcp::server::{CameraParam, CameraPoseParam};
use rs_cam_viz::render::camera::CameraPose;
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::camera::{
    CameraRequest, FitTarget, PosePatch, apply_camera_request, check_camera_request,
};

#[test]
fn a_full_pose_lands_at_once_cam9() {
    let mut state = AppState::new();
    let request = CameraRequest {
        pose: PosePatch {
            target: Some(Point3::new(12.0, -4.0, 3.5)),
            distance: Some(321.0),
            yaw: Some(0.25),
            pitch: Some(-0.5),
        },
        ..CameraRequest::default()
    };
    let echo = apply_camera_request(&mut state, &request).expect("the pose is in range");
    let want = CameraPose {
        target: Point3::new(12.0, -4.0, 3.5),
        distance: 321.0,
        yaw: 0.25,
        pitch: -0.5,
    };
    assert_eq!(state.camera.pose().pose(), want);
    assert_eq!(echo.pose, want);
    assert!(echo.framed.is_none());
}

fn wire(pose: CameraPoseParam) -> CameraRequest {
    CameraParam {
        pose: Some(pose),
        ..CameraParam::default()
    }
    .into()
}

#[test]
fn a_refused_request_changes_nothing_cam9() {
    let mut state = AppState::new();
    state
        .camera
        .preset(rs_cam_viz::render::camera::ViewPreset::Isometric);
    let before = *state.camera.pose();

    let refused = [
        (
            "NaN distance",
            wire(CameraPoseParam {
                distance: Some(f64::NAN),
                ..CameraPoseParam::default()
            }),
        ),
        (
            "distance 0",
            wire(CameraPoseParam {
                distance: Some(0.0),
                ..CameraPoseParam::default()
            }),
        ),
        (
            "pitch_deg 91",
            wire(CameraPoseParam {
                pitch_deg: Some(91.0),
                ..CameraPoseParam::default()
            }),
        ),
        (
            "NaN target",
            wire(CameraPoseParam {
                target: Some([0.0, f64::NAN, 0.0]),
                // A valid field beside the bad one must not apply either.
                distance: Some(50.0),
                ..CameraPoseParam::default()
            }),
        ),
        (
            "fit model with no model",
            CameraRequest {
                fit: Some(FitTarget::Model),
                preset: Some(rs_cam_viz::render::camera::ViewPreset::Top),
                ..CameraRequest::default()
            },
        ),
        (
            "fit selection with no selected toolpath",
            CameraRequest {
                fit: Some(FitTarget::Selection),
                ..CameraRequest::default()
            },
        ),
    ];
    for (name, request) in refused {
        let answer = apply_camera_request(&mut state, &request);
        assert!(answer.is_err(), "{name} was not refused");
        assert_eq!(*state.camera.pose(), before, "{name} changed the camera");
    }

    // `fit` beside `frame_items` in one call: the check refuses it.
    let request = CameraRequest {
        fit: Some(FitTarget::Model),
        ..CameraRequest::default()
    };
    let focus = state.focus().clone();
    let refusal = check_camera_request(&state, &request, &focus, true)
        .expect_err("two framings in one call are refused");
    assert!(refusal.contains("frame_items"), "{refusal}");
    assert_eq!(*state.camera.pose(), before);
}
