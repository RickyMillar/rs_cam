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

use crate::feeds::entry_rule::StoredFeedChip;
use serde::Serialize;
use serde::ser::SerializeMap;

use crate::compute::catalog::{Axial, OperationConfig, Radial};
use crate::compute::tool_config::ToolConfig;
use crate::feeds::FeedsInput;
use crate::feeds::force_budget::BudgetDepth;
use crate::feeds::planned::{
    BodyColumn, BodyDepth, CeilingEdge, DeclaredCuts, DeflectionPlanned, ForceBudgetPlanned,
    ForceEnv, PlaneDepth, PlaneRole, TableEnv, ValueCheck, VendorDepthTablePlanned, budget_rank,
    judge_declared_cuts, lowest_ceiling, planned_limit_of,
};
use crate::feeds::sourced::{
    Basis, ChipMm, FeedMmMin, MachineField, PlungeMmMin, Rpm, Sourced, lower_to_cap,
};
use crate::feeds::suggest::SuggestContext;
use crate::feeds::{
    FeedsError, FeedsResult, FeedsWarning, MIN_AE_MM, MIN_AP_MM, PassRole, SpindleStrategy,
    ToolGeometryHint, VendorLut, calculate,
};
use crate::limits::{BoundSide, CutKind, NoNumberReason};
use crate::machine::MachineProfile;
use crate::material::Material;
use crate::tool::ToolDefinition;

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
/// machine). The FM1 walk went from under 1 s to 4.9 s. P19: the test
/// envelope has a model box, so it also times the `Runtime` limit and the
/// refined vendor depth table edge; it measured 121 to 148 ms alone
/// (115 to 120 ms before P19 with the same box) and 515 ms inside the
/// parallel `--lib` run beside another session's viz suite (2026-11-07,
/// operator's machine, loaded). P29-10: after P29-8 8d it measured 342 to
/// 346 ms alone, because each edge solve judged the entries, which never
/// set a ceiling; the edges now judge only the cuts that set a ceiling
/// (`planned::lowest_ceiling`, the same values) and it measured 205 to
/// 214 ms alone (2026-11-13, operator's machine, `P29-10_DESIGN.md`
/// section 15).
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

/// P16 (review D5, D7): why the envelope proposes no depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NoDepthProposal {
    /// The material has no force line (`Material::force_line` refuses), so
    /// no limit can model the depth (operator ruling D5, the P13c add-door
    /// rule). The op keeps the depth it has.
    NoForceLine,
    /// The op follows a surface at an offset (`Axial::SurfaceOffset`,
    /// Project Curve), so the simulation gives the depth into the stock
    /// (lead ruling D7).
    AfterSimulation,
}

/// P29-10 (10b): one note of the envelope. The list is closed: a new kind
/// does not compile until [`Self::face`] and [`Self::text`] state its
/// words. A cut that the plan may not cut is no note: its rows carry
/// `Caveat::MayOccur` and its exemption row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnvelopeNote {
    /// The envelope proposes no depth, with the reason.
    NoDepth(NoDepthProposal),
    /// No width up to the ceiling (mm) meets the move budget.
    MoveBudgetNotMet {
        /// The width ceiling of the move budget (mm).
        ceiling_mm: f64,
    },
    /// The assumptions of a computed width
    /// (`force_budget::COMPUTED_WIDTH_ASSUMPTIONS`).
    ComputedWidth,
    /// No depth meets the force budget at some width: the minimum depth
    /// ships there (Q6).
    ForceBudgetFloor,
    /// The force budget sets the depth and the move budget is not met
    /// there (trap 2); no number moves.
    MoveBudgetConflict,
}

impl EnvelopeNote {
    /// The token of the kind on the wire.
    #[must_use]
    pub const fn kind(self) -> &'static str {
        match self {
            Self::NoDepth(_) => "no_depth",
            Self::MoveBudgetNotMet { .. } => "move_budget_not_met",
            Self::ComputedWidth => "computed_width",
            Self::ForceBudgetFloor => "force_budget_floor",
            Self::MoveBudgetConflict => "move_budget_conflict",
        }
    }

    /// The short face for a narrow row (the SAFE CUT header).
    #[must_use]
    pub const fn face(self) -> &'static str {
        match self {
            Self::NoDepth(NoDepthProposal::NoForceLine) => "No depth: no force line",
            Self::NoDepth(NoDepthProposal::AfterSimulation) => "Depth known after simulation",
            Self::MoveBudgetNotMet { .. } => "Move budget not met",
            Self::ComputedWidth => crate::feeds::force_budget::COMPUTED_WIDTH_ASSUMPTIONS_FACE,
            Self::ForceBudgetFloor => crate::feeds::force_budget::FORCE_BUDGET_FLOOR_FACE,
            Self::MoveBudgetConflict => "Move budget not met at the depth",
        }
    }

    /// The whole text: the hover, MCP and the CLI.
    #[must_use]
    pub fn text(self) -> String {
        match self {
            Self::NoDepth(reason) => reason.clause().to_owned(),
            Self::MoveBudgetNotMet { ceiling_mm } => move_budget_not_met_note(ceiling_mm),
            Self::ComputedWidth => crate::feeds::force_budget::computed_width_assumptions_line(),
            Self::ForceBudgetFloor => {
                crate::feeds::force_budget::FORCE_BUDGET_FLOOR_NOTE.to_owned()
            }
            Self::MoveBudgetConflict => MOVE_BUDGET_CONFLICT.to_owned(),
        }
    }
}

impl Serialize for EnvelopeNote {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("kind", self.kind())?;
        map.serialize_entry("face", self.face())?;
        map.serialize_entry("text", &self.text())?;
        map.end()
    }
}

impl NoDepthProposal {
    /// One operator-facing clause, the same on every surface.
    #[must_use]
    pub const fn clause(self) -> &'static str {
        match self {
            Self::NoForceLine => {
                "no depth proposal: this material has no measured force line (ruling B6)"
            }
            Self::AfterSimulation => {
                "no depth proposal: the depth into the stock is known after the simulation"
            }
        }
    }
}

/// P16: the depth of the proposal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProposedDepth {
    /// The envelope proposes this depth (mm).
    Mm(f64),
    /// The envelope proposes no depth, for this reason. An apply writes no
    /// depth.
    None(NoDepthProposal),
}

/// P16: the proposal of the envelope: a width and, when the envelope can
/// model it, a depth.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Proposal {
    /// The proposed depth, or the reason for no depth.
    pub depth: ProposedDepth,
    /// The proposed radial width (mm).
    pub ae_mm: f64,
}

impl Proposal {
    /// The proposed depth (mm), or `None` when the envelope proposes no
    /// depth.
    #[must_use]
    pub const fn ap_mm(&self) -> Option<f64> {
        match self.depth {
            ProposedDepth::Mm(mm) => Some(mm),
            ProposedDepth::None(_) => None,
        }
    }

    /// The proposed point, or `None` when the envelope proposes no depth.
    #[must_use]
    pub const fn point(&self) -> Option<CutPoint> {
        match self.depth {
            ProposedDepth::Mm(ap_mm) => Some(CutPoint {
                ap_mm,
                ae_mm: self.ae_mm,
            }),
            ProposedDepth::None(_) => None,
        }
    }

    /// The reason for no depth, when the envelope proposes none.
    #[must_use]
    pub const fn no_depth(&self) -> Option<NoDepthProposal> {
        match self.depth {
            ProposedDepth::Mm(_) => None,
            ProposedDepth::None(reason) => Some(reason),
        }
    }
}

/// The JSON shape keeps `ap_mm` and `ae_mm`; `ap_mm` is `null` when the
/// envelope proposes no depth, and `no_depth` gives the reason.
impl Serialize for Proposal {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("ap_mm", &self.ap_mm())?;
        map.serialize_entry("ae_mm", &self.ae_mm)?;
        if let Some(reason) = self.no_depth() {
            map.serialize_entry("no_depth", &reason)?;
            map.serialize_entry("no_depth_text", reason.clause())?;
        }
        map.end()
    }
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

/// P29-10 10g: the part of the width x depth plane frame that a limit is
/// ([`LimitKind::frame`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FramePart {
    /// The end of the width axis (the tool diameter).
    WidthEnd,
    /// The top of the depth axis (a hard stop of the tool).
    DepthTop,
    /// The origin (the smallest engagement).
    Origin,
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
    /// The machine rigidity cap (roughing). P13c (O1; the Adaptive family
    /// since P29-4): an `ap(ae)` curve, the depth where the peak
    /// engaged-edge force meets the machine force budget.
    Rigidity,
    /// P13c (O3, O4): the corner-coverage rule of a free-width pocket, an
    /// `ae` ceiling at `(1 + 1/sqrt 2) r`: a wider stepover leaves uncut
    /// material in a sharp offset corner.
    CornerCoverage,
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
    /// Every variant, in declaration order (P29-2: `limits::LimitId` maps
    /// each one).
    pub const ALL: [Self; 14] = crate::all_variants!(LimitKind:
        FluteGuard, CuttingLength, VendorAxial, VendorDepthTable, Rigidity, CornerCoverage,
        Power, Deflection, ChipBand, RubbingFloor, FeedCap, Diameter, Runtime, EngagementFloor,
    );

    /// P29-10 10g: the part of the plane frame that this limit is, if it is
    /// one. A hard stop of the tool or the calculator bounds the plane: the
    /// diameter ends the width axis, the cutting length and the flute guard
    /// end the depth axis, and the engagement floor is the origin. Every
    /// other limit is a line inside the frame. One match, no wildcard: a
    /// surface reads this fact and does not match on the kind.
    #[must_use]
    pub const fn frame(self) -> Option<FramePart> {
        match self {
            Self::Diameter => Some(FramePart::WidthEnd),
            Self::CuttingLength | Self::FluteGuard => Some(FramePart::DepthTop),
            Self::EngagementFloor => Some(FramePart::Origin),
            Self::VendorAxial
            | Self::VendorDepthTable
            | Self::Rigidity
            | Self::CornerCoverage
            | Self::Power
            | Self::Deflection
            | Self::ChipBand
            | Self::RubbingFloor
            | Self::FeedCap
            | Self::Runtime => None,
        }
    }

    /// P29-10 10g: the name of the limit's line on the plane, from the one
    /// writer of a limit name (`limits::LimitId::text`, lead ruling R4).
    #[must_use]
    pub const fn chart_name(self) -> &'static str {
        let text = crate::limits::LimitId::of_limit_kind(self).text();
        match text.chart.line() {
            Some(name) => name,
            None => text.short,
        }
    }

    /// P29-10 10g: `true` when the limit's name gives its depth as a share
    /// of the tool diameter on a flat edge ("Vendor DOC 1\u{00D7}D"): the
    /// vendor prints a depth per pass for both depth rows.
    #[must_use]
    pub const fn names_depth_in_diameters(self) -> bool {
        match self {
            Self::VendorAxial | Self::VendorDepthTable => true,
            Self::FluteGuard
            | Self::CuttingLength
            | Self::Rigidity
            | Self::CornerCoverage
            | Self::Power
            | Self::Deflection
            | Self::ChipBand
            | Self::RubbingFloor
            | Self::FeedCap
            | Self::Diameter
            | Self::Runtime
            | Self::EngagementFloor => false,
        }
    }

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
    /// P19b: `ae >= f(ap)` (a floor) or `ae <= f(ap)` (a ceiling): one
    /// `[ap, ae]` pair per sample, ascending in `ap`, linear between them.
    /// Above the last sample the last value holds. Below the first sample
    /// a ceiling keeps the first value, and a floor is not met at any
    /// width: no width up to the ceiling of that limit meets it there (the
    /// move budget at a shallow depth). See [`ae_curve_at`].
    AeCurve {
        /// Floor or ceiling.
        bound: CurveBound,
        /// The `[ap, ae]` samples, ascending in `ap`.
        points: Vec<[f64; 2]>,
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

impl Edge {
    /// `true` for an edge that bounds the cut from below (a depth floor, a
    /// width floor, a floor curve). [`SafeEnvelope::place`] and
    /// `limits::BoundSide` read it.
    #[must_use]
    pub const fn is_floor(&self) -> bool {
        match self {
            Self::ApFloor { .. }
            | Self::AeFloor { .. }
            | Self::AeCurve {
                bound: CurveBound::Floor,
                ..
            } => true,
            Self::ApCeiling { .. }
            | Self::ApCurve { .. }
            | Self::AeCeiling { .. }
            | Self::AeCurve {
                bound: CurveBound::Ceiling,
                ..
            }
            | Self::PerPoint
            | Self::Unmodelled { .. } => false,
        }
    }
}

/// P19b: which side of an [`Edge::AeCurve`] is inside.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CurveBound {
    /// `ae >= f(ap)`.
    Floor,
    /// `ae <= f(ap)`.
    Ceiling,
}

/// P19b: the width bound of an [`Edge::AeCurve`] at the depth `ap_mm`:
/// linear between samples, the last value above the last sample. Below the
/// first sample a ceiling keeps the first value and a floor gives
/// `f64::INFINITY` (no width meets it there). `None` for an empty curve.
#[must_use]
pub fn ae_curve_at(bound: CurveBound, points: &[[f64; 2]], ap_mm: f64) -> Option<f64> {
    let first = points.first()?;
    if ap_mm < first[0] {
        return Some(match bound {
            CurveBound::Floor => f64::INFINITY,
            CurveBound::Ceiling => first[1],
        });
    }
    interpolate_curve(points, ap_mm)
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
    /// P29-7 7c: the declared cut that sets this limit at the proposal
    /// width (the card prints "machine rigidity, slot ring"). `None` for a
    /// limit that does not change with the cut, or with no number there.
    #[serde(serialize_with = "cut_label")]
    pub binding_cut: Option<CutKind>,
}

/// The words of a binding cut on the wire (`CutKind::label`).
fn cut_label<S: serde::Serializer>(
    cut: &Option<CutKind>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    cut.map(CutKind::label).serialize(serializer)
}

impl Limit {
    /// The `ap` ceiling of this limit at the width `ae_mm`, if it has one.
    #[must_use]
    pub fn ap_ceiling_at(&self, ae_mm: f64) -> Option<f64> {
        match &self.edge {
            Edge::ApCeiling { mm } => Some(*mm),
            Edge::ApCurve { points } => interpolate_curve(points, ae_mm),
            Edge::ApFloor { .. }
            | Edge::AeCeiling { .. }
            | Edge::AeFloor { .. }
            | Edge::AeCurve { .. }
            | Edge::PerPoint
            | Edge::Unmodelled { .. } => None,
        }
    }

    /// P19b: the width bound of this limit at the depth `ap_mm`, and
    /// whether it is a floor: an `AeFloor`, an `AeCeiling`, or an
    /// `AeCurve` read at `ap_mm` ([`ae_curve_at`]). `None` for a limit
    /// with no width bound.
    #[must_use]
    pub fn ae_bound_at(&self, ap_mm: f64) -> Option<(f64, CurveBound)> {
        match &self.edge {
            Edge::AeFloor { mm } => Some((*mm, CurveBound::Floor)),
            Edge::AeCeiling { mm } => Some((*mm, CurveBound::Ceiling)),
            Edge::AeCurve { bound, points } => {
                ae_curve_at(*bound, points, ap_mm).map(|v| (v, *bound))
            }
            Edge::ApCeiling { .. }
            | Edge::ApCurve { .. }
            | Edge::ApFloor { .. }
            | Edge::PerPoint
            | Edge::Unmodelled { .. } => None,
        }
    }

    /// P19b: the smallest depth in the sample span of an `AeCurve` at which
    /// the width `ae_mm` meets the bound, by the straight segments between
    /// the samples. `Some(f64::INFINITY)` when no depth on the curve meets
    /// it; `None` for an edge that is not an `AeCurve`. The safe-region
    /// fill reads it as the bottom of a column (a width floor that falls
    /// with the depth is a depth floor that falls with the width).
    #[must_use]
    pub fn depth_floor_at_width(&self, ae_mm: f64) -> Option<f64> {
        let Edge::AeCurve { bound, points } = &self.edge else {
            return None;
        };
        let meets = |v: f64| match bound {
            CurveBound::Floor => ae_mm >= v,
            CurveBound::Ceiling => ae_mm <= v,
        };
        if let Some(first) = points.first()
            && meets(first[1])
        {
            return Some(first[0]);
        }
        for w in points.windows(2) {
            let [a, b] = w else { continue };
            if meets(b[1]) {
                // The crossing on this segment, linear between the two
                // samples.
                let span = b[1] - a[1];
                let t = if span.abs() > 0.0 {
                    ((ae_mm - a[1]) / span).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                return Some(a[0] + t * (b[0] - a[0]));
            }
        }
        Some(f64::INFINITY)
    }

    /// The signed margin of `p` against this limit, as a share of the
    /// limit: positive inside, negative outside. `None` for a limit that
    /// has no edge at `p`.
    #[must_use]
    pub fn margin(&self, p: CutPoint) -> Option<f64> {
        let ceiling = |value: f64, cap: f64| (cap > 0.0).then(|| (cap - value) / cap);
        let floor = |value: f64, low: f64| (low > 0.0).then(|| (value - low) / low);
        match &self.edge {
            Edge::ApCeiling { .. } | Edge::ApCurve { .. } => self
                .ap_ceiling_at(p.ae_mm)
                .and_then(|cap| ceiling(p.ap_mm, cap)),
            Edge::ApFloor { mm } => floor(p.ap_mm, *mm),
            Edge::AeCeiling { mm } => ceiling(p.ae_mm, *mm),
            Edge::AeFloor { mm } => floor(p.ae_mm, *mm),
            // P19b: the bound at the depth of `p`. A floor that no width
            // meets (below the first sample) is crossed by the whole share.
            Edge::AeCurve { bound, points } => {
                match (bound, ae_curve_at(*bound, points, p.ap_mm)) {
                    (CurveBound::Floor, Some(v)) if v.is_infinite() => Some(-1.0),
                    (CurveBound::Floor, Some(v)) => floor(p.ae_mm, v),
                    (CurveBound::Ceiling, Some(v)) => ceiling(p.ae_mm, v),
                    (CurveBound::Floor | CurveBound::Ceiling, None) => None,
                }
            }
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

/// K7: the facts of the tool and the sample that place a measured point
/// ([`SafeEnvelope::placement_of_measured`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasuredTool {
    /// The engaged diameter at the sample depth (mm),
    /// `Engagement::d_eng_mm`.
    pub d_eng_mm: f64,
    /// P20i: the power of the sample as the power gate grades it
    /// (`tool_load::power::gate_sample_power`). `None`: the gate does not
    /// grade the sample (no force line, an air cut, a transit or entry
    /// sample, a drill cycle), so the placement does not judge its power.
    pub power: Option<MeasuredPower>,
}

/// P20i: the power figure of one measured sample, as the power gate
/// grades it: the sample engagement at the feed the gate reads.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct MeasuredPower {
    /// The predicted spindle power (kW).
    pub required_kw: f64,
    /// The rated spindle power at the sample RPM (kW).
    pub available_kw: f64,
    /// The feed of the figure (mm/min): the emitted feed, or the reached
    /// feed (P20c).
    pub feed_mm_min: f64,
}

impl MeasuredPower {
    /// The signed share of the rated power left: positive inside, negative
    /// over the budget. `None` with no rated power.
    #[must_use]
    pub fn margin(&self) -> Option<f64> {
        (self.available_kw > 0.0)
            .then(|| (self.available_kw - self.required_kw) / self.available_kw)
    }
}

/// K7: the value of a point that one limit judges, and the limit there.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct JudgedValue {
    /// `"depth"`, `"width"` or (P20i) `"power"`.
    pub axis: &'static str,
    /// The value of the point on that axis, in `unit`.
    pub measured: f64,
    /// The limit at the point, in `unit`.
    pub limit: f64,
    /// `"mm"` for a depth or a width, `"kW"` for power.
    pub unit: &'static str,
    /// P20i: the feed of a power value (mm/min), the feed the power gate
    /// reads. `None` for a depth or a width.
    pub feed_mm_min: Option<f64>,
}

/// Where a point sits against the boundary.
///
/// P29-10 10i (lead ruling 4): the wire carries the decision (`kind`) and
/// the limit (`binding`), never the margin: the number of the stated point
/// is its stated row (`SafeEnvelope::rows`).
#[derive(Debug, Clone, Copy, PartialEq)]
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

impl Serialize for Placement {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        match self {
            Self::Inside { .. } => map.serialize_entry("kind", "inside")?,
            Self::Outside { binding, .. } => {
                map.serialize_entry("kind", "outside")?;
                map.serialize_entry("binding", binding)?;
            }
        }
        map.end()
    }
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
    /// The spindle power at the point (kW); `None` when not modelled
    /// (P20d). It is the power of `power_case` at `power_width_mm`, not
    /// always at `ae_mm` (K7 change 3).
    pub power_kw: Option<f64>,
    /// P20i: the case of the pre-sim power set that `power_kw` is the
    /// power of (`FeedsResult::power_case`): the nominal width, the planner
    /// cap of a 3D Rough or the slot of a slot first pass.
    pub power_case: crate::feeds::engagement::PowerCase,
    /// P20i: the width (mm) that `power_kw` is evaluated at: `ae_mm` for
    /// the nominal case, `min(trochoid_cap_mult x ae, D)` for the planner
    /// cap, `D` for a slot.
    pub power_width_mm: f64,
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
    fn from_result(
        ap_mm: f64,
        ae_mm: f64,
        r: &FeedsResult,
        flutes: u32,
        cuts: &crate::feeds::planned::DeclaredCuts,
    ) -> Self {
        // P20i: the width of the case that the result names.
        let power_width_mm = crate::feeds::planned::power_case_width(cuts, r.power_case, ae_mm);
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
            power_case: r.power_case,
            power_width_mm,
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
    let fits_curve = r.available_power_kw <= 0.0
        || r.power_kw
            .is_none_or(|kw| kw <= r.available_power_kw * (1.0 + 1e-9));
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
    /// The spindle power (kW); `None` when a corner of the cell has no
    /// modelled power (P20d).
    pub power_kw: Option<f64>,
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
            power_kw: match (p00.power_kw, p10.power_kw, p01.power_kw, p11.power_kw) {
                (Some(a), Some(b), Some(c), Some(d)) => {
                    let low = a + ti * (b - a);
                    let high = c + ti * (d - c);
                    Some(low + tj * (high - low))
                }
                _ => None,
            },
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
    /// P29-10z (lead ruling Q2): the feed modulation of the simulation run
    /// (`RunModulation`). A planned or stated row of a limit that the
    /// modulator caps, on a cut it re-times, carries
    /// `Caveat::BeforeModulation`. No default: a surface states the value
    /// of its run, a reader with no run states `Off`.
    pub modulation: crate::dressup::feed_modulation::RunModulation,
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
    /// P29-7 7c: the declared cuts of the op on the tool, built once. Every
    /// limit that changes with the cut reads them through the one driver
    /// (`planned::judge_declared_cuts`).
    cuts: DeclaredCuts,
    /// P29-8e (c): the vertical feed of each entry kind and of the drill
    /// cut, for the vertical-rate check.
    vertical: crate::feeds::planned::VerticalFeeds,
    /// P29-10z (lead ruling Q2): the feed modulation of the run.
    modulation: crate::dressup::feed_modulation::RunModulation,
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
        self.calc_at_with(p, crate::feeds::StepRecording::Skip)
    }

    /// [`Self::calc_at`] with its step records: the pick point (P13b, lead
    /// ruling 5). The grid points skip them.
    fn calc_at_recorded(&self, p: CutPoint) -> FeedsResult {
        self.calc_at_with(p, crate::feeds::StepRecording::Record)
    }

    fn calc_at_with(&self, p: CutPoint, recording: crate::feeds::StepRecording) -> FeedsResult {
        let mut input = FeedsInput::of_operation(
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
        crate::feeds::calculate_with(&input, crate::feeds::DepthClamps::DrawnAsLimits, recording)
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
            // P20d: an advisory figure (the axial plunge) bounds nothing.
            !f.section.gates()
                || f.available_kw <= 0.0
                || f.required_kw <= f.available_kw * (1.0 + 1e-9)
        })
    }
}

/// P29-10 10g (lead ruling R2): one solved column of the force-budget edge.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BudgetColumn {
    /// The width (mm).
    pub ae_mm: f64,
    /// The ceiling (mm); `None` where no depth below the flute meets the
    /// budget (the edge then sits on the flute top).
    pub ap_mm: Option<f64>,
    /// What sets the ceiling in this column.
    pub sets: ColumnCeiling,
    /// The declared cut whose ceiling is the lowest in this column.
    #[serde(serialize_with = "cut_label_ser")]
    pub cut: CutKind,
}

fn cut_label_ser<S: serde::Serializer>(cut: &CutKind, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(cut.label())
}

/// P29-10 10g: what sets the force-budget edge in one column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnCeiling {
    /// The depth where the peak force of the cut meets the budget.
    ForceBudget,
    /// A geometric cap: the force at the smallest depth the calculator
    /// writes is already over the budget (the depth floor).
    DepthFloor,
    /// A geometric cap: no depth below the flute meets the budget (the edge
    /// rides the flute top).
    FluteTop,
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
    /// The one proposal, with the reason it was chosen. P16: the depth is
    /// [`ProposedDepth::None`] when the envelope cannot model it.
    pub proposal: Sourced<Proposal>,
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
    /// and the warning when no depth meets the force budget. P29-10 (10b):
    /// a closed list, [`EnvelopeNote`].
    pub notes: Vec<EnvelopeNote>,
    /// P29-7 7c: the check of each declared cut at the proposal, for each
    /// limit that changes with the cut: the force budget, the deflection
    /// and the vendor depth table (the depth of the proposal against the
    /// ceiling of the cut), and the chip band and the rubbing floor (the
    /// planned peak chip of the cut). A cut that the plan may not cut
    /// (`Presence::MayOccur`) is judged here, never drawn as a ceiling, and
    /// names its exemption row. The working list of `read_declared_cuts`;
    /// [`Self::rows`] is the record.
    planned: Vec<PlannedRow>,
    /// P29-10: THE rows of the envelope: one planned row per declared cut
    /// and planned limit (`Subject::Planned`, at the proposal), then one
    /// stated row per limit of the boundary when `stated` is `Some`
    /// (`Subject::Stated`, `limits::of_envelope`).
    rows: Vec<crate::limits::CheckRow>,
    /// P29-10 10g: the declared cuts on the plane at the proposal point
    /// (the point of the planned rows) and at the stated point
    /// (`feeds::plane`), written with the rows.
    marks_at_proposal: Option<crate::feeds::plane::PlaneMarks>,
    marks_at_stated: Option<crate::feeds::plane::PlaneMarks>,
    /// The operator's stated point, when every free axis is stated.
    pub stated: Option<CutPoint>,
    /// P29-7 7c: the limits that draw a depth ceiling, in the draw order,
    /// and the limits that check a value of each cut ([`plane_roles`]).
    ceilings: Vec<CeilingEdge>,
    value_checks: Vec<ValueCheck>,
    /// Where the operator's stated point sits.
    pub operator: Option<Placement>,
    inputs: OwnedInputs,
    /// The point a proposal pick computes its speeds at: the proposal, and
    /// on an axis with no proposed depth the depth of `propose` (the stated
    /// depth lowered to a cap, else the calculator depth). The apply writes
    /// no depth then, and pass 9 sizes the feed at the depth the op keeps.
    proposal_point: CutPoint,
    /// P13c (O1): the force-budget model of the `Rigidity` limit, when the
    /// op's rule is the force budget. The proposal solves it exactly.
    budget: Option<BudgetModel>,
    /// P13c (O4): the chip-sized limits (the vendor depth table, power,
    /// deflection) at the suggested chip, when the boundary is drawn at the
    /// operator's chip. A pick writes the suggested speeds, so the
    /// removal-rate pick stays inside both sets (else it would sit outside
    /// its own envelope once its speeds are written).
    pick_limits: Vec<Limit>,
    /// P19: the ceilings solved exactly at one width (the stated width, the
    /// proposal width). `place` and `judged_value` read them; the drawn
    /// boundary does not hold them, so a drawn edge does not move when the
    /// stated cut moves.
    exact: Vec<ExactCeiling>,
    /// P19: the move budget that `place` judges each point with, when the
    /// `Runtime` limit draws no edge.
    move_budget: Option<MoveBudget>,
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

/// P19: the role of an exact ceiling at one width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExactRole {
    /// The ceiling at the chip of the drawn edge. It takes the place of the
    /// drawn value at that width.
    CurveChip,
    /// The force-budget depth at the proposal width at the pick chip
    /// (`BudgetChip::Pick`). The lower of it and the other value holds, so
    /// the proposal sits on the edge it names.
    Pick,
}

/// P19: when [`SafeEnvelope::add_exact_ceilings`] runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExactPhase {
    /// Before the proposal: the stated width.
    Stated,
    /// After the proposal: the proposal width.
    Proposal,
}

/// P19: a ceiling (`ap`, mm) of one limit, solved exactly at one width.
#[derive(Debug, Clone, Copy, PartialEq)]
struct ExactCeiling {
    kind: LimitKind,
    role: ExactRole,
    ae_mm: f64,
    ap_mm: f64,
    /// P29-10 10g: the declared cut that gives the ceiling, when the solve
    /// names one (the force budget).
    cut: Option<CutKind>,
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
struct SolvedColumns(Vec<(f64, Solved)>);

/// P29-7 7c: a force-budget depth and the declared cut that gives it (the
/// lowest over the cuts that the plan always cuts).
type Solved = (BudgetDepth, CutKind);

impl SolvedColumns {
    fn get(&self, ae: f64) -> Option<Solved> {
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
        map.serialize_entry(
            "binding_cut",
            &self
                .proposal_binding
                .and_then(|kind| self.limit(kind))
                .and_then(|l| l.binding_cut)
                .map(CutKind::label),
        )?;
        map.serialize_entry("limit_rows", &self.rows)?;
        /// The two sets of marks on the wire.
        #[derive(Serialize)]
        struct Marks<'m> {
            proposal: Option<&'m crate::feeds::plane::PlaneMarks>,
            stated: Option<&'m crate::feeds::plane::PlaneMarks>,
        }
        map.serialize_entry(
            "plane_marks",
            &Marks {
                proposal: self.marks_at_proposal.as_ref(),
                stated: self.marks_at_stated.as_ref(),
            },
        )?;
        map.serialize_entry("force_budget_columns", &self.force_budget_columns())?;
        map.serialize_entry("outside_cut", &self.outside_cut().map(CutKind::label))?;
        map.serialize_entry("notes", &self.notes)?;
        map.serialize_entry("stated", &self.stated)?;
        map.serialize_entry("operator", &self.operator)?;
        map.end()
    }
}

/// P29-7 7c: the check of one declared cut at the proposal, before it
/// becomes a [`crate::limits::CheckRow`] (P29-10).
#[derive(Debug, Clone, PartialEq)]
struct PlannedRow {
    /// The declared cut.
    cut: CutKind,
    /// The exemption row of a cut that the plan may not cut
    /// (`Presence::MayOccur`, lead ruling Q1): its check is a warning row,
    /// never a ceiling. `None` for a cut that the plan always cuts.
    may_occur_row: Option<&'static str>,
    /// P29-8 8d: the own depth of a cut that is not on the body plane (an
    /// entry) and its rule. The check then holds the own depth as its
    /// value, against the cut's root (a value check, never a ceiling).
    own_depth: Option<crate::compute::catalog::OwnDepth>,
    /// P29-8 8d: why the entry rule gives the cut no feed before the
    /// simulation (`EntryNoNumber::clause`), or `None`.
    entry_no_number: Option<&'static str>,
    /// P29-9 9b: the value is a lower bound of the cut.
    lower_bound: bool,
    /// P29-9: the chip reads a stored entry feed.
    stored_feed: bool,
    /// P29-10 PB: the chip floor is a reference, with the reason.
    advisory_floor: Option<crate::tool_load::verdict::AdvisoryLowSide>,
    /// P29-10 10i: the point the row judges.
    judged_at: crate::limits::JudgedAt,
    /// P29-10 10i (ruling 5): a depth ceiling read at the suggested chip.
    at_suggested_chip: bool,
    /// P29-10 10i (ruling 3): the scope row of a cell the scope does not
    /// judge.
    scope_row: Option<&'static str>,
    /// The check.
    check: crate::limits::Check,
}

impl PlannedRow {
    /// The display row. `at_earlier_leave`: the body reads the earlier
    /// op's leave (P29-9 9c).
    fn into_row(
        self,
        at_earlier_leave: bool,
        at: Option<crate::limits::PlanePoint>,
        modulation: crate::dressup::feed_modulation::RunModulation,
    ) -> crate::limits::CheckRow {
        use crate::limits::{CheckState, LimitId};
        let reads_leave = at_earlier_leave
            && self.own_depth.is_none()
            && matches!(self.check.state, CheckState::Within | CheckState::Exceeds)
            && matches!(
                self.check.limit,
                LimitId::ForceBudget
                    | LimitId::ToolDeflection
                    | LimitId::VendorDepthTable
                    | LimitId::SpindlePower
            );
        let compared = compared_of_planned(self.check.limit);
        let modulation = if modulation.caps(self.check.limit, self.cut) {
            crate::limits::RowModulation::CapsTheBound
        } else {
            crate::limits::RowModulation::NotCapped
        };
        crate::limits::CheckRow::planned(
            self.cut,
            self.check,
            crate::limits::PlannedFacts {
                may_occur_row: self.may_occur_row,
                own_depth: self.own_depth.map(|o| crate::limits::PlannedOwnDepth {
                    mm: o.mm,
                    rule: o.rule.clause(),
                    worst_case: o.rule == crate::compute::catalog::OwnDepthRule::RampDepthPerPass,
                }),
                lower_bound: self.lower_bound,
                stored_feed: self.stored_feed,
                advisory_floor: self.advisory_floor,
                modulation,
                judged_at: self.judged_at,
                ceiling_chip: if self.at_suggested_chip {
                    crate::limits::CeilingChipSource::Suggested
                } else {
                    crate::limits::CeilingChipSource::OfThePoint
                },
                scope_row: self.scope_row,
                at_earlier_leave: reads_leave,
                entry_no_number: self.entry_no_number,
                compared,
                at,
            },
        )
    }
}

/// P29-10 (lead ruling 3): what a planned row of `limit` compares. A depth
/// ceiling compares the proposal depth (or an entry's own depth) with the
/// depth where the limit binds; the power ceiling and every value check
/// compare the limit's own quantity. One match on the plane role, no
/// wildcard.
fn compared_of_planned(limit: crate::limits::LimitId) -> crate::limits::Compared {
    use crate::limits::Compared;
    match planned_limit_of(limit) {
        PlaneRole::DepthCeiling(
            CeilingEdge::ForceBudget | CeilingEdge::Deflection | CeilingEdge::VendorDepthTable,
        ) => Compared::Depth,
        PlaneRole::DepthCeiling(CeilingEdge::Power)
        | PlaneRole::ValueCheck(_)
        | PlaneRole::NotACutLimit(_) => Compared::Load,
    }
}

/// P29-7 7c: THE one loop over every limit. Each limit states its role on
/// the plane (`planned::planned_limit_of`, one match with no wildcard); the
/// envelope draws the depth ceilings in the draw order of `CeilingEdge`
/// and writes the value checks. A new limit is read with no edit here.
fn plane_roles() -> (Vec<CeilingEdge>, Vec<ValueCheck>) {
    let mut ceilings = Vec::new();
    let mut checks = Vec::new();
    for limit in crate::limits::LimitId::ALL {
        match planned_limit_of(limit) {
            PlaneRole::DepthCeiling(edge) => ceilings.push(edge),
            PlaneRole::ValueCheck(check) => checks.push(check),
            PlaneRole::NotACutLimit(_) => {}
        }
    }
    ceilings.sort_unstable();
    checks.sort_unstable();
    (ceilings, checks)
}

/// The depth ceiling that draws the envelope kind `kind`, if one does.
fn ceiling_edge_of(kind: LimitKind) -> Option<CeilingEdge> {
    match planned_limit_of(crate::limits::LimitId::of_limit_kind(kind)) {
        PlaneRole::DepthCeiling(edge) => (edge.kind() == kind).then_some(edge),
        PlaneRole::ValueCheck(_) | PlaneRole::NotACutLimit(_) => None,
    }
}

/// P29-10 10i: the speeds of the chip and power rows at a judged point.
struct PointSpeeds {
    /// The calculator at the point: the chip band, the matched row and the
    /// vertical-rate limit.
    r: FeedsResult,
    /// The spindle speed (rpm), the feed (mm/min) and the rated power (kW)
    /// at that speed; `None` when the op states no feed or speed (the
    /// stated point, ruling 5).
    rpm_feed_kw: Option<(f64, f64, f64)>,
    /// The body chip (mm per tooth), or `None` (ruling 5).
    fz: Option<f64>,
    /// The chip interval of the stored op feed (the stated point).
    stored: Option<StoredFeedChip>,
}

/// P29-10 10i: the chip a depth ceiling reads at a judged point.
#[derive(Debug, Clone, Copy, PartialEq)]
enum CeilingChip {
    /// The proposal: the curve chip, and for the force budget the lower
    /// ceiling of the curve chip and the pick chip (`judged_ceiling`).
    CurveAndPick,
    /// The stated chip (the stored op feed over the stated speed).
    Stated(f64),
    /// Ruling 5: the op states no feed or speed: the chip of the drawn
    /// edge, and the row says so (`Caveat::AtSuggestedChip`).
    Suggested(f64),
}

/// **P29-10 10i (lead ruling L1): the point at which the declared cuts
/// are judged.** One judge, two points: `read_declared_cuts` takes this as
/// its one argument, and its two constructors are the only places that
/// differ: [`SafeEnvelope::at_proposal`] and [`SafeEnvelope::at_stated`].
struct JudgedPoint {
    at: crate::limits::JudgedAt,
    /// The point (depth, width).
    point: CutPoint,
    /// The depth of the body plane at the point; `None` with no depth
    /// before the simulation.
    plane_ap: Option<f64>,
    /// The chip of the depth ceilings.
    ceiling_chip: CeilingChip,
    /// The speeds of the chip and power rows; `None` gives no such rows
    /// (the proposal with no commanded chip, as before 10i).
    speeds: Option<PointSpeeds>,
}

/// The rows of one planned limit at the proposal, with their unit.
struct CutRows {
    unit: &'static str,
    rows: Vec<CutRow>,
}

/// One declared cut against one planned limit at the proposal.
struct CutRow {
    cut: CutKind,
    presence: crate::compute::catalog::Presence,
    /// P29-8 8d: the body plane, or the cut's own depth.
    depth: PlaneDepth,
    value: Option<f64>,
    /// P29-9 task 1: the value that the state judges, when it is not
    /// `value`: the end of a stored entry feed's chip interval toward the
    /// inside (`entry_rule::StoredFeedChip`). The margin and the state read
    /// it; the record keeps `value`.
    judged: Option<f64>,
    /// The bound, `Ok(None)` with no bound (the cut is inside), or the
    /// reason for no number.
    bound: Result<Option<f64>, crate::limits::NoNumberReason>,
    /// P29-9 9b: the value is a lower bound of the cut (`BodyDepth::
    /// AtLeast`): a row inside reads `NoNumber(OnlyALowerBound)`.
    lower_bound: bool,
    /// P29-8e (c): the cut has no number by its geometry (a cut with no
    /// vertical feed): the row reads `NotApplicable`.
    geometry: Option<crate::limits::Geometry>,
    /// P29-8e (Q11): the basis of this row's own bound, when it differs by
    /// row (the arm of the vertical-rate limit); else the check's basis.
    own_basis: Option<Basis>,
    /// P29-10: the chip reads a stored entry feed.
    stored_feed: bool,
    /// P29-10 PB: the chip floor of the row is a reference (its low side is
    /// advisory), with the reason.
    advisory_floor: Option<crate::tool_load::verdict::AdvisoryLowSide>,
    /// P29-10 10i (ruling 5): a depth ceiling read at the suggested chip.
    at_suggested_chip: bool,
    /// P29-10 10i (ruling 3): the scope row of a cell the scope does not
    /// judge.
    scope_row: Option<&'static str>,
}

impl CutRow {
    fn of(
        cut: CutKind,
        presence: crate::compute::catalog::Presence,
        depth: PlaneDepth,
        value: Option<f64>,
        bound: Result<Option<f64>, crate::limits::NoNumberReason>,
    ) -> Self {
        Self {
            cut,
            presence,
            depth,
            value,
            judged: None,
            bound,
            lower_bound: false,
            geometry: None,
            own_basis: None,
            stored_feed: false,
            advisory_floor: None,
            at_suggested_chip: false,
            scope_row: None,
        }
    }

    /// P29-10 10i: the row of a cell; a cell that the scope does not judge
    /// carries its row of `EXEMPTIONS` (ruling 3: the drill cycle names
    /// `P28-20`, not `S-NotJudgedHere`).
    fn of_cell<A>(
        cut: CutKind,
        presence: crate::compute::catalog::Presence,
        depth: PlaneDepth,
        value: Option<f64>,
        cell: &crate::feeds::planned::PlannedCell<A>,
        bound: Result<Option<f64>, crate::limits::NoNumberReason>,
    ) -> Self {
        use crate::feeds::planned::PlannedCell;
        let scope_row = match cell {
            PlannedCell::NotJudged(n) => Some(n.row),
            PlannedCell::Judged(_) | PlannedCell::NotACutLimit(_) => None,
        };
        Self {
            scope_row,
            ..Self::of(cut, presence, depth, value, bound)
        }
    }

    /// P29-10 10i (ruling 5): the row of a depth ceiling read at the
    /// suggested chip.
    fn at_suggested_chip(self, at_suggested_chip: bool) -> Self {
        Self {
            at_suggested_chip,
            ..self
        }
    }

    /// P29-8e (Q11): the row with the basis of its own bound.
    fn with_basis(self, own_basis: Option<Basis>) -> Self {
        Self { own_basis, ..self }
    }

    /// P29-8e (c): the row of a cut that the limit does not apply to by its
    /// geometry.
    fn not_applicable(self, geometry: crate::limits::Geometry) -> Self {
        Self {
            geometry: Some(geometry),
            ..self
        }
    }

    /// P29-9 9b: the row of a value that is a lower bound of the cut.
    fn at_lower_bound(self, lower_bound: bool) -> Self {
        Self {
            lower_bound,
            ..self
        }
    }

    /// P29-9 task 1: the row with the value that its state judges.
    fn judged_at(self, judged: Option<f64>) -> Self {
        Self { judged, ..self }
    }

    /// P29-10: the row whose chip reads a stored entry feed.
    fn with_stored_feed(self, stored_feed: bool) -> Self {
        Self {
            stored_feed,
            ..self
        }
    }

    /// P29-10 PB: the row whose chip floor is a reference.
    fn with_advisory_floor(
        self,
        advisory_floor: Option<crate::tool_load::verdict::AdvisoryLowSide>,
    ) -> Self {
        Self {
            advisory_floor,
            ..self
        }
    }

    /// The check record of the row.
    fn check(
        &self,
        judged_at: crate::limits::JudgedAt,
        limit: crate::limits::LimitId,
        unit: &'static str,
        side: crate::limits::BoundSide,
        basis: Option<Basis>,
    ) -> PlannedRow {
        use crate::limits::{Check, CheckState, NoNumberReason};
        let (bound, margin, state) = match (self.geometry, self.bound) {
            (Some(geometry), _) => (None, None, CheckState::NotApplicable(geometry)),
            (None, bound) => match bound {
                Ok(Some(b)) => {
                    // P29-9 (lead ruling): the margin follows the judged value,
                    // so a row never reads `Within` with a margin below zero.
                    let margin = self.judged.or(self.value).and_then(|v| side.margin(v, b));
                    let state = margin.map_or(
                        CheckState::NoNumber(NoNumberReason::NotModelled),
                        CheckState::of_margin,
                    );
                    match (self.lower_bound, state) {
                        // P29-9 9b: a lower bound inside its limit proves
                        // nothing (the real cut is at least as deep).
                        (true, CheckState::Within) => (
                            Some(b),
                            None,
                            CheckState::NoNumber(NoNumberReason::OnlyALowerBound),
                        ),
                        (
                            true | false,
                            CheckState::Within
                            | CheckState::Exceeds
                            | CheckState::NoNumber(_)
                            | CheckState::NotApplicable(_),
                        ) => (Some(b), margin, state),
                    }
                }
                // No bound (no root below the flute, a table that holds every
                // printed depth): the cut is inside; a lower bound proves
                // nothing (9b).
                Ok(None) if self.lower_bound => (
                    None,
                    None,
                    CheckState::NoNumber(NoNumberReason::OnlyALowerBound),
                ),
                Ok(None) => (None, None, CheckState::Within),
                Err(reason) => (None, None, CheckState::NoNumber(reason)),
            },
        };
        PlannedRow {
            cut: self.cut,
            may_occur_row: match self.presence {
                crate::compute::catalog::Presence::Always => None,
                crate::compute::catalog::Presence::MayOccur(may) => Some(may.row),
            },
            own_depth: match self.depth {
                PlaneDepth::Body | PlaneDepth::Axial => None,
                PlaneDepth::Own(own) => Some(own),
            },
            entry_no_number: None,
            lower_bound: self.lower_bound,
            stored_feed: self.stored_feed,
            advisory_floor: self.advisory_floor,
            judged_at,
            at_suggested_chip: self.at_suggested_chip,
            scope_row: self.scope_row,
            check: Check {
                limit,
                value: self.value,
                bound,
                unit,
                side,
                margin,
                state,
                basis: self.own_basis.clone().or(basis),
                binding: false,
                population: None,
            },
        }
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
    writes_depth: bool,
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

    /// P16: `true` when the apply writes the picked depth: the depth axis
    /// is free, and a proposal pick has a proposed depth. An apply never
    /// writes a depth that the envelope did not propose.
    #[must_use]
    pub const fn writes_depth(&self) -> bool {
        self.writes_depth
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
        let feeds_input = FeedsInput::of_operation(
            input.operation,
            input.tool,
            input.material,
            input.machine,
            input.lut,
            input.spindle_strategy,
        );
        crate::feeds::validate_tool_for_operation(&feeds_input)?;
        // P29-8 8d (lead ruling Q5): the declared cuts with the toolpath's
        // configured entry (the Suggest context's dressups, else the
        // registry default). The calculator's own list reads the registry
        // default; both lists hold the same cuts on the body plane, and only
        // a cut on the body plane sets the pre-sim power
        // (`planned::sets_a_ceiling`).
        let entry = crate::feeds::entry_rule::ConfiguredEntry::of_operation(
            input.operation,
            input.context.dressups(),
        );
        // P29-9 9b: the facts of the plan (the setup frame of a Project
        // Curve from below) from the Suggest context.
        let declared = DeclaredCuts::of_operation_with(
            input.operation,
            input.tool,
            &entry,
            input.context.plan_facts(),
        );
        let vertical = crate::feeds::planned::VerticalFeeds::of_operation(
            input.operation,
            input.tool,
            input.material,
            input.context.dressups(),
            &entry,
        );
        let inputs = OwnedInputs {
            operation: input.operation.clone(),
            tool: input.tool.clone(),
            machine: input.machine.clone(),
            material: input.material.clone(),
            lut: input.lut,
            spindle_strategy: input.spindle_strategy,
            cuts: declared,
            vertical,
            modulation: input.modulation,
        };
        // The calculator at the op's own inputs: the family default of an
        // unstated axis, and the matched row.
        let base = calculate(&feeds_input);
        let d = input.tool.diameter;
        let plan = axes_plan(input.operation, input.tool, &base, &inputs.cuts);
        let (_, role) = input.operation.feeds_style();

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
        // P20i: the declared cuts the calculator reads at each point.
        let cuts = declared;
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
                    &cuts,
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
        // P29-7 7c: every limit that changes with the cut draws its edge
        // through its role (`planned::planned_limit_of`), in the draw order
        // of `CeilingEdge`. A new role does not compile until each match
        // below states it.
        let (ceilings, value_checks) = plane_roles();
        let mut budget = None;
        for &edge in &ceilings {
            match edge {
                CeilingEdge::VendorDepthTable => vendor_depth_limit(
                    &mut boundary,
                    (&input, &inputs),
                    &base,
                    &speeds,
                    &plan,
                    (operator, Drawn::Yes),
                ),
                CeilingEdge::Power => power_limit(
                    &mut boundary,
                    &inputs,
                    &speeds,
                    &plan,
                    base.power_case,
                    operator,
                ),
                CeilingEdge::Deflection => {
                    deflection_limit(&mut boundary, &inputs, &speeds, &plan, operator);
                }
                CeilingEdge::ForceBudget => {
                    budget = rigidity_limit(&mut boundary, &inputs, &speeds, &plan, operator);
                }
            }
        }
        let mut runtime_notes = Vec::new();
        let move_budget = runtime_limit(&mut boundary, &mut runtime_notes, &input, &plan, d);
        per_point_limits(&mut boundary, &base, &speeds);
        // The chip-sized edges at the suggested chip, when the boundary is
        // drawn at the operator's chip (a pick writes the suggested speeds).
        let mut pick_limits = Vec::new();
        if operator.is_some() {
            for &edge in &ceilings {
                match edge {
                    CeilingEdge::VendorDepthTable => vendor_depth_limit(
                        &mut pick_limits,
                        (&input, &inputs),
                        &base,
                        &speeds,
                        &plan,
                        (None, Drawn::No),
                    ),
                    CeilingEdge::Power => power_limit(
                        &mut pick_limits,
                        &inputs,
                        &speeds,
                        &plan,
                        base.power_case,
                        None,
                    ),
                    CeilingEdge::Deflection => {
                        deflection_limit(&mut pick_limits, &inputs, &speeds, &plan, None);
                    }
                    // The exact solver reads the pick chip
                    // (`budget_depth_at`, `BudgetChip::Pick`).
                    CeilingEdge::ForceBudget => {}
                }
            }
        }

        let base_point = CutPoint {
            ap_mm: base.axial_depth_mm,
            ae_mm: base.radial_width_mm,
        };
        let mut env = Self {
            axes: plan.axes,
            boundary,
            speeds,
            proposal: Sourced::new(
                Proposal {
                    depth: ProposedDepth::Mm(base_point.ap_mm),
                    ae_mm: base_point.ae_mm,
                },
                Basis::derived("not computed", Vec::new()),
            ),
            proposal_point: base_point,
            proposal_binding: None,
            proposal_limits: Vec::new(),
            notes: Vec::new(),
            planned: Vec::new(),
            rows: Vec::new(),
            marks_at_proposal: None,
            marks_at_stated: None,
            ceilings,
            value_checks,
            stated: None,
            operator: None,
            inputs,
            budget,
            pick_limits,
            exact: Vec::new(),
            move_budget,
            operator_fz: operator.map(|c| c.fz_mm),
            table_band_max: base
                .chipload_bounds
                .map(|b| b.max_mm_per_tooth)
                .or(base.chipload_point_mm)
                .filter(|v| v.is_finite() && *v > 0.0)
                .filter(|_| base.matched_lut_row.is_some() && plan.axes.ap_free() && !plan.peck),
        };
        let peck_rule = plan.peck.then(|| drill_peck_rule(&input)).flatten();
        // P16 (review D5, D7): the envelope proposes no depth that it cannot
        // model. A surface-offset op has no depth before the simulation; a
        // material with no force line has no limit that models the depth
        // (a drill peck is a drill rule, not a force depth).
        // P29-9 9b: the checks judge a Project Curve at the lower bound of
        // its depth (`BodyDepth::AtLeast`); no proposal moves.
        let no_depth = if input.operation.cut_spec().axial.depth_after_simulation() {
            Some(NoDepthProposal::AfterSimulation)
        } else if plan.axes.ap_free() && !plan.peck && input.material.force_line().is_err() {
            Some(NoDepthProposal::NoForceLine)
        } else {
            None
        };
        env.add_exact_ceilings(&plan, ExactPhase::Stated);
        // EXEMPT(P28-05, P28-06)
        let removal_rate = matches!(role, PassRole::Roughing)
            && !plan.peck
            && no_depth.is_none()
            && (plan.computed_width || matches!(env.axes, EnvelopeAxes::AxialBar { .. }));
        let (point, binding, limits) = if removal_rate {
            let pick = env.propose_removal_rate(&plan);
            (pick.point, pick.limits.first().copied(), pick.limits)
        } else {
            let proposed = env.propose(&plan, &base, peck_rule);
            match (no_depth, plan.computed_width) {
                // The P13c add-door rule: with no force answer the computed
                // width is the corner width (`geometry_width`), never above
                // the stated width (K2).
                (Some(NoDepthProposal::NoForceLine), true) => {
                    let (point, binding, limits) =
                        env.geometry_width_proposal(&proposed.point, &plan, d);
                    (point, binding, limits)
                }
                // With no proposed depth, a depth limit binds nothing.
                (Some(NoDepthProposal::NoForceLine), false)
                | (Some(NoDepthProposal::AfterSimulation), _) => {
                    let binding = proposed.ae_binding;
                    (proposed.point, binding, binding.into_iter().collect())
                }
                (None, _) => {
                    let binding = proposed.ap_binding.or(proposed.ae_binding);
                    (proposed.point, binding, binding.into_iter().collect())
                }
            }
        };
        env.proposal_point = point.get();
        env.proposal = Sourced::new(
            Proposal {
                depth: match no_depth {
                    Some(reason) => ProposedDepth::None(reason),
                    None => ProposedDepth::Mm(point.get().ap_mm),
                },
                ae_mm: point.get().ae_mm,
            },
            point.basis().clone(),
        );
        env.proposal_binding = binding;
        env.proposal_limits = limits;
        if let Some(reason) = no_depth {
            env.notes.push(EnvelopeNote::NoDepth(reason));
        }
        env.add_exact_ceilings(&plan, ExactPhase::Proposal);
        let proposal = env.at_proposal();
        env.planned = env.read_declared_cuts(&proposal);
        env.notes.extend(runtime_notes);
        if plan.computed_width {
            env.notes.push(EnvelopeNote::ComputedWidth);
        }
        if env
            .limit(LimitKind::Rigidity)
            .is_some_and(|l| l.law == crate::feeds::force_budget::FORCE_BUDGET_FLOOR_LAW)
        {
            env.notes.push(EnvelopeNote::ForceBudgetFloor);
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
        env.write_rows();
        if let Some(note) = env.move_budget_conflict_note() {
            env.notes.push(note);
        }
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

    /// P19: the `ap` ceiling of the limit of `kind` at the width `ae_mm`
    /// that [`Self::placement_of`] judges a point against. At the stated
    /// width it is the exact value, and at the proposal width the force
    /// budget at the pick chip where that is lower; elsewhere it is the
    /// drawn edge ([`Limit::ap_ceiling_at`]). The drawn edge holds no
    /// sample at those widths, so it does not move with the stated cut.
    #[must_use]
    pub fn judged_ap_ceiling(&self, kind: LimitKind, ae_mm: f64) -> Option<f64> {
        self.judged_ceiling(self.limit(kind)?, ae_mm)
    }

    /// K7 (feeds contract P9): the value of the measured point `p` that the
    /// limit of `kind` judges, and the limit there, by the rules of
    /// [`Self::placement_of_measured`]. `None` for a limit with no edge
    /// (judged per point, or unmodelled), or no limit of that kind.
    /// P20i: `Power` gives the gate's figure of the sample (kW, at the
    /// gate's feed), with or without a drawn Power edge.
    #[must_use]
    pub fn judged_value(
        &self,
        kind: LimitKind,
        p: CutPoint,
        tool: MeasuredTool,
    ) -> Option<JudgedValue> {
        if kind == LimitKind::Power {
            let power = tool.power?;
            return Some(JudgedValue {
                axis: "power",
                measured: power.required_kw,
                limit: power.available_kw,
                unit: "kW",
                feed_mm_min: Some(power.feed_mm_min),
            });
        }
        let l = self.limit(kind)?;
        let edge = Self::measured_edge(l, tool);
        let judged = Limit { edge, ..l.clone() };
        let (axis, measured_mm, limit_mm) = self.axis_reading(&judged, p)?;
        Some(JudgedValue {
            axis,
            measured: measured_mm,
            limit: limit_mm,
            unit: "mm",
            feed_mm_min: None,
        })
    }

    /// The axis, the value and the bound (mm) of `p` on the edge of
    /// `judged`: the depth against an `ap` edge, the width against an `ae`
    /// edge. `None` for an edge that judges no point.
    fn axis_reading(&self, judged: &Limit, p: CutPoint) -> Option<(&'static str, f64, f64)> {
        Some(match judged.edge {
            Edge::ApCeiling { .. } | Edge::ApCurve { .. } => {
                ("depth", p.ap_mm, self.judged_ceiling(judged, p.ae_mm)?)
            }
            Edge::ApFloor { mm } => ("depth", p.ap_mm, mm),
            Edge::AeCeiling { mm } | Edge::AeFloor { mm } => ("width", p.ae_mm, mm),
            Edge::AeCurve { .. } => (
                "width",
                p.ae_mm,
                judged
                    .ae_bound_at(p.ap_mm)
                    .map(|(v, _)| v)
                    .filter(|v| v.is_finite())?,
            ),
            Edge::PerPoint | Edge::Unmodelled { .. } => return None,
        })
    }

    /// The machine the envelope is drawn for. P20i: the measured placement
    /// reads its rated power curve, as the power gate does.
    #[must_use]
    pub const fn machine(&self) -> &MachineProfile {
        &self.inputs.machine
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
    /// - P20i: power is judged by the gate's figure of the sample
    ///   (`tool.power`: the sample engagement at the feed the gate reads,
    ///   against the rated power at the sample RPM), not by the drawn Power
    ///   edge. The drawn edge is a pre-simulation picture at the op's stated
    ///   feed and at the worst `PowerCase` of the op (the slot of a pocket,
    ///   the planner cap of a 3D Rough); a measured sample has its own arc
    ///   and its own feed. With no figure the point's power is not judged.
    ///
    /// - A floor (the move budget, the engagement floor) is a rule for the
    ///   operator's stated point; a simulation sample with a lighter cut than
    ///   the plan crosses no limit, so the floors do not place it.
    /// - The `Diameter` ceiling is the engaged diameter at the sample depth
    ///   (`tool.d_eng_mm`, from `Engagement::d_eng_mm`), not the nominal
    ///   diameter: a tapered ball or a V-bit cuts wider than its tip as the
    ///   cone engages (lead decision, 2026-11-05). For a flat tool the two
    ///   are equal.
    #[must_use]
    pub fn placement_of_measured(&self, p: CutPoint, tool: MeasuredTool) -> Placement {
        self.place(p, Some(tool))
    }

    /// The ceiling of `l` that a measured point of `tool` meets: the edge
    /// itself, or the `Diameter` ceiling at the engaged diameter.
    fn measured_edge(l: &Limit, tool: MeasuredTool) -> Edge {
        match (l.kind, &l.edge) {
            (LimitKind::Diameter, Edge::AeCeiling { .. }) => Edge::AeCeiling { mm: tool.d_eng_mm },
            (_, edge) => edge.clone(),
        }
    }

    fn place(&self, p: CutPoint, measured: Option<MeasuredTool>) -> Placement {
        let mut inside = f64::INFINITY;
        let mut worst: Option<(f64, LimitKind)> = None;
        self.each_placing_margin(p, measured, |m, kind, _| {
            if m < -1e-9 {
                if worst.is_none_or(|(w, _)| m < w) {
                    worst = Some((m, kind));
                }
            } else {
                inside = inside.min(m);
            }
        });
        match worst {
            Some((margin, binding)) => Placement::Outside { binding, margin },
            None => Placement::Inside {
                margin: if inside.is_finite() { inside } else { 1.0 },
            },
        }
    }

    /// Every margin that places the point `p`, in the order [`Self::place`]
    /// folds them: each limit of the boundary that judges `p` (with its
    /// index in `boundary`), then the gate's power of a measured point,
    /// then the exact move budget (both with no index). P29-2:
    /// `limits::project` reads the same margins, so a `Check` and the
    /// `Placement` cannot judge a point by two rules.
    fn each_placing_margin(
        &self,
        p: CutPoint,
        measured: Option<MeasuredTool>,
        mut each: impl FnMut(f64, LimitKind, Option<usize>),
    ) {
        for (index, l) in self.boundary.iter().enumerate() {
            let judged;
            let l = match measured {
                None => l,
                Some(tool) => {
                    judged = Limit {
                        edge: Self::measured_edge(l, tool),
                        ..l.clone()
                    };
                    &judged
                }
            };
            let is_floor = l.edge.is_floor();
            if measured.is_some() && is_floor {
                continue;
            }
            // P20i: a measured point's power is the gate's figure (below).
            if measured.is_some() && l.kind == LimitKind::Power {
                continue;
            }
            // A fixed axis is not the operator's choice: its edges do not
            // place the point.
            let on_fixed_axis = match l.edge {
                Edge::ApCeiling { .. } | Edge::ApCurve { .. } | Edge::ApFloor { .. } => {
                    !self.axes.ap_free()
                }
                Edge::AeCeiling { .. } | Edge::AeFloor { .. } | Edge::AeCurve { .. } => {
                    !self.axes.ae_free()
                }
                Edge::PerPoint | Edge::Unmodelled { .. } => true,
            };
            if on_fixed_axis {
                continue;
            }
            // P19b: the drawn move-budget curve is a picture; the exact
            // per-point judgement below (`move_budget_margin`) places the
            // point.
            if l.kind == LimitKind::Runtime && self.move_budget.is_some() {
                continue;
            }
            let Some(m) = self.judged_margin(l, p) else {
                continue;
            };
            each(m, l.kind, Some(index));
        }
        // P20i: the power of a measured point, by the gate's figure.
        if let Some(m) = measured.and_then(|t| t.power).and_then(|p| p.margin()) {
            each(m, LimitKind::Power, None);
        }
        // P19: a `Runtime` limit with no edge still judges the point by the
        // predicted moves at its own depth and width. A floor does not place
        // a measured point (see `placement_of_measured`).
        if measured.is_none()
            && let Some(m) = self.move_budget_margin(p)
        {
            each(m, LimitKind::Runtime, None);
        }
    }

    /// P29-2: the margins that place the planned point `p`
    /// ([`Self::each_placing_margin`], no measured tool): the limit kind,
    /// the margin and the index of the limit in `boundary` (`None` for the
    /// exact move budget).
    #[must_use]
    pub(crate) fn placing_margins(&self, p: CutPoint) -> Vec<(LimitKind, f64, Option<usize>)> {
        let mut out = Vec::new();
        self.each_placing_margin(p, None, |m, kind, index| out.push((kind, m, index)));
        out
    }

    /// P29-2: the value and the bound of the planned point `p` on the limit
    /// `l` of this envelope (the reading of [`Self::judged_value`] with no
    /// measured tool): `(value, bound)` in mm.
    #[must_use]
    pub(crate) fn planned_reading(&self, l: &Limit, p: CutPoint) -> Option<(f64, f64)> {
        self.axis_reading(l, p)
            .map(|(_, value, bound)| (value, bound))
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
            PickRequest::Proposal => {
                Sourced::new(self.proposal_point, self.proposal.basis().clone())
            }
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
        // P16: a proposal pick with no proposed depth writes no depth.
        let writes_depth = self.axes.ap_free()
            && match request {
                PickRequest::Proposal => self.proposal.get().ap_mm().is_some(),
                PickRequest::Point { .. } | PickRequest::Peck(_) => true,
            };
        let p = point.get();
        EnvelopePick {
            envelope: self,
            request,
            placement: self.placement_of(p),
            result: self.inputs.calc_at_recorded(p),
            point,
            writes_depth,
        }
    }

    /// P16 (review D5): the P13c add-door width of a computed-width op with
    /// no force line: the corner width (`geometry_default::geometry_width`),
    /// never above the stated width (K2). It returns the point, the limit
    /// that lowered the stated width, and the limits the width sits on.
    fn geometry_width_proposal(
        &self,
        point: &Sourced<CutPoint>,
        plan: &AxesPlan,
        d: f64,
    ) -> (Sourced<CutPoint>, Option<LimitKind>, Vec<LimitKind>) {
        let (width, width_basis) = crate::compute::geometry_default::geometry_width(d);
        let kind = LimitKind::CornerCoverage;
        let p = point.get();
        let (ae, binding, limits) = match plan.stated_ae {
            Some(stated) if stated <= width => {
                (Sourced::new(stated, Basis::stated()), None, Vec::new())
            }
            Some(stated) => (
                lower_to_cap(
                    Sourced::new(stated, Basis::stated()),
                    width,
                    crate::compute::geometry_default::GEOMETRY_WIDTH_LAW,
                    &width_basis,
                ),
                Some(kind),
                vec![kind],
            ),
            None => (Sourced::new(width, width_basis), None, vec![kind]),
        };
        let basis = Basis::derived(
            "proposal: corner width, no depth (no force line)",
            vec![ae.basis().clone()],
        );
        (
            Sourced::new(
                CutPoint {
                    ap_mm: p.ap_mm,
                    ae_mm: ae.get(),
                },
                basis,
            ),
            binding,
            limits,
        )
    }

    /// The proposal: `min(stated, binding cap)` on a stated axis (never a
    /// raise, K2), the calculator's point on an unstated axis.
    fn propose(&self, plan: &AxesPlan, base: &FeedsResult, peck_rule: Option<f64>) -> Proposed {
        // P19b: a width ceiling that moves with the depth is read at the
        // stated depth, else the calculator depth.
        let ap_for_width = plan.stated_ap.unwrap_or(base.axial_depth_mm);
        let ae_cap = self
            .boundary
            .iter()
            .filter_map(|l| match &l.edge {
                Edge::AeCeiling { mm } => Some((*mm, l)),
                Edge::AeCurve {
                    bound: CurveBound::Ceiling,
                    points,
                } => ae_curve_at(CurveBound::Ceiling, points, ap_for_width).map(|mm| (mm, l)),
                Edge::ApCeiling { .. }
                | Edge::ApCurve { .. }
                | Edge::ApFloor { .. }
                | Edge::AeFloor { .. }
                | Edge::AeCurve {
                    bound: CurveBound::Floor,
                    ..
                }
                | Edge::PerPoint
                | Edge::Unmodelled { .. } => None,
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let mut ae_binding = None;
        let mut ap_binding = None;
        let ae = match (self.axes, plan.stated_ae) {
            (EnvelopeAxes::Plane { .. } | EnvelopeAxes::RadialBar { .. }, Some(stated)) => {
                let s = Sourced::new(stated, Basis::stated());
                match ae_cap {
                    Some((cap, l)) if cap < stated => {
                        ae_binding = Some(l.kind);
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
                // P19: the stated width reads its exact ceiling
                // (`judged_ceiling`), not the drawn interpolation.
                let cap = self
                    .boundary
                    .iter()
                    .filter_map(|l| self.judged_ceiling(l, ae_mm).map(|c| (c, l)))
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                match cap {
                    Some((c, l)) if c < s.get() => {
                        ap_binding = Some(l.kind);
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
        Proposed {
            point: Sourced::new(point, basis),
            ae_binding,
            ap_binding,
        }
    }
}

/// The result of [`SafeEnvelope::propose`]: the point and the limit that
/// lowered each stated axis.
struct Proposed {
    point: Sourced<CutPoint>,
    ae_binding: Option<LimitKind>,
    ap_binding: Option<LimitKind>,
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
    /// exactly (not interpolated between grid columns), and the declared
    /// cut that gives it (P29-7 7c: the lowest over the declared cuts,
    /// [`budget_solve`]). `None` when the op has no budget edge or the model
    /// cannot answer.
    fn budget_depth_at(&self, ae_mm: f64, chip: BudgetChip) -> Option<Solved> {
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
        budget_solve(&self.inputs, &tool, ae_mm, fz, model.budget_n)
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
    /// closed form (`vendor_depth_limit` at one width, the lowest over the
    /// declared cuts), or the span top where the table holds every printed
    /// depth.
    fn table_ceiling_at(&self, ae_mm: f64, fz: f64) -> Option<f64> {
        let band_max = self.table_band_max?;
        let env = TableEnv {
            band_max_mm: band_max,
            d_mm: self.inputs.tool.diameter,
            top_mm: self.speeds.aps.last().copied().unwrap_or(MIN_AP_MM),
        };
        Some(table_solve(&self.inputs.cuts, ae_mm, fz, &env).0.0)
    }

    /// The tip-deflection ceiling at `ae_mm` and the chip `fz` (the one
    /// solver of `deflection_limit`, the lowest over the declared cuts), or
    /// the span top with no root.
    fn deflection_ceiling_at(&self, ae_mm: f64, fz: f64) -> Option<f64> {
        let tool = crate::compute::cutter::build_cutter(&self.inputs.tool).ok()?;
        let top = self.speeds.aps.last().copied().unwrap_or(MIN_AP_MM);
        Some(deflection_solve(&self.inputs, &tool, ae_mm, fz).map_or(top, |(mm, _)| mm))
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
                let depth_edge = matches!(l.edge, Edge::ApCurve { .. } | Edge::ApCeiling { .. });
                let drawn = || l.ap_ceiling_at(ae_mm);
                // P29-7 7c: the exact solve of each depth ceiling, through
                // its role; a power edge and every other limit read the
                // drawn edge.
                let exact = match ceiling_edge_of(l.kind) {
                    Some(CeilingEdge::ForceBudget) if depth_edge && self.budget.is_some() => {
                        let top = self.speeds.aps.last().copied().unwrap_or(MIN_AP_MM);
                        match self.budget_depth_at(ae_mm, BudgetChip::Pick) {
                            Some((d, _)) => Some(d.ceiling_mm().unwrap_or(top)),
                            None => drawn(),
                        }
                    }
                    Some(CeilingEdge::VendorDepthTable) if depth_edge => {
                        self.table_ceiling_at(ae_mm, pick_fz())
                    }
                    Some(CeilingEdge::Deflection) if matches!(l.edge, Edge::ApCurve { .. }) => {
                        self.deflection_ceiling_at(ae_mm, pick_fz())
                    }
                    Some(
                        CeilingEdge::ForceBudget
                        | CeilingEdge::VendorDepthTable
                        | CeilingEdge::Deflection
                        | CeilingEdge::Power,
                    )
                    | None => drawn(),
                };
                exact.map(|c| (c, l.kind))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }

    /// P13c (O1), P19: solve the ceilings at the stated width and the
    /// proposal width exactly, so a point at that width is placed against
    /// the solved value, not an interpolation. Both widths also take the
    /// pick's own force-budget depth (the chip it ships, at or under the
    /// curve depth), so the proposal sits on the edge it names. P29-7 7c
    /// (trap 1): before 7c only the proposal width took it, so `propose`
    /// read the stated chip while the card judged at the pick chip.
    ///
    /// P19: the values go to `exact`, not to the drawn boundary. A sample
    /// at the stated or the proposal width in the drawn curve moved the
    /// drawn edge when the stated cut moved (the pick sample at another
    /// chip drew a notch).
    ///
    /// `phase` picks the set: `Stated` runs before the proposal (the
    /// proposal reads the stated width exactly, as the drawn sample gave it
    /// before P19), `Proposal` after it (it needs the proposal width).
    fn add_exact_ceilings(&mut self, plan: &AxesPlan, phase: ExactPhase) {
        let top = self.speeds.aps.last().copied().unwrap_or(MIN_AP_MM);
        let curve = |kind: LimitKind| {
            self.limit(kind)
                .is_some_and(|l| matches!(l.edge, Edge::ApCurve { .. }))
        };
        let width = |w: Option<f64>| w.filter(|ae| ae.is_finite() && *ae > 0.0);
        let at = match phase {
            ExactPhase::Stated => width(plan.stated_ae),
            ExactPhase::Proposal => width(Some(self.proposal_point.ae_mm)),
        };
        let Some(ae) = at else {
            return;
        };
        let mut exact = Vec::new();
        for edge in self.ceilings.clone() {
            match edge {
                CeilingEdge::ForceBudget => {
                    if curve(LimitKind::Rigidity) && self.budget.is_some() {
                        // P29-7 7c (trap 1, lead ruling Q6): the stated width
                        // reads the pick chip too, so `propose` reads the chip
                        // the card judges at (`max(stated chip, suggested
                        // chip)`).
                        let chips: &[(BudgetChip, ExactRole)] = &[
                            (BudgetChip::Curve, ExactRole::CurveChip),
                            (BudgetChip::Pick, ExactRole::Pick),
                        ];
                        for &(chip, role) in chips {
                            if let Some((d, cut)) = self.budget_depth_at(ae, chip) {
                                exact.push(ExactCeiling {
                                    kind: LimitKind::Rigidity,
                                    role,
                                    ae_mm: ae,
                                    ap_mm: d.ceiling_mm().unwrap_or(top),
                                    cut: Some(cut),
                                });
                            }
                        }
                    }
                }
                // The vendor depth table is drawn at the operator's chip when
                // the op states one; with none, the drawn columns read the
                // grid chip.
                CeilingEdge::VendorDepthTable => {
                    if curve(LimitKind::VendorDepthTable)
                        && let Some(fz) = self.operator_fz
                        && let Some(ap_mm) = self.table_ceiling_at(ae, fz)
                    {
                        exact.push(ExactCeiling {
                            kind: LimitKind::VendorDepthTable,
                            role: ExactRole::CurveChip,
                            ae_mm: ae,
                            ap_mm,
                            cut: None,
                        });
                    }
                }
                // The drawn edge judges the point: no exact solve.
                CeilingEdge::Power | CeilingEdge::Deflection => {}
            }
        }
        self.exact.extend(exact);
    }

    /// P19: the `ap` ceiling of `l` at the width `ae_mm` that a point is
    /// judged against: the drawn value, or the exact value at the stated
    /// and the proposal width, and the lower of that and the pick depth at
    /// the proposal width.
    fn judged_ceiling(&self, l: &Limit, ae_mm: f64) -> Option<f64> {
        let drawn = l.ap_ceiling_at(ae_mm)?;
        let at = |role: ExactRole| {
            self.exact
                .iter()
                .find(|e| e.kind == l.kind && e.role == role && (e.ae_mm - ae_mm).abs() < 1e-12)
                .map(|e| e.ap_mm)
        };
        let value = at(ExactRole::CurveChip).unwrap_or(drawn);
        Some(at(ExactRole::Pick).map_or(value, |pick| value.min(pick)))
    }

    /// P19: the signed margin of `p` against `l` (as [`Limit::margin`]),
    /// with an `ap` ceiling read by [`Self::judged_ceiling`].
    fn judged_margin(&self, l: &Limit, p: CutPoint) -> Option<f64> {
        match l.edge {
            Edge::ApCeiling { .. } | Edge::ApCurve { .. } => self
                .judged_ceiling(l, p.ae_mm)
                .and_then(|cap| (cap > 0.0).then(|| (cap - p.ap_mm) / cap)),
            Edge::ApFloor { .. }
            | Edge::AeCeiling { .. }
            | Edge::AeFloor { .. }
            | Edge::AeCurve { .. }
            | Edge::PerPoint
            | Edge::Unmodelled { .. } => l.margin(p),
        }
    }

    /// P19: the signed margin of `p` against the move budget, when the
    /// `Runtime` limit draws no edge: `budget / moves - 1`, negative when
    /// the predicted moves at the depth and width of `p` exceed the budget.
    /// `None` when the predictor gives no count.
    fn move_budget_margin(&self, p: CutPoint) -> Option<f64> {
        let judge = self.move_budget?;
        let mut op = self.inputs.operation.clone();
        let _ = op.set_stepover(p.ae_mm);
        if self.axes.ap_free() {
            let _ = op.set_depth_per_pass(p.ap_mm);
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "a move count; the budget is 5e5, exact in an f64"
        )]
        let moves = crate::feeds::predict::predict_move_count(
            &op,
            Some(&judge.bbox),
            &self.inputs.tool,
            &self.inputs.machine,
        ) as f64;
        (moves > 0.0).then(|| judge.target / moves - 1.0)
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

    /// The chip that a drawn edge reads at the width `ae_mm`: the
    /// operator's chip, else the suggested chip at the shallowest depth.
    fn curve_fz(&self, ae_mm: f64) -> f64 {
        self.operator_fz.unwrap_or_else(|| {
            chip_of(
                &self.inputs.calc_at(CutPoint {
                    ap_mm: MIN_AP_MM,
                    ae_mm,
                }),
                self.inputs.tool.flute_count_planning(),
            )
        })
    }

    /// P29-7 7c: the binding cut of each depth ceiling and the check of
    /// each declared cut at the proposal, for every planned role
    /// (`self.ceilings` and `self.value_checks`, from [`plane_roles`]). One
    /// match per role, no wildcard: a new role does not compile until it
    /// states its rows.
    ///
    /// - A depth ceiling gives one row per declared cut: the proposal
    ///   depth against the ceiling of the cut (the force budget at the
    ///   lower of the curve chip and the pick chip, the ceiling that
    ///   `judged_ceiling` reads; the deflection and the vendor depth table
    ///   at the curve chip); the power gives the power of each cut at the
    ///   speeds of the proposal against the rated power. The binding cut is
    ///   the cut with the lowest ceiling (the power: the highest power) over
    ///   the cuts that the plan always cuts.
    /// - A value check gives one row per declared cut: the planned peak
    ///   chip of the cut at the speeds of the proposal against the bound.
    ///
    /// P29-10 10i (lead ruling L1): ONE judge, two points. `jp` is the
    /// proposal ([`Self::at_proposal`], part two of the drawer) or the
    /// stated cut ([`Self::at_stated`], part one); every row writer reads
    /// the point, its depth, its chip and its speeds from `jp` only. Only
    /// the proposal names the binding cut of an edge.
    fn read_declared_cuts(&mut self, jp: &JudgedPoint) -> Vec<PlannedRow> {
        let tool = crate::compute::cutter::build_cutter(&self.inputs.tool).ok();
        let mut out = Vec::new();
        for edge in self.ceilings.clone() {
            let Some(rows) = self.ceiling_rows(edge, tool.as_ref(), jp) else {
                // P29-9 9c: a limit that draws no edge for a reason of scope
                // says so on the record, one row per declared cut (no op has
                // a silent missing value). No number moves.
                if let Some(reason) = self.no_edge_reason(edge) {
                    out.extend(self.inputs.cuts.list().iter().map(|c| {
                        let depth = PlaneDepth::of_declared(c.depth);
                        // A cut at its own depth holds that depth, as on
                        // every depth row (P29-8 8d).
                        let value = match depth {
                            PlaneDepth::Own(own) => Some(own.mm),
                            PlaneDepth::Body | PlaneDepth::Axial => None,
                        };
                        CutRow::of(c.kind, c.presence, depth, value, Err(reason)).check(
                            jp.at,
                            edge.limit(),
                            "mm",
                            BoundSide::Ceiling,
                            None,
                        )
                    }));
                }
                continue;
            };
            let always = || {
                rows.rows
                    .iter()
                    .filter(|r| crate::feeds::planned::sets_a_ceiling(r.presence, r.depth))
            };
            let first_by = |key: &dyn Fn(&CutRow) -> Option<f64>| {
                let mut best: Option<(f64, CutKind)> = None;
                for r in always() {
                    if let Some(v) = key(r)
                        && best.is_none_or(|(b, _)| v < b)
                    {
                        best = Some((v, r.cut));
                    }
                }
                best.map(|(_, cut)| cut)
            };
            let binding = match edge {
                CeilingEdge::ForceBudget
                | CeilingEdge::Deflection
                | CeilingEdge::VendorDepthTable => first_by(&|r| r.bound.ok().flatten()),
                CeilingEdge::Power => first_by(&|r| r.value.map(|v| -v)),
            };
            let kind = edge.kind();
            if jp.at == crate::limits::JudgedAt::Proposal {
                for l in &mut self.boundary {
                    if l.kind == kind
                        && matches!(l.edge, Edge::ApCurve { .. } | Edge::ApCeiling { .. })
                    {
                        l.binding_cut = binding;
                    }
                }
            }
            let basis = self.limit(kind).map(|l| l.basis.clone());
            out.extend(rows.rows.iter().map(|r| {
                r.check(
                    jp.at,
                    edge.limit(),
                    rows.unit,
                    BoundSide::Ceiling,
                    basis.clone(),
                )
            }));
        }
        if let Some(sp) = jp.speeds.as_ref() {
            for check in self.value_checks.clone() {
                let (side, kind, rows) = self.value_rows(check, jp.point, sp);
                let basis = kind.and_then(|k| self.limit(k)).map(|l| l.basis.clone());
                out.extend(
                    rows.iter()
                        .map(|r| r.check(jp.at, check.limit(), check.unit(), side, basis.clone())),
                );
            }
        }
        // P29-9 9c: a finish pass on the earlier op's leave with no leave
        // has no depth: every depth and power row of a cut on the body plane
        // gives `NoNumber(NoEarlierLeave)`.
        if let BodyDepth::NoEarlierLeave(_) = self.inputs.cuts.body_depth() {
            for row in &mut out {
                // A scope answer (no bound, not judged here) stays: the
                // limit draws no edge whatever the depth.
                if row.own_depth.is_none()
                    && !matches!(row.cut, CutKind::Entry(_))
                    && !matches!(
                        row.check.state,
                        crate::limits::CheckState::NoNumber(
                            crate::limits::NoNumberReason::NoBound
                                | crate::limits::NoNumberReason::NotJudgedHere
                        )
                    )
                    && matches!(
                        row.check.limit,
                        crate::limits::LimitId::ForceBudget
                            | crate::limits::LimitId::ToolDeflection
                            | crate::limits::LimitId::VendorDepthTable
                            | crate::limits::LimitId::SpindlePower
                    )
                {
                    row.check.value = None;
                    row.check.bound = None;
                    row.check.margin = None;
                    row.check.state = crate::limits::CheckState::NoNumber(
                        crate::limits::NoNumberReason::NoEarlierLeave,
                    );
                }
            }
        }
        // P29-8 8d: an entry row names why its entry rule gives no feed.
        for row in &mut out {
            if let CutKind::Entry(kind) = row.cut {
                row.entry_no_number = self
                    .inputs
                    .cuts
                    .entry_no_number(kind)
                    .map(crate::feeds::entry_rule::EntryNoNumber::clause);
            }
        }
        out
    }

    /// P29-10: THE rows of the envelope, written once when the envelope is
    /// whole: the planned rows at the proposal, then the rows of the cut as
    /// the op stands. P29-10 10i (lead ruling L1): every declared cut at the
    /// stated point, by the one judge (`read_declared_cuts` at
    /// [`Self::at_stated`]); then the stated point against each limit that
    /// is not a limit of each cut (`of_envelope`; a limit with a planned
    /// role has its stated rows by cut, so its `of_envelope` row would be a
    /// second number of one limit).
    fn write_rows(&mut self) {
        let at_earlier_leave = matches!(self.inputs.cuts.body_depth(), BodyDepth::EarlierLeave(_));
        let modulation = self.inputs.modulation;
        // P29-10 10g: the planned rows read the proposal point, so the marks
        // there are the points of the rows; the marks at the stated point
        // are the cut that the op holds (one set when the two are one point).
        let proposal =
            crate::feeds::plane::PlaneMarks::at(&self.inputs.cuts, self.proposal_point, true);
        self.marks_at_stated = self.stated.map(|p| {
            let same = (p.ap_mm - self.proposal_point.ap_mm).abs() < 1e-6
                && (p.ae_mm - self.proposal_point.ae_mm).abs() < 1e-6;
            crate::feeds::plane::PlaneMarks::at(&self.inputs.cuts, p, same)
        });
        let mut rows: Vec<crate::limits::CheckRow> = std::mem::take(&mut self.planned)
            .into_iter()
            .map(|r| {
                let at = proposal.of_cut(r.cut).map(|m| m.at);
                r.into_row(at_earlier_leave, at, modulation)
            })
            .collect();
        self.marks_at_proposal = Some(proposal);
        if let Some(p) = self.stated {
            let stated_point = self.at_stated(p);
            let mut stated = self.read_declared_cuts(&stated_point);
            self.mark_outside_row(&mut stated);
            let marks = self.marks_at_stated.as_ref();
            rows.extend(stated.into_iter().map(|r| {
                let at = marks.and_then(|m| m.of_cut(r.cut)).map(|m| m.at);
                r.into_row(at_earlier_leave, at, modulation)
            }));
            // `of_envelope` gives one check per boundary limit, in order,
            // then the exact move budget (a count of moves).
            let checks = crate::limits::of_envelope(self, p);
            rows.extend(checks.into_iter().enumerate().filter_map(|(i, check)| {
                match planned_limit_of(check.limit) {
                    PlaneRole::NotACutLimit(_) => {}
                    PlaneRole::DepthCeiling(_) | PlaneRole::ValueCheck(_) => return None,
                }
                let compared = self
                    .boundary
                    .get(i)
                    .map_or(crate::limits::Compared::Load, |l| {
                        crate::limits::Compared::of_edge(&l.edge)
                    });
                Some(crate::limits::CheckRow::stated(check, compared))
            }));
        }
        self.rows = rows;
    }

    /// P29-10 10i (lead ruling 4): one number on screen. When the
    /// placement puts the stated point outside a limit with a planned role,
    /// the stated cut row of that limit carries `Check::binding`: the row of
    /// the cut that sets the ceiling there ([`Self::outside_cut`]), else the
    /// row of that limit with the lowest margin. Every text about the
    /// stated point reads that row; the placement keeps its pick chip for
    /// the decision only and no surface prints its margin.
    fn mark_outside_row(&self, rows: &mut [PlannedRow]) {
        let Some(Placement::Outside { binding, .. }) = self.operator else {
            return;
        };
        let limit = crate::limits::LimitId::of_limit_kind(binding);
        match planned_limit_of(limit) {
            PlaneRole::DepthCeiling(_) | PlaneRole::ValueCheck(_) => {}
            PlaneRole::NotACutLimit(_) => return,
        }
        let of_cut = self.outside_cut().and_then(|cut| {
            rows.iter()
                .position(|r| r.check.limit == limit && r.cut == cut)
        });
        let lowest = || {
            rows.iter()
                .enumerate()
                .filter(|(_, r)| r.check.limit == limit)
                .min_by(|(_, a), (_, b)| {
                    let m = |r: &PlannedRow| r.check.margin.unwrap_or(f64::INFINITY);
                    m(a).total_cmp(&m(b))
                })
                .map(|(i, _)| i)
        };
        if let Some(row) = of_cut.or_else(lowest).and_then(|i| rows.get_mut(i)) {
            row.check.binding = true;
        }
    }

    /// P29-10: THE rows of the envelope: the planned rows at the proposal,
    /// then the stated rows ([`Self::write_rows`]).
    #[must_use]
    pub fn rows(&self) -> &[crate::limits::CheckRow] {
        &self.rows
    }

    /// P29-10 10g: the declared cuts on the plane at the proposal point,
    /// the point of the planned rows (each mark is tied to the rows of its
    /// cut, `CheckRow::cut`).
    #[must_use]
    pub const fn marks_at_proposal(&self) -> Option<&crate::feeds::plane::PlaneMarks> {
        self.marks_at_proposal.as_ref()
    }

    /// P29-10 10g: the declared cuts on the plane at the stated point (the
    /// cut that the op holds); `None` with no stated point.
    #[must_use]
    pub const fn marks_at_stated(&self) -> Option<&crate::feeds::plane::PlaneMarks> {
        self.marks_at_stated.as_ref()
    }

    /// P29-10 10g (lead ruling R6): the declared cut that sets the ceiling
    /// that places the stated point outside, when the limit that places it
    /// outside is solved per cut (the force budget). The placement judges
    /// the stated width at the lower of the curve chip and the pick chip
    /// (`judged_ceiling`); this names the cut of that lower ceiling. `None`
    /// when the point sits inside, or the limit names no cut.
    #[must_use]
    pub fn outside_cut(&self) -> Option<CutKind> {
        let Some(Placement::Outside { binding, .. }) = self.operator else {
            return None;
        };
        let p = self.stated?;
        let at = |role: ExactRole| {
            self.exact
                .iter()
                .find(|e| e.kind == binding && e.role == role && (e.ae_mm - p.ae_mm).abs() < 1e-12)
        };
        match (at(ExactRole::CurveChip), at(ExactRole::Pick)) {
            (curve, Some(pick)) if curve.is_none_or(|c| pick.ap_mm <= c.ap_mm) => pick.cut,
            (Some(curve), _) => curve.cut,
            (None, _) => None,
        }
    }

    /// P29-10 10g (lead ruling R2): the force-budget edge column by column
    /// (the solved columns of the drawn `Rigidity` edge): the width, the
    /// ceiling, what sets it and the declared cut that gives it. Empty with
    /// no force-budget edge.
    #[must_use]
    pub fn force_budget_columns(&self) -> Vec<BudgetColumn> {
        let Some(model) = self.budget.as_ref() else {
            return Vec::new();
        };
        model
            .solved
            .0
            .iter()
            .map(|(ae_mm, (depth, cut))| BudgetColumn {
                ae_mm: *ae_mm,
                ap_mm: depth.ceiling_mm(),
                sets: match depth {
                    BudgetDepth::Solved(_) => ColumnCeiling::ForceBudget,
                    BudgetDepth::BelowFloor => ColumnCeiling::DepthFloor,
                    BudgetDepth::NoRootBelowFlute => ColumnCeiling::FluteTop,
                },
                cut: *cut,
            })
            .collect()
    }

    /// P29-9 9c: why a depth ceiling draws no edge on this envelope, when
    /// the reason is a scope fact of today; `None` for every other reason
    /// (the depth ceiling then gives no row, as before). One match, no
    /// wildcard.
    ///
    /// - The force budget of a finish pass (ruling R2, the depth limit
    ///   `NoLimit(FinishPass)`): `NoNumber(NoBound)`.
    /// - P29-10 10h: the deflection on a fixed depth axis has no scope
    ///   reason now: `ceiling_rows` judges each cut at the fixed depth.
    fn no_edge_reason(&self, edge: CeilingEdge) -> Option<crate::limits::NoNumberReason> {
        use crate::limits::{DepthLimit, NoDepthLimit, NoNumberReason};
        match edge {
            // EXEMPT(S-NoBound)
            CeilingEdge::ForceBudget => (self.inputs.cuts.list().body_facts().depth_rule
                == DepthLimit::NoLimit(NoDepthLimit::FinishPass))
            .then_some(NoNumberReason::NoBound),
            CeilingEdge::Deflection | CeilingEdge::VendorDepthTable | CeilingEdge::Power => None,
        }
    }

    /// The proposal as a judged point: the proposal depth and width, the
    /// speeds that the proposal ships (one calculator call); no speeds with
    /// no commanded chip.
    fn at_proposal(&self) -> JudgedPoint {
        let p = self.proposal_point;
        let r = self.inputs.calc_at(p);
        let fz = chip_of(&r, self.inputs.tool.flute_count_planning());
        let speeds = (fz > 0.0).then_some(PointSpeeds {
            rpm_feed_kw: Some((r.rpm, r.feed_rate_mm_min, r.available_power_kw)),
            fz: Some(fz),
            stored: None,
            r,
        });
        JudgedPoint {
            at: crate::limits::JudgedAt::Proposal,
            point: p,
            plane_ap: self.proposal.get().ap_mm(),
            ceiling_chip: CeilingChip::CurveAndPick,
            speeds,
        }
    }

    /// P29-10 10i: the stated cut as a judged point: the stated depth and
    /// width, the stored op feed over the stated speed (one write,
    /// `StoredFeedChip`), the calculator at the point for the chip band;
    /// with no stated feed or speed the depth ceilings read the chip of
    /// the drawn edge (ruling 5) and the chip rows have no number.
    fn at_stated(&self, p: CutPoint) -> JudgedPoint {
        let z = self.inputs.tool.flute_count_planning();
        let operator = operator_chip(&self.inputs.operation, &self.inputs.tool);
        let ceiling_chip = match operator {
            Some(c) => CeilingChip::Stated(c.fz_mm),
            None => CeilingChip::Suggested(self.curve_fz(p.ae_mm)),
        };
        // The rule of the proposal: the point has a depth of the body plane
        // when the envelope proposes a depth (a surface offset has none
        // before the simulation, P16).
        let plane_ap = self.proposal.get().ap_mm().map(|_| p.ap_mm);
        JudgedPoint {
            at: crate::limits::JudgedAt::Stated,
            point: p,
            plane_ap,
            ceiling_chip,
            speeds: Some(PointSpeeds {
                r: self.inputs.calc_at(p),
                rpm_feed_kw: operator.map(|c| {
                    (
                        c.rpm,
                        c.feed_mm_min,
                        self.inputs.machine.power_at_rpm(c.rpm),
                    )
                }),
                fz: operator.map(|c| c.fz_mm),
                stored: operator.map(|c| StoredFeedChip::of(c.feed_mm_min, c.rpm, z)),
            }),
        }
    }

    /// The rows of one depth ceiling at the judged point `jp` (see
    /// [`Self::read_declared_cuts`]), or `None` when the op gives the limit
    /// no number there.
    fn ceiling_rows(
        &self,
        edge: CeilingEdge,
        tool: Option<&ToolDefinition>,
        jp: &JudgedPoint,
    ) -> Option<CutRows> {
        use crate::feeds::planned::{PlannedCell, PowerEnv, SpindlePowerPlanned};
        let p = jp.point;
        let cuts = &self.inputs.cuts;
        // P29-10 10i: the chip of the depth ceilings at the point: the
        // curve chip (and the pick chip of the force budget) at the
        // proposal; the stated chip at the stated point, or the suggested
        // chip of the drawn edge with no stated feed (ruling 5).
        let (curve_chip, pick_chip, suggested) = match jp.ceiling_chip {
            CeilingChip::CurveAndPick => {
                (self.curve_fz(p.ae_mm), Some(self.pick_fz(p.ae_mm)), false)
            }
            CeilingChip::Stated(fz) => (fz, None, false),
            CeilingChip::Suggested(fz) => (fz, None, true),
        };
        let suggested_on = |depth: PlaneDepth| suggested && depth == PlaneDepth::Body;
        // P29-9 9b: a body that states a lower bound of its depth is judged
        // at the bound (a Project Curve has no proposal depth); every other
        // body at the proposal depth.
        // P29-9 9c: a finish pass on the earlier op's leave is judged at
        // the leave; with no leave its body rows have no number (set after
        // the rows, `read_declared_cuts`).
        // P29-10 10h: the depth may be absent (no proposal depth); the
        // deflection on a fixed depth axis then says so on its rows, every
        // other limit gives no rows, as before.
        let (body_ap, lower_bound) = match cuts.body_depth() {
            BodyDepth::AtLeast(b) => (Some(b), true),
            BodyDepth::EarlierLeave(leave) => (Some(leave), false),
            BodyDepth::Plane(_) | BodyDepth::NoEarlierLeave(_) | BodyDepth::None(_) => {
                (jp.plane_ap, false)
            }
        };
        let bound_of = |depth: PlaneDepth| match depth {
            PlaneDepth::Body => lower_bound,
            PlaneDepth::Own(_) | PlaneDepth::Axial => false,
        };
        let column = |fz: f64| BodyColumn {
            ae_mm: p.ae_mm,
            chip_mm: Some(fz),
            stored: None,
        };
        let not_judged = Err(NoNumberReason::NotJudgedHere);
        // P29-8 8d: a cut on the body plane is judged at the proposal
        // depth; a cut at its own depth (an entry) at that depth, a value
        // check against its root; an axial cut (a plunge) has no depth on
        // the plane.
        let value_of = |depth: PlaneDepth| match depth {
            PlaneDepth::Body => body_ap,
            PlaneDepth::Own(own) => Some(own.mm),
            PlaneDepth::Axial => None,
        };
        let rows: Vec<CutRow> = match edge {
            CeilingEdge::ForceBudget => {
                body_ap?;
                let model = self.budget.as_ref()?;
                self.limit(LimitKind::Rigidity)?;
                let env = ForceEnv {
                    tool: tool?,
                    material: &self.inputs.material,
                    bound: model.budget_n,
                };
                let curve =
                    judge_declared_cuts::<ForceBudgetPlanned>(cuts, column(curve_chip), &env);
                let pick = pick_chip
                    .map(|fz| judge_declared_cuts::<ForceBudgetPlanned>(cuts, column(fz), &env));
                curve
                    .iter()
                    .enumerate()
                    .map(|(i, a)| {
                        let b = pick
                            .as_ref()
                            .and_then(|pick| pick.iter().nth(i))
                            .map_or(a.value, |e| e.value);
                        let bound = match (a.value, b) {
                            (PlannedCell::Judged(Some(x)), PlannedCell::Judged(Some(y))) => {
                                Ok(match (x.ceiling_mm(), y.ceiling_mm()) {
                                    (Some(u), Some(v)) => Some(u.min(v)),
                                    (Some(u), None) | (None, Some(u)) => Some(u),
                                    (None, None) => None,
                                })
                            }
                            (PlannedCell::Judged(_), PlannedCell::Judged(_)) => {
                                Err(NoNumberReason::NotModelled)
                            }
                            (
                                PlannedCell::Judged(_)
                                | PlannedCell::NotJudged(_)
                                | PlannedCell::NotACutLimit(_),
                                _,
                            ) => not_judged,
                        };
                        CutRow::of_cell(
                            a.kind,
                            a.presence,
                            a.depth,
                            value_of(a.depth),
                            &a.value,
                            bound,
                        )
                        .at_lower_bound(bound_of(a.depth))
                        .at_suggested_chip(suggested_on(a.depth))
                    })
                    .collect()
            }
            CeilingEdge::Deflection => {
                // P29-10 10h (lead ruling L3): on a fixed depth axis the
                // envelope draws no deflection edge, and each declared cut is
                // a value check at the fixed depth (the earlier op's leave,
                // the lower bound of a Project Curve, an entry's own depth),
                // with the basis of the drawn edge. On a free axis the rows
                // read the drawn edge, as before. The boundary does not
                // change, so no placement and no proposal moves.
                let fixed = !self.axes.ap_free();
                let drawn = self
                    .limit(LimitKind::Deflection)
                    .is_some_and(|l| matches!(l.edge, Edge::ApCurve { .. }));
                if !fixed && !drawn {
                    return None;
                }
                if !fixed {
                    body_ap?;
                }
                let own_basis = fixed.then(deflection_edge_basis);
                // The beam abstains (T-4): every cut says why, no row is
                // silent.
                let stickout = self.inputs.tool.stickout_planning_mm();
                let abstains = match tool {
                    None => Some(NoNumberReason::NoToolBody),
                    Some(_) if self.inputs.material.force_line().is_err() => {
                        Some(NoNumberReason::NoForceLine)
                    }
                    Some(_) if !(stickout.is_finite() && stickout > 0.0) => {
                        Some(NoNumberReason::NotModelled)
                    }
                    Some(_) => None,
                };
                match (abstains, tool) {
                    (Some(reason), _) if fixed => cuts
                        .list()
                        .iter()
                        .map(|c| {
                            let depth = PlaneDepth::of_declared(c.depth);
                            CutRow::of(c.kind, c.presence, depth, value_of(depth), Err(reason))
                                .with_basis(own_basis.clone())
                                .at_suggested_chip(suggested_on(depth))
                        })
                        .collect(),
                    (Some(_) | None, None) => return None,
                    (Some(_) | None, Some(tool)) => {
                        let env = ForceEnv {
                            tool,
                            material: &self.inputs.material,
                            bound: inputs_deflection_limit_um(&self.inputs),
                        };
                        judge_declared_cuts::<DeflectionPlanned>(cuts, column(curve_chip), &env)
                            .iter()
                            .map(|e| {
                                let value = value_of(e.depth);
                                let bound = match (e.value, e.depth) {
                                    // P29-8 8d: a plunge has no side-cut model; no
                                    // root is no number here, not "inside".
                                    (PlannedCell::Judged(_), PlaneDepth::Axial) => {
                                        Err(NoNumberReason::NotModelled)
                                    }
                                    // 10h: a body with no depth before the
                                    // simulation has no value to judge. A ramp
                                    // or a helix with no own depth has no
                                    // engagement (P29-8 8d: an entry is a slot
                                    // at its own depth only).
                                    (PlannedCell::Judged(_), PlaneDepth::Body)
                                        if value.is_none()
                                            || matches!(
                                                e.kind,
                                                CutKind::Entry(
                                                    crate::limits::EntryKind::Ramp
                                                        | crate::limits::EntryKind::Helix,
                                                )
                                            ) =>
                                    {
                                        Err(NoNumberReason::NotModelled)
                                    }
                                    (
                                        PlannedCell::Judged(root),
                                        PlaneDepth::Body | PlaneDepth::Own(_),
                                    ) => Ok(root),
                                    (
                                        PlannedCell::NotJudged(_) | PlannedCell::NotACutLimit(_),
                                        _,
                                    ) => not_judged,
                                };
                                CutRow::of_cell(e.kind, e.presence, e.depth, value, &e.value, bound)
                                    .at_lower_bound(bound_of(e.depth))
                                    .with_basis(own_basis.clone())
                                    .at_suggested_chip(suggested_on(e.depth))
                            })
                            .collect()
                    }
                }
            }
            CeilingEdge::VendorDepthTable => {
                body_ap?;
                let band_max = self.table_band_max?;
                self.limit(LimitKind::VendorDepthTable)?;
                let env = TableEnv {
                    band_max_mm: band_max,
                    d_mm: self.inputs.tool.diameter,
                    top_mm: self.speeds.aps.last().copied().unwrap_or(MIN_AP_MM),
                };
                judge_declared_cuts::<VendorDepthTablePlanned>(cuts, column(curve_chip), &env)
                    .iter()
                    .map(|e| {
                        let bound = match e.value {
                            PlannedCell::Judged(Some((mm, true))) => Ok(Some(mm)),
                            PlannedCell::Judged(Some((_, false))) => Ok(None),
                            PlannedCell::Judged(None) => Err(NoNumberReason::NotModelled),
                            PlannedCell::NotJudged(_) | PlannedCell::NotACutLimit(_) => not_judged,
                        };
                        CutRow::of_cell(
                            e.kind,
                            e.presence,
                            e.depth,
                            value_of(e.depth),
                            &e.value,
                            bound,
                        )
                        .at_lower_bound(bound_of(e.depth))
                        .at_suggested_chip(suggested_on(e.depth))
                    })
                    .collect()
            }
            // The power of each declared cut at the speeds of the proposal,
            // through the calculator's power terms (Step 6 reads the same
            // terms, `planned::worst_power`), against the rated power.
            CeilingEdge::Power => {
                // P29-9 9b (P16, review D7): the power of a surface offset
                // is known after the simulation, so a body at a lower bound
                // gives no power row (the power figure stays
                // `DepthAfterSimulation`).
                if lower_bound {
                    return None;
                }
                body_ap?;
                let sp = jp.speeds.as_ref()?;
                // P29-10 10i (ruling 5): no stated feed or speed: no power.
                let (Some((rpm, feed, available_kw)), Some(fz)) = (sp.rpm_feed_kw, sp.fz) else {
                    let rows = cuts
                        .list()
                        .iter()
                        .map(|c| {
                            let depth = PlaneDepth::of_declared(c.depth);
                            CutRow::of(
                                c.kind,
                                c.presence,
                                depth,
                                None,
                                Err(NoNumberReason::NoStatedFeed),
                            )
                        })
                        .collect();
                    return Some(CutRows { unit: "kW", rows });
                };
                // P29-9 9c: the body power at the earlier op's leave, else at
                // the depth of the point.
                let power_ap = match cuts.body_depth() {
                    BodyDepth::EarlierLeave(leave) => leave,
                    BodyDepth::Plane(_)
                    | BodyDepth::AtLeast(_)
                    | BodyDepth::NoEarlierLeave(_)
                    | BodyDepth::None(_) => p.ap_mm,
                };
                let mut input = FeedsInput::of_operation(
                    &self.inputs.operation,
                    &self.inputs.tool,
                    &self.inputs.material,
                    &self.inputs.machine,
                    self.inputs.lut,
                    self.inputs.spindle_strategy,
                );
                input.axial_depth_mm = Some(power_ap);
                input.radial_width_mm = Some(p.ae_mm);
                let line = input.power_force_line()?;
                let effective_d = crate::feeds::engagement::EngagedDiameter::of_hint(
                    input.tool_geometry,
                    input.tool_diameter,
                    input.cut_body.max_cut_diameter_mm,
                    power_ap,
                );
                let power = |w: f64| {
                    super::power_model_terms(&input, line, power_ap, w, effective_d, rpm)
                        .kw_at_feed(feed)
                };
                // P29-8 8d: an entry (at its own depth, or a plunge) at its
                // own engagement (the declared cut, `DeclaredCut::at`) and its
                // own feed (the entry chip at the stated speed), through the
                // one forward read of a load (`CutLoad::of_cut`), against the
                // rated power at that speed.
                let entry_rpm = cuts.entry_rpm().ok();
                let z = self.inputs.tool.flute_count_planning().max(1);
                let cut_ctx = crate::compute::catalog::CutContext::of_tool(&self.inputs.tool);
                let entry_power = |kind: CutKind, fz: f64| {
                    let rpm = entry_rpm?;
                    let declared = cuts.list().of_kind(kind)?;
                    let at = crate::feeds::cut_load::OperatingPoint::planned(
                        fz * rpm * f64::from(z),
                        rpm,
                        z,
                    );
                    let cut = declared.at(&cut_ctx, Some(at));
                    crate::feeds::cut_load::CutLoad::of_cut(&cut, tool, Some(line))
                        .and_then(|load| load.power_kw())
                        .ok()
                };
                let env = PowerEnv {
                    column_ae_mm: p.ae_mm,
                    power_kw: &power,
                    entry_power_kw: &entry_power,
                };
                let available = |depth: PlaneDepth| match depth {
                    PlaneDepth::Body => Ok(Some(available_kw)),
                    PlaneDepth::Own(_) | PlaneDepth::Axial => match entry_rpm {
                        Some(rpm) => Ok(Some(self.inputs.machine.power_at_rpm(rpm))),
                        None => Err(NoNumberReason::NotModelled),
                    },
                };
                judge_declared_cuts::<SpindlePowerPlanned>(cuts, column(fz), &env)
                    .iter()
                    .map(|e| match e.value {
                        PlannedCell::Judged(Some((_, kw, _))) => {
                            CutRow::of(e.kind, e.presence, e.depth, Some(kw), available(e.depth))
                        }
                        PlannedCell::Judged(None) => CutRow::of(
                            e.kind,
                            e.presence,
                            e.depth,
                            None,
                            Err(NoNumberReason::NotModelled),
                        ),
                        PlannedCell::NotJudged(_) | PlannedCell::NotACutLimit(_) => {
                            CutRow::of_cell(e.kind, e.presence, e.depth, None, &e.value, not_judged)
                        }
                    })
                    .collect()
            }
        };
        let unit = match edge {
            CeilingEdge::ForceBudget | CeilingEdge::Deflection | CeilingEdge::VendorDepthTable => {
                "mm"
            }
            CeilingEdge::Power => "kW",
        };
        Some(CutRows { unit, rows })
    }

    /// The rows of one value check at the point `p` with the speeds `sp`:
    /// the planned peak chip of each declared cut against the bound, its
    /// side and the envelope kind of its basis. No rows with no matched row
    /// or no band. P29-10 10i (ruling 5): a cut that reads the body chip
    /// when the op states no feed or speed is `NoNumber(NoStatedFeed)`.
    fn value_rows(
        &self,
        check: ValueCheck,
        p: CutPoint,
        sp: &PointSpeeds,
    ) -> (BoundSide, Option<LimitKind>, Vec<CutRow>) {
        use crate::feeds::planned::{
            ChipEnv, ChipMaxPlanned, ChipMinPlanned, ChipReading, PlannedCell, RubbingFloorPlanned,
        };
        let (side, kind) = match check {
            ValueCheck::ChipMax => (BoundSide::Ceiling, LimitKind::ChipBand),
            ValueCheck::ChipMin => (BoundSide::Floor, LimitKind::ChipBand),
            ValueCheck::RubbingFloor => (BoundSide::Floor, LimitKind::RubbingFloor),
            ValueCheck::VerticalRate => {
                return (BoundSide::Ceiling, None, self.vertical_rows(p, sp));
            }
        };
        let kind = Some(kind);
        let r = &sp.r;
        // The upper edge: the band maximum, or the printed point (a point is
        // a hard ceiling, A2). The lower edge is the typed floor
        // (`planned::ChipFloor`, P29-10 PB): a band minimum or a reference
        // from a point, never a band made from one value.
        let Some(band_max) = r
            .chipload_bounds
            .map(|b| b.max_mm_per_tooth)
            .or(r.chipload_point_mm)
            .filter(|_| r.matched_lut_row.is_some())
        else {
            return (side, kind, Vec::new());
        };
        let floor = crate::feeds::planned::ChipFloor::of_result(r);
        let cuts = &self.inputs.cuts;
        let column = BodyColumn {
            ae_mm: p.ae_mm,
            chip_mm: sp.fz,
            stored: sp.stored,
        };
        // Ruling 5: the reason of a body-chip cut with no stated chip.
        let no_chip = |depth: PlaneDepth| match (sp.fz, depth) {
            (None, PlaneDepth::Body) => NoNumberReason::NoStatedFeed,
            (Some(_), PlaneDepth::Body | PlaneDepth::Own(_) | PlaneDepth::Axial)
            | (None, PlaneDepth::Own(_) | PlaneDepth::Axial) => NoNumberReason::NotModelled,
        };
        let d_eng = crate::feeds::engagement::EngagedDiameter::of_hint(
            self.inputs.tool.geometry_hint(),
            self.inputs.tool.diameter,
            crate::feeds::CutBody::of_tool(&self.inputs.tool).max_cut_diameter_mm,
            p.ap_mm,
        )
        .mm();
        let env = |bound_mm: f64| ChipEnv {
            thinning: cuts.thinning(),
            d_eng_mm: d_eng,
            bound_mm,
        };
        let cells: ByCutChip = match check {
            ValueCheck::ChipMax => {
                judge_declared_cuts::<ChipMaxPlanned>(cuts, column, &env(band_max))
            }
            ValueCheck::ChipMin => {
                let Some(floor) = floor else {
                    return (side, kind, Vec::new());
                };
                let env = crate::feeds::planned::ChipFloorEnv {
                    thinning: cuts.thinning(),
                    d_eng_mm: d_eng,
                    floor,
                };
                judge_declared_cuts::<ChipMinPlanned>(cuts, column, &env)
            }
            ValueCheck::RubbingFloor => {
                let (floor, _) =
                    crate::feeds::rubbing_floor_at(r.chipload_bounds, r.chipload_point_mm);
                judge_declared_cuts::<RubbingFloorPlanned>(cuts, column, &env(floor))
            }
            // Answered before the chip band (no band needed).
            ValueCheck::VerticalRate => return (side, None, Vec::new()),
        };
        let rows = cells
            .iter()
            .map(|e| {
                let (value, judged, bound, stored_feed, advisory_floor) = match e.value {
                    PlannedCell::<Option<ChipReading>>::Judged(Some(c)) => (
                        Some(c.peak_mm),
                        Some(c.judged_mm),
                        Ok(Some(c.bound_mm)),
                        c.stored_feed,
                        c.advisory_floor,
                    ),
                    PlannedCell::Judged(None) => (None, None, Err(no_chip(e.depth)), false, None),
                    PlannedCell::NotJudged(_) | PlannedCell::NotACutLimit(_) => {
                        (None, None, Err(NoNumberReason::NotJudgedHere), false, None)
                    }
                };
                CutRow::of_cell(e.kind, e.presence, e.depth, value, &e.value, bound)
                    .judged_at(judged)
                    .with_stored_feed(stored_feed)
                    .with_advisory_floor(advisory_floor)
            })
            .collect();
        (side, kind, rows)
    }

    /// P29-8e (c): one row per declared cut: its vertical feed against
    /// `VerticalRateLimit::bound` at the stated speed (a value check, no
    /// ceiling, no feed moves); a cut with no vertical feed is
    /// `NotApplicable(NoVerticalFeed)`.
    fn vertical_rows(&self, p: CutPoint, sp: &PointSpeeds) -> Vec<CutRow> {
        use crate::feeds::planned::{
            PlannedCell, VerticalEnv, VerticalRatePlanned, VerticalReading,
        };
        let cuts = &self.inputs.cuts;
        let column = BodyColumn {
            ae_mm: p.ae_mm,
            chip_mm: sp.fz,
            stored: sp.stored,
        };
        let env = VerticalEnv {
            limit: sp.r.plunge.vertical_limit(),
            rpm: cuts.entry_rpm(),
            feeds: self.inputs.vertical,
        };
        judge_declared_cuts::<VerticalRatePlanned>(cuts, column, &env)
            .iter()
            .map(|e| {
                let row = |value, bound| {
                    CutRow::of_cell(e.kind, e.presence, e.depth, value, &e.value, bound)
                };
                match e.value {
                    PlannedCell::Judged(VerticalReading::Value {
                        value_mm_min,
                        limit: Some(limit),
                    }) => row(Some(value_mm_min), Ok(Some(limit.mm_min())))
                        .with_basis(env.limit.basis_of(limit.binding())),
                    // No arm holds a rate: nothing bounds the vertical feed.
                    PlannedCell::Judged(VerticalReading::Value {
                        value_mm_min,
                        limit: None,
                    }) => row(Some(value_mm_min), Ok(None)),
                    PlannedCell::Judged(VerticalReading::NoVerticalFeed) => {
                        row(None, Ok(None)).not_applicable(crate::limits::Geometry::NoVerticalFeed)
                    }
                    PlannedCell::Judged(VerticalReading::NoNumber) => {
                        row(None, Err(NoNumberReason::NotModelled))
                    }
                    PlannedCell::NotJudged(_) | PlannedCell::NotACutLimit(_) => {
                        row(None, Err(NoNumberReason::NotJudgedHere))
                    }
                }
            })
            .collect()
    }

    /// P29-7 7c (trap 2): the note when the force budget sets the proposal
    /// depth and the move budget is not met at that depth. No number moves:
    /// `propose` cannot raise a capped depth (K2) and keeps a stated width
    /// (R2), so the card and the record state the conflict.
    fn move_budget_conflict_note(&self) -> Option<EnvelopeNote> {
        let p = self.proposal.get().point()?;
        if !self.proposal_limits.contains(&LimitKind::Rigidity) {
            return None;
        }
        let unmet = self
            .placing_margins(p)
            .iter()
            .any(|(kind, margin, _)| *kind == LimitKind::Runtime && *margin < -1e-9);
        unmet.then_some(EnvelopeNote::MoveBudgetConflict)
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
        // P19b: a width bound that moves with the depth is read at the
        // deepest depth the search may pick (the stated depth, else the
        // span top). A floor that no width meets there bounds nothing.
        let ap_for_width = plan.stated_ap.unwrap_or(match self.axes {
            EnvelopeAxes::Plane { ap, .. } | EnvelopeAxes::AxialBar { ap, .. } => ap.max_mm,
            EnvelopeAxes::RadialBar { ap_mm, .. } | EnvelopeAxes::Fixed { ap_mm, .. } => ap_mm,
        });
        let bound_at_depth =
            |l: &Limit| l.ae_bound_at(ap_for_width).filter(|(mm, _)| mm.is_finite());
        let ae_ceiling = self
            .boundary
            .iter()
            .filter_map(|l| match bound_at_depth(l) {
                Some((mm, CurveBound::Ceiling)) => Some((mm, l.kind)),
                Some((_, CurveBound::Floor)) | None => None,
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let ae_floor = self
            .boundary
            .iter()
            .filter_map(|l| match bound_at_depth(l) {
                Some((mm, CurveBound::Floor)) => Some(mm),
                Some((_, CurveBound::Ceiling)) | None => None,
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
        // The breakpoints of the limits: the grid columns of the curves.
        if hi > lo {
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
        if fixed_ae.is_none()
            && let Some(kind) = width_bound
            && (best_ae - hi).abs() < 1e-9
        {
            limits.push(kind);
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
                .stock()
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
fn axes_plan(
    operation: &OperationConfig,
    tool: &ToolConfig,
    base: &FeedsResult,
    cuts: &DeclaredCuts,
) -> AxesPlan {
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
        Axial::HelixPitch(s) => (true, Some(s.mm()), s.mm()),
        // P29-9 9a: a chamfer states the depth of the tool that cuts its
        // width (the one depth door), not the width as a depth. A tool that
        // gives no depth leaves the axis free and not stated.
        Axial::ChamferWidth(_) => match cuts.body_depth() {
            BodyDepth::Plane(ap) => (true, Some(ap), ap),
            // Not reached for a chamfer: a lower bound is a surface offset,
            // an earlier leave a finish pass.
            BodyDepth::AtLeast(_)
            | BodyDepth::EarlierLeave(_)
            | BodyDepth::NoEarlierLeave(_)
            | BodyDepth::None(_) => (true, None, base.axial_depth_mm),
        },
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
            (
                Axial::Surface { .. }
                | Axial::HelixPitch(_)
                | Axial::ChamferWidth(_)
                | Axial::SurfaceOffset(_),
                None,
            ) => (false, None, base.axial_depth_mm),
        },
        // P16 (review D7): the offset is not a depth. The depth is fixed by
        // the surface and known after the simulation; the axis holds the
        // calculator depth only for the speeds of the grid point.
        Axial::SurfaceOffset(_) => (false, None, base.axial_depth_mm),
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
    AxesPlan {
        axes,
        stated_ap,
        stated_ae,
        peck: matches!(spec.axial, Axial::Peck(_)),
        // P29-3: the offset-ring fact of the op's body cut.
        // EXEMPT(P28-05, P28-07)
        computed_width: radial_free
            && operation
                .cuts(&crate::feeds::entry_rule::ConfiguredEntry::registry_default(operation))
                .body_facts()
                .offset_ring,
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
        binding_cut: None,
        law: "0.8 x flute",
        basis: guard_basis,
        edge: Edge::ApCeiling { mm: guard },
    });
    if input.tool.cutting_length_planning_mm() > 0.0 {
        out.push(Limit {
            kind: LimitKind::CuttingLength,
            binding_cut: None,
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
    // EXEMPT(P28-25)
    if let Some(env) = crate::feeds::suggest::axial_envelope_for_operation(
        input.operation,
        input.tool,
        input.material,
        base.matched_lut_row.as_ref(),
    ) && let (Some(mm), Some(row)) = (env.max_doc_vendor_mm, base.matched_lut_row.as_ref())
    {
        out.push(Limit {
            kind: LimitKind::VendorAxial,
            binding_cut: None,
            law: "vendor ap bound",
            basis: Basis::Printed(crate::feeds::sourced::RowRef(row.observation_id.clone())),
            edge: Edge::ApCeiling { mm },
        });
    }
    // P13c (O3): the corner-coverage rule bounds a free pocket width.
    // EXEMPT(P28-07)
    if plan.computed_width {
        out.push(Limit {
            kind: LimitKind::CornerCoverage,
            binding_cut: None,
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
    out.push(Limit {
        kind: LimitKind::Diameter,
        binding_cut: None,
        law: "ae <= D",
        basis: Basis::tool_diameter(),
        edge: diameter_edge(input, plan, d),
    });
    out.push(Limit {
        kind: LimitKind::EngagementFloor,
        binding_cut: None,
        law: "smallest depth the calculator writes",
        basis: Basis::Rule(crate::feeds::MIN_AP_MM_ROW),
        edge: Edge::ApFloor { mm: MIN_AP_MM },
    });
    out.push(Limit {
        kind: LimitKind::EngagementFloor,
        binding_cut: None,
        law: "smallest width the calculator writes",
        basis: Basis::Rule(crate::feeds::MIN_AE_MM_ROW),
        edge: Edge::AeFloor { mm: MIN_AE_MM },
    });
}

/// P19b: the width ceiling of the tool. A flat end mill engages its
/// nominal diameter at every depth: `AeCeiling { D }`. A ball, a bull nose,
/// a V-bit or a taper engages `d_eng(ap) = 2 x engagement_radius(ap)`
/// (`EngagedDiameter::of_tool`, the one P15 door that a measured point also
/// reads), so on a free depth the ceiling is an `AeCurve` over the depth
/// columns, refined like the move-budget floor. A fixed depth, or a tool
/// that builds no cutter, keeps `AeCeiling { D }`.
fn diameter_edge(input: &EnvelopeInput<'_>, plan: &AxesPlan, d: f64) -> Edge {
    let flat = Edge::AeCeiling { mm: d };
    let span = match plan.axes {
        EnvelopeAxes::Plane { ap, .. } | EnvelopeAxes::AxialBar { ap, .. } => ap,
        EnvelopeAxes::RadialBar { .. } | EnvelopeAxes::Fixed { .. } => return flat,
    };
    match input.tool.geometry_hint() {
        ToolGeometryHint::Flat => return flat,
        ToolGeometryHint::Ball
        | ToolGeometryHint::TaperedBall { .. }
        | ToolGeometryHint::Bull { .. }
        | ToolGeometryHint::VBit { .. } => {}
    }
    let Ok(cutter) = crate::compute::cutter::build_cutter(input.tool) else {
        return flat;
    };
    let d_eng = |ap: f64| {
        Some(crate::feeds::engagement::EngagedDiameter::of_tool(&cutter, ap).mm())
            .filter(|v| v.is_finite())
    };
    let mut columns: Vec<(f64, Option<f64>)> = samples(span)
        .into_iter()
        .map(|ap| (ap, d_eng(ap)))
        .collect();
    refine_columns(
        &mut columns,
        |c: Option<f64>| c.unwrap_or(d),
        d_eng,
        SplitRule {
            levels: TABLE_EDGE_SPLIT_LEVELS,
            two_sided: true,
        },
    );
    let points: Vec<[f64; 2]> = columns
        .iter()
        .filter_map(|(ap, c)| c.map(|ae| [*ap, ae]))
        .collect();
    match points.as_slice() {
        [] => flat,
        [first, ..] if points.iter().all(|p| (p[1] - first[1]).abs() < 1e-9) => {
            Edge::AeCeiling { mm: first[1] }
        }
        _ => Edge::AeCurve {
            bound: CurveBound::Ceiling,
            points,
        },
    }
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
        binding_cut: None,
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
            // P20d: the envelope is a side-cut plane; a drill cycle draws
            // no envelope, and its axial power is advisory.
            (false | true, PowerCase::AxialPlunge) => {
                "axial plunge power is advisory (no envelope limit)"
            }
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
/// (`geometry::vendor_depth_table_ceiling_ratio`). P29-7 7c (ruling Q8):
/// each column is the lowest ceiling over the declared cuts that the plan
/// always cuts (a slot has the largest chip share). The chip is the
/// operator's (L7), else the suggested chip of the grid. The table is
/// printed as a chip rule at 1, 2 and 3 x D with no width (step 0 found that
/// it holds neither the force nor the chip), so it is drawn, not derived
/// from the force model. A peak chip at or below half the band maximum
/// meets the table at every printed depth: that column has no edge. No
/// band or printed point, a drill peck, a fixed depth, or no binding
/// column: no limit.
fn vendor_depth_limit(
    out: &mut Vec<Limit>,
    (input, inputs): (&EnvelopeInput<'_>, &OwnedInputs),
    base: &FeedsResult,
    speeds: &SpeedsMap,
    plan: &AxesPlan,
    (operator, drawn): (Option<OperatorChip>, Drawn),
) {
    // EXEMPT(P28-24)
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
    let flutes = input.tool.flute_count_planning();
    // The ceiling at the body width `ae` and the body chip `fz`, and whether
    // the table binds there: the lowest over the declared cuts (P29-7 7c).
    let env = TableEnv {
        band_max_mm: band_max,
        d_mm: d,
        top_mm: top,
    };
    let ceiling = |ae: f64, fz: f64| table_solve(&inputs.cuts, ae, fz, &env).0;
    // P19: the columns are the grid widths only (no sample at the stated
    // width: it moved the drawn edge when the stated width moved;
    // `add_exact_ceilings` solves the stated point). Between the columns the
    // refinement of the force-budget edge adds samples, so the drawn edge
    // stands within the solver resolution of the closed form everywhere.
    let mut columns: Vec<(f64, Option<(f64, bool)>)> = speeds
        .aes
        .iter()
        .enumerate()
        .map(|(j, &ae)| {
            let fz = match operator {
                Some(chip) => Some(chip.fz_mm),
                None => speeds.at(0, j).map(|p| p.chip.get().0),
            };
            (ae, fz.map(|fz| ceiling(ae, fz)))
        })
        .collect();
    // Not drawn (`pick_limits`): the coarse rank of the removal-rate search
    // reads it between the grid columns and the exact solve
    // (`table_ceiling_at`) decides, so it takes no refinement.
    if drawn == Drawn::Yes {
        refine_columns(
            &mut columns,
            |c: Option<(f64, bool)>| c.map_or(top, |c| c.0),
            |ae| {
                let fz = operator.map_or_else(
                    || {
                        chip_of(
                            &inputs.calc_at(CutPoint {
                                ap_mm: MIN_AP_MM,
                                ae_mm: ae,
                            }),
                            flutes,
                        )
                    },
                    |chip| chip.fz_mm,
                );
                Some(ceiling(ae, fz))
            },
            SplitRule {
                levels: TABLE_EDGE_SPLIT_LEVELS,
                two_sided: true,
            },
        );
    }
    let binds = columns.iter().any(|(_, c)| c.is_some_and(|c| c.1));
    let curve: Vec<[f64; 2]> = columns
        .iter()
        .filter_map(|(ae, c)| c.map(|c| [*ae, c.0]))
        .collect();
    if !binds {
        return;
    }
    let first = curve.first().map_or(0.0, |p| p[1]);
    let flat = curve.iter().all(|p| (p[1] - first).abs() < 1e-9);
    out.push(Limit {
        kind: LimitKind::VendorDepthTable,
        binding_cut: None,
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

/// The law of the `Deflection` limit (P29-7 7d, ruling R5).
const DEFLECTION_LAW: &str = "tip deflection <= min(radial stock to leave, 200 um)";

/// P29-7 7d (ruling R5): THE deflection limit (um) of the op's cuts:
/// `min(leave, 200 um)`, the value of the gate
/// (`tool_load::deflection::deflection_limit_um`).
fn inputs_deflection_limit_um(inputs: &OwnedInputs) -> f64 {
    crate::tool_load::deflection::deflection_limit_um(
        inputs.cuts.list().body_facts().leave_after_mm,
    )
}

/// The tip-deflection edge: per width sample, the shallowest `ap` where
/// the deflection meets the limit, by the one beam
/// (`cutter_axial_constraints`). A column with no root below the flute adds
/// no point (P11a, D-B). P29-7 7c: each column is the lowest root over the
/// declared cuts that the plan always cuts ([`deflection_solve`]). P29-7 7d
/// (ruling R5): the limit is `min(leave, 200 um)`
/// (`tool_load::deflection::deflection_limit_um`), the value of the gate,
/// and every cut with a free depth axis reads it, whatever its role.
fn deflection_limit(
    out: &mut Vec<Limit>,
    inputs: &OwnedInputs,
    speeds: &SpeedsMap,
    plan: &AxesPlan,
    operator: Option<OperatorChip>,
) {
    if !plan.axes.ap_free() {
        return;
    }
    // T-4: the beam cannot answer. State the abstention with its reason.
    // `cutter_axial_constraints` returns its upper search bracket when the
    // beam refuses, which would read as a deep limit (an absence rendered
    // as a reading), so the refusal is decided here first.
    let unmodelled = |reason: crate::feeds::predict::DeflectionUnmodeled| Limit {
        kind: LimitKind::Deflection,
        binding_cut: None,
        law: DEFLECTION_LAW,
        basis: Basis::Rule(crate::feeds::suggest::DEFLECTION_BACKOFF_TARGET_ROW),
        edge: Edge::Unmodelled {
            reason: reason.clause(),
        },
    };
    let Ok(tool_def) = crate::compute::cutter::build_cutter(&inputs.tool) else {
        out.push(unmodelled(
            crate::feeds::predict::DeflectionUnmodeled::ToolOutOfDomain,
        ));
        return;
    };
    if inputs.material.force_line().is_err() {
        out.push(unmodelled(
            crate::feeds::predict::DeflectionUnmodeled::MaterialUnvalidated,
        ));
        return;
    }
    if !(inputs.tool.stickout_planning_mm().is_finite() && inputs.tool.stickout_planning_mm() > 0.0)
    {
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
        // P11a (D-B): the solved depth, or `None` when no depth below the
        // flute reaches the target. The search bracket is never drawn.
        // P29-7 7c: the lowest over the declared cuts.
        columns.push((
            ae,
            deflection_solve(inputs, &tool_def, ae, fz).map(|(mm, _)| mm),
        ));
    }
    if columns.iter().all(|(_, mm)| mm.is_none()) {
        // No column reaches the target below the flute: the limit is
        // absent, and the envelope says so (an absence is stated, never
        // dropped and never drawn as an edge).
        out.push(Limit {
            kind: LimitKind::Deflection,
            binding_cut: None,
            law: DEFLECTION_LAW,
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
        binding_cut: None,
        law: DEFLECTION_LAW,
        basis: deflection_edge_basis(),
        edge: Edge::ApCurve { points: curve },
    });
}

/// The basis of the drawn deflection edge, and of the deflection rows on a
/// fixed depth axis (P29-10 10h): the one beam at the deflection target.
fn deflection_edge_basis() -> Basis {
    Basis::derived(
        "one beam (ToolDefinition::tip_deflection_mm)",
        vec![Basis::Rule(
            crate::feeds::suggest::DEFLECTION_BACKOFF_TARGET_ROW,
        )],
    )
}

/// The bisection steps that place a force-budget depth between two solved
/// width columns (a numerical step, not a formula constant): twelve steps
/// put the sample within 1/4096 of the column gap.
const BUDGET_EDGE_REFINE_STEPS: usize = 12;

/// The most midpoint split passes on the smooth stretches of the
/// force-budget edge (a numerical step, not a formula constant): each pass
/// halves every segment whose straight line stands above the solved depth,
/// and the passes stop when no segment splits. P29-7 7c: the edge is the
/// lowest over the declared cuts, which has a kink where two cuts cross and
/// a steep stretch where the slot ring's unlifted chip moves with the lift
/// at a narrow width; four passes left the line 3 % above the solved depth
/// there (3.175 mm 2F hardwood Pocket, ae 0.059 mm). Twelve is the cap of
/// the vendor depth table edge.
const BUDGET_EDGE_SPLIT_LEVELS: usize = 12;

/// P19: the most midpoint split passes on the vendor depth table edge (a
/// numerical step, not a formula constant). The edge has corners where the
/// chip share crosses 0.5, 0.75 and 1 (`vendor_depth_table_ceiling_ratio`);
/// a straight line across a corner stands off it by more than the solver
/// resolution after four passes on the 8-column grid. The passes stop when
/// no segment splits.
const TABLE_EDGE_SPLIT_LEVELS: usize = 12;

/// P19: whether a limit is drawn (the boundary) or only read by the pick
/// (`pick_limits`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drawn {
    /// In the boundary: drawn and judged.
    Yes,
    /// In `pick_limits`: not drawn.
    No,
}

/// P19: how [`refine_columns`] splits the smooth stretches.
#[derive(Debug, Clone, Copy)]
struct SplitRule {
    /// The most midpoint split passes.
    levels: usize,
    /// Split also where the straight line stands under the solved value.
    /// The force-budget edge splits only where the line stands above it
    /// (P13c: the drawn edge must not draw depths the model refuses); the
    /// vendor depth table splits on both sides, so a drawn point and the
    /// exact judgement agree (lead ruling, P19).
    two_sided: bool,
}

/// P13c fix (lead review 2026-11-06): the force-budget depth is not a
/// smooth function of the width. The peak engaged-edge force of a helical
/// edge plateaus once a full flute pitch is engaged, so where the budget
/// sits inside that plateau a small width step moves the shallowest
/// crossing by millimetres (measured: 3.175 mm 2F in hardwood, ae 2.317 ->
/// 2.324 mm, the depth 8.65 -> 5.65 mm). A straight line between two solved
/// columns then draws depths the model refuses, and a point the envelope
/// places inside can beat the exact proposal. Between each pair of adjacent
/// columns this bisects the width at which the depth leaves the left
/// column's value for the right one's and adds the two bracketing samples,
/// so a cliff stands at its own width and a smooth stretch gains a midpoint.
fn refine_budget_columns(
    columns: &mut Vec<(f64, Option<Solved>)>,
    top: f64,
    solve: impl Fn(f64) -> Option<Solved>,
) {
    refine_columns(
        columns,
        |c: Option<Solved>| c.and_then(|(d, _)| d.ceiling_mm()).unwrap_or(top),
        solve,
        SplitRule {
            levels: BUDGET_EDGE_SPLIT_LEVELS,
            two_sided: false,
        },
    );
}

/// The refinement of [`refine_budget_columns`] for any column value `T`:
/// `depth` reads the drawn depth of a column, `solve` solves a width. A
/// column with no value (`None`) is not refined. P19: the vendor depth
/// table edge uses it too, so the drawn edge is within the solver
/// resolution of the exact ceiling over the plane.
fn refine_columns<T: Copy>(
    columns: &mut Vec<(f64, Option<T>)>,
    depth: impl Fn(Option<T>) -> f64,
    solve: impl Fn(f64) -> Option<T>,
    rule: SplitRule,
) {
    // P14 (STATUS 4d0): a pure memo of `solve` by the exact width bits. The
    // split loop re-solves the midpoint of every segment that did not split
    // on each level; `solve` is deterministic, so a cached answer is the
    // answer it would give again, and every output stays the same.
    let memo = std::cell::RefCell::new(std::collections::HashMap::<u64, Option<T>>::new());
    let solve = |ae: f64| {
        let key = ae.to_bits();
        if let Some(hit) = memo.borrow().get(&key) {
            return *hit;
        }
        let answer = solve(ae);
        memo.borrow_mut().insert(key, answer);
        answer
    };
    let mut added = Vec::new();
    for pair in columns.windows(2) {
        let [(a, ca), (b, cb)] = pair else {
            continue;
        };
        if ca.is_none() || cb.is_none() {
            continue;
        }
        let (da, db) = (depth(*ca), depth(*cb));
        if (da - db).abs() < crate::feeds::cutter_constraints::BINSEARCH_CONVERGENCE_MM {
            continue;
        }
        let (mut lo, mut hi) = ((*a, *ca), (*b, *cb));
        for _ in 0..BUDGET_EDGE_REFINE_STEPS {
            let mid = 0.5 * (lo.0 + hi.0);
            let cm = solve(mid);
            if cm.is_none() {
                break;
            }
            let dm = depth(cm);
            if (dm - da).abs() <= (dm - db).abs() {
                lo = (mid, cm);
            } else {
                hi = (mid, cm);
            }
        }
        for sample in [lo, hi] {
            if (sample.0 - a).abs() > 1e-12 && (sample.0 - b).abs() > 1e-12 {
                added.push(sample);
            }
        }
    }
    columns.extend(added);
    columns.sort_by(|x, y| x.0.total_cmp(&y.0));
    columns.dedup_by(|x, y| (x.0 - y.0).abs() < 1e-12);
    // Then the smooth stretches: split a segment at its midpoint while the
    // straight line there stands above the solved depth by more than the
    // solver's own resolution, up to `rule.levels` times (on both sides of
    // the line with `rule.two_sided`).
    for _ in 0..rule.levels {
        let mut split = Vec::new();
        for pair in columns.windows(2) {
            let [(a, ca), (b, cb)] = pair else {
                continue;
            };
            if ca.is_none() || cb.is_none() {
                continue;
            }
            let mid = 0.5 * (a + b);
            let cm = solve(mid);
            if cm.is_none() {
                continue;
            }
            let line = 0.5 * (depth(*ca) + depth(*cb));
            let off = if rule.two_sided {
                (line - depth(cm)).abs()
            } else {
                line - depth(cm)
            };
            if off > crate::feeds::cutter_constraints::BINSEARCH_CONVERGENCE_MM {
                split.push((mid, cm));
            }
        }
        if split.is_empty() {
            break;
        }
        columns.extend(split);
        columns.sort_by(|x, y| x.0.total_cmp(&y.0));
        columns.dedup_by(|x, y| (x.0 - y.0).abs() < 1e-12);
    }
}

/// The `Rigidity` limit (every cut whose depth limit is the force budget:
/// a roughing cut, and a slot or a groove under every role since P29-5).
/// P13c (O1; the Adaptive family since P29-4): per width sample, the shallowest depth where the peak
/// engaged-edge force at the column chip (the operator's, else the grid
/// chip at the shallowest sample) meets the machine force budget. Extra
/// exact samples at the slot threshold and the corner bound (P19: not at
/// the stated width, see `add_exact_ceilings`). A column with no root
/// takes the span top; no root anywhere, or no force line, is an
/// `Unmodelled` entry with its reason. Between adjacent columns
/// [`refine_budget_columns`] adds the samples of a cliff and of the smooth
/// stretches, so the drawn edge stands on the solved depth. P29-7 7c: each
/// solve is the lowest depth over the declared cuts that the plan always
/// cuts ([`budget_solve`]: the slot ring, the first slot, the planner cap);
/// the body reads the tool diameter for a slot or a groove (P29-5c), and
/// the column keeps its axis width. A chamfer states no depth: it is an
/// `Unmodelled` entry, judged after the simulation. Returns the model the
/// proposal solves exactly.
fn rigidity_limit(
    out: &mut Vec<Limit>,
    inputs: &OwnedInputs,
    speeds: &SpeedsMap,
    plan: &AxesPlan,
    operator: Option<OperatorChip>,
) -> Option<BudgetModel> {
    use crate::feeds::force_budget as fb;
    // P29-3: the depth limit of the op's body cut (P29-5: a slot or a
    // groove reads the budget under every role, ruling R3).
    let depth_limit = inputs.cuts.list().body_facts().depth_rule;
    let rule = inputs.machine.rigidity.depth_cap_of(depth_limit)?;
    let crate::machine::DepthCapRule { budget_n } = rule;
    let basis = budget_basis(&inputs.machine);
    let unmodelled = |reason: &'static str| Limit {
        kind: LimitKind::Rigidity,
        binding_cut: None,
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
        let mut extra = Vec::new();
        // P19: no sample at the stated width; `add_exact_ceilings` solves
        // the stated point without a change to the drawn edge.
        if plan.computed_width {
            extra.push(d * fb::corner_coverage_width_factor());
        }
        for ae in extra {
            if ae > *first && ae < *last && !widths.iter().any(|w| (w.0 - ae).abs() < 1e-12) {
                widths.push((ae, None));
            }
        }
    }
    widths.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut columns: Vec<(f64, Option<Solved>)> = Vec::with_capacity(widths.len());
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
        columns.push((ae, budget_solve(inputs, &tool, ae, fz, budget_n)));
    }
    if columns.iter().all(|(_, c)| c.is_none()) {
        out.push(unmodelled(fb::NO_BUDGET_INPUT));
        return None;
    }
    let top = speeds.aps.last().copied().unwrap_or(MIN_AP_MM);
    refine_budget_columns(&mut columns, top, |ae| {
        let fz = model.operator_fz.unwrap_or_else(|| {
            chip_of(
                &inputs.calc_at(CutPoint {
                    ap_mm: MIN_AP_MM,
                    ae_mm: ae,
                }),
                flutes,
            )
        });
        budget_solve(inputs, &tool, ae, fz, budget_n)
    });
    model.solved = SolvedColumns(
        columns
            .iter()
            .filter_map(|(ae, c)| c.map(|d| (*ae, d)))
            .collect(),
    );
    if columns
        .iter()
        .all(|(_, c)| matches!(c, None | Some((fb::BudgetDepth::NoRootBelowFlute, _))))
    {
        out.push(unmodelled(fb::NO_BUDGET_LIMIT_BELOW_THE_FLUTE));
        return Some(model);
    }
    let floored = columns
        .iter()
        .any(|(_, c)| matches!(c, Some((fb::BudgetDepth::BelowFloor, _))));
    let curve: Vec<[f64; 2]> = columns
        .into_iter()
        .map(|(ae, c)| [ae, c.and_then(|(d, _)| d.ceiling_mm()).unwrap_or(top)])
        .collect();
    let edge = match curve.as_slice() {
        [only] => Edge::ApCeiling { mm: only[1] },
        _ => Edge::ApCurve { points: curve },
    };
    out.push(Limit {
        kind: LimitKind::Rigidity,
        binding_cut: None,
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

/// P29-7 7c: the force-budget depth at the body column `(ae, fz)`: the
/// lowest over the declared cuts that the plan always cuts, through the one
/// driver, and the cut that gives it. The body reads the tool diameter when
/// it declares the slot width (a slot, a groove; P29-5c), and the column
/// width else; every other cut reads its own width and chip
/// (`planned::DeclaredCuts::planned`). A tie keeps the body.
fn budget_solve(
    inputs: &OwnedInputs,
    tool: &ToolDefinition,
    ae: f64,
    fz: f64,
    budget_n: f64,
) -> Option<Solved> {
    let env = ForceEnv {
        tool,
        material: &inputs.material,
        bound: budget_n,
    };
    let column = BodyColumn {
        ae_mm: ae,
        chip_mm: Some(fz),
        stored: None,
    };
    lowest_ceiling::<ForceBudgetPlanned>(&inputs.cuts, column, &env, budget_rank)
        .and_then(|(kind, answer)| answer.map(|d| (d, kind)))
}

/// P29-7 7c: the tip-deflection depth (mm) at the body column `(ae, fz)`:
/// the lowest root over the declared cuts that the plan always cuts, and
/// the cut that gives it. `None`: no cut has a root below the flute.
fn deflection_solve(
    inputs: &OwnedInputs,
    tool: &ToolDefinition,
    ae: f64,
    fz: f64,
) -> Option<(f64, CutKind)> {
    let env = ForceEnv {
        tool,
        material: &inputs.material,
        bound: inputs_deflection_limit_um(inputs),
    };
    let column = BodyColumn {
        ae_mm: ae,
        chip_mm: Some(fz),
        stored: None,
    };
    lowest_ceiling::<DeflectionPlanned>(&inputs.cuts, column, &env, |answer| *answer)
        .and_then(|(kind, answer)| answer.map(|mm| (mm, kind)))
}

/// P29-7 7c (ruling Q8): the vendor depth table ceiling at the body column
/// `(ae, fz)`: the lowest over the declared cuts that the plan always cuts,
/// whether the table binds there, and the cut that gives it. With no cut
/// that has a number, the span top (the table holds every printed depth).
fn table_solve(cuts: &DeclaredCuts, ae: f64, fz: f64, env: &TableEnv) -> ((f64, bool), CutKind) {
    let column = BodyColumn {
        ae_mm: ae,
        chip_mm: Some(fz),
        stored: None,
    };
    lowest_ceiling::<VendorDepthTablePlanned>(cuts, column, env, |answer| answer.map(|(mm, _)| mm))
        .and_then(|(kind, answer)| answer.map(|c| (c, kind)))
        .unwrap_or(((env.top_mm, false), CutKind::Body))
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

/// P19: the reason of a `Runtime` entry when no width up to the width
/// ceiling of the move budget meets the budget. A note gives the ceiling
/// in mm ([`move_budget_not_met_note`]).
pub const MOVE_BUDGET_NOT_MET: &str =
    "the move budget is not met at any width up to the width ceiling";

/// P29-7 7c (trap 2): the note when the move budget is not met at the depth
/// that the force budget allows. Suggest moves no number for it.
pub const MOVE_BUDGET_CONFLICT: &str = "the move budget is not met at the force-budget depth: \
     Suggest keeps the stated width and does not raise the depth over the force budget";

/// The cells of one chip check over the declared cuts.
type ByCutChip = crate::feeds::planned::ByCut<
    crate::feeds::planned::PlannedCell<Option<crate::feeds::planned::ChipReading>>,
>;

/// P19: the note that gives the width ceiling (mm) when the move budget is
/// not met at any width up to it.
#[must_use]
pub fn move_budget_not_met_note(ceiling_mm: f64) -> String {
    format!("the move budget is not met at any width up to {ceiling_mm:.2} mm")
}

/// P19: what the envelope keeps to judge a point against the move budget
/// when the `Runtime` limit is not one width floor (P19b: an `AeCurve` when
/// the move model reads the depth, or `Unmodelled` when no width up to the
/// ceiling meets the budget). `place` judges each point that is not
/// measured by `predict_move_count` at its own depth and width.
#[derive(Debug, Clone, Copy)]
struct MoveBudget {
    /// The model box.
    bbox: crate::geo::BoundingBox3,
    /// The move budget (`STEPOVER_BACKOFF_TARGET_MOVES_ROW`).
    target: f64,
}

/// The move budget limit (pass 3), when the context gives the model box and
/// the op has a free stepover.
///
/// - A move model that does not read the depth: the smallest width with
///   `predict_move_count` <= the budget, an `AeFloor`.
/// - P19b: a move model that reads the depth (`move_count_reads_depth`) on
///   a free depth: an `AeCurve` floor, the smallest width per depth column
///   (bisection of the predictor from the plane only, the stated cut never
///   read), refined between columns like the vendor depth table edge. The
///   saturated shallow depths have no sample: the curve starts at the
///   shallowest depth that meets the budget at the ceiling (no note: the
///   drawn start shows it). `place` still judges each point exactly.
/// - P19: no width up to the ceiling meets the budget: `Unmodelled` with
///   [`MOVE_BUDGET_NOT_MET`] and a note with the ceiling in mm. No floor is
///   drawn at the ceiling (points on it also fail the budget).
///
/// The curve and the `Unmodelled` arm return the [`MoveBudget`] that
/// `place` judges each point with, so the stated point still warns.
fn runtime_limit(
    out: &mut Vec<Limit>,
    notes: &mut Vec<EnvelopeNote>,
    input: &EnvelopeInput<'_>,
    plan: &AxesPlan,
    d: f64,
) -> Option<MoveBudget> {
    // EXEMPT(P28-09)
    if !plan.axes.ae_free() || input.operation.stepover().is_none() {
        return None;
    }
    let bbox = *input.context.model_bbox()?;
    let target = crate::feeds::suggest::STEPOVER_BACKOFF_TARGET_MOVES_ROW.value;
    let ceiling = d * crate::feeds::suggest::STEPOVER_BACKOFF_DIAMETER_FRACTION_ROW.value;
    let ap_span = match plan.axes {
        EnvelopeAxes::Plane { ap, .. } | EnvelopeAxes::AxialBar { ap, .. } => Some(ap),
        EnvelopeAxes::RadialBar { .. } | EnvelopeAxes::Fixed { .. } => None,
    };
    let reads_depth = ap_span.is_some()
        && crate::feeds::predict::move_count_reads_depth(input.operation.capabilities().move_count);
    // The width `s` and, on a model that reads it, the depth `ap`.
    let within = |ap: Option<f64>, s: f64| {
        let mut op = input.operation.clone();
        let _ = op.set_stepover(s);
        if let Some(ap) = ap {
            let _ = op.set_depth_per_pass(ap);
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "a move count; the budget is 5e5, exact in an f64"
        )]
        let moves =
            crate::feeds::predict::predict_move_count(&op, Some(&bbox), input.tool, input.machine)
                as f64;
        moves <= target
    };
    let limit = |edge: Edge| Limit {
        kind: LimitKind::Runtime,
        binding_cut: None,
        law: "predicted moves <= the move budget",
        basis: Basis::derived(
            "move budget",
            vec![
                Basis::Rule(crate::feeds::suggest::STEPOVER_BACKOFF_TARGET_MOVES_ROW),
                Basis::Rule(crate::feeds::suggest::STEPOVER_BACKOFF_DIAMETER_FRACTION_ROW),
            ],
        ),
        edge,
    };
    let judge = MoveBudget { bbox, target };
    // The smallest width with the moves inside the budget, by bisection
    // in [lo, hi] (`within(hi)` holds, `within(lo)` does not).
    let bisect = |ap: Option<f64>| {
        let (mut lo, mut hi) = (MIN_AE_MM, ceiling);
        for _ in 0..RUNTIME_BISECTION_STEPS {
            let mid = 0.5 * (lo + hi);
            if within(ap, mid) {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        hi
    };
    if let (true, Some(span)) = (reads_depth, ap_span) {
        // The move count falls as the depth and the width grow: the
        // shallowest, narrowest corner is the worst point of the plane.
        if within(Some(span.min_mm), MIN_AE_MM) {
            return None;
        }
        // P19b: the width floor per depth column, from the plane only.
        // `None`: no width up to the ceiling meets the budget at that depth
        // (saturated); `MIN_AE_MM` where the narrowest width meets it.
        let floor_at = |ap: f64| {
            if !within(Some(ap), ceiling) {
                None
            } else if within(Some(ap), MIN_AE_MM) {
                Some(MIN_AE_MM)
            } else {
                Some(bisect(Some(ap)))
            }
        };
        let mut columns: Vec<(f64, Option<f64>)> = samples(span)
            .into_iter()
            .map(|ap| (ap, floor_at(ap)))
            .collect();
        if columns.iter().all(|(_, c)| c.is_none()) {
            out.push(limit(Edge::Unmodelled {
                reason: MOVE_BUDGET_NOT_MET,
            }));
            notes.push(EnvelopeNote::MoveBudgetNotMet {
                ceiling_mm: ceiling,
            });
            return Some(judge);
        }
        // The saturated columns are the shallow ones (the count falls with
        // the depth). The curve starts at the shallowest depth that meets
        // the budget at the ceiling, by bisection between the last
        // saturated column and the first met one; no floor is drawn above
        // the ceiling, so a shallower depth is not met at any width. The
        // drawn start says so; no note (a note is for a budget that no
        // point of the plane meets).
        if let Some(first_met) = columns.iter().position(|(_, c)| c.is_some())
            && first_met > 0
            && let (Some(&(lo_ap, _)), Some(&(hi_ap, _))) =
                (columns.get(first_met - 1), columns.get(first_met))
        {
            let (mut lo, mut hi) = (lo_ap, hi_ap);
            for _ in 0..RUNTIME_BISECTION_STEPS {
                let mid = 0.5 * (lo + hi);
                if within(Some(mid), ceiling) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            columns.retain(|(_, c)| c.is_some());
            columns.insert(0, (hi, Some(bisect(Some(hi)))));
        }
        refine_columns(
            &mut columns,
            |c: Option<f64>| c.unwrap_or(ceiling),
            floor_at,
            SplitRule {
                levels: TABLE_EDGE_SPLIT_LEVELS,
                two_sided: true,
            },
        );
        let points: Vec<[f64; 2]> = columns
            .iter()
            .filter_map(|(ap, c)| c.map(|ae| [*ap, ae]))
            .collect();
        out.push(limit(Edge::AeCurve {
            bound: CurveBound::Floor,
            points,
        }));
        return Some(judge);
    }
    if within(None, MIN_AE_MM) {
        return None;
    }
    if !within(None, ceiling) {
        out.push(limit(Edge::Unmodelled {
            reason: MOVE_BUDGET_NOT_MET,
        }));
        notes.push(EnvelopeNote::MoveBudgetNotMet {
            ceiling_mm: ceiling,
        });
        return Some(judge);
    }
    out.push(limit(Edge::AeFloor { mm: bisect(None) }));
    None
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
            binding_cut: None,
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
            binding_cut: None,
            law: "min(0.025, band min) per tooth (warns, never lifts)",
            basis,
            edge: Edge::PerPoint,
        });
    }
    if speeds.points.iter().any(|p| p.feed_capped) {
        out.push(Limit {
            kind: LimitKind::FeedCap,
            binding_cut: None,
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
        // P19: a model box (40 x 40 x 12 mm, the P19 audit box), so the
        // envelope builds the `Runtime` limit and the time budget holds it.
        let bbox = crate::geo::BoundingBox3 {
            min: crate::geo::P3::new(0.0, 0.0, -12.0),
            max: crate::geo::P3::new(40.0, 40.0, 0.0),
        };
        SafeEnvelope::for_cut(EnvelopeInput {
            operation: &op,
            tool: &tool,
            machine: &machine,
            material: &material,
            lut: crate::feeds::embedded_vendor_lut(),
            spindle_strategy: SpindleStrategy::default(),
            context: SuggestContext::no_project(crate::feeds::suggest::NoProject {
                model_bbox: Some(&bbox),
                ..crate::feeds::suggest::NoProject::NONE
            }),
            modulation: crate::dressup::feed_modulation::RunModulation::Off,
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
        assert!(
            env.limit(LimitKind::Runtime).is_some(),
            "the timed envelope builds the move budget limit"
        );
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
        assert!(p.ap_mm().expect("a pocket depth") <= stated.ap_mm + 1e-12);
        assert!(p.ae_mm <= stated.ae_mm + 1e-12);
    }

    /// P29-10 section 15: the ceiling-only driver of the edges gives the
    /// answer of the full driver and `lowest_always`, at every width of a
    /// sweep, for the force budget, the deflection and the vendor depth
    /// table, on a list with entries (a ramp `Always` at its own depth, a
    /// helix and a plunge `MayOccur`) and a slot ring.
    #[test]
    fn the_ceiling_driver_gives_the_lowest_always_answer_p29_10() {
        use crate::feeds::planned::{judge_declared_cuts, lowest_ceiling};
        let mut entries = 0;
        for env in [pocket_envelope(), hardwood_3175_pocket()] {
            let cuts = &env.inputs.cuts;
            entries += cuts
                .list()
                .iter()
                .filter(|c| matches!(c.kind, CutKind::Entry(_)))
                .count();
            let tool = crate::compute::cutter::build_cutter(&env.inputs.tool).unwrap();
            let force = ForceEnv {
                tool: &tool,
                material: &env.inputs.material,
                bound: env.inputs.machine.rigidity.force_budget_n,
            };
            let deflection = ForceEnv {
                bound: inputs_deflection_limit_um(&env.inputs),
                ..force
            };
            // A test input, not a claim: with the chips of the sweep the
            // share crosses every corner of the table.
            let table = TableEnv {
                band_max_mm: 0.05,
                d_mm: env.inputs.tool.diameter,
                top_mm: 3.0 * env.inputs.tool.diameter,
            };
            let d = env.inputs.tool.diameter;
            for i in 1..=40 {
                let ae = d * f64::from(i) / 40.0;
                for fz in [0.01, 0.03, 0.08] {
                    let column = BodyColumn {
                        ae_mm: ae,
                        chip_mm: Some(fz),
                        stored: None,
                    };
                    let full = judge_declared_cuts::<ForceBudgetPlanned>(cuts, column, &force)
                        .lowest_always(budget_rank);
                    let fast =
                        lowest_ceiling::<ForceBudgetPlanned>(cuts, column, &force, budget_rank);
                    assert_eq!(format!("{full:?}"), format!("{fast:?}"), "force {ae} {fz}");
                    let full = judge_declared_cuts::<DeflectionPlanned>(cuts, column, &deflection)
                        .lowest_always(|a| *a);
                    let fast =
                        lowest_ceiling::<DeflectionPlanned>(cuts, column, &deflection, |a| *a);
                    assert_eq!(
                        format!("{full:?}"),
                        format!("{fast:?}"),
                        "deflection {ae} {fz}"
                    );
                    let full = judge_declared_cuts::<VendorDepthTablePlanned>(cuts, column, &table)
                        .lowest_always(|a| a.map(|(mm, _)| mm));
                    let fast =
                        lowest_ceiling::<VendorDepthTablePlanned>(cuts, column, &table, |a| {
                            a.map(|(mm, _)| mm)
                        });
                    assert_eq!(format!("{full:?}"), format!("{fast:?}"), "table {ae} {fz}");
                }
            }
        }
        assert!(entries > 0, "non-vacuity: the lists hold entries");
    }

    /// The 3.175 mm 2F hardwood Pocket of
    /// `the_pocket_proposal_is_the_removal_rate_maximum_p13c` (the FM1 tool,
    /// the Freud chip): free width and depth, no stated speeds.
    fn hardwood_3175_pocket() -> SafeEnvelope {
        use crate::compute::geometry_default::slot_mut;
        use crate::compute::tool_config::ToolFact;
        let mut op = OperationConfig::new_default(OperationType::Pocket);
        for field in ["stepover", "depth_per_pass"] {
            if let Some(slot) = slot_mut(&mut op, field) {
                *slot = None;
            }
        }
        op.set_spindle_rpm(None);
        let mut tool = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
        tool.diameter = 3.175;
        tool.set_flute_count(ToolFact::Stated(2));
        tool.set_cutting_length(ToolFact::Stated(12.0));
        tool.set_shank_diameter(ToolFact::Stated(3.175));
        tool.shaft_diameter = 3.175;
        tool.set_stickout(ToolFact::Stated(tool.cutting_length_planning_mm() + 8.0));
        let machine = MachineProfile::default();
        let material = Material::SolidWood {
            species: crate::material::WoodSpecies::GenericHardwood,
        };
        SafeEnvelope::for_cut(EnvelopeInput {
            operation: &op,
            tool: &tool,
            machine: &machine,
            material: &material,
            lut: crate::feeds::embedded_vendor_lut(),
            spindle_strategy: SpindleStrategy::default(),
            context: SuggestContext::no_project(crate::feeds::suggest::NoProject::NONE),
            modulation: crate::dressup::feed_modulation::RunModulation::Off,
        })
        .unwrap()
    }

    /// P13c fix: the drawn force-budget edge never stands above the solved
    /// depth. The peak force of a helical edge plateaus with depth, and
    /// where the budget sits in that plateau the solved depth drops by
    /// millimetres over a small width step (8.65 -> 5.65 mm at ae 2.317 ->
    /// 2.324 mm here). A straight line between two solved columns drew
    /// 6.9 mm at ae 2.381 mm where the model allows 5.46 mm, so the envelope
    /// placed that point inside and it beat the exact proposal. Red before
    /// `refine_budget_columns`.
    #[test]
    fn the_drawn_force_budget_edge_stands_on_the_solved_depth() {
        let env = hardwood_3175_pocket();
        let edge = env.limit(LimitKind::Rigidity).unwrap();
        let top = *env.speeds.aps.last().unwrap();
        let (lo, hi) = (
            *env.speeds.aes.first().unwrap(),
            *env.speeds.aes.last().unwrap(),
        );
        let mut worst = (0.0, 0.0, 0.0);
        for i in 0..=400 {
            let ae = lo + (hi - lo) * f64::from(i) / 400.0;
            let drawn = edge.ap_ceiling_at(ae).unwrap();
            let solved = env
                .budget_depth_at(ae, BudgetChip::Curve)
                .and_then(|(d, _)| d.ceiling_mm())
                .unwrap_or(top);
            if drawn - solved > worst.1 - worst.2 {
                worst = (ae, drawn, solved);
            }
        }
        println!("worst (ae, drawn, solved) = {worst:?}");
        // The removal-rate search compares its pick with the placed points
        // within 0.5 %; the drawn edge must not stand higher than that.
        assert!(
            worst.1 <= worst.2 * 1.005,
            "the drawn edge stands {} mm above the solved depth at ae {}",
            worst.1 - worst.2,
            worst.0
        );
    }
}
