//! **Every limit before the simulation judges every declared cut** (feeds
//! contract P29-7 7c).
//!
//! The rule, in the order a reader meets it. It has the shape of the side
//! after the simulation (`tool_load::groups`, P29-6):
//!
//! 1. Each op declares its cuts (`OperationConfig::cuts`, P29-3 and 7b).
//!    [`DeclaredCuts`] holds them with the facts that a width column reads.
//! 2. At one body column of the envelope plane ([`BodyColumn`]: the width
//!    and the chip of the body) each declared cut is a [`PlannedCut`] with
//!    its own width (its `DeclaredWidth` rule) and its own chip (its
//!    `DeclaredFeed` rule).
//! 3. A limit before the simulation is a [`PlannedLimit`]: one function over
//!    one planned cut. It never walks the cut list and never decides a
//!    scope.
//! 4. [`judge_declared_cuts`] is the one driver. For each declared cut it
//!    asks [`crate::limits::scope`] (the one scope table of both sides) and
//!    calls the limit on each judged cell. The record ([`ByCut`]) holds one
//!    cell for each declared cut; its one constructor calls the closure once
//!    for each cut, so a record cannot leave a cut out.
//! 5. The envelope draws ONE edge per limit: at each width column the lowest
//!    value over the judged cuts that set a ceiling ([`sets_a_ceiling`]: a
//!    cut with `Presence::Always` on the body plane, [`ByCut::lowest_always`]).
//!    A cut with `Presence::MayOccur` is judged too, but it gives a check row
//!    and a note, never a ceiling (lead ruling Q1 of P29-7): its exemption
//!    row names it.
//! 6. P29-8 8d: an entry is a declared cut with its own depth
//!    ([`PlaneDepth::Own`]: the depth per pass of a ramp, one pitch of a
//!    helix), its own width (a slot) and its own chip (the entry feed of
//!    the op over the stated spindle speed, `ConfiguredEntry::
//!    planned_chip_mm`). Every limit judges it like any cut, as a VALUE
//!    CHECK: the depth ceilings compare the own depth with the cut's root
//!    (lead ruling Q1). An own-depth cut never sets a ceiling of the body
//!    plane and never sets the pre-sim power ([`worst_power`]).
//!
//! To add a limit before the simulation: add a `LimitId` (the compiler asks
//! `limits::scope`, [`planned_limit_of`] and `tool_load::groups::gate_of`),
//! write one `impl PlannedLimit`, and read its edge through the driver.

use crate::compute::catalog::{
    CutList, DeclaredCut, DeclaredDepth, DeclaredFeed, DeclaredWidth, EntryKind, MAX_DECLARED_CUTS,
    OperationConfig, OwnDepth, PlannerWidth, Presence,
};
use crate::compute::tool_config::ToolConfig;
use crate::feeds::CutBody;
use crate::feeds::engagement::{EngagedDiameter, PowerCase};
use crate::feeds::entry_rule::{ConfiguredEntry, EntryNoNumber, StoredFeedChip};
use crate::feeds::force_budget::BudgetDepth;
use crate::feeds::geometry::{RadialThinning, UnliftedFeed};
use crate::limits::{
    BoundSide, CutGroup, CutKind, LimitId, NoEngagementModel, NotJudged, PlanReason, Scope,
};
use crate::material::Material;
use crate::tool::ToolDefinition;

/// Why a declared cut has no chip before the simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannedNoNumber {
    /// The planner caps the feed on its own stock read
    /// (`DeclaredFeed::PlannerCapped`).
    PlannerCapped,
    /// P29-8 8d: the entry rule gives no feed or no chip before the
    /// simulation, for this reason.
    Entry(EntryNoNumber),
    /// The drill cycle sets the feed (Step 9c).
    DrillCycle,
    /// The column states no body chip (the power fold reads the width
    /// only).
    NoBodyChip,
}

/// P29-8 8d: where the depth of a planned cut comes from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaneDepth {
    /// The depth of the body plane: the depth axis of the envelope.
    Body,
    /// The cut's own depth (an entry): a value check, never a ceiling of
    /// the body plane.
    Own(OwnDepth),
    /// A straight descent (a plunge or a peck ladder entry): the axial
    /// circle at its own feed. Only the spindle power has a model for it;
    /// a value check, never a ceiling.
    Axial,
}

impl PlaneDepth {
    /// The plane depth of a declared depth. A stated depth and no depth are
    /// on the body plane; an own depth is the cut's own.
    #[must_use]
    pub const fn of_declared(depth: DeclaredDepth) -> Self {
        match depth {
            DeclaredDepth::Stated(_)
            | DeclaredDepth::OfTool(_)
            | DeclaredDepth::AtLeast(_)
            | DeclaredDepth::EarlierLeave
            | DeclaredDepth::None(_) => Self::Body,
            DeclaredDepth::Own(own) => Self::Own(own),
            DeclaredDepth::Axial => Self::Axial,
        }
    }
}

/// True when a judged cut of `presence` and `depth` sets a depth ceiling of
/// the body plane, and so the pre-sim power: a cut that the plan always
/// cuts, on the body plane. THE one rule of [`ByCut::lowest_always`],
/// [`worst_power`], [`power_case_width`] and the binding cut of the
/// envelope. One match, no wildcard: a new presence or depth source does
/// not compile until it answers here.
#[must_use]
pub const fn sets_a_ceiling(presence: Presence, depth: PlaneDepth) -> bool {
    match (presence, depth) {
        (Presence::Always, PlaneDepth::Body) => true,
        // P29-7 Q1: a cut that the plan may not cut is no ceiling.
        // P29-8 Q1: an entry is a value check at its own depth.
        (Presence::MayOccur(_), PlaneDepth::Body | PlaneDepth::Own(_) | PlaneDepth::Axial)
        | (Presence::Always, PlaneDepth::Own(_) | PlaneDepth::Axial) => false,
    }
}

/// One declared cut at one body column of the envelope plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlannedCut {
    /// The kind.
    pub kind: CutKind,
    /// The group of the cut ([`planned_group`]: the group of the kind, the
    /// drill group for a drill cycle depth).
    pub group: CutGroup,
    /// P29-8 8d: the body plane, or the cut's own depth.
    pub depth: PlaneDepth,
    /// The width of the cut (mm), or why the cut has none on this plane.
    pub width_mm: Result<f64, NoEngagementModel>,
    /// The advance per tooth of the cut (mm), or why it has none.
    pub chip_mm: Result<f64, PlannedNoNumber>,
    /// P29-9 task 1: the chip interval of a stored entry feed
    /// ([`StoredFeedChip`]); `None` for a chip that the calculator gives
    /// (a point). `chip_mm` is its stated chip.
    pub stored_chip: Option<StoredFeedChip>,
    /// Whether the plan always cuts it.
    pub presence: Presence,
}

/// One width column of the envelope plane: the width and the chip of the
/// body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyColumn {
    /// The body width (mm).
    pub ae_mm: f64,
    /// The advance per tooth of the body (mm); `None` for a reader of the
    /// width only (the power fold).
    pub chip_mm: Option<f64>,
    /// P29-10 10i: the chip interval of the stored op feed when the column
    /// is the stated cut (`chip_mm` is its stated chip); `None` for a chip
    /// the calculator gives (the proposal, an edge column).
    pub stored: Option<StoredFeedChip>,
}

/// P29-10 10i (lead ruling 3): **THE group of a declared cut before the
/// simulation**: the group of its kind, except a cut whose depth is the
/// drill cycle (`DeclaredDepth::None(NoEngagementModel::DrillCycle)`: the
/// body of a Drill or an Alignment Pin Drill). A drill op's body is not a
/// milling cut: it is judged in the drill group, so `limits::scope` answers
/// for it as for the drill cycle (the chip band, the rubbing floor and the
/// vendor depth table `NotJudged` row `P28-20`), and the drill gates stay
/// its judge. The sample side reads the drill group for the same op
/// (`tool_load::locality::judged_group`).
#[must_use]
pub const fn planned_group(cut: &DeclaredCut) -> CutGroup {
    match cut.depth {
        DeclaredDepth::None(NoEngagementModel::DrillCycle) => CutGroup::Drill,
        DeclaredDepth::Stated(_)
        | DeclaredDepth::OfTool(_)
        | DeclaredDepth::AtLeast(_)
        | DeclaredDepth::EarlierLeave
        | DeclaredDepth::None(_)
        | DeclaredDepth::Own(_)
        | DeclaredDepth::Axial => cut.kind.group(),
    }
}

/// P29-8 8d: the chip of each entry kind of one op, from the entry rule
/// (`ConfiguredEntry::planned_chip_mm`). The fields are private; one
/// match names every kind ([`Self::get`]).
#[derive(Debug, Clone, Copy, PartialEq)]
struct EntryChips {
    plunge: Result<StoredFeedChip, EntryNoNumber>,
    ramp: Result<StoredFeedChip, EntryNoNumber>,
    helix: Result<StoredFeedChip, EntryNoNumber>,
    peck_ladder: Result<StoredFeedChip, EntryNoNumber>,
}

impl EntryChips {
    /// `lift`: the body lift of the unlifted feed that a ramp or a helix
    /// entry feed is written from (P29-10 10i, lead ruling 1).
    fn of(
        operation: &OperationConfig,
        entry: &ConfiguredEntry,
        flute_count: u32,
        lift: f64,
    ) -> Self {
        let chip = |kind| entry.planned_chip_mm(operation, kind, flute_count, lift);
        Self {
            plunge: chip(EntryKind::Plunge),
            ramp: chip(EntryKind::Ramp),
            helix: chip(EntryKind::Helix),
            peck_ladder: chip(EntryKind::PeckLadder),
        }
    }

    const fn get(&self, kind: EntryKind) -> Result<StoredFeedChip, EntryNoNumber> {
        match kind {
            EntryKind::Plunge => self.plunge,
            EntryKind::Ramp => self.ramp,
            EntryKind::Helix => self.helix,
            EntryKind::PeckLadder => self.peck_ladder,
        }
    }
}

/// P29-9: the depth of the body cut of an op on a tool, resolved: THE depth
/// door ([`DeclaredCuts::body_depth`]). The calculator (`FeedsInput`), the
/// envelope axis and the planned limits read it. One match names every
/// declared depth ([`Self::of_declared`]): a new depth source does not
/// compile until it answers here, and each reader matches this enum with
/// no wildcard.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BodyDepth {
    /// A depth of the body plane (mm): a stated depth, or a depth of the
    /// tool (a chamfer, 9a).
    Plane(f64),
    /// P29-9 9b: a lower bound (mm, above zero) of the depth into the
    /// material (a Project Curve tip below the model surface). A check at
    /// the bound proves `Exceeds` and never `Within`
    /// (`NoNumberReason::OnlyALowerBound`).
    AtLeast(f64),
    /// P29-9 9c: the axial stock (mm) that the earlier op leaves; the
    /// checks judge the finish cut at it.
    EarlierLeave(f64),
    /// P29-9 9c: a finish pass on the earlier op's leave, and no earlier op
    /// states one: the checks of the body give `NoNumber(NoEarlierLeave)`.
    NoEarlierLeave(NoEarlierLeave),
    /// No depth before the simulation, with the reason.
    None(NoEngagementModel),
}

/// P29-9 9c (ruling Q1 (a)): the axial stock that the earlier op leaves for
/// a finish pass. The session fills it (`ProjectSession::suggest_slots`):
/// the nearest earlier enabled op of the same setup that cuts the same
/// model and states an axial leave above 0.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EarlierLeave {
    /// The leave (mm), above 0.
    Mm(f64),
    /// No leave, with the reason.
    None(NoEarlierLeave),
}

/// Why a finish pass has no earlier leave.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoEarlierLeave {
    /// No earlier enabled op of the setup that cuts the same model states
    /// an axial leave above 0, or the reader holds no project.
    NoEarlierOp,
}

impl EarlierLeave {
    /// The leave of an optional value (mm): a value above 0 is a leave,
    /// anything else is no earlier op.
    #[must_use]
    pub fn of_mm(mm: Option<f64>) -> Self {
        match mm {
            Some(v) if v.is_finite() && v > 0.0 => Self::Mm(v),
            Some(_) | None => Self::None(NoEarlierLeave::NoEarlierOp),
        }
    }
}

/// P29-9: the facts of the plan that a declared depth reads beside the op
/// and the tool. A reader with no project (FM1, the add door) passes
/// [`Self::NONE`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanFacts {
    /// 9b: whether the toolpath's setup inverts Z
    /// (`SetupTransformInfo::is_z_flipped`); `None`: not known.
    pub setup_z_flipped: Option<bool>,
    /// 9c: the axial stock that the earlier op leaves.
    pub earlier_leave: EarlierLeave,
}

impl PlanFacts {
    /// No fact of the project: no setup, and no earlier op
    /// (`None(NoEarlierOp)`). Private (P29-11 L-2, S9): only
    /// [`DeclaredCuts::of_operation`] reads it; a reader of the speeds depth
    /// with no facts calls [`body_speeds_depth_of`].
    const NONE: Self = Self {
        setup_z_flipped: None,
        earlier_leave: EarlierLeave::None(NoEarlierLeave::NoEarlierOp),
    };
}

impl BodyDepth {
    /// The body depth of the declared depth `depth` on a tool of shape
    /// `hint`, with the facts of the plan.
    #[must_use]
    pub fn of_declared(
        depth: DeclaredDepth,
        hint: crate::feeds::ToolGeometryHint,
        facts: PlanFacts,
    ) -> Self {
        match depth {
            DeclaredDepth::Stated(mm) => Self::Plane(mm),
            DeclaredDepth::OfTool(of_tool) => match of_tool.mm(hint) {
                Ok(mm) => Self::Plane(mm),
                Err(reason) => Self::None(reason),
            },
            DeclaredDepth::AtLeast(tip) => match tip.lower_bound_mm(facts.setup_z_flipped) {
                Some(mm) => Self::AtLeast(mm),
                None => Self::None(NoEngagementModel::DepthAfterSimulation),
            },
            DeclaredDepth::EarlierLeave => match facts.earlier_leave {
                EarlierLeave::Mm(mm) => Self::EarlierLeave(mm),
                EarlierLeave::None(reason) => Self::NoEarlierLeave(reason),
            },
            DeclaredDepth::None(reason) => Self::None(reason),
            // Not reached: the body is on the body plane.
            DeclaredDepth::Own(_) | DeclaredDepth::Axial => {
                Self::None(NoEngagementModel::NoStatedDepth)
            }
        }
    }

    /// The depth (mm) at which the calculator sizes the speeds: a depth of
    /// the body plane above zero, else `None` (the calculator then reads
    /// its family default, row `P28-31`).
    #[must_use]
    pub fn speeds_mm(self) -> Option<f64> {
        match self {
            Self::Plane(mm) if mm > 0.0 => Some(mm),
            // 9b: the calculator does not size the speeds at a lower bound
            // (P16: the depth is known after the simulation).
            // 9c (ruling Q3): the speeds of a finish keep the default table.
            Self::Plane(_)
            | Self::AtLeast(_)
            | Self::EarlierLeave(_)
            | Self::NoEarlierLeave(_)
            | Self::None(_) => None,
        }
    }
}

/// P29-11 L-2 (S9): the depth at which the calculator sizes the speeds of
/// the body cut of `operation` on a tool of shape `hint`
/// ([`BodyDepth::speeds_mm`]), for a reader with no plan facts (the gate
/// context, `operation_feeds_hints`). No plan fact moves it: every arm that a
/// fact fills gives `None`. So the door takes no facts argument, and no
/// product reader states "no facts".
#[must_use]
pub fn body_speeds_depth_of(
    operation: &OperationConfig,
    hint: crate::feeds::ToolGeometryHint,
) -> Option<f64> {
    body_depth_of(operation, hint, PlanFacts::NONE).speeds_mm()
}

/// P29-9 c8: THE depth door for a reader that holds no declared cut list
/// (the gate context, `operation_feeds_hints`): the body depth of
/// `operation` on a tool of shape `hint`, as [`DeclaredCuts::body_depth`]
/// gives it. `CutSpec::calc_inputs` gives no depth, so no reader keeps a
/// second depth rule.
#[must_use]
pub fn body_depth_of(
    operation: &OperationConfig,
    hint: crate::feeds::ToolGeometryHint,
    facts: PlanFacts,
) -> BodyDepth {
    operation
        .cuts(&ConfiguredEntry::registry_default(operation))
        .of_kind(CutKind::Body)
        .map_or(BodyDepth::None(NoEngagementModel::NoStatedDepth), |c| {
            BodyDepth::of_declared(c.depth, hint, facts)
        })
}

/// The declared cuts of one op on one tool, with the facts that each width
/// column reads. Build it once per envelope ([`Self::of_operation`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeclaredCuts {
    list: CutList,
    d_mm: f64,
    body_depth: Option<DeclaredDepth>,
    body: BodyDepth,
    thinning: RadialThinning,
    lift_d_eng_mm: f64,
    entry_chips: EntryChips,
    entry_rpm: Result<f64, EntryNoNumber>,
}

impl DeclaredCuts {
    /// The cuts of `operation` on `tool`, with the configured entry `entry`
    /// (P29-8 8d). The lift of the unlifted feed is read at the stated depth
    /// per pass (else the tool diameter), the rule of
    /// `UnliftedFeed::of_slot_ring`. The chip of each entry kind is the
    /// entry rule's.
    #[must_use]
    pub fn of_operation(
        operation: &OperationConfig,
        tool: &ToolConfig,
        entry: &ConfiguredEntry,
    ) -> Self {
        Self::of_operation_with(operation, tool, entry, PlanFacts::NONE)
    }

    /// [`Self::of_operation`] with the facts of the plan (P29-9: the
    /// session doors fill them from the toolpath's setup).
    #[must_use]
    pub fn of_operation_with(
        operation: &OperationConfig,
        tool: &ToolConfig,
        entry: &ConfiguredEntry,
        facts: PlanFacts,
    ) -> Self {
        let list = operation.cuts(entry);
        let d_mm = tool.diameter;
        let usable = |v: f64| v.is_finite() && v > 0.0;
        let ap = operation
            .depth_per_pass()
            .filter(|v| usable(*v))
            .unwrap_or(d_mm);
        let lift_d_eng_mm = EngagedDiameter::of_hint(
            tool.geometry_hint(),
            d_mm,
            CutBody::of_tool(tool).max_cut_diameter_mm,
            ap,
        )
        .mm();
        let thinning = list.body_facts().thins_chip();
        // P29-10 10i (lead ruling 1): the lift of the unlifted feed at the
        // stated width (else D), the lift a ramp feed is written over.
        let stated_ae = operation.stepover().filter(|v| usable(*v)).unwrap_or(d_mm);
        let entry_lift = thinning.lift(list.full_feed_width(stated_ae, d_mm), lift_d_eng_mm);
        Self {
            list,
            d_mm,
            body_depth: list.of_kind(CutKind::Body).map(|c| c.depth),
            body: list
                .of_kind(CutKind::Body)
                .map_or(BodyDepth::None(NoEngagementModel::NoStatedDepth), |c| {
                    BodyDepth::of_declared(c.depth, tool.geometry_hint(), facts)
                }),
            thinning,
            lift_d_eng_mm,
            entry_chips: EntryChips::of(operation, entry, tool.flute_count_planning(), entry_lift),
            entry_rpm: ConfiguredEntry::stated_rpm(operation),
        }
    }

    /// P29-8 8d: the spindle speed (rpm) at which the entry chips are read
    /// (the stated speed), or why none.
    ///
    /// # Errors
    ///
    /// [`EntryNoNumber::NoSpindleSpeed`] when the op states no speed.
    pub const fn entry_rpm(&self) -> Result<f64, EntryNoNumber> {
        self.entry_rpm
    }

    /// P29-8 8d: why the entry rule gives an entry of `kind` no chip, or
    /// `None` when it gives one.
    #[must_use]
    pub const fn entry_no_number(&self, kind: EntryKind) -> Option<EntryNoNumber> {
        match self.entry_chips.get(kind) {
            Ok(_) => None,
            Err(reason) => Some(reason),
        }
    }

    /// P29-9: THE depth of the body cut on this tool ([`BodyDepth`]).
    #[must_use]
    pub const fn body_depth(&self) -> BodyDepth {
        self.body
    }

    /// The declared cuts.
    #[must_use]
    pub const fn list(&self) -> &CutList {
        &self.list
    }

    /// The tool diameter (mm).
    #[must_use]
    pub const fn d_mm(&self) -> f64 {
        self.d_mm
    }

    /// The radial thinning rule of the body (`CutFacts::thins_chip`).
    #[must_use]
    pub const fn thinning(&self) -> RadialThinning {
        self.thinning
    }

    /// The engaged diameter (mm) of the lift: the one of the unlifted feed.
    #[must_use]
    pub const fn lift_d_eng_mm(&self) -> f64 {
        self.lift_d_eng_mm
    }

    /// The planned cut of `cut` at `column`.
    ///
    /// The width:
    ///
    /// - the body: the column width; a body that declares the slot width
    ///   (a slot, a groove, P29-5c) reads the tool diameter;
    /// - a cut that states no depth of the body's depth axis (the wall
    ///   finish of a Bore) has no width on this plane;
    /// - `Stated` gives the column width (the stepover IS the column);
    ///   `Slot` gives D; `PassCeiling` gives `c(ae) D`; `CapMult` gives
    ///   `min(m ae, D)`; `Scallop` and `None` give no number.
    ///
    /// The chip: `OpFeed` gives the body chip; `Unlifted` gives the body
    /// chip over the body lift (`UnliftedFeed`: the lift at the full-feed
    /// width of the column, on the engaged diameter of the lift); an entry
    /// gives the chip of its entry rule (P29-8 8d), the same at every
    /// column; the planner-capped and drill feeds give no number.
    ///
    /// A cut at its own depth (an entry) reads its own width rule, not the
    /// body depth.
    #[must_use]
    pub fn planned(&self, cut: &DeclaredCut, column: BodyColumn) -> PlannedCut {
        let d = self.d_mm;
        let ae = column.ae_mm;
        let depth = PlaneDepth::of_declared(cut.depth);
        let of_rule = || match cut.width {
            DeclaredWidth::Stated(_) => Ok(ae),
            DeclaredWidth::Slot => Ok(d),
            DeclaredWidth::PassCeiling { .. } => {
                Ok(PlannerWidth::PassCeiling.full_feed_width_mm(ae, d))
            }
            DeclaredWidth::CapMult { mult, .. } => PlannerWidth::CapMult(mult)
                .power_width_mm(ae, d)
                .ok_or(NoEngagementModel::NoStatedWidth),
            DeclaredWidth::Scallop(_) => Err(NoEngagementModel::NoStatedWidth),
            DeclaredWidth::None(reason) => Err(reason),
        };
        let width_mm = match cut.kind {
            CutKind::Body => Ok(match cut.width {
                DeclaredWidth::Slot => d,
                DeclaredWidth::Stated(_)
                | DeclaredWidth::PassCeiling { .. }
                | DeclaredWidth::CapMult { .. }
                | DeclaredWidth::Scallop(_)
                | DeclaredWidth::None(_) => ae,
            }),
            CutKind::FirstPass(_)
            | CutKind::SpringPass
            | CutKind::WallFinish
            | CutKind::Entry(_)
            | CutKind::EntryApproach
            | CutKind::Cleanup(_)
            | CutKind::CappedStep(_)
            | CutKind::SlotRing
            | CutKind::LeadIn
            | CutKind::LeadOut
            | CutKind::Link
            | CutKind::DressupRelief
            | CutKind::Drill => match depth {
                // P29-8 8d: a cut at its own depth, or an axial cut, has
                // its own width.
                PlaneDepth::Own(_) | PlaneDepth::Axial => of_rule(),
                PlaneDepth::Body => {
                    if self.body_depth.is_some_and(|body| body != cut.depth) {
                        Err(NoEngagementModel::NoStatedDepth)
                    } else {
                        of_rule()
                    }
                }
            },
        };
        let body_chip = column.chip_mm.ok_or(PlannedNoNumber::NoBodyChip);
        let unlift = || {
            self.thinning
                .lift(self.list.full_feed_width(ae, d), self.lift_d_eng_mm)
        };
        // P29-10 10i: a cut fed at the op feed at the stated point reads the
        // interval of the stored op feed (the slot ring over its lift).
        let stored_chip = match cut.feed {
            DeclaredFeed::EntryRule(kind) => self.entry_chips.get(kind).ok(),
            DeclaredFeed::OpFeed => column.stored,
            DeclaredFeed::Unlifted => column.stored.map(|c| c.over_lift(unlift())),
            DeclaredFeed::PlannerCapped | DeclaredFeed::DrillCycle => None,
        };
        let chip_mm = match cut.feed {
            DeclaredFeed::OpFeed => body_chip,
            DeclaredFeed::Unlifted => {
                body_chip.map(|fz| UnliftedFeed::over_lift(fz, unlift()).mm_min())
            }
            DeclaredFeed::PlannerCapped => Err(PlannedNoNumber::PlannerCapped),
            DeclaredFeed::EntryRule(kind) => self
                .entry_chips
                .get(kind)
                .map(StoredFeedChip::stated_mm)
                .map_err(PlannedNoNumber::Entry),
            DeclaredFeed::DrillCycle => Err(PlannedNoNumber::DrillCycle),
        };
        PlannedCut {
            kind: cut.kind,
            group: planned_group(cut),
            depth,
            width_mm,
            chip_mm,
            stored_chip,
            presence: cut.presence,
        }
    }
}

/// **A limit before the simulation**: one function over one planned cut.
/// The driver ([`judge_declared_cuts`], and its ceiling-only form
/// [`lowest_ceiling`]) is its only caller.
pub trait PlannedLimit {
    /// The limit. The driver asks [`crate::limits::scope`] for it on the
    /// group of each cut.
    const LIMIT: LimitId;
    /// What the limit reads beside the cut (the tool, the material, the
    /// budget).
    type Env<'e>;
    /// The answer for one cut.
    type Answer: Copy;
    /// The answer for one judged cut.
    fn judge(cut: &PlannedCut, env: &Self::Env<'_>) -> Self::Answer;
}

/// The cell of one limit for one declared cut.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlannedCell<A> {
    /// The scope judges the cut, and the limit answered.
    Judged(A),
    /// The scope does not judge the cut: its exemption row and clause.
    NotJudged(NotJudged),
    /// The limit is not a limit of each cut (a planned limit never answers
    /// this; the sentry holds it).
    NotACutLimit(PlanReason),
}

/// One entry of a [`ByCut`] record.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CutEntry<T> {
    /// The kind of the declared cut.
    pub kind: CutKind,
    /// Whether the plan always cuts it.
    pub presence: Presence,
    /// P29-8 8d: the body plane, or the cut's own depth.
    pub depth: PlaneDepth,
    /// The value of the cut.
    pub value: T,
}

/// One value for each declared cut. The field is private and the one
/// constructor [`Self::from_fn`] calls its closure once for each declared
/// cut, in the declaration order, so a record cannot leave a cut out.
#[derive(Debug, Clone, PartialEq)]
pub struct ByCut<T> {
    cells: [Option<CutEntry<T>>; MAX_DECLARED_CUTS],
}

impl<T> ByCut<T> {
    /// The record with `f(cut)` for each cut of `cuts`.
    pub fn from_fn(cuts: &CutList, mut f: impl FnMut(&DeclaredCut) -> T) -> Self {
        let mut each = cuts.iter();
        Self {
            cells: std::array::from_fn(|_| {
                each.next().map(|cut| CutEntry {
                    kind: cut.kind,
                    presence: cut.presence,
                    depth: PlaneDepth::of_declared(cut.depth),
                    value: f(cut),
                })
            }),
        }
    }

    /// The entries, in the declaration order.
    pub fn iter(&self) -> impl Iterator<Item = &CutEntry<T>> {
        self.cells.iter().flatten()
    }
}

impl<A: Copy> ByCut<PlannedCell<A>> {
    /// The lowest `value` over the judged cuts that set a ceiling
    /// ([`sets_a_ceiling`]), and the cut that gives it. `value` gives `None` for an answer with no
    /// number. A tie keeps the first cut (the body is first). `None` when no
    /// such cut has a number.
    #[must_use]
    pub fn lowest_always(&self, value: impl Fn(&A) -> Option<f64>) -> Option<(CutKind, A)> {
        let mut best: Option<(f64, CutKind, A)> = None;
        for entry in self.iter() {
            let PlannedCell::Judged(answer) = entry.value else {
                continue;
            };
            if !sets_a_ceiling(entry.presence, entry.depth) {
                continue;
            }
            let Some(v) = value(&answer) else {
                continue;
            };
            keep_lowest(&mut best, v, entry.kind, answer);
        }
        best.map(|(_, kind, answer)| (kind, answer))
    }
}

/// The one tie rule of the lowest ceiling: a cut replaces the best only
/// when its value is strictly lower, so a tie keeps the first cut.
fn keep_lowest<A>(best: &mut Option<(f64, CutKind, A)>, v: f64, kind: CutKind, answer: A) {
    if best.as_ref().is_none_or(|(b, _, _)| v < *b) {
        *best = Some((v, kind, answer));
    }
}

/// **The one driver.** For each declared cut of `cuts` at `column`: the
/// scope cell of `L` on the group of the cut; a judged cell calls `L`.
pub fn judge_declared_cuts<L: PlannedLimit>(
    cuts: &DeclaredCuts,
    column: BodyColumn,
    env: &L::Env<'_>,
) -> ByCut<PlannedCell<L::Answer>> {
    ByCut::from_fn(&cuts.list, |cut| judge_one::<L>(cuts, cut, column, env))
}

/// The cell of `L` for one declared cut: the scope cell on the group of the
/// cut; a judged cell calls `L`.
fn judge_one<L: PlannedLimit>(
    cuts: &DeclaredCuts,
    cut: &DeclaredCut,
    column: BodyColumn,
    env: &L::Env<'_>,
) -> PlannedCell<L::Answer> {
    let planned = cuts.planned(cut, column);
    match crate::limits::scope(L::LIMIT, planned.group) {
        Scope::Judged => PlannedCell::Judged(L::judge(&planned, env)),
        Scope::NotJudged(cell) => PlannedCell::NotJudged(cell),
        Scope::NotACutLimit(reason) => PlannedCell::NotACutLimit(reason),
    }
}

/// **The driver of an edge**: the same answer as
/// `judge_declared_cuts::<L>(cuts, column, env).lowest_always(value)`, but
/// it judges only the cuts that set a ceiling ([`sets_a_ceiling`]).
/// `lowest_always` drops every other cut before it reads the answer, so
/// their judgement is lost work. P29-10: an entry (P29-8 8d) has a width and
/// a chip but never sets a ceiling, and its full solve at each width
/// column of the force-budget edge cost about 40 % of the envelope time
/// (`P29-10_DESIGN.md` section 15). The rows still read every cut through
/// [`judge_declared_cuts`].
pub fn lowest_ceiling<L: PlannedLimit>(
    cuts: &DeclaredCuts,
    column: BodyColumn,
    env: &L::Env<'_>,
    value: impl Fn(&L::Answer) -> Option<f64>,
) -> Option<(CutKind, L::Answer)> {
    let mut best: Option<(f64, CutKind, L::Answer)> = None;
    for cut in cuts.list.iter() {
        if !sets_a_ceiling(cut.presence, PlaneDepth::of_declared(cut.depth)) {
            continue;
        }
        let PlannedCell::Judged(answer) = judge_one::<L>(cuts, cut, column, env) else {
            continue;
        };
        let Some(v) = value(&answer) else {
            continue;
        };
        keep_lowest(&mut best, v, cut.kind, answer);
    }
    best.map(|(_, kind, answer)| (kind, answer))
}

/// The width and the chip of a planned side cut, when it has both. An
/// axial cut (a plunge, P29-8 8d) is no side cut: the force budget, the
/// deflection, the vendor depth table and the chip band have no model for
/// it.
fn width_and_chip(cut: &PlannedCut) -> Option<(f64, f64)> {
    match cut.depth {
        PlaneDepth::Body | PlaneDepth::Own(_) => {}
        PlaneDepth::Axial => return None,
    }
    match (cut.width_mm, cut.chip_mm) {
        (Ok(w), Ok(fz)) => Some((w, fz)),
        (Err(_), _) | (_, Err(_)) => None,
    }
}

/// What the force budget and the deflection read beside the cut.
#[derive(Clone, Copy)]
pub struct ForceEnv<'e> {
    /// The built tool.
    pub tool: &'e ToolDefinition,
    /// The stock material.
    pub material: &'e Material,
    /// The force budget (N) of the force-budget limit, or the deflection
    /// target (um) of the deflection limit.
    pub bound: f64,
}

/// The machine force budget before the simulation: the shallowest depth at
/// which the peak engaged-edge force of the cut, at its width and chip,
/// meets the budget (`force_budget::depth_at_budget`). `None`: no number.
pub struct ForceBudgetPlanned;

impl PlannedLimit for ForceBudgetPlanned {
    const LIMIT: LimitId = LimitId::ForceBudget;
    type Env<'e> = ForceEnv<'e>;
    type Answer = Option<BudgetDepth>;
    fn judge(cut: &PlannedCut, env: &ForceEnv<'_>) -> Option<BudgetDepth> {
        let (w, fz) = width_and_chip(cut)?;
        crate::feeds::force_budget::depth_at_budget(
            env.tool,
            env.material,
            w.max(1.0e-3),
            fz,
            env.bound,
        )
    }
}

/// The ceiling (mm) of a force-budget answer, for the lowest edge: the
/// solved depth, the minimum depth below the floor, and no root ranks
/// above every depth.
#[must_use]
pub fn budget_rank(answer: &Option<BudgetDepth>) -> Option<f64> {
    answer.map(|b| match b {
        BudgetDepth::Solved(mm) => mm,
        BudgetDepth::BelowFloor => crate::feeds::MIN_AP_MM,
        BudgetDepth::NoRootBelowFlute => f64::INFINITY,
    })
}

/// The tool deflection before the simulation: the shallowest depth at which
/// the tip deflection of the cut, at its width and chip, meets the target
/// (`cutter_axial_constraints`, the one beam). `None`: no root below the
/// flute, or no number.
pub struct DeflectionPlanned;

impl PlannedLimit for DeflectionPlanned {
    const LIMIT: LimitId = LimitId::ToolDeflection;
    type Env<'e> = ForceEnv<'e>;
    type Answer = Option<f64>;
    fn judge(cut: &PlannedCut, env: &ForceEnv<'_>) -> Option<f64> {
        let (w, fz) = width_and_chip(cut)?;
        crate::feeds::cutter_constraints::cutter_axial_constraints(
            env.tool,
            env.material,
            w.max(1.0e-3),
            fz,
            None,
            None,
            Some(env.bound),
        )
        .max_doc_deflection_mm
        .filter(|v| v.is_finite() && *v >= 0.0)
    }
}

/// What the vendor depth table reads beside the cut.
#[derive(Debug, Clone, Copy)]
pub struct TableEnv {
    /// The band maximum of the matched row (mm per tooth).
    pub band_max_mm: f64,
    /// The tool diameter (mm).
    pub d_mm: f64,
    /// The top of the depth span (mm): the ceiling where the table holds
    /// every printed depth.
    pub top_mm: f64,
}

/// The vendor depth table before the simulation (P11b L6): the deepest
/// depth where the table's allowed chip still holds the peak chip of the
/// cut (`fz sin psi`, a slot has the largest share). The answer is the
/// ceiling and whether the table binds there (a share above one half).
pub struct VendorDepthTablePlanned;

// P29-11 section 13, ruling 1: no sample gate judges this limit after the
// simulation (`tool_load::groups::gate_of` gives none).
// EXEMPT(S-NoSampleGate)
impl PlannedLimit for VendorDepthTablePlanned {
    const LIMIT: LimitId = LimitId::VendorDepthTable;
    type Env<'e> = TableEnv;
    type Answer = Option<(f64, bool)>;
    fn judge(cut: &PlannedCut, env: &TableEnv) -> Option<(f64, bool)> {
        let (w, fz) = width_and_chip(cut)?;
        Some(table_ceiling(w, fz, env))
    }
}

/// The vendor depth table ceiling at the width `ae_mm` and the chip `fz`,
/// and whether the table binds there.
#[must_use]
pub fn table_ceiling(ae_mm: f64, fz: f64, env: &TableEnv) -> (f64, bool) {
    let psi = crate::feeds::force::immersion_angle(ae_mm, 0.5 * env.d_mm);
    let share = fz * psi.min(std::f64::consts::FRAC_PI_2).sin() / env.band_max_mm;
    if share > 0.5 {
        (
            crate::feeds::geometry::vendor_depth_table_ceiling_ratio(share) * env.d_mm,
            true,
        )
    } else {
        (env.top_mm, false)
    }
}

/// What the power fold reads beside the cut.
pub struct PowerEnv<'e> {
    /// The body width of the column (mm): the width of the nominal case.
    pub column_ae_mm: f64,
    /// The power (kW) of a cut on the body plane at a width (mm), at the
    /// op feed.
    pub power_kw: &'e dyn Fn(f64) -> f64,
    /// P29-8 8d: the power (kW) of a cut off the body plane (an entry at
    /// its own depth, or a plunge): the declared cut of the kind at its own
    /// engagement (`DeclaredCut::at`) and the chip (mm) of the cut, at its
    /// own feed. `None`: the reader gives no number (a reader of the body
    /// plane only: the cut sets no pre-sim power, [`sets_a_ceiling`]).
    pub entry_power_kw: &'e dyn Fn(CutKind, f64) -> Option<f64>,
}

/// The spindle power before the simulation (K7 change 3, today's rule):
/// the power of each declared cut at the op feed, at the width of its case
/// (the nominal case reads the column width; a planner cap and a slot read
/// their own width). Lead ruling Q5: the slot ring is sized at the op
/// feed, not at its unlifted feed, so its figure is above the power that
/// the ring draws (a later step gives power the feed rule).
pub struct SpindlePowerPlanned;

impl PlannedLimit for SpindlePowerPlanned {
    const LIMIT: LimitId = LimitId::SpindlePower;
    type Env<'e> = PowerEnv<'e>;
    type Answer = Option<(PowerCase, f64, f64)>;
    fn judge(cut: &PlannedCut, env: &PowerEnv<'_>) -> Option<(PowerCase, f64, f64)> {
        let w = cut.width_mm.ok()?;
        let case = PowerCase::of_cut_kind(cut.kind);
        let width = match case {
            PowerCase::Nominal => env.column_ae_mm,
            PowerCase::PlannerCap | PowerCase::Slot | PowerCase::AxialPlunge => w,
        };
        match cut.depth {
            PlaneDepth::Body => Some((case, (env.power_kw)(width), width)),
            // P29-8 8d: an entry at its own engagement and its own feed.
            PlaneDepth::Own(_) | PlaneDepth::Axial => {
                let fz = cut.chip_mm.ok()?;
                Some((case, (env.entry_power_kw)(cut.kind, fz)?, width))
            }
        }
    }
}

/// The worst case of the pre-sim power set at the body width `ae`: the
/// largest power over the declared cuts that set a ceiling
/// ([`sets_a_ceiling`]: never an entry, P29-8 8d), its case and its width. A tie goes to the more specific case
/// (`PowerCase::tie_rank`). With no judged cut, the nominal case at `ae`.
pub fn worst_power(
    cuts: &DeclaredCuts,
    ae: f64,
    power_kw: impl Fn(f64) -> f64,
) -> (PowerCase, f64, f64) {
    let env = PowerEnv {
        column_ae_mm: ae,
        power_kw: &power_kw,
        entry_power_kw: &|_, _| None,
    };
    let column = BodyColumn {
        ae_mm: ae,
        chip_mm: None,
        stored: None,
    };
    let cells = judge_declared_cuts::<SpindlePowerPlanned>(cuts, column, &env);
    let mut best: Option<(PowerCase, f64, f64)> = None;
    for entry in cells.iter() {
        let PlannedCell::Judged(Some(this)) = entry.value else {
            continue;
        };
        if !sets_a_ceiling(entry.presence, entry.depth) {
            continue;
        }
        let wins = best
            .is_none_or(|b| this.1 > b.1 || (this.1 >= b.1 && this.0.tie_rank() > b.0.tie_rank()));
        if wins {
            best = Some(this);
        }
    }
    best.unwrap_or_else(|| (PowerCase::Nominal, power_kw(ae), ae))
}

/// The width (mm) of the power case `case` at the body width `ae`: the
/// width of the first declared cut that sets a ceiling
/// ([`sets_a_ceiling`]) of that case, else `ae`.
#[must_use]
pub fn power_case_width(cuts: &DeclaredCuts, case: PowerCase, ae: f64) -> f64 {
    let none = |_: f64| 0.0;
    let env = PowerEnv {
        column_ae_mm: ae,
        power_kw: &none,
        entry_power_kw: &|_, _| None,
    };
    let column = BodyColumn {
        ae_mm: ae,
        chip_mm: None,
        stored: None,
    };
    judge_declared_cuts::<SpindlePowerPlanned>(cuts, column, &env)
        .iter()
        .find_map(|entry| match entry.value {
            PlannedCell::Judged(Some((c, _, w)))
                if c == case && sets_a_ceiling(entry.presence, entry.depth) =>
            {
                Some(w)
            }
            PlannedCell::Judged(_) | PlannedCell::NotJudged(_) | PlannedCell::NotACutLimit(_) => {
                None
            }
        })
        .unwrap_or(ae)
}

/// What the chip checks read beside the cut: the thinning rule and the
/// engaged diameter of the lift, and the bound of the check.
#[derive(Debug, Clone, Copy)]
pub struct ChipEnv {
    /// The thinning rule of the body.
    pub thinning: RadialThinning,
    /// The engaged diameter of the lift (mm).
    pub d_eng_mm: f64,
    /// The bound (mm per tooth): the band maximum (or the printed point) or
    /// the rubbing floor. The `ChipMin` check reads [`ChipFloorEnv`].
    pub bound_mm: f64,
}

/// P29-10 PB (lead ruling on the point band): THE chip floor that the
/// envelope hands to the planned `ChipMin` check. A typed value, never a
/// band made from one printed value: a future limit cannot make a floor
/// from a point without a new variant here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChipFloor {
    /// The minimum of a printed band (mm per tooth) and the class of its row
    /// (`ChipBoundsSource::of_row`).
    BandMin {
        /// The band minimum (mm per tooth).
        mm: f64,
        /// The class of the row (never `VendorLutPointPreset`).
        source: crate::tool_load::verdict::ChipBoundsSource,
    },
    /// The printed value of a row that prints one value (A2): a reference,
    /// not a floor that gates.
    PointReference {
        /// The printed value (mm per tooth).
        mm: f64,
    },
}

impl ChipFloor {
    /// The chip floor of a calculator result: the class of its matched row
    /// by THE one classification (`ChipBoundsSource::of_row`, the gate reads
    /// the same function), then the value the class reads (the printed
    /// point, or the band minimum). `None` with no matched row or no value.
    #[must_use]
    pub fn of_result(r: &crate::feeds::FeedsResult) -> Option<Self> {
        use crate::tool_load::verdict::ChipBoundsSource;
        let row = r.matched_lut_row.as_ref()?;
        let usable = |v: &f64| v.is_finite() && *v > 0.0;
        match ChipBoundsSource::of_row(row) {
            ChipBoundsSource::VendorLutPointPreset => r
                .chipload_point_mm
                .filter(usable)
                .map(|mm| Self::PointReference { mm }),
            source @ (ChipBoundsSource::VendorLut
            | ChipBoundsSource::VendorLutExtrapolated
            | ChipBoundsSource::VendorLutMissingAe) => r
                .chipload_bounds
                .map(|b| b.min_mm_per_tooth)
                .filter(usable)
                .map(|mm| Self::BandMin { mm, source }),
        }
    }

    /// The value of the floor (mm per tooth).
    #[must_use]
    pub const fn mm(self) -> f64 {
        match self {
            Self::BandMin { mm, .. } | Self::PointReference { mm } => mm,
        }
    }

    /// Why the floor is a reference and not a floor that gates, or `None`
    /// for the minimum of a direct band. It reads
    /// `ChipBoundsSource::advisory_low_side`, the rule of the gate.
    #[must_use]
    pub const fn advisory(self) -> Option<crate::tool_load::verdict::AdvisoryLowSide> {
        match self {
            Self::BandMin { source, .. } => source.advisory_low_side(),
            Self::PointReference { .. } => {
                crate::tool_load::verdict::ChipBoundsSource::VendorLutPointPreset
                    .advisory_low_side()
            }
        }
    }
}

/// What the `ChipMin` check reads beside the cut: the thinning rule and the
/// engaged diameter of the lift, and the typed floor (P29-10 PB).
#[derive(Debug, Clone, Copy)]
pub struct ChipFloorEnv {
    /// The thinning rule of the body.
    pub thinning: RadialThinning,
    /// The engaged diameter of the lift (mm).
    pub d_eng_mm: f64,
    /// The floor.
    pub floor: ChipFloor,
}

/// The planned peak chip of one cut against one chip bound.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChipReading {
    /// The peak chip of the cut (mm): its advance per tooth over the lift
    /// at its width.
    pub peak_mm: f64,
    /// P29-9 task 1: the peak chip that the check judges: `peak_mm`, or for
    /// a stored entry feed the end of its interval toward the inside of the
    /// bound ([`StoredFeedChip::toward_inside`]).
    pub judged_mm: f64,
    /// The bound (mm).
    pub bound_mm: f64,
    /// P29-10 (lead ruling 2): the chip reads a stored entry feed
    /// (`StoredFeedChip`) AND the peak chip interval of that stored feed
    /// holds the bound, so the rounding of the feed can change the state.
    /// Only then the row carries the caveat `StoredFeed`.
    pub stored_feed: bool,
    /// P29-10 PB: the bound is a chip floor whose low side is advisory
    /// (`ChipFloor::advisory`), with the reason. `None` for every other
    /// chip bound.
    pub advisory_floor: Option<crate::tool_load::verdict::AdvisoryLowSide>,
}

fn peak_chip(cut: &PlannedCut, env: &ChipEnv, side: BoundSide) -> Option<ChipReading> {
    let (w, fz) = width_and_chip(cut)?;
    let lift = env.thinning.lift(w, env.d_eng_mm);
    let judged_fz = cut.stored_chip.map_or(fz, |c| c.toward_inside(side));
    Some(ChipReading {
        peak_mm: UnliftedFeed::over_lift(fz, lift).mm_min(),
        judged_mm: UnliftedFeed::over_lift(judged_fz, lift).mm_min(),
        bound_mm: env.bound_mm,
        stored_feed: cut.stored_chip.is_some_and(|c| {
            let low = UnliftedFeed::over_lift(c.stated_mm(), lift).mm_min();
            let high = UnliftedFeed::over_lift(c.high_mm(), lift).mm_min();
            low <= env.bound_mm && env.bound_mm <= high
        }),
        advisory_floor: None,
    })
}

/// The upper edge of the printed chip band: the planned peak chip of each
/// cut (a value check, no ceiling).
pub struct ChipMaxPlanned;

impl PlannedLimit for ChipMaxPlanned {
    const LIMIT: LimitId = LimitId::ChipMax;
    type Env<'e> = ChipEnv;
    type Answer = Option<ChipReading>;
    fn judge(cut: &PlannedCut, env: &ChipEnv) -> Option<ChipReading> {
        peak_chip(cut, env, BoundSide::Ceiling)
    }
}

/// The lower edge of the printed chip band (a value check, no ceiling).
pub struct ChipMinPlanned;

impl PlannedLimit for ChipMinPlanned {
    const LIMIT: LimitId = LimitId::ChipMin;
    type Env<'e> = ChipFloorEnv;
    type Answer = Option<ChipReading>;
    fn judge(cut: &PlannedCut, env: &ChipFloorEnv) -> Option<ChipReading> {
        let chip = ChipEnv {
            thinning: env.thinning,
            d_eng_mm: env.d_eng_mm,
            bound_mm: env.floor.mm(),
        };
        peak_chip(cut, &chip, BoundSide::Floor).map(|reading| ChipReading {
            advisory_floor: env.floor.advisory(),
            ..reading
        })
    }
}

/// The rubbing floor (a value check, no ceiling; it warns, never lifts).
pub struct RubbingFloorPlanned;

// P29-11 section 13, ruling 1: no sample gate judges this limit after the
// simulation (`tool_load::groups::gate_of` gives none).
// EXEMPT(S-NoSampleGate)
impl PlannedLimit for RubbingFloorPlanned {
    const LIMIT: LimitId = LimitId::RubbingFloor;
    type Env<'e> = ChipEnv;
    type Answer = Option<ChipReading>;
    fn judge(cut: &PlannedCut, env: &ChipEnv) -> Option<ChipReading> {
        peak_chip(cut, env, BoundSide::Floor)
    }
}

/// A limit that draws a depth ceiling on the envelope plane (P29-7 7c).
/// The declaration order is the draw order of the envelope's boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CeilingEdge {
    /// The vendor depth table ([`VendorDepthTablePlanned`]).
    VendorDepthTable,
    /// The spindle power ([`SpindlePowerPlanned`]: the calculator's power
    /// is the largest over the declared cuts, [`worst_power`]).
    Power,
    /// The tool deflection ([`DeflectionPlanned`]).
    Deflection,
    /// The machine force budget ([`ForceBudgetPlanned`]).
    ForceBudget,
}

impl CeilingEdge {
    /// Every depth ceiling, in the draw order.
    pub const ALL: [Self; 4] = crate::all_variants!(CeilingEdge:
        VendorDepthTable, Power, Deflection, ForceBudget,
    );

    /// The limit of the edge: its `PlannedLimit::LIMIT`.
    #[must_use]
    pub const fn limit(self) -> LimitId {
        match self {
            Self::VendorDepthTable => VendorDepthTablePlanned::LIMIT,
            Self::Power => SpindlePowerPlanned::LIMIT,
            Self::Deflection => DeflectionPlanned::LIMIT,
            Self::ForceBudget => ForceBudgetPlanned::LIMIT,
        }
    }

    /// The envelope kind that draws the edge.
    #[must_use]
    pub const fn kind(self) -> crate::feeds::envelope::LimitKind {
        use crate::feeds::envelope::LimitKind;
        match self {
            Self::VendorDepthTable => LimitKind::VendorDepthTable,
            Self::Power => LimitKind::Power,
            Self::Deflection => LimitKind::Deflection,
            Self::ForceBudget => LimitKind::Rigidity,
        }
    }
}

/// A limit that judges a value of each cut and draws no ceiling (P29-7 7c).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValueCheck {
    /// The upper edge of the chip band ([`ChipMaxPlanned`]).
    ChipMax,
    /// The lower edge of the chip band ([`ChipMinPlanned`]).
    ChipMin,
    /// The rubbing floor ([`RubbingFloorPlanned`]).
    RubbingFloor,
    /// P29-8e (c): the vertical rate of a cut against the one vertical-rate
    /// limit ([`VerticalRatePlanned`]).
    VerticalRate,
}

impl ValueCheck {
    /// The limit of the check: its `PlannedLimit::LIMIT`.
    #[must_use]
    pub const fn limit(self) -> LimitId {
        match self {
            Self::ChipMax => ChipMaxPlanned::LIMIT,
            Self::ChipMin => ChipMinPlanned::LIMIT,
            Self::RubbingFloor => RubbingFloorPlanned::LIMIT,
            Self::VerticalRate => VerticalRatePlanned::LIMIT,
        }
    }

    /// The unit of the value and the bound of the check.
    #[must_use]
    pub const fn unit(self) -> &'static str {
        match self {
            Self::ChipMax | Self::ChipMin | Self::RubbingFloor => "mm",
            Self::VerticalRate => "mm/min",
        }
    }
}

/// P29-8e (c): the vertical feed (mm/min) that each entry kind and the
/// drill cut of one op ship, before the simulation: the stated feed of the
/// entry rule (`ConfiguredEntry::planned_feed_mm_min`); for a ramp and a
/// helix its vertical part (the feed x sin of the ramp angle; the feed x
/// pitch / sqrt((2 pi r)^2 + pitch^2) of the helix the door emits, from
/// `ResolvedEntry`); for a drill the op feed. `None`: no number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VerticalFeeds {
    plunge: Option<f64>,
    ramp: Option<f64>,
    helix: Option<f64>,
    peck_ladder: Option<f64>,
    drill: Option<f64>,
}

impl VerticalFeeds {
    /// The vertical feeds of `operation` on `tool` in `material`, with the
    /// configured entry `entry` and the toolpath's dressups (`None`: the
    /// registry default). A tool that does not build gives no ramp and no
    /// helix number.
    #[must_use]
    pub fn of_operation(
        operation: &OperationConfig,
        tool: &ToolConfig,
        material: &Material,
        dressups: Option<&crate::compute::config::DressupConfig>,
        entry: &ConfiguredEntry,
    ) -> Self {
        let resolved = crate::compute::build_cutter(tool).ok().map(|cutter| {
            crate::feeds::entry_rule::ResolvedEntry::of_planned_op(
                operation,
                &cutter,
                tool.diameter,
                material,
                dressups,
            )
        });
        let feed = |kind| entry.planned_feed_mm_min(operation, kind).ok();
        let ramp_share = resolved.map(|r| r.ramp_angle_deg().to_radians().sin());
        let helix_share = resolved.map(|r| {
            let radius = r.helix_radius().emitted_mm;
            let pitch = r.helix_pitch_mm();
            pitch / ((2.0 * std::f64::consts::PI * radius).powi(2) + pitch * pitch).sqrt()
        });
        Self {
            plunge: feed(EntryKind::Plunge),
            ramp: feed(EntryKind::Ramp).zip(ramp_share).map(|(f, k)| f * k),
            helix: feed(EntryKind::Helix).zip(helix_share).map(|(f, k)| f * k),
            peck_ladder: feed(EntryKind::PeckLadder),
            drill: operation.feed_rate(),
        }
    }
}

/// What the vertical-rate check reads beside the cut.
#[derive(Clone, Copy)]
pub struct VerticalEnv<'e> {
    /// THE vertical-rate limit of the op (`PlungeBasis::vertical_limit`).
    pub limit: &'e crate::feeds::vertical::VerticalRateLimit,
    /// The stated spindle speed (rpm), or why none.
    pub rpm: Result<f64, EntryNoNumber>,
    /// The vertical feeds of the op.
    pub feeds: VerticalFeeds,
}

/// The answer of the vertical-rate check for one cut.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VerticalReading {
    /// The cut has no vertical feed into stock.
    NoVerticalFeed,
    /// The cut has a vertical feed and the plan holds no number for it.
    NoNumber,
    /// The vertical feed (mm/min) and THE limit at the stated speed
    /// (`VerticalRateLimit::limit_at`, lead ruling Q11), with its arm;
    /// `limit` is `None` when no arm holds a rate.
    Value {
        /// The vertical feed (mm/min).
        value_mm_min: f64,
        /// The limit (mm/min) and the arm that binds.
        limit: Option<crate::feeds::vertical::VerticalRate>,
    },
}

/// P29-8e (c): the vertical rate of each cut against THE vertical-rate
/// limit at the stated speed (`VerticalRateLimit::limit_at`, lead ruling
/// Q11: the true limit, not the clamped value; a value check, never a
/// ceiling, and no feed moves). Every straight plunge that ships is the plunge entry cut (the
/// stated plunge of the style None; the fallback plunge, the K1 backstop
/// re-entry and the surface-riding re-entry under a ramp or helix style),
/// so the check judges each of them.
pub struct VerticalRatePlanned;

// P29-11 section 13, ruling 1: no sample gate judges this limit after the
// simulation (`tool_load::groups::gate_of` gives none).
// EXEMPT(S-NoSampleGate)
impl PlannedLimit for VerticalRatePlanned {
    const LIMIT: LimitId = LimitId::PlungeRate;
    type Env<'e> = VerticalEnv<'e>;
    type Answer = VerticalReading;
    fn judge(cut: &PlannedCut, env: &VerticalEnv<'_>) -> VerticalReading {
        let feeds = env.feeds;
        let value = match cut.kind {
            CutKind::Entry(EntryKind::Plunge) => feeds.plunge,
            CutKind::Entry(EntryKind::Ramp) => feeds.ramp,
            CutKind::Entry(EntryKind::Helix) => feeds.helix,
            CutKind::Entry(EntryKind::PeckLadder) => feeds.peck_ladder,
            CutKind::Drill => feeds.drill,
            // A side cut, a link and a relief move in XY; an air lead
            // removes no stock.
            CutKind::Body
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
            | CutKind::DressupRelief => return VerticalReading::NoVerticalFeed,
        };
        match (value, env.rpm) {
            (Some(v), Ok(rpm)) if v.is_finite() && v > 0.0 => VerticalReading::Value {
                value_mm_min: v,
                limit: env.limit.limit_at(rpm),
            },
            (Some(_) | None, Ok(_) | Err(_)) => VerticalReading::NoNumber,
        }
    }
}

/// The role of a limit on the envelope plane (P29-7 7c). The envelope reads
/// every limit through one loop over `LimitId::ALL` and this role: a depth
/// ceiling draws its edge, names its binding cut and writes one check per
/// declared cut; a value check writes one check per declared cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaneRole {
    /// A depth ceiling over the width, the lowest over the declared cuts.
    DepthCeiling(CeilingEdge),
    /// A value of each declared cut against a bound.
    ValueCheck(ValueCheck),
    /// Not a limit of each cut (`limits::scope` answers the same reason on
    /// every group).
    NotACutLimit(PlanReason),
}

impl PlaneRole {
    /// The limit of a planned role, or `None` for a limit that is not a
    /// limit of each cut.
    #[must_use]
    pub const fn limit(self) -> Option<LimitId> {
        match self {
            Self::DepthCeiling(edge) => Some(edge.limit()),
            Self::ValueCheck(check) => Some(check.limit()),
            Self::NotACutLimit(_) => None,
        }
    }
}

/// The role of `limit` on the envelope plane. One match names every limit
/// and has no wildcard arm: a new limit does not compile until it states
/// its role, and the envelope then reads it with no other edit (it draws a
/// new `CeilingEdge` or writes a new `ValueCheck` through one match each).
/// The sentry `every_limit_judges_every_declared_cut_p29_7` holds that a
/// limit that `limits::scope` judges on a group has a planned role that
/// names the limit, and that a `NotACutLimit` role states the reason of the
/// scope.
#[must_use]
pub const fn planned_limit_of(limit: LimitId) -> PlaneRole {
    match limit {
        LimitId::ForceBudget => PlaneRole::DepthCeiling(CeilingEdge::ForceBudget),
        LimitId::ToolDeflection => PlaneRole::DepthCeiling(CeilingEdge::Deflection),
        LimitId::VendorDepthTable => PlaneRole::DepthCeiling(CeilingEdge::VendorDepthTable),
        LimitId::SpindlePower => PlaneRole::DepthCeiling(CeilingEdge::Power),
        LimitId::ChipMax => PlaneRole::ValueCheck(ValueCheck::ChipMax),
        LimitId::ChipMin => PlaneRole::ValueCheck(ValueCheck::ChipMin),
        LimitId::RubbingFloor => PlaneRole::ValueCheck(ValueCheck::RubbingFloor),
        // P29-8e (c): the vertical rate of each cut.
        LimitId::PlungeRate => PlaneRole::ValueCheck(ValueCheck::VerticalRate),
        LimitId::FluteGuard | LimitId::CuttingLength | LimitId::VendorAxial => {
            PlaneRole::NotACutLimit(PlanReason::DepthOnly)
        }
        LimitId::CornerCoverage
        | LimitId::ScallopDepth
        | LimitId::ToolDiameter
        | LimitId::MoveBudget
        | LimitId::EngagementFloor => PlaneRole::NotACutLimit(PlanReason::PlanLevel),
        LimitId::CuttingFeedCap
        | LimitId::PlannerFeedCap
        | LimitId::KinematicReach
        | LimitId::SpindleMaxRpm
        | LimitId::ZAxisRate
        | LimitId::OperatorRpmCeiling
        | LimitId::ChartRpm
        | LimitId::VendorRpmMax
        | LimitId::GantryPush => PlaneRole::NotACutLimit(PlanReason::MachineOrRpm),
        LimitId::DrillChipWelding | LimitId::DrillPeckAdequacy | LimitId::DrillPlungeFeed => {
            PlaneRole::NotACutLimit(PlanReason::DrillGate)
        }
    }
}
