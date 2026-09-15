//! UP4 arm 2: the inspector asks for the same width on every tab.
//!
//! # The defect this exists to catch
//!
//! Seen on screen, 2026-09-14, and nothing else found it. On the Feeds tab
//! the whole right panel's content shifted OFF its own left edge: "Pin Drill"
//! rendered as "in Drill", "Geometry" as "etry", "Apply recommended speeds"
//! with its first character cut. On the Geometry tab, at the identical window
//! size and with the identical panel, nothing was clipped.
//!
//! The mechanism is `AUDIT.md` D-16. `TextWrapMode::Extend` sets an INFINITE
//! max width, so one long annotation — "configured 0.1313 mm/tooth" beside a
//! recommendation — grew the `Ui` past the panel. The panel then clamped to
//! its own maximum and drew the over-wide content right-aligned, which puts
//! its left end outside the clip rect.
//!
//! # Why the width, and not the pixels
//!
//! A screenshot diff would catch this once and then fail on every legitimate
//! change. The invariant that actually holds is narrower: **a tab switch
//! changes what the inspector SHOWS, never how wide it asks to be.** A tab is
//! a peer view of one object, so any tab that needs more width than its
//! siblings is a tab that will clip.
//!
//! This arm drives the real panel through a headless `Context` and compares
//! the width each tab requests.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::material::{Material, PlywoodGrade};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::{properties, tokens};

/// One tab's worst-case row: its name and how it builds.
type TabCase<'a> = (&'a str, &'a dyn Fn(&mut egui::Ui));

/// The default and maximum widths the app gives the inspector (`app.rs`).
const PANEL_WIDTH: f32 = 280.0;
const PANEL_MAX_WIDTH: f32 = 420.0;

fn properties_src() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/properties/mod.rs"))
        .unwrap_or_else(|error| panic!("read properties source: {error}"))
}

/// Return one function's lexical body rather than an arbitrary source window.
///
/// The root-width sentry must inspect `draw_toolpath_panel` itself: its long
/// signature makes a fixed byte range silently exclude the first statement.
fn function_body<'a>(source: &'a str, name: &str) -> &'a str {
    let signature = format!("fn {name}(");
    let start = source
        .find(&signature)
        .unwrap_or_else(|| panic!("{name} moved or was renamed"));
    let body_start = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .unwrap_or_else(|| panic!("{name} has no body"));
    let mut depth = 0_u32;
    for (offset, byte) in source[body_start..].bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[body_start..=body_start + offset];
                }
            }
            _ => {}
        }
    }
    panic!("{name} body is not closed");
}

fn feeds_sources() -> Vec<(PathBuf, String)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/feeds");
    std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("read {}: {error}", dir.display()))
        .map(|entry| {
            let path = entry
                .unwrap_or_else(|error| panic!("read feeds entry: {error}"))
                .path();
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            (path, source)
        })
        .collect()
}

/// Finds a bare `ui.label` in a `ui.horizontal` closure. This is intentionally
/// source-level: the rule is a layout API choice, and a new row must opt into
/// wrapping before it can reproduce the inspector-width defect.
fn bare_labels_in_horizontal_rows(source: &str) -> Vec<usize> {
    let mut findings = Vec::new();
    let lines: Vec<_> = source.lines().collect();
    let mut line = 0;
    while line < lines.len() {
        let current = lines[line];
        if !(current.contains("ui.horizontal(|ui|")
            || current.contains("ui.horizontal_wrapped(|ui|"))
        {
            line += 1;
            continue;
        }
        let indent = current.len() - current.trim_start().len();
        line += 1;
        while line < lines.len() {
            let current = lines[line];
            if current.len() - current.trim_start().len() == indent && current.trim() == "});" {
                break;
            }
            if current.contains("ui.label(") {
                findings.push(line + 1);
            }
            line += 1;
        }
        line += 1;
    }
    findings
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

/// The width a block of rows asks for when laid out inside `PANEL_WIDTH`.
///
/// `Ui::min_rect` is what a `Panel` reads to decide how wide it must be, so
/// this is the same number the real layout consults.
fn requested_width(ctx: &egui::Context, build: impl Fn(&mut egui::Ui)) -> f32 {
    let mut width = 0.0_f32;
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        ui.set_max_width(PANEL_WIDTH);
        let inner = ui.scope(|ui| {
            ui.set_max_width(PANEL_WIDTH);
            build(ui);
        });
        width = inner.response.rect.width();
    });
    out.textures_delta.clear();
    width
}

/// A label beside a long trailing annotation — the shape the Feeds tab uses
/// for "Recommended advance/tooth: 0.0710 mm/tooth · configured 0.1313".
fn feeds_fixture() -> AppState {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.flute_count = 2;

    let stock = StockConfig {
        material: Material::Plywood {
            grade: PlywoodGrade::BalticBirch,
        },
        ..Default::default()
    };

    // The warning shape from the observed Back Rough: its deliberately low
    // advance per tooth trips the rubbing floor while the LUT remains in play.
    let mut operation = OperationConfig::Adaptive3d(Default::default());
    if let OperationConfig::Adaptive3d(config) = &mut operation {
        config.feed_rate = 3_000.0;
        config.spindle_rpm = Some(18_000);
    }
    let config = ToolpathConfig {
        id: rs_cam_core::ToolpathId(0),
        name: "Back Rough width fixture".to_owned(),
        enabled: true,
        operation,
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: 1,
        model_id: 1,
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        face_selection: None,
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    };

    let model = LoadedModel {
        id: 1,
        path: PathBuf::from("back_rough_width_fixture.svg"),
        name: "Back Rough width fixture".to_owned(),
        kind: Some(ModelKind::Svg),
        mesh: None,
        polygons: Some(Arc::new(vec![Polygon2::rectangle(0.0, 0.0, 10.0, 10.0)])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    };
    let mut builder = ProjectSessionBuilder::new()
        .stock(stock)
        .tool(tool)
        .model(model);
    builder
        .add_toolpath(0, config)
        .expect("add Back Rough fixture");
    let mut state = AppState::new();
    state.session = builder.build();
    let id = state.session.toolpath_configs()[0].id;
    // The controller creates this runtime row when it loads or adds a
    // toolpath. Mirror that normal lifecycle so the production panel's
    // write-back can cache the recipe it calculated on the selected tab.
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    state.selection = Selection::Toolpath(id);
    state.gui.pending_toolpath_tab = Some((id, "feeds".to_owned()));
    state
}

#[derive(Debug, Default)]
struct RenderedFeedsPanel {
    requested_width: f32,
    max_left_clip: f32,
    rendered_rubbing_warning: bool,
}

/// Render the production inspector inside the same fixed-width Panel and
/// vertical ScrollArea used by the app, then report whether any painted text
/// starts to the left of its clip rectangle — the exact on-screen UR1 defect.
fn render_feeds_panel(ctx: &egui::Context, state: &mut AppState) -> RenderedFeedsPanel {
    render_feeds_panel_at(ctx, state, PANEL_WIDTH)
}

/// The same render at an arbitrary panel width. The Simulation workspace's
/// right rail defaults to 240 points — narrower than the Toolpaths rail —
/// so the Feeds tab must hold its content at BOTH widths.
fn render_feeds_panel_at(
    ctx: &egui::Context,
    state: &mut AppState,
    width: f32,
) -> RenderedFeedsPanel {
    let mut rendered = RenderedFeedsPanel::default();
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        egui::Panel::right("ur1_toolpath_properties")
            .default_size(width)
            .max_size(width.max(PANEL_MAX_WIDTH))
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let response = ui.scope(|ui| {
                        let mut events = Vec::new();
                        properties::draw(ui, state, &mut events);
                        assert!(
                            events.is_empty(),
                            "a width-only render must not emit UI events"
                        );
                    });
                    rendered.requested_width = response.response.rect.width();
                });
            });
    });
    for clipped in &out.shapes {
        if let egui::epaint::Shape::Text(text) = &clipped.shape {
            rendered.max_left_clip = rendered.max_left_clip.max(
                (clipped.clip_rect.min.x - clipped.shape.visual_bounding_rect().min.x).max(0.0),
            );
            rendered.rendered_rubbing_warning |= text
                .galley
                .job
                .text
                .contains("Commanded advance/tooth below rubbing floor");
        }
    }
    out.textures_delta.clear();
    rendered
}

fn annotated_row(ui: &mut egui::Ui, wrap: bool) {
    ui.horizontal_wrapped(|ui| {
        ui.label("0.0710 mm/tooth");
        let text = "configured 0.1313 mm/tooth from the vendor table, derated for hardness";
        if wrap {
            ui.add(
                egui::Label::new(egui::RichText::new(text).size(tokens::SIZE_CAPTION))
                    .wrap_mode(egui::TextWrapMode::Wrap),
            );
        } else {
            ui.add(
                egui::Label::new(egui::RichText::new(text).size(tokens::SIZE_CAPTION))
                    .wrap_mode(egui::TextWrapMode::Extend),
            );
        }
    });
}

#[test]
fn feeds_horizontal_rows_have_no_bare_labels_ur1() {
    let properties = properties_src();
    let card_start = properties
        .find("fn draw_feeds_card(")
        .expect("draw_feeds_card moved or was renamed");
    let card_end = properties[card_start..]
        .find("// ── Vendor LUT viewer")
        .map(|offset| card_start + offset)
        .expect("draw_feeds_card's following section moved or was renamed");
    let card = &properties[card_start..card_end];
    assert!(
        bare_labels_in_horizontal_rows(card).is_empty(),
        "draw_feeds_card has bare ui.label calls in horizontal rows at lines {:?}; \
         use a wrapping Label or wrapped_small_label so a caption cannot widen the inspector",
        bare_labels_in_horizontal_rows(card),
    );

    for (path, source) in feeds_sources() {
        let bare = bare_labels_in_horizontal_rows(&source);
        assert!(
            bare.is_empty(),
            "{} has bare ui.label calls in horizontal rows at lines {bare:?}; \
             horizontal captions must opt into wrapping",
            path.display(),
        );
    }
}

#[test]
fn inspector_root_caps_children_at_the_panel_width_ur1() {
    let properties = properties_src();
    let panel = function_body(&properties, "draw_toolpath_panel");
    let first_ui_call = panel
        .match_indices("ui.")
        .next()
        .map(|(offset, _)| &panel[offset..])
        .expect("draw_toolpath_panel no longer draws through its ui parameter");
    assert!(
        first_ui_call.starts_with("ui.set_max_width(ui.available_width());"),
        "the inspector's first root action no longer caps child requested widths"
    );
}

#[test]
fn real_warning_shaped_feeds_tab_stays_inside_its_panel_ur1() {
    let ctx = ctx();
    // The rubbing-floor message moved under UR4's one Why disclosure. This
    // layout sentry needs to exercise its content, not assert that collapsed
    // content is painted, so render all disclosures open for this fixture.
    ctx.memory_mut(|memory| memory.set_everything_is_visible(true));
    let mut state = feeds_fixture();
    let id = state.session.toolpath_configs()[0].id;

    // The production tab override is one-shot: the first frame consumes it,
    // calculates the recipe, and persists Feeds in egui memory. The next frame
    // renders the warning through the same steady-state path as the interactive
    // inspector.
    let _ = render_feeds_panel(&ctx, &mut state);
    let rendered = render_feeds_panel(&ctx, &mut state);
    let result = state
        .gui
        .toolpath_rt
        .get(&id)
        .and_then(|runtime| runtime.feeds_result.as_ref())
        .expect("the real Feeds tab must calculate and cache its recipe");
    assert!(
        result.warnings.iter().any(|warning| matches!(
            warning,
            rs_cam_core::feeds::FeedsWarning::ChiploadClampedToFloor { .. }
        )),
        "the Back Rough fixture must render its real rubbing-floor warning; warnings: {:?}",
        result.warnings
    );
    assert!(
        rendered.rendered_rubbing_warning,
        "the steady-state frame did not paint the real rubbing-floor warning"
    );
    assert!(
        rendered.max_left_clip <= 0.5,
        "the real warning-shaped Feeds tab painted text {:.2} points left of its panel clip \
         (requested width {:.2}; panel maximum {PANEL_MAX_WIDTH}). Its content must not slide \
         off the inspector's left edge.",
        rendered.max_left_clip,
        rendered.requested_width,
    );
}

#[test]
fn real_warning_shaped_feeds_tab_fits_the_240_point_rail_ur4() {
    // UR4 moved the canonical comparison into the inspector, and the
    // Simulation workspace's right rail is 240 points — 40 narrower than
    // the panel this file measured first. The modal-width compare grid
    // painted past that rail's left edge, which read on screen as the
    // viewport growing over the column. This arm renders the same real
    // fixture at the rail's own width.
    const SIMULATION_RAIL_WIDTH: f32 = 240.0;
    let ctx = ctx();
    ctx.memory_mut(|memory| memory.set_everything_is_visible(true));
    let mut state = feeds_fixture();
    let id = state.session.toolpath_configs()[0].id;

    let _ = render_feeds_panel_at(&ctx, &mut state, SIMULATION_RAIL_WIDTH);
    let rendered = render_feeds_panel_at(&ctx, &mut state, SIMULATION_RAIL_WIDTH);
    assert!(
        rendered.requested_width <= SIMULATION_RAIL_WIDTH + 0.5,
        "the Feeds tab asked for {:.2} points inside a {SIMULATION_RAIL_WIDTH} point rail \
         (panel maximum {PANEL_MAX_WIDTH}). The Simulation workspace's inspector draws at \
         this width; content must fit, not overflow under the viewport.",
        rendered.requested_width,
    );
    assert!(
        rendered.max_left_clip <= 0.5,
        "the Feeds tab painted text {:.2} points left of its panel clip at the \
         {SIMULATION_RAIL_WIDTH} point Simulation rail; its content must not slide under \
         the viewport.",
        rendered.max_left_clip,
    );
    let result = state
        .gui
        .toolpath_rt
        .get(&id)
        .and_then(|runtime| runtime.feeds_result.as_ref())
        .expect("the rail-width fixture must still calculate its recipe");
    assert!(
        result.warnings.iter().any(|warning| matches!(
            warning,
            rs_cam_core::feeds::FeedsWarning::ChiploadClampedToFloor { .. }
        )),
        "the rail fixture lost its real rubbing-floor warning; the width changed the path"
    );
}

#[test]
fn a_wrapping_annotation_stays_inside_the_panel_up4() {
    let ctx = ctx();
    let wrapped = requested_width(&ctx, |ui| annotated_row(ui, true));
    assert!(
        wrapped <= PANEL_WIDTH + 0.5,
        "a wrapping annotation asked for {wrapped} inside a {PANEL_WIDTH} \
         point panel. It must stay inside, or the panel clips it."
    );
}

#[test]
fn the_extend_mode_really_does_overflow_up4() {
    // Non-vacuity for the arm above. If `Extend` ever stopped overflowing,
    // the first arm would pass for the wrong reason and this programme would
    // believe a defect was fixed when the toolkit had merely changed.
    let ctx = ctx();
    let extended = requested_width(&ctx, |ui| annotated_row(ui, false));
    assert!(
        extended > PANEL_WIDTH,
        "Extend asked for only {extended} inside {PANEL_WIDTH}, so this test \
         no longer reproduces D-16's mechanism and the arm above proves \
         nothing. Re-derive it against the current egui."
    );
}

#[test]
fn every_tab_asks_for_the_same_width_up4() {
    let ctx = ctx();

    // Stand-ins for the four inspector tabs: each is a plausible worst row
    // for that tab, all built the way UP4 builds them.
    let tabs: [TabCase<'_>; 4] = [
        ("Geometry", &|ui: &mut egui::Ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Tool:");
                ui.label("6.00mm End Mill");
            });
        }),
        ("Feeds", &|ui: &mut egui::Ui| annotated_row(ui, true)),
        ("Linking", &|ui: &mut egui::Ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Retract (R):");
                ui.label("2.0 mm");
            });
        }),
        ("Heights", &|ui: &mut egui::Ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Clearance:");
                ui.add(
                    egui::Label::new(
                        egui::RichText::new("pinned, 12.0 mm above the stock top")
                            .size(tokens::SIZE_CAPTION),
                    )
                    .wrap_mode(egui::TextWrapMode::Wrap),
                );
            });
        }),
    ];

    let mut widest: Option<(&str, f32)> = None;
    for (name, build) in &tabs {
        let w = requested_width(&ctx, build);
        assert!(
            w <= PANEL_WIDTH + 0.5,
            "the {name} tab asked for {w} inside a {PANEL_WIDTH} point panel. \
             A tab is a peer view of one object: a tab that needs more width \
             than its siblings is a tab that will clip, and the operator sees \
             the panel's content slide off its own left edge."
        );
        if widest.is_none_or(|(_, best)| w > best) {
            widest = Some((name, w));
        }
    }
    assert!(widest.is_some(), "non-vacuity: no tab was measured");
}
