//! WP7a — `ProjectSessionBuilder` writes ids, order, stock and results
//! verbatim.
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
//! §20 ruling 1.
//!
//! `add_tool` overwrites `ToolConfig::id` and `add_model` overwrites
//! `LoadedModel::id`; `add_model` also fits the stock to the model bounding
//! box. A fixture that names a tool by id therefore reached for a `*_mut()`
//! hatch. WP7 closes those hatches, so the builder is the door that replaces
//! them. A builder that delegated to the CRUD doors would renumber every
//! fixture id, which is why id preservation is a contract and this file is
//! its sentry.
//!
//! The tools go in with the ids 7 and 3, in that order. Both a renumbering
//! builder and a sorting builder fail here.
//!
//! NOT MEASURED: the machine profile and the post configuration. The builder
//! writes both, and no test consumes either today.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::sync::Arc;

use rs_cam_core::ToolpathId;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, HeightsConfig, RestAnalysisConfig, StockSource, ToolpathStats,
};
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::transform::{FaceUp, ZRotation};
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::ops::drill_op::OpData;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    AddModelArgs, AddSetupArgs, AddToolArgs, AddToolpathArgs, Command, DatumConfig, LoadedModel,
    ProjectSessionBuilder, SetupData, ToolpathComputeResult, ToolpathConfig,
};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::debug_trace::ToolpathDebugOptions;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

/// The first tool id. It is above the second, so a sort shows up.
const TOOL_A: usize = 7;
/// The second tool id.
const TOOL_B: usize = 3;
/// The first model id.
const MODEL_A: usize = 5;
/// The second model id.
const MODEL_B: usize = 2;
/// The stock side the builder must keep, in mm.
const STOCK_SIDE_MM: f64 = 111.0;
/// The model side the stock must NOT take, in mm.
const MODEL_SIDE_MM: f64 = 40.0;

fn polygon_model(id: usize, name: &str) -> LoadedModel {
    let square = Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(MODEL_SIDE_MM, 0.0),
        P2::new(MODEL_SIDE_MM, MODEL_SIDE_MM),
        P2::new(0.0, MODEL_SIDE_MM),
    ]);
    LoadedModel {
        id,
        name: name.to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![square])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: std::path::PathBuf::from(name),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

fn toolpath(id: usize, name: &str, tool_id: usize, model_id: usize) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(id),
        name: name.to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(PocketConfig::default()),
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
        rest_analysis: RestAnalysisConfig::default(),
        planner_origin: None,
    }
}

fn setup(id: usize, name: &str, face_up: FaceUp) -> SetupData {
    SetupData {
        id,
        name: name.to_owned(),
        face_up,
        z_rotation: ZRotation::default(),
        datum: DatumConfig::default(),
        model_ids: Vec::new(),
        fixtures: Vec::new(),
        keep_out_zones: Vec::new(),
        toolpath_indices: Vec::new(),
        pause_message: None,
    }
}

fn fake_result() -> ToolpathComputeResult {
    ToolpathComputeResult {
        op_data: OpData::Toolpath(Arc::new(AnnotatedToolpath::new(Toolpath::new()))),
        stats: ToolpathStats::default(),
        debug_trace: None,
        semantic_trace: None,
    }
}

/// The stock the builder must keep. `auto_from_model` is ON, so a builder
/// that fits the stock to a model moves it.
fn stock() -> StockConfig {
    StockConfig {
        x: STOCK_SIDE_MM,
        y: STOCK_SIDE_MM,
        z: STOCK_SIDE_MM,
        auto_from_model: true,
        ..StockConfig::default()
    }
}

/// Two tools, two models, two setups, three toolpaths and one result.
///
/// Setup A takes the first two toolpaths. Setup B takes the third, because
/// `toolpath` appends to the last setup added.
fn built() -> rs_cam_core::session::ProjectSession {
    ProjectSessionBuilder::new()
        .stock(stock())
        .tool(ToolConfig::new_default(ToolId(TOOL_A), ToolType::EndMill))
        .tool(ToolConfig::new_default(ToolId(TOOL_B), ToolType::BallNose))
        .model(polygon_model(MODEL_A, "a.svg"))
        .model(polygon_model(MODEL_B, "b.svg"))
        .setup(setup(4, "Setup A", FaceUp::Top))
        .toolpath(toolpath(10, "first", TOOL_A, MODEL_A))
        .toolpath(toolpath(11, "second", TOOL_B, MODEL_B))
        .setup(setup(9, "Setup B", FaceUp::Bottom))
        .toolpath(toolpath(12, "third", TOOL_A, MODEL_B))
        .result(1, fake_result())
        .build()
}

#[test]
fn builder_keeps_tool_ids_and_order() {
    let session = built();
    let ids: Vec<usize> = session.tools().iter().map(|tool| tool.id.0).collect();
    assert_eq!(
        ids,
        vec![TOOL_A, TOOL_B],
        "the builder renumbered or reordered the tools"
    );
    assert_eq!(session.tools()[0].tool_type, ToolType::EndMill);
    assert_eq!(session.tools()[1].tool_type, ToolType::BallNose);
}

#[test]
fn builder_keeps_model_ids_and_order() {
    let session = built();
    let ids: Vec<usize> = session.models().iter().map(|model| model.id).collect();
    assert_eq!(
        ids,
        vec![MODEL_A, MODEL_B],
        "the builder renumbered or reordered the models"
    );
    assert_eq!(session.models()[0].name, "a.svg");
}

#[test]
fn builder_keeps_toolpath_order_and_bindings() {
    let session = built();
    let names: Vec<&str> = session
        .toolpath_configs()
        .iter()
        .map(|config| config.name.as_str())
        .collect();
    assert_eq!(names, vec!["first", "second", "third"]);
    let ids: Vec<usize> = session
        .toolpath_configs()
        .iter()
        .map(|config| config.id.0)
        .collect();
    assert_eq!(ids, vec![10, 11, 12], "the toolpath ids moved");
    assert_eq!(session.toolpath_configs()[0].tool_id, TOOL_A);
    assert_eq!(session.toolpath_configs()[1].tool_id, TOOL_B);
    assert_eq!(session.toolpath_configs()[2].model_id, MODEL_B);
}

#[test]
fn builder_replaces_the_seeded_setup_then_appends() {
    let session = built();
    assert_eq!(session.setup_count(), 2, "the seeded setup survived");
    let setups = session.list_setups();
    assert_eq!(setups[0].name, "Setup A");
    assert_eq!(setups[0].id, 4);
    assert_eq!(setups[1].name, "Setup B");
    assert_eq!(setups[1].face_up, FaceUp::Bottom);
    assert_eq!(setups[0].toolpath_indices, vec![0, 1]);
    assert_eq!(setups[1].toolpath_indices, vec![2]);
}

#[test]
fn builder_leaves_the_stock_alone() {
    let session = built();
    assert_eq!(
        session.stock_config(),
        &stock(),
        "the builder fitted the stock to a model"
    );
}

#[test]
fn builder_stores_the_result_and_no_revision() {
    let session = built();
    assert!(session.get_result(0).is_none(), "index 0 took a result");
    assert!(session.get_result(1).is_some(), "index 1 lost its result");
    assert!(session.get_result(2).is_none(), "index 2 took a result");
    for index in 0..session.toolpath_count() {
        assert_eq!(
            session.toolpath_revision(index),
            0,
            "the builder bumped the revision of index {index}"
        );
    }
}

#[test]
fn builder_raises_the_id_counters_above_every_supplied_id() {
    let mut session = built();
    // Each add reports its new row in `Effects.created`. The quantity
    // is per row: `add_tool`, `add_toolpath` and `add_setup` report an
    // INDEX, `add_model` reports the new model ID.
    let tool_index = session
        .apply(Command::AddTool(AddToolArgs {
            tool: Box::new(ToolConfig::new_default(ToolId(0), ToolType::EndMill)),
        }))
        .expect("the tool row refuses nothing")
        .created
        .expect("add_tool reports the new tool index");
    let fresh_tool = session.tools()[tool_index].id.0;
    assert!(
        fresh_tool > TOOL_A,
        "a later add_tool collided: {fresh_tool}"
    );
    let fresh_model = session
        .apply(Command::AddModel(AddModelArgs {
            model: Box::new(polygon_model(0, "c.svg")),
        }))
        .expect("the model row refuses nothing")
        .created
        .expect("add_model reports the new model id");
    assert!(
        fresh_model > MODEL_A,
        "a later add_model collided: {fresh_model}"
    );
    let index = session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: 0,
            config: Box::new(toolpath(0, "fourth", TOOL_A, MODEL_A)),
        }))
        .expect("add a toolpath to setup A")
        .created
        .expect("add_toolpath reports the new toolpath index");
    assert!(
        session.toolpath_configs()[index].id.0 > 12,
        "a later add_toolpath collided"
    );
    let setup_index = session
        .apply(Command::AddSetup(AddSetupArgs {
            name: Some("Setup C".to_owned()),
            face_up: FaceUp::Top,
        }))
        .expect("the setup row refuses nothing")
        .created
        .expect("add_setup reports the new setup index");
    assert!(
        session.list_setups()[setup_index].id > 9,
        "a later add_setup collided"
    );
}
