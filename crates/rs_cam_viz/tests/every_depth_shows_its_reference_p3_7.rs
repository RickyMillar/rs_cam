//! Geometry selection P3.7: every depth shows its reference (design 8.1,
//! 8.2; rulings L-10, L-11, O-2).
//!
//! # The claims
//!
//! 1. Each op that holds a floor (Pocket, Profile, Zigzag, Adaptive 2D)
//!    draws the floor row on its Geometry tab: "Floor from:", the reference
//!    of its floor, and the resolved floor Z.
//! 2. Its Heights tab has no Bottom row (one owner, L-11). It draws one
//!    read-only Floor line with the reference and the resolved Z, and the
//!    Top line names its source ("auto: faces").
//! 3. O-2 (option A): with a face picked under a floor that is not "each
//!    picked face", one line offers that floor, and its button sets
//!    `Floor::EachFace { depth: 0 }` through the GUI write route.
//!
//! Every arm drives the production `properties::draw` through a headless
//! `Context` and reads the glyphs it painted.
//!
//! Red proofs:
//! - Claim 1: draw the combo of the floor row with an empty selected text
//!   (`.selected_text("")`): the reference is not painted, and the arm fails.
//! - Claim 3: drop the `set_floor` of the hint button
//!   (`draw_each_face_hint`): the floor stays `Top`, and the arm fails.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test every_depth_shows_its_reference_p3_7
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::floor::Floor;
use rs_cam_core::compute::operation_configs::{
    AdaptiveConfig, PocketConfig, ProfileConfig, ZigzagConfig,
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
    Command, EditSelectionArgs, LoadedModel, ProjectSessionBuilder, SelectionTarget, ToolpathConfig,
};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection as GuiSelection;
use rs_cam_viz::ui::{properties, tokens};

/// The inspector's own width in the Toolpaths workspace (`app.rs`).
const PANEL_WIDTH: f32 = 280.0;
/// The plane Z of the picked face (mm).
const FACE_Z: f64 = -4.0;

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut warmup = ctx.run_ui(egui::RawInput::default(), |_ui| {});
    warmup.textures_delta.clear();
    ctx
}

/// A flat up face of the synthetic part: the square `[x0, x0 + 20]` by
/// `[-10, 10]` at `z`.
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

/// A synthetic STEP part: a flat face at Z 0 (the model top) and the face
/// to pick at [`FACE_Z`]. Its faces are BREP face items.
fn model() -> LoadedModel {
    let enriched = build_enriched_mesh(
        vec![flat(-20.0, 0.0), flat(0.0, FACE_Z)],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    LoadedModel {
        id: 1,
        path: PathBuf::from("synthetic://floor_fixture.step"),
        name: "Floor fixture".to_owned(),
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

/// The pick key of the face at [`FACE_Z`].
fn face_key() -> MatchKey {
    let model = model();
    let item = model
        .items
        .iter()
        .find(|item| match item.shape() {
            Shape::Face(face) => (face.plane_z - FACE_Z).abs() < 1e-9,
            _ => false,
        })
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
        id: rs_cam_core::ToolpathId(0),
        name: "Floor op".to_owned(),
        enabled: true,
        operation,
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: 1,
        model_id: 1,
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

/// The four floor ops, each with another reference.
fn floor_ops() -> Vec<(OperationConfig, Floor)> {
    let empty = Selection::empty;
    vec![
        (
            OperationConfig::Pocket(PocketConfig {
                floor: Floor::StockBottom { depth: 0.5 },
                selection: empty(),
                ..PocketConfig::default()
            }),
            Floor::StockBottom { depth: 0.5 },
        ),
        (
            OperationConfig::Profile(ProfileConfig {
                floor: Floor::Z { z: -9.0 },
                selection: empty(),
                ..ProfileConfig::default()
            }),
            Floor::Z { z: -9.0 },
        ),
        (
            OperationConfig::Zigzag(ZigzagConfig {
                floor: Floor::StockTop { depth: 12.0 },
                selection: empty(),
                ..ZigzagConfig::default()
            }),
            Floor::StockTop { depth: 12.0 },
        ),
        (
            OperationConfig::Adaptive(AdaptiveConfig {
                floor: Floor::Top { depth: 3.0 },
                selection: empty(),
                ..AdaptiveConfig::default()
            }),
            Floor::Top { depth: 3.0 },
        ),
    ]
}

/// An `AppState` with one floor op on the face, its `tab` on screen.
fn state(operation: OperationConfig, tab: properties::ToolpathTab) -> AppState {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 3.0;
    let mut builder = ProjectSessionBuilder::new()
        .stock(StockConfig {
            x: 60.0,
            y: 60.0,
            z: 20.0,
            origin_x: -30.0,
            origin_y: -30.0,
            origin_z: -15.0,
            auto_from_model: false,
            ..StockConfig::default()
        })
        .tool(tool)
        .model(model());
    builder.add_toolpath(0, toolpath(operation)).unwrap();
    let mut state = AppState::new();
    state.session = builder.build();
    let _ = state
        .session
        .apply(Command::EditSelection(EditSelectionArgs {
            index: 0,
            target: SelectionTarget::Op,
            edit: SelectionEdit::AddPick(face_key()),
        }))
        .unwrap();
    let id = state.session.toolpath_configs()[0].id;
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    state.selection = GuiSelection::Toolpath(id);
    state.gui.pending_toolpath_tab = Some((id, tab));
    state
}

/// One painted text run and its screen rect.
struct Painted {
    text: String,
    rect: egui::Rect,
}

/// Draw the production inspector for `state` over passes; `click` presses
/// on pass 3 and releases on pass 4. Returns the runs of the last pass.
fn draw(ctx: &egui::Context, state: &mut AppState, click: Option<egui::Pos2>) -> Vec<Painted> {
    let passes = if click.is_some() { 6 } else { 3 };
    let mut painted = Vec::new();
    for pass in 0..passes {
        let mut events = Vec::new();
        if let Some(at) = click
            && (pass == 3 || pass == 4)
        {
            events.push(egui::Event::PointerMoved(at));
            events.push(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: pass == 3,
                modifiers: egui::Modifiers::default(),
            });
        }
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
        painted.clear();
        for clipped in &out.shapes {
            if let egui::epaint::Shape::Text(text) = &clipped.shape {
                painted.push(Painted {
                    text: text.galley.job.text.clone(),
                    rect: egui::Rect::from_min_size(text.pos, text.galley.size()),
                });
            }
        }
        out.textures_delta.clear();
    }
    painted
}

fn texts(painted: &[Painted]) -> Vec<&str> {
    painted.iter().map(|p| p.text.as_str()).collect()
}

#[test]
fn every_depth_shows_its_reference_p3_7() {
    let ctx = ctx();
    for (operation, floor) in floor_ops() {
        let label = operation.label();
        let reference = floor.reference_label();

        // 1. The Geometry tab: the floor row names its reference and the
        // resolved Z.
        let mut s = state(operation.clone(), properties::ToolpathTab::Geometry);
        let geometry = draw(&ctx, &mut s, None);
        let all = texts(&geometry);
        assert!(
            all.contains(&"Floor from:"),
            "{label}: no floor row: {all:?}"
        );
        assert!(
            all.contains(&reference),
            "{label}: the floor row does not name its reference {reference:?}: {all:?}"
        );
        assert!(
            all.iter().any(|t| t.starts_with("= Z ")),
            "{label}: the floor row does not show the resolved Z: {all:?}"
        );

        // 2. The Heights tab: one read-only Floor line, no Bottom row, and
        // the Top line names its source.
        let mut s = state(operation, properties::ToolpathTab::Heights);
        let heights = draw(&ctx, &mut s, None);
        let all = texts(&heights);
        assert!(
            all.iter()
                .any(|t| t.starts_with("Floor: ") && t.contains(reference) && t.contains("= Z ")),
            "{label}: the Heights tab has no Floor line with {reference:?}: {all:?}"
        );
        assert!(
            !all.contains(&"Bottom:"),
            "{label}: the Heights tab still draws a Bottom row: {all:?}"
        );
        assert!(
            all.contains(&"Top: auto: faces"),
            "{label}: the Top line does not name its source: {all:?}"
        );
    }
}

#[test]
fn the_each_face_hint_sets_the_floor_p3_7() {
    let ctx = ctx();
    let (pocket, _) = floor_ops().remove(0);
    let OperationConfig::Pocket(mut cfg) = pocket else {
        unreachable!()
    };
    cfg.floor = Floor::Top { depth: 3.0 };
    let mut s = state(
        OperationConfig::Pocket(cfg),
        properties::ToolpathTab::Geometry,
    );
    let painted = draw(&ctx, &mut s, None);
    let all = texts(&painted);
    assert!(
        all.iter()
            .any(|t| t.contains("To cut down to each face, choose Each picked face.")),
        "a face is picked under the Top floor: the hint must show: {all:?}"
    );
    let button = painted
        .iter()
        .find(|p| p.text == "Each picked face")
        .unwrap_or_else(|| panic!("the hint has no button: {all:?}"));
    let _ = draw(&ctx, &mut s, Some(button.rect.center()));
    assert_eq!(
        s.session.toolpath_configs()[0].operation.floor(),
        Some(Floor::EachFace { depth: 0.0 }),
        "the button must set the floor to each picked face, depth 0"
    );
    // Under "each picked face" the hint has nothing to offer.
    let after = draw(&ctx, &mut s, None);
    assert!(
        !texts(&after)
            .iter()
            .any(|t| t.contains("To cut down to each face")),
        "the hint must go once the floor is each picked face"
    );
}
