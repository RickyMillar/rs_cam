//! G-MODELOUTLINE — a selection boundary confines a toolpath to the
//! closed regions of ANOTHER model, and it refuses when it cannot.
//!
//! Geometry selection P2.5 replaced `BoundarySource::ModelOutline` and
//! `FaceSelection` with `BoundarySource::Selection`. The old outline of a
//! model is `Selection::closed_shapes_of(model)` (its loops and circles).
//! The selection resolves through the door of an op selection (`resolve`,
//! the setup frame, `view` with `ItemUse::Regions`).
//!
//! # The need
//!
//! The operator machines a terrain STL, and a DXF with one ring (one
//! polygon with one hole) states where the machine may cut. A 3D op on the
//! STL must take its boundary from the DXF. Before this source existed,
//! `BoundarySource::Geometry` fell to the stock rectangle in silence.
//!
//! # The fixture
//!
//! - Model A: a flat 60 x 60 mm plate mesh at z = 1. The toolpath cuts it.
//! - Model B: a ring drawing, built through `detect_containment` as the
//!   DXF import builds it. The exterior is a 40.6 mm square and the hole a
//!   16.6 mm square, both centred. The odd sizes keep raster rows off the
//!   edges.
//! - Model F: a STEP-like model with one flat up face, a 16.6 mm square at
//!   the centre (the size of the ring's hole).
//! - A drop-cutter raster on model A, with the boundary on model B's
//!   closed shapes, containment `Center`.
//!
//! # What is asserted
//!
//! 1. Every CUTTING move lies inside the ring: not outside it, not in the
//!    hole. The test samples each linear segment, not only its end, because
//!    a straight segment between two ring points can cross the hole.
//! 2. Non-vacuity: the same op with the boundary off cuts in the hole and
//!    outside the ring, so assertion 1 can fail.
//! 3. A model id that names no model refuses, and the message names the id
//!    and lists the project models.
//! 4. A model with no closed region refuses, and the message names the
//!    model. Two arms: a mesh (no 2D geometry) and a drawing with only an
//!    open path.
//! 5. The holes option (`holes: true`) on the same ring: every cutting
//!    move lies inside the ring's HOLE, and none lies in the ring or
//!    outside it. This is the edge-band case: the toolpath machines the
//!    area inside the band and stays off the band.
//! 6. The holes option on a model with closed shapes but no hole refuses,
//!    and the message names the model.
//! 7. A face pick (the old `FaceSelection` boundary): every cutting move
//!    lies inside the picked face.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use common::meshes::height_field;
use common::session::{mesh_model, polygon_model, square_polygon, stock_over, toolpath_config};
use common::tools::ball_tool_config;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, BoundaryContainment, BoundarySource};
use rs_cam_core::compute::operation_configs::DropCutterConfig;
use rs_cam_core::geo::{P2, P3, V3};
use rs_cam_core::geometry::enriched_mesh::{
    FaceTessellation, SurfaceParams, SurfaceType, build_enriched_mesh,
};
use rs_cam_core::ids::ModelId;
use rs_cam_core::io::brep_face_items;
use rs_cam_core::polygon::{Polygon2, detect_containment};
use rs_cam_core::selection::{ItemKind, Selection};
use rs_cam_core::session::{
    Command, LoadedModel, ProjectSession, ProjectSessionBuilder, SetBoundaryConfigArgs,
};
use rs_cam_core::toolpath::MoveType;

const PLATE_HALF: f64 = 30.0;
const RING_OUTER_HALF: f64 = 20.3;
const RING_HOLE_HALF: f64 = 8.3;
/// The clip puts its intersection points ON the ring edge, where a strict
/// point-in-polygon test is ambiguous.
const EPS_MM: f64 = 1e-3;
/// Samples per linear segment, ends included.
const SEGMENT_SAMPLES: usize = 11;

/// The ring drawing, nested by the import's own containment pass.
fn ring() -> Polygon2 {
    let nested = detect_containment(vec![
        square_polygon(RING_OUTER_HALF),
        square_polygon(RING_HOLE_HALF),
    ]);
    assert_eq!(nested.len(), 1, "the two squares must nest as one ring");
    assert_eq!(nested[0].holes.len(), 1, "the inner square must be a hole");
    nested.into_iter().next().unwrap()
}

struct Fixture {
    session: ProjectSession,
    mesh_id: usize,
    ring_id: usize,
    open_id: usize,
    solid_id: usize,
    face_id: usize,
}

/// The ids of the drawing and mesh models, in the order the fixture adds
/// them. The face model id is on [`Fixture`].
#[derive(Clone, Copy)]
struct Ids {
    mesh: usize,
    ring: usize,
    open: usize,
    solid: usize,
}

/// A STEP-like model with one flat up face: a square of half-width
/// [`RING_HOLE_HALF`] at the centre, at z = 1.
fn face_model() -> LoadedModel {
    let h = RING_HOLE_HALF;
    let v = vec![
        P3::new(-h, -h, 1.0),
        P3::new(h, -h, 1.0),
        P3::new(h, h, 1.0),
        P3::new(-h, h, 1.0),
    ];
    let face = FaceTessellation {
        boundary_loops: vec![v.clone()],
        vertices: v,
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        surface_type: SurfaceType::Plane,
        surface_params: SurfaceParams::Plane {
            normal: V3::new(0.0, 0.0, 1.0),
            d: 1.0,
        },
    };
    let enriched = build_enriched_mesh(vec![face], Vec::new(), Vec::new()).unwrap();
    LoadedModel {
        mesh: Some(Arc::clone(&enriched.mesh)),
        items: Arc::new(brep_face_items(&enriched)),
        enriched_mesh: Some(Arc::new(enriched)),
        ..mesh_model(height_field(1.0, 1.0, |_, _| 1.0), "face_part")
    }
}

/// The face pick of the one face of model `face_id` in `session`.
fn face_pick(session: &ProjectSession, face_id: usize) -> Selection {
    let model = session
        .models()
        .iter()
        .find(|m| m.id == rs_cam_core::ids::ModelId::from_raw(face_id))
        .expect("the face model");
    let key = model
        .items
        .iter()
        .find(|item| item.kind() == ItemKind::Face)
        .expect("the face model holds a Face item")
        .key()
        .clone();
    Selection::from_picks(vec![key])
}

/// One session: the plate mesh, the ring drawing, a drawing with only an
/// open path, a drawing with one closed square and no hole, and one
/// drop-cutter toolpath on the plate. `source` is the boundary; `None`
/// disables it.
fn fixture(source: impl FnOnce(Ids) -> Option<BoundarySource>) -> Fixture {
    let mut builder = ProjectSessionBuilder::new().stock(stock_over(PLATE_HALF, 5.0));
    let tool_idx = builder.add_tool(ball_tool_config(3.0));
    let tool_id = builder.tools()[tool_idx].id.raw();
    let mesh_id = builder.add_model(mesh_model(
        height_field(PLATE_HALF, 5.0, |_, _| 1.0),
        "terrain",
    ));
    let ring_id = builder.add_model(polygon_model(vec![ring()], "machinable_edge_band"));
    let open_path = Polygon2::with_holes_closed(
        vec![P2::new(-10.0, 0.0), P2::new(0.0, 5.0), P2::new(10.0, 0.0)],
        Vec::new(),
        false,
    );
    let open_id = builder.add_model(polygon_model(vec![open_path], "rivers"));
    let solid_id = builder.add_model(polygon_model(
        vec![square_polygon(RING_OUTER_HALF)],
        "solid_board",
    ));
    let face_id = builder.add_model(face_model());

    let op = OperationConfig::DropCutter(DropCutterConfig {
        stepover: 1.0,
        ..DropCutterConfig::default()
    });
    let mut cfg = toolpath_config(
        "Rough",
        op,
        rs_cam_core::ids::ToolId::from_raw(tool_id),
        mesh_id,
    );
    let ids = Ids {
        mesh: mesh_id.raw(),
        ring: ring_id.raw(),
        open: open_id.raw(),
        solid: solid_id.raw(),
    };
    if let Some(source) = source(ids) {
        cfg.boundary = BoundaryConfig {
            enabled: true,
            source,
            containment: BoundaryContainment::Center,
            offset: 0.0,
        };
    }
    builder
        .add_toolpath(0, cfg)
        .expect("add toolpath to a fresh session");
    Fixture {
        session: builder.build(),
        mesh_id: mesh_id.raw(),
        ring_id: ring_id.raw(),
        open_id: open_id.raw(),
        solid_id: solid_id.raw(),
        face_id: face_id.raw(),
    }
}

/// XY samples along every cutting move, ends included.
fn cutting_samples(session: &ProjectSession) -> Vec<P2> {
    let moves = &session
        .result_at(0)
        .expect("a generated result")
        .toolpath()
        .moves;
    let mut samples = Vec::new();
    for pair in moves.windows(2) {
        let (from, to) = (&pair[0], &pair[1]);
        match to.move_type {
            MoveType::Rapid => {}
            MoveType::Linear { .. } => {
                for k in 0..SEGMENT_SAMPLES {
                    let t = k as f64 / (SEGMENT_SAMPLES - 1) as f64;
                    samples.push(P2::new(
                        from.target.x + t * (to.target.x - from.target.x),
                        from.target.y + t * (to.target.y - from.target.y),
                    ));
                }
            }
            // An arc is not sampled along its sweep; its end is.
            MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => {
                samples.push(P2::new(to.target.x, to.target.y));
            }
        }
    }
    samples
}

fn generate(session: &mut ProjectSession) -> Result<(), String> {
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[test]
fn every_cutting_move_lies_inside_the_ring_g_modeloutline() {
    let mut fx = fixture(|ids| {
        Some(BoundarySource::Selection {
            selection: Selection::closed_shapes_of(ModelId::from_raw(ids.ring)),
            holes: false,
        })
    });
    generate(&mut fx.session).expect("a ring outline boundary must generate");

    let ring = ring();
    let samples = cutting_samples(&fx.session);
    assert!(
        samples.len() > 100,
        "a confined op that emits almost no cutting motion proves nothing: {} samples",
        samples.len()
    );
    let escaped: Vec<&P2> = samples
        .iter()
        .filter(|p| !ring.contains_point_eps(p, EPS_MM))
        .collect();
    assert!(
        escaped.is_empty(),
        "{} of {} cutting samples lie outside the ring or in its hole. First: {:?}",
        escaped.len(),
        samples.len(),
        escaped.first()
    );
}

/// Non-vacuity for the test above: with the boundary off the same op cuts
/// in the hole AND outside the ring.
#[test]
fn without_the_boundary_the_op_cuts_the_hole_and_the_margin_g_modeloutline() {
    let mut fx = fixture(|_| None);
    generate(&mut fx.session).expect("the unbounded op must generate");

    let samples = cutting_samples(&fx.session);
    let in_hole = samples
        .iter()
        .filter(|p| p.x.abs() < RING_HOLE_HALF - 1.0 && p.y.abs() < RING_HOLE_HALF - 1.0)
        .count();
    let outside = samples
        .iter()
        .filter(|p| p.x.abs() > RING_OUTER_HALF + 1.0 || p.y.abs() > RING_OUTER_HALF + 1.0)
        .count();
    assert!(in_hole > 0, "the unbounded op must cut in the hole region");
    assert!(outside > 0, "the unbounded op must cut outside the ring");
}

#[test]
fn a_missing_model_refuses_and_names_the_id_g_modeloutline() {
    let mut fx = fixture(|_| {
        Some(BoundarySource::Selection {
            selection: Selection::closed_shapes_of(ModelId::from_raw(999)),
            holes: false,
        })
    });
    let err = generate(&mut fx.session).expect_err("a missing outline model must refuse");
    assert!(
        err.contains("999"),
        "the refusal must name the missing model id: {err}"
    );
    assert!(
        fx.session.result_at(0).is_none(),
        "a refused generation must cache no result"
    );
    // The ids the operator can pick are listed.
    assert!(
        err.contains(&format!("{} ('machinable_edge_band')", fx.ring_id)),
        "the refusal must list the project models: {err}"
    );
}

#[test]
fn a_model_with_no_closed_polygon_refuses_and_names_it_g_modeloutline() {
    // A mesh has no 2D geometry at all.
    let mut fx = fixture(|ids| {
        Some(BoundarySource::Selection {
            selection: Selection::closed_shapes_of(ModelId::from_raw(ids.mesh)),
            holes: false,
        })
    });
    let err = generate(&mut fx.session).expect_err("a mesh outline must refuse");
    assert!(
        err.contains("terrain") && err.contains(&fx.mesh_id.to_string()),
        "the refusal must name the model: {err}"
    );
    assert!(fx.session.result_at(0).is_none());

    // A drawing with only an open path has no closed polygon.
    let mut fx = fixture(|ids| {
        Some(BoundarySource::Selection {
            selection: Selection::closed_shapes_of(ModelId::from_raw(ids.open)),
            holes: false,
        })
    });
    let err = generate(&mut fx.session).expect_err("an open-path outline must refuse");
    assert!(
        err.contains("rivers") && err.contains(&fx.open_id.to_string()),
        "the refusal must name the model: {err}"
    );
    assert!(fx.session.result_at(0).is_none());
}

/// The edge-band case: `holes: true` on the ring. Every cutting sample
/// lies inside the ring's hole. None lies in the ring or outside it.
#[test]
fn the_holes_option_cuts_only_inside_the_hole_g_modeloutline() {
    let mut fx = fixture(|ids| {
        Some(BoundarySource::Selection {
            selection: Selection::closed_shapes_of(ModelId::from_raw(ids.ring)),
            holes: true,
        })
    });
    generate(&mut fx.session).expect("a ring-holes boundary must generate");

    let hole = square_polygon(RING_HOLE_HALF);
    let ring = ring();
    let samples = cutting_samples(&fx.session);
    // The hole is a 16.6 mm square at a 1 mm stepover: about 16 rows.
    assert!(
        samples.len() > 20,
        "a confined op that emits almost no cutting motion proves nothing: {} samples",
        samples.len()
    );
    let outside_hole: Vec<&P2> = samples
        .iter()
        .filter(|p| !hole.contains_point_eps(p, EPS_MM))
        .collect();
    assert!(
        outside_hole.is_empty(),
        "{} of {} cutting samples lie outside the ring's hole. First: {:?}",
        outside_hole.len(),
        samples.len(),
        outside_hole.first()
    );
    // The same claim from the ring's side: no sample lies in the ring's
    // area more than EPS_MM from its edge, and none lies outside the ring.
    let on_the_band: Vec<&P2> = samples
        .iter()
        .filter(|p| ring.contains_point(p) && ring.distance_to_boundary(p) > EPS_MM)
        .collect();
    assert!(
        on_the_band.is_empty(),
        "{} of {} cutting samples lie in the ring (the band). First: {:?}",
        on_the_band.len(),
        samples.len(),
        on_the_band.first()
    );
    let outside_ring = samples
        .iter()
        .filter(|p| p.x.abs() > RING_OUTER_HALF + EPS_MM || p.y.abs() > RING_OUTER_HALF + EPS_MM)
        .count();
    assert_eq!(
        outside_ring, 0,
        "no cutting sample may lie outside the ring"
    );
}

/// `holes: true` on a model whose closed shapes have no hole refuses, and
/// the message names the model.
#[test]
fn the_holes_option_on_a_model_with_no_hole_refuses_and_names_it_g_modeloutline() {
    let mut fx = fixture(|ids| {
        Some(BoundarySource::Selection {
            selection: Selection::closed_shapes_of(ModelId::from_raw(ids.solid)),
            holes: true,
        })
    });
    let err = generate(&mut fx.session).expect_err("a holes boundary with no hole must refuse");
    assert!(
        err.contains("solid_board") && err.contains(&fx.solid_id.to_string()),
        "the refusal must name the model: {err}"
    );
    assert!(
        err.contains("no hole"),
        "the refusal must say that the shapes have no hole: {err}"
    );
    assert!(
        fx.session.result_at(0).is_none(),
        "a refused generation must cache no result"
    );
}

/// The old `FaceSelection` boundary: a pick of one flat face. Every
/// cutting sample lies inside the face.
#[test]
fn a_face_pick_boundary_cuts_only_inside_the_face_g_modeloutline() {
    let mut fx = fixture(|_| None);
    let selection = face_pick(&fx.session, fx.face_id);
    let _ = fx
        .session
        .apply(Command::SetBoundaryConfig(SetBoundaryConfigArgs {
            index: 0,
            boundary: BoundaryConfig {
                enabled: true,
                source: BoundarySource::Selection {
                    selection,
                    holes: false,
                },
                containment: BoundaryContainment::Center,
                offset: 0.0,
            },
        }))
        .expect("set the face boundary");
    generate(&mut fx.session).expect("a face-pick boundary must generate");

    let face = square_polygon(RING_HOLE_HALF);
    let samples = cutting_samples(&fx.session);
    assert!(
        samples.len() > 20,
        "a confined op that emits almost no cutting motion proves nothing: {} samples",
        samples.len()
    );
    let outside: Vec<&P2> = samples
        .iter()
        .filter(|p| !face.contains_point_eps(p, EPS_MM))
        .collect();
    assert!(
        outside.is_empty(),
        "{} of {} cutting samples lie outside the picked face. First: {:?}",
        outside.len(),
        samples.len(),
        outside.first()
    );
}
