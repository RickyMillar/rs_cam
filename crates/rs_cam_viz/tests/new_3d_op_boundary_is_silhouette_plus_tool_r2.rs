//! R2 — a new 3D operation on a mesh gets the boundary "model silhouette
//! plus one tool diameter".
//!
//! `planning/corne_case_analysis_2026-09-18/ANALYSIS.md` §5, R2.
//!
//! Roadmap B.7 made both creation doors (the GUI controller's
//! `handle_add_toolpath` and the MCP `add_toolpath` door) enable the model
//! silhouette for a 3D op when the project holds a mesh, with offset 0 and
//! `Center` containment. Under `Center` the cutter centre stays inside the
//! silhouette, so the cutter stops one radius short of every outer face:
//! the outer walls are never formed and the outer band is never faced.
//!
//! The ruling: the creation-time boundary is the silhouette expanded by the
//! bound tool's diameter. The rule is ONE core constructor,
//! `BoundaryConfig::for_3d_op(tool_diameter_mm)`, and both doors call it.
//! The offset is a stored number; a later tool change does not follow.
//!
//! What this file drives: the GUI door, through the public
//! `AppController::handle_internal_event(AppEvent::AddToolpath(..))`, on a
//! session with one mesh model and one end mill. `RsCamApp` needs an
//! `eframe::CreationContext`, so the MCP door cannot be driven here; a
//! source scan pins that it calls the same constructor.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::Path;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::config::{BoundaryConfig, BoundaryContainment, BoundarySource};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::mesh::make_test_flat;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder};
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, OptimizeRequest, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::{AppController, Severity};
use rs_cam_viz::ui::AppEvent;

/// Accepts every submission, returns nothing. Nothing here waits on a lane.
struct SilentBackend;

impl ComputeBackend for SilentBackend {
    fn submit_toolpath(&mut self, _request: ComputeRequest) -> ToolpathSubmitOutcome {
        ToolpathSubmitOutcome::Queued
    }
    fn submit_simulation(&mut self, _request: SimulationRequest) {}
    fn submit_collision(&mut self, _request: CollisionRequest) {}
    fn submit_optimize(&mut self, _request: OptimizeRequest) {}
    fn cancel_lane(&mut self, _lane: ComputeLane) {}
    fn drain_results(&mut self) -> Vec<ComputeMessage> {
        Vec::new()
    }
    fn lane_snapshot(&self, lane: ComputeLane) -> LaneSnapshot {
        LaneSnapshot::idle(lane)
    }
    fn generation_control(&self) -> GenerationControl {
        GenerationControl::detached()
    }
}

/// One end mill and one flat mesh model. The mesh is what makes the B.7
/// arm fire; the end mill is a tool every 3D roughing op accepts.
fn mesh_seed() -> AppController<SilentBackend> {
    let mut controller = AppController::with_backend(SilentBackend);
    let mut builder =
        ProjectSessionBuilder::new().tool(ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    let mesh = Arc::new(make_test_flat(40.0));
    let _ = builder.add_model(LoadedModel {
        id: 0,
        path: std::path::PathBuf::from("flat.stl"),
        name: "Flat".to_owned(),
        kind: Some(ModelKind::Stl),
        mesh: Some(mesh),
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    });
    controller.state.session = builder.build();
    controller
}

fn warnings(controller: &AppController<SilentBackend>) -> Vec<String> {
    controller
        .notifications()
        .iter()
        .filter(|n| n.severity != Severity::Info)
        .map(|n| n.message.clone())
        .collect()
}

/// The claim. A 3D rough added through the GUI door carries the silhouette
/// boundary with `Center` containment and an offset equal to the bound
/// tool's diameter.
#[test]
fn a_new_3d_op_on_a_mesh_gets_silhouette_plus_one_tool_diameter() {
    let mut controller = mesh_seed();
    let tool_diameter = controller.state.session.tools()[0].diameter;
    assert!(tool_diameter > 0.0, "the seed tool has a diameter");

    controller.handle_internal_event(AppEvent::AddToolpath(OperationType::Adaptive3d));

    // Non-vacuity: the add must have CREATED the toolpath. A Suggest
    // refusal creates nothing and would pass an assertion on an empty list.
    let configs = controller.state.session.toolpath_configs();
    assert_eq!(
        configs.len(),
        1,
        "the add door must create exactly one toolpath; warnings: {:?}",
        warnings(&controller)
    );
    let boundary = &configs[0].boundary;
    assert!(boundary.enabled, "B.7: the boundary is enabled on a mesh");
    assert_eq!(
        boundary.source,
        BoundarySource::ModelSilhouette,
        "B.7: the source is the model silhouette"
    );
    assert_eq!(
        boundary.containment,
        BoundaryContainment::Center,
        "R2: containment stays Center"
    );
    assert!(
        (boundary.offset - tool_diameter).abs() < 1e-9,
        "R2: the offset is one tool diameter ({tool_diameter}); got {}",
        boundary.offset
    );
    assert_eq!(
        *boundary,
        BoundaryConfig::for_3d_op(tool_diameter),
        "the door writes exactly what the shared constructor builds"
    );
}

/// The constructor is the rule. Pinned on its own so a door that drifts
/// from it and a constructor that drifts from R2 fail on different lines.
#[test]
fn for_3d_op_is_silhouette_center_and_the_diameter() {
    let b = BoundaryConfig::for_3d_op(6.35);
    assert!(b.enabled);
    assert_eq!(b.source, BoundarySource::ModelSilhouette);
    assert_eq!(b.containment, BoundaryContainment::Center);
    assert!((b.offset - 6.35).abs() < 1e-12);
    assert_ne!(
        b,
        BoundaryConfig::default(),
        "the default stays disabled / stock / 0"
    );
}

/// Control arm. A stock-based 2D op takes the plain default: disabled,
/// stock, offset 0. The R2 rule fires for 3D ops only.
#[test]
fn a_new_2d_op_keeps_the_disabled_default() {
    let mut controller = mesh_seed();

    controller.handle_internal_event(AppEvent::AddToolpath(OperationType::Face));

    let configs = controller.state.session.toolpath_configs();
    assert_eq!(
        configs.len(),
        1,
        "the add door must create the Face op; warnings: {:?}",
        warnings(&controller)
    );
    assert_eq!(configs[0].boundary, BoundaryConfig::default());
}

/// The MCP door cannot be driven here (`RsCamApp` needs an
/// `eframe::CreationContext`), so this pins that BOTH door files call the
/// shared constructor on a code line, and that neither still spells the
/// old `BoundarySource::ModelSilhouette, ..default()` literal.
#[test]
fn both_creation_doors_call_the_shared_constructor() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for rel in [
        "src/controller/events/toolpath.rs",
        "src/app/mcp/commands.rs",
    ] {
        let path = root.join(rel);
        let text = std::fs::read_to_string(&path).expect("read door source");
        let code_lines: Vec<&str> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.starts_with("//"))
            .collect();
        // Non-vacuity anchor: the B.7 arm is still in this file.
        assert!(
            code_lines.iter().any(|l| l.contains("m.mesh.is_some()")),
            "{rel}: the B.7 has-mesh arm is gone; the scan below is vacuous"
        );
        assert!(
            code_lines
                .iter()
                .any(|l| l.contains("BoundaryConfig::for_3d_op(")),
            "{rel}: the creation door must call BoundaryConfig::for_3d_op (R2)"
        );
        assert!(
            !code_lines
                .iter()
                .any(|l| l.contains("source: BoundarySource::ModelSilhouette,")
                    || l.contains(
                        "source: crate::state::toolpath::BoundarySource::ModelSilhouette,"
                    )),
            "{rel}: the creation door still builds the pre-R2 literal beside the constructor"
        );
    }
}
