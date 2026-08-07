//! GUI-only runtime overlay state.
//!
//! These types hold presentation and interaction state that is NOT part of
//! the persisted project or the core compute model.  They sit alongside
//! `ProjectSession` in `AppState`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::feeds::FeedsResult;
use rs_cam_core::session::ToolpathConfig;

use super::job::{PostConfig, PostFormat};

// Re-export ComputeStatus from core (canonical definition).
pub use rs_cam_core::compute::config::ComputeStatus;

// Re-export ToolpathResult from the existing toolpath module.
pub use super::toolpath::ToolpathResult;

// ── Per-toolpath runtime state ────────────────────────────────────────

/// GUI-only state for a single toolpath — display, compute status, cached results.
pub struct ToolpathRuntime {
    pub visible: bool,
    pub locked: bool,
    pub auto_regen: bool,
    pub status: ComputeStatus,
    pub result: Option<ToolpathResult>,
    pub stale_since: Option<std::time::Instant>,
    pub feeds_result: Option<FeedsResult>,
    pub debug_trace: Option<Arc<rs_cam_core::debug_trace::ToolpathDebugTrace>>,
    pub semantic_trace: Option<Arc<rs_cam_core::semantic_trace::ToolpathSemanticTrace>>,
    pub debug_trace_path: Option<PathBuf>,
}

impl ToolpathRuntime {
    /// Create runtime state for a new toolpath with sensible defaults.
    pub fn new(auto_regen: bool) -> Self {
        Self {
            visible: true,
            locked: false,
            auto_regen,
            status: ComputeStatus::Pending,
            result: None,
            stale_since: None,
            feeds_result: None,
            debug_trace: None,
            semantic_trace: None,
            debug_trace_path: None,
        }
    }

    /// Clear all computed/cached state (e.g. after param change or on load).
    pub fn clear_runtime(&mut self) {
        self.status = ComputeStatus::Pending;
        self.result = None;
        self.stale_since = None;
        self.feeds_result = None;
        self.debug_trace = None;
        self.semantic_trace = None;
        self.debug_trace_path = None;
    }
}

// ── Per-setup state (W9 / P-2) ────────────────────────────────────────
//
// `SetupRuntime { datum, model_ids }` used to live here as GUI-only
// overlay state, alongside a byte-identical private copy of the datum
// enums. Both fields are operator intent — how the machine is zeroed and
// which models the setup is allowed to use — and neither had a home on
// the wire, so every save dropped them. They now live on core's
// `SetupData` and are persisted; the Setup properties panel already had
// `&mut SetupData` in hand, so it writes straight through to the session
// and there is no overlay left to keep in sync.

// ── Combined view for UI code ─────────────────────────────────────────

/// A read-only combined view of a toolpath's config (from session) and
/// runtime state (from GUI overlay).  Used by UI drawing code.
pub struct ToolpathView<'a> {
    pub config: &'a ToolpathConfig,
    pub runtime: &'a ToolpathRuntime,
    /// Index in the session's `toolpath_configs` vec.
    pub index: usize,
}

// ── Project-level GUI state ───────────────────────────────────────────

/// User-controlled overrides for the tool-load export gate. Each flag bypasses
/// a *distinct* class of refusal — they are deliberately not collapsed into a
/// single "I accept the risk" toggle. Default is the strict policy: refuse on
/// either Exceeds or Unmodeled.
#[derive(Default, Debug, Clone, Copy)]
pub struct ToolLoadOverrides {
    /// Bypass `Unmodeled` refusals (criterion couldn't be evaluated honestly).
    pub accept_unmodeled: bool,
    /// Bypass `Exceeds` refusals (criterion was modeled and predicted to break
    /// the tool, exceed power, or have unsafe stickout).
    pub accept_exceeded: bool,
}

impl ToolLoadOverrides {
    /// Translate to the core policy struct consumed by `export_gcode_checked`.
    pub fn as_policy(&self) -> rs_cam_core::gcode::ToolLoadExportPolicy {
        rs_cam_core::gcode::ToolLoadExportPolicy {
            accept_unmodeled: self.accept_unmodeled,
            accept_exceeded: self.accept_exceeded,
        }
    }
}

/// GUI-only project-level state.
pub struct GuiState {
    pub file_path: Option<PathBuf>,
    pub dirty: bool,
    pub edit_counter: u64,
    /// Viz-friendly post config view (mirrors session post config with enum format).
    pub post: PostConfig,
    /// Per-toolpath GUI runtime state, keyed by toolpath semantic ID.
    pub toolpath_rt: HashMap<rs_cam_core::ToolpathId, ToolpathRuntime>,
    /// User-toggled overrides for the tool-load export gate. Reset on project load.
    pub tool_load_overrides: ToolLoadOverrides,
    /// Recently changed parameters from MCP, with timestamp for fade-out.
    /// Key: "toolpath_{id}_{param}" or "tool_{id}_{param}" or "stock_{param}"
    #[cfg(feature = "mcp")]
    pub mcp_highlights: HashMap<String, std::time::Instant>,
    /// One-shot toolpath-properties tab override, set by the MCP
    /// `set_ui_view` tool: `(target toolpath, tab key)`. Consumed the
    /// next time the properties panel renders *that* toolpath — scoping
    /// to the target makes the override survive whatever frame the
    /// workspace-switch / selection events land on (pre-fix an
    /// intervening render of the previously selected toolpath consumed
    /// it and persisted the tab onto the wrong toolpath). Canonical tab
    /// values: "geometry", "feeds", "linking", "heights", "dressup".
    /// Not cfg-gated on `mcp` so the properties panel can consume it
    /// unconditionally.
    pub pending_toolpath_tab: Option<(crate::state::toolpath::ToolpathId, String)>,
}

impl GuiState {
    pub fn new() -> Self {
        Self {
            file_path: None,
            dirty: false,
            edit_counter: 0,
            post: PostConfig::default(),
            toolpath_rt: HashMap::new(),
            tool_load_overrides: ToolLoadOverrides::default(),
            #[cfg(feature = "mcp")]
            mcp_highlights: HashMap::new(),
            pending_toolpath_tab: None,
        }
    }

    /// Build a `PostConfig` (viz enum format) from the session's string-based config.
    pub fn post_from_session(session_post: &rs_cam_core::session::ProjectPostConfig) -> PostConfig {
        PostConfig {
            // W9 / P-1: the open-coded match this replaces had no
            // `"grblhal"` arm, so reloading a grblHAL project silently
            // reset the Post panel's dropdown to GRBL. `from_token` is
            // the tested resolver; an unknown token still falls back to
            // GRBL rather than failing the load.
            format: PostFormat::from_token(&session_post.format).unwrap_or(PostFormat::Grbl),
            spindle_speed: session_post.spindle_speed,
            safe_z: session_post.safe_z,
            high_feedrate_mode: session_post.high_feedrate_mode,
            high_feedrate: session_post.high_feedrate,
            spindle_strategy: session_post.spindle_strategy,
        }
    }

    /// Sync session post config from the viz-friendly PostConfig.
    pub fn post_to_session(post: &PostConfig) -> rs_cam_core::session::ProjectPostConfig {
        rs_cam_core::session::ProjectPostConfig {
            // Same tokens as before, now from the single writer half of
            // the resolver pair (`PostFormat::to_token`) so the spelling
            // cannot drift from what `from_token` accepts.
            format: post.format.to_token().to_owned(),
            spindle_speed: post.spindle_speed,
            safe_z: post.safe_z,
            high_feedrate_mode: post.high_feedrate_mode,
            high_feedrate: post.high_feedrate,
            spindle_strategy: post.spindle_strategy,
        }
    }

    /// Mark the project as having unsaved changes.
    pub fn mark_edited(&mut self) {
        self.dirty = true;
        self.edit_counter += 1;
    }

    /// Get or create a toolpath runtime entry.
    pub fn toolpath_rt_or_default(&mut self, id: rs_cam_core::ToolpathId) -> &mut ToolpathRuntime {
        self.toolpath_rt
            .entry(id)
            .or_insert_with(|| ToolpathRuntime::new(true))
    }
}

impl Default for GuiState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// W9 / P-1. `post_from_session` used to open-code a token match
    /// with no `"grblhal"` arm, so every project reload silently reset
    /// the Post panel's dropdown from grblHAL to GRBL. Driven off
    /// `PostFormat::ALL` so a fifth dialect cannot be added with a
    /// writer arm and no reader arm.
    #[test]
    fn every_post_format_survives_the_session_round_trip() {
        for &format in PostFormat::ALL {
            let post = PostConfig {
                format,
                ..PostConfig::default()
            };
            let session_post = GuiState::post_to_session(&post);
            assert_eq!(
                GuiState::post_from_session(&session_post).format,
                format,
                "{format:?} did not survive post_to_session -> post_from_session \
                 (token was {:?})",
                session_post.format
            );
        }
    }

    /// The serialized spelling is part of the file format — pin it so
    /// the P-1 fix cannot be "rename the token".
    #[test]
    fn the_written_post_tokens_are_the_shipped_spellings() {
        let tokens: Vec<&str> = PostFormat::ALL.iter().map(|f| f.to_token()).collect();
        assert_eq!(tokens, vec!["grbl", "grblhal", "linuxcnc", "mach3"]);
    }

    /// An unrecognised token still loads as GRBL rather than failing
    /// the project load.
    #[test]
    fn an_unknown_post_token_reads_back_as_grbl() {
        let session_post = rs_cam_core::session::ProjectPostConfig {
            format: "cobalt-cnc".to_owned(),
            ..Default::default()
        };
        assert_eq!(
            GuiState::post_from_session(&session_post).format,
            PostFormat::Grbl
        );
    }
}
