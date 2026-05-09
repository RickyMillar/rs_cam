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
    /// Multi-line postamble block.
    Postamble,
    /// Multi-line program pause (M5 + comment + M0, post-specific).
    ProgramPause { message: String },

    /// Comment rendered via the post's comment style.
    Comment(String),

    /// Verbatim text spliced into output, including any trailing newlines.
    /// Used for: modal-state `writeln!` lines (M5, M3, M6, M7/M8/M9, G40,
    /// G41/G42, G0 Z<safe>) and user-supplied pre/post g-code snippets.
    Raw(String),

    /// Rapid traverse via `post.rapid`.
    Rapid { x: f64, y: f64, z: f64 },
    /// Linear feed with explicit F (first occurrence at this rate).
    Linear {
        x: f64,
        y: f64,
        z: f64,
        feed: f64,
    },
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
}

/// Optional per-program metadata. Empty placeholder for Phase 2; future
/// phases will populate job name, est. time, validator findings, etc.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProgramMetadata {
    pub job_name: Option<String>,
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
