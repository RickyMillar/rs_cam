//! The issue list and the caches behind it.
//!
//! The panel asks for one ordered list of what is wrong with the run. These
//! methods build that list, cache it against the inputs that produced it, and
//! move the focus from one issue to the next. They are an `impl
//! SimulationState` fragment, so every name keeps its path.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use rs_cam_core::session::{LoadReportStamp, ProjectSession, TriageSessionKey};
use rs_cam_core::stock::simulation_cut::SimulationCutIssueKind;
use rs_cam_core::tool_load::ToolLoadReport;

use super::{
    IssueListCacheKey, IssueTraceIdentity, SimulationIssue, SimulationIssueKind, SimulationState,
    SimulationTraceTarget,
};
use crate::state::runtime::GuiState;

impl SimulationState {
    /// The project tool-load report against this view's cut trace.
    ///
    /// The memo is the session's ([`ProjectSession::tool_load_report_for`]),
    /// keyed by every input the report reads and by the trace identity.
    /// This view adds no key of its own. The GUI edit counter is not in the
    /// key: a result adoption moves no counter, and a counter key therefore
    /// kept a report for a toolpath that had generated again.
    ///
    /// The answer is an `Arc`. A clone of the report copies every
    /// kinematic move row, which on a large finish is more work than a
    /// frame has.
    pub fn cached_load_report(&self, session: &ProjectSession) -> Arc<ToolLoadReport> {
        session.tool_load_report_for(self.cut_trace_arc())
    }

    /// The accepted run's cut trace, as the `Arc` that holds it. The load
    /// report memo keys on this `Arc`'s identity.
    pub fn cut_trace_arc(
        &self,
    ) -> Option<&Arc<rs_cam_core::stock::simulation_cut::SimulationCutTrace>> {
        self.results
            .as_ref()
            .and_then(|results| results.cut_trace.as_ref())
    }

    /// Simulation triage, cached on the inputs it reads (see
    /// [`super::SimulationTriageCache`]): the load report, the session
    /// parts beside the report key, and the evidence fingerprint.
    ///
    /// A build reads its kinematic rows from the load report
    /// ([`ProjectSession::verdict_and_triage_with_report`]), so it does not
    /// analyse the moves again. It still walks the cut trace for the
    /// diagnostics and the triage. The inspector's measurability strip asks
    /// for it on every frame the Diagnostics header is open, so a hit must
    /// cost only the key compare. Returned by reference: the triage carries
    /// several `Vec<Finding>`, so even a cache-hit clone would be per-frame
    /// allocation.
    pub fn cached_simulation_triage(
        &mut self,
        session: &ProjectSession,
    ) -> &rs_cam_core::stock::sim_triage::SimulationTriage {
        self.refresh_triage_cache(session);
        &self.debug.triage_cache.triage
    }

    /// The project verdict (F113) of the same diagnostics build as
    /// [`Self::cached_simulation_triage`], on the same cache key. It is
    /// core's [`rs_cam_core::session::ProjectDiagnostics::verdict`], the
    /// answer the CLI summary and the MCP `get_diagnostics` reply give.
    pub fn cached_project_verdict(
        &mut self,
        session: &ProjectSession,
    ) -> Option<rs_cam_core::session::ProjectVerdict> {
        self.refresh_triage_cache(session);
        self.debug.triage_cache.verdict
    }

    /// Build the triage and the project verdict when the cache key moved.
    ///
    /// F187: the evidence is built once. Its fingerprint and the build read
    /// that same value, so the key cannot miss an evidence part the build
    /// reads.
    fn refresh_triage_cache(&mut self, session: &ProjectSession) {
        let report = self.cached_load_report(session);
        let start = std::time::Instant::now();
        // Scoped so the immutable `project_evidence` borrow of `self`
        // ends before the cache write below.
        let (evidence_fp, built) = {
            let evidence = self.project_evidence();
            let evidence_fp = Self::evidence_fingerprint(&evidence);
            let built = (!self
                .debug
                .triage_cache
                .matches(&report, session, evidence_fp))
            .then(|| session.verdict_and_triage_with_report(&evidence, &report));
            (evidence_fp, built)
        };
        let Some((verdict, triage)) = built else {
            return;
        };
        let elapsed = start.elapsed();
        if elapsed > std::time::Duration::from_millis(8) {
            tracing::debug!(
                elapsed_ms = elapsed.as_secs_f64() * 1000.0,
                "slow simulation triage build"
            );
        }
        let cache = &mut self.debug.triage_cache;
        cache.report = Some(LoadReportStamp::of(&report));
        cache.session = Some(TriageSessionKey::of(session));
        cache.evidence_fp = evidence_fp;
        cache.triage = triage;
        cache.verdict = Some(verdict);
        cache.builds += 1;
    }

    /// Build the core [`ProjectEvidence`](rs_cam_core::session::ProjectEvidence) borrow view from viz state.
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
            // K1: the obstacle hits of the accepted run.
            obstacle_collisions: &self.checks.obstacle_collisions,
            cut_trace: self.results.as_ref().and_then(|r| r.cut_trace.as_deref()),
            holder_collisions: self.holder_collision_counts_by_tp(),
            // The cell the accepted run simulated at. Read only to enrich a
            // measurability abstention's reason with the number the operator
            // would have to change; it never decides a verdict.
            resolution_mm: self.results.as_ref().map(|r| r.column_grid_cell_mm),
        }
    }

    /// Fingerprint over every [`Self::project_evidence`] part that is **not**
    /// the cut trace, for [`Self::cached_simulation_triage`]'s key.
    ///
    /// F187: it reads the evidence value the triage build reads, and it
    /// destructures that value with no `..`. A new evidence part is
    /// therefore a compile error here until it is keyed. The load report
    /// stamp covers the trace identity; the other parts move no trace
    /// pointer:
    ///
    /// - `boundaries` — the move ranges the triage attributes findings through;
    /// - `rapid_collisions` and `rapid_collision_move_indices` — the
    ///   through-stock rapids the triage reports as `safety` findings;
    /// - `obstacle_collisions` — the K1 obstacle hits the triage lists;
    /// - `holder_collisions` — the holder counts. Their report is written by
    ///   a *separate async job* (`controller::events::compute`), long after
    ///   the trace lands and with no counter bump: without this fingerprint,
    ///   a holder report arriving while the Diagnostics header is open is
    ///   never reflected in the cached triage;
    /// - `resolution_mm`, which enriches a measurability abstention's reason —
    ///   the one field today's sole consumer (`ui::sim_diagnostics`'s
    ///   `NOT MEASURED` strip) actually renders.
    ///
    /// `O(boundaries + collisions)` per frame, the same order that the
    /// neighbouring `issue_cache_key` pays.
    fn evidence_fingerprint(evidence: &rs_cam_core::session::ProjectEvidence<'_>) -> u64 {
        let rs_cam_core::session::ProjectEvidence {
            boundaries,
            rapid_collisions,
            rapid_collision_move_indices,
            obstacle_collisions,
            // The load report stamp holds the trace identity.
            cut_trace: _,
            holder_collisions,
            resolution_mm,
        } = evidence;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        boundaries.hash(&mut hasher);
        rapid_collisions.len().hash(&mut hasher);
        for collision in *rapid_collisions {
            let rs_cam_core::stock::collision::RapidCollision {
                move_index,
                start,
                end,
            } = collision;
            move_index.hash(&mut hasher);
            for coord in [start.x, start.y, start.z, end.x, end.y, end.z] {
                coord.to_bits().hash(&mut hasher);
            }
        }
        rapid_collision_move_indices.hash(&mut hasher);
        obstacle_collisions.len().hash(&mut hasher);
        for hit in *obstacle_collisions {
            let rs_cam_core::compute::obstacle::ObstacleCollision {
                toolpath_id,
                global_move,
                event,
            } = hit;
            let rs_cam_core::stock::collision::CollisionEvent {
                move_index,
                position,
                penetration_depth,
                segment,
                kind,
            } = event;
            toolpath_id.hash(&mut hasher);
            global_move.hash(&mut hasher);
            move_index.hash(&mut hasher);
            for value in [position.x, position.y, position.z, *penetration_depth] {
                value.to_bits().hash(&mut hasher);
            }
            // No `Hash` on these two: their `Debug` text, the idiom of the
            // core digests.
            format!("{segment:?}|{kind:?}").hash(&mut hasher);
        }
        holder_collisions.len().hash(&mut hasher);
        for (toolpath_id, check) in holder_collisions {
            toolpath_id.hash(&mut hasher);
            format!("{check:?}").hash(&mut hasher);
        }
        resolution_mm.map(f64::to_bits).hash(&mut hasher);
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
        if self.debug.issue_cache.key.as_ref() == Some(&cache_key) {
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
            let toolpath_id = self.locate_move(move_index).map(|loc| loc.toolpath_id);
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

        // G-RAPIDFRAME: a holder event carries the checked toolpath's OWN
        // move index. The located list gives the run move; an event the run
        // does not hold (a disabled toolpath) has no place on this
        // move-ordered list. The readiness row and MCP `inspect_collisions`
        // still count it.
        for hit in self.located_holder_collisions() {
            if let Some(move_index) = hit.global_move {
                issues.push(SimulationIssue {
                    kind: SimulationIssueKind::HolderCollision,
                    toolpath_id: hit.toolpath_id,
                    move_index,
                    label: format!("{} collision", hit.event.segment),
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
        let mut boundary_hasher = std::collections::hash_map::DefaultHasher::new();
        let mut toolpath_traces = Vec::with_capacity(self.boundaries().len());
        for boundary in self.boundaries() {
            boundary.id.raw().hash(&mut boundary_hasher);
            boundary.start_move.hash(&mut boundary_hasher);
            boundary.end_move.hash(&mut boundary_hasher);
            let rt = gui.toolpath_rt.get(&boundary.id);
            toolpath_traces.push(IssueTraceIdentity {
                debug: rt
                    .and_then(|rt| rt.debug_trace.as_ref())
                    .map(Arc::downgrade),
                semantic: rt
                    .and_then(|rt| rt.semantic_trace.as_ref())
                    .map(Arc::downgrade),
            });
        }
        let boundary_fingerprint = boundary_hasher.finish();

        let mut collision_hasher = std::collections::hash_map::DefaultHasher::new();
        for &move_index in &self.checks.rapid_collision_move_indices {
            move_index.hash(&mut collision_hasher);
        }
        if let Some(report) = self.checks.collision_report.as_ref() {
            for collision in &report.collisions {
                collision.move_index.hash(&mut collision_hasher);
            }
        }
        self.checks
            .checked_scope
            .toolpath_id
            .hash(&mut collision_hasher);
        let collision_fingerprint = collision_hasher.finish();

        IssueListCacheKey {
            cut_trace: self
                .results
                .as_ref()
                .and_then(|results| results.cut_trace.as_ref())
                .map(Arc::downgrade),
            toolpath_traces,
            boundary_fingerprint,
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
            self.locate_move(issue.move_index)
                .map(|loc| loc.toolpath_id)
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
