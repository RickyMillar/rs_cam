use serde::{Deserialize, Serialize};

/// Unique identifier for a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolId(pub usize);

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
    pub const ALL: &[ToolType] = &[
        ToolType::EndMill,
        ToolType::BallNose,
        ToolType::BullNose,
        ToolType::VBit,
        ToolType::TaperedBallNose,
    ];

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
    /// Unifies the four historically divergent vocabularies — core
    /// project loader (`ballnose`, wildcard→EndMill), viz legacy loader
    /// (`ball`/`tapered_ball`, wildcard→EndMill), MCP (canonical
    /// snake_case only, `Err` on unknown), viz serde-direct (canonical
    /// only, whole-file parse error on unknown) — into one
    /// case-insensitive union. Pre-T8 the vocabularies disagreed:
    /// `"ball"` parsed to `BallNose` in the viz legacy loader but
    /// silently became `EndMill` in core.
    ///
    /// Returns `None` for unrecognized input. Policy at the call sites
    /// (Q4): file-loading surfaces warn and default to `EndMill`; the
    /// MCP mutation surface returns an explicit error (an interactive
    /// caller should hear "unknown type", not get a surprise end mill).
    pub fn parse_lenient(s: &str) -> Option<ToolType> {
        match s.to_ascii_lowercase().as_str() {
            // "flat" is the legacy save format's EndMill token
            // (pre-TOML-rework writer, commit 978dc9c).
            "end_mill" | "endmill" | "flat" => Some(ToolType::EndMill),
            "ball_nose" | "ballnose" | "ball" => Some(ToolType::BallNose),
            "bull_nose" | "bullnose" => Some(ToolType::BullNose),
            "v_bit" | "vbit" => Some(ToolType::VBit),
            "tapered_ball_nose" | "taperedballnose" | "tapered_ball" => {
                Some(ToolType::TaperedBallNose)
            }
            _ => None,
        }
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
pub const CARBIDE_YOUNGS_MODULUS_N_PER_MM2: f64 = 600_000.0;
/// Young's modulus of high-speed steel (N/mm² == MPa). Handbook value
/// is 200–210 GPa for M2/M42 grades; 200 chosen as a clean reference.
pub const HSS_YOUNGS_MODULUS_N_PER_MM2: f64 = 200_000.0;

impl ToolMaterial {
    pub const ALL: &[ToolMaterial] = &[ToolMaterial::Carbide, ToolMaterial::Hss];

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

/// Cut direction (affects chip evacuation and surface quality).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BitCutDirection {
    UpCut,
    DownCut,
    Compression,
}

impl BitCutDirection {
    pub const ALL: &[BitCutDirection] = &[
        BitCutDirection::UpCut,
        BitCutDirection::DownCut,
        BitCutDirection::Compression,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            BitCutDirection::UpCut => "Up Cut",
            BitCutDirection::DownCut => "Down Cut",
            BitCutDirection::Compression => "Compression",
        }
    }
}

/// Complete tool configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolConfig {
    pub id: ToolId,
    pub name: String,
    /// G-code tool number used for M6 output.
    pub tool_number: u32,
    pub tool_type: ToolType,
    pub diameter: f64,
    pub cutting_length: f64,
    #[serde(default = "default_helix_deg")]
    pub helix_deg: f64,
    #[serde(default)]
    pub corner_radius_mm: f64,
    // Bull Nose
    pub corner_radius: f64,
    // V-Bit (included angle in degrees)
    pub included_angle: f64,
    // Tapered Ball Nose (half-angle in degrees)
    pub taper_half_angle: f64,
    pub shaft_diameter: f64,
    // Holder / collision detection
    pub holder_diameter: f64,
    pub shank_diameter: f64,
    pub shank_length: f64,
    pub stickout: f64,
    // Cutting parameters (for feeds calculation)
    pub flute_count: u32,
    pub tool_material: ToolMaterial,
    pub cut_direction: BitCutDirection,
    // Optional vendor info
    pub vendor: String,
    pub product_id: String,
}

fn default_helix_deg() -> f64 {
    30.0
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
            tool_number: id.0 as u32 + 1,
            tool_type,
            diameter,
            cutting_length: 25.0,
            helix_deg: default_helix_deg(),
            corner_radius_mm: 0.0,
            corner_radius: 2.0,
            included_angle: 90.0,
            taper_half_angle: 15.0,
            shaft_diameter: 6.35,
            holder_diameter: 25.0,
            shank_diameter: 6.35,
            shank_length: 20.0,
            stickout: 45.0,
            flute_count: 2,
            tool_material: ToolMaterial::Carbide,
            cut_direction: BitCutDirection::UpCut,
            vendor: String::new(),
            product_id: String::new(),
        };
        // The block above is the canonical geometry of each type, but
        // it was written type-AGNOSTICALLY: every tool got corner
        // radius 2.0, included angle 90 and taper half-angle 15
        // whatever its type, and those numbers then survived save,
        // load, and every report surface that does not dispatch on
        // `tool_type` first. Keep only what this type actually
        // defines — b0362626 made the MCP creation path do the same,
        // and this is the constructor the GUI and CLI use.
        tool.normalize_geometry();
        tool
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
            _ => self.diameter,
        }
    }

    /// Short description for the project tree.
    pub fn summary(&self) -> String {
        match self.tool_type {
            ToolType::EndMill | ToolType::BallNose => {
                format!("{:.2}mm {}", self.diameter, self.tool_type.label())
            }
            ToolType::BullNose => {
                format!(
                    "{:.2}mm {} (r={:.1})",
                    self.diameter,
                    self.tool_type.label(),
                    self.corner_radius
                )
            }
            ToolType::VBit => {
                format!("{:.0}deg {}", self.included_angle, self.tool_type.label())
            }
            ToolType::TaperedBallNose => {
                format!(
                    "{:.2}mm {} ({:.0}deg)",
                    self.diameter,
                    self.tool_type.label(),
                    self.taper_half_angle
                )
            }
        }
    }

    /// Cross-type geometry this tool is carrying: fields owned by a
    /// DIFFERENT [`ToolType`] that still hold a non-zero value. Read-only
    /// twin of [`ToolConfig::normalize_geometry`].
    pub fn cross_type_geometry(&self) -> Vec<ClearedGeometry> {
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
    /// b0362626's rule, applied here to tools that were already saved
    /// before that commit landed.
    ///
    /// This changes no cutting geometry. Every consumer dispatches on
    /// `tool_type` (or on the `CutterKind` derived from it) before it
    /// reads any of these fields, so on a type that does not own the
    /// field the value was never read: `compute::cutter::build_cutter`
    /// (the only builder of a `MillingCutter` from a `ToolConfig`),
    /// `ToolConfig::envelope_diameter`, `execute::vbit_half_angle`,
    /// `feeds::predict::core_diameter_mm`, and the viz wireframe
    /// builder `render::sim_render::ToolGeometry::from_tool_config`.
    ///
    /// The two consumers that read them type-agnostically are report
    /// surfaces, and they are the reason this exists: `Session::list_tools`
    /// publishes `corner_radius_mm.max(corner_radius)`, so a flat end
    /// mill was reporting a 2 mm corner radius on the MCP wire, and
    /// `tool_library::dedupe_key` folds all three angle fields into its
    /// key, so two identical end mills differing only in dead bull-nose
    /// radius counted as distinct tools.
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
    /// constructors say so with `assert!`: `VBitEndmill::new` requires
    /// `0 < included_angle < 180`, `TaperedBallEndmill::new` requires
    /// `0 < taper_half_angle < 90` AND `shaft_diameter >= diameter`. A
    /// zero in one of those is a PANIC inside `build_cutter`, not a
    /// soft default. Callers that mutate `tool_type` in place (the
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
    /// - imperial names ("1/4in", "1/8\"") — no unit conversion is
    ///   attempted, so an inch figure is never compared;
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
    pub fn name_geometry_mismatches(&self) -> Vec<NameGeometryMismatch> {
        let numbers = parse_name_numbers(&self.name);
        let mut out: Vec<NameGeometryMismatch> = Vec::new();

        for n in &numbers {
            let Some(claim) = self.name_claim(n) else {
                continue;
            };
            push_mismatch(&mut out, claim.against(n.value));
        }

        // Unkeyworded millimetre figures. Judged only when the name
        // carries exactly ONE — then it is "the size".
        let bare: Vec<&NameNumber> = numbers
            .iter()
            .filter(|n| n.unit == NameUnit::Mm && n.role.is_none() && !n.radius_prefixed)
            .collect();
        if let [only] = bare.as_slice() {
            let claim = self.bare_length_claim();
            push_mismatch(&mut out, claim.against(only.value));
        }

        out
    }

    /// What a single figure from the name would be claiming about this
    /// tool — or `None` where this parse declines to read it.
    fn name_claim(&self, n: &NameNumber) -> Option<NameClaim> {
        let tip_radius = self.diameter / 2.0;
        if n.radius_prefixed {
            // An `R` prefix names a RADIUS, whatever unit follows it.
            let mut candidates = vec![tip_radius];
            match self.tool_type {
                ToolType::BullNose => candidates.push(self.corner_radius),
                ToolType::EndMill => candidates.push(self.corner_radius_mm),
                _ => {}
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
        match n.role? {
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
                let mut candidates = vec![self.shank_diameter];
                if self.tool_type == ToolType::TaperedBallNose {
                    candidates.push(self.shaft_diameter);
                }
                Some(NameClaim {
                    quantity: NamedQuantity::ShankDiameter,
                    candidates,
                    reported: self.shank_diameter,
                    field: "shank_diameter",
                })
            }
            NameRole::CuttingLength => Some(NameClaim {
                quantity: NamedQuantity::CuttingLength,
                candidates: vec![self.cutting_length],
                reported: self.cutting_length,
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
                    self.taper_half_angle,
                    self.taper_half_angle * 2.0,
                    self.helix_deg,
                ],
                reported: self.taper_half_angle,
                field: "taper_half_angle",
            }),
            _ => None,
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
            self.cutting_length,
            self.shank_diameter,
            self.shank_length,
            self.stickout,
            self.holder_diameter,
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
    pub const ALL: &[ToolGeometryField] = &[
        ToolGeometryField::CornerRadiusMm,
        ToolGeometryField::CornerRadius,
        ToolGeometryField::IncludedAngle,
        ToolGeometryField::TaperHalfAngle,
        ToolGeometryField::ShaftDiameter,
    ];

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
    pub const fn defines_owner(self) -> bool {
        !matches!(self, ToolGeometryField::CornerRadiusMm)
    }

    /// Whether the field is a length (mm) rather than an angle (deg).
    pub const fn is_length(self) -> bool {
        matches!(
            self,
            ToolGeometryField::CornerRadiusMm
                | ToolGeometryField::CornerRadius
                | ToolGeometryField::ShaftDiameter
        )
    }

    pub fn value_of(self, tool: &ToolConfig) -> f64 {
        match self {
            ToolGeometryField::CornerRadiusMm => tool.corner_radius_mm,
            ToolGeometryField::CornerRadius => tool.corner_radius,
            ToolGeometryField::IncludedAngle => tool.included_angle,
            ToolGeometryField::TaperHalfAngle => tool.taper_half_angle,
            ToolGeometryField::ShaftDiameter => tool.shaft_diameter,
        }
    }

    fn clear_on(self, tool: &mut ToolConfig) {
        self.set_on(tool, 0.0);
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
    /// `VBitEndmill::new` asserts `0 < included_angle < 180`. That is a
    /// panic on the compute worker, reachable from a combo box. See
    /// `ui/properties/tool.rs`'s type selector, which pairs
    /// [`ToolConfig::missing_defining_geometry`] with this.
    pub fn set_on(self, tool: &mut ToolConfig, value: f64) {
        match self {
            ToolGeometryField::CornerRadiusMm => tool.corner_radius_mm = value,
            ToolGeometryField::CornerRadius => tool.corner_radius = value,
            ToolGeometryField::IncludedAngle => tool.included_angle = value,
            ToolGeometryField::TaperHalfAngle => tool.taper_half_angle = value,
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
            _ => "mm",
        }
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
}

/// Relative tolerance for calling a number in a tool's name the same
/// number as one in its geometry. Deliberately loose: names round
/// ("6mm" for a 6.35 mm 1/4" shank), and the defect this guards is a
/// factor of two, not a rounding.
pub const NAME_MATCH_REL_TOL: f64 = 0.10;

/// Do a name figure and a geometry figure agree, within
/// [`NAME_MATCH_REL_TOL`] of the larger of the pair?
fn approx_matches(a: f64, b: f64) -> bool {
    if !a.is_finite() || !b.is_finite() {
        return false;
    }
    let scale = a.abs().max(b.abs());
    if scale == 0.0 {
        return true;
    }
    (a - b).abs() <= NAME_MATCH_REL_TOL * scale
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
    fn against(&self, name_value: f64) -> Option<NameGeometryMismatch> {
        if self.reported <= 0.0 || !self.reported.is_finite() {
            return None;
        }
        if !self.candidates.iter().any(|c| *c > 0.0 && c.is_finite()) {
            return None;
        }
        let agrees = |c: &f64| approx_matches(name_value, *c);
        if self.candidates.iter().any(agrees) {
            return None;
        }
        Some(NameGeometryMismatch {
            quantity: self.quantity,
            name_value,
            config_value: self.reported,
            field: self.field,
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
    /// No unit, or one this parse deliberately does not read (inches).
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
    role: Option<NameRole>,
}

/// Recover the numbers a tool name asserts, with the unit and the role
/// word beside each. See [`ToolConfig::name_geometry_mismatches`] for
/// what this deliberately will not read.
fn parse_name_numbers(name: &str) -> Vec<NameNumber> {
    let lower = name.to_ascii_lowercase();
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
            let (unit, resume) = read_unit(&chars, i);
            i = resume;
            let is_radius = radius_prefixed(&chars, start);
            let parsed = digits.trim_end_matches('.').parse::<f64>().ok();
            if let Some(value) = parsed.filter(|v| v.is_finite() && *v > 0.0) {
                numbers.push(NameNumber {
                    value,
                    unit,
                    radius_prefixed: is_radius,
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

/// Read the unit following a number, allowing one space ("6mm", "7 °",
/// "90 deg"). Returns the unit and the index to resume scanning at.
fn read_unit(chars: &[char], from: usize) -> (NameUnit, usize) {
    let mut i = from;
    if chars.get(i) == Some(&' ') {
        i += 1;
    }
    if chars.get(i) == Some(&'°') {
        return (NameUnit::Degrees, i + 1);
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

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

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

    /// T8: the unified lenient vocabulary, pinned exactly — canonical
    /// serde tokens, the historical core-loader aliases, and the
    /// historical viz-legacy aliases all parse; case is folded; unknown
    /// input is `None` (each surface applies its own Q4 policy on top).
    #[test]
    fn parse_lenient_vocabulary_is_pinned() {
        let table: &[(&str, ToolType)] = &[
            ("end_mill", ToolType::EndMill),
            ("endmill", ToolType::EndMill),
            ("flat", ToolType::EndMill),
            ("ball_nose", ToolType::BallNose),
            ("ballnose", ToolType::BallNose),
            ("ball", ToolType::BallNose),
            ("bull_nose", ToolType::BullNose),
            ("bullnose", ToolType::BullNose),
            ("v_bit", ToolType::VBit),
            ("vbit", ToolType::VBit),
            ("tapered_ball_nose", ToolType::TaperedBallNose),
            ("taperedballnose", ToolType::TaperedBallNose),
            ("tapered_ball", ToolType::TaperedBallNose),
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
        let mut tool = ToolConfig::new_default(ToolId(7), ToolType::EndMill);
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
cutting_length = 20.0
corner_radius = 2.0
included_angle = 90.0
taper_half_angle = 15.0
shaft_diameter = 6.0
holder_diameter = 20.0
shank_diameter = 6.0
shank_length = 20.0
stickout = 40.0
flute_count = 2
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
    /// project saved before b0362626 has on disk, whatever its type.
    fn fully_populated(tool_type: ToolType) -> ToolConfig {
        let mut tool = ToolConfig::new_default(ToolId(0), tool_type);
        tool.corner_radius_mm = 0.5;
        tool.corner_radius = 2.0;
        tool.included_angle = 90.0;
        tool.taper_half_angle = 15.0;
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
    /// type-agnostic defaults b0362626 fixed on the MCP path. Every
    /// default tool must now be complete for its type and carry
    /// nothing belonging to another.
    #[test]
    fn new_default_is_complete_and_carries_no_foreign_geometry() {
        for &tool_type in ToolType::ALL {
            let tool = ToolConfig::new_default(ToolId(3), tool_type);
            let foreign = tool.cross_type_geometry();
            assert!(foreign.is_empty(), "{tool_type:?} carries {foreign:?}");
            let missing = tool.missing_defining_geometry();
            assert!(missing.is_empty(), "{tool_type:?} missing {missing:?}");
        }
        // The specific leak: `Session::list_tools` publishes
        // `corner_radius_mm.max(corner_radius)`, which used to report a
        // 2 mm corner radius for a plain flat end mill.
        let end_mill = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        assert_eq!(end_mill.corner_radius_mm.max(end_mill.corner_radius), 0.0);
        // …while the types that are DEFINED by one of those numbers
        // keep it: a default V-bit is still a 90° V-bit.
        let vbit = ToolConfig::new_default(ToolId(0), ToolType::VBit);
        assert_eq!(vbit.included_angle, 90.0);
        let bull = ToolConfig::new_default(ToolId(0), ToolType::BullNose);
        assert_eq!(bull.corner_radius, 2.0);
        let taper = ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose);
        assert_eq!(taper.taper_half_angle, 15.0);
        assert_eq!(taper.shaft_diameter, 6.35);
    }

    /// `VBitEndmill::new` asserts `0 < included_angle < 180`, so a
    /// zeroed defining field is a panic in `build_cutter`, not a soft
    /// default — the one case a caller that flips `tool_type` in place
    /// has to check.
    #[test]
    fn missing_defining_geometry_names_the_zeroed_field() {
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.tool_type = ToolType::VBit;
        let missing = tool.missing_defining_geometry();
        assert_eq!(missing, vec![ToolGeometryField::IncludedAngle]);
        // A flat end mill with no corner radius is an ordinary flat end
        // mill, so `corner_radius_mm` is never reported missing.
        let plain = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        assert!(plain.missing_defining_geometry().is_empty());
    }

    /// THE motivating fixture (live project, 2026-08-21). The name says
    /// a 2 mm tip; the geometry is `diameter: 1.0`, which for a tapered
    /// ball IS the ball diameter — the tool is half its named size.
    /// Exactly one mismatch, and it names both numbers.
    #[test]
    fn name_says_two_mm_tip_geometry_says_one() {
        let mut tool = ToolConfig::new_default(ToolId(2), ToolType::TaperedBallNose);
        tool.name = "Tapered Ball 2mm tip / 7° / 6mm shank".to_owned();
        tool.diameter = 1.0;
        tool.taper_half_angle = 7.1;
        tool.shaft_diameter = 6.0;
        tool.shank_diameter = 6.0;
        tool.cutting_length = 20.0;

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
        let mut tool = ToolConfig::new_default(ToolId(2), ToolType::TaperedBallNose);
        tool.name = "R1.0mm x 6mm x 20mm 2F Tapered Ball".to_owned();
        tool.diameter = 1.0;
        tool.shaft_diameter = 6.0;
        tool.shank_diameter = 6.0;
        tool.cutting_length = 20.0;

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
            // Imperial: no conversion is attempted.
            ("1/4\" 2F Compression", ToolType::EndMill, 6.35),
            // A bare "6mm" for a 6.35 mm 1/4" tool is rounding.
            ("6mm 2F Carbide End Mill", ToolType::EndMill, 6.35),
            // Product codes and flute counts are not dimensions.
            ("Amana 46200 2F", ToolType::EndMill, 3.175),
            // Empty names claim nothing.
            ("", ToolType::EndMill, 6.0),
            // Several unkeyworded figures: which one is "the" diameter
            // is a guess, so none is judged.
            ("6mm shank x 22mm LOC ball", ToolType::BallNose, 3.0),
        ];
        for &(name, tool_type, diameter) in quiet {
            let mut tool = ToolConfig::new_default(ToolId(0), tool_type);
            tool.name = name.to_owned();
            tool.diameter = diameter;
            let found = tool.name_geometry_mismatches();
            assert!(found.is_empty(), "{name}: unexpected {found:?}");
        }
    }

    /// A lone unkeyworded millimetre figure IS the size, and is judged.
    #[test]
    fn lone_millimetre_figure_is_judged_against_the_diameter() {
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        tool.name = "6mm 2F Carbide End Mill".to_owned();
        tool.diameter = 3.175;
        tool.cutting_length = 25.0;
        tool.shank_diameter = 3.175;
        tool.shank_length = 20.0;

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
        let mut vbit = ToolConfig::new_default(ToolId(0), ToolType::VBit);
        vbit.name = "60 degree V-Bit".to_owned();
        vbit.diameter = 12.7;
        let found = vbit.name_geometry_mismatches();
        assert_eq!(found.len(), 1, "expected one mismatch, got {found:?}");
        assert_eq!(found[0].quantity, NamedQuantity::IncludedAngle);
        assert_eq!(found[0].name_value, 60.0);
        assert_eq!(found[0].config_value, 90.0);

        // Same figure, a type where no angle defines the geometry.
        let mut end_mill = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
        end_mill.name = "60° something".to_owned();
        assert!(end_mill.name_geometry_mismatches().is_empty());

        // A tapered ball quoted as the full included cone (2 × the
        // half-angle) agrees — vendors write it both ways.
        let mut taper = ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose);
        taper.name = "Tapered Ball 14.2 deg".to_owned();
        taper.taper_half_angle = 7.1;
        assert!(taper.name_geometry_mismatches().is_empty());
    }

    /// Role words bind only inside their own phrase, and only when the
    /// phrase holds exactly one of each — otherwise nothing binds.
    #[test]
    fn role_words_bind_within_their_phrase() {
        let mut tool = ToolConfig::new_default(ToolId(0), ToolType::TaperedBallNose);
        tool.name = "Tapered Ball 1mm tip / 7° / 3mm shank".to_owned();
        tool.diameter = 1.0;
        tool.taper_half_angle = 7.0;
        tool.shaft_diameter = 6.0;
        tool.shank_diameter = 6.0;

        let found = tool.name_geometry_mismatches();
        assert_eq!(found.len(), 1, "expected one mismatch, got {found:?}");
        assert_eq!(found[0].quantity, NamedQuantity::ShankDiameter);
        assert_eq!(found[0].name_value, 3.0);
        assert_eq!(found[0].config_value, 6.0);
    }

    /// The tolerance has to be loose enough for shorthand and tight
    /// enough for the defect: 6 vs 6.35 agrees, 2 vs 1 does not.
    #[test]
    fn tolerance_covers_rounding_not_factors() {
        assert!(approx_matches(6.0, 6.35));
        assert!(approx_matches(7.0, 7.1));
        assert!(!approx_matches(2.0, 1.0));
        assert!(!approx_matches(60.0, 90.0));
        // A zero field is unset, not contradicted.
        assert!(approx_matches(0.0, 0.0));
    }
}
