//! Phase 0 — diagnose why a contour-spiral toolpath errors the controller.
//!
//! `planning/ACCEL_FRIENDLY_TOOLPATHS_2026-06-20.md`. The user reported that
//! running a contour-spiral pass on a Shapeoko "just errored the machine".
//! This harness reproduces the *export* path the controller saw — generate a
//! contour-spiral toolpath, run the production arc-fit dressup (tol 0.05, the
//! Roughing-role default), emit GRBL G-code — and then validates the emitted
//! TEXT against the exact rules a GRBL-family controller enforces:
//!
//!   1. Arc endpoint consistency: GRBL recomputes the arc radius at the start
//!      `hypot(I, J)` and at the end `hypot(end - center)` and throws
//!      `error:33` (invalid target) when they differ by more than `$12`
//!      (default 0.010 mm). We validate on the ROUNDED words the emitter
//!      actually writes (3 dp), so decimal-rounding drift is caught too.
//!   2. Finite coordinates — no `NaN`/`inf` tokens.
//!   3. Positive feed on every cutting move (`F0` → `error:22` undefined feed).
//!
//! It also reports segment-length density (how many cut moves are below the
//! accel-ramp length) as context for Phases 1-2, but only asserts on the
//! controller-fatal defects.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::adaptive::{
    AdaptiveParams, CleanupStrategy, EngagementMeasure, PathStrategy2d, adaptive_toolpath,
};
use rs_cam_core::arcfit::fit_arcs;
use rs_cam_core::gcode::{emit_gcode, post};
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::toolpath_spans::AnnotatedToolpath;

const R: f64 = 3.175;
const STEPOVER: f64 = 2.0;
const ARC_TOL: f64 = 0.05; // Roughing-role DressupConfig::arc_tolerance
const GRBL_ARC_TOL: f64 = 0.010; // $12 default — arc endpoint radius tolerance

fn square(size: f64) -> Polygon2 {
    Polygon2::new(vec![
        P2::new(0.0, 0.0),
        P2::new(size, 0.0),
        P2::new(size, size),
        P2::new(0.0, size),
    ])
}

/// A concave star — its reflex corners spike engagement, which is exactly
/// where the spiral inserts trochoid loops (the near-360° geometry under
/// suspicion).
fn star(points: usize, r_out: f64, r_in: f64, cx: f64, cy: f64) -> Polygon2 {
    let mut v = Vec::with_capacity(points * 2);
    for i in 0..(points * 2) {
        let ang = std::f64::consts::PI * (i as f64) / (points as f64);
        let r = if i % 2 == 0 { r_out } else { r_in };
        v.push(P2::new(cx + r * ang.cos(), cy + r * ang.sin()));
    }
    Polygon2::new(v)
}

fn spiral_params() -> AdaptiveParams {
    AdaptiveParams {
        tool_radius: R,
        stepover: STEPOVER,
        cut_depth: -3.0,
        feed_rate: 1500.0,
        plunge_rate: 500.0,
        safe_z: 10.0,
        tolerance: 0.2,
        slot_clearing: false,
        min_cutting_radius: 0.0,
        initial_stock: None,
        cleanup_strategy: CleanupStrategy::ContourParallelHybrid,
        engagement_measure: EngagementMeasure::LeadingArc,
        path_strategy: PathStrategy2d::ContourSpiral,
        trochoid_cap_mult: 1.6,
    }
}

/// Pull the numeric value of an axis word (e.g. `'X'`) out of a G-code line,
/// or `None` if the word is absent. Returns `Some(NaN)` if the token is
/// present but unparseable (so the finite-check can flag it).
fn word(line: &str, axis: char) -> Option<f64> {
    for tok in line.split_whitespace() {
        let mut chars = tok.chars();
        if chars.next() == Some(axis) {
            let rest: String = chars.collect();
            return Some(rest.parse::<f64>().unwrap_or(f64::NAN));
        }
    }
    None
}

#[derive(Default)]
struct Report {
    lines: usize,
    cut_moves: usize,
    arc_moves: usize,
    short_cut_moves: usize, // below accel-ramp length L_min
    arc_radius_violations: Vec<String>,
    nonfinite_lines: Vec<String>,
    /// Cut moves whose *modal effective feed* is zero/undefined (i.e. a
    /// cutting move executes before any `F` word has been seen). These are
    /// the only real `error:22` candidates — modal moves that inherit a
    /// valid prior feed are fine.
    undefined_feed_lines: Vec<String>,
    f_words_emitted: usize,
    first_cut_feed: Option<f64>,
    worst_radius_mismatch: f64,
}

/// The leading G-word of a line, matched on a token boundary so that `G17`
/// (plane select) is never mistaken for `G1` (linear feed).
fn motion_word(line: &str) -> Option<&'static str> {
    match line.split_whitespace().next()? {
        "G0" | "G00" => Some("G0"),
        "G1" | "G01" => Some("G1"),
        "G2" | "G02" => Some("G2"),
        "G3" | "G03" => Some("G3"),
        _ => None,
    }
}

/// L_min for the test feed (1500 mm/min = 25 mm/s) at Shapeoko A≈350 mm/s².
fn l_min() -> f64 {
    let v = 1500.0 / 60.0;
    v * v / (2.0 * 350.0)
}

fn validate_gcode(gcode: &str) -> Report {
    let mut rep = Report::default();
    let (mut px, mut py) = (0.0f64, 0.0f64);
    let mut modal_feed = 0.0f64; // persists across moves, GRBL-style
    let lmin = l_min();

    for line in gcode.lines() {
        rep.lines += 1;
        let l = line.trim();
        let Some(g) = motion_word(l) else {
            continue;
        };
        let is_g1 = g == "G1";
        let is_cw = g == "G2";
        let is_ccw = g == "G3";

        let x = word(l, 'X').unwrap_or(px);
        let y = word(l, 'Y').unwrap_or(py);

        // Modal feed update.
        if let Some(f) = word(l, 'F') {
            rep.f_words_emitted += 1;
            modal_feed = f;
        }

        // Finite-coordinate check on all motion words.
        let mut tokens_finite = x.is_finite() && y.is_finite();
        for axis in ['Z', 'I', 'J', 'F'] {
            if let Some(v) = word(l, axis)
                && !v.is_finite()
            {
                tokens_finite = false;
            }
        }
        if !tokens_finite {
            rep.nonfinite_lines.push(l.to_owned());
        }

        if is_g1 || is_cw || is_ccw {
            rep.cut_moves += 1;
            if rep.first_cut_feed.is_none() {
                rep.first_cut_feed = Some(modal_feed);
            }
            // A cut move is only fatal if its *effective* (modal) feed is
            // undefined — i.e. no F has been seen yet.
            if modal_feed <= 0.0 {
                rep.undefined_feed_lines.push(l.to_owned());
            }
            let seg = ((x - px).powi(2) + (y - py).powi(2)).sqrt();
            if seg > 1e-9 && seg < lmin {
                rep.short_cut_moves += 1;
            }
        }

        if is_cw || is_ccw {
            rep.arc_moves += 1;
            let i = word(l, 'I').unwrap_or(0.0);
            let j = word(l, 'J').unwrap_or(0.0);
            let r_start = (i * i + j * j).sqrt();
            let (cx, cy) = (px + i, py + j);
            let r_end = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
            let mismatch = (r_end - r_start).abs();
            if mismatch > rep.worst_radius_mismatch {
                rep.worst_radius_mismatch = mismatch;
            }
            if mismatch > GRBL_ARC_TOL {
                rep.arc_radius_violations.push(format!(
                    "{l}  [r_start={r_start:.4} r_end={r_end:.4} Δ={mismatch:.4} from ({px:.3},{py:.3})]"
                ));
            }
        }

        px = x;
        py = y;
    }
    rep
}

fn run_case(name: &str, poly: &Polygon2) -> Report {
    let tp = adaptive_toolpath(poly, &spiral_params());
    let fitted = fit_arcs(AnnotatedToolpath::new(tp), ARC_TOL, R);
    let gcode = emit_gcode(&fitted.toolpath, post::grbl(), 18000);
    let rep = validate_gcode(&gcode);

    println!("── contour-spiral GRBL validity: {name} ──");
    println!(
        "  lines={} cut_moves={} arc_moves={} short_cut(<{:.2}mm)={} ({:.0}%)",
        rep.lines,
        rep.cut_moves,
        rep.arc_moves,
        l_min(),
        rep.short_cut_moves,
        if rep.cut_moves > 0 {
            100.0 * rep.short_cut_moves as f64 / rep.cut_moves as f64
        } else {
            0.0
        },
    );
    println!(
        "  worst arc radius mismatch = {:.4} mm (GRBL $12 limit {:.3})",
        rep.worst_radius_mismatch, GRBL_ARC_TOL
    );
    println!(
        "  F words emitted = {}, first cut effective feed = {:?}",
        rep.f_words_emitted, rep.first_cut_feed
    );
    if !rep.arc_radius_violations.is_empty() {
        println!(
            "  ARC RADIUS VIOLATIONS ({}):",
            rep.arc_radius_violations.len()
        );
        for v in rep.arc_radius_violations.iter().take(8) {
            println!("    {v}");
        }
    }
    if !rep.undefined_feed_lines.is_empty() {
        println!("  UNDEFINED FEED ({}):", rep.undefined_feed_lines.len());
        for v in rep.undefined_feed_lines.iter().take(8) {
            println!("    {v}");
        }
    }
    if !rep.nonfinite_lines.is_empty() {
        println!("  NON-FINITE ({}):", rep.nonfinite_lines.len());
        for v in rep.nonfinite_lines.iter().take(8) {
            println!("    {v}");
        }
    }
    rep
}

/// Diagnostic run — prints the defect profile for every shape, never fails.
/// Read its stdout (`cargo test ... -- --nocapture`) to see what GRBL rejects.
#[test]
fn phase0_contour_spiral_gcode_defect_profile() {
    let cases: Vec<(&str, Polygon2)> = vec![
        ("square60", square(60.0)),
        ("star5", star(5, 35.0, 14.0, 40.0, 40.0)),
        ("star7", star(7, 40.0, 13.0, 45.0, 45.0)),
    ];
    for (name, poly) in &cases {
        run_case(name, poly);
    }
}

/// The controller-fatal contract: no emitted arc may violate GRBL's `$12`
/// endpoint tolerance, no cutting move may carry a zero/undefined feed, and
/// no coordinate may be non-finite. This is the Phase-0 sentry — it should
/// FAIL on the current defect and PASS once the spiral export is fixed.
#[test]
fn phase0_contour_spiral_emits_grbl_valid_gcode() {
    let cases: Vec<(&str, Polygon2)> = vec![
        ("square60", square(60.0)),
        ("star5", star(5, 35.0, 14.0, 40.0, 40.0)),
        ("star7", star(7, 40.0, 13.0, 45.0, 45.0)),
    ];
    let mut total_arc_viol = 0usize;
    let mut total_undef_feed = 0usize;
    let mut total_nonfinite = 0usize;
    for (name, poly) in &cases {
        let rep = run_case(name, poly);
        total_arc_viol += rep.arc_radius_violations.len();
        total_undef_feed += rep.undefined_feed_lines.len();
        total_nonfinite += rep.nonfinite_lines.len();
    }
    assert_eq!(
        total_nonfinite, 0,
        "emitted G-code contains non-finite coordinates ({total_nonfinite} lines)"
    );
    assert_eq!(
        total_undef_feed, 0,
        "emitted G-code has cutting moves before any feed is set ({total_undef_feed} lines) — error:22"
    );
    assert_eq!(
        total_arc_viol, 0,
        "emitted G-code contains arcs that violate GRBL's $12 endpoint tolerance \
         ({total_arc_viol} arcs) — these throw error:33 on the machine"
    );
}
