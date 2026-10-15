//! W6.2: the one territory resolver.
//!
//! The territory of an op is where it may cut. It has three parts (AUDIT.md
//! "Territory is three concepts"):
//!
//! 1. XY containment: the boundary region set of the finishes, or the
//!    outline and the centre containment of the 3D Rough planner. Pencil and
//!    the 2D ops read none; the session clips their result after generation.
//! 2. A surface cell mask: the slope band, the steep/shallow threshold, the
//!    flat-area threshold or the unified bands.
//! 3. Material presence: the remaining stock, through the W2.1 guard
//!    ([`StockReference::resolve`]).
//!
//! The prologue resolves the territory ONCE per op ([`Territory::resolve`])
//! from the op's capabilities and its config, and hands it to the adapter
//! on [`super::Prepared`]. An adapter passes the parts its kernel reads. It
//! does not read `ExecutionContext::boundary_regions`, `boundary`,
//! `boundary_centre` or call the stock guard again.

use crate::compute::catalog::{OperationConfig, RestUse, TerritoryXy};
use crate::dexel_stock::TriDexelStock;
use crate::geometry::region_set::RegionSet;
use crate::polygon::Polygon2;
use crate::surface::stock_reference::StockReference;

use super::ExecutionContext;

/// The XY containment of one op, resolved from its
/// [`crate::compute::catalog::Capabilities::territory_xy`].
#[derive(Clone, Copy)]
pub enum XyTerritory<'a> {
    /// The op reads no XY containment while it plans. The session clips the
    /// result after generation.
    ClipAfter,
    /// The boundary region set the op pre-clips to. `None`: no boundary.
    Regions(Option<&'a RegionSet<'a>>),
    /// The 3D Rough planner's boundary inputs.
    PlannerBounds {
        /// The boundary outline (the pre-clear). `None`: no boundary.
        outline: Option<&'a Polygon2>,
        /// The containment polygons of the session's post-generation clip
        /// (G-BOUNDARYPHANTOM). `None`: no boundary, or no clip.
        centre: Option<&'a [Polygon2]>,
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

/// The territory of one op, resolved once by the prologue. Every part is a
/// borrow or a small value, so the type is `Copy`.
#[derive(Clone, Copy)]
pub struct Territory<'a> {
    /// The XY containment.
    pub xy: XyTerritory<'a>,
    /// The surface cell mask. `None`: the op masks no cell by slope.
    pub mask: Option<SurfaceMask>,
    /// The remaining stock as a rest reference, after the W2.1 guard.
    /// `None`: the op reads no reference stock, or there is no mesh to
    /// compare the stock with. A `Planner` op (3D Rough) seeds its planner
    /// from `ExecutionContext::initial_stock` without the guard, and an op
    /// builds its link ceiling from it the same way; neither reads this.
    pub stock: Option<StockReference<'a>>,
}

impl<'a> Territory<'a> {
    /// THE resolver. The prologue calls it once per op.
    ///
    /// The stock part runs the guard only when something reads it: an op
    /// whose rest rule is [`RestUse::Reference`] (Pencil, Unified Finish),
    /// or an enabled generic rest analysis. Both need a mesh.
    pub(super) fn resolve(ctx: &ExecutionContext<'a>, op: &OperationConfig) -> Self {
        let caps = op.capabilities();
        let xy = match caps.territory_xy {
            TerritoryXy::ClipAfter => XyTerritory::ClipAfter,
            TerritoryXy::Regions => XyTerritory::Regions(ctx.boundary_regions),
            TerritoryXy::PlannerBounds => XyTerritory::PlannerBounds {
                outline: ctx.boundary,
                centre: ctx.boundary_centre,
            },
        };
        let reads_stock =
            caps.rest_use == RestUse::Reference || ctx.rest_analysis.is_some_and(|ra| ra.enabled);
        let stock = ctx
            .mesh
            .filter(|_| reads_stock)
            .map(|mesh| StockReference::resolve(ctx.initial_stock, &mesh.bbox));
        Self {
            xy,
            mask: op.surface_mask(),
            stock,
        }
    }

    /// The boundary region set, for an op whose XY part is
    /// [`XyTerritory::Regions`]. `None` for every other XY part.
    pub fn regions(&self) -> Option<&'a RegionSet<'a>> {
        match self.xy {
            XyTerritory::Regions(regions) => regions,
            XyTerritory::ClipAfter | XyTerritory::PlannerBounds { .. } => None,
        }
    }

    /// The remaining stock when the guard accepted it as the rest reference.
    pub fn reference_stock(&self) -> Option<&'a TriDexelStock> {
        self.stock.and_then(StockReference::stock)
    }
}
