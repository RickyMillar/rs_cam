//! P1 (geometry selection) — no op closes an open path with a chord in
//! silence.
//!
//! # The claim
//!
//! `Polygon2.closed` carries the open flag from the import door to the
//! prologue. Before P1 every closed-region op (Pocket, Profile, 2D
//! Adaptive, Zigzag, Chamfer, V-Carve, Inlay, Inlay Plug) and Trace closed
//! an open path of three or more points with a chord and cut it, and
//! dropped a 2-point line, with no word.
//!
//! The lead ruling (DESIGN D2/D13) for the default selection:
//!
//! - (a) a closed-region op (`ItemUse::Regions`) given closed loops and
//!   open paths cuts the closed loops only, the same toolpath as with the
//!   closed loops alone, and records ONE `SelectionNoteFinding` that counts
//!   the open paths, gives the first one's start and length, and names the
//!   fix. The diagnostics carry it as `geom.selection_note` (Caution).
//!   P2.3a: the finding holds the `SelectionNote`s of the selection view
//!   (one per layer; the first item named by its drawing key).
//! - (b) with open paths only, it refuses, and the message names the open
//!   paths and the fix. P2.3a: the refusal is the selection `Empty`
//!   refusal (`OperationError::Selection`).
//! - (c) Trace with compensation `None` (`ItemUse::Curves`) cuts an open
//!   path on the line, with no chord, and cuts a 2-point line.
//! - (d) P6.2 (rewritten): Trace `Left` reads `ItemUse::Curves`. It cuts
//!   the open paths beside their drawn line (the open offset kernel) and
//!   the closed loop inside it (O-1: the left of a counter-clockwise
//!   loop), with no chord and no note. Before P6.2 it was a closed-region
//!   op, and (a) and (b) covered it.
//! - (e) P1b (lead ruling 2026-10-19): Trace on the line cuts exactly the
//!   drawn open path. With the default dressups of Trace (lead-in/out on),
//!   no move goes past the start or the end of an open path. A closed loop
//!   with a side (Left or Right) keeps its leads; P6.0c gives a loop on the
//!   line no lead.
//!
//! Red proof (2026-10-19): with `trace_polygons_at_z` sent through
//! `emit_closed_contour_with_intent` for open paths again, (c) fails on
//! the chord and (with the chord check off) on the missing 2-point line.
//! With the prologue giving a `Regions` op every polygon again, (a) and
//! (b) fail.
//!
//! Red proof (P1b, 2026-10-19): with Trace back on `TRANSFORM_DEPTH_ORDER`
//! (leads on every pass), (e) fails: the lead-in plunge of the V lands at
//! X 12.3 Y -10.9, outside the drawn extent. (c) fails too: the line cut
//! no longer starts at the drawn start. Since P6.0c that proof no longer
//! goes red: an open path states no material side and Trace has no
//! surface probe, so the lead dressup gives it no lead in any scope.
//! `LeadScope::ClosedPassesOnly` is a second guard now.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test open_paths_are_never_chord_closed_p1
//! ```

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::op_fixtures::{no_dressups, rectangle, tool_for};
use common::session::{polygon_model, single_op_session_with};
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::{GeometryRequirement, ItemUse, OperationConfig, OperationType};
use rs_cam_core::compute::config::ArcFitParams;
use rs_cam_core::compute::toolpath_stats::SelectionNoteFinding;
use rs_cam_core::diagnostics::{Severity, ids};
use rs_cam_core::geo::P2;
use rs_cam_core::ops::trace_path::TraceCompensation;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::ItemKind;
use rs_cam_core::session::{ProjectSession, ToolpathConfig};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};

/// A 100 x 60 x 10 mm board, centred on X0 Y0, its top at Z0.
fn board() -> StockConfig {
    StockConfig {
        x: 100.0,
        y: 60.0,
        z: 10.0,
        origin_x: -50.0,
        origin_y: -30.0,
        origin_z: -10.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

/// The closed loop: a 30 x 30 mm square on the left half of the board.
fn square() -> Polygon2 {
    rectangle(-40.0, -15.0, -10.0, 15.0)
}

/// The V: an open path of three points on the right half. Its chord, the
/// line a chord-closing op cuts, runs along Y -10 from X 35 back to X 15.
const V: [[f64; 2]; 3] = [[15.0, -10.0], [25.0, 10.0], [35.0, -10.0]];

/// The line: a 2-point open path on the right half, along Y 20.
const LINE: [[f64; 2]; 2] = [[10.0, 20.0], [40.0, 20.0]];

fn open(points: &[[f64; 2]]) -> Polygon2 {
    Polygon2::open_path(points.iter().map(|&[x, y]| P2::new(x, y)).collect())
}

/// Every closed-region op that reads a drawing, from the registry. Face
/// reads the stock outline by default (P4.2, `is_stock_based`), not the
/// drawing, so it is not in the list. P6.2: Trace offset to one side reads
/// curves, so it left the list (arm (d)).
fn region_ops() -> Vec<OperationConfig> {
    let ops: Vec<OperationConfig> = OperationType::ALL
        .iter()
        .filter(|op| {
            op.geometry_requirement() == GeometryRequirement::Items
                && op.capabilities().item_use == ItemUse::Regions
                && !op.is_stock_based()
        })
        .map(|&op| OperationConfig::new_default(op))
        .collect();
    assert!(
        ops.len() >= 8,
        "non-vacuity: the eight closed-region ops, found {}",
        ops.len()
    );
    ops
}

fn trace(compensation: TraceCompensation) -> OperationConfig {
    let mut op = OperationConfig::new_default(OperationType::Trace);
    let OperationConfig::Trace(cfg) = &mut op else {
        unreachable!("the default of Trace is a Trace config")
    };
    cfg.compensation = compensation;
    op
}

fn session(
    op: &OperationConfig,
    polygons: Vec<Polygon2>,
    tweak: impl FnOnce(&mut ToolpathConfig),
) -> ProjectSession {
    single_op_session_with(
        board(),
        tool_for(op.op_type()),
        polygon_model(polygons, "p1_open_paths"),
        "p1",
        op.clone(),
        tweak,
    )
}

fn generate(session: &mut ProjectSession) -> Result<Toolpath, String> {
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .map_err(|e| e.to_string())?;
    Ok(session
        .result_at(0)
        .expect("a generated op has a result")
        .annotated()
        .toolpath
        .clone())
}

/// (a): with the V and the line beside the square, a closed-region
/// op emits exactly the toolpath of the square alone, and the note counts
/// the two open paths.
#[test]
fn a_region_op_cuts_only_the_closed_loops_and_names_the_open_paths_p1() {
    for op in region_ops() {
        let label = op.label();
        let alone = generate(&mut session(&op, vec![square()], |_| {}))
            .unwrap_or_else(|e| panic!("{label}: the square alone generates: {e}"));
        assert!(
            alone.moves.iter().any(|m| m.move_type != MoveType::Rapid),
            "{label}: non-vacuity, the square is cut"
        );
        let mut mixed = session(&op, vec![square(), open(&V), open(&LINE)], |_| {});
        let both = generate(&mut mixed)
            .unwrap_or_else(|e| panic!("{label}: closed + open generates: {e}"));
        assert_eq!(
            format!("{:?}", both.moves),
            format!("{:?}", alone.moves),
            "{label}: the open paths change nothing it cuts"
        );
        let stats = &mixed.result_at(0).expect("the result").stats;
        let finding: &SelectionNoteFinding = stats
            .selection_note
            .as_ref()
            .unwrap_or_else(|| panic!("{label}: the open paths are named"));
        assert_eq!(finding.op, label);
        // One note: both paths are on one layer (none).
        assert_eq!(finding.notes.len(), 1, "{label}: {finding:?}");
        let note = &finding.notes[0];
        assert_eq!(
            (note.kind, note.count, note.layer.as_deref()),
            (ItemKind::Path, 2, None),
            "{label}"
        );
        // The first path left out is the V, named by its drawn start and
        // its length.
        assert_eq!(note.first.key.anchor, V[0], "{label}");
        assert!(
            (note.first.key.measure - 2.0 * 10.0_f64.hypot(20.0)).abs() < 1e-9,
            "{label}: {}",
            note.first.key.measure
        );
    }
}

/// (a): the square alone records no note, and the diagnostics of the mixed
/// drawing carry the note as Caution with the fix.
#[test]
fn the_note_reaches_the_diagnostics_p1() {
    let op = OperationConfig::new_default(OperationType::Pocket);
    let mut alone = session(&op, vec![square()], |_| {});
    let _ = generate(&mut alone).expect("the square generates");
    assert_eq!(
        alone.result_at(0).expect("the result").stats.selection_note,
        None,
        "nothing left out, nothing named"
    );

    let mut mixed = session(&op, vec![square(), open(&V)], |_| {});
    let _ = generate(&mut mixed).expect("closed + open generates");
    let diags = mixed
        .diagnose_toolpath_with_trace(0, None)
        .expect("diagnose");
    let d = diags
        .iter()
        .find(|d| d.id.as_str() == ids::GEOM_SELECTION_NOTE)
        .expect("the diagnostics carry the note");
    assert_eq!(d.severity, Severity::Caution);
    for needle in [
        "Pocket: 1 open path is not cut",
        "cuts closed areas only",
        "X 15.000 Y -10.000",
        "Use Trace or Project Curve",
        "close it in the drawing",
    ] {
        assert!(d.message.contains(needle), "{needle:?} in {}", d.message);
    }
}

/// (b): with open paths only, a closed-region op refuses, and the
/// message names the open paths and the fix.
#[test]
fn a_region_op_given_only_open_paths_refuses_p1() {
    for op in region_ops() {
        let label = op.label();
        let err = generate(&mut session(&op, vec![open(&V), open(&LINE)], |_| {}))
            .expect_err("no closed loop: the op refuses");
        for needle in [
            format!("{label}: nothing is left to cut"),
            "2 open paths are not cut".to_owned(),
            "Use Trace or Project Curve".to_owned(),
            "close it in the drawing".to_owned(),
        ] {
            assert!(err.contains(&needle), "{label}: {needle:?} in {err}");
        }
    }
}

/// The fed moves of `tp` as XY segments, each from the previous target.
fn fed_segments(tp: &Toolpath) -> Vec<(P2, P2, f64)> {
    tp.moves
        .windows(2)
        .filter(|w| w[1].move_type != MoveType::Rapid)
        .map(|w| {
            (
                P2::new(w[0].target.x, w[0].target.y),
                P2::new(w[1].target.x, w[1].target.y),
                w[1].target.z,
            )
        })
        .collect()
}

/// (c): Trace on the line cuts the V with no chord and cuts the 2-point
/// line, with every dressup off and with the default dressups of Trace
/// (entry, links: the same dressups an open Project Curve path passes).
#[test]
fn trace_cuts_an_open_path_on_the_line_with_no_chord_p1() {
    let op = trace(TraceCompensation::None);
    for (what, dressed) in [("no dressups", false), ("default dressups", true)] {
        let mut s = session(&op, vec![open(&V), open(&LINE)], |tc| {
            if !dressed {
                tc.dressups = no_dressups();
            }
        });
        let tp = generate(&mut s).unwrap_or_else(|e| panic!("{what}: Trace generates: {e}"));
        let fed = fed_segments(&tp);
        assert!(!fed.is_empty(), "{what}: non-vacuity, Trace cuts");
        assert_eq!(
            s.result_at(0).expect("the result").stats.selection_note,
            None,
            "{what}: Trace on the line leaves nothing out"
        );

        // No fed move runs along the chord of the V (both ends on Y -10,
        // between its two legs). A leg rises 20 mm in Y, so it never
        // matches.
        let chord = fed.iter().find(|(a, b, _)| {
            a.y < -9.99 && b.y < -9.99 && (a - b).norm() > 1.0 && a.x.min(b.x) < 34.0
        });
        assert!(
            chord.is_none(),
            "{what}: a chord of the V is cut: {chord:?}"
        );

        // The V is cut leg by leg, below the stock top.
        let cut_to = |x: f64, y: f64| {
            fed.iter()
                .any(|(_, b, z)| *z < 0.0 && (b - P2::new(x, y)).norm() < 1e-6)
        };
        assert!(cut_to(V[1][0], V[1][1]), "{what}: the V apex is cut");
        assert!(cut_to(V[2][0], V[2][1]), "{what}: the V end is cut");

        // The 2-point line is cut, from end to end, and no further: P1b
        // gives an open Trace path no lead (see the test below).
        let line_cut = fed.iter().any(|(a, b, z)| {
            *z < 0.0
                && (a - P2::new(LINE[0][0], LINE[0][1])).norm() < 1e-6
                && (b - P2::new(LINE[1][0], LINE[1][1])).norm() < 1e-6
        });
        assert!(line_cut, "{what}: the 2-point line is cut");
        // No fed move comes back along the line to its start.
        let back = fed.iter().any(|(a, b, z)| {
            *z < 0.0
                && (a - P2::new(LINE[1][0], LINE[1][1])).norm() < 1e-6
                && (b - P2::new(LINE[0][0], LINE[0][1])).norm() < 1e-6
        });
        assert!(!back, "{what}: the line is not cut back to its start");
    }
}

/// (d), P6.2: Trace `Left` cuts the square, the V and the line, with no
/// note. The V is cut beside its line (round the outside of its apex), the
/// line one tool radius to its left (+Y), the square inside (O-1). No fed
/// move joins the two ends of the V offset (its chord).
#[test]
fn trace_left_cuts_the_open_paths_beside_their_line_p6_2() {
    let op = trace(TraceCompensation::Left);
    let r = tool_for(OperationType::Trace).diameter / 2.0;
    let mut s = session(&op, vec![square(), open(&V), open(&LINE)], |tc| {
        tc.dressups = no_dressups();
    });
    let tp = generate(&mut s).expect("Trace Left generates");
    assert_eq!(
        s.result_at(0).expect("the result").stats.selection_note,
        None,
        "Trace Left leaves nothing out"
    );
    let fed = fed_segments(&tp);
    let cut_at = |x: f64, y: f64| {
        fed.iter()
            .any(|(_, b, z)| *z < 0.0 && (b - P2::new(x, y)).norm() < 1e-6)
    };
    // The line: its end, one radius to the left (+Y).
    assert!(cut_at(LINE[1][0], LINE[1][1] + r), "the line is cut at +Y");
    // The V: the top of the arc round its apex (a right turn, so the left
    // is the outside of the bend).
    assert!(
        cut_at(V[1][0], V[1][1] + r),
        "the V apex is cut round its outside"
    );
    // The square: its first corner, one radius inside.
    let [x0, y0] = [-40.0, -15.0];
    assert!(cut_at(x0 + r, y0 + r), "the square is cut inside (O-1)");
    // No chord between the two ends of the V offset: both ends sit at
    // Y -10 + R / sqrt(5), on the left normals of the two legs.
    let end_y = -10.0 + r / 5.0_f64.sqrt();
    let chord = fed.iter().find(|(a, b, _)| {
        (a.y - end_y).abs() < 1e-6 && (b.y - end_y).abs() < 1e-6 && (a - b).norm() > 1.0
    });
    assert!(chord.is_none(), "a chord of the V offset is cut: {chord:?}");
}

/// The XY box of `points`, as (min, max).
fn extent(points: &[[f64; 2]]) -> (P2, P2) {
    let mut lo = P2::new(f64::INFINITY, f64::INFINITY);
    let mut hi = P2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    for &[x, y] in points {
        lo = P2::new(lo.x.min(x), lo.y.min(y));
        hi = P2::new(hi.x.max(x), hi.y.max(y));
    }
    (lo, hi)
}

/// (e): with the default dressups of Trace (lead-in/out on), every fed
/// move of an open path lands inside the XY extent of the path it cuts.
/// A lead arc starts or ends one lead radius past the drawn end, so it
/// lands outside. The tolerance is the default arc-fit budget: the arc fit
/// of Trace may move a fed point by that much and no more.
///
/// P6.0c (break): the closed square in the same drawing is cut on the line
/// too, so both of its sides are material, and it keeps its plunge (no
/// lead). Before P6.0c it took a lead arc into one side. A closed loop with
/// a side (Trace Left or Right) keeps its lead:
/// `a_lead_curls_away_from_the_part_p6_0c.rs` holds that arm.
#[test]
fn trace_cuts_an_open_path_with_no_lead_past_its_ends_p1b() {
    let op = trace(TraceCompensation::None);
    let mut s = session(&op, vec![square(), open(&V), open(&LINE)], |tc| {
        assert!(
            tc.dressups.lead_in_out.is_some(),
            "non-vacuity: the default Trace dressups hold a lead"
        );
    });
    let tp = generate(&mut s).expect("Trace generates");
    let tol = ArcFitParams::default().tolerance;
    let boxes = [extent(&V), extent(&LINE)];
    let inside = |p: P2, (lo, hi): (P2, P2)| {
        p.x >= lo.x - tol && p.x <= hi.x + tol && p.y >= lo.y - tol && p.y <= hi.y + tol
    };

    // Every fed move to the right of X 0 (the open paths) lands in the
    // box of V or of LINE.
    let mut open_fed = 0;
    for m in &tp.moves {
        if m.move_type == MoveType::Rapid || m.target.x < 0.0 {
            continue;
        }
        open_fed += 1;
        let p = P2::new(m.target.x, m.target.y);
        assert!(
            inside(p, boxes[0]) || inside(p, boxes[1]),
            "a fed move of an open path goes past its drawn ends: {:?} ({:?})",
            m.target,
            m.intent
        );
        assert!(
            !matches!(m.intent, MoveIntent::LeadIn | MoveIntent::LeadOut),
            "an open path takes no lead: {:?}",
            m.target
        );
    }
    assert!(open_fed > 0, "non-vacuity: the open paths are cut");

    // P6.0c: the closed square (left of X 0) is on the line too, so it
    // takes no lead either.
    let square_leads = tp
        .moves
        .iter()
        .filter(|m| m.target.x < 0.0 && m.intent == MoveIntent::LeadIn)
        .count();
    assert_eq!(
        square_leads, 0,
        "a closed Trace loop on the line takes no lead (P6.0c)"
    );
}
