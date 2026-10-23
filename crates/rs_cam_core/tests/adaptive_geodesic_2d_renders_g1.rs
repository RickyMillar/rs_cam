//! G1 render harness of the adaptive research 2026-10-23, Phase 2, candidate
//! (a): the geodesic-wavefront field for 2D clearing
//! (`rs_cam_core::adaptive::research_geodesic`, behind `research`).
//!
//! The harness plans the two development shapes, `test_data/ux_2d_star.toml`
//! and `test_data/ux_2d_pocket.toml`, with each 2D strategy and with
//! candidate (a), on the same region with the same params, and writes one
//! SVG and one text summary per strategy per shape. It ranks nothing and
//! scores nothing: the operator picks by look (PLAN.md, "Look-review
//! protocol").
//!
//! - Strategies: `PathStrategy2d::Agent`, `PathStrategy2d::ContourSpiral`
//!   and candidate (a). The 2D op exposes no contour-parallel strategy: its
//!   contour-parallel path runs only behind the narrow-region gate
//!   (`CleanupStrategy::ContourParallelNarrow` / `Hybrid`), so it is not a
//!   column here.
//! - Params: `AdaptiveConfig::default()` (stepover, tolerance, cleanup,
//!   measure, corner radius, slot lines), the 6 mm end mill of the fixture,
//!   and the entry helix radius the session gives a new Adaptive toolpath
//!   (`DressupConfig::for_op(OperationType::Adaptive)`, `helix_radius_for`).
//!   The rest of `AdaptiveParams` is set as `clearing_2d.rs` sets it. Z does
//!   not enter an XY plan: the cut level is one depth per pass under the
//!   stock top, and the safe Z is one depth per pass over it.
//! - The SVG: cutting moves blue, keep-down links orange, rapids dashed
//!   grey, each entry (plunge) a red ring, the outline black, and for
//!   candidate (a) its wraps in pale grey under the moves.
//! - The summary: cut and link length, entries, lifts (retracts), keep-down
//!   links and the planner's pass-load readout (`research_geodesic` module
//!   doc, `PassLoadReadout`).
//!
//! A shipped column must be the product plan: the harness asserts that its
//! toolpath equals `adaptive_toolpath` on the same outline, move by move.
//!
//! # Running it
//!
//! ```text
//! cargo test --release -p rs_cam_core --features research,test-support \
//!     --test adaptive_geodesic_2d_renders_g1 -- --ignored --nocapture
//! ```
//!
//! Output: `$ADAPTIVE_GEODESIC_SVG_DIR`, else
//! `<CARGO_TARGET_TMPDIR>/adaptive_geodesic_2d_g1/` (under `target/`).

#![expect(
    clippy::expect_used,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the harness prints where it wrote"
)]

mod common;

use std::fmt::Write as _;
use std::path::PathBuf;

use rs_cam_core::adaptive::research_geodesic::{Candidate2d, Run2d, run_2d};
use rs_cam_core::adaptive::{AdaptiveParams, KeepDownLinks, PathStrategy2d, adaptive_toolpath};
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::config::{DressupConfig, DressupEntryStyle};
use rs_cam_core::compute::fixture_values::fixture_feeds;
use rs_cam_core::compute::operation_configs::AdaptiveConfig;
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};

/// The two development shapes (PLAN.md, "Fixtures").
const SHAPES: &[(&str, &str)] = &[
    ("star", "test_data/ux_2d_star.toml"),
    ("pocket", "test_data/ux_2d_pocket.toml"),
];

/// The tool of the render: the fixture's 6 mm end mill.
const TOOL_DIAMETER_MM: f64 = 6.0;

/// SVG canvas width (px) and margin (px). Display only.
const CANVAS_PX: f64 = 900.0;
const MARGIN_PX: f64 = 24.0;

fn svg_output_dir() -> PathBuf {
    std::env::var_os("ADAPTIVE_GEODESIC_SVG_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("adaptive_geodesic_2d_g1"),
        PathBuf::from,
    )
}

/// The largest closed region of the fixture's drawing (the star; the
/// rounded rectangle with its round island as a hole), and the fixture's
/// end mill.
fn fixture(project: &str) -> (Polygon2, FlatEndmill) {
    let path = common::repo_root().join(project);
    let session = ProjectSession::load(common::require_fixture(&path)).expect("load the fixture");
    let outline = session
        .models()
        .iter()
        .flat_map(|m| m.items.closed_regions().to_vec())
        .max_by(|a, b| a.area().total_cmp(&b.area()))
        .expect("the fixture draws a closed region");
    let tool = session
        .tools()
        .iter()
        .find(|t| (t.diameter - TOOL_DIAMETER_MM).abs() < 1e-9)
        .expect("the fixture holds the 6 mm end mill");
    (
        outline,
        FlatEndmill::new(tool.diameter, tool.cutting_length),
    )
}

/// The shared params (module doc, "Params").
fn params(tool: &FlatEndmill, strategy: PathStrategy2d) -> AdaptiveParams {
    let cfg = AdaptiveConfig::default();
    let dressups = DressupConfig::for_op(OperationType::Adaptive);
    let entry_helix_radius = if dressups.entry_style == DressupEntryStyle::Helix {
        dressups.helix_radius_for(tool).emitted_mm
    } else {
        0.0
    };
    // S11: the op has no feed default; a fixture states the pre-S11 values.
    let (feed_rate, plunge_rate) = fixture_feeds(OperationType::Adaptive);
    AdaptiveParams {
        tool_radius: tool.diameter / 2.0,
        stepover: cfg.stepover,
        cut_depth: -cfg.depth_per_pass,
        feed_rate,
        plunge_rate,
        safe_z: cfg.depth_per_pass,
        tolerance: cfg.tolerance,
        slot_clearing: cfg.slot_clearing,
        min_cutting_radius: cfg.min_cutting_radius,
        standing_material: None,
        cleanup_strategy: cfg.cleanup_strategy,
        engagement_measure: cfg.engagement_measure,
        path_strategy: strategy,
        // As `clearing_2d.rs` sets it for 2D Adaptive.
        trochoid_cap_mult: 1.2,
        keep_down_links: KeepDownLinks::WithinPassLoad,
        entry_helix_radius,
    }
}

/// A shipped column is the product plan, move by move.
fn assert_product_parity(outline: &Polygon2, params: &AdaptiveParams, run: &Run2d, label: &str) {
    if run.summary.pieces != 1 {
        return;
    }
    let product = adaptive_toolpath(outline, params);
    assert_eq!(
        product.moves.len(),
        run.toolpath.moves.len(),
        "{label}: the research run must emit the product plan"
    );
    for (a, b) in product.moves.iter().zip(&run.toolpath.moves) {
        assert!(
            (a.target - b.target).norm() < 1e-9 && a.intent == b.intent,
            "{label}: the research run must emit the product plan"
        );
    }
}

/// World XY to SVG pixels (Y up in the world, down on the page).
struct View {
    min_x: f64,
    max_y: f64,
    scale: f64,
    height: f64,
}

impl View {
    fn over(outline: &Polygon2) -> Self {
        let xs = outline.exterior.iter().map(|p| p.x);
        let ys = outline.exterior.iter().map(|p| p.y);
        let (min_x, max_x) = xs.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        });
        let (min_y, max_y) = ys.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        });
        let scale = (CANVAS_PX - 2.0 * MARGIN_PX) / (max_x - min_x).max(max_y - min_y);
        Self {
            min_x,
            max_y,
            scale,
            height: (max_y - min_y) * scale + 2.0 * MARGIN_PX,
        }
    }

    fn px(&self, x: f64, y: f64) -> (f64, f64) {
        (
            MARGIN_PX + (x - self.min_x) * self.scale,
            MARGIN_PX + (self.max_y - y) * self.scale,
        )
    }
}

fn polyline(view: &View, pts: &[P2], closed: bool, style: &str) -> String {
    let mut d = String::new();
    for (k, p) in pts.iter().enumerate() {
        let (x, y) = view.px(p.x, p.y);
        let _ = write!(d, "{}{x:.2},{y:.2} ", if k == 0 { "M" } else { "L" });
    }
    if closed {
        d.push('Z');
    }
    format!("<path d='{d}' {style}/>\n")
}

fn summary_text(shape: &str, label: &str, run: &Run2d) -> String {
    let s = &run.summary;
    let l = &s.pass_load;
    let mut t = String::new();
    let _ = writeln!(t, "{shape} / {label}");
    let _ = writeln!(t, "pieces: {}", s.pieces);
    let _ = writeln!(t, "cut length: {:.1} mm", s.cut_length_mm);
    let _ = writeln!(
        t,
        "keep-down links: {} ({:.1} mm)",
        s.keep_down_links, s.link_length_mm
    );
    let _ = writeln!(t, "entries (plunges): {}", s.entries);
    let _ = writeln!(
        t,
        "retracts (lifts to safe Z, last included): {}",
        s.retracts
    );
    let _ = writeln!(
        t,
        "pass load (planner readout): max {:.4} vs ceiling {:.4}; {} of {} steps over; {} exempt plunge-departure steps",
        l.max_reading, l.ceiling, l.steps_over, l.steps, l.exempt_steps
    );
    for (i, f) in run.fields.iter().enumerate() {
        let _ = writeln!(
            t,
            "field piece {i}: applied {}, seed {:?}, seed radius {:.2} mm, levels {}, wrap pieces {}, converged {}",
            f.applied, f.seed, f.seed_radius, f.levels, f.chains, f.converged
        );
    }
    t
}

fn render(outline: &Polygon2, run: &Run2d, cut_depth: f64, caption: &str) -> String {
    let view = View::over(outline);
    let mut svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='{CANVAS_PX}' height='{h:.0}' viewBox='0 0 {CANVAS_PX} {h:.0}'>\n<rect width='100%' height='100%' fill='white'/>\n",
        h = view.height
    );
    for wrap in &run.wraps {
        svg.push_str(&polyline(
            &view,
            wrap,
            false,
            "fill='none' stroke='#d0d0d0' stroke-width='0.6'",
        ));
    }
    svg.push_str(&polyline(
        &view,
        &outline.exterior,
        true,
        "fill='none' stroke='black' stroke-width='1.2'",
    ));
    for hole in &outline.holes {
        svg.push_str(&polyline(
            &view,
            hole,
            true,
            "fill='none' stroke='black' stroke-width='1.2'",
        ));
    }
    draw_moves(&view, &run.toolpath, cut_depth, &mut svg);
    for (k, line) in caption.lines().enumerate() {
        let _ = writeln!(
            svg,
            "<text x='6' y='{}' font-family='monospace' font-size='11'>{line}</text>",
            14 + 13 * k
        );
    }
    svg.push_str("</svg>\n");
    svg
}

fn draw_moves(view: &View, tp: &Toolpath, cut_depth: f64, svg: &mut String) {
    let at_depth = |z: f64| (z - cut_depth).abs() < 1e-6;
    let mut prev: Option<rs_cam_core::geo::P3> = None;
    for m in &tp.moves {
        let to = m.target;
        if let Some(from) = prev {
            let (x0, y0) = view.px(from.x, from.y);
            let (x1, y1) = view.px(to.x, to.y);
            let flat = (from.x - to.x).hypot(from.y - to.y) > 1e-9;
            match m.move_type {
                MoveType::Rapid if flat => {
                    let _ = writeln!(
                        svg,
                        "<line x1='{x0:.2}' y1='{y0:.2}' x2='{x1:.2}' y2='{y1:.2}' stroke='#999' stroke-width='0.6' stroke-dasharray='3,3'/>"
                    );
                }
                MoveType::Rapid => {}
                MoveType::Linear { .. } if m.intent == MoveIntent::EntryPlunge => {
                    let _ = writeln!(
                        svg,
                        "<circle cx='{x1:.2}' cy='{y1:.2}' r='4' fill='none' stroke='#d00' stroke-width='1.4'/>"
                    );
                }
                MoveType::Linear { .. } if flat && at_depth(from.z) && at_depth(to.z) => {
                    let colour = if m.intent == MoveIntent::Linking {
                        "#f08000"
                    } else {
                        "#1f5fbf"
                    };
                    let _ = writeln!(
                        svg,
                        "<line x1='{x0:.2}' y1='{y0:.2}' x2='{x1:.2}' y2='{y1:.2}' stroke='{colour}' stroke-width='0.9'/>"
                    );
                }
                MoveType::Linear { .. } => {}
                MoveType::ArcCW { i, j, .. } | MoveType::ArcCCW { i, j, .. } => {
                    // Y flips on the page, so a world CCW arc is a page CW
                    // arc: SVG sweep 1.
                    let ccw = matches!(m.move_type, MoveType::ArcCCW { .. });
                    let radius = i.hypot(j);
                    let c = (from.x + i, from.y + j);
                    let a0 = (from.y - c.1).atan2(from.x - c.0);
                    let a1 = (to.y - c.1).atan2(to.x - c.0);
                    let mut sweep = if ccw { a1 - a0 } else { a0 - a1 };
                    if sweep <= 0.0 {
                        sweep += std::f64::consts::TAU;
                    }
                    let large = u8::from(sweep > std::f64::consts::PI);
                    let r_px = radius * view.scale;
                    let _ = writeln!(
                        svg,
                        "<path d='M{x0:.2},{y0:.2} A{r_px:.2},{r_px:.2} 0 {large} {} {x1:.2},{y1:.2}' fill='none' stroke='#1f5fbf' stroke-width='0.9'/>",
                        u8::from(ccw)
                    );
                }
            }
        }
        prev = Some(to);
    }
}

#[test]
#[ignore = "instrument: writes the G1 renders of candidate (a) and the 2D baselines for the operator's look"]
fn render_candidate_a_beside_the_2d_strategies() {
    let dir = svg_output_dir();
    std::fs::create_dir_all(&dir).expect("create the output dir");
    let mut all = String::new();
    for &(shape, project) in SHAPES {
        let (outline, tool) = fixture(project);
        let columns = [
            ("agent", Candidate2d::Shipped(PathStrategy2d::Agent)),
            (
                "contour_spiral",
                Candidate2d::Shipped(PathStrategy2d::ContourSpiral),
            ),
            ("geodesic_a", Candidate2d::Geodesic),
        ];
        for (label, candidate) in columns {
            let strategy = match candidate {
                Candidate2d::Shipped(s) => s,
                Candidate2d::Geodesic => PathStrategy2d::Agent,
            };
            let params = params(&tool, strategy);
            let run = run_2d(&outline, &params, candidate);
            if matches!(candidate, Candidate2d::Shipped(_)) {
                assert_product_parity(&outline, &params, &run, label);
            } else {
                assert!(
                    run.fields.iter().any(|f| f.applied),
                    "{shape}: candidate (a) must apply on a development shape: {:?}",
                    run.fields
                );
            }
            let text = summary_text(shape, label, &run);
            let svg = render(&outline, &run, params.cut_depth, &text);
            let stem = dir.join(format!("{shape}_{label}"));
            std::fs::write(stem.with_extension("svg"), svg).expect("write the SVG");
            std::fs::write(stem.with_extension("txt"), &text).expect("write the summary");
            all.push_str(&text);
            all.push('\n');
        }
    }
    std::fs::write(dir.join("summary.txt"), &all).expect("write the summary");
    eprintln!("{all}wrote {}", dir.display());
}
