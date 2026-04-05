//! Machine profile definitions for feeds & speeds calculation.
//!
//! Defines spindle configuration, power model, rigidity factors, and
//! machine presets. Ported from reference/shapeoko_feeds_and_speeds/src/machine_profile.rs.

use serde::{Deserialize, Serialize};

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
    pub max_feed_mm_min: f64,
    pub max_shank_mm: f64,
    pub rigidity: RigidityProfile,
    pub safety_factor: f64,
}

impl Default for MachineProfile {
    fn default() -> Self {
        Self::generic_wood_router()
    }
}

impl MachineProfile {
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
            max_shank_mm: 7.0,
            rigidity: RigidityProfile::default(),
            safety_factor: 0.80,
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
            max_shank_mm: 6.35,
            rigidity: RigidityProfile::default(),
            safety_factor: 0.80,
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

    /// Serialization key for TOML project files.
    pub fn to_key(&self) -> String {
        if self.name.contains("VFD") {
            "shapeoko_vfd".to_owned()
        } else if self.name.contains("Makita") {
            "shapeoko_makita".to_owned()
        } else {
            "generic".to_owned()
        }
    }

    /// Parse from TOML key.
    pub fn from_key(key: &str) -> Self {
        match key {
            "shapeoko_vfd" => Self::shapeoko_vfd(),
            "shapeoko_makita" => Self::shapeoko_makita(),
            _ => Self::generic_wood_router(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

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
    fn test_key_roundtrip() {
        for (_, profile) in MachineProfile::presets() {
            let key = profile.to_key();
            let restored = MachineProfile::from_key(&key);
            assert_eq!(
                profile.name, restored.name,
                "roundtrip failed for key '{key}'"
            );
        }
    }
}
