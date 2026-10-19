//! **G-DRILLPICKSTALE** (F4.8) — a picked drill hole that no longer
//! names an item of the drawing refuses; it never drills.
//!
//! # The mechanism, before P2.3c
//!
//! `DrillConfig::selected_holes` and `AlignmentPinDrillConfig::selected_holes`
//! stored a pick as a raw XY COORDINATE. The GUI picker copied a target's
//! `(x, y)` straight in. F4.4 (G-RELOADTARGETS) made every refresh door
//! replace the model's targets, so the record followed the file and the
//! picks did not: the operator picked a hole, moved it in CAD, reloaded,
//! and the machine drilled the PREVIOUS position. F4.8 made the generator
//! re-resolve each pick against the current targets and refuse a stale
//! one, but it exempted a model with NO targets, because an empty target
//! list also meant "the caller resolved no model".
//!
//! # What this file pins (geometry selection P2.3c)
//!
//! A pick is a `MatchKey` in the op's `Selection`. `selection::resolve`
//! matches it against the items of the CURRENT model; a pick that matches
//! no item is a `LostPick`, and the prologue refuses with
//! `RefusalCause::LostPick`, naming the pick. When every pick matches, the
//! op drills them. A refusal is the safe outcome; a silent wrong cut is
//! the worst one.
//!
//! # The exemption is gone
//!
//! The resolver knows whether the model resolved (`ResolveScope`), so a
//! pick on a model that holds no point and no circle is lost like any
//! other: `a_pick_on_a_model_with_no_holes_refuses` (the F4.8 residual
//! closes). A caller that resolves no model gives an empty selection, and
//! a Drill there refuses `Empty`.
//!
//! Source: `planning/ui_fix_2026-09-09/reports/F4.4.md` §7 row F4.8;
//! `planning/geometry_selection_2026-10-16/DESIGN.md` 5.3.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::make_endmill_6mm;
use common::session::{TestHole, toolpath_config};

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::DressupEntryStyle;
use rs_cam_core::compute::operation_configs::{
    AlignmentPinDrillConfig, DrillConfig, DrillCycleType,
};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::{ItemKind, MatchKey, Selection, SourceRef};
use rs_cam_core::session::{
    AdoptModelGeometryArgs, Command, LoadedModel, ProjectSession, ProjectSessionBuilder,
    ToolpathComputeResult,
};

/// Where the drawing's one hole sits when the operator picks it.
const HOLE_A: [f64; 2] = [20.0, 30.0];

/// Where the same hole sits after the operator moves it in CAD. 40 mm is
/// four tool diameters away, so no tolerance can confuse the two.
const HOLE_B: [f64; 2] = [60.0, 30.0];

/// The diameter of the drawn hole (mm).
const HOLE_DIAMETER: f64 = 6.0;

/// The phrase of the `LostPick` refusal (`RefusalCause::LostPick`).
/// Asserting the phrase rather than a bare `is_err()` keeps an unrelated
/// generation failure, or the `Empty` refusal, from passing this file
/// vacuously.
const LOST_PHRASE: &str = "no item of the drawing matches this pick";

// ── fixtures ────────────────────────────────────────────────────────────

/// A plate covering the stock footprint. A loop is never a hole source
/// (G-DRILLCENTROID); it exists so the model is a real 2D model.
fn plate_polygon() -> Polygon2 {
    Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(100.0, 0.0),
        P2::new(100.0, 100.0),
        P2::new(0.0, 100.0),
    ])
}

fn target_at(xy: [f64; 2]) -> TestHole {
    TestHole {
        x: xy[0],
        y: xy[1],
        layer: "holes".to_owned(),
        diameter: Some(HOLE_DIAMETER),
    }
}

/// The drawing, with its one circle at `hole`. `None` models a drawing
/// that holds no circle.
fn plate_model(hole: Option<[f64; 2]>) -> LoadedModel {
    let targets: Vec<TestHole> = hole.into_iter().map(target_at).collect();
    LoadedModel {
        id: 0,
        name: "plate".to_owned(),
        mesh: None,
        items: Arc::new(common::session::drawing_items(
            vec![plate_polygon()],
            &targets,
        )),
        path: PathBuf::from("synthetic://plate.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    }
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

/// The key a viewport pick of the circle at `xy` stores: the toolpath
/// model, the kind, the layer, the centre and the diameter.
fn pick_at(xy: [f64; 2]) -> MatchKey {
    MatchKey {
        source: SourceRef::ToolpathModel,
        kind: ItemKind::Circle,
        layer: Some("holes".to_owned()),
        anchor: xy,
        measure: HOLE_DIAMETER,
        brep: None,
    }
}

/// A selection of these picks and no rule.
fn picks(at: &[[f64; 2]]) -> Selection {
    Selection::from_picks(at.iter().copied().map(pick_at).collect())
}

fn drill_op(at: &[[f64; 2]]) -> OperationConfig {
    OperationConfig::Drill(DrillConfig {
        depth: 5.0,
        cycle: DrillCycleType::Simple,
        selection: picks(at),
        ..DrillConfig::default()
    })
}

/// A pin drill whose ONLY holes are picks — the setup has no alignment
/// pins, so the emitted columns are exactly the picks.
fn pin_drill_op(at: &[[f64; 2]]) -> OperationConfig {
    OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig {
        diameter: 6.0,
        cycle: DrillCycleType::Simple,
        selection: picks(at),
        ..AlignmentPinDrillConfig::default()
    })
}

/// One stock, one tool, one model, one drill op — through the production
/// `add_*` doors. Entry styling and the rapid reorder are pinned off so
/// the emitted columns are exactly the holes.
fn session_with(model: LoadedModel, op: OperationConfig) -> ProjectSession {
    let mut builder = ProjectSessionBuilder::new().stock(stock());
    let tool_idx = builder.add_tool(make_endmill_6mm());
    let tool_id = builder.tools()[tool_idx].id.0;
    let model_id = builder.add_model(model);
    let mut tc = toolpath_config("Drill", op, tool_id, model_id);
    tc.dressups.entry_style = DressupEntryStyle::None;
    tc.dressups.optimize_rapid_order = false;
    let _ = builder.add_toolpath(0, tc).expect("add drill toolpath");
    builder.build()
}

/// Replace the session's model geometry the way every GUI refresh door
/// does since F4.4 — `LoadedModel::adopt_geometry`, the ONE field split.
/// This is what a Reload from disk leaves behind.
///
/// WP7: the `AdoptModelGeometry` row is that door in core. Beside the
/// field split it drops the results of the toolpaths bound to the model,
/// which every arm below regenerates anyway.
fn refresh_model(session: &mut ProjectSession, fresh: LoadedModel) {
    let model_id = session
        .models()
        .first()
        .expect("the fixture model must be in the session")
        .id;
    let _ = session
        .apply(Command::AdoptModelGeometry(AdoptModelGeometryArgs {
            model_id,
            geometry: Box::new(fresh),
            units: None,
        }))
        .expect("the fixture model is in the session");
}

/// The distinct XY columns one toolpath visits, to micron resolution — a
/// drill cycle only ever parks and descends, so this is the set of holes
/// it machines.
fn columns_of(result: &ToolpathComputeResult) -> Vec<(i64, i64)> {
    let tp = &result.op_data.annotated().toolpath;
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

fn micron(xy: [f64; 2]) -> (i64, i64) {
    let x = (xy[0] * 1000.0).round() as i64;
    let y = (xy[1] * 1000.0).round() as i64;
    (x, y)
}

/// Generate index 0 and return either the columns it drilled or the error
/// it refused with.
fn generate(session: &mut ProjectSession) -> Result<Vec<(i64, i64)>, String> {
    let cancel = AtomicBool::new(false);
    match session.generate_toolpath(0, &cancel) {
        Ok(result) => Ok(columns_of(result)),
        Err(e) => Err(e.to_string()),
    }
}

// ── non-vacuity ─────────────────────────────────────────────────────────

/// The circle centres of a model, in import order.
fn circle_centres(model: &LoadedModel) -> Vec<[f64; 2]> {
    model
        .items
        .iter()
        .filter(|i| i.kind() == ItemKind::Circle)
        .map(|i| i.key().anchor)
        .collect()
}

/// Without this every arm below could be comparing empty lists.
#[test]
fn the_fixture_models_move_the_hole_this_file_assumes() {
    let a = circle_centres(&plate_model(Some(HOLE_A)));
    let b = circle_centres(&plate_model(Some(HOLE_B)));
    let none = circle_centres(&plate_model(None));

    assert_eq!(a, vec![HOLE_A], "drawing A must hold exactly one circle");
    assert_eq!(b, vec![HOLE_B], "drawing B must hold exactly one circle");
    assert!(none.is_empty(), "the no-hole drawing must hold no circle");
    assert!(
        (a[0][0] - b[0][0]).abs() > 1.0,
        "the hole must MOVE between A and B, else every arm below is vacuous"
    );
    // The pick key is the key the import gives the circle (bar the source,
    // which the session binds to the model id).
    let key = plate_model(Some(HOLE_A))
        .items
        .iter()
        .find(|i| i.kind() == ItemKind::Circle)
        .map(|i| i.key().clone())
        .unwrap();
    assert!(
        pick_at(HOLE_A).names_same_geometry(&key),
        "the pick must name the drawn circle: {key:?}"
    );
}

// ── the defect ──────────────────────────────────────────────────────────

/// Assert that `result` is the `LostPick` refusal, and that it names the
/// pick at `xy`.
fn assert_lost(result: Result<Vec<(i64, i64)>, String>, xy: [f64; 2], case: &str) {
    match result {
        Ok(cols) => panic!(
            "{case}: the pick at {xy:?} names no item of the current drawing, \
             so generation must REFUSE. It generated and drilled {cols:?}."
        ),
        Err(msg) => {
            assert!(
                msg.contains(LOST_PHRASE),
                "{case}: the refusal must say the pick names no item — got {msg:?}"
            );
            let at = format!("X {:.3} Y {:.3}", xy[0], xy[1]);
            assert!(
                msg.contains(&at),
                "{case}: the refusal must name the pick ({at}) — got {msg:?}"
            );
        }
    }
}

/// **The operator's case.** The hole was picked at `HOLE_A`. The operator
/// moved it in CAD and reloaded, so the drawing's circle is now at
/// `HOLE_B`. The pick matches no item, so the op refuses and names it.
#[test]
fn a_pick_whose_target_moved_refuses_instead_of_drilling_the_old_position() {
    let mut session = session_with(plate_model(Some(HOLE_A)), drill_op(&[HOLE_A]));
    refresh_model(&mut session, plate_model(Some(HOLE_B)));
    assert_lost(generate(&mut session), HOLE_A, "Drill, moved hole");
}

/// The same on the pin drill: its extra holes are the same selection.
#[test]
fn a_pin_drill_pick_whose_target_moved_refuses() {
    let mut session = session_with(plate_model(Some(HOLE_A)), pin_drill_op(&[HOLE_A]));
    refresh_model(&mut session, plate_model(Some(HOLE_B)));
    assert_lost(generate(&mut session), HOLE_A, "Pin Drill, moved hole");
}

/// One stale pick among several still refuses. A partial emission would be
/// the worst outcome of all: the operator sees a toolpath, counts the
/// holes, and two of the three are right.
#[test]
fn one_stale_pick_among_several_refuses_rather_than_drilling_the_rest() {
    // The drawing keeps HOLE_A and never had HOLE_B.
    let mut session = session_with(plate_model(Some(HOLE_A)), drill_op(&[HOLE_A, HOLE_B]));
    assert_lost(generate(&mut session), HOLE_B, "Drill, one stale of two");
}

/// **The F4.8 residual, closed.** The operator deletes every hole from the
/// drawing. Before P2.3c an empty target list exempted the picks, so the
/// op drilled the old positions. The resolver knows the model resolved,
/// so the pick is lost like any other.
#[test]
fn a_pick_on_a_model_with_no_holes_refuses() {
    let mut session = session_with(plate_model(Some(HOLE_A)), drill_op(&[HOLE_A]));
    refresh_model(&mut session, plate_model(None));
    assert_lost(generate(&mut session), HOLE_A, "Drill, every hole deleted");
}

// ── the control ─────────────────────────────────────────────────────────

/// **The control.** A pick that still names its item drills it, and
/// drills only it. It proves that the arms above are capable of passing.
#[test]
fn a_pick_that_still_names_its_target_drills_it_control() {
    let mut session = session_with(plate_model(Some(HOLE_A)), drill_op(&[HOLE_A]));
    // A reload that changed nothing — the same door, the same items.
    refresh_model(&mut session, plate_model(Some(HOLE_A)));

    let cols = generate(&mut session).expect("a resolving pick must generate");
    assert_eq!(
        cols,
        vec![micron(HOLE_A)],
        "the op must drill exactly the picked circle"
    );
}
