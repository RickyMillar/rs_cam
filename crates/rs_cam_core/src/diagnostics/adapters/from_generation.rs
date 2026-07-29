//! Adapter: generation-time geometry findings → [`Diagnostic`] list.
//!
//! These are facts an operation learned about the geometry while
//! generating, which are not properties of the emitted toolpath and not
//! derivable from a simulation trace — see
//! [`crate::compute::execute::GenerationFindings`].
//!
//! Why this adapter exists at all: `scallop` has always measured the
//! interior area its ring cascade failed to reach, and has always
//! reported it exclusively through `tracing::warn!`. A warning nobody
//! sees is not a warning — the campaign shipped a 28 mm block of
//! unmachined material for weeks because no run installed a subscriber,
//! and the GUI's diagnostics list had no channel for it at all
//! (`planning/unified_v3_design.md` §13/§14c/§14h).

use crate::compute::config::{
    STANDING_MATERIAL_DOMAIN, STANDING_MATERIAL_RESOLUTION, STANDING_MATERIAL_STAGE, ToolpathStats,
};
use crate::diagnostics::{
    Category, Confidence, Diagnostic, DiagnosticId, DiagnosticState, Scope, Severity, Source, ids,
};
use crate::ids::ToolpathId;

/// Area (mm²) below which standing material is not worth a diagnostic.
///
/// A ring cascade can finish with a sliver of un-collapsed polygon that
/// the next pass covers anyway. One square millimetre is far below the
/// scale of the defect this exists to catch (837 mm² and 4 461 mm²
/// measured on wanaka) while keeping the list quiet on rounding.
const STANDING_MATERIAL_FLOOR_MM2: f64 = 1.0;

/// Emit diagnostics for a toolpath's generation-time findings.
pub fn diagnostics_from_generation(
    toolpath_id: ToolpathId,
    stats: &ToolpathStats,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    // `None` = not measured (no ring cascade ran). An absent measurement is
    // not a defect claim, and it is NOT the same as a measured zero — see
    // `ToolpathStats::standing_material_mm2` (A/M9).
    let Some(area) = stats.standing_material_mm2 else {
        return out;
    };
    // NaN never reports either, for the same reason.
    if area.partial_cmp(&STANDING_MATERIAL_FLOOR_MM2) != Some(std::cmp::Ordering::Greater) {
        return out;
    }

    out.push(Diagnostic {
        id: DiagnosticId::from(ids::GEOM_STANDING_MATERIAL),
        scope: Scope::Toolpath { id: toolpath_id },
        category: Category::Geometry,
        severity: Severity::Caution,
        // Measured during generation from the cascade's own residual
        // polygons — not a heuristic, and not superseded by a sim, which
        // cannot see material the toolpath never attempted to cut.
        confidence: Confidence::Verified,
        state: DiagnosticState::Current,
        source: Source::StaticValidation,
        message: format!(
            "{area:.0} mm² of material left UNCUT inside the machining \
             region: the scallop ring cascade hit its ring cap before the \
             offsets collapsed, so the INTERIOR was never reached. The part \
             will have a raised island. Reduce the region, coarsen the \
             scallop height, or split the operation. \
             [{domain}; {stage}; {resolution}. Report-only — no gate.]",
            domain = STANDING_MATERIAL_DOMAIN,
            stage = STANDING_MATERIAL_STAGE,
            resolution = STANDING_MATERIAL_RESOLUTION,
        ),
        evidence: None,
        fix: None,
        supersedes: vec![],
        suppressed_diagnostics: vec![],
    });
    out
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

    fn stats(standing_material_mm2: f64) -> ToolpathStats {
        ToolpathStats {
            standing_material_mm2: Some(standing_material_mm2),
            ..ToolpathStats::default()
        }
    }

    #[test]
    fn silent_when_nothing_is_left_standing() {
        assert!(diagnostics_from_generation(ToolpathId(1), &stats(0.0)).is_empty());
    }

    /// A/M9: "no cascade ran" is a different state from "a cascade measured
    /// zero", and neither is a defect — but only the former may be reported
    /// as unmeasured downstream, so the adapter must accept it.
    #[test]
    fn silent_when_nothing_was_measured() {
        assert!(
            diagnostics_from_generation(ToolpathId(1), &ToolpathStats::default()).is_empty(),
            "default stats carry `None` — not measured"
        );
    }

    /// A sliver of un-collapsed polygon is not a raised island.
    #[test]
    fn silent_below_the_floor() {
        assert!(diagnostics_from_generation(ToolpathId(1), &stats(0.5)).is_empty());
    }

    /// NaN is "not measured", not "defect" — an unmeasured cascade must not
    /// manufacture a scrap warning.
    #[test]
    fn silent_on_nan() {
        assert!(diagnostics_from_generation(ToolpathId(1), &stats(f64::NAN)).is_empty());
    }

    /// The wanaka case that motivated the whole channel: 837 mm² of
    /// standing material that only a `tracing::warn!` ever mentioned.
    #[test]
    fn reports_a_real_uncut_island() {
        let out = diagnostics_from_generation(ToolpathId(7), &stats(837.0));
        assert_eq!(out.len(), 1);
        let d = &out[0];
        assert_eq!(d.id, DiagnosticId::from(ids::GEOM_STANDING_MATERIAL));
        assert_eq!(d.scope, Scope::Toolpath { id: ToolpathId(7) });
        assert_eq!(d.category, Category::Geometry);
        assert_eq!(d.severity, Severity::Caution);
        assert!(
            d.message.contains("837"),
            "the area is the actionable number: {}",
            d.message
        );
        // M1: an area with no declared domain is exactly the unlabelled
        // `f64` the audit found being compared across domains.
        for needle in [
            STANDING_MATERIAL_DOMAIN,
            STANDING_MATERIAL_STAGE,
            STANDING_MATERIAL_RESOLUTION,
        ] {
            assert!(
                d.message.contains(needle),
                "message must declare {needle:?}: {}",
                d.message
            );
        }
    }
}
