//! The issue list and the caches behind it.
//!
//! The panel asks for one ordered list of what is wrong with the run. These
//! methods build that list, cache it against the inputs that produced it, and
//! move the focus from one issue to the next. They are an `impl
//! SimulationState` fragment, so every name keeps its path.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;

use rs_cam_core::session::ProjectSession;
use rs_cam_core::stock::simulation_cut::SimulationCutIssueKind;
use rs_cam_core::tool_load::ToolLoadReport;

use super::{
    IssueListCacheKey, SimulationIssue, SimulationIssueKind, SimulationState,
    SimulationTraceTarget, weak_matches,
};
use crate::state::runtime::GuiState;

impl SimulationState {
    /// Project tool-load report cached by simulation trace pointer and GUI edit counter.
    ///
    /// The report evaluates chipload/power/deflection for every enabled toolpath and
    /// scans the cut trace for the sample-based criteria. Both the bottom timeline and
    /// right inspector need it every frame, so compute it once per trace/edit version
    /// and return a cheap clone for borrow-friendly UI code.
    pub fn cached_load_report(
        &mut self,
        session: &ProjectSession,
        edit_counter: u64,
    ) -> ToolLoadReport {
        let live = self
            .results
            .as_ref()
            .and_then(|results| results.cut_trace.as_ref());
        if weak_matches(self.debug.load_report_cache.trace.as_ref(), live)
            && self.debug.load_report_cache.edit_counter == edit_counter
            && let Some(report) = &self.debug.load_report_cache.report
        {
            return report.clone();
        }
        let stored_trace = live.map(Arc::downgrade);

        let start = std::time::Instant::now();
        let sim_trace = self.results.as_ref().and_then(|r| r.cut_trace.as_deref());
        let report = rs_cam_core::gcode::project_load_report(session, sim_trace);
        let elapsed = start.elapsed();
        if elapsed > std::time::Duration::from_millis(8) {
            tracing::debug!(
                elapsed_ms = elapsed.as_secs_f64() * 1000.0,
                "slow simulation tool-load report build"
            );
        }
        self.debug.load_report_cache.trace = stored_trace;
        self.debug.load_report_cache.edit_counter = edit_counter;
        self.debug.load_report_cache.report = Some(report.clone());
        report
    }

    pub fn cached_chipload_envelopes(
        &mut self,
        session: &ProjectSession,
        edit_counter: u64,
    ) -> HashMap<rs_cam_core::ToolpathId, Range<f64>> {
        let live = self
            .results
            .as_ref()
            .and_then(|results| results.cut_trace.as_ref());
        if weak_matches(self.debug.chipload_envelope_cache.trace.as_ref(), live)
            && self.debug.chipload_envelope_cache.edit_counter == edit_counter
            && let Some(envelopes) = &self.debug.chipload_envelope_cache.envelopes
        {
            return envelopes.clone();
        }
        let stored_trace = live.map(Arc::downgrade);

        let start = std::time::Instant::now();
        let sim_trace = self.results.as_ref().and_then(|r| r.cut_trace.as_deref());
        let envelopes = rs_cam_core::tool_load::chipload_envelopes_for_session(session, sim_trace);
        let elapsed = start.elapsed();
        if elapsed > std::time::Duration::from_millis(8) {
            tracing::debug!(
                envelope_count = envelopes.len(),
                elapsed_ms = elapsed.as_secs_f64() * 1000.0,
                "slow chipload envelope build"
            );
        }
        self.debug.chipload_envelope_cache.trace = stored_trace;
        self.debug.chipload_envelope_cache.edit_counter = edit_counter;
        self.debug.chipload_envelope_cache.envelopes = Some(envelopes.clone());
        envelopes
    }

    /// Simulation triage cached by simulation trace identity, GUI edit
    /// counter and evidence fingerprint — the trace and edit rule is the same
    /// as [`Self::cached_load_report`] and [`Self::cached_chipload_envelopes`];
    /// the fingerprint is this cache's alone, because it is the only one of
    /// the three whose build reads state outside the trace
    /// ([`Self::project_evidence`], and see [`Self::evidence_fingerprint`]).
    ///
    /// Building it is `O(samples × toolpaths)` with a per-toolpath sort (full
    /// `ProjectDiagnostics` + `MeasurabilityReport` + `SimulationTriage::build`,
    /// plus a `build_cutter` per toolpath), and the inspector's measurability
    /// strip asks for it on every frame the Diagnostics header is open.
    /// Returned by reference: the triage carries several `Vec<Finding>`, so
    /// even a cache-hit clone would be per-frame allocation.
    pub fn cached_simulation_triage(
        &mut self,
        session: &ProjectSession,
        edit_counter: u64,
    ) -> &rs_cam_core::stock::sim_triage::SimulationTriage {
        let evidence_fp = self.evidence_fingerprint();
        let fresh = {
            let live = self
                .results
                .as_ref()
                .and_then(|results| results.cut_trace.as_ref());
            self.debug
                .triage_cache
                .matches(live, edit_counter, evidence_fp)
        };
        if !fresh {
            let stored_trace = self
                .results
                .as_ref()
                .and_then(|results| results.cut_trace.as_ref())
                .map(Arc::downgrade);
            let start = std::time::Instant::now();
            // Scoped so the immutable `project_evidence` borrow of `self`
            // ends before the cache write below.
            let triage = {
                let evidence = self.project_evidence();
                session.simulation_triage(&evidence)
            };
            let elapsed = start.elapsed();
            if elapsed > std::time::Duration::from_millis(8) {
                tracing::debug!(
                    elapsed_ms = elapsed.as_secs_f64() * 1000.0,
                    "slow simulation triage build"
                );
            }
            self.debug.triage_cache.built = true;
            self.debug.triage_cache.trace = stored_trace;
            self.debug.triage_cache.edit_counter = edit_counter;
            self.debug.triage_cache.evidence_fp = evidence_fp;
            self.debug.triage_cache.triage = triage;
        }
        &self.debug.triage_cache.triage
    }

    /// Build the core [`ProjectEvidence`] borrow view from viz state.
    ///
    /// One builder, so the GUI panel, the MCP handlers and anything else on
    /// the viz side hand core the same evidence. It lives here rather than in
    /// `app::mcp` because that module is behind the `mcp` feature and the GUI
    /// needs this with or without it.
    pub fn project_evidence(&self) -> rs_cam_core::session::ProjectEvidence<'_> {
        let boundaries = self
            .results
            .as_ref()
            .map(|r| {
                r.boundaries
                    .iter()
                    .map(|b| (b.id, b.start_move, b.end_move))
                    .collect()
            })
            .unwrap_or_default();
        rs_cam_core::session::ProjectEvidence {
            boundaries,
            rapid_collisions: &self.checks.rapid_collisions,
            rapid_collision_move_indices: &self.checks.rapid_collision_move_indices,
            cut_trace: self.results.as_ref().and_then(|r| r.cut_trace.as_deref()),
            holder_collisions: self.holder_collision_counts_by_tp(),
            // The cell the accepted run simulated at. Read only to enrich a
            // measurability abstention's reason with the number the operator
            // would have to change; it never decides a verdict.
            resolution_mm: self.results.as_ref().map(|r| r.column_grid_cell_mm),
        }
    }

    /// Fingerprint over every [`Self::project_evidence`] input that is **not**
    /// the cut trace, for [`Self::cached_simulation_triage`]'s key.
    ///
    /// The trace is keyed by weak-pinned identity; these are the other four
    /// evidence fields, none of which move the trace pointer and none of which
    /// bump the GUI edit counter:
    ///
    /// - `boundaries` — the move ranges the triage attributes findings through;
    /// - `rapid_collisions` and `rapid_collision_move_indices` — the
    ///   through-stock rapids the triage reports as `safety` findings;
    /// - the holder-collision report's move indices, which is the whole of
    ///   what [`Self::holder_collision_counts_by_tp`] reads. That report is
    ///   written by a *separate async job* (`controller::events::compute`),
    ///   long after the trace lands and with no counter bump: without this
    ///   fingerprint, a holder report arriving while the Diagnostics header is
    ///   open is never reflected in the cached triage;
    /// - `resolution_mm`, which enriches a measurability abstention's reason —
    ///   the one field today's sole consumer (`ui::sim_diagnostics`'s
    ///   `NOT MEASURED` strip) actually renders.
    ///
    /// Mirrors the `collision_fingerprint` the neighbouring `issue_cache_key`
    /// has always folded in. `O(boundaries + collisions)` per frame, the same
    /// order that key already pays.
    fn evidence_fingerprint(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for boundary in self.boundaries() {
            boundary.id.0.hash(&mut hasher);
            boundary.start_move.hash(&mut hasher);
            boundary.end_move.hash(&mut hasher);
        }
        self.checks.rapid_collisions.len().hash(&mut hasher);
        for collision in &self.checks.rapid_collisions {
            collision.move_index.hash(&mut hasher);
            for coord in [
                collision.start.x,
                collision.start.y,
                collision.start.z,
                collision.end.x,
                collision.end.y,
                collision.end.z,
            ] {
                coord.to_bits().hash(&mut hasher);
            }
        }
        self.checks
            .rapid_collision_move_indices
            .len()
            .hash(&mut hasher);
        for &move_index in &self.checks.rapid_collision_move_indices {
            move_index.hash(&mut hasher);
        }
        match self.checks.collision_report.as_ref() {
            Some(report) => {
                report.collisions.len().hash(&mut hasher);
                for collision in &report.collisions {
                    collision.move_index.hash(&mut hasher);
                }
            }
            // Distinguish "no report yet" from "report with no collisions":
            // the async holder job replacing the former with the latter is
            // exactly the transition this fingerprint exists to catch.
            None => u64::MAX.hash(&mut hasher),
        }
        self.results
            .as_ref()
            .map(|r| r.column_grid_cell_mm.to_bits())
            .hash(&mut hasher);
        hasher.finish()
    }

    /// Sorted issue list for the current simulation.
    ///
    /// Returns a shared handle, not a copy: the list runs to tens of
    /// thousands of entries (each with a `String` label) and three or four
    /// panels ask for it every frame, so a cache hit must cost an `Arc`
    /// bump rather than a deep clone. `Arc<[_]>` derefs to `&[_]`, so read
    /// sites (`iter`, `get`, indexing, `&issues` into a `&[_]` parameter)
    /// are unchanged.
    pub fn issues(&mut self, gui: &GuiState, max_feed_mm_min: f64) -> Arc<[SimulationIssue]> {
        self.ensure_issue_cache(gui, max_feed_mm_min);
        Arc::clone(&self.debug.issue_cache.issues)
    }

    /// Number of `Hotspot` issues, folded in when the issue cache is built.
    /// The timeline's pill needs only this count and used to clone the whole
    /// list to get it.
    pub fn issue_hotspot_count(&mut self, gui: &GuiState, max_feed_mm_min: f64) -> usize {
        self.ensure_issue_cache(gui, max_feed_mm_min);
        self.debug.issue_cache.hotspot_count
    }

    fn ensure_issue_cache(&mut self, gui: &GuiState, max_feed_mm_min: f64) {
        self.sync_debug_state(gui);
        let cache_key = self.issue_cache_key(gui, max_feed_mm_min);
        if self.debug.issue_cache.key == Some(cache_key) {
            return;
        }

        let start = std::time::Instant::now();
        let mut issues = Vec::new();

        for boundary in self.boundaries().to_vec() {
            let Some(rt) = gui.toolpath_rt.get(&boundary.id) else {
                continue;
            };
            if let Some(trace) = rt.debug_trace.as_ref() {
                for (annotation_index, annotation) in trace.annotations.iter().enumerate() {
                    issues.push(SimulationIssue {
                        kind: SimulationIssueKind::Annotation,
                        toolpath_id: Some(boundary.id),
                        move_index: boundary.start_move + annotation.move_index,
                        label: annotation.label.clone(),
                        semantic_item_id: rt.semantic_trace.as_ref().and_then(|semantic_trace| {
                            semantic_trace
                                .items
                                .iter()
                                .find(|item| {
                                    item.move_start
                                        .is_some_and(|start| start <= annotation.move_index)
                                        && item
                                            .move_end
                                            .is_some_and(|end| annotation.move_index <= end)
                                })
                                .map(|item| item.id)
                        }),
                        debug_span_id: None,
                        hotspot_index: None,
                        annotation_index: Some(annotation_index),
                    });
                }

                for (hotspot_index, hotspot) in trace.hotspots.iter().enumerate() {
                    let Some(target) =
                        self.trace_target_for_hotspot(gui, boundary.id, hotspot_index)
                    else {
                        continue;
                    };
                    issues.push(SimulationIssue {
                        kind: SimulationIssueKind::Hotspot,
                        toolpath_id: Some(boundary.id),
                        move_index: target.move_index,
                        label: format!("{} hotspot {}", hotspot.kind, hotspot_index + 1),
                        semantic_item_id: target.semantic_item_id,
                        debug_span_id: hotspot.representative_span_id.or(target.debug_span_id),
                        hotspot_index: Some(hotspot_index),
                        annotation_index: None,
                    });
                }
            }
        }

        if let Some(trace) = self
            .results
            .as_ref()
            .and_then(|results| results.cut_trace.as_ref())
        {
            for issue in &trace.issues {
                let toolpath_id = issue.toolpath_id;
                let Some(global_move) = self.global_move_for_local(toolpath_id, issue.move_index)
                else {
                    continue;
                };
                issues.push(SimulationIssue {
                    kind: match issue.kind {
                        SimulationCutIssueKind::AirCut => SimulationIssueKind::AirCut,
                        SimulationCutIssueKind::LowEngagement => SimulationIssueKind::LowEngagement,
                    },
                    toolpath_id: Some(toolpath_id),
                    move_index: global_move,
                    label: issue.label.clone(),
                    semantic_item_id: issue.semantic_item_id,
                    debug_span_id: None,
                    hotspot_index: None,
                    annotation_index: None,
                });
            }
        }

        for &move_index in &self.checks.rapid_collision_move_indices {
            let toolpath_id = self
                .move_to_local_toolpath_move(move_index)
                .map(|(_, id, _)| id);
            issues.push(SimulationIssue {
                kind: SimulationIssueKind::RapidCollision,
                toolpath_id,
                move_index,
                label: "Rapid collision".to_owned(),
                semantic_item_id: None,
                debug_span_id: None,
                hotspot_index: None,
                annotation_index: None,
            });
        }

        if let Some(report) = self.checks.collision_report.as_ref() {
            for collision in &report.collisions {
                let toolpath_id = self
                    .move_to_local_toolpath_move(collision.move_index)
                    .map(|(_, id, _)| id);
                issues.push(SimulationIssue {
                    kind: SimulationIssueKind::HolderCollision,
                    toolpath_id,
                    move_index: collision.move_index,
                    label: format!("{} collision", collision.segment),
                    semantic_item_id: None,
                    debug_span_id: None,
                    hotspot_index: None,
                    annotation_index: None,
                });
            }
        }

        // D1 (census §3.5), ruled at Checkpoint D D-6. Severity was a
        // TIEBREAK under `move_index`, so an operator stepping the list with
        // `focus_issue_delta` reached collisions in path order — i.e. at
        // random relative to how much they matter — and a second,
        // contradictory rank in `sim_op_list.rs` put collisions first. One
        // rank now, severity-major, with `move_index` as the LAST key:
        // "what should I look at" is answered before "where is it".
        issues.sort_by(|left, right| {
            issue_kind_rank(left.kind)
                .cmp(&issue_kind_rank(right.kind))
                .then_with(|| left.move_index.cmp(&right.move_index))
                .then_with(|| left.label.cmp(&right.label))
        });
        let elapsed = start.elapsed();
        if elapsed > std::time::Duration::from_millis(8) {
            tracing::debug!(
                issue_count = issues.len(),
                elapsed_ms = elapsed.as_secs_f64() * 1000.0,
                "slow simulation issue list build"
            );
        }
        let hotspot_count = issues
            .iter()
            .filter(|issue| issue.kind == SimulationIssueKind::Hotspot)
            .count();
        self.debug.issue_cache.key = Some(cache_key);
        self.debug.issue_cache.issues = Arc::from(issues);
        self.debug.issue_cache.hotspot_count = hotspot_count;
    }

    fn issue_cache_key(&self, gui: &GuiState, max_feed_mm_min: f64) -> IssueListCacheKey {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for boundary in self.boundaries() {
            boundary.id.0.hash(&mut hasher);
            if let Some(rt) = gui.toolpath_rt.get(&boundary.id) {
                if let Some(trace) = rt.debug_trace.as_ref() {
                    (Arc::as_ptr(trace) as usize).hash(&mut hasher);
                    trace.annotations.len().hash(&mut hasher);
                    trace.hotspots.len().hash(&mut hasher);
                }
                if let Some(trace) = rt.semantic_trace.as_ref() {
                    (Arc::as_ptr(trace) as usize).hash(&mut hasher);
                    trace.items.len().hash(&mut hasher);
                }
            }
        }
        let debug_trace_fingerprint = hasher.finish();

        let mut collision_hasher = std::collections::hash_map::DefaultHasher::new();
        for &move_index in &self.checks.rapid_collision_move_indices {
            move_index.hash(&mut collision_hasher);
        }
        if let Some(report) = self.checks.collision_report.as_ref() {
            for collision in &report.collisions {
                collision.move_index.hash(&mut collision_hasher);
            }
        }
        let collision_fingerprint = collision_hasher.finish();

        IssueListCacheKey {
            cut_trace_ptr: self
                .results
                .as_ref()
                .and_then(|results| results.cut_trace.as_ref())
                .map(|trace| Arc::as_ptr(trace) as usize),
            gui_edit_counter: gui.edit_counter,
            debug_trace_fingerprint,
            max_feed_bits: max_feed_mm_min.to_bits(),
            collision_fingerprint,
        }
    }

    pub fn current_issue(
        &mut self,
        gui: &GuiState,
        max_feed_mm_min: f64,
    ) -> Option<SimulationIssue> {
        let issues = self.issues(gui, max_feed_mm_min);
        let index = self.debug.focused_issue_index?;
        issues.get(index).cloned()
    }

    pub fn focus_issue_delta(
        &mut self,
        gui: &GuiState,
        max_feed_mm_min: f64,
        delta: isize,
    ) -> Option<SimulationTraceTarget> {
        let issues = self.issues(gui, max_feed_mm_min);
        if issues.is_empty() {
            self.debug.focused_issue_index = None;
            self.debug.focused_hotspot = None;
            return None;
        }

        let len = issues.len() as isize;
        let current = self.debug.focused_issue_index.map(|index| index as isize);
        let next = match current {
            Some(index) => (index + delta).rem_euclid(len),
            None if delta < 0 => len - 1,
            None => 0,
        } as usize;
        self.debug.focused_issue_index = Some(next);
        let issue = issues.get(next)?.clone();
        self.debug.focused_hotspot = issue.toolpath_id.zip(issue.hotspot_index);

        if let Some(toolpath_id) = issue.toolpath_id {
            if let Some(hotspot_index) = issue.hotspot_index {
                if let Some(item_id) = issue.semantic_item_id {
                    self.pin_semantic_item(toolpath_id, item_id);
                }
                return self.trace_target_for_hotspot(gui, toolpath_id, hotspot_index);
            }
            if let Some(annotation_index) = issue.annotation_index
                && let Some(rt) = gui.toolpath_rt.get(&toolpath_id)
                && let Some(trace) = rt.debug_trace.as_ref()
                && let Some(annotation) = trace.annotations.get(annotation_index)
            {
                if let Some(item_id) = issue.semantic_item_id {
                    self.pin_semantic_item(toolpath_id, item_id);
                }
                return self.trace_target_for_annotation(toolpath_id, annotation);
            }
            if matches!(
                issue.kind,
                SimulationIssueKind::AirCut | SimulationIssueKind::LowEngagement
            ) {
                if let Some(item_id) = issue.semantic_item_id {
                    self.pin_semantic_item(toolpath_id, item_id);
                }
                return Some(SimulationTraceTarget {
                    toolpath_id,
                    move_index: issue.move_index,
                    semantic_item_id: issue.semantic_item_id,
                    debug_span_id: issue.debug_span_id,
                });
            }
            if let Some(item_id) = issue.semantic_item_id {
                self.pin_semantic_item(toolpath_id, item_id);
                return self.trace_target_for_item(gui, toolpath_id, item_id, false);
            }
        }

        let toolpath_id = issue.toolpath_id.or_else(|| {
            self.move_to_local_toolpath_move(issue.move_index)
                .map(|(_, id, _)| id)
        })?;

        Some(SimulationTraceTarget {
            toolpath_id,
            move_index: issue.move_index,
            semantic_item_id: issue.semantic_item_id,
            debug_span_id: issue.debug_span_id,
        })
    }
}

/// Display rank, **worst first**. Lower sorts earlier.
///
/// D1 (census §3.5), ruled D-6. This used to run the other way — collisions
/// last, behind hotspots, annotations and per-sample air-cut noise — while
/// `sim_op_list.rs` ranked collisions first. Two contradictory orderings
/// over one list is how a rapid-through-stock ends up below an air-cut run
/// in the panel the operator scans before pressing go.
///
/// The order mirrors the census §3.1 classes: safety, then action-required,
/// then bounded advisories, then the diagnostic-sample tallies that are
/// emission noise by construction.
fn issue_kind_rank(kind: SimulationIssueKind) -> u8 {
    match kind {
        // Class A — physical damage if run.
        SimulationIssueKind::RapidCollision => 0,
        SimulationIssueKind::HolderCollision => 1,
        // Class C — bounded advisories.
        SimulationIssueKind::Hotspot => 2,
        SimulationIssueKind::Annotation => 3,
        // Class D/E — per-run tallies, not defect counts.
        SimulationIssueKind::LowEngagement => 4,
        SimulationIssueKind::AirCut => 5,
    }
}
