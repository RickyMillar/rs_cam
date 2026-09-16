use rs_cam_core::gcode::{
    GcodePhase, GcodeSetupPhase, PhaseTool, ToolLoadExportPolicy, WizardOverlay,
    export_gcode_multi_setup_with_overlay_checked, export_gcode_phases_with_overlay_checked,
    replace_rapids_with_feed,
};
use rs_cam_core::gcode_validator::{MachineSafety, Severity, validate_machine_safety};
use rs_cam_core::session::ProjectSession;

/// Run the machine-safety pass over freshly emitted G-code and log any
/// findings (non-blocking). Uses the post's safe-Z as the clearance
/// plane; the depth-floor and feed-cap checks stay off until the machine
/// profile is plumbed through (turning them on without correct limits
/// would risk false positives). Runs before any high-feedrate rapid→feed
/// rewrite so the rapid structure is still intact to check.
fn log_machine_safety(gcode: &str, safe_z: f64) {
    let findings = validate_machine_safety(
        gcode,
        MachineSafety {
            clearance_z: safe_z,
            min_z: None,
            max_feed_mm_min: None,
        },
    );
    if findings.is_empty() {
        return;
    }
    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    tracing::warn!(
        total = findings.len(),
        errors,
        "g-code machine-safety pass raised findings; first: line {} — {}",
        findings.first().map(|f| f.line).unwrap_or(0),
        findings.first().map(|f| f.message.as_str()).unwrap_or(""),
    );
}

use crate::state::freshness::{FreshnessState, freshness_at};
use crate::state::job::ToolConfig;
use crate::state::runtime::{GuiState, StaleResultPolicy};
use crate::state::simulation::SimulationState;

/// Pull the wizard's per-job overrides into a `WizardOverlay` for the
/// emit step. The default overlay (no fields set) is byte-identical to
/// the pre-overlay export path.
///
/// Dry-run resolution: when `wizard.dry_run` is true, `dry_run_safe_z`
/// is set to the effective safe-Z (`wizard.safe_z_override` or
/// `gui.post.safe_z`). Otherwise it stays `None` and the overlay's
/// `apply_to_program` no-ops on cutting Z values.
fn overlay_for(gui: &GuiState) -> WizardOverlay {
    let w = &gui.wizard;
    let dry_run_safe_z = if w.dry_run {
        Some(w.safe_z_override.unwrap_or(gui.post.safe_z))
    } else {
        None
    };
    WizardOverlay {
        wcs_override: w.wcs_override,
        units_override: w.units_override,
        safe_z_override: w.safe_z_override,
        spindle_warmup_secs: w.spindle_warmup_secs,
        dry_run_safe_z,
        tool_change_override: w.tool_change_override,
    }
}

fn phase_tool_for_export(tool: &ToolConfig) -> PhaseTool<'_> {
    PhaseTool {
        id: tool.id.0,
        number: tool.tool_number,
        label: tool.name.as_str(),
    }
}

/// G-EXPORT-DATUM (2026-08-19): collect each toolpath re-expressed in the
/// shared export datum (stock-relative XY — see
/// [`rs_cam_core::gcode::export_datum_shift_for_toolpath`]). Returned as an
/// owning vec because [`GcodePhase`] borrows its toolpath and the shifted
/// copy has to outlive the phases. Zero-shift toolpaths (non-identity
/// setups, zero-origin stock) stay borrowed — no clone.
///
/// `indices` are indices into `session.toolpath_configs()`; the setup a
/// toolpath belongs to (hence its emission frame) is resolved from there.
///
/// # Which store the program is built from — G-MODEXPORT (2026-08-22)
///
/// `session.results` is the owner. It is the store the F-036b/F-039
/// adaptive feed-modulation post-pass writes
/// ([`rs_cam_core::session::ProjectSession::modulate_simulation_trace`] →
/// `apply_adaptive_feed_modulation` swaps the modulated
/// `Arc<AnnotatedToolpath>` into it after every simulation, and into
/// nothing else). Reading `gui.toolpath_rt[id].result` first — the compute
/// worker's PRE-modulation output — is what made every GUI/MCP export
/// carry commanded feeds while the export gate, which reads the
/// (modulated) viz cut trace, graded a different schedule: measured live
/// as a `Within` chipload verdict at ~443 mm/min over a file emitted at a
/// flat F3000.
///
/// The viz store stays as a **fallback**, not as a second owner, because
/// one divergence is genuinely reachable in the other direction:
/// [`rs_cam_core::session::ProjectSession::invalidate_tool`] (a GUI tool
/// param edit — `ui::properties::commit_tool_draft`) and
/// `Command::RestoreToolpathSnapshot` drop `session.results` entries, while
/// the viz store keeps its result so the UI can draw the stale toolpath.
/// Nothing modulated exists for such a toolpath anyway, so falling back to
/// the worker IR preserves the pre-fix behaviour exactly. Every other
/// production write of `toolpath_rt[..].result` is immediately preceded by
/// the F1_RCA sync into `session.results`
/// (`controller/events/compute.rs::drain_compute_results`), so on the
/// normal generate → simulate → export path the session slot is present
/// and wins.
///
/// Sentried by `tests/modulated_feeds_reach_gcode_g_modexport.rs`, which
/// asserts the emitted **F-words**, not the trace.
///
/// # An enabled op with no result refuses — G-EXPORTSKIP (2026-09-10)
///
/// Until this date the collect was a `filter_map`: an ENABLED toolpath
/// with no result in either store dropped out and the program shipped
/// without it, on every export surface, with no word to the operator
/// (R05 §5). The reachable case is a rest op left `AwaitingPriorStock`
/// by a `generate_all` that ran out of rounds; a never-generated op and
/// an `Error` op are the other two shapes. Now every enabled op in scope
/// with no result is a refusal that names it — see
/// [`blocking_toolpaths`] — and a DISABLED op is still skipped, which
/// is the intended meaning of "disabled".
///
/// # An EDITED op refuses too — G-STALEXPORT (2026-09-10)
///
/// The fallback described above is no longer silent. G-FRESHSTATE (F2.1)
/// made every input edit drop the core result, which turned the viz
/// store's copy — kept so the viewport can draw the old path — into the
/// store the export would quietly fall back to, on far more routes than
/// before. Drawing the previous geometry and CUTTING it are different
/// permissions. The fallback is therefore no longer taken by default: an
/// operation whose [`FreshnessState`] is not `Current` blocks the export
/// and is named, and the previous geometry is emitted only under
/// [`StaleResultPolicy::AcceptPreviousGeometry`], which the operator
/// sets per export.
fn emitted_toolpaths<'a>(
    session: &'a ProjectSession,
    gui: &'a GuiState,
    indices: impl Iterator<Item = usize>,
    stale: StaleResultPolicy,
) -> Result<
    Vec<(usize, std::borrow::Cow<'a, rs_cam_core::toolpath::Toolpath>)>,
    crate::error::VizError,
> {
    let indices: Vec<usize> = indices.collect();
    let blockers: Vec<BlockingToolpath> =
        blocking_toolpaths(session, gui, indices.iter().copied(), stale)
            .into_iter()
            .filter(|b| !b.waived_by_operator)
            .collect();
    if !blockers.is_empty() {
        return Err(crate::error::VizError::Export(blocking_refusal_text(
            &blockers,
        )));
    }
    Ok(indices
        .into_iter()
        .filter_map(|idx| {
            let tc = session.toolpath_configs().get(idx)?;
            if !tc.enabled {
                return None;
            }
            let toolpath = emitted_result_toolpath(session, gui, idx, tc, stale)?;
            let shift = rs_cam_core::gcode::export_datum_shift_for_toolpath(session, idx);
            Some((
                idx,
                rs_cam_core::gcode::toolpath_in_export_datum(toolpath, shift),
            ))
        })
        .collect())
}

/// The one result-lookup the export reads: `session.results` first — the
/// store the feed-modulation post-pass writes (see the G-MODEXPORT note
/// on [`emitted_toolpaths`]) — and the viz store only when the operator
/// has accepted the previous geometry for this export.
///
/// `None` means the toolpath has nothing this export is allowed to emit.
/// Under [`StaleResultPolicy::Refuse`] that includes an edited operation
/// whose viz copy still exists; the caller has already refused it by
/// name. Nothing modulated exists for such an operation (the modulation
/// pass writes only into `session.results`, which the edit dropped), so
/// what the acceptance emits is the compute worker's pre-modulation
/// output, exactly as the pre-G-STALEXPORT fallback did.
fn emitted_result_toolpath<'a>(
    session: &'a ProjectSession,
    gui: &'a GuiState,
    idx: usize,
    tc: &rs_cam_core::session::ToolpathConfig,
    stale: StaleResultPolicy,
) -> Option<&'a rs_cam_core::toolpath::Toolpath> {
    if let Some(result) = session.get_result(idx) {
        return Some(result.toolpath());
    }
    if !stale.accepts_previous_geometry() {
        return None;
    }
    Some(gui.toolpath_rt.get(&tc.id)?.result.as_ref()?.toolpath())
}

/// An ENABLED toolpath inside an export scope whose result is not the
/// answer for the configuration on screen: never generated, still
/// generating, waiting on upstream stock, failed — or edited since it was
/// generated (G-EXPORTSKIP, extended by G-STALEXPORT).
///
/// One row per such op. The export refuses on every row that is not
/// `waived_by_operator`; the pre-flight modal lists all of them, blocking
/// rows as failures and waived ones as cautions, printing the same
/// `message` either way.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockingToolpath {
    /// Index into `session.toolpath_configs()`.
    pub index: usize,
    pub id: rs_cam_core::ToolpathId,
    pub name: String,
    /// Why it is not exportable, as every surface reads it.
    pub freshness: FreshnessState,
    /// The operator-facing sentence, from [`blocking_toolpath_message`].
    pub message: String,
    /// True when the operator has accepted this row for this export, so
    /// it does NOT block. Only an edited operation can be waived, and
    /// only under [`StaleResultPolicy::AcceptPreviousGeometry`]: a
    /// missing result cannot be waived, because there is no geometry to
    /// emit in its place.
    pub waived_by_operator: bool,
}

/// The one text builder for an operation the export cannot emit as the
/// current answer. The export refusal, the pre-flight row and the MCP
/// `export_gcode` reply all print exactly this, so no two surfaces can
/// disagree about why an operation blocks.
///
/// Keyed on [`FreshnessState`] since G-STALEXPORT. It used to take the
/// raw [`ComputeStatus`], which could not tell an operation that was
/// never generated from one whose result belongs to a previous parameter
/// set — both read `Done`/absent — and so had no sentence for the second.
pub fn blocking_toolpath_message(name: &str, freshness: &FreshnessState) -> String {
    match freshness {
        FreshnessState::EditedSince => format!(
            "'{name}' was edited after it was generated — the stored result is the \
             previous parameter set's geometry, not what the panel now shows; \
             regenerate it, or accept the previous geometry to cut it as it was"
        ),
        FreshnessState::Regenerating => {
            format!("'{name}' is still generating — wait for it to finish")
        }
        FreshnessState::WaitingOnUpstream(_) => format!(
            "'{name}' is still waiting on upstream stock — run Generate All / simulate the prior operation"
        ),
        FreshnessState::Error(err) => format!("'{name}' failed to generate: {err}"),
        // `Current` never reaches here, and a `Disabled` op is skipped
        // rather than blocking. Kept exhaustive so a new state has to
        // choose its sentence here.
        FreshnessState::Current | FreshnessState::NoResult | FreshnessState::Disabled => {
            format!("'{name}' is not generated")
        }
    }
}

/// Every ENABLED toolpath in `indices` whose result is not current, in
/// scope order, each marked with whether `stale` waives it. Empty means
/// the export can proceed; a list of only waived rows also means it can.
/// Disabled toolpaths and out-of-range indices are ignored.
pub fn blocking_toolpaths(
    session: &ProjectSession,
    gui: &GuiState,
    indices: impl Iterator<Item = usize>,
    stale: StaleResultPolicy,
) -> Vec<BlockingToolpath> {
    indices
        .filter_map(|idx| {
            let tc = session.toolpath_configs().get(idx)?;
            let freshness = freshness_at(session, gui, idx)?;
            if matches!(
                freshness,
                FreshnessState::Current | FreshnessState::Disabled
            ) {
                return None;
            }
            // A waived row must still have geometry to emit. `EditedSince`
            // carries a drawable viz result by construction; the check is
            // here so the refusal and the emit cannot disagree if that
            // ever stops being true.
            let waived_by_operator = matches!(freshness, FreshnessState::EditedSince)
                && emitted_result_toolpath(session, gui, idx, tc, stale).is_some();
            Some(BlockingToolpath {
                index: idx,
                id: tc.id,
                name: tc.name.clone(),
                message: blocking_toolpath_message(&tc.name, &freshness),
                freshness,
                waived_by_operator,
            })
        })
        .collect()
}

/// The refusal an export surface returns for one or more blocking ops:
/// their messages, in scope order, joined with `; `. Wrapped by
/// `VizError::Export`, so the operator reads `Export failed: 'X' is …`.
fn blocking_refusal_text(blockers: &[BlockingToolpath]) -> String {
    blockers
        .iter()
        .map(|b| b.message.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}

fn gcode_phase_for_session_toolpath<'a>(
    session: &'a ProjectSession,
    gui: &'a GuiState,
    tc: &'a rs_cam_core::session::ToolpathConfig,
    toolpath: &'a rs_cam_core::toolpath::Toolpath,
) -> Option<GcodePhase<'a>> {
    let tool = session.tools().iter().find(|t| t.id.0 == tc.tool_id);

    Some(GcodePhase {
        toolpath,
        // Per-toolpath spindle RPM: the op's own override wins, the
        // project default is only a fallback — same resolution as the
        // core export path (`export_gcode_checked`). Pre-fix this read
        // `gui.post.spindle_speed` unconditionally, flattening every
        // op's RPM (WANAKA's 12194/10610/21000) to the project default.
        spindle_rpm: rs_cam_core::compute::catalog::effective_spindle_rpm(
            &tc.operation,
            gui.post.spindle_speed,
        ),
        label: &tc.name,
        pre_gcode: tc.pre_gcode.as_deref(),
        post_gcode: tc.post_gcode.as_deref(),
        tool: tool.map(phase_tool_for_export),
        coolant: tc.coolant,
        controller_compensation: rs_cam_core::gcode::controller_compensation_for(tc),
    })
}

/// The cut trace lives on viz `SimulationState`, not on `session.simulation`.
/// Pull from there so the export gate evaluates chipload/power against the
/// active simulation run.
fn viz_sim_trace(
    sim: &SimulationState,
) -> Option<&rs_cam_core::stock::simulation_cut::SimulationCutTrace> {
    sim.results.as_ref().and_then(|r| r.cut_trace.as_deref())
}

/// Build the tool-load report the checked export functions enforce the
/// policy against (C1, 2026-06-11). Evaluated from the shared session +
/// the viz-side cut trace — the same inputs the readiness panel and the
/// MCP `tool_load_report` tool use, so what the user sees is what the
/// gate enforces.
fn viz_load_report(
    session: &ProjectSession,
    sim: &SimulationState,
) -> rs_cam_core::tool_load::ToolLoadReport {
    rs_cam_core::gcode::project_load_report(session, viz_sim_trace(sim))
}

/// Export all enabled toolpaths as a single G-code file (session-based).
pub fn export_gcode_from_session(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
) -> Result<String, crate::error::VizError> {
    export_gcode_from_session_with_policy(
        session,
        gui,
        sim,
        gui.tool_load_overrides.as_policy(),
        gui.stale_export,
    )
}

/// Same as [`export_gcode_from_session`], but lets the caller supply an
/// explicit tool-load policy and stale-result policy instead of reading
/// `gui.tool_load_overrides` / `gui.stale_export`. Used by the MCP export
/// path so an automation client can pass its accept flags directly
/// without mutating GUI state.
pub fn export_gcode_from_session_with_policy(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
    policy: ToolLoadExportPolicy,
    stale: StaleResultPolicy,
) -> Result<String, crate::error::VizError> {
    let post = gui.post.format.definition();

    let emitted = emitted_toolpaths(session, gui, 0..session.toolpath_configs().len(), stale)?;
    let phases: Vec<GcodePhase<'_>> = emitted
        .iter()
        .filter_map(|(idx, tp)| {
            let tc = session.toolpath_configs().get(*idx)?;
            gcode_phase_for_session_toolpath(session, gui, tc, tp.as_ref())
        })
        .collect();

    if phases.is_empty() {
        return Err(crate::error::VizError::Export(
            "No computed toolpaths to export".to_owned(),
        ));
    }

    let mut gcode = export_gcode_phases_with_overlay_checked(
        &phases,
        post,
        &viz_load_report(session, sim),
        policy,
        &overlay_for(gui),
    )
    .map_err(|e| crate::error::VizError::Export(e.to_string()))?;

    log_machine_safety(&gcode, gui.post.safe_z);

    if gui.post.high_feedrate_mode {
        gcode = replace_rapids_with_feed(&gcode, gui.post.high_feedrate, post);
    }

    Ok(gcode)
}

/// Export all setups as a single G-code file with M0 pauses (session-based).
pub fn export_combined_gcode_from_session(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
) -> Result<String, crate::error::VizError> {
    let post = gui.post.format.definition();

    // Shifted toolpaths must outlive the borrowed phases, so build the
    // whole project's set up front and slice it per setup below.
    let emitted = emitted_toolpaths(
        session,
        gui,
        0..session.toolpath_configs().len(),
        gui.stale_export,
    )?;
    let setup_phases: Vec<GcodeSetupPhase<'_>> = session
        .list_setups()
        .iter()
        .filter_map(|setup| {
            // Driven by `setup.toolpath_indices`, NOT by `emitted`'s own
            // order: the setup's list is the machining order and it is
            // not necessarily ascending after an op reorder.
            let phases: Vec<GcodePhase<'_>> = setup
                .toolpath_indices
                .iter()
                .filter_map(|tp_idx| {
                    let (_, tp) = emitted.iter().find(|(idx, _)| idx == tp_idx)?;
                    let tc = session.toolpath_configs().get(*tp_idx)?;
                    gcode_phase_for_session_toolpath(session, gui, tc, tp.as_ref())
                })
                .collect();
            if phases.is_empty() {
                None
            } else {
                Some(GcodeSetupPhase {
                    setup_label: &setup.name,
                    phases,
                    pause_message: setup.pause_message.as_deref(),
                })
            }
        })
        .collect();

    if setup_phases.is_empty() {
        return Err(crate::error::VizError::Export(
            "No computed toolpaths to export".to_owned(),
        ));
    }

    let mut gcode = export_gcode_multi_setup_with_overlay_checked(
        &setup_phases,
        post,
        gui.post.safe_z,
        &viz_load_report(session, sim),
        gui.tool_load_overrides.as_policy(),
        &overlay_for(gui),
    )
    .map_err(|e| crate::error::VizError::Export(e.to_string()))?;

    log_machine_safety(&gcode, gui.post.safe_z);

    if gui.post.high_feedrate_mode {
        gcode = replace_rapids_with_feed(&gcode, gui.post.high_feedrate, post);
    }

    Ok(gcode)
}

/// Export a single toolpath (by semantic id) as G-code (session-based).
pub fn export_single_toolpath_from_session(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
    toolpath_id: rs_cam_core::ToolpathId,
) -> Result<String, crate::error::VizError> {
    let post = gui.post.format.definition();

    let tp_index = session
        .toolpath_configs()
        .iter()
        .position(|tc| tc.id == toolpath_id)
        .ok_or_else(|| {
            crate::error::VizError::Export(format!("Toolpath id {toolpath_id} not found"))
        })?;
    let tc = session.toolpath_configs().get(tp_index).ok_or_else(|| {
        crate::error::VizError::Export(format!("Toolpath id {toolpath_id} not found"))
    })?;

    if !tc.enabled {
        return Err(crate::error::VizError::Export(format!(
            "Toolpath '{}' is disabled",
            tc.name
        )));
    }

    // G-EXPORTSKIP / G-STALEXPORT: an enabled op whose result is not
    // current is refused inside `emitted_toolpaths` with the shared text.
    // The `first()` fallback is unreachable after that `?` and keeps the
    // same sentence.
    let emitted = emitted_toolpaths(session, gui, std::iter::once(tp_index), gui.stale_export)?;
    let (_, emitted_toolpath) = emitted.first().ok_or_else(|| {
        let freshness = freshness_at(session, gui, tp_index).unwrap_or(FreshnessState::NoResult);
        crate::error::VizError::Export(blocking_toolpath_message(&tc.name, &freshness))
    })?;
    let phase = gcode_phase_for_session_toolpath(session, gui, tc, emitted_toolpath.as_ref())
        .ok_or_else(|| {
            crate::error::VizError::Export(format!(
                "Toolpath '{}' has no computed result — generate it first",
                tc.name
            ))
        })?;

    let mut gcode = export_gcode_phases_with_overlay_checked(
        std::slice::from_ref(&phase),
        post,
        &viz_load_report(session, sim),
        gui.tool_load_overrides.as_policy(),
        &overlay_for(gui),
    )
    .map_err(|e| crate::error::VizError::Export(e.to_string()))?;

    log_machine_safety(&gcode, gui.post.safe_z);

    if gui.post.high_feedrate_mode {
        gcode = replace_rapids_with_feed(&gcode, gui.post.high_feedrate, post);
    }

    Ok(gcode)
}

/// Export only the toolpaths from a single setup as G-code (session-based).
pub fn export_setup_gcode_from_session(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
    setup_id: crate::state::job::SetupId,
) -> Result<String, crate::error::VizError> {
    export_setup_gcode_from_session_with_policy(
        session,
        gui,
        sim,
        setup_id,
        gui.tool_load_overrides.as_policy(),
        gui.stale_export,
    )
}

/// Same as [`export_setup_gcode_from_session`], but lets the caller supply
/// explicit tool-load and stale-result policies (e.g. the MCP `accept_*`
/// flags) instead of defaulting to the GUI's own.
pub fn export_setup_gcode_from_session_with_policy(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
    setup_id: crate::state::job::SetupId,
    policy: ToolLoadExportPolicy,
    stale: StaleResultPolicy,
) -> Result<String, crate::error::VizError> {
    let setup = session
        .list_setups()
        .iter()
        .find(|s| s.id == setup_id.0)
        .ok_or_else(|| crate::error::VizError::Export(format!("Setup {setup_id:?} not found")))?;

    let post = gui.post.format.definition();

    let emitted = emitted_toolpaths(session, gui, setup.toolpath_indices.iter().copied(), stale)?;
    let phases: Vec<GcodePhase<'_>> = emitted
        .iter()
        .filter_map(|(idx, tp)| {
            let tc = session.toolpath_configs().get(*idx)?;
            gcode_phase_for_session_toolpath(session, gui, tc, tp.as_ref())
        })
        .collect();

    if phases.is_empty() {
        return Err(crate::error::VizError::Export(format!(
            "No computed toolpaths in setup '{}'",
            setup.name,
        )));
    }

    let mut gcode = export_gcode_phases_with_overlay_checked(
        &phases,
        post,
        &viz_load_report(session, sim),
        policy,
        &overlay_for(gui),
    )
    .map_err(|e| crate::error::VizError::Export(e.to_string()))?;

    log_machine_safety(&gcode, gui.post.safe_z);

    if gui.post.high_feedrate_mode {
        gcode = replace_rapids_with_feed(&gcode, gui.post.high_feedrate, post);
    }

    Ok(gcode)
}
