//! **Measure-only instrument for Checkpoint B Q4's second half.**
//!
//! `planning/review_2026-08-04/CHIPLOAD_LITERATURE_VERDICT.md` §4
//! recommends the LUT path adopt the formula path's exponents —
//! `chipload ∝ D^0.61` (retiring `^1.0`) and `chipload ∝ Janka^-0.5`
//! (also retiring `^1.0`). Checkpoint B Q4 ruled that **implementation
//! returns as a separate approval with magnitudes**. This file produces
//! the magnitudes and changes nothing: it reads
//! `vendor_lookup::{CHIPLOAD_DIAMETER_EXPONENT, CHIPLOAD_HARDNESS_EXPONENT}`
//! (both 1.0, shipped) and applies the proposed values *in the test*.
//!
//! Output goes to `planning/review_2026-08-04/LAW_MAGNITUDE_TABLES.md`
//! for the operator. `#[ignore]`d because it is a report, not a gate —
//! nothing here can fail in a way that means the crate is broken, and a
//! reporting harness that occupies the Cargo slot on every run is a tax.
//!
//! Committed the moment it lints clean, per
//! `feedback_commit_instruments_before_gates`: the tables in the
//! planning doc are only trustworthy if the thing that produced them is
//! in the tree and re-runnable.
//!
//! Run with:
//! ```text
//! cargo test -p rs_cam_core --test law_magnitude_measurement -- --ignored --nocapture
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr
)]

use rs_cam_core::feeds::ToolGeometryHint;
use rs_cam_core::feeds::embedded_vendor_lut;
use rs_cam_core::feeds::vendor_lookup::{
    CHIPLOAD_DIAMETER_EXPONENT, CHIPLOAD_HARDNESS_EXPONENT, LookupQuery, apply_chipload_law,
    find_best_chip_envelope_row, is_extrapolated_for_ratios,
};
use rs_cam_core::feeds::vendor_lut::{
    HardnessKind, LutOperationFamily, LutPassRole, MaterialFamily, ToolFamily,
};

/// B-lit §4.1's recommendation. **Not shipped.**
const PROPOSED_DIAMETER_EXPONENT: f64 = 0.61;
/// B-lit §4.2's recommendation. **Not shipped.**
const PROPOSED_HARDNESS_EXPONENT: f64 = 0.5;

struct Cell {
    label: &'static str,
    tool_family: ToolFamily,
    hint: ToolGeometryHint,
    diameter_mm: f64,
    flute_count: u32,
    material_family: MaterialFamily,
    janka: f64,
    operation_family: LutOperationFamily,
    pass_role: LutPassRole,
}

fn cells() -> Vec<Cell> {
    vec![
        Cell {
            label: "_litmatrix_ipe_janka_scaling / oak (Ø3 flat 2F pocket rough)",
            tool_family: ToolFamily::FlatEnd,
            hint: ToolGeometryHint::Flat,
            diameter_mm: 3.0,
            flute_count: 2,
            material_family: MaterialFamily::Hardwood,
            janka: 1360.0,
            operation_family: LutOperationFamily::Pocket,
            pass_role: LutPassRole::Roughing,
        },
        Cell {
            label: "_litmatrix_ipe_janka_scaling / ipe (Ø3 flat 2F pocket rough)",
            tool_family: ToolFamily::FlatEnd,
            hint: ToolGeometryHint::Flat,
            diameter_mm: 3.0,
            flute_count: 2,
            material_family: MaterialFamily::Hardwood,
            janka: 3510.0,
            operation_family: LutOperationFamily::Pocket,
            pass_role: LutPassRole::Roughing,
        },
        Cell {
            label: "_litmatrix_rubbing_floor_clamp / ipe (Ø6 flat 2F pocket rough)",
            tool_family: ToolFamily::FlatEnd,
            hint: ToolGeometryHint::Flat,
            diameter_mm: 6.0,
            flute_count: 2,
            material_family: MaterialFamily::Hardwood,
            janka: 3510.0,
            operation_family: LutOperationFamily::Pocket,
            pass_role: LutPassRole::Roughing,
        },
        Cell {
            label: "_litmatrix_rubbing_floor_clamp / oak control (Ø6 flat 2F pocket rough)",
            tool_family: ToolFamily::FlatEnd,
            hint: ToolGeometryHint::Flat,
            diameter_mm: 6.0,
            flute_count: 2,
            material_family: MaterialFamily::Hardwood,
            janka: 1360.0,
            operation_family: LutOperationFamily::Pocket,
            pass_role: LutPassRole::Roughing,
        },
        Cell {
            label: "_litmatrix_rpm_only_lut_chipload (Ø12 flat 4F adaptive rough, oak)",
            tool_family: ToolFamily::FlatEnd,
            hint: ToolGeometryHint::Flat,
            diameter_mm: 12.0,
            flute_count: 4,
            material_family: MaterialFamily::Hardwood,
            janka: 1360.0,
            operation_family: LutOperationFamily::Adaptive,
            pass_role: LutPassRole::Roughing,
        },
        Cell {
            label: "B3 live cell (Ø1 tapered ball 2F scallop finish, hard maple 1450)",
            tool_family: ToolFamily::TaperedBallNose,
            hint: ToolGeometryHint::TaperedBall {
                tip_radius: 0.5,
                taper_angle_deg: 5.26,
            },
            diameter_mm: 1.0,
            flute_count: 2,
            material_family: MaterialFamily::Hardwood,
            janka: 1450.0,
            operation_family: LutOperationFamily::Scallop,
            pass_role: LutPassRole::Finish,
        },
        Cell {
            label: "chipload-gate in-module cell (Ø6 flat 2F pocket rough, hard maple 1450)",
            tool_family: ToolFamily::FlatEnd,
            hint: ToolGeometryHint::Flat,
            diameter_mm: 6.0,
            flute_count: 2,
            material_family: MaterialFamily::Hardwood,
            janka: 1450.0,
            operation_family: LutOperationFamily::Pocket,
            pass_role: LutPassRole::Roughing,
        },
    ]
}

#[test]
#[ignore = "measurement harness — produces LAW_MAGNITUDE_TABLES.md, not a gate"]
fn measure_law_magnitudes_on_the_named_cells() {
    println!(
        "\nshipped exponents: diameter ^{CHIPLOAD_DIAMETER_EXPONENT}, \
         hardness ^{CHIPLOAD_HARDNESS_EXPONENT}"
    );
    println!(
        "proposed (B-lit §4, NOT adopted): diameter ^{PROPOSED_DIAMETER_EXPONENT}, \
         hardness ^{PROPOSED_HARDNESS_EXPONENT}\n"
    );
    println!(
        "| cell | winning row | raw d | raw h | band today (mm/tooth) | band proposed | band × | flag today | flag proposed |"
    );
    println!("|---|---|---:|---:|---|---|---:|---|---|");
    for c in cells() {
        let query = LookupQuery {
            tool_family: c.tool_family,
            tool_subfamily: None,
            diameter_mm: c.diameter_mm,
            flute_count: c.flute_count,
            material_family: c.material_family,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(c.janka),
            operation_family: c.operation_family,
            pass_role: c.pass_role,
        };
        let Some(row) = find_best_chip_envelope_row(embedded_vendor_lut(), &query, &c.hint) else {
            println!("| {} | **NO ROW MATCHED** | | | | | | | |", c.label);
            continue;
        };
        let (draw, hraw) = (
            row.chipload_diameter_ratio_raw,
            row.chipload_hardness_ratio_raw,
        );
        let today = apply_chipload_law(draw, CHIPLOAD_DIAMETER_EXPONENT)
            * apply_chipload_law(hraw, CHIPLOAD_HARDNESS_EXPONENT);
        let proposed = apply_chipload_law(draw, PROPOSED_DIAMETER_EXPONENT)
            * apply_chipload_law(hraw, PROPOSED_HARDNESS_EXPONENT);
        // The bounds on `row` are already scaled by `today`; recover the
        // unscaled row values and re-apply the proposed scale.
        let unscale = |v: Option<f64>| v.map(|x| x / today);
        let (rmin, rmax) = (unscale(row.chip_load_min_mm), unscale(row.chip_load_max_mm));
        let fmt = |lo: Option<f64>, hi: Option<f64>, k: f64| match (lo, hi) {
            (Some(a), Some(b)) => format!("{:.5}–{:.5}", a * k, b * k),
            (None, Some(b)) => format!("—–{:.5}", b * k),
            _ => "(no band)".to_owned(),
        };
        // The rider: the flag reads the RAW ratios, so it does not move.
        // Reported beside what it WOULD have been on the applied scale.
        let flag_today = row.is_extrapolated;
        let flag_if_on_applied = proposed.ln().abs()
            > rs_cam_core::feeds::vendor_lookup::CHIPLOAD_EXTRAPOLATION_LN_THRESHOLD;
        println!(
            "| {} | `{}` (Ø{:.3}) | {draw:.4} | {hraw:.4} | {} | {} | ×{:.3} | {} | {} (raw-rule: {}) |",
            c.label,
            row.observation_id,
            row.row_diameter_mm,
            fmt(rmin, rmax, today),
            fmt(rmin, rmax, proposed),
            proposed / today,
            flag_today,
            flag_if_on_applied,
            is_extrapolated_for_ratios(draw, hraw),
        );
    }
    println!();
}

#[test]
#[ignore = "measurement harness — whole-LUT sweep for LAW_MAGNITUDE_TABLES.md"]
fn measure_law_magnitudes_across_the_whole_lut() {
    // Per-row, per-query-diameter band multiplier. Reported as
    // quantiles so a reader can see the shape, plus the named worst
    // rows the approval needs.
    let lut = embedded_vendor_lut();
    let rows: Vec<_> = lut
        .observations
        .iter()
        .filter(|o| o.chipload_max_mm_tooth.is_some() && o.diameter_mm.is_some())
        .collect();
    println!(
        "\nchipload-bearing rows with a diameter anchor: {}",
        rows.len()
    );
    println!(
        "\n| query Ø | min × | p25 | median | p75 | max × | most-lowered row | most-raised row |"
    );
    println!("|---:|---:|---:|---:|---:|---:|---|---|");
    for q in [1.0_f64, 1.5875, 2.0, 3.175, 6.0, 6.35, 12.0, 12.7] {
        let mut vals: Vec<(f64, &str)> = rows
            .iter()
            .map(|o| {
                let rd = o.diameter_mm.unwrap_or(1.0);
                let raw = (q / rd).clamp(0.1, 10.0);
                (
                    apply_chipload_law(raw, PROPOSED_DIAMETER_EXPONENT)
                        / apply_chipload_law(raw, CHIPLOAD_DIAMETER_EXPONENT),
                    o.observation_id.as_str(),
                )
            })
            .collect();
        vals.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let n = vals.len();
        let pc = |k: f64| vals[(k * n as f64) as usize % n].0;
        println!(
            "| {q:.4} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | `{}` ×{:.3} | `{}` ×{:.3} |",
            vals[0].0,
            pc(0.25),
            pc(0.5),
            pc(0.75),
            vals[n - 1].0,
            vals[0].1,
            vals[0].0,
            vals[n - 1].1,
            vals[n - 1].0,
        );
    }
    println!();
}
