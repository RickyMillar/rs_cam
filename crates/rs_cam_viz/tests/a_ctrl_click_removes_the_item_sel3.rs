//! B4 (SEL-3): a Ctrl-click in pick mode removes the item.
//!
//! # The claim
//!
//! 1. `PickClick::primary` is the one translation of the modifiers of a
//!    pick click: Ctrl (Cmd on macOS) wins over Shift.
//! 2. `click_event` with `Ctrl` on an item gives `RemovePick`; on an item
//!    whose chip is off it gives no event (that item is not selected).
//!    `Plain` still toggles and `Shift` adds.
//! 3. Through the controller: a pick, then a Ctrl-click on the same item,
//!    leaves the selection without it, and the Ctrl-click is one undo
//!    step. A Ctrl-click on the default selection writes nothing (O-PICK:
//!    core `click_edit` answers `AddPick` there).

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::{Item, ItemKind, ItemSet, MatchKey, Selection, Shape, SourceRef};
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, SelectionTarget, ToolpathConfig};
use rs_cam_viz::compute::{
    CollisionRequest, ComputeBackend, ComputeLane, ComputeMessage, ComputeRequest,
    GenerationControl, LaneSnapshot, SimulationRequest, ToolpathSubmitOutcome,
};
use rs_cam_viz::controller::AppController;
use rs_cam_viz::state::Workspace;
use rs_cam_viz::state::pick::{PickMode, held_kinds};
use rs_cam_viz::state::selection::Selection as UiSelection;
use rs_cam_viz::ui::AppEvent;
use rs_cam_viz::ui::pick_bar::{PickClick, click_event};

/// A compute backend that accepts every submission and returns nothing.
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

fn square(layer: &str, x: f64) -> Item {
    Item::new(
        SourceRef::ToolpathModel,
        Some(layer.into()),
        Shape::Loop(Polygon2::new(vec![
            P2::new(x, 10.0),
            P2::new(x + 20.0, 10.0),
            P2::new(x + 20.0, 30.0),
            P2::new(x, 30.0),
        ])),
    )
    .unwrap()
}

/// Two loops (CUT, TEXT) and one Pocket at its default selection, focused,
/// with its pick mode on.
fn controller() -> (AppController<SilentBackend>, [MatchKey; 2]) {
    let items = vec![square("CUT", 10.0), square("TEXT", 50.0)];
    let keys = [0, 1].map(|i| items[i].key().clone());
    let mut builder = ProjectSessionBuilder::new()
        .stock(StockConfig {
            x: 100.0,
            y: 80.0,
            z: Some(20.0),
            origin_x: 0.0,
            origin_y: 0.0,
            origin_z: -20.0,
            auto_from_model: false,
            ..StockConfig::default()
        })
        .tool(ToolConfig::new_default(
            ToolId::from_raw(1),
            ToolType::EndMill,
        ));
    let model_id = builder.add_model(LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(0),
        name: "drawing.dxf".to_owned(),
        mesh: None,
        items: Arc::new(ItemSet::new(items)),
        path: PathBuf::from("drawing.dxf"),
        kind: Some(ModelKind::Dxf),
        units: Some(ModelUnits::Millimeters),
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    });
    let tc = ToolpathConfig {
        shoe: Default::default(),
        id: rs_cam_core::ToolpathId::from_raw(0),
        name: "Pocket".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(Default::default()),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: Some(rs_cam_core::ids::ToolId::from_raw(1)),
        model_id,
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
    let _ = builder.add_toolpath(0, tc).unwrap();
    let mut c = AppController::with_backend(SilentBackend);
    c.state_mut().replace_session(builder.build());
    let tc = c.state().session().toolpath_configs()[0].clone();
    let state = c.state_mut();
    state.workspace = Workspace::Toolpaths;
    let _ = state.set_focus(UiSelection::Toolpath(tc.id));
    state.gui.selection_ui.set_pick(Some(PickMode::new(
        tc.id,
        SelectionTarget::Op,
        tc.operation.capabilities().item_use,
    )));
    (c, keys)
}

fn held(c: &AppController<SilentBackend>) -> BTreeSet<ItemKind> {
    held_kinds(&c.state().session().models()[0])
}

/// The event of a pick-mode click, through the production map.
fn event(
    c: &mut AppController<SilentBackend>,
    key: &MatchKey,
    chip_off: Option<ItemKind>,
    click: PickClick,
) -> Option<AppEvent> {
    let item_use = c.state().session().toolpath_configs()[0]
        .operation
        .capabilities()
        .item_use;
    let held = held(c);
    let mode = c
        .state_mut()
        .gui
        .selection_ui
        .pick_mut()
        .expect("pick mode on");
    click_event(mode, item_use, &held, key.clone(), chip_off, click)
}

fn stored(c: &AppController<SilentBackend>) -> Selection {
    c.state().session().toolpath_configs()[0]
        .operation
        .selection()
        .unwrap()
        .clone()
}

#[test]
fn ctrl_wins_over_shift_sel3() {
    use egui::Modifiers;
    assert_eq!(PickClick::primary(Modifiers::NONE), PickClick::Plain);
    assert_eq!(PickClick::primary(Modifiers::SHIFT), PickClick::Shift);
    assert_eq!(PickClick::primary(Modifiers::COMMAND), PickClick::Ctrl);
    assert_eq!(
        PickClick::primary(Modifiers::COMMAND | Modifiers::SHIFT),
        PickClick::Ctrl
    );
}

#[test]
fn a_ctrl_click_maps_to_remove_pick_sel3() {
    let (mut c, [cut, _]) = controller();
    let name = |e: Option<AppEvent>| match e {
        Some(AppEvent::RemovePick { .. }) => "RemovePick",
        Some(AppEvent::TogglePick { .. }) => "TogglePick",
        Some(AppEvent::AddPick { .. }) => "AddPick",
        Some(_) => "other",
        None => "none",
    };
    assert_eq!(
        name(event(&mut c, &cut, None, PickClick::Ctrl)),
        "RemovePick"
    );
    assert_eq!(
        name(event(&mut c, &cut, Some(ItemKind::Loop), PickClick::Ctrl)),
        "none",
        "a chip-off item is not selected: no event"
    );
    assert_eq!(
        name(event(&mut c, &cut, None, PickClick::Plain)),
        "TogglePick"
    );
    assert_eq!(name(event(&mut c, &cut, None, PickClick::Shift)), "AddPick");
}

#[test]
fn a_ctrl_click_removes_the_item_in_one_undo_step_sel3() {
    let (mut c, [cut, text]) = controller();
    let source = |k: &MatchKey| MatchKey {
        source: SourceRef::ToolpathModel,
        ..k.clone()
    };

    // A Ctrl-click on the default selection writes nothing.
    let before = stored(&c);
    let undo_before = c.state().history.undo_len();
    let e = event(&mut c, &cut, None, PickClick::Ctrl).expect("an event");
    c.handle_internal_event(e);
    assert_eq!(stored(&c), before, "a Ctrl-click on the default wrote");
    assert_eq!(c.state().history.undo_len(), undo_before, "no undo step");

    // Two shift-clicks pick both loops (the first replaces the default).
    for key in [&cut, &text] {
        let e = event(&mut c, key, None, PickClick::Shift).expect("an event");
        c.handle_internal_event(e);
    }
    assert_eq!(stored(&c).picks().len(), 2);

    // A Ctrl-click on the CUT loop removes it, in one undo step.
    let undo_before = c.state().history.undo_len();
    let e = event(&mut c, &cut, None, PickClick::Ctrl).expect("an event");
    c.handle_internal_event(e);
    let after = stored(&c);
    assert_eq!(after.picks().len(), 1, "{after:?}");
    assert!(
        after.picks()[0].matches(&source(&text)),
        "the TEXT loop stays"
    );
    assert_eq!(
        c.state().history.undo_len(),
        undo_before + 1,
        "the Ctrl-click is one undo step"
    );

    // A second Ctrl-click on the removed item writes nothing.
    let e = event(&mut c, &cut, None, PickClick::Ctrl).expect("an event");
    c.handle_internal_event(e);
    assert_eq!(stored(&c), after, "a Ctrl-click never adds");

    // Undo restores the CUT loop.
    c.handle_internal_event(AppEvent::Undo);
    assert_eq!(stored(&c).picks().len(), 2, "undo restores the item");
}
