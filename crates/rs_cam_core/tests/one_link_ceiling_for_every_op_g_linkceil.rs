//! G-LINKCEIL — one link ceiling for every op (ruling R1, W1.1).
//!
//! # The defect this is red against
//!
//! Two rules built the link ceiling (`planning/op_architecture_2026-10-14/`
//! AUDIT.md, D1). Project Curve used `tool_def.radius()` and
//! `heights.top_z`, always. The raster family used `envelope_radius_mm()`
//! and the stock top, only with a stock snapshot. On fresh stock, Project
//! Curve with a pinned `top_z` below the stock top put every fed link at
//! `top_z + PLUNGE_CLEARANCE_MM`, inside the standing stock.
//!
//! # The rule now
//!
//! `compute::execute::link_ceiling_for` is the one builder:
//!
//! - stock = the remaining stock, when the op has a snapshot;
//! - tool radius = the envelope radius;
//! - fallback top = `dressup::fresh_stock_top_z(stock top, heights.top_z)`,
//!   the G-PECKSPLIT rule;
//! - each op states its `LinkCeilingNeed`: Project Curve always, the
//!   finishing ops only with remaining stock.
//!
//! # The cases
//!
//! 1. Fresh stock, 25 mm thick, Project Curve with `top_z` pinned at 7 mm:
//!    no fed link point sits below the stock top + `PLUNGE_CLEARANCE_MM`.
//! 2. A raster finish and Project Curve over the same remaining stock build
//!    the same ceiling values.
//! 3. Auto heights: the output does not change. G-OPGOLDEN
//!    (`every_op_keeps_its_toolpath_g_opgolden`) holds the toolpath of every
//!    op; this file asserts the reason (the Auto top IS the stock top).
//! 4. A finishing op on fresh stock builds no ceiling.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test one_link_ceiling_for_every_op_g_linkceil
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::meshes::height_field;
use common::session::{polygon_model, stock_over, toolpath_config};
use common::tools::endmill_tool_config;
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{HeightContext, HeightMode, HeightsConfig};
use rs_cam_core::compute::execute::{LinkCeilingNeed, link_ceiling_for};
use rs_cam_core::compute::operation_configs::ProjectCurveConfig;
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::ids::ModelId;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::ProjectSessionBuilder;
use rs_cam_core::stock::radial_profile::{LUT_SAMPLES, RadialProfileLUT};
use rs_cam_core::tool::{FlatEndmill, MillingCutter};
use rs_cam_core::toolpath::{Move, MoveIntent, MoveType, PLUNGE_CLEARANCE_MM};

// ── Case 1 fixture ──────────────────────────────────────────────────────

/// Half extent (mm) of the plate mesh.
const HALF: f64 = 15.0;
/// The plate surface Z (mm). The stock stands 20 mm above it.
const PLATE_Z: f64 = 5.0;
/// Fresh stock thickness (mm), from Z0: the stock top is at 25.
const STOCK_Z: f64 = 25.0;
/// The pinned `top_z` (mm): above the plate, 18 mm below the stock top.
const PINNED_TOP_Z: f64 = 7.0;
/// The pinned retract (mm). It sits above `STOCK_Z + PLUNGE_CLEARANCE_MM`,
/// so the relinker does not refuse a link that clears the stock top.
const RETRACT_Z: f64 = 40.0;
const TOOL_DIAMETER_MM: f64 = 2.0;
const CUT_DEPTH_MM: f64 = 1.0;
const POINT_SPACING_MM: f64 = 0.5;
/// The chains: short open paths along X, `GAP_MM` apart.
const CHAIN_COUNT: usize = 5;
const CHAIN_LEN_MM: f64 = 4.0;
const GAP_MM: f64 = 1.5;
const FIRST_CHAIN_X_MM: f64 = -12.0;
/// The chain cap: over the gap, under the next chain.
const CHAIN_DISTANCE_MM: f64 = 3.0;

fn curves() -> Vec<Polygon2> {
    (0..CHAIN_COUNT)
        .map(|i| {
            let x0 = FIRST_CHAIN_X_MM + i as f64 * (CHAIN_LEN_MM + GAP_MM);
            Polygon2::open_path(vec![P2::new(x0, 0.0), P2::new(x0 + CHAIN_LEN_MM, 0.0)])
        })
        .collect()
}

/// Generate Project Curve through the session (the GUI, MCP and CLI door)
/// on fresh stock, with `top_z` pinned below the stock top.
fn project_curve_on_fresh_stock(chain_distance_mm: f64) -> Vec<Move> {
    let mut builder = ProjectSessionBuilder::new().stock(stock_over(HALF, STOCK_Z));
    let tool_idx = builder.add_tool(endmill_tool_config(TOOL_DIAMETER_MM));
    let tool_id = builder.tools()[tool_idx].id.0;
    let plate =
        common::session::mesh_model(height_field(HALF, 1.0, |_, _| PLATE_Z), "g_linkceil_plate");
    let plate_id = builder.add_model(plate);
    let curve_id = builder.add_model(polygon_model(curves(), "g_linkceil_curves"));
    let op = OperationConfig::ProjectCurve(ProjectCurveConfig {
        depth: CUT_DEPTH_MM,
        point_spacing: POINT_SPACING_MM,
        chain_distance_mm,
        surface_model_id: Some(ModelId(plate_id)),
        ..ProjectCurveConfig::default()
    });
    let mut config = toolpath_config("Project Curve", op, tool_id, curve_id);
    config.heights = HeightsConfig {
        clearance_z: HeightMode::Manual(RETRACT_Z + 5.0),
        retract_z: HeightMode::Manual(RETRACT_Z),
        top_z: HeightMode::Manual(PINNED_TOP_Z),
        bottom_z: HeightMode::Manual(0.0),
        ..HeightsConfig::default()
    };
    let index = builder.add_toolpath(0, config).expect("add Project Curve");
    let mut session = builder.build();
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(index, &cancel)
        .expect("Project Curve generates on the plate")
        .toolpath()
        .moves
        .clone()
}

fn same_point(a: P3, b: P3) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9 && (a.z - b.z).abs() < 1e-9
}

/// Every fed position, selected by MOVE TYPE, never by intent.
fn fed_positions(moves: &[Move]) -> Vec<P3> {
    moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .map(|m| m.target)
        .collect()
}

/// Case 1. On fresh stock no dexel answers, so the ceiling is the fallback
/// top everywhere under the tool disc. That top must be the STOCK top.
#[test]
fn fresh_stock_project_curve_links_clear_the_stock_top() {
    let chained = project_curve_on_fresh_stock(CHAIN_DISTANCE_MM);
    let unchained = project_curve_on_fresh_stock(0.0);
    // A link ENDS on the next chain's entry, which is a cut position. All
    // other link points are traverse geometry.
    let cut_positions = fed_positions(&unchained);
    let link_points: Vec<P3> = chained
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid) && m.intent == MoveIntent::Linking)
        .map(|m| m.target)
        .filter(|p| !cut_positions.iter().any(|c| same_point(*c, *p)))
        .collect();
    assert!(
        !link_points.is_empty(),
        "control: the fixture must chain, or there is nothing to check"
    );
    let required = STOCK_Z + PLUNGE_CLEARANCE_MM;
    let low: Vec<&P3> = link_points
        .iter()
        .filter(|p| p.z < required - 1e-9)
        .collect();
    assert!(
        low.is_empty(),
        "{} of {} fed link points sit below the stock top + clearance \
         ({required:.3}); the lowest is at Z {:.3}. The ceiling read the \
         pinned top_z ({PINNED_TOP_Z}) and not the stock top ({STOCK_Z}).",
        low.len(),
        link_points.len(),
        low.iter().map(|p| p.z).fold(f64::INFINITY, f64::min),
    );
}

// ── Case 2 fixture ──────────────────────────────────────────────────────

/// Remaining stock: a 3 mm slab over Z0, cleared to Z0 except one rib.
/// This is the `ribbed_stock` shape of `compute/execute/project_curve_chaining.rs`.
fn ribbed_stock() -> TriDexelStock {
    let mut stock = TriDexelStock::from_stock(-10.0, -10.0, 10.0, 10.0, -12.0, 3.0, 0.25);
    let clearer = FlatEndmill::new(3.0, 25.0);
    let radius = 1.5;
    let lut = RadialProfileLUT::from_cutter(&clearer, LUT_SAMPLES);
    let mut x = -10.0 - radius;
    while x <= 10.0 + radius {
        // Leave a rib standing across X in [-1, 1].
        if !(x > -1.0 - radius && x < 1.0 + radius) {
            stock.stamp_linear_segment(
                &lut,
                radius,
                P3::new(x, -10.0 - radius, 0.0),
                P3::new(x, 10.0 + radius, 0.0),
                StockCutDirection::FromTop,
            );
        }
        x += radius * 0.5;
    }
    stock
}

/// Case 2. Same remaining stock, same tool, same heights: the raster finish
/// and Project Curve build the same ceiling. Before R1 the radius
/// (`radius()` against `envelope_radius_mm()`) and the fallback top
/// (`heights.top_z` against the stock top) came from two rules.
#[test]
fn raster_and_project_curve_build_the_same_ceiling_on_remaining_stock() {
    let stock = ribbed_stock();
    let tool = rs_cam_core::compute::cutter::build_cutter(&endmill_tool_config(6.0)).unwrap();
    let stock_top_z = 3.0;
    // A pinned top below the stock top: the case where the two rules split.
    let heights_top_z = 1.0;
    let raster = link_ceiling_for(
        OperationType::DropCutter,
        Some(&stock),
        &tool,
        stock_top_z,
        heights_top_z,
        None,
    )
    .expect("a finishing op with remaining stock builds a ceiling");
    let curve = link_ceiling_for(
        OperationType::ProjectCurve,
        Some(&stock),
        &tool,
        stock_top_z,
        heights_top_z,
        None,
    )
    .expect("Project Curve always builds a ceiling");
    assert!(
        std::ptr::eq(raster.stock.unwrap(), curve.stock.unwrap()),
        "both read the same remaining stock"
    );
    assert_eq!(raster.tool_radius.to_bits(), curve.tool_radius.to_bits());
    assert_eq!(
        raster.fallback_top_z.to_bits(),
        curve.fallback_top_z.to_bits()
    );
    assert_eq!(curve.tool_radius, tool.envelope_radius_mm());
    assert_eq!(
        curve.fallback_top_z, stock_top_z,
        "the stock top, not top_z"
    );
    // Every finishing op that links agrees with the raster.
    for op in [
        OperationType::Waterline,
        OperationType::Scallop,
        OperationType::Pencil,
        OperationType::UnifiedFinish,
    ] {
        let c = link_ceiling_for(op, Some(&stock), &tool, stock_top_z, heights_top_z, None)
            .unwrap_or_else(|| panic!("{op:?} builds a ceiling on remaining stock"));
        assert_eq!(
            c.tool_radius.to_bits(),
            raster.tool_radius.to_bits(),
            "{op:?}"
        );
        assert_eq!(
            c.fallback_top_z.to_bits(),
            raster.fallback_top_z.to_bits(),
            "{op:?}"
        );
    }
}

/// Case 3. Under Auto heights the resolved `top_z` IS the stock top, so the
/// old Project Curve fallback (`heights.top_z`) and the new one
/// (`max(stock top, top_z)`) are the same number, and the envelope radius of
/// a shipped tool is its `radius()`. G-OPGOLDEN holds the full toolpath of
/// every op under these heights.
#[test]
fn auto_heights_keep_the_old_ceiling_values() {
    let ctx = HeightContext {
        stock_top_z: STOCK_Z,
        ..HeightContext::simple(RETRACT_Z, 0.0)
    };
    let auto_top_z = HeightsConfig::default().resolve(&ctx).top_z;
    assert_eq!(auto_top_z, STOCK_Z);
    let tool =
        rs_cam_core::compute::cutter::build_cutter(&endmill_tool_config(TOOL_DIAMETER_MM)).unwrap();
    let ceiling = link_ceiling_for(
        OperationType::ProjectCurve,
        None,
        &tool,
        STOCK_Z,
        auto_top_z,
        None,
    )
    .expect("Project Curve always builds a ceiling");
    assert_eq!(ceiling.fallback_top_z.to_bits(), auto_top_z.to_bits());
    assert_eq!(ceiling.tool_radius.to_bits(), tool.radius().to_bits());
}

/// Case 4. On fresh stock the mesh IS the material for a finishing op, so
/// it builds no ceiling (unchanged). Project Curve builds one; an op that
/// makes no link builds none.
#[test]
fn a_finishing_op_on_fresh_stock_builds_no_ceiling() {
    let tool =
        rs_cam_core::compute::cutter::build_cutter(&endmill_tool_config(TOOL_DIAMETER_MM)).unwrap();
    for &op in OperationType::ALL {
        let ceiling = link_ceiling_for(op, None, &tool, STOCK_Z, PINNED_TOP_Z, None);
        match op.capabilities().link_ceiling {
            LinkCeilingNeed::Always => {
                let c = ceiling.unwrap_or_else(|| panic!("{op:?} needs a fresh-stock ceiling"));
                assert!(c.stock.is_none());
                assert_eq!(c.fallback_top_z, STOCK_Z, "{op:?}: the stock top");
            }
            LinkCeilingNeed::WithRemainingStock | LinkCeilingNeed::NoLink => {
                assert!(ceiling.is_none(), "{op:?} built a fresh-stock ceiling");
            }
        }
    }
    assert_eq!(
        OperationType::ProjectCurve.capabilities().link_ceiling,
        LinkCeilingNeed::Always
    );
    for op in [
        OperationType::DropCutter,
        OperationType::Waterline,
        OperationType::Scallop,
        OperationType::Pencil,
        OperationType::UnifiedFinish,
    ] {
        assert_eq!(
            op.capabilities().link_ceiling,
            LinkCeilingNeed::WithRemainingStock,
            "{op:?}"
        );
    }
}
