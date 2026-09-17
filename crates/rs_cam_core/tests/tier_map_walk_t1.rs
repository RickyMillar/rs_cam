//! T1 — the n-tool residual walk and its tier labels
//! (`planning/multitool_2026-08-23/ORCHESTRATION_PLAN.md` Phase T, task T1).
//!
//! The fixture is the one the plan names: **a plane with a hemispherical bowl
//! tighter than the coarse tool's radius**. It is built so the two answers the
//! tier map must give are separable by construction, not by tuning:
//!
//! | where | coarse Ø3 ball (R1.5) | fine Ø0.5 ball (R0.25) | residual |
//! |---|---|---|---|
//! | the plane | tip rests at z = 0 | tip rests at z = 0 | 0 |
//! | the bowl centre | wedged on the rim at `-(R − √(R² − r_b²))` = **−0.6** | reaches the floor at **−1.2** | **0.6 mm** |
//!
//! With `tolerance_mm = 0.03` the plane is inside tolerance and the bowl floor
//! is 20× outside it, so a walk that inverts the label order, references the
//! wrong tool, or drops the coarse tool's own surface instead of the finest
//! reference cannot pass both rows.
//!
//! The bowl radius (1.2 mm) is deliberately **smaller** than the coarse ball's
//! 1.5 mm radius: the coarse tool physically cannot enter, which is the
//! geometric fact the tier decision is supposed to discover.
//!
//! What is NOT asserted here, and why: the map does **no boundary erosion**, so
//! a coarse tool hanging off the part rim reads a false-high residual and the
//! rim reads as fine territory. That is stated in `tier_map`'s module doc and
//! left to the consumer (Phase I / `rest_field`'s chamfer distance), so the
//! plane assertions below stay one coarse envelope inside the mesh edge.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};

use common::tools::ball_cutter;
use rs_cam_core::maps::tier_map::{
    NO_TIER, ResidualTreatment, TierLadder, TierMap, TierMapError, TierMapParams, compute_tier_map,
    ladder_drops_at,
};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::surface::dropcutter::point_drop_cutter;
use rs_cam_core::tool::MillingCutter;

/// Half-extent of the fixture plate (mm).
const HALF_MM: f64 = 6.0;
/// Height-field sampling pitch (mm).
const STEP_MM: f64 = 0.2;
/// Bowl radius (mm) — tighter than the coarse tool's 1.5 mm radius.
const BOWL_R_MM: f64 = 1.2;

/// A flat plate at z = 0 with one hemispherical bowl of radius [`BOWL_R_MM`]
/// centred on the origin, its floor at `-BOWL_R_MM`.
fn plane_with_bowl() -> TriangleMesh {
    common::meshes::height_field(HALF_MM, STEP_MM, |x, y| {
        let d = (x * x + y * y).sqrt();
        if d < BOWL_R_MM {
            // Spherical bowl of radius BOWL_R_MM whose sphere centre sits at
            // z = 0: floor at -BOWL_R_MM, meeting the plane tangentially
            // (vertical wall) at the rim.
            -(BOWL_R_MM * BOWL_R_MM - d * d).sqrt()
        } else {
            0.0
        }
    })
}

fn params() -> TierMapParams {
    TierMapParams {
        cell_mm: 0.25,
        tolerance_mm: 0.03,
        margin_mm: 0.5,
        treatment: ResidualTreatment::Raw,
    }
}

fn never_cancel() -> impl Fn() -> bool + Send + Sync {
    || false
}

/// Build the fixture map with the plan's two-tier ladder: a Ø3 ball that
/// cannot enter the bowl, and a Ø0.5 ball that can.
fn build_fixture() -> TierMap {
    let mesh = plane_with_bowl();
    let index = SpatialIndex::build_auto(&mesh);
    let coarse = ball_cutter(3.0);
    let fine = ball_cutter(0.5);
    let tools: [&dyn MillingCutter; 2] = [&coarse, &fine];
    let ladder = TierLadder::new(&tools).expect("coarse -> fine ladder is valid");
    compute_tier_map(&mesh, &index, &ladder, &params(), &never_cancel())
        .expect("uncancelled tier map")
}

#[test]
fn the_plane_is_claimed_by_the_coarsest_tier() {
    let map = build_fixture();
    // Stay one coarse envelope (1.5 mm) plus a cell inside the plate edge and
    // one coarse envelope clear of the bowl: those two bands are the only
    // places the coarse tool is not resting on something it does not cover.
    let mut checked = 0usize;
    for row in 0..map.grid.ny {
        for col in 0..map.grid.nx {
            let (x, y) = map.cell_center(row, col).unwrap();
            if x.abs() > HALF_MM - 2.0 || y.abs() > HALF_MM - 2.0 {
                continue;
            }
            if (x * x + y * y).sqrt() < BOWL_R_MM + 2.0 {
                continue;
            }
            checked += 1;
            assert_eq!(
                map.label_at(row, col),
                Some(0),
                "flat ground at ({x:.2}, {y:.2}) must be coarse-tier territory"
            );
        }
    }
    assert!(
        checked > 200,
        "the plane sample must be a real population, got {checked} cells"
    );
}

#[test]
fn the_bowl_floor_falls_to_the_finer_tier() {
    let map = build_fixture();
    let (row, col) = map.nearest_cell(0.0, 0.0).expect("bowl centre is on grid");
    assert_eq!(
        map.label_at(row, col),
        Some(1),
        "the bowl floor is out of the coarse tool's reach and must be fine-tier"
    );

    // And it is a region, not a single cell: every cell whose centre is well
    // inside the bowl is fine-tier too.
    let mut inner = 0usize;
    for r in 0..map.grid.ny {
        for c in 0..map.grid.nx {
            let (x, y) = map.cell_center(r, c).unwrap();
            if (x * x + y * y).sqrt() > BOWL_R_MM * 0.5 {
                continue;
            }
            inner += 1;
            assert_eq!(
                map.label_at(r, c),
                Some(1),
                "bowl interior at ({x:.2}, {y:.2}) must be fine-tier"
            );
        }
    }
    assert!(inner >= 4, "bowl interior sample too small: {inner} cells");
}

#[test]
fn outside_the_mesh_is_the_no_tier_sentinel() {
    let map = build_fixture();
    // The grid is padded past the mesh bbox by the finest envelope plus the
    // margin, so the outer corner is beyond every tool's reach.
    let corner = map.label_at(0, 0);
    assert_eq!(
        corner,
        Some(NO_TIER),
        "the padded corner is off the part and must carry the sentinel"
    );
    let (x, y) = map.cell_center(0, 0).unwrap();
    assert!(
        x < -HALF_MM && y < -HALF_MM,
        "corner cell ({x:.2}, {y:.2}) should lie outside the plate"
    );
    assert!(
        map.unassigned_cells() > 0,
        "a padded grid must carry some sentinel cells"
    );
    assert_eq!(
        map.tier_cell_counts().iter().sum::<usize>() + map.unassigned_cells(),
        map.labels.len(),
        "every cell is either a tier or the sentinel"
    );
}

#[test]
fn one_max_radius_query_reproduces_point_drop_cutter_exactly() {
    // G1's load-bearing claim: a single spatial-index query at the LARGEST
    // ladder envelope, filtered per tool by `drop_cutter_can_contact`, is
    // bit-identical to querying per tool at its own radius. If this ever
    // stops holding, the shared walk silently changes every drop it takes.
    let mesh = plane_with_bowl();
    let index = SpatialIndex::build_auto(&mesh);
    let coarse = ball_cutter(3.0);
    let mid = ball_cutter(1.0);
    let fine = ball_cutter(0.5);
    let tools: [&dyn MillingCutter; 3] = [&coarse, &mid, &fine];
    let ladder = TierLadder::new(&tools).expect("ladder is coarse -> fine");

    let mut compared = 0usize;
    for i in 0..40 {
        for j in 0..40 {
            let x = -7.0 + f64::from(i) * 0.35;
            let y = -7.0 + f64::from(j) * 0.35;
            let shared = ladder_drops_at(x, y, &mesh, &index, &ladder);
            assert_eq!(shared.len(), 3);
            for (k, tool) in tools.iter().enumerate() {
                let solo = point_drop_cutter(x, y, &mesh, &index, *tool);
                assert_eq!(
                    shared[k].z.to_bits(),
                    solo.z.to_bits(),
                    "tool {k} at ({x:.2}, {y:.2}): shared-query drop must be bit-identical"
                );
                assert_eq!(shared[k].contacted, solo.contacted);
                compared += 1;
            }
        }
    }
    assert_eq!(compared, 40 * 40 * 3);
}

#[test]
fn two_runs_produce_an_identical_map() {
    let mesh = plane_with_bowl();
    let index = SpatialIndex::build_auto(&mesh);
    let coarse = ball_cutter(3.0);
    let fine = ball_cutter(0.5);
    let tools: [&dyn MillingCutter; 2] = [&coarse, &fine];
    let ladder = TierLadder::new(&tools).unwrap();

    let a = compute_tier_map(&mesh, &index, &ladder, &params(), &never_cancel()).unwrap();
    let b = compute_tier_map(&mesh, &index, &ladder, &params(), &never_cancel()).unwrap();

    assert_eq!((a.grid.nx, a.grid.ny), (b.grid.nx, b.grid.ny));
    assert_eq!(a.labels, b.labels, "tier labels must be deterministic");
    // NaN != NaN, so compare the finest-drop plane bitwise rather than by
    // value — a determinism sentry that skips the sentinel cells is not one.
    let a_bits: Vec<u32> = a.finest_z.iter().map(|z| z.to_bits()).collect();
    let b_bits: Vec<u32> = b.finest_z.iter().map(|z| z.to_bits()).collect();
    assert_eq!(a_bits, b_bits, "finest-drop plane must be deterministic");
}

#[test]
fn tier_mask_selects_exactly_its_own_label() {
    let map = build_fixture();
    for k in 0..2usize {
        let mask = map.tier_mask(k);
        assert_eq!(mask.len(), map.labels.len());
        let want = u8::try_from(k).unwrap();
        for (i, flag) in mask.iter().enumerate() {
            assert_eq!(*flag, map.labels[i] == want, "mask cell {i} disagrees");
        }
        assert_eq!(
            mask.iter().filter(|f| **f).count(),
            map.tier_cell_counts()[k]
        );
    }
    assert!(
        map.tier_mask(9).iter().all(|f| !f),
        "a tier index past the ladder selects nothing"
    );
    let covered = map.covered_mask();
    for (i, flag) in covered.iter().enumerate() {
        assert_eq!(*flag, map.labels[i] != NO_TIER);
    }
}

#[test]
fn cancellation_aborts_the_walk_without_panicking() {
    let mesh = plane_with_bowl();
    let index = SpatialIndex::build_auto(&mesh);
    let coarse = ball_cutter(3.0);
    let fine = ball_cutter(0.5);
    let tools: [&dyn MillingCutter; 2] = [&coarse, &fine];
    let ladder = TierLadder::new(&tools).unwrap();

    let polls = AtomicUsize::new(0);
    let cancel_at_once = || {
        polls.fetch_add(1, Ordering::Relaxed);
        true
    };
    let err = compute_tier_map(&mesh, &index, &ladder, &params(), &cancel_at_once)
        .expect_err("an always-cancelling walk must not return a map");
    assert!(matches!(err, TierMapError::Cancelled));
    assert!(
        polls.load(Ordering::Relaxed) > 0,
        "the walk must actually poll the cancel token — rest_field's does not"
    );
}

#[test]
fn a_degenerate_ladder_is_refused_rather_than_mislabelled() {
    let empty: [&dyn MillingCutter; 0] = [];
    assert!(matches!(
        TierLadder::new(&empty),
        Err(TierMapError::EmptyLadder)
    ));

    let coarse = ball_cutter(3.0);
    let fine = ball_cutter(0.5);
    // Fine first is the wrong way round: "coarsest tool that reaches" is
    // meaningless on an unordered ladder, so it is refused, not guessed.
    let backwards: [&dyn MillingCutter; 2] = [&fine, &coarse];
    match TierLadder::new(&backwards) {
        Err(TierMapError::LadderNotCoarseToFine { index }) => assert_eq!(index, 1),
        other => panic!("expected a coarse->fine refusal, got {other:?}"),
    }
}

#[test]
fn a_single_tool_ladder_labels_every_covered_cell_zero() {
    let mesh = plane_with_bowl();
    let index = SpatialIndex::build_auto(&mesh);
    let fine = ball_cutter(0.5);
    let tools: [&dyn MillingCutter; 1] = [&fine];
    let ladder = TierLadder::new(&tools).unwrap();
    let map = compute_tier_map(&mesh, &index, &ladder, &params(), &never_cancel()).unwrap();
    for (i, &label) in map.labels.iter().enumerate() {
        assert!(
            label == 0 || label == NO_TIER,
            "cell {i} of a one-tool ladder must be tier 0 or the sentinel, got {label}"
        );
    }
    assert!(map.tier_cell_counts()[0] > 0);
}
