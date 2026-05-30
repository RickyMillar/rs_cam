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
    LongleafPine,
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
            // 2026-05-30 Phase 2C: corrected from repo's 500 to 710
            // per Wood Database (verbatim quote in
            // planning/data_ingest_2026-05-29/hardness.md).
            WoodSpecies::RadiataPine => 710.0,
            // 2026-05-30 Phase 2C: renamed from SouthernYellowPine (a
            // trade group covering Longleaf/Loblolly/Shortleaf/Slash
            // pines, each with a different Janka). The Wood Database
            // Longleaf Pine entry is 870 lbf. When Loblolly Pine data
            // lands as its own Grade-A primary, add a separate
            // WoodSpecies::LoblollyPine variant.
            WoodSpecies::LongleafPine => 870.0,
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
            WoodSpecies::LongleafPine => "Longleaf Pine",
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

/// Plastic surface hardness, preserving the original measurement scale.
///
/// Shore D and Rockwell M are not interchangeable — they measure
/// different things on different ranges — so this enum keeps them
/// distinct rather than fabricating a cross-scale equivalence.
/// Downstream consumers that need a single scalar pick the conversion
/// appropriate for their use.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PlasticHardness {
    ShoreD(f64),
    RockwellM(f64),
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

    /// Surface hardness in its measured scale.
    ///
    /// Citations (per `planning/data_ingest_2026-05-29/hardness.md`):
    /// - HDPE: ISO 868 / Direct Plastics, Shore D 64.0.
    /// - Polycarbonate: ASTM D2240 / Treatstock, Shore D 80.0.
    /// - Delrin (POM-H): ASTM D2240 / Alro, Shore D 86.0.
    /// - Acrylic (PMMA): MakeItFrom, Rockwell M 93.0 — PMMA is
    ///   typically reported in Rockwell M; Shore D is not the standard
    ///   scale for it, so the value is exposed in its native scale.
    /// - Generic: no primary hardness datum (returns `None`).
    pub fn hardness(self) -> Option<PlasticHardness> {
        match self {
            PlasticFamily::Hdpe => Some(PlasticHardness::ShoreD(64.0)),
            PlasticFamily::Polycarbonate => Some(PlasticHardness::ShoreD(80.0)),
            PlasticFamily::Delrin => Some(PlasticHardness::ShoreD(86.0)),
            PlasticFamily::Acrylic => Some(PlasticHardness::RockwellM(93.0)),
            PlasticFamily::Generic => None,
        }
    }
}

/// Aluminum alloy variants. The two listed are the common machining
/// targets; per-alloy Brinell hardness is anchored to ASM/MatWeb.
///
/// Power and deflection gates refuse on aluminum until Phase 3 beat F
/// (Aluminum Kienzle `kc1.1 / mc` archival hunt) lands a primary-source
/// `kc1.1 + mc` pair — see `Material::kc_n_per_mm2`. The chipload gate
/// is independent of Kc and operates as soon as the vendor LUT carries
/// aluminum rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AluminumAlloy {
    /// 6061-T6 — general-purpose alloy. Brinell 95 (ASM, MatWeb).
    Alloy6061T6,
    /// 7075-T6 — aerospace alloy, harder. Brinell 150 (ASM, MatWeb).
    Alloy7075T6,
}

impl AluminumAlloy {
    /// Brinell hardness (HB). Sources: ASM Aerospace Specification
    /// Metals / MatWeb. These are the standard datapoints used in
    /// vendor feed/speed lookups for aluminum.
    pub fn brinell_hb(self) -> f64 {
        match self {
            AluminumAlloy::Alloy6061T6 => 95.0,
            AluminumAlloy::Alloy7075T6 => 150.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AluminumAlloy::Alloy6061T6 => "Aluminum 6061-T6",
            AluminumAlloy::Alloy7075T6 => "Aluminum 7075-T6",
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
    Aluminum {
        alloy: AluminumAlloy,
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
            // Aluminum is well outside wood's Janka baseline; the
            // hardness_index abstraction is a *wood-baseline* normaliser,
            // not a metals one. The values below are placeholders that
            // scale roughly with Brinell so any feed-rate derate that
            // happens to read hardness_index doesn't return 0. The
            // safety-critical gates that consume Kc refuse on aluminum
            // until Phase 3 beat F.
            Material::Aluminum { alloy } => (alloy.brinell_hb() / 60.0).powf(0.4),
            Material::Foam { density } => match density {
                FoamDensity::Low => 0.15,
                FoamDensity::Medium => 0.25,
                FoamDensity::High => 0.40,
            },
            Material::Custom { hardness_index, .. } => *hardness_index,
        }
    }

    /// Specific cutting force in N/mm². Used for power calculation.
    ///
    /// `None` for materials whose Kc has no primary measurement — the
    /// tool-load gates refuse via `UnmodeledReason::MaterialUnvalidated`
    /// rather than predicting force from a fabricated constant. This
    /// keeps the type system honest about which materials carry a
    /// validated cutting-force model.
    pub fn kc_n_per_mm2(&self) -> Option<f64> {
        match self {
            Material::SolidWood { species } => Some(match species {
                WoodSpecies::GenericSoftwood => 6.0,
                WoodSpecies::RadiataPine => 6.0,
                WoodSpecies::LongleafPine => 7.0,
                WoodSpecies::GenericHardwood => 14.0,
                WoodSpecies::HardMaple => 15.0,
                WoodSpecies::Walnut => 12.0,
                WoodSpecies::Birch => 13.0,
                WoodSpecies::WhiteOak => 14.0,
                WoodSpecies::Jarrah => 19.0,
                WoodSpecies::Ipe => 28.0,
            }),
            Material::Plywood { grade } => Some(match grade {
                PlywoodGrade::Softwood => 8.0,
                PlywoodGrade::BalticBirch => 13.0,
                PlywoodGrade::HardwoodFaced => 11.0,
            }),
            Material::SheetGood { kind } => Some(match kind {
                SheetGoodKind::Mdf => 10.0,
                SheetGoodKind::Hdf => 12.0,
                SheetGoodKind::Particleboard => 9.0,
            }),
            // Per-family plastics — only families with a fetched primary
            // measurement return Some. Others refuse via the gate's
            // `MaterialUnvalidated` arm rather than predicting force
            // from a fabricated constant. See `kc.md` in the
            // 2026-05-29 data-ingest for the citations.
            Material::Plastic { family } => match family {
                // Yang 2022 measured cutting-yield-stress midpoint of
                // 33.85–46.89 N/mm² for HDPE (DOI 10.3390/polym14010189).
                PlasticFamily::Hdpe => Some(40.0),
                // No fetched primary force study exists for PC, PMMA, or
                // POM — the historical generic 4.0 was a fabricated
                // baseline. Until a measurement lands, the gate refuses.
                PlasticFamily::Polycarbonate
                | PlasticFamily::Acrylic
                | PlasticFamily::Delrin
                | PlasticFamily::Generic => None,
            },
            // Aluminum refuses by default until Phase 3 beat F lands a
            // fetched-primary `kc1.1 + mc` Kienzle pair. Switching to
            // `Some(kc1.1 * h^(-mc))` at a representative chip thickness
            // is a one-line change once the constants land.
            Material::Aluminum { .. } => None,
            Material::Foam { density } => Some(match density {
                FoamDensity::Low => 1.0,
                FoamDensity::Medium => 2.0,
                FoamDensity::High => 3.0,
            }),
            Material::Custom { kc, .. } => {
                if kc.is_finite() && *kc > 0.0 {
                    Some(*kc)
                } else {
                    None
                }
            }
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
            // Aluminum on a wood router is application-edge; this is a
            // conservative SFM placeholder, not validated machining
            // guidance. Real cutting-data lookup should always come
            // from the vendor LUT for aluminum operations.
            Material::Aluminum { .. } => 100.0,
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
            // Conservative placeholder — aluminum plunge guidance
            // should come from the vendor LUT, not this default.
            Material::Aluminum { .. } => 250.0 / h,
            Material::Foam { .. } => 2000.0,
            Material::Custom { .. } => 800.0 / h,
        }
    }

    /// Display label for UI.
    pub fn label(&self) -> String {
        match self {
            Material::SolidWood { species } => species.label().to_owned(),
            Material::Plywood { grade } => grade.label().to_owned(),
            Material::SheetGood { kind } => kind.label().to_owned(),
            Material::Plastic { family } => family.label().to_owned(),
            Material::Aluminum { alloy } => alloy.label().to_owned(),
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
                "Longleaf Pine",
                Material::SolidWood {
                    species: WoodSpecies::LongleafPine,
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
            // Aluminum
            (
                "Aluminum 6061-T6",
                Material::Aluminum {
                    alloy: AluminumAlloy::Alloy6061T6,
                },
            ),
            (
                "Aluminum 7075-T6",
                Material::Aluminum {
                    alloy: AluminumAlloy::Alloy7075T6,
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
                WoodSpecies::LongleafPine => "longleaf_pine",
                WoodSpecies::GenericHardwood => "hardwood",
                WoodSpecies::HardMaple => "hard_maple",
                WoodSpecies::Walnut => "walnut",
                WoodSpecies::Birch => "birch",
                WoodSpecies::WhiteOak => "white_oak",
                WoodSpecies::Jarrah => "jarrah",
                WoodSpecies::Ipe => "ipe",
            }
            .to_owned(),
            Material::Plywood { grade } => match grade {
                PlywoodGrade::Softwood => "plywood_softwood",
                PlywoodGrade::BalticBirch => "baltic_birch",
                PlywoodGrade::HardwoodFaced => "plywood_hardwood",
            }
            .to_owned(),
            Material::SheetGood { kind } => match kind {
                SheetGoodKind::Mdf => "mdf",
                SheetGoodKind::Hdf => "hdf",
                SheetGoodKind::Particleboard => "particleboard",
            }
            .to_owned(),
            Material::Plastic { family } => match family {
                PlasticFamily::Generic => "plastic",
                PlasticFamily::Acrylic => "acrylic",
                PlasticFamily::Hdpe => "hdpe",
                PlasticFamily::Delrin => "delrin",
                PlasticFamily::Polycarbonate => "polycarbonate",
            }
            .to_owned(),
            Material::Aluminum { alloy } => match alloy {
                AluminumAlloy::Alloy6061T6 => "aluminum_6061_t6",
                AluminumAlloy::Alloy7075T6 => "aluminum_7075_t6",
            }
            .to_owned(),
            Material::Foam { density } => match density {
                FoamDensity::Low => "foam_low",
                FoamDensity::Medium => "foam_medium",
                FoamDensity::High => "foam_high",
            }
            .to_owned(),
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
            // Canonical key for the renamed variant.
            "longleaf_pine" => Material::SolidWood {
                species: WoodSpecies::LongleafPine,
            },
            // Legacy alias: 2026-05-30 Phase 2C renamed
            // `SouthernYellowPine` → `LongleafPine` because Janka is
            // per-species and SYP is a trade group. Preserves load-
            // compat for project files saved before the rename. Write
            // path emits only "longleaf_pine".
            "southern_yellow_pine" => Material::SolidWood {
                species: WoodSpecies::LongleafPine,
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
            "aluminum_6061_t6" => Material::Aluminum {
                alloy: AluminumAlloy::Alloy6061T6,
            },
            "aluminum_7075_t6" => Material::Aluminum {
                alloy: AluminumAlloy::Alloy7075T6,
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
        }
        .kc_n_per_mm2()
        .unwrap();
        let hard = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        }
        .kc_n_per_mm2()
        .unwrap();
        let ipe = Material::SolidWood {
            species: WoodSpecies::Ipe,
        }
        .kc_n_per_mm2()
        .unwrap();
        assert!(soft < hard);
        assert!(hard < ipe);
    }

    #[test]
    fn test_sheet_good_kc_progression() {
        let mdf = Material::SheetGood {
            kind: SheetGoodKind::Mdf,
        }
        .kc_n_per_mm2()
        .unwrap();
        let hdf = Material::SheetGood {
            kind: SheetGoodKind::Hdf,
        }
        .kc_n_per_mm2()
        .unwrap();
        let particle = Material::SheetGood {
            kind: SheetGoodKind::Particleboard,
        }
        .kc_n_per_mm2()
        .unwrap();
        assert!(hdf > mdf);
        assert!(mdf > particle);
    }

    #[test]
    fn plastic_kc_only_some_when_validated() {
        let hdpe = Material::Plastic {
            family: PlasticFamily::Hdpe,
        };
        assert!(matches!(hdpe.kc_n_per_mm2(), Some(v) if (v - 40.0).abs() < 1e-6));
        for family in [
            PlasticFamily::Polycarbonate,
            PlasticFamily::Acrylic,
            PlasticFamily::Delrin,
            PlasticFamily::Generic,
        ] {
            let m = Material::Plastic { family };
            assert_eq!(
                m.kc_n_per_mm2(),
                None,
                "{family:?} must refuse Kc until a fetched primary lands"
            );
        }
    }

    #[test]
    fn plastic_hardness_preserves_scale() {
        assert!(matches!(
            PlasticFamily::Hdpe.hardness(),
            Some(PlasticHardness::ShoreD(v)) if (v - 64.0).abs() < 1e-6
        ));
        assert!(matches!(
            PlasticFamily::Polycarbonate.hardness(),
            Some(PlasticHardness::ShoreD(v)) if (v - 80.0).abs() < 1e-6
        ));
        assert!(matches!(
            PlasticFamily::Delrin.hardness(),
            Some(PlasticHardness::ShoreD(v)) if (v - 86.0).abs() < 1e-6
        ));
        // PMMA is natively Rockwell M; do not silently convert it.
        assert!(matches!(
            PlasticFamily::Acrylic.hardness(),
            Some(PlasticHardness::RockwellM(v)) if (v - 93.0).abs() < 1e-6
        ));
        assert_eq!(PlasticFamily::Generic.hardness(), None);
    }

    #[test]
    fn custom_with_invalid_kc_returns_none() {
        let bad = Material::Custom {
            name: "Bad".into(),
            hardness_index: 1.0,
            kc: -1.0,
        };
        assert_eq!(bad.kc_n_per_mm2(), None);
        let nan = Material::Custom {
            name: "NaN".into(),
            hardness_index: 1.0,
            kc: f64::NAN,
        };
        assert_eq!(nan.kc_n_per_mm2(), None);
        let good = Material::Custom {
            name: "OK".into(),
            hardness_index: 1.0,
            kc: 15.0,
        };
        assert_eq!(good.kc_n_per_mm2(), Some(15.0));
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
                .any(|(_, m)| matches!(m, Material::Aluminum { .. }))
        );
        assert!(
            catalog
                .iter()
                .any(|(_, m)| matches!(m, Material::Foam { .. }))
        );
    }

    #[test]
    fn legacy_southern_yellow_pine_key_aliases_to_longleaf() {
        // 2026-05-30 Phase 2C: `SouthernYellowPine` was a trade-group
        // misnomer — SYP covers Longleaf/Loblolly/Shortleaf/Slash with
        // different Jankas per species. The variant was renamed to
        // `LongleafPine` (Janka 870 lbf, Wood Database). Legacy
        // project files saved before the rename carry the old key;
        // `from_key` must alias them to `LongleafPine` so loading
        // doesn't silently fall back to default softwood.
        let restored = Material::from_key("southern_yellow_pine");
        assert_eq!(
            restored,
            Material::SolidWood {
                species: WoodSpecies::LongleafPine,
            },
            "legacy SYP key must map to LongleafPine"
        );
        // And the write path emits only the new canonical key.
        assert_eq!(restored.to_key(), "longleaf_pine");
    }

    #[test]
    fn longleaf_pine_janka_matches_wood_database() {
        assert!((WoodSpecies::LongleafPine.janka_lbf() - 870.0).abs() < 1e-9);
    }

    #[test]
    fn radiata_pine_janka_matches_wood_database() {
        // 2026-05-30 Phase 2C: corrected from repo's pre-Phase-2 500
        // lbf to the Wood Database value 710 lbf.
        assert!((WoodSpecies::RadiataPine.janka_lbf() - 710.0).abs() < 1e-9);
    }

    #[test]
    fn aluminum_kc_refuses_until_kienzle_lands() {
        for alloy in [AluminumAlloy::Alloy6061T6, AluminumAlloy::Alloy7075T6] {
            let m = Material::Aluminum { alloy };
            assert_eq!(
                m.kc_n_per_mm2(),
                None,
                "{alloy:?} must refuse Kc until Phase 3 beat F lands kc1.1/mc"
            );
        }
    }

    #[test]
    fn aluminum_brinell_matches_asm_anchors() {
        assert!((AluminumAlloy::Alloy6061T6.brinell_hb() - 95.0).abs() < 1e-9);
        assert!((AluminumAlloy::Alloy7075T6.brinell_hb() - 150.0).abs() < 1e-9);
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
