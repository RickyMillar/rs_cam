//! Data-driven post-processor definition.
//!
//! A `PostDefinition` captures a controller's dialect as **data** rather
//! than as Rust code: decimal-place rules, preamble/postamble templates,
//! comment style, and (future) limits and command overrides. Three
//! built-in dialects ship as TOML files embedded via `include_str!`
//! (`grbl`, `linuxcnc`, `mach3`); end users can layer custom posts
//! alongside in a future config-dir lookup.
//!
//! The intended consumer is `gcode::emitter` — it walks a `Program` IR
//! and renders bytes using `PostDefinition` formatting rules, replacing
//! the old `PostProcessor` trait + three impls. See
//! `planning/GCODE_EXPORT_OVERHAUL.md` Phase 3 for the broader rationale.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Spindle speed in revolutions per minute. Newtype to prevent mixing
/// with feedrate or other scalars at function boundaries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Rpm(pub u32);

impl Rpm {
    pub fn get(self) -> u32 {
        self.0
    }
}

/// Tool feedrate in mm/min. Newtype guards against unit mixing
/// (see plan: "formatting bugs from unit mixing have killed real machines").
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Feedrate(pub f64);

impl Feedrate {
    pub fn get(self) -> f64 {
        self.0
    }
}

/// Safe-Z retract height in mm (in the active WCS).
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SafeZ(pub f64);

impl SafeZ {
    pub fn get(self) -> f64 {
        self.0
    }
}

/// Per-axis decimal places for emitted move words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Decimals {
    pub xyz: usize,
    pub feed: usize,
    pub ijk: usize,
}

/// Optional clamps surfaced to the wizard / validator. Enforced by the
/// emitter in Phase 4b: spindle RPM and feedrate words are clamped to
/// `max_rpm` / `max_feed` when present, with a warning comment emitted
/// at the clamp site. Shipped TOMLs leave these unset (no clamping)
/// until the wizard surfaces them or a per-machine config layers on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
pub struct PostLimits {
    #[serde(default)]
    pub max_rpm: Option<Rpm>,
    #[serde(default)]
    pub max_feed: Option<Feedrate>,
}

/// Work-coordinate-system selector. One of G54..G59 (Fanuc-standard
/// six WCS slots). Extended frames (G54.1 P1..P9) intentionally
/// out-of-scope for the 3-axis-router use case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WcsCode {
    G54,
    G55,
    G56,
    G57,
    G58,
    G59,
}

impl WcsCode {
    /// Render as the bare g-code word (no trailing newline).
    pub fn as_word(self) -> &'static str {
        match self {
            WcsCode::G54 => "G54",
            WcsCode::G55 => "G55",
            WcsCode::G56 => "G56",
            WcsCode::G57 => "G57",
            WcsCode::G58 => "G58",
            WcsCode::G59 => "G59",
        }
    }
}

/// Units the post emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Units {
    #[default]
    Mm,
    Inch,
}

impl Units {
    /// G-code modal word: G21 (mm) or G20 (inch).
    pub fn as_word(self) -> &'static str {
        match self {
            Units::Mm => "G21",
            Units::Inch => "G20",
        }
    }
}

/// Arc linearisation: when enabled, arcs whose radius is below
/// `threshold_mm` are emitted as a single chord (G1) instead of a
/// G2/G3 word. Some legacy controllers reject sub-mm arcs outright;
/// linearising sidesteps the rejection at the cost of one chord per
/// micro-arc.
///
/// The conversion happens in `program_builder` so the emitter sees only
/// `Statement::Linear` for linearised arcs — it doesn't need to know.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct ArcLinearize {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_arc_linearize_threshold")]
    pub threshold_mm: f64,
}

impl Default for ArcLinearize {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold_mm: default_arc_linearize_threshold(),
        }
    }
}

fn default_arc_linearize_threshold() -> f64 {
    0.05
}

/// Data-driven post processor definition. Loaded from TOML.
///
/// Templates use `{spindle_rpm}` (preamble) and `{message_comment}`
/// (program_pause); `comment.format` is a single line containing
/// `{text}`. Move-line formatting is hard-coded in the emitter and
/// driven by `decimals`.
#[derive(Debug, Clone, Deserialize)]
pub struct PostDefinition {
    pub name: String,
    pub decimals: Decimals,
    pub preamble: String,
    pub postamble: String,
    pub program_pause: String,
    pub comment: CommentStyle,
    #[serde(default)]
    pub limits: PostLimits,
    /// Default work-coordinate system. When set, the preamble template
    /// can reference `{wcs_word}` (renders to "G54", "G55", ...) and
    /// `{wcs_line}` (renders to "G54\n" — empty if `wcs` is None).
    #[serde(default)]
    pub wcs: Option<WcsCode>,
    /// Units the post emits. Drives `{units_word}` (G21/G20) substitution
    /// in the preamble template.
    #[serde(default)]
    pub units: Units,
    /// Arc-linearisation policy applied by `program_builder`.
    #[serde(default)]
    pub arc_linearize: ArcLinearize,
}

/// Comment formatting. `format` contains `{text}`; the emitter renders
/// `format!("{}\n", format.replace("{text}", text))`.
#[derive(Debug, Clone, Deserialize)]
pub struct CommentStyle {
    pub format: String,
}

/// Errors from loading a `PostDefinition`.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("toml parse error: {0}")]
    Toml(#[from] toml::de::Error),
}

impl PostDefinition {
    /// Parse a TOML string into a `PostDefinition`.
    pub fn from_toml(s: &str) -> Result<Self, LoadError> {
        Ok(toml::from_str(s)?)
    }

    /// Render the preamble. Substitutes the following template tokens:
    ///
    /// - `{spindle_rpm}` → numeric RPM passed in
    /// - `{units_word}` → `G21` or `G20` from `self.units`
    /// - `{wcs_word}` → e.g. `G54` (empty string if `self.wcs` is None)
    /// - `{wcs_line}` → e.g. `G54\n` (empty string if `self.wcs` is None;
    ///   use this in templates instead of `{wcs_word}\n` to avoid leaving
    ///   an empty blank line when no WCS is configured)
    pub fn render_preamble(&self, rpm: u32) -> String {
        let wcs_word = self.wcs.map(WcsCode::as_word).unwrap_or("");
        let wcs_line = match self.wcs {
            Some(w) => format!("{}\n", w.as_word()),
            None => String::new(),
        };
        self.preamble
            .replace("{spindle_rpm}", &rpm.to_string())
            .replace("{units_word}", self.units.as_word())
            .replace("{wcs_word}", wcs_word)
            .replace("{wcs_line}", &wcs_line)
    }

    /// Render the postamble verbatim (no substitutions in Phase 3).
    pub fn render_postamble(&self) -> String {
        self.postamble.clone()
    }

    /// Render a comment line: `format.replace("{text}", text)` + trailing `\n`.
    pub fn render_comment(&self, text: &str) -> String {
        format!("{}\n", self.comment.format.replace("{text}", text))
    }

    /// Render a program-pause block. Substitutes `{message_comment}`
    /// with the message wrapped in this post's comment style (no trailing
    /// newline — the template provides it).
    pub fn render_program_pause(&self, message: &str) -> String {
        let formatted = self.comment.format.replace("{text}", message);
        self.program_pause.replace("{message_comment}", &formatted)
    }
}

// ----- shipped posts (TOML embedded at build time) -----

const GRBL_TOML: &str = include_str!("../../posts/grbl.toml");
const LINUXCNC_TOML: &str = include_str!("../../posts/linuxcnc.toml");
const MACH3_TOML: &str = include_str!("../../posts/mach3.toml");

static GRBL: OnceLock<PostDefinition> = OnceLock::new();
static LINUXCNC: OnceLock<PostDefinition> = OnceLock::new();
static MACH3: OnceLock<PostDefinition> = OnceLock::new();

/// The shipped GRBL post definition.
pub fn grbl() -> &'static PostDefinition {
    GRBL.get_or_init(|| {
        // SAFETY: the shipped TOML is validated by `posts_load_*` tests below.
        // A malformed shipped TOML would fail those tests in CI before
        // reaching production, so unwrap-on-init is acceptable here.
        #[allow(clippy::expect_used)]
        PostDefinition::from_toml(GRBL_TOML).expect("shipped grbl.toml must parse")
    })
}

/// The shipped LinuxCNC post definition.
pub fn linuxcnc() -> &'static PostDefinition {
    LINUXCNC.get_or_init(|| {
        // SAFETY: see `grbl()` — shipped TOML is test-gated.
        #[allow(clippy::expect_used)]
        PostDefinition::from_toml(LINUXCNC_TOML).expect("shipped linuxcnc.toml must parse")
    })
}

/// The shipped Mach3 post definition.
pub fn mach3() -> &'static PostDefinition {
    MACH3.get_or_init(|| {
        // SAFETY: see `grbl()` — shipped TOML is test-gated.
        #[allow(clippy::expect_used)]
        PostDefinition::from_toml(MACH3_TOML).expect("shipped mach3.toml must parse")
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn shipped_posts_load() {
        // Each shipped TOML must parse and have sensible decimals.
        for post in [grbl(), linuxcnc(), mach3()] {
            assert!(!post.name.is_empty(), "{} has empty name", post.name);
            assert!(post.decimals.xyz <= 6, "{} xyz dp absurd", post.name);
            assert!(post.decimals.feed <= 6, "{} feed dp absurd", post.name);
            assert!(post.decimals.ijk <= 6, "{} ijk dp absurd", post.name);
            assert!(
                post.comment.format.contains("{text}"),
                "{} comment.format missing {{text}}",
                post.name
            );
        }
    }

    #[test]
    fn render_preamble_substitutes_rpm() {
        let post = grbl();
        let p = post.render_preamble(18_000);
        assert!(p.contains("M3 S18000"), "rendered preamble: {p}");
        assert!(!p.contains("{spindle_rpm}"));
    }

    #[test]
    fn render_comment_wraps_text() {
        let post = grbl();
        assert_eq!(post.render_comment("Hello"), "(Hello)\n");
    }

    #[test]
    fn render_program_pause_wraps_message() {
        let post = grbl();
        let pause = post.render_program_pause("Rotate stock");
        assert!(pause.contains("M5"));
        assert!(pause.contains("(Rotate stock)"));
        assert!(pause.contains("M0"));
        assert!(!pause.contains("{message_comment}"));
    }

    #[test]
    fn linuxcnc_wcs_field_renders_g54_via_template() {
        // linuxcnc.toml uses {wcs_line} + wcs="G54"; the rendered preamble
        // must contain "G54\n" to match the byte-baseline.
        let p = linuxcnc().render_preamble(18_000);
        assert!(p.contains("G54\n"), "linuxcnc preamble: {p}");
        assert!(!p.contains("{wcs_line}"));
    }

    #[test]
    fn wcs_none_renders_empty_line() {
        // Custom post with wcs = None: {wcs_line} → "" (no blank line).
        let toml = r#"
            name = "Test"
            preamble = """\
(start)
{wcs_line}M3 S{spindle_rpm}
"""
            postamble = "M30\n"
            program_pause = "M0\n"
            [decimals]
            xyz = 3
            feed = 0
            ijk = 3
            [comment]
            format = "({text})"
        "#;
        let post = PostDefinition::from_toml(toml).unwrap();
        assert!(post.wcs.is_none());
        let p = post.render_preamble(1000);
        assert!(!p.contains("{wcs"), "unrendered token: {p}");
        assert!(!p.contains("\n\n"), "blank line leaked: {p}");
    }

    #[test]
    fn units_word_substitutes_g21_g20() {
        let toml = |units: &str| {
            format!(
                r#"
                name = "T"
                units = "{units}"
                preamble = "{{units_word}} M3 S{{spindle_rpm}}\n"
                postamble = "M30\n"
                program_pause = "M0\n"
                [decimals]
                xyz = 3
                feed = 0
                ijk = 3
                [comment]
                format = "({{text}})"
            "#
            )
        };
        let mm = PostDefinition::from_toml(&toml("mm")).unwrap();
        let inch = PostDefinition::from_toml(&toml("inch")).unwrap();
        assert_eq!(mm.units, Units::Mm);
        assert_eq!(inch.units, Units::Inch);
        assert!(mm.render_preamble(1000).starts_with("G21 "));
        assert!(inch.render_preamble(1000).starts_with("G20 "));
    }

    #[test]
    fn arc_linearize_defaults_disabled() {
        for post in [grbl(), linuxcnc(), mach3()] {
            assert!(
                !post.arc_linearize.enabled,
                "{} arc_linearize should default off",
                post.name
            );
            assert!(
                (post.arc_linearize.threshold_mm - 0.05).abs() < 1e-9,
                "{} default threshold should be 0.05",
                post.name
            );
        }
    }
}
