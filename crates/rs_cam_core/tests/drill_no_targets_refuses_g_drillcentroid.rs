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
//! A hole position comes from a `DrillTarget` (a DXF `POINT` or a circle /
//! arc centre, or a circle-like closed shape in an SVG — see
//! `svg_input::circle_like_drill_targets`) or from an explicit pick — never
//! from a polygon outline. With no pick, the op drills every target the
//! model exposes. With no pick and no targets, the generator refuses with
//! `MissingGeometry("No drill targets — …")` and no toolpath is stored.
//! The centroid fallback is removed entirely, not merely gated: the
//! generator reads targets only, and the import door decides what is a
//! circle. `fixtures/demo_pocket.svg` (a rounded rect with a `<circle>`)
//! still drills its circle, and only its circle.
//!
//! ## What this measured pre-fix
//!
//! | case | expected | pre-fix |
//! |---|---|---|
//! | star SVG, default selection | refusal, no result stored | `Ok`, 1 hole at the centroid, 4 moves |
//! | pocket SVG (rect + circle), default selection | 1 hole at the circle centre (40, 30) | 2 holes: the circle AND the rect's centroid |
//! | one circle target, default selection | 1 hole at the circle centre | 1 hole at the polygon centroid |
//! | targets present, empty pick | refusal (unchanged) | refusal |

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::make_endmill_6mm;
use common::session::toolpath_config;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::DressupEntryStyle;
use rs_cam_core::compute::operation_configs::{DrillConfig, DrillCycleType};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::dxf_input::{DrillTarget, DrillTargetKind};
use rs_cam_core::io::load_model_file;
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
/// polygon, zero drill targets — an SVG never exposes any.
fn star_model() -> LoadedModel {
    let path = repo_root().join("fixtures/demo_star.svg");
    let model = load_model_file(&path, 0, ModelKind::Svg, ModelUnits::Millimeters)
        .expect("demo_star.svg imports");
    assert!(
        model.drill_targets.is_empty(),
        "fixture is vacuous: the star SVG exposes drill targets"
    );
    assert!(
        model.polygons.as_deref().is_some_and(|p| !p.is_empty()),
        "fixture is vacuous: the star SVG has no closed polygon"
    );
    model
}

/// `fixtures/demo_pocket.svg`: a rounded rectangle with a `<circle>` at
/// (40, 30) r=10 inside it. usvg flattens the circle; `detect_containment`
/// files it as the rectangle's hole; the import door classifies the ring as
/// circle-like and exposes ONE target.
fn pocket_model() -> LoadedModel {
    let path = repo_root().join("fixtures/demo_pocket.svg");
    load_model_file(&path, 0, ModelKind::Svg, ModelUnits::Millimeters)
        .expect("demo_pocket.svg imports")
}

/// The same star polygon, plus ONE circle-centre target — what a DXF with an
/// outline and one `CIRCLE` entity imports to.
fn star_with_one_circle() -> LoadedModel {
    let mut model = star_model();
    model.drill_targets = Arc::new(vec![DrillTarget {
        x: CIRCLE_CENTRE[0],
        y: CIRCLE_CENTRE[1],
        layer: "holes".to_owned(),
        kind: DrillTargetKind::CircleCenter { diameter: 6.0 },
    }]);
    model.layers = Arc::new(vec!["holes".to_owned()]);
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

fn drill_op(selected_holes: Option<Vec<[f64; 2]>>) -> OperationConfig {
    OperationConfig::Drill(DrillConfig {
        depth: 5.0,
        cycle: DrillCycleType::Simple,
        selected_holes,
        ..DrillConfig::default()
    })
}

/// One stock, one tool, one model, one drill op — through the production
/// `add_*` doors. Entry styling and the rapid reorder are pinned off so the
/// emitted columns are exactly the holes.
fn session_with(model: LoadedModel, op: OperationConfig) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock());
    let tool_idx = builder.add_tool(make_endmill_6mm());
    let tool_id = builder.tools()[tool_idx].id.0;
    let model_id = builder.add_model(model);
    let mut tc = toolpath_config("Drill", op, tool_id, model_id);
    tc.dressups.entry_style = DressupEntryStyle::None;
    tc.dressups.optimize_rapid_order = false;
    let _ = builder.add_toolpath(0, tc).expect("add drill toolpath");
    let session = builder.build();
    session
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
    let mut session = session_with(star_model(), drill_op(None));
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
        err.contains("No drill targets"),
        "the refusal must name the missing input; got: {err}"
    );
    assert!(
        session.get_result(0).is_none(),
        "a refused generation must store no toolpath"
    );
}

/// The documented SVG feature survives: a real `<circle>` in an SVG drills,
/// at its centre, and the rectangle around it is not a hole.
#[test]
fn an_svg_circle_drills_at_its_centre_and_the_outline_does_not() {
    let model = pocket_model();
    assert_eq!(
        model.drill_targets.len(),
        1,
        "the pocket SVG exposes exactly its circle: {:?}",
        model.drill_targets
    );
    let mut session = session_with(model, drill_op(None));
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
            .get_result(0)
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
    let mut session = session_with(star_with_one_circle(), drill_op(None));
    let cols = drilled_columns(&mut session, 0);
    let expected = vec![(
        (CIRCLE_CENTRE[0] * 1000.0).round() as i64,
        (CIRCLE_CENTRE[1] * 1000.0).round() as i64,
    )];
    assert_eq!(
        cols, expected,
        "with no pick the op drills every target the model exposes — the one \
         circle centre — and never the outline's centroid (50, 51.1)"
    );
}

#[test]
fn an_empty_pick_still_refuses_when_targets_exist() {
    let mut session = session_with(star_with_one_circle(), drill_op(Some(Vec::new())));
    let cancel = AtomicBool::new(false);
    let err = session
        .generate_toolpath(0, &cancel)
        .err()
        .map(|e| e.to_string())
        .expect("an explicit empty selection refuses rather than drilling everything");
    assert!(err.contains("No drill targets selected"), "got: {err}");
}
