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
