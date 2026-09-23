//! G-CUTCARDS: the Inspector's cut-metric cards read the gate, and the time
//! series is a drawer that starts closed.
//!
//! Packages C and E of `planning/sim_cut_metrics_2026-09-23/PLAN.md`.
//!
//! # The defect this exists to catch
//!
//! The operator found the cut metrics hard to read (§1). Five co-equal time
//! series sat in the bottom panel with no limit on four of them, and the
//! limit rows stated one number. The fix puts one histogram per metric in
//! the Inspector, binned from the GATE's own population, and moves the time
//! series behind one toggle.
//!
//! Three things can break it silently:
//!
//! 1. A card that bins its own samples instead of the gate's. The card then
//!    shows mass outside the band while the badge says `Within` (§5).
//! 2. A chart built on `egui_plot`, which keeps its bounds per id from frame
//!    to frame and grows the 240 point rail.
//! 3. The "Signal graphs" disclosure or the normalised summary track coming
//!    back, or the drawer opening by default.
//!
//! # The arms
//!
//! - Arm A (source): the Inspector draws a "Cut metrics" section from the
//!   memo, the memo calls core's `metric_distribution` and
//!   `gate_population`, and the chart names no `egui_plot`.
//! - Arm B (behavioural): the chart draws in a headless `egui::Context` at
//!   the rail width, stays inside it, and a click on a bar returns that bin.
//!   The pure helpers word a bin and place it against the bounds.
//! - Arm C: `SimulationState::new()` has the drawer closed, and the
//!   timeline holds no "Signal graphs" disclosure and no summary track.
//!
//! # What arm B does not cover
//!
//! A behavioural arm that draws the whole section needs a `SimulationResults`
//! with a cut trace that `sim_trace_is_fresh` accepts. That result holds a
//! stock mesh, checkpoints and playback data; no viz test builds one today,
//! and a hand-built trace reads stale, so the section would draw only its
//! `NotMeasured` state. Arm A holds the section's wiring instead.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use rs_cam_core::tool_load::{Histogram, PopulationSample};
use rs_cam_viz::state::simulation::SimulationState;
use rs_cam_viz::ui::components::histogram::{
    self, BAR_HEIGHT, BinSide, DistributionChart, STUB_WIDTH,
};
use rs_cam_viz::ui::tokens;

const DIAGNOSTICS: &str = "ui/sim_diagnostics.rs";
const TIMELINE: &str = "ui/sim_timeline.rs";
const CHART: &str = "ui/components/histogram.rs";
const STATE: &str = "state/simulation.rs";

/// The rail width the Inspector opens at.
const RAIL_WIDTH: f32 = 240.0;

/// The tolerance for layout rounding, in points.
const SLACK: f32 = 0.5;

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn read(rel: &str) -> String {
    let path = src_root().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// `src` with every `//` comment removed, so a name quoted in a comment is
/// not read as code.
fn code_only(src: &str) -> String {
    src.lines()
        .map(|line| match line.find("//") {
            Some(i) => &line[..i],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The source of one function, from its `fn` line to the next item at
/// column zero.
fn function_source<'a>(src: &'a str, signature: &str) -> &'a str {
    let start = src
        .find(signature)
        .unwrap_or_else(|| panic!("{signature} is gone; the sentry's anchor is stale"));
    let rest = &src[start + signature.len()..];
    let end = rest
        .find("\n}\n")
        .unwrap_or_else(|| panic!("{signature} never closes at column zero"));
    &rest[..end]
}

/// A population from 0.01 to 0.10 mm/tooth: 10 samples below a 0.02
/// floor, 80 in the band and 10 above a 0.08 ceiling. Each sample weighs
/// 0.1 s and sits on its own move.
fn fixture() -> Histogram {
    let samples: Vec<PopulationSample> = (0..100)
        .map(|i| PopulationSample {
            value: 0.01 + 0.09 * f64::from(i) / 99.0,
            weight_s: 0.1,
            move_index: 1000 + i as usize,
            sample_index: i as usize,
        })
        .collect();
    Histogram::build(&samples, Some(0.02), Some(0.08), 24)
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

// ---------------------------------------------------------------------------
// Arm A — the source wiring
// ---------------------------------------------------------------------------

#[test]
fn the_inspector_draws_cut_metric_cards_from_the_gate_g_cutcards() {
    let diagnostics = code_only(&read(DIAGNOSTICS));
    let section = function_source(&diagnostics, "fn draw_cut_metrics_section(");
    assert!(
        section.contains("Cut metrics"),
        "the Inspector section must be titled \"Cut metrics\""
    );
    let view = function_source(&diagnostics, "fn cut_metrics_view(");
    assert!(
        view.contains("cached_cut_metrics("),
        "the section must read the memo, never build distributions per frame"
    );
    assert!(
        view.contains("focused_toolpath()"),
        "the section is scoped to the focused toolpath: the limits are per \
         toolpath, so a project-wide histogram cannot carry a band"
    );
    // Follow-up 2026-09-24: the card no longer paints the limit row as a
    // second line under its title. The operator found the cards too tall.
    // The row's facts moved into the hover of the status glyph, and that
    // hover is built from the same renderer parts `verdict_badge` uses, so
    // G-OWNBOUND still holds: no number in it is typed in the card.
    let card_fn = function_source(&diagnostics, "fn draw_cut_metric_card(");
    assert!(
        card_fn.contains("cut_metric_status_hover("),
        "each card carries its criterion's limit row in the status hover"
    );
    let hover_fn = function_source(&diagnostics, "fn cut_metric_status_hover(");
    for part in ["verdict_face(", "row_caption(", "verdict_tooltip("] {
        assert!(
            hover_fn.contains(part),
            "the status hover must build the limit row from `{part}`, the \
             part `verdict_badge` paints for Readiness (G-OWNBOUND)"
        );
    }
    assert!(
        card_fn.contains("ShowSeries(") && !card_fn.contains("See time series"),
        "the card title opens the metric's track; the card has no second \
         \"See time series\" link (the section keeps its one toggle)"
    );
    assert!(
        section.contains("Play or select a toolpath."),
        "with no toolpath in focus the section must say so"
    );
    assert!(
        section.contains("SimJumpToMove"),
        "a bar click must seek through the existing seek command"
    );
    assert!(
        section.contains("global_move_for_local("),
        "the population's move index is toolpath-local; the seek must \
         convert it, or a click seeks into the wrong toolpath"
    );
    assert!(
        !section.contains("AppEvent::RunSimulation"),
        "the section must not start a simulation; the workspace primary is \
         the one run route"
    );

    let card = function_source(&diagnostics, "fn draw_cut_metric_card(");
    assert!(
        card.contains("DistributionChart::new("),
        "each measured card draws the shared histogram component"
    );
    assert!(
        card.contains("NotMeasured::new()"),
        "a metric core could not measure draws the abstention mark, never \
         an empty histogram (X-VAC)"
    );
    assert!(
        hover_fn.contains("Depth is reported; the deflection limit decides."),
        "a depth reading with no cap (feeds ruling R2) must say which gate \
         decides, not draw an invented limit"
    );
    let caption = function_source(&diagnostics, "fn card_caption(");
    assert!(
        caption.contains("below_s > 0.0") && caption.contains("above_s > 0.0"),
        "the caption names only a share that is out of band; a \"0 % above \
         ceiling\" line is noise"
    );
    let glyph = function_source(&diagnostics, "fn status_glyph(");
    assert!(
        glyph.contains("no limit"),
        "a card with no bound states the absence in its status glyph"
    );
    assert!(
        glyph.contains("distribution.state"),
        "the status glyph takes its role from the GATE verdict, not from \
         the in-band share"
    );
    assert!(
        !card_fn.contains("StatusChip") && !card_fn.contains("Card::new("),
        "the card is a compact row group: no chip and no card frame"
    );

    let state = code_only(&read(STATE));
    let build = function_source(&state, "fn build_cut_metric_set(");
    for call in [
        "metric_distribution(",
        "gate_population(",
        "sim_trace_is_fresh(",
    ] {
        assert!(
            build.contains(call),
            "the memo must call core's {call}. The card must bin the gate's \
             own population; a copy of the filters drifts from the gate."
        );
    }
    let memo = function_source(&state, "pub fn cached_cut_metrics(");
    assert!(
        memo.contains("weak_matches(") && memo.contains("edit_counter"),
        "the memo is keyed by the trace identity and the edit counter, the \
         rule of cached_load_report"
    );
}

#[test]
fn the_chart_is_painted_not_plotted_g_cutcards() {
    let chart = code_only(&read(CHART));
    assert!(
        chart.contains("pub struct DistributionChart"),
        "{CHART} no longer defines DistributionChart; the sentry is stale"
    );
    assert!(
        !chart.contains("egui_plot"),
        "{CHART} uses egui_plot. A plot keeps its bounds per id from frame to \
         frame and brings axes the rail does not need (PLAN §4 C)."
    );
    assert!(
        !chart.contains("Color32::from_rgb("),
        "{CHART} names a colour literal; a component reads tokens only"
    );
}

// ---------------------------------------------------------------------------
// Arm B — the chart in a real pass
// ---------------------------------------------------------------------------

#[test]
fn the_chart_fits_the_rail_and_a_click_names_its_bin_g_cutcards() {
    let hist = fixture();
    let bins = hist.weights_s.len();
    assert_eq!(bins, 26, "24 inner bins and two overflow bins");

    let ctx = ctx();
    let mut chart_rect = egui::Rect::NOTHING;
    let mut available = 0.0_f32;
    let mut target = egui::Pos2::ZERO;
    let mut target_bin = 0usize;
    let mut clicked: Option<usize> = None;

    // Passes 0-2 settle the layout and name the target. Pass 3 presses and
    // pass 4 releases: egui resolves a click from the rects the previous
    // pass recorded.
    for pass in 0..5 {
        let mut events = Vec::new();
        if pass >= 3 {
            events.push(egui::Event::PointerMoved(target));
            events.push(egui::Event::PointerButton {
                pos: target,
                button: egui::PointerButton::Primary,
                pressed: pass == 3,
                modifiers: egui::Modifiers::default(),
            });
        }
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(RAIL_WIDTH, 400.0),
            )),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            ui.set_max_width(RAIL_WIDTH);
            available = ui.available_width();
            let shown = DistributionChart::new(&hist, "mm/tooth").show(ui);
            chart_rect = shown.response.rect;
            if shown.clicked_bin.is_some() {
                clicked = shown.clicked_bin;
            }
        });
        out.textures_delta.clear();
        if pass == 2 {
            // The middle of inner bin 12, half-way up the bars.
            target_bin = 12;
            let inner_left = chart_rect.left() + STUB_WIDTH;
            let inner_right = chart_rect.right() - STUB_WIDTH;
            let step = (inner_right - inner_left) / (bins - 2) as f32;
            let x = inner_left + step * (target_bin as f32 - 1.0 + 0.5);
            target = egui::pos2(x, chart_rect.top() + BAR_HEIGHT * 0.5);
        }
    }

    assert!(
        chart_rect.width() <= RAIL_WIDTH + SLACK,
        "the chart is {} points wide in a {RAIL_WIDTH} point rail; it must \
         take the available width and no more",
        chart_rect.width()
    );
    assert!(
        (chart_rect.width() - available).abs() <= SLACK,
        "the chart must fill the available width ({available} points), not {} points",
        chart_rect.width()
    );
    assert!(
        chart_rect.height() >= BAR_HEIGHT,
        "the bar area is {BAR_HEIGHT} points high"
    );
    assert_eq!(
        clicked,
        Some(target_bin),
        "a click on bin {target_bin} must name that bin, so the panel can \
         seek to its first sample"
    );
    assert!(
        hist.first_move_per_bin[target_bin].is_some(),
        "the fixture's target bin must hold a sample to seek to"
    );
}

#[test]
fn a_bin_is_placed_and_worded_against_the_gate_bounds_g_cutcards() {
    let hist = fixture();
    let last = hist.weights_s.len() - 1;

    // The inner range is the population's percentile range, 0.01 to 0.10,
    // in 24 bins of 0.00375. The first inner bin sits under the 0.02 floor,
    // the last inner bin over the 0.08 ceiling, and bin 12 in the band.
    assert_eq!(histogram::bin_side(&hist, 1), BinSide::Below);
    assert_eq!(histogram::bin_side(&hist, 12), BinSide::InBand);
    assert_eq!(histogram::bin_side(&hist, last - 1), BinSide::Above);
    let below: usize = (0..=last)
        .filter(|&bin| histogram::bin_side(&hist, bin) == BinSide::Below)
        .map(|bin| hist.counts[bin])
        .sum();
    let above: usize = (0..=last)
        .filter(|&bin| histogram::bin_side(&hist, bin) == BinSide::Above)
        .map(|bin| hist.counts[bin])
        .sum();
    assert!(
        below > 0 && above > 0,
        "the fixture has mass on both sides of the band; the chart must \
         colour it (below {below}, above {above})"
    );

    let text = histogram::bin_hover_text(&hist, 12, 1.0, "mm/tooth");
    for part in ["\u{2013}", "mm/tooth", "% of cut time", "samples"] {
        assert!(
            text.contains(part),
            "the bar hover must read `lo–hi unit · N % of cut time · M \
             samples`; {text:?} has no {part:?}"
        );
    }

    assert_eq!(histogram::format_share(0.2), "<1 %");
    assert_eq!(histogram::format_share(0.0), "0 %");
    assert_eq!(histogram::format_share(14.4), "14 %");
    assert_eq!(histogram::format_value(0.0312), "0.031");
}

/// Follow-up 2026-09-24: the x axis follows the data. A spindle-power
/// limit forty times the peak used to squash every sample into one bar at
/// the left edge. A far bound is now off scale at the edge, and a near
/// bound stays to scale.
#[test]
fn a_far_bound_is_off_scale_and_a_near_bound_is_to_scale_g_cutcards() {
    let samples: Vec<PopulationSample> = (0..50)
        .map(|i| PopulationSample {
            value: 0.01 + 0.01 * f64::from(i) / 49.0,
            weight_s: 0.1,
            move_index: i as usize,
            sample_index: i as usize,
        })
        .collect();
    let far = Histogram::build(&samples, None, Some(0.84), 24);
    assert!(
        *far.edges.last().unwrap() < 0.84,
        "core bins over the data; the far ceiling must not widen the edges"
    );
    assert_eq!(
        histogram::bound_place(&far, 0.84),
        histogram::BoundPlace::OffRight
    );
    let occupied = far.counts.iter().filter(|&&c| c > 0).count();
    assert!(
        occupied > 10,
        "the bars keep their resolution: {occupied} bins hold samples"
    );

    let near = Histogram::build(&samples, None, Some(0.0205), 24);
    assert_eq!(
        histogram::bound_place(&near, 0.0205),
        histogram::BoundPlace::ToScale,
        "a bound just past the data range stays to scale"
    );
    let low = Histogram::build(&samples, Some(0.0001), None, 24);
    assert_eq!(
        histogram::bound_place(&low, 0.0001),
        histogram::BoundPlace::OffLeft
    );

    // The chart still fits the rail with an off-scale marker.
    let ctx = ctx();
    let mut width = 0.0_f32;
    for _ in 0..2 {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.set_max_width(RAIL_WIDTH);
            width = DistributionChart::new(&far, "kW")
                .show(ui)
                .response
                .rect
                .width();
        });
        out.textures_delta.clear();
    }
    assert!(
        width <= RAIL_WIDTH + SLACK,
        "the chart is {width} points wide"
    );

    let stub = histogram::bin_hover_text(&far, far.weights_s.len() - 1, 1.0, "kW");
    assert!(
        stub.contains("Overflow bin"),
        "the overflow stub's hover must say what it is: {stub:?}"
    );
}

// ---------------------------------------------------------------------------
// Arm C — the drawer
// ---------------------------------------------------------------------------

#[test]
fn the_time_series_drawer_starts_closed_g_cutcards() {
    let sim = SimulationState::new();
    assert!(
        !sim.time_series_open,
        "the time-series drawer is closed by default (PLAN §3.3)"
    );
    assert!(sim.time_series_scroll_to.is_none());

    let timeline = code_only(&read(TIMELINE));
    assert!(
        timeline.contains("fn draw_time_series("),
        "{TIMELINE} no longer defines draw_time_series; the sentry is stale"
    );
    let drawer = function_source(&timeline, "fn draw_time_series(");
    assert!(
        drawer.contains("time_series_open"),
        "the drawer must draw nothing unless the Inspector opened it"
    );
    for gone in [
        "Signal graphs",
        "advance/tooth vs band max",
        "CollapsingHeader",
    ] {
        assert!(
            !timeline.contains(gone),
            "{TIMELINE} holds {gone:?} again. The drawer has one scroll area \
             and no disclosure, and the normalised summary track moved to \
             the chipload card."
        );
    }
    assert!(
        !drawer.contains("Polygon::new(") && timeline.matches("Polygon::new(").count() == 1,
        "the drawer shades no limit zone (PLAN §7 Q1); the file's one polygon \
         is the DepthPass band in draw_signal_track"
    );
}

/// Non-vacuity: every scanned file exists and holds real code.
#[test]
fn the_scan_is_not_vacuous_g_cutcards() {
    for (rel, floor) in [
        (DIAGNOSTICS, 20_000),
        (TIMELINE, 20_000),
        (CHART, 2_000),
        (STATE, 10_000),
    ] {
        let src = read(rel);
        assert!(
            src.len() >= floor,
            "{rel} is only {} bytes, under {floor}; an absence scan over it \
             would pass for the wrong reason",
            src.len()
        );
    }
    assert_eq!(
        code_only("let a = 1; // egui_plot").trim(),
        "let a = 1;",
        "the comment stripper is inert"
    );
}
