//! Geometry selection P3.1: one heights resolver for every surface.
//!
//! # The claim
//!
//! ONE session rule (`session/heights.rs`) gives the resolved heights to
//! generation, the GUI and MCP, in the setup frame. Generation calls it
//! with the inputs it holds; every other surface reads
//! `ProjectSession::toolpath_heights`. So each surface gives one Top Z and
//! one Bottom Z, and each is the ABSOLUTE value the fixture states (an
//! agreement on a wrong number is red):
//!
//! 1. A Pocket face pick on a Top setup: the top is the face Z (defect D-A:
//!    the GUI and MCP showed the stock top), the bottom is the face Z minus
//!    the depth.
//! 2. The same on a Bottom setup: the down face is up there, at the setup
//!    Z `stock top - model Z`.
//! 3. A `ModelTop` top on an STL, Top and Bottom setups: the top is the mesh
//!    top in the setup frame plus the offset. The MCP diagnostics path
//!    (`toolpath_heights`) gives the same numbers here.
//! 4. L-4 and D-B: a drawing has no mesh, so its context holds no model Z.
//!    A `ModelTop` top on it refuses generation, and the session answer
//!    names the same refusal for the Heights tab.
//! 5. No viz production source builds a height context or resolves heights
//!    by itself (a source scan; `HeightsConfig::resolve` is `pub(crate)`,
//!    and the scan covers the one open door, `ToolpathHeights::from_context`).
//!
//! The MCP arm of the face pick reads the MCP diagnostics themselves: a
//! Pocket whose floor is below the stock bottom only when it cuts from the
//! face gives `geom.depth_beyond_stock` only when the MCP path applies the
//! face top (the P3.1 swap of `session/compute/export.rs`, landed in P3.2).
//!
//! ```text
//! cargo test -p rs_cam_core -q --test one_heights_resolver_for_every_surface_p3_1
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

use common::session::{mesh_model, pick_faces_in, polygon_model, toolpath_config};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, HeightMode, HeightReference, ReferenceOffset};
use rs_cam_core::compute::operation_configs::{DropCutterConfig, PocketConfig};
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::diagnostics::adapters::from_static_checks::ResolvedHeights as McpHeights;
use rs_cam_core::diagnostics::ids;
use rs_cam_core::geo::{P3, V3};
use rs_cam_core::geometry::enriched_mesh::{
    FaceTessellation, SurfaceParams, SurfaceType, build_enriched_mesh,
};
use rs_cam_core::io::brep_face_items;
use rs_cam_core::mesh::make_test_hemisphere;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    Command, GenerateToolpathArgs, Job, JobHandle, LoadedModel, ProjectSession,
    ProjectSessionBuilder, SelectionTarget, SessionError, SetSetupFaceArgs, TopSource,
};

/// The model Z of the up face (face 0).
const UP_FACE_Z: f64 = 0.0;
/// The model Z of the down face (face 1).
const DOWN_FACE_Z: f64 = -18.0;
/// The BREP id of the up face.
const UP_FACE: u16 = 0;
/// The BREP id of the down face.
const DOWN_FACE: u16 = 1;
/// The side of the square part (mm).
const PART_SIDE: f64 = 10.0;
/// The stock reaches this far past the part on every side and in Z (mm).
const STOCK_MARGIN: f64 = 5.0;
/// The pocket depth (mm). One pass.
const POCKET_DEPTH: f64 = 2.0;
/// The tool diameter (mm): it fits the 10 mm face.
const TOOL_DIAMETER: f64 = 3.0;
/// The hemisphere radius of the STL arm (mm): the mesh spans Z 0..R.
const DOME_RADIUS: f64 = 8.0;
/// The offset of the `ModelTop` top of the STL arm (mm).
const MODEL_TOP_OFFSET: f64 = -1.0;
/// The float noise of a resolved Z (mm).
const EPS: f64 = 1e-9;

/// A flat square face at `z`, wound so that its triangles face `up`.
fn square_face(z: f64, up: bool) -> FaceTessellation {
    let v = vec![
        P3::new(0.0, 0.0, z),
        P3::new(PART_SIDE, 0.0, z),
        P3::new(PART_SIDE, PART_SIDE, z),
        P3::new(0.0, PART_SIDE, z),
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

/// The part: face 0 up at [`UP_FACE_Z`], face 1 down at [`DOWN_FACE_Z`].
/// Its faces are items at import, as a STEP part's are.
fn part_model() -> LoadedModel {
    let enriched = build_enriched_mesh(
        vec![
            square_face(UP_FACE_Z, true),
            square_face(DOWN_FACE_Z, false),
        ],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(0),
        name: "p3_1_part".to_owned(),
        mesh: Some(Arc::clone(&enriched.mesh)),
        items: Arc::new(brep_face_items(&enriched)),
        path: PathBuf::from("synthetic://p3_1_part.step"),
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

/// The stock: [`STOCK_MARGIN`] past the part on every side and in Z. It
/// also holds the dome of the STL arm.
fn stock() -> StockConfig {
    StockConfig {
        x: PART_SIDE + 2.0 * STOCK_MARGIN + 2.0 * DOME_RADIUS,
        y: PART_SIDE + 2.0 * STOCK_MARGIN + 2.0 * DOME_RADIUS,
        z: UP_FACE_Z - DOWN_FACE_Z + 2.0 * STOCK_MARGIN,
        origin_x: -STOCK_MARGIN - DOME_RADIUS,
        origin_y: -STOCK_MARGIN - DOME_RADIUS,
        origin_z: DOWN_FACE_Z - STOCK_MARGIN,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// The stock top in the world frame (= the Top setup frame).
fn world_stock_top() -> f64 {
    let s = stock();
    s.origin_z + s.z
}

/// The stock bottom in the world frame.
fn world_stock_bottom() -> f64 {
    stock().origin_z
}

/// One op on `model`, on a setup with `face_up`. `top` is the stored Top Z.
fn session_with(
    model: LoadedModel,
    op: OperationConfig,
    top: HeightMode,
    face_up: FaceUp,
) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock());
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_DIAMETER));
    let tool_id = builder.tools()[tool_idx].id.raw();
    let model_id = builder.add_model(model);
    let mut cfg = toolpath_config(
        "p3_1 op",
        op,
        rs_cam_core::ids::ToolId::from_raw(tool_id),
        model_id,
    );
    cfg.boundary = BoundaryConfig::default();
    cfg.heights.top_z = top;
    let _ = builder.add_toolpath(0, cfg).unwrap();
    let mut session = builder.build();
    let _ = session
        .apply(Command::SetSetupFace(SetSetupFaceArgs {
            setup_index: 0,
            face_up,
        }))
        .unwrap();
    session
}

fn pocket(depth: f64) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        floor: rs_cam_core::compute::floor::Floor::top(depth),
        depth_per_pass: depth,
        ..PocketConfig::default()
    })
}

/// A Pocket that picks `faces` with an auto top, on a setup with `face_up`.
fn pocket_session(faces: &[u16], depth: f64, face_up: FaceUp) -> ProjectSession {
    let mut s = session_with(part_model(), pocket(depth), HeightMode::Auto, face_up);
    pick_faces_in(&mut s, 0, SelectionTarget::Op, faces).unwrap();
    s
}

/// A Drop Cutter on a dome STL whose Top Z reads the Model Top.
fn model_top_session(face_up: FaceUp) -> ProjectSession {
    let op = OperationConfig::DropCutter(DropCutterConfig {
        stepover: 1.0,
        ..DropCutterConfig::default()
    });
    let top = HeightMode::FromReference(ReferenceOffset {
        reference: HeightReference::ModelTop,
        offset: MODEL_TOP_OFFSET,
    });
    session_with(
        mesh_model(make_test_hemisphere(DOME_RADIUS, 16), "p3_1_dome"),
        op,
        top,
        face_up,
    )
}

/// The (top, bottom) that generation hands the op: the request snapshot of
/// the generation job.
fn generation(session: &mut ProjectSession) -> (f64, f64) {
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
    let read = |key: &str| snapshot["heights"][key].as_f64().expect("a height");
    (read("top_z"), read("bottom_z"))
}

/// The (top, bottom) of the session resolver: the GUI reads it (the Heights
/// tab, the diagram, the toolpath rows).
fn session_heights(session: &ProjectSession) -> (f64, f64) {
    let tc = &session.toolpath_configs()[0];
    let answer = session.toolpath_heights(tc).expect("the heights resolve");
    (answer.heights.top_z, answer.heights.bottom_z)
}

/// The (top, bottom) of the path that the MCP diagnostics read
/// (`diagnose_toolpath_with_trace`: `toolpath_heights`, then
/// `ResolvedHeights::from_toolpath_heights`).
fn mcp_today(session: &ProjectSession) -> (f64, f64) {
    let tc = &session.toolpath_configs()[0];
    let answer = session.toolpath_heights(tc).expect("the heights resolve");
    let h = McpHeights::from_toolpath_heights(&answer, &tc.heights);
    (h.top_z, h.bottom_z)
}

#[track_caller]
fn assert_pair(surface: &str, got: (f64, f64), want: (f64, f64)) {
    assert!(
        (got.0 - want.0).abs() < EPS && (got.1 - want.1).abs() < EPS,
        "{surface}: (top, bottom) = {got:?}, want {want:?}"
    );
}

#[test]
fn a_face_pick_gives_one_top_and_one_bottom_on_a_top_setup() {
    let mut s = pocket_session(&[UP_FACE], POCKET_DEPTH, FaceUp::Top);
    let want = (UP_FACE_Z, UP_FACE_Z - POCKET_DEPTH);
    assert_pair("generation", generation(&mut s), want);
    assert_pair("toolpath_heights", session_heights(&s), want);
    let tc = &s.toolpath_configs()[0];
    let answer = s.toolpath_heights(tc).unwrap();
    assert_eq!(answer.top_from, TopSource::AutoFaces);
    assert_eq!(answer.face_top_z(), Some(UP_FACE_Z));
    assert!(
        UP_FACE_Z < world_stock_top() - 1.0,
        "the fixture must hold the face below the stock top, or the arm is vacuous"
    );
}

#[test]
fn a_face_pick_gives_one_top_and_one_bottom_on_a_bottom_setup() {
    let mut s = pocket_session(&[DOWN_FACE], POCKET_DEPTH, FaceUp::Bottom);
    // The setup frame is zero-rooted at the stock bottom as the setup sees
    // it, so a world Z maps to `stock top - Z` (the P2.3 sentry).
    let face_top = world_stock_top() - DOWN_FACE_Z;
    let want = (face_top, face_top - POCKET_DEPTH);
    assert_pair("generation", generation(&mut s), want);
    assert_pair("toolpath_heights", session_heights(&s), want);
}

#[test]
fn a_model_top_on_an_stl_gives_one_top_and_one_bottom() {
    // Top setup: the dome spans Z 0..R in the world frame.
    let mut s = model_top_session(FaceUp::Top);
    // The Drop Cutter depth dial: the auto bottom is the top minus it.
    let depth = s.toolpath_configs()[0]
        .operation
        .default_depth_for_heights();
    assert!(
        depth > 0.0,
        "the arm reads the depth arm of the auto bottom"
    );
    let top = DOME_RADIUS + MODEL_TOP_OFFSET;
    let want = (top, top - depth);
    assert_pair("generation", generation(&mut s), want);
    assert_pair("toolpath_heights", session_heights(&s), want);
    assert_pair("MCP diagnostics (today)", mcp_today(&s), want);

    // Bottom setup: the dome hangs from Z (stock top) down to (stock top -
    // R) in the zero-rooted setup frame, so its top is the stock top.
    let mut s = model_top_session(FaceUp::Bottom);
    let top_in_setup = world_stock_top();
    let top = top_in_setup + MODEL_TOP_OFFSET;
    let want = (top, top - depth);
    assert_pair("generation", generation(&mut s), want);
    assert_pair("toolpath_heights", session_heights(&s), want);
    assert_pair("MCP diagnostics (today)", mcp_today(&s), want);
}

#[test]
fn a_model_top_on_a_drawing_refuses_l4() {
    let drawing = polygon_model(
        vec![Polygon2::rectangle(0.0, 0.0, PART_SIDE, PART_SIDE)],
        "p3_1_drawing",
    );
    let top = HeightMode::FromReference(ReferenceOffset {
        reference: HeightReference::ModelTop,
        offset: 0.0,
    });
    let mut s = session_with(drawing.clone(), pocket(POCKET_DEPTH), top, FaceUp::Top);
    let tc = s.toolpath_configs()[0].clone();
    let answer = s.toolpath_heights(&tc).expect("the heights resolve");
    // D-B: a drawing has no Z extent, so the context holds no model Z.
    assert_eq!(answer.context.model_top_z, None);
    assert_eq!(answer.context.model_bottom_z, None);
    let shown = answer
        .model_reference_refusal(&tc.heights)
        .expect("the session names the L-4 refusal");
    let cancel = AtomicBool::new(false);
    let Err(SessionError::MissingGeometry(text)) = s.generate_toolpath(0, &cancel) else {
        panic!("generation must refuse a Model Top on a drawing (L-4)");
    };
    assert!(
        text.contains(&shown),
        "generation says {text:?}; the tab says {shown:?}"
    );
    assert!(
        text.contains("Model Top") && text.contains("Top Z"),
        "{text}"
    );

    // The same drawing with an auto top generates: the refusal names only
    // the model reference.
    let mut s = session_with(drawing, pocket(POCKET_DEPTH), HeightMode::Auto, FaceUp::Top);
    let tc = s.toolpath_configs()[0].clone();
    assert_eq!(
        s.toolpath_heights(&tc)
            .unwrap()
            .model_reference_refusal(&tc.heights),
        None
    );
    let _ = s
        .generate_toolpath(0, &cancel)
        .expect("an auto top on a drawing generates");
}

/// The MCP diagnostics apply the face top. A Pocket from the up face at Z 0
/// with a depth past the stock bottom cautions `geom.depth_beyond_stock`;
/// from the stock top it would not.
#[test]
fn the_mcp_diagnostics_read_the_face_top() {
    // Floor from the face: 0 - depth, below the stock bottom. Floor from
    // the stock top: (stock top) - depth, above it.
    let depth = UP_FACE_Z - world_stock_bottom() + 1.0;
    assert!(world_stock_top() - depth > world_stock_bottom());
    let s = pocket_session(&[UP_FACE], depth, FaceUp::Top);
    let ids: Vec<String> = s
        .diagnose_toolpath(0)
        .expect("the diagnostics answer")
        .into_iter()
        .map(|d| d.id.as_str().to_owned())
        .collect();
    assert!(
        ids.iter().any(|id| id == ids::GEOM_DEPTH_BEYOND_STOCK),
        "the MCP diagnostics resolve without the face top: {ids:?}"
    );
}

/// The production half of a source file: the lines before its inline test
/// module (`#[cfg(test)]` and, within a few attribute lines, `mod x {`),
/// with no comment. A `#[cfg(test)] mod tests;` declaration does not end it.
fn production_code(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let end = (0..lines.len())
        .find(|&at| {
            lines[at].trim() == "#[cfg(test)]"
                && lines[at + 1..lines.len().min(at + 8)].iter().any(|l| {
                    let l = l.trim();
                    l.starts_with("mod ") && l.ends_with('{')
                })
        })
        .unwrap_or(lines.len());
    lines[..end]
        .iter()
        .map(|line| line.split("//").next().unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The viz production source builds no height context and resolves no
/// heights by itself: it reads `toolpath_heights`. `HeightsConfig::resolve`
/// is `pub(crate)` (the compiler holds that door); this scan holds the
/// others.
#[test]
fn no_viz_surface_resolves_heights_by_itself() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rs_cam_viz/src");
    let mut files = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
    }
    assert!(files.len() > 50, "the scan found {} files", files.len());
    let mut found = Vec::new();
    let mut readers = 0;
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if name.ends_with("tests.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let production = production_code(&text);
        readers += production.matches(".toolpath_heights(").count();
        for needle in [
            "ToolpathHeights::from_context(",
            ".height_context(",
            "height_context_for_toolpath(",
            "HeightContext {",
            "HeightContext::simple(",
        ] {
            if production.contains(needle) {
                found.push(format!("{}: {needle}", path.display()));
            }
        }
    }
    // Non-vacuity: the viz surfaces DO read the session resolver (the
    // panel snapshot, the viewport planes and the entry preview, the
    // drawing plane).
    assert!(
        readers >= 4,
        "the scan finds {readers} calls of `.toolpath_heights(` in viz production code"
    );
    assert!(
        found.is_empty(),
        "a viz surface resolves heights by itself:\n{}",
        found.join("\n")
    );
}
