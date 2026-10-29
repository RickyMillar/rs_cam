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

/// A C3a: the same claim through the input state machine. The drag start
/// records the anchor as the pivot; 20 `step` + `apply_camera` rounds turn
/// the camera about it.
#[test]
fn a_drag_orbits_about_the_point_under_the_press_cam1() {
    use rs_cam_core::settings::MouseScheme;
    use rs_cam_viz::interaction::input::{DragAnchor, InputEnv, InputSample, apply_camera, step};
    use rs_cam_viz::state::camera::CameraState;
    use rs_cam_viz::state::viewport_input::DragState;

    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(VW, VH));
    for projection in [ProjectionMode::Perspective, ProjectionMode::Orthographic] {
        let base = camera(projection);
        let mut state = CameraState::default();
        state.set_pose(base.pose(), rs_cam_viz::state::camera::CameraWrite::Instant);
        state.set_projection(projection);
        let pivot = Point3::new(30.0_f32, -10.0, 6.0);
        let basis = state.pose().basis();
        let env = InputEnv {
            scheme: MouseScheme::DesktopCad,
            pick_mode_on: false,
            press_on_tab: false,
            drag_anchor: Some(DragAnchor {
                point: [pivot.x, pivot.y, pivot.z],
                depth: (pivot - basis.eye).dot(&basis.forward),
            }),
        };
        let start = pixel(state.pose(), pivot);
        let mut drag = DragState::Idle;
        for round in 0..20 {
            let (dx, dy) = if round < 10 {
                (14.0, 40.0)
            } else {
                (-9.0, -25.0)
            };
            let sample = InputSample {
                started: (round == 0).then_some(egui::PointerButton::Middle),
                dragging: Some(egui::PointerButton::Middle),
                stopped: false,
                delta: egui::vec2(dx, dy),
                press_origin: Some(egui::pos2(start[0], start[1])),
                pointer: Some(egui::pos2(start[0], start[1])),
                hover_pos: None,
                modifiers: egui::Modifiers::NONE,
                clicked: false,
                secondary_clicked: false,
                hovered: false,
                zoom_delta: 1.0,
                scroll: egui::Vec2::ZERO,
            };
            let actions = step(&mut drag, &sample, &env);
            apply_camera(&actions, &mut state, rect);
            let now = pixel(state.pose(), pivot);
            let moved = (now[0] - start[0]).hypot(now[1] - start[1]);
            assert!(
                moved < TOL_PX,
                "{projection:?} round {round}: the pivot moved {moved} px"
            );
            assert!(
                (state.pose().distance - base.distance).abs() < 1e-3,
                "{projection:?} round {round}: the distance changed"
            );
        }
        assert!(
            matches!(drag, DragState::Orbit { pivot: Some(_) }),
            "{projection:?}: the drag did not hold its pivot: {drag:?}"
        );
    }
}
