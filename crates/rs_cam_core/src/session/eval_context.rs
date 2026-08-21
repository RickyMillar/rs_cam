//! Unified per-setup evaluation context.
//!
//! Before F-030 the same stock-frame + transform decision shape was
//! independently re-derived at 5 separate sites (load, core compute,
//! core simulate, viz worker, viz controller gen + sim). Each finding
//! in the F-024/F-026/F-027/F-028 family surfaced as "fix at one site,
//! production took a different site that was still broken." Every site
//! checked `face_up == Top && z_rotation == Deg0` for identity-setup
//! detection, built world-vs-local `stock_bbox` differently, and
//! constructed `HeightContext` from a frame that did or did not match
//! the toolpath emission frame.
//!
//! [`SetupEvalContext`] is the canonical carrier. Constructed once by
//! [`SetupEvalContext::build_for_setup`]; consumed by every site that
//! needed any of these values. Identity-or-not is encoded once in
//! `local_to_global` (`None` = identity).

use crate::compute::config::effective_safe_z;
use crate::compute::transform::{FaceUp, SetupTransformInfo, ZRotation};
use crate::geo::{BoundingBox3, P3};

use super::{ProjectSession, SetupData};

/// Single source of truth for a (session, setup) tuple's stock-frame
/// and transform decisions.
///
/// **Invariants** (by construction):
/// - `world_stock_bbox` honors `StockConfig::origin_{x,y,z}` (delegates
///   to [`ProjectSession::stock_bbox`]).
/// - `local_stock_bbox` is zero-rooted (`min == (0,0,0)`); `max` is the
///   post-face/rotation effective stock dimensions.
/// - `local_to_global == None` ⇔ setup is identity (`face_up == Top`
///   AND `z_rotation == Deg0`). For identity setups the toolpath emits
///   in world frame; for non-identity setups the toolpath emits in
///   setup-local frame and `local_to_global` carries the back-transform.
/// - `heights_stock_bbox == world_stock_bbox` for identity setups
///   (F-028) and `== local_stock_bbox` for non-identity setups. This is
///   the frame `HeightContext::stock_top_z` and the dexel grid must
///   match for the engagement metric to read correctly (F-024).
/// - `safe_z` is `effective_safe_z(post.safe_z, heights_stock_bbox.max.z)`
///   — the **emission** frame, the same one `stock_top_z` above uses.
///   A retract plane and the depth ladder it retracts from have to be
///   measured against the same stock top.
///
///   This read used to be `local_stock_bbox.max.z`, justified as
///   "conservatively-higher … always safe". It is neither, because the
///   local top is the stock *thickness* while the world top is
///   `origin_z + thickness`: for `origin_z > SAFE_Z_CLEARANCE_MM` the
///   floor lands **inside** the stock (12 mm stock at `origin_z = 20`
///   floored rapids at Z17 under a world top of 32), and for
///   `origin_z < 0` — the documented 2D convention — it sat needlessly
///   high, which on wanaka200 put five whole depth levels in air ahead
///   of the first cutting one. Non-identity setups are unaffected:
///   `heights_stock_bbox == local_stock_bbox` for them. Sentried by
///   `safe_z_emission_frame_g_safez_local.rs` (G-SAFEZ-LOCAL).
#[derive(Clone)]
pub struct SetupEvalContext {
    /// World-frame stock bbox. Honors origin + `auto_from_model`
    /// re-derivation at load time (F-026).
    pub world_stock_bbox: BoundingBox3,

    /// Setup-local zero-rooted bbox after face-up + Z-rotation.
    /// Always `min == (0,0,0)`.
    pub local_stock_bbox: BoundingBox3,

    /// Setup transform info. `None` ⇔ identity setup (`face_up == Top`
    /// AND `z_rotation == Deg0`); `Some(info)` carries the
    /// local↔global matrix for non-identity setups.
    pub local_to_global: Option<SetupTransformInfo>,

    /// Stock bbox in the frame the toolpath emits in.
    /// Identity setups → `world_stock_bbox`; non-identity → `local_stock_bbox`.
    /// Used to derive `HeightContext::{stock_top_z, stock_bottom_z}` so
    /// the depth-stepping anchor and the simulator's per-setup dexel
    /// grid live in the same frame (F-024 + F-028).
    pub heights_stock_bbox: BoundingBox3,

    /// Effective safe_z. Floored at `heights_stock_bbox.max.z +
    /// SAFE_Z_CLEARANCE_MM` — the frame the toolpath emits in
    /// (G-SAFEZ-LOCAL).
    pub safe_z: f64,

    /// Setup orientation. Kept here so consumers don't have to plumb
    /// the raw enum through alongside the context (e.g. ProjectCurve's
    /// `setup_z_flipped` flag, AS-style cut-direction selection).
    pub face_up: FaceUp,
    pub z_rotation: ZRotation,
}

impl SetupEvalContext {
    /// Build the context for an explicit face/rotation pair. Useful
    /// when the caller already resolved the setup (or is using
    /// non-default identity-equivalent values).
    pub fn build(
        session: &ProjectSession,
        face_up: FaceUp,
        z_rotation: ZRotation,
    ) -> SetupEvalContext {
        let world_stock_bbox = session.stock_bbox();
        let xform = session.setup_transform_info(face_up, z_rotation);
        let local_stock_bbox = xform.effective_stock_bbox();
        let local_to_global = if xform.needs_transform() {
            Some(xform)
        } else {
            None
        };
        let heights_stock_bbox = if local_to_global.is_some() {
            local_stock_bbox
        } else {
            world_stock_bbox
        };
        let safe_z = effective_safe_z(session.post_config().safe_z, heights_stock_bbox.max.z);
        SetupEvalContext {
            world_stock_bbox,
            local_stock_bbox,
            local_to_global,
            heights_stock_bbox,
            safe_z,
            face_up,
            z_rotation,
        }
    }

    /// Build the context for a setup (or identity defaults if `None`).
    /// Preferred entry point: most production sites have a
    /// `Option<&SetupData>` from `find_setup_for_toolpath_index` or
    /// `list_setups()[i]`.
    pub fn build_for_setup(
        session: &ProjectSession,
        setup: Option<&SetupData>,
    ) -> SetupEvalContext {
        let face_up = setup.map(|s| s.face_up).unwrap_or_default();
        let z_rotation = setup.map(|s| s.z_rotation).unwrap_or_default();
        Self::build(session, face_up, z_rotation)
    }

    /// `true` when the setup applies a non-identity face_up / z_rotation
    /// transform. Equivalent to `self.local_to_global.is_some()`.
    pub fn needs_transform(&self) -> bool {
        self.local_to_global.is_some()
    }

    /// `true` when the setup transform Z-inverts the toolpath
    /// (`face_up == Bottom`). Drives `ProjectCurve::setup_z_flipped`.
    pub fn is_z_flipped(&self) -> bool {
        self.local_to_global
            .as_ref()
            .is_some_and(|info| info.is_z_flipped())
    }

    /// Translation applied to this setup's emitted toolpath so every
    /// setup in a project expresses XY in ONE frame: **stock-relative**,
    /// with program X0 Y0 at the stock's min corner.
    ///
    /// G-EXPORT-DATUM (2026-08-19). Identity setups emit their toolpath
    /// in the WORLD frame; non-identity setups emit in the zero-rooted
    /// setup-local frame (which is already stock-relative — see
    /// [`crate::compute::transform::SetupTransformInfo::world_to_local`],
    /// whose first step is `-stock_origin`). When
    /// `StockConfig::origin_{x,y} != 0` those two frames differ by
    /// exactly the origin, so a two-sided job exported one file per
    /// setup handed the operator two different XY datums under a single
    /// plain `G54` — and the only header warning was about **Z**. Keeping
    /// one XY zero across the flip then machines the second side off by
    /// the origin (240×250 stock at origin (-20,-25): 20 mm / 25 mm).
    ///
    /// Stock-relative is the frame to converge on because it is what the
    /// non-identity setups already emit, it is the frame
    /// `StockConfig::alignment_pins` are dimensioned in (the feature that
    /// physically registers the flip), and it names a datum the operator
    /// can actually find: the stock's own corner.
    ///
    /// **Z is deliberately NOT shifted.** See the module note on
    /// `gcode::export_datum_shift_for_toolpath`.
    pub fn export_datum_shift(&self) -> P3 {
        if self.local_to_global.is_some() {
            // Non-identity: the toolpath is already stock-relative.
            P3::new(0.0, 0.0, 0.0)
        } else {
            // Identity: world → stock-relative is a pure XY translation.
            P3::new(
                -self.world_stock_bbox.min.x,
                -self.world_stock_bbox.min.y,
                0.0,
            )
        }
    }

    /// The simulator's per-setup local bbox slot. F-024 requires `None`
    /// for identity setups so `run_simulation` falls back to
    /// `request.stock_bbox` (world frame) for the dexel grid; non-identity
    /// setups forward the local zero-rooted bbox alongside the
    /// `local_to_global` transform.
    pub fn sim_local_stock_bbox(&self) -> Option<BoundingBox3> {
        if self.local_to_global.is_some() {
            Some(self.local_stock_bbox)
        } else {
            None
        }
    }
}
