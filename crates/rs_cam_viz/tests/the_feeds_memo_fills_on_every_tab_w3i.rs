//! W3i: the feeds memo fills when the toolpath panel draws, whatever tab
//! is active.
//!
//! # The defect (found by W6, 2026-10-30)
//!
//! The memo of the feeds calculator (`entry.feeds_memo`, F216) filled only
//! when the Passes or the Feeds tab drew. On a fresh selection the Geometry
//! tab showed no notes counter for the feed findings, and the placement
//! missed them, until another tab had drawn.
//!
//! # The repair
//!
//! The panel host calls `feeds_speeds::refresh_feeds_memo` once per frame,
//! before it places the findings and before any tab draws.
//!
//! # The arm
//!
//! A fresh selection on the Geometry tab: before a frame the placement holds
//! no feed finding (non-vacuity); after ONE frame of the Geometry tab it
//! holds the rubbing-floor finding of the fixture, and the tab never drew
//! the Feeds tab.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::AppState;
use rs_cam_viz::state::runtime::ToolpathRuntime;
use rs_cam_viz::state::selection::Selection;
use rs_cam_viz::ui::{properties, tokens};

/// The default inspector width (`app.rs`).
const PANEL_WIDTH: f32 = 280.0;

/// The fixture of `inspector_width_is_tab_independent_up4.rs`: its recipe
/// carries the rubbing-floor warning (the derivation is there).
fn feeds_fixture_machine() -> MachineProfile {
    let mut machine = MachineProfile::generic_wood_router();
    machine.max_cutting_feed_mm_min = Some(200.0);
    machine
}

/// A real Feeds tab whose calculated recipe carries the rubbing-floor
/// warning, so the tab renders a warning row beside its annotated rows.
///
/// # Feeds matrix R5 re-bless (2026-09-23)
///
/// The fixture was the observed Back Rough: a Ø6 flat 2F `Adaptive3d` in
/// Baltic birch plywood. R5 moved that cell onto the printed Spektra row
/// `amana-flat-plywood-hardwood-pocket-6000-2f-spektra` (0.1159 mm/tooth,
/// max only), far above the 0.025 mm/tooth floor, so the recipe carries no
/// warning now. The width contract needs a real warning row, not that
/// particular cell.
///
/// The fixture now uses a cell that
/// `planning/feeds_matrix_2026-09-23/matrix_2026-09-23.csv` records with
/// `ChiploadClampedToFloor` (renamed `ChiploadBelowRubbingFloor` by ruling
/// R4 WP2a, 2026-09-23): a Ø3.175 flat 2F `Profile` in generic hardwood
/// (Janka 1450). The tool is the matrix instrument's `tool_of` tool. The
/// Feeds tab calculates the recipe with `feeds_result_for_operation`. That
/// door ignores the configured feed and RPM, and `Profile` gives no depth
/// or width hint. The arithmetic of the warning:
///
/// * The row is `amana-flat-hardwood-contour-3175-2f` (VendorBacked, so R1
///   does not refuse it): 0.018-0.030 mm/tooth at Janka 1300.
/// * The hardness scale is (1300 / 1450)^0.5 = 0.9469, so the band is
///   0.0170-0.0284 and the seed chipload is the midpoint, 0.0227.
/// * The matrix records the axial depth as 2.54 mm = 0.8 x D, so the depth
///   de-rate is 1.0 and the band maximum stays 0.0284.
/// * Ruling R4 (2026-09-24) removed the 0.75 safety factor, and the long-tool
///   share moved into the load target (Q7), so the feed keeps the seed
///   0.0227. Ruling Q9 made the floor min(0.025, band min) = 0.0170. 0.0227
///   is above it, so the cell alone no longer raises the warning.
/// * The fixture therefore runs on a machine with a cutting-feed ceiling of
///   200 mm/min, a real configuration (a slow gantry). The ceiling clamps the
///   feed to 200 mm/min. The RPM is at least the machine minimum 8000 RPM,
///   so the commanded advance is at most 200 / (2 x 8000) = 0.0125 mm/tooth,
///   below the 0.0170 floor. The calculator raises
///   `ChiploadBelowRubbingFloor` and the line paints
///   "Advance per tooth below the rubbing floor". The engine does not lift
///   the feed (ruling R4 WP2a).
fn feeds_fixture() -> AppState {
    let mut tool = ToolConfig::new_default(ToolId::from_raw(1), ToolType::EndMill);
    tool.diameter = 3.175;
    tool.flute_count = 2;
    tool.cutting_length = 12.0;
    tool.shank_diameter = 3.175;
    tool.shaft_diameter = 3.175;
    tool.stickout = 20.0;

    let stock = StockConfig {
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..Default::default()
    };

    // The configured feed and RPM are not the recipe. They give the tab a
    // "configured" value to compare with the recommendation, which is the
    // annotated row shape that D-16 widened.
    let mut operation = OperationConfig::Profile(Default::default());
    if let OperationConfig::Profile(config) = &mut operation {
        config.feed_rate = Some(3_000.0);
        config.spindle_rpm = Some(18_000);
    }
    let config = ToolpathConfig {
        shoe: Default::default(),
        id: rs_cam_core::ToolpathId::from_raw(0),
        name: "Hardwood profile width fixture".to_owned(),
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
    };

    let model = LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(1),
        path: PathBuf::from("hardwood_profile_width_fixture.svg"),
        name: "Hardwood profile width fixture".to_owned(),
        kind: Some(ModelKind::Svg),
        mesh: None,
        items: Arc::new(rs_cam_core::selection::ItemSet::from_polygons(vec![
            Polygon2::rectangle(0.0, 0.0, 10.0, 10.0),
        ])),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    };
    let mut builder = ProjectSessionBuilder::new()
        .stock(stock)
        .machine(feeds_fixture_machine())
        .tool(tool)
        .model(model);
    builder
        .add_toolpath(0, config)
        .expect("add the hardwood profile fixture");
    let mut state = AppState::new();
    state.replace_session(builder.build());
    let id = state.session().toolpath_configs()[0].id;
    // The controller creates this runtime row when it loads or adds a
    // toolpath. Mirror that normal lifecycle so the production panel's
    // write-back can cache the recipe it calculated on the selected tab.
    state.gui.toolpath_rt.insert(id, ToolpathRuntime::new(true));
    let _ = state.set_focus(Selection::Toolpath(id));
    // W3i: the Geometry tab, the one tab that never drew the feeds.
    state.gui.pending_toolpath_tab = Some((id, rs_cam_viz::ui::properties::ToolpathTab::Geometry));
    state
}

/// Every placed finding id of the toolpath, in every place.
fn placed_ids(state: &AppState) -> Vec<String> {
    let id = state.session().toolpath_configs()[0].id;
    let placed = properties::placed_findings(id, state).expect("the fixture toolpath has a panel");
    placed
        .tool
        .iter()
        .chain(&placed.by_field)
        .chain(&placed.issues)
        .chain(&placed.notes)
        .map(|d| d.id.as_str().to_owned())
        .collect()
}

#[test]
fn the_geometry_tab_places_the_feed_findings_on_the_first_frame_w3i() {
    let mut state = feeds_fixture();
    let floor = rs_cam_core::diagnostics::ids::FEEDS_CHIPLOAD_BELOW_FLOOR;
    // Non-vacuity: no frame has drawn, so the memo is empty.
    assert!(
        !placed_ids(&state).iter().any(|id| id == floor),
        "the placement holds the feed finding before any frame drew"
    );

    let ctx = egui::Context::default();
    tokens::apply(&ctx);
    tokens::apply_fonts(&ctx);
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
        egui::Panel::right("w3i_properties")
            .default_size(PANEL_WIDTH)
            .max_size(PANEL_WIDTH)
            .resizable(false)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let mut events = Vec::new();
                    properties::draw(ui, &mut state, &mut events);
                });
            });
    });
    out.textures_delta.clear();

    let ids = placed_ids(&state);
    assert!(
        ids.iter().any(|id| id == floor),
        "one frame of the Geometry tab must fill the feeds memo, so the \
         rubbing-floor finding is placed; placed: {ids:?}"
    );
}
