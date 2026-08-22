//! G-UNIFIEDCRASH sentry — a dense `unified_finish` must GENERATE, not panic.
//!
//! Observed live 2026-08-23 (`planning/airrun_2026-08-19/
//! OVERNIGHT_TUNING_2026-08-23.md`): `unified_finish` on the wanaka200
//! terrain project died with *"index out of bounds: the len is 0 but the
//! index is 1"* at `scallop_height` 0.03 / `z_step` 0.6 / `raster_stepover`
//! 0.6, and generated cleanly on the SAME project at 0.1 / 0.3 / 0.6. The
//! worker's `catch_unwind` turned it into a generation `Error`, so nothing
//! about the failure named a line.
//!
//! # What it actually was
//!
//! The MidSteep band's scallop cascade tidies every offset ring with
//! `cavalier_contours`' `Polyline::remove_redundant`, whose closed-polyline
//! tail reads `pl.at(1)` out of a result the line above it may just have
//! emptied — a raw slice index, so a ring that reduces to a POINT panics.
//! A cascade's terminal rings approach a point by construction, and
//! `scallop_height` is what sets how many of them there are and how close
//! together: 0.03 on that fixture is roughly twice the rings at half the
//! spacing of 0.1. `z_step` is exonerated — the project resolved `bottom_z`
//! to the stock top (G-UNIFIEDBOTTOMZ), so the VerySteep band laddered zero
//! levels at BOTH 0.3 and 0.6.
//!
//! The containment lives in `polygon::remove_redundant_contained`, and the
//! DETERMINISTIC witness — the three-vertex ring that makes the dependency
//! panic on demand — is the unit test beside it
//! (`a_ring_that_reduces_to_a_point_does_not_panic_g_unifiedcrash`). This
//! file is the operation-level arm: it drives the real generator through the
//! real entry point at the parameter combination that died, so a future
//! change that reintroduces an uncontained dependency panic anywhere in the
//! band pipeline fails here rather than in an operator's session.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::ResolvedHeights;
use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::compute::execute::execute_operation_annotated;
use rs_cam_core::compute::operation_configs::UnifiedFinishConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::geo::BoundingBox3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh, make_test_hemisphere};
use rs_cam_core::toolpath_spans::AnnotatedToolpath;
use std::sync::atomic::AtomicBool;

/// A hemisphere is the cheapest mixed-slope surface that exercises all three
/// bands: near-flat at the pole (raster), mid-slope on the flanks (scallop —
/// the band that owns this defect), near-vertical at the equator
/// (waterline). Same fixture `unified_finish_semantic_regions` uses.
fn hemisphere() -> (TriangleMesh, SpatialIndex) {
    let mesh = make_test_hemisphere(20.0, 16);
    let index = SpatialIndex::build(&mesh, 12.0);
    (mesh, index)
}

/// The live project's tool: a tapered ball, where `radius()` (shank) and
/// `cusp_radius()` (tip) diverge — the class the ring cascade is sized off.
fn tapered_ball_tool() -> ToolConfig {
    ToolConfig {
        diameter: 1.5,
        taper_half_angle: 10.0,
        shaft_diameter: 6.0,
        ..ToolConfig::new_default(ToolId(2), ToolType::TaperedBallNose)
    }
}

/// Generate through the SAME entry point `session::compute::generate_toolpath`
/// and the GUI worker both call.
///
/// Returns the `Result` rather than unwrapping it, so a caller can say which
/// of "panicked", "refused" and "generated" it is asserting.
fn generate(
    scallop_height: f64,
    z_step: f64,
    raster_stepover: f64,
) -> Result<AnnotatedToolpath, rs_cam_core::compute::OperationError> {
    let (mesh, index) = hemisphere();
    let tool_cfg = tapered_ball_tool();
    let tool_def = build_cutter(&tool_cfg);

    let op = OperationConfig::UnifiedFinish(UnifiedFinishConfig {
        // Coarse everywhere the defect does not live: this is a
        // does-it-survive gate, not a surface-quality one.
        tolerance: 0.5,
        sampling: 1.0,
        scallop_height,
        raster_stepover,
        z_step,
        ..UnifiedFinishConfig::default()
    });

    let bbox = mesh.bbox;
    let heights = ResolvedHeights {
        clearance_z: bbox.max.z + 10.0,
        retract_z: bbox.max.z + 5.0,
        feed_z: bbox.max.z + 1.0,
        top_z: bbox.max.z,
        bottom_z: bbox.min.z,
        top_pinned: true,
        bottom_pinned: true,
    };
    let stock_bbox = BoundingBox3 {
        min: bbox.min,
        max: bbox.max,
    };

    let cancel = AtomicBool::new(false);
    execute_operation_annotated(
        &op,
        Some(&mesh),
        Some(&index),
        None,
        &tool_def,
        &tool_cfg,
        &heights,
        &[],
        &stock_bbox,
        None,
        None,
        None,
        &cancel,
        None,
        None,
        None,
    )
}

/// The combination that died. The assertion is that the call RETURNS and
/// returns cutting geometry — a panic fails the test by unwinding, and an
/// empty toolpath would make the "it survived" claim vacuous.
#[test]
fn a_dense_scallop_band_generates_instead_of_panicking_g_unifiedcrash() {
    // scallop 0.03 / z_step 0.6 / raster 0.6 — the live combination.
    let annotated = generate(0.03, 0.6, 0.6).expect("dense arm must generate");
    assert!(
        !annotated.toolpath.moves.is_empty(),
        "a surviving generation that emitted nothing would make this \
         sentry vacuous"
    );
}

/// The control: the combination the same project generated cleanly on, and
/// the one a tuned production candidate is built from. The containment is
/// only allowed to change what happens ON a panic, and this arm never
/// panicked — so it must still generate.
#[test]
fn the_working_parameter_set_still_generates_g_unifiedcrash() {
    let annotated = generate(0.1, 0.3, 0.6).expect("control arm must generate");
    assert!(
        !annotated.toolpath.moves.is_empty(),
        "the control arm must emit cutting geometry"
    );
}

/// `scallop_height` is the dial that decides how many terminal rings the
/// cascade produces, so it is the dial that decides how often the collapsed
/// ring is reached. Held against the SAME `z_step` — varying both at once is
/// what left the live report unable to say which one mattered.
///
/// This is not a claim about surface quality; it is the non-vacuity guard
/// for the dense arm above: if a finer scallop height ever stopped producing
/// more cutting geometry, that arm would have stopped exercising the
/// cascade it exists to exercise.
#[test]
fn a_finer_scallop_height_really_does_drive_more_rings_g_unifiedcrash() {
    let dense = generate(0.03, 0.3, 0.6).expect("dense arm generates");
    let coarse = generate(0.1, 0.3, 0.6).expect("coarse arm generates");
    assert!(
        dense.toolpath.moves.len() > coarse.toolpath.moves.len(),
        "scallop 0.03 must emit more moves than 0.1 at the same z_step \
         ({} vs {})",
        dense.toolpath.moves.len(),
        coarse.toolpath.moves.len()
    );
}
