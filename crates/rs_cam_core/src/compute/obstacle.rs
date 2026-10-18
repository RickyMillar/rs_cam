//! K1 (`planning/op_architecture_2026-10-14/PLAN.md`, item K): the
//! obstacles of a setup and the keep-out service.
//!
//! An obstacle is a solid the tool must not enter: a clamp, a vise jaw, a
//! keep-out zone. Fixtures and keep-out zones are ONE concept (operator
//! ruling 2026-10-15). The geometry is a box, a prism from closed 2D
//! outlines, or an imported mesh. Every value is in the SETUP frame: the
//! frame the setup's toolpaths emit in, with the setup's +Z up. An obstacle
//! is fixed to the table, not to the part. So the resolver reads all three
//! kinds as they are, with NO setup transform: a flip of the part on a
//! Bottom setup does not move a clamp. (A stock change is material and
//! moves with the part, so it does map through the setup transform; the
//! two share only the model validation.)
//!
//! # The model of the solid
//!
//! The service grows each obstacle by its clearance and reads it as a
//! solid from its top DOWN: the tool assembly (cutter, shank, holder, and
//! the spindle above them) extends upward, so a tool under an obstacle
//! footprint below its top is always a collision. The obstacle bottom
//! therefore never frees the tool. A mesh obstacle is its XY silhouette,
//! from its top down. A vertical retract from an allowed position is
//! always allowed under this model.
//!
//! # The service
//!
//! [`ObstacleSet::resolve`] maps one setup's enabled obstacles into the
//! setup frame. It REFUSES an obstacle whose model is missing: a clamp
//! that vanished in silence is the defect K1 fixes. [`ObstacleSet::for_tool`]
//! then gives the per-tool view ([`ObstacleToolView`]): one footprint per
//! assembly band, grown by the band radius. Every consumer reads that one
//! view:
//!
//! - the per-op step (`session::compute`, after the dressups), through
//!   [`ObstaclePermit`] and the shared clip walk;
//! - the link ceiling (`finish::surface_link::LinkCeiling::obstacles`);
//! - the territory pre-clip (`compute::execute::territory`), for the 3D
//!   ops and the area ops;
//! - the simulation check and `collision_check`
//!   ([`ObstacleToolView::check_toolpath`]).
//!
//! [`ObstacleSet::plane_z`] is the one obstacle plane: the highest grown
//! top plus [`SAFE_Z_CLEARANCE_MM`].
//!
//! # The check height and the plane
//!
//! Two heights, one rule each. The CHECK forbids a band of the tool below
//! its ceiling, `tip_ceiling_z` = grown top minus the band offset: the
//! grown top (top plus clearance) is the physical limit. The PLANE (grown
//! top plus [`SAFE_Z_CLEARANCE_MM`]) is only the height the tool retracts
//! to and crosses at; it is a margin above the limit, and the check does
//! not read it.
//!
//! # The plane is local (K3)
//!
//! The height resolver does not read the obstacles: an op keeps its own
//! retract and clearance. A move rises to the plane only where it must:
//!
//! - the per-op step splits each move that the check forbids, and the
//!   tool crosses the obstacle at the plane;
//! - the first move of every toolpath in a setup with an obstacle is a
//!   rapid at the plane or higher ([`clip_annotated_to_obstacles`]). The
//!   step tags the rapid that it inserts `MoveIntent::ObstacleClearance`. So the
//!   G-code builder's join into a toolpath rises to the plane before it
//!   traverses, from a known start ([`JoinStart::After`]) and from an
//!   unknown one ([`JoinStart::Unknown`]).

use std::hash::{DefaultHasher, Hash, Hasher};

use serde::{Deserialize, Serialize};

use crate::compute::config::SAFE_Z_CLEARANCE_MM;
use crate::compute::stock_change::{
    ModelGeometryFacts, StockChangeOp, StockChangeRefusal, StockGeometry, is_closed_outline,
    validate_stock_geometry,
};
use crate::geo::P3;
use crate::geometry::boundary::{ClipPermit, maximal_inside_intervals};
use crate::ids::{ModelId, ObstacleId};
use crate::polygon::Polygon2;
use crate::stock::collision::{AssemblySegment, CollisionEvent, CollisionKind, ToolAssembly};
use crate::toolpath::{Move, MoveType, Toolpath};

/// The geometry of an obstacle. Every value is in the setup frame (the
/// frame the setup's toolpaths emit in), for all three kinds. The resolver
/// applies no setup transform: an obstacle is fixed to the table, and a
/// part flip does not move it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ObstacleGeometry {
    /// An axis-aligned box, `min` to `max` (mm).
    Box {
        /// The low corner `[x, y, z]`.
        min: [f64; 3],
        /// The high corner `[x, y, z]`.
        max: [f64; 3],
    },
    /// A prism from the closed 2D outlines of `model_ids`, from `z_bottom`
    /// to `z_top`. The outline XY values are setup-frame XY values, as
    /// they are in the file.
    OutlineExtrude {
        /// The 2D models whose closed outlines give the prism section.
        model_ids: Vec<ModelId>,
        /// The prism bottom (mm).
        z_bottom: f64,
        /// The prism top (mm).
        z_top: f64,
    },
    /// An imported mesh. The mesh coordinates are setup-frame coordinates,
    /// as they are in the file. The mesh does not move with the part.
    Model {
        /// The mesh model.
        model_id: ModelId,
    },
}

impl ObstacleGeometry {
    /// Every model this geometry reads.
    #[must_use]
    pub fn model_ids(&self) -> Vec<ModelId> {
        match self {
            Self::Box { .. } => Vec::new(),
            Self::OutlineExtrude { model_ids, .. } => model_ids.clone(),
            Self::Model { model_id } => vec![*model_id],
        }
    }

    /// Does this geometry read `model_id`?
    #[must_use]
    pub fn reads_model(&self, model_id: ModelId) -> bool {
        self.model_ids().contains(&model_id)
    }

    /// The short kind word that a refusal and a surface print.
    #[must_use]
    pub fn kind_label(&self) -> &'static str {
        match self {
            Self::Box { .. } => "box",
            Self::OutlineExtrude { .. } => "outline extrude",
            Self::Model { .. } => "model",
        }
    }

    /// The stock-change form of a model geometry, for the shared model
    /// validation. `None` for a box.
    fn as_stock_geometry(&self) -> Option<StockGeometry> {
        match self {
            Self::Box { .. } => None,
            Self::OutlineExtrude {
                model_ids,
                z_bottom,
                z_top,
            } => Some(StockGeometry::OutlineExtrude {
                model_ids: model_ids.clone(),
                z_bottom: *z_bottom,
                z_top: *z_top,
            }),
            Self::Model { model_id } => Some(StockGeometry::Model {
                model_id: *model_id,
            }),
        }
    }
}

/// One obstacle of a setup: a solid the tool must not enter.
///
/// The record derives `PartialEq` because a GUI panel edits a CLONE of it
/// and must not apply a command for an edit that moved nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Obstacle {
    /// The id, unique within the setup.
    pub id: ObstacleId,
    /// The name the operator gave. A collision names the obstacle by it.
    #[serde(default)]
    pub name: String,
    /// A disabled obstacle has no effect.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// The distance (mm) the tool keeps from the geometry, on every side.
    #[serde(default = "default_clearance")]
    pub clearance: f64,
    /// The solid.
    pub geometry: ObstacleGeometry,
}

fn default_true() -> bool {
    true
}

/// The clearance of a new obstacle (mm): the value the old `Fixture`
/// record defaulted to.
pub const DEFAULT_OBSTACLE_CLEARANCE_MM: f64 = 3.0;

fn default_clearance() -> f64 {
    DEFAULT_OBSTACLE_CLEARANCE_MM
}

/// The spans (mm) of a new box obstacle: the old GUI clamp, 30 by 15 by 20.
pub const DEFAULT_OBSTACLE_BOX_MM: [f64; 3] = [30.0, 15.0, 20.0];

impl Obstacle {
    /// A new enabled box obstacle at the setup origin, with the default
    /// spans and clearance. The GUI and MCP "add" doors start from it.
    #[must_use]
    pub fn new_default_box(id: ObstacleId) -> Self {
        Self {
            name: format!("Obstacle {}", id.0 + 1),
            id,
            enabled: true,
            clearance: DEFAULT_OBSTACLE_CLEARANCE_MM,
            geometry: ObstacleGeometry::Box {
                min: [0.0, 0.0, 0.0],
                max: DEFAULT_OBSTACLE_BOX_MM,
            },
        }
    }

    /// A digest of every field that decides where the tool may go: the
    /// enabled flag, the clearance and the geometry. The name is not in
    /// it, so a rename keeps the results.
    #[must_use]
    pub fn effect_digest(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        self.enabled.hash(&mut hasher);
        format!("{:?}", (self.clearance, &self.geometry)).hash(&mut hasher);
        hasher.finish()
    }
}

/// The id for a new obstacle in a setup: one more than the largest id the
/// setup holds, or 0 for an empty list. The GUI and MCP both call it.
#[must_use]
pub fn next_obstacle_id(obstacles: &[Obstacle]) -> ObstacleId {
    ObstacleId(obstacles.iter().map(|o| o.id.0 + 1).max().unwrap_or(0))
}

/// Why an obstacle was refused. Each variant names the rule it broke.
#[derive(Debug, Clone, PartialEq)]
pub enum ObstacleRefusal {
    /// A model geometry broke a rule of the shared model validation.
    Geometry(StockChangeRefusal),
    /// A box value is NaN or infinite.
    BoxNotFinite,
    /// A box has `min >= max` on an axis.
    EmptyBox {
        /// The axis name: `x`, `y` or `z`.
        axis: &'static str,
    },
    /// The clearance is negative, NaN or infinite.
    BadClearance {
        /// The value.
        clearance: f64,
    },
    /// Two obstacles in one setup carry the same id.
    DuplicateId {
        /// The id.
        id: ObstacleId,
    },
}

impl std::fmt::Display for ObstacleRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Geometry(refusal) => {
                // The shared text says "stock change"; an obstacle reads it
                // with its own noun.
                let text = refusal.to_string().replace("stock change", "obstacle");
                f.write_str(&text)
            }
            Self::BoxNotFinite => f.write_str("a box obstacle needs finite corner values"),
            Self::EmptyBox { axis } => {
                write!(f, "a box obstacle needs min below max on the {axis} axis")
            }
            Self::BadClearance { clearance } => write!(
                f,
                "an obstacle clearance must be a finite value of 0 or more, and it is {clearance}"
            ),
            Self::DuplicateId { id } => {
                write!(f, "the setup already holds an obstacle with id {}", id.0)
            }
        }
    }
}

impl std::error::Error for ObstacleRefusal {}

/// Check one obstacle against the project models.
///
/// # Errors
/// The [`ObstacleRefusal`] that names the first rule the obstacle breaks.
pub fn validate_obstacle<'a>(
    obstacle: &Obstacle,
    facts: impl Fn(ModelId) -> Option<ModelGeometryFacts<'a>>,
) -> Result<(), ObstacleRefusal> {
    if !obstacle.clearance.is_finite() || obstacle.clearance < 0.0 {
        return Err(ObstacleRefusal::BadClearance {
            clearance: obstacle.clearance,
        });
    }
    match &obstacle.geometry {
        ObstacleGeometry::Box { min, max } => {
            if min.iter().chain(max.iter()).any(|v| !v.is_finite()) {
                return Err(ObstacleRefusal::BoxNotFinite);
            }
            for (axis, (lo, hi)) in ["x", "y", "z"].into_iter().zip(min.iter().zip(max.iter())) {
                if lo >= hi {
                    return Err(ObstacleRefusal::EmptyBox { axis });
                }
            }
            Ok(())
        }
        other => {
            let Some(geometry) = other.as_stock_geometry() else {
                return Ok(());
            };
            validate_stock_geometry(&geometry, StockChangeOp::Add, facts)
                .map_err(ObstacleRefusal::Geometry)
        }
    }
}

/// The geometry facts of one project model, for [`validate_obstacle`]
/// and the stock-change validation.
#[must_use]
pub fn model_geometry_facts(
    models: &[crate::session::LoadedModel],
    model_id: ModelId,
) -> Option<ModelGeometryFacts<'_>> {
    models
        .iter()
        .find(|m| m.id == model_id.0)
        .map(|m| ModelGeometryFacts {
            name: &m.name,
            has_mesh: m.mesh.is_some(),
            has_closed_outline: m
                .outlines_pre_selection()
                .is_some_and(|p| p.iter().any(is_closed_outline)),
        })
}

/// Why an obstacle set could not be resolved. The text names the obstacle.
#[derive(Debug, Clone, PartialEq)]
pub struct ObstacleResolveError {
    /// The obstacle name.
    pub name: String,
    /// What failed.
    pub reason: String,
}

impl std::fmt::Display for ObstacleResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "obstacle '{}' cannot be resolved: {}. Fix or disable the obstacle; \
             the tool would otherwise not avoid it",
            self.name, self.reason
        )
    }
}

impl std::error::Error for ObstacleResolveError {}

/// The raster cell (mm) of a mesh obstacle's silhouette. The silhouette
/// then grows by one more cell, because the raster outline can lie up to
/// one cell inside the true projection.
///
/// It IS the default silhouette cell, so the resolver can read the
/// per-mesh memo `maps::geom_cache::cached_silhouette` (K2). If obstacles
/// get their own cell, the resolver must go back to the uncached raster.
pub const SILHOUETTE_CELL_MM: f64 = crate::geometry::boundary::SILHOUETTE_CELL_SIZE;

/// One enabled obstacle, resolved into the setup frame and grown by its
/// clearance.
#[derive(Debug, Clone)]
pub struct ResolvedObstacle {
    /// The id.
    pub id: ObstacleId,
    /// The name, for the collision text.
    pub name: String,
    /// The XY footprint, grown by the clearance. No holes: the obstacle is
    /// solid.
    pub footprint: Vec<Polygon2>,
    /// The top Z, grown by the clearance.
    pub top_z: f64,
    /// The bottom Z, grown by the clearance. A display value: the service
    /// reads the obstacle as solid from its top down (module doc).
    pub bottom_z: f64,
    /// The XY section before the clearance grow: the box rectangle, the
    /// outline union, or the silhouette after its one-cell grow. A display
    /// value (K2): the renderer draws it as the obstacle itself.
    pub outline: Vec<Polygon2>,
    /// The bottom Z of [`Self::outline`], without the clearance.
    pub outline_bottom_z: f64,
    /// The top Z of [`Self::outline`], without the clearance.
    pub outline_top_z: f64,
}

/// The enabled obstacles of one setup, in the setup frame. Empty when the
/// setup has none.
#[derive(Debug, Clone, Default)]
pub struct ObstacleSet {
    obstacles: Vec<ResolvedObstacle>,
}

/// Grow `polygons` outward by `distance` (mm), keeping only the exteriors.
///
/// The offset keeps its arcs and flattens them once. A flattened arc is a
/// chord, and a chord lies inside its arc by up to the flatten deviation.
/// So the offset distance includes that deviation, and no point of the
/// result is closer than `distance` to the input. (`offset_polygon` keeps
/// only the chord endpoints: on a 90 degree corner it cut 29 % of the
/// distance off the keep-out.)
fn grow(
    polygons: &[Polygon2],
    distance: f64,
    name: &str,
) -> Result<Vec<Polygon2>, ObstacleResolveError> {
    let mut out = Vec::new();
    for polygon in polygons {
        let solid = Polygon2::new(polygon.exterior.clone());
        if distance <= 0.0 {
            out.push(solid);
            continue;
        }
        let policy = crate::polygon::FlattenPolicy::untoleranced();
        let (grown, failure) = crate::polygon::OffsetRingSet::from_polygon(&solid)
            .offset_reported(-(distance + policy.max_deviation_mm()));
        if let Some(failure) = failure {
            return Err(ObstacleResolveError {
                name: name.to_owned(),
                reason: format!("the outward offset failed ({failure:?})"),
            });
        }
        out.extend(
            grown
                .to_polygons(policy)
                .into_iter()
                .map(|p| Polygon2::new(p.exterior)),
        );
    }
    Ok(out)
}

/// The Z range of a geometry in the setup frame, without the clearance.
/// `None` when a model the geometry reads is missing. A mesh gives its
/// bounding box Z range: its coordinates are setup-frame coordinates.
fn geometry_z_range(
    geometry: &ObstacleGeometry,
    models: &[crate::session::LoadedModel],
) -> Option<(f64, f64)> {
    match geometry {
        ObstacleGeometry::Box { min, max } => Some((min[2], max[2])),
        ObstacleGeometry::OutlineExtrude {
            z_bottom, z_top, ..
        } => Some((*z_bottom, *z_top)),
        ObstacleGeometry::Model { model_id } => {
            let mesh = models
                .iter()
                .find(|m| m.id == model_id.0)
                .and_then(|m| m.mesh.as_ref())?;
            Some((mesh.bbox.min.z, mesh.bbox.max.z))
        }
    }
}

impl ObstacleSet {
    /// THE resolver: one setup's enabled obstacles, in the setup frame,
    /// grown by their clearance.
    ///
    /// All three kinds read their values as setup-frame values, with no
    /// setup transform (module doc): an obstacle is fixed to the table.
    ///
    /// # Errors
    /// [`ObstacleResolveError`] when an enabled obstacle reads a model that
    /// is missing or holds no geometry of the kind it needs, or when a
    /// value is not usable. The resolver never skips an obstacle.
    pub fn resolve(
        obstacles: &[Obstacle],
        models: &[crate::session::LoadedModel],
    ) -> Result<Self, ObstacleResolveError> {
        let mut resolved = Vec::new();
        for obstacle in obstacles.iter().filter(|o| o.enabled) {
            let refuse = |reason: String| ObstacleResolveError {
                name: obstacle.name.clone(),
                reason,
            };
            validate_obstacle(obstacle, |id| model_geometry_facts(models, id))
                .map_err(|e| refuse(e.to_string()))?;
            let c = obstacle.clearance;
            let (raw_footprint, bottom, top) = match &obstacle.geometry {
                ObstacleGeometry::Box { min, max } => (
                    vec![Polygon2::rectangle(min[0], min[1], max[0], max[1])],
                    min[2],
                    max[2],
                ),
                ObstacleGeometry::OutlineExtrude {
                    model_ids,
                    z_bottom,
                    z_top,
                } => {
                    let mut closed = Vec::new();
                    for model_id in model_ids {
                        let polygons = models
                            .iter()
                            .find(|m| m.id == model_id.0)
                            .and_then(|m| m.outlines_pre_selection())
                            .ok_or_else(|| {
                                refuse(format!("model id {} holds no 2D outline", model_id.0))
                            })?;
                        closed.extend(polygons.iter().filter(|p| is_closed_outline(p)).cloned());
                    }
                    let union = Polygon2::union_all(&closed);
                    if union.is_empty() {
                        return Err(refuse(
                            "the outline models hold no closed outline with an area".to_owned(),
                        ));
                    }
                    (union, *z_bottom, *z_top)
                }
                ObstacleGeometry::Model { model_id } => {
                    let mesh = models
                        .iter()
                        .find(|m| m.id == model_id.0)
                        .and_then(|m| m.mesh.clone())
                        .ok_or_else(|| refuse(format!("model id {} holds no mesh", model_id.0)))?;
                    // The per-mesh memo: the pin panel resolves every
                    // setup each frame, and the raster is the cost (K2).
                    let silhouette = crate::maps::geom_cache::cached_silhouette(&mesh);
                    if silhouette.is_empty() {
                        return Err(refuse("the mesh has an empty silhouette".to_owned()));
                    }
                    let silhouette = grow(&silhouette, SILHOUETTE_CELL_MM, &obstacle.name)?;
                    let (bottom, top) = geometry_z_range(&obstacle.geometry, models)
                        .ok_or_else(|| refuse(format!("model id {} holds no mesh", model_id.0)))?;
                    (silhouette, bottom, top)
                }
            };
            let footprint = grow(&raw_footprint, c, &obstacle.name)?;
            if footprint.is_empty() {
                return Err(refuse("the footprint is empty".to_owned()));
            }
            resolved.push(ResolvedObstacle {
                id: obstacle.id,
                name: obstacle.name.clone(),
                footprint,
                top_z: top + c,
                bottom_z: bottom - c,
                outline: raw_footprint,
                outline_bottom_z: bottom,
                outline_top_z: top,
            });
        }
        Ok(Self {
            obstacles: resolved,
        })
    }

    /// A set of already-resolved obstacles. Tests and probes build one
    /// directly; production reads [`Self::resolve`].
    #[must_use]
    pub fn from_resolved(obstacles: Vec<ResolvedObstacle>) -> Self {
        Self { obstacles }
    }

    /// No enabled obstacle.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.obstacles.is_empty()
    }

    /// The resolved obstacles.
    #[must_use]
    pub fn obstacles(&self) -> &[ResolvedObstacle] {
        &self.obstacles
    }

    /// The highest grown top. `None` for an empty set.
    #[must_use]
    pub fn top_z(&self) -> Option<f64> {
        self.obstacles
            .iter()
            .map(|o| o.top_z)
            .max_by(f64::total_cmp)
    }

    /// THE obstacle plane: the highest grown top plus
    /// [`SAFE_Z_CLEARANCE_MM`]. The tool crosses an obstacle at this Z.
    /// `None` for an empty set.
    #[must_use]
    pub fn plane_z(&self) -> Option<f64> {
        self.top_z().map(|top| top + SAFE_Z_CLEARANCE_MM)
    }

    /// Every grown footprint, for a pre-clip that subtracts them.
    #[must_use]
    pub fn footprints(&self) -> Vec<Polygon2> {
        self.obstacles
            .iter()
            .flat_map(|o| o.footprint.iter().cloned())
            .collect()
    }

    /// The per-tool view: one footprint per assembly band, grown by the
    /// band radius.
    ///
    /// H1: the bands are the cutter, the shank and every hard head part of
    /// `assembly` (each part with a diameter). Two more rules read `stock`,
    /// the fresh stock box of the setup in the setup frame:
    ///
    /// - A hard SHOE part also keeps out of the stock box. The cutter, the
    ///   shank and the fixed head parts do not: the cutter is in the stock
    ///   by design, and the workpiece check (`stock::collision`) judges the
    ///   fixed parts against the part mesh.
    /// - A shelf (a part with no width limit) is a Z floor for the tip: the
    ///   highest top (the stock top and every grown obstacle top) minus the
    ///   shelf height above the tip. The floor holds at every XY.
    ///
    /// # Errors
    /// [`ObstacleResolveError`] when an outward offset fails.
    pub fn for_tool(
        &self,
        assembly: &ToolAssembly,
        stock: Option<&crate::geo::BoundingBox3>,
    ) -> Result<ObstacleToolView, ObstacleResolveError> {
        let mut bands: Vec<(AssemblySegment, f64, f64)> =
            vec![(AssemblySegment::Cutter, 0.0, assembly.cutter_radius)];
        bands.extend(
            assembly
                .segments()
                .into_iter()
                .map(|band| (band.segment, band.z_offset, band.radius)),
        );
        let mut entries = Vec::new();
        for (index, obstacle) in self.obstacles.iter().enumerate() {
            for (band_index, &(segment, offset, radius)) in bands.iter().enumerate() {
                if radius <= 0.0 || !radius.is_finite() {
                    continue;
                }
                let grown = grow(&obstacle.footprint, radius, &obstacle.name)?;
                // K3: only the cutter band (the first band, offset 0) takes
                // part in the territory pre-clip. Its material form is the
                // footprint plus the margin: an area op insets its outline
                // by the cutter radius, and the margin keeps the inset
                // centre path clear of the flattened chords of both
                // offsets.
                let cutter = band_index == 0;
                let material = if cutter {
                    grow(&obstacle.footprint, MATERIAL_MARGIN_MM, &obstacle.name)?
                } else {
                    Vec::new()
                };
                entries.push(BandEntry::new(
                    index,
                    segment,
                    obstacle.top_z - offset,
                    cutter,
                    grown,
                    material,
                ));
            }
        }
        let mut names: Vec<String> = self.obstacles.iter().map(|o| o.name.clone()).collect();
        // H1: the hard shoe parts against the fresh stock box.
        let mut stock_index = None;
        if let Some(stock) = stock {
            let stock_footprint = [Polygon2::rectangle(
                stock.min.x,
                stock.min.y,
                stock.max.x,
                stock.max.y,
            )];
            for &(segment, offset, radius) in &bands {
                if segment != AssemblySegment::Shoe || radius <= 0.0 || !radius.is_finite() {
                    continue;
                }
                let index = *stock_index.get_or_insert(names.len());
                let grown = grow(&stock_footprint, radius, STOCK_NAME)?;
                entries.push(BandEntry::new(
                    index,
                    segment,
                    stock.max.z - offset,
                    false,
                    grown,
                    Vec::new(),
                ));
            }
            if stock_index.is_some() {
                names.push(STOCK_NAME.to_owned());
            }
        }
        // H1: the lowest shelf is a Z floor under the highest top.
        if let Some((shelf, segment)) = assembly.shelf_offset() {
            let stock_top = stock.map(|b| (b.max.z, None));
            let obstacle_top = self
                .obstacles
                .iter()
                .enumerate()
                .map(|(i, o)| (o.top_z, Some(i)))
                .max_by(|a, b| a.0.total_cmp(&b.0));
            let highest = [stock_top, obstacle_top]
                .into_iter()
                .flatten()
                .max_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((top, source)) = highest {
                let index = match source {
                    Some(i) => i,
                    None => *stock_index.get_or_insert_with(|| {
                        names.push(STOCK_NAME.to_owned());
                        names.len() - 1
                    }),
                };
                entries.push(BandEntry::floor(index, segment, top - shelf));
            }
        }
        Ok(ObstacleToolView {
            names,
            stock_index,
            plane_z: self.plane_z(),
            footprints: self.footprints(),
            entries,
        })
    }
}

/// The name a hit on the stock box prints (H1).
const STOCK_NAME: &str = "stock";

/// The margin (mm) of a material footprint ([`BandEntry::material`])
/// past the cutter band. An area op insets its clipped outline by the cutter
/// radius with its own flattened arcs, and the band footprint grows by the
/// flatten deviation. Without a margin the inset path can end a few
/// hundredths of a millimetre inside the band, and the obstacle step then
/// lifts at every row end. 0.1 mm is ten flatten deviations; the cost is
/// 0.1 mm of cut near an obstacle.
const MATERIAL_MARGIN_MM: f64 = 0.1;

/// One obstacle, for one assembly band: the footprint grown by the band
/// radius, and the tip Z below which the band reaches the obstacle.
///
/// H1: a shelf FLOOR is an entry with no footprint that holds at every XY
/// (`everywhere`).
#[derive(Debug, Clone)]
struct BandEntry {
    /// A floor: the entry holds at every XY, and `grown` is empty.
    everywhere: bool,
    obstacle: usize,
    segment: AssemblySegment,
    /// The band strikes the obstacle when the tip is below this Z and the
    /// tip XY is inside `grown`.
    tip_ceiling_z: f64,
    /// The cutter band: the band whose ceiling is the grown top. Only it
    /// takes part in the territory pre-clip (K3).
    cutter: bool,
    grown: Vec<Polygon2>,
    /// For the cutter band, the footprint grown by [`MATERIAL_MARGIN_MM`]:
    /// the region an area op's OUTLINE must lose so that its tool centre,
    /// inset by the cutter radius, keeps out of `grown` (K3). Empty for
    /// the other bands.
    material: Vec<Polygon2>,
    /// The XY box of `grown`: `[x_min, y_min, x_max, y_max]`.
    bbox: [f64; 4],
}

impl BandEntry {
    fn new(
        obstacle: usize,
        segment: AssemblySegment,
        tip_ceiling_z: f64,
        cutter: bool,
        grown: Vec<Polygon2>,
        material: Vec<Polygon2>,
    ) -> Self {
        let mut lo = [f64::INFINITY, f64::INFINITY];
        let mut hi = [f64::NEG_INFINITY, f64::NEG_INFINITY];
        for p in &grown {
            let [x0, y0, x1, y1] = p.bbox();
            lo = [lo[0].min(x0), lo[1].min(y0)];
            hi = [hi[0].max(x1), hi[1].max(y1)];
        }
        Self {
            everywhere: false,
            obstacle,
            segment,
            tip_ceiling_z,
            cutter,
            grown,
            material,
            bbox: [lo[0], lo[1], hi[0], hi[1]],
        }
    }

    /// A shelf floor at `tip_floor_z`, at every XY (H1).
    fn floor(obstacle: usize, segment: AssemblySegment, tip_floor_z: f64) -> Self {
        Self {
            everywhere: true,
            obstacle,
            segment,
            tip_ceiling_z: tip_floor_z,
            cutter: false,
            grown: Vec::new(),
            material: Vec::new(),
            bbox: [
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
                f64::INFINITY,
                f64::INFINITY,
            ],
        }
    }

    /// Does the entry hold at the tip XY `p`?
    fn holds_at(&self, p: &crate::geo::P2) -> bool {
        self.everywhere
            || (p.x >= self.bbox[0]
                && p.x <= self.bbox[2]
                && p.y >= self.bbox[1]
                && p.y <= self.bbox[3]
                && strictly_inside(&self.grown, p))
    }
}

/// A Z tolerance (mm): a tip this close to a band ceiling is at it, not
/// below it.
const Z_EPS_MM: f64 = 1e-6;

/// An XY tolerance (mm): a point this close to a grown footprint edge is on
/// the edge, which the tool touches and does not enter. The clip walk puts
/// each re-entry descent exactly on that edge.
const XY_EPS_MM: f64 = 1e-6;

/// Is `p` inside one of `grown`, deeper than [`XY_EPS_MM`]?
fn strictly_inside(grown: &[Polygon2], p: &crate::geo::P2) -> bool {
    grown
        .iter()
        .any(|g| g.contains_point(p) && g.distance_to_boundary(p) > XY_EPS_MM)
}

/// A parameter tolerance on a move.
const T_EPS: f64 = 1e-9;

/// One obstacle hit on a move: the parameter interval, the obstacle and
/// the band.
#[derive(Debug, Clone, Copy)]
struct Forbidden {
    t0: f64,
    t1: f64,
    obstacle: usize,
    segment: AssemblySegment,
    tip_ceiling_z: f64,
}

/// The obstacles of a setup for one tool. See [`ObstacleSet::for_tool`].
///
/// THE check-height rule: the view forbids a band of the tool below its
/// `tip_ceiling_z`, the GROWN top (top plus clearance) minus the band
/// offset. The grown top is the physical limit. The obstacle plane
/// ([`Self::plane_z`], grown top plus [`SAFE_Z_CLEARANCE_MM`]) is not a
/// limit: it is the height the tool retracts to and crosses at, a margin
/// above the limit.
#[derive(Debug, Clone)]
pub struct ObstacleToolView {
    names: Vec<String>,
    /// H1: the index in `names` of the stock box, when an entry reads it.
    stock_index: Option<usize>,
    plane_z: Option<f64>,
    /// The grown footprints of the set ([`ObstacleSet::footprints`]).
    footprints: Vec<Polygon2>,
    entries: Vec<BandEntry>,
}

impl ObstacleToolView {
    /// No entry: no obstacle, no stock entry and no shelf floor.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// H1: the shelf floor of the view, the lowest tip Z the shelf allows
    /// at any XY. `None` when the head has no shelf or the view has no top
    /// to read.
    #[must_use]
    pub fn floor_z(&self) -> Option<f64> {
        self.entries
            .iter()
            .filter(|e| e.everywhere)
            .map(|e| e.tip_ceiling_z)
            .max_by(f64::total_cmp)
    }

    /// K3: the CUTTER band footprint of every obstacle whose grown top is
    /// above `min_tip_z`, for the territory pre-clip of the tool centre
    /// (the 3D finishes, Waterline, Project Curve, the 3D Rough centre).
    ///
    /// The shank and holder bands are not in it. Whether they reach an
    /// obstacle depends on the real tip Z, which the pre-clip does not
    /// know; read at the op's lowest Z they cut real territory away (the
    /// holder footprint on testpiece_mini, 2026-10-15). The post-dressup
    /// step judges them at the real Z, as in K1.
    #[must_use]
    pub fn cutter_footprints(&self, min_tip_z: f64) -> Vec<Polygon2> {
        self.entries
            .iter()
            .filter(|e| e.cutter && e.tip_ceiling_z > min_tip_z)
            .flat_map(|e| e.grown.iter().cloned())
            .collect()
    }

    /// The material form of [`Self::cutter_footprints`]: each footprint
    /// grown by [`MATERIAL_MARGIN_MM`]. An area op (Pocket, Zigzag, Face,
    /// 2D Adaptive) removes them from its outline before it insets the
    /// outline by the cutter radius (K3).
    #[must_use]
    pub fn cutter_material_footprints(&self, min_tip_z: f64) -> Vec<Polygon2> {
        self.entries
            .iter()
            .filter(|e| e.cutter && e.tip_ceiling_z > min_tip_z)
            .flat_map(|e| e.material.iter().cloned())
            .collect()
    }

    /// The grown footprints of the set, without a band radius
    /// ([`ObstacleSet::footprints`]). The 3D Rough pre-clear reads them.
    #[must_use]
    pub fn footprints(&self) -> &[Polygon2] {
        &self.footprints
    }

    /// The obstacle plane of the set. See [`ObstacleSet::plane_z`].
    #[must_use]
    pub fn plane_z(&self) -> Option<f64> {
        self.plane_z
    }

    /// The parameter intervals of the linear move `a`→`b` on which a band
    /// of the tool is inside an obstacle.
    fn forbidden(&self, a: P3, b: P3) -> Vec<Forbidden> {
        let mut out = Vec::new();
        let (mx0, mx1) = (a.x.min(b.x), a.x.max(b.x));
        let (my0, my1) = (a.y.min(b.y), a.y.max(b.y));
        let dz = b.z - a.z;
        for entry in &self.entries {
            let [x0, y0, x1, y1] = entry.bbox;
            if mx1 < x0 || mx0 > x1 || my1 < y0 || my0 > y1 {
                continue;
            }
            // The Z part: the tip is below the band ceiling.
            let ceiling = entry.tip_ceiling_z - Z_EPS_MM;
            let (z0, z1) = if dz.abs() < 1e-12 {
                if a.z < ceiling { (0.0, 1.0) } else { continue }
            } else {
                let tc = (ceiling - a.z) / dz;
                if dz > 0.0 {
                    (0.0, tc.min(1.0))
                } else {
                    (tc.max(0.0), 1.0)
                }
            };
            if z1 <= z0 {
                continue;
            }
            // The XY part: the tip XY is inside the grown footprint. A
            // stationary move (a vertical one) is a point in XY.
            let xy: Vec<(f64, f64)> = if entry.everywhere {
                vec![(0.0, 1.0)]
            } else if (b.x - a.x).abs() < 1e-12 && (b.y - a.y).abs() < 1e-12 {
                let p = crate::geo::P2::new(a.x, a.y);
                if strictly_inside(&entry.grown, &p) {
                    vec![(0.0, 1.0)]
                } else {
                    Vec::new()
                }
            } else {
                maximal_inside_intervals(a, b, &entry.grown)
            };
            for (s0, s1) in xy {
                let t0 = s0.max(z0);
                let t1 = s1.min(z1);
                if t1 > t0 + T_EPS {
                    out.push(Forbidden {
                        t0,
                        t1,
                        obstacle: entry.obstacle,
                        segment: entry.segment,
                        tip_ceiling_z: entry.tip_ceiling_z,
                    });
                }
            }
        }
        out
    }

    /// Is the tool inside an obstacle with its tip at `p`? Returns the
    /// first band entry that strikes.
    fn point_hit(&self, p: P3) -> Option<&BandEntry> {
        let xy = crate::geo::P2::new(p.x, p.y);
        self.entries
            .iter()
            .find(|e| p.z < e.tip_ceiling_z - Z_EPS_MM && e.holds_at(&xy))
    }

    /// The lowest tip Z at `(x, y)` that keeps every band above the
    /// obstacles: the highest band ceiling whose grown footprint holds the
    /// point. `NEG_INFINITY` where no band reaches an obstacle.
    #[must_use]
    pub fn required_tip_z_at(&self, x: f64, y: f64) -> f64 {
        let xy = crate::geo::P2::new(x, y);
        self.entries
            .iter()
            .filter(|e| e.holds_at(&xy))
            .map(|e| e.tip_ceiling_z)
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// May the tool tip stand at `p`?
    #[must_use]
    pub fn point_allowed(&self, p: P3) -> bool {
        self.point_hit(p).is_none()
    }

    /// The allowed parameter intervals of the linear move `a`→`b`: the
    /// complement of every obstacle interval, sorted and disjoint.
    #[must_use]
    pub fn allowed_intervals(&self, a: P3, b: P3) -> Vec<(f64, f64)> {
        let mut bad: Vec<(f64, f64)> = self.forbidden(a, b).iter().map(|f| (f.t0, f.t1)).collect();
        bad.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut allowed = Vec::new();
        let mut cursor = 0.0_f64;
        for (t0, t1) in bad {
            if t0 > cursor + T_EPS {
                allowed.push((cursor, t0));
            }
            cursor = cursor.max(t1);
        }
        if cursor < 1.0 - T_EPS {
            allowed.push((cursor, 1.0));
        }
        allowed
    }

    /// The chords that an arc move travels, for the obstacle tests.
    fn arc_points(start: P3, m: &Move) -> Vec<P3> {
        match m.move_type {
            MoveType::ArcCW { i, j, .. } => {
                crate::geometry::arc_util::linearize_arc(start, m.target, i, j, true, ARC_CHORD_MM)
            }
            MoveType::ArcCCW { i, j, .. } => {
                crate::geometry::arc_util::linearize_arc(start, m.target, i, j, false, ARC_CHORD_MM)
            }
            MoveType::Rapid | MoveType::Linear { .. } => vec![start, m.target],
        }
    }

    /// The highest band ceiling of the view: the highest grown top. A tip
    /// at or above it clears every obstacle at any XY. `None` for an empty
    /// view.
    #[must_use]
    pub fn max_ceiling_z(&self) -> Option<f64> {
        self.entries
            .iter()
            .map(|e| e.tip_ceiling_z)
            .max_by(f64::total_cmp)
    }

    /// The first forbidden interval on the polyline `points`, as the hit
    /// position and the interval.
    fn polyline_hit(&self, points: &[P3]) -> Option<(P3, Forbidden)> {
        points.windows(2).find_map(|w| {
            let &[a, b] = w else {
                return None;
            };
            self.forbidden(a, b).into_iter().next().map(|f| {
                let t = 0.5 * (f.t0 + f.t1);
                let p = P3::new(
                    a.x + t * (b.x - a.x),
                    a.y + t * (b.y - a.y),
                    a.z + t * (b.z - a.z),
                );
                (p, f)
            })
        })
    }

    /// The hit of the join into the first move `m`: the motion the G-code
    /// builder emits from `start` to `m.target` (see [`JoinStart`]).
    fn join_hit(&self, start: JoinStart, m: &Move) -> Option<(P3, Forbidden)> {
        let t = m.target;
        match start {
            JoinStart::After(p) => self.polyline_hit(&Self::join_points(p, m)),
            JoinStart::Unknown => {
                // The XY before the move is not known: a Z-first rapid
                // traverses at `t.z` from anywhere, and a feed comes from
                // anywhere. Only a target at or above every ceiling is
                // safe.
                let ceiling = self.max_ceiling_z()?;
                if t.z < ceiling - Z_EPS_MM {
                    let e = self
                        .entries
                        .iter()
                        .find(|e| e.tip_ceiling_z >= ceiling - Z_EPS_MM)?;
                    return Some((
                        t,
                        Forbidden {
                            t0: 0.0,
                            t1: 1.0,
                            obstacle: e.obstacle,
                            segment: e.segment,
                            tip_ceiling_z: e.tip_ceiling_z,
                        },
                    ));
                }
                self.point_hit(t).map(|e| {
                    (
                        t,
                        Forbidden {
                            t0: 0.0,
                            t1: 1.0,
                            obstacle: e.obstacle,
                            segment: e.segment,
                            tip_ceiling_z: e.tip_ceiling_z,
                        },
                    )
                })
            }
        }
    }

    /// H1, the shoe solver: how far every band of the view must rise so
    /// that `toolpath` (with its join, as [`Self::check_toolpath`] reads it)
    /// strikes nothing. The answer is the largest
    /// `tip_ceiling_z - z` over every point of every move where a band holds
    /// at the tip XY. `NEG_INFINITY` when no band holds anywhere on it.
    ///
    /// The tip Z is linear on each straight piece, so the lowest tip Z on
    /// an inside interval is at one of its ends: the answer is exact for
    /// lines and for the chords of an arc.
    #[must_use]
    pub fn required_lift(&self, toolpath: &Toolpath, join: JoinStart) -> f64 {
        let mut lift = f64::NEG_INFINITY;
        if self.is_empty() {
            return lift;
        }
        let mut prev: Option<P3> = None;
        for m in &toolpath.moves {
            let points = match (prev, join) {
                (Some(start), _) => Self::arc_points(start, m),
                (None, JoinStart::After(p)) => Self::join_points(p, m),
                (None, JoinStart::Unknown) => {
                    // The XY before the move is not known: the target must
                    // be at or above every ceiling.
                    if let Some(ceiling) = self.max_ceiling_z() {
                        lift = lift.max(ceiling - m.target.z);
                    }
                    vec![m.target, m.target]
                }
            };
            prev = Some(m.target);
            for w in points.windows(2) {
                let &[a, b] = w else {
                    continue;
                };
                lift = lift.max(self.segment_lift(a, b));
            }
        }
        lift
    }

    /// [`Self::required_lift`] for one straight piece `a`→`b`.
    fn segment_lift(&self, a: P3, b: P3) -> f64 {
        let (mx0, mx1) = (a.x.min(b.x), a.x.max(b.x));
        let (my0, my1) = (a.y.min(b.y), a.y.max(b.y));
        let z_at = |t: f64| a.z + t * (b.z - a.z);
        let mut lift = f64::NEG_INFINITY;
        for entry in &self.entries {
            let [x0, y0, x1, y1] = entry.bbox;
            if mx1 < x0 || mx0 > x1 || my1 < y0 || my0 > y1 {
                continue;
            }
            let xy: Vec<(f64, f64)> = if entry.everywhere {
                vec![(0.0, 1.0)]
            } else if (b.x - a.x).abs() < 1e-12 && (b.y - a.y).abs() < 1e-12 {
                if strictly_inside(&entry.grown, &crate::geo::P2::new(a.x, a.y)) {
                    vec![(0.0, 1.0)]
                } else {
                    Vec::new()
                }
            } else {
                maximal_inside_intervals(a, b, &entry.grown)
            };
            for (s0, s1) in xy {
                let low = z_at(s0).min(z_at(s1));
                lift = lift.max(entry.tip_ceiling_z - low);
            }
        }
        lift
    }

    /// The motion the G-code builder emits from `p` into the first move
    /// `m` of a toolpath, as a polyline (see [`JoinStart::After`]).
    fn join_points(p: P3, m: &Move) -> Vec<P3> {
        let t = m.target;
        match m.move_type {
            // `gcode::program_builder::push_rapid`: a rising diagonal lifts
            // first, then traverses; a falling one traverses at the old Z,
            // then plunges.
            MoveType::Rapid => {
                let xy_changed = t.x != p.x || t.y != p.y;
                if xy_changed && t.z != p.z {
                    let corner = if t.z > p.z {
                        P3::new(p.x, p.y, t.z)
                    } else {
                        P3::new(t.x, t.y, p.z)
                    };
                    vec![p, corner, t]
                } else {
                    vec![p, t]
                }
            }
            // A feed or an arc runs from the last position as it is.
            MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => {
                Self::arc_points(p, m)
            }
        }
    }

    /// What a hit on entry obstacle `index` struck: the stock box (H1) or
    /// the named obstacle.
    fn kind_of(&self, index: usize) -> CollisionKind {
        if Some(index) == self.stock_index {
            CollisionKind::Stock
        } else {
            CollisionKind::Obstacle {
                name: self.names.get(index).cloned().unwrap_or_default(),
            }
        }
    }

    /// Every hit of `toolpath`, at most one per move: every move class,
    /// the rapids included, with the cutter, shank and holder. The move
    /// index is the toolpath's own.
    ///
    /// `join` says where the tool is before the first move. The join
    /// motion into the first move is checked as the G-code builder emits
    /// it ([`JoinStart`]); a hit on it is reported at move index 0.
    #[must_use]
    pub fn check_toolpath(&self, toolpath: &Toolpath, join: JoinStart) -> Vec<CollisionEvent> {
        let mut hits = Vec::new();
        if self.is_empty() {
            return hits;
        }
        let mut prev: Option<P3> = None;
        for (move_index, m) in toolpath.moves.iter().enumerate() {
            let hit = match prev {
                Some(start) => self.polyline_hit(&Self::arc_points(start, m)),
                None => self.join_hit(join, m),
            };
            prev = Some(m.target);
            if let Some((position, f)) = hit {
                hits.push(CollisionEvent {
                    move_index,
                    position,
                    penetration_depth: f.tip_ceiling_z - position.z,
                    segment: f.segment,
                    kind: self.kind_of(f.obstacle),
                });
            }
        }
        hits
    }
}

/// Where the tool is before the first move of a toolpath, for the obstacle
/// check of the join (K1). The rule mirrors the G-code builder
/// (`gcode::program_builder`):
///
/// - [`Self::After`]: the previous toolpath of the same setup ran with the
///   same tool. The builder carries its last position. A rapid that rises
///   lifts first, then traverses; a rapid that falls traverses at the old
///   Z, then plunges; a feed runs straight.
/// - [`Self::Unknown`]: the first toolpath of a setup (program start, or
///   after the setup-change pause), or the first after a tool change. The
///   builder forgets the position there (the operator may have jogged), so
///   the rapid goes Z-first and then traverses from an XY that is not
///   known. The check then needs the target at or above every ceiling.
///
/// A raw G-code snippet (`pre_gcode` / `post_gcode`) also makes the builder
/// forget the position. The simulation request does not carry the
/// snippets, so this rule does not model that reset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JoinStart {
    /// The last position of the previous toolpath, same setup, same tool.
    After(P3),
    /// The position is not known.
    Unknown,
}

impl JoinStart {
    /// THE join rule over the toolpaths that run before this one in the
    /// same setup, in run order, as `(tool key, toolpath)`. Both the
    /// simulation and `collision_check` call it.
    ///
    /// The builder fires a tool change at every toolpath whose tool
    /// differs from the loaded one, and the change forgets the position.
    /// An empty toolpath moves nothing but its tool change still fires.
    pub fn after_previous<'a, T: PartialEq + 'a>(
        previous: impl DoubleEndedIterator<Item = (T, &'a Toolpath)>,
        this_tool: &T,
    ) -> Self {
        for (tool, toolpath) in previous.rev() {
            if &tool != this_tool {
                // A tool change fires between that toolpath and this one.
                return Self::Unknown;
            }
            if let Some(last) = toolpath.moves.last() {
                return Self::After(last.target);
            }
        }
        Self::Unknown
    }
}

/// One obstacle hit of a simulation run (K1).
#[derive(Debug, Clone)]
pub struct ObstacleCollision {
    /// The toolpath the move belongs to.
    pub toolpath_id: crate::ids::ToolpathId,
    /// The run-global move index (the simulation timeline).
    pub global_move: usize,
    /// The hit. `event.move_index` is the toolpath's own move index and
    /// `event.kind` is [`CollisionKind::Obstacle`]. The join from the
    /// previous toolpath is not a toolpath move: a hit on it is reported at
    /// move 0 of the toolpath it leads into.
    pub event: CollisionEvent,
}

/// The chord length (mm) the obstacle tests linearise an arc with.
const ARC_CHORD_MM: f64 = 0.25;

/// The obstacles as a [`ClipPermit`]: the per-op step's rule for the
/// shared clip walk. An arc is kept whole only when no chord of it meets
/// an obstacle.
pub(crate) struct ObstaclePermit<'v> {
    pub(crate) view: &'v ObstacleToolView,
}

impl ClipPermit for ObstaclePermit<'_> {
    fn point_allowed(&self, p: P3) -> bool {
        self.view.point_allowed(p)
    }

    fn allowed_intervals(&self, start: P3, end: P3) -> Vec<(f64, f64)> {
        self.view.allowed_intervals(start, end)
    }

    fn arc_allowed(&self, start: P3, m: &Move) -> bool {
        let points = ObstacleToolView::arc_points(start, m);
        points.windows(2).all(|w| match *w {
            [a, b] => self.view.forbidden(a, b).is_empty(),
            _ => true,
        })
    }
}

/// THE per-op step (K1): split every move of `annotated` that would put
/// the tool inside an obstacle. The tool goes up to the obstacle plane (or
/// `safe_z` when it is higher), across, and back down, through the shared
/// clip walk. A move that the check allows keeps its own height: the plane
/// is local to the crossings (K3).
///
/// K3: the step also starts the toolpath at the plane. When the first
/// move is below it, a rapid to the plane at the same XY goes before it.
/// The G-code builder then reaches the toolpath at the plane from any
/// start: a rising join lifts first, then traverses
/// ([`JoinStart::After`]), and a join from an unknown position goes Z
/// first ([`JoinStart::Unknown`]).
///
/// An empty view passes the toolpath through with an identity mapping.
pub fn clip_annotated_to_obstacles(
    annotated: crate::trace::toolpath_spans::AnnotatedToolpath,
    view: &ObstacleToolView,
    safe_z: f64,
    plunge_rate_mm_min: Option<f64>,
) -> crate::trace::transform_provenance::Transformed {
    use crate::trace::toolpath_spans::AnnotatedToolpath;
    use crate::trace::transform_provenance::Transformed;

    let AnnotatedToolpath {
        toolpath,
        spans,
        spans_valid,
        produced,
    } = annotated;
    // H1: a view with only a shelf floor (no obstacle) passes a toolpath
    // that stays above the floor through untouched, and it never adds the
    // K3 start rapid: that rapid is the obstacle join rule.
    let floor_only_and_clear =
        view.plane_z().is_none() && toolpath.moves.iter().all(|m| view.point_allowed(m.target));
    let (clipped, mapping) = if view.is_empty() || floor_only_and_clear {
        let n = toolpath.moves.len();
        (toolpath, (0..=n).collect::<Vec<usize>>())
    } else {
        let plane = view.plane_z().map_or(safe_z, |p| p.max(safe_z));
        let (mut clipped, mut mapping) = crate::geometry::boundary::clip_walk(
            &toolpath,
            &ObstaclePermit { view },
            plane,
            plunge_rate_mm_min,
        );
        if view.plane_z().is_some() {
            start_at_the_plane(&mut clipped, &mut mapping, plane);
        }
        (clipped, mapping)
    };
    let spans: Vec<crate::trace::toolpath_spans::Span> =
        spans.iter().map(|s| s.remap(&mapping)).collect();
    Transformed::from_mapping(
        AnnotatedToolpath {
            toolpath: clipped,
            spans,
            spans_valid,
            produced,
        },
        mapping,
    )
}

/// K3: put a rapid to `plane` at the XY of the first move in front of
/// it, when the first move is below `plane`. The inserted move belongs to
/// input move 0, so every later entry of `mapping` moves up by one. Its
/// intent is [`MoveIntent::ObstacleClearance`]: the G-code drill reader
/// reads that intent, not the move shape.
///
/// [`MoveIntent::ObstacleClearance`]: crate::toolpath::MoveIntent::ObstacleClearance
fn start_at_the_plane(toolpath: &mut Toolpath, mapping: &mut [usize], plane: f64) {
    let Some(first) = toolpath.moves.first() else {
        return;
    };
    if first.target.z >= plane - Z_EPS_MM {
        return;
    }
    let lift = Move {
        target: P3::new(first.target.x, first.target.y, plane),
        move_type: MoveType::Rapid,
        intent: crate::toolpath::MoveIntent::ObstacleClearance,
    };
    toolpath.moves.insert(0, lift);
    for entry in mapping.iter_mut().skip(1) {
        *entry += 1;
    }
}

#[cfg(test)]
#[allow(
    // SAFETY: test code; a failed lookup is a failed test.
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    /// Cutter r = 3 (flutes 20), shank r = 3 for 20, holder r = 17.5 for 40.
    fn assembly() -> ToolAssembly {
        ToolAssembly {
            cutter_radius: 3.0,
            cutter_length: 20.0,
            shank_diameter: 6.0,
            head: vec![crate::stock::collision::HeadBand::holder_at_face(
                35.0, 40.0,
            )],
            stickout: 40.0,
        }
    }

    /// A box clamp x 0..10, y 0..10, top 20, clearance 1.
    fn clamp(top: f64) -> ObstacleSet {
        ObstacleSet::resolve(
            &[Obstacle {
                id: ObstacleId(0),
                name: "Clamp".to_owned(),
                enabled: true,
                clearance: 1.0,
                geometry: ObstacleGeometry::Box {
                    min: [0.0, 0.0, -5.0],
                    max: [10.0, 10.0, top],
                },
            }],
            &[],
        )
        .unwrap()
    }

    /// K2: the resolver keeps the section before the clearance grow, for
    /// the renderer. A box gives its own rectangle and Z range; the grown
    /// footprint contains it and is larger.
    #[test]
    fn a_resolved_obstacle_keeps_its_outline_before_the_clearance_k2() {
        let set = ObstacleSet::resolve(
            &[Obstacle {
                id: ObstacleId(0),
                name: "Clamp".to_owned(),
                enabled: true,
                clearance: 3.0,
                geometry: ObstacleGeometry::Box {
                    min: [0.0, 0.0, -5.0],
                    max: [10.0, 20.0, 15.0],
                },
            }],
            &[],
        )
        .unwrap();
        let resolved = &set.obstacles()[0];
        assert_eq!(resolved.outline.len(), 1);
        assert!((resolved.outline[0].area() - 200.0).abs() < 1e-9);
        assert_eq!(resolved.outline_bottom_z, -5.0);
        assert_eq!(resolved.outline_top_z, 15.0);
        assert_eq!(resolved.top_z, 18.0);
        assert_eq!(resolved.bottom_z, -8.0);
        let grown: f64 = resolved.footprint.iter().map(Polygon2::area).sum();
        assert!(grown > 200.0, "the grown footprint is larger: {grown}");
        for p in &resolved.outline[0].exterior {
            assert!(
                resolved.footprint.iter().any(|f| f.contains_point(p)),
                "the footprint contains the outline corner {p:?}"
            );
        }
    }

    /// K2: two resolves of one mesh obstacle give the same outline and
    /// footprint (the second reads the silhouette memo).
    #[test]
    fn a_mesh_obstacle_resolves_to_the_same_outline_twice_k2() {
        let model = crate::session::LoadedModel {
            id: 4,
            name: "clamp.stl".to_owned(),
            mesh: Some(std::sync::Arc::new(crate::mesh::make_test_hemisphere(
                8.0, 12,
            ))),
            items: Default::default(),
            path: std::path::PathBuf::from("synthetic://clamp.stl"),
            kind: None,
            units: None,
            enriched_mesh: None,
            winding_report: None,
            load_error: None,
        };
        let obstacle = Obstacle {
            id: ObstacleId(0),
            name: "Clamp".to_owned(),
            enabled: true,
            clearance: 2.0,
            geometry: ObstacleGeometry::Model {
                model_id: ModelId(4),
            },
        };
        let models = [model];
        let first = ObstacleSet::resolve(std::slice::from_ref(&obstacle), &models).unwrap();
        let second = ObstacleSet::resolve(std::slice::from_ref(&obstacle), &models).unwrap();
        let rings = |polygons: &[Polygon2]| -> Vec<Vec<crate::geo::P2>> {
            polygons.iter().map(|p| p.exterior.clone()).collect()
        };
        let (a, b) = (&first.obstacles()[0], &second.obstacles()[0]);
        assert!(!a.outline.is_empty());
        assert_eq!(rings(&a.outline), rings(&b.outline));
        assert_eq!(rings(&a.footprint), rings(&b.footprint));
        assert_eq!(a.outline_top_z, b.outline_top_z);
    }

    #[test]
    fn the_plane_is_the_grown_top_plus_the_safe_clearance() {
        let set = clamp(20.0);
        assert_eq!(set.top_z(), Some(21.0));
        assert_eq!(set.plane_z(), Some(21.0 + SAFE_Z_CLEARANCE_MM));
    }

    /// K3: a toolpath that starts below the plane gets a rapid to the
    /// plane in front of it; one that starts at the plane does not. A move
    /// far from the clamp keeps its own height.
    #[test]
    fn the_step_starts_the_toolpath_at_the_plane_k3() {
        let set = clamp(20.0);
        let view = set.for_tool(&assembly(), None).unwrap();
        let plane = set.plane_z().unwrap();
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(-50.0, 5.0, 10.0));
        tp.feed_to(P3::new(-50.0, 5.0, 0.0), 300.0);
        tp.rapid_to(P3::new(-50.0, 5.0, 10.0));
        tp.rapid_to(P3::new(-50.0, 40.0, 10.0));
        let n = tp.moves.len();
        let annotated = crate::trace::toolpath_spans::AnnotatedToolpath::new(tp);
        let mut channels = crate::trace::transform_provenance::ReconcileSet::new(None, None);
        let out = clip_annotated_to_obstacles(annotated, &view, 10.0, Some(300.0))
            .reconcile(&mut channels)
            .into_inner();
        assert_eq!(out.toolpath.moves.len(), n + 1);
        assert_eq!(out.toolpath.moves[0].target, P3::new(-50.0, 5.0, plane));
        assert_eq!(
            out.toolpath.moves[0].intent,
            crate::toolpath::MoveIntent::ObstacleClearance
        );
        // The traverse far from the clamp stays at its own Z.
        assert_eq!(out.toolpath.moves[n].target, P3::new(-50.0, 40.0, 10.0));
        // A second pass changes nothing.
        let again = clip_annotated_to_obstacles(out, &view, 10.0, Some(300.0))
            .reconcile(&mut channels)
            .into_inner();
        assert_eq!(again.toolpath.moves.len(), n + 1);
    }

    #[test]
    fn a_cut_beside_the_clamp_keeps_the_cutter_radius_and_clearance() {
        let view = clamp(20.0).for_tool(&assembly(), None).unwrap();
        // Grown footprint: x -1..11. The cutter (r 3) reaches it below
        // x = -4.
        assert!(!view.point_allowed(P3::new(-3.9, 5.0, 0.0)));
        assert!(view.point_allowed(P3::new(-4.1, 5.0, 0.0)));
        // Above the grown top the cutter passes over.
        assert!(view.point_allowed(P3::new(5.0, 5.0, 21.5)));
    }

    #[test]
    fn the_holder_band_applies_where_it_reaches_the_obstacle() {
        // Tall clamp, top 50 (grown 51). The holder (r 17.5) starts 40
        // above the tip, so with the tip below 11 it reaches the clamp.
        let view = clamp(50.0).for_tool(&assembly(), None).unwrap();
        // 10 mm left of the grown edge: clear of the cutter, inside the
        // holder radius.
        assert!(!view.point_allowed(P3::new(-11.0, 5.0, 0.0)));
        // Same XY with the tip at 12: the holder is above the grown top.
        assert!(view.point_allowed(P3::new(-11.0, 5.0, 12.0)));
        // A low clamp (grown top 21): the holder never reaches it.
        let low = clamp(20.0).for_tool(&assembly(), None).unwrap();
        assert!(low.point_allowed(P3::new(-11.0, 5.0, 0.0)));
    }

    #[test]
    fn a_move_across_the_clamp_is_split_and_crosses_at_the_plane() {
        let set = clamp(20.0);
        let view = set.for_tool(&assembly(), None).unwrap();
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(-20.0, 5.0, 30.0));
        tp.rapid_to(P3::new(-20.0, 5.0, 0.0));
        tp.feed_to(P3::new(30.0, 5.0, 0.0), 1000.0);
        let annotated = crate::trace::toolpath_spans::AnnotatedToolpath::new(tp);
        let mut channels = crate::trace::transform_provenance::ReconcileSet::new(None, None);
        let out = clip_annotated_to_obstacles(annotated, &view, 10.0, Some(300.0))
            .reconcile(&mut channels)
            .into_inner();
        let plane = set.plane_z().unwrap();
        assert!(
            view.check_toolpath(&out.toolpath, JoinStart::Unknown)
                .is_empty()
        );
        assert!(
            out.toolpath
                .moves
                .iter()
                .any(|m| (m.target.z - plane).abs() < 1e-9),
            "the tool crosses at the obstacle plane"
        );
        // The kept cuts end at the grown footprint plus the cutter radius.
        // Each grow adds the flatten deviation (0.01 mm), so the edge is
        // up to 0.05 mm further out, never closer.
        let cut_xs: Vec<f64> = out
            .toolpath
            .moves
            .iter()
            .filter(|m| matches!(m.move_type, MoveType::Linear { .. }) && m.target.z == 0.0)
            .map(|m| m.target.x)
            .collect();
        assert!(
            cut_xs.iter().any(|x| (-4.05..=-4.0).contains(x)),
            "{cut_xs:?}"
        );
        assert!(cut_xs.iter().any(|x| (x - 30.0).abs() < 1e-6), "{cut_xs:?}");
    }

    #[test]
    fn the_check_sees_a_rapid_through_the_clamp() {
        let view = clamp(20.0).for_tool(&assembly(), None).unwrap();
        let mut tp = Toolpath::new();
        tp.rapid_to(P3::new(-20.0, 5.0, 15.0));
        tp.rapid_to(P3::new(30.0, 5.0, 15.0));
        // The join starts where the first move ends: it moves nothing.
        let hits = view.check_toolpath(&tp, JoinStart::After(P3::new(-20.0, 5.0, 15.0)));
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].move_index, 1);
        assert_eq!(
            hits[0].kind,
            CollisionKind::Obstacle {
                name: "Clamp".to_owned()
            }
        );
        assert_eq!(hits[0].segment, AssemblySegment::Cutter);
    }

    #[test]
    fn an_arc_that_sweeps_through_the_clamp_is_not_kept() {
        let view = clamp(20.0).for_tool(&assembly(), None).unwrap();
        // A half circle of radius 15 about (5, -12): both ends are far
        // from the clamp, the middle passes through it.
        let start = P3::new(-10.0, -12.0, 0.0);
        let m = Move {
            target: P3::new(20.0, -12.0, 0.0),
            move_type: MoveType::ArcCW {
                i: 15.0,
                j: 0.0,
                feed_rate: 1000.0,
            },
            intent: crate::toolpath::MoveIntent::Unknown,
        };
        let permit = ObstaclePermit { view: &view };
        assert!(view.point_allowed(start) && view.point_allowed(m.target));
        assert!(!permit.arc_allowed(start, &m));
    }

    #[test]
    fn a_missing_model_refuses_by_name() {
        let err = ObstacleSet::resolve(
            &[Obstacle {
                id: ObstacleId(0),
                name: "Vise".to_owned(),
                enabled: true,
                clearance: 0.0,
                geometry: ObstacleGeometry::Model {
                    model_id: ModelId(9),
                },
            }],
            &[],
        )
        .unwrap_err();
        assert_eq!(err.name, "Vise");
        // A disabled obstacle is not read.
        let ok = ObstacleSet::resolve(
            &[Obstacle {
                id: ObstacleId(0),
                name: "Vise".to_owned(),
                enabled: false,
                clearance: 0.0,
                geometry: ObstacleGeometry::Model {
                    model_id: ModelId(9),
                },
            }],
            &[],
        )
        .unwrap();
        assert!(ok.is_empty());
    }

    /// A grown corner keeps the full distance: no chord of the flattened
    /// arc cuts into it. Two grows (clearance 2, then cutter 3) give at
    /// least 5 mm from the raw box at every point, the corners too.
    #[test]
    fn a_grown_corner_keeps_the_full_distance() {
        let square = Polygon2::new(vec![
            crate::geo::P2::new(60.0, 22.5),
            crate::geo::P2::new(80.0, 22.5),
            crate::geo::P2::new(80.0, 42.5),
            crate::geo::P2::new(60.0, 42.5),
        ]);
        let clearance = grow(&[square], 2.0, "Clamp").unwrap();
        let band = grow(&clearance, 3.0, "Clamp").unwrap();
        let mut min_gap = f64::INFINITY;
        let mut max_gap: f64 = 0.0;
        for p in band.iter().flat_map(|poly| poly.exterior.iter()) {
            let dx = (60.0 - p.x).max(0.0).max(p.x - 80.0);
            let dy = (22.5 - p.y).max(0.0).max(p.y - 42.5);
            let gap = dx.hypot(dy);
            min_gap = min_gap.min(gap);
            max_gap = max_gap.max(gap);
        }
        assert!(
            min_gap >= 5.0 - 1e-9,
            "a vertex is {min_gap} mm from the box"
        );
        assert!(max_gap <= 5.05, "a vertex is {max_gap} mm from the box");
    }
}
