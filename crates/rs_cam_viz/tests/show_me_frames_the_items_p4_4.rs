//! P4.4: "Show me" frames the items of a note and the position of a lost
//! pick.
//!
//! # The claim
//!
//! 1. A click on a note row of the Selection section ("Show me"; the note
//!    stays one `DataTable` row, IC2) sends ONE
//!    `AppEvent::FrameSelectionItems` with the keys of the skipped items of
//!    the note's kind and layer, and edits nothing.
//! 2. The "Show me" button of a lost pick sends the stored key of the pick.
//! 3. `picking::selection_frame_bounds` gives the box the camera fits: the
//!    display box of the item rings for a note, and the stored anchor grown
//!    by half the key measure for a lost pick (it has no item).
//! 4. P3.8: under the floor "each picked face", a face at the top gives a
//!    note that names that face. "Show me" on the note frames that one
//!    face, not every skipped face of its kind and layer.
//!
//! The test drives the production inspector (`properties::draw`) through a
//! headless `Context`, as `the_selection_section_writes_edits_only_p4_3`
//! does.
//!
//! Red proofs (each run, red, reverted):
//!
//! - `note_item_keys` returning no key: the note row did not answer the
//!   click and claim 1 went red (no frame request).
//! - `selection_frame_bounds` with the lost-pick arm dropped (a key with
//!   no item gives no point): claim 3 went red (`None`).
//! - `note_item_keys` with the kind-and-layer filter for an at-the-top
//!   note too (the P3.7 code): claim 4 went red (two keys, not one).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::selection::{ItemKind, MatchKey, Selection, SelectionEdit};
use rs_cam_core::session::{
    Command, EditSelectionArgs, ProjectSessionBuilder, SelectionTarget, ToolpathConfig,
};
use rs_cam_viz::interaction::picking::selection_frame_bounds;
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection as UiSelection;
use rs_cam_viz::ui::{AppEvent, properties, tokens};

const PANEL_WIDTH: f32 = 280.0;
const SCREEN_HEIGHT: f32 = 4000.0;

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

fn toolpath(name: &str, op: OperationType) -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        id: rs_cam_core::ToolpathId(0),
        name: name.to_owned(),
        enabled: true,
        operation: OperationConfig::new_default(op),
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: 1,
        model_id: 0,
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

/// The P2.7 drawing (squares on `CUT` and `KEEP`, two circles on `HOLES`,
/// an open line on `TEXT`): toolpath 0 a default Pocket (it skips the open
/// line with a note), toolpath 1 a Drill with one lost circle pick.
fn state() -> (AppState, MatchKey) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../rs_cam_core/tests/fixtures/selection_p2_7/layers.dxf");
    let model = rs_cam_core::io::load_model_file(
        &path,
        0,
        ModelKind::Dxf,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .expect("the drawing loads");
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 3.0;
    let mut builder = ProjectSessionBuilder::new()
        .stock(StockConfig::default())
        .tool(tool)
        .model(model);
    builder
        .add_toolpath(0, toolpath("Pocket", OperationType::Pocket))
        .unwrap();
    builder
        .add_toolpath(0, toolpath("Drill", OperationType::Drill))
        .unwrap();
    let mut state = AppState::new();
    state.session = builder.build();
    let circles = state
        .session
        .list_model_items(0, Some("HOLES"), Some(ItemKind::Circle))
        .unwrap();
    let mut lost = circles[0].key.clone();
    lost.anchor = [500.0, 400.0];
    let _ = state
        .session
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 1,
            target: SelectionTarget::Op,
            edit: SelectionEdit::Replace(Box::new(Selection::from_picks(vec![lost.clone()]))),
        }))
        .unwrap();
    for config in state.session.toolpath_configs() {
        state
            .gui
            .toolpath_rt
            .insert(config.id, ToolpathRuntime::new(true));
    }
    (state, lost)
}

fn select(state: &mut AppState, index: usize) {
    let id = state.session.toolpath_configs()[index].id;
    state.selection = UiSelection::Toolpath(id);
    state.gui.pending_toolpath_tab = Some((id, properties::ToolpathTab::Geometry));
}

/// Click the first painted text `label` in the production inspector and
/// return every event (passes 0-2 settle and find, 3 presses, 4 releases).
fn click(ctx: &egui::Context, state: &mut AppState, label: &str) -> Vec<AppEvent> {
    let mut target: Option<egui::Pos2> = None;
    let mut painted: Vec<String> = Vec::new();
    let mut all = Vec::new();
    for pass in 0..5 {
        let mut input_events = Vec::new();
        if pass >= 3 {
            let pos =
                target.unwrap_or_else(|| panic!("no painted text '{label}'; painted: {painted:?}"));
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
            for clipped in &out.shapes {
                if let egui::epaint::Shape::Text(text) = &clipped.shape {
                    let t = text.galley.job.text.clone();
                    if t == label && target.is_none() {
                        target = Some(clipped.shape.visual_bounding_rect().center());
                    }
                    painted.push(t);
                }
            }
        }
        out.textures_delta.clear();
        all.extend(events);
    }
    all
}

/// The one frame request among `events`, and no selection edit.
fn one_frame_request(events: &[AppEvent]) -> Vec<MatchKey> {
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, AppEvent::EditToolpathSelection { .. })),
        "Show me edits nothing"
    );
    let requests: Vec<Vec<MatchKey>> = events
        .iter()
        .filter_map(|e| match e {
            AppEvent::FrameSelectionItems { keys, .. } => Some(keys.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(requests.len(), 1, "one Show me click, one frame request");
    requests.into_iter().next().unwrap()
}

#[test]
fn show_me_frames_a_note_and_a_lost_pick_p4_4() {
    let ctx = ctx();
    let (mut state, lost) = state();

    // Claim 1: the note of the open line on TEXT.
    select(&mut state, 0);
    // The note row paints the detail mark after its text (it has a hover).
    let note = format!(
        "1 open path on layer TEXT is not cut {}",
        tokens::GLYPH_DETAIL
    );
    let keys = one_frame_request(&click(&ctx, &mut state, &note));
    let paths = state
        .session
        .list_model_items(0, Some("TEXT"), Some(ItemKind::Path))
        .unwrap();
    assert_eq!(paths.len(), 1, "the fixture holds one open line");
    assert_eq!(keys.len(), 1, "the note frames its one skipped path");
    assert!(keys[0].matches(&paths[0].key), "{keys:?}");

    // Claim 3, note arm: the box of the path ring in the display frame.
    let pocket = state.session.toolpath_configs()[0].id;
    let bbox = selection_frame_bounds(&state.session, pocket, &keys).expect("a box");
    let ring = state.session.models()[0]
        .items
        .iter()
        .find(|i| i.kind() == ItemKind::Path)
        .and_then(|i| i.shape().drawing_ring().cloned())
        .unwrap();
    let frame = rs_cam_viz::interaction::picking::toolpath_setup_frame(&state.session, pocket);
    let shown: Vec<_> = ring
        .exterior
        .iter()
        .map(|p| {
            rs_cam_viz::interaction::picking::item_display_xy(
                frame.as_ref(),
                state.session.stock_config(),
                *p,
            )
        })
        .collect();
    let min_x = shown.iter().map(|p| p.x).fold(f64::MAX, f64::min);
    let max_x = shown.iter().map(|p| p.x).fold(f64::MIN, f64::max);
    let min_y = shown.iter().map(|p| p.y).fold(f64::MAX, f64::min);
    let max_y = shown.iter().map(|p| p.y).fold(f64::MIN, f64::max);
    assert!((bbox.min.x - min_x).abs() < 1e-9 && (bbox.max.x - max_x).abs() < 1e-9);
    assert!((bbox.min.y - min_y).abs() < 1e-9 && (bbox.max.y - max_y).abs() < 1e-9);
    assert!(max_x - min_x > 1.0, "the box has the length of the path");

    // Claim 2: the lost pick of the Drill.
    select(&mut state, 1);
    let keys = one_frame_request(&click(&ctx, &mut state, "Show me"));
    assert_eq!(keys.len(), 1);
    assert!(
        keys[0].names_same_geometry(&lost),
        "the lost pick frames its stored key: {keys:?}"
    );

    // Claim 3, lost arm: the anchor grown by half the circle diameter, in
    // the display frame.
    let drill = state.session.toolpath_configs()[1].id;
    let bbox = selection_frame_bounds(&state.session, drill, &keys).expect("a box for a lost pick");
    let frame = rs_cam_viz::interaction::picking::toolpath_setup_frame(&state.session, drill);
    let half = (lost.measure / 2.0).max(rs_cam_viz::interaction::picking::POINT_MARK_RADIUS_MM);
    let corner = |dx: f64, dy: f64| {
        rs_cam_viz::interaction::picking::item_display_xy(
            frame.as_ref(),
            state.session.stock_config(),
            rs_cam_core::geo::P2::new(lost.anchor[0] + dx, lost.anchor[1] + dy),
        )
    };
    let a = corner(-half, -half);
    let b = corner(half, half);
    assert!(
        (bbox.min.x - a.x.min(b.x)).abs() < 1e-9 && (bbox.max.x - a.x.max(b.x)).abs() < 1e-9,
        "{bbox:?}"
    );
    assert!(
        (bbox.min.y - a.y.min(b.y)).abs() < 1e-9 && (bbox.max.y - a.y.max(b.y)).abs() < 1e-9,
        "{bbox:?}"
    );
    assert!(
        (bbox.max.x - bbox.min.x - 2.0 * half).abs() < 1e-9,
        "the box is one key measure wide"
    );
}

// ── P3.8: "Show me" on an at-the-top note frames the face of the note ──

/// A flat up face of the synthetic part: the square `[x0, x0 + 20]` by
/// `[-10, 10]` at `z`.
fn flat(x0: f64, z: f64) -> rs_cam_core::geometry::enriched_mesh::FaceTessellation {
    use rs_cam_core::geo::{P3, V3};
    use rs_cam_core::geometry::enriched_mesh::{FaceTessellation, SurfaceParams, SurfaceType};
    let v = vec![
        P3::new(x0, -10.0, z),
        P3::new(x0 + 20.0, -10.0, z),
        P3::new(x0 + 20.0, 10.0, z),
        P3::new(x0, 10.0, z),
    ];
    FaceTessellation {
        boundary_loops: vec![v.clone()],
        vertices: v,
        triangles: vec![[0, 1, 2], [0, 2, 3]],
        surface_type: SurfaceType::Plane,
        surface_params: SurfaceParams::Plane {
            normal: V3::new(0.0, 0.0, 1.0),
            d: z,
        },
    }
}

/// A Pocket under "each picked face" (depth 0) on a synthetic STEP part
/// with three up faces: two at the stock top (Z 0) and one at Z -4. The
/// two faces at the top give two notes of one kind and one layer (a BREP
/// face has no layer). Returns the state and the key of each face.
fn at_the_top_state() -> (AppState, Vec<MatchKey>) {
    use rs_cam_core::compute::floor::Floor;
    use rs_cam_core::compute::operation_configs::PocketConfig;
    use rs_cam_core::selection::SourceRef;
    use std::sync::Arc;

    let enriched = rs_cam_core::geometry::enriched_mesh::build_enriched_mesh(
        vec![flat(-40.0, 0.0), flat(-15.0, 0.0), flat(10.0, -4.0)],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let model = rs_cam_core::session::LoadedModel {
        id: 0,
        path: std::path::PathBuf::from("synthetic://at_the_top.step"),
        name: "At the top".to_owned(),
        kind: None,
        mesh: Some(Arc::clone(&enriched.mesh)),
        items: Arc::new(rs_cam_core::io::brep_face_items(&enriched)),
        enriched_mesh: Some(Arc::new(enriched)),
        units: None,
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    };
    let keys: Vec<MatchKey> = model
        .items
        .iter()
        .map(|item| MatchKey {
            source: SourceRef::ToolpathModel,
            ..item.key().clone()
        })
        .collect();
    assert_eq!(keys.len(), 3, "the part holds three faces");
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 3.0;
    let mut pocket = toolpath("Pocket", OperationType::Pocket);
    pocket.operation = OperationConfig::Pocket(PocketConfig {
        floor: Floor::EachFace { depth: 0.0 },
        selection: Selection::empty(),
        ..PocketConfig::default()
    });
    let mut builder = ProjectSessionBuilder::new()
        .stock(StockConfig {
            x: 80.0,
            y: 40.0,
            z: 15.0,
            origin_x: -40.0,
            origin_y: -20.0,
            origin_z: -15.0,
            auto_from_model: false,
            ..StockConfig::default()
        })
        .tool(tool)
        .model(model);
    builder.add_toolpath(0, pocket).unwrap();
    let mut state = AppState::new();
    state.session = builder.build();
    for key in &keys {
        let _ = state
            .session
            .apply(Command::EditSelection(EditSelectionArgs {
                index: 0,
                target: SelectionTarget::Op,
                edit: SelectionEdit::AddPick(key.clone()),
            }))
            .unwrap();
    }
    for config in state.session.toolpath_configs() {
        state
            .gui
            .toolpath_rt
            .insert(config.id, ToolpathRuntime::new(true));
    }
    (state, keys)
}

/// Geometry selection P3.8 (the P3.7 gap): under "each picked face" a
/// face at the top gives its own note, which names that face. "Show me" on
/// the note frames that face only, not every skipped face of its kind and
/// layer.
///
/// Red proof: `note_item_keys` with the kind-and-layer filter for an
/// at-the-top note too. The first note row then frames both faces at the
/// top, and the one-key assertion fails.
#[test]
fn show_me_frames_only_the_face_of_an_at_the_top_note_p3_8() {
    let ctx = ctx();
    let (mut state, _keys) = at_the_top_state();
    let tc = state.session.toolpath_configs()[0].clone();
    let report = state
        .session
        .selection_status(&tc, SelectionTarget::Op)
        .expect("the selection resolves");
    assert!(report.refusal.is_none(), "{:?}", report.refusal);
    assert_eq!(report.notes.len(), 2, "two faces are at the top");
    let note_faces: Vec<&MatchKey> = report.notes.iter().map(|n| &n.first.key).collect();
    assert!(
        !note_faces[0].names_same_geometry(note_faces[1]),
        "the two notes name two faces"
    );

    // The keys of each note, as the panel builds them: one face per note.
    let model = state.session.models().first();
    let per_note = rs_cam_viz::state::selection_highlight::note_item_keys(model, &report);
    for (note, frame) in report.notes.iter().zip(&per_note) {
        assert_eq!(frame.len(), 1, "a note frames its one face: {frame:?}");
        assert!(frame[0].names_same_geometry(&note.first.key), "{frame:?}");
    }

    // The production route: a click on the first note row sends one
    // frame request with the face of that note.
    select(&mut state, 0);
    let row = format!("1 face is not cut {}", tokens::GLYPH_DETAIL);
    let framed = one_frame_request(&click(&ctx, &mut state, &row));
    assert_eq!(framed.len(), 1, "the note frames one face: {framed:?}");
    assert!(
        framed[0].names_same_geometry(&report.notes[0].first.key),
        "{framed:?}"
    );
}
