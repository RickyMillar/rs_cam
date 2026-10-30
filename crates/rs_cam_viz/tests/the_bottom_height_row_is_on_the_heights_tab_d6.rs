//! Geometry selection D6 (REFERENCE_AUDIT row D6; Fusion "Bottom height",
//! FreeCAD "Final Depth"): the floor of an op is the Bottom height row of
//! the Heights tab, directly under the Top height row.
//!
//! # The claims
//!
//! 1. For each `OpFloor` op (Pocket, Profile, Zigzag, Adaptive 2D, Drill,
//!    Bore) the Heights tab draws "Bottom height:" with the reference of
//!    the floor, and no row label stands between "Top height:" and
//!    "Bottom height:". The Passes tab draws no floor row ("Bottom height:"
//!    and the old "Floor from:" are both absent). The read-only Floor line
//!    of the Heights tab ("Floor: ...") is gone (one owner, IC1).
//! 2. GUI/MCP parity: a change of reference in the Bottom height row (two
//!    clicks in the production inspector: the combo, then "Stock bottom")
//!    writes the floor that MCP `set_toolpath_floor` writes for the same
//!    floor Z, and the two sessions hold one op config.
//!
//! Every arm drives the production `properties::draw` through a headless
//! `Context`, which flushes the draft through `ReplaceToolpathConfig`.
//!
//! Red proof (2026-10-23): with `draw_heights_params` not drawing the
//! Bottom height row of an `OpFloor` op (the Heights tab before D6), claim 1
//! fails on its first assertion (the Heights tab has no "Bottom height:"),
//! and claim 2 fails: it finds no Bottom height row to click.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test the_bottom_height_row_is_on_the_heights_tab_d6
//! ```

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::operation_configs::{
    AdaptiveConfig, BoreConfig, DrillConfig, PocketConfig, ProfileConfig, ZigzagConfig,
};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::{P3, V3};
use rs_cam_core::geometry::enriched_mesh::{
    FaceTessellation, SurfaceParams, SurfaceType, build_enriched_mesh,
};
use rs_cam_core::io::brep_face_items;
use rs_cam_core::selection::{MatchKey, Selection, SelectionEdit, Shape, SourceRef};
use rs_cam_core::session::{
    Command, EditSelectionArgs, LoadedModel, ProjectSession, ProjectSessionBuilder,
    SelectionTarget, SetToolpathFloorArgs, ToolpathConfig,
};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection as GuiSelection;
use rs_cam_viz::ui::{properties, tokens};

/// The inspector's own width in the Toolpaths workspace (`app.rs`).
const PANEL_WIDTH: f32 = 280.0;
/// The plane Z of the picked face (mm).
const FACE_Z: f64 = -4.0;
/// The stock bottom (mm): `origin_z` of the stock.
const STOCK_BOTTOM_Z: f64 = -15.0;
/// The start floor: 3 mm below the Top height (the face top, UX-1).
const START_DEPTH: f64 = 3.0;

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

/// A flat up face: the square `[x0, x0 + 20]` by `[-10, 10]` at `z`.
fn flat(x0: f64, z: f64) -> FaceTessellation {
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

/// A synthetic STEP part: a face at Z 0 (the model top) and the face to
/// pick at [`FACE_Z`].
fn model() -> LoadedModel {
    let enriched = build_enriched_mesh(
        vec![flat(-20.0, 0.0), flat(0.0, FACE_Z)],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(1),
        path: PathBuf::from("synthetic://d6_fixture.step"),
        name: "D6 fixture".to_owned(),
        kind: None,
        mesh: Some(Arc::clone(&enriched.mesh)),
        items: Arc::new(brep_face_items(&enriched)),
        enriched_mesh: Some(Arc::new(enriched)),
        units: None,
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    }
}

fn face_key() -> MatchKey {
    let model = model();
    let item = model
        .items
        .iter()
        .find(|item| matches!(item.shape(), Shape::Face(face) if (face.plane_z - FACE_Z).abs() < 1e-9))
        .expect("the part holds the face to pick")
        .clone();
    MatchKey {
        source: SourceRef::ToolpathModel,
        ..item.key().clone()
    }
}

fn toolpath(operation: OperationConfig) -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        feeds_lock: Default::default(),
        id: rs_cam_core::ToolpathId::from_raw(0),
        name: "Floor op".to_owned(),
        enabled: true,
        operation,
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: Some(rs_cam_core::ids::ToolId::from_raw(1)),
        model_id: rs_cam_core::ids::ModelId::from_raw(1),
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

/// Every `OpFloor` op, each with the floor 3 mm below the Top height.
fn floor_ops() -> Vec<OperationConfig> {
    let floor = Floor::Top { depth: START_DEPTH };
    vec![
        OperationConfig::Pocket(PocketConfig {
            floor: floor.clone(),
            selection: Selection::empty(),
            ..PocketConfig::default()
        }),
        OperationConfig::Profile(ProfileConfig {
            floor: floor.clone(),
            selection: Selection::empty(),
            ..ProfileConfig::default()
        }),
        OperationConfig::Zigzag(ZigzagConfig {
            floor: floor.clone(),
            selection: Selection::empty(),
            ..ZigzagConfig::default()
        }),
        OperationConfig::Adaptive(AdaptiveConfig {
            floor: floor.clone(),
            selection: Selection::empty(),
            ..AdaptiveConfig::default()
        }),
        OperationConfig::Drill(DrillConfig {
            floor: floor.clone(),
            ..DrillConfig::default()
        }),
        OperationConfig::Bore(BoreConfig {
            floor,
            ..BoreConfig::default()
        }),
    ]
}

/// The session of one floor op. An area op picks the face at [`FACE_Z`];
/// the floor is written back after the pick (ruling Q2 sets "Each picked
/// face" on the first face pick).
fn session(operation: OperationConfig) -> ProjectSession {
    let mut tool = ToolConfig::new_default(ToolId::from_raw(1), ToolType::EndMill);
    tool.diameter = 3.0;
    let mut builder = ProjectSessionBuilder::new()
        .stock(StockConfig {
            x: 60.0,
            y: 60.0,
            z: Some(20.0),
            origin_x: -30.0,
            origin_y: -30.0,
            origin_z: STOCK_BOTTOM_Z,
            auto_from_model: false,
            ..StockConfig::default()
        })
        .tool(tool)
        .model(model());
    let floor = operation.floor();
    let picks_faces = !matches!(
        operation.capabilities().item_use,
        rs_cam_core::compute::catalog::ItemUse::Holes { .. }
    );
    builder.add_toolpath(0, toolpath(operation)).unwrap();
    let mut s = builder.build();
    if picks_faces {
        let _ = s
            .apply(Command::EditSelection(EditSelectionArgs {
                index: 0,
                target: SelectionTarget::Op,
                edit: SelectionEdit::AddPick(face_key()),
            }))
            .unwrap();
    }
    if let Some(floor) = floor {
        let _ = s
            .apply(Command::SetToolpathFloor(SetToolpathFloorArgs {
                index: 0,
                floor,
            }))
            .unwrap();
    }
    s
}

fn state(session: ProjectSession, tab: properties::ToolpathTab) -> AppState {
    let mut state = AppState::new();
    state.replace_session(session);
    let id = state.session().toolpath_configs()[0].id;
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    let _ = state.set_focus(GuiSelection::Toolpath(id));
    state.gui.pending_toolpath_tab = Some((id, tab));
    state
}

/// One painted text run and where it sits.
#[derive(Debug, Clone)]
struct Painted {
    text: String,
    rect: egui::Rect,
}

/// Draw the production inspector once, with `events`. Returns the text
/// runs it painted.
fn frame(ctx: &egui::Context, state: &mut AppState, events: Vec<egui::Event>) -> Vec<Painted> {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(PANEL_WIDTH, 4000.0),
        )),
        events,
        ..Default::default()
    };
    let mut app_events = Vec::new();
    let mut out = ctx.run_ui(input, |ui| {
        ui.set_max_width(PANEL_WIDTH);
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.set_max_width(PANEL_WIDTH);
            properties::draw(ui, state, &mut app_events);
        });
    });
    let painted = out
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::epaint::Shape::Text(text) => Some(Painted {
                text: text.galley.job.text.clone(),
                rect: text.visual_bounding_rect(),
            }),
            _ => None,
        })
        .collect();
    out.textures_delta.clear();
    painted
}

/// Three quiet frames, so the tab switch and the layout settle.
fn settle(ctx: &egui::Context, state: &mut AppState) -> Vec<Painted> {
    let mut painted = Vec::new();
    for _ in 0..3 {
        painted = frame(ctx, state, Vec::new());
    }
    painted
}

/// A press and a release at `at`, then two quiet frames.
fn click(ctx: &egui::Context, state: &mut AppState, at: egui::Pos2) -> Vec<Painted> {
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::default(),
    };
    let _ = frame(
        ctx,
        state,
        vec![egui::Event::PointerMoved(at), button(true)],
    );
    let _ = frame(ctx, state, vec![button(false)]);
    let _ = frame(ctx, state, Vec::new());
    frame(ctx, state, Vec::new())
}

fn find<'a>(painted: &'a [Painted], text: &str) -> Vec<&'a Painted> {
    painted.iter().filter(|p| p.text == text).collect()
}

/// Claim 1.
#[test]
fn the_bottom_height_row_is_on_the_heights_tab_d6() {
    let ctx = ctx();
    for operation in floor_ops() {
        let label = operation.label();
        let reference = operation.floor().unwrap().reference_label();

        let mut s = state(session(operation.clone()), properties::ToolpathTab::Heights);
        let heights = settle(&ctx, &mut s);
        let bottom = find(&heights, "Bottom height");
        assert_eq!(
            bottom.len(),
            1,
            "{label}: the Heights tab draws one Bottom height row: {:?}",
            heights.iter().map(|p| &p.text).collect::<Vec<_>>()
        );
        // W3d: "Top height" is the label of the Top row and the reference
        // of the floor in the Bottom height combo; the label is the one
        // in the label column, left of the Bottom height combo.
        let bottom_x = bottom[0].rect.left();
        let top: Vec<_> = find(&heights, "Top height")
            .into_iter()
            .filter(|p| (p.rect.left() - bottom_x).abs() < 4.0)
            .collect();
        assert_eq!(top.len(), 1, "{label}: one Top height row");
        let (top_y, bottom_y) = (top[0].rect.center().y, bottom[0].rect.center().y);
        assert!(
            bottom_y > top_y,
            "{label}: Bottom height ({bottom_y}) is not under Top height ({top_y})"
        );
        // W3d: the labels have no colon; a row label is one of the five.
        let between: Vec<&str> = heights
            .iter()
            .filter(|p| {
                [
                    "Clearance",
                    "Retract",
                    "Feed",
                    "Top height",
                    "Bottom height",
                ]
                .contains(&p.text.as_str())
                    && (p.rect.left() - bottom_x).abs() < 4.0
                    && p.rect.center().y > top_y + 1.0
                    && p.rect.center().y < bottom_y - 1.0
            })
            .map(|p| p.text.as_str())
            .collect();
        assert!(
            between.is_empty(),
            "{label}: rows between Top height and Bottom height: {between:?}"
        );
        // The row names the reference of the floor, on the row of its
        // label, and the resolved Z.
        assert!(
            heights
                .iter()
                .any(|p| p.text == reference && (p.rect.center().y - bottom_y).abs() < 4.0),
            "{label}: the Bottom height row does not name {reference:?}"
        );
        // W3d (F5): the row shows the resolved floor Z, absolute, as the
        // diagram does.
        let tc = &s.session().toolpath_configs()[0];
        let floor_z = s
            .session()
            .toolpath_heights(tc)
            .unwrap()
            .floor_z(&tc.heights)
            .unwrap()
            .unwrap();
        let shown = format!("{floor_z:.2}");
        assert!(
            heights
                .iter()
                .any(|p| p.text == shown && (p.rect.center().y - bottom_y).abs() < 4.0),
            "{label}: the Bottom height row does not show the resolved Z {shown}"
        );
        assert!(
            !heights.iter().any(|p| p.text.starts_with("Floor: ")),
            "{label}: the read-only Floor line is still drawn"
        );

        let mut s = state(session(operation), properties::ToolpathTab::Passes);
        let cut = settle(&ctx, &mut s);
        for gone in ["Bottom height", "Floor from:"] {
            assert!(
                find(&cut, gone).is_empty(),
                "{label}: the Passes tab still draws {gone:?}"
            );
        }
    }
}

/// Claim 2.
#[test]
fn a_bottom_height_edit_writes_the_set_toolpath_floor_floor_d6() {
    let ctx = ctx();
    let pocket = floor_ops().remove(0);
    let mut s = state(session(pocket.clone()), properties::ToolpathTab::Heights);
    let floor_z = |session: &ProjectSession| {
        let tc = &session.toolpath_configs()[0];
        session
            .toolpath_heights(tc)
            .unwrap()
            .floor_z(&tc.heights)
            .unwrap()
            .unwrap()
    };
    let before = floor_z(s.session());
    assert!(
        (before - (FACE_Z - START_DEPTH)).abs() < 1e-9,
        "fixture: the floor starts 3 mm under the face, got {before}"
    );

    let painted = settle(&ctx, &mut s);
    let label = find(&painted, "Bottom height")[0].rect;
    let combo = painted
        .iter()
        .find(|p| p.text == "Top height" && (p.rect.center().y - label.center().y).abs() < 4.0)
        .expect("the Bottom height combo names Top height")
        .rect;
    let open = click(&ctx, &mut s, combo.center());
    let entry = open
        .iter()
        .rev()
        .find(|p| p.text == "Stock bottom")
        .expect("the open combo lists Stock bottom")
        .rect;
    let _ = click(&ctx, &mut s, entry.center());
    let gui_floor = s.session().toolpath_configs()[0].operation.floor().unwrap();
    assert_eq!(
        gui_floor,
        Floor::StockBottom {
            depth: STOCK_BOTTOM_Z - before
        },
        "the row re-expresses the floor in the new reference (L-10)"
    );
    assert!(
        (floor_z(s.session()) - before).abs() < 1e-9,
        "the cut moved"
    );

    // MCP: the same floor through `set_toolpath_floor` on a twin session.
    let mut mcp = session(pocket);
    let _ = mcp
        .apply(Command::SetToolpathFloor(SetToolpathFloorArgs {
            index: 0,
            floor: Floor::StockBottom {
                depth: STOCK_BOTTOM_Z - before,
            },
        }))
        .unwrap();
    assert_eq!(
        serde_json::to_value(&mcp.toolpath_configs()[0].operation).unwrap(),
        serde_json::to_value(&s.session().toolpath_configs()[0].operation).unwrap(),
        "the GUI row and set_toolpath_floor store one op config"
    );
}
