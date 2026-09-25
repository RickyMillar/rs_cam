//! One typed cutting-force line per material (ruling B6, 2026-09-24).
//!
//! ## The line
//!
//! The force per mm of engaged edge is affine in the chip thickness `h`:
//!
//! ```text
//! Fc / ap = Ks · h + F_edge
//! ```
//!
//! [`crate::material::Material::force_line`] returns one [`ForceLine`] or
//! one [`ForceLineRefusal`]. Every consumer reads the same line:
//! `feeds::force` (deflection), `feeds::efficiency`, `feeds::predict`,
//! `tool_load::power` and `tool_load::deflection`. Power reads
//! `line × grain_factor`. Deflection reads the line alone. Every shipped
//! line has `grain_factor = 1.0`.
//!
//! ## The two printed forms
//!
//! - Solid wood: the Curti 2021 density law (Table 5, helix 0). Curti
//!   divides both Ks and Int by the density, so `Ks = Ks_n · ρ` and
//!   `F_edge = Int_n · ρ`. Each term takes its upper envelope over the
//!   grain angle and over up- and down-milling. [`curti_helix0_envelope`]
//!   computes the envelope from the printed quadratics; no envelope value
//!   is typed as a literal. The envelope holds the grain spread, so the
//!   grain factor is 1.0.
//! - MDF: the Goli 2018 printed line (Table 3), Ks 31.44 N/mm² and Int
//!   3.36 N/mm. MDF is isotropic in plane, so the grain factor is 1.0.
//!
//! Every other material refuses with a named reason.
//!
//! ## Density
//!
//! `ρ = SG × 1000 × 1.12`, from FPL Table 5-3a at 12 % MC. FPL footnote b:
//! "Specific gravity based on weight when ovendry and volume at 12%
//! moisture content". So `SG × 1000` alone is the oven-dry mass over the
//! 12 % volume, and the 1.12 factor adds the 12 % water back.
//!
//! A species with no Table 5-3a row (radiata pine, jarrah, ipe) reads the
//! basic SG `Gb` of FPL Table 5-5a (ovendry mass, green volume). FPL Ch.4
//! Eq. (4-11) converts it to the 12 % MC volume basis,
//! `G12 = Gb / [1 − 0.265 Gb (1 − 12/MCfs)]` with MCfs 30 %, and then the
//! same `G12 × 1000 × 1.12` gives ρ ([`fpl_eq_4_11_g12`]).
//!
//! ## Chip range
//!
//! A mean chip outside the printed range does not refuse. The point carries
//! a [`ChipRegime`], and the card states the extrapolation. The mean chip
//! is `h_m = fz · (1 − cos ψ) / ψ` (Goli 2018 and Curti 2021 both fit
//! against it).
//!
//! Sources: `planning/extrapolation_2026-09-24/fetch/G7/verified_rows.json`
//! and `fetch/G7/sources/marcon2021_generalized_force_model.txt` (Curti R.,
//! Marcon B., Denaud L., Togni M., Furferi R., Goli G. 2021, Eur. J. Wood
//! Wood Prod., doi:10.1007/s00107-021-01667-5); Goli G. et al. 2018,
//! Materials 11(12):2575, doi:10.3390/ma11122575. Plan:
//! `planning/extrapolation_2026-09-24/B6_PLAN.md`.

/// Source id of the Curti 2021 density law.
pub const CURTI_2021_SOURCE_ID: &str = "g7_curti2021_generalized_wood_model";

/// Source id of the Goli 2018 MDF line.
pub const GOLI_2018_SOURCE_ID: &str = "g7_goli2018_round_shape_ks";

/// Form id of the solid-wood line.
pub const CURTI_2021_FORM_ID: &str = "curti2021_density_law_helix0_envelope";

/// Form id of the MDF line.
pub const GOLI_2018_FORM_ID: &str = "goli2018_mdf_affine";

/// The density range of the Curti 2021 fit (kg/m³). Curti tests five
/// species from 287.1 to 1079.5 kg/m³ (Table 1) and names the range
/// "between 287 and 1080 kg/m3 to avoid extrapolation".
pub const CURTI_DENSITY_RANGE_KG_M3: (f64, f64) = (287.0, 1080.0);

/// The mean chip range of the Curti 2021 fit (mm): "average uncut chip
/// thicknesses h roughly varying from 40 µm ... to 100 µm".
pub const CURTI_CHIP_RANGE_MM: (f64, f64) = (0.04, 0.10);

/// The grain-angle range of the envelope (degrees; 0 = along the grain).
/// Curti prints "varying from 0° to 179°". Each helix-0 maximum is at an
/// interior vertex, so the end of the range does not move the envelope.
pub const CURTI_GRAIN_ANGLE_RANGE_DEG: (f64, f64) = (0.0, 180.0);

/// The mean chip range of the Goli 2018 MDF fit (mm), Table 3.
pub const GOLI_2018_MDF_CHIP_RANGE_MM: (f64, f64) = (0.041, 0.091);

/// Goli 2018 Table 3, MDF, up-milling: "Ks [N mm−2] | 31.44 (2.68) |
/// 25.81 | 35.58".
pub const GOLI_2018_MDF_KS_N_PER_MM2: f64 = 31.44;

/// Goli 2018 Table 3, MDF, up-milling: "Int [N mm−1] | 3.36 (0.27) | 2.96 |
/// 3.83".
pub const GOLI_2018_MDF_INT_N_PER_MM: f64 = 3.36;

/// The density of the Goli 2018 MDF specimen (kg/m³). The hover text
/// states it. The engine does not scale the MDF line by density.
pub const GOLI_2018_MDF_DENSITY_KG_M3: f64 = 711.0;

/// FPL Table 5-3a gives SG on the oven-dry mass and the 12 % MC volume.
/// This factor gives the density at 12 % MC.
pub const FPL_12PCT_MOISTURE_FACTOR: f64 = 1.12;

/// Source id of the FPL GTR-190 Chapter 5 Table 5-5a rows (basic SG).
pub const FPL_TABLE_5_5A_SOURCE_ID: &str = "g7_fpl_gtr190_ch5_table5_5a";

/// Source id of the FPL GTR-190 Chapter 4 SG conversion, Eq. (4-11).
pub const FPL_CH4_SG_CONVERSION_SOURCE_ID: &str = "g7_fpl_gtr190_ch4_sg_conversion";

/// The source id of the Table 5-3a rows (the species library id).
pub const FPL_TABLE_5_3A_SOURCE_ID: &str = "fpl_ch5_2010";

/// FPL Ch.4 Eq. (4-10): `S0 = 26.5 Gb`, so Eq. (4-11) carries 0.265.
pub const FPL_EQ_4_11_COEFF: f64 = 0.265;

/// The fibre saturation point MCfs (%) for Eq. (4-11). FPL prints no
/// species value for radiata pine, jarrah or ipe; Fig. 4-6 is "assumed to
/// be 30% MC", the FPL default.
pub const FPL_FIBRE_SATURATION_MC_PCT: f64 = 30.0;

/// The target moisture content (%) of the conversion: the Table 5-3a
/// basis.
pub const FPL_TARGET_MC_PCT: f64 = 12.0;

/// FPL Ch.4 Eq. (4-11) at x = 12 % and MCfs = 30 %:
/// `G12 = Gb / [1 − 0.265 Gb (1 − x/MCfs)]`, from a basic SG `Gb`
/// (ovendry mass, green volume) to the 12 % MC volume basis.
///
/// FPL's own worked example (Ch.4, after Eq. (4-11)): white ash, Gb 0.55,
/// reads G12 = 0.605 from Figure 4-6; the equation gives 0.6027.
#[must_use]
pub fn fpl_eq_4_11_g12(basic_specific_gravity: f64) -> f64 {
    let a = 1.0 - FPL_TARGET_MC_PCT / FPL_FIBRE_SATURATION_MC_PCT;
    basic_specific_gravity / (1.0 - FPL_EQ_4_11_COEFF * basic_specific_gravity * a)
}

/// One printed quadratic in grain angle, `a·GA² + b·GA + c`, GA in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GrainQuadratic {
    /// The GA² coefficient.
    pub a: f64,
    /// The GA coefficient.
    pub b: f64,
    /// The constant term.
    pub c: f64,
}

impl GrainQuadratic {
    /// A quadratic from its three printed coefficients.
    #[must_use]
    pub const fn new(a: f64, b: f64, c: f64) -> Self {
        Self { a, b, c }
    }

    /// The value at grain angle `ga_deg`.
    #[must_use]
    pub fn at(self, ga_deg: f64) -> f64 {
        (self.a * ga_deg + self.b) * ga_deg + self.c
    }

    /// The maximum over [`CURTI_GRAIN_ANGLE_RANGE_DEG`], as `(value, GA)`.
    ///
    /// The candidates are the two ends and, for a downward parabola, the
    /// vertex `−b / 2a` when it lies inside the range.
    #[must_use]
    pub fn max_over_grain(self) -> (f64, f64) {
        let (lo, hi) = CURTI_GRAIN_ANGLE_RANGE_DEG;
        let mut best = (self.at(lo), lo);
        let end = (self.at(hi), hi);
        if end.0 > best.0 {
            best = end;
        }
        if self.a < 0.0 {
            let vertex = -self.b / (2.0 * self.a);
            if (lo..=hi).contains(&vertex) {
                let top = (self.at(vertex), vertex);
                if top.0 > best.0 {
                    best = top;
                }
            }
        }
        best
    }
}

/// Curti 2021 Table 5, up-milling, helix 0: "Kmodel = −5 ⋅ 10−6 GA2 + 1⋅10−3
/// GA + 26⋅10−3". Unit: N/mm² per kg/m³ (Table 5 prints no unit; the
/// verifier infers it from the density normalisation).
pub const CURTI_UP_HELIX0_KS_N: GrainQuadratic = GrainQuadratic::new(-5e-6, 1e-3, 26e-3);

/// Curti 2021 Table 5, up-milling, helix 0: "Intmodel = −1⋅10−7 GA2 + 1⋅10−5
/// GA + 55⋅10−4". Unit: N/mm per kg/m³.
pub const CURTI_UP_HELIX0_INT_N: GrainQuadratic = GrainQuadratic::new(-1e-7, 1e-5, 55e-4);

/// Curti 2021 Table 5, down-milling, helix 0: "Kmodel = −3⋅10−6 GA2 + 5 ⋅
/// 10−4 GA + 56⋅10−3". Unit: N/mm² per kg/m³.
pub const CURTI_DOWN_HELIX0_KS_N: GrainQuadratic = GrainQuadratic::new(-3e-6, 5e-4, 56e-3);

/// Curti 2021 Table 5, down-milling, helix 0: "Intmodel = −3⋅10−7 GA2 +
/// 4⋅10−5 A + 47⋅10−4". The source prints "A" for "GA" (a typo the
/// verifier records). Unit: N/mm per kg/m³.
pub const CURTI_DOWN_HELIX0_INT_N: GrainQuadratic = GrainQuadratic::new(-3e-7, 4e-5, 47e-4);

/// The milling mode of one printed Curti row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MillingMode {
    /// Up-milling (conventional).
    Up,
    /// Down-milling (climb).
    Down,
}

impl MillingMode {
    /// The operator label, for example "down-milling".
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Up => "up-milling",
            Self::Down => "down-milling",
        }
    }
}

/// One term of the envelope: the value and the printed state it comes from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnvelopeTerm {
    /// The normalised value (per kg/m³).
    pub value: f64,
    /// The grain angle (degrees) of the maximum.
    pub grain_angle_deg: f64,
    /// The milling mode of the maximum.
    pub mode: MillingMode,
}

/// The larger of the up and down maxima of one term.
fn envelope_term(up: GrainQuadratic, down: GrainQuadratic) -> EnvelopeTerm {
    let (up_value, up_ga) = up.max_over_grain();
    let (down_value, down_ga) = down.max_over_grain();
    if down_value > up_value {
        EnvelopeTerm {
            value: down_value,
            grain_angle_deg: down_ga,
            mode: MillingMode::Down,
        }
    } else {
        EnvelopeTerm {
            value: up_value,
            grain_angle_deg: up_ga,
            mode: MillingMode::Up,
        }
    }
}

/// The helix-0 envelope, per term, with the state each term comes from:
/// `(Ks_n, Int_n)`.
#[must_use]
pub fn curti_helix0_envelope_terms() -> (EnvelopeTerm, EnvelopeTerm) {
    (
        envelope_term(CURTI_UP_HELIX0_KS_N, CURTI_DOWN_HELIX0_KS_N),
        envelope_term(CURTI_UP_HELIX0_INT_N, CURTI_DOWN_HELIX0_INT_N),
    )
}

/// The helix-0 envelope `(Ks_n, Int_n)`: for each term, the maximum over
/// the grain angle, then the larger mode. Derived: Ks_n 0.0768333 (down,
/// GA 83.3), Int_n 0.0060333 (down, GA 66.7).
///
/// Public so the `tests/` sentry can pin it.
#[must_use]
pub fn curti_helix0_envelope() -> (f64, f64) {
    let (ks, int) = curti_helix0_envelope_terms();
    (ks.value, int.value)
}

/// One FPL row and its printed SG: the 12 % MC SG of a Table 5-3a row, or
/// the basic SG `Gb` of a Table 5-5a row ([`DensitySource::FplBasicRow`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FplSgRow {
    /// The row name as FPL prints it.
    pub row: &'static str,
    /// The printed specific gravity.
    pub specific_gravity: f64,
}

impl FplSgRow {
    /// A row from its name and its printed SG.
    #[must_use]
    pub const fn new(row: &'static str, specific_gravity: f64) -> Self {
        Self {
            row,
            specific_gravity,
        }
    }
}

/// Where a [`WoodDensity`] comes from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DensitySource {
    /// One FPL Table 5-3a row.
    FplRow(FplSgRow),
    /// The mean SG of several FPL Table 5-3a rows. A generic species takes
    /// the mean of the rows that its old `Kc` comment named.
    FplMean(&'static [FplSgRow]),
    /// The `specific_gravity_12` of one FPL row of the species library
    /// (`data/wood_species.toml`). The string is the row's display name.
    FplLibraryRow(&'static str),
    /// One FPL Table 5-5a row. The row holds the printed basic SG `Gb`;
    /// Eq. (4-11) converts it to G12 ([`fpl_eq_4_11_g12`]).
    FplBasicRow(FplSgRow),
}

impl DensitySource {
    /// The FPL row names, for the hover text and the MCP field. A mean
    /// reads "mean of <row>; <row>; <row>"; row names carry commas.
    #[must_use]
    pub fn row_names(&self) -> String {
        match self {
            Self::FplRow(row) => row.row.to_owned(),
            Self::FplMean(rows) => {
                let names: Vec<&str> = rows.iter().map(|r| r.row).collect();
                format!("mean of {}", names.join("; "))
            }
            Self::FplLibraryRow(name) => (*name).to_owned(),
            Self::FplBasicRow(row) => row.row.to_owned(),
        }
    }

    /// A short machine-readable id: `fpl_row`, `fpl_mean`,
    /// `fpl_library_row` or `fpl_basic_row`.
    #[must_use]
    pub const fn kind_id(&self) -> &'static str {
        match self {
            Self::FplRow(_) => "fpl_row",
            Self::FplMean(_) => "fpl_mean",
            Self::FplLibraryRow(_) => "fpl_library_row",
            Self::FplBasicRow(_) => "fpl_basic_row",
        }
    }

    /// The source ids the density reads.
    #[must_use]
    pub const fn source_ids(&self) -> &'static [&'static str] {
        match self {
            Self::FplRow(_) | Self::FplMean(_) | Self::FplLibraryRow(_) => {
                &[FPL_TABLE_5_3A_SOURCE_ID]
            }
            Self::FplBasicRow(_) => &[FPL_TABLE_5_5A_SOURCE_ID, FPL_CH4_SG_CONVERSION_SOURCE_ID],
        }
    }

    /// The printed basic SG `Gb` of a Table 5-5a row; `None` otherwise.
    #[must_use]
    pub const fn basic_specific_gravity(&self) -> Option<f64> {
        match self {
            Self::FplBasicRow(row) => Some(row.specific_gravity),
            Self::FplRow(_) | Self::FplMean(_) | Self::FplLibraryRow(_) => None,
        }
    }

    /// One line for the card and the MCP field, for example
    /// "FPL Table 5-3a, 12 % MC: Pine, longleaf", or for a Table 5-5a row
    /// "FPL Table 5-5a basic SG 0.42 (ovendry weight, green volume),
    /// converted to 12 % MC by FPL Ch.4 Eq. (4-11) with MCfs 30 %: G12
    /// 0.4501".
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::FplBasicRow(row) => format!(
                "FPL Table 5-5a basic SG {gb:.2} (ovendry weight, green volume), converted to 12 \
                 % MC by FPL Ch.4 Eq. (4-11) with MCfs {mcfs:.0} %: G12 {g12:.4}",
                gb = row.specific_gravity,
                mcfs = FPL_FIBRE_SATURATION_MC_PCT,
                g12 = fpl_eq_4_11_g12(row.specific_gravity),
            ),
            Self::FplRow(_) | Self::FplMean(_) | Self::FplLibraryRow(_) => {
                format!("FPL Table 5-3a, 12 % MC: {}", self.row_names())
            }
        }
    }
}

/// A wood density at 12 % MC, with its SG and its source.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WoodDensity {
    specific_gravity: f64,
    rho_kg_m3: f64,
    source: DensitySource,
}

impl WoodDensity {
    /// `ρ = SG × 1000 × 1.12` from an FPL SG.
    #[must_use]
    pub fn from_fpl_specific_gravity(specific_gravity: f64, source: DensitySource) -> Self {
        Self {
            specific_gravity,
            rho_kg_m3: specific_gravity * 1000.0 * FPL_12PCT_MOISTURE_FACTOR,
            source,
        }
    }

    /// One FPL row.
    #[must_use]
    pub fn fpl_row(row: FplSgRow) -> Self {
        Self::from_fpl_specific_gravity(row.specific_gravity, DensitySource::FplRow(row))
    }

    /// One FPL Table 5-5a row: the printed basic SG `Gb`, converted to G12
    /// by Eq. (4-11), then `ρ = G12 × 1000 × 1.12`. The stored
    /// [`Self::specific_gravity`] is G12; [`Self::basic_specific_gravity`]
    /// keeps the printed `Gb`.
    #[must_use]
    pub fn fpl_basic_row(row: FplSgRow) -> Self {
        Self::from_fpl_specific_gravity(
            fpl_eq_4_11_g12(row.specific_gravity),
            DensitySource::FplBasicRow(row),
        )
    }

    /// The mean SG of several FPL rows. An empty list gives a NaN SG, and
    /// [`ForceLine::curti_density_law`] refuses it.
    #[must_use]
    pub fn fpl_mean(rows: &'static [FplSgRow]) -> Self {
        let sum: f64 = rows.iter().map(|r| r.specific_gravity).sum();
        let mean = sum / rows.len() as f64;
        Self::from_fpl_specific_gravity(mean, DensitySource::FplMean(rows))
    }

    /// A density given in kg/m³. The SG is `ρ / 1120`. The range tests
    /// use this door, because an SG does not land on 287.0 exactly.
    #[must_use]
    pub fn from_density_kg_m3(rho_kg_m3: f64, source: DensitySource) -> Self {
        Self {
            specific_gravity: rho_kg_m3 / (1000.0 * FPL_12PCT_MOISTURE_FACTOR),
            rho_kg_m3,
            source,
        }
    }

    /// The SG at 12 % MC (G12; for a Table 5-5a row, the converted value).
    #[must_use]
    pub const fn specific_gravity(&self) -> f64 {
        self.specific_gravity
    }

    /// The printed basic SG `Gb`, for a Table 5-5a row only.
    #[must_use]
    pub const fn basic_specific_gravity(&self) -> Option<f64> {
        self.source.basic_specific_gravity()
    }

    /// The density at 12 % MC (kg/m³).
    #[must_use]
    pub const fn rho_kg_m3(&self) -> f64 {
        self.rho_kg_m3
    }

    /// Where the SG comes from.
    #[must_use]
    pub const fn source(&self) -> DensitySource {
        self.source
    }
}

/// The printed form a [`ForceLine`] comes from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ForceBasis {
    /// Curti 2021, helix 0, upper envelope, times the density.
    CurtiDensityLaw {
        /// The density the law reads.
        density: WoodDensity,
    },
    /// Goli 2018 Table 3, MDF, as printed.
    Goli2018Mdf,
}

/// Where a mean chip sits against the printed chip range of a line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChipRegime {
    /// Inside the printed range (the ends count as inside).
    Measured,
    /// Below the printed range. The line is extended.
    BelowMeasured {
        /// The mean chip (mm).
        mean_chip_mm: f64,
    },
    /// Above the printed range. The line is extended.
    AboveMeasured {
        /// The mean chip (mm).
        mean_chip_mm: f64,
    },
}

impl ChipRegime {
    /// The MCP wire id: `measured`, `below_measured_range` or
    /// `above_measured_range`.
    #[must_use]
    pub const fn wire_id(&self) -> &'static str {
        match self {
            Self::Measured => "measured",
            Self::BelowMeasured { .. } => "below_measured_range",
            Self::AboveMeasured { .. } => "above_measured_range",
        }
    }

    /// `true` when the chip is inside the printed range.
    #[must_use]
    pub const fn is_measured(&self) -> bool {
        matches!(self, Self::Measured)
    }
}

/// One typed cutting-force line. The fields are private: the two
/// constructors are the only doors, so a line always has a printed basis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForceLine {
    ks_n_per_mm2: f64,
    f_edge_n_per_mm: f64,
    grain_factor: f64,
    chip_range_mm: (f64, f64),
    basis: ForceBasis,
}

impl ForceLine {
    /// The Curti 2021 density law at `density`: `Ks = Ks_n · ρ`,
    /// `F_edge = Int_n · ρ`, with the helix-0 envelope.
    ///
    /// # Errors
    ///
    /// - [`ForceLineRefusal::NoDensity`] when ρ is not finite or not
    ///   positive.
    /// - [`ForceLineRefusal::DensityOutOfRange`] when ρ is outside
    ///   [`CURTI_DENSITY_RANGE_KG_M3`]. The ends are inside.
    pub fn curti_density_law(density: WoodDensity) -> Result<Self, ForceLineRefusal> {
        let rho = density.rho_kg_m3();
        if !(rho.is_finite() && rho > 0.0) {
            return Err(ForceLineRefusal::NoDensity);
        }
        let (lo, hi) = CURTI_DENSITY_RANGE_KG_M3;
        if !(lo..=hi).contains(&rho) {
            return Err(ForceLineRefusal::DensityOutOfRange { rho_kg_m3: rho });
        }
        let (ks_n, int_n) = curti_helix0_envelope();
        Ok(Self {
            ks_n_per_mm2: ks_n * rho,
            f_edge_n_per_mm: int_n * rho,
            grain_factor: 1.0,
            chip_range_mm: CURTI_CHIP_RANGE_MM,
            basis: ForceBasis::CurtiDensityLaw { density },
        })
    }

    /// The Goli 2018 MDF line, as printed.
    #[must_use]
    pub const fn goli_2018_mdf() -> Self {
        Self {
            ks_n_per_mm2: GOLI_2018_MDF_KS_N_PER_MM2,
            f_edge_n_per_mm: GOLI_2018_MDF_INT_N_PER_MM,
            grain_factor: 1.0,
            chip_range_mm: GOLI_2018_MDF_CHIP_RANGE_MM,
            basis: ForceBasis::Goli2018Mdf,
        }
    }

    /// The slope `Ks` (N/mm²).
    #[must_use]
    pub const fn ks_n_per_mm2(&self) -> f64 {
        self.ks_n_per_mm2
    }

    /// The edge intercept `F_edge` (N per mm of engaged edge).
    #[must_use]
    pub const fn f_edge_n_per_mm(&self) -> f64 {
        self.f_edge_n_per_mm
    }

    /// The factor that power applies on both terms. 1.0 on every shipped
    /// line. Deflection does not read it.
    #[must_use]
    pub const fn grain_factor(&self) -> f64 {
        self.grain_factor
    }

    /// The printed mean-chip range `(lo, hi)` (mm).
    #[must_use]
    pub const fn chip_range_mm(&self) -> (f64, f64) {
        self.chip_range_mm
    }

    /// The printed form this line comes from.
    #[must_use]
    pub const fn basis(&self) -> ForceBasis {
        self.basis
    }

    /// The equivalent specific force `Ks + F_edge / h` (N/mm²) at chip
    /// `h_mm`. `None` when `h_mm` is not finite and positive.
    #[must_use]
    pub fn kc_eq(&self, h_mm: f64) -> Option<f64> {
        (h_mm.is_finite() && h_mm > 0.0).then_some(self.ks_n_per_mm2 + self.f_edge_n_per_mm / h_mm)
    }

    /// The source id, as `planning/extrapolation_2026-09-24/fetch/G7/
    /// sources.json` names it. `data/vendor_lut/source_manifest.json` does
    /// not list the G7 sources yet.
    #[must_use]
    pub const fn source_id(&self) -> &'static str {
        match self.basis {
            ForceBasis::CurtiDensityLaw { .. } => CURTI_2021_SOURCE_ID,
            ForceBasis::Goli2018Mdf => GOLI_2018_SOURCE_ID,
        }
    }

    /// The form id: [`CURTI_2021_FORM_ID`] or [`GOLI_2018_FORM_ID`].
    #[must_use]
    pub const fn form_id(&self) -> &'static str {
        match self.basis {
            ForceBasis::CurtiDensityLaw { .. } => CURTI_2021_FORM_ID,
            ForceBasis::Goli2018Mdf => GOLI_2018_FORM_ID,
        }
    }

    /// The density the line reads. `None` for MDF: the Goli line is
    /// printed, not scaled by density.
    #[must_use]
    pub const fn density(&self) -> Option<WoodDensity> {
        match self.basis {
            ForceBasis::CurtiDensityLaw { density } => Some(density),
            ForceBasis::Goli2018Mdf => None,
        }
    }

    /// The SG at 12 % MC, when the line reads a density.
    #[must_use]
    pub fn specific_gravity(&self) -> Option<f64> {
        self.density().map(|d| d.specific_gravity())
    }

    /// The density (kg/m³), when the line reads one.
    #[must_use]
    pub fn density_kg_m3(&self) -> Option<f64> {
        self.density().map(|d| d.rho_kg_m3())
    }

    /// The printed basic SG `Gb`, when the density comes from a Table 5-5a
    /// row.
    #[must_use]
    pub fn basic_specific_gravity(&self) -> Option<f64> {
        self.density().and_then(|d| d.basic_specific_gravity())
    }

    /// The density source line, when the line reads a density.
    #[must_use]
    pub fn density_source_text(&self) -> Option<String> {
        self.density().map(|d| d.source().describe())
    }

    /// Where `mean_chip_mm` sits against [`Self::chip_range_mm`]. The ends
    /// are inside. A chip that is not a number reads as below.
    #[must_use]
    pub fn chip_regime(&self, mean_chip_mm: f64) -> ChipRegime {
        let (lo, hi) = self.chip_range_mm;
        if (lo..=hi).contains(&mean_chip_mm) {
            ChipRegime::Measured
        } else if mean_chip_mm > hi {
            ChipRegime::AboveMeasured { mean_chip_mm }
        } else {
            ChipRegime::BelowMeasured { mean_chip_mm }
        }
    }

    /// This line, evaluated at one mean chip.
    #[must_use]
    pub fn at_mean_chip(self, mean_chip_mm: f64) -> ForceAtPoint {
        ForceAtPoint {
            line: self,
            mean_chip_mm,
            chip_regime: self.chip_regime(mean_chip_mm),
        }
    }

    /// The card line (plan §6), for example "Force line: Curti 2021
    /// density law, ρ 676 kg/m³ (FPL SG 0.60), upper envelope".
    #[must_use]
    pub fn card_text(&self) -> String {
        match self.basis {
            ForceBasis::CurtiDensityLaw { density } => match density.basic_specific_gravity() {
                None => format!(
                    "Force line: Curti 2021 density law, ρ {:.0} kg/m³ (FPL SG {:.2}), upper \
                     envelope",
                    density.rho_kg_m3(),
                    density.specific_gravity()
                ),
                Some(gb) => format!(
                    "Force line: Curti 2021 density law, ρ {:.0} kg/m³ (FPL Gb {gb:.2}, G12 \
                     {:.2}), upper envelope",
                    density.rho_kg_m3(),
                    density.specific_gravity()
                ),
            },
            ForceBasis::Goli2018Mdf => "Force line: Goli 2018 MDF, printed".to_owned(),
        }
    }

    /// The hover text (plan §6).
    #[must_use]
    pub fn detail_text(&self) -> String {
        match self.basis {
            ForceBasis::CurtiDensityLaw { density } => {
                let (ks_term, int_term) = curti_helix0_envelope_terms();
                let (rho_lo, rho_hi) = CURTI_DENSITY_RANGE_KG_M3;
                let (h_lo, h_hi) = CURTI_CHIP_RANGE_MM;
                format!(
                    "Ks = {ks_n:.7} × ρ = {ks:.2} N/mm², F_edge = {int_n:.7} × ρ = {fe:.3} N/mm \
                     per mm of edge. Ks_n and Int_n are the upper envelope of Curti 2021 Table 5 \
                     at helix 0, over grain angle 0-180° and up- and down-milling, taken per \
                     term (derived; Ks_n from {ks_mode} GA {ks_ga:.0}°, Int_n from {int_mode} GA \
                     {int_ga:.0}°). ρ = SG {sg:.4} × 1000 × 1.12 = {rho:.1} kg/m³ at 12 % MC, \
                     from {basis}. Valid: ρ {rho_lo:.0}-{rho_hi:.0} kg/m³, mean \
                     chip {h_lo:.2}-{h_hi:.2} mm, rake 25°, one laboratory; Table 6 NRMSE \
                     8.1-37.8 %. Helix: the engine has no helix input; helix 0 is the largest \
                     force Curti prints. Grain factor 1.0: the envelope holds the grain spread. \
                     Source: {source}.",
                    ks_n = ks_term.value,
                    ks = self.ks_n_per_mm2,
                    int_n = int_term.value,
                    fe = self.f_edge_n_per_mm,
                    ks_mode = ks_term.mode.label(),
                    ks_ga = ks_term.grain_angle_deg,
                    int_mode = int_term.mode.label(),
                    int_ga = int_term.grain_angle_deg,
                    sg = density.specific_gravity(),
                    rho = density.rho_kg_m3(),
                    basis = density_basis_clause(density.source()),
                    source = CURTI_2021_SOURCE_ID,
                )
            }
            ForceBasis::Goli2018Mdf => {
                let (h_lo, h_hi) = GOLI_2018_MDF_CHIP_RANGE_MM;
                format!(
                    "Ks {ks:.2} N/mm², F_edge {fe:.2} N/mm per mm of edge, printed (Goli 2018 \
                     Table 3: up-milling, straight blade, rake 25°, {rho:.0} kg/m³). Valid: mean \
                     chip {h_lo:.3}-{h_hi:.3} mm. Grain factor 1.0: MDF is isotropic in plane. \
                     Source: {source}.",
                    ks = self.ks_n_per_mm2,
                    fe = self.f_edge_n_per_mm,
                    rho = GOLI_2018_MDF_DENSITY_KG_M3,
                    source = GOLI_2018_SOURCE_ID,
                )
            }
        }
    }

    /// The confidence text of the power gate (plan §6).
    #[must_use]
    pub fn power_confidence_text(&self) -> String {
        let (lo, hi) = self.chip_range_mm;
        format!(
            "force line {} ({}); grain factor {:.1}; helix 0 (no helix input); a sample outside \
             the measured mean chip {lo:.3}-{hi:.3} mm extends the line",
            self.form_id(),
            self.source_id(),
            self.grain_factor,
        )
    }
}

/// The density clause of the hover: "FPL Table 5-3a (<rows>)" (plan §6),
/// or for a Table 5-5a row its [`DensitySource::describe`] line and row.
fn density_basis_clause(source: DensitySource) -> String {
    match source {
        DensitySource::FplBasicRow(_) => {
            format!("{} ({})", source.describe(), source.row_names())
        }
        DensitySource::FplRow(_) | DensitySource::FplMean(_) | DensitySource::FplLibraryRow(_) => {
            format!("FPL Table 5-3a ({})", source.row_names())
        }
    }
}

/// A force line evaluated at one mean chip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForceAtPoint {
    /// The line.
    pub line: ForceLine,
    /// The mean chip `fz · (1 − cos ψ) / ψ` (mm).
    pub mean_chip_mm: f64,
    /// Where the mean chip sits against the printed range.
    pub chip_regime: ChipRegime,
}

impl ForceAtPoint {
    /// The extrapolation line of the card (plan §6). `None` when the chip
    /// is inside the printed range.
    #[must_use]
    pub fn extrapolation_text(&self) -> Option<String> {
        let (lo, hi) = self.line.chip_range_mm();
        let (side, h) = match self.chip_regime {
            ChipRegime::Measured => return None,
            ChipRegime::BelowMeasured { mean_chip_mm } => ("below", mean_chip_mm),
            ChipRegime::AboveMeasured { mean_chip_mm } => ("above", mean_chip_mm),
        };
        Some(format!(
            "Mean chip {h:.3} mm is {side} the measured {lo:.3}-{hi:.3} mm: the force line is \
             extended (stated extrapolation)"
        ))
    }
}

/// The mean uncut chip `h_m = fz · (1 − cos ψ) / ψ` (mm), for a feed per
/// tooth `fz_mm` and an engagement arc `immersion_rad`. `None` when an
/// input is not finite and positive.
#[must_use]
pub fn mean_chip_mm(fz_mm: f64, immersion_rad: f64) -> Option<f64> {
    let usable = |v: f64| v.is_finite() && v > 0.0;
    if !(usable(fz_mm) && usable(immersion_rad)) {
        return None;
    }
    let psi = immersion_rad.min(std::f64::consts::PI);
    Some(fz_mm * (1.0 - psi.cos()) / psi)
}

/// Why a material has no force line. Plan §3 holds the reason texts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ForceLineRefusal {
    /// Every plywood grade.
    Plywood,
    /// Particleboard.
    Particleboard,
    /// HDF.
    Hdf,
    /// Every plastic family, HDPE included.
    Plastic,
    /// Every aluminium alloy.
    Aluminum,
    /// Every foam density.
    Foam,
    /// Every fiberglass grade.
    Fiberglass,
    /// `Material::Custom`.
    Custom,
    /// A species with no printed density.
    NoDensity,
    /// A density outside [`CURTI_DENSITY_RANGE_KG_M3`].
    DensityOutOfRange {
        /// The density (kg/m³).
        rho_kg_m3: f64,
    },
}

impl ForceLineRefusal {
    /// The variant name, for the FM1 column `refused:<Variant>` and the MCP
    /// `refused` field.
    #[must_use]
    pub const fn variant_id(&self) -> &'static str {
        match self {
            Self::Plywood => "Plywood",
            Self::Particleboard => "Particleboard",
            Self::Hdf => "Hdf",
            Self::Plastic => "Plastic",
            Self::Aluminum => "Aluminum",
            Self::Foam => "Foam",
            Self::Fiberglass => "Fiberglass",
            Self::Custom => "Custom",
            Self::NoDensity => "NoDensity",
            Self::DensityOutOfRange { .. } => "DensityOutOfRange",
        }
    }

    /// The card headline (plan §3).
    #[must_use]
    pub fn headline(&self) -> String {
        match self {
            Self::Plywood => "No force line: plywood (ruling B6)".to_owned(),
            Self::Particleboard => "No force line: particleboard (ruling B6)".to_owned(),
            Self::Hdf => "No force line: HDF (no measurement)".to_owned(),
            Self::Plastic => "No force line: plastics (ruling B6)".to_owned(),
            Self::Aluminum => "No force line: aluminium (no per-edge line)".to_owned(),
            Self::Foam => "No force line: foam (no measurement)".to_owned(),
            Self::Fiberglass => "No force line: fiberglass (no measurement)".to_owned(),
            Self::Custom => "No force line: custom material (ruling B6)".to_owned(),
            Self::NoDensity => "No force line: no printed density for this species".to_owned(),
            Self::DensityOutOfRange { rho_kg_m3 } => {
                let (lo, hi) = CURTI_DENSITY_RANGE_KG_M3;
                format!("No force line: density {rho_kg_m3:.0} kg/m³ is outside {lo:.0}-{hi:.0}")
            }
        }
    }

    /// The hover detail (plan §3).
    #[must_use]
    pub const fn detail(&self) -> &'static str {
        match self {
            Self::Plywood => {
                "The only plywood measurement is a figure read for poplar plywood (Goli 2023). No \
                 source measures Baltic birch or softwood plywood, and the density law does not \
                 fit plywood (G7 T5)."
            }
            Self::Particleboard => {
                "The router-rig line is a figure read (Goli 2023). Pałubicki 2021 prints a total \
                 kc at 40-60 m/s, which is a different quantity."
            }
            Self::Hdf => "No fetched source measures HDF.",
            Self::Plastic => {
                "No source prints a cutting-force line for a plastic. The HDPE figure (Yang 2022) \
                 is a yield stress, not a cutting force."
            }
            Self::Aluminum => {
                "The only pair is a Kienzle kc1.1 800 / mc 0.25 from an aggregator (VDI 3323 \
                 group 22, grade A-secondary). It is a power law with no printed chip range, not \
                 an affine per-edge line."
            }
            Self::Foam => {
                "No fetched source measures foam. The old 1-3 N/mm² values had no source."
            }
            Self::Fiberglass => "No fetched source measures a glass-resin laminate.",
            Self::Custom => "A custom material carries no measured line. B6 takes no typed line.",
            Self::NoDensity => {
                "FPL Table 5-3a has no specific gravity for this species, and no fetched source \
                 prints its density. The Curti law needs a density."
            }
            Self::DensityOutOfRange { .. } => {
                "Curti 2021 fits five species from 287 to 1080 kg/m³ and states that the model \
                 must not extrapolate."
            }
        }
    }

    /// `(headline, detail)` for the card and its hover.
    #[must_use]
    pub fn card_text(&self) -> (String, String) {
        (self.headline(), self.detail().to_owned())
    }
}

/// `(card line, hover text)` for either branch of
/// [`crate::material::Material::force_line`]: the line's card and detail
/// text, or the refusal's headline and detail.
#[must_use]
pub fn card_lines(result: &Result<ForceLine, ForceLineRefusal>) -> (String, String) {
    match result {
        Ok(line) => (line.card_text(), line.detail_text()),
        Err(refusal) => refusal.card_text(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    const TEST_ROW: FplSgRow = FplSgRow::new("test row", 0.5);

    fn density(rho: f64) -> WoodDensity {
        WoodDensity::from_density_kg_m3(rho, DensitySource::FplRow(TEST_ROW))
    }

    /// The envelope from the printed quadratics, per term (plan §2.1).
    #[test]
    fn the_envelope_is_the_down_milling_vertex_per_term() {
        let (ks, int) = curti_helix0_envelope_terms();
        assert!((ks.value - 0.076_833_3).abs() < 1e-7, "Ks_n {}", ks.value);
        assert!(
            (int.value - 0.006_033_3).abs() < 1e-7,
            "Int_n {}",
            int.value
        );
        assert_eq!(ks.mode, MillingMode::Down);
        assert_eq!(int.mode, MillingMode::Down);
        assert!((ks.grain_angle_deg - 250.0 / 3.0).abs() < 1e-9);
        assert!((int.grain_angle_deg - 200.0 / 3.0).abs() < 1e-9);
        // The up-milling maxima of plan §2.1, so the "larger mode" step has
        // two real candidates.
        let (up_ks, up_ks_ga) = CURTI_UP_HELIX0_KS_N.max_over_grain();
        let (up_int, up_int_ga) = CURTI_UP_HELIX0_INT_N.max_over_grain();
        assert!((up_ks - 0.076).abs() < 1e-12 && (up_ks_ga - 100.0).abs() < 1e-9);
        assert!((up_int - 0.005_75).abs() < 1e-12 && (up_int_ga - 50.0).abs() < 1e-9);
    }

    /// The per-term envelope sits at most 0.5 % above the worst single
    /// (mode, grain angle) state at h 0.05 and 0.10 mm.
    #[test]
    fn the_envelope_costs_under_half_a_percent() {
        let (ks_n, int_n) = curti_helix0_envelope();
        for h in [0.05_f64, 0.10] {
            let envelope = ks_n * h + int_n;
            let mut worst = 0.0_f64;
            for (ks_q, int_q) in [
                (CURTI_UP_HELIX0_KS_N, CURTI_UP_HELIX0_INT_N),
                (CURTI_DOWN_HELIX0_KS_N, CURTI_DOWN_HELIX0_INT_N),
            ] {
                for step in 0..=1800_u32 {
                    let ga = f64::from(step) / 10.0;
                    worst = worst.max(ks_q.at(ga) * h + int_q.at(ga));
                }
            }
            let excess = envelope / worst - 1.0;
            assert!(
                (0.0..=0.005).contains(&excess),
                "h {h}: envelope {envelope} vs worst state {worst}"
            );
        }
    }

    /// `ρ = SG × 1000 × 1.12`.
    #[test]
    fn the_density_is_the_fpl_sg_at_twelve_percent() {
        let d = WoodDensity::fpl_row(FplSgRow::new("Pine, longleaf", 0.59));
        assert!((d.rho_kg_m3() - 660.8).abs() < 1e-9);
        let line = ForceLine::curti_density_law(d).unwrap();
        let (ks_n, int_n) = curti_helix0_envelope();
        assert!((line.ks_n_per_mm2() - ks_n * 660.8).abs() < 1e-9);
        assert!((line.f_edge_n_per_mm() - int_n * 660.8).abs() < 1e-9);
        assert!((line.ks_n_per_mm2() - 50.77).abs() < 0.01);
        assert!((line.f_edge_n_per_mm() - 3.987).abs() < 0.001);
        assert!((line.grain_factor() - 1.0).abs() < 1e-12);
    }

    /// The density range edges: the ends are inside.
    #[test]
    fn the_density_range_holds_its_ends() {
        assert!(ForceLine::curti_density_law(density(287.0)).is_ok());
        assert!(ForceLine::curti_density_law(density(1080.0)).is_ok());
        for rho in [286.9, 1080.1] {
            assert_eq!(
                ForceLine::curti_density_law(density(rho)),
                Err(ForceLineRefusal::DensityOutOfRange { rho_kg_m3: rho })
            );
        }
        for rho in [0.0, -1.0, f64::NAN] {
            assert_eq!(
                ForceLine::curti_density_law(density(rho)),
                Err(ForceLineRefusal::NoDensity)
            );
        }
    }

    /// The chip regime: the ends are inside.
    #[test]
    fn the_chip_regime_names_its_side() {
        let line = ForceLine::goli_2018_mdf();
        assert_eq!(line.chip_regime(0.041), ChipRegime::Measured);
        assert_eq!(line.chip_regime(0.091), ChipRegime::Measured);
        assert_eq!(
            line.chip_regime(0.03),
            ChipRegime::BelowMeasured { mean_chip_mm: 0.03 }
        );
        assert_eq!(
            line.chip_regime(0.12),
            ChipRegime::AboveMeasured { mean_chip_mm: 0.12 }
        );
        let text = line.at_mean_chip(0.03).extrapolation_text().unwrap();
        assert_eq!(
            text,
            "Mean chip 0.030 mm is below the measured 0.041-0.091 mm: the force line is \
             extended (stated extrapolation)"
        );
        assert!(line.at_mean_chip(0.05).extrapolation_text().is_none());
    }

    /// The mean chip at full slot is `2·fz/π`.
    #[test]
    fn the_mean_chip_of_a_slot() {
        let h = mean_chip_mm(0.1, std::f64::consts::PI).unwrap();
        assert!((h - 0.2 / std::f64::consts::PI).abs() < 1e-12);
        assert!(mean_chip_mm(0.0, 1.0).is_none());
        assert!(mean_chip_mm(0.1, 0.0).is_none());
    }

    /// The MDF texts of plan §6.
    #[test]
    fn the_mdf_texts_are_the_plan_strings() {
        let line = ForceLine::goli_2018_mdf();
        assert_eq!(line.card_text(), "Force line: Goli 2018 MDF, printed");
        assert_eq!(
            line.detail_text(),
            "Ks 31.44 N/mm², F_edge 3.36 N/mm per mm of edge, printed (Goli 2018 Table 3: \
             up-milling, straight blade, rake 25°, 711 kg/m³). Valid: mean chip 0.041-0.091 mm. \
             Grain factor 1.0: MDF is isotropic in plane. Source: g7_goli2018_round_shape_ks."
        );
        assert!(line.kc_eq(0.05).is_some_and(|k| (k - 98.64).abs() < 1e-9));
        assert!(line.kc_eq(0.0).is_none());
    }

    /// The solid-wood hover names the derived state of each term.
    #[test]
    fn the_curti_hover_names_the_state_of_each_term() {
        let line = ForceLine::curti_density_law(WoodDensity::fpl_row(FplSgRow::new(
            "Pine, longleaf",
            0.59,
        )))
        .unwrap();
        let text = line.detail_text();
        assert!(text.starts_with("Ks = 0.0768333 × ρ = 50.77 N/mm², F_edge = 0.0060333 × ρ"));
        assert!(text.contains("Ks_n from down-milling GA 83°, Int_n from down-milling GA 67°"));
        assert!(text.contains("ρ = SG 0.5900 × 1000 × 1.12 = 660.8 kg/m³"));
        assert!(text.contains("(Pine, longleaf)"));
        assert!(text.contains("Valid: ρ 287-1080 kg/m³, mean chip 0.04-0.10 mm"));
        assert_eq!(
            line.card_text(),
            "Force line: Curti 2021 density law, ρ 661 kg/m³ (FPL SG 0.59), upper envelope"
        );
    }

    /// Eq. (4-11) reproduces FPL's worked example (Ch.4, after Eq.
    /// (4-11)): "The average basic specific gravity Gb for this species is
    /// 0.55 ... G12 = 0.605" (white ash, read from Figure 4-6), and "678 kg
    /// m–3" at 12 % MC. The equation gives 0.6027 and 675.0 kg/m³.
    #[test]
    fn eq_4_11_reproduces_the_fpl_white_ash_example() {
        let g12 = fpl_eq_4_11_g12(0.55);
        assert!((g12 / 0.605 - 1.0).abs() < 0.005, "G12 {g12}");
        let d = WoodDensity::fpl_basic_row(FplSgRow::new("Ash, white", 0.55));
        assert!(
            (d.rho_kg_m3() / 678.0 - 1.0).abs() < 0.005,
            "ρ {}",
            d.rho_kg_m3()
        );
        assert_eq!(d.basic_specific_gravity(), Some(0.55));
        assert!((d.specific_gravity() - g12).abs() < 1e-12);
    }

    /// The Table 5-5a rows of the fetch: radiata 504.1, jarrah 839.9 and
    /// ipe 1207.0 kg/m³; ipe is above the Curti range.
    #[test]
    fn the_basic_rows_convert_and_ipe_is_out_of_range() {
        let rho = |gb| WoodDensity::fpl_basic_row(FplSgRow::new("row", gb)).rho_kg_m3();
        assert!((rho(0.42) - 504.1).abs() < 0.1, "radiata {}", rho(0.42));
        assert!((rho(0.67) - 839.9).abs() < 0.1, "jarrah {}", rho(0.67));
        assert!((rho(0.92) - 1207.0).abs() < 0.1, "ipe {}", rho(0.92));
        let ipe = WoodDensity::fpl_basic_row(FplSgRow::new("Ipe", 0.92));
        match ForceLine::curti_density_law(ipe) {
            Err(ForceLineRefusal::DensityOutOfRange { rho_kg_m3 }) => {
                assert!((rho_kg_m3 - 1207.0).abs() < 0.1, "ipe ρ {rho_kg_m3}");
            }
            other => panic!("ipe must refuse DensityOutOfRange, got {other:?}"),
        }
        let radiata = WoodDensity::fpl_basic_row(FplSgRow::new("Pine, radiata", 0.42));
        assert_eq!(
            radiata.source().describe(),
            "FPL Table 5-5a basic SG 0.42 (ovendry weight, green volume), converted to 12 % MC \
             by FPL Ch.4 Eq. (4-11) with MCfs 30 %: G12 0.4501"
        );
        assert_eq!(
            radiata.source().source_ids(),
            &[FPL_TABLE_5_5A_SOURCE_ID, FPL_CH4_SG_CONVERSION_SOURCE_ID]
        );
    }

    #[test]
    fn the_out_of_range_headline_prints_the_density() {
        let r = ForceLineRefusal::DensityOutOfRange { rho_kg_m3: 1200.4 };
        assert_eq!(
            r.headline(),
            "No force line: density 1200 kg/m³ is outside 287-1080"
        );
        assert_eq!(r.variant_id(), "DensityOutOfRange");
    }
}
