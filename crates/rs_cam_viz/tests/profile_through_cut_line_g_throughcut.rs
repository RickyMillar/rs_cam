//! G-THROUGHCUT sentry: a Profile at or beyond the stock thickness names
//! the through cut and its holding.
//!
//! UX-R03-006 (`planning/ui_review_2026-09-09/results/R03/REPORT.md`): a
//! Profile at depth 12 on a 12 mm board with zero tabs generated and
//! simulated, and no surface said the part would be free on the last pass.
//!
//! Cases on the pure builder `profile_through_cut_line`:
//! (a) depth under the thickness: `None`;
//! (b) depth EXACTLY the thickness, no tabs: the "no tabs configured" line;
//! (c) depth beyond the thickness, four tabs: the "4 tabs" line;
//! (d) one tab reads singular; a fractional board keeps one decimal;
//! plus the inspector form `profile_through_cut`, session-built the way the
//! params panel builds it, with an `Auto` Top Z and with a Top Z pinned
//! below the stock top.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::ToolpathId;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{HeightMode, HeightReference, ReferenceOffset};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, ProjectSessionBuilder, ToolpathConfig};
use rs_cam_viz::state::job::{ModelKind, ModelUnits};
use rs_cam_viz::ui::properties::{
    ThroughCut, depth_beyond_stock, profile_through_cut, profile_through_cut_line,
};

const TOOL: usize = 1;
const MODEL_2D: usize = 4;
const STOCK_THICKNESS_MM: f64 = 18.0;

const NO_TABS_18: &str = "Through cut of a 18 mm board · Holding: no tabs configured";

// ── The pure builder ────────────────────────────────────────────────────

#[test]
fn a_partial_depth_profile_has_no_through_cut_line_g_throughcut() {
    assert_eq!(profile_through_cut_line(6.0, 12.0, 0), None);
    assert_eq!(profile_through_cut_line(11.9, 12.0, 4), None);
    // No board, no through cut.
    assert_eq!(profile_through_cut_line(12.0, 0.0, 0), None);
    assert_eq!(profile_through_cut_line(f64::NAN, 12.0, 0), None);
    assert_eq!(profile_through_cut_line(12.0, f64::INFINITY, 0), None);
}

#[test]
fn b_depth_exactly_at_the_thickness_names_the_through_cut_and_no_tabs_g_throughcut() {
    assert_eq!(
        profile_through_cut_line(12.0, 12.0, 0).as_deref(),
        Some("Through cut of a 12 mm board · Holding: no tabs configured"),
        "(b) the boundary depth == thickness IS a through cut"
    );
    // Float noise on the equal side must not hide the line.
    assert_eq!(
        profile_through_cut_line(12.0 - 1e-9, 12.0, 0).as_deref(),
        Some("Through cut of a 12 mm board · Holding: no tabs configured"),
    );
}

#[test]
fn c_depth_beyond_the_thickness_names_the_tabs_g_throughcut() {
    assert_eq!(
        profile_through_cut_line(15.0, 12.0, 4).as_deref(),
        Some("Through cut of a 12 mm board · Holding: 4 tabs"),
    );
}

#[test]
fn d_one_tab_is_singular_and_a_fractional_board_keeps_one_decimal_g_throughcut() {
    assert_eq!(
        profile_through_cut_line(18.5, 18.5, 1).as_deref(),
        Some("Through cut of a 18.5 mm board · Holding: 1 tab"),
    );
    assert_eq!(
        profile_through_cut_line(19.05, 19.05, 2).as_deref(),
        Some("Through cut of a 19.1 mm board · Holding: 2 tabs"),
    );
}

// ── The inspector form, session-built ───────────────────────────────────

fn polygon_model(id: usize) -> LoadedModel {
    LoadedModel {
        id: rs_cam_core::ids::ModelId::from_raw(id),
        path: PathBuf::from(format!("model_{id}.svg")),
        name: format!("2D {id}"),
        kind: Some(ModelKind::Svg),
        mesh: None,
        items: Arc::new(rs_cam_core::selection::ItemSet::from_polygons(vec![
            Polygon2::rectangle(-10.0, -10.0, 10.0, 10.0),
        ])),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
        import_options: Default::default(),
        import_report: None,
        drawing: Default::default(),
    }
}

fn toolpath(name: &str, model_id: usize, op: OperationConfig) -> ToolpathConfig {
    ToolpathConfig {
        shoe: Default::default(),
        id: ToolpathId::from_raw(0), // assigned by session.add_toolpath
        name: name.to_owned(),
        enabled: true,
        operation: op,
        dressups: Default::default(),
        heights: Default::default(),
        tool_id: rs_cam_core::ids::ToolId::from_raw(TOOL),
        model_id: rs_cam_core::ids::ModelId::from_raw(model_id),
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

fn profile(depth: f64, tab_count: usize) -> OperationConfig {
    let mut op = OperationConfig::Profile(Default::default());
    if let OperationConfig::Profile(cfg) = &mut op {
        cfg.floor = rs_cam_core::compute::floor::Floor::top(depth);
        cfg.tab_count = tab_count;
    }
    op
}

/// One tool, one 2D model, 18 mm stock, one setup.
///
/// The builder is returned unbuilt, so the caller adds its Profile through
/// the same door.
fn build_session() -> ProjectSessionBuilder {
    let mut tool = ToolConfig::new_default(ToolId::from_raw(TOOL), ToolType::EndMill);
    tool.diameter = 6.0;
    let stock = StockConfig {
        z: STOCK_THICKNESS_MM,
        auto_from_model: false,
        ..StockConfig::default()
    };
    ProjectSessionBuilder::new()
        .tool(tool)
        .model(polygon_model(MODEL_2D))
        .stock(stock)
}

/// The rule the params panel reads for the Profile at `idx`.
fn rule(session: &ProjectSession, idx: usize) -> Option<ThroughCut> {
    let tc = &session.toolpath_configs()[idx];
    let height_ctx = session.toolpath_heights(tc).expect("the heights resolve");
    let OperationConfig::Profile(cfg) = &tc.operation else {
        panic!("the toolpath at {idx} is a Profile");
    };
    profile_through_cut(cfg, &tc.heights, &height_ctx)
}

#[test]
fn a_full_depth_profile_with_no_tabs_reads_the_seed_line_g_throughcut() {
    let mut builder = build_session();
    let idx = builder
        .add_toolpath(
            0,
            toolpath("Cut out", MODEL_2D, profile(STOCK_THICKNESS_MM, 0)),
        )
        .expect("add_toolpath reports the new toolpath index");
    let session = builder.build();
    let finding = rule(&session, idx).expect("(a) depth == thickness is a through cut");
    assert_eq!(finding.message(), NO_TABS_18);
    assert!((finding.depth_mm - STOCK_THICKNESS_MM).abs() < 1e-9);
    assert!((finding.stock_thickness_mm - STOCK_THICKNESS_MM).abs() < 1e-9);
    assert_eq!(finding.tab_count, 0);

    // The two rules partition the boundary: at depth == thickness the
    // G-DEPTHSTOCK caution stays silent and this line speaks.
    let tc = &session.toolpath_configs()[idx];
    let height_ctx = session.toolpath_heights(tc).expect("the heights resolve");
    assert_eq!(
        depth_beyond_stock(&tc.operation, &tc.heights, &height_ctx),
        None,
        "(a) exactly at the thickness is not an exceedance"
    );
}

#[test]
fn a_partial_depth_profile_reads_nothing_g_throughcut() {
    let mut builder = build_session();
    let idx = builder
        .add_toolpath(0, toolpath("Rebate", MODEL_2D, profile(6.0, 0)))
        .expect("add_toolpath reports the new toolpath index");
    let session = builder.build();
    assert_eq!(rule(&session, idx), None);
}

#[test]
fn a_depth_beyond_the_board_shows_both_the_line_and_the_caution_g_throughcut() {
    let mut builder = build_session();
    let idx = builder
        .add_toolpath(0, toolpath("Deep", MODEL_2D, profile(25.0, 3)))
        .expect("add_toolpath reports the new toolpath index");
    let session = builder.build();
    let finding = rule(&session, idx).expect("beyond the board is a through cut");
    assert_eq!(
        finding.message(),
        "Through cut of a 18 mm board · Holding: 3 tabs"
    );
    let tc = &session.toolpath_configs()[idx];
    let height_ctx = session.toolpath_heights(tc).expect("the heights resolve");
    let caution = depth_beyond_stock(&tc.operation, &tc.heights, &height_ctx)
        .expect("beyond the board is also an exceedance");
    assert!((caution.excess_mm - 7.0).abs() < 1e-9);
}

#[test]
fn a_top_z_pinned_below_the_stock_top_counts_towards_the_through_cut_g_throughcut() {
    // Depth 12 on an 18 mm board is a rebate — until Top Z is pinned 6 mm
    // down (a face pass went first), at which point the bottom reaches the
    // stock bottom. The generator cuts `top_z - depth`, so the line reads it.
    let mut builder = build_session();
    let mut tc = toolpath("After face", MODEL_2D, profile(12.0, 0));
    tc.heights.top_z = HeightMode::FromReference(ReferenceOffset {
        reference: HeightReference::StockTop,
        offset: -6.0,
    });
    let idx = builder
        .add_toolpath(0, tc)
        .expect("add_toolpath reports the new toolpath index");
    let session = builder.build();
    let finding = rule(&session, idx).expect("pinned Top Z reaches the stock bottom");
    assert_eq!(finding.message(), NO_TABS_18);
    assert!(
        (finding.depth_mm - STOCK_THICKNESS_MM).abs() < 1e-9,
        "the depth the rule compares is from the STOCK top: {}",
        finding.depth_mm
    );

    // The same op with Top Z at the stock top is a rebate again.
    let mut builder = build_session();
    let idx = builder
        .add_toolpath(0, toolpath("Rebate", MODEL_2D, profile(12.0, 0)))
        .expect("add_toolpath reports the new toolpath index");
    let session = builder.build();
    assert_eq!(rule(&session, idx), None);
}
