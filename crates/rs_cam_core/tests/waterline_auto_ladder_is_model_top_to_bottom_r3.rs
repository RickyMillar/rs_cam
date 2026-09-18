//! R3 / R4 sentry (Corne case, 2026-09-18) — an Auto waterline ladders the
//! model, never the bed.
//!
//! ## The defect
//!
//! `WaterlineConfig` declares `DepthSemantics::None`, so `HeightContext::
//! op_depth` is `0.0` and `HeightsConfig::resolve` gave an Auto `bottom_z`
//! of `top_z - 0 = stock top`. The adapter fed `top_z`/`bottom_z` straight
//! into the ladder, so an Auto waterline had ONE level at the stock top,
//! above the mesh, and an empty result that the empty gate called legitimate
//! rest machining. One drag on the Heights diagram then pinned `top_z` to
//! `-1.37`: one level BELOW the part, and the tool cut the bed
//! (`planning/corne_case_analysis_2026-09-18/ANALYSIS.md` §4.1-§4.3).
//!
//! ## The fix, and what this file pins
//!
//! * R3 — `HeightsConfig::resolve`: on a zero-depth context the Auto bottom
//!   is the model bottom (floored at the stock bottom). The Auto TOP stays at
//!   the stock top: the session's entry-descent pass reads `heights.top_z`
//!   as the fresh-stock ceiling, and a model-top Auto would rapid a
//!   fresh-stock rough into the overhead. A context WITH a depth keeps the
//!   F-028 arithmetic to the bit.
//! * R4 — `ops::waterline::waterline_ladder`: a level at or below the
//!   emission-frame stock bottom is dropped, and the adapter records a
//!   `WaterlineLadderFinding` that the diagnostics list renders.
//! * §4.5 — a level within 1e-3 of a horizontal facet moves 0.01 mm DOWN.
//! * The generator and the span builder read ONE ladder.
//!
//! ## Contract
//!
//! * **Kind**: GATE (default CI, synthetic, seconds).
//! * **Fixtures**: `HeightContext` literals for the resolver; a box with
//!   its top at 10.0 for the nudge; `make_test_hemisphere` through the
//!   production session path for the end-to-end arms.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::session::{generate, mesh_model, single_op_session_with, stock_over};
use common::tools::endmill_tool_config;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{HeightContext, HeightMode, HeightsConfig};
use rs_cam_core::compute::operation_configs::WaterlineConfig;
use rs_cam_core::diagnostics::{Severity, ids};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{TriangleMesh, make_test_hemisphere};
use rs_cam_core::ops::waterline::{
    FLAT_FACE_NUDGE_MM, WaterlineLadder, waterline_ladder, waterline_z_levels,
};
use rs_cam_core::session::ProjectSession;

const EPS: f64 = 1e-9;

// ── (a) The resolver ─────────────────────────────────────────────────────

/// Stock 0..23, model 0..18 — the Corne numbers.
fn corne_ctx(op_depth: f64) -> HeightContext {
    HeightContext {
        safe_z: 28.0,
        op_depth,
        stock_top_z: 23.0,
        stock_bottom_z: 0.0,
        model_top_z: Some(18.0),
        model_bottom_z: Some(0.0),
    }
}

#[test]
fn a_zero_depth_auto_bottom_is_the_model_bottom_and_the_top_stays_the_stock_top() {
    let h = HeightsConfig::default().resolve(&corne_ctx(0.0));
    assert!(
        (h.top_z - 23.0).abs() < EPS,
        "the Auto top is the stock top, whatever the depth: {}",
        h.top_z
    );
    assert!(
        (h.bottom_z - 0.0).abs() < EPS,
        "R3: the Auto bottom of a zero-depth op is the model bottom: {}",
        h.bottom_z
    );
    assert!(!h.top_pinned);
    assert!(!h.bottom_pinned);
}

#[test]
fn a_zero_depth_auto_bottom_never_sits_below_the_stock_bottom() {
    // Model 2..18 above a stock floor at 0: the model bottom wins.
    let ctx = HeightContext {
        model_bottom_z: Some(2.0),
        ..corne_ctx(0.0)
    };
    let h = HeightsConfig::default().resolve(&ctx);
    assert!((h.bottom_z - 2.0).abs() < EPS, "{}", h.bottom_z);

    // Model reaching BELOW the stock (a through part on a thin board): the
    // stock bottom floors it.
    let ctx = HeightContext {
        model_bottom_z: Some(-4.0),
        ..corne_ctx(0.0)
    };
    let h = HeightsConfig::default().resolve(&ctx);
    assert!((h.bottom_z - 0.0).abs() < EPS, "{}", h.bottom_z);

    // No model at all: the stock bottom.
    let ctx = HeightContext {
        model_top_z: None,
        model_bottom_z: None,
        stock_bottom_z: -3.0,
        ..corne_ctx(0.0)
    };
    let h = HeightsConfig::default().resolve(&ctx);
    assert!((h.bottom_z + 3.0).abs() < EPS, "{}", h.bottom_z);
}

#[test]
fn a_context_with_a_depth_keeps_the_f028_arithmetic() {
    let h = HeightsConfig::default().resolve(&corne_ctx(5.0));
    assert!((h.top_z - 23.0).abs() < EPS, "{}", h.top_z);
    assert!(
        (h.bottom_z - 18.0).abs() < EPS,
        "2.5D: bottom = top - depth, unchanged by R3: {}",
        h.bottom_z
    );
    // A pinned top still anchors the depth.
    let pinned = HeightsConfig {
        top_z: HeightMode::Manual(20.0),
        ..HeightsConfig::default()
    }
    .resolve(&corne_ctx(5.0));
    assert!((pinned.bottom_z - 15.0).abs() < EPS, "{}", pinned.bottom_z);
}

#[test]
fn a_pinned_bottom_on_a_zero_depth_op_is_left_alone() {
    let h = HeightsConfig {
        bottom_z: HeightMode::Manual(0.5),
        ..HeightsConfig::default()
    }
    .resolve(&corne_ctx(0.0));
    assert!((h.bottom_z - 0.5).abs() < EPS, "{}", h.bottom_z);
    assert!(h.bottom_pinned);
}

// ── (c) The ladder builder ───────────────────────────────────────────────

/// A closed box `[-half, half]² × [z0, z1]`, with horizontal facets at both
/// `z0` and `z1`.
fn box_mesh(half: f64, z0: f64, z1: f64) -> TriangleMesh {
    let v = vec![
        P3::new(-half, -half, z1),
        P3::new(half, -half, z1),
        P3::new(half, half, z1),
        P3::new(-half, half, z1),
        P3::new(-half, -half, z0),
        P3::new(half, -half, z0),
        P3::new(half, half, z0),
        P3::new(-half, half, z0),
    ];
    let t = vec![
        [0, 1, 2],
        [0, 2, 3],
        [4, 6, 5],
        [4, 7, 6],
        [0, 4, 5],
        [0, 5, 1],
        [1, 5, 6],
        [1, 6, 2],
        [2, 6, 7],
        [2, 7, 3],
        [3, 7, 4],
        [3, 4, 0],
    ];
    TriangleMesh::from_raw(v, t)
}

#[test]
fn a_level_on_a_flat_face_moves_down_by_a_hundredth() {
    // Box top at 10.0, bottom at 4.0, on a stock whose bottom is at 0.0.
    let mesh = box_mesh(10.0, 4.0, 10.0);
    let WaterlineLadder {
        levels,
        planned_levels,
        nudged_levels,
        dropped_below_stock,
    } = waterline_ladder(&mesh, 10.0, 4.0, 1.0, 0.0);
    assert_eq!(planned_levels, 7, "10, 9, …, 4");
    assert_eq!(
        nudged_levels, 2,
        "the top face at 10.0 and the bottom face at 4.0"
    );
    assert_eq!(
        dropped_below_stock, 0,
        "nothing sits at the stock bottom (0.0)"
    );
    assert!(
        (levels[0] - (10.0 - FLAT_FACE_NUDGE_MM)).abs() < EPS,
        "the level at 10.0 becomes 9.99: {}",
        levels[0]
    );
    assert!(
        (levels[1] - 9.0).abs() < EPS,
        "an off-face level is untouched"
    );
    assert!(
        (levels[6] - (4.0 - FLAT_FACE_NUDGE_MM)).abs() < EPS,
        "the level at 4.0 becomes 3.99: {}",
        levels[6]
    );
    // Never UP: no delivered level is above its planned level.
    for (planned, delivered) in waterline_z_levels(10.0, 4.0, 1.0).iter().zip(&levels) {
        assert!(delivered <= planned, "{delivered} > {planned}");
    }
}

#[test]
fn a_level_at_or_below_the_stock_bottom_is_dropped() {
    // Hemisphere r=10 on a stock 0..12: the last level (0.0) IS the bed.
    let mesh = make_test_hemisphere(10.0, 6);
    let ladder = waterline_ladder(&mesh, 12.0, 0.0, 1.0, 0.0);
    assert_eq!(ladder.planned_levels, 13);
    assert_eq!(ladder.dropped_below_stock, 1, "the level at Z 0.0");
    assert_eq!(
        ladder.nudged_levels, 0,
        "a hemisphere has no horizontal facet"
    );
    assert_eq!(ladder.levels.len(), 12);
    assert!(
        ladder.levels.iter().all(|&z| z > 0.0),
        "{:?}",
        ladder.levels
    );

    // The same model on a thicker stock (bottom at -5): the model-bottom
    // level at 0.0 is above the bed and survives.
    let ladder = waterline_ladder(&mesh, 12.0, 0.0, 1.0, -5.0);
    assert_eq!(ladder.dropped_below_stock, 0);
    assert_eq!(ladder.levels.len(), 13);

    // The as-found pin, both heights at -1.37: one level below the part,
    // dropped; nothing is cut.
    let ladder = waterline_ladder(&mesh, -1.37, -1.37, 1.0, 0.0);
    assert_eq!(ladder.planned_levels, 1);
    assert_eq!(ladder.dropped_below_stock, 1);
    assert!(ladder.levels.is_empty());

    // A pinned top below an Auto (model) bottom: an inverted range, empty.
    let ladder = waterline_ladder(&mesh, -1.37, 0.0, 1.0, 0.0);
    assert_eq!(ladder.planned_levels, 0);
    assert!(ladder.levels.is_empty());
}

#[test]
fn the_nudge_runs_before_the_floor() {
    // A flat model bottom that coincides with the stock bottom: the level
    // nudges to -0.01 and the floor then drops it — the bed is never cut.
    let mesh = box_mesh(10.0, 0.0, 10.0);
    let ladder = waterline_ladder(&mesh, 10.0, 0.0, 5.0, 0.0);
    assert_eq!(ladder.planned_levels, 3, "10, 5, 0");
    assert_eq!(ladder.nudged_levels, 2, "the top and the bottom face");
    assert_eq!(ladder.dropped_below_stock, 1);
    assert_eq!(ladder.levels.len(), 2);
}

// ── (b) End to end through the production session path ──────────────────

const HEMI_RADIUS_MM: f64 = 10.0;
const STOCK_HALF_MM: f64 = 12.0;
const STOCK_HEIGHT_MM: f64 = 12.0;

fn waterline_session(heights: HeightsConfig) -> ProjectSession {
    let mut session = single_op_session_with(
        stock_over(STOCK_HALF_MM, STOCK_HEIGHT_MM),
        endmill_tool_config(4.0),
        mesh_model(make_test_hemisphere(HEMI_RADIUS_MM, 6), "hemisphere"),
        "Waterline",
        OperationConfig::Waterline(WaterlineConfig {
            z_step: 1.0,
            sampling: 0.5,
            ..WaterlineConfig::default()
        }),
        |cfg| cfg.heights = heights,
    );
    generate(&mut session, 0);
    session
}

#[test]
fn an_auto_waterline_ladders_the_model_and_records_a_clean_finding() {
    let session = waterline_session(HeightsConfig::default());
    let result = session.get_result(0).expect("generated result");
    let f = result
        .stats
        .waterline_ladder
        .expect("the waterline adapter records its ladder on every run");
    // Auto top = stock top (12), Auto bottom = model bottom (0) = stock
    // bottom; the level at 0.0 is the bed and is dropped.
    assert!(
        (f.requested_top_z_mm - STOCK_HEIGHT_MM).abs() < EPS,
        "{f:?}"
    );
    assert!((f.requested_bottom_z_mm - 0.0).abs() < EPS, "{f:?}");
    assert_eq!(f.planned_levels, 13, "{f:?}");
    assert_eq!(f.dropped_below_stock, 1, "{f:?}");
    assert_eq!(f.delivered_levels, 12, "{f:?}");
    assert!(f.delivered_levels >= 2);
    assert!(f.floored(), "one level sat on the bed");
    assert!(!f.degenerate());

    // The result cuts: levels 10..1 cross the hemisphere.
    assert!(
        result.stats.cutting_distance > 0.0,
        "an Auto waterline must cut the model, not one level above it"
    );
    // No emitted move at or below the stock bottom.
    let below = result
        .annotated()
        .toolpath
        .moves
        .iter()
        .filter(|m| m.move_type.is_cutting() && m.target.z <= 0.0)
        .count();
    assert_eq!(below, 0, "no cutting move may reach the bed");

    // The floor is a Caution on the diagnostics list; the ladder is not
    // degenerate, so that id is absent.
    let diags = session
        .diagnose_toolpath_with_trace(0, None)
        .expect("diagnose");
    let floored = diags
        .iter()
        .find(|d| d.id.as_str() == ids::GEOM_WATERLINE_LEVELS_BELOW_STOCK)
        .expect("R4: a dropped level raises geom.waterline_levels_below_stock");
    assert_eq!(floored.severity, Severity::Caution);
    assert!(floored.message.contains("DROPPED"), "{}", floored.message);
    assert!(
        !diags
            .iter()
            .any(|d| d.id.as_str() == ids::GEOM_WATERLINE_LADDER_EMPTY),
        "twelve levels reached the cutter"
    );

    // Narration carries the same sentence.
    let narration = session.narrate_toolpath(0).expect("narrate");
    assert!(
        narration.contains("Waterline ladder FLOORED:"),
        "{narration}"
    );
}

#[test]
fn a_top_pinned_below_the_stock_bottom_gives_an_empty_ladder_and_the_finding() {
    // The as-found Corne pin: top -1.37, bottom Auto (now the model bottom,
    // 0.0) — an inverted range.
    let session = waterline_session(HeightsConfig {
        top_z: HeightMode::Manual(-1.37),
        ..HeightsConfig::default()
    });
    let result = session.get_result(0).expect("generated result");
    let f = result.stats.waterline_ladder.expect("recorded");
    assert_eq!(f.planned_levels, 0, "{f:?}");
    assert_eq!(f.delivered_levels, 0, "{f:?}");
    assert!(f.degenerate());
    assert!(
        result.stats.cutting_distance.abs() < EPS,
        "nothing is cut below the part"
    );
    let diags = session
        .diagnose_toolpath_with_trace(0, None)
        .expect("diagnose");
    assert!(
        diags
            .iter()
            .any(|d| d.id.as_str() == ids::GEOM_WATERLINE_LADDER_EMPTY),
        "R3: an empty ladder is reported, not passed as legitimate silence"
    );

    // Both heights pinned at -1.37: ONE level below the part, dropped.
    let session = waterline_session(HeightsConfig {
        top_z: HeightMode::Manual(-1.37),
        bottom_z: HeightMode::Manual(-1.37),
        ..HeightsConfig::default()
    });
    let result = session.get_result(0).expect("generated result");
    let f = result.stats.waterline_ladder.expect("recorded");
    assert_eq!(f.planned_levels, 1, "{f:?}");
    assert_eq!(f.dropped_below_stock, 1, "{f:?}");
    assert_eq!(f.delivered_levels, 0, "{f:?}");
    assert!(
        result.stats.cutting_distance.abs() < EPS,
        "the -1.37 pin never cuts the bed again"
    );
    let diags = session
        .diagnose_toolpath_with_trace(0, None)
        .expect("diagnose");
    for id in [
        ids::GEOM_WATERLINE_LEVELS_BELOW_STOCK,
        ids::GEOM_WATERLINE_LADDER_EMPTY,
    ] {
        assert!(
            diags.iter().any(|d| d.id.as_str() == id),
            "{id} must be raised"
        );
    }
}
