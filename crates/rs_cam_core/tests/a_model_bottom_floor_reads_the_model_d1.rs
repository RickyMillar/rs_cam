//! Geometry selection D1 (REFERENCE_AUDIT row D1, Fusion "Model bottom"):
//! the floor reference "Model bottom".
//!
//! # The claims
//!
//! 1. On the STEP stepped block (model Z 0..30) in a stock Z -5..30, a
//!    Pocket of the Z 30 face with the floor `{ from = "model_bottom",
//!    depth = d }` resolves its floor to the model bottom less `d`: the
//!    lowest Z of the model in the setup frame. Generation cuts down to
//!    that Z and not below it.
//! 2. The depth is signed as for every other reference: a negative depth
//!    stands above the model bottom, a positive depth goes below it. No
//!    rule clamps the floor at the stock bottom (as for "Model top").
//! 3. On a DXF drawing the model has no mesh, so it has no bottom. The
//!    `set_toolpath_floor` door takes the floor (as it takes "Model top"),
//!    the heights answer `FloorRefusal::NoModelMesh`, and generation
//!    refuses with that text.
//!
//! Red proof (2026-10-23): with `Floor::resolve` of `ModelBottom` reading
//! `ctx.model_top_z` (the model top, Z 30), claim 1 fails: the floor
//! refuses `AboveTop` (Z 31 over the top Z 30).
//!
//! ```text
//! cargo test -p rs_cam_core -q --features step --test a_model_bottom_floor_reads_the_model_d1
//! ```

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use common::session::single_op_session_with;
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::floor::{Floor, FloorRefusal};
use rs_cam_core::compute::operation_configs::PocketConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::ids::ModelId;
use rs_cam_core::selection::{Selection, Shape, SourceRef};
use rs_cam_core::session::{
    Command, LoadedModel, ProjectSession, SessionError, SetToolpathFloorArgs,
};
use rs_cam_core::toolpath::MoveType;

const STEPPED: &str = "tests/fixtures/step/stepped_block.step";
const DXF: &str = "tests/fixtures/selection_p2_7/layers.dxf";

/// The model top of the stepped block (mm): the face that the Pocket cuts.
const MODEL_TOP: f64 = 30.0;
/// The stock reaches this far below the model bottom (mm), so the model
/// bottom and the stock bottom are two Z values.
const STOCK_BELOW: f64 = 5.0;
/// The tool diameter (mm).
const TOOL_DIAMETER: f64 = 6.0;
/// The tolerance of a Z (mm).
const EPS: f64 = 1e-6;

fn load(rel: &str, kind: ModelKind) -> LoadedModel {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    rs_cam_core::io::load_model_file(
        &path,
        ModelId::from_raw(0),
        kind,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// The stock X -5..65, Y -5..45, Z -5..30 (the part box with a margin).
fn stock() -> StockConfig {
    StockConfig {
        x: 70.0,
        y: 50.0,
        z: Some(MODEL_TOP + STOCK_BELOW),
        origin_x: -5.0,
        origin_y: -5.0,
        origin_z: -STOCK_BELOW,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// A Pocket of the Z 30 face of the stepped block.
fn step_session() -> ProjectSession {
    let model = load(STEPPED, ModelKind::Step);
    let top_face = model
        .items
        .iter()
        .find(|item| {
            matches!(item.shape(), Shape::Face(face) if (face.plane_z - MODEL_TOP).abs() < EPS)
        })
        .expect("the stepped block has a Z 30 face")
        .key()
        .clone();
    let pick = rs_cam_core::selection::MatchKey {
        source: SourceRef::ToolpathModel,
        ..top_face
    };
    single_op_session_with(
        stock(),
        endmill_tool_config(TOOL_DIAMETER),
        model,
        "d1 pocket",
        OperationConfig::Pocket(PocketConfig {
            selection: Selection::from_picks(vec![pick]),
            ..PocketConfig::default()
        }),
        |_| {},
    )
}

/// The lowest Z of the model mesh: the model bottom in the Top setup.
fn model_bottom(s: &ProjectSession) -> f64 {
    let mesh = s.models()[0]
        .mesh
        .as_ref()
        .expect("a STEP model has a mesh");
    mesh.bbox.min.z
}

fn set_floor(s: &mut ProjectSession, floor: Floor) {
    let _ = s
        .apply(Command::SetToolpathFloor(SetToolpathFloorArgs {
            index: 0,
            floor,
        }))
        .expect("the door takes the model bottom floor");
}

fn resolved_floor(s: &ProjectSession) -> Result<f64, FloorRefusal> {
    let tc = s.toolpath_configs()[0].clone();
    let answer = s.toolpath_heights(&tc).expect("the heights resolve");
    answer.floor_z(&tc.heights).expect("a Pocket holds a floor")
}

/// The lowest Z of a feed move of the generated Pocket.
fn lowest_cut(s: &mut ProjectSession) -> f64 {
    let cancel = AtomicBool::new(false);
    let result = s
        .generate_toolpath(0, &cancel)
        .unwrap_or_else(|e| panic!("the Pocket generates: {e}"));
    result
        .op_data
        .annotated()
        .toolpath
        .moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .map(|m| m.target.z)
        .fold(f64::INFINITY, f64::min)
}

/// Claims 1 and 2: the floor is the model bottom less the depth, and the
/// cut goes down to it.
#[test]
fn a_model_bottom_floor_cuts_to_the_model_bottom_d1() {
    let mut s = step_session();
    let bottom = model_bottom(&s);
    assert!(
        bottom.abs() < EPS,
        "fixture: the stepped block stands on Z 0, got {bottom}"
    );
    let stock_bottom = -STOCK_BELOW;

    // A depth of -1 stands 1 mm above the model bottom.
    set_floor(&mut s, Floor::ModelBottom { depth: -1.0 });
    let floor = resolved_floor(&s).expect("the floor resolves on a mesh");
    assert!(
        (floor - (bottom + 1.0)).abs() < EPS,
        "floor {floor} is not the model bottom {bottom} + 1"
    );
    let lowest = lowest_cut(&mut s);
    assert!(
        (lowest - floor).abs() < EPS,
        "the Pocket cuts to Z {lowest}, not to the floor Z {floor}"
    );

    // A depth of 0 is the model bottom.
    set_floor(&mut s, Floor::ModelBottom { depth: 0.0 });
    let floor = resolved_floor(&s).unwrap();
    assert!((floor - bottom).abs() < EPS, "floor {floor} vs {bottom}");

    // A positive depth goes below it, and no rule stops it at the stock
    // bottom.
    let depth = STOCK_BELOW + 3.0;
    set_floor(&mut s, Floor::ModelBottom { depth });
    let floor = resolved_floor(&s).unwrap();
    assert!(
        (floor - (bottom - depth)).abs() < EPS && floor < stock_bottom,
        "floor {floor}: not the model bottom less {depth} below the stock bottom \
         {stock_bottom}"
    );
}

/// Claim 3: a drawing has no model bottom. The door takes the floor, and
/// generation refuses with the typed floor refusal text.
#[test]
fn a_model_bottom_floor_refuses_on_a_drawing_d1() {
    let mut s = single_op_session_with(
        stock(),
        endmill_tool_config(TOOL_DIAMETER),
        load(DXF, ModelKind::Dxf),
        "d1 dxf pocket",
        OperationConfig::Pocket(PocketConfig::default()),
        |_| {},
    );
    assert!(s.models()[0].mesh.is_none(), "fixture: a DXF has no mesh");
    set_floor(&mut s, Floor::ModelBottom { depth: 0.0 });
    let tc = s.toolpath_configs()[0].clone();
    let answer = s.toolpath_heights(&tc).unwrap();
    assert_eq!(
        answer.floor_refusal(&tc.heights),
        Some(FloorRefusal::NoModelMesh)
    );
    let cancel = AtomicBool::new(false);
    let err = s.generate_toolpath(0, &cancel).map(|_| ()).unwrap_err();
    let SessionError::InvalidParam(text) = &err else {
        panic!("the floor refusal, got {err:?}");
    };
    assert!(
        text.contains(&FloorRefusal::NoModelMesh.to_string()),
        "the refusal names the missing model Z: {text}"
    );
}
