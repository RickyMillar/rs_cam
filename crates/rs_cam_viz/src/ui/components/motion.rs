//! `DESIGN_SPEC.md` §5 as named helpers.
//!
//! # Why these exist rather than a bare `animate_bool_with_time`
//!
//! `Context::animate_bool_with_time` is **hardcoded to linear** (§10.4).
//! Linear motion is the single most recognisable sign that nobody chose an
//! easing: it starts and stops abruptly at both ends. Every helper here goes
//! through `animate_bool_with_time_and_easing` instead.
//!
//! # The durations
//!
//! | Helper | Seconds | Easing | Use |
//! |---|---|---|---|
//! | [`fast`] | 0.12 | `cubic_out` | hover, press |
//! | [`base`] | 0.18 | `cubic_out` | disclosure, panel swap, verdict change |
//! | [`slow`] | 0.24 | `cubic_out` | toast in, scrim fade |
//! | [`leaving`] | 0.24 | `cubic_in` | a toast going away (ruling R11) |
//!
//! Everything that ARRIVES eases out, so it decelerates into place.
//! A toast LEAVING eases in, so it accelerates away — the symmetric partner,
//! ruled in §4.13 R11 because §5 named no function for it.

use crate::ui::tokens;

/// A hover or press state change, 0.12 s, decelerating.
pub fn fast(ctx: &egui::Context, id: egui::Id, on: bool) -> f32 {
    ctx.animate_bool_with_time_and_easing(
        id,
        on,
        tokens::MOTION_FAST,
        egui::emath::easing::cubic_out,
    )
}

/// A disclosure, a panel swap, a chip changing verdict. 0.18 s, decelerating.
pub fn base(ctx: &egui::Context, id: egui::Id, on: bool) -> f32 {
    ctx.animate_bool_with_time_and_easing(
        id,
        on,
        tokens::MOTION_BASE,
        egui::emath::easing::cubic_out,
    )
}

/// A toast arriving or a scrim fading in. 0.24 s, decelerating.
pub fn slow(ctx: &egui::Context, id: egui::Id, on: bool) -> f32 {
    ctx.animate_bool_with_time_and_easing(
        id,
        on,
        tokens::MOTION_SLOW,
        egui::emath::easing::cubic_out,
    )
}

/// A toast going away. 0.24 s, ACCELERATING (ruling R11).
pub fn leaving(ctx: &egui::Context, id: egui::Id, on: bool) -> f32 {
    ctx.animate_bool_with_time_and_easing(
        id,
        on,
        tokens::MOTION_SLOW,
        egui::emath::easing::cubic_in,
    )
}

/// Mix two colours by `t`, for a fill that animates between states.
///
/// `egui::lerp` does not cover `Color32`, and hand-rolling the blend at each
/// call site is how the crate ended up with 201 distinct colour triples.
#[must_use]
pub fn mix(from: egui::Color32, to: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |a: u8, b: u8| -> u8 {
        let a = f32::from(a);
        let b = f32::from(b);
        (a + (b - a) * t).round().clamp(0.0, 255.0) as u8
    };
    egui::Color32::from_rgba_premultiplied(
        f(from.r(), to.r()),
        f(from.g(), to.g()),
        f(from.b(), to.b()),
        f(from.a(), to.a()),
    )
}
