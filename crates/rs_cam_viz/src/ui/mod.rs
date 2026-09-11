#![deny(clippy::indexing_slicing)]

pub mod automation;
pub mod components;
pub mod export_wizard;
pub mod feeds_modal;
pub mod machine_library_modal;
pub mod menu_bar;
pub mod multitool_planner;
pub mod optimize_modal;
pub mod optimize_project;
pub mod overlays;
pub mod preflight;
pub mod properties;
pub mod readiness;
pub mod readiness_panel;
pub mod setup_panel;
pub mod shortcuts_window;
pub mod sim_debug;
pub mod sim_diagnostics;
pub mod sim_op_list;
pub mod sim_timeline;
pub mod status_bar;
pub mod theme;
pub mod tool_library_modal;
pub mod toolpath_panel;
pub mod toolpath_row_controls;
pub mod viewport_overlay;
pub mod workspace_bar;

use crate::state::job::{FixtureId, KeepOutId, ModelId, SetupId, ToolConfig, ToolId, ToolType};
use crate::state::toolpath::{OperationType, ToolpathId};
use crate::ui_command::UiCommand;
use rs_cam_core::enriched_mesh::FaceGroupId;
use std::path::PathBuf;

// NOTE (A-4, Checkpoint I-1, 2026-08-12): the `FeedsField` enum and the
// `AppEvent::ApplyFeedsField` variant that used to live here are GONE, and
// their absence is a safety property, not a tidy-up. They backed six per-row
// `Apply` buttons in the Feeds & Speeds modal that wrote
// `FeedsExplain::recommended` straight into the operation — no validation of
// the tool × operation pairing, and no `enforce_invariants`, so none of the
// plunge/stepover clamps, the rigidity and cutting-length DOC clamps, the
// deflection back-off or the rounding ran. Measured on the shipped default
// Ø6.35 2-flute flat end mill in a Pocket op, the per-field DOC `Apply` wrote
// **4.445 mm** where the funnel writes **1.27 mm** (3.50×).
// Every surviving apply goes through `rs_cam_core::feeds::suggest::apply` with
// an explicit `ApplyScope`. Do not reintroduce a field-grained apply event
// without re-reading `planning/review_2026-08-08/APPLY_CONTRACT_CENSUS.md` §3.4
// — sentried by `apply_contract_a3::per_field_apply_affordance_no_longer_exists`.

/// Events emitted by UI components, processed after the UI pass.
#[derive(Debug)]
pub enum AppEvent {
    // File
    ImportStl(PathBuf),
    ImportSvg(PathBuf),
    ImportDxf(PathBuf),
    ImportStep(PathBuf),
    RescaleModel(ModelId, crate::state::job::ModelUnits),
    RemoveModel(ModelId),
    ReloadModel(ModelId),
    /// Point an existing model at a different file on disk, keeping its id,
    /// name and declared units (G-MODELRELINK, F4.3).
    ///
    /// Not "import as new": every `ToolpathConfig::model_id` that names this
    /// model keeps naming it, which is the whole point — the geometry moved,
    /// the operations did not.
    RelinkModel(ModelId, std::path::PathBuf),
    ExportCombinedGcode,
    ExportSetupGcode(SetupId),
    ExportSetupSheet,
    ExportSvgPreview,
    SaveJob,
    OpenJob,
    /// Toggle generator-trace capture on every toolpath (sets
    /// `debug_options.enabled` across the whole project).
    SetGeneratorTraceCaptureAll(bool),

    // Tools
    AddTool(ToolType),
    /// Add a fully-specified tool copied from a library catalog. The
    /// session reassigns the id on insert.
    AddToolFromLibrary(Box<ToolConfig>),
    DuplicateTool(ToolId),
    RemoveTool(ToolId),

    // Machine Library modal (snapshot model — mirrors the tool library)
    /// Import the named library machine into the project as a snapshot
    /// copy (no live link), making it the inline machine.
    ImportMachineFromLibrary(String),

    // Setups
    AddSetup,
    RenameSetup(SetupId, String),
    /// One-click two-sided setup: create flipped Setup 2, set flip axis, auto-place pins.
    SetupTwoSided,

    // Fixtures and keep-out zones
    AddFixture(SetupId),
    RemoveFixture(SetupId, FixtureId),
    AddKeepOut(SetupId),
    RemoveKeepOut(SetupId, KeepOutId),

    // Toolpaths
    AddToolpath(OperationType),
    DuplicateToolpath(ToolpathId),
    RemoveToolpath(ToolpathId),
    MoveToolpathUp(ToolpathId),
    MoveToolpathDown(ToolpathId),
    /// Reorder a toolpath within its current setup. The `usize` is the
    /// INSERTION GAP the operator dropped on, counted in the setup's own
    /// plan order: `0` is above the first card, `n` below the last.
    ReorderToolpath(ToolpathId, usize),
    /// Move a toolpath from its current setup to a different setup. The
    /// `Option<usize>` is the insertion gap in the TARGET setup, or `None`
    /// to append — MCP `move_toolpath_to_setup` takes no position argument
    /// and passes `None` (G-DROPINDEX).
    MoveToolpathToSetup(ToolpathId, SetupId, Option<usize>),
    ToggleToolpathEnabled(ToolpathId),
    GenerateToolpath(ToolpathId),
    GenerateAll,

    // Simulation
    RunSimulation,
    RunSimulationWith(Vec<ToolpathId>),

    // Pre-flight / Export
    ExportGcodeConfirmed,
    /// Switch the visible wizard step. Bounds-checked in the handler.
    WizardSetStep(u8),
    /// Update the post-processor format from the wizard's Step 1 dropdown.
    WizardSetPost(crate::state::job::PostFormat),
    /// Step 2: pick how the emitted g-code is split across files.
    WizardSetOutputLayout(crate::state::wizard::OutputLayout),
    /// Step 2: update the filename-template field. Substitutions like
    /// `{job}` / `{setup}` / `{toolpath}` are applied at save time
    /// based on the active layout.
    WizardSetFilenameTemplate(String),
    /// Step 3: WCS override. `None` = use the post's default.
    WizardSetWcsOverride(Option<rs_cam_core::gcode::WcsCode>),
    /// Step 3: units override. `None` = inherit from the post.
    WizardSetUnitsOverride(Option<rs_cam_core::gcode::Units>),
    /// Step 3: safe-Z override (mm). `None` = use the project default
    /// (`gui.post.safe_z`).
    WizardSetSafeZOverride(Option<f64>),
    /// Step 3: dry-run mode. When true, the emit pass clamps every
    /// cutting move's Z to the effective safe-Z so the spindle stays
    /// in air for the whole program.
    WizardSetDryRun(bool),
    /// Step 4: spindle warmup dwell in seconds. Zero disables.
    WizardSetSpindleWarmup(u32),
    /// Step 4: tool-change handling override. `None` = use the post's
    /// `tool_change` template; `Pause`/`M6` swap the template;
    /// `Suppress` strips tool-change blocks (keeping per-tool RPM).
    WizardSetToolChangeMode(Option<rs_cam_core::gcode::ToolChangeMode>),
    /// Step 4.5: per-setup pause-message override. `None` falls back to
    /// the default `Setup change: <name>` text emitted before the
    /// inter-setup `M0`. `Some("...")` replaces it verbatim — used to
    /// instruct the operator to run a sender macro (Z probe, corner
    /// probe, home) before pressing Resume.
    WizardSetSetupPauseMessage {
        setup_id: usize,
        message: Option<String>,
    },
    /// Step 5: toggle the "I understand the risks, allow save with
    /// validator errors" override.
    WizardSetAllowValidatorErrors(bool),
    /// Step 6: commit the export to disk using the wizard's settings.
    /// Pops a file/directory picker, writes the file(s), pushes a
    /// notification, and closes the wizard on success.
    WizardSave,

    // Optimize (U2 of OPTIMIZER_UX_PLAN.md)
    /// Open the Optimize modal for a specific toolpath. Triggers
    /// optimize_toolpath synchronously and stashes the outcome on
    /// `AppState::optimize_modal`. Long-running — the GUI freezes
    /// until U3's worker-thread integration lands.
    OpenOptimizeModal(ToolpathId),
    /// Apply a candidate from the Optimize modal. Carries the candidate
    /// index into `OptimizeOutcome::Ranked(..)` so the controller can
    /// look up the params + delta from the cached outcome rather than
    /// shipping the full `OperationConfig` through an event.
    ApplyOptimizeCandidate {
        toolpath_id: ToolpathId,
        /// Index into the cached `Ranked` candidates list. Index 0 is
        /// the baseline (apply does nothing); index ≥ 1 selects a
        /// non-baseline candidate.
        candidate_index: usize,
    },
    /// OPT-005 — accept an operator suggestion from the Optimize modal:
    /// set the named axis to `value` on the toolpath, then re-run the
    /// search against the new baseline (re-opens the modal). An explicit
    /// operator click — never an auto-apply — that removes the manual
    /// re-typing step the prose suggestions used to require.
    ReoptimizeWithAxisOverride {
        toolpath_id: ToolpathId,
        axis: rs_cam_core::tool_load::optimize::KnobAxis,
        value: f64,
    },

    // Feeds & Speeds modal (Feeds-tab redesign)
    /// Open the redesigned Feeds & Speeds modal focused on a specific
    /// toolpath. The modal renders the current/recommended comparison
    /// card plus three machinist charts; it re-derives the underlying
    /// `FeedsExplain` from session state every frame.
    OpenFeedsModal(ToolpathId),
    /// Set the project-level spindle policy (chart-fidelity vs
    /// max-speed). Persists into `ProjectPostConfig.spindle_strategy`.
    /// All Suggest calls and the Feeds modal use the new policy on the
    /// next frame. Re-emit per change since the strategy alters every
    /// toolpath's recommendation simultaneously.
    SetSpindleStrategy(rs_cam_core::feeds::SpindleStrategy),
    /// Apply every recommended value (RPM, feed, plunge, DOC, WOC) to a
    /// toolpath in one transactional update. The Apply-all button on
    /// the comparison card routes here.
    ///
    /// Goes through `feeds::suggest::apply` with `ApplyScope::Both`, so it
    /// refuses outright on a tool × operation pairing
    /// `validate_tool_for_operation` declines, and the values written are the
    /// invariant-resolved ones — identical to the properties panel's two
    /// buttons applied together (Checkpoint I-1).
    ApplyFeedsAll(ToolpathId),
    /// S1 — set (or clear) the scallop-driven-stepover target on a
    /// DropCutter toolpath. `Some(h)` switches WOC to be derived from
    /// the cusp height + tool tip radius; `None` restores the formula
    /// stepover. No-op for operations that don't carry the field.
    SetDropCutterScallopHeight {
        toolpath_id: ToolpathId,
        value: Option<f64>,
    },
    /// Apply the Feeds recommendation across every selected toolpath
    /// in project-rollup mode.
    ApplyFeedsProject,
    /// Apply a custom (feed, rpm) pair from the Chart C drag-to-explore
    /// interaction. Routed only when the user releases the drag inside
    /// the chart bounds and confirms.
    ApplyFeedsExplore {
        toolpath_id: ToolpathId,
        feed_mm_min: f64,
        rpm: f64,
    },
    /// Apply Feeds recommendations to every selected (checked) toolpath.
    ApplyFeedsProjectSelected,

    // Optimize project (U3 of OPTIMIZER_UX_PLAN.md)
    /// Open the project-level Optimize rollup. Submits an
    /// `OptimizeRequest::Project` to the worker lane, which walks
    /// every enabled toolpath. The view opens in `Loading` state
    /// immediately; the rollup populates when the worker returns.
    OpenOptimizeProject,
    /// Apply every row whose checkbox is currently true. Each
    /// applied candidate is the first-safe recommendation from that
    /// row's outcome. Routes through `Command::RestoreToolpathSnapshot`.
    ApplyOptimizeProject,

    // Multi-tool finishing planner (Phase U of the multi-tool plan)
    /// Run `preview_multitool_plan` on the Optimize worker lane. The
    /// session moves into the request and comes back on the result, the
    /// same shape `OpenOptimizeProject` uses.
    PreviewMultitoolPlan,
    /// Emit the previewed ladder through `apply_multitool_plan`. Enabled
    /// only on a Ready preview: applying something the operator has not
    /// been shown is the thing the veto exists to prevent.
    ApplyMultitoolPlan,

    // Collision
    RunCollisionCheck,

    // Face selection
    ToggleFaceSelection {
        toolpath_id: ToolpathId,
        model_id: ModelId,
        face_id: FaceGroupId,
    },

    // Drill target selection (DXF point / circle centre picked in viewport)
    ToggleDrillTarget {
        toolpath_id: ToolpathId,
        xy: [f64; 2],
    },

    // Edit
    //
    // WP6 deleted five post-write notification events — `StockChanged`,
    // `StockMaterialChanged`, `HeightPlanesChanged`, `MachineChanged` and
    // `FixtureChanged` (plan section 14 ruling 3). A panel now applies
    // one `Command`, which carries the invalidation the event used to
    // ask for, and raises `AppState::panel_side_effects` for the view
    // work that is not a core rule.
    Undo,
    Redo,

    // View
    /// One row of the view registry
    /// ([`for_each_ui_command!`](crate::for_each_ui_command)).
    ///
    /// WP13 moved 55 variants here. A view command writes the VIEW —
    /// the camera, a modal, the playback index, a per-user library file
    /// — and never `ProjectSession`. It keeps travelling on this
    /// channel because a view write and a session write arrive from the
    /// same draw pass and must stay in order.
    Ui(UiCommand),
}
