//! Controller test suites, by theme.
//!
//! **SHL-03.** This was one 6 956-line file spanning about eleven eras,
//! with twenty-two banner comments and no module of its own — the one
//! file in `controller/` where the directory's "one file per theme"
//! pattern had never been applied. The split is pure motion: every test
//! keeps its name and its body, and only its module path changed.
//!
//! What stays here is the fixture set every theme shares:
//! `ScriptedBackend` and the other scripted compute backends,
//! `sample_controller`, the push / edit / pump helpers, and the
//! freshness and undo fixtures. A child reaches them with
//! `use super::*;`.

mod crud;
mod drain_results;
mod freshness;
mod generate_all;
mod inspect_simulation;
mod mcp_cancel;
mod model_relink;
mod optimize;
mod planner;
mod post_mirror;
mod rest_dependency;
mod selection;
mod simulation_state;
mod smoke;
mod stock_frame;
mod undo;
mod view_simulation;
mod workspace;

use std::sync::Arc;

use std::time::{SystemTime, UNIX_EPOCH};

use egui::{CentralPanel, Context, Panel};

use rs_cam_core::geo::P3;

use rs_cam_core::mesh::make_test_flat;

use rs_cam_core::toolpath::Toolpath;

use super::*;

use crate::compute::{
    CollisionRequest, ComputeMessage, ComputeRequest, JobRequest, LaneState, OptimizeRequest,
    SimulationRequest, SimulationResult, ToolpathSubmitOutcome,
};

use crate::state::job::{SetupId, ToolConfig, ToolId, ToolType};

use crate::state::runtime::ToolpathRuntime;

use crate::state::selection::Selection;

use crate::state::toolpath::{Adaptive3dConfig, OperationConfig, ToolpathId, ToolpathResult};

use crate::ui_command::{NoArgs, SimJumpToMoveArgs, UiCommand};

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};

use rs_cam_core::session::{
    AddModelArgs, AddSetupArgs, AddToolpathArgs, AdoptResultArgs, Command,
    InvalidateToolpathInputsArgs, LoadedModel, ProjectSessionBuilder, ReplaceToolpathConfigArgs,
    ReplaceToolsArgs, SetBoundaryConfigArgs, SetFeedsProvenanceArgs, SetPostConfigArgs,
    SetSetupFaceArgs, SetSetupRotationArgs, SetStockConfigArgs, SetStockSourceArgs,
    SetToolpathEnabledArgs, ToolpathConfig,
};

struct ScriptedBackend {
    toolpath_lane: LaneSnapshot,
    analysis_lane: LaneSnapshot,
    optimize_lane: LaneSnapshot,
    reach_lane: LaneSnapshot,
    job_lane: LaneSnapshot,
    drained: Vec<ComputeMessage>,
    /// G-REGEN-RACE: the one piece of real lane state the supersede rule
    /// reads. Setting it stands for "the toolpath lane is currently
    /// running a job for this toolpath", which is the state
    /// `ThreadedComputeBackend::submit_toolpath` turns into
    /// `SupersededActive`. Modelled rather than stubbed so the controller
    /// under test makes the same decision the real lane makes.
    active_toolpath_id: Option<ToolpathId>,
    submitted: Vec<ToolpathId>,
    /// Optimize-lane submissions, kept WHOLE rather than counted. Only the
    /// project rollup rides this lane since WP14b.
    optimize_requests: Vec<OptimizeRequest>,
    /// `Job`-lane submissions, kept WHOLE for the same reason. The Phase U
    /// tier preview and the per-toolpath Optimize run both ride this lane
    /// since WP14b, and each handle carries what was actually asked for, so
    /// a test can witness the request rather than only that one happened.
    ///
    /// `ComputeBackend::submit_job` has an empty default body, so a backend
    /// that does not override it records nothing.
    job_requests: Vec<JobRequest>,
}

impl ScriptedBackend {
    fn new() -> Self {
        Self {
            toolpath_lane: LaneSnapshot::idle(ComputeLane::Toolpath),
            analysis_lane: LaneSnapshot::idle(ComputeLane::Analysis),
            optimize_lane: LaneSnapshot::idle(ComputeLane::Optimize),
            reach_lane: LaneSnapshot::idle(ComputeLane::Reach),
            job_lane: LaneSnapshot::idle(ComputeLane::Job),
            drained: Vec::new(),
            active_toolpath_id: None,
            submitted: Vec::new(),
            optimize_requests: Vec::new(),
            job_requests: Vec::new(),
        }
    }
}

impl ComputeBackend for ScriptedBackend {
    fn submit_toolpath(&mut self, request: ComputeRequest) -> ToolpathSubmitOutcome {
        let id = request.viz.toolpath_id;
        self.submitted.push(id);
        if self.active_toolpath_id == Some(id) {
            ToolpathSubmitOutcome::SupersededActive
        } else {
            ToolpathSubmitOutcome::Queued
        }
    }
    fn submit_simulation(&mut self, _request: SimulationRequest) {}
    fn submit_collision(&mut self, _request: CollisionRequest) {}
    fn submit_optimize(&mut self, request: OptimizeRequest) {
        self.optimize_requests.push(request);
    }
    fn submit_job(&mut self, request: JobRequest) {
        self.job_requests.push(request);
    }

    fn cancel_lane(&mut self, lane: ComputeLane) {
        match lane {
            ComputeLane::Toolpath => self.toolpath_lane.state = LaneState::Cancelling,
            ComputeLane::Analysis => self.analysis_lane.state = LaneState::Cancelling,
            ComputeLane::Optimize => self.optimize_lane.state = LaneState::Cancelling,
            ComputeLane::Reach => self.reach_lane.state = LaneState::Cancelling,
            ComputeLane::Job => self.job_lane.state = LaneState::Cancelling,
        }
    }

    fn drain_results(&mut self) -> Vec<ComputeMessage> {
        std::mem::take(&mut self.drained)
    }

    fn lane_snapshot(&self, lane: ComputeLane) -> LaneSnapshot {
        match lane {
            ComputeLane::Toolpath => self.toolpath_lane.clone(),
            ComputeLane::Analysis => self.analysis_lane.clone(),
            ComputeLane::Optimize => self.optimize_lane.clone(),
            ComputeLane::Reach => self.reach_lane.clone(),
            ComputeLane::Job => self.job_lane.clone(),
        }
    }

    fn generation_control(&self) -> crate::compute::GenerationControl {
        crate::compute::GenerationControl::detached()
    }
}

fn temp_path(name: &str, extension: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("rs_cam_{name}_{nanos}.{extension}"))
}

fn fixture_path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn sample_controller() -> AppController<ScriptedBackend> {
    let mut controller = AppController::with_backend(ScriptedBackend::new());
    sample_project_into(&mut controller);
    controller
}

/// The shared fixture body, generic over the backend so tests that need a
/// result-echoing double (A/M11's fixpoint chain) get the same project.
fn sample_project_into<B: ComputeBackend>(controller: &mut AppController<B>) {
    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    // Every caller hands in a controller straight from `with_backend`, so the
    // builder replaces an empty session.
    let mut builder = ProjectSessionBuilder::new().tool(tool);

    let mesh = Arc::new(make_test_flat(40.0));
    let _ = builder.add_model(LoadedModel {
        id: 0,
        path: std::path::PathBuf::from("flat.stl"),
        name: "Flat".to_owned(),
        kind: Some(ModelKind::Stl),
        mesh: Some(Arc::clone(&mesh)),
        polygons: None,
        drill_targets: std::sync::Arc::new(Vec::new()),
        layers: std::sync::Arc::new(Vec::new()),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    });

    let tp_config = ToolpathConfig {
        id: ToolpathId(0),
        name: "Scallop".to_owned(),
        enabled: true,
        operation: OperationConfig::Scallop(
            rs_cam_core::compute::operation_configs::ScallopConfig::default(),
        ),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: 1,
        model_id: 0,
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        face_selection: None,
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    };
    let _ = builder.add_toolpath(0, tp_config).unwrap();
    controller.state.session = builder.build();
    let tp_id = controller.state.session.toolpath_configs()[0].id;
    let mut rt = ToolpathRuntime::new(true);
    rt.result = Some(ToolpathResult {
        annotated: Arc::new(rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
            Toolpath::new(),
        )),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
        debug_trace_path: None,
        drill_op: None,
    });
    controller.state.gui.toolpath_rt.insert(tp_id, rt);

    controller.state.selection = Selection::Toolpath(tp_id);
}

/// The index of the toolpath carrying `id`.
///
/// The core setters take an index. A test that holds an id converts it here.
fn index_of<B: ComputeBackend>(controller: &AppController<B>, id: ToolpathId) -> usize {
    controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .position(|tc| tc.id == id)
        .expect("the toolpath id belongs to this session")
}

/// Append another toolpath to setup 0 of a `sample_controller()` project,
/// sharing its tool and model. Returns the new toolpath's id.
fn push_toolpath<B: ComputeBackend>(controller: &mut AppController<B>, name: &str) -> ToolpathId {
    let next = controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .map(|tc| tc.id.0 + 1)
        .max()
        .unwrap_or(0);
    let cfg = ToolpathConfig {
        id: ToolpathId(next),
        name: name.to_owned(),
        enabled: true,
        operation: OperationConfig::Scallop(
            rs_cam_core::compute::operation_configs::ScallopConfig::default(),
        ),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: 1,
        model_id: 0,
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        face_selection: None,
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    };
    let _ = controller
        .state
        .session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(cfg),
        }))
        .unwrap();
    ToolpathId(next)
}

fn render_snapshot(
    controller: &mut AppController<ScriptedBackend>,
) -> crate::ui::automation::UiAutomationSnapshot {
    let ctx = Context::default();
    // UI-06: the real font set, not `FontDefinitions::empty()`. The
    // inspector reaches a weight through a NAMED family
    // (`tokens::FAMILY_SEMIBOLD`), and epaint panics on a named family that
    // no font is bound to. With empty fonts this harness could render only
    // the tabs that never ask for a weight.
    crate::ui::tokens::apply_fonts(&ctx);
    // epaint 0.36 asserts in `Drop for TexturesDelta` that the deltas were
    // either applied or cleared on purpose. eframe applies them in the real
    // app; this harness has no GPU and never uploads a texture, so it clears
    // them, which is what the assert's own message prescribes.
    let mut output = ctx.run_ui(Default::default(), |ui| {
        crate::ui::automation::begin_frame(ui.ctx());

        Panel::right("properties").show(ui, |ui| {
            let events = &mut controller.events;
            crate::ui::properties::draw(ui, &mut controller.state, events);
        });

        Panel::bottom("status_bar").show(ui, |ui| {
            let lanes = controller.lane_snapshots();
            let collision_count = controller.collision_positions.len();
            crate::ui::status_bar::draw(
                ui,
                &controller.state,
                collision_count,
                &lanes,
                controller.load_warnings(),
            );
        });

        CentralPanel::default().show(ui, |ui| {
            let lanes = controller.lane_snapshots();
            let events = &mut controller.events;
            crate::ui::viewport_overlay::draw(
                ui,
                &mut controller.state,
                crate::render::camera::ProjectionMode::Perspective,
                &lanes,
                events,
            );
        });

        // DC7 deleted the "Project Load Warnings" window this harness used to
        // mirror. The warnings reach the operator as a count on the status
        // bar, which the panel above already draws.
    });
    output.textures_delta.clear();
    crate::ui::automation::snapshot(&ctx)
}

/// Helper: inject minimal simulation results into the controller.
fn inject_sim_results(controller: &mut AppController<ScriptedBackend>, num_setups: usize) {
    use rs_cam_core::stock::stock_mesh::StockMesh;

    let mesh = StockMesh {
        vertices: vec![0.0; 9],
        indices: vec![0, 1, 2],
        colors: vec![0.5; 9],
    };

    let total_moves = 10 * num_setups;
    let mut boundaries = Vec::new();
    for i in 0..num_setups {
        boundaries.push(crate::compute::worker::SimBoundary {
            id: ToolpathId(0),
            name: format!("Op {}", i + 1),
            tool_name: "EndMill".to_owned(),
            start_move: i * 10,
            end_move: (i + 1) * 10,
            direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
        });
    }

    controller
        .compute
        .drained
        .push(ComputeMessage::Simulation(Ok(Box::new(SimulationResult {
            core: rs_cam_core::compute::simulate::SimulationResult {
                mesh,
                total_moves,
                deviations: None,
                column_deviations: None,
                boundaries,
                checkpoints: Vec::new(),
                rapid_collisions: Vec::new(),
                rapid_collision_move_indices: Vec::new(),
                cut_trace: None,
                column_grid_cell_mm: 0.5,
                resolution_clamped: false,
                prior_stocks: std::collections::HashMap::new(),
            },
            playback_data: Vec::new(),
            cut_trace_path: None,
        }))));

    controller.drain_compute_results();
}

/// Push one successful completion for `ToolpathId(0)` onto the fake
/// lane's queue, the way `drain_compute_results_repopulates_session_results`
/// does.
fn push_toolpath_completion(
    controller: &mut AppController<ScriptedBackend>,
    revision: Option<u64>,
) {
    controller
        .compute
        .drained
        .push(ComputeMessage::Toolpath(Box::new(
            crate::compute::worker::ComputeResult {
                toolpath_id: ToolpathId(0),
                revision,
                result: Ok(ToolpathResult {
                    annotated: Arc::new(
                        rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(Toolpath::new()),
                    ),
                    stats: Default::default(),
                    debug_trace: None,
                    semantic_trace: None,
                    debug_trace_path: None,
                    drill_op: None,
                }),
                debug_trace: None,
                semantic_trace: None,
                debug_trace_path: None,
            },
        )));
}

// ---------------------------------------------------------------------------
// P2.2/P2.3 — DerivedRestRegions boundary dependent staleness (Fix 2).
// A toolpath whose enabled boundary is `DerivedRestRegions` referencing
// another toolpath's cached `rest_regions` must be marked stale whenever
// that source regenerates or is removed — otherwise its cached clip keeps
// reflecting regions that no longer match the source's latest state.
// ---------------------------------------------------------------------------

fn add_derived_rest_dependent(
    controller: &mut AppController<ScriptedBackend>,
    source_id: ToolpathId,
) -> ToolpathId {
    let dependent_config = ToolpathConfig {
        id: ToolpathId(0), // placeholder — add_toolpath assigns the real id
        name: "Rest scallop".to_owned(),
        enabled: true,
        operation: OperationConfig::Scallop(
            rs_cam_core::compute::operation_configs::ScallopConfig::default(),
        ),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: 1,
        model_id: 0,
        pre_gcode: None,
        post_gcode: None,
        boundary: crate::state::toolpath::BoundaryConfig {
            enabled: true,
            source: crate::state::toolpath::BoundarySource::DerivedRestRegions {
                source_toolpath_id: source_id,
            },
            containment: crate::state::toolpath::BoundaryContainment::Center,
            offset: 0.0,
        },
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        face_selection: None,
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    };
    controller
        .state
        .session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(dependent_config),
        }))
        .expect("dependent toolpath should be added to setup 0")
        .created
        .expect("the AddToolpath row reports the new toolpath index");
    let dependent_id = controller.state.session.toolpath_configs()[1].id;
    controller
        .state
        .gui
        .toolpath_rt
        .insert(dependent_id, ToolpathRuntime::new(true));
    dependent_id
}

// ---------------------------------------------------------------------------
// F-028 viz-path follow-up (2026-05-25)
//
// The original F-028 fix (commit `329c5cf`) only patched
// `session::compute::compute_simulation_groups` (site 1). Round-07 smoke
// verified through the MCP exposed that the viz / MCP / GUI pipeline takes
// a different path: `AppController::submit_toolpath_compute`
// (`controller/events/compute.rs`). That path always built the
// `HeightContext` from the **local zero-rooted** bbox even for identity
// setups, and ALSO transformed mesh/polygons by `-stock.origin` into a
// setup-local frame. The toolpath generator then anchored its depth
// stepping at `heights.top_z = stock.z = 12` (for AS001) and emitted cuts
// at Z=10, 8, 6 in the setup-local frame.
//
// The downstream simulation path, however, drops `local_to_global = None`
// for identity setups (F-024 follow-up `1dd1aa7`) — so the dexel grid was
// rebuilt in *world* frame. The generator's local-frame cuts at Z=10 sat
// 10 mm above the world stock top at Z=0; the cutter swept through air
// the whole run. Round-07 MCP smoke evidence:
//   AS001 pocket: z_level = 10/8/6, peak_axial_doc_mm = 0,
//   total_removed_volume_est_mm3 = 0, chipload = Unmodeled
//   (all_samples_air_cut_or_rapid), air cut = 96 % of total runtime, 0 rapid
//   collisions, 0 sampling errors.
//
// The fix mirrors `session::compute::compute` (site 1): identity setups
// (`face_up = Top`, `z_rotation = Deg0`) skip the geometry transform and
// build the `HeightContext` + worker `stock_bbox` from the world bbox.
// The `transform_setup = Some(...)` branch is now gated on
// `s.needs_transform()` — identity setups fall through to the
// `transform_setup = None` branch and emit cuts in world frame to match
// the downstream sim path's world-frame dexel grid.
//
// Test design: a `CapturingBackend` records the `ComputeRequest`
// submitted by `submit_toolpath_compute` so we can pin the heights frame
// directly. Pre-fix the captured `heights.top_z = 12` and
// `stock_bbox.max.z = 12` (local). Post-fix both are 0 (world stock top
// for AS001).
// ---------------------------------------------------------------------------

/// Capturing backend that records the most recent toolpath `ComputeRequest`.
/// Used by the F-028 viz-path follow-up regression test to assert that
/// `submit_toolpath_compute` resolves heights in the world frame for
/// identity setups.
#[derive(Default)]
struct CapturingBackend {
    captured: Option<ComputeRequest>,
}

impl crate::compute::ComputeBackend for CapturingBackend {
    fn submit_toolpath(&mut self, request: ComputeRequest) -> ToolpathSubmitOutcome {
        self.captured = Some(request);
        ToolpathSubmitOutcome::Queued
    }
    fn submit_simulation(&mut self, _request: SimulationRequest) {}
    fn submit_collision(&mut self, _request: CollisionRequest) {}
    fn submit_optimize(&mut self, _request: OptimizeRequest) {}
    fn cancel_lane(&mut self, _lane: crate::compute::ComputeLane) {}
    fn drain_results(&mut self) -> Vec<ComputeMessage> {
        Vec::new()
    }
    fn lane_snapshot(&self, lane: crate::compute::ComputeLane) -> crate::compute::LaneSnapshot {
        crate::compute::LaneSnapshot::idle(lane)
    }
    fn generation_control(&self) -> crate::compute::GenerationControl {
        crate::compute::GenerationControl::detached()
    }
}

// ── A/M11: generate_all as a fixpoint over the rest-stock chain ──────────

/// A backend that models the one rule that makes the ladder necessary:
/// prior stock for an operation appears only when a **simulation** runs
/// AFTER its predecessor has generated. Toolpath submits succeed
/// immediately; each simulation publishes a prior-stock snapshot for every
/// op whose immediate predecessor in `chain` has generated by then.
#[cfg(feature = "mcp")]
struct RestChainBackend {
    /// Toolpath ids in index order — the stock chain.
    chain: Vec<ToolpathId>,
    generated: std::collections::HashSet<ToolpathId>,
    drained: Vec<ComputeMessage>,
    pub simulations: usize,
    /// When set, this toolpath always fails to generate.
    poison: Option<ToolpathId>,
}

#[cfg(feature = "mcp")]
impl RestChainBackend {
    fn new() -> Self {
        Self {
            chain: Vec::new(),
            generated: std::collections::HashSet::new(),
            drained: Vec::new(),
            simulations: 0,
            poison: None,
        }
    }
}

#[cfg(feature = "mcp")]
impl ComputeBackend for RestChainBackend {
    fn submit_toolpath(&mut self, request: ComputeRequest) -> ToolpathSubmitOutcome {
        let id = request.viz.toolpath_id;
        // The real lane stamps the revision `start` read (`worker.rs`), so the
        // drain adopts at that revision. A fake that reports `None` adopts at
        // the CURRENT revision and accepts a result the operator has edited past.
        let revision = Some(request.handle.revision);
        let result = if self.poison == Some(id) {
            Err(crate::compute::ComputeError::Message(
                "synthetic hard failure".to_owned(),
            ))
        } else {
            self.generated.insert(id);
            Ok(ToolpathResult {
                annotated: Arc::new(rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
                    Toolpath::new(),
                )),
                stats: Default::default(),
                debug_trace: None,
                semantic_trace: None,
                debug_trace_path: None,
                drill_op: None,
            })
        };
        self.drained.push(ComputeMessage::Toolpath(Box::new(
            crate::compute::ComputeResult {
                toolpath_id: id,
                revision,
                result,
                debug_trace: None,
                semantic_trace: None,
                debug_trace_path: None,
            },
        )));
        ToolpathSubmitOutcome::Queued
    }

    fn submit_simulation(&mut self, _request: SimulationRequest) {
        self.simulations += 1;
        let bbox = rs_cam_core::geo::BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(10.0, 10.0, 10.0),
        };
        let mut prior_stocks = std::collections::HashMap::new();
        for window in self.chain.windows(2) {
            let (prev, next) = (window[0], window[1]);
            if self.generated.contains(&prev) {
                prior_stocks.insert(
                    next,
                    Arc::new(rs_cam_core::dexel_stock::TriDexelStock::from_bounds(
                        &bbox, 2.0,
                    )),
                );
            }
        }
        self.drained
            .push(ComputeMessage::Simulation(Ok(Box::new(SimulationResult {
                core: rs_cam_core::compute::simulate::SimulationResult {
                    mesh: rs_cam_core::stock::stock_mesh::StockMesh {
                        vertices: Vec::new(),
                        indices: Vec::new(),
                        colors: Vec::new(),
                    },
                    total_moves: 0,
                    deviations: None,
                    column_deviations: None,
                    boundaries: Vec::new(),
                    checkpoints: Vec::new(),
                    rapid_collisions: Vec::new(),
                    rapid_collision_move_indices: Vec::new(),
                    cut_trace: None,
                    column_grid_cell_mm: 0.5,
                    resolution_clamped: false,
                    prior_stocks,
                },
                playback_data: Vec::new(),
                cut_trace_path: None,
            }))));
    }

    fn submit_collision(&mut self, _request: CollisionRequest) {}
    fn submit_optimize(&mut self, _request: OptimizeRequest) {}
    fn cancel_lane(&mut self, _lane: ComputeLane) {}
    fn drain_results(&mut self) -> Vec<ComputeMessage> {
        std::mem::take(&mut self.drained)
    }
    fn lane_snapshot(&self, lane: ComputeLane) -> LaneSnapshot {
        LaneSnapshot::idle(lane)
    }
    fn generation_control(&self) -> crate::compute::GenerationControl {
        crate::compute::GenerationControl::detached()
    }
}

/// Build a project whose toolpaths form a `depth`-deep rest chain: index 0
/// cuts fresh stock, every later op takes the remaining stock of the one
/// before it.
#[cfg(feature = "mcp")]
fn rest_chain_controller(depth: usize) -> AppController<RestChainBackend> {
    let mut controller = AppController::with_backend(RestChainBackend::new());
    sample_project_into(&mut controller);
    for i in 1..=depth {
        push_toolpath(&mut controller, &format!("Rest {i}"));
    }
    let ids: Vec<ToolpathId> = controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .map(|tc| tc.id)
        .collect();
    for index in 1..controller.state.session.toolpath_count() {
        let _ = controller
            .state
            .session
            .apply(Command::SetStockSource(SetStockSourceArgs {
                index,
                source: crate::state::toolpath::StockSource::FromRemainingStock,
            }))
            .expect("the index comes from the session");
    }
    // The fixture op at index 0 starts with a cached result; clear it so the
    // run really is "from cold".
    for id in &ids {
        let rt = controller.state.gui.toolpath_rt_or_default(*id);
        rt.result = None;
        rt.status = crate::state::toolpath::ComputeStatus::Pending;
    }
    controller.compute.chain = ids;
    controller.pending_mcp = Some(crate::mcp_bridge::PendingMcpCompute::new());
    controller
}

/// Drive the controller until the generate_all oneshot resolves, or give up.
/// Returns the parsed response and the number of pump iterations.
#[cfg(feature = "mcp")]
fn pump_until_resolved(
    controller: &mut AppController<RestChainBackend>,
    rx: &mut tokio::sync::oneshot::Receiver<crate::mcp_bridge::McpResponse>,
) -> serde_json::Value {
    for _ in 0..200 {
        let events = controller.drain_events();
        for event in events {
            controller.handle_internal_event(event);
        }
        controller.drain_compute_results();
        if let Ok(resp) = rx.try_recv() {
            let payload = resp.result.expect("generate_all replies Ok(json)");
            return serde_json::from_str(&payload)
                .unwrap_or_else(|e| panic!("generate_all reply is not JSON ({e}): {payload}"));
        }
    }
    panic!("generate_all never resolved — the fixpoint loop is not terminating");
}

// ── G-REGEN-RACE: generate_all and process_auto_regen stop cancelling ────
//
// Reproduced live by B-4b on a never-minimised window and re-derived from
// the code here: `load_project` marks every toolpath `auto_regen` + stale,
// `process_auto_regen` submits them 500 ms later, and an agent's
// `generate_all` arriving while one of them is still the lane's ACTIVE job
// resubmits that same toolpath. The lane's resubmit-cancels-and-requeues
// rule aborts the in-flight job and queues the replacement; the abandoned
// job's `Cancelled` then drained as the toolpath's *outcome*, so
// `generate_all` reported `"Back Rough: generation cancelled"`,
// `generated: 0` for work it had itself replaced — while the replacement it
// queued went on to succeed unobserved.

/// A backend that models the toolpath lane's two load-bearing rules and
/// nothing else: (1) a submit for the toolpath that is currently ACTIVE
/// cancels that job and queues the replacement; (2) a cancelled job still
/// reports, with `Err(Cancelled)`. `finish_active` stands for the worker
/// thread completing whatever it is running.
#[cfg(feature = "mcp")]
struct LaneModelBackend {
    active: Option<ToolpathId>,
    active_cancelled: bool,
    queue: std::collections::VecDeque<ToolpathId>,
    drained: Vec<ComputeMessage>,
    /// Every toolpath id ever handed to `submit_toolpath`, in order.
    submits: Vec<ToolpathId>,
    /// The revision each submit read, kept the way the real lane keeps it on
    /// the handle. The completion stamps it, so the drain adopts at the
    /// revision the submit answered and refuses a result the operator has
    /// edited past.
    revisions: std::collections::HashMap<ToolpathId, u64>,
}

#[cfg(feature = "mcp")]
impl LaneModelBackend {
    fn new() -> Self {
        Self {
            active: None,
            active_cancelled: false,
            queue: std::collections::VecDeque::new(),
            drained: Vec::new(),
            submits: Vec::new(),
            revisions: std::collections::HashMap::new(),
        }
    }

    fn start_next(&mut self) {
        if self.active.is_none() {
            self.active = self.queue.pop_front();
            self.active_cancelled = false;
        }
    }

    /// The worker finishing its current job. A job whose cancel flag was
    /// set reports `Cancelled` even though it ran, exactly as
    /// `spawn_toolpath_lane` does.
    fn finish_active(&mut self) {
        let Some(id) = self.active.take() else {
            return;
        };
        let result = if self.active_cancelled {
            Err(crate::compute::ComputeError::Cancelled)
        } else {
            Ok(ToolpathResult {
                annotated: Arc::new(rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
                    Toolpath::new(),
                )),
                stats: Default::default(),
                debug_trace: None,
                semantic_trace: None,
                debug_trace_path: None,
                drill_op: None,
            })
        };
        self.active_cancelled = false;
        let revision = self.revisions.get(&id).copied();
        self.drained.push(ComputeMessage::Toolpath(Box::new(
            crate::compute::ComputeResult {
                toolpath_id: id,
                revision,
                result,
                debug_trace: None,
                semantic_trace: None,
                debug_trace_path: None,
            },
        )));
        self.start_next();
    }
}

#[cfg(feature = "mcp")]
impl ComputeBackend for LaneModelBackend {
    fn submit_toolpath(&mut self, request: ComputeRequest) -> ToolpathSubmitOutcome {
        let id = request.viz.toolpath_id;
        self.submits.push(id);
        self.revisions.insert(id, request.handle.revision);
        self.queue.retain(|queued| *queued != id);
        let outcome = if self.active == Some(id) {
            self.active_cancelled = true;
            ToolpathSubmitOutcome::SupersededActive
        } else {
            ToolpathSubmitOutcome::Queued
        };
        self.queue.push_back(id);
        self.start_next();
        outcome
    }

    fn submit_simulation(&mut self, _request: SimulationRequest) {}
    fn submit_collision(&mut self, _request: CollisionRequest) {}
    fn submit_optimize(&mut self, _request: OptimizeRequest) {}
    fn cancel_lane(&mut self, _lane: ComputeLane) {
        if self.active.is_some() {
            self.active_cancelled = true;
        }
    }

    fn drain_results(&mut self) -> Vec<ComputeMessage> {
        std::mem::take(&mut self.drained)
    }

    fn lane_snapshot(&self, lane: ComputeLane) -> LaneSnapshot {
        LaneSnapshot::idle(lane)
    }

    fn generation_control(&self) -> crate::compute::GenerationControl {
        crate::compute::GenerationControl::detached()
    }
}

/// A one-op project in exactly the state `load_project` leaves behind:
/// auto-regen armed, stale, no cached result.
#[cfg(feature = "mcp")]
fn freshly_loaded_controller() -> (AppController<LaneModelBackend>, ToolpathId) {
    let mut controller = AppController::with_backend(LaneModelBackend::new());
    sample_project_into(&mut controller);
    let tp_id = controller.state.session.toolpath_configs()[0].id;
    let rt = controller.state.gui.toolpath_rt_or_default(tp_id);
    rt.result = None;
    rt.status = crate::state::toolpath::ComputeStatus::Pending;
    rt.auto_regen = true;
    // Older than the 500 ms debounce, i.e. the sweep is due.
    rt.stale_since = Some(std::time::Instant::now() - std::time::Duration::from_millis(600));
    controller.pending_mcp = Some(crate::mcp_bridge::PendingMcpCompute::new());
    (controller, tp_id)
}

/// Pump the controller — events, then the lane finishing a job, then the
/// drain — until the `generate_all` oneshot resolves.
#[cfg(feature = "mcp")]
fn pump_lane_model(
    controller: &mut AppController<LaneModelBackend>,
    rx: &mut tokio::sync::oneshot::Receiver<crate::mcp_bridge::McpResponse>,
) -> serde_json::Value {
    for _ in 0..200 {
        let events = controller.drain_events();
        for event in events {
            controller.handle_internal_event(event);
        }
        controller.compute.finish_active();
        controller.drain_compute_results();
        if let Ok(resp) = rx.try_recv() {
            let payload = resp.result.expect("generate_all replies Ok(json)");
            return serde_json::from_str(&payload)
                .unwrap_or_else(|e| panic!("generate_all reply is not JSON ({e}): {payload}"));
        }
    }
    panic!("generate_all never resolved");
}

// ── Phase U: the multi-tool finishing planner dialog ─────────────────────
//
// The claim these carry is the VETO: opening the dialog and rejecting it must
// leave the project exactly as it was. The core sentry
// (`multitool_preview_u1.rs`) makes the byte-identical version of that claim
// about `preview_multitool_plan` itself; these make it about the GUI path,
// where the session is lent to a worker and handed back.

/// The sample project plus a two-ball ladder with distinct tip radii.
fn planner_controller() -> AppController<ScriptedBackend> {
    let mut controller = sample_controller();
    let mut tools = controller.state.session.tools().to_vec();
    for (raw_id, diameter) in [(2usize, 4.0f64), (3, 2.0)] {
        let mut tool = ToolConfig::new_default(ToolId(raw_id), ToolType::BallNose);
        tool.name = format!("Ball {diameter}");
        tool.diameter = diameter;
        tools.push(tool);
    }
    let _ = controller
        .state
        .session
        .apply(Command::ReplaceTools(ReplaceToolsArgs { tools }))
        .expect("the session takes the tool list");
    controller
}

/// Tick every ball-nose row, which is the two-radius ladder the dialog needs.
fn tick_ball_tools(controller: &mut AppController<ScriptedBackend>) {
    let planner = controller
        .state
        .multitool_planner
        .as_mut()
        .expect("the dialog is open");
    for row in &mut planner.tools {
        row.selected = row.name.starts_with("Ball ");
    }
}

/// A fingerprint of everything the planner is allowed to leave alone.
fn project_fingerprint(controller: &AppController<ScriptedBackend>) -> Vec<String> {
    controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .map(|tc| {
            format!(
                "{}|{}|{}|{:?}",
                tc.name,
                tc.tool_id,
                tc.enabled,
                tc.planner_origin.as_ref().map(|o| (o.plan_id, o.tier))
            )
        })
        .collect()
}

// ── G-FRESHSTATE — one freshness model (F2.1) ────────────────────────
//
// R0.1 (`planning/ui_fix_2026-09-09/research/R0.1.md`) §2.3 is a matrix of
// every mutation and what each of the three staleness stores did with it.
// The tests below walk that matrix against the ONE state the surfaces now
// read: `state::freshness::freshness_at`, derived from the core result
// cache. Each row asserts the state, and — where the row is one of the
// eight the core used to keep a result through — that the core result is
// actually gone, which is what stopped export emitting geometry for a
// configuration the project no longer had.

use crate::state::freshness::{FreshnessState, freshness_at};

use rs_cam_core::session::ToolpathComputeResult;

/// A cached core result, standing for "this toolpath has been generated".
fn core_result() -> ToolpathComputeResult {
    ToolpathComputeResult {
        op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(Arc::new(
            rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(Toolpath::new()),
        )),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
    }
}

/// Put every toolpath in the project into the `Current` state: a core
/// result, a `Done` status, and a drawable GUI result (the fixture already
/// gives toolpath 0 one).
fn generate_all_for_test<B: ComputeBackend>(controller: &mut AppController<B>) {
    let ids: Vec<(usize, ToolpathId)> = controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .enumerate()
        .map(|(idx, tc)| (idx, tc.id))
        .collect();
    for (index, id) in ids {
        let revision = controller.state.session.toolpath_revision(index);
        let _ = controller
            .state
            .session
            .apply(Command::AdoptResult(AdoptResultArgs {
                index,
                revision,
                result: Box::new(core_result()),
            }))
            .expect("index is in range");
        let rt = controller.state.gui.toolpath_rt_or_default(id);
        rt.status = crate::state::runtime::ComputeStatus::Done;
        rt.stale_since = None;
        if rt.result.is_none() {
            rt.result = Some(ToolpathResult {
                annotated: Arc::new(rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
                    Toolpath::new(),
                )),
                stats: Default::default(),
                debug_trace: None,
                semantic_trace: None,
                debug_trace_path: None,
                drill_op: None,
            });
        }
    }
    controller.state.gui.dirty = false;
}

fn state_of<B: ComputeBackend>(controller: &AppController<B>, index: usize) -> FreshnessState {
    freshness_at(&controller.state.session, &controller.state.gui, index)
        .expect("toolpath index exists")
}

/// Drive the inspector's write-back exactly as the panel does: build the
/// entry from the session, mutate it the way the widget would, write it
/// back. `edit` receives the entry.
///
/// The stale stamp and the dirty flag live inside the write-back since
/// WP5, so this helper adds neither. A helper that added its own would
/// stop measuring what the panel does.
fn panel_edit<B: ComputeBackend>(
    controller: &mut AppController<B>,
    id: ToolpathId,
    edit: impl FnOnce(&mut crate::state::toolpath::ToolpathEntry),
) {
    let mut entry = crate::ui::properties::build_entry_from_session_and_gui(
        id,
        &controller.state.session,
        &controller.state.gui,
    )
    .expect("toolpath exists");
    edit(&mut entry);
    // The panel's own order. The runtime write-back runs first because it
    // copies `entry.stale_since`; the config write-back stamps the fresh
    // value. A helper that reversed the two would stop modelling the
    // panel, and the stale-stamp assertions below would pass over a
    // defect the operator sees as a green card on dropped geometry.
    crate::ui::properties::write_entry_runtime_to_gui(&entry, &mut controller.state.gui);
    let _ = crate::ui::properties::write_entry_config_to_session(&entry, &mut controller.state);
}

// ── F2.5 / G-UNDOFRESH — an undo leaves the project where the same edit
//    made by hand would leave it ─────────────────────────────────────────
//
// The PLAN row names three symptoms — `ToolChange` undo does not call
// `invalidate_tool`, no arm calls `mark_edited`, `ToolpathParamChange` undo
// does not re-stale the simulation — and it was written before F2.1 existed.
// The rule underneath all three is one sentence: **after any undo or redo,
// the derived `FreshnessState` of every affected toolpath, the dirty flag and
// the simulation's staleness are what they would be if the operator had made
// that same edit by hand.** Each symptom is that rule broken in one place.
//
// On whether an undone edit restores `Current`: it does not, deliberately.
// The argument is in `reports/F2.5.md` §3 and in `apply_toolpath_snapshot`'s
// doc; `an_undone_param_edit_does_not_resurrect_the_old_result` is where it
// is pinned, so the decision cannot be reversed by accident.

/// A project with one generated toolpath, one simulation result recorded as
/// fresh, and a clean dirty flag — the state an operator is in when they
/// reach for Ctrl+Z.
fn controller_ready_for_undo() -> (AppController<ScriptedBackend>, ToolpathId) {
    let mut controller = sample_controller();
    generate_all_for_test(&mut controller);
    let tp_id = controller.state.session.toolpath_configs()[0].id;
    controller.state.simulation.last_run = Some(crate::state::simulation::SimulationRunMeta {
        sim_generation: 1,
        last_sim_edit_counter: controller.state.gui.edit_counter,
        // A hand-built fresh run answers the capture revision now set.
        accepted_metric_options_revision: Some(controller.state.simulation.metric_options_revision),
    });
    controller.state.gui.dirty = false;
    (controller, tp_id)
}

// ── F4.3 / G-MODELRELINK — the missing-model repair route ────────────────
//
// A project whose model file has moved had NO repair route in the UI. The
// only actions were "Reload from disk" — the same path that just failed —
// and "Delete", which is refused while any toolpath references the model,
// i.e. exactly when it matters. `load_error`, which holds the loader's own
// reason, was rendered nowhere in the GUI. And a project saved its model
// paths verbatim, so a folder copied to another machine could not resolve
// them even when the model travelled with it.

/// Write a project directory with a `models/` subfolder and a job that
/// points at a file inside it. Returns the temp directory.
fn relink_fixture_dir(name: &str) -> std::path::PathBuf {
    let dir = temp_path(name, "dir");
    std::fs::create_dir_all(dir.join("models")).expect("create fixture dir");
    let toml = r#"format_version = 3

[job]
name = "Relink Fixture"

[[tools]]
id = 1
name = "Fixture End Mill"
type = "end_mill"

[[models]]
id = 1
path = "models/missing.stl"
name = "Plate"
kind = "stl"

[[setups]]
name = "Setup 1"

[[setups.toolpaths]]
id = 1
name = "Rough"
type = "adaptive3d"
enabled = true
tool_id = 1
model_id = 1
"#;
    std::fs::write(dir.join("job.toml"), toml).expect("write fixture job");
    dir
}

// ── WP19 — every `Effects` answer reaches the view ───────────────────
//
// A core mutation answers with an `Effects`. `stale` reaches the
// toolpath cards; `simulation_cleared` reaches the viewport, because the
// session and the viewport hold ONE simulation (WP11b, N12 item 10).
// The source-scan half of this sentry is
// `crates/rs_cam_viz/tests/effects_are_stamped_wp19.rs`.

/// A rest-chain controller whose SESSION holds a simulation.
///
/// The control is load-bearing. `Effects::simulation_cleared` is
/// `simulation_before && self.simulation.is_none()`
/// (`session/command.rs`), so a fixture whose session holds no
/// simulation reports `false` on every row, and an arm built on it
/// proves nothing. The preamble is
/// `a_save_keeps_the_simulation_so_a_rest_op_still_starts_wp17`'s.
#[cfg(feature = "mcp")]
fn controller_holding_a_simulation() -> AppController<RestChainBackend> {
    let mut controller = rest_chain_controller(1);
    let ids: Vec<ToolpathId> = controller
        .state
        .session
        .toolpath_configs()
        .iter()
        .map(|tc| tc.id)
        .collect();
    controller.submit_toolpath_compute(ids[0]);
    controller.drain_compute_results();
    assert!(
        controller.run_simulation_with_all(),
        "the simulation must submit, or the arm measures nothing"
    );
    controller.drain_compute_results();
    assert!(
        controller.state.session.simulation_result().is_some(),
        "the control: the session holds a simulation before the edit"
    );
    assert!(
        controller.state.simulation.results.is_some(),
        "the control: the viewport holds one too"
    );
    controller
}

// --- SHL-01: one door rebuilds the post mirror --------------------------

/// The mirror must carry the WHOLE session block, not the one field a
/// route remembered to copy.
///
/// `PostConfig` has no `PartialEq`, so the comparison runs through
/// `post_to_session`, which is the writer half of the same resolver pair
/// `post_from_session` reads. A field that reaches neither half is a
/// field the project file does not carry either.
fn assert_post_mirror_matches_session<B: ComputeBackend>(
    controller: &AppController<B>,
    route: &str,
) {
    let mirrored = crate::state::runtime::GuiState::post_to_session(&controller.state.gui.post);
    assert_eq!(
        &mirrored,
        controller.state.session.post_config(),
        "SHL-01: after {route} the viz post mirror does not match the session block"
    );
}

/// Drive every mirror field away from the session, so a route that copies
/// ONE field back leaves the rest wrong.
fn drift_the_post_mirror<B: ComputeBackend>(controller: &mut AppController<B>) {
    let post = &mut controller.state.gui.post;
    post.format = rs_cam_core::gcode::PostFormat::Mach3;
    post.spindle_speed = post.spindle_speed.wrapping_add(1234);
    post.safe_z += 37.5;
    post.high_feedrate_mode = !post.high_feedrate_mode;
    post.high_feedrate += 111.0;
    post.spindle_strategy = rs_cam_core::feeds::SpindleStrategy::MaxSpeed;
}
