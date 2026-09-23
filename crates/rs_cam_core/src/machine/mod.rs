//! Machine profile definitions for feeds & speeds calculation.
//!
//! Defines spindle configuration, power model, rigidity factors, and
//! machine presets. Ported from reference/shapeoko_feeds_and_speeds/src/machine_profile.rs.
//!
//! The folder holds the machine model: this profile, the kinematics, the
//! utilisation instrument and the strategy advisor that reads them.

pub mod kinematic_utilization;
pub mod kinematics;
pub mod strategy_advisor;

use serde::{Deserialize, Serialize};

use crate::machine::kinematics::MachineKinematics;

/// Spindle speed control type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpindleConfig {
    Variable { min_rpm: f64, max_rpm: f64 },
    Discrete { speeds: Vec<f64> },
}

/// Spindle power model.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PowerModel {
    /// VFD: constant torque up to rated RPM, power scales linearly with RPM.
    VfdConstantTorque { rated_power_kw: f64, rated_rpm: f64 },
    /// Router-type: roughly constant power across RPM range.
    ConstantPower { power_kw: f64 },
}

/// Chip load formula parameters derived from empirical data.
/// ChipLoad = K0 * D^p * (1/H)^q
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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
    /// - [`OperationFamily::Drill`]: a drill cycle is Z-only, so there
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

/// Complete machine profile for feeds calculation.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// [`Self::cutting_feed_ceiling_mm_min`]. (Field name kept for
    /// serde compatibility with existing project files and
    /// machine-library TOMLs.)
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
    /// `safety_factor` (a feed factor of 0.75 or 0.80) is ignored on load;
    /// there is no alias (ruling 2026-09-16, "no legacy, breaking OK").
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
    /// F4 — the feed ceiling for CUTTING moves. Explicit
    /// `max_cutting_feed_mm_min` when set, else
    /// `min(travel, DEFAULT_CUTTING_FEED_CAP_MM_MIN)`. Never exceeds
    /// the travel rate — a cut can't outrun the axes.
    pub fn cutting_feed_ceiling_mm_min(&self) -> f64 {
        self.max_cutting_feed_mm_min
            .unwrap_or(DEFAULT_CUTTING_FEED_CAP_MM_MIN)
            .min(self.max_feed_mm_min)
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
                // SAFETY: speeds is non-empty by construction; mid-index is always valid
                #[allow(clippy::indexing_slicing)]
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

    /// Parse a preset key. Unknown keys fall back to the generic profile.
    ///
    /// **Test door.** The literature-matrix shim
    /// (`crates/rs_cam_core/tests/literature_matrix/shim.rs`) is the only
    /// caller; it maps a cell's `machine_class` string to a profile. No
    /// production path reads it. The viz legacy project loader that also
    /// read a persisted `machine = "<key>"` line is gone, so the key is
    /// a test input now, not a file format.
    pub fn from_key(key: &str) -> Self {
        match key {
            "shapeoko_vfd" => Self::shapeoko_vfd(),
            "shapeoko_makita" => Self::shapeoko_makita(),
            _ => Self::generic_wood_router(),
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
    /// (`from_key` above is a test door); structural equality
    /// (same serde-JSON form) is the honest preset test.
    pub fn matching_preset_index(&self) -> Option<usize> {
        let self_json = serde_json::to_string(self).ok()?;
        Self::presets()
            .iter()
            .position(|(_, p)| serde_json::to_string(p).ok().as_deref() == Some(&self_json))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
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

    /// Ruling R4 (2026-09-24): a profile written with the old key
    /// `safety_factor` loads, the key is ignored, and the dial takes the
    /// default. A profile with neither key also takes the default.
    #[test]
    fn the_old_safety_factor_key_is_ignored_on_load() {
        let mut value = serde_json::to_value(MachineProfile::generic_wood_router()).unwrap();
        let obj = value.as_object_mut().unwrap();
        obj.remove("aggressiveness");
        obj.insert("safety_factor".to_owned(), serde_json::json!(0.75));
        let loaded: MachineProfile = serde_json::from_value(value).unwrap();
        assert_eq!(loaded.aggressiveness, DEFAULT_AGGRESSIVENESS);
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
