//! CAM-2 (design A, commit C1): the wheel zooms toward the point under the
//! cursor.
//!
//! # The claim
//!
//! `OrbitCamera::zoom_toward(P, k)` is a homothety about `P` with the
//! clamped distance ratio. The view basis does not change, so `P` keeps its
//! pixel in the perspective and the orthographic projection. The claim holds
//! for a point at the target depth and for a point off that depth.
//!
//! The clamp arm: at the low end of `DISTANCE_RANGE` a zoom in is a no-op.
//! The target does not move, because the clamped ratio is 1.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use nalgebra::{Point3, Vector3};
use rs_cam_viz::render::camera::{DISTANCE_RANGE, OrbitCamera, ProjectionMode};

const VW: f32 = 1600.0;
const VH: f32 = 900.0;
const TOL_PX: f32 = 1e-2;

/// The tolerance of the set-up check only. `unproject_ray` inverts the f32
/// view-projection matrix (near 0.1, far 10 000), and a measured round trip
/// is off by up to about 0.04 px. The zoom claim itself uses `TOL_PX`.
const SETUP_TOL_PX: f32 = 0.25;

fn camera(projection: ProjectionMode) -> OrbitCamera {
    let mut cam = OrbitCamera::new();
    cam.target = Point3::new(30.0, -12.0, 4.0);
    cam.distance = 150.0;
    cam.yaw = 0.7;
    cam.pitch = 0.45;
    cam.projection = projection;
    cam
}

fn pixel(cam: &OrbitCamera, p: Point3<f32>) -> [f32; 2] {
    cam.project_to_screen([p.x, p.y, p.z], VW / VH, VW, VH)
        .expect("the point is in front of the camera")
}

#[test]
fn the_zoom_keeps_the_point_under_the_cursor_cam2() {
    let cursors = [
        ("centre", [VW * 0.5, VH * 0.5]),
        ("corner", [20.0, 15.0]),
        ("edge", [VW - 10.0, VH * 0.5]),
    ];
    let mut checked = 0;
    for projection in [ProjectionMode::Perspective, ProjectionMode::Orthographic] {
        for (name, [sx, sy]) in cursors {
            for k in [0.5_f32, 0.9, 1.1, 2.0] {
                let base = camera(projection);
                let at_depth = base
                    .point_at_target_depth(sx, sy, VW, VH)
                    .expect("the ray meets the target plane");
                // The same pixel, 40 units nearer the eye than the target
                // plane: a point off the target depth.
                let (origin, dir) = base.unproject_ray(sx, sy, VW / VH, VW, VH).expect("a ray");
                let t = (at_depth - origin).dot(&dir) - 40.0;
                let off_depth = origin + dir * t;
                for (which, p) in [("at depth", at_depth), ("off depth", off_depth)] {
                    let before = pixel(&base, p);
                    assert!(
                        (before[0] - sx).abs() < SETUP_TOL_PX
                            && (before[1] - sy).abs() < SETUP_TOL_PX,
                        "{projection:?} {name} {which}: P is not under the cursor before the zoom: {before:?} vs ({sx}, {sy})"
                    );
                    let mut cam = base;
                    cam.zoom_toward(p, k);
                    let after = pixel(&cam, p);
                    let moved = (after[0] - before[0]).hypot(after[1] - before[1]);
                    assert!(
                        moved < TOL_PX,
                        "{projection:?} {name} {which} k={k}: P moved {moved} px"
                    );
                    let want = base.distance * k;
                    assert!(
                        (cam.distance - want).abs() < 1e-3 * want,
                        "{projection:?} {name} {which} k={k}: distance {} != {want}",
                        cam.distance
                    );
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 2 * 3 * 4 * 2, "every arm ran");
}

#[test]
fn a_clamped_zoom_moves_nothing_cam2() {
    for projection in [ProjectionMode::Perspective, ProjectionMode::Orthographic] {
        let mut cam = camera(projection);
        cam.distance = DISTANCE_RANGE.0;
        let before = cam.pose();
        let p = cam.target + Vector3::new(3.0, -2.0, 1.0);
        cam.zoom_toward(p, 0.5);
        assert_eq!(
            cam.pose(),
            before,
            "{projection:?}: a zoom in at the low end of DISTANCE_RANGE changed the pose"
        );
    }
}
