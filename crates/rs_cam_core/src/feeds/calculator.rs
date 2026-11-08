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
//!   engagement floor, the shank check; P16 deleted the slot cap).
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
use crate::trace::lineage::{BasisRecord, StepQuantity, StepRecord};

/// Feeds contract P11a (L4, D-A): what the calculator does with a depth
/// above the flute guard (Step 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DepthClamps {
    /// Warn when the depth is above the guard ([`super::calculate`]).
    /// P11b: a stated depth ships unchanged, so the feed and the power are
    /// at that depth; only a calculator depth (the family default) is
    /// lowered to the guard.
    Apply,
    /// Keep the requested depth. The envelope grid draws the guard as a
    /// limit, so every value of a grid point (feed, chip, power) is
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

impl Chip {
    /// The rubbing floor (mm/tooth) and its bound: the recipe row's band or
    /// point, else the envelope row's (P1). The one floor of Step 9b (the
    /// warning) and of the power ladder's last rung (P16, review D8).
    fn rubbing_floor(&self) -> (f64, crate::feeds::RubbingFloorSource) {
        crate::feeds::rubbing_floor_at(
            self.band.or(self.floor_band_fallback),
            self.point.or(self.floor_point_fallback),
        )
    }
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
    /// published `FeedsResult::effective_diameter_mm`). P15: the one door.
    effective_d: crate::feeds::engagement::EngagedDiameter,
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

/// P13b (lead ruling 5): whether a calculation writes its step records.
/// The Apply path ([`super::calculate`]) and the envelope pick point
/// record; the envelope grid does not (one call per grid point).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StepRecording {
    /// Write [`FeedsResult::steps`].
    Record,
    /// Leave [`FeedsResult::steps`] empty.
    Skip,
}

/// P13b (K8, ruling Q7): the step records of one calculation. A step that
/// changes a value leaves one record per value it changes: the value
/// before, the value after and the basis after. A step that changes only
/// a basis, or only warns, leaves none.
struct StepLog {
    on: bool,
    records: Vec<StepRecord>,
    last: [Option<f64>; StepQuantity::ALL.len()],
}

impl StepLog {
    fn new(recording: StepRecording) -> Self {
        Self {
            on: recording == StepRecording::Record,
            records: Vec::new(),
            last: [None; StepQuantity::ALL.len()],
        }
    }

    /// Note `value` of `q` after `step`: a record when it changed.
    fn note(&mut self, step: &str, q: StepQuantity, value: f64, basis: &Basis) {
        if !self.on {
            return;
        }
        let Some(slot) = StepQuantity::ALL
            .iter()
            .position(|x| *x == q)
            .and_then(|i| self.last.get_mut(i))
        else {
            return;
        };
        let before = *slot;
        if before.map(f64::to_bits) == Some(value.to_bits()) {
            return;
        }
        *slot = Some(value);
        self.records.push(StepRecord::new(
            step,
            q,
            before,
            value,
            BasisRecord::of(basis),
        ));
    }

    fn speed(&mut self, step: &str, speed: &Speed) {
        self.note(step, StepQuantity::Rpm, speed.rpm, &speed.basis);
    }

    fn cut(&mut self, step: &str, cut: &Cut) {
        self.note(step, StepQuantity::Depth, cut.ap, &cut.ap_basis);
        self.note(step, StepQuantity::Width, cut.ae, &cut.ae_basis);
    }

    /// Every value of the state after Step 5.
    fn state(&mut self, step: &str, s: &CalcState) {
        self.speed(step, &s.speed);
        self.note(step, StepQuantity::Chip, s.chip.chip_load, &s.chip.basis);
        self.cut(step, &s.cut);
        self.note(step, StepQuantity::Feed, s.feed.feed, &s.feed.basis);
    }
}

/// [`super::calculate`] with the depth-clamp policy `clamps`.
///
/// P13b: each step call is followed by a note of the values it can move,
/// so [`FeedsResult::steps`] holds one record per step that changed a value
/// (sentry `every_calculator_step_leaves_one_record_p13b`).
pub(crate) fn calculate_with(
    input: &FeedsInput,
    clamps: DepthClamps,
    recording: StepRecording,
) -> FeedsResult {
    let mut log = StepLog::new(recording);
    let mut warnings = Vec::new();
    let lut_d = speed::engaged_diameter(input);
    let mut speed = speed::step1_rpm(input, &lut_d);
    log.speed("step1_rpm", &speed);
    let chip = chip::step2_chip_load(input, lut_d.diameter, &mut warnings);
    log.note(
        "step2_chip_load",
        StepQuantity::Chip,
        chip.chip_load,
        &chip.basis,
    );
    speed::step2_vendor_rpm(input, &chip, &mut speed);
    log.speed("step2_vendor_rpm", &speed);
    speed::step2b_max_speed(input, &chip, &mut speed);
    log.speed("step2b_max_speed", &speed);
    speed::step2b_milling_tier(input, &mut speed);
    log.speed("step2b_milling_tier", &speed);
    let mut cut = cut::step3_engagement(input, &mut warnings);
    log.cut("step3_engagement", &cut);
    cut::step4_flute_guard(input, clamps, &mut cut, &mut warnings);
    log.cut("step4_flute_guard", &cut);
    cut::engagement_floor(input, &mut cut);
    log.cut("engagement_floor", &cut);
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
    log.state("step5_feed", &s);
    feed::step5b_long_tool(&mut s);
    log.state("step5b_long_tool", &s);
    power::step6_power(&mut s);
    // The feed basis after the power check changes a basis only; the note
    // after it gives the Step 6 records their final basis.
    feed::feed_basis_after_power(&mut s);
    log.state("step6_power", &s);
    feed::step7_feed_ceiling(&mut s);
    log.state("step7_feed_ceiling", &s);
    let mut entry = close::step8_plunge(&s);
    log.note(
        "step8_plunge",
        StepQuantity::Plunge,
        entry.plunge_rate,
        &entry.step8_basis(&s),
    );
    close::step9b_rubbing_floor(&mut s);
    close::step9c_drill(&mut s, &mut entry);
    log.state("step9c_drill", &s);
    let plunge_9c = entry.plunge_rate;
    let mut result = close::result(s, entry);
    log.note(
        "step9c_drill",
        StepQuantity::Plunge,
        plunge_9c,
        &result.bases.plunge,
    );
    // P20d: no record when the power is not modelled.
    if let Some(kw) = result.power_kw {
        log.note("result_power", StepQuantity::Power, kw, &result.bases.power);
    }
    result.steps = log.records;
    result
}
