//! GUI-only runtime overlay state.
//!
//! These types hold presentation and interaction state that is NOT part of
//! the persisted project or the core compute model.  They sit alongside
//! `ProjectSession` in `AppState`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::feeds::FeedsResult;
use rs_cam_core::maps::reach_map::ReachMap;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};

use super::job::{PostConfig, PostFormat};
use super::toolpath::ToolpathId;

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

// ── Per-tool reach map overlay (P5) ───────────────────────────────────

/// Where the selected toolpath's reach map is in its life cycle.
///
/// `Idle` means **no map is wanted** — the selection is not a reach-capable
/// operation, or it carries no mesh or no tool. It never means "measured
/// zero"; a measured map that found no surface says so through
/// [`ReachMap::is_measured`], and the panel prints `reach: not measured`
/// rather than a clean percentage.
pub enum ReachStatus {
    Idle,
    Computing,
    Ready(Arc<ReachMap>),
    Failed(String),
}

/// The reach map the viewport is drawing, plus what it was resolved for.
///
/// The map is built on a worker (`ComputeLane::Reach`) because a cold walk is
/// one full-grid drop-cutter pass — seconds, not milliseconds. Nothing here
/// borrows the session: `rs_cam_core::maps::reach_map::ReachMapRequest` carries its
/// own mesh, index and cutter, which is what lets the walk leave the UI
/// thread without lending the session out.
pub struct ReachOverlayState {
    /// The toolpath this answer belongs to. A result for any other toolpath
    /// is a stale supersede and is dropped, never shown as a failure.
    pub toolpath: Option<ToolpathId>,
    pub status: ReachStatus,
    /// Bumped on every map that is not the map already held, and read by
    /// `ReachOverlayUploadKey` — so a result identical to the last one
    /// rebuilds no GPU buffer.
    ///
    /// Compared against [`Self::last_map`], NOT against [`Self::status`]: the
    /// scheduler sets `status = Computing` on submit, so by the time a result
    /// lands `status` never holds a map to compare with, and every arrival
    /// would look new.
    pub generation: u64,
    /// The last map this overlay accepted, kept only as the identity
    /// [`Self::generation`] is compared against.
    ///
    /// It deliberately survives [`Self::clear`]: the reach memo hands back the
    /// same `Arc` for a warm key, so a deselect-and-reselect must not bump the
    /// generation and rebuild a 661 k-triangle buffer. The upload key's
    /// `None` → `Some` transition already forces the rebuild that a genuine
    /// deselect needs, so the generation only has to separate consecutive
    /// live keys.
    pub last_map: Option<Arc<ReachMap>>,
    /// The session edit counter the request was resolved at. Part of the
    /// scheduling key.
    ///
    /// **Known behaviour: the overlay blinks off on an unrelated edit.**
    /// `GuiState::mark_edited` bumps this counter for any project edit —
    /// renaming another toolpath, a post-config change, each keystroke in a
    /// name field — so the sweep resubmits and the overlay hides behind
    /// `reach: computing…` until the walk answers (a memo hit, plus one
    /// `vertex_gaps` pass, off the frame loop). Comparing the resolved
    /// requests instead would remove the blink, and it cannot be done
    /// honestly from this crate: the memo's own tool-geometry key
    /// (`rs_cam_core::maps::tool_shape_key::ToolShapeKey`) is `pub(crate)` to core,
    /// and the fields that ARE reachable miss the common case — editing the
    /// selected tool's diameter leaves `tool_id` and, inside the cell clamp,
    /// `ReachMapParams` unchanged. A blink is a cosmetic cost; showing the
    /// previous tool's reach map is a wrong answer.
    pub edit_counter: u64,
    /// When the in-flight request was submitted, or `None` when none is.
    ///
    /// Read only by the scheduler's stuck-`Computing` recovery, which is why
    /// it is a timestamp rather than a flag: a backend that runs no Reach lane
    /// (every scripted test double) reports an idle lane the instant after a
    /// submit, so an unbounded recovery would resubmit on every pump for ever.
    pub requested_at: Option<std::time::Instant>,
    /// One colour per mesh vertex, computed on the worker beside the map.
    ///
    /// **On the worker, not in the upload pass**: `vertex_gaps` runs one
    /// spatial-index query per vertex, which on a board-sized terrain is a
    /// visible hitch if it lands on the frame loop. The upload pass only
    /// checks the length against the mesh it is about to draw.
    pub colors: Option<Arc<Vec<[f32; 3]>>>,
}

impl ReachOverlayState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            toolpath: None,
            status: ReachStatus::Idle,
            generation: 0,
            last_map: None,
            edit_counter: 0,
            requested_at: None,
            colors: None,
        }
    }

    /// The map, when one is held for `toolpath`.
    #[must_use]
    pub fn ready_map(&self) -> Option<&Arc<ReachMap>> {
        match &self.status {
            ReachStatus::Ready(map) => Some(map),
            _ => None,
        }
    }

    /// Forget the answer and the key it was asked under. The next scheduler
    /// sweep decides what to ask for.
    ///
    /// [`Self::generation`] and [`Self::last_map`] deliberately survive — see
    /// their docs for why a deselect must not make the next identical map
    /// look new.
    pub fn clear(&mut self) {
        self.toolpath = None;
        self.status = ReachStatus::Idle;
        self.edit_counter = 0;
        self.requested_at = None;
        self.colors = None;
    }
}

impl Default for ReachOverlayState {
    fn default() -> Self {
        Self::new()
    }
}

/// One colour per mesh vertex for the reach overlay.
///
/// The single construction site for the overlay's colours, so the worker, the
/// upload pass and the sentry test cannot drift apart. `NaN` gaps — an
/// underside, an overhang, anything off the measured population — come back
/// as the model shader's own neutral diffuse colour, so a not-measured vertex
/// looks exactly like the plain model rather than like a reachable one.
#[must_use]
pub fn reach_overlay_colors(
    map: &ReachMap,
    mesh: &TriangleMesh,
    index: &SpatialIndex,
) -> Vec<[f32; 3]> {
    map.vertex_colors(mesh, index)
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

/// Whether THIS export may emit an operation's PREVIOUS geometry — the
/// result of the generation before the operator edited the operation.
///
/// Deliberately not a `bool`. An operator who overrides this gate is
/// choosing to cut geometry that does not match the parameters on
/// screen, and a bare `true` at a call site says none of that.
///
/// Off by default, and held here beside [`ToolLoadOverrides`] rather than
/// in the project file: it is a decision about one export, not a property
/// of the job, so it resets whenever a project is loaded (both loaders
/// build a fresh `GuiState`). R0.1 §7 Q5 asked the operator to choose
/// between this and a persisted project setting and has not been
/// answered; this follows R0.1's own recommendation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StaleResultPolicy {
    /// The default. An operation edited since its last generation blocks
    /// the export, and the refusal names it.
    #[default]
    Refuse,
    /// Emit the previous generation's geometry for an edited operation.
    /// The file will not match the parameters on screen.
    AcceptPreviousGeometry,
}

impl StaleResultPolicy {
    /// Whether an edited operation may be exported under this policy.
    pub fn accepts_previous_geometry(self) -> bool {
        matches!(self, Self::AcceptPreviousGeometry)
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
    /// G-STALEXPORT: whether the next export may emit an edited
    /// operation's previous geometry. Reset on project load, like
    /// [`Self::tool_load_overrides`]; never written to the project file.
    pub stale_export: StaleResultPolicy,
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
    /// Per-tool reach map for the selected toolpath (P5). Scheduled by
    /// `AppController::process_reach_overlay`, filled by the Reach compute
    /// lane, drawn by `ViewportCallback::show_reach_overlay`.
    pub reach_overlay: ReachOverlayState,
    /// Resumable export-wizard settings.
    ///
    /// WP6b, plan section 19 ruling 5. The record lived on
    /// `ProjectSession` behind a `wizard_mut` hatch. Nothing saved it and
    /// the project loader reset it on every load, so it is GUI state and
    /// not project data. It sits here rather than on `AppState` because
    /// `GuiState` is the argument every export door already takes, so
    /// `io::export::overlay_for` reads it without a new parameter.
    pub wizard: crate::state::wizard::WizardState,
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
            stale_export: StaleResultPolicy::default(),
            #[cfg(feature = "mcp")]
            mcp_highlights: HashMap::new(),
            pending_toolpath_tab: None,
            reach_overlay: ReachOverlayState::new(),
            wizard: crate::state::wizard::WizardState::default(),
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
