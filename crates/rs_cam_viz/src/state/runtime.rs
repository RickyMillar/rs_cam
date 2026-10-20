//! GUI-only runtime overlay state.
//!
//! These types hold presentation and interaction state that is NOT part of
//! the persisted project or the core compute model.  They sit alongside
//! `ProjectSession` in `AppState`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::feeds::{FeedsError, FeedsResult};
use rs_cam_core::maps::reach_map::ReachMap;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};

use crate::render::upload_cache::{self, Refresh, SlotInputs};

use super::job::PostConfig;
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
    /// The key and the by-products of `feeds_result` (F216). Only
    /// [`FeedsMemo::refresh`] writes the two.
    pub feeds_memo: FeedsMemo,
    pub debug_trace: Option<Arc<rs_cam_core::trace::debug_trace::ToolpathDebugTrace>>,
    pub semantic_trace: Option<Arc<rs_cam_core::trace::semantic_trace::ToolpathSemanticTrace>>,
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
            feeds_memo: FeedsMemo::default(),
            debug_trace: None,
            semantic_trace: None,
            debug_trace_path: None,
        }
    }
}

// ── The toolpath panel's feeds memo (F216) ────────────────────────────

/// What the toolpath panel's feeds work reads, read ONCE per frame (F216).
///
/// The Geometry tab (the ⚡ pills) and the Feeds tab (the card) both hand
/// this value to [`FeedsMemo::refresh`]. The key derives from it
/// ([`SlotInputs::key`] destructures it with no `..`), and the build takes
/// only it. So a new input is a compile error in `key` until it is keyed:
/// the W11 shape of `render::upload_cache`.
pub struct FeedsInputs<'a> {
    pub operation: &'a rs_cam_core::compute::catalog::OperationConfig,
    pub tool: &'a rs_cam_core::compute::tool_config::ToolConfig,
    pub material: &'a rs_cam_core::material::Material,
    pub machine: &'a rs_cam_core::machine::MachineProfile,
    pub spindle_strategy: rs_cam_core::feeds::SpindleStrategy,
    /// The box the funnel's runtime-sanity stepover back-off reads (Q1).
    pub model_bbox: Option<&'a rs_cam_core::geo::BoundingBox3>,
}

/// The stored key of [`FeedsMemo`]. Built only by [`SlotInputs::key`].
///
/// `OperationConfig` and `MachineProfile` have no `PartialEq`, so the key
/// holds their `Debug` text (the `Obstacle::effect_digest` idiom). A derived
/// `Debug` prints every field, so the text can only over-key, never miss an
/// edit.
#[derive(Debug, Clone, PartialEq)]
pub struct FeedsKey {
    operation: String,
    tool: rs_cam_core::compute::tool_config::ToolConfig,
    material: rs_cam_core::material::Material,
    machine: String,
    spindle_strategy: rs_cam_core::feeds::SpindleStrategy,
    /// `BoundingBox3` has no `PartialEq`: the two corners.
    model_bbox: Option<[f64; 6]>,
}

impl SlotInputs for FeedsInputs<'_> {
    type Key = FeedsKey;

    fn key(&self) -> FeedsKey {
        let Self {
            operation,
            tool,
            material,
            machine,
            spindle_strategy,
            model_bbox,
        } = self;
        // The vendor LUT is not an input: `embedded_vendor_lut` is one
        // static value for the life of the process.
        FeedsKey {
            operation: format!("{operation:?}"),
            tool: (*tool).clone(),
            material: (*material).clone(),
            machine: format!("{machine:?}"),
            spindle_strategy: *spindle_strategy,
            model_bbox: model_bbox.map(|b| [b.min.x, b.min.y, b.min.z, b.max.x, b.max.y, b.max.z]),
        }
    }
}

/// The funnel dry run the ⚡ pills offer (G-PILLCLAMP). The UI owns it
/// (`ui::properties::pills::pill_previews`); the memo runs it after the
/// calculator. The signature fixes what the dry run may read.
pub type PillDryRun = fn(
    &rs_cam_core::compute::catalog::OperationConfig,
    &FeedsResult,
    &rs_cam_core::compute::tool_config::ToolConfig,
    &rs_cam_core::machine::MachineProfile,
    &rs_cam_core::material::Material,
    Option<&rs_cam_core::geo::BoundingBox3>,
) -> rs_cam_core::feeds::suggest::FieldApplyPreviews;

impl FeedsInputs<'_> {
    /// The calculator and the funnel dry run. This is the work the memo
    /// saves.
    fn build(
        &self,
        dry_run: PillDryRun,
    ) -> Result<(FeedsResult, rs_cam_core::feeds::suggest::FieldApplyPreviews), FeedsError> {
        let Self {
            operation,
            tool,
            material,
            machine,
            spindle_strategy,
            model_bbox,
        } = self;
        let result = rs_cam_core::feeds::suggest::feeds_result_for_operation(
            operation,
            tool,
            material,
            machine,
            rs_cam_core::feeds::embedded_vendor_lut(),
            *spindle_strategy,
        )?;
        let previews = dry_run(operation, &result, tool, machine, material, *model_bbox);
        Ok((result, previews))
    }
}

/// The memo of the toolpath panel's feeds work (F216).
///
/// The panel ran the calculator and the funnel dry run on every frame.
/// The memo runs them once per change of [`FeedsInputs`]. It sits beside
/// `feeds_result` on `ToolpathRuntime` and `ToolpathEntry`; the panel copies
/// both in and back each frame.
#[derive(Debug, Clone, Default)]
pub struct FeedsMemo {
    key: Option<FeedsKey>,
    /// The funnel dry run for the stored result. `None` on a refusal.
    previews: Option<rs_cam_core::feeds::suggest::FieldApplyPreviews>,
    /// The calculator's refusal. The Feeds tab prints it.
    refusal: Option<FeedsError>,
    /// The count of builds: the instrument of the F216 sentry.
    builds: u64,
}

impl FeedsMemo {
    /// The one writer of `result` and of this memo.
    ///
    /// When the key of `inputs` matches the stored key, nothing runs.
    /// Otherwise the calculator and `dry_run` run once, and `result`
    /// holds the answer (`None` on a refusal).
    pub fn refresh(
        &mut self,
        result: &mut Option<FeedsResult>,
        inputs: FeedsInputs<'_>,
        dry_run: PillDryRun,
    ) {
        let Refresh::Build(Some(inputs)) = upload_cache::refresh(&mut self.key, Some(inputs))
        else {
            return;
        };
        self.builds += 1;
        match inputs.build(dry_run) {
            Ok((answer, previews)) => {
                *result = Some(answer);
                self.previews = Some(previews);
                self.refusal = None;
            }
            Err(error) => {
                *result = None;
                self.previews = None;
                self.refusal = Some(error);
            }
        }
    }

    /// The funnel dry run for the stored result.
    pub fn previews(&self) -> Option<&rs_cam_core::feeds::suggest::FieldApplyPreviews> {
        self.previews.as_ref()
    }

    /// Why the calculator refused, when it did.
    pub fn refusal(&self) -> Option<&FeedsError> {
        self.refusal.as_ref()
    }

    /// How many times the calculator ran for this toolpath.
    pub fn builds(&self) -> u64 {
        self.builds
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
    /// The memo key of the request this answer was asked under, or `None`
    /// when the selected toolpath resolves no request (an operation that a
    /// reach map does not speak about, no mesh, or no tool). Part of the
    /// scheduling key, beside [`Self::toolpath`].
    ///
    /// The key is built from the inputs the map reads: the mesh identity
    /// (the setup transform gives a new mesh `Arc`), the tool shape, the
    /// tolerance from the operation config, the cell and the ids. It is
    /// `rs_cam_core::maps::reach_map_cache::ReachRequestKey`, the core memo's
    /// own key, so the overlay resubmits exactly when the memo would answer
    /// a new question.
    ///
    /// It is NOT `GuiState::edit_counter`. That counter misses each MCP core
    /// command and each controller command that does not call
    /// `mark_edited`, so an MCP `set_tool_param` left the previous tool's
    /// map on screen. It also moved on each unrelated edit, and the overlay
    /// then blinked to `reach: computing…` for no new question.
    pub key: Option<rs_cam_core::maps::reach_map_cache::ReachRequestKey>,
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
            key: None,
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
        self.key = None;
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
    /// `set_ui_view` tool: `(target toolpath, tab)`. Consumed the
    /// next time the properties panel renders *that* toolpath — scoping
    /// to the target makes the override survive whatever frame the
    /// workspace-switch / selection events land on (pre-fix an
    /// intervening render of the previously selected toolpath consumed
    /// it and persisted the tab onto the wrong toolpath).
    ///
    /// UI-06: the tab is the [`crate::ui::properties::ToolpathTab`] enum,
    /// not its agent-facing key. The MCP boundary parses the key once;
    /// a GUI-internal writer names the variant it means.
    /// Not cfg-gated on `mcp` so the properties panel can consume it
    /// unconditionally.
    pub pending_toolpath_tab: Option<(
        crate::state::toolpath::ToolpathId,
        crate::ui::properties::ToolpathTab,
    )>,
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
    /// H2: the memo of the core shoe plan. The toolpath panel and the run
    /// sheet read the plan on every frame; the memo builds it once per
    /// change of an input.
    pub shoe_plan: crate::state::shoe_plan::ShoePlanMemo,
    /// P4.3: the pick mode and the `+ Rule` draft of the selection
    /// section. View state only: a project load resets it with the rest of
    /// this struct, and no file holds it.
    pub selection_ui: crate::state::pick::SelectionUi,
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
            shoe_plan: crate::state::shoe_plan::ShoePlanMemo::default(),
            selection_ui: crate::state::pick::SelectionUi::default(),
        }
    }

    // UI-07: `post_from_session` and `post_to_session` lived here. They
    // copied six fields each way between `gcode::PostConfig` and
    // `session::ProjectPostConfig`, and a field added to one side reached
    // three of the four halves. The session holds `gcode::PostConfig` now,
    // so `gui.post` is a CLONE of it and a new field needs no edit here.

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

    use rs_cam_core::gcode::PostFormat;

    /// W9 / P-1. The Post panel's dropdown used to reset from grblHAL to
    /// GRBL on every reload, because one of two hand-written translations
    /// had no `"grblhal"` arm.
    ///
    /// UI-07 deleted both translations: the session holds
    /// `gcode::PostConfig` itself. The round trip that remains is the ONE
    /// that can still lose a dialect — serialise the config and read it
    /// back, which is what saving and loading a project does.
    #[test]
    fn every_post_format_survives_the_session_round_trip() {
        for &format in PostFormat::ALL {
            let post = PostConfig {
                format,
                ..PostConfig::default()
            };
            let json = serde_json::to_string(&post).expect("the post config serialises");
            let back: PostConfig = serde_json::from_str(&json).expect("and reads back");
            assert_eq!(
                back.format, format,
                "{format:?} did not survive the project-file round trip (wrote {json})"
            );
        }
    }

    /// The serialized spelling is part of the file format — pin it so the
    /// P-1 fix cannot be "rename the token".
    ///
    /// UI-07 made serde WRITE these same tokens, so the two halves the
    /// pin covers are now one. A project written before UI-07 reads back
    /// unchanged.
    #[test]
    fn the_written_post_tokens_are_the_shipped_spellings() {
        let tokens: Vec<&str> = PostFormat::ALL.iter().map(|f| f.to_token()).collect();
        assert_eq!(tokens, vec!["grbl", "grblhal", "linuxcnc", "mach3"]);

        for (&format, token) in PostFormat::ALL.iter().zip(&tokens) {
            let written = serde_json::to_string(&format).expect("the format serialises");
            assert_eq!(
                written,
                format!("\"{token}\""),
                "serde must write the token the project file already holds"
            );
        }
    }

    /// W4 (F090, no legacy): the underscore spellings are gone; each one
    /// fails to read back instead of loading as its post.
    #[test]
    fn the_underscore_post_spellings_are_refused() {
        for spelling in ["\"grbl_hal\"", "\"linux_cnc\""] {
            assert!(
                serde_json::from_str::<PostFormat>(spelling).is_err(),
                "{spelling} must not load"
            );
        }
    }

    /// A project file with no post block at all loads as the default
    /// rather than failing.
    #[test]
    fn an_absent_post_block_reads_back_as_the_default() {
        let back: PostConfig = serde_json::from_str("{}").expect("an empty post block loads");
        assert_eq!(back, PostConfig::default());
    }
}
