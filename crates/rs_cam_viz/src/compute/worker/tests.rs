use super::*;
use crate::compute::{ComputeBackend, ComputeLane, ComputeMessage, LaneState};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::make_test_flat;
use rs_cam_core::toolpath::Toolpath;
use std::thread;
use std::time::{Duration, Instant};

use crate::state::job::{ToolId, ToolType};

fn sample_request(operation: OperationConfig, stock_source: StockSource) -> ComputeRequest {
    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    ComputeRequest {
        toolpath_id: ToolpathId(1),
        toolpath_name: "Sample".to_string(),
        polygons: None,
        mesh: None,
        operation,
        dressups: DressupConfig::default(),
        stock_source,
        tool,
        safe_z: 10.0,
        prev_tool_radius: None,
        stock_bbox: Some(BoundingBox3 {
            min: P3::new(10.0, 20.0, -5.0),
            max: P3::new(40.0, 60.0, 12.0),
        }),
        boundary_enabled: false,
        boundary_containment: BoundaryContainment::Center,
        keep_out_footprints: Vec::new(),
        heights: HeightsConfig::default().resolve(10.0, 5.0),
        debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
    }
}

#[test]
fn feed_optimization_uses_real_stock_bounds() {
    let request = sample_request(
        OperationConfig::new_default(OperationType::Pocket),
        StockSource::Fresh,
    );

    let heightmap = helpers::feed_optimization_heightmap(&request).unwrap();
    assert_eq!(heightmap.origin_x, 10.0);
    assert_eq!(heightmap.origin_y, 20.0);
    assert_eq!(heightmap.stock_top_z, 12.0);
    assert_eq!(heightmap.cell_size, 1.5875);
}

#[test]
fn feed_optimization_rejects_remaining_stock() {
    let request = sample_request(
        OperationConfig::new_default(OperationType::Pocket),
        StockSource::FromRemainingStock,
    );

    let error = match helpers::feed_optimization_heightmap(&request) {
        Ok(_) => panic!("remaining-stock feed optimization should be rejected"),
        Err(error) => error,
    };
    assert_eq!(
        error,
        "Phase 1 feed optimization only supports fresh stock, not remaining-stock workflows."
    );
}

#[test]
fn feed_optimization_rejects_mesh_derived_operations() {
    let request = sample_request(
        OperationConfig::new_default(OperationType::DropCutter),
        StockSource::Fresh,
    );

    let error = match helpers::feed_optimization_heightmap(&request) {
        Ok(_) => panic!("mesh-derived feed optimization should be rejected"),
        Err(error) => error,
    };
    assert_eq!(
        error,
        "Phase 1 feed optimization only supports operations that start from flat stock, not mesh-derived surfaces."
    );
}

fn quick_pocket_request(id: usize) -> ComputeRequest {
    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    ComputeRequest {
        toolpath_id: ToolpathId(id),
        toolpath_name: format!("Pocket {id}"),
        polygons: Some(Arc::new(vec![Polygon2::rectangle(
            -20.0, -20.0, 20.0, 20.0,
        )])),
        mesh: None,
        operation: OperationConfig::new_default(OperationType::Pocket),
        dressups: DressupConfig::default(),
        stock_source: StockSource::Fresh,
        tool,
        safe_z: 10.0,
        prev_tool_radius: None,
        stock_bbox: Some(BoundingBox3 {
            min: P3::new(-25.0, -25.0, -5.0),
            max: P3::new(25.0, 25.0, 10.0),
        }),
        boundary_enabled: false,
        boundary_containment: BoundaryContainment::Center,
        keep_out_footprints: Vec::new(),
        heights: HeightsConfig::default().resolve(10.0, 5.0),
        debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
    }
}

fn heavy_dropcutter_request(id: usize) -> ComputeRequest {
    let tool = ToolConfig::new_default(ToolId(1), ToolType::BallNose);
    let mesh = make_test_flat(120.0);
    let mut cfg = match OperationConfig::new_default(OperationType::DropCutter) {
        OperationConfig::DropCutter(cfg) => cfg,
        _ => unreachable!("default op kind mismatch"),
    };
    cfg.stepover = 0.25;
    cfg.min_z = -5.0;
    ComputeRequest {
        toolpath_id: ToolpathId(id),
        toolpath_name: format!("DropCutter {id}"),
        polygons: None,
        mesh: Some(Arc::new(mesh)),
        operation: OperationConfig::DropCutter(cfg),
        dressups: DressupConfig::default(),
        stock_source: StockSource::Fresh,
        tool,
        safe_z: 10.0,
        prev_tool_radius: None,
        stock_bbox: Some(BoundingBox3 {
            min: P3::new(-60.0, -60.0, -5.0),
            max: P3::new(60.0, 60.0, 10.0),
        }),
        boundary_enabled: false,
        boundary_containment: BoundaryContainment::Center,
        keep_out_footprints: Vec::new(),
        heights: HeightsConfig::default().resolve(10.0, 5.0),
        debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
    }
}

fn waterline_request(id: usize) -> ComputeRequest {
    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    let mesh = make_test_flat(60.0);
    let mut cfg = match OperationConfig::new_default(OperationType::Waterline) {
        OperationConfig::Waterline(cfg) => cfg,
        _ => unreachable!("default op kind mismatch"),
    };
    cfg.start_z = 0.0;
    cfg.final_z = -2.0;
    cfg.z_step = 1.0;
    cfg.sampling = 1.0;
    ComputeRequest {
        toolpath_id: ToolpathId(id),
        toolpath_name: format!("Waterline {id}"),
        polygons: None,
        mesh: Some(Arc::new(mesh)),
        operation: OperationConfig::Waterline(cfg),
        dressups: DressupConfig::default(),
        stock_source: StockSource::Fresh,
        tool,
        safe_z: 10.0,
        prev_tool_radius: None,
        stock_bbox: Some(BoundingBox3 {
            min: P3::new(-30.0, -30.0, -5.0),
            max: P3::new(30.0, 30.0, 10.0),
        }),
        boundary_enabled: false,
        boundary_containment: BoundaryContainment::Center,
        keep_out_footprints: Vec::new(),
        heights: HeightsConfig::default().resolve(10.0, 5.0),
        debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
    }
}

fn adaptive3d_request(id: usize) -> ComputeRequest {
    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    let mesh = make_test_flat(60.0);
    let mut cfg = match OperationConfig::new_default(OperationType::Adaptive3d) {
        OperationConfig::Adaptive3d(cfg) => cfg,
        _ => unreachable!("default op kind mismatch"),
    };
    cfg.depth_per_pass = 2.0;
    cfg.stock_top_z = 6.0;
    cfg.detect_flat_areas = true;
    cfg.region_ordering = crate::state::toolpath::RegionOrdering::ByArea;
    ComputeRequest {
        toolpath_id: ToolpathId(id),
        toolpath_name: format!("Adaptive3d {id}"),
        polygons: None,
        mesh: Some(Arc::new(mesh)),
        operation: OperationConfig::Adaptive3d(cfg),
        dressups: DressupConfig::default(),
        stock_source: StockSource::Fresh,
        tool,
        safe_z: 10.0,
        prev_tool_radius: None,
        stock_bbox: Some(BoundingBox3 {
            min: P3::new(-30.0, -30.0, -5.0),
            max: P3::new(30.0, 30.0, 10.0),
        }),
        boundary_enabled: false,
        boundary_containment: BoundaryContainment::Center,
        keep_out_footprints: Vec::new(),
        heights: HeightsConfig::default().resolve(10.0, 5.0),
        debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
    }
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

    SimulationRequest {
        groups: vec![SetupSimGroup {
            toolpaths: vec![(
                ToolpathId(99),
                "Long Sim".to_string(),
                Arc::new(toolpath),
                tool,
            )],
            direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
        }],
        stock_bbox: BoundingBox3 {
            min: P3::new(0.0, 0.0, -2.0),
            max: P3::new(100.0, 100.0, 10.0),
        },
        stock_top_z: 10.0,
        resolution: 0.5,
        model_mesh: None,
    }
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
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut debug_req = quick_pocket_request(41);
    debug_req.debug_options.enabled = true;

    let debug_result = super::execute::run_compute(&debug_req, &cancel);
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

    let plain_result = super::execute::run_compute(&quick_pocket_request(42), &cancel)
        .result
        .expect("plain compute should succeed");
    assert!(plain_result.debug_trace.is_none());
    assert!(plain_result.semantic_trace.is_none());
    assert!(plain_result.debug_trace_path.is_none());
    assert_toolpaths_match(&debug_toolpath.toolpath, &plain_result.toolpath);
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
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut request = sample_request(
        OperationConfig::new_default(OperationType::Pocket),
        StockSource::Fresh,
    );
    request.toolpath_id = ToolpathId(77);
    request.toolpath_name = "Dressup phases".to_string();
    request.polygons = Some(Arc::new(vec![Polygon2::rectangle(
        -20.0, -20.0, 20.0, 20.0,
    )]));
    request.dressups.arc_fitting = true;
    request.dressups.feed_optimization = true;
    request.debug_options.enabled = true;

    let result = super::execute::run_compute(&request, &cancel)
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
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut request = heavy_dropcutter_request(78);
    request.debug_options.enabled = true;

    let result = super::execute::run_compute(&request, &cancel)
        .result
        .expect("dropcutter debug compute should succeed");
    let trace = result
        .debug_trace
        .as_ref()
        .expect("dropcutter debug compute should return a trace");

    assert!(trace.spans.iter().any(|span| span.kind == "prepare_input"));
    assert!(
        trace
            .spans
            .iter()
            .any(|span| span.kind == "dropcutter_grid")
    );
    assert!(trace.spans.iter().any(|span| span.kind == "rasterize_grid"));

    if let Some(path) = result.debug_trace_path.as_ref() {
        std::fs::remove_file(path).ok();
    }
}

#[test]
fn debug_trace_records_waterline_prepare_and_slice_phases() {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut request = waterline_request(79);
    request.debug_options.enabled = true;

    let result = super::execute::run_compute(&request, &cancel)
        .result
        .expect("waterline debug compute should succeed");
    let trace = result
        .debug_trace
        .as_ref()
        .expect("waterline debug compute should return a trace");

    assert!(trace.spans.iter().any(|span| span.kind == "prepare_input"));
    assert!(
        trace
            .spans
            .iter()
            .any(|span| span.kind == "waterline_slices")
    );

    if let Some(path) = result.debug_trace_path.as_ref() {
        std::fs::remove_file(path).ok();
    }
}

#[test]
fn cancelled_toolpath_returns_partial_debug_trace() {
    let mut backend = ThreadedComputeBackend::new();
    let mut request = heavy_dropcutter_request(88);
    request.debug_options.enabled = true;
    backend.submit_toolpath(request);
    thread::sleep(Duration::from_millis(20));
    backend.cancel_lane(ComputeLane::Toolpath);

    let cancelled = wait_for(&mut backend, Duration::from_secs(5), |message| {
        matches!(
            message,
            ComputeMessage::Toolpath(ComputeResult {
                toolpath_id: ToolpathId(88),
                result: Err(ComputeError::Cancelled),
                ..
            })
        )
    });
    let cancelled = match cancelled {
        Some(ComputeMessage::Toolpath(result)) => result,
        Some(_) => panic!("expected toolpath result"),
        None => panic!("expected cancelled toolpath result"),
    };
    assert!(
        cancelled.debug_trace.is_some(),
        "cancelled debug run should return a partial trace"
    );
    assert!(
        cancelled.semantic_trace.is_some(),
        "cancelled debug run should return a partial semantic trace"
    );
    let trace_path = cancelled
        .debug_trace_path
        .as_ref()
        .expect("cancelled debug run should write an artifact");
    assert!(
        trace_path.exists(),
        "expected trace artifact at {:?}",
        trace_path
    );
    std::fs::remove_file(trace_path).ok();
}

#[test]
fn semantic_trace_records_entry_params_and_boundary_clip() {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut request = quick_pocket_request(90);
    request.debug_options.enabled = true;
    request.boundary_enabled = true;
    request.stock_bbox = Some(BoundingBox3 {
        min: P3::new(-10.0, -10.0, -5.0),
        max: P3::new(10.0, 10.0, 10.0),
    });
    request.dressups.entry_style = crate::state::toolpath::DressupEntryStyle::Helix;
    request.dressups.lead_in_out = true;
    request.dressups.link_moves = true;
    request.dressups.arc_fitting = true;
    request.dressups.optimize_rapid_order = true;

    let result = super::execute::run_compute(&request, &cancel)
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
            item.kind == rs_cam_core::semantic_trace::ToolpathSemanticKind::Entry
                && item.label == "Helix entry"
        })
        .expect("helix entry item should be present");
    assert_eq!(
        helix.params.values.get("radius"),
        Some(&serde_json::json!(request.dressups.helix_radius))
    );
    assert_eq!(
        helix.params.values.get("pitch"),
        Some(&serde_json::json!(request.dressups.helix_pitch))
    );

    let boundary_clip = semantic_trace
        .items
        .iter()
        .find(|item| item.kind == rs_cam_core::semantic_trace::ToolpathSemanticKind::BoundaryClip)
        .expect("boundary clip item should be present");
    assert_eq!(
        boundary_clip.params.values.get("containment"),
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

#[test]
fn adaptive3d_semantic_trace_records_runtime_structure() {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut request = adaptive3d_request(91);
    request.debug_options.enabled = true;

    let result = super::execute::run_compute(&request, &cancel)
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
            .any(|item| item.label == "Z level planning"),
        "expected planning item"
    );
    assert!(
        semantic_trace
            .items
            .iter()
            .any(|item| item.label == "Flat shelf detection"),
        "expected flat detection item"
    );
    assert!(
        semantic_trace
            .items
            .iter()
            .any(|item| item.label == "Region detection"),
        "expected region detection item"
    );
    assert!(
        semantic_trace
            .items
            .iter()
            .any(|item| item.kind == rs_cam_core::semantic_trace::ToolpathSemanticKind::DepthLevel),
        "expected depth-level semantics"
    );
    let pass = semantic_trace
        .items
        .iter()
        .find(|item| item.label.starts_with("Adaptive pass "))
        .expect("expected adaptive pass semantic item");
    assert!(pass.move_start.is_some() && pass.move_end.is_some());
    assert!(pass.params.values.get("step_count").is_some());
    assert!(pass.params.values.get("yield_ratio").is_some());

    if let Some(path) = result.debug_trace_path.as_ref() {
        std::fs::remove_file(path).ok();
    }
}

#[test]
fn running_lane_snapshot_reports_current_phase() {
    let mut backend = ThreadedComputeBackend::new();
    backend.submit_toolpath(heavy_dropcutter_request(89));

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
    backend.submit_toolpath(quick_pocket_request(7));

    let result = wait_for(&mut backend, Duration::from_secs(5), |message| {
        matches!(
            message,
            ComputeMessage::Toolpath(ComputeResult {
                toolpath_id: ToolpathId(7),
                result: Ok(_),
                ..
            })
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
    backend.submit_toolpath(heavy_dropcutter_request(1));
    thread::sleep(Duration::from_millis(20));
    backend.submit_toolpath(quick_pocket_request(2));
    backend.submit_toolpath(quick_pocket_request(2));

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
    backend.submit_toolpath(heavy_dropcutter_request(3));
    thread::sleep(Duration::from_millis(20));
    backend.submit_toolpath(quick_pocket_request(3));

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
                ComputeMessage::Toolpath(ComputeResult {
                    toolpath_id: ToolpathId(3),
                    result: Err(ComputeError::Cancelled),
                    ..
                }) => {
                    saw_cancelled = true;
                }
                ComputeMessage::Toolpath(ComputeResult {
                    toolpath_id: ToolpathId(3),
                    result: Ok(_),
                    ..
                }) => {
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
        toolpath: Arc::new({
            let mut toolpath = Toolpath::new();
            toolpath.rapid_to(P3::new(0.0, 0.0, 5.0));
            toolpath.feed_to(P3::new(0.0, 0.0, -1.0), 300.0);
            toolpath
        }),
        tool: ToolConfig::new_default(ToolId(1), ToolType::EndMill),
        mesh: Arc::new(make_test_flat(20.0)),
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
    backend.submit_toolpath(heavy_dropcutter_request(4));
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

    // --- Bottom setup: toolpath in *global stock frame* (pre-transformed) ---
    // In the bottom setup's local frame, the stock is flipped.
    // For Bottom: inverse_transform maps local (x,y,z) → global (x, D-y, H-z).
    // A cut at local Z = stock_h - 5 (5mm from bottom surface) means
    // global Z = H - (H - 5) = 5.0.
    // After pre-transform, the toolpath points are in global frame.
    let mut bottom_tp = Toolpath::new();
    bottom_tp.rapid_to(P3::new(25.0, 25.0, -5.0));
    // Cut at global Z=5 (5mm above stock bottom), direction FromBottom
    for i in 0..20 {
        let x = 20.0 + (i as f64) * 0.5;
        bottom_tp.feed_to(P3::new(x, 25.0, 5.0), 600.0);
    }

    let request = SimulationRequest {
        groups: vec![
            SetupSimGroup {
                toolpaths: vec![(
                    ToolpathId(1),
                    "Top Cut".to_string(),
                    Arc::new(top_tp),
                    tool.clone(),
                )],
                direction: StockCutDirection::FromTop,
            },
            SetupSimGroup {
                toolpaths: vec![(
                    ToolpathId(2),
                    "Bottom Cut".to_string(),
                    Arc::new(bottom_tp),
                    tool.clone(),
                )],
                direction: StockCutDirection::FromBottom,
            },
        ],
        stock_bbox,
        stock_top_z: 20.0,
        resolution: 0.5,
        model_mesh: None,
    };

    let mut backend = ThreadedComputeBackend::new();
    backend.submit_simulation(request);

    let result = wait_for(&mut backend, Duration::from_secs(10), |msg| {
        matches!(msg, ComputeMessage::Simulation(Ok(_)))
    });
    let result = match result.unwrap() {
        ComputeMessage::Simulation(Ok(r)) => r,
        _ => panic!("expected simulation result"),
    };

    // Should have 2 boundaries (one per toolpath)
    assert_eq!(result.boundaries.len(), 2);
    assert_eq!(result.boundaries[0].direction, StockCutDirection::FromTop);
    assert_eq!(
        result.boundaries[1].direction,
        StockCutDirection::FromBottom
    );

    // Should have 2 checkpoints
    assert_eq!(result.checkpoints.len(), 2);

    // Should have playback data for both toolpaths
    assert_eq!(result.playback_data.len(), 2);

    // Verify the stock state after both setups:
    // At the cut location (x=25, y=25), top was cut to Z=15 and bottom to Z=5.
    // The remaining material should be from Z=5 to Z=15.
    let final_stock = &result.checkpoints[1].stock;
    let (r, c) = final_stock.z_grid.world_to_cell(25.0, 25.0).unwrap();
    let ray = final_stock.z_grid.ray(r, c);
    assert!(!ray.is_empty(), "ray should have material");

    // Ray should have one segment from ~5.0 to ~15.0
    assert_eq!(ray.len(), 1, "should be a single segment");
    let seg = ray[0];
    assert!(
        (seg.enter - 5.0).abs() < 1.0,
        "bottom cut should leave material starting near Z=5, got {}",
        seg.enter
    );
    assert!(
        (seg.exit - 15.0).abs() < 1.0,
        "top cut should leave material ending near Z=15, got {}",
        seg.exit
    );

    // Verify checkpoint 0 (after top cut only): material should be from Z=0 to Z=15
    let after_top = &result.checkpoints[0].stock;
    let (r, c) = after_top.z_grid.world_to_cell(25.0, 25.0).unwrap();
    let ray = after_top.z_grid.ray(r, c);
    assert_eq!(ray.len(), 1);
    assert!(
        ray[0].enter.abs() < 0.01,
        "after top only, material starts at Z=0"
    );
    assert!(
        (ray[0].exit - 15.0).abs() < 1.0,
        "after top only, material ends near Z=15, got {}",
        ray[0].exit
    );

    // Verify an uncut location: far from the cut path
    let (r, c) = final_stock.z_grid.world_to_cell(5.0, 5.0).unwrap();
    let ray = final_stock.z_grid.ray(r, c);
    assert_eq!(ray.len(), 1, "uncut area should be one full segment");
    assert!(ray[0].enter.abs() < 0.01);
    assert!((ray[0].exit - 20.0).abs() < 0.01);
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

    // Two toolpaths in two setup groups
    let mut tp1 = Toolpath::new();
    tp1.rapid_to(P3::new(15.0, 15.0, 15.0));
    for i in 0..50 {
        tp1.feed_to(P3::new(10.0 + i as f64 * 0.2, 15.0, 7.0), 600.0);
    }

    let mut tp2 = Toolpath::new();
    tp2.rapid_to(P3::new(15.0, 15.0, -5.0));
    for i in 0..50 {
        tp2.feed_to(P3::new(10.0 + i as f64 * 0.2, 15.0, 3.0), 600.0);
    }

    let request = SimulationRequest {
        groups: vec![
            SetupSimGroup {
                toolpaths: vec![(
                    ToolpathId(1),
                    "Top".to_string(),
                    Arc::new(tp1),
                    tool.clone(),
                )],
                direction: StockCutDirection::FromTop,
            },
            SetupSimGroup {
                toolpaths: vec![(
                    ToolpathId(2),
                    "Bottom".to_string(),
                    Arc::new(tp2),
                    tool.clone(),
                )],
                direction: StockCutDirection::FromBottom,
            },
        ],
        stock_bbox,
        stock_top_z: 10.0,
        resolution: 0.5,
        model_mesh: None,
    };

    let mut backend = ThreadedComputeBackend::new();
    backend.submit_simulation(request);

    let result = wait_for(&mut backend, Duration::from_secs(10), |msg| {
        matches!(msg, ComputeMessage::Simulation(Ok(_)))
    });
    let result = match result.unwrap() {
        ComputeMessage::Simulation(Ok(r)) => r,
        _ => panic!("expected simulation result"),
    };

    // Checkpoint 0 is after top setup, checkpoint 1 is after bottom setup
    assert_eq!(result.checkpoints.len(), 2);

    // Checkpoint 0 stock should NOT have bottom cuts applied
    let cp0 = &result.checkpoints[0].stock;
    let (r, c) = cp0.z_grid.world_to_cell(15.0, 15.0).unwrap();
    let ray = cp0.z_grid.ray(r, c);
    assert_eq!(ray.len(), 1);
    // Bottom of ray should be at Z=0 (no bottom cut yet)
    assert!(
        ray[0].enter.abs() < 0.01,
        "checkpoint 0 bottom should be Z=0"
    );

    // Checkpoint 1 should have both cuts
    let cp1 = &result.checkpoints[1].stock;
    let (r, c) = cp1.z_grid.world_to_cell(15.0, 15.0).unwrap();
    let ray = cp1.z_grid.ray(r, c);
    assert_eq!(ray.len(), 1);
    assert!(
        ray[0].enter > 2.0,
        "checkpoint 1 should have bottom material removed"
    );

    // Boundary directions are correct
    assert_eq!(result.boundaries[0].direction, StockCutDirection::FromTop);
    assert_eq!(
        result.boundaries[1].direction,
        StockCutDirection::FromBottom
    );
}
