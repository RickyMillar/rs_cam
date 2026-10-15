//! The session inputs of the alignment-pin placer and of the pin wall
//! check (P6, 2026-10-12): the part models, the obstacles and the tools
//! that drill the pin holes, one for each pin size (R1). Also the dowel
//! notes of one setup for the export header and the setup sheet (P10).

use crate::compute::alignment_pins::{
    PIN_CAD_ROUNDING_MM, PIN_MATCH_TOL_MM, PinFinding, PinObstacle, PinObstacleKind, PinSetupNotes,
    pin_diameters, pin_setup_notes, pin_tool_findings, same_pin_diameter,
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

    /// The models that count as the part for the pin placer and the pin
    /// wall check: the operator's choice (`StockConfig::pin_part_models`,
    /// P7) when one is stored, else [`Self::default_pin_part_models`].
    ///
    /// A stored id with no loaded model is dropped, so a removed model
    /// does not stay part. Every reader takes this answer: the stock
    /// panel, the placer, the project findings and the MCP `pin_report`.
    #[must_use]
    pub fn pin_part_models(&self) -> Vec<ModelId> {
        match &self.stock.pin_part_models {
            Some(chosen) => self
                .models
                .iter()
                .filter(|m| chosen.contains(&m.id))
                .map(|m| ModelId(m.id))
                .collect(),
            None => self.default_pin_part_models(),
        }
    }

    /// The pin diameters: each stored pin diameter and the diameter of
    /// each tool that drills pin holes.
    fn pin_diameters(&self) -> Vec<f64> {
        self.stock
            .alignment_pins
            .iter()
            .map(|p| p.diameter)
            .chain(
                self.pin_tool_bindings()
                    .into_iter()
                    .filter_map(|b| b.tool_diameter),
            )
            .chain(self.default_pin_tool_diameter())
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
    /// - each enabled obstacle of each setup (K1), mapped from its setup
    ///   frame to the world frame, so it applies to the pins of every
    ///   setup. The footprint includes the obstacle's own clearance.
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
        // K1: every enabled obstacle of every setup, mapped from its
        // setup frame to the world frame. An identity setup's frame IS the
        // world frame. Another setup maps the corners of the obstacle's
        // box (footprint by Z range) and keeps their XY rectangle: the safe
        // reading, and the only one for a lateral setup.
        for setup in &self.setups {
            let ctx = super::SetupEvalContext::build_for_setup(self, Some(setup));
            // K1: when a setup's obstacles cannot be resolved, the wall to
            // them is not measured. `pin_obstacle_findings` reports it; the
            // skip is not silent.
            let Ok(set) =
                crate::compute::obstacle::ObstacleSet::resolve(&setup.obstacles, &self.models)
            else {
                continue;
            };
            for obstacle in set.obstacles() {
                let Some(info) = ctx.local_to_global.as_ref() else {
                    out.extend(obstacle.footprint.iter().map(|polygon| {
                        PinObstacle::from_polygon(
                            PinObstacleKind::Obstacle,
                            &obstacle.name,
                            polygon,
                        )
                    }));
                    continue;
                };
                let mut lo = (f64::INFINITY, f64::INFINITY);
                let mut hi = (f64::NEG_INFINITY, f64::NEG_INFINITY);
                for polygon in &obstacle.footprint {
                    let [x0, y0, x1, y1] = polygon.bbox();
                    for x in [x0, x1] {
                        for y in [y0, y1] {
                            for z in [obstacle.bottom_z, obstacle.top_z] {
                                let p = info.local_to_global(crate::geo::P3::new(x, y, z));
                                let (wx, wy) =
                                    (p.x + info.stock_origin_x, p.y + info.stock_origin_y);
                                lo = (lo.0.min(wx), lo.1.min(wy));
                                hi = (hi.0.max(wx), hi.1.max(wy));
                            }
                        }
                    }
                }
                if lo.0.is_finite() && hi.0.is_finite() {
                    out.push(PinObstacle::rectangle(
                        PinObstacleKind::Obstacle,
                        &obstacle.name,
                        lo,
                        hi,
                    ));
                }
            }
        }
        out
    }

    /// K1: one finding for each setup whose obstacles cannot be resolved.
    /// [`Self::pin_obstacles`] leaves those obstacles out, so the pin walls
    /// to them are not measured; this finding says so. The pin placement
    /// findings and the pin report both carry it.
    #[must_use]
    pub fn pin_obstacle_findings(&self) -> Vec<PinFinding> {
        self.setups
            .iter()
            .filter_map(|setup| {
                crate::compute::obstacle::ObstacleSet::resolve(&setup.obstacles, &self.models)
                    .err()
                    .map(|e| PinFinding {
                        kind: crate::compute::alignment_pins::PinFindingKind::ObstacleUnresolved,
                        text: format!(
                            "Setup '{}': the pin walls do not include an obstacle. {e}.",
                            setup.name
                        ),
                    })
            })
            .collect()
    }

    /// The pin-drill operations: for each, its toolpath index and the pin
    /// diameter it drills, in toolpath order (R1).
    #[must_use]
    pub fn pin_drill_ops(&self) -> Vec<(usize, f64)> {
        self.toolpath_configs
            .iter()
            .enumerate()
            .filter_map(|(index, tc)| match &tc.operation {
                OperationConfig::AlignmentPinDrill(cfg) => Some((index, cfg.diameter)),
                _ => None,
            })
            .collect()
    }

    /// The index of the first pin-drill operation of the pin size
    /// `diameter`, or `None`.
    #[must_use]
    pub fn pin_drill_op_for(&self, diameter: f64) -> Option<usize> {
        self.pin_drill_ops()
            .into_iter()
            .find(|(_, d)| same_pin_diameter(*d, diameter))
            .map(|(index, _)| index)
    }

    /// The diameter of the tool `tool_id`, or `None` when no tool has the
    /// id or its diameter is not positive.
    fn tool_diameter(&self, tool_id: usize) -> Option<f64> {
        self.tools
            .iter()
            .find(|t| t.id.0 == tool_id)
            .map(|t| t.diameter)
            .filter(|d| *d > 0.0)
    }

    /// The id of the tool that drills the holes of the pin size `diameter`
    /// (R1).
    ///
    /// The tool of the pin-drill operation of that size, else the tool of
    /// a drilling operation (`Drill` or `AlignmentPinDrill`) whose tool has
    /// that diameter (within `PIN_CAD_ROUNDING_MM`), else a tool of that
    /// diameter (a `VBit` first), else `None`. The tool
    /// type does not say "drill" (a twist drill is modelled as a `VBit`
    /// with a 118 deg point), so the operation that drills holes is the
    /// evidence. It is never `tools[0]`: in the mini test piece that is a
    /// 3 mm end mill. The pin door reads this id when it creates the
    /// operation of a size, so the plan and the hole agree.
    #[must_use]
    pub fn pin_drill_tool_for(&self, diameter: f64) -> Option<usize> {
        if let Some(index) = self.pin_drill_op_for(diameter) {
            return self.toolpath_configs.get(index).map(|tc| tc.tool_id);
        }
        self.toolpath_configs
            .iter()
            .filter(|tc| tc.operation.op_type().is_drill_kinematics())
            .map(|tc| tc.tool_id)
            .find(|&id| {
                self.tool_diameter(id)
                    .is_some_and(|d| same_pin_diameter(d, diameter))
            })
            .or_else(|| self.pin_tool_of_size(diameter))
    }

    /// A tool of the size `diameter` that no drilling operation names
    /// yet: the first `VBit` of the size (the project's model of a twist
    /// drill), else the first tool of the size. The tool that "Add a <d>
    /// mm drill" or the MCP `add_tool` adds is such a tool, so the door
    /// finds it.
    fn pin_tool_of_size(&self, diameter: f64) -> Option<usize> {
        use crate::compute::tool_config::ToolType;
        let of_size = || {
            self.tools
                .iter()
                .filter(move |t| t.diameter > 0.0 && same_pin_diameter(t.diameter, diameter))
        };
        of_size()
            .find(|t| t.tool_type == ToolType::VBit)
            .or_else(|| of_size().next())
            .map(|t| t.id.0)
    }

    /// The ids of every tool whose diameter is `diameter` (within
    /// `PIN_CAD_ROUNDING_MM`), in tool order: the choice the pin section
    /// offers for one pin size (R2). The tool that
    /// [`Self::pin_drill_tool_for`] names is first.
    #[must_use]
    pub fn pin_tool_candidates(&self, diameter: f64) -> Vec<usize> {
        let mut out: Vec<usize> = self
            .tools
            .iter()
            .filter(|t| t.diameter > 0.0 && same_pin_diameter(t.diameter, diameter))
            .map(|t| t.id.0)
            .collect();
        if let Some(first) = self.pin_drill_tool_for(diameter)
            && let Some(at) = out.iter().position(|id| *id == first)
        {
            out.remove(at);
            out.insert(0, first);
        }
        out
    }

    /// One row for each pin size of the stored pins, in ascending order of
    /// diameter (R1). Every reader of "the pin tool" reads this list: the
    /// pin-tool findings, the pin report, the stock panel and the MCP
    /// `pin_report`.
    #[must_use]
    pub fn pin_tool_bindings(&self) -> Vec<PinToolBinding> {
        self.pin_tool_bindings_for(&self.stock.alignment_pins)
    }

    /// [`Self::pin_tool_bindings`] for the pin set `pins` (a draft of the
    /// stock panel or the pin wizard).
    #[must_use]
    pub fn pin_tool_bindings_for(
        &self,
        pins: &[crate::compute::alignment_pins::AlignmentPin],
    ) -> Vec<PinToolBinding> {
        pin_diameters(pins)
            .into_iter()
            .map(|pin_diameter| {
                let tool_id = self.pin_drill_tool_for(pin_diameter);
                PinToolBinding {
                    pin_diameter,
                    pin_count: pins
                        .iter()
                        .filter(|p| same_pin_diameter(p.diameter, pin_diameter))
                        .count(),
                    op_index: self.pin_drill_op_for(pin_diameter),
                    tool_id,
                    tool_diameter: tool_id.and_then(|id| self.tool_diameter(id)),
                }
            })
            .collect()
    }

    /// The pin diameter the placer gives a NEW pin set when no pin is
    /// stored: the diameter of the tool of the first pin-drill operation,
    /// else of the first `Drill` operation. `None` when no operation drills
    /// holes. It is never `tools[0]`.
    #[must_use]
    pub fn default_pin_tool_diameter(&self) -> Option<f64> {
        let first_of = |pin_drill: bool| {
            self.toolpath_configs.iter().find(|tc| {
                if pin_drill {
                    matches!(tc.operation, OperationConfig::AlignmentPinDrill(_))
                } else {
                    matches!(tc.operation, OperationConfig::Drill(_))
                }
            })
        };
        first_of(true)
            .or_else(|| first_of(false))
            .and_then(|tc| self.tool_diameter(tc.tool_id))
    }

    /// The pin findings that need the session and not only the stock: a
    /// wall thinner than `PIN_WALL_MM` to the stock edge, to the part
    /// models ([`Self::pin_part_models`]), to a keep-out
    /// zone or to a fixture, and a pin size whose drill has another
    /// diameter ([`Self::pin_tool_bindings`]).
    #[must_use]
    pub fn pin_placement_findings(&self) -> Vec<PinFinding> {
        let obstacles = self.pin_obstacles(&self.pin_part_models());
        let mut out = self.stock.pin_wall_findings(&obstacles);
        out.extend(self.pin_tool_mismatch_findings(&self.stock.alignment_pins));
        if !self.stock.alignment_pins.is_empty() {
            out.extend(self.pin_obstacle_findings());
        }
        out
    }

    /// The pin-tool findings of `pins`: one for each pin size whose drill
    /// has another diameter.
    pub(crate) fn pin_tool_mismatch_findings(
        &self,
        pins: &[crate::compute::alignment_pins::AlignmentPin],
    ) -> Vec<PinFinding> {
        let sizes: Vec<(f64, Option<f64>)> = self
            .pin_tool_bindings_for(pins)
            .iter()
            .map(|b| (b.pin_diameter, b.tool_diameter))
            .collect();
        pin_tool_findings(&sizes)
    }

    /// The dowel notes of the setup `setup_id` for the per-setup G-code
    /// header and the setup sheet (P10). See
    /// [`crate::compute::alignment_pins::pin_setup_notes`].
    ///
    /// `None` when the stock has no pins, when no setup flips the part in
    /// the pin plane ([`crate::compute::StockConfig::pin_flip_map`]), or
    /// when no setup has the id. The pins register a flip; without one,
    /// the notes do not help the operator.
    #[must_use]
    pub fn pin_setup_notes(&self, setup_id: usize) -> Option<PinSetupNotes> {
        let pins = &self.stock.alignment_pins;
        if pins.is_empty() {
            return None;
        }
        self.stock
            .pin_flip_map(self.setups.iter().map(|s| (s.face_up, s.z_rotation)))?;
        let setup = self.setups.iter().find(|s| s.id == setup_id)?;
        pin_setup_notes(
            pins,
            setup.face_up,
            setup.z_rotation,
            self.stock.x,
            self.stock.y,
            self.setup_drills_dowel_holes(setup),
        )
    }

    /// `true` when `setup` drills every dowel hole: each pin is drilled by
    /// an enabled pin-drill operation of its size in the setup, or by a
    /// drill or pocket operation in the setup with a hole within
    /// [`PIN_MATCH_TOL_MM`] of the pin (a drawing of the dowel holes, as
    /// in the mini test piece).
    fn setup_drills_dowel_holes(&self, setup: &super::SetupData) -> bool {
        let in_setup = |id: usize| {
            setup.toolpath_indices.iter().any(|&i| {
                self.toolpath_configs
                    .get(i)
                    .is_some_and(|tc| tc.enabled && tc.id.0 == id)
            })
        };
        let pin_drill_sizes: Vec<f64> = setup
            .toolpath_indices
            .iter()
            .filter_map(|&i| self.toolpath_configs.get(i))
            .filter(|tc| tc.enabled)
            .filter_map(|tc| match &tc.operation {
                OperationConfig::AlignmentPinDrill(cfg) => Some(cfg.diameter),
                _ => None,
            })
            .collect();
        let pins = &self.stock.alignment_pins;
        let (_, hole_ops) = self.dowel_hole_ops();
        let setup_hole_ops: Vec<_> = hole_ops
            .iter()
            .filter(|op| in_setup(op.toolpath_id))
            .collect();
        !pins.is_empty()
            && pins.iter().all(|p| {
                pin_drill_sizes
                    .iter()
                    .any(|d| same_pin_diameter(*d, p.diameter))
                    || setup_hole_ops.iter().any(|op| {
                        op.holes
                            .iter()
                            .any(|h| (p.x - h.x).hypot(p.y - h.y) <= PIN_MATCH_TOL_MM)
                    })
            })
    }
}

/// One pin size and the drill that makes its holes
/// ([`ProjectSession::pin_tool_bindings`], R1).
#[derive(Debug, Clone, PartialEq)]
pub struct PinToolBinding {
    /// The pin diameter of the size, in mm.
    pub pin_diameter: f64,
    /// The number of pins of the size.
    pub pin_count: usize,
    /// The toolpath index of the pin-drill operation of the size, or
    /// `None` when the door has not created it.
    pub op_index: Option<usize>,
    /// The tool that drills the holes of the size
    /// ([`ProjectSession::pin_drill_tool_for`]), or `None` when no tool
    /// of the size drills holes.
    pub tool_id: Option<usize>,
    /// The diameter of that tool.
    pub tool_diameter: Option<f64>,
}

/// The point angle of the twist drill that
/// [`ProjectSession::pin_drill_tool_template`] gives a new pin drill when
/// the project has no drill to copy: the 118 deg of the 6 mm twist drill
/// of the mini test piece (`planning/rivmap_block_job/testpiece_mini.toml`,
/// tool id 3, "6 mm twist drill, 118 deg point").
pub const TWIST_DRILL_POINT_DEG: f64 = 118.0;

impl ProjectSession {
    /// A drill of `diameter` for the holes of one pin size (R2): the tool
    /// that the pin section's "Add a <d> mm drill" adds.
    ///
    /// - The type and the settings come from the tool of a pin-drill or
    ///   drill operation (the project's own drill), else a `VBit` with
    ///   [`TWIST_DRILL_POINT_DEG`] (a twist drill, as the mini test piece
    ///   models it).
    /// - The diameter, the shaft and the shank are `diameter`.
    /// - The cutting length reaches the hole bottom: the stock height plus
    ///   the spoilboard penetration of a pin-drill operation (else the
    ///   default of `AlignmentPinDrillConfig`). The stick-out is never
    ///   shorter than the cutting length.
    /// - The tool number is one more than the largest in the project.
    ///
    /// The id is a placeholder; the `AddTool` row assigns it.
    #[must_use]
    pub fn pin_drill_tool_template(
        &self,
        diameter: f64,
    ) -> crate::compute::tool_config::ToolConfig {
        use crate::compute::tool_config::{ToolConfig, ToolId, ToolType};
        let source = self
            .toolpath_configs
            .iter()
            .filter(|tc| tc.operation.op_type().is_drill_kinematics())
            .find_map(|tc| self.tools.iter().find(|t| t.id.0 == tc.tool_id));
        let mut tool = match source {
            Some(t) => t.clone(),
            None => {
                let mut t = ToolConfig::new_default(ToolId(0), ToolType::VBit);
                t.included_angle = TWIST_DRILL_POINT_DEG;
                t
            }
        };
        let penetration = self
            .toolpath_configs
            .iter()
            .find_map(|tc| match &tc.operation {
                OperationConfig::AlignmentPinDrill(cfg) => Some(cfg.spoilboard_penetration),
                _ => None,
            })
            .unwrap_or_else(|| {
                crate::compute::operation_configs::AlignmentPinDrillConfig::default()
                    .spoilboard_penetration
            });
        let reach = self.stock.z + penetration;
        tool.id = ToolId(0);
        tool.diameter = diameter;
        tool.shaft_diameter = diameter;
        tool.shank_diameter = diameter;
        tool.cutting_length = tool.cutting_length.max(reach);
        tool.stickout = tool.stickout.max(tool.cutting_length);
        tool.tool_number = self.tools.iter().map(|t| t.tool_number).max().unwrap_or(0) + 1;
        tool.name = if tool.tool_type == ToolType::VBit {
            format!(
                "{diameter:.2} mm twist drill, {:.0} deg point",
                tool.included_angle
            )
        } else {
            format!("{diameter:.2} mm drill")
        };
        tool.vendor.clear();
        tool.product_id.clear();
        let _ = tool.normalize_geometry();
        tool
    }
}
