//! G-code export and the per-toolpath report contexts.
//!
//! Holds the export entries and the context builders the diagnostics adapters
//! read — model reference, preconditions, heights and feeds. Split out of
//! `session/compute.rs` (P4).

use tracing::instrument;

use crate::compute::tool_config::ToolConfig;
use crate::session::{ProjectEvidence, ProjectSession, SessionError};
use crate::stock::simulation_cut::SimulationCutTrace;

/// W2b: how much of the toolpath list
/// [`ProjectSession::diagnose_toolpath_with_trace`] builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiagnoseDepth {
    /// Every source.
    Full,
    /// The sources that can refuse generation only
    /// ([`ProjectSession::diagnose_toolpath_gate`]).
    Gate,
}

/// K6 (lead decision 4): the toolpaths an export emits and the ones it
/// leaves out, from [`ProjectSession::export_scope`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExportScope {
    /// The enabled toolpaths with a current result, in scope order.
    pub emitted: Vec<usize>,
    /// The enabled toolpaths with no current result, and why.
    pub left_out: Vec<(usize, String)>,
}

impl ProjectSession {
    /// **F067: the one checked export door.** The GUI, MCP and the CLI
    /// all emit through here, so one project state gives one program and
    /// one set of checks on every surface.
    ///
    /// In order:
    /// 1. K6 (ruling Q6): the warnings ([`Self::export_warnings`]) of the
    ///    emitted toolpaths, the ops left out and the load report. No
    ///    finding refuses; each warning prints one header comment.
    /// 2. [`crate::gcode::emit_checked`]: the emit errors (inch units, a
    ///    bad feed, controller compensation) and the program. The post and
    ///    the inter-setup retract come from THIS session's post config,
    ///    never from a caller's copy (F072).
    /// 3. The machine-safety pass ([`validate_machine_safety`]) on the
    ///    program as emitted, BEFORE any rewrite, while the rapids are
    ///    still `G0`. The clearance plane is the emitted retract plane of
    ///    the toolpaths in `request.emitted`
    ///    ([`emission_frame_clearance_z`], R11). The feed cap is the
    ///    machine's travel rate, `max_feed_mm_min`: the rapid rewrite
    ///    writes rapids as feeds at the travel rate, so the cutting
    ///    ceiling would flag every rewritten rapid (T-9). The depth-floor
    ///    check stays off: no caller has a stock-bottom limit to hand it.
    /// 4. The high-feedrate rapid→feed rewrite, when the post config asks
    ///    for it (`high_feedrate_mode`).
    ///
    /// Until F067 only the GUI/MCP door ran 3 and 4, so the CLI emitted
    /// `G0` rapids where the GUI emitted `G1` traverses, and no CLI file
    /// was safety-checked. The findings are advisory on every surface: no
    /// door refuses on them.
    ///
    /// [`validate_machine_safety`]: crate::export::gcode_validator::validate_machine_safety
    /// [`emission_frame_clearance_z`]: crate::export::gcode_validator::emission_frame_clearance_z
    ///
    /// # Errors
    /// [`crate::gcode::EmitError`] when the emitter cannot write a valid
    /// program. No finding is an error.
    pub fn export_program(
        &self,
        request: crate::gcode::ExportRequest<'_>,
    ) -> Result<crate::gcode::ExportedGcode, crate::gcode::EmitError> {
        use crate::export::gcode_validator::{
            MachineSafety, Severity, emission_frame_clearance_z, validate_machine_safety,
        };

        let crate::gcode::ExportRequest {
            phases,
            emitted,
            left_out,
            report,
            overlay,
        } = request;
        let warnings = self.export_warnings(&emitted, &left_out, report);
        let load_outcome = self.export_load_findings(&emitted, report).outcome;
        let post_config = &self.post;
        let post = post_config.format.definition();
        let mut gcode =
            crate::gcode::emit_checked(&phases, post, post_config.safe_z, overlay, &warnings)?;

        let machine_safety = validate_machine_safety(
            &gcode,
            MachineSafety {
                clearance_z: emission_frame_clearance_z(self, emitted.iter().copied()),
                min_z: None,
                max_feed_mm_min: Some(self.machine.max_feed_mm_min),
            },
        );
        if let Some(first) = machine_safety.first() {
            let errors = machine_safety
                .iter()
                .filter(|f| f.severity == Severity::Error)
                .count();
            tracing::warn!(
                total = machine_safety.len(),
                errors,
                "g-code machine-safety pass raised findings; first: line {} — {}",
                first.line,
                first.message,
            );
        }

        if post_config.high_feedrate_mode {
            gcode = crate::gcode::replace_rapids_with_feed(&gcode, post_config.high_feedrate, post);
        }

        Ok(crate::gcode::ExportedGcode {
            gcode,
            machine_safety,
            load_outcome,
            warnings,
            feeds_cautions: self.feeds_cautions(emitted.iter().copied()),
        })
    }

    /// D47 (G-RESTSTALE): the load findings of the toolpaths the program
    /// emits. An enabled toolpath outside `emitted` (a CLI `--skip`, a
    /// setup the export leaves out) has no result, so the simulation did
    /// not carve it and the program does not cut it; its row would read
    /// `StaleSimulation`.
    fn export_load_findings(
        &self,
        emitted: &[usize],
        report: &crate::tool_load::ToolLoadReport,
    ) -> crate::gcode::LoadFindings {
        let emitted_ids: Vec<crate::ids::ToolpathId> = emitted
            .iter()
            .filter_map(|idx| self.toolpath_configs.get(*idx))
            .map(|tc| tc.id)
            .collect();
        crate::gcode::load_findings(&crate::tool_load::ToolLoadReport {
            per_toolpath: report
                .per_toolpath
                .iter()
                .filter(|verdict| emitted_ids.contains(&verdict.toolpath_id))
                .cloned()
                .collect(),
        })
    }

    /// **K6 (CONTRACT K6, ruling Q6): every finding an export names.** The
    /// export door, the GUI pre-flight chips and the MCP reply read this
    /// one list. It walks [`crate::diagnostics::Gate::ALL`] with one
    /// exhaustive `match`: each gate decides whether its finding is raised;
    /// the id's `diag_ids!` row gives the danger. Nothing here refuses.
    ///
    /// `emitted` are the toolpath indices the program holds; `left_out` the
    /// enabled ops in scope with no current result and the sentence that
    /// says why ([`Self::export_scope`]); `report` the load report the
    /// load rows read.
    #[must_use]
    pub fn export_warnings(
        &self,
        emitted: &[usize],
        left_out: &[(usize, String)],
        report: &crate::tool_load::ToolLoadReport,
    ) -> Vec<crate::gcode::ExportWarning> {
        use crate::diagnostics::{Gate, ids};
        let name_at = |idx: usize| {
            self.toolpath_configs
                .get(idx)
                .map_or_else(|| "?".to_owned(), |tc| tc.name.clone())
        };
        let name_of = |id: crate::ids::ToolpathId| {
            self.toolpath_configs
                .iter()
                .find(|tc| tc.id == id)
                .map_or_else(|| format!("toolpath {id}"), |tc| tc.name.clone())
        };
        let mut out: Vec<crate::gcode::ExportWarning> = Vec::new();
        let mut push = |id: &'static str, toolpath: String, detail: String| {
            if let Some(warning) = crate::gcode::ExportWarning::new(id, toolpath, detail) {
                out.push(warning);
            }
        };
        let load = self.export_load_findings(emitted, report);
        for gate in Gate::ALL {
            match gate {
                Gate::HeightRefuses => {
                    for tc in emitted.iter().filter_map(|i| self.toolpath_configs.get(*i)) {
                        if let Some(found) = self.height_order_refusal(tc) {
                            for violation in &found.violations {
                                push(violation.id(), tc.name.clone(), found.to_string());
                            }
                        }
                    }
                }
                Gate::GougeBlocks => {
                    for &idx in emitted {
                        if let Some(row) = self.model_gouge_refusal(idx) {
                            push(ids::SAFETY_CUTS_INTO_MODEL, name_at(idx), row);
                        }
                    }
                }
                Gate::CentreCutPlunge => {
                    for &idx in emitted {
                        if let Some(plunge) = self
                            .result_at(idx)
                            .and_then(|r| r.stats.non_centre_cutting_plunge)
                        {
                            push(
                                ids::SAFETY_NON_CENTRE_CUTTING_PLUNGE,
                                name_at(idx),
                                format!("Toolpath '{}': {plunge}", name_at(idx)),
                            );
                        }
                    }
                }
                Gate::CentreCutDescent => {
                    if let Some(simulation) = self.simulation.as_ref() {
                        for tc in emitted.iter().filter_map(|i| self.toolpath_configs.get(*i)) {
                            let rows = crate::compute::simulate::non_centre_cutting_refusal_rows(
                                simulation,
                                &[tc.id],
                                |id| {
                                    self.toolpath_configs
                                        .iter()
                                        .find(|t| t.id == id)
                                        .map(|t| t.name.as_str())
                                },
                            );
                            for row in rows {
                                push(ids::SAFETY_NON_CENTRE_CUTTING_DESCENT, tc.name.clone(), row);
                            }
                        }
                    }
                }
                Gate::LoadRowRefuses | Gate::LoadUnmodeled => {
                    let unmodeled = matches!(gate, Gate::LoadUnmodeled);
                    for row in load
                        .rows
                        .iter()
                        .filter(|r| r.id.ends_with(".unmodeled") == unmodeled)
                    {
                        push(row.id, name_of(row.toolpath), row.detail.clone());
                    }
                }
                Gate::NoCurrentResult => {
                    for (idx, reason) in left_out {
                        push(ids::EXPORT_OP_LEFT_OUT, name_at(*idx), reason.clone());
                    }
                }
                Gate::GuiConfirm => {
                    if let Some(simulation) = self.simulation.as_ref() {
                        let evidence = crate::session::ProjectEvidence::from_simulation(simulation);
                        for tc in emitted.iter().filter_map(|i| self.toolpath_configs.get(*i)) {
                            let count = evidence
                                .attributed_rapid_collisions()
                                .iter()
                                .filter(|hit| hit.toolpath.is_some_and(|(id, _)| id == tc.id))
                                .count();
                            if count > 0 {
                                push(
                                    ids::PROJECT_RAPID_COLLISION,
                                    tc.name.clone(),
                                    format!(
                                        "Toolpath '{}': {count} rapid move(s) go through stock",
                                        tc.name
                                    ),
                                );
                            }
                        }
                    }
                }
                Gate::BuriedFace => {
                    for &idx in emitted {
                        if let Some(buried) = self.result_at(idx).and_then(|r| r.stats.buried_face)
                        {
                            push(ids::PRECOND_BURIED_FACE, name_at(idx), buried.to_string());
                        }
                    }
                }
                Gate::ToolKind => {
                    for tc in emitted.iter().filter_map(|i| self.toolpath_configs.get(*i)) {
                        let Some(tool) = self.toolpath_tool(tc) else {
                            continue;
                        };
                        if let Some(text) = tc
                            .operation
                            .op_type()
                            .tool_refusal(tool.tool_type.cutter_kind())
                        {
                            push(ids::COMPAT_TOOL_KIND_REFUSED, tc.name.clone(), text);
                        }
                    }
                }
                Gate::Feasibility => {
                    for &idx in emitted {
                        let Some(report) = self
                            .result_at(idx)
                            .and_then(|r| r.stats.feasibility.as_ref())
                        else {
                            continue;
                        };
                        for row in report.rows() {
                            push(
                                row.id(),
                                name_at(idx),
                                format!("Toolpath '{}': {}", name_at(idx), row.text()),
                            );
                        }
                    }
                }
            }
        }
        out
    }

    /// **K6 (lead decision 4): THE export scope rule.** Of the toolpaths in
    /// `indices`, the enabled ones whose result `current` accepts are
    /// emitted; every other enabled one is left out and named
    /// (`export.op_left_out`) with the sentence `reason` gives. A disabled
    /// toolpath is skipped. The core door ([`Self::export_gcode_checked`])
    /// and the viz door (`io::export::emitted_toolpaths`) both call it.
    pub fn export_scope(
        &self,
        indices: impl Iterator<Item = usize>,
        current: impl Fn(usize) -> bool,
        reason: impl Fn(usize) -> String,
    ) -> ExportScope {
        let mut scope = ExportScope::default();
        for idx in indices {
            let Some(tc) = self.toolpath_configs.get(idx) else {
                continue;
            };
            if !tc.enabled {
                continue;
            }
            if current(idx) {
                scope.emitted.push(idx);
            } else {
                scope.left_out.push((idx, reason(idx)));
            }
        }
        scope
    }

    /// UX-7b (SAFETY): the export row of toolpath `index` when its stored
    /// result cuts into the mesh of its model, else `None`. The row is the
    /// toolpath name and the finding sentence of the diagnostic
    /// `safety.cuts_into_model`. K6: the export warns on it.
    #[must_use]
    pub fn model_gouge_refusal(&self, index: usize) -> Option<String> {
        let finding = self.result_at(index)?.stats.model_gouge?;
        // B2: a cut into the leave alone (a 3D Rough) is Caution, not an
        // export warning.
        if !finding.blocks_export() {
            return None;
        }
        let name = self
            .toolpath_configs
            .get(index)
            .map_or("?", |tc| tc.name.as_str());
        Some(format!("Toolpath '{name}': {}", finding.message()))
    }

    /// Export every enabled toolpath that holds a session result, through
    /// [`Self::export_program`], with the load report built over
    /// `sim_trace` ([`Self::build_tool_load_report`]). The CLI and the
    /// core tests take this door; the GUI and MCP build their own phases
    /// and take `export_program`.
    ///
    /// # Errors
    /// [`crate::gcode::EmitError`] only: no finding refuses (K6).
    #[instrument(skip(self, sim_trace))]
    pub fn export_gcode_checked(
        &self,
        sim_trace: Option<&crate::stock::simulation_cut::SimulationCutTrace>,
    ) -> Result<crate::gcode::ExportedGcode, crate::gcode::EmitError> {
        let report = self.build_tool_load_report(sim_trace);
        self.export_gcode_with_report(&report)
    }

    /// [`Self::export_gcode_checked`] with the load report the caller
    /// gives. An enabled toolpath with no session result is left out and
    /// named (K6, `export.op_left_out`). The overlay is the default with the
    /// controller options of the session's post config and machine profile
    /// (G5-G7, G10).
    ///
    /// # Errors
    /// [`crate::gcode::EmitError`] only: no finding refuses (K6).
    pub fn export_gcode_with_report(
        &self,
        report: &crate::tool_load::ToolLoadReport,
    ) -> Result<crate::gcode::ExportedGcode, crate::gcode::EmitError> {
        use crate::gcode::{GcodePhase, PhaseTool};

        let scope = self.export_scope(
            0..self.toolpath_configs.len(),
            |idx| self.result_at(idx).is_some(),
            |idx| {
                format!(
                    "'{}' has no current result: generate it",
                    self.toolpath_configs
                        .get(idx)
                        .map_or("?", |tc| tc.name.as_str())
                )
            },
        );

        // G-EXPORT-DATUM: re-express each toolpath in the shared export
        // datum BEFORE building phases, so the emitted program has one XY
        // zero across every setup. Owned here (not written back to the
        // session) so nothing downstream of generation is disturbed.
        let emitted: Vec<(usize, std::borrow::Cow<'_, crate::toolpath::Toolpath>)> = scope
            .emitted
            .iter()
            .filter_map(|&idx| {
                let result = self.result_at(idx)?;
                let shift = crate::gcode::export_datum_shift_for_toolpath(self, idx);
                Some((
                    idx,
                    crate::gcode::toolpath_in_export_datum(result.toolpath(), shift),
                ))
            })
            .collect();

        let mut by_index: Vec<(usize, GcodePhase<'_>)> = emitted
            .iter()
            .filter_map(|(idx, emitted_toolpath)| {
                let tc = self.toolpath_configs.get(*idx)?;
                // Tool identity (config id) + display number + name, so the
                // emitter inserts the post's tool-change block between
                // toolpaths that use different tools. Change detection keys
                // on the config id — NOT the user-curated T-number, which
                // collides in real projects (WANAKA: two distinct tools both
                // T1).
                let tool = self.toolpath_tool(tc).map(|t| PhaseTool {
                    id: t.id,
                    number: t.tool_number,
                    label: t.name.as_str(),
                });
                let phase = GcodePhase {
                    toolpath: emitted_toolpath.as_ref(),
                    spindle_rpm: crate::compute::catalog::effective_spindle_rpm(
                        &tc.operation,
                        self.post.spindle_speed,
                    ),
                    label: &tc.name,
                    tool,
                    // P0-D1 (2026-09-11): the operation's own coolant.
                    coolant: tc.coolant,
                    pre_gcode: tc.pre_gcode.as_deref(),
                    post_gcode: tc.post_gcode.as_deref(),
                    controller_compensation: crate::gcode::controller_compensation_for(tc),
                    drill: crate::gcode::phase_drill_for(tc),
                    // W2.5 (D10): this door emits the session slot, so the
                    // slot's tag names the feeds.
                    feeds: self
                        .feed_stage(*idx)
                        .map(|provenance| provenance.statement()),
                };
                Some((*idx, phase))
            })
            .collect();

        // G3 (SAFETY, 2026-10-08): a project with more than one setup gets
        // an M0 pause between the setups.
        let phases = crate::gcode::arrange_phases_by_setup(self, |idx| {
            let pos = by_index.iter().position(|(i, _)| *i == idx)?;
            Some(by_index.swap_remove(pos).1)
        });

        let mut overlay = crate::gcode::WizardOverlay::default();
        self.post
            .apply_controller_options(&self.machine, &mut overlay);
        self.export_program(crate::gcode::ExportRequest {
            phases,
            emitted: emitted.iter().map(|(idx, _)| *idx).collect(),
            left_out: scope.left_out,
            report,
            overlay: &overlay,
        })
    }

    /// W2.5 (D10): the caution line of each toolpath in `indices` whose
    /// session result was modulated from a simulation the session no
    /// longer holds. The export door and the GUI pre-flight modal read
    /// this one list. A toolpath with no session result (the GUI emits
    /// its previous geometry with planned feeds) adds no line.
    #[must_use]
    pub fn feeds_cautions(&self, indices: impl Iterator<Item = usize>) -> Vec<String> {
        indices
            .filter_map(|idx| {
                let tc = self.toolpath_configs.get(idx)?;
                if !tc.enabled {
                    return None;
                }
                let feeds = self.feed_stage(idx)?;
                feeds
                    .is_superseded()
                    .then(|| format!("'{}': {}", tc.name, feeds.statement()))
            })
            .collect()
    }

    /// F070: the gate-context parts of the toolpath at `index`, built once
    /// for the export load report and the GUI cut-metric cards.
    ///
    /// `None` when the index holds no config, the config names a removed
    /// tool, or the tool is outside its domain (F029): such a toolpath
    /// gets no verdict. The spans and the drill payload come from the
    /// stored result; the spans are `None` when a transform invalidated
    /// them (F2.2), so the gates fall back to the per-sample transit flag.
    #[must_use]
    pub fn load_context_parts(
        &self,
        index: usize,
    ) -> Option<crate::tool_load::LoadContextParts<'_>> {
        let tc = self.toolpath_configs.get(index)?;
        let tool_cfg = self.toolpath_tool(tc)?;
        let tool = crate::compute::cutter::build_cutter(tool_cfg).ok()?;
        let (family, pass_role) = tc.operation.feeds_style();
        let result = self.result_at(index);
        Some(crate::tool_load::LoadContextParts {
            toolpath_id: tc.id,
            tool,
            material: &self.stock.material,
            operation_family: crate::feeds::vendor_normalize::op_family_to_lut(family),
            pass_role: pass_role.into(),
            // S11: an op with no feed has no load context.
            operation_feed_rate_mm_min: tc.operation.feed_rate()?,
            operation_kind: tc.operation.op_type(),
            spans: result
                .filter(|r| r.annotated().spans_valid)
                .map(|r| r.annotated().spans.as_slice()),
            drill_op: result.and_then(|r| r.drill_op()).map(AsRef::as_ref),
        })
    }

    /// Compute the per-toolpath tool-load report against current state. Used
    /// by `get_tool_load_report` (MCP) and the export gate. Returns deflection
    /// + chipload populated; power is `Unmodeled(NotImplemented)` until
    ///
    /// Phase 1b lands the arc-engagement-driven power criterion.
    pub fn tool_load_report(&self) -> crate::tool_load::ToolLoadReport {
        let sim_trace = self
            .simulation
            .as_ref()
            .and_then(|simulation| simulation.cut_trace.as_deref());
        self.build_tool_load_report(sim_trace)
    }

    /// Compute the unified diagnostic list for a single toolpath
    /// using the session's own cached simulation. Consumers that
    /// have a sim trace held outside the core session (the GUI
    /// keeps its trace on the viz-side state) should call
    /// [`Self::diagnose_toolpath_with_trace`] instead — otherwise
    /// the load gates will read as `NeedsSimulation` even when a
    /// fresh trace exists elsewhere.
    ///
    /// Returns the list with [`crate::diagnostics::apply_supersession`]
    /// already applied — heuristic pre-sim hints vanish when sim
    /// evidence is current.
    pub fn diagnose_toolpath(
        &self,
        index: usize,
    ) -> Result<Vec<crate::diagnostics::Diagnostic>, SessionError> {
        let sim_trace = self
            .simulation
            .as_ref()
            .and_then(|sim| sim.cut_trace.as_deref());
        self.diagnose_toolpath_with_trace(index, sim_trace)
    }

    /// Same as [`Self::diagnose_toolpath`] but takes an explicit
    /// sim trace. Used by the GUI MCP handler where the active sim
    /// trace lives on the viz-side state, not on the core session.
    ///
    /// The heights come from THE session rule ([`Self::toolpath_heights`],
    /// geometry selection P3.1) and the feeds-calculator result from
    /// [`Self::feeds_result_for_toolpath`], so MCP consumers see the same
    /// heights / pre-sim hint diagnostics the GUI panel renders.
    pub fn diagnose_toolpath_with_trace(
        &self,
        index: usize,
        sim_trace: Option<&crate::stock::simulation_cut::SimulationCutTrace>,
    ) -> Result<Vec<crate::diagnostics::Diagnostic>, SessionError> {
        self.diagnose_toolpath_at_depth(index, sim_trace, DiagnoseDepth::Full)
    }

    /// W2b: the findings of the toolpath at `index` that can refuse
    /// generation, for [`crate::session::toolpath_run_state`],
    /// which the GUI header reads on every repaint.
    ///
    /// It is [`Self::diagnose_toolpath_with_trace`] without the three
    /// costly inputs: the project load report (built, not memoised), the
    /// feeds calculator and the Suggest records. No finding of those
    /// sources refuses generation (each load gate, feeds hint and Suggest
    /// record is `Critical` or lower, and none is the tool-kind refusal),
    /// so the refusing subset of this list equals the subset of the full
    /// list. The sentry `one_run_state_for_every_surface_w1` holds that.
    pub(crate) fn diagnose_toolpath_gate(
        &self,
        index: usize,
    ) -> Result<Vec<crate::diagnostics::Diagnostic>, SessionError> {
        self.diagnose_toolpath_at_depth(index, None, DiagnoseDepth::Gate)
    }

    fn diagnose_toolpath_at_depth(
        &self,
        index: usize,
        sim_trace: Option<&crate::stock::simulation_cut::SimulationCutTrace>,
        depth: DiagnoseDepth,
    ) -> Result<Vec<crate::diagnostics::Diagnostic>, SessionError> {
        let tc = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        // R2: an op with no tool has no tool checks, no feeds and no load
        // report. Its list holds the precondition findings (the no-tool
        // state first among them) and the model references.
        if tc.tool_id.is_none() {
            return Ok(crate::diagnostics::diagnose_toolpath_with_no_tool(
                tc.id,
                &tc.operation,
                &self.precondition_context_for_toolpath(tc),
                &self.model_ref_context_for_toolpath(tc),
            ));
        }
        let tool = self.tool_of(tc)?;
        let stale_defaults = crate::compute::validate::validate_one_toolpath(
            tc,
            Some(tool),
            &self.stock.material,
            self.stock_bbox().min.z,
        );
        // Build the load report from the *supplied* trace so callers
        // can plug in viz-side traces. `build_tool_load_report`
        // applies its own staleness check via `sim_trace_is_fresh_for`.
        let full = depth == DiagnoseDepth::Full;
        let report = full.then(|| self.build_tool_load_report(sim_trace));
        let load_verdict = report
            .as_ref()
            .and_then(|report| report.per_toolpath.iter().find(|v| v.toolpath_id == tc.id));

        // N4 (2026-09-10): resolve the toolpath's OWN heights, pins and
        // all. P3.1: through THE session rule, so the snapshot holds the
        // face top that generation cuts from. A selection that does not
        // resolve gives no heights snapshot; the selection diagnostics name
        // it.
        let heights = self.toolpath_heights(tc).ok().map(|answer| {
            crate::diagnostics::adapters::from_static_checks::ResolvedHeights::from_toolpath_heights(
                &answer,
                &tc.heights,
            )
        });
        let feeds_result = full
            .then(|| self.feeds_result_for_toolpath(tc, tool))
            .flatten();
        let suggest_warnings = full
            .then(|| self.suggest_warnings_for_toolpath(tc, tool))
            .flatten();
        let preconditions = self.precondition_context_for_toolpath(tc);
        let model_refs = self.model_ref_context_for_toolpath(tc);
        // Generation-time findings ride on the stats of this toolpath's own
        // generated result. Absent until it has been generated, which is
        // exactly when there is nothing to report.
        let stats = self
            .toolpath_configs
            .iter()
            .position(|t| t.id == tc.id)
            .and_then(|idx| self.results.get(&idx))
            .map(|r| &r.stats);

        let inputs = crate::diagnostics::ToolpathDiagnoseInputs {
            toolpath_id: tc.id,
            operation: &tc.operation,
            tool,
            heights: heights.as_ref(),
            feeds_result: feeds_result.as_ref(),
            suggest_warnings: suggest_warnings.as_deref(),
            load_verdict,
            stale_defaults: &stale_defaults,
            preconditions: Some(&preconditions),
            model_refs: Some(&model_refs),
            stats,
        };
        Ok(crate::diagnostics::diagnose_toolpath_inputs(&inputs))
    }

    /// The per-toolpath list of [`Self::diagnose_toolpath_with_trace`], plus
    /// every project SAFETY finding that names this toolpath.
    ///
    /// G-RAPIDFRAME (2026-10-01): the per-toolpath route took no collision
    /// evidence, so MCP `get_toolpath_diagnostics` never listed a rapid or
    /// holder collision, the findings that must stop a cut. This route reads
    /// the same [`ProjectEvidence`] as
    /// [`Self::diagnose_project_with_evidence`], so the two lists cannot
    /// disagree about one collision. The cut trace comes from the evidence
    /// too.
    ///
    /// It also appends the toolpath's own simulation-triage findings
    /// ([`crate::stock::sim_triage::SimulationTriage::own_toolpath_findings`]),
    /// so the per-op MCP list carries the rough coverage islands
    /// (G-ROUGHCOV) and the link and standing-material cuts. The cost is one
    /// triage build per call.
    ///
    /// A caller that also publishes the project list must use
    /// [`Self::diagnose_toolpath_with_trace`] for the per-toolpath half, or
    /// it publishes each safety finding twice.
    pub fn diagnose_toolpath_with_evidence(
        &self,
        index: usize,
        evidence: &ProjectEvidence<'_>,
    ) -> Result<Vec<crate::diagnostics::Diagnostic>, SessionError> {
        use crate::diagnostics::{Category, DiagnosticEvidence, Scope};
        let mut out = self.diagnose_toolpath_with_trace(index, evidence.cut_trace)?;
        let id = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?
            .id;
        let names_this_toolpath = |diag: &crate::diagnostics::Diagnostic| {
            diag.scope == (Scope::Toolpath { id })
                || matches!(
                    &diag.evidence,
                    Some(DiagnosticEvidence::Counts { offender_toolpath_ids, .. })
                        if offender_toolpath_ids.contains(&id)
                )
        };
        out.extend(
            self.diagnose_project_with_evidence(evidence)
                .into_iter()
                .filter(|diag| diag.category == Category::Safety && names_this_toolpath(diag)),
        );
        // Lead ruling 2026-10-23 (G-ROUGHCOV): the findings the simulation
        // triage computes itself for this toolpath (rough coverage islands,
        // link and standing-material cuts, entry load, plunge class, rest
        // slower than fresh). Each carries this toolpath's id in its scope;
        // no other source makes them, so none is listed twice. The triage
        // advisories (air-cut hotspots) are not appended.
        let triage = self.simulation_triage(evidence);
        out.extend(triage.own_toolpath_findings(id).cloned());
        // W1: the appended findings get the fit too, so no anchor on this
        // list names a field the op does not draw.
        if let Some(tc) = self.toolpath_configs.get(index) {
            let op_type = tc.operation.op_type();
            for d in &mut out {
                d.anchor = crate::diagnostics::presentation::fit_anchor(d.anchor, op_type);
            }
        }
        Ok(out)
    }

    /// Build a [`ModelRefContext`](crate::diagnostics::adapters::from_model_refs::ModelRefContext) for a single toolpath (F-023).
    /// Captures whether the toolpath's `model_id` resolves against the
    /// project's loaded models so the unified diagnostic stream emits
    /// the same "Selected model missing" signal the GUI banner has
    /// always shown. GUI consumers snapshot this context before borrowing
    /// their editable toolpath entry.
    pub fn model_ref_context_for_toolpath(
        &self,
        tc: &super::ToolpathConfig,
    ) -> crate::diagnostics::diagnose::ModelRefContext {
        crate::diagnostics::diagnose::ModelRefContext {
            model_id: tc.model_id,
            model_resolved: self.models.iter().any(|m| m.id == tc.model_id),
        }
    }

    /// Build a [`PreconditionContext`](crate::diagnostics::adapters::from_preconditions::PreconditionContext) for a single toolpath. Captures
    /// the target model's geometry kind (so the project-curve
    /// preconditions can verify a curve / mesh source is in scope), the
    /// holes of a drill op's selection, the refusal of the selection of
    /// any op, and the live stock pin diameters.
    ///
    /// Pure helper — no I/O, no mutation. Safe to call repeatedly per
    /// diagnose round. GUI consumers snapshot this context before borrowing
    /// their editable toolpath entry.
    pub fn precondition_context_for_toolpath(
        &self,
        tc: &super::ToolpathConfig,
    ) -> crate::diagnostics::diagnose::PreconditionContext {
        use crate::diagnostics::diagnose::{PreconditionContext, TargetModelGeometry};

        let target_model =
            self.models
                .iter()
                .find(|m| m.id == tc.model_id)
                .map(|m| TargetModelGeometry {
                    has_polygons: m.items.ring_count() > 0,
                    has_mesh: m.mesh.is_some(),
                });
        // Geometry selection P2.3c, P2.4: the holes and the refusal come
        // from the resolution generation reads (`selection_status`, the
        // memo), for every op.
        let (selection_holes, selection_refusal) =
            match self.selection_status(tc, super::super::SelectionTarget::Op) {
                Ok(report) => (
                    report.holes.len(),
                    report.refusal.as_ref().map(ToString::to_string),
                ),
                Err(error) => (0, Some(error.to_string())),
            };

        let any_loaded_model_has_mesh = self.models.iter().any(|m| m.mesh.is_some());
        // UX-1c: the fresh-stock refusal of a buried face, from THE heights
        // rule that generation reads.
        let buried_face_refusal = self.toolpath_heights(tc).ok().and_then(|answer| {
            answer
                .buried_face_refusal(&tc.operation, &tc.heights, tc.stock_source)
                .map(|refusal| refusal.to_string())
        });

        PreconditionContext {
            target_model,
            any_loaded_model_has_mesh,
            // The generator drills the live stock pins (see
            // `generate_toolpath`), so the check counts them here.
            stock_alignment_pin_diameters: self
                .stock
                .alignment_pins
                .iter()
                .map(|p| p.diameter)
                .collect(),
            selection_holes,
            selection_refusal,
            selection_empty: tc
                .operation
                .selection()
                .is_some_and(crate::selection::Selection::is_empty),
            buried_face_refusal,
            stock_thickness_refusal: self.stock_thickness_refusal(),
            no_tool_state: self
                .toolpath_configs
                .iter()
                .position(|t| t.id == tc.id)
                .and_then(|index| self.no_tool_state(index)),
        }
    }

    /// Run the feeds calculator for a single toolpath against the session's
    /// material/machine/post — used by [`Self::diagnose_toolpath_with_trace`]
    /// to surface the same feeds warnings + pre-sim heuristic hints the GUI
    /// params panel emits.
    ///
    /// Returns `None` for op kinds whose feeds_style doesn't model cutting
    /// (e.g. tool-change-only ops), in which case the heuristic hint
    /// adapter is skipped.
    pub fn feeds_result_for_toolpath(
        &self,
        tc: &super::ToolpathConfig,
        tool: &ToolConfig,
    ) -> Option<crate::feeds::FeedsResult> {
        // A refused tool × operation pairing (e.g. flat endmill on a
        // Scallop op) maps to `None` here — the diagnostics layer
        // treats "no recipe available" the same as "feeds_style
        // doesn't model cutting".
        crate::feeds::suggest::feeds_result_for_operation(
            &tc.operation,
            tool,
            &self.stock.material,
            &self.machine,
            crate::feeds::embedded_vendor_lut(),
            self.post.spindle_strategy,
        )
        .ok()
    }

    /// The records of the Suggest run behind this toolpath's recipe, for
    /// [`Self::diagnose_toolpath_with_trace`] (ruling R4, 2026-09-24: no
    /// invisible calculation).
    ///
    /// The session does not store the records of the last Suggest press.
    /// This helper runs Suggest again on the operation, with the context
    /// that [`Self::cutter_op_profile`] builds, the same way
    /// [`Self::feeds_result_for_toolpath`] runs the calculator again. The
    /// records describe what Suggest does to the recipe. They can differ
    /// from what the operation holds after a speeds-only apply or a hand
    /// edit.
    ///
    /// Returns `None` when no suggested field (feed, plunge, RPM, stepover,
    /// depth per pass) carries a Suggest source in `feeds_provenance`, so a
    /// hand-typed recipe gets no record of a calculation that did not occur.
    /// Feeds contract P5 (K2b, K6): the `OutsideEnvelope` warning of a stated
    /// point outside the safe envelope is added in every case, a typed value
    /// included, so the GUI ribbon and MCP read one list.
    /// A speeds-only apply still gets the record: the dial computed an
    /// engagement that the operation does not hold, and the Caution says so.
    /// An operation that the operator rewrote in every
    /// field gets no record. Also `None` when Suggest refuses the tool and
    /// operation pair.
    pub fn suggest_warnings_for_toolpath(
        &self,
        tc: &super::ToolpathConfig,
        tool: &ToolConfig,
    ) -> Option<Vec<crate::feeds::suggest::SuggestWarning>> {
        use crate::feeds::ProvenanceSource;
        use crate::feeds::suggest::{StockContext, SuggestContext, SuggestForOperationInput};

        let from_suggest = |v: &Option<crate::feeds::ValueProvenance>| {
            v.as_ref().is_some_and(|p| {
                matches!(
                    p.source,
                    ProvenanceSource::VendorLut
                        | ProvenanceSource::Formula
                        | ProvenanceSource::EdgeRadiusFloor
                        // G10 (R7): the plunge and the ramp now stamp the
                        // rule that set them; both are Suggest's own.
                        | ProvenanceSource::PublishedRule
                        | ProvenanceSource::RepoRule
                )
            })
        };
        let prov = &tc.feeds_provenance;
        let any_from_suggest = [
            &prov.feed_rate,
            &prov.plunge_rate,
            &prov.spindle_rpm,
            &prov.stepover,
            &prov.depth_per_pass,
            &prov.ramp_feed_rate,
        ]
        .into_iter()
        .any(from_suggest);
        // K2b, K6 (feeds contract P5, ruling Q6): a stated point outside the
        // safe envelope is a warning that names the binding limit. A typed
        // value gets it too, so it does not wait for a Suggest stamp. This
        // helper is the one source of the GUI ribbon and of MCP.
        let outside = self
            .safe_envelope(tc)
            .and_then(Result::ok)
            .and_then(|envelope| match (envelope.stated, envelope.operator) {
                (
                    Some(point),
                    Some(crate::feeds::envelope::Placement::Outside { binding, margin }),
                ) => Some(crate::feeds::suggest::SuggestWarning::OutsideEnvelope {
                    binding,
                    margin,
                    ap_mm: point.ap_mm,
                    ae_mm: point.ae_mm,
                }),
                (Some(_) | None, Some(crate::feeds::envelope::Placement::Inside { .. }) | None)
                | (None, Some(crate::feeds::envelope::Placement::Outside { .. })) => None,
            });
        if !any_from_suggest {
            return outside.map(|w| vec![w]);
        }
        let stock_ctx = StockContext::from_stock_bbox(self.stock_bbox(), self.stock.padding);
        let model_bboxes = self.collect_model_bboxes();
        let model_bbox = model_bboxes
            .iter()
            .find(|(id, _)| *id == tc.model_id)
            .map(|(_, b)| b);
        crate::feeds::suggest::suggest_for_operation(SuggestForOperationInput {
            operation: &tc.operation,
            tool,
            machine: &self.machine,
            material: &self.stock.material,
            lut: crate::feeds::embedded_vendor_lut(),
            spindle_strategy: self.post.spindle_strategy,
            context: SuggestContext {
                model_bbox,
                stock: Some(&stock_ctx),
                dressups: Some(&tc.dressups),
                ..SuggestContext::default()
            },
        })
        .ok()
        .map(|s| {
            let mut warnings = s.warnings;
            warnings.extend(outside);
            warnings
        })
    }

    /// Project-wide diagnostics derived from the
    /// [`ProjectDiagnostics`](crate::session::diagnostics_types::ProjectDiagnostics) snapshot using the session's cached
    /// simulation. Consumers with sim evidence outside the session
    /// (the GUI) should call [`Self::diagnose_project_with_evidence`].
    pub fn diagnose_project(&self) -> Vec<crate::diagnostics::Diagnostic> {
        // Batch entry point — same full-evidence semantics as
        // `diagnostics()`, including the holder-collision sweep.
        let diag = self.diagnostics();
        let mut out = crate::diagnostics::diagnose_project_diagnostics(&diag);
        out.extend(self.tool_name_diagnostics());
        out.extend(self.tool_stickout_diagnostics());
        out
    }

    /// The tool-scoped static findings: one `tool.name_size_mismatch`
    /// for each tool whose name names a tip size the geometry does not
    /// have. It reads no simulation, so it is cheap.
    pub fn tool_name_diagnostics(&self) -> Vec<crate::diagnostics::Diagnostic> {
        self.tools()
            .iter()
            .flat_map(crate::diagnostics::adapters::from_static_checks::diagnostics_from_tool_name)
            .collect()
    }

    /// The tool-scoped finding of FC0 item 7: one
    /// `tool.stickout_shorter_than_flute` for each tool whose stick-out is
    /// shorter than its flute. It reads no simulation, so it is cheap.
    pub fn tool_stickout_diagnostics(&self) -> Vec<crate::diagnostics::Diagnostic> {
        self.tools()
            .iter()
            .flat_map(
                crate::diagnostics::adapters::from_static_checks::diagnostics_from_tool_stickout,
            )
            .collect()
    }

    /// Same as [`Self::diagnose_project`] but takes a borrow view
    /// over sim evidence. Used by the GUI MCP handler.
    pub fn diagnose_project_with_evidence(
        &self,
        evidence: &ProjectEvidence<'_>,
    ) -> Vec<crate::diagnostics::Diagnostic> {
        let diag = self.diagnostics_with_evidence(evidence);
        let mut out = crate::diagnostics::diagnose_project_diagnostics(&diag);
        out.extend(self.tool_name_diagnostics());
        out.extend(self.tool_stickout_diagnostics());
        out
    }
}

impl ProjectSession {
    // F121: the freshness rule and the load-report builder are session
    // policy, so they live here and not in `gcode/`.

    /// Check whether the cached sim trace's provenance still matches the
    /// project's current state. PR-4: this is the staleness signal that
    /// drives [`UnmodeledReason::StaleSimulation`](crate::tool_load::verdict::UnmodeledReason::StaleSimulation) emissions out of the
    /// load gates — when current toolpaths or tools differ from the
    /// trace's recorded hashes, the gates can no longer be honoured
    /// against the current configs.
    ///
    /// Returns `true` when the trace is safe to evaluate against. A trace
    /// without a `provenance` block carries no evidence of what it was
    /// simulated against, so L7 (operator ruling, 2026-09-16) reads it as
    /// STALE. The arm used to return `true` for backward compatibility
    /// with a pre-provenance schema, which let a gate be honoured against
    /// a configuration nobody had checked.
    ///
    /// Cheap: only iterates enabled toolpaths and hashes the cached
    /// annotated toolpath. Doesn't recompute anything heavy.
    pub fn sim_trace_is_fresh(&self, trace: &SimulationCutTrace) -> bool {
        // L7: no provenance, no evidence. The simulator stamps the block on
        // every trace it produces (`compute/simulate.rs`), so a trace without
        // one came from somewhere that cannot say what it simulated.
        if trace.provenance.is_none() {
            return false;
        }
        (0..self.toolpath_count()).all(|index| {
            self.toolpath_config_at(index).is_none_or(|tc| !tc.enabled)
                || self.sim_trace_is_fresh_for(trace, index)
        })
    }

    /// [`Self::sim_trace_is_fresh`] for ONE operation: its own provenance only.
    ///
    /// G-STALECARDS (2026-09-24). The project-wide answer let one edited
    /// operation stale every other operation's verdicts, and a re-run could
    /// never clear it while the edited operation had no core result. The load
    /// report and the cut-metric cards now judge each operation by its own
    /// row: it holds a core result, the trace carved that result, and the
    /// configuration has not moved since. A config-hash entry that is missing
    /// (a pre-PR-4 trace) reads as a match, as before.
    pub fn sim_trace_is_fresh_for(&self, trace: &SimulationCutTrace, index: usize) -> bool {
        let Some(tc) = self.toolpath_config_at(index) else {
            return false;
        };
        let Some(provenance) = trace.provenance.as_ref() else {
            return false;
        };
        // A toolpath with no core result was not simulated: the trace covers
        // fewer toolpaths than the project now holds.
        let Some(result) = self.result_at(index) else {
            return false;
        };
        let Some(expected) = provenance.toolpath_hashes.get(&tc.id) else {
            return false;
        };
        if *expected != crate::compute::simulate::hash_toolpath(&result.annotated().toolpath) {
            return false;
        }
        provenance
            .operation_config_hashes
            .get(&tc.id)
            .is_none_or(|expected_cfg| {
                *expected_cfg == crate::compute::simulate::hash_operation_config(&tc.operation)
            })
    }

    /// Build a `ToolLoadReport` from a `ProjectSession`. Delegates each
    /// toolpath to `tool_load::evaluate_toolpath` (the single verdict
    /// assembly site), which runs all three criteria: `chipload` and
    /// `power` (per-sample, require `sim_trace`) and `deflection`
    /// (per-sample tip deflection from cutting force).
    ///
    /// PR-4: when `sim_trace` is `Some` but a row's own provenance fails
    /// (`sim_trace_is_fresh_for`, per operation since G-STALECARDS), that
    /// row's evaluators run with `sim_trace = None` and the resulting
    /// `Unmodeled::SimulationRequired` verdicts are post-processed into
    /// `Unmodeled::StaleSimulation`. The diagnostics adapter renders
    /// `StaleSimulation` as `DiagnosticState::StaleEvidence`, which the
    /// UI displays as a neutral "re-run simulation" hint rather than a
    /// scary warning.
    pub fn build_tool_load_report(
        &self,
        sim_trace: Option<&SimulationCutTrace>,
    ) -> crate::tool_load::ToolLoadReport {
        // Freshness gate — see [`Self::sim_trace_is_fresh`] for the contract.
        // G-STALECARDS: freshness is judged PER OPERATION, below. A trace
        // exists or not for the whole report; each row then uses it only when
        // its own provenance holds.
        let offered_trace = sim_trace;

        let mut per_toolpath = Vec::new();
        for (idx, tc) in self.toolpath_configs().iter().enumerate() {
            if !tc.enabled {
                continue;
            }
            // F070: the one context door. `None` for a removed tool or a tool
            // outside its domain (F029): skip rather than fabricate a verdict.
            let Some(parts) = self.load_context_parts(idx) else {
                continue;
            };
            let machine = self.machine();
            // The export uses the strict bands, `ToleranceBands::default()`.
            let strict_tolerance = crate::tool_load::ToleranceBands::default();
            let row_is_fresh =
                offered_trace.is_some_and(|trace| self.sim_trace_is_fresh_for(trace, idx));
            let sim_trace = offered_trace.filter(|_| row_is_fresh);
            // Phase 6 task 2: `tool_load::evaluate_toolpath` is the single
            // verdict assembly site.
            let mut verdict = crate::tool_load::evaluate_toolpath(
                &parts.ctx(),
                sim_trace,
                Some(machine),
                &strict_tolerance,
            );
            // Phase 4 — the kinematic reading. `evaluate_toolpath` cannot build
            // it: `ToolpathLoadContext` carries neither the emitted `Toolpath`
            // nor the operation's `plunge_rate`. This site holds the whole
            // `ProjectSession`, so it fills the slot from the session's single
            // producer, `kinematic_utilization_of`. This caller holds no result
            // of its own, so it takes the by-index convenience over the
            // session's stored result; the MCP narration, which DOES hold one,
            // passes its own move list to `_of` instead.
            // Phase 3: `sim_trace` is the trace this report is judging (already
            // `None` when it was thrown away as stale), so it is what decides
            // whether the reading describes planned or emitted feeds.
            verdict.kinematic_utilization = self.kinematic_utilization_for(idx, sim_trace);
            // PR-4: a row whose trace was thrown away as stale rewrites its
            // `SimulationRequired` verdicts to `StaleSimulation`, so the
            // diagnostics adapter surfaces `StaleEvidence` instead of
            // `NeedsSimulation`. G-STALECARDS: only THIS row; the others keep
            // their verdicts. F197: one method names every field.
            if offered_trace.is_some() && !row_is_fresh {
                verdict.mark_simulation_stale();
            }
            per_toolpath.push(verdict);
        }

        crate::tool_load::ToolLoadReport { per_toolpath }
    }
}
