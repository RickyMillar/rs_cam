//! A C3a: the orbit pivot and the pan grab point come from the model
//! surface under the press origin.
//!
//! # The claim
//!
//! `picking::surface_hit` gives the nearest model surface point on the
//! pointer ray, in the display frame:
//!
//! 1. A ray through a one-triangle mesh model gives the point on the
//!    triangle.
//! 2. A ray that misses the triangle gives `None`.
//! 3. A hidden model gives `None` (the pick tests only what the viewport
//!    draws).

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::selection::ItemSet;
use rs_cam_core::session::{LoadedModel, ProjectSession, ProjectSessionBuilder};
use rs_cam_viz::interaction::picking::{PickContext, surface_hit};
use rs_cam_viz::render::camera::OrbitCamera;
use rs_cam_viz::state::overlays::ModelVisibility;

const VW: f32 = 1600.0;
const VH: f32 = 900.0;

fn session() -> ProjectSession {
    let mesh = TriangleMesh::from_raw(
        vec![
            P3::new(-50.0, -50.0, 0.0),
            P3::new(50.0, -50.0, 0.0),
            P3::new(0.0, 50.0, 0.0),
        ],
        vec![[0, 1, 2]],
    );
    let model = LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(0),
        name: "triangle.stl".to_owned(),
        mesh: Some(Arc::new(mesh)),
        items: Arc::new(ItemSet::default()),
        path: PathBuf::from("triangle.stl"),
        kind: Some(ModelKind::Stl),
        units: Some(ModelUnits::Millimeters),
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    };
    let mut builder = ProjectSessionBuilder::new();
    let _ = builder.add_model(model);
    builder.build()
}

fn ctx<'a>(
    camera: &'a OrbitCamera,
    visibility: &'a ModelVisibility,
    at: [f32; 2],
) -> PickContext<'a> {
    PickContext {
        camera,
        screen_x: at[0],
        screen_y: at[1],
        aspect: VW / VH,
        vw: VW,
        vh: VH,
        visibility,
    }
}

#[test]
fn the_drag_anchor_is_the_surface_point_c3a() {
    let session = session();
    let camera = OrbitCamera::new();
    let shown = ModelVisibility::default();

    // 1. The centre ray meets the triangle. The expected point is the same
    //    ray (`unproject_ray`) cut by the plane z = 0, so the check holds
    //    the hit to the ray and not to the f32 round trip of the camera.
    let at = [VW * 0.5, VH * 0.5];
    let hit = surface_hit(&ctx(&camera, &shown, at), &session, None)
        .expect("the centre ray meets the triangle");
    let (o, d) = camera
        .unproject_ray(at[0], at[1], VW / VH, VW, VH)
        .expect("a ray");
    let t = -f64::from(o.z) / f64::from(d.z);
    let want = P3::new(
        f64::from(o.x) + f64::from(d.x) * t,
        f64::from(o.y) + f64::from(d.y) * t,
        0.0,
    );
    assert!(
        (hit - want).norm() < 1e-6,
        "the hit {hit:?} is not the ray point {want:?}"
    );
    assert!(
        hit.coords.norm() < 0.05,
        "the centre ray hit {hit:?}, far from the target"
    );

    // 2. The top-left ray passes far beyond the triangle.
    assert!(
        surface_hit(&ctx(&camera, &shown, [0.0, 0.0]), &session, None).is_none(),
        "a ray that misses the triangle gave a point"
    );

    // 3. A hidden model gives no point.
    let model_id = session.models().first().expect("one model").id;
    let mut hidden = ModelVisibility::default();
    let _ = hidden.set_visible(model_id, false);
    assert!(
        surface_hit(&ctx(&camera, &hidden, [VW * 0.5, VH * 0.5]), &session, None).is_none(),
        "a hidden model gave a point"
    );
}
