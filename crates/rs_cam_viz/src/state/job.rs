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

/// Unique identifier for a setup (workholding / orientation context).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SetupId(pub usize);

/// Unique identifier for a fixture within a setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FixtureId(pub usize);

/// Unique identifier for a keep-out zone within a setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeepOutId(pub usize);

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

/// Which face of the stock is oriented upward in this setup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FaceUp {
    #[default]
    Top,
    Bottom,
    Front,
    Back,
    Left,
    Right,
}

impl FaceUp {
    pub const ALL: &[FaceUp] = &[
        FaceUp::Top,
        FaceUp::Bottom,
        FaceUp::Front,
        FaceUp::Back,
        FaceUp::Left,
        FaceUp::Right,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            FaceUp::Top => "Top",
            FaceUp::Bottom => "Bottom",
            FaceUp::Front => "Front",
            FaceUp::Back => "Back",
            FaceUp::Left => "Left",
            FaceUp::Right => "Right",
        }
    }

    /// Operator instruction for achieving this orientation from default (Top).
    pub fn flip_instruction(&self) -> &'static str {
        match self {
            FaceUp::Top => "No flip needed",
            FaceUp::Bottom => "Flip 180 deg on X axis",
            FaceUp::Front => "Rotate 90 deg forward on X axis",
            FaceUp::Back => "Rotate 90 deg backward on X axis",
            FaceUp::Left => "Rotate 90 deg left on Y axis",
            FaceUp::Right => "Rotate 90 deg right on Y axis",
        }
    }

    pub fn to_key(&self) -> &'static str {
        match self {
            FaceUp::Top => "top",
            FaceUp::Bottom => "bottom",
            FaceUp::Front => "front",
            FaceUp::Back => "back",
            FaceUp::Left => "left",
            FaceUp::Right => "right",
        }
    }

    pub fn from_key(s: &str) -> Self {
        match s {
            "bottom" => FaceUp::Bottom,
            "front" => FaceUp::Front,
            "back" => FaceUp::Back,
            "left" => FaceUp::Left,
            "right" => FaceUp::Right,
            _ => FaceUp::Top,
        }
    }

    /// Transform a point from world coords to this orientation's local frame.
    pub fn transform_point(
        &self,
        p: rs_cam_core::geo::P3,
        stock_w: f64,
        stock_d: f64,
        stock_h: f64,
    ) -> rs_cam_core::geo::P3 {
        use rs_cam_core::geo::P3;
        match self {
            FaceUp::Top => p,
            FaceUp::Bottom => P3::new(p.x, stock_d - p.y, stock_h - p.z),
            FaceUp::Front => P3::new(p.x, stock_h - p.z, p.y),
            FaceUp::Back => P3::new(p.x, p.z, stock_d - p.y),
            FaceUp::Left => P3::new(stock_h - p.z, p.y, p.x),
            FaceUp::Right => P3::new(p.z, p.y, stock_w - p.x),
        }
    }

    /// Effective stock dimensions (W', D', H') after this face-up transform.
    pub fn effective_stock(&self, w: f64, d: f64, h: f64) -> (f64, f64, f64) {
        match self {
            FaceUp::Top | FaceUp::Bottom => (w, d, h),
            FaceUp::Front | FaceUp::Back => (w, h, d),
            FaceUp::Left | FaceUp::Right => (h, d, w),
        }
    }
}

/// Rotation of the stock about the vertical (Z) axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ZRotation {
    #[default]
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

impl ZRotation {
    pub const ALL: &[ZRotation] = &[
        ZRotation::Deg0,
        ZRotation::Deg90,
        ZRotation::Deg180,
        ZRotation::Deg270,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            ZRotation::Deg0 => "0 deg",
            ZRotation::Deg90 => "90 deg",
            ZRotation::Deg180 => "180 deg",
            ZRotation::Deg270 => "270 deg",
        }
    }

    pub fn to_key(&self) -> &'static str {
        match self {
            ZRotation::Deg0 => "0",
            ZRotation::Deg90 => "90",
            ZRotation::Deg180 => "180",
            ZRotation::Deg270 => "270",
        }
    }

    pub fn from_key(s: &str) -> Self {
        match s {
            "90" => ZRotation::Deg90,
            "180" => ZRotation::Deg180,
            "270" => ZRotation::Deg270,
            _ => ZRotation::Deg0,
        }
    }

    /// Transform a point's XY coords by Z rotation in the setup frame.
    pub fn transform_point(
        &self,
        p: rs_cam_core::geo::P3,
        eff_w: f64,
        eff_d: f64,
    ) -> rs_cam_core::geo::P3 {
        use rs_cam_core::geo::P3;
        match self {
            ZRotation::Deg0 => p,
            ZRotation::Deg90 => P3::new(eff_d - p.y, p.x, p.z),
            ZRotation::Deg180 => P3::new(eff_w - p.x, eff_d - p.y, p.z),
            ZRotation::Deg270 => P3::new(p.y, eff_w - p.x, p.z),
        }
    }

    /// Effective stock dims after Z rotation (swaps W and D for 90/270).
    pub fn effective_stock(&self, w: f64, d: f64, h: f64) -> (f64, f64, f64) {
        match self {
            ZRotation::Deg0 | ZRotation::Deg180 => (w, d, h),
            ZRotation::Deg90 | ZRotation::Deg270 => (d, w, h),
        }
    }
}

/// Which corner of the stock to probe for XY datum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    FrontLeft,
    FrontRight,
    BackLeft,
    BackRight,
}

impl Corner {
    pub const ALL: &[Corner] = &[
        Corner::FrontLeft,
        Corner::FrontRight,
        Corner::BackLeft,
        Corner::BackRight,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Corner::FrontLeft => "Front-Left",
            Corner::FrontRight => "Front-Right",
            Corner::BackLeft => "Back-Left",
            Corner::BackRight => "Back-Right",
        }
    }

    pub fn to_key(&self) -> &'static str {
        match self {
            Corner::FrontLeft => "fl",
            Corner::FrontRight => "fr",
            Corner::BackLeft => "bl",
            Corner::BackRight => "br",
        }
    }

    pub fn from_key(s: &str) -> Self {
        match s {
            "fr" => Corner::FrontRight,
            "bl" => Corner::BackLeft,
            "br" => Corner::BackRight,
            _ => Corner::FrontLeft,
        }
    }
}

/// How the operator establishes XY zero for this setup.
#[derive(Debug, Clone, PartialEq)]
pub enum XYDatum {
    CornerProbe(Corner),
    CenterOfStock,
    AlignmentPins,
    Manual,
}

impl Default for XYDatum {
    fn default() -> Self {
        XYDatum::CornerProbe(Corner::FrontLeft)
    }
}

impl XYDatum {
    pub fn label(&self) -> &str {
        match self {
            XYDatum::CornerProbe(c) => match c {
                Corner::FrontLeft => "Corner Probe (Front-Left)",
                Corner::FrontRight => "Corner Probe (Front-Right)",
                Corner::BackLeft => "Corner Probe (Back-Left)",
                Corner::BackRight => "Corner Probe (Back-Right)",
            },
            XYDatum::CenterOfStock => "Center of Stock",
            XYDatum::AlignmentPins => "Alignment Pins",
            XYDatum::Manual => "Manual",
        }
    }

    pub fn to_key(&self) -> String {
        match self {
            XYDatum::CornerProbe(c) => format!("corner_{}", c.to_key()),
            XYDatum::CenterOfStock => "center".into(),
            XYDatum::AlignmentPins => "pins".into(),
            XYDatum::Manual => "manual".into(),
        }
    }

    pub fn from_key(s: &str) -> Self {
        if let Some(corner) = s.strip_prefix("corner_") {
            XYDatum::CornerProbe(Corner::from_key(corner))
        } else {
            match s {
                "center" => XYDatum::CenterOfStock,
                "pins" => XYDatum::AlignmentPins,
                "manual" => XYDatum::Manual,
                _ => XYDatum::default(),
            }
        }
    }
}

/// How the operator establishes Z zero for this setup.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ZDatum {
    #[default]
    StockTop,
    MachineTable,
    FixedOffset(f64),
    Manual,
}

impl ZDatum {
    pub fn label(&self) -> String {
        match self {
            ZDatum::StockTop => "Stock Top".into(),
            ZDatum::MachineTable => "Machine Table".into(),
            ZDatum::FixedOffset(z) => format!("Fixed Offset ({z:.1} mm)"),
            ZDatum::Manual => "Manual".into(),
        }
    }

    pub fn to_key(&self) -> String {
        match self {
            ZDatum::StockTop => "stock_top".into(),
            ZDatum::MachineTable => "table".into(),
            ZDatum::FixedOffset(z) => format!("offset:{z}"),
            ZDatum::Manual => "manual".into(),
        }
    }

    pub fn from_key(s: &str) -> Self {
        if let Some(val) = s.strip_prefix("offset:") {
            ZDatum::FixedOffset(val.parse().unwrap_or(0.0))
        } else {
            match s {
                "table" => ZDatum::MachineTable,
                "manual" => ZDatum::Manual,
                _ => ZDatum::StockTop,
            }
        }
    }
}

/// How to establish the work coordinate system for a setup.
#[derive(Debug, Clone, Default)]
pub struct DatumConfig {
    pub xy_method: XYDatum,
    pub z_method: ZDatum,
    pub notes: String,
}

/// A physical alignment pin position for part registration between setups.
#[derive(Debug, Clone)]
pub struct AlignmentPin {
    pub x: f64,
    pub y: f64,
    pub diameter: f64,
}

impl AlignmentPin {
    pub fn new(x: f64, y: f64, diameter: f64) -> Self {
        Self { x, y, diameter }
    }
}

/// Kind of workholding fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureKind {
    Clamp,
    Vise,
    VacuumPod,
    Custom,
}

impl FixtureKind {
    pub const ALL: &[FixtureKind] = &[
        FixtureKind::Clamp,
        FixtureKind::Vise,
        FixtureKind::VacuumPod,
        FixtureKind::Custom,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            FixtureKind::Clamp => "Clamp",
            FixtureKind::Vise => "Vise",
            FixtureKind::VacuumPod => "Vacuum Pod",
            FixtureKind::Custom => "Custom",
        }
    }
}

/// A physical workholding device positioned on the machine table.
#[derive(Debug, Clone)]
pub struct Fixture {
    pub id: FixtureId,
    pub name: String,
    pub kind: FixtureKind,
    pub enabled: bool,
    /// Position of the fixture's min corner in workpiece coordinates (mm).
    pub origin_x: f64,
    pub origin_y: f64,
    pub origin_z: f64,
    /// Dimensions of the fixture bounding box (mm).
    pub size_x: f64,
    pub size_y: f64,
    pub size_z: f64,
    /// Extra clearance around the fixture for tool avoidance (mm).
    pub clearance: f64,
}

impl Fixture {
    pub fn new_default(id: FixtureId) -> Self {
        Self {
            id,
            name: format!("Fixture {}", id.0 + 1),
            kind: FixtureKind::Clamp,
            enabled: true,
            origin_x: 0.0,
            origin_y: 0.0,
            origin_z: 0.0,
            size_x: 30.0,
            size_y: 15.0,
            size_z: 20.0,
            clearance: 3.0,
        }
    }

    /// Physical bounding box of the fixture.
    pub fn bbox(&self) -> BoundingBox3 {
        use rs_cam_core::geo::P3;
        BoundingBox3 {
            min: P3::new(self.origin_x, self.origin_y, self.origin_z),
            max: P3::new(
                self.origin_x + self.size_x,
                self.origin_y + self.size_y,
                self.origin_z + self.size_z,
            ),
        }
    }

    /// Bounding box inflated by the clearance margin (used for avoidance).
    pub fn clearance_bbox(&self) -> BoundingBox3 {
        use rs_cam_core::geo::P3;
        let c = self.clearance;
        BoundingBox3 {
            min: P3::new(self.origin_x - c, self.origin_y - c, self.origin_z),
            max: P3::new(
                self.origin_x + self.size_x + c,
                self.origin_y + self.size_y + c,
                self.origin_z + self.size_z,
            ),
        }
    }

    /// XY footprint (clearance bbox projected) as a polygon for boundary subtraction.
    pub fn footprint(&self) -> rs_cam_core::polygon::Polygon2 {
        let bb = self.clearance_bbox();
        rs_cam_core::polygon::Polygon2::rectangle(bb.min.x, bb.min.y, bb.max.x, bb.max.y)
    }
}

/// A rectangular region the tool must avoid (XY only, full Z extent).
#[derive(Debug, Clone)]
pub struct KeepOutZone {
    pub id: KeepOutId,
    pub name: String,
    pub enabled: bool,
    /// Position of the zone's min corner (mm).
    pub origin_x: f64,
    pub origin_y: f64,
    /// Dimensions of the zone (mm).
    pub size_x: f64,
    pub size_y: f64,
}

impl KeepOutZone {
    pub fn new_default(id: KeepOutId) -> Self {
        Self {
            id,
            name: format!("Keep-Out {}", id.0 + 1),
            enabled: true,
            origin_x: 0.0,
            origin_y: 0.0,
            size_x: 20.0,
            size_y: 20.0,
        }
    }

    /// XY footprint as a polygon for boundary subtraction.
    pub fn footprint(&self) -> rs_cam_core::polygon::Polygon2 {
        rs_cam_core::polygon::Polygon2::rectangle(
            self.origin_x,
            self.origin_y,
            self.origin_x + self.size_x,
            self.origin_y + self.size_y,
        )
    }
}

/// A named group of toolpaths sharing a common workholding context.
pub struct Setup {
    pub id: SetupId,
    pub name: String,
    pub face_up: FaceUp,
    pub z_rotation: ZRotation,
    pub datum: DatumConfig,
    pub alignment_pins: Vec<AlignmentPin>,
    pub fixtures: Vec<Fixture>,
    pub keep_out_zones: Vec<KeepOutZone>,
    pub toolpaths: Vec<super::toolpath::ToolpathEntry>,
}

impl Setup {
    pub fn new(id: SetupId, name: String) -> Self {
        Self {
            id,
            name,
            face_up: FaceUp::default(),
            z_rotation: ZRotation::default(),
            datum: DatumConfig::default(),
            alignment_pins: Vec::new(),
            fixtures: Vec::new(),
            keep_out_zones: Vec::new(),
            toolpaths: Vec::new(),
        }
    }

    /// Transform a point from world coords to this setup's local frame.
    pub fn transform_point(
        &self,
        p: rs_cam_core::geo::P3,
        stock: &StockConfig,
    ) -> rs_cam_core::geo::P3 {
        let p = self.face_up.transform_point(p, stock.x, stock.y, stock.z);
        let (eff_w, eff_d, _) = self.face_up.effective_stock(stock.x, stock.y, stock.z);
        self.z_rotation.transform_point(p, eff_w, eff_d)
    }

    /// Effective stock dimensions in this setup's local frame.
    pub fn effective_stock(&self, stock: &StockConfig) -> (f64, f64, f64) {
        let (w, d, h) = self.face_up.effective_stock(stock.x, stock.y, stock.z);
        self.z_rotation.effective_stock(w, d, h)
    }

    /// Whether this setup requires geometry transforms (non-identity orientation).
    pub fn needs_transform(&self) -> bool {
        self.face_up != FaceUp::Top || self.z_rotation != ZRotation::Deg0
    }
}

/// Transform a mesh into a setup's local coordinate frame.
pub fn transform_mesh(
    mesh: &rs_cam_core::mesh::TriangleMesh,
    setup: &Setup,
    stock: &StockConfig,
) -> rs_cam_core::mesh::TriangleMesh {
    let new_verts: Vec<rs_cam_core::geo::P3> = mesh
        .vertices
        .iter()
        .map(|v| setup.transform_point(*v, stock))
        .collect();
    rs_cam_core::mesh::TriangleMesh::from_raw(new_verts, mesh.triangles.clone())
}

/// Transform 2D polygons into a setup's local frame (XY projection).
pub fn transform_polygons(
    polygons: &[rs_cam_core::polygon::Polygon2],
    setup: &Setup,
    stock: &StockConfig,
) -> Vec<rs_cam_core::polygon::Polygon2> {
    use rs_cam_core::geo::{P2, P3};

    polygons
        .iter()
        .map(|poly| {
            let ext: Vec<P2> = poly
                .exterior
                .iter()
                .map(|p| {
                    let p3 = setup.transform_point(P3::new(p.x, p.y, 0.0), stock);
                    P2::new(p3.x, p3.y)
                })
                .collect();
            let holes: Vec<Vec<P2>> = poly
                .holes
                .iter()
                .map(|hole| {
                    hole.iter()
                        .map(|p| {
                            let p3 = setup.transform_point(P3::new(p.x, p.y, 0.0), stock);
                            P2::new(p3.x, p3.y)
                        })
                        .collect()
                })
                .collect();
            let mut result = rs_cam_core::polygon::Polygon2::with_holes(ext, holes);
            result.ensure_winding();
            result
        })
        .collect()
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
    pub setups: Vec<Setup>,
    /// Monotonic counter incremented on every edit (for staleness detection).
    pub edit_counter: u64,
    next_model_id: usize,
    next_tool_id: usize,
    next_toolpath_id: usize,
    next_setup_id: usize,
    next_fixture_id: usize,
    next_keep_out_id: usize,
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
            setups: vec![Setup::new(SetupId(0), "Setup 1".into())],
            edit_counter: 0,
            next_model_id: 0,
            next_tool_id: 0,
            next_toolpath_id: 0,
            next_setup_id: 1,
            next_fixture_id: 0,
            next_keep_out_id: 0,
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

    pub fn next_setup_id(&mut self) -> SetupId {
        let id = SetupId(self.next_setup_id);
        self.next_setup_id += 1;
        id
    }

    pub fn next_fixture_id(&mut self) -> FixtureId {
        let id = FixtureId(self.next_fixture_id);
        self.next_fixture_id += 1;
        id
    }

    pub fn next_keep_out_id(&mut self) -> KeepOutId {
        let id = KeepOutId(self.next_keep_out_id);
        self.next_keep_out_id += 1;
        id
    }

    /// Iterate over all toolpaths (flat view across all setups).
    pub fn all_toolpaths(&self) -> impl Iterator<Item = &super::toolpath::ToolpathEntry> {
        self.setups.iter().flat_map(|setup| setup.toolpaths.iter())
    }

    /// Mutable iteration over all toolpaths (flat view across all setups).
    pub fn all_toolpaths_mut(
        &mut self,
    ) -> impl Iterator<Item = &mut super::toolpath::ToolpathEntry> {
        self.setups
            .iter_mut()
            .flat_map(|setup| setup.toolpaths.iter_mut())
    }

    /// Find a toolpath by ID across all setups.
    pub fn find_toolpath(
        &self,
        id: super::toolpath::ToolpathId,
    ) -> Option<&super::toolpath::ToolpathEntry> {
        self.all_toolpaths().find(|toolpath| toolpath.id == id)
    }

    /// Find a mutable toolpath by ID across all setups.
    pub fn find_toolpath_mut(
        &mut self,
        id: super::toolpath::ToolpathId,
    ) -> Option<&mut super::toolpath::ToolpathEntry> {
        self.all_toolpaths_mut().find(|toolpath| toolpath.id == id)
    }

    /// Total toolpath count across all setups.
    pub fn toolpath_count(&self) -> usize {
        self.setups.iter().map(|setup| setup.toolpaths.len()).sum()
    }

    /// Add a toolpath to the default (first) setup.
    pub fn push_toolpath(&mut self, entry: super::toolpath::ToolpathEntry) {
        if let Some(setup) = self.setups.first_mut() {
            setup.toolpaths.push(entry);
        }
    }

    /// Add a toolpath to a specific setup.
    pub fn push_toolpath_to_setup(
        &mut self,
        setup_id: SetupId,
        entry: super::toolpath::ToolpathEntry,
    ) {
        if let Some(setup) = self.setups.iter_mut().find(|setup| setup.id == setup_id) {
            setup.toolpaths.push(entry);
        }
    }

    /// Remove a toolpath by ID from whatever setup contains it.
    pub fn remove_toolpath(&mut self, id: super::toolpath::ToolpathId) {
        for setup in &mut self.setups {
            setup.toolpaths.retain(|toolpath| toolpath.id != id);
        }
    }

    /// Move a toolpath one position earlier within its setup. Returns true if moved.
    pub fn move_toolpath_up(&mut self, id: super::toolpath::ToolpathId) -> bool {
        for setup in &mut self.setups {
            if let Some(pos) = setup
                .toolpaths
                .iter()
                .position(|toolpath| toolpath.id == id)
            {
                if pos > 0 {
                    setup.toolpaths.swap(pos, pos - 1);
                    return true;
                }
                return false;
            }
        }
        false
    }

    /// Move a toolpath one position later within its setup. Returns true if moved.
    pub fn move_toolpath_down(&mut self, id: super::toolpath::ToolpathId) -> bool {
        for setup in &mut self.setups {
            if let Some(pos) = setup
                .toolpaths
                .iter()
                .position(|toolpath| toolpath.id == id)
            {
                if pos + 1 < setup.toolpaths.len() {
                    setup.toolpaths.swap(pos, pos + 1);
                    return true;
                }
                return false;
            }
        }
        false
    }

    /// Iterate toolpaths with a global index (for color assignment and stable ordering).
    pub fn toolpaths_enumerated(
        &self,
    ) -> impl Iterator<Item = (usize, &super::toolpath::ToolpathEntry)> {
        self.setups
            .iter()
            .flat_map(|setup| setup.toolpaths.iter())
            .enumerate()
    }

    /// Return the setup that owns a given toolpath ID.
    pub fn setup_of_toolpath(&self, id: super::toolpath::ToolpathId) -> Option<SetupId> {
        self.setups
            .iter()
            .find(|setup| setup.toolpaths.iter().any(|toolpath| toolpath.id == id))
            .map(|setup| setup.id)
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
            .setups
            .iter()
            .flat_map(|setup| setup.toolpaths.iter())
            .map(|toolpath| toolpath.id.0)
            .max()
            .map_or(0, |id| id + 1);
        self.next_setup_id = self
            .setups
            .iter()
            .map(|setup| setup.id.0)
            .max()
            .map_or(0, |id| id + 1);
        self.next_fixture_id = self
            .setups
            .iter()
            .flat_map(|setup| setup.fixtures.iter())
            .map(|fixture| fixture.id.0)
            .max()
            .map_or(0, |id| id + 1);
        self.next_keep_out_id = self
            .setups
            .iter()
            .flat_map(|setup| setup.keep_out_zones.iter())
            .map(|keep_out| keep_out.id.0)
            .max()
            .map_or(0, |id| id + 1);
    }
}

impl Default for JobState {
    fn default() -> Self {
        Self::new()
    }
}
