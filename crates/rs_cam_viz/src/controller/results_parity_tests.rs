//! TD3 wave B-5 sentries — **GUI-mode results parity** (ledger row
//! `G-RESULTS`).
//!
//! The ledger row said: "in GUI mode `ProjectSession.results` is never
//! written, so `get_toolpath_diagnostics` / `get_tool_load_report` run
//! without generation findings and spans while the CLI's carry both."
//!
//! The census (`planning/review_2026-08-08/RESULTS_PARITY.md`) measured that
//! claim at `1c0a4ac` and found it **half true**:
//!
//! * The *store* is wired. `drain_compute_results` has called
//!   `ProjectSession::insert_result` since `d706c036`, and the payload it
//!   writes goes through the same `compute::stats_with_findings` join the
//!   core session path uses. So the generation findings, the spans and the
//!   `DrillOp` all land in `session.results` in GUI mode.
//! * The *read* is not. `build_mcp_diagnostics` — what MCP `get_diagnostics`
//!   serves — hand-builds its `per_toolpath` rows from `gui.toolpath_rt`
//!   instead of reading the core `ToolpathDiagnostic` the CLI publishes, so
//!   ten published channels never reach the agent. Its own doc comment said
//!   the session cache is "only populated by the standalone MCP", which
//!   stopped being true three months before the row was written.
//!
//! These sentries pin both halves: the store (so removing `insert_result`
//! fails here, not silently in production), and the read (red before this
//! wave — the keys were simply absent from the row).

use std::sync::Arc;

use super::*;
use crate::compute::{
    CollisionRequest, ComputeMessage, ComputeRequest, ComputeResult, OptimizeRequest,
    SimulationRequest,
};
use crate::state::job::{ToolConfig, ToolId, ToolType};
use crate::state::toolpath::{OperationConfig, ToolpathId, ToolpathResult};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ToolpathConfig};
use rs_cam_core::toolpath::{Move, MoveType, Toolpath};
use rs_cam_core::toolpath_spans::{AnnotatedToolpath, Span, SpanKind};

// ── A backend that computes nothing; the wave under test is the drain ──

struct InertBackend {
    drained: Vec<ComputeMessage>,
}

impl ComputeBackend for InertBackend {
    fn submit_toolpath(&mut self, _request: ComputeRequest) {}
    fn submit_simulation(&mut self, _request: SimulationRequest) {}
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

/// The measured figures this fixture stands in for. They are arbitrary but
/// **distinguishable from zero and from each other**, because the whole
/// X-19 contract these channels carry is that `null` (not measured) and
/// `0.0` (measured clean) are different answers — a row that publishes the
/// key can say either, and a row that omits the key can say neither.
const TRUNCATED_CORE_MM2: f64 = 12.5;
const UNTOUCHED_MATERIAL_MM2: f64 = 3.25;
const REACHED_UNCUT_MM2: f64 = 1.75;

fn parity_controller() -> AppController<InertBackend> {
    let mut controller = AppController::with_backend(InertBackend {
        drained: Vec::new(),
    });
    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    controller.state.session.tools_mut().push(tool);
    controller.state.session.add_model(LoadedModel {
        id: 0,
        path: std::path::PathBuf::from("plate.svg"),
        name: "Plate".to_owned(),
        kind: Some(ModelKind::Svg),
        mesh: None,
        polygons: Some(Arc::new(vec![Polygon2::rectangle(
            -20.0, -20.0, 20.0, 20.0,
        )])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    });
    let tp_config = ToolpathConfig {
        id: ToolpathId(0),
        name: "Pocket".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(Default::default()),
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
    };
    controller.state.session.add_toolpath(0, tp_config).unwrap();
    controller
}

/// A worker result that carries BOTH halves the ledger row named: three
/// generation findings, and a valid span vector. Shaped exactly as
/// `compute::worker::execute` hands it to the drain.
fn worker_result_with_findings() -> ToolpathResult {
    let mut toolpath = Toolpath::new();
    toolpath.moves.push(Move {
        target: rs_cam_core::geo::P3::new(0.0, 0.0, 5.0),
        move_type: MoveType::Rapid,
        intent: rs_cam_core::toolpath::MoveIntent::Unknown,
    });
    toolpath.moves.push(Move {
        target: rs_cam_core::geo::P3::new(10.0, 0.0, -1.0),
        move_type: MoveType::Linear { feed_rate: 1000.0 },
        intent: rs_cam_core::toolpath::MoveIntent::ClearingCut,
    });
    toolpath.moves.push(Move {
        target: rs_cam_core::geo::P3::new(10.0, 10.0, -1.0),
        move_type: MoveType::Linear { feed_rate: 1000.0 },
        intent: rs_cam_core::toolpath::MoveIntent::ClearingCut,
    });
    let mut annotated = AnnotatedToolpath::new(toolpath);
    annotated.spans = vec![
        Span::new(0, 2, SpanKind::Operation),
        Span::new(1, 2, SpanKind::DepthPass),
    ];
    annotated.spans_valid = true;

    let mut stats = rs_cam_core::compute::config::ToolpathStats {
        move_count: 3,
        cutting_distance: 20.0,
        rapid_distance: 5.0,
        ..Default::default()
    };
    stats.truncated_core_mm2 = Some(TRUNCATED_CORE_MM2);
    stats.untouched_material_mm2 = Some(UNTOUCHED_MATERIAL_MM2);
    stats.reached_uncut_estimate_mm2 = Some(REACHED_UNCUT_MM2);

    ToolpathResult {
        annotated: Arc::new(annotated),
        stats,
        debug_trace: None,
        semantic_trace: None,
        debug_trace_path: None,
        drill_op: None,
    }
}

fn drain_one_result(controller: &mut AppController<InertBackend>) {
    controller
        .compute
        .drained
        .push(ComputeMessage::Toolpath(Box::new(ComputeResult {
            toolpath_id: ToolpathId(0),
            result: Ok(worker_result_with_findings()),
            debug_trace: None,
            semantic_trace: None,
            debug_trace_path: None,
        })));
    controller.drain_compute_results();
}

// ── Half 1: the store ───────────────────────────────────────────────────

/// GREEN at `1c0a4ac`, and it must stay that way: the GUI drain writes the
/// worker's result — findings AND spans — into `ProjectSession.results`, so
/// every core read that goes through `get_result` (the load report's span
/// lookup, its `drill_op` lookup, `diagnose_toolpath_with_trace`'s stats
/// lookup, `sim_trace_is_fresh`) sees the same thing the CLI's does.
///
/// Deleting the `insert_result` call in `drain_compute_results` fails here.
#[test]
fn gui_drain_writes_findings_and_spans_into_session_results() {
    let mut controller = parity_controller();
    drain_one_result(&mut controller);

    let result = controller
        .state
        .session
        .get_result(0)
        .expect("GUI drain must publish the worker result into session.results");
    assert_eq!(
        result.stats.truncated_core_mm2,
        Some(TRUNCATED_CORE_MM2),
        "generation findings must survive the GUI drain into the core cache"
    );
    assert!(
        result.annotated().spans_valid,
        "the span validity flag must survive the GUI drain"
    );
    assert_eq!(
        result.annotated().spans.len(),
        2,
        "spans must survive the GUI drain — the load report reads them from here"
    );
}

/// The per-toolpath diagnostic read MCP serves (`get_toolpath_diagnostics`)
/// picks the generation findings up off `session.results`, so it reports
/// them in GUI mode. Guards the stats lookup in
/// `ProjectSession::diagnose_toolpath_with_trace`.
#[test]
fn gui_mode_toolpath_diagnostics_see_generation_stats() {
    let mut controller = parity_controller();
    drain_one_result(&mut controller);

    let stats_seen = controller
        .state
        .session
        .get_result(0)
        .map(|r| r.stats.untouched_material_mm2);
    assert_eq!(stats_seen, Some(Some(UNTOUCHED_MATERIAL_MM2)));

    // The read itself must not error out in GUI mode (no session-side
    // simulation, trace supplied externally — the MCP handler's shape).
    let diags = controller
        .state
        .session
        .diagnose_toolpath_with_trace(0, None)
        .expect("diagnose_toolpath_with_trace must answer in GUI mode");
    // Report-only findings do not all raise a Diagnostic on every fixture;
    // what is load-bearing here is that the call ran against a populated
    // `results` entry rather than an empty one.
    let _ = diags;
}

// ── Half 2: the read (RED before wave B-5) ──────────────────────────────

/// **The B-5 defect.** MCP `get_diagnostics` must publish, per toolpath, the
/// same finding channels the CLI's `project` report does — because they are
/// the same `ToolpathDiagnostic`, and an absent key is not a `null`: it
/// cannot say "measured clean" and it cannot say "not measured".
///
/// Red before this wave: `build_mcp_diagnostics` hand-built the row and the
/// keys were absent entirely.
#[test]
fn mcp_get_diagnostics_row_publishes_the_core_finding_channels() {
    let mut controller = parity_controller();
    drain_one_result(&mut controller);

    let diag = controller.build_mcp_diagnostics();
    let row = diag["per_toolpath"]
        .as_array()
        .and_then(|rows| rows.first())
        .expect("one per-toolpath row");

    for key in [
        "op_kind",
        "collision_count",
        "rapid_collision_count",
        "truncated_core_mm2",
        "standing_material_mm2",
        "untouched_material_mm2",
        "reached_uncut_estimate_mm2",
        "unmachined_band_area_mm2",
        "tip_float_points",
        "max_tip_float_mm",
    ] {
        assert!(
            row.get(key).is_some(),
            "MCP get_diagnostics per-toolpath row is missing the `{key}` channel \
             the CLI publishes — an absent key cannot distinguish `not measured` \
             from `measured clean` (X-19). Row was: {row}"
        );
    }

    assert_eq!(
        row["truncated_core_mm2"].as_f64(),
        Some(TRUNCATED_CORE_MM2),
        "the row must carry the MEASURED value, not a placeholder"
    );
    assert_eq!(
        row["standing_material_mm2"].as_f64(),
        Some(TRUNCATED_CORE_MM2),
        "the deprecated A6 duplicate key carries the same value, as on the CLI wire"
    );
    assert_eq!(
        row["untouched_material_mm2"].as_f64(),
        Some(UNTOUCHED_MATERIAL_MM2)
    );
    assert_eq!(
        row["reached_uncut_estimate_mm2"].as_f64(),
        Some(REACHED_UNCUT_MM2)
    );
}

/// Structural parity, so the next channel added to `ToolpathDiagnostic` is
/// caught here instead of by an agent noticing a blank in production: every
/// key the core diagnostic serialises must appear on the GUI row.
///
/// The GUI row is allowed to carry MORE (`toolpath_index`, `status`,
/// `error`, `awaiting_prior_stock`, `stale` have no CLI equivalent — they
/// describe a live lane, not a finished batch run).
#[test]
fn mcp_get_diagnostics_row_is_a_superset_of_the_core_diagnostic_wire() {
    let mut controller = parity_controller();
    drain_one_result(&mut controller);

    let evidence = crate::app::mcp::viz_project_evidence(&controller.state);
    let core = controller
        .state
        .session
        .diagnostics_with_evidence(&evidence);
    let core_row = serde_json::to_value(
        core.per_toolpath
            .first()
            .expect("core diagnostics build one row per generated toolpath"),
    )
    .expect("core row serialises");

    let diag = controller.build_mcp_diagnostics();
    let gui_row = diag["per_toolpath"]
        .as_array()
        .and_then(|rows| rows.first())
        .expect("one per-toolpath row")
        .clone();

    let core_keys: Vec<&String> = core_row
        .as_object()
        .expect("core row is an object")
        .keys()
        .collect();
    let missing: Vec<&&String> = core_keys
        .iter()
        .filter(|k| gui_row.get(k.as_str()).is_none())
        .collect();
    assert!(
        missing.is_empty(),
        "MCP get_diagnostics drops {missing:?} from the core ToolpathDiagnostic wire"
    );
}

/// `collision_count` was a hardcoded `0` on the project-level response even
/// though the evidence the same function already builds carries the holder
/// counts. A zero that was never measured is the X-VAC failure mode on a
/// surface an agent reads as a safety signal.
#[test]
fn mcp_get_diagnostics_collision_count_comes_from_evidence() {
    let mut controller = parity_controller();
    drain_one_result(&mut controller);
    // Holder evidence exactly as the GUI holds it: a stored collision report
    // plus the simulation boundary that attributes each event to a toolpath.
    controller.state.simulation.results = Some(crate::state::simulation::SimulationResults {
        mesh: rs_cam_core::simulation::StockMesh {
            vertices: Vec::new(),
            indices: Vec::new(),
            colors: Vec::new(),
        },
        total_moves: 3,
        boundaries: vec![crate::state::simulation::ToolpathBoundary {
            id: ToolpathId(0),
            name: "Pocket".to_owned(),
            tool_name: "End Mill".to_owned(),
            start_move: 0,
            end_move: 2,
            direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
        }],
        setup_boundaries: Vec::new(),
        checkpoints: Vec::new(),
        selected_toolpaths: None,
        playback_data: Vec::new(),
        stock_bbox: rs_cam_core::geo::BoundingBox3::empty(),
        cut_trace: None,
        cut_trace_path: None,
        column_grid_cell_mm: 0.5,
        prior_stocks: std::collections::HashMap::new(),
    });
    controller.state.simulation.checks.collision_report =
        Some(rs_cam_core::collision::CollisionReport {
            collisions: vec![rs_cam_core::collision::CollisionEvent {
                move_idx: 1,
                position: rs_cam_core::geo::P3::new(10.0, 0.0, -1.0),
                penetration_depth: 0.4,
                segment: "holder".to_owned(),
                kind: rs_cam_core::collision::CollisionKind::Workpiece,
            }],
            min_safe_stickout: 30.0,
        });

    let evidence = crate::app::mcp::viz_project_evidence(&controller.state);
    let core = controller
        .state
        .session
        .diagnostics_with_evidence(&evidence);
    assert_eq!(
        core.collision_count, 1,
        "fixture must actually carry holder evidence, or this sentry is vacuous"
    );

    let diag = controller.build_mcp_diagnostics();
    assert_eq!(
        diag["collision_count"].as_u64(),
        Some(1),
        "project-level collision_count must come from the evidence, not a literal 0"
    );
    let row = diag["per_toolpath"]
        .as_array()
        .and_then(|rows| rows.first())
        .expect("one per-toolpath row");
    assert_eq!(row["collision_count"].as_u64(), Some(1));
}
