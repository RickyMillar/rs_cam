//! G-code export and the per-toolpath report contexts.
//!
//! Holds the export entries and the context builders the diagnostics adapters
//! read — model reference, preconditions, heights and feeds. Split out of
//! `session/compute.rs` (P4).

use tracing::instrument;

use crate::compute::tool_config::{ToolConfig, ToolId};
use crate::session::{ProjectEvidence, ProjectSession, SessionError};

impl ProjectSession {
    /// **F067: the one checked export door.** The GUI, MCP and the CLI
    /// all emit through here, so one project state gives one program and
    /// one set of checks on every surface.
    ///
    /// In order:
    /// 1. [`crate::gcode::emit_checked`]: the inch and controller
    ///    compensation refusals, the load gate, the program. The post and
    ///    the inter-setup retract come from THIS session's post config,
    ///    never from a caller's copy (F072).
    /// 2. The machine-safety pass ([`validate_machine_safety`]) on the
    ///    program as emitted, BEFORE any rewrite, while the rapids are
    ///    still `G0`. The clearance plane is the emitted retract plane of
    ///    the toolpaths in `request.emitted`
    ///    ([`emission_frame_clearance_z`], R11). The feed cap is the
    ///    machine's travel rate, `max_feed_mm_min`: the rapid rewrite
    ///    writes rapids as feeds at the travel rate, so the cutting
    ///    ceiling would flag every rewritten rapid (T-9). The depth-floor
    ///    check stays off: no caller has a stock-bottom limit to hand it.
    /// 3. The high-feedrate rapid→feed rewrite, when the post config asks
    ///    for it (`high_feedrate_mode`).
    ///
    /// F033: before step 1, an emitted toolpath whose height planes break
    /// the order refuses ([`crate::gcode::ExportRefusal::HeightOrder`],
    /// [`Self::height_order_refusal`]).
    ///
    /// Until F067 only the GUI/MCP door ran 2 and 3, so the CLI emitted
    /// `G0` rapids where the GUI emitted `G1` traverses, and no CLI file
    /// was safety-checked. The findings are advisory on every surface: no
    /// door refuses on them.
    ///
    /// [`validate_machine_safety`]: crate::export::gcode_validator::validate_machine_safety
    /// [`emission_frame_clearance_z`]: crate::export::gcode_validator::emission_frame_clearance_z
    pub fn export_program(
        &self,
        request: crate::gcode::ExportRequest<'_>,
    ) -> Result<crate::gcode::ExportedGcode, crate::gcode::ExportRefusal> {
        use crate::export::gcode_validator::{
            MachineSafety, Severity, emission_frame_clearance_z, validate_machine_safety,
        };

        let crate::gcode::ExportRequest {
            phases,
            emitted,
            report,
            policy,
            overlay,
        } = request;
        // F033 (operator ruling 2): an emitted toolpath whose height planes
        // break the order refuses the whole export, before any emit. The
        // rows are the text of `HeightOrderRefusal`, which the edit doors
        // and generation give too.
        let height_rows: Vec<String> = emitted
            .iter()
            .filter_map(|idx| self.toolpath_configs.get(*idx))
            .filter_map(|tc| self.height_order_refusal(tc))
            .map(|refusal| refusal.to_string())
            .collect();
        if !height_rows.is_empty() {
            return Err(crate::gcode::ExportRefusal::HeightOrder { rows: height_rows });
        }
        let post_config = &self.post;
        let post = post_config.format.definition();
        let crate::gcode::CheckedProgram {
            mut gcode,
            load_outcome,
        } = crate::gcode::emit_checked(&phases, post, post_config.safe_z, report, policy, overlay)?;

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
            feeds_cautions: self.feeds_cautions(emitted.iter().copied()),
        })
    }

    /// Export every enabled toolpath that holds a session result, through
    /// [`Self::export_program`]. The CLI and the core tests take this
    /// door; the GUI and MCP build their own phases (they may emit an
    /// edited operation's previous geometry) and take `export_program`.
    ///
    /// The load gate reads [`crate::gcode::project_load_report`] over
    /// `sim_trace`. The overlay is the default with the controller options
    /// of the session's post config and machine profile (G5-G7, G10).
    #[instrument(skip(self, sim_trace))]
    pub fn export_gcode_checked(
        &self,
        sim_trace: Option<&crate::stock::simulation_cut::SimulationCutTrace>,
        policy: crate::gcode::ToolLoadExportPolicy,
    ) -> Result<crate::gcode::ExportedGcode, crate::gcode::ExportRefusal> {
        use crate::gcode::{GcodePhase, PhaseTool};

        let report = crate::gcode::project_load_report(self, sim_trace);

        // G-EXPORT-DATUM: re-express each toolpath in the shared export
        // datum BEFORE building phases, so the emitted program has one XY
        // zero across every setup. Owned here (not written back to the
        // session) so nothing downstream of generation is disturbed.
        let emitted: Vec<(usize, std::borrow::Cow<'_, crate::toolpath::Toolpath>)> = self
            .toolpath_configs
            .iter()
            .enumerate()
            .filter_map(|(idx, tc)| {
                if !tc.enabled {
                    return None;
                }
                let result = self.get_result(idx)?;
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
                let tool = self
                    .tools()
                    .iter()
                    .find(|t| t.id.0 == tc.tool_id)
                    .map(|t| PhaseTool {
                        id: t.id.0,
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
            report: &report,
            policy,
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
        let tool_cfg = self.get_tool(ToolId(tc.tool_id))?;
        let tool = crate::compute::cutter::build_cutter(tool_cfg).ok()?;
        let (family, pass_role) = tc.operation.feeds_style();
        let result = self.get_result(index);
        Some(crate::tool_load::LoadContextParts {
            toolpath_id: tc.id,
            tool,
            material: &self.stock.material,
            operation_family: crate::feeds::vendor_normalize::op_family_to_lut(family),
            pass_role: pass_role.into(),
            operation_feed_rate_mm_min: tc.operation.feed_rate(),
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
        crate::gcode::project_load_report(self, sim_trace)
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
        let tc = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        let tool = self
            .find_tool_by_raw_id(tc.tool_id)
            .ok_or(SessionError::ToolNotFound(ToolId(tc.tool_id)))?;
        let stale_defaults = crate::compute::validate::validate_one_toolpath(
            tc,
            Some(tool),
            &self.stock.material,
            self.stock_bbox().min.z,
        );
        // Build the load report from the *supplied* trace so callers
        // can plug in viz-side traces. `gcode::project_load_report`
        // applies its own staleness check via `sim_trace_is_fresh`.
        let report = crate::gcode::project_load_report(self, sim_trace);
        let load_verdict = report.per_toolpath.iter().find(|v| v.toolpath_id == tc.id);

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
        let feeds_result = self.feeds_result_for_toolpath(tc, tool);
        let suggest_warnings = self.suggest_warnings_for_toolpath(tc, tool);
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
        if !any_from_suggest {
            return None;
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
        .map(|s| s.warnings)
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

    /// Same as [`Self::diagnose_project`] but takes a borrow view
    /// over sim evidence. Used by the GUI MCP handler.
    pub fn diagnose_project_with_evidence(
        &self,
        evidence: &ProjectEvidence<'_>,
    ) -> Vec<crate::diagnostics::Diagnostic> {
        let diag = self.diagnostics_with_evidence(evidence);
        let mut out = crate::diagnostics::diagnose_project_diagnostics(&diag);
        out.extend(self.tool_name_diagnostics());
        out
    }
}
