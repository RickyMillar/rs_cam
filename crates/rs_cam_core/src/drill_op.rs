//! First-class drilling operation data model — DEXEL roadmap §6.E / Step 3.
//!
//! A [`DrillOp`] represents end-cutting plunge moves as a list of holes
//! with a cycle, tool profile, and feed/spindle parameters. It exists
//! alongside the linearized [`AnnotatedToolpath`] (dual-representation
//! invariant) so the simulator can apply analytical stock removal and
//! emit drill-specific metrics while G-code export and wire rendering
//! continue to consume the linearized form.
//!
//! The two representations are produced atomically in
//! `compute/execute.rs` from the same `OperationConfig`, and invalidated
//! together by any `set_toolpath_param` mutation that drops the cached
//! `ToolpathComputeResult`.

use crate::material::Material;
use crate::toolpath_spans::AnnotatedToolpath;
use std::sync::Arc;

/// One hole in a drilling operation.
///
/// `top_z` is the entry surface and `bottom_z` is the deepest point reached
/// by the tool tip — "deepest" meaning *furthest along the tool's advance*,
/// not "numerically smaller".
///
/// As constructed by the generators these are setup-local coordinates, where
/// the tool always advances along −Z and so `top_z >= bottom_z`. That
/// ordering is **not** an invariant of the type: `group_drill_op_to_global`
/// maps holes into the stock-relative global frame, and a `FaceUp::Bottom`
/// setup's `z → H − z` comes back with `top_z <= bottom_z`. Nothing is wrong
/// with such a hole; it is entered from the low side. What consumes it has to
/// know which, and the answer is the group's
/// [`crate::dexel_stock::StockCutDirection`] — never the sign of
/// `top_z - bottom_z`, which is degenerate at zero depth, and never a
/// `min`/`max` of the pair, which silently re-points the tool. See
/// `TriDexelStock::apply_drill_op` (G-DRILLFLIP).
///
/// The XY pair likewise names the axis only while the drill axis *is* Z; a
/// lateral setup's hole cannot be expressed here at all (G-DRILLLATERAL).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrillHole {
    pub xy: [f64; 2],
    pub top_z: f64,
    pub bottom_z: f64,
}

/// Where hole positions originate, controlling regenerate behavior and
/// project-IO round-trip.
///
/// - `Snapshot`: XY positions captured into the operation config at
///   submit time (e.g. `AlignmentPinDrillConfig.holes`). Survives
///   project save/load directly as part of the `OperationConfig` TOML.
/// - `ModelDerived`: positions are computed from polygon centroids at
///   generate-time and discarded after each regenerate. Project IO does
///   not persist them; they re-resolve from the model on load.
///
/// See §6.E AlignmentPin vs Drill hole-source asymmetry.
#[derive(Debug, Clone, PartialEq)]
pub enum HoleSource {
    Snapshot(Vec<[f64; 2]>),
    ModelDerived,
}

/// Tool-tip geometry for analytical stock removal.
///
/// `Flat` sets `ray_top = bottom_z` across the cylinder footprint.
/// `StandardTwist` adds a conical bottom of half-angle ~31° (118° included
/// angle is the standard twist-drill point). `Spot` is parameterized for
/// spot drills / center drills with a steeper included angle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToolProfile {
    /// Flat-bottom — e.g. end-mill used for drilling.
    Flat,
    /// Standard twist drill (118° included → ~31° from axis).
    StandardTwist,
    /// Spot drill / center drill with explicit included angle.
    Spot { included_angle_deg: f64 },
}

impl ToolProfile {
    /// Cone half-angle from the tool axis (radians). `Flat` returns 0.
    pub fn cone_half_angle_rad(self) -> f64 {
        match self {
            ToolProfile::Flat => 0.0,
            // 118° included → 59° from axis → 90° - 59° = 31° point half-angle.
            // The cone half-angle measured from the *axis* is 59°.
            ToolProfile::StandardTwist => 59.0_f64.to_radians(),
            ToolProfile::Spot { included_angle_deg } => (included_angle_deg * 0.5).to_radians(),
        }
    }

    /// Tip-protrusion height for a tool of `radius_mm`: how far the tip
    /// extends below the cone-radius transition (i.e. how much taller
    /// the cylinder section is than the cone section).
    ///
    /// For `Flat`, returns 0. For coned profiles, the cone reaches its
    /// widest point at `radius_mm` from the axis at a vertical distance
    /// of `radius_mm / tan(half_angle)` below the cylindrical shoulder.
    pub fn tip_protrusion_mm(self, radius_mm: f64) -> f64 {
        match self {
            ToolProfile::Flat => 0.0,
            _ => {
                let half = self.cone_half_angle_rad();
                if half <= 0.0 {
                    0.0
                } else {
                    radius_mm / half.tan()
                }
            }
        }
    }
}

/// A drilling operation: a list of holes with cycle, tool profile, and
/// feed/spindle parameters. Carried as the `DrillOp` variant of
/// [`OpData`] alongside the linearized [`AnnotatedToolpath`].
#[derive(Debug, Clone)]
pub struct DrillOp {
    pub holes: Vec<DrillHole>,
    pub hole_source: HoleSource,
    pub tool_profile: ToolProfile,
    /// Drill tool diameter (mm). Carried directly so the analytical
    /// removal kernel and gate calculations don't need to re-resolve the
    /// assigned tool from session state.
    pub tool_diameter_mm: f64,
    pub cycle: crate::drill::DrillCycle,
    pub feed_rate_mm_min: f64,
    pub spindle_rpm: u32,
    pub flute_count: u32,
    /// Workpiece material. Threaded from stock config at construction
    /// time; consumed by drill-specific gates (chip welding, peck
    /// adequacy) in PR2.
    pub material: Material,
    /// The R-plane (setup-local Z): the height the tool rapids down to
    /// and starts FEEDING from, once per peck.
    ///
    /// R-2 (2026-08-04). This is `effective_safe_z(cfg.retract_z,
    /// stock_top)` = `max(raw, stock_top + SAFE_Z_CLEARANCE_MM)`, and
    /// since `SAFE_Z_CLEARANCE_MM` is 5.0 while the `DrillConfig`
    /// default `retract_z` is 2.0, it is `stock_top + 5.0` on any
    /// default project. The emitter has always rooted its peck grid
    /// here — that is the Fanuc G83 convention and it is correct — but
    /// `DrillOp` did not carry it, so every metric derived from this
    /// struct modelled a cycle starting at the material surface and
    /// silently dropped the approach feed and every re-entry.
    pub retract_z_mm: f64,
}

/// Dual-representation payload of a [`ToolpathComputeResult`].
///
/// Invariant: the `Arc<AnnotatedToolpath>` in either variant was produced
/// atomically with the rest of this value from a single
/// `OperationConfig`. Any `set_toolpath_param` mutation that invalidates
/// the cached `ToolpathComputeResult` invalidates both representations.
#[derive(Debug, Clone)]
pub enum OpData {
    Toolpath(Arc<AnnotatedToolpath>),
    DrillOp(Arc<DrillOp>, Arc<AnnotatedToolpath>),
}

impl OpData {
    /// Linearized toolpath view. Both variants carry one — see the
    /// dual-representation invariant.
    pub fn annotated(&self) -> &Arc<AnnotatedToolpath> {
        match self {
            OpData::Toolpath(a) => a,
            OpData::DrillOp(_, a) => a,
        }
    }

    /// Drill-op view when present. `None` for non-drill operations.
    pub fn drill_op(&self) -> Option<&Arc<DrillOp>> {
        match self {
            OpData::Toolpath(_) => None,
            OpData::DrillOp(d, _) => Some(d),
        }
    }

    pub fn is_drill_op(&self) -> bool {
        matches!(self, OpData::DrillOp(_, _))
    }
}
