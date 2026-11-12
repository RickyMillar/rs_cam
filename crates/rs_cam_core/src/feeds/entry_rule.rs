//! P29-8 (commit 8a): THE entry rule. One door from an op to both entry
//! emitters and to Suggest.
//!
//! Two doors emit entries: the dressup door (`compute/execute/dressup_apply.rs`
//! calls `dressup::apply_entry`) and the 3D Rough door
//! (`compute/execute/rough_3d.rs` gives `adaptive3d/path.rs` its
//! `EntryStyle3d`). Suggest reads the same values (`feeds::ramp::entry_geometry`,
//! `entry_notes`). Each of them builds a [`ResolvedEntry`] and reads its
//! numbers from it. The entry constants are private rows of this file, so no
//! emitter can name an entry number.
//!
//! The config defaults (`DressupConfig::default`, the `Adaptive3dConfig`
//! serde defaults, the `BoreConfig` pitch) read [`stored_default`]. The
//! sentry `one_entry_rule_two_emitters_p29_8` holds that no other file calls
//! it and that no emitter reads an entry field of a config.
//!
//! 8a moves no number: each row keeps the value it had at its old site.

use crate::compute::catalog::PlungeFeedRule;
use crate::compute::config::{DressupConfig, DressupEntryStyle, HelixRadius};
use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
use crate::limits::EntryKind;
use crate::material::FormulaConstant;
use crate::tool::MillingCutter;
use crate::toolpath::{MoveType, Toolpath};

/// F201: the dressup ramp entry angle (degrees) a new toolpath starts with.
/// Repo rule, no source (G10). No wood or router source prints a ramp angle
/// (ruling Q9). It was `compute::config::DRESSUP_RAMP_ANGLE_DEG`.
const DRESSUP_RAMP_ANGLE_DEG: FormulaConstant = FormulaConstant::unsourced(
    "entry_rule.ramp_angle_deg.dressup",
    3.0,
    "repo rule with no primary source (G10 ruling Q9; P29-8 O2 asks the operator)",
);

/// F201: the 3D Rough ramp entry angle (degrees) a new toolpath starts
/// with. Repo rule, no source (G10, operator decision D4 2026-09-25: one
/// rule for the GUI and the CLI). It was
/// `compute::operation_configs::ADAPTIVE3D_RAMP_ANGLE_DEG`.
const ADAPTIVE3D_RAMP_ANGLE_DEG: FormulaConstant = FormulaConstant::unsourced(
    "entry_rule.ramp_angle_deg.adaptive3d",
    10.0,
    "repo rule with no primary source (G10 ruling Q9, decision D4; P29-8 O2 asks the operator)",
);

/// F201: the dressup helix entry pitch (mm per turn) a new toolpath starts
/// with. Repo rule, no source (G10 ruling Q8). It was
/// `compute::config::DRESSUP_HELIX_PITCH_MM`.
const DRESSUP_HELIX_PITCH_MM: FormulaConstant = FormulaConstant::unsourced(
    "entry_rule.helix_pitch_mm.dressup",
    1.0,
    "repo rule with no primary source (G10 ruling Q8; P29-8 Q2 keeps it)",
);

/// F201: the 3D Rough helix entry pitch (mm per turn) a new toolpath
/// starts with. Repo rule, no source (G10 ruling Q8, decision D4). It was
/// `compute::operation_configs::ADAPTIVE3D_HELIX_PITCH_MM`.
const ADAPTIVE3D_HELIX_PITCH_MM: FormulaConstant = FormulaConstant::unsourced(
    "entry_rule.helix_pitch_mm.adaptive3d",
    2.0,
    "repo rule with no primary source (G10 ruling Q8, decision D4; P29-8 Q2 keeps it)",
);

/// F201: the helix entry radius as a fraction of the tool diameter, when
/// the operator has not set one: r = 0.3 x D. Repo rule, no source (G10
/// rulings Q6 and Q8). Both doors read it. It was
/// `compute::config::HELIX_RADIUS_OVER_D`.
const HELIX_RADIUS_OVER_D: FormulaConstant = FormulaConstant::unsourced(
    "entry_rule.helix_radius_over_d",
    0.3,
    "repo rule with no primary source (G10 rulings Q6 and Q8)",
);

/// F201: the fraction of the first linear feed that the dressup entry
/// plunges at ([`ResolvedEntry::dressup_plunge_feed_mm_min`]). It was
/// `compute::catalog::ENTRY_PLUNGE_FIRST_FEED_FRACTION`; the name stays.
const ENTRY_PLUNGE_FIRST_FEED_FRACTION: FormulaConstant = FormulaConstant::unsourced(
    "dressup.entry_plunge_rate.first_linear_feed_fraction",
    0.5,
    "repo rule with no primary source (dressup_apply.rs entry_plunge_rate, feeds contract P3)",
);

/// F201: the plunge feed of the dressup entry (mm/min) when the toolpath
/// has no linear move ([`ResolvedEntry::dressup_plunge_feed_mm_min`]). It
/// was `compute::catalog::ENTRY_PLUNGE_FALLBACK_MM_MIN`; the name stays.
const ENTRY_PLUNGE_FALLBACK_MM_MIN: FormulaConstant = FormulaConstant::unsourced(
    "dressup.entry_plunge_rate.fallback_mm_min",
    500.0,
    "repo rule with no primary source (dressup_apply.rs entry_plunge_rate, feeds contract P3)",
);

/// The F201 rows of this file (`feeds::formula_constants` reads them).
#[must_use]
pub fn formula_constants() -> [FormulaConstant; 7] {
    [
        DRESSUP_RAMP_ANGLE_DEG,
        ADAPTIVE3D_RAMP_ANGLE_DEG,
        DRESSUP_HELIX_PITCH_MM,
        ADAPTIVE3D_HELIX_PITCH_MM,
        HELIX_RADIUS_OVER_D,
        ENTRY_PLUNGE_FIRST_FEED_FRACTION,
        ENTRY_PLUNGE_FALLBACK_MM_MIN,
    ]
}

/// The door that emits an op's entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryDoor {
    /// `dressup::apply_entry`, through the dressup pipeline.
    Dressup,
    /// The 3D Rough planner (`adaptive3d/path.rs`).
    Adaptive3d,
}

/// The entry values a new op of one door stores: the rule of that door.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct StoredDefault {
    pub(crate) ramp_angle_deg: f64,
    pub(crate) helix_pitch_mm: f64,
    /// The helix radius over D (the 3D Rough `helix_radius_factor`).
    pub(crate) helix_radius_over_d: f64,
}

/// The entry values a new op of `door` stores. Only the config defaults
/// call this (sentry `one_entry_rule_two_emitters_p29_8`).
#[must_use]
pub(crate) fn stored_default(door: EntryDoor) -> StoredDefault {
    let (ramp, pitch) = door_rows(door);
    StoredDefault {
        ramp_angle_deg: ramp.value,
        helix_pitch_mm: pitch.value,
        helix_radius_over_d: HELIX_RADIUS_OVER_D.value,
    }
}

/// The ramp-angle row and the helix-pitch row of `door`.
fn door_rows(door: EntryDoor) -> (FormulaConstant, FormulaConstant) {
    match door {
        EntryDoor::Dressup => (DRESSUP_RAMP_ANGLE_DEG, DRESSUP_HELIX_PITCH_MM),
        EntryDoor::Adaptive3d => (ADAPTIVE3D_RAMP_ANGLE_DEG, ADAPTIVE3D_HELIX_PITCH_MM),
    }
}

/// The helix rule as text: "0.3 x D". The card and the GUI label read it.
#[must_use]
pub fn helix_rule_text() -> String {
    format!("{} x D", HELIX_RADIUS_OVER_D.value)
}

/// The helix rule on a diameter as text: "0.3 x D = 0.95 mm".
#[must_use]
pub fn helix_rule_line(diameter_mm: f64) -> String {
    let rule_mm = HELIX_RADIUS_OVER_D.value * diameter_mm;
    format!("{} x D = {rule_mm:.2} mm", HELIX_RADIUS_OVER_D.value)
}

/// The helix radius a dressup config emits on `cutter`: the operator value,
/// or 0.3 x D, capped at the flat bottom (G10 Q6).
#[must_use]
pub fn dressup_helix_radius(cfg: &DressupConfig, cutter: &dyn MillingCutter) -> HelixRadius {
    match cfg.helix_radius {
        Some(r) => HelixRadius::resolve(r, false, cutter),
        None => HelixRadius::resolve(
            HELIX_RADIUS_OVER_D.value * crate::compute::config::helix_entry_diameter_mm(cutter),
            true,
            cutter,
        ),
    }
}

/// The helix radius a 3D Rough config emits on `cutter`: D x
/// `helix_radius_factor`, capped at the flat bottom (G10 Q6). The request
/// is the rule when the factor is the rule.
#[must_use]
pub fn adaptive3d_helix_radius(cfg: &Adaptive3dConfig, cutter: &dyn MillingCutter) -> HelixRadius {
    let rule = HELIX_RADIUS_OVER_D.value;
    HelixRadius::resolve(
        crate::compute::config::helix_entry_diameter_mm(cutter) * cfg.helix_radius_factor,
        (cfg.helix_radius_factor - rule).abs() <= 1e-9,
        cutter,
    )
}

/// Everything an emitter needs for one op's entries, resolved once.
///
/// The fields are private: the two constructors are the only builders, so
/// an emitter reads an entry number only through this value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedEntry {
    door: EntryDoor,
    /// The configured entry kind. `None`: no entry (dressup style None).
    kind: Option<EntryKind>,
    /// The ramp angle (degrees) the op states.
    ramp_angle_deg: f64,
    /// The ramp angle of the door's rule (the card compares the two).
    rule_ramp_angle_deg: f64,
    /// The angle (degrees) of the ramp that replaces a helix with no room.
    /// Lead ruling 2026-11-12: the op's own ramp angle.
    fallback_ramp_angle_deg: f64,
    /// The helix radius the door emits.
    helix_radius: HelixRadius,
    /// The helix pitch (mm per turn) the op states.
    helix_pitch_mm: f64,
    /// The helix pitch of the door's rule.
    rule_helix_pitch_mm: f64,
}

impl ResolvedEntry {
    /// The entry of a dressup config.
    ///
    /// `cutter` is the op's tool. With no cutter (a test context) the flat
    /// bottom and the nominal D are unknown, so the helix radius is the
    /// request: the operator value, or 0.3 x `tool_diameter`.
    #[must_use]
    pub fn of_dressup(
        cfg: &DressupConfig,
        cutter: Option<&dyn MillingCutter>,
        tool_diameter: f64,
    ) -> Self {
        let helix_radius = match cutter {
            Some(cutter) => dressup_helix_radius(cfg, cutter),
            None => {
                let r = cfg
                    .helix_radius
                    .unwrap_or(HELIX_RADIUS_OVER_D.value * tool_diameter);
                HelixRadius {
                    requested_mm: r,
                    emitted_mm: r,
                    flat_bottom_mm: None,
                    core_floor_mm: None,
                    from_rule: cfg.helix_radius.is_none(),
                }
            }
        };
        let kind = dressup_kind(cfg);
        let (rule_ramp, rule_pitch) = door_rows(EntryDoor::Dressup);
        Self {
            door: EntryDoor::Dressup,
            kind,
            ramp_angle_deg: cfg.ramp_angle,
            rule_ramp_angle_deg: rule_ramp.value,
            fallback_ramp_angle_deg: cfg.ramp_angle,
            helix_radius,
            helix_pitch_mm: cfg.helix_pitch,
            rule_helix_pitch_mm: rule_pitch.value,
        }
    }

    /// The entry of a 3D Rough config on `cutter`.
    #[must_use]
    pub fn of_adaptive3d(cfg: &Adaptive3dConfig, cutter: &dyn MillingCutter) -> Self {
        let (rule_ramp, rule_pitch) = door_rows(EntryDoor::Adaptive3d);
        Self {
            door: EntryDoor::Adaptive3d,
            kind: Some(adaptive3d_kind(cfg)),
            ramp_angle_deg: cfg.ramp_angle_deg,
            rule_ramp_angle_deg: rule_ramp.value,
            fallback_ramp_angle_deg: cfg.ramp_angle_deg,
            helix_radius: adaptive3d_helix_radius(cfg, cutter),
            helix_pitch_mm: cfg.helix_pitch,
            rule_helix_pitch_mm: rule_pitch.value,
        }
    }

    /// The door of this entry.
    #[must_use]
    pub fn door(&self) -> EntryDoor {
        self.door
    }

    /// The configured entry kind; `None` for no entry.
    #[must_use]
    pub fn kind(&self) -> Option<EntryKind> {
        self.kind
    }

    /// The ramp angle (degrees) the op states.
    #[must_use]
    pub fn ramp_angle_deg(&self) -> f64 {
        self.ramp_angle_deg
    }

    /// The ramp angle (degrees) of the door's rule.
    #[must_use]
    pub fn rule_ramp_angle_deg(&self) -> f64 {
        self.rule_ramp_angle_deg
    }

    /// The angle (degrees) of the ramp that replaces a helix with no room.
    #[must_use]
    pub fn fallback_ramp_angle_deg(&self) -> f64 {
        self.fallback_ramp_angle_deg
    }

    /// The helix radius the door emits, with its rule facts.
    #[must_use]
    pub fn helix_radius(&self) -> HelixRadius {
        self.helix_radius
    }

    /// The helix pitch (mm per turn) the op states.
    #[must_use]
    pub fn helix_pitch_mm(&self) -> f64 {
        self.helix_pitch_mm
    }

    /// The helix pitch (mm per turn) of the door's rule.
    #[must_use]
    pub fn rule_helix_pitch_mm(&self) -> f64 {
        self.rule_helix_pitch_mm
    }

    /// The dressup ramp of this entry.
    #[must_use]
    pub fn dressup_ramp(&self) -> crate::dressup::EntryStyle {
        crate::dressup::EntryStyle::Ramp {
            max_angle_deg: self.ramp_angle_deg,
        }
    }

    /// The dressup helix of this entry.
    #[must_use]
    pub fn dressup_helix(&self) -> crate::dressup::EntryStyle {
        crate::dressup::EntryStyle::Helix {
            radius: self.helix_radius.emitted_mm,
            pitch: self.helix_pitch_mm,
            fallback_ramp_angle_deg: self.fallback_ramp_angle_deg,
        }
    }

    /// The 3D Rough planner entry of this entry. A peck ladder is the
    /// planner's `Plunge`; a dressup entry with no kind also gives `Plunge`.
    #[must_use]
    pub fn style_3d(&self) -> crate::adaptive3d::EntryStyle3d {
        match self.kind {
            None | Some(EntryKind::Plunge | EntryKind::PeckLadder) => {
                crate::adaptive3d::EntryStyle3d::Plunge
            }
            Some(EntryKind::Ramp) => crate::adaptive3d::EntryStyle3d::Ramp {
                max_angle_deg: self.ramp_angle_deg,
            },
            Some(EntryKind::Helix) => crate::adaptive3d::EntryStyle3d::Helix {
                radius: self.helix_radius.emitted_mm,
                pitch: self.helix_pitch_mm,
                fallback_ramp_angle_deg: self.fallback_ramp_angle_deg,
            },
        }
    }

    /// The dressup door's plunge feed (mm/min) for the air lead, the
    /// stock-aware plunge and the plunge fallback, on `toolpath` as the
    /// entry step sees it: the first linear feed times 0.5, else 500 mm/min
    /// (the contract rule `PlungeFeedRule::FirstLinearFeedFraction`). The
    /// emitters then take the `min` with the feed of the plunge move.
    ///
    /// The 3D Rough door feeds the same moves at the op plunge rate
    /// (`Adaptive3dParams::plunge_rate`). P29-8 O1 asks the operator whether
    /// the dressup door does too.
    #[must_use]
    pub fn dressup_plunge_feed_mm_min(toolpath: &Toolpath) -> f64 {
        toolpath
            .moves
            .iter()
            .find_map(|m| match m.move_type {
                MoveType::Linear { feed_rate } => {
                    Some(feed_rate * ENTRY_PLUNGE_FIRST_FEED_FRACTION.value)
                }
                MoveType::Rapid | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => None,
            })
            .unwrap_or(ENTRY_PLUNGE_FALLBACK_MM_MIN.value)
    }
}

/// The plunge feed rule of the dressup door (the air lead, the stock-aware
/// plunge and the fallback plunge). [`ResolvedEntry::dressup_plunge_feed_mm_min`]
/// evaluates it on a toolpath; [`ConfiguredEntry::planned_feed_mm_min`]
/// reads it before generation. P29-8 O1 (commit 8c) changes this one value.
const DRESSUP_PLUNGE_RULE: PlungeFeedRule = PlungeFeedRule::FirstLinearFeedFraction;

/// The configured entry kind of a dressup config. `None`: the style None
/// (the op enters by the plunge of its contract).
fn dressup_kind(cfg: &DressupConfig) -> Option<EntryKind> {
    match cfg.entry_style {
        DressupEntryStyle::None => None,
        DressupEntryStyle::Ramp => Some(EntryKind::Ramp),
        DressupEntryStyle::Helix => Some(EntryKind::Helix),
    }
}

/// The configured entry kind of a 3D Rough config. The planner's plunge is
/// a peck ladder.
fn adaptive3d_kind(cfg: &Adaptive3dConfig) -> EntryKind {
    match cfg.entry_style {
        Adaptive3dEntryStyle::Plunge => EntryKind::PeckLadder,
        Adaptive3dEntryStyle::Ramp => EntryKind::Ramp,
        Adaptive3dEntryStyle::Helix => EntryKind::Helix,
    }
}

/// True for a feed or a speed that a cut can run at.
fn usable(v: &f64) -> bool {
    v.is_finite() && *v > 0.0
}

/// P29-8 8d: what `OperationConfig::cuts` reads of an op's entry. It is a
/// projection of [`ResolvedEntry`]: the configured kind
/// ([`ResolvedEntry::kind`]) and the helix pitch (the depth of a helix
/// entry cut). It needs no cutter, so every reader of the cut list can
/// build it.
///
/// The fields are private. The constructors are [`Self::of_resolved`] (an
/// emitter that holds a [`ResolvedEntry`]) and [`Self::of_operation`] (a
/// reader with the op and, when it holds the toolpath, its dressups).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConfiguredEntry {
    /// The configured kind; `None` for the dressup style None.
    kind: Option<EntryKind>,
    /// The helix pitch (mm per turn) the op states.
    helix_pitch_mm: f64,
}

/// Why a declared entry cut has no feed or no chip before the simulation
/// (P29-8 8d). Each reason is a fact of the plan, not a defect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryNoNumber {
    /// The op states no plunge rate (S11).
    NoPlungeRate,
    /// The dressup door plunges at a fraction of the first linear feed of
    /// the toolpath, else at a fixed feed (`PlungeFeedRule::
    /// FirstLinearFeedFraction`). The first linear feed is known only
    /// after generation (P29-8 O1, commit 8c). On a Profile with tabs the
    /// same cut holds the tab re-plunge at the op plunge rate (row
    /// `U-TabReplunge`), so the cut has no one feed.
    FirstLinearFeedAfterGeneration,
    /// The op states no spindle speed, so its entry feed gives no chip
    /// (S11).
    NoSpindleSpeed,
    /// The descent is the cut: the op has no entry (`EntryContract::
    /// DescentIsCut`).
    DescentIsCut,
}

impl EntryNoNumber {
    /// The reason in words, for the card and MCP.
    #[must_use]
    pub const fn clause(self) -> &'static str {
        match self {
            Self::NoPlungeRate => "the op states no plunge rate",
            Self::FirstLinearFeedAfterGeneration => {
                "the dressup plunge feed reads the first linear feed of the toolpath, known only \
                 after generation"
            }
            Self::NoSpindleSpeed => "the op states no spindle speed",
            Self::DescentIsCut => "the descent is the cut: the op has no entry",
        }
    }
}

impl ConfiguredEntry {
    /// The projection of an emitter's resolved entry.
    #[must_use]
    pub fn of_resolved(entry: &ResolvedEntry) -> Self {
        Self {
            kind: entry.kind,
            helix_pitch_mm: entry.helix_pitch_mm,
        }
    }

    /// The configured entry of `op`. A 3D Rough reads its own config; any
    /// other op reads `dressups`, else the dressups that a new toolpath of
    /// the op gets (`DressupConfig::for_op`, the registry default). An op
    /// whose contract ignores the dressup entry style has no configured
    /// kind.
    #[must_use]
    pub fn of_operation(
        op: &crate::compute::catalog::OperationConfig,
        dressups: Option<&DressupConfig>,
    ) -> Self {
        if let crate::compute::catalog::OperationConfig::Adaptive3d(c) = op {
            return Self {
                kind: Some(adaptive3d_kind(c)),
                helix_pitch_mm: c.helix_pitch,
            };
        }
        let fresh;
        let cfg = match dressups {
            Some(cfg) => cfg,
            None => {
                fresh = DressupConfig::for_op(op.op_type());
                &fresh
            }
        };
        let applies = op.op_type().registry_entry().entry.entry_style_applies();
        Self {
            kind: if applies { dressup_kind(cfg) } else { None },
            helix_pitch_mm: cfg.helix_pitch,
        }
    }

    /// The registry default of `op`: [`Self::of_operation`] with no
    /// dressups. A reader that holds no toolpath passes it.
    #[must_use]
    pub fn registry_default(op: &crate::compute::catalog::OperationConfig) -> Self {
        Self::of_operation(op, None)
    }

    /// The configured kind; `None` for the dressup style None.
    #[must_use]
    pub fn kind(&self) -> Option<EntryKind> {
        self.kind
    }

    /// The helix pitch (mm per turn) the op states.
    #[must_use]
    pub fn helix_pitch_mm(&self) -> f64 {
        self.helix_pitch_mm
    }

    /// The feed (mm/min) at which the plan cuts an entry of `kind` on
    /// `op`, before generation:
    ///
    /// - a ramp or a helix: the op's entry feed (`entry_feed_rate`: the
    ///   ramp feed at or below the cut feed), the feed both emitters write.
    ///   With no ramp feed both emitters take the plunge feed of the door
    ///   (`unwrap_or(feed_rate)` in `dressup::entry_descent`);
    /// - a plunge or a peck ladder: the plunge feed of the door. The 3D
    ///   Rough door and a straight-plunge contract with
    ///   `PlungeFeedRule::OpPlungeRate` read the op plunge rate; the
    ///   generator plunge of a dressup op with the style None too; every
    ///   other dressup plunge reads [`DRESSUP_PLUNGE_RULE`].
    ///
    /// # Errors
    ///
    /// An [`EntryNoNumber`] when the plan has no feed before generation.
    pub fn planned_feed_mm_min(
        &self,
        op: &crate::compute::catalog::OperationConfig,
        kind: EntryKind,
    ) -> Result<f64, EntryNoNumber> {
        use crate::compute::catalog::{EntryContract, NoneEntry};
        let op_rate = || {
            op.plunge_rate()
                .filter(usable)
                .ok_or(EntryNoNumber::NoPlungeRate)
        };
        let of_rule = |rule: PlungeFeedRule| match rule {
            PlungeFeedRule::OpPlungeRate => op_rate(),
            PlungeFeedRule::FirstLinearFeedFraction => {
                Err(EntryNoNumber::FirstLinearFeedAfterGeneration)
            }
        };
        let door_plunge = || match op.op_type().registry_entry().entry {
            EntryContract::Planner { .. } => op_rate(),
            EntryContract::StraightPlunge { feed, .. } => of_rule(feed),
            EntryContract::Dressup { none_is, .. } => match (self.kind, none_is) {
                // The style None keeps the generator plunge.
                (None, NoneEntry::GeneratorPlunge) => op_rate(),
                (None, NoneEntry::StockAwarePlunge) | (Some(_), _) => of_rule(DRESSUP_PLUNGE_RULE),
            },
            EntryContract::DescentIsCut => Err(EntryNoNumber::DescentIsCut),
        };
        match kind {
            EntryKind::Ramp | EntryKind::Helix => match op.entry_feed_rate().filter(usable) {
                Some(feed) => Ok(feed),
                // The dressup door emits every ramp and helix of a dressup
                // op, at its own plunge rule (the generator plunge of the
                // style None is not a ramp).
                None => match op.op_type().registry_entry().entry {
                    EntryContract::Dressup { .. } => of_rule(DRESSUP_PLUNGE_RULE),
                    EntryContract::Planner { .. }
                    | EntryContract::StraightPlunge { .. }
                    | EntryContract::DescentIsCut => door_plunge(),
                },
            },
            EntryKind::Plunge | EntryKind::PeckLadder => door_plunge(),
        }
    }

    /// The chip (mm per tooth) of an entry of `kind` on `op` with
    /// `flute_count` flutes: [`Self::planned_feed_mm_min`] over the stated
    /// spindle speed times the flute count. The stated values only: the
    /// entry is a value check of the cut the op states.
    ///
    /// # Errors
    ///
    /// The errors of [`Self::planned_feed_mm_min`], and
    /// [`EntryNoNumber::NoSpindleSpeed`].
    pub fn planned_chip_mm(
        &self,
        op: &crate::compute::catalog::OperationConfig,
        kind: EntryKind,
        flute_count: u32,
    ) -> Result<f64, EntryNoNumber> {
        let feed = self.planned_feed_mm_min(op, kind)?;
        let rpm = Self::stated_rpm(op)?;
        Ok(feed / (rpm * f64::from(flute_count.max(1))))
    }

    /// The spindle speed (rpm) the op states.
    ///
    /// # Errors
    ///
    /// [`EntryNoNumber::NoSpindleSpeed`] when the op states none.
    pub fn stated_rpm(op: &crate::compute::catalog::OperationConfig) -> Result<f64, EntryNoNumber> {
        op.spindle_rpm()
            .map(f64::from)
            .filter(usable)
            .ok_or(EntryNoNumber::NoSpindleSpeed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::P3;

    #[test]
    fn the_dressup_plunge_feed_reads_the_first_linear_feed() {
        let mut t = Toolpath::new();
        t.rapid_to(P3::new(0.0, 0.0, 10.0));
        t.feed_to(P3::new(1.0, 0.0, 10.0), 1000.0);
        let half = 1000.0 * ENTRY_PLUNGE_FIRST_FEED_FRACTION.value;
        assert_eq!(
            ResolvedEntry::dressup_plunge_feed_mm_min(&t).to_bits(),
            half.to_bits()
        );
        assert_eq!(
            ResolvedEntry::dressup_plunge_feed_mm_min(&Toolpath::new()).to_bits(),
            ENTRY_PLUNGE_FALLBACK_MM_MIN.value.to_bits()
        );
    }

    #[test]
    fn the_stored_defaults_are_the_door_rows() {
        let d = stored_default(EntryDoor::Dressup);
        assert_eq!(d.ramp_angle_deg, DRESSUP_RAMP_ANGLE_DEG.value);
        assert_eq!(d.helix_pitch_mm, DRESSUP_HELIX_PITCH_MM.value);
        let a = stored_default(EntryDoor::Adaptive3d);
        assert_eq!(a.ramp_angle_deg, ADAPTIVE3D_RAMP_ANGLE_DEG.value);
        assert_eq!(a.helix_pitch_mm, ADAPTIVE3D_HELIX_PITCH_MM.value);
        assert_eq!(a.helix_radius_over_d, HELIX_RADIUS_OVER_D.value);
    }
}
