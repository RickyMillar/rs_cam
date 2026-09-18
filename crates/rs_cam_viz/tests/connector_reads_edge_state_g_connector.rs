//! G-CONNECTOR sentry: the gutter connector is the card's one dependency
//! surface, and it reads the core's edge model.
//!
//! # What this replaces
//!
//! The Rest card carried a `dep` / `no dep` badge with its own predicate
//! (`rest_badge`). R3 of `planning/gen_sim_rest_ux_2026-09-18/` folds that
//! badge into the connector: a `PrevTool` edge says the same three things
//! (`Resolved` is `Ready`, `Stale` is `Pending`, `Missing` is `Broken`) and
//! it says them for every dependency kind, not for Rest alone.
//! `tests/rest_badge_one_predicate_g_restbadge.rs` is deleted; its eight
//! cases live in arm 2 below, so G-RESTBADGE keeps its guard.
//!
//! # The four arms
//!
//! 1. Role, stroke and hover for the nine `(EdgeKind, EdgeState)` pairs.
//! 2. G-RESTBADGE, carried over: the `PrevTool` edge is `Broken` exactly
//!    when the static validator refuses the same configuration.
//! 3. Geometry: the elbow sits on the gutter centre line, and an off-list
//!    source draws a stub rather than a line to nothing.
//! 4. Non-vacuity: the panel calls `draw_connectors(` and names neither the
//!    deleted badge nor its words.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::ToolpathId;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::dependencies::{EdgeKind, EdgeState, primary_edges, state as edge_state};
use rs_cam_core::session::{
    AddSetupArgs, AddToolpathArgs, AdoptResultArgs, Command, InvalidateToolpathInputsArgs,
    LoadedModel, ProjectSession, ProjectSessionBuilder, SetToolpathEnabledArgs, ToolpathConfig,
};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::job::{ModelId, ModelKind, ModelUnits};
use rs_cam_viz::state::runtime::{ComputeStatus, ToolpathRuntime};
use rs_cam_viz::state::toolpath::{OperationType, ToolpathEntry};
use rs_cam_viz::ui::components::Role;
use rs_cam_viz::ui::properties::{ToolpathValidationContext, validate_toolpath};
use rs_cam_viz::ui::tokens;
use rs_cam_viz::ui::toolpath_panel::{
    RowGeometry, arrow_tip, connector_path, edge_hover, edge_role, edge_stroke, rail_gaps,
};

const PANEL_SRC: &str = include_str!("../src/ui/toolpath_panel.rs");

const ROUGH_TOOL: usize = 1;
const REST_TOOL: usize = 2;
const MODEL_A: usize = 4;
const MODEL_B: usize = 5;

const VALIDATOR_REFUSAL: &str = "earlier enabled operation";

// ── arm 1 — role, stroke and hover for the nine pairs ───────────────────

const KINDS: [EdgeKind; 3] = [EdgeKind::Stock, EdgeKind::Regions, EdgeKind::PrevTool];
const STATES: [EdgeState; 3] = [EdgeState::Ready, EdgeState::Pending, EdgeState::Broken];

/// The role ladder of §4.4: a pass, a wait, a fault.
///
/// The first drawing gave a ready edge a 1 point `HAIRLINE` grey, on the
/// reading that structure is not a verdict. On screen the operator could see
/// nothing at all, and ruled on 2026-09-18 that a satisfied dependency is a
/// thing to confirm: it draws GREEN, at the `Ok` role, like any other pass.
#[test]
fn every_edge_state_takes_its_own_role_g_connector() {
    assert_eq!(edge_role(EdgeState::Ready), Role::Ok);
    assert_eq!(edge_role(EdgeState::Pending), Role::Caution);
    assert_eq!(edge_role(EdgeState::Broken), Role::Danger);

    let ready = edge_stroke(EdgeState::Ready);
    let pending = edge_stroke(EdgeState::Pending);
    let broken = edge_stroke(EdgeState::Broken);

    // Each state DRAWS in its own role's colour. A line the operator cannot
    // see reports nothing, so this is the arm that keeps the ruling.
    assert_eq!(ready.colour, Role::Ok.text());
    assert_eq!(pending.colour, Role::Caution.text());
    assert_eq!(broken.colour, Role::Danger.text());
    assert_ne!(
        ready.colour,
        tokens::HAIRLINE,
        "a connector is never drawn in the border grey: that was the \
         invisible first drawing"
    );

    // Never a hairline, whatever the state.
    for (name, spec) in [("ready", ready), ("pending", pending), ("broken", broken)] {
        assert!(
            spec.width >= 2.0,
            "the {name} connector is {} points and the floor is 2",
            spec.width
        );
    }

    // §2.6 rule 3: colour is never the only channel. A ready edge is the
    // only SOLID one, and the width is the third channel, because a twelve
    // point gutter cannot hold a glyph.
    assert!(
        ready.dash.is_none(),
        "a ready edge is the one solid line, which is its second channel"
    );
    assert!(
        pending.dash.is_some() && broken.dash.is_some(),
        "an unready edge must carry a dash, or the colour is the only channel"
    );
    assert!(
        broken.width > pending.width,
        "a broken edge is heavier than a pending one: {} vs {}",
        broken.width,
        pending.width
    );
}

/// Nine pairs, nine distinct hovers, and each names its source.
#[test]
fn every_edge_pair_says_something_different_g_connector() {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for kind in KINDS {
        for state in STATES {
            let hover = edge_hover(Some("Back Rough"), kind, state, false);
            assert!(
                !hover.is_empty(),
                "{kind:?}/{state:?} says nothing on hover"
            );
            assert!(
                seen.insert(hover.clone()),
                "{kind:?}/{state:?} repeats a hover another pair already uses: {hover}"
            );
        }
    }
    assert_eq!(
        seen.len(),
        9,
        "non-vacuity: nine pairs must yield nine strings"
    );

    // The two unready states say WHY, and a ready one does not have to.
    for kind in KINDS {
        let ready = edge_hover(Some("Back Rough"), kind, EdgeState::Ready, false);
        for state in [EdgeState::Pending, EdgeState::Broken] {
            let unready = edge_hover(Some("Back Rough"), kind, state, false);
            assert!(
                unready.len() > ready.len(),
                "{kind:?}/{state:?} must add a reason to the ready wording: \
                 {unready:?} against {ready:?}"
            );
        }
    }

    // Seven of the nine name the source. The two that do not are the arms
    // where there IS no source to name.
    let named = KINDS
        .iter()
        .flat_map(|k| STATES.iter().map(move |s| (*k, *s)))
        .filter(|(k, s)| edge_hover(Some("Back Rough"), *k, *s, false).contains("Back Rough"))
        .count();
    assert!(
        named >= 7,
        "the hover exists to name the source, and only {named} of nine do"
    );
}

/// A source that is drawn can be clicked, and the hover says so.
#[test]
fn a_drawn_source_offers_the_click_g_connector() {
    let quiet = edge_hover(Some("Back Rough"), EdgeKind::Stock, EdgeState::Ready, false);
    let clickable = edge_hover(Some("Back Rough"), EdgeKind::Stock, EdgeState::Ready, true);
    assert!(
        !quiet.contains("Click"),
        "a source that is not in the list must not offer a click: {quiet}"
    );
    assert!(
        clickable.starts_with(&quiet) && clickable.contains("Click to select it."),
        "a drawn source adds the click line to the same wording: {clickable}"
    );
}

// ── arm 2 — G-RESTBADGE, carried over ───────────────────────────────────

fn polygon_model(id: usize) -> LoadedModel {
    LoadedModel {
        id,
        path: PathBuf::from(format!("model_{id}.svg")),
        name: format!("2D {id}"),
        kind: Some(ModelKind::Svg),
        mesh: None,
        polygons: Some(Arc::new(vec![Polygon2::rectangle(
            -10.0, -10.0, 10.0, 10.0,
        )])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    }
}

fn tool(id: usize, diameter: f64) -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(id), ToolType::EndMill);
    tool.diameter = diameter;
    tool
}

fn toolpath(name: &str, tool_id: usize, model_id: usize, op: OperationConfig) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(0), // assigned by session.add_toolpath
        name: name.to_owned(),
        enabled: true,
        operation: op,
        dressups: Default::default(),
        heights: Default::default(),
        tool_id,
        model_id,
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
    }
}

fn rest_op() -> OperationConfig {
    let mut op = OperationConfig::Rest(Default::default());
    if let OperationConfig::Rest(cfg) = &mut op {
        cfg.prev_tool_id = Some(ToolId(ROUGH_TOOL));
    }
    op
}

fn pocket_op() -> OperationConfig {
    OperationConfig::Pocket(Default::default())
}

/// A state with two tools, two 2D models and one setup. No toolpaths yet.
fn fresh_state() -> AppState {
    let mut state = AppState::new();
    state.session = ProjectSessionBuilder::new()
        .tool(tool(ROUGH_TOOL, 10.0))
        .tool(tool(REST_TOOL, 6.0))
        .model(polygon_model(MODEL_A))
        .model(polygon_model(MODEL_B))
        .build();
    state
}

/// A cached core result, standing for "this toolpath has been generated".
fn generated_result() -> rs_cam_core::session::ToolpathComputeResult {
    rs_cam_core::session::ToolpathComputeResult {
        op_data: rs_cam_core::ops::drill_op::OpData::Toolpath(Arc::new(
            rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
                rs_cam_core::toolpath::Toolpath::new(),
            ),
        )),
        stats: Default::default(),
        debug_trace: None,
        semantic_trace: None,
    }
}

/// Add a toolpath to `setup_idx`, give it a generated result, return its id.
fn add(state: &mut AppState, setup_idx: usize, tc: ToolpathConfig) -> ToolpathId {
    let idx = state
        .session
        .apply(Command::AddToolpath(AddToolpathArgs {
            setup_index: setup_idx,
            config: Box::new(tc),
        }))
        .unwrap()
        .created
        .expect("the AddToolpath row reports the new toolpath index");
    let id = state.session.toolpath_configs()[idx].id;
    let mut rt = ToolpathRuntime::new(true);
    rt.status = ComputeStatus::Done;
    state.gui.toolpath_rt.insert(id, rt);
    let revision = state.session.toolpath_revision(idx);
    let _ = state
        .session
        .apply(Command::AdoptResult(AdoptResultArgs {
            index: idx,
            revision,
            result: Box::new(generated_result()),
        }))
        .expect("index is in range");
    id
}

/// The `PrevTool` edge of `rest_id`, read through the one core door the
/// card and the MCP wire both read.
fn prev_tool_edge(session: &ProjectSession, rest_id: ToolpathId) -> EdgeState {
    let edge = primary_edges(session)
        .into_iter()
        .find(|e| e.from == rest_id && e.kind == EdgeKind::PrevTool)
        .expect("a Rest op always declares a PrevTool edge");
    edge_state(&edge, session)
}

/// Does the static validator refuse this Rest op for want of a predecessor?
fn validator_blocks(state: &AppState, rest_id: ToolpathId, model_id: usize) -> bool {
    let tc = state
        .session
        .toolpath_configs()
        .iter()
        .find(|tc| tc.id == rest_id)
        .expect("rest op is in the session");
    let OperationConfig::Rest(rest_cfg) = &tc.operation else {
        panic!("expected a Rest op");
    };
    let mut entry = ToolpathEntry::for_operation(
        rest_id,
        tc.name.clone(),
        ToolId(REST_TOOL),
        ModelId(model_id),
        OperationType::Rest,
    );
    if let OperationConfig::Rest(cfg) = &mut entry.operation {
        cfg.prev_tool_id = rest_cfg.prev_tool_id;
    }
    validate_toolpath(
        &entry,
        &ToolpathValidationContext::from_session(&state.session),
    )
    .iter()
    .any(|e| e.contains(VALIDATOR_REFUSAL))
}

fn assert_agree(case: &str, state: &AppState, rest_id: ToolpathId, model_id: usize) -> EdgeState {
    let edge = prev_tool_edge(&state.session, rest_id);
    let refused = validator_blocks(state, rest_id, model_id);
    assert_eq!(
        edge == EdgeState::Broken,
        refused,
        "{case}: the connector reads {edge:?} while the validator refusal is \
         {refused}. One rule, two surfaces (G-RESTBADGE)."
    );
    edge
}

#[test]
fn a_predecessor_below_the_rest_op_is_broken_on_both_surfaces_g_connector() {
    let mut state = fresh_state();
    let rest_id = add(
        &mut state,
        0,
        toolpath("Rest", REST_TOOL, MODEL_A, rest_op()),
    );
    let _below = add(
        &mut state,
        0,
        toolpath("Pocket", ROUGH_TOOL, MODEL_A, pocket_op()),
    );
    let edge = assert_agree("(a) predecessor below", &state, rest_id, MODEL_A);
    assert_eq!(edge, EdgeState::Broken);
}

#[test]
fn a_disabled_predecessor_above_is_broken_on_both_surfaces_g_connector() {
    let mut state = fresh_state();
    let rough_id = add(
        &mut state,
        0,
        toolpath("Pocket", ROUGH_TOOL, MODEL_A, pocket_op()),
    );
    let rest_id = add(
        &mut state,
        0,
        toolpath("Rest", REST_TOOL, MODEL_A, rest_op()),
    );
    let rough_idx = state
        .session
        .toolpath_configs()
        .iter()
        .position(|tc| tc.id == rough_id)
        .unwrap();
    let _ = state
        .session
        .apply(Command::SetToolpathEnabled(SetToolpathEnabledArgs {
            index: rough_idx,
            enabled: false,
        }))
        .unwrap();
    let edge = assert_agree("(b) predecessor disabled", &state, rest_id, MODEL_A);
    assert_eq!(edge, EdgeState::Broken);
}

#[test]
fn a_qualifying_predecessor_is_ready_on_both_surfaces_g_connector() {
    let mut state = fresh_state();
    let _rough = add(
        &mut state,
        0,
        toolpath("Pocket", ROUGH_TOOL, MODEL_A, pocket_op()),
    );
    let rest_id = add(
        &mut state,
        0,
        toolpath("Rest", REST_TOOL, MODEL_A, rest_op()),
    );
    let edge = assert_agree("(c) qualifying predecessor", &state, rest_id, MODEL_A);
    assert_eq!(
        edge,
        EdgeState::Ready,
        "(c): a generated predecessor is Ready, not Pending"
    );
}

/// The old `Stale` badge. A predecessor that has not generated is a WAIT.
#[test]
fn a_predecessor_that_has_not_generated_is_pending_g_connector() {
    let mut state = fresh_state();
    let rough_id = add(
        &mut state,
        0,
        toolpath("Pocket", ROUGH_TOOL, MODEL_A, pocket_op()),
    );
    let rest_id = add(
        &mut state,
        0,
        toolpath("Rest", REST_TOOL, MODEL_A, rest_op()),
    );
    let rough_idx = state
        .session
        .toolpath_configs()
        .iter()
        .position(|tc| tc.id == rough_id)
        .unwrap();
    let _ = state
        .session
        .apply(Command::InvalidateToolpathInputs(
            InvalidateToolpathInputsArgs { index: rough_idx },
        ))
        .unwrap();
    let edge = assert_agree("(c2) pending predecessor", &state, rest_id, MODEL_A);
    assert_eq!(
        edge,
        EdgeState::Pending,
        "(c2): the predecessor exists, so the validator passes and the edge waits"
    );
}

#[test]
fn a_predecessor_in_another_setup_is_broken_on_both_surfaces_g_connector() {
    let mut state = fresh_state();
    let flip = state
        .session
        .apply(Command::AddSetup(AddSetupArgs {
            name: Some("Flip".to_owned()),
            face_up: FaceUp::Bottom,
        }))
        .expect("the session accepts a second setup")
        .created
        .expect("the AddSetup row reports the new setup index");
    let _other = add(
        &mut state,
        0,
        toolpath("Pocket", ROUGH_TOOL, MODEL_A, pocket_op()),
    );
    let rest_id = add(
        &mut state,
        flip,
        toolpath("Rest", REST_TOOL, MODEL_A, rest_op()),
    );
    let edge = assert_agree("(d) predecessor in another setup", &state, rest_id, MODEL_A);
    assert_eq!(edge, EdgeState::Broken);
}

#[test]
fn a_predecessor_on_another_model_is_broken_on_both_surfaces_g_connector() {
    let mut state = fresh_state();
    let _other_model = add(
        &mut state,
        0,
        toolpath("Pocket", ROUGH_TOOL, MODEL_B, pocket_op()),
    );
    let rest_id = add(
        &mut state,
        0,
        toolpath("Rest", REST_TOOL, MODEL_A, rest_op()),
    );
    let edge = assert_agree("predecessor on another model", &state, rest_id, MODEL_A);
    assert_eq!(edge, EdgeState::Broken);
}

#[test]
fn a_predecessor_with_another_tool_is_broken_on_both_surfaces_g_connector() {
    let mut state = fresh_state();
    let _wrong_tool = add(
        &mut state,
        0,
        toolpath("Pocket", REST_TOOL, MODEL_A, pocket_op()),
    );
    let rest_id = add(
        &mut state,
        0,
        toolpath("Rest", REST_TOOL, MODEL_A, rest_op()),
    );
    let edge = assert_agree("predecessor with the wrong tool", &state, rest_id, MODEL_A);
    assert_eq!(edge, EdgeState::Broken);
}

#[test]
fn no_previous_tool_configured_is_broken_g_connector() {
    let mut state = fresh_state();
    let _rough = add(
        &mut state,
        0,
        toolpath("Pocket", ROUGH_TOOL, MODEL_A, pocket_op()),
    );
    let rest_id = add(
        &mut state,
        0,
        toolpath(
            "Rest",
            REST_TOOL,
            MODEL_A,
            OperationConfig::Rest(Default::default()),
        ),
    );
    assert_eq!(prev_tool_edge(&state.session, rest_id), EdgeState::Broken);
}

// ── arm 3 — geometry ────────────────────────────────────────────────────

/// One row at `top`: a card 200 points wide with its swatch inside the
/// card's own 3 point left padding, which is where the real one sits.
fn row(top: f32) -> RowGeometry {
    let card = egui::Rect::from_min_size(egui::pos2(40.0, top), egui::vec2(200.0, 26.0));
    RowGeometry {
        card,
        swatch: egui::Rect::from_min_size(
            egui::pos2(card.left() + 3.0, top + 5.0),
            egui::vec2(8.0, 16.0),
        ),
        colour: egui::Color32::from_rgb(0x30, 0xA0, 0x60),
    }
}

/// The rail is STRAIGHT, and it runs down the swatch column.
///
/// The first drawing hung it in a gutter left of the cards, with a jog out
/// of the source row and a jog back into the dependent one. On screen it
/// read as a bracket beside the list rather than a line between two rows
/// (operator, 2026-09-18). The swatch column is the one x every row already
/// shares.
#[test]
fn the_rail_runs_straight_down_the_swatch_column_g_connector() {
    let source = row(0.0);
    let target = row(60.0);
    let path = connector_path(Some(source), target);

    assert_eq!(path.points.len(), 2, "the rail is straight: {path:?}");
    let (head, foot) = (path.points[0], path.points[1]);
    assert!(
        (head.x - foot.x).abs() < f32::EPSILON,
        "a rail with a jog in it is the bracket this replaced: {path:?}"
    );
    assert!(
        (head.x - target.swatch.center().x).abs() < f32::EPSILON,
        "the rail runs at the SWATCH centre, not beside the card: rail {} \
         swatch {}",
        head.x,
        target.swatch.center().x
    );
    assert!(
        (head.x - source.swatch.center().x).abs() < f32::EPSILON,
        "every row shares that column, so the rail leaves the source swatch \
         dead centre too"
    );
    assert!(
        (head.y - source.swatch.bottom()).abs() < f32::EPSILON,
        "the rail starts at the bottom of the SOURCE swatch: {path:?}"
    );
    assert!(
        head.x > target.card.left(),
        "the rail is INSIDE the card now, so the list keeps its full width"
    );
}

/// The arrowhead points DOWN into the dependent swatch, and stops above it.
#[test]
fn the_arrowhead_lands_on_top_of_the_dependent_swatch_g_connector() {
    let target = row(60.0);
    let path = connector_path(Some(row(0.0)), target);
    let tip = arrow_tip(target);

    assert_eq!(tip, path.tip, "the path reports the tip the caller paints");
    assert!(
        (tip.x - target.swatch.center().x).abs() < f32::EPSILON,
        "the arrow is centred on the swatch it points into: {tip:?}"
    );
    let gap = target.swatch.top() - tip.y;
    assert!(
        gap > 0.0 && gap <= 2.0,
        "the tip stops just ABOVE the swatch, and this one is {gap} points \
         off it"
    );
    let foot = path.points[1];
    assert!(
        foot.y < tip.y,
        "the rail stops short of the tip, so the line and the arrowhead do \
         not overlap: foot {} tip {}",
        foot.y,
        tip.y
    );
    assert!(
        tip.y - foot.y <= 6.0,
        "the arrowhead is small: it is {} points long",
        tip.y - foot.y
    );
}

/// A chain A to B to C is two COLLINEAR segments on one rail.
#[test]
fn a_chain_is_one_rail_with_the_swatches_threaded_on_it_g_connector() {
    let a = row(0.0);
    let b = row(40.0);
    let c = row(80.0);
    let first = connector_path(Some(a), b);
    let second = connector_path(Some(b), c);

    for point in first.points.iter().chain(second.points.iter()) {
        assert!(
            (point.x - a.swatch.center().x).abs() < f32::EPSILON,
            "every point of a chain sits on one column: {point:?}"
        );
    }
    assert!(
        (second.points[0].y - b.swatch.bottom()).abs() < f32::EPSILON,
        "the second segment leaves B's OWN swatch, which is where the first \
         one arrived: {second:?}"
    );
    assert!(
        first.tip.y < b.swatch.top() && second.points[0].y > b.swatch.top(),
        "B's swatch sits BETWEEN the two segments, threaded on the rail"
    );
}

/// An off-list source draws a short rail above the row, not a line to
/// nothing.
#[test]
fn an_off_list_source_draws_a_stub_g_connector() {
    let target = row(200.0);
    let path = connector_path(None, target);
    let stub = path.stub.expect("an undrawn source draws the stub");

    assert!(
        (stub.x - target.swatch.center().x).abs() < f32::EPSILON,
        "the stub keeps the rail's column: {path:?}"
    );
    assert!(
        stub.y < target.swatch.top(),
        "the stub sits ABOVE the swatch, where the source would have been"
    );
    let span = target.swatch.top() - stub.y;
    assert!(
        span > 0.0 && span < target.card.height(),
        "the stub is short: it says `not in this list`, it does not pretend \
         to reach a row. It spans {span} points."
    );
}

/// A rail yields the operator no hit strip over a swatch it crosses.
///
/// The rail runs down the swatch column, so a rail to a row two below passes
/// through the swatch of the row between. That band is the crossed row's own
/// drag grip, and a connector that took its clicks would make the row
/// undraggable.
#[test]
fn a_crossed_swatch_keeps_its_own_clicks_g_connector() {
    // A rail from y=20 to y=100 across one swatch at 40..=56.
    let gaps = rail_gaps(20.0, 100.0, &[(40.0, 56.0)]);
    assert_eq!(gaps, vec![(20.0, 40.0), (56.0, 100.0)]);

    // Two crossings, given out of order, and one that misses the rail.
    let gaps = rail_gaps(0.0, 100.0, &[(60.0, 70.0), (200.0, 216.0), (20.0, 30.0)]);
    assert_eq!(gaps, vec![(0.0, 20.0), (30.0, 60.0), (70.0, 100.0)]);

    // A clear rail is one strip, and a fully covered one is none.
    assert_eq!(rail_gaps(0.0, 40.0, &[]), vec![(0.0, 40.0)]);
    assert!(rail_gaps(10.0, 20.0, &[(0.0, 30.0)]).is_empty());
}

// ── arm 4 — non-vacuity ─────────────────────────────────────────────────

#[test]
fn the_panel_draws_connectors_and_no_badge_g_connector() {
    assert!(
        PANEL_SRC.len() > 10_000,
        "the panel source is {} bytes, which is too small to be the \
         operations panel",
        PANEL_SRC.len()
    );
    let code: String = PANEL_SRC
        .lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        code.contains("draw_connectors("),
        "the card's dependency surface is the connector; the panel must call it"
    );
    assert!(
        code.contains("primary_edges("),
        "the connector reads the core's one edge model, the same door the \
         MCP `depends_on` row reads (W5)"
    );
    for gone in ["draw_rest_badge(", "RestBadge", "\"no dep\"", "\"dep\""] {
        assert!(
            !code.contains(gone),
            "R3 folded the Rest `dep` badge into the connector, and {gone} \
             came back. One idea in two places is what R3 deleted."
        );
    }
    assert!(
        code.contains("egui::DragAndDrop::has_payload_of_type"),
        "the connector pass must stand down during a drag: the rects move, \
         so a line drawn then is wrong as well as ugly"
    );
}
