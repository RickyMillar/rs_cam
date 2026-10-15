//! B16 (W6.4) — the Unified Finish link BETWEEN two regions clears standing
//! stock.
//!
//! # The gap
//!
//! The router (`route_greedy`) joins two band regions with a surface link
//! when the link is faster than a retract. Before B16 that link read no
//! link ceiling when it was planned: it rode the mesh. On a
//! `FromRemainingStock` op material stands above the mesh wherever no prior
//! op has cut yet, so the link was a cutting feed through it (the
//! G-LINKLOAD class). The link inside a region already read the ceiling
//! (`island_stay_down_links_o3`).
//!
//! # The fixture
//!
//! A 55° cone cut off by a flat ring: the planner thresholds (30° / 80°)
//! put the cone in ONE mid-steep region and the ring in ONE shallow region,
//! so the router has exactly one junction. The relink inside a region is
//! OFF (`intra_region_hookup_mm = 0`), so every fed `Linking` move comes
//! from the router. A solid dexel block stands +3 mm above every point.
//!
//! # The claims
//!
//! 1. Population: with no ceiling, the router joins the two regions with a
//!    surface link. The junction is linkable, so claim 2 is not vacuous.
//! 2. With the ceiling, the router still links the two regions (a lift is
//!    cheaper than a retract), and every fed `Linking` move that travels in
//!    XY sits at or above the block top plus the plunge clearance at both
//!    ends.
//!
//! Red proof, done once by hand: with `None` passed for the ceiling in
//! `route_greedy`, claim 2 fails.
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_link_between_regions_clears_standing_stock_b16
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::tools::ball_cutter;
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::finish::finish_planner::FinishPlannerParams;
use rs_cam_core::finish::surface_link::LinkCeiling;
use rs_cam_core::finish::unified_finish::{
    UnifiedFinishParams, UnifiedFinishReport, unified_finish_toolpath_with_cancel,
};
use rs_cam_core::machine::kinematics::{LinkKinematics, MachineKinematics};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::toolpath::{Move, MoveIntent, MoveType, PLUNGE_CLEARANCE_MM, Toolpath};

/// Half-extent of the fixture plate (mm).
const HALF_MM: f64 = 9.0;
/// Height-field sampling pitch (mm).
const STEP_MM: f64 = 0.25;
/// Cone slope (degrees), between the pinned band thresholds.
const CONE_SLOPE_DEG: f64 = 55.0;
/// Depth of the flat ring under the apex (mm).
const RING_DEPTH_MM: f64 = 5.0;
/// Top of the standing dexel block (mm), above every point of the part.
const STOCK_TOP_Z: f64 = 3.0;
/// Retract plane (mm), far above the block, so a lifted link is never
/// refused for reaching it.
const SAFE_Z: f64 = 30.0;

/// The cone, cut off by a flat ring at [`RING_DEPTH_MM`] under the apex.
fn capped_cone() -> TriangleMesh {
    let grade = CONE_SLOPE_DEG.to_radians().tan();
    common::meshes::height_field(HALF_MM, STEP_MM, |x, y| {
        (-grade * (x * x + y * y).sqrt()).max(-RING_DEPTH_MM)
    })
}

/// A solid block over the plate, its top at [`STOCK_TOP_Z`].
fn standing_block() -> TriDexelStock {
    TriDexelStock::from_stock(
        -HALF_MM - 1.0,
        -HALF_MM - 1.0,
        HALF_MM + 1.0,
        HALF_MM + 1.0,
        -30.0,
        STOCK_TOP_Z,
        0.25,
    )
}

fn params() -> UnifiedFinishParams {
    UnifiedFinishParams {
        scallop_height: 0.05,
        tolerance: 0.05,
        sampling: 0.5,
        raster_stepover: 0.5,
        stock_to_leave: 0.0,
        safe_z: SAFE_Z,
        // OFF: every fed link then comes from the router.
        intra_region_hookup_mm: 0.0,
        ..UnifiedFinishParams::default()
    }
}

/// Band thresholds either side of [`CONE_SLOPE_DEG`]: the cone is one
/// mid-steep region and the ring one shallow region.
fn planner() -> FinishPlannerParams {
    FinishPlannerParams {
        steep_threshold_deg: 30.0,
        waterline_threshold_deg: 80.0,
        ..FinishPlannerParams::for_tool(1.0)
    }
}

fn kinematics() -> LinkKinematics {
    LinkKinematics {
        kinematics: MachineKinematics::generic_wood_router(),
        max_feed_mm_min: 5000.0,
        rapid_feed_mm_min: 5000.0,
    }
}

fn generate(ceiling: Option<LinkCeiling<'_>>) -> (Toolpath, UnifiedFinishReport) {
    let mesh = capped_cone();
    let index = SpatialIndex::build(&mesh, 5.0);
    let cutter = ball_cutter(2.0);
    let link = kinematics();
    let cancel = AtomicBool::new(false);
    let (tp, _anns, report) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        0.0,
        -RING_DEPTH_MM - 1.0,
        &params(),
        &planner(),
        None,
        Some(&link),
        None,
        None,
        ceiling,
        &(|| cancel.load(std::sync::atomic::Ordering::SeqCst)),
    )
    .expect("the fixture generates");
    (tp, report)
}

/// Consecutive-move pairs whose second is a fed `Linking` move that travels
/// in XY.
fn horizontal_link_legs(tp: &Toolpath) -> Vec<(Move, Move)> {
    tp.moves
        .windows(2)
        .filter(|w| {
            let cur = &w[1];
            cur.intent == MoveIntent::Linking
                && !matches!(cur.move_type, MoveType::Rapid)
                && (cur.target.x - w[0].target.x).hypot(cur.target.y - w[0].target.y) > 1e-6
        })
        .map(|w| (w[0].clone(), w[1].clone()))
        .collect()
}

#[test]
fn the_router_link_between_regions_clears_standing_stock() {
    // Claim 1: the fixture's single junction is a surface link when no
    // ceiling is in scope.
    let (fresh_tp, fresh) = generate(None);
    assert!(fresh.relink.is_none(), "the intra-region relink is off");
    assert_eq!(
        fresh.region_table.len(),
        2,
        "the fixture plans one mid-steep and one shallow region: {:?}",
        fresh.region_table
    );
    assert!(
        fresh.links.iter().any(|l| l.surface),
        "the router keeps a surface link between the regions on fresh stock, \
         so the claim below is not vacuous: {:?}",
        fresh.links
    );
    assert!(
        !horizontal_link_legs(&fresh_tp).is_empty(),
        "the fresh surface link travels in XY"
    );

    // Claim 2: under the ceiling, no fed link leg is below the block.
    let stock = standing_block();
    let (tp, report) = generate(Some(LinkCeiling {
        stock: Some(&stock),
        tool_radius: 1.0,
        fallback_top_z: STOCK_TOP_Z,
        obstacles: None,
    }));
    let floor = STOCK_TOP_Z + PLUNGE_CLEARANCE_MM - 1e-6;
    let legs = horizontal_link_legs(&tp);
    // Non-vacuity: the junction still links (a lift is far cheaper than a
    // retract to `SAFE_Z`), so the legs below are a real population.
    assert!(
        report.links.iter().any(|l| l.surface) && !legs.is_empty(),
        "the router keeps a (lifted) link under the ceiling: {:?}",
        report.links
    );
    let offenders: Vec<&(Move, Move)> = legs
        .iter()
        .filter(|(from, to)| from.target.z < floor || to.target.z < floor)
        .collect();
    assert!(
        offenders.is_empty(),
        "a fed link between regions traverses below the standing material \
         top: a cutting feed through stock the op has not cleared \
         (G-LINKLOAD): {} of {} legs, first {:?} -> {:?}; links {:?}",
        offenders.len(),
        legs.len(),
        offenders[0].0.target,
        offenders[0].1.target,
        report.links
    );
}
