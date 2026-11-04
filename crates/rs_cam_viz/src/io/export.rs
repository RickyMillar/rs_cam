use rs_cam_core::export::gcode_validator::{Finding, Severity};
use rs_cam_core::gcode::{
    ExportPhases, ExportRequest, ExportWarning, GcodePhase, LoadPolicyOutcome, PhaseTool,
    WizardOverlay, arrange_phases_by_setup,
};
use rs_cam_core::session::{FeedStage, ProjectSession};

/// F067: the export result is core's, from the one session door
/// (`ProjectSession::export_program`): the program, the machine-safety
/// findings, the load outcome, the warnings (K6) and the feeds cautions.
pub use rs_cam_core::gcode::ExportedGcode;

/// One toolpath an export emits: its index, its motion in the export
/// datum, and the feed stage of that motion (W2.5, D10).
struct EmittedToolpath<'a> {
    index: usize,
    toolpath: std::borrow::Cow<'a, rs_cam_core::toolpath::Toolpath>,
    feeds: FeedStage,
}

/// The text an export reply appends for the feeds cautions
/// ([`ProjectSession::feeds_cautions`], W2.5 D10); `None` when there is
/// none.
pub fn feeds_caution_report(cautions: &[String]) -> Option<String> {
    if cautions.is_empty() {
        return None;
    }
    let mut report = format!(
        "Feeds caution: {} toolpath(s) emit feeds modulated from an earlier simulation.",
        cautions.len()
    );
    for caution in cautions {
        report.push_str(&format!("\n  {caution}"));
    }
    Some(report)
}

/// How many findings a text report names one by one. The rest reach the
/// reader as a count.
const MACHINE_SAFETY_REPORT_LINES: usize = 5;

/// Render the machine-safety findings for a caller that reports text.
///
/// `None` is a clean pass, so a clean export's message stays exactly what
/// it was. An `Error`-severity finding is a confirmed safety or
/// correctness issue (`Severity::Error`'s own definition), so the first
/// line says that plainly rather than leaving the reader to weigh a
/// count.
pub fn machine_safety_report(findings: &[Finding]) -> Option<String> {
    if findings.is_empty() {
        return None;
    }
    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let mut report = if errors > 0 {
        format!(
            "MACHINE-SAFETY ERROR: {errors} of {} findings are errors. \
             Do not run this program until you fix them.",
            findings.len()
        )
    } else {
        format!(
            "Machine-safety warnings: {} findings, none at error severity.",
            findings.len()
        )
    };
    for finding in findings.iter().take(MACHINE_SAFETY_REPORT_LINES) {
        let label = match finding.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        };
        report.push_str(&format!(
            "\n  [{label}] line {}: {}",
            finding.line, finding.message
        ));
    }
    if findings.len() > MACHINE_SAFETY_REPORT_LINES {
        report.push_str(&format!(
            "\n  ... and {} more.",
            findings.len() - MACHINE_SAFETY_REPORT_LINES
        ));
    }
    Some(report)
}

/// The success text an export surface reports: what it wrote, then what
/// the machine-safety pass found (EDG-06).
///
/// A clean pass returns `header` unchanged, so a clean export reads
/// exactly as it did before this row.
pub fn export_success_text(header: String, findings: &[Finding]) -> String {
    match machine_safety_report(findings) {
        Some(report) => format!("{header}\n{report}"),
        None => header,
    }
}

/// The reply an export surface gives an agent: [`export_success_text`],
/// then the load advisories (F051), the warnings (K6: each finding the
/// export names) and the [`feeds_caution_report`] when there is one
/// (W2.5, D10).
pub fn export_reply_text(header: String, exported: &ExportedGcode) -> String {
    export_reply_text_parts(
        header,
        &exported.machine_safety,
        &exported.load_outcome,
        &exported.warnings,
        &exported.feeds_cautions,
    )
}

/// [`export_reply_text`] from the parts, for a reply that merges several
/// files (MCP `split_setups`).
pub fn export_reply_text_parts(
    header: String,
    findings: &[Finding],
    load_outcome: &LoadPolicyOutcome,
    warnings: &[ExportWarning],
    feeds_cautions: &[String],
) -> String {
    let mut text = export_success_text(header, findings);
    for report in [
        load_outcome.report(),
        rs_cam_core::gcode::warnings_report(warnings),
        feeds_caution_report(feeds_cautions),
    ]
    .into_iter()
    .flatten()
    {
        text = format!("{text}\n{report}");
    }
    text
}

use crate::state::freshness::{FreshnessState, freshness_at};
use crate::state::job::ToolConfig;
use crate::state::runtime::GuiState;
use crate::state::simulation::SimulationState;

/// Pull the wizard's per-job overrides into a `WizardOverlay` for the
/// emit step. The default overlay (no fields set) is byte-identical to
/// the pre-overlay export path.
///
/// Dry-run resolution: when `wizard.dry_run` is true, `dry_run_safe_z`
/// is set to the effective safe-Z (`wizard.safe_z_override` or the
/// session post config's `safe_z`). Otherwise it stays `None` and the overlay's
/// `apply_to_program` no-ops on cutting Z values.
///
/// The controller options (spindle wait, mist, native drill cycles, the S
/// range, `$341=4`) come from the session's post block and machine
/// profile through [`rs_cam_core::gcode::PostConfig::apply_controller_options`],
/// the one resolution every export door uses (G5-G7, G10, G11).
pub fn overlay_for(session: &ProjectSession, gui: &GuiState) -> WizardOverlay {
    let w = &gui.wizard;
    let dry_run_safe_z = if w.dry_run {
        Some(w.safe_z_override.unwrap_or(session.post_config().safe_z))
    } else {
        None
    };
    let mut overlay = WizardOverlay {
        wcs_override: w.wcs_override,
        units_override: w.units_override,
        safe_z_override: w.safe_z_override,
        spindle_warmup_secs: w.spindle_warmup_secs,
        dry_run_safe_z,
        tool_change_override: w.tool_change_override,
        ..WizardOverlay::default()
    };
    session
        .post_config()
        .apply_controller_options(session.machine(), &mut overlay);
    overlay
}

/// The post-format validator, run with the controller options this export
/// emits with (G8): the mist output, from the same overlay.
pub fn validate_export(session: &ProjectSession, gui: &GuiState, gcode: &str) -> Vec<Finding> {
    let overlay = overlay_for(session, gui);
    rs_cam_core::export::gcode_validator::validate_with(
        gcode,
        session.post_config().format,
        &rs_cam_core::export::gcode_validator::ValidatorOptions::from_overlay(&overlay),
    )
}

fn phase_tool_for_export(tool: &ToolConfig) -> PhaseTool<'_> {
    PhaseTool {
        id: tool.id,
        number: tool.tool_number,
        label: tool.name.as_str(),
    }
}

/// The toolpaths an export emits and the ones it leaves out.
struct Emitted<'a> {
    toolpaths: Vec<EmittedToolpath<'a>>,
    /// K6: each enabled op in scope with no current result, and the
    /// [`left_out_message`] that names it.
    left_out: Vec<(usize, String)>,
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
/// `session.results` is the one store. It is the store the F-036b/F-039
/// adaptive feed-modulation post-pass writes. Reading the compute worker's
/// PRE-modulation output (`gui.toolpath_rt[id].result`) is what made
/// every GUI/MCP export carry commanded feeds while the export gate graded
/// a different schedule. The viz store is never read here.
///
/// # An op with no current result is left out and named — K6
///
/// G-EXPORTSKIP (2026-09-10) and G-STALEXPORT made an enabled op with no
/// current result (never generated, generating, waiting on upstream
/// stock, failed, or edited since it was generated) refuse the export.
/// K6 (feeds contract P2, lead decision 4, ruling Q6): every surface
/// exports the rest and names each such op (`export.op_left_out`), through
/// core's one rule ([`ProjectSession::export_scope`]). A disabled op is
/// skipped, the intended meaning of "disabled". The previous geometry of
/// an edited op is never emitted.
fn emitted_toolpaths<'a>(
    session: &'a ProjectSession,
    gui: &'a GuiState,
    indices: impl Iterator<Item = usize>,
) -> Emitted<'a> {
    let scope = session.export_scope(
        indices,
        |idx| {
            session.result_at(idx).is_some()
                && freshness_at(session, gui, idx) == Some(FreshnessState::Current)
        },
        |idx| {
            let name = session
                .toolpath_configs()
                .get(idx)
                .map_or("?", |tc| tc.name.as_str());
            let freshness = freshness_at(session, gui, idx).unwrap_or(FreshnessState::NoResult);
            left_out_message(name, &freshness)
        },
    );
    let toolpaths = scope
        .emitted
        .into_iter()
        .filter_map(|idx| {
            let result = session.result_at(idx)?;
            let feeds = session.feed_stage_of(idx, &result.provenance);
            let shift = rs_cam_core::gcode::export_datum_shift_for_toolpath(session, idx);
            Some(EmittedToolpath {
                index: idx,
                toolpath: rs_cam_core::gcode::toolpath_in_export_datum(result.toolpath(), shift),
                feeds,
            })
        })
        .collect();
    Emitted {
        toolpaths,
        left_out: scope.left_out,
    }
}

/// An ENABLED toolpath inside an export scope whose result is not the
/// answer for the configuration on screen: never generated, still
/// generating, waiting on upstream stock, failed — or edited since it was
/// generated (G-EXPORTSKIP, G-STALEXPORT).
///
/// K6: the export leaves each one out and names it
/// (`export.op_left_out`); the pre-flight counts it in the Part chip and
/// lists `message` on hover. The two print the same `message`.
#[derive(Debug, Clone, PartialEq)]
pub struct LeftOutToolpath {
    /// Index into `session.toolpath_configs()`.
    pub index: usize,
    pub id: rs_cam_core::ToolpathId,
    pub name: String,
    /// Why it is not exportable, as every surface reads it.
    pub freshness: FreshnessState,
    /// The operator-facing sentence, from [`left_out_message`].
    pub message: String,
}

/// The one text builder for an operation the export leaves out. The
/// export warning, the pre-flight hover and the MCP `export_gcode` reply
/// all print exactly this, so no two surfaces can disagree about why an
/// operation is left out.
///
/// Keyed on [`FreshnessState`] since G-STALEXPORT: the raw compute status
/// cannot tell an operation that was never generated from one whose result
/// belongs to a previous parameter set.
pub fn left_out_message(name: &str, freshness: &FreshnessState) -> String {
    match freshness {
        FreshnessState::EditedSince => format!(
            "'{name}' was edited after it was generated — the stored result is the \
             previous parameter set's geometry, not what the panel now shows; \
             regenerate it to export it"
        ),
        FreshnessState::Regenerating => {
            format!("'{name}' is still generating — wait for it to finish")
        }
        FreshnessState::WaitingOnUpstream(_) => format!(
            "'{name}' is still waiting on upstream stock — run Generate All / simulate the prior operation"
        ),
        FreshnessState::Error(err) => format!("'{name}' failed to generate: {err}"),
        // `Current` never reaches here, and a `Disabled` op is skipped
        // rather than left out. Kept exhaustive so a new state has to
        // choose its sentence here.
        FreshnessState::Current | FreshnessState::NoResult | FreshnessState::Disabled => {
            format!("'{name}' is not generated")
        }
    }
}

/// Every ENABLED toolpath in `indices` that the export leaves out, in
/// scope order. Disabled toolpaths and out-of-range indices are ignored.
pub fn left_out_toolpaths(
    session: &ProjectSession,
    gui: &GuiState,
    indices: impl Iterator<Item = usize>,
) -> Vec<LeftOutToolpath> {
    emitted_toolpaths(session, gui, indices)
        .left_out
        .into_iter()
        .filter_map(|(idx, message)| {
            let tc = session.toolpath_configs().get(idx)?;
            Some(LeftOutToolpath {
                index: idx,
                id: tc.id,
                name: tc.name.clone(),
                freshness: freshness_at(session, gui, idx)?,
                message,
            })
        })
        .collect()
}

/// K6: every warning the whole-project export would name, for the GUI
/// pre-flight chips. One list with the export
/// ([`ProjectSession::export_warnings`]); nothing is emitted.
pub fn preflight_warnings(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
) -> Vec<ExportWarning> {
    let emitted = emitted_toolpaths(session, gui, 0..session.toolpath_configs().len());
    let indices: Vec<usize> = emitted.toolpaths.iter().map(|e| e.index).collect();
    session.export_warnings(
        &indices,
        &emitted.left_out,
        &viz_load_report(session, sim),
        sim.cut_trace_arc().map(std::sync::Arc::as_ref),
    )
}

fn gcode_phase_for_session_toolpath<'a>(
    session: &'a ProjectSession,
    tc: &'a rs_cam_core::session::ToolpathConfig,
    toolpath: &'a rs_cam_core::toolpath::Toolpath,
    feeds: FeedStage,
) -> GcodePhase<'a> {
    let tool = session.tools().iter().find(|t| Some(t.id) == tc.tool_id);

    GcodePhase {
        toolpath,
        // Per-toolpath spindle RPM: the op's own override wins, the
        // project default is only a fallback — same resolution as the
        // core export path (`ProjectSession::export_gcode_checked`).
        spindle_rpm: rs_cam_core::compute::catalog::effective_spindle_rpm(
            &tc.operation,
            session.post_config().spindle_speed,
        ),
        label: &tc.name,
        pre_gcode: tc.pre_gcode.as_deref(),
        post_gcode: tc.post_gcode.as_deref(),
        tool: tool.map(phase_tool_for_export),
        coolant: tc.coolant,
        controller_compensation: rs_cam_core::gcode::controller_compensation_for(tc),
        drill: rs_cam_core::gcode::phase_drill_for(tc),
        feeds: Some(feeds.statement()),
    }
}

/// Build the tool-load report the export reads its load warnings from (C1,
/// 2026-06-11; K6: it warns, it does not refuse). Evaluated from the shared
/// session + the viz-side cut trace — the same inputs the readiness panel
/// and the MCP `tool_load_report` tool use, so what the user sees is what
/// the export names. The cut trace lives on viz `SimulationState`, not on
/// `session.simulation`, so the report reads it from there. The session
/// memo answers, so the gate and the panels read one report.
fn viz_load_report(
    session: &ProjectSession,
    sim: &SimulationState,
) -> std::sync::Arc<rs_cam_core::tool_load::ToolLoadReport> {
    sim.cached_load_report(session)
}

/// F067: hand the GUI's phases to the one core door. The door runs the
/// load gate, the machine-safety pass and the rapid rewrite; this crate
/// holds no copy of any of them.
fn export_through_core(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
    phases: ExportPhases<'_>,
    emitted: &Emitted<'_>,
) -> Result<ExportedGcode, crate::error::VizError> {
    if phases.phases().is_empty() && emitted.left_out.is_empty() {
        return Err(crate::error::VizError::Export(
            "No computed toolpaths to export".to_owned(),
        ));
    }
    let report = viz_load_report(session, sim);
    let overlay = overlay_for(session, gui);
    session
        .export_program(ExportRequest {
            phases,
            emitted: emitted.toolpaths.iter().map(|e| e.index).collect(),
            left_out: emitted.left_out.clone(),
            report: &report,
            // K5: the trace the report was built from (freshness of the
            // simulation rows).
            sim_trace: sim.cut_trace_arc().map(std::sync::Arc::as_ref),
            overlay: &overlay,
        })
        .map_err(|e| crate::error::VizError::Export(e.to_string()))
}

/// Export all enabled toolpaths as a single G-code file (session-based).
///
/// G3: the door puts an M0 pause between setups. The text only, for a
/// preview or a test. A door that writes a file for an operator takes
/// [`export_gcode_from_session_reporting`] and shows the load outcome, the
/// warnings, the machine-safety findings and the feeds cautions.
pub fn export_gcode_from_session(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
) -> Result<String, crate::error::VizError> {
    export_gcode_from_session_reporting(session, gui, sim).map(text_only)
}

/// The program text of an export, for a caller that shows nothing else.
fn text_only(exported: ExportedGcode) -> String {
    let ExportedGcode {
        gcode,
        // The text-only doors feed a preview or a test. A door that writes
        // a file for an operator takes a `_reporting` door instead.
        machine_safety: _,
        load_outcome: _,
        warnings: _,
        feeds_cautions: _,
    } = exported;
    gcode
}

/// The same export as [`export_gcode_from_session`], and it also hands
/// back what the checks found (EDG-06, F051, K6).
///
/// A caller that reports to an operator or to an agent takes this door,
/// and must not re-run `validate_machine_safety` on the returned text:
/// the pass runs before the high-feedrate rapid→feed rewrite, so a re-run
/// on the returned program can miss every rapid finding.
pub fn export_gcode_from_session_reporting(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
) -> Result<ExportedGcode, crate::error::VizError> {
    let emitted = emitted_toolpaths(session, gui, 0..session.toolpath_configs().len());
    // G3 (SAFETY, 2026-10-08): two or more setups get an M0 pause between
    // them, through the core arrangement every single-file door shares.
    let arranged = arrange_phases_by_setup(session, |idx| {
        let emitted = emitted.toolpaths.iter().find(|e| e.index == idx)?;
        let tc = session.toolpath_configs().get(idx)?;
        Some(gcode_phase_for_session_toolpath(
            session,
            tc,
            emitted.toolpath.as_ref(),
            emitted.feeds,
        ))
    });
    export_through_core(session, gui, sim, arranged, &emitted)
}

/// How many setups hold an enabled toolpath. A layout default and the MCP
/// split read it (G3).
pub fn setups_with_enabled_ops(session: &ProjectSession) -> usize {
    session
        .list_setups()
        .iter()
        .filter(|s| {
            s.toolpath_indices().iter().any(|i| {
                session
                    .toolpath_configs()
                    .get(*i)
                    .is_some_and(|tc| tc.enabled)
            })
        })
        .count()
}

/// One file of a "one file per setup" export.
pub struct SetupFile {
    pub setup_id: rs_cam_core::ids::SetupId,
    pub name: String,
    /// The header ([`setup_file_header`]) and the program.
    pub gcode: String,
    /// What the machine-safety pass found in this file (EDG-06).
    pub machine_safety: Vec<Finding>,
    /// F051: the load advisories of this file.
    pub load_outcome: LoadPolicyOutcome,
    /// K6: the warnings of this file (its header names each one).
    pub warnings: Vec<ExportWarning>,
    /// W2.5: the feeds cautions of this file
    /// ([`ProjectSession::feeds_cautions`]).
    pub feeds_cautions: Vec<String>,
}

/// The "one file per setup" export: one [`SetupFile`] per setup that has
/// an emitted toolpath, in setup order, each with its header.
///
/// G3 (2026-10-08): the GUI layout and MCP `split_setups` both take this
/// door, so the two surfaces write the same files for one project. A setup
/// with no enabled toolpath is skipped (it has nothing to cut). Any other
/// refusal names the setup.
pub fn export_per_setup_files(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
) -> Result<Vec<SetupFile>, crate::error::VizError> {
    let mut files: Vec<(rs_cam_core::ids::SetupId, String, ExportedGcode)> = Vec::new();
    for setup in session.list_setups() {
        let has_enabled = setup.toolpath_indices().iter().any(|i| {
            session
                .toolpath_configs()
                .get(*i)
                .is_some_and(|tc| tc.enabled)
        });
        if !has_enabled {
            continue;
        }
        let exported = export_setup_gcode_from_session_reporting(session, gui, sim, setup.id)
            .map_err(|e| crate::error::VizError::Export(format!("setup '{}': {e}", setup.name)))?;
        files.push((setup.id, setup.name.clone(), exported));
    }
    let post = session.post_config().format.definition();
    let total = files.len();
    // The setup of the file before each file: its header compares the two
    // orientations, so a reminder to turn the part appears only when the
    // part turns.
    let previous_ids: Vec<Option<rs_cam_core::ids::SetupId>> = (0..total)
        .map(|i| {
            i.checked_sub(1)
                .and_then(|p| files.get(p))
                .map(|(id, _, _)| *id)
        })
        .collect();
    Ok(files
        .into_iter()
        .enumerate()
        .map(|(i, (setup_id, name, exported))| {
            let ExportedGcode {
                gcode,
                machine_safety,
                load_outcome,
                warnings,
                feeds_cautions,
            } = exported;
            SetupFile {
                gcode: format!(
                    "{}{}",
                    setup_file_header(
                        session,
                        post,
                        setup_id,
                        previous_ids.get(i).copied().flatten(),
                        i,
                        total
                    ),
                    gcode
                ),
                setup_id,
                name,
                machine_safety,
                load_outcome,
                warnings,
                feeds_cautions,
            }
        })
        .collect())
}

/// The header of one per-setup G-code file: the setup, its datum and,
/// after the first file, what the operator does to the part. When the
/// orientation (face up and Z rotation) differs from the setup of the file
/// before, the header tells the operator to turn the part; when it is the
/// same, it tells the operator not to move the part or the zero.
///
/// G3 (2026-10-08): ONE builder for the GUI "one file per setup" layout
/// and MCP `export_gcode` with `split_setups`. The GUI files had no header
/// before, so the two surfaces wrote different files for one project.
///
/// G-EXPORT-DATUM + Z-datum (2026-09-07): X0 Y0 is the stock's min corner
/// in EVERY file (the export re-expresses every setup in the
/// stock-relative frame), so the operator KEEPS the XY zero across the
/// flip. Z follows the setup's own `datum.z_method` (see
/// `rs_cam_core::gcode::export_datum_shift_for_toolpath`): the header
/// states that declared datum, and for the `StockTop` default the
/// coordinates are already zeroed to the top.
///
/// P10 (2026-10-12): when the stock has alignment pins and a setup flips
/// the part, every file lists the dowels, the file that drills the dowel
/// holes says so, and a flipped file adds the seat line to its turn
/// message, one orientation cue and, when the dowels do not key the flip,
/// one warning line.
///
/// `index` is 0-based; `total` is the number of files written.
pub fn setup_file_header(
    session: &ProjectSession,
    post: &rs_cam_core::gcode::PostDefinition,
    setup_id: rs_cam_core::ids::SetupId,
    previous_setup_id: Option<rs_cam_core::ids::SetupId>,
    index: usize,
    total: usize,
) -> String {
    let setup = session.list_setups().iter().find(|s| s.id == setup_id);
    let previous =
        previous_setup_id.and_then(|id| session.list_setups().iter().find(|s| s.id == id));
    let name = setup.map_or("", |s| s.name.as_str());
    let z_datum = match setup.map(|s| &s.datum.z_method) {
        None | Some(rs_cam_core::session::ZDatum::StockTop) => "Z0 = top of stock".to_owned(),
        Some(rs_cam_core::session::ZDatum::MachineTable) => {
            "Z0 = machine table / spoilboard -- zero Z there \
             (NOT applied to coordinates)"
                .to_owned()
        }
        Some(rs_cam_core::session::ZDatum::FixedOffset(z)) => format!(
            "Z0 = fixed offset {z:.3}mm from machine home \
             (declared; NOT applied to coordinates)"
        ),
        Some(rs_cam_core::session::ZDatum::Manual) => {
            "Z0 = set manually per setup notes (NOT applied to coordinates)".to_owned()
        }
    };
    let mut header =
        post.render_comment(&format!("rs_cam setup {}/{total}: \"{name}\"", index + 1));
    header.push_str(&post.render_comment(&format!(
        "DATUM: X0 Y0 = stock min corner (SAME in every setup file); {z_datum}"
    )));
    // P10: the dowels, when the stock has pins and a setup flips the part.
    // The text comes from core (`ProjectSession::pin_setup_notes`), which
    // the setup sheet reads too. The budget is 3 + N lines for N dowels.
    let pin_notes = session.pin_setup_notes(setup_id);
    if let Some(notes) = &pin_notes {
        for line in &notes.pin_lines {
            header.push_str(&post.render_comment(line));
        }
        if notes.drills_holes {
            header.push_str(&post.render_comment(&format!(
                "This file drills the dowel holes {}.",
                notes.dowel_range
            )));
        }
        for line in notes.cue.iter().chain(&notes.warning) {
            header.push_str(&post.render_comment(line));
        }
    }
    if index > 0 {
        // An operator message: a MSG line on grblHAL, so the sender shows
        // it before the file starts to cut.
        let orientation = |s: &rs_cam_core::session::SetupData| (s.face_up, s.z_rotation);
        let turned = match (setup, previous) {
            (Some(s), Some(p)) => orientation(s) != orientation(p),
            // Unknown setups: keep the reminder, the safe side.
            _ => true,
        };
        let z_note = match setup.map(|s| &s.datum.z_method) {
            None | Some(rs_cam_core::session::ZDatum::StockTop) => "re-zero Z on the new top",
            Some(rs_cam_core::session::ZDatum::MachineTable) => {
                "Z0 stays on the spoilboard, do not re-zero Z"
            }
            Some(_) => "set Z to the datum above",
        };
        let message = if turned {
            let from = previous.map_or_else(String::new, |p| {
                format!("{} {} -> ", p.face_up.label(), p.z_rotation.label())
            });
            let to = setup.map_or_else(String::new, |s| {
                format!("{} {}", s.face_up.label(), s.z_rotation.label())
            });
            let seat = pin_notes
                .as_ref()
                .and_then(|n| n.seat.as_deref())
                .map_or_else(String::new, |seat| format!("{seat}; "));
            format!(
                "TURN THE PART BEFORE RUNNING ({from}{to}) -- {seat}KEEP the same X/Y zero; {z_note}"
            )
        } else {
            "SAME POSITION as the previous file -- do not move the part or the zero".to_owned()
        };
        header.push_str(&post.render_message_line(&message));
    }
    header
}

/// Export a single toolpath (by semantic id) as G-code (session-based).
/// The text only; see [`export_single_toolpath_reporting`].
pub fn export_single_toolpath_from_session(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
    toolpath_id: rs_cam_core::ToolpathId,
) -> Result<String, crate::error::VizError> {
    export_single_toolpath_reporting(session, gui, sim, toolpath_id).map(text_only)
}

/// The single-toolpath export, with what the checks found. The GUI "one
/// file per toolpath" layout takes it.
pub fn export_single_toolpath_reporting(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
    toolpath_id: rs_cam_core::ToolpathId,
) -> Result<ExportedGcode, crate::error::VizError> {
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

    // K6: an enabled op whose result is not current is left out and named
    // (`export.op_left_out`); the program then holds no phase.
    let emitted = emitted_toolpaths(session, gui, std::iter::once(tp_index));
    let phases: Vec<GcodePhase<'_>> = emitted
        .toolpaths
        .iter()
        .map(|e| gcode_phase_for_session_toolpath(session, tc, e.toolpath.as_ref(), e.feeds))
        .collect();
    export_through_core(session, gui, sim, ExportPhases::OneSetup(phases), &emitted)
}

/// Export only the toolpaths from a single setup as G-code (session-based).
pub fn export_setup_gcode_from_session(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
    setup_id: crate::state::job::SetupId,
) -> Result<String, crate::error::VizError> {
    export_setup_gcode_from_session_reporting(session, gui, sim, setup_id).map(text_only)
}

/// The same per-setup export, and it also hands back what the
/// machine-safety pass found (EDG-06). The split-setups MCP route takes
/// this door; see [`export_gcode_from_session_reporting`] for why the
/// findings travel with the text instead of being recomputed.
pub fn export_setup_gcode_from_session_reporting(
    session: &ProjectSession,
    gui: &GuiState,
    sim: &SimulationState,
    setup_id: crate::state::job::SetupId,
) -> Result<ExportedGcode, crate::error::VizError> {
    let setup = session
        .list_setups()
        .iter()
        .find(|s| s.id == setup_id)
        .ok_or_else(|| crate::error::VizError::Export(format!("Setup {setup_id:?} not found")))?;

    let emitted = emitted_toolpaths(session, gui, setup.toolpath_indices().iter().copied());
    let phases: Vec<GcodePhase<'_>> = emitted
        .toolpaths
        .iter()
        .filter_map(|e| {
            let tc = session.toolpath_configs().get(e.index)?;
            Some(gcode_phase_for_session_toolpath(
                session,
                tc,
                e.toolpath.as_ref(),
                e.feeds,
            ))
        })
        .collect();

    if phases.is_empty() && emitted.left_out.is_empty() {
        return Err(crate::error::VizError::Export(format!(
            "No computed toolpaths in setup '{}'",
            setup.name,
        )));
    }
    export_through_core(session, gui, sim, ExportPhases::OneSetup(phases), &emitted)
}
