//! B20 (C2) — a lifted link lands on the next cut as an ENTRY, not as a
//! link feed.
//!
//! # The gap
//!
//! Under a link ceiling a finishing link lifts clear of the standing
//! material (`shape_under_ceiling`, the `Lifted` arm). Two emitters use
//! it: the relink inside a fragment set (`relink_fragments`, a clearance
//! hop) and the Unified Finish router link between regions (B16). Both
//! ended the hop with ONE vertical feed from the clearance height down to
//! the next entry, tagged `MoveIntent::Linking`, at the CUTTING feed.
//!
//! The clearance height is the standing material top plus the plunge
//! clearance. Where material stands above the entry, that vertical feed is
//! a plunge into stock at the cut feed. The entry-descent pass
//! (`optimize_entry_descents`) reads only a rapid followed by an
//! `EntryPlunge`, so it did not see the descent either.
//!
//! # The fixture
//!
//! The B16 fixture: a 55° cone cut off by a flat ring, so the planner makes
//! one mid-steep and one shallow region. A solid dexel block stands 3 mm
//! above every point of the part, so EVERY lifted link lands through 3 mm
//! or more of stock.
//!
//! # The claims
//!
//! For the router link (relink off) and for the relink hops (relink on):
//!
//! 1. Population: the toolpath holds at least one vertical fed descent
//!    that starts at or above the block top and ends below it.
//! 2. Every such descent is an `EntryPlunge` at the plunge feed.
//!
//! Red before the fix, measured 2026-10-16: claim 2 failed for both
//! emitters (the descents were `Linking` at 1000 mm/min, the cut feed).
//!
//! ```text
//! cargo test -p rs_cam_core -q --test a_lifted_link_lands_as_an_entry_b20
//! ```

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
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
use rs_cam_core::toolpath::{Move, MoveIntent, MoveType, Toolpath};

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
/// Retract plane (mm), far above the block.
const SAFE_Z: f64 = 30.0;
/// The cut feed and the plunge feed of the fixture (mm/min).
const FEED: f64 = 1000.0;
const PLUNGE: f64 = 500.0;

fn capped_cone() -> TriangleMesh {
    let grade = CONE_SLOPE_DEG.to_radians().tan();
    common::meshes::height_field(HALF_MM, STEP_MM, |x, y| {
        (-grade * (x * x + y * y).sqrt()).max(-RING_DEPTH_MM)
    })
}

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

fn params(intra_region_hookup_mm: f64) -> UnifiedFinishParams {
    UnifiedFinishParams {
        scallop_height: 0.05,
        tolerance: 0.05,
        sampling: 0.5,
        raster_stepover: 0.5,
        stock_to_leave: 0.0,
        safe_z: SAFE_Z,
        feed_rate: FEED,
        plunge_rate: PLUNGE,
        intra_region_hookup_mm,
        ..UnifiedFinishParams::default()
    }
}

fn planner() -> FinishPlannerParams {
    FinishPlannerParams {
        steep_threshold_deg: 30.0,
        waterline_threshold_deg: 80.0,
        ..FinishPlannerParams::for_tool(1.0)
    }
}

fn generate(intra_region_hookup_mm: f64) -> (Toolpath, UnifiedFinishReport) {
    let mesh = capped_cone();
    let index = SpatialIndex::build(&mesh, 5.0);
    let cutter = ball_cutter(2.0);
    let link = LinkKinematics {
        kinematics: MachineKinematics::generic_wood_router(),
        max_feed_mm_min: 5000.0,
        rapid_feed_mm_min: 5000.0,
    };
    let stock = standing_block();
    let cancel = AtomicBool::new(false);
    let (tp, _anns, report) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        0.0,
        -RING_DEPTH_MM - 1.0,
        &params(intra_region_hookup_mm),
        &planner(),
        None,
        Some(&link),
        None,
        None,
        Some(LinkCeiling {
            stock: Some(&stock),
            tool_radius: 1.0,
            fallback_top_z: STOCK_TOP_Z,
            obstacles: None,
        }),
        &(|| cancel.load(std::sync::atomic::Ordering::SeqCst)),
    )
    .expect("the fixture generates");
    (tp, report)
}

/// Every fed vertical descent that starts at or above the block top and
/// ends below it: a feed that enters the standing block from above.
fn descents_into_the_block(tp: &Toolpath) -> Vec<(Move, Move)> {
    tp.moves
        .windows(2)
        .filter(|w| {
            let (from, to) = (&w[0], &w[1]);
            !matches!(to.move_type, MoveType::Rapid)
                && (to.target.x - from.target.x).hypot(to.target.y - from.target.y) < 1e-6
                && from.target.z >= STOCK_TOP_Z - 1e-6
                && to.target.z < STOCK_TOP_Z - 0.1
        })
        .map(|w| (w[0].clone(), w[1].clone()))
        .collect()
}

fn feed_of(m: &Move) -> Option<f64> {
    match m.move_type {
        MoveType::Linear { feed_rate } => Some(feed_rate),
        _ => None,
    }
}

fn assert_entries(tp: &Toolpath, what: &str) {
    let descents = descents_into_the_block(tp);
    assert!(
        !descents.is_empty(),
        "{what}: no descent enters the block, so the claim is vacuous"
    );
    let offenders: Vec<&(Move, Move)> = descents
        .iter()
        .filter(|(_, to)| to.intent != MoveIntent::EntryPlunge || feed_of(to) != Some(PLUNGE))
        .collect();
    assert!(
        offenders.is_empty(),
        "{what}: {} of {} descents into standing stock are not an EntryPlunge at the \
         plunge feed {PLUNGE}; first {:?} -> {:?} ({:?}, feed {:?})",
        offenders.len(),
        descents.len(),
        offenders[0].0.target,
        offenders[0].1.target,
        offenders[0].1.intent,
        feed_of(&offenders[0].1),
    );
}

/// The router link between the two regions (relink off).
#[test]
fn a_router_link_lands_as_an_entry() {
    let (tp, report) = generate(0.0);
    assert!(
        report.links.iter().any(|l| l.surface),
        "the router keeps a lifted link under the ceiling: {:?}",
        report.links
    );
    assert_entries(&tp, "router link");
}

/// The clearance hops of the relink inside a region (relink on).
#[test]
fn a_clearance_hop_lands_as_an_entry() {
    let (tp, report) = generate(5.0);
    let hops = report.relink.as_ref().map_or(0, |r| r.clearance_hops);
    assert!(
        hops > 0,
        "the relink takes clearance hops: {:?}",
        report.relink
    );
    assert_entries(&tp, "clearance hop");
}
