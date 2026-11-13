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
    /// The wall finish of a Bore: the allowance, up the whole wall
    /// (P29-7 7b).
    WallFinish,
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
    /// A cut that a dressup adds to the path: today only the dogbone
    /// relief at an inside corner (`dressup::apply_dogbones`, the one
    /// emitter of a `DressupArtifact` span). The `DressupArtifact` span
    /// marks it; its moves keep the cut intent.
    DressupRelief,
    /// A drill cycle.
    Drill,
}

/// The group that a gate reports a cut in (P29-6, ruling R4: "Entries and
/// cleanup are judged like any cut, as their own group"; operator Q1,
/// 2026-11-12: "I think all moves should be judged"). Every kind belongs
/// to exactly one group ([`CutKind::group`]). Each gate gives one verdict
/// per group in a fixed record ([`ByGroup`]), and the one function
/// [`super::scope`] states for each limit and each group whether the
/// limit judges it.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum CutGroup {
    /// The pass that the operator states, the wider first passes, the
    /// spring passes, the wall finish, the capped steps and the slot ring.
    Body,
    /// An entry into material, the feed through air above it, and the
    /// lead into the cut.
    Entry,
    /// A cleanup cut and a cut that a dressup adds (the dogbone relief).
    Cleanup,
    /// A link between two cuts and the lead out of the cut.
    Link,
    /// A drill cycle: an axial cut with no radial engagement. The side
    /// limits do not judge it (rows `P28-04`, `P28-10`, `P28-17`,
    /// `P28-20`); the drill gates do.
    Drill,
}

impl CutGroup {
    /// Every group, in declaration order.
    pub const ALL: [Self; 5] = [
        Self::Body,
        Self::Entry,
        Self::Cleanup,
        Self::Link,
        Self::Drill,
    ];

    /// The group in words, for a message: "body", "entry", "cleanup",
    /// "link", "drill".
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Body => "body",
            Self::Entry => "entry",
            Self::Cleanup => "cleanup",
            Self::Link => "link",
            Self::Drill => "drill",
        }
    }
}

impl CutKind {
    /// The kind in words, for the card and the envelope record ("slot
    /// ring", "planner cap"). One match names every kind.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Body => "body",
            Self::FirstPass(FirstPassKind::Slot) => "first slot",
            Self::FirstPass(FirstPassKind::FirstRow) => "first row",
            Self::FirstPass(FirstPassKind::PlannerCap) => "planner cap",
            Self::SpringPass => "spring pass",
            Self::WallFinish => "wall finish",
            Self::Entry(EntryKind::Plunge) => "plunge entry",
            Self::Entry(EntryKind::Ramp) => "ramp entry",
            Self::Entry(EntryKind::Helix) => "helix entry",
            Self::Entry(EntryKind::PeckLadder) => "peck ladder entry",
            Self::EntryApproach => "entry approach",
            Self::Cleanup(CleanupKind::WaterlineSweep) => "waterline sweep",
            Self::Cleanup(CleanupKind::ResidueMop) => "residue mop",
            Self::Cleanup(CleanupKind::ContourResidue) => "contour residue",
            Self::CappedStep(_) => "capped step",
            Self::SlotRing => "slot ring",
            Self::LeadIn => "lead in",
            Self::LeadOut => "lead out",
            Self::Link => "link",
            Self::DressupRelief => "dogbone relief",
            Self::Drill => "drill cycle",
        }
    }

    /// The group of this kind. One match names every kind and has no
    /// wildcard arm: a new kind does not compile until it states its
    /// group, so no kind can fall outside the gates.
    #[must_use]
    pub const fn group(self) -> CutGroup {
        match self {
            Self::Body
            | Self::FirstPass(_)
            | Self::SpringPass
            | Self::WallFinish
            | Self::CappedStep(_)
            | Self::SlotRing => CutGroup::Body,
            Self::Entry(_) | Self::EntryApproach | Self::LeadIn => CutGroup::Entry,
            Self::Cleanup(_) | Self::DressupRelief => CutGroup::Cleanup,
            Self::Link | Self::LeadOut => CutGroup::Link,
            Self::Drill => CutGroup::Drill,
        }
    }
}

/// One value for each [`CutGroup`] (P29-6 6d, lead ruling 1): the fixed
/// record of a gate result. The fields are private and the one
/// constructor [`Self::from_fn`] calls its closure once for each group, in
/// one exhaustive struct literal, so a record cannot leave a group out: a
/// new group does not compile until every record holds it.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ByGroup<T> {
    body: T,
    entry: T,
    cleanup: T,
    link: T,
    drill: T,
}

impl<T> ByGroup<T> {
    /// The record with `f(group)` for each group, called in the order of
    /// [`CutGroup::ALL`].
    pub fn from_fn(mut f: impl FnMut(CutGroup) -> T) -> Self {
        Self {
            body: f(CutGroup::Body),
            entry: f(CutGroup::Entry),
            cleanup: f(CutGroup::Cleanup),
            link: f(CutGroup::Link),
            drill: f(CutGroup::Drill),
        }
    }

    /// The value of `group`.
    #[must_use]
    pub const fn get(&self, group: CutGroup) -> &T {
        match group {
            CutGroup::Body => &self.body,
            CutGroup::Entry => &self.entry,
            CutGroup::Cleanup => &self.cleanup,
            CutGroup::Link => &self.link,
            CutGroup::Drill => &self.drill,
        }
    }

    /// The value of `group`, to change.
    pub fn get_mut(&mut self, group: CutGroup) -> &mut T {
        match group {
            CutGroup::Body => &mut self.body,
            CutGroup::Entry => &mut self.entry,
            CutGroup::Cleanup => &mut self.cleanup,
            CutGroup::Link => &mut self.link,
            CutGroup::Drill => &mut self.drill,
        }
    }

    /// Each group with its value, in the order of [`CutGroup::ALL`].
    pub fn iter(&self) -> impl Iterator<Item = (CutGroup, &T)> {
        CutGroup::ALL.into_iter().map(move |g| (g, self.get(g)))
    }

    /// Each group with its value, to change.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (CutGroup, &mut T)> {
        let Self {
            body,
            entry,
            cleanup,
            link,
            drill,
        } = self;
        [
            (CutGroup::Body, body),
            (CutGroup::Entry, entry),
            (CutGroup::Cleanup, cleanup),
            (CutGroup::Link, link),
            (CutGroup::Drill, drill),
        ]
        .into_iter()
    }
}

/// Why a cut has no engagement model. `CutLoad` then gives no number.
/// P29-8 8d: a ramp and a helix entry have a model (a slot at their own
/// depth), so no variant names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoEngagementModel {
    /// A straight plunge of a ball, a bull nose, a V-bit or a tapered ball
    /// (`feeds::section::AxialSectionRefusal`).
    PlungeOfShapedTip,
    /// The depth into the stock is known only after the simulation.
    DepthAfterSimulation,
    /// The cut states no depth.
    NoStatedDepth,
    /// The cut states no width.
    NoStatedWidth,
    /// P29-9 (ruling Q4): a peck is a drill cycle. It has no side-cut
    /// depth; the drill gates judge it.
    DrillCycle,
    /// P29-9: the depth is a depth of the tool (a chamfer depth is the cone
    /// of a V-bit), and this tool is not a V-bit.
    NotAVBit,
    /// A simulation sample that removes material and states no arc of
    /// engagement, or an arc that gives no width (a straight plunge
    /// sample; `Cut::of_sample`).
    NoMeasuredArc,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
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

/// The radial stock that a cut leaves on the wall for a later pass (P29-7
/// 7d: the op's radial stock-to-leave field only; the role is not read).
/// The deflection limit is `min(leave, 200 um)`
/// (`tool_load::deflection::exceeds_limit`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Leave {
    /// The op leaves this radial stock (mm), above 0.
    Mm(f64),
    /// The op states no radial stock to leave above 0: no field, a leave
    /// of 0, or a finish pass.
    None,
}

/// Why a cut has no depth limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoDepthLimit {
    /// A drill cycle: the descent is the cut.
    DrillCycle,
    /// A finish pass (ruling R2, 2026-09-23: deflection decides).
    FinishPass,
}

/// The material sides of a body cut (P29-5, ruling R3). A geometry fact:
/// a slot or a groove has material on both sides of the tool, every other
/// side cut has material on one side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodySides {
    /// Material on both sides: a slot or a groove.
    Both,
    /// Material on one side, or no side cut.
    One,
}

/// The depth limit of a cut: the machine force budget, or none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthLimit {
    /// The machine force budget sets the depth.
    ForceBudget,
    /// No depth limit applies.
    NoLimit(NoDepthLimit),
}

/// The geometry facts of a cut that stand in the place of the labels of
/// the operation. In P29-3 each fact holds the value of today's label.
///
/// P29-11 L-2 (S3): the fields are private. [`CutFacts::declared`] is the one
/// constructor, and only `compute/catalog/cuts.rs` (`OperationConfig::cuts`)
/// calls it (the P29-last guard scans for it); [`CutFacts::DRILL_CYCLE`] is
/// the facts of a drill cycle. A reader takes a fact through its getter, so
/// no code outside the declaration states a fact of a cut.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutFacts {
    /// The radial chip-thinning rule of the cut.
    thins_chip: RadialThinning,
    /// The corner-coverage rule of an offset ring applies.
    offset_ring: bool,
    /// The radial stock that the cut leaves for a later pass.
    leave_after_mm: Leave,
    /// The depth limit of the cut.
    depth_rule: DepthLimit,
}

impl CutFacts {
    /// THE constructor of the facts of a declared cut (P29-11 L-2, S3). Only
    /// `compute/catalog/cuts.rs` calls it.
    #[must_use]
    pub const fn declared(
        thins_chip: RadialThinning,
        offset_ring: bool,
        leave_after_mm: Leave,
        depth_limit: DepthLimit,
    ) -> Self {
        Self {
            thins_chip,
            offset_ring,
            leave_after_mm,
            depth_rule: depth_limit,
        }
    }

    /// The radial chip-thinning rule of the cut.
    #[must_use]
    pub const fn thins_chip(&self) -> RadialThinning {
        self.thins_chip
    }

    /// Whether the corner-coverage rule of an offset ring applies.
    #[must_use]
    pub const fn offset_ring(&self) -> bool {
        self.offset_ring
    }

    /// The radial stock that the cut leaves for a later pass.
    #[must_use]
    pub const fn leave_after_mm(&self) -> Leave {
        self.leave_after_mm
    }

    /// The depth limit of the cut.
    #[must_use]
    pub const fn depth_rule(&self) -> DepthLimit {
        self.depth_rule
    }

    /// The facts of a drill cycle, today's values: no thinning, not an
    /// offset ring, no stock-to-leave field, no depth limit. The straight
    /// plunge door of `feeds` reads it. Commit B moves the filling of
    /// facts into the cut list of each op.
    pub const DRILL_CYCLE: Self = Self {
        thins_chip: RadialThinning::DrillCycle,
        offset_ring: false,
        leave_after_mm: Leave::None,
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
