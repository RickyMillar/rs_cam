//! F-015 regression — op-precondition static validation rules for
//! rest machining, drill, and project_curve must surface as
//! `Severity::Blocking` diagnostics through
//! `ProjectSession::diagnose_toolpath_with_trace` so the MCP
//! `add_toolpath` / `set_toolpath_param` `diagnostic_delta` carries them
//! before generation.
//!
//! Before this fix the three preconditions only fired at generate time
//! (`compute::execute` returned `OperationError::MissingGeometry` /
//! `OperationError::Other(...)`), leaving the GUI / MCP in a wedged
//! state where the params panel said "OK" but Generate failed.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{
    DrillConfig, PocketConfig, ProjectCurveConfig, RestConfig,
};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::diagnostics::{Diagnostic, Severity, ids};
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::mesh::make_test_flat;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};

// ── helpers ─────────────────────────────────────────────────────────

fn unit_square() -> Polygon2 {
    Polygon2::new(vec![
        P2::new(-1.0, -1.0),
        P2::new(1.0, -1.0),
        P2::new(1.0, 1.0),
        P2::new(-1.0, 1.0),
    ])
}

fn polygon_model(id: usize) -> LoadedModel {
    LoadedModel {
        id,
        name: format!("curve_{id}"),
        mesh: None,
        polygons: Some(Arc::new(vec![unit_square()])),
        path: PathBuf::from(format!("synthetic://curve_{id}.svg")),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

fn mesh_model(id: usize) -> LoadedModel {
    // Minimal flat square so `mesh.is_some()` and `bbox()` resolves;
    // geometry isn't exercised by the diagnose path under test.
    let mesh = make_test_flat(10.0);
    LoadedModel {
        id,
        name: format!("surface_{id}"),
        mesh: Some(Arc::new(mesh)),
        polygons: None,
        path: PathBuf::from(format!("synthetic://surface_{id}.stl")),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

fn make_tp(name: &str, op: OperationConfig, tool_id: usize, model_id: usize) -> ToolpathConfig {
    ToolpathConfig {
        id: 0,
        name: name.to_owned(),
        enabled: true,
        operation: op,
        dressups: DressupConfig::default(),
        heights: HeightsConfig::default(),
        tool_id,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::Fresh,
        coolant: CoolantMode::Off,
        face_selection: None,
        debug_options: ToolpathDebugOptions::default(),
    }
}

fn has_id(diags: &[Diagnostic], id: &str) -> bool {
    diags.iter().any(|d| d.id.as_str() == id)
}

// ── Rest machining (Acceptance test #1) ─────────────────────────────

#[test]
fn rest_op_without_prior_enabled_tool_surfaces_blocking_diagnostic() {
    let mut session = ProjectSession::new_empty();
    let mut t_small = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    t_small.diameter = 3.0;
    let mut t_large = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    t_large.diameter = 6.0;
    session.add_tool(t_small);
    session.add_tool(t_large);
    let mid = session.add_model(polygon_model(0));

    // Rest op with prev_tool_id = the 6 mm tool, but nothing earlier in
    // the setup uses it. Generation will fail at runtime — diagnose
    // must surface that before that point.
    let rest_cfg = RestConfig {
        prev_tool_id: Some(ToolId(1)),
        ..RestConfig::default()
    };
    let idx = session
        .add_toolpath(
            0,
            make_tp("rest_dangler", OperationConfig::Rest(rest_cfg), 0, mid),
        )
        .unwrap();

    let diags = session.diagnose_toolpath_with_trace(idx, None).unwrap();
    assert!(
        has_id(&diags, ids::PRECOND_REST_NO_PRIOR),
        "expected PRECOND_REST_NO_PRIOR, got {:?}",
        diags.iter().map(|d| d.id.as_str()).collect::<Vec<_>>()
    );
    let diag = diags
        .iter()
        .find(|d| d.id.as_str() == ids::PRECOND_REST_NO_PRIOR)
        .unwrap();
    assert_eq!(diag.severity, Severity::Blocking);
}

#[test]
fn rest_op_without_prev_tool_id_surfaces_blocking_diagnostic() {
    let mut session = ProjectSession::new_empty();
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 3.0;
    session.add_tool(tool);
    let mid = session.add_model(polygon_model(0));

    let rest_cfg = RestConfig {
        prev_tool_id: None,
        ..RestConfig::default()
    };
    let idx = session
        .add_toolpath(
            0,
            make_tp("rest_no_prev", OperationConfig::Rest(rest_cfg), 0, mid),
        )
        .unwrap();

    let diags = session.diagnose_toolpath_with_trace(idx, None).unwrap();
    assert!(
        has_id(&diags, ids::PRECOND_REST_PREV_TOOL_MISSING),
        "expected PRECOND_REST_PREV_TOOL_MISSING, got {:?}",
        diags.iter().map(|d| d.id.as_str()).collect::<Vec<_>>()
    );
}

#[test]
fn rest_op_with_correct_prior_is_silent() {
    let mut session = ProjectSession::new_empty();
    let mut t_small = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    t_small.diameter = 3.0;
    let mut t_large = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    t_large.diameter = 6.0;
    session.add_tool(t_small);
    session.add_tool(t_large);
    let mid = session.add_model(polygon_model(0));

    // Prior op uses the large tool.
    session
        .add_toolpath(
            0,
            make_tp(
                "rough",
                OperationConfig::Pocket(PocketConfig::default()),
                1,
                mid,
            ),
        )
        .unwrap();
    // Rest op references it.
    let rest_cfg = RestConfig {
        prev_tool_id: Some(ToolId(1)),
        ..RestConfig::default()
    };
    let idx = session
        .add_toolpath(
            0,
            make_tp("rest_clean", OperationConfig::Rest(rest_cfg), 0, mid),
        )
        .unwrap();

    let diags = session.diagnose_toolpath_with_trace(idx, None).unwrap();
    assert!(
        !has_id(&diags, ids::PRECOND_REST_NO_PRIOR),
        "rest precondition must clear when a prior enabled op uses the prev tool: {:?}",
        diags.iter().map(|d| d.id.as_str()).collect::<Vec<_>>()
    );
    assert!(!has_id(&diags, ids::PRECOND_REST_PREV_TOOL_MISSING));
    assert!(!has_id(&diags, ids::PRECOND_REST_PREV_TOOL_NOT_LARGER));
}

// ── ProjectCurve (Acceptance test #2) ───────────────────────────────

#[test]
fn project_curve_in_single_model_project_surfaces_blocking_diagnostic() {
    // Project has only a surface STL — no curve polygons anywhere.
    let mut session = ProjectSession::new_empty();
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 3.0;
    session.add_tool(tool);
    let mid = session.add_model(mesh_model(0));

    let cfg = ProjectCurveConfig::default();
    let idx = session
        .add_toolpath(
            0,
            make_tp("pc_no_curve", OperationConfig::ProjectCurve(cfg), 0, mid),
        )
        .unwrap();

    let diags = session.diagnose_toolpath_with_trace(idx, None).unwrap();
    assert!(
        has_id(&diags, ids::PRECOND_PROJECT_CURVE_NO_CURVE),
        "expected PRECOND_PROJECT_CURVE_NO_CURVE, got {:?}",
        diags.iter().map(|d| d.id.as_str()).collect::<Vec<_>>()
    );
    let diag = diags
        .iter()
        .find(|d| d.id.as_str() == ids::PRECOND_PROJECT_CURVE_NO_CURVE)
        .unwrap();
    assert_eq!(diag.severity, Severity::Blocking);
}

#[test]
fn project_curve_without_any_surface_mesh_surfaces_blocking_diagnostic() {
    // Project has only a polygon model — no STL anywhere.
    let mut session = ProjectSession::new_empty();
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 3.0;
    session.add_tool(tool);
    let mid = session.add_model(polygon_model(0));

    let cfg = ProjectCurveConfig::default();
    let idx = session
        .add_toolpath(
            0,
            make_tp("pc_no_surface", OperationConfig::ProjectCurve(cfg), 0, mid),
        )
        .unwrap();

    let diags = session.diagnose_toolpath_with_trace(idx, None).unwrap();
    assert!(
        has_id(&diags, ids::PRECOND_PROJECT_CURVE_NO_SURFACE),
        "expected PRECOND_PROJECT_CURVE_NO_SURFACE, got {:?}",
        diags.iter().map(|d| d.id.as_str()).collect::<Vec<_>>()
    );
}

#[test]
fn project_curve_with_curve_and_surface_models_is_silent() {
    let mut session = ProjectSession::new_empty();
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 3.0;
    session.add_tool(tool);
    let curve_id = session.add_model(polygon_model(0));
    let _surface_id = session.add_model(mesh_model(1));

    let cfg = ProjectCurveConfig::default();
    let idx = session
        .add_toolpath(
            0,
            make_tp(
                "pc_happy",
                OperationConfig::ProjectCurve(cfg),
                0,
                curve_id,
            ),
        )
        .unwrap();

    let diags = session.diagnose_toolpath_with_trace(idx, None).unwrap();
    let pc_diags: Vec<_> = diags
        .iter()
        .filter(|d| d.id.as_str().starts_with("precondition.project_curve"))
        .collect();
    assert!(
        pc_diags.is_empty(),
        "happy-path project_curve must emit no precondition diagnostics, got {pc_diags:?}"
    );
}

// ── Drill (Acceptance test #3) ──────────────────────────────────────

#[test]
fn drill_op_against_mesh_only_model_surfaces_blocking_diagnostic() {
    // Drill needs polygons (hole centroids). Mesh-only model fails at
    // generate time with "No hole positions found"; the precondition
    // adapter must surface that before that point.
    let mut session = ProjectSession::new_empty();
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 3.0;
    session.add_tool(tool);
    let mid = session.add_model(mesh_model(0));

    let idx = session
        .add_toolpath(
            0,
            make_tp(
                "drill_no_holes",
                OperationConfig::Drill(DrillConfig::default()),
                0,
                mid,
            ),
        )
        .unwrap();

    let diags = session.diagnose_toolpath_with_trace(idx, None).unwrap();
    assert!(
        has_id(&diags, ids::PRECOND_DRILL_NO_HOLES),
        "expected PRECOND_DRILL_NO_HOLES, got {:?}",
        diags.iter().map(|d| d.id.as_str()).collect::<Vec<_>>()
    );
    let diag = diags
        .iter()
        .find(|d| d.id.as_str() == ids::PRECOND_DRILL_NO_HOLES)
        .unwrap();
    assert_eq!(diag.severity, Severity::Blocking);
}

#[test]
fn drill_op_against_polygon_model_is_silent() {
    let mut session = ProjectSession::new_empty();
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 3.0;
    session.add_tool(tool);
    let mid = session.add_model(polygon_model(0));

    let idx = session
        .add_toolpath(
            0,
            make_tp(
                "drill_happy",
                OperationConfig::Drill(DrillConfig::default()),
                0,
                mid,
            ),
        )
        .unwrap();

    let diags = session.diagnose_toolpath_with_trace(idx, None).unwrap();
    assert!(
        !has_id(&diags, ids::PRECOND_DRILL_NO_HOLES),
        "polygon-bearing drill model must not fire precondition: {:?}",
        diags.iter().map(|d| d.id.as_str()).collect::<Vec<_>>()
    );
}
