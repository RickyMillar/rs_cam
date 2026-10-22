//! UX-1c (SAFETY, operator ruling 2026-10-22): Chamfer, V-Carve, Inlay and
//! Inlay Plug on a face below standing material refuse at generation.
//!
//! # The defect
//!
//! These four ops cut one depth from the face plane, with no ladder that a
//! higher top can step down (`FaceTopRule::RefuseBuried`). UX-1b measured
//! them on the stepped block (face top Z 20, stock top Z 32, fresh stock):
//! Chamfer 13.0 mm into the material, V-Carve first cutting move 12.05 mm
//! under the stock top, Inlay 14.9 mm, Inlay Plug 12.9 mm.
//!
//! # The claim
//!
//! For each of the four ops:
//!
//! 1. Fresh stock, the Z 20 face picked, stock top Z 32: generation refuses
//!    with `OperationError::BuriedFace` and the ruling text ("The face is
//!    12.0 mm below the stock top and no earlier op clears it. Clear it
//!    first (Pocket or Face), or start from "After previous ops"."). The
//!    diagnostics state the same text before generation
//!    (`precondition.buried_face`, `Blocking`).
//! 2. Fresh stock, the Z 30 face picked, stock top Z 30 (the face AT the
//!    stock top): the op generates, and the diagnostics hold no
//!    `precondition.buried_face`.
//! 3. "After previous ops": a Pocket on the Z 20 face first clears it to
//!    0.5 mm below the face, then the op on the Z 20 face generates. The
//!    check reads the standing material over the face, not the stock box.
//! 4. The same op on "After previous ops" after a Pocket that stops 6 mm
//!    above the face (absolute floor Z 26) refuses with the
//!    standing-material text.
//!
//! One more arm: a PINNED top at the face (Manual Z 20) on fresh stock
//! does not refuse (the operator's statement, item 3 of the task).
//!
//! # The fixture
//!
//! The stepped block of the UX-1 sentry
//! (`a_face_top_never_starts_below_the_stock_top_ux1`), built in memory:
//! up faces Z 30 (X 0..20), Z 20 (X 20..40), Z 10 (X 40..60), Y 0..40.
//!
//! Red proof: with the session refusal (`ToolpathHeights::buried_face_refusal`)
//! returning `None`, every claim-1 arm fails (the op generates). With the
//! prologue check (`prologue::refuse_buried_face`) returning `Ok(())`,
//! every claim-4 arm fails.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_buried_face_refuses_a_one_depth_op_ux1c
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
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use common::session::{pick_faces_in, toolpath_config};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{HeightMode, StockSource};
use rs_cam_core::compute::execute::{BuriedUnder, OperationError};
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::operation_configs::{
    ChamferConfig, InlayConfig, InlayPlugConfig, PocketConfig, VCarveConfig,
};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolType};
use rs_cam_core::diagnostics::ids;
use rs_cam_core::geo::{P3, V3};
use rs_cam_core::geometry::enriched_mesh::{
    FaceTessellation, SurfaceParams, SurfaceType, build_enriched_mesh,
};
use rs_cam_core::ids::ToolId;
use rs_cam_core::io::brep_face_items;
use rs_cam_core::selection::Selection;
use rs_cam_core::session::{
    LoadedModel, ProjectSession, ProjectSessionBuilder, SelectionTarget, SessionError,
    SimulationOptions, SimulationResolution,
};

/// The BREP id of the Z 30 up face (X 0..20).
const FACE_Z30: u16 = 0;
/// The BREP id of the Z 20 up face (X 20..40).
const FACE_Z20: u16 = 1;
/// The stock reaches this far past the part in X and Y (mm).
const XY_MARGIN: f64 = 5.0;
/// The simulation cell (mm).
const SIM_CELL: f64 = 0.5;
/// The Pocket tool diameter (mm).
const POCKET_TOOL_DIAMETER: f64 = 6.0;

/// A flat rectangle X `x0..x1`, Y 0..40 at `z`, wound so that its
/// triangles face `up`.
fn rect_face(x0: f64, x1: f64, z: f64, up: bool) -> FaceTessellation {
    let v = vec![
        P3::new(x0, 0.0, z),
        P3::new(x1, 0.0, z),
        P3::new(x1, 40.0, z),
        P3::new(x0, 40.0, z),
    ];
    let triangles = if up {
        vec![[0, 1, 2], [0, 2, 3]]
    } else {
        vec![[0, 2, 1], [0, 3, 2]]
    };
    FaceTessellation {
        boundary_loops: vec![v.clone()],
        vertices: v,
        triangles,
        surface_type: SurfaceType::Plane,
        surface_params: SurfaceParams::Plane {
            normal: V3::new(0.0, 0.0, if up { 1.0 } else { -1.0 }),
            d: z,
        },
    }
}

/// The flat faces of the stepped block: three up steps and the down face.
fn stepped_block() -> LoadedModel {
    let enriched = build_enriched_mesh(
        vec![
            rect_face(0.0, 20.0, 30.0, true),
            rect_face(20.0, 40.0, 20.0, true),
            rect_face(40.0, 60.0, 10.0, true),
            rect_face(0.0, 60.0, 0.0, false),
        ],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(0),
        name: "ux1c_stepped_block".to_owned(),
        mesh: Some(Arc::clone(&enriched.mesh)),
        items: Arc::new(brep_face_items(&enriched)),
        path: PathBuf::from("synthetic://ux1c_stepped_block.step"),
        kind: None,
        units: None,
        enriched_mesh: Some(Arc::new(enriched)),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    }
}

/// A 60° V-bit, 12.7 mm (the shipped V-Carve sentry tool).
fn vbit() -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId::from_raw(0), ToolType::VBit);
    t.diameter = 12.7;
    t.cutting_length = 11.0;
    t.included_angle = 60.0;
    t.shank_diameter = 6.35;
    t.stickout = 30.0;
    t.flute_count = 2;
    t.name = "V-bit 60deg".to_owned();
    t
}

/// The four one-depth ops, each with an empty selection (the face pick
/// fills it), with the op label that the refusal text starts with.
fn one_depth_ops() -> Vec<(&'static str, OperationConfig)> {
    [
        OperationConfig::Chamfer(ChamferConfig {
            selection: Selection::empty(),
            ..ChamferConfig::default()
        }),
        OperationConfig::VCarve(VCarveConfig {
            selection: Selection::empty(),
            ..VCarveConfig::default()
        }),
        OperationConfig::Inlay(InlayConfig {
            selection: Selection::empty(),
            ..InlayConfig::default()
        }),
        OperationConfig::InlayPlug(InlayPlugConfig {
            selection: Selection::empty(),
            ..InlayPlugConfig::default()
        }),
    ]
    .into_iter()
    .map(|op| (op.op_type().label(), op))
    .collect()
}

/// A Pocket of the picked face to `depth` below it (the UX-1 clamp starts
/// it at the stock top).
fn pocket(depth: f64) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        floor: Floor::top(depth),
        selection: Selection::empty(),
        ..PocketConfig::default()
    })
}

/// The stock Z 0..`stock_top`, the part with a margin in X and Y.
fn stock(stock_top: f64) -> StockConfig {
    StockConfig {
        x: 60.0 + 2.0 * XY_MARGIN,
        y: 40.0 + 2.0 * XY_MARGIN,
        z: stock_top,
        origin_x: -XY_MARGIN,
        origin_y: -XY_MARGIN,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// The op `op` on `face`, on fresh stock Z 0..`stock_top`, with `top`.
fn fresh_session(
    op: &OperationConfig,
    face: u16,
    stock_top: f64,
    top: HeightMode,
) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock(stock_top));
    let tool_idx = builder.add_tool(vbit());
    let tool_id = builder.tools()[tool_idx].id;
    let model_id = builder.add_model(stepped_block());
    let mut cfg = toolpath_config("one depth", op.clone(), tool_id, model_id);
    cfg.heights.top_z = top;
    let _ = builder.add_toolpath(0, cfg).unwrap();
    let mut session = builder.build();
    pick_faces_in(&mut session, 0, SelectionTarget::Op, &[face]).unwrap();
    session
}

fn generate(session: &mut ProjectSession, index: usize) -> Result<(), SessionError> {
    let cancel = AtomicBool::new(false);
    session.generate_toolpath(index, &cancel).map(|_| ())
}

fn simulate(session: &mut ProjectSession) {
    let cancel = AtomicBool::new(false);
    let _ = session
        .run_simulation(
            &SimulationOptions {
                resolution: SimulationResolution::Fixed(SIM_CELL),
                adaptive_feed_modulation: false,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("the simulation completes");
}

/// The `precondition.buried_face` diagnostics of toolpath `index`.
fn buried_face_diagnostics(session: &ProjectSession, index: usize) -> Vec<String> {
    session
        .diagnose_toolpath_with_trace(index, None)
        .expect("the toolpath diagnoses")
        .into_iter()
        .filter(|d| d.id.as_str() == ids::PRECOND_BURIED_FACE)
        .map(|d| d.message)
        .collect()
}

/// The refusal of a buried face, or a panic that names what came back.
#[track_caller]
fn expect_buried(outcome: Result<(), SessionError>, name: &str, under: BuriedUnder) -> String {
    match outcome {
        Err(SessionError::Operation(OperationError::BuriedFace(refusal))) => {
            assert_eq!(refusal.under, under, "{name}: {refusal}");
            refusal.to_string()
        }
        Err(other) => panic!("{name}: want the buried-face refusal, got: {other}"),
        Ok(()) => panic!("{name}: generated; want the buried-face refusal"),
    }
}

/// Claim 1: fresh stock, the face 12 mm below the stock top, refuses with
/// the ruling text on generation and in the diagnostics.
#[test]
fn a_buried_face_refuses_on_fresh_stock_ux1c() {
    for (name, op) in one_depth_ops() {
        let mut session = fresh_session(&op, FACE_Z20, 32.0, HeightMode::Auto);
        let static_texts = buried_face_diagnostics(&session, 0);
        let text = expect_buried(generate(&mut session, 0), name, BuriedUnder::StockTop);
        let want = format!(
            "{name}: the face is 12.0 mm below the stock top and no earlier op clears it. \
             Clear it first (Pocket or Face), or start from \"After previous ops\"."
        );
        assert_eq!(text, want, "{name}: the refusal text");
        assert_eq!(
            static_texts,
            vec![format!("{want} — generation will fail.")],
            "{name}: the diagnostics state the generation text before generation"
        );
    }
}

/// Claim 2: a face AT the stock top generates.
#[test]
fn a_face_at_the_stock_top_generates_ux1c() {
    for (name, op) in one_depth_ops() {
        let mut session = fresh_session(&op, FACE_Z30, 30.0, HeightMode::Auto);
        assert!(
            buried_face_diagnostics(&session, 0).is_empty(),
            "{name}: a face at the stock top has no buried-face diagnostic"
        );
        generate(&mut session, 0).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

/// A pinned top is the operator's statement: no refusal.
#[test]
fn a_pinned_top_does_not_refuse_ux1c() {
    for (name, op) in one_depth_ops() {
        let mut session = fresh_session(&op, FACE_Z20, 32.0, HeightMode::Manual(20.0));
        assert!(
            buried_face_diagnostics(&session, 0).is_empty(),
            "{name}: a pinned top has no buried-face diagnostic"
        );
        generate(&mut session, 0).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

/// A Pocket of the Z 20 face to `pocket_depth` below it on fresh stock,
/// then `op` on the same face on "After previous ops". The Pocket is
/// generated and simulated; the op is not generated.
fn cascade(op: &OperationConfig, pocket_depth: f64) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock(32.0));
    let endmill = builder.add_tool(endmill_tool_config(POCKET_TOOL_DIAMETER));
    let endmill_id = builder.tools()[endmill].id;
    let vbit_idx = builder.add_tool(vbit());
    let vbit_id = builder.tools()[vbit_idx].id;
    let model_id = builder.add_model(stepped_block());
    let mut clear = toolpath_config("clear", pocket(pocket_depth), endmill_id, model_id);
    clear.heights.top_z = HeightMode::Auto;
    let _ = builder.add_toolpath(0, clear).unwrap();
    let mut rest = toolpath_config("one depth", op.clone(), vbit_id, model_id);
    rest.heights.top_z = HeightMode::Auto;
    rest.stock_source = StockSource::FromRemainingStock;
    let _ = builder.add_toolpath(0, rest).unwrap();
    let mut session = builder.build();
    pick_faces_in(&mut session, 0, SelectionTarget::Op, &[FACE_Z20]).unwrap();
    pick_faces_in(&mut session, 1, SelectionTarget::Op, &[FACE_Z20]).unwrap();
    generate(&mut session, 0).expect("the Pocket generates");
    simulate(&mut session);
    session
}

/// Claim 3: after a Pocket that clears the face, the op generates on
/// "After previous ops", and the diagnostics hold no buried-face row (the
/// static check is the fresh-stock rule).
#[test]
fn a_cleared_face_generates_on_the_remaining_stock_ux1c() {
    for (name, op) in one_depth_ops() {
        let mut session = cascade(&op, 0.5);
        assert!(
            buried_face_diagnostics(&session, 1).is_empty(),
            "{name}: the remaining-stock op has no static buried-face diagnostic"
        );
        generate(&mut session, 1).unwrap_or_else(|e| panic!("{name}: {e}"));
        let moves = session
            .result_at(1)
            .expect("the op holds a result")
            .toolpath()
            .moves
            .len();
        assert!(
            moves > 0,
            "{name}: the op on the cleared face emitted no move"
        );
    }
}

/// Claim 4: a Pocket that stops 6 mm above the face leaves material over
/// it, and the op on "After previous ops" refuses on the standing material.
#[test]
fn a_face_under_standing_material_refuses_on_the_remaining_stock_ux1c() {
    for (name, op) in one_depth_ops() {
        let mut session = cascade_above(&op);
        let text = expect_buried(
            generate(&mut session, 1),
            name,
            BuriedUnder::StandingMaterial,
        );
        assert!(
            text.starts_with(&format!("{name}: the face is ")),
            "{name}: {text}"
        );
        assert!(
            text.contains("mm below the material that stands over it"),
            "{name}: {text}"
        );
    }
}

/// A Pocket of the Z 20 face to an absolute floor at Z 26, 6 mm above
/// the face, on fresh stock; then `op` on the same face on "After previous
/// ops". The Pocket is generated and simulated; the op is not generated.
fn cascade_above(op: &OperationConfig) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock(32.0));
    let endmill = builder.add_tool(endmill_tool_config(POCKET_TOOL_DIAMETER));
    let endmill_id = builder.tools()[endmill].id;
    let vbit_idx = builder.add_tool(vbit());
    let vbit_id = builder.tools()[vbit_idx].id;
    let model_id = builder.add_model(stepped_block());
    let mut clear = toolpath_config(
        "clear above",
        OperationConfig::Pocket(PocketConfig {
            floor: Floor::Z { z: 26.0 },
            selection: Selection::empty(),
            ..PocketConfig::default()
        }),
        endmill_id,
        model_id,
    );
    clear.heights.top_z = HeightMode::Auto;
    let _ = builder.add_toolpath(0, clear).unwrap();
    let mut rest = toolpath_config("one depth", op.clone(), vbit_id, model_id);
    rest.heights.top_z = HeightMode::Auto;
    rest.stock_source = StockSource::FromRemainingStock;
    let _ = builder.add_toolpath(0, rest).unwrap();
    let mut session = builder.build();
    pick_faces_in(&mut session, 0, SelectionTarget::Op, &[FACE_Z20]).unwrap();
    pick_faces_in(&mut session, 1, SelectionTarget::Op, &[FACE_Z20]).unwrap();
    generate(&mut session, 0).expect("the Pocket generates");
    simulate(&mut session);
    session
}
