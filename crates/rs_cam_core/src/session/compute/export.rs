//! G-code export and the per-toolpath report contexts.
//!
//! Holds the export entries and the context builders the diagnostics adapters
//! read — model reference, preconditions, heights and feeds. Split out of
//! `session/compute.rs` (P4).

use std::path::Path;

use tracing::instrument;

use crate::compute::tool_config::{ToolConfig, ToolId};
use crate::session::{ProjectEvidence, ProjectSession, SessionError};

/// Transform an axis-aligned bbox from world frame into a setup-local
/// frame defined by `info`. Result remains axis-aligned because all
/// setup transforms are 90° increments + translation.
fn transform_bbox_world_to_local(
    bbox: &crate::geo::BoundingBox3,
    info: &crate::compute::transform::SetupTransformInfo,
) -> crate::geo::BoundingBox3 {
    use crate::geo::P3;
    let corners = [
        P3::new(bbox.min.x, bbox.min.y, bbox.min.z),
        P3::new(bbox.max.x, bbox.min.y, bbox.min.z),
        P3::new(bbox.min.x, bbox.max.y, bbox.min.z),
        P3::new(bbox.max.x, bbox.max.y, bbox.min.z),
        P3::new(bbox.min.x, bbox.min.y, bbox.max.z),
        P3::new(bbox.max.x, bbox.min.y, bbox.max.z),
        P3::new(bbox.min.x, bbox.max.y, bbox.max.z),
        P3::new(bbox.max.x, bbox.max.y, bbox.max.z),
    ];
    let mut min = P3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut max = P3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for c in corners {
        let p = info.world_to_local(c);
        min.x = min.x.min(p.x);
        min.y = min.y.min(p.y);
        min.z = min.z.min(p.z);
        max.x = max.x.max(p.x);
        max.y = max.y.max(p.y);
        max.z = max.z.max(p.z);
    }
    crate::geo::BoundingBox3 { min, max }
}

impl ProjectSession {
    /// Export G-code for all computed toolpaths under the default tool-load
    /// policy (refuse on Exceeds or Unmodeled). For an override-capable
    /// variant see [`export_gcode_with_policy`].
    #[instrument(skip(self))]
    pub fn export_gcode(&self, path: &Path) -> Result<(), SessionError> {
        self.export_gcode_with_policy(path, crate::gcode::ToolLoadExportPolicy::default())
    }

    /// Export G-code with an explicit tool-load policy. Used by callers that
    /// want to override `Exceeds` or `Unmodeled` verdicts (UI checkbox,
    /// `--accept-…` CLI flag, MCP parameter).
    #[instrument(skip(self))]
    pub fn export_gcode_with_policy(
        &self,
        path: &Path,
        policy: crate::gcode::ToolLoadExportPolicy,
    ) -> Result<(), SessionError> {
        let gcode = crate::gcode::export_gcode_checked(
            self,
            self.simulation
                .as_ref()
                .and_then(|simulation| simulation.cut_trace.as_deref()),
            policy,
        )
        .map_err(|e| SessionError::Export(e.to_string()))?;
        std::fs::write(path, gcode).map_err(|e| {
            SessionError::Export(format!("Failed to write G-code to {}: {e}", path.display()))
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
    /// PR-5 polish: computes heights via [`Self::height_context_for_toolpath`]
    /// and the feeds-calculator result via [`Self::feeds_result_for_toolpath`],
    /// so MCP consumers see the same heights / pre-sim hint diagnostics the
    /// GUI panel renders.
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

        let height_ctx = self.height_context_for_toolpath(tc);
        // N4 (2026-09-10): resolve the toolpath's OWN heights. This used to
        // call `ResolvedHeights::from_context`, which projects the stock top
        // and the safe Z into the five slots and drops every pin. Three of the
        // four plane-order checks could not fire on this route at all, the
        // fourth only on a bad safe Z, and a pinned Top Z never reached
        // `geom.depth_beyond_stock`. The GUI ribbon resolved the same config
        // all along; the two surfaces now share one derivation.
        let heights =
            crate::diagnostics::adapters::from_static_checks::ResolvedHeights::from_heights(
                &tc.heights,
                &height_ctx,
            );
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
            heights: Some(&heights),
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

    /// Build a [`ModelRefContext`] for a single toolpath (F-023).
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

    /// Build a [`PreconditionContext`] for a single toolpath. Captures
    /// the prior toolpaths in the same setup (so the rest-machining
    /// precondition can verify a larger upstream tool exists), the
    /// target model's geometry kind (so the drill / project-curve
    /// preconditions can verify a curve / mesh source is in scope),
    /// and the per-tool diameters used by the rest "prev tool must be
    /// larger" check.
    ///
    /// Pure helper — no I/O, no mutation. Safe to call repeatedly per
    /// diagnose round. GUI consumers snapshot this context before borrowing
    /// their editable toolpath entry.
    pub fn precondition_context_for_toolpath(
        &self,
        tc: &super::ToolpathConfig,
    ) -> crate::diagnostics::diagnose::PreconditionContext {
        use crate::diagnostics::diagnose::{
            PreconditionContext, PriorToolpathSummary, TargetModelGeometry, ToolDiameterEntry,
        };

        // Find the setup that owns this toolpath and collect prior
        // toolpaths in that setup (lower display index than `tc`).
        let mut prior_toolpaths_in_setup = Vec::new();
        if let Some(setup) = self.find_setup_for_toolpath_id(tc.id) {
            for &idx in &setup.toolpath_indices {
                if let Some(other) = self.toolpath_configs.get(idx) {
                    if other.id == tc.id {
                        break;
                    }
                    prior_toolpaths_in_setup.push(PriorToolpathSummary {
                        enabled: other.enabled,
                        tool_id: other.tool_id,
                        model_id: other.model_id,
                    });
                }
            }
        }

        let target_model =
            self.models
                .iter()
                .find(|m| m.id == tc.model_id)
                .map(|m| TargetModelGeometry {
                    has_polygons: m.polygons.as_ref().is_some_and(|p| !p.is_empty()),
                    has_mesh: m.mesh.is_some(),
                    drill_target_count: m.drill_targets.len(),
                });

        let any_loaded_model_has_mesh = self.models.iter().any(|m| m.mesh.is_some());

        let tool_diameters = self
            .tools
            .iter()
            .map(|t| ToolDiameterEntry {
                id: t.id,
                diameter: t.diameter,
            })
            .collect();

        PreconditionContext {
            prior_toolpaths_in_setup,
            target_model,
            any_loaded_model_has_mesh,
            tool_diameters,
        }
    }

    /// Build a [`HeightContext`] for a given toolpath. Mirrors the GUI's
    /// `height_context_from_session` so MCP consumers see the same heights
    /// diagnostics the params panel renders.
    ///
    /// F-030: stock-frame + transform decisions delegate to
    /// [`super::SetupEvalContext`] so this path and the generation path
    /// (`generate_toolpath`) share a single source of truth for
    /// `stock_top_z` / `safe_z` / model-bbox-in-frame.
    pub fn height_context_for_toolpath(
        &self,
        tc: &super::ToolpathConfig,
    ) -> crate::compute::config::HeightContext {
        let setup = self.find_setup_for_toolpath_id(tc.id);
        let ctx = super::SetupEvalContext::build_for_setup(self, setup);
        let raw_mb = self
            .models
            .iter()
            .find(|m| m.id == tc.model_id)
            .and_then(|m| {
                m.mesh.as_ref().map(|mesh| mesh.bbox).or_else(|| {
                    m.polygons
                        .as_deref()
                        .and_then(|v| super::mutation::polygons_bbox(v))
                })
            });
        // Apply the setup transform that owns this toolpath so
        // model_top/bottom_z are in the setup-local frame for non-identity
        // setups. Identity setups leave the world bbox untouched.
        let mb = match (raw_mb, ctx.local_to_global.as_ref()) {
            (Some(b), Some(info)) => Some(transform_bbox_world_to_local(&b, info)),
            (Some(b), None) => Some(b),
            _ => None,
        };
        let heights_bbox = ctx.heights_stock_bbox;
        crate::compute::config::HeightContext {
            safe_z: ctx.safe_z,
            op_depth: tc.operation.default_depth_for_heights(),
            stock_top_z: heights_bbox.max.z,
            stock_bottom_z: heights_bbox.min.z,
            model_top_z: mb.map(|b| b.max.z),
            model_bottom_z: mb.map(|b| b.min.z),
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
    /// An operation that the optimizer or the operator rewrote in every
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
    /// [`ProjectDiagnostics`] snapshot using the session's cached
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
