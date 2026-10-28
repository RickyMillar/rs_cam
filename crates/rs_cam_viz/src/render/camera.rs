use nalgebra::{Matrix4, Point3, Vector3};

/// View preset orientations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewPreset {
    Top,
    Front,
    Right,
    Isometric,
}

/// Camera projection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionMode {
    Perspective,
    Orthographic,
}

/// The largest pitch magnitude. The camera never looks straight down the Z
/// axis, so the view matrix keeps a defined up vector.
pub const PITCH_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - 0.01;

/// The range that holds the orbit distance.
pub const DISTANCE_RANGE: (f32, f32) = (0.1, 50_000.0);

/// The orbit angle for one pixel of drag.
pub const ORBIT_RAD_PER_PX: f32 = 0.005;

/// The zoom rate for one unit of wheel scroll: `k = exp(-scroll * ZOOM_PER_WHEEL)`.
pub const ZOOM_PER_WHEEL: f32 = 0.001;

/// The yaw that looks at the front of the stock: the eye on -Y, +X to the
/// right of the screen (the Z-up CAD convention, design A Q1).
pub const YAW_FRONT: f32 = -std::f32::consts::FRAC_PI_2;

/// The yaw that looks at the right side: the eye on +X, +Y to the right of
/// the screen.
pub const YAW_RIGHT: f32 = 0.0;

/// The yaw that looks at the back: the eye on +Y, -X to the right.
pub const YAW_BACK: f32 = std::f32::consts::FRAC_PI_2;

/// The yaw that looks at the left side: the eye on -X, -Y to the right.
pub const YAW_LEFT: f32 = std::f32::consts::PI;

/// The yaw of the isometric preset: the mean of [`YAW_FRONT`] and
/// [`YAW_RIGHT`], so the eye sits front-right (+X, -Y).
pub const YAW_ISO: f32 = (YAW_FRONT + YAW_RIGHT) / 2.0;

/// The part of the camera state that a navigation gesture writes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraPose {
    pub target: Point3<f32>,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
}

/// The eye and the three unit axes of the view, in world space. They are
/// the same axes that `OrbitCamera::view_matrix` uses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraBasis {
    pub eye: Point3<f32>,
    /// The unit vector from the eye to the target.
    pub forward: Vector3<f32>,
    /// The unit vector to the right of the screen.
    pub right: Vector3<f32>,
    /// The unit vector to the top of the screen.
    pub up: Vector3<f32>,
}

/// Orbit camera for the 3D viewport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrbitCamera {
    pub target: Point3<f32>,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
    pub near: f32,
    pub far: f32,
    pub projection: ProjectionMode,
}

impl OrbitCamera {
    pub fn new() -> Self {
        Self {
            target: Point3::new(0.0, 0.0, 0.0),
            distance: 200.0,
            yaw: std::f32::consts::FRAC_PI_4,
            pitch: std::f32::consts::FRAC_PI_6,
            fov: 45.0_f32.to_radians(),
            near: 0.1,
            far: 10000.0,
            projection: ProjectionMode::Perspective,
        }
    }

    /// Toggle between perspective and orthographic projection.
    pub fn toggle_projection(&mut self) {
        self.projection = match self.projection {
            ProjectionMode::Perspective => ProjectionMode::Orthographic,
            ProjectionMode::Orthographic => ProjectionMode::Perspective,
        };
    }

    /// Camera position in world space.
    pub fn eye(&self) -> Point3<f32> {
        let x = self.distance * self.pitch.cos() * self.yaw.cos();
        let y = self.distance * self.pitch.cos() * self.yaw.sin();
        let z = self.distance * self.pitch.sin();
        self.target + Vector3::new(x, y, z)
    }

    /// View matrix (world -> camera).
    pub fn view_matrix(&self) -> Matrix4<f32> {
        let eye = self.eye();
        let up = Vector3::new(0.0, 0.0, 1.0);
        Matrix4::look_at_rh(&eye, &self.target, &up)
    }

    /// Projection matrix (camera -> clip).
    pub fn projection_matrix(&self, aspect: f32) -> Matrix4<f32> {
        match self.projection {
            ProjectionMode::Perspective => {
                Matrix4::new_perspective(aspect, self.fov, self.near, self.far)
            }
            ProjectionMode::Orthographic => {
                // Match the perspective frustum's visible half-height at the
                // orbit target so toggling feels like a "flatten" rather than
                // a jump. Near/far must allow content on both sides of the
                // target since the origin is clamped by the orbit distance.
                let half_h = self.distance * (self.fov * 0.5).tan();
                let half_w = half_h * aspect;
                Matrix4::new_orthographic(-half_w, half_w, -half_h, half_h, -self.far, self.far)
            }
        }
    }

    /// Combined view-projection matrix.
    #[expect(
        clippy::indexing_slicing,
        reason = "nalgebra 4x4 matrix slice is always 16 elements"
    )]
    pub fn view_proj(&self, aspect: f32) -> [[f32; 4]; 4] {
        let vp = self.projection_matrix(aspect) * self.view_matrix();
        let s = vp.as_slice();
        [
            [s[0], s[1], s[2], s[3]],
            [s[4], s[5], s[6], s[7]],
            [s[8], s[9], s[10], s[11]],
            [s[12], s[13], s[14], s[15]],
        ]
    }

    /// The navigation pose: target, distance, yaw and pitch.
    pub fn pose(&self) -> CameraPose {
        CameraPose {
            target: self.target,
            distance: self.distance,
            yaw: self.yaw,
            pitch: self.pitch,
        }
    }

    /// Write a pose. The function clamps the pitch to `PITCH_LIMIT` and the
    /// distance to `DISTANCE_RANGE`.
    pub fn set_pose(&mut self, pose: CameraPose) {
        self.target = pose.target;
        self.distance = pose.distance.clamp(DISTANCE_RANGE.0, DISTANCE_RANGE.1);
        self.yaw = pose.yaw;
        self.pitch = pose.pitch.clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// The eye and the view axes. `right` and `up` are the axes of
    /// `Matrix4::look_at_rh` with the world Z axis as up.
    pub fn basis(&self) -> CameraBasis {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        // The eye sits at `target - forward * distance` (see `eye`).
        let forward = Vector3::new(-cp * cy, -cp * sy, -sp);
        let right = Vector3::new(-sy, cy, 0.0);
        let up = right.cross(&forward);
        CameraBasis {
            eye: self.eye(),
            forward,
            right,
            up,
        }
    }

    /// The point on the ray through the pixel `(sx, sy)` in the plane through
    /// `target` normal to `forward`. `vw` and `vh` are the viewport size in
    /// pixels. The function gives `None` when the ray is parallel to the plane.
    pub fn point_at_target_depth(&self, sx: f32, sy: f32, vw: f32, vh: f32) -> Option<Point3<f32>> {
        if vw <= 0.0 || vh <= 0.0 {
            return None;
        }
        let (origin, dir) = self.unproject_ray(sx, sy, vw / vh, vw, vh)?;
        let forward = self.basis().forward;
        let denom = dir.dot(&forward);
        if denom.abs() < 1e-6 {
            return None;
        }
        let t = (self.target - origin).dot(&forward) / denom;
        Some(origin + dir * t)
    }

    /// The world size of one pixel at the camera depth `depth`. `vh` is the
    /// viewport height in pixels. The orthographic projection ignores
    /// `depth` and uses the half-height of `projection_matrix`.
    pub fn world_per_pixel(&self, depth: f32, vh: f32) -> f32 {
        let tan_half = (self.fov * 0.5).tan();
        match self.projection {
            ProjectionMode::Perspective => 2.0 * depth * tan_half / vh,
            ProjectionMode::Orthographic => {
                let half_h = self.distance * tan_half;
                2.0 * half_h / vh
            }
        }
    }

    /// Orbit about the world point `pivot` by a drag of `(dx, dy)` pixels.
    /// The pivot keeps its camera-space coordinates, so it keeps its pixel in
    /// both projections. The distance does not change. With
    /// `pivot = target` this is the plain turntable orbit.
    pub fn orbit_about(&mut self, pivot: Point3<f32>, dx: f32, dy: f32) {
        let old = self.basis();
        let rel = pivot - old.eye;
        let q = Vector3::new(rel.dot(&old.right), rel.dot(&old.up), rel.dot(&old.forward));
        self.yaw -= dx * ORBIT_RAD_PER_PX;
        self.pitch = (self.pitch + dy * ORBIT_RAD_PER_PX).clamp(-PITCH_LIMIT, PITCH_LIMIT);
        let new = self.basis();
        let eye = pivot - (new.right * q.x + new.up * q.y + new.forward * q.z);
        self.target = eye + new.forward * self.distance;
    }

    /// Zoom by the distance ratio `k` toward the world point `point`. The
    /// move is a homothety about `point` with the clamped ratio, so `point`
    /// keeps its pixel in both projections.
    pub fn zoom_toward(&mut self, point: Point3<f32>, k: f32) {
        let d = self.distance;
        let d_new = (k * d).clamp(DISTANCE_RANGE.0, DISTANCE_RANGE.1);
        let kk = d_new / d;
        self.target += (point - self.target) * (1.0 - kk);
        self.distance = d_new;
    }

    /// Pan by a drag of `(dx, dy)` pixels in a viewport `vh` pixels high.
    /// A point at the camera depth `grab_depth` (default: the target depth)
    /// moves by the same `(dx, dy)` pixels.
    pub fn pan_pixels(&mut self, dx: f32, dy: f32, vh: f32, grab_depth: Option<f32>) {
        if vh <= 0.0 {
            return;
        }
        let s = self.world_per_pixel(grab_depth.unwrap_or(self.distance), vh);
        let b = self.basis();
        self.target += b.right * (-dx * s) + b.up * (dy * s);
    }

    /// The pose of a view preset. The target and the distance do not change.
    ///
    /// Top looks down with the Front yaw, so +X is to the right and +Y up on
    /// the screen (the Z-up CAD convention). Isometric takes the yaw
    /// [`YAW_ISO`], the mean of the Front and Right yaws (the eye front-right
    /// and above), and keeps the pitch pi/6.
    pub fn preset_pose(&self, preset: ViewPreset) -> CameraPose {
        let (yaw, pitch) = match preset {
            ViewPreset::Top => (YAW_FRONT, PITCH_LIMIT),
            ViewPreset::Front => (YAW_FRONT, 0.0),
            ViewPreset::Right => (YAW_RIGHT, 0.0),
            ViewPreset::Isometric => (YAW_ISO, std::f32::consts::FRAC_PI_6),
        };
        CameraPose {
            yaw,
            pitch,
            ..self.pose()
        }
    }

    /// The pose that frames the box `min`..`max`: the target at the box
    /// centre and the distance 1.8 times the largest box extent. The yaw and
    /// the pitch do not change.
    pub fn fit_pose(&self, min: [f32; 3], max: [f32; 3]) -> CameraPose {
        let cx = (min[0] + max[0]) * 0.5;
        let cy = (min[1] + max[1]) * 0.5;
        let cz = (min[2] + max[2]) * 0.5;
        let extent = (max[0] - min[0]).max(max[1] - min[1]).max(max[2] - min[2]);
        CameraPose {
            target: Point3::new(cx, cy, cz),
            distance: extent * 1.8,
            ..self.pose()
        }
    }

    /// Project a world-space point to screen coordinates (in pixels).
    /// Returns None if the point is behind the camera.
    pub fn project_to_screen(
        &self,
        world: [f32; 3],
        aspect: f32,
        viewport_w: f32,
        viewport_h: f32,
    ) -> Option<[f32; 2]> {
        let vp = self.projection_matrix(aspect) * self.view_matrix();
        let p = nalgebra::Vector4::new(world[0], world[1], world[2], 1.0);
        let clip = vp * p;
        if clip.w <= 0.0 {
            return None; // behind camera
        }
        let ndc_x = clip.x / clip.w;
        let ndc_y = clip.y / clip.w;
        let screen_x = (ndc_x + 1.0) * 0.5 * viewport_w;
        let screen_y = (1.0 - ndc_y) * 0.5 * viewport_h; // flip Y for screen coords
        Some([screen_x, screen_y])
    }

    /// Cast a ray from screen pixel coordinates into world space.
    /// Returns `(origin, direction)` where origin is on the near plane
    /// and direction is a normalized world-space vector.
    pub fn unproject_ray(
        &self,
        screen_x: f32,
        screen_y: f32,
        aspect: f32,
        viewport_w: f32,
        viewport_h: f32,
    ) -> Option<(Point3<f32>, Vector3<f32>)> {
        let vp = self.projection_matrix(aspect) * self.view_matrix();
        let inv_vp = vp.try_inverse()?;

        // Screen pixels → NDC (same convention as project_to_screen)
        let ndc_x = (screen_x / viewport_w) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_y / viewport_h) * 2.0;

        // Unproject near and far plane points
        let near_clip = nalgebra::Vector4::new(ndc_x, ndc_y, -1.0, 1.0);
        let far_clip = nalgebra::Vector4::new(ndc_x, ndc_y, 1.0, 1.0);

        let near_world = inv_vp * near_clip;
        let far_world = inv_vp * far_clip;

        if near_world.w.abs() < 1e-10 || far_world.w.abs() < 1e-10 {
            return None;
        }

        let near_pt = Point3::new(
            near_world.x / near_world.w,
            near_world.y / near_world.w,
            near_world.z / near_world.w,
        );
        let far_pt = Point3::new(
            far_world.x / far_world.w,
            far_world.y / far_world.w,
            far_world.z / far_world.w,
        );

        let dir = (far_pt - near_pt).normalize();
        Some((near_pt, dir))
    }
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    #[test]
    fn unproject_round_trip() {
        let cam = OrbitCamera::new();
        let aspect = 16.0 / 9.0;
        let vw = 1920.0;
        let vh = 1080.0;
        let world = [10.0f32, 5.0, 3.0];

        let screen = cam
            .project_to_screen(world, aspect, vw, vh)
            .expect("point should be in front of camera");

        let (origin, dir) = cam
            .unproject_ray(screen[0], screen[1], aspect, vw, vh)
            .expect("should produce a ray");

        // The ray should pass near the original world point.
        // Closest point on ray to world: project world onto ray.
        let w = Point3::new(world[0], world[1], world[2]);
        let ow = w - origin;
        let t = ow.dot(&dir);
        let closest = origin + dir * t;
        let error = (closest - w).norm();
        assert!(
            error < 0.1,
            "Ray should pass near original point, error = {error}"
        );
    }

    #[test]
    fn unproject_round_trip_orthographic() {
        let mut cam = OrbitCamera::new();
        cam.projection = ProjectionMode::Orthographic;
        let aspect = 16.0 / 9.0;
        let vw = 1920.0;
        let vh = 1080.0;
        let world = [10.0f32, 5.0, 3.0];

        let screen = cam
            .project_to_screen(world, aspect, vw, vh)
            .expect("point should be in the view box");

        let (origin, dir) = cam
            .unproject_ray(screen[0], screen[1], aspect, vw, vh)
            .expect("should produce a ray");

        // The orthographic ray is parallel to the view axis and must pass
        // near the original point.
        let w = Point3::new(world[0], world[1], world[2]);
        let ow = w - origin;
        let t = ow.dot(&dir);
        let closest = origin + dir * t;
        let error = (closest - w).norm();
        assert!(
            error < 0.1,
            "Ray should pass near original point, error = {error}"
        );
        let along = dir.dot(&cam.basis().forward);
        assert!(
            (along - 1.0).abs() < 1e-4,
            "the orthographic ray must follow the view axis, dot = {along}"
        );
    }
}
