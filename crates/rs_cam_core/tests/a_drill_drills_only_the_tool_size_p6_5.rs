//! Geometry selection P6.5 (operator ruling O-3, design 4.2): a Drill
//! drills the points and the circles of the tool size only.
//!
//! # The claim
//!
//! A circle states the size of its hole (S11). A Drill makes holes of the
//! tool size, so before P6.5 it drilled a 10 mm circle with a 6 mm tool and
//! said nothing (defect D-D: `drilling.rs` read the hole XY only).
//!
//! 1. Under the default rule the Drill drills the two points and the circle
//!    of the tool size. It leaves out the larger and the smaller circle,
//!    with one `HoleFit` note each (lead ruling L-18). The toolpath (the
//!    prologue's holes) and the drill op (`build_drill_op_for_config`)
//!    name the same three holes, and `selection_status` gives the two
//!    circles the state `Skipped`.
//! 2. A pick of the larger circle refuses `HoleDoesNotFit`, by item, and
//!    the status gives the same refusal.
//! 3. The tolerance `e` is `CAD_ROUNDING_MM` on both sides of the tool
//!    diameter: a circle within `0.999 e` fits, a circle `1.001 e` away does
//!    not. 6.0 mm and 6.35 mm (1/4 in) are two sizes.
//! 4. Freshness (the memo): a tool diameter edit builds the status of the
//!    Drill again, and the states follow the new tool.
//! 5. Pin Drill keeps every hole (`HoleFitRule::None`, lead ruling L-19).
//! 6. The Drill `size_tolerance` (operator ruling 2026-10-22) widens the
//!    tool size on both sides: with a 6.0 mm tool and a tolerance of 0.2 mm
//!    the Drill drills a 5.859 mm circle and still leaves out a 6.25 mm
//!    circle, and the note names the tolerance. At the default
//!    (`CAD_ROUNDING_MM`) the 5.859 mm circle is left out, as in P6.5. The
//!    domain refuses a tolerance below `CAD_ROUNDING_MM` at the door, and a
//!    tolerance edit builds the Drill status again.
//! 7. The wanaka100 Holes Drill (12 DXF circles of 5.859 mm, a 6.0 mm tool,
//!    `size_tolerance = 0.2` in the pinned project) fits all 12 circles.
//!
//! Red proof (run, red, reverted): the prologue without the fit step
//! (`require_items` without `fit_holes`). Claim 1 went red: the toolpath
//! drilled the larger and the smaller circle.
//!
//! Red proof of claims 6 and 7 (run, red, reverted): `hole_fit` with
//! `CAD_ROUNDING_MM` in place of `size.tolerance`. Three tests went red:
//! the 0.2 mm arm drilled only the point, the tolerance edit left the
//! 5.859 mm circle `Skipped`, and the wanaka circles did not fit.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_drill_drills_only_the_tool_size_p6_5
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use common::session::{
    TestHole, drawing_items, polygon_model, single_op_session_with, stock_under,
};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::{AlignmentPinDrillConfig, DrillConfig};
use rs_cam_core::selection::hole_fit::DrillSize;
use rs_cam_core::selection::{
    CAD_ROUNDING_MM, HoleFit, ItemKind, MatchKey, NoteCause, RefusalCause, Selection,
    SelectionEdit, SourceRef,
};
use rs_cam_core::session::{
    Command, EditSelectionArgs, ItemState, LoadedModel, ProjectSession, SelectionReport,
    SelectionTarget, SessionError, SetToolParamArgs, SetToolpathParamArgs,
};
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

/// The tool diameter (mm): 1/4 in, so a 6 mm circle is another size.
const TOOL_D: f64 = 6.35;
/// The larger circle (mm): between `D` and `2 D`.
const LARGER_D: f64 = 10.0;
/// The smaller circle (mm).
const SMALLER_D: f64 = 4.0;
/// The stock half extent and height (mm).
const STOCK_HALF: f64 = 30.0;
const STOCK_HEIGHT: f64 = 10.0;

const POINT_A: [f64; 2] = [-15.0, -15.0];
const POINT_B: [f64; 2] = [15.0, -15.0];
const TOOL_SIZE_AT: [f64; 2] = [0.0, 0.0];
const LARGER_AT: [f64; 2] = [-15.0, 15.0];
const SMALLER_AT: [f64; 2] = [15.0, 15.0];

fn hole(xy: [f64; 2], diameter: Option<f64>) -> TestHole {
    TestHole {
        x: xy[0],
        y: xy[1],
        layer: "HOLES".to_owned(),
        diameter,
    }
}

fn model_of(holes: &[TestHole]) -> LoadedModel {
    let mut model = polygon_model(Vec::new(), "p6_5_holes");
    model.items = Arc::new(drawing_items(Vec::new(), holes));
    model
}

/// Design 6 `hole_model`: 2 points, a circle of the tool size, a larger
/// circle and a smaller circle.
fn hole_model() -> LoadedModel {
    model_of(&[
        hole(POINT_A, None),
        hole(POINT_B, None),
        hole(TOOL_SIZE_AT, Some(TOOL_D)),
        hole(LARGER_AT, Some(LARGER_D)),
        hole(SMALLER_AT, Some(SMALLER_D)),
    ])
}

fn session(model: LoadedModel, op: OperationConfig) -> ProjectSession {
    single_op_session_with(
        stock_under(STOCK_HALF, STOCK_HEIGHT),
        endmill_tool_config(TOOL_D),
        model,
        "p6_5",
        op,
        |_| {},
    )
}

fn drill(selection: Selection) -> OperationConfig {
    OperationConfig::Drill(DrillConfig {
        selection,
        ..DrillConfig::default()
    })
}

/// An XY rounded to 1 µm.
fn key(xy: [f64; 2]) -> [i64; 2] {
    [
        (xy[0] * 1000.0).round() as i64,
        (xy[1] * 1000.0).round() as i64,
    ]
}

/// The XY of every drilling move of the toolpath.
fn drilled(tp: &Toolpath) -> BTreeSet<[i64; 2]> {
    tp.moves
        .iter()
        .filter(|m| m.intent == MoveIntent::Drilling)
        .map(|m| key([m.target.x, m.target.y]))
        .collect()
}

fn generate(s: &mut ProjectSession) -> Result<Toolpath, SessionError> {
    let cancel = AtomicBool::new(false);
    s.generate_toolpath(0, &cancel)
        .map(|r| r.op_data.annotated().toolpath.clone())
}

fn report(s: &ProjectSession) -> Arc<SelectionReport> {
    let tc = s.toolpath_configs()[0].clone();
    s.selection_status(&tc, SelectionTarget::Op).unwrap()
}

/// The state of the item at `xy`.
fn state_at(s: &ProjectSession, r: &SelectionReport, xy: [f64; 2]) -> ItemState {
    let at = s.models()[0]
        .items
        .iter()
        .position(|i| key(i.key().anchor) == key(xy))
        .unwrap_or_else(|| panic!("no item at {xy:?}"));
    r.item_states[at]
}

/// The pick key of the item at `xy`.
fn pick_at(s: &ProjectSession, xy: [f64; 2]) -> MatchKey {
    let item = s.models()[0]
        .items
        .iter()
        .find(|i| key(i.key().anchor) == key(xy))
        .unwrap_or_else(|| panic!("no item at {xy:?}"));
    MatchKey {
        source: SourceRef::ToolpathModel,
        ..item.key().clone()
    }
}

/// The tool diameter of the first tool of `s`.
fn tool_d(s: &ProjectSession) -> f64 {
    s.tools()[0].diameter
}

/// The `HoleFit` notes of the last generation, as (circle XY, fit).
fn fit_notes(s: &ProjectSession) -> Vec<([i64; 2], HoleFit, usize)> {
    let stats = &s.get_result(0).expect("the result").stats;
    stats
        .selection_note
        .as_ref()
        .map(|f| {
            f.notes
                .iter()
                .filter_map(|n| match n.cause {
                    NoteCause::HoleFit { fit, size } => {
                        assert_eq!(size.diameter, tool_d(s), "the note names the tool");
                        assert_eq!(n.kind, ItemKind::Circle);
                        Some((key(n.first.key.anchor), fit, n.count))
                    }
                    NoteCause::KindNotServed | NoteCause::AtTheTop { .. } => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Claim 1.
#[test]
fn the_default_rule_drills_the_points_and_the_tool_size_circle() {
    let mut s = session(hole_model(), drill(Selection::drawing()));
    let tp = generate(&mut s).expect("the drill generates");
    let want: BTreeSet<[i64; 2]> = [POINT_A, POINT_B, TOOL_SIZE_AT]
        .into_iter()
        .map(key)
        .collect();
    assert_eq!(
        drilled(&tp),
        want,
        "the toolpath drills the two points and the circle of the tool size"
    );
    let op_holes: BTreeSet<[i64; 2]> = s
        .get_result(0)
        .unwrap()
        .drill_op()
        .expect("a drill op")
        .holes
        .iter()
        .map(|h| key(h.xy))
        .collect();
    assert_eq!(op_holes, want, "the drill op names the same holes");

    let mut notes = fit_notes(&s);
    notes.sort_by_key(|n| n.0);
    let mut expected = vec![
        (key(LARGER_AT), HoleFit::Larger, 1),
        (key(SMALLER_AT), HoleFit::Smaller, 1),
    ];
    expected.sort_by_key(|n| n.0);
    assert_eq!(notes, expected, "one note for each circle left out");
    let text = s
        .get_result(0)
        .unwrap()
        .stats
        .selection_note
        .as_ref()
        .unwrap()
        .message();
    assert!(
        text.contains("is larger than the tool diameter 6.350 mm")
            && text.contains("is smaller than the tool diameter 6.350 mm")
            && text.contains("add a Bore op for it, or use a tool of the circle size"),
        "the note text names the fit and the fix: {text}"
    );

    let r = report(&s);
    assert!(r.refusal.is_none(), "{:?}", r.refusal);
    assert_eq!(state_at(&s, &r, LARGER_AT), ItemState::Skipped);
    assert_eq!(state_at(&s, &r, SMALLER_AT), ItemState::Skipped);
    for xy in [POINT_A, POINT_B, TOOL_SIZE_AT] {
        assert_eq!(state_at(&s, &r, xy), ItemState::Cut, "{xy:?}");
    }
    let report_holes: BTreeSet<[i64; 2]> = r.holes.iter().map(|h| key([h.xy.x, h.xy.y])).collect();
    assert_eq!(report_holes, want, "the status holes are the drilled holes");
    assert_eq!(r.skipped_by_kind.get(&ItemKind::Circle), Some(&2));
    let fits: Vec<HoleFit> = r.hole_fits.iter().map(|(_, f)| *f).collect();
    assert_eq!(
        fits,
        [
            HoleFit::NoSize,
            HoleFit::NoSize,
            HoleFit::ToolSize,
            HoleFit::Larger,
            HoleFit::Smaller
        ],
        "the report gives the fit of each hole item"
    );
}

/// Claim 2.
#[test]
fn a_pick_of_the_larger_circle_refuses_by_item() {
    let mut s = session(hole_model(), drill(Selection::drawing()));
    let pick = pick_at(&s, LARGER_AT);
    let _ = s
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit: SelectionEdit::Replace(Box::new(Selection::from_picks(vec![
                pick_at(&s, TOOL_SIZE_AT),
                pick,
            ]))),
        }))
        .unwrap();
    let status = report(&s).refusal.clone().expect("the status refuses");
    let err = generate(&mut s).expect_err("the pick refuses");
    let SessionError::Selection(refusal) = err else {
        panic!("not a selection refusal: {err}");
    };
    assert_eq!(
        refusal, status,
        "the status and generation give one refusal"
    );
    assert_eq!(
        refusal.cause,
        RefusalCause::HoleDoesNotFit {
            fit: HoleFit::Larger,
            size: DrillSize::strict(TOOL_D),
            rule: rs_cam_core::selection::HoleFitRule::ToolSize,
        }
    );
    assert_eq!(refusal.item.as_ref().unwrap().key.anchor, LARGER_AT);
    let text = refusal.to_string();
    assert!(
        text.starts_with("Drill: circle on layer 'HOLES'")
            && text.contains("diameter 10.000 mm")
            && text.contains("is larger than the tool diameter 6.350 mm"),
        "{text}"
    );
}

/// Claim 3.
#[test]
fn the_tolerance_is_the_rounding_on_both_sides() {
    let e = CAD_ROUNDING_MM;
    let inside_up = [-20.0, 0.0];
    let inside_down = [-10.0, 0.0];
    let outside_up = [10.0, 0.0];
    let outside_down = [20.0, 0.0];
    let six_mm = [0.0, 20.0];
    let mut s = session(
        model_of(&[
            hole(inside_up, Some(TOOL_D + 0.999 * e)),
            hole(inside_down, Some(TOOL_D - 0.999 * e)),
            hole(outside_up, Some(TOOL_D + 1.001 * e)),
            hole(outside_down, Some(TOOL_D - 1.001 * e)),
            hole(six_mm, Some(6.0)),
        ]),
        drill(Selection::drawing()),
    );
    let tp = generate(&mut s).expect("the drill generates");
    let want: BTreeSet<[i64; 2]> = [inside_up, inside_down].into_iter().map(key).collect();
    assert_eq!(drilled(&tp), want, "only the circles within e are drilled");
    let mut notes = fit_notes(&s);
    notes.sort_by_key(|n| n.0);
    let mut expected = vec![
        (key(outside_up), HoleFit::Larger, 1),
        (key(outside_down), HoleFit::Smaller, 1),
        (key(six_mm), HoleFit::Smaller, 1),
    ];
    expected.sort_by_key(|n| n.0);
    assert_eq!(notes, expected);
}

/// Claim 4.
#[test]
fn a_tool_diameter_edit_stales_the_drill_item_states() {
    let mut s = session(hole_model(), drill(Selection::drawing()));
    let before = report(&s);
    assert_eq!(state_at(&s, &before, LARGER_AT), ItemState::Skipped);
    assert_eq!(state_at(&s, &before, TOOL_SIZE_AT), ItemState::Cut);
    let builds = s.selection_status_builds();
    let _ = report(&s);
    assert_eq!(s.selection_status_builds(), builds, "no edit, no build");

    let _ = s
        .apply(Command::SetToolParam(SetToolParamArgs {
            index: 0,
            param: "diameter".to_owned(),
            value: serde_json::json!(LARGER_D),
        }))
        .expect("the tool edit applies");
    let after = report(&s);
    assert_eq!(
        s.selection_status_builds(),
        builds + 1,
        "a tool diameter edit builds the Drill status again"
    );
    assert_eq!(state_at(&s, &after, LARGER_AT), ItemState::Cut);
    assert_eq!(state_at(&s, &after, TOOL_SIZE_AT), ItemState::Skipped);
    assert_eq!(state_at(&s, &after, SMALLER_AT), ItemState::Skipped);
    let tp = generate(&mut s).expect("the drill generates");
    let want: BTreeSet<[i64; 2]> = [POINT_A, POINT_B, LARGER_AT].into_iter().map(key).collect();
    assert_eq!(drilled(&tp), want, "generation follows the new tool too");
}

/// Claim 5.
#[test]
fn a_pin_drill_keeps_every_hole() {
    let mut s = session(
        hole_model(),
        OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig {
            selection: Selection::drawing(),
            ..AlignmentPinDrillConfig::default()
        }),
    );
    let tp = generate(&mut s).expect("the pin drill generates");
    let want: BTreeSet<[i64; 2]> = [POINT_A, POINT_B, TOOL_SIZE_AT, LARGER_AT, SMALLER_AT]
        .into_iter()
        .map(key)
        .collect();
    assert_eq!(drilled(&tp), want, "Pin Drill drills every hole");
    assert!(fit_notes(&s).is_empty());
    assert!(report(&s).hole_fits.is_empty(), "Pin Drill has no fit rule");
}

/// Claim 6: the wanaka hole and the tool of the wanaka Holes Drill.
const WANAKA_TOOL_D: f64 = 6.0;
const WANAKA_HOLE_D: f64 = 5.859;
/// 6.25 - 6.0 = 0.25 > 0.2: outside the tolerance.
const WIDE_HOLE_D: f64 = 6.25;
const WANAKA_AT: [f64; 2] = [-15.0, 0.0];
const WIDE_AT: [f64; 2] = [15.0, 0.0];

/// A 6.0 mm tool, a point, a 5.859 mm circle and a 6.25 mm circle, with a
/// Drill of `tolerance` (`None`: the default).
fn tolerance_session(tolerance: Option<f64>) -> ProjectSession {
    let mut cfg = DrillConfig {
        selection: Selection::drawing(),
        ..DrillConfig::default()
    };
    if let Some(t) = tolerance {
        cfg.size_tolerance = t;
    }
    single_op_session_with(
        stock_under(STOCK_HALF, STOCK_HEIGHT),
        endmill_tool_config(WANAKA_TOOL_D),
        model_of(&[
            hole(POINT_A, None),
            hole(WANAKA_AT, Some(WANAKA_HOLE_D)),
            hole(WIDE_AT, Some(WIDE_HOLE_D)),
        ]),
        "p6_5 tolerance",
        OperationConfig::Drill(cfg),
        |_| {},
    )
}

/// The selection-note text of the last generation.
fn note_text(s: &ProjectSession) -> String {
    s.get_result(0)
        .unwrap()
        .stats
        .selection_note
        .as_ref()
        .unwrap()
        .message()
}

/// Claim 6: a tolerance of 0.2 mm drills the 5.859 mm circle and leaves
/// out the 6.25 mm circle; the note names the tolerance.
#[test]
fn a_size_tolerance_drills_the_near_circle_only() {
    let mut s = tolerance_session(Some(0.2));
    let tp = generate(&mut s).expect("the drill generates");
    let want: BTreeSet<[i64; 2]> = [POINT_A, WANAKA_AT].into_iter().map(key).collect();
    assert_eq!(
        drilled(&tp),
        want,
        "the 5.859 mm circle is within 0.2 mm of the 6.0 mm tool"
    );
    assert_eq!(
        fit_notes(&s),
        vec![(key(WIDE_AT), HoleFit::Larger, 1)],
        "the 6.25 mm circle is 0.25 mm from the tool: left out"
    );
    let text = note_text(&s);
    assert!(
        text.contains(
            "is larger than the tool diameter 6.000 mm by more than the size tolerance 0.200 mm"
        ),
        "the note names the tolerance: {text}"
    );
    let r = report(&s);
    assert_eq!(state_at(&s, &r, WANAKA_AT), ItemState::Cut);
    assert_eq!(state_at(&s, &r, WIDE_AT), ItemState::Skipped);
}

/// Claim 6: at the default tolerance the 5.859 mm circle is left out (the
/// P6.5 rule), and the note does not name a tolerance.
#[test]
fn the_default_tolerance_keeps_the_p6_5_rule() {
    assert_eq!(DrillConfig::default().size_tolerance, CAD_ROUNDING_MM);
    let mut s = tolerance_session(None);
    let tp = generate(&mut s).expect("the drill generates");
    let want: BTreeSet<[i64; 2]> = [key(POINT_A)].into_iter().collect();
    assert_eq!(drilled(&tp), want, "only the point is drilled");
    let mut notes = fit_notes(&s);
    notes.sort_by_key(|n| n.0);
    assert_eq!(
        notes,
        vec![
            (key(WANAKA_AT), HoleFit::Smaller, 1),
            (key(WIDE_AT), HoleFit::Larger, 1),
        ]
    );
    let text = note_text(&s);
    assert!(
        !text.contains("size tolerance"),
        "the default tolerance is not named: {text}"
    );
}

/// Claim 6: the domain refuses a tolerance below `CAD_ROUNDING_MM` at the
/// setter door, and the config keeps its value.
#[test]
fn a_tolerance_below_the_rounding_is_refused_at_the_door() {
    let mut s = tolerance_session(None);
    let err = s
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "size_tolerance".to_owned(),
            value: serde_json::json!(CAD_ROUNDING_MM / 2.0),
        }))
        .expect_err("a tolerance below the CAD rounding refuses");
    assert!(
        err.to_string().contains("size_tolerance"),
        "the refusal names the parameter: {err}"
    );
    let OperationConfig::Drill(cfg) = &s.toolpath_configs()[0].operation else {
        panic!("not a drill");
    };
    assert_eq!(cfg.size_tolerance, CAD_ROUNDING_MM, "the value is kept");
}

/// Claim 6: a tolerance edit builds the Drill status again, and the states
/// follow the new tolerance.
#[test]
fn a_tolerance_edit_stales_the_drill_item_states() {
    let mut s = tolerance_session(None);
    let before = report(&s);
    assert_eq!(state_at(&s, &before, WANAKA_AT), ItemState::Skipped);
    let builds = s.selection_status_builds();
    let _ = report(&s);
    assert_eq!(s.selection_status_builds(), builds, "no edit, no build");
    let _ = s
        .apply(Command::SetToolpathParam(SetToolpathParamArgs {
            index: 0,
            param: "size_tolerance".to_owned(),
            value: serde_json::json!(0.2),
        }))
        .expect("the tolerance edit applies");
    let after = report(&s);
    assert_eq!(
        s.selection_status_builds(),
        builds + 1,
        "a tolerance edit builds the Drill status again"
    );
    assert_eq!(state_at(&s, &after, WANAKA_AT), ItemState::Cut);
    assert_eq!(state_at(&s, &after, WIDE_AT), ItemState::Skipped);
}

/// Claim 7: the pinned wanaka project loads through the project door, and
/// its Holes Drill fits all 12 circles of `holes.dxf`.
#[test]
fn the_wanaka_holes_drill_fits_all_twelve_circles() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("wanaka_2026-08-16_f530995a.toml");
    let s = ProjectSession::load(common::require_fixture(&path)).expect("the project loads");
    let tc = s
        .toolpath_configs()
        .iter()
        .find(|tc| tc.name == "Holes")
        .expect("the Holes toolpath")
        .clone();
    let OperationConfig::Drill(cfg) = &tc.operation else {
        panic!("Holes is not a Drill");
    };
    assert_eq!(cfg.size_tolerance, 0.2, "the project sets the tolerance");
    let r = s.selection_status(&tc, SelectionTarget::Op).unwrap();
    assert!(r.refusal.is_none(), "{:?}", r.refusal);
    let circles: Vec<HoleFit> = r.hole_fits.iter().map(|(_, f)| *f).collect();
    assert_eq!(circles.len(), 12, "the drawing holds 12 circles");
    assert!(
        circles.iter().all(|f| *f == HoleFit::ToolSize),
        "every circle fits: {circles:?}"
    );
    assert_eq!(r.holes.len(), 12, "the Drill drills all 12 circles");
}
