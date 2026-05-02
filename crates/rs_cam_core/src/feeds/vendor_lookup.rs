//! Vendor LUT lookup with scoring algorithm.
//!
//! Filters observations by must-match criteria, scores remaining candidates,
//! and returns the best match with chipload midpoint.

use super::vendor_lut::{
    HardnessKind, LutOperationFamily, LutPassRole, MaterialFamily, ToolFamily, VendorLut,
    VendorObservation,
};

/// Query parameters for LUT lookup.
pub struct LookupQuery {
    pub tool_family: ToolFamily,
    pub tool_subfamily: Option<String>,
    pub diameter_mm: f64,
    pub flute_count: u32,
    pub material_family: MaterialFamily,
    pub hardness_kind: Option<HardnessKind>,
    pub hardness_value: Option<f64>,
    pub operation_family: LutOperationFamily,
    pub pass_role: LutPassRole,
}

pub type LookupCriteria = LookupQuery;
pub type MatchedRow = LookupResult;

/// Result of a successful LUT lookup.
#[derive(Debug, Clone)]
pub struct LookupResult {
    pub chip_load_mm: f64,
    /// Lower bound of the observation's chipload range, if reported.
    /// Below this is rubbing/burning territory.
    pub chip_load_min_mm: Option<f64>,
    /// Upper bound of the observation's chipload range, if reported.
    /// Above this is breakage territory.
    pub chip_load_max_mm: Option<f64>,
    pub rpm_nominal: Option<f64>,
    pub rpm_min: Option<f64>,
    pub rpm_max: Option<f64>,
    pub ap_min_mm: Option<f64>,
    pub ap_max_mm: Option<f64>,
    pub ae_min_mm: Option<f64>,
    pub ae_max_mm: Option<f64>,
    pub observation_id: String,
    pub source_vendor: String,
    pub score: i64,
    /// Diameter-proximity component of the composite score (0–200).
    /// 200 = exact diameter match; lower = more derated. Surfaced so
    /// downstream consumers (e.g. the F&S suggest module) can flag a
    /// match that won on tie-breakers despite a poor diameter fit.
    pub diameter_match_score: i64,
}

pub fn find_best_row(lut: &VendorLut, criteria: &LookupCriteria) -> Option<MatchedRow> {
    lookup_best(lut, criteria)
}

/// All compatible rows for the given criteria, sorted by composite score
/// descending. Used by the F&S suggest module to enumerate alternatives —
/// the gate only needs the best, the suggest module needs to consider
/// trade-offs across rows whose RPM falls inside the machine spindle.
pub fn enumerate_matching_rows(lut: &VendorLut, criteria: &LookupCriteria) -> Vec<MatchedRow> {
    let mut all: Vec<(i64, MatchedRow)> = Vec::new();
    for (i, obs) in lut.observations.iter().enumerate() {
        if !passes_must_match(criteria, obs) {
            continue;
        }
        let (score, diam_score) = score_observation(criteria, obs);
        all.push((score, build_result(obs, score, diam_score, i)));
    }
    all.sort_by_key(|(score, _)| -*score);
    all.into_iter().map(|(_, row)| row).collect()
}

fn build_result(
    obs: &VendorObservation,
    score: i64,
    diameter_match_score: i64,
    _idx: usize,
) -> LookupResult {
    LookupResult {
        chip_load_mm: chipload_midpoint(obs),
        chip_load_min_mm: obs.chipload_min_mm_tooth,
        chip_load_max_mm: obs.chipload_max_mm_tooth,
        rpm_nominal: obs.rpm_nominal,
        rpm_min: obs.rpm_min,
        rpm_max: obs.rpm_max,
        ap_min_mm: obs.ap_min_mm,
        ap_max_mm: obs.ap_max_mm,
        ae_min_mm: obs.ae_min_mm,
        ae_max_mm: obs.ae_max_mm,
        observation_id: obs.observation_id.clone(),
        source_vendor: format!("{:?}", obs.source_vendor),
        score,
        diameter_match_score,
    }
}

/// Find the best matching observation for a query.
pub fn lookup_best(lut: &VendorLut, query: &LookupQuery) -> Option<LookupResult> {
    let mut best: Option<(i64, i64, usize)> = None;

    for (i, obs) in lut.observations.iter().enumerate() {
        if !passes_must_match(query, obs) {
            continue;
        }
        let (score, diam_score) = score_observation(query, obs);
        if let Some((best_score, _, _)) = best {
            if score > best_score {
                best = Some((score, diam_score, i));
            }
        } else {
            best = Some((score, diam_score, i));
        }
    }

    best.map(|(score, diameter_match_score, i)| {
        // SAFETY: `i` was stored from a valid iteration over `lut.observations`
        #[allow(clippy::indexing_slicing)]
        let obs = &lut.observations[i];
        build_result(obs, score, diameter_match_score, i)
    })
}

fn passes_must_match(query: &LookupQuery, obs: &VendorObservation) -> bool {
    if obs.operation_family != query.operation_family {
        return false;
    }
    if obs.material_family != query.material_family {
        return false;
    }
    if !tool_family_compatible(query.tool_family, obs.tool_family) {
        return false;
    }
    let ratio = query.diameter_mm / obs.diameter_mm;
    if !(0.5..=2.0).contains(&ratio) {
        return false;
    }
    true
}

fn score_observation(query: &LookupQuery, obs: &VendorObservation) -> (i64, i64) {
    let mut score: i64 = 1000;
    score += tool_family_score(query.tool_family, obs.tool_family);
    score += obs.row_kind.score();
    score += obs.evidence_grade.score();

    let flute_diff = (query.flute_count as i64 - obs.flute_count as i64).unsigned_abs();
    score += match flute_diff {
        0 => 80,
        1 => 30,
        _ => -20,
    };

    let log_ratio = (query.diameter_mm / obs.diameter_mm).ln().abs();
    let diam_score = ((1.0 - log_ratio / 2.0_f64.ln()) * 200.0).clamp(0.0, 200.0) as i64;
    score += diam_score;

    if let (Some(qk), Some(qv), Some(ok), Some(ov)) = (
        query.hardness_kind,
        query.hardness_value,
        obs.hardness_kind,
        obs.hardness_value,
    ) && qk == ok
        && ov > 0.0
    {
        let rel_diff = ((qv - ov) / ov).abs();
        let h_score = ((1.0 - rel_diff) * 80.0).clamp(0.0, 80.0) as i64;
        score += h_score;
    }

    if let (Some(qs), Some(os)) = (&query.tool_subfamily, &obs.tool_subfamily)
        && qs == os
    {
        score += 50;
    }

    if query.pass_role == obs.pass_role {
        score += 45;
    } else {
        score -= 25;
    }

    (score, diam_score)
}

/// Check if a query tool family is compatible with an observation tool family.
fn tool_family_compatible(query: ToolFamily, obs: ToolFamily) -> bool {
    if query == obs {
        return true;
    }
    // Fallback compatibility
    matches!(
        (query, obs),
        (ToolFamily::BullNose, ToolFamily::FlatEnd)
            | (ToolFamily::TaperedBallNose, ToolFamily::BallNose)
            | (ToolFamily::FlatEnd, ToolFamily::BullNose)
    )
}

/// Score for tool family match quality.
fn tool_family_score(query: ToolFamily, obs: ToolFamily) -> i64 {
    if query == obs {
        return 220;
    }
    match (query, obs) {
        (ToolFamily::BullNose, ToolFamily::FlatEnd) => 120,
        (ToolFamily::TaperedBallNose, ToolFamily::BallNose) => 110,
        (ToolFamily::FlatEnd, ToolFamily::BullNose) => 100,
        _ => 0, // shouldn't reach here due to compatible filter
    }
}

/// Extract chipload as midpoint of min/max.
fn chipload_midpoint(obs: &VendorObservation) -> f64 {
    match (obs.chipload_min_mm_tooth, obs.chipload_max_mm_tooth) {
        (Some(min), Some(max)) => (min + max) / 2.0,
        (Some(v), None) | (None, Some(v)) => v,
        (None, None) => {
            // Infer from feed/rpm/flutes if available
            if let (Some(rpm), Some(_)) = (obs.rpm_nominal, Some(obs.flute_count)) {
                if rpm > 0.0 && obs.flute_count > 0 {
                    // No feed data in schema, return 0
                    0.0
                } else {
                    0.0
                }
            } else {
                0.0
            }
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::str_to_string
)]
mod tests {
    use super::*;

    fn embedded_lut() -> VendorLut {
        VendorLut::embedded()
    }

    #[test]
    fn find_best_row_matches_legacy_lookup_best() {
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::FlatEnd,
            tool_subfamily: Some("upcut".to_string()),
            diameter_mm: 6.0,
            flute_count: 2,
            material_family: MaterialFamily::Softwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(600.0),
            operation_family: LutOperationFamily::Adaptive,
            pass_role: LutPassRole::Roughing,
        };
        let via_canonical = find_best_row(&lut, &query).expect("canonical row");
        let via_legacy = lookup_best(&lut, &query).expect("legacy row");
        assert_eq!(via_canonical.observation_id, via_legacy.observation_id);
    }

    #[test]
    fn test_exact_match_6mm_2f_flat_softwood_adaptive() {
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::FlatEnd,
            tool_subfamily: Some("upcut".to_string()),
            diameter_mm: 6.0,
            flute_count: 2,
            material_family: MaterialFamily::Softwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(600.0),
            operation_family: LutOperationFamily::Adaptive,
            pass_role: LutPassRole::Roughing,
        };
        let result = lookup_best(&lut, &query).expect("should find match");
        assert!(
            result.score > 1400,
            "score {} should be > 1400",
            result.score
        );
        assert_eq!(
            result.observation_id,
            "amana-flat-softwood-adaptive-6000-2f"
        );
        // Midpoint of 0.065-0.11 = 0.0875
        assert!(
            (result.chip_load_mm - 0.0875).abs() < 0.001,
            "chipload {} should be ~0.0875",
            result.chip_load_mm
        );
    }

    #[test]
    fn test_near_match_5mm_still_finds_6mm() {
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::FlatEnd,
            tool_subfamily: None,
            diameter_mm: 5.0,
            flute_count: 2,
            material_family: MaterialFamily::Softwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(600.0),
            operation_family: LutOperationFamily::Adaptive,
            pass_role: LutPassRole::Roughing,
        };
        let result = lookup_best(&lut, &query).expect("should find match");
        // Should match 6mm row with diameter penalty
        assert!(result.chip_load_mm > 0.03, "chipload should be reasonable");
    }

    #[test]
    fn test_ball_nose_lookup() {
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::BallNose,
            tool_subfamily: None,
            diameter_mm: 6.0,
            flute_count: 2,
            material_family: MaterialFamily::Softwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(600.0),
            operation_family: LutOperationFamily::Parallel,
            pass_role: LutPassRole::Finish,
        };
        let result = lookup_best(&lut, &query).expect("should find ball nose match");
        assert!(result.chip_load_mm > 0.02 && result.chip_load_mm < 0.06);
        assert!(result.observation_id.contains("ball"));
    }

    #[test]
    fn test_no_match_returns_none() {
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::FlatEnd,
            tool_subfamily: None,
            diameter_mm: 50.0, // way too big, no 0.5-2x ratio match
            flute_count: 2,
            material_family: MaterialFamily::Softwood,
            hardness_kind: None,
            hardness_value: None,
            operation_family: LutOperationFamily::Adaptive,
            pass_role: LutPassRole::Roughing,
        };
        assert!(lookup_best(&lut, &query).is_none());
    }

    #[test]
    fn test_bull_nose_falls_back_to_flat_end() {
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::BullNose,
            tool_subfamily: None,
            diameter_mm: 6.0,
            flute_count: 2,
            material_family: MaterialFamily::Softwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(600.0),
            operation_family: LutOperationFamily::Adaptive,
            pass_role: LutPassRole::Roughing,
        };
        let result =
            lookup_best(&lut, &query).expect("bull nose should fallback to flat/bull rows");
        assert!(result.chip_load_mm > 0.03);
    }

    #[test]
    fn test_sub_1mm_tapered_ball_softwood_finish_matches() {
        // Item E coverage gain: a 1mm tapered ball + parallel + finish + softwood
        // must match at least one row after sub-1mm coverage was added (2026-05-02).
        // Source: Amana ZrN 3D Profiling chart sub-1mm rows.
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::TaperedBallNose,
            tool_subfamily: None,
            diameter_mm: 1.0,
            flute_count: 2,
            material_family: MaterialFamily::Softwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(600.0),
            operation_family: LutOperationFamily::Parallel,
            pass_role: LutPassRole::Finish,
        };
        let result = lookup_best(&lut, &query).expect(
            "1mm tapered ball + softwood + parallel/finish should match a sub-1mm ball-nose row",
        );
        // Should match one of the new ZrN sub-1mm rows.
        assert!(
            result.observation_id.contains("zrn"),
            "expected match against ZrN sub-1mm row, got {}",
            result.observation_id
        );
        // The 2-flute 1mm ball-nose row should win on the (TaperedBallNose -> BallNose)
        // family fallback at exact diameter and exact flute count.
        assert_eq!(
            result.observation_id, "amana-ball-softwood-parallel-1000-2f-zrn",
            "expected the 1mm 2-flute ZrN row to win for a 1mm 2-flute query"
        );
        // Chipload midpoint should match the published values: midpoint(0.01905, 0.0508).
        let expected_mid = (0.01905 + 0.0508) / 2.0;
        assert!(
            (result.chip_load_mm - expected_mid).abs() < 1e-4,
            "expected chipload midpoint ~{expected_mid}, got {}",
            result.chip_load_mm
        );
    }

    #[test]
    fn test_sub_1mm_tapered_ball_hardwood_finish_documented_gap() {
        // Item E refusal-first: the Amana ZrN 3D Profiling chart conflates
        // softwood/hardwood under one "Wood" row, and we deliberately did NOT
        // emit hardwood rows from that source for sub-1mm tools. This test
        // documents the gap: a 1mm tapered ball + hardwood + parallel/finish
        // should NOT match any sub-1mm row (it can only match larger 3.175mm+
        // rows that are out of the 0.5x-2x ratio for a 1mm query).
        // If this test starts failing because someone added a hardwood row,
        // verify the source PDF actually distinguishes hardwood from softwood.
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::TaperedBallNose,
            tool_subfamily: None,
            diameter_mm: 1.0,
            flute_count: 2,
            material_family: MaterialFamily::Hardwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(1450.0),
            operation_family: LutOperationFamily::Parallel,
            pass_role: LutPassRole::Finish,
        };
        // No match expected — coverage gap is real.
        assert!(
            lookup_best(&lut, &query).is_none(),
            "no hardwood-specific sub-1mm ball/tapered-ball rows exist; \
             if a match was found, verify the new row's source PDF distinguishes \
             hardwood from softwood, not just lumps them as 'wood'"
        );
    }

    #[test]
    fn test_rpm_nominal_returned() {
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::FlatEnd,
            tool_subfamily: None,
            diameter_mm: 6.0,
            flute_count: 2,
            material_family: MaterialFamily::Softwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(600.0),
            operation_family: LutOperationFamily::Adaptive,
            pass_role: LutPassRole::Roughing,
        };
        let result = lookup_best(&lut, &query).expect("should find match for 6mm flat softwood");
        assert_eq!(result.rpm_nominal, Some(18000.0));
    }
}
