//! G-DRILLCENTROID (UX-R03-004) — a drill op with no drill targets refuses
//! instead of drilling the centroid of every closed polygon.
//!
//! ## The defect
//!
//! `drill_holes_for_config` resolved a `Drill` op with no explicit pick
//! (`selected_holes == None`) to the vertex centroid of every closed polygon
//! in the model. On `fixtures/demo_star.svg` — one closed polygon, no
//! circles, no points — that generated ONE hole through the middle of the
//! star at (50, 51.1), simulated clean, and exported `Within` on all three
//! drill gates. Nothing in the inspector said where the hole came from.
//!
//! ## The decision this sentry pins
//!
//! A hole position comes from a `Point` or a `Circle` item of the op's
//! selection (a DXF `POINT`, a circle / arc centre, or a circle-like closed
//! shape in an SVG, which the SVG door makes a `Circle` item) — never from
//! a polygon outline. With the default selection, the op drills every
//! point and circle of the model. With none, the prologue refuses with
//! `OperationError::Selection` (`Empty`, "nothing is left to cut", P2.3c)
//! and no toolpath is stored. The centroid fallback is removed entirely,
//! not merely gated: only points and circles serve a `Holes` op, and the
//! import door decides what is a circle. `fixtures/demo_pocket.svg` (a
//! rounded rect with a `<circle>`) still drills its circle, and only its
//! circle.
//!
//! ## What this measured pre-fix
//!
//! | case | expected | pre-fix |
//! |---|---|---|
//! | star SVG, default selection | refusal, no result stored | `Ok`, 1 hole at the centroid, 4 moves |
//! | pocket SVG (rect + circle), default selection | 1 hole at the circle centre (40, 30) | 2 holes: the circle AND the rect's centroid |
//! | one circle target, default selection | 1 hole at the circle centre | 1 hole at the polygon centroid |
//! | targets present, empty selection | refusal (unchanged) | refusal |

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;
use common::make_endmill_6mm;
use common::session::{TestHole, toolpath_config};

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::DressupEntryStyle;
use rs_cam_core::compute::operation_configs::{DrillConfig, DrillCycleType};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::ToolConfig;
use rs_cam_core::io::load_model_file;
use rs_cam_core::selection::{ItemKind, Selection, Shape};
use rs_cam_core::session::{LoadedModel, ProjectSession, ProjectSessionBuilder};

/// The one circle target the "drawing with a circle" case exposes, in
/// model coordinates. Nowhere near the star's centroid (50, 51.1).
const CIRCLE_CENTRE: [f64; 2] = [20.0, 30.0];

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is crates/rs_cam_core.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root is two levels above the crate manifest")
        .to_path_buf()
}

/// `fixtures/demo_star.svg` through the interactive import door: one closed
/// polygon, no point and no circle.
fn star_model() -> LoadedModel {
    let path = repo_root().join("fixtures/demo_star.svg");
    let model = load_model_file(
        &path,
        rs_cam_core::ids::ModelId::from_raw(0),
        ModelKind::Svg,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .expect("demo_star.svg imports");
    assert!(
        !model
            .items
            .iter()
            .any(|i| matches!(i.kind(), ItemKind::Point | ItemKind::Circle)),
        "fixture is vacuous: the star SVG holds a point or a circle"
    );
    assert!(
        model
            .outlines_pre_selection()
            .is_some_and(|p| !p.is_empty()),
        "fixture is vacuous: the star SVG has no closed polygon"
    );
    model
}

/// `fixtures/demo_pocket.svg`: a rounded rectangle with a `<circle>` at
/// (40, 30) r=10 inside it. usvg flattens the circle; `detect_containment`
/// files it as the rectangle's hole; the import door classifies the ring as
/// circle-like and makes ONE `Circle` item.
fn pocket_model() -> LoadedModel {
    let path = repo_root().join("fixtures/demo_pocket.svg");
    load_model_file(
        &path,
        rs_cam_core::ids::ModelId::from_raw(0),
        ModelKind::Svg,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .expect("demo_pocket.svg imports")
}

/// The same star polygon, plus ONE circle — what a DXF with an outline and
/// one `CIRCLE` entity imports to (P2.1: the circle is one item).
fn star_with_one_circle() -> LoadedModel {
    let mut model = star_model();
    let target = TestHole {
        x: CIRCLE_CENTRE[0],
        y: CIRCLE_CENTRE[1],
        layer: "holes".to_owned(),
        diameter: Some(6.0),
    };
    let outlines = model.outlines_pre_selection().unwrap_or_default();
    model.items = Arc::new(common::session::drawing_items(
        outlines.as_ref().clone(),
        &[target],
    ));
    model.kind = Some(ModelKind::Dxf);
    model
}

fn stock() -> StockConfig {
    StockConfig {
        x: 120.0,
        y: 120.0,
        z: 12.0,
        origin_x: -10.0,
        origin_y: -10.0,
        origin_z: -12.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn drill_op(selection: Selection) -> OperationConfig {
    OperationConfig::Drill(DrillConfig {
        depth: 5.0,
        cycle: DrillCycleType::Simple,
        selection,
        ..DrillConfig::default()
    })
}

/// One stock, one tool, one model, one drill op — through the production
/// `add_*` doors. Entry styling and the rapid reorder are pinned off so the
/// emitted columns are exactly the holes.
fn session_with(model: LoadedModel, op: OperationConfig) -> ProjectSession {
    session_with_tool(model, op, make_endmill_6mm())
}

/// [`session_with`] with `tool`.
fn session_with_tool(model: LoadedModel, op: OperationConfig, tool: ToolConfig) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock());
    let tool_idx = builder.add_tool(tool);
    let tool_id = builder.tools()[tool_idx].id.raw();
    let model_id = builder.add_model(model);
    let mut tc = toolpath_config(
        "Drill",
        op,
        rs_cam_core::ids::ToolId::from_raw(tool_id),
        model_id,
    );
    tc.dressups.entry_style = DressupEntryStyle::None;
    tc.dressups.optimize_rapid_order = false;
    let _ = builder.add_toolpath(0, tc).expect("add drill toolpath");
    builder.build()
}

/// The distinct XY columns one toolpath visits, to micron resolution.
fn drilled_columns(session: &mut ProjectSession, index: usize) -> Vec<(i64, i64)> {
    let cancel = AtomicBool::new(false);
    let result = session
        .generate_toolpath(index, &cancel)
        .expect("drill generation must succeed");
    let tp = &result.op_data.annotated().toolpath;
    assert!(
        !tp.moves.is_empty(),
        "fixture is vacuous: toolpath {index} emitted no moves"
    );
    let mut cols: Vec<(i64, i64)> = tp
        .moves
        .iter()
        .map(|m| {
            (
                (m.target.x * 1000.0).round() as i64,
                (m.target.y * 1000.0).round() as i64,
            )
        })
        .collect();
    cols.sort_unstable();
    cols.dedup();
    cols
}

#[test]
fn a_drawing_with_no_targets_refuses_instead_of_drilling_the_centroid() {
    let mut session = session_with(star_model(), drill_op(Selection::drawing()));
    let cancel = AtomicBool::new(false);
    let err = match session.generate_toolpath(0, &cancel) {
        Ok(result) => {
            let tp = &result.op_data.annotated().toolpath;
            panic!(
                "a Drill op on a drawing with no circles or points generated {} moves \
                 (columns {:?}) — pre-fix this was one hole at the star's polygon centroid",
                tp.moves.len(),
                tp.moves
                    .iter()
                    .map(|m| (m.target.x, m.target.y))
                    .take(3)
                    .collect::<Vec<_>>()
            );
        }
        Err(e) => e.to_string(),
    };
    assert!(
        err.contains("nothing is left to cut"),
        "the refusal must name the missing input; got: {err}"
    );
    assert!(
        session.result_at(0).is_none(),
        "a refused generation must store no toolpath"
    );
}

/// The documented SVG feature survives: a real `<circle>` in an SVG drills,
/// at its centre, and the rectangle around it is not a hole.
#[test]
fn an_svg_circle_drills_at_its_centre_and_the_outline_does_not() {
    let model = pocket_model();
    let circles = model
        .items
        .iter()
        .filter(|i| i.kind() == ItemKind::Circle)
        .count();
    assert_eq!(circles, 1, "the pocket SVG holds exactly its circle");
    // P6.5 (O-3): a Drill drills a circle of the tool size only, so the
    // tool is the size of the circle.
    let diameter = model
        .items
        .iter()
        .find_map(|i| match i.shape() {
            Shape::Circle { diameter, .. } => Some(*diameter),
            Shape::Loop(_) | Shape::Path(_) | Shape::Point(_) | Shape::Face(_) => None,
        })
        .expect("the circle");
    let tool = ToolConfig {
        diameter,
        ..make_endmill_6mm()
    };
    let mut session = session_with_tool(model, drill_op(Selection::drawing()), tool);
    let cols = drilled_columns(&mut session, 0);
    assert_eq!(cols.len(), 1, "one hole, got columns {cols:?}");
    let (x, y) = (cols[0].0 as f64 / 1000.0, cols[0].1 as f64 / 1000.0);
    assert!(
        (x - 40.0).abs() <= 0.1 && (y - 30.0).abs() <= 0.1,
        "the hole sits at the circle centre (40, 30) within 0.1 mm, got ({x}, {y})"
    );
    // The rounded rectangle's vertex centroid is near (40, 30) too only by
    // symmetry of THIS fixture along X; its Y centroid is 30 as well, so
    // pin the count (one hole) rather than the position for that half.
    assert!(
        !session
            .result_at(0)
            .unwrap()
            .op_data
            .annotated()
            .toolpath
            .moves
            .is_empty()
    );
}

#[test]
fn a_drawing_with_one_circle_drills_the_circle_centre_and_nothing_else() {
    let mut session = session_with(star_with_one_circle(), drill_op(Selection::drawing()));
    let cols = drilled_columns(&mut session, 0);
    let expected = vec![(
        (CIRCLE_CENTRE[0] * 1000.0).round() as i64,
        (CIRCLE_CENTRE[1] * 1000.0).round() as i64,
    )];
    assert_eq!(
        cols, expected,
        "with the default selection the op drills every point and circle of \
         the model — the one circle centre — and never the outline's centroid \
         (50, 51.1)"
    );
}

#[test]
fn an_empty_selection_still_refuses_when_circles_exist() {
    let mut session = session_with(star_with_one_circle(), drill_op(Selection::empty()));
    let cancel = AtomicBool::new(false);
    let err = session
        .generate_toolpath(0, &cancel)
        .err()
        .map(|e| e.to_string())
        .expect("an explicit empty selection refuses rather than drilling everything");
    assert!(
        err.contains("the selection holds no item to cut"),
        "got: {err}"
    );
}
