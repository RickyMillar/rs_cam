//! Unified diagnostic model — single source of truth for any
//! actionable, advisory, or stateful finding about a toolpath / setup /
//! project.
//!
//! # Why this module exists
//!
//! Before unification there were five independent diagnostic surfaces:
//!
//! 1. GUI `collect_warnings` — heuristic stepover/feed/depth ratios
//! 2. [`crate::feeds::FeedsResult::warnings`] — clamp / slot / scallop
//!    notices from the feeds calculator
//! 3. [`crate::tool_load::ToolLoadReport`] — chipload / power /
//!    deflection / drill-gate verdicts
//! 4. [`crate::session::ProjectDiagnostics::verdicts`] — collision,
//!    air-cut, plunge stress, generated-empty
//! 5. [`crate::compute::validate::StaleDefault`] — stale-default
//!    fixer rules
//!
//! Each emitted shape-incompatible findings, sometimes contradicting
//! each other ("X above recommended" vs "chipload within"), with no
//! mechanism to dedupe or prioritize.
//!
//! This module supplies a single [`Diagnostic`] schema and a set of
//! adapters that translate each upstream source into it. The
//! [`apply_supersession`] reducer drops heuristic items when verified
//! evidence covers the same dimension.
//!
//! # Consumers
//!
//! The PR-2 landing (this commit) is plumbing only — the existing
//! consumers still read their original surfaces. PR-3 cuts the GUI
//! params panel and the MCP `get_toolpath_diagnostics` endpoint over
//! to [`crate::diagnostics::diagnose_toolpath`] and friends; PR-4
//! wires sim-evidence freshness through [`DiagnosticState`].
//!
//! # Schema design
//!
//! Four orthogonal axes — keep them independent in the UI:
//!
//! - [`Severity`] — consequence: will it break the tool, ruin the
//!   part, waste a run, or just whisper?
//! - [`Category`] — kind of finding: safety, geometry, tool-load,
//!   quality, efficiency, state.
//! - [`Confidence`] — evidence quality: vendor LUT row / sim sample
//!   cited, or rule-of-thumb.
//! - [`DiagnosticState`] — freshness: current evidence, needs sim,
//!   stale, or not applicable.
//!
//! Conflating any pair (e.g. mixing `state = NeedsSimulation` into
//! the severity ladder) is what produced the prior UX confusion
//! between "we couldn't measure it" and "it's dangerous".

pub mod adapters;
pub mod diagnose;
pub mod evidence;
pub mod fix;
pub mod ids;
pub mod supersession;

#[cfg(test)]
mod tests;

pub use diagnose::{
    ToolpathDiagnoseInputs, diagnose_project_diagnostics, diagnose_toolpath_inputs,
};
pub use evidence::DiagnosticEvidence;
pub use fix::DiagnosticFix;
pub use supersession::apply_supersession;

use serde::{Deserialize, Serialize};

/// Stable string identifier for a diagnostic rule. Adapters use the
/// constants in [`ids`] so downstream code can match on rule kind
/// without parsing the human message.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DiagnosticId(pub String);

impl DiagnosticId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for DiagnosticId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl From<String> for DiagnosticId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// What this diagnostic refers to. Adapters pick the narrowest scope
/// the upstream source can express.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Scope {
    Project,
    Setup {
        id: usize,
    },
    Toolpath {
        id: usize,
    },
    Tool {
        id: usize,
    },
    /// Sub-toolpath: specific move range. `start` and `end` are
    /// toolpath-local move indices.
    MoveRange {
        toolpath_id: usize,
        start: usize,
        end: usize,
    },
}

/// Consequence ladder. Bigger consequence → higher rank. Compare with
/// [`Severity::rank`] so the UI can sort and threshold by severity
/// without hardcoding the enum order.
///
/// Definitions:
/// - **Blocking** — generation cannot proceed (geometry impossible,
///   required input missing). UI shows red; export gate refuses.
/// - **Critical** — will break the tool, ruin the part, or violate
///   safety. Action required before run.
/// - **Caution** — degrades quality or stresses the tool; the operator
///   should consciously approve before running.
/// - **Hint** — preflight nudge; vanishes when verified evidence
///   supersedes it.
/// - **Info** — educational ("very fine stepover — slower cycle");
///   never blocks anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Hint,
    Caution,
    Critical,
    Blocking,
}

impl Severity {
    /// Numeric rank — useful when the discriminant order isn't enough
    /// (e.g. when the UI needs "show max severity badge").
    pub fn rank(self) -> u8 {
        match self {
            Self::Info => 0,
            Self::Hint => 1,
            Self::Caution => 2,
            Self::Critical => 3,
            Self::Blocking => 4,
        }
    }

    /// True for findings the user must look at before running. Used by
    /// the export gate and the top-of-panel ribbon.
    pub fn is_actionable(self) -> bool {
        self.rank() >= Self::Caution.rank()
    }
}

/// Topical bucket. The UI uses this to group findings under section
/// headers and to colour-code without overloading severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// Will damage the part, the tool, or the machine. Collisions,
    /// over-deflection, plunge-rate violations.
    Safety,
    /// Geometric impossibility or Z-frame ordering bug. Stepover >
    /// diameter, bottom > top, retract < feed.
    Geometry,
    /// Cutting envelope (chipload / power / deflection / drill gates).
    ToolLoad,
    /// Surface finish, scallop height, scallop-leaving stepover.
    Quality,
    /// Cycle time, air-cut percentage, low-engagement waste.
    Efficiency,
    /// Workflow / freshness: needs simulation, needs regenerate,
    /// pending compute.
    State,
}

/// Trust level for the finding. The UI surfaces this beside the
/// severity badge so users don't anchor on `Approximate` numbers as
/// hard limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// Every input independently validated (e.g. vendor LUT row +
    /// sim sample with arc engagement captured).
    Verified,
    /// Physics or empirical model with disclosed simplifications
    /// (e.g. cantilever bending only, isotropic Kc).
    Approximate,
    /// Derived from config inputs alone — no sim, no LUT extrapolation.
    /// Geometric inequalities (stepover > diameter) live here.
    Static,
    /// Rule of thumb. Pre-sim hints fall here; they are suppressed
    /// whenever verified evidence covers the same dimension.
    Heuristic,
}

/// Whether the diagnostic's underlying evidence is fresh, missing, or
/// not applicable. Distinct from [`Severity`] so the UI can render
/// "needs simulation" without flashing yellow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticState {
    /// Evidence is current and reflects the toolpath as configured.
    Current,
    /// The diagnostic kind needs a simulation trace; none exists.
    NeedsSimulation,
    /// A simulation trace exists but the toolpath or upstream inputs
    /// changed since it was captured — re-run to verify.
    StaleEvidence,
    /// The diagnostic kind does not apply to this op (e.g. milling
    /// chipload on a drill cycle). Surfaced as state, not severity,
    /// so it never reads as a warning.
    NotApplicable,
}

/// Which upstream module produced the finding. Adapters set this so
/// consumers can filter by provenance without inspecting message text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    StaticValidation,
    FeedsCalculator,
    ToolLoad,
    Simulation,
    StaleDefault,
}

/// One unified diagnostic. Construct via adapters in
/// [`crate::diagnostics::adapters`]; consumers read the assembled
/// list out of [`diagnose_toolpath`] (PR-3).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Stable rule key (e.g. `"load.chipload.low"`). See [`ids`] for
    /// the constants adapters reach for.
    pub id: DiagnosticId,
    pub scope: Scope,
    pub category: Category,
    pub severity: Severity,
    pub confidence: Confidence,
    pub state: DiagnosticState,
    pub source: Source,
    /// One-line operator-facing message. Adapters keep this terse
    /// (the UI panel renders a list of these and details on click).
    pub message: String,
    /// Where the finding came from — sim sample range, LUT row,
    /// geometric inequality. Optional because some workflow notices
    /// (needs-simulation, generated-empty) have no specific evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<DiagnosticEvidence>,
    /// Auto-applyable fix payload. `None` when the finding is purely
    /// advisory (no canonical fix the system can apply automatically).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<DiagnosticFix>,
    /// Stable IDs of diagnostics this one supersedes. The
    /// [`apply_supersession`] reducer drops any diagnostic in this
    /// list whose `id` matches an active (state = `Current`)
    /// diagnostic's supersedes entry.
    ///
    /// Example: `load.chipload` (sim-backed) lists
    /// `feeds.feed_vs_lut_high`, `feeds.feed_vs_lut_low`,
    /// `feeds.stepover_vs_lut`, `feeds.dpp_vs_lut`. When sim evidence
    /// is current, the heuristic pre-sim hints vanish — no
    /// duplicated noise.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supersedes: Vec<DiagnosticId>,
    /// Debug-only sidecar — IDs of diagnostics that *were* dropped by
    /// the reducer because this one superseded them. Populated by
    /// [`apply_supersession`] when a heuristic was hidden by Current
    /// evidence, so consumers (or `rs-cam` MCP agents) can surface
    /// "this hint was hidden because chipload.within is current."
    ///
    /// Empty when no shadow heuristics were present in the input or
    /// when this diagnostic was not the one doing the superseding.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suppressed_diagnostics: Vec<DiagnosticId>,
}

impl Diagnostic {
    /// True when this diagnostic should affect the toolpath badge in
    /// the headline (Caution or worse, Current evidence).
    pub fn is_headline(&self) -> bool {
        self.state == DiagnosticState::Current && self.severity.is_actionable()
    }
}
