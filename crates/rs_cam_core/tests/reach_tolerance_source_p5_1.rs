//! F2 / F5 (P5.1, 2026-09-08) — the reach map's BAR, and its GRID.
//!
//! # F2, the bar
//!
//! `drop_cutter` declares a stepover, not a scallop height, so
//! `OperationConfig::scallop_height()` answers `None` for it and every raster
//! finish fell to the 0.05 mm default. On the operator's wanaka pass — a
//! tapered ball of tip radius 2.0 mm at a 1.5 mm stepover — that raster's own
//! cusp is `R − sqrt(R² − (s/2)²)` = **0.146 mm**, so the map was judging the
//! surface against a bar three times finer than the pass spacing can ever
//! deliver, and painting the difference red.
//!
//! The size of the mistake was measured independently of this repo's code, by
//! rasterising the terrain STL and taking the ball closing on a 0.15 mm
//! lattice: **55.0 %** of the area missed at 0.05 mm against **39.5 %** at
//! 0.146 mm. Same surface, same tool; only the second number answers a
//! question the operator can act on.
//!
//! # F5, the grid
//!
//! `ReachMapParams::for_cutter` used to bisect for the coarsest cell whose
//! plane-only sampling floor stayed under half the tolerance. Three probes of
//! one tool at three tolerances therefore came back on cells 0.645, 0.75 and
//! 0.625 mm — three percentages that were meant to be compared, on three
//! different grids. The cell now follows the TOOL and the model; the
//! tolerance classifies on it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use common::tools::{ball_tool_config, endmill_tool_config, tapered_ball_tool_config};
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::operation_configs::{DropCutterConfig, ScallopConfig};
use rs_cam_core::compute::tool_config::ToolConfig;
use rs_cam_core::reach_map::{DEFAULT_REACH_TOLERANCE_MM, ReachMapParams, ReachToleranceSource};
use rs_cam_core::session::ProjectSession;
use rs_cam_core::tool::{BallEndmill, MillingCutter, TaperedBallEndmill};

/// The shipped wanaka finishing pass: Ø4 tapered ball (tip radius 2.0 mm) at
/// a 1.5 mm raster stepover.
const WANAKA_TIP_R_MM: f64 = 2.0;
const WANAKA_STEPOVER_MM: f64 = 1.5;

fn drop_cutter(stepover: f64, scallop_height: Option<f64>) -> OperationConfig {
    OperationConfig::DropCutter(DropCutterConfig {
        stepover,
        scallop_height,
        ..DropCutterConfig::default()
    })
}

/// A one-toolpath session over a small mesh, so `reach_tolerance_for` has a
/// real tool record to read the tip geometry from.
fn session(tool: ToolConfig, op: OperationConfig) -> ProjectSession {
    common::session::single_op_session(
        common::session::stock_under(8.0, 6.0),
        tool,
        common::session::mesh_model(common::meshes::height_field(8.0, 0.5, |_, _| 0.0), "pad"),
        "op",
        op,
    )
}

// ── F2: the bar ────────────────────────────────────────────────────────

/// The wanaka case, in closed form. A `drop_cutter` on a spherical tip takes
/// its own raster cusp as the bar, and names it.
#[test]
fn a_raster_finish_takes_the_cusp_of_its_own_stepover() {
    let session = session(
        tapered_ball_tool_config(2.0 * WANAKA_TIP_R_MM, 3.0, 6.0),
        drop_cutter(WANAKA_STEPOVER_MM, None),
    );
    let (tolerance, source) = session.reach_tolerance_for(0);

    let expected =
        rs_cam_core::scallop_math::scallop_height_flat(WANAKA_TIP_R_MM, WANAKA_STEPOVER_MM);
    assert!(
        (expected - 0.146).abs() < 0.001,
        "the fixture must reproduce the reported 0.146 mm; the closed form gives \
         {expected:.5}"
    );
    assert!(
        (tolerance - expected).abs() < 1e-9,
        "a raster finish's bar is the cusp of its own stepover: expected \
         {expected:.5} mm, got {tolerance:.5} mm"
    );
    match source {
        ReachToleranceSource::CuspOfStepover {
            stepover_mm,
            tip_radius_mm,
        } => {
            assert!((stepover_mm - WANAKA_STEPOVER_MM).abs() < 1e-9);
            // The TIP sphere, not the Ø6 shank: reading the envelope here
            // would report a cusp three times too small on this tool.
            assert!(
                (tip_radius_mm - WANAKA_TIP_R_MM).abs() < 1e-9,
                "the cusp must come from the TIP sphere ({WANAKA_TIP_R_MM} mm), \
                 not the shank; got {tip_radius_mm}"
            );
        }
        other => panic!("expected a derived cusp, got {other:?}"),
    }
    // The operator-facing sentence, pinned so a rename cannot silently drop
    // the provenance the whole finding was about.
    let described = source.describe(tolerance);
    assert!(
        described.contains("cusp of stepover") && described.contains("1.500"),
        "the source line must name the stepover it derived from: {described}"
    );
}

/// A FLAT tip leaves no cusp between passes, however coarse the stepover, so
/// it keeps the default.
///
/// `cusp_radius_mm()` cannot answer this question — on a flat endmill it
/// returns the full radius, which would invent a 3 mm-radius "tip sphere" and
/// hand a Ø6 flat a 0.19 mm bar off a 1.5 mm stepover. The geometry hint is
/// read instead, matching `DropCutterConfig::scallop_height`'s own doc ("no
/// effect on flat/bull tools").
#[test]
fn a_flat_tip_keeps_the_default_however_coarse_its_stepover() {
    let session = session(endmill_tool_config(6.0), drop_cutter(3.0, None));
    let (tolerance, source) = session.reach_tolerance_for(0);
    assert!(
        (tolerance - DEFAULT_REACH_TOLERANCE_MM).abs() < 1e-9,
        "a flat tip has no raster cusp, so the bar stays the default; got \
         {tolerance:.5} mm"
    );
    assert_eq!(source, ReachToleranceSource::Default);

    // The trap this guards: `cusp_radius_mm()` on the same tool is 3.0, and a
    // cusp derived from THAT would be a plausible-looking 0.19 mm.
    let flat_cusp_radius = rs_cam_core::tool::FlatEndmill::new(6.0, 25.0).cusp_radius_mm();
    let wrong = rs_cam_core::scallop_math::scallop_height_flat(flat_cusp_radius, 3.0);
    assert!(
        wrong > 0.15,
        "the fixture must be one where the WRONG rule gives a distinctly \
         different answer, or it proves nothing; got {wrong:.5}"
    );
}

/// A declared scallop height outranks the derived cusp — both on an op whose
/// whole dial is a scallop height, and on a `drop_cutter` carrying the
/// Suggest dial.
#[test]
fn a_declared_scallop_height_outranks_the_derived_cusp() {
    let declared = session(
        ball_tool_config(4.0),
        OperationConfig::Scallop(ScallopConfig {
            scallop_height: 0.08,
            ..ScallopConfig::default()
        }),
    );
    let (tolerance, source) = declared.reach_tolerance_for(0);
    assert!((tolerance - 0.08).abs() < 1e-9, "got {tolerance:.5}");
    assert_eq!(source, ReachToleranceSource::DeclaredScallopHeight);

    // A drop_cutter with BOTH: the declared height wins, and it is not the
    // 0.146 the stepover would have derived.
    let both = session(
        tapered_ball_tool_config(2.0 * WANAKA_TIP_R_MM, 3.0, 6.0),
        drop_cutter(WANAKA_STEPOVER_MM, Some(0.02)),
    );
    let (tolerance, source) = both.reach_tolerance_for(0);
    assert!((tolerance - 0.02).abs() < 1e-9, "got {tolerance:.5}");
    assert_eq!(source, ReachToleranceSource::DeclaredScallopHeight);
}

/// A stepover fine enough to beat the default does not LOWER the bar below
/// it.
///
/// The derived cusp is a floor on what the raster can deliver, not a
/// tightening of the finish bar the default names: an operator who steps over
/// at 0.3 mm on a R2 tip has a 0.006 mm cusp, and judging the map against
/// that would paint every part red for reasons no tool change can fix.
#[test]
fn a_fine_stepover_does_not_tighten_the_bar_below_the_default() {
    let session = session(
        tapered_ball_tool_config(2.0 * WANAKA_TIP_R_MM, 3.0, 6.0),
        drop_cutter(0.3, None),
    );
    let cusp = rs_cam_core::scallop_math::scallop_height_flat(WANAKA_TIP_R_MM, 0.3);
    assert!(
        cusp < DEFAULT_REACH_TOLERANCE_MM,
        "the fixture must have a cusp under the default; got {cusp:.5}"
    );
    let (tolerance, source) = session.reach_tolerance_for(0);
    assert!((tolerance - DEFAULT_REACH_TOLERANCE_MM).abs() < 1e-9);
    assert_eq!(source, ReachToleranceSource::Default);
}

/// A caller override outranks everything and says so.
#[test]
fn a_caller_override_outranks_everything() {
    let session = session(
        tapered_ball_tool_config(2.0 * WANAKA_TIP_R_MM, 3.0, 6.0),
        drop_cutter(WANAKA_STEPOVER_MM, None),
    );
    let (tolerance, source) = session.reach_tolerance_with_override(0, Some(0.3));
    assert!((tolerance - 0.3).abs() < 1e-9);
    assert_eq!(source, ReachToleranceSource::CallerOverride);

    // A non-finite or non-positive override is not an override.
    for bad in [0.0, -1.0, f64::NAN] {
        let (tolerance, source) = session.reach_tolerance_with_override(0, Some(bad));
        assert_eq!(
            source,
            ReachToleranceSource::CuspOfStepover {
                stepover_mm: WANAKA_STEPOVER_MM,
                tip_radius_mm: WANAKA_TIP_R_MM,
            },
            "an override of {bad} must fall through to the derived cusp"
        );
        assert!(tolerance > DEFAULT_REACH_TOLERANCE_MM);
    }
}

/// Which operations own a LATERAL raster stepover, pinned as a list.
///
/// Derived from nothing: it is a per-operation ruling, because four of the
/// ten reach-map operations publish a `stepover()` that is not a surface
/// raster spacing. A future op added to the reach-map set must decide this
/// deliberately, which is what a pinned list forces.
#[test]
fn exactly_four_reach_map_operations_own_a_lateral_raster_stepover() {
    let mut actual: Vec<&str> = OperationType::ALL
        .iter()
        .filter(|op| op.lateral_raster_stepover())
        .map(|op| op.kind_str())
        .collect();
    actual.sort_unstable();
    assert_eq!(
        actual,
        vec![
            "drop_cutter",
            "horizontal_finish",
            "spiral_finish",
            "steep_shallow"
        ]
    );

    // Every one of them must also be an op the reach map speaks about, or the
    // derivation is dead code.
    for op in OperationType::ALL
        .iter()
        .filter(|op| op.lateral_raster_stepover())
    {
        assert!(
            op.supports_reach_map(),
            "{} owns a raster stepover but carries no reach map",
            op.kind_str()
        );
    }

    // The four exclusions, named so a reader sees the reason rather than a
    // count. Waterline's step is vertical; pencil's is an offset fan around
    // one seam; radial's is angular, so the spacing varies from hub to rim;
    // ramp_finish publishes no stepover at all.
    for op in [
        OperationType::Waterline,
        OperationType::Pencil,
        OperationType::RadialFinish,
        OperationType::RampFinish,
    ] {
        assert!(
            !op.lateral_raster_stepover(),
            "{} must not derive a raster cusp",
            op.kind_str()
        );
        assert!(
            op.supports_reach_map(),
            "{} is still a reach-map op",
            op.kind_str()
        );
    }
}

/// Waterline carries a reach map and keeps the default bar: its `z_step` is a
/// vertical drop between contours, and it publishes no `stepover()`.
#[test]
fn waterline_keeps_the_default_bar() {
    let session = session(
        ball_tool_config(4.0),
        OperationConfig::Waterline(
            rs_cam_core::compute::operation_configs::WaterlineConfig::default(),
        ),
    );
    let (tolerance, source) = session.reach_tolerance_for(0);
    assert!((tolerance - DEFAULT_REACH_TOLERANCE_MM).abs() < 1e-9);
    assert_eq!(source, ReachToleranceSource::Default);
}

// ── F5: the grid ───────────────────────────────────────────────────────

/// The cell does not move with the tolerance — the operator's own F5 report.
///
/// Three probes of one tool at three bars came back on cells 0.645, 0.75 and
/// 0.625 mm, so three percentages that were meant to be compared sat on three
/// grids. The tolerance now classifies on a grid the TOOL fixes.
#[test]
fn the_cell_does_not_move_with_the_tolerance() {
    let taper = TaperedBallEndmill::new(2.0 * WANAKA_TIP_R_MM, 3.0, 6.0, 25.0);
    let ball = BallEndmill::new(4.0, 25.0);
    let fine = BallEndmill::new(1.0, 25.0);
    for cutter in [
        &taper as &dyn MillingCutter,
        &ball as &dyn MillingCutter,
        &fine as &dyn MillingCutter,
    ] {
        let cells: Vec<f64> = [0.01, 0.05, 0.146, 0.3, 1.0]
            .iter()
            .map(|t| ReachMapParams::for_cutter(cutter, *t).cell_mm)
            .collect();
        let first = cells[0];
        assert!(
            cells.iter().all(|c| (c - first).abs() < 1e-12),
            "the cell must not move with the tolerance; a cusp radius of \
             {:.3} mm gave {cells:?}",
            cutter.cusp_radius_mm()
        );
    }
}

/// The cell IS the tool's tip scale, so two different tools still get two
/// different grids — the half of the old rule that was right.
#[test]
fn the_cell_still_follows_the_tools_tip_sphere() {
    let coarse = BallEndmill::new(4.0, 25.0);
    let fine = BallEndmill::new(0.8, 25.0);
    let coarse_cell = ReachMapParams::for_cutter(&coarse, 0.05).cell_mm;
    let fine_cell = ReachMapParams::for_cutter(&fine, 0.05).cell_mm;
    assert!(
        fine_cell < coarse_cell,
        "a finer tool must not get a coarser grid: R{:.2} took {fine_cell:.4} mm \
         and R{:.2} took {coarse_cell:.4} mm",
        fine.cusp_radius_mm(),
        coarse.cusp_radius_mm()
    );
    // And on a tapered ball it is the TIP, not the shank: the shipped taper
    // reports `radius() == 3.0` and a cell read off that would hand the
    // finest tool in the library the coarsest grid.
    let taper = common::tools::wanaka_taper();
    let taper_cell = ReachMapParams::for_cutter(&taper, 0.05).cell_mm;
    let shank_ball = BallEndmill::new(2.0 * taper.radius(), 25.0);
    let shank_cell = ReachMapParams::for_cutter(&shank_ball, 0.05).cell_mm;
    assert!(
        taper_cell < shank_cell,
        "the taper's Ø1 TIP must set its cell, not its Ø6 shank: got \
         {taper_cell:.4} mm against the shank-sized {shank_cell:.4} mm"
    );
}
