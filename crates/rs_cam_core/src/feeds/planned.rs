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
//!    value over the judged cuts with `Presence::Always`
//!    ([`ByCut::lowest_always`]). A cut with `Presence::MayOccur` is judged
//!    too, but it gives a check row and a note, never a ceiling (lead ruling
//!    Q1): its exemption row names it.
//!
//! To add a limit before the simulation: add a `LimitId` (the compiler asks
//! `limits::scope`, [`planned_limit_of`] and `tool_load::groups::gate_of`),
//! write one `impl PlannedLimit`, and read its edge through the driver.

use crate::compute::catalog::{
    CutList, DeclaredCut, DeclaredDepth, DeclaredFeed, DeclaredWidth, MAX_DECLARED_CUTS,
    OperationConfig, PlannerWidth, Presence,
};
use crate::compute::tool_config::ToolConfig;
use crate::feeds::CutBody;
use crate::feeds::engagement::{EngagedDiameter, PowerCase};
use crate::feeds::force_budget::BudgetDepth;
use crate::feeds::geometry::{RadialThinning, UnliftedFeed};
use crate::limits::{CutGroup, CutKind, LimitId, NoEngagementModel, NotJudged, PlanReason, Scope};
use crate::material::Material;
use crate::tool::ToolDefinition;

/// Why a declared cut has no chip before the simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlannedNoNumber {
    /// The planner caps the feed on its own stock read
    /// (`DeclaredFeed::PlannerCapped`).
    PlannerCapped,
    /// An entry rule sets the feed (P29-8).
    EntryRule,
    /// The drill cycle sets the feed (Step 9c).
    DrillCycle,
    /// The column states no body chip (the power fold reads the width
    /// only).
    NoBodyChip,
}

/// One declared cut at one body column of the envelope plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlannedCut {
    /// The kind.
    pub kind: CutKind,
    /// The group of the kind (`CutKind::group`).
    pub group: CutGroup,
    /// The width of the cut (mm), or why the cut has none on this plane.
    pub width_mm: Result<f64, NoEngagementModel>,
    /// The advance per tooth of the cut (mm), or why it has none.
    pub chip_mm: Result<f64, PlannedNoNumber>,
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
}

/// The declared cuts of one op on one tool, with the facts that each width
/// column reads. Build it once per envelope ([`Self::of_operation`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeclaredCuts {
    list: CutList,
    d_mm: f64,
    body_depth: Option<DeclaredDepth>,
    thinning: RadialThinning,
    lift_d_eng_mm: f64,
}

impl DeclaredCuts {
    /// The cuts of `operation` on `tool`. The lift of the unlifted feed is
    /// read at the stated depth per pass (else the tool diameter), the rule
    /// of `UnliftedFeed::of_slot_ring`.
    #[must_use]
    pub fn of_operation(operation: &OperationConfig, tool: &ToolConfig) -> Self {
        let list = operation.cuts();
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
        Self {
            list,
            d_mm,
            body_depth: list.of_kind(CutKind::Body).map(|c| c.depth),
            thinning: list.body_facts().thins_chip,
            lift_d_eng_mm,
        }
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
    /// width of the column, on the engaged diameter of the lift); the
    /// planner-capped, entry and drill feeds give no number.
    #[must_use]
    pub fn planned(&self, cut: &DeclaredCut, column: BodyColumn) -> PlannedCut {
        let d = self.d_mm;
        let ae = column.ae_mm;
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
            | CutKind::Drill => {
                if self.body_depth.is_some_and(|body| body != cut.depth) {
                    Err(NoEngagementModel::NoStatedDepth)
                } else {
                    match cut.width {
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
                    }
                }
            }
        };
        let body_chip = column.chip_mm.ok_or(PlannedNoNumber::NoBodyChip);
        let chip_mm = match cut.feed {
            DeclaredFeed::OpFeed => body_chip,
            DeclaredFeed::Unlifted => body_chip.map(|fz| {
                let lift = self
                    .thinning
                    .lift(self.list.full_feed_width(ae, d), self.lift_d_eng_mm);
                UnliftedFeed::over_lift(fz, lift).mm_min()
            }),
            DeclaredFeed::PlannerCapped => Err(PlannedNoNumber::PlannerCapped),
            DeclaredFeed::EntryRule => Err(PlannedNoNumber::EntryRule),
            DeclaredFeed::DrillCycle => Err(PlannedNoNumber::DrillCycle),
        };
        PlannedCut {
            kind: cut.kind,
            group: cut.kind.group(),
            width_mm,
            chip_mm,
            presence: cut.presence,
        }
    }
}

/// **A limit before the simulation**: one function over one planned cut.
/// The driver [`judge_declared_cuts`] is its only caller.
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
    /// The lowest `value` over the judged cuts with `Presence::Always`, and
    /// the cut that gives it. `value` gives `None` for an answer with no
    /// number. A tie keeps the first cut (the body is first). `None` when no
    /// such cut has a number.
    #[must_use]
    pub fn lowest_always(&self, value: impl Fn(&A) -> Option<f64>) -> Option<(CutKind, A)> {
        let mut best: Option<(f64, CutKind, A)> = None;
        for entry in self.iter() {
            let (Presence::Always, PlannedCell::Judged(answer)) = (entry.presence, entry.value)
            else {
                continue;
            };
            let Some(v) = value(&answer) else {
                continue;
            };
            if best.is_none_or(|(b, _, _)| v < b) {
                best = Some((v, entry.kind, answer));
            }
        }
        best.map(|(_, kind, answer)| (kind, answer))
    }
}

/// **The one driver.** For each declared cut of `cuts` at `column`: the
/// scope cell of `L` on the group of the cut; a judged cell calls `L`.
pub fn judge_declared_cuts<L: PlannedLimit>(
    cuts: &DeclaredCuts,
    column: BodyColumn,
    env: &L::Env<'_>,
) -> ByCut<PlannedCell<L::Answer>> {
    ByCut::from_fn(&cuts.list, |cut| {
        let planned = cuts.planned(cut, column);
        match crate::limits::scope(L::LIMIT, planned.group) {
            Scope::Judged => PlannedCell::Judged(L::judge(&planned, env)),
            Scope::NotJudged(cell) => PlannedCell::NotJudged(cell),
            Scope::NotACutLimit(reason) => PlannedCell::NotACutLimit(reason),
        }
    })
}

/// The width and the chip of a planned cut, when it has both.
fn width_and_chip(cut: &PlannedCut) -> Option<(f64, f64)> {
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
    /// The power (kW) of a cut at a width (mm), at the op feed.
    pub power_kw: &'e dyn Fn(f64) -> f64,
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
        Some((case, (env.power_kw)(width), width))
    }
}

/// The worst case of the pre-sim power set at the body width `ae`: the
/// largest power over the declared cuts with `Presence::Always`, its case
/// and its width. A tie goes to the more specific case
/// (`PowerCase::tie_rank`). With no judged cut, the nominal case at `ae`.
pub fn worst_power(
    cuts: &DeclaredCuts,
    ae: f64,
    power_kw: impl Fn(f64) -> f64,
) -> (PowerCase, f64, f64) {
    let env = PowerEnv {
        column_ae_mm: ae,
        power_kw: &power_kw,
    };
    let column = BodyColumn {
        ae_mm: ae,
        chip_mm: None,
    };
    let cells = judge_declared_cuts::<SpindlePowerPlanned>(cuts, column, &env);
    let mut best: Option<(PowerCase, f64, f64)> = None;
    for entry in cells.iter() {
        let (Presence::Always, PlannedCell::Judged(Some(this))) = (entry.presence, entry.value)
        else {
            continue;
        };
        let wins = best
            .is_none_or(|b| this.1 > b.1 || (this.1 >= b.1 && this.0.tie_rank() > b.0.tie_rank()));
        if wins {
            best = Some(this);
        }
    }
    best.unwrap_or_else(|| (PowerCase::Nominal, power_kw(ae), ae))
}

/// The width (mm) of the power case `case` at the body width `ae`: the
/// width of the first declared cut with `Presence::Always` of that case,
/// else `ae`.
#[must_use]
pub fn power_case_width(cuts: &DeclaredCuts, case: PowerCase, ae: f64) -> f64 {
    let none = |_: f64| 0.0;
    let env = PowerEnv {
        column_ae_mm: ae,
        power_kw: &none,
    };
    let column = BodyColumn {
        ae_mm: ae,
        chip_mm: None,
    };
    judge_declared_cuts::<SpindlePowerPlanned>(cuts, column, &env)
        .iter()
        .find_map(|entry| match (entry.presence, entry.value) {
            (Presence::Always, PlannedCell::Judged(Some((c, _, w)))) if c == case => Some(w),
            (Presence::Always | Presence::MayOccur(_), _) => None,
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
    /// The bound (mm per tooth): the band maximum, the band minimum or the
    /// rubbing floor.
    pub bound_mm: f64,
}

/// The planned peak chip of one cut against one chip bound.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChipReading {
    /// The peak chip of the cut (mm): its advance per tooth over the lift
    /// at its width.
    pub peak_mm: f64,
    /// The bound (mm).
    pub bound_mm: f64,
}

fn peak_chip(cut: &PlannedCut, env: &ChipEnv) -> Option<ChipReading> {
    let (w, fz) = width_and_chip(cut)?;
    let lift = env.thinning.lift(w, env.d_eng_mm);
    Some(ChipReading {
        peak_mm: UnliftedFeed::over_lift(fz, lift).mm_min(),
        bound_mm: env.bound_mm,
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
        peak_chip(cut, env)
    }
}

/// The lower edge of the printed chip band (a value check, no ceiling).
pub struct ChipMinPlanned;

impl PlannedLimit for ChipMinPlanned {
    const LIMIT: LimitId = LimitId::ChipMin;
    type Env<'e> = ChipEnv;
    type Answer = Option<ChipReading>;
    fn judge(cut: &PlannedCut, env: &ChipEnv) -> Option<ChipReading> {
        peak_chip(cut, env)
    }
}

/// The rubbing floor (a value check, no ceiling; it warns, never lifts).
pub struct RubbingFloorPlanned;

impl PlannedLimit for RubbingFloorPlanned {
    const LIMIT: LimitId = LimitId::RubbingFloor;
    type Env<'e> = ChipEnv;
    type Answer = Option<ChipReading>;
    fn judge(cut: &PlannedCut, env: &ChipEnv) -> Option<ChipReading> {
        peak_chip(cut, env)
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
}

impl ValueCheck {
    /// The limit of the check: its `PlannedLimit::LIMIT`.
    #[must_use]
    pub const fn limit(self) -> LimitId {
        match self {
            Self::ChipMax => ChipMaxPlanned::LIMIT,
            Self::ChipMin => ChipMinPlanned::LIMIT,
            Self::RubbingFloor => RubbingFloorPlanned::LIMIT,
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
        | LimitId::PlungeRate
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
