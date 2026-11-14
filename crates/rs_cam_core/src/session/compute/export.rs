//! G-code export: the checked export door, the export warnings and scope,
//! and the feeds cautions. Split out of `session/compute.rs` (P4). The
//! report contexts and diagnostics live in `toolpath_diagnostics.rs`, the
//! freshness rule and the measured lineage in `measured.rs` (P14, L6).

use tracing::instrument;

use crate::session::ProjectSession;
use crate::stock::simulation_cut::SimulationCutTrace;

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
            sim_trace,
            overlay,
        } = request;
        let warnings = self.export_warnings(&emitted, &left_out, report, sim_trace);
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
            emitted_feeds: emitted
                .iter()
                .filter_map(|&idx| {
                    Some(crate::session::NamedEmittedFeeds {
                        toolpath: self.toolpath_configs.get(idx)?.name.clone(),
                        feeds: self.emitted_feeds(idx)?,
                    })
                })
                .collect(),
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
    /// load rows read; `sim_trace` the cut trace that report was built
    /// from (the K5 simulation rows read it for freshness).
    #[must_use]
    pub fn export_warnings(
        &self,
        emitted: &[usize],
        left_out: &[(usize, String)],
        report: &crate::tool_load::ToolLoadReport,
        sim_trace: Option<&SimulationCutTrace>,
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
                Gate::GougeWarns => {
                    for &idx in emitted {
                        if let Some(row) = self.model_gouge_export_warning(idx) {
                            push(ids::SAFETY_CUTS_INTO_MODEL, name_at(idx), row);
                        }
                    }
                }
                Gate::CentreCutPlunge => {
                    for &idx in emitted {
                        // P22-0c-3 (one answer per move): a fresh
                        // simulation measured the core on the live stock,
                        // so its answer (`Gate::CentreCutDescent`) stands
                        // for the static one.
                        if self.core_contact_measured(idx, sim_trace).is_some() {
                            continue;
                        }
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
                            let rows = crate::compute::simulate::non_centre_cutting_warning_rows(
                                simulation,
                                &[tc.id],
                                |id| {
                                    self.toolpath_configs
                                        .iter()
                                        .find(|t| t.id == id)
                                        .map(|t| t.name.as_str())
                                },
                                |id| self.contact_frame_of(id),
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
                        // P22-0c-4 (ruling Q1): the fed shank, holder and
                        // shoe contacts that a fresh simulation measured.
                        let stock_checks = self.holder_checks_with_stock(Vec::new());
                        for tc in emitted.iter().filter_map(|i| self.toolpath_configs.get(*i)) {
                            let measured = stock_checks.iter().any(|(id, check)| {
                                *id == tc.id
                                    && matches!(
                                        check,
                                        crate::stock::collision::HolderCollisionCheck::MeasuredOnStock(n)
                                            if *n > 0
                                    )
                            });
                            if !measured {
                                continue;
                            }
                            if let Some(row) = crate::compute::simulate::body_contact_row(
                                &simulation.body_contacts,
                                tc.id,
                                self.contact_frame_of(tc.id),
                            ) {
                                push(
                                    ids::PROJECT_HOLDER_COLLISION,
                                    tc.name.clone(),
                                    format!("Toolpath '{}': {row}", tc.name),
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
                        // K5 ReachFlute: a fresh simulation measured the
                        // flute reach, so its row (`Gate::FluteOverreach`)
                        // stands for the pre-simulation one.
                        let measured = self.flute_reach_measured(idx, sim_trace).is_some();
                        for row in report.rows().iter().filter(|row| {
                            // The G-CENTRECUT gates print CentreCut.
                            row.rule() != crate::compute::feasibility::PhysicalRule::CentreCut
                                && !(measured
                                    && row.rule()
                                        == crate::compute::feasibility::PhysicalRule::ReachFlute)
                        }) {
                            push(
                                row.id(),
                                name_at(idx),
                                format!("Toolpath '{}': {}", name_at(idx), row.text()),
                            );
                        }
                    }
                }
                Gate::SimOutsideEnvelope => {
                    for &idx in emitted {
                        if let Some(text) = self
                            .envelope_placement_measured(idx, sim_trace)
                            .and_then(|p| p.outside_row())
                        {
                            push(
                                ids::PROJECT_SIM_OUTSIDE_ENVELOPE,
                                name_at(idx),
                                format!("Toolpath '{}': {text}", name_at(idx)),
                            );
                        }
                    }
                }
                Gate::UnmodulatedOverLimit => {
                    // P30-2: a superseded result gets no second line; the
                    // feeds caution of W2.5 names it.
                    for &idx in emitted {
                        if self.feed_stage(idx) != Some(crate::session::FeedStage::Planned) {
                            continue;
                        }
                        if let Some(face) = self.stated_row_over_until_modulated(idx) {
                            push(
                                ids::EXPORT_STATED_FEED_OVER_LIMIT,
                                name_at(idx),
                                format!(
                                    "Toolpath '{}': {face}; the export carries the stated feed, \
                                     a simulation with feed modulation caps this cut",
                                    name_at(idx)
                                ),
                            );
                        }
                    }
                }
                Gate::FluteOverreach => {
                    for &idx in emitted {
                        if let Some(crate::compute::simulate::FluteReach::Over(text)) =
                            self.flute_reach_measured(idx, sim_trace)
                        {
                            push(
                                ids::TOOL_FLUTE_OVERREACH,
                                name_at(idx),
                                format!("Toolpath '{}': {text}", name_at(idx)),
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
    pub fn model_gouge_export_warning(&self, index: usize) -> Option<String> {
        let finding = self.result_at(index)?.stats.model_gouge?;
        // B2: a cut into the leave alone (a 3D Rough) is Caution, not an
        // export warning.
        if !finding.warns_at_export() {
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
        self.export_gcode_with(&report, sim_trace)
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
        let sim_trace = self
            .simulation
            .as_ref()
            .and_then(|simulation| simulation.cut_trace.as_deref());
        self.export_gcode_with(report, sim_trace)
    }

    /// [`Self::export_gcode_with_report`] with the cut trace the report was
    /// built from (K5: the simulation rows read it for freshness).
    fn export_gcode_with(
        &self,
        report: &crate::tool_load::ToolLoadReport,
        sim_trace: Option<&SimulationCutTrace>,
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
                    // W2.5 (D10), P30-1: this door emits the session slot,
                    // so the record of that slot names the feeds.
                    feeds: self.emitted_feeds(*idx),
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
            sim_trace,
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
}
