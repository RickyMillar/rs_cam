//! F-015 regression — op-precondition static validation rules for
//! drill and project_curve must surface as
//! `Severity::Blocking` diagnostics through
//! `ProjectSession::diagnose_toolpath_with_trace` so the MCP
//! `add_toolpath` / `set_toolpath_param` `diagnostic_delta` carries them
//! before generation.
//!
//! Before this fix the preconditions only fired at generate time
//! (`compute::execute` returned `OperationError::MissingGeometry` /
//! `OperationError::Other(...)`), leaving the GUI / MCP in a wedged
//! state where the params panel said "OK" but Generate failed.
//!
//! The rest machining cases (acceptance test #1) went with the Rest
//! operation on 2026-10-14 (ruling R3).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::ids::ToolpathId;
use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{DrillConfig, ProjectCurveConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::diagnostics::{Diagnostic, Severity, ids};
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::mesh::make_test_flat;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;

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
        id: rs_cam_core::ids::ModelId::from_raw(id),
        name: format!("curve_{id}"),
        mesh: None,
        items: Arc::new(rs_cam_core::selection::ItemSet::from_polygons(vec![
            unit_square(),
        ])),
        path: PathBuf::from(format!("synthetic://curve_{id}.svg")),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    }
}

/// A drawing with one circle — what a DXF with a `CIRCLE` imports to (one
/// item since P2.1). The `Drill` precondition reads the targets, not the
/// polygons (G-DRILLCENTROID).
fn target_model(id: usize) -> LoadedModel {
    use rs_cam_core::selection::{Item, ItemSet, Shape, SourceRef};
    let mut model = polygon_model(id);
    let mut items: Vec<Item> = model.items.iter().cloned().collect();
    items.push(
        Item::new(
            SourceRef::ToolpathModel,
            Some("holes".into()),
            Shape::circle(rs_cam_core::geo::P2::new(0.0, 0.0), 3.0),
        )
        .unwrap(),
    );
    model.items = Arc::new(ItemSet::new(items));
    model
}

fn mesh_model(id: usize) -> LoadedModel {
    // Minimal flat square so `mesh.is_some()` and `bbox()` resolves;
    // geometry isn't exercised by the diagnose path under test.
    let mesh = make_test_flat(10.0);
    LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(id),
        name: format!("surface_{id}"),
        mesh: Some(Arc::new(mesh)),
        items: Default::default(),
        path: PathBuf::from(format!("synthetic://surface_{id}.stl")),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    }
}

fn make_tp(name: &str, op: OperationConfig, tool_id: usize, model_id: usize) -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        id: ToolpathId::from_raw(0),
        name: name.to_owned(),
        enabled: true,
        operation: op,
        dressups: DressupConfig::default(),
        heights: HeightsConfig::default(),
        tool_id: rs_cam_core::ids::ToolId::from_raw(tool_id),
        model_id: rs_cam_core::ids::ModelId::from_raw(model_id),
        pre_gcode: None,
        post_gcode: None,
        boundary: BoundaryConfig::default(),
        boundary_inherit: true,
        stock_source: StockSource::Fresh,
        coolant: CoolantMode::Off,
        debug_options: ToolpathDebugOptions::default(),
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

fn has_id(diags: &[Diagnostic], id: &str) -> bool {
    diags.iter().any(|d| d.id.as_str() == id)
}

// ── ProjectCurve (Acceptance test #2) ───────────────────────────────

#[test]
fn project_curve_in_single_model_project_surfaces_blocking_diagnostic() {
    // Project has only a surface STL — no curve polygons anywhere.
    let mut builder = ProjectSessionBuilder::new();
    let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 3.0;
    builder.add_tool(tool);
    let mid = builder.add_model(mesh_model(0));

    let cfg = ProjectCurveConfig::default();
    let idx = builder
        .add_toolpath(
            0,
            make_tp(
                "pc_no_curve",
                OperationConfig::ProjectCurve(cfg),
                0,
                mid.raw(),
            ),
        )
        .unwrap();
    let session = builder.build();

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
    let mut builder = ProjectSessionBuilder::new();
    let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 3.0;
    builder.add_tool(tool);
    let mid = builder.add_model(polygon_model(0));

    let cfg = ProjectCurveConfig::default();
    let idx = builder
        .add_toolpath(
            0,
            make_tp(
                "pc_no_surface",
                OperationConfig::ProjectCurve(cfg),
                0,
                mid.raw(),
            ),
        )
        .unwrap();
    let session = builder.build();

    let diags = session.diagnose_toolpath_with_trace(idx, None).unwrap();
    assert!(
        has_id(&diags, ids::PRECOND_PROJECT_CURVE_NO_SURFACE),
        "expected PRECOND_PROJECT_CURVE_NO_SURFACE, got {:?}",
        diags.iter().map(|d| d.id.as_str()).collect::<Vec<_>>()
    );
}

#[test]
fn project_curve_with_curve_and_surface_models_is_silent() {
    let mut builder = ProjectSessionBuilder::new();
    let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 3.0;
    builder.add_tool(tool);
    let curve_id = builder.add_model(polygon_model(0));
    let _surface_id = builder.add_model(mesh_model(1));

    let cfg = ProjectCurveConfig::default();
    let idx = builder
        .add_toolpath(
            0,
            make_tp(
                "pc_happy",
                OperationConfig::ProjectCurve(cfg),
                0,
                curve_id.raw(),
            ),
        )
        .unwrap();
    let session = builder.build();

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
    let mut builder = ProjectSessionBuilder::new();
    let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 3.0;
    builder.add_tool(tool);
    let mid = builder.add_model(mesh_model(0));

    let idx = builder
        .add_toolpath(
            0,
            make_tp(
                "drill_no_holes",
                OperationConfig::Drill(DrillConfig::default()),
                0,
                mid.raw(),
            ),
        )
        .unwrap();
    let session = builder.build();

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
fn drill_op_against_target_bearing_model_is_silent() {
    let mut builder = ProjectSessionBuilder::new();
    let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 3.0;
    builder.add_tool(tool);
    let mid = builder.add_model(target_model(0));

    let idx = builder
        .add_toolpath(
            0,
            make_tp(
                "drill_happy",
                OperationConfig::Drill(DrillConfig::default()),
                0,
                mid.raw(),
            ),
        )
        .unwrap();
    let session = builder.build();

    let diags = session.diagnose_toolpath_with_trace(idx, None).unwrap();
    assert!(
        !has_id(&diags, ids::PRECOND_DRILL_NO_HOLES),
        "target-bearing drill model must not fire precondition: {:?}",
        diags.iter().map(|d| d.id.as_str()).collect::<Vec<_>>()
    );
}

/// G-DRILLCENTROID (UX-R03-004): a drawing with closed shapes but no
/// circles or points is NOT a hole source. Pre-fix this was silent and the
/// generator drilled the centroid of every polygon.
#[test]
fn drill_op_against_polygon_only_model_fires_no_targets() {
    let mut builder = ProjectSessionBuilder::new();
    let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 3.0;
    builder.add_tool(tool);
    let mid = builder.add_model(polygon_model(0));

    let idx = builder
        .add_toolpath(
            0,
            make_tp(
                "drill_outline_only",
                OperationConfig::Drill(DrillConfig::default()),
                0,
                mid.raw(),
            ),
        )
        .unwrap();
    let session = builder.build();

    let diags = session.diagnose_toolpath_with_trace(idx, None).unwrap();
    let diag = diags
        .iter()
        .find(|d| d.id.as_str() == ids::PRECOND_DRILL_NO_HOLES)
        .unwrap_or_else(|| {
            panic!(
                "polygon-only drill model must fire PRECOND_DRILL_NO_HOLES, got {:?}",
                diags.iter().map(|d| d.id.as_str()).collect::<Vec<_>>()
            )
        });
    assert_eq!(diag.severity, Severity::Blocking);
    // Geometry selection P2.3c: the message is the generator's own
    // selection refusal (`Empty`), which names the skipped loops and the fix.
    assert!(
        diag.message.starts_with("Drill: ")
            && diag.message.contains("closed loop")
            && diag.message.contains("Fix: "),
        "got {:?}",
        diag.message
    );
}
