use serde::{Deserialize, Serialize};

use crate::geo::BoundingBox3;

/// Unique identifier for a loaded model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelId(pub usize);

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
    Step,
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

// Re-export PostFormat from core (single source of truth).
pub use crate::gcode::PostFormat;

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

/// Which axis the stock flips about when changing setups.
///
/// Determines the symmetry constraint for alignment pin placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FlipAxis {
    /// Flip left-right (mirror about the X centerline, Y stays).
    Horizontal,
    /// Flip front-back (mirror about the Y centerline, X stays).
    Vertical,
}

impl FlipAxis {
    pub fn label(&self) -> &'static str {
        match self {
            FlipAxis::Horizontal => "Horizontal",
            FlipAxis::Vertical => "Vertical",
        }
    }
}

/// A physical alignment pin position for part registration between setups.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// Stock material configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockConfig {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub origin_x: f64,
    pub origin_y: f64,
    pub origin_z: f64,
    pub auto_from_model: bool,
    pub padding: f64,
    pub material: crate::material::Material,
    /// Alignment pins for multi-setup registration (stock-level, persists across flips).
    #[serde(default)]
    pub alignment_pins: Vec<AlignmentPin>,
    /// Flip axis for multi-setup work -- constrains pin symmetry.
    #[serde(default)]
    pub flip_axis: Option<FlipAxis>,
    /// Workholding rigidity for feeds calculation.
    #[serde(default = "default_workholding_rigidity")]
    pub workholding_rigidity: crate::feeds::WorkholdingRigidity,
}

fn default_workholding_rigidity() -> crate::feeds::WorkholdingRigidity {
    crate::feeds::WorkholdingRigidity::Medium
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
            material: crate::material::Material::default(),
            alignment_pins: Vec::new(),
            flip_axis: None,
            workholding_rigidity: crate::feeds::WorkholdingRigidity::Medium,
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
        use crate::geo::P3;
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
