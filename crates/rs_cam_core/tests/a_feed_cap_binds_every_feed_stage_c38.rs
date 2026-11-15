//! C38 §15 — a planner's per-move feed cap (`Move::feed_cap`) binds every
//! feed stage, and a move with no cap is unchanged.
//!
//! The research arm lowers the feed of a turn or a slot step by a load
//! rule (`F × c/w`). The feed modulation dressup (ConstrainedMax) then set
//! the feed from the chipload band and the engagement alone, so it raised
//! those moves back to the band limit (Corne D29: 1554 strip samples over
//! the bound at F 4000). The cap is the generic fix: a stage that rewrites
//! a feed keeps it at or below the cap.
//!
//! Each test runs one stage twice on the same moves: once with no cap on
//! any move, once with a cap on one move. It asserts:
//! 1. the capped move leaves the stage at the cap (or below it);
//! 2. every move with no cap leaves the stage with the feed of the run
//!    with no cap at all, bit for bit (the shipped parity);
//! 3. a cap above the stage's feed changes nothing.

#![expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::dressup::feed_modulation::{
    BindingConstraint, ChiploadBand, ModulationContext, ModulationStrategy, PerMoveEngagement,
    adaptive_feed_modulate,
};
use rs_cam_core::dressup::feedopt::{FeedOptParams, optimize_feed_rates};
use rs_cam_core::geo::P3;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::{MoveIntent, Toolpath};
use rs_cam_core::trace::toolpath_spans::AnnotatedToolpath;

/// The commanded feed: the feed a load rule lowered (mm/min).
const COMMANDED: f64 = 900.0;
/// The index of the capped move (move 0 is the rapid).
const CAPPED: usize = 2;

fn cutting_ceiling(mm_min: f64) -> rs_cam_core::machine::CuttingCeilingMmMin {
    rs_cam_core::machine::MachineProfile {
        max_feed_mm_min: mm_min,
        max_cutting_feed_mm_min: Some(mm_min),
        ..rs_cam_core::machine::MachineProfile::default()
    }
    .cutting_ceiling()
}

fn ctx(k: &MachineKinematics, strategy: ModulationStrategy, banded: bool) -> ModulationContext<'_> {
    ModulationContext {
        spindle_rpm: 18_000.0,
        flute_count: 2,
        cutting_ceiling: cutting_ceiling(4000.0),
        rapid_feed_mm_min: 10_000.0,
        chipload_band: banded.then(|| ChiploadBand::new(0.02, 0.08).unwrap()),
        radial_thinning: rs_cam_core::dressup::feed_modulation::RadialThinning::Applied,
        kinematics: k,
        strategy,
        mode: rs_cam_core::dressup::feed_modulation::ModulationMode::Full,
        feed_scale: 1.0,
        deflection_inputs: None,
        power_inputs: None,
        force_budget_inputs: None,
        sample_engagements: &[],
        nominal_axial_doc_mm: 2.0,
        // Every move here runs along +X with a 2 mm descent at most, which
        // classifies as a ramp, so the plunge guard touches nothing.
        plunge_rate_mm_min: 300.0,
    }
}

/// A rapid, then four 50 mm cuts at `feed`; move `CAPPED` carries `cap`.
fn straight(feed: f64, cap: Option<f64>) -> Toolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to(P3::new(0.0, 0.0, 0.0));
    for i in 0..4 {
        let x = (i + 1) as f64 * 50.0;
        tp.feed_to_with_intent(P3::new(x, 0.0, -2.0), feed, MoveIntent::ClearingCut);
    }
    tp.moves[CAPPED].feed_cap = cap;
    tp
}

fn feeds(tp: &Toolpath) -> Vec<Option<f64>> {
    tp.moves.iter().map(|m| m.move_type.feed_rate()).collect()
}

/// A full slot on every cutting move.
fn engagements(tp: &Toolpath) -> Vec<PerMoveEngagement> {
    tp.moves
        .iter()
        .map(|m| {
            if m.move_type.is_cutting() {
                PerMoveEngagement {
                    radial_woc_fraction: 1.0,
                    axial_doc_fraction: 1.0,
                    axial_doc_mm: 2.0,
                    thinning_lift: None,
                }
            } else {
                PerMoveEngagement::default()
            }
        })
        .collect()
}

/// Run the modulator on `straight(COMMANDED, cap)`: the feeds out and the
/// binding of the capped move.
fn modulate(
    strategy: ModulationStrategy,
    banded: bool,
    cap: Option<f64>,
) -> (Vec<Option<f64>>, Option<BindingConstraint>) {
    let k = MachineKinematics::shapeoko_xxl_stock();
    let mut tp = straight(COMMANDED, cap);
    let eng = engagements(&tp);
    let outcome = adaptive_feed_modulate(&mut tp, &eng, &ctx(&k, strategy, banded)).unwrap();
    let binding = outcome.per_move.get(&CAPPED).map(|&(_, b)| b);
    (feeds(&tp), binding)
}

fn assert_cap_binds(strategy: ModulationStrategy, banded: bool, cap: f64) {
    let (free, _) = modulate(strategy, banded, None);
    let (capped, binding) = modulate(strategy, banded, Some(cap));
    let free_feed = free[CAPPED].unwrap();
    let capped_feed = capped[CAPPED].unwrap();
    // The fixture must make the stage want more than the cap, or the test
    // proves nothing.
    assert!(
        free_feed > cap + 1.0,
        "{strategy:?} banded={banded}: with no cap the stage writes {free_feed}, \
         not above the cap {cap}; the fixture does not test the cap"
    );
    assert!(
        (capped_feed - cap).abs() < 1e-9,
        "{strategy:?} banded={banded}: the capped move leaves at {capped_feed}, not at its cap {cap}"
    );
    assert_eq!(binding, Some(BindingConstraint::FeedCap));
    for (i, (a, b)) in free.iter().zip(&capped).enumerate() {
        if i != CAPPED {
            assert_eq!(
                a.map(f64::to_bits),
                b.map(f64::to_bits),
                "{strategy:?} banded={banded}: move {i} has no cap, yet its feed changed"
            );
        }
    }
    // A cap above what the stage writes changes nothing.
    let (high, high_binding) = modulate(strategy, banded, Some(free_feed + 100.0));
    let (_, free_binding) = modulate(strategy, banded, None);
    assert_eq!(
        high.iter().map(|f| f.map(f64::to_bits)).collect::<Vec<_>>(),
        free.iter().map(|f| f.map(f64::to_bits)).collect::<Vec<_>>(),
        "{strategy:?} banded={banded}: a cap above the feed changed a feed"
    );
    assert_eq!(high_binding, free_binding);
}

/// ConstrainedMax raises a full slot to the band limit, 0.08 × 18000 × 2
/// = 2880 mm/min. The capped move keeps its 900.
#[test]
fn constrained_max_keeps_a_capped_move_at_its_cap() {
    assert_cap_binds(ModulationStrategy::ConstrainedMax, true, COMMANDED);
}

/// BandMid targets the band middle; a cap under it binds.
#[test]
fn band_mid_keeps_a_capped_move_at_its_cap() {
    let (free, _) = modulate(ModulationStrategy::BandMid, true, None);
    let cap = free[CAPPED].unwrap() * 0.5;
    assert_cap_binds(ModulationStrategy::BandMid, true, cap);
}

/// The bandless arm clamps the commanded feed to the machine ceiling. A
/// cap under the commanded feed binds.
#[test]
fn the_bandless_arm_keeps_a_capped_move_at_its_cap() {
    assert_cap_binds(ModulationStrategy::ConstrainedMax, false, COMMANDED * 0.5);
}

/// The feed optimisation dressup lifts a move in air to its ceiling
/// (3000 mm/min). The capped move stays at its cap; the others are
/// unchanged.
#[test]
fn feed_optimisation_keeps_a_capped_move_at_its_cap() {
    let run = |cap: Option<f64>| {
        let tool = FlatEndmill::new(6.0, 25.0);
        // Stock far below the path: every move cuts air.
        let mut stock = TriDexelStock::from_stock(0.0, 0.0, 250.0, 10.0, -50.0, -40.0, 1.0);
        let params = FeedOptParams {
            nominal_feed_rate: 1000.0,
            max_feed_rate: 3000.0,
            min_feed_rate: 200.0,
            ramp_rate: 500.0,
            air_cut_threshold: 0.05,
            plunge_rate_mm_min: None,
        };
        let tp = straight(COMMANDED, cap);
        feeds(&optimize_feed_rates(AnnotatedToolpath::new(tp), &tool, &mut stock, &params).toolpath)
    };
    let free = run(None);
    let capped = run(Some(COMMANDED));
    assert!(
        free[CAPPED].unwrap() > COMMANDED + 1.0,
        "free feed {:?}",
        free[CAPPED]
    );
    assert!(
        (capped[CAPPED].unwrap() - COMMANDED).abs() < 1e-9,
        "capped feed {:?}",
        capped[CAPPED]
    );
    for (i, (a, b)) in free.iter().zip(&capped).enumerate() {
        if i != CAPPED {
            assert_eq!(
                a.map(f64::to_bits),
                b.map(f64::to_bits),
                "move {i} has no cap, yet its feed changed"
            );
        }
    }
}
