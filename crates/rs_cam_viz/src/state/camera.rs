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
use rs_cam_core::geo::BoundingBox3;

use super::AppState;
use super::selection::Selection;
use crate::render::camera::{
    CameraPose, DISTANCE_RANGE, OrbitCamera, PITCH_LIMIT, ProjectionMode, ViewPreset,
};

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
    first_model_bbox(session).map(|bbox| bbox_corners(&bbox))
}

/// The bounding box of the first model that has one.
fn first_model_bbox(session: &rs_cam_core::session::ProjectSession) -> Option<BoundingBox3> {
    session
        .models()
        .iter()
        .find_map(rs_cam_core::session::LoadedModel::bbox)
}

/// Frame the first model of the project, or put the camera back to its
/// start when the project has no model. Reset View, the job load and the
/// import confirm with no box call this one routine.
pub fn fit_to_first_model(state: &mut AppState) {
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

// ── The MCP camera door (design A, C5) ─────────────────────────────────

/// What `set_ui_view camera.fit` frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FitTarget {
    /// The first model: the Reset View rule ([`first_model_bounds`]).
    Model,
    /// The picked items of the selected toolpath ([`selection_fit_bounds`]).
    Selection,
}

/// A pose patch in radians. An omitted field keeps its value.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PosePatch {
    pub target: Option<Point3<f32>>,
    pub distance: Option<f32>,
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
}

/// The camera part of a `set_ui_view` call, in crate types.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CameraRequest {
    pub preset: Option<ViewPreset>,
    pub projection: Option<ProjectionMode>,
    pub fit: Option<FitTarget>,
    pub pose: PosePatch,
}

/// The camera after a call: what the `set_ui_view` reply and `get_ui_view`
/// echo.
#[derive(Debug, Clone, Copy)]
pub struct CameraEcho {
    pub pose: CameraPose,
    pub projection: ProjectionMode,
    /// The vertical field of view, radians.
    pub fov: f32,
    /// The preset whose yaw and pitch the pose holds exactly, if any.
    pub view_preset: Option<ViewPreset>,
    /// The box that this call framed, if it framed one.
    pub framed: Option<BoundingBox3>,
}

/// The view presets, in echo order.
const PRESETS: [ViewPreset; 4] = [
    ViewPreset::Top,
    ViewPreset::Front,
    ViewPreset::Right,
    ViewPreset::Isometric,
];

/// The camera of `state` as an echo, with no framed box.
pub fn camera_echo(state: &AppState) -> CameraEcho {
    let camera = state.camera.pose();
    let pose = camera.pose();
    let view_preset = PRESETS.into_iter().find(|p| {
        let preset = camera.preset_pose(*p);
        preset.yaw == pose.yaw && preset.pitch == pose.pitch
    });
    CameraEcho {
        pose,
        projection: camera.projection,
        fov: camera.fov,
        view_preset,
        framed: None,
    }
}

/// The box that `fit: selection` frames: the picked items of the
/// toolpath that `focus` names, through the "Show me" bounds routine
/// (`display_frame::selection_frame_bounds`). `None` with no toolpath focus or no
/// picked item.
fn selection_fit_bounds_for(state: &AppState, focus: &Selection) -> Option<BoundingBox3> {
    let Selection::Toolpath(id) = *focus else {
        return None;
    };
    let session = state.session();
    let (_, tc) = session.find_toolpath_config_by_id(id)?;
    let target = super::selection_highlight::highlight_target(tc);
    let report = session.selection_status(tc, target).ok()?;
    if report.selected.is_empty() {
        return None;
    }
    crate::state::display_frame::selection_frame_bounds(session, id, &report.selected)
}

/// The box that `fit: selection` frames for the present focus.
pub fn selection_fit_bounds(state: &AppState) -> Option<BoundingBox3> {
    selection_fit_bounds_for(state, state.focus())
}

/// Check every field of `req` before any part of a call applies.
///
/// `focus` is the focus that the call leaves when the camera applies (a
/// `toolpath_index` in the same call moves it). `with_frame_items` is true
/// when the same call frames items: two framings in one call are refused.
pub fn check_camera_request(
    state: &AppState,
    req: &CameraRequest,
    focus: &Selection,
    with_frame_items: bool,
) -> Result<(), String> {
    let PosePatch {
        target,
        distance,
        yaw,
        pitch,
    } = req.pose;
    if let Some(t) = target
        && !(t.x.is_finite() && t.y.is_finite() && t.z.is_finite())
    {
        return Err("camera.pose.target must be three finite numbers".to_owned());
    }
    if let Some(d) = distance
        && !(d.is_finite() && (DISTANCE_RANGE.0..=DISTANCE_RANGE.1).contains(&d))
    {
        return Err(format!(
            "camera.pose.distance must be a finite number in [{}, {}] mm",
            DISTANCE_RANGE.0, DISTANCE_RANGE.1
        ));
    }
    if let Some(y) = yaw
        && !y.is_finite()
    {
        return Err("camera.pose.yaw_deg must be a finite number".to_owned());
    }
    if let Some(p) = pitch
        && !(p.is_finite() && p.abs() <= PITCH_LIMIT)
    {
        return Err(format!(
            "camera.pose.pitch_deg must be a finite number in [-{0:.3}, {0:.3}] degrees",
            f64::from(PITCH_LIMIT).to_degrees()
        ));
    }
    match req.fit {
        Some(_) if with_frame_items => {
            Err("camera.fit and frame_items both frame the camera; send one".to_owned())
        }
        Some(FitTarget::Model) if first_model_bbox(state.session()).is_none() => {
            Err("camera.fit model needs a model; the project has none".to_owned())
        }
        Some(FitTarget::Selection) if selection_fit_bounds_for(state, focus).is_none() => {
            Err("fit selection needs a selected toolpath with picked items".to_owned())
        }
        Some(FitTarget::Model | FitTarget::Selection) | None => Ok(()),
    }
}

/// The one door MCP uses to move the camera. It checks every field first
/// ([`check_camera_request`] with the present focus); a refusal changes
/// nothing. Then it applies in the order fit, preset, projection, pose.
/// Every write is instant.
pub fn apply_camera_request(
    state: &mut AppState,
    req: &CameraRequest,
) -> Result<CameraEcho, String> {
    let focus = state.focus().clone();
    check_camera_request(state, req, &focus, false)?;
    let framed = match req.fit {
        Some(FitTarget::Model) => first_model_bbox(state.session()),
        Some(FitTarget::Selection) => selection_fit_bounds(state),
        None => None,
    };
    if let Some(bbox) = framed {
        let (min, max) = bbox_corners(&bbox);
        state.camera.fit(min, max);
    }
    if let Some(preset) = req.preset {
        state.camera.preset(preset);
    }
    if let Some(mode) = req.projection {
        state.camera.set_projection(mode);
    }
    let PosePatch {
        target,
        distance,
        yaw,
        pitch,
    } = req.pose;
    let now = state.camera.pose().pose();
    state.camera.set_pose(CameraPose {
        target: target.unwrap_or(now.target),
        distance: distance.unwrap_or(now.distance),
        yaw: yaw.unwrap_or(now.yaw),
        pitch: pitch.unwrap_or(now.pitch),
    });
    Ok(CameraEcho {
        framed,
        ..camera_echo(state)
    })
}

/// The wire mirrors (`rs_cam_mcp`): an exhaustive `match` in each
/// direction (F166), and a full destructure of the camera object (F118).
/// Degrees on the wire, radians here (A-Q5).
#[cfg(feature = "mcp")]
mod wire {
    use super::{CameraRequest, FitTarget, PosePatch, ProjectionMode, ViewPreset};
    use nalgebra::Point3;
    use rs_cam_mcp::server::{
        CameraParam, CameraPoseParam, FitParam, ProjectionParam, ViewPresetParam,
    };

    impl From<ViewPresetParam> for ViewPreset {
        fn from(p: ViewPresetParam) -> Self {
            match p {
                ViewPresetParam::Top => Self::Top,
                ViewPresetParam::Front => Self::Front,
                ViewPresetParam::Right => Self::Right,
                ViewPresetParam::Iso => Self::Isometric,
            }
        }
    }

    impl From<ViewPreset> for ViewPresetParam {
        fn from(p: ViewPreset) -> Self {
            match p {
                ViewPreset::Top => Self::Top,
                ViewPreset::Front => Self::Front,
                ViewPreset::Right => Self::Right,
                ViewPreset::Isometric => Self::Iso,
            }
        }
    }

    impl From<ProjectionParam> for ProjectionMode {
        fn from(p: ProjectionParam) -> Self {
            match p {
                ProjectionParam::Perspective => Self::Perspective,
                ProjectionParam::Orthographic => Self::Orthographic,
            }
        }
    }

    impl From<ProjectionMode> for ProjectionParam {
        fn from(p: ProjectionMode) -> Self {
            match p {
                ProjectionMode::Perspective => Self::Perspective,
                ProjectionMode::Orthographic => Self::Orthographic,
            }
        }
    }

    impl From<FitParam> for FitTarget {
        fn from(p: FitParam) -> Self {
            match p {
                FitParam::Model => Self::Model,
                FitParam::Selection => Self::Selection,
            }
        }
    }

    impl From<FitTarget> for FitParam {
        fn from(p: FitTarget) -> Self {
            match p {
                FitTarget::Model => Self::Model,
                FitTarget::Selection => Self::Selection,
            }
        }
    }

    impl From<CameraParam> for CameraRequest {
        fn from(param: CameraParam) -> Self {
            let CameraParam {
                preset,
                projection,
                fit,
                pose,
            } = param;
            let CameraPoseParam {
                target,
                distance,
                yaw_deg,
                pitch_deg,
            } = pose.unwrap_or_default();
            Self {
                preset: preset.map(Into::into),
                projection: projection.map(Into::into),
                fit: fit.map(Into::into),
                pose: PosePatch {
                    target: target.map(|[x, y, z]| Point3::new(x as f32, y as f32, z as f32)),
                    distance: distance.map(|d| d as f32),
                    yaw: yaw_deg.map(|d| d.to_radians() as f32),
                    pitch: pitch_deg.map(|d| d.to_radians() as f32),
                },
            }
        }
    }
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
