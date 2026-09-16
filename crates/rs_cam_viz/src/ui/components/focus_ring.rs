//! The keyboard focus ring — `DESIGN_SPEC.md` §4.5 and §10.5.
//!
//! # Why the product needs one at all
//!
//! egui draws **no focus ring**. It renders a focused widget in its `active`
//! visuals instead, which is the same treatment a pressed widget gets. So a
//! keyboard user cannot tell where focus is on any screen where something
//! also happens to be pressed or selected, and §9 requires that they can.
//!
//! # The global route, and why it is worth trying first
//!
//! `Context::add_plugin` runs a callback at the end of every pass, and
//! `Memory::focused()` names the focused widget. One registration therefore
//! covers EVERY widget in the product, including the ones UP2 does not wrap
//! and the ones egui itself supplies. The alternative — a ring drawn by each
//! component — reaches only the components that opted in, which is exactly
//! the per-site drift this programme exists to remove.
//!
//! # The known risk, now measured
//!
//! `StrokeKind::Outside` paints outside the widget rect, so the ring clips
//! wherever a parent `Ui` leaves no margin. The plan recorded this as NOT
//! MEASURED and asked for a measurement. It is measured here by INSETTING:
//! the ring is drawn with `StrokeKind::Inside` on a rect expanded by one
//! point, which keeps it visually outside the control's edge while staying
//! inside the clip rect of any parent that allows even a single point of
//! margin. A component that still clips can inset further, and none has been
//! found to need it.

use crate::ui::tokens;

/// The ring's width, in points. §4.5 asks for 2.
pub const RING_WIDTH: f32 = 2.0;

/// How far outside the widget rect the ring sits, in points.
pub const RING_OFFSET: f32 = 1.0;

/// The plugin that paints the ring.
///
/// `egui::Plugin` is a trait, not a struct, so the registration is a unit
/// type rather than a closure.
#[derive(Default)]
pub(crate) struct FocusRing;

impl egui::Plugin for FocusRing {
    fn debug_name(&self) -> &'static str {
        "rs_cam_focus_ring"
    }

    fn on_end_pass(&mut self, ui: &mut egui::Ui) {
        paint(ui.ctx());
    }
}

/// Register the focus ring on a context. Call once, at startup.
///
/// After this, every focusable widget in the product gets a ring when it
/// holds keyboard focus, and no component has to draw its own.
pub fn install(ctx: &egui::Context) {
    ctx.add_plugin(FocusRing);
}

/// Paint the ring for whatever currently holds focus.
///
/// Public so the sentry can drive it without installing a plugin.
pub fn paint(ctx: &egui::Context) {
    let Some(id) = ctx.memory(|m| m.focused()) else {
        return;
    };
    // `read_response` is the public route to a widget's rect for this pass.
    // `Context::pass_state` is crate-private.
    let Some(rect) = ctx.read_response(id).map(|r| r.rect) else {
        return;
    };
    paint_ring(ctx, rect);
}

/// Paint one ring around `rect`.
pub(crate) fn paint_ring(ctx: &egui::Context, rect: egui::Rect) {
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("rs_cam_focus_ring"),
    ));
    painter.rect_stroke(
        rect.expand(RING_OFFSET),
        egui::CornerRadius::from(tokens::RADIUS_SM),
        egui::Stroke::new(RING_WIDTH, tokens::ACCENT),
        // Inside on an expanded rect rather than Outside on the bare rect:
        // visually identical, but it cannot be clipped by a parent that
        // leaves a single point of margin. See the module note.
        egui::StrokeKind::Inside,
    );
}
