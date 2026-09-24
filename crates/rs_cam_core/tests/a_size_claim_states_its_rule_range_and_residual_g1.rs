//! G1 — a size claim states its rule, its range and its residual
//! (extrapolation P1 step 3, `planning/extrapolation_2026-09-24/P1_PLAN.md`
//! §2.3, orchestrator decision 2).
//!
//! `feeds::extrapolation::SizeLaw` is the G1 rule. `vendor_lookup::build_result`
//! calls it on every matched row, so every consumer reads the claim. This
//! sentry calls the rule directly on an anchor row that it takes from the
//! embedded LUT by id, so the pinned numbers do not depend on the lookup's
//! score. Where a case needs a series that the LUT does not print (the
//! inclusive window edges, the per-family spread), it clones one LUT row into
//! a one-row LUT and changes only the field under test.
//!
//! How each number was derived: python3 over the JSON files in
//! `data/vendor_lut/observations/` (the band mid is `(min + max) / 2`, or the
//! one printed bound), with the formulas of `extrapolation/size.rs`. Each
//! test states its own arithmetic.
//!
//! The arms:
//!
//! - form A: a 1.0 mm flat end mill between the printed 0.79375 mm and 1.5 mm
//!   rows of the Spektra softwood pocket series;
//! - form A across charts: a 1.0 mm 3-flute tapered tip on the Onsrud 1/8 in
//!   row, served by the Amana v8 3-flute series;
//! - form B: a 2.5 mm and a 12.7 mm flat end mill on the Onsrud 60-100mw
//!   hardwood series (3.175 / 4.7625 / 6.35 / 9.525 mm), slope 0.40;
//! - form C: a 3.175 mm ball nose on the grade-c 6.0 mm Amana ball row
//!   (ratio 0.529, spread borrowed from flat end mills), a 6.0 mm tapered tip
//!   on the Onsrud 1/4 in row (the tapered spread), and a flat end mill at
//!   exactly 2x (the flat spread);
//! - the inclusive 0.5x and 2.0x edges on a flat end mill under 1.5 mm,
//!   and the refusal just past 2x;
//! - a V-bit takes no claim and no window in P1 (`VBitExempt`, P1_PLAN §4
//!   risk 5);
//! - the refusals: a 25.4 mm flat end mill 8x off its row, a 1.0 mm ball
//!   nose 6x off its row, a 0.3 mm tapered tip, a 1.0 mm tapered Scallop, and
//!   a 1.0 mm 4-flute tapered tip that no chart brackets;
//! - the constants: the exponent 0.61, the flat and tapered spreads, the
//!   0.5 mm tip floor and the [0.5, 2] window.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::feeds::embedded_vendor_lut;
use rs_cam_core::feeds::extrapolation::{
    Claim, ClaimConfidence, ClaimResidual, EXACT_DIAMETER_TOLERANCE, Extrapolation,
    FLAT_END_SLOPE_SPREAD, Gap, SIZE_WINDOW_MAX, SIZE_WINDOW_MIN, SizeBasis, SizeForm, SizeLaw,
    SpreadFamily, TAPERED_SLOPE_SPREAD,
};
use rs_cam_core::feeds::support::TAPERED_MIN_TIP_MM;
use rs_cam_core::feeds::vendor_lookup::{CHIPLOAD_DIAMETER_EXPONENT, LookupQuery};
use rs_cam_core::feeds::vendor_lut::{
    HardnessKind, LutOperationFamily, LutPassRole, MaterialFamily, ToolFamily, VendorLut,
    VendorObservation,
};

const TOL: f64 = 1e-9;

fn row(id: &str) -> VendorObservation {
    embedded_vendor_lut()
        .observations
        .iter()
        .find(|o| o.observation_id == id)
        .unwrap_or_else(|| panic!("{id} is not in the embedded LUT"))
        .clone()
}

fn query(
    tool_family: ToolFamily,
    diameter_mm: f64,
    flute_count: u32,
    material_family: MaterialFamily,
    janka: f64,
    operation_family: LutOperationFamily,
    pass_role: LutPassRole,
) -> LookupQuery {
    LookupQuery {
        tool_family,
        tool_subfamily: None,
        diameter_mm,
        flute_count,
        material_family,
        hardness_kind: Some(HardnessKind::Janka),
        hardness_value: Some(janka),
        operation_family,
        pass_role,
    }
}

fn claim_of(basis: &SizeBasis) -> &Claim {
    basis
        .claim()
        .unwrap_or_else(|| panic!("expected a claim, got {basis:?}"))
}

fn refusal_of(basis: &SizeBasis) -> &str {
    match basis {
        SizeBasis::Refused { gap, reason } => {
            assert_eq!(*gap, Gap::Size);
            assert!(
                reason.starts_with("no published figure for a "),
                "every size refusal starts with the FM0 prefix: {reason}"
            );
            reason
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// A one-row LUT: `anchor` alone, so its series has one size.
fn one_row_lut(anchor: &VendorObservation) -> VendorLut {
    VendorLut {
        observations: vec![anchor.clone()],
    }
}

/// The constants that the rule and its card cite.
#[test]
fn the_size_constants_are_pinned_g1() {
    assert_eq!(CHIPLOAD_DIAMETER_EXPONENT, 0.61);
    // EXTRAPOLATION_G1 §1.2: flat-end slopes +0.29 to +1.25 (34 series);
    // tapered-ball slopes +0.00 to +1.06 (4 series). Decision 2.
    assert_eq!(FLAT_END_SLOPE_SPREAD, (0.29, 1.25));
    assert_eq!(TAPERED_SLOPE_SPREAD, (0.00, 1.06));
    assert_eq!(TAPERED_MIN_TIP_MM, 0.5);
    assert_eq!((SIZE_WINDOW_MIN, SIZE_WINDOW_MAX), (0.5, 2.0));
    assert_eq!(EXACT_DIAMETER_TOLERANCE, 1e-3);
    assert_eq!(SizeLaw.gap(), Gap::Size);
    assert_eq!(Gap::Size.group(), "G1");
    // The flat spread at 2x: 2^(0.29 - 0.61) = 0.8010699, 2^(1.25 - 0.61) =
    // 1.5583, the "x0.80-1.56 at 2x" of the card.
    assert!((2.0_f64.powf(0.29 - 0.61) - 0.801_069_878).abs() < 1e-8);
    assert!((2.0_f64.powf(1.25 - 0.61) - 1.558_329_159).abs() < 1e-8);
}

/// An exact row is `Exact`; the claim does not run.
#[test]
fn a_printed_size_is_exact_g1() {
    let anchor = row("amana-flat-softwood-pocket-0794-2f-spektra");
    let q = query(
        ToolFamily::FlatEnd,
        0.794,
        2,
        MaterialFamily::Softwood,
        600.0,
        LutOperationFamily::Pocket,
        LutPassRole::Roughing,
    );
    // 0.794 / 0.79375 - 1 = 3.1e-4, inside the 1e-3 tolerance.
    assert_eq!(
        SizeLaw.basis(embedded_vendor_lut(), &q, &anchor),
        SizeBasis::Exact
    );
}

/// Form A. Series: `amana_spektra_spiral_plunge_v24`, flat end, subfamily
/// `spektra_spiral_plunge`, softwood, 2F, pocket, roughing. The 0.79375 mm
/// row prints 0.0254 (min = max), the 1.5 mm row 0.0508. The mids double,
/// so the log-log mid at 1.0 mm is `0.0254 * 2^t` with
/// `t = ln(1.0 / 0.79375) / ln(1.5 / 0.79375) = 0.362928842984`, and the
/// scale on the anchor is `2^t = 1.286034051728`.
#[test]
fn form_a_interpolates_between_two_printed_sizes_g1() {
    let anchor = row("amana-flat-softwood-pocket-0794-2f-spektra");
    let q = query(
        ToolFamily::FlatEnd,
        1.0,
        2,
        MaterialFamily::Softwood,
        600.0,
        LutOperationFamily::Pocket,
        LutPassRole::Roughing,
    );
    let basis = SizeLaw.basis(embedded_vendor_lut(), &q, &anchor);
    let c = claim_of(&basis);
    assert_eq!(c.gap, Gap::Size);
    assert_eq!(
        c.form,
        SizeForm::Interpolated {
            lo_mm: 0.79375,
            hi_mm: 1.5
        }
    );
    assert!((c.scale - 1.286_034_051_728).abs() < TOL, "{c:?}");
    assert_eq!(c.range_mm, 0.79375..=1.5);
    assert_eq!(
        c.residual,
        ClaimResidual::Bracket {
            lo_value: 0.0254,
            hi_value: 0.0508
        }
    );
    assert_eq!(
        c.source_rows,
        vec![
            "amana-flat-softwood-pocket-0794-2f-spektra".to_owned(),
            "amana-flat-softwood-pocket-1500-2f-spektra".to_owned(),
        ]
    );
    assert_eq!(c.series_source_id, "amana_spektra_spiral_plunge_v24");
    assert_eq!(c.anchor_diameter_mm, 0.79375);
    assert_eq!(c.query_diameter_mm, 1.0);
    assert_eq!(c.confidence, ClaimConfidence::OneWitness);
    let (headline, detail) = c.card_text();
    assert_eq!(
        headline,
        "extrapolated (G1 size): x1.29 from the 0.79375 mm row"
    );
    assert_eq!(
        detail,
        "interpolated x1.29 from the 0.79375 mm row between the printed 0.79375 mm and 1.5 mm \
         rows of amana_spektra_spiral_plunge_v24 (log-log); valid 0.79375-1.5 mm; the printed \
         values are 0.0254 and 0.0508 mm/tooth"
    );
}

/// Form A across charts (step 6). A 1.0 mm 3-flute tapered tip on the
/// Onsrud 77-100 1/8 in hardwood row (3.175 mm, band 0.0762-0.127, mid
/// 0.1016). The LUT files it under pocket/roughing, and the G3 family rule
/// serves it to the parallel query (A3). The Onsrud series has no bracket, so the rule tries the
/// other tapered charts with the same material, flutes, family and role.
/// The Amana v8 3-flute series (0.79375 mm: 0.01905-0.0508, mid 0.034925;
/// 3.175 mm: 0.0381-0.0635, mid 0.0508) brackets 1.0 mm:
/// `t = ln(1.0 / 0.79375) / ln(4) = 0.166621704058`, the mid at 1.0 mm is
/// `0.034925 * (0.0508 / 0.034925)^t = 0.037174943265`, and the scale on the
/// Onsrud anchor is `0.037174943265 / 0.1016 = 0.365895110877`.
#[test]
fn form_a_reads_another_chart_for_a_micro_tapered_tip_g1() {
    let anchor = row("onsrud-hardwood-77-100-1_8-pocket");
    let q = query(
        ToolFamily::TaperedBallNose,
        1.0,
        3,
        MaterialFamily::Hardwood,
        1450.0,
        LutOperationFamily::Parallel,
        LutPassRole::Finish,
    );
    let basis = SizeLaw.basis(embedded_vendor_lut(), &q, &anchor);
    let c = claim_of(&basis);
    assert_eq!(
        c.form,
        SizeForm::Interpolated {
            lo_mm: 0.79375,
            hi_mm: 3.175
        }
    );
    assert!((c.scale - 0.365_895_110_877).abs() < TOL, "{c:?}");
    assert_eq!(
        c.source_rows,
        vec![
            "amana-tapered-hardwood-parallel-0794-3f-zrn-v8".to_owned(),
            "amana-tapered-hardwood-parallel-3175-3f-zrn-v8".to_owned(),
        ]
    );
    assert_eq!(c.series_source_id, "amana_zrn_3d_profiling_v8");
    assert_eq!(c.anchor_diameter_mm, 3.175);
}

/// Form B. Series: `onsrud_hard_wood_cutting_data`, flat end,
/// `60_100mw_compression_spiral`, hardwood, 2F, contour, finish; mids
/// 0.2794 / 0.3302 / 0.381 / 0.4318 mm/tooth at 3.175 / 4.7625 / 6.35 /
/// 9.525 mm. The OLS fit of ln(mid) on ln(d) gives slope 0.402736549890,
/// r² 0.992078200407 and the log-residual rms 0.014449773744. One printed
/// step past the span: `3.175² / 4.7625 = 2.116666666667` to
/// `9.525² / 6.35 = 14.2875`. At 2.5 mm on the 3.175 mm row the scale is
/// `(2.5 / 3.175)^0.402736549890 = 0.908227081527`; at 12.7 mm on the
/// 9.525 mm row it is `(12.7 / 9.525)^0.402736549890 = 1.122838759522`.
#[test]
fn form_b_uses_the_series_own_slope_one_step_past_the_span_g1() {
    let q = |d: f64| {
        query(
            ToolFamily::FlatEnd,
            d,
            2,
            MaterialFamily::Hardwood,
            1450.0,
            LutOperationFamily::Contour,
            LutPassRole::Finish,
        )
    };
    let below = SizeLaw.basis(
        embedded_vendor_lut(),
        &q(2.5),
        &row("onsrud-hardwood-60-100mw-1_8-finish"),
    );
    let c = claim_of(&below);
    let SizeForm::SeriesSlope { slope, r2, sizes } = c.form else {
        panic!("expected form B, got {c:?}");
    };
    assert!((slope - 0.402_736_549_890).abs() < TOL, "{c:?}");
    assert!((r2 - 0.992_078_200_407).abs() < TOL, "{c:?}");
    assert_eq!(sizes, 4);
    let ClaimResidual::FitRms { fraction } = c.residual else {
        panic!("expected the fit rms, got {c:?}");
    };
    assert!((fraction - 0.014_449_773_744).abs() < TOL, "{c:?}");
    assert!((c.scale - 0.908_227_081_527).abs() < TOL, "{c:?}");
    assert!((c.range_mm.start() - 2.116_666_666_667).abs() < TOL);
    assert!((c.range_mm.end() - 14.2875).abs() < TOL);
    assert_eq!(c.source_rows.len(), 4);
    let (headline, detail) = c.card_text();
    assert_eq!(
        headline,
        "extrapolated (G1 size): x0.91 from the 3.175 mm row"
    );
    assert_eq!(
        detail,
        "scaled x0.91 from the 3.175 mm row by the chart's own size slope d^0.40 (r2 0.99, 4 \
         printed sizes of onsrud_hard_wood_cutting_data); valid 2.117-14.288 mm, one printed \
         step past the chart; fit rms 1.4 %"
    );

    let above = SizeLaw.basis(
        embedded_vendor_lut(),
        &q(12.7),
        &row("onsrud-hardwood-60-100mw-3_8-finish"),
    );
    assert!((claim_of(&above).scale - 1.122_838_759_522).abs() < TOL);

    // Past one step (2.1 mm < 2.1167 mm) the series does not answer, and the
    // generic law does: 2.1 / 3.175 = 0.661, inside the window.
    let past = SizeLaw.basis(
        embedded_vendor_lut(),
        &q(2.1),
        &row("onsrud-hardwood-60-100mw-1_8-finish"),
    );
    assert!(matches!(
        claim_of(&past).form,
        SizeForm::GenericFallback { .. }
    ));
}

/// Form C on a ball nose. The anchor `amana-ball-hardwood-scallop-6000-2f`
/// is grade c, so it has no series. `r = 3.175 / 6.0 = 0.529166666667`,
/// scale `r^0.61 = 0.678252514240`. The ball has no printed series, so the
/// spread is the flat one, borrowed: `r^(0.29 - 0.61) = 1.225886907070` and
/// `r^(1.25 - 0.61) = 0.665425112532`, sorted lo 0.6654 hi 1.2259.
#[test]
fn form_c_on_a_ball_borrows_the_flat_spread_g1() {
    let anchor = row("amana-ball-hardwood-scallop-6000-2f");
    let q = query(
        ToolFamily::BallNose,
        3.175,
        2,
        MaterialFamily::Hardwood,
        1450.0,
        LutOperationFamily::Scallop,
        LutPassRole::Finish,
    );
    let basis = SizeLaw.basis(embedded_vendor_lut(), &q, &anchor);
    let c = claim_of(&basis);
    assert_eq!(c.form, SizeForm::GenericFallback { exponent: 0.61 });
    assert!((c.scale - 0.678_252_514_240).abs() < TOL, "{c:?}");
    assert_eq!(c.range_mm, 3.0..=12.0);
    let ClaimResidual::VendorSpread {
        lo,
        hi,
        slopes,
        family,
    } = c.residual
    else {
        panic!("expected the vendor spread, got {c:?}");
    };
    assert!((lo - 0.665_425_112_532).abs() < TOL, "{c:?}");
    assert!((hi - 1.225_886_907_070).abs() < TOL, "{c:?}");
    assert_eq!(slopes, FLAT_END_SLOPE_SPREAD);
    assert_eq!(family, SpreadFamily::BorrowedFromFlatEnd);
    let (headline, detail) = c.card_text();
    assert_eq!(
        headline,
        "extrapolated (G1 size): x0.68 from the 6.0 mm row"
    );
    assert_eq!(
        detail,
        "scaled x0.68 from the 6.0 mm row by the generic size law d^0.61; valid 0.5-2x of the \
         row (3.000-12.000 mm); spread borrowed from flat end mills: flat-end vendors spread \
         x0.80-1.56 at 2x (x0.67-1.23 at this size)"
    );
}

/// Form C on a tapered tip at 1.5 mm or more (decision 2). The anchor
/// `onsrud-hardwood-77-100-1_4-pocket` (6.35 mm, served to the parallel
/// query by the G3 family rule) is the only 2-flute size of its series. `r = 6.0 / 6.35 = 0.944881889764`, scale
/// `r^0.61 = 0.966007037458`; the tapered spread gives
/// `r^(0.00 - 0.61) = 1.035189145859` and `r^(1.06 - 0.61) = 0.974809799301`.
/// At 2x the tapered spread is `2^-0.61 = 0.66` to `2^0.45 = 1.37`.
#[test]
fn form_c_on_a_tapered_tip_states_the_tapered_spread_g1() {
    let anchor = row("onsrud-hardwood-77-100-1_4-pocket");
    let q = query(
        ToolFamily::TaperedBallNose,
        6.0,
        2,
        MaterialFamily::Hardwood,
        1450.0,
        LutOperationFamily::Parallel,
        LutPassRole::Finish,
    );
    let basis = SizeLaw.basis(embedded_vendor_lut(), &q, &anchor);
    let c = claim_of(&basis);
    assert!((c.scale - 0.966_007_037_458).abs() < TOL, "{c:?}");
    let ClaimResidual::VendorSpread {
        lo,
        hi,
        slopes,
        family,
    } = c.residual
    else {
        panic!("expected the vendor spread, got {c:?}");
    };
    assert!((lo - 0.974_809_799_301).abs() < TOL, "{c:?}");
    assert!((hi - 1.035_189_145_859).abs() < TOL, "{c:?}");
    assert_eq!(slopes, TAPERED_SLOPE_SPREAD);
    assert_eq!(family, SpreadFamily::TaperedBall);
    let (_, detail) = c.card_text();
    assert!(
        detail
            .ends_with("; tapered-ball vendors spread x0.66-1.37 at 2x (x0.97-1.04 at this size)"),
        "{detail}"
    );
}

/// Form C on a flat end mill at exactly 2x states the flat spread, not
/// borrowed. A one-row LUT of the 0.79375 mm Spektra row; the query is
/// 1.5875 mm, and `1.5875 / 0.79375 = 2.0` exactly (both are the nearest
/// doubles to 127/80 and 127/160). Scale `2^0.61 = 1.526259...`, spread
/// `2^-0.32 = 0.801070` to `2^0.64 = 1.558329` (python3: 0.8010698775896221,
/// 1.5583291593209998).
#[test]
fn form_c_on_a_flat_end_mill_states_the_flat_spread_g1() {
    let anchor = row("amana-flat-softwood-pocket-0794-2f-spektra");
    let lut = one_row_lut(&anchor);
    let q = query(
        ToolFamily::FlatEnd,
        1.5875,
        2,
        MaterialFamily::Softwood,
        600.0,
        LutOperationFamily::Pocket,
        LutPassRole::Roughing,
    );
    let basis = SizeLaw.basis(&lut, &q, &anchor);
    let c = claim_of(&basis);
    assert!((c.scale - 2.0_f64.powf(0.61)).abs() < TOL, "{c:?}");
    let ClaimResidual::VendorSpread { lo, hi, family, .. } = c.residual else {
        panic!("expected the vendor spread, got {c:?}");
    };
    assert!((lo - 0.801_069_878).abs() < 1e-8);
    assert!((hi - 1.558_329_159).abs() < 1e-8);
    assert_eq!(family, SpreadFamily::FlatEnd);
    let (_, detail) = c.card_text();
    assert!(
        detail.contains("; flat-end vendors spread x0.80-1.56 at 2x"),
        "{detail}"
    );
}

/// The window is inclusive at 0.5x and 2.0x. A flat end mill under 1.5 mm
/// on a one-row LUT (a clone of the 0.79375 mm Spektra row, alone, with the
/// diameter under test): at 1.0 / 2.0 = 0.5 and 1.0 / 0.5 = 2.0 the claim is
/// form C; at 1.0 / 2.01 = 0.4975 it refuses with the micro rule's text.
#[test]
fn the_window_edges_are_inclusive_g1() {
    let anchor = row("amana-flat-softwood-pocket-0794-2f-spektra");
    let q = |d: f64| {
        query(
            ToolFamily::FlatEnd,
            d,
            2,
            MaterialFamily::Softwood,
            600.0,
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
        )
    };
    let at = |row_d: f64, key: f64| {
        let mut a = anchor.clone();
        a.diameter_mm = Some(row_d);
        SizeLaw.basis(&one_row_lut(&a), &q(key), &a)
    };
    // 1.0 / 2.0 = 0.5 exactly: inside. Scale 0.5^0.61 = 0.655196701929.
    let low = at(2.0, 1.0);
    let c = claim_of(&low);
    assert!((c.scale - 0.655_196_701_929).abs() < TOL);
    assert_eq!(c.range_mm, 1.0..=4.0);
    let ClaimResidual::VendorSpread { family, .. } = c.residual else {
        panic!("expected the vendor spread, got {c:?}");
    };
    assert_eq!(family, SpreadFamily::FlatEnd);
    // 1.0 / 0.5 = 2.0 exactly: inside.
    let high = at(0.5, 1.0);
    assert_eq!(claim_of(&high).range_mm, 0.25..=1.0);
    // 1.0 / 2.01 = 0.4975: outside, and the micro rule's text names the row.
    let past = at(2.01, 1.0);
    assert_eq!(
        refusal_of(&past),
        "no published figure for a 1.00 mm flat end mill; the nearest chart row is 2.01 mm, \
         2.0x the tool, outside the 0.5x to 2x window a tool under 1.5 mm needs (ruling R1 \
         applied to size)"
    );
}

/// A V-bit takes no claim and no window in P1 (P1_PLAN §4 risk 5, until
/// ruling B4): the basis is `VBitExempt` at any ratio, and the lookup keeps
/// the generic `raw^0.61` scale. The anchor is a clone of a flat row with
/// the family set to V-bit, alone in its LUT.
#[test]
fn a_v_bit_takes_no_size_claim_in_p1_g1() {
    let mut anchor = row("amana-flat-softwood-pocket-0794-2f-spektra");
    anchor.tool_family = ToolFamily::ChamferVbit;
    for (row_d, key) in [(2.0, 1.0), (2.01, 1.0), (1.0, 6.0), (1.0, 1.0)] {
        let mut a = anchor.clone();
        a.diameter_mm = Some(row_d);
        let q = query(
            ToolFamily::ChamferVbit,
            key,
            2,
            MaterialFamily::Softwood,
            600.0,
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
        );
        assert_eq!(
            SizeLaw.basis(&one_row_lut(&a), &q, &a),
            SizeBasis::VBitExempt,
            "row {row_d} mm, key {key} mm"
        );
    }
}

/// The refusals. Each text starts with "no published figure for a ".
#[test]
fn a_size_outside_every_form_refuses_g1() {
    let lut = embedded_vendor_lut();

    // A 25.4 mm flat end mill on `amana-zrn-flat-softwood-pocket-3175-2f`.
    // Its series (3.175 / 6.0 / 6.35 mm) reaches one step to
    // 6.35² / 6.0 = 6.72 mm, and 25.4 / 3.175 = 8x is past the window.
    let big = SizeLaw.basis(
        lut,
        &query(
            ToolFamily::FlatEnd,
            25.4,
            2,
            MaterialFamily::Softwood,
            600.0,
            LutOperationFamily::Pocket,
            LutPassRole::Roughing,
        ),
        &row("amana-zrn-flat-softwood-pocket-3175-2f"),
    );
    let reason = refusal_of(&big);
    assert!(
        reason.starts_with(
            "no published figure for a 25.40 mm flat end mill; the nearest chart row is 3.175 mm"
        ) && reason.ends_with("(extrapolation G1)"),
        "{reason}"
    );

    // A 1.0 mm ball nose on the 6.0 mm grade-c ball Scallop row: 6x.
    let ball = SizeLaw.basis(
        lut,
        &query(
            ToolFamily::BallNose,
            1.0,
            2,
            MaterialFamily::Hardwood,
            1450.0,
            LutOperationFamily::Scallop,
            LutPassRole::Finish,
        ),
        &row("amana-ball-hardwood-scallop-6000-2f"),
    );
    assert_eq!(
        refusal_of(&ball),
        "no published figure for a 1.00 mm ball nose; the nearest chart row is 6 mm, 6.0x the \
         tool, outside the 0.5x to 2x window a tool under 1.5 mm needs (ruling R1 applied to \
         size)"
    );

    // A 0.3 mm tapered tip on the printed 0.5 mm SpeTool row (ratio 0.6,
    // inside the window): the tip floor refuses (ruling B1).
    let tip = SizeLaw.basis(
        lut,
        &query(
            ToolFamily::TaperedBallNose,
            0.3,
            2,
            MaterialFamily::Hardwood,
            1450.0,
            LutOperationFamily::Parallel,
            LutPassRole::Finish,
        ),
        &row("spetool-tapered-hardwood-parallel-0500-2f"),
    );
    assert_eq!(
        refusal_of(&tip),
        "no published figure for a 0.30 mm tapered ball nose: no wood chart prints a tip under \
         0.5 mm (ruling B1)"
    );

    // A 1.0 mm tapered Scallop on the Onsrud 1/4 in row (6.35 mm, 6.35x),
    // which the G3 family rule serves to the Scallop query. No tapered Scallop chart of grade a or b brackets 1.0 mm.
    let scallop = SizeLaw.basis(
        lut,
        &query(
            ToolFamily::TaperedBallNose,
            1.0,
            2,
            MaterialFamily::Hardwood,
            1450.0,
            LutOperationFamily::Scallop,
            LutPassRole::Finish,
        ),
        &row("onsrud-hardwood-77-100-1_4-pocket"),
    );
    assert_eq!(
        refusal_of(&scallop),
        "no published figure for a 1.00 mm tapered ball nose; the nearest chart row is 6.35 mm, \
         6.3x the tool, outside the 0.5x to 2x window a tool under 1.5 mm needs (ruling R1 \
         applied to size)"
    );

    // A 1.0 mm 4-flute tapered tip on the Amana v8 1.5 mm 4-flute row (1.5x,
    // inside the window). The 4-flute series starts at 1.5 mm and no other
    // chart prints a 4-flute tapered tip, so no chart brackets 1.0 mm.
    let four = SizeLaw.basis(
        lut,
        &query(
            ToolFamily::TaperedBallNose,
            1.0,
            4,
            MaterialFamily::Hardwood,
            1450.0,
            LutOperationFamily::Parallel,
            LutPassRole::Finish,
        ),
        &row("amana-tapered-hardwood-parallel-1500-4f-zrn-v8"),
    );
    assert_eq!(
        refusal_of(&four),
        "no published figure for a 1.00 mm tapered ball nose: a tapered tip under 1.5 mm ships \
         only inside a chart's printed tips, and no chart brackets this tip (ruling B1, \
         extrapolation G1)"
    );
}
