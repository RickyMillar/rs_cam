//! Feeds contract P29-3 (commit B): the cuts that each op declares.
//!
//! [`OperationConfig::cuts`] is the one list of the planned cuts of an op:
//! the body pass, the wider cuts that the plan makes first or holds as a
//! cap, the slot ring, the drill cycle and the entry kinds. It holds
//! today's cuts and today's scope only: a cut is in the list when a reader
//! before the simulation reads it today (`P29-3_DESIGN.md` section 2.1).
//! A cut that the generator makes and no reader reads is a row `U-*` of
//! `limits::EXEMPTIONS`.
//!
//! `cuts()` is the one filler of the facts of a cut (`limits::CutFacts`).
//! In P29-3 each fact holds the value of today's label (rows `F-*` of the
//! exemption list); P29-4 to P29-7 replace each value with geometry.
//!
//! The readers: `feeds::engagement::PowerCases` (the slot case and the
//! planner width), the depth rule of the envelope, Suggest and the feed
//! modulator, the offset-ring test of the envelope and Suggest, the
//! thinning rule of the session and Suggest doors, and the slot ring of
//! `feeds::geometry::UnliftedFeed::of_slot_ring`.
//!
//! The sentry is `tests/every_op_declares_its_cuts_p29_3.rs`.

use crate::compute::tool_config::ToolConfig;
use crate::feeds::cut_load::OperatingPoint;
use crate::feeds::engagement::{EngagedDiameter, Engagement};
use crate::feeds::envelope::CutPoint;
use crate::feeds::geometry::RadialThinning;
use crate::feeds::{CutBody, OperationFamily as FeedsOperationFamily, PassRole};
use crate::limits::{
    Cut, CutEngagement, CutFacts, CutKind, CutSource, EntryKind, FirstPassKind, Leave,
    NoEngagementModel,
};
use crate::machine::RigidityProfile;

use super::{Axial, FirstPass, OperationConfig, PlannerWidth, Radial};

/// The most cuts that one op declares. The sentry holds every op below it.
pub const MAX_DECLARED_CUTS: usize = 8;

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
    /// A full slot: the tool diameter.
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
    /// A stated depth (mm).
    Stated(f64),
    /// No depth is stated, or the simulation gives it.
    None(NoEngagementModel),
}

/// One cut that an op declares before the simulation: the stated numbers
/// and the facts. [`Self::at`] makes the [`Cut`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeclaredCut {
    /// The kind.
    pub kind: CutKind,
    /// The width.
    pub width: DeclaredWidth,
    /// The depth.
    pub depth: DeclaredDepth,
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
    /// - An entry: no model (a ramp, a helix; a plunge or a peck states no
    ///   depth; P29-8).
    /// - Every other kind: a side cut at the stated width and depth, with no
    ///   helix (as the calculator's power door), or no model when one of the
    ///   two is not stated. A scallop width is not stated here.
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
            CutKind::Drill => CutEngagement::axial_plunge(hint, d, ctx.body.core_diameter_mm),
            CutKind::Entry(EntryKind::Ramp) => CutEngagement::NoModel(NoEngagementModel::Ramp),
            CutKind::Entry(EntryKind::Helix) => CutEngagement::NoModel(NoEngagementModel::Helix),
            CutKind::Entry(EntryKind::Plunge | EntryKind::PeckLadder) => {
                CutEngagement::NoModel(NoEngagementModel::NoStatedDepth)
            }
            CutKind::Body
            | CutKind::FirstPass(_)
            | CutKind::SpringPass
            | CutKind::EntryApproach
            | CutKind::Cleanup(_)
            | CutKind::CappedStep(_)
            | CutKind::SlotRing
            | CutKind::LeadIn
            | CutKind::LeadOut
            | CutKind::Link => side(),
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
/// cap, the first slot, the slot ring, the drill cycle, then the entry
/// kinds in the order of `EntryKind::ALL`. A fixed array: no heap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutList {
    cuts: [Option<DeclaredCut>; MAX_DECLARED_CUTS],
}

impl CutList {
    const EMPTY: Self = Self {
        cuts: [None; MAX_DECLARED_CUTS],
    };

    /// Adds `cut` in the first free place. The list holds at most
    /// [`MAX_DECLARED_CUTS`]; the sentry holds every op below it, so no
    /// cut is lost.
    fn push(&mut self, cut: DeclaredCut) {
        if let Some(free) = self.cuts.iter_mut().find(|c| c.is_none()) {
            *free = Some(cut);
        }
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

/// The builder of one op's list. Every cut of the op takes the op's facts.
struct Declare {
    list: CutList,
    facts: CutFacts,
}

impl Declare {
    fn cut(&mut self, kind: CutKind, width: DeclaredWidth, depth: DeclaredDepth) {
        self.list.push(DeclaredCut {
            kind,
            width,
            depth,
            facts: self.facts,
        });
    }
}

impl OperationConfig {
    /// P29-3: the cuts that this op declares before the simulation. One
    /// match names every op and has no wildcard arm: a new op does not
    /// compile until it states its cuts. Each arm states what is special
    /// to the op; the body, the planner cap and the first slot come from
    /// the op's cut spec and planner width (K1, P23a).
    #[must_use]
    pub fn cuts(&self) -> CutList {
        let mut declare = Declare {
            list: CutList::EMPTY,
            facts: self.label_facts(),
        };
        match self {
            // The descent is the cut. The drill door of `feeds` reads the
            // same facts (`CutFacts::DRILL_CYCLE`).
            OperationConfig::Drill(_) | OperationConfig::AlignmentPinDrill(_) => {
                declare.facts = CutFacts::DRILL_CYCLE;
                self.declare_side_cuts(&mut declare);
                declare.cut(
                    CutKind::Drill,
                    DeclaredWidth::None(NoEngagementModel::NoStatedWidth),
                    DeclaredDepth::None(NoEngagementModel::NoStatedDepth),
                );
            }
            // P23d: the first ring of each level of a contour Pocket is a
            // full slot (`UnliftedFeed::of_slot_ring`).
            OperationConfig::Pocket(c) => {
                self.declare_side_cuts(&mut declare);
                match c.pattern {
                    super::super::operation_configs::PocketPattern::Contour => declare.cut(
                        CutKind::SlotRing,
                        DeclaredWidth::Slot,
                        self.declared_depth(),
                    ),
                    super::super::operation_configs::PocketPattern::Zigzag => {}
                }
            }
            OperationConfig::Face(_)
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
            | OperationConfig::ProjectCurve(_) => self.declare_side_cuts(&mut declare),
        }
        for kind in self.op_type().registry_entry().entry.record_kinds().kinds() {
            declare.cut(
                CutKind::Entry(kind),
                DeclaredWidth::None(NoEngagementModel::NoStatedWidth),
                DeclaredDepth::None(NoEngagementModel::NoStatedDepth),
            );
        }
        declare.list
    }

    /// The body, the planner cap and the first slot of the op, in the tie
    /// order of the power cases (`PowerCases::worst`: a later case wins a
    /// tie).
    fn declare_side_cuts(&self, declare: &mut Declare) {
        let spec = self.cut_spec();
        let depth = self.declared_depth();
        let body_width = match spec.radial {
            Radial::Width(Some(s)) => DeclaredWidth::Stated(s.mm()),
            Radial::Slot => DeclaredWidth::Slot,
            Radial::Scallop(s) => DeclaredWidth::Scallop(s.mm()),
            Radial::Width(None)
            | Radial::Groove
            | Radial::AngularStep(_)
            | Radial::SurfaceFollow
            | Radial::Axial => DeclaredWidth::None(NoEngagementModel::NoStatedWidth),
        };
        declare.cut(CutKind::Body, body_width, depth);
        let stepover = match spec.radial {
            Radial::Width(s) => s.map(|s| s.mm()),
            Radial::Slot
            | Radial::Groove
            | Radial::Scallop(_)
            | Radial::AngularStep(_)
            | Radial::SurfaceFollow
            | Radial::Axial => None,
        };
        match self.planner_width() {
            PlannerWidth::None => {}
            PlannerWidth::PassCeiling => declare.cut(
                CutKind::FirstPass(FirstPassKind::PlannerCap),
                DeclaredWidth::PassCeiling { stepover },
                depth,
            ),
            PlannerWidth::CapMult(mult) => declare.cut(
                CutKind::FirstPass(FirstPassKind::PlannerCap),
                DeclaredWidth::CapMult { stepover, mult },
                depth,
            ),
        }
        // K7: a slot case for a first pass that meets material on both
        // sides, or a radial cut that is a slot.
        // EXEMPT(M12)
        if matches!(spec.first_pass, FirstPass::Slot) || matches!(spec.radial, Radial::Slot) {
            declare.cut(
                CutKind::FirstPass(FirstPassKind::Slot),
                DeclaredWidth::Slot,
                depth,
            );
        }
    }

    /// The depth of the op's side cuts: a depth per pass, a Z step, a
    /// one-pass depth or the surface cap. A peck, a helix pitch and a
    /// chamfer width are not a side-cut depth; a surface offset gives its
    /// depth after the simulation.
    fn declared_depth(&self) -> DeclaredDepth {
        let not_stated = DeclaredDepth::None(NoEngagementModel::NoStatedDepth);
        match self.cut_spec().axial {
            Axial::PerPass(s) | Axial::ZStep(s) | Axial::OnePass(s) | Axial::Surface { max: s } => {
                s.map_or(not_stated, |s| DeclaredDepth::Stated(s.mm()))
            }
            Axial::Peck(_) | Axial::HelixPitch(_) | Axial::ChamferWidth(_) => not_stated,
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
            // EXEMPT(F-LeaveAfter)
            leave_after_mm: match (role, self.radial_stock_to_leave_mm()) {
                (PassRole::Finish | PassRole::SemiFinish, _) => Leave::FinishPass,
                (PassRole::Roughing, None) => Leave::NoField,
                (PassRole::Roughing, Some(mm)) if mm.is_finite() && mm > 0.0 => Leave::Mm(mm),
                (PassRole::Roughing, Some(_)) => Leave::Zero,
            },
            // EXEMPT(F-DepthRule)
            depth_rule: RigidityProfile::depth_limit(family, role),
        }
    }
}
