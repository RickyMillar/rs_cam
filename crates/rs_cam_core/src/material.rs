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
    /// Effective Janka hardness (lbf) for the plywood grade — driven by
    /// the dominant veneer species. Used by both `hardness_index()`
    /// (the feed-rate scaling normaliser) and
    /// `feeds::vendor_normalize::material_to_lut` (the LUT hardness
    /// query) so the two paths can't drift.
    pub fn effective_janka_lbf(self) -> f64 {
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
    /// Effective Janka hardness (lbf) for the engineered-wood sheet
    /// good — used by both `hardness_index()` and the LUT hardness
    /// query in `feeds::vendor_normalize::material_to_lut`. These
    /// values are the substrate density proxy; the actual cutting-
    /// force `Kc` lives on `Material::kc_n_per_mm2()` and is
    /// independent.
    pub fn effective_janka_lbf(self) -> f64 {
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
    /// UHMW-PE (ultra-high-molecular-weight polyethylene).
    /// Mitsubishi TIVAR 1000, Shore D 66 (ASTM D2240). Added Phase D
    /// 2026-05-31. Source: `hardness_extra.md` H.1.
    UhmwPe,
    /// Polypropylene (homopolymer). Shore D 70 corroborated by SIMONA
    /// PP-H (ASTM D2240) and Direct Plastics PP-H (ISO 868). Added
    /// Phase D 2026-05-31. Source: `hardness_extra.md` H.1.
    Polypropylene,
    /// Nylon 6/6 (PA66). Anchored to Mitsubishi Nylatron GS
    /// (MoS2-filled cast machinable grade — the CAM-relevant form):
    /// Shore D 85, Rockwell M 85, Rockwell R 115 all corroborated on
    /// one datasheet (ASTM D2240 / D785). Added Phase D 2026-05-31.
    /// Source: `hardness_extra.md` H.1. The `hardness()` accessor
    /// returns the Shore D scalar (multi-scale corroborated).
    Nylon66,
    /// ABS. Rockwell R 100-110 range (MakeItFrom material-group page;
    /// ASTM D785 implied by MakeItFrom's house style, not echoed on
    /// the per-page). Surface hardness reported as Rockwell R 105
    /// (range midpoint). Added Phase D 2026-05-31. Source:
    /// `hardness_extra.md` H.1.
    Abs,
    /// PETG. Rockwell R 115 (Plaskolite VIVAK datasheet, ASTM D785).
    /// Added Phase D 2026-05-31. Source: `hardness_extra.md` H.1.
    Petg,
    /// Rigid PVC Type 1 sheet (ASTM D-1784 class 12454-B). Shore D 74
    /// (Interstate AM product page; ASTM D2240 not explicitly echoed
    /// but is the universal Shore D method — scale-only citation per
    /// staging doc caveat). Added Phase D 2026-05-31. Source:
    /// `hardness_extra.md` H.1.
    RigidPvc,
}

/// Plastic surface hardness, preserving the original measurement scale.
///
/// Shore D, Rockwell M, and Rockwell R are not interchangeable — they
/// measure different things on different ranges — so this enum keeps
/// them distinct rather than fabricating a cross-scale equivalence.
/// Downstream consumers that need a single scalar pick the conversion
/// appropriate for their use.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PlasticHardness {
    ShoreD(f64),
    RockwellM(f64),
    /// Rockwell R scale — used for softer engineering plastics
    /// (ABS, PETG, Nylon R-scale readings). Added Phase D 2026-05-31
    /// for the plastic family expansion. The R-scale uses a 1/2"
    /// ball indenter and a 60 kgf minor + 100 kgf major load — it is
    /// a *separate* scale from Rockwell M (1/4" ball, 100 kgf major)
    /// and the two are not interchangeable.
    RockwellR(f64),
}

impl PlasticFamily {
    pub fn label(self) -> &'static str {
        match self {
            PlasticFamily::Generic => "Generic Plastic",
            PlasticFamily::Acrylic => "Acrylic",
            PlasticFamily::Hdpe => "HDPE",
            PlasticFamily::Delrin => "Delrin",
            PlasticFamily::Polycarbonate => "Polycarbonate",
            PlasticFamily::UhmwPe => "UHMW-PE",
            PlasticFamily::Polypropylene => "Polypropylene",
            PlasticFamily::Nylon66 => "Nylon 6/6",
            PlasticFamily::Abs => "ABS",
            PlasticFamily::Petg => "PETG",
            PlasticFamily::RigidPvc => "Rigid PVC",
        }
    }

    /// Surface hardness in its measured scale.
    ///
    /// Citations (per `planning/data_ingest_2026-05-29/hardness.md`
    /// and `planning/data_ingest_2026-05-30/hardness_extra.md`):
    /// - HDPE: ISO 868 / Direct Plastics, Shore D 64.0.
    /// - Polycarbonate: ASTM D2240 / Treatstock, Shore D 80.0.
    /// - Delrin (POM-H): ASTM D2240 / Alro, Shore D 86.0.
    /// - Acrylic (PMMA): MakeItFrom, Rockwell M 93.0 — PMMA is
    ///   typically reported in Rockwell M; Shore D is not the standard
    ///   scale for it, so the value is exposed in its native scale.
    /// - UHMW-PE: ASTM D2240 / Mitsubishi TIVAR 1000, Shore D 66.0.
    /// - Polypropylene: ASTM D2240 / SIMONA PP-H, Shore D 70.0.
    /// - Nylon 6/6 (Nylatron GS): ASTM D2240, Shore D 85.0 (also
    ///   reports Rockwell M 85 / R 115 on the same datasheet).
    /// - ABS: ASTM D785 / MakeItFrom range 100-110, Rockwell R 105.0
    ///   (midpoint).
    /// - PETG: ASTM D785 / Plaskolite VIVAK, Rockwell R 115.0.
    /// - Rigid PVC Type 1: Shore D 74.0 (Interstate AM; scale-only).
    /// - Generic: no primary hardness datum (returns `None`).
    pub fn hardness(self) -> Option<PlasticHardness> {
        match self {
            PlasticFamily::Hdpe => Some(PlasticHardness::ShoreD(64.0)),
            PlasticFamily::Polycarbonate => Some(PlasticHardness::ShoreD(80.0)),
            PlasticFamily::Delrin => Some(PlasticHardness::ShoreD(86.0)),
            PlasticFamily::Acrylic => Some(PlasticHardness::RockwellM(93.0)),
            PlasticFamily::UhmwPe => Some(PlasticHardness::ShoreD(66.0)),
            PlasticFamily::Polypropylene => Some(PlasticHardness::ShoreD(70.0)),
            PlasticFamily::Nylon66 => Some(PlasticHardness::ShoreD(85.0)),
            PlasticFamily::Abs => Some(PlasticHardness::RockwellR(105.0)),
            PlasticFamily::Petg => Some(PlasticHardness::RockwellR(115.0)),
            PlasticFamily::RigidPvc => Some(PlasticHardness::ShoreD(74.0)),
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
    /// 2024-T3 — aerospace structural; high strength-to-weight, lower
    /// corrosion resistance. Brinell 120 (ASM Aerospace Specification
    /// Metals, MatWeb; 500 g load, 10 mm ball). Added Phase C
    /// (completion plan 2026-05-31). Source: `planning/data_ingest_
    /// 2026-05-30/hardness_extra.md` H.2.
    Alloy2024T3,
    /// 5052-H32 — marine/sheet alloy, excellent formability and
    /// corrosion resistance. Brinell 60 (ASM, MatWeb; 500 g, 10 mm).
    /// Added Phase C 2026-05-31. Source: `hardness_extra.md` H.2.
    Alloy5052H32,
    /// 3003-H14 — general-purpose sheet alloy, moderate strength.
    /// Brinell 42 (MakeItFrom material properties; ASTM not echoed
    /// on per-alloy page — scale-only). Added Phase C 2026-05-31.
    /// Source: `hardness_extra.md` H.2.
    Alloy3003H14,
    /// 1100-O — commercially pure aluminum, annealed (softest
    /// production aluminum). Brinell 23 (MakeItFrom; ASTM
    /// scale-only). Added Phase C 2026-05-31. Source:
    /// `hardness_extra.md` H.2.
    Alloy1100O,
    /// 7050-T7651 — high-strength aerospace plate; T7651 is the
    /// stress-relieved + over-aged temper for stress-corrosion
    /// resistance. Brinell 147 (ASM-calculated; Kaiser Aluminum
    /// mill data reports 150 on the same alloy/temper — both
    /// consistent within rounding). Added Phase C 2026-05-31.
    /// Source: `hardness_extra.md` H.2.
    Alloy7050T7651,
}

impl AluminumAlloy {
    /// Brinell hardness (HB). Sources: ASM Aerospace Specification
    /// Metals / MatWeb, with MakeItFrom and Kaiser mill data filling
    /// in alloys not hosted on `asm.matweb.com`. These are the
    /// standard datapoints used in vendor feed/speed lookups for
    /// aluminum. See `planning/data_ingest_2026-05-30/hardness_extra.md`
    /// H.2 for the verbatim per-alloy quotes.
    pub fn brinell_hb(self) -> f64 {
        match self {
            AluminumAlloy::Alloy6061T6 => 95.0,
            AluminumAlloy::Alloy7075T6 => 150.0,
            AluminumAlloy::Alloy2024T3 => 120.0,
            AluminumAlloy::Alloy5052H32 => 60.0,
            AluminumAlloy::Alloy3003H14 => 42.0,
            AluminumAlloy::Alloy1100O => 23.0,
            AluminumAlloy::Alloy7050T7651 => 147.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AluminumAlloy::Alloy6061T6 => "Aluminum 6061-T6",
            AluminumAlloy::Alloy7075T6 => "Aluminum 7075-T6",
            AluminumAlloy::Alloy2024T3 => "Aluminum 2024-T3",
            AluminumAlloy::Alloy5052H32 => "Aluminum 5052-H32",
            AluminumAlloy::Alloy3003H14 => "Aluminum 3003-H14",
            AluminumAlloy::Alloy1100O => "Aluminum 1100-O",
            AluminumAlloy::Alloy7050T7651 => "Aluminum 7050-T7651",
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
    /// Wood-Janka-normalised hardness index driving feed-rate scaling
    /// in [`crate::feeds::calculate`]. `1.0 = soft wood baseline (Janka
    /// 600 lbf)`; formula `(janka / 600)^0.4`.
    ///
    /// **Semantics caveat (read before consuming the value):** this is
    /// a *wood-baseline* normaliser. For materials outside the wood
    /// regime — plastics, aluminum — the value is a placeholder
    /// scaling factor with no shared physical meaning across classes:
    /// - Plastic: hardcoded `0.5` regardless of family (per-family
    ///   refinement would route through `PlasticFamily::hardness()`).
    /// - Aluminum: `(brinell / 60)^0.4` — Brinell isn't on the Janka
    ///   scale; treat the number as a feed-rate placeholder, not a
    ///   physical hardness.
    /// - Foam: per-density hardcode (0.15 / 0.25 / 0.40).
    ///
    /// The safety-critical force-prediction path
    /// (`Material::kc_n_per_mm2`) is independent of this and refuses
    /// cleanly for unmeasured materials.
    ///
    /// `Material::Custom { hardness_index, .. }` ignores invalid user
    /// inputs (NaN / non-positive) and falls back to the softwood
    /// baseline of 1.0 — keeps downstream `1.0 / hardness` consumers
    /// out of NaN territory.
    pub fn hardness_index(&self) -> f64 {
        match self {
            Material::SolidWood { species } => (species.janka_lbf() / 600.0).powf(0.4),
            Material::Plywood { grade } => (grade.effective_janka_lbf() / 600.0).powf(0.4),
            Material::SheetGood { kind } => (kind.effective_janka_lbf() / 600.0).powf(0.4),
            Material::Plastic { .. } => 0.5,
            Material::Aluminum { alloy } => (alloy.brinell_hb() / 60.0).powf(0.4),
            Material::Foam { density } => match density {
                FoamDensity::Low => 0.15,
                FoamDensity::Medium => 0.25,
                FoamDensity::High => 0.40,
            },
            Material::Custom { hardness_index, .. } => {
                if hardness_index.is_finite() && *hardness_index > 0.0 {
                    *hardness_index
                } else {
                    // Pre-Phase-1E this returned raw `*hardness_index`,
                    // propagating NaN / negative values into the
                    // feed-rate ramp (`1.0 / hardness` → NaN/-inf).
                    // Fall back to the softwood baseline (1.0) so the
                    // pipeline never serves an invalid scalar from
                    // user-typed Custom material data.
                    1.0
                }
            }
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
            // TODO Phase 3 — per-species solid-wood Kc has no fetched
            // direct measurement; current values track FPL Ch.5 shear-
            // parallel-to-grain strength (≈6–16 MPa) rather than
            // peripheral milling specific cutting force (≈30–40 N/mm²
            // for boards). Phase 3 beat C will inform a per-species
            // derivation backbone via the shear-strength × edge-radius
            // size-effect factor. Until then, kept as-is; the Phase 2B
            // sheet-good update is the highest-confidence move.
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
            // TODO Phase 3 — per-grade plywood Kc has no fetched
            // primary measurement; current values track shear-parallel
            // shear strength of the dominant veneer rather than peripheral
            // milling specific cutting force. Phase 3 beat C (FPL Ch.5
            // systematic extract) will inform a per-species derivation.
            Material::Plywood { grade } => Some(match grade {
                PlywoodGrade::Softwood => 8.0,
                PlywoodGrade::BalticBirch => 13.0,
                PlywoodGrade::HardwoodFaced => 11.0,
            }),
            // Sheet-good Kc (Phase 2B, 2026-05-30): measured-literature
            // values from `planning/data_ingest_2026-05-29/kc.md`. The
            // pre-Phase-2B values (Mdf=10, Hdf=12, Particleboard=9)
            // tracked shear-parallel-to-grain strength of the substrate
            // rather than peripheral-milling specific cutting force,
            // under-predicting force by 3–4× and being absorbed by the
            // old 2.5× ANISOTROPY_MULTIPLIER. Paired with the
            // GRAIN_ANISOTROPY_FACTOR drop 2.5 → 2.0 to keep the
            // physically-meaningful product `Kc × factor` honest.
            Material::SheetGood { kind } => Some(match kind {
                // PMC6315737 round-shape Ks for MDF: average 31.44
                // (SD 2.68; range 25.81–35.58). Isotropic in plane.
                SheetGoodKind::Mdf => 31.4,
                // No direct HDF Kc measurement; derived as MDF scaled
                // by the HDF/MDF density ratio (~880/750 ≈ 1.17×).
                // TODO Phase 3: replace with fetched HDF cutting-force
                // measurement.
                SheetGoodKind::Hdf => 36.8,
                // Pałubicki 2021 (DOI 10.3390/ma14092208) average of
                // slow (32.0) and fast (37.6) peripheral up-milling
                // principal cutting force for particleboard at
                // vc=40/60 m/s, rake 13°, h up to ~0.31 mm.
                SheetGoodKind::Particleboard => 35.0,
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
                // Phase D 2026-05-31 — six new plastic families added
                // alongside the existing PC/PMMA/POM gap. None have a
                // fetched milling-regime Kc value (kc_extra.md / Round-2
                // recorded a PMMA 276.5 N/mm² value but with an explicit
                // "do NOT promote — size-effect inflated" caveat;
                // UHMW/PP/Nylon/ABS/PETG/PVC have no primary milling
                // measurements at all). Refuse-first stays the rule —
                // the gate refuses via `MaterialUnvalidated` rather
                // than predicting force from a fabricated constant.
                PlasticFamily::Polycarbonate
                | PlasticFamily::Acrylic
                | PlasticFamily::Delrin
                | PlasticFamily::Generic
                | PlasticFamily::UhmwPe
                | PlasticFamily::Polypropylene
                | PlasticFamily::Nylon66
                | PlasticFamily::Abs
                | PlasticFamily::Petg
                | PlasticFamily::RigidPvc => None,
            },
            // Aluminum Kienzle pair from Machining Doctor's VDI 3323
            // table (Wayback 2024-08-13 snapshot of
            // `machiningdoctor.com/specific-cutting-force-chart`),
            // cross-checked against Sandvik's 2017 EN-GB aluminium
            // ISO-N page bound of 350–700 N/mm². Both 6061-T6 and
            // 7075-T6 sit at the same VDI group-22 anchor:
            //   kc1.1 = 800 N/mm², mc = 0.25
            // Evaluated at a representative chip thickness h = 0.1 mm:
            //   Kc = 800 · 0.1^(-0.25) ≈ 1422.8 N/mm²
            // Citation: planning/data_ingest_2026-05-30/aluminum_kc.md
            // (Phase 3 beat F result, "Grade A-secondary").
            //
            // Per-alloy specialisation: 7075-T6 reads slightly higher
            // in the Sandvik aluminium-specific band (350–700) than
            // 6061-T6; until a per-alloy Kienzle pair lands, we use
            // the shared VDI group-22 value for both. The
            // documented "Grade A-secondary" qualification is on the
            // citation in aluminum_kc.md, not on the type.
            Material::Aluminum { .. } => {
                const KC11: f64 = 800.0;
                const MC: f64 = 0.25;
                const REPRESENTATIVE_H_MM: f64 = 0.1;
                Some(KC11 * REPRESENTATIVE_H_MM.powf(-MC))
            }
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

    /// Drill-cycle depth-to-diameter ratio above which chip welding
    /// becomes likely without a pecking cycle. Drilling rule of thumb,
    /// not derived from `Kc`; the values are application-edge per
    /// material class. Wood scales by Janka; sheet goods and plywood
    /// share a single substrate-density-weighted threshold; aluminum
    /// is the standard ~3×D limit; foam is permissive; Custom scales
    /// roughly with the user-supplied hardness.
    ///
    /// Consumed by `drill_metrics::evaluate_chip_welding_risk` and
    /// `tool_load::drill_gates::evaluate_chip_welding`. Centralised
    /// here so adding a `Material` variant is one diff site, not
    /// two — pre-Phase-1E this was a free function in
    /// `drill_metrics.rs` and the Aluminum addition had to touch
    /// that and `drill_gates::plunge_feed_envelope` separately with
    /// matching values.
    pub fn drill_chip_welding_threshold_dtd(&self) -> f64 {
        match self {
            Material::SolidWood { species } => {
                let janka = species.janka_lbf();
                if janka <= 700.0 {
                    8.0 // softwood
                } else if janka <= 1500.0 {
                    6.0 // medium hardwood
                } else {
                    5.0 // dense hardwood
                }
            }
            Material::Plywood { .. } | Material::SheetGood { .. } => 5.0,
            Material::Plastic { .. } => 4.0,
            // Aluminum chip welding starts around D/d ≈ 3 (industry
            // rule of thumb; pecks mandatory beyond). Conservative
            // even for 6061 — denser alloys want lower D/d.
            Material::Aluminum { .. } => 3.0,
            Material::Foam { .. } => 12.0,
            Material::Custom { hardness_index, .. } => {
                // Softer materials evacuate better. Clamp to the
                // wood-to-foam range so a degenerate user-supplied
                // hardness can't blow this open.
                (8.0 / hardness_index.max(0.5)).clamp(2.0, 12.0)
            }
        }
    }

    /// Drill-cycle per-peck max depth-to-diameter. A single peck
    /// deeper than this traps chips even within a pecking cycle.
    ///
    /// Same rationale + centralisation as
    /// `drill_chip_welding_threshold_dtd`.
    pub fn drill_per_peck_max_dtd(&self) -> f64 {
        match self {
            Material::SolidWood { .. } => 2.0,
            Material::Plywood { .. } | Material::SheetGood { .. } => 1.5,
            Material::Plastic { .. } => 1.0,
            // Aluminum per-peck ≤ 1×D — standard machining-textbook
            // limit for chip evacuation without through-coolant.
            Material::Aluminum { .. } => 1.0,
            Material::Foam { .. } => 4.0,
            Material::Custom { .. } => 1.5,
        }
    }

    /// Drill plunge feed envelope (mm/min per mm diameter): below the
    /// min the cutter rubs / burns; above the max it breaks or
    /// stalls. Same dispatch pattern as the chip-welding methods.
    ///
    /// Consumed by `tool_load::drill_gates::evaluate_plunge_feed`.
    pub fn drill_plunge_feed_envelope_per_mm(&self) -> (f64, f64) {
        match self {
            Material::SolidWood { .. } => (50.0, 400.0),
            Material::Plywood { .. } | Material::SheetGood { .. } => (40.0, 350.0),
            Material::Plastic { .. } => (60.0, 500.0),
            // Aluminum on a wood router is application-edge —
            // conservative envelope (slower than wood min, lower than
            // plastic max). Real aluminum drilling should always be
            // vendor-LUT-driven.
            Material::Aluminum { .. } => (40.0, 250.0),
            Material::Foam { .. } => (100.0, 1000.0),
            Material::Custom { .. } => (40.0, 500.0),
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
            (
                "UHMW-PE",
                Material::Plastic {
                    family: PlasticFamily::UhmwPe,
                },
            ),
            (
                "Polypropylene",
                Material::Plastic {
                    family: PlasticFamily::Polypropylene,
                },
            ),
            (
                "Nylon 6/6",
                Material::Plastic {
                    family: PlasticFamily::Nylon66,
                },
            ),
            (
                "ABS",
                Material::Plastic {
                    family: PlasticFamily::Abs,
                },
            ),
            (
                "PETG",
                Material::Plastic {
                    family: PlasticFamily::Petg,
                },
            ),
            (
                "Rigid PVC",
                Material::Plastic {
                    family: PlasticFamily::RigidPvc,
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
            (
                "Aluminum 2024-T3",
                Material::Aluminum {
                    alloy: AluminumAlloy::Alloy2024T3,
                },
            ),
            (
                "Aluminum 5052-H32",
                Material::Aluminum {
                    alloy: AluminumAlloy::Alloy5052H32,
                },
            ),
            (
                "Aluminum 3003-H14",
                Material::Aluminum {
                    alloy: AluminumAlloy::Alloy3003H14,
                },
            ),
            (
                "Aluminum 1100-O",
                Material::Aluminum {
                    alloy: AluminumAlloy::Alloy1100O,
                },
            ),
            (
                "Aluminum 7050-T7651",
                Material::Aluminum {
                    alloy: AluminumAlloy::Alloy7050T7651,
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
                PlasticFamily::UhmwPe => "uhmw_pe",
                PlasticFamily::Polypropylene => "polypropylene",
                PlasticFamily::Nylon66 => "nylon66",
                PlasticFamily::Abs => "abs",
                PlasticFamily::Petg => "petg",
                PlasticFamily::RigidPvc => "rigid_pvc",
            }
            .to_owned(),
            Material::Aluminum { alloy } => match alloy {
                AluminumAlloy::Alloy6061T6 => "aluminum_6061_t6",
                AluminumAlloy::Alloy7075T6 => "aluminum_7075_t6",
                AluminumAlloy::Alloy2024T3 => "aluminum_2024_t3",
                AluminumAlloy::Alloy5052H32 => "aluminum_5052_h32",
                AluminumAlloy::Alloy3003H14 => "aluminum_3003_h14",
                AluminumAlloy::Alloy1100O => "aluminum_1100_o",
                AluminumAlloy::Alloy7050T7651 => "aluminum_7050_t7651",
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
            "uhmw_pe" => Material::Plastic {
                family: PlasticFamily::UhmwPe,
            },
            "polypropylene" => Material::Plastic {
                family: PlasticFamily::Polypropylene,
            },
            "nylon66" => Material::Plastic {
                family: PlasticFamily::Nylon66,
            },
            "abs" => Material::Plastic {
                family: PlasticFamily::Abs,
            },
            "petg" => Material::Plastic {
                family: PlasticFamily::Petg,
            },
            "rigid_pvc" => Material::Plastic {
                family: PlasticFamily::RigidPvc,
            },
            "aluminum_6061_t6" => Material::Aluminum {
                alloy: AluminumAlloy::Alloy6061T6,
            },
            "aluminum_7075_t6" => Material::Aluminum {
                alloy: AluminumAlloy::Alloy7075T6,
            },
            "aluminum_2024_t3" => Material::Aluminum {
                alloy: AluminumAlloy::Alloy2024T3,
            },
            "aluminum_5052_h32" => Material::Aluminum {
                alloy: AluminumAlloy::Alloy5052H32,
            },
            "aluminum_3003_h14" => Material::Aluminum {
                alloy: AluminumAlloy::Alloy3003H14,
            },
            "aluminum_1100_o" => Material::Aluminum {
                alloy: AluminumAlloy::Alloy1100O,
            },
            "aluminum_7050_t7651" => Material::Aluminum {
                alloy: AluminumAlloy::Alloy7050T7651,
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
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
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
    fn test_sheet_good_kc_in_measured_literature_band() {
        // Phase 2B replaced the shear-strength-derived Kc constants
        // (mdf=10 / hdf=12 / particle=9 — pre-Phase-2B) with measured
        // literature values from peripheral-milling studies
        // (planning/data_ingest_2026-05-29/kc.md). The post-Phase-2B
        // ordering (HDF 36.8 > Particleboard 35.0 > MDF 31.4) does NOT
        // match the old density-derived intuition — Pałubicki 2021's
        // particleboard measurements run a touch above the PMC6315737
        // round-shape MDF measurement. The right invariants are:
        //   - all three sheet-good Kc sit in the literature band
        //     (~31–37 N/mm² for peripheral milling at low rake)
        //   - HDF is densest, sits at the top
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
        for (label, kc) in [("Mdf", mdf), ("Hdf", hdf), ("Particleboard", particle)] {
            assert!(
                (30.0..=40.0).contains(&kc),
                "{label} Kc {kc} N/mm² must sit in the measured literature band 30–40 N/mm²"
            );
        }
        assert!(hdf >= mdf, "HDF (denser) must not be below MDF");
        assert!(hdf >= particle, "HDF (densest engineered wood) must top sheet goods");
    }

    #[test]
    fn plastic_kc_only_some_when_validated() {
        let hdpe = Material::Plastic {
            family: PlasticFamily::Hdpe,
        };
        assert!(matches!(hdpe.kc_n_per_mm2(), Some(v) if (v - 40.0).abs() < 1e-6));
        // Phase D 2026-05-31 — 6 new families joined the refusal set.
        // All UHMW/PP/Nylon/ABS/PETG/PVC have NO fetched milling-regime
        // Kc; refusal-first stays the rule. Updating this list when a
        // primary Kc value lands is the per-family promotion checklist.
        for family in [
            PlasticFamily::Polycarbonate,
            PlasticFamily::Acrylic,
            PlasticFamily::Delrin,
            PlasticFamily::Generic,
            PlasticFamily::UhmwPe,
            PlasticFamily::Polypropylene,
            PlasticFamily::Nylon66,
            PlasticFamily::Abs,
            PlasticFamily::Petg,
            PlasticFamily::RigidPvc,
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
        // Phase D 2026-05-31 — 6 new families. The scale must match the
        // staged citation (Shore D for polyolefins + PVC + Nylatron GS;
        // Rockwell R for ABS/PETG). A future drift that silently
        // converts Rockwell R → Shore D would fail these `matches!`
        // asserts, not just a numeric tolerance check.
        assert!(matches!(
            PlasticFamily::UhmwPe.hardness(),
            Some(PlasticHardness::ShoreD(v)) if (v - 66.0).abs() < 1e-6
        ));
        assert!(matches!(
            PlasticFamily::Polypropylene.hardness(),
            Some(PlasticHardness::ShoreD(v)) if (v - 70.0).abs() < 1e-6
        ));
        assert!(matches!(
            PlasticFamily::Nylon66.hardness(),
            Some(PlasticHardness::ShoreD(v)) if (v - 85.0).abs() < 1e-6
        ));
        assert!(matches!(
            PlasticFamily::Abs.hardness(),
            Some(PlasticHardness::RockwellR(v)) if (v - 105.0).abs() < 1e-6
        ));
        assert!(matches!(
            PlasticFamily::Petg.hardness(),
            Some(PlasticHardness::RockwellR(v)) if (v - 115.0).abs() < 1e-6
        ));
        assert!(matches!(
            PlasticFamily::RigidPvc.hardness(),
            Some(PlasticHardness::ShoreD(v)) if (v - 74.0).abs() < 1e-6
        ));
        assert_eq!(PlasticFamily::Generic.hardness(), None);
    }

    #[test]
    fn custom_with_invalid_hardness_falls_back_to_softwood_baseline() {
        // S3-12 fix: Custom material with NaN / non-positive
        // hardness_index propagated raw values into `1.0 / hardness`
        // consumers, producing NaN / -inf. Now coerces to 1.0
        // (softwood baseline) so the pipeline always has a defined
        // scalar.
        let nan = Material::Custom {
            name: "NaN-hardness".into(),
            hardness_index: f64::NAN,
            kc: 10.0,
        };
        assert!((nan.hardness_index() - 1.0).abs() < 1e-9);
        let neg = Material::Custom {
            name: "negative-hardness".into(),
            hardness_index: -1.0,
            kc: 10.0,
        };
        assert!((neg.hardness_index() - 1.0).abs() < 1e-9);
        let zero = Material::Custom {
            name: "zero-hardness".into(),
            hardness_index: 0.0,
            kc: 10.0,
        };
        assert!((zero.hardness_index() - 1.0).abs() < 1e-9);
        // Valid value passes through unchanged.
        let good = Material::Custom {
            name: "valid".into(),
            hardness_index: 1.4,
            kc: 10.0,
        };
        assert!((good.hardness_index() - 1.4).abs() < 1e-9);
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
    fn aluminum_kc_computes_from_kienzle_pair() {
        // Phase 4 enabled the Kienzle pair (kc1.1=800, mc=0.25) for
        // all aluminum alloys, evaluated at h=0.1 mm:
        //   Kc = 800 · 0.1^(-0.25) ≈ 1422.8 N/mm²
        // Per D3 of the completion plan, Kc remains the single shared
        // Kienzle pair across all alloys (vendor sources don't
        // differentiate Kc by alloy at our fidelity).
        let expected = 800.0 * 0.1_f64.powf(-0.25);
        for alloy in [
            AluminumAlloy::Alloy6061T6,
            AluminumAlloy::Alloy7075T6,
            AluminumAlloy::Alloy2024T3,
            AluminumAlloy::Alloy5052H32,
            AluminumAlloy::Alloy3003H14,
            AluminumAlloy::Alloy1100O,
            AluminumAlloy::Alloy7050T7651,
        ] {
            let m = Material::Aluminum { alloy };
            let kc = m
                .kc_n_per_mm2()
                .expect("aluminum Kc must be Some after Phase 4 promoted the Kienzle pair");
            assert!(
                (kc - expected).abs() < 1.0,
                "{alloy:?} Kc {kc} must match VDI 3323 group-22 kc1.1·h^-mc ({expected})"
            );
        }
    }

    #[test]
    fn aluminum_brinell_matches_asm_anchors() {
        // Phase C added Alloy2024T3 / 5052H32 / 3003H14 / 1100O /
        // 7050T7651 per `planning/data_ingest_2026-05-30/hardness_extra.md`
        // H.2. Sources are mixed (ASM matweb for the heavy hitters,
        // MakeItFrom for 3003/1100) but every value is read verbatim
        // from a fetched datasheet — see the per-variant doc comments
        // and the literature_parity sentries for citations.
        for (alloy, expected) in [
            (AluminumAlloy::Alloy6061T6, 95.0),
            (AluminumAlloy::Alloy7075T6, 150.0),
            (AluminumAlloy::Alloy2024T3, 120.0),
            (AluminumAlloy::Alloy5052H32, 60.0),
            (AluminumAlloy::Alloy3003H14, 42.0),
            (AluminumAlloy::Alloy1100O, 23.0),
            (AluminumAlloy::Alloy7050T7651, 147.0),
        ] {
            assert!(
                (alloy.brinell_hb() - expected).abs() < 1e-9,
                "{alloy:?} Brinell mismatch: got {} want {expected}",
                alloy.brinell_hb()
            );
        }
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
