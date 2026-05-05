//! Invariant: the simulator's per-sample `arc_engagement_radians` must agree
//! with what the same sample's stamp actually removed from the dexel stock.
//!
//! Repro from the AGENTSEARCH_NEXT_SESSION worktree O5c plan: a "second pass"
//! through previously-cleared stock should not report nonzero arc engagement
//! when the per-cell stamps remove no material (and vice versa). The two
//! metrics are computed in the same loop in
//! `dexel_stock::stamping::stamp_segment_with_metrics`, but until this fix
//! they used DIFFERENT geometries:
//!   - engagement check: midpoint disk against pre-stamp ray top
//!   - per-cell stamp:   per-cell t-projected depth + per-cell radial profile
//! For sloped segments and non-flat tools the two geometries disagree at
//! off-axis cells, leaking phantom engagement into samples that removed no
//! material (or hiding real engagement when stamps did remove material).

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
    tool::{BallEndmill, FlatEndmill},
    toolpath::Toolpath,
};

fn build_stock_50x50x10() -> TriDexelStock {
    // 50x50x10 mm rectangular stock with the top at z=10, bottom at z=0.
    let bbox = BoundingBox3 {
        min: P3::new(0.0, 0.0, 0.0),
        max: P3::new(50.0, 50.0, 10.0),
    };
    TriDexelStock::from_bounds(&bbox, 0.25)
}

fn assert_arc_and_removal_agree<'a>(
    samples: impl Iterator<Item = &'a rs_cam_core::simulation_cut::SimulationCutSample>,
    label: &str,
) {
    let mut violations = Vec::new();
    for sample in samples.filter(|sample| sample.is_cutting) {
        let arc = sample.arc_engagement_radians.unwrap_or(0.0);
        let removed = sample.removed_volume_est_mm3;
        let arc_says_engaged = arc > 0.05;
        let stamp_removed = removed > 0.0;
        if arc_says_engaged != stamp_removed {
            violations.push((sample.sample_index, sample.position, arc, removed));
        }
    }

    if !violations.is_empty() {
        eprintln!(
            "{label}: {} sample(s) disagree (arc_engagement vs stamp removal):",
            violations.len()
        );
        for (idx, pos, arc, removed) in violations.iter().take(20) {
            eprintln!(
                "  sample {idx}: pos=({:.3},{:.3},{:.3}) arc={arc:.4} rad, removed={removed:.6} mm^3",
                pos[0], pos[1], pos[2],
            );
        }
        panic!("{label}: {} sample(s) disagree", violations.len());
    }
}

#[test]
fn flat_endmill_second_pass_through_cleared_strip_agrees() {
    // The exact scenario from AGENTSEARCH_NEXT_SESSION.md (O5c):
    // 50x50x10 mm stock, 6mm flat endmill, sweep at z=8 from x=10 to x=40,
    // then sweep back at the SAME z. The second sweep enters fully-cleared
    // territory and must report both arc=0 and removed=0 on every sample.
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
            2.0,
            None,
            true,
            &never_cancel,
        )
        .expect("simulation succeeds");

    assert!(!samples.is_empty(), "simulation should emit samples");

    // Sanity: at least one first-pass sample actually engaged material.
    let first_pass_engaged = samples
        .iter()
        .filter(|sample| sample.move_index < first_pass_end_index)
        .any(|sample| {
            sample.arc_engagement_radians.unwrap_or(0.0) > 0.05
                && sample.removed_volume_est_mm3 > 0.0
        });
    assert!(
        first_pass_engaged,
        "first pass should engage and remove material"
    );

    // The invariant on the second pass through the cleared trail.
    assert_arc_and_removal_agree(
        samples
            .iter()
            .filter(|sample| sample.move_index >= first_pass_end_index),
        "flat-endmill second pass through cleared strip",
    );
}

#[test]
fn ball_endmill_sloped_second_pass_through_cleared_strip_agrees() {
    // Same fixture as above but with a ball endmill on a sloped second pass
    // (z=9 -> z=7 over the cleared trail). The midpoint vs per-cell geometry
    // disagreement is most visible here: a ball endmill has a non-flat radial
    // profile, so off-axis cells in a sloped segment have stamp depths that
    // differ from the midpoint depth used by the engagement check. Without
    // the fix, samples in the second pass report stamp removal that the
    // engagement check missed (or vice versa).
    let cutter = BallEndmill::new(6.0, 25.0);
    let flat = FlatEndmill::new(6.0, 25.0);
    let mut stock = build_stock_50x50x10();

    let never_cancel = || false;

    // First pass: clear z=8 along y=25 with a flat endmill.
    let mut clear_pass = Toolpath::new();
    clear_pass.rapid_to(P3::new(10.0, 25.0, 8.0));
    clear_pass.feed_to(P3::new(40.0, 25.0, 8.0), 600.0);
    stock
        .simulate_toolpath_with_metrics_with_cancel(
            &clear_pass,
            &flat,
            StockCutDirection::FromTop,
            0,
            12_000,
            2,
            3000.0,
            2.0,
            None,
            true,
            &never_cancel,
        )
        .expect("first pass succeeds");

    // Second pass: ball endmill on a sloped trajectory through the cleared
    // strip.
    let mut second_pass = Toolpath::new();
    second_pass.rapid_to(P3::new(10.0, 25.0, 9.0));
    second_pass.feed_to(P3::new(40.0, 25.0, 7.0), 600.0);
    let samples = stock
        .simulate_toolpath_with_metrics_with_cancel(
            &second_pass,
            &cutter,
            StockCutDirection::FromTop,
            1,
            12_000,
            2,
            3000.0,
            1.0,
            None,
            true,
            &never_cancel,
        )
        .expect("second pass succeeds");

    assert_arc_and_removal_agree(samples.iter(), "ball-endmill sloped second pass");
}

#[test]
fn flat_endmill_adjacent_stepover_pass_agrees() {
    // Stepover scenario: parallel passes 3mm apart with a 6mm cutter, so the
    // second pass overlaps the first by 50%. Every cutting sample on the
    // second pass should have agreement between arc and removal.
    let cutter = FlatEndmill::new(6.0, 25.0);
    let mut stock = build_stock_50x50x10();

    let mut toolpath = Toolpath::new();
    toolpath.rapid_to(P3::new(10.0, 25.0, 8.0));
    toolpath.feed_to(P3::new(40.0, 25.0, 8.0), 600.0);
    let first_pass_end_index = toolpath.moves.len();
    toolpath.rapid_to(P3::new(40.0, 28.0, 8.0));
    toolpath.feed_to(P3::new(10.0, 28.0, 8.0), 600.0);

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
            2.0,
            None,
            true,
            &never_cancel,
        )
        .expect("simulation succeeds");

    assert_arc_and_removal_agree(
        samples
            .iter()
            .filter(|sample| sample.move_index >= first_pass_end_index),
        "flat-endmill adjacent stepover second pass",
    );
}
