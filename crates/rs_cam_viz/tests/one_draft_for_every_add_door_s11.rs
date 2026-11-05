//! S11 parity (operator ruling 2026-10-23): the GUI, MCP and the CLI build
//! a new op through ONE core door, `ProjectSession::draft_toolpath`, so the
//! same request gives the same numbers and the same unset state on every
//! surface.
//!
//! # The claims
//!
//! 1. The GUI add door (`AppEvent::AddToolpath`) stores the config of the
//!    core draft: the operation (every param, nulls included), the
//!    dressups, the heights and the boundary are equal. On a drawing with
//!    a backed end mill the Pocket has no floor (`unset_fields` =
//!    `["floor"]`); with a ball nose in plywood (an `Unbacked` cell) it has
//!    no feeds either: the op names them as unset, and no toast repeats
//!    that state (W2b).
//! 2. The MCP `add_toolpath` arm (`RsCamApp::core_add_toolpath`) builds its
//!    config through `draft_toolpath` and replies with `unset_fields`, and
//!    the GUI door calls the same door. `RsCamApp` needs an
//!    `eframe::CreationContext`, so a source scan pins the MCP arm (as the
//!    R2 sentry does). The CLI twin is
//!    `rs_cam_cli/src/job.rs::the_job_drafts_through_the_core_door_s11`.
//!
//! Red proof (run by hand, 2026-10-23): the GUI door back on
//! `DressupConfig::default()` (claims 1 and 2 go red).
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test one_draft_for_every_add_door_s11
//! ```

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::Path;

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::not_set::FEEDS_NOT_SET_TEXT;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::material::{Material, PlywoodGrade};
use rs_cam_core::session::{ProjectSessionBuilder, ToolpathDraft};
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::AppController;
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

/// The P2.7 drawing, one tool, and the stock `material`.
fn seed(tool: ToolConfig, material: Material) -> AppController<SilentBackend> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rs_cam_core/tests/fixtures/selection_p2_7/layers.dxf");
    let model = rs_cam_core::io::load_model_file(
        &path,
        rs_cam_core::ids::ModelId::from_raw(0),
        ModelKind::Dxf,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .expect("the drawing loads");
    let mut builder = ProjectSessionBuilder::new()
        .stock(rs_cam_core::compute::StockConfig {
            material,
            ..rs_cam_core::compute::StockConfig::default()
        })
        .tool(tool);
    let _ = builder.add_model(model);
    let mut controller = AppController::with_backend(SilentBackend);
    controller.state.replace_session(builder.build());
    controller
}

fn core_draft(controller: &AppController<SilentBackend>, op_type: OperationType) -> ToolpathDraft {
    let s = controller.state.session();
    s.draft_toolpath(
        rs_cam_core::session::DraftRequest::new(op_type, 0, format!("{} 1", op_type.label()))
            .tool(s.tools()[0].id)
            .model(s.models()[0].id),
    )
    .expect("the tool fits")
}

/// Claim 1: the GUI door stores the core draft.
fn assert_gui_is_the_draft(mut controller: AppController<SilentBackend>) -> Vec<&'static str> {
    let draft = core_draft(&controller, OperationType::Pocket);
    controller.handle_internal_event(AppEvent::AddToolpath(OperationType::Pocket));
    let configs = controller.state.session().toolpath_configs();
    assert_eq!(configs.len(), 1, "the GUI door creates the op");
    let gui = &configs[0];
    assert_eq!(
        gui.operation.params_value_including_nulls(),
        draft.config.operation.params_value_including_nulls(),
        "one operation"
    );
    assert_eq!(gui.operation.floor(), draft.config.operation.floor());
    assert_eq!(
        serde_json::to_value(&gui.dressups).expect("json"),
        serde_json::to_value(&draft.config.dressups).expect("json"),
        "one dressup config (`DressupConfig::for_op`)"
    );
    assert_eq!(gui.heights, draft.config.heights);
    assert_eq!(gui.boundary, draft.config.boundary);
    assert_eq!(gui.tool_id, draft.config.tool_id);
    assert_eq!(gui.model_id, draft.config.model_id);
    let unset = gui.operation.unset_fields();
    if draft.feeds_refusal.is_some() {
        // W2b: a state is not a toast. The unset feeds are a field finding
        // of the op (the Feeds rows show it); no toast repeats it.
        let toast = controller
            .notifications()
            .iter()
            .any(|n| n.message.contains(FEEDS_NOT_SET_TEXT));
        assert!(!toast, "no toast repeats the unset feeds (W2b)");
        assert!(
            unset.contains(&"feed_rate"),
            "the op names the unset feed rate: {unset:?}"
        );
    }
    unset
}

#[test]
fn the_gui_add_door_stores_the_core_draft_s11() {
    let end_mill = ToolConfig::builder(ToolId::from_raw(1), ToolType::EndMill)
        .diameter(3.0)
        .build();
    let backed = seed(end_mill, Material::default());
    assert_eq!(assert_gui_is_the_draft(backed), vec!["floor"]);

    let ball = ToolConfig::builder(ToolId::from_raw(1), ToolType::BallNose)
        .diameter(6.35)
        .build();
    let unbacked = seed(
        ball,
        Material::Plywood {
            grade: PlywoodGrade::BalticBirch,
        },
    );
    // P13c (rulings O1, O3, lead ruling on item 1): the Pocket stepover and
    // depth are the envelope proposal; plywood has no force line and the
    // cell has no backed row, so the add door writes the geometry width and
    // no depth (no underived default).
    assert_eq!(
        assert_gui_is_the_draft(unbacked),
        vec![
            "floor",
            "depth_per_pass",
            "feed_rate",
            "plunge_rate",
            "spindle_rpm"
        ]
    );
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

/// Claim 2: the MCP arm and the GUI door call THE draft door, and neither
/// builds the op by itself.
#[test]
fn the_mcp_and_gui_add_doors_call_the_draft_door_s11() {
    for (file, signature) in [
        ("src/app/mcp/commands.rs", "fn core_add_toolpath("),
        (
            "src/controller/events/toolpath.rs",
            "fn add_toolpath_with_model(",
        ),
    ] {
        let code = code_of(file);
        let body = method_body(&code, signature);
        assert!(
            body.contains(".draft_toolpath("),
            "{file}: `{signature}` must build the op through `draft_toolpath`"
        );
        for raw in [
            "suggest_params(",
            "default_operation(",
            "DressupConfig::default()",
        ] {
            assert!(
                !body.contains(raw),
                "{file}: `{signature}` builds the op itself (`{raw}`)"
            );
        }
    }
    let mcp = code_of("src/app/mcp/commands.rs");
    assert!(
        method_body(&mcp, "fn core_add_toolpath(").contains("unset_fields()"),
        "the MCP reply names the fields that are not set"
    );
}
