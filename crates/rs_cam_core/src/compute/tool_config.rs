use serde::{Deserialize, Serialize};

pub use crate::tool::size_units::SizeUnits;
use crate::tool::size_units::{self, MM_PER_INCH};
pub use crate::tool::{ToolDimension, ToolGeometryError};

/// CONTRACT K3 (feeds contract P6): the tool-fact type lives in `tool_fact.rs`.
pub use super::tool_fact::{
    CentreCutting, FACT_FORMS, FACT_KEYS, FactKind, NEW_DEFAULT_BASIS, ToolFact, bare_fact_keys,
    bare_facts_text, centre_cutting_serde,
};
use crate::material::FormulaConstant;

/// The array of every variant of a unit enum, with a compile-time proof that
/// the list is complete (F162). Write `all_variants!(Type: A, B, C)`; the
/// macro gives `[Type::A, Type::B, Type::C]`. It also holds an exhaustive
/// `match` over the same names, so a new variant that is not in the list is
/// a compile error (`E0004`) at the `ALL` site. Name the type, not `Self`:
/// the proof is a nested item, and `Self` does not reach into it.
///
/// Example: `pub const ALL: &[ToolType] = &all_variants!(ToolType: EndMill, ..);`
#[macro_export]
macro_rules! all_variants {
    ($ty:ident: $first:ident $(, $rest:ident)* $(,)?) => {{
        const _: () = match $ty::$first {
            $ty::$first => (),
            $($ty::$rest => (),)*
        };
        [$ty::$first $(, $ty::$rest)*]
    }};
}

/// F004: the tool id lives in `crate::ids` beside the other id types.
pub use crate::ids::ToolId;

/// Tool type matching the five cutter types in rs_cam_core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolType {
    EndMill,
    BallNose,
    BullNose,
    VBit,
    TaperedBallNose,
}

impl ToolType {
    pub const ALL: &[ToolType] = &crate::all_variants!(ToolType:
        EndMill, BallNose, BullNose, VBit, TaperedBallNose,
    );

    pub fn label(&self) -> &'static str {
        match self {
            ToolType::EndMill => "End Mill",
            ToolType::BallNose => "Ball Nose",
            ToolType::BullNose => "Bull Nose",
            ToolType::VBit => "V-Bit",
            ToolType::TaperedBallNose => "Tapered Ball Nose",
        }
    }

    pub fn has_ball_tip(&self) -> bool {
        matches!(self, ToolType::BallNose | ToolType::TaperedBallNose)
    }

    /// Convenience `ToolType → CutterKind` classification for call
    /// sites that only hold a `ToolConfig`. The PRIMARY derivation is
    /// [`crate::feeds::ToolGeometryHint::cutter_kind`] (every
    /// `MillingCutter` yields a hint); this layered shortcut must stay
    /// the bijective inverse of [`crate::feeds::CutterKind::tool_type`]
    /// (pinned by `tool_type_cutter_kind_round_trips`).
    pub const fn cutter_kind(self) -> crate::feeds::CutterKind {
        match self {
            ToolType::EndMill => crate::feeds::CutterKind::Flat,
            ToolType::BallNose => crate::feeds::CutterKind::Ball,
            ToolType::BullNose => crate::feeds::CutterKind::Bull,
            ToolType::VBit => crate::feeds::CutterKind::VBit,
            ToolType::TaperedBallNose => crate::feeds::CutterKind::TaperedBall,
        }
    }

    /// The snake_case serde repr of this tool type as a `&'static str`,
    /// for const contexts (registry tool-constraint schemas). Must match
    /// the serde rename — pinned by `tool_type_serde_repr_pinned`.
    pub const fn serde_token(self) -> &'static str {
        match self {
            ToolType::EndMill => "end_mill",
            ToolType::BallNose => "ball_nose",
            ToolType::BullNose => "bull_nose",
            ToolType::VBit => "v_bit",
            ToolType::TaperedBallNose => "tapered_ball_nose",
        }
    }

    /// THE tool-type string parser (Phase 3 T8, decision Q4).
    ///
    /// One vocabulary for every surface: the canonical serde token of
    /// each type, matched case-insensitively. T8 unified four divergent
    /// alias tables here; L9 then deleted the eight aliases those
    /// tables had carried (`endmill`, `flat`, `ballnose`, `ball`,
    /// `bullnose`, `vbit`, `taperedballnose`, `tapered_ball`). Each one
    /// named a loader this build no longer has.
    ///
    /// "Lenient" now means the case fold and the `Option` return, not a
    /// second spelling.
    ///
    /// Returns `None` for unrecognized input. Policy at the call sites
    /// (Q4): file-loading surfaces warn and default to `EndMill`; the
    /// MCP mutation surface returns an explicit error (an interactive
    /// caller should hear "unknown type", not get a surprise end mill).
    pub fn parse_lenient(s: &str) -> Option<ToolType> {
        let token = s.to_ascii_lowercase();
        ToolType::ALL
            .iter()
            .copied()
            .find(|t| t.serde_token() == token)
    }
}

/// Tool material (affects chip load and wear).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolMaterial {
    Carbide,
    Hss,
}

/// Young's modulus of tungsten carbide (N/mm² == MPa). Cited range
/// 550–650 GPa across grades; 600 is the canonical handbook value.
pub(crate) const CARBIDE_YOUNGS_MODULUS_N_PER_MM2: f64 = 600_000.0;
/// Young's modulus of high-speed steel (N/mm² == MPa). Handbook value
/// is 200–210 GPa for M2/M42 grades; 200 chosen as a clean reference.
pub(crate) const HSS_YOUNGS_MODULUS_N_PER_MM2: f64 = 200_000.0;

impl ToolMaterial {
    pub const ALL: &[ToolMaterial] = &crate::all_variants!(ToolMaterial: Carbide, Hss);

    pub fn label(&self) -> &'static str {
        match self {
            ToolMaterial::Carbide => "Carbide",
            ToolMaterial::Hss => "HSS",
        }
    }

    /// Young's modulus E in N/mm² (= MPa). Used by the deflection gate
    /// to translate cutting force into tip displacement.
    pub fn youngs_modulus_n_per_mm2(&self) -> f64 {
        match self {
            ToolMaterial::Carbide => CARBIDE_YOUNGS_MODULUS_N_PER_MM2,
            ToolMaterial::Hss => HSS_YOUNGS_MODULUS_N_PER_MM2,
        }
    }
}

/// The flute form of the tool: which way the flutes carry the chip (it
/// affects chip evacuation and surface quality), or none.
///
/// Feeds contract P24a: `Straight` is a straight-flute bit (a carbide
/// tipped straight router bit). Its edge is parallel to the tool axis, so
/// its helix angle is 0: [`ToolConfig::hold_flute_form`] keeps the pair.
/// The value also declares the vendor series of the tool
/// (`feeds::series::ToolSeries::for_cut_direction`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BitCutDirection {
    UpCut,
    DownCut,
    Compression,
    /// A straight flute: no helix (P24a).
    Straight,
}

impl BitCutDirection {
    pub const ALL: &[BitCutDirection] = &crate::all_variants!(BitCutDirection:
        UpCut, DownCut, Compression, Straight,
    );

    pub fn label(&self) -> &'static str {
        match self {
            BitCutDirection::UpCut => "Up Cut",
            BitCutDirection::DownCut => "Down Cut",
            BitCutDirection::Compression => "Compression",
            BitCutDirection::Straight => "Straight",
        }
    }

    /// The file token of the value (`up_cut`, `down_cut`, `compression`,
    /// `straight`): the serde spelling, which the project file, the tool
    /// library, MCP and the CLI job file read.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            BitCutDirection::UpCut => "up_cut",
            BitCutDirection::DownCut => "down_cut",
            BitCutDirection::Compression => "compression",
            BitCutDirection::Straight => "straight",
        }
    }
}

/// One field of a [`ToolConfig`] that `set_tool_param` writes (F118).
///
/// The token of each variant is the [`ToolConfig`] field name. The MCP
/// wire and the CLI `--tool-set` key name a field by this token. The
/// fields that no variant names have their own doors: the identity
/// (`id`, `name`, `tool_number`, `vendor`, `product_id`) and the type
/// (`tool_type`) change through `ReplaceTool` or `add_tool`. The sentry
/// `every_stock_and_tool_field_is_reachable_f118` holds that list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolParamKey {
    Diameter,
    CuttingLength,
    HelixDeg,
    CornerRadiusMm,
    CornerRadius,
    IncludedAngle,
    TaperHalfAngle,
    ShaftDiameter,
    ShankDiameter,
    Stickout,
    FluteCount,
    ToolMaterial,
    CutDirection,
    SizeUnits,
    CentreCutting,
    /// D12: the flat tip of a V-bit (`v_tip_diameter`).
    VTipDiameter,
}

impl ToolParamKey {
    pub const ALL: &[ToolParamKey] = &crate::all_variants!(ToolParamKey:
        Diameter, CuttingLength, HelixDeg, CornerRadiusMm, CornerRadius,
        IncludedAngle, TaperHalfAngle, ShaftDiameter, ShankDiameter, Stickout,
        FluteCount, ToolMaterial, CutDirection, SizeUnits, CentreCutting, VTipDiameter,
    );

    /// The snake_case token: the serde form and the [`ToolConfig`] field
    /// name. The test `tool_param_key_tokens_are_the_serde_form` holds the
    /// two equal.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Diameter => "diameter",
            Self::CuttingLength => "cutting_length",
            Self::HelixDeg => "helix_deg",
            Self::CornerRadiusMm => "corner_radius_mm",
            Self::CornerRadius => "corner_radius",
            Self::IncludedAngle => "included_angle",
            Self::TaperHalfAngle => "taper_half_angle",
            Self::ShaftDiameter => "shaft_diameter",
            Self::ShankDiameter => "shank_diameter",
            Self::Stickout => "stickout",
            Self::FluteCount => "flute_count",
            Self::ToolMaterial => "tool_material",
            Self::CutDirection => "cut_direction",
            Self::SizeUnits => "size_units",
            Self::CentreCutting => "centre_cutting",
            Self::VTipDiameter => "v_tip_diameter",
        }
    }

    /// The key whose token is `token`, exactly. `None` for any other text.
    pub fn parse(token: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.token() == token)
    }

    /// Every token, comma-separated, for a refusal that lists them.
    pub fn tokens_joined() -> String {
        Self::ALL
            .iter()
            .map(|k| k.token())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Whether a write of this key changes what the tool cuts. `false`
    /// only for `size_units`: it changes how the size is SHOWN and no
    /// stored length, so it drops no result.
    pub const fn changes_the_cut(self) -> bool {
        !matches!(self, Self::SizeUnits)
    }

    /// Write `value` into the field of `tool` that this key names.
    ///
    /// K3 (feeds contract P6): a tool fact written here is
    /// `ToolFact::Stated` (an operator edit). `centre_cutting` takes
    /// `"yes"`, an object with `centre_core_diameter_mm` and
    /// `blade_protrusion_mm` (the tool does not cut at its centre), or `null`
    /// (not known). `v_tip_diameter` takes a number (mm) or `null` (not
    /// known).
    ///
    /// The write checks only the JSON type of `value`. The caller runs
    /// [`ToolConfig::validate`] on the result (F029), so a value outside
    /// its domain refuses there.
    ///
    /// # Errors
    ///
    /// A message that names the key and the type it needs.
    pub fn write(self, tool: &mut ToolConfig, value: &serde_json::Value) -> Result<(), String> {
        let number = |field: &mut f64| -> Result<(), String> {
            *field = value
                .as_f64()
                .ok_or_else(|| format!("{} must be a number (got {value})", self.token()))?;
            Ok(())
        };
        let stated = || -> Result<ToolFact<f64>, String> {
            value
                .as_f64()
                .map(ToolFact::Stated)
                .ok_or_else(|| format!("{} must be a number (got {value})", self.token()))
        };
        match self {
            Self::Diameter => number(&mut tool.diameter),
            Self::CuttingLength => {
                tool.set_cutting_length(stated()?);
                Ok(())
            }
            Self::HelixDeg => number(&mut tool.helix_deg),
            Self::CornerRadiusMm => number(&mut tool.corner_radius_mm),
            Self::CornerRadius => number(&mut tool.corner_radius),
            Self::IncludedAngle => number(&mut tool.included_angle),
            Self::TaperHalfAngle => {
                tool.set_taper_half_angle(stated()?);
                Ok(())
            }
            Self::ShaftDiameter => number(&mut tool.shaft_diameter),
            Self::ShankDiameter => {
                tool.set_shank_diameter(stated()?);
                Ok(())
            }
            Self::Stickout => {
                tool.set_stickout(stated()?);
                Ok(())
            }
            Self::FluteCount => {
                let count = value
                    .as_u64()
                    .and_then(|v| u32::try_from(v).ok())
                    .ok_or_else(|| format!("flute_count must be a whole number (got {value})"))?;
                tool.set_flute_count(ToolFact::Stated(count));
                Ok(())
            }
            Self::ToolMaterial => {
                tool.tool_material = serde_json::from_value(value.clone()).map_err(|e| {
                    format!("tool_material must be \"carbide\" or \"hss\" (got {value}: {e})")
                })?;
                Ok(())
            }
            Self::CutDirection => {
                tool.cut_direction = serde_json::from_value(value.clone()).map_err(|e| {
                    format!(
                        "cut_direction must be \"up_cut\", \"down_cut\", \"compression\" or \
                         \"straight\" (got {value}: {e})"
                    )
                })?;
                // P24a: a straight flute has no helix.
                let _ = tool.hold_flute_form();
                Ok(())
            }
            Self::SizeUnits => {
                tool.size_units = Some(serde_json::from_value(value.clone()).map_err(|e| {
                    format!("size_units must be \"metric\" or \"imperial\" (got {value}: {e})")
                })?);
                Ok(())
            }
            Self::CentreCutting => {
                let fact = if value.is_null() {
                    ToolFact::Unknown
                } else if value.as_str() == Some("yes") {
                    ToolFact::Stated(CentreCutting::Yes)
                } else {
                    let geometry: NonCentreCuttingGeometry = serde_json::from_value(value.clone())
                        .map_err(|e| {
                            format!(
                                "centre_cutting must be \"yes\", an object with \
                                 centre_core_diameter_mm and blade_protrusion_mm (the tool does \
                                 not cut at its centre), or null (not known) (got {value}: {e})"
                            )
                        })?;
                    ToolFact::Stated(CentreCutting::No(geometry))
                };
                tool.set_centre_cutting(fact);
                Ok(())
            }
            Self::VTipDiameter => {
                let fact = if value.is_null() {
                    ToolFact::Unknown
                } else {
                    stated()?
                };
                tool.set_v_tip_diameter(fact);
                Ok(())
            }
        }
    }
}

/// Measurements of a tool that does not cut at its centre.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NonCentreCuttingGeometry {
    pub centre_core_diameter_mm: f64,
    pub blade_protrusion_mm: f64,
}

/// G-CENTRECUT: an entry builder could not keep a non-centre-cutting tool
/// inside its limits. It had to plunge straight into stock deeper than the
/// blade protrusion. K6 (ruling Q6): a warning, not a refusal; the job
/// records it (`ToolpathStats::non_centre_cutting_plunge`,
/// `safety.non_centre_cutting_plunge`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NonCentreCuttingRefusal {
    /// The plunge target, in the emission frame.
    pub at: crate::geo::P3,
    /// How far the plunge goes into stock (mm).
    pub depth_mm: f64,
    /// The blade protrusion of the tool (mm).
    pub max_plunge_depth_mm: f64,
}

impl std::fmt::Display for NonCentreCuttingRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the tool does not cut at its centre, and an entry at X{:.3} Y{:.3} Z{:.3} \
             must plunge {:.2} mm straight into stock (the blades stand {:.2} mm below \
             the centre). Use a ramp or helix entry that has room, or a centre-cutting tool",
            self.at.x, self.at.y, self.at.z, self.depth_mm, self.max_plunge_depth_mm
        )
    }
}

/// Why a helix entry fell back (feeds contract P22b, lead ruling on the
/// fallback text): one finding id (`HelixCore`), two reasons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelixFallbackReason {
    /// The entry centre has no room for any helix: the circle of the
    /// smallest helix leaves the region the entry must stay inside (a fit of
    /// 0, `dressup::HELIX_MIN_FIT_MM`).
    NoRoom,
    /// The region holds a helix, but its radius is under the core radius of
    /// a tool that does not cut at its centre (a smaller helix leaves a
    /// post under the core).
    BelowCoreRadius,
}

impl HelixFallbackReason {
    /// The finding text of the reason.
    #[must_use]
    pub const fn text(self) -> &'static str {
        match self {
            Self::NoRoom => "no room for the helix",
            Self::BelowCoreRadius => "helix radius below the core radius",
        }
    }
}

impl HelixCoreFallback {
    /// Why this entry fell back: no room at all, or a fit under the core
    /// floor (the fit decides, by the dressup's own room test).
    #[must_use]
    pub fn reason(&self) -> HelixFallbackReason {
        if self.fit_mm > crate::dressup::HELIX_MIN_FIT_MM {
            HelixFallbackReason::BelowCoreRadius
        } else {
            HelixFallbackReason::NoRoom
        }
    }
}

/// The entry that a helix fell back to (G-SIMCORE, `P22_DESIGN.md` section
/// 9: helix, then ramp, then plunge).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryFallback {
    /// The ramp, at the limit angle of the tool (G-CENTRECUT).
    Ramp,
    /// The ramp had no room either: a straight plunge
    /// ([`NonCentreCuttingRefusal`] names it too).
    Plunge,
    /// P22-0c-4: a ramp-finish ring inside the core diameter. The core
    /// cannot descend on it, so no pass is cut and the ring is left uncut.
    NoPass,
}

impl EntryFallback {
    /// The word the finding prints.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ramp => "ramp",
            Self::Plunge => "plunge",
            Self::NoPass => "no pass",
        }
    }
}

/// G-SIMCORE (P22-0b): a helix entry of a tool that does not cut at its
/// centre where the region had no room for the core floor
/// (`HelixRadius::resolve`): a smaller helix leaves a post under the core,
/// so the entry fell back. A warning (`safety.helix_inside_core`, K5 rule
/// `HelixCore`); never a refusal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HelixCoreFallback {
    /// The entry target, in the emission frame.
    pub at: crate::geo::P3,
    /// The largest helix radius the region holds at the entry (mm).
    pub fit_mm: f64,
    /// The core floor of the tool (mm): half the core diameter.
    pub core_floor_mm: f64,
    /// The entry that the toolpath ships instead.
    pub fallback: EntryFallback,
}

/// The tolerance (mm) of the non-centre-cutting checks: float noise on a
/// planned depth and on a dexel height read.
pub const NON_CENTRE_CUTTING_TOL_MM: f64 = 0.02;

// ── K3 legacy loader defaults (feeds contract P6, decision E4) ─────────
//
// The `default_*` functions of `session/project_file.rs` held these numbers
// before K3 (moved by copy in commit B). An `Unknown` fact plans at its
// default (`<fact>_planning_..()`), and the lineage marks it with a
// `tool.fact_unknown` warning. No file in the repo prints them: each is an
// F201 row marked Unsourced (`feeds::formula_constants`).

const LEGACY_DEFAULT_CLAIM: &str = "the project-loader default before K3 (feeds contract P6)";

/// The legacy loader default of the flute (cutting) length, mm.
pub const LEGACY_DEFAULT_CUTTING_LENGTH_MM: f64 = 25.0;

/// F201: the legacy loader default of the flute (cutting) length, mm ([`LEGACY_DEFAULT_CUTTING_LENGTH_MM`]).
pub const LEGACY_DEFAULT_CUTTING_LENGTH_MM_ROW: FormulaConstant = FormulaConstant::unsourced(
    "compute.tool_config.LEGACY_DEFAULT_CUTTING_LENGTH_MM",
    LEGACY_DEFAULT_CUTTING_LENGTH_MM,
    LEGACY_DEFAULT_CLAIM,
);

/// The legacy loader default of the taper half-angle, degrees.
pub const LEGACY_DEFAULT_TAPER_HALF_ANGLE_DEG: f64 = 15.0;

/// F201: the legacy loader default of the taper half-angle, degrees ([`LEGACY_DEFAULT_TAPER_HALF_ANGLE_DEG`]).
pub const LEGACY_DEFAULT_TAPER_HALF_ANGLE_DEG_ROW: FormulaConstant = FormulaConstant::unsourced(
    "compute.tool_config.LEGACY_DEFAULT_TAPER_HALF_ANGLE_DEG",
    LEGACY_DEFAULT_TAPER_HALF_ANGLE_DEG,
    LEGACY_DEFAULT_CLAIM,
);

/// The legacy loader default of the shank diameter, mm.
pub const LEGACY_DEFAULT_SHANK_DIAMETER_MM: f64 = 6.35;

/// F201: the legacy loader default of the shank diameter, mm ([`LEGACY_DEFAULT_SHANK_DIAMETER_MM`]).
pub const LEGACY_DEFAULT_SHANK_DIAMETER_MM_ROW: FormulaConstant = FormulaConstant::unsourced(
    "compute.tool_config.LEGACY_DEFAULT_SHANK_DIAMETER_MM",
    LEGACY_DEFAULT_SHANK_DIAMETER_MM,
    LEGACY_DEFAULT_CLAIM,
);

/// The legacy loader default of the stick-out, mm.
pub const LEGACY_DEFAULT_STICKOUT_MM: f64 = 45.0;

/// F201: the legacy loader default of the stick-out, mm ([`LEGACY_DEFAULT_STICKOUT_MM`]).
pub const LEGACY_DEFAULT_STICKOUT_MM_ROW: FormulaConstant = FormulaConstant::unsourced(
    "compute.tool_config.LEGACY_DEFAULT_STICKOUT_MM",
    LEGACY_DEFAULT_STICKOUT_MM,
    LEGACY_DEFAULT_CLAIM,
);

/// The legacy loader default of the flute count. A count, so it is a `u32`;
/// [`LEGACY_DEFAULT_FLUTE_COUNT_ROW`] holds the same number for F201 (a unit test holds the two equal).
pub const LEGACY_DEFAULT_FLUTE_COUNT: u32 = 2;

/// F201: the legacy loader default of the flute count.
pub const LEGACY_DEFAULT_FLUTE_COUNT_ROW: FormulaConstant = FormulaConstant::unsourced(
    "compute.tool_config.LEGACY_DEFAULT_FLUTE_COUNT",
    LEGACY_DEFAULT_FLUTE_COUNT as f64,
    LEGACY_DEFAULT_CLAIM,
);

/// The five F201 rows of the legacy loader defaults.
pub const LEGACY_DEFAULT_ROWS: [FormulaConstant; 5] = [
    LEGACY_DEFAULT_CUTTING_LENGTH_MM_ROW,
    LEGACY_DEFAULT_TAPER_HALF_ANGLE_DEG_ROW,
    LEGACY_DEFAULT_SHANK_DIAMETER_MM_ROW,
    LEGACY_DEFAULT_STICKOUT_MM_ROW,
    LEGACY_DEFAULT_FLUTE_COUNT_ROW,
];

/// Complete tool configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolConfig {
    pub id: ToolId,
    pub name: String,
    /// G-code tool number used for M6 output.
    pub tool_number: u32,
    pub tool_type: ToolType,
    pub diameter: f64,
    /// K3: the flute (cutting) length, mm.
    #[serde(
        default = "ToolFact::unknown",
        skip_serializing_if = "ToolFact::is_unknown"
    )]
    cutting_length: ToolFact<f64>,
    #[serde(default = "default_helix_deg")]
    pub helix_deg: f64,
    #[serde(default)]
    pub corner_radius_mm: f64,
    // Bull Nose
    pub corner_radius: f64,
    // V-Bit (included angle in degrees)
    pub included_angle: f64,
    /// K3: the V-bit tip flat diameter, mm. P6 adds the fact (Unknown by
    /// default); P7 (VTipFlat) reads it.
    #[serde(
        default = "ToolFact::unknown",
        skip_serializing_if = "ToolFact::is_unknown"
    )]
    v_tip_diameter: ToolFact<f64>,
    // Tapered Ball Nose (half-angle in degrees)
    /// K3: the taper half-angle, degrees.
    #[serde(
        default = "ToolFact::unknown",
        skip_serializing_if = "ToolFact::is_unknown"
    )]
    taper_half_angle: ToolFact<f64>,
    pub shaft_diameter: f64,
    // Shank / collision detection (H1: the holder is the machine head)
    /// K3: the shank diameter, mm.
    #[serde(
        default = "ToolFact::unknown",
        skip_serializing_if = "ToolFact::is_unknown"
    )]
    shank_diameter: ToolFact<f64>,
    /// K3: the stick-out from the holder face, mm.
    #[serde(
        default = "ToolFact::unknown",
        skip_serializing_if = "ToolFact::is_unknown"
    )]
    stickout: ToolFact<f64>,
    // Cutting parameters (for feeds calculation)
    /// K3: the flute count.
    #[serde(
        default = "ToolFact::unknown",
        skip_serializing_if = "ToolFact::is_unknown"
    )]
    flute_count: ToolFact<u32>,
    pub tool_material: ToolMaterial,
    pub cut_direction: BitCutDirection,
    // Optional vendor info
    pub vendor: String,
    pub product_id: String,
    /// The unit a person reads and types this tool's size in. `None`
    /// means "infer from the name" ([`Self::effective_size_units`]). A
    /// display and entry convention only: every length above is in mm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_units: Option<SizeUnits>,
    /// K3: does the tool cut at its centre? `No` holds the measured centre
    /// core and blade protrusion. `Unknown` plans as a centre-cutting tool
    /// (the motion before K3); ruling D12 makes it a required input that
    /// stops generation (P7 commit A2).
    #[serde(
        default = "ToolFact::unknown",
        skip_serializing_if = "ToolFact::is_unknown",
        with = "super::tool_fact::centre_cutting_serde"
    )]
    centre_cutting: ToolFact<CentreCutting>,
}

/// Every field of a [`ToolConfig`], for a loader (review note 1 of P6
/// commit B). [`ToolConfig::from_record`] builds the tool with a struct
/// literal and the loader writes a `ToolRecord` literal, both with no `..`,
/// so a new field does not compile until each loader states it.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolRecord {
    pub id: ToolId,
    pub name: String,
    pub tool_number: u32,
    pub tool_type: ToolType,
    pub diameter: f64,
    pub cutting_length: ToolFact<f64>,
    pub helix_deg: f64,
    pub corner_radius_mm: f64,
    pub corner_radius: f64,
    pub included_angle: f64,
    pub v_tip_diameter: ToolFact<f64>,
    pub taper_half_angle: ToolFact<f64>,
    pub shaft_diameter: f64,
    pub shank_diameter: ToolFact<f64>,
    pub stickout: ToolFact<f64>,
    pub flute_count: ToolFact<u32>,
    pub tool_material: ToolMaterial,
    pub cut_direction: BitCutDirection,
    pub vendor: String,
    pub product_id: String,
    pub size_units: Option<SizeUnits>,
    pub centre_cutting: ToolFact<CentreCutting>,
}

impl ToolConfig {
    /// The tool of a loader's record: every field as the record states it.
    #[must_use]
    pub fn from_record(record: ToolRecord) -> Self {
        let ToolRecord {
            id,
            name,
            tool_number,
            tool_type,
            diameter,
            cutting_length,
            helix_deg,
            corner_radius_mm,
            corner_radius,
            included_angle,
            v_tip_diameter,
            taper_half_angle,
            shaft_diameter,
            shank_diameter,
            stickout,
            flute_count,
            tool_material,
            cut_direction,
            vendor,
            product_id,
            size_units,
            centre_cutting,
        } = record;
        Self {
            id,
            name,
            tool_number,
            tool_type,
            diameter,
            cutting_length,
            helix_deg,
            corner_radius_mm,
            corner_radius,
            included_angle,
            v_tip_diameter,
            taper_half_angle,
            shaft_diameter,
            shank_diameter,
            stickout,
            flute_count,
            tool_material,
            cut_direction,
            vendor,
            product_id,
            size_units,
            centre_cutting,
        }
    }
}

fn default_helix_deg() -> f64 {
    crate::tool::DEFAULT_HELIX_DEG
}

impl ToolConfig {
    pub fn new_default(id: ToolId, tool_type: ToolType) -> Self {
        let (name, diameter) = match tool_type {
            ToolType::EndMill => ("End Mill".to_owned(), 6.35),
            ToolType::BallNose => ("Ball Nose".to_owned(), 6.35),
            ToolType::BullNose => ("Bull Nose".to_owned(), 12.7),
            ToolType::VBit => ("V-Bit".to_owned(), 12.7),
            ToolType::TaperedBallNose => ("Tapered Ball Nose".to_owned(), 3.175),
        };
        let mut tool = Self {
            id,
            name,
            tool_number: id.raw() as u32 + 1,
            tool_type,
            diameter,
            cutting_length: ToolFact::estimated(25.0, NEW_DEFAULT_BASIS),
            helix_deg: default_helix_deg(),
            corner_radius_mm: 0.0,
            corner_radius: 2.0,
            included_angle: 90.0,
            v_tip_diameter: ToolFact::Unknown,
            taper_half_angle: ToolFact::estimated(15.0, NEW_DEFAULT_BASIS),
            shaft_diameter: 6.35,
            shank_diameter: ToolFact::estimated(6.35, NEW_DEFAULT_BASIS),
            stickout: ToolFact::estimated(45.0, NEW_DEFAULT_BASIS),
            flute_count: ToolFact::estimated(2, NEW_DEFAULT_BASIS),
            tool_material: ToolMaterial::Carbide,
            cut_direction: BitCutDirection::UpCut,
            vendor: String::new(),
            product_id: String::new(),
            size_units: None,
            centre_cutting: ToolFact::Unknown,
        };
        // The block above is the canonical geometry of each type, but
        // it was written type-AGNOSTICALLY: every tool got corner
        // radius 2.0, included angle 90 and taper half-angle 15
        // whatever its type, and those numbers then survived save,
        // load, and every report surface that does not dispatch on
        // `tool_type` first. Keep only what this type actually
        // defines — 4b1e6f21 made the MCP creation path do the same,
        // and this is the constructor the GUI and CLI use.
        tool.normalize_geometry();
        tool
    }

    // ── K3 readers (feeds contract P6, decision E2) ────────────────────
    //
    // Two readers per fact. `<fact>_fact()` gives the fact with its kind
    // (the lineage, the rules). `<fact>_planning_..()` gives the value that
    // planning uses: the stated, catalogue or estimated value, else the
    // legacy loader default (`LEGACY_DEFAULT_*`, the ONE default per fact).
    // The lineage records the kind and warns on each `Unknown` fact
    // (`tool.fact_unknown`), so a planning reader does not carry it.

    /// The stick-out (mm) that planning uses.
    pub fn stickout_planning_mm(&self) -> f64 {
        self.stickout.value_or(LEGACY_DEFAULT_STICKOUT_MM)
    }

    /// The flute (cutting) length (mm) that planning uses.
    pub fn cutting_length_planning_mm(&self) -> f64 {
        self.cutting_length
            .value_or(LEGACY_DEFAULT_CUTTING_LENGTH_MM)
    }

    /// The shank diameter (mm) that planning uses.
    pub fn shank_diameter_planning_mm(&self) -> f64 {
        self.shank_diameter
            .value_or(LEGACY_DEFAULT_SHANK_DIAMETER_MM)
    }

    /// The taper half-angle (degrees) that planning uses.
    pub fn taper_half_angle_planning_deg(&self) -> f64 {
        self.taper_half_angle
            .value_or(LEGACY_DEFAULT_TAPER_HALF_ANGLE_DEG)
    }

    /// The flute count that planning uses.
    pub fn flute_count_planning(&self) -> u32 {
        self.flute_count.value_or(LEGACY_DEFAULT_FLUTE_COUNT)
    }

    /// The stick-out fact.
    pub fn stickout_fact(&self) -> &ToolFact<f64> {
        &self.stickout
    }

    /// The flute (cutting) length fact.
    pub fn cutting_length_fact(&self) -> &ToolFact<f64> {
        &self.cutting_length
    }

    /// The shank diameter fact.
    pub fn shank_diameter_fact(&self) -> &ToolFact<f64> {
        &self.shank_diameter
    }

    /// The taper half-angle fact.
    pub fn taper_half_angle_fact(&self) -> &ToolFact<f64> {
        &self.taper_half_angle
    }

    /// The flute count fact.
    pub fn flute_count_fact(&self) -> &ToolFact<u32> {
        &self.flute_count
    }

    /// The centre-cutting fact.
    pub fn centre_cutting_fact(&self) -> &ToolFact<CentreCutting> {
        &self.centre_cutting
    }

    /// The V-bit tip flat diameter fact (P7 reads it).
    pub fn v_tip_diameter_fact(&self) -> &ToolFact<f64> {
        &self.v_tip_diameter
    }

    /// K5 VTipFlat (feeds contract P7): the flat tip diameter (mm) that the
    /// cutter model uses. It is the tip that `planning()` gives (stated,
    /// catalogue or estimated), else 0: a sharp point, the model before K5.
    /// The feasibility stage warns on an `Unknown` tip of a V-bit.
    /// `build_cutter` and [`Self::geometry_hint`] read it, so the two stay
    /// equal.
    pub fn v_tip_planning_mm(&self) -> f64 {
        self.v_tip_diameter.value_or(0.0)
    }

    // ── K3 setters (feeds contract P6, decision E3) ─────────────────────
    //
    // Each setter stores the fact. A GUI, MCP, CLI or session edit writes
    // `ToolFact::Stated`; a loader writes the kind its file states.

    /// Set the stick-out (mm).
    pub fn set_stickout(&mut self, fact: ToolFact<f64>) {
        self.stickout = fact;
    }

    /// Set the flute (cutting) length (mm).
    pub fn set_cutting_length(&mut self, fact: ToolFact<f64>) {
        self.cutting_length = fact;
    }

    /// Set the shank diameter (mm).
    pub fn set_shank_diameter(&mut self, fact: ToolFact<f64>) {
        self.shank_diameter = fact;
    }

    /// Set the taper half-angle (degrees).
    pub fn set_taper_half_angle(&mut self, fact: ToolFact<f64>) {
        self.taper_half_angle = fact;
    }

    /// Set the flute count.
    pub fn set_flute_count(&mut self, fact: ToolFact<u32>) {
        self.flute_count = fact;
    }

    /// Set the centre-cutting fact.
    pub fn set_centre_cutting(&mut self, fact: ToolFact<CentreCutting>) {
        self.centre_cutting = fact;
    }

    /// Set the V-bit tip flat diameter (mm).
    pub fn set_v_tip_diameter(&mut self, fact: ToolFact<f64>) {
        self.v_tip_diameter = fact;
    }

    /// Stable equality key for de-duplicating tools within a catalog.
    ///
    /// Policy (operator ruling, F176, 2026-10-19): the geometry and the
    /// material make a library tool distinct. Each field is IN or OUT:
    ///
    /// - IN: every cutter dimension (`tool_type`, `diameter`,
    ///   `cutting_length`, `helix_deg`, `corner_radius_mm`, `corner_radius`,
    ///   `included_angle`, `taper_half_angle`, `shaft_diameter`), the
    ///   `shank_diameter` (a fixed part of the physical tool: deflection,
    ///   collision and the simulation cache read it), `flute_count`,
    ///   `tool_material` and `cut_direction`.
    /// - OUT: `id` (project-local), `name`, `tool_number`, `stickout` (how
    ///   the tool is mounted, not what it is), `vendor`, `product_id` and
    ///   `size_units` (display only).
    ///
    /// K3 (feeds contract P6): a fact enters the key by its planning value,
    /// never by its kind: the same tool, measured or not, is one tool. IN:
    /// `centre_cutting` (yes, no with its geometry, or unknown) and
    /// `v_tip_diameter`.
    ///
    /// The destructure names every field with no `..`, so a new
    /// `ToolConfig` field does not compile until someone puts it IN or OUT.
    /// Lengths format at 0.001 mm and angles at 0.01 degree, so a float
    /// round trip through TOML does not split one tool into two.
    pub(crate) fn library_dedupe_key(&self) -> String {
        let ToolConfig {
            id: _,
            name: _,
            tool_number: _,
            tool_type,
            diameter,
            cutting_length,
            helix_deg,
            corner_radius_mm,
            corner_radius,
            included_angle,
            v_tip_diameter,
            taper_half_angle,
            shaft_diameter,
            shank_diameter,
            stickout: _,
            flute_count,
            tool_material,
            cut_direction,
            vendor: _,
            product_id: _,
            size_units: _,
            centre_cutting,
        } = self;
        let cutting_length = cutting_length.value_or(LEGACY_DEFAULT_CUTTING_LENGTH_MM);
        let taper_half_angle = taper_half_angle.value_or(LEGACY_DEFAULT_TAPER_HALF_ANGLE_DEG);
        let shank_diameter = shank_diameter.value_or(LEGACY_DEFAULT_SHANK_DIAMETER_MM);
        let flute_count = flute_count.value_or(LEGACY_DEFAULT_FLUTE_COUNT);
        let v_tip = v_tip_diameter
            .planning()
            .map_or_else(|| "v_tip_unknown".to_owned(), |(v, _)| format!("{v:.3}"));
        let non_centre_cutting = match centre_cutting.planning() {
            None => "centre_unknown".to_owned(),
            Some((CentreCutting::Yes, _)) => "centre_cutting".to_owned(),
            Some((CentreCutting::No(geometry), _)) => format!(
                "non_centre:{:.3}:{:.3}",
                geometry.centre_core_diameter_mm, geometry.blade_protrusion_mm
            ),
        };
        format!(
            "{tool_type:?}|{diameter:.3}|{flute_count}|{cutting_length:.3}|{taper_half_angle:.2}|\
             {included_angle:.2}|{corner_radius:.3}|{corner_radius_mm:.3}|{shaft_diameter:.3}|\
             {shank_diameter:.3}|{helix_deg:.2}|{tool_material:?}|{cut_direction:?}|\
             {non_centre_cutting}|{v_tip}",
        )
    }

    /// Feeds contract P24a: hold the one fact "a straight flute has no
    /// helix". A tool whose flute form is [`BitCutDirection::Straight`]
    /// takes `helix_deg = 0`. Returns the helix it replaced, when it was
    /// not 0 (a loader files a warning with it; a setter needs no notice).
    /// Every write door calls it: `ToolParamKey::CutDirection`, the session
    /// commands `AddTool` and `ReplaceTool`, the project loader and the
    /// tool-library loader. Every reader of `helix_deg` (the cutter, the
    /// engaged-edge force, the modulator) then reads 0 with no rule of its
    /// own.
    pub fn hold_flute_form(&mut self) -> Option<f64> {
        match self.cut_direction {
            BitCutDirection::Straight if self.helix_deg != 0.0 => {
                let stated = self.helix_deg;
                self.helix_deg = 0.0;
                Some(stated)
            }
            BitCutDirection::Straight
            | BitCutDirection::UpCut
            | BitCutDirection::DownCut
            | BitCutDirection::Compression => None,
        }
    }

    /// F029 step 1: the one domain check of a tool record.
    ///
    /// Every door that creates or changes a tool calls it: the session
    /// commands (`AddTool`, `AddToolFromLibrary`, `ReplaceTool`,
    /// `ReplaceTools`, `SetToolParam`), the project loader, the tool-library
    /// loader, the MCP `add_tool` builder and [`crate::compute::cutter::build_cutter`].
    /// The bounds are [`ToolDimension::domain`], which math gives. The GUI
    /// drag range ([`ToolDimension::ui_range`]) refuses nothing.
    ///
    /// It checks the shared dimensions and the geometry that the tool's
    /// OWN type reads. Geometry that belongs to another type is not
    /// checked: no consumer reads it (see [`Self::normalize_geometry`]).
    pub fn validate(&self) -> Result<(), ToolGeometryError> {
        let d = self.diameter;
        ToolDimension::Diameter.check(d, d)?;
        ToolDimension::CuttingLength.check(self.cutting_length_planning_mm(), d)?;
        ToolDimension::HelixDeg.check(self.helix_deg, d)?;
        // P24a: a straight flute has helix 0. Every write door calls
        // `hold_flute_form` first, so this refuses only a record that a
        // caller built by hand with the pair apart.
        match self.cut_direction {
            BitCutDirection::Straight => {
                ToolDimension::StraightHelixDeg.check(self.helix_deg, d)?;
            }
            BitCutDirection::UpCut | BitCutDirection::DownCut | BitCutDirection::Compression => {}
        }
        ToolDimension::FluteCount.check(f64::from(self.flute_count_planning()), d)?;
        ToolDimension::ShankDiameter.check(self.shank_diameter_planning_mm(), d)?;
        ToolDimension::Stickout.check(self.stickout_planning_mm(), d)?;
        if let Some(geometry) = self.non_centre_cutting_geometry() {
            ToolDimension::CentreCoreDiameterMm.check(geometry.centre_core_diameter_mm, d)?;
            ToolDimension::BladeProtrusionMm.check(geometry.blade_protrusion_mm, d)?;
        }
        match self.tool_type {
            ToolType::EndMill => ToolDimension::CornerRadiusMm.check(self.corner_radius_mm, d),
            ToolType::BallNose => Ok(()),
            ToolType::BullNose => ToolDimension::CornerRadius.check(self.corner_radius, d),
            ToolType::VBit => {
                ToolDimension::IncludedAngle.check(self.included_angle, d)?;
                // P7 A2 (F029): a tip that `planning()` gives is in its
                // domain, 0 <= tip < diameter. An Unknown tip is a missing
                // input (D12), not a domain fault.
                match self.v_tip_diameter.planning() {
                    Some((tip, _)) => ToolDimension::VTipDiameter.check(*tip, d),
                    None => Ok(()),
                }
            }
            ToolType::TaperedBallNose => {
                ToolDimension::TaperHalfAngle.check(self.taper_half_angle_planning_deg(), d)?;
                ToolDimension::ShaftDiameter.check(self.shaft_diameter, d)
            }
        }
    }

    /// FC0 item 7 (decision D4, ruling Q6): the stick-out is shorter than
    /// the flute, so part of the flute sits inside the collet. A real
    /// set-up, so [`Self::validate`] accepts it; the project load and the
    /// project diagnostics warn (`tool.stickout_shorter_than_flute`). A
    /// stick-out of 0 is "not stated" (the deflection gate abstains on it)
    /// and gives `false`.
    pub fn stickout_shorter_than_flute(&self) -> bool {
        self.stickout_planning_mm() > 0.0
            && self.stickout_planning_mm() < self.cutting_length_planning_mm()
    }

    /// The sentence of [`Self::stickout_shorter_than_flute`]. It names
    /// both fields.
    pub fn stickout_shorter_than_flute_text(&self) -> String {
        format!(
            "The stick-out (stickout = {} mm) is shorter than the flute (cutting_length = {} mm): \
             part of the flute is inside the collet. Check both values.",
            self.stickout_planning_mm(),
            self.cutting_length_planning_mm()
        )
    }

    /// The measured geometry of a non-centre-cutting tool: `Some` only for
    /// a `CentreCutting::No(..)` fact of any known kind. `Yes` and `Unknown`
    /// give `None`; an `Unknown` tool plans as a centre-cutting tool (the
    /// motion before K3); ruling D12 makes it a required input (P7 A2).
    pub fn non_centre_cutting_geometry(&self) -> Option<NonCentreCuttingGeometry> {
        match self.centre_cutting.planning() {
            Some((CentreCutting::No(geometry), _)) => Some(*geometry),
            Some((CentreCutting::Yes, _)) | None => None,
        }
    }

    /// The feeds geometry hint of this tool, read from the record with no
    /// cutter built.
    ///
    /// A data mapping that must equal `build_cutter(self)?.to_geometry_hint()`
    /// for every valid tool (pinned by `geometry_hint_matches_the_built_cutter`),
    /// the same layered shortcut as [`ToolType::cutter_kind`]. The Suggest
    /// input builder reads it, so a hint needs no fallible cutter build.
    pub fn geometry_hint(&self) -> crate::feeds::ToolGeometryHint {
        use crate::feeds::ToolGeometryHint;
        match self.tool_type {
            ToolType::EndMill => ToolGeometryHint::Flat,
            ToolType::BallNose => ToolGeometryHint::Ball,
            ToolType::BullNose => ToolGeometryHint::Bull {
                corner_radius: self.corner_radius,
            },
            // K5 VTipFlat: the tip that `build_cutter` builds.
            ToolType::VBit => ToolGeometryHint::VBit {
                included_angle: self.included_angle,
                tip_diameter: self.v_tip_planning_mm(),
            },
            ToolType::TaperedBallNose => ToolGeometryHint::TaperedBall {
                tip_radius: self.diameter / 2.0,
                taper_angle_deg: self.taper_half_angle_planning_deg(),
            },
        }
    }

    /// Diameter of the cutter's widest cutting envelope.
    ///
    /// For most tools this equals `diameter`. For `TaperedBallNose`, `diameter`
    /// is the ball tip diameter while the shaft (cone base) is larger — the
    /// envelope is the shaft. Use this for boundary clipping, keep-out
    /// offsets, helix-entry radius, or anywhere that cares about the tool's
    /// outer cutting footprint rather than the tip.
    pub fn envelope_diameter(&self) -> f64 {
        match self.tool_type {
            ToolType::TaperedBallNose => self.shaft_diameter.max(self.diameter),
            ToolType::EndMill | ToolType::BallNose | ToolType::BullNose | ToolType::VBit => {
                self.diameter
            }
        }
    }

    /// Short description for the project tree, the toolpath picker and
    /// the simulation list: the type label, then [`Self::size_label`].
    /// A tapered ball also gives the taper half-angle, as it did before.
    pub fn summary(&self) -> String {
        let size = self.size_label();
        match self.tool_type {
            ToolType::TaperedBallNose => format!(
                "{} {size}, {:.1}\u{00B0} half-angle",
                self.tool_type.label(),
                self.taper_half_angle_planning_deg()
            ),
            ToolType::EndMill | ToolType::BallNose | ToolType::BullNose | ToolType::VBit => {
                format!("{} {size}", self.tool_type.label())
            }
        }
    }

    /// The short label of the tool for a closed dropdown (W3g): the type
    /// and the size that sets the cut, in the unit of
    /// [`Self::effective_size_units`]. [`Self::summary`] is the full text.
    ///
    /// - End mill: `End mill Ø6.00` (with ` R0.50` for a corner radius)
    /// - Ball nose: `Ball nose Ø6.00`
    /// - Bull nose: `Bull nose Ø6.00 R1.00`
    /// - V-bit: `V-bit Ø12.70 60°`
    /// - Tapered ball: `Tapered ball R1.00 · 5.7°` (the tip radius and the
    ///   taper half-angle)
    pub fn short_label(&self) -> String {
        let length = |mm: f64| match self.effective_size_units() {
            SizeUnits::Metric => format!("{mm:.2}"),
            SizeUnits::Imperial => size_units::inch_text(mm),
        };
        let d = self.diameter;
        match self.tool_type {
            ToolType::EndMill if self.corner_radius_mm > 0.0 => format!(
                "End mill \u{00D8}{} R{}",
                length(d),
                length(self.corner_radius_mm)
            ),
            ToolType::EndMill => format!("End mill \u{00D8}{}", length(d)),
            ToolType::BallNose => format!("Ball nose \u{00D8}{}", length(d)),
            ToolType::BullNose => format!(
                "Bull nose \u{00D8}{} R{}",
                length(d),
                length(self.corner_radius)
            ),
            ToolType::VBit => format!(
                "V-bit \u{00D8}{} {:.0}\u{00B0}",
                length(d),
                self.included_angle
            ),
            ToolType::TaperedBallNose => format!(
                "Tapered ball R{} \u{00B7} {:.1}\u{00B0}",
                length(d / 2.0),
                self.taper_half_angle_planning_deg()
            ),
        }
    }

    /// The unit this tool's size is shown and entered in: the stored
    /// [`Self::size_units`], else Imperial when the name carries an inch
    /// token (`"`, `”`, `in`, `inch`, a fraction such as `1/4"`, a
    /// decimal such as `0.25in`), else Metric.
    pub fn effective_size_units(&self) -> SizeUnits {
        self.size_units.unwrap_or_else(|| {
            if parse_name_numbers(&self.name).iter().any(|n| n.inch) {
                SizeUnits::Imperial
            } else {
                SizeUnits::Metric
            }
        })
    }

    /// The tool size in one convention, for every human-facing surface.
    ///
    /// `Ø` always prefixes a DIAMETER and `R` always prefixes a RADIUS,
    /// so a vendor name such as "R1.0mm x 6mm" can stand next to the
    /// label and the operator can compare the two. The stored values
    /// are not changed; `diameter` stays the wire key.
    ///
    /// In [`SizeUnits::Metric`]:
    ///
    /// - End mill: `Ø6.00 mm` (with `, corner R0.50` when it has one)
    /// - Ball nose: `Ø6.00 mm (R3.00)`
    /// - Tapered ball: `tip Ø2.00 mm (R1.00), shank Ø6.00`
    /// - Bull nose: `Ø6.00 mm, corner R1.00`
    /// - V-bit: `Ø12.70 mm, 60°` (the nominal diameter and the
    ///   included angle)
    ///
    /// In [`SizeUnits::Imperial`] the inch figure comes first and the mm
    /// figure follows in parentheses. An inch figure is the nearest 1/64"
    /// fraction when the size is within 0.5 % of it, else decimal inches:
    ///
    /// - End mill: `Ø1/4" (6.35 mm)`, `Ø0.236" (6.00 mm)`
    /// - Ball nose: `Ø1/4" (R1/8", 6.35 mm)`
    /// - Tapered ball: `tip Ø1/16" (R1/32", 1.59 mm), shank Ø1/4"`
    /// - Bull nose: `Ø1/2" (12.70 mm), corner R1/16"`
    /// - V-bit: `Ø1/2" (12.70 mm), 60°`
    pub fn size_label(&self) -> String {
        match self.effective_size_units() {
            SizeUnits::Metric => self.metric_size_label(),
            SizeUnits::Imperial => self.imperial_size_label(),
        }
    }

    fn metric_size_label(&self) -> String {
        let d = self.diameter;
        match self.tool_type {
            ToolType::EndMill if self.corner_radius_mm > 0.0 => {
                format!("\u{00D8}{d:.2} mm, corner R{:.2}", self.corner_radius_mm)
            }
            ToolType::EndMill => format!("\u{00D8}{d:.2} mm"),
            ToolType::BallNose => format!("\u{00D8}{d:.2} mm (R{:.2})", d / 2.0),
            ToolType::BullNose => {
                format!("\u{00D8}{d:.2} mm, corner R{:.2}", self.corner_radius)
            }
            ToolType::VBit => format!("\u{00D8}{d:.2} mm, {:.0}\u{00B0}", self.included_angle),
            ToolType::TaperedBallNose => format!(
                "tip \u{00D8}{d:.2} mm (R{:.2}), shank \u{00D8}{:.2}",
                d / 2.0,
                self.upper_diameter_mm()
            ),
        }
    }

    fn imperial_size_label(&self) -> String {
        let d = self.diameter;
        let inch = size_units::inch_text;
        match self.tool_type {
            ToolType::EndMill if self.corner_radius_mm > 0.0 => format!(
                "\u{00D8}{} ({d:.2} mm), corner R{}",
                inch(d),
                inch(self.corner_radius_mm)
            ),
            ToolType::EndMill => format!("\u{00D8}{} ({d:.2} mm)", inch(d)),
            ToolType::BallNose => {
                format!("\u{00D8}{} (R{}, {d:.2} mm)", inch(d), inch(d / 2.0))
            }
            ToolType::BullNose => format!(
                "\u{00D8}{} ({d:.2} mm), corner R{}",
                inch(d),
                inch(self.corner_radius)
            ),
            ToolType::VBit => format!(
                "\u{00D8}{} ({d:.2} mm), {:.0}\u{00B0}",
                inch(d),
                self.included_angle
            ),
            ToolType::TaperedBallNose => format!(
                "tip \u{00D8}{} (R{}, {d:.2} mm), shank \u{00D8}{}",
                inch(d),
                inch(d / 2.0),
                inch(self.upper_diameter_mm())
            ),
        }
    }

    /// The diameter above the cutting tip that the size label calls the
    /// shank: THE rule of the body, `tool::body::upper_diameter_mm` (the
    /// shank, else the cone base; P22-0c-6). The label reads the record,
    /// not a built body, so it cannot fail.
    fn upper_diameter_mm(&self) -> f64 {
        crate::tool::body::upper_diameter_mm(
            self.shank_diameter_planning_mm(),
            self.envelope_diameter(),
        )
    }

    /// The label for the `diameter` field on an edit form. On a tapered
    /// ball `diameter` is the TIP diameter, so the label says so.
    pub fn diameter_field_label(&self) -> &'static str {
        match self.tool_type {
            ToolType::TaperedBallNose => "Tip \u{00D8}:",
            ToolType::EndMill | ToolType::BallNose | ToolType::BullNose | ToolType::VBit => {
                "Diameter \u{00D8}:"
            }
        }
    }

    /// The tip radius as read-only text beside the diameter field
    /// (`= R1.00`, or `= R1/8"` in [`SizeUnits::Imperial`]), for the two
    /// types whose tip is a ball. `None` for the other types.
    pub fn derived_tip_radius_text(&self) -> Option<String> {
        let r = self.diameter / 2.0;
        self.tool_type
            .has_ball_tip()
            .then(|| match self.effective_size_units() {
                SizeUnits::Metric => format!("= R{r:.2}"),
                SizeUnits::Imperial => format!("= R{}", size_units::inch_text(r)),
            })
    }

    /// Cross-type geometry this tool is carrying: fields owned by a
    /// DIFFERENT [`ToolType`] that still hold a non-zero value. Read-only
    /// twin of [`ToolConfig::normalize_geometry`].
    pub(crate) fn cross_type_geometry(&self) -> Vec<ClearedGeometry> {
        ToolGeometryField::ALL
            .iter()
            .filter(|f| f.owner() != self.tool_type)
            .map(|f| ClearedGeometry {
                field: *f,
                was: f.value_of(self),
            })
            .filter(|c| c.was != 0.0)
            .collect()
    }

    /// Zero the geometry belonging to OTHER tool types, returning what
    /// was cleared. Geometry that defines this type is left alone —
    /// 4b1e6f21's rule, applied here to tools that were already saved
    /// before that commit landed.
    ///
    /// This changes no cutting geometry. Every consumer dispatches on
    /// `tool_type` (or on the `CutterKind` derived from it) before it
    /// reads any of these fields, so on a type that does not own the
    /// field the value was never read: `compute::cutter::build_cutter`
    /// (the only builder of a `MillingCutter` from a `ToolConfig`),
    /// `ToolConfig::envelope_diameter`, `ops::chamfer::vbit_half_angle`,
    /// `feeds::predict::bending_diameter_mm`, and the viz wireframe
    /// builder `render::sim_render::ToolGeometry::from_tool_config`.
    ///
    /// The two consumers that read them type-agnostically are report
    /// surfaces, and they are the reason this exists: `Session::list_tools`
    /// publishes `corner_radius_mm.max(corner_radius)`, so a flat end
    /// mill was reporting a 2 mm corner radius on the MCP wire, and
    /// `tool_library::dedupe_key` puts every geometry field into its key,
    /// so two identical end mills differing only in dead bull-nose radius
    /// counted as distinct tools.
    pub fn normalize_geometry(&mut self) -> Vec<ClearedGeometry> {
        let cleared = self.cross_type_geometry();
        for c in &cleared {
            c.field.clear_on(self);
        }
        cleared
    }

    /// Geometry the tool's own type requires but does not have.
    ///
    /// Not a heuristic — a V-bit with no included angle is not an
    /// under-specified V-bit, it is not a V-bit at all, and the cutter
    /// constructors refuse it: `VBitEndmill::new` and
    /// `TaperedBallEndmill::new` check the [`ToolDimension`] domain, and
    /// [`Self::validate`] refuses the record at every tool door (F029; until
    /// then a zero here was a panic inside `build_cutter`). Callers that
    /// mutate `tool_type` in place (the
    /// GUI's tool-type combo is the only one) must check this after the
    /// switch and refill from `ToolConfig::new_default(id, new_type)`.
    pub fn missing_defining_geometry(&self) -> Vec<ToolGeometryField> {
        ToolGeometryField::ALL
            .iter()
            .filter(|f| f.owner() == self.tool_type && f.defines_owner())
            .filter(|f| f.value_of(self) <= 0.0)
            .copied()
            .collect()
    }

    /// Advisory check: does this tool's free-text `name` agree with its
    /// typed geometry?
    ///
    /// Motivating case (live project, 2026-08-21): a tool named
    /// "Tapered Ball 2mm tip / 7° / 6mm shank" carrying `diameter: 1.0`.
    /// For a tapered ball `diameter` IS the ball/tip diameter
    /// (`TaperedBallEndmill::new(ball_diameter, ..)`), so the tool was
    /// half its named size. Every feed decision followed the number and
    /// every human decision followed the name, and nothing compared the
    /// two.
    ///
    /// ADVISORY ONLY. Names are free text and this parse is a
    /// heuristic, so no caller may refuse on it — a false refusal on a
    /// legitimately odd name is worse than the defect it guards. It
    /// prefers silence to a guess, and deliberately does NOT flag:
    ///
    /// - a `Ø` or `D` figure that is not the FIRST figure in the name
    ///   ("R1.0 x Ø6" may name the shank);
    /// - a number with no unit ("2F", "Amana 46200", "T3") — that is
    ///   not a dimension;
    /// - a phrase carrying several united numbers or several role words
    ///   ("6mm shank x 20mm loc") — nothing binds, rather than binding
    ///   the wrong pair;
    /// - unkeyworded millimetre figures unless the name contains
    ///   exactly ONE of them: a vendor string like
    ///   "R0.5mm x 6mm x 20mm 2F Tapered Ball" lists tip radius, shank
    ///   and flute length, and choosing which one is "the" diameter is
    ///   a guess;
    /// - degrees on the three types where no angle defines the
    ///   geometry — there, a degree figure is as likely to be the helix;
    /// - a field reading zero, which means unset, not contradicted;
    /// - anything inside [`NAME_MATCH_REL_TOL`], which is loose enough
    ///   to cover "6mm" written for a 6.35 mm (1/4") shank.
    ///
    /// Inch figures ("1/4\"", "1/8in", "0.25in") are converted to
    /// millimetres and then read as any other millimetre figure.
    pub fn name_geometry_mismatches(&self) -> Vec<NameGeometryMismatch> {
        let numbers = parse_name_numbers(&self.name);
        let mut out: Vec<NameGeometryMismatch> = Vec::new();

        for (i, n) in numbers.iter().enumerate() {
            let Some(claim) = self.name_claim(n, i == 0) else {
                continue;
            };
            push_mismatch(&mut out, claim.against(n.value, n.units()));
        }

        // Unkeyworded millimetre figures. Judged only when the name
        // carries exactly ONE — then it is "the size".
        let bare: Vec<&NameNumber> = numbers
            .iter()
            .filter(|n| {
                n.unit == NameUnit::Mm
                    && n.role.is_none()
                    && !n.radius_prefixed
                    && !n.diameter_prefixed
            })
            .collect();
        if let [only] = bare.as_slice() {
            let claim = self.bare_length_claim();
            let found = claim
                .against(only.value, only.units())
                .or_else(|| self.unit_crossing(only));
            push_mismatch(&mut out, found);
        }

        out
    }

    /// The unit rule (operator ruling, 2026-09-24): the lone size of a
    /// name in one unit system, on a tool whose diameter is a size of
    /// the OTHER system, is a real mismatch, even when the figure matches
    /// the shank or another dimension. "1/4\"" on a Ø6.00 tool, or "6mm"
    /// on a Ø6.35 (1/4") tool, warns.
    fn unit_crossing(&self, n: &NameNumber) -> Option<NameGeometryMismatch> {
        let d = self.diameter;
        if d <= 0.0 || !d.is_finite() || approx_matches_within(n.value, d, NAME_TIP_REL_TOL) {
            return None;
        }
        (n.inch != size_units::looks_imperial(d)).then(|| NameGeometryMismatch {
            quantity: NamedQuantity::TipDiameter,
            name_value: n.value,
            config_value: d,
            field: "diameter",
            name_units: n.units(),
        })
    }

    /// The one name/geometry disagreement about the TIP size, if any.
    ///
    /// This is the subset of [`Self::name_geometry_mismatches`] that a
    /// token CLEARLY names the cutting size: an `R` figure, a figure
    /// with the word "tip", a `Ø`/`D` figure first in the name, or the
    /// single millimetre figure of a name. A tip claim is judged at
    /// [`NAME_TIP_REL_TOL`] (2 %), and the unit rule applies. The tool panel shows it as a
    /// caution, and the diagnostics carry it as
    /// `tool.name_size_mismatch`. It is advisory: no caller refuses on it.
    pub fn name_tip_size_mismatch(&self) -> Option<NameGeometryMismatch> {
        self.name_geometry_mismatches()
            .into_iter()
            .find(|m| m.quantity.is_tip_size())
    }

    /// What a single figure from the name would be claiming about this
    /// tool — or `None` where this parse declines to read it. `is_first`
    /// is true for the first figure in the name.
    fn name_claim(&self, n: &NameNumber, is_first: bool) -> Option<NameClaim> {
        let tip_radius = self.diameter / 2.0;
        if n.radius_prefixed {
            // An `R` prefix names a RADIUS, whatever unit follows it.
            let mut candidates = vec![tip_radius];
            match self.tool_type {
                ToolType::BullNose => candidates.push(self.corner_radius),
                ToolType::EndMill => candidates.push(self.corner_radius_mm),
                ToolType::BallNose | ToolType::VBit | ToolType::TaperedBallNose => {}
            }
            return Some(NameClaim {
                quantity: NamedQuantity::TipRadius,
                candidates,
                reported: tip_radius,
                field: "diameter",
            });
        }
        if n.unit == NameUnit::Degrees {
            return self.angle_claim();
        }
        if n.unit != NameUnit::Mm {
            return None;
        }
        let Some(role) = n.role else {
            // A `Ø` or `D` figure first in the name is the cutting size.
            // Later in the name it can be the shank, so it is not read.
            return (n.diameter_prefixed && is_first).then(|| NameClaim {
                quantity: NamedQuantity::TipDiameter,
                candidates: vec![self.diameter],
                reported: self.diameter,
                field: "diameter",
            });
        };
        match role {
            NameRole::Tip => Some(NameClaim {
                quantity: NamedQuantity::TipDiameter,
                candidates: vec![self.diameter],
                reported: self.diameter,
                field: "diameter",
            }),
            NameRole::Shank => {
                // On a tapered ball the cone base and the shank are the
                // same stated figure often enough that either reading
                // counts as agreement.
                let mut candidates = vec![self.shank_diameter_planning_mm()];
                if self.tool_type == ToolType::TaperedBallNose {
                    candidates.push(self.shaft_diameter);
                }
                Some(NameClaim {
                    quantity: NamedQuantity::ShankDiameter,
                    candidates,
                    reported: self.shank_diameter_planning_mm(),
                    field: "shank_diameter",
                })
            }
            NameRole::CuttingLength => Some(NameClaim {
                quantity: NamedQuantity::CuttingLength,
                candidates: vec![self.cutting_length_planning_mm()],
                reported: self.cutting_length_planning_mm(),
                field: "cutting_length",
            }),
        }
    }

    /// Degrees are judged only on the two types where an angle DEFINES
    /// the geometry; on the other three a degree figure in a name is as
    /// likely to be the helix. `helix_deg` counts as agreement in both
    /// cases, because "30° helix" is a legitimate thing to write.
    fn angle_claim(&self) -> Option<NameClaim> {
        match self.tool_type {
            ToolType::VBit => Some(NameClaim {
                quantity: NamedQuantity::IncludedAngle,
                candidates: vec![self.included_angle, self.helix_deg],
                reported: self.included_angle,
                field: "included_angle",
            }),
            ToolType::TaperedBallNose => Some(NameClaim {
                quantity: NamedQuantity::ConeHalfAngle,
                // Vendors quote a taper per-side or as the full
                // included cone; both readings count.
                candidates: vec![
                    self.taper_half_angle_planning_deg(),
                    self.taper_half_angle_planning_deg() * 2.0,
                    self.helix_deg,
                ],
                reported: self.taper_half_angle_planning_deg(),
                field: "taper_half_angle",
            }),
            ToolType::EndMill | ToolType::BallNose | ToolType::BullNose => None,
        }
    }

    /// The claim a lone unkeyworded millimetre figure makes: it is the
    /// tool's size. Checked against every dimension the tool actually
    /// has before being called a contradiction, and reported against
    /// `diameter`, because that is what such a name means.
    fn bare_length_claim(&self) -> NameClaim {
        let mut candidates = vec![
            self.diameter,
            self.envelope_diameter(),
            self.cutting_length_planning_mm(),
            self.shank_diameter_planning_mm(),
            self.stickout_planning_mm(),
        ];
        // Plus the length geometry this type owns — which is exactly
        // what `normalize_geometry` leaves behind.
        let owned = ToolGeometryField::ALL
            .iter()
            .filter(|f| f.owner() == self.tool_type && f.is_length())
            .map(|f| f.value_of(self));
        candidates.extend(owned);
        NameClaim {
            quantity: NamedQuantity::TipDiameter,
            candidates,
            reported: self.diameter,
            field: "diameter",
        }
    }
}

/// A geometry field that exactly one [`ToolType`] consumes.
///
/// All five live on every [`ToolConfig`], but each is read by exactly
/// one arm of the type dispatch in `compute::cutter::build_cutter`. On
/// any other type the stored number is dead weight — dead, but not
/// harmless: it survives save/load and reaches the report surfaces that
/// do not dispatch on type. See [`ToolConfig::normalize_geometry`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolGeometryField {
    /// Corner radius of a flat end mill (`FlatEndmill::corner_radius_mm`).
    CornerRadiusMm,
    /// Bull-nose corner radius (`BullNoseEndmill::new`, 2nd argument).
    CornerRadius,
    /// V-bit full included angle, degrees.
    IncludedAngle,
    /// Tapered-ball cone half-angle, degrees.
    TaperHalfAngle,
    /// Tapered-ball cone base ("shaft") diameter, mm.
    ShaftDiameter,
}

impl ToolGeometryField {
    pub const ALL: &[ToolGeometryField] = &crate::all_variants!(ToolGeometryField:
        CornerRadiusMm, CornerRadius, IncludedAngle, TaperHalfAngle, ShaftDiameter,
    );

    /// The one tool type that consumes this field.
    pub const fn owner(self) -> ToolType {
        match self {
            ToolGeometryField::CornerRadiusMm => ToolType::EndMill,
            ToolGeometryField::CornerRadius => ToolType::BullNose,
            ToolGeometryField::IncludedAngle => ToolType::VBit,
            ToolGeometryField::TaperHalfAngle | ToolGeometryField::ShaftDiameter => {
                ToolType::TaperedBallNose
            }
        }
    }

    /// The field name as spelled in project TOML and on the MCP wire.
    pub const fn field_name(self) -> &'static str {
        match self {
            ToolGeometryField::CornerRadiusMm => "corner_radius_mm",
            ToolGeometryField::CornerRadius => "corner_radius",
            ToolGeometryField::IncludedAngle => "included_angle",
            ToolGeometryField::TaperHalfAngle => "taper_half_angle",
            ToolGeometryField::ShaftDiameter => "shaft_diameter",
        }
    }

    /// Whether the owning type is degenerate without it. A flat end
    /// mill with no corner radius is an ordinary flat end mill; a V-bit
    /// with no included angle is not a V-bit at all.
    pub(crate) const fn defines_owner(self) -> bool {
        !matches!(self, ToolGeometryField::CornerRadiusMm)
    }

    /// Whether the field is a length (mm) rather than an angle (deg).
    pub(crate) const fn is_length(self) -> bool {
        matches!(
            self,
            ToolGeometryField::CornerRadiusMm
                | ToolGeometryField::CornerRadius
                | ToolGeometryField::ShaftDiameter
        )
    }

    /// The value of this field on `tool`. K3: an `Unknown` taper half-angle
    /// reads 0, the value of geometry the tool does not have.
    pub fn value_of(self, tool: &ToolConfig) -> f64 {
        match self {
            ToolGeometryField::CornerRadiusMm => tool.corner_radius_mm,
            ToolGeometryField::CornerRadius => tool.corner_radius,
            ToolGeometryField::IncludedAngle => tool.included_angle,
            ToolGeometryField::TaperHalfAngle => tool.taper_half_angle.value_or(0.0),
            ToolGeometryField::ShaftDiameter => tool.shaft_diameter,
        }
    }

    /// K3: a cleared taper half-angle is `Unknown` (no fact); every other
    /// field is 0.
    fn clear_on(self, tool: &mut ToolConfig) {
        match self {
            ToolGeometryField::TaperHalfAngle => tool.taper_half_angle = ToolFact::Unknown,
            ToolGeometryField::CornerRadiusMm
            | ToolGeometryField::CornerRadius
            | ToolGeometryField::IncludedAngle
            | ToolGeometryField::ShaftDiameter => self.set_on(tool, 0.0),
        }
    }

    /// Write this field on `tool`.
    ///
    /// The counterpart to [`Self::value_of`], and the reason it is public:
    /// an in-place TYPE SWITCH has to refill the geometry that DEFINES the
    /// new type, and the only honest source for those values is
    /// `ToolConfig::new_default(id, new_type)`. Without a setter the caller
    /// would have to re-enumerate the fields itself, which is exactly the
    /// per-type knowledge this enum exists to hold in one place.
    ///
    /// Concretely: `new_default` normalises, so a tool switched from
    /// EndMill to VBit in the GUI arrives with `included_angle = 0.0`, and
    /// `VBitEndmill::new` refuses it (F029; it used to assert, a panic
    /// reachable from a combo box). See
    /// `ui/properties/tool.rs`'s type selector, which pairs
    /// [`ToolConfig::missing_defining_geometry`] with this.
    ///
    /// K3: the taper half-angle is a tool fact; the value comes from
    /// `new_default`, so this writes it `Estimated` with that basis.
    pub fn set_on(self, tool: &mut ToolConfig, value: f64) {
        match self {
            ToolGeometryField::CornerRadiusMm => tool.corner_radius_mm = value,
            ToolGeometryField::CornerRadius => tool.corner_radius = value,
            ToolGeometryField::IncludedAngle => tool.included_angle = value,
            ToolGeometryField::TaperHalfAngle => {
                tool.taper_half_angle = ToolFact::estimated(value, NEW_DEFAULT_BASIS);
            }
            ToolGeometryField::ShaftDiameter => tool.shaft_diameter = value,
        }
    }
}

/// A cross-type geometry value [`ToolConfig::normalize_geometry`]
/// cleared (or would clear), kept so a loader can report what changed
/// instead of editing a saved project silently.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClearedGeometry {
    pub field: ToolGeometryField,
    pub was: f64,
}

/// What a number recovered from a tool NAME was claiming.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedQuantity {
    TipDiameter,
    TipRadius,
    ShankDiameter,
    CuttingLength,
    ConeHalfAngle,
    IncludedAngle,
}

impl NamedQuantity {
    pub const fn label(self) -> &'static str {
        match self {
            NamedQuantity::TipDiameter => "tip diameter",
            NamedQuantity::TipRadius => "tip radius",
            NamedQuantity::ShankDiameter => "shank diameter",
            NamedQuantity::CuttingLength => "cutting length",
            NamedQuantity::ConeHalfAngle => "taper half-angle",
            NamedQuantity::IncludedAngle => "included angle",
        }
    }

    pub const fn unit(self) -> &'static str {
        match self {
            NamedQuantity::ConeHalfAngle | NamedQuantity::IncludedAngle => "deg",
            NamedQuantity::TipDiameter
            | NamedQuantity::TipRadius
            | NamedQuantity::ShankDiameter
            | NamedQuantity::CuttingLength => "mm",
        }
    }

    /// True for the two quantities that name the cutting (tip) size.
    pub const fn is_tip_size(self) -> bool {
        matches!(self, NamedQuantity::TipDiameter | NamedQuantity::TipRadius)
    }
}

/// One advisory disagreement between a tool's free-text `name` and its
/// typed geometry. Both numbers are carried so the message can name
/// them — the defect this exists for is invisible unless the reader
/// sees the pair.
#[derive(Debug, Clone, PartialEq)]
pub struct NameGeometryMismatch {
    pub quantity: NamedQuantity,
    /// The value the NAME asserts, in [`NamedQuantity::unit`].
    pub name_value: f64,
    /// The value the typed geometry carries, same unit.
    pub config_value: f64,
    /// The `ToolConfig` field `config_value` was read from.
    pub field: &'static str,
    /// The unit the NAME wrote the figure in. `name_value` is in mm
    /// whatever this says; the caution prints the figure back in it.
    pub name_units: SizeUnits,
}

impl NameGeometryMismatch {
    pub fn message(&self) -> String {
        format!(
            "MISMATCH: name says {} {:.3} {}, geometry says {:.3} {} ({})",
            self.quantity.label(),
            self.name_value,
            self.quantity.unit(),
            self.config_value,
            self.quantity.unit(),
            self.field,
        )
    }

    /// The operator caution for a tip-size disagreement, in the
    /// `Ø`/`R` convention of [`ToolConfig::size_label`]:
    /// "The name says R1.00 (Ø2.00 mm) but the tool is Ø1.00 mm. Check
    /// the diameter." `None` for a quantity that is not the tip size.
    pub fn caution_line(&self) -> Option<String> {
        let name = match (self.quantity, self.name_units) {
            (NamedQuantity::TipRadius, SizeUnits::Metric) => format!(
                "R{:.2} (\u{00D8}{:.2} mm)",
                self.name_value,
                self.name_value * 2.0
            ),
            (NamedQuantity::TipRadius, SizeUnits::Imperial) => format!(
                "R{} (\u{00D8}{}, {:.2} mm)",
                size_units::inch_text(self.name_value),
                size_units::inch_text(self.name_value * 2.0),
                self.name_value * 2.0
            ),
            (NamedQuantity::TipDiameter, SizeUnits::Metric) => {
                format!("\u{00D8}{:.2} mm", self.name_value)
            }
            (NamedQuantity::TipDiameter, SizeUnits::Imperial) => format!(
                "{} ({:.2} mm)",
                size_units::inch_text(self.name_value),
                self.name_value
            ),
            _ => return None,
        };
        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "projection: only the named variant(s) carry this (named: NamedQuantity::TipRadius)"
        )]
        let diameter = match self.quantity {
            NamedQuantity::TipRadius => self.config_value * 2.0,
            _ => self.config_value,
        };
        // The tool's figure reads in the unit it was most likely sold in.
        let tool = if size_units::looks_imperial(diameter) {
            format!(
                "\u{00D8}{} ({diameter:.2} mm)",
                size_units::inch_text(diameter)
            )
        } else {
            format!("\u{00D8}{diameter:.2} mm")
        };
        Some(format!(
            "The name says {name} but the tool is {tool}. Check the diameter."
        ))
    }
}

/// Relative tolerance for calling a number in a tool's name the same
/// number as one in its geometry. Deliberately loose: names round
/// ("6mm" for a 6.35 mm 1/4" shank), and the defect this guards is a
/// factor of two, not a rounding.
pub const NAME_MATCH_REL_TOL: f64 = 0.10;

/// Relative tolerance for a TIP size claim (tip diameter or tip radius).
/// Tight since the operator ruled the unit convention (2026-09-24): the
/// tool is sold in one unit, so "1/4\"" on a Ø6.00 tool (5.5 % apart) is a
/// real mismatch, not a rounding.
pub const NAME_TIP_REL_TOL: f64 = 0.02;

/// Do a name figure and a geometry figure agree, within
/// [`NAME_MATCH_REL_TOL`] of the larger of the pair? Only the tests read
/// the 10 % rule by this name; production code passes its tolerance.
#[cfg(test)]
fn approx_matches(a: f64, b: f64) -> bool {
    approx_matches_within(a, b, NAME_MATCH_REL_TOL)
}

/// Do `a` and `b` agree within `tol` of the larger of the pair?
fn approx_matches_within(a: f64, b: f64, tol: f64) -> bool {
    if !a.is_finite() || !b.is_finite() {
        return false;
    }
    let scale = a.abs().max(b.abs());
    if scale == 0.0 {
        return true;
    }
    (a - b).abs() <= tol * scale
}

/// What one figure in a tool name would be claiming about the tool:
/// every geometry value it could legitimately be naming, plus the one
/// to report against when it names none of them.
struct NameClaim {
    quantity: NamedQuantity,
    candidates: Vec<f64>,
    reported: f64,
    field: &'static str,
}

impl NameClaim {
    /// Report a disagreement only when the name figure matches NONE of
    /// the geometry values it could plausibly be naming, and at least
    /// one of those is actually set — a zero field is unset, not
    /// contradicted.
    fn against(&self, name_value: f64, name_units: SizeUnits) -> Option<NameGeometryMismatch> {
        if self.reported <= 0.0 || !self.reported.is_finite() {
            return None;
        }
        if !self.candidates.iter().any(|c| *c > 0.0 && c.is_finite()) {
            return None;
        }
        let tol = if self.quantity.is_tip_size() {
            NAME_TIP_REL_TOL
        } else {
            NAME_MATCH_REL_TOL
        };
        let agrees = |c: &f64| approx_matches_within(name_value, *c, tol);
        if self.candidates.iter().any(agrees) {
            return None;
        }
        Some(NameGeometryMismatch {
            quantity: self.quantity,
            name_value,
            config_value: self.reported,
            field: self.field,
            name_units,
        })
    }
}

/// Append, unless the same claim is already reported — a name may
/// repeat a figure ("6mm shank, 6mm collet") and one advisory line per
/// distinct claim is the useful output.
fn push_mismatch(out: &mut Vec<NameGeometryMismatch>, found: Option<NameGeometryMismatch>) {
    let Some(m) = found else {
        return;
    };
    let duplicate = out
        .iter()
        .any(|e| e.quantity == m.quantity && e.name_value.to_bits() == m.name_value.to_bits());
    if !duplicate {
        out.push(m);
    }
}

/// Unit read off the characters that follow a number in a tool name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NameUnit {
    Mm,
    Degrees,
    /// Inches (`"`, `in`, `inch`). The scan converts the value to
    /// millimetres and stores [`NameUnit::Mm`].
    Inch,
    /// No unit.
    None,
}

/// A role word standing next to a number in the same phrase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NameRole {
    Tip,
    Shank,
    CuttingLength,
}

/// One number recovered from a tool name, with everything the parse
/// could say about what it refers to.
#[derive(Debug, Clone, Copy)]
struct NameNumber {
    value: f64,
    unit: NameUnit,
    /// An `R` glued to the digits ("R0.5") — a radius, not a diameter.
    radius_prefixed: bool,
    /// A `Ø`, `⌀` or `D` glued to the digits ("Ø6", "D6") — a diameter.
    diameter_prefixed: bool,
    /// The name wrote the figure in inches. `value` is in mm already.
    inch: bool,
    role: Option<NameRole>,
}

impl NameNumber {
    fn units(&self) -> SizeUnits {
        if self.inch {
            SizeUnits::Imperial
        } else {
            SizeUnits::Metric
        }
    }
}

/// Recover the numbers a tool name asserts, with the unit and the role
/// word beside each. See [`ToolConfig::name_geometry_mismatches`] for
/// what this deliberately will not read.
fn parse_name_numbers(name: &str) -> Vec<NameNumber> {
    let lower = fold_inch_fractions(&name.to_ascii_lowercase());
    let mut out = Vec::new();
    // Segment on the separators names actually use, so a role word
    // binds only to the number it shares a phrase with:
    // "2mm tip / 7° / 6mm shank".
    for segment in lower.split(['/', ',', ';', '(', ')', '[', ']', '|', '\n']) {
        let (mut numbers, roles) = scan_segment(segment);
        // Bind only when the phrase is unambiguous: exactly one number
        // carrying a unit, and exactly one role word. Vendor strings
        // that pack three dimensions and two keywords into one phrase
        // are left unbound rather than paired by guesswork.
        let united = numbers.iter().filter(|n| n.unit != NameUnit::None).count();
        let bound = match roles.as_slice() {
            [role] if united == 1 => Some(*role),
            _ => None,
        };
        if let Some(role) = bound {
            for n in numbers.iter_mut().filter(|n| n.unit != NameUnit::None) {
                n.role = Some(role);
            }
        }
        out.append(&mut numbers);
    }
    out
}

/// Walk one phrase, collecting united numbers and role words.
fn scan_segment(segment: &str) -> (Vec<NameNumber>, Vec<NameRole>) {
    let chars: Vec<char> = segment.chars().collect();
    let mut numbers = Vec::new();
    let mut roles = Vec::new();
    let mut i = 0usize;
    while let Some(&c) = chars.get(i) {
        if c.is_ascii_digit() {
            let start = i;
            let mut digits = String::new();
            while let Some(&d) = chars.get(i) {
                if d.is_ascii_digit() || d == '.' {
                    digits.push(d);
                    i += 1;
                } else {
                    break;
                }
            }
            let (mut unit, resume) = read_unit(&chars, i);
            i = resume;
            let is_radius = radius_prefixed(&chars, start);
            let is_diameter = diameter_prefixed(&chars, start);
            let parsed = digits.trim_end_matches('.').parse::<f64>().ok();
            if let Some(mut value) = parsed.filter(|v| v.is_finite() && *v > 0.0) {
                let inch = unit == NameUnit::Inch;
                if inch {
                    value *= MM_PER_INCH;
                    unit = NameUnit::Mm;
                }
                // "Ø6" and "D6" carry no unit: a diameter prefix means mm.
                if is_diameter && unit == NameUnit::None {
                    unit = NameUnit::Mm;
                }
                numbers.push(NameNumber {
                    value,
                    unit,
                    radius_prefixed: is_radius,
                    diameter_prefixed: is_diameter,
                    inch,
                    role: None,
                });
            }
            continue;
        }
        if c.is_alphabetic() {
            let mut word = String::new();
            while let Some(&w) = chars.get(i) {
                if w.is_alphabetic() {
                    word.push(w);
                    i += 1;
                } else {
                    break;
                }
            }
            // Whole words only, and only words that name a dimension
            // unambiguously. "ball" is NOT one of them: it appears in
            // "Tapered Ball", where it names the tool, not a figure.
            match word.as_str() {
                "tip" => roles.push(NameRole::Tip),
                "shank" | "shaft" => roles.push(NameRole::Shank),
                "loc" => roles.push(NameRole::CuttingLength),
                _ => {}
            }
            continue;
        }
        i += 1;
    }
    (numbers, roles)
}

/// Is the number starting at `start` preceded by a bare `R` ("R0.5"),
/// rather than by the tail of a word ("tr8" is a product code)?
fn radius_prefixed(chars: &[char], start: usize) -> bool {
    let Some(prev) = start.checked_sub(1) else {
        return false;
    };
    if chars.get(prev) != Some(&'r') {
        return false;
    }
    match start.checked_sub(2) {
        None => true,
        Some(before) => chars.get(before).is_none_or(|c| !c.is_alphanumeric()),
    }
}

/// Is the number starting at `start` preceded by a bare `Ø`, `⌀` or `D`
/// ("Ø6", "D6"), rather than by the tail of a word ("hd6")?
fn diameter_prefixed(chars: &[char], start: usize) -> bool {
    let Some(prev) = start.checked_sub(1) else {
        return false;
    };
    if !matches!(
        chars.get(prev),
        Some('d' | '\u{00D8}' | '\u{00F8}' | '\u{2300}')
    ) {
        return false;
    }
    match start.checked_sub(2) {
        None => true,
        Some(before) => chars.get(before).is_none_or(|c| !c.is_alphanumeric()),
    }
}

/// Rewrite each inch fraction ("1/4\"", "1/8in") as a decimal ("0.25\"")
/// before the name is split on `/`. A fraction with no inch unit after
/// it is left alone: "2mm tip / 7°" is two phrases, not a fraction.
fn fold_inch_fractions(lower: &str) -> String {
    let chars: Vec<char> = lower.chars().collect();
    let mut out = String::with_capacity(lower.len());
    let mut i = 0usize;
    while i < chars.len() {
        if let Some((value, end)) = inch_fraction_at(&chars, i) {
            out.push_str(&value.to_string());
            i = end;
            continue;
        }
        if let Some(&c) = chars.get(i) {
            out.push(c);
        }
        i += 1;
    }
    out
}

/// An inch fraction `<digits>/<digits>` that starts at `i` (not inside a
/// longer number) and is followed by an inch unit. Returns the decimal
/// value and the index of the unit.
fn inch_fraction_at(chars: &[char], i: usize) -> Option<(f64, usize)> {
    let starts_number = i
        .checked_sub(1)
        .and_then(|p| chars.get(p))
        .is_none_or(|c| !c.is_ascii_digit() && *c != '.');
    if !starts_number {
        return None;
    }
    let digits_from = |from: usize| -> (String, usize) {
        let mut j = from;
        let mut d = String::new();
        while let Some(&c) = chars.get(j).filter(|c| c.is_ascii_digit()) {
            d.push(c);
            j += 1;
        }
        (d, j)
    };
    let (num, slash) = digits_from(i);
    if num.is_empty() || chars.get(slash) != Some(&'/') {
        return None;
    }
    let (den, end) = digits_from(slash + 1);
    let numerator = num.parse::<f64>().ok()?;
    let denominator = den.parse::<f64>().ok().filter(|d| *d > 0.0)?;
    let (unit, _) = read_unit(chars, end);
    (unit == NameUnit::Inch).then_some((numerator / denominator, end))
}

/// Read the unit following a number, allowing one space ("6mm", "7 °",
/// "90 deg", "1/4\"", "0.25in"). Returns the unit and the index to resume
/// scanning at.
fn read_unit(chars: &[char], from: usize) -> (NameUnit, usize) {
    let mut i = from;
    if chars.get(i) == Some(&' ') {
        i += 1;
    }
    if chars.get(i) == Some(&'°') {
        return (NameUnit::Degrees, i + 1);
    }
    // An inch mark directly after the digits (no space: `6 "` is not a
    // unit), or the word "in" / "inch" / "inches".
    if matches!(chars.get(from), Some('"' | '\u{201D}')) {
        return (NameUnit::Inch, from + 1);
    }
    if chars.get(i) == Some(&'i') && chars.get(i + 1) == Some(&'n') {
        let mut j = i;
        while chars.get(j).is_some_and(|c| c.is_alphabetic()) {
            j += 1;
        }
        let word: String = chars.get(i..j).unwrap_or_default().iter().collect();
        if matches!(word.as_str(), "in" | "inch" | "inches") {
            return (NameUnit::Inch, j);
        }
    }
    if chars.get(i) == Some(&'m')
        && chars.get(i + 1) == Some(&'m')
        && chars.get(i + 2).is_none_or(|c| !c.is_alphabetic())
    {
        return (NameUnit::Mm, i + 2);
    }
    let deg_word = chars.get(i) == Some(&'d')
        && chars.get(i + 1) == Some(&'e')
        && chars.get(i + 2) == Some(&'g');
    if deg_word {
        // "deg", "degree", "degrees" — consume the whole word.
        let mut j = i;
        while chars.get(j).is_some_and(|c| c.is_alphabetic()) {
            j += 1;
        }
        return (NameUnit::Degrees, j);
    }
    (NameUnit::None, from)
}

// ── The test-tool builder (feeds contract P6 step 8) ─────────────────────

#[cfg(any(test, feature = "test-support"))]
impl ToolConfig {
    /// The test tool (feeds contract P7, ruling D12): [`Self::new_default`]
    /// with the required facts stated, so a test tool is an explicit test
    /// case: centre cutting `Stated(Yes)`, and a V-bit's tip `Stated(0.0)`
    /// (a sharp point, the cutter `new_default` plans). A production tool
    /// keeps them `Unknown` until the operator states them.
    #[must_use]
    pub fn test_default(id: ToolId, tool_type: ToolType) -> Self {
        let mut tool = Self::new_default(id, tool_type);
        tool.set_centre_cutting(ToolFact::Stated(CentreCutting::Yes));
        if tool_type == ToolType::VBit {
            tool.set_v_tip_diameter(ToolFact::Stated(0.0));
        }
        tool
    }

    /// The test-tool builder: it starts from [`Self::test_default`] and a
    /// test states each fact in one call. Each fact setter writes
    /// `ToolFact::Stated`; a fact with no setter call keeps its
    /// `test_default` kind (`Estimated`, basis `new_default`, for the
    /// numeric facts; `Stated` for centre cutting and the V tip).
    pub fn builder(id: ToolId, tool_type: ToolType) -> ToolConfigBuilder {
        ToolConfigBuilder {
            tool: Self::test_default(id, tool_type),
        }
    }

    /// The builder that starts from this tool (a test that changes a few
    /// facts of a helper's tool).
    pub fn to_builder(self) -> ToolConfigBuilder {
        ToolConfigBuilder { tool: self }
    }

    /// F118: every field name of `ToolConfig`, by an exhaustive destructure
    /// with no `..`, so a new field does not compile until the F118 sentry
    /// (`every_stock_and_tool_field_is_reachable_f118`) names it. It lives
    /// here because the fact fields are private (feeds contract P6).
    pub fn field_names() -> Vec<&'static str> {
        let ToolConfig {
            id: _,
            name: _,
            tool_number: _,
            tool_type: _,
            diameter: _,
            cutting_length: _,
            helix_deg: _,
            corner_radius_mm: _,
            corner_radius: _,
            included_angle: _,
            v_tip_diameter: _,
            taper_half_angle: _,
            shaft_diameter: _,
            shank_diameter: _,
            stickout: _,
            flute_count: _,
            tool_material: _,
            cut_direction: _,
            vendor: _,
            product_id: _,
            size_units: _,
            centre_cutting: _,
        } = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
        vec![
            "id",
            "name",
            "tool_number",
            "tool_type",
            "diameter",
            "cutting_length",
            "helix_deg",
            "corner_radius_mm",
            "corner_radius",
            "included_angle",
            "v_tip_diameter",
            "taper_half_angle",
            "shaft_diameter",
            "shank_diameter",
            "stickout",
            "flute_count",
            "tool_material",
            "cut_direction",
            "vendor",
            "product_id",
            "size_units",
            "centre_cutting",
        ]
    }
}

/// See [`ToolConfig::builder`]. Test and `test-support` builds only.
#[cfg(any(test, feature = "test-support"))]
#[derive(Debug, Clone)]
#[must_use]
pub struct ToolConfigBuilder {
    tool: ToolConfig,
}

#[cfg(any(test, feature = "test-support"))]
impl ToolConfigBuilder {
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.tool.name = name.into();
        self
    }

    pub fn tool_number(mut self, tool_number: u32) -> Self {
        self.tool.tool_number = tool_number;
        self
    }

    pub fn helix_deg(mut self, helix_deg: f64) -> Self {
        self.tool.helix_deg = helix_deg;
        self
    }

    pub fn tool_material(mut self, tool_material: ToolMaterial) -> Self {
        self.tool.tool_material = tool_material;
        self
    }

    pub fn cut_direction(mut self, cut_direction: BitCutDirection) -> Self {
        self.tool.cut_direction = cut_direction;
        self
    }

    pub fn vendor(mut self, vendor: impl Into<String>) -> Self {
        self.tool.vendor = vendor.into();
        self
    }

    pub fn product_id(mut self, product_id: impl Into<String>) -> Self {
        self.tool.product_id = product_id.into();
        self
    }

    pub fn size_units(mut self, size_units: Option<SizeUnits>) -> Self {
        self.tool.size_units = size_units;
        self
    }

    /// The non-centre-cutting geometry: `Some` states `CentreCutting::No`;
    /// `None` states `CentreCutting::Yes` (a test tool states its facts,
    /// ruling D12; `None` meant a centre-cutting tool before K3).
    pub fn non_centre_cutting(mut self, geometry: Option<NonCentreCuttingGeometry>) -> Self {
        self.tool.centre_cutting =
            ToolFact::Stated(geometry.map_or(CentreCutting::Yes, CentreCutting::No));
        self
    }

    pub fn diameter(mut self, diameter_mm: f64) -> Self {
        self.tool.diameter = diameter_mm;
        self
    }

    /// The flute (cutting) length, mm.
    pub fn flute_mm(mut self, cutting_length_mm: f64) -> Self {
        self.tool.cutting_length = ToolFact::Stated(cutting_length_mm);
        self
    }

    pub fn stickout_mm(mut self, stickout_mm: f64) -> Self {
        self.tool.stickout = ToolFact::Stated(stickout_mm);
        self
    }

    pub fn shank_mm(mut self, shank_diameter_mm: f64) -> Self {
        self.tool.shank_diameter = ToolFact::Stated(shank_diameter_mm);
        self
    }

    pub fn flutes(mut self, flute_count: u32) -> Self {
        self.tool.flute_count = ToolFact::Stated(flute_count);
        self
    }

    pub fn taper_half_angle_deg(mut self, taper_half_angle_deg: f64) -> Self {
        self.tool.taper_half_angle = ToolFact::Stated(taper_half_angle_deg);
        self
    }

    /// The tool does not cut at its centre: the measured centre core
    /// diameter and blade protrusion, mm.
    pub fn non_centre(mut self, centre_core_diameter_mm: f64, blade_protrusion_mm: f64) -> Self {
        self.tool.centre_cutting = ToolFact::Stated(CentreCutting::No(NonCentreCuttingGeometry {
            centre_core_diameter_mm,
            blade_protrusion_mm,
        }));
        self
    }

    /// The tool cuts at its centre (stated).
    pub fn centre_cutting(mut self) -> Self {
        self.tool.centre_cutting = ToolFact::Stated(CentreCutting::Yes);
        self
    }

    /// The tapered-ball shaft (cone base) diameter, mm.
    pub fn shaft_mm(mut self, shaft_diameter_mm: f64) -> Self {
        self.tool.shaft_diameter = shaft_diameter_mm;
        self
    }

    /// The bull-nose corner radius (`corner_radius`), mm.
    pub fn corner_radius(mut self, corner_radius: f64) -> Self {
        self.tool.corner_radius = corner_radius;
        self
    }

    /// The end-mill corner radius (`corner_radius_mm`), mm.
    pub fn corner_radius_mm(mut self, corner_radius_mm: f64) -> Self {
        self.tool.corner_radius_mm = corner_radius_mm;
        self
    }

    /// The V-bit included angle, degrees.
    pub fn included_angle_deg(mut self, included_angle_deg: f64) -> Self {
        self.tool.included_angle = included_angle_deg;
        self
    }

    pub fn build(self) -> ToolConfig {
        self.tool
    }
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]
mod tests {
    use super::*;

    /// `ToolConfig::geometry_hint` is a data shortcut for the hint the
    /// built cutter gives. The two must agree for every type.
    #[test]
    fn geometry_hint_matches_the_built_cutter() {
        for &tool_type in ToolType::ALL {
            let mut tool = ToolConfig::new_default(ToolId::from_raw(0), tool_type);
            if tool_type == ToolType::TaperedBallNose {
                tool.diameter = 1.0;
            }
            let built = crate::compute::cutter::build_cutter(&tool).unwrap();
            assert_eq!(
                tool.geometry_hint(),
                built.to_geometry_hint(),
                "{tool_type:?}"
            );
        }
    }

    /// F029: `validate` checks the geometry the tool's own type reads, and
    /// not another type's dead fields.
    #[test]
    fn validate_checks_the_geometry_the_type_owns() {
        let mut vbit = ToolConfig::new_default(ToolId::from_raw(0), ToolType::VBit);
        assert!(vbit.validate().is_ok());
        vbit.included_angle = 0.0;
        assert_eq!(
            vbit.validate().map_err(|e| e.dimension),
            Err(ToolDimension::IncludedAngle)
        );
        // A flat end mill carries no angle; a dead field is not checked.
        let mut flat = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
        flat.included_angle = 0.0;
        assert!(flat.validate().is_ok());
        flat.flute_count = ToolFact::Stated(0);
        assert_eq!(
            flat.validate().map_err(|e| e.dimension),
            Err(ToolDimension::FluteCount)
        );
    }

    /// FC0 item 7 (decision D4 = warn, ruling Q6): a stick-out shorter
    /// than the flute is a real set-up (part of the flute in the collet),
    /// so `validate` accepts it. The project load warns, and the project
    /// diagnostics carry one Caution per tool that names both fields. A
    /// stick-out of 0 is "not stated" (the deflection gate abstains on it):
    /// it gets no refusal and no warning, as before.
    #[test]
    fn a_stickout_shorter_than_the_flute_warns_fc0() {
        const ID: &str = "tool.stickout_shorter_than_flute";
        let mut short = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
        short.cutting_length = ToolFact::Stated(25.0);
        short.stickout = ToolFact::Stated(20.0);
        assert!(short.validate().is_ok(), "D4: the program never refuses it");

        // The load warning.
        let text = "name = \"Short\"\ndiameter = 6.0\ncutting_length = { stated = 25.0 }\nstickout = { stated = 20.0 }\n";
        let section: crate::session::ProjectToolSection =
            toml::from_str(text).expect("parse tool section");
        let mut warnings = Vec::new();
        let _ = crate::session::project_file::tool_from_project_section(&section, 0, &mut warnings);
        assert_eq!(warnings.len(), 1, "one load warning: {warnings:?}");
        let message = warnings[0].message();
        assert!(
            message.contains("stickout") && message.contains("cutting_length"),
            "the warning names both fields: {message}"
        );

        // The project finding.
        let findings = |tool: ToolConfig| {
            let mut builder = crate::session::ProjectSessionBuilder::new();
            let _ = builder.add_tool(tool);
            builder
                .build()
                .diagnose_project()
                .into_iter()
                .filter(|d| d.id.as_str() == ID)
                .collect::<Vec<_>>()
        };
        let found = findings(short.clone());
        assert_eq!(found.len(), 1, "one finding per short tool");
        assert_eq!(found[0].severity, crate::diagnostics::Severity::Caution);
        assert!(
            found[0].message.contains("stickout") && found[0].message.contains("cutting_length"),
            "{}",
            found[0].message
        );

        // Equal lengths and a zero stick-out: no finding, no refusal.
        let mut equal = short.clone();
        equal.stickout = ToolFact::Stated(25.0);
        assert!(findings(equal).is_empty(), "stick-out = flute is not short");
        let mut zero = short;
        zero.stickout = ToolFact::Stated(0.0);
        assert!(zero.validate().is_ok(), "a zero stick-out loads as before");
        assert!(findings(zero).is_empty(), "a zero stick-out is not stated");
    }

    /// Parity freeze (architectural refactor §7.2): the snake_case serde
    /// repr of every [`ToolType`] — canonical in project TOML, the MCP
    /// wire format, MCP error messages, and the viz legacy tool-type
    /// maps. Pinned as literals so a rename anywhere fails here first.
    #[test]
    fn tool_type_serde_repr_pinned() {
        const PINNED: &[(&str, ToolType)] = &[
            ("end_mill", ToolType::EndMill),
            ("ball_nose", ToolType::BallNose),
            ("bull_nose", ToolType::BullNose),
            ("v_bit", ToolType::VBit),
            ("tapered_ball_nose", ToolType::TaperedBallNose),
        ];
        assert_eq!(PINNED.len(), ToolType::ALL.len());

        for &(repr, tool_type) in PINNED {
            assert_eq!(
                serde_json::to_value(tool_type).expect("serialize tool type"),
                serde_json::Value::String(repr.to_owned()),
                "{tool_type:?}: serde repr drifted from the pinned canonical name"
            );
            let parsed: ToolType =
                serde_json::from_value(serde_json::Value::String(repr.to_owned()))
                    .expect("canonical name must deserialize");
            assert_eq!(parsed, tool_type);
            // The const-context mirror must agree with serde exactly.
            assert_eq!(
                tool_type.serde_token(),
                repr,
                "{tool_type:?}: serde_token() drifted from the serde repr"
            );
        }
    }

    /// T8 + L9: the vocabulary, pinned exactly — the canonical serde
    /// token of each type, case folded. L9 deleted the eight historical
    /// aliases, so every one of them must now read as unknown. Unknown
    /// input is `None`; each surface applies its own Q4 policy on top.
    #[test]
    fn parse_lenient_vocabulary_is_pinned() {
        let table: &[(&str, ToolType)] = &[
            ("end_mill", ToolType::EndMill),
            ("ball_nose", ToolType::BallNose),
            ("bull_nose", ToolType::BullNose),
            ("v_bit", ToolType::VBit),
            ("tapered_ball_nose", ToolType::TaperedBallNose),
        ];
        for &(token, expected) in table {
            assert_eq!(ToolType::parse_lenient(token), Some(expected), "{token}");
            // Case-insensitive.
            assert_eq!(
                ToolType::parse_lenient(&token.to_ascii_uppercase()),
                Some(expected),
                "{token} (uppercase)"
            );
        }
        // Round trip: every canonical token parses to its own type.
        for &tool_type in ToolType::ALL {
            assert_eq!(
                ToolType::parse_lenient(tool_type.serde_token()),
                Some(tool_type)
            );
        }
        // L9: every deleted alias reads as unknown. A caller that sends
        // one now hears so, instead of getting a tool it did not name.
        for alias in [
            "endmill",
            "flat",
            "ballnose",
            "ball",
            "bullnose",
            "vbit",
            "taperedballnose",
            "tapered_ball",
        ] {
            assert_eq!(ToolType::parse_lenient(alias), None, "{alias}");
        }
        // Unknown stays None — the default-EndMill policy is the
        // caller's, not the parser's.
        assert_eq!(ToolType::parse_lenient("definitely_not_a_tool"), None);
        assert_eq!(ToolType::parse_lenient(""), None);
    }

    #[test]
    fn youngs_modulus_matches_canonical_values() {
        assert_eq!(ToolMaterial::Carbide.youngs_modulus_n_per_mm2(), 600_000.0);
        assert_eq!(ToolMaterial::Hss.youngs_modulus_n_per_mm2(), 200_000.0);
        // Carbide is meaningfully stiffer than HSS.
        assert!(
            ToolMaterial::Carbide.youngs_modulus_n_per_mm2()
                > 2.5 * ToolMaterial::Hss.youngs_modulus_n_per_mm2()
        );
    }

    #[test]
    fn toml_round_trip_preserves_helix_corner_radius_and_material() {
        let mut tool = ToolConfig::new_default(ToolId::from_raw(7), ToolType::EndMill);
        tool.helix_deg = 45.0;
        tool.corner_radius_mm = 0.1;
        tool.tool_material = ToolMaterial::Hss;
        let toml = toml::to_string(&tool).expect("serialize tool config");
        let decoded: ToolConfig = toml::from_str(&toml).expect("deserialize tool config");
        assert_eq!(decoded.helix_deg, 45.0);
        assert_eq!(decoded.corner_radius_mm, 0.1);
        assert_eq!(decoded.tool_material, ToolMaterial::Hss);
    }

    #[test]
    fn old_toml_defaults_new_tool_fields() {
        let decoded: ToolConfig = toml::from_str(
            r#"
id = 0
name = "Old"
tool_number = 1
tool_type = "end_mill"
diameter = 6.0
cutting_length = { stated = 20.0 }
corner_radius = 2.0
included_angle = 90.0
taper_half_angle = { stated = 15.0 }
shaft_diameter = 6.0
shank_diameter = { stated = 6.0 }
stickout = { stated = 40.0 }
flute_count = { stated = 2 }
tool_material = "carbide"
cut_direction = "up_cut"
vendor = ""
product_id = ""
"#,
        )
        .expect("old tool config deserializes");
        assert_eq!(decoded.helix_deg, 30.0);
        assert_eq!(decoded.corner_radius_mm, 0.0);
    }

    /// A tool carrying every geometry field at once — the shape a
    /// project saved before 4b1e6f21 has on disk, whatever its type.
    fn fully_populated(tool_type: ToolType) -> ToolConfig {
        let mut tool = ToolConfig::new_default(ToolId::from_raw(0), tool_type);
        tool.corner_radius_mm = 0.5;
        tool.corner_radius = 2.0;
        tool.included_angle = 90.0;
        tool.taper_half_angle = ToolFact::Stated(15.0);
        tool.shaft_diameter = 6.35;
        tool
    }

    /// The type dispatch decides which geometry is real. Normalisation
    /// keeps exactly the owner's fields and zeroes the rest, for every
    /// type — so a flat end mill stops publishing a bull-nose radius.
    #[test]
    fn normalize_keeps_only_the_owning_types_geometry() {
        for &tool_type in ToolType::ALL {
            let mut tool = fully_populated(tool_type);
            let cleared = tool.normalize_geometry();
            for field in ToolGeometryField::ALL {
                let owns = field.owner() == tool_type;
                let kept = field.value_of(&tool) != 0.0;
                assert_eq!(kept, owns, "{}: {tool_type:?}", field.field_name());
            }
            for c in &cleared {
                assert_ne!(c.field.owner(), tool_type);
                assert_ne!(c.was, 0.0, "cleared values are reported, not invented");
            }
            // Idempotent: a second pass has nothing left to clear.
            assert!(tool.normalize_geometry().is_empty());
        }
    }

    /// The GUI/CLI constructor is the surviving source of the
    /// type-agnostic defaults 4b1e6f21 fixed on the MCP path. Every
    /// default tool must now be complete for its type and carry
    /// nothing belonging to another.
    #[test]
    fn new_default_is_complete_and_carries_no_foreign_geometry() {
        for &tool_type in ToolType::ALL {
            let tool = ToolConfig::new_default(ToolId::from_raw(3), tool_type);
            let foreign = tool.cross_type_geometry();
            assert!(foreign.is_empty(), "{tool_type:?} carries {foreign:?}");
            let missing = tool.missing_defining_geometry();
            assert!(missing.is_empty(), "{tool_type:?} missing {missing:?}");
        }
        // The specific leak: `Session::list_tools` publishes
        // `corner_radius_mm.max(corner_radius)`, which used to report a
        // 2 mm corner radius for a plain flat end mill.
        let end_mill = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
        assert_eq!(end_mill.corner_radius_mm.max(end_mill.corner_radius), 0.0);
        // …while the types that are DEFINED by one of those numbers
        // keep it: a default V-bit is still a 90° V-bit.
        let vbit = ToolConfig::new_default(ToolId::from_raw(0), ToolType::VBit);
        assert_eq!(vbit.included_angle, 90.0);
        let bull = ToolConfig::new_default(ToolId::from_raw(0), ToolType::BullNose);
        assert_eq!(bull.corner_radius, 2.0);
        let taper = ToolConfig::new_default(ToolId::from_raw(0), ToolType::TaperedBallNose);
        assert_eq!(taper.taper_half_angle_planning_deg(), 15.0);
        assert_eq!(taper.shaft_diameter, 6.35);
    }

    /// `VBitEndmill::new` refuses an included angle outside its domain, so a
    /// zeroed defining field builds no cutter, not a soft default — the one
    /// case a caller that flips `tool_type` in place has to check.
    #[test]
    fn missing_defining_geometry_names_the_zeroed_field() {
        let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
        tool.tool_type = ToolType::VBit;
        let missing = tool.missing_defining_geometry();
        assert_eq!(missing, vec![ToolGeometryField::IncludedAngle]);
        // A flat end mill with no corner radius is an ordinary flat end
        // mill, so `corner_radius_mm` is never reported missing.
        let plain = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
        assert!(plain.missing_defining_geometry().is_empty());
    }

    /// THE motivating fixture (live project, 2026-08-21). The name says
    /// a 2 mm tip; the geometry is `diameter: 1.0`, which for a tapered
    /// ball IS the ball diameter — the tool is half its named size.
    /// Exactly one mismatch, and it names both numbers.
    #[test]
    fn name_says_two_mm_tip_geometry_says_one() {
        let mut tool = ToolConfig::new_default(ToolId::from_raw(2), ToolType::TaperedBallNose);
        tool.name = "Tapered Ball 2mm tip / 7° / 6mm shank".to_owned();
        tool.diameter = 1.0;
        tool.taper_half_angle = ToolFact::Stated(7.1);
        tool.shaft_diameter = 6.0;
        tool.shank_diameter = ToolFact::Stated(6.0);
        tool.cutting_length = ToolFact::Stated(20.0);

        let found = tool.name_geometry_mismatches();
        assert_eq!(found.len(), 1, "expected one mismatch, got {found:?}");
        let m = &found[0];
        assert_eq!(m.quantity, NamedQuantity::TipDiameter);
        assert_eq!(m.name_value, 2.0);
        assert_eq!(m.config_value, 1.0);
        assert_eq!(m.field, "diameter");
        // Both numbers have to reach the reader: the operator sized a
        // whole finishing strategy off the name.
        let msg = m.message();
        assert!(msg.contains("2.000"), "{msg}");
        assert!(msg.contains("1.000"), "{msg}");
    }

    /// An `R` prefix names a radius, so "R1.0" on a Ø1.0 tapered ball
    /// is the same factor-of-two defect written the vendor's way.
    #[test]
    fn radius_prefixed_name_catches_the_same_factor_of_two() {
        let mut tool = ToolConfig::new_default(ToolId::from_raw(2), ToolType::TaperedBallNose);
        tool.name = "R1.0mm x 6mm x 20mm 2F Tapered Ball".to_owned();
        tool.diameter = 1.0;
        tool.shaft_diameter = 6.0;
        tool.shank_diameter = ToolFact::Stated(6.0);
        tool.cutting_length = ToolFact::Stated(20.0);

        let found = tool.name_geometry_mismatches();
        assert_eq!(found.len(), 1, "expected one mismatch, got {found:?}");
        assert_eq!(found[0].quantity, NamedQuantity::TipRadius);
        assert_eq!(found[0].name_value, 1.0);
        assert_eq!(found[0].config_value, 0.5);
    }

    /// The check must stay quiet on names it cannot read confidently —
    /// a false alarm on every legitimately-odd name would train the
    /// reader to ignore the one that matters.
    #[test]
    fn quiet_on_names_this_parse_declines_to_read() {
        let quiet: &[(&str, ToolType, f64)] = &[
            // Imperial: 1/4" converts to 6.35 mm and agrees.
            ("1/4\" 2F Compression", ToolType::EndMill, 6.35),
            // A 1" name with the millimetre figure beside it (wanaka).
            ("1\" (25.4mm)", ToolType::EndMill, 25.4),
            // A Ø figure that is not first may name the shank.
            (
                "R1.0mm x \u{00D8}6 shank Tapered Ball",
                ToolType::TaperedBallNose,
                2.0,
            ),
            // Carbide-style catalogue name: the "#201" is a product code
            // and the 1/4" is the tool's own size.
            ("#201 1/4\" Square", ToolType::EndMill, 6.35),
            // Product codes and flute counts are not dimensions.
            ("Amana 46200 2F", ToolType::EndMill, 3.175),
            // Empty names claim nothing.
            ("", ToolType::EndMill, 6.0),
            // Several unkeyworded figures: which one is "the" diameter
            // is a guess, so none is judged.
            ("6mm shank x 22mm LOC ball", ToolType::BallNose, 3.0),
        ];
        for &(name, tool_type, diameter) in quiet {
            let mut tool = ToolConfig::new_default(ToolId::from_raw(0), tool_type);
            tool.name = name.to_owned();
            tool.diameter = diameter;
            let found = tool.name_geometry_mismatches();
            assert!(found.is_empty(), "{name}: unexpected {found:?}");
        }
    }

    /// A lone unkeyworded millimetre figure IS the size, and is judged.
    #[test]
    fn lone_millimetre_figure_is_judged_against_the_diameter() {
        let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
        tool.name = "6mm 2F Carbide End Mill".to_owned();
        tool.diameter = 3.175;
        tool.cutting_length = ToolFact::Stated(25.0);
        tool.shank_diameter = ToolFact::Stated(3.175);

        let found = tool.name_geometry_mismatches();
        assert_eq!(found.len(), 1, "expected one mismatch, got {found:?}");
        assert_eq!(found[0].quantity, NamedQuantity::TipDiameter);
        assert_eq!(found[0].name_value, 6.0);
        assert_eq!(found[0].config_value, 3.175);
    }

    /// Degrees are only judged where an angle DEFINES the geometry. On
    /// a V-bit a wrong angle is a different tool; on an end mill a
    /// degree figure is as likely to be the helix, so it is ignored.
    #[test]
    fn degrees_judged_only_where_an_angle_defines_the_tool() {
        let mut vbit = ToolConfig::new_default(ToolId::from_raw(0), ToolType::VBit);
        vbit.name = "60 degree V-Bit".to_owned();
        vbit.diameter = 12.7;
        let found = vbit.name_geometry_mismatches();
        assert_eq!(found.len(), 1, "expected one mismatch, got {found:?}");
        assert_eq!(found[0].quantity, NamedQuantity::IncludedAngle);
        assert_eq!(found[0].name_value, 60.0);
        assert_eq!(found[0].config_value, 90.0);

        // Same figure, a type where no angle defines the geometry.
        let mut end_mill = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
        end_mill.name = "60° something".to_owned();
        assert!(end_mill.name_geometry_mismatches().is_empty());

        // A tapered ball quoted as the full included cone (2 × the
        // half-angle) agrees — vendors write it both ways.
        let mut taper = ToolConfig::new_default(ToolId::from_raw(0), ToolType::TaperedBallNose);
        taper.name = "Tapered Ball 14.2 deg".to_owned();
        taper.taper_half_angle = ToolFact::Stated(7.1);
        assert!(taper.name_geometry_mismatches().is_empty());
    }

    /// Role words bind only inside their own phrase, and only when the
    /// phrase holds exactly one of each — otherwise nothing binds.
    #[test]
    fn role_words_bind_within_their_phrase() {
        let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::TaperedBallNose);
        tool.name = "Tapered Ball 1mm tip / 7° / 3mm shank".to_owned();
        tool.diameter = 1.0;
        tool.taper_half_angle = ToolFact::Stated(7.0);
        tool.shaft_diameter = 6.0;
        tool.shank_diameter = ToolFact::Stated(6.0);

        let found = tool.name_geometry_mismatches();
        assert_eq!(found.len(), 1, "expected one mismatch, got {found:?}");
        assert_eq!(found[0].quantity, NamedQuantity::ShankDiameter);
        assert_eq!(found[0].name_value, 3.0);
        assert_eq!(found[0].config_value, 6.0);
    }

    /// One convention on every surface: `Ø` prefixes a diameter, `R` a
    /// radius, and a tapered ball says "tip".
    #[test]
    fn size_label_per_tool_type() {
        let mut end_mill = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
        end_mill.diameter = 6.0;
        assert_eq!(end_mill.size_label(), "\u{00D8}6.00 mm");
        end_mill.corner_radius_mm = 0.5;
        assert_eq!(end_mill.size_label(), "\u{00D8}6.00 mm, corner R0.50");

        let mut ball = ToolConfig::new_default(ToolId::from_raw(0), ToolType::BallNose);
        ball.diameter = 6.0;
        assert_eq!(ball.size_label(), "\u{00D8}6.00 mm (R3.00)");

        let mut taper = ToolConfig::new_default(ToolId::from_raw(0), ToolType::TaperedBallNose);
        taper.diameter = 2.0;
        taper.shank_diameter = ToolFact::Stated(6.0);
        taper.shaft_diameter = 6.0;
        taper.taper_half_angle = ToolFact::Stated(5.7);
        assert_eq!(
            taper.size_label(),
            "tip \u{00D8}2.00 mm (R1.00), shank \u{00D8}6.00"
        );
        assert_eq!(
            taper.summary(),
            "Tapered Ball Nose tip \u{00D8}2.00 mm (R1.00), shank \u{00D8}6.00, 5.7\u{00B0} half-angle"
        );
        assert_eq!(taper.diameter_field_label(), "Tip \u{00D8}:");
        assert_eq!(
            taper.short_label(),
            "Tapered ball R1.00 \u{00B7} 5.7\u{00B0}"
        );
        assert_eq!(taper.derived_tip_radius_text().as_deref(), Some("= R1.00"));

        let mut bull = ToolConfig::new_default(ToolId::from_raw(0), ToolType::BullNose);
        bull.diameter = 6.0;
        bull.corner_radius = 1.0;
        assert_eq!(bull.size_label(), "\u{00D8}6.00 mm, corner R1.00");
        assert_eq!(bull.derived_tip_radius_text(), None);

        let mut vbit = ToolConfig::new_default(ToolId::from_raw(0), ToolType::VBit);
        vbit.diameter = 12.7;
        vbit.included_angle = 60.0;
        assert_eq!(vbit.size_label(), "\u{00D8}12.70 mm, 60\u{00B0}");
        assert_eq!(vbit.summary(), "V-Bit \u{00D8}12.70 mm, 60\u{00B0}");
        assert_eq!(vbit.diameter_field_label(), "Diameter \u{00D8}:");
        assert_eq!(vbit.short_label(), "V-bit \u{00D8}12.70 60\u{00B0}");
        assert_eq!(bull.short_label(), "Bull nose \u{00D8}6.00 R1.00");
        assert_eq!(ball.short_label(), "Ball nose \u{00D8}6.00");
        assert_eq!(end_mill.short_label(), "End mill \u{00D8}6.00 R0.50");
    }

    /// The tokens the parse reads, each checked against a tool whose
    /// geometry disagrees by a factor of two, and against one that
    /// agrees.
    #[test]
    fn tip_size_tokens_are_read() {
        let cases: &[(&str, ToolType, f64, f64)] = &[
            // (name, type, agreeing diameter, disagreeing diameter)
            (
                "R1.0mm x 6mm x 20mm 2F Tapered Ball",
                ToolType::TaperedBallNose,
                2.0,
                1.0,
            ),
            ("R1 Ball", ToolType::BallNose, 2.0, 4.0),
            ("\u{00D8}6 2F Upcut", ToolType::EndMill, 6.0, 3.0),
            ("D6 Flat", ToolType::EndMill, 6.0, 3.0),
            (
                "Tapered Ball 2mm tip / 7\u{00B0} / 6mm shank",
                ToolType::TaperedBallNose,
                2.0,
                1.0,
            ),
            ("1/4\" Downcut", ToolType::EndMill, 6.35, 3.175),
            ("1/8in Ball", ToolType::BallNose, 3.175, 6.35),
            ("0.25in End Mill", ToolType::EndMill, 6.35, 3.175),
            ("6mm 2F Carbide End Mill", ToolType::EndMill, 6.0, 3.0),
        ];
        for &(name, tool_type, agrees, disagrees) in cases {
            let mut tool = ToolConfig::new_default(ToolId::from_raw(0), tool_type);
            tool.name = name.to_owned();
            tool.diameter = agrees;
            tool.shank_diameter = ToolFact::Stated(6.0);
            tool.shaft_diameter = 6.0_f64.max(agrees);
            tool.cutting_length = ToolFact::Stated(20.0);
            tool.stickout = ToolFact::Stated(35.0);
            assert!(
                tool.name_tip_size_mismatch().is_none(),
                "{name}: agreeing geometry raised {:?}",
                tool.name_tip_size_mismatch()
            );
            // The disagreeing tool is really that size, shank included:
            // a lone figure that matches the shank is not a mismatch.
            tool.diameter = disagrees;
            tool.shank_diameter = ToolFact::Stated(disagrees);
            let m = tool.name_tip_size_mismatch();
            assert!(m.is_some(), "{name}: Ø{disagrees} raised nothing");
            assert!(
                m.and_then(|m| m.caution_line()).is_some(),
                "{name}: no caution line"
            );
        }
    }

    /// The caution names the name's figure and the tool's figure in the
    /// `Ø`/`R` convention.
    #[test]
    fn caution_line_names_both_sizes() {
        let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::TaperedBallNose);
        tool.name = "R1.0mm x 6mm x 20mm 2F Tapered Ball".to_owned();
        tool.diameter = 1.0;
        tool.shaft_diameter = 6.0;
        tool.shank_diameter = ToolFact::Stated(6.0);
        tool.cutting_length = ToolFact::Stated(20.0);
        let line = tool
            .name_tip_size_mismatch()
            .and_then(|m| m.caution_line())
            .expect("a caution");
        assert_eq!(
            line,
            "The name says R1.00 (\u{00D8}2.00 mm) but the tool is \u{00D8}1.00 mm. Check the diameter."
        );

        tool.name = "Tapered Ball 2mm tip / 7\u{00B0} / 6mm shank".to_owned();
        let line = tool
            .name_tip_size_mismatch()
            .and_then(|m| m.caution_line())
            .expect("a caution");
        assert_eq!(
            line,
            "The name says \u{00D8}2.00 mm but the tool is \u{00D8}1.00 mm. Check the diameter."
        );
    }

    /// Two tolerances, one per kind of claim. A TIP claim is judged at
    /// 2 %: the operator ruled the unit convention (2026-09-24), so a
    /// tool is sold in ONE unit and "1/4\"" (6.35) on a Ø6.00 tool is a
    /// real mismatch, not a rounding. Shank, flute length and angle
    /// claims keep 10 %, loose enough for "6mm shank" on a 6.35 collet
    /// shank and for 7° written for 7.1°. Neither passes a factor of two.
    #[test]
    fn tolerance_covers_rounding_not_factors() {
        assert!(!approx_matches_within(6.0, 6.35, NAME_TIP_REL_TOL));
        assert!(approx_matches_within(6.0, 6.05, NAME_TIP_REL_TOL));
        assert!(!approx_matches_within(2.0, 1.0, NAME_TIP_REL_TOL));
        assert!(approx_matches(6.0, 6.35));
        assert!(approx_matches(7.0, 7.1));
        assert!(!approx_matches(2.0, 1.0));
        assert!(!approx_matches(60.0, 90.0));
        // A zero field is unset, not contradicted.
        assert!(approx_matches(0.0, 0.0));
    }

    /// The unit rule: a lone size in one unit system on a tool whose
    /// diameter is a size of the other system warns, even when the
    /// figure matches the shank. The caution names both figures, each
    /// in its own unit.
    #[test]
    fn a_unit_crossing_warns_and_names_both_units() {
        // 1/4" name, Ø6.00 tool on a 1/4" (6.35) shank.
        let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
        tool.name = "1/4\" 2F Downcut".to_owned();
        tool.diameter = 6.0;
        tool.shank_diameter = ToolFact::Stated(6.35);
        let line = tool
            .name_tip_size_mismatch()
            .and_then(|m| m.caution_line())
            .expect("1/4\" on \u{00D8}6.00 warns");
        assert_eq!(
            line,
            "The name says 1/4\" (6.35 mm) but the tool is \u{00D8}6.00 mm. Check the diameter."
        );

        // The reverse: "6mm" name on a Ø6.35 (1/4") tool.
        tool.name = "6mm 2F Carbide End Mill".to_owned();
        tool.diameter = 6.35;
        tool.shank_diameter = ToolFact::Stated(6.35);
        let line = tool
            .name_tip_size_mismatch()
            .and_then(|m| m.caution_line())
            .expect("6mm on \u{00D8}6.35 warns");
        assert_eq!(
            line,
            "The name says \u{00D8}6.00 mm but the tool is \u{00D8}1/4\" (6.35 mm). \
             Check the diameter."
        );

        // Same unit on both sides and the figure is the shank: quiet.
        tool.name = "1/4\" Upcut".to_owned();
        tool.diameter = 3.175;
        tool.shank_diameter = ToolFact::Stated(6.35);
        assert!(tool.name_tip_size_mismatch().is_none());
    }

    /// With no stored unit, an inch token in the name selects Imperial;
    /// anything else is Metric. A stored unit wins.
    #[test]
    fn size_units_are_inferred_from_the_name() {
        let imperial = [
            "1/4\" Downcut",
            "1/8in Ball",
            "0.25in End Mill",
            "#201 1/4\" Square",
            "1\" (25.4mm)",
            "1/16 inch tapered",
        ];
        for name in imperial {
            let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
            tool.name = name.to_owned();
            assert_eq!(tool.effective_size_units(), SizeUnits::Imperial, "{name}");
        }
        let metric = [
            "6mm 2F Carbide End Mill",
            "R1.0mm x 6mm x 20mm 2F Tapered Ball",
            "Tapered Ball 2mm tip / 7\u{00B0} / 6mm shank",
            "End Mill",
            "Amana 46200 2F",
            "",
        ];
        for name in metric {
            let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
            tool.name = name.to_owned();
            assert_eq!(tool.effective_size_units(), SizeUnits::Metric, "{name}");
        }
        let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
        tool.name = "1/4\" Downcut".to_owned();
        tool.size_units = Some(SizeUnits::Metric);
        assert_eq!(tool.effective_size_units(), SizeUnits::Metric);
    }

    /// Imperial labels: the inch figure first, mm in parentheses, radii
    /// in inches too.
    #[test]
    fn imperial_size_label_per_tool_type() {
        let imperial = |tool_type: ToolType, diameter: f64| {
            let mut tool = ToolConfig::new_default(ToolId::from_raw(0), tool_type);
            tool.diameter = diameter;
            tool.size_units = Some(SizeUnits::Imperial);
            tool
        };
        let end_mill = imperial(ToolType::EndMill, 6.35);
        assert_eq!(end_mill.size_label(), "\u{00D8}1/4\" (6.35 mm)");
        assert_eq!(
            imperial(ToolType::EndMill, 6.0).size_label(),
            "\u{00D8}0.236\" (6.00 mm)"
        );
        let mut ball = imperial(ToolType::BallNose, 6.35);
        assert_eq!(ball.size_label(), "\u{00D8}1/4\" (R1/8\", 6.35 mm)");
        assert_eq!(ball.derived_tip_radius_text().as_deref(), Some("= R1/8\""));
        ball.size_units = Some(SizeUnits::Metric);
        assert_eq!(ball.size_label(), "\u{00D8}6.35 mm (R3.17)");

        let mut taper = imperial(ToolType::TaperedBallNose, 1.5875);
        taper.shank_diameter = ToolFact::Stated(6.35);
        assert_eq!(
            taper.size_label(),
            "tip \u{00D8}1/16\" (R1/32\", 1.59 mm), shank \u{00D8}1/4\""
        );
        let mut bull = imperial(ToolType::BullNose, 12.7);
        bull.corner_radius = 1.5875;
        assert_eq!(
            bull.size_label(),
            "\u{00D8}1/2\" (12.70 mm), corner R1/16\""
        );
        let mut vbit = imperial(ToolType::VBit, 12.7);
        vbit.included_angle = 60.0;
        assert_eq!(vbit.size_label(), "\u{00D8}1/2\" (12.70 mm), 60\u{00B0}");

        // Inferred from the name, with no stored unit.
        let mut named = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
        named.name = "#201 1/4\" Square".to_owned();
        named.diameter = 6.35;
        assert_eq!(named.size_label(), "\u{00D8}1/4\" (6.35 mm)");
        assert!(named.name_tip_size_mismatch().is_none());
    }

    /// A saved tool without the field loads as `None` (infer), and a set
    /// field round-trips as its snake_case token.
    #[test]
    fn size_units_serde() {
        let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
        let json = serde_json::to_value(&tool).unwrap();
        assert!(json.get("size_units").is_none(), "None is not written");
        tool.size_units = Some(SizeUnits::Imperial);
        let json = serde_json::to_value(&tool).unwrap();
        assert_eq!(json["size_units"], "imperial");
        let back: ToolConfig = serde_json::from_value(json).unwrap();
        assert_eq!(back.size_units, Some(SizeUnits::Imperial));
    }

    #[test]
    fn non_centre_cutting_serde_and_limits() {
        let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
        // An 8 mm core needs a wider tool than the 6.35 mm default.
        tool.diameter = 18.0;
        tool.set_centre_cutting(ToolFact::Stated(CentreCutting::No(
            NonCentreCuttingGeometry {
                centre_core_diameter_mm: 8.0,
                blade_protrusion_mm: 1.0,
            },
        )));
        // P22-0c-2: the limits are the body core zone's.
        let core = crate::compute::cutter::build_cutter(&tool)
            .unwrap()
            .body()
            .core()
            .unwrap();
        assert_eq!(core.protrusion_mm, 1.0);
        assert!((core.max_descent_angle_deg() - 7.125_016_349).abs() < 1e-9);
        let json = serde_json::to_value(&tool).unwrap();
        assert_eq!(json["centre_cutting"]["stated"], "no");
        assert_eq!(json["centre_cutting"]["centre_core_diameter_mm"], 8.0);
        let back: ToolConfig = serde_json::from_value(json).unwrap();
        assert_eq!(back.centre_cutting_fact(), tool.centre_cutting_fact());
    }

    /// K3: only a `No(..)` fact gives limits; `Yes` and `Unknown` give none
    /// (an `Unknown` tool keeps the motion of a centre-cutting tool).
    #[test]
    fn only_a_no_fact_gives_non_centre_cutting_limits() {
        let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
        assert!(tool.centre_cutting_fact().is_unknown());
        assert!(tool.non_centre_cutting_geometry().is_none());
        tool.set_centre_cutting(ToolFact::Stated(CentreCutting::Yes));
        assert!(tool.non_centre_cutting_geometry().is_none());
        tool.set_centre_cutting(ToolFact::estimated(
            CentreCutting::No(NonCentreCuttingGeometry {
                centre_core_diameter_mm: 3.0,
                blade_protrusion_mm: 1.0,
            }),
            "test",
        ));
        assert!(tool.non_centre_cutting_geometry().is_some());
    }

    /// K3 (decision E4): the `u32` flute-count default equals the value of
    /// its F201 row, and each f64 default equals its row.
    #[test]
    fn each_legacy_default_equals_its_f201_row() {
        assert_eq!(
            f64::from(LEGACY_DEFAULT_FLUTE_COUNT),
            LEGACY_DEFAULT_FLUTE_COUNT_ROW.value
        );
        assert_eq!(
            LEGACY_DEFAULT_CUTTING_LENGTH_MM,
            LEGACY_DEFAULT_CUTTING_LENGTH_MM_ROW.value
        );
        assert_eq!(
            LEGACY_DEFAULT_TAPER_HALF_ANGLE_DEG,
            LEGACY_DEFAULT_TAPER_HALF_ANGLE_DEG_ROW.value
        );
        assert_eq!(
            LEGACY_DEFAULT_SHANK_DIAMETER_MM,
            LEGACY_DEFAULT_SHANK_DIAMETER_MM_ROW.value
        );
        assert_eq!(
            LEGACY_DEFAULT_STICKOUT_MM,
            LEGACY_DEFAULT_STICKOUT_MM_ROW.value
        );
    }

    /// K3: an `Unknown` fact plans at its legacy default; `new_default`
    /// writes `Estimated` with the basis `new_default`.
    #[test]
    fn an_unknown_fact_plans_at_the_legacy_default() {
        let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
        assert_eq!(tool.stickout_fact().kind(), FactKind::Estimated);
        tool.set_stickout(ToolFact::Unknown);
        tool.set_flute_count(ToolFact::Unknown);
        assert_eq!(tool.stickout_planning_mm(), LEGACY_DEFAULT_STICKOUT_MM);
        assert_eq!(tool.flute_count_planning(), LEGACY_DEFAULT_FLUTE_COUNT);
    }

    #[test]
    fn non_centre_cutting_domain_refuses_invalid_measurements() {
        let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
        tool.set_centre_cutting(ToolFact::Stated(CentreCutting::No(
            NonCentreCuttingGeometry {
                centre_core_diameter_mm: tool.diameter,
                blade_protrusion_mm: 0.0,
            },
        )));
        assert_eq!(
            tool.validate().unwrap_err().dimension,
            ToolDimension::CentreCoreDiameterMm
        );
        tool.set_centre_cutting(ToolFact::Stated(CentreCutting::No(
            NonCentreCuttingGeometry {
                centre_core_diameter_mm: 3.0,
                blade_protrusion_mm: -0.1,
            },
        )));
        assert_eq!(
            tool.validate().unwrap_err().dimension,
            ToolDimension::BladeProtrusionMm
        );
    }

    /// F118: the token of each key is its serde form, and `parse` reads
    /// only that token.
    #[test]
    fn tool_param_key_tokens_are_the_serde_form() {
        for key in ToolParamKey::ALL {
            assert_eq!(
                serde_json::to_value(key).unwrap(),
                serde_json::Value::String(key.token().to_owned()),
                "{key:?}"
            );
            assert_eq!(ToolParamKey::parse(key.token()), Some(*key));
            assert_eq!(ToolParamKey::parse(&key.token().to_ascii_uppercase()), None);
        }
        assert_eq!(ToolParamKey::parse("not_a_field"), None);
    }

    /// F118: the four fields the old string table could not set are
    /// written, and a value of the wrong JSON type refuses.
    #[test]
    fn tool_param_key_writes_the_fields_the_old_table_missed() {
        let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
        ToolParamKey::HelixDeg
            .write(&mut tool, &serde_json::json!(40.0))
            .unwrap();
        ToolParamKey::CornerRadiusMm
            .write(&mut tool, &serde_json::json!(0.5))
            .unwrap();
        ToolParamKey::ToolMaterial
            .write(&mut tool, &serde_json::json!("hss"))
            .unwrap();
        ToolParamKey::CutDirection
            .write(&mut tool, &serde_json::json!("down_cut"))
            .unwrap();
        assert!((tool.helix_deg - 40.0).abs() < 1e-12);
        assert!((tool.corner_radius_mm - 0.5).abs() < 1e-12);
        assert_eq!(tool.tool_material, ToolMaterial::Hss);
        assert_eq!(tool.cut_direction, BitCutDirection::DownCut);
        assert!(
            ToolParamKey::ToolMaterial
                .write(&mut tool, &serde_json::json!("steel"))
                .is_err()
        );
        assert!(
            ToolParamKey::FluteCount
                .write(&mut tool, &serde_json::json!(2.5))
                .is_err()
        );
        assert!(!ToolParamKey::SizeUnits.changes_the_cut());
        assert!(ToolParamKey::HelixDeg.changes_the_cut());
    }

    #[test]
    fn non_centre_cutting_param_is_atomic() {
        let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
        tool.set_centre_cutting(ToolFact::Stated(CentreCutting::No(
            NonCentreCuttingGeometry {
                centre_core_diameter_mm: 3.0,
                blade_protrusion_mm: 1.0,
            },
        )));
        ToolParamKey::CentreCutting
            .write(&mut tool, &serde_json::Value::Null)
            .unwrap();
        assert!(tool.centre_cutting_fact().is_unknown());
        ToolParamKey::CentreCutting
            .write(&mut tool, &serde_json::json!("yes"))
            .unwrap();
        assert_eq!(
            tool.centre_cutting_fact(),
            &ToolFact::Stated(CentreCutting::Yes)
        );
        ToolParamKey::CentreCutting
            .write(
                &mut tool,
                &serde_json::json!({"centre_core_diameter_mm": 3.0, "blade_protrusion_mm": 1.0}),
            )
            .unwrap();
        assert!(tool.non_centre_cutting_geometry().is_some());
    }
}
