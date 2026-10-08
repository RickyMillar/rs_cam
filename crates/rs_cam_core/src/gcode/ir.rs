//! G-code program intermediate representation.
//!
//! `Statement` is the discrete-event IR built by `program_builder` and
//! consumed by the emitter. Each variant maps 1:1 to a byte slice the
//! current emitter writes, so the IR refactor stays byte-identical:
//!
//! - High-level moves (`Rapid`, `Linear`, `ArcCw/Ccw`) are formatted by
//!   the post-processor's per-move methods (`post.rapid`, `post.linear`,
//!   `post.arc_cw`, `post.arc_ccw`). `LinearModal` carries the elided-F
//!   variant produced by the existing modal `last_feed` book-keeping.
//! - Multi-line blocks (`Preamble`, `Postamble`, `ProgramPause`) defer
//!   to the post's block helpers.
//! - `Comment(String)` is rendered via `post.comment` so dialect
//!   conventions (parens vs semicolons) flow through untouched.
//! - `Raw(String)` covers everything the existing emitter wrote with a
//!   bare `writeln!` — modal-state lines like `M5`, `M3 S<rpm>`,
//!   `M6 T<n>`, coolant `M7`/`M8`/`M9`, controller comp `G40`/`G41 D<n>`,
//!   and user-supplied `pre_gcode`/`post_gcode` snippets. Newlines are
//!   preserved exactly so the emitter can splice them in unchanged.

#[derive(Clone, Debug, PartialEq)]
pub enum Statement {
    /// Multi-line preamble block (post-specific).
    Preamble { spindle_rpm: u32 },
    /// Set spindle speed clockwise: `M3 S<rpm>\n`. Distinct from
    /// `Raw("M3 S<n>\n")` so the emitter can apply `PostLimits.max_rpm`
    /// clamping at this single chokepoint.
    SpindleSet { rpm: u32 },
    /// Multi-line postamble block.
    Postamble,
    /// Multi-line program pause (M5 + comment + M0, post-specific).
    ProgramPause { message: String },

    /// Comment rendered via the post's comment style.
    Comment(String),

    /// Tool change rendered via the post's `tool_change` template.
    /// `tool_number` substitutes `{tool_number}`; `label` (the tool's
    /// display name) feeds the `{message_comment}` operator message
    /// (`TOOL CHANGE: <label> [T<n>]` wrapped in the post's comment
    /// style). Posts without M6 support (vanilla GRBL) template this
    /// as a spindle-stop + message + M0 pause instead of `M6 T{n}`.
    ToolChange { tool_number: u32, label: String },

    /// Verbatim text spliced into output, including any trailing newlines.
    /// Used for: modal-state `writeln!` lines (M5, M3, M6, M7/M8/M9, G40,
    /// G41/G42, G0 Z<safe>) and user-supplied pre/post g-code snippets.
    Raw(String),

    /// Rapid traverse via `post.rapid`.
    Rapid { x: f64, y: f64, z: f64 },
    /// Linear feed with explicit F (first occurrence at this rate).
    Linear { x: f64, y: f64, z: f64, feed: f64 },
    /// Linear feed with elided F (modal — same rate as previous Linear).
    LinearModal { x: f64, y: f64, z: f64 },
    /// Clockwise arc (XY plane, IJK relative center).
    ArcCw {
        x: f64,
        y: f64,
        z: f64,
        i: f64,
        j: f64,
        feed: f64,
    },
    /// Counter-clockwise arc (XY plane, IJK relative center).
    ArcCcw {
        x: f64,
        y: f64,
        z: f64,
        i: f64,
        j: f64,
        feed: f64,
    },
    /// Rapid Z-retract to safe height. Formatted with the post's
    /// `decimal_places` (the only Z-only modal-state line that depends
    /// on per-post precision; everything else is post-agnostic Raw).
    SafeZRetract { z: f64 },
    /// G7: a dwell, `G4 P<seconds>`. The G82 dwell at a hole bottom in an
    /// expanded drill cycle.
    Dwell { seconds: f64 },
    /// G7: one hole of a native canned drill cycle, on a post with
    /// `canned_drill_cycles` and only when the export asks for it.
    /// `first` writes `G98` and the F word. Every line carries X Y Z R and
    /// the P (G82) or Q (G83) word, so a line never depends on a word an
    /// earlier line set.
    CannedDrill {
        kind: CannedDrillKind,
        x: f64,
        y: f64,
        z: f64,
        r: f64,
        feed: f64,
        first: bool,
    },
    /// G7: `G80`, the end of a native canned drill cycle.
    CannedCancel,
}

/// G7: the native drill cycle a hole is written as.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CannedDrillKind {
    /// `G81`: feed to depth, rapid out.
    Simple,
    /// `G82 P<s>`: feed to depth, dwell, rapid out.
    Dwell { seconds: f64 },
    /// `G83 Q<mm>`: peck.
    Peck { peck_mm: f64 },
}

/// Optional per-program metadata. Empty placeholder for Phase 2; future
/// phases will populate job name, est. time, validator findings, etc.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProgramMetadata {
    pub job_name: Option<String>,
    /// G2: the tool the program starts with, as `(T number, label)`. A
    /// preamble with the `{first_tool_change}` token (grblHAL) writes an
    /// `M6` for it, so the first probe after homing sets the tool length
    /// reference with the tool the operator zeroes with.
    pub first_tool: Option<(u32, String)>,
}

/// A complete g-code program: ordered statements plus metadata.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Program {
    pub statements: Vec<Statement>,
    pub metadata: ProgramMetadata,
}

impl Program {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, statement: Statement) {
        self.statements.push(statement);
    }
}
