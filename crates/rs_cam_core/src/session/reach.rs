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
use crate::interrupt::{CancelCheck, Cancelled};
use crate::reach_map::{DEFAULT_REACH_TOLERANCE_MM, ReachMap, ReachMapParams, ReachMapRequest};

use super::{ProjectSession, SetupEvalContext};

impl ProjectSession {
    /// The reach tolerance (mm) for one toolpath: its own declared cusp /
    /// scallop height where it has one, else [`DEFAULT_REACH_TOLERANCE_MM`].
    ///
    /// `stock_to_leave` is deliberately **not** folded in. It is an intended
    /// uniform vertical offset, while the reach gap is measured against the
    /// true mesh; adding it would raise the bar by the leave amount and hide
    /// a real miss of exactly that size behind an intended one.
    #[must_use]
    pub fn reach_tolerance_for(&self, toolpath_index: usize) -> f64 {
        self.toolpath_configs
            .get(toolpath_index)
            .and_then(|tc| tc.operation.scallop_height())
            .filter(|h| h.is_finite() && *h > 0.0)
            .unwrap_or(DEFAULT_REACH_TOLERANCE_MM)
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
        let tolerance_mm = tolerance_override
            .filter(|t| t.is_finite() && *t > 0.0)
            .unwrap_or_else(|| self.reach_tolerance_for(toolpath_index));
        let params = ReachMapParams::for_cutter(cutter.as_ref(), tolerance_mm);

        Some(ReachMapRequest {
            mesh,
            index,
            cutter,
            params,
            tool_id: tool_cfg.id.0,
            model_id,
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
