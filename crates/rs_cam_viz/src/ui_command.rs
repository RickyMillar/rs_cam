//! The view registry: every command and read that never enters the core
//! session.
//!
//! This is the second of two registries. `for_each_command!`
//! (`crates/rs_cam_core/src/session/command.rs`) declares what the
//! session runs. `for_each_ui_command!` here declares what the VIEW
//! runs: a camera move, a modal, a playback jump, a screenshot, a read
//! of the view's own simulation slot, a read of the per-user library
//! files.
//!
//! The two registries share one vocabulary. This file reuses core's
//! [`CommandKind`], [`Reach`] and [`Surfaces`] rather than declaring its
//! own, so one sentry compares both. It cannot be one registry: core
//! writes `$payload` into `pub enum Command`, and `rs_cam_viz` depends on
//! `rs_cam_core` and never the reverse, so core cannot name a view
//! payload type.
//!
//! The columns are the same six: the row kind (`UiCommand` or
//! `UiQuery`), the identifier, the wire name, the payload type, the
//! answer type, and the surface table. A `UiCommand` row answers `()` —
//! its effect IS the view state it writes. A `UiQuery` row answers
//! `String`, the JSON reply the MCP arm builds today.
//!
//! Two doors run these rows.
//!
//! - A `UiCommand` travels as `AppEvent::Ui(UiCommand)` and reaches the
//!   existing event dispatch.
//! - A `UiQuery` runs through `RsCamApp::ui_query`, which takes `&self`.
//!   A read that cannot hold `&mut` cannot write the session. The door
//!   hangs on the application and NOT on `AppState`, which ruling 4
//!   named: `AppState` holds neither the toast stack nor the
//!   controller's triage builder, so two of the eight reads could not be
//!   answered there.
//!
//! WP13 ruling 5: the three library listings and the ten file-store rows
//! read and write a PER-USER FILE aggregate, not the project and not the
//! view. They are `UiQuery` / `UiCommand` for now, and no `Store` kind is
//! opened. If a later package moves the tool library into core, those 13
//! rows move with it.
//!
//! WP23: a row that declares `gui: Reach::Reached` has a constructor in a
//! production view file. A dispatch arm is not a caller, so a row whose
//! last control went away is DELETED and not flipped to `Skip`. The
//! census in `crates/rs_cam_viz/tests/command_surface_completeness.rs`
//! enforces this.
//!
//! What this file does NOT declare: a row that mutates `ProjectSession`.
//! `crates/rs_cam_viz/tests/command_surface_completeness.rs` measures
//! that, with two named exemptions.

use crate::render::camera::ViewPreset;
use crate::state::job::{FaceUp, ToolConfig};
use crate::state::runtime::StaleResultPolicy;
use crate::state::selection::Selection;
use crate::state::toolpath::ToolpathId;
use crate::state::{NomogramExplore, ProjectFeedsSort, Workspace};
use rs_cam_core::session::CommandId;

pub use rs_cam_core::session::{CommandKind, Reach, Surfaces};

// ── Payloads ─────────────────────────────────────────────────────────
//
// The muncher takes `$payload:ident`, so every payload is one named
// type. A row with no arguments names [`NoArgs`]; a row carrying one
// value names a type alias; a row carrying several names a struct.

/// The payload of a row that carries no arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NoArgs;

/// Which tree node the operator selected.
pub type SelectArgs = Selection;

/// Which stored camera angle to take.
pub type SetViewPresetArgs = ViewPreset;

/// Which face the operator asked to look at.
pub type PreviewOrientationArgs = FaceUp;

/// Which workspace to show.
pub type SwitchWorkspaceArgs = Workspace;

/// Which toolpath to show or hide.
pub type ToggleToolpathVisibilityArgs = ToolpathId;

/// Which toolpath to open in the simulation workspace.
pub type InspectToolpathInSimulationArgs = ToolpathId;

/// Which move to move playback to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimJumpToMoveArgs {
    /// The move index, counted over the whole simulated program.
    pub move_index: usize,
}

/// Which operation boundary to move playback to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimJumpToOpStartArgs {
    /// The index into the simulation's operation boundary list.
    pub boundary_index: usize,
}

/// Where inside one toolpath to move playback to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimScrubToolpathArgs {
    /// The toolpath index.
    pub index: usize,
    /// The position inside that toolpath, from 0 to 100.
    pub percent: f64,
}

/// Which toolpath to move playback to the start of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimJumpToToolpathStartArgs {
    /// The toolpath index.
    pub index: usize,
}

/// Which toolpath to move playback to the end of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimJumpToToolpathEndArgs {
    /// The toolpath index.
    pub index: usize,
}

/// What to capture from the simulation viewport, and where to write it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenshotSimulationArgs {
    /// The PNG file to write.
    pub path: String,
    /// The image width in pixels.
    pub width: Option<u32>,
    /// The image height in pixels.
    pub height: Option<u32>,
    /// Which stored checkpoint to show.
    pub checkpoint: Option<usize>,
    /// Whether to draw the toolpaths over the stock.
    pub include_toolpaths: Option<bool>,
}

/// What to capture from the toolpath viewport, and where to write it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenshotToolpathArgs {
    /// The toolpath index.
    pub index: usize,
    /// The PNG file to write.
    pub path: String,
    /// The image width in pixels.
    pub width: Option<u32>,
    /// The image height in pixels.
    pub height: Option<u32>,
    /// Whether to draw the stock.
    pub show_stock: Option<bool>,
    /// Whether to draw the rapid moves.
    pub include_rapids: Option<bool>,
    /// Whether to shade the model surface by the per-tool reach map.
    pub reach_overlay: Option<bool>,
}

/// Where to write the whole-window capture, and how big to make it.
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenshotGuiArgs {
    /// The PNG file to write.
    pub path: String,
    /// The window width in logical points, applied before the capture.
    pub width: Option<f32>,
    /// The window height in logical points, applied before the capture.
    pub height: Option<f32>,
}

/// Which view state to drive the application to.
///
/// The fields are applied in declaration order. `overlays` is applied
/// after `workspace`, because a workspace carries overlay defaults that
/// would otherwise land on top of these writes (P6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetUiViewArgs {
    /// The workspace to show.
    pub workspace: Option<String>,
    /// The toolpath to select.
    pub toolpath_index: Option<usize>,
    /// The properties tab to show.
    pub properties_tab: Option<String>,
    /// The tree node to select.
    pub select: Option<String>,
    /// The modal to open.
    pub modal: Option<String>,
    /// The viewport overlays to switch, by registry id.
    pub overlays: Option<std::collections::BTreeMap<String, bool>>,
}

/// Whether the project feeds rollup is on screen.
pub type SetProjectFeedsOpenArgs = bool;

/// Which sort order the project rollup table takes.
pub type SetFeedsProjectSortArgs = ProjectFeedsSort;

/// Where to put the drag-to-explore overlay point, or `None` to clear it.
pub type SetFeedsExploreArgs = Option<NomogramExplore>;

/// Which project rollup row to select or deselect.
pub type ToggleFeedsProjectRowArgs = ToolpathId;

/// Whether the project rollup draws its scatter overlay.
pub type SetFeedsProjectScatterArgs = bool;

/// Whether every project rollup row is selected.
pub type SetFeedsProjectSelectAllArgs = bool;

/// Which export-gate verdicts the operator accepts.
///
/// The two flags are independent. `accept_unmodeled` bypasses only
/// `Unmodeled` verdicts; `accept_exceeded` bypasses only `Exceeds`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetToolLoadOverrideArgs {
    /// Whether an unmodelled tool load may still export.
    pub accept_unmodeled: bool,
    /// Whether an exceeded tool load may still export.
    pub accept_exceeded: bool,
}

/// Which project rollup row of the optimizer to select or deselect.
pub type ToggleOptimizeProjectRowArgs = usize;

/// Whether export emits the previous geometry of an edited operation.
pub type SetStaleExportPolicyArgs = StaleResultPolicy;

/// Which library tool to delete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteLibraryToolArgs {
    /// The catalog file name.
    pub catalog: String,
    /// The tool position inside that catalog.
    pub index: usize,
}

/// Which library tool to replace, and what to replace it with.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateLibraryToolArgs {
    /// The catalog file name.
    pub catalog: String,
    /// The tool position inside that catalog.
    pub index: usize,
    /// The edited copy.
    pub tool: Box<ToolConfig>,
}

/// Which library tool to move, and where to move it to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveLibraryToolArgs {
    /// The catalog the tool leaves.
    pub from: String,
    /// The tool position inside that catalog.
    pub index: usize,
    /// The catalog the tool joins.
    pub to: String,
}

/// The name of the catalog to create.
pub type CreateToolCatalogArgs = String;

/// The name of the catalog to delete.
pub type DeleteToolCatalogArgs = String;

/// Which catalog to rename, and what to call it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameToolCatalogArgs {
    /// The current name.
    pub old: String,
    /// The new name.
    pub new: String,
}

/// The name of the catalog to de-duplicate.
pub type DedupeToolCatalogArgs = String;

/// The library name to save the project machine under.
pub type SaveMachineToLibraryArgs = String;

/// The library machine to delete.
pub type DeleteMachineFromLibraryArgs = String;

/// Which library machine to rename, and what to call it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameMachineInLibraryArgs {
    /// The current name.
    pub old: String,
    /// The new name.
    pub new: String,
}

/// Which part of the cut trace to report, and how much of it.
#[derive(Debug, Clone, PartialEq)]
pub struct GetCutTraceArgs {
    /// The toolpath id the trace is stored under. An unmatched id is
    /// refused. This is an id, never an index.
    pub toolpath_id: Option<usize>,
    /// The cap on reported hotspots.
    pub max_hotspots: Option<usize>,
    /// The cap on reported issues.
    pub max_issues: Option<usize>,
    /// A `SpanKind` filter, in the snake_case spelling.
    pub span_kind: Option<String>,
    /// An exact `SpanId` match.
    pub span_id: Option<u32>,
    /// A `DepthPass` `pass_index` match.
    pub pass_index: Option<u32>,
    /// Whether to include the per-peck drill samples.
    pub include_drill_samples: bool,
    /// The per-array caps of Checkpoint L.
    ///
    /// `rs_cam_mcp` is an optional dependency, and `get_cut_trace` is a
    /// wire-only row: its `gui` column says `Skip`, and both the door
    /// that builds this payload and the door that reads it sit behind
    /// `#[cfg(feature = "mcp")]`. The field carries the same gate, so
    /// the view registry compiles without the `mcp` feature.
    #[cfg(feature = "mcp")]
    pub caps: rs_cam_mcp::response::CutTraceCaps,
}

/// Which toasts to report, and how many.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GetNotificationsArgs {
    /// Whether to include toasts whose time on screen has run out.
    pub include_expired: bool,
    /// The cap on reported toasts.
    pub limit: Option<usize>,
}

/// Which tool catalog to list.
pub type ListToolCatalogArgs = String;

// ── The registry ─────────────────────────────────────────────────────

/// Declares every view command and view read row once.
///
/// The columns are: the row kind (`UiCommand` or `UiQuery`), the
/// identifier, the wire name, the payload type, the answer type, and the
/// surface table. A `UiCommand` row answers `()`; a `UiQuery` row answers
/// the type its door reports. Edit this list, not the blocks a callback
/// macro generates from it.
///
/// The macro carries `#[macro_export]` for the reason core's carries it:
/// a sentry in `tests/` counts the rows with its own callback.
///
/// Three rows carry BOTH a GUI control and an MCP tool —
/// `SimJumpToMove`, `SimJumpToStart` and `SimJumpToEnd`. They are one row
/// each, not two: a row is an identity, and each surface keeps its own
/// handler.
#[macro_export]
macro_rules! for_each_ui_command {
    ($m:ident) => {
        $m! {
            //  kind       id                 wire name            payload
            //  answer     surfaces

            // ── Selection, view and workspace ───────────────────────
            (UiCommand, ExportGcode, "open_export_preflight", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "the export_gcode tool exports; it opens no pre-flight panel",
                 ),
                 cli: Reach::Skip("the batch CLI draws no pre-flight panel"),
             }),
            (UiCommand, Select, "select_in_tree", SelectArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("set_ui_view carries the selection instead"),
                 cli: Reach::Skip("the batch CLI draws no tree"),
             }),
            (UiCommand, SetViewPreset, "set_view_preset", SetViewPresetArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool moves the camera to a preset"),
                 cli: Reach::Skip("the batch CLI draws no viewport"),
             }),
            (UiCommand, ToggleProjection, "toggle_projection", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool switches the camera projection"),
                 cli: Reach::Skip("the batch CLI draws no viewport"),
             }),
            (UiCommand, PreviewOrientation, "preview_orientation", PreviewOrientationArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool previews a setup orientation"),
                 cli: Reach::Skip("the batch CLI draws no viewport"),
             }),
            (UiCommand, ResetView, "reset_view", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool refits the camera"),
                 cli: Reach::Skip("the batch CLI draws no viewport"),
             }),
            (UiCommand, SwitchWorkspace, "switch_workspace", SwitchWorkspaceArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("set_ui_view carries the workspace instead"),
                 cli: Reach::Skip("the batch CLI draws no workspace"),
             }),
            (UiCommand, ToggleToolpathVisibility, "toggle_toolpath_visibility",
             ToggleToolpathVisibilityArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool hides one toolpath in the viewport"),
                 cli: Reach::Skip("the batch CLI draws no viewport"),
             }),
            (UiCommand, ToggleShowAllToolpaths, "toggle_show_all_toolpaths", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "set_ui_view's overlays map carries the all_toolpaths row instead",
                 ),
                 cli: Reach::Skip("the batch CLI draws no viewport"),
             }),
            (UiCommand, InspectToolpathInSimulation, "inspect_toolpath_in_simulation",
             InspectToolpathInSimulationArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "sim_jump_to_toolpath_start carries the same intent on the wire",
                 ),
                 cli: Reach::Skip("the batch CLI draws no simulation workspace"),
             }),
            (UiCommand, ShowShortcuts, "show_shortcuts", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool opens the shortcut window"),
                 cli: Reach::Skip("the batch CLI draws no shortcut window"),
             }),
            (UiCommand, Quit, "quit", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "no wire tool closes the window the MCP server is embedded in",
                 ),
                 cli: Reach::Skip("the batch CLI runs to completion and exits"),
             }),

            // ── Simulation playback ─────────────────────────────────
            (UiCommand, ResetSimulation, "reset_simulation", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("run_simulation replaces a run; no wire tool clears one"),
                 cli: Reach::Skip("the batch CLI holds no playback state"),
             }),
            (UiCommand, ToggleSimPlayback, "toggle_sim_playback", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool starts or stops playback"),
                 cli: Reach::Skip("the batch CLI holds no playback state"),
             }),
            (UiCommand, SimStepForward, "sim_step_forward", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("sim_jump_to_move names the move instead"),
                 cli: Reach::Skip("the batch CLI holds no playback state"),
             }),
            (UiCommand, SimStepBackward, "sim_step_backward", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("sim_jump_to_move names the move instead"),
                 cli: Reach::Skip("the batch CLI holds no playback state"),
             }),
            (UiCommand, SimJumpToStart, "sim_jump_to_start", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Skip("the batch CLI holds no playback state"),
             }),
            (UiCommand, SimJumpToEnd, "sim_jump_to_end", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Skip("the batch CLI holds no playback state"),
             }),
            (UiCommand, SimJumpToMove, "sim_jump_to_move", SimJumpToMoveArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Skip("the batch CLI holds no playback state"),
             }),
            (UiCommand, SimJumpToOpStart, "sim_jump_to_op_start", SimJumpToOpStartArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "sim_jump_to_toolpath_start names the toolpath, not the boundary",
                 ),
                 cli: Reach::Skip("the batch CLI holds no playback state"),
             }),
            (UiCommand, SimScrubToolpath, "sim_scrub_toolpath", SimScrubToolpathArgs, (),
             Surfaces {
                 gui: Reach::Skip(
                     "the timeline drags through sim_jump_to_move, which names the move",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip("the batch CLI holds no playback state"),
             }),
            (UiCommand, SimJumpToToolpathStart, "sim_jump_to_toolpath_start",
             SimJumpToToolpathStartArgs, (),
             Surfaces {
                 gui: Reach::Skip(
                     "the operation list jumps through sim_jump_to_op_start",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip("the batch CLI holds no playback state"),
             }),
            (UiCommand, SimJumpToToolpathEnd, "sim_jump_to_toolpath_end",
             SimJumpToToolpathEndArgs, (),
             Surfaces {
                 gui: Reach::Skip("no GUI control jumps to an operation's last move"),
                 mcp: Reach::Reached,
                 cli: Reach::Skip("the batch CLI holds no playback state"),
             }),

            // ── Screenshots ─────────────────────────────────────────
            (UiCommand, ScreenshotSimulation, "screenshot_simulation",
             ScreenshotSimulationArgs, (),
             Surfaces {
                 gui: Reach::Skip(
                     "the operator reads the viewport; no GUI control writes a PNG",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip("the batch CLI draws no viewport"),
             }),
            (UiCommand, ScreenshotToolpath, "screenshot_toolpath", ScreenshotToolpathArgs, (),
             Surfaces {
                 gui: Reach::Skip(
                     "the operator reads the viewport; no GUI control writes a PNG",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip("the batch CLI draws no viewport"),
             }),
            (UiCommand, ScreenshotGui, "screenshot_gui", ScreenshotGuiArgs, (),
             Surfaces {
                 gui: Reach::Skip(
                     "the operator reads the window; no GUI control captures it",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip("the batch CLI draws no window"),
             }),
            (UiCommand, SetUiView, "set_ui_view", SetUiViewArgs, (),
             Surfaces {
                 gui: Reach::Skip(
                     "a GUI control writes the one field it owns, never a view bundle",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip("the batch CLI draws no view"),
             }),

            // ── Modal open and close ────────────────────────────────
            (UiCommand, OpenToolLibrary, "open_tool_library", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("list_tool_library reports the same catalogs"),
                 cli: Reach::Skip("the batch CLI draws no modal"),
             }),
            (UiCommand, CloseToolLibrary, "close_tool_library", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool closes a modal it cannot open"),
                 cli: Reach::Skip("the batch CLI draws no modal"),
             }),
            (UiCommand, OpenMachineLibrary, "open_machine_library", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("list_machine_library reports the same machines"),
                 cli: Reach::Skip("the batch CLI draws no modal"),
             }),
            (UiCommand, CloseMachineLibrary, "close_machine_library", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool closes a modal it cannot open"),
                 cli: Reach::Skip("the batch CLI draws no modal"),
             }),
            (UiCommand, OpenExportWizard, "open_export_wizard", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("export_gcode exports without the wizard"),
                 cli: Reach::Skip("the batch CLI draws no wizard"),
             }),
            (UiCommand, CloseExportWizard, "close_export_wizard", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool closes a wizard it cannot open"),
                 cli: Reach::Skip("the batch CLI draws no wizard"),
             }),
            (UiCommand, OpenMultitoolPlanner, "open_multitool_planner", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("plan_multitool_finishing emits the ladder directly"),
                 cli: Reach::Skip("the batch CLI draws no planner"),
             }),
            (UiCommand, CloseMultitoolPlanner, "close_multitool_planner", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool closes a planner it cannot open"),
                 cli: Reach::Skip("the batch CLI draws no planner"),
             }),

            // ── Feeds modal view state ──────────────────────────────
            (UiCommand, CloseFeedsModal, "close_feeds_modal", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("apply_feeds writes the recipe without a modal"),
                 cli: Reach::Skip("the batch CLI draws no modal"),
             }),
            // DC5a deleted `SetFeedsModalMode`. The feeds modal held TWO
            // scopes behind a mode flip; the project rollup moved to the
            // Readiness workspace, so there is no mode left to switch. The
            // row below opens that rollup and is its replacement.
            (UiCommand, SetProjectFeedsOpen, "set_project_feeds_open",
             SetProjectFeedsOpenArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("apply_feeds writes the recipe without a rollup"),
                 cli: Reach::Skip("the batch CLI draws no rollup"),
             }),
            (UiCommand, SetFeedsProjectSort, "set_feeds_project_sort",
             SetFeedsProjectSortArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool sorts a table it cannot see"),
                 cli: Reach::Skip("the batch CLI draws no table"),
             }),
            (UiCommand, SetFeedsExplore, "set_feeds_explore", SetFeedsExploreArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool drags a chart overlay"),
                 cli: Reach::Skip("the batch CLI draws no chart"),
             }),
            (UiCommand, ToggleFeedsProjectRow, "toggle_feeds_project_row",
             ToggleFeedsProjectRowArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("apply_feeds names the toolpath it writes"),
                 cli: Reach::Skip("the batch CLI draws no table"),
             }),
            (UiCommand, SetFeedsProjectScatter, "set_feeds_project_scatter",
             SetFeedsProjectScatterArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool switches a chart overlay"),
                 cli: Reach::Skip("the batch CLI draws no chart"),
             }),
            (UiCommand, SetFeedsProjectSelectAll, "set_feeds_project_select_all",
             SetFeedsProjectSelectAllArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("apply_feeds names the toolpath it writes"),
                 cli: Reach::Skip("the batch CLI draws no table"),
             }),
            (UiCommand, SetToolLoadOverride, "set_tool_load_override",
             SetToolLoadOverrideArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("export_gcode carries both accept flags per call"),
                 cli: Reach::Skip("the batch CLI carries its own export flags"),
             }),

            // ── Optimizer modal view state ──────────────────────────
            (UiCommand, CancelOptimizeRun, "cancel_optimize_run", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "an MCP caller cancels its own job and draws no progress row",
                 ),
                 cli: Reach::Skip("the batch CLI draws no progress row"),
             }),
            (UiCommand, CloseOptimizeModal, "close_optimize_modal", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("optimize_toolpath answers without a modal"),
                 cli: Reach::Skip("the batch CLI draws no modal"),
             }),
            (UiCommand, CloseOptimizeProject, "close_optimize_project", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool opens the project rollup"),
                 cli: Reach::Skip("the batch CLI draws no rollup"),
             }),
            (UiCommand, ToggleOptimizeProjectRow, "toggle_optimize_project_row",
             ToggleOptimizeProjectRowArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool selects a rollup row"),
                 cli: Reach::Skip("the batch CLI draws no rollup"),
             }),

            // ── Export policy ───────────────────────────────────────
            (UiCommand, SetStaleExportPolicy, "set_stale_export_policy",
             SetStaleExportPolicyArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "export_gcode carries accept_previous_geometry per call",
                 ),
                 cli: Reach::Skip("the batch CLI carries its own export flags"),
             }),

            // ── Tool and machine library files (ruling 5) ───────────
            (UiCommand, DeleteLibraryTool, "delete_library_tool", DeleteLibraryToolArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool writes the per-user tool library"),
                 cli: Reach::Skip("the batch CLI reads the library and never writes it"),
             }),
            (UiCommand, UpdateLibraryTool, "update_library_tool", UpdateLibraryToolArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool writes the per-user tool library"),
                 cli: Reach::Skip("the batch CLI reads the library and never writes it"),
             }),
            (UiCommand, MoveLibraryTool, "move_library_tool", MoveLibraryToolArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool writes the per-user tool library"),
                 cli: Reach::Skip("the batch CLI reads the library and never writes it"),
             }),
            (UiCommand, CreateToolCatalog, "create_tool_catalog", CreateToolCatalogArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool writes the per-user tool library"),
                 cli: Reach::Skip("the batch CLI reads the library and never writes it"),
             }),
            (UiCommand, DeleteToolCatalog, "delete_tool_catalog", DeleteToolCatalogArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool writes the per-user tool library"),
                 cli: Reach::Skip("the batch CLI reads the library and never writes it"),
             }),
            (UiCommand, RenameToolCatalog, "rename_tool_catalog", RenameToolCatalogArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool writes the per-user tool library"),
                 cli: Reach::Skip("the batch CLI reads the library and never writes it"),
             }),
            (UiCommand, DedupeToolCatalog, "dedupe_tool_catalog", DedupeToolCatalogArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool writes the per-user tool library"),
                 cli: Reach::Skip("the batch CLI reads the library and never writes it"),
             }),
            (UiCommand, SaveMachineToLibrary, "save_machine_to_library",
             SaveMachineToLibraryArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool writes the per-user machine library"),
                 cli: Reach::Skip("the batch CLI reads the library and never writes it"),
             }),
            (UiCommand, DeleteMachineFromLibrary, "delete_machine_from_library",
             DeleteMachineFromLibraryArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool writes the per-user machine library"),
                 cli: Reach::Skip("the batch CLI reads the library and never writes it"),
             }),
            (UiCommand, RenameMachineInLibrary, "rename_machine_in_library",
             RenameMachineInLibraryArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no wire tool writes the per-user machine library"),
                 cli: Reach::Skip("the batch CLI reads the library and never writes it"),
             }),

            // ── Compute lane control ────────────────────────────────
            (UiCommand, CancelCompute, "cancel_compute", NoArgs, (),
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "cancel_generation cancels the toolpath lane alone, off the frame loop",
                 ),
                 cli: Reach::Skip("the batch CLI computes on its own thread"),
             }),

            // ── View reads ──────────────────────────────────────────
            (UiQuery, GetDiagnostics, "get_diagnostics", NoArgs, String,
             Surfaces {
                 gui: Reach::Skip(
                     "the diagnostics panel reads the simulation slot without this door",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip("the CLI project report reads the core session"),
             }),
            (UiQuery, GetToolLoadReport, "get_tool_load_report", NoArgs, String,
             Surfaces {
                 gui: Reach::Skip(
                     "the readiness panel reads the simulation slot without this door",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip("the CLI project report reads the core session"),
             }),
            (UiQuery, GetCutTrace, "get_cut_trace", GetCutTraceArgs, String,
             Surfaces {
                 gui: Reach::Skip(
                     "the simulation panels read the trace in the slot without this door",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip("the CLI holds no view simulation slot"),
             }),
            (UiQuery, InspectCollisions, "inspect_collisions", NoArgs, String,
             Surfaces {
                 gui: Reach::Skip(
                     "the diagnostics panel reads the simulation slot without this door",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip("the CLI holds no view simulation slot"),
             }),
            (UiQuery, GetNotifications, "get_notifications", GetNotificationsArgs, String,
             Surfaces {
                 gui: Reach::Skip("the toast stack draws itself; no panel reads it"),
                 mcp: Reach::Reached,
                 cli: Reach::Skip("the batch CLI pushes no toasts"),
             }),
            (UiQuery, ListToolLibrary, "list_tool_library", NoArgs, String,
             Surfaces {
                 gui: Reach::Skip(
                     "the Tool Library modal loads its own snapshot of the catalog files",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip("the batch CLI lists no library"),
             }),
            (UiQuery, ListToolCatalog, "list_tool_catalog", ListToolCatalogArgs, String,
             Surfaces {
                 gui: Reach::Skip(
                     "the Tool Library modal loads its own snapshot of the catalog files",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip("the batch CLI lists no library"),
             }),
            (UiQuery, ListMachineLibrary, "list_machine_library", NoArgs, String,
             Surfaces {
                 gui: Reach::Skip(
                     "the Machine Library modal loads its own snapshot of the library files",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip("the batch CLI lists no library"),
             }),
        }
    };
}

/// Splits the view registry's rows by kind, and emits the [`UiCommand`]
/// and [`UiQuery`] payload enums plus [`UiQueryAnswer`].
///
/// GENERATED support macro for `define_ui_command_registry!`, called with
/// the same rows that macro receives. It walks the row list one row at a
/// time — an incremental "tt-muncher" — sorting each row's columns into
/// the `ui_commands` or `ui_queries` accumulator by matching the literal
/// identifier `UiCommand` or `UiQuery` in the row's first column. The
/// base case (no rows left) is tried first on every recursive call, so it
/// fires the moment both accumulators hold every row and none remains.
///
/// This is core's `split_command_rows!` with two accumulators rather than
/// three. The shape is deliberately the same, so a reader who knows one
/// knows the other.
macro_rules! split_ui_rows {
    // Base case: no rows left. Emit the two payload enums and their
    // `id()`, from the two accumulators built by the arms below.
    (@split
        ui_commands = [
            $( ($c_id:ident, $c_wire:literal, $c_payload:ident) ),* $(,)?
        ],
        ui_queries = [
            $( ($q_id:ident, $q_wire:literal, $q_payload:ident, $q_answer:ty) ),* $(,)?
        ] $(,)?
    ) => {
        /// One view command and its arguments.
        ///
        /// GENERATED from the `for_each_ui_command!` list — edit the
        /// list, not this block.
        ///
        /// A view command writes the VIEW. It never enters
        /// `ProjectSession`, and
        /// `crates/rs_cam_viz/tests/command_surface_completeness.rs`
        /// measures that over the handler arms.
        ///
        /// The enum derives `Debug` alone, as `AppEvent` does. Nothing
        /// copies a view command: it is dispatched once and dropped.
        #[derive(Debug)]
        pub enum UiCommand {
            $(
                #[doc = concat!("The `", $c_wire, "` view command.")]
                $c_id($c_payload),
            )*
        }

        impl UiCommand {
            /// The identifier of this view command. GENERATED.
            pub fn id(&self) -> UiCommandId {
                match self {
                    $(UiCommand::$c_id(_) => UiCommandId::$c_id,)*
                }
            }
        }

        /// One view read and its arguments.
        ///
        /// GENERATED from the `for_each_ui_command!` list — edit the
        /// list, not this block.
        #[derive(Debug)]
        pub enum UiQuery {
            $(
                #[doc = concat!("The `", $q_wire, "` view read.")]
                $q_id($q_payload),
            )*
        }

        impl UiQuery {
            /// The identifier of this view read. GENERATED.
            pub fn id(&self) -> UiCommandId {
                match self {
                    $(UiQuery::$q_id(_) => UiCommandId::$q_id,)*
                }
            }
        }

        /// The answer to one [`UiQuery`].
        ///
        /// GENERATED — one variant per `UiQuery` row, named by that
        /// row's answer column. Every row answers a JSON document as a
        /// `String`, which is what the MCP arm reports today.
        #[derive(Debug, Clone)]
        pub enum UiQueryAnswer {
            $(
                #[doc = concat!("The answer to the `", $q_wire, "` read.")]
                $q_id($q_answer),
            )*
        }

        impl UiQueryAnswer {
            /// The JSON document this answer carries. GENERATED.
            ///
            /// Every `UiQuery` row answers one JSON document, so the MCP
            /// dispatch needs one arm and not eight. The variant is what
            /// records WHICH read answered.
            pub fn into_json(self) -> String {
                match self {
                    $(UiQueryAnswer::$q_id(json) => json,)*
                }
            }
        }
    };

    // Next row is a `UiCommand` row: file it and recurse.
    (@split
        ui_commands = [$($ui_commands:tt)*],
        ui_queries = [$($ui_queries:tt)*],
        (UiCommand, $id:ident, $wire:literal, $payload:ident, $answer:ty, $surfaces:expr),
        $($rest:tt)*
    ) => {
        split_ui_rows! {
            @split
            ui_commands = [$($ui_commands)* ($id, $wire, $payload),],
            ui_queries = [$($ui_queries)*],
            $($rest)*
        }
    };

    // Next row is a `UiQuery` row: file it and recurse.
    (@split
        ui_commands = [$($ui_commands:tt)*],
        ui_queries = [$($ui_queries:tt)*],
        (UiQuery, $id:ident, $wire:literal, $payload:ident, $answer:ty, $surfaces:expr),
        $($rest:tt)*
    ) => {
        split_ui_rows! {
            @split
            ui_commands = [$($ui_commands)*],
            ui_queries = [$($ui_queries)* ($id, $wire, $payload, $answer),],
            $($rest)*
        }
    };
}

macro_rules! define_ui_command_registry {
    (
        $( ($kind:ident, $id:ident, $wire:literal, $payload:ident, $answer:ty, $surfaces:expr) ),+
        $(,)?
    ) => {
        /// The identifier of one view command or view read, without its
        /// arguments.
        ///
        /// The registry columns hang here. A payload enum cannot host a
        /// `const ALL`. GENERATED from the `for_each_ui_command!` list,
        /// over every row regardless of kind.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum UiCommandId {
            $(
                #[doc = concat!("The identifier of the `", $wire, "` row.")]
                $id,
            )+
        }

        impl UiCommandId {
            /// Every view row, in registry order. GENERATED.
            pub const ALL: &[UiCommandId] = &[$(UiCommandId::$id,)+];

            /// The name this row carries on the wire. GENERATED.
            pub fn wire_name(self) -> &'static str {
                match self {
                    $(UiCommandId::$id => $wire,)+
                }
            }

            /// How the view runs this row. GENERATED.
            pub fn kind(self) -> CommandKind {
                match self {
                    $(UiCommandId::$id => CommandKind::$kind,)+
                }
            }

            /// Which surfaces reach this row, and why one does not.
            /// GENERATED.
            pub fn surfaces(self) -> Surfaces {
                match self {
                    $(UiCommandId::$id => $surfaces,)+
                }
            }
        }

        // The payload and answer enums need one row list per kind, so the
        // kind-agnostic block above hands the same rows to the splitter.
        split_ui_rows! {
            @split
            ui_commands = [],
            ui_queries = [],
            $( ($kind, $id, $wire, $payload, $answer, $surfaces), )+
        }
    };
}
for_each_ui_command!(define_ui_command_registry);

/// One row of EITHER registry.
///
/// The two registries answer for two different things, and nothing else
/// holds them in one list. A sentry that asks "does any wire name appear
/// twice across the whole command surface?" needs this union: without it
/// a view row could claim a core row's name, and the MCP router would
/// then register two tools under one name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceId {
    /// A core row: a `Command`, a `Query` or a `Job`.
    Core(CommandId),
    /// A view row: a `UiCommand` or a `UiQuery`.
    Ui(UiCommandId),
}

impl SurfaceId {
    /// Every row of both registries, core first, then view.
    ///
    /// This is a function and not a `const ALL`, which is the one place
    /// this file departs from core's shape. A const concatenation of two
    /// `&[T]` needs a loop with indexing inside a const function, and
    /// `indexing_slicing` is denied. An iterator chain says the same
    /// thing and needs no exemption.
    pub fn all() -> Vec<SurfaceId> {
        CommandId::ALL
            .iter()
            .copied()
            .map(SurfaceId::Core)
            .chain(UiCommandId::ALL.iter().copied().map(SurfaceId::Ui))
            .collect()
    }

    /// The name this row carries on the wire.
    pub fn wire_name(self) -> &'static str {
        match self {
            SurfaceId::Core(id) => id.wire_name(),
            SurfaceId::Ui(id) => id.wire_name(),
        }
    }

    /// How its own door runs this row.
    pub fn kind(self) -> CommandKind {
        match self {
            SurfaceId::Core(id) => id.kind(),
            SurfaceId::Ui(id) => id.kind(),
        }
    }

    /// Which surfaces reach this row, and why one does not.
    pub fn surfaces(self) -> Surfaces {
        match self {
            SurfaceId::Core(id) => id.surfaces(),
            SurfaceId::Ui(id) => id.surfaces(),
        }
    }
}
