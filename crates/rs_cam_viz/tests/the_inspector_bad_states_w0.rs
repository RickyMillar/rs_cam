//! W0: the toolpath inspector in bad states, measured before the tidy.
//!
//! # Why this file exists
//!
//! The IC1 sentry (`the_inspector_text_is_bounded_ic1`) renders one clean
//! fixture: a tool on each op, filled feeds, a clean selection and no reach
//! map. So it never renders an error state (finding F14 of
//! `planning/toolpath_panel_review_2026-10-30/FINDINGS.md`). The tidy
//! programme (W1-W6) moves the error text of the panel. This file records
//! the counts BEFORE that work, so W6 can turn them into asserts.
//!
//! # What it measures
//!
//! The test renders the real inspector (`properties::draw`) at the 280-point
//! panel width of IC1. It renders each tab of a Pocket (2D), a 3D Rough
//! (Adaptive3d) and a 3D Finish (Scallop) in each bad state. Each state has
//! its own fresh fixture. For each screen it reads every painted galley:
//!
//! - the full galley text (not the truncated face), its length, and whether
//!   the text was truncated (elided by the galley, or clipped);
//! - how many times each distinct message paints on the screen;
//! - the number of visible text rows.
//!
//! A second test counts the raw `ui.label(` and `ui.horizontal(` sites per
//! file under `src/ui/properties/`.
//!
//! The tests RECORD. They do not assert the W6 rules. They assert only that
//! each bad state paints its marker more times than the clean state, so a
//! baseline of the clean state cannot pass for a bad state (the F14 lesson).
//!
//! Run with `--nocapture` to print the tables. Set `W0_WRITE_BASELINE=1` to
//! write `planning/toolpath_panel_review_2026-10-30/W0_BASELINE.md`.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the test prints its measurements for the reader"
)]

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rs_cam_core::ToolpathId;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, BoundarySource};
use rs_cam_core::compute::fixture_values;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::P3;
use rs_cam_core::maps::reach_map::reach_map_for_mesh;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh, make_test_hemisphere};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{
    AdoptResultArgs, Command, LoadedModel, ProjectSessionBuilder, SetBoundaryConfigArgs,
    SetToolpathParamArgs, ToolpathComputeResult, ToolpathConfig,
};
use rs_cam_core::tool::BallEndmill;
use rs_cam_core::toolpath::Toolpath;
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::freshness::{FreshnessState, freshness_at};
use rs_cam_viz::state::runtime::{
    ReachStatus, ToolpathRun, ToolpathRuntime, reach_overlay_surface,
};
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::state::toolpath::ToolpathResult;
use rs_cam_viz::ui::properties::{self, ToolpathTab};
use rs_cam_viz::ui::tokens;

/// The default inspector width of IC1 (`app.rs`).
const PANEL_WIDTH: f32 = 280.0;

/// The 6 mm flat end mill.
const TOOL_FLAT: usize = 1;
/// The 6 mm ball nose. Scallop needs a ball tip.
const TOOL_BALL: usize = 2;
const MODEL_2D: usize = 4;
const MODEL_3D: usize = 5;

/// The hemisphere radius (mm) of the 3D model. A curved surface gives the
/// reach map a measured population.
const HEMISPHERE_RADIUS: f64 = 10.0;
const HEMISPHERE_DIVISIONS: usize = 12;

/// The reach recipe of `reach_overlay_p5`: a Ø6 ball, 0.5 mm cells and a
/// tolerance under the grid floor, so the over-statement caution row paints.
const REACH_TOLERANCE_MM: f64 = 0.0001;
const REACH_CELL_MM: f64 = 0.5;

/// The D53 refusal text of IC1 (`session/compute/generation.rs`), the
/// longest generate refusal the panel can show.
const REFUSAL: &str = "'Edges' takes its boundary from the rest regions of 'Body rough' \
    with Center containment, but its tool radius 5.000 mm is larger than the tool \
    radius of 'Body rough', 3.000 mm. The regions are tool-centre positions of that \
    tool, so this tool could cut up to 2.000 mm past the machining boundary of 'Body \
    rough'. Set the boundary containment to Inside, or use a tool no larger than the \
    tool of 'Body rough'.";

/// The feed the stale state writes over the fixture feed.
const EDITED_FEED: f64 = 900.0;

/// A face over this many characters breaks the W6 rule (PLAN_EXECUTION §3).
const FACE_LIMIT: usize = 60;

// ── The fixture ─────────────────────────────────────────────────────────

/// One op family of the toolpath inspector.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Family {
    Pocket,
    Rough3d,
    Finish3d,
}

impl Family {
    const ALL: [Family; 3] = [Family::Pocket, Family::Rough3d, Family::Finish3d];

    fn name(self) -> &'static str {
        match self {
            Family::Pocket => "Pocket",
            Family::Rough3d => "3D Rough",
            Family::Finish3d => "3D Finish",
        }
    }

    fn operation(self) -> OperationConfig {
        match self {
            Family::Pocket => OperationConfig::Pocket(Default::default()),
            Family::Rough3d => OperationConfig::Adaptive3d(Default::default()),
            Family::Finish3d => OperationConfig::Scallop(Default::default()),
        }
    }

    fn model(self) -> usize {
        match self {
            Family::Pocket => MODEL_2D,
            Family::Rough3d | Family::Finish3d => MODEL_3D,
        }
    }

    /// The tool that fits the op.
    fn tool(self) -> usize {
        match self {
            Family::Pocket | Family::Rough3d => TOOL_FLAT,
            Family::Finish3d => TOOL_BALL,
        }
    }
}

/// One state of the inspector. `Clean` is the reference; the others are
/// the bad states of W0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum State {
    Clean,
    NoTool,
    FeedsUnset,
    WrongTool,
    Refused,
    ReachOn,
    BoundaryOn,
    StaleAfterEdit,
    /// Extra (not in the W0 list): the feeds are not set AND the last run
    /// refused for that reason. The operator saw this case (F3).
    FeedsUnsetRefused,
}

impl State {
    const ALL: [State; 9] = [
        State::Clean,
        State::NoTool,
        State::FeedsUnset,
        State::WrongTool,
        State::Refused,
        State::ReachOn,
        State::BoundaryOn,
        State::StaleAfterEdit,
        State::FeedsUnsetRefused,
    ];

    fn name(self) -> &'static str {
        match self {
            State::Clean => "clean",
            State::NoTool => "no tool",
            State::FeedsUnset => "feeds unset",
            State::WrongTool => "wrong tool kind",
            State::Refused => "last run refused",
            State::ReachOn => "reach map on",
            State::BoundaryOn => "boundary on (Selection)",
            State::StaleAfterEdit => "stale after edit",
            State::FeedsUnsetRefused => "feeds unset + refused (extra)",
        }
    }

    /// Why the state does not apply to a family, or `None` when it does.
    fn skip_reason(self, family: Family) -> Option<&'static str> {
        match (self, family) {
            (State::WrongTool, Family::Pocket | Family::Rough3d) => {
                Some("the registry row is ANY_TOOL: every tool kind fits")
            }
            (State::ReachOn, Family::Pocket | Family::Rough3d) => {
                Some("`supports_reach_map` is false: no reach rows exist for the op")
            }
            _ => None,
        }
    }

    fn feeds_unset(self) -> bool {
        matches!(self, State::FeedsUnset | State::FeedsUnsetRefused)
    }
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

fn hemisphere() -> TriangleMesh {
    make_test_hemisphere(HEMISPHERE_RADIUS, HEMISPHERE_DIVISIONS)
}

fn mesh_model(id: usize) -> LoadedModel {
    LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(id),
        path: PathBuf::from(format!("model_{id}.stl")),
        name: format!("Mesh {id}"),
        kind: Some(ModelKind::Stl),
        mesh: Some(Arc::new(hemisphere())),
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

fn toolpath(
    name: &str,
    model_id: usize,
    tool: Option<usize>,
    operation: OperationConfig,
) -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        id: ToolpathId::from_raw(0),
        name: name.to_owned(),
        enabled: true,
        operation,
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: tool.map(rs_cam_core::ids::ToolId::from_raw),
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

fn sample_toolpath(feed: f64) -> Toolpath {
    let mut path = Toolpath::new();
    path.rapid_to(P3::new(0.0, 0.0, 5.0));
    path.feed_to(P3::new(10.0, 0.0, -1.0), feed);
    path.feed_to(P3::new(10.0, 10.0, -1.0), feed);
    path.rapid_to(P3::new(10.0, 10.0, 5.0));
    path
}

fn core_result(path: Toolpath) -> ToolpathComputeResult {
    ToolpathComputeResult {
        op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(Arc::new(AnnotatedToolpath::new(
            path,
        ))),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
        provenance: rs_cam_core::session::ResultProvenance::Dressed,
    }
}

fn viz_result(path: Toolpath) -> ToolpathResult {
    ToolpathResult {
        annotated: Arc::new(AnnotatedToolpath::new(path)),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
        debug_trace_path: None,
        drill_op: None,
    }
}

/// A flat end mill and a ball nose (both 6 mm), one outline, one
/// hemisphere, an 18 mm board and the three ops. The state applies to the
/// op of `target` only. Returns the state and the id of the target op.
fn fixture(target: Family, bad: State) -> (AppState, ToolpathId) {
    let mut flat = ToolConfig::new_default(ToolId::from_raw(TOOL_FLAT), ToolType::EndMill);
    flat.diameter = 6.0;
    let mut ball = ToolConfig::new_default(ToolId::from_raw(TOOL_BALL), ToolType::BallNose);
    ball.diameter = 6.0;
    let stock = StockConfig {
        z: Some(18.0),
        auto_from_model: false,
        ..StockConfig::default()
    };
    let mut builder = ProjectSessionBuilder::new()
        .tool(flat)
        .tool(ball)
        .model(polygon_model(MODEL_2D))
        .model(mesh_model(MODEL_3D))
        .stock(stock);
    for family in Family::ALL {
        let is_target = family == target;
        let mut op = family.operation();
        // S11: the fixture states the feeds and the floor, so the clean
        // state has no finding. The feeds-unset state keeps the floor and
        // leaves the feeds empty. These three ops have no other unset value.
        if is_target && bad.feeds_unset() {
            if let Some(floor) = fixture_values::fixture_floor(op.op_type()) {
                assert!(op.set_floor(floor), "the op holds a floor");
            }
        } else {
            fixture_values::fill_unset(&mut op);
        }
        let tool = match (is_target, bad) {
            (true, State::NoTool) => None,
            (true, State::WrongTool) => Some(TOOL_FLAT),
            _ => Some(family.tool()),
        };
        builder
            .add_toolpath(0, toolpath(family.name(), family.model(), tool, op))
            .expect("add a fixture toolpath");
    }
    let mut state = AppState::new();
    state.replace_session(builder.build());
    let configs = state.session().toolpath_configs();
    let ids: Vec<_> = configs.iter().map(|c| c.id).collect();
    let index = configs
        .iter()
        .position(|c| c.name == target.name())
        .expect("the target toolpath");
    let id = ids[index];
    for each in ids {
        state
            .gui
            .toolpath_rt
            .insert(each, ToolpathRuntime::new(true));
    }
    if bad.feeds_unset() {
        let op = &state.session().toolpath_configs()[index].operation;
        assert!(
            op.feed_rate().is_none() && op.plunge_rate().is_none() && op.spindle_rpm().is_none(),
            "precondition: the feeds of {} are not set",
            target.name()
        );
    }
    match bad {
        State::Clean | State::NoTool | State::FeedsUnset | State::WrongTool => {}
        State::Refused => set_run(&mut state, id, ToolpathRun::Failed(REFUSAL.to_owned())),
        State::FeedsUnsetRefused => {
            // The shape of the core `NotSet` refusal (`compute/not_set.rs`).
            let text = format!(
                "{}: {} (not set: feed rate, plunge rate, spindle speed)",
                target.name(),
                rs_cam_core::compute::not_set::FEEDS_NOT_SET_TEXT
            );
            set_run(&mut state, id, ToolpathRun::Failed(text));
        }
        State::ReachOn => {
            let mesh = hemisphere();
            let index_3d = SpatialIndex::build_auto(&mesh);
            let cutter = BallEndmill::new(6.0, 25.0);
            let map = Arc::new(reach_map_for_mesh(
                &mesh,
                &cutter,
                REACH_TOLERANCE_MM,
                REACH_CELL_MM,
            ));
            assert!(map.is_measured(), "precondition: the map is measured");
            let surface = reach_overlay_surface(&map, &mesh, &index_3d, 3.0);
            state.viewport.show_reach_map = true;
            let overlay = &mut state.gui.reach_overlay;
            overlay.toolpath = Some(id);
            overlay.status = ReachStatus::Ready(map);
            overlay.surface = Some(Arc::new(surface));
        }
        State::BoundaryOn => {
            let current = state.session().toolpath_configs()[index].boundary.clone();
            let mut boundary: BoundaryConfig = current.with_source(
                BoundarySource::Selection {
                    selection: rs_cam_core::selection::Selection::closed_shapes_of(
                        rs_cam_core::ids::ModelId::from_raw(MODEL_2D),
                    ),
                    holes: false,
                },
                Some(6.0),
            );
            boundary.enabled = true;
            let _ = state
                .apply(Command::SetBoundaryConfig(SetBoundaryConfigArgs {
                    index,
                    boundary,
                }))
                .expect("the boundary edit applies");
        }
        State::StaleAfterEdit => {
            let revision = state.session().toolpath_revision_at(index);
            let _ = state
                .apply(Command::AdoptResult(AdoptResultArgs {
                    index,
                    revision,
                    result: Box::new(core_result(sample_toolpath(1000.0))),
                }))
                .expect("seed the core result");
            set_run(
                &mut state,
                id,
                ToolpathRun::Done(viz_result(sample_toolpath(1000.0))),
            );
            let _ = state
                .apply(Command::SetToolpathParam(SetToolpathParamArgs {
                    index,
                    param: "feed_rate".to_owned(),
                    value: serde_json::json!(EDITED_FEED),
                }))
                .expect("the param edit applies");
            assert_eq!(
                freshness_at(state.session(), &state.gui, index),
                Some(FreshnessState::EditedSince),
                "precondition: the edit makes the result stale"
            );
        }
    }
    (state, id)
}

fn set_run(state: &mut AppState, id: ToolpathId, run: ToolpathRun) {
    state
        .gui
        .toolpath_rt
        .get_mut(&id)
        .expect("fixture runtime")
        .run = run;
}

// ── The render and the measures (the machinery of IC1) ─────────────────

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

/// One painted text galley.
#[derive(Debug, Clone)]
struct Painted {
    /// The full galley text (`job.text`), not the face.
    text: String,
    /// The text the rows paint (the face). It is shorter than `text` when
    /// the galley elided the tail.
    face: String,
    rows: usize,
    bands: Vec<(f32, f32)>,
    /// How far the text paints past the right edge of its clip rectangle.
    overflow: f32,
    elided: bool,
}

impl Painted {
    /// The galley elided its tail, or the clip rectangle cut it.
    fn truncated(&self) -> bool {
        self.elided || self.overflow > 0.5
    }
}

/// The number of visible text lines (IC1): the row bands of every galley,
/// merged where they overlap.
fn visible_lines(painted: &[Painted]) -> usize {
    let mut bands: Vec<(f32, f32)> = painted.iter().flat_map(|p| p.bands.clone()).collect();
    bands.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut lines = 0;
    let mut bottom = f32::NEG_INFINITY;
    for (top, low) in bands {
        if top < bottom - 3.0 {
            bottom = bottom.max(low);
        } else {
            lines += 1;
            bottom = low;
        }
    }
    lines
}

/// Render the inspector the way the app does and return its painted text.
fn render(ctx: &egui::Context, state: &mut AppState, width: f32) -> Vec<Painted> {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 6000.0),
        )),
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        egui::Panel::right("w0_inspector")
            .default_size(width)
            .max_size(width)
            .resizable(false)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut events = Vec::new();
                    properties::draw(ui, state, &mut events);
                });
            });
    });
    let painted = out
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::Text(text) if !text.galley.job.text.trim().is_empty() => {
                let galley = &text.galley;
                Some(Painted {
                    text: galley.job.text.clone(),
                    face: galley
                        .rows
                        .iter()
                        .map(|row| row.row.text())
                        .collect::<Vec<_>>()
                        .join(" "),
                    rows: galley.rows.len(),
                    bands: galley
                        .rows
                        .iter()
                        .map(|row| {
                            let rect = row.rect();
                            (text.pos.y + rect.min.y, text.pos.y + rect.max.y)
                        })
                        .collect(),
                    overflow: clipped.shape.visual_bounding_rect().max.x - clipped.clip_rect.max.x,
                    elided: galley.elided,
                })
            }
            _ => None,
        })
        .collect();
    out.textures_delta.clear();
    painted
}

/// Render one tab twice (the first frame settles one-shot state) on a fresh
/// context and return the second frame.
fn screen(state: &mut AppState, id: ToolpathId, tab: ToolpathTab) -> Vec<Painted> {
    let ctx = ctx();
    state.gui.pending_toolpath_tab = Some((id, tab));
    let _ = state.set_focus(Selection::Toolpath(id));
    let _ = render(&ctx, state, PANEL_WIDTH);
    render(&ctx, state, PANEL_WIDTH)
}

/// Whitespace normalised: the key of the repeat count.
fn normalise(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The repeat key with a leading glyph token removed ("✕ msg" and "msg"
/// are one message to the operator).
fn message_key(text: &str) -> String {
    let normal = normalise(text);
    normal
        .trim_start_matches(|c: char| !c.is_alphanumeric() && c != '\'' && c != '"')
        .to_owned()
}

/// A text of three words or more is a message; a shorter text is a label,
/// a unit or a value, and its repeats are not counted as message repeats.
fn is_message(key: &str) -> bool {
    key.split_whitespace().count() >= 3
}

/// The measures of one screen.
struct Screen {
    family: Family,
    state: State,
    tab: ToolpathTab,
    painted: Vec<Painted>,
    visible: usize,
    /// Each message key and the number of times it paints.
    repeats: BTreeMap<String, usize>,
}

impl Screen {
    fn new(family: Family, state: State, tab: ToolpathTab, painted: Vec<Painted>) -> Self {
        let mut repeats = BTreeMap::new();
        for p in &painted {
            *repeats.entry(message_key(&p.text)).or_insert(0) += 1;
        }
        Self {
            family,
            state,
            tab,
            visible: visible_lines(&painted),
            painted,
            repeats,
        }
    }

    fn max_message_repeat(&self) -> usize {
        self.repeats
            .iter()
            .filter(|(key, _)| is_message(key))
            .map(|(_, n)| *n)
            .max()
            .unwrap_or(0)
    }

    fn truncated(&self) -> usize {
        self.painted.iter().filter(|p| p.truncated()).count()
    }

    fn longest_face(&self) -> usize {
        self.painted
            .iter()
            .map(|p| p.face.chars().count())
            .max()
            .unwrap_or(0)
    }

    fn longest_text(&self) -> usize {
        self.painted
            .iter()
            .map(|p| p.text.chars().count())
            .max()
            .unwrap_or(0)
    }
}

/// The key of an exception row: the family, the state and the text.
type RowKey = (Family, State, String);

/// A long text over the tabs of one family and state: the longest face, the
/// truncated flag and the tabs that paint it.
type LongText = (usize, bool, Vec<String>);

/// The text that proves a bad state painted, or `None` for the clean
/// reference.
fn marker(state: State) -> Option<&'static [&'static str]> {
    match state {
        State::Clean => None,
        State::NoTool => Some(&["No tool"]),
        // The three empty feed fields paint "set a value". The clean state
        // paints it too, for its unset dressup values, but fewer times.
        State::FeedsUnset | State::FeedsUnsetRefused => Some(&["set a value"]),
        State::WrongTool => Some(&["needs a ball nose"]),
        State::Refused => Some(&["Refused"]),
        State::ReachOn => Some(&["Unreachable"]),
        // The boundary rows paint only while the boundary is on.
        State::BoundaryOn => Some(&["Containment:"]),
        State::StaleAfterEdit => Some(&["edited since"]),
    }
}

/// The number of galleys over every tab of one family and state that hold
/// a marker text.
fn marker_count(screens: &[Screen], family: Family, state: State, needles: &[&str]) -> usize {
    screens
        .iter()
        .filter(|s| s.family == family && s.state == state)
        .flat_map(|s| s.painted.iter())
        .filter(|p| needles.iter().any(|n| p.text.contains(n)))
        .count()
}

fn md_cell(text: &str) -> String {
    normalise(text).replace('|', "\\|")
}

fn render_all() -> Vec<Screen> {
    let mut screens = Vec::new();
    for state in State::ALL {
        for family in Family::ALL {
            if state.skip_reason(family).is_some() {
                continue;
            }
            let (mut app, id) = fixture(family, state);
            for tab in ToolpathTab::ALL {
                let painted = screen(&mut app, id, *tab);
                screens.push(Screen::new(family, state, *tab, painted));
            }
        }
    }
    screens
}

/// The inventory of every screen, as one string. The caller prints it with
/// one call, so the output of the other test does not split it.
fn inventory(screens: &[Screen]) -> String {
    let mut out = String::new();
    for s in screens {
        let _ = writeln!(
            out,
            "== {} / {} / {:?}: {} visible lines, {} galleys, {} truncated, max message repeat {}",
            s.family.name(),
            s.state.name(),
            s.tab,
            s.visible,
            s.painted.len(),
            s.truncated(),
            s.max_message_repeat()
        );
        for p in &s.painted {
            let key = message_key(&p.text);
            let _ = writeln!(
                out,
                "   [{} rows{}{} x{}] {} chars: {}",
                p.rows,
                if p.elided { " ELIDED" } else { "" },
                if p.overflow > 0.5 { " CLIPPED" } else { "" },
                s.repeats.get(&key).copied().unwrap_or(0),
                p.text.chars().count(),
                p.text.replace('\n', " / ")
            );
        }
    }
    out
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one test renders every state and writes the baseline tables in one place"
)]
fn the_inspector_bad_states_w0() {
    let screens = render_all();
    let inventory = inventory(&screens);

    // Non-vacuity: the renders painted the inspector.
    let total: usize = screens.iter().map(|s| s.visible).sum();
    assert!(total > 500, "the renders painted only {total} rows");

    // Preconditions: in each family it applies to, each bad state paints
    // its marker more times than the clean state does. A baseline of the
    // clean state cannot pass for a bad state (F14).
    let mut missing = Vec::new();
    for state in State::ALL {
        let Some(needles) = marker(state) else {
            continue;
        };
        for family in Family::ALL {
            if state.skip_reason(family).is_some() {
                continue;
            }
            let bad = marker_count(&screens, family, state, needles);
            let clean = marker_count(&screens, family, State::Clean, needles);
            if bad <= clean {
                missing.push(format!(
                    "{} / {}: {needles:?} paints {bad} times, the clean state {clean} times",
                    family.name(),
                    state.name()
                ));
            }
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));

    // The summary table: one row per screen.
    let mut summary = String::new();
    let _ = writeln!(
        summary,
        "| Family | State | Tab | Visible rows | Galleys | Truncated | Max repeat | Longest face | Longest text |"
    );
    let _ = writeln!(summary, "|---|---|---|---:|---:|---:|---:|---:|---:|");
    for s in &screens {
        let _ = writeln!(
            summary,
            "| {} | {} | {:?} | {} | {} | {} | {} | {} | {} |",
            s.family.name(),
            s.state.name(),
            s.tab,
            s.visible,
            s.painted.len(),
            s.truncated(),
            s.max_message_repeat(),
            s.longest_face(),
            s.longest_text()
        );
    }

    // Per state: the worst repeat and the longest face over every family
    // and tab.
    let mut per_state = String::new();
    let _ = writeln!(
        per_state,
        "| State | Screens | Worst repeat | Message of the worst repeat | Longest face | Longest text | Texts over {FACE_LIMIT} chars |"
    );
    let _ = writeln!(per_state, "|---|---:|---:|---|---:|---:|---:|");
    for state in State::ALL {
        let of_state: Vec<&Screen> = screens.iter().filter(|s| s.state == state).collect();
        let worst = of_state
            .iter()
            .flat_map(|s| s.repeats.iter().filter(|(k, _)| is_message(k)))
            .max_by_key(|(_, n)| **n);
        let long_texts: std::collections::BTreeSet<String> = of_state
            .iter()
            .flat_map(|s| s.painted.iter())
            .filter(|p| p.text.chars().count() > FACE_LIMIT)
            .map(|p| normalise(&p.text))
            .collect();
        let _ = writeln!(
            per_state,
            "| {} | {} | {} | {} | {} | {} | {} |",
            state.name(),
            of_state.len(),
            worst.map_or(0, |(_, n)| *n),
            worst.map_or_else(String::new, |(k, _)| md_cell(k)),
            of_state.iter().map(|s| s.longest_face()).max().unwrap_or(0),
            of_state.iter().map(|s| s.longest_text()).max().unwrap_or(0),
            long_texts.len()
        );
    }

    // The exceptions: each message that paints two or more times on one
    // screen, and each text over the face limit. One row per family, state
    // and text; the tabs are listed.
    let mut repeats_rows: BTreeMap<RowKey, (usize, Vec<String>)> = BTreeMap::new();
    let mut long_rows: BTreeMap<RowKey, LongText> = BTreeMap::new();
    for s in &screens {
        for (key, n) in &s.repeats {
            if *n >= 2 && is_message(key) {
                let row = repeats_rows
                    .entry((s.family, s.state, key.clone()))
                    .or_insert((0, Vec::new()));
                row.0 = row.0.max(*n);
                row.1.push(format!("{:?}", s.tab));
            }
        }
        for p in &s.painted {
            if p.text.chars().count() > FACE_LIMIT {
                let row = long_rows
                    .entry((s.family, s.state, normalise(&p.text)))
                    .or_insert((0, false, Vec::new()));
                row.0 = row.0.max(p.face.chars().count());
                row.1 |= p.truncated();
                let tab = format!("{:?}", s.tab);
                if !row.2.contains(&tab) {
                    row.2.push(tab);
                }
            }
        }
    }
    let mut repeats_md = String::new();
    let _ = writeln!(
        repeats_md,
        "| Family | State | Max per screen | Tabs | Message |"
    );
    let _ = writeln!(repeats_md, "|---|---|---:|---|---|");
    for ((family, state, key), (n, tabs)) in &repeats_rows {
        let _ = writeln!(
            repeats_md,
            "| {} | {} | {} | {} | {} |",
            family.name(),
            state.name(),
            n,
            tabs.join(", "),
            md_cell(key)
        );
    }
    let mut long_md = String::new();
    let _ = writeln!(
        long_md,
        "| Family | State | Chars | Face chars | Truncated | Tabs | Full text |"
    );
    let _ = writeln!(long_md, "|---|---|---:|---:|---|---|---|");
    for ((family, state, text), (face, truncated, tabs)) in &long_rows {
        let _ = writeln!(
            long_md,
            "| {} | {} | {} | {} | {} | {} | {} |",
            family.name(),
            state.name(),
            text.chars().count(),
            face,
            if *truncated { "yes" } else { "no" },
            tabs.join(", "),
            md_cell(text)
        );
    }

    eprintln!("{inventory}\n{per_state}\n{summary}\n{repeats_md}\n{long_md}");

    if std::env::var_os("W0_WRITE_BASELINE").is_some() {
        let scan = scan_table(&source_scan());
        let skipped: String = State::ALL
            .iter()
            .flat_map(|state| {
                Family::ALL.iter().filter_map(move |family| {
                    state
                        .skip_reason(*family)
                        .map(|why| format!("- {} / {}: {why}.\n", family.name(), state.name()))
                })
            })
            .collect();
        let doc = baseline_doc(
            &per_state,
            &summary,
            &repeats_md,
            &long_md,
            &scan,
            &skipped,
            &tree_state(),
        );
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../planning/toolpath_panel_review_2026-10-30/W0_BASELINE.md");
        std::fs::write(&path, doc).expect("write the baseline");
        eprintln!("wrote {}", path.display());
    }
}

fn baseline_doc(
    per_state: &str,
    summary: &str,
    repeats: &str,
    long: &str,
    scan: &str,
    skipped: &str,
    tree: &str,
) -> String {
    format!(
        "# W0 baseline: the toolpath inspector in bad states\n\
\n\
This file records the counts before the tidy programme (W1-W6). The test\n\
`crates/rs_cam_viz/tests/the_inspector_bad_states_w0.rs` writes it. Do not\n\
edit it by hand. To write it again, run this command:\n\
\n\
```\n\
W0_WRITE_BASELINE=1 scripts/cargo_lane.sh test -p rs_cam_viz -q --test the_inspector_bad_states_w0 -- --nocapture\n\
```\n\
\n\
## The tree that the test measured\n\
\n\
The counts are true for this tree only. A file that differs from HEAD\n\
changes the counts.\n\
\n\
{tree}\n\
## The method\n\
\n\
The test renders the real inspector (`properties::draw`) at the 280-point\n\
panel width of IC1. It renders three op families:\n\
\n\
- Pocket (2D), on a 20 mm square outline, with a 6 mm flat end mill.\n\
- 3D Rough (Adaptive3d), on a 10 mm hemisphere, with the flat end mill.\n\
- 3D Finish (Scallop), on the hemisphere, with a 6 mm ball nose.\n\
\n\
Each state has its own fresh fixture. The state applies to one op only.\n\
The test renders each tab of `ToolpathTab::ALL` in each state. It renders\n\
each tab two times on a fresh egui context and reads the second frame.\n\
\n\
The states are these:\n\
\n\
1. clean: the reference. The tool fits, the feeds and the floor are set,\n\
   and nothing refused.\n\
2. no tool: the op has no tool.\n\
3. feeds unset: the feed, the plunge and the spindle speed are empty. The\n\
   floor stays set.\n\
4. wrong tool kind: the flat end mill on the Scallop.\n\
5. last run refused: the run state holds the D53 refusal text of IC1.\n\
6. reach map on: `show_reach_map` is on and the overlay holds a ready map\n\
   of the hemisphere (Ø6 ball, 0.5 mm cells, tolerance 0.0001 mm). The\n\
   overlay also holds the refined surface, so the scale row paints.\n\
7. boundary on (Selection): the machining boundary is on, and its source\n\
   is the closed shapes of the 2D outline.\n\
8. stale after edit: the op has a result, then the feed changes to\n\
   900 mm/min. The freshness is `EditedSince`.\n\
9. feeds unset + refused (extra): state 3 plus a refusal in the shape of\n\
   the core `NotSet` text. This state is not in the W0 list.\n\
\n\
These states do not apply, and the test skips them:\n\
\n\
{skipped}\n\
The test asserts only one rule: each bad state paints its marker text\n\
more times than the clean state does. It does not assert the W6 rules yet.\n\
\n\
## The measures\n\
\n\
- Visible rows: the row bands of all galleys, merged where they overlap\n\
  (the IC1 measure). A row of label, value and unit is one row.\n\
- Galleys: the painted text galleys on the screen.\n\
- Truncated: a galley that elided its tail, or that the clip rectangle\n\
  cut.\n\
- Repeat: the number of galleys on one screen with the same message key.\n\
  The key is the full galley text, with the whitespace normalised and a\n\
  leading glyph (for example `✕` or `⚠`) removed. The test counts only a\n\
  text of three words or more as a message. A shorter text is a label, a\n\
  unit or a value.\n\
- Longest face: the most characters that one galley paints. The face is\n\
  shorter than the full text when the galley elided the tail.\n\
- Longest text: the most characters in one full galley text.\n\
\n\
The test prints the full galley inventory of each screen with\n\
`--nocapture`. This file holds the tables only.\n\
\n\
## The limits of the measures\n\
\n\
- The repeat key is the text. It does not see one fact in two words. For\n\
  example, the wrong tool state paints the tool refusal as three different\n\
  texts on one screen, and the repeat count is 1. W6 must count a\n\
  diagnostic id (the W1 `anchor` and `face`), not a text.\n\
- The three empty feed fields each paint the hint \"set a value\". The\n\
  repeat count is 3, but each hint belongs to its own field. W6 must not\n\
  count a field hint as a repeated message.\n\
- `OneLine` elides each long text, so no face is long. The full text\n\
  behind the face can be long. The tables give both lengths.\n\
\n\
## Worst values per state\n\
\n\
{per_state}\n\
## Repeated messages\n\
\n\
Each row is a message that paints two or more times on one screen.\n\
\n\
{repeats}\n\
## Texts over {FACE_LIMIT} characters\n\
\n\
Each row is a full galley text over {FACE_LIMIT} characters. W6 sets {FACE_LIMIT}\n\
characters as the face limit.\n\
\n\
{long}\n\
## Raw rows in the source\n\
\n\
The count of `ui.label(` and `ui.horizontal(` sites per file under\n\
`crates/rs_cam_viz/src/ui/properties/`. The scan ignores a line comment.\n\
\n\
{scan}\n\
## Each screen\n\
\n\
{summary}"
    )
}

/// The commit and the changed source files of the tree under test, as
/// Markdown. The baseline names the tree it measured.
fn tree_state() -> String {
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .args(args)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .map(|out| String::from_utf8_lossy(&out.stdout).trim_end().to_owned())
            .unwrap_or_default()
    };
    let head = git(&["log", "-1", "--format=%h %s"]);
    let changed = git(&[
        "status",
        "--short",
        "--",
        "../rs_cam_viz/src",
        "../rs_cam_core/src",
    ]);
    let changed = if changed.is_empty() {
        "(none)".to_owned()
    } else {
        changed
    };
    format!(
        "- HEAD: `{head}`\n- Source files that differ from HEAD (viz `src/` and core \
         `src/`):\n\n```\n{changed}\n```\n"
    )
}

// ── The source scan ─────────────────────────────────────────────────────

/// The raw-row sites of one file: the path under `ui/properties/`, the
/// `ui.label(` count and the `ui.horizontal(` count.
type ScanRow = (String, usize, usize);

fn source_scan() -> Vec<ScanRow> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/properties");
    let mut files = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read the properties folder") {
            let path = entry.expect("a folder entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
    }
    let mut rows: Vec<ScanRow> = files
        .iter()
        .map(|path| {
            let source = std::fs::read_to_string(path).expect("read a source file");
            let (mut labels, mut horizontals) = (0, 0);
            for line in source.lines() {
                // A line comment is not code.
                let code = line.split("//").next().unwrap_or("");
                labels += code.matches("ui.label(").count();
                horizontals += code.matches("ui.horizontal(").count();
            }
            let name = path
                .strip_prefix(&root)
                .expect("under the root")
                .display()
                .to_string();
            (name, labels, horizontals)
        })
        .collect();
    rows.sort_by(|a, b| (b.1 + b.2).cmp(&(a.1 + a.2)).then(a.0.cmp(&b.0)));
    rows
}

fn scan_table(rows: &[ScanRow]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "| File | `ui.label(` | `ui.horizontal(` | Total |");
    let _ = writeln!(out, "|---|---:|---:|---:|");
    for (name, labels, horizontals) in rows {
        let _ = writeln!(
            out,
            "| `{name}` | {labels} | {horizontals} | {} |",
            labels + horizontals
        );
    }
    let labels: usize = rows.iter().map(|r| r.1).sum();
    let horizontals: usize = rows.iter().map(|r| r.2).sum();
    let _ = writeln!(
        out,
        "| **all {} files** | {labels} | {horizontals} | {} |",
        rows.len(),
        labels + horizontals
    );
    out
}

#[test]
fn the_raw_rows_are_counted_w0() {
    let rows = source_scan();
    eprintln!("{}", scan_table(&rows));
    // Non-vacuity: the scan read the folder and found the raw rows that the
    // toolpath panel has today.
    assert!(rows.len() > 10, "the scan read {} files", rows.len());
    let panel = rows
        .iter()
        .find(|r| r.0 == "toolpath_panel.rs")
        .expect("the scan reads toolpath_panel.rs");
    assert!(
        panel.1 + panel.2 > 0,
        "toolpath_panel.rs has raw rows today"
    );
    // The comment rule: a line that is only a comment counts zero.
    let code = "// ui.label(\"x\")".split("//").next().unwrap_or("");
    assert_eq!(code.matches("ui.label(").count(), 0);
}
