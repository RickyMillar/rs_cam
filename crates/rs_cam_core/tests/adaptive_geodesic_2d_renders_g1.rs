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
//!   and candidate (a) four times: `geodesic_g1` (`GeodesicOptions::G1`,
//!   the form of the G1 look, the "before"), `geodesic_a`
//!   (`GeodesicOptions::C2`, the G1 fixes with the G1 join, the C3 state),
//!   `geodesic_morph_alt` (`GeodesicOptions::C4_ALT`: the morph join, the
//!   core spiral and half-pitch alternating legs in the lobes) and
//!   `geodesic_morph_return` (`GeodesicOptions::C4_RETURN`: one-way legs
//!   and a return through cleared stock).
//! - The record columns, only when `ADAPTIVE_GEODESIC_EXTRA` is set (any
//!   value): `geodesic_a_through` (the fixes with the G1 meeting rule),
//!   `geodesic_a_ramp` (the ramp join that C3 reverted, for its
//!   refused-step dump) and `geodesic_morph_alt_full` (the brief's literal
//!   alternation, one leg per band, for its refused-step dump). The 2D op exposes no contour-parallel strategy: its
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
//! - The SVG and the PNG (the harness draws both from one list of marks, so
//!   no outside converter can drop the colours): the cleaned planner plan
//!   (`Run2d::spans`, before the toolpath emission), one colour and stroke
//!   width per producer and move kind. Clearing cuts blue, the starter
//!   purple, the wall pass green (widest), residue contours brown, the
//!   residue mop magenta, keep-down links orange, rapids dashed grey, each
//!   entry a red ring, each step over the pass load a black ring, the
//!   outline black, and candidate (a)'s wraps pale grey under the moves.
//!   The legend is in the SVG caption.
//! - The summary: cut and link length, entries, lifts (retracts), keep-down
//!   links and the planner's pass-load readout (`research_geodesic` module
//!   doc, `PassLoadReadout`), with one line per step over the ceiling: its
//!   producer, cut or link, segment, position and reading; the entries by
//!   producer (`EntrySplit`: starter, clearing at a wrap end, clearing
//!   mid-wrap, residue contour, wall pass, residue mop); the readout per
//!   producer (max reading, steps, steps over) of the emitted plan, and per
//!   producer and part (spiral, morph leg, return, wrap, join) of the raw
//!   plan, where a step over is a step the replay refuses; the first ten
//!   such refused steps; and, for the morph join, the lobe counts.
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

use rs_cam_core::adaptive::research_geodesic::{
    Candidate2d, GeodesicOptions, JoinRule, LobeMorph, MeetingRule, PassLoadReadout, Producer,
    Run2d, Span2d, SpanKind, run_2d,
};
use rs_cam_core::adaptive::{AdaptiveParams, KeepDownLinks, PathStrategy2d, adaptive_toolpath};
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::config::{DressupConfig, DressupEntryStyle};
use rs_cam_core::compute::fixture_values::fixture_feeds;
use rs_cam_core::compute::operation_configs::AdaptiveConfig;
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::tool::FlatEndmill;

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

/// World XY to page pixels (Y up in the world, down on the page).
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

    fn px(&self, p: P2) -> (f64, f64) {
        (
            MARGIN_PX + (p.x - self.min_x) * self.scale,
            MARGIN_PX + (self.max_y - p.y) * self.scale,
        )
    }
}

/// One drawn item, in page pixels. The SVG and the PNG draw the same list.
enum Mark {
    /// A polyline: colour, stroke width (px), dashed.
    Line {
        pts: Vec<(f64, f64)>,
        rgb: [u8; 3],
        width: f64,
        dashed: bool,
    },
    /// A ring: centre, radius (px), colour.
    Ring {
        at: (f64, f64),
        radius: f64,
        rgb: [u8; 3],
    },
}

/// The style of a span of the plan (the legend is in the caption).
fn span_style(span: &Span2d) -> ([u8; 3], f64, bool) {
    match (span.kind, span.producer) {
        (SpanKind::Rapid, _) => ([0x99, 0x99, 0x99], 0.8, true),
        (SpanKind::Link, _) => ([0xf0, 0x80, 0x00], 1.0, false),
        (SpanKind::Cut, Producer::WallPass) => ([0x1a, 0x8a, 0x3a], 2.0, false),
        (SpanKind::Cut, Producer::ResidueContour) => ([0xb5, 0x65, 0x1d], 1.6, false),
        (SpanKind::Cut, Producer::ResidueMop) => ([0xc0, 0x27, 0x8f], 1.6, false),
        (SpanKind::Cut, Producer::Starter) => ([0x7b, 0x3f, 0xa0], 1.2, false),
        (SpanKind::Cut, _) => ([0x1f, 0x5f, 0xbf], 1.0, false),
    }
}

const LEGEND: &str = "blue clearing cut | purple starter | green wall pass | brown residue contour | magenta residue mop\norange keep-down link | grey dashed rapid | red ring entry | black ring step over the pass load | pale grey (a) wraps";

fn marks(outline: &Polygon2, run: &Run2d, view: &View) -> Vec<Mark> {
    let line = |pts: &[P2], rgb: [u8; 3], width: f64, dashed: bool| Mark::Line {
        pts: pts.iter().map(|&p| view.px(p)).collect(),
        rgb,
        width,
        dashed,
    };
    let mut out: Vec<Mark> = run
        .wraps
        .iter()
        .map(|w| line(w, [0xd0, 0xd0, 0xd0], 0.6, false))
        .collect();
    for ring in std::iter::once(&outline.exterior).chain(outline.holes.iter()) {
        let closed: Vec<P2> = ring.iter().chain(ring.first()).copied().collect();
        out.push(line(&closed, [0, 0, 0], 1.2, false));
    }
    for span in &run.spans {
        let (rgb, width, dashed) = span_style(span);
        out.push(line(&span.pts, rgb, width, dashed));
        if span.kind == SpanKind::Rapid
            && let Some(&entry) = span.pts.last()
        {
            out.push(Mark::Ring {
                at: view.px(entry),
                radius: 4.0,
                rgb: [0xdd, 0, 0],
            });
        }
    }
    for over in &run.summary.pass_load.over {
        out.push(Mark::Ring {
            at: view.px(over.to),
            radius: 7.0,
            rgb: [0, 0, 0],
        });
    }
    out
}

fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

fn svg_of(marks: &[Mark], view: &View, caption: &str) -> String {
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{CANVAS_PX}\" height=\"{h:.0}\" viewBox=\"0 0 {CANVAS_PX} {h:.0}\">\n<rect width=\"100%\" height=\"100%\" fill=\"#ffffff\"/>\n",
        h = view.height
    );
    for mark in marks {
        match mark {
            Mark::Line {
                pts,
                rgb,
                width,
                dashed,
            } => {
                let mut d = String::new();
                for (k, (x, y)) in pts.iter().enumerate() {
                    let _ = write!(d, "{}{x:.1},{y:.1}", if k == 0 { "M" } else { "L" });
                }
                let dash = if *dashed {
                    " stroke-dasharray=\"3,3\""
                } else {
                    ""
                };
                let _ = writeln!(
                    svg,
                    "<path d=\"{d}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{width}\"{dash}/>",
                    hex(*rgb)
                );
            }
            Mark::Ring { at, radius, rgb } => {
                let _ = writeln!(
                    svg,
                    "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{radius}\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.4\"/>",
                    at.0,
                    at.1,
                    hex(*rgb)
                );
            }
        }
    }
    for (k, line) in caption.lines().chain(LEGEND.lines()).enumerate() {
        let _ = writeln!(
            svg,
            "<text x=\"6\" y=\"{}\" font-family=\"monospace\" font-size=\"11\" fill=\"#000000\">{line}</text>",
            14 + 13 * k
        );
    }
    svg.push_str("</svg>\n");
    svg
}

/// The PNG of the same marks, drawn here so that no outside converter can
/// drop the colours (the G1 PNGs came out in one colour). No text: the
/// caption is in the SVG and the `.txt`.
fn png_of(marks: &[Mark], view: &View) -> image::RgbImage {
    let (w, h) = (CANVAS_PX as u32, view.height.ceil() as u32);
    let mut img = image::RgbImage::from_pixel(w, h, image::Rgb([255, 255, 255]));
    let mut dot = |x: f64, y: f64, radius: f64, rgb: [u8; 3]| {
        let r = radius.max(0.5);
        let (x0, x1) = ((x - r).floor() as i64, (x + r).ceil() as i64);
        let (y0, y1) = ((y - r).floor() as i64, (y + r).ceil() as i64);
        for py in y0..=y1 {
            for px in x0..=x1 {
                let inside = (px as f64 - x).hypot(py as f64 - y) <= r;
                if inside && px >= 0 && py >= 0 && (px as u32) < w && (py as u32) < h {
                    img.put_pixel(px as u32, py as u32, image::Rgb(rgb));
                }
            }
        }
    };
    // Sample spacing along a stroke and the dash period (px): display only.
    let step = 0.5;
    let dash = 6.0;
    for mark in marks {
        match mark {
            Mark::Line {
                pts,
                rgb,
                width,
                dashed,
            } => {
                let mut along = 0.0_f64;
                for pair in pts.windows(2) {
                    let &[(ax, ay), (bx, by)] = pair else {
                        continue;
                    };
                    let len = (bx - ax).hypot(by - ay);
                    let n = (len / step).ceil().max(1.0) as usize;
                    for k in 0..=n {
                        let t = k as f64 / n as f64;
                        let on = !*dashed || (along + t * len) % dash < dash / 2.0;
                        if on {
                            dot(ax + t * (bx - ax), ay + t * (by - ay), width / 2.0, *rgb);
                        }
                    }
                    along += len;
                }
            }
            Mark::Ring { at, radius, rgb } => {
                let n = (std::f64::consts::TAU * radius / step).ceil() as usize;
                for k in 0..n {
                    let a = std::f64::consts::TAU * k as f64 / n as f64;
                    dot(at.0 + radius * a.cos(), at.1 + radius * a.sin(), 0.7, *rgb);
                }
            }
        }
    }
    img
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
    for o in &l.over {
        let _ = writeln!(
            t,
            "  over: {} {} (segment {}) from ({:.2}, {:.2}) to ({:.2}, {:.2}), reading {:.4}",
            o.producer.label(),
            if o.link { "link" } else { "cut" },
            o.segment,
            o.from.x,
            o.from.y,
            o.to.x,
            o.to.y,
            o.reading
        );
    }
    tallies(&mut t, "emitted plan", l);
    tallies(&mut t, "raw plan", &s.raw_pass_load);
    let e = &s.entry_split;
    let _ = writeln!(
        t,
        "entries by producer: starter {}, clearing at a wrap end (join) {}, clearing mid-wrap (replay split) {}, residue contour {}, wall pass {}, residue mop {}, other {}",
        e.starter,
        e.clearing_join,
        e.clearing_split,
        e.residue_contour,
        e.wall_pass,
        e.residue_mop,
        e.other
    );
    let r = &s.raw_pass_load;
    let _ = writeln!(
        t,
        "raw plan (before the replay): {} of {} steps over the ceiling (the steps the replay refuses; exact up to the first); the first ten:",
        r.steps_over, r.steps
    );
    for o in r.over.iter().take(10) {
        let _ = writeln!(
            t,
            "  refused: {} {} {} (segment {}) from ({:.2}, {:.2}) to ({:.2}, {:.2}), reading {:.4}",
            o.producer.label(),
            if o.link { "link" } else { "cut" },
            o.part.label(),
            o.segment,
            o.from.x,
            o.from.y,
            o.to.x,
            o.to.y,
            o.reading
        );
    }
    for (i, f) in run.fields.iter().enumerate() {
        let _ = writeln!(
            t,
            "field piece {i}: applied {}, seed {:?}, seed radius {:.2} mm, levels {}, wrap pieces {}, converged {}, front branches {}, spiral levels {}",
            f.applied,
            f.seed,
            f.seed_radius,
            f.levels,
            f.chains,
            f.converged,
            f.branches,
            f.spiral_levels
        );
        let b = &f.lobes;
        if b.lobes + b.plain_wraps > 0 {
            let _ = writeln!(
                t,
                "lobes piece {i}: {} lobes of {} arcs, {} plain wraps; not joined: split {}, branch {}, length ratio {}, gap {}; lobes that left the region (cut plain) {}; return length {:.1} mm",
                b.lobes,
                b.lobe_arcs,
                b.plain_wraps,
                b.stop_split,
                b.stop_branch,
                b.stop_ratio,
                b.stop_gap,
                b.outside,
                b.return_length_mm
            );
        }
    }
    t
}

/// One line per class of steps of `readout` ([`PassLoadReadout::tallies`]).
fn tallies(t: &mut String, title: &str, readout: &PassLoadReadout) {
    let _ = writeln!(
        t,
        "load by class, {title} (max reading, steps over of steps):"
    );
    for c in &readout.tallies {
        let _ = writeln!(
            t,
            "  {}: max {:.4}, {} of {}",
            c.label, c.max_reading, c.steps_over, c.steps
        );
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
        let mut columns = vec![
            ("agent", Candidate2d::Shipped(PathStrategy2d::Agent)),
            (
                "contour_spiral",
                Candidate2d::Shipped(PathStrategy2d::ContourSpiral),
            ),
            ("geodesic_g1", Candidate2d::Geodesic(GeodesicOptions::G1)),
            ("geodesic_a", Candidate2d::Geodesic(GeodesicOptions::C2)),
            (
                "geodesic_morph_alt",
                Candidate2d::Geodesic(GeodesicOptions::C4_ALT),
            ),
            (
                "geodesic_morph_return",
                Candidate2d::Geodesic(GeodesicOptions::C4_RETURN),
            ),
        ];
        if std::env::var_os("ADAPTIVE_GEODESIC_EXTRA").is_some() {
            columns.extend([
                (
                    "geodesic_a_through",
                    Candidate2d::Geodesic(GeodesicOptions {
                        meeting: MeetingRule::Through,
                        ..GeodesicOptions::C2
                    }),
                ),
                (
                    "geodesic_a_ramp",
                    Candidate2d::Geodesic(GeodesicOptions {
                        join: JoinRule::Ramp,
                        ..GeodesicOptions::C2
                    }),
                ),
                (
                    "geodesic_morph_alt_full",
                    Candidate2d::Geodesic(GeodesicOptions {
                        join: JoinRule::Morph(LobeMorph::AlternateFull),
                        ..GeodesicOptions::C2
                    }),
                ),
            ]);
        }
        for (label, candidate) in columns {
            let strategy = match candidate {
                Candidate2d::Shipped(s) => s,
                Candidate2d::Geodesic(_) => PathStrategy2d::Agent,
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
            let view = View::over(&outline);
            let drawn = marks(&outline, &run, &view);
            let stem = dir.join(format!("{shape}_{label}"));
            std::fs::write(stem.with_extension("svg"), svg_of(&drawn, &view, &text))
                .expect("write the SVG");
            png_of(&drawn, &view)
                .save(stem.with_extension("png"))
                .expect("write the PNG");
            std::fs::write(stem.with_extension("txt"), &text).expect("write the summary");
            all.push_str(&text);
            all.push('\n');
        }
    }
    std::fs::write(dir.join("summary.txt"), &all).expect("write the summary");
    eprintln!("{all}wrote {}", dir.display());
}
