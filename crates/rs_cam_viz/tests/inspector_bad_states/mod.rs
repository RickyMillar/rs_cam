//! The bad-state fixtures of the toolpath inspector (W0), shared by the
//! W0 record (`the_inspector_bad_states_w0`) and the W2b sentry
//! (`one_place_per_diagnostic_w2b`): the fixture project, the bad states,
//! and the render of the real inspector with its painted text.

#![allow(
    dead_code,
    reason = "each test binary that includes this module uses only part of it"
)]

use std::path::PathBuf;
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
    AdoptResultArgs, Command, ForgetResultArgs, LoadedModel, ProjectSessionBuilder,
    SetBoundaryConfigArgs, SetToolpathParamArgs, ToolpathComputeResult, ToolpathConfig,
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
pub const PANEL_WIDTH: f32 = 280.0;

/// The 6 mm flat end mill.
pub const TOOL_FLAT: usize = 1;
/// The 6 mm ball nose. Scallop needs a ball tip.
pub const TOOL_BALL: usize = 2;
pub const MODEL_2D: usize = 4;
pub const MODEL_3D: usize = 5;

/// The hemisphere radius (mm) of the 3D model. A curved surface gives the
/// reach map a measured population.
pub const HEMISPHERE_RADIUS: f64 = 10.0;
pub const HEMISPHERE_DIVISIONS: usize = 12;

/// The reach recipe of `reach_overlay_p5`: a Ø6 ball, 0.5 mm cells and a
/// tolerance under the grid floor, so the over-statement caution row paints.
pub const REACH_TOLERANCE_MM: f64 = 0.0001;
pub const REACH_CELL_MM: f64 = 0.5;

/// The D53 refusal text of IC1 (`session/compute/generation.rs`), the
/// longest generate refusal the panel can show.
pub const REFUSAL: &str = "'Edges' takes its boundary from the rest regions of 'Body rough' \
    with Center containment, but its tool radius 5.000 mm is larger than the tool \
    radius of 'Body rough', 3.000 mm. The regions are tool-centre positions of that \
    tool, so this tool could cut up to 2.000 mm past the machining boundary of 'Body \
    rough'. Set the boundary containment to Inside, or use a tool no larger than the \
    tool of 'Body rough'.";

/// The feed the stale state writes over the fixture feed.
pub const EDITED_FEED: f64 = 900.0;

// ── The fixture ─────────────────────────────────────────────────────────

/// One op family of the toolpath inspector.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Family {
    Pocket,
    Rough3d,
    Finish3d,
}

impl Family {
    pub const ALL: [Family; 3] = [Family::Pocket, Family::Rough3d, Family::Finish3d];

    pub fn name(self) -> &'static str {
        match self {
            Family::Pocket => "Pocket",
            Family::Rough3d => "3D Rough",
            Family::Finish3d => "3D Finish",
        }
    }

    pub fn operation(self) -> OperationConfig {
        match self {
            Family::Pocket => OperationConfig::Pocket(Default::default()),
            Family::Rough3d => OperationConfig::Adaptive3d(Default::default()),
            Family::Finish3d => OperationConfig::Scallop(Default::default()),
        }
    }

    pub fn model(self) -> usize {
        match self {
            Family::Pocket => MODEL_2D,
            Family::Rough3d | Family::Finish3d => MODEL_3D,
        }
    }

    /// The tool that fits the op.
    pub fn tool(self) -> usize {
        match self {
            Family::Pocket | Family::Rough3d => TOOL_FLAT,
            Family::Finish3d => TOOL_BALL,
        }
    }
}

/// One state of the inspector. `Clean` is the reference; the others are
/// the bad states of W0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum State {
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
    pub const ALL: [State; 9] = [
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

    pub fn name(self) -> &'static str {
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
    pub fn skip_reason(self, family: Family) -> Option<&'static str> {
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

    pub fn feeds_unset(self) -> bool {
        matches!(self, State::FeedsUnset | State::FeedsUnsetRefused)
    }
}

pub fn polygon_model(id: usize) -> LoadedModel {
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

pub fn hemisphere() -> TriangleMesh {
    make_test_hemisphere(HEMISPHERE_RADIUS, HEMISPHERE_DIVISIONS)
}

pub fn mesh_model(id: usize) -> LoadedModel {
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

pub fn toolpath(
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

pub fn sample_toolpath(feed: f64) -> Toolpath {
    let mut path = Toolpath::new();
    path.rapid_to(P3::new(0.0, 0.0, 5.0));
    path.feed_to(P3::new(10.0, 0.0, -1.0), feed);
    path.feed_to(P3::new(10.0, 10.0, -1.0), feed);
    path.rapid_to(P3::new(10.0, 10.0, 5.0));
    path
}

pub fn core_result(path: Toolpath) -> ToolpathComputeResult {
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

pub fn viz_result(path: Toolpath) -> ToolpathResult {
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
pub fn fixture(target: Family, bad: State) -> (AppState, ToolpathId) {
    fixture_with_stock_z(target, bad, Some(18.0))
}

/// [`fixture`] with the stock thickness `stock_z` (`None`: not set, a
/// project finding of every op).
pub fn fixture_with_stock_z(
    target: Family,
    bad: State,
    stock_z: Option<f64>,
) -> (AppState, ToolpathId) {
    let mut flat = ToolConfig::new_default(ToolId::from_raw(TOOL_FLAT), ToolType::EndMill);
    flat.diameter = 6.0;
    let mut ball = ToolConfig::new_default(ToolId::from_raw(TOOL_BALL), ToolType::BallNose);
    ball.diameter = 6.0;
    let stock = StockConfig {
        z: stock_z,
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
        State::Refused => record_refusal(&mut state, id, index, REFUSAL.to_owned()),
        State::FeedsUnsetRefused => {
            // The shape of the core `NotSet` refusal (`compute/not_set.rs`).
            let text = format!(
                "{}: {} (not set: feed rate, plunge rate, spindle speed)",
                target.name(),
                rs_cam_core::compute::not_set::FEEDS_NOT_SET_TEXT
            );
            record_refusal(&mut state, id, index, text);
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

/// A refused run as the app records it: the runtime row reads `Failed`, and
/// the controller drain sends `ForgetResult` with the refusal, which core
/// records for the run state (W1). W2b: the header reads the core run
/// state, so the fixture records both, as the app does.
pub fn record_refusal(state: &mut AppState, id: ToolpathId, index: usize, text: String) {
    set_run(state, id, ToolpathRun::Failed(text.clone()));
    let _ = state
        .apply(Command::ForgetResult(ForgetResultArgs {
            index,
            refusal: Some(text),
        }))
        .expect("the refusal records");
}

pub fn set_run(state: &mut AppState, id: ToolpathId, run: ToolpathRun) {
    state
        .gui
        .toolpath_rt
        .get_mut(&id)
        .expect("fixture runtime")
        .run = run;
}

// ── The render and the measures (the machinery of IC1) ─────────────────

pub fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

/// One painted text galley.
#[derive(Debug, Clone)]
pub struct Painted {
    /// The full galley text (`job.text`), not the face.
    pub text: String,
    /// The text the rows paint (the face). It is shorter than `text` when
    /// the galley elided the tail.
    pub face: String,
    pub rows: usize,
    pub bands: Vec<(f32, f32)>,
    /// How far the text paints past the right edge of its clip rectangle.
    pub overflow: f32,
    pub elided: bool,
}

impl Painted {
    /// The galley elided its tail, or the clip rectangle cut it.
    pub fn truncated(&self) -> bool {
        self.elided || self.overflow > 0.5
    }
}

/// The number of visible text lines (IC1): the row bands of every galley,
/// merged where they overlap.
pub fn visible_lines(painted: &[Painted]) -> usize {
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
pub fn render(ctx: &egui::Context, state: &mut AppState, width: f32) -> Vec<Painted> {
    render_with_outlines(ctx, state, width).0
}

/// True for the well of a `PropRow` in a field state (W3f): the well fill
/// with a Danger or Caution outline.
fn is_state_outline(shape: &egui::epaint::Shape) -> bool {
    use rs_cam_viz::ui::components::Role;
    match shape {
        egui::epaint::Shape::Rect(rect) => {
            rect.fill == rs_cam_viz::ui::tokens::INPUT_WELL
                && [Role::Danger.text(), Role::Caution.text()].contains(&rect.stroke.color)
        }
        _ => false,
    }
}

/// [`render`], and the number of field rows in a Danger or Caution state
/// (W3f: a field finding is the state of its row, not a text).
pub fn render_with_outlines(
    ctx: &egui::Context,
    state: &mut AppState,
    width: f32,
) -> (Vec<Painted>, usize) {
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
    let outlines = out
        .shapes
        .iter()
        .filter(|clipped| is_state_outline(&clipped.shape))
        .count();
    out.textures_delta.clear();
    (painted, outlines)
}

/// Render one tab twice (the first frame settles one-shot state) on a fresh
/// context and return the second frame.
pub fn screen(state: &mut AppState, id: ToolpathId, tab: ToolpathTab) -> Vec<Painted> {
    screen_with_outlines(state, id, tab).0
}

/// [`screen`], and the number of field rows in a Danger or Caution state.
pub fn screen_with_outlines(
    state: &mut AppState,
    id: ToolpathId,
    tab: ToolpathTab,
) -> (Vec<Painted>, usize) {
    let ctx = ctx();
    state.gui.pending_toolpath_tab = Some((id, tab));
    let _ = state.set_focus(Selection::Toolpath(id));
    let _ = render(&ctx, state, PANEL_WIDTH);
    render_with_outlines(&ctx, state, PANEL_WIDTH)
}
