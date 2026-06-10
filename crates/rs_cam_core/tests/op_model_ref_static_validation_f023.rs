//! F-023 regression — toolpath `model_id` references that don't
//! resolve against the loaded project must surface as a
//! `Severity::Blocking` `ref.model_missing` diagnostic through
//! [`rs_cam_core::session::ProjectSession::diagnose_toolpath_with_trace`].
//!
//! Before this fix the check lived in the viz-side `validate_toolpath`
//! function — the GUI "Selected model missing" banner fired correctly,
//! but the MCP `add_toolpath` / `set_toolpath_param` `diagnostic_delta`
//! envelope (and `get_toolpath_diagnostics` / `get_project_diagnostics`)
//! silently dropped it on the floor, leaving agents driving the
//! workflow with no way to detect the wedge until generate-time.
//!
//! The shape matches F-015's `op_precondition_static_validation_f015`
//! file: same session-level construction, same end-to-end exercise of
//! the orchestrator.

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
use rs_cam_core::compute::operation_configs::{FaceConfig, PocketConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::diagnostics::{Diagnostic, Severity, ids};
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
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

fn make_tp(name: &str, op: OperationConfig, tool_id: usize, model_id: usize) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(0),
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
        feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
    }
}

fn has_id(diags: &[Diagnostic], id: &str) -> bool {
    diags.iter().any(|d| d.id.as_str() == id)
}

// ── Acceptance test #1: model-ref doesn't resolve ──────────────────

#[test]
fn pocket_op_with_unresolved_model_id_surfaces_blocking_diagnostic() {
    // Reproduces the round-02 smoke AS001 scenario shape: a toolpath
    // is added with a model_id that doesn't resolve against any
    // loaded model. F-023 requires the unified diagnostic stream to
    // surface this, not just the GUI banner.
    let mut session = ProjectSession::new_empty();
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 3.0;
    session.add_tool(tool);
    let mid = session.add_model(polygon_model(0));
    // Sanity: prove we're using a model_id the session does NOT carry.
    assert_ne!(mid, 999_usize, "test fixture must use a dangling id");

    let idx = session
        .add_toolpath(
            0,
            make_tp(
                "pocket_bad_ref",
                OperationConfig::Pocket(PocketConfig::default()),
                0,
                /* model_id = */ 999,
            ),
        )
        .unwrap();

    let diags = session.diagnose_toolpath_with_trace(idx, None).unwrap();
    assert!(
        has_id(&diags, ids::REF_MODEL_MISSING),
        "expected REF_MODEL_MISSING (id {}), got {:?}",
        ids::REF_MODEL_MISSING,
        diags.iter().map(|d| d.id.as_str()).collect::<Vec<_>>()
    );
    let diag = diags
        .iter()
        .find(|d| d.id.as_str() == ids::REF_MODEL_MISSING)
        .unwrap();
    assert_eq!(diag.severity, Severity::Blocking);
    assert!(
        diag.message.contains("999"),
        "message should cite the invalid model_id, got: {}",
        diag.message
    );
}

// ── Positive case: model-ref resolves ──────────────────────────────

#[test]
fn pocket_op_with_resolved_model_id_emits_no_ref_diagnostic() {
    let mut session = ProjectSession::new_empty();
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 3.0;
    session.add_tool(tool);
    let mid = session.add_model(polygon_model(0));

    let idx = session
        .add_toolpath(
            0,
            make_tp(
                "pocket_good_ref",
                OperationConfig::Pocket(PocketConfig::default()),
                0,
                mid,
            ),
        )
        .unwrap();

    let diags = session.diagnose_toolpath_with_trace(idx, None).unwrap();
    assert!(
        !has_id(&diags, ids::REF_MODEL_MISSING),
        "resolved model_id must not fire REF_MODEL_MISSING, got {:?}",
        diags.iter().map(|d| d.id.as_str()).collect::<Vec<_>>()
    );
}

// ── Stock-based ops are exempt ─────────────────────────────────────

#[test]
fn face_op_with_unresolved_model_id_is_silent() {
    // Face is a stock-based op — it doesn't need a loaded model, so a
    // dangling model_id should NOT trip the ref check.
    let mut session = ProjectSession::new_empty();
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 3.0;
    session.add_tool(tool);
    // No models loaded at all.

    let idx = session
        .add_toolpath(
            0,
            make_tp(
                "face_no_model",
                OperationConfig::Face(FaceConfig::default()),
                0,
                /* model_id = */ 999,
            ),
        )
        .unwrap();

    let diags = session.diagnose_toolpath_with_trace(idx, None).unwrap();
    assert!(
        !has_id(&diags, ids::REF_MODEL_MISSING),
        "stock-based op must skip the model-ref check, got {:?}",
        diags.iter().map(|d| d.id.as_str()).collect::<Vec<_>>()
    );
}
