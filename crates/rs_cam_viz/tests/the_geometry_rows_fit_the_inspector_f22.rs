//! FR-2 F22, F23, F25, F27: the Geometry tab fits the inspector column.
//!
//! # The defect
//!
//! Seen on screen (FR-2, 2026-10-23, 1400 x 900). A long Geometry-tab row
//! ("Ø 6.00 at (15.0, 15.0) Z 20.0... Rule... ×"), the floor face slot or
//! a footer line made the inspector grow past the window edge, and the
//! tab strip then cut "Linking".
//!
//! # The claim
//!
//! The real inspector (`properties::draw`) in the real side panel wrapper
//! (`app::side_panel`, default width 340 since R4) keeps its width, and no painted
//! text and no widget goes past the right edge of the panel content. The
//! cases are the long rows of the FR-2 screenshots:
//!
//! 1. a STEP Drill (`block_with_blind_hole.step`) on its default rule
//!    (R-B: one hole row), and the same Drill with a pick of the floor
//!    circle: the refusal (F27; W3a: the Danger state of the Cuts row,
//!    the text on its hover);
//! 2. the same Drill with a pick of the rim circle at Z 20: a row with
//!    "Rule…" and "×";
//! 3. a DXF Drill with circles of three sizes, a point and open paths on
//!    six layers, on its default rule and with picks;
//! 4. a STEP Pocket (`stepped_block.step`) on two faces with one face as
//!    the floor: the Floor face row (F23 names it "Face Z…"), and the
//!    Floor row on "Each picked face" with a Z range (F25).
//!
//! 5. a STEP Pocket whose result the model gouge guard blocked (F20,
//!    FR-2 screenshot 04): the header row, the act buttons and the Safety
//!    footer line.
//!
//! The cause (found by this test): the header row beside Generate ("Done
//! · edited since — regenerate", "old: 12345 moves") did not truncate.
//! It grew the content, and every row below took the wider width.
//!
//! Red proof (2026-10-23): `toolpath_panel.rs` at HEAD 89e87887 gives
//! "STEP pocket blocked: the content used 1477.0, the panel gives
//! 1384.0", and "Linking", "Use as floor", "×" and the Safety line paint
//! past the edge. The F27 arm fails at HEAD on the header text.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test the_geometry_rows_fit_the_inspector_f22
//! ```

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod inspector_harness_p6_7;

use std::path::PathBuf;

use inspector_harness_p6_7 as h;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::{Item, ItemKind, ItemSet, MatchKey, SelectionEdit, Shape, SourceRef};
use rs_cam_core::session::{
    Command, EditSelectionArgs, ProjectSession, ProjectSessionBuilder, SelectionTarget,
    SetToolpathFloorArgs,
};
use rs_cam_viz::app::{SidePanelEdge, side_panel};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection as UiSelection;
use rs_cam_viz::ui::properties;

/// The window of the FR-2 screenshots.
const SCREEN: egui::Vec2 = egui::vec2(1400.0, 900.0);
/// The default width of the inspector (`app.rs`; R4, operator ruling
/// 2026-10-30: 340).
const PANEL_WIDTH: f32 = 340.0;

/// What one frame of the inspector measured.
#[derive(Debug, Default)]
struct Frame {
    /// The rect that the panel gives its content.
    content: Option<egui::Rect>,
    /// The rect that the content used.
    used: Option<egui::Rect>,
    /// Each painted text, with its right edge.
    texts: Vec<(String, f32)>,
    /// The painted texts that egui elided ("…").
    elided: Vec<String>,
}

/// One frame of the real inspector in the real side panel.
fn frame(ctx: &egui::Context, state: &mut AppState) -> Frame {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
        ..Default::default()
    };
    let mut measured = Frame::default();
    let mut events = Vec::new();
    let mut out = ctx.run_ui(input, |ui| {
        side_panel(
            ui,
            SidePanelEdge::Right,
            "toolpath_properties",
            PANEL_WIDTH,
            |ui| {
                measured.content = Some(ui.max_rect());
                properties::draw(ui, state, &mut events);
                measured.used = Some(ui.min_rect());
            },
        );
    });
    for clipped in &out.shapes {
        if let egui::epaint::Shape::Text(text) = &clipped.shape
            && !text.galley.job.text.trim().is_empty()
        {
            measured.texts.push((
                text.galley.job.text.clone(),
                clipped.shape.visual_bounding_rect().max.x,
            ));
            if text.galley.elided {
                measured.elided.push(text.galley.job.text.clone());
            }
        }
    }
    out.textures_delta.clear();
    measured
}

/// Five frames (the panel stores its width for the next frame); the last.
fn settle(state: &mut AppState) -> Frame {
    let ctx = h::ctx();
    let mut last = Frame::default();
    for _ in 0..5 {
        last = frame(&ctx, state);
    }
    last
}

/// The breaches of one settled frame: the panel width, the used rect and
/// each text past the content right edge.
fn breaches(case: &str, f: &Frame) -> Vec<String> {
    let content = f.content.expect("the panel draws its content");
    let used = f.used.expect("the panel draws its content");
    let mut out = Vec::new();
    let panel = SCREEN.x - content.min.x;
    if panel > PANEL_WIDTH + 1.0 {
        out.push(format!("{case}: the panel grew to {panel:.1}"));
    }
    if used.max.x > content.max.x + 0.5 {
        out.push(format!(
            "{case}: the content used {:.1}, the panel gives {:.1}",
            used.max.x, content.max.x
        ));
    }
    for (text, right) in &f.texts {
        if *right > content.max.x + 1.0 {
            out.push(format!(
                "{case}: '{text}' ends at {right:.1}, past {:.1}",
                content.max.x
            ));
        }
    }
    out
}

/// A project of `session`, on the Geometry tab of toolpath 0.
fn state_of(session: ProjectSession) -> AppState {
    let mut state = AppState::new();
    state.replace_session(session);
    let parts = state.split_mut();
    for config in parts.session.toolpath_configs() {
        parts
            .gui
            .toolpath_rt
            .insert(config.id, ToolpathRuntime::new(true));
    }
    let id = state.session().toolpath_configs()[0].id;
    let _ = state.set_focus(UiSelection::Toolpath(id));
    state.gui.pending_toolpath_tab = Some((id, properties::ToolpathTab::Geometry));
    state
}

fn tool(diameter: f64) -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId::from_raw(h::TOOL), ToolType::EndMill);
    tool.diameter = diameter;
    tool
}

fn step_session(path: &str, op: OperationType, stock: StockConfig) -> ProjectSession {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path);
    let model = rs_cam_core::io::load_model_file(
        &path,
        rs_cam_core::ids::ModelId::from_raw(h::MODEL),
        ModelKind::Step,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .expect("the STEP fixture loads");
    let mut builder = ProjectSessionBuilder::new()
        .stock(stock)
        .tool(tool(6.0))
        .model(model);
    builder
        .add_toolpath(0, h::toolpath("F22", OperationConfig::new_default(op)))
        .expect("add the toolpath");
    builder.build()
}

fn edit(session: &mut ProjectSession, edit: SelectionEdit) {
    let _ = session
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit,
        }))
        .expect("the edit applies");
}

/// The keys of the items of model 0 that `keep` accepts.
fn keys(session: &ProjectSession, keep: impl Fn(&Item) -> bool) -> Vec<MatchKey> {
    session.models()[0]
        .items
        .iter()
        .filter(|item| keep(item))
        .map(|item| item.key().clone())
        .collect()
}

/// The block 40 x 30 x 20 with a Ø 6 blind hole at (15, 15); the stock is
/// the block.
fn blind_hole_drill() -> ProjectSession {
    let stock = StockConfig {
        auto_from_model: true,
        // S11: the stock default before S11 (the raw builder fits no auto
        // stock).
        z: Some(25.0),
        ..StockConfig::default()
    };
    step_session(
        "../rs_cam_core/tests/fixtures/step/block_with_blind_hole.step",
        OperationType::Drill,
        stock,
    )
}

/// R-B: the STEP Drill with a pick BY KEY of the floor circle of the hole
/// (Z 12): it refuses (`HoleFloorNotRim`). Before R-B the default STEP
/// Drill refused; now it cuts the rim, so this fixture gives the refusal.
fn refused_hole_drill() -> ProjectSession {
    let mut session = blind_hole_drill();
    let floor = keys(&session, |item| {
        item.kind() == ItemKind::Circle && item.z().is_some_and(|z| (z - 12.0).abs() < 1e-6)
    });
    assert_eq!(floor.len(), 1, "fixture: one floor circle");
    let mut key = floor[0].clone();
    key.source = SourceRef::ToolpathModel;
    edit(&mut session, SelectionEdit::AddPick(key));
    session
}

/// Circles Ø 6 (the tool), Ø 8 and Ø 10, a point, and one open path on
/// each of six layers with long names.
fn dxf_drill() -> ProjectSession {
    let item = |layer: &str, shape: Shape| {
        Item::new(SourceRef::ToolpathModel, Some(layer.into()), shape).expect("an item")
    };
    let mut items = vec![
        item("HOLES_THROUGH", Shape::circle(P2::new(15.0, 15.0), 6.0)),
        item("HOLES_THROUGH", Shape::circle(P2::new(45.0, 15.0), 8.0)),
        item(
            "HOLES_COUNTERBORE",
            Shape::circle(P2::new(75.0, 15.0), 10.0),
        ),
        item("CENTRE_MARKS", Shape::Point(P2::new(105.0, 15.0))),
    ];
    for (i, layer) in [
        "ENGRAVING_TEXT_FRONT",
        "ENGRAVING_TEXT_BACK",
        "DIMENSIONS_AND_NOTES",
        "CONSTRUCTION_LINES",
        "OUTLINE_REFERENCE",
        "TITLE_BLOCK_BORDER",
    ]
    .iter()
    .enumerate()
    {
        let y = 40.0 + 5.0 * i as f64;
        let mut path = Polygon2::new(vec![
            P2::new(0.0, y),
            P2::new(60.0, y),
            P2::new(90.0, y + 2.0),
        ]);
        path.closed = false;
        items.push(item(layer, Shape::Path(path)));
    }
    let stock = StockConfig {
        x: 160.0,
        y: 100.0,
        z: Some(18.0),
        origin_x: -20.0,
        origin_y: -10.0,
        origin_z: -18.0,
        auto_from_model: false,
        ..StockConfig::default()
    };
    let mut builder = ProjectSessionBuilder::new()
        .stock(stock)
        .tool(tool(6.0))
        .model(h::drawing_model(ItemSet::new(items)));
    builder
        .add_toolpath(
            0,
            h::toolpath("F22", OperationConfig::new_default(OperationType::Drill)),
        )
        .expect("add the toolpath");
    builder.build()
}

/// The stepped block Pocket on its two lowest up faces, with the lower as
/// the floor ("A picked face"), or both faces under "Each picked face".
fn stepped_pocket(floor_face: bool) -> ProjectSession {
    let mut session = step_session(
        "../../fixtures/gui_step/stepped_block.step",
        OperationType::Pocket,
        StockConfig {
            auto_from_model: true,
            // S11: the stock default before S11 (the raw builder fits no
            // auto stock).
            z: Some(25.0),
            ..StockConfig::default()
        },
    );
    let mut faces: Vec<(f64, MatchKey)> = session.models()[0]
        .items
        .iter()
        .filter_map(|item| match item.shape() {
            Shape::Face(face) if face.normal.z > 0.99 => Some((face.plane_z, item.key().clone())),
            _ => None,
        })
        .collect();
    faces.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert!(faces.len() >= 3, "fixture: three up faces: {faces:?}");
    for (_, key) in faces.iter().take(3) {
        edit(&mut session, SelectionEdit::AddPick(key.clone()));
    }
    if floor_face {
        let _ = session
            .apply(Command::SetToolpathFloor(SetToolpathFloorArgs {
                index: 0,
                floor: Floor::Face {
                    key: faces[0].1.clone(),
                    depth: 0.0,
                },
            }))
            .expect("the face floor applies");
    }
    session
}

/// Give toolpath 0 a current result that the model gouge guard blocked
/// (`safety.cuts_into_model`), as FR-2 screenshot 04 shows.
fn block(state: &mut AppState) {
    use rs_cam_core::compute::catalog::ModelGougeRule;
    use rs_cam_core::compute::model_gouge::ModelGougeFinding;
    use rs_cam_core::compute::toolpath_stats::ToolpathStats;
    use rs_cam_viz::state::runtime::ToolpathRun;
    use rs_cam_viz::state::toolpath::ToolpathResult;
    let id = state.session().toolpath_configs()[0].id;
    let stats = ToolpathStats {
        move_count: 12_345,
        model_gouge: Some(ModelGougeFinding {
            rule: ModelGougeRule::Surface {
                fix: "Set the floor to Each picked face",
            },
            judged: 12_184,
            gouging: 4_827,
            deepest_mm: 10.0,
            at: [38.9, -2.8, -2.0],
            surface_z: 8.0,
            move_index: 12,
            depth_top_z: None,
            below_surface_mm: 10.0,
            at_wall: false,
        }),
        ..ToolpathStats::default()
    };
    let result = ToolpathResult {
        annotated: std::sync::Arc::new(rs_cam_core::trace::toolpath_spans::AnnotatedToolpath::new(
            rs_cam_core::toolpath::Toolpath::new(),
        )),
        stats,
        debug_trace: None,
        semantic_trace: None,
        debug_trace_path: None,
        drill_op: None,
    };
    state
        .gui
        .toolpath_rt
        .get_mut(&id)
        .expect("the runtime row")
        .run = ToolpathRun::Done(result);
}

#[test]
fn the_geometry_rows_fit_the_inspector_f22() {
    let mut found = Vec::new();

    // 1. The STEP Drill on its default rule (R-B: the hole row
    //    "Ø6 hole, 8 deep"), and a refused STEP Drill.
    let mut state = state_of(blind_hole_drill());
    found.extend(breaches("STEP drill default", &settle(&mut state)));
    let mut state = state_of(refused_hole_drill());
    found.extend(breaches("STEP drill refused", &settle(&mut state)));

    // 2. The STEP Drill with the rim circle at the top picked.
    let mut session = blind_hole_drill();
    let mut rims: Vec<(f64, MatchKey)> = session.models()[0]
        .items
        .iter()
        .filter(|item| item.kind() == ItemKind::Circle)
        .map(|item| (item.z().unwrap_or(f64::NAN), item.key().clone()))
        .collect();
    rims.sort_by(|a, b| b.0.total_cmp(&a.0));
    let rim = rims.first().expect("fixture: a rim circle").1.clone();
    edit(&mut session, SelectionEdit::AddPick(rim));
    let mut state = state_of(session);
    let f = settle(&mut state);
    assert!(
        f.texts.iter().any(|(t, _)| t == "Rule\u{2026}"),
        "the picked row has its acts: {:?}",
        f.texts
    );
    found.extend(breaches("STEP drill pick", &f));

    // 3. The DXF Drill, default and with picks of each kind.
    let mut state = state_of(dxf_drill());
    found.extend(breaches("DXF drill default", &settle(&mut state)));
    let mut session = dxf_drill();
    for key in keys(&session, |item| {
        matches!(item.kind(), ItemKind::Circle | ItemKind::Point)
    }) {
        edit(&mut session, SelectionEdit::AddPick(key));
    }
    let mut state = state_of(session);
    found.extend(breaches("DXF drill picks", &settle(&mut state)));

    // 5. A Pocket whose result the model gouge guard blocked (F20).
    let mut state = state_of(stepped_pocket(false));
    block(&mut state);
    let f = settle(&mut state);
    found.extend(breaches("STEP pocket blocked", &f));

    // 4. The STEP Pocket with a floor face.
    let mut state = state_of(stepped_pocket(true));
    let f = settle(&mut state);
    assert!(
        f.texts.iter().any(|(t, _)| t.contains("Floor face")),
        "the floor face slot paints: {:?}",
        f.texts
    );
    found.extend(breaches("STEP pocket floor face", &f));

    assert!(
        found.is_empty(),
        "rows past the inspector edge:\n{}",
        found.join("\n")
    );
}

/// F23: the Floor face row names the face as the list rows do: "Face Z"
/// and its plane Z, not the anchor and the size. W3a: the row is a
/// `PropRow`, so the label and the value are two painted texts.
#[test]
fn the_floor_face_slot_names_the_face_z_f23() {
    let mut state = state_of(stepped_pocket(true));
    let f = settle(&mut state);
    let at: Vec<usize> = f
        .texts
        .iter()
        .enumerate()
        .filter(|(_, (t, _))| t == "Floor face")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(at.len(), 1, "one Floor face row: {:?}", f.texts);
    let value = &f.texts[at[0] + 1].0;
    assert!(
        value.starts_with("Face Z"),
        "the row names the face Z: {value}"
    );
    assert!(!value.contains(" at ("), "no anchor on the face: {value}");
}

/// F25: under "Each picked face" the Floor row gives the Z range of the
/// face floors ("Z a to b"), not one Z. W3a: a read-only `PropRow`: the
/// label "Floor", the value "Each picked face", the range as the unit.
#[test]
fn each_picked_face_shows_the_floor_range_f25() {
    let mut state = state_of(stepped_pocket(false));
    let f = settle(&mut state);
    let at: Vec<usize> = f
        .texts
        .iter()
        .enumerate()
        .filter(|(_, (t, _))| t == "Floor")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(at.len(), 1, "one Floor row: {:?}", f.texts);
    let row: Vec<&String> = f.texts[at[0]..].iter().take(3).map(|(t, _)| t).collect();
    assert!(
        row.iter().any(|t| *t == "Each picked face"),
        "the Floor row names its reference: {row:?}"
    );
    assert!(
        row.iter()
            .any(|t| t.starts_with("Z ") && t.contains(" to ")),
        "the Floor row gives a Z range: {row:?}"
    );
}

/// F27 (IC1), W2b, W3a: a refusal of the selection is the Danger state of
/// the Cuts row (field key `selection`, drawn for a Pocket: `placement::drawn_fields`); its text is
/// the row hover and paints nowhere. The status line reads "Not ready",
/// and the issue stack holds no row for it: the placement gives the core
/// finding to the Cuts row.
#[test]
fn a_refusal_paints_once_f27() {
    let session = refused_hole_drill();
    let report = session
        .selection_status(&session.toolpath_configs()[0], SelectionTarget::Op)
        .expect("the status answers");
    let refusal = report
        .refusal
        .as_ref()
        .expect("fixture: the STEP Drill on the floor circle refuses")
        .to_string();
    let mut state = state_of(session);
    let f = settle(&mut state);
    let head: String = refusal.chars().take(40).collect();
    let full: Vec<&String> = f
        .texts
        .iter()
        .map(|(t, _)| t)
        .filter(|t| t.contains(&head))
        .collect();
    assert!(full.is_empty(), "the refusal is the row hover: {full:#?}");
    assert!(
        f.texts.iter().any(|(t, _)| t == "Cuts"),
        "the Cuts row paints: {:?}",
        f.texts
    );
    assert!(
        f.texts.iter().any(|(t, _)| t == "Not ready"),
        "the status line reads one word: {:?}",
        f.texts
    );
    let faces = [
        rs_cam_core::diagnostics::presentation::face(
            rs_cam_core::diagnostics::ids::PRECOND_DRILL_NO_HOLES,
        ),
        rs_cam_core::diagnostics::presentation::face(
            rs_cam_core::diagnostics::ids::PRECOND_SELECTION_REFUSED,
        ),
    ];
    let issue: Vec<&String> = f
        .texts
        .iter()
        .map(|(t, _)| t)
        .filter(|t| faces.iter().any(|face| face == *t))
        .collect();
    assert!(issue.is_empty(), "no issue row: {:?}", f.texts);
    // Non-vacuity: the placement gives the selection finding to the field
    // row, not to nothing.
    let id = state.session().toolpath_configs()[0].id;
    let placed =
        rs_cam_viz::ui::properties::placed_findings(id, &state).expect("the toolpath is placed");
    assert!(
        placed.by_field.iter().any(|d| faces.contains(&d.face)),
        "the selection finding sits on the Cuts row: {:?}",
        placed.by_field
    );
}

/// F32, F28: on the default rule of a DXF Drill, the six open paths (a
/// kind that a Drill never cuts) are ONE line, not one line per layer. On
/// the default rule there is no "Selected" row (GUI/MCP parity: the GUI
/// shows no count that MCP `selection_status` does not give). The default
/// list holds only the kinds that the Drill cuts. The per-layer list is in the Skipped items list,
/// which names what it holds (F28). A size that the Drill can cut keeps its
/// own note.
///
/// Red proof (2026-10-23): `selection_section.rs` at HEAD 89e87887 paints
/// six lines "1 open path on layer … is not cut" and no one line; this
/// test fails on its first assertion. With `geometry_tab.rs` at HEAD the
/// F23 and F25 tests fail too ("Face at (50.0, 20.0), 28.3 mm across";
/// "Each picked face · Z 10.00").
#[test]
fn a_kind_the_op_never_cuts_is_one_line_f32() {
    let mut state = state_of(dxf_drill());
    let f = settle(&mut state);
    let texts: Vec<&String> = f.texts.iter().map(|(t, _)| t).collect();
    let one: Vec<&&String> = texts
        .iter()
        .filter(|t| t.starts_with("Drill cuts circles and points; 6 other items are not cut"))
        .collect();
    assert_eq!(one.len(), 1, "one line for the open paths: {texts:#?}");
    assert!(
        !texts.iter().any(|t| t.contains("open path on layer")),
        "no line per layer: {texts:#?}"
    );
    assert!(
        texts
            .iter()
            .any(|t| t.starts_with("1 circle on layer HOLES_COUNTERBORE")),
        "a circle that does not fit keeps its note: {texts:#?}"
    );
    assert!(
        !texts.iter().any(|t| t.starts_with("Selected")),
        "no Selected row on the default rule: {texts:#?}"
    );
    // The default list holds the 3 circles and the point, not the paths.
    // W3g: the default rule is the Source row; the Cuts row gives the
    // count of the kinds that the Drill cuts.
    assert!(
        texts.iter().any(|t| *t == "Every point, circle")
            && texts.iter().any(|t| *t == "1 point \u{00B7} 3 circles"),
        "the default list holds the kinds that the Drill cuts: {texts:#?}"
    );
    assert!(
        texts
            .iter()
            .any(|t| t.starts_with("Skipped items \u{00B7} ")),
        "the derivation names what it holds: {texts:#?}"
    );
}

/// GUI/MCP parity: with picks, "Selected" is the held count, the same
/// number as MCP `selection_status` (`selected`).
#[test]
fn selected_is_the_held_count_with_picks() {
    let mut session = dxf_drill();
    for key in keys(&session, |item| {
        matches!(item.kind(), ItemKind::Circle | ItemKind::Point)
    }) {
        edit(&mut session, SelectionEdit::AddPick(key));
    }
    let held = session
        .selection_status(&session.toolpath_configs()[0], SelectionTarget::Op)
        .expect("the status answers")
        .selected
        .len();
    let mut state = state_of(session);
    let f = settle(&mut state);
    let want = format!("{held} items");
    let at = f
        .texts
        .iter()
        .position(|(t, _)| t.starts_with("Selected"))
        .expect("a Selected row with picks");
    assert_eq!(f.texts[at + 1].0, want, "{:?}", f.texts);
}
