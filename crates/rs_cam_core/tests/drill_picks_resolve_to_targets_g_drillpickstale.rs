//! **G-DRILLPICKSTALE** (F4.8) — a picked drill hole is a frozen
//! coordinate, so a pick that no longer names a target still drills.
//!
//! # The mechanism
//!
//! `DrillConfig::selected_holes` and `AlignmentPinDrillConfig::selected_holes`
//! store a pick as a raw XY COORDINATE, not as a reference into the model's
//! drill targets. The GUI picker copies `DrillTarget::{x, y}` straight in
//! (`crates/rs_cam_viz/src/ui/properties/operations/drill.rs:73`, and the
//! viewport toggle at `crates/rs_cam_viz/src/controller/events/mod.rs:208-229`).
//! `drill_holes_for_config` (`crates/rs_cam_core/src/compute/execute.rs:1007`)
//! then returns those coordinates and never looks at the targets:
//!
//! ```ignore
//! if let Some(selected) = &cfg.selected_holes {
//!     return Ok(selected.iter().map(...).collect());
//! }
//! ```
//!
//! F4.4 (G-RELOADTARGETS) made every refresh door replace
//! `LoadedModel::drill_targets`, so the record now follows the file. The
//! picks do not. The operator picks three holes, moves them in CAD,
//! reloads, regenerates — and the machine drills the PREVIOUS positions.
//! F4.4's fix cannot reach this: the generator never reads the field F4.4
//! repaired.
//!
//! # What this file pins
//!
//! A pick is re-resolved against the model's CURRENT targets. When every
//! pick still names a target the op drills them. When a pick names none of
//! them the generator REFUSES. A refusal is the safe outcome; a silent
//! wrong cut is the worst one.
//!
//! # The scope boundary this file also pins
//!
//! The check runs only when the model exposes at least one target. When
//! the target list is EMPTY there is nothing to re-resolve against, and an
//! empty list means two different things at this seam:
//!
//! - the model exposes no targets, or
//! - the CALLER resolved no model at all —
//!   `execute_operation_annotated` passes `&[]` on purpose
//!   (`crates/rs_cam_core/src/compute/execute.rs:3313`: "a `Drill` op on
//!   this path drills only what it carries in `selected_holes`"), and every
//!   drill fixture in this suite builds its model with
//!   `drill_targets: Arc::new(Vec::new())`.
//!
//! `a_pick_on_a_model_that_exposes_no_targets_still_generates` is the
//! preservation control for that caller class. The residual it names — the
//! operator DELETES every hole from the drawing, so the list goes empty and
//! the stale picks stand — is recorded in
//! `planning/ui_fix_2026-09-09/reports/F4.8.md`. It needs a
//! "was a model resolved" signal this seam does not carry.
//!
//! # Why the refusal cannot be the OLD one
//!
//! `drill_targets_refusal` already refuses two shapes: no pick and no
//! targets, and an EMPTY pick. Every arm below asserts that predicate
//! returns `None` first, so a refusal observed here is the new check and
//! not one of those two.
//!
//! Source: `planning/ui_fix_2026-09-09/reports/F4.4.md` §7 row F4.8.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;
use common::make_endmill_6mm;
use common::session::toolpath_config;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::DressupEntryStyle;
use rs_cam_core::compute::operation_configs::{
    AlignmentPinDrillConfig, DrillConfig, DrillCycleType,
};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::dxf_input::{DrillTarget, DrillTargetKind};
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    AdoptModelGeometryArgs, Command, LoadedModel, ProjectSession, ToolpathComputeResult,
};

/// Where the drawing's one hole sits when the operator picks it.
const HOLE_A: [f64; 2] = [20.0, 30.0];

/// Where the same hole sits after the operator moves it in CAD. 40 mm is
/// four tool diameters away, so no tolerance can confuse the two.
const HOLE_B: [f64; 2] = [60.0, 30.0];

/// The phrase the new refusal carries. Asserting the phrase rather than a
/// bare `is_err()` keeps an unrelated generation failure from passing this
/// file vacuously; the `drill_targets_refusal` check in each arm excludes
/// the two refusals that already existed.
const STALE_PHRASE: &str = "no longer";

// ── fixtures ────────────────────────────────────────────────────────────

/// A plate covering the stock footprint. Every op here picks its holes, so
/// the polygon is never a hole source (G-DRILLCENTROID); it exists so the
/// model is a real 2D model.
fn plate_polygon() -> Polygon2 {
    Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(100.0, 0.0),
        P2::new(100.0, 100.0),
        P2::new(0.0, 100.0),
    ])
}

fn target_at(xy: [f64; 2]) -> DrillTarget {
    DrillTarget {
        x: xy[0],
        y: xy[1],
        layer: "holes".to_owned(),
        kind: DrillTargetKind::CircleCenter { diameter: 6.0 },
    }
}

/// The drawing, with its one hole at `hole`. `targets` empty models a
/// drawing that exposes none.
fn plate_model(hole: Option<[f64; 2]>) -> LoadedModel {
    let targets: Vec<DrillTarget> = hole.into_iter().map(target_at).collect();
    let layers: Vec<String> = if targets.is_empty() {
        Vec::new()
    } else {
        vec!["holes".to_owned()]
    };
    LoadedModel {
        id: 0,
        name: "plate".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![plate_polygon()])),
        drill_targets: Arc::new(targets),
        layers: Arc::new(layers),
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

fn drill_op(picks: &[[f64; 2]]) -> OperationConfig {
    OperationConfig::Drill(DrillConfig {
        depth: 5.0,
        cycle: DrillCycleType::Simple,
        selected_holes: Some(picks.to_vec()),
        ..DrillConfig::default()
    })
}

/// A pin drill whose ONLY holes are picks — `holes` (the stock alignment
/// pin snapshot) is empty, so the emitted columns are exactly the picks.
fn pin_drill_op(picks: &[[f64; 2]]) -> OperationConfig {
    OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig {
        holes: Vec::new(),
        cycle: DrillCycleType::Simple,
        selected_holes: Some(picks.to_vec()),
        ..AlignmentPinDrillConfig::default()
    })
}

/// One stock, one tool, one model, one drill op — through the production
/// `add_*` doors. Entry styling and the rapid reorder are pinned off so
/// the emitted columns are exactly the holes.
fn session_with(model: LoadedModel, op: OperationConfig) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    let _ = session.set_stock_config(stock());
    let tool_idx = session
        .add_tool(make_endmill_6mm())
        .created
        .expect("add_tool reports the new tool index");
    let tool_id = session.tools()[tool_idx].id.0;
    let model_id = session
        .add_model(model)
        .created
        .expect("add_model reports the new model id");
    let mut tc = toolpath_config("Drill", op, tool_id, model_id);
    tc.dressups.entry_style = DressupEntryStyle::None;
    tc.dressups.optimize_rapid_order = false;
    let _ = session.add_toolpath(0, tc).expect("add drill toolpath");
    session
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

/// Without this every arm below could be comparing empty lists, or could
/// be tripping one of the TWO refusals that already existed.
#[test]
fn the_fixture_models_move_the_target_this_file_assumes() {
    let a = plate_model(Some(HOLE_A));
    let b = plate_model(Some(HOLE_B));
    let none = plate_model(None);

    assert_eq!(
        a.drill_targets.len(),
        1,
        "drawing A must expose exactly one drill target"
    );
    assert_eq!(
        b.drill_targets.len(),
        1,
        "drawing B must expose exactly one drill target"
    );
    assert!(
        none.drill_targets.is_empty(),
        "the no-hole drawing must expose no drill target"
    );
    assert!(
        (a.drill_targets[0].x - b.drill_targets[0].x).abs() > 1.0,
        "the hole must MOVE between A and B, else every arm below is vacuous"
    );

    // The OLD predicate exonerates a non-empty pick whatever the target
    // count, so any refusal observed below is the new check.
    let OperationConfig::Drill(cfg) = drill_op(&[HOLE_A]) else {
        panic!("drill_op must build a Drill config");
    };
    for count in [0_usize, 1, 2] {
        assert!(
            rs_cam_core::compute::execute::drill_targets_refusal(&cfg, count).is_none(),
            "`drill_targets_refusal` must NOT refuse a non-empty pick at \
             target count {count} — if it does, this file cannot tell the \
             new refusal from the old ones"
        );
    }
}

// ── the defect ──────────────────────────────────────────────────────────

/// **The operator's case.** The hole was picked at `HOLE_A`. The operator
/// moved it in CAD and reloaded, so the record's target is now `HOLE_B`.
/// The pick is a frozen coordinate, so the machine drills `HOLE_A` — a
/// position that is in no version of the drawing any more.
#[test]
fn a_pick_whose_target_moved_refuses_instead_of_drilling_the_old_position() {
    let mut session = session_with(plate_model(Some(HOLE_A)), drill_op(&[HOLE_A]));
    refresh_model(&mut session, plate_model(Some(HOLE_B)));

    match generate(&mut session) {
        Ok(cols) => panic!(
            "the picked hole moved from {HOLE_A:?} to {HOLE_B:?}, so the \
             pick names no target on the current drawing and generation \
             must REFUSE. It generated and drilled {cols:?}. \
             `drill_holes_for_config` returns `cfg.selected_holes` verbatim \
             and never reads `drill_targets`, so the machine cuts the \
             previous version's hole."
        ),
        Err(msg) => assert!(
            msg.contains(STALE_PHRASE),
            "the refusal must say the pick no longer names a target — got {msg:?}"
        ),
    }
}

/// The same defect on the pin drill. `AlignmentPinDrillConfig::
/// selected_holes` is the same frozen coordinate, resolved by
/// `pin_holes_in_emission_frame`.
#[test]
fn a_pin_drill_pick_whose_target_moved_refuses() {
    let mut session = session_with(plate_model(Some(HOLE_A)), pin_drill_op(&[HOLE_A]));
    refresh_model(&mut session, plate_model(Some(HOLE_B)));

    match generate(&mut session) {
        Ok(cols) => panic!(
            "the pin drill's picked hole moved from {HOLE_A:?} to \
             {HOLE_B:?}, so generation must REFUSE. It drilled {cols:?}."
        ),
        Err(msg) => assert!(
            msg.contains(STALE_PHRASE),
            "the refusal must say the pick no longer names a target — got {msg:?}"
        ),
    }
}

/// One stale pick among several still refuses. A partial emission would be
/// the worst outcome of all: the operator sees a toolpath, counts the
/// holes, and two of the three are right.
#[test]
fn one_stale_pick_among_several_refuses_rather_than_drilling_the_rest() {
    // The drawing keeps HOLE_A and never had HOLE_B.
    let model = plate_model(Some(HOLE_A));
    let mut session = session_with(model, drill_op(&[HOLE_A, HOLE_B]));
    match generate(&mut session) {
        Ok(cols) => panic!(
            "{HOLE_B:?} names no target on this drawing, so generation must \
             REFUSE rather than drill the picks that still resolve. It \
             drilled {cols:?}."
        ),
        Err(msg) => assert!(
            msg.contains(STALE_PHRASE),
            "the refusal must say the pick no longer names a target — got {msg:?}"
        ),
    }
}

// ── the controls ────────────────────────────────────────────────────────

/// **The control.** A pick that still names its target drills it, and
/// drills only it. This passes before the fix and after it, and it is what
/// proves the three assertions above are capable of passing.
#[test]
fn a_pick_that_still_names_its_target_drills_it_control() {
    let mut session = session_with(plate_model(Some(HOLE_A)), drill_op(&[HOLE_A]));
    // A reload that changed nothing — the same door, the same targets.
    refresh_model(&mut session, plate_model(Some(HOLE_A)));

    let cols = generate(&mut session).expect("a resolving pick must generate");
    assert_eq!(
        cols,
        vec![micron(HOLE_A)],
        "the op must drill exactly the picked target"
    );
}

/// **The scope boundary, and a preservation control.** A model that
/// exposes NO targets carries no evidence to re-resolve a pick against,
/// and `&[]` also reaches this seam from callers that resolved no model at
/// all. Those picks stand, exactly as they do today.
///
/// This is deliberate and it is a residual: the operator who DELETES every
/// hole from the drawing still drills the old positions. See the module
/// doc and `reports/F4.8.md`.
#[test]
fn a_pick_on_a_model_that_exposes_no_targets_still_generates() {
    let mut session = session_with(plate_model(None), drill_op(&[HOLE_A]));

    let cols = generate(&mut session).expect(
        "a pick on a model with no targets must still generate — every \
         drill fixture in this suite depends on it",
    );
    assert_eq!(
        cols,
        vec![micron(HOLE_A)],
        "and it must drill the pick it was given"
    );
}
