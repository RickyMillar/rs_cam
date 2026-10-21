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
//!
//! Red proof (run, red, reverted): the prologue without the fit step
//! (`require_items` without `fit_holes`). Claim 1 went red: the toolpath
//! drilled the larger and the smaller circle.
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
use rs_cam_core::selection::{
    CAD_ROUNDING_MM, HoleFit, ItemKind, MatchKey, NoteCause, RefusalCause, Selection,
    SelectionEdit, SourceRef,
};
use rs_cam_core::session::{
    Command, EditSelectionArgs, ItemState, LoadedModel, ProjectSession, SelectionReport,
    SelectionTarget, SessionError, SetToolParamArgs,
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
                    NoteCause::HoleFit { fit, tool_diameter } => {
                        assert_eq!(tool_diameter, TOOL_D, "the note names the tool");
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
            && text.contains("use a Pocket on the circle, or a tool of the circle size"),
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
            tool_diameter: TOOL_D,
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
