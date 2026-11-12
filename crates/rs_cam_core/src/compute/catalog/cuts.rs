//! Feeds contract P29-3 (commit B): the cuts that each op declares.
//!
//! [`OperationConfig::cuts`] is the one list of the planned cuts of an op:
//! the body pass, the wider cuts that the plan makes first or holds as a
//! cap, the slot ring, the spring passes, the row connectors, the cleanup
//! cuts, the capped steps, the wall finish of a Bore, the drill cycle and
//! the entry kinds. P29-7 7b: each cut that the generator makes is in the
//! list, with its feed rule ([`DeclaredFeed`]) and its presence
//! ([`Presence`]), with one exception: the capped steps, where each op
//! declares one `FeedCapSource` of the several that its planner can write.
//! P29-8 8d: an entry is a declared cut with an engagement, a feed and a
//! presence. A ramp or a helix is a slot at its own depth
//! ([`DeclaredDepth::Own`]); the planned limits judge it as a value check,
//! never as a ceiling of the body plane. The configured kind
//! ([`ConfiguredEntry`]) is `Always`; every other kind of the op's entry
//! contract is `MayOccur` (row `U-EntryFallback`). The re-plunge after a
//! tab of a Profile is the plunge entry cut (row `U-TabReplunge` holds the
//! open cause). A cut that the plan makes only where a generation fact
//! asks is [`Presence::MayOccur`]; it names a row of `limits::EXEMPTIONS`,
//! because no limit caps it before the simulation.
//!
//! `cuts()` is the one filler of the facts of a cut (`limits::CutFacts`).
//! In P29-3 each fact holds the value of today's label (rows `F-*` of the
//! exemption list); P29-4 to P29-7 replace each value with geometry.
//!
//! The readers: `feeds::planned` (P29-7 7c: every limit before the
//! simulation judges every declared cut, and the pre-sim power is the
//! largest power over them), [`CutList::full_feed_width`] (the width of
//! the Suggest lift), the depth rule of the envelope, Suggest and the feed
//! modulator, the offset-ring test of the envelope and Suggest, the
//! thinning rule of the session and Suggest doors, the slot ring of
//! `feeds::geometry::UnliftedFeed::of_slot_ring`, and (P29-5c) the body
//! width that the force-budget solve reads (`feeds::envelope` and
//! `feeds::force_budget::cap_cut_of_operation`).
//!
//! The sentry is `tests/every_op_declares_its_cuts_p29_3.rs`.

use crate::compute::tool_config::ToolConfig;
use crate::feeds::cut_load::OperatingPoint;
use crate::feeds::engagement::{EngagedDiameter, Engagement};
use crate::feeds::entry_rule::ConfiguredEntry;
use crate::feeds::envelope::CutPoint;
use crate::feeds::geometry::RadialThinning;
use crate::feeds::{CutBody, OperationFamily as FeedsOperationFamily, PassRole};
use crate::limits::{
    Cut, CutEngagement, CutFacts, CutKind, CutSource, EntryKind, FirstPassKind, Leave,
    NoEngagementModel,
};

use super::{Axial, FirstPass, OperationConfig, PlannerWidth, Radial};

/// The most cuts that one op declares (P29-7 7b: a geodesic 3D Rough
/// declares 10). A cut over the bound is not lost silently:
/// [`CutList::dropped`] names it, and the sentry holds every op fixture
/// with no dropped cut.
pub const MAX_DECLARED_CUTS: usize = 12;

/// The tool of a planned cut: what [`DeclaredCut::at`] reads to make a
/// [`Cut`]. `cuts()` reads no tool: the list of an op is the same for
/// every tool.
#[derive(Debug, Clone, Copy)]
pub struct CutContext<'a> {
    /// The tool config.
    pub tool: &'a ToolConfig,
    /// The cutting cap and the core of the built tool.
    pub body: CutBody,
}

impl<'a> CutContext<'a> {
    /// The context of `tool`, with the body of the built tool
    /// ([`CutBody::of_tool`]).
    #[must_use]
    pub fn of_tool(tool: &'a ToolConfig) -> Self {
        Self {
            tool,
            body: CutBody::of_tool(tool),
        }
    }
}

/// The width that a declared cut states.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeclaredWidth {
    /// A stated width (mm).
    Stated(f64),
    /// A full slot: the tool diameter. A groove declares it too (P29-5c):
    /// on a V-bit the engagement clamps the width at the cone at the depth.
    Slot,
    /// The pass ceiling `c D` of the stated stepover (`None`: not set).
    PassCeiling {
        /// The stated stepover (mm).
        stepover: Option<f64>,
    },
    /// `mult x stepover`, at most D (the unsourced `trochoid_cap_mult`).
    CapMult {
        /// The stated stepover (mm).
        stepover: Option<f64>,
        /// The multiplier.
        mult: f64,
    },
    /// The width that the scallop target `mm` gives on the tool.
    Scallop(f64),
    /// No width is stated.
    None(NoEngagementModel),
}

/// The depth that a declared cut states.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeclaredDepth {
    /// A stated depth (mm): a depth of the body plane.
    Stated(f64),
    /// P29-8 8d: the cut's own depth, not the body depth (an entry). The
    /// planned limits judge it as a value check, never as a ceiling of the
    /// body plane (lead ruling Q1).
    Own(OwnDepth),
    /// P29-8 8d: a straight descent (a plunge or a peck ladder entry): the
    /// axial circle of the tool at its own feed, with no side-cut depth.
    /// Only the spindle power reads it (`CutLoad`, the axial section); no
    /// side-cut limit has a model for it.
    Axial,
    /// No depth is stated, or the simulation gives it.
    None(NoEngagementModel),
}

/// P29-8 8d: the own depth of a cut that is not on the body plane (an
/// entry), and the rule that gives it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OwnDepth {
    /// The depth (mm).
    pub mm: f64,
    /// The rule that gives the depth.
    pub rule: OwnDepthRule,
}

/// The rule of an [`OwnDepth`]. One match names every rule
/// ([`Self::clause`]): a new rule does not compile until it states its
/// words for the card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnDepthRule {
    /// A ramp entry: the depth per pass. The ramp takes the full material
    /// depth (operator ruling 2026-09-24), so its last lap is a slot of that
    /// depth. The worst case (lead ruling Q1b): a lap ladder engages less.
    RampDepthPerPass,
    /// A helix entry: one pitch. One lap engages one pitch at the leading
    /// edge (the Bore rule of P29-5b).
    HelixPitch,
}

impl OwnDepthRule {
    /// The rule in words, for the card and MCP.
    #[must_use]
    pub const fn clause(self) -> &'static str {
        match self {
            Self::RampDepthPerPass => "the ramp's last lap at full depth (worst case)",
            Self::HelixPitch => "one helix pitch at the leading edge",
        }
    }
}

/// The feed at which the emitter cuts a declared cut (P29-7 7b).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclaredFeed {
    /// The op feed (the emitter writes `feed_rate`).
    OpFeed,
    /// The op feed over the lift of the body (P23d: the slot ring,
    /// `feeds::geometry::UnliftedFeed::of_slot_ring`).
    Unlifted,
    /// A feed the planner caps on its own stock read; no number before the
    /// simulation.
    PlannerCapped,
    /// The feed of the entry rule of this entry kind (P29-8 8d: the ramp
    /// feed of a ramp or a helix, the plunge feed of the door for a plunge
    /// or a peck ladder; `ConfiguredEntry::planned_feed_mm_min`).
    EntryRule(EntryKind),
    /// The feed of the drill cycle (Step 9c).
    DrillCycle,
}

impl DeclaredFeed {
    /// The feed rule of a cut of `kind`. One match names every kind and
    /// has no wildcard arm: a new kind does not compile until it states its
    /// feed.
    #[must_use]
    pub const fn of_kind(kind: CutKind) -> Self {
        match kind {
            CutKind::Body
            | CutKind::FirstPass(_)
            | CutKind::SpringPass
            | CutKind::Cleanup(_)
            | CutKind::WallFinish
            | CutKind::LeadIn
            | CutKind::LeadOut
            | CutKind::Link
            | CutKind::DressupRelief => Self::OpFeed,
            CutKind::SlotRing => Self::Unlifted,
            CutKind::CappedStep(_) => Self::PlannerCapped,
            CutKind::Entry(kind) => Self::EntryRule(kind),
            // The air lead runs at the plunge feed of the door.
            CutKind::EntryApproach => Self::EntryRule(EntryKind::Plunge),
            CutKind::Drill => Self::DrillCycle,
        }
    }
}

/// A declared cut that the plan cuts only where a generation fact asks.
/// No limit caps it before the simulation, so it is an exemption: `row`
/// is the id of its row of `limits::EXEMPTIONS` (lead ruling Q1 of
/// P29-7). The sentry `the_exemption_list_only_shrinks_p29_3` holds the
/// link, as for a `limits::NotJudged` cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MayOccur {
    /// The id of the row of `EXEMPTIONS`.
    pub row: &'static str,
    /// The generation fact that asks for the cut.
    pub reason: &'static str,
}

/// Whether the plan always cuts a declared cut (P29-7 7b).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    /// The plan cuts it wherever its rule applies (a spring pass at the
    /// final depth, a wall finish, a capped step where the planner caps):
    /// no generation fact can leave it out. The limits read it as a ceiling.
    Always,
    /// The plan cuts it only where a generation fact asks.
    MayOccur(MayOccur),
}

/// The generation fact of an entry kind that the op does not configure
/// (row `U-EntryFallback`). One match names every kind.
const fn entry_fallback_reason(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Plunge => {
            "a ramp with no run or a helix with no room falls back to a plunge; a Profile \
             re-plunges after each tab (row U-TabReplunge)"
        }
        EntryKind::Ramp => {
            "a helix with no room falls back to a ramp at the op's own angle; a non-centre-cutting \
             tool ramps under the style None; a Pencil ramps its crease on rest stock"
        }
        EntryKind::Helix => "the contract allows a helix, and the op states another entry kind",
        EntryKind::PeckLadder => {
            "the contract allows a peck ladder, and the op states another entry kind"
        }
    }
}

/// The generation fact of the residual sweep of a geodesic 3D Rough.
const GEODESIC_SWEEP: &str =
    "the residual full-slot sweep of a geodesic level where the grow refuses";

/// One cut that an op declares before the simulation: the stated numbers,
/// the feed rule, the presence and the facts. [`Self::at`] makes the
/// [`Cut`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeclaredCut {
    /// The kind.
    pub kind: CutKind,
    /// The width.
    pub width: DeclaredWidth,
    /// The depth.
    pub depth: DeclaredDepth,
    /// The feed rule ([`DeclaredFeed::of_kind`]).
    pub feed: DeclaredFeed,
    /// Whether the plan always cuts it.
    pub presence: Presence,
    /// The facts.
    pub facts: CutFacts,
}

impl DeclaredCut {
    /// The planned cut of this declaration on the tool of `ctx` at `point`
    /// (`None`: the feed is not known yet).
    ///
    /// The engagement:
    ///
    /// - [`CutKind::Drill`]: the straight plunge of the tool
    ///   ([`CutEngagement::axial_plunge`]).
    /// - A plunge or a peck ladder entry: the straight plunge of the tool.
    /// - A ramp or a helix entry (P29-8 8d), and every other kind: a side
    ///   cut at the stated width and depth (an entry: the slot at its own
    ///   depth), with no helix (as the calculator's power door), or no model
    ///   when one of the two is not stated. A scallop width is not stated
    ///   here.
    #[must_use]
    pub fn at(&self, ctx: &CutContext<'_>, point: Option<OperatingPoint>) -> Cut {
        Cut {
            kind: self.kind,
            engagement: self.engagement(ctx),
            point,
            source: CutSource::Planned,
            facts: self.facts,
        }
    }

    fn engagement(&self, ctx: &CutContext<'_>) -> CutEngagement {
        let hint = ctx.tool.geometry_hint();
        let d = ctx.tool.diameter;
        let side = || -> CutEngagement {
            let ae = match self.width_mm(d) {
                Ok(ae) => ae,
                Err(reason) => return CutEngagement::NoModel(reason),
            };
            let ap = match self.depth {
                DeclaredDepth::Stated(ap) => ap,
                DeclaredDepth::Own(own) => own.mm,
                // Not reached: an axial cut is the straight plunge above.
                DeclaredDepth::Axial => {
                    return CutEngagement::NoModel(NoEngagementModel::NoStatedDepth);
                }
                DeclaredDepth::None(reason) => return CutEngagement::NoModel(reason),
            };
            CutEngagement::Side(Engagement::from_cut(
                CutPoint {
                    ap_mm: ap,
                    ae_mm: ae,
                },
                hint,
                EngagedDiameter::of_hint(hint, d, ctx.body.max_cut_diameter_mm, ap),
                None,
            ))
        };
        match self.kind {
            CutKind::Drill | CutKind::Entry(EntryKind::Plunge | EntryKind::PeckLadder) => {
                CutEngagement::axial_plunge(hint, d, ctx.body.core_diameter_mm)
            }
            CutKind::Entry(EntryKind::Ramp | EntryKind::Helix)
            | CutKind::Body
            | CutKind::FirstPass(_)
            | CutKind::SpringPass
            | CutKind::WallFinish
            | CutKind::EntryApproach
            | CutKind::Cleanup(_)
            | CutKind::CappedStep(_)
            | CutKind::SlotRing
            | CutKind::LeadIn
            | CutKind::LeadOut
            | CutKind::Link
            | CutKind::DressupRelief => side(),
        }
    }

    /// The width (mm) on a tool of diameter `d`, or why none exists.
    fn width_mm(&self, d: f64) -> Result<f64, NoEngagementModel> {
        let not_stated = NoEngagementModel::NoStatedWidth;
        match self.width {
            DeclaredWidth::Stated(ae) => Ok(ae),
            DeclaredWidth::Slot => Ok(d),
            DeclaredWidth::PassCeiling { stepover } => stepover
                .map(|ae| PlannerWidth::PassCeiling.full_feed_width_mm(ae, d))
                .ok_or(not_stated),
            DeclaredWidth::CapMult { stepover, mult } => stepover
                .and_then(|ae| PlannerWidth::CapMult(mult).power_width_mm(ae, d))
                .ok_or(not_stated),
            // The scallop width needs the tool's ball radius, a rule of the
            // envelope (`axes_plan`); no reader asks for it yet (P29-9).
            DeclaredWidth::Scallop(_) => Err(not_stated),
            DeclaredWidth::None(reason) => Err(reason),
        }
    }
}

/// The cuts of one op, in the declaration order: the body, the planner
/// cap, the first slot, the slot ring, the other cuts of the op, the drill
/// cycle, then the entry kinds in the order of `EntryKind::ALL`. A fixed
/// array: no heap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutList {
    cuts: [Option<DeclaredCut>; MAX_DECLARED_CUTS],
    /// The kind of the first cut that the full list could not hold.
    dropped: Option<CutKind>,
}

/// The list holds [`MAX_DECLARED_CUTS`] cuts and has no free place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CutListFull;

impl CutList {
    const EMPTY: Self = Self {
        cuts: [None; MAX_DECLARED_CUTS],
        dropped: None,
    };

    /// Adds `cut` in the first free place, or `Err(CutListFull)` when the
    /// list holds [`MAX_DECLARED_CUTS`] cuts.
    fn push(&mut self, cut: DeclaredCut) -> Result<(), CutListFull> {
        let free = self
            .cuts
            .iter_mut()
            .find(|c| c.is_none())
            .ok_or(CutListFull)?;
        *free = Some(cut);
        Ok(())
    }

    /// The kind of the first cut that the list could not hold, or `None`
    /// when it holds every cut of the op. The sentry
    /// `every_op_declares_its_cuts_p29_3` holds `None` for every op fixture.
    #[must_use]
    pub fn dropped(&self) -> Option<CutKind> {
        self.dropped
    }

    /// The declared cuts, in order.
    pub fn iter(&self) -> impl Iterator<Item = &DeclaredCut> {
        self.cuts.iter().flatten()
    }

    /// The count of declared cuts.
    #[must_use]
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// True when the list declares no cut. Not reached: every op declares
    /// its body.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The first cut of `kind`, if the op declares one.
    #[must_use]
    pub fn of_kind(&self, kind: CutKind) -> Option<&DeclaredCut> {
        self.iter().find(|c| c.kind == kind)
    }

    /// True when the op declares a cut of `kind`.
    #[must_use]
    pub fn declares(&self, kind: CutKind) -> bool {
        self.of_kind(kind).is_some()
    }

    /// P23a: the widest width (mm) that the planner holds at the full feed,
    /// at the stated stepover `ae` on a tool of diameter `d`: the width of
    /// the planner-cap cut ([`PlannerWidth::full_feed_width_mm`]), else
    /// `ae`. Every reader of the Suggest lift takes its width here: Step 5,
    /// the unlifted feed, the power ladder's width rung, pass 9, the
    /// efficiency figure and the load context of the gates.
    #[must_use]
    pub fn full_feed_width(&self, ae: f64, d: f64) -> f64 {
        self.planner_width().full_feed_width_mm(ae, d)
    }

    /// The planner width rule of the planner-cap cut, or
    /// [`PlannerWidth::None`] when the op declares none.
    #[must_use]
    pub fn planner_width(&self) -> PlannerWidth {
        match self.of_kind(CutKind::FirstPass(FirstPassKind::PlannerCap)) {
            Some(cap) => match cap.width {
                DeclaredWidth::PassCeiling { .. } => PlannerWidth::PassCeiling,
                DeclaredWidth::CapMult { mult, .. } => PlannerWidth::CapMult(mult),
                DeclaredWidth::Stated(_)
                | DeclaredWidth::Slot
                | DeclaredWidth::Scallop(_)
                | DeclaredWidth::None(_) => PlannerWidth::None,
            },
            None => PlannerWidth::None,
        }
    }

    /// The facts of the body pass. Every op declares its body first, so
    /// this is the facts of the first cut.
    #[must_use]
    pub fn body_facts(&self) -> CutFacts {
        match self.iter().next() {
            Some(first) => first.facts,
            // Not reached: `cuts()` declares the body of every op first.
            None => CutFacts::DRILL_CYCLE,
        }
    }
}

/// How [`OperationConfig::cuts`] declares the first slot of an op.
#[derive(Debug, Clone, Copy)]
enum FirstSlot {
    /// From the cut spec: a first pass that is a slot, or a slot cut.
    OfSpec,
    /// The slot ring of a contour Pocket, in the place of the first slot.
    Ring,
    /// A full slot that the plan cuts only where a generation fact asks.
    MayOccur(MayOccur),
}

/// The builder of one op's list. Every cut of the op takes the op's facts
/// and the feed rule of its kind.
struct Declare {
    list: CutList,
    facts: CutFacts,
}

impl Declare {
    fn cut(
        &mut self,
        kind: CutKind,
        width: DeclaredWidth,
        depth: DeclaredDepth,
        presence: Presence,
    ) {
        let cut = DeclaredCut {
            kind,
            width,
            depth,
            feed: DeclaredFeed::of_kind(kind),
            presence,
            facts: self.facts,
        };
        // The named fallback of a full list: the first cut that it could
        // not hold is kept by kind (`CutList::dropped`), not lost.
        if let Err(CutListFull) = self.list.push(cut) {
            self.list.dropped = self.list.dropped.or(Some(kind));
        }
    }

    /// A cut with no stated width and no stated depth.
    fn unstated(&mut self, kind: CutKind) {
        self.cut(
            kind,
            DeclaredWidth::None(NoEngagementModel::NoStatedWidth),
            DeclaredDepth::None(NoEngagementModel::NoStatedDepth),
            Presence::Always,
        );
    }
}

impl OperationConfig {
    /// P29-3: the cuts that this op declares before the simulation. One
    /// match names every op and has no wildcard arm: a new op does not
    /// compile until it states its cuts. Each arm states what is special
    /// to the op; the body, the planner cap and the first slot come from
    /// the op's cut spec and planner width (K1, P23a). P29-7 7b: each arm
    /// also states the spring passes, the row connectors, the cleanup cuts,
    /// the capped steps and the wall finish that its generator makes.
    ///
    /// P29-8 8d (lead ruling Q5): `entry` is the op's configured entry
    /// ([`ConfiguredEntry`]: from the toolpath's dressups, or the registry
    /// default for a reader that holds no toolpath). It sets which entry
    /// kind is `Always` and the depth of a helix entry.
    #[must_use]
    pub fn cuts(&self, entry: &ConfiguredEntry) -> CutList {
        use super::super::operation_configs::{ClearingStrategy, PocketPattern};
        use crate::adaptive::CleanupStrategy;
        use crate::adaptive3d::FeedCapSource;
        use crate::limits::CleanupKind;
        let mut declare = Declare {
            list: CutList::EMPTY,
            facts: self.label_facts(),
        };
        match self {
            // The descent is the cut. The drill door of `feeds` reads the
            // same facts (`CutFacts::DRILL_CYCLE`).
            OperationConfig::Drill(_) | OperationConfig::AlignmentPinDrill(_) => {
                declare.facts = CutFacts::DRILL_CYCLE;
                self.declare_side_cuts(&mut declare, FirstSlot::OfSpec);
                declare.unstated(CutKind::Drill);
            }
            OperationConfig::Pocket(c) => {
                match c.pattern {
                    // P23d: the first ring of each level of a contour Pocket
                    // is a full slot (`UnliftedFeed::of_slot_ring`). The
                    // ring IS the first slot: no second first-slot cut.
                    PocketPattern::Contour => self.declare_side_cuts(&mut declare, FirstSlot::Ring),
                    PocketPattern::Zigzag => {
                        self.declare_side_cuts(&mut declare, FirstSlot::OfSpec);
                        self.declare_row_connector(&mut declare);
                    }
                }
                if c.finishing_passes > 0 {
                    declare.unstated(CutKind::SpringPass);
                }
            }
            OperationConfig::Profile(c) => {
                self.declare_side_cuts(&mut declare, FirstSlot::OfSpec);
                if c.finishing_passes > 0 {
                    declare.unstated(CutKind::SpringPass);
                }
            }
            OperationConfig::Face(_) | OperationConfig::Zigzag(_) => {
                self.declare_side_cuts(&mut declare, FirstSlot::OfSpec);
                self.declare_row_connector(&mut declare);
            }
            OperationConfig::Adaptive(c) => {
                self.declare_side_cuts(&mut declare, FirstSlot::OfSpec);
                match c.cleanup_strategy {
                    CleanupStrategy::Legacy => {}
                    CleanupStrategy::ResidueMop | CleanupStrategy::ContourParallelNarrow => {
                        declare.unstated(CutKind::Cleanup(CleanupKind::ResidueMop));
                    }
                    CleanupStrategy::ContourParallelHybrid => {
                        declare.unstated(CutKind::Cleanup(CleanupKind::ContourResidue));
                    }
                }
                declare.unstated(CutKind::CappedStep(FeedCapSource::DepartureWindow));
            }
            OperationConfig::Adaptive3d(c) => {
                match c.clearing_strategy {
                    ClearingStrategy::ContourParallel => {
                        self.declare_side_cuts(&mut declare, FirstSlot::OfSpec);
                        declare.unstated(CutKind::Cleanup(CleanupKind::WaterlineSweep));
                    }
                    // P29-5b option A: the sweep runs only where the grow
                    // refuses, a generation fact. Lead ruling Q1 of P29-7:
                    // the cut is an exemption, so it names its row.
                    ClearingStrategy::GeodesicAdaptive => {
                        self.declare_side_cuts(
                            &mut declare,
                            // EXEMPT(U-PerimeterSweep)
                            FirstSlot::MayOccur(MayOccur {
                                row: "U-PerimeterSweep",
                                reason: GEODESIC_SWEEP,
                            }),
                        );
                        declare.unstated(CutKind::Cleanup(CleanupKind::WaterlineSweep));
                        declare.unstated(CutKind::Cleanup(CleanupKind::ResidueMop));
                    }
                }
                declare.unstated(CutKind::CappedStep(FeedCapSource::StockRead3d));
            }
            // The finish pass of a Bore: out to the bore radius, one turn on
            // the floor, a helix up the wall (`ops::bore::bore_hole`). Its
            // width is the allowance; it states no side-cut depth.
            OperationConfig::Bore(c) => {
                self.declare_side_cuts(&mut declare, FirstSlot::OfSpec);
                if c.finish_allowance.is_finite() && c.finish_allowance > 0.0 {
                    declare.cut(
                        CutKind::WallFinish,
                        DeclaredWidth::Stated(c.finish_allowance),
                        DeclaredDepth::None(NoEngagementModel::NoStatedDepth),
                        Presence::Always,
                    );
                }
            }
            OperationConfig::VCarve(_)
            | OperationConfig::Inlay(_)
            | OperationConfig::InlayPlug(_)
            | OperationConfig::Trace(_)
            | OperationConfig::Chamfer(_)
            | OperationConfig::DropCutter(_)
            | OperationConfig::Waterline(_)
            | OperationConfig::Pencil(_)
            | OperationConfig::Scallop(_)
            | OperationConfig::UnifiedFinish(_)
            | OperationConfig::SteepShallow(_)
            | OperationConfig::RampFinish(_)
            | OperationConfig::SpiralFinish(_)
            | OperationConfig::RadialFinish(_)
            | OperationConfig::HorizontalFinish(_)
            | OperationConfig::ProjectCurve(_) => {
                self.declare_side_cuts(&mut declare, FirstSlot::OfSpec);
            }
        }
        self.declare_entries(&mut declare, entry);
        declare.list
    }

    /// P29-8 8d: the entry cuts, one per kind of the op's entry contract
    /// (`EntryContract::record_kinds`), in the order of `EntryKind::ALL`.
    ///
    /// | Kind | Width | Depth | Engagement |
    /// |---|---|---|---|
    /// | ramp | slot | own: the depth per pass ([`OwnDepthRule::RampDepthPerPass`]) | a slot at a slope |
    /// | helix | slot | own: one pitch ([`OwnDepthRule::HelixPitch`]) | a slot of depth `pitch` |
    /// | plunge, peck ladder | slot (the circle) | axial ([`DeclaredDepth::Axial`]) | the straight plunge |
    ///
    /// The configured kind is `Always` (the style None of a dressup op and
    /// a planner op with no style enter by the plunge); every other kind is
    /// `MayOccur` with the row `U-EntryFallback`. The plunge cut of a
    /// Profile holds the re-plunge after each tab (row `U-TabReplunge`).
    fn declare_entries(&self, declare: &mut Declare, entry: &ConfiguredEntry) {
        use super::EntryContract;
        let contract = self.op_type().registry_entry().entry;
        let always = match contract {
            EntryContract::Dressup { .. } | EntryContract::Planner { .. } => {
                Some(entry.kind().unwrap_or(EntryKind::Plunge))
            }
            EntryContract::StraightPlunge { .. } => Some(EntryKind::Plunge),
            EntryContract::DescentIsCut => None,
        };
        let body_depth = self.declared_depth();
        let no_width = DeclaredWidth::None(NoEngagementModel::NoStatedWidth);
        let no_depth = DeclaredDepth::None(NoEngagementModel::NoStatedDepth);
        // A slot at its own depth; with no own depth the entry has no
        // engagement, so it states no width either (a cut on the body plane
        // with no depth would read the body depth).
        let slot_at = |depth: DeclaredDepth| match depth {
            DeclaredDepth::Own(_) => (DeclaredWidth::Slot, depth),
            DeclaredDepth::Stated(_) | DeclaredDepth::Axial | DeclaredDepth::None(_) => {
                (no_width, no_depth)
            }
        };
        for kind in contract.record_kinds().kinds() {
            let (width, depth) = match kind {
                EntryKind::Ramp => slot_at(match body_depth {
                    DeclaredDepth::Stated(mm) => DeclaredDepth::Own(OwnDepth {
                        mm,
                        rule: OwnDepthRule::RampDepthPerPass,
                    }),
                    // The body depth is never an own depth.
                    DeclaredDepth::Own(_) | DeclaredDepth::Axial | DeclaredDepth::None(_) => {
                        no_depth
                    }
                }),
                EntryKind::Helix => {
                    let pitch = entry.helix_pitch_mm();
                    slot_at(if pitch.is_finite() && pitch > 0.0 {
                        DeclaredDepth::Own(OwnDepth {
                            mm: pitch,
                            rule: OwnDepthRule::HelixPitch,
                        })
                    } else {
                        no_depth
                    })
                }
                // The full circle of the tool, descending.
                EntryKind::Plunge | EntryKind::PeckLadder => {
                    (DeclaredWidth::Slot, DeclaredDepth::Axial)
                }
            };
            let presence = if always == Some(kind) {
                Presence::Always
            } else {
                // EXEMPT(U-EntryFallback)
                Presence::MayOccur(MayOccur {
                    row: "U-EntryFallback",
                    reason: entry_fallback_reason(kind),
                })
            };
            declare.cut(CutKind::Entry(kind), width, depth, presence);
        }
    }

    /// The body, the planner cap and the first slot of the op, in the tie
    /// order of the power cases (`feeds::planned::worst_power`: a later case wins a
    /// tie). `first_slot` says how the first slot is declared.
    fn declare_side_cuts(&self, declare: &mut Declare, first_slot: FirstSlot) {
        let spec = self.cut_spec();
        let depth = self.declared_depth();
        // P29-5c: a groove engages its whole cone on both sides, so its body
        // width is the slot width. The engagement clamps the width at the
        // cone at the depth (`EngagedDiameter::of_hint`).
        let body_width = match spec.radial {
            Radial::Width(Some(s)) => DeclaredWidth::Stated(s.mm()),
            Radial::Slot | Radial::Groove => DeclaredWidth::Slot,
            Radial::Scallop(s) => DeclaredWidth::Scallop(s.mm()),
            Radial::Width(None)
            | Radial::AngularStep(_)
            | Radial::SurfaceFollow
            | Radial::Axial => DeclaredWidth::None(NoEngagementModel::NoStatedWidth),
        };
        declare.cut(CutKind::Body, body_width, depth, Presence::Always);
        let stepover = self.stated_stepover();
        match self.planner_width() {
            PlannerWidth::None => {}
            PlannerWidth::PassCeiling => declare.cut(
                CutKind::FirstPass(FirstPassKind::PlannerCap),
                DeclaredWidth::PassCeiling { stepover },
                depth,
                Presence::Always,
            ),
            PlannerWidth::CapMult(mult) => declare.cut(
                CutKind::FirstPass(FirstPassKind::PlannerCap),
                DeclaredWidth::CapMult { stepover, mult },
                depth,
                Presence::Always,
            ),
        }
        match first_slot {
            FirstSlot::OfSpec => {
                // K7: a first pass that meets material on both sides, or a
                // radial cut that is a slot.
                if matches!(spec.first_pass, FirstPass::Slot) || matches!(spec.radial, Radial::Slot)
                {
                    declare.cut(
                        CutKind::FirstPass(FirstPassKind::Slot),
                        DeclaredWidth::Slot,
                        depth,
                        Presence::Always,
                    );
                }
            }
            FirstSlot::Ring => declare.cut(
                CutKind::SlotRing,
                DeclaredWidth::Slot,
                depth,
                Presence::Always,
            ),
            FirstSlot::MayOccur(may) => declare.cut(
                CutKind::FirstPass(FirstPassKind::Slot),
                DeclaredWidth::Slot,
                depth,
                Presence::MayOccur(may),
            ),
        }
    }

    /// The row connectors of a raster (Face, Zigzag, a zigzag Pocket): a
    /// fed move along the boundary, one stepover long, at the depth of the
    /// level.
    fn declare_row_connector(&self, declare: &mut Declare) {
        let width = match self.stated_stepover() {
            Some(s) => DeclaredWidth::Stated(s),
            None => DeclaredWidth::None(NoEngagementModel::NoStatedWidth),
        };
        declare.cut(
            CutKind::Link,
            width,
            self.declared_depth(),
            Presence::Always,
        );
    }

    /// The stated stepover of a stepover cut, else `None`.
    fn stated_stepover(&self) -> Option<f64> {
        match self.cut_spec().radial {
            Radial::Width(s) => s.map(|s| s.mm()),
            Radial::Slot
            | Radial::Groove
            | Radial::Scallop(_)
            | Radial::AngularStep(_)
            | Radial::SurfaceFollow
            | Radial::Axial => None,
        }
    }

    /// The depth of the op's side cuts: a depth per pass, a Z step, a
    /// one-pass depth, the surface cap or a helix pitch (one helix turn,
    /// P29-5b). A peck and a chamfer width are not a side-cut depth; a
    /// surface offset gives its depth after the simulation.
    fn declared_depth(&self) -> DeclaredDepth {
        let not_stated = DeclaredDepth::None(NoEngagementModel::NoStatedDepth);
        match self.cut_spec().axial {
            Axial::PerPass(s) | Axial::ZStep(s) | Axial::OnePass(s) | Axial::Surface { max: s } => {
                s.map_or(not_stated, |s| DeclaredDepth::Stated(s.mm()))
            }
            Axial::HelixPitch(s) => DeclaredDepth::Stated(s.mm()),
            Axial::Peck(_) | Axial::ChamferWidth(_) => not_stated,
            Axial::SurfaceOffset(_) => DeclaredDepth::None(NoEngagementModel::DepthAfterSimulation),
        }
    }

    /// The facts of the op's cuts: today's label values, filled here only
    /// (rows `F-*` of `limits::EXEMPTIONS`).
    fn label_facts(&self) -> CutFacts {
        let (family, role) = self.feeds_style();
        CutFacts {
            // EXEMPT(F-ThinsChip)
            thins_chip: RadialThinning::of(family, role),
            // EXEMPT(F-OffsetRing)
            offset_ring: family == FeedsOperationFamily::Pocket
                && role == PassRole::Roughing
                && matches!(self.cut_spec().radial, Radial::Width(_)),
            // P29-7 7d (ruling R5): the radial leave field only.
            leave_after_mm: match self.radial_stock_to_leave_mm() {
                Some(mm) if mm.is_finite() && mm > 0.0 => Leave::Mm(mm),
                Some(_) | None => Leave::None,
            },
            depth_rule: self.op_type().body_depth_limit(),
        }
    }
}
