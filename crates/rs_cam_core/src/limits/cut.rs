//! Feeds contract P29-3 (commit A): the cut object.
//!
//! [`Cut`] is one cut that a limit judges: its kind, its engagement, its
//! operating point, where it comes from, and the geometry facts that stand
//! in the place of the labels of the operation. Every part is `Copy` and
//! holds no heap value, so a cut for each simulation sample allocates
//! nothing.
//!
//! In commit A no reader builds a cut from an operation: only the straight
//! plunge of a drill cycle reads one (`feeds::cut_load::CutLoad::of_cut`).
//! The facts hold the values of today's labels. Each fact is a row of the
//! exemption list (`exemptions.rs`, ids `F-*`) until P29-4 to P29-7 give it
//! a geometry value.
//!
//! No type of this file names an operation type, an operation family or a
//! pass role (`CLAUDE.md` invariant 1).

use crate::adaptive3d::FeedCapSource;
use crate::feeds::ToolGeometryHint;
use crate::feeds::cut_load::OperatingPoint;
use crate::feeds::engagement::Engagement;
use crate::feeds::geometry::RadialThinning;
use crate::feeds::section::EngagedSection;

use super::DepthRule;

/// One kind of entry into material. `compute::catalog::entry_contract`
/// re-exports it: the entry contract of each op names these kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// A straight descent at one XY.
    Plunge,
    /// A descent along a line or a zigzag.
    Ramp,
    /// A descent along a helix.
    Helix,
    /// Plunges at one XY with a retract between them (3D Rough).
    PeckLadder,
}

impl EntryKind {
    /// Every kind, in declaration order.
    pub const ALL: [Self; 4] = [Self::Plunge, Self::Ramp, Self::Helix, Self::PeckLadder];
}

/// A wider cut that the plan makes first, or holds as a cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstPassKind {
    /// A full slot into fresh stock.
    Slot,
    /// The first row of a raster.
    FirstRow,
    /// The widest width the planner holds at the full feed.
    PlannerCap,
}

/// A cleanup cut after the body of a level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleanupKind {
    /// The waterline sweep of steep walls after a 3D level.
    WaterlineSweep,
    /// The residue mop of a slice.
    ResidueMop,
    /// The contour-parallel residue pass.
    ContourResidue,
}

/// The kind of a cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutKind {
    /// The pass that the operator states.
    Body,
    /// A wider cut that the plan makes first or holds as a cap.
    FirstPass(FirstPassKind),
    /// A repeat at the final depth.
    SpringPass,
    /// An entry into material.
    Entry(EntryKind),
    /// A feed through air above an entry.
    EntryApproach,
    /// A cleanup cut.
    Cleanup(CleanupKind),
    /// A step that a planner cuts at a capped feed, with the rule that
    /// capped it.
    CappedStep(FeedCapSource),
    /// The first ring of a contour pocket level (P23d): a capped move with
    /// no feed-cap item.
    SlotRing,
    /// A lead into the cut.
    LeadIn,
    /// A lead out of the cut.
    LeadOut,
    /// A link between two cuts.
    Link,
    /// A drill cycle.
    Drill,
}

/// Why a cut has no engagement model. `CutLoad` then gives no number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoEngagementModel {
    /// An inclined entry along a line (P29-8).
    Ramp,
    /// An inclined entry along a helix (P29-8).
    Helix,
    /// A straight plunge of a ball, a bull nose, a V-bit or a tapered ball
    /// (`feeds::section::AxialSectionRefusal`).
    PlungeOfShapedTip,
    /// The depth into the stock is known only after the simulation.
    DepthAfterSimulation,
    /// The cut states no depth (P29-9).
    NoStatedDepth,
    /// The cut states no width (P29-9).
    NoStatedWidth,
}

/// How the tool engages the stock in one cut.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CutEngagement {
    /// A side cut.
    Side(Engagement),
    /// A straight plunge of a flat end mill: the axial circle of diameter
    /// `diameter_mm`, less the core `core_diameter_mm` (0 for a tool that
    /// cuts at its centre). Build it with [`Self::axial_plunge`].
    AxialPlunge {
        /// The cutting diameter (mm).
        diameter_mm: f64,
        /// The core diameter (mm); 0 for a centre-cutting tool.
        core_diameter_mm: f64,
    },
    /// No model gives the engagement of this cut.
    NoModel(NoEngagementModel),
}

impl CutEngagement {
    /// The engagement of a straight plunge of a tool of shape `tool`.
    /// `EngagedSection::axial_plunge` is the one rule that tells a flat
    /// end from a shaped tip: a flat end gives [`Self::AxialPlunge`],
    /// every other tip gives `NoModel(PlungeOfShapedTip)`.
    #[must_use]
    pub fn axial_plunge(tool: ToolGeometryHint, diameter_mm: f64, core_diameter_mm: f64) -> Self {
        match EngagedSection::axial_plunge(tool, diameter_mm, core_diameter_mm) {
            Ok(_) => Self::AxialPlunge {
                diameter_mm,
                core_diameter_mm,
            },
            Err(crate::feeds::section::AxialSectionRefusal::TipShapeNotModelled) => {
                Self::NoModel(NoEngagementModel::PlungeOfShapedTip)
            }
        }
    }
}

/// Which depth of a simulation sample a cut reads (open item M4: one
/// sample has two depths).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleDepth {
    /// `axial_doc_mm`, the depth of the layer the sample removes
    /// (`Engagement::from_sample`).
    RemovedLayer,
    /// `axial_engagement_mm`, the engaged edge length
    /// (`tool_load::deflection::sample_engagement`).
    EngagedEdge,
}

/// Where a cut comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutSource {
    /// A cut that the plan states before the simulation.
    Planned,
    /// A cut that the simulation measured.
    Measured {
        /// The index of the sample in the cut trace.
        sample_index: u32,
        /// The index of the move of the sample.
        move_index: u32,
        /// The depth of the sample that the cut reads.
        depth: SampleDepth,
    },
}

/// The radial stock that a cut leaves on the wall for a later pass. The
/// four cases of the deflection limit (`tool_load::deflection::
/// exceeds_limit`), each with its own clause.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Leave {
    /// A finish or semi-finish pass: no later pass, the finish target
    /// holds.
    FinishPass,
    /// A rough cut of an op with no stock-to-leave field.
    NoField,
    /// A rough cut whose leave is 0: the cut is the final wall.
    Zero,
    /// A rough cut that leaves this radial stock (mm), above 0.
    Mm(f64),
}

/// Why a cut has no depth limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoDepthLimit {
    /// A drill cycle: the descent is the cut.
    DrillCycle,
    /// A finish pass (ruling R2, 2026-09-23: deflection decides).
    FinishPass,
}

/// The depth limit of a cut: one of the two depth rules, or none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthLimit {
    /// A depth rule applies.
    Rule(DepthRule),
    /// No depth rule applies.
    NoLimit(NoDepthLimit),
}

/// The geometry facts of a cut that stand in the place of the labels of
/// the operation. In P29-3 each fact holds the value of today's label.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutFacts {
    /// The radial chip-thinning rule of the cut.
    pub thins_chip: RadialThinning,
    /// The corner-coverage rule of an offset ring applies.
    pub offset_ring: bool,
    /// The radial stock that the cut leaves for a later pass.
    pub leave_after_mm: Leave,
    /// The depth limit of the cut.
    pub depth_rule: DepthLimit,
}

impl CutFacts {
    /// The facts of a drill cycle, today's values: no thinning, not an
    /// offset ring, no stock-to-leave field, no depth limit. The straight
    /// plunge door of `feeds` reads it. Commit B moves the filling of
    /// facts into the cut list of each op.
    pub const DRILL_CYCLE: Self = Self {
        thins_chip: RadialThinning::DrillCycle,
        offset_ring: false,
        leave_after_mm: Leave::NoField,
        depth_rule: DepthLimit::NoLimit(NoDepthLimit::DrillCycle),
    };
}

/// One cut that a limit judges. See the module doc.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cut {
    /// The kind.
    pub kind: CutKind,
    /// The engagement.
    pub engagement: CutEngagement,
    /// The feed, the spindle speed, the chip and the flute count; `None`
    /// when the feed is not known yet.
    pub point: Option<OperatingPoint>,
    /// Where the cut comes from.
    pub source: CutSource,
    /// The geometry facts.
    pub facts: CutFacts,
}

impl Cut {
    /// The planned straight plunge of a drill cycle of a tool of shape
    /// `tool` at `point` ([`CutEngagement::axial_plunge`],
    /// [`CutFacts::DRILL_CYCLE`]).
    #[must_use]
    pub fn planned_drill(
        tool: ToolGeometryHint,
        diameter_mm: f64,
        core_diameter_mm: f64,
        point: OperatingPoint,
    ) -> Self {
        Self {
            kind: CutKind::Drill,
            engagement: CutEngagement::axial_plunge(tool, diameter_mm, core_diameter_mm),
            point: Some(point),
            source: CutSource::Planned,
            facts: CutFacts::DRILL_CYCLE,
        }
    }
}
