//! N4 sentry: the inspector ribbon and the MCP route must report one
//! diagnostic id set for the same toolpath.
//!
//! The GUI ribbon calls `collect_diagnostics`
//! (`ui/properties/mod.rs` → `ui/properties/operations/mod.rs`). MCP
//! `get_toolpath_diagnostics` calls
//! `ProjectSession::diagnose_toolpath_with_trace`. An operator and an
//! agent must read the same findings.
//!
//! # What N4 found
//!
//! The two routes built their heights snapshot from different sources.
//! The ribbon resolves the toolpath's own `HeightsConfig`. The core
//! route called `ResolvedHeights::from_context`, which PROJECTS the
//! stock top and the safe Z into the five slots and drops every pin.
//! Three `heights_checks` predicates therefore could not fire at all on
//! the MCP route:
//!
//! * `geom.bottom_above_top_z`,
//! * `geom.feed_z_below_top_z`,
//! * `geom.clearance_z_below_retract_z`.
//!
//! A fourth, `geom.retract_z_below_feed_z`, fired only on a bad safe Z.
//! And `geom.depth_beyond_stock` missed a Top Z pinned below the stock
//! top. The GUI raised that caution; the MCP route stayed silent.
//!
//! The test the repo carried before N4
//! (`gui_and_mcp_diagnostic_ids_match`) could not see this. Both of its
//! arms called `diagnose_toolpath_inputs` with one input set, so the
//! assertion could not fail. This file calls the real ribbon function
//! instead.
//!
//! # N9 runtime-red extension
//!
//! N9 adds Rest and dangling-model fixtures for the former context
//! asymmetry. The ribbon arm calls `toolpath_panel_snapshot`, the same owned
//! entry + context assembly helper used by the production `draw` caller, and
//! the GUI collector requires both contexts. These arms therefore fail if
//! production assembly drops either context instead of merely proving that
//! the collector accepts manually built inputs.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::ToolpathId;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{HeightMode, HeightReference, HeightsConfig, ReferenceOffset};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::diagnostics::Diagnostic;
use rs_cam_core::diagnostics::ids::{
    GEOM_BOTTOM_ABOVE_TOP_Z, GEOM_CLEARANCE_Z_BELOW_RETRACT_Z, GEOM_DEPTH_BEYOND_STOCK,
    GEOM_FEED_Z_BELOW_TOP_Z, GEOM_RETRACT_Z_BELOW_FEED_Z,
};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, ToolpathConfig};
use rs_cam_viz::state::job::{ModelKind, ModelUnits};
use rs_cam_viz::state::runtime::GuiState;
use rs_cam_viz::ui::properties::{collect_diagnostics, toolpath_panel_snapshot};

const TOOL: usize = 1;
const REST_TOOL: usize = 2;
const PREVIOUS_TOOL: usize = 3;
const MODEL_2D: usize = 4;
const STOCK_THICKNESS_MM: f64 = 18.0;

/// The four height ids the projection could not raise.
const HEIGHT_IDS: &[&str] = &[
    GEOM_BOTTOM_ABOVE_TOP_Z,
    GEOM_FEED_Z_BELOW_TOP_Z,
    GEOM_RETRACT_Z_BELOW_FEED_Z,
    GEOM_CLEARANCE_Z_BELOW_RETRACT_Z,
];

// ── fixture ─────────────────────────────────────────────────────────────

fn polygon_model(id: usize) -> LoadedModel {
    LoadedModel {
        id,
        path: PathBuf::from(format!("model_{id}.svg")),
        name: format!("2D {id}"),
        kind: Some(ModelKind::Svg),
        mesh: None,
        polygons: Some(Arc::new(vec![Polygon2::rectangle(
            -10.0, -10.0, 10.0, 10.0,
        )])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        enriched_mesh: None,
        units: Some(ModelUnits::Millimeters),
        winding_report: None,
        load_error: None,
    }
}

fn toolpath(
    name: &str,
    model_id: usize,
    op: OperationConfig,
    heights: HeightsConfig,
) -> ToolpathConfig {
    ToolpathConfig {
        id: ToolpathId(0), // assigned by session.add_toolpath
        name: name.to_owned(),
        enabled: true,
        operation: op,
        dressups: Default::default(),
        heights,
        tool_id: TOOL,
        model_id,
        pre_gcode: None,
        post_gcode: None,
        boundary: Default::default(),
        boundary_inherit: true,
        stock_source: Default::default(),
        coolant: Default::default(),
        face_selection: None,
        debug_options: Default::default(),
        feeds_provenance: Default::default(),
        rest_analysis: Default::default(),
        planner_origin: None,
    }
}

fn pocket(depth: f64) -> OperationConfig {
    let mut op = OperationConfig::Pocket(Default::default());
    if let OperationConfig::Pocket(cfg) = &mut op {
        cfg.depth = depth;
    }
    op
}

fn pinned(reference: HeightReference, offset: f64) -> HeightMode {
    HeightMode::FromReference(ReferenceOffset { reference, offset })
}

/// Three tools, one 2D model, an 18 mm board, one setup.
fn session() -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    let mut tool = ToolConfig::new_default(ToolId(TOOL), ToolType::EndMill);
    tool.diameter = 6.0;
    let mut rest_tool = ToolConfig::new_default(ToolId(REST_TOOL), ToolType::EndMill);
    rest_tool.diameter = 3.0;
    let mut previous_tool = ToolConfig::new_default(ToolId(PREVIOUS_TOOL), ToolType::EndMill);
    previous_tool.diameter = 6.0;
    let _ = session.replace_tools(vec![tool, rest_tool, previous_tool]);
    session.models_mut().push(polygon_model(MODEL_2D));
    let stock = StockConfig {
        z: STOCK_THICKNESS_MM,
        auto_from_model: false,
        ..StockConfig::default()
    };
    let _ = session.set_stock_config(stock);
    session
}

/// Add one operation and return its index.
fn add_operation(session: &mut ProjectSession, config: ToolpathConfig) -> usize {
    session
        .add_toolpath(0, config)
        .expect("the session accepts the operation")
}

/// Add one Pocket and return its index.
fn add_pocket(session: &mut ProjectSession, depth: f64, heights: HeightsConfig) -> usize {
    add_operation(
        session,
        toolpath("Pocket", MODEL_2D, pocket(depth), heights),
    )
}

fn rest() -> OperationConfig {
    let mut op = OperationConfig::Rest(Default::default());
    if let OperationConfig::Rest(cfg) = &mut op {
        cfg.prev_tool_id = Some(ToolId(PREVIOUS_TOOL));
    }
    op
}

// ── the two surfaces ────────────────────────────────────────────────────

/// The ribbon arm. This is the function `ui/properties/mod.rs` calls to
/// fill the inspector's diagnostics ribbon, with the inputs that panel
/// hands it. `feeds_result` is the value the Feeds tab caches on the
/// entry; `feeds_result_for_toolpath` is the same calculator call with
/// the session's own material, machine and spindle strategy.
fn ribbon_diagnostics(session: &ProjectSession, idx: usize) -> Vec<Diagnostic> {
    let tc = &session.toolpath_configs()[idx];
    let tool = session
        .tools()
        .iter()
        .find(|t| t.id.0 == tc.tool_id)
        .cloned()
        .expect("the tool is in the session");
    let height_ctx = session.height_context_for_toolpath(tc);
    let stale_defaults = rs_cam_core::compute::validate::validate_one_toolpath(
        tc,
        Some(&tool),
        &session.stock_config().material,
        session.stock_config().bbox().min.z,
    );
    let load_report = rs_cam_core::gcode::project_load_report(session, None);
    let load_verdict = load_report
        .per_toolpath
        .iter()
        .find(|v| v.toolpath_id == tc.id);
    let mut snapshot = toolpath_panel_snapshot(tc.id, session, &GuiState::default())
        .expect("the toolpath resolves for the properties panel");
    snapshot.entry.feeds_result = session.feeds_result_for_toolpath(tc, &tool);
    collect_diagnostics(
        &snapshot.entry,
        Some(&tool),
        &stale_defaults,
        Some(&height_ctx),
        &snapshot.preconditions,
        &snapshot.model_refs,
        load_verdict,
    )
}

fn ids_of(diags: &[Diagnostic]) -> HashSet<String> {
    diags.iter().map(|d| d.id.0.clone()).collect()
}

/// The MCP arm. `get_toolpath_diagnostics` calls this method.
fn mcp_ids(session: &ProjectSession, idx: usize) -> HashSet<String> {
    let diags = session
        .diagnose_toolpath_with_trace(idx, None)
        .expect("the toolpath exists");
    ids_of(&diags)
}

fn ribbon_ids(session: &ProjectSession, idx: usize) -> HashSet<String> {
    ids_of(&ribbon_diagnostics(session, idx))
}

// ── arm 1: the baseline ─────────────────────────────────────────────────

/// A 6 mm Pocket with Auto heights on an 18 mm board. Both surfaces
/// report the same ids, and neither raises a height finding.
///
/// Auto resolves `feed_z` to `safe_z - 2`, and `safe_z` is floored at
/// `stock_top + SAFE_Z_CLEARANCE_MM`, so the approach plane stays above
/// the board. This arm is green before and after the N4 fix.
#[test]
fn the_two_surfaces_agree_on_a_pocket_with_auto_heights() {
    let mut session = session();
    let idx = add_pocket(&mut session, 6.0, HeightsConfig::default());

    let ribbon = ribbon_ids(&session, idx);
    let mcp = mcp_ids(&session, idx);

    assert!(
        !ribbon.contains(rs_cam_core::diagnostics::ids::REF_MODEL_MISSING),
        "a resolved model must raise no missing-model finding on the ribbon; it reported {ribbon:?}"
    );
    assert!(
        !mcp.contains(rs_cam_core::diagnostics::ids::REF_MODEL_MISSING),
        "a resolved model must raise no missing-model finding on the MCP route; it reported {mcp:?}"
    );
    for id in HEIGHT_IDS {
        assert!(
            !ribbon.contains(*id),
            "Auto heights must raise no {id} on the ribbon; it reported {ribbon:?}"
        );
        assert!(
            !mcp.contains(*id),
            "Auto heights must raise no {id} on the MCP route; it reported {mcp:?}"
        );
    }
    assert_eq!(
        ribbon, mcp,
        "the ribbon and the MCP route disagree on a Pocket with Auto heights\n\
         ribbon: {ribbon:?}\nMCP: {mcp:?}"
    );
}

// ── arm 2: a pinned Top Z deepens the cut ───────────────────────────────

/// A 15 mm Pocket with Top Z pinned 5 mm below the stock top cuts 2 mm
/// through an 18 mm board. The generators cut `top_z - depth`, so both
/// surfaces must carry `geom.depth_beyond_stock`.
///
/// This arm is RED before the N4 fix: the MCP set lacks the id, because
/// `ResolvedHeights::from_context` reports the stock top as `top_z` and
/// drops the pin.
#[test]
fn a_pinned_top_z_reaches_both_surfaces() {
    let mut session = session();
    let heights = HeightsConfig {
        top_z: pinned(HeightReference::StockTop, -5.0),
        ..HeightsConfig::default()
    };
    let idx = add_pocket(&mut session, 15.0, heights);

    let ribbon = ribbon_ids(&session, idx);
    // Non-vacuity: assert the ribbon carries the caution BEFORE the
    // equality below, so two empty sets cannot pass this test.
    assert!(
        ribbon.contains(GEOM_DEPTH_BEYOND_STOCK),
        "the ribbon must caution on a 15 mm cut under a Top Z pinned 5 mm \
         down; it reported {ribbon:?}"
    );

    let mcp = mcp_ids(&session, idx);
    assert!(
        mcp.contains(GEOM_DEPTH_BEYOND_STOCK),
        "the MCP route must carry the same caution; it reported {mcp:?}"
    );
    assert_eq!(
        ribbon, mcp,
        "the ribbon and the MCP route disagree on a pinned Top Z\n\
         ribbon: {ribbon:?}\nMCP: {mcp:?}"
    );
}

// ── arm 3: a pinned Retract Z below a pinned Feed Z ──────────────────────

/// Retract Z pinned 5 mm above the stock top, Feed Z pinned 10 mm above
/// it. The retract plane then sits below the approach plane, which
/// `heights_checks` reports as `geom.retract_z_below_feed_z`.
///
/// Both pins reference the stock top, so the finding does not depend on
/// the post's safe Z. Clearance Z stays Auto at `retract + 10`, above
/// the retract plane, and `feed_z` stays above `top_z`, so this arm adds
/// exactly one id.
///
/// This arm is RED before the N4 fix: the MCP route reads `retract_z` as
/// the safe Z and `feed_z` as the stock top, so the comparison cannot
/// fire.
#[test]
fn a_retract_pinned_below_the_feed_plane_reaches_both_surfaces() {
    let mut session = session();
    let heights = HeightsConfig {
        retract_z: pinned(HeightReference::StockTop, 5.0),
        feed_z: pinned(HeightReference::StockTop, 10.0),
        ..HeightsConfig::default()
    };
    let idx = add_pocket(&mut session, 6.0, heights);

    let ribbon = ribbon_ids(&session, idx);
    // Non-vacuity, as in arm 2.
    assert!(
        ribbon.contains(GEOM_RETRACT_Z_BELOW_FEED_Z),
        "the ribbon must report a retract plane below the feed plane; it \
         reported {ribbon:?}"
    );

    let mcp = mcp_ids(&session, idx);
    assert!(
        mcp.contains(GEOM_RETRACT_Z_BELOW_FEED_Z),
        "the MCP route must report the same finding; it reported {mcp:?}"
    );
    assert_eq!(
        ribbon, mcp,
        "the ribbon and the MCP route disagree on pinned retract and feed \
         planes\nribbon: {ribbon:?}\nMCP: {mcp:?}"
    );
}

// ── N9: ribbon/session context parity ──────────────────────────────────

#[test]
fn rest_without_an_enabled_previous_tool_reaches_both_surfaces() {
    let mut session = session();
    let idx = add_operation(
        &mut session,
        toolpath("Rest", MODEL_2D, rest(), HeightsConfig::default()),
    );
    session.toolpath_configs_mut()[idx].tool_id = REST_TOOL;

    let ribbon = ribbon_ids(&session, idx);
    assert!(
        ribbon.contains(rs_cam_core::diagnostics::ids::PRECOND_REST_NO_PRIOR),
        "the ribbon must report the missing Rest predecessor; it reported {ribbon:?}"
    );

    let mcp = mcp_ids(&session, idx);
    assert!(
        mcp.contains(rs_cam_core::diagnostics::ids::PRECOND_REST_NO_PRIOR),
        "the MCP route must report the missing Rest predecessor; it reported {mcp:?}"
    );
    assert_eq!(
        ribbon, mcp,
        "the ribbon and MCP route disagree on a Rest operation without a predecessor\n\
         ribbon: {ribbon:?}\nMCP: {mcp:?}"
    );
}

#[test]
fn rest_with_an_enabled_previous_tool_clears_the_precondition_on_both_surfaces() {
    let mut session = session();
    let predecessor = add_operation(
        &mut session,
        toolpath("Rough", MODEL_2D, pocket(6.0), HeightsConfig::default()),
    );
    session.toolpath_configs_mut()[predecessor].tool_id = PREVIOUS_TOOL;
    let rest = add_operation(
        &mut session,
        toolpath("Rest", MODEL_2D, rest(), HeightsConfig::default()),
    );
    session.toolpath_configs_mut()[rest].tool_id = REST_TOOL;

    let ribbon = ribbon_ids(&session, rest);
    let mcp = mcp_ids(&session, rest);
    assert!(
        !ribbon.contains(rs_cam_core::diagnostics::ids::PRECOND_REST_NO_PRIOR),
        "the ribbon must clear the Rest predecessor finding; it reported {ribbon:?}"
    );
    assert!(
        !mcp.contains(rs_cam_core::diagnostics::ids::PRECOND_REST_NO_PRIOR),
        "the MCP route must clear the Rest predecessor finding; it reported {mcp:?}"
    );
    assert_eq!(
        ribbon, mcp,
        "the surfaces disagree on a valid Rest predecessor"
    );
}

#[test]
fn a_pocket_with_a_dangling_model_reaches_both_surfaces() {
    let mut session = session();
    let idx = add_operation(
        &mut session,
        toolpath("Pocket", 999, pocket(6.0), HeightsConfig::default()),
    );

    let ribbon = ribbon_ids(&session, idx);
    assert!(
        ribbon.contains(rs_cam_core::diagnostics::ids::REF_MODEL_MISSING),
        "the ribbon must report the dangling model reference; it reported {ribbon:?}"
    );

    let mcp = mcp_ids(&session, idx);
    assert!(
        mcp.contains(rs_cam_core::diagnostics::ids::REF_MODEL_MISSING),
        "the MCP route must report the dangling model reference; it reported {mcp:?}"
    );
    assert_eq!(
        ribbon, mcp,
        "the ribbon and MCP route disagree on a dangling model reference\n\
         ribbon: {ribbon:?}\nMCP: {mcp:?}"
    );
}
