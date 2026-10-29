//! R5b: the view cube geometry (operator ruling, live look round 5: "the
//! gizmo becomes a larger view cube whose faces click to the six views").
//!
//! Pure functions, no egui painter: the square of the cube in the viewport,
//! the press test of a drag (`on_cube`), the visible faces of the cube for
//! a camera orientation (`visible_faces`) and the preset of the face under
//! a point (`face_at`). `ui::view_cube` paints them; the frame reads
//! `on_cube` for `InputEnv::press_on_gizmo` and `face_at` for a click.
//!
//! The cube turns with the camera rotation only and has no perspective:
//! a corner `p` (a point of the unit cube scaled to [`half_edge`]) goes to
//! `centre + (p · right, -(p · up))`, with the camera's `right` and `up`.

use crate::render::camera::{OrbitCamera, ViewPreset};
use crate::ui::tokens;
use nalgebra::Vector3;

/// The gap between the cube square and the viewport corner, in points.
pub const VIEW_CUBE_MARGIN: f32 = 10.0;

/// The height of the projection toggle under the cube, in points: one
/// button row (`tokens::ROW_ACTION`, the interact height).
pub const PROJECTION_TOGGLE_HEIGHT: f32 = tokens::ROW_ACTION;

/// A face counts as visible when its normal faces the eye by more than
/// this cosine. A face seen edge-on (a side face in the Top view, at
/// `PITCH_LIMIT`) is a sliver and takes no click.
pub const VISIBLE_COS: f32 = 0.05;

/// The square of the cube in the top-right corner of `viewport`.
#[must_use]
pub fn cube_rect(viewport: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(
            viewport.right() - VIEW_CUBE_MARGIN - tokens::VIEW_CUBE_PX,
            viewport.top() + VIEW_CUBE_MARGIN,
        ),
        egui::vec2(tokens::VIEW_CUBE_PX, tokens::VIEW_CUBE_PX),
    )
}

/// The rectangle of the projection toggle, under the cube.
#[must_use]
pub fn toggle_rect(viewport: egui::Rect) -> egui::Rect {
    let cube = cube_rect(viewport);
    egui::Rect::from_min_size(
        egui::pos2(cube.left(), cube.bottom() + tokens::SPACE_2),
        egui::vec2(cube.width(), PROJECTION_TOGGLE_HEIGHT),
    )
}

/// The whole corner the cube and the toggle take: the legend keeps clear
/// of it.
#[must_use]
pub fn corner_rect(viewport: egui::Rect) -> egui::Rect {
    cube_rect(viewport).union(toggle_rect(viewport))
}

/// The screen point `pos` is on the cube square of `viewport`: a left drag
/// from there orbits (`InputEnv::press_on_gizmo`).
#[must_use]
pub fn on_cube(viewport: egui::Rect, pos: egui::Pos2) -> bool {
    cube_rect(viewport).contains(pos)
}

/// Half the edge of the cube, in points: the projection of a cube of this
/// half edge never leaves the square (its widest view, along a diagonal,
/// spans `2 * half_edge * sqrt(3)`).
#[must_use]
pub fn half_edge() -> f32 {
    tokens::VIEW_CUBE_PX * 0.5 / 3.0_f32.sqrt()
}

/// One face of the cube: the outward normal, the preset a click sends, its
/// label and its corners on the unit cube, in order round the face.
struct FaceDef {
    normal: [f32; 3],
    preset: ViewPreset,
    label: &'static str,
    corners: [[f32; 3]; 4],
}

/// The six faces. The preset of a face is the view that looks at it: the
/// Front face is the -Y face, and `ViewPreset::Front` puts the eye on -Y.
const FACES: [FaceDef; 6] = [
    FaceDef {
        normal: [0.0, 0.0, 1.0],
        preset: ViewPreset::Top,
        label: "Top",
        corners: [
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
        ],
    },
    FaceDef {
        normal: [0.0, 0.0, -1.0],
        preset: ViewPreset::Bottom,
        label: "Bottom",
        corners: [
            [-1.0, -1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [1.0, 1.0, -1.0],
            [1.0, -1.0, -1.0],
        ],
    },
    FaceDef {
        normal: [0.0, -1.0, 0.0],
        preset: ViewPreset::Front,
        label: "Front",
        corners: [
            [-1.0, -1.0, -1.0],
            [1.0, -1.0, -1.0],
            [1.0, -1.0, 1.0],
            [-1.0, -1.0, 1.0],
        ],
    },
    FaceDef {
        normal: [0.0, 1.0, 0.0],
        preset: ViewPreset::Back,
        label: "Back",
        corners: [
            [1.0, 1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
        ],
    },
    FaceDef {
        normal: [1.0, 0.0, 0.0],
        preset: ViewPreset::Right,
        label: "Right",
        corners: [
            [1.0, -1.0, -1.0],
            [1.0, 1.0, -1.0],
            [1.0, 1.0, 1.0],
            [1.0, -1.0, 1.0],
        ],
    },
    FaceDef {
        normal: [-1.0, 0.0, 0.0],
        preset: ViewPreset::Left,
        label: "Left",
        corners: [
            [-1.0, 1.0, -1.0],
            [-1.0, -1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [-1.0, 1.0, 1.0],
        ],
    },
];

/// One visible face of the cube on the screen.
#[derive(Debug, Clone, PartialEq)]
pub struct CubeFace {
    pub preset: ViewPreset,
    pub label: &'static str,
    /// The corners on the screen, in order round the face.
    pub corners: [egui::Pos2; 4],
    /// How squarely the face looks at the eye: the cosine between its
    /// normal and the direction to the eye, in (`VISIBLE_COS`, 1]. The
    /// painter lights a face by it (the headlight rule).
    pub facing: f32,
}

impl CubeFace {
    /// The mean of the four corners.
    #[must_use]
    pub fn centre(&self) -> egui::Pos2 {
        let sum = self
            .corners
            .iter()
            .fold(egui::Vec2::ZERO, |acc, c| acc + c.to_vec2());
        (sum / 4.0).to_pos2()
    }

    /// The point `pos` is inside the face (a convex quad; the edge counts).
    #[must_use]
    pub fn contains(&self, pos: egui::Pos2) -> bool {
        let mut sign = 0.0_f32;
        for i in 0..4 {
            let (Some(a), Some(b)) = (self.corners.get(i), self.corners.get((i + 1) % 4)) else {
                return false;
            };
            let cross = (b.x - a.x) * (pos.y - a.y) - (b.y - a.y) * (pos.x - a.x);
            if cross.abs() <= f32::EPSILON {
                continue;
            }
            if sign == 0.0 {
                sign = cross.signum();
            } else if cross.signum() != sign {
                return false;
            }
        }
        true
    }
}

/// The faces of the cube that face the eye of `camera`, in the square
/// `rect`. The cube is convex, so visible faces never overlap.
#[must_use]
pub fn visible_faces(rect: egui::Rect, camera: &OrbitCamera) -> Vec<CubeFace> {
    let basis = camera.basis();
    let centre = rect.center();
    let h = half_edge();
    let screen = |p: [f32; 3]| -> egui::Pos2 {
        let v = Vector3::new(p[0], p[1], p[2]) * h;
        centre + egui::vec2(v.dot(&basis.right), -v.dot(&basis.up))
    };
    FACES
        .iter()
        .filter_map(|face| {
            let n = Vector3::new(face.normal[0], face.normal[1], face.normal[2]);
            // The eye looks along `forward`; a face whose normal points
            // back at the eye is seen.
            let facing = -n.dot(&basis.forward);
            (facing > VISIBLE_COS).then(|| CubeFace {
                preset: face.preset,
                label: face.label,
                corners: face.corners.map(screen),
                facing,
            })
        })
        .collect()
}

/// The preset of the cube face under `pos`, or `None` off the cube.
#[must_use]
pub fn face_at(rect: egui::Rect, camera: &OrbitCamera, pos: egui::Pos2) -> Option<ViewPreset> {
    visible_faces(rect, camera)
        .into_iter()
        .filter(|f| f.contains(pos))
        .max_by(|a, b| a.facing.total_cmp(&b.facing))
        .map(|f| f.preset)
}
