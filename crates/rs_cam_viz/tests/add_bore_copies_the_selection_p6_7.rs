//! Geometry selection P6.7 (L-17): "Add a Bore for these" on the Drill hole
//! rows adds one Bore after the Drill with a copy of the Drill selection,
//! and one undo removes it.
//!
//! # The claim
//!
//! - The Selection section of a Drill shows the action only with a hole
//!   size that the Drill leaves out as larger than the tool. With a 10 mm
//!   tool the drawing holds no such size (6 mm is smaller, 10 mm is the
//!   tool size), and the action is not painted. A Bore shows no action.
//! - With a 6 mm tool the 10 mm circle is larger: one click sends ONE
//!   `AddBoreFromDrill`. The controller adds one Bore, directly after the
//!   Drill in the setup, with the Drill selection (a layer rule, not the
//!   default), the Drill model and tool, and one undo step.
//! - One undo removes the Bore. A redo adds it again, and the next undo
//!   removes that Bore too (the step names the new id).
//!
//! Red proof (P6.7): write `Selection::drawing()` in place of the copy in
//! `ProjectSession::add_bore_from_drill`; the selection assertion fails.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test add_bore_copies_the_selection_p6_7
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod inspector_harness_p6_7;

use std::sync::Arc;

use inspector_harness_p6_7 as h;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::operation_configs::DrillConfig;
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::P2;
use rs_cam_core::selection::{Filter, Item, ItemSet, Rule, Selection, Shape, SourceRef};
use rs_cam_core::session::ProjectSessionBuilder;
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::AppController;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection as UiSelection;
use rs_cam_viz::ui::{AppEvent, properties};

/// A backend that runs nothing: the test reads the session only.
struct SilentBackend;

impl ComputeBackend for SilentBackend {
    fn submit_toolpath(&mut self, _request: ComputeRequest) -> ToolpathSubmitOutcome {
        ToolpathSubmitOutcome::Queued
    }
    fn submit_simulation(&mut self, _request: SimulationRequest) {}
    fn submit_collision(&mut self, _request: CollisionRequest) {}
    fn cancel_lane(&mut self, _lane: ComputeLane) {}
    fn drain_results(&mut self) -> Vec<ComputeMessage> {
        Vec::new()
    }
    fn lane_snapshot(&self, lane: ComputeLane) -> LaneSnapshot {
        LaneSnapshot::idle(lane)
    }
    fn generation_control(&self) -> GenerationControl {
        GenerationControl::detached()
    }
}

/// The label of the action (the production constant is crate-private).
const ADD_A_BORE: &str = "Add a Bore for these";

/// The layer of the circles. The Drill selection is a rule on it.
const HOLES: &str = "HOLES";

/// Two 6 mm circles and one 10 mm circle on `HOLES`, and one point on
/// another layer.
fn holes() -> ItemSet {
    let layer: Option<Arc<str>> = Some(Arc::from(HOLES));
    let circle = |x: f64, d: f64| {
        Item::new(
            SourceRef::ToolpathModel,
            layer.clone(),
            Shape::circle(P2::new(x, 0.0), d),
        )
        .expect("a circle")
    };
    ItemSet::new(vec![
        circle(0.0, 6.0),
        circle(20.0, 6.0),
        circle(40.0, 10.0),
        Item::new(
            SourceRef::ToolpathModel,
            Some(Arc::from("POINTS")),
            Shape::Point(P2::new(-20.0, 0.0)),
        )
        .expect("a point"),
    ])
}

/// The Drill selection: one rule on `HOLES`. It is not the default, so a
/// copy is not a default by chance.
fn rule() -> Selection {
    Selection::from_rules(vec![Rule {
        source: SourceRef::ToolpathModel,
        filter: Filter {
            layers: vec![HOLES.to_owned()],
            ..Filter::default()
        },
    }])
}

/// A controller with the drawing, an end mill of `tool_diameter` and one
/// Drill (with [`rule`]), shown on its Geometry tab.
fn controller(tool_diameter: f64) -> AppController<SilentBackend> {
    let mut tool = ToolConfig::new_default(ToolId::from_raw(h::TOOL), ToolType::EndMill);
    tool.diameter = tool_diameter;
    let mut builder = ProjectSessionBuilder::new()
        .stock(StockConfig {
            x: 100.0,
            y: 40.0,
            z: 18.0,
            origin_x: -30.0,
            origin_y: -20.0,
            origin_z: -18.0,
            auto_from_model: false,
            ..StockConfig::default()
        })
        .tool(tool)
        .model(h::drawing_model(holes()));
    let drill = OperationConfig::Drill(DrillConfig {
        depth: 6.0,
        selection: rule(),
        ..DrillConfig::default()
    });
    builder
        .add_toolpath(0, h::toolpath("Holes", drill))
        .expect("add the Drill");
    let mut c = AppController::with_backend(SilentBackend);
    c.state_mut().replace_session(builder.build());
    let id = c.state().session().toolpath_configs()[0].id;
    let state = c.state_mut();
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    show(&mut c, id);
    c
}

/// Select the toolpath `id` on its Geometry tab.
fn show(c: &mut AppController<SilentBackend>, id: rs_cam_viz::state::toolpath::ToolpathId) {
    let state = c.state_mut();
    state.selection = UiSelection::Toolpath(id);
    state.gui.pending_toolpath_tab = Some((id, properties::ToolpathTab::Geometry));
}

/// The setup order of the toolpath ids.
fn order(c: &AppController<SilentBackend>) -> Vec<rs_cam_viz::state::toolpath::ToolpathId> {
    let session = c.state().session();
    session.list_setups()[0]
        .toolpath_indices()
        .iter()
        .map(|&i| session.toolpath_configs()[i].id)
        .collect()
}

#[test]
fn add_bore_copies_the_selection_p6_7() {
    // No size larger than the tool: no action.
    let ctx = h::ctx();
    let mut c = controller(10.0);
    let painted = h::render(&ctx, c.state_mut(), h::RAIL_WIDTH);
    assert!(
        !h::containing(&painted, "\u{00D8} 10.000").is_empty(),
        "the Drill paints its hole rows (non-vacuity)"
    );
    assert!(
        h::find(&painted, ADD_A_BORE).is_empty(),
        "no hole is larger than a 10 mm tool, so no action"
    );

    // A 6 mm tool: the 10 mm circle is larger, so the action shows.
    let ctx = h::ctx();
    let mut c = controller(6.0);
    let drill_id = c.state().session().toolpath_configs()[0].id;
    let painted = h::render(&ctx, c.state_mut(), h::RAIL_WIDTH);
    assert!(
        !h::find(&painted, ADD_A_BORE).is_empty(),
        "a hole larger than the tool shows the action; painted: {:?}",
        painted.iter().map(|p| &p.text).collect::<Vec<_>>()
    );
    assert!(!h::find(&painted, "1 hole to bore").is_empty());
    let wrapped = h::wrapped(&painted);
    assert!(wrapped.is_empty(), "IC1: no text wraps: {wrapped:?}");
    let wide = h::past_the_rail(&painted);
    assert!(wide.is_empty(), "every row fits the rail: {wide:?}");

    // One click: one command event.
    let events = h::click(&ctx, c.state_mut(), ADD_A_BORE, 0);
    let adds: Vec<&AppEvent> = events
        .iter()
        .filter(|e| matches!(e, AppEvent::AddBoreFromDrill(_)))
        .collect();
    assert_eq!(adds.len(), 1, "one click, one event: {events:?}");
    assert!(matches!(adds[0], AppEvent::AddBoreFromDrill(id) if *id == drill_id));

    let steps = c.state().history.undo_len();
    for event in events {
        c.handle_internal_event(event);
    }
    let session = c.state().session();
    assert_eq!(session.toolpath_configs().len(), 2, "one Bore is added");
    let ids = order(&c);
    assert_eq!(ids.len(), 2);
    assert_eq!(ids[0], drill_id, "the Drill stays first");
    let bore_id = ids[1];
    let (_, bore) = session
        .find_toolpath_config_by_id(bore_id)
        .expect("the Bore");
    let (_, drill) = session
        .find_toolpath_config_by_id(drill_id)
        .expect("the Drill");
    let OperationConfig::Bore(bore_op) = &bore.operation else {
        panic!("the new op is a Bore, not {}", bore.operation.label());
    };
    let OperationConfig::Drill(drill_op) = &drill.operation else {
        panic!("the first op is the Drill");
    };
    assert_eq!(bore_op.selection, rule(), "the Bore holds the Drill rule");
    assert_eq!(bore_op.selection, drill_op.selection);
    assert_eq!(bore.model_id, drill.model_id);
    assert_eq!(bore.tool_id, drill.tool_id);
    assert_eq!(
        c.state().history.undo_len(),
        steps + 1,
        "the add is one undo step"
    );
    assert_eq!(
        c.state().selection,
        UiSelection::Toolpath(drill_id),
        "the selection stays on the Drill"
    );

    assert!(session.find_toolpath_config_by_id(bore_id).is_some());

    // One undo removes the Bore.
    c.handle_internal_event(AppEvent::Undo);
    assert_eq!(c.state().session().toolpath_configs().len(), 1);
    assert_eq!(order(&c), vec![drill_id], "the undo removes the Bore");
    assert_eq!(c.state().history.undo_len(), steps);

    // A redo adds it again; the next undo removes the new Bore.
    c.handle_internal_event(AppEvent::Redo);
    assert_eq!(c.state().session().toolpath_configs().len(), 2);
    assert_eq!(order(&c)[0], drill_id);
    c.handle_internal_event(AppEvent::Undo);
    assert_eq!(
        order(&c),
        vec![drill_id],
        "the undo after a redo removes the new Bore"
    );

    // The Bore panel shows no action. (A panel switch flushes a parameter
    // snapshot into the history, so this arm runs after the undo arms.)
    c.handle_internal_event(AppEvent::Redo);
    let bore_id = order(&c)[1];
    show(&mut c, bore_id);
    let painted = h::render(&ctx, c.state_mut(), h::RAIL_WIDTH);
    assert!(
        !h::containing(&painted, "helix r").is_empty(),
        "the Bore paints its hole rows (non-vacuity)"
    );
    assert!(
        h::find(&painted, ADD_A_BORE).is_empty(),
        "a Bore offers no Bore"
    );
}
