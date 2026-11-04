//! R-A parity (operator ruling 2026-10-23): the default Profile on a mesh
//! model has one config on every surface. The GUI, MCP and the CLI add an
//! op through THE draft door (`ProjectSession::draft_toolpath`) and the
//! `AddToolpath` row, and that row runs THE part outline rule
//! (`ProjectSession::part_outline_floor_due`).
//!
//! # The claims
//!
//! 1. The GUI add door (`AppEvent::AddToolpath(Profile)`) on
//!    `stepped_block.step` stores the config that the core add door gives.
//!    W3h: a new op holds nothing, so the floor rule fires on the first
//!    selection edit: with the rule "every item it can cut" (the part
//!    outline) both doors give the operation (every param, nulls
//!    included), the floor "Model bottom" depth 0, side Outside, no tabs.
//!    The GUI notice names the note `Effects::MODEL_BOTTOM_FLOOR_NOTE`.
//! 2. MCP: `one_draft_for_every_add_door_s11` pins that the MCP arm calls
//!    the same draft door and sends the same row; the CLI twin is
//!    `rs_cam_cli/src/job.rs::a_job_profile_on_a_step_model_cuts_the_outline_ra`.
//!
//! Red proof (run by hand, 2026-10-23): with the outline rule off in
//! `add_toolpath`, no floor is set (the add keeps the stock default) and
//! claim 1 fails.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test the_default_mesh_profile_is_one_config_ra
//! ```

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::Path;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::session::{
    AddToolpathArgs, Command, Effects, ProjectSession, ProjectSessionBuilder,
};
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

/// The stepped block (X 0..60, Y 0..40, Z 0..30) in a stock 5 mm larger
/// in X and Y, with a 6 mm end mill.
fn session() -> ProjectSession {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rs_cam_core/tests/fixtures/step/stepped_block.step");
    let model = rs_cam_core::io::load_model_file(
        &path,
        rs_cam_core::ids::ModelId::from_raw(0),
        ModelKind::Step,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .expect("the model loads");
    let mut builder = ProjectSessionBuilder::new()
        .stock(rs_cam_core::compute::StockConfig {
            x: 70.0,
            y: 50.0,
            z: Some(30.0),
            origin_x: -5.0,
            origin_y: -5.0,
            origin_z: 0.0,
            auto_from_model: false,
            ..rs_cam_core::compute::StockConfig::default()
        })
        .tool(
            ToolConfig::builder(ToolId::from_raw(1), ToolType::EndMill)
                .diameter(6.0)
                .build(),
        );
    let _ = builder.add_model(model);
    builder.build()
}

/// Claim 1. W3h: the add holds nothing, so no floor rule fires at the add;
/// the rule "every item it can cut" (the part outline) then sets "Model
/// bottom" on both doors, with the one note.
#[test]
fn the_gui_door_gives_the_core_default_mesh_profile_ra() {
    use rs_cam_core::selection::{Selection, SelectionEdit};
    use rs_cam_core::session::{EditSelectionArgs, SelectionTarget};
    let mut core = session();
    let draft = core
        .draft_toolpath(
            rs_cam_core::session::DraftRequest::new(
                OperationType::Profile,
                0,
                format!("{} 1", OperationType::Profile.label()),
            )
            .tool(core.tools()[0].id)
            .model(core.models()[0].id),
        )
        .expect("the tool fits");
    let effects = core
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(draft.config),
        }))
        .expect("the add");
    assert_eq!(effects.floor_set_note(), None, "W3h: the add holds nothing");
    let effects = core
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit: SelectionEdit::Replace(Box::new(Selection::drawing())),
        }))
        .expect("the selection edit");
    assert_eq!(
        effects.floor_set_note(),
        Some(Effects::MODEL_BOTTOM_FLOOR_NOTE)
    );
    let want = &core.toolpath_configs()[0];
    let OperationConfig::Profile(cfg) = &want.operation else {
        panic!("a Profile");
    };
    assert_eq!(cfg.floor, Floor::ModelBottom { depth: 0.0 });
    assert_eq!(cfg.side, rs_cam_core::ops::profile::ProfileSide::Outside);
    assert_eq!((cfg.tab_count, cfg.tab_points.len()), (0, 0));

    let mut controller = AppController::with_backend(SilentBackend);
    controller.state.replace_session(session());
    controller.handle_internal_event(AppEvent::AddToolpath(OperationType::Profile));
    let id = {
        let configs = controller.state.session().toolpath_configs();
        assert_eq!(configs.len(), 1, "the GUI door creates the op");
        configs[0].id
    };
    controller.handle_internal_event(AppEvent::EditToolpathSelection {
        toolpath_id: id,
        target: SelectionTarget::Op,
        edit: SelectionEdit::Replace(Box::new(Selection::drawing())),
    });
    let gui = &controller.state.session().toolpath_configs()[0];
    assert_eq!(
        gui.operation.params_value_including_nulls(),
        want.operation.params_value_including_nulls(),
        "one operation"
    );
    assert_eq!(gui.operation.floor(), want.operation.floor());
    assert!(
        controller
            .notifications()
            .iter()
            .any(|n| n.message.contains(Effects::MODEL_BOTTOM_FLOOR_NOTE)),
        "the GUI notice names the note"
    );
}
