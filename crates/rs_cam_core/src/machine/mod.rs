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
    /// The profile's own factor for this family: `doc_roughing_factor`,
    /// `doc_finishing_factor` or `adaptive_doc_factor`.
    pub factor: f64,
    /// The tool diameter the factor multiplies, in mm.
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
    /// `None` for a family with no axial cap. Today that is
    /// [`OperationFamily::Drill`] alone: a drill cycle is Z-only, so
    /// there is no radial engagement and no axial rule of thumb to
    /// judge it by. A drill operation also carries no
    /// `depth_per_pass`, so the Suggest clamp never reaches this arm.
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
    /// | any other, `SemiFinish` or `Finish` | `doc_finishing_factor` |
    ///
    /// **This is the one producer of the cap.** The Suggest clamp calls
    /// it for its roughing branch and the post-simulation depth
    /// criterion (`tool_load::depth`) calls it for the bound it judges
    /// against, so the number the operator's recipe was lowered to and
    /// the number the row draws against are the same number.
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
            // The profile publishes two milling factors, and the split
            // is roughing against not-roughing. A semi-finish pass is
            // not roughing, so it reads the finishing factor.
            (_, PassRole::SemiFinish | PassRole::Finish) => self.doc_finishing_factor,
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
    pub safety_factor: f64,
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

    /// T-18 — the cutting-feed ceiling stated on the COMMANDED axis:
    /// [`Self::cutting_feed_ceiling_mm_min`] × `safety_factor`.
    ///
    /// `feeds::calculate` names two axes (the F-2 block in `feeds/mod.rs`):
    ///
    /// - RAW axis — the feed before Step 9.
    /// - COMMANDED axis — the feed the machine receives, after Step 9.
    ///
    /// Step 7 caps the RAW feed at [`Self::cutting_feed_ceiling_mm_min`] with
    /// no factor. Step 9 then multiplies the feed by `safety_factor`. The
    /// calculator's output invariant is therefore
    ///
    /// ```text
    /// commanded_feed <= cutting_feed_ceiling_mm_min() * safety_factor
    /// ```
    ///
    /// and this function is the right-hand side. Every clamp that runs AFTER
    /// Step 9 — the Step 9b rubbing-floor lift, the Step 9c drill envelope
    /// clamp, and Suggest pass 9 in `feeds/suggest/adaptive_entry.rs` — reads
    /// this value. A clamp that reads `max_feed_mm_min * safety_factor`
    /// instead caps a CUTTING feed against a fraction of the gantry TRAVEL
    /// rate, which is a different quantity (T-18).
    ///
    /// On every shipped preset the travel rate sits under
    /// [`DEFAULT_CUTTING_FEED_CAP_MM_MIN`], so the ceiling equals the travel
    /// rate and this value equals `max_feed_mm_min * safety_factor`. The two
    /// separate on a profile with an explicit `max_cutting_feed_mm_min` below
    /// travel, or with a gantry faster than the default cap.
    pub fn commanded_cutting_feed_ceiling_mm_min(&self) -> f64 {
        self.cutting_feed_ceiling_mm_min() * self.safety_factor
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
            safety_factor: 0.75,
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
            safety_factor: 0.80,
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
            safety_factor: 0.80,
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

    /// T-18 — the COMMANDED-axis ceiling. On every shipped preset the
    /// travel rate sits under the default cutting cap, so the ceiling equals
    /// the travel rate and the commanded ceiling equals
    /// `max_feed_mm_min * safety_factor`. A post-Step-9 clamp therefore moves
    /// no shipped number when it swaps one expression for the other. The two
    /// separate on a fast gantry with a slow cutting ceiling.
    #[test]
    fn commanded_cutting_ceiling_is_the_ceiling_after_the_safety_factor() {
        for preset in [
            MachineProfile::generic_wood_router(),
            MachineProfile::shapeoko_vfd(),
            MachineProfile::shapeoko_makita(),
        ] {
            assert!(
                (preset.cutting_feed_ceiling_mm_min() - preset.max_feed_mm_min).abs() < 1e-9,
                "{}: travel sits under the default cutting cap",
                preset.name
            );
            assert!(
                (preset.commanded_cutting_feed_ceiling_mm_min()
                    - preset.max_feed_mm_min * preset.safety_factor)
                    .abs()
                    < 1e-9,
                "{}: the commanded ceiling must equal the travel rate after the safety \
                 factor on a preset",
                preset.name
            );
        }

        let mut fast_gantry = MachineProfile::generic_wood_router();
        fast_gantry.max_feed_mm_min = 10_000.0;
        fast_gantry.max_cutting_feed_mm_min = Some(500.0);
        fast_gantry.safety_factor = 0.8;
        assert!((fast_gantry.commanded_cutting_feed_ceiling_mm_min() - 400.0).abs() < 1e-9);
        assert!(
            (fast_gantry.max_feed_mm_min * fast_gantry.safety_factor - 8_000.0).abs() < 1e-9,
            "the travel-rate expression is the quantity T-18 replaced"
        );
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

    #[test]
    fn test_safety_factor_range() {
        for (_, p) in MachineProfile::presets() {
            assert!(p.safety_factor >= 0.5 && p.safety_factor <= 1.0);
        }
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
