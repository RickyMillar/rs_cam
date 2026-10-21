//! P2.2 — the default selection gives the old lists (DESIGN 4.4, 14 row
//! P2.2, 17).
//!
//! `view(resolve(Selection::drawing()))` must give, bit for bit:
//!
//! - for `ItemUse::Curves`: `outlines_pre_selection()`;
//! - for `ItemUse::Regions`: the same list less its open paths (P1 leaves
//!   them out);
//! - for `ItemUse::Holes`: the positions of the old drill target list
//!   (`drill_targets_pre_selection()` until P2.3c deleted it; now
//!   `common::session::old_drill_targets`, the same rule on the same
//!   inputs), in the same order.
//!
//! A list that is empty (or `None`) before must refuse with `Empty` now.
//!
//! The claim is checked in the model frame and through the frame door: on
//! a `Bottom` flip, `view(resolve(..).to_setup_frame(info))` against the
//! old list moved by `apply_to_drawing_polygons` (the door generation used
//! before P2.3) and the old targets moved by `drawing_to_local`.
//!
//! The drawings: every file of the P2.1 baseline, and every hand-built
//! drawing the G-OPGOLDEN cases use (in both frames of the case session),
//! and the P1 sentry drawing.
//!
//! The claim does NOT hold everywhere. P2.2 measured three classes of
//! difference; P2.3a closed the third (lead ruling "fix b": an open path
//! keeps its drawn direction). The two lists below freeze the rest, with
//! the cause of each. A view that differs and is not in a list, or a
//! listed view that is equal now, turns this sentry red. Every G-OPGOLDEN
//! drawing and the P1 drawing are equal.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use rs_cam_core::compute::catalog::{ItemUse, OperationConfig, OperationType};
use rs_cam_core::compute::config::StockSource;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::compute::transform::{FaceUp, SetupTransformInfo, ZRotation};
use rs_cam_core::geo::{BoundingBox3, P2, P3};
use rs_cam_core::io::{infer_kind_from_path, load_model_file};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::{HoleFitRule, RefusalCause, ResolveScope, Selection, resolve, view};
use rs_cam_core::session::LoadedModel;

use common::op_fixtures::{self as f, CaseSpec, Fixture, Frame, Prior, case_session};
use common::session::polygon_model;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const BASELINE: &str = "crates/rs_cam_core/tests/fixtures/selection_p2_1/bridges_before_p2_1.txt";

fn dump(polys: &[Polygon2]) -> String {
    let mut out = String::new();
    for p in polys {
        write!(out, "P closed={} n={} :", p.closed, p.exterior.len()).unwrap();
        for q in &p.exterior {
            write!(out, " {:x},{:x}", q.x.to_bits(), q.y.to_bits()).unwrap();
        }
        writeln!(out).unwrap();
        for h in &p.holes {
            write!(out, " H n={} :", h.len()).unwrap();
            for q in h {
                write!(out, " {:x},{:x}", q.x.to_bits(), q.y.to_bits()).unwrap();
            }
            writeln!(out).unwrap();
        }
    }
    out
}

fn dump_points(points: &[P2]) -> String {
    points
        .iter()
        .map(|p| format!("{:x},{:x}", p.x.to_bits(), p.y.to_bits()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A short, readable account of where two dumps part.
fn first_difference(old: &str, new: &str) -> String {
    let (o, n): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    let at = o
        .iter()
        .zip(&n)
        .position(|(a, b)| a != b)
        .unwrap_or(o.len().min(n.len()));
    format!(
        "{} vs {} lines; first difference at line {at}:\n    old {}\n    new {}",
        o.len(),
        n.len(),
        o.get(at).map_or("<none>", |l| &l[..l.len().min(160)]),
        n.get(at).map_or("<none>", |l| &l[..l.len().min(160)]),
    )
}

/// One difference: the key `<label> [<frame>] <view>` and the detail.
type Diff = (String, String);

/// Compare the three views of `model` with its two bridges. `setup`: the
/// flip to compare through. Returns one entry per view that differs.
fn differences(label: &str, model: &LoadedModel, setup: Option<&SetupTransformInfo>) -> Vec<Diff> {
    let mut out = Vec::new();
    let frame = if setup.is_some() { "flipped" } else { "model" };
    let moved = |polys: Vec<Polygon2>| match setup {
        Some(info) => info.apply_to_drawing_polygons(&polys),
        None => polys,
    };
    let old_curves: Vec<Polygon2> = moved(
        model
            .outlines_pre_selection()
            .map(|p| p.as_ref().clone())
            .unwrap_or_default(),
    );
    let old_regions: Vec<Polygon2> = old_curves.iter().filter(|p| p.closed).cloned().collect();
    let old_holes: Vec<P2> = common::session::old_drill_targets(model)
        .iter()
        .map(|&(x, y, _, _)| {
            let p = P2::new(x, y);
            setup.map_or(p, |info| info.drawing_to_local(p))
        })
        .collect();

    let stock = BoundingBox3 {
        min: P3::new(-1000.0, -1000.0, -50.0),
        max: P3::new(1000.0, 1000.0, 50.0),
    };
    let resolved = resolve(
        &Selection::drawing(),
        &ResolveScope {
            toolpath_model: Ok(model),
            models: std::slice::from_ref(model),
            stock_bbox: &stock,
            setup,
            mesh_faces: &[],
        },
    );
    let resolved = resolved.to_setup_frame(setup).unwrap();

    let mut polys = |use_: ItemUse, old: &[Polygon2], name: &str| match view(&resolved, use_) {
        Ok((items, _)) => {
            let new = items.curves().unwrap().as_slice();
            let (a, b) = (dump(old), dump(new));
            if a != b {
                out.push((
                    format!("{label} [{frame}] {name}"),
                    first_difference(&a, &b),
                ));
            }
        }
        Err(refusal) => {
            let empty = matches!(refusal.cause, RefusalCause::Empty { .. });
            if !(old.is_empty() && empty) {
                out.push((
                    format!("{label} [{frame}] {name}"),
                    format!("old {} polygons, new refuses: {refusal}", old.len()),
                ));
            }
        }
    };
    polys(ItemUse::Curves, &old_curves, "curves");
    polys(ItemUse::Regions, &old_regions, "regions");

    match view(
        &resolved,
        ItemUse::Holes {
            may_be_empty: false,
            fit: HoleFitRule::ToolSize,
        },
    ) {
        Ok((items, _)) => {
            let new: Vec<P2> = items
                .holes()
                .unwrap()
                .as_slice()
                .iter()
                .map(|h| h.xy)
                .collect();
            if dump_points(&old_holes) != dump_points(&new) {
                let mut sorted_old = old_holes.clone();
                let mut sorted_new = new.clone();
                let key = |p: &P2| (p.x.to_bits(), p.y.to_bits());
                sorted_old.sort_by_key(key);
                sorted_new.sort_by_key(key);
                let order_only = dump_points(&sorted_old) == dump_points(&sorted_new);
                out.push((
                    format!("{label} [{frame}] holes"),
                    format!(
                        "{} old vs {} new, order only: {order_only}; old {:?} new {:?}",
                        old_holes.len(),
                        new.len(),
                        old_holes.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>(),
                        new.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>(),
                    ),
                ));
            }
        }
        Err(refusal) => {
            if !(old_holes.is_empty() && matches!(refusal.cause, RefusalCause::Empty { .. })) {
                out.push((
                    format!("{label} [{frame}] holes"),
                    format!("old {} targets, new refuses: {refusal}", old_holes.len()),
                ));
            }
        }
    }
    out
}

/// A `Bottom` flip of a stock that holds the drawing.
fn flip_for(model: &LoadedModel) -> SetupTransformInfo {
    let b = model.items.bbox().unwrap_or([0.0, 0.0, 1.0, 1.0]);
    SetupTransformInfo {
        face_up: FaceUp::Bottom,
        z_rotation: ZRotation::Deg0,
        stock_x: b[2] - b[0] + 20.0,
        stock_y: b[3] - b[1] + 20.0,
        stock_z: 10.0,
        stock_origin_x: b[0] - 10.0,
        stock_origin_y: b[1] - 10.0,
        stock_origin_z: -10.0,
    }
}

fn both_frames(label: &str, model: &LoadedModel) -> Vec<Diff> {
    let mut out = differences(label, model, None);
    out.extend(differences(label, model, Some(&flip_for(model))));
    out
}

/// Assert that the views that differ are exactly `expected`, and print
/// every detail when they are not.
fn assert_differences(diffs: &[Diff], expected: &[&str]) {
    let mut got: Vec<&str> = diffs.iter().map(|(k, _)| k.as_str()).collect();
    got.sort_unstable();
    let mut want = expected.to_vec();
    want.sort_unstable();
    let details: Vec<String> = diffs.iter().map(|(k, d)| format!("{k}: {d}")).collect();
    assert_eq!(
        got,
        want,
        "the set of views that differ from the old lists moved:\n{}",
        details.join("\n")
    );
}

/// The views that differ on the fixture files, MEASURED by P2.2. Each
/// class is an open point for P2.3 (the P2.2 report gives the fixes). A
/// change in this list is a finding: a class healed, or a new one.
///
/// 1. Tie order: equal-area rings are equal in the file units, and the
///    unit scale (or the mirror) makes their areas differ in the last bit,
///    so the area sort of `detect_containment` orders them otherwise. The
///    bridge nests before the scale; `view` nests after it.
/// 2. SVG circle centre, 1 ULP at inches: the item centre is found in the
///    file units and then scaled; the bridge finds it on the scaled ring.
///
/// A third class closed in P2.3a: `detect_containment` reversed an open
/// path whose shoelace area is negative, and the mirror of a `Bottom` flip
/// changed that sign, so the two orders (nest then move, move then nest)
/// gave opposite directions. An open path now keeps its drawn direction
/// (lead ruling "fix b"), so the two orders agree. A new entry of that
/// class is a regression.
const FIXTURE_FILE_DIFFERENCES: &[&str] = &[
    // 1. Tie order.
    "crates/rs_cam_core/tests/fixtures/wanaka100/rivmap_export/holes.dxf Inches [model] curves",
    "crates/rs_cam_core/tests/fixtures/wanaka100/rivmap_export/holes.dxf Inches [model] regions",
    "crates/rs_cam_core/tests/fixtures/wanaka100/rivmap_export/holes.dxf Inches [flipped] curves",
    "crates/rs_cam_core/tests/fixtures/wanaka100/rivmap_export/holes.dxf Inches [flipped] regions",
    "crates/rs_cam_core/tests/fixtures/wanaka100/rivmap_export/holes.dxf Millimeters [flipped] curves",
    "crates/rs_cam_core/tests/fixtures/wanaka100/rivmap_export/holes.dxf Millimeters [flipped] regions",
    // Two open paths of equal chord-closed area swap places (an 8-point
    // and a 9-point path, lines 31 and 32 of the dump). Class 3 hid this
    // entry until P2.3a closed it.
    "crates/rs_cam_core/tests/fixtures/rivmap100_dxf_2026-10-20_c3808eb0/rivers_aligned.dxf Millimeters [flipped] curves",
    "crates/rs_cam_core/tests/fixtures/rivmap100_dxf_2026-10-20_c3808eb0/rivers_aligned.dxf Inches [flipped] curves",
    // 2. SVG circle centre.
    "fixtures/demo_pocket.svg Inches [model] holes",
    "fixtures/demo_pocket.svg Inches [flipped] holes",
    "crates/rs_cam_core/tests/fixtures/selection_p2_1/groups.svg Inches [model] holes",
    "crates/rs_cam_core/tests/fixtures/selection_p2_1/groups.svg Inches [flipped] holes",
];

/// The views that differ on the hand-built drawings: none. Every
/// G-OPGOLDEN drawing is equal in both frames. The P1 drawing is equal
/// too since P2.3a: `view` nests the clockwise V and keeps its drawn
/// direction, so the V starts at X 15 as drawn (before "fix b", the
/// nesting reversed it to start at X 35).
const GOLDEN_DRAWING_DIFFERENCES: &[&str] = &[];

/// Every file of the P2.1 baseline, at the units it names.
fn baseline_files() -> Vec<(String, LoadedModel)> {
    let text = std::fs::read_to_string(repo().join(BASELINE)).unwrap();
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let label = line.split(" | ").next().unwrap();
        let Some((rel, units)) = label.rsplit_once(' ') else {
            continue; // an in-test drawing; see `golden_drawings`
        };
        let units = match units {
            "Millimeters" => ModelUnits::Millimeters,
            "Inches" => ModelUnits::Inches,
            other => panic!("units {other}"),
        };
        let path = repo().join(rel);
        let kind = infer_kind_from_path(&path).unwrap();
        if kind == ModelKind::Step && !cfg!(feature = "step") {
            continue;
        }
        let model = load_model_file(&path, 0, kind, units, Default::default()).unwrap();
        out.push((label.to_owned(), model));
    }
    out
}

#[test]
fn the_default_selection_gives_the_old_lists_on_every_fixture_file() {
    let files = baseline_files();
    let want = if cfg!(feature = "step") { 46 } else { 30 };
    assert_eq!(
        files.len(),
        want,
        "non-vacuity: the baseline files that ran"
    );
    let diffs: Vec<Diff> = files
        .iter()
        .flat_map(|(label, model)| both_frames(label, model))
        .collect();
    assert_differences(&diffs, FIXTURE_FILE_DIFFERENCES);
}

/// The P1 sentry drawing: a square, a clockwise V and a 2-point line
/// (`open_paths_are_never_chord_closed_p1.rs`).
fn p1_drawing() -> LoadedModel {
    let open =
        |pts: &[[f64; 2]]| Polygon2::open_path(pts.iter().map(|&[x, y]| P2::new(x, y)).collect());
    polygon_model(
        vec![
            f::rectangle(-40.0, -15.0, -10.0, 15.0),
            open(&[[15.0, -10.0], [25.0, 10.0], [35.0, -10.0]]),
            open(&[[10.0, 20.0], [40.0, 20.0]]),
        ],
        "p1_open_paths",
    )
}

/// The hand-built drawings of G-OPGOLDEN (and the P1 sentry), each with
/// the frame its golden case uses it in.
fn golden_drawings() -> Vec<(String, LoadedModel, Option<SetupTransformInfo>)> {
    let mut out: Vec<(String, LoadedModel, Option<SetupTransformInfo>)> = vec![
        ("outline_model".into(), f::outline_model(), None),
        ("curve_model".into(), f::curve_model(), None),
        ("flat_prior_model".into(), f::flat_prior_model(), None),
        // `every_op_keeps_its_toolpath_g_opgolden.rs`: split_curve_model,
        // square_curve_model and the `regions` boundary model.
        (
            "opgolden_split_curve".into(),
            polygon_model(
                vec![
                    Polygon2::open_path(vec![P2::new(-7.0, -2.0), P2::new(-2.0, 2.0)]),
                    Polygon2::open_path(vec![P2::new(-1.0, 2.0), P2::new(3.0, -2.0)]),
                    Polygon2::open_path(vec![P2::new(5.0, -2.0), P2::new(7.0, 2.0)]),
                ],
                "opgolden_split_curve",
            ),
            None,
        ),
        (
            "opgolden_square_curve".into(),
            polygon_model(vec![f::rectangle(-4.0, -4.0, 4.0, 4.0)], "sq"),
            None,
        ),
        (
            "dressup_face".into(),
            polygon_model(vec![f::rectangle(0.0, 0.0, 40.0, 30.0)], "dressup_face"),
            None,
        ),
        ("p1_open_paths".into(), p1_drawing(), None),
    ];
    // The case sessions in both frames: the flipped frame holds the
    // mirrored models and a `Bottom` setup.
    for frame in [Frame::TopUp, Frame::Flipped] {
        for (op, prior, fixture) in [
            (OperationType::Pocket, Prior::RestPrior, Fixture::Standard),
            (OperationType::Pocket, Prior::None, Fixture::NothingToCut),
            (OperationType::ProjectCurve, Prior::None, Fixture::Standard),
        ] {
            let (session, _) = case_session(CaseSpec {
                name: "p2_2",
                op: OperationConfig::new_default(op),
                prior,
                stock_source: StockSource::Fresh,
                frame,
                fixture,
                tool: None,
            });
            let info = (frame == Frame::Flipped)
                .then(|| session.setup_transform_info(FaceUp::Bottom, ZRotation::Deg0));
            for model in session.models() {
                if model.items.is_empty() {
                    continue;
                }
                out.push((
                    format!("case {frame:?} {}", model.name),
                    model.clone(),
                    info.clone(),
                ));
            }
        }
    }
    out
}

#[test]
fn the_default_selection_gives_the_old_lists_on_the_golden_drawings() {
    let drawings = golden_drawings();
    let mut diffs = Vec::new();
    for (label, model, setup) in &drawings {
        diffs.extend(differences(label, model, None));
        let info = setup.clone().unwrap_or_else(|| flip_for(model));
        diffs.extend(differences(label, model, Some(&info)));
    }
    assert_differences(&diffs, GOLDEN_DRAWING_DIFFERENCES);
}

/// Lead ruling "fix b": the clockwise V of the P1 drawing keeps its drawn
/// direction through `view`, so it starts at X 15, as drawn. Before the
/// ruling the nesting reversed it to start at X 35.
#[test]
fn an_open_path_keeps_its_drawn_direction_in_the_view() {
    let model = p1_drawing();
    let stock = BoundingBox3 {
        min: P3::new(-1000.0, -1000.0, -50.0),
        max: P3::new(1000.0, 1000.0, 50.0),
    };
    let resolved = resolve(
        &Selection::drawing(),
        &ResolveScope {
            toolpath_model: Ok(&model),
            models: std::slice::from_ref(&model),
            stock_bbox: &stock,
            setup: None,
            mesh_faces: &[],
        },
    );
    let (items, _) = view(&resolved, ItemUse::Curves).unwrap();
    let v = items
        .curves()
        .unwrap()
        .as_slice()
        .iter()
        .find(|p| !p.closed && p.exterior.len() == 3)
        .expect("the V path is in the view");
    assert_eq!(
        v.exterior.first().map(|p| (p.x, p.y)),
        Some((15.0, -10.0)),
        "the V starts where it is drawn"
    );
}
