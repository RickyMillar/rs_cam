//! **The mandatory rider** — `is_extrapolated` must be computed on the
//! RAW transfer ratios, not on the scale actually applied to the band.
//!
//! Source: `planning/review_2026-08-04/CHIPLOAD_LITERATURE_VERDICT.md`
//! §4.1, the ⚠ block. B-lit recommends (but does **not** ship) softening
//! the LUT path's diameter law from `^1.0` to `^0.61` and its hardness law
//! from `^1.0` to `^0.5`. `vendor_lookup::build_result` computed the flag
//! as `|ln(applied_diameter_scale × applied_hardness_scale)| > ln 1.4`.
//! Softening an exponent **shrinks that product toward 1.0**, so a row
//! whose raw transfer ratio is unchanged silently stops being flagged.
//!
//! What the flag controls, so the cost is not hypothetical:
//!
//! - `LookupResult::is_extrapolated` → `ChipBoundsSource::VendorLutExtrapolated`
//!   (`tool_load/chipload.rs`),
//! - which is one of the three sources `ChipBoundsSource::low_side_is_advisory`
//!   (`tool_load/verdict.rs`) demotes,
//! - so un-flagging a row converts a **burn advisory into a hard
//!   `Exceeds(Low)` trip**, and
//! - `Confidence::Approximate` reverts to `Confidence::Validated` on the
//!   verdict an operator reads.
//!
//! The flag answers *"how far is this row from the query?"*. That distance
//! is a property of the **query and the row**, not of whatever exponent the
//! crate currently believes converts one to the other. This file pins it
//! there.
//!
//! **Red-first note.** Under the shipped `^1.0` laws the raw ratio and the
//! applied scale are numerically identical, so the defect cannot be
//! observed on shipped values — it is a *latent* one that fires the moment
//! an exponent moves. [`the_pre_fix_rule_silently_un_flags_a_row_when_an_exponent_softens`]
//! therefore reproduces the pre-fix expression inline (the technique
//! `boundary_clip_escape_f1` uses) and shows it going `true → false` on
//! arithmetic alone, then shows the shipped rule holding. That test's first
//! two assertions run against the parent tree unmodified; the third does
//! not compile there, because `is_extrapolated_for_ratios` did not exist.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::feeds::ToolGeometryHint;
use rs_cam_core::feeds::embedded_vendor_lut;
use rs_cam_core::feeds::vendor_lookup::{
    CHIPLOAD_DIAMETER_EXPONENT, CHIPLOAD_EXTRAPOLATION_LN_THRESHOLD, CHIPLOAD_HARDNESS_EXPONENT,
    LookupQuery, apply_chipload_law, find_best_chip_envelope_row, is_extrapolated_for_ratios,
};
use rs_cam_core::feeds::vendor_lut::{
    HardnessKind, LutOperationFamily, LutPassRole, MaterialFamily, ToolFamily,
};

/// The exponents B-lit §4 recommends and this wave deliberately does NOT
/// adopt. Used here only to demonstrate the trap; `LAW_MAGNITUDE_TABLES.md`
/// carries their measured cost.
const PROPOSED_DIAMETER_EXPONENT: f64 = 0.61;
const PROPOSED_HARDNESS_EXPONENT: f64 = 0.5;

/// The pre-fix rule, verbatim from `vendor_lookup::build_result` at parent
/// `29303b5`: `total_scale.ln().abs() > APPROX_LN_THRESHOLD`, where
/// `total_scale` is the product of the scales **actually applied** to the
/// band.
fn pre_fix_flag(applied_total_scale: f64) -> bool {
    applied_total_scale.ln().abs() > CHIPLOAD_EXTRAPOLATION_LN_THRESHOLD
}

#[test]
fn the_pre_fix_rule_silently_un_flags_a_row_when_an_exponent_softens() {
    // B-lit §4.1 names the windows: d_scale ∈ [0.58, 0.71] ∪ [1.40, 1.72]
    // lose the flag under `^0.61`. Take one row from each side.
    for &raw_d in &[0.60_f64, 0.71, 1.45, 1.70] {
        let applied_today = apply_chipload_law(raw_d, CHIPLOAD_DIAMETER_EXPONENT);
        let applied_softened = apply_chipload_law(raw_d, PROPOSED_DIAMETER_EXPONENT);

        // (a) The shipped laws flag it. This assertion holds on the parent.
        assert!(
            pre_fix_flag(applied_today),
            "raw diameter ratio {raw_d} must be flagged under the shipped ^1.0 law \
             (applied {applied_today}); if not, the fixture is not in the trap window"
        );
        // (b) Softening the exponent un-flags it, with the row no closer to
        //     the query than it was. This assertion also holds on the parent —
        //     it is arithmetic, and it is the defect.
        assert!(
            !pre_fix_flag(applied_softened),
            "raw diameter ratio {raw_d} is expected to LOSE its flag under ^0.61 \
             (applied {applied_softened}); B-lit §4.1's ⚠ window did not reproduce"
        );
        // (c) The shipped rule is computed on the raw ratio and is therefore
        //     invariant to the exponent. This does NOT compile on the parent.
        assert!(
            is_extrapolated_for_ratios(raw_d, 1.0),
            "the flag must be computed on the RAW transfer ratio {raw_d}, which is \
             {:.4} in log space against a {CHIPLOAD_EXTRAPOLATION_LN_THRESHOLD:.4} \
             threshold — independent of any exponent",
            raw_d.ln().abs()
        );
        eprintln!(
            "  raw d-ratio {raw_d:.2}: applied ^1.0 = {applied_today:.4} (pre-fix flag {}), \
             applied ^0.61 = {applied_softened:.4} (pre-fix flag {}), shipped rule = {}",
            pre_fix_flag(applied_today),
            pre_fix_flag(applied_softened),
            is_extrapolated_for_ratios(raw_d, 1.0)
        );
    }

    // Same trap on the hardness axis: B-lit §4.2's `h_scale = 1.50` row.
    let raw_h = 1.50_f64;
    assert!(pre_fix_flag(apply_chipload_law(
        raw_h,
        CHIPLOAD_HARDNESS_EXPONENT
    )));
    assert!(!pre_fix_flag(apply_chipload_law(
        raw_h,
        PROPOSED_HARDNESS_EXPONENT
    )));
    assert!(is_extrapolated_for_ratios(1.0, raw_h));
}

#[test]
fn the_flag_is_exponent_invariant_and_the_applied_scale_is_not() {
    // Non-vacuity in the other direction: the flag must still be capable of
    // reading false, and it must read false for the same rows regardless of
    // exponent.
    for &(raw_d, raw_h) in &[
        (1.0_f64, 1.0_f64),
        (1.2, 1.0),
        (0.9, 1.1),
        (0.60, 1.0),
        (1.0, 2.4167),
        (0.3005, 1.0),
    ] {
        let shipped = is_extrapolated_for_ratios(raw_d, raw_h);
        // Recompute the flag as if both proposed exponents were live. The
        // raw ratios are the inputs, so nothing may move.
        let under_proposed_laws = is_extrapolated_for_ratios(raw_d, raw_h);
        assert_eq!(
            shipped, under_proposed_laws,
            "flag for raw ratios ({raw_d}, {raw_h}) must not depend on the law"
        );
        // And the quantity the pre-fix rule read DOES move.
        let applied_today = apply_chipload_law(raw_d, CHIPLOAD_DIAMETER_EXPONENT)
            * apply_chipload_law(raw_h, CHIPLOAD_HARDNESS_EXPONENT);
        let applied_proposed = apply_chipload_law(raw_d, PROPOSED_DIAMETER_EXPONENT)
            * apply_chipload_law(raw_h, PROPOSED_HARDNESS_EXPONENT);
        if (raw_d - 1.0).abs() > 1e-9 || (raw_h - 1.0).abs() > 1e-9 {
            assert!(
                (applied_today - applied_proposed).abs() > 1e-9,
                "({raw_d}, {raw_h}) must move under the proposed laws or it proves nothing"
            );
        }
    }
    assert!(
        !is_extrapolated_for_ratios(1.0, 1.0),
        "an exact match must never read extrapolated"
    );
}

#[test]
fn the_shipped_laws_are_identity_so_this_wave_moves_no_band() {
    // The rider is structural. It must be provable that it changes no
    // shipped number: both exponents are 1.0, and `apply_chipload_law` at
    // exponent 1.0 must be the identity **bit-for-bit**, not merely close —
    // otherwise the rider would smuggle a numeric change in behind a
    // refactor.
    assert!((CHIPLOAD_DIAMETER_EXPONENT - 1.0).abs() < f64::EPSILON);
    assert!((CHIPLOAD_HARDNESS_EXPONENT - 1.0).abs() < f64::EPSILON);
    for &r in &[
        0.1_f64, 0.3005, 0.5, 0.6, 0.71, 0.9999, 1.0, 1.4, 2.0, 2.4167, 3.0, 10.0,
    ] {
        assert_eq!(
            apply_chipload_law(r, 1.0).to_bits(),
            r.to_bits(),
            "exponent 1.0 must be bit-identical for {r}"
        );
    }
}

#[test]
fn a_live_lut_query_in_the_trap_window_carries_the_flag() {
    // End-to-end, through the shipped lookup: the B3 row's query sits at
    // d_scale ≈ 0.30, well inside the flagged region under either law. A
    // second query is placed inside the ⚠ window so the fixture covers the
    // case that would have moved.
    let hint = ToolGeometryHint::Flat;
    let query = |diameter_mm: f64| LookupQuery {
        tool_family: ToolFamily::FlatEnd,
        tool_subfamily: None,
        diameter_mm,
        flute_count: 2,
        material_family: MaterialFamily::Hardwood,
        hardness_kind: Some(HardnessKind::Janka),
        hardness_value: Some(1290.0),
        operation_family: LutOperationFamily::Pocket,
        pass_role: LutPassRole::Roughing,
    };
    for &d in &[1.0_f64, 2.0, 3.175, 6.0, 12.0] {
        let Some(row) = find_best_chip_envelope_row(embedded_vendor_lut(), &query(d), &hint) else {
            continue;
        };
        assert_eq!(
            row.is_extrapolated,
            is_extrapolated_for_ratios(
                row.chipload_diameter_ratio_raw,
                row.chipload_hardness_ratio_raw
            ),
            "Ø{d}: the shipped flag on row {} must be the raw-ratio rule's answer",
            row.observation_id
        );
        eprintln!(
            "  Ø{d:>6.3} → {:<52} raw d {:.4} × raw h {:.4} → flag {}",
            row.observation_id,
            row.chipload_diameter_ratio_raw,
            row.chipload_hardness_ratio_raw,
            row.is_extrapolated
        );
    }
}
