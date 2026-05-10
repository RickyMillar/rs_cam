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

/// Optional clamps surfaced to the wizard / validator. Phase 3 only
/// stores them; enforcement lands in Phase 4 alongside the broadened
/// fixture corpus.
#[derive(Debug, Clone, Copy, Default, PartialEq, Deserialize)]
pub struct PostLimits {
    #[serde(default)]
    pub max_rpm: Option<Rpm>,
    #[serde(default)]
    pub max_feed: Option<Feedrate>,
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

    /// Render the preamble with `{spindle_rpm}` substituted.
    pub fn render_preamble(&self, rpm: u32) -> String {
        self.preamble.replace("{spindle_rpm}", &rpm.to_string())
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
}
