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
//! 5. Operator ruling O-2: the SVG door applies every transform and honours
//!    the declared size. The five P4.0 probe rows import at the size the CSS
//!    rules give (96 px = 1 in = 25.4 mm; a unitless length is px), the
//!    declared size reads what each file declares, and a user scale
//!    multiplies after.
//! 6. A circle under a uniform scale with a rotation stays a `Circle` with
//!    the scaled diameter; under a non-uniform scale or a skew it is a loop.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits};
use rs_cam_core::geo::P2;
use rs_cam_core::io::svg_input::{SVG_DEFAULT_UNITS, load_svg_data, svg_declared_size};
use rs_cam_core::io::{ImportOptions, default_import_units, load_model_file};
use rs_cam_core::selection::join::{CLOSE_TOL_MM, JoinCounts};
use rs_cam_core::selection::{ItemKind, Shape};
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
const JOIN_SVG: &[u8] =
    br#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100">
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

// ── 5. The SVG door honours the declared size (O-2, the P4.0 rows) ────

/// mm per CSS px, written here from the CSS absolute units (96 px = 1 in =
/// 25.4 mm), not read from the door: a wrong door constant goes red.
const MM_PER_PX: f64 = 25.4 / 96.0;

/// The five P4.0 probe rows: the input, the declared width and height in mm
/// (`None` = no absolute size), the declared viewBox, and the size and
/// origin of the import at the default scale.
struct ProbeRow {
    svg: String,
    declared_mm: (Option<f64>, Option<f64>),
    view_box: Option<[f64; 4]>,
    size: (f64, f64),
    origin: (f64, f64),
}

const RECT: &str = r#"<rect x="0" y="0" width="100" height="50"/>"#;

fn svg(attrs: &str, body: &str) -> String {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" {attrs}>{body}</svg>"#)
}

fn probe_rows() -> Vec<ProbeRow> {
    let row = |svg, declared_mm, view_box, size, origin| ProbeRow {
        svg,
        declared_mm,
        view_box,
        size,
        origin,
    };
    // Row 5: viewport 100 x 50 mm, viewBox 377.95 x 188.98 user units.
    // sx = 100 / 377.95 = 0.2645852, sy = 50 / 188.98 = 0.2645782. The
    // default `xMidYMid meet` takes s = min(sx, sy) = 50 / 188.98. The rect
    // fills the viewBox: 377.95 s = 99.9974 mm wide, 188.98 s = 50 mm high,
    // and the free 100 - 99.9974 = 0.0026 mm splits in half to the left.
    // So the row is 100 x 50 within the 0.003 mm that the rounded viewBox
    // leaves.
    let s5 = (100.0 / 377.95_f64).min(50.0 / 188.98);
    vec![
        // Row 1: viewBox 100 x 50 on 100 x 50 mm: 1 user unit = 1 mm.
        row(
            svg(r#"width="100mm" height="50mm" viewBox="0 0 100 50""#, RECT),
            (Some(100.0), Some(50.0)),
            Some([0.0, 0.0, 100.0, 50.0]),
            (100.0, 50.0),
            (0.0, 0.0),
        ),
        // Row 2: a unitless length is px (SVG spec). No viewBox: 1 user
        // unit = 1 px. 100 px x 25.4 / 96 = 26.458 mm; 50 px = 13.229 mm.
        row(
            svg(r#"width="100" height="50""#, RECT),
            (Some(100.0 * MM_PER_PX), Some(50.0 * MM_PER_PX)),
            None,
            (100.0 * MM_PER_PX, 50.0 * MM_PER_PX),
            (0.0, 0.0),
        ),
        // Row 3: the viewport is 100 x 50 mm, but with no viewBox 1 user
        // unit is still 1 px. The rect is 100 x 50 px = 26.458 x 13.229 mm;
        // the declared size only sets the viewport (lead ruling C).
        row(
            svg(r#"width="100mm" height="50mm""#, RECT),
            (Some(100.0), Some(50.0)),
            None,
            (100.0 * MM_PER_PX, 50.0 * MM_PER_PX),
            (0.0, 0.0),
        ),
        // Row 4: as row 1 (1 user unit = 1 mm), in a group. scale(2) makes
        // the rect 200 x 100; translate(10,5) moves its corner to (10, 5).
        row(
            svg(
                r#"width="100mm" height="50mm" viewBox="0 0 100 50""#,
                &format!(r#"<g transform="translate(10,5) scale(2)">{RECT}</g>"#),
            ),
            (Some(100.0), Some(50.0)),
            Some([0.0, 0.0, 100.0, 50.0]),
            (200.0, 100.0),
            (10.0, 5.0),
        ),
        // Row 5: a px viewBox on a mm size (arithmetic above).
        row(
            svg(
                r#"width="100mm" height="50mm" viewBox="0 0 377.95 188.98""#,
                r#"<rect x="0" y="0" width="377.95" height="188.98"/>"#,
            ),
            (Some(100.0), Some(50.0)),
            Some([0.0, 0.0, 377.95, 188.98]),
            (377.95 * s5, 188.98 * s5),
            ((100.0 - 377.95 * s5) / 2.0, 0.0),
        ),
    ]
}

/// Size and origin of the import, 1e-4 mm: the path data of usvg is f32.
fn assert_bbox(
    import: &rs_cam_core::io::DrawingImport,
    size: (f64, f64),
    origin: (f64, f64),
    what: &str,
) {
    let [x0, y0, x1, y1] = import.items.bbox().unwrap();
    let close = |a: f64, b: f64| (a - b).abs() < 1e-4;
    assert!(
        close(x1 - x0, size.0) && close(y1 - y0, size.1),
        "{what}: the import is {} x {}, the CSS rules give {size:?}",
        x1 - x0,
        y1 - y0,
    );
    assert!(
        close(x0, origin.0) && close(y0, origin.1),
        "{what}: origin ({x0}, {y0}), the CSS rules give {origin:?}"
    );
}

#[test]
fn the_five_p4_0_rows_import_at_their_declared_size_o2() {
    assert_eq!(
        SVG_DEFAULT_UNITS,
        ModelUnits::Millimeters,
        "the default is 1:1 on the declared size"
    );
    assert_eq!(default_import_units(ModelKind::Svg), SVG_DEFAULT_UNITS);
    let rows = probe_rows();
    // Non-vacuity: row 5 is 100 x 50 within the rounding of its viewBox.
    assert!((rows[4].size.0 - 100.0).abs() < 0.003 && (rows[4].size.1 - 50.0).abs() < 1e-9);
    for row in rows {
        let svg = &row.svg;
        let declared = svg_declared_size(svg.as_bytes()).unwrap();
        let mm =
            |l: &Option<rs_cam_core::io::svg_input::DeclaredLength>| l.as_ref().and_then(|l| l.mm);
        let (w, h) = (mm(&declared.width), mm(&declared.height));
        let close = |a: Option<f64>, b: Option<f64>| match (a, b) {
            (Some(a), Some(b)) => (a - b).abs() < 1e-9,
            (a, b) => a == b,
        };
        assert!(
            close(w, row.declared_mm.0) && close(h, row.declared_mm.1),
            "{svg}: declared {w:?} x {h:?}"
        );
        assert_eq!(declared.view_box, row.view_box, "{svg}");
        // The dialog's "File size" is the viewport.
        let (vw, vh) = declared.viewport_mm().unwrap();
        assert!(
            close(Some(vw), row.declared_mm.0) && close(Some(vh), row.declared_mm.1),
            "{svg}: viewport {vw} x {vh}"
        );

        let import = load_svg_data(
            svg.as_bytes(),
            0.1,
            SVG_DEFAULT_UNITS.scale_factor(),
            CLOSE_TOL_MM,
        )
        .unwrap();
        assert_bbox(&import, row.size, row.origin, svg);
    }
    // A user scale multiplies after the declared size: row 1 in inches.
    let inches = load_svg_data(
        probe_rows()[0].svg.as_bytes(),
        0.1,
        ModelUnits::Inches.scale_factor(),
        CLOSE_TOL_MM,
    )
    .unwrap();
    assert_bbox(
        &inches,
        (100.0 * 25.4, 50.0 * 25.4),
        (0.0, 0.0),
        "row 1 in inches",
    );
    // The readout text keeps the units as written.
    let declared = svg_declared_size(probe_rows()[0].svg.as_bytes()).unwrap();
    assert_eq!(
        declared.summary_line(),
        r#"width="100mm" height="50mm" viewBox="0 0 100 50""#
    );
    // For an ordinary file the "File size" and the "Result" agree.
    let import = load_svg_data(probe_rows()[0].svg.as_bytes(), 0.1, 1.0, CLOSE_TOL_MM).unwrap();
    let [x0, y0, x1, y1] = import.items.bbox().unwrap();
    assert_eq!(declared.viewport_mm(), Some((x1 - x0, y1 - y0)));
}

// ── 6. A circle under a transform (O-2) ─────────────────────────────────

/// A 1:1 file (1 user unit = 1 mm) with one circle of radius 5 at (20, 10)
/// under the group transform `transform`.
fn circle_in_group(transform: &str) -> String {
    svg(
        r#"width="100mm" height="100mm" viewBox="0 0 100 100""#,
        &format!(r#"<g transform="{transform}"><circle cx="20" cy="10" r="5"/></g>"#),
    )
}

fn circles(svg: &str) -> Vec<(P2, f64)> {
    load_svg_data(svg.as_bytes(), 0.1, 1.0, CLOSE_TOL_MM)
        .unwrap()
        .items
        .iter()
        .filter_map(|i| match i.shape() {
            Shape::Circle {
                centre, diameter, ..
            } => Some((*centre, *diameter)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_circle_under_a_uniform_scale_stays_a_circle_o2() {
    // Control: with no transform the circle is a Circle of diameter 10.
    let plain = circles(&circle_in_group("translate(0,0)"));
    assert_eq!(plain.len(), 1, "{plain:?}");
    assert!((plain[0].1 - 10.0).abs() < 0.02, "{plain:?}");
    // rotate(30) after scale(2): the diameter is 2 x 2 x 5 = 20. The centre
    // (20, 10) scales to (40, 20) and turns by 30 degrees:
    // x = 40 cos 30 - 20 sin 30 = 34.641 - 10 = 24.641,
    // y = 40 sin 30 + 20 cos 30 = 20 + 17.321 = 37.321.
    let turned = circles(&circle_in_group("rotate(30) scale(2)"));
    assert_eq!(
        turned.len(),
        1,
        "a uniform scale with a rotation keeps the Circle"
    );
    let (centre, diameter) = turned[0];
    let (s, c) = 30_f64.to_radians().sin_cos();
    let (ex, ey) = (40.0 * c - 20.0 * s, 40.0 * s + 20.0 * c);
    assert!((diameter - 20.0).abs() < 0.02, "diameter {diameter}");
    assert!(
        (centre.x - ex).abs() < 0.01 && (centre.y - ey).abs() < 0.01,
        "centre {centre:?}, expected ({ex}, {ey})"
    );
}

#[test]
fn a_circle_under_a_non_uniform_scale_becomes_a_loop_o2() {
    // scale(2,1) makes the r = 5 circle an ellipse with semi-axes 10 and 5;
    // skewX(30) moves the top by tan 30 x y. Each radius spread is far
    // above the 2 % circle rule, so each is a closed Loop.
    for transform in ["scale(2,1)", "skewX(30)"] {
        let svg = circle_in_group(transform);
        assert!(circles(&svg).is_empty(), "{transform}: not a Circle");
        let items = load_svg_data(svg.as_bytes(), 0.1, 1.0, CLOSE_TOL_MM)
            .unwrap()
            .items;
        let kinds: Vec<ItemKind> = items.iter().map(|i| i.kind()).collect();
        assert_eq!(kinds, [ItemKind::Loop], "{transform}");
        if transform == "scale(2,1)" {
            // The ellipse spans 2 x 2 x 5 = 20 in x and 2 x 5 = 10 in y.
            let [x0, y0, x1, y1] = items.bbox().unwrap();
            assert!((x1 - x0 - 20.0).abs() < 0.05 && (y1 - y0 - 10.0).abs() < 0.05);
        }
    }
}
