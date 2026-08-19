//! G-code generation and post-processors.
//!
//! Note: `let _ = write!(...)` is used throughout this module for writing to `String`.
//! Writing to `String` is infallible (only fails on OOM, which panics regardless),
//! so discarding the `Result` with `let _ =` is safe.

pub mod emitter;
pub mod ir;
mod modal;
pub mod post;
pub mod program_builder;
pub mod wizard_overlay;

pub use ir::{Program, ProgramMetadata, Statement};
pub use post::{
    ArcLinearize, CommentStyle, Decimals, Feedrate, LoadError as PostLoadError, PostDefinition,
    PostLimits, Rpm, SafeZ, Units, WcsCode,
};
pub use wizard_overlay::{ToolChangeMode, WizardOverlay};

use crate::compute::catalog::effective_spindle_rpm;
use crate::session::ProjectSession;
use crate::simulation_cut::SimulationCutTrace;
use crate::toolpath::Toolpath;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::{Display, Write};

/// Policy knobs reserved for tool-load-aware export checks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ToolLoadExportPolicy {
    pub accept_unmodeled: bool,
    pub accept_exceeded: bool,
}

/// Error returned by checked G-code export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportError {
    message: String,
}

impl ExportError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ExportError {}

/// Direction for G41/G42 controller cutter compensation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControllerCompensation {
    /// G41 — cutter compensation left.
    Left,
    /// G42 — cutter compensation right.
    Right,
}

/// Coolant mode for G-code output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoolantMode {
    /// No coolant (default).
    #[default]
    Off,
    /// Mist coolant (M7).
    Mist,
    /// Flood coolant (M8).
    Flood,
    /// Both mist and flood (M7 + M8).
    Both,
}

impl CoolantMode {
    /// Emit the G-code command(s) to activate this coolant mode.
    /// Returns an empty string for `Off`.
    pub fn start_gcode(self) -> &'static str {
        match self {
            CoolantMode::Off => "",
            CoolantMode::Mist => "M7\n",
            CoolantMode::Flood => "M8\n",
            CoolantMode::Both => "M7\nM8\n",
        }
    }

    /// Returns true if coolant is active (not Off).
    pub fn is_active(self) -> bool {
        self != CoolantMode::Off
    }
}

/// Emit G-code from a toolpath using the given post definition.
pub fn emit_gcode(toolpath: &Toolpath, post: &PostDefinition, spindle_rpm: u32) -> String {
    emitter::emit_program(&program_builder::build_single(toolpath, spindle_rpm), post)
}

/// Render a `Program` to g-code text using the given post definition.
///
/// Re-export of `emitter::emit_program` for backward-compatible call
/// sites; all formatting decisions live in the emitter module.
pub fn emit_program(program: &Program, post: &PostDefinition) -> String {
    emitter::emit_program(program, post)
}

/// The tool a phase cuts with, as seen by the g-code program builder.
///
/// `id` is the *identity* — the tool config id (`ToolConfig::id.0`).
/// Tool-change detection keys on it, NOT on the user-curated display
/// `number`: real projects carry T-number collisions across distinct
/// tools (WANAKA: "End Mill" and "Tapered Ball 2mm" both T1), which
/// silently suppressed the change when detection keyed on the number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhaseTool<'a> {
    /// Tool config id — the identity used for change detection.
    pub id: usize,
    /// Display T-number, substituted into the post's `tool_change`
    /// template (`M6 T{n}` / operator message).
    pub number: u32,
    /// Tool name for the operator message
    /// (`TOOL CHANGE: {label} [T{number}]`).
    pub label: &'a str,
}

/// A phase in a multi-operation job: toolpath + spindle speed + label.
pub struct GcodePhase<'a> {
    pub toolpath: &'a Toolpath,
    pub spindle_rpm: u32,
    pub label: &'a str,
    /// Raw G-code to emit before this toolpath's moves (user-defined).
    pub pre_gcode: Option<&'a str>,
    /// Raw G-code to emit after this toolpath's moves (user-defined).
    pub post_gcode: Option<&'a str>,
    /// The tool this phase cuts with. If `Some` and its `id` differs
    /// from the previous phase's, the post's tool-change template is
    /// emitted before this phase.
    pub tool: Option<PhaseTool<'a>>,
    /// Coolant mode for this phase. Coolant on/off commands are emitted
    /// when the mode changes between phases.
    pub coolant: CoolantMode,
    /// Controller cutter compensation (G41/G42). When `Some`, a G41/G42
    /// command is emitted before the first cutting move and G40 after the last.
    pub controller_compensation: Option<ControllerCompensation>,
}

/// Emit G-code from multiple phases, inserting tool changes, spindle speed
/// changes, and coolant commands between operations as needed.
///
/// Test-only shim around `emit_gcode_phased_with_overlay(_, _, &default)`.
/// Production callers go through `export_gcode_phases_(with_overlay_)checked`.
#[cfg(test)]
fn emit_gcode_phased(phases: &[GcodePhase<'_>], post: &PostDefinition) -> String {
    emit_gcode_phased_with_overlay(phases, post, &WizardOverlay::default())
}

/// Same as `emit_gcode_phased`, but applies a `WizardOverlay` (WCS, units,
/// spindle warmup) on top of the post's defaults. `WizardOverlay::default()`
/// is byte-identical to the no-overlay path.
pub(crate) fn emit_gcode_phased_with_overlay(
    phases: &[GcodePhase<'_>],
    post: &PostDefinition,
    overlay: &WizardOverlay,
) -> String {
    let mut program = program_builder::build_phased(phases);
    prepend_t_collision_warnings(&mut program, phases.iter(), post, overlay);
    emitter::emit_program_with_overlay(&program, post, overlay)
}

/// A7 — distinct tools sharing a display T-number on an M6 post: the
/// controller keys the physical swap on the T word, so `M6 T1` → `M6 T1`
/// silently skips the change. Detection keys on the *effective* post
/// (overlay tool-change override applied); pause-style posts name the
/// incoming tool in the operator message, so they are not affected.
fn prepend_t_collision_warnings<'a>(
    program: &mut Program,
    phases: impl Iterator<Item = &'a GcodePhase<'a>>,
    post: &PostDefinition,
    overlay: &WizardOverlay,
) {
    let effective_post = overlay.applied_post(post);
    if !effective_post.tool_change.contains("M6") {
        return;
    }
    // number → distinct tool config ids seen under that number.
    let mut by_number: std::collections::BTreeMap<u32, Vec<usize>> =
        std::collections::BTreeMap::new();
    for phase in phases {
        if let Some(tool) = phase.tool {
            let ids = by_number.entry(tool.number).or_default();
            if !ids.contains(&tool.id) {
                ids.push(tool.id);
            }
        }
    }
    let mut warnings: Vec<Statement> = Vec::new();
    for (number, ids) in &by_number {
        if ids.len() > 1 {
            warnings.push(Statement::Comment(format!(
                "WARNING: distinct tools share T{number} — M6 will not trigger a physical change"
            )));
        }
    }
    for (offset, w) in warnings.into_iter().enumerate() {
        program.statements.insert(offset, w);
    }
}

// ── Export datum (G-EXPORT-DATUM, 2026-08-19) ────────────────────────
//
// Every setup's G-code must express XY in ONE frame, or a two-sided job
// hands the operator two datums under a single `G54` and the second side
// machines off by the stock origin. The chosen frame is **stock-relative**:
// program X0 Y0 at the stock's min corner, which is what non-identity
// setups already emit and the frame `StockConfig::alignment_pins` — the
// feature that physically registers the flip — is dimensioned in.
//
// The stored/generated toolpath is NOT touched: this is a translation
// applied at emit time only, so the simulator, the viewport, screenshots
// and every metric keep reading the frame they were built against. (The
// separate G-SIM-IDENTITY-FRAME fix shifts the *simulation* stock by the
// same vector inside `compute::simulate`; the two never compose because
// neither writes back to the toolpath.)
//
// ── Why Z is NOT shifted ────────────────────────────────────────────
//
// The shift is XY-only, deliberately:
//
//  * XY is never re-zeroed between setups — the operator flips the part
//    against the same pins and keeps the same XY zero. A disagreement in
//    XY is therefore silent and fatal. Z *is* explicitly re-zeroed between
//    setups (the split-export header says so), so a per-file Z datum is
//    an instruction problem, not a registration problem. The fix for Z is
//    to NAME the datum in the header, which the split export now does.
//  * Shifting Z would move program Z0 to the stock's UNDERSIDE for every
//    identity setup. That breaks the repo's documented 2D convention —
//    `StockConfig::update_from_bbox` sets `origin_z = bbox.min.z - z` for
//    2D models precisely so the stock TOP sits at Z0 and 2D ops cut at
//    negative Z. Every 2D project would move from "zero to the top of the
//    stock" (self-correcting for actual stock thickness, and the near
//    universal router convention) to "zero to the spoilboard" (every cut
//    depth then carries the nominal-vs-actual thickness error).
//  * It buys nothing physical. The retract plane is already derived from
//    the LOCAL stock top (F-024, `SetupEvalContext::safe_z`), so a Z shift
//    would re-express the same physical height with a bigger number, not
//    change any motion.

/// Translation from the frame `toolpath_index`'s toolpath was generated
/// in to the shared export datum. See the module note above; XY only,
/// zero for non-identity setups, `-stock_bbox.min` in XY for identity
/// setups. A toolpath that belongs to no setup is treated as identity
/// (that is the frame the generator used for it).
pub fn export_datum_shift_for_toolpath(
    session: &ProjectSession,
    toolpath_index: usize,
) -> crate::geo::P3 {
    let setup = session.find_setup_for_toolpath_index(toolpath_index);
    crate::session::SetupEvalContext::build_for_setup(session, setup).export_datum_shift()
}

/// Apply an export-datum shift to a toolpath, borrowing unchanged when
/// the shift is zero (the non-identity case, and every zero-origin
/// project). Arc `i`/`j` are start→centre offsets, so a pure translation
/// leaves them alone.
pub fn toolpath_in_export_datum(toolpath: &Toolpath, shift: crate::geo::P3) -> Cow<'_, Toolpath> {
    if shift.x == 0.0 && shift.y == 0.0 && shift.z == 0.0 {
        return Cow::Borrowed(toolpath);
    }
    let mut shifted = toolpath.clone();
    for m in &mut shifted.moves {
        m.target = crate::geo::P3::new(
            m.target.x + shift.x,
            m.target.y + shift.y,
            m.target.z + shift.z,
        );
    }
    Cow::Owned(shifted)
}

/// Emit checked G-code from a project session.
pub fn export_gcode_checked(
    project: &ProjectSession,
    sim_trace: Option<&SimulationCutTrace>,
    policy: ToolLoadExportPolicy,
) -> Result<String, ExportError> {
    // Run the project-level tool-load report and gate on policy. All
    // three criteria (chipload, power, deflection) are fully evaluated;
    // a criterion that can't be modeled for this toolpath (no sim trace,
    // no vendor row, drill kinematics) returns `Unmodeled` and is gated
    // behind `policy.accept_unmodeled`.
    let report = project_load_report(project, sim_trace);

    // W9 / P-1: this used to open-code a token match that had no
    // `"grblhal"` arm, so a grblHAL project exported GRBL. One
    // resolver now; an unrecognised token still falls back to GRBL
    // (the historic behaviour) rather than refusing the export.
    let post_format =
        PostFormat::from_token(&project.post_config().format).unwrap_or(PostFormat::Grbl);
    let post = post_format.definition();

    // G-EXPORT-DATUM: re-express each toolpath in the shared export
    // datum BEFORE building phases, so the emitted program has one XY
    // zero across every setup. Owned here (not written back to the
    // session) so nothing downstream of generation is disturbed.
    let emitted: Vec<(usize, Cow<'_, Toolpath>)> = project
        .toolpath_configs()
        .iter()
        .enumerate()
        .filter_map(|(idx, _)| {
            let result = project.get_result(idx)?;
            let shift = export_datum_shift_for_toolpath(project, idx);
            Some((idx, toolpath_in_export_datum(result.toolpath(), shift)))
        })
        .collect();

    let phases: Vec<GcodePhase<'_>> = emitted
        .iter()
        .filter_map(|(idx, emitted_toolpath)| {
            let tc = project.toolpath_configs().get(*idx)?;
            // Pull tool identity (config id) + display number + name
            // from the matching tool config so the emitter can insert
            // the post's tool-change block between toolpaths that use
            // different tools. Change detection keys on the config id —
            // NOT the user-curated T-number, which collides in real
            // projects (WANAKA: two distinct tools both T1). The modal
            // layer suppresses duplicate changes for consecutive
            // same-tool phases, so this is always safe to populate.
            let tool = project
                .tools()
                .iter()
                .find(|t| t.id.0 == tc.tool_id)
                .map(|t| PhaseTool {
                    id: t.id.0,
                    number: t.tool_number,
                    label: t.name.as_str(),
                });
            Some(GcodePhase {
                toolpath: emitted_toolpath.as_ref(),
                spindle_rpm: effective_spindle_rpm(
                    &tc.operation,
                    project.post_config().spindle_speed,
                ),
                label: &tc.name,
                tool,
                coolant: CoolantMode::Off,
                pre_gcode: tc.pre_gcode.as_deref(),
                post_gcode: tc.post_gcode.as_deref(),
                controller_compensation: controller_comp_for_project_toolpath(tc),
            })
        })
        .collect();

    // The phase-level checked emit enforces `policy` against `report`
    // (C1, 2026-06-11) — the single enforcement chokepoint shared with
    // the viz / MCP / CLI callers.
    export_gcode_phases_checked(&phases, post, &report, policy)
}

/// Check whether the cached sim trace's provenance still matches the
/// project's current state. PR-4: this is the staleness signal that
/// drives [`UnmodeledReason::StaleSimulation`] emissions out of the
/// load gates — when current toolpaths or tools differ from the
/// trace's recorded hashes, the gates can no longer be honoured
/// against the current configs.
///
/// Returns `true` when the trace is safe to evaluate against. Traces
/// without a `provenance` block (pre-provenance schema) get the
/// benefit of the doubt — backward-compat with older traces — and
/// return `true`.
///
/// Cheap: only iterates enabled toolpaths and hashes the cached
/// annotated toolpath. Doesn't recompute anything heavy.
pub fn sim_trace_is_fresh(project: &ProjectSession, trace: &SimulationCutTrace) -> bool {
    let Some(provenance) = trace.provenance.as_ref() else {
        // No provenance → schema predates the freshness check. Keep
        // the legacy behaviour of treating it as fresh; the user can
        // still hit "Run simulation" to invalidate manually.
        return true;
    };
    for (idx, tc) in project.toolpath_configs().iter().enumerate() {
        if !tc.enabled {
            continue;
        }
        let Some(result) = project.get_result(idx) else {
            // A new toolpath that was never simulated — sim trace
            // covers fewer toolpaths than the project now has.
            // Treat as stale so the new toolpath's gates surface
            // as "needs current simulation".
            return false;
        };
        let expected = match provenance.toolpath_hashes.get(&tc.id) {
            Some(h) => *h,
            None => return false,
        };
        let actual = crate::compute::simulate::hash_toolpath(&result.annotated().toolpath);
        if expected != actual {
            return false;
        }
        // Config-level hash comparison — catches edits that don't
        // change move geometry (e.g. `feed_rate`, `plunge_rate`) but
        // do invalidate the cached load verdicts. Pre-PR-4 traces
        // have an empty `operation_config_hashes` map; treat a
        // missing entry as a config match for backward-compat.
        if let Some(expected_cfg) = provenance.operation_config_hashes.get(&tc.id) {
            let actual_cfg = crate::compute::simulate::hash_operation_config(&tc.operation);
            if *expected_cfg != actual_cfg {
                return false;
            }
        }
    }
    true
}

/// Rewrite a verdict's [`UnmodeledReason::SimulationRequired`] into
/// [`UnmodeledReason::StaleSimulation`]. Used after the load gates
/// run to convert "no trace" verdicts into "stale trace" verdicts
/// when the caller knows a trace exists but is stale.
fn rewrite_sim_required_to_stale_chipload(v: &mut crate::tool_load::ChiploadVerdict) {
    if let crate::tool_load::ChiploadVerdict::Unmodeled { reason } = v
        && matches!(
            reason,
            crate::tool_load::UnmodeledReason::SimulationRequired
        )
    {
        *reason = crate::tool_load::UnmodeledReason::StaleSimulation;
    }
}

fn rewrite_sim_required_to_stale_power(v: &mut crate::tool_load::PowerVerdict) {
    if let crate::tool_load::PowerVerdict::Unmodeled { reason } = v
        && matches!(
            reason,
            crate::tool_load::UnmodeledReason::SimulationRequired
        )
    {
        *reason = crate::tool_load::UnmodeledReason::StaleSimulation;
    }
}

fn rewrite_sim_required_to_stale_deflection(v: &mut crate::tool_load::verdict::DeflectionVerdict) {
    if let crate::tool_load::verdict::DeflectionVerdict::Unmodeled { reason } = v
        && matches!(
            reason,
            crate::tool_load::UnmodeledReason::SimulationRequired
        )
    {
        *reason = crate::tool_load::UnmodeledReason::StaleSimulation;
    }
}

/// Classification of the cached simulation trace's relationship to the
/// current project state. Resolves the ambiguity between "no trace ever
/// existed" (e.g. project just loaded) and "trace exists but is stale"
/// (e.g. user edited a toolpath since the last sim).
///
/// Down-stream gates use [`Self::effective_trace`] to decide whether
/// to evaluate against the cached trace; the diagnostics layer reads
/// [`Self::is_stale`] to know whether to rewrite
/// `Unmodeled::SimulationRequired` into `Unmodeled::StaleSimulation`.
#[derive(Debug, Clone, Copy)]
pub enum SimEvidenceMeta<'a> {
    /// No simulation trace has been cached on this evidence path.
    Missing,
    /// The cached trace's hashes match the current project state.
    Fresh(&'a SimulationCutTrace),
    /// The cached trace exists but the project has drifted since it
    /// was captured. Down-stream gates are evaluated *without* the
    /// trace so verdicts read as `Unmodeled::SimulationRequired`;
    /// the post-process step then promotes those to
    /// `Unmodeled::StaleSimulation`.
    Stale,
}

impl<'a> SimEvidenceMeta<'a> {
    /// Classify a trace against the current project state.
    pub fn resolve(project: &ProjectSession, trace: Option<&'a SimulationCutTrace>) -> Self {
        match trace {
            None => Self::Missing,
            Some(t) if sim_trace_is_fresh(project, t) => Self::Fresh(t),
            Some(_) => Self::Stale,
        }
    }

    /// The trace to feed to the load-gate evaluators. `None` for
    /// `Missing` and `Stale` — gates surface as
    /// `Unmodeled::SimulationRequired` and the stale-rewrite pass
    /// fixes that up afterwards.
    pub fn effective_trace(self) -> Option<&'a SimulationCutTrace> {
        match self {
            Self::Fresh(t) => Some(t),
            _ => None,
        }
    }

    /// True iff a stale trace was discarded — drives the
    /// `SimulationRequired` → `StaleSimulation` rewrite at the end of
    /// `project_load_report`.
    pub fn is_stale(self) -> bool {
        matches!(self, Self::Stale)
    }
}

/// Build a `ToolLoadReport` from a `ProjectSession`. Delegates each
/// toolpath to `tool_load::evaluate_toolpath` (the single verdict
/// assembly site), which runs all three criteria: `chipload` and
/// `power` (per-sample, require `sim_trace`) and `deflection`
/// (per-sample tip deflection from cutting force).
///
/// PR-4: when `sim_trace` is `Some` but `sim_trace_is_fresh` returns
/// false (project state has drifted since the trace was captured),
/// the evaluators run with `sim_trace = None` and the resulting
/// `Unmodeled::SimulationRequired` verdicts are post-processed into
/// `Unmodeled::StaleSimulation`. The diagnostics adapter renders
/// `StaleSimulation` as `DiagnosticState::StaleEvidence`, which the
/// UI displays as a neutral "re-run simulation" hint rather than a
/// scary warning.
pub fn project_load_report(
    project: &ProjectSession,
    sim_trace: Option<&SimulationCutTrace>,
) -> crate::tool_load::ToolLoadReport {
    use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
    use crate::feeds::{OperationFamily, PassRole};

    // Freshness gate — see [`sim_trace_is_fresh`] for the contract.
    let sim_evidence = SimEvidenceMeta::resolve(project, sim_trace);
    let sim_trace = sim_evidence.effective_trace();
    let sim_is_stale = sim_evidence.is_stale();

    let material = &project.stock_config().material;
    let mut per_toolpath = Vec::new();
    for (idx, tc) in project.toolpath_configs().iter().enumerate() {
        if !tc.enabled {
            continue;
        }
        let Some(tool_cfg) = project.get_tool(crate::compute::tool_config::ToolId(tc.tool_id))
        else {
            // Toolpath references a tool that's been removed — skip rather
            // than fabricate a verdict.
            continue;
        };
        // Spans live on the AnnotatedToolpath in the cached compute
        // result; absent if the toolpath hasn't been generated yet —
        // classifiers fall back to engagement-only labels in that case.
        // F2.2: spans invalidated by a transform (TSP split) are
        // corrupted ancestry — pass None so the gates fall back to the
        // per-sample `in_transit_span` flag instead of misclassifying.
        let spans: Option<&[crate::toolpath_spans::Span]> = project
            .get_result(idx)
            .filter(|r| r.annotated().spans_valid)
            .map(|r| r.annotated().spans.as_slice());
        let tool_def = crate::compute::cutter::build_cutter(tool_cfg);
        let spec = tc.operation.spec();
        let lut_op = match spec.feeds_family {
            OperationFamily::Adaptive => LutOperationFamily::Adaptive,
            OperationFamily::Pocket => LutOperationFamily::Pocket,
            OperationFamily::Contour => LutOperationFamily::Contour,
            OperationFamily::Parallel => LutOperationFamily::Parallel,
            OperationFamily::Scallop => LutOperationFamily::Scallop,
            OperationFamily::Trace => LutOperationFamily::Trace,
            OperationFamily::Face => LutOperationFamily::Face,
            OperationFamily::Drill => LutOperationFamily::Drill,
        };
        let lut_pass = match spec.feeds_pass_role {
            PassRole::Roughing => LutPassRole::Roughing,
            PassRole::SemiFinish => LutPassRole::SemiFinish,
            PassRole::Finish => LutPassRole::Finish,
        };
        let machine = project.machine();
        // Item C: pass the operation's commanded feed rate so the
        // chipload guardrail can filter out transient (plunge/ramp/entry)
        // samples and only measure steady-state cutting against the LUT.
        let operation_feed_rate_mm_min = tc.operation.feed_rate();
        // gcode export keeps strict LUT/machine-ceiling behaviour — only
        // the optimizer routes wider tolerance bands. See `ToleranceBands`.
        let strict_tolerance = crate::tool_load::ToleranceBands::default();
        // §6.E / Step 3 PR2: when the toolpath is a drill op, thread the
        // `DrillOp` payload into the context so `evaluate_toolpath` runs
        // the drill-specific gates. The existing chipload / power /
        // deflection gates remain `Unmodeled(NotApplicableForOp)` for
        // these toolpaths — drill gates supplement, not replace, that
        // signal.
        let drill_op = project.get_result(idx).and_then(|r| r.drill_op());
        // Phase 6 task 2: delegate verdict assembly to
        // `tool_load::evaluate_toolpath` — the single assembly site
        // shared with the optimizer — instead of re-assembling the
        // struct inline (the inline copy had already diverged on
        // `modulation_summary`).
        let load_ctx = crate::tool_load::ToolpathLoadContext {
            toolpath_id: tc.id,
            tool: &tool_def,
            material,
            operation_family: lut_op,
            pass_role: lut_pass,
            operation_feed_rate_mm_min,
            operation_kind: tc.operation.op_type(),
            spans,
            drill_op: drill_op.map(|arc| arc.as_ref()),
        };
        per_toolpath.push(crate::tool_load::evaluate_toolpath(
            &load_ctx,
            sim_trace,
            Some(machine),
            &strict_tolerance,
        ));
    }
    // PR-4: if we threw away a stale trace upstream, rewrite the
    // resulting `SimulationRequired` verdicts to `StaleSimulation`
    // so the diagnostics adapter surfaces them as
    // `DiagnosticState::StaleEvidence` ("re-run sim") instead of
    // `NeedsSimulation` ("never simulated").
    if sim_is_stale {
        for verdict in &mut per_toolpath {
            rewrite_sim_required_to_stale_chipload(&mut verdict.chipload);
            rewrite_sim_required_to_stale_power(&mut verdict.power);
            rewrite_sim_required_to_stale_deflection(&mut verdict.deflection);
        }
    }

    crate::tool_load::ToolLoadReport { per_toolpath }
}

/// Enforce a `ToolLoadExportPolicy` against a report. Returns a structured
/// error message naming each offending toolpath and criterion so the user
/// (or UI) sees exactly what to override.
pub fn enforce_load_policy(
    report: &crate::tool_load::ToolLoadReport,
    policy: &ToolLoadExportPolicy,
) -> Result<(), ExportError> {
    if !policy.accept_exceeded {
        let exceeded = report.exceeded_criteria();
        if !exceeded.is_empty() {
            let mut msg =
                String::from("G-code export refused: tool load exceeded on toolpath(s):\n");
            for (id, crits) in &exceeded {
                let reason_list: Vec<String> = crits
                    .iter()
                    .map(|ec| format!("{}={}", ec.label, ec.reason_label))
                    .collect();
                let _ = writeln!(msg, "  toolpath {id}: {}", reason_list.join(", "));
            }
            msg.push_str(
                "Pass `accept_exceeded=true` to override (this is a known-dangerous override).",
            );
            return Err(ExportError::new(msg));
        }
    }
    if !policy.accept_unmodeled && report.any_unmodeled() {
        // Distinguish "stale simulation" (re-sim required) from
        // "never simulated / no LUT data" — they're different user
        // actions even though both block export.
        let any_stale = report.per_toolpath.iter().any(|v| {
            matches!(
                &v.chipload,
                crate::tool_load::ChiploadVerdict::Unmodeled {
                    reason: crate::tool_load::UnmodeledReason::StaleSimulation
                }
            ) || matches!(
                &v.power,
                crate::tool_load::PowerVerdict::Unmodeled {
                    reason: crate::tool_load::UnmodeledReason::StaleSimulation
                }
            ) || matches!(
                &v.deflection,
                crate::tool_load::DeflectionVerdict::Unmodeled {
                    reason: crate::tool_load::UnmodeledReason::StaleSimulation
                }
            )
        });
        let headline = if any_stale {
            "G-code export refused: cached simulation is stale — re-run simulation \
             to verify load gates against current toolpath state.\n"
        } else {
            "G-code export refused: tool load not fully modeled for toolpath(s):\n"
        };
        let mut msg = String::from(headline);
        for v in &report.per_toolpath {
            if v.any_unmodeled() {
                let mut crits = Vec::new();
                if let crate::tool_load::ChiploadVerdict::Unmodeled { reason } = &v.chipload {
                    crits.push(format!("chipload={reason:?}"));
                }
                if let crate::tool_load::PowerVerdict::Unmodeled { reason } = &v.power {
                    crits.push(format!("power={reason:?}"));
                }
                if let crate::tool_load::DeflectionVerdict::Unmodeled { reason } = &v.deflection {
                    crits.push(format!("deflection={reason:?}"));
                }
                let _ = writeln!(msg, "  toolpath {}: {}", v.toolpath_id, crits.join(", "));
            }
        }
        if any_stale {
            msg.push_str(
                "Run simulation, then export again. Pass `accept_unmodeled=true` to \
                 export against the stale evidence anyway.",
            );
        } else {
            msg.push_str("Pass `accept_unmodeled=true` to acknowledge unmodeled criteria.");
        }
        return Err(ExportError::new(msg));
    }
    Ok(())
}

/// Emit checked G-code from pre-built phases.
///
/// `report` is the tool-load report the gate enforces `policy` against
/// (C1, 2026-06-11). It is deliberately **non-optional** so no caller
/// can silently skip the gate: callers with a `ProjectSession` build it
/// via [`project_load_report`]; callers without load-evaluation context
/// (the CLI job-file path) must pass an explicitly empty report
/// (`ToolLoadReport { per_toolpath: vec![] }`), which documents at the
/// call site that no evaluation was performed.
pub fn export_gcode_phases_checked(
    phases: &[GcodePhase<'_>],
    post: &PostDefinition,
    report: &crate::tool_load::ToolLoadReport,
    policy: ToolLoadExportPolicy,
) -> Result<String, ExportError> {
    export_gcode_phases_with_overlay_checked(
        phases,
        post,
        report,
        policy,
        &WizardOverlay::default(),
    )
}

/// Same as `export_gcode_phases_checked`, plus a `WizardOverlay` applied
/// to the emit step. Default overlay is byte-identical to the no-overlay
/// path (Cow::Borrowed both ways).
///
/// Enforces, in order:
/// 1. effective units must be mm (A1 — inch output is a cosmetic G20
///    today; refusing beats a silent 25.4× error),
/// 2. `policy` against `report` (C1 — the tool-load gate).
pub fn export_gcode_phases_with_overlay_checked(
    phases: &[GcodePhase<'_>],
    post: &PostDefinition,
    report: &crate::tool_load::ToolLoadReport,
    policy: ToolLoadExportPolicy,
    overlay: &WizardOverlay,
) -> Result<String, ExportError> {
    refuse_inch_units(post, overlay)?;
    enforce_load_policy(report, &policy)?;
    Ok(emit_gcode_phased_with_overlay(phases, post, overlay))
}

/// A1 — inch output is not implemented: a units override to `Inch` only
/// swaps the modal word (G21 → G20) while every coordinate stays in
/// millimeters, a silent 25.4× scale error. Hard-refuse until a real
/// conversion lands (backlogged).
fn refuse_inch_units(post: &PostDefinition, overlay: &WizardOverlay) -> Result<(), ExportError> {
    let effective_units = overlay.units_override.unwrap_or(post.units);
    if effective_units == Units::Inch {
        return Err(ExportError::new(
            "inch output not yet supported — coordinates are millimeters; \
             set units back to mm (G21) to export",
        ));
    }
    Ok(())
}

fn controller_comp_for_project_toolpath(
    tc: &crate::session::ToolpathConfig,
) -> Option<ControllerCompensation> {
    use crate::compute::{CompensationType, OperationConfig};
    use crate::profile::ProfileSide;

    if let OperationConfig::Profile(ref cfg) = tc.operation
        && cfg.compensation == CompensationType::InControl
    {
        return Some(match (cfg.side, cfg.climb) {
            (ProfileSide::Outside, true) => ControllerCompensation::Right,
            (ProfileSide::Outside, false) => ControllerCompensation::Left,
            (ProfileSide::Inside, true) => ControllerCompensation::Left,
            (ProfileSide::Inside, false) => ControllerCompensation::Right,
        });
    }
    None
}

/// A group of toolpath phases belonging to one setup.
pub struct GcodeSetupPhase<'a> {
    pub setup_label: &'a str,
    pub phases: Vec<GcodePhase<'a>>,
    /// Optional override for the M0 pause message emitted between this setup
    /// and the previous one (i.e. shown by g-Sender / UGS / CNCjs as the
    /// pause prompt). `None` falls back to `Setup change: <setup_label>`.
    pub pause_message: Option<&'a str>,
}

/// Emit checked G-code for multiple setups with M0 pauses between them.
///
/// See [`export_gcode_phases_checked`] for the `report` contract — it is
/// non-optional by design so the tool-load gate cannot be skipped.
pub fn export_gcode_multi_setup_checked(
    setups: &[GcodeSetupPhase<'_>],
    post: &PostDefinition,
    safe_z: f64,
    report: &crate::tool_load::ToolLoadReport,
    policy: ToolLoadExportPolicy,
) -> Result<String, ExportError> {
    export_gcode_multi_setup_with_overlay_checked(
        setups,
        post,
        safe_z,
        report,
        policy,
        &WizardOverlay::default(),
    )
}

/// Same as `export_gcode_multi_setup_checked`, plus a `WizardOverlay`.
/// Enforces the same gates as [`export_gcode_phases_with_overlay_checked`].
pub fn export_gcode_multi_setup_with_overlay_checked(
    setups: &[GcodeSetupPhase<'_>],
    post: &PostDefinition,
    safe_z: f64,
    report: &crate::tool_load::ToolLoadReport,
    policy: ToolLoadExportPolicy,
    overlay: &WizardOverlay,
) -> Result<String, ExportError> {
    refuse_inch_units(post, overlay)?;
    enforce_load_policy(report, &policy)?;
    Ok(emit_gcode_multi_setup_with_overlay(
        setups, post, safe_z, overlay,
    ))
}

/// Emit G-code for multiple setups with M0 pauses between them.
///
/// Test-only shim around `emit_gcode_multi_setup_with_overlay(_, _, _, &default)`.
/// Production callers go through `export_gcode_multi_setup_(with_overlay_)checked`.
#[cfg(test)]
fn emit_gcode_multi_setup(
    setups: &[GcodeSetupPhase<'_>],
    post: &PostDefinition,
    safe_z: f64,
) -> String {
    emit_gcode_multi_setup_with_overlay(setups, post, safe_z, &WizardOverlay::default())
}

/// Same as `emit_gcode_multi_setup`, but applies a `WizardOverlay`. The
/// overlay's `safe_z_override` (when set) replaces the `safe_z` argument
/// for the inter-setup retract; WCS/units/warmup apply as in
/// `emit_gcode_phased_with_overlay`.
pub(crate) fn emit_gcode_multi_setup_with_overlay(
    setups: &[GcodeSetupPhase<'_>],
    post: &PostDefinition,
    safe_z: f64,
    overlay: &WizardOverlay,
) -> String {
    let effective_safe_z = overlay.safe_z_override.unwrap_or(safe_z);
    let mut program = program_builder::build_multi_setup(setups, effective_safe_z);
    prepend_t_collision_warnings(
        &mut program,
        setups.iter().flat_map(|s| s.phases.iter()),
        post,
        overlay,
    );
    emitter::emit_program_with_overlay(&program, post, overlay)
}

/// Replace G0 rapid moves with G1 at a high feedrate.
/// Used for machines with unpredictable rapid behavior (e.g., GRBL "dogleg" rapids).
///
/// The inserted feed is clamped to `post.limits.max_feed` when set, and
/// the rewrite is a no-op when `high_feedrate <= 0` (a `F0.0` word would
/// stall the machine).
///
/// Known limits (sound for emitter output, documented for user snippets):
/// - lines after the final `M5` are rewritten too — since the postamble
///   safe-Z retract moved *before* `M5` (C2, 2026-06-11) the shipped
///   posts have no bare `G0` after `M5`, but custom postambles might;
/// - `G00` / lowercase `g0` spellings in user pre/post snippets are not
///   recognized and pass through unchanged.
pub fn replace_rapids_with_feed(gcode: &str, high_feedrate: f64, post: &PostDefinition) -> String {
    if high_feedrate <= 0.0 {
        return gcode.to_owned();
    }
    let feed = post
        .limits
        .max_feed
        .map_or(high_feedrate, |max| high_feedrate.min(max.get()));
    let mut output = String::with_capacity(gcode.len());
    for line in gcode.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("G0 ") || trimmed.starts_with("G0X") {
            // Replace G0 with G1 and append feedrate
            let rest = trimmed.get(2..).unwrap_or("");
            let _ = writeln!(output, "G1{rest} F{feed:.1}");
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    output
}

/// Post-processor format selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostFormat {
    Grbl,
    /// `rename_all = "snake_case"` serialises this as `"grbl_hal"`,
    /// while the project-file writer spells it `"grblhal"`
    /// ([`PostFormat::to_token`]). The alias makes the typed serde wire
    /// (the viz fallback schema) accept the project-file spelling too,
    /// so a grblHAL project cannot be rejected by whichever loader
    /// happens to read it. Serialisation output is unchanged.
    #[serde(alias = "grblhal")]
    GrblHal,
    #[serde(alias = "linuxcnc")]
    LinuxCnc,
    Mach3,
}

impl PostFormat {
    /// All available post-processor formats.
    pub const ALL: &[PostFormat] = &[
        PostFormat::Grbl,
        PostFormat::GrblHal,
        PostFormat::LinuxCnc,
        PostFormat::Mach3,
    ];

    /// Human-readable display label for UI.
    pub fn label(self) -> &'static str {
        match self {
            PostFormat::Grbl => "GRBL",
            PostFormat::GrblHal => "grblHAL",
            PostFormat::LinuxCnc => "LinuxCNC",
            PostFormat::Mach3 => "Mach3",
        }
    }

    /// Returns the shipped `PostDefinition` for this dialect.
    pub fn definition(self) -> &'static PostDefinition {
        match self {
            PostFormat::Grbl => post::grbl(),
            PostFormat::GrblHal => post::grblhal(),
            PostFormat::LinuxCnc => post::linuxcnc(),
            PostFormat::Mach3 => post::mach3(),
        }
    }

    /// The canonical token this format is written as in a project /
    /// job file. Every writer must go through here — `to_token` and
    /// [`Self::from_token`] are the only two halves of the on-disk
    /// spelling, so they cannot drift apart the way four hand-written
    /// `match` arms did (W9 / P-1).
    pub fn to_token(self) -> &'static str {
        match self {
            PostFormat::Grbl => "grbl",
            PostFormat::GrblHal => "grblhal",
            PostFormat::LinuxCnc => "linuxcnc",
            PostFormat::Mach3 => "mach3",
        }
    }

    /// **The** resolver for a post token read off disk or off a CLI
    /// flag. Case-insensitive; accepts the underscore spellings as
    /// aliases of the canonical [`Self::to_token`] output.
    ///
    /// W9 / P-1: three production readers used to open-code this match
    /// and only one of them knew `"grblhal"` — the other two fell
    /// through to `Grbl`, so selecting grblHAL in the GUI, saving and
    /// reloading silently downgraded both the dropdown and the emitted
    /// G-code dialect. Anything that turns a string into a
    /// `PostFormat` must call this rather than re-derive it. Returning
    /// `None` (rather than defaulting) is deliberate: the caller
    /// chooses whether an unknown token is a fallback or an error.
    pub fn from_token(name: &str) -> Option<PostFormat> {
        match name.trim().to_lowercase().as_str() {
            "grbl" => Some(PostFormat::Grbl),
            "grblhal" | "grbl_hal" => Some(PostFormat::GrblHal),
            "linuxcnc" | "linux_cnc" => Some(PostFormat::LinuxCnc),
            "mach3" => Some(PostFormat::Mach3),
            _ => None,
        }
    }
}

/// Get a `PostDefinition` by name (CLI / config-string lookup).
pub fn get_post_definition(name: &str) -> Option<&'static PostDefinition> {
    PostFormat::from_token(name).map(PostFormat::definition)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::geo::P3;
    use crate::ids::ToolpathId;
    use crate::toolpath::Toolpath;

    /// "No load evaluation performed" report — what callers without a
    /// `ProjectSession` (CLI job path, fixture captures) pass.
    fn empty_report() -> crate::tool_load::ToolLoadReport {
        crate::tool_load::ToolLoadReport {
            per_toolpath: vec![],
        }
    }

    #[test]
    fn checked_phased_export_is_byte_identical_to_legacy_emitter() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp1.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));
        tp2.feed_to(P3::new(30.0, 0.0, 0.0), 800.0);

        let phases = vec![
            GcodePhase {
                toolpath: &tp1,
                spindle_rpm: 18_000,
                label: "Op 0 — pocket",
                pre_gcode: Some("M8"),
                post_gcode: Some("M9"),
                tool: Some(PhaseTool {
                    id: 1,
                    number: 1,
                    label: "Tool One",
                }),
                coolant: CoolantMode::Mist,
                controller_compensation: None,
            },
            GcodePhase {
                toolpath: &tp2,
                spindle_rpm: 16_000,
                label: "Op 1 — profile",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 2,
                    number: 2,
                    label: "Tool Two",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
            },
        ];

        let legacy = emit_gcode_phased(&phases, post::grbl());
        let checked = export_gcode_phases_checked(
            &phases,
            post::grbl(),
            &empty_report(),
            ToolLoadExportPolicy::default(),
        )
        .expect("checked export should succeed");

        assert_eq!(checked, legacy);
    }

    #[test]
    fn checked_multi_setup_export_is_byte_identical_to_legacy_emitter() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp1.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));
        tp2.feed_to(P3::new(30.0, 0.0, 0.0), 800.0);

        let setups = vec![
            GcodeSetupPhase {
                setup_label: "Top",
                phases: vec![GcodePhase {
                    toolpath: &tp1,
                    spindle_rpm: 18_000,
                    label: "Top pocket",
                    pre_gcode: None,
                    post_gcode: None,
                    tool: Some(PhaseTool {
                        id: 1,
                        number: 1,
                        label: "Tool One",
                    }),
                    coolant: CoolantMode::Off,
                    controller_compensation: None,
                }],
                pause_message: None,
            },
            GcodeSetupPhase {
                setup_label: "Bottom",
                phases: vec![GcodePhase {
                    toolpath: &tp2,
                    spindle_rpm: 16_000,
                    label: "Bottom profile",
                    pre_gcode: None,
                    post_gcode: None,
                    tool: Some(PhaseTool {
                        id: 2,
                        number: 2,
                        label: "Tool Two",
                    }),
                    coolant: CoolantMode::Flood,
                    controller_compensation: None,
                }],
                pause_message: None,
            },
        ];

        let legacy = emit_gcode_multi_setup(&setups, post::grbl(), 15.0);
        let checked = export_gcode_multi_setup_checked(
            &setups,
            post::grbl(),
            15.0,
            &empty_report(),
            ToolLoadExportPolicy::default(),
        )
        .expect("checked export should succeed");

        assert_eq!(checked, legacy);
    }

    #[test]
    fn test_grbl_gcode() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);
        tp.feed_to(P3::new(20.0, 0.0, 0.0), 1000.0);

        let gcode = emit_gcode(&tp, post::grbl(), 18000);

        assert!(gcode.contains("G17 G21 G90"));
        assert!(gcode.contains("M3 S18000"));
        assert!(gcode.contains("G0 X0.000 Y0.000 Z10.000"));
        assert!(gcode.contains("G1 X10.000 Y0.000 Z0.000 F1000"));
        // Second G1 should omit F (modal)
        assert!(gcode.contains("G1 X20.000 Y0.000 Z0.000\n"));
        assert!(gcode.contains("M5"));
        assert!(gcode.contains("M30"));
    }

    #[test]
    fn test_mach3_gcode() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let gcode = emit_gcode(&tp, post::mach3(), 18000);

        assert!(gcode.contains("G17 G21 G90"));
        assert!(gcode.contains("G4 P2"), "Mach3 should have spindle dwell");
        assert!(gcode.contains("G28 G91 Z0"), "Mach3 should have G28 return");
        assert!(gcode.contains("M30"));
    }

    #[test]
    fn test_grbl_arc_gcode() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(10.0, 0.0, -3.0));
        tp.arc_cw_to(P3::new(0.0, 10.0, -3.0), -10.0, 0.0, 1000.0);
        tp.arc_ccw_to(P3::new(-10.0, 0.0, -3.0), 0.0, 10.0, 1000.0);

        let gcode = emit_gcode(&tp, post::grbl(), 18000);

        assert!(gcode.contains("G2 X0.000 Y10.000 Z-3.000 I-10.000 J0.000 F1000"));
        assert!(gcode.contains("G3 X-10.000 Y0.000 Z-3.000 I0.000 J10.000 F1000"));
    }

    #[test]
    fn test_get_post_definition() {
        assert!(get_post_definition("grbl").is_some());
        assert!(get_post_definition("grblhal").is_some());
        assert!(get_post_definition("grbl_hal").is_some());
        assert!(get_post_definition("linuxcnc").is_some());
        assert!(get_post_definition("linux_cnc").is_some());
        assert!(get_post_definition("mach3").is_some());
        assert!(get_post_definition("unknown").is_none());
    }

    /// W9 / P-1. Every token this repo writes must resolve back to the
    /// format that wrote it. `PostFormat::ALL` drives the loop so a
    /// fifth dialect cannot be added with a `to_token` arm and no
    /// `from_token` arm — the exact asymmetry that lost grblHAL.
    #[test]
    fn every_post_token_round_trips_through_the_resolver() {
        for &format in PostFormat::ALL {
            let token = format.to_token();
            assert_eq!(
                PostFormat::from_token(token),
                Some(format),
                "{token:?} did not resolve back to {format:?}"
            );
            assert_eq!(
                get_post_definition(token).map(|d| d.name.as_str()),
                Some(format.definition().name.as_str()),
                "{token:?} resolved to the wrong post definition"
            );
        }
    }

    /// The resolver is case- and whitespace-insensitive, and the
    /// underscore spellings stay accepted as aliases.
    #[test]
    fn the_post_resolver_accepts_the_alias_spellings() {
        assert_eq!(
            PostFormat::from_token("  GRBLHAL "),
            Some(PostFormat::GrblHal)
        );
        assert_eq!(
            PostFormat::from_token("grbl_hal"),
            Some(PostFormat::GrblHal)
        );
        assert_eq!(
            PostFormat::from_token("LinuxCNC"),
            Some(PostFormat::LinuxCnc)
        );
        assert_eq!(PostFormat::from_token("cobalt"), None);
    }

    /// The typed serde wire (the viz fallback project schema) must
    /// accept the project-file spelling as well as its own
    /// `snake_case` output, or a grblHAL project written by the primary
    /// writer is unreadable by the fallback loader.
    #[test]
    fn the_typed_post_wire_accepts_both_spellings() {
        assert_eq!(
            serde_json::from_str::<PostFormat>("\"grblhal\"").ok(),
            Some(PostFormat::GrblHal)
        );
        assert_eq!(
            serde_json::from_str::<PostFormat>("\"grbl_hal\"").ok(),
            Some(PostFormat::GrblHal)
        );
        assert_eq!(
            serde_json::from_str::<PostFormat>("\"linuxcnc\"").ok(),
            Some(PostFormat::LinuxCnc)
        );
    }

    #[test]
    fn test_emit_gcode_phased_same_spindle() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp1.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));
        tp2.feed_to(P3::new(30.0, 0.0, 0.0), 800.0);

        let phases = vec![
            GcodePhase {
                toolpath: &tp1,
                spindle_rpm: 18000,
                label: "Op 0 — pocket",
                pre_gcode: None,
                post_gcode: None,
                tool: None,
                coolant: CoolantMode::Off,
                controller_compensation: None,
            },
            GcodePhase {
                toolpath: &tp2,
                spindle_rpm: 18000,
                label: "Op 1 — profile",
                pre_gcode: None,
                post_gcode: None,
                tool: None,
                coolant: CoolantMode::Off,
                controller_compensation: None,
            },
        ];
        let gcode = emit_gcode_phased(&phases, post::grbl());

        // Should have one preamble with M3 S18000
        assert!(gcode.contains("M3 S18000"));
        // Should have both operations' comments
        assert!(gcode.contains("(Op 0"));
        assert!(gcode.contains("(Op 1"));
        // No extra spindle speed change since both are 18000
        let m3_count = gcode.matches("M3 S").count();
        assert_eq!(m3_count, 1, "Same spindle speed should not emit extra M3");
    }

    #[test]
    fn test_emit_gcode_phased_different_spindle() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp1.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));
        tp2.feed_to(P3::new(30.0, 0.0, 0.0), 800.0);

        let phases = vec![
            GcodePhase {
                toolpath: &tp1,
                spindle_rpm: 18000,
                label: "Op 0 — rough",
                pre_gcode: None,
                post_gcode: None,
                tool: None,
                coolant: CoolantMode::Off,
                controller_compensation: None,
            },
            GcodePhase {
                toolpath: &tp2,
                spindle_rpm: 24000,
                label: "Op 1 — finish",
                pre_gcode: None,
                post_gcode: None,
                tool: None,
                coolant: CoolantMode::Off,
                controller_compensation: None,
            },
        ];
        let gcode = emit_gcode_phased(&phases, post::grbl());

        // Should have preamble M3 S18000 + later M3 S24000
        assert!(gcode.contains("M3 S18000"));
        assert!(gcode.contains("M3 S24000"));
        let m3_count = gcode.matches("M3 S").count();
        assert_eq!(
            m3_count, 2,
            "Different spindle speeds should emit M3 for each"
        );
    }

    #[test]
    fn per_op_spindle_rpm_drives_phase_emission() {
        // End-to-end: building each `GcodePhase`'s `spindle_rpm` via
        // `effective_spindle_rpm` should emit per-toolpath M3 S<rpm>
        // commands in the resulting G-code. TP0 carries an override of
        // 12000; TP1 falls back to the project default of 18000.
        use crate::compute::catalog::{OperationConfig, effective_spindle_rpm};
        use crate::compute::operation_configs::PocketConfig;

        let mut op_override = OperationConfig::Pocket(PocketConfig::default());
        op_override.set_spindle_rpm(Some(12_000));
        let op_default = OperationConfig::Pocket(PocketConfig::default());
        let project_default_rpm: u32 = 18_000;

        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp1.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));
        tp2.feed_to(P3::new(30.0, 0.0, 0.0), 800.0);

        let phases = vec![
            GcodePhase {
                toolpath: &tp1,
                spindle_rpm: effective_spindle_rpm(&op_override, project_default_rpm),
                label: "TP0 — override",
                pre_gcode: None,
                post_gcode: None,
                tool: None,
                coolant: CoolantMode::Off,
                controller_compensation: None,
            },
            GcodePhase {
                toolpath: &tp2,
                spindle_rpm: effective_spindle_rpm(&op_default, project_default_rpm),
                label: "TP1 — default",
                pre_gcode: None,
                post_gcode: None,
                tool: None,
                coolant: CoolantMode::Off,
                controller_compensation: None,
            },
        ];
        let gcode = emit_gcode_phased(&phases, post::grbl());

        assert!(
            gcode.contains("M3 S12000"),
            "TP0's override should produce M3 S12000:\n{gcode}"
        );
        assert!(
            gcode.contains("M3 S18000"),
            "TP1 should fall back to project default 18000:\n{gcode}"
        );
    }

    #[test]
    fn test_program_pause_stops_and_pauses() {
        let pause = post::grbl().render_program_pause("Rotate stock");
        assert!(pause.contains("M5"));
        assert!(pause.contains("(Rotate stock)"));
        assert!(pause.contains("M0"));
    }

    #[test]
    fn test_emit_gcode_multi_setup_inserts_pause_between_setups() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp1.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));
        tp2.feed_to(P3::new(30.0, 0.0, 0.0), 800.0);

        let setups = vec![
            GcodeSetupPhase {
                setup_label: "Top",
                phases: vec![GcodePhase {
                    toolpath: &tp1,
                    spindle_rpm: 18_000,
                    label: "Pocket",
                    pre_gcode: None,
                    post_gcode: None,
                    tool: None,
                    coolant: CoolantMode::Off,
                    controller_compensation: None,
                }],
                pause_message: None,
            },
            GcodeSetupPhase {
                setup_label: "Bottom",
                phases: vec![GcodePhase {
                    toolpath: &tp2,
                    spindle_rpm: 24_000,
                    label: "Profile",
                    pre_gcode: None,
                    post_gcode: None,
                    tool: None,
                    coolant: CoolantMode::Off,
                    controller_compensation: None,
                }],
                pause_message: None,
            },
        ];

        let gcode = emit_gcode_multi_setup(&setups, post::grbl(), 15.0);

        assert!(gcode.contains("(=== Top ===)"));
        assert!(gcode.contains("(=== Bottom ===)"));
        assert!(gcode.contains("G0 Z15.000"));
        assert!(gcode.contains("(Setup change: Bottom)"));
        assert!(gcode.contains("M0"));
        assert!(gcode.contains("M3 S18000"));
        assert!(gcode.contains("M3 S24000"));
    }

    #[test]
    fn test_pre_post_gcode_emitted_in_phased() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let phases = vec![GcodePhase {
            toolpath: &tp,
            spindle_rpm: 18000,
            label: "Test Op",
            pre_gcode: Some("G55\nG10 L20 P2 X0 Y0 Z0"),
            post_gcode: Some("M9"),
            tool: None,
            coolant: CoolantMode::Off,
            controller_compensation: None,
        }];
        let gcode = emit_gcode_phased(&phases, post::grbl());

        // pre_gcode should appear after the label comment, before moves
        let label_pos = gcode.find("(Test Op)").expect("label comment");
        let pre_pos = gcode.find("G55").expect("pre_gcode G55");
        let move_pos = gcode.find("G0 X0.000").expect("first rapid move");
        let post_pos = gcode.find("M9").expect("post_gcode M9");
        let postamble_pos = gcode.find("M30").expect("postamble");

        assert!(label_pos < pre_pos, "pre_gcode should follow label comment");
        assert!(pre_pos < move_pos, "pre_gcode should precede moves");
        assert!(move_pos < post_pos, "post_gcode should follow moves");
        assert!(
            post_pos < postamble_pos,
            "post_gcode should precede postamble"
        );

        // Verify multi-line pre_gcode is emitted correctly
        assert!(gcode.contains("G10 L20 P2 X0 Y0 Z0"));
    }

    #[test]
    fn test_pre_post_gcode_emitted_in_multi_setup() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let setups = vec![GcodeSetupPhase {
            setup_label: "Top",
            phases: vec![GcodePhase {
                toolpath: &tp,
                spindle_rpm: 18_000,
                label: "Pocket",
                pre_gcode: Some("G55"),
                post_gcode: Some("M9"),
                tool: None,
                coolant: CoolantMode::Off,
                controller_compensation: None,
            }],
            pause_message: None,
        }];

        let gcode = emit_gcode_multi_setup(&setups, post::grbl(), 15.0);

        let label_pos = gcode.find("(Pocket)").expect("label comment");
        let pre_pos = gcode.find("G55").expect("pre_gcode G55");
        let move_pos = gcode.find("G0 X0.000").expect("first rapid move");
        let post_pos = gcode.find("M9").expect("post_gcode M9");

        assert!(label_pos < pre_pos);
        assert!(pre_pos < move_pos);
        assert!(move_pos < post_pos);
    }

    #[test]
    fn test_empty_pre_post_gcode_omitted() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));

        let phases = vec![GcodePhase {
            toolpath: &tp,
            spindle_rpm: 18000,
            label: "Empty",
            pre_gcode: Some(""),
            post_gcode: None,
            tool: None,
            coolant: CoolantMode::Off,
            controller_compensation: None,
        }];
        let gcode = emit_gcode_phased(&phases, post::grbl());

        // Should not have extra blank lines from empty pre/post
        let lines: Vec<&str> = gcode.lines().collect();
        assert!(
            !lines.iter().any(|l| l.is_empty()),
            "No blank lines from empty pre/post gcode"
        );
    }

    #[test]
    fn test_m6_tool_change_between_phases() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp1.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));
        tp2.feed_to(P3::new(30.0, 0.0, 0.0), 800.0);

        let phases = vec![
            GcodePhase {
                toolpath: &tp1,
                spindle_rpm: 18000,
                label: "Op 0 — 6mm endmill",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 1,
                    number: 1,
                    label: "Tool One",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
            },
            GcodePhase {
                toolpath: &tp2,
                spindle_rpm: 24000,
                label: "Op 1 — 3mm ballnose",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 2,
                    number: 2,
                    label: "Tool Two",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
            },
        ];
        // LinuxCNC's post implements M6 natively, so its tool_change
        // template is the classic M5 + M6 pair.
        let gcode = emit_gcode_phased(&phases, post::linuxcnc());

        // M6 T2 should appear between the two phases
        assert!(
            gcode.contains("M6 T2"),
            "Should emit M6 T2 for tool change to tool 2"
        );
        // M5 should appear before the tool change (spindle stop)
        let m5_pos = gcode.find("M5\nM6").expect("M5 before M6");
        let m6_pos = gcode.find("M6 T2").expect("M6 T2");
        assert!(m5_pos < m6_pos, "M5 should precede M6");

        // M3 S24000 should appear after the tool change (spindle restart)
        let m3_pos = gcode[m6_pos..].find("M3 S24000").expect("M3 after M6");
        assert!(m3_pos > 0, "M3 should follow M6");

        // No M6 T1 — first tool is assumed already loaded
        assert!(
            !gcode.contains("M6 T1"),
            "First tool should not emit M6 (already loaded)"
        );
    }

    #[test]
    fn test_grbl_tool_change_is_pause_not_m6() {
        // Vanilla Grbl rejects M6 (error:20) — its tool_change template
        // is spindle-off + operator message + M0 pause; the SpindleSet
        // right after doubles as the resume spin-up.
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp1.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));
        tp2.feed_to(P3::new(30.0, 0.0, 0.0), 800.0);

        let phases = vec![
            GcodePhase {
                toolpath: &tp1,
                spindle_rpm: 18000,
                label: "Op 0",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 1,
                    number: 1,
                    label: "End Mill",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
            },
            GcodePhase {
                toolpath: &tp2,
                spindle_rpm: 10610,
                label: "Op 1",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 2,
                    number: 2,
                    label: "Tapered Ball 2mm",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
            },
        ];
        let gcode = emit_gcode_phased(&phases, post::grbl());

        assert!(
            !gcode.contains("M6"),
            "Grbl post must not emit M6 (error:20 on vanilla GRBL):\n{gcode}"
        );
        let msg = "(TOOL CHANGE: Tapered Ball 2mm [T2])";
        let msg_pos = gcode.find(msg).expect("operator message");
        let pause_pos = gcode[msg_pos..].find("M0\n").expect("M0 after message") + msg_pos;
        let spinup_pos = gcode[pause_pos..]
            .find("M3 S10610")
            .expect("resume spin-up after pause")
            + pause_pos;
        assert!(msg_pos < pause_pos && pause_pos < spinup_pos);
        // Spindle off precedes the message.
        let m5_pos = gcode[..msg_pos].rfind("M5\n").expect("M5 before message");
        assert!(m5_pos < msg_pos);
    }

    #[test]
    fn test_no_m6_when_same_tool() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));

        let phases = vec![
            GcodePhase {
                toolpath: &tp1,
                spindle_rpm: 18000,
                label: "Op 0",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 1,
                    number: 1,
                    label: "Tool One",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
            },
            GcodePhase {
                toolpath: &tp2,
                spindle_rpm: 18000,
                label: "Op 1",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 1,
                    number: 1,
                    label: "Tool One",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
            },
        ];
        let gcode = emit_gcode_phased(&phases, post::linuxcnc());

        assert!(!gcode.contains("M6"), "Same tool id should not emit M6");
    }

    #[test]
    fn test_tool_change_fires_on_id_despite_colliding_numbers() {
        // WANAKA regression: two DISTINCT tools both curated as T1.
        // Detection keyed on the display number suppressed the change;
        // keyed on the config id it must fire.
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp1.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));
        tp2.feed_to(P3::new(30.0, 0.0, 0.0), 800.0);

        let phases = vec![
            GcodePhase {
                toolpath: &tp1,
                spindle_rpm: 12194,
                label: "Rough",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 1,
                    number: 1,
                    label: "End Mill",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
            },
            GcodePhase {
                toolpath: &tp2,
                spindle_rpm: 10610,
                label: "Finish",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 2,
                    number: 1, // SAME display number, different tool
                    label: "Tapered Ball 2mm",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
            },
        ];

        // M6-style post: change fires with the (colliding) display T1.
        let lcnc = emit_gcode_phased(&phases, post::linuxcnc());
        assert!(
            lcnc.contains("M6 T1"),
            "tool change must fire on id change even when T-numbers collide:\n{lcnc}"
        );

        // Pause-style post: operator message names the incoming tool.
        let grbl = emit_gcode_phased(&phases, post::grbl());
        assert!(
            grbl.contains("(TOOL CHANGE: Tapered Ball 2mm [T1])"),
            "pause-style change must fire on id change:\n{grbl}"
        );
    }

    #[test]
    fn test_coolant_mist_m7() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);

        let phases = vec![GcodePhase {
            toolpath: &tp,
            spindle_rpm: 18000,
            label: "Mist coolant op",
            pre_gcode: None,
            post_gcode: None,
            tool: None,
            coolant: CoolantMode::Mist,
            controller_compensation: None,
        }];
        let gcode = emit_gcode_phased(&phases, post::grbl());

        assert!(gcode.contains("M7"), "Mist coolant should emit M7");
        assert!(gcode.contains("M9"), "Should emit M9 before postamble");

        // M7 should appear before moves, M9 before postamble
        let m7_pos = gcode.find("M7").expect("M7");
        let move_pos = gcode.find("G0 X0.000").expect("move");
        let m9_pos = gcode.find("M9").expect("M9");
        let m5_pos = gcode.rfind("M5").expect("M5 in postamble");
        assert!(m7_pos < move_pos, "M7 should precede moves");
        assert!(m9_pos < m5_pos, "M9 should precede postamble M5");
    }

    #[test]
    fn test_coolant_flood_m8() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));

        let phases = vec![GcodePhase {
            toolpath: &tp,
            spindle_rpm: 18000,
            label: "Flood coolant op",
            pre_gcode: None,
            post_gcode: None,
            tool: None,
            coolant: CoolantMode::Flood,
            controller_compensation: None,
        }];
        let gcode = emit_gcode_phased(&phases, post::grbl());

        assert!(gcode.contains("M8"), "Flood coolant should emit M8");
        assert!(gcode.contains("M9"), "Should emit M9 before postamble");
    }

    #[test]
    fn test_coolant_both_m7_m8() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));

        let phases = vec![GcodePhase {
            toolpath: &tp,
            spindle_rpm: 18000,
            label: "Both coolant op",
            pre_gcode: None,
            post_gcode: None,
            tool: None,
            coolant: CoolantMode::Both,
            controller_compensation: None,
        }];
        let gcode = emit_gcode_phased(&phases, post::grbl());

        assert!(gcode.contains("M7"), "Both mode should emit M7");
        assert!(gcode.contains("M8"), "Both mode should emit M8");
        assert!(gcode.contains("M9"), "Should emit M9 before postamble");
    }

    #[test]
    fn test_coolant_off_no_commands() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));

        let phases = vec![GcodePhase {
            toolpath: &tp,
            spindle_rpm: 18000,
            label: "No coolant",
            pre_gcode: None,
            post_gcode: None,
            tool: None,
            coolant: CoolantMode::Off,
            controller_compensation: None,
        }];
        let gcode = emit_gcode_phased(&phases, post::grbl());

        assert!(!gcode.contains("M7"), "Off should not emit M7");
        assert!(!gcode.contains("M8"), "Off should not emit M8");
        assert!(!gcode.contains("M9"), "Off should not emit M9");
    }

    #[test]
    fn test_tool_change_with_coolant() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));

        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));

        let phases = vec![
            GcodePhase {
                toolpath: &tp1,
                spindle_rpm: 18000,
                label: "Op 0",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 1,
                    number: 1,
                    label: "Tool One",
                }),
                coolant: CoolantMode::Flood,
                controller_compensation: None,
            },
            GcodePhase {
                toolpath: &tp2,
                spindle_rpm: 24000,
                label: "Op 1",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 2,
                    number: 2,
                    label: "Tool Two",
                }),
                coolant: CoolantMode::Mist,
                controller_compensation: None,
            },
        ];
        // LinuxCNC: M6-style tool change AND native M7 mist (Grbl's post
        // filters M7 via unsupported_mcodes).
        let gcode = emit_gcode_phased(&phases, post::linuxcnc());

        // Should have M8 for first phase, M9 before tool change, M6 T2, M3, M7 for second
        assert!(gcode.contains("M8"), "First phase should have M8 flood");
        assert!(gcode.contains("M6 T2"), "Should have tool change");
        assert!(gcode.contains("M7"), "Second phase should have M7 mist");
        // M9 should appear before M6 (coolant off before tool change)
        let m9_pos = gcode.find("M9").expect("M9");
        let m6_pos = gcode.find("M6 T2").expect("M6 T2");
        assert!(
            m9_pos < m6_pos,
            "M9 should precede M6 (coolant off before tool change)"
        );
    }

    // ----- enforce_load_policy negative tests -----

    use crate::tool_load::verdict::{
        ChipBounds, ChipBoundsSource, ChiploadMetric, ChiploadStatistic, DeflectionBounds,
        SampleEvidence,
    };
    use crate::tool_load::{
        ChiploadVerdict, Confidence, DeflectionVerdict, PowerVerdict, ToolLoadReport,
        ToolpathLoadVerdict, UnmodeledReason,
    };

    fn report_with(
        chipload: ChiploadVerdict,
        power: PowerVerdict,
        deflection: DeflectionVerdict,
    ) -> ToolLoadReport {
        ToolLoadReport {
            per_toolpath: vec![ToolpathLoadVerdict {
                toolpath_id: ToolpathId(4),
                chipload,
                power,
                deflection,
                drill_gates: None,
                modulation_summary: None,
                feed_explanation: None,
            }],
        }
    }

    fn chip_bounds() -> ChipBounds {
        ChipBounds {
            min_mm_per_tooth: Some(0.038),
            max_mm_per_tooth: 0.07,
            source: ChipBoundsSource::VendorLut,
        }
    }

    fn chipload_within(peak: f64) -> ChiploadVerdict {
        ChiploadVerdict::Within {
            approach_to_min: None,
            approach_to_max: ChiploadMetric {
                observed_mm_per_tooth: peak,
                statistic: ChiploadStatistic::PeakInRange,
                evidence: SampleEvidence::empty(),
                bounds: chip_bounds(),
            },
            confidence: Confidence::Validated,
            entry_spikes: Vec::new(),
            burn_advisory: None,
            ceiling_advisory: None,
        }
    }

    fn chipload_unmodeled(reason: UnmodeledReason) -> ChiploadVerdict {
        ChiploadVerdict::Unmodeled { reason }
    }

    fn power_not_implemented() -> PowerVerdict {
        PowerVerdict::Unmodeled {
            reason: UnmodeledReason::NotImplemented("phase 1b".to_owned()),
        }
    }

    fn deflection_bounds_default() -> DeflectionBounds {
        DeflectionBounds {
            validated_within_mm: 0.050,
            exceeds_mm: 0.200,
        }
    }

    fn deflection_within(peak_mm: f64) -> DeflectionVerdict {
        DeflectionVerdict::Within {
            peak_mm,
            bounds: deflection_bounds_default(),
            evidence: SampleEvidence::empty(),
            confidence: Confidence::Validated,
            entry_spike: None,
        }
    }

    fn deflection_exceeds(peak_mm: f64) -> DeflectionVerdict {
        DeflectionVerdict::Exceeds {
            peak_mm,
            bounds: deflection_bounds_default(),
            evidence: SampleEvidence::at(0),
            confidence: Confidence::Validated,
        }
    }

    #[test]
    fn enforce_blocks_exceeded_by_default() {
        let report = report_with(
            chipload_within(0.05),
            power_not_implemented(),
            deflection_exceeds(10.0),
        );
        let policy = ToolLoadExportPolicy::default();
        let err =
            enforce_load_policy(&report, &policy).expect_err("default policy must block Exceeds");
        let msg = err.to_string();
        assert!(
            msg.contains("toolpath 4"),
            "error names the toolpath: {msg}"
        );
        assert!(
            msg.contains("deflection"),
            "error names the criterion: {msg}"
        );
        assert!(msg.contains("stiffness"), "error names the reason: {msg}");
    }

    #[test]
    fn enforce_lets_exceeded_through_only_with_explicit_flag() {
        // Same scenario, but with accept_exceeded=true. Note unmodeled is
        // also true — if it weren't, the report's unmodeled chipload/power
        // criteria would still block. Verify the two flags are distinct.
        let report = report_with(
            chipload_within(0.05),
            power_not_implemented(),
            deflection_exceeds(10.0),
        );

        // accept_unmodeled alone is NOT enough — Exceeds still blocks.
        let policy_u = ToolLoadExportPolicy {
            accept_unmodeled: true,
            accept_exceeded: false,
        };
        assert!(
            enforce_load_policy(&report, &policy_u).is_err(),
            "accept_unmodeled alone must not bypass Exceeds"
        );

        // accept_exceeded alone bypasses Exceeds, but unmodeled still blocks
        // (chipload+power are NotImplemented).
        let policy_e = ToolLoadExportPolicy {
            accept_unmodeled: false,
            accept_exceeded: true,
        };
        assert!(
            enforce_load_policy(&report, &policy_e).is_err(),
            "Exceeds bypassed but Unmodeled still blocks"
        );

        // Both flags accept everything.
        let policy_both = ToolLoadExportPolicy {
            accept_unmodeled: true,
            accept_exceeded: true,
        };
        assert!(enforce_load_policy(&report, &policy_both).is_ok());
    }

    #[test]
    fn enforce_blocks_unmodeled_by_default() {
        let report = report_with(
            chipload_unmodeled(UnmodeledReason::NotImplemented("phase 5".to_owned())),
            power_not_implemented(),
            deflection_within(3.0),
        );
        let err = enforce_load_policy(&report, &ToolLoadExportPolicy::default())
            .expect_err("Unmodeled must block by default");
        let msg = err.to_string();
        assert!(msg.contains("toolpath 4"), "names toolpath: {msg}");
        assert!(msg.contains("chipload"), "names unmodeled criterion: {msg}");
    }

    #[test]
    fn enforce_passes_when_all_within() {
        let report = report_with(
            chipload_within(0.05),
            PowerVerdict::Within {
                peak_kw: 0.5,
                available_kw: 0.71,
                evidence: SampleEvidence::empty(),
                confidence: Confidence::Approximate("isotropic Kc only".into()),
                entry_spike: None,
            },
            deflection_within(3.5),
        );
        assert!(
            enforce_load_policy(&report, &ToolLoadExportPolicy::default()).is_ok(),
            "all-Within report must pass"
        );
    }

    #[test]
    fn enforce_distinguishes_unmodeled_reasons() {
        // Different `Unmodeled` reasons produce different error messages so
        // the user knows whether to "run sim" vs "this material has no LUT".
        let r1 = report_with(
            chipload_unmodeled(UnmodeledReason::SimulationRequired),
            power_not_implemented(),
            deflection_within(3.0),
        );
        let r2 = report_with(
            chipload_unmodeled(UnmodeledReason::NoVendorData),
            power_not_implemented(),
            deflection_within(3.0),
        );
        let m1 = enforce_load_policy(&r1, &ToolLoadExportPolicy::default())
            .expect_err("blocks")
            .to_string();
        let m2 = enforce_load_policy(&r2, &ToolLoadExportPolicy::default())
            .expect_err("blocks")
            .to_string();
        assert!(m1.contains("SimulationRequired"));
        assert!(m2.contains("NoVendorData"));
    }

    // ── C1: gate enforcement on the phase-level checked exports ──────

    fn power_within() -> PowerVerdict {
        PowerVerdict::Within {
            peak_kw: 0.5,
            available_kw: 0.71,
            evidence: SampleEvidence::empty(),
            confidence: Confidence::Validated,
            entry_spike: None,
        }
    }

    /// Report whose only failure is a modeled `Exceeds` — so only the
    /// `accept_exceeded` flag is needed to override.
    fn exceeds_only_report() -> ToolLoadReport {
        report_with(
            chipload_within(0.05),
            power_within(),
            deflection_exceeds(10.0),
        )
    }

    fn one_phase_fixture() -> Toolpath {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 10.0));
        tp.feed_to(P3::new(10.0, 0.0, 0.0), 1000.0);
        tp
    }

    #[test]
    fn phases_checked_blocks_exceeds_and_honours_accept_exceeded() {
        let tp = one_phase_fixture();
        let phases = vec![GcodePhase {
            toolpath: &tp,
            spindle_rpm: 18_000,
            label: "Op 0",
            pre_gcode: None,
            post_gcode: None,
            tool: None,
            coolant: CoolantMode::Off,
            controller_compensation: None,
        }];
        let report = exceeds_only_report();

        let err = export_gcode_phases_with_overlay_checked(
            &phases,
            post::grbl(),
            &report,
            ToolLoadExportPolicy::default(),
            &WizardOverlay::default(),
        )
        .expect_err("default policy must refuse an Exceeds report on the phases path");
        assert!(err.to_string().contains("tool load exceeded"), "{err}");

        let ok = export_gcode_phases_with_overlay_checked(
            &phases,
            post::grbl(),
            &report,
            ToolLoadExportPolicy {
                accept_unmodeled: false,
                accept_exceeded: true,
            },
            &WizardOverlay::default(),
        );
        assert!(ok.is_ok(), "accept_exceeded=true must export: {ok:?}");
    }

    #[test]
    fn multi_setup_checked_blocks_exceeds_and_honours_accept_exceeded() {
        let tp = one_phase_fixture();
        let setups = vec![GcodeSetupPhase {
            setup_label: "Top",
            phases: vec![GcodePhase {
                toolpath: &tp,
                spindle_rpm: 18_000,
                label: "Op 0",
                pre_gcode: None,
                post_gcode: None,
                tool: None,
                coolant: CoolantMode::Off,
                controller_compensation: None,
            }],
            pause_message: None,
        }];
        let report = exceeds_only_report();

        let err = export_gcode_multi_setup_with_overlay_checked(
            &setups,
            post::grbl(),
            15.0,
            &report,
            ToolLoadExportPolicy::default(),
            &WizardOverlay::default(),
        )
        .expect_err("default policy must refuse an Exceeds report on the multi-setup path");
        assert!(err.to_string().contains("tool load exceeded"), "{err}");

        let ok = export_gcode_multi_setup_with_overlay_checked(
            &setups,
            post::grbl(),
            15.0,
            &report,
            ToolLoadExportPolicy {
                accept_unmodeled: false,
                accept_exceeded: true,
            },
            &WizardOverlay::default(),
        );
        assert!(ok.is_ok(), "accept_exceeded=true must export: {ok:?}");
    }

    // ── A1: inch units hard-refuse ────────────────────────────────────

    #[test]
    fn inch_units_override_refuses_export() {
        let tp = one_phase_fixture();
        let phases = vec![GcodePhase {
            toolpath: &tp,
            spindle_rpm: 18_000,
            label: "Op 0",
            pre_gcode: None,
            post_gcode: None,
            tool: None,
            coolant: CoolantMode::Off,
            controller_compensation: None,
        }];
        let overlay = WizardOverlay {
            units_override: Some(Units::Inch),
            ..Default::default()
        };
        let err = export_gcode_phases_with_overlay_checked(
            &phases,
            post::grbl(),
            &empty_report(),
            ToolLoadExportPolicy::default(),
            &overlay,
        )
        .expect_err("inch override must refuse export");
        assert!(
            err.to_string().contains("inch output not yet supported"),
            "{err}"
        );

        // Multi-setup path refuses too.
        let setups = vec![GcodeSetupPhase {
            setup_label: "Top",
            phases,
            pause_message: None,
        }];
        let err = export_gcode_multi_setup_with_overlay_checked(
            &setups,
            post::grbl(),
            15.0,
            &empty_report(),
            ToolLoadExportPolicy::default(),
            &overlay,
        )
        .expect_err("inch override must refuse multi-setup export");
        assert!(
            err.to_string().contains("inch output not yet supported"),
            "{err}"
        );
    }

    // ── A7: colliding display T-numbers on an M6 post ────────────────

    #[test]
    fn t_number_collision_warns_on_m6_post_only() {
        let mut tp1 = Toolpath::new();
        tp1.rapid_to(P3::new(0.0, 0.0, 10.0));
        let mut tp2 = Toolpath::new();
        tp2.rapid_to(P3::new(20.0, 0.0, 10.0));

        let phases = vec![
            GcodePhase {
                toolpath: &tp1,
                spindle_rpm: 18_000,
                label: "Rough",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 1,
                    number: 1,
                    label: "End Mill",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
            },
            GcodePhase {
                toolpath: &tp2,
                spindle_rpm: 10_610,
                label: "Finish",
                pre_gcode: None,
                post_gcode: None,
                tool: Some(PhaseTool {
                    id: 2,
                    number: 1, // SAME display number, different tool
                    label: "Tapered Ball 2mm",
                }),
                coolant: CoolantMode::Off,
                controller_compensation: None,
            },
        ];

        // M6 post (LinuxCNC): warning comment at program start.
        let lcnc = emit_gcode_phased(&phases, post::linuxcnc());
        assert!(
            lcnc.contains("WARNING: distinct tools share T1"),
            "M6 post must warn on T-number collision:\n{lcnc}"
        );
        assert!(
            lcnc.starts_with("(WARNING"),
            "warning must be prepended at program start:\n{lcnc}"
        );

        // Pause-style post (GRBL): operator message names the tool, no warning.
        let grbl = emit_gcode_phased(&phases, post::grbl());
        assert!(
            !grbl.contains("WARNING: distinct tools share"),
            "pause-style post must not warn:\n{grbl}"
        );

        // Distinct numbers on an M6 post: no warning.
        let mut distinct = phases;
        if let Some(p) = distinct.get_mut(1)
            && let Some(t) = p.tool.as_mut()
        {
            t.number = 2;
        }
        let lcnc2 = emit_gcode_phased(&distinct, post::linuxcnc());
        assert!(!lcnc2.contains("WARNING: distinct tools share"));
    }
}
