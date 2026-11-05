//! CONTRACT K2b (feeds contract P5, ruling Q4): Suggest returns a space of
//! safe choices, not one number.
//!
//! [`SafeEnvelope::for_cut`] is the one builder. It reads the op's
//! [`CutSpec`](crate::compute::catalog::CutSpec) (K1) for the axes and the
//! operator's point, and runs the closed-form calculator ([`calculate`]) on a
//! sample grid. No generation and no simulation run here (ruling Q5).
//!
//! The envelope keeps the limits that the calculator and the invariant
//! passes compute and then discard today: each is a [`Limit`] with its law
//! and its [`Basis`]. It gives one proposal (`min(stated, binding cap)` on a
//! stated axis; never a raise, K2) and the placement of the operator's
//! point.
//!
//! A write of a geometry value needs an [`EnvelopePick`], and only
//! [`SafeEnvelope::pick`] makes one. A point that the envelope did not
//! compute cannot reach the apply funnel. The refusals of the compiler are
//! `compile_fail` doctests on [`envelope_refusals`].

use serde::Serialize;
use serde::ser::SerializeMap;

use crate::compute::catalog::{Axial, OperationConfig, Radial};
use crate::compute::tool_config::ToolConfig;
use crate::feeds::sourced::{
    Basis, ChipMm, FeedMmMin, MachineField, PlungeMmMin, Rpm, Sourced, lower_to_cap,
};
use crate::feeds::suggest::{SuggestContext, feeds_input_for_operation};
use crate::feeds::{
    FeedsError, FeedsResult, FeedsWarning, MIN_AE_MM, MIN_AP_MM, OperationFamily, PassRole,
    SpindleStrategy, ToolGeometryHint, VendorLut, calculate,
};
use crate::machine::MachineProfile;
use crate::material::Material;

/// The number of samples per free axis of the speeds grid.
///
/// A performance choice, not physics (lead decision D5): it needs no
/// printed source. A plane has `ENVELOPE_GRID_N` x `ENVELOPE_GRID_N`
/// points, a bar `ENVELOPE_GRID_N`. Each point is one closed-form
/// `calculate` call; the `Power` edge adds at most
/// `EDGE_BISECTION_STEPS` calls per width sample that it crosses.
/// Measured 2026-11-03 on the operator's machine (debug build, the unit
/// test `the_envelope_meets_its_time_budget`): one Pocket plane envelope,
/// 64 points, 4 ms (22 to 34 ms after P13c, see [`ENVELOPE_TIME_BUDGET_MS`]).
/// The FM1 walk took 0.88 s before P13c and 4.9 s after.
pub const ENVELOPE_GRID_N: usize = 8;

/// The time budget (ms, debug build) of one plane envelope, which the unit
/// test `the_envelope_meets_its_time_budget` holds: about ten times the
/// time measured alone, for a loaded machine. A performance number, not
/// physics (D5). P13c raised it from 50 ms (ten times 4 ms): the force-budget
/// edge solves the depth at each width sample (a scan and a bisection of
/// the engaged-edge sum) and the removal-rate proposal solves it again
/// around its pick; one Pocket plane envelope measured 22 to 34 ms alone
/// and 120 ms inside the parallel `--lib` run (2026-11-05, operator's
/// machine). The FM1 walk went from under 1 s to 4.9 s.
pub const ENVELOPE_TIME_BUDGET_MS: u128 = 300;

/// The bisection steps that place a `Power` edge between two grid samples.
/// Twelve halvings put the edge within 1/4096 of one grid step: a
/// solver limit with a stated reason, not a formula constant.
const EDGE_BISECTION_STEPS: usize = 12;

/// The bisection steps of the `Runtime` floor (the move count is cheap).
const RUNTIME_BISECTION_STEPS: usize = 32;

/// One point of the (depth, width) plane.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct CutPoint {
    /// The axial depth per pass (mm).
    pub ap_mm: f64,
    /// The radial width (stepover, mm).
    pub ae_mm: f64,
}

/// One axis of the envelope: the span that the grid covers.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct AxisSpan {
    /// The low end (mm).
    pub min_mm: f64,
    /// The high end (mm).
    pub max_mm: f64,
}

/// Which of ap and ae the op can vary (from the `CutSpec`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EnvelopeAxes {
    /// Both free: a stepover (or a scallop width) and a depth per pass.
    Plane {
        /// The depth span.
        ap: AxisSpan,
        /// The width span.
        ae: AxisSpan,
    },
    /// The depth is free; the width is fixed by the cut (a slot, a groove,
    /// a drill, a surface-following width).
    AxialBar {
        /// The depth span.
        ap: AxisSpan,
        /// The fixed width (mm).
        ae_mm: f64,
    },
    /// The width is free; the surface sets the depth.
    RadialBar {
        /// The width span.
        ae: AxisSpan,
        /// The fixed depth (mm).
        ap_mm: f64,
    },
    /// Neither is free: the surface sets both.
    Fixed {
        /// The depth (mm).
        ap_mm: f64,
        /// The width (mm).
        ae_mm: f64,
    },
}

impl EnvelopeAxes {
    const fn ap_free(&self) -> bool {
        matches!(self, Self::Plane { .. } | Self::AxialBar { .. })
    }

    const fn ae_free(&self) -> bool {
        matches!(self, Self::Plane { .. } | Self::RadialBar { .. })
    }
}

/// The kind of a limit. One row per row of the P5 card table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitKind {
    /// The flute guard: an `ap` ceiling.
    FluteGuard,
    /// The tool cutting length: an `ap` ceiling.
    CuttingLength,
    /// The vendor row's axial bound: an `ap` ceiling.
    VendorAxial,
    /// Feeds contract P11b (L6): the vendor depth table (Onsrud, Freud,
    /// Amana: 1 x D full chip, 2 x D less 25 %, 3 x D less 50 %) read at the
    /// operator's peak chip: an `ap` ceiling per width. The vendors print no
    /// width for it. A point past it is outside the envelope (a warning,
    /// Q6); the table no longer scales the chip.
    VendorDepthTable,
    /// The machine rigidity cap (roughing). P13c (O1): on the Pocket and
    /// Profile families an `ap(ae)` curve, the depth where the peak
    /// engaged-edge force meets the machine force budget; on the Adaptive
    /// family an `ap` ceiling (`adaptive_doc_factor x D`).
    Rigidity,
    /// P13c (O3, O4): the corner-coverage rule of a free-width pocket, an
    /// `ae` ceiling at `(1 + 1/sqrt 2) r`: a wider stepover leaves uncut
    /// material in a sharp offset corner.
    CornerCoverage,
    /// The slot rule: an `ap` ceiling where `ae` is above the threshold.
    Slot,
    /// The rated spindle power: an `ap(ae)` curve.
    Power,
    /// The tip deflection target (roughing): an `ap(ae)` curve.
    Deflection,
    /// The chipload band: judged per point.
    ChipBand,
    /// The rubbing floor: judged per point (it warns, never lifts).
    RubbingFloor,
    /// The machine cutting-feed cap: judged per point.
    FeedCap,
    /// The tool diameter: an `ae` ceiling.
    Diameter,
    /// The move budget: an `ae` floor (needs the model box).
    Runtime,
    /// The smallest engagement the calculator writes: the plot origin.
    EngagementFloor,
}

impl LimitKind {
    /// The name that the card, the warning and MCP print.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::FluteGuard => "flute guard",
            Self::CuttingLength => "cutting length",
            Self::VendorAxial => "vendor axial bound",
            Self::VendorDepthTable => "vendor depth ceiling (width not printed)",
            Self::Rigidity => "machine rigidity",
            Self::CornerCoverage => "corner coverage",
            Self::Slot => "slot rule",
            Self::Power => "spindle power",
            Self::Deflection => "tool deflection",
            Self::ChipBand => "chipload band",
            Self::RubbingFloor => "rubbing floor",
            Self::FeedCap => "machine feed cap",
            Self::Diameter => "tool diameter",
            Self::Runtime => "move budget",
            Self::EngagementFloor => "engagement floor",
        }
    }
}

/// The shape of one limit on the (ap, ae) plane.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Edge {
    /// `ap <= mm` for every `ae`.
    ApCeiling {
        /// The ceiling (mm).
        mm: f64,
    },
    /// `ap <= mm` where `ae > ae_mm` (the slot step).
    ApCeilingAbove {
        /// The width above which the ceiling holds (mm).
        ae_mm: f64,
        /// The ceiling (mm).
        mm: f64,
    },
    /// `ap <= f(ae)`: one `(ae, ap)` pair per sample, linear between them.
    ApCurve {
        /// The samples, ascending in `ae`.
        points: Vec<[f64; 2]>,
    },
    /// `ap >= mm`.
    ApFloor {
        /// The floor (mm).
        mm: f64,
    },
    /// `ae <= mm`.
    AeCeiling {
        /// The ceiling (mm).
        mm: f64,
    },
    /// `ae >= mm`.
    AeFloor {
        /// The floor (mm).
        mm: f64,
    },
    /// Judged per point of the [`SpeedsMap`]; no edge on the plane.
    PerPoint,
    /// The limit is not modelled for this cut, and `reason` says why (T-4:
    /// an abstention is stated, never dropped and never read as "no
    /// limit"). No edge on the plane.
    Unmodelled {
        /// The clause of the abstention.
        reason: &'static str,
    },
}

/// One limit of the envelope: its kind, its law and its basis.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Limit {
    /// The kind.
    pub kind: LimitKind,
    /// The law, in a few words.
    pub law: &'static str,
    /// How the limit is known.
    pub basis: Basis,
    /// The shape on the plane.
    pub edge: Edge,
}

impl Limit {
    /// The `ap` ceiling of this limit at the width `ae_mm`, if it has one.
    #[must_use]
    pub fn ap_ceiling_at(&self, ae_mm: f64) -> Option<f64> {
        match &self.edge {
            Edge::ApCeiling { mm } => Some(*mm),
            Edge::ApCeilingAbove { ae_mm: above, mm } => (ae_mm > *above).then_some(*mm),
            Edge::ApCurve { points } => interpolate_curve(points, ae_mm),
            Edge::ApFloor { .. }
            | Edge::AeCeiling { .. }
            | Edge::AeFloor { .. }
            | Edge::PerPoint
            | Edge::Unmodelled { .. } => None,
        }
    }

    /// The signed margin of `p` against this limit, as a share of the
    /// limit: positive inside, negative outside. `None` for a limit that
    /// has no edge at `p`.
    #[must_use]
    pub fn margin(&self, p: CutPoint) -> Option<f64> {
        let ceiling = |value: f64, cap: f64| (cap > 0.0).then(|| (cap - value) / cap);
        let floor = |value: f64, low: f64| (low > 0.0).then(|| (value - low) / low);
        match &self.edge {
            Edge::ApCeiling { .. } | Edge::ApCeilingAbove { .. } | Edge::ApCurve { .. } => self
                .ap_ceiling_at(p.ae_mm)
                .and_then(|cap| ceiling(p.ap_mm, cap)),
            Edge::ApFloor { mm } => floor(p.ap_mm, *mm),
            Edge::AeCeiling { mm } => ceiling(p.ae_mm, *mm),
            Edge::AeFloor { mm } => floor(p.ae_mm, *mm),
            Edge::PerPoint | Edge::Unmodelled { .. } => None,
        }
    }
}

/// `ap` at `ae` on a curve of `(ae, ap)` samples: linear between samples,
/// the end value outside them.
fn interpolate_curve(points: &[[f64; 2]], ae_mm: f64) -> Option<f64> {
    let first = points.first()?;
    let last = points.last()?;
    if ae_mm <= first[0] {
        return Some(first[1]);
    }
    if ae_mm >= last[0] {
        return Some(last[1]);
    }
    points.windows(2).find_map(|w| match w {
        [a, b] if ae_mm >= a[0] && ae_mm <= b[0] => {
            let span = b[0] - a[0];
            let t = if span > 0.0 {
                (ae_mm - a[0]) / span
            } else {
                0.0
            };
            Some(a[1] + t * (b[1] - a[1]))
        }
        _ => None,
    })
}

/// K7: the facts of the tool that place a measured point
/// ([`SafeEnvelope::placement_of_measured`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasuredTool {
    /// The engaged diameter at the sample depth (mm),
    /// `Engagement::d_eng_mm`.
    pub d_eng_mm: f64,
    /// A cone tool (tapered ball, V-bit): the slot rule does not judge it.
    pub cone: bool,
}

/// K7: the value of a point that one limit judges, and the limit there.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct JudgedValue {
    /// `"depth"` or `"width"`.
    pub axis: &'static str,
    /// The value of the point on that axis (mm).
    pub measured_mm: f64,
    /// The limit at the point (mm).
    pub limit_mm: f64,
}

/// Where a point sits against the boundary.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Placement {
    /// Inside every edge. `margin` is the smallest share of a limit left.
    Inside {
        /// The smallest margin, a share of the limit.
        margin: f64,
    },
    /// Outside an edge. `binding` is the limit crossed by the largest share;
    /// `margin` is that (negative) share.
    Outside {
        /// The limit that the point crosses.
        binding: LimitKind,
        /// The negative share of the limit.
        margin: f64,
    },
}

/// One point of the speeds grid. Its values come from one closed-form
/// `calculate` call at `(ap, ae)`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpeedsPoint {
    /// The depth (mm).
    pub ap_mm: f64,
    /// The width (mm).
    pub ae_mm: f64,
    /// The spindle speed.
    #[serde(serialize_with = "value_only")]
    pub rpm: Sourced<Rpm>,
    /// The side feed.
    #[serde(serialize_with = "value_only")]
    pub feed: Sourced<FeedMmMin>,
    /// The plunge feed.
    #[serde(serialize_with = "value_only")]
    pub plunge: Sourced<PlungeMmMin>,
    /// The advance per tooth.
    #[serde(serialize_with = "value_only")]
    pub chip: Sourced<ChipMm>,
    /// The spindle power at the point (kW).
    pub power_kw: f64,
    /// The rated power at the point's RPM (kW).
    pub available_kw: f64,
    /// The full chip fits the rated power at the point.
    pub fits_power: bool,
    /// The commanded chip is under the rubbing floor (a warning).
    pub rubbing: bool,
    /// The machine feed cap holds the feed.
    pub feed_capped: bool,
}

fn value_only<T: Serialize, S: serde::Serializer>(
    v: &Sourced<T>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    v.value().serialize(serializer)
}

impl SpeedsPoint {
    fn from_result(ap_mm: f64, ae_mm: f64, r: &FeedsResult, flutes: u32) -> Self {
        let feed_capped = r
            .warnings
            .iter()
            .any(|w| matches!(w, FeedsWarning::FeedRateClamped { .. }));
        let rubbing = r
            .warnings
            .iter()
            .any(|w| matches!(w, FeedsWarning::ChiploadBelowRubbingFloor { .. }));
        Self {
            ap_mm,
            ae_mm,
            rpm: Sourced::new(Rpm(r.rpm), r.bases.rpm.clone()),
            feed: Sourced::new(FeedMmMin(r.feed_rate_mm_min), r.bases.feed.clone()),
            plunge: Sourced::new(PlungeMmMin(r.plunge_rate_mm_min), r.bases.plunge.clone()),
            chip: Sourced::new(ChipMm(chip_of(r, flutes)), r.bases.chip.clone()),
            power_kw: r.power_kw,
            available_kw: r.available_power_kw,
            fits_power: fits_power(r),
            rubbing,
            feed_capped,
        }
    }
}

/// The commanded advance per tooth of a result: feed / (RPM x Z).
fn chip_of(r: &FeedsResult, flutes: u32) -> f64 {
    let z = f64::from(flutes);
    if r.rpm > 0.0 && z > 0.0 {
        r.feed_rate_mm_min / (r.rpm * z)
    } else {
        0.0
    }
}

/// The full chip fits the rated spindle power at the point: no feed cut by
/// the power ladder, and the shipped power at or under the curve.
fn fits_power(r: &FeedsResult) -> bool {
    let fits_curve =
        r.available_power_kw <= 0.0 || r.power_kw <= r.available_power_kw * (1.0 + 1e-9);
    fits_curve && r.derates.power_limit >= 1.0
}

/// A cheap read of the speeds at any `(ap, ae)` (P5b: the GUI drags a
/// candidate ring and reads this every frame).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct SpeedsSample {
    /// The spindle speed (rev/min).
    pub rpm: f64,
    /// The side feed (mm/min).
    pub feed_mm_min: f64,
    /// The plunge feed (mm/min).
    pub plunge_mm_min: f64,
    /// The spindle power (kW).
    pub power_kw: f64,
}

/// `(ap, ae)` -> the speeds. A grid of [`SpeedsPoint`], row-major in `ae`
/// then `ap`, with a bilinear lookup.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpeedsMap {
    /// The `ap` samples (mm), ascending.
    pub aps: Vec<f64>,
    /// The `ae` samples (mm), ascending.
    pub aes: Vec<f64>,
    /// `points[j * aps.len() + i]` is the point at `(aps[i], aes[j])`.
    pub points: Vec<SpeedsPoint>,
}

impl SpeedsMap {
    /// The point at grid index `(i, j)`.
    #[must_use]
    pub fn at(&self, i: usize, j: usize) -> Option<&SpeedsPoint> {
        self.points.get(j * self.aps.len() + i)
    }

    /// The speeds at `(ap, ae)` by bilinear interpolation on the grid (the
    /// end values outside it). No `calculate` call: a lookup.
    #[must_use]
    pub fn speeds_at(&self, ap_mm: f64, ae_mm: f64) -> Option<SpeedsSample> {
        let (i0, i1, ti) = bracket(&self.aps, ap_mm)?;
        let (j0, j1, tj) = bracket(&self.aes, ae_mm)?;
        let p00 = self.at(i0, j0)?;
        let p10 = self.at(i1, j0)?;
        let p01 = self.at(i0, j1)?;
        let p11 = self.at(i1, j1)?;
        let lerp = |f: &dyn Fn(&SpeedsPoint) -> f64| {
            let low = f(p00) + ti * (f(p10) - f(p00));
            let high = f(p01) + ti * (f(p11) - f(p01));
            low + tj * (high - low)
        };
        Some(SpeedsSample {
            rpm: lerp(&|p| p.rpm.get().0),
            feed_mm_min: lerp(&|p| p.feed.get().0),
            plunge_mm_min: lerp(&|p| p.plunge.get().0),
            power_kw: lerp(&|p| p.power_kw),
        })
    }
}

/// The two indices around `x` and the share between them.
fn bracket(xs: &[f64], x: f64) -> Option<(usize, usize, f64)> {
    let last = xs.len().checked_sub(1)?;
    let first = *xs.first()?;
    if xs.len() == 1 || x <= first {
        return Some((0, 0, 0.0));
    }
    if x >= *xs.last()? {
        return Some((last, last, 0.0));
    }
    let i = xs.iter().position(|v| *v >= x)?.max(1);
    let (a, b) = (*xs.get(i - 1)?, *xs.get(i)?);
    let t = if b > a { (x - a) / (b - a) } else { 0.0 };
    Some((i - 1, i, t))
}

/// The inputs of [`SafeEnvelope::for_cut`]. The context lends the model box
/// (the `Runtime` floor); the envelope keeps no borrow of it.
#[derive(Clone, Copy)]
pub struct EnvelopeInput<'a> {
    /// The op, with the operator's values.
    pub operation: &'a OperationConfig,
    /// The tool.
    pub tool: &'a ToolConfig,
    /// The machine.
    pub machine: &'a MachineProfile,
    /// The stock material.
    pub material: &'a Material,
    /// The vendor table. The envelope keeps it for a pick off the grid.
    pub lut: &'static VendorLut,
    /// The spindle policy of the post.
    pub spindle_strategy: SpindleStrategy,
    /// The Suggest context (the model box).
    pub context: SuggestContext<'a>,
}

/// The owned inputs that a pick off the grid needs.
#[derive(Clone)]
struct OwnedInputs {
    operation: OperationConfig,
    tool: ToolConfig,
    machine: MachineProfile,
    material: Material,
    lut: &'static VendorLut,
    spindle_strategy: SpindleStrategy,
}

impl std::fmt::Debug for OwnedInputs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedInputs")
            .field("operation", &self.operation.op_type())
            .field("tool", &self.tool.id)
            .finish_non_exhaustive()
    }
}

impl OwnedInputs {
    /// One closed-form `calculate` call at `p`. Both axes are given, so the
    /// calculator reads no family default for them.
    ///
    /// Feeds contract P11a (L4, D-A): the calculator keeps the requested
    /// depth (`DepthClamps::DrawnAsLimits`). The flute guard and the slot
    /// cap are limits of the envelope, so the feed, the chip and the power
    /// of the point are at the depth it names. A pick reads the same call,
    /// so a pick outside the guard or the cap ships the speeds of the
    /// depth it writes.
    fn calc_at(&self, p: CutPoint) -> FeedsResult {
        let mut input = feeds_input_for_operation(
            &self.operation,
            &self.tool,
            &self.material,
            &self.machine,
            self.lut,
            self.spindle_strategy,
        );
        input.axial_depth_mm = Some(p.ap_mm);
        input.radial_width_mm = Some(p.ae_mm);
        input.target_scallop_mm = None;
        crate::feeds::calculate_with(&input, crate::feeds::DepthClamps::DrawnAsLimits)
    }

    /// P11b (L7): the operator's stated feed and RPM fit the rated power at
    /// `p` (`power_at_operating_point`, the K7 worst case of the op). A
    /// material with no force line has no power figure and fits.
    fn operator_fits_power(&self, p: CutPoint, chip: OperatorChip) -> bool {
        let mut op = self.operation.clone();
        let _ = op.set_depth_per_pass(p.ap_mm);
        let _ = op.set_stepover(p.ae_mm);
        let point = crate::feeds::suggest::CalculatorOperatingPoint {
            radial_width_mm: p.ae_mm,
            axial_depth_mm: p.ap_mm,
            feed_rate_mm_min: chip.feed_mm_min,
            rpm: chip.rpm,
        };
        crate::feeds::power_at_operating_point(
            &op,
            &self.tool,
            &self.material,
            &self.machine,
            Some(point),
        )
        .map_or(true, |f| {
            f.available_kw <= 0.0 || f.required_kw <= f.available_kw * (1.0 + 1e-9)
        })
    }
}

/// CONTRACT K2b. Built only by [`SafeEnvelope::for_cut`]; no public
/// constructor and no `Default`.
#[derive(Debug, Clone)]
pub struct SafeEnvelope {
    /// Which of ap and ae the op can vary.
    pub axes: EnvelopeAxes,
    /// Each limit with its law and its basis.
    pub boundary: Vec<Limit>,
    /// The speeds at each grid point.
    pub speeds: SpeedsMap,
    /// The one proposed point, with the reason it was chosen.
    pub proposal: Sourced<CutPoint>,
    /// The limit that lowered the proposal below the operator's point, if
    /// one did. P13c (O4): on a removal-rate pick, the limit that sets its
    /// depth.
    pub proposal_binding: Option<LimitKind>,
    /// P13c (O4): every limit the proposal sits on (the depth limit, the
    /// width bound, a slot step that stops the width). Empty when the
    /// proposal sits on none.
    pub proposal_limits: Vec<LimitKind>,
    /// P13c: the notes every surface prints with the proposal: the
    /// assumptions of a computed width (`force_budget::COMPUTED_WIDTH_ASSUMPTIONS`)
    /// and the warning when no depth meets the force budget.
    pub notes: Vec<String>,
    /// The operator's stated point, when every free axis is stated.
    pub stated: Option<CutPoint>,
    /// Where the operator's stated point sits.
    pub operator: Option<Placement>,
    inputs: OwnedInputs,
    /// P13c (O1): the force-budget model of the `Rigidity` limit, when the
    /// op's rule is the force budget. The proposal solves it exactly.
    budget: Option<BudgetModel>,
    /// P13c (O4): the chip-sized limits (the vendor depth table, power,
    /// deflection) at the suggested chip, when the boundary is drawn at the
    /// operator's chip. A pick writes the suggested speeds, so the
    /// removal-rate pick stays inside both sets (else it would sit outside
    /// its own envelope once its speeds are written).
    pick_limits: Vec<Limit>,
    /// The operator's advance per tooth, when the op states feed and RPM.
    operator_fz: Option<f64>,
    /// The band maximum the vendor depth table reads (P11b L6), when the
    /// matched row prints one.
    table_band_max: Option<f64>,
}

/// P13c (O1): what the `Rigidity` force-budget edge reads.
#[derive(Debug, Clone, PartialEq)]
struct BudgetModel {
    /// The machine force budget (N).
    budget_n: f64,
    /// The operator's advance per tooth, when the op states feed and RPM.
    operator_fz: Option<f64>,
    /// The depths the limit solved, by width.
    solved: SolvedColumns,
}

/// The chip a force-budget depth is solved at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BudgetChip {
    /// The drawn edge: the operator's chip, else the suggested chip (P11b,
    /// L7).
    Curve,
    /// The removal-rate pick: the larger of the operator's chip and the
    /// suggested chip at the width, because a pick writes the suggested
    /// speeds (P13c).
    Pick,
}

/// The force-budget depths the `Rigidity` limit solved, by width: the
/// proposal reads a width it holds instead of solving it again.
#[derive(Debug, Clone, Default, PartialEq)]
struct SolvedColumns(Vec<(f64, crate::feeds::force_budget::BudgetDepth)>);

impl SolvedColumns {
    fn get(&self, ae: f64) -> Option<crate::feeds::force_budget::BudgetDepth> {
        self.0
            .iter()
            .find(|(w, _)| w.to_bits() == ae.to_bits())
            .map(|(_, d)| *d)
    }
}

impl Serialize for SafeEnvelope {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("axes", &self.axes)?;
        map.serialize_entry("boundary", &self.boundary)?;
        map.serialize_entry("speeds", &self.speeds)?;
        map.serialize_entry("proposal", &self.proposal)?;
        map.serialize_entry("proposal_binding", &self.proposal_binding)?;
        map.serialize_entry("proposal_limits", &self.proposal_limits)?;
        map.serialize_entry("notes", &self.notes)?;
        map.serialize_entry("stated", &self.stated)?;
        map.serialize_entry("operator", &self.operator)?;
        map.end()
    }
}

/// What a pick asks for. Every request gives a point; none is refused (Q6):
/// a point outside the boundary gives `Placement::Outside` and the K6
/// warning `project.outside_envelope`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PickRequest {
    /// The envelope's proposal.
    Proposal,
    /// A point of the plane. A fixed axis keeps its fixed value.
    Point {
        /// The depth (mm).
        ap_mm: f64,
        /// The width (mm).
        ae_mm: f64,
    },
    /// A drill peck depth (mm): the one free axis of a drill.
    Peck(f64),
}

/// A point that the envelope computed. Built only by
/// [`SafeEnvelope::pick`]; the speeds come from the envelope at the point.
#[derive(Debug, Clone)]
pub struct EnvelopePick<'env> {
    envelope: &'env SafeEnvelope,
    request: PickRequest,
    point: Sourced<CutPoint>,
    placement: Placement,
    result: FeedsResult,
}

impl<'env> EnvelopePick<'env> {
    /// The envelope that made the pick.
    #[must_use]
    pub const fn envelope(&self) -> &'env SafeEnvelope {
        self.envelope
    }

    /// The request.
    #[must_use]
    pub const fn request(&self) -> PickRequest {
        self.request
    }

    /// The picked point and its basis.
    #[must_use]
    pub const fn point(&self) -> &Sourced<CutPoint> {
        &self.point
    }

    /// Where the point sits.
    #[must_use]
    pub const fn placement(&self) -> Placement {
        self.placement
    }

    /// The calculator result at the point (one closed-form call).
    #[must_use]
    pub const fn result(&self) -> &FeedsResult {
        &self.result
    }

    /// The axes of the envelope (which geometry fields a pick writes).
    #[must_use]
    pub const fn axes(&self) -> EnvelopeAxes {
        self.envelope.axes
    }
}

/// The operator's values of a `CutSpec`, and the axes they give.
struct AxesPlan {
    axes: EnvelopeAxes,
    stated_ap: Option<f64>,
    stated_ae: Option<f64>,
    /// The free axis is a drill peck.
    peck: bool,
    /// P13c (O3, O4): the width is free and computed: a Pocket-family
    /// roughing op with a stepover. The corner-coverage rule bounds it and
    /// the proposal is the removal-rate pick.
    computed_width: bool,
}

impl SafeEnvelope {
    /// Build the envelope of one op, tool, machine and material.
    ///
    /// # Errors
    ///
    /// [`FeedsError::ToolOutOfDomain`] for a tool that builds no cutter,
    /// and the refusal of [`crate::feeds::validate_tool_for_operation`]
    /// (D6: a missing input is not a finding).
    pub fn for_cut(input: EnvelopeInput<'_>) -> Result<Self, FeedsError> {
        input.tool.validate().map_err(FeedsError::ToolOutOfDomain)?;
        let feeds_input = feeds_input_for_operation(
            input.operation,
            input.tool,
            input.material,
            input.machine,
            input.lut,
            input.spindle_strategy,
        );
        crate::feeds::validate_tool_for_operation(&feeds_input)?;
        let inputs = OwnedInputs {
            operation: input.operation.clone(),
            tool: input.tool.clone(),
            machine: input.machine.clone(),
            material: input.material.clone(),
            lut: input.lut,
            spindle_strategy: input.spindle_strategy,
        };
        // The calculator at the op's own inputs: the family default of an
        // unstated axis, and the matched row.
        let base = calculate(&feeds_input);
        let d = input.tool.diameter;
        let plan = axes_plan(input.operation, input.tool, &base);
        let (family, role) = input.operation.feeds_style();

        let ap_samples = match plan.axes {
            EnvelopeAxes::Plane { ap, .. } | EnvelopeAxes::AxialBar { ap, .. } => samples(ap),
            EnvelopeAxes::RadialBar { ap_mm, .. } | EnvelopeAxes::Fixed { ap_mm, .. } => {
                vec![ap_mm]
            }
        };
        let ae_samples = match plan.axes {
            EnvelopeAxes::Plane { ae, .. } | EnvelopeAxes::RadialBar { ae, .. } => samples(ae),
            EnvelopeAxes::AxialBar { ae_mm, .. } | EnvelopeAxes::Fixed { ae_mm, .. } => {
                vec![ae_mm]
            }
        };
        let mut points = Vec::with_capacity(ap_samples.len() * ae_samples.len());
        for &ae in &ae_samples {
            for &ap in &ap_samples {
                let r = inputs.calc_at(CutPoint {
                    ap_mm: ap,
                    ae_mm: ae,
                });
                points.push(SpeedsPoint::from_result(
                    ap,
                    ae,
                    &r,
                    input.tool.flute_count_planning(),
                ));
            }
        }
        let speeds = SpeedsMap {
            aps: ap_samples,
            aes: ae_samples,
            points,
        };

        // P11b (L7): the force limits are sized at the operator's chip (the
        // stated feed / (rpm x Z)), so a higher chip moves them in. With no
        // stated feed or RPM (S11) they read the suggested chip of the grid.
        let operator = operator_chip(input.operation, input.tool);
        let mut boundary = Vec::new();
        geometric_limits(&mut boundary, &input, &base, &plan, d);
        vendor_depth_limit(&mut boundary, &input, &base, &speeds, &plan, operator);
        power_limit(
            &mut boundary,
            &inputs,
            &speeds,
            &plan,
            base.power_case,
            operator,
        );
        deflection_limit(&mut boundary, &input, &speeds, &plan, role, operator);
        let budget = rigidity_limit(
            &mut boundary,
            &inputs,
            &speeds,
            &plan,
            (family, role),
            operator,
        );
        runtime_limit(&mut boundary, &input, &plan, d);
        per_point_limits(&mut boundary, &base, &speeds);
        let mut pick_limits = Vec::new();
        if operator.is_some() {
            vendor_depth_limit(&mut pick_limits, &input, &base, &speeds, &plan, None);
            power_limit(
                &mut pick_limits,
                &inputs,
                &speeds,
                &plan,
                base.power_case,
                None,
            );
            deflection_limit(&mut pick_limits, &input, &speeds, &plan, role, None);
        }

        let mut env = Self {
            axes: plan.axes,
            boundary,
            speeds,
            proposal: Sourced::new(
                CutPoint {
                    ap_mm: base.axial_depth_mm,
                    ae_mm: base.radial_width_mm,
                },
                Basis::derived("not computed", Vec::new()),
            ),
            proposal_binding: None,
            proposal_limits: Vec::new(),
            notes: Vec::new(),
            stated: None,
            operator: None,
            inputs,
            budget,
            pick_limits,
            operator_fz: operator.map(|c| c.fz_mm),
            table_band_max: base
                .chipload_bounds
                .map(|b| b.max_mm_per_tooth)
                .or(base.chipload_point_mm)
                .filter(|v| v.is_finite() && *v > 0.0)
                .filter(|_| base.matched_lut_row.is_some() && plan.axes.ap_free() && !plan.peck),
        };
        let peck_rule = plan.peck.then(|| drill_peck_rule(&input)).flatten();
        let removal_rate = matches!(role, PassRole::Roughing)
            && !plan.peck
            && (plan.computed_width
                || (family != OperationFamily::Adaptive
                    && matches!(env.axes, EnvelopeAxes::AxialBar { .. })));
        if removal_rate {
            let pick = env.propose_removal_rate(&plan);
            env.proposal = pick.point;
            env.proposal_binding = pick.limits.first().copied();
            env.proposal_limits = pick.limits;
        } else {
            let (proposal, binding) = env.propose(&plan, &base, peck_rule);
            env.proposal = proposal;
            env.proposal_binding = binding;
            env.proposal_limits = binding.into_iter().collect();
        }
        env.add_exact_budget_samples(&[
            (Some(env.proposal.get().ae_mm), BudgetChip::Pick),
            (plan.stated_ae, BudgetChip::Curve),
        ]);
        if plan.computed_width {
            env.notes
                .push(crate::feeds::force_budget::computed_width_assumptions_line());
        }
        if env
            .limit(LimitKind::Rigidity)
            .is_some_and(|l| l.law == crate::feeds::force_budget::FORCE_BUDGET_FLOOR_LAW)
        {
            env.notes
                .push(crate::feeds::force_budget::FORCE_BUDGET_FLOOR_NOTE.to_owned());
        }
        env.stated = match (plan.axes, plan.stated_ap, plan.stated_ae) {
            (EnvelopeAxes::Plane { .. }, Some(ap), Some(ae)) => Some(CutPoint {
                ap_mm: ap,
                ae_mm: ae,
            }),
            (EnvelopeAxes::AxialBar { ae_mm, .. }, Some(ap), _) => {
                Some(CutPoint { ap_mm: ap, ae_mm })
            }
            (EnvelopeAxes::RadialBar { ap_mm, .. }, _, Some(ae)) => {
                Some(CutPoint { ap_mm, ae_mm: ae })
            }
            (EnvelopeAxes::Fixed { ap_mm, ae_mm }, _, _) => Some(CutPoint { ap_mm, ae_mm }),
            (
                EnvelopeAxes::Plane { .. }
                | EnvelopeAxes::AxialBar { .. }
                | EnvelopeAxes::RadialBar { .. },
                _,
                _,
            ) => None,
        };
        env.operator = env.stated.map(|p| env.placement_of(p));
        Ok(env)
    }

    /// The lowest `ap` ceiling at the width `ae_mm`, and its limit.
    #[must_use]
    pub fn ap_ceiling_at(&self, ae_mm: f64) -> Option<(f64, LimitKind)> {
        self.boundary
            .iter()
            .filter_map(|l| l.ap_ceiling_at(ae_mm).map(|c| (c, l.kind)))
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }

    /// The limit of `kind`, if the envelope has one.
    #[must_use]
    pub fn limit(&self, kind: LimitKind) -> Option<&Limit> {
        self.boundary.iter().find(|l| l.kind == kind)
    }

    /// K7 (feeds contract P9): the value of the measured point `p` that the
    /// limit of `kind` judges, and the limit there, by the rules of
    /// [`Self::placement_of_measured`]. `None` for a limit with no edge
    /// (judged per point, or unmodelled), a limit that does not judge this
    /// tool, or no limit of that kind.
    #[must_use]
    pub fn judged_value(
        &self,
        kind: LimitKind,
        p: CutPoint,
        tool: MeasuredTool,
    ) -> Option<JudgedValue> {
        let l = self.limit(kind)?;
        let edge = Self::measured_edge(l, tool)?;
        let judged = Limit { edge, ..l.clone() };
        let (axis, measured_mm, limit_mm) = match judged.edge {
            Edge::ApCeiling { .. } | Edge::ApCeilingAbove { .. } | Edge::ApCurve { .. } => {
                ("depth", p.ap_mm, judged.ap_ceiling_at(p.ae_mm)?)
            }
            Edge::ApFloor { mm } => ("depth", p.ap_mm, mm),
            Edge::AeCeiling { mm } | Edge::AeFloor { mm } => ("width", p.ae_mm, mm),
            Edge::PerPoint | Edge::Unmodelled { .. } => return None,
        };
        Some(JudgedValue {
            axis,
            measured_mm,
            limit_mm,
        })
    }

    /// Where the point `p` sits against the boundary. A query for any
    /// `(ap, ae)`; no `calculate` call.
    #[must_use]
    pub fn placement_of(&self, p: CutPoint) -> Placement {
        self.place(p, None)
    }

    /// K7 (feeds contract P9): where a MEASURED point sits against the
    /// ceilings of the boundary.
    ///
    /// - A floor (the move budget, the engagement floor) is a rule for the
    ///   operator's stated point; a simulation sample with a lighter cut than
    ///   the plan crosses no limit, so the floors do not place it.
    /// - The `Diameter` ceiling is the engaged diameter at the sample depth
    ///   (`tool.d_eng_mm`, from `Engagement::d_eng`), not the nominal
    ///   diameter: a tapered ball or a V-bit cuts wider than its tip as the
    ///   cone engages (lead decision, 2026-11-05). For a flat tool the two
    ///   are equal.
    /// - A cone tool is not judged by the `Slot` rule: the rule caps the
    ///   depth of a flat end mill that cuts a full-width slot, and a V
    ///   groove (or a tapered ball's cone) is always a full groove.
    #[must_use]
    pub fn placement_of_measured(&self, p: CutPoint, tool: MeasuredTool) -> Placement {
        self.place(p, Some(tool))
    }

    /// The ceiling of `l` that a measured point of `tool` meets: the edge
    /// itself, the `Diameter` ceiling at the engaged diameter, or `None`
    /// for a limit that does not judge this tool.
    fn measured_edge(l: &Limit, tool: MeasuredTool) -> Option<Edge> {
        match (l.kind, &l.edge) {
            (LimitKind::Slot, _) if tool.cone => None,
            (LimitKind::Diameter, Edge::AeCeiling { .. }) => {
                Some(Edge::AeCeiling { mm: tool.d_eng_mm })
            }
            (_, edge) => Some(edge.clone()),
        }
    }

    fn place(&self, p: CutPoint, measured: Option<MeasuredTool>) -> Placement {
        let mut inside = f64::INFINITY;
        let mut worst: Option<(f64, LimitKind)> = None;
        for l in &self.boundary {
            let judged;
            let l = match measured {
                None => l,
                Some(tool) => match Self::measured_edge(l, tool) {
                    None => continue,
                    Some(edge) => {
                        judged = Limit { edge, ..l.clone() };
                        &judged
                    }
                },
            };
            let is_floor = match l.edge {
                Edge::ApFloor { .. } | Edge::AeFloor { .. } => true,
                Edge::ApCeiling { .. }
                | Edge::ApCeilingAbove { .. }
                | Edge::ApCurve { .. }
                | Edge::AeCeiling { .. }
                | Edge::PerPoint
                | Edge::Unmodelled { .. } => false,
            };
            if measured.is_some() && is_floor {
                continue;
            }
            // A fixed axis is not the operator's choice: its edges do not
            // place the point.
            let on_fixed_axis = match l.edge {
                Edge::ApCeiling { .. }
                | Edge::ApCeilingAbove { .. }
                | Edge::ApCurve { .. }
                | Edge::ApFloor { .. } => !self.axes.ap_free(),
                Edge::AeCeiling { .. } | Edge::AeFloor { .. } => !self.axes.ae_free(),
                Edge::PerPoint | Edge::Unmodelled { .. } => true,
            };
            if on_fixed_axis {
                continue;
            }
            let Some(m) = l.margin(p) else { continue };
            if m < -1e-9 {
                if worst.is_none_or(|(w, _)| m < w) {
                    worst = Some((m, l.kind));
                }
            } else {
                inside = inside.min(m);
            }
        }
        match worst {
            Some((margin, binding)) => Placement::Outside { binding, margin },
            None => Placement::Inside {
                margin: if inside.is_finite() { inside } else { 1.0 },
            },
        }
    }

    /// Pick a point. No request is refused (Q6). A point off the grid gets
    /// one more closed-form `calculate` call, so its speeds still come from
    /// the envelope.
    #[must_use]
    pub fn pick(&self, request: PickRequest) -> EnvelopePick<'_> {
        let fixed = match self.axes {
            EnvelopeAxes::Plane { .. } => None,
            EnvelopeAxes::AxialBar { ae_mm, .. } => Some((None, Some(ae_mm))),
            EnvelopeAxes::RadialBar { ap_mm, .. } => Some((Some(ap_mm), None)),
            EnvelopeAxes::Fixed { ap_mm, ae_mm } => Some((Some(ap_mm), Some(ae_mm))),
        };
        let fix = |p: CutPoint| match fixed {
            None => p,
            Some((ap, ae)) => CutPoint {
                ap_mm: ap.unwrap_or(p.ap_mm),
                ae_mm: ae.unwrap_or(p.ae_mm),
            },
        };
        let point = match request {
            PickRequest::Proposal => self.proposal.clone(),
            PickRequest::Point { ap_mm, ae_mm } => {
                Sourced::new(fix(CutPoint { ap_mm, ae_mm }), Basis::picked())
            }
            PickRequest::Peck(mm) => Sourced::new(
                fix(CutPoint {
                    ap_mm: mm,
                    ae_mm: self.proposal.get().ae_mm,
                }),
                Basis::picked(),
            ),
        };
        let p = point.get();
        EnvelopePick {
            envelope: self,
            request,
            placement: self.placement_of(p),
            result: self.inputs.calc_at(p),
            point,
        }
    }

    /// The proposal: `min(stated, binding cap)` on a stated axis (never a
    /// raise, K2), the calculator's point on an unstated axis.
    fn propose(
        &self,
        plan: &AxesPlan,
        base: &FeedsResult,
        peck_rule: Option<f64>,
    ) -> (Sourced<CutPoint>, Option<LimitKind>) {
        let ae_cap = self
            .boundary
            .iter()
            .filter_map(|l| match l.edge {
                Edge::AeCeiling { mm } => Some((mm, l)),
                Edge::ApCeiling { .. }
                | Edge::ApCeilingAbove { .. }
                | Edge::ApCurve { .. }
                | Edge::ApFloor { .. }
                | Edge::AeFloor { .. }
                | Edge::PerPoint
                | Edge::Unmodelled { .. } => None,
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let mut binding = None;
        let ae = match (self.axes, plan.stated_ae) {
            (EnvelopeAxes::Plane { .. } | EnvelopeAxes::RadialBar { .. }, Some(stated)) => {
                let s = Sourced::new(stated, Basis::stated());
                match ae_cap {
                    Some((cap, l)) if cap < stated => {
                        binding = Some(l.kind);
                        lower_to_cap(s, cap, l.law, &l.basis)
                    }
                    Some(_) | None => s,
                }
            }
            (EnvelopeAxes::Plane { .. } | EnvelopeAxes::RadialBar { .. }, None) => {
                Sourced::new(base.radial_width_mm, base.bases.ae.clone())
            }
            (EnvelopeAxes::AxialBar { ae_mm, .. } | EnvelopeAxes::Fixed { ae_mm, .. }, _) => {
                Sourced::new(ae_mm, Basis::derived("fixed by the cut", vec![]))
            }
        };
        let ae_mm = ae.get();
        let ap = match (self.axes, plan.stated_ap) {
            (EnvelopeAxes::Plane { .. } | EnvelopeAxes::AxialBar { .. }, stated) => {
                let s = match stated {
                    Some(v) => Sourced::new(v, Basis::stated()),
                    None => Sourced::new(base.axial_depth_mm, base.bases.ap.clone()),
                };
                // Card row 29: the peck proposal is the drill peck rule,
                // lowered from the stated peck, never raised (K2).
                let s = match peck_rule {
                    Some(rule) => lower_to_cap(
                        s,
                        rule,
                        "drill peck rule",
                        &Basis::derived(DRILL_PECK_LAW, vec![Basis::tool_diameter()]),
                    ),
                    None => s,
                };
                let cap = self
                    .boundary
                    .iter()
                    .filter_map(|l| l.ap_ceiling_at(ae_mm).map(|c| (c, l)))
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                match cap {
                    Some((c, l)) if c < s.get() => {
                        binding = Some(l.kind);
                        lower_to_cap(s, c, l.law, &l.basis)
                    }
                    Some(_) | None => s,
                }
            }
            (EnvelopeAxes::RadialBar { ap_mm, .. } | EnvelopeAxes::Fixed { ap_mm, .. }, _) => {
                Sourced::new(ap_mm, Basis::derived("fixed by the cut", vec![]))
            }
        };
        let point = CutPoint {
            ap_mm: ap.get(),
            ae_mm,
        };
        let basis = Basis::derived(
            "proposal: min(stated, binding cap)",
            vec![ap.basis().clone(), ae.basis().clone()],
        );
        (Sourced::new(point, basis), binding)
    }
}

/// One scored width of the removal-rate search: the removal rate, the
/// depth, and the limit that sets the depth.
type Scored = (f64, f64, Option<LimitKind>);

/// P13c (O4): a removal-rate pick and the limits it sits on.
struct RemovalRatePick {
    point: Sourced<CutPoint>,
    limits: Vec<LimitKind>,
}

/// The width samples of the removal-rate search (a numerical step, not a
/// formula constant): the width span is sampled this many times, the
/// limit breakpoints are added, and the best sample is refined.
const REMOVAL_RATE_WIDTH_SAMPLES: usize = 24;

/// The refinement steps around the best width sample (golden section):
/// six steps shrink the bracket to 0.618^6 = 6 % of two candidate steps.
const REMOVAL_RATE_REFINE_STEPS: usize = 6;

/// The best coarse candidates that the exact force-budget solve re-scores
/// (the coarse rank reads curves interpolated between width samples).
const REMOVAL_RATE_EXACT_CANDIDATES: usize = 3;

impl SafeEnvelope {
    /// P13c (O1): the force-budget depth at the width `ae_mm`, solved
    /// exactly (not interpolated between grid columns). `None` when the op
    /// has no budget edge or the model cannot answer.
    fn budget_depth_at(
        &self,
        ae_mm: f64,
        chip: BudgetChip,
    ) -> Option<crate::feeds::force_budget::BudgetDepth> {
        let model = self.budget.as_ref()?;
        // The solved columns are at the curve chip; a pick with no operator
        // chip reads the same chip.
        if (matches!(chip, BudgetChip::Curve) || model.operator_fz.is_none())
            && let Some(d) = model.solved.get(ae_mm)
        {
            return Some(d);
        }
        let tool = crate::compute::cutter::build_cutter(&self.inputs.tool).ok()?;
        let suggested = || {
            chip_of(
                &self.inputs.calc_at(CutPoint {
                    ap_mm: MIN_AP_MM,
                    ae_mm,
                }),
                self.inputs.tool.flute_count_planning(),
            )
        };
        let fz = match (chip, model.operator_fz) {
            (BudgetChip::Curve, Some(fz)) => fz,
            (BudgetChip::Curve, None) => suggested(),
            // A pick writes the suggested speeds at the point, so its depth
            // holds at the suggested chip and at the operator's chip (the
            // larger chip gives the shallower depth).
            (BudgetChip::Pick, Some(fz)) => fz.max(suggested()),
            (BudgetChip::Pick, None) => suggested(),
        };
        crate::feeds::force_budget::depth_at_budget(
            &tool,
            &self.inputs.material,
            ae_mm,
            fz,
            model.budget_n,
        )
    }

    /// The advance per tooth a pick at the width `ae_mm` is judged at: the
    /// larger of the operator's chip and the suggested chip there (a pick
    /// writes the suggested speeds).
    fn pick_fz(&self, ae_mm: f64) -> f64 {
        let suggested = chip_of(
            &self.inputs.calc_at(CutPoint {
                ap_mm: MIN_AP_MM,
                ae_mm,
            }),
            self.inputs.tool.flute_count_planning(),
        );
        self.operator_fz.map_or(suggested, |fz| fz.max(suggested))
    }

    /// The vendor depth table ceiling at `ae_mm` and the chip `fz`, in
    /// closed form (`vendor_depth_limit` at one width), or the span top
    /// where the table holds every printed depth.
    fn table_ceiling_at(&self, ae_mm: f64, fz: f64) -> Option<f64> {
        let band_max = self.table_band_max?;
        let d = self.inputs.tool.diameter;
        let top = self.speeds.aps.last().copied().unwrap_or(MIN_AP_MM);
        let psi = crate::feeds::force::immersion_angle(ae_mm, 0.5 * d);
        let share = fz * psi.min(std::f64::consts::FRAC_PI_2).sin() / band_max;
        Some(if share > 0.5 {
            crate::feeds::geometry::vendor_depth_table_ceiling_ratio(share) * d
        } else {
            top
        })
    }

    /// The tip-deflection ceiling at `ae_mm` and the chip `fz` (the one
    /// solver of `deflection_limit`), or the span top with no root.
    fn deflection_ceiling_at(&self, ae_mm: f64, fz: f64) -> Option<f64> {
        let tool = crate::compute::cutter::build_cutter(&self.inputs.tool).ok()?;
        let top = self.speeds.aps.last().copied().unwrap_or(MIN_AP_MM);
        let c = crate::feeds::cutter_constraints::cutter_axial_constraints(
            &tool,
            &self.inputs.material,
            ae_mm.max(1.0e-3),
            fz,
            None,
            None,
            Some(crate::feeds::suggest::DEFLECTION_BACKOFF_TARGET_UM),
        );
        Some(
            c.max_doc_deflection_mm
                .filter(|v| v.is_finite() && *v >= 0.0)
                .unwrap_or(top),
        )
    }

    /// The lowest `ap` ceiling at `ae_mm` and its limit, with the force
    /// budget, the vendor depth table and the deflection edge solved
    /// exactly at `ae_mm` and the pick chip (the other curves read between
    /// their width samples).
    fn exact_ap_ceiling_at(&self, ae_mm: f64) -> Option<(f64, LimitKind)> {
        let fz = std::cell::OnceCell::new();
        let pick_fz = || *fz.get_or_init(|| self.pick_fz(ae_mm));
        self.boundary
            .iter()
            .chain(self.pick_limits.iter())
            .filter_map(|l| {
                let exact = match (l.kind, &l.edge) {
                    (LimitKind::Rigidity, Edge::ApCurve { .. } | Edge::ApCeiling { .. })
                        if self.budget.is_some() =>
                    {
                        let top = self.speeds.aps.last().copied().unwrap_or(MIN_AP_MM);
                        match self.budget_depth_at(ae_mm, BudgetChip::Pick) {
                            Some(d) => Some(d.ceiling_mm().unwrap_or(top)),
                            None => l.ap_ceiling_at(ae_mm),
                        }
                    }
                    (
                        LimitKind::VendorDepthTable,
                        Edge::ApCurve { .. } | Edge::ApCeiling { .. },
                    ) => self.table_ceiling_at(ae_mm, pick_fz()),
                    (LimitKind::Deflection, Edge::ApCurve { .. }) => {
                        self.deflection_ceiling_at(ae_mm, pick_fz())
                    }
                    _ => l.ap_ceiling_at(ae_mm),
                };
                exact.map(|c| (c, l.kind))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }

    /// P13c (O1): add an exact force-budget sample to the `Rigidity` curve
    /// at each width given, so a point at that width is placed against the
    /// solved depth, not an interpolation. The proposal width takes the
    /// pick's own depth (the chip it ships, at or under the curve depth),
    /// so the proposal sits on the edge it names.
    fn add_exact_budget_samples(&mut self, widths: &[(Option<f64>, BudgetChip)]) {
        if self.budget.is_none() {
            return;
        }
        let top = self.speeds.aps.last().copied().unwrap_or(MIN_AP_MM);
        let mut samples = Vec::new();
        for (ae, chip) in widths.iter().filter_map(|(w, c)| w.map(|w| (w, *c))) {
            if !(ae.is_finite() && ae > 0.0) {
                continue;
            }
            if let Some(d) = self.budget_depth_at(ae, chip) {
                samples.push([ae, d.ceiling_mm().unwrap_or(top)]);
            }
        }
        let Some(limit) = self
            .boundary
            .iter_mut()
            .find(|l| l.kind == LimitKind::Rigidity)
        else {
            return;
        };
        if let Edge::ApCurve { points } = &mut limit.edge {
            for p in samples {
                match points.iter_mut().find(|q| (q[0] - p[0]).abs() < 1e-12) {
                    // The lower of the two depths at one width.
                    Some(q) => q[1] = q[1].min(p[1]),
                    None => points.push(p),
                }
            }
            points.sort_by(|a, b| a[0].total_cmp(&b[0]));
        }
    }

    /// The removal rate (mm3/min) of the point `(ap, ae)`: `ae x ap x vf`,
    /// `vf` the feed the calculator ships there (the chip-thinning lift,
    /// the feed cap and its RPM arm).
    fn removal_rate(&self, p: CutPoint) -> f64 {
        let vf = self.inputs.calc_at(p).feed_rate_mm_min;
        p.ae_mm * p.ap_mm * vf.max(0.0)
    }

    /// The deepest depth at `ae` inside every `ap` ceiling, the stated
    /// depth and the span, and the limit that sets it.
    fn deepest_at(&self, plan: &AxesPlan, ae: f64) -> (f64, Option<LimitKind>) {
        let top = match self.axes {
            EnvelopeAxes::Plane { ap, .. } | EnvelopeAxes::AxialBar { ap, .. } => ap.max_mm,
            EnvelopeAxes::RadialBar { ap_mm, .. } | EnvelopeAxes::Fixed { ap_mm, .. } => {
                return (ap_mm, None);
            }
        };
        let mut ap = plan.stated_ap.unwrap_or(top);
        let mut kind = None;
        if let Some((c, k)) = self.exact_ap_ceiling_at(ae)
            && c < ap
        {
            // A ceiling under the floor ships the floor (Q6, the
            // force-budget floor arm); a stated depth under the floor stays
            // (K2: never raised).
            ap = c.max(MIN_AP_MM.min(ap));
            kind = Some(k);
        }
        (ap, kind)
    }

    /// P13c (O4): the proposal of a roughing op is the cut with the most
    /// removal per minute inside every limit. The search box is bounded by
    /// the operator's stated values (K2: a proposal never raises a stated
    /// axis) and, on a free width, by the `ae` ceilings (the corner rule,
    /// the diameter) and floors (the move budget). A fixed width keeps its
    /// width and picks the depth.
    fn propose_removal_rate(&self, plan: &AxesPlan) -> RemovalRatePick {
        let fixed_ae = match self.axes {
            EnvelopeAxes::AxialBar { ae_mm, .. } | EnvelopeAxes::Fixed { ae_mm, .. } => Some(ae_mm),
            EnvelopeAxes::Plane { .. } | EnvelopeAxes::RadialBar { .. } => None,
        };
        let ae_ceiling = self
            .boundary
            .iter()
            .filter_map(|l| match l.edge {
                Edge::AeCeiling { mm } => Some((mm, l.kind)),
                Edge::ApCeiling { .. }
                | Edge::ApCeilingAbove { .. }
                | Edge::ApCurve { .. }
                | Edge::ApFloor { .. }
                | Edge::AeFloor { .. }
                | Edge::PerPoint
                | Edge::Unmodelled { .. } => None,
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let ae_floor = self
            .boundary
            .iter()
            .filter_map(|l| match l.edge {
                Edge::AeFloor { mm } => Some(mm),
                Edge::ApCeiling { .. }
                | Edge::ApCeilingAbove { .. }
                | Edge::ApCurve { .. }
                | Edge::ApFloor { .. }
                | Edge::AeCeiling { .. }
                | Edge::PerPoint
                | Edge::Unmodelled { .. } => None,
            })
            .fold(MIN_AE_MM, f64::max);
        let (lo, hi, width_bound) = match fixed_ae {
            Some(ae) => (ae, ae, None),
            None => {
                let mut hi = ae_ceiling.map_or(self.inputs.tool.diameter, |c| c.0);
                let mut bound = ae_ceiling.map(|c| c.1);
                if let Some(stated) = plan.stated_ae
                    && stated < hi
                {
                    hi = stated;
                    bound = None;
                }
                (ae_floor.min(hi), hi, bound)
            }
        };
        // The exact evaluation solves the force budget at the width; the
        // coarse one reads the stored curves (interpolated between the
        // width samples). The coarse scan ranks the candidates, the exact
        // one decides between the best and refines around it.
        let exact_cache: std::cell::RefCell<Vec<(f64, Scored)>> =
            std::cell::RefCell::new(Vec::new());
        let eval = |ae: f64| -> Scored {
            if let Some((_, r)) = exact_cache
                .borrow()
                .iter()
                .find(|(w, _)| w.to_bits() == ae.to_bits())
            {
                return *r;
            }
            let (ap, kind) = self.deepest_at(plan, ae);
            let r = (
                self.removal_rate(CutPoint {
                    ap_mm: ap,
                    ae_mm: ae,
                }),
                ap,
                kind,
            );
            exact_cache.borrow_mut().push((ae, r));
            r
        };
        let coarse = |ae: f64| -> f64 {
            let top = match self.axes {
                EnvelopeAxes::Plane { ap, .. } | EnvelopeAxes::AxialBar { ap, .. } => ap.max_mm,
                EnvelopeAxes::RadialBar { ap_mm, .. } | EnvelopeAxes::Fixed { ap_mm, .. } => ap_mm,
            };
            let mut ap = plan.stated_ap.unwrap_or(top);
            for l in self.boundary.iter().chain(self.pick_limits.iter()) {
                if let Some(c) = l.ap_ceiling_at(ae) {
                    ap = ap.min(c);
                }
            }
            // The coarse feed is the speeds grid, read between its samples
            // (no calculator call); the exact evaluation calls it.
            let ap = ap.max(MIN_AP_MM);
            let vf = self
                .speeds
                .speeds_at(ap.min(top), ae)
                .map_or(0.0, |sp| sp.feed_mm_min);
            ae * ap * vf.max(0.0)
        };
        let mut candidates: Vec<f64> = if hi > lo {
            let n = REMOVAL_RATE_WIDTH_SAMPLES.max(2);
            #[expect(
                clippy::cast_precision_loss,
                reason = "a sample index under 100, exact in an f64"
            )]
            let span = |i: usize| lo + (hi - lo) * i as f64 / (n - 1) as f64;
            (0..n).map(span).collect()
        } else {
            vec![hi]
        };
        // The breakpoints of the limits: a slot step (the rule holds above
        // its width, so its width is the widest point under the step), and
        // the grid columns of the curves.
        if hi > lo {
            for l in &self.boundary {
                if let Edge::ApCeilingAbove { ae_mm, .. } = l.edge
                    && ae_mm > lo
                    && ae_mm < hi
                {
                    candidates.push(ae_mm);
                }
            }
            candidates.extend(
                self.speeds
                    .aes
                    .iter()
                    .copied()
                    .filter(|&a| a > lo && a < hi),
            );
        }
        candidates.sort_by(f64::total_cmp);
        candidates.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
        let mut ranked: Vec<(usize, f64)> = candidates
            .iter()
            .enumerate()
            .map(|(i, &ae)| (i, coarse(ae)))
            .collect();
        ranked.sort_by(|x, y| y.1.total_cmp(&x.1).then(y.0.cmp(&x.0)));
        let mut best_i = ranked.first().map_or(0, |r| r.0);
        let mut best_mrr = f64::NEG_INFINITY;
        for &(i, _) in ranked.iter().take(REMOVAL_RATE_EXACT_CANDIDATES) {
            let ae = candidates.get(i).copied().unwrap_or(hi);
            let m = eval(ae).0;
            if m > best_mrr {
                best_mrr = m;
                best_i = i;
            }
        }
        let mut best_ae = candidates.get(best_i).copied().unwrap_or(hi);
        // Refine between the neighbours of the best candidate (golden
        // section; the removal rate is smooth between breakpoints).
        if candidates.len() > 2 {
            let mut a = candidates
                .get(best_i.saturating_sub(1))
                .copied()
                .unwrap_or(lo);
            let mut b = candidates.get(best_i + 1).copied().unwrap_or(hi);
            let g = (5.0_f64.sqrt() - 1.0) / 2.0;
            let mut c = b - g * (b - a);
            let mut e = a + g * (b - a);
            let mut fc = eval(c).0;
            let mut fe = eval(e).0;
            for _ in 0..REMOVAL_RATE_REFINE_STEPS {
                for (w, m) in [(c, fc), (e, fe)] {
                    if m > best_mrr {
                        best_mrr = m;
                        best_ae = w;
                    }
                }
                if fc >= fe {
                    b = e;
                    e = c;
                    fe = fc;
                    c = b - g * (b - a);
                    fc = eval(c).0;
                } else {
                    a = c;
                    c = e;
                    fc = fe;
                    e = a + g * (b - a);
                    fe = eval(e).0;
                }
            }
            for (w, m) in [(c, fc), (e, fe)] {
                if m > best_mrr {
                    best_mrr = m;
                    best_ae = w;
                }
            }
        }
        let (_, ap, depth_kind) = eval(best_ae);
        let mut limits: Vec<LimitKind> = depth_kind.into_iter().collect();
        if fixed_ae.is_none() {
            if let Some(kind) = width_bound
                && (best_ae - hi).abs() < 1e-9
            {
                limits.push(kind);
            }
            // A slot step that stops the width at its threshold.
            for l in &self.boundary {
                if let Edge::ApCeilingAbove { ae_mm, .. } = l.edge
                    && (best_ae - ae_mm).abs() < 1e-9
                    && !limits.contains(&l.kind)
                {
                    limits.push(l.kind);
                }
            }
        }
        let mut bases: Vec<Basis> = limits
            .iter()
            .filter_map(|k| self.limit(*k).map(|l| l.basis.clone()))
            .collect();
        if plan.stated_ap.is_some() || plan.stated_ae.is_some() {
            bases.push(Basis::stated());
        }
        let point = CutPoint {
            ap_mm: ap,
            ae_mm: best_ae,
        };
        RemovalRatePick {
            point: Sourced::new(
                point,
                Basis::derived("proposal: most removal per minute inside the limits", bases),
            ),
            limits,
        }
    }
}

/// The law of the drill peck rule: half the Janka-banded per-peck maximum
/// (`Material::drill_default_peck_depth_mm`), inside the schema range and
/// under the hole (`apply::peck_for_hole`). A repo rule (ruling B5).
const DRILL_PECK_LAW: &str = "half the per-peck maximum of the material (repo rule, ruling B5)";

/// The drill peck rule of a drill op: the peck that the Suggest drill
/// defaults give (card row 29). `None` for an op with no peck.
fn drill_peck_rule(input: &EnvelopeInput<'_>) -> Option<f64> {
    let d = input.tool.diameter;
    if !d.is_finite() || d <= 0.0 {
        return None;
    }
    let peck = input.material.drill_default_peck_depth_mm(d);
    match input.operation {
        OperationConfig::Drill(cfg) => Some(crate::feeds::suggest::peck_for_hole(
            crate::compute::catalog::OperationType::Drill,
            peck,
            cfg.floor.depth_below_top(),
        )),
        OperationConfig::AlignmentPinDrill(cfg) => Some(crate::feeds::suggest::peck_for_hole(
            crate::compute::catalog::OperationType::AlignmentPinDrill,
            peck,
            input
                .context
                .stock
                .and_then(|s| s.stock_z)
                .map(|z| z + cfg.spoilboard_penetration),
        )),
        OperationConfig::Face(_)
        | OperationConfig::Pocket(_)
        | OperationConfig::Profile(_)
        | OperationConfig::Adaptive(_)
        | OperationConfig::VCarve(_)
        | OperationConfig::Inlay(_)
        | OperationConfig::InlayPlug(_)
        | OperationConfig::Zigzag(_)
        | OperationConfig::Trace(_)
        | OperationConfig::Bore(_)
        | OperationConfig::Chamfer(_)
        | OperationConfig::DropCutter(_)
        | OperationConfig::Adaptive3d(_)
        | OperationConfig::Waterline(_)
        | OperationConfig::Pencil(_)
        | OperationConfig::Scallop(_)
        | OperationConfig::UnifiedFinish(_)
        | OperationConfig::SteepShallow(_)
        | OperationConfig::RampFinish(_)
        | OperationConfig::SpiralFinish(_)
        | OperationConfig::RadialFinish(_)
        | OperationConfig::HorizontalFinish(_)
        | OperationConfig::ProjectCurve(_) => None,
    }
}

/// `ENVELOPE_GRID_N` evenly spaced samples of a span.
fn samples(span: AxisSpan) -> Vec<f64> {
    let n = ENVELOPE_GRID_N.max(2);
    let step = (span.max_mm - span.min_mm) / (n - 1) as f64;
    (0..n).map(|i| span.min_mm + step * i as f64).collect()
}

/// The axes and the operator's values of an op (from its `CutSpec`).
fn axes_plan(operation: &OperationConfig, tool: &ToolConfig, base: &FeedsResult) -> AxesPlan {
    let spec = operation.cut_spec();
    let d = tool.diameter;
    let ball_r = match tool.geometry_hint() {
        ToolGeometryHint::Ball => d / 2.0,
        ToolGeometryHint::TaperedBall { tip_radius, .. } => tip_radius,
        ToolGeometryHint::Bull { corner_radius } => corner_radius,
        ToolGeometryHint::Flat | ToolGeometryHint::VBit { .. } => 0.0,
    };
    // `Some(Some(v))`: free and stated; `Some(None)`: free, not stated;
    // `None`: fixed at the value in the second slot.
    let (radial_free, stated_ae, fixed_ae) = match spec.radial {
        // P10: an unset stepover is free and not stated.
        Radial::Width(s) => (true, s.map(|s| s.mm()), base.radial_width_mm),
        Radial::Scallop(s) => {
            let w = crate::feeds::geometry::scallop_stepover(ball_r, s.mm());
            (true, w, w.unwrap_or(base.radial_width_mm))
        }
        Radial::Slot | Radial::Axial => (false, None, d),
        Radial::Groove | Radial::AngularStep(_) | Radial::SurfaceFollow => {
            (false, None, base.radial_width_mm)
        }
    };
    let (axial_free, stated_ap, fixed_ap) = match spec.axial {
        Axial::HelixPitch(s) | Axial::ChamferWidth(s) => (true, Some(s.mm()), s.mm()),
        // P10: an unset per-pass depth, Z step or peck is free and not
        // stated, as an unset one-pass depth (S11).
        Axial::PerPass(s)
        | Axial::ZStep(s)
        | Axial::Peck(s)
        | Axial::OnePass(s)
        | Axial::Surface { max: s } => match (spec.axial, s) {
            (_, Some(s)) => (true, Some(s.mm()), s.mm()),
            (Axial::PerPass(_) | Axial::ZStep(_) | Axial::Peck(_) | Axial::OnePass(_), None) => {
                (true, None, base.axial_depth_mm)
            }
            (Axial::Surface { .. } | Axial::HelixPitch(_) | Axial::ChamferWidth(_), None) => {
                (false, None, base.axial_depth_mm)
            }
        },
    };
    let flute = if tool.cutting_length_planning_mm() > 0.0 {
        tool.cutting_length_planning_mm()
    } else {
        crate::feeds::flute_guard_of(0.0, crate::compute::tool_config::FactKind::Stated, d).0
    };
    let ap_span = AxisSpan {
        min_mm: MIN_AP_MM,
        max_mm: flute.max(stated_ap.unwrap_or(0.0)).max(MIN_AP_MM * 2.0),
    };
    let ae_span = AxisSpan {
        min_mm: MIN_AE_MM,
        max_mm: d.max(stated_ae.unwrap_or(0.0)).max(MIN_AE_MM * 2.0),
    };
    let axes = match (axial_free, radial_free) {
        (true, true) => EnvelopeAxes::Plane {
            ap: ap_span,
            ae: ae_span,
        },
        (true, false) => EnvelopeAxes::AxialBar {
            ap: ap_span,
            ae_mm: fixed_ae,
        },
        (false, true) => EnvelopeAxes::RadialBar {
            ae: ae_span,
            ap_mm: fixed_ap,
        },
        (false, false) => EnvelopeAxes::Fixed {
            ap_mm: fixed_ap,
            ae_mm: fixed_ae,
        },
    };
    let (family, role) = operation.feeds_style();
    AxesPlan {
        axes,
        stated_ap,
        stated_ae,
        peck: matches!(spec.axial, Axial::Peck(_)),
        computed_width: radial_free
            && matches!(spec.radial, Radial::Width(_))
            && family == OperationFamily::Pocket
            && role == PassRole::Roughing,
    }
}

/// The closed-form edges: flute guard, cutting length, vendor axial,
/// rigidity, slot, diameter, engagement floor.
fn geometric_limits(
    out: &mut Vec<Limit>,
    input: &EnvelopeInput<'_>,
    base: &FeedsResult,
    plan: &AxesPlan,
    d: f64,
) {
    let length_kind = input.tool.cutting_length_fact().kind();
    let (guard, guard_basis) =
        crate::feeds::flute_guard_of(input.tool.cutting_length_planning_mm(), length_kind, d);
    out.push(Limit {
        kind: LimitKind::FluteGuard,
        law: "0.8 x flute",
        basis: guard_basis,
        edge: Edge::ApCeiling { mm: guard },
    });
    if input.tool.cutting_length_planning_mm() > 0.0 {
        out.push(Limit {
            kind: LimitKind::CuttingLength,
            law: "tool cutting length",
            basis: Basis::tool_fact(
                crate::feeds::sourced::ToolFactName::CuttingLength,
                length_kind,
            ),
            edge: Edge::ApCeiling {
                mm: input.tool.cutting_length_planning_mm(),
            },
        });
    }
    // Today's pass 0 scope: the ops that the registry routes to the axial
    // envelope (`Capabilities::axial_envelope`).
    if let Some(env) = crate::feeds::suggest::axial_envelope_for_operation(
        input.operation,
        input.tool,
        input.material,
        base.matched_lut_row.as_ref(),
    ) && let (Some(mm), Some(row)) = (env.max_doc_vendor_mm, base.matched_lut_row.as_ref())
    {
        out.push(Limit {
            kind: LimitKind::VendorAxial,
            law: "vendor ap bound",
            basis: Basis::Printed(crate::feeds::sourced::RowRef(row.observation_id.clone())),
            edge: Edge::ApCeiling { mm },
        });
    }
    // P13c (O3): the corner-coverage rule bounds a free pocket width.
    if plan.computed_width {
        out.push(Limit {
            kind: LimitKind::CornerCoverage,
            law: crate::feeds::force_budget::CORNER_COVERAGE_LAW,
            basis: Basis::derived(
                "corner coverage (1 + 1/sqrt 2) r",
                vec![
                    Basis::Rule(crate::feeds::force_budget::corner_coverage_row(
                        crate::feeds::force_budget::CORNER_COVERAGE_ROW_NAME,
                    )),
                    Basis::tool_diameter(),
                ],
            ),
            edge: Edge::AeCeiling {
                mm: d * crate::feeds::force_budget::corner_coverage_width_factor(),
            },
        });
    }
    // A drill peck is a plunge: it cuts no side slot, so the slot rule does
    // not bound it.
    if !plan.peck {
        let (threshold_d, cap_d, slot_basis) = crate::feeds::slot_rule(input.machine);
        out.push(Limit {
            kind: LimitKind::Slot,
            law: "slot: ap <= machine slot_doc_factor x D where ae > 0.85 D",
            basis: slot_basis,
            edge: Edge::ApCeilingAbove {
                ae_mm: d * threshold_d,
                mm: d * cap_d,
            },
        });
    }
    out.push(Limit {
        kind: LimitKind::Diameter,
        law: "ae <= D",
        basis: Basis::tool_diameter(),
        edge: Edge::AeCeiling { mm: d },
    });
    out.push(Limit {
        kind: LimitKind::EngagementFloor,
        law: "smallest depth the calculator writes",
        basis: Basis::Rule(crate::feeds::MIN_AP_MM_ROW),
        edge: Edge::ApFloor { mm: MIN_AP_MM },
    });
    out.push(Limit {
        kind: LimitKind::EngagementFloor,
        law: "smallest width the calculator writes",
        basis: Basis::Rule(crate::feeds::MIN_AE_MM_ROW),
        edge: Edge::AeFloor { mm: MIN_AE_MM },
    });
}

/// The operator's chip of an op (feeds contract P11b, L7): the stated feed
/// over the stated RPM and the flute count, with the RPM and the feed it
/// came from. `None` when the op states no feed or no RPM (S11).
#[derive(Debug, Clone, Copy, PartialEq)]
struct OperatorChip {
    /// The advance per tooth (mm).
    fz_mm: f64,
    /// The stated feed (mm/min).
    feed_mm_min: f64,
    /// The stated RPM.
    rpm: f64,
}

fn operator_chip(operation: &OperationConfig, tool: &ToolConfig) -> Option<OperatorChip> {
    let feed = operation.feed_rate()?;
    let rpm = f64::from(operation.spindle_rpm()?);
    let z = f64::from(tool.flute_count_planning().max(1));
    (feed.is_finite() && feed > 0.0 && rpm > 0.0).then(|| OperatorChip {
        fz_mm: feed / (rpm * z),
        feed_mm_min: feed,
        rpm,
    })
}

/// The rated-power edge: per width sample, the deepest `ap` at which the
/// chip fits the spindle curve. Between two grid samples the edge is placed
/// by bisection.
/// K7 change 3: the power edge is drawn at the worst case of the op's
/// pre-sim set, and its law names the case.
/// P11b (L7): with an operator chip the power is the operator's (the stated
/// feed and RPM at the point, `power_at_operating_point`), so a higher chip
/// moves the edge in. With none, the full suggested chip of the grid point.
fn power_limit(
    out: &mut Vec<Limit>,
    inputs: &OwnedInputs,
    speeds: &SpeedsMap,
    plan: &AxesPlan,
    case: crate::feeds::engagement::PowerCase,
    operator: Option<OperatorChip>,
) {
    if !plan.axes.ap_free() {
        return;
    }
    let fits_at = |ap: f64, ae: f64| -> bool {
        match operator {
            Some(chip) => inputs.operator_fits_power(
                CutPoint {
                    ap_mm: ap,
                    ae_mm: ae,
                },
                chip,
            ),
            None => fits_power(&inputs.calc_at(CutPoint {
                ap_mm: ap,
                ae_mm: ae,
            })),
        }
    };
    let mut points = Vec::new();
    for (j, &ae) in speeds.aes.iter().enumerate() {
        let fits = |i: usize| match operator {
            Some(_) => speeds.aps.get(i).is_some_and(|&ap| fits_at(ap, ae)),
            None => speeds.at(i, j).is_some_and(|p| p.fits_power),
        };
        let Some(first_out) = (0..speeds.aps.len()).find(|&i| !fits(i)) else {
            continue;
        };
        let (mut lo, mut hi) = match first_out.checked_sub(1) {
            Some(i) => (
                speeds.aps.get(i).copied().unwrap_or(MIN_AP_MM),
                speeds.aps.get(first_out).copied().unwrap_or(MIN_AP_MM),
            ),
            None => (0.0, speeds.aps.first().copied().unwrap_or(MIN_AP_MM)),
        };
        for _ in 0..EDGE_BISECTION_STEPS {
            let mid = 0.5 * (lo + hi);
            if fits_at(mid, ae) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        points.push([ae, lo]);
    }
    if points.is_empty() {
        return;
    }
    // A width sample with no crossing has no edge in the span: the curve
    // takes the top of the span there, so a crossing column stays exact.
    let top = speeds.aps.last().copied().unwrap_or(MIN_AP_MM);
    let mut curve: Vec<[f64; 2]> = speeds
        .aes
        .iter()
        .map(|&ae| {
            points
                .iter()
                .find(|p| (p[0] - ae).abs() < 1e-12)
                .copied()
                .unwrap_or([ae, top])
        })
        .collect();
    curve.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-12);
    use crate::feeds::engagement::PowerCase;
    out.push(Limit {
        kind: LimitKind::Power,
        law: match (operator.is_some(), case) {
            (false, PowerCase::Nominal) => "rated spindle power at the full chip (nominal case)",
            (false, PowerCase::PlannerCap) => {
                "rated spindle power at the full chip (planner cap case)"
            }
            (false, PowerCase::Slot) => "rated spindle power at the full chip (slot case)",
            (true, PowerCase::Nominal) => {
                "rated spindle power at the operator's chip (nominal case)"
            }
            (true, PowerCase::PlannerCap) => {
                "rated spindle power at the operator's chip (planner cap case)"
            }
            (true, PowerCase::Slot) => "rated spindle power at the operator's chip (slot case)",
        },
        basis: Basis::derived(
            "two-term power model <= rated curve",
            vec![Basis::Machine(MachineField::PowerCurve)],
        ),
        edge: Edge::ApCurve { points: curve },
    });
}

/// P11b (L6): the vendor depth table as a limit. Per width sample, the
/// deepest `ap` where the table's allowed chip (`band max x
/// doc_derating_scale(ap / D)`) still holds the PEAK chip at that width
/// (`geometry::vendor_depth_table_ceiling_ratio`). The chip is the
/// operator's (L7), else the suggested chip of the grid. The table is
/// printed as a chip rule at 1, 2 and 3 x D with no width (step 0 found that
/// it holds neither the force nor the chip), so it is drawn, not derived
/// from the force model. A peak chip at or below half the band maximum
/// meets the table at every printed depth: that column has no edge. No
/// band or printed point, a drill peck, a fixed depth, or no binding
/// column: no limit.
fn vendor_depth_limit(
    out: &mut Vec<Limit>,
    input: &EnvelopeInput<'_>,
    base: &FeedsResult,
    speeds: &SpeedsMap,
    plan: &AxesPlan,
    operator: Option<OperatorChip>,
) {
    if !plan.axes.ap_free() || plan.peck {
        return;
    }
    let band_max = base
        .chipload_bounds
        .map(|b| b.max_mm_per_tooth)
        .or(base.chipload_point_mm)
        .filter(|v| v.is_finite() && *v > 0.0);
    let (Some(band_max), Some(row)) = (band_max, base.matched_lut_row.as_ref()) else {
        return;
    };
    let d = input.tool.diameter;
    if !(d.is_finite() && d > 0.0) {
        return;
    }
    // A column where the peak chip is at or below half the band maximum is
    // inside the table at every printed depth; above 3 x D the vendors print
    // nothing, and the static check `feeds.depth_beyond_published_table`
    // names that depth. So the column has no edge in the span and takes the
    // span top (the power edge's rule). No binding column: no limit.
    let top = speeds.aps.last().copied().unwrap_or(MIN_AP_MM);
    // The edge is closed form, so it also takes a sample at the operator's
    // stated width: the ceiling there is exact, not an interpolation between
    // two grid columns.
    let mut widths: Vec<(f64, Option<usize>)> = speeds
        .aes
        .iter()
        .enumerate()
        .map(|(j, &ae)| (ae, Some(j)))
        .collect();
    if let (Some(ae), Some(_)) = (plan.stated_ae, operator)
        && !speeds.aes.iter().any(|w| (w - ae).abs() < 1e-12)
    {
        widths.push((ae, None));
        widths.sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    let mut curve: Vec<[f64; 2]> = Vec::with_capacity(widths.len());
    let mut binds = false;
    for (ae, column) in widths {
        let fz = match (operator, column) {
            (Some(chip), _) => chip.fz_mm,
            (None, Some(j)) => match speeds.at(0, j) {
                Some(p) => p.chip.get().0,
                None => continue,
            },
            (None, None) => continue,
        };
        let psi = crate::feeds::force::immersion_angle(ae, 0.5 * d);
        let peak = fz * psi.min(std::f64::consts::FRAC_PI_2).sin();
        let share = peak / band_max;
        if share > 0.5 {
            binds = true;
            let ratio = crate::feeds::geometry::vendor_depth_table_ceiling_ratio(share);
            curve.push([ae, ratio * d]);
        } else {
            curve.push([ae, top]);
        }
    }
    if !binds {
        return;
    }
    let first = curve.first().map_or(0.0, |p| p[1]);
    let flat = curve.iter().all(|p| (p[1] - first).abs() < 1e-9);
    out.push(Limit {
        kind: LimitKind::VendorDepthTable,
        law: if operator.is_some() {
            "vendor depth table at the operator's peak chip (1/2/3 x D)"
        } else {
            "vendor depth table at the suggested peak chip (1/2/3 x D)"
        },
        basis: Basis::derived(
            "vendor depth table (Onsrud, Freud, Amana)",
            vec![Basis::Printed(crate::feeds::sourced::RowRef(
                row.observation_id.clone(),
            ))],
        ),
        edge: if flat {
            Edge::ApCeiling { mm: first }
        } else {
            Edge::ApCurve { points: curve }
        },
    });
}

/// P11a (D-B): the reason of a `Deflection` entry when no depth below the
/// flute reaches the target in any column.
pub const NO_DEFLECTION_LIMIT_BELOW_THE_FLUTE: &str = "no deflection limit below the flute";

/// The tip-deflection edge (roughing, as pass 6): per width sample, the
/// shallowest `ap` where the deflection meets the 200 µm target, by the
/// one beam (`cutter_axial_constraints`). A column with no root below the
/// flute adds no point (P11a, D-B).
fn deflection_limit(
    out: &mut Vec<Limit>,
    input: &EnvelopeInput<'_>,
    speeds: &SpeedsMap,
    plan: &AxesPlan,
    role: PassRole,
    operator: Option<OperatorChip>,
) {
    if !plan.axes.ap_free() || !matches!(role, PassRole::Roughing) {
        return;
    }
    // T-4: the beam cannot answer. State the abstention with its reason.
    // `cutter_axial_constraints` returns its upper search bracket when the
    // beam refuses, which would read as a deep limit (an absence rendered
    // as a reading), so the refusal is decided here first.
    let unmodelled = |reason: crate::feeds::predict::DeflectionUnmodeled| Limit {
        kind: LimitKind::Deflection,
        law: "tip deflection <= 200 um",
        basis: Basis::Rule(crate::feeds::suggest::DEFLECTION_BACKOFF_TARGET_ROW),
        edge: Edge::Unmodelled {
            reason: reason.clause(),
        },
    };
    let Ok(tool_def) = crate::compute::cutter::build_cutter(input.tool) else {
        out.push(unmodelled(
            crate::feeds::predict::DeflectionUnmodeled::ToolOutOfDomain,
        ));
        return;
    };
    if input.material.force_line().is_err() {
        out.push(unmodelled(
            crate::feeds::predict::DeflectionUnmodeled::MaterialUnvalidated,
        ));
        return;
    }
    if !(input.tool.stickout_planning_mm().is_finite() && input.tool.stickout_planning_mm() > 0.0) {
        out.push(unmodelled(
            crate::feeds::predict::DeflectionUnmodeled::NoStickout,
        ));
        return;
    }
    let mut columns: Vec<(f64, Option<f64>)> = Vec::new();
    for (j, &ae) in speeds.aes.iter().enumerate() {
        // P11b (L7): the operator's chip; else the suggested chip at the
        // shallowest sample (the largest chip of the column).
        let fz = match operator {
            Some(chip) => chip.fz_mm,
            None => match speeds.at(0, j) {
                Some(p) => p.chip.get().0,
                None => continue,
            },
        };
        let c = crate::feeds::cutter_constraints::cutter_axial_constraints(
            &tool_def,
            input.material,
            ae.max(1.0e-3),
            fz,
            None,
            None,
            Some(crate::feeds::suggest::DEFLECTION_BACKOFF_TARGET_UM),
        );
        // P11a (D-B): the solved depth, or `None` when no depth below the
        // flute reaches the target. The search bracket is never drawn.
        columns.push((
            ae,
            c.max_doc_deflection_mm
                .filter(|v| v.is_finite() && *v >= 0.0),
        ));
    }
    if columns.iter().all(|(_, mm)| mm.is_none()) {
        // No column reaches the target below the flute: the limit is
        // absent, and the envelope says so (an absence is stated, never
        // dropped and never drawn as an edge).
        out.push(Limit {
            kind: LimitKind::Deflection,
            law: "tip deflection <= 200 um",
            basis: Basis::Rule(crate::feeds::suggest::DEFLECTION_BACKOFF_TARGET_ROW),
            edge: Edge::Unmodelled {
                reason: NO_DEFLECTION_LIMIT_BELOW_THE_FLUTE,
            },
        });
        return;
    }
    // A column with no root has no edge in the span: the curve takes the
    // top of the span there (the power edge's rule), so a crossing column
    // stays exact.
    let top = speeds.aps.last().copied().unwrap_or(MIN_AP_MM);
    let curve: Vec<[f64; 2]> = columns
        .into_iter()
        .map(|(ae, mm)| [ae, mm.unwrap_or(top)])
        .collect();
    out.push(Limit {
        kind: LimitKind::Deflection,
        law: "tip deflection <= 200 um",
        basis: Basis::derived(
            "one beam (ToolDefinition::tip_deflection_mm)",
            vec![Basis::Rule(
                crate::feeds::suggest::DEFLECTION_BACKOFF_TARGET_ROW,
            )],
        ),
        edge: Edge::ApCurve { points: curve },
    });
}

/// The `Rigidity` limit (roughing). P13c (O1):
///
/// - The Adaptive family: `adaptive_doc_factor x D`, an `ap` ceiling
///   (`suggest::rigidity_cap_mm`).
/// - Every other roughing family: per width sample, the shallowest depth
///   where the peak engaged-edge force at the column chip (the operator's,
///   else the grid chip at the shallowest sample) meets the machine force
///   budget. Extra exact samples at the slot threshold, the corner bound
///   and the stated width. A column with no root takes the span top; no
///   root anywhere, or no force line, is an `Unmodelled` entry with its
///   reason. Returns the model the proposal solves exactly.
fn rigidity_limit(
    out: &mut Vec<Limit>,
    inputs: &OwnedInputs,
    speeds: &SpeedsMap,
    plan: &AxesPlan,
    (family, role): (OperationFamily, PassRole),
    operator: Option<OperatorChip>,
) -> Option<BudgetModel> {
    use crate::feeds::force_budget as fb;
    let rule = inputs.machine.rigidity.depth_cap_rule(family, role)?;
    if !matches!(role, PassRole::Roughing) {
        return None;
    }
    let budget_n = match rule {
        crate::machine::DepthCapRule::Factor(_) => {
            let probe_depth = match plan.axes {
                EnvelopeAxes::Plane { ap, .. } | EnvelopeAxes::AxialBar { ap, .. } => ap.max_mm,
                EnvelopeAxes::RadialBar { ap_mm, .. } | EnvelopeAxes::Fixed { ap_mm, .. } => ap_mm,
            };
            if let Some(cap) = crate::feeds::suggest::rigidity_cap_mm(
                family,
                &inputs.tool,
                &inputs.machine,
                role,
                probe_depth,
            ) {
                out.push(Limit {
                    kind: LimitKind::Rigidity,
                    law: "machine rigidity depth cap (adaptive_doc_factor x D)",
                    basis: crate::feeds::rigidity_basis(&inputs.machine),
                    edge: Edge::ApCeiling { mm: cap },
                });
            }
            return None;
        }
        crate::machine::DepthCapRule::ForceBudget(n) => n,
    };
    let basis = budget_basis(&inputs.machine);
    let unmodelled = |reason: &'static str| Limit {
        kind: LimitKind::Rigidity,
        law: fb::FORCE_BUDGET_LAW,
        basis: basis.clone(),
        edge: Edge::Unmodelled { reason },
    };
    if inputs.material.force_line().is_err() {
        out.push(unmodelled(fb::NO_FORCE_LINE));
        return None;
    }
    let Ok(tool) = crate::compute::cutter::build_cutter(&inputs.tool) else {
        out.push(unmodelled(fb::NO_BUDGET_INPUT));
        return None;
    };
    let mut model = BudgetModel {
        budget_n,
        operator_fz: operator.map(|c| c.fz_mm),
        solved: SolvedColumns::default(),
    };
    let flutes = inputs.tool.flute_count_planning();
    let d = inputs.tool.diameter;
    let mut widths: Vec<(f64, Option<usize>)> = speeds
        .aes
        .iter()
        .enumerate()
        .map(|(j, &ae)| (ae, Some(j)))
        .collect();
    if let (Some(first), Some(last)) = (speeds.aes.first(), speeds.aes.last()) {
        let (threshold_d, _, _) = crate::feeds::slot_rule(&inputs.machine);
        let mut extra = vec![d * threshold_d];
        if plan.computed_width {
            extra.push(d * fb::corner_coverage_width_factor());
        }
        extra.extend(plan.stated_ae);
        for ae in extra {
            if ae > *first && ae < *last && !widths.iter().any(|w| (w.0 - ae).abs() < 1e-12) {
                widths.push((ae, None));
            }
        }
    }
    widths.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut columns: Vec<(f64, Option<fb::BudgetDepth>)> = Vec::with_capacity(widths.len());
    for (ae, column) in widths {
        let fz = match (model.operator_fz, column) {
            (Some(fz), _) => fz,
            (None, Some(j)) => match speeds.at(0, j) {
                Some(p) => p.chip.get().0,
                None => continue,
            },
            (None, None) => chip_of(
                &inputs.calc_at(CutPoint {
                    ap_mm: MIN_AP_MM,
                    ae_mm: ae,
                }),
                flutes,
            ),
        };
        columns.push((
            ae,
            fb::depth_at_budget(&tool, &inputs.material, ae.max(1.0e-3), fz, budget_n),
        ));
    }
    if columns.iter().all(|(_, c)| c.is_none()) {
        out.push(unmodelled(fb::NO_BUDGET_INPUT));
        return None;
    }
    model.solved = SolvedColumns(
        columns
            .iter()
            .filter_map(|(ae, c)| c.map(|d| (*ae, d)))
            .collect(),
    );
    if columns
        .iter()
        .all(|(_, c)| matches!(c, None | Some(fb::BudgetDepth::NoRootBelowFlute)))
    {
        out.push(unmodelled(fb::NO_BUDGET_LIMIT_BELOW_THE_FLUTE));
        return Some(model);
    }
    let floored = columns
        .iter()
        .any(|(_, c)| matches!(c, Some(fb::BudgetDepth::BelowFloor)));
    let top = speeds.aps.last().copied().unwrap_or(MIN_AP_MM);
    let curve: Vec<[f64; 2]> = columns
        .into_iter()
        .map(|(ae, c)| [ae, c.and_then(fb::BudgetDepth::ceiling_mm).unwrap_or(top)])
        .collect();
    let edge = match curve.as_slice() {
        [only] => Edge::ApCeiling { mm: only[1] },
        _ => Edge::ApCurve { points: curve },
    };
    out.push(Limit {
        kind: LimitKind::Rigidity,
        law: if floored {
            fb::FORCE_BUDGET_FLOOR_LAW
        } else {
            fb::FORCE_BUDGET_LAW
        },
        basis,
        edge,
    });
    Some(model)
}

/// P13c (O1): the basis of a force-budget value: the machine rigidity (its
/// class text) and, on the hobby default budget, the chart row that sets
/// it (F201 Cited).
fn budget_basis(machine: &MachineProfile) -> Basis {
    let mut leaves = vec![crate::feeds::rigidity_basis(machine)];
    let hobby = crate::feeds::force_budget::hobby_budget().force_n;
    if machine.rigidity.force_budget_n.to_bits() == hobby.to_bits() {
        leaves.push(Basis::Rule(crate::feeds::force_budget::hobby_budget_row()));
    }
    Basis::derived(crate::feeds::force_budget::FORCE_BUDGET_LAW, leaves)
}

/// The move-budget floor on the width (pass 3), when the context gives the
/// model box and the op has a free stepover.
fn runtime_limit(out: &mut Vec<Limit>, input: &EnvelopeInput<'_>, plan: &AxesPlan, d: f64) {
    if !plan.axes.ae_free() || input.operation.stepover().is_none() {
        return;
    }
    let Some(bbox) = input.context.model_bbox else {
        return;
    };
    let target = crate::feeds::suggest::STEPOVER_BACKOFF_TARGET_MOVES_ROW.value;
    let ceiling = d * crate::feeds::suggest::STEPOVER_BACKOFF_DIAMETER_FRACTION_ROW.value;
    let moves_at = |s: f64| {
        let mut op = input.operation.clone();
        op.set_stepover(s);
        crate::feeds::predict::predict_move_count(&op, Some(bbox), input.tool, input.machine)
    };
    let within = |s: f64| {
        #[expect(
            clippy::cast_precision_loss,
            reason = "a move count; the budget is 5e5, exact in an f64"
        )]
        let moves = moves_at(s) as f64;
        moves <= target
    };
    if within(MIN_AE_MM) {
        return;
    }
    let floor = if within(ceiling) {
        let (mut lo, mut hi) = (MIN_AE_MM, ceiling);
        for _ in 0..RUNTIME_BISECTION_STEPS {
            let mid = 0.5 * (lo + hi);
            if within(mid) {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        hi
    } else {
        ceiling
    };
    out.push(Limit {
        kind: LimitKind::Runtime,
        law: "predicted moves <= the move budget",
        basis: Basis::derived(
            "move budget",
            vec![
                Basis::Rule(crate::feeds::suggest::STEPOVER_BACKOFF_TARGET_MOVES_ROW),
                Basis::Rule(crate::feeds::suggest::STEPOVER_BACKOFF_DIAMETER_FRACTION_ROW),
            ],
        ),
        edge: Edge::AeFloor { mm: floor },
    });
}

/// The limits that the speeds map judges per point: the chipload band, the
/// rubbing floor and the machine feed cap. Each is listed when a grid point
/// meets it.
fn per_point_limits(out: &mut Vec<Limit>, base: &FeedsResult, speeds: &SpeedsMap) {
    if let Some(row) = base.matched_lut_row.as_ref()
        && (base.chipload_bounds.is_some() || base.chipload_point_mm.is_some())
    {
        out.push(Limit {
            kind: LimitKind::ChipBand,
            law: "printed chip band (peak chip)",
            basis: Basis::Printed(crate::feeds::sourced::RowRef(row.observation_id.clone())),
            edge: Edge::PerPoint,
        });
    }
    if speeds.points.iter().any(|p| p.rubbing) {
        let (_, source) =
            crate::feeds::rubbing_floor_at(base.chipload_bounds, base.chipload_point_mm);
        let basis = match (source, base.matched_lut_row.as_ref()) {
            (crate::feeds::RubbingFloorSource::RepoConstant, _) | (_, None) => {
                Basis::Rule(crate::material::FormulaConstant::unsourced(
                    "feeds.RUBBING_FLOOR_MM_TOOTH",
                    crate::feeds::RUBBING_FLOOR_MM_TOOTH,
                    "repo rule with no primary source (the doc says so)",
                ))
            }
            (
                crate::feeds::RubbingFloorSource::BandMinimum
                | crate::feeds::RubbingFloorSource::BandMaximum
                | crate::feeds::RubbingFloorSource::PrintedPoint,
                Some(row),
            ) => Basis::Printed(crate::feeds::sourced::RowRef(row.observation_id.clone())),
        };
        out.push(Limit {
            kind: LimitKind::RubbingFloor,
            law: "min(0.025, band min) per tooth (warns, never lifts)",
            basis,
            edge: Edge::PerPoint,
        });
    }
    if speeds.points.iter().any(|p| p.feed_capped) {
        out.push(Limit {
            kind: LimitKind::FeedCap,
            law: "machine cutting-feed ceiling",
            basis: Basis::Machine(MachineField::CuttingFeedCap),
            edge: Edge::PerPoint,
        });
    }
}

/// The K2b refusals of the compiler. Each `compile_fail` block has a
/// sibling block that compiles.
///
/// Code cannot make an envelope pick without an envelope (the fields are
/// private):
///
/// ```compile_fail
/// use rs_cam_core::feeds::envelope::EnvelopePick;
/// fn make() -> EnvelopePick<'static> {
///     EnvelopePick { point: todo!(), placement: todo!(), result: todo!() }
/// }
/// ```
///
/// Code cannot make an envelope without `for_cut` (no `Default`):
///
/// ```compile_fail
/// use rs_cam_core::feeds::envelope::SafeEnvelope;
/// let _ = SafeEnvelope::default();
/// ```
///
/// The sibling: an envelope picks a point, and the pick reads its result.
///
/// ```
/// use rs_cam_core::feeds::envelope::{PickRequest, SafeEnvelope};
/// fn read(env: &SafeEnvelope) -> f64 {
///     env.pick(PickRequest::Proposal).result().feed_rate_mm_min
/// }
/// ```
#[doc(hidden)]
pub mod envelope_refusals {}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
    reason = "test code: a panic is how a test reports a failure; the budget test prints its measurement"
)]
mod tests {
    use super::*;
    use crate::compute::catalog::OperationType;
    use crate::compute::tool_config::{ToolId, ToolType};

    fn pocket_envelope() -> SafeEnvelope {
        let op = crate::compute::fixture_values::fixture_op(OperationType::Pocket);
        let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
        tool.diameter = 6.0;
        let machine = MachineProfile::default();
        let material = Material::default();
        SafeEnvelope::for_cut(EnvelopeInput {
            operation: &op,
            tool: &tool,
            machine: &machine,
            material: &material,
            lut: crate::feeds::embedded_vendor_lut(),
            spindle_strategy: SpindleStrategy::default(),
            context: SuggestContext::default(),
        })
        .expect("a 6 mm end mill pocket is backed")
    }

    #[test]
    fn the_envelope_meets_its_time_budget() {
        // Warm the LUT before the clock starts.
        let _ = pocket_envelope();
        let start = std::time::Instant::now();
        let env = pocket_envelope();
        let ms = start.elapsed().as_millis();
        println!(
            "one Pocket plane envelope: {ms} ms, {} points",
            env.speeds.points.len()
        );
        assert_eq!(env.speeds.points.len(), ENVELOPE_GRID_N * ENVELOPE_GRID_N);
        assert!(ms <= ENVELOPE_TIME_BUDGET_MS, "{ms} ms over the budget");
    }

    #[test]
    fn the_lookup_returns_the_grid_value_at_a_grid_point() {
        let env = pocket_envelope();
        let p = env.speeds.at(2, 3).unwrap();
        let s = env.speeds.speeds_at(p.ap_mm, p.ae_mm).unwrap();
        assert!((s.feed_mm_min - p.feed.get().0).abs() < 1e-9);
        assert!((s.rpm - p.rpm.get().0).abs() < 1e-9);
    }

    #[test]
    fn the_proposal_never_raises_a_stated_axis() {
        let env = pocket_envelope();
        let stated = env.stated.expect("a pocket states both axes");
        let p = env.proposal.get();
        assert!(p.ap_mm <= stated.ap_mm + 1e-12);
        assert!(p.ae_mm <= stated.ae_mm + 1e-12);
    }
}
