//! P2.1 — every import door gives items (DESIGN section 14, row P2.1).
//!
//! The DXF, SVG and STEP doors fill `LoadedModel::items`. The claims:
//!
//! 1. A DXF gives one item per entity and one per chain, and every loop and
//!    path carries its layer.
//! 2. A circle (CIRCLE, full ARC) is ONE item, not a loop and a target.
//! 3. An SVG keeps its open paths, and an item takes the `id` of its
//!    nearest labelled group as its layer.
//! 4. A STEP face item carries a SIGNED normal: an up face points to +Z and
//!    a down face to -Z, whatever the plane Z.
//! 5. The items give the lists of the import before P2.1, bit for bit:
//!    `outlines_pre_selection` the nested polygon list, and the points and
//!    circles the drill target list, in the same order, with the same model
//!    bbox. P2.3c deleted the target bridge (`drill_targets_pre_selection`);
//!    `dump` writes the list that bridge gave, rebuilt on the same inputs
//!    (`common::session::old_drill_targets`), so the frozen baseline holds.
//!
//! Claim 5 compares against `fixtures/selection_p2_1/bridges_before_p2_1.txt`.
//! A probe on the import code of efe9d168 (before P2.1) wrote one line per
//! fixture and units: the polygon count, the target count, and an FNV-1a 64
//! hash of the canonical dump (`dump` below: the bit pattern of every point,
//! the closed flag, the holes, the order, the targets, the bbox). A hash
//! keeps the baseline small; the counts tell you where a change landed.
//! The fixtures are every tracked DXF, SVG and STEP file of the repo (less a
//! planning copy of `fixtures/demo_star.svg`) and the
//! flat drawings of `common/op_fixtures.rs`, each at mm and at inches (the
//! inch arm pins the order of the two scales).
//!
//! One designed change is not in the baseline: a closed DXF polyline of 2
//! points was dropped in silence; it is now a 2-point path (DESIGN 2.2).
//! `a_closed_two_point_polyline_is_kept_as_a_path` holds it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::geo::{P3, V3};
use rs_cam_core::geometry::enriched_mesh::{
    FaceTessellation, SurfaceParams, SurfaceType, build_enriched_mesh,
};
use rs_cam_core::io::{brep_face_items, infer_kind_from_path, load_model_file};
use rs_cam_core::selection::{ItemKind, Shape};
use rs_cam_core::session::LoadedModel;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const SELECTION_FIXTURES: &str = "crates/rs_cam_core/tests/fixtures/selection_p2_1";

fn load(rel: &str, units: ModelUnits) -> LoadedModel {
    let path = repo().join(rel);
    let kind = infer_kind_from_path(&path).expect("a model extension");
    load_model_file(&path, 0, kind, units).unwrap_or_else(|e| panic!("{rel} loads: {e}"))
}

fn count(model: &LoadedModel, kind: ItemKind) -> usize {
    model.items.iter().filter(|i| i.kind() == kind).count()
}

// ── 1. DXF layers ───────────────────────────────────────────────────────

#[test]
fn every_dxf_loop_and_path_carries_its_layer() {
    let model = load(
        &format!("{SELECTION_FIXTURES}/mix.dxf"),
        ModelUnits::Millimeters,
    );
    // 17 entities: 3 closed outlines and 1 full ellipse, 3 circles, 1 point,
    // and 9 open pieces that chain to 1 loop (the LINES square) and 5 paths.
    assert_eq!(count(&model, ItemKind::Loop), 4, "{:#?}", model.items);
    assert_eq!(count(&model, ItemKind::Path), 5);
    assert_eq!(count(&model, ItemKind::Circle), 3);
    assert_eq!(count(&model, ItemKind::Point), 1);
    for item in model.items.iter() {
        let layer = item.layer().unwrap_or_else(|| panic!("no layer: {item:?}"));
        assert!(!layer.is_empty(), "an empty layer: {item:?}");
    }
    let layer_of = |kind: ItemKind, layer: &str| {
        model
            .items
            .iter()
            .filter(|i| i.kind() == kind && i.layer() == Some(layer))
            .count()
    };
    assert_eq!(
        layer_of(ItemKind::Loop, "LINES"),
        1,
        "the LINE square is one loop"
    );
    assert_eq!(
        layer_of(ItemKind::Path, "B"),
        1,
        "a chain does not cross a layer"
    );
    assert_eq!(layer_of(ItemKind::Path, "TEXT"), 2);
    assert_eq!(
        model.items.layers(),
        ["0", "B", "HOLES", "LINES", "OUTER", "POCKET", "SPL", "TEXT"]
    );
}

// ── 2. One item per circle ──────────────────────────────────────────────

#[test]
fn a_circle_is_one_item() {
    let model = load(&format!("{SELECTION_FIXTURES}/mix.dxf"), ModelUnits::Inches);
    let circles: Vec<(f64, f64, f64)> = model
        .items
        .iter()
        .filter_map(|i| match i.shape() {
            Shape::Circle {
                centre, diameter, ..
            } => Some((centre.x, centre.y, *diameter)),
            _ => None,
        })
        .collect();
    // CIRCLE (20, 20) r 5, full ARC (80, 20) r 4, CIRCLE (60, 45) r 3; the
    // declared inch units scale every number by 25.4.
    let want = [(20.0, 20.0, 10.0), (80.0, 20.0, 8.0), (60.0, 45.0, 6.0)];
    assert_eq!(circles.len(), want.len());
    for (got, want) in circles.iter().zip(want) {
        assert!((got.0 - want.0 * 25.4).abs() < 1e-9, "{got:?} vs {want:?}");
        assert!((got.1 - want.1 * 25.4).abs() < 1e-9, "{got:?} vs {want:?}");
        assert!((got.2 - want.2 * 25.4).abs() < 1e-9, "{got:?} vs {want:?}");
    }
    // No loop sits on a circle centre: the circle is not also a loop.
    for loop_item in model.items.iter().filter(|i| i.kind() == ItemKind::Loop) {
        for c in &circles {
            assert!(
                (loop_item.key().anchor[0] - c.0).abs() > 1e-3
                    || (loop_item.key().anchor[1] - c.1).abs() > 1e-3,
                "a loop duplicates the circle at {c:?}"
            );
        }
    }
    // One hole item per circle and one per POINT.
    let holes: Vec<_> = model
        .items
        .iter()
        .filter(|i| matches!(i.kind(), ItemKind::Point | ItemKind::Circle))
        .collect();
    assert_eq!(
        holes
            .iter()
            .filter(|i| i.kind() == ItemKind::Circle)
            .count(),
        3
    );
    assert_eq!(holes.len(), 4);
}

// ── 3. SVG open paths and group labels ──────────────────────────────────

#[test]
fn an_svg_keeps_its_open_paths_and_group_labels() {
    let model = load(
        &format!("{SELECTION_FIXTURES}/groups.svg"),
        ModelUnits::Millimeters,
    );
    let with = |kind: ItemKind, layer: Option<&str>| {
        model
            .items
            .iter()
            .filter(|i| i.kind() == kind && i.layer() == layer)
            .count()
    };
    assert_eq!(
        with(ItemKind::Path, None),
        1,
        "the top-level open path is kept"
    );
    assert_eq!(
        with(ItemKind::Path, Some("pocket")),
        1,
        "an open subpath after a closed one is kept, with its group label"
    );
    assert_eq!(
        with(ItemKind::Circle, Some("holes")),
        2,
        "{:#?}",
        model.items
    );
    assert_eq!(with(ItemKind::Loop, Some("outline")), 1);
    assert_eq!(
        with(ItemKind::Loop, Some("pocket")),
        2,
        "a group without an id takes the label of its parent group"
    );
    assert_eq!(
        with(ItemKind::Loop, None),
        1,
        "the triangle outside a group"
    );
    assert_eq!(model.items.len(), 8);
}

// ── 4. STEP face normal sign ────────────────────────────────────────────

/// A flat square face at `z`, wound so that its triangles face `up`. The
/// plane normal is +Z for both faces, as the STEP door sets it for a plane
/// at Z >= 0: the item must not read it.
fn square_face(z: f64, up: bool) -> FaceTessellation {
    let v = vec![
        P3::new(0.0, 0.0, z),
        P3::new(10.0, 0.0, z),
        P3::new(10.0, 10.0, z),
        P3::new(0.0, 10.0, z),
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
            normal: V3::new(0.0, 0.0, 1.0),
            d: z,
        },
    }
}

#[test]
fn a_face_item_carries_the_sign_of_its_face() {
    let mesh = build_enriched_mesh(
        vec![square_face(5.0, true), square_face(0.0, false)],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let items = brep_face_items(&mesh);
    let faces: Vec<_> = items
        .iter()
        .map(|i| match i.shape() {
            Shape::Face(face) => face.clone(),
            other => panic!("a STEP item is a face, got {other:?}"),
        })
        .collect();
    assert_eq!(faces.len(), 2);
    assert!(
        (faces[0].normal.z - 1.0).abs() < 1e-12,
        "{:?}",
        faces[0].normal
    );
    assert!((faces[0].plane_z - 5.0).abs() < 1e-12);
    assert!(
        (faces[1].normal.z + 1.0).abs() < 1e-12,
        "{:?}",
        faces[1].normal
    );
    assert!(faces[1].plane_z.abs() < 1e-12);
    // The key has no Z (DESIGN 9.3): the two squares differ by BREP id only.
    assert_eq!(items.iter().next().unwrap().key().anchor, [5.0, 5.0]);
}

#[cfg(feature = "step")]
#[test]
fn a_step_block_has_an_up_top_and_a_down_bottom() {
    let model = load(
        "fixtures/gui_step/block_40x40x60.step",
        ModelUnits::Millimeters,
    );
    let bbox = model.bbox().unwrap();
    let faces: Vec<_> = model
        .items
        .iter()
        .filter_map(|i| match i.shape() {
            Shape::Face(face) => Some(face.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(faces.len(), 2, "the top and the bottom are the flat faces");
    let top = faces
        .iter()
        .find(|f| (f.plane_z - bbox.max.z).abs() < 1e-6)
        .expect("a face at the top");
    let bottom = faces
        .iter()
        .find(|f| (f.plane_z - bbox.min.z).abs() < 1e-6)
        .expect("a face at the bottom");
    assert!(top.normal.z > 0.99, "the top faces up: {:?}", top.normal);
    assert!(
        bottom.normal.z < -0.99,
        "the bottom faces down: {:?}",
        bottom.normal
    );
    assert!(!model.items.has_drawing(), "a STEP model is not a drawing");
    assert!(model.outlines_pre_selection().is_none());
}

// ── 5. The bridges equal the lists before P2.1 ──────────────────────────

/// The canonical dump of the two bridge lists and the bbox. The text is the
/// format of the probe that recorded the baseline; change nothing here.
///
/// One designed change (P2.3a, lead ruling "fix b"): `detect_containment`
/// no longer reverses an open path whose shoelace area is negative, so an
/// open path keeps its drawn direction. The dump writes an open path in
/// the direction of the import before P2.1 (the same rule, over a list of
/// two or more polygons), so the baseline holds every other bit.
fn dump(m: &LoadedModel) -> String {
    let mut out = String::new();
    match m.outlines_pre_selection() {
        None => writeln!(out, " polygons None").unwrap(),
        Some(ps) => {
            writeln!(out, " polygons {}", ps.len()).unwrap();
            for p in ps.iter() {
                write!(out, "  P closed={} n={} :", p.closed, p.exterior.len()).unwrap();
                let reversed_before_fix_b = !p.closed && ps.len() > 1 && p.signed_area() < 0.0;
                let exterior: Vec<_> = if reversed_before_fix_b {
                    p.exterior.iter().rev().collect()
                } else {
                    p.exterior.iter().collect()
                };
                for q in exterior {
                    write!(out, " {:x},{:x}", q.x.to_bits(), q.y.to_bits()).unwrap();
                }
                writeln!(out).unwrap();
                for h in &p.holes {
                    write!(out, "   H n={} :", h.len()).unwrap();
                    for q in h {
                        write!(out, " {:x},{:x}", q.x.to_bits(), q.y.to_bits()).unwrap();
                    }
                    writeln!(out).unwrap();
                }
            }
        }
    }
    for (x, y, layer, diameter) in common::session::old_drill_targets(m) {
        let d = match diameter {
            None => "point".to_owned(),
            Some(diameter) => format!("circle {:?}", diameter.to_bits()),
        };
        writeln!(out, "  T {:?} {:?} {layer} {d}", x.to_bits(), y.to_bits()).unwrap();
    }
    match m.bbox() {
        None => writeln!(out, " bbox None").unwrap(),
        Some(b) => writeln!(
            out,
            " bbox {:x} {:x} {:x} {:x} {:x} {:x}",
            b.min.x.to_bits(),
            b.min.y.to_bits(),
            b.min.z.to_bits(),
            b.max.x.to_bits(),
            b.max.y.to_bits(),
            b.max.z.to_bits()
        )
        .unwrap(),
    }
    out
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// The baseline line of one model.
fn line(label: &str, m: &LoadedModel) -> String {
    let polygons = m
        .outlines_pre_selection()
        .map_or_else(|| "None".to_owned(), |p| p.len().to_string());
    format!(
        "{label} | polygons {polygons} | targets {} | fnv {:016x}",
        common::session::old_drill_targets(m).len(),
        fnv1a(dump(m).as_bytes())
    )
}

#[test]
fn the_bridges_give_the_lists_of_the_import_before_p2_1() {
    let baseline = std::fs::read_to_string(
        repo().join(format!("{SELECTION_FIXTURES}/bridges_before_p2_1.txt")),
    )
    .unwrap();
    let mut checked = 0;
    for expected in baseline.lines().filter(|l| !l.starts_with('#')) {
        let label = expected.split(" | ").next().unwrap();
        let model = if let Some((rel, units)) = label.rsplit_once(' ') {
            let units = match units {
                "Millimeters" => ModelUnits::Millimeters,
                "Inches" => ModelUnits::Inches,
                other => panic!("units {other} in the baseline"),
            };
            let kind = infer_kind_from_path(Path::new(rel)).unwrap();
            if kind == ModelKind::Step && !cfg!(feature = "step") {
                continue;
            }
            load(rel, units)
        } else {
            use common::op_fixtures as f;
            match label {
                "outline_model" => f::outline_model(),
                "curve_model" => f::curve_model(),
                "flat_prior_model" => f::flat_prior_model(),
                other => panic!("unknown in-test drawing {other}"),
            }
        };
        assert_eq!(line(label, &model), expected, "the bridge moved");
        checked += 1;
    }
    // Non-vacuity: 20 files and 4 synthetic files at two units (less the
    // 2 entries of the designed change), plus 3 in-test drawings; without
    // the `step` feature the 8 STEP files (16 entries) are skipped. P2.2
    // removed the planning copy of `fixtures/demo_star.svg` (the bytes are
    // equal), so a planning edit cannot move this sentry.
    let want = if cfg!(feature = "step") { 49 } else { 33 };
    assert_eq!(checked, want, "the baseline lines that ran");
}

/// DESIGN 2.2: a closed polyline of 2 points was dropped in silence. It is
/// a 2-point path now, and the bridge gives it as an open path after the
/// lists of the old import.
#[test]
fn a_closed_two_point_polyline_is_kept_as_a_path() {
    let model = load(
        &format!("{SELECTION_FIXTURES}/degenerate.dxf"),
        ModelUnits::Millimeters,
    );
    let paths: Vec<_> = model
        .items
        .iter()
        .filter(|i| i.kind() == ItemKind::Path)
        .collect();
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].layer(), Some("THIN"));
    let outlines = model.outlines_pre_selection().unwrap();
    assert_eq!(outlines.len(), 2, "the rectangle and the 2-point path");
    assert!(!outlines[1].closed && outlines[1].exterior.len() == 2);
}
