//! Where a drawing item, a face or a pin sits in the display frame of a
//! setup: the one geometry the picker, the uploads and the camera fit share.
//!
//! Moved out of `interaction/picking.rs` (F3 layering): `state/` must not
//! import `interaction/`, and the camera fit (`state::camera`) reads
//! [`selection_frame_bounds`].

use crate::state::job::{SetupFrame, StockConfig};
use crate::state::toolpath::ToolpathId;
use rs_cam_core::geo::{BoundingBox3, P2, P3};
use rs_cam_core::session::ProjectSession;

/// The Z (display frame) at which the drawing of `tc` draws, and at which a
/// pick tests it (geometry selection P4.5: the one plane of the draw and
/// the pick). A Trace draws at its last cutting level; every other op at
/// the displayed stock top plus [`HOLE_MARK_LIFT_MM`]. A Project Curve
/// draws its lines on the surface; the pick tests them on this plane.
#[must_use]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "projection: only the named variant(s) carry this (named: OperationConfig::Trace)"
)]
pub fn drawing_plane_z(
    session: &ProjectSession,
    tc: &rs_cam_core::session::ToolpathConfig,
    frame: Option<&SetupFrame>,
) -> f64 {
    match &tc.operation {
        // Geometry selection P3.1: the Trace top is THE heights of the
        // session (`toolpath_heights`), the top that generation cuts from.
        // A refused height draws at the stock top, as every other op.
        rs_cam_core::compute::catalog::OperationConfig::Trace(_) => {
            match session.toolpath_heights(tc) {
                Ok(resolved) => {
                    let heights = resolved.heights;
                    tc.operation
                        .cutting_levels(&heights)
                        .last()
                        .copied()
                        .unwrap_or(heights.top_z)
                        + HOLE_MARK_LIFT_MM
                }
                Err(_) => drawing_display_z(frame, session.stock_config()),
            }
        }
        _ => drawing_display_z(frame, session.stock_config()),
    }
}

/// The lift of a hole marker above the displayed stock top (mm), so the
/// marker draws above the stock face. It is the lift of the drawing lines
/// (`app/gpu_upload.rs`, `planar_z`), which the markers share.
pub const HOLE_MARK_LIFT_MM: f64 = 0.05;

/// Where the drawing point `p` (model frame) is drawn and picked, in XY:
/// the setup transform of the drawing lines. `frame` is the setup of the
/// toolpath; `None` gives the raw model point (geometry selection P4.4:
/// the one point function of the draw, the pick and the "Show me" frame).
#[must_use]
pub fn item_display_xy(frame: Option<&SetupFrame>, stock: &StockConfig, p: P2) -> P2 {
    match frame {
        Some(frame) => {
            let q = frame.transform_point(P3::new(p.x, p.y, 0.0), stock);
            P2::new(q.x, q.y)
        }
        None => p,
    }
}

/// Where the viewport draws the point `p` of `item`, in the display
/// frame (geometry selection UX-7). A face draws at its plane Z and a STEP
/// ring, edge, circle or point at its own Z ([`Item::plane_z`]), moved as
/// the model mesh is moved (`SetupFrame::transform_point`). A drawing item
/// and a stock item draw on `plane` (the [`drawing_plane_z`] plane).
///
/// [`Item::plane_z`]: rs_cam_core::selection::Item::plane_z
#[must_use]
pub fn item_display_point(
    frame: Option<&SetupFrame>,
    stock: &StockConfig,
    item: &rs_cam_core::selection::Item,
    p: P2,
    plane: f64,
) -> P3 {
    match item.plane_z() {
        Some(z) => {
            let q = P3::new(p.x, p.y, z);
            frame.map_or(q, |f| f.transform_point(q, stock))
        }
        None => {
            let q = item_display_xy(frame, stock, p);
            P3::new(q.x, q.y, plane)
        }
    }
}

/// The displayed stock top plus [`HOLE_MARK_LIFT_MM`]: the plane the
/// drawing lines and the item marks draw on (no projection).
#[must_use]
pub fn drawing_display_z(frame: Option<&SetupFrame>, stock: &StockConfig) -> f64 {
    match frame {
        Some(frame) => frame.effective_stock(stock).2 + HOLE_MARK_LIFT_MM,
        None => stock.top_z() + HOLE_MARK_LIFT_MM,
    }
}

/// The radius (mm) of the mark of a point item, of a drill marker of a
/// point, and the half size of an open path end mark and of a lost point
/// or path cross. It was the literal of the drill marker radius in
/// `app/gpu_upload.rs` until P4.4.
pub const POINT_MARK_RADIUS_MM: f64 = 1.5;

/// The box that "Show me" frames for the items `keys` of the toolpath
/// `id`, in the display frame (geometry selection P4.4). An item is found
/// in its source by key; a key that names no item (a lost pick) frames
/// its stored anchor, grown by half its measure (a loop, a face, a circle)
/// or by [`POINT_MARK_RADIUS_MM`] (a point, a path). `None` when no key
/// gives a point.
#[must_use]
pub fn selection_frame_bounds(
    session: &ProjectSession,
    id: ToolpathId,
    keys: &[rs_cam_core::selection::MatchKey],
) -> Option<BoundingBox3> {
    use rs_cam_core::selection::{ItemKind, Shape, SourceRef};
    let (_, tc) = session.find_toolpath_config_by_id(id)?;
    let stock = session.stock_config();
    let frame = toolpath_setup_frame(session, id);
    let z = drawing_display_z(frame.as_ref(), stock);
    let mut points: Vec<P2> = Vec::new();
    for key in keys {
        let model_id = match key.source {
            SourceRef::ToolpathModel => Some(tc.model_id),
            SourceRef::Model(m) => Some(m),
            SourceRef::Stock => None,
        };
        let Some(model_id) = model_id else {
            let (w, d) = match frame.as_ref() {
                Some(frame) => {
                    let (w, d, _) = frame.effective_stock(stock);
                    (w, d)
                }
                None => (stock.x, stock.y),
            };
            let (x0, y0) = match frame {
                Some(_) => (0.0, 0.0),
                None => (stock.origin_x, stock.origin_y),
            };
            points.push(P2::new(x0, y0));
            points.push(P2::new(x0 + w, y0 + d));
            continue;
        };
        let item = session
            .models()
            .iter()
            .find(|m| m.id == model_id)
            .and_then(|m| m.items.iter().find(|i| key.names_same_geometry(i.key())));
        let raw: Vec<P2> = match item.map(|i| i.shape()) {
            Some(Shape::Loop(p) | Shape::Path(p)) => p.exterior.clone(),
            Some(Shape::Circle { ring, .. }) => ring.exterior.clone(),
            Some(Shape::Face(face)) => face.rings.exterior.clone(),
            Some(Shape::Point(p)) => {
                let r = POINT_MARK_RADIUS_MM;
                vec![P2::new(p.x - r, p.y - r), P2::new(p.x + r, p.y + r)]
            }
            None => {
                let half = match key.kind {
                    ItemKind::Loop | ItemKind::Face | ItemKind::Circle => {
                        (key.measure / 2.0).max(POINT_MARK_RADIUS_MM)
                    }
                    ItemKind::Point | ItemKind::Path => POINT_MARK_RADIUS_MM,
                };
                let [x, y] = key.anchor;
                vec![P2::new(x - half, y - half), P2::new(x + half, y + half)]
            }
        };
        points.extend(
            raw.into_iter()
                .map(|p| item_display_xy(frame.as_ref(), stock, p)),
        );
    }
    let first = points.first()?;
    let (mut x0, mut y0, mut x1, mut y1) = (first.x, first.y, first.x, first.y);
    for p in &points {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    Some(BoundingBox3 {
        min: P3::new(x0, y0, z),
        max: P3::new(x1, y1, z),
    })
}

/// The setup frame of the toolpath `id`, or `None` when no setup owns it.
#[must_use]
pub fn toolpath_setup_frame(session: &ProjectSession, id: ToolpathId) -> Option<SetupFrame> {
    session
        .setup_of_toolpath_id(id)
        .and_then(|index| session.list_setups().get(index))
        .map(SetupFrame::of)
}
