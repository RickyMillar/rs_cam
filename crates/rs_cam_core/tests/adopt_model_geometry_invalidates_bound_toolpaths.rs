//! WP6b — `AdoptModelGeometry` replaces one model's geometry and drops
//! the results of the toolpaths bound to it, and only those.
//!
//! Programme: `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
//! §4 WP6b and §20 ruling 4.
//!
//! The GUI holds three model-refresh doors — rescale, reload and relink.
//! Each one re-imported the file and wrote the record through the
//! `models_mut` hatch, then ran its own invalidation sweep beside it
//! (G-RELOADTARGETS F4.4, G-RESCALESTALE F4.7). Two of the three defects
//! that history records came from the two halves drifting apart. This row
//! is the one door: it adopts the geometry AND drops the dependent
//! results, in one mutation, and reports the set it dropped.
//!
//! Four properties:
//!
//! 1. The new geometry lands, `drill_targets` and `layers` included.
//! 2. `Effects::stale` names every toolpath bound to that model.
//! 3. A toolpath bound to ANOTHER model keeps its result.
//! 4. An id that names no model is refused, and nothing moves.
//!
//! Red before the fix: `Command::AdoptModelGeometry` does not exist, so
//! this file does not compile. That is the red — a build error, not a
//! failed assertion.
//!
//! NOT MEASURED here: the `units` write is a caller-supplied override
//! that only the rescale door sends. Property 1 pins both arms of it.

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
use rs_cam_core::compute::stock_config::ModelUnits;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::transform::{FaceUp, ZRotation};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::drill_op::OpData;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::io::dxf_input::{DrillTarget, DrillTargetKind};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    AdoptModelGeometryArgs, Command, DatumConfig, LoadedModel, ProjectSession,
    ProjectSessionBuilder, SessionError, SetupData, ToolpathComputeResult, ToolpathConfig,
};
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::toolpath_spans::AnnotatedToolpath;

/// The model the refresh replaces.
const MODEL_A: usize = 5;
/// The model the refresh must not touch.
const MODEL_B: usize = 2;
/// An id no model in the fixture carries.
const MODEL_ABSENT: usize = 77;
/// The tool every toolpath binds.
const TOOL: usize = 7;
/// The side of the square the fixture imports first, in mm.
const SIDE_BEFORE_MM: f64 = 40.0;
/// The side of the square the refresh brings in, in mm.
const SIDE_AFTER_MM: f64 = 90.0;

fn square_model(id: usize, name: &str, side_mm: f64, targets: Vec<DrillTarget>) -> LoadedModel {
    let square = Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(side_mm, 0.0),
        P2::new(side_mm, side_mm),
        P2::new(0.0, side_mm),
    ]);
    LoadedModel {
        id,
        name: name.to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![square])),
        drill_targets: Arc::new(targets),
        layers: Arc::new(Vec::new()),
        path: std::path::PathBuf::from(name),
        kind: None,
        units: Some(ModelUnits::Millimeters),
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
}

fn toolpath(id: usize, name: &str, model_id: usize) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(id),
        name: name.to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(PocketConfig::default()),
        dressups: DressupConfig::default(),
        heights: HeightsConfig::default(),
        tool_id: TOOL,
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

fn setup() -> SetupData {
    SetupData {
        id: 1,
        name: "Setup 1".to_owned(),
        face_up: FaceUp::Top,
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

/// Two models and three toolpaths. Index 0 and index 2 bind `MODEL_A`;
/// index 1 binds `MODEL_B`. Every index holds a result.
fn fixture() -> ProjectSession {
    ProjectSessionBuilder::new()
        .tool(ToolConfig::new_default(ToolId(TOOL), ToolType::EndMill))
        .model(square_model(MODEL_A, "a.svg", SIDE_BEFORE_MM, Vec::new()))
        .model(square_model(MODEL_B, "b.svg", SIDE_BEFORE_MM, Vec::new()))
        .setup(setup())
        .toolpath(toolpath(10, "binds a", MODEL_A))
        .toolpath(toolpath(11, "binds b", MODEL_B))
        .toolpath(toolpath(12, "binds a too", MODEL_A))
        .result(0, fake_result())
        .result(1, fake_result())
        .result(2, fake_result())
        .build()
}

/// The polygon side the model carries now, in mm.
fn side_of(session: &ProjectSession, model_id: usize) -> f64 {
    let model = session
        .models()
        .iter()
        .find(|model| model.id == model_id)
        .expect("the fixture carries the model");
    let polygons = model.polygons.as_deref().expect("the model has polygons");
    let mut max_x: f64 = 0.0;
    for point in &polygons[0].exterior {
        max_x = max_x.max(point.x);
    }
    max_x
}

/// Property 1 — the new geometry lands, with its drill targets.
#[test]
fn the_row_adopts_the_new_geometry_and_its_drill_targets() {
    let mut session = fixture();
    assert_eq!(side_of(&session, MODEL_A), SIDE_BEFORE_MM);
    let fresh = square_model(
        MODEL_A,
        "a.svg",
        SIDE_AFTER_MM,
        vec![DrillTarget {
            x: 1.0,
            y: 2.0,
            layer: "holes".to_owned(),
            kind: DrillTargetKind::CircleCenter { diameter: 3.0 },
        }],
    );
    let _ = session
        .apply(Command::AdoptModelGeometry(AdoptModelGeometryArgs {
            model_id: MODEL_A,
            geometry: Box::new(fresh),
            units: None,
        }))
        .expect("the model exists, so the row applies");
    assert_eq!(
        side_of(&session, MODEL_A),
        SIDE_AFTER_MM,
        "the row must adopt the new polygons"
    );
    let model = session
        .models()
        .iter()
        .find(|model| model.id == MODEL_A)
        .expect("the model survives");
    assert_eq!(
        model.drill_targets.len(),
        1,
        "G-RELOADTARGETS: the row must adopt the new drill targets, or a \
         drill operation cuts the previous import's holes"
    );
    assert_eq!(
        model.units,
        Some(ModelUnits::Millimeters),
        "`units: None` means the caller overrides nothing, so the record \
         keeps the units it carries"
    );
}

/// Property 1, second arm — a supplied unit declaration is written.
#[test]
fn a_supplied_unit_declaration_is_written() {
    let mut session = fixture();
    let fresh = square_model(MODEL_A, "a.svg", SIDE_AFTER_MM, Vec::new());
    let _ = session
        .apply(Command::AdoptModelGeometry(AdoptModelGeometryArgs {
            model_id: MODEL_A,
            geometry: Box::new(fresh),
            units: Some(ModelUnits::Inches),
        }))
        .expect("the model exists, so the row applies");
    let model = session
        .models()
        .iter()
        .find(|model| model.id == MODEL_A)
        .expect("the model survives");
    assert_eq!(
        model.units,
        Some(ModelUnits::Inches),
        "the rescale door declares the new units, and the row writes them"
    );
}

/// Properties 2 and 3 — the bound results drop, and only those.
#[test]
fn the_row_drops_the_bound_results_and_only_those() {
    let mut session = fixture();
    let fresh = square_model(MODEL_A, "a.svg", SIDE_AFTER_MM, Vec::new());
    let effects = session
        .apply(Command::AdoptModelGeometry(AdoptModelGeometryArgs {
            model_id: MODEL_A,
            geometry: Box::new(fresh),
            units: None,
        }))
        .expect("the model exists, so the row applies");
    let stale: Vec<usize> = effects.stale.iter().copied().collect();
    assert_eq!(
        stale,
        vec![0, 2],
        "the row must report every toolpath bound to the model, and no other"
    );
    assert!(
        session.get_result(0).is_none(),
        "index 0 binds the refreshed model, so its cached geometry answers \
         a file that is gone"
    );
    assert!(
        session.get_result(2).is_none(),
        "index 2 binds the refreshed model too"
    );
    assert!(
        session.get_result(1).is_some(),
        "index 1 binds another model; a refresh of one model must not drop \
         the whole project"
    );
}

/// Property 4 — an id that names no model is refused.
#[test]
fn an_absent_model_id_is_refused_and_nothing_moves() {
    let mut session = fixture();
    let fresh = square_model(MODEL_ABSENT, "gone.svg", SIDE_AFTER_MM, Vec::new());
    let outcome = session.apply(Command::AdoptModelGeometry(AdoptModelGeometryArgs {
        model_id: MODEL_ABSENT,
        geometry: Box::new(fresh),
        units: None,
    }));
    assert!(
        matches!(outcome, Err(SessionError::MissingGeometry(_))),
        "an id that names no model is a refusal, not a silent no-op"
    );
    assert_eq!(session.models().len(), 2, "the refusal added no model");
    for index in 0..3 {
        assert!(
            session.get_result(index).is_some(),
            "the refusal dropped result {index}"
        );
    }
}
