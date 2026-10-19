#![deny(clippy::indexing_slicing)]

pub mod automation;
pub mod components;
pub mod export_wizard;
pub mod feeds;
pub mod generation_resolution_modal;
pub mod machine_library_modal;
pub mod menu_bar;
pub mod multitool_planner;
pub mod overlays;
pub mod pin_wizard;
pub mod preferences;
pub mod preflight;
pub mod properties;
pub mod readiness;
pub mod readiness_panel;
pub mod setup_deletion_modal;
pub mod setup_panel;
pub mod shoe_list;
pub mod shortcuts_window;
pub mod sim_debug;
pub mod sim_diagnostics;
pub mod sim_op_list;
pub mod sim_timeline;
pub mod sim_trace_modal;
pub mod status_bar;
pub mod theme;
pub mod tokens;
pub mod tool_library_modal;
pub mod toolpath_panel;
pub mod toolpath_row_controls;
pub mod viewport_overlay;
pub mod workspace_bar;

use crate::state::job::{ModelId, ObstacleId, SetupId, ToolConfig, ToolId, ToolType};
use crate::state::toolpath::{OperationType, ToolpathId};
use crate::ui_command::UiCommand;
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

/// File-dialog extensions accepted for model import.
///
/// The list lives in `rs_cam_core::io`, beside the table
/// [`model_import_event`] classifies against. This module used to keep a
/// hand-written copy, and a format added to one and not to the other let
/// the operator pick a file the classifier then refused.
pub(crate) use rs_cam_core::io::MODEL_FILE_EXTENSIONS;

/// Classify a selected model path into the import event used by every UI
/// import route.
pub(crate) fn model_import_event(path: PathBuf) -> Option<AppEvent> {
    use rs_cam_core::compute::stock_config::ModelKind;
    // C10: one extension table, in `rs_cam_core::io`. This arm used to
    // carry its own copy, and so did three other sites.
    match rs_cam_core::io::infer_kind_from_path(&path)? {
        ModelKind::Stl => Some(AppEvent::ImportStl(path)),
        ModelKind::Svg => Some(AppEvent::ImportSvg(path)),
        ModelKind::Dxf => Some(AppEvent::ImportDxf(path)),
        ModelKind::Step => Some(AppEvent::ImportStep(path)),
    }
}

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
    /// File ▸ Preferences: open the settings window. The controller reads
    /// the settings file and the backend budget once, on open.
    OpenPreferences,
    /// File ▸ Preferences ▸ Apply: write the draft to the settings file and
    /// apply the live values (the memory budget, the undo depth, the
    /// library folders and the rest). The controller reads the draft from
    /// `AppState::preferences` (`ui::preferences::PreferencesState::chosen_settings`).
    ApplyPreferences,
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
    /// Open the shared setup-deletion confirmation.
    RequestRemoveSetup(SetupId),
    /// Close the shared setup-deletion confirmation without changing the project.
    CancelRemoveSetup,
    RemoveSetup(SetupId),
    RenameSetup(SetupId, String),
    /// One-click two-sided setup: create flipped Setup 2, set flip axis, auto-place pins.
    SetupTwoSided,
    /// Add a setup with this face at 0 deg for the pins ("Add a Top setup"
    /// and "Add a Bottom setup" of the pin wizard). The selection stays,
    /// and the pin door runs its synchronisation.
    PinAddSetup(crate::state::job::FaceUp),
    /// The pin wizard's part-model choice (`None`: the default rule).
    /// Through the pin door, at once.
    PinPartModels(Option<Vec<usize>>),
    /// R4: the pin wizard's Apply. The pin door adds the drills of
    /// `add_drills`, applies `pins` as one `SetAlignmentPins` (one undo
    /// step), binds the tools of `use_tools` and synchronises the pin-drill
    /// operations.
    PinWizardApply {
        pins: Vec<crate::state::job::AlignmentPin>,
        add_drills: Vec<f64>,
        use_tools: Vec<(f64, usize)>,
    },

    // Obstacles (K1: fixtures and keep-out zones are one concept)
    AddObstacle(SetupId),
    RemoveObstacle(SetupId, ObstacleId),
    /// K2: the list-row checkbox. The controller applies the STORED record
    /// with the new flag, so an open editor draft cannot ride along.
    SetObstacleEnabled(SetupId, ObstacleId, bool),
    /// H2: the toolpath panel's dust shoe row. The controller applies one
    /// `SetToolpathShoe` and pushes the undo step. `merge`: the edit is a
    /// later frame of one gap drag, so it joins the newest undo step.
    SetToolpathShoe {
        tp_id: ToolpathId,
        shoe: rs_cam_core::machine::head::ShoeSetting,
        merge: bool,
    },

    // Stock changes (S4): one intent per action of the setup panel's list.
    // The controller turns each into one S2 command.
    EditStockChanges(
        SetupId,
        crate::ui::properties::stock_changes::StockChangeIntent,
    ),

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
    /// Operator ruling 2026-10-15: generate the fresh twin of a rest
    /// toolpath on request and record both cycle times
    /// (`ProjectSession::compare_rest_with_fresh`).
    CompareRestWithFresh(ToolpathId),
    GenerateAll,
    /// Write the ONE stored simulation resolution (G-RESTRES) through
    /// `Command::SetSimulationResolution`. The Simulation panel sends it once
    /// per edit, not once per drag frame.
    SetSimulationResolution(rs_cam_core::session::SimulationResolution),
    /// Make one setup current, and nothing else (operator, 2026-09-19).
    ///
    /// The DEFAULT route resolves whatever a scope depends on: Generate All
    /// and a single Generate both walk the edges. This is the narrow one,
    /// for an operator who knows the setups above are current and does not
    /// want them re-walked.
    GenerateSetupOnly(SetupId),
    /// Stop the generation plan in flight (W3). The Generate All button IS
    /// the cancel while a plan runs: a disabled control with no route would
    /// leave a long plan unstoppable from the panel.
    CancelGeneration,

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
    /// Step 4 (grblHAL): one controller option of the project's post
    /// block. `None` returns it to "from the machine profile" (G11).
    WizardSetControllerOption(crate::state::wizard::ControllerOption, Option<bool>),
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
    /// Apply the recommended SPEEDS only (RPM, feed, plunge) to a toolpath,
    /// holding DOC and WOC exactly as authored. The `Match vendor chipload`
    /// button on the comparison card routes here.
    ///
    /// Goes through the same `feeds::suggest::apply` funnel as
    /// [`AppEvent::ApplyFeedsAll`], with `ApplyScope::Speeds` instead of
    /// `ApplyScope::Both`. It is NOT a per-field apply. Since G-SPEEDSAPPLY
    /// the funnel keeps the operation's own depth per pass and stepover and
    /// resolves the speeds for them through
    /// `enforce_invariants_at_fixed_geometry` on a scratch clone. The
    /// passes that move geometry do not run. A refused tool × operation
    /// pairing still cannot be written.
    ///
    /// Phase C of `planning/load_model_2026-09-16/SPEC.md`: the chipload
    /// verdict row tells the operator their chipload is thin, and this is the
    /// answer to it that does not also rewrite the cut geometry.
    ApplyFeedsSpeeds(ToolpathId),
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

    // Multi-tool finishing planner (Phase U of the multi-tool plan)
    /// Submit the `preview_tier_map` `Job` row on the Job lane (WP14b).
    /// The handle carries the ladder and the geometry, so the session
    /// stays on `AppState` and every panel stays readable.
    PreviewMultitoolPlan,
    /// Emit the previewed ladder through `apply_multitool_plan`. Enabled
    /// only on a Ready preview: applying something the operator has not
    /// been shown is the thing the veto exists to prevent.
    ApplyMultitoolPlan,

    // Collision
    RunCollisionCheck,

    // Geometry selection P2.6: one viewport click on an item (a hole
    // marker or a flat face) of the selected toolpath. `key` is the match
    // key a pick of the item stores. The controller resolves first
    // (`ProjectSession::pick_edit`), then sends `AddPick` or `RemovePick`.
    TogglePick {
        toolpath_id: ToolpathId,
        key: rs_cam_core::selection::MatchKey,
    },

    // One edit of the op selection from a panel (the face Clear button).
    EditToolpathSelection {
        toolpath_id: ToolpathId,
        edit: rs_cam_core::selection::SelectionEdit,
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
