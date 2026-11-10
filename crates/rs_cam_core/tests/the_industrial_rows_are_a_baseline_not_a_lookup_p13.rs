//! Feeds contract P13 item 5 (operator ruling D13 item 5, lead rulings
//! 2026-11-05): the `industrial_only/` rows are evidence for the model, not
//! a lookup to ship from.
//!
//! - The four Freud 1/2 in rows (`freud-solid-carbide-half-*`) are not in
//!   the shipped lookup. Commit 51edd494d (2026-06-01) had copied them into
//!   `observations/freud_solid_carbide.json`; P13 removes the copies.
//! - The Freud chart prints one solid-carbide table "based on cut depth equal
//!   to bit diameter", with no roughing or finishing split. Every remaining
//!   Freud row is (Pocket, Roughing), so it can anchor law B, and its note
//!   says why.
//! - The baseline: the in-series size law, computed here from the shipped
//!   rows (no typed exponent), predicts each industrial row from Freud's own
//!   largest smaller row of the same material. The ratio printed / law must
//!   stay inside the band the step 0 study measured for this estimator:
//!   `research_p13_items45/results.json`, key `fits.within_group_slope`
//!   (k = 0.5224 over 33 groups; ratios 1.119, 1.361, 1.160, 1.810). The
//!   law is low on every row: Freud's own size slope (1.16) is about twice
//!   the in-series slope, so the series term also carries a size term.
//!
//! Red before P13 items 4-5: the four rows are in the shipped lookup, and
//! the Freud wood rows carry the role `finish`.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]
#![expect(
    clippy::print_stderr,
    reason = "the baseline sentry prints the law-vs-row ratios (lead ruling, P13 item 5)"
)]

use std::collections::BTreeMap;

use rs_cam_core::feeds::embedded_vendor_lut;
use rs_cam_core::feeds::series::{ToolSeries, row_series};
use rs_cam_core::feeds::vendor_lut::{
    EvidenceGrade, LutOperationFamily, LutPassRole, MaterialFamily, ObservationKind, ToolFamily,
    Vendor, VendorObservation,
};

const INDUSTRIAL_JSON: &str =
    include_str!("../data/vendor_lut/industrial_only/freud_solid_carbide_industrial.json");

#[derive(serde::Deserialize)]
struct ObservationFile {
    observations: Vec<VendorObservation>,
}

fn industrial_rows() -> Vec<VendorObservation> {
    serde_json::from_str::<ObservationFile>(INDUSTRIAL_JSON)
        .expect("the industrial_only file parses")
        .observations
}

/// The material class of the step 0 fit (`analysis.py` `CLASS`): the four
/// classes the wood charts print.
fn wood_class(m: MaterialFamily) -> Option<&'static str> {
    match m {
        MaterialFamily::Softwood => Some("softwood"),
        MaterialFamily::Hardwood => Some("hardwood"),
        MaterialFamily::Mdf | MaterialFamily::Hdf | MaterialFamily::Particleboard => Some("mdf"),
        MaterialFamily::PlywoodSoftwood | MaterialFamily::PlywoodHardwood => Some("plywood"),
        MaterialFamily::Acrylic
        | MaterialFamily::Hdpe
        | MaterialFamily::Polycarbonate
        | MaterialFamily::Delrin
        | MaterialFamily::Aluminum
        | MaterialFamily::Fiberglass => None,
    }
}

fn mid(o: &VendorObservation) -> f64 {
    match (o.chipload_min_mm_tooth, o.chipload_max_mm_tooth) {
        (Some(a), Some(b)) => (a + b) / 2.0,
        (Some(v), None) | (None, Some(v)) => v,
        (None, None) => 0.0,
    }
}

/// `feeds::extrapolation::size` `is_series_row`: a diameter, a chip, grade a
/// or b, not a fallback row.
fn is_series_row(o: &VendorObservation) -> bool {
    o.diameter_mm.is_some_and(|d| d > 0.0)
        && mid(o) > 0.0
        && matches!(o.evidence_grade, EvidenceGrade::A | EvidenceGrade::B)
        && o.row_kind != ObservationKind::Fallback
}

/// One wood flat-end point per (class, printed cell): a copy of one cell
/// under a second family, role or material of the same class counts once.
/// A series group: source, subfamily, material class, flute count.
type GroupKey = (String, String, &'static str, u32);

struct Point {
    group: GroupKey,
    vendor: Vendor,
    material: MaterialFamily,
    d: f64,
    chip: f64,
}

fn points(rows: &[VendorObservation]) -> Vec<Point> {
    let mut seen: BTreeMap<String, Point> = BTreeMap::new();
    for o in rows {
        if o.tool_family != ToolFamily::FlatEnd || !is_series_row(o) {
            continue;
        }
        // P24a (lead ruling 2026-11-10): the study is the spiral in-series
        // law. A row of the straight series is not a point of the estimator
        // and not an anchor.
        if row_series(o) == Some(ToolSeries::Straight) {
            continue;
        }
        let Some(class) = wood_class(o.material_family) else {
            continue;
        };
        let d = o.diameter_mm.unwrap_or_default();
        let sub = o.tool_subfamily.clone().unwrap_or_default();
        let key = format!(
            "{class}|{}|{sub}|{d:.4}|{}|{:?}|{:?}",
            o.source_id, o.flute_count, o.chipload_min_mm_tooth, o.chipload_max_mm_tooth
        );
        seen.entry(key).or_insert_with(|| Point {
            group: (o.source_id.clone(), sub, class, o.flute_count),
            vendor: o.source_vendor,
            material: o.material_family,
            d,
            chip: mid(o),
        });
    }
    seen.into_values().collect()
}

/// The pooled within-group slope of ln chip on ln D, groups = (source,
/// subfamily, class, flutes), each group of two or more points demeaned.
fn within_group_slope(pts: &[Point]) -> (f64, usize) {
    let mut groups: BTreeMap<&GroupKey, Vec<(f64, f64)>> = BTreeMap::new();
    for p in pts {
        groups
            .entry(&p.group)
            .or_default()
            .push((p.d.ln(), p.chip.ln()));
    }
    let (mut sxy, mut sxx, mut n) = (0.0, 0.0, 0);
    for v in groups.values().filter(|v| v.len() >= 2) {
        let len = v.len() as f64;
        let mx = v.iter().map(|p| p.0).sum::<f64>() / len;
        let my = v.iter().map(|p| p.1).sum::<f64>() / len;
        sxy += v.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum::<f64>();
        sxx += v.iter().map(|p| (p.0 - mx).powi(2)).sum::<f64>();
        n += 1;
    }
    (sxy / sxx, n)
}

#[test]
fn the_industrial_rows_are_not_in_the_shipped_lookup_p13() {
    let industrial = industrial_rows();
    assert_eq!(
        industrial.len(),
        4,
        "the industrial_only file holds four rows"
    );
    let lut = embedded_vendor_lut();
    for row in &industrial {
        assert!(
            !lut.observations
                .iter()
                .any(|o| o.observation_id == row.observation_id),
            "{} is evidence, not a shipped row",
            row.observation_id
        );
    }
}

#[test]
fn every_freud_row_is_the_one_printed_table_p13() {
    let freud: Vec<&VendorObservation> = embedded_vendor_lut()
        .observations
        .iter()
        .filter(|o| o.source_vendor == Vendor::Freud)
        .collect();
    assert!(!freud.is_empty(), "non-vacuity: the Freud rows load");
    for o in freud {
        assert_eq!(
            (o.operation_family, o.pass_role),
            (LutOperationFamily::Pocket, LutPassRole::Roughing),
            "{}: the chart prints one table, with no roughing or finishing split",
            o.observation_id
        );
        assert!(
            o.notes
                .as_deref()
                .is_some_and(|n| n.contains("no roughing or finishing split")),
            "{}: the note states why the row is (Pocket, Roughing)",
            o.observation_id
        );
    }
}

#[test]
fn the_size_law_stays_inside_the_measured_band_on_the_industrial_rows_p13() {
    let shipped = points(&embedded_vendor_lut().observations);
    let (k, groups) = within_group_slope(&shipped);
    eprintln!("in-series size slope k = {k:.4} over {groups} groups");
    // results.json `fits.within_group_slope`: k 0.5224 over 33 groups at
    // step 0. Feeds contract P16 item 3 (lead ruling 2026-11-06) added 12
    // printed Spektra rows (hardwood, plywood softwood and plywood hardwood
    // at 0.794, 1.587, 2.381 and 4.763 mm, 2F). They join 3 existing series
    // groups, so the group count stays 33. The same estimator over the
    // shipped JSON gives k 0.5701 (0.5224 without the 12 rows).
    assert_eq!(groups, 33, "the group count of the step 0 estimator");
    assert!((k - 0.5701).abs() < 5e-4, "k is the P16 value: {k}");
    // The band of printed / law: the min and max of the 4 industrial
    // ratios at k 0.5701. Each step 0 ratio (results.json) times
    // (d / anchor d)^(0.5224 - 0.5701): half hardwood 1.1186 -> 1.1033,
    // half softwood 1.3608 -> 1.3165, half MDF 1.1603 -> 1.1226, half
    // plywood hardwood 1.8101 -> 1.7512.
    const BAND: (f64, f64) = (1.103, 1.752);
    for row in industrial_rows() {
        let d = row
            .diameter_mm
            .expect("an industrial row prints a diameter");
        let anchor = shipped
            .iter()
            .filter(|p| p.vendor == Vendor::Freud && p.material == row.material_family && p.d < d)
            .max_by(|a, b| a.d.total_cmp(&b.d))
            .expect("Freud prints a smaller size of the same material");
        let law = anchor.chip * (d / anchor.d).powf(k);
        let ratio = mid(&row) / law;
        eprintln!(
            "{}: printed {:.4}, law {law:.4} (anchor {:.3} mm), printed / law {ratio:.3}",
            row.observation_id,
            mid(&row),
            anchor.d
        );
        // The P13 claim: every industrial row prints above the size law.
        assert!(
            ratio > 1.0,
            "{}: printed / law {ratio:.3} is not above the law",
            row.observation_id
        );
        assert!(
            ratio >= BAND.0 && ratio <= BAND.1,
            "{}: printed / law {ratio:.3} is outside the measured band {BAND:?}",
            row.observation_id
        );
    }
}
