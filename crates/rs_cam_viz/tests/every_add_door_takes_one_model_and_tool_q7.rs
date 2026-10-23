//! Q7 + R2 parity (operator ruling 2026-10-23): the GUI add door and the
//! MCP `add_toolpath` door with no tool and no model give the SAME op as
//! the core draft (`DraftRequest::new`: the first fitting tool, the model
//! rule).
//!
//! # The fixture
//!
//! An STL mesh FIRST and a drawing second; tools `[V-bit, end mill]`.
//!
//! # The claims
//!
//! 1. The GUI add of a Pocket with nothing selected stores the core draft:
//!    the drawing (not the first model), the same tool and the same
//!    operation.
//! 2. The GUI add of a Drop Cutter with the drawing selected in the Setup
//!    rail stores the core draft with that selected model: the mesh,
//!    because a drawing does not serve a 3D op.
//! 3. MCP: `core_add_toolpath` maps no `tool_index` to
//!    `ToolChoice::FirstFitting` and no `model_id` to `ModelChoice::Rule`,
//!    and `add_toolpath_via_gui` passes its `model_id` to the GUI handler.
//!    `RsCamApp` needs an `eframe::CreationContext`, so a source scan pins
//!    these arms (as the S11 sentry does). The CLI names its tool and its
//!    model, so it drafts with both exact (the S11 CLI twin).
//!
//! Red proof: make `model_for_new_op` return the first project model.
//! Claim 1 fails on the model.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test every_add_door_takes_one_model_and_tool_q7
//! ```

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ModelId;
use rs_cam_core::selection::{Item, ItemSet, Shape, SourceRef};
use rs_cam_core::session::{DraftRequest, LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::AppController;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::AppEvent;

/// Accepts every submission, returns nothing.
struct SilentBackend;

impl ComputeBackend for SilentBackend {
    fn submit_toolpath(&mut self, _request: ComputeRequest) -> ToolpathSubmitOutcome {
        ToolpathSubmitOutcome::Queued
    }
    fn submit_simulation(&mut self, _request: SimulationRequest) {}
    fn submit_collision(&mut self, _request: CollisionRequest) {}
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

const MESH: ModelId = ModelId::from_raw(0);
const DRAWING: ModelId = ModelId::from_raw(1);

fn model(id: ModelId, name: &str) -> LoadedModel {
    LoadedModel {
        id,
        path: PathBuf::from(format!("{name}.svg")),
        name: name.to_owned(),
        kind: None,
        mesh: None,
        items: Arc::default(),
        enriched_mesh: None,
        units: Some(rs_cam_core::compute::stock_config::ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    }
}

fn controller() -> AppController<SilentBackend> {
    let mesh = LoadedModel {
        mesh: Some(Arc::new(rs_cam_core::mesh::make_test_flat(40.0))),
        kind: Some(rs_cam_core::compute::stock_config::ModelKind::Stl),
        ..model(MESH, "part")
    };
    let square = Item::new(
        SourceRef::ToolpathModel,
        None,
        Shape::Loop(rs_cam_core::polygon::Polygon2::new(vec![
            P2::new(-10.0, -10.0),
            P2::new(10.0, -10.0),
            P2::new(10.0, 10.0),
            P2::new(-10.0, 10.0),
        ])),
    )
    .unwrap_or_else(|e| panic!("a square: {e:?}"));
    let drawing = LoadedModel {
        items: Arc::new(ItemSet::new(vec![square])),
        kind: Some(rs_cam_core::compute::stock_config::ModelKind::Svg),
        ..model(DRAWING, "drawing")
    };
    let session = ProjectSessionBuilder::new()
        .tool(ToolConfig::new_default(ToolId::from_raw(0), ToolType::VBit))
        .tool(ToolConfig::new_default(
            ToolId::from_raw(1),
            ToolType::EndMill,
        ))
        .model(mesh)
        .model(drawing)
        .build();
    let mut controller = AppController::with_backend(SilentBackend);
    controller.state.replace_session(session);
    controller
}

fn assert_same(gui: &ToolpathConfig, core: &ToolpathConfig) {
    assert_eq!(gui.model_id, core.model_id, "one model");
    assert_eq!(gui.tool_id, core.tool_id, "one tool");
    assert_eq!(
        gui.operation.params_value_including_nulls(),
        core.operation.params_value_including_nulls(),
        "one operation"
    );
    assert_eq!(gui.boundary, core.boundary, "one boundary");
}

#[test]
fn every_add_door_takes_one_model_and_tool_q7() {
    // Claim 1: nothing selected.
    let mut c = controller();
    let core = c
        .state
        .session()
        .draft_toolpath(DraftRequest::new(OperationType::Pocket, 0, "Pocket 1"))
        .expect("the draft")
        .config;
    assert_eq!(core.model_id, DRAWING, "the rule takes the drawing");
    c.handle_internal_event(AppEvent::AddToolpath(OperationType::Pocket));
    let gui = c.state.session().toolpath_configs()[0].clone();
    assert_same(&gui, &core);

    // Claim 2: the drawing selected in the Setup rail, a 3D op.
    let mut c = controller();
    c.state.selection = Selection::Model(DRAWING);
    let core = c
        .state
        .session()
        .draft_toolpath(
            DraftRequest::new(OperationType::DropCutter, 0, "Drop Cutter 1")
                .selected_model(Some(DRAWING)),
        )
        .expect("the draft")
        .config;
    assert_eq!(core.model_id, MESH, "a drawing does not serve a 3D op");
    c.handle_internal_event(AppEvent::AddToolpath(OperationType::DropCutter));
    let gui = c.state.session().toolpath_configs()[0].clone();
    assert_same(&gui, &core);
}

/// The code of a source file, each line cut at its first `//`.
fn code_of(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
        .lines()
        .map(|line| line.find("//").map_or(line, |at| &line[..at]))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The body of `fn name` in `code`, to the first line that closes it at
/// four spaces of indent (a method).
fn method_body<'a>(code: &'a str, signature: &str) -> &'a str {
    let start = code
        .find(signature)
        .unwrap_or_else(|| panic!("`{signature}` is missing"));
    let rest = &code[start..];
    let end = rest
        .find("\n    }\n")
        .unwrap_or_else(|| panic!("`{signature}` has no closing brace"));
    &rest[..end]
}

/// Claim 3: the MCP arms map an absent tool and model to the core rules.
#[test]
fn the_mcp_add_doors_take_the_core_rules_q7() {
    let mcp = code_of("src/app/mcp/commands.rs");
    let add = method_body(&mcp, "fn core_add_toolpath(");
    for needle in [
        "None => rs_cam_core::session::ToolChoice::FirstFitting",
        "None => rs_cam_core::session::ModelChoice::Rule { selected: None }",
        "\"no_tool\"",
    ] {
        assert!(add.contains(needle), "`core_add_toolpath` lacks `{needle}`");
    }
    let generation = code_of("src/app/mcp/generation.rs");
    let via_gui = method_body(&generation, "fn mcp_add_toolpath_via_gui(");
    assert!(
        via_gui.contains(".add_toolpath_with_model("),
        "`add_toolpath_via_gui` passes its `model_id` to the GUI handler"
    );
}
