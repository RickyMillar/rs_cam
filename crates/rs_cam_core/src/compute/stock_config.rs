//! The stock model: the blank a setup presents to the cutter.
//!
//! `StockConfig` and the two model descriptors. The alignment-pin algorithm
//! lives in `compute/alignment_pins/` and the post-processor configuration
//! in `gcode/mod.rs`; this file calls the first and names neither.

use serde::{Deserialize, Serialize};

use crate::compute::alignment_pins::{
    AlignmentPin, PIN_WALL_MM, PinFinding, PinFlipReport, PinObstacle, PinPlacementError,
    PinPlacementRequest, PinSetupMap, pin_findings_for_setups, pin_flip_map, pin_wall_findings,
    place_keyed_pins, validate_pins_for_flip,
};
use crate::compute::transform::{FaceUp, ZRotation};
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
    /// The models that count as the part for the pin placer and the pin
    /// wall check (P7, 2026-10-12), by model id. `None` means the default
    /// rule, `ProjectSession::default_pin_part_models`. The operator
    /// chooses in the stock panel; `ProjectSession::pin_part_models` is
    /// the one reader.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin_part_models: Option<Vec<crate::ids::ModelId>>,
    // P3 (2026-10-12): `flip_axis` is gone. It was a cache of the setups,
    // and a stale cache drew the wrong axis line. Every reader derives the
    // axis from the setups through [`Self::pin_flip_map`].
    // Ruling R4 Q8 (2026-09-24): `workholding_rigidity` is gone. The machine
    // aggressiveness dial is the one load margin.
    // The project loader refuses an old file that holds either key
    // (`session::project_file::RETIRED_KEYS`).
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
            pin_part_models: None,
        }
    }
}

/// A partial edit of a [`StockConfig`] (F118): each `Some` field replaces
/// the stored value, and each `None` field keeps it.
///
/// The MCP `set_stock_config` tool and the CLI job stock top build this
/// patch. [`Self::apply_to`] merges it onto the current stock, and the
/// surface then sends the whole result as `Command::SetStockConfig`, so
/// the one domain check ([`StockConfig::validate`]) runs at the door.
///
/// [`Self::apply_to`] destructures [`StockConfig`] with no `..`. A new
/// stock field is therefore a compile error here until someone decides
/// whether the patch carries it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StockConfigPatch {
    /// The X edge length, mm.
    pub x: Option<f64>,
    /// The Y edge length, mm.
    pub y: Option<f64>,
    /// The Z edge length (the thickness), mm.
    pub z: Option<f64>,
    /// The X of the min corner, mm.
    pub origin_x: Option<f64>,
    /// The Y of the min corner, mm.
    pub origin_y: Option<f64>,
    /// The Z of the min corner (the stock bottom), mm.
    pub origin_z: Option<f64>,
    /// The margin `update_from_bbox` adds on each side of the model, mm.
    pub padding: Option<f64>,
    /// The stock material.
    pub material: Option<crate::material::Material>,
    /// Auto-fit the stock to the model bounding box. `None` follows the
    /// rule in [`Self::apply_to`].
    pub auto_from_model: Option<bool>,
}

/// What [`StockConfigPatch::apply_to`] changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockPatchReport {
    /// The geometry fields the patch wrote (`x` .. `origin_z`), in field
    /// order. A material or padding edit is not geometry.
    pub geometry_set: Vec<&'static str>,
    /// `auto_from_model` before the patch.
    pub auto_from_model_was: bool,
    /// `auto_from_model` after the patch.
    pub auto_from_model: bool,
}

impl StockConfigPatch {
    /// Merge this patch onto `stock`.
    ///
    /// The auto-fit rule (gap 2, 2026-08-19 run log): an explicit
    /// dimension or origin is a statement about the physical stock on the
    /// bed, so a patch that sets one and does not state `auto_from_model`
    /// clears the flag. `padding` is an input of the auto-fit rule, not a
    /// statement about the stock, so it does not clear the flag. An
    /// explicit `auto_from_model` always wins.
    ///
    /// The merge writes no domain check. The door of `SetStockConfig`
    /// refuses a value outside its domain.
    pub fn apply_to(&self, stock: &mut StockConfig) -> StockPatchReport {
        let Self {
            x: patch_x,
            y: patch_y,
            z: patch_z,
            origin_x: patch_origin_x,
            origin_y: patch_origin_y,
            origin_z: patch_origin_z,
            padding: patch_padding,
            material: patch_material,
            auto_from_model: patch_auto,
        } = self;
        let StockConfig {
            x,
            y,
            z,
            origin_x,
            origin_y,
            origin_z,
            auto_from_model,
            padding,
            material,
            // The pins have their own rows (`set_alignment_pins`,
            // `add_alignment_pin`, `remove_alignment_pin`).
            alignment_pins: _,
            // The pin part models have their own row (`SetPinPartModels`).
            pin_part_models: _,
        } = stock;

        let mut geometry_set = Vec::new();
        for (name, value, target) in [
            ("x", *patch_x, x),
            ("y", *patch_y, y),
            ("z", *patch_z, z),
            ("origin_x", *patch_origin_x, origin_x),
            ("origin_y", *patch_origin_y, origin_y),
            ("origin_z", *patch_origin_z, origin_z),
        ] {
            if let Some(v) = value {
                *target = v;
                geometry_set.push(name);
            }
        }
        if let Some(v) = patch_padding {
            *padding = *v;
        }
        if let Some(m) = patch_material {
            *material = m.clone();
        }
        let auto_from_model_was = *auto_from_model;
        *auto_from_model = match patch_auto {
            Some(explicit) => *explicit,
            None if !geometry_set.is_empty() => false,
            None => auto_from_model_was,
        };
        StockPatchReport {
            geometry_set,
            auto_from_model_was,
            auto_from_model: *auto_from_model,
        }
    }
}

impl StockConfig {
    /// F210 (W6b): check every numeric field against
    /// [`crate::compute::stock_domain::StockDimension::domain`].
    ///
    /// # Errors
    ///
    /// The [`crate::compute::stock_domain::StockDomainError`] of the first
    /// field outside its domain, in field order.
    pub fn validate(&self) -> Result<(), crate::compute::stock_domain::StockDomainError> {
        use crate::compute::stock_domain::StockDimension as D;
        for (dimension, value) in [
            (D::X, self.x),
            (D::Y, self.y),
            (D::Z, self.z),
            (D::OriginX, self.origin_x),
            (D::OriginY, self.origin_y),
            (D::OriginZ, self.origin_z),
            (D::Padding, self.padding),
        ] {
            dimension.check(value)?;
        }
        Ok(())
    }

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

    /// Three registration pins (one on the flip axis, a mirror pair off
    /// it) that key the flipped setup, or a stated reason why none exist.
    ///
    /// `obstacles` are in WORLD mm (as models, keep-outs and fixtures
    /// carry them); the stock origin is subtracted here because
    /// `alignment_pins` are stock-local. Pass an empty list only when no
    /// part, keep-out or fixture exists — an empty list to dodge a refusal
    /// puts pins through the part.
    ///
    /// `pin_diameter` comes from the tool that drills the pin holes
    /// (`ProjectSession::pin_drill_tool_for`) when one does. A hole
    /// planned at 6 mm and drilled with a 3 mm cutter is a hole no 6 mm
    /// dowel ever sees.
    pub fn plan_keyed_pins(
        &self,
        face_up: FaceUp,
        z_rotation: ZRotation,
        obstacles: &[PinObstacle],
        pin_diameter: f64,
    ) -> Result<[AlignmentPin; 3], PinPlacementError> {
        place_keyed_pins(&PinPlacementRequest {
            stock_w: self.x,
            stock_d: self.y,
            obstacles: self.obstacles_to_local(obstacles),
            face_up,
            z_rotation,
            pin_diameter,
            wall_mm: PIN_WALL_MM,
        })
    }

    /// The wall findings of the stored pins (P6): a pin with less than
    /// [`PIN_WALL_MM`] to the stock edge, the part, a keep-out zone or a
    /// fixture. `obstacles` are in WORLD mm, as for
    /// [`Self::plan_keyed_pins`].
    #[must_use]
    pub fn pin_wall_findings(&self, obstacles: &[PinObstacle]) -> Vec<PinFinding> {
        pin_wall_findings(
            &self.alignment_pins,
            self.x,
            self.y,
            &self.obstacles_to_local(obstacles),
        )
    }

    /// World-frame obstacles moved into the stock-local pin frame.
    fn obstacles_to_local(&self, obstacles: &[PinObstacle]) -> Vec<PinObstacle> {
        obstacles
            .iter()
            .map(|o| o.translated(-self.origin_x, -self.origin_y))
            .collect()
    }

    /// What this stock's stored pins do under one setup (`face_up` and
    /// `z_rotation`).
    ///
    /// Reachable from anywhere in core, which is the point: the live
    /// defect was a saved project whose pins were centre-symmetric —
    /// invariant under a 180 deg ROTATION, which is not the flip — and no
    /// load path looked.
    pub fn validate_pins_for_flip(&self, face_up: FaceUp, z_rotation: ZRotation) -> PinFlipReport {
        validate_pins_for_flip(&self.alignment_pins, face_up, z_rotation, self.x, self.y)
    }

    /// The pin findings for every setup in `setups`, deduplicated. Call
    /// this on load and in the diagnostics, and publish the texts.
    pub fn pin_findings_for_setups(
        &self,
        setups: impl IntoIterator<Item = (FaceUp, ZRotation)>,
    ) -> Vec<PinFinding> {
        pin_findings_for_setups(&self.alignment_pins, setups, self.x, self.y)
    }

    /// The flip the pins of this stock register: the map of the first
    /// setup in `setups` whose flip is in plane (see
    /// [`crate::compute::alignment_pins::pin_flip_map`]).
    /// Its [`PinSetupMap::axis_segment`] is the flip axis.
    #[must_use]
    pub fn pin_flip_map(
        &self,
        setups: impl IntoIterator<Item = (FaceUp, ZRotation)>,
    ) -> Option<PinSetupMap> {
        pin_flip_map(setups, self.x, self.y)
    }
}

#[cfg(test)]
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

    /// F118: a patch writes only its `Some` fields, `padding` included,
    /// and a geometry write clears auto-fit unless the patch states it.
    #[test]
    fn a_stock_patch_writes_only_its_fields() {
        let mut stock = StockConfig::default();
        let before = stock.clone();
        let report = StockConfigPatch {
            padding: Some(2.0),
            ..StockConfigPatch::default()
        }
        .apply_to(&mut stock);
        assert!((stock.padding - 2.0).abs() < 1e-12);
        assert!(report.geometry_set.is_empty());
        assert_eq!(stock.auto_from_model, before.auto_from_model);
        assert_eq!(stock.x.to_bits(), before.x.to_bits());

        let report = StockConfigPatch {
            x: Some(80.0),
            origin_z: Some(-10.0),
            ..StockConfigPatch::default()
        }
        .apply_to(&mut stock);
        assert_eq!(report.geometry_set, vec!["x", "origin_z"]);
        assert!(report.auto_from_model_was);
        assert!(!stock.auto_from_model);

        let _ = StockConfigPatch {
            z: Some(12.0),
            auto_from_model: Some(true),
            ..StockConfigPatch::default()
        }
        .apply_to(&mut stock);
        assert!(stock.auto_from_model, "an explicit flag wins");
    }
}
