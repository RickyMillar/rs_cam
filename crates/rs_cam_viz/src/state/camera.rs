//! The viewport camera as GUI view state (design A, commit C2).
//!
//! `CameraState` holds the `OrbitCamera`. The field is private, so every
//! pose write goes through a method here. The compiler holds that rule. The
//! camera math stays in `render::camera`; this wrapper adds no arithmetic.
//!
//! F096: the camera is view state. It is not a `Command`, it does not write
//! the session, and it does not stale a result. The project file and the
//! setup sheet do not hold it.

use nalgebra::Point3;

use crate::render::camera::{CameraPose, OrbitCamera, ProjectionMode, ViewPreset};

/// The viewport camera. Read it with [`CameraState::pose`]; write it with
/// the other methods.
#[derive(Debug, Clone, Default)]
pub struct CameraState {
    pose: OrbitCamera,
}

impl CameraState {
    /// The camera: the one read door (view-projection, eye, projection to
    /// the screen, the pick ray).
    pub fn pose(&self) -> &OrbitCamera {
        &self.pose
    }

    /// Write a pose. Every pose write ends here. `OrbitCamera::set_pose`
    /// clamps the pitch and the distance.
    pub fn set_pose(&mut self, to: CameraPose) {
        self.pose.set_pose(to);
    }

    /// Turn the camera to a view preset. The target and the distance do not
    /// change.
    pub fn preset(&mut self, p: ViewPreset) {
        self.set_pose(self.pose.preset_pose(p));
    }

    /// Turn the camera to `yaw` and `pitch`. The target and the distance do
    /// not change.
    pub fn orientation(&mut self, yaw: f32, pitch: f32) {
        self.set_pose(CameraPose {
            yaw,
            pitch,
            ..self.pose.pose()
        });
    }

    /// Frame the box `min`..`max` (`OrbitCamera::fit_pose`).
    pub fn fit(&mut self, min: [f32; 3], max: [f32; 3]) {
        self.set_pose(self.pose.fit_pose(min, max));
    }

    /// Frame the box `bounds`. With no box, put the camera back to its
    /// start pose and projection. Reset View and the job-load fit call this
    /// with [`first_model_bounds`].
    pub fn fit_or_reset(&mut self, bounds: Option<([f32; 3], [f32; 3])>) {
        if let Some((min, max)) = bounds {
            self.fit(min, max);
        } else {
            let start = OrbitCamera::new();
            self.set_pose(start.pose());
            self.set_projection(start.projection);
        }
    }

    /// Set the projection mode.
    pub fn set_projection(&mut self, mode: ProjectionMode) {
        self.pose.projection = mode;
    }

    /// Toggle between the perspective and the orthographic projection.
    pub fn toggle_projection(&mut self) {
        self.pose.toggle_projection();
    }

    /// Orbit about `pivot` by a drag of `(dx, dy)` pixels.
    pub fn orbit_about(&mut self, pivot: Point3<f32>, dx: f32, dy: f32) {
        let mut next = self.pose;
        next.orbit_about(pivot, dx, dy);
        self.set_pose(next.pose());
    }

    /// Zoom by the distance ratio `k` toward `point`.
    pub fn zoom_toward(&mut self, point: Point3<f32>, k: f32) {
        let mut next = self.pose;
        next.zoom_toward(point, k);
        self.set_pose(next.pose());
    }

    /// Pan by a drag of `(dx, dy)` pixels in a viewport `vh` pixels high.
    pub fn pan_pixels(&mut self, dx: f32, dy: f32, vh: f32, grab_depth: Option<f32>) {
        let mut next = self.pose;
        next.pan_pixels(dx, dy, vh, grab_depth);
        self.set_pose(next.pose());
    }
}

/// The bounding box of the first model that has one, as `(min, max)`.
/// Reset View and the job-load fit frame this box.
pub fn first_model_bounds(
    session: &rs_cam_core::session::ProjectSession,
) -> Option<([f32; 3], [f32; 3])> {
    session
        .models()
        .iter()
        .find_map(rs_cam_core::session::LoadedModel::bbox)
        .map(|bbox| bbox_corners(&bbox))
}

/// Frame the first model of the project, or put the camera back to its
/// start when the project has no model. Reset View, the job load and the
/// import confirm with no box call this one routine.
pub fn fit_to_first_model(state: &mut super::AppState) {
    let bounds = first_model_bounds(state.session());
    state.camera.fit_or_reset(bounds);
}

/// The corners of a core box as the `f32` arrays that the camera takes.
pub fn bbox_corners(bbox: &rs_cam_core::geo::BoundingBox3) -> ([f32; 3], [f32; 3]) {
    (
        [bbox.min.x as f32, bbox.min.y as f32, bbox.min.z as f32],
        [bbox.max.x as f32, bbox.max.y as f32, bbox.max.z as f32],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cast(m: [[f32; 4]; 4]) -> [[u32; 4]; 4] {
        m.map(|row| row.map(f32::to_bits))
    }

    #[test]
    fn the_camera_state_has_one_write_door_c2() {
        // `set_pose` clamps the pitch and the distance.
        let mut state = CameraState::default();
        state.set_pose(CameraPose {
            target: Point3::new(1.0, 2.0, 3.0),
            distance: 0.0,
            yaw: 0.4,
            pitch: 3.0,
        });
        let p = state.pose().pose();
        assert_eq!(p.distance, crate::render::camera::DISTANCE_RANGE.0);
        assert_eq!(p.pitch, crate::render::camera::PITCH_LIMIT);

        // `preset` is `set_pose(preset_pose)`: the matrix is bitwise equal.
        for preset in [
            ViewPreset::Top,
            ViewPreset::Front,
            ViewPreset::Right,
            ViewPreset::Isometric,
        ] {
            let mut state = CameraState::default();
            state.preset(preset);
            let mut cam = OrbitCamera::new();
            cam.set_pose(cam.preset_pose(preset));
            assert_eq!(
                cast(state.pose().view_proj(16.0 / 9.0)),
                cast(cam.view_proj(16.0 / 9.0)),
                "{preset:?}"
            );
        }

        // `fit` is `set_pose(fit_pose)`.
        let (min, max) = ([-5.0, 2.0, -1.0], [40.0, 22.0, 9.5]);
        let mut state = CameraState::default();
        state.fit(min, max);
        let mut cam = OrbitCamera::new();
        cam.set_pose(cam.fit_pose(min, max));
        assert_eq!(*state.pose(), cam);
        assert_eq!(
            cast(state.pose().view_proj(16.0 / 9.0)),
            cast(cam.view_proj(16.0 / 9.0))
        );

        // `fit_or_reset` with no box gives the start camera back.
        state.toggle_projection();
        state.fit_or_reset(None);
        assert_eq!(*state.pose(), OrbitCamera::new());
    }
}
