//! Machine profile definitions for feeds & speeds calculation.
//!
//! Defines spindle configuration, power model, rigidity factors, and
//! machine presets. Ported from reference/shapeoko_feeds_and_speeds/src/machine_profile.rs.
//!
//! The folder holds the machine model: this profile, the kinematics, the
//! utilisation instrument and the strategy advisor that reads them.

pub mod domain;
pub mod head;
pub mod kinematic_utilization;
pub mod kinematics;
pub mod strategy_advisor;

use serde::{Deserialize, Serialize};

use crate::machine::kinematics::MachineKinematics;

/// Spindle speed control type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum SpindleConfig {
    Variable { min_rpm: f64, max_rpm: f64 },
    Discrete { speeds: Vec<f64> },
}

/// Spindle power model.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum PowerModel {
    /// VFD: constant torque up to rated RPM, power scales linearly with RPM.
    VfdConstantTorque { rated_power_kw: f64, rated_rpm: f64 },
    /// Router-type: roughly constant power across RPM range.
    ConstantPower { power_kw: f64 },
}

/// Chip load formula parameters derived from empirical data.
/// ChipLoad = K0 * D^p * (1/H)^q
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChipLoadFormula {
    pub k0: f64, // base coefficient
    pub p: f64,  // diameter exponent
    pub q: f64,  // hardness exponent
}

impl Default for ChipLoadFormula {
    fn default() -> Self {
        // Soft wood baseline from Shapeoko empirical data
        Self {
            k0: 0.024,
            p: 0.61,
            q: 1.26,
        }
    }
}

/// Machine-specific DOC/WOC parameters reflecting rigidity.
///
/// Feeds contract P11c (D13 items 3 and 6): the one default set is
/// [`RigidityProfile::default`], the hobby grade set ([`MachineClass::Hobby`]).
/// `woc_roughing_factor`, `woc_roughing_max_mm` and `woc_finishing_mm` are
/// deleted (no reader; break; review item M11). P16 (operator ruling D12):
/// `slot_doc_factor`, the 0.25 D slot depth cap, is deleted (a retired key);
/// the force budget sets the slot depth.
///
/// Feeds contract P13c (ruling O1): `force_budget_n` replaces the typed
/// `doc_roughing_factor` (0.25 D) and the unread `doc_finishing_factor`
/// (break). The roughing depth of the Pocket and Profile families is the
/// depth where the peak engaged-edge force meets the budget
/// (`feeds::force_budget::depth_at_budget`). A file reads through
/// [`RigidityProfileWire`] inside [`MachineProfileWire`]: the class rule
/// gives a hobby table with no budget the derived hobby budget.
///
/// Feeds contract P29-4 commit C: `adaptive_doc_factor` is deleted (a
/// retired key). The force budget caps the Adaptive depth (ruling R1), so
/// no reader of the factor stays.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RigidityProfile {
    pub adaptive_woc_factor: f64,
    /// P13c (O1): the lateral force budget (N), a stand-in for the frame
    /// stiffness (frame deflection = F / k_frame). The hobby grade default
    /// is the largest peak force of the printed hobby full-slot cuts
    /// (`feeds::force_budget::hobby_budget`).
    pub force_budget_n: f64,
}

/// The file form of a [`RigidityProfile`] (P13c): `force_budget_n` may be
/// absent; [`MachineProfile::try_from`] applies the class rule (hobby: the
/// derived hobby budget; industrial: refuse).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RigidityProfileWire {
    adaptive_woc_factor: f64,
    #[serde(default)]
    force_budget_n: Option<f64>,
}

/// One named rigidity value, for a surface that lists them (the GUI
/// machine panel). The order is the field order of [`RigidityProfile`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigidityValue {
    /// The field name, as the machine file spells it.
    pub field: &'static str,
    /// The value.
    pub value: f64,
    /// The unit: `"x D"`, a factor of the diameter, or `"N"`.
    pub unit: &'static str,
}

impl RigidityProfile {
    /// Each value with its field name and unit (P11c: the GUI machine
    /// panel lists them, with the class beside each value).
    #[must_use]
    pub const fn values(&self) -> [RigidityValue; 2] {
        [
            RigidityValue {
                field: "adaptive_woc_factor",
                value: self.adaptive_woc_factor,
                unit: "x D",
            },
            RigidityValue {
                field: "force_budget_n",
                value: self.force_budget_n,
                unit: "N",
            },
        ]
    }
}

/// The class of a machine (feeds contract P11c, D13 item 6, operator
/// 2026-11-05: "make it explicit that this number is 'hobby grade' vs
/// 'industrial'"). The machine table states it; no default.
///
/// - [`Self::Hobby`] has the one default rigidity set
///   ([`RigidityProfile::default`]: the Shapeoko port, Unsourced). A hobby
///   table with no `[rigidity]` block reads it.
/// - [`Self::Industrial`] carries no values: its table states its own
///   `[rigidity]` block, else the table refuses to load.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MachineClass {
    /// A hobby grade router (the Shapeoko class).
    Hobby,
    /// An industrial router. No default values.
    Industrial,
}

impl MachineClass {
    /// The name that the GUI, MCP and the CLI print.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Hobby => "hobby grade",
            Self::Industrial => "industrial",
        }
    }
}

/// How the rigidity values of one profile are known: the class and
/// whether the values are the class default. Built by
/// [`MachineProfile::rigidity_basis`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RigidityBasis {
    /// The hobby grade default set (the Shapeoko port), Unsourced.
    HobbyDefault,
    /// Values that the machine file states, on a hobby grade machine.
    HobbyStated,
    /// Values that the machine file states, on an industrial machine.
    IndustrialStated,
}

impl RigidityBasis {
    /// The text that the GUI machine panel, MCP, the CLI and the K8
    /// lineage print beside each rigidity value (the law of the
    /// `machine.rigidity` basis).
    #[must_use]
    pub const fn text(self) -> &'static str {
        match self {
            Self::HobbyDefault => {
                "hobby grade (Shapeoko port, Unsourced; force budget from the hobby slot charts)"
            }
            Self::HobbyStated => "hobby grade, stated by the machine file",
            Self::IndustrialStated => "industrial, stated by the machine file",
        }
    }

    /// The short face of [`Self::text`] for a narrow panel (IC1: a text
    /// over 60 characters takes a short face, the full text on hover).
    #[must_use]
    pub const fn short(self) -> &'static str {
        match self {
            Self::HobbyDefault => "hobby grade default",
            Self::HobbyStated => "hobby grade, stated",
            Self::IndustrialStated => "industrial, stated",
        }
    }

    /// The machine class of this basis.
    #[must_use]
    pub const fn class(self) -> MachineClass {
        match self {
            Self::HobbyDefault | Self::HobbyStated => MachineClass::Hobby,
            Self::IndustrialStated => MachineClass::Industrial,
        }
    }
}

/// **The axial depth cap one rigidity profile implies for one body
/// cut.** S3 (2026-09-18), P13c (O1).
///
/// The cap carries its inputs, so the bound and the provenance beside it
/// cannot disagree ([`Self::cap_mm`]). `feeds::force_budget::rigidity_depth_cap`
/// is the one producer; the envelope `Rigidity` limit and the post-sim
/// depth gate read it. P29-4 deleted the Adaptive factor rule; the enum
/// keeps its `kind` tag on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RigidityDepthCap {
    /// Every roughing cut (P13c, O1; the Adaptive family since P29-4) and
    /// every slot or groove (P29-5, R3): the shallowest depth where the peak engaged-edge force at `ae_mm`
    /// and `fz_mm` meets `budget_n`.
    ForceBudget {
        /// The machine force budget (N).
        budget_n: f64,
        /// The width the depth is solved at (mm).
        ae_mm: f64,
        /// The advance per tooth the depth is solved at (mm).
        fz_mm: f64,
        /// The solved depth (mm), or the minimum depth when `below_floor`.
        depth_mm: f64,
        /// The peak at the minimum depth is already over the budget: the
        /// minimum depth ships with a warning (Q6).
        below_floor: bool,
    },
}

impl RigidityDepthCap {
    /// The cap itself, in mm.
    #[must_use]
    pub fn cap_mm(&self) -> f64 {
        match *self {
            Self::ForceBudget { depth_mm, .. } => depth_mm,
        }
    }
}

/// The rule of the rigidity depth cap of one body cut
/// ([`RigidityProfile::depth_cap_of`]): the machine force budget. Every
/// cut whose depth limit is `DepthLimit::ForceBudget` reads it (P29-4,
/// ruling Q4: one named field; P29-5, ruling R3: a slot or a groove too).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DepthCapRule {
    /// The machine force budget (N).
    pub budget_n: f64,
}

impl RigidityProfile {
    /// **The depth limit of one body cut, with no number.** S3
    /// (2026-09-18), P13c (O1), P29-3, P29-5.
    ///
    /// | Family / role / sides | Limit |
    /// |---|---|
    /// | `Drill` | none: a drill cycle is Z-only |
    /// | any other, `Roughing` | the force budget |
    /// | any other, `SemiFinish` or `Finish`, `Both` | the force budget |
    /// | any other, `SemiFinish` or `Finish`, `One` | none (R2) |
    ///
    /// Feeds matrix R2 (2026-09-23): no vendor publishes an axial
    /// finishing cap for wood; the deflection gate is the limit of a
    /// one-sided finish pass. Ruling R3 (2026-11-11, "slot is a slot!"):
    /// a slot or a groove has material on both sides, so it reads the
    /// force budget under every pass role. P29-4 (ruling R1): the Adaptive
    /// family has no arm of its own.
    ///
    /// `OperationType::body_depth_limit` is the one caller: it reads the
    /// three inputs from the registry row. [`Self::depth_cap_of`] adds the
    /// numbers.
    #[must_use]
    pub const fn depth_limit(
        family: crate::feeds::OperationFamily,
        pass_role: crate::feeds::PassRole,
        sides: crate::limits::BodySides,
    ) -> crate::limits::DepthLimit {
        use crate::feeds::{OperationFamily, PassRole};
        use crate::limits::{BodySides, DepthLimit, NoDepthLimit};
        match (family, pass_role, sides) {
            // EXEMPT(P28-04)
            (OperationFamily::Drill, _, _) => DepthLimit::NoLimit(NoDepthLimit::DrillCycle),
            (_, PassRole::Roughing, _) => DepthLimit::ForceBudget,
            // R3: a slot is a slot under every op.
            (_, PassRole::SemiFinish | PassRole::Finish, BodySides::Both) => {
                DepthLimit::ForceBudget
            }
            // EXEMPT(P28-02, S-NoBound)
            (_, PassRole::SemiFinish | PassRole::Finish, BodySides::One) => {
                DepthLimit::NoLimit(NoDepthLimit::FinishPass)
            }
        }
    }

    /// P29-3: the rule of the axial depth cap for the depth limit of a cut
    /// (`limits::CutFacts::depth_rule`), with this machine's numbers.
    /// `None` when the cut has no depth limit.
    #[must_use]
    pub const fn depth_cap_of(&self, limit: crate::limits::DepthLimit) -> Option<DepthCapRule> {
        use crate::limits::{DepthLimit, NoDepthLimit};
        match limit {
            DepthLimit::ForceBudget => Some(DepthCapRule {
                budget_n: self.force_budget_n,
            }),
            DepthLimit::NoLimit(NoDepthLimit::DrillCycle | NoDepthLimit::FinishPass) => None,
        }
    }
}

/// The one default rigidity set: hobby grade ([`MachineClass::Hobby`]).
///
/// Feeds contract P11c (D13 item 6, ruled 2026-11-05: "let the default be
/// about shapeoko"). The values are a port of
/// `shapeoko_vfd_1500w_balanced()` in the gitignored
/// `reference/shapeoko_feeds_and_speeds/src/machine_profile.rs` (commit
/// 172d6de60); no printed source is in the repo, so each value is
/// Unsourced ([`RigidityBasis::HobbyDefault`]). P13c (O1): `force_budget_n` is derived, not
/// ported: the largest peak force of the printed hobby full-slot cuts
/// (`feeds::force_budget::hobby_budget`, F201 Cited). `generic_wood_router`
/// and the two Shapeoko presets read this set; a hobby machine table with
/// no `[rigidity]` block reads it too.
impl Default for RigidityProfile {
    fn default() -> Self {
        Self {
            adaptive_woc_factor: 0.25,
            force_budget_n: crate::feeds::force_budget::hobby_budget().force_n,
        }
    }
}

/// F002: the machine TRAVEL rate, mm/min — what rapids, links and the
/// kinematics cruise cap can command. Only [`MachineProfile::travel_rate`]
/// makes one; the field is private.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct TravelRateMmMin(f64);

impl TravelRateMmMin {
    /// The rate, mm/min.
    #[must_use]
    pub const fn mm_min(self) -> f64 {
        self.0
    }
}

/// F002: the ceiling of a CUTTING feed, mm/min (P16, operator ruling D9):
/// `max_cutting_feed_mm_min` when stated, else the smaller of the X and Y
/// rates of the kinematics (`$110`, `$111`), else the travel rate. It is
/// never above the travel rate. Only [`MachineProfile::cutting_ceiling`]
/// makes one; the fields are private, so a travel rate cannot pose as a
/// cutting cap. [`Self::capped_at`] can only lower it (G-FEEDMAX).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CuttingCeilingMmMin {
    mm_min: f64,
    source: CuttingCeilingSource,
}

/// P16 (D9): what set a [`CuttingCeilingMmMin`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CuttingCeilingSource {
    /// The machine states `max_cutting_feed_mm_min`.
    Stated,
    /// The smaller of the X and Y maximum rates of the kinematics.
    AxisRate,
    /// No cutting cap is stated: the travel rate `max_feed_mm_min` bounds
    /// the cut (a cut cannot outrun the axes). Step 7 warns
    /// (`FeedsWarning::CuttingCapUnstated`) when this ceiling binds.
    TravelRate,
    /// The op's own feed cap (`DressupConfig::feed_max_rate`) lowered the
    /// machine ceiling ([`CuttingCeilingMmMin::capped_at`]).
    OpFeedCap,
}

impl CuttingCeilingSource {
    /// The wire id (MCP `inspect_machine`).
    #[must_use]
    pub const fn wire_id(self) -> &'static str {
        match self {
            Self::Stated => "stated",
            Self::AxisRate => "axis_rate",
            Self::TravelRate => "travel_rate",
            Self::OpFeedCap => "op_feed_cap",
        }
    }

    /// A short operator name of the source (P21c: the binding limit of a
    /// constant-chip RPM, `feeds::RpmLimit::CuttingCeiling`).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Stated => "cutting feed cap",
            Self::AxisRate => "X/Y axis rate",
            Self::TravelRate => "travel rate",
            Self::OpFeedCap => "op feed cap (feed_max_rate)",
        }
    }
}

impl CuttingCeilingMmMin {
    /// The ceiling, mm/min.
    #[must_use]
    pub const fn mm_min(self) -> f64 {
        self.mm_min
    }

    /// What set the ceiling.
    #[must_use]
    pub const fn source(self) -> CuttingCeilingSource {
        self.source
    }

    /// The ceiling with a floor of `floor_mm_min` (the `.max(1.0)` guard the
    /// sim and the advisor take against a zero profile).
    #[must_use]
    pub const fn at_least(self, floor_mm_min: f64) -> Self {
        Self {
            mm_min: self.mm_min.max(floor_mm_min),
            source: self.source,
        }
    }

    /// G-FEEDMAX: the ceiling, lowered to the op's own cut-feed cap
    /// (`DressupConfig::feed_max_rate`) when that is set, finite and
    /// above zero. It only lowers, so the result is still no faster than
    /// the machine: a cap above the machine ceiling changes nothing.
    #[must_use]
    pub fn capped_at(self, cap_mm_min: Option<f64>) -> Self {
        match cap_mm_min {
            Some(cap) if cap.is_finite() && cap > 0.0 && cap < self.mm_min => Self {
                mm_min: cap,
                source: CuttingCeilingSource::OpFeedCap,
            },
            _ => self,
        }
    }

    /// P20e: the feed an op with the stated feed `op_feed_mm_min` commands
    /// in steady cutting under this ceiling: `min(op feed, ceiling)`, with
    /// the limit that binds. A tie is [`SteadyFeedLimit::OpFeed`] (no cap
    /// binds). Build the ceiling with
    /// [`MachineProfile::op_cutting_ceiling`], the cap the modulator reads.
    #[must_use]
    pub fn steady_feed(self, op_feed_mm_min: f64) -> SteadyFeedMmMin {
        if op_feed_mm_min <= self.mm_min {
            return SteadyFeedMmMin {
                mm_min: op_feed_mm_min,
                op_feed_mm_min,
                limit: SteadyFeedLimit::OpFeed,
            };
        }
        let limit = match self.source {
            CuttingCeilingSource::OpFeedCap => SteadyFeedLimit::OpFeedCap,
            source @ (CuttingCeilingSource::Stated
            | CuttingCeilingSource::AxisRate
            | CuttingCeilingSource::TravelRate) => SteadyFeedLimit::MachineCeiling(source),
        };
        SteadyFeedMmMin {
            mm_min: self.mm_min,
            op_feed_mm_min,
            limit,
        }
    }
}

/// P20e: the limit that sets a [`SteadyFeedMmMin`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteadyFeedLimit {
    /// The op feed (`feed_rate`): no cap is below it.
    OpFeed,
    /// The op's own feed cap (`DressupConfig::feed_max_rate`).
    OpFeedCap,
    /// The machine cutting ceiling, with what set it.
    MachineCeiling(CuttingCeilingSource),
}

impl SteadyFeedLimit {
    /// The wire id (MCP `get_suggest_rationale`, CLI `suggest`).
    #[must_use]
    pub const fn wire_id(self) -> &'static str {
        match self {
            Self::OpFeed => "op_feed",
            Self::OpFeedCap => "op_feed_cap",
            Self::MachineCeiling(_) => "machine_ceiling",
        }
    }

    /// A short operator name of the limit.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::OpFeed => "the op feed",
            Self::OpFeedCap => "the op feed cap (feed_max_rate)",
            Self::MachineCeiling(_) => "the machine cutting ceiling",
        }
    }
}

/// P20e: the feed (mm/min) an op commands in steady cutting:
/// `min(op feed, op feed cap, machine cutting ceiling)`. Only
/// [`CuttingCeilingMmMin::steady_feed`] and [`Self::op_feed_only`] make
/// one. The chip gate filter, the histogram and the Suggest power figure
/// read it; the modulator caps at the same ceiling
/// ([`MachineProfile::op_cutting_ceiling`]), so the two cannot diverge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SteadyFeedMmMin {
    mm_min: f64,
    op_feed_mm_min: f64,
    limit: SteadyFeedLimit,
}

impl SteadyFeedMmMin {
    /// The steady feed with no cap below the op feed: the op feed binds.
    /// The door of a gate context that a test builds by hand.
    #[must_use]
    pub const fn op_feed_only(op_feed_mm_min: f64) -> Self {
        Self {
            mm_min: op_feed_mm_min,
            op_feed_mm_min,
            limit: SteadyFeedLimit::OpFeed,
        }
    }

    /// The steady feed, mm/min.
    #[must_use]
    pub const fn mm_min(self) -> f64 {
        self.mm_min
    }

    /// The stated op feed, mm/min.
    #[must_use]
    pub const fn op_feed_mm_min(self) -> f64 {
        self.op_feed_mm_min
    }

    /// The limit that binds.
    #[must_use]
    pub const fn limit(self) -> SteadyFeedLimit {
        self.limit
    }
}

/// Complete machine profile for feeds calculation.
///
/// P11c (D13 item 6): a file reads through [`MachineProfileWire`], the one
/// place of the class rule: `class` is required; a hobby table with no
/// `[rigidity]` block reads [`RigidityProfile::default`]; an industrial
/// table with no `[rigidity]` block refuses to load.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "MachineProfileWire")]
pub struct MachineProfile {
    pub name: String,
    pub spindle: SpindleConfig,
    pub power: PowerModel,
    pub chip_load: ChipLoadFormula,
    /// Machine TRAVEL rate ($110-class) — what rapids, linking moves,
    /// and the kinematics integrator can command. F4 (2026-06-10):
    /// this is NOT the cutting ceiling. Pre-F4 the optimizer / suggest
    /// / modulation all read this field as the cutting-feed cap, so a
    /// 10 000 mm/min-travel profile could be told to CUT hardwood at
    /// 10 000. Cutting consumers read
    /// [`Self::cutting_feed_ceiling_mm_min`] or the typed
    /// [`Self::cutting_ceiling`]; a travel consumer can read the typed
    /// [`Self::travel_rate`] (F002). (Field name kept: the project files
    /// and machine-library TOMLs carry it, and the machine panel edits it.)
    pub max_feed_mm_min: f64,
    /// F4 — explicit ceiling for CUTTING feeds (optimizer search
    /// space, suggest recalibration, adaptive feed modulation).
    /// `None` (P16, D9): the smaller of the kinematics X and Y rates,
    /// else the travel rate, with a warning where it binds
    /// ([`Self::cutting_ceiling`]).
    #[serde(default)]
    pub max_cutting_feed_mm_min: Option<f64>,
    pub max_shank_mm: f64,
    /// P11c (D13 item 6): the machine class. Required in a file.
    pub class: MachineClass,
    pub rigidity: RigidityProfile,
    /// Linear-axis kinematics limits used by the F-034 cycle-time
    /// integrator. **`None` for every built-in preset** — the absence
    /// of kinematics IS the feature flag for F-034. When `Some`, the
    /// simulator routes `total_runtime_s` through
    /// [`crate::machine::kinematics::compute_cycle_time`] instead of
    /// the naive dexel-sample sum. When `None`, behavior is byte-
    /// identical to pre-F-034.
    #[serde(default)]
    pub kinematics: Option<MachineKinematics>,
    /// What the controller firmware reports about itself (G10): the
    /// dialect and the settings the export reads. `None` until a `$$`
    /// dump is imported. Every built-in preset ships `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controller: Option<ControllerSettings>,
    /// H1: the head above the holder face and the dust shoe. A file
    /// without the table reads [`head::MachineHead::unmodelled`], the band
    /// every tool had before H1; the writer leaves that head out, so an
    /// old file saves as it loaded.
    #[serde(default, skip_serializing_if = "head::MachineHead::is_unmodelled")]
    pub head: head::MachineHead,
}

/// The file form of a [`MachineProfile`] (P11c): the same keys, with the
/// `[rigidity]` block optional. [`MachineProfile::try_from`] applies the
/// class rule.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineProfileWire {
    name: String,
    spindle: SpindleConfig,
    power: PowerModel,
    chip_load: ChipLoadFormula,
    max_feed_mm_min: f64,
    #[serde(default)]
    max_cutting_feed_mm_min: Option<f64>,
    max_shank_mm: f64,
    class: MachineClass,
    #[serde(default)]
    rigidity: Option<RigidityProfileWire>,
    #[serde(default)]
    kinematics: Option<MachineKinematics>,
    #[serde(default)]
    controller: Option<ControllerSettings>,
    #[serde(default)]
    head: head::MachineHead,
}

/// The refusal of an industrial machine table with no `[rigidity]` block.
pub const INDUSTRIAL_NEEDS_RIGIDITY: &str = "machine class `industrial` has no default rigidity: \
     state a [rigidity] block (adaptive_woc_factor, force_budget_n), or \
     set \
     class = \"hobby\" to use the hobby grade default (Shapeoko port, Unsourced)";

/// P13c (O1): the refusal of an industrial `[rigidity]` block with no
/// `force_budget_n`. An industrial machine has no default budget.
pub const INDUSTRIAL_NEEDS_FORCE_BUDGET: &str = "machine class `industrial` has no default \
     force budget: state force_budget_n (N) in the [rigidity] block, or set class = \"hobby\" \
     to use the hobby budget (derived from the Sienci and Carbide 3D hobby slot charts)";

impl TryFrom<MachineProfileWire> for MachineProfile {
    type Error = &'static str;

    fn try_from(w: MachineProfileWire) -> Result<Self, Self::Error> {
        let rigidity = match (w.class, w.rigidity) {
            (class, Some(r)) => RigidityProfile {
                adaptive_woc_factor: r.adaptive_woc_factor,
                force_budget_n: match (class, r.force_budget_n) {
                    (_, Some(n)) => n,
                    (MachineClass::Hobby, None) => {
                        crate::feeds::force_budget::hobby_budget().force_n
                    }
                    (MachineClass::Industrial, None) => return Err(INDUSTRIAL_NEEDS_FORCE_BUDGET),
                },
            },
            (MachineClass::Hobby, None) => RigidityProfile::default(),
            (MachineClass::Industrial, None) => return Err(INDUSTRIAL_NEEDS_RIGIDITY),
        };
        Ok(Self {
            name: w.name,
            spindle: w.spindle,
            power: w.power,
            chip_load: w.chip_load,
            max_feed_mm_min: w.max_feed_mm_min,
            max_cutting_feed_mm_min: w.max_cutting_feed_mm_min,
            max_shank_mm: w.max_shank_mm,
            class: w.class,
            rigidity,
            kinematics: w.kinematics,
            controller: w.controller,
            head: w.head,
        })
    }
}

/// The controller firmware a `$$` / `$I` dump came from (G10).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControllerFirmware {
    /// The dump did not say. A bare `$$` dump looks the same on Grbl 1.1
    /// and on grblHAL.
    #[default]
    Unknown,
    /// Grbl 1.1.
    Grbl,
    /// grblHAL. The welcome line starts `GrblHAL` (report.c:312), or
    /// `$I` prints `[FIRMWARE:grblHAL]` (report.c:1111).
    GrblHal,
}

impl ControllerFirmware {
    /// The post dialect this firmware selects, or `None` when unknown.
    pub fn post_format(self) -> Option<crate::gcode::PostFormat> {
        match self {
            Self::Unknown => None,
            Self::Grbl => Some(crate::gcode::PostFormat::Grbl),
            Self::GrblHal => Some(crate::gcode::PostFormat::GrblHal),
        }
    }
}

/// The controller settings the export reads, from an imported `$$` dump
/// (G10). Each field is `None` when the dump did not carry it. The
/// setting numbers and units are grblHAL's (settings.h:78-80, 212-214,
/// 269; settings.c:2412-2499).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControllerSettings {
    #[serde(default)]
    pub firmware: ControllerFirmware,
    /// `$30` maximum spindle speed, RPM.
    #[serde(default)]
    pub rpm_max: Option<f64>,
    /// `$31` minimum spindle speed, RPM.
    #[serde(default)]
    pub rpm_min: Option<f64>,
    /// `$32` mode (settings.c:2415). The file holds the number.
    #[serde(default)]
    pub mode: Option<ControllerMode>,
    /// `$340` spindle at-speed tolerance, percent. Above 0, a spindle with
    /// an encoder makes M3 wait for the speed (spindle_control.c:787-806).
    #[serde(default)]
    pub spindle_at_speed_tolerance_pct: Option<f64>,
    /// `$341` tool change mode (settings.h:856-862, settings.c:2482). The
    /// file holds the number.
    #[serde(default)]
    pub tool_change_mode: Option<ControllerToolChangeMode>,
    /// `$342` tool change probing distance, mm.
    #[serde(default)]
    pub tool_change_probing_distance_mm: Option<f64>,
    /// `$394` spindle on delay, seconds (stored in ms, settings.c:1498-1499).
    /// M3 waits this long (spindle_control.c:787-789).
    #[serde(default)]
    pub spindle_on_delay_s: Option<f64>,
    /// The board has a mist output: `$I` lists `M` in `[OPT:` (report.c:934-935).
    /// `None` when no `$I` output was imported.
    #[serde(default)]
    pub mist_output: Option<bool>,
    /// How the board drives the spindle, from the `$I` plugin lines.
    /// `None` when no `$I` output was imported.
    #[serde(default)]
    pub spindle_drive: Option<SpindleDrive>,
}

/// How a grblHAL board drives the spindle (G5, G10).
///
/// `$30`/`$31` are the PWM spindle's range. A spindle plugin (a VFD over
/// Modbus) can override them (settings.c:2593-2594, "can be overridden by
/// spindle plugins"). The operator's board reads `$30=1000` and drives an
/// H-100 VFD, so a clamp from `$30` would turn S18000 into S1000.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpindleDrive {
    /// `$I` lists no spindle plugin: the PWM spindle, so `$30`/`$31` are
    /// the S range.
    Pwm,
    /// `$I` lists a spindle plugin (the name, e.g. `H-100 VFD`). `$30`/`$31`
    /// are information only.
    Plugin(String),
}

/// A numbered grblHAL setting held a value its enum does not name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("${setting}={value} is not a value this build knows")]
pub struct UnknownSettingValue {
    /// The setting number, for example 341.
    pub setting: u32,
    /// The value the setting held.
    pub value: u32,
}

/// `$32` mode (settings.c:2415). F202: the five-meaning `u32` of `$341` and
/// this three-meaning `u32` are enums; a value outside the list cannot load.
/// The project file holds the number (`try_from`/`into`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub enum ControllerMode {
    /// 0: normal.
    Normal,
    /// 1: laser.
    Laser,
    /// 2: lathe.
    Lathe,
}

impl ControllerMode {
    /// The `$32` number.
    pub const fn number(self) -> u32 {
        match self {
            Self::Normal => 0,
            Self::Laser => 1,
            Self::Lathe => 2,
        }
    }
}

impl TryFrom<u32> for ControllerMode {
    type Error = UnknownSettingValue;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Normal),
            1 => Ok(Self::Laser),
            2 => Ok(Self::Lathe),
            _ => Err(UnknownSettingValue { setting: 32, value }),
        }
    }
}

impl From<ControllerMode> for u32 {
    fn from(mode: ControllerMode) -> Self {
        mode.number()
    }
}

impl std::fmt::Display for ControllerMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.number())
    }
}

/// `$341` tool change mode (settings.h:856-862, settings.c:2482).
///
/// Distinct from [`crate::gcode::ToolChangeMode`], which is how ONE export
/// writes a change (pause, M6, suppress). This is what the controller does
/// with an M6. The project file holds the number (`try_from`/`into`), so a
/// value outside the list refuses at load.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub enum ControllerToolChangeMode {
    /// 0: normal.
    Normal,
    /// 1: manual touch off.
    ManualTouchOff,
    /// 2: manual touch off at G59.3.
    ManualTouchOffAtG59_3,
    /// 3: automatic touch off at G59.3.
    AutomaticTouchOffAtG59_3,
    /// 4: grblHAL ignores M6 (gcode.c:1838) and sets the new tool at once
    /// (gcode.c:4144-4147).
    IgnoreM6,
}

impl ControllerToolChangeMode {
    /// The `$341` number.
    pub const fn number(self) -> u32 {
        match self {
            Self::Normal => 0,
            Self::ManualTouchOff => 1,
            Self::ManualTouchOffAtG59_3 => 2,
            Self::AutomaticTouchOffAtG59_3 => 3,
            Self::IgnoreM6 => 4,
        }
    }
}

impl TryFrom<u32> for ControllerToolChangeMode {
    type Error = UnknownSettingValue;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Normal),
            1 => Ok(Self::ManualTouchOff),
            2 => Ok(Self::ManualTouchOffAtG59_3),
            3 => Ok(Self::AutomaticTouchOffAtG59_3),
            4 => Ok(Self::IgnoreM6),
            _ => Err(UnknownSettingValue {
                setting: 341,
                value,
            }),
        }
    }
}

impl From<ControllerToolChangeMode> for u32 {
    fn from(mode: ControllerToolChangeMode) -> Self {
        mode.number()
    }
}

impl std::fmt::Display for ControllerToolChangeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.number())
    }
}

impl ControllerSettings {
    /// True when M3 waits for the spindle on the controller: a spindle on
    /// delay (`$394`) or an at-speed tolerance (`$340`) above zero.
    pub fn waits_for_spindle(&self) -> bool {
        self.spindle_on_delay_s.is_some_and(|s| s > 0.0)
            || self.spindle_at_speed_tolerance_pct.is_some_and(|p| p > 0.0)
    }

    /// The seconds one spindle start costs on the controller (`$394`), for
    /// the cycle-time estimate. 0 when unknown.
    pub fn spindle_wait_s(&self) -> f64 {
        self.spindle_on_delay_s.filter(|s| *s > 0.0).unwrap_or(0.0)
    }

    /// The controller S range `($31, $30)` the export clamps S into. Only
    /// for a PWM spindle that `$I` confirmed: with a spindle plugin, or
    /// with no `$I` at all, `$30`/`$31` may not be the spindle's range
    /// (see [`SpindleDrive`]), so the answer is `None` and S is not
    /// clamped.
    pub fn rpm_range(&self) -> Option<(f64, f64)> {
        if self.spindle_drive != Some(SpindleDrive::Pwm) {
            return None;
        }
        match (self.rpm_min, self.rpm_max) {
            (Some(lo), Some(hi)) if hi > 0.0 && lo <= hi => Some((lo, hi)),
            _ => None,
        }
    }

    /// Write onto `self` every fact `stated` holds (B6).
    ///
    /// An import overwrites only what the dump states. A `None` field and
    /// an `Unknown` firmware state nothing, so `self` keeps its value: a
    /// bare `$$` dump does not erase the firmware, the spindle plugin or
    /// the tool-change probe distance an earlier `$I` import gave.
    pub fn merge_stated(&mut self, stated: &ControllerSettings) {
        // Name every field, so a new field cannot be left out here.
        let ControllerSettings {
            firmware,
            rpm_max,
            rpm_min,
            mode,
            spindle_at_speed_tolerance_pct,
            tool_change_mode,
            tool_change_probing_distance_mm,
            spindle_on_delay_s,
            mist_output,
            spindle_drive,
        } = stated;
        if *firmware != ControllerFirmware::Unknown {
            self.firmware = *firmware;
        }
        fn keep<T: Clone>(slot: &mut Option<T>, stated: &Option<T>) {
            if let Some(value) = stated {
                *slot = Some(value.clone());
            }
        }
        keep(&mut self.rpm_max, rpm_max);
        keep(&mut self.rpm_min, rpm_min);
        keep(&mut self.mode, mode);
        keep(
            &mut self.spindle_at_speed_tolerance_pct,
            spindle_at_speed_tolerance_pct,
        );
        keep(&mut self.tool_change_mode, tool_change_mode);
        keep(
            &mut self.tool_change_probing_distance_mm,
            tool_change_probing_distance_mm,
        );
        keep(&mut self.spindle_on_delay_s, spindle_on_delay_s);
        keep(&mut self.mist_output, mist_output);
        keep(&mut self.spindle_drive, spindle_drive);
    }

    /// True when `$341=4`: the board ignores M6. An unknown mode (no `$341`
    /// imported) is `false`: M6 runs.
    pub fn ignores_m6(&self) -> bool {
        match self.tool_change_mode {
            Some(ControllerToolChangeMode::IgnoreM6) => true,
            Some(
                ControllerToolChangeMode::Normal
                | ControllerToolChangeMode::ManualTouchOff
                | ControllerToolChangeMode::ManualTouchOffAtG59_3
                | ControllerToolChangeMode::AutomaticTouchOffAtG59_3,
            )
            | None => false,
        }
    }
}

impl Default for MachineProfile {
    fn default() -> Self {
        Self::generic_wood_router()
    }
}

impl MachineProfile {
    /// P11c (D13 item 6): how the rigidity values are known. A hobby
    /// machine whose values equal [`RigidityProfile::default`] reads the
    /// hobby grade default; any other values are stated by the file.
    #[must_use]
    pub fn rigidity_basis(&self) -> RigidityBasis {
        match self.class {
            MachineClass::Hobby if self.rigidity == RigidityProfile::default() => {
                RigidityBasis::HobbyDefault
            }
            MachineClass::Hobby => RigidityBasis::HobbyStated,
            MachineClass::Industrial => RigidityBasis::IndustrialStated,
        }
    }

    /// F210 (W6b): check the numeric profile fields and the kinematics
    /// block against [`domain::MachineDimension::domain`]. The head has its
    /// own check ([`head::MachineHead::validate`]).
    ///
    /// # Errors
    ///
    /// The [`domain::MachineDomainError`] of the first field outside its
    /// domain.
    pub fn validate(&self) -> Result<(), domain::MachineDomainError> {
        use domain::MachineDimension as D;
        D::MaxFeed.check(self.max_feed_mm_min, None)?;
        if let Some(cap) = self.max_cutting_feed_mm_min {
            D::MaxCuttingFeed.check(cap, None)?;
        }
        D::MaxShank.check(self.max_shank_mm, None)?;
        if let Some(kinematics) = &self.kinematics {
            kinematics.validate()?;
        }
        Ok(())
    }

    /// F4 — the feed ceiling for CUTTING moves, mm/min: the value of
    /// [`Self::cutting_ceiling`].
    pub fn cutting_feed_ceiling_mm_min(&self) -> f64 {
        self.cutting_ceiling().mm_min()
    }

    /// F002: the cutting-feed ceiling as its own type. A consumer that caps
    /// a CUT (the modulator) takes [`CuttingCeilingMmMin`], so the travel
    /// rate cannot reach it by mistake.
    ///
    /// P16 (operator ruling D9, 2026-11-06): no default cap. The order is:
    /// 1. `max_cutting_feed_mm_min` when stated ([`CuttingCeilingSource::Stated`]);
    /// 2. else the smaller of the X and Y rates of `kinematics.max_rate_xyz_mm_min`
    ///    ([`CuttingCeilingSource::AxisRate`], the `$$` import);
    /// 3. else the travel rate ([`CuttingCeilingSource::TravelRate`]).
    ///
    /// Each is held at or below the travel rate: a cut cannot outrun the
    /// axes. The travel rate is a required key, so a ceiling always exists.
    #[must_use]
    pub fn cutting_ceiling(&self) -> CuttingCeilingMmMin {
        let travel = self.max_feed_mm_min;
        let axis_xy = self
            .kinematics
            .and_then(|k| k.max_rate_xyz_mm_min)
            .map(|[x, y, _]| x.min(y))
            .filter(|r| r.is_finite() && *r > 0.0);
        let (mm_min, source) = match (self.max_cutting_feed_mm_min, axis_xy) {
            (Some(cap), _) => (cap, CuttingCeilingSource::Stated),
            (None, Some(rate)) => (rate, CuttingCeilingSource::AxisRate),
            (None, None) => (travel, CuttingCeilingSource::TravelRate),
        };
        CuttingCeilingMmMin {
            mm_min: mm_min.min(travel),
            source,
        }
    }

    /// P20e: THE cutting ceiling of one op: the machine ceiling (with the
    /// `.max(1.0)` guard against a zero profile), lowered to the op's own
    /// feed cap `feed_max_rate` (G-FEEDMAX). The post-sim modulator and the
    /// strategy advisor cap at it; the gates and the Suggest power figure
    /// read [`Self::op_steady_feed`], which calls it.
    #[must_use]
    pub fn op_cutting_ceiling(&self, feed_max_rate: Option<f64>) -> CuttingCeilingMmMin {
        self.cutting_ceiling()
            .at_least(1.0)
            .capped_at(feed_max_rate)
    }

    /// P20e: the feed an op with the stated feed `op_feed_mm_min` commands
    /// in steady cutting: `min(op feed, feed_max_rate, cutting ceiling)`
    /// through [`Self::op_cutting_ceiling`], with the limit that binds.
    #[must_use]
    pub fn op_steady_feed(
        &self,
        op_feed_mm_min: f64,
        feed_max_rate: Option<f64>,
    ) -> SteadyFeedMmMin {
        self.op_cutting_ceiling(feed_max_rate)
            .steady_feed(op_feed_mm_min)
    }

    /// P20b: the Z axis rate (mm/min, GRBL `$112`), the third value of
    /// `kinematics.max_rate_xyz_mm_min`. `None` when the machine states no
    /// per-axis rates; the generic kinematics fallback is not read. The
    /// feeds vertical-rate limit (`feeds::vertical::VerticalRateLimit`)
    /// is the one reader.
    #[must_use]
    pub fn z_axis_rate_mm_min(&self) -> Option<f64> {
        self.kinematics
            .and_then(|k| k.max_rate_xyz_mm_min)
            .map(|[_, _, z]| z)
            .filter(|r| r.is_finite() && *r > 0.0)
    }

    /// P21c: this profile with the per-axis maximum rates (mm/min, GRBL
    /// `$110`-`$112`) and accelerations (mm/s^2, `$120`-`$122`) stated,
    /// as the `$$` import states them. The other kinematics values hold
    /// (the defaults when the profile has no kinematics).
    #[must_use]
    pub fn with_axis_limits(
        mut self,
        max_rate_xyz_mm_min: [f64; 3],
        acceleration_xyz_mm_s2: [f64; 3],
    ) -> Self {
        let mut kinematics = self.kinematics.unwrap_or_default();
        kinematics.max_rate_xyz_mm_min = Some(max_rate_xyz_mm_min);
        kinematics.acceleration_xyz_mm_s2 = Some(acceleration_xyz_mm_s2);
        self.kinematics = Some(kinematics);
        self
    }

    /// P21c: `true` when the machine states its per-axis maximum rates
    /// (`kinematics.max_rate_xyz_mm_min`, GRBL `$110`-`$112`). The
    /// constant-chip RPM rule needs them (`feeds::constant_chip_gap`).
    #[must_use]
    pub fn states_axis_rates(&self) -> bool {
        self.kinematics
            .is_some_and(|k| k.max_rate_xyz_mm_min.is_some())
    }

    /// P21c (ruling Q3): `true` when the machine states its per-axis
    /// accelerations (`kinematics.acceleration_xyz_mm_s2`, GRBL
    /// `$120`-`$122`). The scalar acceleration does not count.
    #[must_use]
    pub fn states_axis_accelerations(&self) -> bool {
        self.kinematics
            .is_some_and(|k| k.acceleration_xyz_mm_s2.is_some())
    }

    /// F002: the travel rate (`max_feed_mm_min`, the $110 class) as its own
    /// type: the rate of rapids, links and the kinematics cruise cap.
    #[must_use]
    pub fn travel_rate(&self) -> TravelRateMmMin {
        TravelRateMmMin(self.max_feed_mm_min)
    }

    /// Acceleration-aware kinematics for estimators that opt into them —
    /// the F-034 cycle-time integrator on demand, and the strategy
    /// advisor's wall-clock comparison (STRATEGY_ADVISOR_2026-06-17).
    /// Returns the profile's explicit [`MachineKinematics`] when set,
    /// else a conservative wood-router default (200 mm/s²).
    ///
    /// Distinct from the `kinematics` field: that field's `None` is the
    /// F-034 feature flag that keeps *live-sim* `total_runtime_s`
    /// byte-identical, so presets must keep it `None`. This accessor
    /// always yields a usable model so accel-aware consumers (which
    /// estimate, not replay) don't special-case unconfigured profiles —
    /// without changing what the live sim does.
    pub fn effective_kinematics(&self) -> MachineKinematics {
        self.kinematics
            .unwrap_or_else(MachineKinematics::generic_wood_router)
    }

    /// The machine envelope a link stage costs a link against a retract
    /// with (P1 W4a). The one builder: generation reads it, and a machine
    /// edit compares it before and after to find the results it stales
    /// (B7).
    ///
    /// It reads [`Self::effective_kinematics`], the cutting-feed ceiling
    /// and the travel rate. A change of any of the three can move a link.
    pub fn link_kinematics(&self) -> kinematics::LinkKinematics {
        kinematics::LinkKinematics {
            kinematics: self.effective_kinematics(),
            max_feed_mm_min: self.cutting_feed_ceiling_mm_min().max(1.0),
            rapid_feed_mm_min: self.max_feed_mm_min.max(1.0),
        }
    }

    /// Conservative generic wood router defaults.
    pub fn generic_wood_router() -> Self {
        MachineProfile {
            name: "Generic Wood Router".to_owned(),
            spindle: SpindleConfig::Variable {
                min_rpm: 8000.0,
                max_rpm: 24000.0,
            },
            power: PowerModel::ConstantPower { power_kw: 0.8 },
            chip_load: ChipLoadFormula::default(),
            max_feed_mm_min: 4000.0,
            max_cutting_feed_mm_min: None,
            max_shank_mm: 6.35,
            // P11c (D13 item 6): the one default set (hobby grade). The
            // old generic block (0.20 / 1.5 / 0.20) is deleted.
            class: MachineClass::Hobby,
            rigidity: RigidityProfile::default(),
            // F-034: presets ship with `None` to keep runtime
            // behavior byte-identical. Callers opt in by setting
            // this field on the active profile.
            kinematics: None,
            controller: None,
            head: head::MachineHead::default(),
        }
    }

    /// Shapeoko with 1.5kW VFD spindle (ER11 collet).
    pub fn shapeoko_vfd() -> Self {
        MachineProfile {
            name: "Shapeoko (1.5kW VFD)".to_owned(),
            spindle: SpindleConfig::Variable {
                min_rpm: 6000.0,
                max_rpm: 24000.0,
            },
            power: PowerModel::VfdConstantTorque {
                rated_power_kw: 1.5,
                rated_rpm: 24000.0,
            },
            chip_load: ChipLoadFormula {
                k0: 0.024,
                p: 0.61,
                q: 1.26,
            },
            max_feed_mm_min: 5000.0,
            max_cutting_feed_mm_min: None,
            max_shank_mm: 7.0,
            class: MachineClass::Hobby,
            rigidity: RigidityProfile::default(),
            kinematics: None,
            controller: None,
            head: head::MachineHead::default(),
        }
    }

    /// Shapeoko with Makita RT0701C router (discrete speed dial).
    pub fn shapeoko_makita() -> Self {
        MachineProfile {
            name: "Shapeoko (Makita RT0701C)".to_owned(),
            spindle: SpindleConfig::Discrete {
                speeds: vec![10000.0, 12000.0, 17000.0, 22000.0, 27000.0, 30000.0],
            },
            power: PowerModel::ConstantPower { power_kw: 0.71 },
            chip_load: ChipLoadFormula {
                k0: 0.024,
                p: 0.61,
                q: 1.26,
            },
            max_feed_mm_min: 5000.0,
            max_cutting_feed_mm_min: None,
            max_shank_mm: 6.35,
            class: MachineClass::Hobby,
            rigidity: RigidityProfile::default(),
            kinematics: None,
            controller: None,
            head: head::MachineHead::default(),
        }
    }

    /// All built-in presets for UI dropdown.
    pub fn presets() -> Vec<(&'static str, MachineProfile)> {
        vec![
            ("Generic Wood Router", MachineProfile::generic_wood_router()),
            ("Shapeoko (1.5kW VFD)", MachineProfile::shapeoko_vfd()),
            (
                "Shapeoko (Makita RT0701C)",
                MachineProfile::shapeoko_makita(),
            ),
        ]
    }

    /// Clamp RPM to the machine's spindle range.
    pub fn clamp_rpm(&self, rpm: f64) -> f64 {
        match &self.spindle {
            SpindleConfig::Variable { min_rpm, max_rpm } => rpm.clamp(*min_rpm, *max_rpm),
            SpindleConfig::Discrete { speeds } => {
                #[expect(
                    clippy::indexing_slicing,
                    reason = "speeds is non-empty by construction; mid-index is always valid"
                )]
                let fallback = &speeds[speeds.len() / 2];
                *speeds
                    .iter()
                    .min_by(|&&a, &&b| (a - rpm).abs().total_cmp(&(b - rpm).abs()))
                    .unwrap_or(fallback)
            }
        }
    }

    /// The highest RPM this spindle can actually run that is **at or below**
    /// `rpm`, clamped into the spindle's range.
    ///
    /// [`Self::clamp_rpm`] snaps a discrete spindle to the NEAREST listed
    /// speed, which rounds UP about half the time. That is right for "what
    /// will this machine actually run", and wrong for any caller reducing the
    /// RPM to shed load: asking a Makita for 9 000 gets 10 000 back, so a
    /// power-limited traverse would raise the power it was called to reduce.
    ///
    /// Returns the spindle's minimum when `rpm` sits below everything the
    /// spindle offers. The caller must therefore check the result rather than
    /// assume it got what it asked for — the traverse is often partial, and a
    /// partial traverse reported as a whole one is a lie about the cut.
    pub fn next_rpm_at_or_below(&self, rpm: f64) -> f64 {
        match &self.spindle {
            SpindleConfig::Variable { min_rpm, max_rpm } => rpm.clamp(*min_rpm, *max_rpm),
            SpindleConfig::Discrete { speeds } => {
                let at_or_below = speeds
                    .iter()
                    .copied()
                    .filter(|s| *s <= rpm)
                    .reduce(f64::max);
                match at_or_below {
                    Some(s) => s,
                    // Nothing this slow exists. Hand back the slowest speed
                    // the spindle has, which is the closest it can get.
                    None => speeds
                        .iter()
                        .copied()
                        .reduce(f64::min)
                        .unwrap_or_else(|| self.clamp_rpm(rpm)),
                }
            }
        }
    }

    /// Available spindle power at the given RPM.
    pub fn power_at_rpm(&self, rpm: f64) -> f64 {
        match self.power {
            PowerModel::VfdConstantTorque {
                rated_power_kw,
                rated_rpm,
            } => {
                if rpm <= 0.0 {
                    return 0.0;
                }
                rated_power_kw * (rpm.min(rated_rpm) / rated_rpm)
            }
            PowerModel::ConstantPower { power_kw } => power_kw,
        }
    }

    /// RPM range as (min, max).
    pub fn rpm_range(&self) -> (f64, f64) {
        match &self.spindle {
            SpindleConfig::Variable { min_rpm, max_rpm } => (*min_rpm, *max_rpm),
            SpindleConfig::Discrete { speeds } => {
                let min = speeds.iter().cloned().reduce(f64::min).unwrap_or(10000.0);
                let max = speeds.iter().cloned().reduce(f64::max).unwrap_or(30000.0);
                (min, max)
            }
        }
    }

    /// Index into [`Self::presets`] of the preset this profile
    /// structurally equals, or `None` for a custom/edited machine.
    ///
    /// R6 (tech-debt review 2026-06-10): replaces the removed
    /// `to_key()`, which dispatched on `name.contains("VFD")`/
    /// `"Makita"` — renaming a machine silently changed its identity,
    /// and an edited preset still claimed to BE the preset. Current
    /// project files serialize the full profile inline
    /// (`ProjectFile.job.machine`), so no key is written anywhere
    /// (F091 deleted the `from_key` test door; the literature-matrix shim
    /// holds its own key table); structural equality (same serde-JSON
    /// form) is the honest preset test.
    pub fn matching_preset_index(&self) -> Option<usize> {
        let self_json = serde_json::to_string(self).ok()?;
        Self::presets()
            .iter()
            .position(|(_, p)| serde_json::to_string(p).ok().as_deref() == Some(&self_json))
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    /// P16 (operator ruling D9): no default cutting cap. With no stated
    /// cap the ceiling is the smaller kinematics X/Y rate, else the travel
    /// rate. A stated cap wins. No ceiling exceeds the travel rate.
    #[test]
    fn cutting_ceiling_reads_the_stated_cap_then_the_axis_rate_then_travel_d9() {
        let mut m = MachineProfile::generic_wood_router();
        m.max_feed_mm_min = 10_000.0;
        let c = m.cutting_ceiling();
        assert!(
            (c.mm_min() - 10_000.0).abs() < 1e-9,
            "travel bounds the cut: {c:?}"
        );
        assert_eq!(c.source(), CuttingCeilingSource::TravelRate);

        // The axis rate: the smaller of X and Y, Z is not read.
        let mut k = MachineKinematics::generic_wood_router();
        k.max_rate_xyz_mm_min = Some([9_000.0, 8_500.0, 1_000.0]);
        m.kinematics = Some(k);
        let c = m.cutting_ceiling();
        assert!((c.mm_min() - 8_500.0).abs() < 1e-9, "axis rate: {c:?}");
        assert_eq!(c.source(), CuttingCeilingSource::AxisRate);

        // A stated cap wins.
        m.max_cutting_feed_mm_min = Some(8_000.0);
        let c = m.cutting_ceiling();
        assert!((c.mm_min() - 8_000.0).abs() < 1e-9);
        assert_eq!(c.source(), CuttingCeilingSource::Stated);

        // ...but never exceeds travel.
        m.max_feed_mm_min = 4_000.0;
        assert!((m.cutting_feed_ceiling_mm_min() - 4_000.0).abs() < 1e-9);

        // An op cap only lowers, and names itself.
        let lowered = m.cutting_ceiling().capped_at(Some(3_000.0));
        assert!((lowered.mm_min() - 3_000.0).abs() < 1e-9);
        assert_eq!(lowered.source(), CuttingCeilingSource::OpFeedCap);
        let kept = m.cutting_ceiling().capped_at(Some(9_000.0));
        assert_eq!(kept.source(), CuttingCeilingSource::Stated);
    }

    #[test]
    fn test_vfd_power_scales_linearly() {
        let m = MachineProfile::shapeoko_vfd();
        let p_low = m.power_at_rpm(6000.0);
        let p_high = m.power_at_rpm(24000.0);
        assert!(
            (p_high - 1.5).abs() < 1e-9,
            "full RPM should give full power"
        );
        assert!(
            (p_low - 0.375).abs() < 1e-9,
            "quarter RPM should give quarter power"
        );
    }

    #[test]
    fn test_constant_power_is_constant() {
        let m = MachineProfile::shapeoko_makita();
        assert_eq!(m.power_at_rpm(10000.0), m.power_at_rpm(30000.0));
    }

    #[test]
    fn test_clamp_rpm_variable() {
        let m = MachineProfile::shapeoko_vfd();
        assert_eq!(m.clamp_rpm(3000.0), 6000.0);
        assert_eq!(m.clamp_rpm(30000.0), 24000.0);
        assert_eq!(m.clamp_rpm(18000.0), 18000.0);
    }

    #[test]
    fn test_clamp_rpm_discrete() {
        let m = MachineProfile::shapeoko_makita();
        // Ideal ~10610 should snap to 10000
        assert_eq!(m.clamp_rpm(10610.0), 10000.0);
        // Very high should snap to 30000
        assert_eq!(m.clamp_rpm(50000.0), 30000.0);
    }

    /// Ruling R4 (2026-09-24) and F034: a profile written with the old
    /// key `safety_factor` refuses (no key is ignored). D9 (2026-11-04):
    /// the old key `aggressiveness` refuses too.
    #[test]
    fn the_old_safety_factor_key_is_refused_on_load() {
        let mut value = serde_json::to_value(MachineProfile::generic_wood_router()).unwrap();
        let mut dial = value.clone();
        dial.as_object_mut()
            .unwrap()
            .insert("aggressiveness".to_owned(), serde_json::json!(0.85));
        let err = serde_json::from_value::<MachineProfile>(dial).unwrap_err();
        assert!(err.to_string().contains("aggressiveness"), "{err}");
        value
            .as_object_mut()
            .unwrap()
            .insert("safety_factor".to_owned(), serde_json::json!(0.75));
        let err = serde_json::from_value::<MachineProfile>(value).unwrap_err();
        assert!(err.to_string().contains("safety_factor"), "{err}");
    }

    /// F202: the `$341` and `$32` numbers round-trip, and a number the
    /// enum does not name refuses at load.
    #[test]
    fn controller_modes_hold_the_number_and_refuse_an_unknown_one() {
        let settings = ControllerSettings {
            mode: Some(ControllerMode::Lathe),
            tool_change_mode: Some(ControllerToolChangeMode::IgnoreM6),
            ..ControllerSettings::default()
        };
        let text = toml::to_string(&settings).unwrap();
        assert!(text.contains("mode = 2"), "{text}");
        assert!(text.contains("tool_change_mode = 4"), "{text}");
        let back: ControllerSettings = toml::from_str(&text).unwrap();
        assert_eq!(back, settings);
        assert!(back.ignores_m6());
        assert!(toml::from_str::<ControllerSettings>("tool_change_mode = 5").is_err());
        assert!(toml::from_str::<ControllerSettings>("mode = 3").is_err());
        assert!(toml::from_str::<ControllerSettings>("tool_change = 4").is_err());
    }

    #[test]
    fn matching_preset_index_identifies_each_preset() {
        for (i, (label, profile)) in MachineProfile::presets().iter().enumerate() {
            assert_eq!(
                profile.matching_preset_index(),
                Some(i),
                "preset '{label}' must match itself"
            );
        }
    }

    /// R6: identity is structural, not name-based — an edited preset is
    /// a custom machine even if its name still says "VFD", and a rename
    /// alone doesn't change which preset it is.
    #[test]
    fn matching_preset_index_rejects_edited_and_survives_rename() {
        let mut edited = MachineProfile::shapeoko_vfd();
        edited.max_feed_mm_min += 1.0;
        assert_eq!(edited.matching_preset_index(), None);

        let mut renamed = MachineProfile::shapeoko_vfd();
        renamed.name = "My router".to_owned();
        // A renamed profile differs structurally too (name serializes),
        // so it reads as custom — never as a different preset.
        assert_eq!(renamed.matching_preset_index(), None);
    }
}
