//! Geometric helpers for feeds calculation — effective diameter, chip thinning,
//! scallop stepover, V-bit width at depth.
//!
//! These are pure geometry functions with no material/machine dependencies.
//! Ported from reference/shapeoko_feeds_and_speeds/src/calcs.rs lines 523-720.

/// Feeds contract P11b (decision L1): the radial chip-thinning lift that
/// Suggest applies to the feed, so that the PEAK chip equals the band chip.
///
/// The law is Garr's (`CREDITS.md`, "Garr Tool, Chip Thinning"): the
/// published chip is the programmed feed per tooth, and it equals the
/// maximum chip thickness only at a radial engagement of 50 % of the
/// diameter or more. Below that, the peak chip is `h = fz sin(psi)` with
/// `cos psi = 1 - ae / r` ([`crate::feeds::force::immersion_angle`]). Thus
/// the feed that gives the band chip as the peak chip is `fz = h_band /
/// sin(psi)`, and this function returns `1 / sin(psi)`. At or above half the
/// diameter it returns 1 (Garr prints no change there).
///
/// One formula with no cap (decision L3): the machine feed ceiling, the
/// spindle power and the rubbing floor bound the lifted feed downstream.
/// Feeds contract P14 (review M2): this is the one law; `dressup::feedopt`
/// and the modulator band-mid read it through [`RadialThinning`] (the
/// clamped copy `radial_chip_thinning_factor` is deleted).
///
/// `d_eng_mm` is the engaged diameter at the cut depth (the K7
/// `Engagement::d_eng_mm`: `effective_diameter` or `2 x engagement_radius`).
/// A degenerate input (no width, no diameter) returns 1: no engagement, no
/// lift.
#[must_use]
pub fn peak_chip_thinning_lift(ae_mm: f64, d_eng_mm: f64) -> f64 {
    let psi = crate::feeds::force::immersion_angle(ae_mm, 0.5 * d_eng_mm);
    if psi <= 0.0 || psi >= std::f64::consts::FRAC_PI_2 {
        1.0
    } else {
        1.0 / psi.sin()
    }
}

/// Feeds contract P11b2: whether a cut thins its chip radially, the one rule
/// that Suggest, the post-sim chipload gate and the feed modulator read.
///
/// Decision L2: a finish pass (`PassRole::Finish`) holds the band chip as
/// its advance (Garr's finish row keeps a low chip on purpose). A drill cycle
/// has no radial cut. Every other cut thins, and its peak chip is the
/// advance over [`peak_chip_thinning_lift`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadialThinning {
    /// The cut thins: the peak chip is `fz / peak_chip_thinning_lift`.
    Applied,
    /// A finish pass (L2): the peak chip is taken as the advance.
    FinishPass,
    /// A drill cycle: no radial cut, no lift.
    DrillCycle,
}

impl RadialThinning {
    /// The rule of an operation's feeds style (family and pass role).
    #[must_use]
    pub fn of(family: crate::feeds::OperationFamily, role: crate::feeds::PassRole) -> Self {
        // EXEMPT(P28-18, F-ThinsChip)
        if family == crate::feeds::OperationFamily::Drill {
            Self::DrillCycle
        } else if role == crate::feeds::PassRole::Finish {
            Self::FinishPass
        } else {
            Self::Applied
        }
    }

    /// The rule of a gate context: the op's own pass role (not a routed
    /// row role). The gate refuses a drill cycle before it reads a chip, so
    /// this never returns [`Self::DrillCycle`].
    #[must_use]
    pub fn of_lut_role(role: crate::feeds::vendor_lut::LutPassRole) -> Self {
        // EXEMPT(P28-18, F-ThinsChip)
        match role {
            crate::feeds::vendor_lut::LutPassRole::Finish => Self::FinishPass,
            crate::feeds::vendor_lut::LutPassRole::Roughing
            | crate::feeds::vendor_lut::LutPassRole::SemiFinish => Self::Applied,
        }
    }

    /// The applied lift at the radial width `ae_mm` on the engaged
    /// diameter `d_eng_mm`: [`peak_chip_thinning_lift`] when the cut thins,
    /// else 1.
    #[must_use]
    pub fn lift(self, ae_mm: f64, d_eng_mm: f64) -> f64 {
        match self {
            Self::Applied => peak_chip_thinning_lift(ae_mm, d_eng_mm),
            Self::FinishPass | Self::DrillCycle => 1.0,
        }
    }
}

/// Feeds contract P18 (review D14): the feed before the radial
/// chip-thinning lift.
///
/// The Garr lift (`CREDITS.md`, "Garr Tool, Chip Thinning") is a correction
/// for a narrow radial width. An axial plunge, and a ramp or helix entry
/// into fresh stock, has no radial thinning: the tool cuts the full width.
/// Thus the plunge and the ramp feed read this feed, not the shipped
/// (lifted) feed.
///
/// The value is `feed / lift`, with the lift that [`RadialThinning::lift`]
/// gives at the shipped width. The shipped feed already carries every cap
/// after the lift (the power rungs, the machine cutting ceiling, the Q1
/// arms), so this value is at or below the shipped feed and at or below
/// `band chip x rpm x Z`. A lift that is not finite or below 1 counts as 1,
/// so the value is never above the feed.
///
/// The field is private. The doors are [`Self::at_cut`] (a cut: the
/// thinning rule, the width and the engaged diameter),
/// [`crate::feeds::FeedsResult::unlifted_feed`] (the calculator's lift) and
/// the apply funnel's door at the kept geometry. The plunge resolver
/// ([`crate::feeds::plunge::resolve_plunge_at`]) and the ramp resolver
/// ([`crate::feeds::ramp::resolve_ramp_feed`]) take this type, so a caller
/// cannot pass the lifted feed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnliftedFeed(f64);

impl UnliftedFeed {
    /// The feed `feed_mm_min` over the lift of a cut: the rule `thinning`
    /// at the radial width `ae_mm` on the engaged diameter `d_eng`.
    #[must_use]
    pub fn at_cut(
        feed_mm_min: f64,
        thinning: RadialThinning,
        ae_mm: f64,
        d_eng: crate::feeds::engagement::EngagedDiameter,
    ) -> Self {
        Self::over_lift(feed_mm_min, thinning.lift(ae_mm, d_eng.mm()))
    }

    /// Feeds contract P23d (operator ruling 2026-11-10): the feed of the
    /// slot-first ring of `operation`, or `None` for an op with no such
    /// ring or no feed. The first (outermost) ring of each level of a
    /// contour Pocket cuts a full slot in fresh stock, where the radial
    /// chip-thinning lift does not hold. Its feed is the op feed over the
    /// lift that Suggest sized the op feed with: the thinning rule of the
    /// op's feeds style at the lift width of its stepover
    /// (`PowerCases::full_feed_width`), on the engaged diameter at its
    /// depth per pass. The value is at or below the op feed; it equals the
    /// op feed where the lift is 1.
    ///
    /// The one door: the Pocket generator (`compute::execute::clearing_2d`)
    /// feeds the ring at it, and Suggest prints it
    /// (`profile::SuggestedSpeeds::slot_ring_feed_mm_min`).
    ///
    /// Not covered (P23d open items): the first line of a Zigzag raster,
    /// the opt-in slot lines of a 2D Adaptive, and the first ring round an
    /// island.
    #[must_use]
    pub fn of_slot_ring(
        operation: &crate::compute::catalog::OperationConfig,
        geometry: crate::feeds::ToolGeometryHint,
        tool_diameter_mm: f64,
        max_cut_diameter_mm: f64,
    ) -> Option<Self> {
        // P29-3: the op declares the slot ring (`OperationConfig::cuts`:
        // a contour Pocket).
        let cuts = operation.cuts();
        let slot_first = cuts.declares(crate::limits::CutKind::SlotRing);
        if !slot_first {
            return None;
        }
        let usable = |v: f64| v.is_finite() && v > 0.0;
        let feed = operation.feed_rate().filter(|v| usable(*v))?;
        let Some(ae) = operation.stepover().filter(|v| usable(*v)) else {
            return Some(Self(feed));
        };
        let ap = operation
            .depth_per_pass()
            .filter(|v| usable(*v))
            .unwrap_or(tool_diameter_mm);
        let width = crate::feeds::engagement::PowerCases::of_cuts(&cuts)
            .full_feed_width(ae, tool_diameter_mm);
        Some(Self::at_cut(
            feed,
            cuts.body_facts().thins_chip,
            width,
            crate::feeds::engagement::EngagedDiameter::of_hint(
                geometry,
                tool_diameter_mm,
                max_cut_diameter_mm,
                ap,
            ),
        ))
    }

    /// The feed `feed_mm_min` over the lift `lift`. A lift that is not
    /// finite or below 1 counts as 1.
    #[must_use]
    pub(in crate::feeds) fn over_lift(feed_mm_min: f64, lift: f64) -> Self {
        if lift.is_finite() && lift > 1.0 {
            Self(feed_mm_min / lift)
        } else {
            Self(feed_mm_min)
        }
    }

    /// The feed, mm/min.
    #[must_use]
    pub const fn mm_min(self) -> f64 {
        self.0
    }
}

// There is deliberately NO `ball_effective_diameter` and NO
// `bull_nose_effective_diameter` here. Feeds contract P15 deleted both. The
// bull-nose copy used the corner circle only (`ball_effective_diameter(2 r,
// ap)`) and dropped the flat centre `D - 2 r`: a 6 mm r1 tool at ap 0.2 got
// 1.2 mm against the true 5.2 mm, so the thinning lift overfed it. The ball
// copy floored ap 0 at `0.01 x D`. `feeds::effective_diameter` now calls the
// cutter's own width (`tool::ball_radius_at_height`,
// `tool::bull_nose_radius_at_height`).

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
/// Feeds contract P11b (L6) and P11b2: no reader scales a chip or a band by
/// this table any more (not the feed, not Suggest's band, not the post-sim
/// gate, not the modulator band). It stays as the one statement of the
/// printed table: [`vendor_depth_table_ceiling_ratio`] is its inverse, the
/// envelope limit `LimitKind::VendorDepthTable`, and the unit test holds the
/// inverse to it.
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

/// A validated chipload band (mm/tooth): the printed bounds of a vendor
/// LUT row that passed [`ChiploadBoundPolicy`]. `min_mm_per_tooth` is `None`
/// only when `AllowHalfBand` accepted a row with no published floor.
///
/// P11b2 (break): this was `DeratedChiploadBand`. No reader de-rates the
/// band by depth now (rulings L1 and L6), so the band is the printed band at
/// every depth.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ValidatedChiploadBand {
    pub min_mm_per_tooth: Option<f64>,
    pub max_mm_per_tooth: f64,
}

impl ValidatedChiploadBand {
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

/// Validate a vendor LUT row's published chipload bounds: the one home of
/// the rule (S.8) for Suggest (`feeds::calculate`, the band re-derivation),
/// the post-sim chipload gate (`tool_load::chipload`) and the chip target
/// of the modulator and the viewport (`tool_load::chip_target_for_toolpath`).
///
/// P11b2 (break): this was `derate_chipload_bounds(min, max, doc_ratio,
/// policy)`. The depth ratio is gone: every reader reads the printed band
/// at every depth (ruling L6); the depth table is an envelope limit.
///
/// Returns `None` when the raw bounds don't satisfy `policy`: the max
/// bound must always be present, finite, and positive; the min bound,
/// when present, must additionally be finite, positive, and `<= max`
/// (a present-but-invalid min rejects the row under both policies —
/// only an *absent* min is treated as "no floor published" and allowed
/// through under `AllowHalfBand`).
pub fn validate_chipload_bounds(
    min_mm_per_tooth: Option<f64>,
    max_mm_per_tooth: Option<f64>,
    policy: ChiploadBoundPolicy,
) -> Option<ValidatedChiploadBand> {
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
    Some(ValidatedChiploadBand {
        min_mm_per_tooth: min,
        max_mm_per_tooth: max,
    })
}

/// Feeds contract P11b (decision L6): the vendor depth table read as a
/// depth ceiling at a chip.
///
/// `chip_share` is the peak chip as a share of the printed band maximum.
/// The table (Onsrud, Freud, Amana; [`doc_derating_scale`]) allows the
/// chip `band max x doc_derating_scale(ap / D)` at a depth `ap`. This
/// function returns the deepest ratio `ap / D` where that allowed chip is
/// still at or above the chip: the inverse of [`doc_derating_scale`] on its
/// printed span.
///
/// - `chip_share <= 0.50`: 3.0, the last printed point
///   ([`DOC_DERATE_LAST_PRINTED_RATIO`]). No vendor prints a deeper row.
/// - `0.50 < chip_share <= 1.0`: the linear inverse between the printed
///   points (the interpolation is this crate's own, as in
///   [`doc_derating_scale`]).
/// - `chip_share > 1.0`: 1.0. The chip is above the band at every depth;
///   the chip band, not the depth table, names that.
///
/// The vendors print no width for the table ("width not printed"), so the
/// ceiling does not read the width except through the peak chip.
#[must_use]
pub fn vendor_depth_table_ceiling_ratio(chip_share: f64) -> f64 {
    if !chip_share.is_finite() || chip_share <= 0.5 {
        DOC_DERATE_LAST_PRINTED_RATIO
    } else if chip_share <= 0.75 {
        2.0 + (0.75 - chip_share) / 0.25
    } else if chip_share <= 1.0 {
        1.0 + (1.0 - chip_share) / 0.25
    } else {
        1.0
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
///   half of R2 stands: the deflection gate still uses the engaged cone
///   diameter at the depth (the depth ladder and the band de-rate are
///   retired, P11b and P11b2).
/// - V-bit: the nominal diameter at every depth (ruling B4, 2026-09-25,
///   G5). A V-bit chart prints its chip load against the tool's cutting
///   diameter (Onsrud 37-series, Whiteside) or against the included angle
///   only (Amana AMS-159, Spektra engraving, Onsrud 37-00 / 37-20). No
///   chart prints a chip load against the engaged width. An angle-keyed
///   row is not scaled at any key (`SizeBasis::AngleKey`).
/// - Flat, ball, bull: the nominal diameter, unchanged.
///
/// The key is the lookup key only. The depth ladder, the band de-rate and
/// the factor depth cap are retired (P11b, P11b2, P29-4); the depth limit
/// of a cut is the depth where the cut force meets the machine force
/// budget. The surface-speed RPM, the formula chip load and the deflection
/// of a V-bit stay at the engaged width (`feeds::calculate` step 1). `ap`
/// and `shank_d` stay in the signature for the depth-dependent families.
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
    // `ap` and `shank_d` do not move any key since B4; they stay for the
    // callers, which pass the depth and the shank of every family.
    let _ = (ap, shank_d);
    match geom {
        crate::feeds::ToolGeometryHint::TaperedBall { .. }
        | crate::feeds::ToolGeometryHint::VBit { .. }
        | crate::feeds::ToolGeometryHint::Flat
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
/// - V-bit: `cutter.diameter()`, the nominal diameter at every depth
///   (ruling B4, 2026-09-25). See [`lut_key_diameter_mm`].
/// - Flat, ball, bull: the nominal diameter.
///
/// For a tapered ball the depth half of R2 stands: the deflection gate
/// reads the engaged cone diameter (`lookup_diameter_at`), not this key.
/// The depth ladder, the band de-rate and the factor depth cap are retired
/// (P11b, P11b2, P29-4). For a V-bit only the surface-speed RPM, the
/// formula chip load and the deflection gate read the engaged width. `ap`
/// moves no key since B4; it stays in the signature for the callers.
#[must_use]
pub fn lut_key_diameter_for_cutter(cutter: &dyn crate::tool::MillingCutter, ap: f64) -> f64 {
    let _ = ap;
    match cutter.geometry_hint() {
        crate::feeds::ToolGeometryHint::TaperedBall { tip_radius, .. } => {
            if tip_radius.is_finite() && tip_radius > 0.0 {
                2.0 * tip_radius
            } else {
                cutter.diameter()
            }
        }
        crate::feeds::ToolGeometryHint::VBit { .. }
        | crate::feeds::ToolGeometryHint::Flat
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
    // F075: the one cone width (`tool::vbit_cone_radius_at_height`). This fn
    // takes no cutter, so it keeps its own guard and returns `None`.
    let width = tip_d + 2.0 * crate::tool::vbit_cone_radius_at_height(half_angle.tan(), ap);
    Some(width.max(tip_d))
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    // `test_ball_effective_*` and `test_bull_nose_transitions_to_nominal`
    // lived here until P15 deleted the two hint copies. The door is held by
    // `tests/one_d_eng_door_for_every_engagement_p15.rs`.

    // `test_tapered_ball_effective_scales_with_depth` lived here until C3.
    // It passed `nominal_d = 6.0` with `tip_r = 0.5` — a binding no call site
    // produces — so it exercised the one branch of the retired straight-cone
    // model that was not clamped flat, and read healthy while the reachable
    // behaviour was a constant. Its replacement is the DOC sweep in
    // `tests/tapered_width_model_parity_c3.rs`, which drives the tapered arm
    // through `feeds::calculate` at the production binding.

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
        // Ball nose 6mm, effective ~2.62mm at 0.3mm depth → factor ~2.29
        let d_eff =
            crate::feeds::effective_diameter(crate::feeds::ToolGeometryHint::Ball, 6.0, 6.0, 0.3);
        let factor = axial_chip_thinning_factor_for_ball(6.0, d_eff);
        assert!(factor > 1.5, "expected thinning factor > 1.5, got {factor}");
        assert!(factor < 4.0);
    }

    #[test]
    fn test_axial_thinning_clamped_to_4() {
        assert!((axial_chip_thinning_factor_for_ball(6.0, 0.5) - 4.0).abs() < 1e-9);
    }

    #[test]
    fn the_depth_table_ceiling_inverts_the_printed_scale_p11b() {
        // The printed points: the full band at 1 x D, 0.75 at 2 x D, 0.50
        // at 3 x D.
        for (share, ratio) in [(1.0, 1.0), (0.75, 2.0), (0.5, 3.0)] {
            assert!((vendor_depth_table_ceiling_ratio(share) - ratio).abs() < 1e-12);
            assert!((doc_derating_scale(ratio) - share).abs() < 1e-12);
        }
        // Between them the inverse meets the scale.
        for share in [0.55, 0.6, 0.8, 0.95] {
            let r = vendor_depth_table_ceiling_ratio(share);
            assert!((doc_derating_scale(r) - share).abs() < 1e-12, "{share}");
        }
        assert!((vendor_depth_table_ceiling_ratio(0.2) - 3.0).abs() < 1e-12);
        assert!((vendor_depth_table_ceiling_ratio(1.4) - 1.0).abs() < 1e-12);
    }

    // ── validate_chipload_bounds (S.8; P11b2: no depth ratio) ─────

    #[test]
    fn test_validate_chipload_bounds_require_both_keeps_the_printed_band() {
        let band =
            validate_chipload_bounds(Some(0.05), Some(0.10), ChiploadBoundPolicy::RequireBoth)
                .unwrap();
        assert!((band.min_mm_per_tooth.unwrap() - 0.05).abs() < 1e-12);
        assert!((band.max_mm_per_tooth - 0.10).abs() < 1e-12);
    }

    #[test]
    fn test_validate_chipload_bounds_require_both_rejects_missing_min() {
        assert!(
            validate_chipload_bounds(None, Some(0.10), ChiploadBoundPolicy::RequireBoth).is_none()
        );
    }

    #[test]
    fn test_validate_chipload_bounds_require_both_rejects_missing_max() {
        assert!(
            validate_chipload_bounds(Some(0.05), None, ChiploadBoundPolicy::RequireBoth).is_none()
        );
    }

    #[test]
    fn test_validate_chipload_bounds_allow_half_band_keeps_missing_min() {
        let band =
            validate_chipload_bounds(None, Some(0.10), ChiploadBoundPolicy::AllowHalfBand).unwrap();
        assert!(band.min_mm_per_tooth.is_none());
        assert!((band.max_mm_per_tooth - 0.10).abs() < 1e-9);
    }

    #[test]
    fn test_validate_chipload_bounds_allow_half_band_still_rejects_missing_max() {
        assert!(
            validate_chipload_bounds(Some(0.05), None, ChiploadBoundPolicy::AllowHalfBand)
                .is_none()
        );
    }

    #[test]
    fn test_validate_chipload_bounds_present_but_invalid_min_rejects_under_both_policies() {
        // min > max: present-but-malformed rejects the whole row, it
        // does not silently degrade to a half-band.
        assert!(
            validate_chipload_bounds(Some(0.20), Some(0.10), ChiploadBoundPolicy::RequireBoth)
                .is_none()
        );
        assert!(
            validate_chipload_bounds(Some(0.20), Some(0.10), ChiploadBoundPolicy::AllowHalfBand)
                .is_none()
        );
    }

    #[test]
    fn test_validated_band_into_pair_round_trips() {
        let band = ValidatedChiploadBand {
            min_mm_per_tooth: Some(0.05),
            max_mm_per_tooth: 0.10,
        };
        assert_eq!(band.into_pair(), Some((0.05, 0.10)));

        let half = ValidatedChiploadBand {
            min_mm_per_tooth: None,
            max_mm_per_tooth: 0.10,
        };
        assert_eq!(half.into_pair(), None);
    }
}
