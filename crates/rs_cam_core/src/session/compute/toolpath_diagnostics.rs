//! The per-toolpath report contexts and diagnostics: the gate-context
//! parts, the tool-load report, the diagnose doors, the model-reference,
//! precondition and feeds contexts, and the project diagnostics. Split out
//! of `session/compute/export.rs` (feeds contract P14, review L6); a pure move.

use crate::compute::tool_config::ToolConfig;
use crate::session::{ProjectEvidence, ProjectSession, SessionError};

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

impl ProjectSession {
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
        // P11b2: the lift of the stated cut, by Suggest's own rule (the
        // engaged diameter at the stated depth, `feeds::effective_diameter`).
        let (stated_ap, stated_ae, _) = tc.operation.cut_spec().calc_inputs(tool_cfg.diameter);
        let commanded_thinning_lift = stated_ap.zip(stated_ae).map(|(ap, ae)| {
            let d_eng = crate::feeds::engagement::EngagedDiameter::of_hint(
                tool_cfg.geometry_hint(),
                tool_cfg.diameter,
                // P22-0c-6b: the body's cutting cap (ruling 1).
                tool.body().max_cut_diameter_mm(),
                ap,
            );
            // P23a: the width of Suggest's lift (the pass ceiling of an
            // adaptive plan; `PowerCases::full_feed_width`).
            let width = crate::feeds::engagement::PowerCases::of_operation(&tc.operation)
                .full_feed_width(ae, tool_cfg.diameter);
            // P29-1: the lift of the one load, on the stated cut at the
            // planner width.
            crate::feeds::cut_load::CutLoad::thinning_lift(
                &crate::feeds::engagement::Engagement::from_cut(
                    crate::feeds::envelope::CutPoint {
                        ap_mm: ap,
                        ae_mm: width,
                    },
                    tool_cfg.geometry_hint(),
                    d_eng,
                    None,
                ),
                // P29-3: the thinning rule of the op's body cut.
                tc.operation.cuts().body_facts().thins_chip,
            )
        });
        // S11: an op with no feed has no load context.
        let op_feed = tc.operation.feed_rate()?;
        Some(crate::tool_load::LoadContextParts {
            toolpath_id: tc.id,
            tool,
            material: &self.stock.material,
            operation_family: crate::feeds::vendor_normalize::op_family_to_lut(family),
            pass_role: pass_role.into(),
            operation_feed_rate_mm_min: op_feed,
            // P20e: the steady feed through THE op ceiling, the cap the
            // modulator reads (G-FEEDMAX).
            steady_feed: self
                .machine
                .op_steady_feed(op_feed, tc.dressups.feed_max_rate),
            operation_kind: tc.operation.op_type(),
            spans: result
                .filter(|r| r.annotated().spans_valid)
                .map(|r| r.annotated().spans.as_slice()),
            drill_op: result.and_then(|r| r.drill_op()).map(AsRef::as_ref),
            commanded_thinning_lift,
            // P17: the radial leave of the op, the rough deflection limit.
            radial_stock_to_leave_mm: tc.operation.radial_stock_to_leave_mm(),
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
            &self.machine,
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
        // K5 ReachFlute: the simulation stage, when a fresh run measured it.
        let flute_reach = self.flute_reach_measured(index, sim_trace);
        // G-CENTRECUT (P22-0c-3): the core answer of a fresh run.
        let core_contact = self.core_contact_measured(index, sim_trace);
        // K7: the measured engagement on the envelope, when a fresh run has it.
        let envelope_placement = self.envelope_placement_measured(index, sim_trace);
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
            flute_reach: flute_reach.as_ref(),
            core_contact: core_contact.as_ref(),
            envelope_placement: envelope_placement.as_ref(),
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
        // P22-0c-3: a contact row the toolpath list already holds (the
        // measured core or flute answer of a fresh run) is not listed twice.
        let own_ids: Vec<crate::diagnostics::DiagnosticId> =
            out.iter().map(|diag| diag.id.clone()).collect();
        out.extend(
            self.diagnose_project_with_evidence(evidence)
                .into_iter()
                .filter(|diag| diag.category == Category::Safety && names_this_toolpath(diag))
                .filter(|diag| !own_ids.contains(&diag.id)),
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
            // D12: the stop that generation gives (`not_set_for_tool`).
            not_set: self
                .toolpath_tool(tc)
                .and_then(|tool| tc.operation.not_set_for_tool(tool)),
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
            self.post.spindle_policy(),
        )
        .ok()
    }

    /// Feeds contract P21c (`P21_DESIGN.md` section 2.4, A3): the share of
    /// the fed time of the generated toolpath at `index` that runs at the
    /// commanded feed (`machine::kinematics::reached_feed_share`). A
    /// report: no rule caps the RPM with it. `None` when the toolpath has
    /// no result or the machine states no kinematics. The ceiling and the
    /// rapid feed are the ones that the simulation reads.
    #[must_use]
    pub fn reached_feed_share_at(
        &self,
        index: usize,
    ) -> Option<crate::machine::kinematics::ReachedFeedShare> {
        let tc = self.toolpath_configs.get(index)?;
        let result = self.results.get(&index)?;
        let kinematics = self.machine.kinematics?;
        Some(crate::machine::kinematics::reached_feed_share(
            result.toolpath(),
            &kinematics,
            self.machine
                .op_cutting_ceiling(tc.dressups.feed_max_rate)
                .mm_min(),
            self.post.rapid_feed_mm_min(&self.machine),
        ))
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
            spindle_strategy: self.post.spindle_policy(),
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
