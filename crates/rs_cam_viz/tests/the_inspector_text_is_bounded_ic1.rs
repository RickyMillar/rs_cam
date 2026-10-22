//! IC1: the inspector shows fields, not paragraphs.
//!
//! # The defect this exists to catch
//!
//! The operator, 2026-10-13: "the inspector panel is obscene. there is so
//! much text." Each tab printed explanations, provenance sentences and
//! notes under its fields. The clean-up moved explanations into hover text
//! and provenance into badges. This sentry keeps the result.
//!
//! # What it measures
//!
//! The test renders the real inspector (`properties::draw`) in the app's
//! 280-point panel for a pocket, a 3D rough and a 3D finish on each tab, and
//! for the other inspector panels. It reads every painted text galley. Then
//! it asserts two rules:
//!
//! 1. No galley wraps to more than one row, except a refusal or an error
//!    that blocks work (`ALLOWED_MULTI_LINE`).
//! 2. The number of visible text rows on each surface stays under a cap.
//!    The caps come from the after-state with a small margin.
//!
//! Run it with `--nocapture` to print the inventory of each surface.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::ToolpathId;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{HeightMode, HeightsConfig};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::job::ModelId;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::properties::{self, ToolpathTab};
use rs_cam_viz::ui::tokens;

/// The default inspector width (`app.rs`).
const PANEL_WIDTH: f32 = 280.0;

/// The Simulation workspace's right rail, the narrowest inspector.
const RAIL_WIDTH: f32 = 240.0;

const TOOL: usize = 1;
const MODEL_2D: usize = 4;
const MODEL_3D: usize = 5;

/// Text that may wrap to more than one row: a refusal or an error that
/// blocks work. Such text paints in the danger colour, and only such text
/// does, so the colour is the test.
fn may_wrap(painted: &Painted) -> bool {
    painted.color == Some(tokens::DANGER)
}

fn polygon_model(id: usize) -> LoadedModel {
    LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(id),
        path: PathBuf::from(format!("model_{id}.svg")),
        name: format!("Outline {id}"),
        kind: Some(ModelKind::Svg),
        mesh: None,
        items: Arc::new(rs_cam_core::selection::ItemSet::from_polygons(vec![
            Polygon2::rectangle(-10.0, -10.0, 10.0, 10.0),
        ])),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    }
}

fn mesh_model(id: usize) -> LoadedModel {
    LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(id),
        path: PathBuf::from(format!("model_{id}.stl")),
        name: format!("Mesh {id}"),
        kind: Some(ModelKind::Stl),
        mesh: Some(Arc::new(rs_cam_core::mesh::make_test_flat(20.0))),
        items: Default::default(),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    }
}

fn toolpath(name: &str, model_id: usize, operation: OperationConfig) -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        id: ToolpathId::from_raw(0),
        name: name.to_owned(),
        enabled: true,
        operation,
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: rs_cam_core::ids::ToolId::from_raw(TOOL),
        model_id: rs_cam_core::ids::ModelId::from_raw(model_id),
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
    }
}

/// One 6 mm end mill, one outline, one mesh, an 18 mm board and three
/// operations: a pocket, a 3D rough and a 3D finish.
fn fixture() -> AppState {
    let mut tool = ToolConfig::new_default(ToolId::from_raw(TOOL), ToolType::EndMill);
    tool.diameter = 6.0;
    let stock = StockConfig {
        z: 18.0,
        auto_from_model: false,
        ..StockConfig::default()
    };
    let mut builder = ProjectSessionBuilder::new()
        .tool(tool)
        .model(polygon_model(MODEL_2D))
        .model(mesh_model(MODEL_3D))
        .stock(stock);
    for (name, model, op, heights) in [
        (
            "Pocket",
            MODEL_2D,
            // A spindle override, so the Feeds tab shows the overridden
            // Spindle row.
            OperationConfig::Pocket(rs_cam_core::compute::operation_configs::PocketConfig {
                spindle_rpm: Some(18_000),
                ..Default::default()
            }),
            // One pinned row, so the Heights tab shows its Auto button.
            HeightsConfig {
                top_z: HeightMode::Manual(18.0),
                ..HeightsConfig::default()
            },
        ),
        (
            "3D Rough",
            MODEL_3D,
            OperationConfig::Adaptive3d(Default::default()),
            HeightsConfig::default(),
        ),
        (
            "3D Finish",
            MODEL_3D,
            OperationConfig::DropCutter(Default::default()),
            HeightsConfig::default(),
        ),
    ] {
        let mut config = toolpath(name, model, op);
        config.heights = heights;
        builder
            .add_toolpath(0, config)
            .expect("add a fixture toolpath");
    }
    let mut state = AppState::new();
    state.replace_session(builder.build());
    let ids: Vec<_> = state
        .session()
        .toolpath_configs()
        .iter()
        .map(|c| c.id)
        .collect();
    for id in ids {
        state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    }
    state
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

/// One painted text galley.
#[derive(Debug)]
struct Painted {
    text: String,
    rows: usize,
    /// The screen bands of its rows, top and bottom.
    bands: Vec<(f32, f32)>,
    /// How far the text paints past the right edge of its clip rectangle.
    overflow: f32,
    /// The left and right screen edges of the text.
    span: (f32, f32),
    /// The colour of the first section.
    color: Option<egui::Color32>,
}

/// The number of visible text lines: the row bands of every galley, merged
/// where they overlap. A row of label, value and unit is ONE line.
fn visible_lines(painted: &[Painted]) -> usize {
    let mut bands: Vec<(f32, f32)> = painted.iter().flat_map(|p| p.bands.clone()).collect();
    bands.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut lines = 0;
    let mut bottom = f32::NEG_INFINITY;
    for (top, low) in bands {
        // Two bands on one line overlap by most of a row; adjacent lines
        // touch or overlap by a pixel or two.
        if top < bottom - 3.0 {
            bottom = bottom.max(low);
        } else {
            lines += 1;
            bottom = low;
        }
    }
    lines
}

/// One frame of the inspector: its painted text, and how much wider than
/// the scroll area its content asked to be.
struct Frame {
    painted: Vec<Painted>,
    excess_width: f32,
}

/// Render the inspector the way the app does and return its painted text.
///
/// egui does not paint a widget outside the clip rectangle, so a row that is
/// too wide hides its tail instead of painting it. `excess_width` catches
/// that case: the content's own width against the width it was given.
fn render(ctx: &egui::Context, state: &mut AppState, width: f32) -> Frame {
    let mut excess_width = 0.0_f32;
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 6000.0),
        )),
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        egui::Panel::right("ic1_inspector")
            .default_size(width)
            .max_size(width)
            .resizable(false)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let given = ui.available_width();
                    let used = ui
                        .scope(|ui| {
                            let mut events = Vec::new();
                            properties::draw(ui, state, &mut events);
                        })
                        .response
                        .rect
                        .width();
                    excess_width = used - given;
                });
            });
    });
    let painted = out
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::Text(text) if !text.galley.job.text.trim().is_empty() => {
                let bands = text
                    .galley
                    .rows
                    .iter()
                    .map(|row| {
                        let rect = row.rect();
                        (text.pos.y + rect.min.y, text.pos.y + rect.max.y)
                    })
                    .collect();
                Some(Painted {
                    text: text.galley.job.text.clone(),
                    rows: text.galley.rows.len(),
                    bands,
                    overflow: clipped.shape.visual_bounding_rect().max.x - clipped.clip_rect.max.x,
                    span: (
                        clipped.shape.visual_bounding_rect().min.x,
                        clipped.shape.visual_bounding_rect().max.x,
                    ),
                    color: text.galley.job.sections.first().map(|s| s.format.color),
                })
            }
            _ => None,
        })
        .collect();
    out.textures_delta.clear();
    Frame {
        painted,
        excess_width,
    }
}

/// Render one selection twice (the first frame settles one-shot state) and
/// return the second frame's text.
fn surface(
    state: &mut AppState,
    selection: Selection,
    tab: Option<ToolpathTab>,
    width: f32,
) -> Frame {
    let ctx = ctx();
    if let (Selection::Toolpath(id), Some(tab)) = (&selection, tab) {
        state.gui.pending_toolpath_tab = Some((*id, tab));
    }
    state.selection = selection;
    let _ = render(&ctx, state, width);
    render(&ctx, state, width)
}

/// One surface the sentry reads: a name, the selection, the toolpath tab,
/// the panel width and the cap on visible text lines.
type Surface = (String, Selection, Option<ToolpathTab>, f32, usize);

/// Every surface the sentry reads. The toolpath tabs render at the default
/// width with a line cap, and again in the 240-point rail for the layout
/// rules only.
fn surfaces(state: &AppState) -> Vec<Surface> {
    let configs = state.session().toolpath_configs();
    let setup = state.session().list_setups()[0].id;
    let mut out = Vec::new();
    // The caps: the after-state of IC1 (2026-10-13) plus 3 lines. The tabs
    // in order: Geometry, Feeds & Speeds, Linking, Heights, Dressup. Before
    // IC1 the Feeds & Speeds tab painted 52 to 60 lines with 14 to 16
    // paragraphs. Round 2 (IC2, same day) moved the derivation into the
    // closed `Details`: Feeds & Speeds went from 32-36 to 22-25 lines.
    // Geometry selection P4.3 added the Selection section to the Pocket
    // Geometry tab (the header, Cuts, Rules, the actions and `Details`):
    // 26 lines, so the cap is 26 plus 3 by the same rule.
    let caps: [(&str, [usize; 5]); 3] = [
        ("Pocket", [29, 25, 22, 21, 13]),
        ("3D Rough", [23, 25, 22, 20, 13]),
        ("3D Finish", [21, 28, 22, 23, 15]),
    ];
    for (name, tab_caps) in caps {
        let id = configs
            .iter()
            .find(|c| c.name == name)
            .map(|c| c.id)
            .expect("fixture toolpath");
        for (tab, cap) in ToolpathTab::ALL.iter().zip(tab_caps) {
            out.push((
                format!("{name} / {tab:?}"),
                Selection::Toolpath(id),
                Some(*tab),
                PANEL_WIDTH,
                cap,
            ));
            out.push((
                format!("{name} / {tab:?} @{RAIL_WIDTH}"),
                Selection::Toolpath(id),
                Some(*tab),
                RAIL_WIDTH,
                usize::MAX,
            ));
        }
    }
    let panels: [(&str, Selection, usize); 7] = [
        ("Stock", Selection::Stock, 19),
        ("Setup", Selection::Setup(setup), 23),
        ("Tool", Selection::Tool(ToolId::from_raw(TOOL)), 20),
        ("Post", Selection::PostProcessor, 9),
        ("Machine", Selection::Machine, 21),
        ("Model 2D", Selection::Model(ModelId::from_raw(MODEL_2D)), 7),
        (
            "Model 3D",
            Selection::Model(ModelId::from_raw(MODEL_3D)),
            14,
        ),
    ];
    for (name, selection, cap) in panels {
        out.push((name.to_owned(), selection, None, PANEL_WIDTH, cap));
    }
    out
}

#[test]
fn the_inspector_text_is_bounded_ic1() {
    let mut state = fixture();
    let mut failures = Vec::new();
    let mut total_rows = 0_usize;
    for (name, selection, tab, width, cap) in surfaces(&state) {
        let frame = surface(&mut state, selection, tab, width);
        let painted = frame.painted;
        if frame.excess_width > 0.5 {
            failures.push(format!(
                "{name}: the content is {:.1} points wider than the panel; its right \
                 edge is cut off",
                frame.excess_width
            ));
        }
        let rows = visible_lines(&painted);
        total_rows += rows;
        let multi: Vec<&Painted> = painted.iter().filter(|p| p.rows > 1).collect();
        eprintln!(
            "== {name}: {rows} visible lines, {} galleys, {} multi-line, {:.1} excess width",
            painted.len(),
            multi.len(),
            frame.excess_width
        );
        for p in &painted {
            eprintln!(
                "   [{}{}] {:.0}-{:.0} {}",
                p.rows,
                if p.overflow > 0.5 { " CLIPPED" } else { "" },
                p.span.0,
                p.span.1,
                p.text.replace('\n', " / ")
            );
        }
        for p in multi {
            if !may_wrap(p) {
                failures.push(format!("{name}: a {}-row text block: {:?}", p.rows, p.text));
            }
        }
        for p in painted.iter().filter(|p| p.overflow > 0.5) {
            failures.push(format!(
                "{name}: {:?} paints {:.1} points past its clip rectangle",
                p.text, p.overflow
            ));
        }
        if rows > cap {
            failures.push(format!(
                "{name}: {rows} visible text rows, cap {cap}. Move explanations into hover \
                 text and rare fields into the Advanced section."
            ));
        }
    }
    // Non-vacuity: the renders painted the inspector.
    assert!(
        total_rows > 100,
        "the renders painted only {total_rows} rows"
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Non-vacuity for the two measures above. A wrapped sentence in a narrow
/// column must count as a multi-row galley and as more than one visible
/// line, and the one-line component must keep the same sentence on one row.
#[test]
fn the_measures_see_a_paragraph_ic1() {
    const SENTENCE: &str = "This sentence is much wider than the narrow column it is drawn \
                            in, so a wrapping label breaks it over several rows.";
    let ctx = ctx();
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        ui.set_max_width(120.0);
        ui.add(egui::Label::new(SENTENCE).wrap());
        ui.add(rs_cam_viz::ui::components::OneLine::new(SENTENCE));
    });
    let painted: Vec<Painted> = out
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::Text(text) => Some(Painted {
                text: text.galley.job.text.clone(),
                rows: text.galley.rows.len(),
                bands: text
                    .galley
                    .rows
                    .iter()
                    .map(|row| {
                        let rect = row.rect();
                        (text.pos.y + rect.min.y, text.pos.y + rect.max.y)
                    })
                    .collect(),
                overflow: 0.0,
                span: (0.0, 0.0),
                color: None,
            }),
            _ => None,
        })
        .collect();
    out.textures_delta.clear();
    assert_eq!(painted.len(), 2, "expected two galleys: {painted:?}");
    assert!(
        painted[0].rows > 1,
        "the wrapped label painted {} row",
        painted[0].rows
    );
    assert_eq!(
        painted[1].rows, 1,
        "OneLine painted {} rows",
        painted[1].rows
    );
    assert!(
        visible_lines(&painted) > 2,
        "a wrapped paragraph and a line counted as {} lines",
        visible_lines(&painted)
    );
    assert!(painted.iter().all(|p| p.text == SENTENCE));
}
