//! Vendor LUT lookup with scoring algorithm.
//!
//! Filters observations by must-match criteria, scores remaining candidates,
//! and returns the best match with chipload midpoint.

use super::vendor_lut::{
    HardnessKind, LutOperationFamily, LutPassRole, MaterialFamily, ToolFamily, VendorLut,
    VendorObservation,
};

/// Query parameters for LUT lookup.
#[derive(Debug, Clone)]
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
///
/// Chipload fields are **scaled** to the query's diameter and hardness so the
/// engaged-edge geometry remains the truth (we don't lie about engaged
/// diameter to fit the LUT). When `is_extrapolated` is true the scaling
/// factors diverge from 1.0 by more than ±40 %, and downstream consumers
/// must demote verdict confidence to `Approximate` with a detail describing
/// the scaling.
#[derive(Debug, Clone)]
pub struct LookupResult {
    /// Scaled chipload midpoint (mm/tooth).
    pub chip_load_mm: f64,
    /// Scaled lower bound of the observation's chipload range, if reported.
    /// Below this is rubbing/burning territory.
    pub chip_load_min_mm: Option<f64>,
    /// Scaled upper bound of the observation's chipload range, if reported.
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
    /// The matched observation's calibrated diameter (mm). Useful for
    /// debug/UI to show how far the query's diameter diverged from the
    /// row's calibration.
    pub row_diameter_mm: f64,
    /// Linear scale factor applied to chipload bounds for diameter:
    /// `query.diameter_mm / row.diameter_mm`. 1.0 = exact match, no scaling.
    pub chipload_diameter_scale: f64,
    /// Linear scale factor applied to chipload bounds for hardness:
    /// `row.hardness_value / query.hardness_value` when kinds match,
    /// 1.0 otherwise. Softer query → larger factor → higher chipload.
    pub chipload_hardness_scale: f64,
    /// True when the combined chipload scaling diverges from 1.0 by more
    /// than ±40 % (`|ln(diameter × hardness)| > ln(1.4)`). Verdicts derived
    /// from this row must be reported with `Confidence::Approximate`.
    pub is_extrapolated: bool,
}

pub fn find_best_row(lut: &VendorLut, criteria: &LookupCriteria) -> Option<MatchedRow> {
    lookup_best(lut, criteria)
}

/// Maximum V-bit included-angle mismatch (degrees) tolerated before a
/// vendor row is rejected for a cone cutter. A 90° bit must not be matched
/// against a 120° V-groove row — the chip geometry and engaged width at a
/// given depth differ too much for a chipload envelope to carry over. Rows
/// that don't record an angle (legacy data) are exempt from the gate and
/// scored neutrally, so V-bit lookups never regress below `find_best_row`.
const VBIT_ANGLE_TOLERANCE_DEG: f64 = 20.0;

/// Bonus for an exact V-bit angle match, decaying linearly to 0 at the
/// tolerance edge. Sized to dominate the diameter-proximity term (≤ 200)
/// so a same-angle row outranks a closer-diameter wrong-angle row.
const VBIT_ANGLE_MATCH_BONUS: f64 = 200.0;

/// Angle-aware variant of [`find_best_row`] for V-bit / chamfer cutters.
///
/// When both the query and a candidate row carry an included angle, rows
/// whose angle differs by more than [`VBIT_ANGLE_TOLERANCE_DEG`] are
/// rejected and closer-angle rows score higher. When either side lacks an
/// angle the row passes through with neutral scoring, so this never does
/// worse than `find_best_row` on angle-less legacy data. This is the slot
/// incoming V-groove datasets plug into: tag a row with `included_angle_deg`
/// and it becomes selectable only for cutters of that cone angle.
pub fn find_best_vbit_row(
    lut: &VendorLut,
    criteria: &LookupCriteria,
    query_included_angle_deg: Option<f64>,
) -> Option<MatchedRow> {
    let mut best: Option<(i64, i64, usize)> = None;
    for (i, obs) in lut.observations.iter().enumerate() {
        if !passes_must_match(criteria, obs) {
            continue;
        }
        let angle_bonus = match (query_included_angle_deg, obs.included_angle_deg) {
            (Some(q), Some(o)) => {
                let diff = (q - o).abs();
                if diff > VBIT_ANGLE_TOLERANCE_DEG {
                    continue;
                }
                ((1.0 - diff / VBIT_ANGLE_TOLERANCE_DEG) * VBIT_ANGLE_MATCH_BONUS) as i64
            }
            _ => 0,
        };
        let (base_score, diam_score) = score_observation(criteria, obs);
        let score = base_score + angle_bonus;
        if best.is_none_or(|(bs, _, _)| score > bs) {
            best = Some((score, diam_score, i));
        }
    }
    best.map(|(score, diameter_match_score, i)| {
        // SAFETY: `i` was stored from a valid iteration over `lut.observations`
        #[allow(clippy::indexing_slicing)]
        let obs = &lut.observations[i];
        build_result(obs, criteria, score, diameter_match_score, i)
    })
}

/// Canonical LUT lookup entry point — routes V-bit / chamfer queries
/// to the angle-aware matcher, everything else to the diameter+
/// hardness matcher. Use this from every Suggest / Explain / MCP /
/// gate call site so a 30° V-bit query never silently matches a 90°
/// row.
///
/// Pre-Phase-1E only `tool_load::chipload::evaluate` did the routing
/// inline; the Suggest path and the GUI feeds modal called
/// `find_best_row` directly and lost the angle constraint, so a
/// 30° V-bit toolpath's "recommended chipload" came from whatever
/// V-bit angle happened to win the diameter score. Centralising the
/// dispatch here keeps that contract in one place.
pub fn find_best_row_for_geometry(
    lut: &VendorLut,
    criteria: &LookupCriteria,
    geometry: &crate::feeds::ToolGeometryHint,
) -> Option<MatchedRow> {
    match geometry {
        crate::feeds::ToolGeometryHint::VBit { included_angle, .. } => {
            find_best_vbit_row(lut, criteria, Some(*included_angle))
        }
        crate::feeds::ToolGeometryHint::Flat
        | crate::feeds::ToolGeometryHint::Ball
        | crate::feeds::ToolGeometryHint::Bull { .. }
        | crate::feeds::ToolGeometryHint::TaperedBall { .. } => find_best_row(lut, criteria),
    }
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
        all.push((score, build_result(obs, criteria, score, diam_score, i)));
    }
    all.sort_by_key(|(score, _)| -*score);
    all.into_iter().map(|(_, row)| row).collect()
}

/// Cap on extrapolation scaling so a wildly mismatched row can't return
/// physically absurd chipload bounds. Linear scaling outside this band is
/// clamped before being applied — the lookup still succeeds, the
/// `is_extrapolated` flag is set, and the verdict is downgraded to
/// `Approximate`. 0.1× / 10× covers a 100× diameter span which is well past
/// any real wood-tool extrapolation we want to do.
const SCALE_CLAMP_LO: f64 = 0.1;
const SCALE_CLAMP_HI: f64 = 10.0;

/// Threshold past which a scaled row is reported as extrapolated. ±40 %
/// (≈ ln 1.4) on the *combined* diameter × hardness scale. Picked to span
/// the LUT's existing diameter-row spacing (3.175 mm → 6.0 mm = 1.89×) so
/// neighbouring rows don't trip Approximate, but a 1 mm tip against a
/// 3.175 mm row (0.31× = ln 1.16) does.
const APPROX_LN_THRESHOLD: f64 = 0.336_472_236_621_213_07; // f64::ln(1.4)

fn diameter_scale_factor(query_d: f64, row_d: Option<f64>) -> f64 {
    match row_d {
        Some(rd) if rd > 0.0 && query_d > 0.0 => {
            (query_d / rd).clamp(SCALE_CLAMP_LO, SCALE_CLAMP_HI)
        }
        // No anchor diameter on the row (v-bit / diameter-window
        // article): chipload bounds carry through unscaled.
        _ => 1.0,
    }
}

fn hardness_scale_factor(query: &LookupQuery, obs: &VendorObservation) -> f64 {
    match (
        query.hardness_kind,
        query.hardness_value,
        obs.hardness_kind,
        obs.hardness_value,
    ) {
        (Some(qk), Some(qv), Some(ok), Some(ov)) if qk == ok && qv > 0.0 && ov > 0.0 => {
            // Chipload roughly inverse with hardness: softer material
            // tolerates larger chipload at the same RPM. Apply
            // `obs.hardness / query.hardness` so a hardwood-row used for a
            // softwood query (qv < ov) returns a *higher* scale (more
            // chipload allowed), and vice versa.
            (ov / qv).clamp(SCALE_CLAMP_LO, SCALE_CLAMP_HI)
        }
        _ => 1.0,
    }
}

fn build_result(
    obs: &VendorObservation,
    query: &LookupQuery,
    score: i64,
    diameter_match_score: i64,
    _idx: usize,
) -> LookupResult {
    let diameter_scale = diameter_scale_factor(query.diameter_mm, obs.diameter_mm);
    let hardness_scale = hardness_scale_factor(query, obs);
    let total_scale = diameter_scale * hardness_scale;
    let is_extrapolated = total_scale.ln().abs() > APPROX_LN_THRESHOLD;

    LookupResult {
        chip_load_mm: chipload_midpoint(obs) * total_scale,
        chip_load_min_mm: obs.chipload_min_mm_tooth.map(|v| v * total_scale),
        chip_load_max_mm: obs.chipload_max_mm_tooth.map(|v| v * total_scale),
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
        // 0.0 is a safe sentinel for "no diameter anchor" because real
        // cutter diameters are always > 0; downstream consumers that
        // care about the no-anchor case can check `< f64::EPSILON`.
        row_diameter_mm: obs.diameter_mm.unwrap_or(0.0),
        chipload_diameter_scale: diameter_scale,
        chipload_hardness_scale: hardness_scale,
        is_extrapolated,
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
        build_result(obs, query, score, diameter_match_score, i)
    })
}

/// Group `MaterialFamily` into broad categories for cross-family
/// extrapolation. Within a category we allow score-only matching (e.g.
/// hardwood query against softwood row), with chipload bounds scaled by the
/// hardness ratio. Across categories the lookup hard-rejects: cutting wood
/// and cutting aluminum live in entirely different chipload regimes and
/// linear extrapolation between them isn't meaningful.
fn material_category(family: MaterialFamily) -> u8 {
    match family {
        MaterialFamily::Softwood
        | MaterialFamily::Hardwood
        | MaterialFamily::PlywoodSoftwood
        | MaterialFamily::PlywoodHardwood
        | MaterialFamily::Mdf
        | MaterialFamily::Hdf
        | MaterialFamily::Particleboard => 0,
        MaterialFamily::Acrylic
        | MaterialFamily::Hdpe
        | MaterialFamily::Polycarbonate
        | MaterialFamily::Delrin => 1,
        MaterialFamily::Aluminum => 2,
    }
}

fn materials_compatible(query: MaterialFamily, obs: MaterialFamily) -> bool {
    material_category(query) == material_category(obs)
}

fn passes_must_match(query: &LookupQuery, obs: &VendorObservation) -> bool {
    if obs.operation_family != query.operation_family {
        return false;
    }
    // Material category is a hard filter; exact family is now a score
    // contributor in `score_observation`. Within wood/plastic/metal,
    // chipload bounds are scaled by the hardness ratio so the engaged-edge
    // truth carries through even when the LUT only has rows for a
    // neighbouring family. See `LookupResult::chipload_hardness_scale`.
    if !materials_compatible(query.material_family, obs.material_family) {
        return false;
    }
    if !tool_family_compatible(query.tool_family, obs.tool_family) {
        return false;
    }
    // Diameter ratio gate intentionally relaxed to a sanity floor only.
    // Engaged-edge geometry is the truth (`tool.lookup_diameter_at(doc)`);
    // chipload bounds get scaled to the query's diameter in `build_result`
    // and the verdict is downgraded to `Approximate` past ±40 % divergence.
    // Without the relax a 1 mm tapered-ball tip can never reach a 3.175 mm
    // calibrated row even though the LUT trends are well-behaved.
    if query.diameter_mm <= 0.0 {
        return false;
    }
    // V-bit rows and diameter-window articles intentionally carry
    // `diameter_mm = None` — they're matched by angle / material /
    // operation rather than by diameter. Let them through the gate;
    // scoring + build_result handle the no-anchor case (no diameter
    // contribution to score, scale factor pinned to 1.0).
    if let Some(obs_d) = obs.diameter_mm {
        if obs_d <= 0.0 {
            return false;
        }
        let ratio = query.diameter_mm / obs_d;
        if !(SCALE_CLAMP_LO..=SCALE_CLAMP_HI).contains(&ratio) {
            return false;
        }
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

    // Diameter scoring is skipped for rows without a diameter anchor
    // (v-bit, diameter-window articles). They still compete on the
    // other axes; v-bit angle bonus in find_best_vbit_row provides
    // the per-row specificity.
    let diam_score = match obs.diameter_mm {
        Some(obs_d) if obs_d > 0.0 => {
            let log_ratio = (query.diameter_mm / obs_d).ln().abs();
            ((1.0 - log_ratio / 2.0_f64.ln()) * 200.0).clamp(0.0, 200.0) as i64
        }
        _ => 0,
    };
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

    // Material-family exact match is a strong score contributor (was a
    // hard filter pre-relaxation). Within a category, exact-family rows
    // should still win over neighbouring-family rows so a hardwood query
    // prefers a hardwood row over a softwood row when both exist.
    if query.material_family == obs.material_family {
        score += 100;
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
    fn test_no_match_returns_none_when_outside_sanity_floor() {
        // Diameter ratio is gated by a generous sanity floor only
        // (`SCALE_CLAMP_LO..=SCALE_CLAMP_HI`, currently 0.1..=10.0). Past
        // that bound the lookup hard-rejects.
        // 2026-05-30: bumped from 100 mm to 200 mm — the Phase 1C
        // Whiteside RD5218H 12.7 mm row brought the largest hardwood
        // flat-end adaptive-roughing row into the wood category, so the
        // old 100 mm probe (100/12.7 = 7.9×) sat inside the sanity
        // floor. 200 mm restores the >10× margin (200/12.7 = 15.7×).
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::FlatEnd,
            tool_subfamily: None,
            diameter_mm: 200.0,
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
    fn test_no_match_when_material_category_differs() {
        // Material category is still a hard filter — a wood query must
        // not extrapolate from an aluminum row even though the diameter
        // and op family align.
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::BullNose,
            tool_subfamily: None,
            diameter_mm: 6.0,
            flute_count: 3,
            material_family: MaterialFamily::Aluminum,
            hardness_kind: Some(HardnessKind::Hb),
            hardness_value: Some(95.0),
            operation_family: LutOperationFamily::Parallel,
            pass_role: LutPassRole::Finish,
        };
        // No aluminum + parallel/finish + bull-nose row exists; the
        // wood-category bull-nose rows must not satisfy this query.
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
    fn diameter_scaling_linearly_scales_chipload_bounds() {
        // A 6 mm tapered ball query against the 3.175 mm hardwood
        // parallel/finish row should match (diameter ratio 1.89 is past
        // the ±40 % Approximate threshold) and scale chipload bounds
        // linearly. Compare against known LUT values: row chipload
        // 0.018–0.032 (hardwood 3.175 mm parallel/finish) → scaled
        // 0.018 × 1.89 ≈ 0.034, 0.032 × 1.89 ≈ 0.060. Hardness scale is
        // 1.0 (both rows use janka 1450).
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::TaperedBallNose,
            tool_subfamily: None,
            diameter_mm: 6.0,
            flute_count: 2,
            material_family: MaterialFamily::Hardwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(1450.0),
            operation_family: LutOperationFamily::Parallel,
            pass_role: LutPassRole::Finish,
        };
        let result = lookup_best(&lut, &query).expect("must match a tapered-ball parallel row");
        // The 6.0 mm hardwood row is an exact-diameter match.
        assert!(
            (result.row_diameter_mm - 6.0).abs() < 1e-9,
            "expected exact 6.0 mm row, got {} mm",
            result.row_diameter_mm
        );
        assert!((result.chipload_diameter_scale - 1.0).abs() < 1e-6);
        assert!((result.chipload_hardness_scale - 1.0).abs() < 1e-6);
        assert!(!result.is_extrapolated);
    }

    #[test]
    fn hardness_scaling_uses_obs_over_query() {
        // A softwood query should match the softwood parallel/finish row
        // (no extrapolation needed). Hardness scale = obs/query = 600/600
        // = 1.0 because the row's hardness equals the query's.
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::TaperedBallNose,
            tool_subfamily: None,
            diameter_mm: 6.0,
            flute_count: 2,
            material_family: MaterialFamily::Softwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(600.0),
            operation_family: LutOperationFamily::Parallel,
            pass_role: LutPassRole::Finish,
        };
        let result = lookup_best(&lut, &query).expect("must match softwood row");
        assert!((result.chipload_hardness_scale - 1.0).abs() < 1e-6);
    }

    /// Phase 1C smoke — an aluminum query against the embedded LUT
    /// must now reach a row. Before the 2026-05-30 ingest the LUT
    /// carried zero `MaterialFamily::Aluminum` rows; after Phase 1C the
    /// Helical 6061 + Amana ZrN aluminum + Garr (deferred) sources
    /// land an aluminum-only category at every roughing-class flat-end
    /// query in the 3–13 mm diameter window.
    #[test]
    fn phase_1c_aluminum_query_reaches_a_row() {
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::FlatEnd,
            tool_subfamily: None,
            diameter_mm: 6.35,
            flute_count: 3,
            material_family: MaterialFamily::Aluminum,
            hardness_kind: Some(HardnessKind::Hb),
            hardness_value: Some(95.0),
            operation_family: LutOperationFamily::Adaptive,
            pass_role: LutPassRole::Roughing,
        };
        let result = lookup_best(&lut, &query)
            .expect("Phase 1C wires aluminum rows live; an adaptive/roughing flat-end aluminum query must match");
        assert!(result.chip_load_mm > 0.0);
    }

    #[test]
    fn material_category_hard_filter_blocks_wood_to_metal() {
        // Wood query must not extrapolate from aluminum row even when
        // op family / diameter / pass-role would otherwise permit it.
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::BullNose,
            tool_subfamily: None,
            diameter_mm: 6.0,
            flute_count: 3,
            material_family: MaterialFamily::Hardwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(1450.0),
            operation_family: LutOperationFamily::Adaptive,
            pass_role: LutPassRole::Roughing,
        };
        let result = lookup_best(&lut, &query).expect("hardwood bull-nose adaptive row exists");
        // The aluminum row in `amana_3d_profiling.json` must not be
        // selected — its observation_id contains "aluminum".
        assert!(
            !result.observation_id.contains("aluminum"),
            "expected wood-category row, got {}",
            result.observation_id
        );
    }

    #[test]
    fn test_sub_1mm_tapered_ball_hardwood_finish_extrapolates() {
        // Used to be a "documented gap" — pre-G5/G6/G7 (2026-05-08) the
        // [0.5, 2.0] hard ratio gate refused on the larger rows and the
        // lookup returned None. With engaged-edge scaling the lookup
        // now matches the closest hardwood tapered-ball row, scales
        // chipload bounds by the diameter ratio, and flags the result
        // as extrapolated so verdicts derived from it carry `Approximate`
        // confidence.
        //
        // Asserts the spirit (extrapolation + scaling), not a specific
        // row id — Phase 4+ LUT promotions can legitimately introduce
        // closer matches without invalidating the property.
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::TaperedBallNose,
            tool_subfamily: None,
            diameter_mm: 0.5,
            flute_count: 2,
            material_family: MaterialFamily::Hardwood,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(1450.0),
            operation_family: LutOperationFamily::Parallel,
            pass_role: LutPassRole::Finish,
        };
        let result = lookup_best(&lut, &query)
            .expect("0.5mm hardwood tapered ball should extrapolate from the closest available row");
        assert!(
            result.is_extrapolated,
            "0.5mm against any tapered-ball hardwood row in the LUT must trip extrapolation"
        );
        assert!(
            result.row_diameter_mm >= 1.0,
            "expected to scale up from a >=1mm row (no sub-1mm tapered hardwood row \
             exists), got row diameter {}",
            result.row_diameter_mm
        );
        let expected_scale = 0.5 / result.row_diameter_mm;
        assert!(
            (result.chipload_diameter_scale - expected_scale).abs() < 1e-6,
            "scale should be query/row = {expected_scale}, got {}",
            result.chipload_diameter_scale
        );
        // Same hardness anchor (natural-wood Janka 1450), so hardness scale is unity.
        assert!((result.chipload_hardness_scale - 1.0).abs() < 1e-6);
        // Scaled chipload must be strictly less than the row's raw min.
        if let Some(scaled) = result.chip_load_min_mm {
            assert!(
                scaled < 0.1,
                "scaled chipload {scaled} must be sub-0.1 mm/tooth at 0.5 mm tool",
            );
        }
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

    fn vbit_query(diameter_mm: f64, material: MaterialFamily, janka: f64) -> LookupQuery {
        LookupQuery {
            tool_family: ToolFamily::ChamferVbit,
            tool_subfamily: None,
            diameter_mm,
            flute_count: 2,
            material_family: material,
            hardness_kind: Some(HardnessKind::Janka),
            hardness_value: Some(janka),
            operation_family: LutOperationFamily::Trace,
            pass_role: LutPassRole::Finish,
        }
    }

    #[test]
    fn vbit_90deg_matches_insert_vgroove_not_120() {
        // A 90° V-bit must select an Amana insert-vgroove (90°) row and
        // never the Whiteside 120° row, whose cone angle is 30° away —
        // past the 20° tolerance.
        let lut = embedded_lut();
        let query = vbit_query(6.0, MaterialFamily::Hardwood, 1450.0);
        let result =
            find_best_vbit_row(&lut, &query, Some(90.0)).expect("90° V-bit must match a 90° row");
        assert_eq!(result.observation_id, "amana-vbit-hardwood-trace-6000-2f");
    }

    #[test]
    fn vbit_120deg_matches_120_row_not_90() {
        // A 120° V-bit must select the Whiteside 120° row, not the 90°
        // Amana rows (30° away, rejected by the tolerance gate).
        let lut = embedded_lut();
        let query = vbit_query(12.0, MaterialFamily::Hardwood, 1450.0);
        let result = find_best_vbit_row(&lut, &query, Some(120.0))
            .expect("120° V-bit must match the 120° row");
        assert_eq!(result.observation_id, "whiteside-vbit-hardwood-trace-12000-2f");
    }

    #[test]
    fn vbit_no_query_angle_falls_back_to_score_only() {
        // With no query angle the gate is inert — every angle-compatible
        // row is eligible and the best-scoring one wins (legacy behaviour,
        // never worse than find_best_row).
        let lut = embedded_lut();
        let query = vbit_query(6.0, MaterialFamily::Hardwood, 1450.0);
        let with_none = find_best_vbit_row(&lut, &query, None).expect("angle-less query matches");
        let via_plain = find_best_row(&lut, &query).expect("plain lookup matches");
        assert_eq!(with_none.observation_id, via_plain.observation_id);
    }

    #[test]
    fn dispatcher_routes_vbit_to_angle_aware_lookup() {
        // S1-2 contract: `find_best_row_for_geometry` must route a
        // V-bit geometry hint to the angle-aware matcher, so a 90°
        // query never silently lands a 120° row. With dispatcher,
        // identical to calling find_best_vbit_row directly; without
        // dispatcher, the plain find_best_row would ignore the angle.
        let lut = embedded_lut();
        let query = vbit_query(6.0, MaterialFamily::Hardwood, 1450.0);
        let geom = crate::feeds::ToolGeometryHint::VBit {
            included_angle: 90.0,
            tip_diameter: 0.0,
        };
        let via_dispatch = find_best_row_for_geometry(&lut, &query, &geom)
            .expect("dispatcher must reach the 90° row");
        let via_direct = find_best_vbit_row(&lut, &query, Some(90.0))
            .expect("direct angle-aware lookup must reach the 90° row");
        assert_eq!(via_dispatch.observation_id, via_direct.observation_id);
    }

    #[test]
    fn dispatcher_routes_flat_to_plain_lookup() {
        // Non-V-bit geometry falls through to find_best_row — angle
        // gate doesn't apply. Identical to calling find_best_row.
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
        let geom = crate::feeds::ToolGeometryHint::Flat;
        let via_dispatch =
            find_best_row_for_geometry(&lut, &query, &geom).expect("dispatch must match flat");
        let via_direct = find_best_row(&lut, &query).expect("plain must match flat");
        assert_eq!(via_dispatch.observation_id, via_direct.observation_id);
    }

    #[test]
    fn vbit_matches_angle_only_row_without_diameter() {
        // Phase 5 Step 5.2: a 30° Spektra engrave row carrying
        // `diameter_mm = None` must be selectable by an angle-aware
        // V-bit query of the same angle. Pre-relaxation `passes_must_match`
        // rejected any obs with `diameter_mm <= 0.0`; the new row would
        // have been invisible to the lookup.
        let lut = embedded_lut();
        let query = vbit_query(3.175, MaterialFamily::Softwood, 600.0);
        let result = find_best_vbit_row(&lut, &query, Some(30.0))
            .expect("30° angle-only v-bit row must be reachable for a 30° query");
        // Either the new Spektra engrave row OR the legacy
        // `amana-vgroove-softwood-trace-30deg-1f` row (which carries
        // diameter_mm = 6.35) can win on score; both have the same
        // chipload data. Assert we land a 30° softwood-class row,
        // not a far-angle row.
        let won_an_angle_only_row = result.row_diameter_mm < f64::EPSILON;
        let won_the_legacy_row =
            result.observation_id == "amana-vgroove-softwood-trace-30deg-1f";
        assert!(
            won_an_angle_only_row || won_the_legacy_row,
            "expected angle-only or legacy 30° row, got {} (row_diameter={})",
            result.observation_id,
            result.row_diameter_mm
        );
    }

    #[test]
    fn diameter_window_row_matches_flat_query_no_extrapolation() {
        // Phase 5 Step 5.2: the Onsrud polycarbonate article row
        // (diameter_mm = None) is matched on tool_family + material +
        // operation. A polycarbonate contour-finish query against the
        // embedded LUT must reach it and the chipload bounds must
        // carry through unscaled (no diameter extrapolation).
        let lut = embedded_lut();
        let query = LookupQuery {
            tool_family: ToolFamily::FlatEnd,
            tool_subfamily: Some("o_flute_upcut".to_owned()),
            diameter_mm: 6.35,
            flute_count: 1,
            material_family: MaterialFamily::Polycarbonate,
            hardness_kind: None,
            hardness_value: None,
            operation_family: LutOperationFamily::Contour,
            pass_role: LutPassRole::Finish,
        };
        let result = lookup_best(&lut, &query)
            .expect("Onsrud polycarbonate window row must be reachable");
        // Anchorless rows return `chipload_diameter_scale = 1.0` regardless
        // of query diameter — diameter extrapolation is intentionally
        // off for these rows.
        if result.row_diameter_mm < f64::EPSILON {
            assert!(
                (result.chipload_diameter_scale - 1.0).abs() < 1e-9,
                "anchorless row must report scale=1.0, got {}",
                result.chipload_diameter_scale,
            );
        }
    }

    #[test]
    fn vbit_rejects_row_outside_angle_tolerance() {
        // A 150° bit has no row within 20° (the embedded ladder tops out at
        // 120°), so the angle-aware lookup refuses rather than returning a
        // far-angle row. (60° now matches a real 60° V-groove row, so this
        // probe uses an angle beyond the populated range.)
        let lut = embedded_lut();
        let query = vbit_query(6.0, MaterialFamily::Hardwood, 1450.0);
        assert!(
            find_best_vbit_row(&lut, &query, Some(150.0)).is_none(),
            "150° bit must not match any row (nearest is 120°, 30° away)"
        );
    }
}
