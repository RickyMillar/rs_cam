//! The freshness rule of a simulation trace, what a fresh trace measured
//! (flute reach, envelope placement, the lineage) and the tool-load report
//! builder. Split out of `session/compute/export.rs` (feeds contract P14,
//! review L6); a pure move.

use crate::session::ProjectSession;
use crate::stock::simulation_cut::SimulationCutTrace;

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

    /// K5 ReachFlute (feeds contract P7 commit B): what the stored
    /// simulation measured for the toolpath at `index`, when `trace` is
    /// fresh for it ([`Self::sim_trace_is_fresh_for`]: the run carved this
    /// result and the config has not moved). `None`: no fresh simulation
    /// measured it, so the pre-simulation row ("by the stock box") stands.
    /// The diagnostics, the GUI inspector and the export read this one
    /// answer.
    #[must_use]
    pub fn flute_reach_measured(
        &self,
        index: usize,
        trace: Option<&SimulationCutTrace>,
    ) -> Option<crate::compute::simulate::FluteReach> {
        let trace = trace?;
        if !self.sim_trace_is_fresh_for(trace, index) {
            return None;
        }
        let simulation = self.simulation.as_ref()?;
        let id = self.toolpath_config_at(index)?.id;
        Some(crate::compute::simulate::flute_reach(
            simulation,
            id,
            self.contact_frame(index),
        ))
    }

    /// P22-0c-4 (operator ruling Q1): the holder outcomes with the stock
    /// answer in place of the mesh answer. For each toolpath that the
    /// stored simulation carved by the walk and is fresh for
    /// ([`Self::sim_trace_is_fresh_for`]), the outcome is
    /// `MeasuredOnStock(n)`, `n` = its fed shank, holder and shoe contacts
    /// (`SimulationResult::body_contacts`). Every other toolpath keeps its
    /// `mesh` outcome: no simulation, a stale one, or a drill (its removal
    /// is analytic and never enters the walk). The mesh check stays the
    /// answer before a simulation and the source of `min_safe_stickout`.
    #[must_use]
    pub fn holder_checks_with_stock(
        &self,
        mesh: Vec<(
            crate::ids::ToolpathId,
            crate::stock::collision::HolderCollisionCheck,
        )>,
    ) -> Vec<(
        crate::ids::ToolpathId,
        crate::stock::collision::HolderCollisionCheck,
    )> {
        let Some(simulation) = self.simulation.as_ref() else {
            return mesh;
        };
        let Some(trace) = simulation.cut_trace.as_deref() else {
            return mesh;
        };
        let mut out = mesh;
        for index in 0..self.toolpath_count() {
            let (Some(tc), Some(result)) = (self.toolpath_config_at(index), self.result_at(index))
            else {
                continue;
            };
            if result.drill_op().is_some() || !self.sim_trace_is_fresh_for(trace, index) {
                continue;
            }
            let stock = crate::stock::collision::HolderCollisionCheck::MeasuredOnStock(
                crate::compute::simulate::fed_body_contact_count(&simulation.body_contacts, tc.id),
            );
            match out.iter_mut().find(|(id, _)| *id == tc.id) {
                Some(entry) => entry.1 = stock,
                None => out.push((tc.id, stock)),
            }
        }
        out
    }

    /// P22-0c-3 (one frame): the frame the contact findings of the
    /// toolpath at `index` state their positions in, the G-code frame of
    /// its setup (`gcode::export_datum_shift_for_toolpath`). Every surface
    /// (diagnostics, triage, export, MCP) reads this one frame.
    #[must_use]
    pub fn contact_frame(&self, index: usize) -> crate::compute::simulate::ContactFrame {
        crate::compute::simulate::ContactFrame {
            program_shift: crate::gcode::export_datum_shift_for_toolpath(self, index),
        }
    }

    /// The [`Self::contact_frame`] of the toolpath with id `id`;
    /// [`crate::compute::simulate::ContactFrame::EMISSION`] for an id the
    /// project does not hold.
    #[must_use]
    pub fn contact_frame_of(
        &self,
        id: crate::ids::ToolpathId,
    ) -> crate::compute::simulate::ContactFrame {
        self.toolpath_configs
            .iter()
            .position(|tc| tc.id == id)
            .map_or(crate::compute::simulate::ContactFrame::EMISSION, |index| {
                self.contact_frame(index)
            })
    }

    /// G-CENTRECUT (P22-0c-3, one answer per move): what the stored
    /// simulation measured on the core of the toolpath at `index`, when
    /// `trace` is fresh for it and its tool has a core zone (a
    /// `CentreCutting::No` fact). `None`: no fresh run measured it, or the
    /// tool cuts at its centre; then the static
    /// `safety.non_centre_cutting_plunge` stands. `Some` supersedes it on
    /// every surface: the diagnostics, the GUI inspector and the export.
    #[must_use]
    pub fn core_contact_measured(
        &self,
        index: usize,
        trace: Option<&SimulationCutTrace>,
    ) -> Option<crate::compute::simulate::CoreContact> {
        let trace = trace?;
        if !self.sim_trace_is_fresh_for(trace, index) {
            return None;
        }
        let tc = self.toolpath_config_at(index)?;
        self.toolpath_tool(tc)?.non_centre_cutting_geometry()?;
        let simulation = self.simulation.as_ref()?;
        Some(crate::compute::simulate::core_contact(
            simulation,
            tc.id,
            self.contact_frame(index),
        ))
    }

    /// K7 (feeds contract P9): the measured engagement of the toolpath at
    /// `index` placed against its K2b envelope
    /// ([`crate::compute::simulate::envelope_placement`], the tenth shared
    /// decision), when `trace` is fresh for it. `None`: no fresh
    /// simulation, no envelope (a refused pairing) or no load context.
    /// Ruling Q5: Suggest never reads it; the diagnostics, the GUI, MCP and
    /// the export read this one answer.
    #[must_use]
    pub fn envelope_placement_measured(
        &self,
        index: usize,
        trace: Option<&SimulationCutTrace>,
    ) -> Option<crate::compute::simulate::EnvelopePlacement> {
        let trace = trace?;
        if !self.sim_trace_is_fresh_for(trace, index) {
            return None;
        }
        let tc = self.toolpath_config_at(index)?;
        let envelope = self.safe_envelope(tc)?.ok()?;
        let parts = self.load_context_parts(index)?;
        Some(crate::compute::simulate::envelope_placement(
            trace,
            &parts.ctx(),
            &envelope,
        ))
    }

    /// K8 + K7 (feeds contract P9): the lineage of the toolpath at `index`
    /// with what a fresh simulation measured on its envelope
    /// ([`Self::envelope_placement_measured`]). With no result (a loaded
    /// project, or an edit that dropped the result) it is the stored
    /// lineage ([`Self::stored_lineage`], P13b); the run state says which.
    /// `None` when no toolpath is at `index` or it has no tool; `measured`
    /// is `None` with no fresh trace. MCP `get_toolpath_lineage`, the CLI
    /// and the GUI read it.
    #[must_use]
    pub fn lineage_measured(
        &self,
        index: usize,
        trace: Option<&SimulationCutTrace>,
    ) -> Option<crate::trace::lineage::Lineage> {
        let Some(result) = self.result_at(index) else {
            return self.stored_lineage(index);
        };
        let measured = self
            .envelope_placement_measured(index, trace)
            .map(|p| crate::trace::lineage::MeasuredLineage::of(&p));
        Some(result.lineage.with_measured(measured))
    }

    /// P13b (lead ruling, "a loaded project shows the stored lineage until
    /// a regenerate replaces it"): the lineage of the toolpath at `index`
    /// from the stored values only: the cut, the pick, the stamps, the tool
    /// facts, the entry contract and the step records of the project file.
    /// The sections that a generation makes (the entries, the findings of
    /// the generation, the rule rows, the measured placement) are empty.
    /// `None` when no toolpath is at `index` or it has no tool.
    #[must_use]
    pub fn stored_lineage(&self, index: usize) -> Option<crate::trace::lineage::Lineage> {
        let tc = self.toolpath_config_at(index)?;
        let tool = self.toolpath_tool(tc)?;
        Some(crate::trace::lineage::Lineage::stored(
            &tc.operation,
            tool,
            &tc.feeds_provenance,
        ))
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
