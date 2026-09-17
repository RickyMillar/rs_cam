//! Collision checks, triage and the diagnostic findings.
//!
//! Holds the collision and kinematic reads, the two triage entries, the
//! evidence-bearing diagnostics pass, and the air-cut and plunge-stress
//! offender scans it reports on. Split out of `session/compute.rs` (P4).

use std::sync::atomic::AtomicBool;

use tracing::instrument;

use crate::compute::collision_check::{
    CollisionCheckRequest, CollisionCheckResult, HolderCollisionCheck, run_collision_check,
};
use crate::compute::cutter::build_cutter;
use crate::compute::tool_config::ToolId;
use crate::ids::ToolpathId;
use crate::session::{
    ProjectDiagnostics, ProjectEvidence, ProjectSession, SessionError, ToolpathDiagnostic, Verdict,
    VerdictEvidence, VerdictKind, VerdictSeverity,
};

/// Outcome of scanning every toolpath's air-cut percentage against its
/// op-kind's high-water threshold.
///
/// Two lists, because a gate now has three possible answers, not two: it
/// fired, it stayed silent, or **it declined to answer**. Collapsing the
/// third into the second is exactly the failure the census measured — a
/// finishing pass under the measurement floor reads 95.9% air cut and trips
/// a 30% band while removing material perfectly well.
#[derive(Debug, Default)]
pub(super) struct AirCutScan {
    /// Toolpaths over their threshold.
    pub(super) offenders: Vec<AirCutOffender>,
    /// Toolpaths where the air-cut metric is `NotMeasurable`, so no verdict
    /// was formed either way.
    pub(super) abstentions: Vec<AirCutAbstention>,
}

/// One toolpath over its op-kind's air-cut threshold.
///
/// R-7 / census D6: carries the `ToolpathId` alongside the display name.
/// The verdict used to publish names only and re-resolve them to ids by
/// string match against `toolpath_configs`, so two toolpaths sharing a name
/// collapsed to whichever came first — the verdict then pointed the operator
/// at the innocent one. The identity now travels with the finding; the name
/// is for display.
#[derive(Debug, Clone)]
pub(super) struct AirCutOffender {
    pub(super) id: ToolpathId,
    pub(super) name: String,
    pub(super) air_cut_pct: f64,
}

/// One toolpath whose air-cut metric is not measurable. Same identity rule
/// as [`AirCutOffender`].
#[derive(Debug, Clone)]
pub(super) struct AirCutAbstention {
    pub(super) id: ToolpathId,
    pub(super) name: String,
    pub(super) reason: crate::stock::sim_measurability::MeasurabilityReason,
}

/// Identify toolpaths whose air-cut percentage exceeds their op-kind's
/// high-water threshold — **abstaining** where the metric is not measurable.
///
/// Pure helper; takes only the data it needs so it can be unit-tested
/// without constructing a full `SimulationResult`. See
/// `planning/P1_AIR_CUT_THRESHOLDS_RCA.md` for the threshold rationale and
/// [`crate::stock::sim_measurability`] for the abstention rule (Checkpoint D Q2,
/// 2026-08-04). No threshold moved; a `NotMeasurable` metric simply stops
/// feeding this gate.
pub(super) fn air_cut_offenders_for_toolpaths(
    toolpath_summaries: &[crate::stock::simulation_cut::SimulationToolpathCutSummary],
    toolpath_configs: &[super::ToolpathConfig],
    measurability: &crate::stock::sim_measurability::MeasurabilityReport,
) -> AirCutScan {
    use crate::stock::sim_measurability::SimMetric;
    use crate::stock::simulation_cut::AirCutRatios;

    let mut scan = AirCutScan::default();
    for tp_summary in toolpath_summaries {
        if tp_summary.total_runtime_s <= 0.0 {
            continue;
        }
        let Some(tc) = toolpath_configs
            .iter()
            .find(|t| t.id == tp_summary.toolpath_id)
        else {
            continue;
        };
        // Census D5: a disabled toolpath is not part of the job. Its summary
        // can outlive the disable (traces are not cleared on toggle), so
        // without this the operator gets a warning about an op that will not
        // run, and no way to make it go away. The sibling
        // `plunge_stress_offenders_for_session` has always checked this.
        if !tc.enabled {
            continue;
        }
        let Some(threshold) = tc.operation.op_type().air_cut_high_threshold_pct() else {
            continue;
        };
        // Checkpoint D Q2: the metric this gate reads may not be a
        // measurement at all. Abstain with the reason rather than compare a
        // non-number against a threshold. Note the abstention is recorded,
        // not swallowed — the caller publishes it.
        let verdict = measurability.for_metric(tp_summary.toolpath_id, SimMetric::AirCut);
        if verdict.abstains() {
            if let Some(reason) = verdict.reason() {
                scan.abstentions.push(AirCutAbstention {
                    id: tc.id,
                    name: tc.name.clone(),
                    reason,
                });
            }
            continue;
        }
        // LH-1: `air_cut_high_threshold_pct` is defined against TOTAL runtime
        // (cutting + rapids). Named accessor, not a bare division, so the
        // choice is visible here and cannot silently drift to the
        // cutting-time reading the MCP narration prints.
        let air_pct = tp_summary.air_cut_pct_of_total_runtime();
        if air_pct > threshold {
            scan.offenders.push(AirCutOffender {
                id: tc.id,
                name: tc.name.clone(),
                air_cut_pct: air_pct,
            });
        }
    }
    scan
}

/// Identify toolpaths whose configured plunge rate exceeds the safe cap
/// for the tool's geometry (ball / tapered-ball). Returns `(name,
/// plunge_rate, cap)` triples in toolpath order.
///
/// See `planning/P2_PLUNGE_STRESS_GATE_RCA.md`.
pub(super) fn plunge_stress_offenders_for_session(
    session: &ProjectSession,
) -> Vec<(String, f64, f64)> {
    use crate::compute::tool_config::ToolId;
    use crate::tool::MillingCutter;
    use crate::tool_load::plunge_stress::check_plunge_stress;

    let mut offenders = Vec::new();
    for tc in &session.toolpath_configs {
        if !tc.enabled {
            continue;
        }
        let Some(tool_cfg) = session.get_tool(ToolId(tc.tool_id)) else {
            continue;
        };
        let tool_def = crate::compute::cutter::build_cutter(tool_cfg);
        let geometry = tool_def.to_geometry_hint();
        let plunge_rate = tc.operation.plunge_rate();
        if plunge_rate <= 0.0 {
            continue;
        }
        if let Some(w) = check_plunge_stress(geometry, tool_def.diameter(), plunge_rate) {
            offenders.push((tc.name.clone(), w.plunge_rate_mm_min, w.safe_cap_mm_min));
        }
    }
    offenders
}

impl ProjectSession {
    /// Run a collision check for a specific toolpath by index.
    #[instrument(skip(self))]
    pub fn collision_check(
        &self,
        index: usize,
        cancel: &AtomicBool,
    ) -> Result<CollisionCheckResult, SessionError> {
        self.collision_check_with_index(index, None, cancel)
    }

    /// [`Self::collision_check`] with a spatial index the caller already
    /// built over this toolpath's model mesh (CMP-24). The sweep hands one
    /// index per distinct model; a single check hands `None`.
    fn collision_check_with_index(
        &self,
        index: usize,
        prebuilt_index: Option<&crate::mesh::SpatialIndex>,
        cancel: &AtomicBool,
    ) -> Result<CollisionCheckResult, SessionError> {
        let tc = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        let result = self
            .results
            .get(&index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        let model = self
            .find_model_by_raw_id(tc.model_id)
            .and_then(|m| m.mesh.as_ref())
            .ok_or_else(|| {
                SessionError::MissingGeometry("Collision check requires a 3D mesh".to_owned())
            })?;

        let tool = self
            .find_tool_by_raw_id(tc.tool_id)
            .ok_or(SessionError::ToolNotFound(ToolId(tc.tool_id)))?;
        let tool_def = build_cutter(tool);

        let request = CollisionCheckRequest {
            toolpath: result.toolpath(),
            tool: tool_def,
            mesh: model,
            obstacles: self.collision_obstacles_for_toolpath(index),
            index: prebuilt_index,
        };
        let check_result = run_collision_check(&request, cancel)?;
        Ok(check_result)
    }

    /// Build the fixture obstacle set for a toolpath's setup — every
    /// enabled fixture as a clearance-expanded axis-aligned box. The
    /// single source of truth shared by the core collision check above
    /// and the GUI worker path, so both flag holder-vs-fixture crashes
    /// identically (W0.1 / P6-003). Returns empty when the toolpath has
    /// no setup or no enabled fixtures.
    pub fn collision_obstacles_for_toolpath(
        &self,
        index: usize,
    ) -> Vec<crate::stock::collision::CollisionObstacle> {
        let Some(setup) = self.find_setup_for_toolpath_index(index) else {
            return Vec::new();
        };
        setup
            .fixtures
            .iter()
            .filter(|f| f.enabled)
            .map(|f| {
                let c = f.clearance;
                crate::stock::collision::CollisionObstacle {
                    id: f.id.0,
                    aabb: crate::geo::BoundingBox3 {
                        min: crate::geo::P3::new(f.origin_x - c, f.origin_y - c, f.origin_z - c),
                        max: crate::geo::P3::new(
                            f.origin_x + f.size_x + c,
                            f.origin_y + f.size_y + c,
                            f.origin_z + f.size_z + c,
                        ),
                    },
                }
            })
            .collect()
    }

    /// Narrate one generated toolpath in prose for agent-oriented debugging.
    #[instrument(skip(self))]
    pub fn narrate_toolpath(&self, index: usize) -> Result<String, SessionError> {
        let tc = self
            .toolpath_configs
            .get(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        let result = self
            .results
            .get(&index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        let tool = self
            .find_tool_by_raw_id(tc.tool_id)
            .ok_or(SessionError::ToolNotFound(ToolId(tc.tool_id)))?;
        let tool_def = build_cutter(tool);
        let cut_trace = self
            .simulation
            .as_ref()
            .and_then(|sim| sim.cut_trace.as_deref());
        // Checkpoint D Q2: narration reads the SAME measurability report the
        // gates and the triage do, so it cannot publish a percentage the
        // gates have already declined to act on.
        let measurability = cut_trace.map(|trace| {
            crate::stock::sim_measurability::MeasurabilityReport::from_trace(
                trace,
                self.simulation.as_ref().map(|sim| sim.column_grid_cell_mm),
            )
        });
        let mut context = crate::trace::narrate::ToolpathNarrationContext {
            measurability: measurability.as_ref(),
            toolpath_id: Some(tc.id),
            toolpath_name: Some(tc.name.as_str()),
            operation_label: Some(tc.operation.label()),
            operation_kind: Some(tc.operation.op_type()),
            depth_per_pass_mm: tc.operation.depth_per_pass(),
            stepover_mm: tc.operation.stepover(),
            tool_diameter_mm: Some(tool.diameter),
            feed_rate_mm_min: Some(tc.operation.feed_rate()),
            spindle_rpm: Some(
                tc.operation
                    .spindle_rpm()
                    .unwrap_or(self.post.spindle_speed),
            ),
            flute_count: Some(tool.flute_count),
            // B7 divergence 1, resolved 2026-08-06: the intent-aware
            // expression is now shared with the GUI's narration via
            // `is_drill_cycle_for_narration`, so the two readers can no
            // longer disagree about whether a toolpath is a drill cycle.
            is_drill_cycle: crate::trace::narrate::is_drill_cycle_for_narration(
                tc.operation.op_type(),
                &result.annotated().toolpath.moves,
            ),
            material: Some(&self.stock.material),
            // Every ToolpathStats-derived channel is filled by
            // `absorb_stats` below — one exhaustive join, so a new finding
            // cannot reach one narration and miss the other (B7).
            ..Default::default()
        };
        context.absorb_stats(&result.stats);

        Ok(crate::trace::narrate::narrate_toolpath_with_context(
            result.annotated(),
            result.semantic_trace.as_ref(),
            cut_trace,
            result.debug_trace.as_ref(),
            &tool_def,
            &context,
        ))
    }

    /// Compute project diagnostics from current results and the
    /// session's cached simulation. Consumers that hold sim evidence
    /// outside the core session (the GUI keeps it on viz-side state)
    /// should call [`Self::diagnostics_with_evidence`] instead.
    ///
    /// This batch entry point runs the holder/shank collision sweep
    /// (one `collision_check` per computed toolpath) to build full
    /// evidence — appropriate for CLI/export, NOT for per-frame UI.
    #[instrument(skip(self))]
    /// [`Self::simulation_triage`] against this session's own simulation —
    /// the convenience path for batch callers that do not assemble their own
    /// [`ProjectEvidence`].
    pub fn triage(&self) -> crate::stock::sim_triage::SimulationTriage {
        let no_cancel = AtomicBool::new(false);
        let holder_collisions = self.holder_collision_counts(&no_cancel);
        let Some(sim) = self.simulation.as_ref() else {
            return crate::stock::sim_triage::SimulationTriage::default();
        };
        let evidence =
            ProjectEvidence::from_simulation_with_holder_collisions(sim, holder_collisions);
        self.simulation_triage(&evidence)
    }

    pub fn diagnostics(&self) -> ProjectDiagnostics {
        let no_cancel = AtomicBool::new(false);
        let holder_collisions = self.holder_collision_counts(&no_cancel);
        let evidence = self
            .simulation
            .as_ref()
            .map(|sim| {
                ProjectEvidence::from_simulation_with_holder_collisions(
                    sim,
                    holder_collisions.clone(),
                )
            })
            .unwrap_or_else(|| ProjectEvidence {
                holder_collisions: holder_collisions.clone(),
                ..ProjectEvidence::default()
            });
        self.diagnostics_with_evidence(&evidence)
    }

    /// One spatial index per DISTINCT model the computed toolpaths bind,
    /// keyed by the model's raw id (CMP-24).
    ///
    /// The sweep below used to build one index per TOOLPATH, so a project
    /// whose N toolpaths bind one model paid for the same index N times.
    /// The build is the expensive half of a collision check, and the
    /// session's own answer to that cost was a doc line telling callers
    /// not to call it. This hoists the build instead.
    ///
    /// A toolpath whose model carries no mesh contributes no entry, and
    /// the sweep then reads [`HolderCollisionCheck::NotApplicable`] for it.
    pub(crate) fn holder_collision_indices(
        &self,
    ) -> std::collections::BTreeMap<usize, crate::mesh::SpatialIndex> {
        let mut indices = std::collections::BTreeMap::new();
        for (idx, tc) in self.toolpath_configs.iter().enumerate() {
            if !self.results.contains_key(&idx) || indices.contains_key(&tc.model_id) {
                continue;
            }
            let Some(mesh) = self
                .find_model_by_raw_id(tc.model_id)
                .and_then(|m| m.mesh.as_ref())
            else {
                continue;
            };
            indices.insert(tc.model_id, crate::mesh::SpatialIndex::build_auto(mesh));
        }
        indices
    }

    /// Run the holder/shank collision check for every computed toolpath
    /// and return one [`HolderCollisionCheck`] per toolpath.
    ///
    /// **A check that FAILED is not a check that found nothing** (CMP-14).
    /// This used to answer `0` for a failure, a not-applicable check and a
    /// clean check alike, so no consumer could tell them apart — on the one
    /// question that wrecks a machine. The three states are now distinct at
    /// the producer and every consumer decides for itself.
    ///
    /// This is the expensive sweep (one spatial-index build per distinct
    /// model plus an interpolated toolpath walk per toolpath). Interactive
    /// surfaces should reuse their last dedicated check instead of calling
    /// this per frame.
    pub fn holder_collision_counts(
        &self,
        cancel: &AtomicBool,
    ) -> Vec<(ToolpathId, HolderCollisionCheck)> {
        let indices = self.holder_collision_indices();
        self.toolpath_configs
            .iter()
            .enumerate()
            .filter(|(idx, _)| self.results.contains_key(idx))
            .map(|(idx, tc)| {
                let outcome =
                    match self.collision_check_with_index(idx, indices.get(&tc.model_id), cancel) {
                        Ok(r) => {
                            HolderCollisionCheck::Measured(r.collision_report.collisions.len())
                        }
                        // The 2D case the CLI calls expected: no mesh, so
                        // nothing to check against.
                        Err(SessionError::MissingGeometry(_)) => {
                            HolderCollisionCheck::NotApplicable
                        }
                        Err(e) => {
                            tracing::warn!(
                                index = idx,
                                error = %e,
                                "Holder collision check failed; the count is unknown"
                            );
                            HolderCollisionCheck::Failed
                        }
                    };
                (tc.id, outcome)
            })
            .collect()
    }

    /// The Phase 4 kinematic reading for the toolpath at `index`, measured
    /// over the move list the CALLER holds.
    ///
    /// This is the single construction site for
    /// [`crate::machine::kinematic_utilization::analyse_toolpath`] in the session.
    /// It takes `toolpath` rather than fetching one, because a caller that
    /// already holds a result must be measured on THAT result. The GUI's
    /// MCP narration is the case: it narrates the move list on its own
    /// runtime overlay, and re-fetching by index would describe one
    /// toolpath's motion with another generation's numbers — the "B7
    /// divergence" class its handler already fought once.
    ///
    /// Returns `None` when the toolpath is disabled or does not exist. The
    /// caller's `toolpath` is the evidence; this function never asks
    /// whether a result exists.
    ///
    /// The feed envelope MIRRORS the production runtime integrator in
    /// `apply_adaptive_feed_modulation` (`max_feed` is the CUTTING
    /// ceiling, `rapid_feed` honours `high_feedrate_mode`, kinematics come
    /// from `effective_kinematics`, which never returns `None`). Keep the two
    /// in step: the instrument and the integrator must not disagree about the
    /// machine — that is the "one physics site" ruling of the Phase 1 plan.
    ///
    /// `trace` is the simulation the caller is reading, or `None`. It decides
    /// only the reading's [`crate::machine::kinematic_utilization::FeedsProvenance`],
    /// never a number. Pass the trace the caller displays beside this reading;
    /// the GUI keeps its trace on viz state, so the session's own
    /// `self.simulation` is the wrong source there.
    pub fn kinematic_utilization_of(
        &self,
        index: usize,
        toolpath: &crate::toolpath::Toolpath,
        trace: Option<&crate::stock::simulation_cut::SimulationCutTrace>,
    ) -> Option<crate::machine::kinematic_utilization::ToolpathKinematicUtilization> {
        let tc = self.toolpath_configs.get(index)?;
        if !tc.enabled {
            return None;
        }
        let kinematics = self.machine.effective_kinematics();
        let max_feed = self.machine.cutting_feed_ceiling_mm_min().max(1.0);
        let rapid_feed = if self.post.high_feedrate_mode {
            self.post.high_feedrate.max(1.0)
        } else {
            self.machine.max_feed_mm_min.max(1.0)
        };
        let mut util = crate::machine::kinematic_utilization::analyse_toolpath(
            toolpath,
            tc.id,
            &kinematics,
            max_feed,
            rapid_feed,
            tc.operation.plunge_rate(),
        );
        util.feeds_provenance = Self::feeds_provenance_of(tc.id, toolpath, trace);
        Some(util)
    }

    /// Does `toolpath` carry the feeds the post-processor will emit?
    ///
    /// The question is answered from the EVIDENCE, never from timing: every
    /// modulated feed the trace recorded for this toolpath must be present on
    /// this move list. That is true of `self.results` after
    /// [`Self::apply_adaptive_feed_modulation`] wrote the modulated clone
    /// back, and false of a worker's pre-modulation IR — which is exactly the
    /// list the GUI's MCP narration holds.
    ///
    /// A trace that records NO modulated feed for the toolpath reads
    /// `Emitted`: the modulator did not fire (no vendor band, no cut
    /// samples), so the stored plan IS what export emits. No trace at all
    /// reads `Planned`.
    fn feeds_provenance_of(
        id: ToolpathId,
        toolpath: &crate::toolpath::Toolpath,
        trace: Option<&crate::stock::simulation_cut::SimulationCutTrace>,
    ) -> crate::machine::kinematic_utilization::FeedsProvenance {
        use crate::machine::kinematic_utilization::FeedsProvenance;
        let Some(trace) = trace else {
            return FeedsProvenance::Planned;
        };
        for ((tp_id, move_index), (feed, _binding)) in &trace.modulated_feeds {
            if *tp_id != id {
                continue;
            }
            let Some(m) = toolpath.moves.get(*move_index) else {
                return FeedsProvenance::Planned;
            };
            let Some(actual) = m.move_type.feed_rate() else {
                return FeedsProvenance::Planned;
            };
            if (actual - feed).abs() > 1e-6 {
                return FeedsProvenance::Planned;
            }
        }
        FeedsProvenance::Emitted
    }

    /// [`Self::kinematic_utilization_of`] on the session's OWN stored result
    /// for `index`, or `None` when there is none.
    ///
    /// The convenience form, for a caller that holds no result of its own —
    /// the tool-load report, the triage builder and the CLI. A caller that
    /// does hold one must call [`Self::kinematic_utilization_of`] with it.
    pub fn kinematic_utilization_for(
        &self,
        index: usize,
        trace: Option<&crate::stock::simulation_cut::SimulationCutTrace>,
    ) -> Option<crate::machine::kinematic_utilization::ToolpathKinematicUtilization> {
        let result = self.results.get(&index)?;
        self.kinematic_utilization_of(index, &result.annotated().toolpath, trace)
    }

    /// [`Self::kinematic_utilization_for`] over every enabled toolpath that
    /// carries a result, keyed by [`ToolpathId`].
    ///
    /// Drill ops are NOT excluded. A drill cycle's descents are exactly the
    /// population the plunge-class backstop exists to grade, and the
    /// engagement-metric exclusion that keeps drills out of
    /// `toolpath_summaries` says nothing about their Z rates.
    pub fn kinematic_utilizations(
        &self,
        trace: Option<&crate::stock::simulation_cut::SimulationCutTrace>,
    ) -> std::collections::BTreeMap<
        ToolpathId,
        crate::machine::kinematic_utilization::ToolpathKinematicUtilization,
    > {
        (0..self.toolpath_configs.len())
            .filter_map(|idx| {
                let util = self.kinematic_utilization_for(idx, trace)?;
                Some((util.toolpath_id, util))
            })
            .collect()
    }

    /// Same as [`Self::diagnostics`] but takes a borrow view over
    /// simulation evidence. Used by the GUI MCP handler where the
    /// active sim lives on viz-side state, not on the core session.
    ///
    /// Rapid collision counts come from the supplied evidence (which checks
    /// against the actual remaining stock surface). If no evidence is
    /// supplied, rapid collision counts are 0 — we don't fall back to the
    /// inaccurate original-bbox check.
    #[instrument(skip_all)]
    /// The page-one answer: one [`crate::stock::sim_triage::SimulationTriage`] for
    /// every consumer — GUI panel, MCP JSON, CLI report, narration.
    ///
    /// This is the single construction site on purpose. The census found
    /// five surfaces each assembling, ranking and truncating the issue
    /// channel their own way, which is how the GUI came to rank collisions
    /// last while `sim_op_list.rs` ranked them first. Anything that wants to
    /// answer "what should I act on?" calls this; nothing re-derives it.
    pub fn simulation_triage(
        &self,
        evidence: &ProjectEvidence<'_>,
    ) -> crate::stock::sim_triage::SimulationTriage {
        self.simulation_triage_with_diagnostics(evidence, &self.diagnostics_with_evidence(evidence))
    }

    /// [`Self::simulation_triage`] for a caller that has ALREADY built the
    /// [`ProjectDiagnostics`] for this same evidence and wants to publish
    /// both — the MCP `get_diagnostics` response is exactly that shape (a
    /// per-toolpath diagnostic array plus a triage block).
    ///
    /// TD3 wave B-5. Without this seam that response builds the project
    /// diagnostics twice per call: once for its own `per_toolpath` rows and
    /// once inside `simulation_triage`. The alternative — the GUI keeping a
    /// hand-rolled per-toolpath row so it only pays for the triage — is what
    /// this wave removed, and it is what dropped ten published channels off
    /// the agent-facing wire in the first place.
    ///
    /// `project_diagnostics` MUST be `self.diagnostics_with_evidence(evidence)`
    /// for the same `evidence`; passing anything else makes the triage
    /// describe a project state that never existed.
    pub fn simulation_triage_with_diagnostics(
        &self,
        evidence: &ProjectEvidence<'_>,
        project_diagnostics: &ProjectDiagnostics,
    ) -> crate::stock::sim_triage::SimulationTriage {
        use crate::stock::sim_triage::{SimulationTriage, TriageInputs};

        let Some(trace) = evidence.cut_trace else {
            return SimulationTriage::default();
        };
        let diagnostics =
            crate::diagnostics::adapters::from_project_diagnostics::diagnostics_from_project(
                project_diagnostics,
            );
        let measurability = crate::stock::sim_measurability::MeasurabilityReport::from_trace(
            trace,
            evidence.resolution_mm,
        );
        let tool_diameters_mm = self
            .toolpath_configs
            .iter()
            .filter_map(|tc| {
                let tool_cfg = self.get_tool(crate::compute::tool_config::ToolId(tc.tool_id))?;
                let cutter = crate::compute::cutter::build_cutter(tool_cfg);
                Some((tc.id, crate::tool::MillingCutter::diameter(&cutter)))
            })
            .collect();

        // G-ENTRYLOAD scope: the same predicate that arms the pencil entry
        // ramp (`gen_initial_stock`) — entries are graded only where they
        // descend through rest material the entry planner had no stock
        // reading for. Fresh-stock entry plunges are planned motion.
        let rest_driven: std::collections::BTreeSet<crate::ids::ToolpathId> = self
            .toolpath_configs
            .iter()
            .filter(|tc| tc.stock_source == crate::session::StockSource::FromRemainingStock)
            .map(|tc| tc.id)
            .collect();

        // Phase 4 — the plunge-class backstop's population. Every enabled
        // toolpath with a result, drill ops included; the finding itself is
        // NOT gated on `rest_driven`, because an untagged vertical descent is
        // a defect on fresh stock exactly as it is on rest stock.
        // The trace the caller is triaging decides the readings' feeds
        // provenance (Phase 3), so it is the one to pass — not
        // `self.simulation`, which may hold a different run, or none.
        // Since N12 item 10 the GUI adopts its simulation into the
        // session through `Command::AdoptSimulation`, so that slot is
        // populated in the GUI process too; the rule is unchanged,
        // because the caller's own trace is still the one it displays.
        let kinematic_utilization = self.kinematic_utilizations(Some(trace));

        SimulationTriage::build_with_rest_context(
            &TriageInputs {
                trace,
                measurability: &measurability,
                diagnostics: &diagnostics,
                rapid_collisions: evidence.rapid_collisions,
                holder_collisions: &evidence.holder_collisions,
                tool_diameters_mm: &tool_diameters_mm,
                kinematic_utilization: &kinematic_utilization,
                region_of: None,
            },
            &rest_driven,
        )
    }

    pub fn diagnostics_with_evidence(&self, evidence: &ProjectEvidence<'_>) -> ProjectDiagnostics {
        let mut per_toolpath = Vec::new();
        let mut total_collision_count: usize = 0;
        let mut collision_checks_failed: usize = 0;
        let mut total_rapid_collision_count: usize = 0;

        // Per-TP context collected in the toolpath loop for later verdict
        // emission. We index by `toolpath_id` (which equals `tc.id`).
        struct RapidWorst {
            move_index: usize,
            z: f64,
        }
        let mut holder_collisions_by_tp: Vec<(ToolpathId, String, usize)> = Vec::new();
        let mut failed_checks_by_tp: Vec<(ToolpathId, String)> = Vec::new();
        let mut rapid_collisions_by_tp: Vec<(ToolpathId, String, usize, RapidWorst)> = Vec::new();
        let mut empty_results_by_tp: Vec<(ToolpathId, String, &str)> = Vec::new();

        // Build per-boundary maps from the simulation result. Each boundary
        // maps to one toolpath via its `id`.
        type RapidCountsByBoundary = Vec<(ToolpathId, usize)>;
        type RapidWorstByBoundary = Vec<(ToolpathId, RapidWorst)>;
        let (rapid_counts_by_boundary, rapid_worst_by_boundary): (
            RapidCountsByBoundary,
            RapidWorstByBoundary,
        ) = {
            let counts = evidence
                .boundaries
                .iter()
                .map(|&(id, start, end)| {
                    let count = evidence
                        .rapid_collision_move_indices
                        .iter()
                        .filter(|&&mi| mi >= start && mi < end)
                        .count();
                    (id, count)
                })
                .collect();

            // Pick the worst (lowest end.z = deepest descent) rapid collision
            // per boundary so the verdict layer can cite a representative
            // move for the fix hint.
            let worst = evidence
                .boundaries
                .iter()
                .filter_map(|&(id, start, end)| {
                    evidence
                        .rapid_collisions
                        .iter()
                        .filter(|rc| rc.move_index >= start && rc.move_index < end)
                        .min_by(|a, c| {
                            a.end
                                .z
                                .partial_cmp(&c.end.z)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                        .map(|rc| {
                            (
                                id,
                                RapidWorst {
                                    move_index: rc.move_index,
                                    z: rc.end.z,
                                },
                            )
                        })
                })
                .collect();

            (counts, worst)
        };

        for (idx, tc) in self.toolpath_configs.iter().enumerate() {
            if let Some(result) = self.results.get(&idx) {
                // Look up rapid collision count + worst move from simulation
                // boundaries.
                let rapid_count = rapid_counts_by_boundary
                    .iter()
                    .filter(|(id, _)| *id == tc.id)
                    .map(|(_, count)| *count)
                    .sum::<usize>();
                total_rapid_collision_count += rapid_count;

                if rapid_count > 0
                    && let Some((_, worst)) =
                        rapid_worst_by_boundary.iter().find(|(id, _)| *id == tc.id)
                {
                    rapid_collisions_by_tp.push((
                        tc.id,
                        tc.name.clone(),
                        rapid_count,
                        RapidWorst {
                            move_index: worst.move_index,
                            z: worst.z,
                        },
                    ));
                }

                // Holder/shank collisions come from the supplied evidence
                // (the caller's most recent dedicated check, or
                // `holder_collision_counts` for batch paths). This used
                // to run `collision_check` inline — a spatial-index
                // build + full toolpath sweep PER TOOLPATH — which the
                // GUI setup panel then executed every frame (2026-06-11
                // setup-tab lag). Diagnostics consume evidence; they do
                // not compute it.
                //
                // CMP-14, the second `unwrap_or(0)`: a toolpath ABSENT
                // from the evidence was read as a measured zero here,
                // summed into `collision_count` and tested with `> 0`.
                // The GUI supplies only the toolpaths its last check
                // found hits on, so absence there means NOT MEASURED. It
                // now reports `None` and raises no verdict — a toolpath
                // nobody checked is neither clean nor dirty.
                let holder_check = evidence
                    .holder_collisions
                    .iter()
                    .find(|(id, _)| *id == tc.id)
                    .map(|(_, check)| *check);
                let holder_collision_count = holder_check.and_then(HolderCollisionCheck::count);
                total_collision_count += holder_collision_count.unwrap_or(0);
                if holder_check.is_some_and(HolderCollisionCheck::failed) {
                    collision_checks_failed += 1;
                    failed_checks_by_tp.push((tc.id, tc.name.clone()));
                }

                if let Some(count) = holder_collision_count
                    && count > 0
                {
                    holder_collisions_by_tp.push((tc.id, tc.name.clone(), count));
                }

                // C7: detect toolpaths that generated successfully but laid
                // down zero in-material cut. Drill kinematics are exempt —
                // the dexel-side cutting metric doesn't apply to Z-only ops.
                let op_type = tc.operation.op_type();
                if result.stats.cutting_distance <= 0.0 && !op_type.is_drill_kinematics() {
                    empty_results_by_tp.push((tc.id, tc.name.clone(), op_type.label()));
                }

                let tool_name = self
                    .find_tool_by_raw_id(tc.tool_id)
                    .map(|t| t.name.clone())
                    .unwrap_or_default();

                per_toolpath.push(ToolpathDiagnostic {
                    toolpath_id: tc.id,
                    name: tc.name.clone(),
                    operation_type: op_type.label().to_owned(),
                    op_kind: op_type.kind_str().to_owned(),
                    tool_name,
                    move_count: result.stats.move_count,
                    cutting_distance_mm: result.stats.cutting_distance,
                    rapid_distance_mm: result.stats.rapid_distance,
                    // `null` = the check FAILED, so no count exists.
                    // Consumers must not coerce it to 0 (CMP-14).
                    collision_count: holder_collision_count,
                    rapid_collision_count: rapid_count,
                    truncated_core_mm2: result.stats.truncated_core_mm2,
                    // B8 — the untouched/standing split, on the same wire as
                    // the core it must not be confused with.
                    untouched_material_mm2: result.stats.untouched_material_mm2,
                    reached_uncut_estimate_mm2: result.stats.reached_uncut_estimate_mm2,
                    // Wave D1 — the MCP wire. `None` serialises null.
                    unmachined_band_area_mm2: result
                        .stats
                        .dropped_band
                        .as_deref()
                        .map(|f| f.area_mm2),
                    tip_float_points: result.stats.tip_float.map(|f| f.floating_points),
                    max_tip_float_mm: result.stats.tip_float.map(|f| f.max_float_mm),
                    // C2 follow-up 2 — the shallow band's decomposition
                    // telemetry, carried whole. `None` = not measured.
                    monotone_cells: result.stats.monotone_cells,
                });
            }
        }

        // Extract simulation metrics if available.
        //
        // LH-1: air cut is published under BOTH denominators, each named.
        // Every threshold in this file and in the GUI is tuned against the
        // total-runtime reading. The cutting-time reading (what the MCP
        // narration reports) travels beside it under its own name.
        let (
            total_runtime_s,
            air_cut_pct_of_total_runtime,
            air_cut_pct_of_cutting_time,
            average_engagement,
        ) = if let Some(trace) = evidence.cut_trace {
            use crate::stock::simulation_cut::AirCutRatios;
            let summary = &trace.summary;
            (
                summary.total_runtime_s,
                summary.air_cut_pct_of_total_runtime(),
                summary.air_cut_pct_of_cutting_time(),
                summary.average_engagement,
            )
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };

        // Per-TP op-kind-aware air-cut warnings. A blanket project-wide
        // threshold (the old `>40%`) fires falsely on projects that contain
        // even one sparse-pattern op like ProjectCurve, where 80–95% air-cut
        // is intrinsic. Per-TP thresholds live on `OperationType`
        // (see `OperationType::air_cut_high_threshold_pct`).
        // P1 — see planning/P1_AIR_CUT_THRESHOLDS_RCA.md.
        //
        // Checkpoint D Q2 (2026-08-04): before comparing anything against a
        // threshold, ask whether the number is a measurement. A pass under
        // the dexel's 0.05 mm fresh-material floor removes material fine and
        // reports engagement of exactly zero, i.e. ~96% air cut — which
        // trips every shipped band. Those toolpaths ABSTAIN, with the reason
        // published beside the verdicts rather than silently dropped.
        let measurability = evidence
            .cut_trace
            .map(|trace| {
                crate::stock::sim_measurability::MeasurabilityReport::from_trace(
                    trace,
                    evidence.resolution_mm,
                )
            })
            .unwrap_or_default();
        let air_cut_scan = evidence
            .cut_trace
            .map(|trace| {
                air_cut_offenders_for_toolpaths(
                    &trace.toolpath_summaries,
                    &self.toolpath_configs,
                    &measurability,
                )
            })
            .unwrap_or_default();
        let air_cut_offenders = air_cut_scan.offenders;
        let air_cut_abstentions = air_cut_scan.abstentions;

        // P2: plunge-stress warnings for small ball / tapered-ball tools.
        // Fix 2 caps fresh LUT recommendations, but pre-Fix-2 projects carry
        // static-default plunge rates that bypass the cap. See
        // `planning/P2_PLUNGE_STRESS_GATE_RCA.md`.
        let plunge_stress_offenders = plunge_stress_offenders_for_session(self);

        // ── Build the structured verdict list (severity-ranked) ───────
        let mut verdicts: Vec<Verdict> = Vec::new();

        // Critical: holder/shank collision per TP.
        for (id, name, count) in &holder_collisions_by_tp {
            verdicts.push(Verdict {
                severity: VerdictSeverity::Critical,
                kind: VerdictKind::HolderCollision,
                headline: format!(
                    "ERROR: holder/shank collisions on TP{id} '{name}' ({count} collisions)"
                ),
                offender_toolpath_ids: vec![*id],
                fix_hint: "Increase tool stickout, switch to a tool with a smaller holder, \
                           or raise the operation's safe-Z. Re-run collision check after \
                           the change."
                    .to_owned(),
                evidence: VerdictEvidence {
                    move_index: None,
                    z_value: None,
                    count: Some(*count),
                },
            });
        }

        // Important: the alignment pins cannot register the flip a setup
        // is programmed for (CMP-27).
        //
        // The judgement is `StockConfig::validate_pins_for_flip`, whose
        // own doc asks callers to run it on load and publish its
        // warnings. Three production call sites did, all in viz, so the
        // CLI's headless load and MCP `load_project` heard nothing and
        // `add_alignment_pin` applied a pin with no check at all. Raised
        // here, every surface that reads diagnostics gets the same
        // answer, and it stays current as pins are added and removed
        // rather than being frozen at load time.
        //
        // Deduped by headline: a `Top` setup contributes only the bounds
        // line, and a project with several setups repeats it.
        {
            let mut seen = std::collections::HashSet::new();
            for setup in &self.setups {
                for detail in self.stock.validate_pins_for_flip(setup.face_up).warnings() {
                    if !seen.insert(detail.clone()) {
                        continue;
                    }
                    verdicts.push(Verdict {
                        severity: VerdictSeverity::Important,
                        kind: VerdictKind::AlignmentPinsUnkeyed,
                        headline: detail,
                        offender_toolpath_ids: Vec::new(),
                        fix_hint: "Re-place the pins so the pair is asymmetric under the \
                                   mirror the flip is NOT, or let the stock panel place \
                                   them. A pair that seats in both orientations registers \
                                   neither."
                            .to_owned(),
                        evidence: VerdictEvidence {
                            move_index: None,
                            z_value: None,
                            count: None,
                        },
                    });
                }
            }
        }

        // Critical: a holder/shank check that could not answer. The
        // absence is the finding — CMP-14's whole point is that it must
        // not be reported as a clean toolpath.
        for (id, name) in &failed_checks_by_tp {
            verdicts.push(Verdict {
                severity: VerdictSeverity::Critical,
                kind: VerdictKind::HolderCheckFailed,
                headline: format!(
                    "NOT CHECKED: the holder/shank collision check failed on TP{id} '{name}'"
                ),
                offender_toolpath_ids: vec![*id],
                fix_hint: "Nothing is known about holder clearance on this toolpath. Re-run \
                           the collision check. If it fails again, confirm the toolpath's \
                           model still carries a mesh and its tool still exists."
                    .to_owned(),
                evidence: VerdictEvidence {
                    move_index: None,
                    z_value: None,
                    count: None,
                },
            });
        }

        // Critical: rapid-through-stock collisions per TP, with worst-move
        // context for the fix hint.
        for (id, name, count, worst) in &rapid_collisions_by_tp {
            verdicts.push(Verdict {
                severity: VerdictSeverity::Critical,
                kind: VerdictKind::RapidCollision,
                headline: format!(
                    "WARNING: rapid collisions on TP{id} '{name}' ({count} collisions, \
                     worst at move {move_index}, z={z:.3})",
                    move_index = worst.move_index,
                    z = worst.z,
                ),
                offender_toolpath_ids: vec![*id],
                fix_hint: "Likely cause: inter-region rapid moves not lifting to safe-Z. \
                           Fix: increase retract_z in operation params, raise safe-Z on \
                           the post-config, or set a tighter boundary so the lift path \
                           clears already-cut regions."
                    .to_owned(),
                evidence: VerdictEvidence {
                    move_index: Some(worst.move_index),
                    z_value: Some(worst.z),
                    count: Some(*count),
                },
            });
        }

        // Important: unsafe plunge feed on small ball / tapered-ball tools.
        for (name, rate, cap) in &plunge_stress_offenders {
            verdicts.push(Verdict {
                severity: VerdictSeverity::Important,
                kind: VerdictKind::PlungeStress,
                headline: format!(
                    "WARNING: unsafe plunge rate on '{name}' ({rate:.0} > cap {cap:.0} mm/min)"
                ),
                offender_toolpath_ids: self
                    .toolpath_configs
                    .iter()
                    .filter(|tc| tc.name == *name)
                    .map(|tc| tc.id)
                    .collect(),
                fix_hint: format!(
                    "Cap plunge feed at {cap:.0} mm/min for this tool geometry. Use \
                     the Feeds tab Suggest button to repopulate with safe values."
                ),
                evidence: VerdictEvidence {
                    move_index: None,
                    z_value: None,
                    count: None,
                },
            });
        }

        // Important: toolpath generated but produced zero in-material cut.
        for (id, name, op_label) in &empty_results_by_tp {
            verdicts.push(Verdict {
                severity: VerdictSeverity::Important,
                kind: VerdictKind::GeneratedEmpty,
                headline: format!(
                    "WARNING: TP{id} '{name}' ({op_label}) generated but produced zero \
                     in-material cut"
                ),
                offender_toolpath_ids: vec![*id],
                fix_hint: "Check that the setup orientation, stock alignment, and target \
                           model geometry overlap. For project_curve specifically, \
                           `depth` follows the \"positive = into material\" convention — \
                           a negative depth lifts the cutter into air. The \
                           project_curve_negative_depth validator rule flags this \
                           pre-generation; the project_summary.stale_defaults entry has \
                           a one-click flip-sign fix."
                    .to_owned(),
                evidence: VerdictEvidence {
                    move_index: None,
                    z_value: None,
                    count: None,
                },
            });
        }

        // Polish: air-cut high (per-TP, op-kind-aware threshold).
        if !air_cut_offenders.is_empty() {
            let names: Vec<String> = air_cut_offenders
                .iter()
                // LH-1: the threshold is on the TOTAL-RUNTIME measure; the
                // headline says so rather than leaving "air-cut" ambiguous.
                .map(|o| {
                    format!(
                        "'{}' is {:.0}% air-cut of total runtime",
                        o.name, o.air_cut_pct
                    )
                })
                .collect();
            // R-7: the id came with the finding. No name round-trip, so
            // duplicate names cannot collapse two toolpaths into one.
            let offender_ids: Vec<ToolpathId> = air_cut_offenders.iter().map(|o| o.id).collect();
            verdicts.push(Verdict {
                severity: VerdictSeverity::Polish,
                kind: VerdictKind::AirCut,
                headline: format!("WARNING: high air cutting on {}", names.join(", ")),
                offender_toolpath_ids: offender_ids,
                fix_hint: "Tighten the boundary, lower the stock-top, or pre-rough with \
                           a faster op so the finishing pass doesn't traverse uncut \
                           material. Negligible for sparse projection ops."
                    .to_owned(),
                evidence: VerdictEvidence {
                    move_index: None,
                    z_value: None,
                    count: None,
                },
            });
        }

        // Checkpoint D Q2: state every abstention. A gate that silently
        // declines is indistinguishable from a gate that passed, which is
        // the whole failure this ruling addresses — so the abstention gets a
        // verdict of its own, naming the toolpath, the reason, and what is
        // still trustworthy.
        if !air_cut_abstentions.is_empty() {
            let names: Vec<String> = air_cut_abstentions
                .iter()
                .map(|a| format!("'{}': {}", a.name, a.reason.describe()))
                .collect();
            verdicts.push(Verdict {
                severity: VerdictSeverity::Polish,
                kind: VerdictKind::MeasurabilityAbstained,
                headline: format!(
                    "NOT MEASURED: air-cut % withheld for {} toolpath(s) — {}",
                    air_cut_abstentions.len(),
                    names.join("; ")
                ),
                offender_toolpath_ids: air_cut_abstentions.iter().map(|a| a.id).collect(),
                fix_hint: "This is a statement about the SIMULATION, not the toolpath. \
                           Collision detection, material removal and axial DOC are \
                           unaffected and remain valid. Where the reason is a coarse \
                           cell, re-simulate below the tool's TIP radius; where it is \
                           the fixed 0.05 mm fresh-material floor, the engagement \
                           channel cannot see a pass this shallow at any resolution — \
                           judge it on removed material and surface quality instead."
                    .to_owned(),
                evidence: VerdictEvidence {
                    move_index: None,
                    z_value: None,
                    count: Some(air_cut_abstentions.len()),
                },
            });
        }

        // Sort by severity (Critical → Important → Polish). Within a
        // severity the insertion order above is the intended display order.
        verdicts.sort_by_key(|v| v.severity);

        ProjectDiagnostics {
            total_runtime_s,
            air_cut_pct_of_total_runtime,
            air_cut_pct_of_cutting_time,
            average_engagement,
            collision_count: total_collision_count,
            collision_checks_failed,
            rapid_collision_count: total_rapid_collision_count,
            per_toolpath,
            verdicts,
        }
    }

    // ── Export ──────────────────────────────────────────────────────
}
