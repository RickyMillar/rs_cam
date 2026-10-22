//! UX-1 (SAFETY, operator ruling Q1, 2026-10-22): an auto top under a face
//! pick never starts below the stock top.
//!
//! # The defect
//!
//! Measured in the GUI on `fixtures/gui_step/stepped_block.step` (block
//! 60 x 40, Z 0..30, three steps): a Profile with the two lower up faces
//! picked (Z 20 and Z 10), floor `Top Z` depth 12, depth per pass 2, stock
//! Z 0..32, start from fresh stock. The heights read "auto: faces", top 20,
//! bottom 8. The first level was Z 18, so the tool went 14 mm into the
//! material in one pass (narrate: peak axial DOC 14.00 mm against 2.00 per
//! pass).
//!
//! # The claim
//!
//! The ruling: the auto top under a face pick is the higher of the face top
//! and the stock top. The depth still measures from the face top, as under
//! L-15, so the floor does not move. For each arm:
//!
//! 1. The face top is below the stock top (non-vacuity: the old rule then
//!    starts below the stock top).
//! 2. The resolved top (`toolpath_heights`, the read of every surface) is
//!    the stock top in the setup frame, and generation hands the op the
//!    same top (the request snapshot).
//! 3. The floor is the face top less the depth (it does not move).
//! 4. The first cutting level is at most one depth per pass below the stock
//!    top.
//! 5. On fresh stock, the simulated peak axial DOC of the toolpath is at
//!    most the depth per pass plus [`DOC_SLACK`].
//!
//! Arms: a Top setup (the GUI case) and a Bottom (flipped) setup, where the
//! down face of the model is up and its setup-frame Z is below the
//! setup-frame stock top.
//!
//! UX-1b (the same ruling) adds Face and Trace, each on both setups. Their
//! depth dial (`depth`, `depth_per_pass`) steps from the top to the depth
//! below `ResolvedHeights::depth_base_z`, which is the face top here. For
//! them, claim 3 also holds the last level at the face top less the depth.
//! Measured before UX-1b (0.5 mm sim, fresh stock): Face Top setup top 20,
//! levels [19], peak axial DOC 0.73 mm (the stock-aware entry ramp descends
//! each row from the stock top); Face Bottom setup top 32, levels [31],
//! peak 2.83 mm against 1 mm per pass; Trace Top setup top 20, first level
//! 19.5, peak 12.5 mm against 0.5; Trace Bottom setup top 32, first level
//! 31.5, peak 6.5 mm. After: Face first level 31 and 37, peak 0.73 and
//! 1.0 mm; Trace first level 31.5 and 37.5, peak 0.5 and 0.5 mm.
//!
//! The other ops that take a face pick keep the face top
//! (`FaceTopRule::Face`); `compute::catalog::capabilities` states why, and
//! `every_op_keeps_its_face_top_rule_ux1b` freezes the column.
//!
//! Not in this file: an op on "After previous ops". The ruling says its
//! remaining-stock clip skips the air levels; a cheap cascade fixture on a
//! face pick does not exist yet (follow-up).
//!
//! # The fixture
//!
//! The `step` feature is off in a plain core test build, so the part is
//! built in memory (`build_enriched_mesh` and `io::brep_face_items`), as the
//! P3.4 and P4.1 sentries build theirs. It has the up faces of the stepped
//! block (Z 30 on X 0..20, Z 20 on X 20..40, Z 10 on X 40..60, Y 0..40)
//! and the down face at Z 0.
//!
//! Red proof: the old rule (`heights.top_z = face_top`) in
//! `session::heights::apply_rule`. Both arms failed at claim 2. The measured
//! values also break claims 4 and 5: Top arm top 20, first level 18, peak
//! axial DOC 14.0 mm (the GUI measure); Bottom arm top 32, first level 30,
//! peak axial DOC 8.0 mm. UX-1b: the rows of Face and Trace set back to
//! `FaceTopRule::Face`: the four Face and Trace arms failed at claim 2.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_face_top_never_starts_below_the_stock_top_ux1
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
use rs_cam_core::compute::config::HeightMode;
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::operation_configs::{FaceConfig, ProfileConfig, TraceConfig};
use rs_cam_core::compute::tool_config::ToolConfig;
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::geo::{P3, V3};
use rs_cam_core::geometry::enriched_mesh::{
    FaceTessellation, SurfaceParams, SurfaceType, build_enriched_mesh,
};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::io::brep_face_items;
use rs_cam_core::selection::Selection;
use rs_cam_core::session::{
    Command, GenerateToolpathArgs, Job, JobHandle, LoadedModel, ProjectSession,
    ProjectSessionBuilder, SelectionTarget, SetSetupFaceArgs, SimulationOptions,
    SimulationResolution,
};

/// The BREP id of the Z 20 up face (X 20..40).
const FACE_Z20: u16 = 1;
/// The BREP id of the Z 10 up face (X 40..60).
const FACE_Z10: u16 = 2;
/// The BREP id of the down face at Z 0 (up on a Bottom setup).
const FACE_BOTTOM: u16 = 3;
/// The depth per pass of the Profile (mm), as in the GUI case.
const DEPTH_PER_PASS: f64 = 2.0;
/// The depth and the depth per pass of the Face (mm).
const FACE_DEPTH: f64 = 1.0;
const FACE_DEPTH_PER_PASS: f64 = 1.0;
/// The depth and the depth per pass of the Trace (mm): the defaults of
/// `TraceConfig`.
const TRACE_DEPTH: f64 = 1.0;
const TRACE_DEPTH_PER_PASS: f64 = 0.5;
/// The tool diameter (mm).
const TOOL_DIAMETER: f64 = 6.0;
/// The Trace tool diameter (mm).
const TRACE_TOOL_DIAMETER: f64 = 3.0;
/// The stock reaches this far past the part in X and Y (mm), as in the GUI
/// case (stock X -5..65, Y -5..45).
const XY_MARGIN: f64 = 5.0;
/// The simulation cell (mm).
const SIM_CELL: f64 = 0.5;
/// The slack on the simulated axial DOC (mm): the 0.1 mm grid margin that
/// the F-028 sentry (`face_stock_top_frame_f028`) states for a 0.5 mm cell.
const DOC_SLACK: f64 = 0.1;
/// The float noise of a Z (mm).
const EPS: f64 = 1e-6;

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
        name: "ux1_stepped_block".to_owned(),
        mesh: Some(Arc::clone(&enriched.mesh)),
        items: Arc::new(brep_face_items(&enriched)),
        path: PathBuf::from("synthetic://ux1_stepped_block.step"),
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

/// One arm: the op, its tool, its depth below the face and its step down.
struct Arm {
    name: &'static str,
    op: OperationConfig,
    tool: ToolConfig,
    /// The depth below the face top (mm): the floor is the face top less
    /// this.
    depth: f64,
    /// The depth per pass of the op (mm).
    step: f64,
}

/// A Profile with the floor `Top Z` `depth`.
fn profile_arm(name: &'static str, depth: f64) -> Arm {
    Arm {
        name,
        op: OperationConfig::Profile(ProfileConfig {
            floor: Floor::top(depth),
            depth_per_pass: DEPTH_PER_PASS,
            selection: Selection::empty(),
            ..ProfileConfig::default()
        }),
        tool: endmill_tool_config(TOOL_DIAMETER),
        depth,
        step: DEPTH_PER_PASS,
    }
}

/// A Face of the picked faces (UX-1b).
fn face_arm(name: &'static str) -> Arm {
    Arm {
        name,
        op: OperationConfig::Face(FaceConfig {
            depth: FACE_DEPTH,
            depth_per_pass: FACE_DEPTH_PER_PASS,
            selection: Selection::empty(),
            ..FaceConfig::default()
        }),
        tool: endmill_tool_config(TOOL_DIAMETER),
        depth: FACE_DEPTH,
        step: FACE_DEPTH_PER_PASS,
    }
}

/// A Trace of the outlines of the picked faces (UX-1b).
fn trace_arm(name: &'static str) -> Arm {
    Arm {
        name,
        op: OperationConfig::Trace(TraceConfig {
            depth: TRACE_DEPTH,
            depth_per_pass: TRACE_DEPTH_PER_PASS,
            selection: Selection::empty(),
            ..TraceConfig::default()
        }),
        tool: endmill_tool_config(TRACE_TOOL_DIAMETER),
        depth: TRACE_DEPTH,
        step: TRACE_DEPTH_PER_PASS,
    }
}

/// The op of `arm` on `faces`, on a setup with `face_up`, over the stock
/// Z `stock_bottom..32`.
fn op_session(arm: &Arm, faces: &[u16], face_up: FaceUp, stock_bottom: f64) -> ProjectSession {
    let stock = StockConfig {
        x: 60.0 + 2.0 * XY_MARGIN,
        y: 40.0 + 2.0 * XY_MARGIN,
        z: 32.0 - stock_bottom,
        origin_x: -XY_MARGIN,
        origin_y: -XY_MARGIN,
        origin_z: stock_bottom,
        auto_from_model: false,
        ..StockConfig::default()
    };
    let mut builder = ProjectSessionBuilder::new().stock(stock);
    let tool_idx = builder.add_tool(arm.tool.clone());
    let tool_id = builder.tools()[tool_idx].id.raw();
    let model_id = builder.add_model(stepped_block());
    let mut cfg = toolpath_config(
        arm.name,
        arm.op.clone(),
        rs_cam_core::ids::ToolId::from_raw(tool_id),
        model_id,
    );
    cfg.heights.top_z = HeightMode::Auto;
    let _ = builder.add_toolpath(0, cfg).unwrap();
    let mut session = builder.build();
    let _ = session
        .apply(Command::SetSetupFace(SetSetupFaceArgs {
            setup_index: 0,
            face_up,
        }))
        .unwrap();
    pick_faces_in(&mut session, 0, SelectionTarget::Op, faces).unwrap();
    session
}

/// The top Z and the depth base that generation hands the op (the request
/// snapshot).
fn generated_top_z(session: &mut ProjectSession) -> (f64, f64) {
    let cancel = AtomicBool::new(false);
    let JobHandle::GenerateToolpath(handle) = session
        .start(
            Job::GenerateToolpath(GenerateToolpathArgs { index: 0 }),
            &cancel,
        )
        .expect("step (i) captures the generation inputs")
    else {
        panic!("the generate_toolpath row answers its own handle variant");
    };
    let snapshot = handle.request_snapshot();
    (
        snapshot["heights"]["top_z"]
            .as_f64()
            .expect("the snapshot holds the top"),
        snapshot["heights"]["depth_base_z"]
            .as_f64()
            .expect("the snapshot holds the depth base"),
    )
}

/// The simulated peak axial DOC of toolpath 0 on fresh stock (mm).
fn peak_axial_doc(session: &mut ProjectSession) -> f64 {
    let cancel = AtomicBool::new(false);
    let _ = session
        .generate_toolpath(0, &cancel)
        .expect("the face pick generates");
    let opts = SimulationOptions {
        resolution: SimulationResolution::Fixed(SIM_CELL),
        skip_ids: Vec::new(),
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy:
            rs_cam_core::dressup::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_feed_scale: 1.0,
    };
    session
        .run_simulation(&opts, &cancel)
        .expect("the simulation completes");
    let sim = session.simulation_result().expect("a simulation result");
    let trace = sim.cut_trace.as_ref().expect("a metric cut trace");
    let summary = trace
        .toolpath_summaries
        .iter()
        .find(|s| s.toolpath_id == ToolpathId::from_raw(0))
        .expect("a summary for the op");
    summary.peak_axial_doc_mm
}

/// Claims 1 to 5 on one arm.
#[track_caller]
fn assert_arm(mut session: ProjectSession, arm: &Arm) {
    let name = arm.name;
    let step = arm.step;
    let depth = arm.depth;
    let tc = session.toolpath_configs()[0].clone();
    let answer = session.toolpath_heights(&tc).expect("the heights resolve");
    let stock_top = answer.context.stock_top_z;
    let face_top = answer.face_top_z().expect("the picked faces give a top");

    // 1. Non-vacuity: the face top is below the stock top.
    assert!(
        face_top < stock_top - step,
        "{name}: the face top {face_top} is not below the stock top {stock_top}; \
         the arm proves nothing"
    );

    // 2. The top is the stock top, on every surface.
    assert!(
        (answer.heights.top_z - stock_top).abs() < EPS,
        "{name}: the auto top is {}, want the stock top {stock_top} (the face top is \
         {face_top})",
        answer.heights.top_z
    );
    let (generated, generated_base) = generated_top_z(&mut session);
    assert!(
        (generated - stock_top).abs() < EPS,
        "{name}: generation hands the op the top {generated}, want {stock_top}"
    );

    // 3. The floor measures from the face top.
    assert!(
        (answer.heights.depth_base_z - face_top).abs() < EPS
            && (generated_base - face_top).abs() < EPS,
        "{name}: the depth base is {} (generation {generated_base}), want the face top \
         {face_top}",
        answer.heights.depth_base_z
    );
    assert!(
        (answer.heights.bottom_z - (face_top - depth)).abs() < EPS,
        "{name}: the floor is {}, want the face top {face_top} less {depth}",
        answer.heights.bottom_z
    );
    let levels = tc.operation.cutting_levels(&answer.heights);
    let last = *levels.last().expect("the op has levels");
    assert!(
        (last - (face_top - depth)).abs() < EPS,
        "{name}: the last level is Z {last}, want the face top {face_top} less {depth}; \
         levels {levels:?}"
    );

    // 4. The first level is at most one pass below the stock top.
    let first = levels[0];
    assert!(
        stock_top - first <= step + EPS,
        "{name}: the first level is Z {first}, {} mm below the stock top {stock_top} \
         (depth per pass {step}); levels {levels:?}",
        stock_top - first
    );

    // 5. The simulated peak axial DOC on fresh stock.
    let doc = peak_axial_doc(&mut session);
    assert!(
        doc > 0.0,
        "{name}: the op removed no material; the DOC bound proves nothing"
    );
    assert!(
        doc <= step + DOC_SLACK,
        "{name}: the peak axial DOC is {doc:.2} mm, want at most {step} + {DOC_SLACK} mm"
    );
}

/// The GUI case: Top setup, the Z 20 and Z 10 faces, floor `Top Z` 12,
/// stock Z 0..32.
#[test]
fn a_face_top_never_starts_below_the_stock_top_ux1() {
    let arm = profile_arm("Profile, Top setup", 12.0);
    assert_arm(
        op_session(&arm, &[FACE_Z20, FACE_Z10], FaceUp::Top, 0.0),
        &arm,
    );
}

/// The flipped arm: a Bottom setup picks the down face of the model, which
/// is up there. The stock reaches 6 mm below the part, so the setup-frame
/// face top (32) is below the setup-frame stock top (38).
#[test]
fn a_face_top_never_starts_below_the_stock_top_on_a_bottom_setup_ux1() {
    let arm = profile_arm("Profile, Bottom setup", 4.0);
    assert_arm(op_session(&arm, &[FACE_BOTTOM], FaceUp::Bottom, -6.0), &arm);
}

/// UX-1b: a Face of the Z 20 and Z 10 faces on a Top setup.
#[test]
fn a_face_op_starts_at_the_stock_top_ux1b() {
    let arm = face_arm("Face, Top setup");
    assert_arm(
        op_session(&arm, &[FACE_Z20, FACE_Z10], FaceUp::Top, 0.0),
        &arm,
    );
}

/// UX-1b: a Face of the down face on a Bottom setup.
#[test]
fn a_face_op_starts_at_the_stock_top_on_a_bottom_setup_ux1b() {
    let arm = face_arm("Face, Bottom setup");
    assert_arm(op_session(&arm, &[FACE_BOTTOM], FaceUp::Bottom, -6.0), &arm);
}

/// UX-1b: a Trace of the outlines of the Z 20 and Z 10 faces on a Top
/// setup.
#[test]
fn a_trace_op_starts_at_the_stock_top_ux1b() {
    let arm = trace_arm("Trace, Top setup");
    assert_arm(
        op_session(&arm, &[FACE_Z20, FACE_Z10], FaceUp::Top, 0.0),
        &arm,
    );
}

/// UX-1b: a Trace of the outline of the down face on a Bottom setup.
#[test]
fn a_trace_op_starts_at_the_stock_top_on_a_bottom_setup_ux1b() {
    let arm = trace_arm("Trace, Bottom setup");
    assert_arm(op_session(&arm, &[FACE_BOTTOM], FaceUp::Bottom, -6.0), &arm);
}
