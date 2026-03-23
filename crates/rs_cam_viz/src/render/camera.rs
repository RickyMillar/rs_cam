use nalgebra::{Matrix4, Point3, Vector3};

/// View preset orientations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewPreset {
    Top,
    Front,
    Right,
    Isometric,
}

/// Orbit camera for the 3D viewport.
pub struct OrbitCamera {
    pub target: Point3<f32>,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub fov: f32,
    pub near: f32,
    pub far: f32,
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
        }
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
        Matrix4::new_perspective(aspect, self.fov, self.near, self.far)
    }

    /// Combined view-projection matrix.
    #[allow(clippy::indexing_slicing)] // nalgebra 4x4 matrix slice is always 16 elements
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

    /// Handle orbit input (delta in screen-space pixels).
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * 0.005;
        self.pitch = (self.pitch + dy * 0.005).clamp(
            -std::f32::consts::FRAC_PI_2 + 0.01,
            std::f32::consts::FRAC_PI_2 - 0.01,
        );
    }

    /// Handle pan input (delta in screen-space pixels).
    pub fn pan(&mut self, dx: f32, dy: f32) {
        let scale = self.distance * 0.001;
        // Right vector in world space
        let right = Vector3::new(-self.yaw.sin(), self.yaw.cos(), 0.0);
        // Up vector in world space (perpendicular to view direction, in the vertical plane)
        let forward = Vector3::new(
            self.pitch.cos() * self.yaw.cos(),
            self.pitch.cos() * self.yaw.sin(),
            self.pitch.sin(),
        );
        let up = Vector3::new(0.0, 0.0, 1.0);
        let cam_up = up - forward * forward.dot(&up);
        let cam_up = if cam_up.norm() > 1e-6 {
            cam_up.normalize()
        } else {
            up
        };

        self.target += right * (-dx * scale) + cam_up * (dy * scale);
    }

    /// Handle zoom input (scroll delta).
    pub fn zoom(&mut self, delta: f32) {
        self.distance *= (-delta * 0.001).exp();
        self.distance = self.distance.clamp(0.1, 50000.0);
    }

    /// Fit the camera to a bounding box.
    #[allow(clippy::indexing_slicing)] // fixed-size [f32; 3] arrays
    pub fn fit_to_bounds(&mut self, min: [f32; 3], max: [f32; 3]) {
        let cx = (min[0] + max[0]) * 0.5;
        let cy = (min[1] + max[1]) * 0.5;
        let cz = (min[2] + max[2]) * 0.5;
        self.target = Point3::new(cx, cy, cz);

        let extent = (max[0] - min[0]).max(max[1] - min[1]).max(max[2] - min[2]);
        self.distance = extent * 1.8;
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

    /// Set a preset view orientation.
    pub fn set_preset(&mut self, preset: ViewPreset) {
        match preset {
            ViewPreset::Top => {
                self.yaw = 0.0;
                self.pitch = std::f32::consts::FRAC_PI_2 - 0.01;
            }
            ViewPreset::Front => {
                self.yaw = 0.0;
                self.pitch = 0.0;
            }
            ViewPreset::Right => {
                self.yaw = std::f32::consts::FRAC_PI_2;
                self.pitch = 0.0;
            }
            ViewPreset::Isometric => {
                self.yaw = std::f32::consts::FRAC_PI_4;
                self.pitch = std::f32::consts::FRAC_PI_6;
            }
        }
    }
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
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
}
