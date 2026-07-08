//! v3.4 (2026-06-04): Geometry classification for the combined-Suggest
//! strategy passes.
//!
//! The strategy-aware orchestrator picks `entry_style` (and eventually
//! `clearing_strategy` and `stock_to_leave`) based on the geometry the
//! operation runs against. This module produces a coarse classification
//! the strategy passes consume; today's signal comes from the operation
//! type and model bounding box only (a "cheap" classifier that doesn't
//! reach into mesh data).
//!
//! Slope-histogram analysis on STL meshes and face-type histograms on
//! STEP B-reps are tracked as follow-up work — when those land, the
//! classifier will return a richer signal for the
//! `ShallowTerrain` / `SteepTerrain` / `MixedTerrain` split. Today the
//! 3D op types collapse to `MixedTerrain` and the strategy passes use
//! the bbox-feasibility helpers below to decide between Ramp and Helix.

use crate::compute::catalog::OperationType;
use crate::geo::BoundingBox3;

/// Coarse classification of the geometry an operation is acting on.
///
/// Consumed by the v3.3+ strategy passes (currently
/// [`crate::feeds::suggest::pick_adaptive3d_entry_style`]) to decide
/// between safe-default fallback (Ramp) and engagement-quality
/// promotion (Helix) based on whether the model has the headroom for
/// the more aggressive choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryClass {
    /// Mostly flat surfaces (max slope < ~30°). Best for parallel
    /// raster ops (DropCutter, Scallop, HorizontalFinish); helix entry
    /// is trivially available.
    ///
    /// Today (op-type-only classifier) this is *not* returned — slope
    /// analysis is needed to distinguish it from `MixedTerrain`.
    ShallowTerrain,
    /// Steep walls or vertical features dominate. Waterline-friendly;
    /// helix entry needs an interior pocket of size ≥ helix_radius × 2.
    ///
    /// Today (op-type-only classifier) this is *not* returned.
    SteepTerrain,
    /// Mix of flat and steep surfaces — the common 3D roughing case.
    /// Adaptive clearing recommended; helix entry feasible when the
    /// bbox has headroom (see [`helix_feasible_in_bbox`]).
    MixedTerrain,
    /// 2D / pocket-only geometry (Pocket, Profile, Adaptive 2D, Trace,
    /// Face). Strategy auto-pick is moot — the entry-style /
    /// clearing-strategy knobs don't apply.
    PocketLike,
    /// Feature-driven input (V-carve from polyline, drill holes from
    /// hole positions, project_curve from explicit curve geometry).
    /// Strategy passes skip these entirely.
    FeatureDriven,
    /// Can't classify — degenerate bbox, unknown op type, etc. The
    /// strategy passes treat this as "no signal" and fall back to safe
    /// defaults (Ramp instead of Helix).
    Unknown,
}

/// Classify the geometry an operation runs against.
///
/// Op-type-first dispatch: features like V-carve / drill / project-curve
/// route to `FeatureDriven` without inspecting the bbox; 2D ops route to
/// `PocketLike`; 3D ops fall through to `MixedTerrain` when the bbox
/// is well-formed and `Unknown` otherwise.
///
/// A future iteration will accept an additional `ModelGeometrySummary`
/// argument (carrying STL slope histogram, STEP face-type histogram)
/// that lets the 3D branch distinguish `ShallowTerrain` /
/// `SteepTerrain` from `MixedTerrain`.
pub fn classify(op_type: OperationType, model_bbox: Option<&BoundingBox3>) -> GeometryClass {
    use OperationType::*;
    match op_type {
        // Feature-driven: input geometry IS the path, no terrain
        // classification applies.
        VCarve | ProjectCurve | Drill | AlignmentPinDrill | Trace | Inlay => {
            GeometryClass::FeatureDriven
        }
        // 2D / pocket-like: planar engagement, strategy auto-pick moot.
        Pocket | Rest | Profile | Chamfer | Face | Pencil | Adaptive => GeometryClass::PocketLike,
        // 3D ops fall through to terrain-aware classification.
        Adaptive3d | DropCutter | Scallop | UnifiedFinish | Waterline | HorizontalFinish
        | SteepShallow | SpiralFinish | RadialFinish | Zigzag | RampFinish => {
            classify_3d_terrain(model_bbox)
        }
    }
}

fn classify_3d_terrain(model_bbox: Option<&BoundingBox3>) -> GeometryClass {
    let Some(bbox) = model_bbox else {
        return GeometryClass::Unknown;
    };
    let dx = (bbox.max.x - bbox.min.x).max(0.0);
    let dy = (bbox.max.y - bbox.min.y).max(0.0);
    let dz = (bbox.max.z - bbox.min.z).max(0.0);
    if !(dx.is_finite() && dy.is_finite() && dz.is_finite()) {
        return GeometryClass::Unknown;
    }
    if dx <= 0.0 || dy <= 0.0 {
        return GeometryClass::Unknown;
    }
    // Today's coarse classifier collapses 3D geometry to MixedTerrain
    // — without slope-histogram analysis we can't reliably distinguish
    // shallow-vs-steep. Strategy passes that care about the
    // distinction (Helix promotion: prefers ShallowTerrain) will use
    // bbox-feasibility helpers in addition to this signal.
    //
    // `ShallowTerrain` / `SteepTerrain` are reserved variants: this fn
    // only receives a bbox (no slope-histogram input exists yet), so
    // there is nothing trivial to threshold on. Wiring them requires the
    // `ModelGeometrySummary` slope/face-type signal described above, not
    // a local change here. See tracker S.14 (planning/finishing_stack_review_2026-07.md).
    GeometryClass::MixedTerrain
}

/// Returns `true` when a helix entry of the given `helix_radius_factor`
/// has room to engage inside the model bounding box without immediately
/// overshooting an edge.
///
/// The required interior diameter for a clean helix is roughly
/// `helix_radius × 2 + 2 × safety_margin`. Adopting the canonical
/// safety margin `tool_diameter` (one full diameter on each side) gives
/// the simple criterion below. Defensively conservative; future
/// iterations may relax it when we have model topology information
/// (interior pocket geometry, not just bbox).
///
/// Returns `false` when:
/// - `model_bbox` is `None` (no signal),
/// - either bbox dimension is non-finite or non-positive,
/// - the shorter bbox dimension is below the helix headroom threshold.
pub fn helix_feasible_in_bbox(
    model_bbox: Option<&BoundingBox3>,
    tool_diameter_mm: f64,
    helix_radius_factor: f64,
) -> bool {
    let Some(bbox) = model_bbox else {
        return false;
    };
    if !tool_diameter_mm.is_finite() || tool_diameter_mm <= 0.0 {
        return false;
    }
    if !helix_radius_factor.is_finite() || helix_radius_factor <= 0.0 {
        return false;
    }
    let dx = (bbox.max.x - bbox.min.x).max(0.0);
    let dy = (bbox.max.y - bbox.min.y).max(0.0);
    if !(dx.is_finite() && dy.is_finite()) {
        return false;
    }
    let short = dx.min(dy);
    // Required short-axis room: helix_radius × 2 (the spiral) + tool
    // diameter × 2 (one diameter margin per side).
    let required = helix_radius_factor * tool_diameter_mm * 2.0 + tool_diameter_mm * 2.0;
    short >= required
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use crate::geo::P3;

    fn bbox(dx: f64, dy: f64, dz: f64) -> BoundingBox3 {
        BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(dx, dy, dz),
        }
    }

    #[test]
    fn feature_driven_routes_correctly() {
        for op in [
            OperationType::VCarve,
            OperationType::ProjectCurve,
            OperationType::Drill,
            OperationType::AlignmentPinDrill,
            OperationType::Trace,
            OperationType::Inlay,
        ] {
            assert_eq!(
                classify(op, Some(&bbox(100.0, 100.0, 20.0))),
                GeometryClass::FeatureDriven,
                "{op:?} must route to FeatureDriven"
            );
        }
    }

    #[test]
    fn pocket_like_routes_correctly() {
        for op in [
            OperationType::Pocket,
            OperationType::Rest,
            OperationType::Profile,
            OperationType::Chamfer,
            OperationType::Face,
            OperationType::Pencil,
            OperationType::Adaptive,
        ] {
            assert_eq!(
                classify(op, Some(&bbox(100.0, 100.0, 20.0))),
                GeometryClass::PocketLike,
                "{op:?} must route to PocketLike"
            );
        }
    }

    #[test]
    fn three_d_ops_default_to_mixed_terrain() {
        for op in [
            OperationType::Adaptive3d,
            OperationType::DropCutter,
            OperationType::Scallop,
            OperationType::Waterline,
            OperationType::HorizontalFinish,
        ] {
            assert_eq!(
                classify(op, Some(&bbox(100.0, 100.0, 20.0))),
                GeometryClass::MixedTerrain,
                "{op:?} must default to MixedTerrain without slope info"
            );
        }
    }

    #[test]
    fn three_d_op_with_no_bbox_returns_unknown() {
        assert_eq!(
            classify(OperationType::Adaptive3d, None),
            GeometryClass::Unknown
        );
    }

    #[test]
    fn three_d_op_with_degenerate_bbox_returns_unknown() {
        // Zero-width X dimension.
        assert_eq!(
            classify(OperationType::Adaptive3d, Some(&bbox(0.0, 100.0, 20.0))),
            GeometryClass::Unknown
        );
    }

    /// Wanaka motivating case: 6 mm tool, helix_radius_factor 0.3
    /// (Adaptive3dConfig default), bbox 140 × 150 mm. Required headroom
    /// = 0.3 × 6 × 2 + 6 × 2 = 15.6 mm. 140 mm short axis easily clears.
    #[test]
    fn helix_feasible_on_wanaka_back_rough() {
        assert!(helix_feasible_in_bbox(
            Some(&bbox(140.0, 150.0, 50.0)),
            6.0,
            0.3,
        ));
    }

    /// Tight bbox where helix won't engage cleanly: 10 × 10 mm with a
    /// 6 mm tool. Required ~15.6 mm; short axis 10 mm < required.
    #[test]
    fn helix_infeasible_on_tight_bbox() {
        assert!(!helix_feasible_in_bbox(
            Some(&bbox(10.0, 10.0, 20.0)),
            6.0,
            0.3,
        ));
    }

    #[test]
    fn helix_infeasible_when_bbox_missing() {
        assert!(!helix_feasible_in_bbox(None, 6.0, 0.3));
    }

    #[test]
    fn helix_infeasible_with_invalid_inputs() {
        // Zero tool diameter.
        assert!(!helix_feasible_in_bbox(
            Some(&bbox(100.0, 100.0, 20.0)),
            0.0,
            0.3,
        ));
        // Zero helix radius factor.
        assert!(!helix_feasible_in_bbox(
            Some(&bbox(100.0, 100.0, 20.0)),
            6.0,
            0.0,
        ));
        // Non-finite.
        assert!(!helix_feasible_in_bbox(
            Some(&bbox(100.0, 100.0, 20.0)),
            f64::NAN,
            0.3,
        ));
    }
}
