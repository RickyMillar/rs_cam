use std::path::PathBuf;
use std::sync::Arc;

use rs_cam_core::geo::BoundingBox3;
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::polygon::Polygon2;
use serde::{Deserialize, Serialize};

/// Unique identifier for a loaded model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelId(pub usize);

/// Unique identifier for a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolId(pub usize);

/// What kind of geometry was loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    Stl,
    Svg,
    Dxf,
}

/// Assumed units of the imported STL (determines scale factor to mm).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "scale", rename_all = "snake_case")]
pub enum ModelUnits {
    Millimeters,
    Inches,
    Meters,
    Centimeters,
    Custom(f64),
}

impl ModelUnits {
    pub const PRESETS: &[(ModelUnits, &'static str)] = &[
        (ModelUnits::Millimeters, "mm (1:1)"),
        (ModelUnits::Inches, "inches (x25.4)"),
        (ModelUnits::Centimeters, "cm (x10)"),
        (ModelUnits::Meters, "m (x1000)"),
    ];

    pub fn scale_factor(&self) -> f64 {
        match self {
            ModelUnits::Millimeters => 1.0,
            ModelUnits::Inches => 25.4,
            ModelUnits::Meters => 1000.0,
            ModelUnits::Centimeters => 10.0,
            ModelUnits::Custom(s) => *s,
        }
    }

    pub fn label(&self) -> String {
        match self {
            ModelUnits::Millimeters => "mm".into(),
            ModelUnits::Inches => "inches".into(),
            ModelUnits::Meters => "m".into(),
            ModelUnits::Centimeters => "cm".into(),
            ModelUnits::Custom(s) => format!("x{s:.3}"),
        }
    }
}

/// A loaded geometry model.
pub struct LoadedModel {
    pub id: ModelId,
    pub path: PathBuf,
    pub name: String,
    pub kind: ModelKind,
    pub mesh: Option<Arc<TriangleMesh>>,
    pub polygons: Option<Arc<Vec<Polygon2>>>,
    pub units: ModelUnits,
    /// Percentage of inconsistent winding edges (from check_winding). None if not STL.
    pub winding_report: Option<f64>,
    /// Load/import failure preserved so broken references can round-trip.
    pub load_error: Option<String>,
}

impl LoadedModel {
    pub fn placeholder(
        id: ModelId,
        path: PathBuf,
        name: String,
        kind: ModelKind,
        units: ModelUnits,
        load_error: String,
    ) -> Self {
        Self {
            id,
            path,
            name,
            kind,
            mesh: None,
            polygons: None,
            units,
            winding_report: None,
            load_error: Some(load_error),
        }
    }
}

/// Tool type matching the five cutter types in rs_cam_core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
pub enum CutDirection {
    UpCut,
    DownCut,
    Compression,
}

impl CutDirection {
    pub const ALL: &[CutDirection] = &[
        CutDirection::UpCut,
        CutDirection::DownCut,
        CutDirection::Compression,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            CutDirection::UpCut => "Up Cut",
            CutDirection::DownCut => "Down Cut",
            CutDirection::Compression => "Compression",
        }
    }
}

/// Complete tool configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    pub id: ToolId,
    pub name: String,
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
    pub cut_direction: CutDirection,
    // Optional vendor info
    pub vendor: String,
    pub product_id: String,
}

impl ToolConfig {
    pub fn new_default(id: ToolId, tool_type: ToolType) -> Self {
        let (name, diameter) = match tool_type {
            ToolType::EndMill => ("End Mill".to_string(), 6.35),
            ToolType::BallNose => ("Ball Nose".to_string(), 6.35),
            ToolType::BullNose => ("Bull Nose".to_string(), 12.7),
            ToolType::VBit => ("V-Bit".to_string(), 12.7),
            ToolType::TaperedBallNose => ("Tapered Ball Nose".to_string(), 3.175),
        };
        Self {
            id,
            name,
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
            cut_direction: CutDirection::UpCut,
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

/// Post-processor format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostFormat {
    Grbl,
    LinuxCnc,
    Mach3,
}

impl PostFormat {
    pub const ALL: &[PostFormat] = &[PostFormat::Grbl, PostFormat::LinuxCnc, PostFormat::Mach3];

    pub fn label(&self) -> &'static str {
        match self {
            PostFormat::Grbl => "GRBL",
            PostFormat::LinuxCnc => "LinuxCNC",
            PostFormat::Mach3 => "Mach3",
        }
    }
}

/// Post-processor configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostConfig {
    pub format: PostFormat,
    pub spindle_speed: u32,
    pub safe_z: f64,
    /// Convert G0 rapids to G1 at high feedrate (for machines with unpredictable rapid behavior).
    pub high_feedrate_mode: bool,
    pub high_feedrate: f64,
}

impl Default for PostConfig {
    fn default() -> Self {
        Self {
            format: PostFormat::Grbl,
            spindle_speed: 18000,
            safe_z: 10.0,
            high_feedrate_mode: false,
            high_feedrate: 5000.0,
        }
    }
}

/// Stock material configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StockConfig {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub origin_x: f64,
    pub origin_y: f64,
    pub origin_z: f64,
    pub auto_from_model: bool,
    pub padding: f64,
    pub material: rs_cam_core::material::Material,
}

impl Default for StockConfig {
    fn default() -> Self {
        Self {
            x: 100.0,
            y: 100.0,
            z: 25.0,
            origin_x: 0.0,
            origin_y: 0.0,
            origin_z: 0.0,
            auto_from_model: true,
            padding: 5.0,
            material: rs_cam_core::material::Material::default(),
        }
    }
}

impl StockConfig {
    /// Update stock dimensions from model bounding box.
    pub fn update_from_bbox(&mut self, bbox: &BoundingBox3) {
        self.x = bbox.max.x - bbox.min.x + 2.0 * self.padding;
        self.y = bbox.max.y - bbox.min.y + 2.0 * self.padding;
        self.z = bbox.max.z - bbox.min.z + self.padding;
        self.origin_x = bbox.min.x - self.padding;
        self.origin_y = bbox.min.y - self.padding;
        self.origin_z = bbox.min.z;
    }

    /// Get the bounding box of the stock.
    pub fn bbox(&self) -> BoundingBox3 {
        use rs_cam_core::geo::P3;
        BoundingBox3 {
            min: P3::new(self.origin_x, self.origin_y, self.origin_z),
            max: P3::new(
                self.origin_x + self.x,
                self.origin_y + self.y,
                self.origin_z + self.z,
            ),
        }
    }
}

/// The full job state.
pub struct JobState {
    pub name: String,
    pub file_path: Option<PathBuf>,
    pub dirty: bool,
    pub models: Vec<LoadedModel>,
    pub stock: StockConfig,
    pub tools: Vec<ToolConfig>,
    pub post: PostConfig,
    pub machine: rs_cam_core::machine::MachineProfile,
    pub toolpaths: Vec<super::toolpath::ToolpathEntry>,
    /// Monotonic counter incremented on every edit (for staleness detection).
    pub edit_counter: u64,
    next_model_id: usize,
    next_tool_id: usize,
    next_toolpath_id: usize,
}

impl JobState {
    pub fn new() -> Self {
        Self {
            name: "Untitled".to_string(),
            file_path: None,
            dirty: false,
            models: Vec::new(),
            stock: StockConfig::default(),
            tools: Vec::new(),
            post: PostConfig::default(),
            machine: rs_cam_core::machine::MachineProfile::default(),
            toolpaths: Vec::new(),
            edit_counter: 0,
            next_model_id: 0,
            next_tool_id: 0,
            next_toolpath_id: 0,
        }
    }

    pub fn next_model_id(&mut self) -> ModelId {
        let id = ModelId(self.next_model_id);
        self.next_model_id += 1;
        id
    }

    pub fn next_tool_id(&mut self) -> ToolId {
        let id = ToolId(self.next_tool_id);
        self.next_tool_id += 1;
        id
    }

    pub fn next_toolpath_id(&mut self) -> super::toolpath::ToolpathId {
        let id = super::toolpath::ToolpathId(self.next_toolpath_id);
        self.next_toolpath_id += 1;
        id
    }

    /// Mark the job as edited (increments edit counter for staleness tracking).
    pub fn mark_edited(&mut self) {
        self.dirty = true;
        self.edit_counter += 1;
    }

    pub fn sync_next_ids(&mut self) {
        self.next_model_id = self
            .models
            .iter()
            .map(|m| m.id.0)
            .max()
            .map_or(0, |id| id + 1);
        self.next_tool_id = self
            .tools
            .iter()
            .map(|t| t.id.0)
            .max()
            .map_or(0, |id| id + 1);
        self.next_toolpath_id = self
            .toolpaths
            .iter()
            .map(|tp| tp.id.0)
            .max()
            .map_or(0, |id| id + 1);
    }
}

impl Default for JobState {
    fn default() -> Self {
        Self::new()
    }
}
