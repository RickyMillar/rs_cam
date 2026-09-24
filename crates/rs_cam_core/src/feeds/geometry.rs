//! Geometric helpers for feeds calculation — effective diameter, chip thinning,
//! scallop stepover, V-bit width at depth.
//!
//! These are pure geometry functions with no material/machine dependencies.
//! Ported from reference/shapeoko_feeds_and_speeds/src/calcs.rs lines 523-720.

/// Radial Chip Thinning Factor (RCTF).
///
/// When radial engagement is less than half the tool diameter, the actual chip
/// is thinner than the nominal feed-per-tooth. Feed rate must increase to
/// maintain consistent chip load.
///
/// `ae_mm` — radial width of cut (mm)
/// `diameter_mm` — tool diameter (mm)
///
/// Returns a multiplier >= 1.0 to apply to the nominal feed rate.
pub fn radial_chip_thinning_factor(ae_mm: f64, diameter_mm: f64) -> f64 {
    if diameter_mm <= 0.0 || ae_mm <= 0.0 {
        return 1.0;
    }
    let ae_ratio = (ae_mm / diameter_mm).clamp(0.0, 0.5);
    if ae_ratio >= 0.5 {
        return 1.0;
    }
    let denom = (1.0 - (1.0 - 2.0 * ae_ratio).powi(2)).sqrt();
    if denom <= 0.0 {
        1.0
    } else {
        (1.0 / denom).clamp(1.0, 4.0)
    }
}

/// Effective cutting diameter for a ball nose end mill at shallow axial depth.
///
/// For `ap >= R` (radius), the full diameter is engaged.
/// For `ap < R`, only a smaller circle of the ball contacts the material.
pub fn ball_effective_diameter(nominal_d: f64, axial_depth: f64) -> f64 {
    if nominal_d <= 0.0 {
        return 0.0;
    }
    let radius = nominal_d * 0.5;
    let ap = axial_depth.max(0.0);
    if ap <= 0.0 {
        return 0.01_f64.max(nominal_d * 0.01);
    }
    if ap >= radius {
        return nominal_d;
    }
    let value = 2.0 * (ap * (nominal_d - ap)).sqrt();
    value.max(0.01)
}

// There is deliberately NO `tapered_ball_effective_diameter` here.
//
// C3 (2026-08-02) retired it. It was a STRAIGHT CONE rooted at the tip
// radius, `2*(tip_r + ap*tan(alpha))`, with no tangency blend — a third
// implementation of a profile the crate already models exactly twice, in
// [`crate::tool::MillingCutter::width_at_height`] on `TaperedBallEndmill`
// and in [`crate::feeds::ToolGeometryHint::engaged_diameter_at_doc`].
//
// It was also inert: every production call site reached it through
// `feeds::effective_diameter`, which passes the tool's own named diameter as
// `nominal_d` and derives `tip_r` from the same tool, so `tip_r ==
// nominal_d / 2` and the whole expression collapsed under its own
// `.clamp(0.01, nominal_d)` to the constant `nominal_d`. A tapered ball was
// therefore fed as if it engaged its full tip diameter at any depth, while
// a plain ball of the same tip got the exact contact circle — up to a 2.1x
// feed difference for the same physical tip at 0.05 mm DOC.
//
// The one caller now asks `ToolGeometryHint::engaged_diameter_at_doc`, which
// is the same geometry as the cutter trait and is kept honest against it by
// `feeds::tests::engaged_diameter_at_doc_matches_lookup_diameter_at_across_shapes`.
// The measured cost of the retired model is pinned in
// `tests/tapered_width_model_parity_c3.rs`.

/// Effective cutting diameter for a bull nose end mill.
///
/// Below the corner radius, behaves like a ball of diameter `2*corner_r`.
/// Above the corner radius, full nominal diameter.
pub fn bull_nose_effective_diameter(nominal_d: f64, corner_r: f64, axial_depth: f64) -> f64 {
    if nominal_d <= 0.0 || corner_r <= 0.0 {
        return nominal_d.max(0.01);
    }
    let ap = axial_depth.max(0.0);
    if ap <= corner_r {
        let corner_effective = ball_effective_diameter(2.0 * corner_r, ap);
        corner_effective.clamp(0.01, nominal_d)
    } else {
        nominal_d
    }
}

/// Scallop-based stepover for a ball nose end mill.
///
/// Given a target scallop height, computes the required stepover distance.
/// Returns `None` if the scallop target is invalid (>= ball radius or <= 0).
pub fn scallop_stepover(ball_radius: f64, target_scallop: f64) -> Option<f64> {
    if ball_radius <= 0.0 || target_scallop <= 0.0 || target_scallop >= ball_radius {
        return None;
    }
    let inside = 2.0 * ball_radius * target_scallop - target_scallop.powi(2);
    if inside <= 0.0 {
        return None;
    }
    Some(2.0 * inside.sqrt())
}

/// Axial chip thinning factor for ball nose tools.
///
/// When ball effective diameter < nominal, the chip is thinner along the axis.
/// Compensate by multiplying feed by (nominal / effective), clamped to [1.0, 4.0].
pub fn axial_chip_thinning_factor_for_ball(nominal_d: f64, effective_d: f64) -> f64 {
    if nominal_d <= 0.0 || effective_d <= 0.0 {
        return 1.0;
    }
    (nominal_d / effective_d).clamp(1.0, 4.0)
}

/// The published depth-of-cut de-rate: the one scale for the feed and for
/// the chipload band.
///
/// Three wood vendors print the same three points. The exact strings are:
///
/// - Onsrud, `onsrud_plastic` rows' `ap_rule`: "cut depth per pass = cutting
///   edge diameter (1xD); 2xD reduce chip load 25%; 3xD reduce chip load
///   50%". The Onsrud wood sheets print "DEPTH OF CUT: 1 x D Use recommended
///   chip load / 2 x D Reduce chip load by 25% / 3 x D Reduce chip load by
///   50%".
/// - Freud: "If Cut Depth is 2X the bit diameter, reduce the Chip Load by at
///   least 25%" and "If Cut Depth is 3X the bit diameter, reduce the Chip
///   Load by at least 50%".
/// - Amana: "1 x D Use recommended feed rate / 2 x D Reduce feed rate by 25%
///   / 3 x D Reduce feed rate by 50%". At the chart's fixed RPM and flute
///   count a feed reduction is the same chip-load reduction.
///
/// The vendors publish the rule at three points only. The linear
/// interpolation between them is this crate's own. No vendor prints a value
/// above 3 x D, so the scale holds the last printed point, 0.50, there, and
/// [`depth_beyond_published_table`] tells the caller that it did.
///
/// Consumers: the feed (through [`depth_tier_multiplier`]), the chipload
/// band that `feeds::calculate` returns (at the shipped depth), Suggest's
/// band re-derivation, and the post-simulation chipload gate in
/// `tool_load::chipload` (at the measured peak depth). One function keeps
/// them in phase; an earlier private copy in the gate drifted and caused
/// the v3.0c Wanaka Back Rough false `Exceeds(High)`.
///
/// | ratio (DOC/D) | scale                              |
/// |---------------|------------------------------------|
/// | <= 1.0        | 1.000 (printed)                    |
/// | 2.0           | 0.750 (printed)                    |
/// | 3.0           | 0.500 (printed)                    |
/// | between       | linear (this crate's own)          |
/// | > 3.0         | 0.500 (held at the last printed point) |
pub fn doc_derating_scale(ratio: f64) -> f64 {
    if !ratio.is_finite() || ratio <= 1.0 {
        1.0
    } else if ratio <= 2.0 {
        1.0 - 0.25 * (ratio - 1.0)
    } else if ratio <= 3.0 {
        0.75 - 0.25 * (ratio - 2.0)
    } else {
        0.5
    }
}

/// The last depth-to-diameter ratio that the vendors print (3 x D).
pub const DOC_DERATE_LAST_PRINTED_RATIO: f64 = 3.0;

/// True when the depth is past the published de-rate table.
///
/// Above [`DOC_DERATE_LAST_PRINTED_RATIO`] no vendor prints a factor, and
/// [`doc_derating_scale`] holds 0.50. The static checks raise a Caution
/// from this answer.
pub fn depth_beyond_published_table(ratio: f64) -> bool {
    ratio.is_finite() && ratio > DOC_DERATE_LAST_PRINTED_RATIO
}

/// How strictly a chipload band's bounds must be populated before a
/// consumer will act on it.
///
/// Suggest and the post-sim gates disagree here: Suggest needs a
/// complete envelope to aim `SuggestAggressiveness` at a specific
/// point, but the post-sim chipload trip gate can still flag a
/// breakage risk (chipload above the max) even when a row publishes no
/// rubbing/burn floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChiploadBoundPolicy {
    /// Both bounds must be present, finite, positive, and ordered
    /// (`min <= max`); a present-but-invalid bound rejects the row
    /// outright (matches `RequireBoth`'s pre-existing behavior at
    /// every site that used it — see call sites for detail). Used by
    /// Suggest's feed-up recalibration (`feeds::calculate` and
    /// `suggest::recompute_chipload_bounds_for_dpp`) and by
    /// `tool_load::chipload_envelopes_for_session`'s viewport-coloring
    /// `Range<f64>`, which structurally can't express a one-sided band.
    RequireBoth,
    /// A genuinely absent low bound is acceptable: burn/rubbing can't
    /// be modeled without a floor, but the high-side breakage bound
    /// alone is still actionable. A *present-but-invalid* low bound
    /// still rejects the row (same rule as `RequireBoth` — only
    /// "missing" degrades to a half-band, not "malformed"). Used by
    /// the post-sim chipload trip gate
    /// (`tool_load::chipload::matched_chip_envelope`'s caller).
    AllowHalfBand,
}

/// A DOC-derated chipload band (mm/tooth), after [`doc_derating_scale`]
/// has been applied to whichever raw LUT bounds passed
/// [`ChiploadBoundPolicy`] validation. `min_mm_per_tooth` is `None`
/// only when `AllowHalfBand` accepted a row with no published floor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeratedChiploadBand {
    pub min_mm_per_tooth: Option<f64>,
    pub max_mm_per_tooth: f64,
}

impl DeratedChiploadBand {
    /// Collapse to a both-bounds-required pair (e.g. Suggest's
    /// `ChiploadBounds`). `None` if the low bound is absent —
    /// unreachable for bands built with
    /// `ChiploadBoundPolicy::RequireBoth`, but the conversion stays
    /// total rather than assuming the invariant.
    pub fn into_pair(self) -> Option<(f64, f64)> {
        self.min_mm_per_tooth
            .map(|min| (min, self.max_mm_per_tooth))
    }
}

/// Validate and DOC-derate a vendor LUT row's published chipload
/// bounds.
///
/// Single home for the wrapper around [`doc_derating_scale`] that used
/// to be independently hand-maintained at four call sites: Suggest's
/// in-place derivation in `feeds::calculate`, Suggest's post-mutation
/// re-derivation in `suggest::recompute_chipload_bounds_for_dpp`, the
/// post-sim chipload trip gate in `tool_load::chipload`, and the
/// viewport-coloring envelope map in
/// `tool_load::chipload_envelopes_for_session`. See
/// `planning/finishing_stack_review_2026-07.md` S.8.
///
/// `doc_ratio` is the caller's `axial_doc_mm / effective_diameter_mm`.
/// Callers guard the zero/negative-diameter case slightly differently
/// (some floor the diameter before dividing, some short-circuit the
/// ratio to `0.0`, some force `0.0` outright to bypass derating for an
/// operation family it doesn't apply to, e.g. drilling) — all are
/// equivalent inputs here, since [`doc_derating_scale`] maps every
/// ratio `<= 1.0` to a scale of `1.0`.
///
/// Returns `None` when the raw bounds don't satisfy `policy`: the max
/// bound must always be present, finite, and positive; the min bound,
/// when present, must additionally be finite, positive, and `<= max`
/// (a present-but-invalid min rejects the row under both policies —
/// only an *absent* min is treated as "no floor published" and allowed
/// through under `AllowHalfBand`).
pub fn derate_chipload_bounds(
    min_mm_per_tooth: Option<f64>,
    max_mm_per_tooth: Option<f64>,
    doc_ratio: f64,
    policy: ChiploadBoundPolicy,
) -> Option<DeratedChiploadBand> {
    let max = match max_mm_per_tooth {
        Some(max) if max.is_finite() && max > 0.0 => max,
        _ => return None,
    };
    let min = match min_mm_per_tooth {
        Some(min) if min.is_finite() && min > 0.0 && min <= max => Some(min),
        Some(_invalid) => return None,
        None => None,
    };
    if min.is_none() && matches!(policy, ChiploadBoundPolicy::RequireBoth) {
        return None;
    }
    let scale = doc_derating_scale(doc_ratio);
    Some(DeratedChiploadBand {
        min_mm_per_tooth: min.map(|m| m * scale),
        max_mm_per_tooth: max * scale,
    })
}

/// The depth de-rate on the feed, at axial depth `ap` on a tool of
/// `diameter`.
///
/// This is [`doc_derating_scale`] at `ap / diameter`, so the feed and the
/// chipload band use one function (feeds matrix ruling R3, 2026-09-23).
/// A non-positive diameter gets no de-rate.
pub fn depth_tier_multiplier(ap: f64, diameter: f64) -> f64 {
    if diameter <= 0.0 {
        return 1.0;
    }
    doc_derating_scale(ap / diameter)
}

/// **The diameter the feed's depth ladder divides by, at axial depth
/// `ap`.** Feeds matrix R2 (2026-09-23).
///
/// - Tapered ball: the engaged diameter at `ap`
///   ([`crate::feeds::ToolGeometryHint::engaged_diameter_at_doc`]). That
///   is the diameter at which `feeds::calculate` de-rates the band and at
///   which the post-sim chipload gate de-rates at the peak
///   (`lookup_diameter_at`, the parity twin; the ×0.704 of EVIDENCE
///   6-10). Before R2 the feed divided by the tip, so the feed and the
///   band read two ratios on one tool.
/// - Flat, ball, bull: the nominal diameter, which is also the engaged
///   diameter for these shapes.
/// - V-bit: the nominal diameter, unchanged. The band de-rates a V-bit at
///   its engaged width; moving the feed to that width changes narrow
///   V-bit feeds, which R2 did not rule on.
///
/// `feeds::calculate` Step 5a, Suggest pass 9 and the pass 9 sentry all
/// call this one function.
#[must_use]
pub fn feed_ladder_diameter_mm(
    geom: crate::feeds::ToolGeometryHint,
    ap: f64,
    nominal_d: f64,
    shank_d: f64,
) -> f64 {
    match geom {
        crate::feeds::ToolGeometryHint::TaperedBall { .. } => {
            geom.engaged_diameter_at_doc(ap.max(0.0), nominal_d, shank_d)
        }
        crate::feeds::ToolGeometryHint::Flat
        | crate::feeds::ToolGeometryHint::Ball
        | crate::feeds::ToolGeometryHint::Bull { .. }
        | crate::feeds::ToolGeometryHint::VBit { .. } => nominal_d,
    }
}

/// **The diameter the rigidity depth cap multiplies, at one axial depth.**
/// Feeds matrix R2 (2026-09-23).
///
/// The cap is `factor × D` (`RigidityProfile::depth_cap_mm`). Before R2
/// the Suggest clamp gave it the tip of a tapered ball and the
/// post-simulation depth gate gave it the shank (EVIDENCE 5.1-7, 3.5-15).
/// Both doors now call this function, each at its own depth: Suggest at
/// the depth it ships, the gate at the measured peak.
///
/// - Tapered ball: [`MillingCutter::lookup_diameter_at`], the engaged
///   diameter at the depth. That is the ball chord below the tangency
///   height and the cone diameter above it, capped at the shank. It is
///   the same diameter at which the chipload gate applies the depth
///   ladder (the ×0.704 of EVIDENCE 6-10), and at which
///   `feeds::calculate` de-rates the feed and the band
///   (`ToolGeometryHint::engaged_diameter_at_doc`, the parity twin).
/// - Flat, ball and bull: the nominal diameter, which is also
///   `lookup_diameter_at` for these shapes.
/// - V-bit: the nominal diameter. The engaged width of a V-bit goes to
///   zero at the tip, so a cap of `factor × width` has no positive depth
///   that satisfies it.
///
/// A negative or non-finite depth reads as zero.
#[must_use]
pub fn depth_cap_diameter_mm(cutter: &dyn crate::tool::MillingCutter, depth_mm: f64) -> f64 {
    let depth = if depth_mm.is_finite() {
        depth_mm.max(0.0)
    } else {
        0.0
    };
    match cutter.geometry_hint() {
        crate::feeds::ToolGeometryHint::TaperedBall { .. } => cutter.lookup_diameter_at(depth),
        crate::feeds::ToolGeometryHint::Flat
        | crate::feeds::ToolGeometryHint::Ball
        | crate::feeds::ToolGeometryHint::Bull { .. }
        | crate::feeds::ToolGeometryHint::VBit { .. } => cutter.diameter(),
    }
}

/// **The diameter at which the vendor LUT row is looked up and scaled,
/// at axial depth `ap`.** Ruling A1 (operator, 2026-09-24,
/// `planning/extrapolation_2026-09-24/RULINGS.md`).
///
/// - Tapered ball: the nominal diameter, which is the ball tip. Every
///   tapered chart (Onsrud, Amana, SpeTool, Whiteside) prints the
///   chipload against the tip, so the row is read at the tip at every
///   depth. A1 reopens the lookup half of feeds matrix R2 only. The depth
///   half of R2 stands: the depth ladder ([`feed_ladder_diameter_mm`]),
///   the depth cap ([`depth_cap_diameter_mm`]), the band de-rate and the
///   deflection gate still use the engaged cone diameter at the depth.
/// - V-bit: the engaged width at `ap`
///   ([`crate::feeds::ToolGeometryHint::engaged_diameter_at_doc`]),
///   unchanged. No ruling moves the V-bit key yet (G5 / B4).
/// - Flat, ball, bull: the nominal diameter, unchanged.
///
/// `nominal_d` is the tool diameter that Suggest carries
/// (`ToolConfig::diameter`, the tip of a tapered ball). Suggest calls
/// this function; the gate, the viewport and the optimizer call the
/// cutter twin [`lut_key_diameter_for_cutter`]. The two give one key for
/// one tool, so all four consumers read the same row.
#[must_use]
pub fn lut_key_diameter_mm(
    geom: crate::feeds::ToolGeometryHint,
    ap: f64,
    nominal_d: f64,
    shank_d: f64,
) -> f64 {
    match geom {
        crate::feeds::ToolGeometryHint::TaperedBall { .. } => nominal_d,
        crate::feeds::ToolGeometryHint::VBit { .. } => {
            geom.engaged_diameter_at_doc(ap.max(0.0), nominal_d, shank_d)
        }
        crate::feeds::ToolGeometryHint::Flat
        | crate::feeds::ToolGeometryHint::Ball
        | crate::feeds::ToolGeometryHint::Bull { .. } => nominal_d,
    }
}

/// **The LUT lookup key of a cutter, at axial depth `ap`.** The cutter
/// twin of [`lut_key_diameter_mm`] (ruling A1, 2026-09-24,
/// `planning/extrapolation_2026-09-24/RULINGS.md`).
///
/// - Tapered ball: the ball tip, `2 × tip_radius` from the geometry hint.
///   Do not use `cutter.diameter()` here: on a tapered ball it is the
///   shaft diameter, not the tip. A hint with no tip radius falls back to
///   `cutter.diameter()`.
/// - V-bit: [`crate::tool::MillingCutter::lookup_diameter_at`], the
///   engaged width at `ap`, unchanged.
/// - Flat, ball, bull: the nominal diameter.
///
/// The depth half of R2 stands. The depth ladder, the depth cap, the
/// band de-rate and the deflection gate read the engaged cone diameter
/// (`lookup_diameter_at`), not this key.
#[must_use]
pub fn lut_key_diameter_for_cutter(cutter: &dyn crate::tool::MillingCutter, ap: f64) -> f64 {
    match cutter.geometry_hint() {
        crate::feeds::ToolGeometryHint::TaperedBall { tip_radius, .. } => {
            if tip_radius.is_finite() && tip_radius > 0.0 {
                2.0 * tip_radius
            } else {
                cutter.diameter()
            }
        }
        crate::feeds::ToolGeometryHint::VBit { .. } => {
            let depth = if ap.is_finite() { ap.max(0.0) } else { 0.0 };
            cutter.lookup_diameter_at(depth)
        }
        crate::feeds::ToolGeometryHint::Flat
        | crate::feeds::ToolGeometryHint::Ball
        | crate::feeds::ToolGeometryHint::Bull { .. } => cutter.diameter(),
    }
}

/// V-bit cut width at a given depth.
///
/// `included_angle` — full V angle in degrees
/// `tip_d` — tip flat diameter (mm), 0 for pointed
/// `ap` — axial depth (mm)
pub fn vbit_width_at_depth(included_angle: f64, tip_d: f64, ap: f64) -> Option<f64> {
    if included_angle <= 0.0 || included_angle >= 180.0 || tip_d < 0.0 || ap < 0.0 {
        return None;
    }
    let half_angle = (included_angle * 0.5).to_radians();
    let width = tip_d + 2.0 * ap * half_angle.tan();
    Some(width.max(tip_d))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_rctf_full_engagement() {
        let f = radial_chip_thinning_factor(6.0, 6.0);
        assert!((f - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_rctf_half_engagement() {
        let f = radial_chip_thinning_factor(3.0, 6.0);
        assert!((f - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_rctf_quarter_engagement() {
        let f = radial_chip_thinning_factor(1.5, 6.0);
        assert!(f > 1.1 && f < 1.3, "got {f}");
    }

    #[test]
    fn test_rctf_light_engagement() {
        let f = radial_chip_thinning_factor(0.6, 6.0);
        assert!(f > 1.5, "got {f}");
    }

    #[test]
    fn test_rctf_zero_engagement() {
        assert_eq!(radial_chip_thinning_factor(0.0, 6.0), 1.0);
    }

    #[test]
    fn test_ball_effective_full_depth() {
        let d_eff = ball_effective_diameter(6.0, 3.0);
        assert!((d_eff - 6.0).abs() < 1e-9);
    }

    #[test]
    fn test_ball_effective_shallow() {
        let d_eff = ball_effective_diameter(6.0, 0.3);
        assert!(d_eff > 0.0 && d_eff < 6.0, "got {d_eff}");
    }

    // `test_tapered_ball_effective_scales_with_depth` lived here until C3.
    // It passed `nominal_d = 6.0` with `tip_r = 0.5` — a binding no call site
    // produces — so it exercised the one branch of the retired straight-cone
    // model that was not clamped flat, and read healthy while the reachable
    // behaviour was a constant. Its replacement is the DOC sweep in
    // `tests/tapered_width_model_parity_c3.rs`, which drives the tapered arm
    // through `feeds::calculate` at the production binding.

    #[test]
    fn test_bull_nose_transitions_to_nominal() {
        let shallow = bull_nose_effective_diameter(6.0, 1.0, 0.2);
        let deep = bull_nose_effective_diameter(6.0, 1.0, 1.5);
        assert!(shallow < 6.0);
        assert!((deep - 6.0).abs() < 1e-9);
    }

    #[test]
    fn test_scallop_stepover_valid() {
        let stepover = scallop_stepover(3.0, 0.03).expect("should be valid");
        assert!(stepover > 0.0 && stepover < 6.0);
    }

    #[test]
    fn test_scallop_stepover_invalid() {
        assert!(scallop_stepover(3.0, 3.0).is_none());
        assert!(scallop_stepover(3.0, 0.0).is_none());
        assert!(scallop_stepover(0.0, 0.03).is_none());
    }

    #[test]
    fn test_vbit_width_increases_with_depth() {
        let shallow = vbit_width_at_depth(60.0, 0.2, 0.2).expect("valid V-bit params");
        let deep = vbit_width_at_depth(60.0, 0.2, 1.0).expect("valid V-bit params");
        assert!(deep > shallow);
    }

    #[test]
    fn test_vbit_invalid_angles() {
        assert!(vbit_width_at_depth(0.0, 0.2, 1.0).is_none());
        assert!(vbit_width_at_depth(180.0, 0.2, 1.0).is_none());
    }

    #[test]
    fn test_scallop_stepover_reference_value() {
        // 3mm ball radius, 0.1mm scallop target
        // stepover = 2 * sqrt(2*R*h - h^2) = 2 * sqrt(2*3*0.1 - 0.01) = 2 * sqrt(0.59) ≈ 1.536
        let stepover = scallop_stepover(3.0, 0.1).expect("valid scallop params");
        assert!((stepover - 1.536).abs() < 0.01, "got {stepover}");
    }

    #[test]
    fn test_axial_thinning_at_full_depth() {
        // When effective == nominal, no thinning
        assert!((axial_chip_thinning_factor_for_ball(6.0, 6.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_axial_thinning_at_shallow_cut() {
        // Ball nose 6mm, effective ~2.68mm at 0.3mm depth → factor ~2.24
        let d_eff = ball_effective_diameter(6.0, 0.3);
        let factor = axial_chip_thinning_factor_for_ball(6.0, d_eff);
        assert!(factor > 1.5, "expected thinning factor > 1.5, got {factor}");
        assert!(factor < 4.0);
    }

    #[test]
    fn test_axial_thinning_clamped_to_4() {
        assert!((axial_chip_thinning_factor_for_ball(6.0, 0.5) - 4.0).abs() < 1e-9);
    }

    #[test]
    fn test_depth_tier_multiplier_shallow() {
        assert!((depth_tier_multiplier(3.0, 6.0) - 1.0).abs() < 1e-9); // 0.5D
    }

    #[test]
    fn test_depth_tier_multiplier_1d() {
        assert!((depth_tier_multiplier(6.0, 6.0) - 1.0).abs() < 1e-9); // exactly 1D
    }

    #[test]
    fn test_depth_tier_multiplier_deep() {
        // R3 (2026-09-23): the feed uses the one published scale, linear
        // between the printed points and held at 0.50 above 3 x D.
        assert!((depth_tier_multiplier(9.0, 6.0) - 0.875).abs() < 1e-9); // 1.5D
        assert!((depth_tier_multiplier(12.0, 6.0) - 0.75).abs() < 1e-9); // 2D
        assert!((depth_tier_multiplier(18.0, 6.0) - 0.50).abs() < 1e-9); // 3D
        assert!((depth_tier_multiplier(24.0, 6.0) - 0.50).abs() < 1e-9); // 4D
    }

    // ── derate_chipload_bounds (S.8 dedup) ─────────────────────────

    #[test]
    fn test_derate_chipload_bounds_require_both_scales_both_sides() {
        let band = derate_chipload_bounds(
            Some(0.05),
            Some(0.10),
            2.0, // doc_ratio -> scale 0.75
            ChiploadBoundPolicy::RequireBoth,
        )
        .unwrap();
        assert!((band.min_mm_per_tooth.unwrap() - 0.0375).abs() < 1e-9);
        assert!((band.max_mm_per_tooth - 0.075).abs() < 1e-9);
    }

    #[test]
    fn test_derate_chipload_bounds_require_both_rejects_missing_min() {
        assert!(
            derate_chipload_bounds(None, Some(0.10), 1.0, ChiploadBoundPolicy::RequireBoth)
                .is_none()
        );
    }

    #[test]
    fn test_derate_chipload_bounds_require_both_rejects_missing_max() {
        assert!(
            derate_chipload_bounds(Some(0.05), None, 1.0, ChiploadBoundPolicy::RequireBoth)
                .is_none()
        );
    }

    #[test]
    fn test_derate_chipload_bounds_allow_half_band_keeps_missing_min() {
        let band =
            derate_chipload_bounds(None, Some(0.10), 1.0, ChiploadBoundPolicy::AllowHalfBand)
                .unwrap();
        assert!(band.min_mm_per_tooth.is_none());
        assert!((band.max_mm_per_tooth - 0.10).abs() < 1e-9);
    }

    #[test]
    fn test_derate_chipload_bounds_allow_half_band_still_rejects_missing_max() {
        assert!(
            derate_chipload_bounds(Some(0.05), None, 1.0, ChiploadBoundPolicy::AllowHalfBand)
                .is_none()
        );
    }

    #[test]
    fn test_derate_chipload_bounds_present_but_invalid_min_rejects_under_both_policies() {
        // min > max: present-but-malformed rejects the whole row, it
        // does not silently degrade to a half-band.
        assert!(
            derate_chipload_bounds(
                Some(0.20),
                Some(0.10),
                1.0,
                ChiploadBoundPolicy::RequireBoth
            )
            .is_none()
        );
        assert!(
            derate_chipload_bounds(
                Some(0.20),
                Some(0.10),
                1.0,
                ChiploadBoundPolicy::AllowHalfBand
            )
            .is_none()
        );
    }

    #[test]
    fn test_derate_chipload_bounds_no_derate_at_or_below_1x_ratio() {
        let band = derate_chipload_bounds(
            Some(0.05),
            Some(0.10),
            0.4,
            ChiploadBoundPolicy::RequireBoth,
        )
        .unwrap();
        assert!((band.min_mm_per_tooth.unwrap() - 0.05).abs() < 1e-9);
        assert!((band.max_mm_per_tooth - 0.10).abs() < 1e-9);
    }

    #[test]
    fn test_derated_band_into_pair_round_trips() {
        let band = DeratedChiploadBand {
            min_mm_per_tooth: Some(0.05),
            max_mm_per_tooth: 0.10,
        };
        assert_eq!(band.into_pair(), Some((0.05, 0.10)));

        let half = DeratedChiploadBand {
            min_mm_per_tooth: None,
            max_mm_per_tooth: 0.10,
        };
        assert_eq!(half.into_pair(), None);
    }
}
