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
}

/// Tool material (affects chip load and wear).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolMaterial {
    Carbide,
    Hss,
}

impl ToolMaterial {
    pub const ALL: &[ToolMaterial] = &[ToolMaterial::Carbide, ToolMaterial::Hss];

    pub fn label(&self) -> &'static str {
        match self {
            ToolMaterial::Carbide => "Carbide",
            ToolMaterial::Hss => "HSS",
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
#[derive(Debug, Clone, Serialize, Deserialize)]
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
        Self {
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
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

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
}
