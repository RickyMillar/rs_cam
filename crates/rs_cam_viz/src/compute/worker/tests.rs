use super::*;
use crate::compute::{ComputeBackend, ComputeLane, ComputeMessage, LaneState};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{make_test_flat, make_test_hemisphere};
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
        enriched_mesh: None,
        face_selection: None,
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

    let stock = helpers::feed_optimization_stock(&request).unwrap();
    assert!((stock.z_grid.origin_u - 10.0).abs() < 0.001);
    assert!((stock.z_grid.origin_v - 20.0).abs() < 0.001);
    assert!((stock.stock_bbox.max.z - 12.0).abs() < 0.001);
    assert!((stock.z_grid.cell_size - 1.5875).abs() < 0.001);
}

#[test]
fn feed_optimization_rejects_remaining_stock() {
    let request = sample_request(
        OperationConfig::new_default(OperationType::Pocket),
        StockSource::FromRemainingStock,
    );

    let error = match helpers::feed_optimization_stock(&request) {
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

    let error = match helpers::feed_optimization_stock(&request) {
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
        enriched_mesh: None,
        face_selection: None,
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
        enriched_mesh: None,
        face_selection: None,
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
        enriched_mesh: None,
        face_selection: None,
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
        enriched_mesh: None,
        face_selection: None,
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

fn adaptive_request(id: usize) -> ComputeRequest {
    let mut request = quick_pocket_request(id);
    request.toolpath_name = format!("Adaptive {id}");
    request.operation = OperationConfig::new_default(OperationType::Adaptive);
    request
}

fn profile_request(id: usize) -> ComputeRequest {
    let mut request = quick_pocket_request(id);
    request.toolpath_name = format!("Profile {id}");
    let mut cfg = match OperationConfig::new_default(OperationType::Profile) {
        OperationConfig::Profile(cfg) => cfg,
        _ => unreachable!("default op kind mismatch"),
    };
    cfg.finishing_passes = 1;
    cfg.tab_count = 2;
    request.operation = OperationConfig::Profile(cfg);
    request
}

fn drill_request(id: usize) -> ComputeRequest {
    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    let mut cfg = match OperationConfig::new_default(OperationType::Drill) {
        OperationConfig::Drill(cfg) => cfg,
        _ => unreachable!("default op kind mismatch"),
    };
    cfg.cycle = crate::state::toolpath::DrillCycleType::Peck;
    ComputeRequest {
        toolpath_id: ToolpathId(id),
        toolpath_name: format!("Drill {id}"),
        polygons: Some(Arc::new(vec![
            Polygon2::rectangle(-10.0, -10.0, -6.0, -6.0),
            Polygon2::rectangle(6.0, 6.0, 10.0, 10.0),
        ])),
        mesh: None,
        enriched_mesh: None,
        face_selection: None,
        operation: OperationConfig::Drill(cfg),
        dressups: DressupConfig::default(),
        stock_source: StockSource::Fresh,
        tool,
        safe_z: 10.0,
        prev_tool_radius: None,
        stock_bbox: Some(BoundingBox3 {
            min: P3::new(-20.0, -20.0, -15.0),
            max: P3::new(20.0, 20.0, 10.0),
        }),
        boundary_enabled: false,
        boundary_containment: BoundaryContainment::Center,
        keep_out_footprints: Vec::new(),
        heights: HeightsConfig::default().resolve(10.0, 5.0),
        debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
    }
}

fn steep_shallow_request(id: usize) -> ComputeRequest {
    let tool = ToolConfig::new_default(ToolId(1), ToolType::BallNose);
    let mesh = make_test_hemisphere(20.0, 16);
    ComputeRequest {
        toolpath_id: ToolpathId(id),
        toolpath_name: format!("SteepShallow {id}"),
        polygons: None,
        mesh: Some(Arc::new(mesh)),
        enriched_mesh: None,
        face_selection: None,
        operation: OperationConfig::new_default(OperationType::SteepShallow),
        dressups: DressupConfig::default(),
        stock_source: StockSource::Fresh,
        tool,
        safe_z: 10.0,
        prev_tool_radius: None,
        stock_bbox: Some(BoundingBox3 {
            min: P3::new(-20.0, -20.0, -20.0),
            max: P3::new(20.0, 20.0, 20.0),
        }),
        boundary_enabled: false,
        boundary_containment: BoundaryContainment::Center,
        keep_out_footprints: Vec::new(),
        heights: HeightsConfig::default().resolve(10.0, 5.0),
        debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
    }
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

fn pencil_request(id: usize) -> ComputeRequest {
    let tool = ToolConfig::new_default(ToolId(1), ToolType::BallNose);
    ComputeRequest {
        toolpath_id: ToolpathId(id),
        toolpath_name: format!("Pencil {id}"),
        polygons: None,
        mesh: Some(Arc::new(make_v_groove_mesh(40.0, 6.0, 12.0))),
        enriched_mesh: None,
        face_selection: None,
        operation: OperationConfig::new_default(OperationType::Pencil),
        dressups: DressupConfig::default(),
        stock_source: StockSource::Fresh,
        tool,
        safe_z: 10.0,
        prev_tool_radius: None,
        stock_bbox: Some(BoundingBox3 {
            min: P3::new(0.0, -15.0, -10.0),
            max: P3::new(40.0, 15.0, 10.0),
        }),
        boundary_enabled: false,
        boundary_containment: BoundaryContainment::Center,
        keep_out_footprints: Vec::new(),
        heights: HeightsConfig::default().resolve(10.0, 5.0),
        debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
    }
}

fn scallop_request(id: usize) -> ComputeRequest {
    let tool = ToolConfig::new_default(ToolId(1), ToolType::BallNose);
    let mut cfg = match OperationConfig::new_default(OperationType::Scallop) {
        OperationConfig::Scallop(cfg) => cfg,
        _ => unreachable!("default op kind mismatch"),
    };
    cfg.continuous = true;
    cfg.scallop_height = 0.2;
    cfg.tolerance = 0.2;
    ComputeRequest {
        toolpath_id: ToolpathId(id),
        toolpath_name: format!("Scallop {id}"),
        polygons: None,
        mesh: Some(Arc::new(make_test_hemisphere(20.0, 16))),
        enriched_mesh: None,
        face_selection: None,
        operation: OperationConfig::Scallop(cfg),
        dressups: DressupConfig::default(),
        stock_source: StockSource::Fresh,
        tool,
        safe_z: 20.0,
        prev_tool_radius: None,
        stock_bbox: Some(BoundingBox3 {
            min: P3::new(-25.0, -25.0, -5.0),
            max: P3::new(25.0, 25.0, 25.0),
        }),
        boundary_enabled: false,
        boundary_containment: BoundaryContainment::Center,
        keep_out_footprints: Vec::new(),
        heights: HeightsConfig::default().resolve(20.0, 5.0),
        debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
    }
}

fn ramp_finish_request(id: usize) -> ComputeRequest {
    let tool = ToolConfig::new_default(ToolId(1), ToolType::BallNose);
    let mut cfg = match OperationConfig::new_default(OperationType::RampFinish) {
        OperationConfig::RampFinish(cfg) => cfg,
        _ => unreachable!("default op kind mismatch"),
    };
    cfg.max_stepdown = 2.0;
    cfg.sampling = 2.0;
    cfg.tolerance = 0.2;
    ComputeRequest {
        toolpath_id: ToolpathId(id),
        toolpath_name: format!("Ramp finish {id}"),
        polygons: None,
        mesh: Some(Arc::new(make_test_hemisphere(20.0, 16))),
        enriched_mesh: None,
        face_selection: None,
        operation: OperationConfig::RampFinish(cfg),
        dressups: DressupConfig::default(),
        stock_source: StockSource::Fresh,
        tool,
        safe_z: 20.0,
        prev_tool_radius: None,
        stock_bbox: Some(BoundingBox3 {
            min: P3::new(-25.0, -25.0, -5.0),
            max: P3::new(25.0, 25.0, 25.0),
        }),
        boundary_enabled: false,
        boundary_containment: BoundaryContainment::Center,
        keep_out_footprints: Vec::new(),
        heights: HeightsConfig::default().resolve(20.0, 5.0),
        debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
    }
}

fn spiral_finish_request(id: usize) -> ComputeRequest {
    let tool = ToolConfig::new_default(ToolId(1), ToolType::BallNose);
    let mut cfg = match OperationConfig::new_default(OperationType::SpiralFinish) {
        OperationConfig::SpiralFinish(cfg) => cfg,
        _ => unreachable!("default op kind mismatch"),
    };
    cfg.stepover = 2.0;
    ComputeRequest {
        toolpath_id: ToolpathId(id),
        toolpath_name: format!("Spiral finish {id}"),
        polygons: None,
        mesh: Some(Arc::new(make_test_hemisphere(20.0, 16))),
        enriched_mesh: None,
        face_selection: None,
        operation: OperationConfig::SpiralFinish(cfg),
        dressups: DressupConfig::default(),
        stock_source: StockSource::Fresh,
        tool,
        safe_z: 20.0,
        prev_tool_radius: None,
        stock_bbox: Some(BoundingBox3 {
            min: P3::new(-25.0, -25.0, -5.0),
            max: P3::new(25.0, 25.0, 25.0),
        }),
        boundary_enabled: false,
        boundary_containment: BoundaryContainment::Center,
        keep_out_footprints: Vec::new(),
        heights: HeightsConfig::default().resolve(20.0, 5.0),
        debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
    }
}

fn radial_finish_request(id: usize) -> ComputeRequest {
    let tool = ToolConfig::new_default(ToolId(1), ToolType::BallNose);
    let mut cfg = match OperationConfig::new_default(OperationType::RadialFinish) {
        OperationConfig::RadialFinish(cfg) => cfg,
        _ => unreachable!("default op kind mismatch"),
    };
    cfg.angular_step = 30.0;
    cfg.point_spacing = 2.0;
    ComputeRequest {
        toolpath_id: ToolpathId(id),
        toolpath_name: format!("Radial finish {id}"),
        polygons: None,
        mesh: Some(Arc::new(make_test_flat(80.0))),
        enriched_mesh: None,
        face_selection: None,
        operation: OperationConfig::RadialFinish(cfg),
        dressups: DressupConfig::default(),
        stock_source: StockSource::Fresh,
        tool,
        safe_z: 15.0,
        prev_tool_radius: None,
        stock_bbox: Some(BoundingBox3 {
            min: P3::new(-40.0, -40.0, -5.0),
            max: P3::new(40.0, 40.0, 15.0),
        }),
        boundary_enabled: false,
        boundary_containment: BoundaryContainment::Center,
        keep_out_footprints: Vec::new(),
        heights: HeightsConfig::default().resolve(15.0, 5.0),
        debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
    }
}

fn horizontal_finish_request(id: usize) -> ComputeRequest {
    let tool = ToolConfig::new_default(ToolId(1), ToolType::BallNose);
    let mut cfg = match OperationConfig::new_default(OperationType::HorizontalFinish) {
        OperationConfig::HorizontalFinish(cfg) => cfg,
        _ => unreachable!("default op kind mismatch"),
    };
    cfg.stepover = 3.0;
    ComputeRequest {
        toolpath_id: ToolpathId(id),
        toolpath_name: format!("Horizontal finish {id}"),
        polygons: None,
        mesh: Some(Arc::new(make_test_flat(80.0))),
        enriched_mesh: None,
        face_selection: None,
        operation: OperationConfig::HorizontalFinish(cfg),
        dressups: DressupConfig::default(),
        stock_source: StockSource::Fresh,
        tool,
        safe_z: 15.0,
        prev_tool_radius: None,
        stock_bbox: Some(BoundingBox3 {
            min: P3::new(-40.0, -40.0, -5.0),
            max: P3::new(40.0, 40.0, 15.0),
        }),
        boundary_enabled: false,
        boundary_containment: BoundaryContainment::Center,
        keep_out_footprints: Vec::new(),
        heights: HeightsConfig::default().resolve(15.0, 5.0),
        debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
    }
}

fn project_curve_request(id: usize) -> ComputeRequest {
    let tool = ToolConfig::new_default(ToolId(1), ToolType::BallNose);
    let mut cfg = match OperationConfig::new_default(OperationType::ProjectCurve) {
        OperationConfig::ProjectCurve(cfg) => cfg,
        _ => unreachable!("default op kind mismatch"),
    };
    cfg.depth = 0.75;
    cfg.point_spacing = 1.0;
    ComputeRequest {
        toolpath_id: ToolpathId(id),
        toolpath_name: format!("Project curve {id}"),
        polygons: Some(Arc::new(vec![
            Polygon2::rectangle(-12.0, -12.0, 12.0, 12.0),
            Polygon2::rectangle(-6.0, -4.0, 6.0, 4.0),
        ])),
        mesh: Some(Arc::new(make_test_hemisphere(20.0, 16))),
        enriched_mesh: None,
        face_selection: None,
        operation: OperationConfig::ProjectCurve(cfg),
        dressups: DressupConfig::default(),
        stock_source: StockSource::Fresh,
        tool,
        safe_z: 15.0,
        prev_tool_radius: None,
        stock_bbox: Some(BoundingBox3 {
            min: P3::new(-25.0, -25.0, -5.0),
            max: P3::new(25.0, 25.0, 25.0),
        }),
        boundary_enabled: false,
        boundary_containment: BoundaryContainment::Center,
        keep_out_footprints: Vec::new(),
        heights: HeightsConfig::default().resolve(15.0, 5.0),
        debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
    }
}

fn assert_cutting_moves_are_semantically_covered(result: &ToolpathResult) {
    let semantic_trace = result
        .semantic_trace
        .as_ref()
        .expect("semantic trace should be attached");
    for (move_idx, mv) in result.toolpath.moves.iter().enumerate() {
        let is_cut = matches!(
            mv.move_type,
            rs_cam_core::toolpath::MoveType::Linear { .. }
                | rs_cam_core::toolpath::MoveType::ArcCW { .. }
                | rs_cam_core::toolpath::MoveType::ArcCCW { .. }
        );
        if !is_cut {
            continue;
        }
        assert!(
            semantic_trace.items.iter().any(|item| {
                item.kind != rs_cam_core::semantic_trace::ToolpathSemanticKind::Operation
                    && item.move_start.is_some_and(|start| start <= move_idx)
                    && item.move_end.is_some_and(|end| move_idx <= end)
            }),
            "expected move {move_idx} to be covered by a non-root semantic item"
        );
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
            toolpaths: vec![SetupSimToolpath {
                id: ToolpathId(99),
                name: "Long Sim".to_string(),
                toolpath: Arc::new(toolpath),
                tool,
                semantic_trace: None,
            }],
            direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
        }],
        stock_bbox: BoundingBox3 {
            min: P3::new(0.0, 0.0, -2.0),
            max: P3::new(100.0, 100.0, 10.0),
        },
        stock_top_z: 10.0,
        resolution: 0.5,
        metric_options: rs_cam_core::simulation_cut::SimulationMetricOptions::default(),
        spindle_rpm: 18_000,
        rapid_feed_mm_min: 5_000.0,
        model_mesh: None,
    }
}

fn small_simulation_request_with_metrics(enabled: bool) -> SimulationRequest {
    let tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    let mut toolpath = Toolpath::new();
    toolpath.rapid_to(P3::new(2.0, 2.0, 8.0));
    toolpath.feed_to(P3::new(2.0, 2.0, 4.0), 400.0);
    toolpath.feed_to(P3::new(12.0, 2.0, 4.0), 400.0);
    toolpath.rapid_to(P3::new(12.0, 2.0, 8.0));

    SimulationRequest {
        groups: vec![SetupSimGroup {
            toolpaths: vec![SetupSimToolpath {
                id: ToolpathId(1),
                name: "Metrics".to_string(),
                toolpath: Arc::new(toolpath),
                tool,
                semantic_trace: None,
            }],
            direction: rs_cam_core::dexel_stock::StockCutDirection::FromTop,
        }],
        stock_bbox: BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(20.0, 20.0, 10.0),
        },
        stock_top_z: 10.0,
        resolution: 0.5,
        metric_options: rs_cam_core::simulation_cut::SimulationMetricOptions { enabled },
        spindle_rpm: 18_000,
        rapid_feed_mm_min: 5_000.0,
        model_mesh: None,
    }
}

fn small_simulation_request_with_semantic_metrics(enabled: bool) -> SimulationRequest {
    let mut req = small_simulation_request_with_metrics(enabled);
    let recorder = rs_cam_core::semantic_trace::ToolpathSemanticRecorder::new("Metrics", "Metrics");
    let root = recorder.root_context();
    let op = root.start_item(
        rs_cam_core::semantic_trace::ToolpathSemanticKind::Operation,
        "Metrics",
    );
    let pass = op.context().start_item(
        rs_cam_core::semantic_trace::ToolpathSemanticKind::Pass,
        "Pass 1",
    );
    if let Some(toolpath) = req.groups.first().and_then(|group| group.toolpaths.first()) {
        pass.bind_to_toolpath(&toolpath.toolpath, 0, toolpath.toolpath.moves.len());
    }
    let semantic_trace = Arc::new(recorder.finish());
    req.groups[0].toolpaths[0].semantic_trace = Some(semantic_trace);
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
    assert!(pass.params.values.contains_key("step_count"));
    assert!(pass.params.values.contains_key("yield_ratio"));

    if let Some(path) = result.debug_trace_path.as_ref() {
        std::fs::remove_file(path).ok();
    }
}

#[test]
fn adaptive_semantic_trace_records_runtime_structure() {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut request = adaptive_request(92);
    request.debug_options.enabled = true;

    let result = super::execute::run_compute(&request, &cancel)
        .result
        .expect("adaptive debug compute should succeed");
    let semantic_trace = result
        .semantic_trace
        .as_ref()
        .expect("semantic trace should be attached");

    assert!(
        semantic_trace.items.iter().any(
            |item| item.kind == rs_cam_core::semantic_trace::ToolpathSemanticKind::SlotClearing
        ),
        "expected slot-clearing semantics"
    );
    assert!(
        semantic_trace
            .items
            .iter()
            .any(|item| item.kind == rs_cam_core::semantic_trace::ToolpathSemanticKind::Cleanup),
        "expected cleanup semantics"
    );
    let pass = semantic_trace
        .items
        .iter()
        .find(|item| item.label.starts_with("Adaptive pass "))
        .expect("expected adaptive pass item");
    assert!(pass.params.values.contains_key("step_count"));
    assert!(pass.params.values.contains_key("exit_reason"));
    assert_cutting_moves_are_semantically_covered(&result);
}

#[test]
fn profile_semantic_trace_records_depth_and_finish_structure() {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut request = profile_request(93);
    request.debug_options.enabled = true;

    let result = super::execute::run_compute(&request, &cancel)
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
            .any(|item| item.kind == rs_cam_core::semantic_trace::ToolpathSemanticKind::DepthLevel),
        "expected depth-level semantics"
    );
    assert!(
        semantic_trace
            .items
            .iter()
            .any(|item| item.kind == rs_cam_core::semantic_trace::ToolpathSemanticKind::FinishPass),
        "expected finish-pass semantics"
    );
    assert_cutting_moves_are_semantically_covered(&result);
}

#[test]
fn drill_semantic_trace_records_cycle_children() {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut request = drill_request(94);
    request.debug_options.enabled = true;

    let result = super::execute::run_compute(&request, &cancel)
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
            .any(|item| item.kind == rs_cam_core::semantic_trace::ToolpathSemanticKind::Hole),
        "expected hole semantics"
    );
    assert!(
        semantic_trace
            .items
            .iter()
            .any(|item| item.kind == rs_cam_core::semantic_trace::ToolpathSemanticKind::Cycle),
        "expected cycle semantics"
    );
    assert_cutting_moves_are_semantically_covered(&result);
}

#[test]
fn steep_shallow_semantic_trace_splits_steep_and_shallow_regions() {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut request = steep_shallow_request(95);
    request.debug_options.enabled = true;

    let result = super::execute::run_compute(&request, &cancel)
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
            .any(|item| item.label == "Steep contours"),
        "expected steep partition"
    );
    assert!(
        semantic_trace
            .items
            .iter()
            .any(|item| item.label == "Shallow raster"),
        "expected shallow partition"
    );
    assert_cutting_moves_are_semantically_covered(&result);
}

#[test]
fn pencil_semantic_trace_records_chain_and_offset_pass_structure() {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut request = pencil_request(96);
    request.debug_options.enabled = true;

    let result = super::execute::run_compute(&request, &cancel)
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
            .any(|item| item.kind == rs_cam_core::semantic_trace::ToolpathSemanticKind::Chain),
        "expected chain semantics"
    );
    assert!(
        semantic_trace.items.iter().any(|item| {
            item.kind == rs_cam_core::semantic_trace::ToolpathSemanticKind::Centerline
                || item.kind == rs_cam_core::semantic_trace::ToolpathSemanticKind::OffsetPass
        }),
        "expected centerline or offset-pass semantics"
    );
    assert_cutting_moves_are_semantically_covered(&result);
}

#[test]
fn scallop_semantic_trace_records_band_and_ring_structure() {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut request = scallop_request(97);
    request.debug_options.enabled = true;

    let result = super::execute::run_compute(&request, &cancel)
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
            .any(|item| item.kind == rs_cam_core::semantic_trace::ToolpathSemanticKind::Band),
        "expected band semantics"
    );
    let ring = semantic_trace
        .items
        .iter()
        .find(|item| item.kind == rs_cam_core::semantic_trace::ToolpathSemanticKind::Ring)
        .expect("expected ring semantics");
    assert!(ring.params.values.contains_key("ring_index"));
    assert!(ring.move_start.is_some() && ring.move_end.is_some());
    assert_cutting_moves_are_semantically_covered(&result);
}

#[test]
fn ramp_finish_semantic_trace_records_terrace_and_ramp_structure() {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut request = ramp_finish_request(98);
    request.debug_options.enabled = true;

    let result = super::execute::run_compute(&request, &cancel)
        .result
        .expect("ramp finish debug compute should succeed");
    let semantic_trace = result
        .semantic_trace
        .as_ref()
        .expect("semantic trace should be attached");

    assert!(
        semantic_trace.items.iter().any(|item| item.kind
            == rs_cam_core::semantic_trace::ToolpathSemanticKind::Band
            && item.label.starts_with("Terrace ")),
        "expected terrace band semantics"
    );
    let ramp = semantic_trace
        .items
        .iter()
        .find(|item| item.kind == rs_cam_core::semantic_trace::ToolpathSemanticKind::Ramp)
        .expect("expected ramp semantics");
    assert!(ramp.params.values.contains_key("upper_z"));
    assert!(ramp.params.values.contains_key("lower_z"));
    assert_cutting_moves_are_semantically_covered(&result);
}

#[test]
fn spiral_finish_semantic_trace_records_band_and_ring_structure() {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut request = spiral_finish_request(99);
    request.debug_options.enabled = true;

    let result = super::execute::run_compute(&request, &cancel)
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
            .any(|item| item.label == "Spiral band"),
        "expected spiral band semantics"
    );
    let ring = semantic_trace
        .items
        .iter()
        .find(|item| item.kind == rs_cam_core::semantic_trace::ToolpathSemanticKind::Ring)
        .expect("expected ring semantics");
    assert!(ring.params.values.contains_key("radius_mm"));
    assert_cutting_moves_are_semantically_covered(&result);
}

#[test]
fn radial_finish_semantic_trace_records_ray_angles() {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut request = radial_finish_request(100);
    request.debug_options.enabled = true;

    let result = super::execute::run_compute(&request, &cancel)
        .result
        .expect("radial finish debug compute should succeed");
    let semantic_trace = result
        .semantic_trace
        .as_ref()
        .expect("semantic trace should be attached");

    let ray = semantic_trace
        .items
        .iter()
        .find(|item| item.kind == rs_cam_core::semantic_trace::ToolpathSemanticKind::Ray)
        .expect("expected ray semantics");
    assert!(ray.params.values.contains_key("angle_deg"));
    assert!(ray.params.values.contains_key("direction"));
    assert_cutting_moves_are_semantically_covered(&result);
}

#[test]
fn horizontal_finish_semantic_trace_records_slice_passes() {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut request = horizontal_finish_request(101);
    request.debug_options.enabled = true;

    let result = super::execute::run_compute(&request, &cancel)
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
            .any(|item| item.kind == rs_cam_core::semantic_trace::ToolpathSemanticKind::Slice),
        "expected slice semantics"
    );
    assert!(
        semantic_trace
            .items
            .iter()
            .any(|item| item.kind == rs_cam_core::semantic_trace::ToolpathSemanticKind::Pass),
        "expected pass semantics"
    );
    assert_cutting_moves_are_semantically_covered(&result);
}

#[test]
fn project_curve_semantic_trace_records_source_curve_groups() {
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let mut request = project_curve_request(102);
    request.debug_options.enabled = true;

    let result = super::execute::run_compute(&request, &cancel)
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
            .any(|item| item.label.starts_with("Source curve ")),
        "expected source-curve grouping"
    );
    let curve = semantic_trace
        .items
        .iter()
        .find(|item| item.label.starts_with("Projected curve "))
        .expect("expected projected curve semantics");
    assert!(curve.params.values.contains_key("source_curve_index"));
    assert!(curve.params.values.contains_key("depth"));
    assert_cutting_moves_are_semantically_covered(&result);
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
                toolpaths: vec![SetupSimToolpath {
                    id: ToolpathId(1),
                    name: "Top Cut".to_string(),
                    toolpath: Arc::new(top_tp),
                    tool: tool.clone(),
                    semantic_trace: None,
                }],
                direction: StockCutDirection::FromTop,
            },
            SetupSimGroup {
                toolpaths: vec![SetupSimToolpath {
                    id: ToolpathId(2),
                    name: "Bottom Cut".to_string(),
                    toolpath: Arc::new(bottom_tp),
                    tool: tool.clone(),
                    semantic_trace: None,
                }],
                direction: StockCutDirection::FromBottom,
            },
        ],
        stock_bbox,
        stock_top_z: 20.0,
        resolution: 0.5,
        metric_options: rs_cam_core::simulation_cut::SimulationMetricOptions::default(),
        spindle_rpm: 18_000,
        rapid_feed_mm_min: 5_000.0,
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
                toolpaths: vec![SetupSimToolpath {
                    id: ToolpathId(1),
                    name: "Top".to_string(),
                    toolpath: Arc::new(tp1),
                    tool: tool.clone(),
                    semantic_trace: None,
                }],
                direction: StockCutDirection::FromTop,
            },
            SetupSimGroup {
                toolpaths: vec![SetupSimToolpath {
                    id: ToolpathId(2),
                    name: "Bottom".to_string(),
                    toolpath: Arc::new(tp2),
                    tool: tool.clone(),
                    semantic_trace: None,
                }],
                direction: StockCutDirection::FromBottom,
            },
        ],
        stock_bbox,
        stock_top_z: 10.0,
        resolution: 0.5,
        metric_options: rs_cam_core::simulation_cut::SimulationMetricOptions::default(),
        spindle_rpm: 18_000,
        rapid_feed_mm_min: 5_000.0,
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

#[test]
fn simulation_metrics_capture_emits_cut_trace_and_artifact() {
    let mut backend = ThreadedComputeBackend::new();
    backend.submit_simulation(small_simulation_request_with_metrics(true));

    let result = wait_for(&mut backend, Duration::from_secs(10), |msg| {
        matches!(msg, ComputeMessage::Simulation(Ok(_)))
    })
    .expect("simulation result");
    let result = match result {
        ComputeMessage::Simulation(Ok(result)) => result,
        _ => panic!("expected successful simulation"),
    };

    let trace = result.cut_trace.as_ref().expect("cut trace");
    assert!(trace.summary.sample_count > 0);
    assert!(trace.summary.total_runtime_s > 0.0);
    assert!(
        trace
            .toolpath_summaries
            .iter()
            .any(|summary| summary.toolpath_id == 1)
    );
    if let Some(path) = result.cut_trace_path.as_ref() {
        assert!(path.exists(), "expected cut trace artifact to exist");
        std::fs::remove_file(path).ok();
    } else {
        panic!("expected cut trace artifact path");
    }
}

#[test]
fn simulation_metrics_capture_emits_semantic_cut_summaries() {
    let mut backend = ThreadedComputeBackend::new();
    backend.submit_simulation(small_simulation_request_with_semantic_metrics(true));

    let result = wait_for(&mut backend, Duration::from_secs(10), |msg| {
        matches!(msg, ComputeMessage::Simulation(Ok(_)))
    })
    .expect("simulation result");
    let result = match result {
        ComputeMessage::Simulation(Ok(result)) => result,
        _ => panic!("expected successful simulation"),
    };

    let trace = result.cut_trace.as_ref().expect("cut trace");
    let summary = trace
        .semantic_summaries
        .iter()
        .find(|summary| summary.toolpath_id == 1)
        .expect("semantic cut summary");
    assert_eq!(summary.label, "Pass 1");
    assert!(summary.sample_count > 0);
    assert!(summary.average_mrr_mm3_s >= 0.0);
    assert!(summary.representative_sample_index <= summary.sample_count);

    if let Some(path) = result.cut_trace_path.as_ref() {
        std::fs::remove_file(path).ok();
    }
}
