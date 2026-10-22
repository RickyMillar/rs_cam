//! Data-driven post-processor definition.
//!
//! A `PostDefinition` captures a controller's dialect as **data** rather
//! than as Rust code: decimal-place rules, preamble/postamble templates,
//! comment style, and (future) limits and command overrides. Four
//! built-in dialects ship as TOML files embedded via `include_str!`
//! (`grbl`, `grblhal`, `linuxcnc`, `mach3`); end users can layer custom
//! posts alongside in a future config-dir lookup.
//!
//! The intended consumer is `gcode::emitter` — it walks a `Program` IR
//! and renders bytes using `PostDefinition` formatting rules, replacing
//! the old `PostProcessor` trait + three impls. See
//! `planning/GCODE_EXPORT_OVERHAUL.md` Phase 3 for the broader rationale.

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// Spindle speed in revolutions per minute. Newtype to prevent mixing
/// with feedrate or other scalars at function boundaries.
///
/// Every `u32` is a valid speed. `S0` is "spindle off" (see the G5 rule in
/// `emitter::clamp_rpm`), so the constructor does not refuse zero.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Rpm(u32);

impl Rpm {
    #[must_use]
    pub const fn new(rpm: u32) -> Self {
        Self(rpm)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// A value that is not in the domain of a G-code quantity: `Feedrate`
/// needs a finite value above zero, `SafeZ` needs a finite value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuantityOutOfDomain {
    /// The quantity name, for example `"feedrate"`.
    pub quantity: &'static str,
    /// The value the constructor refused.
    pub value: f64,
}

impl std::fmt::Display for QuantityOutOfDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {} is out of its domain", self.quantity, self.value)
    }
}

impl std::error::Error for QuantityOutOfDomain {}

/// Tool feedrate in mm/min. Newtype guards against unit mixing
/// (see plan: "formatting bugs from unit mixing have killed real machines").
///
/// [`Feedrate::new`] is the checked door: a feed is finite and above zero
/// (`F0` moves nothing). A post TOML value goes through it. The program
/// builder uses `Feedrate::from_toolpath`: the toolpath owns the value,
/// and the builder passes it through unchanged, so the G-code bytes do
/// not change.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Feedrate(f64);

impl Feedrate {
    /// A feedrate in mm/min. `None` when the value is not finite or not
    /// above zero.
    #[must_use]
    pub fn new(mm_min: f64) -> Option<Self> {
        (mm_min.is_finite() && mm_min > 0.0).then_some(Self(mm_min))
    }

    /// The feed of a toolpath move, unchanged. The builder converts once
    /// here, at the door from `MoveType` to `Statement`.
    #[must_use]
    pub(crate) const fn from_toolpath(mm_min: f64) -> Self {
        Self(mm_min)
    }

    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for Feedrate {
    type Error = QuantityOutOfDomain;

    fn try_from(mm_min: f64) -> Result<Self, Self::Error> {
        Self::new(mm_min).ok_or(QuantityOutOfDomain {
            quantity: "feedrate",
            value: mm_min,
        })
    }
}

impl From<Feedrate> for f64 {
    fn from(feed: Feedrate) -> Self {
        feed.0
    }
}

/// Safe-Z retract height in mm (in the active WCS).
///
/// [`SafeZ::new`] is the checked door: a height is finite. Zero and
/// negative heights are valid, because a local Z frame can put the
/// retract plane below zero. The program builder uses
/// `SafeZ::from_toolpath` for a toolpath Z or the export safe Z, and
/// passes the value through unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct SafeZ(f64);

impl SafeZ {
    /// A retract height in mm. `None` when the value is not finite.
    #[must_use]
    pub fn new(z: f64) -> Option<Self> {
        z.is_finite().then_some(Self(z))
    }

    /// A toolpath or export Z, unchanged. The builder converts once here.
    #[must_use]
    pub(crate) const fn from_toolpath(z: f64) -> Self {
        Self(z)
    }

    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }

    /// The higher of the two heights (`f64::max`).
    #[must_use]
    pub fn max(self, other: Self) -> Self {
        Self(self.0.max(other.0))
    }
}

impl TryFrom<f64> for SafeZ {
    type Error = QuantityOutOfDomain;

    fn try_from(z: f64) -> Result<Self, Self::Error> {
        Self::new(z).ok_or(QuantityOutOfDomain {
            quantity: "safe Z",
            value: z,
        })
    }
}

impl From<SafeZ> for f64 {
    fn from(z: SafeZ) -> Self {
        z.0
    }
}

/// Per-axis decimal places for emitted move words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct PostLimits {
    #[serde(default)]
    pub max_rpm: Option<Rpm>,
    /// G5: the lowest S the controller runs. A non-zero S below it is
    /// raised to it, with a warning comment.
    #[serde(default)]
    pub min_rpm: Option<Rpm>,
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
#[serde(deny_unknown_fields)]
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

/// Collapse newlines in comment text so the rendered `(...)` block
/// stays on one line. Tabs and CR are also collapsed for parser safety.
///
/// Parentheses are mapped to square brackets: GRBL (and rs274-family
/// parsers) end a `(...)` comment at the FIRST `)`, so a toolpath named
/// `Rivers (back)` would render `(Rivers (back))` and leave a stray `)`
/// on the line as bare g-code.
///
/// The output is ASCII only (G-ASCII, 2026-10-08). The controller reads
/// a byte above 0x7F as a real-time command, also inside a comment:
///
/// - grblHAL: `protocol_enqueue_realtime_command` (protocol.c:833)
///   acts on 0x80 (protocol.c:883) and on the override bytes 0x90..0x9E
///   (protocol.c:958-979) before it looks at the comment flags.
/// - Grbl 1.1: the serial ISR acts on every byte above 0x7F
///   (serial.c:156-183).
///
/// A UTF-8 em dash is `E2 80 94`, and 0x94 is "feed override -1 %"
/// (grbl.h:132, Grbl 1.1 config.h:71). So each `—` in a toolpath name
/// lowered the feed override by 1 %. A known character maps to an ASCII
/// look-alike. Any other non-ASCII character maps to `_`.
///
/// The printable real-time characters `!` (feed hold), `~` (cycle start)
/// and `?` (status) also map. Grbl 1.1 acts on them anywhere in the
/// stream, comments included (serial.c:151-154, config.h:52-54). A `~`
/// that the sender streams after an `M0` would resume the pause.
/// DEL (0x7F) is a backspace for grblHAL (protocol.c:320-325), so it
/// maps to a space, as the other control characters do.
pub(crate) fn sanitize_comment_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\n' => out.push_str(" / "),
            '\r' | '\t' => out.push(' '),
            '(' => out.push('['),
            ')' => out.push(']'),
            '!' | '?' => out.push('.'),
            '~' => out.push('-'),
            c if c.is_ascii_control() => out.push(' '),
            c if c.is_ascii() => out.push(c),
            other => out.push_str(ascii_look_alike(other)),
        }
    }
    out
}

/// The ASCII text that stands in for one non-ASCII character in a
/// comment. See [`sanitize_comment_text`].
fn ascii_look_alike(ch: char) -> &'static str {
    match ch {
        '\u{2010}'..='\u{2015}' | '\u{2212}' => "-",
        '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{2032}' => "'",
        '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{2033}' => "\"",
        '\u{2026}' => "...",
        '\u{00D7}' => "x",
        '\u{00B0}' => "deg",
        '\u{00B1}' => "+/-",
        '\u{2264}' => "<=",
        '\u{2265}' => ">=",
        '\u{2192}' => "->",
        '\u{2190}' => "<-",
        '\u{00B5}' | '\u{03BC}' => "u",
        '\u{00A0}' => " ",
        _ => "_",
    }
}

/// Data-driven post processor definition. Loaded from TOML.
///
/// Templates use `{spindle_rpm}` (preamble) and `{message_comment}`
/// (program_pause); `comment.format` is a single line containing
/// `{text}`. Move-line formatting is hard-coded in the emitter and
/// driven by `decimals`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the struct mirrors the post TOML file, and each bool is one key of that file"
)]
pub struct PostDefinition {
    pub name: String,
    pub decimals: Decimals,
    pub preamble: String,
    pub postamble: String,
    pub program_pause: String,
    /// Multi-line tool-change template. Tokens:
    ///
    /// - `{tool_number}` → the display T-number of the incoming tool
    /// - `{message_comment}` → `TOOL CHANGE: <label> [T<n>]` wrapped in
    ///   this post's comment style
    ///
    /// Required (F090: no default for a post that omits it). The M6
    /// posts write `M5` + `M6 T{tool_number}`. Controllers without M6
    /// (vanilla GRBL) template a spindle-stop + message + `M0` pause
    /// instead; the `SpindleSet` emitted right after the change doubles
    /// as the resume spin-up.
    pub tool_change: String,
    pub comment: CommentStyle,
    /// The format of an operator message: `{message_comment}` in the
    /// `program_pause` and `tool_change` templates. It contains `{text}`.
    /// `None` uses `comment.format`.
    ///
    /// grblHAL sets `(MSG,{text})`. The parser sends the text of a
    /// comment that starts with `MSG,` to the sender as a message
    /// (gcode.c:1139-1150). Grbl 1.1 has no MSG (gcode.c:387), so the GRBL
    /// post keeps the plain comment.
    #[serde(default)]
    pub message_format: Option<String>,
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
    /// Arc-linearisation policy applied by the emitter.
    #[serde(default)]
    pub arc_linearize: ArcLinearize,
    /// M-codes the controller does not implement. Lines containing any
    /// of these M-words are dropped at emit time and replaced with a
    /// warning comment. Use this for user pre/post snippets that target
    /// a different controller than the project's post.
    ///
    /// Examples:
    /// - Grbl 1.1 lacks M7 (mist coolant) — list `7` here so user
    ///   snippets that emit `M7` get commented out instead of failing
    ///   the parser.
    #[serde(default)]
    pub unsupported_mcodes: Vec<u32>,
    /// Whether the controller implements cutter compensation
    /// (G40/G41/G42). When false, comp lines from program_builder are
    /// dropped at emit time with a warning comment. Grbl 1.1 has no
    /// cutter comp; LinuxCNC and Mach3 do.
    #[serde(default = "default_supports_cutter_comp")]
    pub supports_cutter_comp: bool,
    /// G3: a project with two or more setups exports one file per setup
    /// by default. True for grblHAL, where an `M0` setup pause is a feed
    /// hold that refuses a jog (gcode.c:4979-4981, system.c:239-242), so
    /// the operator cannot re-zero inside one program. A single-file
    /// export is still possible; it keeps the `M0`.
    #[serde(default)]
    pub prefer_one_file_per_setup: bool,
    /// G7: the controller runs G81/G82/G83 drill cycles, so the export
    /// option "native drill cycles" applies. True for grblHAL
    /// (gcode.c:1606-1613). Off by default in the export.
    #[serde(default)]
    pub canned_drill_cycles: bool,
    /// G6: M7 depends on the board, so the export option "mist output
    /// present" may lift it off `unsupported_mcodes`. True for grblHAL
    /// (gcode.c:1855-1856 refuses M7 only when the board has no mist
    /// output). False for Grbl 1.1, whose post denies M7 always.
    #[serde(default)]
    pub mist_option: bool,
}

fn default_supports_cutter_comp() -> bool {
    // Default true preserves the existing emission behaviour for the
    // LinuxCNC/Mach3 posts (which DO support comp). Grbl/grblHAL TOMLs
    // override to false explicitly.
    true
}

/// Comment formatting. `format` contains `{text}`; the emitter renders
/// `format!("{}\n", format.replace("{text}", text))`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
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
        self.render_preamble_with_tool(rpm, None)
    }

    /// [`Self::render_preamble`], plus the `{first_tool_change}` token
    /// (G2): this post's tool-change block for `first_tool` when the block
    /// commands M6, else nothing. A pause-style template never writes an
    /// M0 at the program start.
    pub fn render_preamble_with_tool(&self, rpm: u32, first_tool: Option<(u32, &str)>) -> String {
        let wcs_word = self.wcs.map(WcsCode::as_word).unwrap_or("");
        let wcs_line = match self.wcs {
            Some(w) => format!("{}\n", w.as_word()),
            None => String::new(),
        };
        let first_tool_change = match first_tool {
            Some((number, label)) if self.tool_change_commands_m6() => {
                self.render_tool_change(number, label)
            }
            _ => String::new(),
        };
        self.preamble
            .replace("{first_tool_change}", &first_tool_change)
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
    ///
    /// Embedded `\n` in `text` is collapsed to ` / ` so the comment
    /// stays on a single line — controllers reject `(...)` blocks that
    /// span multiple lines (the second line is treated as bare g-code).
    pub fn render_comment(&self, text: &str) -> String {
        let sanitized = sanitize_comment_text(text);
        format!("{}\n", self.comment.format.replace("{text}", &sanitized))
    }

    /// Render one operator message in this post's message format (see
    /// [`Self::message_format`]), with no trailing newline. The text is
    /// sanitised as a comment is (ASCII only, one line).
    fn render_message(&self, message: &str) -> String {
        let sanitized = sanitize_comment_text(message);
        self.message_format
            .as_deref()
            .unwrap_or(&self.comment.format)
            .replace("{text}", &sanitized)
    }

    /// Render one operator message as a whole line, in this post's
    /// message format: `(MSG,<text>)` on grblHAL, a comment elsewhere.
    pub fn render_message_line(&self, message: &str) -> String {
        format!("{}\n", self.render_message(message))
    }

    /// Render a program-pause block. Substitutes `{message_comment}`
    /// with the message in this post's message format (no trailing
    /// newline — the template provides it). Multi-line messages are
    /// collapsed (see `render_comment`).
    pub fn render_program_pause(&self, message: &str) -> String {
        self.program_pause
            .replace("{message_comment}", &self.render_message(message))
    }

    /// Render a tool-change block from the post's `tool_change`
    /// template. Substitutes `{tool_number}` with the display T-number
    /// and `{message_comment}` with `TOOL CHANGE: <label> [T<n>]`
    /// wrapped in this post's comment style. The operator message uses
    /// square brackets (and `sanitize_comment_text` maps any parens in
    /// the label to brackets) so it stays parser-safe inside a `(...)`
    /// comment.
    pub fn render_tool_change(&self, tool_number: u32, label: &str) -> String {
        let message = format!("TOOL CHANGE: {label} [T{tool_number}]");
        self.tool_change
            .replace("{tool_number}", &tool_number.to_string())
            .replace("{message_comment}", &self.render_message(&message))
    }

    /// True when the `tool_change` template commands `M6`: the controller
    /// runs the change. False for a pause-style template (`M0`).
    pub fn tool_change_commands_m6(&self) -> bool {
        self.tool_change
            .lines()
            .any(|l| l.split_whitespace().any(|w| w.eq_ignore_ascii_case("M6")))
    }
}

// ----- shipped posts (TOML embedded at build time) -----

const GRBL_TOML: &str = include_str!("../../posts/grbl.toml");
const GRBLHAL_TOML: &str = include_str!("../../posts/grblhal.toml");
const LINUXCNC_TOML: &str = include_str!("../../posts/linuxcnc.toml");
const MACH3_TOML: &str = include_str!("../../posts/mach3.toml");

static GRBL: OnceLock<PostDefinition> = OnceLock::new();
static GRBLHAL: OnceLock<PostDefinition> = OnceLock::new();
static LINUXCNC: OnceLock<PostDefinition> = OnceLock::new();
static MACH3: OnceLock<PostDefinition> = OnceLock::new();

/// The shipped GRBL post definition.
pub fn grbl() -> &'static PostDefinition {
    GRBL.get_or_init(|| {
        #[expect(
            clippy::expect_used,
            reason = "the shipped TOML is validated by `posts_load_*` tests below. A malformed shipped TOML would fail those tests in CI before reaching production, so unwrap-on-init is acceptable here."
        )]
        PostDefinition::from_toml(GRBL_TOML).expect("shipped grbl.toml must parse")
    })
}

/// The shipped grblHAL post definition.
pub fn grblhal() -> &'static PostDefinition {
    GRBLHAL.get_or_init(|| {
        #[expect(
            clippy::expect_used,
            reason = "see `grbl()` — shipped TOML is test-gated."
        )]
        PostDefinition::from_toml(GRBLHAL_TOML).expect("shipped grblhal.toml must parse")
    })
}

/// The shipped LinuxCNC post definition.
pub fn linuxcnc() -> &'static PostDefinition {
    LINUXCNC.get_or_init(|| {
        #[expect(
            clippy::expect_used,
            reason = "see `grbl()` — shipped TOML is test-gated."
        )]
        PostDefinition::from_toml(LINUXCNC_TOML).expect("shipped linuxcnc.toml must parse")
    })
}

/// The shipped Mach3 post definition.
pub fn mach3() -> &'static PostDefinition {
    MACH3.get_or_init(|| {
        #[expect(
            clippy::expect_used,
            reason = "see `grbl()` — shipped TOML is test-gated."
        )]
        PostDefinition::from_toml(MACH3_TOML).expect("shipped mach3.toml must parse")
    })
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    #[test]
    fn shipped_posts_load() {
        // Each shipped TOML must parse and have sensible decimals.
        for post in [grbl(), grblhal(), linuxcnc(), mach3()] {
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
            tool_change = "M5\nM6 T{tool_number}\n"
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
                tool_change = "M5\nM6 T{{tool_number}}\n"
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
    fn grblhal_post_has_g54_and_units_metadata() {
        let p = grblhal();
        assert_eq!(p.wcs, Some(WcsCode::G54));
        assert_eq!(p.units, Units::Mm);
        let preamble = p.render_preamble(18_000);
        assert!(preamble.contains("G54\n"), "grblhal preamble: {preamble}");
        assert!(preamble.contains("M3 S18000"));
    }

    #[test]
    fn shipped_posts_enable_arc_linearize() {
        // Phase 4b: every shipped post enables arc linearisation at the
        // 0.05mm default threshold to dodge offline-parser rejections
        // on sub-mm arcs (real bug surfaced by F10 fixture).
        for post in [grbl(), grblhal(), linuxcnc(), mach3()] {
            assert!(
                post.arc_linearize.enabled,
                "{}: arc_linearize disabled",
                post.name
            );
            assert!(
                (post.arc_linearize.threshold_mm - 0.05).abs() < 1e-9,
                "{}: threshold should be 0.05, got {}",
                post.name,
                post.arc_linearize.threshold_mm
            );
        }
    }

    #[test]
    fn comment_renderer_collapses_newlines() {
        let p = grbl();
        let c = p.render_comment("line one\nline two\rline three\tafter tab");
        assert_eq!(c, "(line one / line two line three after tab)\n");
        // Exactly one trailing newline; no embedded newlines.
        assert_eq!(c.matches('\n').count(), 1);
        assert!(c.ends_with('\n'));
    }

    #[test]
    fn shipped_post_unsupported_mcodes() {
        assert_eq!(grbl().unsupported_mcodes, vec![6, 7]);
        // G6: grblHAL refuses M7 without a mist output (gcode.c:1855-1856).
        // The export option "mist output present" lifts it.
        assert_eq!(grblhal().unsupported_mcodes, vec![7]);
        assert!(linuxcnc().unsupported_mcodes.is_empty());
        assert!(mach3().unsupported_mcodes.is_empty());
    }

    #[test]
    fn comment_renderer_maps_parens_to_brackets() {
        // GRBL ends a comment at the first ')' — nested parens in a
        // toolpath name would leave a stray ')' as bare g-code.
        let p = grbl();
        assert_eq!(p.render_comment("Rivers (back)"), "(Rivers [back])\n");
    }

    #[test]
    fn shipped_post_tool_change_templates() {
        // Grbl 1.1: manual change. Spindle off, operator message, M0
        // pause. Resume spin-up comes from the SpindleSet that the
        // program builder emits right after the ToolChange statement.
        assert_eq!(grbl().tool_change, "M5\n{message_comment}\nM0\n");
        assert!(!grbl().tool_change_commands_m6());
        // grblHAL (G2, operator ruling 2026-10-08): the board runs the
        // change. M5, the operator message, then M6 T<n>.
        assert_eq!(
            grblhal().tool_change,
            "M5\n{message_comment}\nM6 T{tool_number}\n"
        );
        assert!(grblhal().tool_change_commands_m6());
        // LinuxCNC / Mach3: native M6.
        for p in [linuxcnc(), mach3()] {
            assert_eq!(
                p.tool_change, "M5\nM6 T{tool_number}\n",
                "{}: expected M6-style tool change",
                p.name
            );
            assert!(p.tool_change_commands_m6());
        }
    }

    #[test]
    fn render_tool_change_substitutes_number_and_message() {
        let block = linuxcnc().render_tool_change(2, "Tapered Ball 2mm");
        assert_eq!(block, "M5\nM6 T2\n");

        let block = grbl().render_tool_change(2, "Tapered Ball 2mm");
        assert_eq!(block, "M5\n(TOOL CHANGE: Tapered Ball 2mm [T2])\nM0\n");

        // G4: grblHAL writes the message as a MSG comment.
        let block = grblhal().render_tool_change(2, "Tapered Ball 2mm");
        assert_eq!(
            block,
            "M5\n(MSG,TOOL CHANGE: Tapered Ball 2mm [T2])\nM6 T2\n"
        );
    }

    #[test]
    fn grblhal_program_pause_uses_msg_and_says_why() {
        // G3 + G4: the setup pause names the setup in a MSG and says
        // first why a single file is a poor fit on grblHAL.
        let pause = grblhal().render_program_pause("Setup change: Bottom");
        assert_eq!(
            pause,
            "M5\n(MSG,M0 hold: no jog. To re-zero, export one file per setup.)\n\
             (MSG,Setup change: Bottom)\nM0\n"
        );
        // The GRBL post keeps a plain comment (Grbl 1.1 has no MSG).
        assert!(
            grbl()
                .render_program_pause("Setup change: Bottom")
                .contains("\n(Setup change: Bottom)\n")
        );
    }

    /// G-ASCII (SAFETY): every byte of a rendered comment is ASCII, and
    /// the printable real-time characters `!`, `~`, `?` are gone. An em
    /// dash was `E2 80 94`; 0x94 lowers the feed override by 1 %.
    #[test]
    fn comment_renderer_is_ascii_only() {
        let text = "Op 0 \u{2014} pocket \u{00D7}2 at 45\u{00B0}! ok? ~x \u{4E2D}\u{007F}";
        for post in [grbl(), grblhal(), linuxcnc(), mach3()] {
            for rendered in [
                post.render_comment(text),
                post.render_program_pause(text),
                post.render_tool_change(1, text),
            ] {
                assert!(rendered.is_ascii(), "{}: {rendered:?}", post.name);
                let body: String = rendered.lines().filter(|l| l.starts_with('(')).collect();
                assert!(
                    !body.contains(['!', '~', '?']),
                    "{}: real-time character left in {body:?}",
                    post.name
                );
            }
        }
        assert_eq!(
            grbl().render_comment(text),
            "(Op 0 - pocket x2 at 45deg. ok. -x _ )\n"
        );
    }

    #[test]
    fn render_tool_change_sanitizes_label_parens() {
        let block = grbl().render_tool_change(3, "End Mill (rough)");
        assert_eq!(block, "M5\n(TOOL CHANGE: End Mill [rough] [T3])\nM0\n");
    }

    /// F090 and F034: a post TOML without `tool_change` refuses (no
    /// default), and so does a key the post does not hold.
    #[test]
    fn a_post_without_tool_change_or_with_an_unknown_key_refuses() {
        let base = r#"
            name = "Test"
            preamble = "M3 S{spindle_rpm}\n"
            postamble = "M30\n"
            program_pause = "M0\n"
            [decimals]
            xyz = 3
            feed = 0
            ijk = 3
            [comment]
            format = "({text})"
        "#;
        let err = PostDefinition::from_toml(base).unwrap_err();
        assert!(err.to_string().contains("tool_change"), "{err}");
        let with = format!("tool_change = \"M5\\nM6 T{{tool_number}}\\n\"\n{base}");
        let post = PostDefinition::from_toml(&with).unwrap();
        assert_eq!(post.render_tool_change(4, "Bit"), "M5\nM6 T4\n");
        let typo = format!("tool_chnage = \"M0\"\n{with}");
        let err = PostDefinition::from_toml(&typo).unwrap_err();
        assert!(err.to_string().contains("tool_chnage"), "{err}");
    }

    #[test]
    fn shipped_post_supports_cutter_comp() {
        assert!(!grbl().supports_cutter_comp);
        assert!(!grblhal().supports_cutter_comp);
        assert!(linuxcnc().supports_cutter_comp);
        assert!(mach3().supports_cutter_comp);
    }

    /// F005: the checked doors refuse a value outside the quantity domain.
    /// A feed is finite and above zero. A retract height is finite; zero
    /// and negative heights are valid in a local Z frame.
    #[test]
    fn checked_quantity_doors_refuse_out_of_domain_values_f005() {
        assert_eq!(Feedrate::new(600.0).map(Feedrate::get), Some(600.0));
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(Feedrate::new(bad).is_none(), "feedrate {bad} must refuse");
        }
        for good in [-12.5, 0.0, 10.0] {
            assert_eq!(SafeZ::new(good).map(SafeZ::get), Some(good));
        }
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(SafeZ::new(bad).is_none(), "safe Z {bad} must refuse");
        }
        assert_eq!(Rpm::new(0).get(), 0, "S0 is spindle off and stays valid");
        assert_eq!(
            SafeZ::from_toolpath(2.0).max(SafeZ::from_toolpath(5.0)),
            SafeZ::from_toolpath(5.0)
        );
    }

    /// F005: a post TOML limit goes through the checked door. A zero or
    /// NaN `max_feed` refuses at load; a valid one loads unchanged.
    #[test]
    fn a_post_max_feed_outside_its_domain_refuses_at_load_f005() {
        let post_with = |max_feed: &str| {
            r#"
                name = "Capped"
                preamble = "M3 S{spindle_rpm}\n"
                postamble = "M30\n"
                program_pause = "M0\n"
                tool_change = "M5\nM6 T{tool_number}\n"
                [decimals]
                xyz = 3
                feed = 0
                ijk = 3
                [comment]
                format = "({text})"
                [limits]
                max_feed = MAX_FEED
            "#
            .replace("MAX_FEED", max_feed)
        };
        let loaded = PostDefinition::from_toml(&post_with("800.0")).expect("valid post");
        assert_eq!(loaded.limits.max_feed.map(Feedrate::get), Some(800.0));
        for bad in ["0.0", "-5.0", "nan", "inf"] {
            assert!(
                PostDefinition::from_toml(&post_with(bad)).is_err(),
                "max_feed = {bad} must refuse"
            );
        }
    }
}
