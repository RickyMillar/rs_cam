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
//! 340-point panel (R4) for a pocket, a 3D rough and a 3D finish on each tab, and
//! for the other inspector panels. It reads every painted text galley. Then
//! it asserts two rules:
//!
//! 1. No galley wraps to more than one row, except a refusal or an error
//!    that blocks work (`may_wrap`). G-E (2026-10-28): in the toolpath
//!    panel no text wraps at all, a refusal included: a refusal is one
//!    word or one line, with its text on hover
//!    (`a_generate_refusal_takes_one_line_g_e`).
//! 2. The number of visible text rows on each surface stays under a cap.
//!    Every cap is the measured W3f state with no margin (W6 rule).
//! 3. W6: in every W0 bad state (`inspector_bad_states`), no painted text
//!    is over the 60-character face limit, counted on the full galley
//!    text (`no_painted_text_is_over_the_face_limit_w6`).
//!
//! Run it with `--nocapture` to print the inventory of each surface.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the test prints its measurements for the reader"
)]

mod inspector_bad_states;

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

/// The default toolpath inspector width (`app.rs`; R4, operator ruling
/// 2026-10-30: 340).
const PANEL_WIDTH: f32 = 340.0;

/// The default width of the setup properties panel (`app.rs`), where the
/// other inspector panels also draw. It stays 280.
const SETUP_PANEL_WIDTH: f32 = 280.0;

/// W2b: the height one issue row adds above the tab strip: the row
/// (`ROW_ACTION`), the space above the stack and the item gap below it.
const ONE_ISSUE_ROW: f32 = tokens::ROW_ACTION + 2.0 * tokens::SPACE_2;

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
        feeds_lock: Default::default(),
        id: ToolpathId::from_raw(0),
        name: name.to_owned(),
        enabled: true,
        operation,
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: Some(rs_cam_core::ids::ToolId::from_raw(TOOL)),
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
        z: Some(18.0),
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
        // S11: the fixture states the feeds and the depths; the Pocket floor
        // stays not set, so the unset-floor rows are measured too.
        let mut op = op;
        rs_cam_core::compute::fixture_values::fill_unset_keep_floor(&mut op);
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
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: foreign enum egui Shape; the arm skips every shape that this test does not read"
)]
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
    let _ = state.set_focus(selection);
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
    // in order (D7, 2026-10-23): Geometry, Cut, Heights, Feeds & Speeds,
    // Linking, Dressup. Before
    // IC1 the Feeds & Speeds tab painted 52 to 60 lines with 14 to 16
    // paragraphs. Round 2 (IC2, same day) moved the derivation into the
    // closed `Details`: Feeds & Speeds went from 32-36 to 22-25 lines.
    // Geometry selection P4.3 added the Selection section to the Pocket
    // Geometry tab (the header, Cuts, Rules, the actions and `Details`):
    // 26 lines, so the cap is 26 plus 3 by the same rule.
    // GT-1 (2026-10-22): the Geometry tab (the selection) comes first and
    // the old Geometry tab is Cut. Measured: Geometry 11 / 6 / 8 lines and
    // Cut 24 / 20 / 18 lines; each cap is the measure plus 3.
    // R2 + R4 (2026-10-30): five tabs in the order Geometry, Passes,
    // Feeds, Heights, Linking at 340 points. The Input row and the
    // Machining Boundary section moved to Geometry, the Dressup rows to
    // Linking, the reach rows out of the footer. Measured: Pocket
    // 13/17/23/21/24, 3D Rough 9/15/22/18/22, 3D Finish 11/12/24/20/21;
    // each cap was the measure plus 3.
    // W2b (2026-10-30): the per-tab footer is gone (the issue stack and the
    // notes counter replace it). Measured: Pocket 12/15/21/19/22, 3D Rough
    // 8/13/20/16/20, 3D Finish 10/10/22/18/19. W6 rule: each cap IS the
    // measure, with no margin, so a new line on a tab fails here.
    // W3f (2026-10-30), after W3a-e (every field row a `PropRow`, the
    // feeds card in Explore, the small-caps section labels): Pocket
    // 10/15/13/17/22, 3D Rough 7/13/12/15/15, 3D Finish 9/10/13/16/14.
    // Each cap is the measure, with no margin.
    // W3g (2026-10-30): the Feeds tab Cut row is gone (the suggestion is
    // the "⚡" action of Depth/pass and Stepover on Passes): Feeds 13/12/13
    // -> 12/11/12. The other tabs keep their measure (each W3g fix swaps a
    // line for a line).
    // W3i (2026-10-30): the Feeds tab holds the chart inline, a Chipload
    // row, the two apply buttons (two lines at 280 points), the
    // `Recommendation` Details and `Vendor data…`; the Explore button and
    // the vendor expander are gone. Measured: Feeds 22/21/23. The chart is
    // ONE block, but this counter reads painted text, so the plot's axis
    // ticks and axis names count as lines here (the feed ticks stack, one
    // line each). Each cap is the measure, with the chart's text in it.
    // P5b (operator ruling 2026-11-03): the Feeds tab merged into Passes,
    // which now holds the envelope plot (its header, axis names and edge
    // labels), the nomogram, the op rows, the speed rows and the card.
    // Measured Passes: Pocket 36, 3D Rough 34, 3D Finish 28 (each under the
    // old Passes + Feeds sum: 37, 34, 33). Geometry, Heights and Linking
    // keep their measure. Each cap is the measure, with no margin.
    let caps: [(&str, [usize; 4]); 3] = [
        ("Pocket", [10, 36, 17, 22]),
        ("3D Rough", [7, 34, 15, 15]),
        ("3D Finish", [9, 28, 16, 14]),
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
    // W3f (2026-10-30): each panel cap is its measure, with no margin (the
    // caps before were the measure plus 3).
    let panels: [(&str, Selection, usize); 7] = [
        ("Stock", Selection::Stock, 16),
        ("Setup", Selection::Setup(setup), 18),
        ("Tool", Selection::Tool(ToolId::from_raw(TOOL)), 17),
        ("Post", Selection::PostProcessor, 6),
        ("Machine", Selection::Machine, 19),
        ("Model 2D", Selection::Model(ModelId::from_raw(MODEL_2D)), 4),
        (
            "Model 3D",
            Selection::Model(ModelId::from_raw(MODEL_3D)),
            11,
        ),
    ];
    for (name, selection, cap) in panels {
        out.push((name.to_owned(), selection, None, SETUP_PANEL_WIDTH, cap));
    }
    out
}

/// A refused run as the app records it (W2b): the runtime row reads
/// `Failed`, and `ForgetResult` hands the refusal to core, which the header
/// run state reads.
fn record_refusal(state: &mut AppState, id: ToolpathId, refusal: &str) {
    use rs_cam_core::session::{Command, ForgetResultArgs};
    state
        .gui
        .toolpath_rt
        .get_mut(&id)
        .expect("fixture runtime")
        .run = rs_cam_viz::state::runtime::ToolpathRun::Failed(refusal.to_owned());
    let index = state
        .session()
        .find_toolpath_config_by_id(id)
        .map(|(index, _)| index)
        .expect("fixture toolpath");
    let _ = state
        .apply(Command::ForgetResult(ForgetResultArgs {
            index,
            refusal: Some(refusal.to_owned()),
        }))
        .expect("the refusal records");
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
        // G-E: in the toolpath panel ("<op> / <tab>") no text wraps.
        let toolpath_tab = name.contains(" / ");
        for p in multi {
            if toolpath_tab || !may_wrap(p) {
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

/// W6: the face of a last-run refusal over the face limit
/// (`toolpath_panel/issues.rs` `LAST_RUN_REFUSED`).
const LAST_RUN_REFUSED: &str = "The last run was refused";

/// The face limit of design rule 6 (core `diagnostics::FACE_MAX_CHARS`).
const FACE_LIMIT: usize = rs_cam_core::diagnostics::FACE_MAX_CHARS;

/// W6 (PLAN_EXECUTION §3 W6): in every W0 bad state, on every tab of the
/// three op families, at the default width, no painted text is over the
/// face limit (60 characters). The count reads the full galley text, not
/// the elided row, so a truncated row counts its whole text. The rule
/// covers each finding face, each issue row, each refusal and each label.
///
/// Red proof: the W0 record measured two texts over the limit before W6,
/// the 406-character D53 refusal on the issue row and the 108-character
/// "Feeds unavailable: …" line; each fails this test.
///
/// One id paints at most once per screen: `one_place_per_diagnostic_w2b`
/// (`each_diagnostic_paints_at_most_once_w2b`) holds it over the same
/// states and tabs.
#[test]
fn no_painted_text_is_over_the_face_limit_w6() {
    use inspector_bad_states::{Family, State, fixture, screen};
    assert_eq!(FACE_LIMIT, 60, "design rule 6");
    assert!(
        inspector_bad_states::REFUSAL.chars().count() > FACE_LIMIT,
        "non-vacuity: the Refused state holds a refusal over the limit"
    );
    let mut failures = Vec::new();
    let mut screens = 0_usize;
    let mut short_faces = 0_usize;
    let mut longest = 0_usize;
    for family in Family::ALL {
        for bad in State::ALL {
            if bad.skip_reason(family).is_some() {
                continue;
            }
            let (mut state, id) = fixture(family, bad);
            for tab in ToolpathTab::ALL {
                screens += 1;
                for p in screen(&mut state, id, *tab) {
                    let n = p.text.chars().count();
                    longest = longest.max(n);
                    if p.text == LAST_RUN_REFUSED || p.text == "Feeds unavailable" {
                        short_faces += 1;
                    }
                    if n > FACE_LIMIT {
                        failures.push(format!(
                            "{} / {} / {tab:?}: {n} characters: {:?}",
                            family.name(),
                            bad.name(),
                            p.text
                        ));
                    }
                }
            }
        }
    }
    eprintln!("W6: {screens} screens, longest painted text {longest} characters");
    // P5b: four tabs per state (the Feeds tab merged into Passes): 92.
    assert!(screens > 80, "non-vacuity: {screens} screens");
    assert!(
        short_faces > 0,
        "non-vacuity: no state painted the short face of a long refusal"
    );
    failures.sort();
    failures.dedup();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Non-vacuity for the two measures above. A wrapped sentence in a narrow
/// column must count as a multi-row galley and as more than one visible
/// line, and the one-line component must keep the same sentence on one row.
#[test]
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "test: foreign enum egui Shape; the arm skips every shape that this test does not read"
)]
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

/// G-E sub-step 1, W2b: a refusal from the last generate takes ONE word in
/// the status line ("Failed") and ONE issue row under it, and no text wraps. Before G-E
/// the D53 refusal printed in full beside Generate: 15 rows, and the tab
/// strip moved down 257 points.
#[test]
fn a_generate_refusal_takes_one_line_g_e() {
    // The D53 refusal text (`session/compute/generation.rs`), the longest
    // generate refusal the panel can show.
    const REFUSAL: &str = "'Edges' takes its boundary from the rest regions of 'Body rough' \
        with Center containment, but its tool radius 5.000 mm is larger than the tool \
        radius of 'Body rough', 3.000 mm. The regions are tool-centre positions of that \
        tool, so this tool could cut up to 2.000 mm past the machining boundary of 'Body \
        rough'. Set the boundary containment to Inside, or use a tool no larger than the \
        tool of 'Body rough'.";
    let mut state = fixture();
    let id = state
        .session()
        .toolpath_configs()
        .iter()
        .find(|c| c.name == "3D Rough")
        .map(|c| c.id)
        .expect("fixture toolpath");
    let tab_top = |painted: &[Painted]| {
        painted
            .iter()
            .find(|p| p.text == "Geometry")
            .and_then(|p| p.bands.first().map(|b| b.0))
            .expect("the tab strip paints")
    };
    let clean = surface(
        &mut state,
        Selection::Toolpath(id),
        Some(ToolpathTab::Passes),
        PANEL_WIDTH,
    );
    record_refusal(&mut state, id, REFUSAL);
    let refused = surface(
        &mut state,
        Selection::Toolpath(id),
        Some(ToolpathTab::Passes),
        PANEL_WIDTH,
    );
    // W2b: the status word of core `RunState::Failed`, and the reason as
    // ONE issue row. W6: the reason is over the face limit, so the row
    // paints the short face and the reason is its hover.
    assert!(
        refused.painted.iter().any(|p| p.text == "Failed"),
        "the state word: {:?}",
        refused.painted.iter().map(|p| &p.text).collect::<Vec<_>>()
    );
    assert!(
        refused
            .painted
            .iter()
            .any(|p| p.text == LAST_RUN_REFUSED && p.rows == 1),
        "the issue row paints the short face on one row"
    );
    assert!(
        !refused.painted.iter().any(|p| p.text == REFUSAL),
        "the full refusal is the hover, not the face"
    );
    let wrapped: Vec<&String> = refused
        .painted
        .iter()
        .filter(|p| p.rows > 1)
        .map(|p| &p.text)
        .collect();
    assert!(wrapped.is_empty(), "no text wraps: {wrapped:?}");
    assert_eq!(
        visible_lines(&refused.painted),
        visible_lines(&clean.painted) + 1,
        "the refusal adds one line"
    );
    // W2b: the refusal is ONE issue row: `ROW_ACTION` high, with the space
    // above it and the item gap below it. A second row would move the
    // strip twice as far.
    let moved = tab_top(&refused.painted) - tab_top(&clean.painted);
    assert!(
        (0.0..=ONE_ISSUE_ROW).contains(&moved),
        "the tab strip moves down one issue row at most, not {moved:.1} points"
    );
    assert!(
        refused.excess_width <= 0.5,
        "no row is wider than the panel"
    );
}

/// G-E sub-step 3: a rest-boundary refusal from the last generate has a
/// short face and the `⟳` action that generates its source. Here the 3D
/// Finish takes its boundary from the rest regions of the 3D Rough, which
/// has no result: the face is `✕ Generate '3D Rough' first`, on one row
/// with the action, and the full refusal stays on the hover.
#[test]
fn a_rest_refusal_has_a_short_face_and_an_action_g_e() {
    use rs_cam_core::compute::config::{BoundaryConfig, BoundarySource};
    use rs_cam_core::session::{Command, RestBoundaryRefusal, SetBoundaryConfigArgs};
    let mut state = fixture();
    let configs = state.session().toolpath_configs();
    let index = |name: &str| {
        configs
            .iter()
            .position(|c| c.name == name)
            .expect("fixture toolpath")
    };
    let (rough, finish) = (index("3D Rough"), index("3D Finish"));
    let rough_id = configs[rough].id;
    let finish_id = configs[finish].id;
    let mut boundary: BoundaryConfig = configs[finish].boundary.clone();
    boundary.enabled = true;
    boundary.source = BoundarySource::DerivedRestRegions {
        source_toolpath_id: rough_id,
    };
    let _ = state
        .apply(Command::SetBoundaryConfig(SetBoundaryConfigArgs {
            index: finish,
            boundary,
        }))
        .expect("the boundary edit applies");
    let refusal = state
        .session()
        .rest_boundary_refusal(finish)
        .expect("the source has no result");
    assert!(matches!(refusal, RestBoundaryRefusal::NotGenerated { .. }));
    record_refusal(&mut state, finish_id, &refusal.to_string());

    let frame = surface(
        &mut state,
        Selection::Toolpath(finish_id),
        Some(ToolpathTab::Passes),
        PANEL_WIDTH,
    );
    let face = frame
        .painted
        .iter()
        .find(|p| p.text == "Generate '3D Rough' first")
        .expect("the short face");
    let action = frame
        .painted
        .iter()
        .find(|p| p.text == "\u{27F3}")
        .expect("the generate action");
    assert_eq!(face.rows, 1);
    assert!(
        (face.bands[0].0 - action.bands[0].0).abs() < 8.0,
        "the face and its action share one row"
    );
    assert!(
        !frame.painted.iter().any(|p| p.text == refusal.to_string()),
        "the full refusal is on the hover, not the face"
    );
    assert!(frame.painted.iter().all(|p| p.rows == 1), "no text wraps");
    assert!(frame.excess_width <= 0.5, "the row fits the panel");
}

/// G-E sub-step 4, W2b: a validation error with no core finding is ONE
/// issue row: no wrap, the tab strip down by one issue row at most.
/// The 3D Rough is bound to the 2D model, so it has no 3D mesh.
#[test]
fn a_validation_error_takes_one_line_g_e() {
    use rs_cam_core::session::{Command, SetToolpathModelArgs};
    let mut state = fixture();
    let configs = state.session().toolpath_configs();
    let index = configs
        .iter()
        .position(|c| c.name == "3D Rough")
        .expect("fixture toolpath");
    let id = configs[index].id;
    let tab_top = |painted: &[Painted]| {
        painted
            .iter()
            .find(|p| p.text == "Geometry")
            .and_then(|p| p.bands.first().map(|b| b.0))
            .expect("the tab strip paints")
    };
    let clean = surface(
        &mut state,
        Selection::Toolpath(id),
        Some(ToolpathTab::Passes),
        PANEL_WIDTH,
    );
    let _ = state
        .apply(Command::SetToolpathModel(SetToolpathModelArgs {
            index,
            model_id: ModelId::from_raw(MODEL_2D),
        }))
        .expect("the model edit applies");
    let refused = surface(
        &mut state,
        Selection::Toolpath(id),
        Some(ToolpathTab::Passes),
        PANEL_WIDTH,
    );
    assert!(
        refused
            .painted
            .iter()
            .any(|p| p.text.starts_with("Selected model")),
        "non-vacuity: the validation error paints: {:?}",
        refused.painted.iter().map(|p| &p.text).collect::<Vec<_>>()
    );
    assert!(refused.painted.iter().all(|p| p.rows == 1), "no text wraps");
    // W2b: the refusal is ONE issue row: `ROW_ACTION` high, with the space
    // above it and the item gap below it. A second row would move the
    // strip twice as far.
    let moved = tab_top(&refused.painted) - tab_top(&clean.painted);
    assert!(
        (0.0..=ONE_ISSUE_ROW).contains(&moved),
        "the tab strip moves down one issue row at most, not {moved:.1} points"
    );
}
