//! "Cut past open sides" (operator ruling 2026-10-23): the Pocket row of
//! the Cut tab.
//!
//! # The claims
//!
//! 1. A Pocket on `stepped_block.step` (its Z rings are model faces) shows
//!    the row "Cut past open sides:" beside the "Open paths:" row, and the
//!    row fits the 240-point rail with no wrap.
//! 2. A drawing Pocket (no face) does not show the row.
//! 3. A drawing Pocket with the setting on shows the row, so the operator
//!    can turn it off.
//!
//! The checkbox and the hover read the registry help of the parameter
//! (`cut_past_open_sides`), so the GUI hover is the MCP schema help.
//!
//! Red proof: draw the row for every Pocket (drop the `model_areas`
//! test in `draw_pocket_params`); claim 2 fails.
//!
//! ```text
//! cargo test -p rs_cam_viz -q --test the_pocket_open_sides_row_opensides
//! ```

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod inspector_harness_p6_7;

use inspector_harness_p6_7 as h;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::operation_configs::{HELP_CUT_PAST_OPEN_SIDES, PocketConfig};
use rs_cam_core::compute::stock_config::{ModelKind, ModelUnits, StockConfig};
use rs_cam_core::geo::P2;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::ItemSet;

const ROW: &str = "Cut past open sides:";

fn stock() -> StockConfig {
    StockConfig {
        x: 70.0,
        y: 50.0,
        z: Some(30.0),
        origin_x: -5.0,
        origin_y: -5.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn pocket(on: bool) -> OperationConfig {
    OperationConfig::Pocket(PocketConfig {
        cut_past_open_sides: on,
        ..PocketConfig::default()
    })
}

fn step_model() -> rs_cam_core::session::LoadedModel {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/gui_step/stepped_block.step");
    rs_cam_core::io::load_model_file(
        &path,
        rs_cam_core::ids::ModelId::from_raw(h::MODEL),
        ModelKind::Step,
        ModelUnits::Millimeters,
        Default::default(),
    )
    .expect("the STEP fixture loads")
}

fn square() -> rs_cam_core::session::LoadedModel {
    h::drawing_model(ItemSet::from_polygons(vec![Polygon2::new(vec![
        P2::new(10.0, 10.0),
        P2::new(30.0, 10.0),
        P2::new(30.0, 30.0),
        P2::new(10.0, 30.0),
    ])]))
}

fn painted_on_cut_tab(
    model: rs_cam_core::session::LoadedModel,
    on: bool,
) -> Vec<inspector_harness_p6_7::Painted> {
    let ctx = h::ctx();
    let mut state = h::state(model, 6.0, stock(), pocket(on));
    h::show_tab(&mut state, rs_cam_viz::ui::properties::ToolpathTab::Cut);
    h::render(&ctx, &mut state, h::RAIL_WIDTH)
}

#[test]
fn a_face_pocket_shows_the_row_opensides() {
    let painted = painted_on_cut_tab(step_model(), false);
    assert!(
        !h::find(&painted, ROW).is_empty(),
        "the row shows for model faces; painted: {:?}",
        painted.iter().map(|p| &p.text).collect::<Vec<_>>()
    );
    assert!(
        !h::find(&painted, "Open paths:").is_empty(),
        "beside the Open paths row"
    );
    let wrapped = h::wrapped(&painted);
    assert!(wrapped.is_empty(), "IC1: no text wraps: {wrapped:?}");
    let wide = h::past_the_rail(&painted);
    assert!(
        wide.is_empty(),
        "every row fits the 240-point rail: {wide:?}"
    );
    // The hover is the registry help of the parameter.
    let help = OperationType::Pocket
        .registry_entry()
        .params
        .defs
        .iter()
        .find(|d| d.name == "cut_past_open_sides")
        .and_then(|d| d.help);
    assert_eq!(help, Some(HELP_CUT_PAST_OPEN_SIDES));
    assert_eq!(
        HELP_CUT_PAST_OPEN_SIDES,
        "Clear the stock beyond the edges of the face that have no wall, out to the stock edge."
    );
}

#[test]
fn a_drawing_pocket_shows_no_row_opensides() {
    let painted = painted_on_cut_tab(square(), false);
    assert!(
        !h::find(&painted, "Open paths:").is_empty(),
        "non-vacuity: the Cut tab is drawn"
    );
    assert!(h::find(&painted, ROW).is_empty(), "no face, no row");
    let painted = painted_on_cut_tab(square(), true);
    assert!(
        !h::find(&painted, ROW).is_empty(),
        "the row shows while the setting is on"
    );
}
