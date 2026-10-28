//! Step 7.8 (`planning/adaptive_research_2026-10-23/integration/
//! step78_design.md`): the template "Geodesic body + edges" has ONE door.
//!
//! 1. The GUI action (`AppEvent::AddGeodesicPair`, the add menu item)
//!    stores the core draft of the pair (`draft_geodesic_pair`): the same
//!    two ops, in order, with the edge op's rest-regions source set to op
//!    1's new id. Op 1 is selected, and both ops have a runtime entry.
//! 2. The MCP arm drafts with the same core door and sends the same one
//!    command; an absent tool and model take the core rules (Q7). The add
//!    menu draws the item. `RsCamApp` needs an `eframe::CreationContext`,
//!    so a source scan pins these arms (as the Q7 sentry does).
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test the_template_has_one_door_s7_8
//! ```

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rs_cam_core::compute::config::BoundarySource;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::ids::ModelId;
use rs_cam_core::session::{
    GeodesicPairRequest, LoadedModel, ProjectSessionBuilder, ToolpathConfig,
};
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
    let session = ProjectSessionBuilder::new()
        .tool(ToolConfig::new_default(
            ToolId::from_raw(0),
            ToolType::EndMill,
        ))
        .model(mesh)
        .build();
    let mut controller = AppController::with_backend(SilentBackend);
    controller.state.replace_session(session);
    controller
}

fn assert_same(gui: &ToolpathConfig, core: &ToolpathConfig) {
    assert_eq!(gui.name, core.name, "one name");
    assert_eq!(gui.model_id, core.model_id, "one model");
    assert_eq!(gui.tool_id, core.tool_id, "one tool");
    assert_eq!(
        gui.operation.params_value_including_nulls(),
        core.operation.params_value_including_nulls(),
        "one operation"
    );
    assert_eq!(gui.stock_source, core.stock_source, "one stock source");
    assert_eq!(gui.rest_analysis, core.rest_analysis, "one rest analysis");
}

#[test]
fn the_template_has_one_door_s7_8() {
    let mut c = controller();
    let core = c
        .state
        .session()
        .draft_geodesic_pair(GeodesicPairRequest::new(0, None))
        .expect("the pair drafts");
    let core_edge = core.edges.expect("the edge op").config;
    c.handle_internal_event(AppEvent::AddGeodesicPair);
    let configs = c.state.session().toolpath_configs().to_vec();
    assert_eq!(configs.len(), 2, "one action, two ops");
    assert_same(&configs[0], &core.body.config);
    assert_same(&configs[1], &core_edge);
    assert_eq!(
        configs[1].boundary.source,
        BoundarySource::DerivedRestRegions {
            source_toolpath_id: configs[0].id
        },
        "the command wrote op 1's id"
    );
    assert_eq!(*c.state.focus(), Selection::Toolpath(configs[0].id));
    for tc in &configs {
        assert!(
            c.state.gui.toolpath_rt.contains_key(&tc.id),
            "{}: no runtime entry",
            tc.name
        );
    }
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

/// The body of `fn name` in `code`, to the first line that closes it:
/// `close` is `"\n    }\n"` for a method and `"\n}\n"` for a free function.
fn fn_body<'a>(code: &'a str, signature: &str, close: &str) -> &'a str {
    let start = code
        .find(signature)
        .unwrap_or_else(|| panic!("`{signature}` is missing"));
    let rest = &code[start..];
    let end = rest
        .find(close)
        .unwrap_or_else(|| panic!("`{signature}` has no closing brace"));
    &rest[..end]
}

#[test]
fn the_mcp_door_and_the_menu_take_the_core_draft_s7_8() {
    let mcp = code_of("src/app/mcp/commands.rs");
    let add = fn_body(&mcp, "fn core_add_geodesic_pair(", "\n    }\n");
    for needle in [
        "None => rs_cam_core::session::ToolChoice::FirstFitting",
        "None => rs_cam_core::session::ModelChoice::Rule { selected: None }",
        ".draft_geodesic_pair(",
        "Command::AddGeodesicBodyAndEdges(",
    ] {
        assert!(
            add.contains(needle),
            "`core_add_geodesic_pair` lacks `{needle}`"
        );
    }
    let menu = code_of("src/ui/toolpath_panel.rs");
    let items = fn_body(&menu, "fn add_menu_items(", "\n}\n");
    assert!(
        items.contains("add_geodesic_pair_menu_item("),
        "the add menu does not draw the template item"
    );
    let item = fn_body(&menu, "fn add_geodesic_pair_menu_item(", "\n}\n");
    assert!(item.contains("AppEvent::AddGeodesicPair"));
    assert!(item.contains("add_menu_offers(session, OperationType::Adaptive3d)"));
}
