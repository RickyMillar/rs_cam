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
//! **Red-first note.** Under the then-shipped `^1.0` laws the raw ratio and
//! the applied scale were numerically identical, so the defect could not be
//! observed on shipped values — it was a *latent* one that fires the moment
//! an exponent moves. [`the_pre_fix_rule_silently_un_flags_a_row_when_an_exponent_softens`]
//! therefore reproduces the pre-fix expression inline (the technique
//! `boundary_clip_escape_f1` uses) and shows it going `true → false` on
//! arithmetic alone, then shows the shipped rule holding. That test's first
//! two assertions ran against the `29303b5` parent unmodified; the third did
//! not compile there, because `is_extrapolated_for_ratios` did not exist.
//!
//! ## THE EXPONENTS MOVED — 2026-08-06, and the rider held
//!
//! `CHIPLOAD_DIAMETER_EXPONENT` is now **0.61** and
//! `CHIPLOAD_HARDNESS_EXPONENT` is now **0.5**. This is the exact event
//! the rider was written to survive, so it is worth stating plainly what
//! the file measures now:
//!
//! - [`a_live_lut_query_in_the_trap_window_carries_the_flag`] — **the
//!   rider itself. Unmodified and green.** Every shipped flag still equals
//!   the raw-ratio rule's answer, at Ø1 / 2 / 3.175 / 6 / 12, i.e. the
//!   **8.0 % of (query, row) pairs** measured in `LAW_MAGNITUDE_TABLES.md`
//!   §1 that would have silently lost their flag did not lose it.
//! - [`the_flag_is_exponent_invariant_and_the_applied_scale_is_not`] —
//!   **green, re-pinned**: its "does the applied scale move?" control now
//!   compares the live laws against the **retired** `^1.0` pair rather
//!   than against the proposed one, which is the same statement now that
//!   the proposal is what ships.
//! - [`the_pre_fix_rule_silently_un_flags_a_row_when_an_exponent_softens`]
//!   — **green, re-pinned**: it is a historical exhibit and must keep
//!   demonstrating `^1.0 → ^0.61`, so it reads a local
//!   [`RETIRED_LINEAR_EXPONENT`] instead of the live constant. Reading the
//!   live constant would have made it compare 0.61 against 0.61 and assert
//!   nothing.
//! - [`the_shipped_laws_are_identity_so_this_wave_moves_no_band`] — this
//!   one was **true of the previous wave only**, and is restated rather
//!   than deleted. See its body.

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

/// The exponents B-lit §4 recommended. **Adopted 2026-08-06** — they are
/// now `vendor_lookup::{CHIPLOAD_DIAMETER_EXPONENT,
/// CHIPLOAD_HARDNESS_EXPONENT}`. Kept as local literals so the exhibit
/// below keeps demonstrating the *transition*, which is what it is for;
/// if they ever disagree with the live constants that is a signal, and
/// [`the_shipped_laws_are_identity_so_this_wave_moves_no_band`] asserts
/// they do not.
const PROPOSED_DIAMETER_EXPONENT: f64 = 0.61;
const PROPOSED_HARDNESS_EXPONENT: f64 = 0.5;

/// The linear law that shipped until 2026-08-06. The exhibit needs a
/// *pair* of exponents to show the flag moving between them, so it reads
/// this rather than the live constant — which is now the softened side of
/// the same pair.
const RETIRED_LINEAR_EXPONENT: f64 = 1.0;

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
        // RE-PINNED 2026-08-06: was `CHIPLOAD_DIAMETER_EXPONENT`, which is
        // now 0.61 — the softened side. The exhibit is about the move
        // BETWEEN the two laws, so the retired one must be named
        // explicitly or the test compares 0.61 with 0.61.
        let applied_today = apply_chipload_law(raw_d, RETIRED_LINEAR_EXPONENT);
        let applied_softened = apply_chipload_law(raw_d, PROPOSED_DIAMETER_EXPONENT);

        // (a) The retired ^1.0 law flags it. This assertion holds on the
        //     `29303b5` parent against the then-live constant.
        assert!(
            pre_fix_flag(applied_today),
            "raw diameter ratio {raw_d} must be flagged under the retired ^1.0 law \
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
        RETIRED_LINEAR_EXPONENT
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
        // And the quantity the pre-fix rule read DOES move. RE-PINNED
        // 2026-08-06: the "proposed" side is now what ships, so the
        // control compares the LIVE laws against the RETIRED linear pair.
        // Same statement, opposite end held fixed.
        let applied_retired = apply_chipload_law(raw_d, RETIRED_LINEAR_EXPONENT)
            * apply_chipload_law(raw_h, RETIRED_LINEAR_EXPONENT);
        let applied_live = apply_chipload_law(raw_d, CHIPLOAD_DIAMETER_EXPONENT)
            * apply_chipload_law(raw_h, CHIPLOAD_HARDNESS_EXPONENT);
        if (raw_d - 1.0).abs() > 1e-9 || (raw_h - 1.0).abs() > 1e-9 {
            assert!(
                (applied_retired - applied_live).abs() > 1e-9,
                "({raw_d}, {raw_h}) must move between the retired and live laws \
                 or it proves nothing"
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
    // RESTATED 2026-08-06, name kept so the change is visible in the diff.
    //
    // This read:
    //     assert!((CHIPLOAD_DIAMETER_EXPONENT - 1.0).abs() < f64::EPSILON);
    //     assert!((CHIPLOAD_HARDNESS_EXPONENT - 1.0).abs() < f64::EPSILON);
    // and it was the rider wave's own proof that introducing the
    // `apply_chipload_law` seam smuggled no numeric change in behind a
    // refactor. The exponents have since been adopted, so the claim is
    // now false BY DESIGN and pinning it would freeze the decision the
    // operator made.
    //
    // Two things survive it, and both are load-bearing:
    assert!(
        (CHIPLOAD_DIAMETER_EXPONENT - PROPOSED_DIAMETER_EXPONENT).abs() < f64::EPSILON,
        "the live diameter law must be the one LAW_MAGNITUDE_TABLES.md measured \
         ({PROPOSED_DIAMETER_EXPONENT}); got {CHIPLOAD_DIAMETER_EXPONENT}. Every \
         magnitude in that document, and every re-pin made against it, is keyed to \
         this value."
    );
    assert!(
        (CHIPLOAD_HARDNESS_EXPONENT - PROPOSED_HARDNESS_EXPONENT).abs() < f64::EPSILON,
        "the live hardness law must be the one LAW_MAGNITUDE_TABLES.md measured \
         ({PROPOSED_HARDNESS_EXPONENT}); got {CHIPLOAD_HARDNESS_EXPONENT}"
    );
    // And the identity property of the seam itself, unchanged — so a
    // rollback to `^1.0` is provably a bit-for-bit no-op rather than a
    // second numeric event.
    for &r in &[
        0.1_f64, 0.3005, 0.5, 0.6, 0.71, 0.9999, 1.0, 1.4, 2.0, 2.4167, 3.0, 10.0,
    ] {
        assert_eq!(
            apply_chipload_law(r, RETIRED_LINEAR_EXPONENT).to_bits(),
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
