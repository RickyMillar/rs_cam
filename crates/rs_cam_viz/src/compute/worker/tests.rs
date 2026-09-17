use super::*;
use crate::compute::worker::test_fixture::{
    RequestSpec, board, request as build_request, stock_between,
};
use crate::compute::{ComputeBackend, ComputeLane, ComputeMessage, LaneState};
use crate::state::toolpath::{DressupConfig, OperationConfig, OperationType};
use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::compute::simulate::{SimGroupEntry, SimToolpathEntry};
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::mesh::{make_test_flat, make_test_hemisphere};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::toolpath::Toolpath;
use std::thread;
use std::time::{Duration, Instant};

use crate::state::job::{ToolId, ToolType};

// WP11b: these fixtures state a SPEC and let `test_fixture::request` run the
// production submit door. Two fixture facts an assertion depends on:
//
// * the 2D fixtures top their board at `z = 0`, which is where
//   `HeightContext::simple` used to put `heights.top_z`. The session derives
//   the heights from the stock, so the stock has to say it.
// * the 3D fixtures keep their original stock bounds, with the board top
//   ABOVE the mesh. A roughing pass over a board level with the mesh removes
//   nothing, and the empty-generation gate refuses that.

/// A 40 x 40 mm square over a board topped at `z = 0`.
fn pocket_spec(id: usize) -> RequestSpec {
    let mut spec = RequestSpec::new(
        id,
        &format!("Pocket {id}"),
        OperationConfig::new_default(OperationType::Pocket),
        ToolConfig::new_default(ToolId(1), ToolType::EndMill),
    );
    spec.stock = board(25.0, 15.0);
    spec.dressups = DressupConfig::default();
    spec
}

fn adaptive_spec(id: usize) -> RequestSpec {
    let mut spec = pocket_spec(id);
    spec.name = format!("Adaptive {id}");
    spec.operation = OperationConfig::new_default(OperationType::Adaptive);
    spec
}

fn profile_spec(id: usize) -> RequestSpec {
    let OperationConfig::Profile(mut cfg) = OperationConfig::new_default(OperationType::Profile)
    else {
        unreachable!("default op kind mismatch");
    };
    cfg.finishing_passes = 1;
    cfg.tab_count = 2;
    let mut spec = pocket_spec(id);
    spec.name = format!("Profile {id}");
    spec.operation = OperationConfig::Profile(cfg);
    spec
}

fn heavy_dropcutter_spec(id: usize) -> RequestSpec {
    let OperationConfig::DropCutter(mut cfg) =
        OperationConfig::new_default(OperationType::DropCutter)
    else {
        unreachable!("default op kind mismatch");
    };
    cfg.stepover = 0.25;
    cfg.min_z = -5.0;
    let mut spec = RequestSpec::new(
        id,
        &format!("DropCutter {id}"),
        OperationConfig::DropCutter(cfg),
        ToolConfig::new_default(ToolId(1), ToolType::BallNose),
    )
    .with_mesh(make_test_flat(120.0));
    spec.stock = stock_between(P3::new(-60.0, -60.0, -5.0), P3::new(60.0, 60.0, 10.0));
    spec.dressups = DressupConfig::default();
    spec
}

fn waterline_spec(id: usize) -> RequestSpec {
    let OperationConfig::Waterline(mut cfg) =
        OperationConfig::new_default(OperationType::Waterline)
    else {
        unreachable!("default op kind mismatch");
    };
    cfg.z_step = 1.0;
    cfg.sampling = 1.0;
    let mut spec = RequestSpec::new(
        id,
        &format!("Waterline {id}"),
        OperationConfig::Waterline(cfg),
        ToolConfig::new_default(ToolId(1), ToolType::EndMill),
    )
    .with_mesh(make_test_flat(60.0));
    spec.stock = stock_between(P3::new(-30.0, -30.0, -5.0), P3::new(30.0, 30.0, 10.0));
    spec.dressups = DressupConfig::default();
    spec
}

fn adaptive3d_spec(id: usize) -> RequestSpec {
    let OperationConfig::Adaptive3d(mut cfg) =
        OperationConfig::new_default(OperationType::Adaptive3d)
    else {
        unreachable!("default op kind mismatch");
    };
    cfg.depth_per_pass = 2.0;
    cfg.detect_flat_areas = true;
    cfg.region_ordering = crate::state::toolpath::RegionOrdering::ByArea;
    let mut spec = RequestSpec::new(
        id,
        &format!("Adaptive3d {id}"),
        OperationConfig::Adaptive3d(cfg),
        ToolConfig::new_default(ToolId(1), ToolType::EndMill),
    )
    .with_mesh(make_test_flat(60.0));
    spec.stock = stock_between(P3::new(-30.0, -30.0, -5.0), P3::new(30.0, 30.0, 10.0));
    spec.dressups = DressupConfig::default();
    spec
}

fn drill_spec(id: usize) -> RequestSpec {
    let OperationConfig::Drill(mut cfg) = OperationConfig::new_default(OperationType::Drill) else {
        unreachable!("default op kind mismatch");
    };
    cfg.cycle = crate::state::toolpath::DrillCycleType::Peck;
    let mut spec = RequestSpec::new(
        id,
        &format!("Drill {id}"),
        OperationConfig::Drill(cfg),
        ToolConfig::new_default(ToolId(1), ToolType::EndMill),
    );
    spec.polygons = Some(vec![
        Polygon2::rectangle(-10.0, -10.0, -6.0, -6.0),
        Polygon2::rectangle(6.0, 6.0, 10.0, 10.0),
    ]);
    // G-DRILLCENTROID: the holes are drill targets, not the two rectangles'
    // centroids (which the generator no longer reads).
    spec.drill_targets = vec![
        rs_cam_core::io::dxf_input::DrillTarget {
            x: -8.0,
            y: -8.0,
            layer: "holes".to_owned(),
            kind: rs_cam_core::io::dxf_input::DrillTargetKind::CircleCenter { diameter: 4.0 },
        },
        rs_cam_core::io::dxf_input::DrillTarget {
            x: 8.0,
            y: 8.0,
            layer: "holes".to_owned(),
            kind: rs_cam_core::io::dxf_input::DrillTargetKind::CircleCenter { diameter: 4.0 },
        },
    ];
    spec.stock = board(20.0, 25.0);
    spec.dressups = DressupConfig::default();
    spec
}

fn steep_shallow_spec(id: usize) -> RequestSpec {
    let mut spec = RequestSpec::new(
        id,
        &format!("SteepShallow {id}"),
        OperationConfig::new_default(OperationType::SteepShallow),
        ToolConfig::new_default(ToolId(1), ToolType::BallNose),
    )
    .with_mesh(make_test_hemisphere(20.0, 16));
    spec.stock = stock_between(P3::new(-20.0, -20.0, -20.0), P3::new(20.0, 20.0, 20.0));
    spec.dressups = DressupConfig::default();
    spec
}

fn make_v_groove_mesh(length: f64, depth: f64, width: f64) -> TriangleMesh {
    TriangleMesh::from_raw(
        vec![
            P3::new(0.0, -width, 0.0),
            P3::new(length, -width, 0.0),
            P3::new(0.0, 0.0, -depth),
            P3::new(length, 0.0, -depth),
            P3::new(0.0, width, 0.0),
            P3::new(length, width, 0.0),
        ],
        vec![[0, 2, 1], [1, 2, 3], [2, 4, 3], [3, 4, 5]],
    )
}

fn pencil_spec(id: usize) -> RequestSpec {
    let mut spec = RequestSpec::new(
        id,
        &format!("Pencil {id}"),
        OperationConfig::new_default(OperationType::Pencil),
        ToolConfig::new_default(ToolId(1), ToolType::BallNose),
    )
    .with_mesh(make_v_groove_mesh(40.0, 6.0, 12.0));
    spec.stock = stock_between(P3::new(0.0, -15.0, -10.0), P3::new(40.0, 15.0, 10.0));
    spec.dressups = DressupConfig::default();
    spec
}

fn scallop_spec(id: usize) -> RequestSpec {
    let OperationConfig::Scallop(mut cfg) = OperationConfig::new_default(OperationType::Scallop)
    else {
        unreachable!("default op kind mismatch");
    };
    cfg.continuous = true;
    cfg.scallop_height = 0.2;
    cfg.tolerance = 0.2;
    let mut spec = RequestSpec::new(
        id,
        &format!("Scallop {id}"),
        OperationConfig::Scallop(cfg),
        ToolConfig::new_default(ToolId(1), ToolType::BallNose),
    )
    .with_mesh(make_test_hemisphere(20.0, 16));
    spec.stock = stock_between(P3::new(-25.0, -25.0, -5.0), P3::new(25.0, 25.0, 25.0));
    spec.dressups = DressupConfig::default();
    spec
}

fn ramp_finish_spec(id: usize) -> RequestSpec {
    let OperationConfig::RampFinish(mut cfg) =
        OperationConfig::new_default(OperationType::RampFinish)
    else {
        unreachable!("default op kind mismatch");
    };
    cfg.max_stepdown = 2.0;
    cfg.sampling = 2.0;
    cfg.tolerance = 0.2;
    let mut spec = RequestSpec::new(
        id,
        &format!("Ramp finish {id}"),
        OperationConfig::RampFinish(cfg),
        ToolConfig::new_default(ToolId(1), ToolType::BallNose),
    )
    .with_mesh(make_test_hemisphere(20.0, 16));
    spec.stock = stock_between(P3::new(-25.0, -25.0, -5.0), P3::new(25.0, 25.0, 25.0));
    spec.dressups = DressupConfig::default();
    spec
}

fn spiral_finish_spec(id: usize) -> RequestSpec {
    let OperationConfig::SpiralFinish(mut cfg) =
        OperationConfig::new_default(OperationType::SpiralFinish)
    else {
        unreachable!("default op kind mismatch");
    };
    cfg.stepover = 2.0;
    let mut spec = RequestSpec::new(
        id,
        &format!("Spiral finish {id}"),
        OperationConfig::SpiralFinish(cfg),
        ToolConfig::new_default(ToolId(1), ToolType::BallNose),
    )
    .with_mesh(make_test_hemisphere(20.0, 16));
    spec.stock = stock_between(P3::new(-25.0, -25.0, -5.0), P3::new(25.0, 25.0, 25.0));
    spec.dressups = DressupConfig::default();
    spec
}

fn radial_finish_spec(id: usize) -> RequestSpec {
    let OperationConfig::RadialFinish(mut cfg) =
        OperationConfig::new_default(OperationType::RadialFinish)
    else {
        unreachable!("default op kind mismatch");
    };
    cfg.angular_step = 30.0;
    cfg.point_spacing = 2.0;
    let mut spec = RequestSpec::new(
        id,
        &format!("Radial finish {id}"),
        OperationConfig::RadialFinish(cfg),
        ToolConfig::new_default(ToolId(1), ToolType::BallNose),
    )
    .with_mesh(make_test_flat(80.0));
    spec.stock = stock_between(P3::new(-40.0, -40.0, -5.0), P3::new(40.0, 40.0, 15.0));
    spec.dressups = DressupConfig::default();
    spec
}

fn horizontal_finish_spec(id: usize) -> RequestSpec {
    let OperationConfig::HorizontalFinish(mut cfg) =
        OperationConfig::new_default(OperationType::HorizontalFinish)
    else {
        unreachable!("default op kind mismatch");
    };
    cfg.stepover = 3.0;
    let mut spec = RequestSpec::new(
        id,
        &format!("Horizontal finish {id}"),
        OperationConfig::HorizontalFinish(cfg),
        ToolConfig::new_default(ToolId(1), ToolType::BallNose),
    )
    .with_mesh(make_test_flat(80.0));
    spec.stock = stock_between(P3::new(-40.0, -40.0, -5.0), P3::new(40.0, 40.0, 15.0));
    spec.dressups = DressupConfig::default();
    spec
}

fn project_curve_spec(id: usize) -> RequestSpec {
    let OperationConfig::ProjectCurve(mut cfg) =
        OperationConfig::new_default(OperationType::ProjectCurve)
    else {
        unreachable!("default op kind mismatch");
    };
    cfg.depth = 0.75;
    cfg.point_spacing = 1.0;
    let mut spec = RequestSpec::new(
        id,
        &format!("Project curve {id}"),
        OperationConfig::ProjectCurve(cfg),
        ToolConfig::new_default(ToolId(1), ToolType::BallNose),
    );
    spec.mesh = Some(make_test_hemisphere(20.0, 16));
    spec.polygons = Some(vec![
        Polygon2::rectangle(-12.0, -12.0, 12.0, 12.0),
        Polygon2::rectangle(-6.0, -4.0, 6.0, 4.0),
    ]);
    spec.stock = stock_between(P3::new(-25.0, -25.0, -5.0), P3::new(25.0, 25.0, 25.0));
    spec.dressups = DressupConfig::default();
    spec
}

fn long_simulation_request() -> SimulationRequest {
    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    let mut toolpath = Toolpath::new();
    toolpath.rapid_to(P3::new(0.0, 0.0, 5.0));
    for i in 0..80_000 {
        let x = (i % 400) as f64 * 0.2;
        let y = (i / 400) as f64 * 0.2;
        toolpath.feed_to(P3::new(x, y, -1.0), 600.0);
    }

    let stock_bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, -2.0),
        max: P3::new(100.0, 100.0, 10.0),
    };
    SimulationRequest {
        core: rs_cam_core::compute::simulate::SimulationRequest {
            groups: vec![SimGroupEntry {
                toolpaths: vec![SimToolpathEntry {
                    id: ToolpathId(99),
                    name: "Long Sim".to_owned(),
                    annotated: Arc::new(
                        rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(toolpath),
                    ),
                    tool: Arc::new(build_cutter(&tool)),
                    flute_count: tool.flute_count,
                    tool_summary: tool.summary(),
                    semantic_trace: None,
                    spindle_rpm: None,
                    metrics_not_applicable: false,
                    drill_op: None,
                    operation_config_hash: 0,
                }],
                direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                local_stock_bbox: None,
                local_to_global: None,
                phantom_prior_stock: None,
            }],
            stock_bbox,
            stock_top_z: 10.0,
            resolution: 0.5,
            metric_options: rs_cam_core::stock::simulation_cut::SimulationMetricOptions::default(),
            spindle_rpm: 18_000,
            rapid_feed_mm_min: 5_000.0,
            model_mesh: None,
            kinematics: None,
        },
        memoize_prefix: false,
    }
}

fn small_simulation_request_with_metrics(enabled: bool) -> SimulationRequest {
    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    let mut toolpath = Toolpath::new();
    toolpath.rapid_to(P3::new(2.0, 2.0, 8.0));
    toolpath.feed_to(P3::new(2.0, 2.0, 4.0), 400.0);
    toolpath.feed_to(P3::new(12.0, 2.0, 4.0), 400.0);
    toolpath.rapid_to(P3::new(12.0, 2.0, 8.0));

    let stock_bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(20.0, 20.0, 10.0),
    };
    SimulationRequest {
        core: rs_cam_core::compute::simulate::SimulationRequest {
            groups: vec![SimGroupEntry {
                toolpaths: vec![SimToolpathEntry {
                    id: ToolpathId(1),
                    name: "Metrics".to_owned(),
                    annotated: Arc::new(
                        rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(toolpath),
                    ),
                    tool: Arc::new(build_cutter(&tool)),
                    flute_count: tool.flute_count,
                    tool_summary: tool.summary(),
                    semantic_trace: None,
                    spindle_rpm: None,
                    metrics_not_applicable: false,
                    drill_op: None,
                    operation_config_hash: 0,
                }],
                direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                local_stock_bbox: None,
                local_to_global: None,
                phantom_prior_stock: None,
            }],
            stock_bbox,
            stock_top_z: 10.0,
            resolution: 0.5,
            metric_options: rs_cam_core::stock::simulation_cut::SimulationMetricOptions {
                enabled,
                capture_arc_engagement: enabled,
            },
            spindle_rpm: 18_000,
            rapid_feed_mm_min: 5_000.0,
            model_mesh: None,
            kinematics: None,
        },
        memoize_prefix: false,
    }
}

fn small_simulation_request_with_semantic_metrics(enabled: bool) -> SimulationRequest {
    let mut req = small_simulation_request_with_metrics(enabled);
    let recorder =
        rs_cam_core::trace::semantic_trace::ToolpathSemanticRecorder::new("Metrics", "Metrics");
    let root = recorder.root_context();
    let op = root.start_item(
        rs_cam_core::trace::semantic_trace::ToolpathSemanticKind::Operation,
        "Metrics",
    );
    let pass = op.context().start_item(
        rs_cam_core::trace::semantic_trace::ToolpathSemanticKind::Pass,
        "Pass 1",
    );
    if let Some(toolpath) = req
        .core
        .groups
        .first()
        .and_then(|group| group.toolpaths.first())
    {
        pass.bind_to_toolpath(
            &toolpath.annotated.toolpath,
            0,
            toolpath.annotated.toolpath.moves.len(),
        );
    }
    let semantic_trace = Arc::new(recorder.finish());
    req.core.groups[0].toolpaths[0].semantic_trace = Some(semantic_trace);
    req
}

fn wait_for<F>(
    backend: &mut ThreadedComputeBackend,
    timeout: Duration,
    mut predicate: F,
) -> Option<ComputeMessage>
where
    F: FnMut(&ComputeMessage) -> bool,
{
    let start = Instant::now();
    while start.elapsed() < timeout {
        for message in backend.drain_results() {
            if predicate(&message) {
                return Some(message);
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
    None
}

fn assert_toolpaths_match(left: &Toolpath, right: &Toolpath) {
    assert_eq!(
        left.moves.len(),
        right.moves.len(),
        "toolpaths should have the same move count"
    );

    for (index, (lhs, rhs)) in left.moves.iter().zip(&right.moves).enumerate() {
        assert_eq!(
            lhs.move_type, rhs.move_type,
            "move type mismatch at index {index}"
        );
        assert!(
            (lhs.target.x - rhs.target.x).abs() < 1e-9,
            "x mismatch at move {index}: {} vs {}",
            lhs.target.x,
            rhs.target.x
        );
        assert!(
            (lhs.target.y - rhs.target.y).abs() < 1e-9,
            "y mismatch at move {index}: {} vs {}",
            lhs.target.y,
            rhs.target.y
        );
        assert!(
            (lhs.target.z - rhs.target.z).abs() < 1e-9,
            "z mismatch at move {index}: {} vs {}",
            lhs.target.z,
            rhs.target.z
        );
    }
}

#[test]
fn debug_enabled_compute_attaches_trace_and_keeps_geometry_stable() {
    let debug_req = build_request(pocket_spec(41).with_debug_trace());

    let debug_result = super::execute::run_compute(&debug_req);
    let debug_toolpath = debug_result.result.expect("debug compute should succeed");
    let trace = debug_toolpath
        .debug_trace
        .as_ref()
        .expect("debug compute should return a trace");
    let semantic_trace = debug_toolpath
        .semantic_trace
        .as_ref()
        .expect("debug compute should return a semantic trace");
    let trace_path = debug_toolpath
        .debug_trace_path
        .clone()
        .expect("debug compute should write a trace artifact");
    assert!(
        trace_path.exists(),
        "expected trace artifact at {:?}",
        trace_path
    );
    assert!(trace.spans.iter().any(|span| span.kind == "core_generate"));
    assert!(trace.spans.iter().any(|span| span.kind == "dressups"));
    assert!(trace.spans.iter().any(|span| span.kind == "final_stats"));
    assert!(semantic_trace.summary.item_count > 0);
    assert!(
        semantic_trace.summary.move_linked_item_count > 0,
        "semantic trace should contain move-linked items"
    );

    let payload = std::fs::read_to_string(&trace_path).expect("read trace artifact");
    assert!(payload.contains("\"toolpath_name\": \"Pocket 41\""));
    assert!(payload.contains("\"semantic_trace\""));
    std::fs::remove_file(&trace_path).ok();

    let plain_result = super::execute::run_compute(&build_request(pocket_spec(42)))
        .result
        .expect("plain compute should succeed");
    // WP11b behaviour change: `execute_job` builds both recorders on every
    // generation, because the CLI reads both traces off every result. What
    // `debug_options` gates is the per-dressup ITEM set and the artifact
    // FILE, which is what an operator asks for when they tick the box.
    assert!(plain_result.debug_trace.is_some());
    assert!(plain_result.semantic_trace.is_some());
    assert!(plain_result.debug_trace_path.is_none());
    assert_toolpaths_match(debug_toolpath.toolpath(), plain_result.toolpath());
    assert_eq!(
        debug_toolpath.stats.move_count,
        plain_result.stats.move_count
    );
    assert!(
        (debug_toolpath.stats.cutting_distance - plain_result.stats.cutting_distance).abs() < 1e-9
    );
    assert!((debug_toolpath.stats.rapid_distance - plain_result.stats.rapid_distance).abs() < 1e-9);
}

#[test]
fn debug_trace_records_arc_fit_and_feed_optimization_phases() {
    let mut spec = pocket_spec(77).with_debug_trace();
    spec.name = "Dressup phases".to_owned();
    spec.dressups.arc_fitting = true;
    spec.dressups.feed_optimization = true;
    let request = build_request(spec);

    let result = super::execute::run_compute(&request)
        .result
        .expect("debug compute should succeed");
    let trace = result
        .debug_trace
        .as_ref()
        .expect("debug compute should return a trace");

    assert!(trace.spans.iter().any(|span| span.kind == "arc_fit"));
    assert!(
        trace
            .spans
            .iter()
            .any(|span| span.kind == "feed_optimization")
    );

    if let Some(path) = result.debug_trace_path.as_ref() {
        std::fs::remove_file(path).ok();
    }
}

#[test]
fn debug_trace_records_dropcutter_prepare_and_rasterize_phases() {
    let request = build_request(heavy_dropcutter_spec(78).with_debug_trace());

    let result = super::execute::run_compute(&request)
        .result
        .expect("dropcutter debug compute should succeed");
    let trace = result
        .debug_trace
        .as_ref()
        .expect("dropcutter debug compute should return a trace");

    // After Phase 3A (dispatch via core), the debug trace records
    // core_generate and dressups spans rather than operation-internal spans.
    assert!(
        trace.spans.iter().any(|span| span.kind == "core_generate"),
        "expected core_generate span in debug trace"
    );

    if let Some(path) = result.debug_trace_path.as_ref() {
        std::fs::remove_file(path).ok();
    }
}

#[test]
fn debug_trace_records_waterline_prepare_and_slice_phases() {
    let request = build_request(waterline_spec(79).with_debug_trace());

    let result = super::execute::run_compute(&request)
        .result
        .expect("waterline debug compute should succeed");
    let trace = result
        .debug_trace
        .as_ref()
        .expect("waterline debug compute should return a trace");

    // After Phase 3A (dispatch via core), the debug trace records
    // core_generate and dressups spans rather than operation-internal spans.
    assert!(
        trace.spans.iter().any(|span| span.kind == "core_generate"),
        "expected core_generate span in debug trace"
    );

    if let Some(path) = result.debug_trace_path.as_ref() {
        std::fs::remove_file(path).ok();
    }
}

/// A cancelled generation reports `Cancelled` and carries NO partial trace.
///
/// WP11b behaviour change. The viz worker used to finish its recorders on
/// the error path and hand back what they had. `execute_job` returns the
/// traces WITH a result, so a refusal returns neither — `SessionError`
/// carries no trace slot. The verdict itself is unchanged, and that is what
/// the lane and the drain read.
#[test]
fn cancelled_toolpath_reports_cancelled_and_no_partial_trace() {
    let mut backend = ThreadedComputeBackend::new();
    let request = build_request(heavy_dropcutter_spec(88).with_debug_trace());
    backend.submit_toolpath(request);
    thread::sleep(Duration::from_millis(20));
    backend.cancel_lane(ComputeLane::Toolpath);

    let cancelled = wait_for(&mut backend, Duration::from_secs(5), |message| {
        matches!(
            message,
            ComputeMessage::Toolpath(result)
                if matches!(
                    **result,
                    ComputeResult {
                        toolpath_id: ToolpathId(88),
                        result: Err(ComputeError::Cancelled),
                        ..
                    }
                )
        )
    });
    let cancelled = match cancelled {
        Some(ComputeMessage::Toolpath(result)) => result,
        Some(_) => panic!("expected toolpath result"),
        None => panic!("expected cancelled toolpath result"),
    };
    assert!(
        cancelled.debug_trace.is_none(),
        "a refused generation returns no trace: execute_job returns the \
         traces with a result"
    );
    assert!(
        cancelled.semantic_trace.is_none(),
        "a refused generation returns no semantic trace either"
    );
    assert!(
        cancelled.debug_trace_path.is_none(),
        "no trace means no artifact file"
    );
}

#[test]
fn semantic_trace_records_entry_params_and_boundary_clip() {
    let mut spec = pocket_spec(90).with_debug_trace();
    spec.boundary.enabled = true;
    // A board SMALLER than the 40 x 40 polygon, so the stock-rectangle
    // boundary really clips.
    spec.stock = board(10.0, 15.0);
    spec.polygons = Some(vec![Polygon2::rectangle(-20.0, -20.0, 20.0, 20.0)]);
    spec.dressups.entry_style = crate::state::toolpath::DressupEntryStyle::Helix;
    spec.dressups.lead_in_out = true;
    spec.dressups.link_moves = true;
    spec.dressups.arc_fitting = true;
    spec.dressups.optimize_rapid_order = true;
    let request = build_request(spec);

    let result = super::execute::run_compute(&request)
        .result
        .expect("semantic debug compute should succeed");
    let semantic_trace = result
        .semantic_trace
        .as_ref()
        .expect("semantic trace should be attached");

    let helix = semantic_trace
        .items
        .iter()
        .find(|item| {
            item.kind == rs_cam_core::trace::semantic_trace::ToolpathSemanticKind::Entry
                && item.label == "Helix entry"
        })
        .expect("helix entry item should be present");
    assert_eq!(
        helix
            .params
            .get(rs_cam_core::trace::semantic_trace::SemanticKey::Radius),
        Some(&serde_json::json!(DressupConfig::default().helix_radius))
    );
    assert_eq!(
        helix
            .params
            .get(rs_cam_core::trace::semantic_trace::SemanticKey::Pitch),
        Some(&serde_json::json!(DressupConfig::default().helix_pitch))
    );

    let boundary_clip = semantic_trace
        .items
        .iter()
        .find(|item| {
            item.kind == rs_cam_core::trace::semantic_trace::ToolpathSemanticKind::BoundaryClip
        })
        .expect("boundary clip item should be present");
    assert_eq!(
        boundary_clip
            .params
            .get(rs_cam_core::trace::semantic_trace::SemanticKey::Containment),
        Some(&serde_json::json!("center"))
    );
    assert!(
        semantic_trace
            .items
            .iter()
            .any(|item| item.move_start.is_some() && item.move_end.is_some()),
        "expected move-linked semantic items"
    );

    if let Some(path) = result.debug_trace_path.as_ref() {
        std::fs::remove_file(path).ok();
    }
}

/// P2.2/P2.3 regression (rest-cascade live repro, bug (b)): a
/// `DerivedRestRegions` boundary whose source toolpath's rest analysis
/// produced multiple disjoint islands must clip against the FULL region
/// set via `apply_boundary_clip_multi`/`clip_toolpath_to_boundary_set_with_provenance`,
/// not silently degrade to the stock rectangle because the regions don't
/// union down to a single polygon (genuine terrain rest analysis commonly
/// yields many disjoint islands — the old worker enforcement clip only
/// consulted a pre-unioned single-polygon field and fell back to
/// `stock_rect()` whenever the union wasn't exactly one polygon).
#[test]
fn derived_rest_regions_boundary_clips_to_all_disjoint_regions() {
    let mut spec = pocket_spec(200);
    spec.boundary.enabled = true;
    // Two small islands near opposite corners of the -20..20 pocket, with a
    // large untouched gap between them (e.g. around the origin) that a
    // single-polygon-union fallback to the full stock/pocket rectangle
    // would have cut through.
    // WP11b: the regions come from a SOURCE toolpath's cached result now,
    // because that is where a `DerivedRestRegions` boundary reads them. The
    // fixture seeds one and points the boundary at it.
    spec.rest_source_regions = Some(vec![
        Polygon2::rectangle(-18.0, -18.0, -8.0, -8.0),
        Polygon2::rectangle(8.0, 8.0, 18.0, 18.0),
    ]);
    let request = build_request(spec);

    let result = super::execute::run_compute(&request)
        .result
        .expect("derived-rest-regions boundary compute should succeed");

    let in_region_a = |x: f64, y: f64| (-18.5..=-7.5).contains(&x) && (-18.5..=-7.5).contains(&y);
    let in_region_b = |x: f64, y: f64| (7.5..=18.5).contains(&x) && (7.5..=18.5).contains(&y);
    let mut saw_cut_in_gap = false;
    let mut saw_cut_at_all = false;
    for mv in &result.annotated.toolpath.moves {
        if matches!(mv.move_type, rs_cam_core::toolpath::MoveType::Linear { .. }) {
            saw_cut_at_all = true;
            let (x, y) = (mv.target.x, mv.target.y);
            if !in_region_a(x, y) && !in_region_b(x, y) {
                saw_cut_in_gap = true;
            }
        }
    }
    assert!(saw_cut_at_all, "expected at least some cutting moves");
    assert!(
        !saw_cut_in_gap,
        "cutting moves must stay within the two derived rest regions, not the gap between \
         them (a single-polygon-union fallback would cut the whole pocket rectangle)"
    );
}

/// P2.4 regression: the GUI worker's `generate_via_core` bridge must thread
/// `derived_rest_regions` into `execute_operation_annotated`
/// (not just the post-generation enforcement clip), the same way
/// `ProjectSession::generate_toolpath` does on the core session path. Before
/// this fix, `generate_via_core` always called the plain
/// `execute_operation_annotated` wrapper (`boundary_regions = None`), so
/// scallop generated concentric rings over the WHOLE hemisphere and only the
/// post-generation clip trimmed the result down to the two islands — same
/// final containment as the sibling test above proves, but full-part
/// generation cost. Confining generation itself to two small islands (a
/// tiny fraction of the hemisphere's footprint) must produce a materially
/// smaller toolpath than generating over the whole hemisphere, not just a
/// clipped-down copy of the full-part path.
#[test]
fn derived_rest_regions_boundary_shrinks_generation_not_just_clips_it() {
    // Baseline: no boundary at all — scallop covers the whole hemisphere
    // footprint (~pi * 20^2 ~= 1257 sq mm).
    let baseline_request = build_request(scallop_spec(300));
    let baseline = super::execute::run_compute(&baseline_request)
        .result
        .expect("baseline scallop compute should succeed");
    let baseline_moves = baseline.annotated.toolpath.moves.len();

    // Bounded: two small 6x6 islands (72 sq mm total) in opposite quadrants,
    // well inside the hemisphere's footprint, with a large untouched gap
    // between them.
    let mut bounded_spec = scallop_spec(301);
    bounded_spec.boundary.enabled = true;
    bounded_spec.rest_source_regions = Some(vec![
        Polygon2::rectangle(-15.0, -15.0, -9.0, -9.0),
        Polygon2::rectangle(9.0, 9.0, 15.0, 15.0),
    ]);
    let bounded_request = build_request(bounded_spec);
    let bounded = super::execute::run_compute(&bounded_request)
        .result
        .expect("derived-rest-regions scallop compute should succeed");
    let bounded_moves = bounded.annotated.toolpath.moves.len();

    // Post-clip containment still holds (mirrors the sibling disjoint-region
    // test above): every cutting move stays within the two islands.
    let in_region_a = |x: f64, y: f64| (-15.5..=-8.5).contains(&x) && (-15.5..=-8.5).contains(&y);
    let in_region_b = |x: f64, y: f64| (8.5..=15.5).contains(&x) && (8.5..=15.5).contains(&y);
    let mut saw_cut_at_all = false;
    let mut saw_cut_outside_regions = false;
    for mv in &bounded.annotated.toolpath.moves {
        if matches!(mv.move_type, rs_cam_core::toolpath::MoveType::Linear { .. }) {
            saw_cut_at_all = true;
            let (x, y) = (mv.target.x, mv.target.y);
            if !in_region_a(x, y) && !in_region_b(x, y) {
                saw_cut_outside_regions = true;
            }
        }
    }
    assert!(saw_cut_at_all, "expected at least some cutting moves");
    assert!(
        !saw_cut_outside_regions,
        "cutting moves must stay within the two derived rest regions"
    );

    // The real regression check: generation itself must have skipped the
    // area outside the two islands, not merely clipped a full-hemisphere
    // toolpath down after the fact. A post-hoc-only clip keeps (or grows,
    // via crossing rapids) the total move count relative to the unbounded
    // baseline; a generation-level pre-clip produces a toolpath an order of
    // magnitude smaller because scallop never rings the empty gap at all.
    assert!(
        bounded_moves < baseline_moves,
        "boundary-confined generation ({bounded_moves} moves) must produce fewer moves than \
         unbounded full-hemisphere generation ({baseline_moves} moves) — otherwise generation \
         is scanning the whole part and only the post-generation clip is trimming it"
    );
    assert!(
        bounded_moves * 4 < baseline_moves,
        "boundary-confined generation ({bounded_moves} moves) should be a small fraction of \
         the unbounded baseline ({baseline_moves} moves), matching the ~17x area reduction \
         (72 sq mm of islands vs ~1257 sq mm of hemisphere footprint) — a small reduction would \
         suggest the pre-clip isn't actually reaching the generator"
    );
}

/// P2.5 end-to-end: a `derived_rest_regions` boundary source no longer has
/// to come from a pencil `RestDepth` toolpath — ANY op with `rest_analysis`
/// enabled produces the same `rest_regions` shape, and a downstream
/// toolpath boundary-sources off it exactly the same way. Generates a
/// Scallop toolpath with generic rest analysis enabled, takes the
/// `rest_regions` it produced, and feeds them into a second toolpath's
/// `DerivedRestRegions` boundary — the consumer side (`apply_boundary_clip_multi`
/// / `clip_toolpath_to_boundary_set_with_provenance`) never even sees which
/// op produced the regions, but this proves the *producer* side (a
/// non-pencil op) genuinely emits a usable set, not just an empty `Some(vec![])`.
#[test]
fn non_pencil_rest_analysis_source_feeds_a_downstream_boundary() {
    let mut source_spec = scallop_spec(400);
    source_spec.rest_analysis = crate::state::toolpath::RestAnalysisConfig {
        enabled: true,
        reference_tool_id: None,
        cell_mm: 1.0,
        min_valley_depth: 0.05,
        region_margin_mm: 0.5,
        ..Default::default()
    };
    let source_request = build_request(source_spec);
    let source_result = super::execute::run_compute(&source_request)
        .result
        .expect("scallop with rest_analysis enabled should succeed");
    let source_regions = std::sync::Arc::clone(
        source_result
            .annotated
            .rest_regions
            .as_ref()
            .expect("non-pencil op with rest_analysis enabled should produce rest_regions"),
    );
    assert!(
        !source_regions.is_empty(),
        "a hemisphere mesh should yield at least one non-trivial rest region"
    );

    // Feed those exact regions into a downstream toolpath's boundary, same
    // shape a controller would resolve from `source_result.annotated.rest_regions`.
    let mut downstream_spec = pocket_spec(401);
    downstream_spec.boundary.enabled = true;
    downstream_spec.rest_source_regions = Some((*source_regions).clone());
    let downstream_request = build_request(downstream_spec);

    let downstream = super::execute::run_compute(&downstream_request)
        .result
        .expect("downstream toolpath sourcing a non-pencil rest-regions boundary should succeed");
    assert!(
        !downstream.annotated.toolpath.moves.is_empty(),
        "downstream toolpath should still produce moves when confined to the scallop-sourced \
         rest regions"
    );
}

#[test]
fn adaptive3d_semantic_trace_records_runtime_structure() {
    // After Phase 3A the per-pass / planning / shelf-detection annotations
    // emitted by the old viz-side adaptive3d wrapper are no longer produced;
    // dispatch goes through `core::execute_operation`, which leaves
    // adaptive3d with the generic span-derived `DepthLevel` items emitted by
    // `compute/annotate.rs`. Verify the trace exists and at least one
    // DepthLevel item is attached.
    let request = build_request(adaptive3d_spec(91).with_debug_trace());

    let result = super::execute::run_compute(&request)
        .result
        .expect("adaptive3d debug compute should succeed");
    let semantic_trace = result
        .semantic_trace
        .as_ref()
        .expect("semantic trace should be attached");

    assert!(
        semantic_trace
            .items
            .iter()
            .any(|item| item.kind
                == rs_cam_core::trace::semantic_trace::ToolpathSemanticKind::Operation),
        "expected top-level Operation scope"
    );
    assert!(
        semantic_trace.items.iter().any(|item| item.kind
            == rs_cam_core::trace::semantic_trace::ToolpathSemanticKind::DepthLevel),
        "expected depth-level semantics from generic span annotation"
    );

    if let Some(path) = result.debug_trace_path.as_ref() {
        std::fs::remove_file(path).ok();
    }
}

#[test]
fn adaptive_semantic_trace_records_runtime_structure() {
    // After Phase 3A, detailed semantic annotations (slot-clearing, cleanup,
    // passes) are no longer produced by the viz layer.  Verify the operation
    // succeeds and a top-level semantic trace is attached.
    let request = build_request(adaptive_spec(92).with_debug_trace());

    let result = super::execute::run_compute(&request)
        .result
        .expect("adaptive debug compute should succeed");
    let semantic_trace = result
        .semantic_trace
        .as_ref()
        .expect("semantic trace should be attached");

    assert!(
        semantic_trace
            .items
            .iter()
            .any(|item| item.kind
                == rs_cam_core::trace::semantic_trace::ToolpathSemanticKind::Operation),
        "expected top-level Operation scope"
    );
}

#[test]
fn profile_semantic_trace_records_depth_and_finish_structure() {
    // After Phase 3A, detailed depth-level/finish-pass annotations are no
    // longer produced.  Verify the operation succeeds with a semantic trace.
    let request = build_request(profile_spec(93).with_debug_trace());

    let result = super::execute::run_compute(&request)
        .result
        .expect("profile debug compute should succeed");
    let semantic_trace = result
        .semantic_trace
        .as_ref()
        .expect("semantic trace should be attached");

    assert!(
        semantic_trace
            .items
            .iter()
            .any(|item| item.kind
                == rs_cam_core::trace::semantic_trace::ToolpathSemanticKind::Operation),
        "expected top-level Operation scope"
    );
}

#[test]
fn drill_semantic_trace_records_cycle_children() {
    let request = build_request(drill_spec(94).with_debug_trace());

    let result = super::execute::run_compute(&request)
        .result
        .expect("drill debug compute should succeed");
    let semantic_trace = result
        .semantic_trace
        .as_ref()
        .expect("semantic trace should be attached");

    assert!(
        semantic_trace
            .items
            .iter()
            .any(|item| item.kind == rs_cam_core::trace::semantic_trace::ToolpathSemanticKind::Hole),
        "expected drill Hole semantic item"
    );
    assert!(
        semantic_trace.items.iter().any(
            |item| item.kind == rs_cam_core::trace::semantic_trace::ToolpathSemanticKind::Cycle
        ),
        "expected drill Cycle semantic item"
    );
}

#[test]
fn steep_shallow_semantic_trace_splits_steep_and_shallow_regions() {
    let request = build_request(steep_shallow_spec(95).with_debug_trace());

    let result = super::execute::run_compute(&request)
        .result
        .expect("steep/shallow debug compute should succeed");
    let semantic_trace = result
        .semantic_trace
        .as_ref()
        .expect("semantic trace should be attached");

    assert!(
        semantic_trace
            .items
            .iter()
            .any(|item| item.kind
                == rs_cam_core::trace::semantic_trace::ToolpathSemanticKind::Operation),
        "expected top-level Operation scope"
    );
}

#[test]
fn pencil_semantic_trace_records_chain_and_offset_pass_structure() {
    let request = build_request(pencil_spec(96).with_debug_trace());

    let result = super::execute::run_compute(&request)
        .result
        .expect("pencil debug compute should succeed");
    let semantic_trace = result
        .semantic_trace
        .as_ref()
        .expect("semantic trace should be attached");

    assert!(
        semantic_trace
            .items
            .iter()
            .any(|item| item.kind
                == rs_cam_core::trace::semantic_trace::ToolpathSemanticKind::Operation),
        "expected top-level Operation scope"
    );
}

#[test]
fn scallop_semantic_trace_records_band_and_ring_structure() {
    let request = build_request(scallop_spec(97).with_debug_trace());

    let result = super::execute::run_compute(&request)
        .result
        .expect("scallop debug compute should succeed");
    let semantic_trace = result
        .semantic_trace
        .as_ref()
        .expect("semantic trace should be attached");

    assert!(
        semantic_trace
            .items
            .iter()
            .any(|item| item.kind
                == rs_cam_core::trace::semantic_trace::ToolpathSemanticKind::Operation),
        "expected top-level Operation scope"
    );
}

#[test]
fn ramp_finish_semantic_trace_records_terrace_and_ramp_structure() {
    let request = build_request(ramp_finish_spec(98).with_debug_trace());

    let result = super::execute::run_compute(&request)
        .result
        .expect("ramp finish debug compute should succeed");
    let semantic_trace = result
        .semantic_trace
        .as_ref()
        .expect("semantic trace should be attached");

    assert!(
        semantic_trace
            .items
            .iter()
            .any(|item| item.kind
                == rs_cam_core::trace::semantic_trace::ToolpathSemanticKind::Operation),
        "expected top-level Operation scope"
    );
}

#[test]
fn spiral_finish_semantic_trace_records_band_and_ring_structure() {
    let request = build_request(spiral_finish_spec(99).with_debug_trace());

    let result = super::execute::run_compute(&request)
        .result
        .expect("spiral finish debug compute should succeed");
    let semantic_trace = result
        .semantic_trace
        .as_ref()
        .expect("semantic trace should be attached");

    assert!(
        semantic_trace
            .items
            .iter()
            .any(|item| item.kind
                == rs_cam_core::trace::semantic_trace::ToolpathSemanticKind::Operation),
        "expected top-level Operation scope"
    );
}

#[test]
fn radial_finish_semantic_trace_records_ray_angles() {
    let request = build_request(radial_finish_spec(100).with_debug_trace());

    let result = super::execute::run_compute(&request)
        .result
        .expect("radial finish debug compute should succeed");
    let semantic_trace = result
        .semantic_trace
        .as_ref()
        .expect("semantic trace should be attached");

    assert!(
        semantic_trace
            .items
            .iter()
            .any(|item| item.kind
                == rs_cam_core::trace::semantic_trace::ToolpathSemanticKind::Operation),
        "expected top-level Operation scope"
    );
}

#[test]
fn horizontal_finish_semantic_trace_records_slice_passes() {
    let request = build_request(horizontal_finish_spec(101).with_debug_trace());

    let result = super::execute::run_compute(&request)
        .result
        .expect("horizontal finish debug compute should succeed");
    let semantic_trace = result
        .semantic_trace
        .as_ref()
        .expect("semantic trace should be attached");

    assert!(
        semantic_trace
            .items
            .iter()
            .any(|item| item.kind
                == rs_cam_core::trace::semantic_trace::ToolpathSemanticKind::Operation),
        "expected top-level Operation scope"
    );
}

#[test]
fn project_curve_semantic_trace_records_source_curve_groups() {
    let request = build_request(project_curve_spec(102).with_debug_trace());

    let result = super::execute::run_compute(&request)
        .result
        .expect("project curve debug compute should succeed");
    let semantic_trace = result
        .semantic_trace
        .as_ref()
        .expect("semantic trace should be attached");

    assert!(
        semantic_trace
            .items
            .iter()
            .any(|item| item.kind
                == rs_cam_core::trace::semantic_trace::ToolpathSemanticKind::Operation),
        "expected top-level Operation scope"
    );
}

#[test]
fn running_lane_snapshot_reports_current_phase() {
    let mut backend = ThreadedComputeBackend::new();
    backend.submit_toolpath(build_request(heavy_dropcutter_spec(89)));

    let start = Instant::now();
    let mut saw_phase = false;
    while start.elapsed() < Duration::from_secs(2) && !saw_phase {
        let snapshot = backend.lane_snapshot(ComputeLane::Toolpath);
        saw_phase = snapshot.state == LaneState::Running && snapshot.current_phase.is_some();
        if !saw_phase {
            thread::sleep(Duration::from_millis(10));
        }
    }

    assert!(
        saw_phase,
        "expected running lane snapshot to expose current_phase"
    );
}

#[test]
fn analysis_lane_snapshot_reports_current_phase() {
    let mut backend = ThreadedComputeBackend::new();
    backend.submit_simulation(long_simulation_request());

    let start = Instant::now();
    let mut saw_phase = false;
    while start.elapsed() < Duration::from_secs(2) && !saw_phase {
        let snapshot = backend.lane_snapshot(ComputeLane::Analysis);
        saw_phase = snapshot.state == LaneState::Running && snapshot.current_phase.is_some();
        if !saw_phase {
            thread::sleep(Duration::from_millis(10));
        }
    }

    assert!(
        saw_phase,
        "expected analysis lane snapshot to expose current_phase"
    );
}

#[test]
fn analysis_cancel_completes_quickly() {
    let mut backend = ThreadedComputeBackend::new();
    backend.submit_simulation(long_simulation_request());
    thread::sleep(Duration::from_millis(20));

    let start = Instant::now();
    backend.cancel_lane(ComputeLane::Analysis);
    let result = wait_for(&mut backend, Duration::from_secs(5), |message| {
        matches!(
            message,
            ComputeMessage::Simulation(Err(ComputeError::Cancelled))
        )
    });
    assert!(result.is_some(), "expected cancelled simulation result");
    assert!(
        start.elapsed() < Duration::from_millis(250),
        "analysis cancel exceeded 250 ms: {:?}",
        start.elapsed()
    );
}

#[test]
fn toolpath_and_analysis_lanes_run_independently() {
    let mut backend = ThreadedComputeBackend::new();
    backend.submit_simulation(long_simulation_request());
    thread::sleep(Duration::from_millis(20));
    backend.submit_toolpath(build_request(pocket_spec(7)));

    let result = wait_for(&mut backend, Duration::from_secs(5), |message| {
        matches!(
            message,
            ComputeMessage::Toolpath(result)
                if matches!(
                    **result,
                    ComputeResult {
                        toolpath_id: ToolpathId(7),
                        result: Ok(_),
                        ..
                    }
                )
        )
    });
    assert!(
        result.is_some(),
        "expected toolpath result while simulation was active"
    );
    assert!(backend.lane_snapshot(ComputeLane::Analysis).is_active());
}

#[test]
fn duplicate_queued_toolpaths_are_coalesced() {
    let mut backend = ThreadedComputeBackend::new();
    backend.submit_toolpath(build_request(heavy_dropcutter_spec(1)));
    thread::sleep(Duration::from_millis(20));
    backend.submit_toolpath(build_request(pocket_spec(2)));
    backend.submit_toolpath(build_request(pocket_spec(2)));

    let snapshot = backend.lane_snapshot(ComputeLane::Toolpath);
    assert!(matches!(
        snapshot.state,
        LaneState::Running | LaneState::Cancelling
    ));
    assert_eq!(
        snapshot.queue_depth, 1,
        "duplicate queued toolpath should be coalesced"
    );
}

#[test]
fn resubmitting_active_toolpath_cancels_and_replaces_it() {
    let mut backend = ThreadedComputeBackend::new();
    backend.submit_toolpath(build_request(heavy_dropcutter_spec(3)));
    thread::sleep(Duration::from_millis(20));
    backend.submit_toolpath(build_request(pocket_spec(3)));

    let snapshot = backend.lane_snapshot(ComputeLane::Toolpath);
    assert_eq!(snapshot.queue_depth, 1);
    assert_eq!(
        snapshot.current_job.as_deref(),
        Some("DropCutter 3 (3D Finish)")
    );
    assert_eq!(snapshot.state, LaneState::Cancelling);

    let start = Instant::now();
    let mut saw_cancelled = false;
    let mut saw_replacement = false;
    while start.elapsed() < Duration::from_secs(5) && !(saw_cancelled && saw_replacement) {
        for message in backend.drain_results() {
            match message {
                ComputeMessage::Toolpath(result)
                    if matches!(
                        *result,
                        ComputeResult {
                            toolpath_id: ToolpathId(3),
                            result: Err(ComputeError::Cancelled),
                            ..
                        }
                    ) =>
                {
                    saw_cancelled = true;
                }
                ComputeMessage::Toolpath(result)
                    if matches!(
                        *result,
                        ComputeResult {
                            toolpath_id: ToolpathId(3),
                            result: Ok(_),
                            ..
                        }
                    ) =>
                {
                    saw_replacement = true;
                }
                _ => {}
            }
        }
        if !(saw_cancelled && saw_replacement) {
            thread::sleep(Duration::from_millis(10));
        }
    }

    assert!(saw_cancelled, "expected active toolpath cancellation");
    assert!(saw_replacement, "expected replacement toolpath result");
}

#[test]
fn analysis_requests_replace_stale_work() {
    let mut backend = ThreadedComputeBackend::new();
    backend.submit_simulation(long_simulation_request());
    thread::sleep(Duration::from_millis(20));
    backend.submit_collision(CollisionRequest {
        annotated: Arc::new(rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
            {
                let mut toolpath = Toolpath::new();
                toolpath.rapid_to(P3::new(0.0, 0.0, 5.0));
                toolpath.feed_to(P3::new(0.0, 0.0, -1.0), 300.0);
                toolpath
            },
        )),
        tool: ToolConfig::new_default(ToolId(1), ToolType::EndMill),
        mesh: Arc::new(make_test_flat(20.0)),
        obstacles: Vec::new(),
    });

    let snapshot = backend.lane_snapshot(ComputeLane::Analysis);
    assert_eq!(snapshot.state, LaneState::Cancelling);
    assert_eq!(snapshot.queue_depth, 1);

    let start = Instant::now();
    let mut saw_cancelled = false;
    let mut saw_collision = false;
    while start.elapsed() < Duration::from_secs(5) && !(saw_cancelled && saw_collision) {
        for message in backend.drain_results() {
            match message {
                ComputeMessage::Simulation(Err(ComputeError::Cancelled)) => {
                    saw_cancelled = true;
                }
                ComputeMessage::Collision(Ok(_)) => {
                    saw_collision = true;
                }
                _ => {}
            }
        }
        if !(saw_cancelled && saw_collision) {
            thread::sleep(Duration::from_millis(10));
        }
    }

    assert!(saw_cancelled, "expected stale simulation cancellation");
    assert!(saw_collision, "expected replacement collision result");
}

#[test]
fn cancel_all_marks_both_lanes_cancelling() {
    let mut backend = ThreadedComputeBackend::new();
    backend.submit_toolpath(build_request(heavy_dropcutter_spec(4)));
    backend.submit_simulation(long_simulation_request());
    thread::sleep(Duration::from_millis(20));

    backend.cancel_all();

    assert_eq!(
        backend.lane_snapshot(ComputeLane::Toolpath).state,
        LaneState::Cancelling
    );
    assert_eq!(
        backend.lane_snapshot(ComputeLane::Analysis).state,
        LaneState::Cancelling
    );
}

/// Multi-setup simulation: top cuts remove material from above, bottom cuts
/// (after coordinate transform) remove material from below. Verifies that a
/// single TriDexelStock carries forward between setups.
#[test]
fn multi_setup_top_bottom_simulation() {
    use rs_cam_core::dexel_stock::StockCutDirection;

    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    // Tool diameter is 6.35mm (default endmill)

    // Stock: 50x50x20 at origin
    let stock_bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(50.0, 50.0, 20.0),
    };

    // --- Top setup: cut a pocket from above ---
    let mut top_tp = Toolpath::new();
    top_tp.rapid_to(P3::new(25.0, 25.0, 25.0));
    // Cut at Z=15 (5mm depth from top surface at Z=20)
    for i in 0..20 {
        let x = 20.0 + (i as f64) * 0.5;
        top_tp.feed_to(P3::new(x, 25.0, 15.0), 600.0);
    }

    // --- Bottom setup: toolpath in SETUP-LOCAL frame ---
    // In the restored architecture, the bottom setup's toolpaths are in
    // setup-local coordinates (always cutting from Z-axis top down).
    // SetupTransformInfo handles the local-to-global transform.
    // Local (25, 25, 15) cuts 5mm from top in the flipped frame.
    let mut bottom_tp = Toolpath::new();
    bottom_tp.rapid_to(P3::new(25.0, 25.0, 25.0));
    for i in 0..20 {
        let x = 20.0 + (i as f64) * 0.5;
        bottom_tp.feed_to(P3::new(x, 25.0, 15.0), 600.0);
    }

    let request = SimulationRequest {
        core: rs_cam_core::compute::simulate::SimulationRequest {
            groups: vec![
                SimGroupEntry {
                    toolpaths: vec![SimToolpathEntry {
                        id: ToolpathId(1),
                        name: "Top Cut".to_owned(),
                        annotated: Arc::new(
                            rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(top_tp),
                        ),
                        tool: Arc::new(build_cutter(&tool)),
                        flute_count: tool.flute_count,
                        tool_summary: tool.summary(),
                        semantic_trace: None,
                        spindle_rpm: None,
                        metrics_not_applicable: false,
                        drill_op: None,
                        operation_config_hash: 0,
                    }],
                    direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                    local_stock_bbox: None,
                    local_to_global: None,
                    phantom_prior_stock: None,
                },
                SimGroupEntry {
                    toolpaths: vec![SimToolpathEntry {
                        id: ToolpathId(2),
                        name: "Bottom Cut".to_owned(),
                        annotated: Arc::new(
                            rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(bottom_tp),
                        ),
                        tool: Arc::new(build_cutter(&tool)),
                        flute_count: tool.flute_count,
                        tool_summary: tool.summary(),
                        semantic_trace: None,
                        spindle_rpm: None,
                        metrics_not_applicable: false,
                        drill_op: None,
                        operation_config_hash: 0,
                    }],
                    direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                    local_stock_bbox: Some(stock_bbox),
                    local_to_global: Some(SetupTransformInfo {
                        face_up: crate::state::job::FaceUp::Bottom,
                        z_rotation: crate::state::job::ZRotation::Deg0,
                        stock_x: 50.0,
                        stock_y: 50.0,
                        stock_z: 20.0,
                        ..Default::default()
                    }),
                    phantom_prior_stock: None,
                },
            ],
            stock_bbox,
            stock_top_z: 20.0,
            resolution: 0.5,
            metric_options: rs_cam_core::stock::simulation_cut::SimulationMetricOptions::default(),
            spindle_rpm: 18_000,
            rapid_feed_mm_min: 5_000.0,
            model_mesh: None,
            kinematics: None,
        },
        memoize_prefix: false,
    };

    let mut backend = ThreadedComputeBackend::new();
    backend.submit_simulation(request);

    let result = wait_for(&mut backend, Duration::from_secs(10), |msg| {
        matches!(msg, ComputeMessage::Simulation(Ok(_)))
    });
    let ComputeMessage::Simulation(Ok(result)) = result.unwrap() else {
        panic!("expected simulation result");
    };

    // Should have 2 boundaries (one per toolpath)
    assert_eq!(result.core.boundaries.len(), 2);
    assert_eq!(
        result.core.boundaries[0].direction,
        StockCutDirection::FromTop
    );
    assert_eq!(
        result.core.boundaries[1].direction,
        StockCutDirection::FromBottom
    );

    // Should have 2 checkpoints (one per setup)
    assert_eq!(result.core.checkpoints.len(), 2);

    // Should have playback data for both toolpaths
    assert_eq!(result.playback_data.len(), 2);

    // Checkpoints store GLOBAL-frame stocks (for playback).
    // checkpoint[0] = after top-setup: global stock with top cut at Z=15
    let after_top = &result.core.checkpoints[0].stock;
    let (r, c) = after_top.z_grid.world_to_cell(25.0, 25.0).unwrap();
    let ray = after_top.z_grid.ray(r, c);
    assert_eq!(ray.len(), 1, "after top cut: one segment");
    assert!(ray[0].enter.abs() < 0.01, "material starts at Z=0");
    assert!(
        (ray[0].exit - 15.0).abs() < 1.0,
        "top cut ends near Z=15, got {}",
        ray[0].exit
    );

    // checkpoint[1] = after both setups: global stock with top + bottom cuts
    // Top cut at Z=15 (from top), bottom cut at Z=5 (from bottom).
    // Remaining material: Z=5 to Z=15.
    let after_both = &result.core.checkpoints[1].stock;
    let (r, c) = after_both.z_grid.world_to_cell(25.0, 25.0).unwrap();
    let ray = after_both.z_grid.ray(r, c);
    assert_eq!(ray.len(), 1, "after both cuts: one segment");
    assert!(
        (ray[0].enter - 5.0).abs() < 1.0,
        "bottom cut leaves material starting near Z=5, got {}",
        ray[0].enter
    );
    assert!(
        (ray[0].exit - 15.0).abs() < 1.0,
        "top cut leaves material ending near Z=15, got {}",
        ray[0].exit
    );

    // Final composited mesh should have non-zero vertex data
    assert!(
        !result.core.mesh.vertices.is_empty(),
        "composited mesh should not be empty"
    );
}

/// Verify backward scrub across setup boundaries uses checkpoints correctly.
#[test]
fn multi_setup_backward_scrub_uses_checkpoints() {
    use rs_cam_core::dexel_stock::StockCutDirection;

    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    let stock_bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(30.0, 30.0, 10.0),
    };

    // Two toolpaths in two setup groups (setup-local frame).
    let mut tp1 = Toolpath::new();
    tp1.rapid_to(P3::new(15.0, 15.0, 15.0));
    for i in 0..50 {
        tp1.feed_to(P3::new(10.0 + i as f64 * 0.2, 15.0, 7.0), 600.0);
    }

    // Bottom setup: toolpath in setup-local frame (always top-down).
    // Cuts at local Z=7 (3mm from top of 10mm stock).
    let mut tp2 = Toolpath::new();
    tp2.rapid_to(P3::new(15.0, 15.0, 15.0));
    for i in 0..50 {
        tp2.feed_to(P3::new(10.0 + i as f64 * 0.2, 15.0, 7.0), 600.0);
    }

    let request = SimulationRequest {
        core: rs_cam_core::compute::simulate::SimulationRequest {
            groups: vec![
                SimGroupEntry {
                    toolpaths: vec![SimToolpathEntry {
                        id: ToolpathId(1),
                        name: "Top".to_owned(),
                        annotated: Arc::new(
                            rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(tp1),
                        ),
                        tool: Arc::new(build_cutter(&tool)),
                        flute_count: tool.flute_count,
                        tool_summary: tool.summary(),
                        semantic_trace: None,
                        spindle_rpm: None,
                        metrics_not_applicable: false,
                        drill_op: None,
                        operation_config_hash: 0,
                    }],
                    direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                    local_stock_bbox: None,
                    local_to_global: None,
                    phantom_prior_stock: None,
                },
                SimGroupEntry {
                    toolpaths: vec![SimToolpathEntry {
                        id: ToolpathId(2),
                        name: "Bottom".to_owned(),
                        annotated: Arc::new(
                            rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(tp2),
                        ),
                        tool: Arc::new(build_cutter(&tool)),
                        flute_count: tool.flute_count,
                        tool_summary: tool.summary(),
                        semantic_trace: None,
                        spindle_rpm: None,
                        metrics_not_applicable: false,
                        drill_op: None,
                        operation_config_hash: 0,
                    }],
                    direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                    local_stock_bbox: Some(stock_bbox),
                    local_to_global: Some(SetupTransformInfo {
                        face_up: crate::state::job::FaceUp::Bottom,
                        z_rotation: crate::state::job::ZRotation::Deg0,
                        stock_x: 30.0,
                        stock_y: 30.0,
                        stock_z: 10.0,
                        ..Default::default()
                    }),
                    phantom_prior_stock: None,
                },
            ],
            stock_bbox,
            stock_top_z: 10.0,
            resolution: 0.5,
            metric_options: rs_cam_core::stock::simulation_cut::SimulationMetricOptions::default(),
            spindle_rpm: 18_000,
            rapid_feed_mm_min: 5_000.0,
            model_mesh: None,
            kinematics: None,
        },
        memoize_prefix: false,
    };

    let mut backend = ThreadedComputeBackend::new();
    backend.submit_simulation(request);

    let result = wait_for(&mut backend, Duration::from_secs(10), |msg| {
        matches!(msg, ComputeMessage::Simulation(Ok(_)))
    });
    let ComputeMessage::Simulation(Ok(result)) = result.unwrap() else {
        panic!("expected simulation result");
    };

    // Checkpoints store GLOBAL-frame stocks for playback.
    assert_eq!(result.core.checkpoints.len(), 2);

    // Checkpoint 0: global stock after top-setup cut at Z=7
    let cp0 = &result.core.checkpoints[0].stock;
    let (r, c) = cp0.z_grid.world_to_cell(15.0, 15.0).unwrap();
    let ray = cp0.z_grid.ray(r, c);
    assert_eq!(ray.len(), 1);
    assert!(ray[0].enter.abs() < 0.01, "material starts at Z=0");
    assert!(
        (ray[0].exit - 7.0).abs() < 1.0,
        "top cut ends near Z=7, got {}",
        ray[0].exit
    );

    // Checkpoint 1: global stock after both cuts (top at Z=7, bottom at Z=3)
    let cp1 = &result.core.checkpoints[1].stock;
    let (r, c) = cp1.z_grid.world_to_cell(15.0, 15.0).unwrap();
    let ray = cp1.z_grid.ray(r, c);
    assert_eq!(ray.len(), 1);
    assert!(
        ray[0].enter > 2.0,
        "after both cuts, material bottom raised above Z=2, got {}",
        ray[0].enter
    );

    // Boundary directions are correct
    assert_eq!(
        result.core.boundaries[0].direction,
        StockCutDirection::FromTop
    );
    assert_eq!(
        result.core.boundaries[1].direction,
        StockCutDirection::FromBottom
    );
}

#[test]
fn simulation_metrics_capture_emits_cut_trace_and_artifact() {
    let mut backend = ThreadedComputeBackend::new();
    backend.submit_simulation(small_simulation_request_with_metrics(true));

    let result = wait_for(&mut backend, Duration::from_secs(10), |msg| {
        matches!(msg, ComputeMessage::Simulation(Ok(_)))
    })
    .expect("simulation result");
    let ComputeMessage::Simulation(Ok(result)) = result else {
        panic!("expected successful simulation");
    };

    let trace = result.core.cut_trace.as_ref().expect("cut trace");
    assert!(trace.summary.sample_count > 0);
    assert!(trace.summary.total_runtime_s > 0.0);
    assert!(
        trace
            .toolpath_summaries
            .iter()
            .any(|summary| summary.toolpath_id == ToolpathId(1))
    );
    if let Some(path) = result.cut_trace_path.as_ref() {
        assert!(path.exists(), "expected cut trace artifact to exist");
        std::fs::remove_file(path).ok();
    } else {
        panic!("expected cut trace artifact path");
    }
}

// Regression: playback_data must carry drill_op for drill toolpaths so the
// live-sim forward-replay can call `apply_drill_op` (analytical kernel)
// instead of falling through to `simulate_toolpath_range`'s degenerate-Z
// dexel stamping. Without this, drill holes vanish or look wrong when
// scrubbing forward through a drill TP boundary, because the live-sim mesh
// diverges from what the compute path applied to the checkpoint stock.
//
// See `crates/rs_cam_viz/src/app/simulation.rs::update_live_sim` and the
// compute-side analogue at `compute/simulate.rs:387-406`.
#[test]
fn playback_data_carries_drill_op_for_drill_toolpaths() {
    use rs_cam_core::material::Material;
    use rs_cam_core::ops::drill::DrillCycle;
    use rs_cam_core::ops::drill_op::{DrillHole, DrillOp, HoleSource, ToolProfile};

    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    // The toolpath here is a placeholder — drill TPs in the analytical path
    // still emit motion (plunge+retract per hole), but the analytical kernel
    // ignores it and operates on `DrillOp.holes` directly.
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(5.0, 5.0, 10.0));
    tp.feed_to(P3::new(5.0, 5.0, 4.0), 300.0);
    tp.rapid_to(P3::new(5.0, 5.0, 10.0));

    let drill_op = Arc::new(DrillOp {
        holes: vec![DrillHole {
            xy: [5.0, 5.0],
            top_z: 10.0,
            bottom_z: 4.0,
        }],
        hole_source: HoleSource::ModelDerived,
        tool_profile: ToolProfile::Flat,
        tool_diameter_mm: 2.0,
        cycle: DrillCycle::Simple,
        feed_rate_mm_min: 300.0,
        spindle_rpm: 18_000,
        flute_count: 2,
        // R-2 (c4ef696) added this field; these two fixtures were missed and
        // left `rs_cam_viz`'s test target uncompilable at HEAD. `stock_top +
        // SAFE_Z_CLEARANCE_MM` is the documented default (10.0 + 5.0).
        retract_z_mm: 15.0,
        material: Material::default(),
    });

    let stock_bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(10.0, 10.0, 10.0),
    };

    let request = SimulationRequest {
        core: rs_cam_core::compute::simulate::SimulationRequest {
            groups: vec![SimGroupEntry {
                toolpaths: vec![SimToolpathEntry {
                    id: ToolpathId(42),
                    name: "Drill".to_owned(),
                    annotated: Arc::new(
                        rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(tp),
                    ),
                    tool: Arc::new(build_cutter(&tool)),
                    flute_count: tool.flute_count,
                    tool_summary: tool.summary(),
                    semantic_trace: None,
                    spindle_rpm: None,
                    metrics_not_applicable: true,
                    drill_op: Some(Arc::clone(&drill_op)),
                    operation_config_hash: 0,
                }],
                direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                local_stock_bbox: None,
                local_to_global: None,
                phantom_prior_stock: None,
            }],
            stock_bbox,
            stock_top_z: 10.0,
            resolution: 0.5,
            metric_options: rs_cam_core::stock::simulation_cut::SimulationMetricOptions::default(),
            spindle_rpm: 18_000,
            rapid_feed_mm_min: 5_000.0,
            model_mesh: None,
            kinematics: None,
        },
        memoize_prefix: false,
    };

    let mut backend = ThreadedComputeBackend::new();
    backend.submit_simulation(request);

    let msg = wait_for(&mut backend, Duration::from_secs(10), |msg| {
        matches!(msg, ComputeMessage::Simulation(Ok(_)))
    })
    .expect("simulation result");
    let ComputeMessage::Simulation(Ok(result)) = msg else {
        panic!("expected successful simulation");
    };

    assert_eq!(result.playback_data.len(), 1);
    let replay_drill_op = &result.playback_data[0].drill_op;
    let replay_drill_op = replay_drill_op
        .as_ref()
        .expect("drill TP must carry a drill_op in playback_data so live-sim can apply it");
    assert_eq!(replay_drill_op.holes.len(), 1);
    // No setup transform applied — global frame equals local.
    assert_eq!(replay_drill_op.holes[0].xy, [5.0, 5.0]);
    assert!((replay_drill_op.holes[0].top_z - 10.0).abs() < 1e-9);
    assert!((replay_drill_op.holes[0].bottom_z - 4.0).abs() < 1e-9);
    assert!((replay_drill_op.tool_diameter_mm - 2.0).abs() < 1e-9);
}

// Regression: when the drill TP sits in a flipped setup (face_up=Bottom),
// the playback_data drill_op must be re-expressed in the global stock
// frame — otherwise live-sim's `apply_drill_op` would carve a hole at
// the wrong Z (and possibly outside the stock bbox).
#[test]
fn playback_data_drill_op_transforms_to_global_frame_in_flipped_setup() {
    use rs_cam_core::material::Material;
    use rs_cam_core::ops::drill::DrillCycle;
    use rs_cam_core::ops::drill_op::{DrillHole, DrillOp, HoleSource, ToolProfile};

    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(5.0, 5.0, 10.0));
    tp.feed_to(P3::new(5.0, 5.0, 4.0), 300.0);
    tp.rapid_to(P3::new(5.0, 5.0, 10.0));

    let drill_op = Arc::new(DrillOp {
        holes: vec![DrillHole {
            xy: [5.0, 5.0],
            top_z: 10.0,
            bottom_z: 4.0,
        }],
        hole_source: HoleSource::ModelDerived,
        tool_profile: ToolProfile::Flat,
        tool_diameter_mm: 2.0,
        cycle: DrillCycle::Simple,
        feed_rate_mm_min: 300.0,
        spindle_rpm: 18_000,
        flute_count: 2,
        // R-2 (c4ef696) added this field; these two fixtures were missed and
        // left `rs_cam_viz`'s test target uncompilable at HEAD. `stock_top +
        // SAFE_Z_CLEARANCE_MM` is the documented default (10.0 + 5.0).
        retract_z_mm: 15.0,
        material: Material::default(),
    });

    let stock_bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(10.0, 10.0, 10.0),
    };

    let request = SimulationRequest {
        core: rs_cam_core::compute::simulate::SimulationRequest {
            groups: vec![SimGroupEntry {
                toolpaths: vec![SimToolpathEntry {
                    id: ToolpathId(7),
                    name: "Drill (back)".to_owned(),
                    annotated: Arc::new(
                        rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(tp),
                    ),
                    tool: Arc::new(build_cutter(&tool)),
                    flute_count: tool.flute_count,
                    tool_summary: tool.summary(),
                    semantic_trace: None,
                    spindle_rpm: None,
                    metrics_not_applicable: true,
                    drill_op: Some(Arc::clone(&drill_op)),
                    operation_config_hash: 0,
                }],
                direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                local_stock_bbox: Some(stock_bbox),
                local_to_global: Some(SetupTransformInfo {
                    face_up: crate::state::job::FaceUp::Bottom,
                    z_rotation: crate::state::job::ZRotation::Deg0,
                    stock_x: 10.0,
                    stock_y: 10.0,
                    stock_z: 10.0,
                    ..Default::default()
                }),
                phantom_prior_stock: None,
            }],
            stock_bbox,
            stock_top_z: 10.0,
            resolution: 0.5,
            metric_options: rs_cam_core::stock::simulation_cut::SimulationMetricOptions::default(),
            spindle_rpm: 18_000,
            rapid_feed_mm_min: 5_000.0,
            model_mesh: None,
            kinematics: None,
        },
        memoize_prefix: false,
    };

    let mut backend = ThreadedComputeBackend::new();
    backend.submit_simulation(request);

    let msg = wait_for(&mut backend, Duration::from_secs(10), |msg| {
        matches!(msg, ComputeMessage::Simulation(Ok(_)))
    })
    .expect("simulation result");
    let ComputeMessage::Simulation(Ok(result)) = msg else {
        panic!("expected successful simulation");
    };

    let replay_drill_op = &result.playback_data[0].drill_op;
    let replay_drill_op = replay_drill_op
        .as_ref()
        .expect("drill TP must carry a drill_op");
    // FaceUp::Bottom flips Z about the midplane (5.0), so local top_z=10
    // maps to global 0, local bottom_z=4 to global 6. Global frame is what
    // live-sim's `apply_drill_op` consumes.
    let h = &replay_drill_op.holes[0];
    assert!(
        (h.top_z - 0.0).abs() < 1e-6,
        "global top_z after FaceUp::Bottom flip should be 0, got {}",
        h.top_z
    );
    assert!(
        (h.bottom_z - 6.0).abs() < 1e-6,
        "global bottom_z after FaceUp::Bottom flip should be 6, got {}",
        h.bottom_z
    );
}

// G-LATERALSCRUB. A lateral setup replays in its OWN frame, not the global
// one: the global playback stock turns only its Z grid into a closed solid,
// so a lateral stamp there lands in a side grid that is appended as an open
// surface inside an intact block and removes nothing anyone can see.
//
// The two halves asserted here are the contract the viewport depends on —
// the playback entry says which frame it is in, and the checkpoint the
// viewport resets to is in that same frame and carries the cut.
#[test]
fn a_lateral_setup_replays_in_its_own_frame_and_its_checkpoint_carries_the_cut() {
    use rs_cam_core::dexel_stock::StockCutDirection;

    // Stock 20 x 10 x 8; `FaceUp::Front` maps (w, d, h) -> (w, h, d), so the
    // work plane is 20 by 8 and the tool axis runs the 10 mm depth.
    let stock_bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(20.0, 10.0, 8.0),
    };
    let local_bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(20.0, 8.0, 10.0),
    };

    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    // A groove in the Front work plane, 2 mm down from the lateral top (10).
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(5.0, 4.0, 12.0));
    tp.feed_to(P3::new(5.0, 4.0, 8.0), 300.0);
    tp.feed_to(P3::new(15.0, 4.0, 8.0), 600.0);
    tp.rapid_to(P3::new(15.0, 4.0, 12.0));

    let request = SimulationRequest {
        core: rs_cam_core::compute::simulate::SimulationRequest {
            groups: vec![SimGroupEntry {
                toolpaths: vec![SimToolpathEntry {
                    id: ToolpathId(11),
                    name: "Front groove".to_owned(),
                    annotated: Arc::new(
                        rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(tp),
                    ),
                    tool: Arc::new(build_cutter(&tool)),
                    flute_count: tool.flute_count,
                    tool_summary: tool.summary(),
                    semantic_trace: None,
                    spindle_rpm: None,
                    metrics_not_applicable: false,
                    drill_op: None,
                    operation_config_hash: 0,
                }],
                direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                local_stock_bbox: Some(local_bbox),
                local_to_global: Some(SetupTransformInfo {
                    face_up: crate::state::job::FaceUp::Front,
                    z_rotation: crate::state::job::ZRotation::Deg0,
                    stock_x: 20.0,
                    stock_y: 10.0,
                    stock_z: 8.0,
                    ..Default::default()
                }),
                phantom_prior_stock: None,
            }],
            stock_bbox,
            stock_top_z: 8.0,
            resolution: 0.5,
            metric_options: rs_cam_core::stock::simulation_cut::SimulationMetricOptions::default(),
            spindle_rpm: 18_000,
            rapid_feed_mm_min: 5_000.0,
            model_mesh: None,
            kinematics: None,
        },
        memoize_prefix: false,
    };

    let mut backend = ThreadedComputeBackend::new();
    backend.submit_simulation(request);

    let msg = wait_for(&mut backend, Duration::from_secs(30), |msg| {
        matches!(msg, ComputeMessage::Simulation(Ok(_)))
    })
    .expect("simulation result");
    let ComputeMessage::Simulation(Ok(result)) = msg else {
        panic!("expected successful simulation");
    };

    let entry = &result.playback_data[0];
    assert!(
        entry.frame.is_some(),
        "a lateral setup's playback entry must declare the setup-local frame; \
         `None` means it would be stamped into the global stock, where the cut \
         cannot be rendered at all"
    );
    assert_eq!(
        entry.direction,
        StockCutDirection::FromTop,
        "setup-local Z is always the tool axis, so the local replay stamps FromTop"
    );
    assert_eq!(
        entry.stock_bbox.max.z, local_bbox.max.z,
        "the entry's stock bbox must be the LOCAL one (lateral top 10), not the \
         global stock height"
    );

    let cp = result.core.checkpoints.last().expect("one checkpoint");
    assert!(
        cp.stock_local_to_global.is_some(),
        "a lateral setup's checkpoint must publish its local stock, since that is \
         the only object that carries the cut"
    );
    // The cut must be IN it: rays under the groove have lost material.
    let grid = &cp.stock.z_grid;
    let (row, col) = grid
        .world_to_cell(10.0, 4.0)
        .expect("groove midpoint is inside the local grid");
    let top = grid
        .top_z_at(row, col)
        .expect("material remains below the cut");
    assert!(
        f64::from(top) < local_bbox.max.z - 1.0,
        "G-LATERALSCRUB: the lateral groove did not reach the checkpoint stock's \
         solid — top_z at the groove midpoint is {top}, still at the uncut lateral \
         top {}",
        local_bbox.max.z
    );
}

#[test]
fn simulation_metrics_capture_emits_semantic_cut_summaries() {
    let mut backend = ThreadedComputeBackend::new();
    backend.submit_simulation(small_simulation_request_with_semantic_metrics(true));

    let result = wait_for(&mut backend, Duration::from_secs(10), |msg| {
        matches!(msg, ComputeMessage::Simulation(Ok(_)))
    })
    .expect("simulation result");
    let ComputeMessage::Simulation(Ok(result)) = result else {
        panic!("expected successful simulation");
    };

    let trace = result.core.cut_trace.as_ref().expect("cut trace");
    let summary = trace
        .semantic_summaries
        .iter()
        .find(|summary| summary.toolpath_id == ToolpathId(1))
        .expect("semantic cut summary");
    assert_eq!(summary.label, "Pass 1");
    assert!(summary.sample_count > 0);
    assert!(summary.average_mrr_mm3_s >= 0.0);
    assert!(summary.representative_sample_index <= summary.sample_count);

    if let Some(path) = result.cut_trace_path.as_ref() {
        std::fs::remove_file(path).ok();
    }
}

/// F-024 (viz-path follow-up, 2026-05-25): regression test asserting that
/// an identity setup grids its dexels in the WORLD frame, which is what
/// `local_stock_bbox = None` beside `local_to_global = None` asks for.
/// A zero-rooted `local_stock_bbox` (Z=[0, stock_z]) here instead spans
/// world Z=[0, 12] while the toolpath emits cuts at world Z=-2. The
/// cutter then sits below every dexel ray, `ray_blend_above` clears the
/// full ray length, and per-sample `axial_engagement_mm` reads the full
/// stock height instead of the commanded DOC.
///
/// CMP-19 (2026-09-18): the drop rule itself is no longer the worker's.
/// The viz worker had its own copy in `build_core_simulation_request`;
/// the controller now calls `SetupEvalContext::sim_local_stock_bbox`,
/// the same accessor `ProjectSession::run_simulation` calls. This test
/// keeps the CONSEQUENCE pinned on the viz simulation entry point.
///
/// AS001-shape: identity setup, hardwood 100x100x12 with origin at
/// `(-10, -10, -12)` so stock top is at world Z=0. A handful of linear
/// cutting moves at Z=-2 (the first-pass plane) replays the AS001 pocket
/// shape without depending on the pocket generator. The viz request
/// mirrors what `controller::events::simulation` produces for identity
/// setups: `local_stock_bbox` rooted at zero-local (Z=[0, 12]) and
/// `local_to_global = None`.
///
/// Pre-fix: per-sample `axial_engagement_mm` reads ~12 (full stock
/// height). Post-fix: reads ~2 (commanded DOC ± grid discretisation).
#[test]
fn as001_viz_path_first_pass_axial_engagement_within_commanded_doc_f024() {
    use rs_cam_core::stock::simulation_cut::CutKinematics;

    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.shank_diameter = 6.35;
    tool.shank_length = 20.0;
    tool.stickout = 45.0;
    tool.flute_count = 2;
    tool.name = "End Mill 6mm".to_owned();

    // World-frame stock: 100x100x12 with origin at (-10, -10, -12) — stock
    // top at world Z=0, bottom at Z=-12. Matches the AS001 fixture.
    let world_stock_bbox = BoundingBox3 {
        min: P3::new(-10.0, -10.0, -12.0),
        max: P3::new(90.0, 90.0, 0.0),
    };
    // First-pass-of-pocket-style linear cutting at world Z=-2. The exact
    // span shape doesn't matter — what matters is that the toolpath emits
    // cuts in world frame at Z=-2 while the viz-side local bbox is in
    // local Z=[0, 12]. With the fix the dexel grid lives in world frame
    // (via stock_bbox fallback) so the cutter at Z=-2 stamps the ray top
    // for ~2 mm. Without the fix the cutter sits below every ray and
    // strips ~12 mm.
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(10.0, 30.0, 5.0));
    tp.feed_to(P3::new(10.0, 30.0, -2.0), 385.0);
    for i in 0..40 {
        let x = 10.0 + (i as f64) * 1.5;
        tp.feed_to(P3::new(x, 30.0, -2.0), 770.0);
    }
    tp.rapid_to(P3::new(70.0, 30.0, 5.0));

    let request = SimulationRequest {
        core: rs_cam_core::compute::simulate::SimulationRequest {
            groups: vec![SimGroupEntry {
                toolpaths: vec![SimToolpathEntry {
                    id: ToolpathId(1),
                    name: "AS001 Pocket Pass 1".to_owned(),
                    annotated: Arc::new(
                        rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(tp),
                    ),
                    tool: Arc::new(build_cutter(&tool)),
                    flute_count: tool.flute_count,
                    tool_summary: tool.summary(),
                    semantic_trace: None,
                    spindle_rpm: Some(18_000),
                    metrics_not_applicable: false,
                    drill_op: None,
                    operation_config_hash: 0,
                }],
                // Identity setup shape from controller/events/simulation.rs:
                // no local bbox, no transform. The zero-rooted local bbox
                // `effective_stock_bbox()` returns was forwarded here inside
                // `Some(...)` before F-024, and that was the defect.
                direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
                local_stock_bbox: None,
                local_to_global: None,
                phantom_prior_stock: None,
            }],
            stock_bbox: world_stock_bbox,
            stock_top_z: 0.0,
            resolution: 1.0,
            metric_options: rs_cam_core::stock::simulation_cut::SimulationMetricOptions {
                enabled: true,
                capture_arc_engagement: true,
            },
            spindle_rpm: 18_000,
            rapid_feed_mm_min: 5_000.0,
            model_mesh: None,
            kinematics: None,
        },
        memoize_prefix: false,
    };

    // Drive the viz production sim entry point directly (the same function
    // the worker thread calls from `worker.rs:739`).
    let cancel = AtomicBool::new(false);
    let result = super::execute::run_simulation_with_phase(&request, &cancel, |_phase| {}, None)
        .expect("viz simulation completes");

    let cut_trace = result.core.cut_trace.as_ref().expect("metric cut trace");

    // Filter to non-plunge linear cutting samples on the first-pass Z plane
    // (Z = -2 ± 0.5). Pre-fix every such sample reads
    // `axial_engagement_mm` ~= stock_z (= 12.0). Post-fix should report
    // the commanded ~2.0 mm plus grid discretisation margin.
    let mut first_pass_axials: Vec<f64> = cut_trace
        .samples
        .iter()
        .filter(|s| s.is_cutting && s.cut_kinematics == CutKinematics::Linear)
        .filter(|s| (s.position[2] - (-2.0)).abs() < 0.5)
        .map(|s| s.axial_engagement_mm)
        .collect();
    first_pass_axials.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    assert!(
        !first_pass_axials.is_empty(),
        "expected at least one linear cutting sample near Z=-2 on the first pass; got 0"
    );

    let peak = *first_pass_axials.last().unwrap_or(&0.0);
    assert!(
        peak <= 3.0,
        "F-024 viz-path: first-pass axial engagement should be <= 3.0 mm (commanded 2.0 + grid \
         discretisation margin); got peak = {peak:.4} mm across {} samples. Pre-fix this reads \
         the full stock height (~12 mm) because the viz worker forwarded the zero-rooted local \
         bbox to core even for identity setups, so the dexel grid spanned world Z=[0, 12] while \
         the toolpath cut at world Z=-2.",
        first_pass_axials.len()
    );
}

// ── C1 item 4b: the reconcile path runs in the DEFAULT configuration ─────

/// Pre-C1 the worker's remap calls were each wrapped in
/// `if let Some(recorder) = semantic_recorder.as_ref()`, and the recorder was
/// only ever constructed when `debug_options.enabled`. So the code that keeps
/// index-carrying channels in step with the transforms never ran in the
/// product the operator uses, and every test of it ran a different branch
/// than production does.
///
/// Under C1 the `ReconcileSet` is built unconditionally and every transform
/// reconciles through it. WP11b moved the pipeline into
/// `rs_cam_core::session::execute_job`, which records ALWAYS and gates only
/// the per-dressup ITEMS on `debug_options` — so the shape this test drives
/// is now the shipped one directly: `debug_options.enabled == false`, a
/// recorder present, the remap path measured on the default configuration.
///
/// It also pins C1 item 4a: each per-dressup item now declares whether its
/// move range is the moves the step actually restructured or an explicit
/// whole-path claim, instead of every item silently binding `0..len`.
#[test]
fn worker_reconciles_semantic_links_with_debug_options_disabled() {
    let dressed = |debug: bool| {
        let mut spec = pocket_spec(77);
        // Transforms that actually move indices: reorder, arc collapse,
        // link bridges, ramp entries. Without these the reconcile is
        // vacuous.
        spec.dressups.optimize_rapid_order = true;
        spec.dressups.link_moves = true;
        spec.dressups.link_max_distance = 50.0;
        spec.dressups.arc_fitting = true;
        spec.dressups.arc_tolerance = 0.05;
        spec.dressups.entry_style = crate::state::toolpath::DressupEntryStyle::Ramp;
        spec.debug_options.enabled = debug;
        build_request(spec)
    };

    // WP11b: `execute_job` owns the recorders, so this door no longer takes
    // one, and the semantic trace comes back on the result. The claim is
    // unchanged: the reconcile runs with `debug_options.enabled` FALSE,
    // which is the shipped configuration.
    let plain = dressed(false);
    let outcome = super::execute::run_compute_with_phase_tracker(&plain, None);
    let result = outcome.result.expect("compute should succeed");
    let move_count = result.toolpath().moves.len();
    assert!(
        move_count > 0,
        "non-vacuity: the fixture must cut something"
    );

    let trace = result
        .semantic_trace
        .as_ref()
        .expect("core records a semantic trace on every generation");
    assert!(
        trace.summary.move_linked_item_count > 0,
        "non-vacuity: unlinking everything would satisfy the bounds check below trivially"
    );

    for item in &trace.items {
        match (item.move_start, item.move_end) {
            (Some(start), Some(end)) => {
                assert!(end >= start, "item {} has an inverted link", item.id);
                assert!(
                    end < move_count,
                    "item {} ({:?} {:?}) is linked to moves {start}..={end} but the shipped \
                     toolpath has only {move_count} moves — slicing by this range reads out of \
                     bounds. With debug options OFF, this is the path the product runs.",
                    item.id,
                    item.kind,
                    item.label
                );
            }
            // Deleted-move policy: unlinked, never half-linked.
            (None, None) => {}
            half => panic!("item {} is half-linked {half:?}", item.id),
        }
    }

    // Item 4a: per-dressup items declare the scope of their claim.
    //
    // WP11b moved the ITEM gate onto `debug_options`, which is where the
    // GUI worker had it (it built no recorder at all without the flag). So
    // this half of the claim is measured on the debug configuration, and
    // the bounds half above stays on the shipped one.
    let traced = dressed(true);
    let traced_result = super::execute::run_compute_with_phase_tracker(&traced, None)
        .result
        .expect("the debug compute should succeed");
    let traced_trace = traced_result
        .semantic_trace
        .as_ref()
        .expect("core records a semantic trace on every generation");
    let dressup_items: Vec<_> = traced_trace
        .items
        .iter()
        .filter(|i| {
            i.params
                .get(rs_cam_core::trace::semantic_trace::SemanticKey::MoveScope)
                .is_some()
        })
        .collect();
    assert!(
        !dressup_items.is_empty(),
        "the dressup steps should each record a scoped item"
    );
    for item in dressup_items {
        let scope = item
            .params
            .values
            .get("move_scope")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(
            scope == "touched_moves" || scope == "whole_path",
            "item {} ({}) has an unexpected move_scope {scope:?}",
            item.id,
            item.label
        );
    }
    if let Some(path) = traced_result.debug_trace_path.as_ref() {
        std::fs::remove_file(path).ok();
    }
}

// ---------------------------------------------------------------------------
// G-REGEN-RACE — the lane says, under its own lock, whether a submit
// superseded the active job.
//
// The toolpath lane's submit rule is resubmit-cancels-and-requeues. Before
// this, the rule was invisible to callers: the abandoned job's
// `ComputeError::Cancelled` arrived looking exactly like an operator or an
// agent having cancelled the generation, and the drain reported it as the
// toolpath's outcome. Anything downstream that tried to reconstruct "was
// that cancel mine?" from a lane snapshot would be re-running the same race
// one level up, so the answer is returned from inside the lock.
// ---------------------------------------------------------------------------

/// A cheap replacement request for `id` — cheap on purpose, so the test's
/// own supersede does not leave a second heavy DropCutter running behind it.
fn cheap_request_for(id: usize) -> ComputeRequest {
    build_request(pocket_spec(id))
}

#[test]
fn resubmitting_the_active_toolpath_reports_a_supersede() {
    let mut backend = ThreadedComputeBackend::new();

    // An idle lane has nothing to supersede.
    assert_eq!(
        backend.submit_toolpath(build_request(heavy_dropcutter_spec(91))),
        ToolpathSubmitOutcome::Queued,
        "the first submit onto an idle lane cannot supersede anything"
    );

    // Retry rather than sleep-and-hope, and resubmit the HEAVY request so
    // every attempt leaves another long job queued for the same toolpath.
    // That is what makes the loop terminate rather than flake: if an
    // attempt lands in the gap between two jobs it simply queues the next
    // window instead of consuming the only one. A fixed sleep would make
    // this assertion about the machine's speed rather than the lane's rule.
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut superseded = false;
    while Instant::now() < deadline {
        if backend.submit_toolpath(build_request(heavy_dropcutter_spec(91)))
            == ToolpathSubmitOutcome::SupersededActive
        {
            superseded = true;
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert!(
        superseded,
        "resubmitting the toolpath the lane is actively running must report \
         SupersededActive — that report is what stops the resulting Cancelled \
         from being read as the toolpath's outcome"
    );

    // Replace the heavy job still queued behind the supersede with a cheap
    // one, so the test does not pay for a second DropCutter to tear down.
    let _ = backend.submit_toolpath(cheap_request_for(91));

    // A different toolpath never supersedes, however busy the lane is.
    assert_eq!(
        backend.submit_toolpath(cheap_request_for(92)),
        ToolpathSubmitOutcome::Queued,
        "a submit for a different toolpath must never claim a supersede"
    );

    backend.cancel_lane(ComputeLane::Toolpath);
    let _ = wait_for(&mut backend, Duration::from_secs(60), |message| {
        matches!(message, ComputeMessage::Toolpath(result)
            if matches!(result.toolpath_id, ToolpathId(92)))
    });
}
