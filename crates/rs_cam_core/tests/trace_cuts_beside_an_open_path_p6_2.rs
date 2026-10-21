//! P6.2 (geometry selection) — Trace `Left` and `Right` cut beside an open
//! path, on the stated side of its drawn direction (operator ruling O-1).
//!
//! # The claim
//!
//! O-1 (2026-10-21): the label wins. `Left` puts the tool centre on the
//! LEFT of the direction of travel, as G41, on an open path and on a
//! closed loop. Through the session door (`generate_toolpath`):
//!
//! - (a) Trace `Left` of an open line drawn east cuts at `+Y` of it, one
//!   tool radius away; `Right` cuts at `-Y`. The cut runs from the start
//!   normal to the end normal of the line, in the drawn direction.
//! - (b) On a `Bottom` setup the side holds in the drawing frame: each cut
//!   point, mapped back with `SetupTransformInfo::local_to_drawing`, is on
//!   the same side as in (a). The mirror of the setup keeps the point
//!   order of an open path (P2.2), so the kernel takes the other sign in
//!   the setup frame (`TraceParams::drawing_mirrored`).
//! - (c) No fed move goes past the end normals of the drawn path, with the
//!   default dressups of Trace (lead-in/out on), and with a ramp entry
//!   asked for on a 2 mm line (design risk "ramp entry on a short open
//!   path"; Trace strips the entry, `DressupPolicy::FORCE_NO_ENTRY`). An
//!   open offset pass takes no lead: a lead arc starts one lead radius back
//!   along the travel, past the start normal.
//! - (d) A closed loop drawn counter-clockwise: Trace `Left` cuts inside
//!   it (the left of the counter-clockwise travel), one tool radius in,
//!   and the pass runs counter-clockwise. This is the O-1 break: before
//!   P6.2 a closed `Left` cut outside (design D-A).
//!
//! Red proof (P6.2): with the open sign swapped in
//! `trace_compensated_polygons_reported`, (a) and (b) fail on the side.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test trace_cuts_beside_an_open_path_p6_2
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::op_fixtures::{no_dressups, tool_for};
use common::session::{polygon_model, toolpath_config};
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{ArcFitParams, DressupConfig, DressupEntryStyle};
use rs_cam_core::compute::transform::{FaceUp, ZRotation};
use rs_cam_core::geo::P2;
use rs_cam_core::ops::trace_path::TraceCompensation;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::ProjectSessionBuilder;
use rs_cam_core::toolpath::{MoveIntent, MoveType};

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

/// The open line, drawn east along Y 10.
const LINE: [[f64; 2]; 2] = [[-30.0, 10.0], [30.0, 10.0]];

/// A 2 mm line, drawn east along Y -15.
const SHORT: [[f64; 2]; 2] = [[0.0, -15.0], [2.0, -15.0]];

fn open(points: &[[f64; 2]]) -> Polygon2 {
    Polygon2::open_path(points.iter().map(|&[x, y]| P2::new(x, y)).collect())
}

fn trace(compensation: TraceCompensation) -> OperationConfig {
    let mut op = OperationConfig::new_default(OperationType::Trace);
    let OperationConfig::Trace(cfg) = &mut op else {
        unreachable!("the default of Trace is a Trace config")
    };
    cfg.compensation = compensation;
    op
}

/// The tool radius of the Trace test tool.
fn tool_radius() -> f64 {
    tool_for(OperationType::Trace).diameter / 2.0
}

/// The fed moves (not rapids) of one generated Trace, in the DRAWING frame,
/// with their Z and intent.
fn fed_in_drawing(
    face_up: FaceUp,
    compensation: TraceCompensation,
    polygons: Vec<Polygon2>,
    dressups: DressupConfig,
) -> Vec<(P2, f64, MoveIntent)> {
    let mut builder = ProjectSessionBuilder::new().stock(board());
    let tool_idx = builder.add_tool(tool_for(OperationType::Trace));
    let tool_id = builder.tools()[tool_idx].id.0;
    let model_id = builder.add_model(polygon_model(polygons, "p6_2_open_side"));
    let mut cfg = toolpath_config("p6_2", trace(compensation), tool_id, model_id);
    cfg.dressups = dressups;
    let setup = if face_up == FaceUp::Top {
        0
    } else {
        builder.add_setup("Flipped".to_owned(), face_up)
    };
    let _ = builder
        .add_toolpath(setup, cfg)
        .expect("add the Trace toolpath");
    let mut session = builder.build();
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .unwrap_or_else(|e| panic!("{face_up:?} {compensation:?}: Trace generates: {e}"));
    let info = session.setup_transform_info(face_up, ZRotation::Deg0);
    let to_drawing = |p: P2| {
        if face_up == FaceUp::Top {
            p
        } else {
            info.local_to_drawing(p)
        }
    };
    session
        .get_result(0)
        .expect("a generated op has a result")
        .toolpath()
        .moves
        .iter()
        .filter(|m| m.move_type != MoveType::Rapid)
        .map(|m| {
            (
                to_drawing(P2::new(m.target.x, m.target.y)),
                m.target.z,
                m.intent,
            )
        })
        .collect()
}

/// The arc-fit budget of the default dressups: the fit may move a fed
/// point by this much and no more.
fn tol() -> f64 {
    ArcFitParams::default().tolerance
}

/// Assert that every fed move of `fed` lies between the end normals of the
/// east line `line` (X from its start to its end, `tol` slack), and that
/// every fed move at the cut lies one tool radius to `side` (+1 left, -1
/// right) of it, and that the cut spans the line.
fn assert_beside(what: &str, fed: &[(P2, f64, MoveIntent)], line: [[f64; 2]; 2], side: f64) {
    let [[x0, y], [x1, _]] = line;
    let r = tool_radius();
    let tol = tol();
    assert!(!fed.is_empty(), "{what}: non-vacuity, Trace cuts");
    let mut cut_x = (f64::INFINITY, f64::NEG_INFINITY);
    for &(p, _z, intent) in fed {
        assert!(
            p.x >= x0 - tol && p.x <= x1 + tol,
            "{what}: a fed move goes past an end normal of the drawn line: {p:?} ({intent:?})"
        );
        assert!(
            !matches!(intent, MoveIntent::LeadIn | MoveIntent::LeadOut),
            "{what}: an open offset pass takes no lead: {p:?}"
        );
        // The plunge (to the first point) and the cut body. The Z frame of
        // a Bottom setup differs, so the intent tells, not the Z.
        if matches!(intent, MoveIntent::EntryPlunge | MoveIntent::FinishingCut) {
            assert!(
                (p.y - (y + side * r)).abs() <= tol,
                "{what}: a cut point is not one radius ({r}) to the {} of the line: {p:?}",
                if side > 0.0 {
                    "left (+Y)"
                } else {
                    "right (-Y)"
                }
            );
            cut_x = (cut_x.0.min(p.x), cut_x.1.max(p.x));
        }
    }
    // The cut runs from the start normal to the end normal.
    assert!(
        (cut_x.0 - x0).abs() <= tol && (cut_x.1 - x1).abs() <= tol,
        "{what}: the cut spans X {cut_x:?}, not {x0}..{x1}"
    );
}

/// (a) + (b) + (c): Left at +Y, Right at -Y, on a Top and on a Bottom
/// setup, with no dressups and with the default dressups of Trace.
#[test]
fn trace_cuts_beside_an_open_path_on_the_stated_side_p6_2() {
    let defaults = DressupConfig::for_op(OperationType::Trace);
    assert!(
        defaults.lead_in_out.is_some(),
        "non-vacuity: the default Trace dressups hold a lead"
    );
    for face_up in [FaceUp::Top, FaceUp::Bottom] {
        for (compensation, side) in [
            (TraceCompensation::Left, 1.0),
            (TraceCompensation::Right, -1.0),
        ] {
            for (dressed, dressups) in [
                ("no dressups", no_dressups()),
                ("defaults", defaults.clone()),
            ] {
                let what = format!("{face_up:?} {compensation:?} {dressed}");
                let fed = fed_in_drawing(face_up, compensation, vec![open(&LINE)], dressups);
                assert_beside(&what, &fed, LINE, side);
            }
        }
    }
}

/// (c): a 2 mm line with the default dressups and a ramp entry asked for.
/// No fed move passes its end normals.
#[test]
fn a_short_open_path_takes_no_move_past_its_ends_p6_2() {
    let mut dressups = DressupConfig::for_op(OperationType::Trace);
    dressups.entry_style = DressupEntryStyle::Ramp;
    for (compensation, side) in [
        (TraceCompensation::Left, 1.0),
        (TraceCompensation::Right, -1.0),
    ] {
        let what = format!("short {compensation:?}");
        let fed = fed_in_drawing(
            FaceUp::Top,
            compensation,
            vec![open(&SHORT)],
            dressups.clone(),
        );
        assert_beside(&what, &fed, SHORT, side);
    }
}

/// (d), the O-1 break: a closed square drawn counter-clockwise. Trace
/// `Left` cuts inside it, one radius in, and the pass runs
/// counter-clockwise, so the tool is on the left of travel. `Right` cuts
/// outside.
#[test]
fn a_closed_trace_left_cuts_on_the_left_of_travel_o1() {
    let square = Polygon2::new(vec![
        P2::new(-20.0, -20.0),
        P2::new(0.0, -20.0),
        P2::new(0.0, 0.0),
        P2::new(-20.0, 0.0),
    ]);
    assert!(square.signed_area() > 0.0, "the square is drawn CCW");
    let r = tool_radius();
    for (compensation, grow) in [(TraceCompensation::Left, -r), (TraceCompensation::Right, r)] {
        let fed = fed_in_drawing(
            FaceUp::Top,
            compensation,
            vec![square.clone()],
            no_dressups(),
        );
        let cut: Vec<P2> = fed
            .iter()
            .filter(|(_, z, _)| *z < 0.0)
            .map(|(p, _, _)| *p)
            .collect();
        assert!(cut.len() >= 4, "{compensation:?}: non-vacuity");
        let b = Polygon2::new(cut.clone()).bbox();
        let want = [-20.0 - grow, -20.0 - grow, 0.0 + grow, 0.0 + grow];
        for (got, want) in b.iter().zip(want) {
            assert!(
                (got - want).abs() < 1e-6,
                "{compensation:?}: the cut box {b:?}, not {want:?}"
            );
        }
        // The pass runs counter-clockwise (the drawn direction).
        let area: f64 = cut
            .iter()
            .zip(cut.iter().cycle().skip(1))
            .map(|(a, b)| a.x * b.y - b.x * a.y)
            .sum();
        assert!(area > 0.0, "{compensation:?}: the pass runs CCW ({area})");
    }
}
