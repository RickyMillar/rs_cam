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
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RigidityProfile {
    pub doc_roughing_factor: f64,
    pub doc_finishing_factor: f64,
    pub woc_roughing_factor: f64,
    pub woc_roughing_max_mm: f64,
    pub woc_finishing_mm: f64,
    pub adaptive_doc_factor: f64,
    pub adaptive_woc_factor: f64,
}

/// **The axial depth cap one rigidity profile implies for one
/// operation family, and the factor behind it.** S3 (2026-09-18).
///
/// The cap is a RULE OF THUMB with no published source. It is the only
/// axial bound this crate has, and it sets the depth on nearly every
/// recipe, so it is stated as a value with its factor rather than
/// recomputed at each reader. [`RigidityProfile::depth_cap_mm`] is the
/// one producer.
///
/// The cap is not stored: [`Self::cap_mm`] multiplies the two fields it
/// carries, so the bound and the provenance beside it cannot disagree.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RigidityDepthCap {
    /// The profile's own factor for this family: `doc_roughing_factor`
    /// or `adaptive_doc_factor`.
    pub factor: f64,
    /// The diameter the factor multiplies, in mm: the engaged diameter
    /// at the judged depth, from
    /// `crate::feeds::geometry::depth_cap_diameter_mm`. On a flat, ball,
    /// bull or V-bit tool that is the nominal diameter. On a tapered
    /// ball it is the cone diameter at the depth, capped at the shank.
    pub diameter_mm: f64,
}

impl RigidityDepthCap {
    /// The cap itself, in mm: `factor × diameter_mm`.
    #[must_use]
    pub fn cap_mm(&self) -> f64 {
        self.factor * self.diameter_mm
    }
}

impl RigidityProfile {
    /// **The axial depth cap this profile implies for one operation
    /// family, and the factor behind it.** S3 (2026-09-18).
    ///
    /// `None` for an operation with no axial cap:
    ///
    /// - [`OperationFamily::Drill`](crate::feeds::OperationFamily::Drill): a drill cycle is Z-only, so there
    ///   is no radial engagement and no axial rule of thumb to judge it
    ///   by. A drill operation also carries no `depth_per_pass`, so the
    ///   Suggest clamp never reaches this arm.
    /// - A `SemiFinish` or `Finish` pass outside the adaptive family.
    ///   Feeds matrix R2 (2026-09-23): no vendor publishes an axial
    ///   finishing cap as a fraction of D for wood (EVIDENCE 5.1-3,
    ///   5.1-12), and the operator ruled for the most flexibility. The
    ///   depth of a finishing pass is reported, and the deflection
    ///   gate is its limit. `doc_finishing_factor` stays on the profile
    ///   but no cap reads it.
    ///
    /// The factor follows the family first and the pass role second,
    /// which is the order `feeds::suggest::invariants::clamp_dpp_to_rigidity`
    /// already used: an adaptive pass is a deep, narrow strategy whose
    /// axial ceiling is deliberately far above the conventional one, so
    /// the family decides before the role does.
    ///
    /// | Family / role | Factor |
    /// |---|---|
    /// | `Drill` | none |
    /// | `Adaptive` | `adaptive_doc_factor` |
    /// | any other, `Roughing` | `doc_roughing_factor` |
    /// | any other, `SemiFinish` or `Finish` | none (R2) |
    ///
    /// **This is the one producer of the cap.** The Suggest clamp calls
    /// it for its roughing branch and the post-simulation depth
    /// criterion (`tool_load::depth`) calls it for the bound it judges
    /// against, so the number the operator's recipe was lowered to and
    /// the number the row draws against are the same number. Both
    /// callers pass `diameter_mm` from the one function
    /// `crate::feeds::geometry::depth_cap_diameter_mm` at their own
    /// depth (the shipped depth and the measured peak).
    #[must_use]
    pub fn depth_cap_mm(
        &self,
        family: crate::feeds::OperationFamily,
        pass_role: crate::feeds::PassRole,
        diameter_mm: f64,
    ) -> Option<RigidityDepthCap> {
        use crate::feeds::{OperationFamily, PassRole};
        let factor = match (family, pass_role) {
            (OperationFamily::Drill, _) => return None,
            (OperationFamily::Adaptive, _) => self.adaptive_doc_factor,
            (_, PassRole::Roughing) => self.doc_roughing_factor,
            // Feeds matrix R2 (2026-09-23): a finishing or semi-finishing
            // pass has no axial ceiling. Its depth is reported, and the
            // deflection gate decides.
            (_, PassRole::SemiFinish | PassRole::Finish) => return None,
        };
        Some(RigidityDepthCap {
            factor,
            diameter_mm,
        })
    }
}

impl Default for RigidityProfile {
    fn default() -> Self {
        Self {
            doc_roughing_factor: 0.25,
            doc_finishing_factor: 0.10,
            woc_roughing_factor: 0.80,
            woc_roughing_max_mm: 6.35,
            woc_finishing_mm: 0.635,
            adaptive_doc_factor: 2.0,
            adaptive_woc_factor: 0.25,
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

/// F002: the ceiling of a CUTTING feed, mm/min:
/// `max_cutting_feed_mm_min` when set, else
/// `min(travel, DEFAULT_CUTTING_FEED_CAP_MM_MIN)`, never above the travel
/// rate. Only [`MachineProfile::cutting_ceiling`] makes one; the field is
/// private, so a travel rate cannot pose as a cutting cap.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct CuttingCeilingMmMin(f64);

impl CuttingCeilingMmMin {
    /// The ceiling, mm/min.
    #[must_use]
    pub const fn mm_min(self) -> f64 {
        self.0
    }

    /// The ceiling with a floor of `floor_mm_min` (the `.max(1.0)` guard the
    /// sim and the advisor take against a zero profile).
    #[must_use]
    pub const fn at_least(self, floor_mm_min: f64) -> Self {
        Self(self.0.max(floor_mm_min))
    }
}

/// Complete machine profile for feeds calculation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    /// `None` derives the conservative default
    /// `min(travel, DEFAULT_CUTTING_FEED_CAP_MM_MIN)`. Set explicitly
    /// on rigid machines that genuinely cut faster.
    #[serde(default)]
    pub max_cutting_feed_mm_min: Option<f64>,
    pub max_shank_mm: f64,
    pub rigidity: RigidityProfile,
    /// The machine aggressiveness dial (ruling R4, 2026-09-24). It sets the
    /// LOAD of a Suggest recipe as a fraction of the load at the base
    /// engagement. The chipload does not move: Suggest makes the depth per
    /// pass and the stepover smaller (one common scale) until the predicted
    /// lateral force and spindle power are at or below this fraction of
    /// their base values. See `feeds::suggest::aggressiveness`.
    ///
    /// - Default [`DEFAULT_AGGRESSIVENESS`] (0.85, operator ruling Q1).
    /// - Range for the panel slider: [`AGGRESSIVENESS_MIN`] to
    ///   [`AGGRESSIVENESS_MAX`] (0.50 to 1.50).
    /// - 1.00: no dial action. Above 1.00 Suggest raises the engagement
    ///   inside the existing clamps and files a Caution (ruling Q3: warn,
    ///   do not refuse).
    ///
    /// It is not a feed factor and not a power fraction. The spindle power
    /// ceiling is the rated curve [`Self::power_at_rpm`] (ruling Q2).
    ///
    /// Serde: a file without the key loads the default. The old key
    /// `safety_factor` (a feed factor of 0.75 or 0.80) refuses on load
    /// (F034, `deny_unknown_fields`; the project loader names it through
    /// `RETIRED_KEYS`); there is no alias (ruling 2026-09-16).
    #[serde(default = "default_aggressiveness")]
    pub aggressiveness: f64,
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

/// The default machine aggressiveness (operator ruling R4 Q1, 2026-09-24).
pub const DEFAULT_AGGRESSIVENESS: f64 = 0.85;

/// The lowest aggressiveness the machine panel offers.
pub const AGGRESSIVENESS_MIN: f64 = 0.50;

/// The highest aggressiveness the machine panel offers. Values above 1.00
/// raise the engagement above the base and file a Caution (ruling Q3).
pub const AGGRESSIVENESS_MAX: f64 = 1.50;

fn default_aggressiveness() -> f64 {
    DEFAULT_AGGRESSIVENESS
}

/// F4 — conservative cutting-feed cap used when a profile doesn't set
/// `max_cutting_feed_mm_min` explicitly. 6 000 mm/min sits above every
/// vendor wood-routing recommendation in the embedded literature net
/// (Onsrud industrial charts top out near 5 000–6 000 for the machine
/// classes we model) while staying far under gantry travel rates
/// (10 000+). A machine that genuinely cuts faster declares it.
pub const DEFAULT_CUTTING_FEED_CAP_MM_MIN: f64 = 6000.0;

impl MachineProfile {
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
        D::Aggressiveness.check(self.aggressiveness, None)?;
        if let Some(kinematics) = &self.kinematics {
            kinematics.validate()?;
        }
        Ok(())
    }

    /// F4 — the feed ceiling for CUTTING moves. Explicit
    /// `max_cutting_feed_mm_min` when set, else
    /// `min(travel, DEFAULT_CUTTING_FEED_CAP_MM_MIN)`. Never exceeds
    /// the travel rate — a cut can't outrun the axes.
    pub fn cutting_feed_ceiling_mm_min(&self) -> f64 {
        self.cutting_ceiling().mm_min()
    }

    /// F002: the cutting-feed ceiling as its own type. A consumer that caps
    /// a CUT (the modulator) takes [`CuttingCeilingMmMin`], so the travel
    /// rate cannot reach it by mistake.
    #[must_use]
    pub fn cutting_ceiling(&self) -> CuttingCeilingMmMin {
        CuttingCeilingMmMin(
            self.max_cutting_feed_mm_min
                .unwrap_or(DEFAULT_CUTTING_FEED_CAP_MM_MIN)
                .min(self.max_feed_mm_min),
        )
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
            rigidity: RigidityProfile {
                doc_roughing_factor: 0.20,
                doc_finishing_factor: 0.08,
                woc_roughing_factor: 0.70,
                woc_roughing_max_mm: 5.0,
                woc_finishing_mm: 0.50,
                adaptive_doc_factor: 1.5,
                adaptive_woc_factor: 0.20,
            },
            aggressiveness: DEFAULT_AGGRESSIVENESS,
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
            rigidity: RigidityProfile::default(),
            // Until ruling R4 (2026-09-24) the Shapeoko presets carried a
            // FEED factor of 0.80 here. The dial is a load target, not a
            // feed factor, so 0.80 has no meaning under it; the presets
            // take the default.
            aggressiveness: DEFAULT_AGGRESSIVENESS,
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
            rigidity: RigidityProfile::default(),
            // Until ruling R4 (2026-09-24) the Shapeoko presets carried a
            // FEED factor of 0.80 here. The dial is a load target, not a
            // feed factor, so 0.80 has no meaning under it; the presets
            // take the default.
            aggressiveness: DEFAULT_AGGRESSIVENESS,
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

    /// F4 — the validation case from the defect-class tracker: a
    /// profile with 10 000 mm/min travel ($110-class, the ricky-XXL
    /// setup) must NOT expose 10 000 as the cutting ceiling — the
    /// derived default caps cutting at 6 000 while rapids keep the
    /// full travel rate. An explicit `max_cutting_feed_mm_min` wins,
    /// but can never exceed travel.
    #[test]
    fn cutting_ceiling_split_from_travel_rate() {
        let mut m = MachineProfile::generic_wood_router();
        m.max_feed_mm_min = 10_000.0;
        assert!(
            (m.cutting_feed_ceiling_mm_min() - DEFAULT_CUTTING_FEED_CAP_MM_MIN).abs() < 1e-9,
            "10k-travel profile must derive the conservative cutting cap"
        );

        // Explicit ceiling wins.
        m.max_cutting_feed_mm_min = Some(8_000.0);
        assert!((m.cutting_feed_ceiling_mm_min() - 8_000.0).abs() < 1e-9);

        // ...but never exceeds travel.
        m.max_feed_mm_min = 4_000.0;
        assert!((m.cutting_feed_ceiling_mm_min() - 4_000.0).abs() < 1e-9);

        // Built-ins (travel ≤ cap) keep pre-F4 behavior exactly.
        let preset = MachineProfile::shapeoko_vfd();
        assert!((preset.cutting_feed_ceiling_mm_min() - preset.max_feed_mm_min).abs() < 1e-9);
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

    /// Ruling R4 (2026-09-24): every preset carries the default dial, and
    /// the default sits inside the panel range.
    #[test]
    fn every_preset_carries_the_default_aggressiveness() {
        assert!((AGGRESSIVENESS_MIN..=AGGRESSIVENESS_MAX).contains(&DEFAULT_AGGRESSIVENESS));
        for (_, p) in MachineProfile::presets() {
            assert_eq!(p.aggressiveness, DEFAULT_AGGRESSIVENESS);
        }
    }

    /// Ruling R4 (2026-09-24) and F034: a profile written with the old
    /// key `safety_factor` refuses (no key is ignored). A profile with
    /// neither key takes the default.
    #[test]
    fn the_old_safety_factor_key_is_refused_on_load() {
        let mut value = serde_json::to_value(MachineProfile::generic_wood_router()).unwrap();
        let obj = value.as_object_mut().unwrap();
        obj.remove("aggressiveness");
        let defaulted: MachineProfile = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(defaulted.aggressiveness, DEFAULT_AGGRESSIVENESS);
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
