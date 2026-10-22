//! **G-UNITSRELOAD** — a 2D model's unit scale is applied on import and
//! dropped on reload.
//!
//! # The defect
//!
//! `ModelUnits` is a per-model scale factor (`Inches` = 25.4, `Meters` =
//! 1000, …). A project file stores the model's **source path** plus its
//! declared units — geometry is not embedded, it is re-imported from the
//! source file every load. So both load paths must apply the same scale to
//! the same file. They do not:
//!
//! | path | STL | SVG | DXF polygons | DXF drill targets |
//! |---|---|---|---|---|
//! | interactive import — `io::load_model_file` | scaled | scaled | scaled | scaled |
//! | project load — `session::project_file::load_model_geometry` | scaled | **dropped** | **dropped** | **dropped** |
//!
//! `load_model_geometry` computes `let scale = model.units…scale_factor()`
//! and then passes it only to `TriangleMesh::from_stl_scaled`. The SVG and
//! DXF arms never see it. `save.rs:144` writes `units: m.units` — the
//! *original* declared units — so the information is on disk and simply not
//! consumed.
//!
//! # Why it is worse than a wrong number
//!
//! It is silent, and it only appears after a save/reload cycle, so the
//! session that authored the job sees correct geometry throughout. An
//! inch-authored DXF comes back **25.4× smaller** while the stock keeps the
//! dimensions saved beside it — and `StockConfig::update_from_bbox` runs on
//! import, not on load, so nothing re-fits and nothing complains. The
//! toolpaths regenerate cleanly around a part that is now a fortieth of its
//! intended size.
//!
//! This is the third divergence found in this pair of loaders. The previous
//! two were closed 2026-06-08 with "nothing left to consolidate"; that was
//! true of the divergences then known, and this file is the reason the claim
//! needs a test rather than a note.
//!
//! # What is pinned
//!
//! Equivalence, not a magic number: the same file at the same declared units
//! must produce the same geometry through both doors. Written that way
//! deliberately — a test asserting "the bbox is 254 mm wide" would pass just
//! as well if both paths were wrong together.

#![expect(
    clippy::expect_used,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::{Path, PathBuf};

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::io::{ImportOptions, load_model_file};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::ItemKind;
use rs_cam_core::session::LoadedModel;

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is crates/rs_cam_core.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root is two levels above the crate manifest")
        .to_path_buf()
}

fn dxf_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rivers_aligned.dxf")
}

fn svg_fixture() -> PathBuf {
    repo_root().join("fixtures/demo_star.svg")
}

/// Total XY extent of a polygon set — a single scalar that moves linearly
/// with any uniform scale, so a ratio between two loads is the scale factor
/// one applied and the other did not.
fn extent(polygons: &[Polygon2]) -> (f64, f64) {
    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for poly in polygons {
        for p in poly.exterior.iter().chain(poly.holes.iter().flatten()) {
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
        }
    }
    (max_x - min_x, max_y - min_y)
}

/// Load the fixture through the PROJECT-FILE door, at the given units, by
/// building a minimal project that references it.
fn via_project_file(path: &Path, kind: ModelKind, units: ModelUnits) -> Vec<Polygon2> {
    via_project_file_model(path, kind, units, None)
        .outlines_pre_selection()
        .expect("a 2D model must carry polygons")
        .as_ref()
        .clone()
}

/// The model the PROJECT-FILE door loads, with an optional
/// `[models.import]` join tolerance (P4.8).
fn via_project_file_model(
    path: &Path,
    kind: ModelKind,
    units: ModelUnits,
    join_tolerance_mm: Option<f64>,
) -> LoadedModel {
    let import = join_tolerance_mm
        .map(|t| format!("\n[models.import]\njoin_tolerance_mm = {t}\n"))
        .unwrap_or_default();
    let toml = format!(
        r#"
format_version = 3

[[models]]
id = 0
path = "{}"
name = "fixture"
kind = "{}"
units = {{ kind = "{}" }}
{import}"#,
        path.display(),
        match kind {
            ModelKind::Dxf => "dxf",
            ModelKind::Svg => "svg",
            _ => panic!("this fixture only covers 2D kinds"),
        },
        match units {
            ModelUnits::Millimeters => "millimeters",
            ModelUnits::Inches => "inches",
            ModelUnits::Meters => "meters",
            ModelUnits::Centimeters => "centimeters",
            ModelUnits::Custom(_) => panic!("custom units are not part of this fixture"),
        },
    );
    let dir = std::env::temp_dir().join(format!(
        "rs_cam_units_reload_{}_{:?}_{:?}",
        std::process::id(),
        units,
        join_tolerance_mm
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let project_path = dir.join("project.toml");
    std::fs::write(&project_path, toml).expect("write project");

    let session = rs_cam_core::session::ProjectSession::load(&project_path)
        .expect("the fixture project must load");
    let model = session.models().first().expect("one model").clone();
    let _ = std::fs::remove_dir_all(&dir);
    model
}

/// Non-vacuity: both doors must actually return geometry, or the equivalence
/// below is a comparison of two empty sets.
#[test]
fn both_load_paths_return_geometry() {
    let cases: [(PathBuf, ModelKind); 2] = [
        (dxf_fixture(), ModelKind::Dxf),
        (svg_fixture(), ModelKind::Svg),
    ];
    for (path, kind) in cases {
        assert!(path.exists(), "fixture missing: {}", path.display());
        let direct = load_model_file(
            &path,
            rs_cam_core::ids::ModelId::from_raw(0),
            kind,
            ModelUnits::Millimeters,
            Default::default(),
        )
        .expect("interactive import must succeed");
        let direct_outlines = direct.outlines_pre_selection().expect("polygons");
        let direct_polys = direct_outlines.as_ref();
        assert!(
            !direct_polys.is_empty(),
            "{} produced no polygons through the import door",
            path.display()
        );
        let (w, h) = extent(direct_polys);
        assert!(
            w > 0.0 && h > 0.0,
            "{} has zero extent ({w} x {h}); the ratio test below would be undefined",
            path.display()
        );
    }
}

/// The equivalence. Same file, same declared units, two doors, one geometry.
#[test]
fn the_two_load_paths_agree_on_scale() {
    let cases: [(PathBuf, ModelKind); 2] = [
        (dxf_fixture(), ModelKind::Dxf),
        (svg_fixture(), ModelKind::Svg),
    ];
    for (path, kind) in cases {
        for units in [ModelUnits::Millimeters, ModelUnits::Inches] {
            let direct = load_model_file(
                &path,
                rs_cam_core::ids::ModelId::from_raw(0),
                kind,
                units,
                Default::default(),
            )
            .expect("interactive import must succeed");
            let direct_outlines = direct.outlines_pre_selection().expect("polygons");
            let direct_polys = direct_outlines.as_ref();
            let loaded = via_project_file(&path, kind, units);

            let (dw, dh) = extent(direct_polys);
            let (lw, lh) = extent(&loaded);

            assert!(
                (dw - lw).abs() < 1e-6 && (dh - lh).abs() < 1e-6,
                "{} at {units:?}: the two load paths disagree on scale.\n  \
                 import door: {dw:.4} x {dh:.4} mm\n  \
                 project door: {lw:.4} x {lh:.4} mm\n  \
                 ratio: {:.4} x {:.4} (expected 1.0)\n  \
                 `load_model_geometry` computes the units scale and applies it \
                 only to STL; the SVG and DXF arms drop it. A project saved \
                 after import therefore reloads at a different size, silently.",
                path.display(),
                if lw != 0.0 { dw / lw } else { f64::NAN },
                if lh != 0.0 { dh / lh } else { f64::NAN },
            );
        }
    }
}

/// And the units dial is not inert on either door — if `Inches` produced the
/// same geometry as `Millimeters`, the test above would pass trivially.
#[test]
fn the_units_dial_actually_scales() {
    let path = dxf_fixture();
    let mm = load_model_file(
        &path,
        rs_cam_core::ids::ModelId::from_raw(0),
        ModelKind::Dxf,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .expect("mm load");
    let inch = load_model_file(
        &path,
        rs_cam_core::ids::ModelId::from_raw(0),
        ModelKind::Dxf,
        ModelUnits::Inches,
        Default::default(),
    )
    .expect("inch load");
    let (mw, _) = extent(&mm.outlines_pre_selection().expect("polygons"));
    let (iw, _) = extent(&inch.outlines_pre_selection().expect("polygons"));
    assert!(
        (iw / mw - 25.4).abs() < 1e-6,
        "Inches must scale by 25.4, got {:.6} ({mw:.4} -> {iw:.4})",
        iw / mw
    );
}

/// P4.8: the join tolerance is an import option, so it must reach both
/// doors as the units do. The same file at the same units and tolerance
/// gives the same items through the import door and the project door, at
/// mm and at inches. Non-vacuity: the wide tolerance joins pieces that the
/// default leaves apart.
#[test]
fn the_two_load_paths_agree_on_the_join_tolerance() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/selection_p2_1/mix.dxf");
    let paths = |m: &LoadedModel| {
        m.items
            .iter()
            .filter(|i| i.kind() == ItemKind::Path)
            .count()
    };
    let wide = ImportOptions {
        join_tolerance_mm: 50.0,
    };
    for units in [ModelUnits::Millimeters, ModelUnits::Inches] {
        let default = load_model_file(
            &path,
            rs_cam_core::ids::ModelId::from_raw(0),
            ModelKind::Dxf,
            units,
            ImportOptions::default(),
        )
        .expect("default load");
        let direct = load_model_file(
            &path,
            rs_cam_core::ids::ModelId::from_raw(0),
            ModelKind::Dxf,
            units,
            wide,
        )
        .expect("wide load");
        assert!(
            paths(&direct) < paths(&default),
            "the wide tolerance must join pieces ({} vs {})",
            paths(&direct),
            paths(&default)
        );
        let loaded = via_project_file_model(&path, ModelKind::Dxf, units, Some(50.0));
        assert_eq!(
            loaded.import_options, wide,
            "the project door reads the option"
        );
        let keys = |m: &LoadedModel| {
            m.items
                .iter()
                .map(|i| format!("{:?}", i.key()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            keys(&loaded),
            keys(&direct),
            "{units:?}: the two doors disagree on the join"
        );
        assert_eq!(loaded.import_report, direct.import_report);
    }
}
