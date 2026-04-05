//! Material definitions for feeds & speeds calculation.
//!
//! Provides material hardness index and specific cutting force (Kc) values
//! used by the feeds calculator to determine chip load, feed rate, and power.
//! Ported from reference/shapeoko_feeds_and_speeds/src/params/mod.rs.

use serde::{Deserialize, Serialize};

/// Wood species with Janka hardness data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WoodSpecies {
    GenericSoftwood,
    RadiataPine,
    SouthernYellowPine,
    GenericHardwood,
    HardMaple,
    Walnut,
    Birch,
    WhiteOak,
    Jarrah,
    Ipe,
}

impl WoodSpecies {
    pub fn janka_lbf(self) -> f64 {
        match self {
            WoodSpecies::GenericSoftwood => 600.0,
            WoodSpecies::RadiataPine => 500.0,
            WoodSpecies::SouthernYellowPine => 690.0,
            WoodSpecies::GenericHardwood => 1450.0,
            WoodSpecies::HardMaple => 1450.0,
            WoodSpecies::Walnut => 1010.0,
            WoodSpecies::Birch => 1260.0,
            WoodSpecies::WhiteOak => 1360.0,
            WoodSpecies::Jarrah => 1910.0,
            WoodSpecies::Ipe => 3510.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            WoodSpecies::GenericSoftwood => "Generic Softwood",
            WoodSpecies::RadiataPine => "Radiata Pine",
            WoodSpecies::SouthernYellowPine => "Southern Yellow Pine",
            WoodSpecies::GenericHardwood => "Generic Hardwood",
            WoodSpecies::HardMaple => "Hard Maple",
            WoodSpecies::Walnut => "Walnut",
            WoodSpecies::Birch => "Birch",
            WoodSpecies::WhiteOak => "White Oak",
            WoodSpecies::Jarrah => "Jarrah",
            WoodSpecies::Ipe => "Ipe",
        }
    }
}

/// Plywood grade affecting effective hardness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlywoodGrade {
    Softwood,
    BalticBirch,
    HardwoodFaced,
}

impl PlywoodGrade {
    fn effective_janka_lbf(self) -> f64 {
        match self {
            PlywoodGrade::Softwood => 600.0,
            PlywoodGrade::BalticBirch => 1200.0,
            PlywoodGrade::HardwoodFaced => 1000.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PlywoodGrade::Softwood => "Softwood Plywood",
            PlywoodGrade::BalticBirch => "Baltic Birch",
            PlywoodGrade::HardwoodFaced => "Hardwood Faced",
        }
    }
}

/// Sheet good kinds (MDF, HDF, particleboard).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SheetGoodKind {
    Mdf,
    Hdf,
    Particleboard,
}

impl SheetGoodKind {
    fn effective_janka_lbf(self) -> f64 {
        match self {
            SheetGoodKind::Mdf => 1100.0,
            SheetGoodKind::Hdf => 1300.0,
            SheetGoodKind::Particleboard => 750.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SheetGoodKind::Mdf => "MDF",
            SheetGoodKind::Hdf => "HDF",
            SheetGoodKind::Particleboard => "Particleboard",
        }
    }
}

/// Plastic family for router work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlasticFamily {
    Generic,
    Acrylic,
    Hdpe,
    Delrin,
    Polycarbonate,
}

impl PlasticFamily {
    pub fn label(self) -> &'static str {
        match self {
            PlasticFamily::Generic => "Generic Plastic",
            PlasticFamily::Acrylic => "Acrylic",
            PlasticFamily::Hdpe => "HDPE",
            PlasticFamily::Delrin => "Delrin",
            PlasticFamily::Polycarbonate => "Polycarbonate",
        }
    }
}

/// Foam density for sign-making and prototyping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FoamDensity {
    Low,
    Medium,
    High,
}

impl FoamDensity {
    pub fn label(self) -> &'static str {
        match self {
            FoamDensity::Low => "Low Density",
            FoamDensity::Medium => "Medium Density",
            FoamDensity::High => "High Density (Renshape)",
        }
    }
}

/// Material being cut. Determines chip load scaling and power requirements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Material {
    SolidWood {
        species: WoodSpecies,
    },
    Plywood {
        grade: PlywoodGrade,
    },
    SheetGood {
        kind: SheetGoodKind,
    },
    Plastic {
        family: PlasticFamily,
    },
    Foam {
        density: FoamDensity,
    },
    Custom {
        name: String,
        hardness_index: f64,
        kc: f64,
    },
}

impl Default for Material {
    fn default() -> Self {
        Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        }
    }
}

impl Material {
    /// Normalized hardness index. 1.0 = soft wood baseline (Janka 600 lbf).
    /// Formula: (Janka / 600)^0.4
    pub fn hardness_index(&self) -> f64 {
        match self {
            Material::SolidWood { species } => (species.janka_lbf() / 600.0).powf(0.4),
            Material::Plywood { grade } => (grade.effective_janka_lbf() / 600.0).powf(0.4),
            Material::SheetGood { kind } => (kind.effective_janka_lbf() / 600.0).powf(0.4),
            Material::Plastic { .. } => 0.5,
            Material::Foam { density } => match density {
                FoamDensity::Low => 0.15,
                FoamDensity::Medium => 0.25,
                FoamDensity::High => 0.40,
            },
            Material::Custom { hardness_index, .. } => *hardness_index,
        }
    }

    /// Specific cutting force in N/mm². Used for power calculation.
    pub fn kc_n_per_mm2(&self) -> f64 {
        match self {
            Material::SolidWood { species } => match species {
                WoodSpecies::GenericSoftwood => 6.0,
                WoodSpecies::RadiataPine => 6.0,
                WoodSpecies::SouthernYellowPine => 7.0,
                WoodSpecies::GenericHardwood => 14.0,
                WoodSpecies::HardMaple => 15.0,
                WoodSpecies::Walnut => 12.0,
                WoodSpecies::Birch => 13.0,
                WoodSpecies::WhiteOak => 14.0,
                WoodSpecies::Jarrah => 19.0,
                WoodSpecies::Ipe => 28.0,
            },
            Material::Plywood { grade } => match grade {
                PlywoodGrade::Softwood => 8.0,
                PlywoodGrade::BalticBirch => 13.0,
                PlywoodGrade::HardwoodFaced => 11.0,
            },
            Material::SheetGood { kind } => match kind {
                SheetGoodKind::Mdf => 10.0,
                SheetGoodKind::Hdf => 12.0,
                SheetGoodKind::Particleboard => 9.0,
            },
            Material::Plastic { .. } => 4.0,
            Material::Foam { density } => match density {
                FoamDensity::Low => 1.0,
                FoamDensity::Medium => 2.0,
                FoamDensity::High => 3.0,
            },
            Material::Custom { kc, .. } => *kc,
        }
    }

    /// Recommended base cutting surface speed in m/min.
    /// Used to derive initial RPM from tool diameter.
    pub fn base_cutting_speed_m_min(&self) -> f64 {
        match self {
            Material::SolidWood { .. } => 200.0,
            Material::Plywood { .. } => 180.0,
            Material::SheetGood { .. } => 170.0,
            Material::Plastic { .. } => 250.0,
            Material::Foam { .. } => 300.0,
            Material::Custom { .. } => 200.0,
        }
    }

    /// Base plunge feed rate estimate in mm/min.
    /// Material-dependent; divided by hardness for wood-like materials.
    pub fn plunge_rate_base(&self) -> f64 {
        let h = self.hardness_index();
        match self {
            Material::SolidWood { .. } => 1000.0 / h,
            Material::Plywood { .. } | Material::SheetGood { .. } => 900.0 / h,
            Material::Plastic { .. } => 1500.0,
            Material::Foam { .. } => 2000.0,
            Material::Custom { .. } => 800.0 / h,
        }
    }

    /// Display label for UI.
    pub fn label(&self) -> String {
        match self {
            Material::SolidWood { species } => species.label().to_string(),
            Material::Plywood { grade } => grade.label().to_string(),
            Material::SheetGood { kind } => kind.label().to_string(),
            Material::Plastic { family } => family.label().to_string(),
            Material::Foam { density } => format!("Foam ({})", density.label()),
            Material::Custom { name, .. } => name.clone(),
        }
    }

    /// Catalog of common materials for UI dropdowns.
    pub fn catalog() -> Vec<(&'static str, Material)> {
        vec![
            // Wood
            (
                "Softwood (Pine/Spruce)",
                Material::SolidWood {
                    species: WoodSpecies::GenericSoftwood,
                },
            ),
            (
                "Radiata Pine",
                Material::SolidWood {
                    species: WoodSpecies::RadiataPine,
                },
            ),
            (
                "Southern Yellow Pine",
                Material::SolidWood {
                    species: WoodSpecies::SouthernYellowPine,
                },
            ),
            (
                "Hardwood (Generic)",
                Material::SolidWood {
                    species: WoodSpecies::GenericHardwood,
                },
            ),
            (
                "Hard Maple",
                Material::SolidWood {
                    species: WoodSpecies::HardMaple,
                },
            ),
            (
                "Walnut",
                Material::SolidWood {
                    species: WoodSpecies::Walnut,
                },
            ),
            (
                "Birch",
                Material::SolidWood {
                    species: WoodSpecies::Birch,
                },
            ),
            (
                "White Oak",
                Material::SolidWood {
                    species: WoodSpecies::WhiteOak,
                },
            ),
            (
                "Jarrah",
                Material::SolidWood {
                    species: WoodSpecies::Jarrah,
                },
            ),
            (
                "Ipe",
                Material::SolidWood {
                    species: WoodSpecies::Ipe,
                },
            ),
            // Plywood
            (
                "Softwood Plywood",
                Material::Plywood {
                    grade: PlywoodGrade::Softwood,
                },
            ),
            (
                "Baltic Birch Plywood",
                Material::Plywood {
                    grade: PlywoodGrade::BalticBirch,
                },
            ),
            (
                "Hardwood Faced Plywood",
                Material::Plywood {
                    grade: PlywoodGrade::HardwoodFaced,
                },
            ),
            // Sheet goods
            (
                "MDF",
                Material::SheetGood {
                    kind: SheetGoodKind::Mdf,
                },
            ),
            (
                "HDF",
                Material::SheetGood {
                    kind: SheetGoodKind::Hdf,
                },
            ),
            (
                "Particleboard",
                Material::SheetGood {
                    kind: SheetGoodKind::Particleboard,
                },
            ),
            // Plastic
            (
                "Acrylic",
                Material::Plastic {
                    family: PlasticFamily::Acrylic,
                },
            ),
            (
                "HDPE",
                Material::Plastic {
                    family: PlasticFamily::Hdpe,
                },
            ),
            (
                "Delrin",
                Material::Plastic {
                    family: PlasticFamily::Delrin,
                },
            ),
            (
                "Polycarbonate",
                Material::Plastic {
                    family: PlasticFamily::Polycarbonate,
                },
            ),
            // Foam
            (
                "Foam (Low Density)",
                Material::Foam {
                    density: FoamDensity::Low,
                },
            ),
            (
                "Foam (Medium Density)",
                Material::Foam {
                    density: FoamDensity::Medium,
                },
            ),
            (
                "Foam (High Density)",
                Material::Foam {
                    density: FoamDensity::High,
                },
            ),
        ]
    }

    /// Serialization key for TOML project files.
    pub fn to_key(&self) -> String {
        match self {
            Material::SolidWood { species } => match species {
                WoodSpecies::GenericSoftwood => "softwood",
                WoodSpecies::RadiataPine => "radiata_pine",
                WoodSpecies::SouthernYellowPine => "southern_yellow_pine",
                WoodSpecies::GenericHardwood => "hardwood",
                WoodSpecies::HardMaple => "hard_maple",
                WoodSpecies::Walnut => "walnut",
                WoodSpecies::Birch => "birch",
                WoodSpecies::WhiteOak => "white_oak",
                WoodSpecies::Jarrah => "jarrah",
                WoodSpecies::Ipe => "ipe",
            }
            .to_string(),
            Material::Plywood { grade } => match grade {
                PlywoodGrade::Softwood => "plywood_softwood",
                PlywoodGrade::BalticBirch => "baltic_birch",
                PlywoodGrade::HardwoodFaced => "plywood_hardwood",
            }
            .to_string(),
            Material::SheetGood { kind } => match kind {
                SheetGoodKind::Mdf => "mdf",
                SheetGoodKind::Hdf => "hdf",
                SheetGoodKind::Particleboard => "particleboard",
            }
            .to_string(),
            Material::Plastic { family } => match family {
                PlasticFamily::Generic => "plastic",
                PlasticFamily::Acrylic => "acrylic",
                PlasticFamily::Hdpe => "hdpe",
                PlasticFamily::Delrin => "delrin",
                PlasticFamily::Polycarbonate => "polycarbonate",
            }
            .to_string(),
            Material::Foam { density } => match density {
                FoamDensity::Low => "foam_low",
                FoamDensity::Medium => "foam_medium",
                FoamDensity::High => "foam_high",
            }
            .to_string(),
            Material::Custom { name, .. } => format!("custom:{name}"),
        }
    }

    /// Parse from TOML key. Returns default softwood if unrecognized.
    pub fn from_key(key: &str) -> Self {
        match key {
            "softwood" => Material::SolidWood {
                species: WoodSpecies::GenericSoftwood,
            },
            "radiata_pine" => Material::SolidWood {
                species: WoodSpecies::RadiataPine,
            },
            "southern_yellow_pine" => Material::SolidWood {
                species: WoodSpecies::SouthernYellowPine,
            },
            "hardwood" => Material::SolidWood {
                species: WoodSpecies::GenericHardwood,
            },
            "hard_maple" => Material::SolidWood {
                species: WoodSpecies::HardMaple,
            },
            "walnut" => Material::SolidWood {
                species: WoodSpecies::Walnut,
            },
            "birch" => Material::SolidWood {
                species: WoodSpecies::Birch,
            },
            "white_oak" => Material::SolidWood {
                species: WoodSpecies::WhiteOak,
            },
            "jarrah" => Material::SolidWood {
                species: WoodSpecies::Jarrah,
            },
            "ipe" => Material::SolidWood {
                species: WoodSpecies::Ipe,
            },
            "plywood_softwood" => Material::Plywood {
                grade: PlywoodGrade::Softwood,
            },
            "baltic_birch" => Material::Plywood {
                grade: PlywoodGrade::BalticBirch,
            },
            "plywood_hardwood" => Material::Plywood {
                grade: PlywoodGrade::HardwoodFaced,
            },
            "mdf" => Material::SheetGood {
                kind: SheetGoodKind::Mdf,
            },
            "hdf" => Material::SheetGood {
                kind: SheetGoodKind::Hdf,
            },
            "particleboard" => Material::SheetGood {
                kind: SheetGoodKind::Particleboard,
            },
            "plastic" => Material::Plastic {
                family: PlasticFamily::Generic,
            },
            "acrylic" => Material::Plastic {
                family: PlasticFamily::Acrylic,
            },
            "hdpe" => Material::Plastic {
                family: PlasticFamily::Hdpe,
            },
            "delrin" => Material::Plastic {
                family: PlasticFamily::Delrin,
            },
            "polycarbonate" => Material::Plastic {
                family: PlasticFamily::Polycarbonate,
            },
            "foam_low" => Material::Foam {
                density: FoamDensity::Low,
            },
            "foam_medium" => Material::Foam {
                density: FoamDensity::Medium,
            },
            "foam_high" => Material::Foam {
                density: FoamDensity::High,
            },
            _ => Material::default(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_softwood_baseline_hardness_is_one() {
        let m = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        assert!((m.hardness_index() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_hardness_index_ordering() {
        let soft = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let hard = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let ipe = Material::SolidWood {
            species: WoodSpecies::Ipe,
        };
        assert!(soft.hardness_index() < hard.hardness_index());
        assert!(hard.hardness_index() < ipe.hardness_index());
    }

    #[test]
    fn test_kc_progression() {
        let soft = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let hard = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let ipe = Material::SolidWood {
            species: WoodSpecies::Ipe,
        };
        assert!(soft.kc_n_per_mm2() < hard.kc_n_per_mm2());
        assert!(hard.kc_n_per_mm2() < ipe.kc_n_per_mm2());
    }

    #[test]
    fn test_sheet_good_kc_progression() {
        let mdf = Material::SheetGood {
            kind: SheetGoodKind::Mdf,
        }
        .kc_n_per_mm2();
        let hdf = Material::SheetGood {
            kind: SheetGoodKind::Hdf,
        }
        .kc_n_per_mm2();
        let particle = Material::SheetGood {
            kind: SheetGoodKind::Particleboard,
        }
        .kc_n_per_mm2();
        assert!(hdf > mdf);
        assert!(mdf > particle);
    }

    #[test]
    fn test_foam_is_softer_than_wood() {
        let foam = Material::Foam {
            density: FoamDensity::High,
        };
        let soft_wood = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        assert!(foam.hardness_index() < soft_wood.hardness_index());
    }

    #[test]
    fn test_catalog_has_all_families() {
        let catalog = Material::catalog();
        assert!(
            catalog
                .iter()
                .any(|(_, m)| matches!(m, Material::SolidWood { .. }))
        );
        assert!(
            catalog
                .iter()
                .any(|(_, m)| matches!(m, Material::Plywood { .. }))
        );
        assert!(
            catalog
                .iter()
                .any(|(_, m)| matches!(m, Material::SheetGood { .. }))
        );
        assert!(
            catalog
                .iter()
                .any(|(_, m)| matches!(m, Material::Plastic { .. }))
        );
        assert!(
            catalog
                .iter()
                .any(|(_, m)| matches!(m, Material::Foam { .. }))
        );
    }

    #[test]
    fn test_key_roundtrip() {
        for (_, mat) in Material::catalog() {
            let key = mat.to_key();
            let restored = Material::from_key(&key);
            assert_eq!(mat, restored, "roundtrip failed for key '{key}'");
        }
    }

    #[test]
    fn test_hard_maple_hardness_matches_reference() {
        // Reference: (1450/600)^0.4 ≈ 1.425
        let m = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        assert!((m.hardness_index() - 1.425).abs() < 0.01);
    }
}
