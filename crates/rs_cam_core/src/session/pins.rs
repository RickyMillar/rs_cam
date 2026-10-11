//! The session inputs of the alignment-pin placer and of the pin wall
//! check (P6, 2026-10-12): the part models, the obstacles and the tool
//! that drills the pin holes.

use crate::compute::alignment_pins::{
    PIN_CAD_ROUNDING_MM, PIN_MATCH_TOL_MM, PinFinding, PinObstacle, PinObstacleKind,
    pin_tool_finding,
};
use crate::compute::catalog::OperationConfig;
use crate::ids::ModelId;
use crate::io::dxf_input::DrillTargetKind;

use super::ProjectSession;

impl ProjectSession {
    /// The models that the pin placer and the wall check call the part
    /// when the operator has not chosen.
    ///
    /// The default is every model with geometry, except a model whose
    /// footprint bbox covers the whole stock (each side reaches the stock
    /// edge within [`PIN_MATCH_TOL_MM`]). Such a model is a stock outline,
    /// a face boundary or a surface that spans the blank. As part, it
    /// leaves no clear material, so no pin can go anywhere. The mini test
    /// piece (`planning/rivmap_block_job/testpiece_mini.toml`) loads three:
    /// `stock_outline.dxf`, `front_3d_boundary.dxf` and
    /// `front_surface_s1.stl`.
    ///
    /// A pin-hole drawing is not part either: a model with no mesh whose
    /// every entity is a circle with a pin diameter (see
    /// [`Self::is_pin_hole_drawing`]). The mini piece loads one,
    /// `dowel_through.dxf`: two 6 mm circles on the flip axis. As part, it
    /// blocks the whole axis band of the rim. The pin editor (wave D) lets
    /// the operator choose.
    #[must_use]
    pub fn default_pin_part_models(&self) -> Vec<ModelId> {
        let stock = self.stock.bbox();
        let pin_diameters = self.pin_diameters();
        self.models
            .iter()
            .filter(|m| !Self::is_pin_hole_drawing(m, &pin_diameters))
            .filter_map(|m| Some((m.id, m.bbox()?)))
            .filter(|(_, b)| {
                let covers = b.min.x <= stock.min.x + PIN_MATCH_TOL_MM
                    && b.min.y <= stock.min.y + PIN_MATCH_TOL_MM
                    && b.max.x >= stock.max.x - PIN_MATCH_TOL_MM
                    && b.max.y >= stock.max.y - PIN_MATCH_TOL_MM;
                !covers
            })
            .map(|(id, _)| ModelId(id))
            .collect()
    }

    /// The pin diameters: each stored pin diameter and the diameter of the
    /// tool that drills the pin holes.
    fn pin_diameters(&self) -> Vec<f64> {
        self.stock
            .alignment_pins
            .iter()
            .map(|p| p.diameter)
            .chain(self.pin_drill_tool_diameter())
            .collect()
    }

    /// `true` when `model` is a drawing of pin holes: it has no mesh, it
    /// has at least one entity, and every entity is a circle whose
    /// diameter is one of `pin_diameters` within [`PIN_CAD_ROUNDING_MM`].
    ///
    /// The DXF import gives each circle a polygon AND a `CircleCenter`
    /// drill target, and gives no other entity a `CircleCenter` target. So
    /// "every entity is a circle" is: every drill target is a circle
    /// centre, and the polygon count equals the target count. A POINT, a
    /// line or any other outline breaks the equality, and the model stays
    /// part.
    fn is_pin_hole_drawing(model: &super::LoadedModel, pin_diameters: &[f64]) -> bool {
        let targets = model.drill_targets.as_slice();
        let polygon_count = model.polygons.as_deref().map_or(0, Vec::len);
        model.mesh.is_none()
            && !targets.is_empty()
            && polygon_count == targets.len()
            && targets.iter().all(|t| match t.kind {
                DrillTargetKind::CircleCenter { diameter } => pin_diameters
                    .iter()
                    .any(|d| (d - diameter).abs() <= PIN_CAD_ROUNDING_MM),
                DrillTargetKind::Point => false,
            })
    }

    /// The outlines a pin must keep a wall to, in the WORLD frame:
    ///
    /// - the footprint of each model in `part_models`. A model with 2D
    ///   polygons gives the exterior of each polygon. A model with only a
    ///   mesh gives the XY rectangle of its bbox (the mesh silhouette is
    ///   not computed; the rectangle is the safe reading);
    /// - each enabled keep-out zone and each enabled fixture of each
    ///   setup. Their footprints are in the world frame, as a model is, so
    ///   they apply to the pins of every setup. A fixture footprint
    ///   includes its own clearance.
    #[must_use]
    pub fn pin_obstacles(&self, part_models: &[ModelId]) -> Vec<PinObstacle> {
        let mut out = Vec::new();
        for model in self
            .models
            .iter()
            .filter(|m| part_models.contains(&ModelId(m.id)))
        {
            match model.polygons.as_deref() {
                Some(polygons) if !polygons.is_empty() => {
                    out.extend(polygons.iter().map(|polygon| {
                        PinObstacle::from_polygon(PinObstacleKind::Part, &model.name, polygon)
                    }));
                }
                _ => {
                    if let Some(b) = model.bbox() {
                        out.push(PinObstacle::rectangle(
                            PinObstacleKind::Part,
                            &model.name,
                            (b.min.x, b.min.y),
                            (b.max.x, b.max.y),
                        ));
                    }
                }
            }
        }
        for setup in &self.setups {
            for zone in setup.keep_out_zones.iter().filter(|z| z.enabled) {
                out.push(PinObstacle::from_polygon(
                    PinObstacleKind::KeepOut,
                    &zone.name,
                    &zone.footprint(),
                ));
            }
            for fixture in setup.fixtures.iter().filter(|f| f.enabled) {
                out.push(PinObstacle::from_polygon(
                    PinObstacleKind::Fixture,
                    &fixture.name,
                    &fixture.footprint(),
                ));
            }
        }
        out
    }

    /// The id of the tool that drills the alignment pin holes.
    ///
    /// The tool of the first `AlignmentPinDrill` operation, else the tool
    /// of the first `Drill` operation, else `None`. The tool type does not
    /// say "drill" (a twist drill is modelled as a `VBit` with a 118 deg
    /// point), so the operation that drills holes is the evidence. It is
    /// never `tools[0]`: in the mini test piece that is a 3 mm end mill,
    /// and 3 mm pins in 6 mm dowel holes do not register anything. A door
    /// that creates the pin-drill operation reads this id too, so the plan
    /// and the hole agree.
    #[must_use]
    pub fn pin_drill_tool_id(&self) -> Option<usize> {
        self.toolpath_configs
            .iter()
            .find(|tc| matches!(tc.operation, OperationConfig::AlignmentPinDrill(_)))
            .or_else(|| {
                self.toolpath_configs
                    .iter()
                    .find(|tc| matches!(tc.operation, OperationConfig::Drill(_)))
            })
            .map(|tc| tc.tool_id)
    }

    /// The diameter of the tool [`Self::pin_drill_tool_id`] names. `None`
    /// when no tool drills the holes or the tool has no positive diameter.
    #[must_use]
    pub fn pin_drill_tool_diameter(&self) -> Option<f64> {
        let tool_id = self.pin_drill_tool_id()?;
        self.tools
            .iter()
            .find(|t| t.id.0 == tool_id)
            .map(|t| t.diameter)
            .filter(|d| *d > 0.0)
    }

    /// The pin findings that need the session and not only the stock: a
    /// wall thinner than `PIN_WALL_MM` to the stock edge, to the default
    /// part models ([`Self::default_pin_part_models`]), to a keep-out
    /// zone or to a fixture, and a pin diameter that differs from the
    /// tool that drills the holes.
    #[must_use]
    pub fn pin_placement_findings(&self) -> Vec<PinFinding> {
        let obstacles = self.pin_obstacles(&self.default_pin_part_models());
        let mut out = self.stock.pin_wall_findings(&obstacles);
        out.extend(pin_tool_finding(
            &self.stock.alignment_pins,
            self.pin_drill_tool_diameter(),
        ));
        out
    }
}
