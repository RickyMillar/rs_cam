//! Where the viewport shows the alignment pins, and in which colour (P7).
//!
//! C8: the pick used `(pin.x, pin.y, stock.z)`, with no stock origin and no
//! setup transform, while the drawing used both. A click on a flipped setup
//! or on a stock whose origin is not at 0 missed the drawn circle. The GPU
//! upload, the pin numbers and the pick now read [`pin_display_point`] in
//! the frame of [`active_display_setup`], so the three cannot disagree.
//!
//! The colours come from the token module, so the circle in the viewport
//! and the chip in the stock panel show one colour for one state.

use rs_cam_core::compute::alignment_pins::PIN_MATCH_TOL_MM;
use rs_cam_core::geo::P3;
use rs_cam_core::session::{PinStatus, ProjectSession};

use super::job::{SetupFrame, SetupId, StockConfig};
use super::selection::Selection;

/// The lift of a pin mark above the displayed stock top, in mm, so the
/// circle does not fight the stock face for the same depth.
pub const PIN_MARK_LIFT_MM: f64 = 0.1;

/// The setup whose frame the viewport displays, and its id.
///
/// The selected setup, the setup of a selected obstacle, the
/// setup of a selected toolpath, else the first setup. `None` when the
/// project has no setup.
#[must_use]
pub fn active_display_setup(
    session: &ProjectSession,
    selection: &Selection,
) -> Option<(SetupFrame, SetupId)> {
    let setup_id = match selection {
        Selection::Setup(id) => Some(*id),
        Selection::Obstacle(id, _) => Some(*id),
        Selection::Toolpath(tp_id) => session
            .setup_of_toolpath_id(*tp_id)
            .and_then(|idx| session.list_setups().get(idx))
            .map(|sd| SetupId(sd.id)),
        _ => None,
    };
    let setup = match setup_id {
        Some(sid) => session.list_setups().iter().find(|s| SetupId(s.id) == sid),
        None => session.list_setups().first(),
    }?;
    Some((SetupFrame::of(setup), SetupId(setup.id)))
}

/// Where the stock-local point `(x, y)` shows in the display frame of
/// `frame`: the setup transform of the point, on the DISPLAYED stock top
/// plus [`PIN_MARK_LIFT_MM`].
///
/// The Z is the displayed top, not the transformed original top. A
/// `Bottom` setup maps the original top to the underside, where the stock
/// hides the circle (C8).
#[must_use]
pub fn pin_display_point(frame: &SetupFrame, stock: &StockConfig, x: f64, y: f64) -> P3 {
    let world = P3::new(
        x + stock.origin_x,
        y + stock.origin_y,
        stock.origin_z + stock.z,
    );
    let local = frame.transform_point(world, stock);
    let (_, _, top) = frame.effective_stock(stock);
    P3::new(local.x, local.y, top + PIN_MARK_LIFT_MM)
}

/// The display centre of every stored pin in `frame`, in pin order.
#[must_use]
pub fn pin_display_points(frame: &SetupFrame, stock: &StockConfig) -> Vec<P3> {
    stock
        .alignment_pins
        .iter()
        .map(|p| pin_display_point(frame, stock, p.at.x(), p.at.y()))
        .collect()
}

/// The token colour of a pin state.
#[must_use]
pub fn pin_status_colour(status: PinStatus) -> egui::Color32 {
    match status {
        PinStatus::Good => crate::ui::tokens::OK,
        PinStatus::ThinWall => crate::ui::tokens::CAUTION,
        PinStatus::Fails => crate::ui::tokens::DANGER,
    }
}

/// The radius of a ghost hole ring, in mm: the pin radius plus
/// [`PIN_MATCH_TOL_MM`], the slop inside which a dowel finds a hole.
///
/// When the set does not key, the closest wrong seating often puts a ghost
/// hole exactly on a real pin (two pins on the axis: a half turn swaps
/// them). At the pin radius the dashed red ring would lie on the solid red
/// circle and vanish; one tolerance out, it shows as a halo round the pin.
#[must_use]
pub fn ghost_ring_radius(pin_diameter: f64) -> f64 {
    0.5 * pin_diameter + PIN_MATCH_TOL_MM
}

/// The colour of the wrong-orientation ghost.
pub const PIN_GHOST_COLOUR: egui::Color32 = crate::ui::tokens::DANGER;

/// A token colour as the `[0, 1]` RGB triple the line pipeline takes.
#[must_use]
pub fn line_rgb(colour: egui::Color32) -> [f32; 3] {
    [
        f32::from(colour.r()) / 255.0,
        f32::from(colour.g()) / 255.0,
        f32::from(colour.b()) / 255.0,
    ]
}
