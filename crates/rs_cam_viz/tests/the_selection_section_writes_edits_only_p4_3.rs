//! P4.3 (lead ruling L-1): the Selection section sends edits only.
//!
//! # The defect this exists to catch
//!
//! Before P4.3 one field had two writers. The drill hole selector wrote the
//! DRAFT `cfg.selection` in the draw, and the panel write-back copied the
//! draft onto the session; the face picker sent `EditToolpathSelection`. A
//! draft write skips the command row of the edit, so the two writers could
//! store two spellings of one item (P2.7) and the edit had no `Effects` of
//! its own.
//!
//! # What it measures
//!
//! The test drives the production inspector (`properties::draw`) through a
//! headless `Context` and clicks each control of the section: `+ Rule` then
//! `Add` (`Replace` under the default, `AddRule` after it), the rule `✕`
//! (`RemoveRule`), `Reset`, and the picks `Clear` (`Replace` with the rules
//! only), on the op target of a Drill and on the boundary target of a
//! Pocket. Each click must push exactly ONE `AppEvent::EditToolpathSelection`
//! of the expected edit, and the stored operation and boundary must not
//! change in the draw.
//!
//! A second test measures lead ruling L-2: a refusal of the selection
//! shows its whole text once, in the section, and the footer row names the
//! section in one line.
//!
//! # NOT MEASURED
//!
//! What the controller does with the edit: `controller/tests/selection.rs`
//! measures the undo, and the core rows carry their own sentries.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, BoundaryContainment, BoundarySource};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::selection::{
    Filter, ItemKind, KindFilter, Rule, Selection, SelectionEdit, SourceRef,
};
use rs_cam_core::session::{
    Command, EditSelectionArgs, ProjectSessionBuilder, SelectionTarget, ToolpathConfig,
};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection as UiSelection;
use rs_cam_viz::ui::{AppEvent, properties, tokens};

/// The inspector's own width in the Toolpaths workspace (`app.rs`).
const PANEL_WIDTH: f32 = 280.0;

/// Tall enough that every row of the Geometry tab is on screen.
const SCREEN_HEIGHT: f32 = 4000.0;

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

fn toolpath(name: &str, operation: OperationConfig) -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        id: rs_cam_core::ToolpathId::from_raw(0),
        name: name.to_owned(),
        enabled: true,
        operation,
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: rs_cam_core::ids::ToolId::from_raw(1),
        model_id: rs_cam_core::ids::ModelId::from_raw(0),
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

/// A rule on the layer `layer` of the toolpath model.
fn layer_rule(layer: &str) -> Rule {
    Rule {
        source: SourceRef::ToolpathModel,
        filter: Filter {
            layers: vec![layer.to_owned()],
            ..Filter::default()
        },
    }
}

/// The P2.7 drawing (squares on `CUT` and `KEEP`, two circles on `HOLES`,
/// an open line on `TEXT`), a default Drill (toolpath 0) and a Pocket
/// (toolpath 1) whose boundary is a selection of the `CUT` layer.
fn state() -> AppState {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rs_cam_core/tests/fixtures/selection_p2_7/layers.dxf");
    let model = rs_cam_core::io::load_model_file(
        &path,
        rs_cam_core::ids::ModelId::from_raw(0),
        ModelKind::Dxf,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .expect("the drawing loads");
    let mut tool = ToolConfig::new_default(ToolId::from_raw(1), ToolType::EndMill);
    tool.diameter = 3.0;
    let mut builder = ProjectSessionBuilder::new()
        .stock(StockConfig::default())
        .tool(tool)
        .model(model);
    builder
        .add_toolpath(
            0,
            toolpath(
                "Drill",
                OperationConfig::new_default(rs_cam_core::compute::catalog::OperationType::Drill),
            ),
        )
        .unwrap();
    let mut pocket = toolpath(
        "Pocket",
        OperationConfig::new_default(rs_cam_core::compute::catalog::OperationType::Pocket),
    );
    pocket.boundary = BoundaryConfig {
        enabled: true,
        source: BoundarySource::Selection {
            selection: Selection::from_rules(vec![layer_rule("CUT")]),
            holes: false,
        },
        containment: BoundaryContainment::Center,
        offset: 0.0,
    };
    builder.add_toolpath(0, pocket).unwrap();
    let mut state = AppState::new();
    state.replace_session(builder.build());
    let parts = state.split_mut();
    for config in parts.session.toolpath_configs() {
        parts
            .gui
            .toolpath_rt
            .insert(config.id, ToolpathRuntime::new(true));
    }
    select(&mut state, 0);
    state
}

/// Show toolpath `index` on its Geometry tab.
fn select(state: &mut AppState, index: usize) {
    let id = state.session().toolpath_configs()[index].id;
    state.selection = UiSelection::Toolpath(id);
    state.gui.pending_toolpath_tab = Some((id, properties::ToolpathTab::Geometry));
}

/// Apply one edit through the command row, as the controller does.
fn apply(state: &mut AppState, index: usize, target: SelectionTarget, edit: SelectionEdit) {
    let _ = state
        .apply(Command::EditSelection(EditSelectionArgs {
            index,
            target,
            edit,
        }))
        .expect("the edit applies");
}

/// What the draw could write: the stored operation and boundary of every
/// toolpath.
fn stored(state: &AppState) -> Vec<String> {
    state
        .session()
        .toolpath_configs()
        .iter()
        .map(|tc| format!("{:?} {:?}", tc.operation, tc.boundary))
        .collect()
}

/// Click the `nth` painted text equal to `label` in the production
/// inspector, and return every event the five passes pushed.
///
/// Passes 0-2 settle the layout and find the label. Pass 3 presses and
/// pass 4 releases: egui resolves a click from the widget rects of the
/// pass before, so a press and a release in one pass is not the shape of a
/// real pointer.
fn click(ctx: &egui::Context, state: &mut AppState, label: &str, nth: usize) -> Vec<AppEvent> {
    let mut target: Option<egui::Pos2> = None;
    let mut painted: Vec<String> = Vec::new();
    let mut all = Vec::new();
    for pass in 0..5 {
        let mut input_events = Vec::new();
        if pass >= 3 {
            let pos = target.unwrap_or_else(|| {
                panic!("no painted text '{label}' (#{nth}); painted: {painted:?}")
            });
            input_events.push(egui::Event::PointerMoved(pos));
            input_events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: pass == 3,
                modifiers: egui::Modifiers::default(),
            });
        }
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(PANEL_WIDTH, SCREEN_HEIGHT),
            )),
            events: input_events,
            ..Default::default()
        };
        let mut events = Vec::new();
        let mut out = ctx.run_ui(input, |ui| {
            ui.set_max_width(PANEL_WIDTH);
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_max_width(PANEL_WIDTH);
                properties::draw(ui, state, &mut events);
            });
        });
        if pass == 2 {
            painted.clear();
            // A button paints its label in two shapes at one place, so the
            // matches count by place, not by shape.
            let mut places: Vec<egui::Pos2> = Vec::new();
            for clipped in &out.shapes {
                if let egui::epaint::Shape::Text(text) = &clipped.shape {
                    let t = text.galley.job.text.clone();
                    let centre = clipped.shape.visual_bounding_rect().center();
                    if t == label && !places.iter().any(|p| p.distance(centre) < 1.0) {
                        places.push(centre);
                    }
                    painted.push(t);
                }
            }
            target = places.get(nth).copied();
        }
        out.textures_delta.clear();
        all.extend(events);
    }
    all
}

/// The selection edits among `events`, with their targets.
fn edits(events: &[AppEvent]) -> Vec<(SelectionTarget, SelectionEdit)> {
    events
        .iter()
        .filter_map(|event| match event {
            AppEvent::EditToolpathSelection { target, edit, .. } => Some((*target, edit.clone())),
            _ => None,
        })
        .collect()
}

/// Click one control and assert: one edit, and the draw wrote nothing.
fn one_edit(
    ctx: &egui::Context,
    state: &mut AppState,
    label: &str,
    nth: usize,
) -> (SelectionTarget, SelectionEdit) {
    let before = stored(state);
    let events = click(ctx, state, label, nth);
    assert_eq!(
        stored(state),
        before,
        "the '{label}' click changed the stored operation or boundary in the draw: \
         the section must send an edit, not write the draft (L-1)"
    );
    let edits = edits(&events);
    assert_eq!(
        edits.len(),
        1,
        "the '{label}' click must send exactly one EditToolpathSelection; sent {edits:?}"
    );
    edits.into_iter().next().unwrap()
}

#[test]
fn the_selection_section_writes_edits_only_p4_3() {
    let ctx = ctx();
    let mut state = state();

    // `+ Rule` opens the draft and sends nothing; `Add` under the default
    // replaces the default with the one rule (one act, one set of items).
    let before = stored(&state);
    let opened = click(&ctx, &mut state, "+ Rule", 0);
    assert!(edits(&opened).is_empty(), "+ Rule only opens the draft");
    assert_eq!(stored(&state), before);
    assert!(
        state.gui.selection_ui.rule_draft.is_some(),
        "+ Rule must open the draft"
    );
    let (target, edit) = one_edit(&ctx, &mut state, "Add", 0);
    assert_eq!(target, SelectionTarget::Op);
    let SelectionEdit::Replace(replaced) = edit else {
        panic!("Add under the default must Replace; sent {edit:?}");
    };
    let [rule] = replaced.rules() else {
        panic!("Add under the default gives one rule: {replaced:?}");
    };
    assert_eq!(rule.source, SourceRef::ToolpathModel);
    assert_eq!(
        rule.filter.kinds,
        KindFilter::Only(vec![ItemKind::Point, ItemKind::Circle]),
        "a Drill rule takes the kinds a drill serves"
    );
    assert!(
        state.gui.selection_ui.rule_draft.is_none(),
        "Add closes the draft"
    );

    // After a non-default selection, `Add` adds.
    apply(
        &mut state,
        0,
        SelectionTarget::Op,
        SelectionEdit::Replace(Box::new(Selection::from_rules(vec![layer_rule("HOLES")]))),
    );
    let _ = click(&ctx, &mut state, "+ Rule", 0);
    let (_, edit) = one_edit(&ctx, &mut state, "Add", 0);
    assert!(
        matches!(edit, SelectionEdit::AddRule(_)),
        "Add after a non-default selection must AddRule; sent {edit:?}"
    );

    // The rule's remove button.
    let (_, edit) = one_edit(&ctx, &mut state, "\u{2715}", 0);
    assert_eq!(edit, SelectionEdit::RemoveRule(0));

    // Reset, enabled away from the default.
    let (_, edit) = one_edit(&ctx, &mut state, "Reset", 0);
    assert_eq!(edit, SelectionEdit::Reset);

    // The picks Clear keeps the rules.
    let circle = state
        .session()
        .list_model_items(
            rs_cam_core::ids::ModelId::from_raw(0),
            Some("HOLES"),
            Some(ItemKind::Circle),
        )
        .unwrap()[0]
        .key
        .clone();
    apply(
        &mut state,
        0,
        SelectionTarget::Op,
        SelectionEdit::AddPick(circle),
    );
    let (_, edit) = one_edit(&ctx, &mut state, "Clear", 0);
    assert_eq!(
        edit,
        SelectionEdit::Replace(Box::new(Selection::from_rules(vec![layer_rule("HOLES")]))),
        "Clear removes the picks and keeps the rules"
    );

    // The boundary target of the Pocket: its op Reset is first (disabled at
    // the default), the boundary Reset second.
    select(&mut state, 1);
    let ctx = self::ctx();
    let (target, edit) = one_edit(&ctx, &mut state, "Reset", 1);
    assert_eq!(target, SelectionTarget::Boundary);
    assert_eq!(edit, SelectionEdit::Reset);
    let (target, edit) = one_edit(&ctx, &mut state, "\u{2715}", 1);
    assert_eq!(target, SelectionTarget::Boundary);
    assert_eq!(edit, SelectionEdit::RemoveRule(0));
}

/// Every painted text of the production inspector for `state`, after the
/// layout settles.
fn painted(ctx: &egui::Context, state: &mut AppState) -> Vec<String> {
    let mut texts = Vec::new();
    for pass in 0..3 {
        let mut events = Vec::new();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(PANEL_WIDTH, SCREEN_HEIGHT),
            )),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            ui.set_max_width(PANEL_WIDTH);
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_max_width(PANEL_WIDTH);
                properties::draw(ui, state, &mut events);
            });
        });
        if pass == 2 {
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

/// L-2: the refusal has one owner. The section prints the whole refusal;
/// the footer row prints one line that names the section, and keeps the
/// whole text for its hover.
#[test]
fn a_selection_refusal_has_one_owner_p4_3() {
    let ctx = ctx();
    let mut state = state();
    apply(
        &mut state,
        0,
        SelectionTarget::Op,
        SelectionEdit::Replace(Box::new(Selection::empty())),
    );
    let refusal = {
        let tc = &state.session().toolpath_configs()[0];
        state
            .session()
            .selection_status(tc, SelectionTarget::Op)
            .unwrap()
            .refusal
            .as_ref()
            .expect("an empty Drill selection refuses")
            .to_string()
    };
    let texts = painted(&ctx, &mut state);
    let whole: Vec<&String> = texts.iter().filter(|t| t.contains(&refusal)).collect();
    assert!(
        !whole.is_empty(),
        "the section must print the whole refusal '{refusal}'; painted: {texts:?}"
    );
    assert_eq!(
        whole.len(),
        1,
        "the whole refusal must be painted once, by the section; the header and the \
         footer name the section in one line: {whole:?}"
    );
    assert!(
        texts
            .iter()
            .any(|t| t == "The selection refuses. See Selection below."),
        "the header must say why Generate is off in one line; painted: {texts:?}"
    );
    assert!(
        texts
            .iter()
            .any(|t| t.ends_with("the selection refuses. See Selection on the Geometry tab.")),
        "the footer row must name the Selection section in one line; painted: {texts:?}"
    );
}
