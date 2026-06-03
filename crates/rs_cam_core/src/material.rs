//! Material definitions for feeds & speeds calculation.
//!
//! Provides material hardness index and specific cutting force (Kc) values
//! used by the feeds calculator to determine chip load, feed rate, and power.
//! Ported from reference/shapeoko_feeds_and_speeds/src/params/mod.rs.

pub mod wood_species_library;

use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

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
    /// the dominant veneer species. Used by both `feed_scale_factor()`
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
    /// good — used by both `feed_scale_factor()` and the LUT hardness
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

/// Fiber-reinforced composite grade for [`Material::Fiberglass`].
///
/// Added Phase 5 Step 5.3 (2026-06-01) for the staged Garr
/// Fiberglass/Plastics/G10 row promotion (which collapsed three
/// material classes onto one chart entry — only G10/FR4 is
/// confidently routed; the other two would need separate variants
/// when LUT data lands).
///
/// `G10Fr4` is the canonical electrical-grade woven glass-epoxy
/// laminate (NEMA G-10 / FR-4 spec). `Generic` covers other
/// glass-resin composites (chopped strand, glass-polyester) at lower
/// confidence — the vendor LUT row matching collapses both onto
/// `MaterialFamily::Fiberglass` until per-grade LUT data exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FiberglassGrade {
    G10Fr4,
    Generic,
}

impl FiberglassGrade {
    pub fn label(self) -> &'static str {
        match self {
            FiberglassGrade::G10Fr4 => "G10 / FR-4 (glass-epoxy)",
            FiberglassGrade::Generic => "Generic Fiberglass",
        }
    }
}

/// Top-level material category for the hierarchical GUI picker.
///
/// Separates the *enum-shape* of [`Material`] (whose variants are
/// implementation detail driven by the per-class accessors) from the
/// *user-facing classification* a CAM operator browses by ("Wood →
/// Hardwood → Hard Maple"). The GUI's nested menu walks these.
///
/// Wood splits into Softwood / Hardwood by Janka — see
/// [`Material::category`] for the cutoff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaterialCategory {
    Softwood,
    Hardwood,
    Plywood,
    SheetGood,
    Plastic,
    Aluminum,
    Composite,
    Foam,
    Custom,
}

impl MaterialCategory {
    /// Display label for the category header in the GUI menu.
    pub fn label(self) -> &'static str {
        match self {
            Self::Softwood => "Softwood",
            Self::Hardwood => "Hardwood",
            Self::Plywood => "Plywood",
            Self::SheetGood => "Sheet Goods",
            Self::Plastic => "Plastic",
            Self::Aluminum => "Aluminum",
            Self::Composite => "Composite",
            Self::Foam => "Foam",
            Self::Custom => "Custom",
        }
    }

    /// All categories in display order — drives the order of entries
    /// in the GUI's top-level material menu.
    pub fn all() -> &'static [MaterialCategory] {
        &[
            Self::Softwood,
            Self::Hardwood,
            Self::Plywood,
            Self::SheetGood,
            Self::Plastic,
            Self::Aluminum,
            Self::Composite,
            Self::Foam,
            Self::Custom,
        ]
    }

    /// Whether this category groups under a "Wood" parent menu in the
    /// hierarchical picker. The GUI nests Softwood + Hardwood under
    /// Wood ▶ so the operator drills `Wood → Softwood → species`.
    pub fn is_wood(self) -> bool {
        matches!(self, Self::Softwood | Self::Hardwood)
    }
}

/// Material being cut. Determines chip load scaling and power requirements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Material {
    SolidWood {
        species: WoodSpecies,
    },
    /// Parametric solid-wood variant — `(janka_lbf, label, source_id)`.
    /// Lets the GUI surface the broader Wood Database + FPL Ch.5 species
    /// libraries (~148 species in `WOOD_SPECIES_LIBRARY`) without
    /// exploding the [`WoodSpecies`] enum to 150+ pattern-match arms.
    /// Added Phase E (completion plan 2026-05-31).
    ///
    /// - `janka_lbf` — Janka hardness in lbf at 12% MC. Must satisfy
    ///   [`JANKA_CALIBRATED_BAND`] (`200.0..=4000.0`) for the helper
    ///   `Kc` and `feed_scale_factor` lookups to return finite values;
    ///   outside the band, `kc_n_per_mm2()` returns `None` and
    ///   `feed_scale_factor()` falls back to `1.0`.
    /// - `label` — human-readable species name for GUI display
    ///   (`"Red Oak (Northern)"`, `"Black Cherry"`).
    /// - `source_id` — citation key matching an entry in
    ///   `data/vendor_lut/source_manifest.json` (e.g.
    ///   `"wood_database_2026-05-30"` or `"fpl_ch5_2010"`).
    ///
    /// First-class [`WoodSpecies`] enum variants stay the source of
    /// truth for the 10 most-tested species (HardMaple, Walnut, Ipe,
    /// etc.) — they keep their hand-tuned `Kc` values and `Janka`
    /// anchors. The parametric variant is for the long tail.
    SolidWoodByJanka {
        janka_lbf: f64,
        label: String,
        source_id: String,
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
    /// Fiber-reinforced composites (G10/FR4 glass-epoxy, generic
    /// glass-resin laminates). Added Phase 5 Step 5.3 (2026-06-01) to
    /// unlock the staged Garr GP-plastics row that collapsed
    /// "Fiberglass/Plastics/G10" under one chart entry. Distinct from
    /// `Material::Plastic` because the cutting class is abrasive +
    /// fiber-reinforced — tool wear and delamination considerations
    /// don't map onto polymers.
    ///
    /// Refuses Kc — no fetched primary measurement available yet; the
    /// gate refuses via `MaterialUnvalidated` rather than predicting
    /// force from a fabricated constant. Plunge / drill / feed
    /// envelopes are conservative carbide-tool placeholders pending
    /// bench validation.
    Fiberglass {
        grade: FiberglassGrade,
    },
    Custom {
        name: String,
        /// Feed-rate scaling factor with softwood as the baseline (1.0 =
        /// generic softwood; > 1 harder, < 1 softer). Renamed from
        /// `hardness_index` in S2-8 Option B because the previous name
        /// implied a wood-Janka normalised hardness, but the consumers
        /// use it as a *general feed derate* scalar (the `(1.0 / x)^q`
        /// chipload formula, the adaptive ap/ae bracketed derates).
        /// For non-wood materials there is no shared physical meaning —
        /// plastics/aluminum/foam supply per-family placeholders that
        /// drive the same feed math, not a Janka-equivalent hardness.
        ///
        /// Validation rules unchanged from the old field: non-finite or
        /// non-positive values fall through to the softwood baseline
        /// (1.0) in [`Material::feed_scale_factor`].
        feed_scale_factor: f64,
        kc: f64,
    },
}

/// Calibrated Janka band for the parametric [`Material::SolidWoodByJanka`]
/// `Kc` helper. Outside this band the `Kc` regression has no
/// citation-backed validity; the helper returns `None` and gates
/// refuse via `UnmodeledReason::MaterialUnvalidated`.
///
/// The lower bound 200 lbf is well below balsa-class softwoods
/// (Generic softwood baseline = 600 lbf); the upper bound 4000 lbf
/// is above Ipe (3510 lbf, the existing per-species ceiling). Both
/// extremes are well outside the workshop-stock range that informed
/// the folklore-grade per-species `Kc` table.
pub const JANKA_CALIBRATED_BAND_LOW_LBF: f64 = 200.0;
pub const JANKA_CALIBRATED_BAND_HIGH_LBF: f64 = 4000.0;

/// Shared Janka → Kc fallback for parametric wood species.
///
/// **Formula:** `Kc = janka_lbf / 100.0`. Folklore-grade — the
/// existing 10 per-species hardcoded `Kc` values in
/// [`Material::kc_n_per_mm2`] sit roughly on this line (the table
/// fits within ±25 % of `janka/100` except for the
/// `GenericHardwood` and `Walnut` hand-tuned anchors). The literature
/// has no clean Janka → Kc regression at workshop fidelity; this
/// formula exists so the parametric variant can supply *something*
/// finite to the Kc-consuming gates rather than refusing every
/// library species.
///
/// **Returns `None`** outside [`JANKA_CALIBRATED_BAND_LOW_LBF`,
/// `JANKA_CALIBRATED_BAND_HIGH_LBF`] or for non-finite input. The
/// 10 first-class [`WoodSpecies`] variants do NOT route through this
/// helper — their per-species `Kc` constants are the source of truth
/// for those species.
///
/// TODO Phase 3+: replace with a per-species derivation backbone
/// (FPL Ch.5 shear-parallel-to-grain × edge-radius size-effect, per
/// the `kc.md` TODO trail).
pub(crate) fn janka_to_kc_n_per_mm2(janka_lbf: f64) -> Option<f64> {
    if !janka_lbf.is_finite() {
        return None;
    }
    if !(JANKA_CALIBRATED_BAND_LOW_LBF..=JANKA_CALIBRATED_BAND_HIGH_LBF).contains(&janka_lbf) {
        return None;
    }
    Some(janka_lbf / 100.0)
}

/// Shared Janka → drill chip-welding D/d threshold band lookup.
///
/// Three bands matching the existing `Material::SolidWood` per-species
/// switch:
/// - softwood (Janka ≤ 700 lbf) → 8.0
/// - medium hardwood (700 < Janka ≤ 1500 lbf) → 6.0
/// - dense hardwood (Janka > 1500 lbf, includes out-of-band & NaN) → 5.0
///
/// Out-of-band and non-finite Janka fall into the dense-hardwood
/// bucket (5.0) — the most conservative choice for chip evacuation
/// when we don't know what we're drilling.
fn janka_to_drill_chip_welding_dtd(janka_lbf: f64) -> f64 {
    if !janka_lbf.is_finite() || janka_lbf > 1500.0 {
        5.0 // dense hardwood / unknown
    } else if janka_lbf <= 700.0 {
        8.0 // softwood
    } else {
        6.0 // medium hardwood
    }
}

/// Shared Janka → drill *per-peck* max D/d band lookup.
///
/// Wood-fiber pecking tolerance scales with density the same way
/// chip-welding tolerance does: softwoods clear long, fluffy chips
/// readily and tolerate deep individual pecks; dense exotics trap
/// shorter, harder chips and require shallow pecks even within a
/// pecking cycle. Pre-2026-06-03 this accessor returned a flat 2.0
/// for every wood species, collapsing softwood pecks (literature
/// 3–8×D — Onsrud Drill Chart, FPL Wood Handbook §3.7, Vectric drill
/// defaults) to the same 1.0×D Suggest default as dense hardwood.
///
/// Three bands, mirroring `janka_to_drill_chip_welding_dtd`:
/// - softwood (Janka ≤ 700 lbf) → 6.0  → Suggest default 3.0×D
/// - medium hardwood (700 < Janka ≤ 1500 lbf) → 5.0  → 2.5×D default
/// - dense hardwood (Janka > 1500 lbf, includes out-of-band & NaN) → 4.0 → 2.0×D default
///
/// All three sit safely below the matching chip-welding ceilings
/// (8/6/5), preserving headroom for the operator to raise peck depth
/// manually. Out-of-band and non-finite Janka fall into the
/// dense-hardwood bucket — the most conservative choice when we
/// don't know what we're drilling.
fn janka_to_drill_per_peck_max_dtd(janka_lbf: f64) -> f64 {
    if !janka_lbf.is_finite() || janka_lbf > 1500.0 {
        4.0 // dense hardwood / unknown
    } else if janka_lbf <= 700.0 {
        6.0 // softwood
    } else {
        5.0 // medium hardwood
    }
}

impl Default for Material {
    fn default() -> Self {
        Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        }
    }
}

/// Type alias for the cached material picker structure — keeps clippy's
/// `type_complexity` lint quiet without sprinkling `#[allow]`.
type MaterialPickerBuckets = Vec<(MaterialCategory, Vec<(String, Material)>)>;

/// Process-lifetime cache for [`Material::materials_by_category`]. The
/// merge of [`Material::catalog`] + [`wood_species_library::WOOD_SPECIES_LIBRARY`]
/// is fully static — recomputing it per GUI frame was the dominant
/// cost on the setup page (~148 species × dedup walk × ~30 String
/// allocations, all of which the GUI doesn't need until the menu is
/// open). One-shot at first access, then handed out as a `&'static`
/// slice.
static MATERIALS_BY_CATEGORY: LazyLock<MaterialPickerBuckets> =
    LazyLock::new(Material::build_materials_by_category);

impl Material {
    /// Whether this material is in the "wood class" — solid wood
    /// (either curated species or species-aware Janka variant),
    /// plywood, or other sheet goods. Used by feeds/load logic that
    /// applies wood-router-specific engagement floors and band
    /// shapes regardless of which `Material` variant the project
    /// happens to use.
    ///
    /// **Why a helper, not a match arm at each call site:** new
    /// wood-class variants (e.g. species-aware `SolidWoodByJanka`
    /// when FPL Ch.5 lands) are easy to miss when match arms enumerate
    /// `SolidWood | Plywood | SheetGood` inline — exactly the BUG 2
    /// pattern from the 2026-06-02 feeds-pipeline audit.
    pub fn is_wood_class(&self) -> bool {
        matches!(
            self,
            Material::SolidWood { .. }
                | Material::SolidWoodByJanka { .. }
                | Material::Plywood { .. }
                | Material::SheetGood { .. }
        )
    }

    /// Wood-Janka-normalised hardness index driving feed-rate scaling
    /// in [`crate::feeds::calculate`]. `1.0 = soft wood baseline (Janka
    /// 600 lbf)`; formula `(janka / 600)^0.4`.
    ///
    /// **Renamed from `hardness_index` in S2-8 Option B (2026-06-01).**
    /// The previous name implied a Janka-normalised hardness, but the
    /// per-material-class values are not physically commensurable
    /// (plastics get a hardcoded 0.5, aluminum derives from Brinell,
    /// foam from density). The honest name is "feed scale factor" —
    /// the consumers (`feeds::calculate`, the adaptive ap/ae bracketed
    /// derates) treat this as a per-class scalar driving feed math,
    /// not as a hardness in the physical sense.
    ///
    /// For the wood-class branches the value still equals
    /// `(janka_lbf / 600)^0.4`. Callers that genuinely need the Janka
    /// number — wood-class LUT row hardness matching, drill chip-
    /// welding band lookup — should use [`wood_hardness_lbf`] instead
    /// and receive `None` for non-wood materials.
    ///
    /// **Per-class semantics:**
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
    /// `Material::Custom { feed_scale_factor, .. }` ignores invalid
    /// user inputs (NaN / non-positive) and falls back to the softwood
    /// baseline of 1.0 — keeps downstream `1.0 / hardness` consumers
    /// out of NaN territory.
    ///
    /// [`wood_hardness_lbf`]: Self::wood_hardness_lbf
    pub fn feed_scale_factor(&self) -> f64 {
        match self {
            Material::SolidWood { species } => (species.janka_lbf() / 600.0).powf(0.4),
            Material::SolidWoodByJanka { janka_lbf, .. } => {
                if janka_lbf.is_finite()
                    && (JANKA_CALIBRATED_BAND_LOW_LBF..=JANKA_CALIBRATED_BAND_HIGH_LBF)
                        .contains(janka_lbf)
                {
                    (janka_lbf / 600.0).powf(0.4)
                } else {
                    // Out-of-band parametric Janka — same fall-back as
                    // Custom-with-invalid-factor: softwood baseline.
                    1.0
                }
            }
            Material::Plywood { grade } => (grade.effective_janka_lbf() / 600.0).powf(0.4),
            Material::SheetGood { kind } => (kind.effective_janka_lbf() / 600.0).powf(0.4),
            Material::Plastic { .. } => 0.5,
            Material::Aluminum { alloy } => (alloy.brinell_hb() / 60.0).powf(0.4),
            Material::Foam { density } => match density {
                FoamDensity::Low => 0.15,
                FoamDensity::Medium => 0.25,
                FoamDensity::High => 0.40,
            },
            // Fiberglass: harder to feed than hardwood (abrasive, fiber
            // pullout / delamination risk). 1.3 sits above the
            // hardwood baseline (factor ≈ 1.0–1.4 across the Janka
            // range) but below aluminum. Placeholder pending bench
            // validation — refine when real-cut data lands.
            Material::Fiberglass { .. } => 1.3,
            Material::Custom {
                feed_scale_factor, ..
            } => {
                if feed_scale_factor.is_finite() && *feed_scale_factor > 0.0 {
                    *feed_scale_factor
                } else {
                    // Pre-Phase-1E this returned raw `*feed_scale_factor`,
                    // propagating NaN / negative values into the
                    // feed-rate ramp (`1.0 / x` → NaN/-inf).
                    // Fall back to the softwood baseline (1.0) so the
                    // pipeline never serves an invalid scalar from
                    // user-typed Custom material data.
                    1.0
                }
            }
        }
    }

    /// Janka hardness in lbf for wood-class materials. Returns `None`
    /// for plastics, aluminum, foam, and Custom — those materials have
    /// no Janka reading and the alternative metrics (Brinell, Shore-D,
    /// density) live on per-family accessors instead.
    ///
    /// Added in S2-8 Option B (2026-06-01) so wood-specific LUT row
    /// matching can fetch a real Janka value without going through the
    /// inverse-formula hack
    /// (`janka ≈ feed_scale_factor * 600`) that the Custom branch in
    /// `vendor_normalize` previously used. Wood-class branches
    /// (`SolidWood`, `SolidWoodByJanka`, `Plywood`, `SheetGood`) all
    /// return `Some(janka)`; the parametric branch returns `None` for
    /// out-of-band Janka inputs to match `feed_scale_factor`'s
    /// fallback policy.
    pub fn wood_hardness_lbf(&self) -> Option<f64> {
        match self {
            Material::SolidWood { species } => Some(species.janka_lbf()),
            Material::SolidWoodByJanka { janka_lbf, .. } => {
                if janka_lbf.is_finite()
                    && (JANKA_CALIBRATED_BAND_LOW_LBF..=JANKA_CALIBRATED_BAND_HIGH_LBF)
                        .contains(janka_lbf)
                {
                    Some(*janka_lbf)
                } else {
                    None
                }
            }
            Material::Plywood { grade } => Some(grade.effective_janka_lbf()),
            Material::SheetGood { kind } => Some(kind.effective_janka_lbf()),
            Material::Plastic { .. }
            | Material::Aluminum { .. }
            | Material::Foam { .. }
            | Material::Fiberglass { .. }
            | Material::Custom { .. } => None,
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
            // Phase 5 Step 5.4 (2026-06-01): per-species values now
            // pinned to FPL Ch.5 Table 5-3a shear-parallel-to-grain
            // (12% MC) rows. Source: USDA Forest Service Wood
            // Handbook FPL-GTR-190 (2010), Chapter 5 (Kretschmann),
            // Table 5-3a. Per-species rationale + Δ-from-folklore
            // table at planning/data_ingest_2026-05-30/
            // wood_kc_derivation.md.
            //
            // SCOPE NOTE: values stay in the shear-parallel regime
            // (~6–16 N/mm² for North American species, ~28 for Ipe).
            // The literature delta between shear-block testing and
            // peripheral milling Kc (a 3–5× size-effect multiplier)
            // is NOT applied here — that requires a coordinated
            // anisotropy-factor retune (analog to Phase 2B for sheet
            // goods) and bench validation, both of which are Phase
            // 6+ scope. What this step buys: every value now ties
            // back to a specific FPL Table 5-3a row with a verbatim
            // citation, closing the folklore TODO that had carried
            // since Phase 2B.
            //
            // TODO Phase 6+: absolute-Kc calibration via
            // shear × edge-radius size-effect factor + coordinated
            // anisotropy retune. Will shift smoke baselines; needs
            // operator field validation.
            Material::SolidWood { species } => Some(match species {
                // Mid-band of common low-density softwoods (Pondersa
                // pine 7.8, white spruce 6.7, western redcedar 6.8).
                WoodSpecies::GenericSoftwood => 6.5,
                // NZ/AU species, not in FPL Ch.5 — folklore retained.
                // TODO: source from CSIRO or FRI publications.
                WoodSpecies::RadiataPine => 6.0,
                // FPL: "Longleaf 12% ... 10,400" (kPa shear ∥).
                WoodSpecies::LongleafPine => 10.4,
                // Mid-band of common North American hardwoods (Beech
                // American 13.9, Red Maple 12.8, Northern Red Oak
                // 12.3).
                WoodSpecies::GenericHardwood => 13.0,
                // FPL: "Sugar 12% ... 16,100" (kPa shear ∥).
                WoodSpecies::HardMaple => 16.0,
                // FPL: "Walnut, black 12% ... 9,400" (kPa shear ∥).
                WoodSpecies::Walnut => 9.5,
                // FPL: "Yellow 12% ... 13,000" (kPa shear ∥).
                WoodSpecies::Birch => 13.0,
                // FPL: "White 12% 0.68 ... 13,800" (kPa shear ∥;
                // Quercus alba primary row in the white-oak group).
                WoodSpecies::WhiteOak => 13.8,
                // AU species, not in FPL Ch.5 — folklore retained.
                // TODO: source from CSIRO publications.
                WoodSpecies::Jarrah => 19.0,
                // Brazilian species, not in FPL Ch.5 — folklore
                // retained. TODO: source from EMBRAPA / IPT.
                WoodSpecies::Ipe => 28.0,
            }),
            // Parametric variant — `janka_to_kc_n_per_mm2` returns
            // `None` outside the calibrated band, propagating to the
            // gate which refuses via `MaterialUnvalidated` (right
            // behavior: out-of-band Janka has no citation backing).
            Material::SolidWoodByJanka { janka_lbf, .. } => janka_to_kc_n_per_mm2(*janka_lbf),
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
            // Fiberglass Kc — no fetched primary measurement available
            // (Phase 5 Step 5.3, 2026-06-01). Refuse-first: the gate
            // refuses via `MaterialUnvalidated` rather than fabricating
            // a constant. Bench validation is needed to seed a real
            // value; G10/FR4 typical machining-handbook quotes range
            // 100–150 N/mm² but none are workshop-fidelity peripheral
            // milling measurements.
            Material::Fiberglass { .. } => None,
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
            Material::SolidWood { .. } | Material::SolidWoodByJanka { .. } => 200.0,
            Material::Plywood { .. } => 180.0,
            Material::SheetGood { .. } => 170.0,
            Material::Plastic { .. } => 250.0,
            // Aluminum on a wood router is application-edge; this is a
            // conservative SFM placeholder, not validated machining
            // guidance. Real cutting-data lookup should always come
            // from the vendor LUT for aluminum operations.
            Material::Aluminum { .. } => 100.0,
            Material::Foam { .. } => 300.0,
            // Fiberglass cutting speed — conservative placeholder.
            // Vendor Garr GP-plastics row indicates SMM 79–157 m/min
            // for the composite line. Pick the midpoint as the
            // formula fallback SFM; real fiberglass jobs should
            // route through the LUT.
            Material::Fiberglass { .. } => 120.0,
            Material::Custom { .. } => 200.0,
        }
    }

    /// Base plunge feed rate estimate in mm/min, scaled by tool
    /// diameter. Material-dependent baseline (calibrated against a
    /// 6 mm reference cutter) × diameter scale.
    ///
    /// Pre-2026-06-02 this returned a material-only constant, so a
    /// 3 mm bit got the same plunge envelope as a 12 mm bit — too
    /// fast on the small end, too slow on the large end (audit
    /// finding "Plunge envelope does not scale with tool diameter",
    /// workflow `w39ma2j1y`, fix #7). The 6 mm value is preserved
    /// exactly to keep existing recommendations stable; other
    /// diameters scale linearly.
    pub fn plunge_rate_base(&self, tool_diameter_mm: f64) -> f64 {
        let h = self.feed_scale_factor();
        let baseline_at_6mm = match self {
            Material::SolidWood { .. } | Material::SolidWoodByJanka { .. } => 1000.0 / h,
            Material::Plywood { .. } | Material::SheetGood { .. } => 900.0 / h,
            Material::Plastic { .. } => 1500.0,
            // Conservative placeholder — aluminum plunge guidance
            // should come from the vendor LUT, not this default.
            Material::Aluminum { .. } => 250.0 / h,
            Material::Foam { .. } => 2000.0,
            // Fiberglass plunge — slower than wood to avoid
            // delamination on tool entry. Conservative placeholder.
            Material::Fiberglass { .. } => 200.0 / h,
            Material::Custom { .. } => 800.0 / h,
        };
        let d_scale = (tool_diameter_mm.max(0.1) / 6.0).clamp(0.25, 3.0);
        baseline_at_6mm * d_scale
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
                janka_to_drill_chip_welding_dtd(species.janka_lbf())
            }
            // Parametric variant shares the same band lookup; out-of-band
            // Janka falls into the dense-hardwood (5.0) bucket — most
            // conservative for chip evacuation.
            Material::SolidWoodByJanka { janka_lbf, .. } => {
                janka_to_drill_chip_welding_dtd(*janka_lbf)
            }
            Material::Plywood { .. } | Material::SheetGood { .. } => 5.0,
            Material::Plastic { .. } => 4.0,
            // Aluminum chip welding starts around D/d ≈ 3 (industry
            // rule of thumb; pecks mandatory beyond). Conservative
            // even for 6061 — denser alloys want lower D/d.
            Material::Aluminum { .. } => 3.0,
            Material::Foam { .. } => 12.0,
            // Fiberglass drilling — chips evacuate well (dust, not
            // long chips) but tool wear is severe; conservative
            // threshold sits between plastic and aluminum.
            Material::Fiberglass { .. } => 3.0,
            Material::Custom {
                feed_scale_factor, ..
            } => {
                // Softer materials evacuate better. Clamp to the
                // wood-to-foam range so a degenerate user-supplied
                // factor can't blow this open.
                (8.0 / feed_scale_factor.max(0.5)).clamp(2.0, 12.0)
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
            // Wood species — Janka-banded per-peck max. Pre-2026-06-03
            // this returned a flat 2.0 for every wood, which collapsed
            // softwood Suggest pecks to 1.0×D (matrix band 3–8×D) and
            // matched dense hardwood. See `janka_to_drill_per_peck_max_dtd`
            // for the band rationale and sources.
            Material::SolidWood { species } => {
                janka_to_drill_per_peck_max_dtd(species.janka_lbf())
            }
            Material::SolidWoodByJanka { janka_lbf, .. } => {
                janka_to_drill_per_peck_max_dtd(*janka_lbf)
            }
            Material::Plywood { .. } | Material::SheetGood { .. } => 1.5,
            Material::Plastic { .. } => 1.0,
            // Aluminum per-peck ≤ 1×D — standard machining-textbook
            // limit for chip evacuation without through-coolant.
            Material::Aluminum { .. } => 1.0,
            Material::Foam { .. } => 4.0,
            // Fiberglass per-peck — same as aluminum (textbook 1×D
            // limit for abrasive composites).
            Material::Fiberglass { .. } => 1.0,
            Material::Custom { .. } => 1.5,
        }
    }

    /// Recommended default peck depth (mm) for the Suggest pipeline.
    /// Half the material-specific per-peck max — leaves a safety
    /// margin against the chip-welding ceiling and gives the operator
    /// headroom to raise the value manually if they want fewer pecks.
    ///
    /// Used by `feeds::suggest::apply_drill_defaults` to overwrite
    /// `DrillConfig::peck_depth` when the user runs Suggest. Before
    /// 2026-06-02 the default was hardcoded at 3.0 mm regardless of
    /// tool diameter or material — safe for a 6 mm bit in softwood by
    /// coincidence, unsafe for 3 mm bits, and trivially shallow for
    /// 12 mm bits (audit finding "peck_depth is a hardcoded constant
    /// with no diameter/material scaling").
    pub fn drill_default_peck_depth_mm(&self, tool_diameter_mm: f64) -> f64 {
        let max_factor = self.drill_per_peck_max_dtd();
        let default_factor = max_factor * 0.5;
        (default_factor * tool_diameter_mm.max(0.0)).max(0.1)
    }

    /// Drill plunge feed envelope (mm/min per mm diameter): below the
    /// min the cutter rubs / burns; above the max it breaks or
    /// stalls. Same dispatch pattern as the chip-welding methods.
    ///
    /// Consumed by `tool_load::drill_gates::evaluate_plunge_feed`.
    pub fn drill_plunge_feed_envelope_per_mm(&self) -> (f64, f64) {
        match self {
            Material::SolidWood { .. } | Material::SolidWoodByJanka { .. } => (50.0, 400.0),
            Material::Plywood { .. } | Material::SheetGood { .. } => (40.0, 350.0),
            Material::Plastic { .. } => (60.0, 500.0),
            // Aluminum on a wood router is application-edge —
            // conservative envelope (slower than wood min, lower than
            // plastic max). Real aluminum drilling should always be
            // vendor-LUT-driven.
            Material::Aluminum { .. } => (40.0, 250.0),
            Material::Foam { .. } => (100.0, 1000.0),
            // Fiberglass plunge envelope — sub-aluminum (slower max
            // to limit delamination, lower min because abrasion
            // doesn't reward dwelling at the cut face).
            Material::Fiberglass { .. } => (40.0, 200.0),
            Material::Custom { .. } => (40.0, 500.0),
        }
    }

    /// Display label for UI.
    pub fn label(&self) -> String {
        match self {
            Material::SolidWood { species } => species.label().to_owned(),
            Material::SolidWoodByJanka { label, .. } => label.clone(),
            Material::Plywood { grade } => grade.label().to_owned(),
            Material::SheetGood { kind } => kind.label().to_owned(),
            Material::Plastic { family } => family.label().to_owned(),
            Material::Aluminum { alloy } => alloy.label().to_owned(),
            Material::Foam { density } => format!("Foam ({})", density.label()),
            Material::Fiberglass { grade } => grade.label().to_owned(),
            Material::Custom { name, .. } => name.clone(),
        }
    }

    /// Display category for the hierarchical material picker. Maps
    /// the flat `Material` enum (whose variant shape is implementation
    /// detail) to user-facing groupings the GUI's nested menu walks
    /// (`Wood → Softwood/Hardwood`, `Plywood`, `Sheet Goods`,
    /// `Plastic`, `Aluminum`, `Foam`, `Custom`).
    ///
    /// Wood split: < 900 lbf Janka = softwood, ≥ 900 lbf = hardwood
    /// (the conventional rule-of-thumb cutoff; finer than the
    /// botanical gymnosperm/angiosperm distinction but more useful
    /// for CAM since users look in "Hardwood" for hard species).
    pub fn category(&self) -> MaterialCategory {
        match self {
            Material::SolidWood { species } => {
                if species.janka_lbf() < 900.0 {
                    MaterialCategory::Softwood
                } else {
                    MaterialCategory::Hardwood
                }
            }
            Material::SolidWoodByJanka { janka_lbf, .. } => {
                if *janka_lbf < 900.0 {
                    MaterialCategory::Softwood
                } else {
                    MaterialCategory::Hardwood
                }
            }
            Material::Plywood { .. } => MaterialCategory::Plywood,
            Material::SheetGood { .. } => MaterialCategory::SheetGood,
            Material::Plastic { .. } => MaterialCategory::Plastic,
            Material::Aluminum { .. } => MaterialCategory::Aluminum,
            Material::Foam { .. } => MaterialCategory::Foam,
            Material::Fiberglass { .. } => MaterialCategory::Composite,
            Material::Custom { .. } => MaterialCategory::Custom,
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
            // Composite (Phase 5 Step 5.3)
            (
                "G10 / FR-4",
                Material::Fiberglass {
                    grade: FiberglassGrade::G10Fr4,
                },
            ),
            (
                "Fiberglass (Generic)",
                Material::Fiberglass {
                    grade: FiberglassGrade::Generic,
                },
            ),
        ]
    }

    /// Selectable materials grouped by [`MaterialCategory`] for the
    /// hierarchical GUI picker. Merges the curated [`Material::catalog`]
    /// entries with the parametric [`wood_species_library::WOOD_SPECIES_LIBRARY`]
    /// so the picker has one entry point for everything.
    ///
    /// Dedup policy: when a library species shares a Janka anchor
    /// (within ±2 lbf) with a first-class `WoodSpecies` catalog entry,
    /// the catalog entry wins and the library duplicate is dropped —
    /// the curated species has the hand-tuned per-species `Kc` constant,
    /// the library would only give the folklore-grade `janka/100`
    /// approximation. This replaces the brittle alias-string dedup the
    /// initial GUI implementation used.
    ///
    /// Sort order within each category:
    /// - Softwood / Hardwood: by Janka ascending (softest first)
    /// - All others: by display label
    ///
    /// Custom is omitted — it isn't user-selectable from the picker
    /// (the GUI handles Custom via a separate "advanced" path).
    ///
    /// **Performance:** the underlying merge is computed exactly once
    /// on first call and cached for the process lifetime via
    /// [`MATERIALS_BY_CATEGORY`]. The catalog + library are both static
    /// data — the result never changes — so we don't re-walk
    /// `WOOD_SPECIES_LIBRARY` (~148 entries) per frame. Before the
    /// cache landed the GUI's setup page rebuilt this every frame in
    /// the hierarchical material picker; after Phase E added the
    /// library, that re-allocation became visible as setup-page lag.
    pub fn materials_by_category() -> &'static [(MaterialCategory, Vec<(String, Material)>)] {
        &MATERIALS_BY_CATEGORY
    }

    fn build_materials_by_category() -> Vec<(MaterialCategory, Vec<(String, Material)>)> {
        use wood_species_library::WOOD_SPECIES_LIBRARY;

        // Internal builder type keeps the sort_key alongside the entry
        // until the final strip. Aliased to keep clippy's
        // `type_complexity` lint happy.
        type BuilderEntry = (String, Material, Option<f64>);
        type BuilderBucket = (MaterialCategory, Vec<BuilderEntry>);

        let mut groups: Vec<BuilderBucket> = MaterialCategory::all()
            .iter()
            .filter(|c| !matches!(c, MaterialCategory::Custom))
            .map(|c| (*c, Vec::new()))
            .collect();

        // Helper: push into the bucket matching a material's category.
        let push = |groups: &mut Vec<BuilderBucket>,
                    label: String,
                    mat: Material,
                    sort_key: Option<f64>| {
            let cat = mat.category();
            if let Some(g) = groups.iter_mut().find(|(c, _)| *c == cat) {
                g.1.push((label, mat, sort_key));
            }
        };

        // 1. Curated catalog first — first-class species win on ties.
        for (label, mat) in Self::catalog() {
            let sort_key = match &mat {
                Material::SolidWood { species } => Some(species.janka_lbf()),
                _ => None,
            };
            push(&mut groups, label.to_owned(), mat, sort_key);
        }

        // 2. Wood library — skip any species whose Janka matches a
        //    curated first-class wood (±2 lbf tolerance; tighter than
        //    the literature's measurement spread but loose enough to
        //    absorb integer rounding).
        let curated_jankas: Vec<f64> = Self::catalog()
            .into_iter()
            .filter_map(|(_, mat)| {
                if let Material::SolidWood { species } = mat {
                    Some(species.janka_lbf())
                } else {
                    None
                }
            })
            .collect();

        for entry in WOOD_SPECIES_LIBRARY {
            let already_curated = curated_jankas
                .iter()
                .any(|j| (j - entry.janka_lbf).abs() <= 2.0);
            if already_curated {
                continue;
            }
            // Compact dropdown label — `{display_name}  ({janka} lbf)`.
            // The scientific name is still searchable via the picker's
            // filter (the GUI indexes both `display_name` and
            // `scientific_name` on the `WoodSpeciesEntry`); dropping
            // it from the visible line keeps the menu narrow without
            // losing find-by-binomial.
            let label = format!("{}  ({} lbf)", entry.display_name, entry.janka_lbf as i64);
            let mat = Material::SolidWoodByJanka {
                janka_lbf: entry.janka_lbf,
                label: entry.display_name.to_owned(),
                source_id: entry.source_id.to_owned(),
            };
            push(&mut groups, label, mat, Some(entry.janka_lbf));
        }

        // 3. Sort within each category.
        for (cat, entries) in groups.iter_mut() {
            if cat.is_wood() {
                entries.sort_by(|a, b| {
                    a.2.partial_cmp(&b.2)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then(a.0.cmp(&b.0))
                });
            } else {
                entries.sort_by(|a, b| a.0.cmp(&b.0));
            }
        }

        // Strip the sort_key.
        groups
            .into_iter()
            .map(|(c, e)| (c, e.into_iter().map(|(l, m, _)| (l, m)).collect()))
            .collect()
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
            Material::Fiberglass { grade } => match grade {
                FiberglassGrade::G10Fr4 => "fiberglass_g10_fr4",
                FiberglassGrade::Generic => "fiberglass_generic",
            }
            .to_owned(),
            Material::Custom { name, .. } => format!("custom:{name}"),
            // Parametric variant — emits a key carrying source_id +
            // janka_lbf so a from_key roundtrip can reconstruct the
            // variant (the per-species library is consulted at parse
            // time; if the source_id no longer exists in
            // `WOOD_SPECIES_LIBRARY` we still recover label + janka
            // from the key). Format:
            // `solid_wood_by_janka:{source_id}:{janka_lbf}:{label}`
            // The trailing label may contain ASCII punctuation
            // (parens, slashes, dashes) — the parser splits on the
            // first three `:` only so a label can carry colons too.
            Material::SolidWoodByJanka {
                janka_lbf,
                label,
                source_id,
            } => format!("solid_wood_by_janka:{source_id}:{janka_lbf}:{label}"),
        }
    }

    /// Test-fixture constructor for `Material::Custom { name,
    /// feed_scale_factor, kc }` with audit-defaulted scalars (S3-13
    /// from `planning/tool_kinematics_chipload_audit_2026-05-31.md`).
    ///
    /// `feed_scale_factor = 1.0` (softwood baseline), `kc = 10.0`
    /// (mid-MDF band) — the same triplet that `tool_load/power.rs:470`,
    /// `tool_load/deflection.rs:456`, `tool_load/optimize/mod.rs:859`,
    /// and `compute/validate.rs:538` constructed ad hoc before this
    /// helper landed. Tests that want different defaults should
    /// construct `Material::Custom { ... }` directly.
    ///
    /// Only compiled under `#[cfg(test)]` so production code can't depend
    /// on the test helper. Integration tests in `crates/rs_cam_core/tests/`
    /// that want the same fixture should call `Material::Custom { ... }`
    /// directly (the helper is intentionally scoped to in-crate tests
    /// because that's where the audit's 4 call sites live).
    #[cfg(test)]
    pub fn test_fixture_custom(name: &str) -> Self {
        Material::Custom {
            name: name.to_owned(),
            feed_scale_factor: 1.0,
            kc: 10.0,
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
            "fiberglass_g10_fr4" => Material::Fiberglass {
                grade: FiberglassGrade::G10Fr4,
            },
            "fiberglass_generic" => Material::Fiberglass {
                grade: FiberglassGrade::Generic,
            },
            // Parametric solid-wood-by-Janka key. See `to_key` for the
            // emission format. Parses defensively — any malformed
            // segment falls through to the default.
            key if key.starts_with("solid_wood_by_janka:") => {
                let rest = &key["solid_wood_by_janka:".len()..];
                let mut parts = rest.splitn(3, ':');
                if let (Some(source_id), Some(janka_str), Some(label)) =
                    (parts.next(), parts.next(), parts.next())
                    && let Ok(janka_lbf) = janka_str.parse::<f64>()
                {
                    return Material::SolidWoodByJanka {
                        janka_lbf,
                        label: label.to_owned(),
                        source_id: source_id.to_owned(),
                    };
                }
                Material::default()
            }
            _ => Material::default(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_softwood_baseline_feed_scale_is_one() {
        let m = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        assert!((m.feed_scale_factor() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_feed_scale_factor_ordering() {
        let soft = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let hard = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let ipe = Material::SolidWood {
            species: WoodSpecies::Ipe,
        };
        assert!(soft.feed_scale_factor() < hard.feed_scale_factor());
        assert!(hard.feed_scale_factor() < ipe.feed_scale_factor());
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
    fn custom_with_invalid_factor_falls_back_to_softwood_baseline() {
        // S3-12 fix: Custom material with NaN / non-positive
        // feed_scale_factor propagated raw values into `1.0 / x`
        // consumers, producing NaN / -inf. Now coerces to 1.0
        // (softwood baseline) so the pipeline always has a defined
        // scalar.
        let nan = Material::Custom {
            name: "NaN-factor".into(),
            feed_scale_factor: f64::NAN,
            kc: 10.0,
        };
        assert!((nan.feed_scale_factor() - 1.0).abs() < 1e-9);
        let neg = Material::Custom {
            name: "negative-factor".into(),
            feed_scale_factor: -1.0,
            kc: 10.0,
        };
        assert!((neg.feed_scale_factor() - 1.0).abs() < 1e-9);
        let zero = Material::Custom {
            name: "zero-factor".into(),
            feed_scale_factor: 0.0,
            kc: 10.0,
        };
        assert!((zero.feed_scale_factor() - 1.0).abs() < 1e-9);
        // Valid value passes through unchanged.
        let good = Material::Custom {
            name: "valid".into(),
            feed_scale_factor: 1.4,
            kc: 10.0,
        };
        assert!((good.feed_scale_factor() - 1.4).abs() < 1e-9);
    }

    #[test]
    fn custom_with_invalid_kc_returns_none() {
        let bad = Material::Custom {
            name: "Bad".into(),
            feed_scale_factor: 1.0,
            kc: -1.0,
        };
        assert_eq!(bad.kc_n_per_mm2(), None);
        let nan = Material::Custom {
            name: "NaN".into(),
            feed_scale_factor: 1.0,
            kc: f64::NAN,
        };
        assert_eq!(nan.kc_n_per_mm2(), None);
        let good = Material::Custom {
            name: "OK".into(),
            feed_scale_factor: 1.0,
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
        assert!(foam.feed_scale_factor() < soft_wood.feed_scale_factor());
    }

    #[test]
    fn wood_hardness_lbf_returns_some_for_wood_class_only() {
        // S2-8 Option B: wood_hardness_lbf() returns Some(Janka) for
        // wood / plywood / sheet-good and None for everything else.
        assert_eq!(
            Material::SolidWood {
                species: WoodSpecies::GenericSoftwood,
            }
            .wood_hardness_lbf(),
            Some(600.0)
        );
        assert!(
            Material::SolidWoodByJanka {
                janka_lbf: 1450.0,
                label: "Hard Maple".into(),
                source_id: "test".into(),
            }
            .wood_hardness_lbf()
            .is_some()
        );
        // Out-of-band parametric Janka returns None (matches the
        // calibrated-band policy in feed_scale_factor / kc_n_per_mm2).
        assert_eq!(
            Material::SolidWoodByJanka {
                janka_lbf: 100.0,
                label: "below band".into(),
                source_id: "test".into(),
            }
            .wood_hardness_lbf(),
            None
        );
        assert!(
            Material::Plywood {
                grade: PlywoodGrade::BalticBirch,
            }
            .wood_hardness_lbf()
            .is_some()
        );
        assert!(
            Material::SheetGood {
                kind: SheetGoodKind::Mdf,
            }
            .wood_hardness_lbf()
            .is_some()
        );
        assert_eq!(
            Material::Plastic {
                family: PlasticFamily::Hdpe,
            }
            .wood_hardness_lbf(),
            None
        );
        assert_eq!(
            Material::Aluminum {
                alloy: AluminumAlloy::Alloy6061T6,
            }
            .wood_hardness_lbf(),
            None
        );
        assert_eq!(
            Material::Foam {
                density: FoamDensity::Medium,
            }
            .wood_hardness_lbf(),
            None
        );
        assert_eq!(
            Material::test_fixture_custom("custom").wood_hardness_lbf(),
            None
        );
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
    fn test_hard_maple_feed_scale_factor_matches_reference() {
        // Reference: (1450/600)^0.4 ≈ 1.425
        let m = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        assert!((m.feed_scale_factor() - 1.425).abs() < 0.01);
    }
}
