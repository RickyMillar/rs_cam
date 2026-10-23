//! F31: the auto stock fits every model of the project.
//!
//! Operator ruling 2026-10-23: "stock should fit all models". Before the
//! fix, `add_model` fitted an `auto_from_model` stock to the LAST model
//! only. The live case imported a 60 x 40 x 30 block and then a
//! 50 x 40 x 25 block; the stock became the second box plus padding, and
//! the first block stood 5 mm out of the stock top.
//!
//! The rule now is `session::fit_auto_stock`: the stock fits the union
//! of the boxes of all models, with the per-box rule of
//! `StockConfig::update_from_bbox` (padding, Z frame). Every door that
//! changes the model set calls it: `AddModel`, `RemoveModel`,
//! `AdoptModelGeometry` (re-import, rescale, import options),
//! `EditModelDrawing` and the project load.
//!
//! Red before the fix (observed): the two-model assertion fails with the
//! stock at X 95..155, Y -15..35, Z 0..30, the second box plus padding
//! only. The test stops there, so the remove and re-import arms were not
//! observed red; before the fix `remove_model` ran no refit.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::sync::Arc;

use rs_cam_core::compute::StockConfig;
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ModelId;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::session::{
    AddModelArgs, AdoptModelGeometryArgs, Command, LoadedModel, ProjectSession, RemoveModelArgs,
    SetStockConfigArgs,
};

/// A closed box mesh from `min` to `max`.
fn box_mesh(min: P3, max: P3) -> TriangleMesh {
    let v = vec![
        P3::new(min.x, min.y, max.z),
        P3::new(max.x, min.y, max.z),
        P3::new(max.x, max.y, max.z),
        P3::new(min.x, max.y, max.z),
        P3::new(min.x, min.y, min.z),
        P3::new(max.x, min.y, min.z),
        P3::new(max.x, max.y, min.z),
        P3::new(min.x, max.y, min.z),
    ];
    let t = vec![
        [0, 1, 2],
        [0, 2, 3],
        [4, 6, 5],
        [4, 7, 6],
        [0, 4, 5],
        [0, 5, 1],
        [1, 5, 6],
        [1, 6, 2],
        [2, 6, 7],
        [2, 7, 3],
        [3, 7, 4],
        [3, 4, 0],
    ];
    TriangleMesh::from_raw(v, t)
}

fn box_model(name: &str, min: P3, max: P3) -> LoadedModel {
    LoadedModel {
        id: ModelId::placeholder(),
        name: name.to_owned(),
        mesh: Some(Arc::new(box_mesh(min, max))),
        items: Default::default(),
        path: format!("synthetic://{name}.stl").into(),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    }
}

/// The stock box that `update_from_bbox` gives for a mesh box: padding on
/// each XY side, on the top only in Z, the bottom at the box bottom.
fn padded(stock: &StockConfig, bbox: &BoundingBox3) -> BoundingBox3 {
    let p = stock.padding;
    BoundingBox3 {
        min: P3::new(bbox.min.x - p, bbox.min.y - p, bbox.min.z),
        max: P3::new(bbox.max.x + p, bbox.max.y + p, bbox.max.z + p),
    }
}

fn assert_box_eq(got: &BoundingBox3, want: &BoundingBox3, what: &str) {
    let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
    assert!(
        close(got.min.x, want.min.x)
            && close(got.min.y, want.min.y)
            && close(got.min.z, want.min.z)
            && close(got.max.x, want.max.x)
            && close(got.max.y, want.max.y)
            && close(got.max.z, want.max.z),
        "{what}: stock {got:?}, want {want:?}"
    );
}

fn add(session: &mut ProjectSession, model: LoadedModel) -> ModelId {
    let raw = session
        .apply(Command::AddModel(AddModelArgs {
            model: Box::new(model),
        }))
        .expect("add the model")
        .created
        .expect("the add reports the model id");
    session
        .models()
        .iter()
        .find(|m| m.id.raw() == raw)
        .map(|m| m.id)
        .expect("the added model is in the session")
}

#[test]
fn the_auto_stock_holds_every_model_and_refits_on_remove_f31() {
    let mut session = ProjectSession::new_empty();
    assert!(
        session.stock_config().auto_from_model,
        "the default is auto"
    );

    let a_box = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(60.0, 40.0, 30.0),
    };
    let b_box = BoundingBox3 {
        min: P3::new(100.0, -10.0, 0.0),
        max: P3::new(150.0, 30.0, 25.0),
    };
    let _a = add(&mut session, box_model("a", a_box.min, a_box.max));
    // One model: the per-box rule, unchanged.
    let stock = session.stock_config().clone();
    assert_box_eq(&stock.bbox(), &padded(&stock, &a_box), "one model");

    let b = add(&mut session, box_model("b", b_box.min, b_box.max));
    let union = BoundingBox3 {
        min: P3::new(0.0, -10.0, 0.0),
        max: P3::new(150.0, 40.0, 30.0),
    };
    let stock = session.stock_config().clone();
    assert_box_eq(&stock.bbox(), &padded(&stock, &union), "two models");
    for (name, bbox) in [("a", &a_box), ("b", &b_box)] {
        let s = stock.bbox();
        assert!(
            s.min.x <= bbox.min.x
                && s.min.y <= bbox.min.y
                && s.min.z <= bbox.min.z
                && s.max.x >= bbox.max.x
                && s.max.y >= bbox.max.y
                && s.max.z >= bbox.max.z,
            "model {name} {bbox:?} sticks out of the stock {s:?}"
        );
    }

    // Remove `a` (index 0): the stock fits `b` alone.
    let _effects = session
        .apply(Command::RemoveModel(RemoveModelArgs { index: 0 }))
        .expect("remove a");
    let stock = session.stock_config().clone();
    assert_box_eq(&stock.bbox(), &padded(&stock, &b_box), "after remove");

    // A re-import that grows `b` refits the stock.
    let grown = BoundingBox3 {
        min: P3::new(100.0, -10.0, 0.0),
        max: P3::new(170.0, 30.0, 35.0),
    };
    let _effects = session
        .apply(Command::AdoptModelGeometry(AdoptModelGeometryArgs {
            model_id: b,
            geometry: Box::new(box_model("b", grown.min, grown.max)),
            units: None,
        }))
        .expect("re-import b");
    let stock = session.stock_config().clone();
    assert_box_eq(&stock.bbox(), &padded(&stock, &grown), "after re-import");
}

#[test]
fn a_user_set_stock_never_moves_on_a_model_change_f31() {
    let mut session = ProjectSession::new_empty();
    let user = StockConfig {
        x: 300.0,
        y: 200.0,
        z: Some(20.0),
        origin_x: -10.0,
        origin_y: -10.0,
        origin_z: -20.0,
        auto_from_model: false,
        ..StockConfig::default()
    };
    let _effects = session
        .apply(Command::SetStockConfig(SetStockConfigArgs {
            stock: Box::new(user.clone()),
        }))
        .expect("set the user stock");
    let _a = add(
        &mut session,
        box_model("a", P3::new(0.0, 0.0, 0.0), P3::new(60.0, 40.0, 30.0)),
    );
    let _b = add(
        &mut session,
        box_model("b", P3::new(0.0, 0.0, 0.0), P3::new(500.0, 40.0, 25.0)),
    );
    assert_eq!(
        session.stock_config(),
        &user,
        "an import moved a user stock"
    );
    let _effects = session
        .apply(Command::RemoveModel(RemoveModelArgs { index: 1 }))
        .expect("remove b");
    assert_eq!(session.stock_config(), &user, "a remove moved a user stock");
}
