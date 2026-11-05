//! M10 (architecture review 2026-11-05): the add menu offers an op only
//! when ONE model serves it.
//!
//! # The claim
//!
//! `ui::toolpath_panel::add_menu_offers` calls `LoadedModel::serves` (the
//! Q7 rule of the core) for each model. Before M10 it tested "any model has
//! items" and "any model has a mesh" apart, so a drawing and a separate
//! mesh offered Project Curve, an op that needs items and a mesh on one
//! model. No model served it.
//!
//! 1. A drawing-only model and a mesh-only model: the menu does not offer
//!    Project Curve. It offers Pocket (the drawing serves it) and Drop
//!    Cutter (the mesh serves it).
//! 2. No model: the menu offers Face and Pin Drill (`is_stock_based`), and
//!    not Pocket.
//!
//! Red proof: restore the old body (two separate `any` tests). Claim 1
//! fails on Project Curve.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test the_add_menu_offers_what_one_model_serves_m10
//! ```

#![expect(
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ModelId;
use rs_cam_core::selection::{Item, ItemSet, Shape, SourceRef};
use rs_cam_core::session::{LoadedModel, ProjectSession, ProjectSessionBuilder};
use rs_cam_viz::ui::toolpath_panel::add_menu_offers;

/// A model with no mesh and no item.
fn model(id: ModelId, name: &str) -> LoadedModel {
    LoadedModel {
        id,
        path: PathBuf::from(format!("{name}.svg")),
        name: name.to_owned(),
        kind: None,
        mesh: None,
        items: Arc::default(),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    }
}

/// A drawing with one square loop, and no mesh.
fn drawing_only() -> LoadedModel {
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
    LoadedModel {
        items: Arc::new(ItemSet::new(vec![square])),
        kind: Some(ModelKind::Svg),
        ..model(ModelId::from_raw(0), "drawing")
    }
}

/// A mesh with no item.
fn mesh_only() -> LoadedModel {
    LoadedModel {
        mesh: Some(Arc::new(rs_cam_core::mesh::make_test_flat(40.0))),
        kind: Some(ModelKind::Stl),
        ..model(ModelId::from_raw(1), "part")
    }
}

fn session_of(models: Vec<LoadedModel>) -> ProjectSession {
    models
        .into_iter()
        .fold(ProjectSessionBuilder::new(), ProjectSessionBuilder::model)
        .build()
}

#[test]
fn a_drawing_and_a_separate_mesh_do_not_offer_project_curve_m10() {
    let drawing = drawing_only();
    let mesh = mesh_only();
    assert!(
        !drawing.serves(OperationType::ProjectCurve) && !mesh.serves(OperationType::ProjectCurve),
        "the case: no single model serves Project Curve"
    );
    let session = session_of(vec![drawing, mesh]);
    assert!(
        !add_menu_offers(&session, OperationType::ProjectCurve),
        "no model serves Project Curve, so the menu must not offer it"
    );
    assert!(
        add_menu_offers(&session, OperationType::Pocket),
        "the drawing serves Pocket"
    );
    assert!(
        add_menu_offers(&session, OperationType::DropCutter),
        "the mesh serves Drop Cutter"
    );
}

#[test]
fn the_stock_ops_are_offered_with_no_model_m10() {
    let session = session_of(Vec::new());
    assert!(add_menu_offers(&session, OperationType::Face));
    assert!(add_menu_offers(&session, OperationType::AlignmentPinDrill));
    assert!(
        !add_menu_offers(&session, OperationType::Pocket),
        "with no model nothing serves Pocket"
    );
}
