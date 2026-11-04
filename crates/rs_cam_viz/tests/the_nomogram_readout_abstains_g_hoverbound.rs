//! **G-HOVERBOUND — the nomogram readout reports where the pointer IS, or
//! it says nothing.**
//!
//! # The defect
//!
//! Reported by the operator on 2026-09-16 — *"the readout reads where the
//! cursor is, even if the cursor is off the graph"* — and caught in the same
//! capture that was taken to verify the label fix. The readout line under the
//! nomogram read:
//!
//! ```text
//! 37891 RPM · 0 mm/min → commanded advance/tooth 0.0000 mm/tooth ·
//! BURN risk (below band) · past spindle cap
//! ```
//!
//! painted in the DANGER colour, on a chart whose RPM axis stops at 27 600
//! (the 24 000 spindle cap plus the 15 % overrun the plot draws). The pointer
//! was over the inspector panel, not over the chart. There was no operating
//! point at all.
//!
//! `egui_plot::PlotUi::pointer_coordinate` answers for a pointer outside the
//! drawn bounds, so the readout transformed a position that is not on the
//! chart into a feed, an advance per tooth and a burn verdict.
//!
//! # Why this class matters here
//!
//! This is the failure shape this repo has hit repeatedly and names
//! explicitly: **absence rendered as a reading.** An empty triage drawn as an
//! all-clear, a 2D model's zero Z drawn as a blank frame, an empty chart
//! frame that looks like a chart — and now a hazard verdict manufactured from
//! a pointer that is somewhere else entirely. The abstention is the correct
//! output, and it has to be the output.
//!
//! # The bound is the AXIS, not the machine
//!
//! The strip between the spindle cap and the axis maximum is a real place to
//! hover: it is how `· past spindle cap` is read, and that clause is
//! deliberate. So the guard is the plot's own bounds. Arm 3 pins that
//! distinction, because tightening the guard to the machine's cap would
//! silently delete a working feature.
//!
//! # W3i (2026-10-30): the readout is the plot hover
//!
//! The chart is inline on the Feeds tab, and the readout is the hover of
//! the plot. A pointer off the chart opens no hover, so it paints no
//! reading; a pointer on the chart paints it.
//!
//! # W3k (2026-10-31): one line, and no navigation
//!
//! The legend left the hover for labels on the chart; the hover is one line
//! (`RPM · mm/min · mm/t`). The operator found that the hover sometimes
//! stopped. Measured headless: a short drag on the RPM tick labels zoomed
//! the x axis (egui_plot's `allow_axis_zoom_drag`, on by default) to about
//! -100000, and the plot memory kept that zoom. Every pointer position then
//! read outside the axes, so the readout abstained on the whole chart. Arm 4
//! drags the ticks, right-drags a box, double-clicks and scrolls, and holds
//! the readout and the tick labels.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::tool_config::ToolFact;
use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::material::{Material, PlywoodGrade};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::AppState;
use rs_cam_viz::ui::{properties, tokens};

/// Room for the whole window, so the plot is drawn unclipped.
// P5b: the merged Passes tab is taller than 1000 points, so a wheel there
// scrolled the panel and moved the chart off the pointer. A tall screen has
// nothing to scroll: the wheel arm then tests the chart alone.
const SCREEN: egui::Vec2 = egui::Vec2::new(1400.0, 2400.0);

/// The inspector width the chart draws in.
const PANEL_WIDTH: f32 = 340.0;

/// One frame of the inspector on the Feeds tab, with the pointer at
/// `pointer` (when given). Tooltips show at once.
fn frame(
    ctx: &egui::Context,
    state: &mut AppState,
    pointer: Option<egui::Pos2>,
) -> egui::FullOutput {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
        events: pointer
            .map(|p| vec![egui::Event::PointerMoved(p)])
            .unwrap_or_default(),
        ..Default::default()
    };
    ctx.run_ui(input, |ui| {
        egui::Panel::right("hoverbound_properties")
            .default_size(PANEL_WIDTH)
            .max_size(PANEL_WIDTH)
            .resizable(false)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut events = Vec::new();
                    properties::draw(ui, state, &mut events);
                });
            });
    })
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    ctx.all_styles_mut(|style| style.interaction.tooltip_delay = 0.0);
    ctx
}

/// A readout of a real operating point contains this phrase.
///
/// It was `" RPM · "`, which is too loose: the legend's `● Now` row read
/// `17000 RPM · 1291 mm/min` and matched it. W3k deleted the legend; the
/// readout is `<rpm> RPM · <feed> mm/min · <chipload> mm/t`, and no other
/// text on the Feeds tab holds a feed followed by a chipload.
const READING_MARK: &str = " mm/min \u{00B7} ";

fn fixture() -> AppState {
    let mut tool = ToolConfig::new_default(ToolId::from_raw(1), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.set_flute_count(ToolFact::Stated(2));

    let stock = StockConfig {
        material: Material::Plywood {
            grade: PlywoodGrade::BalticBirch,
        },
        ..Default::default()
    };

    let mut operation = OperationConfig::Pocket(Default::default());
    if let OperationConfig::Pocket(config) = &mut operation {
        config.feed_rate = Some(1_291.0);
        config.spindle_rpm = Some(17_000);
    }
    let config = ToolpathConfig {
        shoe: Default::default(),
        feeds_lock: Default::default(),
        id: rs_cam_core::ToolpathId::from_raw(0),
        name: "Hover bound fixture".to_owned(),
        enabled: true,
        operation,
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: Some(rs_cam_core::ids::ToolId::from_raw(1)),
        model_id: rs_cam_core::ids::ModelId::from_raw(1),
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    };

    let model = LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(1),
        path: PathBuf::from("hover_bound_fixture.svg"),
        name: "Hover bound fixture".to_owned(),
        kind: Some(ModelKind::Svg),
        mesh: None,
        items: Arc::new(rs_cam_core::selection::ItemSet::from_polygons(vec![
            Polygon2::rectangle(0.0, 0.0, 10.0, 10.0),
        ])),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    };
    let mut builder = ProjectSessionBuilder::new()
        .stock(stock)
        .tool(tool)
        .model(model);
    builder
        .add_toolpath(0, config)
        .expect("add the hover bound fixture");
    let mut state = AppState::new();
    state.replace_session(builder.build());
    let toolpath_id = state.session().toolpath_configs()[0].id;
    // W3i: the chart is on the Feeds tab of the selected toolpath.
    state.gui.toolpath_rt.insert(
        toolpath_id,
        rs_cam_viz::state::runtime::ToolpathRuntime::new(true),
    );
    let _ = state.set_focus(rs_cam_viz::state::selection::Selection::Toolpath(
        toolpath_id,
    ));
    state.gui.pending_toolpath_tab =
        Some((toolpath_id, rs_cam_viz::ui::properties::ToolpathTab::Passes));
    state
}

/// Draw the window with the pointer parked at `pointer`, and return every
/// text run it painted.
///
/// The pointer is re-sent every frame: egui only keeps a pointer position
/// while it is being told about one, and the plot needs a settled layout
/// before the coordinate it reports means anything.
fn painted_with_pointer(pointer: egui::Pos2) -> Vec<String> {
    let ctx = ctx();
    let mut state = fixture();
    let mut texts = Vec::new();
    for pass in 0..6 {
        let mut out = frame(&ctx, &mut state, Some(pointer));
        if pass == 5 {
            for clipped in &out.shapes {
                if let egui::epaint::Shape::Text(text) = &clipped.shape {
                    texts.push(text.galley.job.text.clone());
                }
            }
        }
        out.textures_delta.clear();
    }
    texts
}

/// A point inside the plot of the nomogram, read from the painted frame:
/// 40 points above the "10000" RPM tick, at the x of that tick. W3i: the
/// chart is inline on the Feeds tab, so the point follows the layout.
fn a_point_on_the_chart() -> egui::Pos2 {
    let ctx = ctx();
    let mut state = fixture();
    let mut tick = None;
    for pass in 0..3 {
        let mut out = frame(&ctx, &mut state, None);
        if pass == 2 {
            for clipped in &out.shapes {
                if let egui::epaint::Shape::Text(text) = &clipped.shape
                    && text.galley.job.text == "10000"
                {
                    tick = Some(text.visual_bounding_rect());
                }
            }
        }
        out.textures_delta.clear();
    }
    let tick = tick.expect("the 10000 RPM tick paints");
    egui::pos2(tick.center().x, tick.top() - 40.0)
}

// ── arm 1 — the pointer is off the chart ─────────────────────────────────

#[test]
fn the_readout_abstains_when_the_pointer_is_off_the_chart_g_hoverbound() {
    // Top-left of the screen: outside the inspector and the chart.
    let texts = painted_with_pointer(egui::pos2(4.0, 4.0));
    let readings: Vec<&String> = texts.iter().filter(|t| t.contains(READING_MARK)).collect();
    assert!(
        readings.is_empty(),
        "the readout reported an operating point for a pointer that is not \
         on the chart: {readings:?}. `pointer_coordinate` answers for a \
         pointer outside the drawn bounds; a verdict built from one is a \
         hazard reading manufactured from an absence."
    );
}

// ── arm 2 — the pointer is on the chart ──────────────────────────────────

/// Non-vacuity, and the half that stops arm 1 being satisfied by deleting
/// the readout altogether.
#[test]
fn the_readout_reports_when_the_pointer_is_on_the_chart_g_hoverbound() {
    let texts = painted_with_pointer(a_point_on_the_chart());
    assert!(
        texts.iter().any(|t| t.contains(READING_MARK)),
        "the readout said nothing for a pointer ON the chart, so arm 1 passes \
         for the wrong reason — a readout that never reports satisfies it. If \
         the window's layout moved, re-derive `a_point_on_the_chart`. Painted \
         runs: {texts:?}"
    );
    assert!(
        texts
            .iter()
            .any(|t| t.contains(READING_MARK) && t.ends_with("mm/t")),
        "W3k: the readout is one line, `RPM · mm/min · mm/t`. Painted runs: {texts:?}"
    );
}

// ── arm 3 — the guard is the axis, not the machine cap ───────────────────

/// `· past spindle cap` is a deliberate clause: the plot draws 15 % past the
/// machine's cap so an operator can hover into the forbidden strip and be
/// told. A guard tightened to the machine's cap instead of the axis bound
/// would delete that feature silently, so the source says which it is.
#[test]
fn the_hover_guard_uses_the_axis_bound_g_hoverbound() {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/feeds/explore.rs"),
    )
    .expect("read explore source");
    let at = source
        .find("pointer_coordinate()")
        .expect("the hover readout moved or was renamed");
    let guard = &source[at..(at + 400).min(source.len())];
    assert!(
        guard.contains("rpm_axis_max") && guard.contains("feed_axis_max"),
        "the hover guard no longer bounds against the plot's own axes. \
         Bounding against the machine cap instead would delete the \
         `past spindle cap` readout, which is the one thing the 15 % overrun \
         strip exists to show."
    );
    assert!(
        source.contains("past spindle cap"),
        "the `past spindle cap` clause is gone; the overrun strip now reports \
         nothing and the axis bound has nothing left to protect"
    );
}

// ── arm 4 — no navigation moves the axes (W3k) ───────────────────────────

/// One frame with these input events.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: foreign enum egui Shape; the arm skips every shape that this test does not read"
)]
fn frame_with(ctx: &egui::Context, state: &mut AppState, events: Vec<egui::Event>) -> Vec<String> {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        egui::Panel::right("hoverbound_properties")
            .default_size(PANEL_WIDTH)
            .max_size(PANEL_WIDTH)
            .resizable(false)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut events = Vec::new();
                    properties::draw(ui, state, &mut events);
                });
            });
    });
    let texts = out
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::epaint::Shape::Text(t) => Some(t.galley.job.text.clone()),
            _ => None,
        })
        .collect();
    out.textures_delta.clear();
    texts
}

fn button(pos: egui::Pos2, button: egui::PointerButton, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Default::default(),
    }
}

/// A drag with `which` from `from` by `by`, over ten frames.
fn drag(
    ctx: &egui::Context,
    state: &mut AppState,
    which: egui::PointerButton,
    from: egui::Pos2,
    by: egui::Vec2,
) {
    let _ = frame_with(
        ctx,
        state,
        vec![egui::Event::PointerMoved(from), button(from, which, true)],
    );
    for k in 1..=10 {
        let at = from + by * (k as f32 / 10.0);
        let _ = frame_with(ctx, state, vec![egui::Event::PointerMoved(at)]);
    }
    let _ = frame_with(ctx, state, vec![button(from + by, which, false)]);
}

/// The RPM tick labels on the face.
fn rpm_ticks(texts: &[String]) -> Vec<String> {
    ["0", "10000", "20000"]
        .iter()
        .filter(|t| texts.iter().any(|s| s == *t))
        .map(|t| (*t).to_owned())
        .collect()
}

/// The pointer reads the chart after each navigation gesture: it moves to
/// `at` and rests there (a tooltip waits for a still pointer).
fn reads(ctx: &egui::Context, state: &mut AppState, at: egui::Pos2) -> bool {
    let mut texts = Vec::new();
    for _ in 0..8 {
        texts = frame_with(ctx, state, vec![egui::Event::PointerMoved(at)]);
    }
    texts.iter().any(|t| t.contains(READING_MARK))
}

/// W3k: a drag on the tick labels, a right drag, a double click and a
/// scroll leave the axes where they were, and the readout still answers.
/// Before W3k the tick drag zoomed the x axis and the readout abstained
/// on the whole chart.
#[test]
fn no_gesture_moves_the_axes_w3k() {
    let at = a_point_on_the_chart();
    let ctx = ctx();
    let mut state = fixture();
    assert!(reads(&ctx, &mut state, at), "the readout must answer first");
    let ticks = rpm_ticks(&frame_with(&ctx, &mut state, Vec::new()));
    assert_eq!(
        ticks.len(),
        3,
        "the RPM ticks 0, 10000, 20000 paint: {ticks:?}"
    );

    // The "10000" tick sits 40 points under `at`.
    let tick = at + egui::vec2(0.0, 40.0);
    drag(
        &ctx,
        &mut state,
        egui::PointerButton::Primary,
        tick,
        egui::vec2(-100.0, 0.0),
    );
    assert!(
        reads(&ctx, &mut state, at),
        "a drag on the RPM ticks stopped the readout"
    );
    drag(
        &ctx,
        &mut state,
        egui::PointerButton::Secondary,
        at,
        egui::vec2(40.0, 30.0),
    );
    assert!(
        reads(&ctx, &mut state, at),
        "a right drag stopped the readout"
    );
    drag(
        &ctx,
        &mut state,
        egui::PointerButton::Primary,
        at,
        egui::vec2(-50.0, 20.0),
    );
    for _ in 0..2 {
        let _ = frame_with(
            &ctx,
            &mut state,
            vec![button(at, egui::PointerButton::Primary, true)],
        );
        let _ = frame_with(
            &ctx,
            &mut state,
            vec![button(at, egui::PointerButton::Primary, false)],
        );
    }
    let _ = frame_with(
        &ctx,
        &mut state,
        vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, -3.0),
            modifiers: Default::default(),
            phase: egui::TouchPhase::Move,
        }],
    );
    assert!(reads(&ctx, &mut state, at), "a gesture stopped the readout");
    assert_eq!(
        rpm_ticks(&frame_with(&ctx, &mut state, Vec::new())),
        ticks,
        "a gesture moved the RPM axis"
    );
}

/// P5c (operator review 2026-11-04, item 8: "feed chart has no y axis
/// numbers"): on the rivmap100 3D Rough the feed axis names its ticks, at
/// the 340-point inspector and at the 240-point rail. A feed tick is a painted number of at least 100
/// left of the centre of the RPM axis title, above the RPM tick row and in
/// the 220 points above the title (the chart).
#[test]
fn the_feed_axis_names_its_ticks_p5c() {
    for width in [PANEL_WIDTH, 240.0] {
        let ctx = ctx();
        let mut state = rivmap_rough();
        let mut texts = Vec::new();
        for _ in 0..4 {
            texts = placed_texts(&ctx, &mut state, width);
        }
        // The RPM axis title is under the chart (the topmost "RPM"; the
        // card under the chart has an RPM row). The feed ticks are left of
        // its centre, above the RPM tick row, in the chart band.
        let title = texts
            .iter()
            .filter(|(t, _)| t == "RPM")
            .map(|(_, r)| *r)
            .min_by(|a, b| a.top().total_cmp(&b.top()))
            .unwrap_or_else(|| panic!("{width}: the RPM axis title paints: {texts:?}"));
        let feed: Vec<&String> = texts
            .iter()
            .filter(|(t, r)| {
                t.parse::<f64>().is_ok_and(|v| v >= 100.0)
                    && r.right() < title.center().x
                    && r.center().y < title.top() - 20.0
                    && r.center().y > title.top() - 220.0
            })
            .map(|(t, _)| t)
            .collect();
        assert!(
            feed.len() >= 2,
            "{width}: the feed axis names at least two ticks: {feed:?}"
        );
    }
}

/// The rivmap100 3D Rough on its Passes tab (the operator's review job).
/// Its machine feed cap puts the feed axis near 10000 mm/min: the default
/// log-10 grid then named only 0 (a 1000 step is about 17 points apart).
fn rivmap_rough() -> AppState {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../rs_cam_core/tests/fixtures/rivmap100_rough_2026-11-04_7effc804/rivmap100.toml"
    );
    let session = rs_cam_core::session::ProjectSession::load(std::path::Path::new(path))
        .expect("the rivmap100 fixture loads");
    let mut state = AppState::new();
    state.replace_session(session);
    let id = state
        .session()
        .toolpath_configs()
        .iter()
        .find(|t| t.name == "3D Rough")
        .expect("rivmap100 has the 3D Rough")
        .id;
    state
        .gui
        .toolpath_rt
        .insert(id, rs_cam_viz::state::runtime::ToolpathRuntime::new(true));
    let _ = state.set_focus(rs_cam_viz::state::selection::Selection::Toolpath(id));
    state.gui.pending_toolpath_tab = Some((id, rs_cam_viz::ui::properties::ToolpathTab::Passes));
    state
}

/// One frame of the inspector `width` points wide: each painted text with
/// its screen rect.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: foreign enum egui Shape; the arm skips every shape that this test does not read"
)]
fn placed_texts(
    ctx: &egui::Context,
    state: &mut AppState,
    width: f32,
) -> Vec<(String, egui::Rect)> {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        egui::Panel::right("hoverbound_properties")
            .default_size(width)
            .min_size(width)
            .max_size(width)
            .resizable(false)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut events = Vec::new();
                    properties::draw(ui, state, &mut events);
                });
            });
    });
    let texts = out
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::epaint::Shape::Text(t) => {
                Some((t.galley.job.text.clone(), t.visual_bounding_rect()))
            }
            _ => None,
        })
        .collect();
    out.textures_delta.clear();
    texts
}
