//! CAM-10 (R5b, operator ruling, live look round 5): the view cube faces
//! map to the six view presets.
//!
//! # The claims
//!
//! 1. For each of the seven presets, a click at the centre of the cube
//!    square gives the face that looks at the eye: Top, Bottom, Front,
//!    Back, Right, Left; at Isometric the centre is the corner where Top,
//!    Front and Right meet, so it gives one of those three.
//! 2. For several camera poses, the centre of each visible face gives that
//!    face's preset, and that preset turns that face to the eye (its
//!    facing becomes the largest of the cube, near 1). So a click on the
//!    word "Front" gives the view that looks at the Front face.
//! 3. A point outside the cube square gives no face, and `on_cube` holds
//!    for the square only.
//! 4. Visible faces never overlap: at most three, and each visible face
//!    faces the eye by more than `VISIBLE_COS`.
//!
//! Pure: no egui context, no painter.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test the_view_cube_faces_map_to_the_six_presets_cam10
//! ```

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_viz::interaction::view_cube::{VISIBLE_COS, cube_rect, face_at, on_cube, visible_faces};
use rs_cam_viz::render::camera::{CameraPose, OrbitCamera, ViewPreset};

fn viewport() -> egui::Rect {
    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0))
}

fn camera_at(preset: ViewPreset) -> OrbitCamera {
    let mut camera = OrbitCamera::new();
    camera.set_pose(camera.preset_pose(preset));
    camera
}

fn camera_yaw_pitch(yaw_deg: f32, pitch_deg: f32) -> OrbitCamera {
    let mut camera = OrbitCamera::new();
    let pose = camera.pose();
    camera.set_pose(CameraPose {
        yaw: yaw_deg.to_radians(),
        pitch: pitch_deg.to_radians(),
        ..pose
    });
    camera
}

#[test]
fn the_centre_of_the_cube_is_the_face_of_the_preset_cam10() {
    let rect = cube_rect(viewport());
    for preset in [
        ViewPreset::Top,
        ViewPreset::Bottom,
        ViewPreset::Front,
        ViewPreset::Back,
        ViewPreset::Right,
        ViewPreset::Left,
    ] {
        let camera = camera_at(preset);
        assert_eq!(
            face_at(rect, &camera, rect.center()),
            Some(preset),
            "the centre of the cube at {preset:?}"
        );
        let faces = visible_faces(rect, &camera);
        assert_eq!(
            faces.len(),
            1,
            "{preset:?} shows one face, not slivers: {faces:?}"
        );
    }
    let iso = face_at(rect, &camera_at(ViewPreset::Isometric), rect.center())
        .expect("the iso centre is on the cube");
    assert!(
        matches!(iso, ViewPreset::Top | ViewPreset::Front | ViewPreset::Right),
        "the iso corner is where Top, Front and Right meet: {iso:?}"
    );
}

#[test]
fn a_face_click_gives_the_view_that_looks_at_it_cam10() {
    let rect = cube_rect(viewport());
    let poses = [
        (-45.0, 30.0),
        (-135.0, 30.0),
        (135.0, 40.0),
        (45.0, -30.0),
        (-60.0, -50.0),
        (170.0, 10.0),
    ];
    let mut checked = 0;
    for (yaw, pitch) in poses {
        let camera = camera_yaw_pitch(yaw, pitch);
        let faces = visible_faces(rect, &camera);
        assert!(
            (1..=3).contains(&faces.len()),
            "({yaw}, {pitch}): {} visible faces",
            faces.len()
        );
        for face in &faces {
            assert!(face.facing > VISIBLE_COS, "a visible face faces the eye");
            let picked = face_at(rect, &camera, face.centre());
            assert_eq!(
                picked,
                Some(face.preset),
                "({yaw}, {pitch}): the centre of {} gives its preset",
                face.label
            );
            // The preset turns that face to the eye.
            let after = camera_at(face.preset);
            let turned = visible_faces(rect, &after);
            let front = turned
                .iter()
                .max_by(|a, b| a.facing.total_cmp(&b.facing))
                .expect("a face faces the eye");
            assert_eq!(
                front.preset, face.preset,
                "({yaw}, {pitch}): {} turns its own face to the eye",
                face.label
            );
            assert!(front.facing > 0.99, "face-on after the preset");
            checked += 1;
        }
    }
    assert!(checked >= 12, "only {checked} faces checked");
}

#[test]
fn off_the_cube_gives_no_face_cam10() {
    let view = viewport();
    let rect = cube_rect(view);
    let camera = camera_at(ViewPreset::Isometric);
    for pos in [
        rect.left_top() - egui::vec2(2.0, 0.0),
        rect.right_bottom() + egui::vec2(0.0, 2.0),
        view.center(),
    ] {
        assert_eq!(face_at(rect, &camera, pos), None, "{pos:?} is off the cube");
        assert!(!on_cube(view, pos), "{pos:?} is off the cube square");
    }
    assert!(on_cube(view, rect.center()));
    assert!(
        rect.right() < view.right() && rect.top() > view.top(),
        "the cube sits inside the top-right corner"
    );
}
