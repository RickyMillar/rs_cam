//! The command registry, and the one door that applies a command or a
//! query.
//!
//! A command is a named, validated mutation of the session. A query is a
//! named, synchronous read. One X-macro list declares every row of both
//! kinds. A callback macro splits the rows by kind and builds the payload
//! enums [`Command`] and [`Query`], the fieldless mirror [`CommandId`],
//! and the per-row columns — the wire name, the kind, the answer type, and
//! the surface table. The idiom is `for_each_op!`
//! (`crates/rs_cam_core/src/compute/catalog.rs:149`).
//!
//! [`ProjectSession::apply`] runs one command and reports [`Effects`] —
//! the toolpath indices the command dropped, whether it cleared the
//! simulation, and the revision of the toolpath it names. One producer
//! answers every surface, so the MCP reply and the core drop cannot
//! disagree. [`ProjectSession::query`] runs one query and reports one
//! [`QueryAnswer`] variant, named after the row.
//!
//! WP3 adds the one construction site for [`Effects`]. Every mutation on
//! [`ProjectSession`] runs its body inside
//! [`ProjectSession::try_with_effects`], so no mutation builds an
//! [`Effects`] of its own.
//!
//! WP9 adds the `Query` kind and its first row, `ToolpathCycleTime`.
//! Later work packages add the rest.
//!
//! WP8 adds `RestoreToolpathSnapshot`, the door the GUI undo stack and
//! the GUI redo stack take. It is the first row that invalidates
//! UNCONDITIONALLY by contract: it compares nothing, so a restore of a
//! byte-identical configuration still drops the chain.
//!
//! WP5 adds `ReplaceToolpathConfig`, the door the GUI inspector takes. It
//! is the first row that GATES its invalidation: the panel applies it on
//! every frame it is open, so the row always writes the configuration and
//! drops the chain only when
//! [`ToolpathConfig::generation_inputs_signature`] moved. The two rows
//! answer two different questions, and neither can serve the other.
//!
//! WP10 adds the `Job` kind and its first row, `GenerateToolpath`. A
//! job runs three synchronous steps and never an `async fn`:
//! [`ProjectSession::start`] captures on the frame loop and answers a
//! [`JobHandle`]; a free function runs the work off the loop holding no
//! session; `apply(Command::AdoptResult { .. })` records the answer.
//! The kind cost the callback macro a third accumulator and one more
//! per-row arm, not a rewrite.
//!
//! WP4 adds the MCP mutation section: most `Command` rows carry an MCP
//! wire name. This module states no row count. `CommandId::ALL.len()`
//! is the count, and a count written in prose goes stale the day the
//! next row lands. This one did, twice. Three MCP mutations stay
//! hand-written holdouts and are NOT rows here
//! (§15 ruling 4): `apply_feeds`, whose funnel resolves tool, machine,
//! material and a recommendation from view state; `plan_multitool_
//! finishing`, whose outcome IS the reply; and `export_gcode`, whose
//! pre-flight gate reads the view's own simulation slot.
//!
//! WP14a adds the second and third `Job` rows,
//! `recommend_clearing_strategy` and `preview_tier_map`. Both are READS:
//! they capture, run off the frame loop and adopt NOTHING, so a `Job` is
//! two steps here and not three. The package also made the answer column
//! uniform: a `Job` row now names what its `execute_*` function returns,
//! the generated [`JobAnswer`] carries those answers, and [`JobHandle`]
//! is the one hand-written block — a handle is named `<Id>Handle` by
//! convention, and `macro_rules!` cannot build an identifier by
//! concatenation.
//!
//! WP14b adds the fourth `Job` row, `optimize_toolpath`, and it is the
//! first job that MUTATES. The optimizer writes the toolpath's
//! parameters, regenerates it and re-simulates the project once per
//! candidate, so it cannot reduce its inputs to a capture list. Step (i)
//! CLONES the session into the handle and takes the baseline cut trace
//! off the session beside it; step (ii) runs the search on that clone;
//! there is no step (iii). [`ProjectSession`] derives `Clone` for this
//! row alone. The GUI used to hand the whole session to the Optimize
//! lane with `std::mem::replace` and draw against an empty placeholder
//! until the lane gave it back.
//!
//! WP15a closes the `apply`-only write path from the registry side
//! (§25 ruling 1). Twenty-four public `ProjectSession` setters had no
//! row: a surface reached them, the registry never named them, and the
//! surface table could not say which surface wrote what. Each one now
//! carries a row whose `apply` arm DELEGATES to it. The setter stays
//! `pub` and keeps the invalidation rule — §12 ruling 2 allows one
//! construction site, and N15 measured what two producers cost. Eleven
//! of the twenty-four carry no production caller at all; they take a
//! row so the property is TOTAL, and a sentry
//! (`tests/setters_have_rows_wp15a.rs`) reads the source and holds it.
//! `set_toolpath_param` is the one setter outside that property: its
//! body calls `apply`, so it is already on the door.
//!
//! WP13 adds the fifth kind, `UiQuery`, and the `GetOperationSchema`
//! row. No row here declares `UiCommand` or `UiQuery`: both kinds belong
//! to the view registry `for_each_ui_command!`
//! (`crates/rs_cam_viz/src/ui_command.rs`), which reuses this module's
//! [`CommandKind`], [`Reach`] and [`Surfaces`] so the two registries
//! speak one language. Core cannot name a view payload type, so a view
//! row cannot live here. `GetOperationSchema` is the exception that
//! proves the rule: the MCP arm read the operation CATALOG and never
//! the view, so ruling 3 makes it a core `Query`.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use super::cycle_time::{self, CycleTime};
use super::{
    GenerateToolpathHandle, MultitoolPlanSpec, MultitoolPreview, OptimizeToolpathHandle,
    PreviewTierMapHandle, ProjectSession, RecommendClearingStrategyHandle, SessionError,
    ToolpathComputeResult, ToolpathConfig,
};
use crate::compute::catalog::{OperationConfig, OperationSchema};
use crate::compute::config::DressupConfig;
use crate::compute::simulate::SimulationResult;
use crate::feeds::FeedsProvenance;
use crate::geometry::enriched_mesh::FaceGroupId;
use crate::machine::strategy_advisor::StrategyRecommendation;
use crate::stock::simulation_cut::SimulationCutTrace;
use crate::tool_load::optimize::OptimizeOutcome;

/// Declares every command, query and job row once.
///
/// The columns are: the row kind (`Command`, `Query` or `Job`), the
/// identifier, the wire name, the payload type, the answer type, and the
/// surface table. A `Command` row's answer type is always [`Effects`]; a
/// `Query` row names its own answer struct; a `Job` row names what its
/// `execute_*` function returns. Edit this list, not the blocks a
/// callback macro generates from it.
///
/// The macro carries `#[macro_export]` because a sentry in `tests/`
/// counts the rows with its own callback. `for_each_op!` needs no export:
/// its completeness sentry is an in-file unit test.
#[macro_export]
macro_rules! for_each_command {
    ($m:ident) => {
        $m! {
            //  kind      id                wire name             payload
            //  answer / handle     surfaces
            (Command, SetToolpathParam, "set_toolpath_param", SetToolpathParamArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "the GUI inspector replaces the whole config through replace_toolpath_config",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Reached,
             }),
            (Command, AdoptResult, "adopt_result", AdoptResultArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "a completion is adopted by the GUI drain, not by a wire tool",
                 ),
                 cli: Reach::Skip(
                     "the CLI generates synchronously and never adopts a completion",
                 ),
             }),
            (Command, AddAlignmentPin, "add_alignment_pin", AddAlignmentPinArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "no GUI control calls this setter; the pin placer writes the stock",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, RemoveAlignmentPin, "remove_alignment_pin", RemoveAlignmentPinArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "no GUI control calls this setter; the pin placer writes the stock",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, AddModel, "import_model", AddModelArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Reached,
             }),
            (Command, AdoptModelGeometry, "adopt_model_geometry", AdoptModelGeometryArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "no MCP tool refreshes a model in place; the wire imports a new one",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI imports each model once and never refreshes it",
                 ),
             }),
            (Command, AddSetup, "add_setup", AddSetupArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, RemoveSetup, "remove_setup", RemoveSetupArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip("no MCP tool removes a setup"),
                 cli: Reach::Skip("the batch CLI exposes no such command"),
             }),
            (Command, SetSetupFace, "set_setup_face", SetSetupFaceArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, SetSetupRotation, "set_setup_rotation", SetSetupRotationArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, SetSetupName, "set_setup_name", SetSetupNameArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "no MCP tool writes this; the wire has no such mutation",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, SetSetupDatum, "set_setup_datum", SetSetupDatumArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "no MCP tool writes this; the wire has no such mutation",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, SetSetupModels, "set_setup_models", SetSetupModelsArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "no MCP tool writes this; the wire has no such mutation",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, SetSetupPauseMessage, "set_setup_pause_message",
             SetSetupPauseMessageArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "no MCP tool writes this; the wire has no such mutation",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, MoveToolpathToSetup, "move_toolpath_to_setup",
             MoveToolpathToSetupArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, SaveProject, "save_project", SaveProjectArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "the GUI saves through its own controller door",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI emits G-code and writes no project file",
                 ),
             }),
            (Command, SetToolParam, "set_tool_param", SetToolParamArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "the GUI tool panel commits a whole draft config, not one parameter",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Reached,
             }),
            (Command, SetToolpathTool, "set_toolpath_tool", SetToolpathToolArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "the GUI inspector replaces the whole config through replace_toolpath_config",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, SetToolpathModel, "set_toolpath_model", SetToolpathModelArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "the GUI inspector replaces the whole config through replace_toolpath_config",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, SetToolpathHeights, "set_toolpath_heights", SetToolpathHeightsArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "the GUI inspector replaces the whole config through replace_toolpath_config",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, SetToolpathDebugOptions, "set_toolpath_debug_options",
             SetToolpathDebugOptionsArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "no MCP tool writes this; the wire has no such mutation",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, AddToolpath, "add_toolpath", AddToolpathArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Reached,
             }),
            (Command, RemoveToolpath, "remove_toolpath", RemoveToolpathArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, AddTool, "add_tool", AddToolArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Reached,
             }),
            (Command, AddToolFromLibrary, "add_tool_from_library", AddToolArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, RemoveTool, "remove_tool", RemoveToolArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, SetStockConfig, "set_stock_config", SetStockConfigArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Reached,
             }),
            (Command, SetStockSource, "set_stock_source", SetStockSourceArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "the operation panel writes the whole config through replace_toolpath_config",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, SetMachine, "load_machine_from_library", SetMachineArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, SetMachineKinematics, "set_machine_kinematics",
             SetMachineKinematicsArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Reached,
             }),
            (Command, ImportMachineSettings, "import_machine_settings",
             ImportMachineSettingsArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, SetPostConfig, "set_spindle_strategy", SetPostConfigArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Reached,
             }),
            (Command, SetBoundaryConfig, "set_boundary_config", SetBoundaryConfigArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "the boundary picker writes the whole config through replace_toolpath_config",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, SetRestAnalysisConfig, "set_rest_analysis_config",
             SetRestAnalysisConfigArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "the GUI inspector replaces the whole config through replace_toolpath_config",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, SetDressupConfig, "set_dressup_config", SetDressupConfigArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "the GUI inspector replaces the whole config through replace_toolpath_config",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, SetDressupField, "set_dressup_field", SetDressupFieldArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "the GUI inspector replaces the whole config through replace_toolpath_config",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, SetToolpathEnabled, "set_toolpath_enabled", SetToolpathEnabledArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Reached,
             }),
            (Query, ToolpathCycleTime, "toolpath_cycle_time", ToolpathCycleTimeArgs,
             ToolpathCycleTimeAnswer,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "get_cut_trace and narrate_toolpath report other quantities",
                 ),
                 cli: Reach::Skip(
                     "the CLI project report prints the simulation total, not per-toolpath",
                 ),
             }),
            (Command, RestoreToolpathSnapshot, "restore_toolpath_snapshot",
             RestoreToolpathSnapshotArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "undo and redo are GUI actions; MCP has no history",
                 ),
                 cli: Reach::Skip("the CLI holds no undo history"),
             }),
            (Command, ReplaceToolpathConfig, "replace_toolpath_config",
             ReplaceToolpathConfigArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "the MCP door edits one named parameter through set_toolpath_param",
                 ),
                 cli: Reach::Reached,
             }),
            (Command, ReplaceTool, "replace_tool", ReplaceToolArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "the MCP door edits one named parameter through set_tool_param",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, ReplaceFixture, "replace_fixture", ReplaceFixtureArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "no MCP tool writes a fixture; the wire has no such mutation",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, ReplaceKeepOut, "replace_keep_out", ReplaceKeepOutArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "no MCP tool writes a keep-out zone; the wire has no such mutation",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Query, GetOperationSchema, "get_operation_schema", GetOperationSchemaArgs,
             GetOperationSchemaAnswer,
             Surfaces {
                 gui: Reach::Skip(
                     "the GUI inspector draws the catalog directly; no panel asks for a schema",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no schema tool",
                 ),
             }),
            // ── WP15a: a row for every public setter ─────────────
            //
            // §25 ruling 1. The setter stays `pub` and keeps the
            // invalidation rule. The row delegates to it, so the two
            // routes cannot carry two staleness models.
            (Command, ReorderToolpath, "reorder_toolpath", ReorderToolpathArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "no MCP tool re-orders the plan; the wire has no such mutation",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, RemoveModel, "remove_model", RemoveModelArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "no MCP tool removes a model; the wire imports one only",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, SetFaceSelection, "set_face_selection", SetFaceSelectionArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "no MCP tool picks BREP faces; the wire has no such mutation",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI picks no faces",
                 ),
             }),
            (Command, SetAlignmentPinDrillHoles, "set_alignment_pin_drill_holes",
             SetAlignmentPinDrillHolesArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "no MCP tool writes the pin holes; the wire has no such mutation",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, SetDrillSelectedHoles, "set_drill_selected_holes",
             SetDrillSelectedHolesArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "no MCP tool picks drill holes; the wire has no such mutation",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, AddFixture, "add_fixture", AddFixtureArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "no MCP tool writes a fixture; the wire has no such mutation",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, RemoveFixture, "remove_fixture", RemoveFixtureArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "no MCP tool writes a fixture; the wire has no such mutation",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, AddKeepOut, "add_keep_out", AddKeepOutArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "no MCP tool writes a keep-out zone; the wire has no such mutation",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, RemoveKeepOut, "remove_keep_out", RemoveKeepOutArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "no MCP tool writes a keep-out zone; the wire has no such mutation",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, AutoEnableRestAnalysis, "auto_enable_rest_analysis",
             AutoEnableRestAnalysisArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "the MCP door writes the whole block through set_rest_analysis_config",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, ForgetResult, "forget_result", ForgetResultArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "a refusal is forgotten by the GUI drain, not by a wire tool",
                 ),
                 cli: Reach::Skip(
                     "the CLI generates synchronously and forgets no completion",
                 ),
             }),
            (Command, SetToolpathOperation, "set_toolpath_operation",
             SetToolpathOperationArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "no GUI control changes an operation kind in place",
                 ),
                 mcp: Reach::Skip(
                     "no MCP tool changes an operation kind; add_toolpath adds a new operation",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, InvalidateStock, "invalidate_stock", InvalidateStockArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "no GUI control drops the results alone; set_stock_config writes and drops",
                 ),
                 mcp: Reach::Skip(
                     "no MCP tool drops the results alone; set_stock_config writes and drops",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, InvalidateMachine, "invalidate_machine", InvalidateMachineArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "no GUI control drops the simulation alone; set_machine writes and drops",
                 ),
                 mcp: Reach::Skip(
                     "no MCP tool drops the simulation alone; set_machine writes and drops",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, InvalidateTool, "invalidate_tool", InvalidateToolArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "no GUI control drops a tool's results alone; replace_tool writes and drops",
                 ),
                 mcp: Reach::Skip(
                     "no MCP tool drops a tool's results alone; set_tool_param writes and drops",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, InvalidateModel, "invalidate_model", InvalidateModelArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "the three model refresh doors take adopt_model_geometry, which also drops",
                 ),
                 mcp: Reach::Skip(
                     "no MCP tool refreshes a model in place; the wire imports a new one",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI imports each model once and never refreshes it",
                 ),
             }),
            (Command, InvalidateToolpathInputs, "invalidate_toolpath_inputs",
             InvalidateToolpathInputsArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "the feeds Apply funnel writes one replace_toolpath_config, which drops",
                 ),
                 mcp: Reach::Skip(
                     "the apply_feeds holdout takes the same funnel and writes one config row",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, UpdateStockFromBbox, "update_stock_from_bbox",
             UpdateStockFromBboxArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "no GUI control sizes the stock from a bounding box",
                 ),
                 mcp: Reach::Skip(
                     "no MCP tool sizes the stock from a bounding box",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, ReplaceTools, "replace_tools", ReplaceToolsArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "no GUI control replaces the whole tools list",
                 ),
                 mcp: Reach::Skip(
                     "no MCP tool replaces the whole tools list",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Command, SetFeedsProvenance, "set_feeds_provenance",
             SetFeedsProvenanceArgs, Effects,
             Surfaces {
                 gui: Reach::Skip(
                     "the optimizer carries the stamp in restore_toolpath_snapshot since WP8",
                 ),
                 mcp: Reach::Skip(
                     "no MCP tool stamps a provenance on its own",
                 ),
                 cli: Reach::Skip(
                     "the batch CLI exposes no such command",
                 ),
             }),
            (Job, GenerateToolpath, "generate_toolpath", GenerateToolpathArgs,
             ToolpathComputeResult,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Reached,
             }),
            (Command, AdoptSimulation, "adopt_simulation", AdoptSimulationArgs, Effects,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Skip(
                     "the MCP run_simulation tool simulates through the GUI lane, which adopts",
                 ),
                 cli: Reach::Skip(
                     "the CLI simulates through run_simulation, which stores the result itself",
                 ),
             }),
            (Job, RecommendClearingStrategy, "recommend_clearing_strategy",
             RecommendClearingStrategyArgs,
             Option<StrategyRecommendation>,
             Surfaces {
                 gui: Reach::Skip(
                     "no GUI panel asks the advisor; the verdict surfaces on the MCP wire only",
                 ),
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no strategy advisor command",
                 ),
             }),
            (Job, PreviewTierMap, "preview_tier_map", PreviewTierMapArgs,
             MultitoolPreview,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no planner preview command",
                 ),
             }),
            (Job, OptimizeToolpath, "optimize_toolpath", OptimizeToolpathArgs,
             OptimizeOutcome,
             Surfaces {
                 gui: Reach::Reached,
                 mcp: Reach::Reached,
                 cli: Reach::Skip(
                     "the batch CLI exposes no optimizer command",
                 ),
             }),
        }
    };
}

/// How the session runs a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    /// A synchronous validated mutation. It returns [`Effects`].
    Command,
    /// A synchronous read. It changes nothing.
    Query,
    /// Three synchronous steps: capture, run off the frame loop, adopt.
    Job,
    /// A view command. It never enters the core session.
    UiCommand,
    /// A view read. It never enters the core session.
    UiQuery,
}

/// Whether one surface reaches a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reach {
    /// The surface calls the command.
    Reached,
    /// The surface does not call the command. The text says why.
    Skip(&'static str),
}

/// The surfaces of one command row.
///
/// This is a struct, not a list. A literal that omits a field does not
/// compile, so a forgotten surface is a build error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Surfaces {
    /// The desktop application.
    pub gui: Reach,
    /// The MCP server the desktop application embeds.
    pub mcp: Reach,
    /// The batch command line.
    pub cli: Reach,
}

/// The arguments of the `set_toolpath_param` command.
#[derive(Debug, Clone, PartialEq)]
pub struct SetToolpathParamArgs {
    /// The index of the toolpath the command writes.
    pub index: usize,
    /// The name of the operation parameter.
    pub param: String,
    /// The value to write.
    pub value: serde_json::Value,
}

/// The arguments of the `adopt_result` command.
///
/// A generation runs off the frame loop, so the configuration can move
/// while it runs. `revision` is the generation-input revision the lane
/// started from, read with
/// [`ProjectSession::toolpath_revision`](super::ProjectSession::toolpath_revision).
/// [`ProjectSession::apply`] compares it against the revision the
/// toolpath carries now and refuses a mismatch with
/// [`SessionError::StaleCompletion`], inserting nothing.
///
/// The command STALES the adopted toolpath's Regions and PrevTool
/// consumers (D1). A source that regenerates without an input edit still
/// publishes a different answer, and a consumer that resolved its boundary
/// against the old rest regions describes a region set that no longer
/// exists. It stales no `FromRemainingStock` consumer: recording an answer
/// removes no material. It keeps the simulation.
///
/// `result` is boxed. A [`ToolpathComputeResult`] carries the whole
/// annotated toolpath, its statistics and two optional traces, which is
/// hundreds of bytes beside the other rows' arguments.
#[derive(Debug, Clone)]
pub struct AdoptResultArgs {
    /// The index of the toolpath the completion answers.
    pub index: usize,
    /// The revision the compute lane started from.
    pub revision: u64,
    /// The computed result.
    pub result: Box<ToolpathComputeResult>,
}

/// The arguments of the `adopt_simulation` command.
///
/// The GUI simulates on its own lane, off the frame loop, and adopts the
/// answer into view state for the viewport. This row carries the SAME
/// answer into the session, so one process holds one simulation state.
/// [`ProjectSession::start`](super::ProjectSession::start) reads the rest
/// snapshot from that state, and a GUI that adopted into view state alone
/// refused every `FromRemainingStock` operation — N12 item 10.
///
/// The command REPLACES whatever simulation the session holds. It stales
/// no toolpath, because it moves no generation input.
///
/// `result` is boxed. A [`SimulationResult`] carries a display mesh, a
/// per-toolpath checkpoint list and a prior-stock map, which is far more
/// than the other rows' arguments.
#[derive(Clone)]
pub struct AdoptSimulationArgs {
    /// The simulation to store, exactly as
    /// [`ProjectSession::run_simulation`](super::ProjectSession::run_simulation)
    /// stores it: the display mesh, the boundaries, the checkpoints, the
    /// collisions, the cut trace and the prior stocks.
    ///
    /// A caller that modulates the cut trace attaches the MODULATED trace
    /// here. `run_simulation` stores the trace its own post-pass leaves,
    /// and a caller that adopted the pre-modulation trace would publish
    /// two different traces on two surfaces.
    pub result: Box<SimulationResult>,
    /// The simulation epoch the lane started from, read with
    /// [`ProjectSession::simulation_epoch`](super::ProjectSession::simulation_epoch).
    ///
    /// D7 (W0c). This row used to store unconditionally, so a run that
    /// landed after an edit had cleared the field re-filled it and every
    /// reader called it current. [`ProjectSession::apply`] now compares
    /// this against the epoch the session carries and refuses a mismatch
    /// with [`SessionError::StaleSimulation`], storing nothing.
    pub epoch: u64,
}

/// Names the simulation's countable parts, and no geometry.
///
/// [`Command`] derives `Debug` and a [`SimulationResult`] publishes none,
/// so this impl answers for the row. It prints what a reader of a log
/// wants — how many moves, how many snapshots, whether a trace came with
/// it — and never a marching-cubes vertex list.
impl std::fmt::Debug for AdoptSimulationArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdoptSimulationArgs")
            .field("total_moves", &self.result.total_moves)
            .field("prior_stocks", &self.result.prior_stocks.len())
            .field("cut_trace", &self.result.cut_trace.is_some())
            .field("epoch", &self.epoch)
            .finish_non_exhaustive()
    }
}

/// The arguments of the `restore_toolpath_snapshot` command.
///
/// The GUI undo stack, the GUI redo stack and the three optimizer apply
/// paths all install a captured `(operation, dressups, face_selection)`
/// triple on one toolpath. The optimizer paths also stamp a feeds
/// provenance on the dimensions they changed, which is why the payload
/// carries one: before WP8 they wrote it in a second call, and an undo
/// then restored the earlier values under the later stamp.
///
/// **The command invalidates UNCONDITIONALLY, by contract.** It compares
/// nothing. A restore of the byte-identical configuration the cached
/// geometry was generated from still drops the chain (F2.5). Nothing
/// records which configuration a cached result answers; the snapshot
/// restores three of the nine fields that decide the geometry; and being
/// wrong this way costs one regeneration, while being wrong the other
/// way exports a program that does not match the project. A
/// signature-gated command cannot serve undo, which is why this row is
/// not `ReplaceToolpathConfig`.
///
/// Three fields are boxed. [`Command`] is one enum, so its size is the
/// size of its largest variant, and an operation configuration, a
/// dressup configuration and six provenance slots together run to
/// several hundred bytes beside the other rows' arguments.
/// `large_enum_variant` is denied.
#[derive(Debug, Clone)]
pub struct RestoreToolpathSnapshotArgs {
    /// The index of the toolpath the command restores.
    pub index: usize,
    /// The operation configuration to install.
    pub operation: Box<OperationConfig>,
    /// The dressup configuration to install.
    pub dressups: Box<DressupConfig>,
    /// The BREP face selection to install.
    pub face_selection: Option<Vec<FaceGroupId>>,
    /// The feeds provenance to install.
    ///
    /// `None` means the caller restores no provenance. The toolpath
    /// keeps the provenance it carries, and the command clears nothing.
    pub feeds_provenance: Option<Box<FeedsProvenance>>,
}

/// The arguments of the `replace_toolpath_config` command.
///
/// The GUI inspector holds no commit event. It rebuilds an owned entry
/// from the session, lets the widgets write it, and writes the entry back
/// on every frame the panel is open. So this command runs on every such
/// frame, and the gate below — not the panel — decides whether the cached
/// geometry survives.
///
/// **The command always writes the configuration, and invalidates only
/// when [`ToolpathConfig::generation_inputs_signature`] moved.** The two
/// halves are independent. Name, coolant, the pre and post G-code and the
/// debug options sit OUTSIDE the signature: they dirty the project and
/// change no motion, so they must land on a frame that drops nothing. A
/// return before the write would stop them landing.
///
/// This row therefore differs from
/// [`RestoreToolpathSnapshotArgs`], which invalidates unconditionally by
/// contract. A restore compares nothing because nothing records which
/// configuration a cached result answers; a replacement carries the whole
/// configuration, so the comparison is available and correct.
///
/// The configuration is boxed. [`Command`] is one enum, so its size is
/// the size of its largest variant, and a [`ToolpathConfig`] carries an
/// operation configuration, a dressup configuration, a heights
/// configuration and six provenance slots. `large_enum_variant` is
/// denied.
#[derive(Debug, Clone)]
pub struct ReplaceToolpathConfigArgs {
    /// The index of the toolpath the command replaces.
    pub index: usize,
    /// The configuration to install, in full.
    pub config: Box<ToolpathConfig>,
}

/// The arguments of the `generate_toolpath` job.
///
/// The payload names the toolpath and nothing else. Every generation
/// input comes from the session at step (i), so a caller cannot hand
/// one in: that is what makes the resolver the only assembly
/// (`planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md` §1).
/// The payload derives no `Copy`. [`ProjectSession::start`] matches on
/// the [`Job`] it is handed, and a `Copy` payload would make that match
/// a read rather than a move — the argument would then be passed by
/// value and never consumed, which `needless_pass_by_value` denies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenerateToolpathArgs {
    /// The index of the toolpath to generate.
    pub index: usize,
}

/// The arguments of the `recommend_clearing_strategy` job.
///
/// The payload names the toolpath and nothing else. The candidate set,
/// the machine and the material all come from the session at step (i).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecommendClearingStrategyArgs {
    /// The index of the toolpath to advise on. The advisor answers for an
    /// `Adaptive3d` operation and reports `None` for every other one.
    pub index: usize,
}

/// The arguments of the `preview_tier_map` job.
///
/// The spec is BOXED. Every other payload in this registry is a handful
/// of scalars, and a plan spec carries three vectors and eleven dials;
/// inline it would set the size of [`Job`] on its own, which
/// `clippy::large_enum_variant` denies.
#[derive(Debug, Clone)]
pub struct PreviewTierMapArgs {
    /// The ladder and the dials to preview. Nothing here is derived from
    /// the session; every field is an operator dial.
    pub spec: Box<MultitoolPlanSpec>,
}

/// The arguments of the `optimize_toolpath` job.
///
/// The payload names the toolpath and nothing else. The baseline cut
/// trace, the machine and the material all come from the session at step
/// (i), so a caller cannot hand one in (§28 ruling 5). The trace used to
/// come from the VIEW's own simulation slot on both surfaces, and the two
/// slots diverge: a session mutation clears the session's. An
/// `optimize_toolpath` issued after such an edit now refuses at submit
/// with [`SessionError::SimulationRequired`], where it used to score
/// against a measurement the edit had already invalidated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizeToolpathArgs {
    /// The index of the toolpath to optimize.
    pub index: usize,
}

/// What a command changed.
///
/// The type carries `#[must_use]`. A caller that runs a mutation and
/// drops the answer states that it drops it, with `let _ =`.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effects {
    /// Every toolpath index whose generation-input revision moved.
    ///
    /// The set comes from the revision map, not from the `drop_result`
    /// call list. `bump_all_revisions`
    /// (`crates/rs_cam_core/src/session/mutation.rs:1339`) moves every
    /// index on a removal and calls `drop_result` on none of them, so a
    /// call list reports nothing stale for a removal.
    pub stale: BTreeSet<usize>,
    /// `true` when a simulation result existed before the command and
    /// does not exist after it.
    ///
    /// The chain writes `None` unconditionally, so the write alone is not
    /// evidence that a simulation existed.
    pub simulation_cleared: bool,
    /// The generation-input revision of the toolpath the command names,
    /// read after the mutation.
    ///
    /// This is [`ProjectSession::toolpath_revision`], never
    /// `next_revision`. `next_revision` is a session-global counter that
    /// any index bumps.
    ///
    /// `Some` only when the command names ONE toolpath that still exists
    /// at the SAME index after the command. A bulk mutation, a
    /// setup-index mutation, a removal and a re-order all report `None`,
    /// which means NOT MEASURED. `0` cannot carry that meaning: every
    /// toolpath starts at revision `0`.
    pub revision: Option<u64>,
    /// What an add created, or `None` when the command created nothing.
    ///
    /// WP4 ruling 3 (§15). Four commands append to a list, and the
    /// caller needs the thing they appended: `AddToolpath`, `AddTool`,
    /// `AddSetup` and `AddModel`. On a pure append [`Self::stale`] holds
    /// the new index alone and [`Self::revision`] is `None`, so neither
    /// field can carry the answer.
    ///
    /// **The four do not report one quantity.** Each reports the value
    /// its producer reported before WP4, because that is the value every
    /// caller already consumes:
    ///
    /// - `AddToolpath` — the new toolpath's INDEX in plan order;
    /// - `AddTool` — the new tool's INDEX in the tools list;
    /// - `AddSetup` — the new setup's INDEX in the setups list;
    /// - `AddModel` — the new model's ID, not its index. A model is
    ///   named by id on every other surface, and `add_model` always
    ///   answered with `LoadedModel::id`.
    ///
    /// Read the row before reading the number. `None` means the command
    /// created nothing; it never means index zero.
    pub created: Option<usize>,
}

// ── WP4 payloads ─────────────────────────────────────────────────
//
// One `*Args` struct per MCP mutation row. `rs_cam_mcp` depends on this
// crate, so core cannot name an `rs_cam_mcp` parameter struct: the
// dependency runs one way. Each surface converts its own parameter
// struct into the core payload at its boundary. That conversion is also
// what keeps the WP2a wire snapshot still: a schema title comes from the
// `rs_cam_mcp` struct, which stays where it is.
//
// A payload carries CORE types, never wire types. A wire string like
// `"bottom"` becomes a `FaceUp` on the surface that parsed it, so one
// parser answers for every surface and a refusal reaches the operator in
// that surface's own words.
//
// A field that carries a large configuration struct is boxed. `Command`
// is one enum over every row, so its size is the size of its largest
// payload, and `large_enum_variant` is a denied lint.

/// The arguments of the `add_alignment_pin` command.
///
/// The setter dedupes against the pins the stock already carries. A
/// duplicate reports an empty [`Effects`], which says the call changed
/// nothing.
#[derive(Debug, Clone)]
pub struct AddAlignmentPinArgs {
    /// The pin centre X, in millimetres, in the stock-local frame.
    pub x: f64,
    /// The pin centre Y, in millimetres, in the stock-local frame.
    pub y: f64,
    /// The pin diameter, in millimetres.
    pub diameter: f64,
}

/// The arguments of the `remove_alignment_pin` command.
#[derive(Debug, Clone)]
pub struct RemoveAlignmentPinArgs {
    /// The index of the pin to remove, in the stock's own pin order.
    pub index: usize,
}

/// The arguments of the `import_model` command.
///
/// The payload is a LOADED model, not a path. The import itself —
/// reading the STL, SVG, DXF or STEP file and applying the declared
/// units — belongs to the surface that owns the file dialogue, and core
/// adopts the geometry that import produced.
///
/// `model.id` is overwritten: the session assigns the next free id.
/// [`Effects::created`] then reports that id.
#[derive(Debug, Clone)]
pub struct AddModelArgs {
    /// The imported model to adopt.
    pub model: Box<super::LoadedModel>,
}

/// The arguments of the `adopt_model_geometry` command.
///
/// The GUI holds three model-refresh doors — rescale, reload and relink.
/// Each re-imports the file and hands the result here. The IMPORT belongs
/// to the surface that owns the file dialogue; core adopts the geometry
/// that import produced and drops the results that read it.
///
/// The payload names the model by `model_id`, and the record keeps that
/// id and its name. Keeping the id is the point of a refresh: every
/// `ToolpathConfig::model_id` goes on naming this model, so the
/// operations survive it.
///
/// `units` of `None` means the caller overrides no unit declaration.
/// Only the rescale door sends `Some`, because a rescale IS the
/// declared-units change.
///
/// The geometry is boxed. [`Command`] is one enum, so its size is the
/// size of its largest payload, and a [`LoadedModel`](super::LoadedModel)
/// carries a mesh, a polygon list and an enriched mesh.
/// `large_enum_variant` is denied.
#[derive(Debug, Clone)]
pub struct AdoptModelGeometryArgs {
    /// The id of the model to refresh.
    pub model_id: usize,
    /// The freshly imported model whose geometry the record adopts.
    pub geometry: Box<super::LoadedModel>,
    /// The unit declaration to write, or `None` to keep the current one.
    pub units: Option<crate::compute::stock_config::ModelUnits>,
}

/// The arguments of the `add_setup` command.
///
/// `name` of `None` means the session names the setup itself, after the
/// count of setups it already holds. That default lives here so the two
/// surfaces cannot name a new setup differently.
#[derive(Debug, Clone)]
pub struct AddSetupArgs {
    /// The name for the new setup, or `None` to let the session name it.
    pub name: Option<String>,
    /// The face of the stock that points up in the new setup.
    pub face_up: crate::compute::transform::FaceUp,
}

/// The arguments of the `remove_setup` command.
#[derive(Debug, Clone)]
pub struct RemoveSetupArgs {
    /// The index of the setup to remove.
    pub setup_index: usize,
}

/// The arguments of the `set_setup_face` command.
#[derive(Debug, Clone)]
pub struct SetSetupFaceArgs {
    /// The index of the setup to write.
    pub setup_index: usize,
    /// The face of the stock that points up.
    pub face_up: crate::compute::transform::FaceUp,
}

/// The arguments of the `set_setup_rotation` command.
#[derive(Debug, Clone)]
pub struct SetSetupRotationArgs {
    /// The index of the setup to write.
    pub setup_index: usize,
    /// The rotation of the stock about the vertical axis.
    pub z_rotation: crate::compute::transform::ZRotation,
}

/// The arguments of the `set_setup_name` command.
#[derive(Debug, Clone)]
pub struct SetSetupNameArgs {
    /// The index of the setup to rename.
    pub setup_index: usize,
    /// The new name.
    pub name: String,
}

/// The arguments of the `set_setup_datum` command.
#[derive(Debug, Clone)]
pub struct SetSetupDatumArgs {
    /// The index of the setup to write.
    pub setup_index: usize,
    /// How the operator zeroes the machine for this setup.
    pub datum: super::DatumConfig,
}

/// The arguments of the `set_setup_models` command.
#[derive(Debug, Clone)]
pub struct SetSetupModelsArgs {
    /// The index of the setup to write.
    pub setup_index: usize,
    /// The models in scope for the setup. An EMPTY list means "all
    /// models"; it is not the same as a list that names every model.
    pub model_ids: Vec<crate::ids::ModelId>,
}

/// The arguments of the `set_setup_pause_message` command.
///
/// The message the operator reads at the setup change. It reaches the
/// EXPORT alone, beside the `M0` the post emits, so [`Effects::stale`] is
/// empty by design: the write moves no geometry a result holds. An empty
/// `Effects` here says the command changed no toolpath, not that nothing
/// was measured.
///
/// The wizard names the setup by id and resolves the index itself, so the
/// payload matches the other setup rows.
#[derive(Debug, Clone)]
pub struct SetSetupPauseMessageArgs {
    /// The index of the setup to write.
    pub setup_index: usize,
    /// The message, or `None` to clear it.
    pub message: Option<String>,
}

/// The arguments of the `move_toolpath_to_setup` command.
#[derive(Debug, Clone)]
pub struct MoveToolpathToSetupArgs {
    /// The index of the toolpath to move, in plan order.
    pub toolpath_index: usize,
    /// The index of the setup the toolpath lands in.
    pub target_setup_index: usize,
    /// The gap the toolpath lands in, counted in the TARGET setup's own
    /// plan order. `None` appends. A drag carries the position the
    /// operator pointed at; the MCP tool has no such argument and passes
    /// `None`.
    pub target_position: Option<usize>,
}

/// The arguments of the `save_project` command.
///
/// The command writes a file and changes no session state, so its
/// [`Effects`] is empty. It is a `Command` and not a `Query` because the
/// operator asks it to act, and because a refusal must reach the same
/// door every other mutation's refusal reaches.
#[derive(Debug, Clone)]
pub struct SaveProjectArgs {
    /// The path of the project file to write.
    pub path: std::path::PathBuf,
}

/// The arguments of the `set_tool_param` command.
#[derive(Debug, Clone)]
pub struct SetToolParamArgs {
    /// The index of the tool to write.
    pub index: usize,
    /// The name of the tool parameter.
    pub param: String,
    /// The value to write.
    pub value: serde_json::Value,
}

/// The arguments of the `replace_tool` command.
///
/// The GUI tool panel edits a DRAFT clone of the tool and commits the
/// whole draft on Apply, so the payload is a whole [`ToolConfig`] and not
/// one named parameter. `tool_id` is the project-assigned id, NOT the
/// tool's position in the tools list; the two agree until a tool is
/// removed.
///
/// The configuration is boxed. A [`ToolConfig`] carries the whole cutter
/// description, which is large beside the other rows' arguments.
#[derive(Debug, Clone)]
pub struct ReplaceToolArgs {
    /// The id of the tool to replace.
    pub tool_id: usize,
    /// The configuration to install.
    pub config: Box<crate::compute::tool_config::ToolConfig>,
}

/// The arguments of the `replace_fixture` command.
///
/// The GUI fixture panel edits every field of one fixture, so the
/// payload is the whole record. `fixture_id` names which fixture of the
/// setup it replaces; the position in the list does not move.
#[derive(Debug, Clone)]
pub struct ReplaceFixtureArgs {
    /// The index of the setup that holds the fixture.
    pub setup_index: usize,
    /// The id of the fixture to replace.
    pub fixture_id: crate::ids::FixtureId,
    /// The fixture to install.
    pub fixture: Box<super::Fixture>,
}

/// The arguments of the `replace_keep_out` command.
///
/// The keep-out twin of [`ReplaceFixtureArgs`].
#[derive(Debug, Clone)]
pub struct ReplaceKeepOutArgs {
    /// The index of the setup that holds the zone.
    pub setup_index: usize,
    /// The id of the zone to replace.
    pub zone_id: crate::ids::KeepOutId,
    /// The zone to install.
    pub zone: Box<super::KeepOutZone>,
}

/// The arguments of the `set_toolpath_tool` command.
///
/// `tool_id` is the project-assigned id of a tool, NOT its position in
/// the tools list. The two agree until a tool is removed.
#[derive(Debug, Clone)]
pub struct SetToolpathToolArgs {
    /// The index of the toolpath to rebind.
    pub index: usize,
    /// The id of the tool to bind.
    pub tool_id: usize,
}

/// The arguments of the `set_toolpath_model` command.
///
/// `model_id` is the project-assigned id of a model, NOT its position in
/// the models list. A rebind does NOT clear a BREP face selection: a
/// face id belongs to the model that was bound when it was picked.
#[derive(Debug, Clone)]
pub struct SetToolpathModelArgs {
    /// The index of the toolpath to rebind.
    pub index: usize,
    /// The id of the model to bind.
    pub model_id: usize,
}

/// The arguments of the `set_toolpath_heights` command.
#[derive(Debug, Clone)]
pub struct SetToolpathHeightsArgs {
    /// The index of the toolpath to write.
    pub index: usize,
    /// The whole heights block. The command replaces it; it patches no
    /// single height.
    pub heights: crate::compute::config::HeightsConfig,
}

/// The arguments of the `set_toolpath_debug_options` command.
///
/// The generate door writes this flag immediately before it runs, so the
/// command moves no revision. A debug trace is an output of a
/// generation, never an input to one.
#[derive(Debug, Clone)]
pub struct SetToolpathDebugOptionsArgs {
    /// The index of the toolpath to write.
    pub index: usize,
    /// The debug options to record for the next generation.
    pub debug_options: crate::trace::debug_trace::ToolpathDebugOptions,
}

/// The arguments of the `add_toolpath` command.
///
/// `config.id` is overwritten: the session assigns the next free id.
/// [`Effects::created`] reports the new toolpath's INDEX in plan order.
#[derive(Debug, Clone)]
pub struct AddToolpathArgs {
    /// The index of the setup the toolpath joins.
    pub setup_index: usize,
    /// The whole toolpath configuration.
    pub config: Box<super::ToolpathConfig>,
}

/// The arguments of the `remove_toolpath` command.
#[derive(Debug, Clone)]
pub struct RemoveToolpathArgs {
    /// The index of the toolpath to remove, in plan order.
    pub index: usize,
}

/// The arguments of the `add_tool` and `add_tool_from_library` commands.
///
/// Two rows share one payload. They differ on the wire, where one takes
/// a tool the operator described and the other takes a catalog name and
/// a row number, and the catalog READ is a surface-side step. By the
/// time either reaches core both carry the same thing: a finished tool.
///
/// `tool.id` is overwritten: the session assigns the next free id.
/// [`Effects::created`] reports the new tool's INDEX in the tools list,
/// which is not its id.
#[derive(Debug, Clone)]
pub struct AddToolArgs {
    /// The tool to add.
    pub tool: Box<crate::compute::tool_config::ToolConfig>,
}

/// The arguments of the `remove_tool` command.
///
/// The command refuses while any toolpath still binds the tool.
#[derive(Debug, Clone)]
pub struct RemoveToolArgs {
    /// The index of the tool to remove.
    pub index: usize,
}

/// The arguments of the `set_stock_config` command.
///
/// The command replaces the WHOLE stock block, so a surface that offers
/// per-field edits reads the current stock, writes its own fields and
/// sends the result. Any stock edit stales EVERY toolpath
/// (G-FRESHSTATE).
#[derive(Debug, Clone)]
pub struct SetStockConfigArgs {
    /// The new stock configuration.
    pub stock: Box<crate::compute::stock_config::StockConfig>,
}

/// The arguments of the `set_stock_source` command.
#[derive(Debug, Clone)]
pub struct SetStockSourceArgs {
    /// The index of the toolpath to write.
    pub index: usize,
    /// Whether the operation cuts fresh stock or the stock the prior
    /// operations leave.
    pub source: crate::compute::config::StockSource,
}

/// The arguments of the `load_machine_from_library` command.
///
/// The row is `SetMachine`, not `LoadMachineFromLibrary`: the payload is
/// a whole machine profile, and the library READ is a surface-side step
/// (§19 ruling 2). One row therefore serves the wire tool and the GUI
/// machine panel alike.
///
#[derive(Debug, Clone)]
pub struct SetMachineArgs {
    /// The whole machine profile to adopt.
    pub machine: Box<crate::machine::MachineProfile>,
}

/// The arguments of the `set_machine_kinematics` command.
///
/// The payload is the FINISHED block. Merging a partial per-axis triple
/// onto the machine's current limits, and refusing a value that is not
/// positive and finite, belong to the surface that collected the
/// numbers: it is what reports the refusal to the operator.
#[derive(Debug, Clone)]
pub struct SetMachineKinematicsArgs {
    /// The kinematics block to write.
    pub kinematics: Box<crate::machine::kinematics::MachineKinematics>,
}

/// The arguments of the `import_machine_settings` command.
///
/// A GRBL `$$` dump carries one field more than
/// [`SetMachineKinematicsArgs`] — the travel rate — so the two rows
/// carry two payloads (§15 ruling 6). The parse belongs to the surface,
/// which also decides whether the dump was recognised at all.
#[derive(Debug, Clone)]
pub struct ImportMachineSettingsArgs {
    /// The kinematics block the dump describes.
    pub kinematics: Box<crate::machine::kinematics::MachineKinematics>,
    /// The travel rate the dump published, in millimetres per minute.
    /// `None` means the dump published none, and the machine keeps the
    /// rate it has.
    pub max_feed_mm_min: Option<f64>,
}

/// The arguments of the `set_spindle_strategy` command.
///
/// The row is `SetPostConfig` (§14.3 ruling 3): the payload is the whole
/// post-processor block, and the spindle strategy is one field of it. A
/// surface that offers the strategy alone reads the current block,
/// writes that field and sends the result.
#[derive(Debug, Clone)]
pub struct SetPostConfigArgs {
    /// The new post-processor configuration.
    pub post: Box<crate::gcode::PostConfig>,
}

/// The arguments of the `set_boundary_config` command.
#[derive(Debug, Clone)]
pub struct SetBoundaryConfigArgs {
    /// The index of the toolpath to write.
    pub index: usize,
    /// The whole boundary block.
    pub boundary: crate::compute::config::BoundaryConfig,
}

/// The arguments of the `set_rest_analysis_config` command.
#[derive(Debug, Clone)]
pub struct SetRestAnalysisConfigArgs {
    /// The index of the toolpath to write.
    pub index: usize,
    /// The whole rest-analysis block.
    pub rest_analysis: crate::compute::config::RestAnalysisConfig,
}

/// The arguments of the `set_dressup_config` command.
#[derive(Debug, Clone)]
pub struct SetDressupConfigArgs {
    /// The index of the toolpath to write.
    pub index: usize,
    /// The whole dressup block.
    pub dressups: Box<crate::compute::config::DressupConfig>,
}

/// The arguments of the `set_dressup_field` command.
///
/// This row patches ONE dressup field by name, where
/// [`SetDressupConfigArgs`] replaces the whole block.
#[derive(Debug, Clone)]
pub struct SetDressupFieldArgs {
    /// The index of the toolpath to write.
    pub index: usize,
    /// The name of the dressup field.
    pub key: String,
    /// The value to write.
    pub value: serde_json::Value,
}

/// The arguments of the `set_toolpath_enabled` command.
///
/// The toggled toolpath KEEPS its own cached result, so [`Effects`]
/// reports the downstream set alone.
#[derive(Debug, Clone)]
pub struct SetToolpathEnabledArgs {
    /// The index of the toolpath to write.
    pub index: usize,
    /// Whether the toolpath takes part in the plan.
    pub enabled: bool,
}

/// The arguments of the `toolpath_cycle_time` read.
///
/// `trace` carries the simulation the caller measured this toolpath
/// against. The session's own `simulation_result()` is a different,
/// usually-empty slot: the GUI simulates off the frame loop and keeps its
/// cut trace in its own state, so a query that read only the session's
/// slot would answer `CuttingOnly` or nothing on every real project. The
/// caller therefore supplies the trace it already holds, the same
/// argument [`cycle_time::toolpath_cycle_time`] always took.
///
/// `cutting_distance_mm` and `nominal_feed_mm_min` feed the `CuttingOnly`
/// fallback, which needs both and neither is on the trace. `None` means
/// the caller has no computed result for this toolpath yet, in which case
/// the read cannot fall back and answers [`CycleTime::NONE`].
#[derive(Debug, Clone)]
pub struct ToolpathCycleTimeArgs {
    /// The index of the toolpath to read.
    pub index: usize,
    /// The cut trace to search for this toolpath's measured runtime.
    pub trace: Option<Arc<SimulationCutTrace>>,
    /// The toolpath's cutting distance, in millimetres.
    pub cutting_distance_mm: Option<f64>,
    /// The toolpath's nominal feed rate, in millimetres per minute.
    pub nominal_feed_mm_min: Option<f64>,
}

/// The answer to the `toolpath_cycle_time` read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToolpathCycleTimeAnswer {
    /// The toolpath's cycle time, and the basis it was measured on.
    pub cycle_time: CycleTime,
}

/// The arguments of the `get_operation_schema` read.
#[derive(Debug, Clone)]
pub struct GetOperationSchemaArgs {
    /// The operation kind, in the `kind_str` spelling the catalog uses.
    pub operation_type: String,
}

/// The answer to the `get_operation_schema` read.
///
/// WP13 ruling 3 makes this a core `Query` and not a `UiQuery`: the read
/// takes the operation catalog and never the view. It reads no session
/// field either, so the arm calls the associated function.
#[derive(Debug, Clone)]
pub struct GetOperationSchemaAnswer {
    /// Every parameter the operation declares, with its type and range.
    pub schema: OperationSchema,
}

// ── WP15a: the payloads of the row-less setters ─────────────────────────
//
// §25 ruling 1 gives every public `ProjectSession` setter a row. Each
// payload below mirrors its setter's parameters in core types, and each
// arm delegates to that setter. The setter keeps the invalidation rule:
// §12 ruling 2 allows one construction site, and N15 measured what two
// producers cost.

/// The arguments of the `reorder_toolpath` command.
///
/// The command moves one operation inside the plan order of the setup
/// that owns both indices. [`Effects::revision`] reads `None`: the
/// command names two toolpaths, so no one index answers for it.
#[derive(Debug, Clone)]
pub struct ReorderToolpathArgs {
    /// The index of the operation to move, in plan order.
    pub from_index: usize,
    /// The index the operation moves to, in plan order.
    pub to_index: usize,
}

/// The arguments of the `remove_model` command.
#[derive(Debug, Clone)]
pub struct RemoveModelArgs {
    /// The index of the model to remove.
    pub index: usize,
}

/// The arguments of the `set_face_selection` command.
///
/// `face_ids` is `None` when the operation machines the whole model.
#[derive(Debug, Clone)]
pub struct SetFaceSelectionArgs {
    /// The index of the toolpath the command writes.
    pub index: usize,
    /// The BREP faces the operation machines.
    pub face_ids: Option<Vec<FaceGroupId>>,
}

/// The arguments of the `set_alignment_pin_drill_holes` command.
///
/// The command refuses a toolpath whose operation is not
/// `AlignmentPinDrill`.
#[derive(Debug, Clone)]
pub struct SetAlignmentPinDrillHolesArgs {
    /// The index of the toolpath the command writes.
    pub index: usize,
    /// The hole centres, in millimetres.
    pub holes: Vec<[f64; 2]>,
}

/// The arguments of the `set_drill_selected_holes` command.
///
/// `selected_holes` is `None` when a `Drill` operation reverts to every
/// polygon centroid the model carries. The command refuses a toolpath
/// whose operation is not a drilling operation.
#[derive(Debug, Clone)]
pub struct SetDrillSelectedHolesArgs {
    /// The index of the toolpath the command writes.
    pub index: usize,
    /// The picked hole centres, in millimetres.
    pub selected_holes: Option<Vec<[f64; 2]>>,
}

/// The arguments of the `add_fixture` command.
///
/// The fixture is boxed, as [`ReplaceFixtureArgs`] boxes its own.
#[derive(Debug, Clone)]
pub struct AddFixtureArgs {
    /// The index of the setup the fixture joins.
    pub setup_index: usize,
    /// The fixture to add.
    pub fixture: Box<super::Fixture>,
}

/// The arguments of the `remove_fixture` command.
#[derive(Debug, Clone)]
pub struct RemoveFixtureArgs {
    /// The index of the setup that holds the fixture.
    pub setup_index: usize,
    /// The id of the fixture to remove.
    pub fixture_id: crate::ids::FixtureId,
}

/// The arguments of the `add_keep_out` command.
///
/// The keep-out twin of [`AddFixtureArgs`].
#[derive(Debug, Clone)]
pub struct AddKeepOutArgs {
    /// The index of the setup the zone joins.
    pub setup_index: usize,
    /// The zone to add.
    pub zone: Box<super::KeepOutZone>,
}

/// The arguments of the `remove_keep_out` command.
#[derive(Debug, Clone)]
pub struct RemoveKeepOutArgs {
    /// The index of the setup that holds the zone.
    pub setup_index: usize,
    /// The id of the zone to remove.
    pub zone_id: crate::ids::KeepOutId,
}

/// The arguments of the `auto_enable_rest_analysis` command.
///
/// The command names the SOURCE operation — the one whose derived rest
/// regions a later operation consumes as a boundary. It reports an
/// empty [`Effects`] when the source already reads rest regions, or
/// when the source is a rest-depth pencil that attaches them itself.
/// "Changed nothing" is an empty `Effects`, not a refusal.
#[derive(Debug, Clone)]
pub struct AutoEnableRestAnalysisArgs {
    /// The id of the source toolpath.
    pub source_id: crate::ids::ToolpathId,
}

/// The arguments of the `forget_result` command.
///
/// This is the `Err`-arm twin of [`AdoptResultArgs`]: a generation that
/// refused leaves a cached result that answers a configuration which is
/// gone, and the GUI drain forgets it here.
///
/// Forgetting an index that carries no result is a no-op, never an
/// error. [`Effects::stale`] holds `index` either way, because the drop
/// bumps the revision whether or not a result was there.
#[derive(Debug, Clone)]
pub struct ForgetResultArgs {
    /// The index of the toolpath whose cached result the command drops.
    pub index: usize,
}

/// The arguments of the `set_toolpath_operation` command.
///
/// The command replaces the operation KIND while the toolpath keeps its
/// tool, heights, boundary, dressups and position in the machining
/// order. The setter re-normalizes the dressups for the new kind.
///
/// The operation is boxed, as [`RestoreToolpathSnapshotArgs`] boxes its
/// own: an [`OperationConfig`] runs to hundreds of bytes and
/// `large_enum_variant` is denied.
#[derive(Debug, Clone)]
pub struct SetToolpathOperationArgs {
    /// The index of the toolpath the command writes.
    pub index: usize,
    /// The operation to install.
    pub operation: Box<OperationConfig>,
}

/// The arguments of the `invalidate_stock` command.
///
/// The command drops EVERY toolpath result, not the simulation alone:
/// heights reference the stock top and an inherited boundary follows
/// the stock outline (G-FRESHSTATE).
///
/// The payload carries no field. The command names the whole stock.
#[derive(Debug, Clone)]
pub struct InvalidateStockArgs;

/// The arguments of the `invalidate_machine` command.
///
/// A machine edit moves no geometry, so the command drops the
/// simulation and no toolpath result.
///
/// The payload carries no field. The command names the whole machine.
#[derive(Debug, Clone)]
pub struct InvalidateMachineArgs;

/// The arguments of the `invalidate_tool` command.
///
/// A toolpath depends on a tool through two doors: the cutter it binds,
/// and a planned-tier boundary whose islands name the tool.
/// [`Effects::stale`] reports both populations.
#[derive(Debug, Clone)]
pub struct InvalidateToolArgs {
    /// The id of the tool whose dependants the command drops.
    pub tool_id: usize,
}

/// The arguments of the `invalidate_model` command.
///
/// A model whose geometry moved invalidates every result generated
/// against it. [`AdoptModelGeometryArgs`] writes the geometry and runs
/// the same rule in ONE mutation, which is why the three GUI refresh
/// doors take that row and not this one (G-RESCALESTALE).
#[derive(Debug, Clone)]
pub struct InvalidateModelArgs {
    /// The id of the model whose dependants the command drops.
    pub model_id: usize,
}

/// The arguments of the `invalidate_toolpath_inputs` command.
///
/// The command drops the toolpath's cached result and every result
/// downstream of it, unconditionally. It compares nothing, so a caller
/// that already knows a generation input moved takes it.
#[derive(Debug, Clone)]
pub struct InvalidateToolpathInputsArgs {
    /// The index of the toolpath the command drops.
    pub index: usize,
}

/// The arguments of the `update_stock_from_bbox` command.
///
/// A two-dimensional bounding box has no Z extent, and
/// `StockConfig::update_from_bbox` then keeps the stock's own
/// thickness. Attaching a drawing does not collapse the board (F-13).
#[derive(Debug, Clone)]
pub struct UpdateStockFromBboxArgs {
    /// The bounding box the stock grows to.
    pub bbox: crate::geo::BoundingBox3,
}

/// The arguments of the `replace_tools` command.
///
/// The command replaces the WHOLE tools list and the next-id counter
/// derived from it. Tool geometry decides material removal, so the
/// command drops the simulation.
#[derive(Debug, Clone)]
pub struct ReplaceToolsArgs {
    /// The tools to install.
    pub tools: Vec<crate::compute::tool_config::ToolConfig>,
}

/// The arguments of the `set_feeds_provenance` command.
///
/// The command stamps a provenance and touches neither the result cache
/// nor the simulation.
///
/// **A caller that also writes the values takes
/// [`RestoreToolpathSnapshotArgs`] instead.** The three optimizer apply
/// paths stamped in a second call until WP8, and an undo then restored
/// the earlier values under the later stamp.
///
/// The provenance is boxed, as [`RestoreToolpathSnapshotArgs`] boxes
/// its own: it carries six slots.
#[derive(Debug, Clone)]
pub struct SetFeedsProvenanceArgs {
    /// The index of the toolpath the command stamps.
    pub index: usize,
    /// The provenance to install.
    pub feeds_provenance: Box<FeedsProvenance>,
}

/// Splits the registry's rows by kind, and emits the [`Command`],
/// [`Query`] and [`Job`] payload enums plus [`QueryAnswer`] and
/// [`JobHandle`].
///
/// GENERATED support macro for `define_command_registry!`, called with
/// the same rows that macro receives. It walks the row list one row at a
/// time — an incremental "tt-muncher" — sorting each row's columns into
/// the `commands`, `queries` or `jobs` accumulator by matching the
/// literal identifier `Command`, `Query` or `Job` in the row's first
/// column. The base case (no rows left) is tried first on every
/// recursive call, so it fires the moment the accumulators hold every
/// row and none remains.
///
/// WP10 added the third accumulator and its per-row arm. A future
/// `UiCommand` row needs one more of each — the shape does not need a
/// rewrite to grow another kind.
macro_rules! split_command_rows {
    // Base case: no rows left. Emit the three payload enums and their
    // `id()`, from the three accumulators built by the arms below.
    (@split
        commands = [$( ($c_id:ident, $c_wire:literal, $c_payload:ident) ),* $(,)?],
        queries = [
            $( ($q_id:ident, $q_wire:literal, $q_payload:ident, $q_answer:ty) ),* $(,)?
        ],
        jobs = [
            $( ($j_id:ident, $j_wire:literal, $j_payload:ident, $j_answer:ty) ),* $(,)?
        ] $(,)?
    ) => {
        /// One command and its arguments.
        ///
        /// GENERATED from the `for_each_command!` list — edit the list,
        /// not this block.
        ///
        /// The enum derives no `PartialEq`. `AdoptResultArgs` carries a
        /// [`ToolpathComputeResult`], whose parts publish no equality —
        /// a computed toolpath answers "is this the current answer?"
        /// through its revision, not through a comparison.
        #[derive(Debug, Clone)]
        pub enum Command {
            $(
                #[doc = concat!("The `", $c_wire, "` command.")]
                $c_id($c_payload),
            )*
        }

        impl Command {
            /// The identifier of this command. GENERATED.
            pub fn id(&self) -> CommandId {
                match self {
                    $(Command::$c_id(_) => CommandId::$c_id,)*
                }
            }
        }

        /// One read and its arguments.
        ///
        /// GENERATED from the `for_each_command!` list — edit the list,
        /// not this block.
        #[derive(Debug, Clone)]
        pub enum Query {
            $(
                #[doc = concat!("The `", $q_wire, "` read.")]
                $q_id($q_payload),
            )*
        }

        impl Query {
            /// The identifier of this read. GENERATED.
            pub fn id(&self) -> CommandId {
                match self {
                    $(Query::$q_id(_) => CommandId::$q_id,)*
                }
            }
        }

        /// The answer to one [`Query`].
        ///
        /// GENERATED — one variant per `Query` row, named by that row's
        /// answer column.
        #[derive(Debug, Clone)]
        pub enum QueryAnswer {
            $(
                #[doc = concat!("The answer to the `", $q_wire, "` read.")]
                $q_id($q_answer),
            )*
        }

        /// One job and its arguments.
        ///
        /// GENERATED from the `for_each_command!` list — edit the list,
        /// not this block.
        ///
        /// A job runs three synchronous steps, never an `async fn`:
        /// [`ProjectSession::start`] captures on the frame loop and
        /// answers a [`JobHandle`], a free function runs the work off
        /// the loop holding no session, and
        /// `apply(Command::AdoptResult { .. })` records the answer.
        #[derive(Debug, Clone)]
        pub enum Job {
            $(
                #[doc = concat!("The `", $j_wire, "` job.")]
                $j_id($j_payload),
            )*
        }

        impl Job {
            /// The identifier of this job. GENERATED.
            pub fn id(&self) -> CommandId {
                match self {
                    $(Job::$j_id(_) => CommandId::$j_id,)*
                }
            }
        }

        /// The answer to one [`Job`].
        ///
        /// GENERATED — one variant per `Job` row, named by that row's
        /// answer column. The answer column names exactly what the row's
        /// `execute_*` function returns, so the registry sees the shape
        /// of every job's answer the way [`QueryAnswer`] shows the shape
        /// of every read's.
        ///
        /// **Every variant boxes its payload, by rule.** A job answers
        /// with a whole computed toolpath or a whole tier map, and
        /// `clippy::large_enum_variant` is denied workspace-wide. Boxing
        /// every variant means the enum's size never depends on what the
        /// largest answer carries, so adding a row can never make this a
        /// size trap. `execute_*` returns the bare type; the caller that
        /// delivers the answer boxes it here.
        #[derive(Debug, Clone)]
        pub enum JobAnswer {
            $(
                #[doc = concat!("The answer to the `", $j_wire, "` job.")]
                $j_id(Box<$j_answer>),
            )*
        }

        impl JobAnswer {
            /// The identifier of the job this answers. GENERATED.
            pub fn id(&self) -> CommandId {
                match self {
                    $(JobAnswer::$j_id(_) => CommandId::$j_id,)*
                }
            }
        }
    };

    // Next row is a `Command` row: file it under `commands` and recurse.
    (@split
        commands = [$($commands:tt)*],
        queries = [$($queries:tt)*],
        jobs = [$($jobs:tt)*],
        (Command, $id:ident, $wire:literal, $payload:ident, $answer:ty, $surfaces:expr),
        $($rest:tt)*
    ) => {
        split_command_rows! {
            @split
            commands = [$($commands)* ($id, $wire, $payload),],
            queries = [$($queries)*],
            jobs = [$($jobs)*],
            $($rest)*
        }
    };

    // Next row is a `Query` row: file it under `queries` and recurse.
    (@split
        commands = [$($commands:tt)*],
        queries = [$($queries:tt)*],
        jobs = [$($jobs:tt)*],
        (Query, $id:ident, $wire:literal, $payload:ident, $answer:ty, $surfaces:expr),
        $($rest:tt)*
    ) => {
        split_command_rows! {
            @split
            commands = [$($commands)*],
            queries = [$($queries)* ($id, $wire, $payload, $answer),],
            jobs = [$($jobs)*],
            $($rest)*
        }
    };

    // Next row is a `Job` row: file it under `jobs` and recurse.
    (@split
        commands = [$($commands:tt)*],
        queries = [$($queries:tt)*],
        jobs = [$($jobs:tt)*],
        (Job, $id:ident, $wire:literal, $payload:ident, $answer:ty, $surfaces:expr),
        $($rest:tt)*
    ) => {
        split_command_rows! {
            @split
            commands = [$($commands)*],
            queries = [$($queries)*],
            jobs = [$($jobs)* ($id, $wire, $payload, $answer),],
            $($rest)*
        }
    };
}

macro_rules! define_command_registry {
    (
        $( ($kind:ident, $id:ident, $wire:literal, $payload:ident, $answer:ty, $surfaces:expr) ),+
        $(,)?
    ) => {
        /// The identifier of one command or read, without its arguments.
        ///
        /// The registry columns hang here. A payload enum cannot host a
        /// `const ALL`. GENERATED from the `for_each_command!` list, over
        /// every row regardless of kind — `ALL` and its methods answer
        /// for a `Command` row and a `Query` row alike.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum CommandId {
            $(
                #[doc = concat!("The identifier of the `", $wire, "` row.")]
                $id,
            )+
        }

        impl CommandId {
            /// Every row, in registry order. GENERATED.
            pub const ALL: &[CommandId] = &[$(CommandId::$id,)+];

            /// The name this row carries on the wire. GENERATED.
            pub fn wire_name(self) -> &'static str {
                match self {
                    $(CommandId::$id => $wire,)+
                }
            }

            /// How the session runs this row. GENERATED.
            pub fn kind(self) -> CommandKind {
                match self {
                    $(CommandId::$id => CommandKind::$kind,)+
                }
            }

            /// Which surfaces reach this row, and why one does not.
            /// GENERATED.
            pub fn surfaces(self) -> Surfaces {
                match self {
                    $(CommandId::$id => $surfaces,)+
                }
            }
        }

        // The payload and answer enums need one row list per kind, so the
        // kind-agnostic block above hands the same rows to the splitter.
        split_command_rows! {
            @split
            commands = [],
            queries = [],
            jobs = [],
            $( ($kind, $id, $wire, $payload, $answer, $surfaces), )+
        }
    };
}
for_each_command!(define_command_registry);

/// What [`ProjectSession::start`] captured for one [`Job`].
///
/// The handle is the whole input of step (ii), so step (ii) needs no
/// session.
///
/// **Hand-written, not generated, and the one block here that is.** The
/// registry's answer column names what `execute_*` RETURNS, which is what
/// [`JobAnswer`] is built from; a handle is a different type, named
/// `<Id>Handle` by convention, and `macro_rules!` cannot build an
/// identifier by concatenation. The convention is held by the compiler
/// rather than by the macro: [`ProjectSession::start`] matches
/// exhaustively over [`Job`], so a row added with no variant here does not
/// compile.
///
/// The enum derives nothing. A handle owns a
/// [`ResolvedGenInputs`](super::ResolvedGenInputs), which publishes
/// neither equality nor a clone: a resolution is the evidence of one
/// submit, and a copy of it would be a second assembly of the same
/// inputs.
///
/// **Every variant boxes its handle, by the same rule [`JobAnswer`]
/// follows.** A handle owns a whole resolution and a whole machine,
/// stock and setup snapshot, and the three rows' handles are of very
/// different sizes; `clippy::large_enum_variant` is denied workspace-wide.
/// Boxing every variant means the enum's size never depends on which row
/// captured the most.
pub enum JobHandle {
    /// What the `generate_toolpath` job captured.
    GenerateToolpath(Box<GenerateToolpathHandle>),
    /// What the `recommend_clearing_strategy` job captured.
    RecommendClearingStrategy(Box<RecommendClearingStrategyHandle>),
    /// What the `preview_tier_map` job captured.
    PreviewTierMap(Box<PreviewTierMapHandle>),
    /// What the `optimize_toolpath` job captured. The heaviest handle
    /// here: it owns a whole cloned session, which is the reason the
    /// boxing rule is not optional.
    OptimizeToolpath(Box<OptimizeToolpathHandle>),
}

impl JobHandle {
    /// The identifier of the job this handle belongs to.
    pub fn id(&self) -> CommandId {
        match self {
            JobHandle::GenerateToolpath(_) => CommandId::GenerateToolpath,
            JobHandle::RecommendClearingStrategy(_) => CommandId::RecommendClearingStrategy,
            JobHandle::PreviewTierMap(_) => CommandId::PreviewTierMap,
            JobHandle::OptimizeToolpath(_) => CommandId::OptimizeToolpath,
        }
    }
}

impl ProjectSession {
    /// Apply one command, and report what it changed.
    ///
    /// This is the one door. Every surface that runs a command takes it,
    /// so every surface reads the same [`Effects`].
    ///
    /// The method measures `stale` generally. It reads every toolpath
    /// revision before the mutation, runs the mutation, and reports every
    /// index whose revision moved. It does not read the invalidation
    /// chain's own drop list.
    pub fn apply(&mut self, command: Command) -> Result<Effects, SessionError> {
        match command {
            Command::SetToolpathParam(args) => {
                let index = args.index;
                self.try_with_effects(Some(index), move |session| {
                    session.set_toolpath_param_impl(index, &args.param, args.value)
                })
            }
            Command::AdoptResult(args) => {
                let AdoptResultArgs {
                    index,
                    revision,
                    result,
                } = args;
                if index >= self.toolpath_count() {
                    return Err(SessionError::ToolpathNotFound(index));
                }
                let current = self.toolpath_revision(index);
                if current != revision {
                    return Err(SessionError::StaleCompletion {
                        index,
                        submitted: revision,
                        current,
                    });
                }
                self.try_with_effects(Some(index), move |session| {
                    session.insert_result(index, *result)?;
                    // D1. The source's OUTPUT moved: its rest regions may
                    // have appeared, changed or vanished, and a Rest
                    // consumer's predecessor now carries a different
                    // answer. Rules (b) and (c) only.
                    //
                    // `chain_seeds` is EMPTY: recording an answer removes
                    // no material, so rule (a) must not fire. Seeding it
                    // would drop every downstream `FromRemainingStock` op
                    // on every adopt, which refuses round 1 of a Generate
                    // All ladder with `StaleCompletion`.
                    //
                    // The call is `walk_output_dependents`, not the edit
                    // door: an adopt must NOT clear the simulation, or it
                    // destroys the `prior_stocks` snapshot the next op in
                    // the ladder reads.
                    let _ = session.walk_output_dependents(
                        std::collections::BTreeSet::from([index]),
                        std::collections::BTreeSet::new(),
                    );
                    Ok(())
                })
            }
            Command::AdoptSimulation(args) => {
                let AdoptSimulationArgs { result, epoch } = args;
                // D7. The same guard `AdoptResult` makes, for the same
                // reason: an edit that cleared the simulation moved the
                // epoch, so this run answers material the project no
                // longer cuts. `FromRemainingStock` reads its prior stock
                // from this field, so storing it would hand a rest
                // operation a superseded snapshot.
                let current = self.simulation_epoch();
                if current != epoch {
                    return Err(SessionError::StaleSimulation {
                        submitted: epoch,
                        current,
                    });
                }
                // The row names no toolpath, so `index` is `None`.
                //
                // `stale` reads empty: a simulation moves no
                // generation-input revision. `simulation_cleared` reads
                // `before && !after`, and the store leaves `after` as
                // `Some`, so it reads false whether or not the session
                // held a simulation already.
                Ok(self.with_effects(None, move |session| {
                    session.simulation = Some(*result);
                }))
            }
            // ── WP4: the MCP mutation section ────────────────────
            //
            // Every arm delegates to the session setter that already
            // owns the rule, and returns the `Effects` that setter
            // reports. No arm derives a stale set of its own: that is
            // the whole point of the move, and N15 measured what two
            // producers cost.
            Command::AddAlignmentPin(args) => {
                let added = self.add_alignment_pin(args.x, args.y, args.diameter);
                // A duplicate pin changes nothing, and "changed
                // nothing" is an empty `Effects`, not a refusal.
                Ok(match added {
                    Some(effects) => effects,
                    None => self.with_effects(None, |_| {}),
                })
            }
            Command::RemoveAlignmentPin(args) => self.remove_alignment_pin(args.index),
            Command::AddModel(args) => Ok(self.add_model(*args.model)),
            Command::AdoptModelGeometry(args) => {
                self.adopt_model_geometry(args.model_id, *args.geometry, args.units)
            }
            Command::AddSetup(args) => {
                let AddSetupArgs { name, face_up } = args;
                // One default name for every surface. The GUI used to
                // own this format and MCP renamed the setup afterwards
                // through a hatch.
                let name =
                    name.unwrap_or_else(|| format!("Setup {}", self.list_setups().len() + 1));
                Ok(self.add_setup(name, face_up))
            }
            Command::RemoveSetup(args) => self.remove_setup(args.setup_index),
            Command::SetSetupFace(args) => self.set_setup_face(args.setup_index, args.face_up),
            Command::SetSetupRotation(args) => {
                self.set_setup_rotation(args.setup_index, args.z_rotation)
            }
            Command::SetSetupName(args) => self.rename_setup(args.setup_index, args.name),
            Command::SetSetupDatum(args) => self.set_setup_datum(args.setup_index, args.datum),
            Command::SetSetupModels(args) => {
                self.set_setup_models(args.setup_index, args.model_ids)
            }
            Command::SetSetupPauseMessage(args) => {
                self.set_setup_pause_message(args.setup_index, args.message)
            }
            Command::MoveToolpathToSetup(args) => self.move_toolpath_to_setup(
                args.toolpath_index,
                args.target_setup_index,
                args.target_position,
            ),
            Command::SaveProject(args) => {
                self.try_with_effects(None, move |session| session.save(&args.path))
            }
            Command::SetToolParam(args) => {
                self.set_tool_param(args.index, &args.param, &args.value)
            }
            Command::ReplaceTool(args) => self.replace_tool(args.tool_id, *args.config),
            Command::ReplaceFixture(args) => {
                self.replace_fixture(args.setup_index, args.fixture_id, *args.fixture)
            }
            Command::ReplaceKeepOut(args) => {
                self.replace_keep_out(args.setup_index, args.zone_id, *args.zone)
            }
            Command::SetToolpathTool(args) => self.set_toolpath_tool(args.index, args.tool_id),
            Command::SetToolpathModel(args) => self.set_toolpath_model(args.index, args.model_id),
            Command::SetToolpathHeights(args) => self.set_heights_config(args.index, args.heights),
            Command::SetToolpathDebugOptions(args) => {
                self.set_toolpath_debug_options(args.index, args.debug_options)
            }
            Command::AddToolpath(args) => self.add_toolpath(args.setup_index, *args.config),
            Command::RemoveToolpath(args) => self.remove_toolpath(args.index),
            Command::AddTool(args) => Ok(self.add_tool(*args.tool)),
            Command::AddToolFromLibrary(args) => Ok(self.add_tool(*args.tool)),
            Command::RemoveTool(args) => self.remove_tool(args.index),
            Command::SetStockConfig(args) => Ok(self.set_stock_config(*args.stock)),
            Command::SetStockSource(args) => self.set_stock_source(args.index, args.source),
            Command::SetMachine(args) => Ok(self.set_machine(*args.machine)),
            Command::SetMachineKinematics(args) => {
                Ok(self.set_machine_kinematics(*args.kinematics))
            }
            Command::ImportMachineSettings(args) => {
                Ok(self.import_machine_settings(*args.kinematics, args.max_feed_mm_min))
            }
            Command::SetPostConfig(args) => Ok(self.set_post_config(*args.post)),
            Command::SetBoundaryConfig(args) => self.set_boundary_config(args.index, args.boundary),
            Command::SetRestAnalysisConfig(args) => {
                self.set_rest_analysis_config(args.index, args.rest_analysis)
            }
            Command::SetDressupConfig(args) => self.set_dressup_config(args.index, *args.dressups),
            Command::SetDressupField(args) => {
                self.set_dressup_field(args.index, &args.key, args.value)
            }
            Command::SetToolpathEnabled(args) => {
                self.set_toolpath_enabled(args.index, args.enabled)
            }
            // ── WP15a: the rows of the row-less setters ───────────
            //
            // §25 ruling 1. Every arm below delegates to the setter
            // that already owns the invalidation rule, exactly as the
            // WP4 section above does. No arm copies a rule, and no arm
            // derives a stale set of its own.
            Command::ReorderToolpath(args) => self.reorder_toolpath(args.from_index, args.to_index),
            Command::RemoveModel(args) => self.remove_model(args.index),
            Command::SetFaceSelection(args) => self.set_face_selection(args.index, args.face_ids),
            Command::SetAlignmentPinDrillHoles(args) => {
                self.set_alignment_pin_drill_holes(args.index, args.holes)
            }
            Command::SetDrillSelectedHoles(args) => {
                self.set_drill_selected_holes(args.index, args.selected_holes)
            }
            Command::AddFixture(args) => self.add_fixture(args.setup_index, *args.fixture),
            Command::RemoveFixture(args) => self.remove_fixture(args.setup_index, args.fixture_id),
            Command::AddKeepOut(args) => self.add_keep_out(args.setup_index, *args.zone),
            Command::RemoveKeepOut(args) => self.remove_keep_out(args.setup_index, args.zone_id),
            Command::AutoEnableRestAnalysis(args) => {
                let enabled = self.auto_enable_rest_analysis_for_source(args.source_id);
                // The setter reports `None` when the source already
                // reads rest regions, or when it is a rest-depth
                // pencil that attaches them itself. "Changed nothing"
                // is an empty `Effects`, not a refusal — the shape
                // `AddAlignmentPin` above uses.
                Ok(match enabled {
                    Some(effects) => effects,
                    None => self.with_effects(None, |_| {}),
                })
            }
            Command::ForgetResult(args) => Ok(self.remove_result(args.index)),
            Command::SetToolpathOperation(args) => {
                self.set_toolpath_operation(args.index, *args.operation)
            }
            Command::InvalidateStock(_) => Ok(self.invalidate_stock()),
            Command::InvalidateMachine(_) => Ok(self.invalidate_machine()),
            Command::InvalidateTool(args) => Ok(self.invalidate_tool(args.tool_id)),
            Command::InvalidateModel(args) => Ok(self.invalidate_model(args.model_id)),
            Command::InvalidateToolpathInputs(args) => {
                Ok(self.invalidate_toolpath_inputs(args.index))
            }
            Command::UpdateStockFromBbox(args) => Ok(self.update_stock_from_bbox(&args.bbox)),
            Command::ReplaceTools(args) => Ok(self.replace_tools(args.tools)),
            Command::SetFeedsProvenance(args) => {
                self.set_feeds_provenance(args.index, *args.feeds_provenance)
            }
            Command::RestoreToolpathSnapshot(args) => {
                let RestoreToolpathSnapshotArgs {
                    index,
                    operation,
                    dressups,
                    face_selection,
                    feeds_provenance,
                } = args;
                if index >= self.toolpath_count() {
                    return Err(SessionError::ToolpathNotFound(index));
                }
                Ok(self.with_effects(Some(index), move |session| {
                    if let Some(tc) = session.toolpath_configs.get_mut(index) {
                        tc.operation = *operation;
                        tc.dressups = *dressups;
                        tc.face_selection = face_selection;
                        if let Some(provenance) = feeds_provenance {
                            tc.feeds_provenance = *provenance;
                        }
                    }
                    // Unconditional by contract. The chain call runs
                    // whether or not the restore moved a field: a
                    // byte-identical restore still drops the chain
                    // (F2.5). The index is checked above, so the read
                    // below answers `Some`.
                    let enabled = session
                        .toolpath_configs
                        .get(index)
                        .is_some_and(|tc| tc.enabled);
                    session.invalidate_result_chain(index, enabled);
                }))
            }
            Command::ReplaceToolpathConfig(args) => {
                let ReplaceToolpathConfigArgs { index, config } = args;
                if index >= self.toolpath_count() {
                    return Err(SessionError::ToolpathNotFound(index));
                }
                Ok(self.with_effects(Some(index), move |session| {
                    let after = config.generation_inputs_signature();
                    let Some(slot) = session.toolpath_configs.get_mut(index) else {
                        return;
                    };
                    let before = slot.generation_inputs_signature();
                    // The write is unconditional. Name, coolant, the pre
                    // and post G-code and the debug options are outside
                    // the signature, so they must land on a frame that
                    // drops nothing.
                    *slot = *config;
                    if after == before {
                        return;
                    }
                    // A generation input moved, so the cached geometry
                    // answers a configuration that is gone. `enabled` is
                    // read after the write, because the replacement can
                    // move it and the chain walk reads the new value.
                    let enabled = session
                        .toolpath_configs
                        .get(index)
                        .is_some_and(|tc| tc.enabled);
                    session.invalidate_result_chain(index, enabled);
                }))
            }
        }
    }

    /// Run one read, and report its answer.
    ///
    /// This is the read door, alongside [`Self::apply`] for mutations. A
    /// query changes nothing, so it takes `&self`.
    pub fn query(&self, query: Query) -> Result<QueryAnswer, SessionError> {
        match query {
            Query::ToolpathCycleTime(args) => {
                let ToolpathCycleTimeArgs {
                    index,
                    trace,
                    cutting_distance_mm,
                    nominal_feed_mm_min,
                } = args;
                let id = self
                    .toolpath_configs()
                    .get(index)
                    .map(|tc| tc.id)
                    .ok_or(SessionError::ToolpathNotFound(index))?;
                // Both fields must be `Some`, or the `CuttingOnly` fallback
                // would divide a real feed by an unmeasured distance
                // coerced to zero — a clean-looking answer over a value
                // that was never measured. Zeroing both instead keeps the
                // fallback's own `nominal_feed_mm_min > 0.0` check honest:
                // it reads no evidence and answers `CycleTime::NONE`.
                let (cutting_distance_mm, nominal_feed_mm_min) =
                    match (cutting_distance_mm, nominal_feed_mm_min) {
                        (Some(distance), Some(feed)) => (distance, feed),
                        _ => (0.0, 0.0),
                    };
                let cycle_time = cycle_time::toolpath_cycle_time(
                    trace.as_deref(),
                    id,
                    cutting_distance_mm,
                    nominal_feed_mm_min,
                );
                Ok(QueryAnswer::ToolpathCycleTime(ToolpathCycleTimeAnswer {
                    cycle_time,
                }))
            }
            Query::GetOperationSchema(args) => {
                let GetOperationSchemaArgs { operation_type } = args;
                let schema = Self::operation_schema(&operation_type)?;
                Ok(QueryAnswer::GetOperationSchema(GetOperationSchemaAnswer {
                    schema,
                }))
            }
        }
    }

    /// Start one job, and report the handle its steps share.
    ///
    /// This is step (i), and it is the third door beside [`Self::apply`]
    /// and [`Self::query`]. A job runs three synchronous steps and never
    /// an `async fn`:
    ///
    /// 1. `start` holds `&mut self` and runs on the frame loop. It drops
    ///    the cached result, checks every precondition, resolves the
    ///    inputs and captures the session reads the work needs. A refusal
    ///    therefore appears at submit time.
    /// 2. A free function runs the work. It reads the handle and holds no
    ///    session, so it runs off the frame loop.
    ///    [`execute_job`](super::execute_job) is that function for the
    ///    `generate_toolpath` row.
    /// 3. `apply(Command::AdoptResult { .. })` records the answer at the
    ///    revision the handle carries. An edit between step (1) and step
    ///    (3) moves that revision and the adopt refuses.
    ///
    /// `cancel` reaches the resolver, which does real geometric work for
    /// one boundary source: a
    /// [`BoundarySource::PlannedTierRegions`](crate::compute::config::BoundarySource::PlannedTierRegions)
    /// boundary walks a full-grid tier map. A `start` that resolved that
    /// map under a flag of its own would make Cancel a lie for the whole
    /// of it.
    /// Three of the four rows leave the caller's session alone.
    /// `recommend_clearing_strategy` and `preview_tier_map` are READS:
    /// they mutate nothing, so their step (i) drops no result and moves
    /// no revision, and they carry no step (iii). `optimize_toolpath`
    /// WRITES, but it writes the handle's own CLONE of the session
    /// (§24 ruling 2): its candidate loop regenerates and re-simulates
    /// per candidate, so it cannot reduce its inputs to a capture list,
    /// and it carries no step (iii) either — the GUI modal applies a
    /// chosen candidate through `Command::RestoreToolpathSnapshot` on an
    /// operator click. `&mut self` is the door's signature, not a claim
    /// that every row writes the caller's session.
    pub fn start(&mut self, job: Job, cancel: &AtomicBool) -> Result<JobHandle, SessionError> {
        match job {
            Job::GenerateToolpath(args) => {
                let handle = self.start_generate_toolpath(args.index, cancel)?;
                Ok(JobHandle::GenerateToolpath(Box::new(handle)))
            }
            Job::RecommendClearingStrategy(args) => {
                let handle = self.capture_recommend_clearing_strategy(args.index, cancel)?;
                Ok(JobHandle::RecommendClearingStrategy(Box::new(handle)))
            }
            Job::PreviewTierMap(args) => {
                let handle = self.capture_preview_tier_map(&args.spec)?;
                Ok(JobHandle::PreviewTierMap(Box::new(handle)))
            }
            Job::OptimizeToolpath(args) => {
                let handle = self.capture_optimize_toolpath(args.index)?;
                Ok(JobHandle::OptimizeToolpath(Box::new(handle)))
            }
        }
    }

    /// Run one mutation and report what it changed.
    ///
    /// **The one construction site of [`Effects`].** The method reads
    /// every toolpath revision and the simulation before `mutate`, runs
    /// `mutate`, and reports the difference. A mutation therefore states
    /// what it changed by changing it, and no two mutations can carry two
    /// staleness models.
    ///
    /// `index` names the ONE toolpath the mutation is about, for
    /// [`Effects::revision`]. Pass `None` from a bulk mutation, from a
    /// setup-index mutation, and from a mutation that moves or removes
    /// the toolpath the index named. An index that names no toolpath
    /// after the mutation also reports `None`.
    ///
    /// An error from `mutate` propagates, and the method reports no
    /// effects.
    ///
    /// [`Effects::created`] is always `None` here. A mutation that
    /// creates something writes the field on the answer this method
    /// returns; the construction site itself cannot know what an
    /// arbitrary closure appended.
    pub(crate) fn try_with_effects<E>(
        &mut self,
        index: Option<usize>,
        mutate: impl FnOnce(&mut Self) -> Result<(), E>,
    ) -> Result<Effects, E> {
        let simulation_before = self.simulation.is_some();
        let revisions_before = self.revision_snapshot();
        mutate(self)?;
        Ok(Effects {
            stale: self.moved_revisions(&revisions_before),
            simulation_cleared: simulation_before && self.simulation.is_none(),
            revision: index
                .filter(|i| *i < self.toolpath_count())
                .map(|i| self.toolpath_revision(i)),
            created: None,
        })
    }

    /// Run one mutation that cannot fail, and report what it changed.
    ///
    /// The infallible half of [`Self::try_with_effects`], which builds
    /// the [`Effects`]. This method builds none of its own.
    pub(crate) fn with_effects(
        &mut self,
        index: Option<usize>,
        mutate: impl FnOnce(&mut Self),
    ) -> Effects {
        self.try_with_effects(index, |session| {
            mutate(session);
            Ok::<(), std::convert::Infallible>(())
        })
        // SAFETY: `Infallible` holds no value, so the closure below has
        // no case to answer. The compiler proves the error arm cannot
        // happen. This is not `unwrap`: nothing can panic here.
        .unwrap_or_else(|never| match never {})
    }

    /// Read every toolpath's generation-input revision, in index order.
    fn revision_snapshot(&self) -> Vec<u64> {
        (0..self.toolpath_count())
            .map(|index| self.toolpath_revision(index))
            .collect()
    }

    /// Every index whose revision differs from the snapshot.
    ///
    /// The walk covers both lengths, because a command that removes a
    /// toolpath shortens the list. An index the snapshot does not carry
    /// counts as moved.
    fn moved_revisions(&self, before: &[u64]) -> BTreeSet<usize> {
        let count = self.toolpath_count().max(before.len());
        let mut moved = BTreeSet::new();
        for index in 0..count {
            if before.get(index).copied() != Some(self.toolpath_revision(index)) {
                moved.insert(index);
            }
        }
        moved
    }
}
