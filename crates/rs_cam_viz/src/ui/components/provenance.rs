//! `ProvKind` + `ProvenanceBadge` — the provenance vocabulary, defined once.
//!
//! Component layer (IA cleanup, Wave 1 / W4.1). The audit found the same
//! "where did this value come from" signal reimplemented three times with
//! three different colours: `pill_color_for_source` (green/amber) and
//! `source_short_label` in `properties/mod.rs`, a cyan narrative line in the
//! same file, and a colourless narrative in `feeds_modal.rs` (finding P7-003).
//!
//! `ProvKind` is the single display enum — one glyph and one canonical RGB per
//! source — and [`ProvenanceBadge`] is the single renderer. It is a **status
//! badge, never an action** (P7-003): it shows provenance and explains it on
//! hover; it never writes a value (that is [`super::suggest::SuggestButton`]'s
//! job, a deliberately separate type).
//!
//! `ProvKind` mirrors the core [`ProvenanceSource`] 1:1 (the honest W2.1 data
//! model) plus one display-only `Inherited` variant for mirror / precedence
//! rows (`〈 18000 〉`). The `From` impls below are the bridge from the stored
//! [`ValueProvenance`] into this vocabulary, so a surface renders the *stored*
//! provenance rather than a recomputed lookup.

use egui::Color32;
use rs_cam_core::feeds::{ProvenanceSource, ValueProvenance};

use crate::ui::theme;

/// How a displayed value came to be — the viz display vocabulary.
///
/// Variants 1–6 map 1:1 to core [`ProvenanceSource`]. `Inherited` is
/// display-only: it marks a value shown as a mirror / project default
/// (`〈 value 〉`) rather than one stored on the operation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProvKind {
    VendorLut,
    Formula,
    EdgeRadiusFloor,
    Manual,
    Optimizer,
    AutoCorrect,
    /// Display-only: shown as an inherited / mirrored value, not stored here.
    Inherited,
}

/// Canonical optimizer blue — distinct from [`theme::INFO`] so a sim-validated
/// value is always tellable from a raw vendor-LUT one (P7-004).
const OPTIMIZER_BLUE: Color32 = Color32::from_rgb(90, 150, 220);

impl ProvKind {
    /// The single source of glyphs. One glyph per kind.
    pub fn glyph(self) -> &'static str {
        match self {
            Self::VendorLut => "\u{25A3}",         // ▣
            Self::Formula => "\u{25B2}",           // ▲
            Self::EdgeRadiusFloor => "\u{230A}",   // ⌊
            Self::Manual => "\u{270E}",            // ✎
            Self::Optimizer => "\u{25C6}",         // ◆
            Self::AutoCorrect => "\u{27F2}",       // ⟲
            Self::Inherited => "\u{2329}\u{232A}", // 〈〉
        }
    }

    /// The single canonical RGB per source. Both fallback kinds share the
    /// "verify against vendor data" amber (`theme::WARNING`) — same colour as
    /// the pre-component pills, distinguished only by glyph.
    pub fn color(self) -> Color32 {
        match self {
            Self::VendorLut => theme::SUCCESS_BRIGHT,
            Self::Formula | Self::EdgeRadiusFloor => theme::WARNING,
            Self::Manual => theme::TEXT_MUTED,
            Self::Optimizer => OPTIMIZER_BLUE,
            Self::AutoCorrect => theme::INFO,
            Self::Inherited => theme::TEXT_FAINT,
        }
    }

    /// Short human label. Wording matches the pre-component narratives so the
    /// collapse is byte-faithful (e.g. `formula fallback`, `edge-radius floor`).
    pub fn label(self) -> &'static str {
        match self {
            Self::VendorLut => "vendor LUT",
            Self::Formula => "formula fallback",
            Self::EdgeRadiusFloor => "edge-radius floor",
            Self::Manual => "manual",
            Self::Optimizer => "sim-optimized",
            Self::AutoCorrect => "auto-corrected",
            Self::Inherited => "inherited",
        }
    }

    /// `"vendor LUT (1234)"` when a reference is present, else `"vendor LUT"`.
    pub fn label_with_reference(self, reference: Option<&str>) -> String {
        match reference {
            Some(r) => format!("{} ({r})", self.label()),
            None => self.label().to_owned(),
        }
    }
}

impl From<ProvenanceSource> for ProvKind {
    fn from(source: ProvenanceSource) -> Self {
        match source {
            ProvenanceSource::VendorLut => Self::VendorLut,
            ProvenanceSource::Formula => Self::Formula,
            ProvenanceSource::EdgeRadiusFloor => Self::EdgeRadiusFloor,
            ProvenanceSource::Manual => Self::Manual,
            ProvenanceSource::Optimizer => Self::Optimizer,
            ProvenanceSource::AutoCorrect => Self::AutoCorrect,
        }
    }
}

impl From<&ValueProvenance> for ProvKind {
    fn from(v: &ValueProvenance) -> Self {
        v.source.into()
    }
}

/// Status badge: glyph + label, coloured by [`ProvKind::color`]. Never applies
/// a value — clicking only surfaces the "why" on hover.
pub struct ProvenanceBadge<'a> {
    kind: ProvKind,
    reference: Option<&'a str>,
    compact: bool,
}

impl<'a> ProvenanceBadge<'a> {
    pub fn new(kind: ProvKind) -> Self {
        Self {
            kind,
            reference: None,
            compact: false,
        }
    }

    /// Attach an observation / candidate id, rendered as `▣ vendor LUT (1234)`.
    pub fn reference(mut self, r: &'a str) -> Self {
        self.reference = Some(r);
        self
    }

    /// As [`Self::reference`] but takes an optional id (ergonomic for callers
    /// holding `Option<&str>`).
    pub fn reference_opt(mut self, r: Option<&'a str>) -> Self {
        self.reference = r;
        self
    }

    /// Glyph only (for tight rows); the full label still shows on hover.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }
}

impl egui::Widget for ProvenanceBadge<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let color = self.kind.color();
        let full = self.kind.label_with_reference(self.reference);
        let text = if self.compact {
            self.kind.glyph().to_owned()
        } else {
            format!("{} {full}", self.kind.glyph())
        };
        ui.label(egui::RichText::new(text).small().color(color))
            .on_hover_text(format!("Source: {full}."))
    }
}
