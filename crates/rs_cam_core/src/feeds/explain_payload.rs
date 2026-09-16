//! **The UI data contract.** Holds [`FeedsExplain`]. For the
//! stage-labelled feed record `FeedExplanation`, see
//! [`super::feed_explanation`].
//!
//! Feeds-and-speeds explanation payload — bundles everything the
//! UI needs to render the Feeds & Speeds surfaces:
//!
//! - the recommended values (delegates to [`super::calculate`])
//! - the matched LUT row, if any, with scaling factors
//! - the echoed query, tool geometry and hardness coordinates
//! - the machine envelope (caps for the feed-RPM nomogram)
//!
//! The payload also carried sibling vendor rows until 2026-09-16. They
//! fed the diameter and hardness charts only, and those charts are gone.
//!
//! The UI stays presentation-only; this struct is the data contract
//! between core and the modal. Build with [`explain`].

use super::vendor_lookup::{LookupQuery, MatchedRow, find_best_row_for_geometry};
use super::{FeedsInput, FeedsResult, calculate, vendor_normalize};
use crate::machine::{MachineProfile, PowerModel};

/// Snapshot of the machine envelope used by the nomogram chart.
/// Pure-numeric copy so the UI doesn't need to thread `MachineProfile`.
#[derive(Debug, Clone, PartialEq)]
pub struct MachineEnvelope {
    pub spindle_min_rpm: f64,
    pub spindle_max_rpm: f64,
    pub max_feed_mm_min: f64,
    /// Rated (or constant) power in kW — used as the headroom ceiling.
    pub max_power_kw: f64,
    pub safety_factor: f64,
}

impl MachineEnvelope {
    fn from_machine(machine: &MachineProfile) -> Self {
        let (min_rpm, max_rpm) = machine.rpm_range();
        let max_power_kw = match machine.power {
            PowerModel::VfdConstantTorque { rated_power_kw, .. } => rated_power_kw,
            PowerModel::ConstantPower { power_kw } => power_kw,
        };
        Self {
            spindle_min_rpm: min_rpm,
            spindle_max_rpm: max_rpm,
            // F4: the explain envelope describes cutting-feed bounds.
            max_feed_mm_min: machine.cutting_feed_ceiling_mm_min(),
            max_power_kw,
            safety_factor: machine.safety_factor,
        }
    }
}

/// Bundle of everything the Feeds & Speeds modal needs to render.
///
/// Computed once per recalculation. The UI reads from this and the
/// caller's `OperationConfig` (current values) to draw the comparison
/// card, the feed-RPM nomogram, and the provenance disclosure.
#[derive(Debug, Clone)]
pub struct FeedsExplain {
    /// Recommended values (RPM, feed, plunge, DOC, WOC, chipload, power, MRR, warnings).
    pub recommended: FeedsResult,
    /// The vendor LUT query that produced the match (echoed back so
    /// the UI can render the input chips without re-deriving them).
    pub query: LookupQuery,
    /// The matched LUT row (None when no row passed the must-match
    /// filters and the calc fell through to the empirical formula).
    pub matched_row: Option<MatchedRow>,
    /// Tool diameter (mm). Echoed so charts can mark "your tool" without
    /// reaching back into the input.
    pub tool_diameter_mm: f64,
    /// Tool shank diameter (mm). Caps the engaged-diameter growth for
    /// tapered-ball tools; echoed so the modal can render the
    /// engaged-diameter-at-DOC annotation without rebuilding the input.
    pub shank_diameter_mm: f64,
    /// Tool geometry hint. Echoed so the modal can show the
    /// engaged-diameter-at-DOC annotation for tapered-ball / V-bit tools
    /// via [`ToolGeometryHint::engaged_diameter_at_doc`].
    pub tool_geometry: super::ToolGeometryHint,
    /// Tool flute count. Used by the feed-RPM nomogram
    /// (`feed = chipload × rpm × flutes`).
    pub flute_count: u32,
    /// Hardness coordinates of the query material. `None` when the
    /// material doesn't expose hardness in a form the LUT knows about.
    pub query_hardness_kind: Option<super::vendor_lut::HardnessKind>,
    pub query_hardness_value: Option<f64>,
    /// Machine envelope for the nomogram.
    pub machine: MachineEnvelope,
}

impl FeedsExplain {
    /// True when the matched row's scaling exceeds the ±40 % calibration
    /// threshold — UI should mark the band as approximate.
    pub fn is_extrapolated(&self) -> bool {
        self.matched_row
            .as_ref()
            .is_some_and(|row| row.is_extrapolated)
    }
}

/// Build the explanation payload for the given input. Runs the same
/// pipeline as [`calculate`], then adds the matched vendor row and a
/// snapshot of the machine envelope.
pub fn explain(input: &FeedsInput<'_>) -> FeedsExplain {
    let recommended = calculate(input);
    let machine = MachineEnvelope::from_machine(input.machine);

    // Checkpoint K (a4) — `to_lookup_query` can now REFUSE (a
    // `ProjectCurve` on a bull-nose / V-bit / facing cutter has no vendor
    // family). The modal then shows the formula-fallback state with the
    // unrouted query echoed for context; `recommended.warnings` carries
    // `NoVendorRowsForRoutedOperation`, which is what the modal prints.
    let routed_query = input
        .vendor_lut
        .and(vendor_normalize::to_lookup_query(input));
    let (query, matched_row) = match (input.vendor_lut, routed_query) {
        (Some(lut), Some(query)) => {
            // Use the geometry-aware dispatcher so the V-bit "matched row"
            // honours the cutter's cone angle.
            let matched = find_best_row_for_geometry(lut, &query, &input.tool_geometry);
            (query, matched)
        }
        _ => {
            // No LUT, or the routing refused — synthesise a query record
            // for echo. The modal shows the formula-fallback state in
            // this branch.
            let query = synthetic_query(input);
            (query, None)
        }
    };

    FeedsExplain {
        recommended,
        query: query.clone(),
        matched_row,
        tool_diameter_mm: input.tool_diameter,
        shank_diameter_mm: input.shank_diameter.unwrap_or(input.tool_diameter),
        tool_geometry: input.tool_geometry,
        flute_count: input.flute_count,
        query_hardness_kind: query.hardness_kind,
        query_hardness_value: query.hardness_value,
        machine,
    }
}

/// The query record echoed back when no LUT lookup happened — because
/// there was no LUT, or because the a4 routing refused this operation ×
/// cutter pairing. Falls back to the **unrouted** family so the modal's
/// input chips still describe the operation the user is looking at; it
/// is an echo, never a lookup.
fn synthetic_query(input: &FeedsInput<'_>) -> LookupQuery {
    vendor_normalize::to_lookup_query_unrouted(input)
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
    use crate::feeds::vendor_lut::{
        LutOperationFamily, LutPassRole, MaterialFamily, ToolFamily, VendorLut,
    };
    use crate::feeds::{
        OperationFamily, PassRole, SetupContext, ToolGeometryHint, embedded_vendor_lut,
    };
    use crate::machine::MachineProfile;
    use crate::material::{Material, WoodSpecies};

    fn make_input<'a>(
        material: &'a Material,
        machine: &'a MachineProfile,
        lut: Option<&'a VendorLut>,
    ) -> FeedsInput<'a> {
        FeedsInput {
            tool_diameter: 6.35,
            flute_count: 2,
            flute_length: 22.0,
            shank_diameter: Some(6.35),
            tool_geometry: ToolGeometryHint::Flat,
            material,
            machine,
            operation: OperationFamily::Pocket,
            operation_kind: None,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: lut,
            setup: SetupContext::default(),
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
        }
    }

    #[test]
    fn explain_returns_recommended_and_envelope() {
        let mat = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let mach = MachineProfile::shapeoko_vfd();
        let lut = embedded_vendor_lut();
        let input = make_input(&mat, &mach, Some(lut));

        let exp = explain(&input);

        assert!(exp.recommended.rpm > 0.0);
        assert!(exp.recommended.feed_rate_mm_min > 0.0);
        assert_eq!(exp.tool_diameter_mm, 6.35);
        assert_eq!(exp.flute_count, 2);
        assert_eq!(exp.machine.max_feed_mm_min, 5000.0);
        assert!(exp.machine.spindle_max_rpm > exp.machine.spindle_min_rpm);
        assert_eq!(exp.query.tool_family, ToolFamily::FlatEnd);
        assert_eq!(exp.query.material_family, MaterialFamily::Hardwood);
    }

    #[test]
    fn explain_carries_matched_row_when_lut_present() {
        let mat = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let mach = MachineProfile::shapeoko_vfd();
        let lut = embedded_vendor_lut();
        let input = make_input(&mat, &mach, Some(lut));

        let exp = explain(&input);
        let matched = exp.matched_row.as_ref().expect("LUT should match");
        assert!(matched.chip_load_mm > 0.0);
        assert!(matched.row_diameter_mm > 0.0);
    }

    #[test]
    fn explain_no_lut_leaves_matched_row_none() {
        let mat = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let mach = MachineProfile::shapeoko_vfd();
        let input = make_input(&mat, &mach, None);

        let exp = explain(&input);
        assert!(exp.matched_row.is_none());
    }

    #[test]
    fn machine_envelope_reflects_machine_caps() {
        let mach = MachineProfile::shapeoko_makita();
        let env = MachineEnvelope::from_machine(&mach);
        let (min_rpm, max_rpm) = mach.rpm_range();
        assert_eq!(env.spindle_min_rpm, min_rpm);
        assert_eq!(env.spindle_max_rpm, max_rpm);
        assert_eq!(env.max_feed_mm_min, mach.max_feed_mm_min);
        assert_eq!(env.safety_factor, mach.safety_factor);
    }

    #[test]
    fn is_extrapolated_propagates_from_matched_row() {
        let mat = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let mach = MachineProfile::shapeoko_vfd();
        let lut = embedded_vendor_lut();
        let mut input = make_input(&mat, &mach, Some(lut));
        // Push diameter far from any calibrated row to force extrapolation.
        input.tool_diameter = 12.7;
        let exp = explain(&input);
        if let Some(row) = &exp.matched_row {
            assert_eq!(exp.is_extrapolated(), row.is_extrapolated);
        }
    }

    #[test]
    fn explain_query_matches_lookup_query() {
        let mat = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let mach = MachineProfile::shapeoko_vfd();
        let lut = embedded_vendor_lut();
        let input = make_input(&mat, &mach, Some(lut));

        let exp = explain(&input);
        assert_eq!(exp.query.operation_family, LutOperationFamily::Pocket);
        assert_eq!(exp.query.pass_role, LutPassRole::Roughing);
        assert_eq!(exp.query.flute_count, 2);
    }
}
