//! Feeds contract P29-2: one name for each limit, and one record for each
//! check of a value against a limit.
//!
//! - [`LimitId`]: the one list of limits. It stands in the place of
//!   `feeds::envelope::LimitKind` (before the simulation),
//!   `tool_load::verdict::CriterionKind` (the gates),
//!   `tool_load::verdict::BindingConstraint` (the modulator),
//!   `feeds::RpmLimit` (the RPM choice) and
//!   `feeds::cutter_constraints::AxialBindingConstraint`. Each old enum maps
//!   to it in a match that names every variant.
//! - [`Check`] (`check.rs`): the one record.
//! - `project.rs`: the projections of the old records to [`Check`].
//! - [`Cut`] (`cut.rs`, P29-3): one cut that a limit judges.
//! - [`EXEMPTIONS`] (`exemptions.rs`, P29-3): the one list of every
//!   exemption from a limit.
//! - [`SampleContext`] (`sample.rs`, P29-3): the side tables of
//!   `Cut::of_sample`, the cut of one simulation sample.
//!
//! In P29-2 the old types stay and no surface reads this folder.
//!
//! No signature of this folder holds an operation type, an operation
//! family or a pass role (`P29_FINDINGS.md` section 4.5): a limit cannot
//! branch on what it cannot see.

mod check;
mod cut;
mod exemptions;
mod project;
mod sample;

pub use check::{BoundSide, Check, CheckState, Geometry, NoNumberReason, basis_of_bound};
pub use cut::{
    CleanupKind, Cut, CutEngagement, CutFacts, CutKind, CutSource, DepthLimit, EntryKind,
    FirstPassKind, Leave, NoDepthLimit, NoEngagementModel, SampleDepth,
};
pub use exemptions::{EXEMPTIONS, Exemption, ExemptionClass, Who};
pub use project::{of_criterion, of_envelope, of_judged_value, of_rpm_choice};
pub use sample::{CapTable, MoveCap, SampleContext};

use serde::Serialize;

use crate::feeds::RpmLimit;
use crate::feeds::cutter_constraints::AxialBindingConstraint;
use crate::feeds::envelope::LimitKind;
use crate::tool_load::verdict::{BindingConstraint, ChipSide, CriterionKind};

/// The one list of limits (P29-2). A member is a physical or a machine
/// limit, not a surface: the spindle power is one member on the envelope,
/// in the gate, in the modulator and in the RPM choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitId {
    /// The rated spindle power at the spindle speed.
    SpindlePower,
    /// The tip deflection of the tool.
    ToolDeflection,
    /// The machine force budget: the peak engaged-edge force.
    ForceBudget,
    /// The printed chip band, upper edge (the peak chip).
    ChipMax,
    /// The printed chip band, lower edge.
    ChipMin,
    /// The rubbing floor `min(0.025, band min)`: it warns, never lifts.
    RubbingFloor,
    /// The flute guard (0.8 x the flute length).
    FluteGuard,
    /// The cutting length of the tool.
    CuttingLength,
    /// The vendor row's axial bound.
    VendorAxial,
    /// The vendor depth table (1, 2, 3 x D).
    VendorDepthTable,
    /// The corner coverage of an offset ring.
    CornerCoverage,
    /// The depth at which a ball leaves the scallop target.
    ScallopDepth,
    /// The tool diameter as a width ceiling.
    ToolDiameter,
    /// The move budget of the plan (a width floor).
    MoveBudget,
    /// The smallest depth and the smallest width the calculator writes.
    EngagementFloor,
    /// The cutting-feed ceiling of the machine or of the operation dial.
    CuttingFeedCap,
    /// The feed cap a planner wrote on a move (`Move::feed_cap`).
    PlannerFeedCap,
    /// The feed a move can reach (acceleration and junctions).
    KinematicReach,
    /// The plunge rate of the operation on a descent.
    PlungeRate,
    /// The largest spindle speed of the machine.
    SpindleMaxRpm,
    /// The Z axis rate of the machine.
    ZAxisRate,
    /// The RPM ceiling the operator set.
    OperatorRpmCeiling,
    /// The chart RPM (the ceiling when the operator set none).
    ChartRpm,
    /// The largest RPM the vendor row prints.
    VendorRpmMax,
    /// The thrust of the gantry (no model).
    GantryPush,
    /// Drill: chip welding.
    DrillChipWelding,
    /// Drill: the peck depth.
    DrillPeckAdequacy,
    /// Drill: the plunge feed envelope.
    DrillPlungeFeed,
}

impl LimitId {
    /// Every member, in declaration order.
    pub const ALL: [Self; 28] = crate::all_variants!(LimitId:
        SpindlePower, ToolDeflection, ForceBudget, ChipMax, ChipMin, RubbingFloor,
        FluteGuard, CuttingLength, VendorAxial, VendorDepthTable, CornerCoverage, ScallopDepth,
        ToolDiameter, MoveBudget, EngagementFloor, CuttingFeedCap, PlannerFeedCap,
        KinematicReach, PlungeRate, SpindleMaxRpm, ZAxisRate, OperatorRpmCeiling, ChartRpm,
        VendorRpmMax, GantryPush, DrillChipWelding, DrillPeckAdequacy, DrillPlungeFeed,
    );

    /// The limit of an envelope limit kind. `LimitKind::Rigidity` is the
    /// force budget (P29-4).
    ///
    /// One old name for two limits: `ChipBand` maps to [`Self::ChipMax`]
    /// (the envelope judges the peak chip against the band; its floor is
    /// the `RubbingFloor` kind), and `EngagementFloor` is one member for
    /// the depth floor and the width floor.
    #[must_use]
    pub const fn of_limit_kind(kind: LimitKind) -> Self {
        match kind {
            LimitKind::FluteGuard => Self::FluteGuard,
            LimitKind::CuttingLength => Self::CuttingLength,
            LimitKind::VendorAxial => Self::VendorAxial,
            LimitKind::VendorDepthTable => Self::VendorDepthTable,
            LimitKind::Rigidity => Self::ForceBudget,
            LimitKind::CornerCoverage => Self::CornerCoverage,
            LimitKind::Power => Self::SpindlePower,
            LimitKind::Deflection => Self::ToolDeflection,
            LimitKind::ChipBand => Self::ChipMax,
            LimitKind::RubbingFloor => Self::RubbingFloor,
            LimitKind::FeedCap => Self::CuttingFeedCap,
            LimitKind::Diameter => Self::ToolDiameter,
            LimitKind::Runtime => Self::MoveBudget,
            LimitKind::EngagementFloor => Self::EngagementFloor,
        }
    }

    /// The limit of a gate criterion. `CriterionKind::DepthOfCut` is the
    /// force budget (P29-4); `side` tells the two limits that
    /// `CriterionKind::Chipload` names.
    #[must_use]
    pub const fn of_criterion(kind: CriterionKind, side: ChipSide) -> Self {
        match kind {
            CriterionKind::Chipload => match side {
                ChipSide::High => Self::ChipMax,
                ChipSide::Low => Self::ChipMin,
            },
            CriterionKind::Power => Self::SpindlePower,
            CriterionKind::Deflection => Self::ToolDeflection,
            CriterionKind::DepthOfCut => Self::ForceBudget,
            CriterionKind::GantryPush => Self::GantryPush,
            CriterionKind::DrillChipWelding => Self::DrillChipWelding,
            CriterionKind::DrillPeckAdequacy => Self::DrillPeckAdequacy,
            CriterionKind::DrillPlungeFeed => Self::DrillPlungeFeed,
        }
    }

    /// The limit of a binding constraint of the feed modulator.
    ///
    /// One word for two limits: `BindingConstraint::FeedCap` is the
    /// planner's cap on a move ([`Self::PlannerFeedCap`]);
    /// `LimitKind::FeedCap` is the machine ceiling
    /// ([`Self::CuttingFeedCap`]), which the modulator names
    /// `MachineMaxFeed`.
    #[must_use]
    pub const fn of_binding(binding: BindingConstraint) -> Self {
        match binding {
            BindingConstraint::ChiploadMax => Self::ChipMax,
            BindingConstraint::DeflectionMax => Self::ToolDeflection,
            BindingConstraint::PowerMax => Self::SpindlePower,
            BindingConstraint::MachineMaxFeed => Self::CuttingFeedCap,
            BindingConstraint::KinematicReach => Self::KinematicReach,
            BindingConstraint::ChiploadMin => Self::ChipMin,
            BindingConstraint::PlungeRate => Self::PlungeRate,
            BindingConstraint::FeedCap => Self::PlannerFeedCap,
            BindingConstraint::ForceBudget => Self::ForceBudget,
        }
    }

    /// The limit of a limit of the RPM choice. The source of a cutting
    /// ceiling (the machine or the operation dial) is a basis, not a limit.
    #[must_use]
    pub const fn of_rpm_limit(limit: RpmLimit) -> Self {
        match limit {
            RpmLimit::SpindleMax => Self::SpindleMaxRpm,
            RpmLimit::CuttingCeiling(_) => Self::CuttingFeedCap,
            RpmLimit::ZAxisRate => Self::ZAxisRate,
            RpmLimit::PowerAtRpm => Self::SpindlePower,
            RpmLimit::OperatorCeiling => Self::OperatorRpmCeiling,
            RpmLimit::ChartRpm => Self::ChartRpm,
            RpmLimit::VendorRpmMax => Self::VendorRpmMax,
        }
    }

    /// The limit of a binding bound of the axial envelope, or `None` for
    /// the two variants that are a state and not a limit (`Unbounded`: no
    /// bound exists; `SafeBandEmpty`: no depth meets both sides).
    #[must_use]
    pub const fn of_axial_binding(binding: AxialBindingConstraint) -> Option<Self> {
        match binding {
            AxialBindingConstraint::Deflection => Some(Self::ToolDeflection),
            AxialBindingConstraint::VendorAp => Some(Self::VendorAxial),
            AxialBindingConstraint::Scallop => Some(Self::ScallopDepth),
            AxialBindingConstraint::Unbounded | AxialBindingConstraint::SafeBandEmpty => None,
        }
    }
}
