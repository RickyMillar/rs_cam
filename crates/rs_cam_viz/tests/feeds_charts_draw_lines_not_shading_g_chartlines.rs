//! **G-CHARTLINES — the feeds charts draw lines, not shading, and draw a
//! band only when the vendor row publishes one.**
//!
//! # The ruling
//!
//! Operator, 2026-09-23 (feeds matrix `RULINGS.md`, second round): *"Always
//! just have the straight line, center or suggested, then add more faint
//! bands around it, no shading, but only if they exist".*
//!
//! So the nomogram draws ONE solid line at the suggested advance per tooth.
//! It draws two fainter lines at the vendor limits ONLY when the matched row
//! publishes both limits. It draws no filled region.
//!
//! # The defect this holds
//!
//! Since the R5 vendor-row fix, many flat-end pocket and adaptive rows
//! publish ONE value per size: a maximum, with no minimum. The chart then
//! made a band anyway, from `0.7 × max` to `max`, and shaded it. That lower
//! limit was invented. The row did not publish it, and Suggest showed no
//! band for the same row.
//!
//! # The arms
//!
//! - **Arm 1 (source).** The feeds chart files construct no `Polygon`, call
//!   no `.polygon(` and no `.fill_color(`, and `vendor_band` delegates to
//!   the core rule. A non-vacuity anchor comes first: `fn draw_chart_c`
//!   must exist. The scan ignores comments.
//! - **Arm 2 (behaviour).** `FeedsExplain::published_chipload_band`, the
//!   rule `vendor_band` reads, returns `None` for a max-only row, a
//!   one-point row, an inverted row and no row, and `Some` for a row that
//!   publishes two different limits. The `FeedsExplain` comes from the real
//!   `explain_feeds` door; only its matched row's two chipload fields are
//!   set per case.
//!
//! `the_corridor_bounds_the_band_g_corridor.rs` holds the rendered side: it
//! reads the suggested line and the corridor lines off the painted shapes.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use rs_cam_core::feeds::{
    FeedsExplain, FeedsInput, OperationFamily, PassRole, SetupContext, SpindleStrategy,
    ToolGeometryHint, embedded_vendor_lut, explain_feeds,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

/// The feeds chart files, relative to this crate's manifest.
const CHART_FILES: [&str; 5] = [
    "src/ui/feeds/explore.rs",
    "src/ui/feeds/shared.rs",
    "src/ui/feeds/compare.rs",
    "src/ui/feeds/why.rs",
    "src/ui/feeds/window.rs",
];

/// Text that paints a filled region with `egui_plot`. None of it may appear
/// in the code of a feeds chart file.
const SHADING: [&str; 4] = ["Polygon", ".polygon(", ".fill_color(", "wedge_polygon"];

fn feeds_file(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

/// The file's code with every comment removed.
///
/// Each line loses everything from its first `//`. That also cuts a string
/// that contains `//`, which only removes text; it can make this scan miss a
/// hit, never find a false one.
fn code_of(relative: &str) -> String {
    let path = feeds_file(relative);
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    source
        .lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The body of `fn name` in `code`, from its signature to the first line
/// that is a lone closing brace at column zero.
fn function_body<'a>(code: &'a str, signature: &str) -> &'a str {
    let start = code
        .find(signature)
        .unwrap_or_else(|| panic!("`{signature}` is missing"));
    let rest = &code[start..];
    let end = rest
        .find("\n}")
        .unwrap_or_else(|| panic!("`{signature}` has no closing brace"));
    &rest[..end]
}

// ── arm 1 — source ──────────────────────────────────────────────────────

#[test]
fn the_feeds_charts_construct_no_shading_g_chartlines() {
    let explore = code_of("src/ui/feeds/explore.rs");
    // Non-vacuity: the nomogram is still where this scan looks for it.
    assert!(
        explore.contains("fn draw_chart_c("),
        "`fn draw_chart_c(` is not in `explore.rs`. The scan below reads the \
         feeds chart files for shading; if the nomogram moved, move \
         `CHART_FILES` with it, or this arm passes on a file with no chart."
    );
    assert!(
        explore.contains("plot_ui.line("),
        "`draw_chart_c` draws no line, so the scan below has nothing to check"
    );

    let mut hits = Vec::new();
    for relative in CHART_FILES {
        let code = code_of(relative);
        for (number, line) in code.lines().enumerate() {
            for token in SHADING {
                if line.contains(token) {
                    hits.push(format!("{relative}:{}: {}", number + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "a feeds chart file paints a filled region. The ruling is lines, not \
         shading (G-CHARTLINES):\n{}",
        hits.join("\n")
    );
}

#[test]
fn the_vendor_band_is_the_published_band_g_chartlines() {
    let shared = code_of("src/ui/feeds/shared.rs");
    let body = function_body(&shared, "pub(crate) fn vendor_band(");
    assert!(
        body.contains("published_chipload_band()"),
        "`vendor_band` does not read `FeedsExplain::published_chipload_band`. \
         It must use the core rule, which never makes a band from one value:\n{body}"
    );
    assert!(
        !body.contains("0.7"),
        "`vendor_band` still makes a lower limit from the maximum:\n{body}"
    );

    // The chart and the why text get the band from `vendor_band` only. A
    // second derivation from the row fields is how the invented band lived.
    for relative in [
        "src/ui/feeds/explore.rs",
        "src/ui/feeds/why.rs",
        "src/ui/feeds/compare.rs",
        "src/ui/feeds/window.rs",
    ] {
        let code = code_of(relative);
        assert!(
            !code.contains("chip_load_min_mm"),
            "`{relative}` reads `chip_load_min_mm` itself. Read the band through \
             `vendor_band` so that one rule decides whether a band exists."
        );
    }

    let explore = code_of("src/ui/feeds/explore.rs");
    let chart = function_body(&explore, "pub(crate) fn draw_chart_c(");
    assert!(
        chart.contains("let band = vendor_band(explain);"),
        "`draw_chart_c` does not take its band lines from `vendor_band`"
    );
}

// ── arm 2 — behaviour ───────────────────────────────────────────────────

/// A real `FeedsExplain` from the `explain_feeds` door, with a matched row.
fn explain_with_a_row() -> FeedsExplain {
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    let machine = MachineProfile::shapeoko_vfd();
    let input = FeedsInput {
        tool_diameter: 6.35,
        flute_count: 2,
        flute_length: 22.0,
        shank_diameter: Some(6.35),
        tool_geometry: ToolGeometryHint::Flat,
        material: &material,
        machine: &machine,
        operation: OperationFamily::Pocket,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(embedded_vendor_lut()),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::default(),
    };
    let explain = explain_feeds(&input);
    assert!(
        explain.matched_row.is_some(),
        "the fixture matched no vendor row, so arm 2 cannot set the row's \
         limits. Choose a tool, material and operation that the embedded \
         table covers."
    );
    explain
}

/// `explain` with its matched row's two chipload limits replaced.
fn with_limits(explain: &FeedsExplain, min: Option<f64>, max: Option<f64>) -> FeedsExplain {
    let mut out = explain.clone();
    let row = out.matched_row.as_mut().expect("the fixture has a row");
    row.chip_load_min_mm = min;
    row.chip_load_max_mm = max;
    out
}

#[test]
fn a_one_value_row_publishes_no_band_g_chartlines() {
    let explain = explain_with_a_row();

    let max_only = with_limits(&explain, None, Some(0.05));
    assert_eq!(
        max_only.published_chipload_band(),
        None,
        "a row with a maximum only publishes no band. A band from it is an \
         invented lower limit."
    );

    let point = with_limits(&explain, Some(0.05), Some(0.05));
    assert_eq!(
        point.published_chipload_band(),
        None,
        "a row with one point (min == max) publishes no band. This is the \
         `VendorLutPointPreset` rule in `tool_load::chipload`."
    );

    let inverted = with_limits(&explain, Some(0.06), Some(0.05));
    assert_eq!(
        inverted.published_chipload_band(),
        None,
        "a row whose minimum is above its maximum publishes no band"
    );

    let min_only = with_limits(&explain, Some(0.03), None);
    assert_eq!(
        min_only.published_chipload_band(),
        None,
        "a row with a minimum only publishes no band"
    );

    let mut no_row = explain;
    no_row.matched_row = None;
    assert_eq!(
        no_row.published_chipload_band(),
        None,
        "no matched row publishes no band"
    );
}

#[test]
fn a_two_limit_row_publishes_its_band_g_chartlines() {
    let explain = explain_with_a_row();
    let both = with_limits(&explain, Some(0.03), Some(0.05));
    assert_eq!(
        both.published_chipload_band(),
        Some((0.03, 0.05)),
        "a row that publishes two different limits publishes exactly that \
         band, with no scaling added by the accessor"
    );
}
