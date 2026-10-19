//! P4.8 — import options and the import report (geometry selection,
//! `planning/geometry_selection_2026-10-16/P4_DESIGN.md`, "P4.8").
//!
//! The claims:
//!
//! 1. A drawing import reports the join of its open pieces per layer: the
//!    pieces, the shapes, the joined shapes, the closed shapes and the open
//!    shapes left (the P2.1 DXF fixture `mix.dxf`).
//! 2. The join tolerance is an import option. 0.5 mm joins a 0.3 mm gap;
//!    the 0.01 mm default does not. The DXF and the SVG door use the same
//!    rule (ruling O-5).
//! 3. The SVG door joins open subpaths whose ends meet within the
//!    tolerance. Before P4.8 it did not join.
//! 4. The options go into the project file with the model, only when they
//!    are not the default, and a reload reads the file with them: the same
//!    items, the same report. An old file with no key loads the defaults.
//! 5. The SVG default scale does not change (O-2 is open): the five P4.0
//!    probe rows import at the sizes the probe measured, and the declared
//!    size reads what each file declares.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::io::svg_input::{
    SVG_DEFAULT_UNITS, load_svg_data, svg_declared_size, svg_default_units,
};
use rs_cam_core::io::{ImportOptions, load_model_file};
use rs_cam_core::selection::ItemKind;
use rs_cam_core::selection::join::{CLOSE_TOL_MM, JoinCounts};
use rs_cam_core::session::{LoadedModel, ProjectSession};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn scratch(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("rs_cam_import_p4_8_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn options(join_tolerance_mm: f64) -> ImportOptions {
    ImportOptions { join_tolerance_mm }
}

fn counts(
    pieces: usize,
    shapes: usize,
    joined: usize,
    closed_by_join: usize,
    open_left: usize,
) -> JoinCounts {
    JoinCounts {
        pieces,
        shapes,
        joined,
        closed_by_join,
        open_left,
    }
}

fn count(model: &LoadedModel, kind: ItemKind) -> usize {
    model.items.iter().filter(|i| i.kind() == kind).count()
}

/// A DXF of two LINE pieces on layer `0` with a gap of `gap` mm between the
/// end of the first and the start of the second, and a third LINE that
/// meets the second exactly.
fn gap_dxf(dir: &Path, gap: f64) -> PathBuf {
    let mut drawing = dxf::Drawing::new();
    for (x1, y1, x2, y2) in [
        (0.0, 0.0, 10.0, 0.0),
        (10.0 + gap, 0.0, 20.0, 0.0),
        (20.0, 0.0, 20.0, 10.0),
    ] {
        drawing.add_entity(dxf::entities::Entity::new(dxf::entities::EntityType::Line(
            dxf::entities::Line {
                p1: dxf::Point::new(x1, y1, 0.0),
                p2: dxf::Point::new(x2, y2, 0.0),
                ..Default::default()
            },
        )));
    }
    let path = dir.join("gap.dxf");
    drawing
        .save_file(path.to_str().unwrap())
        .expect("write the DXF");
    path
}

// ── 1. The report of the P1b fixture ────────────────────────────────────

/// `mix.dxf` (P2.1 fixture): 10 open pieces (4 LINES, 3 TEXT, 1 each on
/// B, 0 and SPL) chain to 1 loop (the LINES square) and 5 paths, and a
/// chain never crosses a layer.
#[test]
fn the_report_counts_the_join_per_layer() {
    let path = repo().join("crates/rs_cam_core/tests/fixtures/selection_p2_1/mix.dxf");
    let model = load_model_file(
        &path,
        0,
        ModelKind::Dxf,
        ModelUnits::Millimeters,
        ImportOptions::default(),
    )
    .unwrap();
    let report = model
        .import_report
        .as_ref()
        .expect("a drawing has a report");
    assert_eq!(report.join_tolerance_mm, CLOSE_TOL_MM);
    assert_eq!(
        report.total(),
        counts(10, 6, 2, 1, 5),
        "10 pieces -> 1 loop and 5 paths: {report:#?}"
    );
    // The totals agree with the items.
    assert_eq!(report.total().open_left, count(&model, ItemKind::Path));
    let row = |layer: &str| {
        report
            .layers
            .iter()
            .find(|r| r.layer.as_deref() == Some(layer))
            .unwrap_or_else(|| panic!("no row for {layer}: {report:#?}"))
            .counts
    };
    assert_eq!(row("LINES"), counts(4, 1, 1, 1, 0), "the LINE square");
    let paths: usize = report.layers.iter().map(|r| r.counts.open_left).sum();
    assert_eq!(paths, 5);
    assert_eq!(row("B").open_left, 1, "a chain does not cross a layer");
    assert_eq!(row("TEXT"), counts(3, 2, 1, 0, 2), "two TEXT pieces join");
    assert_eq!(
        report.summary_line(),
        "10 open pieces \u{2192} 6 shapes \u{b7} 2 joined \u{b7} 1 closed \u{b7} 5 open left"
    );
}

/// A mesh has no join, so no report.
#[test]
fn a_mesh_has_no_report() {
    let path = repo().join("crates/rs_cam_core/tests/fixtures/terrain.stl");
    let model = load_model_file(
        &path,
        0,
        ModelKind::Stl,
        ModelUnits::Millimeters,
        ImportOptions::default(),
    )
    .unwrap();
    assert!(model.import_report.is_none());
}

// ── 2. The tolerance is an option (DXF) ─────────────────────────────────

#[test]
fn a_wider_tolerance_joins_a_dxf_gap() {
    let dir = scratch("dxf_gap");
    let path = gap_dxf(&dir, 0.3);
    let load = |tol: f64| {
        load_model_file(
            &path,
            0,
            ModelKind::Dxf,
            ModelUnits::Millimeters,
            options(tol),
        )
        .unwrap()
    };
    let default = load(CLOSE_TOL_MM);
    assert_eq!(
        count(&default, ItemKind::Path),
        2,
        "0.01 mm does not join 0.3"
    );
    assert_eq!(
        default.import_report.as_ref().unwrap().total(),
        counts(3, 2, 1, 0, 2)
    );
    let wide = load(0.5);
    assert_eq!(count(&wide, ItemKind::Path), 1, "0.5 mm joins 0.3");
    assert_eq!(
        wide.import_report.as_ref().unwrap().total(),
        counts(3, 1, 1, 0, 1)
    );
    assert_eq!(
        wide.import_options,
        options(0.5),
        "the model keeps its options"
    );
    // A tolerance no import can use refuses.
    let refused = load_model_file(
        &path,
        0,
        ModelKind::Dxf,
        ModelUnits::Millimeters,
        options(0.0),
    );
    assert!(refused.is_err(), "a zero tolerance refuses");
    let _ = std::fs::remove_dir_all(&dir);
}

// ── 3. The SVG door joins (O-5) ─────────────────────────────────────────

/// Two open subpaths of one group meet end to end and close a square; a
/// third, in another group, meets them but does not join (D14); a fourth
/// stops 0.3 short of the third. Before P4.8 the door gave four paths.
const JOIN_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
    <g id="A">
        <path d="M 0 0 L 10 0 L 10 10"/>
        <path d="M 10 10 L 0 10 L 0 0.004"/>
    </g>
    <g id="B">
        <path d="M 10 10 L 30 10"/>
        <path d="M 30.3 10 L 40 10"/>
    </g>
</svg>"#;

#[test]
fn svg_open_subpaths_join_within_the_tolerance() {
    let at = |tol: f64| load_svg_data(JOIN_SVG, 0.1, 1.0, tol).unwrap();
    let default = at(CLOSE_TOL_MM);
    let kinds: Vec<(ItemKind, Option<&str>)> = default
        .items
        .iter()
        .map(|i| (i.kind(), i.layer()))
        .collect();
    assert_eq!(
        kinds,
        [
            (ItemKind::Loop, Some("A")),
            (ItemKind::Path, Some("B")),
            (ItemKind::Path, Some("B")),
        ],
        "the A pieces close; B does not join A, and its 0.3 gap stays"
    );
    let a = &default.report.layers[0];
    assert_eq!(a.layer.as_deref(), Some("A"));
    assert_eq!(a.counts, counts(2, 1, 1, 1, 0));
    // 0.5 joins the B gap too: one path on B.
    let wide = at(0.5);
    assert_eq!(wide.items.len(), 2);
    assert_eq!(wide.report.layers[1].counts, counts(2, 1, 1, 0, 1));
    // The join runs in user units, before the scale: at scale 2 the same
    // decisions, at twice the size.
    let scaled = load_svg_data(JOIN_SVG, 0.1, 2.0, CLOSE_TOL_MM).unwrap();
    assert_eq!(scaled.report, default.report);
    let [x0, _, x1, _] = scaled.items.bbox().unwrap();
    assert!((x1 - x0 - 80.0).abs() < 1e-9, "{x0} {x1}");
}

// ── 4. The options round-trip in the project file ───────────────────────

fn write_project(dir: &Path, model: &str, import: Option<f64>) -> PathBuf {
    let import = import
        .map(|t| format!("\n[models.import]\njoin_tolerance_mm = {t}\n"))
        .unwrap_or_default();
    let toml = format!(
        r#"format_version = 3
name = "p4_8"

[[models]]
id = 0
path = "{model}"
name = "drawing"
kind = "dxf"
units = {{ kind = "millimeters" }}
{import}"#
    );
    let path = dir.join("job.toml");
    std::fs::write(&path, toml).unwrap();
    path
}

fn keys(model: &LoadedModel) -> Vec<String> {
    model
        .items
        .iter()
        .map(|i| format!("{:?}", i.key()))
        .collect()
}

#[test]
fn the_import_options_round_trip_through_the_project_file() {
    let dir = scratch("round_trip");
    gap_dxf(&dir, 0.3);
    let job = write_project(&dir, "gap.dxf", Some(0.5));

    let session = ProjectSession::load(&job).expect("load");
    let model = &session.models()[0];
    assert_eq!(
        model.import_options,
        options(0.5),
        "the loader reads the key"
    );
    assert_eq!(count(model, ItemKind::Path), 1, "the load joins at 0.5");

    // The import door and the project door agree.
    let direct = load_model_file(
        &dir.join("gap.dxf"),
        0,
        ModelKind::Dxf,
        ModelUnits::Millimeters,
        options(0.5),
    )
    .unwrap();
    assert_eq!(keys(model), keys(&direct));
    assert_eq!(model.import_report, direct.import_report);

    session.save(&job).expect("save");
    let written = std::fs::read_to_string(&job).unwrap();
    assert!(
        written.contains("[models.import]") && written.contains("join_tolerance_mm = 0.5"),
        "the save writes the options:\n{written}"
    );
    let reloaded = ProjectSession::load(&job).expect("reload");
    let again = &reloaded.models()[0];
    assert_eq!(
        again.import_options,
        options(0.5),
        "a reload keeps the options"
    );
    assert_eq!(keys(again), keys(model), "a reload gives the same items");
    assert_eq!(again.import_report, model.import_report);
    let _ = std::fs::remove_dir_all(&dir);
}

/// An old file with no key loads the defaults (no shim, a serde default),
/// and a project with default options writes no key, so it saves as
/// before P4.8.
#[test]
fn a_file_with_no_options_loads_the_defaults_and_writes_no_key() {
    let dir = scratch("defaults");
    gap_dxf(&dir, 0.3);
    let job = write_project(&dir, "gap.dxf", None);
    let session = ProjectSession::load(&job).expect("load");
    assert_eq!(session.models()[0].import_options, ImportOptions::default());
    assert_eq!(count(&session.models()[0], ItemKind::Path), 2);
    session.save(&job).expect("save");
    let written = std::fs::read_to_string(&job).unwrap();
    assert!(!written.contains("import"), "no key:\n{written}");
    let _ = std::fs::remove_dir_all(&dir);
}

// ── 5. The SVG default scale and the declared size (P4.0 rows) ──────────

/// The five P4.0 probe rows: the input, the declared width and height in mm
/// (`None` = no physical unit), the declared viewBox, and the size and
/// origin of the import at the default scale.
struct ProbeRow {
    svg: String,
    declared_mm: (Option<f64>, Option<f64>),
    view_box: Option<[f64; 4]>,
    size: (f64, f64),
    origin: (f64, f64),
}

const RECT: &str = r#"<rect x="0" y="0" width="100" height="50"/>"#;

fn probe_rows() -> Vec<ProbeRow> {
    let svg = |attrs: &str, body: &str| {
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" {attrs}>{body}</svg>"#)
    };
    let row = |svg, declared_mm, view_box, size, origin| ProbeRow {
        svg,
        declared_mm,
        view_box,
        size,
        origin,
    };
    vec![
        row(
            svg(r#"width="100mm" height="50mm" viewBox="0 0 100 50""#, RECT),
            (Some(100.0), Some(50.0)),
            Some([0.0, 0.0, 100.0, 50.0]),
            (100.0, 50.0),
            (0.0, 0.0),
        ),
        row(
            svg(r#"width="100" height="50""#, RECT),
            (None, None),
            None,
            (100.0, 50.0),
            (0.0, 0.0),
        ),
        row(
            svg(r#"width="100mm" height="50mm""#, RECT),
            (Some(100.0), Some(50.0)),
            None,
            (100.0, 50.0),
            (0.0, 0.0),
        ),
        // The group transform is LOST today (P4.0): the size and the
        // origin of the untransformed rect. O-2 changes this row.
        row(
            svg(
                r#"width="100mm" height="50mm" viewBox="0 0 100 50""#,
                &format!(r#"<g transform="translate(10,5) scale(2)">{RECT}</g>"#),
            ),
            (Some(100.0), Some(50.0)),
            Some([0.0, 0.0, 100.0, 50.0]),
            (100.0, 50.0),
            (0.0, 0.0),
        ),
        // A px viewBox imports 3.78 times too large today. O-2 changes
        // this row.
        row(
            svg(
                r#"width="100mm" height="50mm" viewBox="0 0 377.95 188.98""#,
                r#"<rect x="0" y="0" width="377.95" height="188.98"/>"#,
            ),
            (Some(100.0), Some(50.0)),
            Some([0.0, 0.0, 377.95, 188.98]),
            (377.95, 188.98),
            (0.0, 0.0),
        ),
    ]
}

#[test]
fn the_five_p4_0_rows_keep_their_size_and_read_their_declared_size() {
    assert_eq!(SVG_DEFAULT_UNITS, ModelUnits::Millimeters, "O-2 is open");
    for row in probe_rows() {
        let svg = &row.svg;
        let declared = svg_declared_size(svg.as_bytes()).unwrap();
        assert_eq!(
            svg_default_units(&declared),
            ModelUnits::Millimeters,
            "the default scale stays 1:1 for every row: {svg}"
        );
        let mm =
            |l: &Option<rs_cam_core::io::svg_input::DeclaredLength>| l.as_ref().and_then(|l| l.mm);
        assert_eq!(
            (mm(&declared.width), mm(&declared.height)),
            row.declared_mm,
            "{svg}"
        );
        assert!(declared.width.is_some() && declared.height.is_some());
        assert_eq!(declared.view_box, row.view_box, "{svg}");

        let import = load_svg_data(
            svg.as_bytes(),
            0.1,
            SVG_DEFAULT_UNITS.scale_factor(),
            CLOSE_TOL_MM,
        )
        .unwrap();
        let [x0, y0, x1, y1] = import.items.bbox().unwrap();
        let close = |a: f64, b: f64| (a - b).abs() < 1e-3;
        assert!(
            close(x1 - x0, row.size.0) && close(y1 - y0, row.size.1),
            "{svg}: the import is {} x {}, the P4.0 probe measured {:?}",
            x1 - x0,
            y1 - y0,
            row.size
        );
        assert!(
            close(x0, row.origin.0) && close(y0, row.origin.1),
            "{svg}: origin ({x0}, {y0})"
        );
    }
    // The readout text keeps the units as written.
    let declared = svg_declared_size(probe_rows()[0].svg.as_bytes()).unwrap();
    assert_eq!(
        declared.summary_line(),
        r#"width="100mm" height="50mm" viewBox="0 0 100 50""#
    );
}
