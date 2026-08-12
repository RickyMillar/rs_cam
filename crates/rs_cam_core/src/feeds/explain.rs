//! Feeds-and-speeds explanation payload — bundles everything the
//! UI needs to render the redesigned Feeds & Speeds modal:
//!
//! - the recommended values (delegates to [`super::calculate`])
//! - the matched LUT row, if any, with scaling factors
//! - sibling vendor rows for the diameter and hardness charts
//! - the machine envelope (caps for the feed-RPM nomogram)
//!
//! The UI stays presentation-only; this struct is the data contract
//! between core and the modal. Build with [`explain`].

use super::vendor_lookup::{
    LookupQuery, MatchedRow, enumerate_matching_rows, find_best_row_for_geometry,
};
use super::vendor_lut::VendorLut;
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
    pub fn from_machine(machine: &MachineProfile) -> Self {
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
/// card, the three charts, and the provenance disclosure.
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
    /// Sibling rows in the same `(tool_family, op, pass_role)` cohort —
    /// every diameter / material combo we have data for. Feeds both
    /// Chart A (vs diameter) and Chart B (vs hardness).
    pub sibling_rows: Vec<MatchedRow>,
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

    /// Subset of sibling rows that share the query's material, sorted
    /// by ascending diameter. Used by Chart A (chipload vs diameter).
    pub fn rows_by_diameter(&self) -> Vec<&MatchedRow> {
        let mut rows: Vec<&MatchedRow> = self
            .sibling_rows
            .iter()
            .filter(|row| {
                // Filter to rows whose source observation matches our
                // query's material family. We don't carry the obs back
                // through MatchedRow, so we use hardness as a proxy:
                // rows whose calibrated hardness equals the query's are
                // same-material. This is exact for hardwood/softwood/MDF
                // (each family has a single Janka value) and acceptable
                // for plastics.
                match (
                    self.query_hardness_kind,
                    self.query_hardness_value,
                    row.chipload_hardness_scale,
                ) {
                    (Some(_), Some(_), scale) => (scale - 1.0).abs() < 1e-3,
                    _ => true,
                }
            })
            .collect();
        rows.sort_by(|a, b| a.row_diameter_mm.total_cmp(&b.row_diameter_mm));
        rows
    }

    /// Subset of sibling rows that share the query's diameter (within
    /// 10 %), sorted by row hardness. Used by Chart B (chipload vs
    /// hardness). When no sibling matches the diameter, falls back to
    /// the matched row's calibrated diameter band.
    pub fn rows_by_hardness(&self) -> Vec<&MatchedRow> {
        let target = self.tool_diameter_mm;
        let mut rows: Vec<&MatchedRow> = self
            .sibling_rows
            .iter()
            .filter(|row| {
                if target <= 0.0 || row.row_diameter_mm <= 0.0 {
                    return false;
                }
                let ratio = row.row_diameter_mm / target;
                (0.9..=1.1).contains(&ratio)
            })
            .collect();
        if rows.is_empty()
            && let Some(matched) = &self.matched_row
        {
            rows = self
                .sibling_rows
                .iter()
                .filter(|row| (row.row_diameter_mm - matched.row_diameter_mm).abs() < 0.05)
                .collect();
        }
        // Sort by chipload-hardness-scale ascending — rows with smaller
        // calibrated hardness produce a larger scale factor (softer
        // material → higher chipload). UI plots scale vs chipload.
        rows.sort_by(|a, b| {
            a.chipload_hardness_scale
                .total_cmp(&b.chipload_hardness_scale)
        });
        rows
    }
}

/// Build the explanation payload for the given input. Runs the same
/// pipeline as [`calculate`] but also enumerates sibling vendor rows
/// and snapshots the machine envelope.
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
    let (query, matched_row, sibling_rows) = match (input.vendor_lut, routed_query) {
        (Some(lut), Some(query)) => {
            // Use the geometry-aware dispatcher so V-bit "matched row"
            // honours the cutter's cone angle — the sibling-rows widening
            // below still walks the whole family for chart rendering.
            let matched = find_best_row_for_geometry(lut, &query, &input.tool_geometry);
            let mut family_query = query.clone();
            // For "siblings" we widen the query to the whole tool-family
            // / op / pass_role cohort by relaxing the diameter and
            // hardness ratios. enumerate_matching_rows already runs each
            // sibling through passes_must_match + scoring; that's enough
            // for chart rendering.
            family_query.diameter_mm = lut_family_anchor_diameter(lut, &query);
            let siblings = enumerate_matching_rows(lut, &family_query);
            (query, matched, siblings)
        }
        _ => {
            // No LUT, or the routing refused — synthesise a query record
            // for echo, leave sibling_rows empty. The modal shows the
            // formula-fallback state in this branch.
            let query = synthetic_query(input);
            (query, None, Vec::new())
        }
    };

    FeedsExplain {
        recommended,
        query: query.clone(),
        matched_row,
        sibling_rows,
        tool_diameter_mm: input.tool_diameter,
        shank_diameter_mm: input.shank_diameter.unwrap_or(input.tool_diameter),
        tool_geometry: input.tool_geometry,
        flute_count: input.flute_count,
        query_hardness_kind: query.hardness_kind,
        query_hardness_value: query.hardness_value,
        machine,
    }
}

/// Pick a representative diameter for sibling enumeration. We use the
/// query's diameter so siblings near the calibrated range come out at
/// reasonable scoring, but the relaxed must-match filter (max ratio
/// 20×) lets the chart still pick up the 1 mm and 12.7 mm rows.
fn lut_family_anchor_diameter(_lut: &VendorLut, query: &LookupQuery) -> f64 {
    query.diameter_mm
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
    use crate::feeds::vendor_lut::{LutOperationFamily, LutPassRole, MaterialFamily, ToolFamily};
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
        // Sibling enumeration should include at least the matched row.
        assert!(!exp.sibling_rows.is_empty());
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
        assert!(exp.sibling_rows.is_empty());
    }

    #[test]
    fn rows_by_diameter_filters_to_same_material() {
        let mat = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let mach = MachineProfile::shapeoko_vfd();
        let lut = embedded_vendor_lut();
        let input = make_input(&mat, &mach, Some(lut));

        let exp = explain(&input);
        let rows = exp.rows_by_diameter();
        // Every row returned should have hardness_scale ~= 1.0 (same Janka).
        for r in &rows {
            assert!((r.chipload_hardness_scale - 1.0).abs() < 1e-3);
        }
        // Sorted ascending by diameter.
        for w in rows.windows(2) {
            assert!(w[0].row_diameter_mm <= w[1].row_diameter_mm);
        }
    }

    #[test]
    fn rows_by_hardness_filters_to_same_diameter() {
        let mat = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let mach = MachineProfile::shapeoko_vfd();
        let lut = embedded_vendor_lut();
        let input = make_input(&mat, &mach, Some(lut));

        let exp = explain(&input);
        let rows = exp.rows_by_hardness();
        // Every row should have a diameter within ±10 % of 6.35 mm.
        for r in &rows {
            let ratio = r.row_diameter_mm / 6.35;
            assert!(
                (0.9..=1.1).contains(&ratio),
                "diameter {} out of band",
                r.row_diameter_mm
            );
        }
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
