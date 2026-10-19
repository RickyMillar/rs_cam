//! Collision checks, triage and the diagnostic findings.
//!
//! Holds the collision and kinematic reads, the two triage entries, the
//! evidence-bearing diagnostics pass, and the air-cut and plunge-stress
//! offender scans it reports on. Split out of `session/compute.rs` (P4).

use std::sync::atomic::AtomicBool;

use tracing::instrument;

use crate::compute::collision_check::{
    CollisionCheckRequest, CollisionCheckResult, run_collision_check,
};
use crate::compute::tool_config::ToolId;
use crate::ids::ToolpathId;
use crate::session::diagnostics_types::CutMetricsNotMeasured;
use crate::session::{
    ProjectDiagnostics, ProjectEvidence, ProjectSession, SessionError, ToolpathDiagnostic, Verdict,
    VerdictEvidence, VerdictKind, VerdictSeverity,
};
use crate::stock::collision::HolderCollisionCheck;

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
        let Some(threshold) = tc
            .operation
            .op_type()
            .capabilities()
            .air_cut_high_threshold_pct
        else {
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
        // F029: a tool outside its domain is skipped, as a missing tool is.
        let Ok(tool_def) = crate::compute::cutter::build_cutter(tool_cfg) else {
            continue;
        };
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

/// The session parts that the simulation triage reads, borrowed, built in
/// ONE place ([`TriageView::of`]) (F187).
///
/// [`ProjectSession::triage_from_parts`] reads the session through this view
/// only, and [`TriageSessionKey`] derives from the same view, so the key and
/// the build cannot read different state. Each part is either in
/// [`TriageSessionKey`] or in the load-report key
/// ([`crate::session::LoadReportStamp`]); the destructures in
/// [`TriageView::key`] and [`TriageView::rows`] have no `..`, so a new part
/// is a compile error until someone names its key.
///
/// `stock` and `setups` are not read by the triage assembly itself. They are
/// here because [`ProjectSession::diagnostics_with_evidence`] reads them (the
/// alignment-pin findings and each setup's face) and the triage cache keys
/// that read too. That pass still reads the whole session; its other reads
/// are in the load-report key.
pub(crate) struct TriageView<'a> {
    toolpaths: &'a [crate::session::ToolpathConfig],
    tools: &'a [crate::compute::tool_config::ToolConfig],
    stock: &'a crate::compute::stock_config::StockConfig,
    setups: &'a [crate::session::SetupData],
    /// The current rest-against-fresh comparisons. Empty, and therefore
    /// not allocated, when the session holds none.
    rest_fresh:
        std::collections::BTreeMap<ToolpathId, crate::compute::toolpath_stats::FreshTwinTime>,
}

/// One toolpath row as the triage reads it.
pub(crate) struct TriageRow<'a> {
    id: ToolpathId,
    name: &'a str,
    stock_source: crate::session::StockSource,
    /// The bound tool, `None` for a missing tool id.
    tool: Option<&'a crate::compute::tool_config::ToolConfig>,
    operation: &'a crate::compute::catalog::OperationConfig,
}

impl<'a> TriageView<'a> {
    /// The view of `session` as it is now. It allocates only when the
    /// session holds a current rest-against-fresh comparison.
    pub(crate) fn of(session: &'a ProjectSession) -> Self {
        Self {
            toolpaths: &session.toolpath_configs,
            tools: &session.tools,
            stock: &session.stock,
            setups: &session.setups,
            rest_fresh: session.current_rest_fresh_comparisons(),
        }
    }

    /// Each toolpath row, in session order.
    fn rows(&self) -> impl Iterator<Item = TriageRow<'a>> + '_ {
        let tools = self.tools;
        self.toolpaths.iter().map(move |tc| {
            // No `..`: a new `ToolpathConfig` field is a compile error here
            // until someone decides whether the triage reads it.
            let crate::session::ToolpathConfig {
                id,
                name,
                operation,
                tool_id,
                stock_source,
                // In the load-report key (`RowKey::enabled`). The
                // diagnostics pass reads it there.
                enabled: _,
                // Read by the generation, not by the triage. A field that
                // moves motion gives a new result `Arc`, which the
                // load-report key holds.
                dressups: _,
                heights: _,
                model_id: _,
                boundary: _,
                boundary_inherit: _,
                rest_analysis: _,
                // Export and display fields: the triage reads none.
                pre_gcode: _,
                post_gcode: _,
                coolant: _,
                debug_options: _,
                feeds_provenance: _,
                planner_origin: _,
                shoe: _,
            } = tc;
            TriageRow {
                id: *id,
                name,
                stock_source: *stock_source,
                tool: tools.iter().find(|t| t.id == ToolId(*tool_id)),
                operation,
            }
        })
    }

    /// The key of this view: the parts the load-report key does not hold.
    fn key(&self) -> TriageSessionKey {
        let Self {
            toolpaths: _,
            // In the load-report key, by value.
            tools: _,
            stock,
            setups,
            rest_fresh,
        } = self;
        TriageSessionKey {
            rows: self.rows().map(|row| row.key()).collect(),
            rest_fresh: rest_fresh.clone(),
            stock: (*stock).clone(),
            faces: setups.iter().map(|setup| setup.face_up).collect(),
        }
    }
}

impl TriageRow<'_> {
    /// The key part of this row.
    fn key(&self) -> (ToolpathId, String, crate::session::StockSource) {
        let TriageRow {
            id,
            name,
            stock_source,
            // In the load-report key: `RowKey::{tool_id, operation}` and
            // the tool library by value.
            tool: _,
            operation: _,
        } = self;
        (*id, (*name).to_owned(), *stock_source)
    }

    /// Does `key` describe this row? It allocates nothing.
    fn answers(&self, key: &(ToolpathId, String, crate::session::StockSource)) -> bool {
        let (id, name, source) = key;
        *id == self.id && name == self.name && *source == self.stock_source
    }
}

/// The session parts the simulation triage reads that the load-report key
/// does not hold.
///
/// A cache of the triage keys on three parts, and on no counter:
///
/// 1. the load report it read ([`crate::session::LoadReportStamp`]): the
///    rows, the results, the tools, the material, the post, the machine
///    and the trace;
/// 2. this key: each row's name and stock source, the current
///    rest-against-fresh comparisons, the stock (its pins give the
///    alignment-pin findings) and each setup's face;
/// 3. the evidence the caller gives (collisions, boundaries, resolution).
///
/// The key derives from [`TriageView`], the one view the triage reads the
/// session through (F187).
///
/// A result's `stats` travel with its annotated toolpath. Each write of a
/// result gives a new `Arc`, and the report key compares that identity.
/// `self.simulation` decides a reason only when the caller has no trace,
/// and with no trace the triage is empty.
#[derive(Debug, Clone, PartialEq)]
pub struct TriageSessionKey {
    rows: Vec<(ToolpathId, String, crate::session::StockSource)>,
    /// The current rest-against-fresh comparisons: a new one adds a
    /// finding.
    rest_fresh:
        std::collections::BTreeMap<ToolpathId, crate::compute::toolpath_stats::FreshTwinTime>,
    stock: crate::compute::stock_config::StockConfig,
    faces: Vec<crate::compute::transform::FaceUp>,
}

impl TriageSessionKey {
    /// The key for `session` as it is now.
    pub fn of(session: &ProjectSession) -> Self {
        TriageView::of(session).key()
    }

    /// Does this key describe `session` as it is now? It allocates only
    /// when the session holds a rest-against-fresh comparison.
    pub fn matches(&self, session: &ProjectSession) -> bool {
        self.answers(&TriageView::of(session))
    }

    /// Does this key describe `view`? The same parts as
    /// [`TriageView::key`], compared in place.
    fn answers(&self, view: &TriageView<'_>) -> bool {
        let Self {
            rows,
            rest_fresh,
            stock,
            faces,
        } = self;
        rows.len() == view.toolpaths.len()
            && rows
                .iter()
                .zip(view.rows())
                .all(|(key, row)| row.answers(key))
            && stock == view.stock
            && *rest_fresh == view.rest_fresh
            && faces.len() == view.setups.len()
            && faces
                .iter()
                .zip(view.setups)
                .all(|(face, setup)| *face == setup.face_up)
    }
}

/// The one triage assembly behind
/// [`ProjectSession::simulation_triage_with_diagnostics`] and
/// [`ProjectSession::simulation_triage_with_report`]. The two differ only in
/// where the kinematic readings come from.
///
/// F187: it reads the session only through `view`, and
/// [`TriageSessionKey`] derives from the same view, so a part read here is
/// in that key or in the load-report key by construction.
/// [`ProjectSession::diagnostics_with_evidence`] still reads the session
/// itself; `project_diagnostics` is its answer.
fn triage_from_parts(
    view: &TriageView<'_>,
    evidence: &ProjectEvidence<'_>,
    trace: &crate::stock::simulation_cut::SimulationCutTrace,
    project_diagnostics: &ProjectDiagnostics,
    kinematic_utilization: &std::collections::BTreeMap<
        ToolpathId,
        &crate::machine::kinematic_utilization::ToolpathKinematicUtilization,
    >,
) -> crate::stock::sim_triage::SimulationTriage {
    use crate::stock::sim_triage::{SimulationTriage, TriageInputs, commanded_step_mm};

    let diagnostics =
        crate::diagnostics::adapters::from_project_diagnostics::diagnostics_from_project(
            project_diagnostics,
        );
    let measurability = crate::stock::sim_measurability::MeasurabilityReport::from_trace(
        trace,
        evidence.resolution_mm,
    );
    let tool_diameters_mm = view
        .rows()
        .filter_map(|row| {
            let tool_cfg = row.tool?;
            // F029: no diameter for a tool outside its domain.
            let cutter = crate::compute::cutter::build_cutter(tool_cfg).ok()?;
            Some((row.id, crate::tool::MillingCutter::diameter(&cutter)))
        })
        .collect();

    // G-OVERCUTREF: the commanded step of each stepped toolpath, the
    // reference of the standing-material rule. It reads each row's
    // operation, and the load-report key holds the operation bytes
    // (`RowKey::operation`), so an edit of the depth per pass makes a
    // new triage. `TriageSessionKey` needs no new part.
    let commanded_depth_per_pass_mm: std::collections::BTreeMap<ToolpathId, f64> = view
        .rows()
        .filter_map(|row| commanded_step_mm(row.operation).map(|step| (row.id, step)))
        .collect();

    // G-ENTRYLOAD scope: the same predicate that arms the pencil entry
    // ramp (`gen_initial_stock`) — entries are graded only where they
    // descend through rest material the entry planner had no stock
    // reading for. Fresh-stock entry plunges are planned motion.
    let rest_driven: std::collections::BTreeSet<crate::ids::ToolpathId> = view
        .rows()
        .filter(|row| row.stock_source == crate::session::StockSource::FromRemainingStock)
        .map(|row| row.id)
        .collect();

    // G-LINKPLOUGH scope: a finishing op that is not rest-driven builds
    // no link ceiling (`LinkCeilingNeed::WithRemainingStock`), so its
    // links ride the mesh through the stock its passes cut. The link
    // rule does not grade them.
    let links_on_mesh: std::collections::BTreeSet<crate::ids::ToolpathId> = view
        .rows()
        .filter(|row| {
            row.operation.capabilities().link_ceiling
                == crate::compute::execute::LinkCeilingNeed::WithRemainingStock
                && !rest_driven.contains(&row.id)
        })
        .map(|row| row.id)
        .collect();

    // Phase 4 — the plunge-class backstop's population
    // (`kinematic_utilization`): every enabled toolpath with a result,
    // drill ops included. The finding itself is NOT gated on
    // `rest_driven`, because an untagged vertical descent is a defect on
    // fresh stock exactly as it is on rest stock.

    // G-RAPIDFRAME: the triage names the toolpath and the local move of
    // each rapid collision from the ONE attribution, not from the bare
    // local index.
    let rapid_collisions = evidence.attributed_rapid_collisions();
    let toolpath_names: std::collections::BTreeMap<crate::ids::ToolpathId, String> = view
        .rows()
        .map(|row| (row.id, row.name.to_owned()))
        .collect();

    // Operator ruling 2026-10-15: the rest-against-fresh comparisons a
    // surface asked for, while they are current.
    let fresh_twin = &view.rest_fresh;

    SimulationTriage::build_with_rest_context(
        &TriageInputs {
            trace,
            measurability: &measurability,
            diagnostics: &diagnostics,
            rapid_collisions: &rapid_collisions,
            toolpath_names: &toolpath_names,
            holder_collisions: &evidence.holder_collisions,
            obstacle_collisions: evidence.obstacle_collisions,
            tool_diameters_mm: &tool_diameters_mm,
            commanded_depth_per_pass_mm: &commanded_depth_per_pass_mm,
            kinematic_utilization,
            region_of: None,
        },
        &rest_driven,
        &links_on_mesh,
        fresh_twin,
    )
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
            .and_then(|m| m.mesh.as_deref());
        let obstacles = self.obstacle_set_for_toolpath(index)?;
        // CMP-14 follow-up: the two halves of the check need different
        // things. The mesh half needs a mesh; the obstacle half needs only
        // the obstacles. Only a toolpath with NEITHER a mesh NOR an
        // enabled obstacle has nothing to answer, and that is the 2D case
        // the CLI expects.
        // H1: a head shelf or a fitted shoe reads the stock box, so the
        // check then applies with no mesh and no obstacle too.
        let head = self.head_fit_for_toolpath(index);
        let head_reads_stock = head.head.bands(0.0, head.shoe).iter().any(|band| {
            band.radius_mm.is_none()
                || band.segment == crate::stock::collision::AssemblySegment::Shoe
        });
        if model.is_none() && obstacles.is_empty() && !head_reads_stock {
            return Err(SessionError::MissingGeometry(
                "Collision check requires a 3D mesh or an enabled obstacle".to_owned(),
            ));
        }

        let tool = self
            .find_tool_by_raw_id(tc.tool_id)
            .ok_or(SessionError::ToolNotFound(ToolId(tc.tool_id)))?;
        let tool_def = crate::session::session_cutter(tool)?;

        let request = CollisionCheckRequest {
            head,
            stock: Some(self.setup_stock_box_for_toolpath(index)),
            toolpath: result.toolpath(),
            tool: tool_def,
            mesh: model,
            obstacles,
            join: self.obstacle_join_for_toolpath(index),
            index: prebuilt_index,
        };
        let check_result = run_collision_check(&request, cancel)?;
        Ok(check_result)
    }

    /// K1: the obstacles of a toolpath's setup, resolved into the
    /// setup frame (the frame the toolpath emits in). The one door the
    /// core collision check and the GUI worker read, so both flag the
    /// same hits. Empty when the toolpath has no setup or no enabled
    /// obstacle.
    ///
    /// # Errors
    /// [`SessionError::OperationFailed`] when an enabled obstacle cannot be
    /// resolved (a missing model). The text names the obstacle.
    pub fn obstacle_set_for_toolpath(
        &self,
        index: usize,
    ) -> Result<std::sync::Arc<crate::compute::obstacle::ObstacleSet>, SessionError> {
        let Some(setup) = self.find_setup_for_toolpath_index(index) else {
            return Ok(std::sync::Arc::default());
        };
        self.obstacle_set_for_setup(setup).map(std::sync::Arc::new)
    }

    /// K1: where the tool is before the first move of toolpath `index`,
    /// for the obstacle check of the join
    /// ([`crate::compute::obstacle::JoinStart::after_previous`]). It reads
    /// the toolpaths that run before `index` in its setup: enabled, with a
    /// result, in setup order, the entries the simulation group holds.
    /// The GUI collision check and `collision_check` both call it.
    #[must_use]
    pub fn obstacle_join_for_toolpath(&self, index: usize) -> crate::compute::obstacle::JoinStart {
        use crate::compute::obstacle::JoinStart;
        let (Some(setup), Some(tc)) = (
            self.find_setup_for_toolpath_index(index),
            self.toolpath_configs.get(index),
        ) else {
            return JoinStart::Unknown;
        };
        let before = setup
            .toolpath_indices
            .iter()
            .take_while(|&&i| i != index)
            .filter_map(|&i| {
                let prev = self.toolpath_configs.get(i)?;
                let result = self.results.get(&i)?;
                prev.enabled
                    .then(|| (ToolId(prev.tool_id), result.toolpath()))
            });
        JoinStart::after_previous(before.collect::<Vec<_>>().into_iter(), &ToolId(tc.tool_id))
    }

    /// K1: the obstacles of `setup`, resolved into its frame.
    ///
    /// # Errors
    /// [`SessionError::OperationFailed`] when an enabled obstacle cannot be
    /// resolved.
    pub fn obstacle_set_for_setup(
        &self,
        setup: &crate::session::SetupData,
    ) -> Result<crate::compute::obstacle::ObstacleSet, SessionError> {
        crate::compute::obstacle::ObstacleSet::resolve(&setup.obstacles, &self.models)
            .map_err(|e| SessionError::OperationFailed(e.to_string()))
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
        let tool_def = crate::session::session_cutter(tool)?;
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
        // W2.5 (D10): this narration reads the stored result, the one
        // export emits.
        context.feed_stage = Some(self.feed_stage_of(index, &result.provenance));

        Ok(crate::trace::narrate::narrate_toolpath_with_context(
            result.annotated(),
            result.semantic_trace.as_deref(),
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
    /// A toolpath whose model carries no mesh contributes no entry. The
    /// sweep still runs the fixture half for it, and reads
    /// [`HolderCollisionCheck::NotApplicable`] only when the setup has no
    /// enabled fixture either.
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
                        // K1: an obstacle hit is counted from the
                        // simulation (`SimulationResult::obstacle_collisions`),
                        // so this sweep counts the workpiece hits alone.
                        Ok(r) => HolderCollisionCheck::Measured(
                            r.collision_report
                                .collisions
                                .iter()
                                .filter(|c| {
                                    c.kind == crate::stock::collision::CollisionKind::Workpiece
                                })
                                .count(),
                        ),
                        // The 2D case the CLI calls expected: no mesh AND
                        // no enabled fixture, so nothing to check against.
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
        let Some(trace) = evidence.cut_trace else {
            return crate::stock::sim_triage::SimulationTriage::default();
        };
        // The caller's own trace decides the readings' feeds provenance
        // (Phase 3), so it is the one to pass — not `self.simulation`,
        // which may hold a different run, or none.
        let owned = self.kinematic_utilizations(Some(trace));
        let kinematics = owned.iter().map(|(id, util)| (*id, util)).collect();
        triage_from_parts(
            &TriageView::of(self),
            evidence,
            trace,
            project_diagnostics,
            &kinematics,
        )
    }

    /// [`Self::simulation_triage`] that takes the kinematic readings from
    /// `report` and does not analyse the moves again.
    ///
    /// The kinematic reading walks every move of every toolpath. The load
    /// report holds the same reading per row
    /// ([`crate::tool_load::ToolpathLoadVerdict::kinematic_utilization`]),
    /// so a caller that holds the report gives it here. The numbers are the
    /// same: the trace decides only the reading's feeds provenance, and the
    /// triage does not read the provenance.
    ///
    /// `report` MUST be [`Self::tool_load_report_for`] on this session as it
    /// is now. The report leaves out an enabled row whose tool is missing;
    /// this function reads that row from the session, so the answer equals
    /// [`Self::simulation_triage`] for each row.
    pub fn simulation_triage_with_report(
        &self,
        evidence: &ProjectEvidence<'_>,
        report: &crate::tool_load::ToolLoadReport,
    ) -> crate::stock::sim_triage::SimulationTriage {
        let Some(trace) = evidence.cut_trace else {
            return crate::stock::sim_triage::SimulationTriage::default();
        };
        let project_diagnostics = self.diagnostics_with_evidence(evidence);
        self.triage_with_report_from_parts(evidence, trace, report, &project_diagnostics)
    }

    /// [`Self::simulation_triage_with_report`] plus the project verdict of
    /// the same diagnostics build (F113). The GUI simulation header caches
    /// both on one key, so it builds the project diagnostics once.
    ///
    /// With no cut trace the triage is the empty default, and the verdict
    /// still reads the collision evidence.
    pub fn verdict_and_triage_with_report(
        &self,
        evidence: &ProjectEvidence<'_>,
        report: &crate::tool_load::ToolLoadReport,
    ) -> (
        crate::session::ProjectVerdict,
        crate::stock::sim_triage::SimulationTriage,
    ) {
        let project_diagnostics = self.diagnostics_with_evidence(evidence);
        let triage = match evidence.cut_trace {
            Some(trace) => {
                self.triage_with_report_from_parts(evidence, trace, report, &project_diagnostics)
            }
            None => crate::stock::sim_triage::SimulationTriage::default(),
        };
        (project_diagnostics.verdict(), triage)
    }

    /// The kinematic readings of `report`, plus the session's own reading
    /// of each enabled row the report leaves out, then the triage.
    fn triage_with_report_from_parts(
        &self,
        evidence: &ProjectEvidence<'_>,
        trace: &crate::stock::simulation_cut::SimulationCutTrace,
        report: &crate::tool_load::ToolLoadReport,
        project_diagnostics: &ProjectDiagnostics,
    ) -> crate::stock::sim_triage::SimulationTriage {
        let in_report: std::collections::BTreeSet<ToolpathId> = report
            .per_toolpath
            .iter()
            .map(|verdict| verdict.toolpath_id)
            .collect();
        let missing: Vec<_> = self
            .toolpath_configs
            .iter()
            .enumerate()
            .filter(|(_, tc)| tc.enabled && !in_report.contains(&tc.id))
            .filter_map(|(index, _)| self.kinematic_utilization_for(index, Some(trace)))
            .collect();
        let kinematics = report
            .per_toolpath
            .iter()
            .filter_map(|verdict| {
                let util = verdict.kinematic_utilization.as_ref()?;
                Some((util.toolpath_id, util))
            })
            .chain(missing.iter().map(|util| (util.toolpath_id, util)))
            .collect();
        triage_from_parts(
            &TriageView::of(self),
            evidence,
            trace,
            project_diagnostics,
            &kinematics,
        )
    }

    pub fn diagnostics_with_evidence(&self, evidence: &ProjectEvidence<'_>) -> ProjectDiagnostics {
        let mut per_toolpath = Vec::new();
        let mut total_collision_count: usize = 0;
        let mut collision_checks_failed: usize = 0;

        // Per-TP context collected in the toolpath loop for later verdict
        // emission. We index by `toolpath_id` (which equals `tc.id`).
        let mut holder_collisions_by_tp: Vec<(ToolpathId, String, usize)> = Vec::new();
        let mut obstacle_collisions_by_tp: Vec<(ToolpathId, String, usize, String)> = Vec::new();
        let mut failed_checks_by_tp: Vec<(ToolpathId, String)> = Vec::new();
        let mut rapid_collisions_by_tp: Vec<(ToolpathId, String, RapidAttribution)> = Vec::new();
        let mut empty_results_by_tp: Vec<(ToolpathId, String, &str)> = Vec::new();

        // G-RAPIDFRAME: ONE attribution gives the count and the worst hit
        // of each toolpath. See `rapid_attribution_by_toolpath`.
        let attributed_rapid = evidence.attributed_rapid_collisions();
        let rapid_by_toolpath = rapid_attribution_by_toolpath(&attributed_rapid);
        // F113: the project total counts EVERY hit of the evidence. A hit
        // that no boundary holds, or that sits on a toolpath with no result,
        // is still a rapid through the stock; the per-toolpath sum dropped
        // it, and the MCP verdict (which counted every hit) disagreed.
        let total_rapid_collision_count = attributed_rapid.len();

        for (idx, tc) in self.toolpath_configs.iter().enumerate() {
            if let Some(result) = self.results.get(&idx) {
                // A toolpath can own more than one boundary (one per
                // setup group). The attribution keys by toolpath, so it
                // already merges them.
                let rapid = rapid_by_toolpath.get(&tc.id).copied().unwrap_or_default();
                let rapid_count = rapid.count;

                // The verdict depends on the count only. The worst hit only
                // adds detail to the headline, so a missing worst hit never
                // hides the safety finding.
                if rapid_count > 0 {
                    rapid_collisions_by_tp.push((tc.id, tc.name.clone(), rapid));
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
                // K1: the obstacle hits of the simulation are collisions
                // too. A measured obstacle count is published even when the
                // holder check did not apply: `None` only when the holder
                // check FAILED and no obstacle hit is known.
                let obstacle_hits = evidence
                    .obstacle_collisions
                    .iter()
                    .filter(|hit| hit.toolpath_id == tc.id)
                    .count();
                let holder_collision_count =
                    match holder_check.and_then(HolderCollisionCheck::count) {
                        Some(count) => Some(count + obstacle_hits),
                        None if obstacle_hits > 0 => Some(obstacle_hits),
                        None => None,
                    };
                if let Some(count) = holder_collision_count {
                    total_collision_count += count;
                }
                if obstacle_hits > 0 {
                    let names: std::collections::BTreeSet<&str> = evidence
                        .obstacle_collisions
                        .iter()
                        .filter(|hit| hit.toolpath_id == tc.id)
                        .filter_map(|hit| match &hit.event.kind {
                            crate::stock::collision::CollisionKind::Obstacle { name } => {
                                Some(name.as_str())
                            }
                            // H1: a shoe part or a shelf struck the stock box.
                            crate::stock::collision::CollisionKind::Stock => Some("stock"),
                            crate::stock::collision::CollisionKind::Workpiece => None,
                        })
                        .collect();
                    obstacle_collisions_by_tp.push((
                        tc.id,
                        tc.name.clone(),
                        obstacle_hits,
                        names.into_iter().collect::<Vec<_>>().join(", "),
                    ));
                }
                if holder_check.is_some_and(HolderCollisionCheck::failed) {
                    collision_checks_failed += 1;
                    failed_checks_by_tp.push((tc.id, tc.name.clone()));
                }

                if let Some(count) = holder_collision_count.map(|c| c - obstacle_hits)
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
                    .unwrap_or_else(|| SessionError::ToolNotFound(ToolId(tc.tool_id)).to_string());

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
                    source_stock: result.stats.source_stock.as_ref().map(|source| {
                        crate::compute::source_stock::SourceStockWire::of(
                            source,
                            result.stats.stock_snapshot.as_ref(),
                        )
                    }),
                });
            }
        }

        // Extract simulation metrics if available.
        //
        // LH-1: air cut is published under BOTH denominators, each named.
        // Every threshold in this file and in the GUI is tuned against the
        // total-runtime reading. The cutting-time reading (what the MCP
        // narration reports) travels beside it under its own name.
        //
        // U2 (2026-10-01): with no cut trace the four figures are `None`
        // (NOT MEASURED), not 0.0. A run with no trace published 0 s runtime
        // and 0 % air cut, which reads as a measured, clean run. The reason
        // travels beside them. A simulation exists when the session holds
        // one, or when the caller's evidence carries toolpath boundaries
        // (the GUI builds evidence from its own run).
        let trace_figures = evidence.cut_trace.map(|trace| {
            use crate::stock::simulation_cut::AirCutRatios;
            let summary = &trace.summary;
            (
                summary.total_runtime_s,
                summary.air_cut_pct_of_total_runtime(),
                summary.air_cut_pct_of_cutting_time(),
                summary.average_engagement,
            )
        });
        let total_runtime_s = trace_figures.map(|f| f.0);
        let air_cut_pct_of_total_runtime = trace_figures.map(|f| f.1);
        let air_cut_pct_of_cutting_time = trace_figures.map(|f| f.2);
        let average_engagement = trace_figures.map(|f| f.3);
        let cut_metrics_not_measured = if trace_figures.is_some() {
            None
        } else if self.simulation.is_some() || !evidence.boundaries.is_empty() {
            Some(CutMetricsNotMeasured::NoTrace)
        } else {
            Some(CutMetricsNotMeasured::NoSimulation)
        };

        // Per-TP op-kind-aware air-cut warnings. A blanket project-wide
        // threshold (the old `>40%`) fires falsely on projects that contain
        // even one sparse-pattern op like ProjectCurve, where 80–95% air-cut
        // is intrinsic. Per-TP thresholds live on `OperationType`
        // (see `Capabilities::air_cut_high_threshold_pct`).
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

        // Critical (K1): the tool enters an obstacle of its setup.
        for (id, name, count, obstacles) in &obstacle_collisions_by_tp {
            verdicts.push(Verdict {
                severity: VerdictSeverity::Critical,
                kind: VerdictKind::ObstacleCollision,
                headline: format!(
                    "ERROR: TP{id} '{name}' puts the tool inside obstacle(s) {obstacles} \
                     ({count} moves)"
                ),
                offender_toolpath_ids: vec![*id],
                fix_hint: "Regenerate the toolpath: generation keeps every move out of the \
                           obstacles of its setup. If the hits stay, check the obstacle \
                           geometry and frame, or move the obstacle."
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
        // `pin_findings_for_setups` judges each setup by its full map
        // (`face_up` and `z_rotation`) and dedupes by text: a `Top` setup
        // contributes only the bounds line, and a project with several
        // setups repeats it. Each finding carries its own fix hint.
        for finding in self
            .stock
            .pin_findings_for_setups(self.setups.iter().map(|s| (s.face_up, s.z_rotation)))
        {
            verdicts.push(Verdict {
                severity: VerdictSeverity::Important,
                kind: VerdictKind::AlignmentPinsUnkeyed,
                headline: finding.text,
                offender_toolpath_ids: Vec::new(),
                fix_hint: finding.kind.fix_hint().to_owned(),
                evidence: VerdictEvidence {
                    move_index: None,
                    z_value: None,
                    count: None,
                },
            });
        }

        // Important: the dowel holes have two sources (P8, C6). The stock
        // pins give the pin-drill holes; a drill or pocket operation on a
        // drawing of the holes is a second copy. The judgement is
        // `hole_source_findings`; this loop only publishes it.
        let (pin_drill_sizes, hole_ops) = self.dowel_hole_ops();
        for finding in crate::compute::alignment_pins::hole_source_findings(
            &self.stock.alignment_pins,
            &pin_drill_sizes,
            &hole_ops,
        ) {
            let kind = match finding.kind {
                crate::compute::alignment_pins::HoleSourceKind::DrilledTwice => {
                    VerdictKind::PinHolesDrilledTwice
                }
                crate::compute::alignment_pins::HoleSourceKind::DowelHolesDisagree => {
                    VerdictKind::DowelHolesDisagree
                }
            };
            verdicts.push(Verdict {
                severity: VerdictSeverity::Important,
                kind,
                headline: finding.text,
                offender_toolpath_ids: vec![ToolpathId(finding.toolpath_id)],
                fix_hint: finding.kind.fix_hint().to_owned(),
                evidence: VerdictEvidence {
                    move_index: None,
                    z_value: None,
                    count: None,
                },
            });
        }

        // Important: a pin wall thinner than `PIN_WALL_MM` (to the stock
        // edge, the default part models, a keep-out zone or a fixture), and
        // a pin that differs from the tool that drills the holes (P6). The
        // judgement is `pin_placement_findings`; this loop only publishes it.
        for finding in self.pin_placement_findings() {
            let kind = match finding.kind {
                crate::compute::alignment_pins::PinFindingKind::PinToolMismatch => {
                    VerdictKind::PinToolMismatch
                }
                // K1: a wall to an unresolved obstacle is not measured, so
                // it can be thin. The headline names the obstacle.
                crate::compute::alignment_pins::PinFindingKind::ObstacleUnresolved
                | crate::compute::alignment_pins::PinFindingKind::OutOfBounds
                | crate::compute::alignment_pins::PinFindingKind::NotModelled
                | crate::compute::alignment_pins::PinFindingKind::DoesNotSeat
                | crate::compute::alignment_pins::PinFindingKind::NotKeyed
                | crate::compute::alignment_pins::PinFindingKind::ThinWall => {
                    VerdictKind::PinWallThin
                }
            };
            verdicts.push(Verdict {
                severity: VerdictSeverity::Important,
                kind,
                headline: finding.text,
                offender_toolpath_ids: Vec::new(),
                fix_hint: finding.kind.fix_hint().to_owned(),
                evidence: VerdictEvidence {
                    move_index: None,
                    z_value: None,
                    count: None,
                },
            });
        }

        // Important (Caution): a stated shoe gap below the solver gap (H1).
        // The check reads the stated gap, so the simulation reports the
        // strike; this names the op and the gap it needs.
        if self.machine.head.shoe.is_some() {
            match self.shoe_plan() {
                Ok(rows) => {
                    verdicts.extend(
                        rows.iter()
                            .filter_map(crate::session::ShoePlanRow::gap_too_low_verdict),
                    );
                }
                Err(e) => tracing::warn!(error = %e, "the shoe plan failed; no shoe verdict"),
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

        // Critical: rapid-through-stock collisions per TP, with the deepest
        // hit as the representative move.
        //
        // The headline gives the move in BOTH frames: the toolpath's OWN
        // move index (MCP `inspect_collisions` `local_move`) and the run
        // move index (`global_move`, the simulation timeline).
        for (id, name, rapid) in &rapid_collisions_by_tp {
            let count = rapid.count;
            let worst = rapid.worst;
            let detail = match worst {
                Some(worst) => format!(
                    ", deepest at local move {local} (run move {global}), z={z:.3}",
                    local = worst.local_move,
                    global = worst.global_move,
                    z = worst.z,
                ),
                None => String::new(),
            };
            verdicts.push(Verdict {
                severity: VerdictSeverity::Critical,
                kind: VerdictKind::RapidCollision,
                headline: format!(
                    "WARNING: rapid collisions on TP{id} '{name}' ({count} collisions{detail})"
                ),
                offender_toolpath_ids: vec![*id],
                fix_hint: RAPID_COLLISION_HINT.to_owned(),
                evidence: VerdictEvidence {
                    move_index: worst.map(|worst| worst.local_move),
                    z_value: worst.map(|worst| worst.z),
                    count: Some(count),
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
            cut_metrics_not_measured,
            collision_count: total_collision_count,
            collision_checks_failed,
            rapid_collision_count: total_rapid_collision_count,
            per_toolpath,
            verdicts,
        }
    }

    // ── Export ──────────────────────────────────────────────────────
}

/// The fix hint of a rapid-collision verdict.
///
/// The simulation measures THAT a rapid (G0) move passes through stock. It
/// does not measure why. A live case (rivmap350 '3D Rough 8', 2026-10-01)
/// lifted to the safe Z and then made a G0 descent into uncut stock near a
/// containment boundary, so a "raise the retract" hint was wrong advice.
/// This text states what was measured and gives the shapes neutrally.
pub(crate) const RAPID_COLLISION_HINT: &str = "Measured: a rapid (G0) move passes through stock that no earlier move removed. \
     The simulation does not record the cause. Examine the moves around each listed \
     move: MCP inspect_collisions gives each move with its toolpath and its local and \
     run move index. The rapid can travel below the stock top, or it can descend from \
     the retract height into stock that is not cut yet.";

/// The rapid-collision evidence of one toolpath: the count and the deepest
/// hit.
#[derive(Debug, Clone, Copy, Default)]
struct RapidAttribution {
    count: usize,
    worst: Option<RapidWorst>,
}

/// The deepest rapid collision of one toolpath (lowest `end.z`).
#[derive(Debug, Clone, Copy)]
struct RapidWorst {
    /// The toolpath's own move index.
    local_move: usize,
    /// The run-global move index.
    global_move: usize,
    z: f64,
}

/// G-RAPIDFRAME: the count and the deepest hit of each toolpath, from the
/// ONE attribution ([`ProjectEvidence::attributed_rapid_collisions`]).
///
/// The simulator writes two parallel lists
/// (`compute::simulate::collect_rapid_hits`, `record_pre_carve`):
/// - `RapidCollision::move_index` is the toolpath's OWN (local) move index;
/// - `rapid_collision_move_indices` holds the run-global index,
///   `start_move + local`, at the same position.
///
/// Only the global index can find the toolpath. The attribution compares it
/// with the boundaries through `compute::simulate::locate_global_move`, the
/// same function MCP `inspect_collisions` and the GUI use, so no two surfaces
/// can attribute one collision to two toolpaths.
///
/// Before G-RAPIDFRAME, the worst hit compared the LOCAL index with the
/// GLOBAL range. Only a toolpath at `start_move == 0` matched. Every other
/// toolpath got a count and no verdict.
fn rapid_attribution_by_toolpath(
    attributed: &[crate::stock::collision::AttributedRapidCollision],
) -> std::collections::BTreeMap<ToolpathId, RapidAttribution> {
    let mut out: std::collections::BTreeMap<ToolpathId, RapidAttribution> =
        std::collections::BTreeMap::new();
    for hit in attributed {
        let Some((toolpath_id, local_move)) = hit.toolpath else {
            continue;
        };
        let entry = out.entry(toolpath_id).or_default();
        entry.count += 1;
        if let Some(rc) = hit.collision.as_ref()
            && entry.worst.is_none_or(|worst| rc.end.z < worst.z)
        {
            entry.worst = Some(RapidWorst {
                local_move,
                global_move: hit.global_move,
                z: rc.end.z,
            });
        }
    }
    out
}

impl ProjectSession {
    /// The inputs of the hole-source check (P8): whether an enabled
    /// pin-drill operation exists, and the holes of every other enabled
    /// operation that drills holes, in the stock-local frame.
    ///
    /// - A `Drill` operation drills the holes of its selection
    ///   ([`Self::hole_selection`], the resolution generation reads). A
    ///   selection that refuses drills nothing, so it is not judged.
    /// - A `Pocket` operation clears each circle of its selection.
    ///
    /// The holes are in the MODEL frame. On a `Top` or `Bottom` setup a
    /// drawing maps to the same physical place on the part
    /// (`SetupTransformInfo::drawing_to_local`), so the stock-local
    /// position is the hole minus the stock origin. A lateral setup reads
    /// the drawing in its own work plane, which is not the pin plane, so
    /// its operations are not judged.
    ///
    /// The first value is the pin diameter of each enabled pin-drill
    /// operation (R1).
    pub(crate) fn dowel_hole_ops(&self) -> (Vec<f64>, Vec<crate::compute::alignment_pins::HoleOp>) {
        use crate::compute::alignment_pins::{DrilledHole, HoleOp, HoleOpKind};
        use crate::compute::catalog::OperationConfig;

        let (ox, oy) = (self.stock.origin_x, self.stock.origin_y);
        let mut pin_drill_sizes = Vec::new();
        let mut ops = Vec::new();
        for (index, tc) in self.toolpath_configs.iter().enumerate() {
            if !tc.enabled {
                continue;
            }
            if let OperationConfig::AlignmentPinDrill(cfg) = &tc.operation {
                pin_drill_sizes.push(cfg.diameter);
                continue;
            }
            let lateral = self
                .setups
                .iter()
                .find(|s| s.toolpath_indices.contains(&index))
                .is_some_and(|s| s.face_up.is_lateral());
            if lateral {
                continue;
            }
            let hole = |xy: crate::geo::P2, diameter: Option<f64>| DrilledHole {
                x: xy.x - ox,
                y: xy.y - oy,
                diameter,
            };
            let (kind, holes): (HoleOpKind, Vec<DrilledHole>) = match &tc.operation {
                OperationConfig::Drill(_) => {
                    let Some(selection) = self.hole_selection(tc) else {
                        continue;
                    };
                    if selection.refusal.is_some() {
                        continue;
                    }
                    let holes = selection
                        .holes
                        .iter()
                        .map(|h| hole(h.xy, h.diameter))
                        .collect();
                    (HoleOpKind::Drill, holes)
                }
                OperationConfig::Pocket(_) => {
                    let Ok(resolved) = self.resolve_toolpath_selection(tc) else {
                        continue;
                    };
                    let holes: Vec<DrilledHole> = resolved
                        .items()
                        .iter()
                        .filter_map(|r| match r.item().shape() {
                            crate::selection::Shape::Circle {
                                centre, diameter, ..
                            } => Some(hole(*centre, Some(*diameter))),
                            crate::selection::Shape::Loop(_)
                            | crate::selection::Shape::Path(_)
                            | crate::selection::Shape::Point(_)
                            | crate::selection::Shape::Face(_) => None,
                        })
                        .collect();
                    if holes.is_empty() {
                        continue;
                    }
                    (HoleOpKind::Pocket, holes)
                }
                _ => continue,
            };
            ops.push(HoleOp {
                toolpath_id: tc.id.0,
                name: tc.name.clone(),
                kind,
                holes,
            });
        }
        (pin_drill_sizes, ops)
    }
}
