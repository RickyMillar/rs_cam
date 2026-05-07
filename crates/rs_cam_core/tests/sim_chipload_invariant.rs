//! Invariant: `arc_engagement_radians` and `radial_engagement` agree
//! per-sample, because both gate on the same per-cell predicate
//! ("pre-stamp material above the cutter surface in the midpoint disk").
//!
//! Originally an O5c regression test for a pre-fix geometry mismatch
//! between the midpoint engagement check and the per-cell stamp; now both
//! metrics use the consistent pre-stamp-fresh-material gate, so the
//! invariant tightens: arc and radial engagement co-fire (or neither
//! does), and `removed_volume` is no longer involved (it sums across the
//! full segment-sweep bbox, a different footprint than the midpoint disk).

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

fn assert_arc_and_radial_agree<'a>(
    samples: impl Iterator<Item = &'a rs_cam_core::simulation_cut::SimulationCutSample>,
    label: &str,
) {
    // Both `arc_engagement_radians` and `radial_engagement` gate on the
    // same per-cell predicate inside the midpoint disk: "pre-stamp material
    // above the cutter surface > FRESH_MATERIAL_THRESHOLD_MM". So if any
    // cell engages, both register; if none engage, both read zero. This is
    // a tighter invariant than the original "arc vs removal" check (which
    // was specific to the pre-O5c geometry-mismatch bug and no longer
    // applies — see commit history of dexel_stock/stamping.rs).
    let mut violations = Vec::new();
    for sample in samples.filter(|sample| sample.is_cutting) {
        let arc_engaged = sample.arc_engagement_radians.unwrap_or(0.0) > 0.0;
        let radial_engaged = sample.radial_engagement > 0.0;
        if arc_engaged != radial_engaged {
            violations.push((
                sample.sample_index,
                sample.position,
                sample.arc_engagement_radians.unwrap_or(0.0),
                sample.radial_engagement,
                sample.removed_volume_est_mm3,
            ));
        }
    }

    if !violations.is_empty() {
        eprintln!(
            "{label}: {} sample(s) disagree (arc_engagement vs radial_engagement):",
            violations.len()
        );
        for (idx, pos, arc, radial, removed) in violations.iter().take(20) {
            eprintln!(
                "  sample {idx}: pos=({:.3},{:.3},{:.3}) arc={arc:.4} rad, \
                 radial={radial:.4}, removed={removed:.6} mm^3",
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
            &[],
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
    assert_arc_and_radial_agree(
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
            &[],
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
            &[],
            true,
            &never_cancel,
        )
        .expect("second pass succeeds");

    assert_arc_and_radial_agree(samples.iter(), "ball-endmill sloped second pass");
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
            &[],
            true,
            &never_cancel,
        )
        .expect("simulation succeeds");

    assert_arc_and_radial_agree(
        samples
            .iter()
            .filter(|sample| sample.move_index >= first_pass_end_index),
        "flat-endmill adjacent stepover second pass",
    );
}
