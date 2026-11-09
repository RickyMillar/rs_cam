//! W2 (D69 §3): the helical Waterline widens its step on a vertical wall.
//!
//! Pre-registration:
//! `planning/adaptive_research_2026-10-23/integration/W2_PREREG.md`. Inside a
//! W1 chain, ring `i` is skipped when the rings from the last kept ring to
//! ring `i + 1` coincide (the W1 containment test both ways), the gap is at
//! most the rough's axial step, and the deflection check admits the gap.
//! The first and the last ring stay. The deflection check itself is the
//! lib test `the_wall_step_reads_the_deflection_model_w2`
//! (`compute/execute/finish_raster.rs`).
//!
//! The fixture is one chain of square rings, one per level, 10 to 1 in
//! steps of 1 mm.
//!
//! 1. A vertical wall (all rings the same), rough step 3: the helix keeps
//!    the rings at 10, 7, 4 and 1.
//! 2. The check refuses: every ring stays (the W1 output, move for move).
//! 3. A drafted wall (each ring 0.6 mm wider than the one above, more than
//!    the 0.5 mm sampling): every ring stays.
//! 4. No wall step: the W1 output.

#![expect(
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, make_test_hemisphere};
use rs_cam_core::ops::waterline::{
    ChainLinkProbe, LevelRings, LevelRun, WallStep, WaterlineParams, helical_waterline,
};
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::{MoveIntent, Toolpath};

const TOP: f64 = 10.0;
const LEVELS: usize = 10;
const ROUGH_STEP: f64 = 3.0;

fn params() -> WaterlineParams {
    WaterlineParams {
        sampling: 0.5,
        feed_rate: 1000.0,
        plunge_rate: 300.0,
        safe_z: 20.0,
        stock_to_leave: 0.0,
    }
}

/// One square ring per level; `draft` widens each ring by that much per
/// level below the top.
fn levels(draft: f64) -> Vec<LevelRings> {
    (0..LEVELS)
        .map(|k| {
            let z = TOP - k as f64;
            let half = 20.0 + draft * k as f64;
            let ring = vec![
                P3::new(-half, -half, z),
                P3::new(half, -half, z),
                P3::new(half, half, z),
                P3::new(-half, half, z),
            ];
            LevelRings {
                runs: vec![LevelRun {
                    path: ring.clone(),
                    whole: true,
                    contour: 0,
                }],
                contours: vec![ring],
                rest_kept: false,
                rest_dropped: false,
                outer_rings_skipped: 0,
            }
        })
        .collect()
}

fn helix(levels: &[LevelRings], wall: Option<&WallStep<'_>>) -> Toolpath {
    let mesh = make_test_hemisphere(5.0, 8);
    let index = SpatialIndex::build(&mesh, 5.0);
    // The rings stand far outside the hemisphere: every link is clear.
    let tool = FlatEndmill::new(6.0, 25.0);
    let probe = ChainLinkProbe {
        mesh: &mesh,
        index: &index,
        reduced_tool: &tool,
    };
    rs_cam_core::interrupt::run_uncancellable(|cancel| {
        helical_waterline(levels, &params(), &probe, wall, None, cancel)
    })
}

/// The Z of each flat lap end and descent end: the Z values the chain
/// reaches, top first, one per kept ring.
fn ring_levels(tp: &Toolpath) -> Vec<f64> {
    let mut zs: Vec<f64> = tp
        .moves
        .iter()
        .filter(|m| m.intent == MoveIntent::FinishingCut || m.intent == MoveIntent::EntryPlunge)
        .map(|m| m.target.z)
        .filter(|z| (z - z.round()).abs() < 1e-9)
        .collect();
    zs.dedup();
    zs
}

fn same_moves(a: &Toolpath, b: &Toolpath) -> bool {
    a.moves.len() == b.moves.len()
        && a.moves
            .iter()
            .zip(&b.moves)
            .all(|(x, y)| x.target == y.target && x.intent == y.intent)
}

#[test]
fn a_vertical_wall_keeps_the_rings_at_the_rough_step() {
    let admits = |_: usize, _: &[P3], _: f64| true;
    let wall = WallStep {
        max_step_mm: ROUGH_STEP,
        admits: &admits,
    };
    let rings = levels(0.0);
    let widened = helix(&rings, Some(&wall));
    let w1 = helix(&rings, None);
    assert_eq!(
        ring_levels(&w1),
        vec![10.0, 9.0, 8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0],
        "control: the W1 helix reaches every level"
    );
    assert_eq!(
        ring_levels(&widened),
        vec![10.0, 7.0, 4.0, 1.0],
        "kept rings at most the rough step apart; the first and last stay"
    );
    let entries = |tp: &Toolpath| {
        tp.moves
            .iter()
            .filter(|m| m.intent == MoveIntent::EntryPlunge)
            .count()
    };
    assert_eq!(entries(&widened), 1, "still one chain, one entry");
}

#[test]
fn a_refused_deflection_check_keeps_every_ring() {
    let refuses = |_: usize, _: &[P3], _: f64| false;
    let wall = WallStep {
        max_step_mm: ROUGH_STEP,
        admits: &refuses,
    };
    let rings = levels(0.0);
    assert!(same_moves(
        &helix(&rings, Some(&wall)),
        &helix(&rings, None)
    ));
}

#[test]
fn a_drafted_wall_keeps_every_ring() {
    let admits = |_: usize, _: &[P3], _: f64| true;
    let wall = WallStep {
        max_step_mm: ROUGH_STEP,
        admits: &admits,
    };
    // 0.6 mm per level: more than the 0.5 mm sampling, so no two rings
    // coincide, while each still lies inside the next (one chain).
    let rings = levels(0.6);
    let widened = helix(&rings, Some(&wall));
    assert_eq!(ring_levels(&widened).len(), LEVELS, "no ring is skipped");
    assert!(same_moves(&widened, &helix(&rings, None)));
}

#[test]
fn the_admit_reads_the_gap_and_the_lower_ring() {
    // The check sees the gap to the next ring and that ring's level.
    let seen = std::cell::RefCell::new(Vec::new());
    let record = |k: usize, ring: &[P3], ap: f64| {
        seen.borrow_mut().push((k, ring[0].z, ap));
        true
    };
    let wall = WallStep {
        max_step_mm: ROUGH_STEP,
        admits: &record,
    };
    let _ = helix(&levels(0.0), Some(&wall));
    for &(k, z, ap) in seen.borrow().iter() {
        assert!(
            (z - (TOP - k as f64)).abs() < 1e-9,
            "level index {k} is Z {z}"
        );
        assert!(ap > 0.0 && ap <= ROUGH_STEP + 1e-9, "gap {ap}");
    }
    assert!(!seen.borrow().is_empty());
}
