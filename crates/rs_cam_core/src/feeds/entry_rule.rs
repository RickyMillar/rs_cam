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
//! 8b (operator ruling O2, 2026-11-13: Easel): the ramp angle of an op is
//! [`EntryAngle`], stated or the rule. The rule is one table by the material
//! class of the stock ([`EntryMaterialClass`]), the same for both doors.

use crate::compute::catalog::PlungeFeedRule;
use crate::compute::config::{DressupConfig, DressupEntryStyle, HelixRadius};
use crate::compute::operation_configs::{Adaptive3dConfig, Adaptive3dEntryStyle};
use crate::feeds::vendor_lut::MaterialFamily;
use crate::limits::EntryKind;
use crate::material::{FormulaConstant, Material};
use crate::tool::MillingCutter;

/// The stored Easel page (operator ruling O2: "lets do 5 and 20").
const EASEL_RAMPING: &str =
    "planning/feeds_contract_2026-11-03/research/sources_entry_rules/easel_toolpath_ramping.txt";

/// The R6 research note. Its row A4 quotes the Shaper acrylic cell; the
/// stored Shaper page (`shaper_autopass_material_recommendations.txt`) ends
/// at line 62, before the angle of that cell.
const ENTRY_RULES_NOTE: &str =
    "planning/feeds_contract_2026-11-03/research/entry_rules_2026-11-11.md";

/// F201: the ramp angle rule (degrees) for hardwood. Easel,
/// `easel_toolpath_ramping.txt:13`.
const RAMP_ANGLE_HARDWOOD_DEG: FormulaConstant = FormulaConstant::cited(
    "entry_rule.ramp_angle_deg.hardwood",
    5.0,
    EASEL_RAMPING,
    "The shallower 5° angle ramp is good for hardwoods",
);

/// F201: the ramp angle rule (degrees) for softwood, plywood, MDF, HDF and
/// particleboard. Easel, `easel_toolpath_ramping.txt:13`.
const RAMP_ANGLE_SOFTWOOD_AND_COMPOSITE_DEG: FormulaConstant = FormulaConstant::cited(
    "entry_rule.ramp_angle_deg.softwood_and_composite",
    20.0,
    EASEL_RAMPING,
    "We recommend using the 20° angle ramp for soft woods, plywood, and MDF like materials.",
);

/// F201: the ramp angle rule (degrees) for a non-ferrous metal. Easel,
/// `easel_toolpath_ramping.txt:13` ("soft metals like aluminum").
const RAMP_ANGLE_NON_FERROUS_DEG: FormulaConstant = FormulaConstant::cited(
    "entry_rule.ramp_angle_deg.non_ferrous",
    5.0,
    EASEL_RAMPING,
    "The shallower 5° angle ramp is good for hardwoods and soft metals like aluminum.",
);

/// F201: the ramp angle rule (degrees) for a plastic. Easel prints none.
/// Shaper prints about 10 degrees for acrylic (research note row A4,
/// `entry_rules_2026-11-11.md:62`).
const RAMP_ANGLE_PLASTIC_DEG: FormulaConstant = FormulaConstant::cited(
    "entry_rule.ramp_angle_deg.plastic",
    10.0,
    ENTRY_RULES_NOTE,
    "5ipm per min (~10°)",
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

/// The F201 rows of this file (`feeds::formula_constants` reads them).
#[must_use]
pub fn formula_constants() -> [FormulaConstant; 7] {
    [
        RAMP_ANGLE_HARDWOOD_DEG,
        RAMP_ANGLE_SOFTWOOD_AND_COMPOSITE_DEG,
        RAMP_ANGLE_NON_FERROUS_DEG,
        RAMP_ANGLE_PLASTIC_DEG,
        DRESSUP_HELIX_PITCH_MM,
        ADAPTIVE3D_HELIX_PITCH_MM,
        HELIX_RADIUS_OVER_D,
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

/// The material class that the ramp-angle sources print (Easel prints the
/// angle by material, not by tool; research note 2.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryMaterialClass {
    /// Solid hardwood (Janka above 800 lbf).
    Hardwood,
    /// Softwood, plywood (both grades), MDF, HDF, particleboard.
    SoftwoodAndComposite,
    /// Acrylic, HDPE, polycarbonate, Delrin, fibreglass.
    Plastic,
    /// Aluminium.
    NonFerrous,
}

impl EntryMaterialClass {
    /// The class of `material`, from its vendor-table family
    /// (`vendor_normalize::material_to_lut`).
    #[must_use]
    pub fn of(material: &Material) -> Self {
        match crate::feeds::vendor_normalize::material_to_lut(material).0 {
            MaterialFamily::Hardwood => Self::Hardwood,
            MaterialFamily::Softwood
            | MaterialFamily::PlywoodSoftwood
            | MaterialFamily::PlywoodHardwood
            | MaterialFamily::Mdf
            | MaterialFamily::Hdf
            | MaterialFamily::Particleboard => Self::SoftwoodAndComposite,
            MaterialFamily::Acrylic
            | MaterialFamily::Hdpe
            | MaterialFamily::Polycarbonate
            | MaterialFamily::Delrin
            | MaterialFamily::Fiberglass => Self::Plastic,
            MaterialFamily::Aluminum => Self::NonFerrous,
        }
    }

    /// The class in words, for the card.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Hardwood => "hardwood",
            Self::SoftwoodAndComposite => "softwood, plywood, MDF",
            Self::Plastic => "plastic",
            Self::NonFerrous => "non-ferrous metal",
        }
    }

    /// The F201 row of the ramp angle rule of this class, and the source
    /// that prints it.
    const fn ramp_rule(self) -> (FormulaConstant, &'static str) {
        match self {
            Self::Hardwood => (RAMP_ANGLE_HARDWOOD_DEG, "Easel"),
            Self::SoftwoodAndComposite => (RAMP_ANGLE_SOFTWOOD_AND_COMPOSITE_DEG, "Easel"),
            Self::Plastic => (RAMP_ANGLE_PLASTIC_DEG, "Shaper"),
            Self::NonFerrous => (RAMP_ANGLE_NON_FERROUS_DEG, "Easel"),
        }
    }
}

/// The ramp angle of one op: stated by the operator, or the rule.
///
/// Wire form (P29-8 8b): a number loads as `Stated`; an absent key, `null`
/// or the token `"rule"` loads as `Rule`. The writer saves `Stated` as the
/// number and omits `Rule` (`skip_serializing_if = "EntryAngle::is_rule"`
/// on the field). A new op holds `Rule`; a setter writes `Stated`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum EntryAngle {
    /// An angle (degrees) the operator stated. It stands.
    Stated(f64),
    /// The rule of the stock material ([`EntryMaterialClass`]).
    #[default]
    Rule,
}

/// The token of [`EntryAngle::Rule`] on the wire.
const RULE_TOKEN: &str = "rule";

impl EntryAngle {
    /// True for [`Self::Rule`] (the writer omits it).
    #[must_use]
    pub const fn is_rule(&self) -> bool {
        matches!(self, Self::Rule)
    }

    /// The stated angle, or `None` for the rule.
    #[must_use]
    pub const fn stated(self) -> Option<f64> {
        match self {
            Self::Stated(v) => Some(v),
            Self::Rule => None,
        }
    }

    /// `Some(v)` is `Stated(v)`; `None` is the rule.
    #[must_use]
    pub const fn from_stated(v: Option<f64>) -> Self {
        match v {
            Some(v) => Self::Stated(v),
            None => Self::Rule,
        }
    }
}

impl serde::Serialize for EntryAngle {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Stated(v) => serializer.serialize_f64(*v),
            Self::Rule => serializer.serialize_str(RULE_TOKEN),
        }
    }
}

impl<'de> serde::Deserialize<'de> for EntryAngle {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum Wire {
            Number(f64),
            Token(String),
        }
        match Option::<Wire>::deserialize(deserializer)? {
            None => Ok(Self::Rule),
            Some(Wire::Number(v)) => Ok(Self::Stated(v)),
            Some(Wire::Token(t)) if t == RULE_TOKEN => Ok(Self::Rule),
            // An MCP client that sends every number as text.
            Some(Wire::Token(t)) => match t.trim().parse::<f64>() {
                Ok(v) => Ok(Self::Stated(v)),
                Err(_) => Err(serde::de::Error::custom(format!(
                    "a ramp angle is a number (degrees) or \"{RULE_TOKEN}\", not \"{t}\""
                ))),
            },
        }
    }
}

impl crate::compute::catalog::ParamType for EntryAngle {
    // The schema publishes the angle as an optional number: absent is the
    // rule.
    const TYPE_NAME: &'static str = "option<f64>";
    const OPTION_TYPE_NAME: &'static str = "option<f64>";
    const KIND: crate::compute::catalog::ParamKind = crate::compute::catalog::ParamKind::Real;
    const OPTIONAL: bool = true;
}

/// The ramp angle an op cuts at, with its source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RampAngle {
    /// The angle (degrees) the emitter cuts at.
    pub deg: f64,
    /// True when the operator stated it.
    pub stated: bool,
    /// The rule of the stock class (degrees).
    pub rule_deg: f64,
    /// The stock class.
    pub class: EntryMaterialClass,
    /// The source that prints the rule ("Easel", "Shaper").
    pub source: &'static str,
}

impl RampAngle {
    /// The ramp angle of `angle` in `material`.
    #[must_use]
    pub fn resolve(angle: EntryAngle, material: &Material) -> Self {
        let class = EntryMaterialClass::of(material);
        let (row, source) = class.ramp_rule();
        Self {
            deg: angle.stated().unwrap_or(row.value),
            stated: !angle.is_rule(),
            rule_deg: row.value,
            class,
            source,
        }
    }

    /// The rule in words: "the rule for hardwood (Easel)".
    #[must_use]
    pub fn rule_text(&self) -> String {
        format!("the rule for {} ({})", self.class.label(), self.source)
    }

    /// The source of the angle in words, for the card: the rule, or an
    /// operator value beside the rule.
    #[must_use]
    pub fn source_text(&self) -> String {
        if self.stated {
            format!(
                "operator value (the rule for {}: {:.2}°, {})",
                self.class.label(),
                self.rule_deg,
                self.source
            )
        } else {
            self.rule_text()
        }
    }
}

/// The entry values a new op of one door stores: the rule of that door.
/// The ramp angle is not here: a new op holds [`EntryAngle::Rule`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct StoredDefault {
    pub(crate) helix_pitch_mm: f64,
    /// The helix radius over D (the 3D Rough `helix_radius_factor`).
    pub(crate) helix_radius_over_d: f64,
}

/// The entry values a new op of `door` stores. Only the config defaults
/// call this (sentry `one_entry_rule_two_emitters_p29_8`).
#[must_use]
pub(crate) fn stored_default(door: EntryDoor) -> StoredDefault {
    StoredDefault {
        helix_pitch_mm: door_pitch_row(door).value,
        helix_radius_over_d: HELIX_RADIUS_OVER_D.value,
    }
}

/// The helix-pitch row of `door` (P29-8 Q2: two rows by door).
fn door_pitch_row(door: EntryDoor) -> FormulaConstant {
    match door {
        EntryDoor::Dressup => DRESSUP_HELIX_PITCH_MM,
        EntryDoor::Adaptive3d => ADAPTIVE3D_HELIX_PITCH_MM,
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
    /// The ramp angle the op cuts at: the stated value or the rule of the
    /// stock class (P29-8 8b).
    ramp_angle: RampAngle,
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
    /// request: the operator value, or 0.3 x `tool_diameter`. `material` is
    /// the stock material: the ramp angle rule reads its class.
    #[must_use]
    pub fn of_dressup(
        cfg: &DressupConfig,
        cutter: Option<&dyn MillingCutter>,
        tool_diameter: f64,
        material: &Material,
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
        let ramp_angle = RampAngle::resolve(cfg.ramp_angle, material);
        Self {
            door: EntryDoor::Dressup,
            kind,
            ramp_angle,
            fallback_ramp_angle_deg: ramp_angle.deg,
            helix_radius,
            helix_pitch_mm: cfg.helix_pitch,
            rule_helix_pitch_mm: door_pitch_row(EntryDoor::Dressup).value,
        }
    }

    /// The entry of a 3D Rough config on `cutter` in the stock `material`.
    #[must_use]
    pub fn of_adaptive3d(
        cfg: &Adaptive3dConfig,
        cutter: &dyn MillingCutter,
        material: &Material,
    ) -> Self {
        let ramp_angle = RampAngle::resolve(cfg.ramp_angle_deg, material);
        Self {
            door: EntryDoor::Adaptive3d,
            kind: Some(adaptive3d_kind(cfg)),
            ramp_angle,
            fallback_ramp_angle_deg: ramp_angle.deg,
            helix_radius: adaptive3d_helix_radius(cfg, cutter),
            helix_pitch_mm: cfg.helix_pitch,
            rule_helix_pitch_mm: door_pitch_row(EntryDoor::Adaptive3d).value,
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

    /// The ramp angle (degrees) the emitter cuts at.
    #[must_use]
    pub fn ramp_angle_deg(&self) -> f64 {
        self.ramp_angle.deg
    }

    /// The ramp angle with its source (stated or the rule).
    #[must_use]
    pub fn ramp_angle(&self) -> RampAngle {
        self.ramp_angle
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
            max_angle_deg: self.ramp_angle.deg,
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
                max_angle_deg: self.ramp_angle.deg,
            },
            Some(EntryKind::Helix) => crate::adaptive3d::EntryStyle3d::Helix {
                radius: self.helix_radius.emitted_mm,
                pitch: self.helix_pitch_mm,
                fallback_ramp_angle_deg: self.fallback_ramp_angle_deg,
            },
        }
    }
}

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
    /// - a plunge or a peck ladder: the plunge feed of the door. P29-8 8c
    ///   (operator ruling O1): every door plunges at the op plunge rate
    ///   (`PlungeFeedRule::OpPlungeRate`): the 3D Rough planner, a
    ///   straight-plunge contract, the generator plunge of a dressup op,
    ///   and the dressup air lead, stock-aware plunge and fallback plunge.
    ///
    /// # Errors
    ///
    /// An [`EntryNoNumber`] when the plan has no feed before generation.
    pub fn planned_feed_mm_min(
        &self,
        op: &crate::compute::catalog::OperationConfig,
        kind: EntryKind,
    ) -> Result<f64, EntryNoNumber> {
        use crate::compute::catalog::EntryContract;
        let op_rate = || {
            op.plunge_rate()
                .filter(usable)
                .ok_or(EntryNoNumber::NoPlungeRate)
        };
        let door_plunge = || match op.op_type().registry_entry().entry {
            EntryContract::Planner { .. } | EntryContract::Dressup { .. } => op_rate(),
            EntryContract::StraightPlunge { feed, .. } => match feed {
                PlungeFeedRule::OpPlungeRate => op_rate(),
            },
            EntryContract::DescentIsCut => Err(EntryNoNumber::DescentIsCut),
        };
        match kind {
            EntryKind::Ramp | EntryKind::Helix => match op.entry_feed_rate().filter(usable) {
                Some(feed) => Ok(feed),
                None => door_plunge(),
            },
            EntryKind::Plunge | EntryKind::PeckLadder => door_plunge(),
        }
    }

    /// The chip (mm per tooth) of an entry of `kind` on `op` with
    /// `flute_count` flutes: [`Self::planned_feed_mm_min`] over the stated
    /// spindle speed times the flute count, as a [`StoredFeedChip`]. The
    /// stated values only: the entry is a value check of the cut the op
    /// states.
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
    ) -> Result<StoredFeedChip, EntryNoNumber> {
        let feed = self.planned_feed_mm_min(op, kind)?;
        let rpm = Self::stated_rpm(op)?;
        Ok(StoredFeedChip::of(feed, rpm, flute_count))
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

/// **The chip of a stored feed** (P29-9 task 1): the chip interval that a
/// feed the op stores states, and the end of it that a chip check reads.
///
/// Why: Suggest writes every feed rounded DOWN to a whole
/// [`FEED_WRITE_STEP_MM_MIN`](crate::feeds::suggest::FEED_WRITE_STEP_MM_MIN)
/// (T-9). A stored whole feed `f` is thus any true feed in `[f, f + step)`,
/// and its chip is any value in `[f, f + step) / (rpm x Z)`. A feed that is
/// not a whole step states itself only (the interval is one point).
///
/// The rule: a chip check of a stored feed is `Exceeds` only when the whole
/// interval is outside the bound. A ceiling (the band maximum) reads the low
/// end, the stated chip. A floor (the band minimum, the rubbing floor) reads
/// the high end. The equality rule of the check (`CheckState::of_margin`)
/// then decides. The record keeps the stated chip as its value; the margin
/// follows the end that the check reads (lead ruling, P29-9 section 15).
///
/// The interval is derived from the write step, not a typed tolerance. It
/// covers one write. A value that went through two writes (the ramp feed
/// reads the stored cut feed over the lift, then rounds again) can sit up to
/// `1 + 1 / lift` steps under its source; this rule does not cover that.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StoredFeedChip {
    stated_mm: f64,
    high_mm: f64,
}

impl StoredFeedChip {
    /// The chip of the stored feed `feed_mm_min` at `rpm` with
    /// `flute_count` flutes.
    #[must_use]
    pub fn of(feed_mm_min: f64, rpm: f64, flute_count: u32) -> Self {
        let per_rev = rpm * f64::from(flute_count.max(1));
        let step = crate::feeds::suggest::FEED_WRITE_STEP_MM_MIN;
        let whole_step = (feed_mm_min / step).fract() == 0.0;
        let high_feed = if whole_step {
            feed_mm_min + step
        } else {
            feed_mm_min
        };
        Self {
            stated_mm: feed_mm_min / per_rev,
            high_mm: high_feed / per_rev,
        }
    }

    /// The stated chip: the stored feed over `rpm x Z`.
    #[must_use]
    pub const fn stated_mm(self) -> f64 {
        self.stated_mm
    }

    /// The end of the interval that a check on `side` reads: the low end
    /// (the stated chip) for a ceiling, the high end for a floor.
    #[must_use]
    pub const fn toward_inside(self, side: crate::limits::BoundSide) -> f64 {
        match side {
            crate::limits::BoundSide::Ceiling => self.stated_mm,
            crate::limits::BoundSide::Floor => self.high_mm,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stored_defaults_are_the_door_rows() {
        let d = stored_default(EntryDoor::Dressup);
        assert_eq!(d.helix_pitch_mm, DRESSUP_HELIX_PITCH_MM.value);
        let a = stored_default(EntryDoor::Adaptive3d);
        assert_eq!(a.helix_pitch_mm, ADAPTIVE3D_HELIX_PITCH_MM.value);
        assert_eq!(a.helix_radius_over_d, HELIX_RADIUS_OVER_D.value);
    }

    #[test]
    fn the_ramp_angle_rule_reads_the_material_class() {
        use crate::material::{PlywoodGrade, SheetGoodKind, WoodSpecies};
        let rule = |m: Material| RampAngle::resolve(EntryAngle::Rule, &m);
        let oak = rule(Material::SolidWood {
            species: WoodSpecies::WhiteOak,
        });
        assert_eq!(oak.class, EntryMaterialClass::Hardwood);
        assert_eq!(oak.deg, RAMP_ANGLE_HARDWOOD_DEG.value);
        assert!(!oak.stated);
        for soft in [
            Material::default(),
            Material::Plywood {
                grade: PlywoodGrade::BalticBirch,
            },
            Material::SheetGood {
                kind: SheetGoodKind::Mdf,
            },
        ] {
            assert_eq!(rule(soft).deg, RAMP_ANGLE_SOFTWOOD_AND_COMPOSITE_DEG.value);
        }
        let stated = RampAngle::resolve(EntryAngle::Stated(3.0), &Material::default());
        assert_eq!(stated.deg, 3.0);
        assert!(stated.stated);
        assert_eq!(stated.rule_deg, RAMP_ANGLE_SOFTWOOD_AND_COMPOSITE_DEG.value);
    }

    #[test]
    fn an_angle_loads_as_stated_and_the_rule_as_absent() {
        #[derive(serde::Serialize, serde::Deserialize)]
        struct Row {
            #[serde(default, skip_serializing_if = "EntryAngle::is_rule")]
            a: EntryAngle,
        }
        let load = |text: &str| toml::from_str::<Row>(text).map(|r| r.a);
        assert_eq!(load("a = 3.0").ok(), Some(EntryAngle::Stated(3.0)));
        assert_eq!(load("a = 3").ok(), Some(EntryAngle::Stated(3.0)));
        assert_eq!(load("").ok(), Some(EntryAngle::Rule));
        assert_eq!(load("a = \"rule\"").ok(), Some(EntryAngle::Rule));
        assert!(load("a = \"steep\"").is_err());
        let json = |text: &str| serde_json::from_str::<Row>(text).map(|r| r.a);
        assert_eq!(json("{\"a\": null}").ok(), Some(EntryAngle::Rule));
        let save = |a| toml::to_string(&Row { a }).unwrap_or_default();
        assert_eq!(save(EntryAngle::Stated(3.0)), "a = 3.0\n");
        assert_eq!(save(EntryAngle::Rule), "");
    }
}
