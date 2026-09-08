//! Session-side resolution of a per-tool reach map — P5, 2026-09-08.
//!
//! Two entry points, deliberately split:
//!
//! * [`ProjectSession::reach_map_spec`] resolves everything the walk needs —
//!   mesh, spatial index, cutter, tolerance, cell — and does **no**
//!   drop-cutter work. It is cheap enough to call from the UI thread.
//! * [`ProjectSession::reach_map_for`] is the memoised build. It takes
//!   seconds on a cold key, so it belongs on a worker.
//!
//! The split is what lets the GUI keep the walk off the render loop without
//! lending the whole session out: [`crate::reach_map::ReachMapRequest`] is
//! `Send`, so the UI thread resolves one and a background thread calls
//! [`crate::reach_map_cache::cached_reach_map`] on it. Nothing in the session
//! is borrowed while the walk runs.

use std::sync::Arc;

use crate::compute::cutter::build_cutter;
use crate::feeds::ToolGeometryHint;
use crate::interrupt::{CancelCheck, Cancelled};
use crate::reach_map::{
    DEFAULT_REACH_TOLERANCE_MM, ReachMap, ReachMapParams, ReachMapRequest, ReachToleranceSource,
};
use crate::tool::MillingCutter;

use super::{ProjectSession, SetupEvalContext};

/// The TIP-sphere radius (mm) that forms a raster cusp, or `None` for a
/// geometry that leaves no cusp between passes.
///
/// Mirrors the canonical selection in `feeds::calculate`'s Step 3b and
/// `feeds::cutter_constraints::max_doc_scallop`: a **ball** cusps on its full
/// radius, a **tapered ball** on its tip sphere, and flat / bull / V-bit
/// geometries have no spherical tip — `DropCutterConfig::scallop_height`'s own
/// doc says so in those words ("no effect on flat/bull tools"). Note that
/// `cusp_radius_mm()` cannot answer this: on a flat endmill it returns the
/// full radius, which would invent a cusp the tool does not leave.
fn tip_sphere_radius_mm(cutter: &dyn MillingCutter) -> Option<f64> {
    match cutter.geometry_hint() {
        ToolGeometryHint::Ball => Some(cutter.radius()).filter(|r| *r > 0.0),
        ToolGeometryHint::TaperedBall { tip_radius, .. } => Some(tip_radius).filter(|r| *r > 0.0),
        ToolGeometryHint::Flat | ToolGeometryHint::Bull { .. } | ToolGeometryHint::VBit { .. } => {
            None
        }
    }
}

impl ProjectSession {
    /// The reach tolerance (mm) for one toolpath, and where it came from.
    ///
    /// Three rungs, in order:
    ///
    /// 1. the operation's own declared cusp / scallop height;
    /// 2. the **cusp its own lateral raster leaves**, `R − sqrt(R² − (s/2)²)`
    ///    on the tool's TIP-sphere radius, for the operations whose stepover
    ///    is a surface raster spacing
    ///    ([`crate::compute::catalog::OperationType::lateral_raster_stepover`]);
    /// 3. [`DEFAULT_REACH_TOLERANCE_MM`].
    ///
    /// Rung 2 is F2 (2026-09-08). `drop_cutter` declares a stepover, not a
    /// scallop height, so every raster finish fell to the 0.05 mm default —
    /// and on the operator's wanaka pass (R2.0 tip, 1.5 mm stepover) its own
    /// cusp is **0.146 mm**, so the map was judging the surface against a bar
    /// three times finer than the pass spacing can deliver. The independent
    /// measurement of that terrain reads 55 % of the area missed at 0.05 mm
    /// and 39 % at 0.146 mm: same surface, same tool, and only the second
    /// number answers a question the operator can act on.
    ///
    /// A flat or bull tip leaves no cusp between passes, so it keeps the
    /// default however coarse its stepover — see [`tip_sphere_radius_mm`].
    ///
    /// `stock_to_leave` is deliberately **not** folded in. It is an intended
    /// uniform vertical offset, while the reach gap is measured against the
    /// true mesh; adding it would raise the bar by the leave amount and hide
    /// a real miss of exactly that size behind an intended one.
    #[must_use]
    pub fn reach_tolerance_for(&self, toolpath_index: usize) -> (f64, ReachToleranceSource) {
        let Some(tc) = self.toolpath_configs.get(toolpath_index) else {
            return (DEFAULT_REACH_TOLERANCE_MM, ReachToleranceSource::Default);
        };
        if let Some(declared) = tc
            .operation
            .scallop_height()
            .filter(|h| h.is_finite() && *h > 0.0)
        {
            return (declared, ReachToleranceSource::DeclaredScallopHeight);
        }
        let derived = tc
            .operation
            .op_type()
            .lateral_raster_stepover()
            .then(|| tc.operation.stepover())
            .flatten()
            .filter(|s| s.is_finite() && *s > 0.0)
            .and_then(|stepover_mm| {
                let tool = self.find_tool_by_raw_id(tc.tool_id)?;
                let tip_radius_mm = tip_sphere_radius_mm(&build_cutter(tool))?;
                let cusp = crate::scallop_math::scallop_height_flat(tip_radius_mm, stepover_mm);
                cusp.is_finite().then_some((
                    cusp,
                    ReachToleranceSource::CuspOfStepover {
                        stepover_mm,
                        tip_radius_mm,
                    },
                ))
            })
            // A cusp at or under the bare default is not an improvement on
            // it: a stepover fine enough to beat 0.05 mm has already met the
            // finish bar the default names, and the default is the coarser,
            // more forgiving of the two.
            .filter(|(cusp, _)| *cusp > DEFAULT_REACH_TOLERANCE_MM);
        derived.unwrap_or((DEFAULT_REACH_TOLERANCE_MM, ReachToleranceSource::Default))
    }

    /// [`Self::reach_tolerance_for`] with a caller override folded in — the
    /// pair every reach-map surface should quote, resolved once so the map,
    /// the reply and the panel cannot disagree about the bar or its source.
    #[must_use]
    pub fn reach_tolerance_with_override(
        &self,
        toolpath_index: usize,
        tolerance_override: Option<f64>,
    ) -> (f64, ReachToleranceSource) {
        match tolerance_override.filter(|t| t.is_finite() && *t > 0.0) {
            Some(tolerance) => (tolerance, ReachToleranceSource::CallerOverride),
            None => self.reach_tolerance_for(toolpath_index),
        }
    }

    /// Resolve a reach-map walk for one toolpath, or `None` when the
    /// operation is not one a reach map speaks about
    /// ([`crate::compute::catalog::OperationType::supports_reach_map`]), its
    /// model carries no mesh, or its tool is missing.
    ///
    /// `tolerance_override` replaces the operation's own tolerance — the MCP
    /// surface's probe dial. Pass `None` for the operator-facing answer.
    ///
    /// Does no drop-cutter work: the two geometry lookups it makes
    /// ([`crate::geom_cache::cached_transform`] and
    /// [`crate::geom_cache::cached_auto_index`]) are both memoised on mesh
    /// identity and are already warm on any generated toolpath.
    #[must_use]
    pub fn reach_map_spec(
        &self,
        toolpath_index: usize,
        tolerance_override: Option<f64>,
    ) -> Option<ReachMapRequest> {
        let tc = self.toolpath_configs.get(toolpath_index)?;
        if !tc.operation.op_type().supports_reach_map() {
            return None;
        }
        let tool_cfg = self.find_tool_by_raw_id(tc.tool_id)?;
        let model = self.find_model_by_raw_id(tc.model_id)?;
        let mut mesh = model.mesh.clone()?;
        let model_id = model.id;

        let setup = self.find_setup_for_toolpath_index(toolpath_index);
        let ctx = SetupEvalContext::build_for_setup(self, setup);
        if ctx.needs_transform() {
            mesh = crate::geom_cache::cached_transform(
                &mesh,
                &self.setup_transform_info(ctx.face_up, ctx.z_rotation),
            );
        }
        let index = crate::geom_cache::cached_auto_index(&mesh);

        let cutter = Arc::new(build_cutter(tool_cfg));
        let (tolerance_mm, tolerance_source) =
            self.reach_tolerance_with_override(toolpath_index, tolerance_override);
        let params = ReachMapParams::for_cutter(cutter.as_ref(), tolerance_mm);

        Some(ReachMapRequest {
            mesh,
            index,
            cutter,
            params,
            tool_id: tool_cfg.id.0,
            model_id,
            tolerance_source,
        })
    }

    /// The memoised reach map for one toolpath.
    ///
    /// **Seconds of drop-cutter work on a cold key** — call it from a worker,
    /// never from the UI thread or a render pass. The GUI resolves
    /// [`Self::reach_map_spec`] on the main thread and hands the request to a
    /// background thread, which calls
    /// [`crate::reach_map_cache::cached_reach_map`] directly.
    ///
    /// # Errors
    ///
    /// [`Cancelled`] if `cancel` fires during a build.
    pub fn reach_map_for(
        &self,
        toolpath_index: usize,
        tolerance_override: Option<f64>,
        cancel: &(dyn CancelCheck + Sync),
    ) -> Result<Option<Arc<ReachMap>>, Cancelled> {
        let Some(spec) = self.reach_map_spec(toolpath_index, tolerance_override) else {
            return Ok(None);
        };
        crate::reach_map_cache::cached_reach_map(&spec, cancel).map(Some)
    }
}
