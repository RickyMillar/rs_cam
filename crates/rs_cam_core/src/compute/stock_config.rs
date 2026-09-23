//! The stock model: the blank a setup presents to the cutter.
//!
//! `StockConfig` and the two model descriptors. The alignment-pin algorithm
//! lives in `compute/alignment_pins.rs` and the post-processor configuration
//! in `gcode/mod.rs`; this file calls the first and names neither.

use serde::{Deserialize, Serialize};

use crate::compute::alignment_pins::{
    AlignmentPin, FlipAxis, PIN_WALL_MM, PinFlipReport, PinPlacementError, PinPlacementRequest,
    place_keyed_pins, validate_pins_for_flip,
};
use crate::compute::transform::FaceUp;
use crate::geo::BoundingBox3;

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
    /// Alignment pins for multi-setup registration (stock-level, persists
    /// across flips). Stock-LOCAL: X0Y0 at the stock's min corner.
    #[serde(default)]
    pub alignment_pins: Vec<AlignmentPin>,
    /// Cached [`FlipAxis`] for the viewport's flip centre line and the
    /// stock panel's Mirror affordance.
    ///
    /// **Derived from the setups, not authored, and not a constraint.**
    /// Build it with [`FlipAxis::from_face_up`]. Pin correctness is
    /// judged by [`Self::validate_pins_for_flip`] against the setup's
    /// `face_up`, never against this field — a stored axis that
    /// disagrees with the setups is a lie, and a `null` one beside a
    /// `Bottom` setup is what G-PINAUTO shipped.
    ///
    /// Invariant the GUI maintains: `Some(..)` only while
    /// `alignment_pins` is non-empty. A flipped setup whose pin
    /// placement was refused leaves this `None`, which is what keeps
    /// that state reading as INCOMPLETE rather than configured. Being a
    /// cache, it may still be stale in a file saved elsewhere; readers
    /// must treat it as advisory and never as evidence about the pins.
    #[serde(default)]
    pub flip_axis: Option<FlipAxis>,
    // Ruling R4 Q8 (2026-09-24): `workholding_rigidity` is gone. The machine
    // aggressiveness dial is the one load margin. An old file's key is
    // ignored on load (no alias, no `deny_unknown_fields`).
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
        }
    }
}

impl StockConfig {
    /// Update stock dimensions from a model bounding box.
    ///
    /// XY dimensions always follow the bbox extent + padding.
    ///
    /// Z handling depends on whether the bbox is 3D or 2D:
    /// - **3D** (mesh with non-zero Z extent): stock Z follows the bbox
    ///   Z range + padding, and `origin_z` is set to `bbox.min.z`.
    /// - **2D** (SVG/DXF polygon with `min.z == max.z`): the stock's
    ///   existing Z dimension is preserved (the user's default
    ///   thickness), but `origin_z` is set to `bbox.min.z − self.z`
    ///   so the stock's **top** is at `bbox.min.z`. Pocket / adaptive /
    ///   profile / trace / v-carve / chamfer / drill operations on 2D
    ///   models cut at negative Z (cut_depth < 0 relative to the
    ///   model's Z plane), so the stock must span `[bbox.min.z − z,
    ///   bbox.min.z]` for the cuts to engage material. Without this
    ///   shift, every cutting move was below the stock floor and the
    ///   simulator reported 0 engagement (F-2 in the April review).
    ///
    /// See planning/adaptive_review_2026-04.md F-13 (XY auto-size) and
    /// F-2 (2D Z frame).
    pub fn update_from_bbox(&mut self, bbox: &BoundingBox3) {
        self.x = bbox.max.x - bbox.min.x + 2.0 * self.padding;
        self.y = bbox.max.y - bbox.min.y + 2.0 * self.padding;
        self.origin_x = bbox.min.x - self.padding;
        self.origin_y = bbox.min.y - self.padding;
        let bbox_z_range = bbox.max.z - bbox.min.z;
        if bbox_z_range > 0.0 {
            self.z = bbox_z_range + self.padding;
            self.origin_z = bbox.min.z;
        } else {
            // 2D model: place stock so its TOP is at bbox.min.z (= 0 for
            // SVG/DXF). Leaves self.z unchanged; only origin_z shifts.
            self.origin_z = bbox.min.z - self.z;
        }
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

    /// Two registration pins that key `face_up`, or a stated reason why
    /// none exist.
    ///
    /// `model_bbox` is in WORLD mm (as models carry it); the stock origin
    /// is subtracted here because `alignment_pins` are stock-local. Pass
    /// `None` only when no model is loaded — passing `None` to dodge a
    /// refusal puts pins through the part.
    ///
    /// `pin_diameter` must come from the tool the pin-drill operation
    /// will run. A hole planned at 6 mm and drilled with a 3 mm cutter is
    /// a hole no 6 mm dowel ever sees, and the wall check above was then
    /// computed against a pin that does not exist.
    pub fn plan_keyed_pins(
        &self,
        face_up: FaceUp,
        model_bbox: Option<&BoundingBox3>,
        pin_diameter: f64,
    ) -> Result<[AlignmentPin; 2], PinPlacementError> {
        let model_x_range = model_bbox.map(|b| (b.min.x - self.origin_x, b.max.x - self.origin_x));
        place_keyed_pins(PinPlacementRequest {
            stock_w: self.x,
            stock_d: self.y,
            model_x_range,
            face_up,
            pin_diameter,
            wall_mm: PIN_WALL_MM,
        })
    }

    /// What this stock's stored pins do under `face_up`.
    ///
    /// Reachable from anywhere in core, which is the point: the live
    /// defect was a saved project whose pins were centre-symmetric —
    /// invariant under a 180 deg ROTATION, which is not the flip — and no
    /// load path looked. Call this on load and publish
    /// [`PinFlipReport::warnings`].
    pub fn validate_pins_for_flip(&self, face_up: FaceUp) -> PinFlipReport {
        validate_pins_for_flip(&self.alignment_pins, face_up, self.x, self.y)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::geo::P3;

    #[test]
    fn update_from_bbox_updates_xy_from_2d_bbox() {
        let mut stock = StockConfig {
            x: 100.0,
            y: 100.0,
            z: 25.0,
            origin_x: 0.0,
            origin_y: 0.0,
            origin_z: 0.0,
            padding: 5.0,
            ..StockConfig::default()
        };
        // 2D SVG/DXF polygon bbox: zero Z range
        let bbox = BoundingBox3 {
            min: P3::new(5.0, 5.0, 0.0),
            max: P3::new(35.0, 35.0, 0.0),
        };
        stock.update_from_bbox(&bbox);
        // XY should have been updated to fit 30x30 polygon + 5mm padding each side
        assert!((stock.x - 40.0).abs() < 1e-9);
        assert!((stock.y - 40.0).abs() < 1e-9);
        assert!((stock.origin_x - 0.0).abs() < 1e-9);
        assert!((stock.origin_y - 0.0).abs() < 1e-9);
        // Z dimension should be UNCHANGED from 25.0 — F-13 requires the
        // user's default stock thickness survives attaching a 2D model.
        assert!((stock.z - 25.0).abs() < 1e-9);
        // Z origin shifts so the stock TOP is at bbox.min.z (= 0 for
        // the SVG frame). Before F-2, origin_z stayed at 0 which put
        // the stock entirely below the 2D pocket's cut range.
        assert!(
            (stock.origin_z - (-25.0)).abs() < 1e-9,
            "expected 2D stock to have origin_z = -z (top at 0), got {}",
            stock.origin_z
        );
    }

    #[test]
    fn update_from_bbox_updates_z_from_3d_bbox() {
        let mut stock = StockConfig {
            x: 100.0,
            y: 100.0,
            z: 25.0,
            origin_x: 0.0,
            origin_y: 0.0,
            origin_z: 0.0,
            padding: 5.0,
            ..StockConfig::default()
        };
        // 3D mesh bbox with non-zero Z extent
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(50.0, 40.0, 10.0),
        };
        stock.update_from_bbox(&bbox);
        assert!((stock.x - 60.0).abs() < 1e-9); // 50 + 2*5
        assert!((stock.y - 50.0).abs() < 1e-9); // 40 + 2*5
        assert!((stock.z - 15.0).abs() < 1e-9); // 10 + 5
        assert!((stock.origin_z - 0.0).abs() < 1e-9);
    }
}
