//! One freshness state per toolpath, derived — never stored.
//!
//! Implements `planning/ui_fix_2026-09-09/research/R0.1.md` §4 option A.
//! Before this module three stores each held a piece of the answer and
//! disagreed: the core result cache (`ProjectSession::results`), a GUI
//! timestamp (`ToolpathRuntime::stale_since`) and a project-wide edit
//! counter. The core cache is now the truth about result validity; the
//! GUI adds only what the core cannot know — that the operator is looking
//! at drawable geometry from a previous generation.
//!
//! `stale_since` keeps its remaining job, the 500 ms auto-regeneration
//! debounce clock, and stops being a claim about correctness.

use rs_cam_core::compute::config::{AwaitingPriorStock, ComputeStatus};
use rs_cam_core::session::{ProjectSession, ToolpathConfig};

use super::runtime::{GuiState, ToolpathRuntime};

/// Where a toolpath stands, as every surface should read it.
///
/// Derived on every read from the core result cache plus the compute
/// lane's own status. Never stored and never persisted, so it cannot
/// drift from the thing it describes.
#[derive(Debug, Clone, PartialEq)]
pub enum FreshnessState {
    /// The core holds a result generated from the current inputs.
    Current,
    /// An input changed after the last generation. The GUI's own
    /// `ToolpathRuntime::result` still holds the old geometry so the
    /// viewport can draw it; the core holds nothing.
    EditedSince,
    /// The compute lane owns it right now.
    Regenerating,
    /// Blocked on upstream simulated stock (A/M11) — sequencing, not
    /// failure.
    WaitingOnUpstream(AwaitingPriorStock),
    /// Generation failed.
    Error(String),
    /// Never generated, or cancelled / forgotten.
    NoResult,
    /// `enabled == false`. Wins over everything else.
    Disabled,
}

impl FreshnessState {
    /// Stable machine-readable label, for the MCP wire and for tests.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::EditedSince => "edited_since",
            Self::Regenerating => "regenerating",
            Self::WaitingOnUpstream(_) => "awaiting_prior_stock",
            Self::Error(_) => "error",
            Self::NoResult => "no_result",
            Self::Disabled => "disabled",
        }
    }

    /// Whether a consumer may treat this toolpath's geometry as the
    /// answer for the configuration on screen. Only [`Self::Current`] may.
    ///
    /// `EditedSince` is deliberately NOT current: the geometry the GUI
    /// still draws was generated from inputs the project no longer has.
    pub fn is_current(&self) -> bool {
        matches!(self, Self::Current)
    }
}

/// Derive the state for one toolpath.
///
/// `core_has_result` is `session.get_result(index).is_some()` — the core
/// cache is the truth about result validity, and every mutating path in
/// `ProjectSession` drops the entry through `drop_result`.
///
/// The core alone cannot tell "never generated" from "had a result, then
/// edited": both are an absent entry. The GUI supplies that distinction
/// from its retained `ToolpathRuntime::result`, which is a display matter
/// and stays a display matter.
pub fn freshness(
    tc: &ToolpathConfig,
    rt: Option<&ToolpathRuntime>,
    core_has_result: bool,
) -> FreshnessState {
    if !tc.enabled {
        return FreshnessState::Disabled;
    }
    match rt.map(|r| &r.status) {
        Some(ComputeStatus::Computing) => FreshnessState::Regenerating,
        Some(ComputeStatus::AwaitingPriorStock(blocked)) => {
            FreshnessState::WaitingOnUpstream(blocked.clone())
        }
        Some(ComputeStatus::Error(message)) => FreshnessState::Error(message.clone()),
        _ if core_has_result => FreshnessState::Current,
        _ if rt.is_some_and(|r| r.result.is_some()) => FreshnessState::EditedSince,
        _ => FreshnessState::NoResult,
    }
}

/// Derive the state for the toolpath at `index`, or `None` when no
/// toolpath has that index.
pub fn freshness_at(
    session: &ProjectSession,
    gui: &GuiState,
    index: usize,
) -> Option<FreshnessState> {
    let tc = session.get_toolpath_config(index)?;
    let rt = gui.toolpath_rt.get(&tc.id);
    Some(freshness(tc, rt, session.get_result(index).is_some()))
}
