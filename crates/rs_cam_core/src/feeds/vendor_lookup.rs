//! Vendor LUT lookup with scoring algorithm.
//!
//! Filters observations by must-match criteria, scores remaining candidates,
//! and returns the best match with chipload midpoint.

use super::vendor_lut::*;

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

/// Result of a successful LUT lookup.
#[derive(Debug, Clone)]
pub struct LookupResult {
    pub chip_load_mm: f64,
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
}

/// Find the best matching observation for a query.
pub fn lookup_best(lut: &VendorLut, query: &LookupQuery) -> Option<LookupResult> {
    let mut best: Option<(i64, usize)> = None;

    for (i, obs) in lut.observations.iter().enumerate() {
        // --- Must-match filter ---
        if obs.operation_family != query.operation_family {
            continue;
        }
        if obs.material_family != query.material_family {
            continue;
        }
        if !tool_family_compatible(query.tool_family, obs.tool_family) {
            continue;
        }
        // Diameter ratio must be within 0.5x to 2.0x
        let ratio = query.diameter_mm / obs.diameter_mm;
        if !(0.5..=2.0).contains(&ratio) {
            continue;
        }

        // --- Score components ---
        let mut score: i64 = 1000;

        // Tool family match
        score += tool_family_score(query.tool_family, obs.tool_family);

        // Row kind
        score += obs.row_kind.score();

        // Evidence grade
        score += obs.evidence_grade.score();

        // Flute count
        let flute_diff = (query.flute_count as i64 - obs.flute_count as i64).unsigned_abs();
        score += match flute_diff {
            0 => 80,
            1 => 30,
            _ => -20,
        };

        // Diameter proximity (0-200, log-distance, closer = higher)
        let log_ratio = (query.diameter_mm / obs.diameter_mm).ln().abs();
        let diam_score = ((1.0 - log_ratio / 2.0_f64.ln()) * 200.0).clamp(0.0, 200.0) as i64;
        score += diam_score;

        // Hardness proximity (0-80)
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

        // Subfamily match
        if let (Some(qs), Some(os)) = (&query.tool_subfamily, &obs.tool_subfamily)
            && qs == os
        {
            score += 50;
        }

        // Pass role
        if query.pass_role == obs.pass_role {
            score += 45;
        } else {
            score -= 25;
        }

        if let Some((best_score, _)) = best {
            if score > best_score {
                best = Some((score, i));
            }
        } else {
            best = Some((score, i));
        }
    }

    best.map(|(score, i)| {
        let obs = &lut.observations[i];
        let chip_load = chipload_midpoint(obs);
        LookupResult {
            chip_load_mm: chip_load,
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
        }
    })
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
mod tests {
    use super::*;

    fn embedded_lut() -> VendorLut {
        VendorLut::embedded()
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
        let result = lookup_best(&lut, &query).unwrap();
        assert_eq!(result.rpm_nominal, Some(18000.0));
    }
}
