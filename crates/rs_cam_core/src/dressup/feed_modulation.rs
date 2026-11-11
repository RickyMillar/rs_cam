//! F-036 / F-039 — Per-move adaptive feed modulation algorithm.
//!
//! Two strategies live behind one entry point:
//!
//! - [`ModulationStrategy::ConstrainedMax`] (F-039, default) — solves a
//!   per-move constrained-optimisation problem. Seven candidate limits
//!   compete; the smallest wins, scaled by `feed_scale`, then
//!   floored at the chipload-min band edge. A printed point
//!   ([`ChiploadBand::point`], A2) gets no floor. Each per-move decision
//!   records the [`BindingConstraint`] that drove it so the diagnostic
//!   surface can explain *why* a given feed landed where it did.
//! - [`ModulationStrategy::BandMid`] (F-036, fallback) — the original
//!   "target band-mid × chip-thinning × clamps" heuristic. Kept as a
//!   one-release-cycle fallback so users surprised by F-039's
//!   behaviour can opt back into the legacy algorithm without
//!   reverting the build.
//!
//! ## Module surface
//!
//! - [`adaptive_feed_modulate`] mutates the toolpath's per-move
//!   `feed_rate` and returns a [`ModulationOutcome`] (the count of
//!   rewritten moves, per-move map of `(feed, binding)`).
//! - [`ModulationContext`] carries the per-toolpath inputs (RPM,
//!   flutes, machine cap, LUT band, kinematics, optional
//!   deflection/power/force-budget inputs).
//! - [`BindingConstraint`] is re-exported from
//!   [`crate::tool_load::BindingConstraint`] so consumers can match
//!   without an extra import.
//! - [`ChiploadBand`] and [`PerMoveEngagement`] are unchanged from
//!   F-036; the constrained-max solver consumes the same engagement
//!   summary the band-mid path always has.
//!
//! ## What this module does NOT do
//!
//! - It does not read or write G-code. The toolpath IR carries per-
//!   move `feed_rate`; G-code emission of `F<rate>` on every feed
//!   change is F-036a. F-039 layered an *optional* per-move comment
//!   onto that path; see [`PostDefinition`](crate::gcode::post::PostDefinition) / `emit_modulation_comments`.
//! - It does not call the simulator. Engagement summaries are an
//!   input.
//! - It does not modulate `MoveType::Rapid`, retract moves
//!   (`MoveIntent::Retract`), or drilling plunges (`MoveIntent::Drilling`,
//!   `MoveIntent::EntryPlunge`). The chip-thinning + force corrections
//!   are only well-defined for lateral / arc / helix engagement.
//!
//! ## The Phase 3 plunge guard (2026-09-07)
//!
//! The skip above is keyed on the move's INTENT tag. A generator that
//! emits a vertical descent as a plain cutting move escapes it, and every
//! strategy then lifts that descent to the lateral chipload band. So
//! [`adaptive_feed_modulate`] adds one GEOMETRIC cap after the strategy
//! decides: a move the shared classifier
//! ([`crate::machine::kinematic_utilization::classify_move`]) calls
//! [`crate::machine::kinematic_utilization::MotionClass::Plunge`] is capped at
//! [`ModulationContext::plunge_rate_mm_min`] and reports
//! [`BindingConstraint::PlungeRate`]. The intent skip is UNCHANGED — a
//! tagged plunge still keeps its operator-tuned feed exactly.
//!
//! See `planning/acceptance_loop/findings/F-039-constrained-max-feed-modulation.md`
//! for the design narrative + acceptance bars.

use std::collections::BTreeMap;

use crate::feeds::engagement::Engagement;
use crate::feeds::force::DeflectionCapRefusal;
use crate::machine::CuttingCeilingMmMin;
use crate::machine::kinematics::{MachineKinematics, predicted_feeds_for_toolpath};
use crate::toolpath::{MoveIntent, MoveType, Toolpath};

pub use crate::feeds::geometry::RadialThinning;
pub use crate::tool_load::BindingConstraint;

/// Selects which modulation algorithm runs. Stored on
/// [`crate::session::SimulationOptions`]; defaults to
/// [`Self::ConstrainedMax`] in F-039.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ModulationStrategy {
    /// F-036 "target band-mid" heuristic. Walks the chipload band's
    /// geometric mid as the target and applies the chip-thinning lift of
    /// the one rule (`RadialThinning`, Garr; feeds contract P14, review
    /// M2: it was `1 / sqrt(radial_woc_fraction)`), then clamps at the
    /// band edges and the machine cap. Kept as a fallback for one release
    /// cycle.
    BandMid,
    /// F-039 constrained-max solver (default). Computes the smallest
    /// of seven candidate feed limits — chipload-max, deflection-max,
    /// power-max, force-budget (P16 item 7), machine-max, kinematic-reach,
    /// chipload-min floor — and emits the binding value scaled by
    /// `feed_scale`.
    #[default]
    ConstrainedMax,
}

impl ModulationStrategy {
    /// Stable tag mirroring the strategy enum onto the verdict-side
    /// serialised form. Used by [`adaptive_feed_modulate`] to populate
    /// [`crate::tool_load::ModulationSummary::strategy`].
    pub fn tag(self) -> crate::tool_load::ModulationStrategyTag {
        match self {
            Self::BandMid => crate::tool_load::ModulationStrategyTag::BandMid,
            Self::ConstrainedMax => crate::tool_load::ModulationStrategyTag::ConstrainedMax,
        }
    }
}

/// Chipload band (`mm/tooth`) sourced from the vendor LUT for a given
/// tool / material pair.
///
/// `min` is the matched vendor row's own lower edge — below it the row
/// says the tooth scrapes instead of slicing and in wood the workpiece
/// scorches. `max` is the breakage / over-load ceiling. The modulator
/// floors emitted feeds at `min × rpm × flutes` (after the feed scale)
/// and caps the constrained-max search at `max × rpm × flutes`.
///
/// **Not the same thing as the rubbing floor** (Checkpoint K (d2),
/// 2026-08-13): [`crate::feeds::RUBBING_FLOOR_MM_TOOTH`] is a global,
/// diameter- and material-independent chip-formation threshold, and
/// [`crate::feeds::effective_rubbing_floor`] subordinates it to
/// `band.max`. On a sub-Ø2 tool the two land at **opposite ends of this
/// band**. This field is `band.min` and nothing else.
///
/// ## A printed point (A2, point mode)
///
/// A vendor row that prints one value gives a point, not a band
/// ([`crate::feeds::vendor_lookup::PrintedChipload::Point`]).
/// [`Self::point`] holds it as `min == max == v` and marks it, so
/// [`Self::is_point`] is `true`. The modulator reads a point so:
///
/// - `v × rpm × flutes` is the chipload cap (both strategies).
/// - No floor applies, because nothing printed a minimum. The feed scale
///   can take the feed below the point.
/// - The deflection and power "pin" arms pin to `v`.
/// - A move held at the point keeps the `ChiploadMax` tag (decision Q7).
///
/// [`Self::new`] with `min == max` is NOT a point: it stays a zero-width
/// band with a floor, as before A2.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChiploadBand {
    /// Minimum chipload (mm per tooth). Must be > 0 and ≤ `max`. For a
    /// point it equals `max`, and the modulator applies no floor.
    pub min_mm_per_tooth: f64,
    /// Maximum chipload (mm per tooth). Must be ≥ `min`.
    pub max_mm_per_tooth: f64,
    /// `true` when this holds one printed value (A2). Only
    /// [`Self::point`] sets it.
    point: bool,
}

impl ChiploadBand {
    /// Construct a chipload band. Returns `None` if either value is
    /// non-finite, `min` is ≤ 0, or `max` < `min`.
    pub fn new(min_mm_per_tooth: f64, max_mm_per_tooth: f64) -> Option<Self> {
        if !min_mm_per_tooth.is_finite()
            || !max_mm_per_tooth.is_finite()
            || min_mm_per_tooth <= 0.0
            || max_mm_per_tooth < min_mm_per_tooth
        {
            return None;
        }
        Some(Self {
            min_mm_per_tooth,
            max_mm_per_tooth,
            point: false,
        })
    }

    /// Construct a printed point `v` (A2, point mode). It stores
    /// `min = max = v` and [`Self::is_point`] is `true`. Returns `None`
    /// if `v` is non-finite or ≤ 0.
    pub fn point(value_mm_per_tooth: f64) -> Option<Self> {
        if !value_mm_per_tooth.is_finite() || value_mm_per_tooth <= 0.0 {
            return None;
        }
        Some(Self {
            min_mm_per_tooth: value_mm_per_tooth,
            max_mm_per_tooth: value_mm_per_tooth,
            point: true,
        })
    }

    /// `true` when this holds one printed value, not a band. The
    /// modulator then applies no chipload floor.
    #[inline]
    pub fn is_point(&self) -> bool {
        self.point
    }

    /// Geometric midpoint of the band — the [`ModulationStrategy::BandMid`]
    /// target. Geometric (not arithmetic) so the target sits
    /// proportionally between min and max regardless of band width.
    #[inline]
    pub(crate) fn mid_mm_per_tooth(&self) -> f64 {
        (self.min_mm_per_tooth * self.max_mm_per_tooth).sqrt()
    }
}

/// Per-move engagement summary the modulator consumes.
///
/// One entry per cutting move in the toolpath (Rapids carry no
/// engagement and the modulator skips them). The two fractions match
/// the simulator's `Engagement` axes: `radial_woc_fraction` is the
/// cylinder-side radial WOC fraction (0.0 = air, 1.0 = full slot);
/// `axial_doc_fraction` is the cutter's flute-engaged-height fraction.
/// Both are time-weighted means across the move's samples.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PerMoveEngagement {
    /// Mean radial WOC fraction across the move's cutting samples.
    /// `0.0` is acceptable (the modulator treats the move as air and
    /// leaves its feed at the commanded value).
    pub radial_woc_fraction: f64,
    /// Mean axial DOC fraction across the move's cutting samples.
    ///
    /// **This is a fraction of the tool's FLUTE LENGTH**, not of the
    /// operation's depth per pass — see `dexel_stock::simulation`, which
    /// writes it as `axial_engagement_mm / flute_length`. It is kept for
    /// the zero-engagement short-circuits, which only test it against
    /// zero. **Do not multiply it by a depth**; T-11 did exactly that and
    /// read every cut about ten times too shallow. Use
    /// [`Self::axial_doc_mm`] for the depth.
    pub axial_doc_fraction: f64,
    /// T-11 (2026-09-16) — mean axial engagement in MILLIMETRES across
    /// the move's cutting samples, straight off `CutSample::axial_doc_mm`.
    ///
    /// The dexel measures this directly, so nothing needs to re-derive it.
    /// `0.0` means the move had no measured cutting engagement, which the
    /// solver treats the same way it treats an air move.
    pub axial_doc_mm: f64,
    /// Feeds contract P11b2: the radial chip-thinning lift of the move, the
    /// MINIMUM over its cutting samples of
    /// `tool_load::display::sample_thinning_lift` (the lift the post-sim gate
    /// reads at each sample's measured arc). With the minimum, a move held
    /// at `band.max x lift x rpm x Z` gives a peak chip at or below the band
    /// max at every one of its samples, so the gate cannot disagree. `None`
    /// for a move with no measured arc (a synthetic engagement): the solver
    /// then reads the modelled width `radial_woc_fraction`. Every production
    /// trace carries the arcs.
    pub thinning_lift: Option<f64>,
}

/// Feeds contract P20g: the engagement of one cutting sample, built by the
/// gates' own builders. A move has one F word, so each load cap of the move
/// is the MINIMUM over its samples of that cap at the sample's own
/// engagement (lead ruling 1, 2026-11-09).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SampleEngagement {
    /// The power gate's engagement (`Engagement::from_sample`, depth
    /// `axial_doc_mm`); the power cap reads it.
    pub power: Option<Engagement>,
    /// The deflection gate's engagement
    /// (`tool_load::deflection::sample_engagement`, depth
    /// `axial_engagement_mm`, the peak engaged-edge force); the deflection
    /// and force-budget caps read it.
    pub force: Option<Engagement>,
}

impl SampleEngagement {
    /// The engagement of one simulation sample through the gates' own
    /// builders: the power gate's `Engagement::from_sample` and the
    /// deflection gate's `tool_load::deflection::sample_engagement`. The
    /// session builds the side table with this one function, so the
    /// modulator and the gates read one definition (lead ruling 2).
    #[must_use]
    pub fn of_sample(
        sample: &crate::stock::simulation_cut::SimulationCutSample,
        tool: &crate::tool::ToolDefinition,
    ) -> Self {
        Self {
            power: Engagement::from_sample(sample, tool),
            force: crate::tool_load::deflection::sample_engagement(tool, sample),
        }
    }
}

/// Feeds contract P20g: the sample engagements of one move, an entry of
/// [`ModulationContext::sample_engagements`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MoveSampleEngagements {
    /// One entry per cutting sample of the move that the gates grade (the
    /// session drops the phantom-transit samples with the gate's own
    /// `tool_load::locality::is_phantom_transit`). Empty: the load caps
    /// read the mean engagement of [`PerMoveEngagement`], as before P20g.
    pub samples: Vec<SampleEngagement>,
    /// The mean engagement binds too. The session sets it where the
    /// ContourSpiral planner floor lifted the mean width above the
    /// simulated width (the dexel width reads low on that strategy), so
    /// the per-sample caps never remove that floor.
    pub mean_binds: bool,
}

/// Feeds contract P11a: the cutter facts that the deflection and power
/// caps need to build one `feeds::engagement::Engagement` per move. The
/// engaged diameter is `feeds::effective_diameter` at the MOVE depth (the
/// open P9 diameter; it was the diameter at the toolpath's peak depth).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModulationCutter {
    /// The cutter's shape: the chip section and the engaged diameter.
    pub geometry: crate::feeds::ToolGeometryHint,
    /// The nominal cutting diameter (mm).
    pub diameter_mm: f64,
    /// The cutting cap (mm), `ToolBody::max_cut_diameter_mm`: a tapered
    /// ball's engaged diameter stops at it (P22-0c-6 ruling 1).
    pub max_cut_diameter_mm: f64,
    /// The helix angle (degrees): the lag of the engaged edge.
    pub helix_deg: f64,
}

impl ModulationCutter {
    /// The engaged diameter at the axial depth `ap_mm`.
    #[must_use]
    pub fn d_eng_at(&self, ap_mm: f64) -> f64 {
        self.engaged_diameter_at(ap_mm).mm()
    }

    /// P15: the engaged diameter at `ap_mm` through the one door.
    fn engaged_diameter_at(&self, ap_mm: f64) -> crate::feeds::engagement::EngagedDiameter {
        crate::feeds::engagement::EngagedDiameter::of_hint(
            self.geometry,
            self.diameter_mm,
            self.max_cut_diameter_mm,
            ap_mm,
        )
    }

    /// The one engagement of a move: depth `ap_mm`, width
    /// `woc_fraction x d_eng`.
    fn engagement(&self, ap_mm: f64, woc_fraction: f64) -> crate::feeds::engagement::Engagement {
        let d_eng = self.engaged_diameter_at(ap_mm);
        crate::feeds::engagement::Engagement::from_cut(
            crate::feeds::envelope::CutPoint {
                ap_mm,
                ae_mm: woc_fraction * d_eng.mm().max(0.0),
            },
            self.geometry,
            d_eng,
            Some(self.helix_deg),
        )
    }
}

/// Optional deflection-cap inputs for the constrained-max solver.
/// When `None` the deflection constraint is skipped (typical for
/// unit tests and band-mid runs).
///
/// These carry the **same engaged-edge force + beam compliance the
/// post-sim deflection gate uses**, so the optimizer and gate agree on a
/// cut. The lateral force is the PEAK resultant of the force line summed
/// over the engaged edge (`Engagement::edge_force`, feeds contract P11a,
/// L5), and tip deflection is linear in force `δ = compliance · F`. So
/// the solver turns `δ ≤ bound` into a force budget and solves the feed
/// cap (`Engagement::fz_cap_for_peak_force`). The line comes from
/// [`crate::material::Material::force_line`]; `compliance` is evaluated
/// once at the toolpath's peak axial DOC from the integrated two-section
/// cantilever ([`crate::tool::ToolDefinition::tip_deflection_mm`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeflectionLimitInputs {
    /// The material's force line (the line alone: deflection does not
    /// read the grain factor).
    pub line: crate::material::force_line::ForceLine,
    /// The cutter facts of the engagement.
    pub cutter: ModulationCutter,
    /// Tip deflection per newton of lateral force (mm/N), from the same
    /// integrated cantilever the gate uses, at the toolpath's peak axial
    /// DOC. Deflection is linear in force, so one scalar suffices.
    pub compliance_mm_per_n: f64,
    /// Maximum allowed tip displacement (mm). F-039 uses 0.2 mm
    /// matching [`crate::tool_load::deflection::EXCEEDS_BOUND_MM`].
    pub max_tip_deflection_mm: f64,
}

/// Optional power-cap inputs for the constrained-max solver. `None`
/// disables the power constraint.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerLimitInputs {
    /// The material's force line — the value
    /// [`crate::material::Material::force_line`] returns. The
    /// constrained-max solver applies the line's grain factor internally
    /// (through `tool_load::power::PowerTerms`), so callers don't
    /// pre-multiply (S2-9 in
    /// `planning/tool_kinematics_chipload_audit_2026-05-31.md`).
    pub line: crate::material::force_line::ForceLine,
    /// The cutter facts of the engagement: the chip section (K7, a V-bit
    /// removes half the rectangle) and the engaged diameter at the move
    /// depth (P11a).
    pub cutter: ModulationCutter,
    /// Available spindle power × safety factor at the running RPM
    /// (kW). The constrained-max solver caps the feed so the predicted
    /// two-term power (`tool_load::power::PowerTerms`) stays inside
    /// this. Because the edge term carries no feed, a budget below that
    /// term's own floor has no feed answer at all; the solver pins to
    /// the chipload-min floor and still tags the move `PowerMax`.
    pub available_kw: f64,
}

/// The note of a move summary whose op has the force-budget rule but whose
/// material has no force line: the modulator applies no force-budget limit
/// (feeds contract P16 item 7).
pub const FORCE_BUDGET_NO_FORCE_LINE_NOTE: &str = "force budget: no force line";

/// Feeds contract P20g (lead ruling 2026-11-09): a move whose lowest load
/// cap (power, deflection, force budget) sits below the chip-min floor takes
/// the load cap, not the floor. The load caps are hard limits; the floor is
/// a rub and burn advisory. The modulation summary states this note so the
/// operator sees the rub risk; the chip gate grades the low side too.
pub const LOAD_CAP_BELOW_CHIP_FLOOR_NOTE: &str = "load cap below the chip floor: rub risk";

/// Feeds contract P16 item 7 (D6): the force-budget cap inputs of the
/// constrained-max solver. The cap is the advance per tooth where the PEAK
/// engaged-edge force of the move's one engagement meets the machine force
/// budget (`Engagement::fz_cap_for_peak_force`). It is the inverse of the
/// depth that the depth gate solves at a fixed chip
/// (`feeds::force_budget::rigidity_depth_cap`): one law,
/// `Engagement::edge_force`.
///
/// Build it only through [`ForceBudgetScope::of`], so the modulator and the
/// depth gate read the one rule (`RigidityProfile::depth_cap_of` of the
/// body depth limit, `OperationType::body_depth_limit`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForceBudgetLimitInputs {
    /// The material's force line (the line alone, as the depth gate reads
    /// it: no grain factor).
    pub line: crate::material::force_line::ForceLine,
    /// The cutter facts of the engagement.
    pub cutter: ModulationCutter,
    /// The machine force budget (N), `RigidityProfile::force_budget_n`.
    pub budget_n: f64,
}

/// Feeds contract P16 item 7: whether the force-budget limit applies to an
/// op, from the one rule the depth gate reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ForceBudgetScope {
    /// The op has no depth-cap rule (a one-sided finish pass, a drill): no
    /// limit.
    OutOfScope,
    /// The op's rule is the force budget, but the material has no force
    /// line: no limit, and the move summary states
    /// [`FORCE_BUDGET_NO_FORCE_LINE_NOTE`].
    NoForceLine,
    /// The op's rule is the force budget, with a force line: the limit.
    Limit(ForceBudgetLimitInputs),
}

impl ForceBudgetScope {
    /// **The one scope rule.** The limit applies only where
    /// `rule == Some(DepthCapRule { budget_n })`, the rule the depth
    /// gate reads (`RigidityProfile::depth_cap_of(kind.body_depth_limit())`).
    #[must_use]
    pub fn of(
        rule: Option<crate::machine::DepthCapRule>,
        line: Option<crate::material::force_line::ForceLine>,
        cutter: ModulationCutter,
    ) -> Self {
        use crate::machine::DepthCapRule;
        match (rule, line) {
            (Some(DepthCapRule { budget_n }), Some(line)) => Self::Limit(ForceBudgetLimitInputs {
                line,
                cutter,
                budget_n,
            }),
            (Some(DepthCapRule { .. }), None) => Self::NoForceLine,
            (None, _) => Self::OutOfScope,
        }
    }

    /// The limit inputs, if the limit applies.
    #[must_use]
    pub fn inputs(self) -> Option<ForceBudgetLimitInputs> {
        match self {
            Self::Limit(inputs) => Some(inputs),
            Self::OutOfScope | Self::NoForceLine => None,
        }
    }

    /// The move summary note, if the scope has one.
    #[must_use]
    pub fn note(self) -> Option<&'static str> {
        match self {
            Self::NoForceLine => Some(FORCE_BUDGET_NO_FORCE_LINE_NOTE),
            Self::OutOfScope | Self::Limit(_) => None,
        }
    }
}

/// Static inputs the modulator needs to convert engagement into feed.
///
/// Kept tool/material/machine-agnostic at the struct level so a caller
/// can stage a synthetic context for a unit test without instantiating
/// the full `Tool` / `Material` / `Machine` graph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModulationContext<'a> {
    /// Spindle speed at which the toolpath is commanded (rev/min).
    /// Must be > 0.
    pub spindle_rpm: f64,
    /// Flute count of the active cutter. Must be ≥ 1.
    pub flute_count: u32,
    /// The machine's CUTTING-feed ceiling (F002). It is the
    /// [`CuttingCeilingMmMin`] type, made only by
    /// `MachineProfile::cutting_ceiling`, so the travel rate
    /// (`MachineProfile::max_feed_mm_min`) cannot pose as the cap.
    /// G-FEEDMAX: the session lowers it to the op's
    /// `DressupConfig::feed_max_rate` ([`CuttingCeilingMmMin::capped_at`]),
    /// so no modulated feed exceeds that dial.
    pub cutting_ceiling: CuttingCeilingMmMin,
    /// Rapid-move feed (mm/min) the machine emits for `G0`. Used only
    /// to thread the kinematics integrator correctly; rapids
    /// themselves are not modulated.
    pub rapid_feed_mm_min: f64,
    /// LUT chipload band (mm/tooth). **`None` = bandless**
    /// (wanaka200 IMPLEMENTATION_PLAN work item A, 2026-09-19): a
    /// toolpath whose `(tool, material, op)` tuple resolves no vendor
    /// chipload row (e.g. a V-bit ProjectCurve — the LUT publishes no
    /// V-bit contour rows). Bandless modulation does NO chipload
    /// targeting: the only caps applied are the machine cutting-feed
    /// ceiling and the Phase 3 geometric plunge guard, so a bandless
    /// descent can never escape its operation's plunge rate.
    ///
    /// A printed point ([`ChiploadBand::point`], A2) caps at the point
    /// with no floor.
    pub chipload_band: Option<ChiploadBand>,
    /// Feeds contract P11b2: whether the op thins its chip radially (the
    /// one rule, [`RadialThinning::of`] on the op's feeds style). The band
    /// holds the PEAK chip, so on a thinning op the chipload cap, the floor
    /// and the pin arms are `band x lift x rpm x Z`, with the lift
    /// ([`RadialThinning::lift`]) at the move's modelled engagement. A
    /// finish pass and a drill cycle take lift 1, as Suggest does.
    pub radial_thinning: RadialThinning,
    /// Machine kinematics — drives the `predicted_feeds_for_toolpath`
    /// per-move achievable-velocity cap.
    pub kinematics: &'a MachineKinematics,
    /// F-039 — which algorithm to run.
    pub strategy: ModulationStrategy,
    /// F-039 — the feed scale (default 1.0). The constrained-max solver
    /// multiplies the smallest candidate feed limit by this value, then
    /// applies the chipload-min floor. `1.0` emits at the binding limit;
    /// `0.7` emits at 70 % of it. `BandMid` ignores it.
    ///
    /// A plain multiplier on the modulator's output feed (ruling R4 Q6,
    /// 2026-09-24). No machine field scales it.
    pub feed_scale: f64,
    /// F-039 — optional deflection-cap inputs (see
    /// [`DeflectionLimitInputs`]). `None` disables the constraint.
    pub deflection_inputs: Option<DeflectionLimitInputs>,
    /// F-039 — optional power-cap inputs (see [`PowerLimitInputs`]).
    /// `None` disables the constraint.
    pub power_inputs: Option<PowerLimitInputs>,
    /// Feeds contract P16 item 7 — optional force-budget cap inputs (see
    /// [`ForceBudgetLimitInputs`]). The session sets it from
    /// [`ForceBudgetScope::of`]; `None` disables the constraint.
    pub force_budget_inputs: Option<ForceBudgetLimitInputs>,
    /// Feeds contract P20g — the side table of the sample engagements, one
    /// entry per move ([`MoveSampleEngagements`]). The power, deflection and
    /// force-budget caps of a move are the MINIMUM over its samples. An empty
    /// slice (or a move past its end, or a move with no sample) reads the
    /// mean engagement of [`PerMoveEngagement`], as before P20g.
    pub sample_engagements: &'a [MoveSampleEngagements],
    /// F-039 — per-move axial DOC (mm) at full engagement. Used as
    /// the `axial_doc_mm` term when scaling deflection + power
    /// limits via `engagement.axial_doc_fraction`. Falls back to
    /// the power cutter's nominal diameter when zero/None.
    pub nominal_axial_doc_mm: f64,
    /// Phase 3 (2026-09-07) — the OPERATION's own plunge rate
    /// (mm/min). [`adaptive_feed_modulate`] caps a vertical-dominant
    /// move at this rate whatever its intent tag says.
    ///
    /// A value that is not finite and positive DISABLES the guard: an
    /// operation legitimately carries no plunge rate, and `min(feed,
    /// 0.0)` would stop the machine. Pass `f64::INFINITY` to disable
    /// the guard deliberately (the Phase 3 A/B's control arm).
    pub plunge_rate_mm_min: f64,
}

/// Errors the modulator can return for malformed inputs. Kept small so
/// the workspace's `result_large_err` lint stays happy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModulationError {
    /// `engagements` length didn't match `toolpath.moves.len()`.
    EngagementLengthMismatch,
    /// `spindle_rpm`, `flute_count`, `cutting_ceiling`, or `rapid_feed_mm_min`
    /// was non-positive or non-finite.
    InvalidContext,
}

/// F-039 — outcome of an [`adaptive_feed_modulate`] call.
///
/// Captures both the legacy "how many moves changed" scalar and the
/// per-move `(feed, binding_constraint)` map the new diagnostic
/// surface consumes. The map is keyed by toolpath-local move index;
/// the caller prefixes with `toolpath_id` when stamping onto
/// `SimulationCutTrace::modulated_feeds`.
#[derive(Debug, Clone, Default)]
pub struct ModulationOutcome {
    /// Number of moves whose `feed_rate` this run rewrote (the new feed
    /// differs from the move's feed before the run). The session skips a
    /// rebuild when it is 0. The summary does not read it: its
    /// `moves_touched` compares with the op feed (P20f).
    pub changed: usize,
    /// Per-move map of `(emitted_feed_mm_min, binding_constraint)`
    /// for every move the modulator visited (including no-ops where
    /// the emitted feed matched commanded). Skipped moves (rapids,
    /// drilling, etc.) are absent from the map.
    pub per_move: BTreeMap<usize, (f64, BindingConstraint)>,
    /// The notes of the move summary (P16 item 7:
    /// [`FORCE_BUDGET_NO_FORCE_LINE_NOTE`]). The session adds them; the
    /// summary carries them.
    pub notes: Vec<&'static str>,
}

/// P20f: a move counts as touched when its emitted feed differs from the
/// op feed by more than this (mm/min). The median delta reads the same set.
const TOUCHED_FEED_TOLERANCE_MM_MIN: f64 = 0.5;

impl ModulationOutcome {
    /// Build the F-039 per-toolpath
    /// [`crate::tool_load::ModulationSummary`] from this outcome and
    /// the op feed. The op feed is the one baseline of `moves_touched`
    /// and of the median delta percentage (P20f).
    pub fn build_summary(
        &self,
        commanded_feed_mm_min: f64,
        feed_scale: f64,
        strategy: ModulationStrategy,
    ) -> Option<crate::tool_load::ModulationSummary> {
        if self.per_move.is_empty() {
            return None;
        }
        let moves_total = self.per_move.len();
        // P20f: `moves_touched` and the median read one baseline, the op
        // feed. `self.changed` compares with the move's feed before this
        // run, and a re-simulated toolpath already holds the modulated
        // feeds, so it reads 0 on the second run.
        let mut moves_touched = 0_usize;
        let mut deltas: Vec<f64> = Vec::with_capacity(moves_total);
        let mut binding_counts: BTreeMap<BindingConstraint, usize> = BTreeMap::new();
        for (feed, binding) in self.per_move.values() {
            if (feed - commanded_feed_mm_min).abs() > TOUCHED_FEED_TOLERANCE_MM_MIN {
                moves_touched += 1;
                if commanded_feed_mm_min > 0.0 {
                    deltas.push((feed - commanded_feed_mm_min) / commanded_feed_mm_min * 100.0);
                }
            }
            *binding_counts.entry(*binding).or_insert(0) += 1;
        }
        deltas.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = if deltas.is_empty() {
            0.0
        } else {
            #[expect(
                clippy::indexing_slicing,
                reason = "deltas.len() > 0 verified just above."
            )]
            {
                let mid = deltas.len() / 2;
                if deltas.len() % 2 == 1 {
                    deltas[mid]
                } else {
                    (deltas[mid - 1] + deltas[mid]) / 2.0
                }
            }
        };
        let total = moves_total.max(1) as f64;
        let binding_constraint_distribution: BTreeMap<BindingConstraint, f64> = binding_counts
            .into_iter()
            .filter(|(_, c)| *c > 0)
            .map(|(k, c)| (k, c as f64 / total))
            .collect();
        Some(crate::tool_load::ModulationSummary {
            moves_touched,
            moves_total,
            median_feed_delta_pct: median,
            binding_constraint_distribution,
            feed_scale,
            strategy: strategy.tag(),
            notes: self.notes.iter().map(|n| (*n).to_owned()).collect(),
        })
    }
}

/// Return `true` when the modulator should leave this move's feed alone.
// EXEMPT(M2)
fn should_skip_modulation(move_type: MoveType, intent: MoveIntent) -> bool {
    if matches!(move_type, MoveType::Rapid) {
        return true;
    }
    matches!(
        intent,
        // EXEMPT(P28-30p)
        MoveIntent::Retract
            | MoveIntent::Drilling
            // EXEMPT(P28-30)
            | MoveIntent::EntryPlunge
            // K4: the air lead above an entry keeps its feed, as it did
            // when it was an `EntryPlunge`.
            | MoveIntent::EntryApproach
            // F-040: lead-in / lead-out feeds are operator-tuned (cleaner
            // entry mark / faster chip-clear on exit). Treat them as
            // user-tuned and skip modulation — the dressup's
            // `lead_in_feed_rate` / `lead_out_feed_rate` already carries
            // the operator's intent for these moves.
            // EXEMPT(P28-29)
            | MoveIntent::LeadIn
            | MoveIntent::LeadOut
            // K3: the obstacle step adds this rapid after the modulator;
            // the modulator never changes it.
            | MoveIntent::ObstacleClearance
    )
}

/// F-039 — Constrained-max per-move solver.
///
/// Computes seven candidate feed caps; the smallest binds. The binding
/// constraint is returned so the diagnostic surface can name *why*
/// the move's feed landed where it did. `ctx.feed_scale` multiplies the
/// minimum *before* the chipload-min floor; values above 1.0 push past
/// the constraint and the chipload-min floor still applies.
///
/// A2 (point mode): for a point band ([`ChiploadBand::is_point`]) the
/// chipload cap is `v × rpm × flutes`, the pin arms pin to `v`, and the
/// step-7 floor is skipped. The feed scale still applies.
///
/// P20g: the third value is `true` when a load cap took the move below the
/// chip-min floor (the rub risk the summary notes).
fn constrained_max_move(
    engagement: PerMoveEngagement,
    move_samples: Option<&MoveSampleEngagements>,
    band: ChiploadBand,
    ctx: &ModulationContext<'_>,
    predicted_cap_mm_min: f64,
) -> (f64, BindingConstraint, bool) {
    let flutes = ctx.flute_count.max(1) as f64;

    // Effective WOC fraction: clamped to a small floor because the deflection
    // and power limits below divide by it. The simulator's air-cut samples are
    // gated out upstream; this protects against single-sample noise.
    let woc_eff = engagement.radial_woc_fraction.clamp(1e-3, 1.0);

    // Feeds contract P11b2 (rulings L1 and L6): the band holds the PEAK
    // chip, the chip Suggest sizes the feed for. The peak chip is the advance
    // over the radial chip-thinning lift (Garr, the one law
    // `RadialThinning::lift`), so every chip edge of this solver (the cap,
    // the floor, the pin arms) is `band x lift`. The lift is the move's
    // measured lift (the minimum over its samples of the gate's own reading,
    // `PerMoveEngagement::thinning_lift`), else the lift at the modelled
    // width `woc x d_eng` (with no cutter, the fraction on a unit diameter;
    // the lift is a function of `ae / d_eng` alone). G-CHIPTHIN-HALFFIX (2026-08-19) deleted an earlier lift here
    // (`1 / sqrt(woc)`, a second formula); operator ruling D11 restored the
    // claim as one model, so this site reads the one law, not that formula.
    let lift = move_thinning_lift(engagement, ctx, woc_eff);
    let target_chipload = band.max_mm_per_tooth * lift;
    let floor_chipload = band.min_mm_per_tooth * lift;

    let mut limits: Vec<(f64, BindingConstraint)> = Vec::with_capacity(7);
    // The first pin arm that fired (deflection, power or force budget), if any. For a
    // point the pin value equals the chipload cap, so the tie-break below
    // needs it.
    let mut pinned: Option<(f64, BindingConstraint)> = None;
    // P20g (lead ruling 2026-11-09): the lowest load cap of the move (power,
    // deflection, force budget; a pin is not a cap). The chip floor never
    // lifts the feed above it.
    let mut lowest_load_cap: Option<f64> = None;

    // 1. Chipload-max constraint (the peak chip at the band max, P11b2).
    limits.push((
        target_chipload * ctx.spindle_rpm * flutes,
        BindingConstraint::ChiploadMax,
    ));

    // Feeds contract P20g (lead ruling 1, 2026-11-09): a move has one F
    // word, so the spindle and the tool must hold the WORST point of the
    // move. Each load arm below (deflection, power, force budget) is the
    // MINIMUM over the move's samples of that cap at each sample's own
    // engagement, built by the gate's own builder (the side table
    // `ctx.sample_engagements`). A sample whose edge term alone meets the
    // budget pins the move at the chip floor. The mean engagement of the
    // move binds only where the move has no sample engagement for the arm,
    // or where the session flagged `mean_binds` (the planner floor).
    let (sample_list, mean_binds): (&[SampleEngagement], bool) =
        move_samples.map_or((&[], false), |m| (m.samples.as_slice(), m.mean_binds));
    let floor_feed = floor_chipload * ctx.spindle_rpm * flutes;

    // 2. Deflection cap (feed-aware, gate-consistent). The lateral force
    // is the PEAK resultant engaged-edge force (P11a, L5), and tip
    // deflection is linear in force: δ = compliance · F. So `δ ≤ bound` is
    // the force budget `bound / compliance`, and the feed cap is solved
    // from the same force model the post-sim deflection gate uses. P20g:
    // at each sample's engagement (`deflection::sample_engagement`).
    if let Some(defl) = ctx.deflection_inputs {
        let axial_mm = effective_axial_mm(engagement, ctx);
        // One solver, shared with the inspector: the inverse lives on
        // `feeds::engagement::Engagement`, not here. Two copies of one
        // model drift apart.
        let budget_n = if defl.compliance_mm_per_n > 0.0 {
            defl.max_tip_deflection_mm / defl.compliance_mm_per_n
        } else {
            0.0
        };
        // Even zero feed exceeds the bound (the edge floor alone is over
        // budget), or no lateral engagement: feed cannot rescue deflection,
        // so the arm pins to the chipload-min floor and the binding tag
        // names deflection. Dropping DOC/stepover is the real fix (out of
        // scope for per-move feed). For a point (A2) `min == v`, so this
        // pins to the point. No DOC, no compliance, no Ks: no signal, no
        // cap, and no claim on the binding tag.
        let arm = fold_load_arm(
            sample_list.iter().filter_map(|s| s.force),
            mean_binds,
            || Some(defl.cutter.engagement(axial_mm, woc_eff)),
            |e| force_arm_reading(e, defl.line, ctx, budget_n),
        );
        arm.push_into(
            &mut limits,
            &mut pinned,
            &mut lowest_load_cap,
            floor_feed,
            BindingConstraint::DeflectionMax,
        );
    }

    // 3. Power cap. R1 (2026-09-16): power is the two-term affine model
    // `P = A·(Ks·MRR + F_edge·ap·Vc·z·ψ/2π)`, built from the SAME
    // `(Ks, F_edge)` pair the deflection cap above consumes. One solver
    // again: the model lives in `tool_load::power`, not here.
    //
    // Only the shear term moves with feed, so the cap is an affine
    // inversion, not a ratio — `PowerTerms::feed_for_kw` does it. Power is
    // linear in feed with one budget, so the minimum over the samples of
    // `feed_for_kw` is exact (P20g). The sample engagement is the power
    // gate's own (`Engagement::from_sample`).
    if let Some(pow) = ctx.power_inputs
        && pow.available_kw > 0.0
    {
        let axial_mm = effective_axial_mm(engagement, ctx);
        // P11a: the engaged diameter at the move depth.
        let radial_width = woc_eff * pow.cutter.d_eng_at(axial_mm).max(0.0);
        let mean_modelled = axial_mm > 0.0 && woc_eff > 0.0 && radial_width > 0.0;
        // K7 (feeds contract P9): the one engagement of the cut and the
        // cutter's own section, so the cap, Suggest and the gate read one
        // power for one cut. `PowerTerms::of` applies the grain factor of
        // the line (S2-9).
        //
        // The feed-free edge term alone already meets the power budget, so
        // no feed rescues it — the same shape as `EdgeForceOverBudget` on
        // the deflection side, handled the same way: pin to the
        // chipload-min floor and let the binding tag name power.
        let arm = fold_load_arm(
            sample_list
                .iter()
                .filter_map(|s| s.power)
                .filter(|e| e.ap_mm() > 0.0),
            mean_binds,
            || mean_modelled.then(|| pow.cutter.engagement(axial_mm, woc_eff)),
            |e| {
                let terms = crate::tool_load::power::PowerTerms::of(e.power_inputs(
                    pow.line,
                    ctx.spindle_rpm,
                    flutes,
                ));
                match terms.feed_for_kw(pow.available_kw) {
                    Some(cap) => ArmReading::Cap(cap),
                    None => ArmReading::Pin,
                }
            },
        );
        arm.push_into(
            &mut limits,
            &mut pinned,
            &mut lowest_load_cap,
            floor_feed,
            BindingConstraint::PowerMax,
        );
    }

    // 4. Force-budget cap (feeds contract P16 item 7, D6). The same shape
    // as the deflection cap: the fz where the PEAK engaged-edge force meets
    // the machine force budget. It is the inverse of the depth the depth
    // gate solves at a fixed chip (`feeds::force_budget::rigidity_depth_cap`),
    // one law (`Engagement::edge_force`). The session sets the inputs only
    // where the depth gate applies the force budget (`ForceBudgetScope::of`).
    // P20g: at each sample's engagement, the one the force card reads
    // (`cut_force::sample_peak_force_n` through `deflection::sample_engagement`).
    if let Some(fb) = ctx.force_budget_inputs {
        let axial_mm = effective_axial_mm(engagement, ctx);
        let arm = fold_load_arm(
            sample_list.iter().filter_map(|s| s.force),
            mean_binds,
            || Some(fb.cutter.engagement(axial_mm, woc_eff)),
            |e| force_arm_reading(e, fb.line, ctx, fb.budget_n),
        );
        arm.push_into(
            &mut limits,
            &mut pinned,
            &mut lowest_load_cap,
            floor_feed,
            BindingConstraint::ForceBudget,
        );
    }

    // 5. Machine-feed hard cap.
    limits.push((
        ctx.cutting_ceiling.mm_min(),
        BindingConstraint::MachineMaxFeed,
    ));

    // 6. Kinematic-reach cap.
    if predicted_cap_mm_min.is_finite() && predicted_cap_mm_min > 0.0 {
        limits.push((predicted_cap_mm_min, BindingConstraint::KinematicReach));
    }

    // Pick the smallest cap.
    let (limit, binding) = limits
        .into_iter()
        .filter(|(v, _)| v.is_finite() && *v > 0.0)
        .min_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or((
            ctx.cutting_ceiling.mm_min(),
            BindingConstraint::MachineMaxFeed,
        ));

    // A2 (point mode): a pin arm pins to `v`, which is also the chipload
    // cap, so the two tie. `min_by` keeps the first entry of a tie, which
    // is `ChiploadMax`. The pin names the load-bearing constraint (no feed
    // satisfies it), so for a point the pin tag wins a tie. A band pins at
    // `min < max`, so this changes nothing for a band.
    let binding = match pinned {
        Some((pin_feed, pin_tag)) if band.is_point() && pin_feed <= limit => pin_tag,
        _ => binding,
    };

    let scale = ctx.feed_scale.max(0.0);
    let after_scale = limit * scale;

    // A2 (point mode): no floor for a point. Nothing printed a minimum,
    // so the feed scale can take the feed below the point. The binding
    // tag stays the tag of the smallest cap (Q7: a move held at the
    // point keeps `ChiploadMax`).
    if band.is_point() {
        return (after_scale, binding, false);
    }

    // 7. Chipload-min floor — the matched row's own `band.min`, NOT
    // `feeds::effective_rubbing_floor` (see `ChiploadBand`). Applied last so
    // the feed scale can't drop feeds below the safe floor. When
    // the floor itself sits above the machine's hard cap (rare:
    // machine `max_feed` configured below `band.min × rpm × flutes`)
    // the cap wins — emitting above the cap would crash the
    // controller, which is strictly worse than burning the
    // workpiece. The machine constraint stays load-bearing.
    //
    // Binding-tag rule (per F-039 spec): re-label as `ChiploadMin`
    // only when the feed scale took the limit BELOW the floor
    // (`emitted == floor && limit > floor`). When the *underlying*
    // limit itself sits below the floor (deflection or power forced
    // a low feed even at feed scale 1.0), keep the original
    // binding so the diagnostic surface names the load-bearing
    // physical constraint, not the floor we backed off to.
    // P11b2: the floor holds the PEAK chip at `band.min` too (a light cut
    // makes a thin chip; the floor is what keeps it off the rubbing side).
    //
    // P20g (lead ruling 2026-11-09): a load cap wins over the floor. Power,
    // deflection and the force budget are hard limits; the floor is a rub and
    // burn advisory. Where the floor sits above the lowest load cap, the
    // floor drops to that cap, the binding stays the load arm, and the move
    // reports `below_floor` (the summary note, the chip gate's low side).
    let floor = floor_chipload * ctx.spindle_rpm * flutes;
    let floor_under_ceiling = floor.min(ctx.cutting_ceiling.mm_min());
    let effective_floor =
        lowest_load_cap.map_or(floor_under_ceiling, |cap| floor_under_ceiling.min(cap));
    let below_floor = effective_floor < floor_under_ceiling;
    if after_scale < effective_floor {
        let new_binding = if limit > effective_floor {
            BindingConstraint::ChiploadMin
        } else {
            binding
        };
        (effective_floor, new_binding, below_floor)
    } else {
        (
            after_scale,
            binding,
            below_floor && after_scale < floor_under_ceiling,
        )
    }
}

/// The test door: [`constrained_max_move`] without the P20g flag.
#[cfg(test)]
fn max_safe_feed_for_move(
    engagement: PerMoveEngagement,
    move_samples: Option<&MoveSampleEngagements>,
    band: ChiploadBand,
    ctx: &ModulationContext<'_>,
    predicted_cap_mm_min: f64,
) -> (f64, BindingConstraint) {
    let (feed, binding, _) =
        constrained_max_move(engagement, move_samples, band, ctx, predicted_cap_mm_min);
    (feed, binding)
}

/// Feeds contract P20g: the reading of one load arm at one engagement.
enum ArmReading {
    /// The feed cap (mm/min) at this engagement.
    Cap(f64),
    /// No feed meets the budget (the edge term alone meets it, or no edge
    /// is engaged): the arm pins the move at the chip floor.
    Pin,
    /// No signal (no depth, no budget, no Ks): no cap and no tag.
    NoSignal,
}

/// Feeds contract P20g: one load arm over the engagements of one move. The
/// lowest cap, and whether any engagement pins the move.
#[derive(Default)]
struct LoadArm {
    cap: Option<f64>,
    pinned: bool,
}

impl LoadArm {
    /// Push the arm's limits: the lowest cap, then the floor pin if any
    /// engagement pinned the move (the pre-P20g order of one engagement).
    fn push_into(
        self,
        limits: &mut Vec<(f64, BindingConstraint)>,
        pinned: &mut Option<(f64, BindingConstraint)>,
        lowest_load_cap: &mut Option<f64>,
        floor_feed: f64,
        tag: BindingConstraint,
    ) {
        if let Some(cap) = self.cap {
            limits.push((cap, tag));
            *lowest_load_cap = Some(lowest_load_cap.map_or(cap, |prev| prev.min(cap)));
        }
        if self.pinned {
            let pin = (floor_feed.max(1e-9), tag);
            *pinned = pinned.or(Some(pin));
            limits.push(pin);
        }
    }
}

/// Feeds contract P20g: fold one load arm over the sample engagements of a
/// move (lead ruling 1: the minimum over the samples). `mean` gives the
/// mean engagement of the move; it binds when no sample engagement exists
/// for the arm, or when `mean_binds` is set (the planner floor).
fn fold_load_arm(
    samples: impl Iterator<Item = Engagement>,
    mean_binds: bool,
    mean: impl FnOnce() -> Option<Engagement>,
    reading: impl Fn(&Engagement) -> ArmReading,
) -> LoadArm {
    let mut arm = LoadArm::default();
    let mut add = |e: &Engagement| match reading(e) {
        ArmReading::Cap(cap) => {
            arm.cap = Some(arm.cap.map_or(cap, |prev: f64| prev.min(cap)));
        }
        ArmReading::Pin => arm.pinned = true,
        ArmReading::NoSignal => {}
    };
    let mut any_sample = false;
    for e in samples {
        any_sample = true;
        add(&e);
    }
    if (!any_sample || mean_binds)
        && let Some(e) = mean()
    {
        add(&e);
    }
    arm
}

/// Feeds contract P20g: the deflection and force-budget arms read one
/// solver, `Engagement::fz_cap_for_peak_force`, against a force budget.
fn force_arm_reading(
    engagement: &Engagement,
    line: crate::material::force_line::ForceLine,
    ctx: &ModulationContext<'_>,
    budget_n: f64,
) -> ArmReading {
    match engagement.fz_cap_for_peak_force(line, ctx.flute_count.max(1), budget_n) {
        Ok(fz_cap) => ArmReading::Cap(fz_cap * ctx.spindle_rpm * f64::from(ctx.flute_count.max(1))),
        Err(
            DeflectionCapRefusal::NoLateralEngagement | DeflectionCapRefusal::EdgeForceOverBudget,
        ) => ArmReading::Pin,
        Err(DeflectionCapRefusal::Unmodelled) => ArmReading::NoSignal,
    }
}

/// Feeds contract P11b2: the radial chip-thinning lift of one move. The
/// measured lift ([`PerMoveEngagement::thinning_lift`], the gate's reading)
/// when the trace carries it, else the lift at the modelled engagement
/// (`woc_eff x d_eng` on the engaged diameter at the move depth, the
/// engagement the force caps build). 1 when the op does not thin (a finish
/// pass, a drill cycle: [`RadialThinning`]).
fn move_thinning_lift(
    engagement: PerMoveEngagement,
    ctx: &ModulationContext<'_>,
    woc_eff: f64,
) -> f64 {
    // The op does not thin: no lift, whatever the samples read.
    if ctx.radial_thinning != RadialThinning::Applied {
        return 1.0;
    }
    // The measured lift first (the gate's own reading), else the model.
    if let Some(lift) = engagement
        .thinning_lift
        .filter(|l| l.is_finite() && *l >= 1.0)
    {
        return lift;
    }
    let cutter = ctx
        .power_inputs
        .map(|p| p.cutter)
        .or(ctx.deflection_inputs.map(|d| d.cutter))
        .or(ctx.force_budget_inputs.map(|f| f.cutter));
    let d_eng = cutter
        .map(|c| c.d_eng_at(effective_axial_mm(engagement, ctx)))
        .filter(|d| d.is_finite() && *d > 0.0)
        .unwrap_or(1.0);
    ctx.radial_thinning.lift(woc_eff * d_eng, d_eng)
}

/// The axial depth this move actually cuts, in mm.
///
/// ## T-11 (2026-09-16) — what this used to compute, and why it was wrong
///
/// It used to return `ctx.nominal_axial_doc_mm * engagement.axial_doc_fraction`.
/// That multiplied a depth in mm by a fraction of a DIFFERENT quantity: the
/// dexel writes `axial_doc_fraction` as `axial_engagement_mm / flute_length`
/// (`dexel_stock::simulation`), not as a fraction of the nominal depth. The
/// product was `mm x (mm / flute_length)`, which is not a depth.
///
/// The error factor was `flute_length / nominal_axial_doc_mm`, and it always
/// read too SHALLOW, because a pass is always shallower than the flute. A
/// 2 mm pass on a tool with a 25 mm flute came out at 0.16 mm, 12.5x low.
/// This value scales both the deflection cap and the power cap in the
/// constrained-max solver, so both were far too permissive and the modulator
/// handed out feeds the machine should not have been given. Those feeds reach
/// the post-sim power verdict through `trace.predicted_feeds`
/// (`session::compute` stamps them), so the defect also broke the very
/// gate-versus-modulation agreement that stamping exists to create.
///
/// Every fixture pinned `axial_doc_fraction` at `1.0` — the single value at
/// which the wrong expression returns the right answer — so no test could
/// fail on it. See `planning/TECH_DEBT_REGISTER.md` T-11.
///
/// It now reads the millimetre measurement the dexel already took, which is
/// what `Engagement::axial_doc_fraction`'s own doc comment directs a caller
/// to do.
fn effective_axial_mm(engagement: PerMoveEngagement, ctx: &ModulationContext<'_>) -> f64 {
    let measured = engagement.axial_doc_mm;
    if measured > 0.0 {
        return measured;
    }
    // No per-move measurement. Fall back to the toolpath's nominal depth,
    // which `session::compute` derives as the maximum measured axial
    // engagement over the toolpath's cutting samples — a real reading, not a
    // fabricated one.
    let nominal = ctx.nominal_axial_doc_mm.max(0.0);
    if nominal > 0.0 {
        return nominal;
    }
    // Neither a per-move nor a per-toolpath axial signal exists, which means
    // the trace carried no cutting samples for this path. Returning the
    // engagement diameter here is a proxy, not a measurement: it keeps the
    // pre-T-11 behaviour for legacy and analytical traces rather than
    // silently disabling both caps, and the caller treats a zero as an air
    // move. Either cap's nominal diameter serves.
    ctx.power_inputs
        .map(|p| p.cutter.diameter_mm)
        .or(ctx.deflection_inputs.map(|d| d.cutter.diameter_mm))
        .or(ctx.force_budget_inputs.map(|f| f.cutter.diameter_mm))
        .unwrap_or(0.0)
}

/// Feeds contract P18 (review D15): the feed of a ConstrainedMax move with
/// no engagement, and of a bandless move (no chipload band). The commanded
/// feed, under the cutting ceiling
/// (`MachineMaxFeed`) and under the predicted kinematic cap
/// (`KinematicReach`, the cap of every engaged arm). It only lowers.
fn no_engagement_feed(
    commanded_mm_min: f64,
    cutting_ceiling_mm_min: f64,
    predicted_cap_mm_min: f64,
) -> (f64, BindingConstraint) {
    let under_ceiling = commanded_mm_min.min(cutting_ceiling_mm_min);
    if predicted_cap_mm_min.is_finite()
        && predicted_cap_mm_min > 0.0
        && predicted_cap_mm_min < under_ceiling
    {
        (predicted_cap_mm_min, BindingConstraint::KinematicReach)
    } else {
        (under_ceiling, BindingConstraint::MachineMaxFeed)
    }
}

/// F-036 — "target band-mid" per-move feed (legacy heuristic).
///
/// A2 (point mode): for a point band the target and the ceiling are
/// `v × rpm × flutes`, and no floor applies. The kinematic cap is then
/// not raised to a floor.
fn band_mid_feed_for_move(
    commanded_feed_mm_min: f64,
    engagement: PerMoveEngagement,
    predicted_cap_mm_min: f64,
    ctx: &ModulationContext<'_>,
    band: ChiploadBand,
) -> (f64, BindingConstraint) {
    if engagement.radial_woc_fraction <= 1e-6 && engagement.axial_doc_fraction <= 1e-6 {
        return (commanded_feed_mm_min, BindingConstraint::ChiploadMax);
    }
    let woc = engagement.radial_woc_fraction.clamp(1e-3, 1.0);
    // Feeds contract P14 (review M2): the modulator's one thinning rule
    // (Garr; 1 on a finish pass or a drill cycle).
    let target_chipload = band.mid_mm_per_tooth() * move_thinning_lift(engagement, ctx, woc);
    let flutes = ctx.flute_count.max(1) as f64;
    let base_feed = target_chipload * ctx.spindle_rpm * flutes;

    if band.is_point() {
        // A2: the target and the ceiling are the point. No floor, and no
        // chip-thinning lift (the ceiling is the point, so a lift above it
        // is always clamped back).
        let ceiling = band.max_mm_per_tooth * ctx.spindle_rpm * flutes;
        let feed = ceiling
            .min(ctx.cutting_ceiling.mm_min())
            .min(predicted_cap_mm_min)
            .max(0.0);
        let binding = if (feed - ctx.cutting_ceiling.mm_min()).abs() < 1e-6 {
            BindingConstraint::MachineMaxFeed
        } else if (feed - ceiling).abs() < 1e-6 {
            BindingConstraint::ChiploadMax
        } else if (feed - predicted_cap_mm_min).abs() < 1e-6 {
            BindingConstraint::KinematicReach
        } else {
            BindingConstraint::ChiploadMax
        };
        return (feed, binding);
    }

    let band_floor = band.min_mm_per_tooth * ctx.spindle_rpm * flutes;
    let band_ceiling = band.max_mm_per_tooth * ctx.spindle_rpm * flutes;

    let cap = ctx
        .cutting_ceiling
        .mm_min()
        .min(band_ceiling)
        .min(predicted_cap_mm_min.max(band_floor));
    if cap < band_floor {
        return (cap.max(0.0), BindingConstraint::MachineMaxFeed);
    }
    let clamped = base_feed.clamp(band_floor, cap);
    // Machine-cap check comes first: when the machine cap ties the band
    // ceiling (or another candidate) at the same clamped value, the feed
    // is genuinely machine-limited and the diagnostic should say so
    // rather than falling through to the first tied arm (ChiploadMax).
    let binding = if (clamped - ctx.cutting_ceiling.mm_min()).abs() < 1e-6 {
        BindingConstraint::MachineMaxFeed
    } else if (clamped - band_ceiling).abs() < 1e-6 {
        BindingConstraint::ChiploadMax
    } else if (clamped - band_floor).abs() < 1e-6 {
        BindingConstraint::ChiploadMin
    } else if (clamped - predicted_cap_mm_min).abs() < 1e-6 {
        BindingConstraint::KinematicReach
    } else {
        BindingConstraint::ChiploadMax
    };
    (clamped, binding)
}

/// Apply per-move adaptive feed modulation to `toolpath`.
///
/// Strategy selection: [`ModulationContext::strategy`].
///
/// Returns a [`ModulationOutcome`] with the per-move binding-
/// constraint map. The map is suitable for stamping onto
/// [`crate::stock::simulation_cut::SimulationCutTrace::modulated_feeds`]
/// after prefixing with the toolpath id.
pub fn adaptive_feed_modulate(
    toolpath: &mut Toolpath,
    engagements: &[PerMoveEngagement],
    ctx: &ModulationContext<'_>,
) -> Result<ModulationOutcome, ModulationError> {
    adaptive_feed_modulate_with_materials(toolpath, engagements, &[], ctx)
}

/// S6 (stock changes, "cut as"): the material of one move for the
/// modulator. The session picks it per move through
/// `SimulationCutTrace::effective_material_for_sample` (the main slot by
/// time weight).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum MoveMaterial {
    /// The toolpath's material: the band and the caps of the
    /// [`ModulationContext`]. Every move of a project with no own-material
    /// stock change.
    #[default]
    Toolpath,
    /// An own material: its band and its caps replace the context's.
    /// Boxed: the caps carry the cutter facts of the engagement (P11a).
    Own(Box<OwnMaterialInputs>),
    /// Not judged (an own material with no force data): no band and no
    /// force cap, so the move keeps its commanded feed, clamped to the
    /// machine ceiling and the plunge guard (the bandless arm).
    NotJudged,
}

/// The band and the caps of an own material, built the way the session
/// builds the toolpath's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OwnMaterialInputs {
    /// The chipload band of the material; `None` = bandless.
    pub chipload_band: Option<ChiploadBand>,
    /// The deflection cap of the material.
    pub deflection_inputs: Option<DeflectionLimitInputs>,
    /// The power cap of the material.
    pub power_inputs: Option<PowerLimitInputs>,
    /// The force-budget cap of the material (P16 item 7).
    pub force_budget_inputs: Option<ForceBudgetLimitInputs>,
}

/// [`adaptive_feed_modulate`] with a material per move (S6). An empty
/// `materials` slice, or a move past its end, is [`MoveMaterial::Toolpath`].
pub fn adaptive_feed_modulate_with_materials(
    toolpath: &mut Toolpath,
    engagements: &[PerMoveEngagement],
    materials: &[MoveMaterial],
    ctx: &ModulationContext<'_>,
) -> Result<ModulationOutcome, ModulationError> {
    if engagements.len() != toolpath.moves.len() {
        return Err(ModulationError::EngagementLengthMismatch);
    }
    if !ctx.spindle_rpm.is_finite()
        || ctx.spindle_rpm <= 0.0
        || ctx.flute_count == 0
        || !ctx.cutting_ceiling.mm_min().is_finite()
        || ctx.cutting_ceiling.mm_min() <= 0.0
        || !ctx.rapid_feed_mm_min.is_finite()
        || ctx.rapid_feed_mm_min <= 0.0
    {
        return Err(ModulationError::InvalidContext);
    }

    // Predicted achievable feed per move under the machine's accel /
    // junction limits. As in F-036b, rebuild a synthetic toolpath
    // commanded at the maximum candidate feed so the integrator
    // returns the geometric reach, not commanded-clipped reach.
    // Bandless: probe at the machine ceiling — there is no band ceiling to
    // fold in. P11b2: the band ceiling is `band.max x lift x rpm x Z` at the
    // largest lift of the path (the lift has no cap, L3), so a probe at the
    // un-lifted band ceiling would clip the lift through the kinematic-reach
    // cap. A path that does not thin probes where it did before P11b2.
    let max_lift = engagements
        .iter()
        // A move with no engagement keeps its feed (the solver skips it).
        .filter(|e| e.radial_woc_fraction > 1e-6 || e.axial_doc_fraction > 1e-6)
        .map(|e| move_thinning_lift(*e, ctx, e.radial_woc_fraction.clamp(1e-3, 1.0)))
        .filter(|l| l.is_finite())
        .fold(1.0_f64, f64::max);
    let band_ceiling_feed = ctx
        .chipload_band
        .map_or(ctx.cutting_ceiling.mm_min(), |band| {
            band.max_mm_per_tooth * max_lift * ctx.spindle_rpm * ctx.flute_count.max(1) as f64
        });
    let probe_feed = ctx
        .cutting_ceiling
        .mm_min()
        .min(band_ceiling_feed)
        .max(1e-3);
    let mut probe = toolpath.clone();
    for m in probe.moves.iter_mut() {
        m.move_type = m.move_type.with_feed_rate(probe_feed);
    }
    let predicted = predicted_feeds_for_toolpath(
        &probe,
        ctx.kinematics,
        ctx.cutting_ceiling.mm_min(),
        ctx.rapid_feed_mm_min,
    );
    // Feeds contract P18 (review D15): the reach of a move with no
    // engagement. The probe above runs at the band ceiling, which is a chip
    // limit, not a machine limit; a move with no engagement has no chip, so
    // its reach comes from a probe at the cutting ceiling. When the two
    // probe feeds are equal, the first probe serves.
    // A bandless move (an own-material move with no band, or a not-judged
    // move) has no chip either, so it reads the same probe.
    let no_engagement_fed = engagements.iter().zip(&toolpath.moves).any(|(e, m)| {
        e.radial_woc_fraction <= 1e-6
            && e.axial_doc_fraction <= 1e-6
            && m.move_type.feed_rate().is_some()
    });
    let bandless_move = materials.iter().any(|m| match m {
        MoveMaterial::Toolpath => false,
        MoveMaterial::Own(own) => own.chipload_band.is_none(),
        MoveMaterial::NotJudged => true,
    });
    let reach_at_ceiling =
        if (no_engagement_fed || bandless_move) && probe_feed < ctx.cutting_ceiling.mm_min() {
            let mut at_ceiling = toolpath.clone();
            for m in at_ceiling.moves.iter_mut() {
                m.move_type = m.move_type.with_feed_rate(ctx.cutting_ceiling.mm_min());
            }
            Some(predicted_feeds_for_toolpath(
                &at_ceiling,
                ctx.kinematics,
                ctx.cutting_ceiling.mm_min(),
                ctx.rapid_feed_mm_min,
            ))
        } else {
            None
        };

    let mut outcome = ModulationOutcome::default();
    // P20g: set when any move took a load cap below its chip floor.
    let mut below_chip_floor = false;
    #[expect(
        clippy::indexing_slicing,
        reason = "`i` bounded by `engagements.len()`, which we just verified equals `toolpath.moves.len()`. The Phase 3 guard also reads `moves[i - 1]`, under its own `i > 0` test."
    )]
    for (i, &engagement) in engagements.iter().enumerate() {
        let move_intent = toolpath.moves[i].intent;
        let move_type = toolpath.moves[i].move_type;
        if should_skip_modulation(move_type, move_intent) {
            continue;
        }
        let Some(commanded) = move_type.feed_rate() else {
            continue;
        };
        let predicted_cap = predicted
            .get(&i)
            .copied()
            .unwrap_or(ctx.cutting_ceiling.mm_min())
            .max(1e-3);

        // S6: an own-material move reads its own band and caps; a
        // not-judged move reads none. The default reads the context.
        let move_ctx_storage;
        let ctx: &ModulationContext<'_> = match materials.get(i) {
            None | Some(MoveMaterial::Toolpath) => ctx,
            Some(MoveMaterial::Own(own)) => {
                move_ctx_storage = ModulationContext {
                    chipload_band: own.chipload_band,
                    deflection_inputs: own.deflection_inputs,
                    power_inputs: own.power_inputs,
                    force_budget_inputs: own.force_budget_inputs,
                    ..*ctx
                };
                &move_ctx_storage
            }
            Some(MoveMaterial::NotJudged) => {
                move_ctx_storage = ModulationContext {
                    chipload_band: None,
                    deflection_inputs: None,
                    power_inputs: None,
                    force_budget_inputs: None,
                    ..*ctx
                };
                &move_ctx_storage
            }
        };

        let (mut new_feed, mut binding) = match (ctx.strategy, ctx.chipload_band) {
            // BANDLESS (wanaka200 IMPLEMENTATION_PLAN work item A,
            // 2026-09-19): no vendor chipload row, so no chipload
            // targeting — clamp the commanded feed to the machine
            // cutting ceiling only, then fall through to the geometric
            // plunge guard below. This is the arm a V-bit ProjectCurve
            // reaches: its 20° V point previously descended at the
            // full lateral commanded feed (1165 mm/min against a 400
            // mm/min plunge rate) because the guard lived downstream of
            // a band gate it could never pass. Same binding tag as the
            // banded no-engagement arm, for consistency.
            //
            // Feeds contract P18 (review D15, lead ruling): the reach cap at
            // the cutting ceiling binds here too, by the rule of the
            // no-engagement arm (`no_engagement_feed`). A bandless toolpath
            // probes at the ceiling, so its `predicted_cap` is that reach.
            (_, None) => no_engagement_feed(
                commanded,
                ctx.cutting_ceiling.mm_min(),
                reach_at_ceiling
                    .as_ref()
                    .and_then(|r| r.get(&i).copied())
                    .map_or(predicted_cap, |r| r.max(1e-3)),
            ),
            (ModulationStrategy::BandMid, Some(band)) => {
                band_mid_feed_for_move(commanded, engagement, predicted_cap, ctx, band)
            }
            (ModulationStrategy::ConstrainedMax, Some(band)) => {
                if engagement.radial_woc_fraction <= 1e-6 && engagement.axial_doc_fraction <= 1e-6 {
                    // No engagement — leave the commanded feed
                    // alone, under the ceiling (G-FEEDMAX: the ceiling
                    // holds the op's `feed_max_rate`, a hard cap of every
                    // arm). Record the per-move entry so the diagnostic
                    // surface can still see it.
                    //
                    // Feeds contract P18 (review D15, lead ruling option
                    // 1): the predicted kinematic cap binds here too, as in
                    // every engaged arm. A move that the simulation sees
                    // with no engagement can still be a real cut that the
                    // cell cannot resolve (P_thin_005: a 0.3 mm width on a
                    // 0.5 mm cell). The machine cannot reach a feed above
                    // the cap on that move, so the emitted feed is the cap.
                    let reach = reach_at_ceiling
                        .as_ref()
                        .and_then(|r| r.get(&i).copied())
                        .map_or(predicted_cap, |r| r.max(1e-3));
                    no_engagement_feed(commanded, ctx.cutting_ceiling.mm_min(), reach)
                } else {
                    let (feed, binding, below) = constrained_max_move(
                        engagement,
                        ctx.sample_engagements.get(i),
                        band,
                        ctx,
                        predicted_cap,
                    );
                    // P20g: a load cap took the move below the chip floor.
                    below_chip_floor |= below;
                    (feed, binding)
                }
            }
        };

        // Phase 3 (2026-09-07) — the GEOMETRIC plunge guard.
        //
        // `should_skip_modulation` above protects a plunge that carries a
        // plunge INTENT. The adaptive3d rough emits its step-down and
        // re-entry descents as plain cutting moves, so they fall through
        // that filter and every branch above lifts them to the lateral
        // chipload band. On the wanaka front rough that produced 1807
        // mm/min of pure-vertical descent against a 512 mm/min plunge
        // rate. Intent-keyed protection; untagged geometry escaped it.
        //
        // The guard reads the move's own vector through
        // `kinematic_utilization::classify_move` — the ONE construction
        // site, shared with the Phase 2 instrument, so the guard and the
        // instrument can never disagree about what a plunge is.
        //
        // It is a FLOOR UNDER THE LIFT, not a skip: a move already at or
        // below the plunge rate keeps the strategy's binding, and a tagged
        // `EntryPlunge` never reaches here at all.
        if i > 0 && ctx.plunge_rate_mm_min.is_finite() && ctx.plunge_rate_mm_min > 1e-9 {
            let prev = toolpath.moves[i - 1].target;
            let curr = toolpath.moves[i].target;
            let delta = [curr.x - prev.x, curr.y - prev.y, curr.z - prev.z];
            if crate::machine::kinematic_utilization::classify_move(move_type, delta)
                == crate::machine::kinematic_utilization::MotionClass::Plunge
                && new_feed > ctx.plunge_rate_mm_min
            {
                new_feed = ctx.plunge_rate_mm_min;
                binding = BindingConstraint::PlungeRate;
            }
        }

        // C38 §15: the move's own feed cap (`Move::feed_cap`), which a
        // planner sets where a load rule lowers the feed. It binds after
        // every arm, the floor and the plunge guard, and it only lowers.
        // With no cap (every shipped planner) nothing changes.
        if let Some(cap) = toolpath.moves[i].feed_cap
            && new_feed > cap
        {
            new_feed = cap;
            binding = BindingConstraint::FeedCap;
        }

        // P20c: the per-axis rate on the move's own direction binds last.
        // The engaged arms already read it through `predicted_cap`, but the
        // chipload-min floor can lift a move above that reach; no floor
        // lifts a feed above a rate the controller clamps to.
        if i > 0 {
            let bound = crate::machine::kinematics::rate_bound_feed_of_move(
                &toolpath.moves[i - 1].target,
                &toolpath.moves[i].target,
                move_type,
                new_feed,
                ctx.kinematics,
            );
            if bound < new_feed {
                new_feed = bound;
                binding = BindingConstraint::KinematicReach;
            }
        }

        outcome.per_move.insert(i, (new_feed, binding));
        if (new_feed - commanded).abs() > 1e-6 {
            toolpath.moves[i].move_type = move_type.with_feed_rate(new_feed);
            outcome.changed += 1;
        }
    }
    if below_chip_floor {
        outcome.notes.push(LOAD_CAP_BELOW_CHIP_FLOOR_NOTE);
    }
    Ok(outcome)
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;
    use crate::geo::P3;
    use crate::toolpath::Toolpath;

    fn shapeoko() -> MachineKinematics {
        MachineKinematics::shapeoko_xxl_stock()
    }

    /// A cutting ceiling of `mm_min`, made the one way: a profile whose
    /// travel rate and explicit cutting cap are both `mm_min` (F002).
    fn ceiling(mm_min: f64) -> CuttingCeilingMmMin {
        crate::machine::MachineProfile {
            max_feed_mm_min: mm_min,
            max_cutting_feed_mm_min: Some(mm_min),
            ..crate::machine::MachineProfile::default()
        }
        .cutting_ceiling()
    }

    fn make_ctx<'a>(k: &'a MachineKinematics, band: ChiploadBand) -> ModulationContext<'a> {
        ModulationContext {
            spindle_rpm: 18_000.0,
            flute_count: 2,
            cutting_ceiling: ceiling(4000.0),
            rapid_feed_mm_min: 5000.0,
            chipload_band: Some(band),
            radial_thinning: RadialThinning::Applied,
            kinematics: k,
            strategy: ModulationStrategy::BandMid,
            feed_scale: 1.0,
            deflection_inputs: None,
            power_inputs: None,
            force_budget_inputs: None,
            sample_engagements: &[],
            nominal_axial_doc_mm: 0.0,
            // A realistic operation plunge rate. Every fixture in this
            // module cuts laterally or ramps at 45 degrees, so the Phase 3
            // guard classifies nothing here as a plunge and no expectation
            // below moves. `tests/plunge_guard_p3.rs` exercises the guard.
            plunge_rate_mm_min: 300.0,
        }
    }

    fn band() -> ChiploadBand {
        ChiploadBand::new(0.02, 0.08).unwrap()
    }

    fn straight_toolpath(n_cuts: usize, feed_mm_min: f64) -> Toolpath {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        for i in 0..n_cuts {
            let x = (i + 1) as f64 * 50.0;
            tp.feed_to_with_intent(P3::new(x, 0.0, -2.0), feed_mm_min, MoveIntent::ClearingCut);
        }
        tp
    }

    fn corner_heavy_toolpath(feed_mm_min: f64) -> Toolpath {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        let mut x = 0.0;
        let mut y = 0.0;
        for i in 0..8 {
            if i % 2 == 0 {
                x += 2.0;
            } else {
                y += 2.0;
            }
            tp.feed_to_with_intent(P3::new(x, y, -2.0), feed_mm_min, MoveIntent::ClearingCut);
        }
        tp
    }

    /// S6: a move's material picks its band and caps. `Toolpath` (and an
    /// empty slice) reads the context; `Own` reads its own band; `NotJudged`
    /// keeps the commanded feed (the bandless arm).
    #[test]
    fn a_move_material_picks_its_band_and_caps_s6() {
        let k = shapeoko();
        let mut ctx = make_ctx(&k, band());
        ctx.strategy = ModulationStrategy::ConstrainedMax;
        ctx.plunge_rate_mm_min = f64::INFINITY;
        let commanded = 1500.0;
        let engaged = PerMoveEngagement {
            radial_woc_fraction: 0.5,
            axial_doc_fraction: 0.1,
            axial_doc_mm: 2.0,
            thinning_lift: None,
        };
        let run = |materials: &[MoveMaterial]| {
            let mut tp = straight_toolpath(3, commanded);
            let engagements = vec![engaged; tp.moves.len()];
            let out = adaptive_feed_modulate_with_materials(&mut tp, &engagements, materials, &ctx)
                .unwrap();
            out.per_move
        };
        let plain = {
            let mut tp = straight_toolpath(3, commanded);
            let engagements = vec![engaged; tp.moves.len()];
            adaptive_feed_modulate(&mut tp, &engagements, &ctx)
                .unwrap()
                .per_move
        };
        let n = plain.len() + 1;
        assert_eq!(run(&[]), plain, "an empty slice is the context");
        assert_eq!(run(&vec![MoveMaterial::Toolpath; n]), plain);

        let low = ChiploadBand::new(0.005, 0.01).unwrap();
        let own = run(&vec![
            MoveMaterial::Own(Box::new(OwnMaterialInputs {
                chipload_band: Some(low),
                deflection_inputs: None,
                power_inputs: None,
                force_budget_inputs: None,
            }));
            n
        ]);
        let not_judged = run(&vec![MoveMaterial::NotJudged; n]);
        assert!(!plain.is_empty());
        for (i, (feed, _)) in &plain {
            let (own_feed, _) = own[i];
            assert!(
                own_feed < *feed,
                "the own band caps lower: {own_feed} vs {feed}"
            );
            let (nj_feed, nj_binding) = not_judged[i];
            assert!(
                (nj_feed - commanded).abs() < 1e-9,
                "not judged keeps F{commanded}"
            );
            assert_eq!(nj_binding, BindingConstraint::MachineMaxFeed);
        }
    }

    #[test]
    fn chipload_band_rejects_invalid() {
        assert!(ChiploadBand::new(0.0, 0.05).is_none());
        assert!(ChiploadBand::new(0.05, 0.02).is_none());
        assert!(ChiploadBand::new(f64::NAN, 0.05).is_none());
        assert!(ChiploadBand::new(-0.01, 0.05).is_none());
    }

    #[test]
    fn chipload_band_mid_is_geometric_mean() {
        let b = ChiploadBand::new(0.02, 0.08).unwrap();
        assert!((b.mid_mm_per_tooth() - 0.04).abs() < 1e-9);
    }

    #[test]
    fn engagement_length_mismatch_errors() {
        let mut tp = straight_toolpath(3, 1500.0);
        let engagements = vec![PerMoveEngagement::default(); tp.moves.len() - 1];
        let k = shapeoko();
        let ctx = make_ctx(&k, band());
        let err = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap_err();
        assert_eq!(err, ModulationError::EngagementLengthMismatch);
    }

    #[test]
    fn invalid_context_errors() {
        let mut tp = straight_toolpath(2, 1500.0);
        let engagements = vec![PerMoveEngagement::default(); tp.moves.len()];
        let k = shapeoko();
        let mut ctx = make_ctx(&k, band());
        ctx.spindle_rpm = 0.0;
        assert_eq!(
            adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap_err(),
            ModulationError::InvalidContext
        );
        let mut ctx = make_ctx(&k, band());
        ctx.flute_count = 0;
        assert_eq!(
            adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap_err(),
            ModulationError::InvalidContext
        );
        let mut ctx = make_ctx(&k, band());
        ctx.cutting_ceiling = ceiling(f64::NAN);
        assert_eq!(
            adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap_err(),
            ModulationError::InvalidContext
        );
    }

    /// Step-3 convergence sentry: the feed-aware deflection cap is the
    /// EXACT inverse of the force model — plugging the capped feed back
    /// through `δ = compliance · F_peak` (the engaged-edge peak force,
    /// P11a) lands on the deflection bound. This is what makes the optimizer and the
    /// post-sim gate agree on a cut (they share the model now); the old
    /// `δ ∝ feed` scaling against a feed-blind reference force could not.
    #[test]
    fn deflection_cap_inverts_the_force_model_onto_the_bound() {
        let k = shapeoko();
        let mut ctx = make_ctx(&k, band());
        ctx.strategy = ModulationStrategy::ConstrainedMax;
        ctx.nominal_axial_doc_mm = 2.0;
        // Long-reach tool where the edge floor fits under budget but the
        // chipload-max feed would overshoot, so the affine SOLVE fires
        // (not the can't-satisfy floor branch).
        let bound = 0.2_f64;
        let line = oak_line();
        // The compliance that puts the bound at the band midpoint chip
        // (0.05 mm), so the solve lands inside the band, under the
        // chipload-max cap and above its floor.
        let at_mid = flat_cutter()
            .engagement(2.0, 1.0)
            .edge_force(line, 0.05, ctx.flute_count)
            .unwrap()
            .peak_resultant_n;
        let compliance = bound / at_mid; // mm/N
        ctx.deflection_inputs = Some(DeflectionLimitInputs {
            line,
            cutter: flat_cutter(),
            compliance_mm_per_n: compliance,
            max_tip_deflection_mm: bound,
        });
        // Full slot, full axial: woc = 1, ap = 2.
        let engagement = PerMoveEngagement {
            radial_woc_fraction: 1.0,
            axial_doc_fraction: 1.0,
            // T-11: the depth is now read in mm, not derived from the
            // fraction. 2.0 is what `nominal x fraction` meant here.
            axial_doc_mm: 2.0,
            thinning_lift: None,
        };
        // Large kinematic cap so deflection is the binding constraint.
        let (feed_cap, binding) =
            max_safe_feed_for_move(engagement, None, ctx.chipload_band.unwrap(), &ctx, 1.0e9);
        assert_eq!(
            binding,
            BindingConstraint::DeflectionMax,
            "deflection should bind on this long-reach tool; got {binding:?} at feed {feed_cap}"
        );
        // Plug the capped feed back through the engaged-edge force (P11a)
        // and the compliance.
        let flutes = ctx.flute_count as f64;
        let fz = feed_cap / (ctx.spindle_rpm * flutes);
        let force = flat_cutter()
            .engagement(2.0, 1.0)
            .edge_force(line, fz, ctx.flute_count)
            .unwrap()
            .peak_resultant_n;
        let delta_mm = compliance * force;
        assert!(
            (delta_mm - bound).abs() < 1.0e-9,
            "capped feed must invert onto the deflection bound: δ={delta_mm:.6} mm vs bound {bound} \
             (feed {feed_cap:.1}, fz {fz:.4})"
        );
    }

    /// The white oak force line (Curti 2021, ruling B6).
    fn oak_line() -> crate::material::force_line::ForceLine {
        crate::material::Material::SolidWood {
            species: crate::material::WoodSpecies::WhiteOak,
        }
        .force_line()
        .unwrap()
    }

    /// A 6 mm flat end mill with a 30 degree helix.
    fn flat_cutter() -> ModulationCutter {
        ModulationCutter {
            geometry: crate::feeds::ToolGeometryHint::Flat,
            diameter_mm: 6.0,
            max_cut_diameter_mm: 6.0,
            helix_deg: 30.0,
        }
    }

    /// All-air engagement: feed must stay byte-identical (no modulation
    /// triggered). Invariant for both strategies.
    #[test]
    fn zero_engagement_leaves_feed_unchanged() {
        for strategy in [
            ModulationStrategy::BandMid,
            ModulationStrategy::ConstrainedMax,
        ] {
            let mut tp = straight_toolpath(4, 1500.0);
            let engagements = vec![PerMoveEngagement::default(); tp.moves.len()];
            let k = shapeoko();
            let mut ctx = make_ctx(&k, band());
            ctx.strategy = strategy;
            let outcome = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
            assert_eq!(
                outcome.changed, 0,
                "no engagement should produce no modulation under {strategy:?}"
            );
            for m in &tp.moves {
                if let Some(f) = m.move_type.feed_rate() {
                    assert!(
                        (f - 1500.0).abs() < 1e-9,
                        "feed unchanged under {strategy:?}: got {f}"
                    );
                }
            }
        }
    }

    /// Skipped intents (Retract, Drilling, EntryPlunge) keep their
    /// commanded feed regardless of engagement summary or strategy.
    #[test]
    fn skipped_intents_keep_commanded_feed() {
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(0.0, 0.0, 0.0));
        tp.feed_to_with_intent(P3::new(0.0, 0.0, -3.0), 300.0, MoveIntent::EntryPlunge);
        tp.feed_to_with_intent(P3::new(0.0, 0.0, -6.0), 300.0, MoveIntent::Drilling);
        tp.feed_to_with_intent(P3::new(0.0, 0.0, 5.0), 1500.0, MoveIntent::Retract);
        let engagements = vec![
            PerMoveEngagement::default(),
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
                thinning_lift: None,
            },
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
                thinning_lift: None,
            },
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
                thinning_lift: None,
            },
        ];
        let k = shapeoko();
        let ctx = make_ctx(&k, band());
        let outcome = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        assert_eq!(outcome.changed, 0);
        assert!((tp.moves[1].move_type.feed_rate().unwrap() - 300.0).abs() < 1e-9);
        assert!((tp.moves[2].move_type.feed_rate().unwrap() - 300.0).abs() < 1e-9);
        assert!((tp.moves[3].move_type.feed_rate().unwrap() - 1500.0).abs() < 1e-9);
    }

    /// BandMid: full-slot engagement → mid_band × RPM × flutes.
    #[test]
    fn band_mid_full_slot_targets_mid_band_feed() {
        let mut tp = straight_toolpath(3, 1500.0);
        let engagements = vec![
            PerMoveEngagement::default(),
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
                thinning_lift: None,
            },
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
                thinning_lift: None,
            },
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
                thinning_lift: None,
            },
        ];
        let k = shapeoko();
        let mut ctx = make_ctx(&k, band());
        ctx.strategy = ModulationStrategy::BandMid;
        let outcome = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        assert!(outcome.changed >= 1);
        for m in &tp.moves {
            if !matches!(m.move_type, MoveType::Rapid) {
                let f = m.move_type.feed_rate().unwrap();
                // 0.04 × 18000 × 2 = 1440.
                assert!((f - 1440.0).abs() < 5.0, "expected ~1440, got {f}");
            }
        }
    }

    /// P20f: `moves_touched` and the median read one baseline, the op
    /// feed. A second run over the modulated toolpath rewrites no move,
    /// but its summary still counts the moves off the op feed.
    #[test]
    fn a_remodulated_toolpath_keeps_its_touched_count_p20f() {
        let mut tp = straight_toolpath(3, 1500.0);
        let engagements = vec![
            PerMoveEngagement::default(),
            full_slot(),
            full_slot(),
            full_slot(),
        ];
        let k = shapeoko();
        let mut ctx = make_ctx(&k, band());
        ctx.strategy = ModulationStrategy::BandMid;
        let first = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        let second = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        assert_eq!(second.changed, 0, "the second run rewrites no move");
        let s1 = first
            .build_summary(1500.0, 1.0, ModulationStrategy::BandMid)
            .unwrap();
        let s2 = second
            .build_summary(1500.0, 1.0, ModulationStrategy::BandMid)
            .unwrap();
        assert!(s1.moves_touched > 0, "the first run touches moves");
        assert_eq!(s2.moves_touched, s1.moves_touched);
        assert!((s2.median_feed_delta_pct - s1.median_feed_delta_pct).abs() < 1e-9);
    }

    /// BandMid: a light engagement lifts the band mid by Garr's law.
    /// Feeds contract P14 (review M2) moved this number: the old law
    /// (0.04 / sqrt(0.1) = 0.1265 > 0.08) clamped at the ceiling 2880.
    #[test]
    fn band_mid_light_engagement_lifts_by_garr() {
        let mut tp = straight_toolpath(2, 1500.0);
        let engagements = vec![
            PerMoveEngagement::default(),
            PerMoveEngagement {
                radial_woc_fraction: 0.1,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
                thinning_lift: None,
            },
            PerMoveEngagement {
                radial_woc_fraction: 0.1,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
                thinning_lift: None,
            },
        ];
        let k = shapeoko();
        let mut ctx = make_ctx(&k, band());
        ctx.strategy = ModulationStrategy::BandMid;
        adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        let f = tp.moves[1].move_type.feed_rate().unwrap();
        // Garr at woc 0.1: lift = 1 / sin(acos(1 - 2 x 0.1)) = 1 / 0.6.
        // Chip 0.04 / 0.6 = 0.0667 (band 0.02..0.08, geometric mid 0.04),
        // below the 0.08 ceiling: 0.04 / 0.6 x 18000 x 2 = 2400.
        let want = 0.04 / 0.6 * 18000.0 * 2.0;
        assert!((f - want).abs() < 5.0, "expected {want}, got {f}");
    }

    /// S.4 regression: when the machine cap ties the band ceiling at the
    /// same clamped feed, `band_mid_feed_for_move` must report
    /// `MachineMaxFeed`, not `ChiploadMax` — the machine cap is the
    /// hard physical constraint and should win diagnostic priority over
    /// a coincidentally-equal chipload ceiling. This pins the arm-order
    /// fix (`MachineMaxFeed` checked before `ChiploadMax`).
    #[test]
    fn band_mid_tie_between_machine_cap_and_ceiling_reports_machine_cap() {
        let k = shapeoko();
        let mut ctx = make_ctx(&k, band());
        ctx.strategy = ModulationStrategy::BandMid;
        // band_ceiling = 0.08 * 18000 * 2 = 2880 — tie the machine cap
        // to it exactly.
        ctx.cutting_ceiling = ceiling(2880.0);
        let engagement = PerMoveEngagement {
            radial_woc_fraction: 0.01,
            axial_doc_fraction: 1.0,
            // T-11: the depth is now read in mm, not derived from the
            // fraction. 2.0 is what `nominal x fraction` meant here.
            axial_doc_mm: 2.0,
            thinning_lift: None,
        };
        // Light engagement inflates the chip-thinning target well past
        // the cap, so the clamp — not the natural target — decides the
        // feed and lands exactly on the tie.
        let predicted_cap_mm_min = 10_000.0; // not the tightest constraint
        let (feed, binding) = band_mid_feed_for_move(
            1500.0,
            engagement,
            predicted_cap_mm_min,
            &ctx,
            ctx.chipload_band.unwrap(),
        );
        assert!(
            (feed - 2880.0).abs() < 1e-6,
            "expected tied cap 2880, got {feed}"
        );
        assert_eq!(
            binding,
            BindingConstraint::MachineMaxFeed,
            "machine cap should win the tie, got {binding:?}"
        );
    }

    /// machine-max-feed cap wins.
    #[test]
    fn machine_max_feed_cap_wins() {
        let mut tp = straight_toolpath(2, 1500.0);
        let engagements = vec![
            PerMoveEngagement::default(),
            PerMoveEngagement {
                radial_woc_fraction: 0.1,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
                thinning_lift: None,
            },
            PerMoveEngagement {
                radial_woc_fraction: 0.1,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
                thinning_lift: None,
            },
        ];
        let k = shapeoko();
        let mut ctx = make_ctx(&k, band());
        ctx.cutting_ceiling = ceiling(500.0);
        ctx.strategy = ModulationStrategy::ConstrainedMax;
        adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        for m in &tp.moves {
            if let Some(f) = m.move_type.feed_rate() {
                assert!(f <= 500.0 + 1e-6, "max_feed cap violated: {f}");
            }
        }
    }

    /// Modulated feed never falls below band.min × RPM × flutes.
    #[test]
    fn modulation_never_emits_below_min_chipload() {
        let mut tp = straight_toolpath(3, 4000.0);
        let engagements = vec![
            PerMoveEngagement::default(),
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
                thinning_lift: None,
            },
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
                thinning_lift: None,
            },
            PerMoveEngagement {
                radial_woc_fraction: 1.0,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
                thinning_lift: None,
            },
        ];
        let k = shapeoko();
        let degenerate = ChiploadBand::new(0.05, 0.05).unwrap();
        let mut ctx = make_ctx(&k, degenerate);
        ctx.strategy = ModulationStrategy::ConstrainedMax;
        adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        // 0.05 × 18000 × 2 = 1800 mm/min.
        for m in &tp.moves {
            if let Some(f) = m.move_type.feed_rate() {
                assert!(f >= 1800.0 - 1e-6, "floor violated: {f}");
                assert!(f <= 1800.0 + 1e-6, "ceiling violated: {f}");
            }
        }
    }

    /// F-036b retained invariant: corner-heavy toolpath respects the
    /// predicted-feed cap.
    #[test]
    fn modulation_respects_predicted_feed_cap() {
        let mut tp = corner_heavy_toolpath(4000.0);
        let engagements: Vec<_> = (0..tp.moves.len())
            .map(|_| PerMoveEngagement {
                radial_woc_fraction: 0.1,
                axial_doc_fraction: 1.0,
                // T-11: the depth is now read in mm, not derived from the
                // fraction. 2.0 is what `nominal x fraction` meant here.
                axial_doc_mm: 2.0,
                thinning_lift: None,
            })
            .collect();
        let k = shapeoko();
        let mut ctx = make_ctx(&k, band());
        ctx.strategy = ModulationStrategy::ConstrainedMax;
        let predicted_pre =
            predicted_feeds_for_toolpath(&tp, &k, ctx.cutting_ceiling.mm_min(), 5000.0);
        adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        for (idx, m) in tp.moves.iter().enumerate() {
            if matches!(m.move_type, MoveType::Rapid) {
                continue;
            }
            let f = m.move_type.feed_rate().unwrap();
            if let Some(&cap) = predicted_pre.get(&idx) {
                let band_floor = ctx.chipload_band.unwrap().min_mm_per_tooth
                    * ctx.spindle_rpm
                    * ctx.flute_count as f64;
                let effective_cap = cap.max(band_floor);
                assert!(
                    f <= effective_cap + 1.0,
                    "move {idx} feed {f} exceeds predicted cap {cap} (floor={band_floor})"
                );
            }
        }
    }

    /// Per-segment variation invariant (F-036b retained): modulated
    /// path has ≥ 2 distinct feeds under heterogeneous engagement.
    #[test]
    fn modulation_produces_per_segment_feed_variation() {
        for strategy in [
            ModulationStrategy::BandMid,
            ModulationStrategy::ConstrainedMax,
        ] {
            let mut tp = corner_heavy_toolpath(2000.0);
            let engagements: Vec<_> = (0..tp.moves.len())
                .map(|i| {
                    if matches!(tp.moves[i].move_type, MoveType::Rapid) {
                        PerMoveEngagement::default()
                    } else if i % 2 == 0 {
                        PerMoveEngagement {
                            radial_woc_fraction: 1.0,
                            axial_doc_fraction: 1.0,
                            // T-11: the depth is now read in mm, not derived from the
                            // fraction. 2.0 is what `nominal x fraction` meant here.
                            axial_doc_mm: 2.0,
                            thinning_lift: None,
                        }
                    } else {
                        PerMoveEngagement {
                            radial_woc_fraction: 0.2,
                            axial_doc_fraction: 1.0,
                            // T-11: the depth is now read in mm, not derived from the
                            // fraction. 2.0 is what `nominal x fraction` meant here.
                            axial_doc_mm: 2.0,
                            thinning_lift: None,
                        }
                    }
                })
                .collect();
            let k = shapeoko();
            let mut ctx = make_ctx(&k, band());
            ctx.strategy = strategy;
            adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
            let mut feeds: Vec<f64> = tp
                .moves
                .iter()
                .filter_map(|m| m.move_type.feed_rate())
                .collect();
            feeds.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            feeds.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
            assert!(
                feeds.len() >= 2,
                "expected >= 2 distinct feeds under {strategy:?}, got {feeds:?}"
            );
        }
    }

    // ── A2 (point mode) ────────────────────────────────────────────────

    /// A full-slot engagement at 2 mm axial depth.
    fn full_slot() -> PerMoveEngagement {
        PerMoveEngagement {
            radial_woc_fraction: 1.0,
            axial_doc_fraction: 1.0,
            axial_doc_mm: 2.0,
            thinning_lift: None,
        }
    }

    /// A2: `point(v)` stores `min = max = v` and is a point. `new(v, v)`
    /// is not a point. A non-finite or non-positive value is refused.
    #[test]
    fn point_band_holds_one_value() {
        let p = ChiploadBand::point(0.05).unwrap();
        assert!(p.is_point());
        assert!((p.min_mm_per_tooth - 0.05).abs() < 1e-12);
        assert!((p.max_mm_per_tooth - 0.05).abs() < 1e-12);
        assert!(!ChiploadBand::new(0.05, 0.05).unwrap().is_point());
        assert!(!band().is_point());
        assert!(ChiploadBand::point(0.0).is_none());
        assert!(ChiploadBand::point(-0.01).is_none());
        assert!(ChiploadBand::point(f64::NAN).is_none());
        assert!(ChiploadBand::point(f64::INFINITY).is_none());
    }

    /// A2 ConstrainedMax: the point is the chipload cap.
    /// 0.05 × 18000 × 2 = 1800 mm/min, tag `ChiploadMax` (Q7).
    #[test]
    fn point_constrained_max_caps_at_the_point() {
        let k = shapeoko();
        let mut ctx = make_ctx(&k, ChiploadBand::point(0.05).unwrap());
        ctx.strategy = ModulationStrategy::ConstrainedMax;
        let (feed, binding) =
            max_safe_feed_for_move(full_slot(), None, ctx.chipload_band.unwrap(), &ctx, 1.0e9);
        assert!((feed - 1800.0).abs() < 1e-6, "expected 1800, got {feed}");
        assert_eq!(binding, BindingConstraint::ChiploadMax);
    }

    /// A2 ConstrainedMax: the feed scale applies and no floor restores the
    /// feed. At scale 0.7 a point emits 0.7 × 1800 = 1260 mm/min with the
    /// `ChiploadMax` tag. The zero-width band `new(v, v)` restores its
    /// floor at 1800 and keeps the cap's tag (the pre-A2 behaviour: the
    /// cap equals the floor, so the floor does not re-tag the move).
    #[test]
    fn point_feed_scale_has_no_floor() {
        let k = shapeoko();
        let mut ctx = make_ctx(&k, ChiploadBand::point(0.05).unwrap());
        ctx.strategy = ModulationStrategy::ConstrainedMax;
        ctx.feed_scale = 0.7;
        let (feed, binding) =
            max_safe_feed_for_move(full_slot(), None, ctx.chipload_band.unwrap(), &ctx, 1.0e9);
        assert!((feed - 1260.0).abs() < 1e-6, "expected 1260, got {feed}");
        assert_eq!(binding, BindingConstraint::ChiploadMax);

        let zero_width = ChiploadBand::new(0.05, 0.05).unwrap();
        let (feed, binding) = max_safe_feed_for_move(full_slot(), None, zero_width, &ctx, 1.0e9);
        assert!(
            (feed - 1800.0).abs() < 1e-6,
            "expected floor 1800, got {feed}"
        );
        assert_eq!(binding, BindingConstraint::ChiploadMax);

        // The same through the public entry point: every engaged move
        // emits about 1260, and no floor lifts it back to the point. The
        // kinematic cap on these moves can tie the chipload cap, so this
        // part does not assert the tag. The tolerance matches the sibling
        // tests on this fixture.
        let mut tp = straight_toolpath(3, 1500.0);
        let engagements: Vec<_> = (0..tp.moves.len())
            .map(|i| {
                if i == 0 {
                    PerMoveEngagement::default()
                } else {
                    full_slot()
                }
            })
            .collect();
        let outcome = adaptive_feed_modulate(&mut tp, &engagements, &ctx).unwrap();
        assert_eq!(outcome.changed, 3);
        for (feed, _) in outcome.per_move.values() {
            assert!(*feed < 1800.0 - 1.0, "a floor lifted the feed: {feed}");
            assert!((feed - 1260.0).abs() < 5.0, "expected ~1260, got {feed}");
        }
    }

    /// A2 ConstrainedMax: the deflection pin arm pins to the point. The
    /// edge force alone is over budget (0.1 mm/N × the slot's edge force
    /// at 2 mm, far above 0.2 mm), so the move pins to 1800 mm/min. The pin
    /// value ties the chipload cap; the tag names deflection. At scale
    /// 0.7 the pin gets no floor: 1260 mm/min.
    #[test]
    fn point_deflection_pin_is_the_point() {
        let k = shapeoko();
        let mut ctx = make_ctx(&k, ChiploadBand::point(0.05).unwrap());
        ctx.strategy = ModulationStrategy::ConstrainedMax;
        ctx.deflection_inputs = Some(DeflectionLimitInputs {
            line: oak_line(),
            cutter: flat_cutter(),
            compliance_mm_per_n: 0.1,
            max_tip_deflection_mm: 0.2,
        });
        let (feed, binding) =
            max_safe_feed_for_move(full_slot(), None, ctx.chipload_band.unwrap(), &ctx, 1.0e9);
        assert!(
            (feed - 1800.0).abs() < 1e-6,
            "expected pin 1800, got {feed}"
        );
        assert_eq!(binding, BindingConstraint::DeflectionMax);

        ctx.feed_scale = 0.7;
        let (feed, binding) =
            max_safe_feed_for_move(full_slot(), None, ctx.chipload_band.unwrap(), &ctx, 1.0e9);
        assert!((feed - 1260.0).abs() < 1e-6, "expected 1260, got {feed}");
        assert_eq!(binding, BindingConstraint::DeflectionMax);
    }

    /// A2 ConstrainedMax: the power pin arm pins to the point. A budget
    /// of 1 W is below the edge-force power, so no feed satisfies it and
    /// the move pins to 1800 mm/min with the `PowerMax` tag.
    #[test]
    fn point_power_pin_is_the_point() {
        let k = shapeoko();
        let mut ctx = make_ctx(&k, ChiploadBand::point(0.05).unwrap());
        ctx.strategy = ModulationStrategy::ConstrainedMax;
        ctx.power_inputs = Some(PowerLimitInputs {
            line: crate::material::force_line::ForceLine::goli_2018_mdf(),
            cutter: flat_cutter(),
            available_kw: 0.001,
        });
        let (feed, binding) =
            max_safe_feed_for_move(full_slot(), None, ctx.chipload_band.unwrap(), &ctx, 1.0e9);
        assert!(
            (feed - 1800.0).abs() < 1e-6,
            "expected pin 1800, got {feed}"
        );
        assert_eq!(binding, BindingConstraint::PowerMax);
    }

    /// A2 BandMid: the target and the ceiling are the point, and no floor
    /// lifts a lower kinematic cap. Full slot: 1800 mm/min, `ChiploadMax`.
    /// A kinematic cap of 500 mm/min binds at 500 (`KinematicReach`); the
    /// zero-width band `new(v, v)` lifts that cap to its floor, 1800.
    #[test]
    fn point_band_mid_targets_the_point_with_no_floor() {
        let k = shapeoko();
        let point = ChiploadBand::point(0.05).unwrap();
        let ctx = make_ctx(&k, point);
        let (feed, binding) = band_mid_feed_for_move(1500.0, full_slot(), 1.0e9, &ctx, point);
        assert!((feed - 1800.0).abs() < 1e-6, "expected 1800, got {feed}");
        assert_eq!(binding, BindingConstraint::ChiploadMax);

        // Light engagement: the chip-thinning lift does not pass the point.
        let light = PerMoveEngagement {
            radial_woc_fraction: 0.1,
            ..full_slot()
        };
        let (feed, _) = band_mid_feed_for_move(1500.0, light, 1.0e9, &ctx, point);
        assert!((feed - 1800.0).abs() < 1e-6, "expected 1800, got {feed}");

        let (feed, binding) = band_mid_feed_for_move(1500.0, full_slot(), 500.0, &ctx, point);
        assert!((feed - 500.0).abs() < 1e-6, "expected 500, got {feed}");
        assert_eq!(binding, BindingConstraint::KinematicReach);

        let zero_width = ChiploadBand::new(0.05, 0.05).unwrap();
        let (feed, _) = band_mid_feed_for_move(1500.0, full_slot(), 500.0, &ctx, zero_width);
        assert!(
            (feed - 1800.0).abs() < 1e-6,
            "expected floor 1800, got {feed}"
        );
    }
}
