//! Phase 0 — "Core and GUI export agree on coolant, RPM, tools, and datums".
//!
//! Plan: `planning/arch_consolidation_2026-09-09/PLAN.md` lines 76-131.
//! This file is a CHARACTERIZATION TEST. It changes no production code.
//! Test A states the properties the two doors must agree on and then
//! pins the whole byte string. Test B pins a divergence the plan calls
//! wrong. Read Test B's own doc before you treat its assertion as
//! desired behaviour.
//!
//! # The two doors
//!
//! - Core door: `rs_cam_core::gcode::export_gcode_checked(project,
//!   sim_trace, policy)` — `crates/rs_cam_core/src/gcode/mod.rs:300`.
//!   `ProjectSession::export_gcode_with_policy`
//!   (`crates/rs_cam_core/src/session/compute.rs:4290`) wraps that door
//!   and writes its string to a file. That wrapper forwards the
//!   session's own cut trace, so it matches this file's `None` only
//!   while the session holds no simulation — the state of this
//!   fixture. This file calls the string-returning door.
//! - GUI door: `export_gcode_from_session_with_policy(session, gui, sim,
//!   policy, stale)` — `crates/rs_cam_viz/src/io/export.rs:403`.
//!
//! # A third door exists and this file does not cover it
//!
//! The CLI job-file route builds its own `GcodePhase` values in
//! `crates/rs_cam_cli/src/main.rs:534-548` (per-setup arm) and
//! `crates/rs_cam_cli/src/main.rs:598-614` (single-file arm). That route
//! reads a job file, not a `ProjectSession`, so neither door above
//! serves it. It carries `coolant: phase.coolant`, it always passes
//! `pre_gcode: None`, `post_gcode: None` and
//! `controller_compensation: None`, and it takes no tool-load policy.
//! The two session-based CLI routes DO take the core door:
//! `rs_cam_cli project --emit-gcode`
//! (`crates/rs_cam_cli/src/project.rs:604`) and `rs_cam_cli run`
//! (`crates/rs_cam_cli/src/run.rs:173`).
//!
//! # The GUI-only stages this fixture neutralises
//!
//! The GUI door runs four stages the core door does not have. Each one
//! can move bytes, so the fixture holds each at its no-effect value. A
//! future byte diff is then attributable to one named stage.
//!
//! - Wizard overlay. The core door passes `WizardOverlay::default()`
//!   (`gcode/mod.rs:759`). The GUI door passes `overlay_for(session,
//!   gui)` (`io/export.rs:429-435`), which reads `session.wizard()`.
//!   The fixture never touches the wizard, so `WizardState::default()`
//!   (`session/wizard.rs:72`: `dry_run` false, every override `None`,
//!   `spindle_warmup_secs` 0) gives the default overlay.
//! - High-feedrate rewrite. `replace_rapids_with_feed` runs only when
//!   `gui.post.high_feedrate_mode` (`io/export.rs:438-440`). The
//!   fixture leaves it false, which is `ProjectPostConfig::default()`
//!   (`session/project_file.rs:158`).
//! - Result store and refusal. The GUI door refuses an enabled
//!   operation whose result is missing or stale (G-EXPORTSKIP,
//!   G-STALEXPORT). The fixture gives every enabled operation a core
//!   result and no `gui.toolpath_rt` entry, so `freshness` returns
//!   `Current` (`state/freshness.rs:78-96`) and the door emits the same
//!   toolpath the core door reads.
//! - Post store. The core door resolves the post from
//!   `project.post_config().format` (`gcode/mod.rs:316-318`). The GUI
//!   door resolves it from `gui.post.format` (`io/export.rs:410`). The
//!   fixture bridges the two with `GuiState::post_from_session`
//!   (`state/runtime.rs:357`), so both emit through the shipped GRBL
//!   post (`crates/rs_cam_core/posts/grbl.toml`), the `"grbl"` default
//!   token.
//!
//! Two more inputs are held equal by hand. The simulation trace is
//! absent on both sides: `None` for the core door, `SimulationState::
//! new()` for the GUI door. Both sides take one explicit
//! `ToolLoadExportPolicy` with `accept_unmodeled` and `accept_exceeded`
//! true. No simulation runs here, so every load criterion reads
//! `Unmodeled`. Another file covers the gate.
//!
//! # The emitter formats this file reads
//!
//! - Spindle speed. The preamble template ends `M3 S{spindle_rpm}`
//!   (`posts/grbl.toml`), and `Statement::SpindleSet` renders
//!   `M3 S{rpm}` (`gcode/emitter.rs:211-213`). GRBL declares no
//!   `max_rpm`, so `clamp_rpm` writes no warning comment here.
//! - Tool change. `PostDefinition::render_tool_change`
//!   (`gcode/post.rs:316-323`) builds the message
//!   `TOOL CHANGE: {label} [T{number}]` and substitutes it into the
//!   post's `tool_change` template. On GRBL that template is `M5`, the
//!   message comment, `M0`. The FIRST phase emits no tool change;
//!   `push_initial_tool_comment` announces its tool as
//!   `(LOAD: {label} [T{number}])`
//!   (`gcode/program_builder.rs:372-378`).
//! - Coolant. `CoolantMode::start_gcode` (`gcode/mod.rs:84-92`) gives
//!   `M7` for `Mist`, `M8` for `Flood`, `M7` and `M8` for `Both`. The
//!   program builder pushes the word as a `Statement::Raw` and closes
//!   an active mode with a bare `M9` (`program_builder.rs:155-158`,
//!   `:190-192`, `:199-201`, `:325-353`).
//!
//! Test B uses `Flood`, not `Mist`, for one measured reason. GRBL
//! declares `unsupported_mcodes = [6, 7]`, so `filter_raw`
//! (`gcode/emitter.rs:154-194`) replaces an `M7` line with
//! `(WARNING: M7 unsupported on GRBL; dropped: M7)`. A `Mist` setting
//! therefore reaches the GUI door and still emits no coolant word on
//! this post. That is a separate behaviour from the core door's drop,
//! and a probe that mixed the two would not name either.
//!
//! # The fixture
//!
//! One in-memory session. No generator runs and no simulation runs.
//!
//! - Stock 60 x 70 x 12 at origin (-20, -25, -12), `auto_from_model`
//!   false. The non-zero XY origin is what makes the identity setup's
//!   export datum shift non-zero. A `DatumConfig` edit does not do
//!   this: `FixedOffset` shifts nothing
//!   (`session/eval_context.rs:206-218`).
//! - Two tools with distinct display numbers and names, so a tool
//!   change is visible in the bytes. Neither number equals its config
//!   id plus one, so a regression that keys tool-change detection on
//!   the display number instead of the config id moves these bytes.
//! - Three toolpaths with distinct per-operation spindle overrides.
//!   Toolpaths 0 and 1 sit on the identity setup. Toolpath 2 sits on a
//!   `FaceUp::Bottom` setup, so the datum property has a second frame.
//! - Export datum shifts, read from `SetupEvalContext::
//!   export_datum_shift`: identity (+20, +25, 0) — the Z term is zero
//!   because `origin_z + z == 0` puts the stock top at world Z0 — and
//!   flipped (0, 0, -12). The sample toolpath spans X 0..10 in its own
//!   frame, so the identity phases emit X 20..30 and the flipped phase
//!   emits X 0..10.
//!
//! The fixture inserts every result LAST. A mutating method on
//! `ProjectSession` drops cached results (G-FRESHSTATE), so a mutation
//! after an insert would leave the GUI door refusing on a missing
//! result instead of measuring parity.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::PocketConfig;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::gcode::{CoolantMode, ToolLoadExportPolicy};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::make_test_flat;
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathComputeResult, ToolpathConfig};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::toolpath_spans::AnnotatedToolpath;
use rs_cam_viz::io::export::export_gcode_from_session_with_policy;
use rs_cam_viz::state::runtime::{GuiState, StaleResultPolicy};
use rs_cam_viz::state::simulation::SimulationState;

const STOCK_X: f64 = 60.0;
const STOCK_Y: f64 = 70.0;
const STOCK_Z: f64 = 12.0;
const ORIGIN_X: f64 = -20.0;
const ORIGIN_Y: f64 = -25.0;
const ORIGIN_Z: f64 = -12.0;

/// Cutting feed of every sample move (mm/min). One value, so an F-word
/// difference cannot hide inside this fixture.
const FEED: f64 = 600.0;

const ROUGH_RPM: u32 = 12_000;
const FINISH_RPM: u32 = 21_000;
const FLIP_RPM: u32 = 15_000;

const ROUGH_LABEL: &str = "Rough Top";
const FINISH_LABEL: &str = "Finish Top";
const FLIP_LABEL: &str = "Flip Pass";

const LABELS: [&str; 3] = [ROUGH_LABEL, FINISH_LABEL, FLIP_LABEL];

const ROUGH_TOOL_NAME: &str = "Rough End Mill";
const FINISH_TOOL_NAME: &str = "Finish Ball";

/// Display T-numbers. Neither equals its config id plus one, so
/// tool-change detection that keys on the display number instead of the
/// config id changes these bytes.
const ROUGH_TOOL_NUMBER: u32 = 3;
const FINISH_TOOL_NUMBER: u32 = 7;

// ── Fixture ───────────────────────────────────────────────────────

/// Three cutting moves at `FEED`, bracketed by rapids. The path spans
/// X 0..10 in its own frame.
fn sample_toolpath() -> Toolpath {
    let mut path = Toolpath::new();
    path.rapid_to(P3::new(0.0, 0.0, 5.0));
    path.feed_to(P3::new(10.0, 0.0, -1.0), FEED);
    path.feed_to(P3::new(10.0, 10.0, -1.0), FEED);
    path.feed_to(P3::new(0.0, 10.0, -1.0), FEED);
    path.rapid_to(P3::new(0.0, 10.0, 5.0));
    path
}

fn core_result() -> ToolpathComputeResult {
    let annotated = Arc::new(AnnotatedToolpath::new(sample_toolpath()));
    ToolpathComputeResult {
        op_data: rs_cam_core::drill_op::OpData::Toolpath(annotated),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
    }
}

fn pocket_op(rpm: u32) -> OperationConfig {
    let mut op = OperationConfig::Pocket(PocketConfig::default());
    op.set_spindle_rpm(Some(rpm));
    op
}

/// A `ToolpathConfig` with coolant `Off`. The caller sets a different
/// coolant BEFORE `add_toolpath` — see the module doc on ordering.
fn toolpath_config(name: &str, rpm: u32, tool_id: usize, model_id: usize) -> ToolpathConfig {
    ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: name.to_owned(),
        enabled: true,
        operation: pocket_op(rpm),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    }
}

fn cutter(tool_type: ToolType, name: &str, number: u32) -> ToolConfig {
    let mut config = ToolConfig::new_default(ToolId(0), tool_type);
    config.name = name.to_owned();
    config.tool_number = number;
    config
}

/// Build the shared fixture. `last_coolant` is the coolant mode of the
/// third toolpath, which is the last phase of the program.
fn build_state(last_coolant: CoolantMode) -> (ProjectSession, GuiState, SimulationState) {
    let mut session = ProjectSession::new_empty();
    session.set_name("phase 0 export parity".to_owned());
    session.set_stock_config(StockConfig {
        x: STOCK_X,
        y: STOCK_Y,
        z: STOCK_Z,
        origin_x: ORIGIN_X,
        origin_y: ORIGIN_Y,
        origin_z: ORIGIN_Z,
        auto_from_model: false,
        ..StockConfig::default()
    });

    let rough_bit = cutter(ToolType::EndMill, ROUGH_TOOL_NAME, ROUGH_TOOL_NUMBER);
    let finish_bit = cutter(ToolType::BallNose, FINISH_TOOL_NAME, FINISH_TOOL_NUMBER);
    let rough_idx = session.add_tool(rough_bit);
    let finish_idx = session.add_tool(finish_bit);
    let rough_tool = session.tools()[rough_idx].id.0;
    let finish_tool = session.tools()[finish_idx].id.0;

    let model_id = session.add_model(LoadedModel {
        id: 0,
        path: PathBuf::from("flat.stl"),
        name: "Flat".to_owned(),
        kind: Some(ModelKind::Stl),
        mesh: Some(Arc::new(make_test_flat(40.0))),
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    });

    let flipped = session.add_setup("Flip".to_owned(), FaceUp::Bottom);

    let rough = toolpath_config(ROUGH_LABEL, ROUGH_RPM, rough_tool, model_id);
    let finish = toolpath_config(FINISH_LABEL, FINISH_RPM, finish_tool, model_id);
    let mut flip = toolpath_config(FLIP_LABEL, FLIP_RPM, rough_tool, model_id);
    flip.coolant = last_coolant;

    session.add_toolpath(0, rough).expect("add rough");
    session.add_toolpath(0, finish).expect("add finish");
    session
        .add_toolpath(flipped, flip)
        .expect("add the flipped-setup toolpath");

    for index in 0..LABELS.len() {
        session
            .insert_result(index, core_result())
            .expect("insert core result");
    }

    let mut gui = GuiState::new();
    gui.post = GuiState::post_from_session(session.post_config());
    // The GUI gate reads the same absent trace the core door reads.
    gui.tool_load_overrides.accept_unmodeled = true;

    (session, gui, SimulationState::new())
}

fn policy() -> ToolLoadExportPolicy {
    ToolLoadExportPolicy {
        accept_unmodeled: true,
        accept_exceeded: true,
    }
}

fn core_export(session: &ProjectSession) -> String {
    rs_cam_core::gcode::export_gcode_checked(session, None, policy()).expect("core export")
}

fn gui_export(session: &ProjectSession, gui: &GuiState, sim: &SimulationState) -> String {
    export_gcode_from_session_with_policy(session, gui, sim, policy(), StaleResultPolicy::Refuse)
        .expect("gui export")
}

// ── Per-property extraction ───────────────────────────────────────

/// Every spindle word in emission order. The preamble and each
/// `SpindleSet` render `M3 S{rpm}` on a line of their own, so the order
/// is part of what the two doors must agree on.
fn spindle_words(gcode: &str) -> Vec<u32> {
    gcode
        .lines()
        .filter_map(|line| line.trim().strip_prefix("M3 S"))
        .filter_map(|rpm| rpm.trim().parse::<u32>().ok())
        .collect()
}

/// Every tool-change message line, in emission order. The line carries
/// the tool's display T-number and its name.
fn tool_change_lines(gcode: &str) -> Vec<String> {
    gcode
        .lines()
        .map(|line| line.trim())
        .filter(|line| line.starts_with("(TOOL CHANGE:"))
        .map(|line| line.to_owned())
        .collect()
}

/// True when any line of `gcode` is exactly `word`. The bare coolant
/// M-codes sit on their own lines, and an exact line match cannot
/// collide with the same text inside a comment or a coordinate.
fn has_line(gcode: &str, word: &str) -> bool {
    gcode.lines().any(|line| line.trim() == word)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Extents {
    x_min: f64,
    x_max: f64,
    y_min: f64,
    y_max: f64,
}

/// XY extents of every motion line inside the phase labelled `wanted`.
///
/// Adapted from `crates/rs_cam_core/tests/export_datum_setup_frame.rs`
/// to carry the full label set, because this fixture has three phases.
/// The phase comment is a whole line, and `sanitize_comment_text`
/// rewrites parentheses, so the match is on the trimmed line and no
/// label here contains a parenthesis.
fn phase_extents(gcode: &str, labels: &[&str], wanted: &str) -> Extents {
    let mut in_phase = false;
    let mut x_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    let mut seen = 0usize;

    for line in gcode.lines() {
        let trimmed = line.trim();
        let matched = labels.iter().find(|l| trimmed == format!("({l})"));
        if let Some(label) = matched {
            in_phase = *label == wanted;
            continue;
        }
        if !in_phase {
            continue;
        }
        if !trimmed.starts_with("G0 ") && !trimmed.starts_with("G1 ") {
            continue;
        }
        let mut saw_xy = false;
        for word in trimmed.split_whitespace() {
            let Some(rest) = word.strip_prefix('X').or_else(|| word.strip_prefix('Y')) else {
                continue;
            };
            let Ok(v) = rest.parse::<f64>() else { continue };
            if word.starts_with('X') {
                x_min = x_min.min(v);
                x_max = x_max.max(v);
            } else {
                y_min = y_min.min(v);
                y_max = y_max.max(v);
            }
            saw_xy = true;
        }
        if saw_xy {
            seen += 1;
        }
    }

    assert!(
        seen > 0,
        "phase '{wanted}' emitted no XY motion lines — the fixture is vacuous"
    );
    Extents {
        x_min,
        x_max,
        y_min,
        y_max,
    }
}

// ── Test A — the agreeing properties ──────────────────────────────

/// The core door and the GUI door emit the same bytes for a project
/// whose coolant is `Off` everywhere.
///
/// The non-vacuity block runs FIRST. It shows that the fixture drives
/// each property: three distinct spindle words, a tool change that
/// names the second tool, and an X literal that differs between the
/// identity setup and the flipped setup. Without that block a
/// byte-equality assertion passes on two empty programs.
///
/// The per-property block then compares each property between the two
/// strings. It runs before the byte comparison, so a failure names the
/// property instead of a diff offset.
#[test]
fn core_and_gui_export_are_byte_identical_on_the_agreeing_properties() {
    let (session, gui, sim) = build_state(CoolantMode::Off);
    let core = core_export(&session);
    let viz = gui_export(&session, &gui, &sim);

    // ── Non-vacuity ──
    let core_rpms = spindle_words(&core);
    assert!(
        core_rpms.contains(&ROUGH_RPM) && core_rpms.contains(&FINISH_RPM),
        "the fixture is vacuous on RPM: the core program must carry \
         both S{ROUGH_RPM} and S{FINISH_RPM}, got {core_rpms:?}"
    );
    assert!(
        core_rpms.contains(&FLIP_RPM),
        "the fixture is vacuous on the flipped setup's RPM: expected \
         S{FLIP_RPM}, got {core_rpms:?}"
    );

    let core_changes = tool_change_lines(&core);
    let expected_change = format!("(TOOL CHANGE: {FINISH_TOOL_NAME} [T{FINISH_TOOL_NUMBER}])");
    assert!(
        core_changes.contains(&expected_change),
        "the fixture is vacuous on tool changes: expected \
         {expected_change} in the core program, got {core_changes:?}"
    );

    let core_identity = phase_extents(&core, &LABELS, ROUGH_LABEL);
    let core_flipped = phase_extents(&core, &LABELS, FLIP_LABEL);
    assert!(
        core_identity.x_min > core_flipped.x_max,
        "the fixture is vacuous on the datum: the identity setup's \
         export shift is +{:.1} mm in X, so its phase must emit X above \
         the flipped phase's. Identity {:.3}..{:.3}, flipped \
         {:.3}..{:.3}",
        -ORIGIN_X,
        core_identity.x_min,
        core_identity.x_max,
        core_flipped.x_min,
        core_flipped.x_max
    );

    // ── Per property ──
    assert_eq!(
        core_rpms,
        spindle_words(&viz),
        "RPM: the two doors resolve the per-operation spindle override \
         differently. The core door reads \
         project.post_config().spindle_speed (gcode/mod.rs:361-364); \
         the GUI door reads gui.post.spindle_speed \
         (io/export.rs:331-335)."
    );
    assert_eq!(
        core_changes,
        tool_change_lines(&viz),
        "Tools: the two doors build PhaseTool differently. The core \
         door inlines it (gcode/mod.rs:347-358); the GUI door calls \
         phase_tool_for_export (io/export.rs:71-77)."
    );
    for label in LABELS {
        assert_eq!(
            phase_extents(&core, &LABELS, label),
            phase_extents(&viz, &LABELS, label),
            "Datum: phase '{label}' emits different XY through the two \
             doors. Both call export_datum_shift_for_toolpath and \
             toolpath_in_export_datum, so a difference here means the \
             doors read different toolpaths."
        );
    }

    // ── Bytes ──
    assert_eq!(
        core, viz,
        "The two export doors disagree byte for byte. The per-property \
         checks above passed, so RPM, tools and datums are NOT the \
         cause. Look at a GUI-only stage: the wizard overlay, the \
         high-feedrate rewrite, or which store the emitted toolpath \
         came from. This file's module doc says what each stage is held \
         at."
    );
}

// ── Test B — the pinned divergence ────────────────────────────────

/// The core door drops the operation's coolant setting. The GUI door
/// honours it.
///
/// CAUTION: this test PINS CURRENT BEHAVIOUR. It does not state desired
/// behaviour. `ToolpathConfig::coolant`
/// (`crates/rs_cam_core/src/session/mod.rs:769`) is a per-toolpath core
/// field, and both project-file loaders persist it. The GUI door reads
/// it (`io/export.rs:340`). The core door hardcodes `CoolantMode::Off`
/// (`gcode/mod.rs:367`), so one project emits coolant through one door
/// and not through the other.
///
/// The operator can make either of two rulings.
///
/// - Ruling (a): the core door honours `tc.coolant` at
///   `gcode/mod.rs:367`. This test then inverts into an equality, and
///   Test A absorbs coolant as a fourth agreeing property.
/// - Ruling (b): the field is dead, because no GUI control writes it
///   and MCP has no setter for it. The field and this test are then
///   deleted together.
///
/// Which routes drop the setting today: the core door itself, and
/// therefore `rs_cam_cli project --emit-gcode`
/// (`crates/rs_cam_cli/src/project.rs:604`) and `rs_cam_cli run`
/// (`crates/rs_cam_cli/src/run.rs:173`, through
/// `ProjectSession::export_gcode_with_policy`). The CLI job-file route
/// in this file's module doc keeps its own coolant and is a separate
/// question.
///
/// The fixture puts `Flood` on the LAST toolpath. The builder emits
/// `M8` inside that phase's tool-change block and closes the program
/// with a bare `M9` before the final retract, so both halves of the
/// coolant contract are visible. The module doc says why `Mist` is the
/// wrong probe on this post.
#[test]
fn core_export_drops_the_operations_coolant_setting() {
    let (session, gui, sim) = build_state(CoolantMode::Flood);
    let core = core_export(&session);
    let viz = gui_export(&session, &gui, &sim);

    assert!(
        has_line(&viz, "M8"),
        "the GUI door must emit the flood-coolant start word M8 for an \
         operation whose coolant is Flood (io/export.rs:340)"
    );
    assert!(
        has_line(&viz, "M9"),
        "the GUI door must close an active coolant mode with M9 \
         (program_builder.rs:199-201)"
    );

    assert!(
        !has_line(&core, "M8"),
        "PINNED DIVERGENCE: the core door hardcodes CoolantMode::Off \
         (gcode/mod.rs:367), so it emits no M8 today. If this line \
         fails, the core door now honours tc.coolant — that is ruling \
         (a) in this test's doc. Invert this test and move coolant into \
         Test A."
    );
    assert!(
        !has_line(&core, "M9"),
        "PINNED DIVERGENCE: with the coolant dropped the core door \
         opens no coolant mode, so it emits no closing M9 today. See \
         the M8 assertion above."
    );
}
