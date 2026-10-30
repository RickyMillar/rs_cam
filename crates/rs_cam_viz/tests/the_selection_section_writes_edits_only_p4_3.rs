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
//! headless `Context` and clicks each control of the Geometry tab (GT-1):
//! the row `⋯` then `Rule…` then `Add` (one `Replace` from the facts of a
//! picked item, GT-3), the row `×` (`RemovePick`), the header `⋯` then
//! "Remove rule: …" (`RemoveRule`) and "Clear selection" (W3h), on the op
//! target of a Drill and on the boundary target of a Pocket. Each click must push exactly ONE `AppEvent::EditToolpathSelection`
//! of the expected edit, and the stored operation and boundary must not
//! change in the draw.
//!
//! A second test measures lead ruling L-2 as W3a states it: a refusal of
//! the selection has one owner, the header of the selection list (its
//! Danger mark and its hover); no line paints it and the issue stack holds
//! no row for it.
//!
//! # NOT MEASURED
//!
//! What the controller does with the edit: `controller/tests/selection.rs`
//! measures the undo, and the core rows carry their own sentries.

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
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
        feeds_lock: Default::default(),
        id: rs_cam_core::ToolpathId::from_raw(0),
        name: name.to_owned(),
        enabled: true,
        operation,
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: Some(rs_cam_core::ids::ToolId::from_raw(1)),
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
    let _ = state.set_focus(UiSelection::Toolpath(id));
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
            // W2b: the controls are in the tab body. The issue rows above
            // the tab strip paint the same `✕` glyph as a label, so a match
            // counts only below the strip.
            let strip_y = out
                .shapes
                .iter()
                .find_map(|clipped| match &clipped.shape {
                    egui::epaint::Shape::Text(text)
                        if text.galley.job.text.ends_with("Geometry") =>
                    {
                        Some(clipped.shape.visual_bounding_rect().bottom())
                    }
                    _ => None,
                })
                .unwrap_or(f32::NEG_INFINITY);
            for clipped in &out.shapes {
                if let egui::epaint::Shape::Text(text) = &clipped.shape {
                    let t = text.galley.job.text.clone();
                    let centre = clipped.shape.visual_bounding_rect().center();
                    // A label that ends with ':' matches the texts that
                    // start with it ("Remove rule:").
                    let hit = t == label || (label.ends_with(':') && t.starts_with(label));
                    if hit && centre.y > strip_y && !places.iter().any(|p| p.distance(centre) < 1.0)
                    {
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

    // W3h: a new op holds no item, so the Drill starts at its new-op
    // default, and a pick is the selection.
    apply(&mut state, 0, SelectionTarget::Op, SelectionEdit::Reset);
    apply(&mut state, 1, SelectionTarget::Op, SelectionEdit::Reset);
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
        SelectionEdit::AddPick(circle.clone()),
    );

    // W3h: "Rule…" is in the ⋯ menu of the row (the header menu is the
    // first ⋯, the row menu the second). The menu and "Rule…" open the
    // offers and send nothing.
    let before = stored(&state);
    let menu = click(&ctx, &mut state, MENU, 1);
    assert!(edits(&menu).is_empty(), "the ⋯ menu only opens");
    let opened = click(&ctx, &mut state, "Rule\u{2026}", 0);
    assert!(edits(&opened).is_empty(), "Rule… only opens the offers");
    assert_eq!(stored(&state), before);
    assert!(
        state.gui.selection_ui.offers_for.is_some(),
        "Rule… must open the offers"
    );
    // The first offer: the circles of the picked diameter, as ONE Replace
    // that holds the rule and no pick (the rule holds the pick).
    let (target, edit) = one_edit(&ctx, &mut state, "Add", 0);
    assert_eq!(target, SelectionTarget::Op);
    let SelectionEdit::Replace(replaced) = edit else {
        panic!("an offer must Replace; sent {edit:?}");
    };
    let [rule] = replaced.rules() else {
        panic!("the offer gives one rule: {replaced:?}");
    };
    assert_eq!(rule.source, SourceRef::ToolpathModel);
    assert_eq!(rule.filter.kinds, KindFilter::Only(vec![ItemKind::Circle]));
    assert!(
        rule.filter.diameter.is_some(),
        "the offer names the diameter"
    );
    assert!(
        replaced.picks().is_empty(),
        "the rule replaces the pick it holds"
    );
    assert!(
        state.gui.selection_ui.offers_for.is_none(),
        "Add closes the offers"
    );

    // The row × of a pick sends RemovePick (core turns a rule member into
    // picks, GT-Q8).
    let (_, edit) = one_edit(&ctx, &mut state, "\u{00D7}", 0);
    assert_eq!(edit, SelectionEdit::RemovePick(circle));

    // A rule: "Remove rule: …" in the header ⋯ menu.
    apply(
        &mut state,
        0,
        SelectionTarget::Op,
        SelectionEdit::Replace(Box::new(Selection::from_rules(vec![layer_rule("HOLES")]))),
    );
    let _ = click(&ctx, &mut state, MENU, 0);
    let (_, edit) = one_edit(&ctx, &mut state, "Remove rule:", 0);
    assert_eq!(edit, SelectionEdit::RemoveRule(0));

    // "Clear selection" in the header ⋯ menu: one Replace with nothing.
    let _ = click(&ctx, &mut state, MENU, 0);
    let (_, edit) = one_edit(&ctx, &mut state, "Clear selection", 0);
    assert_eq!(edit, SelectionEdit::Replace(Box::new(Selection::empty())));

    // The boundary target of the Pocket (GT-Q7: the Geometry tab holds it
    // under the Boundary row). The op list of the Pocket is empty (its
    // header ⋯ is the first), so the boundary header ⋯ is the second.
    select(&mut state, 1);
    let ctx = self::ctx();
    let _ = click(&ctx, &mut state, MENU, 1);
    let (target, edit) = one_edit(&ctx, &mut state, "Remove rule:", 0);
    assert_eq!(target, SelectionTarget::Boundary);
    assert_eq!(edit, SelectionEdit::RemoveRule(0));
    // The row × of a boundary rule member: GT-Q8.
    let (target, edit) = one_edit(&ctx, &mut state, "\u{00D7}", 0);
    assert_eq!(target, SelectionTarget::Boundary);
    assert!(matches!(edit, SelectionEdit::RemovePick(_)), "{edit:?}");
}

/// The text of the ⋯ menu act of a list line.
const MENU: &str = "\u{22EF}";

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

/// L-2, W2b, W3a, W3h: a selection refusal has one owner, the header of
/// the selection list. The core finding (field key `selection`; W3h: the
/// empty selection is "Select the geometry to cut") is its Danger mark and
/// its hover; no line paints the refusal, the empty list paints its
/// placeholder, and the issue stack holds no row for it. The status line
/// reads "Not ready".
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
        whole.is_empty(),
        "the refusal is the hover of the list header, not a painted line: {whole:?}"
    );
    assert!(
        texts.iter().any(|t| t == "Selection") && texts.iter().any(|t| t == "Select holes"),
        "the list header owns the state; the placeholder names the pick; painted: {texts:?}"
    );
    assert!(
        texts.iter().any(|t| t == "Not ready"),
        "the status line must say why Generate is off in one word; painted: {texts:?}"
    );
    let face = rs_cam_core::diagnostics::presentation::face(
        rs_cam_core::diagnostics::ids::PRECOND_SELECTION_EMPTY,
    );
    assert_eq!(
        texts.iter().filter(|t| **t == face).count(),
        0,
        "no issue row: the finding is the mark of the list header; painted: {texts:?}"
    );
    // Non-vacuity: the placement gives the finding to the field row.
    let id = state.session().toolpath_configs()[0].id;
    let placed = properties::placed_findings(id, &state).expect("the toolpath exists");
    assert!(
        placed.by_field.iter().any(|d| d.face == face),
        "the finding sits on the selection list: {:?}",
        placed.by_field
    );
}
