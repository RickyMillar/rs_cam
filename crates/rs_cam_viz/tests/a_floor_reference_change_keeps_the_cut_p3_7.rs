//! Geometry selection P3.7 (ruling L-10): a change of the floor reference
//! keeps the cut.
//!
//! # The claim
//!
//! The floor row of Pocket, Profile, Zigzag and Adaptive 2D offers six
//! references. A change of reference re-expresses the value, so the
//! resolved floor Z stays: `FloorInputs::refs.re_express`, the one rule the
//! combo and the height diagram call. The test reads the floor Z back from
//! THE session rule (`ProjectSession::toolpath_heights`) after the GUI
//! write route (`ReplaceToolpathConfig`, L-17), so it measures the cut, not
//! the arithmetic of the helper.
//!
//! The fixture has a stock from Z -15 to Z 5 and a synthetic STEP part: a
//! flat face at Z 0 (the model top) and one picked up face at Z -4. The
//! start floor is `Top { depth: 3 }`: the auto top follows the face, so the
//! floor is at Z -7. The references read four different Z values.
//!
//! Red proof: carry the old number into the new reference in
//! `FloorReferences::re_express` (`Ok(to.with_value(from.value()))`). The
//! Stock top arm then reads Z 2 (5 - 3), and the test fails.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test a_floor_reference_change_keeps_the_cut_p3_7
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::floor::{Floor, FloorRefusal};
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::{P3, V3};
use rs_cam_core::geometry::enriched_mesh::{
    FaceTessellation, SurfaceParams, SurfaceType, build_enriched_mesh,
};
use rs_cam_core::io::brep_face_items;
use rs_cam_core::selection::{MatchKey, Selection, SelectionEdit, Shape, SourceRef};
use rs_cam_core::session::{
    Command, EditSelectionArgs, LoadedModel, ProjectSession, ProjectSessionBuilder,
    ReplaceToolpathConfigArgs, SelectionTarget, ToolpathConfig,
};
use rs_cam_viz::ui::properties::{FloorInputs, floor_runs};

/// The plane Z of the picked face (mm).
const FACE_Z: f64 = -4.0;
/// The start depth below the Top Z (mm): the floor sits at Z -7.
const DEPTH: f64 = 3.0;
const FLOOR_Z: f64 = FACE_Z - DEPTH;

/// A flat up face of the synthetic part: the square `[x0, x0 + 20]` by
/// `[-10, 10]` at `z`.
fn flat(x0: f64, z: f64) -> FaceTessellation {
    let v = vec![
        P3::new(x0, -10.0, z),
        P3::new(x0 + 20.0, -10.0, z),
        P3::new(x0 + 20.0, 10.0, z),
        P3::new(x0, 10.0, z),
    ];
    FaceTessellation {
        boundary_loops: vec![v.clone()],
        vertices: v,
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        surface_type: SurfaceType::Plane,
        surface_params: SurfaceParams::Plane {
            normal: V3::new(0.0, 0.0, 1.0),
            d: z,
        },
    }
}

/// A synthetic STEP part: a flat face at Z 0 (the model top) and the face
/// to pick at [`FACE_Z`]. Its faces are BREP face items.
fn model() -> LoadedModel {
    let enriched = build_enriched_mesh(
        vec![flat(-20.0, 0.0), flat(0.0, FACE_Z)],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(1),
        path: PathBuf::from("synthetic://floor_fixture.step"),
        name: "Floor fixture".to_owned(),
        kind: None,
        mesh: Some(Arc::clone(&enriched.mesh)),
        items: Arc::new(brep_face_items(&enriched)),
        enriched_mesh: Some(Arc::new(enriched)),
        units: None,
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    }
}

/// The pick key of the face at [`FACE_Z`].
fn face_key() -> MatchKey {
    let model = model();
    let item = model
        .items
        .iter()
        .find(|item| match item.shape() {
            Shape::Face(face) => (face.plane_z - FACE_Z).abs() < 1e-9,
            _ => false,
        })
        .expect("the part holds the face to pick")
        .clone();
    MatchKey {
        source: SourceRef::ToolpathModel,
        ..item.key().clone()
    }
}

fn toolpath() -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        id: rs_cam_core::ToolpathId::from_raw(0),
        name: "Floor pocket".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(PocketConfig {
            floor: Floor::Top { depth: DEPTH },
            selection: Selection::empty(),
            ..PocketConfig::default()
        }),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: rs_cam_core::ids::ToolId::from_raw(1),
        model_id: rs_cam_core::ids::ModelId::from_raw(1),
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    }
}

/// A Pocket with the face picked, under `Top { depth: 3 }`.
fn session() -> ProjectSession {
    let mut tool = ToolConfig::new_default(ToolId::from_raw(1), ToolType::EndMill);
    tool.diameter = 3.0;
    let mut builder = ProjectSessionBuilder::new()
        .stock(StockConfig {
            x: 60.0,
            y: 60.0,
            z: 20.0,
            origin_x: -30.0,
            origin_y: -30.0,
            origin_z: -15.0,
            auto_from_model: false,
            ..StockConfig::default()
        })
        .tool(tool)
        .model(model());
    builder.add_toolpath(0, toolpath()).unwrap();
    let mut session = builder.build();
    let _ = session
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit: SelectionEdit::AddPick(face_key()),
        }))
        .unwrap();
    session
}

/// The floor inputs the panel builds for toolpath 0.
fn inputs(session: &ProjectSession) -> FloorInputs {
    let tc = &session.toolpath_configs()[0];
    let answer = session.toolpath_heights(tc).unwrap();
    let report = session.selection_status(tc, SelectionTarget::Op).unwrap();
    FloorInputs::new(&answer, &tc.heights, Some(&report))
}

/// The resolved floor Z of toolpath 0: THE session rule.
fn floor_z(session: &ProjectSession) -> f64 {
    let tc = &session.toolpath_configs()[0];
    session
        .toolpath_heights(tc)
        .unwrap()
        .floor_z(&tc.heights)
        .expect("a floor op")
        .expect("the floor resolves")
}

/// Write `floor` through the GUI route: the whole config
/// (`ReplaceToolpathConfig`, L-17).
fn write(session: &mut ProjectSession, floor: Floor) {
    let mut config = session.toolpath_configs()[0].clone();
    assert!(config.operation.set_floor(floor));
    let _ = session
        .apply(Command::ReplaceToolpathConfig(ReplaceToolpathConfigArgs {
            index: 0,
            config: Box::new(config),
        }))
        .expect("the door takes the re-expressed floor");
}

#[test]
fn a_floor_reference_change_keeps_the_cut_p3_7() {
    let mut s = session();
    assert!(
        (floor_z(&s) - FLOOR_Z).abs() < 1e-9,
        "fixture: the floor starts at Z {FLOOR_Z}, got {}",
        floor_z(&s)
    );
    // Non-vacuity: the references read four different Z values here (the
    // stock top 5, the model top 0, the face and the auto Top Z -4, the
    // stock bottom -15), so a number carried across moves the floor.
    let refs = inputs(&s).refs;
    let mut reference_z: Vec<f64> = Floor::EACH_REFERENCE
        .iter()
        .map(|f| refs.reference_z(f).unwrap())
        .collect();
    reference_z.sort_by(f64::total_cmp);
    reference_z.dedup();
    assert_eq!(
        reference_z.len(),
        4,
        "the references must differ: {reference_z:?}"
    );

    // Walk the references in order, then back to the start: each change
    // keeps the resolved floor Z.
    let mut walk: Vec<Floor> = Floor::EACH_REFERENCE.to_vec();
    walk.push(Floor::Top { depth: 0.0 });
    for template in walk {
        let from = s.toolpath_configs()[0].operation.floor().unwrap();
        let to = inputs(&s)
            .refs
            .re_express(from, template)
            .unwrap_or_else(|r| panic!("{} refused: {r}", template.reference_label()));
        assert!(to.same_reference(&template));
        write(&mut s, to);
        let z = floor_z(&s);
        assert!(
            (z - FLOOR_Z).abs() < 1e-9,
            "{} -> {}: the floor moved from Z {FLOOR_Z} to Z {z} ({to:?})",
            from.reference_label(),
            template.reference_label()
        );
    }
}

/// A reference with no Z, and a value the domain refuses, are not offered:
/// the combo disables the entry with this reason, and nothing is written.
#[test]
fn a_reference_with_no_z_is_not_offered_p3_7() {
    let s = session();
    let mut refs = inputs(&s).refs;
    let from = Floor::Top { depth: DEPTH };
    refs.model_top_z = None;
    refs.face_bottom_z = None;
    assert_eq!(
        refs.re_express(from, Floor::ModelTop { depth: 0.0 }),
        Err(FloorRefusal::NoModelMesh)
    );
    assert_eq!(
        refs.re_express(from, Floor::EachFace { depth: 0.0 }),
        Err(FloorRefusal::NoFace)
    );
    // A Z above the Top Z is not a Top depth: the domain refuses it.
    assert!(matches!(
        refs.re_express(
            Floor::Z {
                z: refs.top_z + 1.0
            },
            Floor::Top { depth: 0.0 }
        ),
        Err(FloorRefusal::OutOfDomain { .. })
    ));
}

/// Geometry selection P3.8 (the P3.7 gap): each floor run of the height
/// diagram has a hover with its Z and its reference, and under "each picked
/// face" the count of faces at that floor. The runs come from THE session
/// rule (`FloorInputs` of `toolpath_heights` and `selection_status`).
///
/// Red proof: the face count dropped from `FloorInputs::new` (each distinct
/// floor counts 1 face): the "2 faces" arm fails.
#[test]
fn each_floor_run_names_its_reference_and_faces_p3_8() {
    let mut s = session();
    // One floor: "FL" with the reference of the floor.
    let floor = Floor::Top { depth: DEPTH };
    let runs = floor_runs(Some((&floor, &inputs(&s))), f64::NAN);
    assert_eq!(runs.len(), 1);
    assert!(!runs[0].dashed);
    assert_eq!(
        runs[0].hover,
        format!("FL: Z {FLOOR_Z:.1}, floor from Top Z")
    );

    // Each picked face: pick the model top face too. The two faces are at
    // Z 0 and Z -4, so with depth 0 the faces give two floors; the stock
    // top is Z 5, so neither face is at the top.
    let top_face = {
        let model = model();
        let item = model
            .items
            .iter()
            .find(|item| match item.shape() {
                Shape::Face(face) => face.plane_z.abs() < 1e-9,
                _ => false,
            })
            .expect("the part holds the model top face")
            .clone();
        MatchKey {
            source: SourceRef::ToolpathModel,
            ..item.key().clone()
        }
    };
    let _ = s
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit: SelectionEdit::AddPick(top_face),
        }))
        .unwrap();
    let each = Floor::EachFace { depth: 0.0 };
    write(&mut s, each);
    let runs = floor_runs(Some((&each, &inputs(&s))), f64::NAN);
    let hovers: Vec<&str> = runs.iter().map(|r| r.hover.as_str()).collect();
    assert_eq!(
        hovers,
        [
            "F1: Z 0.0, 1 face, floor from Each picked face",
            "F2: Z -4.0, 1 face, floor from Each picked face",
        ]
    );
    assert!(runs.iter().all(|r| r.dashed));

    // Two cut faces at one floor give one run with a count of 2: a report
    // with the floor Z -4 on two items and no floor on a third.
    let tc = &s.toolpath_configs()[0];
    let answer = s.toolpath_heights(tc).unwrap();
    let report = rs_cam_core::session::SelectionReport {
        item_floor_z: vec![Some(-4.0), None, Some(-4.0)],
        ..Default::default()
    };
    let two = FloorInputs::new(&answer, &tc.heights, Some(&report));
    let runs = floor_runs(Some((&each, &two)), f64::NAN);
    let hovers: Vec<&str> = runs.iter().map(|r| r.hover.as_str()).collect();
    assert_eq!(hovers, ["F1: Z -4.0, 2 faces, floor from Each picked face"]);
}
