//! `radial_engagement` must be independent of sample-step density.
//!
//! Background: prior to the width-of-cut fix, `radial_engagement` was
//! computed as engaged_area / total_area, where a cell counted as engaged
//! only if THIS stamp removed material from it. Dense overlapping samples
//! re-entered cells the previous sample had already cleared, so engagement
//! collapsed toward 0 even though the cutter was still biting at its
//! leading edge — wanaka's adaptive3d roughing read ~3% peak engagement at
//! a 0.5mm sample step on a 6mm cutter despite a 30% planner target.
//!
//! Fix: gate engagement on pre-stamp fresh material above the cutter
//! surface and measure the perpendicular extent of those cells. Width of
//! cut / tool diameter is the conventional CAM RWoC and is independent of
//! how finely the path is sampled.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::{
    dexel_stock::{StockCutDirection, TriDexelStock},
    geo::{BoundingBox3, P3},
    tool::FlatEndmill,
    toolpath::Toolpath,
};

fn build_stock_50x50x10() -> TriDexelStock {
    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(50.0, 50.0, 10.0),
    };
    TriDexelStock::from_bounds(&bbox, 0.25)
}

fn run_stepover_and_get_steady_state(sample_step_mm: f64) -> f64 {
    // Adaptive-style scenario: clear a 30mm-wide band first, then take a
    // stepover pass 1.8mm offset (30% of 6mm tool = the wanaka target).
    // First pass spans x=5..45 and y=25, second pass spans x=45..5 at
    // y=26.8. We measure engagement in the STEADY-STATE middle of the
    // second pass (x in [15, 35]) — well clear of the edge effects at
    // x=45 (start, where the cutter sees unswept material outside the
    // prior cleared band) and x=5 (end).
    let cutter = FlatEndmill::new(6.0, 25.0);
    let mut stock = build_stock_50x50x10();

    let mut toolpath = Toolpath::new();
    toolpath.rapid_to(P3::new(5.0, 25.0, 8.0));
    toolpath.feed_to(P3::new(45.0, 25.0, 8.0), 600.0);
    let first_pass_end_index = toolpath.moves.len();
    toolpath.rapid_to(P3::new(45.0, 26.8, 8.0));
    toolpath.feed_to(P3::new(5.0, 26.8, 8.0), 600.0);

    let never_cancel = || false;
    let samples = stock
        .simulate_toolpath_with_metrics_with_cancel(
            &toolpath,
            &cutter,
            StockCutDirection::FromTop,
            0,
            12_000,
            2,
            3000.0,
            sample_step_mm,
            None,
            &[],
            true,
            &never_cancel,
        )
        .expect("simulation succeeds");

    // Average steady-state engagement on the second pass.
    let mut sum = 0.0_f64;
    let mut n = 0_usize;
    for s in &samples {
        if s.move_index < first_pass_end_index || !s.is_cutting {
            continue;
        }
        let x = s.position[0];
        if !(15.0..=35.0).contains(&x) {
            continue;
        }
        sum += s.radial_engagement;
        n += 1;
    }
    if n == 0 { 0.0 } else { sum / n as f64 }
}

#[test]
fn radial_engagement_at_typical_sample_step() {
    // 30% RWoC stepover should read in the ~0.20..0.40 ballpark at a
    // typical simulator sample step (0.5mm on a 6mm tool). Before the fix,
    // this reading collapsed to ~0.03.
    //
    // Note: per-sample engagement is fundamentally a "lune of fresh
    // material between consecutive samples" measurement. Below a sample
    // step of ~tool_radius / 6 the lune perpendicular extent shrinks
    // geometrically (≈ 2·sqrt(R·step)). For sample steps that are a
    // meaningful fraction of the tool diameter (the simulator's normal
    // operating regime), the lune is wide enough that the steady-state
    // reading reflects the conventional RWoC.
    let medium = run_stepover_and_get_steady_state(0.5);
    let coarse = run_stepover_and_get_steady_state(2.0);
    eprintln!("steady-state engagement: coarse(2.0mm)={coarse:.3}, medium(0.5mm)={medium:.3}");

    for (label, val) in [("coarse", coarse), ("medium", medium)] {
        assert!(
            val > 0.15,
            "{label} steady-state engagement {val:.3} too low — width-of-cut metric \
             should be near 0.30 (1.8mm bite / 6mm tool) at typical sample density"
        );
        assert!(
            val < 0.50,
            "{label} steady-state engagement {val:.3} too high — width-of-cut metric \
             should be near 0.30"
        );
    }
}

#[test]
fn radial_engagement_full_slot_first_cut() {
    // First cut into solid stock = full slot = engagement 1.0.
    let cutter = FlatEndmill::new(6.0, 25.0);
    let mut stock = build_stock_50x50x10();

    let mut toolpath = Toolpath::new();
    toolpath.rapid_to(P3::new(10.0, 25.0, 8.0));
    toolpath.feed_to(P3::new(40.0, 25.0, 8.0), 600.0);

    let never_cancel = || false;
    let samples = stock
        .simulate_toolpath_with_metrics_with_cancel(
            &toolpath,
            &cutter,
            StockCutDirection::FromTop,
            0,
            12_000,
            2,
            3000.0,
            0.5,
            None,
            &[],
            true,
            &never_cancel,
        )
        .expect("simulation succeeds");

    let cutting: Vec<_> = samples.iter().filter(|s| s.is_cutting).collect();
    assert!(!cutting.is_empty());
    let avg = cutting.iter().map(|s| s.radial_engagement).sum::<f64>() / cutting.len() as f64;
    eprintln!("full-slot avg engagement: {avg:.3}");
    assert!(
        avg > 0.85,
        "full-slot first cut avg engagement {avg:.3} should be ~1.0"
    );
}

#[test]
fn radial_engagement_air_cut_reads_zero() {
    // A second pass identical to the first should bite no fresh material.
    let cutter = FlatEndmill::new(6.0, 25.0);
    let mut stock = build_stock_50x50x10();

    let mut toolpath = Toolpath::new();
    toolpath.rapid_to(P3::new(10.0, 25.0, 8.0));
    toolpath.feed_to(P3::new(40.0, 25.0, 8.0), 600.0);
    let first_pass_end_index = toolpath.moves.len();
    toolpath.feed_to(P3::new(10.0, 25.0, 8.0), 600.0);

    let never_cancel = || false;
    let samples = stock
        .simulate_toolpath_with_metrics_with_cancel(
            &toolpath,
            &cutter,
            StockCutDirection::FromTop,
            0,
            12_000,
            2,
            3000.0,
            0.5,
            None,
            &[],
            true,
            &never_cancel,
        )
        .expect("simulation succeeds");

    for s in samples
        .iter()
        .filter(|s| s.move_index >= first_pass_end_index && s.is_cutting)
    {
        assert!(
            s.radial_engagement < 0.05,
            "second-pass air cut sample read engagement {:.3}, expected ~0",
            s.radial_engagement
        );
    }
}
