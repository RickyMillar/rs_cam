//! `StatusChip` — `DESIGN_SPEC.md` §4.3.
//!
//! A small tinted rectangle carrying a verdict: a glyph, a word, and a
//! ground in the role's tint.
//!
//! # Why the glyph is not optional
//!
//! §2.6 rule 3: **colour is never the only channel.** That rule became
//! load-bearing at UP1. The seven freshness states used to sit on seven
//! near-distinct colours; under the new palette they collapse onto three —
//! `GEN`, `STALE` and `WAIT` are all `CAUTION`, and `PEND` and `OFF` are both
//! `INK_50`. Colour alone therefore separates two of the seven, not seven.
//! Ruling R19 puts the glyph back as the channel that does the work, beside
//! the word that always did.
//!
//! # What this component does NOT do
//!
//! It does not decide a verdict. `toolpath_panel::status_chip` maps a
//! `FreshnessState` to its word and role, that mapping is pinned by
//! `tests/freshness_surfaces_g_freshrender.rs`, and nothing here changes it.
//! This type only draws.

use crate::ui::{components::text, tokens};

/// The five semantic roles of `DESIGN_SPEC.md` §2.6.
///
/// A role is a MEANING, not a colour. Reading the colour off the role is this
/// type's job; no call site should pick one.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Role {
    /// Within a band, current, clear, pass.
    Ok,
    /// Stale, waiting, elevated, review. A measurement came back.
    Caution,
    /// Exceeds, collision, error, refusal.
    Danger,
    /// Informational.
    Info,
    /// NOT MEASURED. An abstention, never a zero.
    Unknown,
}

impl Role {
    /// The role's text colour.
    #[must_use]
    pub fn text(self) -> egui::Color32 {
        match self {
            Self::Ok => tokens::OK,
            Self::Caution => tokens::CAUTION,
            Self::Danger => tokens::DANGER,
            Self::Info => tokens::INFO,
            Self::Unknown => tokens::UNKNOWN,
        }
    }

    /// The role's quiet ground, which is also its surface tint (§2.7).
    #[must_use]
    pub fn tint(self) -> egui::Color32 {
        match self {
            Self::Ok => tokens::TINT_OK,
            Self::Caution => tokens::TINT_CAUTION,
            Self::Danger => tokens::TINT_DANGER,
            Self::Info => tokens::TINT_INFO,
            Self::Unknown => tokens::TINT_UNKNOWN,
        }
    }

    /// The glyph that accompanies the role (§2.6 rule 3).
    ///
    /// `Info` carries none: it is not a verdict, so there is nothing to
    /// abstain from or to pass.
    #[must_use]
    pub fn glyph(self) -> Option<&'static str> {
        match self {
            Self::Ok => Some(tokens::GLYPH_OK),
            Self::Caution => Some(tokens::GLYPH_CAUTION),
            Self::Danger => Some(tokens::GLYPH_DANGER),
            Self::Unknown => Some(tokens::GLYPH_UNKNOWN),
            Self::Info => None,
        }
    }

    /// Sort order for a notice stack: most urgent first (ruling R10).
    ///
    /// §4.11 listed four roles and omitted `Ok`. It sorts last: "this one
    /// passed" is a legitimate notice and it needs the least attention.
    #[must_use]
    pub fn severity_rank(self) -> u8 {
        match self {
            Self::Danger => 0,
            Self::Caution => 1,
            Self::Unknown => 2,
            Self::Info => 3,
            Self::Ok => 4,
        }
    }
}

/// A small tinted rectangle carrying a verdict.
pub struct StatusChip {
    word: String,
    role: Role,
    hover: Option<String>,
}

impl StatusChip {
    pub fn new(word: impl Into<String>, role: Role) -> Self {
        Self {
            word: word.into(),
            role,
            hover: None,
        }
    }

    /// Hover text. The words come from the caller and are not invented here.
    #[must_use]
    pub fn hover(mut self, hover: impl Into<String>) -> Self {
        self.hover = Some(hover.into());
        self
    }
}

impl egui::Widget for StatusChip {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        // The glyph leads, then the word, one SPACE_1 apart (§2.1).
        let label = match self.role.glyph() {
            Some(g) => format!("{g}\u{2009}{}", self.word),
            None => self.word.clone(),
        };

        let response = egui::Frame::default()
            .fill(self.role.tint())
            // Ruling R2: the stroke is the BORDER token, never the role text
            // at an alpha. A per-role blend is a computed value the literal
            // sentry cannot see and that differs at every call site.
            .stroke(egui::Stroke::new(1.0, tokens::BORDER))
            .corner_radius(tokens::RADIUS_SM)
            .inner_margin(egui::Margin::symmetric(
                tokens::SPACE_2 as i8,
                tokens::SPACE_1 as i8,
            ))
            .show(ui, |ui| {
                ui.add(egui::Label::new(
                    text::micro(&label).color(self.role.text()),
                ));
            })
            .response;

        match self.hover {
            Some(h) => response.on_hover_text(h),
            None => response,
        }
    }
}
