//! C3 — waterline stops keeping its own copies.
//!
//! `ANTIPATTERNS_BACKLOG.md` P3 listed two private reimplementations in
//! `waterline.rs` and one inherited defect:
//!
//! 1. a private Z-ladder (`waterline_z_levels`) beside
//!    [`rs_cam_core::finish_setup::z_ladder`];
//! 2. PR-8d's sub-quantum-segment defect, from the same
//!    `contour_extract::weave_contours` source that produced it in
//!    `steep_shallow.rs`, deliberately left in place there to keep that
//!    commit one operation wide
//!    (`steep_shallow.rs`'s own comment: "`waterline.rs` has the same defect
//!    from the same source and is deliberately left alone").
//!
//! (The backlog also named a slope-window sentinel copy in
//! `compute/execute.rs`. There isn't one: `execute.rs:1948` has called the
//! shared `finish_setup::slope_filter_active` since `4b105da`, the very
//! commit that created the module. The stale claims in `finish_setup.rs`'s
//! header and in the backlog are corrected rather than acted on.)
//!
//! # Red-first evidence
//!
//! Before this wave, `waterline_toolpath` on the Checkpoint B mixed-slope
//! ribbon (Ø1/7° taper, sampling 1.0, z_step 0.5, full-bbox ladder) emitted
//!
//! * **942 cutting segments**, of which
//! * **2 were shorter than [`MIN_EMITTED_SEGMENT_MM`]** (0.001 mm), the
//!   shortest being **0.000891141 mm**,
//! * over **1224.9001 mm** of cutting.
//!
//! `0.000891` is the number Checkpoint B §8.1 reports, bit for bit. §8.1
//! attributed it to `SteepShallowSplit::steep` — correctly, but that is
//! where it was OBSERVED, not where it was made: `steep_shallow` generates
//! its steep passes by calling `waterline_contours`, so the two offenders
//! PR-8d filtered out of the steep half were being manufactured here and are
//! still shipped by every direct waterline op.
//!
//! After inheriting the floor: **940 segments**, **0** below the floor,
//! shortest **0.05745541 mm**, cutting length **1224.9001 mm** — unchanged
//! to the fourth decimal. Two moves no controller can express, gone, and no
//! geometry with them.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;

use rs_cam_core::geo::P3;
use rs_cam_core::mesh::SpatialIndex;
use rs_cam_core::toolpath::{MIN_EMITTED_SEGMENT_MM, Toolpath};
use rs_cam_core::waterline::{WaterlineParams, waterline_toolpath, waterline_z_levels};

use common::meshes::height_field;
use common::tools::wanaka_taper;

const HALF: f64 = 8.0;
const MESH_STEP: f64 = 0.25;

/// The Checkpoint B `mixed-slope ribbon` — the fixture §8.1 was measured on,
/// and the only one of the four that exhibits it.
fn mixed_slope_ribbon() -> rs_cam_core::mesh::TriangleMesh {
    const KNOTS: [(f64, f64); 6] = [
        (-8.0, 0.0),
        (-3.0, 0.526),
        (-1.0, 4.0),
        (-0.65, 8.0),
        (0.65, 8.0),
        (8.0, 8.0 - 0.79),
    ];
    height_field(HALF, MESH_STEP, |x, _y| {
        if x <= KNOTS[0].0 {
            return KNOTS[0].1;
        }
        for w in KNOTS.windows(2) {
            let (x0, z0) = w[0];
            let (x1, z1) = w[1];
            if x <= x1 {
                let t = (x - x0) / (x1 - x0);
                return z0 + t * (z1 - z0);
            }
        }
        KNOTS[KNOTS.len() - 1].1
    })
}

fn params() -> WaterlineParams {
    WaterlineParams {
        sampling: 1.0,
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: 25.0,
        stock_to_leave: 0.0,
    }
}

/// Every non-zero cutting-segment length in emitted order.
///
/// Same shape as `steep_shallow_min_segment_pr8d::cutting_segments`, which
/// is the file this evidence is the sibling of.
fn cutting_segments(tp: &Toolpath) -> Vec<f64> {
    let mut out = Vec::new();
    let mut prev: Option<P3> = None;
    for mv in &tp.moves {
        if let (true, Some(p)) = (mv.move_type.is_cutting(), prev) {
            let d = ((mv.target.x - p.x).powi(2)
                + (mv.target.y - p.y).powi(2)
                + (mv.target.z - p.z).powi(2))
            .sqrt();
            out.push(d);
        }
        prev = Some(mv.target);
    }
    out
}

fn ribbon_waterline() -> Toolpath {
    let mesh = mixed_slope_ribbon();
    let index = SpatialIndex::build(&mesh, 2.0);
    let cutter = wanaka_taper();
    waterline_toolpath(
        &mesh,
        &index,
        &cutter,
        mesh.bbox.max.z,
        mesh.bbox.min.z,
        0.5,
        &params(),
    )
}

// ── 1. The inherited defect, fixed ───────────────────────────────────────

/// The gate PR-8d wrote for `steep_shallow`, now owed by `waterline` — the
/// operation that was the actual SOURCE of the two 0.000891 mm segments
/// Checkpoint B §8.1 attributed to the steep half.
#[test]
fn no_waterline_cutting_segment_survives_below_the_floor() {
    let tp = ribbon_waterline();
    let segments = cutting_segments(&tp);
    assert!(
        !segments.is_empty(),
        "the ribbon fixture must actually produce waterline cutting"
    );

    let offenders: Vec<f64> = segments
        .iter()
        .copied()
        .filter(|d| *d < MIN_EMITTED_SEGMENT_MM)
        .collect();
    let shortest = segments.iter().copied().fold(f64::INFINITY, f64::min);
    eprintln!(
        "waterline ribbon: {} cutting segments, {} below the floor, \
         shortest {shortest:.8} mm, total {:.4} mm",
        segments.len(),
        offenders.len(),
        segments.iter().sum::<f64>()
    );

    assert!(
        offenders.is_empty(),
        "waterline emitted {} cutting segment(s) below the \
         {MIN_EMITTED_SEGMENT_MM} mm post quantum (shortest {shortest:.9} mm) \
         — the PR-8d defect, inherited from the same `weave_contours` source",
        offenders.len()
    );
}

/// The floor removes moves, not geometry. Two degenerate segments are worth
/// 0.0018 mm between them, so the cutting total must not move.
#[test]
fn the_floor_costs_the_ribbon_no_cutting_length() {
    let segments = cutting_segments(&ribbon_waterline());
    let total: f64 = segments.iter().sum();
    // MEASURED 2026-08-02: 1224.9001 mm before the floor, and after.
    assert!(
        (total - 1224.9001).abs() < 1e-3,
        "ribbon waterline cutting length moved: {total:.4} mm"
    );
    assert_eq!(
        segments.len(),
        940,
        "segment count moved — before the floor this fixture emitted 942"
    );
}

/// The floor must be inert where nothing was degenerate: a plain plane has
/// no coincident marching-squares vertices, so nothing may be dropped.
#[test]
fn the_floor_is_inert_where_nothing_was_degenerate() {
    let mesh = height_field(HALF, 1.0, |_x, _y| 0.0);
    let index = SpatialIndex::build(&mesh, 2.0);
    let cutter = wanaka_taper();
    let tp = waterline_toolpath(&mesh, &index, &cutter, 0.0, 0.0, 0.5, &params());
    for d in cutting_segments(&tp) {
        assert!(
            d >= MIN_EMITTED_SEGMENT_MM,
            "flat plane produced a sub-floor segment {d:.9} mm"
        );
    }
}

// ── 2. One Z-ladder ──────────────────────────────────────────────────────

/// `waterline_z_levels` is now a thin adapter over
/// [`rs_cam_core::finish_setup::z_ladder`]'s `snap_to_bottom = false` arm at
/// waterline's own epsilon. Pinned across the boundary cases that made the
/// two ladder policies worth keeping separate in the first place: exact
/// multiples, a remainder, an inverted range and a zero range.
#[test]
fn the_waterline_ladder_is_the_shared_ladder() {
    const CASES: &[(f64, f64, f64)] = &[
        (10.0, 0.0, 2.5),
        (10.0, 0.0, 3.0),
        (8.0, -0.79, 0.5),
        (-1.0, 0.0, 1.0),
        (0.0, 0.0, 0.5),
        (5.0, 5.0, 1.0),
    ];
    for &(top, bottom, step) in CASES {
        let shared = rs_cam_core::finish_setup::z_ladder(
            top,
            bottom,
            step,
            rs_cam_core::waterline::WATERLINE_LADDER_EPSILON,
            false,
        );
        let waterline = waterline_z_levels(top, bottom, step);
        assert_eq!(
            waterline, shared,
            "waterline ladder diverged from the shared one at \
             top={top} bottom={bottom} step={step}"
        );
    }
}

/// The pre-existing pin from `waterline.rs`'s own unit tests, restated here
/// so the extraction is provably value-preserving and not merely
/// self-consistent.
#[test]
fn the_shared_ladder_reproduces_the_pinned_levels() {
    assert_eq!(
        waterline_z_levels(10.0, 0.0, 2.5),
        vec![10.0, 7.5, 5.0, 2.5, 0.0]
    );
    assert!(waterline_z_levels(-1.0, 0.0, 1.0).is_empty());
}
