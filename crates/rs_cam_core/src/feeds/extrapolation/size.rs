//! G1: the size claim. A row exists for the family, but not at this
//! diameter (`planning/extrapolation_2026-09-24/EXTRAPOLATION_G1.md`).
//!
//! [`SizeLaw::basis`] applies the rule of P1_PLAN §2.3 in this order. `d`
//! is the query's lookup key and `d_row` is the anchor's diameter.
//!
//! 0. A V-bit query: `VBitExempt`, no claim and no window (P1_PLAN §4
//!    risk 5, until ruling B4).
//! 1. No anchor diameter: `NoDiameterAnchor`. No anchor chipload (an
//!    RPM-only row): `NoChipload`, except that a tool under 1.5 mm keeps the
//!    size window of ruling R1 (`micro_extrapolation_refusal`).
//! 2. `|d / d_row - 1| <= 1e-3`: `Exact`.
//! 3. A tapered ball with `d < 0.5` mm: `Refused` (ruling B1). The code runs
//!    this step before step 2, so that a tip just under 0.5 mm on the 0.5 mm
//!    row refuses on every consumer, as the Suggest support arm does.
//! 4. The series: the rows that match the anchor on source, tool family,
//!    subfamily, material family, flutes, operation family and pass role,
//!    with a diameter, a chipload, grade a or b and a row kind other than
//!    fallback, one row per diameter. A grade-c or fallback anchor has no
//!    series.
//! 5. Form A: two adjacent series sizes bracket `d`. Interpolate the band
//!    mids log-log.
//! 6. A tapered ball under 1.5 mm: form A on any chart's tapered series
//!    with the query's material, flutes, operation family and role, or
//!    `Refused`.
//! 7. Form B (not a tapered ball, a series of 3 or more sizes): `d` lies
//!    at most one printed step past the span. The anchor series' own OLS
//!    slope scales the band.
//! 8. Form C: `d / d_row` in [0.5, 2.0] (inclusive). `(d / d_row)^0.61`,
//!    with the family's vendor spread.
//! 9. Otherwise `Refused`.

use std::collections::BTreeSet;

use super::{
    Claim, ClaimConfidence, ClaimResidual, Extrapolation, Gap, SizeBasis, SizeForm, SpreadFamily,
};
use crate::feeds::support::{
    MICRO_TOOL_DIAMETER_MM, micro_extrapolation_refusal, tapered_tip_floor_refusal,
};
use crate::feeds::vendor_lookup::{CHIPLOAD_DIAMETER_EXPONENT, LookupQuery, chipload_midpoint};
use crate::feeds::vendor_lut::{
    EvidenceGrade, ObservationKind, ToolFamily, VendorLut, VendorObservation,
};

/// Step 2: a query key within this relative distance of the anchor's
/// diameter is the printed size (`Exact`).
pub const EXACT_DIAMETER_TOLERANCE: f64 = 1e-3;

/// The lower edge of the form C window, as the ratio `d / d_row`
/// (inclusive). CHIPLOAD_LITERATURE_VERDICT §4.1 fitted the generic law on
/// this regime; EXTRAPOLATION_G1 §3.1 form C keeps it.
pub const SIZE_WINDOW_MIN: f64 = 0.5;

/// The upper edge of the form C window, as the ratio `d / d_row`
/// (inclusive). See [`SIZE_WINDOW_MIN`].
pub const SIZE_WINDOW_MAX: f64 = 2.0;

/// The tolerance on the window edges, so that a ratio of exactly 0.5 or
/// 2.0 is inside after the floating-point division.
pub const SIZE_WINDOW_EDGE_TOLERANCE: f64 = 1e-9;

/// The range of the printed flat-end size slopes (series with 3 or more
/// sizes): +0.29 to +1.25. EXTRAPOLATION_G1 §1.2 (34 distinct series) and
/// §3.1 form C. Form C states `r^(0.29 - 0.61)` to `r^(1.25 - 0.61)`,
/// which is x0.80 to x1.56 at `r = 2`.
///
/// Ball, bull and V-bit have no printed series, so their form C borrows
/// this spread and the card says so (orchestrator decision 2,
/// 2026-09-24).
pub const FLAT_END_SLOPE_SPREAD: (f64, f64) = (0.29, 1.25);

/// The range of the printed tapered-ball size slopes (tip frame): +0.00
/// (Amana 4F) to +1.06 (SpeTool). EXTRAPOLATION_G1 §1.2 and §1.3 (four
/// series), §3.1. Form C on a tapered ball at 1.5 mm or more states this
/// spread (orchestrator decision 2, 2026-09-24).
pub const TAPERED_SLOPE_SPREAD: (f64, f64) = (0.00, 1.06);

const RULE_A: &str = "G1 form A: log-log interpolation between two adjacent printed sizes of \
     one chart series";
const RULE_B: &str = "G1 form B: the chart series' own size slope (OLS on log band mid against \
     log diameter), one printed step past the span";
const RULE_C: &str = "G1 form C: the generic size law (d / d_row)^0.61 inside 0.5x-2x of the row";

/// The G1 size claim ([`Extrapolation`] for [`Gap::Size`]).
#[derive(Debug, Clone, Copy, Default)]
pub struct SizeLaw;

/// One printed size of a series: the diameter, the band mid, the row.
struct Point<'a> {
    d: f64,
    mid: f64,
    row: &'a VendorObservation,
}

/// The row may stand in a series: a diameter, a chipload, grade a or b,
/// and a row kind other than fallback.
fn is_series_row(obs: &VendorObservation) -> bool {
    obs.diameter_mm.is_some_and(|d| d.is_finite() && d > 0.0)
        && (obs.chipload_min_mm_tooth.is_some() || obs.chipload_max_mm_tooth.is_some())
        && chipload_midpoint(obs) > 0.0
        && matches!(obs.evidence_grade, EvidenceGrade::A | EvidenceGrade::B)
        && obs.row_kind != ObservationKind::Fallback
}

/// The series of `lut` rows that `same` accepts, sorted by diameter (then
/// by id), one row per diameter. At a repeated diameter the first row by
/// id stays, unless the repeat is the anchor.
fn series_where<'a>(
    lut: &'a VendorLut,
    anchor_id: &str,
    same: impl Fn(&VendorObservation) -> bool,
) -> Vec<Point<'a>> {
    let mut rows: Vec<Point<'a>> = lut
        .observations
        .iter()
        .filter(|obs| same(obs) && is_series_row(obs))
        .filter_map(|obs| {
            obs.diameter_mm.map(|d| Point {
                d,
                mid: chipload_midpoint(obs),
                row: obs,
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        a.d.total_cmp(&b.d)
            .then_with(|| a.row.observation_id.cmp(&b.row.observation_id))
    });
    let mut out: Vec<Point<'a>> = Vec::with_capacity(rows.len());
    for p in rows {
        match out.last_mut() {
            Some(last) if (p.d / last.d - 1.0).abs() <= EXACT_DIAMETER_TOLERANCE => {
                if p.row.observation_id == anchor_id {
                    *last = p;
                }
            }
            _ => out.push(p),
        }
    }
    out
}

/// The anchor's own series (step 4). Empty for a grade-c or fallback
/// anchor.
fn anchor_series<'a>(lut: &'a VendorLut, anchor: &VendorObservation) -> Vec<Point<'a>> {
    if !is_series_row(anchor) {
        return Vec::new();
    }
    series_where(lut, &anchor.observation_id, |obs| {
        obs.source_id == anchor.source_id
            && obs.tool_family == anchor.tool_family
            && obs.tool_subfamily == anchor.tool_subfamily
            && obs.material_family == anchor.material_family
            && obs.flute_count == anchor.flute_count
            && obs.operation_family == anchor.operation_family
            && obs.pass_role == anchor.pass_role
    })
}

/// Form A: the two adjacent points that bracket `d`, and the log-log
/// interpolated mid at `d`.
fn bracket<'s, 'a>(series: &'s [Point<'a>], d: f64) -> Option<(&'s Point<'a>, &'s Point<'a>, f64)> {
    series
        .iter()
        .zip(series.iter().skip(1))
        .find(|(lo, hi)| lo.d <= d && d <= hi.d)
        .map(|(lo, hi)| {
            let t = (d / lo.d).ln() / (hi.d / lo.d).ln();
            let mid = (lo.mid.ln() + t * (hi.mid / lo.mid).ln()).exp();
            (lo, hi, mid)
        })
}

/// The form A claim from a bracket.
fn interpolated_claim(
    query: &LookupQuery,
    anchor: &VendorObservation,
    lo: &Point<'_>,
    hi: &Point<'_>,
    mid_at_d: f64,
) -> Claim {
    Claim {
        gap: Gap::Size,
        form: SizeForm::Interpolated {
            lo_mm: lo.d,
            hi_mm: hi.d,
        },
        rule: RULE_A,
        scale: mid_at_d / chipload_midpoint(anchor),
        anchor_diameter_mm: anchor.diameter_mm.unwrap_or(0.0),
        query_diameter_mm: query.diameter_mm,
        source_rows: vec![lo.row.observation_id.clone(), hi.row.observation_id.clone()],
        series_source_id: lo.row.source_id.clone(),
        range_mm: lo.d..=hi.d,
        residual: ClaimResidual::Bracket {
            lo_value: lo.mid,
            hi_value: hi.mid,
        },
        confidence: ClaimConfidence::OneWitness,
    }
}

/// Step 6: form A on any tapered chart series with the query's material,
/// flutes, operation family and pass role (the ruling reads "inside *a*
/// chart's printed tips"). The keys are the query's, not the anchor's: the
/// anchor can win on a near flute count, and a tip on a flute count that no
/// chart prints stays refused (EXTRAPOLATION_G1 §3.4). The charts are tried
/// in (source_id, subfamily) order, and the first bracket wins. The anchor's
/// own chart is tried again with these keys; it gives no bracket where step
/// 5 gave none.
fn cross_chart_bracket(
    lut: &VendorLut,
    query: &LookupQuery,
    anchor: &VendorObservation,
) -> Option<Claim> {
    let same_cell = |obs: &VendorObservation| {
        obs.tool_family == ToolFamily::TaperedBallNose
            && obs.material_family == query.material_family
            && obs.flute_count == query.flute_count
            && obs.operation_family == query.operation_family
            && obs.pass_role == query.pass_role
    };
    let charts: BTreeSet<(&str, Option<&str>)> = lut
        .observations
        .iter()
        .filter(|obs| same_cell(obs) && is_series_row(obs))
        .map(|obs| (obs.source_id.as_str(), obs.tool_subfamily.as_deref()))
        .collect();
    charts.into_iter().find_map(|(source, subfamily)| {
        let series = series_where(lut, &anchor.observation_id, |obs| {
            same_cell(obs) && obs.source_id == source && obs.tool_subfamily.as_deref() == subfamily
        });
        bracket(&series, query.diameter_mm)
            .map(|(lo, hi, mid)| interpolated_claim(query, anchor, lo, hi, mid))
    })
}

/// Form B: the OLS fit of `ln(mid)` on `ln(d)`. Returns (slope, r², rms of
/// the log residuals). The caller passes 3 or more distinct sizes.
fn log_log_fit(series: &[Point<'_>]) -> (f64, f64, f64) {
    let n = series.len() as f64;
    let xs: Vec<f64> = series.iter().map(|p| p.d.ln()).collect();
    let ys: Vec<f64> = series.iter().map(|p| p.mid.ln()).collect();
    let mx = xs.iter().sum::<f64>() / n;
    let my = ys.iter().sum::<f64>() / n;
    let sxx: f64 = xs.iter().map(|x| (x - mx).powi(2)).sum();
    let sxy: f64 = xs.iter().zip(&ys).map(|(x, y)| (x - mx) * (y - my)).sum();
    let slope = if sxx > 0.0 { sxy / sxx } else { 0.0 };
    let intercept = my - slope * mx;
    let ssr: f64 = xs
        .iter()
        .zip(&ys)
        .map(|(x, y)| (y - (intercept + slope * x)).powi(2))
        .sum();
    let sst: f64 = ys.iter().map(|y| (y - my).powi(2)).sum();
    let r2 = if sst > 0.0 { 1.0 - ssr / sst } else { 1.0 };
    (slope, r2, (ssr / n).sqrt())
}

/// Step 7: the one-step range of a series of 3 or more sizes,
/// `d_min² / d_2 ..= d_max² / d_(n-1)`.
fn one_step_range(series: &[Point<'_>]) -> Option<(f64, f64)> {
    let first = series.first()?;
    let second = series.get(1)?;
    let last = series.last()?;
    let before_last = series.get(series.len().checked_sub(2)?)?;
    Some((
        first.d * first.d / second.d,
        last.d * last.d / before_last.d,
    ))
}

/// The form C spread for the query's tool family (decision 2).
fn spread_for(family: ToolFamily) -> (SpreadFamily, (f64, f64)) {
    match family {
        ToolFamily::FlatEnd => (SpreadFamily::FlatEnd, FLAT_END_SLOPE_SPREAD),
        ToolFamily::TaperedBallNose => (SpreadFamily::TaperedBall, TAPERED_SLOPE_SPREAD),
        ToolFamily::BallNose
        | ToolFamily::BullNose
        | ToolFamily::ChamferVbit
        | ToolFamily::FacingBit => (SpreadFamily::BorrowedFromFlatEnd, FLAT_END_SLOPE_SPREAD),
    }
}

/// The form C claim at ratio `r = d / d_row`, valid on `range_mm`.
fn generic_claim(
    query: &LookupQuery,
    anchor: &VendorObservation,
    d_row: f64,
    range_mm: std::ops::RangeInclusive<f64>,
    rule: &'static str,
) -> Claim {
    let p = CHIPLOAD_DIAMETER_EXPONENT;
    let r = query.diameter_mm / d_row;
    let (family, slopes) = spread_for(query.tool_family);
    let a = r.powf(slopes.0 - p);
    let b = r.powf(slopes.1 - p);
    Claim {
        gap: Gap::Size,
        form: SizeForm::GenericFallback { exponent: p },
        rule,
        scale: r.powf(p),
        anchor_diameter_mm: d_row,
        query_diameter_mm: query.diameter_mm,
        source_rows: vec![anchor.observation_id.clone()],
        series_source_id: anchor.source_id.clone(),
        range_mm,
        residual: ClaimResidual::VendorSpread {
            lo: a.min(b),
            hi: a.max(b),
            slopes,
            family,
        },
        confidence: ClaimConfidence::OneWitness,
    }
}

/// The ratio `r` lies in the form C window, edges included.
fn in_window(r: f64) -> bool {
    (SIZE_WINDOW_MIN - SIZE_WINDOW_EDGE_TOLERANCE..=SIZE_WINDOW_MAX + SIZE_WINDOW_EDGE_TOLERANCE)
        .contains(&r)
}

/// The refusal for a key at 1.5 mm or more. The text starts with
/// "no published figure for a ", as every size refusal does (FM0).
fn large_tool_refusal(family: ToolFamily, d: f64, d_row: f64) -> String {
    format!(
        "no published figure for a {d:.2} mm {}; the nearest chart row is {d_row} mm, {:.1}x the \
         tool, outside the {SIZE_WINDOW_MIN}x to {SIZE_WINDOW_MAX}x window of the generic size \
         law and past one printed step of the row's chart series (extrapolation G1)",
        family.label(),
        d_row / d
    )
}

/// The refusal for a tapered tip under 1.5 mm whose row is inside the
/// window, but no chart brackets the tip (step 6).
fn tapered_unprinted_refusal(d: f64) -> String {
    format!(
        "no published figure for a {d:.2} mm tapered ball nose: a tapered tip under \
         {MICRO_TOOL_DIAMETER_MM} mm ships only inside a chart's printed tips, and no chart \
         brackets this tip (ruling B1, extrapolation G1)"
    )
}

fn refused(reason: String) -> SizeBasis {
    SizeBasis::Refused {
        gap: Gap::Size,
        reason,
    }
}

impl Extrapolation for SizeLaw {
    fn gap(&self) -> Gap {
        Gap::Size
    }

    fn basis(&self, lut: &VendorLut, query: &LookupQuery, anchor: &VendorObservation) -> SizeBasis {
        let d = query.diameter_mm;
        let family = query.tool_family;
        // P1_PLAN §4 risk 5 (orchestrator, 2026-09-24): a V-bit takes no
        // size claim and no window until ruling B4. The gate keys it at
        // the engaged width at the sample depth; a window here refused
        // rows that the gate judged before P1.
        if family == ToolFamily::ChamferVbit {
            return SizeBasis::VBitExempt;
        }
        // Step 1.
        let Some(d_row) = anchor.diameter_mm.filter(|v| v.is_finite() && *v > 0.0) else {
            return SizeBasis::NoDiameterAnchor;
        };
        if !(d.is_finite() && d > 0.0) {
            return SizeBasis::NoDiameterAnchor;
        }
        if anchor.chipload_min_mm_tooth.is_none() && anchor.chipload_max_mm_tooth.is_none() {
            // An RPM-only anchor keeps the size window of ruling R1 for a
            // tool under 1.5 mm, as before the claim.
            return match micro_extrapolation_refusal(family, d, d, d_row) {
                Some(reason) => refused(reason),
                None => SizeBasis::NoChipload,
            };
        }
        // Step 3 runs before step 2, as the Suggest support arm runs the tip
        // floor first: a 0.4999 mm tip on the 0.5 mm row is within the exact
        // tolerance, and it must still refuse on every consumer.
        if let Some(reason) = tapered_tip_floor_refusal(family, d) {
            return refused(reason);
        }
        // Step 2.
        if (d / d_row - 1.0).abs() <= EXACT_DIAMETER_TOLERANCE {
            return SizeBasis::Exact;
        }
        // Steps 4 and 5.
        let series = anchor_series(lut, anchor);
        if let Some((lo, hi, mid)) = bracket(&series, d) {
            return SizeBasis::Claim(Box::new(interpolated_claim(query, anchor, lo, hi, mid)));
        }
        let micro = d < MICRO_TOOL_DIAMETER_MM;
        // Step 6.
        if family == ToolFamily::TaperedBallNose && micro {
            if let Some(claim) = cross_chart_bracket(lut, query, anchor) {
                return SizeBasis::Claim(Box::new(claim));
            }
            return refused(
                micro_extrapolation_refusal(family, d, d, d_row)
                    .unwrap_or_else(|| tapered_unprinted_refusal(d)),
            );
        }
        // Step 7.
        if family != ToolFamily::TaperedBallNose
            && series.len() >= 3
            && let Some((lo, hi)) = one_step_range(&series)
            && (lo * (1.0 - SIZE_WINDOW_EDGE_TOLERANCE)..=hi * (1.0 + SIZE_WINDOW_EDGE_TOLERANCE))
                .contains(&d)
        {
            let (slope, r2, rms) = log_log_fit(&series);
            return SizeBasis::Claim(Box::new(Claim {
                gap: Gap::Size,
                form: SizeForm::SeriesSlope {
                    slope,
                    r2,
                    sizes: series.len(),
                },
                rule: RULE_B,
                scale: (d / d_row).powf(slope),
                anchor_diameter_mm: d_row,
                query_diameter_mm: d,
                source_rows: series
                    .iter()
                    .map(|p| p.row.observation_id.clone())
                    .collect(),
                series_source_id: anchor.source_id.clone(),
                range_mm: lo..=hi,
                residual: ClaimResidual::FitRms { fraction: rms },
                confidence: ClaimConfidence::OneWitness,
            }));
        }
        // Step 8.
        let r = d / d_row;
        if in_window(r) {
            return SizeBasis::Claim(Box::new(generic_claim(
                query,
                anchor,
                d_row,
                SIZE_WINDOW_MIN * d_row..=SIZE_WINDOW_MAX * d_row,
                RULE_C,
            )));
        }
        // Step 9.
        refused(
            micro_extrapolation_refusal(family, d, d, d_row)
                .unwrap_or_else(|| large_tool_refusal(family, d, d_row)),
        )
    }
}
