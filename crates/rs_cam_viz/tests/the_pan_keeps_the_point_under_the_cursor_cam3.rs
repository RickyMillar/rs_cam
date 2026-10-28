//! CAM-3 (design A, commit C1): the pan step comes from the projection.
//!
//! # The claim
//!
//! `OrbitCamera::pan_pixels(dx, dy, vh, Some(depth))` moves a point at the
//! camera depth `depth` by exactly `(dx, dy)` pixels, in the perspective and
//! the orthographic projection, at each viewport height.
//!
//! The anchor arm (claim C8 of the viewport parity page): the old pan step
//! was `0.001 * distance` per pixel. The derived step
//! `world_per_pixel(distance, vh)` equals it only at
//! `vh = 2 * tan(fov / 2) / 0.001`.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use nalgebra::Point3;
use rs_cam_viz::render::camera::{OrbitCamera, ProjectionMode};

const TOL_PX: f32 = 1e-2;

fn camera(projection: ProjectionMode) -> OrbitCamera {
    let mut cam = OrbitCamera::new();
    cam.target = Point3::new(-8.0, 22.0, 3.0);
    cam.distance = 120.0;
    cam.yaw = -0.4;
    cam.pitch = 0.6;
    cam.projection = projection;
    cam
}

#[test]
fn the_pan_keeps_the_point_under_the_cursor_cam3() {
    let vw = 1600.0_f32;
    let mut checked = 0;
    for projection in [ProjectionMode::Perspective, ProjectionMode::Orthographic] {
        for vh in [400.0_f32, 828.0, 1400.0] {
            let aspect = vw / vh;
            let base = camera(projection);
            // A grabbed point off the centre and off the target depth.
            let (origin, dir) = base
                .unproject_ray(vw * 0.3, vh * 0.7, aspect, vw, vh)
                .expect("a ray");
            let on_plane = base
                .point_at_target_depth(vw * 0.3, vh * 0.7, vw, vh)
                .expect("the ray meets the target plane");
            let t = (on_plane - origin).dot(&dir) - 25.0;
            let grabbed = origin + dir * t;
            let depth = (grabbed - base.basis().eye).dot(&base.basis().forward);
            for (dx, dy) in [(37.0_f32, -12.0_f32), (-5.0, 60.0), (0.5, 0.25)] {
                let before = base
                    .project_to_screen([grabbed.x, grabbed.y, grabbed.z], aspect, vw, vh)
                    .expect("in view");
                let mut cam = base;
                cam.pan_pixels(dx, dy, vh, Some(depth));
                let after = cam
                    .project_to_screen([grabbed.x, grabbed.y, grabbed.z], aspect, vw, vh)
                    .expect("in view");
                let ex = after[0] - before[0] - dx;
                let ey = after[1] - before[1] - dy;
                assert!(
                    ex.abs() < TOL_PX && ey.abs() < TOL_PX,
                    "{projection:?} vh={vh} drag=({dx},{dy}): the point moved \
                     ({}, {}) px",
                    after[0] - before[0],
                    after[1] - before[1]
                );
                assert_eq!(cam.distance, base.distance, "a pan keeps the distance");
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 2 * 3 * 3, "every arm ran");
}

#[test]
fn the_old_step_is_the_rule_at_one_height_c8() {
    for projection in [ProjectionMode::Perspective, ProjectionMode::Orthographic] {
        let cam = camera(projection);
        let vh = 2.0 * (cam.fov * 0.5).tan() / 0.001;
        let derived = cam.world_per_pixel(cam.distance, vh);
        let old = 0.001 * cam.distance;
        assert!(
            ((derived - old) / old).abs() < 1e-6,
            "{projection:?}: at vh={vh} the derived step {derived} != the old step {old}"
        );
    }
}
