//! B3 — an intra-region stay-down link must clear STANDING STOCK, not ride
//! the mesh (`planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md` Phase O
//! item 3).
//!
//! # The defect this is red against
//!
//! `unified_finish`'s intra-region relink hardcoded
//! `link_ceiling: None`, with a comment claiming "the mesh IS the material
//! here". That is true of a fresh-stock finishing pass and **false** of a
//! `FromRemainingStock` op: there, material stands wherever no prior op has
//! cut yet, and a link that rides the design surface is a **cutting feed
//! straight through it** — the G-LINKLOAD class, fixed for pencil in
//! `fb5339da` and left open here.
//!
//! # The fixture, and why it isolates the intra-region seam
//!
//! A **55° cone** over an 18 mm plate. Every covered cell has the same slope,
//! and the planner's band thresholds are pinned either side of it (30° / 80°),
//! so the decomposition produces exactly **one mid-steep region**. That
//! matters: with one region the router has nothing to link *between*, so every
//! non-rapid `Linking` move in the output came from the intra-region relink —
//! the seam under test — and nothing else can mask or manufacture a pass.
//!
//! The scallop cascade fills that region with concentric rings, but it links
//! its rings natively: the emitted path reaches the relink as TWO fragments
//! with one junction, and that junction's XY hop is wider than the shipped
//! 6 mm hookup (measured: the first run of this sentry declined it
//! `too_far: 1` and the population guard fired). The hookup here is therefore
//! set above the plate diagonal, so `too_far` is impossible by construction
//! and the ceiling is the only thing deciding the junction — the seam under
//! test, with a non-vacuous population (asserted, not assumed — a gate
//! handed an empty population passes and looks healthy).
//!
//! Over it stands a **solid dexel block whose top is +3 mm**, above every
//! point of the cone. So a correct link cannot ride the surface anywhere: it
//! must leave the cut vertically, traverse at `+3 + PLUNGE_CLEARANCE_MM`, and
//! re-enter vertically. `safe_z` is 30, far above that clearance, so the
//! ceiling has no excuse to refuse the link and fall back to a retract —
//! the arm that would make this test vacuously green.
//!
//! # What is asserted, and on what
//!
//! **Stored motion**, never a report field or a plan value: for every pair of
//! consecutive moves whose second is a non-rapid `Linking` move that travels
//! horizontally, BOTH ends must sit at or above the standing material top.
//! The vertical exit and re-entry legs travel no horizontal distance and are
//! therefore exempt by construction rather than by a special case — which is
//! exactly the guarantee `LinkCeiling` states.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::{AtomicBool, Ordering};

use common::tools::ball_cutter;
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::finish::finish_planner::FinishPlannerParams;
use rs_cam_core::finish::surface_link::LinkCeiling;
use rs_cam_core::finish::unified_finish::{
    UnifiedFinishParams, unified_finish_toolpath_with_cancel,
    unified_finish_toolpath_with_cancel_and_ceiling,
};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::toolpath::{Move, MoveIntent, MoveType, Toolpath};

/// Half-extent of the fixture plate (mm).
const HALF_MM: f64 = 9.0;
/// Height-field sampling pitch (mm).
const STEP_MM: f64 = 0.25;
/// Cone slope (degrees). Strictly between the band thresholds pinned below,
/// so the whole surface lands in ONE mid-steep region.
const CONE_SLOPE_DEG: f64 = 55.0;
/// Top of the standing dexel block (mm) — above every point of the cone.
const STOCK_TOP_Z: f64 = 3.0;
/// Retract plane (mm). Deliberately far above `STOCK_TOP_Z +
/// PLUNGE_CLEARANCE_MM` so a lifted link is never refused for reaching it.
const SAFE_Z: f64 = 30.0;

/// A cone of constant [`CONE_SLOPE_DEG`] slope, apex at the origin, falling
/// outward.
fn cone() -> TriangleMesh {
    let grade = CONE_SLOPE_DEG.to_radians().tan();
    common::meshes::height_field(HALF_MM, STEP_MM, |x, y| -grade * (x * x + y * y).sqrt())
}

/// A solid block covering the plate, its top at [`STOCK_TOP_Z`] — the
/// "nothing has cut here yet" stock a `FromRemainingStock` op is handed.
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
        stock_to_leave: 0.0,
        safe_z: SAFE_Z,
        // Above the plate diagonal (√2·2·HALF_MM ≈ 25.5), so the fixture's
        // single junction can never be declined `too_far` — the ceiling is
        // then the only arbiter of the link, which is the seam under test.
        intra_region_hookup_mm: 30.0,
        ..UnifiedFinishParams::default()
    }
}

/// Band thresholds pinned either side of [`CONE_SLOPE_DEG`], so the fixture's
/// single-region property is a property of the TEST and not of whatever the
/// shipped defaults happen to be.
fn planner() -> FinishPlannerParams {
    FinishPlannerParams {
        steep_threshold_deg: 30.0,
        waterline_threshold_deg: 80.0,
        ..FinishPlannerParams::for_tool(1.0)
    }
}

/// Consecutive-move pairs whose second is a fed (non-rapid) `Linking` move
/// that actually travels in XY.
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
fn intra_region_links_clear_standing_stock_g_linkload() {
    let mesh = cone();
    let index = SpatialIndex::build(&mesh, 5.0);
    let cutter = ball_cutter(2.0);
    let stock = standing_block();
    let cancel = AtomicBool::new(false);

    let (tp, _anns, report) = unified_finish_toolpath_with_cancel_and_ceiling(
        &mesh,
        &index,
        &cutter,
        0.0,
        -14.0,
        &params(),
        &planner(),
        None,
        None,
        None,
        None,
        Some(LinkCeiling {
            stock: Some(&stock),
            tool_radius: 1.0,
            fallback_top_z: STOCK_TOP_Z,
        }),
        &(|| cancel.load(Ordering::SeqCst)),
    )
    .expect("the fixture must generate");

    // Population first: a gate handed an empty population passes and looks
    // healthy (CLAUDE.md). If the cascade stops fragmenting this region the
    // assertion below becomes vacuous, and that must fail loudly here rather
    // than quietly pass forever.
    assert!(
        report.relink.surface_links > 0,
        "fixture no longer produces intra-region surface links, so the \
         ceiling assertion would be vacuous: {:?}",
        report.relink
    );

    let legs = horizontal_link_legs(&tp);
    assert!(
        !legs.is_empty(),
        "surface links were reported but no fed Linking move travels in XY: \
         {:?}",
        report.relink
    );

    let floor = STOCK_TOP_Z + rs_cam_core::toolpath::PLUNGE_CLEARANCE_MM - 1e-6;
    let offenders: Vec<(Move, Move)> = legs
        .iter()
        .filter(|(from, to)| from.target.z < floor || to.target.z < floor)
        .cloned()
        .collect();
    assert!(
        offenders.is_empty(),
        "a fed link traversing below the standing material top is a CUTTING \
         feed through stock the op has not cleared (G-LINKLOAD): {} of {} \
         legs, first {:?} -> {:?}",
        offenders.len(),
        legs.len(),
        offenders[0].0.target,
        offenders[0].1.target
    );
}

/// The fresh-stock golden: with no ceiling in scope the op must be
/// **byte-identical** to the entry point every existing caller uses. The
/// `_and_ceiling` arm is a strict extension, not a rewrite — so the two
/// entry points are asserted to produce the same move list rather than the
/// delegation being taken on trust.
#[test]
fn no_ceiling_reproduces_the_legacy_entry_point_exactly() {
    let mesh = cone();
    let index = SpatialIndex::build(&mesh, 5.0);
    let cutter = ball_cutter(2.0);
    let cancel = AtomicBool::new(false);
    let never = || cancel.load(Ordering::SeqCst);

    let (legacy, _a, legacy_report) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &cutter,
        0.0,
        -14.0,
        &params(),
        &planner(),
        None,
        None,
        None,
        None,
        &never,
    )
    .expect("the fixture must generate");

    let (extended, _b, extended_report) = unified_finish_toolpath_with_cancel_and_ceiling(
        &mesh,
        &index,
        &cutter,
        0.0,
        -14.0,
        &params(),
        &planner(),
        None,
        None,
        None,
        None,
        None,
        &never,
    )
    .expect("the fixture must generate");

    assert_eq!(
        legacy.moves.len(),
        extended.moves.len(),
        "move count must not move when no ceiling is supplied"
    );
    for (i, (a, b)) in legacy.moves.iter().zip(extended.moves.iter()).enumerate() {
        assert_eq!(
            format!("{a:?}"),
            format!("{b:?}"),
            "move {i} differs between the legacy entry point and the \
             no-ceiling extended one"
        );
    }
    assert_eq!(
        legacy_report.relink.ceiling_above_safe_z, 0,
        "structurally impossible without a ceiling"
    );
    assert_eq!(
        extended_report.relink.ceiling_above_safe_z, 0,
        "structurally impossible without a ceiling"
    );
}
