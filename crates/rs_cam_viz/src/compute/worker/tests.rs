use super::*;
use crate::compute::{ComputeBackend, ComputeLane, ComputeMessage, LaneState};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::make_test_flat;
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
        toolpaths: vec![(
            ToolpathId(99),
            "Long Sim".to_string(),
            Arc::new(toolpath),
            tool,
        )],
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
                result: Ok(_)
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
                }) => {
                    saw_cancelled = true;
                }
                ComputeMessage::Toolpath(ComputeResult {
                    toolpath_id: ToolpathId(3),
                    result: Ok(_),
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
