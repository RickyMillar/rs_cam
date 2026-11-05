//! The feeds calculator, one function per step (feeds contract P13a,
//! architecture review H2).
//!
//! [`calculate_with`] is the sequence. Each step reads the values that the
//! steps before it wrote and writes its own values with their bases:
//!
//! - [`speed`]: Step 1 (the RPM), the vendor RPM of Step 2, Steps 2b, 2b'
//!   and 2c (the RPM ceilings).
//! - [`chip`]: Step 2 (the chip load, the vendor row and its band).
//! - [`cut`]: Steps 3 to 4c (the depth and the width, the flute guard, the
//!   engagement floor, the slot cap, the shank check).
//! - [`feed`]: Step 5 (the feed), Step 5b (the long-tool share), the feed
//!   basis after the power check, and Step 7 (the machine feed ceiling).
//! - [`power`]: Step 6 (the power check and its ladder).
//! - [`close`]: Steps 8 to 9c (the plunge, the ramp, the rubbing floor, the
//!   drill cycle) and the result.
//!
//! The values move between the steps in four records: [`Speed`], [`Chip`],
//! [`Cut`] and [`Feed`]. A step changes a value and its basis in the same
//! statement (P5, K2).

mod chip;
mod close;
mod cut;
mod feed;
mod power;
mod speed;

use super::sourced::Basis;
use super::{
    ChiploadBounds, ChiploadSource, FeedsInput, FeedsResult, FeedsSupport, FeedsWarning,
    SpindleScaleReason, geometry, vendor_lookup,
};

/// Feeds contract P11a (L4, D-A): what the calculator does with a depth
/// above the flute guard (Step 4) or the slot cap (Step 4b).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DepthClamps {
    /// Warn when the depth is above the guard or the cap
    /// ([`super::calculate`]). P11b: a stated depth ships unchanged, so the
    /// feed and the power are at that depth; only a calculator depth (the
    /// family default) is lowered to the guard or the cap.
    Apply,
    /// Keep the requested depth. The envelope grid draws the guard and the
    /// cap as limits, so every value of a grid point (feed, chip, power) is
    /// at the depth it names. No clamp warning: no depth moved.
    DrawnAsLimits,
}

/// The spindle speed and how the steps moved it.
pub(super) struct Speed {
    /// The RPM.
    rpm: f64,
    /// The basis of the RPM.
    basis: Basis,
    /// The product of the RPM moves after the chart RPM (the modal reads
    /// it as `FeedsDerates::spindle_scale`).
    spindle_scale: f64,
    /// The last step that moved `spindle_scale`.
    spindle_scale_reason: SpindleScaleReason,
}

/// The chip load of Step 2: the vendor row (or the formula) and its band.
pub(super) struct Chip {
    /// The target chip load, mm/tooth.
    chip_load: f64,
    /// The basis of the chip load.
    basis: Basis,
    /// The RPM that the vendor row prints.
    vendor_rpm: Option<f64>,
    /// The highest RPM that the vendor row prints.
    vendor_rpm_max: Option<f64>,
    /// The lowest RPM that the vendor row prints.
    vendor_rpm_min: Option<f64>,
    /// The observation id of the vendor row.
    vendor_source: Option<String>,
    /// Where the chip load comes from.
    chipload_source: ChiploadSource,
    /// The printed band of the recipe row (P11b: at every depth).
    band: Option<ChiploadBounds>,
    /// The printed point of the recipe row (A2).
    point: Option<f64>,
    /// The band of the envelope row that the rubbing floor falls back to
    /// (P1, 2026-08-22).
    floor_band_fallback: Option<ChiploadBounds>,
    /// The point of the envelope row that the rubbing floor falls back to.
    floor_point_fallback: Option<f64>,
    /// The matched recipe row.
    matched_lut_row: Option<vendor_lookup::LookupResult>,
    /// The support arm of the recipe row.
    support: FeedsSupport,
    /// The formula chip load `k0 x De^p x (1/H)^q`.
    formula_chipload: f64,
    /// The material feed scale `H` of the formula.
    feed_scale: f64,
}

/// The depth and the width of the cut.
pub(super) struct Cut {
    /// The axial depth, mm.
    ap: f64,
    /// The radial width, mm.
    ae: f64,
    /// The basis of the depth.
    ap_basis: Basis,
    /// The basis of the width.
    ae_basis: Basis,
}

/// The feed and the factors that Steps 5 to 9c apply to it.
pub(super) struct Feed {
    /// The chip-thinning diameter at the shipped depth (Step 5 on; the
    /// published `FeedsResult::effective_diameter_mm`).
    effective_d: f64,
    /// The radial chip-thinning rule of the op (P11b2).
    thinning: geometry::RadialThinning,
    /// The radial chip-thinning lift at the shipped width.
    radial_lift: f64,
    /// The axial chip-thinning factor of a ball (observed, not applied).
    axial_thinning: f64,
    /// The feed before the Step 6 feed rung settles it (the power ladder
    /// reads and writes it).
    raw_feed: f64,
    /// The feed.
    feed: f64,
    /// The basis of the feed.
    basis: Basis,
    /// The long-tool share of Step 5b (reported, never applied).
    ld_factor: f64,
    /// True when the power check bound.
    power_limited: bool,
    /// The feed factor of the Step 6 feed rung.
    power_factor: f64,
    /// True when the Step 6 feed rung cut the feed.
    feed_power_cut: bool,
    /// The factor of the Step 7 feed cap.
    feed_clamp_factor: f64,
}

/// The state of the steps after Step 5: the input, the warnings and the
/// four records.
pub(super) struct CalcState<'i, 'a> {
    input: &'i FeedsInput<'a>,
    warnings: Vec<FeedsWarning>,
    speed: Speed,
    chip: Chip,
    cut: Cut,
    feed: Feed,
}

/// [`super::calculate`] with the depth-clamp policy `clamps`.
pub(crate) fn calculate_with(input: &FeedsInput, clamps: DepthClamps) -> FeedsResult {
    let mut warnings = Vec::new();
    let lut_d = speed::engaged_diameter(input);
    let mut speed = speed::step1_rpm(input, &lut_d);
    let chip = chip::step2_chip_load(input, lut_d.diameter, &mut warnings);
    speed::step2_vendor_rpm(input, &chip, &mut speed);
    speed::step2b_max_speed(input, &chip, &mut speed);
    speed::step2b_milling_tier(input, &mut speed);
    speed::step2c_drill_tier(input, &mut speed);
    let mut cut = cut::step3_engagement(input, &mut warnings);
    cut::step4_flute_guard(input, clamps, &mut cut, &mut warnings);
    cut::engagement_floor(input, &mut cut);
    cut::step4b_slot(input, clamps, &mut cut, &mut warnings);
    cut::step4c_shank(input, &mut warnings);
    let feed = feed::step5_feed(input, &speed, &chip, &cut);
    let mut s = CalcState {
        input,
        warnings,
        speed,
        chip,
        cut,
        feed,
    };
    feed::step5b_long_tool(&mut s);
    power::step6_power(&mut s);
    feed::feed_basis_after_power(&mut s);
    feed::step7_feed_ceiling(&mut s);
    let mut entry = close::step8_plunge(&s);
    close::step9b_rubbing_floor(&mut s);
    close::step9c_drill(&mut s, &mut entry);
    close::result(s, entry)
}
