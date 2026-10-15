//! W6.2: the one territory resolver.
//!
//! The territory of an op is where it may cut. It has three parts (AUDIT.md
//! "Territory is three concepts"):
//!
//! 1. XY containment: the boundary region set of the finishes, or the
//!    outline and the centre containment of the 3D Rough planner. Pencil and
//!    the 2D ops read none; the session clips their result after generation.
//!    Pin Drill reads none and the session does not clip it (D2, `SetupMap`).
//! 2. A surface cell mask: the slope band, the steep/shallow threshold, the
//!    flat-area threshold or the unified bands.
//! 3. Material presence: the remaining stock, through the W2.1 guard
//!    ([`StockReference::resolve`]).
//!
//! The prologue resolves the territory ONCE per op ([`Territory::resolve`])
//! from the op's capabilities and its config, and hands it to the adapter
//! on [`super::Prepared`]. An adapter passes the parts its kernel reads. It
//! does not call the stock guard again.
//!
//! # The inputs (C3)
//!
//! The boundary inputs and the remaining stock arrive on
//! `ExecutionContext::territory_inputs` ([`TerritoryInputs`]). Its fields
//! are private to this module, so the compiler refuses an op that reads
//! them around the territory. The caller writes them through the builder
//! methods; only [`Territory::resolve`] reads them.
//!
//! # The obstacle pre-clip (K3)
//!
//! This resolver is the ONE place where the setup's obstacles leave an
//! op's territory before the op plans, so the op routes around a clamp and
//! does not lift over it. The session's obstacle step after the dressups
//! stays the safety net; the pre-clip only saves lifts.
//!
//! - A [`XyTerritory::Regions`] op (the 3D finishes, Waterline, Project
//!   Curve) loses the cutter band footprints
//!   ([`crate::compute::obstacle::ObstacleToolView::cutter_footprints`])
//!   from its region set. Without a boundary the set is one open rectangle
//!   over the stock and the mesh, so the op reads a set only when an
//!   obstacle meets it.
//! - The 3D Rough planner ([`XyTerritory::PlannerBounds`]) reads the
//!   obstacle footprints as holes in its pre-clear outline and the cutter
//!   band footprints as holes in its centre containment.
//! - An area op (Pocket, Zigzag, Face, 2D Adaptive) removes the cutter
//!   material footprints from its outlines ([`Territory::area_outlines`]);
//!   it insets the result by the cutter radius as it insets any outline.
//! - A [`XyTerritory::SetupMap`] op (Pin Drill, D2) reads no XY territory
//!   and gets no pre-clip: its holes are absolute setup-map positions. The
//!   session's obstacle step still applies to it.
//!
//! The pre-clip reads the CUTTER band only. The shank and holder bands
//! reach an obstacle only below their own ceilings, which depend on the
//! real tip Z; read at the op's lowest Z they removed real territory
//! (testpiece_mini front finish, 2026-10-15: 12918 mm of cut fell to
//! 7601 mm). The post-dressup step judges them at the real Z, as in K1.
//! The cutter footprints are XY only, so the pre-clip can still leave a
//! cut over a low obstacle (grown top below the surface) to no op; it
//! never allows a cut into one.

use std::borrow::Cow;

use crate::compute::catalog::{OperationConfig, TerritoryXy};
use crate::compute::obstacle::ObstacleToolView;
use crate::dexel_stock::TriDexelStock;
use crate::geometry::boundary::subtract_keepouts;
use crate::geometry::region_set::RegionSet;
use crate::polygon::Polygon2;
use crate::surface::stock_reference::StockReference;
use crate::tool::MillingCutter;

use super::ExecutionContext;

/// The XY containment of one op, resolved from its
/// [`crate::compute::catalog::Capabilities::territory_xy`], with the
/// obstacle pre-clip applied (module doc).
#[derive(Clone)]
pub enum XyTerritory<'a> {
    /// The op reads no XY containment while it plans. The session clips the
    /// result after generation.
    ClipAfter,
    /// The op reads no XY containment, and the session does not clip the
    /// result (D2: absolute setup-map positions, Pin Drill).
    SetupMap,
    /// The boundary region set the op pre-clips to. `None`: no boundary and
    /// no obstacle in reach.
    Regions(Option<RegionSet<'a>>),
    /// The 3D Rough planner's boundary inputs.
    PlannerBounds {
        /// The boundary outline (the pre-clear). `None`: no boundary and no
        /// obstacle.
        outline: Option<Cow<'a, Polygon2>>,
        /// The containment polygons of the session's post-generation clip
        /// (G-BOUNDARYPHANTOM), less the obstacle cutter footprints. `None`:
        /// no boundary and no obstacle.
        centre: Option<Cow<'a, [Polygon2]>>,
    },
}

/// The surface cell mask of one op: the cells of the surface it cuts, by
/// slope. The config states the thresholds; [`OperationConfig::surface_mask`]
/// reads them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SurfaceMask {
    /// Cut only where the surface slope lies in `[from_deg, to_deg]`. Only
    /// an ACTIVE band is a mask (`finish_setup::slope_filter_active`).
    /// Drop Cutter, Scallop, Ramp Finish.
    SlopeBand { from_deg: f64, to_deg: f64 },
    /// Split at a slope: the steep cells get the contour pass and the
    /// shallow cells get the raster. Steep/Shallow.
    SteepShallowSplit { threshold_deg: f64 },
    /// Only the cells flatter than this slope. Horizontal Finish.
    Flat { max_slope_deg: f64 },
    /// The unified planner's band thresholds. Unified Finish.
    Bands {
        steep_threshold_deg: f64,
        waterline_threshold_deg: f64,
    },
}

/// The inputs of [`Territory::resolve`]: the boundary inputs and the
/// remaining stock, all in the emission frame. The caller builds them with
/// [`Self::default`] and the builder methods. The fields are private to this
/// module (C3): an op reads them through [`super::Prepared::territory`]
/// only.
#[derive(Clone, Copy, Default)]
pub struct TerritoryInputs<'a> {
    /// The machined stock the op cuts. `None`: a fresh op.
    stock: Option<&'a TriDexelStock>,
    /// The single boundary outline, the 3D Rough pre-clear.
    boundary: Option<&'a Polygon2>,
    /// P2.3: the multi-region set the mesh-finish family pre-clips
    /// generation to, so sampling never wastes work outside the machining
    /// boundary. `boundary` stays the 3D Rough single-polygon pre-clear.
    regions: Option<&'a RegionSet<'a>>,
    /// G-BOUNDARYPHANTOM: the containment polygons the session's
    /// post-generation boundary clip clips this toolpath to (the regions
    /// after keep-outs, the user offset and the tool-radius containment).
    /// The 3D Rough planner keeps its tool centre inside them so it stamps
    /// only what the clip keeps. `None`: no boundary, or a caller that
    /// applies no clip.
    centre: Option<&'a [Polygon2]>,
}

impl<'a> TerritoryInputs<'a> {
    /// The remaining stock the generator is seeded with. `None`: a fresh op.
    #[must_use]
    pub(crate) fn stock(mut self, stock: Option<&'a TriDexelStock>) -> Self {
        self.stock = stock;
        self
    }

    /// The single boundary outline (the 3D Rough pre-clear).
    #[must_use]
    pub(crate) fn boundary(mut self, boundary: Option<&'a Polygon2>) -> Self {
        self.boundary = boundary;
        self
    }

    /// The boundary region set of the mesh-finish family (P2.3).
    #[must_use]
    pub(crate) fn regions(mut self, regions: Option<&'a RegionSet<'a>>) -> Self {
        self.regions = regions;
        self
    }

    /// The centre containment of the post-generation clip
    /// (G-BOUNDARYPHANTOM).
    #[must_use]
    pub(crate) fn centre(mut self, centre: Option<&'a [Polygon2]>) -> Self {
        self.centre = centre;
        self
    }
}

/// The territory of one op, resolved once by the prologue. The obstacle
/// pre-clip makes owned polygons, so the type is not `Copy`; an adapter
/// reads it by reference ([`super::Prepared::territory`]).
#[derive(Clone)]
pub struct Territory<'a> {
    /// The XY containment.
    pub xy: XyTerritory<'a>,
    /// The surface cell mask. `None`: the op masks no cell by slope.
    pub mask: Option<SurfaceMask>,
    /// The remaining stock as a rest reference, after the W2.1 guard.
    /// `None`: the op reads no reference stock, or there is no mesh to
    /// compare the stock with. The readers whose rule needs the stock
    /// without the guard read [`Self::unguarded_stock`].
    pub stock: Option<StockReference<'a>>,
    /// The remaining stock without the guard. Private (C3): see
    /// [`Self::unguarded_stock`].
    unguarded: Option<&'a TriDexelStock>,
    /// K1: the setup's obstacles for this op's tool. Every op keeps out of
    /// them with no opt-in: the link ceiling lifts over them and the
    /// session clips the result after the dressups. `None`: no obstacle.
    pub obstacles: Option<&'a ObstacleToolView>,
}

impl<'a> Territory<'a> {
    /// THE resolver. The prologue calls it once per op.
    ///
    /// The stock part runs the guard only when something reads it: an op
    /// whose rest rule reads the reference stock
    /// (`RestUse::reads_reference_stock`: `Reference` for Pencil and
    /// Unified Finish, `ContourFilter` for Waterline and Steep and Shallow,
    /// C3), or an enabled generic rest analysis. All of them need a mesh.
    pub(super) fn resolve(ctx: &ExecutionContext<'a>, op: &OperationConfig) -> Self {
        let caps = op.capabilities();
        let inputs = ctx.territory_inputs;
        let obstacles = ctx.obstacles.filter(|view| !view.is_empty());
        let xy = match caps.territory_xy {
            TerritoryXy::ClipAfter => XyTerritory::ClipAfter,
            TerritoryXy::SetupMap => XyTerritory::SetupMap,
            TerritoryXy::Regions => {
                XyTerritory::Regions(regions_less_obstacles(ctx, &inputs, obstacles))
            }
            TerritoryXy::PlannerBounds => planner_bounds_less_obstacles(ctx, &inputs, obstacles),
        };
        let reads_stock =
            caps.rest_use.reads_reference_stock() || ctx.rest_analysis.is_some_and(|ra| ra.enabled);
        let stock = ctx
            .mesh
            .filter(|_| reads_stock)
            .map(|mesh| StockReference::resolve(inputs.stock, &mesh.bbox));
        Self {
            xy,
            mask: op.surface_mask(),
            stock,
            unguarded: inputs.stock,
            obstacles,
        }
    }

    /// The remaining stock WITHOUT the W2.1 guard. `None`: a fresh op.
    ///
    /// The guard compares the stock with the mesh, so a reader that has no
    /// mesh, or that must read the stock as it is, reads this. The readers
    /// are a closed list (C3); a new reader is a design decision:
    ///
    /// - the link ceiling (`shared::op_link_ceiling`, G-LINKCEIL);
    /// - the rest rules of the 2D ops: `LevelClip` and `LevelSkip`
    ///   (`layered::run_layered`), `LevelMask` (2D Adaptive), and
    ///   `StockColumn` (Drill);
    /// - the RAW seed of a `Planner` op (`volumetric::run_volumetric`);
    /// - the Pencil entry ramps (`PencilStock::remaining`);
    /// - `BuriedCurveFinding` (`curve::run_curve`).
    ///
    /// A rest rule that compares the stock with the surface reads
    /// [`Self::reference_stock`].
    pub(super) fn unguarded_stock(&self) -> Option<&'a TriDexelStock> {
        self.unguarded
    }

    /// The boundary region set, for an op whose XY part is
    /// [`XyTerritory::Regions`]. `None` for every other XY part.
    pub fn regions(&self) -> Option<&RegionSet<'a>> {
        match &self.xy {
            XyTerritory::Regions(regions) => regions.as_ref(),
            XyTerritory::ClipAfter | XyTerritory::SetupMap | XyTerritory::PlannerBounds { .. } => {
                None
            }
        }
    }

    /// The planner's outline and centre containment, for an op whose XY
    /// part is [`XyTerritory::PlannerBounds`]. `(None, None)` for every
    /// other XY part.
    pub fn planner_bounds(&self) -> (Option<&Polygon2>, Option<&[Polygon2]>) {
        match &self.xy {
            XyTerritory::PlannerBounds { outline, centre } => {
                (outline.as_deref(), centre.as_deref())
            }
            XyTerritory::ClipAfter | XyTerritory::SetupMap | XyTerritory::Regions(_) => {
                (None, None)
            }
        }
    }

    /// The remaining stock when the guard accepted it as the rest reference.
    pub fn reference_stock(&self) -> Option<&'a TriDexelStock> {
        self.stock.and_then(StockReference::stock)
    }

    /// K3: the outlines of an AREA op (Pocket, Zigzag, Face, 2D Adaptive)
    /// less the cutter material footprints of the obstacles
    /// ([`ObstacleToolView::cutter_material_footprints`]) whose grown top
    /// is above `min_tip_z`, the op's lowest level. The op insets the
    /// result by the cutter radius, so its tool centre keeps out of the
    /// cutter band footprint. The shank and holder bands stay with the
    /// post-dressup step, judged at the real Z. The outlines come back unchanged (borrowed) when no
    /// footprint meets them.
    ///
    /// A contour op (Profile, Trace, Chamfer) does not call it: a clipped
    /// outline cuts a new wall. The session's obstacle step clips it.
    pub fn area_outlines<'p>(
        &self,
        outlines: Cow<'p, [Polygon2]>,
        min_tip_z: f64,
    ) -> Cow<'p, [Polygon2]> {
        let Some(view) = self.obstacles else {
            return outlines;
        };
        match subtract_footprints(&outlines, &view.cutter_material_footprints(min_tip_z)) {
            Some(clipped) => Cow::Owned(clipped),
            None => outlines,
        }
    }
}

/// The region set of a [`TerritoryXy::Regions`] op, less the cutter band
/// footprints of the obstacles. Without a boundary the base is
/// [`open_rectangle`]. `None` when there is no boundary and no footprint
/// meets the base: the op then reads no set, as with no obstacle.
fn regions_less_obstacles<'a>(
    ctx: &ExecutionContext<'a>,
    inputs: &TerritoryInputs<'a>,
    obstacles: Option<&'a ObstacleToolView>,
) -> Option<RegionSet<'a>> {
    let base: Option<&'a [Polygon2]> = inputs.regions.map(|regions| regions.as_slice());
    let Some(view) = obstacles else {
        return base.map(RegionSet::from_slice);
    };
    let holes = view.cutter_footprints(ctx.stock_bbox.min.z);
    let open;
    let polygons: &[Polygon2] = match base {
        Some(regions) => regions,
        None => {
            open = [open_rectangle(ctx)];
            &open
        }
    };
    match subtract_footprints(polygons, &holes) {
        Some(clipped) => Some(RegionSet::new(clipped)),
        None => base.map(RegionSet::from_slice),
    }
}

/// The planner bounds of 3D Rough. Without obstacles they are the
/// boundary inputs as they are. With obstacles:
///
/// - the outline is the boundary outline (its keep-outs are already
///   subtracted by the session), or without a boundary the stock one tool
///   diameter wider on each side, less the obstacle footprints. The
///   planner pre-clears the cells under them and never plans a cut there.
///   A boundary whose regions do not union to one outline gives none;
/// - the centre is the boundary centre, or the outline exterior, less the
///   cutter band footprints of the obstacles above the stock bottom
///   (G-BOUNDARYPHANTOM: the planner stamps only the cut the obstacle step
///   keeps).
fn planner_bounds_less_obstacles<'a>(
    ctx: &ExecutionContext<'a>,
    inputs: &TerritoryInputs<'a>,
    obstacles: Option<&'a ObstacleToolView>,
) -> XyTerritory<'a> {
    let Some(view) = obstacles else {
        return XyTerritory::PlannerBounds {
            outline: inputs.boundary.map(Cow::Borrowed),
            centre: inputs.centre.map(Cow::Borrowed),
        };
    };
    let outline: Option<Cow<'a, Polygon2>> = match (inputs.boundary, inputs.centre) {
        (Some(outline), _) => Some(Cow::Borrowed(outline)),
        // A boundary whose regions do not union to one outline: the
        // planner keeps no outline, as without obstacles.
        (None, Some(_)) => None,
        (None, None) => {
            let b = ctx.stock_bbox;
            let margin = ctx.tool_def.diameter();
            let rect = Polygon2::rectangle(
                b.min.x - margin,
                b.min.y - margin,
                b.max.x + margin,
                b.max.y + margin,
            );
            Some(Cow::Owned(subtract_keepouts(&rect, view.footprints())))
        }
    };
    let base_centre: Vec<Polygon2> = match (inputs.centre, outline.as_deref()) {
        (Some(centre), _) => centre.to_vec(),
        (None, Some(outline)) => vec![Polygon2::new(outline.exterior.clone())],
        (None, None) => Vec::new(),
    };
    let holes = view.cutter_footprints(ctx.stock_bbox.min.z);
    let centre: Vec<Polygon2> = base_centre
        .iter()
        .map(|p| subtract_keepouts(p, &holes))
        .collect();
    XyTerritory::PlannerBounds {
        outline,
        centre: (!centre.is_empty()).then_some(Cow::Owned(centre)),
    }
}

/// The base of a [`TerritoryXy::Regions`] op without a boundary: a
/// rectangle over the stock and the mesh, one tool envelope radius and
/// [`OPEN_MARGIN_MM`] wider on each side. Every sample the op makes lies
/// inside it, so it confines nothing; only the obstacle holes in it act.
fn open_rectangle(ctx: &ExecutionContext<'_>) -> Polygon2 {
    let mut b = *ctx.stock_bbox;
    if let Some(mesh) = ctx.mesh {
        b.min.x = b.min.x.min(mesh.bbox.min.x);
        b.min.y = b.min.y.min(mesh.bbox.min.y);
        b.max.x = b.max.x.max(mesh.bbox.max.x);
        b.max.y = b.max.y.max(mesh.bbox.max.y);
    }
    let margin = ctx.tool_def.envelope_radius_mm() + OPEN_MARGIN_MM;
    Polygon2::rectangle(
        b.min.x - margin,
        b.min.y - margin,
        b.max.x + margin,
        b.max.y + margin,
    )
}

/// The extra margin (mm) of [`open_rectangle`] past the tool envelope.
const OPEN_MARGIN_MM: f64 = 1.0;

/// `regions` less every footprint in `holes`, by a true polygon
/// difference: a footprint can cross a region edge, and an op that offsets
/// its region (the Scallop rings, the area ops) needs a valid polygon.
/// `None` when no footprint box meets a region box: nothing changes.
fn subtract_footprints(regions: &[Polygon2], holes: &[Polygon2]) -> Option<Vec<Polygon2>> {
    let meets =
        |a: [f64; 4], b: [f64; 4]| a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3];
    let near: Vec<Polygon2> = holes
        .iter()
        .filter(|h| regions.iter().any(|r| meets(r.bbox(), h.bbox())))
        .cloned()
        .collect();
    if near.is_empty() {
        return None;
    }
    let mut out = regions.to_vec();
    for hole in Polygon2::union_all(&near) {
        let hole_box = hole.bbox();
        out = out
            .into_iter()
            .flat_map(|region| {
                if meets(region.bbox(), hole_box) {
                    region.difference(&hole)
                } else {
                    vec![region]
                }
            })
            .collect();
    }
    Some(out)
}
