//! Q7 (FreeCAD, Fusion; operator ruling 2026-10-23): a new op takes the
//! models of its setup, the selected model when it serves the op, and the
//! picks of the caller as its selection.
//!
//! # The claim
//!
//! The project holds a STEP part FIRST (id 0) and a DXF-like drawing
//! second (id 1). Before Q7 every add door took the first project model.
//!
//! 1. A 2D op with no selected model takes the DRAWING: the first setup
//!    model that is native to a 2D op (items, no mesh). A 3D op takes the
//!    STEP part (a mesh).
//! 2. A selected model that serves the op wins: a Pocket with the STEP
//!    part selected takes it (a STEP face serves a 2D op). A selected
//!    model that does not serve the op loses: a 3D op with the drawing
//!    selected takes the STEP part.
//! 3. The setup "Models" list limits the choice: with the setup scoped to
//!    the STEP part, a Pocket takes the STEP part.
//! 4. Picks at add (O-PICK): a picked STEP face becomes the op selection
//!    (it replaces the default), and the add sets the each-face floor.
//!
//! Red proof: make `model_for_new_op` return the first project model. Test
//! 1 fails on the Pocket.
//!
//! ```text
//! cargo test -p rs_cam_core -q --features step --test a_new_op_takes_its_setup_model_q7
//! ```

#![cfg(feature = "step")]
#![expect(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::path::PathBuf;

use common::session::{polygon_model, square_polygon};
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::ids::ModelId;
use rs_cam_core::selection::Shape;
use rs_cam_core::session::{
    AddToolpathArgs, Command, DraftRequest, ProjectSession, ProjectSessionBuilder,
    SetSetupModelsArgs,
};

const STEPPED: &str = "tests/fixtures/step/stepped_block.step";
const STEP_ID: ModelId = ModelId::from_raw(0);
const DRAWING_ID: ModelId = ModelId::from_raw(1);

fn session() -> ProjectSession {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(STEPPED);
    let step = rs_cam_core::io::load_model_file(
        &path,
        STEP_ID,
        ModelKind::Step,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .unwrap_or_else(|e| panic!("{STEPPED}: {e}"));
    let mut drawing = polygon_model(vec![square_polygon(10.0)], "drawing");
    drawing.id = DRAWING_ID;
    let mut builder = ProjectSessionBuilder::new().stock(StockConfig {
        x: 70.0,
        y: 50.0,
        z: Some(32.0),
        origin_x: -5.0,
        origin_y: -5.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    });
    let _ = builder.add_tool(ToolConfig {
        diameter: 6.0,
        ..ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill)
    });
    let _ = builder.add_tool(ToolConfig {
        diameter: 6.0,
        ..ToolConfig::new_default(ToolId::from_raw(1), ToolType::BallNose)
    });
    let _ = builder.add_model(step);
    let _ = builder.add_model(drawing);
    let s = builder.build();
    assert_eq!(
        s.models()[0].id,
        STEP_ID,
        "the STEP part is the FIRST model"
    );
    assert!(s.models()[0].mesh.is_some());
    assert_eq!(s.models()[1].id, DRAWING_ID);
    assert!(s.models()[1].mesh.is_none());
    s
}

/// The model of a draft of `op` with the selected model `selected`.
fn model_of(s: &ProjectSession, op: OperationType, selected: Option<ModelId>) -> ModelId {
    s.draft_toolpath(DraftRequest::new(op, 0, op.label()).selected_model(selected))
        .unwrap()
        .config
        .model_id
}

#[test]
fn a_2d_op_takes_the_drawing_of_its_setup_q7() {
    let s = session();
    assert_eq!(
        model_of(&s, OperationType::Pocket, None),
        DRAWING_ID,
        "a 2D op takes the drawing, not the first model"
    );
    assert_eq!(model_of(&s, OperationType::Profile, None), DRAWING_ID);
    assert_eq!(
        model_of(&s, OperationType::DropCutter, None),
        STEP_ID,
        "a 3D op takes the mesh"
    );
}

#[test]
fn the_selected_model_wins_when_it_serves_the_op_q7() {
    let s = session();
    assert_eq!(
        model_of(&s, OperationType::Pocket, Some(STEP_ID)),
        STEP_ID,
        "a STEP face serves a Pocket, so the selected STEP part wins"
    );
    assert_eq!(
        model_of(&s, OperationType::DropCutter, Some(DRAWING_ID)),
        STEP_ID,
        "a drawing does not serve a 3D op, so the rule takes the mesh"
    );
}

#[test]
fn the_setup_models_limit_the_choice_q7() {
    let mut s = session();
    let _ = s
        .apply(Command::SetSetupModels(SetSetupModelsArgs {
            setup_index: 0,
            model_ids: vec![STEP_ID],
        }))
        .unwrap();
    assert_eq!(
        model_of(&s, OperationType::Pocket, None),
        STEP_ID,
        "the setup holds the STEP part only"
    );
    assert_eq!(
        model_of(&s, OperationType::Pocket, Some(DRAWING_ID)),
        STEP_ID,
        "a selected model outside the setup loses"
    );
}

#[test]
fn picks_at_add_become_the_selection_q7() {
    let mut s = session();
    // The flat face of the stepped block at Z 20 (below the stock top).
    let face = s.models()[0]
        .items
        .iter()
        .find_map(|item| match item.shape() {
            Shape::Face(f) if (f.plane_z - 20.0).abs() < 1e-6 => f.brep,
            _ => None,
        })
        .expect("the stepped block has a face at Z 20");
    let draft = s
        .draft_toolpath(
            DraftRequest::new(OperationType::Pocket, 0, "Pocket").picks(vec![(STEP_ID, face)]),
        )
        .unwrap();
    assert!(draft.dropped_picks.is_empty(), "{:?}", draft.dropped_picks);
    assert_eq!(
        draft.config.model_id, STEP_ID,
        "the model of the pick is the selected model"
    );
    let selection = draft.config.operation.selection().unwrap();
    let op_default = draft.config.operation.default_selection().unwrap();
    assert!(
        !selection.is_default_of(&op_default),
        "the pick replaces the default"
    );
    assert_eq!(selection.picks().len(), 1, "one pick: {selection:?}");

    // Non-vacuity: with no pick the draft keeps the default.
    let plain = s
        .draft_toolpath(DraftRequest::new(OperationType::Pocket, 0, "Plain"))
        .unwrap();
    assert!(
        plain
            .config
            .operation
            .selection()
            .unwrap()
            .is_default_of(&op_default)
    );

    let effects = s
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(draft.config),
        }))
        .unwrap();
    assert!(
        effects.floor_set_note().is_some(),
        "a picked face below the stock top sets the each-face floor at the add"
    );
}
