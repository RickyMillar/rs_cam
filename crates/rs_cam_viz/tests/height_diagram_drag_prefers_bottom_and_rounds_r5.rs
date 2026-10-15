//! R5: a drag on the Heights diagram takes the Bottom line when two lines
//! coincide, and it lands the pinned height on the 0.1 mm grid.
//!
//! # The defect this exists to catch
//!
//! `planning/corne_case_analysis_2026-09-18/ANALYSIS.md` §4.2. The saved
//! project carried `heights.top_z = manual −1.3704614721537909`. The only
//! writer of an un-rounded `Manual` value is the drag handler of the Heights
//! diagram, which wrote `current_z + dz` with `dz` derived from screen
//! pixels on every frame of a drag. On a waterline the Auto Top and the Auto
//! Bottom coincided (R3 has since moved the Auto Bottom of a zero-depth op
//! to the model bottom, so arm 3 builds the tie with a model whose bottom
//! sits at the stock top). The hit test took the FIRST line within the threshold
//! with a strict `<`, and Top (index 3) sits before Bottom (index 4), so a
//! drag on the shared line moved Top. The Auto Bottom then followed it. The
//! operator meant to drag the Bottom down to Z 0 and pinned the Top at
//! −1.37 instead. The waterline cut the bed.
//!
//! # What this test measures
//!
//! Arms 1 and 2 pin the two pure decisions, `pick_line` and `round_pin`,
//! so a layout change cannot move them.
//!
//! Arm 3 drives the real diagram through a headless `Context`: a press on
//! the shared Top/Bottom line of a zero-depth waterline, a drag down, and a
//! release. Bottom must be `Manual` on the 0.1 mm grid and Top must stay
//! `Auto`.
//!
//! Arm 4 is the non-vacuity guard for arm 3: with the two lines apart, a
//! drag on the Top line moves Top, not Bottom, so the tie-break is a
//! tie-break and not a constant.
//!
//! Arm 5 scans the row and the tab strip for the pin mark, the Auto release
//! and the INFO badge, each anchored on a line of code, not a comment.
//!
//! # NOT MEASURED
//!
//! What the row looks like on screen, and whether the click on `Auto`
//! reaches the command door. The panel's own snapshot diff owns that.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};

use rs_cam_viz::state::toolpath::{HeightContext, HeightMode, HeightsConfig, OperationType};
use rs_cam_viz::ui::properties::draw_height_diagram;
use rs_cam_viz::ui::properties::operations::{pick_line, round_pin};
use rs_cam_viz::ui::tokens;

/// The panel width the app gives the inspector (`app.rs`, `default_size`).
const PANEL_WIDTH: f32 = 280.0;

/// Clearance(0), Retract(1), Feed(2), Top(3), Bottom(4): the
/// `DiagramLine::index` order in `height_diagram.rs`.
const TOP: usize = 3;
const BOTTOM: usize = 4;

/// One pointer step in a drag, in points. Past egui's 6-point click
/// distance, so the drag starts on the first step, and inside the diagram's
/// 12-point hit threshold.
const DRAG_STEP_PX: f32 = 8.0;

// ── Arm 1: the tie-break ──────────────────────────────────────────────

#[test]
fn coincident_lines_pick_the_bottom_r5() {
    // The waterline case: every line under the pointer at distance zero.
    assert_eq!(pick_line(&[(TOP, 0.0), (BOTTOM, 0.0)]), Some(BOTTOM));
    assert_eq!(pick_line(&[(BOTTOM, 0.0), (TOP, 0.0)]), Some(BOTTOM));
    // All five coincide: the highest index wins.
    let all = [(0, 1.0), (1, 1.0), (2, 1.0), (TOP, 1.0), (BOTTOM, 1.0)];
    assert_eq!(pick_line(&all), Some(BOTTOM));
}

#[test]
fn the_nearest_line_wins_outside_the_tie_r5() {
    assert_eq!(pick_line(&[(TOP, 2.0), (BOTTOM, 6.0)]), Some(TOP));
    assert_eq!(pick_line(&[(BOTTOM, 6.0), (TOP, 2.0)]), Some(TOP));
    // 0.6 points apart is outside the 0.5-point tie.
    assert_eq!(pick_line(&[(TOP, 1.0), (BOTTOM, 1.6)]), Some(TOP));
    // 0.4 points apart is inside it, so the later line wins.
    assert_eq!(pick_line(&[(TOP, 1.0), (BOTTOM, 1.4)]), Some(BOTTOM));
    assert_eq!(pick_line(&[(BOTTOM, 1.4), (TOP, 1.0)]), Some(BOTTOM));
    // A lower index that is nearer by more than the tie keeps its line.
    assert_eq!(pick_line(&[(2, 0.0), (TOP, 3.0), (BOTTOM, 3.0)]), Some(2));
}

#[test]
fn no_candidate_picks_nothing_r5() {
    assert_eq!(pick_line(&[]), None);
    assert_eq!(pick_line(&[(1, 4.0)]), Some(1));
}

// ── Arm 2: the rounding ───────────────────────────────────────────────

#[test]
fn the_corne_value_rounds_to_one_decimal_r5() {
    // The value from the saved project, bit-exact against the printed one.
    assert_eq!(round_pin(-1.370_461_472_153_790_9), -1.4);
    assert_eq!(round_pin(2.25), 2.3);
    assert_eq!(round_pin(-2.25), -2.3);
    assert_eq!(round_pin(9.8), 9.8);
    assert_eq!(round_pin(0.0), 0.0);
}

#[test]
fn a_rounded_pin_never_carries_a_negative_zero_r5() {
    let v = round_pin(-0.04);
    assert_eq!(v, 0.0);
    assert!(
        v.is_sign_positive(),
        "round_pin(-0.04) is {v:?}; the project file would carry -0.0"
    );
}

#[test]
fn a_rounded_pin_sits_on_the_grid_r5() {
    for i in -400..400 {
        let raw = f64::from(i) * 0.037_3 + 0.001_1;
        let v = round_pin(raw);
        assert!((v - raw).abs() <= 0.05 + 1e-9, "{raw} rounded to {v}");
        assert_eq!(round_pin(v), v, "{v} is not a fixed point");
        // Ten times a grid value is an integer.
        assert!(((v * 10.0) - (v * 10.0).round()).abs() < 1e-9, "{v}");
    }
}

// ── Arm 3 and 4: the real diagram, driven headlessly ──────────────────

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    // epaint 0.36's `Drop for TexturesDelta` panics if the delta is dropped
    // unapplied, so every pass clears it before the output goes out of scope.
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

/// A waterline on a fresh stock. `op_depth` is the lever: at 0 the Auto
/// Bottom sits ON the Auto Top (the §4.2 shape); at 10 the two lines are
/// 10 mm apart, about 25 points on this canvas.
fn waterline_ctx(op_depth: f64) -> HeightContext {
    HeightContext {
        safe_z: 20.0,
        op_depth,
        stock_top_z: 9.8,
        stock_bottom_z: -17.2,
        model_top_z: Some(9.8),
        model_bottom_z: Some(0.0),
    }
}

/// One frame of the diagram with the given input events. Returns the shapes
/// the pass painted.
fn frame(
    ctx: &egui::Context,
    heights: &mut HeightsConfig,
    hctx: &HeightContext,
    events: Vec<egui::Event>,
) -> Vec<egui::epaint::ClippedShape> {
    let input = egui::RawInput {
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        ui.set_max_width(PANEL_WIDTH);
        draw_height_diagram(ui, heights, hctx, OperationType::Waterline);
    });
    let shapes = std::mem::take(&mut out.shapes);
    out.textures_delta.clear();
    shapes
}

/// The screen point at the middle of the height line drawn in `color`.
fn line_centre(shapes: &[egui::epaint::ClippedShape], color: egui::Color32) -> egui::Pos2 {
    for clipped in shapes {
        if let egui::Shape::LineSegment { points, stroke } = &clipped.shape
            && stroke.color == color
            && (points[0].y - points[1].y).abs() < 1e-3
        {
            return egui::pos2((points[0].x + points[1].x) * 0.5, points[0].y);
        }
    }
    panic!("the diagram drew no horizontal line in {color:?}");
}

fn moved(pos: egui::Pos2) -> egui::Event {
    egui::Event::PointerMoved(pos)
}

fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

/// Press on `start`, drag two steps of `dy` points, release.
fn drag(
    ctx: &egui::Context,
    heights: &mut HeightsConfig,
    hctx: &HeightContext,
    start: egui::Pos2,
    dy: f32,
) {
    let one = egui::pos2(start.x, start.y + dy);
    let two = egui::pos2(start.x, start.y + 2.0 * dy);
    frame(ctx, heights, hctx, vec![moved(start)]);
    frame(ctx, heights, hctx, vec![button(start, true)]);
    frame(ctx, heights, hctx, vec![moved(one)]);
    frame(ctx, heights, hctx, vec![moved(two)]);
    frame(ctx, heights, hctx, vec![button(two, false)]);
    // One idle frame, so a write that leaks past the drag stop shows here.
    frame(ctx, heights, hctx, vec![moved(two)]);
}

fn manual_value(mode: &HeightMode) -> f64 {
    let HeightMode::Manual(v) = *mode else {
        panic!("expected Manual, found {mode:?}");
    };
    v
}

#[test]
fn a_drag_on_the_shared_line_moves_the_bottom_and_lands_on_the_grid_r5() {
    let ctx = context();
    let mut hctx = waterline_ctx(0.0);
    // R3 (landed with this programme) resolves the Auto Bottom of a
    // zero-depth op to the model bottom, so the Corne tie no longer arises
    // from the resolver alone. A model whose bottom sits at the stock top
    // puts both Auto lines on one Y; the tie-break is what this arm pins.
    hctx.model_bottom_z = Some(hctx.stock_top_z);
    let mut heights = HeightsConfig::default();
    let auto = heights.resolve(&hctx);
    assert_eq!(auto.top_z, auto.bottom_z, "the fixture must coincide");

    // Top is CAUTION; Bottom on a waterline is DANGER at the same Y.
    let shapes = frame(&ctx, &mut heights, &hctx, Vec::new());
    let top = line_centre(&shapes, tokens::CAUTION);
    let bottom = line_centre(&shapes, tokens::DANGER);
    assert!(
        (top.y - bottom.y).abs() < 1e-3,
        "the two lines must coincide"
    );

    drag(&ctx, &mut heights, &hctx, top, DRAG_STEP_PX);

    assert!(
        heights.top_z.is_auto(),
        "the drag pinned Top ({:?}); the operator reached for Bottom",
        heights.top_z
    );
    let v = manual_value(&heights.bottom_z);
    assert!(
        v < auto.bottom_z,
        "a drag down must lower the Bottom: {v} vs auto {}",
        auto.bottom_z
    );
    assert_eq!(
        v,
        round_pin(v),
        "the pinned Bottom {v} is not on the 0.1 mm grid"
    );
    assert!(heights.clearance_z.is_auto());
    assert!(heights.retract_z.is_auto());
    assert!(heights.feed_z.is_auto());
}

#[test]
fn a_drag_on_a_separate_top_line_moves_the_top_r5() {
    let ctx = context();
    let hctx = waterline_ctx(10.0);
    let mut heights = HeightsConfig::default();
    let auto = heights.resolve(&hctx);

    let shapes = frame(&ctx, &mut heights, &hctx, Vec::new());
    let top = line_centre(&shapes, tokens::CAUTION);
    let bottom = line_centre(&shapes, tokens::DANGER);
    assert!(
        (bottom.y - top.y) > DRAG_STEP_PX,
        "the fixture must hold the two lines apart: top {} bottom {}",
        top.y,
        bottom.y
    );

    // Drag UP, away from the Bottom line.
    drag(&ctx, &mut heights, &hctx, top, -DRAG_STEP_PX);

    let v = manual_value(&heights.top_z);
    assert!(
        v > auto.top_z,
        "a drag up must raise the Top: {v} vs {}",
        auto.top_z
    );
    assert_eq!(
        v,
        round_pin(v),
        "the pinned Top {v} is not on the 0.1 mm grid"
    );
    assert!(
        heights.bottom_z.is_auto(),
        "the tie-break must not fire when the lines are apart: {:?}",
        heights.bottom_z
    );
}

// ── Arm 5: the row and the tab strip say so ───────────────────────────

fn src(rel: &str) -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Code lines only. A `//` comment cannot satisfy a scan.
fn code_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect()
}

#[test]
fn the_height_row_marks_a_pin_and_offers_auto_r5() {
    let text = src("ui/properties/operations/mod.rs");
    let lines = code_lines(&text);
    let has = |needle: &str| lines.iter().any(|l| l.contains(needle));
    assert!(has("(pinned)"), "the height row must print `(pinned)`");
    assert!(has("(auto)"), "the height row must still print `(auto)`");
    assert!(
        has("small_button(\"Auto\")"),
        "the height row must offer a one-click `Auto` release"
    );
    assert!(
        has("*mode = HeightMode::Auto"),
        "the `Auto` release must write `HeightMode::Auto`"
    );
}

#[test]
fn the_heights_tab_badge_reads_the_pin_r5() {
    let text = src("ui/properties/tab_badges.rs");
    let lines = code_lines(&text);
    let has = |needle: &str| lines.iter().any(|l| l.contains(needle));
    assert!(has("!mode.is_auto()"), "the badge must read the pin");
    assert!(
        has("then_some(crate::ui::tokens::INFO)"),
        "a pinned height takes the INFO badge, the neutral role"
    );
}

#[test]
fn the_diagram_wires_the_two_pure_decisions_r5() {
    let text = src("ui/properties/operations/height_diagram.rs");
    let lines = code_lines(&text);
    let has = |needle: &str| lines.iter().any(|l| l.contains(needle));
    assert!(
        has("pick_line(&candidates)"),
        "the hit test must call `pick_line`"
    );
    assert!(has("round_pin(v)"), "the drag stop must call `round_pin`");
    assert!(
        has("press_origin()"),
        "the drag must hit-test the press origin"
    );
}
