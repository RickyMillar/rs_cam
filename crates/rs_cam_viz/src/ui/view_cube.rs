//! R5b: the view cube and the projection toggle in the top-right corner of
//! the viewport (operator ruling, live look round 5). The geometry is
//! `interaction::view_cube`; this module paints it and draws the toggle.
//!
//! - The cube turns with the camera rotation. Each visible face is lit by
//!   how squarely it faces the eye (the headlight rule), so the cube reads
//!   as a solid, and carries its label in the tokens' ink. The face under
//!   the pointer takes the accent.
//! - Three edges from the corner at (-X, -Y, -Z) carry the X, Y and Z axis
//!   colours (the CAD convention of the old triad, ruling R22).
//! - A click on a face sends `UiCommand::SetViewPreset` (the frame reads
//!   `face_at`); a drag on the cube orbits (`InputEnv::press_on_gizmo`).
//! - Under the cube, one segmented control "Persp · Ortho" reads the
//!   camera projection and sends `UiCommand::ToggleProjection`.

use crate::interaction::view_cube::{self, CubeFace};
use crate::render::camera::{OrbitCamera, ProjectionMode};
use crate::ui::AppEvent;
use crate::ui::tokens;
use crate::ui_command::{NoArgs, UiCommand};

/// The egui id of the projection toggle area.
pub const TOGGLE_AREA_ID: &str = "viewport_projection_toggle";

/// The hover text of the two toggle choices.
pub const PERSPECTIVE_HOVER: &str = "Perspective: far things look smaller, as the eye sees them.";
pub const ORTHOGRAPHIC_HOVER: &str = "Orthographic: no foreshortening; parallel edges stay parallel and a \
     size reads the same at every depth.";

/// The fill of a face that faces the eye by `facing` (0 edge-on, 1 face-on).
fn face_fill(facing: f32) -> egui::Color32 {
    tokens::INK_25.lerp_to_gamma(tokens::INK_50, facing.clamp(0.0, 1.0))
}

/// Paint the cube of `camera` in the corner of `viewport`. `hover` is the
/// pointer position; the face under it takes the accent.
pub fn paint(
    painter: &egui::Painter,
    viewport: egui::Rect,
    camera: &OrbitCamera,
    hover: Option<egui::Pos2>,
) {
    let rect = view_cube::cube_rect(viewport);
    let faces = view_cube::visible_faces(rect, camera);
    let hovered = hover.and_then(|pos| view_cube::face_at(rect, camera, pos));
    for face in &faces {
        let is_hovered = hovered == Some(face.preset);
        let (fill, stroke) = if is_hovered {
            (tokens::ACCENT_QUIET, tokens::ACCENT)
        } else {
            (face_fill(face.facing), tokens::HAIRLINE)
        };
        painter.add(egui::Shape::convex_polygon(
            face.corners.to_vec(),
            fill,
            egui::Stroke::new(1.0, stroke),
        ));
    }
    paint_axes(painter, rect, camera, &faces);
    for face in &faces {
        // An oblique face is too thin for its word.
        if face.facing < 0.35 {
            continue;
        }
        let ink = if hovered == Some(face.preset) {
            tokens::TEXT_STRONG
        } else {
            tokens::TEXT_BODY
        };
        painter.text(
            face.centre(),
            egui::Align2::CENTER_CENTER,
            face.label,
            egui::FontId::proportional(tokens::SIZE_MICRO),
            ink,
        );
    }
}

/// The X, Y and Z axis colours on the three edges from the (-1, -1, -1)
/// corner. An edge shows when one of its two faces is visible.
fn paint_axes(painter: &egui::Painter, rect: egui::Rect, camera: &OrbitCamera, faces: &[CubeFace]) {
    use crate::render::camera::ViewPreset;
    let basis = camera.basis();
    let h = view_cube::half_edge();
    let centre = rect.center();
    let screen = |p: [f32; 3]| -> egui::Pos2 {
        let v = nalgebra::Vector3::new(p[0], p[1], p[2]) * h;
        centre + egui::vec2(v.dot(&basis.right), -v.dot(&basis.up))
    };
    let shown = |preset: ViewPreset| faces.iter().any(|f| f.preset == preset);
    let origin = [-1.0, -1.0, -1.0];
    let edges = [
        (
            [1.0, -1.0, -1.0],
            tokens::AXIS_X,
            [ViewPreset::Bottom, ViewPreset::Front],
        ),
        (
            [-1.0, 1.0, -1.0],
            tokens::AXIS_Y,
            [ViewPreset::Bottom, ViewPreset::Left],
        ),
        (
            [-1.0, -1.0, 1.0],
            tokens::AXIS_Z,
            [ViewPreset::Front, ViewPreset::Left],
        ),
    ];
    for (end, colour, adjacent) in edges {
        if adjacent.into_iter().any(shown) {
            painter.line_segment(
                [screen(origin), screen(end)],
                egui::Stroke::new(2.0, colour),
            );
        }
    }
}

/// The projection toggle under the cube: "Persp · Ortho". A click on the
/// choice that is not in force sends `ToggleProjection`.
pub fn draw_toggle(
    ctx: &egui::Context,
    viewport: egui::Rect,
    projection: ProjectionMode,
    events: &mut Vec<AppEvent>,
) {
    let rect = view_cube::toggle_rect(viewport);
    egui::Area::new(egui::Id::new(TOGGLE_AREA_ID))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .constrain_to(viewport)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.horizontal(|ui| {
                let size = egui::vec2(rect.width() / 2.0, rect.height());
                for (mode, word, hover) in [
                    (ProjectionMode::Perspective, "Persp", PERSPECTIVE_HOVER),
                    (ProjectionMode::Orthographic, "Ortho", ORTHOGRAPHIC_HOVER),
                ] {
                    let selected = projection == mode;
                    let response = ui
                        .add_sized(
                            size,
                            egui::Button::selectable(
                                selected,
                                egui::RichText::new(word).size(tokens::SIZE_CAPTION),
                            ),
                        )
                        .on_hover_text(hover);
                    if response.clicked() && !selected {
                        events.push(AppEvent::Ui(UiCommand::ToggleProjection(NoArgs)));
                    }
                }
            });
        });
}
