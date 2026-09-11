//! O-A2 — a `PlannedTierRegions` boundary actually confines the op it is on.
//!
//! # The fixture
//!
//! A flat plate with one spherical bowl of radius 1.6 mm. The ladder is a Ø4
//! ball (R2.0) over a Ø2 ball (R1.0), so the geometry separates the two
//! answers by construction rather than by tuning:
//!
//! | where | R2.0 coarse | R1.0 fine | residual |
//! |---|---|---|---|
//! | the plane | tip rests at z = 0 | tip rests at z = 0 | 0 |
//! | the bowl | cannot enter (r_bowl < R) | reaches deeper | large |
//!
//! At a 0.05 mm tolerance the plane is tier 0 and the bowl is tier 1, so a
//! fine-tier op bounded to tier 1 must cut in the bowl and nowhere the
//! islands do not reach.
//!
//! Note the map does **no boundary erosion** — a coarse tool hanging off the
//! plate rim reads a false-high residual, so the rim is fine territory too
//! (`tier_map`'s module doc says so, and `rim_erosion_mm` defaults off). That
//! is fine here: the claim is *inside the islands*, whatever they turn out to
//! be, not *inside the bowl*.
//!
//! # What is asserted, and on what
//!
//! **Stored motion.** Every emitted CUTTING move's XY must lie inside the
//! tier's `machining` polygons. Not narration, not the plan, not the report —
//! the moves, which are the only thing the machine will execute.
//!
//! The oracle is the same public API the resolver uses
//! ([`compute_tier_map`] + [`extract_tier_islands`]) run over an identically
//! constructed mesh. It is a re-derivation on purpose: it tests that the
//! session's resolution reaches the emitter, not that one function equals
//! itself.
//!
//! The epsilon is one planning cell. The boundary clip computes intersection
//! points that lie exactly ON the polygon edge, where point-in-polygon is
//! ambiguous, and the emitter's grid is not the planner's — so a move may sit
//! a fraction of a cell proud of a marching-squares edge without the
//! confinement having failed.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::tools::ball_tool_config;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, BoundaryContainment, BoundarySource, StockSource,
};
use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::compute::operation_configs::UnifiedFinishConfig;
use rs_cam_core::geo::P2;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::polygon::{Polygon2, offset_polygon};
use rs_cam_core::region_set::RegionSet;
use rs_cam_core::session::ProjectSession;
use rs_cam_core::tier_islands::{TierIslandParams, extract_tier_islands};
use rs_cam_core::tier_map::{ResidualTreatment, TierLadder, TierMapParams, compute_tier_map};
use rs_cam_core::tool::MillingCutter;
use rs_cam_core::toolpath::MoveType;

const HALF_MM: f64 = 8.0;
const STEP_MM: f64 = 0.25;
/// Bowl radius (mm) — tighter than the coarse ball's 2.0 mm radius, wider
/// than the fine ball's 1.0 mm.
const BOWL_R_MM: f64 = 1.6;
const CELL_MM: f64 = 0.3;
const TOLERANCE_MM: f64 = 0.05;
const MARGIN_MM: f64 = 0.5;
/// Containment slack: one planning cell. See the module doc.
const EPS_MM: f64 = CELL_MM;

fn plane_with_bowl() -> TriangleMesh {
    common::meshes::height_field(HALF_MM, STEP_MM, |x, y| {
        let d_sq = x * x + y * y;
        if d_sq < BOWL_R_MM * BOWL_R_MM {
            -(BOWL_R_MM * BOWL_R_MM - d_sq).sqrt()
        } else {
            0.0
        }
    })
}

/// The bowl is ~8 mm² and the DERIVED min-island floor for an R1.0 tool is
/// `(2·1.0)²·4 = 16 mm²`, so on the defaults the fixture's own feature would
/// be absorbed back into the coarse tier and this file would end up testing
/// the plate rim instead of the bowl. The floor is pinned explicitly — the
/// dial's `Some(v)` arm is taken verbatim at every coarseness, by contract.
fn island_params() -> TierIslandParams {
    TierIslandParams {
        min_region_area_mm2: Some(1.0),
        ..TierIslandParams::default()
    }
}

fn recipe(tool_ids: Vec<usize>) -> BoundarySource {
    BoundarySource::PlannedTierRegions {
        tool_ids,
        tier: 1,
        cell_mm: CELL_MM,
        tolerance_mm: TOLERANCE_MM,
        margin_mm: MARGIN_MM,
        treatment: ResidualTreatment::Raw,
        islands: island_params(),
    }
}

/// The tier-1 machining polygons, computed independently of the session.
fn oracle_machining_polygons() -> Vec<Polygon2> {
    let mesh = plane_with_bowl();
    let index = SpatialIndex::build(&mesh, 5.0);
    let coarse = build_cutter(&ball_tool_config(4.0));
    let fine = build_cutter(&ball_tool_config(2.0));
    let tools: [&dyn MillingCutter; 2] = [&coarse, &fine];
    let ladder = TierLadder::new(&tools).unwrap();
    let params = TierMapParams {
        cell_mm: CELL_MM,
        tolerance_mm: TOLERANCE_MM,
        margin_mm: MARGIN_MM,
        treatment: ResidualTreatment::Raw,
    };
    let cancel = AtomicBool::new(false);
    let map = compute_tier_map(
        &mesh,
        &index,
        &ladder,
        &params,
        &(|| cancel.load(std::sync::atomic::Ordering::SeqCst)),
    )
    .unwrap();
    let cusp_radii = [coarse.cusp_radius_mm(), fine.cusp_radius_mm()];
    let islands = extract_tier_islands(&map, &island_params(), &cusp_radii).unwrap();
    islands
        .set_for_tier(1)
        .map(|s| s.machining.as_slice().to_vec())
        .unwrap_or_default()
}

#[test]
fn a_fine_tier_op_cuts_only_inside_its_own_islands() {
    let oracle = oracle_machining_polygons();
    assert!(
        !oracle.is_empty(),
        "fixture must give the fine tier territory, or the containment claim \
         below is vacuous"
    );
    // Grown by one planning cell — see the module doc on why the raw
    // polygons are the wrong bar for an emitter on a different grid.
    let slack: Vec<Polygon2> = oracle
        .iter()
        // NEGATIVE distance grows (`offset_polygon`'s convention: positive
        // is inward).
        .flat_map(|p| offset_polygon(p, -EPS_MM))
        .collect();
    let allowed = RegionSet::new(slack);

    let mut session = ProjectSession::new_empty();
    let _ = session.set_stock_config(common::session::stock_under(HALF_MM, 6.0));
    let coarse_idx = session
        .add_tool(ball_tool_config(4.0))
        .created
        .expect("add_tool reports the new tool index");
    let fine_idx = session
        .add_tool(ball_tool_config(2.0))
        .created
        .expect("add_tool reports the new tool index");
    let coarse_id = session.tools()[coarse_idx].id.0;
    let fine_id = session.tools()[fine_idx].id.0;
    let model_id = session
        .add_model(common::session::mesh_model(plane_with_bowl(), "bowl"))
        .created
        .expect("add_model reports the new model id");

    // The op the planner would emit for tier 1, minus the stock chaining:
    // `Fresh` keeps this sentry about the BOUNDARY. The stock source is a
    // separate dependency with its own precondition, and pulling a whole
    // simulate-then-generate cascade in here would test that instead.
    let mut tc = common::session::toolpath_config(
        "Finish tier 1 (R1.0)",
        OperationConfig::UnifiedFinish(UnifiedFinishConfig {
            scallop_height: 0.05,
            tolerance: 0.1,
            ..UnifiedFinishConfig::default()
        }),
        fine_id,
        model_id,
    );
    tc.stock_source = StockSource::Fresh;
    tc.heights = common::session::pinned_heights(0.0, -3.0);
    tc.boundary_inherit = false;
    tc.boundary = BoundaryConfig {
        enabled: true,
        source: recipe(vec![coarse_id, fine_id]),
        containment: BoundaryContainment::Center,
        offset: 0.0,
    };
    let index = session
        .add_toolpath(0, tc)
        .unwrap()
        .created
        .expect("add_toolpath reports the new toolpath index");

    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(index, &cancel)
        .expect("the fine tier must generate");

    let result = session.get_result(index).expect("a generated result");
    let cutting: Vec<P2> = result
        .toolpath()
        .moves
        .iter()
        .filter(|m| !matches!(m.move_type, MoveType::Rapid))
        .map(|m| P2::new(m.target.x, m.target.y))
        .collect();
    assert!(
        !cutting.is_empty(),
        "a confined op that emits no cutting motion proves nothing — a gate \
         handed an empty population passes and looks healthy"
    );

    let outside: Vec<&P2> = cutting.iter().filter(|p| !allowed.contains(p)).collect();
    assert!(
        outside.is_empty(),
        "{} of {} cutting moves lie outside the tier's islands (+{EPS_MM} mm). \
         A boundary that does not confine is worse than no boundary: the fine \
         tool pays full price for the whole board. First offender: {:?}",
        outside.len(),
        cutting.len(),
        outside.first()
    );
}
