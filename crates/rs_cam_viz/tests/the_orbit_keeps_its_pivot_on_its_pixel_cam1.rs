//! CAM-1 (design A, commit C1): the orbit keeps its pivot on its pixel.
//!
//! # The claim
//!
//! `OrbitCamera::orbit_about(Q, dx, dy)` keeps the camera-space coordinates
//! of `Q`. So `Q` keeps its pixel in the perspective and the orthographic
//! projection, also when the pitch reaches its clamp. The distance does not
//! change.
//!
//! The regression anchor: with `Q = target` the yaw and the pitch take the
//! old turntable step, and the target does not move.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use nalgebra::Point3;
use rs_cam_viz::render::camera::{ORBIT_RAD_PER_PX, OrbitCamera, PITCH_LIMIT, ProjectionMode};

const VW: f32 = 1600.0;
const VH: f32 = 900.0;
const TOL_PX: f32 = 1e-2;

fn camera(projection: ProjectionMode) -> OrbitCamera {
    let mut cam = OrbitCamera::new();
    cam.target = Point3::new(12.0, 5.0, -2.0);
    cam.distance = 160.0;
    cam.yaw = 0.3;
    cam.pitch = 0.9;
    cam.projection = projection;
    cam
}

fn pixel(cam: &OrbitCamera, p: Point3<f32>) -> [f32; 2] {
    cam.project_to_screen([p.x, p.y, p.z], VW / VH, VW, VH)
        .expect("the pivot is in front of the camera")
}

#[test]
fn the_orbit_keeps_its_pivot_on_its_pixel_cam1() {
    for projection in [ProjectionMode::Perspective, ProjectionMode::Orthographic] {
        let mut cam = camera(projection);
        let pivot = Point3::new(30.0, -10.0, 6.0);
        let start = pixel(&cam, pivot);
        let mut clamped = false;
        for step in 0..20 {
            // The first ten steps drive the pitch past the upper clamp.
            let (dx, dy) = if step < 10 {
                (14.0, 40.0)
            } else {
                (-9.0, -25.0)
            };
            cam.orbit_about(pivot, dx, dy);
            clamped |= cam.pitch == PITCH_LIMIT;
            let now = pixel(&cam, pivot);
            let moved = (now[0] - start[0]).hypot(now[1] - start[1]);
            assert!(
                moved < TOL_PX,
                "{projection:?} step {step}: the pivot moved {moved} px"
            );
            assert!(
                (cam.distance - 160.0).abs() < 1e-3,
                "{projection:?} step {step}: distance {} changed",
                cam.distance
            );
        }
        assert!(clamped, "{projection:?}: no step reached the pitch clamp");
    }
}

#[test]
fn an_orbit_about_the_target_is_the_old_step_cam1() {
    for projection in [ProjectionMode::Perspective, ProjectionMode::Orthographic] {
        let mut cam = camera(projection);
        for (dx, dy) in [(10.0_f32, -6.0_f32), (-3.0, 400.0), (7.0, -800.0)] {
            let before = cam.pose();
            // The old `orbit` arithmetic, written inline.
            let want_yaw = before.yaw - dx * 0.005;
            let want_pitch = (before.pitch + dy * 0.005).clamp(
                -std::f32::consts::FRAC_PI_2 + 0.01,
                std::f32::consts::FRAC_PI_2 - 0.01,
            );
            assert_eq!(ORBIT_RAD_PER_PX, 0.005, "the orbit rate is the old one");
            cam.orbit_about(before.target, dx, dy);
            assert_eq!(cam.yaw, want_yaw, "{projection:?}: yaw");
            assert_eq!(cam.pitch, want_pitch, "{projection:?}: pitch");
            assert_eq!(cam.distance, before.distance, "{projection:?}: distance");
            let moved = (cam.target - before.target).norm();
            assert!(
                moved < 1e-3,
                "{projection:?}: an orbit about the target moved the target {moved}"
            );
        }
    }
}
