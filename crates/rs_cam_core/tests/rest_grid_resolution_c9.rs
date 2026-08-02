//! C9 sub-item 4 — what the shipped 0.5 mm rest cell misses, measured on the
//! PRODUCTION detector.
//!
//! `checkpoint_c9_sampled_reach.rs` answers the resolution question on the
//! reach model, analytically. This file answers the other two thirds of it on
//! the shipped path — `rest_field::detect_rest_valleys` over a real mesh — for
//! the two consequences a coarse cell has that an analytic cross-section
//! cannot show:
//!
//! * **feature detection** — does the branch get FOUND at all, and how much of
//!   it? Measured as centreline count and total centreline length.
//! * **claims fan width** — what the detector's per-point reach says the fan
//!   can span, which is what `crease_paths::centerline_cut_paths` then emits.
//!
//! The independent variable is `RestFieldParams::cell_mm` and nothing else:
//! same mesh, same tool, same reference, same thresholds. The fixtures are
//! sized at the Ø1 TIP scale, because that is where the plan's question lives
//! — a 0.5 mm cell against a 0.5 mm tip radius is one sample per tool radius,
//! and the question is what that costs.
//!
//! No behavioural change is made by this file.
//!
//! # OPEN ANOMALY — read this before quoting any number below
//!
//! The sweep did not produce the expected "finer grid resolves more detail"
//! curve. It produced the OPPOSITE, on both fixtures, and the C9 wave did not
//! get to the bottom of it:
//!
//! * On the tip-scale groove the SHIPPED 0.5 mm cell finds one centreline of
//!   19.0 mm, and the 0.25 mm and 0.10 mm cells find **nothing at all**.
//! * On the wide control all three cells find both grooves (38.0 / 37.0 /
//!   36.4 mm — flat, as a control should be), but the median per-point reach
//!   **collapses** with refinement: 0.583 mm → 0.242 mm → 0.000 mm. At the
//!   shipped cell that is 2 fan passes at a 0.25 mm stepover; at 0.1 mm it is
//!   none.
//!
//! Both readings cannot be right, and the coarse one is the suspicious one:
//! `rest_field::measure_cross_section` reports `rim_distance = rim_cells ×
//! cell`, so the rim distance a coarse walk reports is quantised UP to the
//! cell, and reach is `rim_distance − profile_rise`. A reach that shrinks
//! toward zero as the measurement gets finer is the signature of a reach that
//! was partly an artefact of the measurement. A hand-check of the wide
//! control says neither end of the sweep is obviously the true answer: a Ø1
//! tip on a 7° cone is ≈ 0.65 mm wide at 1.2 mm depth inside a groove ≈ 2.06
//! mm half-wide at that depth, which argues for roughly 1.4 mm of reach —
//! more than the coarse reading and far more than the fine one.
//!
//! So this file asserts only what it can defend (the fixtures run; the coarse
//! arm detects; the control's detected LENGTH is flat across the sweep) and
//! records the rest as an open question. It is deliberately NOT the basis of
//! the C9 rest-cell recommendation — that comes from
//! `checkpoint_c9_sampled_reach.rs`, which measures the reach model against
//! analytic ground truth and therefore has a right answer to compare to.
//! Diagnosing this belongs to whoever next touches `measure_cross_section`,
//! and the first thing to check is whether the ridge polyline and its
//! perpendicular walk survive refinement at all.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::geo::polyline_length;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::rest_field::{RestFieldParams, RestReference, detect_rest_valleys};
use rs_cam_core::tool::{BallEndmill, MillingCutter};

mod common;

use common::meshes::GroovedBlock;
use common::tools::wanaka_taper;

/// Cell sizes swept. `0.5` is `RestFieldParams::default`'s shipped value.
const CELLS: [f64; 3] = [0.5, 0.25, 0.1];

struct Measured {
    cell: f64,
    centerlines: usize,
    total_length_mm: f64,
    /// Median over sampled points of the narrow-side reach — the scalar the
    /// routing criterion compares and the fan is sized from.
    median_reach_mm: f64,
    widest_reach_mm: f64,
    /// Points whose reach the policy REFUSED (tip float).
    refused_points: usize,
    sampled_points: usize,
}

fn measure(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    cell: f64,
) -> Measured {
    // A Ø12 ball as the rest reference — the same reference the Checkpoint A
    // end-to-end probes use, chosen there because it cannot get anywhere near
    // these grooves, so the groove IS the rest field. Same reference at every
    // cell size.
    let reference = BallEndmill::new(12.0, 25.0);
    let rf = detect_rest_valleys(
        mesh,
        index,
        cutter,
        RestReference::Cutter {
            tool: &reference as &dyn MillingCutter,
            is_surface_probe: false,
        },
        &RestFieldParams {
            cell_mm: cell,
            min_valley_depth: 0.05,
            offset_stepover_mm: 0.25,
            num_offset_passes_cap: 8,
            min_cut_length: 2.0,
            region_margin_mm: 0.5,
        },
    );
    let mut reaches: Vec<f64> = Vec::new();
    let mut refused = 0usize;
    for cl in rf.centerlines.iter() {
        for s in cl.samples.iter() {
            if s.reach.refused {
                refused += 1;
            }
            reaches.push(s.reach.min_mm());
        }
    }
    reaches.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = if reaches.is_empty() {
        0.0
    } else {
        reaches[reaches.len() / 2]
    };
    Measured {
        cell,
        centerlines: rf.centerlines.len(),
        total_length_mm: rf
            .centerlines
            .iter()
            .map(|cl| polyline_length(&cl.points))
            .sum(),
        median_reach_mm: median,
        widest_reach_mm: reaches.last().copied().unwrap_or(0.0),
        refused_points: refused,
        sampled_points: reaches.len(),
    }
}

fn report(label: &str, rows: &[Measured]) {
    println!();
    println!("== C9 sub-item 4 — {label} ==");
    for m in rows {
        println!(
            "  cell {:.2} mm  centrelines={:2}  total length={:8.3} mm  \
             sampled pts={:5}  median reach={:.4} mm  widest={:.4} mm  refused={}",
            m.cell,
            m.centerlines,
            m.total_length_mm,
            m.sampled_points,
            m.median_reach_mm,
            m.widest_reach_mm,
            m.refused_points,
        );
    }
    if let (Some(coarse), Some(fine)) = (rows.first(), rows.last()) {
        let len_ratio = if fine.total_length_mm > 0.0 {
            coarse.total_length_mm / fine.total_length_mm
        } else {
            f64::NAN
        };
        println!(
            "  shipped 0.50 mm cell vs {:.2} mm: length {:.1}% of the fine \
             reading, median reach {:+.4} mm, fan half-band at 0.25 mm \
             stepover {:.0} vs {:.0} passes",
            fine.cell,
            100.0 * len_ratio,
            coarse.median_reach_mm - fine.median_reach_mm,
            (coarse.median_reach_mm / 0.25).floor(),
            (fine.median_reach_mm / 0.25).floor(),
        );
    }
}

/// A groove at the TIP scale — 0.8 mm rim half-width against a 0.5 mm tip
/// radius, so the whole feature spans between three and sixteen cells
/// depending on the sweep. This is the regime the plan's question is about.
#[test]
fn a_tip_scale_groove_is_measured_across_the_rest_cell_sweep() {
    let mesh = GroovedBlock::new(0.8, 70.0, 1.2).dense_step(0.1).build();
    let index = SpatialIndex::build(&mesh, 5.0);
    let cutter = wanaka_taper();
    let rows: Vec<Measured> = CELLS
        .iter()
        .map(|&c| measure(&mesh, &index, &cutter, c))
        .collect();
    report(
        "tip-scale groove (rim half-width 0.8 mm, 70 deg walls)",
        &rows,
    );

    // Only what this fixture can defend. The expected assertion — the fine
    // reading finds at least as much feature as the coarse one — is exactly
    // what FAILS here, and pinning the failure as if it were the intended
    // behaviour would launder an open anomaly into a sentry. See the module
    // doc.
    let coarse = rows.first().expect("swept at least one cell");
    assert!(
        coarse.centerlines > 0 && coarse.total_length_mm > 1.0,
        "the shipped 0.50 mm cell found nothing on a groove it is supposed to \
         detect — the fixture is broken, not the grid"
    );
    let fine = rows.last().expect("swept at least one cell");
    assert!(
        fine.centerlines == 0,
        "the 0.10 mm cell now finds {} centreline(s) on the tip-scale groove \
         where the C9 wave measured NONE. That is the open anomaly in this \
         file's module doc changing behaviour — good news, probably, but it \
         must be re-read and the doc rewritten, not silently absorbed",
        fine.centerlines
    );
}

/// A wider groove the reference tool still cannot enter — the control. Here
/// the feature is many cells across at EVERY sweep point, so cell size should
/// cost little, and the contrast with the tip-scale fixture above is what
/// isolates "the cell is coarse relative to the FEATURE" from "the cell is
/// coarse, full stop".
#[test]
fn a_wide_groove_is_the_control_for_the_same_sweep() {
    let mesh = GroovedBlock::new(2.5, 70.0, 1.2).dense_step(0.1).build();
    let index = SpatialIndex::build(&mesh, 5.0);
    let cutter = wanaka_taper();
    let rows: Vec<Measured> = CELLS
        .iter()
        .map(|&c| measure(&mesh, &index, &cutter, c))
        .collect();
    report(
        "wide groove control (rim half-width 2.5 mm, 70 deg walls)",
        &rows,
    );

    let fine = rows.last().expect("swept at least one cell");
    let coarse = rows.first().expect("swept at least one cell");
    assert!(
        fine.centerlines > 0,
        "the control fixture produced no centrelines"
    );
    assert!(
        coarse.centerlines > 0,
        "the shipped cell lost a groove five cells wide — that would be a \
         detection defect, not a resolution trade"
    );
    // What the control DOES establish: detected LENGTH is stable under
    // refinement (within 10%), so the tip-scale fixture's total loss of
    // detection is a property of that feature's scale, not of refinement in
    // general. This is the half of the anomaly that is pinned.
    assert!(
        (coarse.total_length_mm - fine.total_length_mm).abs() <= 0.1 * fine.total_length_mm,
        "control centreline length moved from {:.3} mm to {:.3} mm across the \
         sweep — the control is no longer controlling for anything",
        coarse.total_length_mm,
        fine.total_length_mm
    );
    // The reach collapse, pinned as the open anomaly it is: the coarse cell
    // reports MORE reach than the fine one on identical geometry.
    assert!(
        coarse.median_reach_mm > fine.median_reach_mm,
        "the reach collapse recorded in this file's module doc no longer \
         reproduces ({:.4} mm coarse vs {:.4} mm fine) — re-read the anomaly \
         and rewrite the doc rather than deleting this line",
        coarse.median_reach_mm,
        fine.median_reach_mm
    );
}
