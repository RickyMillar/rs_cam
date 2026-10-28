//! CAM-4 (design A, commit C4): a camera move eases and ends exactly.
//!
//! # The claim
//!
//! 1. `CameraMotion::sample` gives `from` at the start and `to` bit for bit
//!    at the end; the distance moves one way over 100 samples; the yaw takes
//!    the short arc (350 deg to 10 deg turns +20 deg, not -340 deg).
//! 2. `CameraState::advance` gives true inside the move, false at the end
//!    and after, and `is_moving` is then false. An orbit during a move
//!    drops the move and starts from the sampled pose. A move with no
//!    duration lands at once. A second eased write during a move starts
//!    from the sampled pose (no jump). An MCP write drops a running move.
//! 3. The preset table: `Front` looks from -Y with +X to the right of the
//!    screen, `Right` from +X with +Y to the right, and the orientation
//!    preview of a face gives the same view as the preset of that side.
//! 4. Source arm: the repaint gate in `app.rs` reads the `advance` answer,
//!    and no camera file asks for a repaint (G-LV.1).

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::Path;

use nalgebra::Point3;
use rs_cam_viz::render::camera::{CameraPose, OrbitCamera, ViewPreset};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::camera::{
    CameraMotion, CameraRequest, CameraState, CameraWrite, apply_camera_request,
    face_up_orientation,
};
use rs_cam_viz::state::job::FaceUp;

const DURATION: f64 = 0.24;

fn pose(yaw_deg: f32, distance: f32) -> CameraPose {
    CameraPose {
        target: Point3::new(1.0, 2.0, 3.0),
        distance,
        yaw: yaw_deg.to_radians(),
        pitch: 0.3,
    }
}

fn bits(p: CameraPose) -> [u32; 6] {
    [
        p.target.x.to_bits(),
        p.target.y.to_bits(),
        p.target.z.to_bits(),
        p.distance.to_bits(),
        p.yaw.to_bits(),
        p.pitch.to_bits(),
    ]
}

#[test]
fn a_motion_starts_at_from_and_ends_at_to_cam4() {
    let from = pose(350.0, 100.0);
    let to = CameraPose {
        target: Point3::new(-5.0, 7.0, 0.5),
        ..pose(10.0, 400.0)
    };
    let motion = CameraMotion::new(from, to, 10.0, DURATION);
    assert_eq!(bits(motion.sample(10.0)), bits(from), "the start is `from`");
    assert_eq!(
        bits(motion.sample(10.0 + DURATION)),
        bits(to),
        "the end is `to` bit for bit"
    );
    assert_eq!(
        bits(motion.sample(99.0)),
        bits(to),
        "after the end it is `to`"
    );

    // The distance moves one way.
    let mut last = from.distance;
    for i in 0..=100 {
        let d = motion
            .sample(10.0 + DURATION * f64::from(i) / 100.0)
            .distance;
        assert!(
            d >= last,
            "the distance went back at sample {i}: {d} < {last}"
        );
        last = d;
    }

    // The yaw takes the short arc: +20 deg, through 360 deg.
    let middle = motion.sample(10.0 + DURATION / 2.0);
    let turned = (middle.yaw - from.yaw).to_degrees();
    assert!(
        (turned - 10.0).abs() < 1e-3,
        "half way the yaw turned {turned} deg, not +10 deg"
    );
}

#[test]
fn advance_runs_the_move_and_stops_it_cam4() {
    let mut camera = CameraState::default();
    let to = pose(60.0, 300.0);
    camera.set_pose(
        to,
        CameraWrite::Eased {
            now: 0.0,
            duration: DURATION,
        },
    );
    assert!(camera.is_moving());
    assert!(camera.advance(DURATION / 2.0), "inside the move");
    assert!(!camera.advance(DURATION), "at the end");
    assert!(!camera.is_moving());
    assert!(!camera.advance(DURATION + 1.0), "after the end");
    let mut instant = CameraState::default();
    instant.set_pose(to, CameraWrite::Instant);
    assert_eq!(
        camera.pose(),
        instant.pose(),
        "the end equals an instant write"
    );
}

#[test]
fn an_orbit_during_a_move_keeps_the_sampled_pose_cam4() {
    let mut camera = CameraState::default();
    camera.set_pose(
        pose(90.0, 300.0),
        CameraWrite::Eased {
            now: 0.0,
            duration: DURATION,
        },
    );
    assert!(camera.advance(DURATION / 3.0));
    let sampled = camera.pose().pose();
    let target = sampled.target;
    camera.orbit_about(target, 0.0, 0.0);
    assert!(!camera.is_moving(), "the orbit drops the move");
    let after = camera.pose().pose();
    assert_eq!(after.distance, sampled.distance);
    assert_eq!(after.yaw, sampled.yaw);
    assert_eq!(after.pitch, sampled.pitch);
    assert!(!camera.advance(1.0), "nothing runs after the cancel");
    assert_eq!(camera.pose().pose().distance, sampled.distance);
}

#[test]
fn a_zero_duration_lands_at_once_and_a_second_write_does_not_jump_cam4() {
    let mut camera = CameraState::default();
    camera.set_pose(
        pose(30.0, 150.0),
        CameraWrite::Eased {
            now: 0.0,
            duration: 0.0,
        },
    );
    assert!(!camera.is_moving(), "a move with no duration is instant");
    assert_eq!(camera.pose().pose().distance, 150.0);

    // A second eased write half way starts from the sampled pose.
    camera.set_pose(
        pose(120.0, 600.0),
        CameraWrite::Eased {
            now: 0.0,
            duration: DURATION,
        },
    );
    let now = DURATION / 2.0;
    assert!(camera.advance(now));
    let sampled = camera.pose().pose();
    camera.set_pose(
        pose(-60.0, 50.0),
        CameraWrite::Eased {
            now,
            duration: DURATION,
        },
    );
    assert_eq!(
        bits(camera.pose().pose()),
        bits(sampled),
        "the second write jumped"
    );
    assert!(camera.advance(now + DURATION / 10.0));
}

#[test]
fn an_mcp_write_drops_a_running_move_cam4() {
    let mut state = AppState::new();
    state.camera.preset(
        ViewPreset::Right,
        CameraWrite::Eased {
            now: 0.0,
            duration: DURATION,
        },
    );
    assert!(state.camera.is_moving());
    let _ = apply_camera_request(
        &mut state,
        &CameraRequest {
            preset: Some(ViewPreset::Top),
            ..CameraRequest::default()
        },
    )
    .expect("a preset is never refused");
    assert!(
        !state.camera.is_moving(),
        "the MCP write drops the GUI move"
    );
}

/// The screen x of the world point `p` in a 1600 x 900 view of `camera`.
fn screen_x(camera: &OrbitCamera, p: [f32; 3]) -> f32 {
    camera
        .project_to_screen(p, 16.0 / 9.0, 1600.0, 900.0)
        .expect("in front of the camera")[0]
}

#[test]
fn the_front_and_right_presets_look_from_minus_y_and_plus_x_cam4() {
    let mut front = OrbitCamera::new();
    front.target = Point3::origin();
    front.set_pose(front.preset_pose(ViewPreset::Front));
    assert!(
        front.eye().y < -1.0,
        "Front looks from -Y: eye {:?}",
        front.eye()
    );
    assert!(
        screen_x(&front, [10.0, 0.0, 0.0]) > screen_x(&front, [0.0, 0.0, 0.0]),
        "Front puts +X to the right of the screen"
    );

    let mut right = OrbitCamera::new();
    right.target = Point3::origin();
    right.set_pose(right.preset_pose(ViewPreset::Right));
    assert!(
        right.eye().x > 1.0,
        "Right looks from +X: eye {:?}",
        right.eye()
    );
    assert!(
        screen_x(&right, [0.0, 10.0, 0.0]) > screen_x(&right, [0.0, 0.0, 0.0]),
        "Right puts +Y to the right of the screen"
    );

    // Top: +X to the right and +Y up on the screen (screen y grows down).
    let mut top = OrbitCamera::new();
    top.target = Point3::origin();
    top.set_pose(top.preset_pose(ViewPreset::Top));
    let at = |camera: &OrbitCamera, p: [f32; 3]| {
        camera
            .project_to_screen(p, 16.0 / 9.0, 1600.0, 900.0)
            .expect("in front of the camera")
    };
    let origin = at(&top, [0.0, 0.0, 0.0]);
    let plus_x = at(&top, [10.0, 0.0, 0.0]);
    let plus_y = at(&top, [0.0, 10.0, 0.0]);
    assert!(
        plus_x[0] - origin[0] > (plus_x[1] - origin[1]).abs(),
        "Top puts +X to the right of the screen: {origin:?} -> {plus_x:?}"
    );
    assert!(
        origin[1] - plus_y[1] > (plus_y[0] - origin[0]).abs(),
        "Top puts +Y up on the screen: {origin:?} -> {plus_y:?}"
    );

    // Isometric: the eye is front-right and above (+X, -Y, +Z).
    let mut iso = OrbitCamera::new();
    iso.target = Point3::origin();
    iso.set_pose(iso.preset_pose(ViewPreset::Isometric));
    let eye = iso.eye();
    assert!(
        eye.x > 1.0 && eye.y < -1.0 && eye.z > 1.0,
        "Isometric looks from +X, -Y, +Z: eye {eye:?}"
    );

    // The orientation preview of a side is the preset of that side.
    for (face, preset) in [
        (FaceUp::Front, ViewPreset::Front),
        (FaceUp::Right, ViewPreset::Right),
    ] {
        let mut by_face = CameraState::default();
        let (yaw, pitch) = face_up_orientation(face, by_face.pose().yaw);
        by_face.orientation(yaw, pitch, CameraWrite::Instant);
        let mut by_preset = CameraState::default();
        by_preset.preset(preset, CameraWrite::Instant);
        assert_eq!(by_face.pose(), by_preset.pose(), "{face:?} vs {preset:?}");
    }
}

fn code(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path)
        .expect("read source")
        .lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn only_the_frame_gate_repaints_for_the_camera_cam4() {
    let app = code("src/app.rs");
    let at = app
        .find("fn draw_frame")
        .expect("non-vacuity: app.rs no longer has `fn draw_frame`");
    let frame = &app[at..];
    let advance = frame
        .find(".advance(ctx.input(|i| i.time))")
        .expect("the frame no longer advances the camera move");
    let gate = frame
        .find("|| camera_moving {")
        .expect("the repaint gate no longer reads the camera answer");
    assert!(advance < gate, "the gate reads the answer before it exists");
    assert!(
        frame[..advance].contains("let camera_moving = self"),
        "the advance answer is not kept for the gate"
    );
    for file in [
        "src/state/camera.rs",
        "src/render/camera.rs",
        "src/interaction/input.rs",
    ] {
        assert!(
            !code(file).contains("request_repaint"),
            "{file} asks for a repaint; only the frame gate may do that for the camera"
        );
    }
}
