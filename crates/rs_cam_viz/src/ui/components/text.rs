//! The type rungs egui cannot carry as global styles.
//!
//! `DESIGN_SPEC.md` §3.2 defines nine rungs. **Five map onto egui's built-in
//! `TextStyle` slots** and `tokens::apply` sets them, so they apply
//! everywhere without a call site changing. The remaining four do not exist
//! as global styles and are constructors here.
//!
//! The reason is §10.3: a custom `TextStyle::Name` is a legal key in the
//! style map, but `ui.label()` never reads it. A rung egui has no slot for
//! can only reach a call site as a `RichText` that already carries its font.
//!
//! | Rung | Here | Why it is not a slot |
//! |---|---|---|
//! | `Display` | [`display`] | no built-in slot above `Heading` |
//! | `Subhead` | [`subhead`] | no built-in slot between `Body` and `Small` |
//! | `Micro` | [`micro`] | needs letter spacing, which is per call site (§10.8) |
//! | `BodyStrong` | [`body_strong`] | ruling R6; `.strong()` is a colour, not a weight |
//!
//! [`numeric`] is here too, though `Monospace` IS a slot, because **every
//! measured value renders in `Numeric`** (§3.4) and a named constructor makes
//! that rule greppable.

use crate::ui::tokens;

/// A window title or a page title. 20 points, SemiBold. Rare.
#[must_use]
pub fn display(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into())
        .font(tokens::font_display())
        .color(tokens::TEXT_STRONG)
}

/// A section header. 12 points, SemiBold.
///
/// This is the rung the 41 hand-rolled `.small().strong()` headers become.
#[must_use]
pub fn subhead(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into())
        .font(tokens::font_subhead())
        .color(tokens::TEXT_MUTED)
}

/// Chip text, and nothing else. 10 points, SemiBold, upper case, tracked.
///
/// This is the ONLY rung allowed below the 11-point floor, and §3.2 permits
/// it because a chip is short, upper-case and high-contrast by construction.
///
/// The tracking is applied here rather than globally because
/// `extra_letter_spacing` is per call site and measured in POINTS, not ems
/// (§10.8) — so 0.8 pt at 10 pt is the value, and only a helper keeps it
/// consistent.
#[must_use]
pub fn micro(text: impl AsRef<str>) -> egui::RichText {
    egui::RichText::new(text.as_ref().to_uppercase())
        .font(tokens::font_micro())
        .extra_letter_spacing(tokens::MICRO_TRACKING)
}

/// Emphasis inside body text. 13 points, Inter Medium (ruling R6).
///
/// Prefer this to `.strong()`, which changes the COLOUR and not the weight.
#[must_use]
pub fn body_strong(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into())
        .font(tokens::font_body_strong())
        .color(tokens::TEXT_STRONG)
}

/// A measured value. 13 points, JetBrains Mono.
///
/// §3.4: **every measured value renders in `Numeric`**, and it is a hard rule
/// with a toolkit reason — egui exposes no OpenType feature switch, so
/// tabular figures cannot be turned on for a proportional face. Alignment
/// comes from the monospace family or from nowhere.
#[must_use]
pub fn numeric(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into())
        .font(tokens::font_numeric())
        .color(tokens::TEXT_STRONG)
}

/// A unit suffix. `Caption` in `TEXT_FAINT`.
///
/// §3.4: a unit sits one `SPACE_1` after the number and **never inside the
/// `Numeric` run**, so the digits stay monospaced and the unit does not.
#[must_use]
pub fn unit(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into())
        .size(tokens::SIZE_CAPTION)
        .color(tokens::TEXT_FAINT)
}

/// A supporting sentence or a provenance stamp. `Caption` in `TEXT_MUTED`.
///
/// Note the colour: `TEXT_FAINT` is forbidden for a sentence (§2.4) and is
/// reserved for a unit beside its number.
#[must_use]
pub fn caption(text: impl Into<String>) -> egui::RichText {
    egui::RichText::new(text.into())
        .size(tokens::SIZE_CAPTION)
        .color(tokens::TEXT_MUTED)
}

/// Group thousands with a thin space: `661 212`, never `661212` (§3.4).
///
/// A thin space (U+2009) rather than a comma, because a comma is a decimal
/// separator in half the world and this product reports millimetres.
#[must_use]
pub fn group_thousands(value: i64) -> String {
    let negative = value < 0;
    let digits = value.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if negative {
        out.push('-');
    }
    let first = digits.len() % 3;
    if let Some(head) = digits.get(..first) {
        out.push_str(head);
    }
    let rest = digits.as_bytes().get(first..).unwrap_or_default();
    for (i, chunk) in rest.chunks(3).enumerate() {
        if i > 0 || first > 0 {
            out.push('\u{2009}');
        }
        out.push_str(&String::from_utf8_lossy(chunk));
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn thousands_use_a_thin_space() {
        assert_eq!(group_thousands(661_212), "661\u{2009}212");
        assert_eq!(group_thousands(1_000), "1\u{2009}000");
        assert_eq!(group_thousands(999), "999");
        assert_eq!(group_thousands(0), "0");
        assert_eq!(group_thousands(-12_345), "-12\u{2009}345");
        assert_eq!(group_thousands(1_234_567), "1\u{2009}234\u{2009}567");
    }

    #[test]
    fn micro_upper_cases_and_tracks() {
        let t = micro("pend");
        assert_eq!(t.text(), "PEND");
    }
}
