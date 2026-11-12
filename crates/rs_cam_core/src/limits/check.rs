//! Feeds contract P29-2: [`Check`], the one record of a value against a
//! limit.

use serde::Serialize;

use super::LimitId;
use crate::feeds::sourced::{Basis, DRILL_ENVELOPE_LAW, MachineField, RowRef};
use crate::tool_load::verdict::{BoundSource, GatePopulation, UnmodeledReason};

/// Which side of the bound is inside.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundSide {
    /// The value must stay at or below the bound.
    Ceiling,
    /// The value must stay at or above the bound.
    Floor,
}

impl BoundSide {
    /// The signed margin of `value` against `bound`, as a share of the
    /// bound: positive inside, negative outside (the rule of
    /// `feeds::envelope::Limit::margin`). `None` for a bound that is not
    /// positive.
    #[must_use]
    pub fn margin(self, value: f64, bound: f64) -> Option<f64> {
        match self {
            Self::Ceiling => (bound > 0.0).then(|| (bound - value) / bound),
            Self::Floor => (bound > 0.0).then(|| (value - bound) / bound),
        }
    }
}

/// A geometry reason for which a limit has no meaning on a cut. The list
/// is closed, and it has no member for "deferred", "kept" or "not yet": a
/// limit that an operation skips by a scope decision is never
/// [`CheckState::NotApplicable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Geometry {
    /// The cut has no radial engagement (a straight plunge, a drill cycle).
    NoRadialEngagement,
    /// The cut is not an offset ring (the corner-coverage rule).
    NotAnOffsetRing,
    /// The cut has no vertical feed (the Z axis rate).
    NoVerticalFeed,
}

/// Why a check holds no judgement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NoNumberReason {
    /// The material has no force line (ruling B6).
    NoForceLine,
    /// No simulation ran.
    NoSimulation,
    /// The simulation is stale.
    StaleSimulation,
    /// No vendor row gives a chip band.
    NoVendorRow,
    /// Every sample is a rapid or an air cut.
    NoMaterialRemoved,
    /// The trace holds no engagement arc.
    NoArc,
    /// No built tool: the beam has no body.
    NoToolBody,
    /// The model does not cover the cut (the cutter mode, a model that
    /// does not exist yet).
    NotModelled,
    /// P29-9 9b: the value is a lower bound of the cut (a Project Curve
    /// depth: the stock above the model surface adds to it). The limit
    /// holds at the bound, so the check proves nothing: only a bound that
    /// fails proves `Exceeds`.
    OnlyALowerBound,
    /// A value with no bound. This is a scope exemption of the P28 audit
    /// (today: the depth of a finish or semi-finish pass, ruling R2), NOT a
    /// geometry reason. P29-3 lists it for a ruling. It is never
    /// [`CheckState::NotApplicable`].
    NoBound,
    /// The envelope does not judge the point against this limit today: a
    /// limit on a fixed axis, a limit judged per grid point, a drawn curve
    /// that an exact rule replaces. A scope fact of today, listed in P29-3.
    NotJudgedHere,
}

impl NoNumberReason {
    /// The reason of a refusal of the one load (`feeds::cut_load`).
    #[must_use]
    pub const fn of_load(refusal: crate::feeds::cut_load::NoNumber) -> Self {
        use crate::feeds::cut_load::NoNumber;
        match refusal {
            NoNumber::NoForceLine => Self::NoForceLine,
            NoNumber::NoToolBody => Self::NoToolBody,
            NoNumber::NoFeed | NoNumber::NoSpeed | NoNumber::NotModelled => Self::NotModelled,
        }
    }
}

/// The outcome of a check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state", content = "reason")]
pub enum CheckState {
    /// The value is inside the bound.
    Within,
    /// The value is outside the bound.
    Exceeds,
    /// No judgement, with the reason.
    NoNumber(NoNumberReason),
    /// The limit has no meaning on this cut, by geometry.
    NotApplicable(Geometry),
}

impl CheckState {
    /// The state of a gate refusal. The match names every reason.
    ///
    /// `NotApplicableForOp` is today only the refusal of a drill cycle
    /// ("no continuous engagement"): a straight plunge, so
    /// [`Geometry::NoRadialEngagement`].
    #[must_use]
    pub const fn of_unmodeled(reason: &UnmodeledReason) -> Self {
        match reason {
            UnmodeledReason::SimulationRequired => Self::NoNumber(NoNumberReason::NoSimulation),
            UnmodeledReason::StaleSimulation => Self::NoNumber(NoNumberReason::StaleSimulation),
            UnmodeledReason::ArcEngagementNotCaptured => Self::NoNumber(NoNumberReason::NoArc),
            UnmodeledReason::NoVendorData => Self::NoNumber(NoNumberReason::NoVendorRow),
            UnmodeledReason::MaterialUnvalidated => Self::NoNumber(NoNumberReason::NoForceLine),
            UnmodeledReason::CutterModeUnsupported(_) | UnmodeledReason::NotImplemented(_) => {
                Self::NoNumber(NoNumberReason::NotModelled)
            }
            UnmodeledReason::NotApplicableForOp(_) => {
                Self::NotApplicable(Geometry::NoRadialEngagement)
            }
            UnmodeledReason::AllSamplesAirCutOrRapid => {
                Self::NoNumber(NoNumberReason::NoMaterialRemoved)
            }
        }
    }

    /// The state of a margin: outside below `-1e-9` (the tolerance of
    /// `SafeEnvelope::place`), else inside.
    #[must_use]
    pub fn of_margin(margin: f64) -> Self {
        if margin < -1e-9 {
            Self::Exceeds
        } else {
            Self::Within
        }
    }
}

/// THE record of one value against one limit (P29-2). Every surface reads
/// this record from P29-10; in P29-2 the old records project to it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Check {
    /// The limit.
    pub limit: LimitId,
    /// The value that was judged, in `unit`. For a gate over samples it is
    /// the statistic the gate judged (the peak; the median on the low side
    /// of the chip band). `None` when there is no value.
    pub value: Option<f64>,
    /// The bound at this cut, in `unit`. `None` when there is no bound.
    pub bound: Option<f64>,
    /// The unit of `value` and `bound`.
    pub unit: &'static str,
    /// Which side of the bound is inside.
    pub side: BoundSide,
    /// The margin as a share of the bound: positive inside, negative
    /// outside. `None` with no value, no bound or a bound that is not
    /// positive.
    pub margin: Option<f64>,
    /// The outcome.
    pub state: CheckState,
    /// Where the BOUND comes from. `None` when the old record states none.
    pub basis: Option<Basis>,
    /// Transitional: the numbers of the gate's bound source that `Basis`
    /// does not hold (the RPM of the power curve, the leave, the cut of the
    /// force budget). The one writer of the
    /// card text, `BoundSource::clause`, reads it. It goes at P29-10.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bound_detail: Option<BoundSource>,
    /// `true` when this limit set the value: the limit that places a point
    /// outside the envelope, the limit of the RPM choice. A gate row is
    /// never binding (each gate judges alone).
    pub binding: bool,
    /// The samples behind the value (contributing over offered). `None`
    /// for a planned cut, or when the gate stated none.
    pub population: Option<GatePopulation>,
}

/// THE basis join (P29-2): the basis of a gate's bound source. The match
/// names every variant. `Basis::setting` and `Basis::gates_export` of the
/// result equal the methods of the source (sentry
/// `every_limit_is_a_limit_id_p29_2`).
///
/// The numbers inside the source are not in the basis; a [`Check`] keeps
/// them in `bound_detail` until P29-10.
#[must_use]
pub fn basis_of_bound(source: &BoundSource) -> Basis {
    match source {
        BoundSource::MachinePowerCurve { .. } => Basis::Machine(MachineField::PowerCurve),
        BoundSource::VendorChipBand { source, .. } => {
            Basis::Printed(RowRef(source.row_id().to_owned()))
        }
        BoundSource::DeflectionBudget => {
            Basis::Rule(crate::feeds::suggest::DEFLECTION_BACKOFF_TARGET_ROW)
        }
        BoundSource::StockToLeave { .. } => Basis::Operator {
            law: "radial stock to leave",
        },
        BoundSource::MachineForceBudget { .. } => Basis::Machine(MachineField::Rigidity),
        BoundSource::DrillEnvelope => Basis::Derived {
            law: DRILL_ENVELOPE_LAW,
            from: Vec::new(),
        },
    }
}
