//! W6.4.1: the rest DETECTOR stage. Three sites measure rest material with
//! the rest-depth field: the Pencil `RestDepth` arm, the Unified Finish
//! claims pipeline and the generic post-generation rest analysis. Each one
//! calls [`detect_rest`].
//!
//! # What the stage owns
//!
//! - The ONE reference rule ([`RestReferenceChoice`]): the machined stock,
//!   else the caller's reference tool, else the bare-surface probe with the
//!   sign flipped. The probe ball is built here and nowhere else.
//! - The call of [`detect_rest_valleys`].
//! - The wrap of the rest grid and the rest region polygons into the
//!   [`Produced`] side channel (W6.2), which a `DerivedRestRegions`
//!   consumer reads.
//!
//! # What each caller keeps
//!
//! The POLICY: which tiers it offers and its own detector dials. They
//! differ on purpose:
//!
//! - Pencil offers the stock, then its real reference tool or a nominal
//!   ball bigger than the pencil tool (`ResolvedReference`).
//! - The generic analysis offers the stock, then its configured reference
//!   tool. It has no nominal tier.
//! - Unified Finish offers the stock only when its resolved
//!   `CreaseReference` is `MachinedStock`, and no tool: the claims are
//!   GEOMETRIC unless the operator asks for the machined reference.

use crate::dexel_stock::TriDexelStock;
use crate::geometry::region_mask::RegionCapReport;
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::surface::rest_field::{
    ClearingRegion, RestCenterline, RestFieldParams, RestFieldReport, RestReference,
    detect_rest_valleys,
};
use crate::tool::{BallEndmill, MillingCutter};
use crate::trace::produced::Produced;

/// Diameter (mm) of the bare-surface probe ball: the reference of last
/// resort. Small enough that it reaches every valley the model has.
pub(crate) const SURFACE_PROBE_BALL_DIAMETER_MM: f64 = 0.1;
/// Overall tool length (mm) paired with [`SURFACE_PROBE_BALL_DIAMETER_MM`].
/// The length does not change the ball-tip drop.
pub(crate) const SURFACE_PROBE_BALL_LENGTH_MM: f64 = 10.0;

/// The bare-surface probe ball.
#[must_use]
pub(crate) fn surface_probe_ball() -> BallEndmill {
    BallEndmill::new(SURFACE_PROBE_BALL_DIAMETER_MM, SURFACE_PROBE_BALL_LENGTH_MM)
}

/// The tiers a caller offers, in the order the stage tries them.
#[derive(Clone, Copy, Default)]
pub struct RestReferenceChoice<'a> {
    /// The machined stock, after the shared W2.1 guard. First tier.
    pub stock: Option<&'a TriDexelStock>,
    /// A reference cutter: a real library tool, or a nominal ball. Second
    /// tier.
    pub tool: Option<&'a dyn MillingCutter>,
}

/// Which tier the stage used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestTier {
    /// The machined stock.
    Stock,
    /// The caller's reference cutter.
    Tool,
    /// The bare-surface probe, sign flipped: `rest = pencil_z - probe_z`.
    SelfProbe,
}

impl RestReferenceChoice<'_> {
    /// THE reference rule: the stock, else the tool, else the probe.
    fn resolve<'r>(&self, probe: &'r BallEndmill) -> (RestReference<'r>, RestTier)
    where
        Self: 'r,
    {
        if let Some(stock) = self.stock {
            (RestReference::Stock(stock), RestTier::Stock)
        } else if let Some(tool) = self.tool {
            (
                RestReference::Cutter {
                    tool,
                    is_surface_probe: false,
                },
                RestTier::Tool,
            )
        } else {
            (
                RestReference::Cutter {
                    tool: probe,
                    is_surface_probe: true,
                },
                RestTier::SelfProbe,
            )
        }
    }
}

/// What the stage gives back: the detector's paths and reports, and its
/// grid and regions already wrapped as [`Produced`].
pub struct RestDetection {
    /// The tier the reference came from.
    pub tier: RestTier,
    /// The pencil-routed centrelines.
    pub centerlines: Vec<RestCenterline>,
    /// The wide regions routed to clearing.
    pub clearing_regions: Vec<ClearingRegion>,
    /// The detection metrics.
    pub report: RestFieldReport,
    /// The region count before and after the region cap.
    pub region_cap: RegionCapReport,
    /// `rest_grid` and `rest_regions`, both `Some`.
    pub produced: Produced,
}

/// Run the rest-depth detector with `cutter` as the fine tool, against the
/// first tier `choice` offers.
pub fn detect_rest(
    mesh: &TriangleMesh,
    index: &SpatialIndex,
    cutter: &dyn MillingCutter,
    choice: RestReferenceChoice<'_>,
    params: &RestFieldParams,
) -> RestDetection {
    let probe = surface_probe_ball();
    let (reference, tier) = choice.resolve(&probe);
    let rf = detect_rest_valleys(mesh, index, cutter, reference, params);
    RestDetection {
        tier,
        centerlines: rf.centerlines,
        clearing_regions: rf.clearing_regions,
        report: rf.report,
        region_cap: rf.region_cap,
        produced: Produced {
            rest_grid: Some(std::sync::Arc::new(rf.rest_grid)),
            rest_regions: Some(std::sync::Arc::new(rf.region_polygons)),
            ..Produced::default()
        },
    }
}
