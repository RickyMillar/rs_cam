//! Sentry: a TAPERED ball must be classified at its TIP scale, not its
//! shaft scale.
//!
//! `TaperedBallEndmill::diameter()` deliberately reports the SHAFT diameter
//! ("effective cutting diameter at widest point") because that is the
//! clearance-relevant width. Every feature-scale quantity derived from
//! `radius()` was therefore 6× too large in length and 36× in area for a
//! Ø1 tip on a Ø6 shank, which on real terrain closed and absorbed every
//! steep ribbon before it could be planned (design doc §14q).
//!
//! An independent audit (§14t) found the existing gates could not have
//! caught this: all 56 parameter sweeps and every changed unit-test call
//! site use BALL cutters, where `cusp_radius() == radius()` makes the fix
//! behaviourally inert. This file is the missing coverage — synthetic and
//! fast, so it runs in the default gate rather than behind `#[ignore]`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::finish_planner::{FinishBand, FinishPlannerParams, decompose_surface};
use rs_cam_core::finish_setup::build_classification_surface_with_cancel;
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::tool::{BallEndmill, MillingCutter, TaperedBallEndmill, ToolDefinition};

/// The tool this project actually finishes with: Ø1 tip, Ø6 shaft.
fn taper() -> TaperedBallEndmill {
    TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0)
}

/// 1. The two radii must disagree by exactly the shaft/tip ratio — and must
///    keep disagreeing through the `ToolDefinition` wrapper, which is what
///    the production path actually holds.
#[test]
fn tapered_ball_reports_shaft_radius_and_tip_cusp_radius() {
    let t = taper();
    assert!(
        (t.radius() - 3.0).abs() < 1e-9,
        "radius() must stay the SHAFT radius (clearance); got {}",
        t.radius()
    );
    assert!(
        (t.cusp_radius() - 0.5).abs() < 1e-9,
        "cusp_radius() must be the TIP sphere; got {}",
        t.cusp_radius()
    );

    // `ToolDefinition` delegates `diameter()`/`geometry_hint()`; if that
    // delegation ever breaks, the default trait impl silently falls back to
    // `radius()` and the whole fix evaporates at the only layer that ships.
    let def = ToolDefinition::new(Box::new(taper()), 6.0, 30.0, 25.0, 40.0, 2, ToolMaterial::Carbide);
    assert!((def.radius() - 3.0).abs() < 1e-9);
    assert!(
        (def.cusp_radius() - 0.5).abs() < 1e-9,
        "ToolDefinition must not lose the tip radius; got {}",
        def.cusp_radius()
    );
}

/// 5. A ball nose must be completely unaffected — `diameter()` is honest
///    there, so both radii coincide. This is the claim that bounds the
///    blast radius of the fix to tapered tools only (audit Q6).
#[test]
fn ball_nose_cusp_radius_equals_radius() {
    let ball = BallEndmill::new(6.0, 25.0);
    assert!((ball.radius() - 3.0).abs() < 1e-9);
    assert!(
        (ball.cusp_radius() - ball.radius()).abs() < 1e-12,
        "ball nose must see no change: radius {} vs cusp {}",
        ball.radius(),
        ball.cusp_radius()
    );
    let def = ToolDefinition::new(
        Box::new(BallEndmill::new(6.0, 25.0)),
        6.0,
        30.0,
        25.0,
        40.0,
        2,
        ToolMaterial::Carbide,
    );
    assert!((def.cusp_radius() - def.radius()).abs() < 1e-12);
}

/// A narrow steep ribbon: a flat plateau split by a V-groove whose walls
/// are ~76°. The groove is 2 mm wide — wider than the 0.5 mm tip, far
/// narrower than the 3 mm shaft, which is exactly the band the shaft-scale
/// dials used to erase.
fn ribbon_mesh() -> TriangleMesh {
    // Cross-section (Y-invariant), X from 0..20, groove centred at x=10:
    //   plateau z=0 up to x=9, down to z=-4 at x=10, back to z=0 at x=11.
    // Wall slope = atan(1.0 / 4.0) from vertical => ~76° from horizontal.
    let profile = [
        (0.0_f64, 0.0_f64),
        (9.0, 0.0),
        (10.0, -4.0),
        (11.0, 0.0),
        (20.0, 0.0),
    ];
    let (y0, y1) = (0.0_f64, 20.0_f64);
    let mut vertices = Vec::new();
    for (x, z) in profile {
        vertices.push(P3::new(x, y0, z));
        vertices.push(P3::new(x, y1, z));
    }
    let mut triangles = Vec::new();
    for i in 0..profile.len() - 1 {
        let (a, b) = (2 * i as u32, 2 * i as u32 + 2);
        let (c, d) = (2 * i as u32 + 3, 2 * i as u32 + 1);
        // Wound so both faces point +Z (verified against the flat plateau
        // span, where the cross product comes out (0, 0, +180)).
        triangles.push([a, b, c]);
        triangles.push([a, c, d]);
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// 2. Classification must sample at the TIP scale (cusp/4 = 0.125 mm) while
///    still padding the grid by the full physical sweep (the 3 mm shaft) —
///    the two halves of the rule this fix rests on. A regression that
///    "simplifies" both onto one radius breaks exactly one of these.
#[test]
fn classification_grid_uses_tip_cell_size_and_shaft_padding() {
    let mesh = ribbon_mesh();
    let index = SpatialIndex::build(&mesh, 10.0);
    let t = taper();
    let cancel = || false;
    let surface = build_classification_surface_with_cancel(&mesh, &index, &t, 0.01, &cancel)
        .expect("classification surface");

    assert!(
        (surface.cell_size() - 0.125).abs() < 1e-9,
        "cell size must follow the TIP (cusp/4 = 0.125), not the shaft \
         (radius/4 = 0.75); got {}",
        surface.cell_size()
    );
    // Padding is physical extent: the tool really does sweep a 3 mm radius,
    // so the grid must start a full shaft radius outside the mesh.
    let pad = mesh.bbox.min.x - surface.heightmap.origin_x;
    assert!(
        (pad - t.radius()).abs() < 1e-6,
        "padding must keep the FULL radius (3.0); got {pad}"
    );
}

/// 3. The ribbon must survive decomposition as non-shallow. Before the fix
///    the 144 mm² area floor and 1.5 mm close erased it outright: this
///    groove's steep walls total well under 144 mm² projected.
#[test]
fn steep_ribbon_survives_decomposition_under_a_tapered_tool() {
    let mesh = ribbon_mesh();
    let index = SpatialIndex::build(&mesh, 10.0);
    let t = taper();
    let cancel = || false;
    let surface = build_classification_surface_with_cancel(&mesh, &index, &t, 0.01, &cancel)
        .expect("classification surface");

    let cusp = t.cusp_radius();
    let params = FinishPlannerParams::for_tool(cusp);
    let planned = decompose_surface(&surface, &[], cusp, &params);

    let non_shallow: f64 = planned
        .regions
        .iter()
        .filter(|r| r.band != FinishBand::Shallow)
        .map(|r| r.polygon.area())
        .sum();
    assert!(
        non_shallow > 1.0,
        "a 76° groove must produce SOME non-shallow region; got {non_shallow:.3} mm² \
         across {} regions",
        planned.regions.len()
    );

    // The shaft-scale dials must demonstrably fail on the SAME surface, or
    // this test would pass even with the fix reverted — precisely how the 56
    // ball-cutter sweeps missed the defect in the first place.
    //
    // NOTE the direction is fixture-dependent and area alone is NOT a safe
    // invariant. Here the 144 mm² floor swallows the groove outright (its
    // steep walls project to ~40 mm²), so shaft dials lose the region
    // entirely. On wanaka's fine grid the SAME dials instead produce MORE
    // VerySteep area than the tip dials (423 mm² in 1 region vs 313 mm² in
    // 10) because the 1.5 mm close MERGES neighbouring ribbons into one
    // blob rather than erasing them (§14t). Region COUNT is the invariant
    // that holds in both directions: coarse dials always destroy structure.
    let shaft_params = FinishPlannerParams::for_tool(t.radius());
    let shaft_planned = decompose_surface(&surface, &[], t.radius(), &shaft_params);
    let shaft_regions = shaft_planned
        .regions
        .iter()
        .filter(|r| r.band != FinishBand::Shallow)
        .count();
    let tip_regions = planned
        .regions
        .iter()
        .filter(|r| r.band != FinishBand::Shallow)
        .count();
    assert!(
        shaft_regions < tip_regions,
        "shaft-derived dials must destroy steep STRUCTURE relative to \
         tip-derived ones, else this sentry cannot detect a revert: \
         shaft {shaft_regions} regions vs tip {tip_regions}"
    );
}
