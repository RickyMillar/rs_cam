//! Maps rs_cam_core types to vendor LUT query types.

use super::vendor_lookup::LookupQuery;
use super::vendor_lut::{
    HardnessKind, LutOperationFamily, LutPassRole, MaterialFamily, ToolFamily,
};
use super::{FeedsInput, OperationFamily, PassRole, ToolGeometryHint};
use crate::material::{Material, PlasticHardness};

/// Convert a FeedsInput to a LookupQuery for vendor LUT lookup.
pub fn to_lookup_query(input: &FeedsInput) -> LookupQuery {
    let tool_family = match input.tool_geometry {
        ToolGeometryHint::Flat => ToolFamily::FlatEnd,
        ToolGeometryHint::Ball => ToolFamily::BallNose,
        ToolGeometryHint::Bull { .. } => ToolFamily::BullNose,
        ToolGeometryHint::VBit { .. } => ToolFamily::ChamferVbit,
        ToolGeometryHint::TaperedBall { .. } => ToolFamily::TaperedBallNose,
    };

    let (material_family, hardness_kind, hardness_value) = material_to_lut(input.material);

    let operation_family = match input.operation {
        OperationFamily::Adaptive => LutOperationFamily::Adaptive,
        OperationFamily::Pocket => LutOperationFamily::Pocket,
        OperationFamily::Contour => LutOperationFamily::Contour,
        OperationFamily::Parallel => LutOperationFamily::Parallel,
        OperationFamily::Scallop => LutOperationFamily::Scallop,
        OperationFamily::Trace => LutOperationFamily::Trace,
        OperationFamily::Face => LutOperationFamily::Face,
    };

    let pass_role = match input.pass_role {
        PassRole::Roughing => LutPassRole::Roughing,
        PassRole::SemiFinish => LutPassRole::SemiFinish,
        PassRole::Finish => LutPassRole::Finish,
    };

    LookupQuery {
        tool_family,
        tool_subfamily: None,
        diameter_mm: lookup_diameter_for_input(input),
        flute_count: input.flute_count,
        material_family,
        hardness_kind: Some(hardness_kind),
        hardness_value: Some(hardness_value),
        operation_family,
        pass_role,
    }
}

fn lookup_diameter_for_input(input: &FeedsInput<'_>) -> f64 {
    let axial_doc = input.axial_depth_mm.unwrap_or(input.tool_diameter).max(0.0);
    match input.tool_geometry {
        ToolGeometryHint::TaperedBall {
            tip_radius,
            taper_angle_deg,
        } => {
            let alpha = taper_angle_deg.to_radians();
            let sin_alpha = alpha.sin();
            let cos_alpha = alpha.cos();
            let tan_alpha = alpha.tan();
            if tip_radius <= 0.0 || tan_alpha <= 0.0 {
                return input.tool_diameter;
            }
            let h_contact = tip_radius * (1.0 - sin_alpha);
            let r_contact = tip_radius * cos_alpha;
            let cone_offset = h_contact - r_contact / tan_alpha;
            let radius = if axial_doc <= h_contact {
                (2.0 * tip_radius * axial_doc - axial_doc * axial_doc)
                    .max(0.0)
                    .sqrt()
            } else {
                (axial_doc - cone_offset) * tan_alpha
            };
            (2.0 * radius).clamp(0.0, input.shank_diameter.unwrap_or(input.tool_diameter))
        }
        ToolGeometryHint::VBit { included_angle, .. } => {
            let half = (included_angle * 0.5).to_radians();
            (2.0 * axial_doc * half.tan()).clamp(0.0, input.tool_diameter)
        }
        ToolGeometryHint::Flat | ToolGeometryHint::Ball | ToolGeometryHint::Bull { .. } => {
            input.tool_diameter
        }
    }
}

/// Translate the application-side `Material` into the LUT-side
/// `(MaterialFamily, HardnessKind, hardness_value)` triple a vendor
/// row lookup needs.
///
/// All hardness values come from the canonical accessors on
/// `WoodSpecies` / `PlywoodGrade` / `SheetGoodKind` /
/// `PlasticFamily::hardness()` / `AluminumAlloy::brinell_hb()` so
/// there's exactly ONE source of truth per material. Pre-Phase-1E
/// this function had inline tables that disagreed with the canonical
/// accessors (Acrylic was Shore D 85 here vs Rockwell M 93 on
/// `PlasticFamily::hardness()`).
pub(crate) fn material_to_lut(material: &Material) -> (MaterialFamily, HardnessKind, f64) {
    use crate::material::{PlywoodGrade, SheetGoodKind};
    match material {
        Material::SolidWood { species } => {
            let janka = species.janka_lbf();
            let family = if janka <= 800.0 {
                MaterialFamily::Softwood
            } else {
                MaterialFamily::Hardwood
            };
            (family, HardnessKind::Janka, janka)
        }
        // Parametric solid-wood — Phase E 2026-05-31. Same softwood/
        // hardwood split as the enum variant; the LUT row matcher
        // takes care of band scoring from the actual Janka value.
        Material::SolidWoodByJanka { janka_lbf, .. } => {
            let family = if *janka_lbf <= 800.0 {
                MaterialFamily::Softwood
            } else {
                MaterialFamily::Hardwood
            };
            (family, HardnessKind::Janka, *janka_lbf)
        }
        Material::Plywood { grade } => {
            let family = match grade {
                PlywoodGrade::Softwood => MaterialFamily::PlywoodSoftwood,
                PlywoodGrade::BalticBirch | PlywoodGrade::HardwoodFaced => {
                    MaterialFamily::PlywoodHardwood
                }
            };
            (family, HardnessKind::Janka, grade.effective_janka_lbf())
        }
        Material::SheetGood { kind } => {
            let family = match kind {
                SheetGoodKind::Mdf => MaterialFamily::Mdf,
                SheetGoodKind::Hdf => MaterialFamily::Hdf,
                SheetGoodKind::Particleboard => MaterialFamily::Particleboard,
            };
            (family, HardnessKind::Janka, kind.effective_janka_lbf())
        }
        Material::Plastic { family } => {
            // Per-family LUT material class for row matching. The LUT's
            // MaterialFamily enum has only Acrylic / Hdpe /
            // Polycarbonate / Delrin on the plastic side, so the
            // Phase D (2026-05-31) UHMW/PP/Nylon/ABS/PETG/PVC additions
            // bin into the closest available chemistry — these are
            // best-effort routing hints, not citation-backed mappings.
            // When no vendor LUT row exists (the common case for the
            // newer families) the lookup misses cleanly and the
            // formula fallback takes over, so the binning has
            // minimal practical impact today. TODO Phase E+: extend
            // `MaterialFamily` with per-family bins as LUT rows land.
            let lut_family = match family {
                crate::material::PlasticFamily::Acrylic
                | crate::material::PlasticFamily::Generic
                // ABS / PETG / PVC are amorphous rigid sheet plastics
                // — Acrylic is the LUT's closest "rigid amorphous" bin.
                | crate::material::PlasticFamily::Abs
                | crate::material::PlasticFamily::Petg
                | crate::material::PlasticFamily::RigidPvc => MaterialFamily::Acrylic,
                // UHMW-PE / PP are polyolefins like HDPE; HDPE is the
                // LUT's closest polyolefin bin.
                crate::material::PlasticFamily::Hdpe
                | crate::material::PlasticFamily::UhmwPe
                | crate::material::PlasticFamily::Polypropylene => MaterialFamily::Hdpe,
                // Nylon 6/6 routes to Delrin (the LUT's engineering-
                // thermoplastic bin — POM and PA are both crystalline
                // engineering polymers with comparable machining
                // behaviour).
                crate::material::PlasticFamily::Delrin
                | crate::material::PlasticFamily::Nylon66 => MaterialFamily::Delrin,
                crate::material::PlasticFamily::Polycarbonate => MaterialFamily::Polycarbonate,
            };
            // Canonical hardness from PlasticFamily::hardness().
            // PMMA reads in Rockwell M natively, ABS/PETG read in
            // Rockwell R — the LUT row's hardness scaling needs the
            // matching kind. When the family has no fetched hardness
            // (Generic), default to a Shore-D-equivalent 80 to keep
            // the lookup numeric without inventing a citation.
            let (hardness_kind, hardness_value) = match family.hardness() {
                Some(PlasticHardness::ShoreD(v)) => (HardnessKind::ShoreD, v),
                Some(PlasticHardness::RockwellM(v)) | Some(PlasticHardness::RockwellR(v)) => {
                    // The LUT currently models only Janka / Hb / ShoreD.
                    // Rockwell M (PMMA) and Rockwell R (ABS/PETG) are
                    // both surfaced under ShoreD as comparable-magnitude
                    // scalars until the LUT grows a Rockwell kind.
                    // TODO Phase 3+: plumb Rockwell HardnessKind through
                    // `vendor_lut.rs` (would need per-row Rockwell
                    // scale annotations on the vendor side too).
                    (HardnessKind::ShoreD, v)
                }
                None => (HardnessKind::ShoreD, 80.0),
            };
            (lut_family, hardness_kind, hardness_value)
        }
        Material::Aluminum { alloy } => (
            MaterialFamily::Aluminum,
            HardnessKind::Hb,
            alloy.brinell_hb(),
        ),
        Material::Fiberglass { .. } => {
            // Fiber-reinforced composites — own MaterialFamily +
            // material_category (3) in vendor_lookup. No fetched
            // hardness measurement on the wood / metal scale; emit
            // ShoreD as a structural placeholder so the lookup is
            // numerically defined (LUT row scoring against this
            // value contributes 0 since no Fiberglass rows currently
            // carry a hardness annotation).
            (MaterialFamily::Fiberglass, HardnessKind::ShoreD, 110.0)
        }
        Material::Foam { .. } => {
            // Foam has no LUT data — formula fallback regardless of the
            // values we return here. The triple is structural noise.
            (MaterialFamily::Softwood, HardnessKind::Janka, 200.0)
        }
        Material::Custom {
            feed_scale_factor, ..
        } => {
            // `Material::Custom` is the user-typed escape hatch — it
            // carries no true Janka reading. For LUT matching we need
            // *some* `(MaterialFamily, HardnessKind, value)` triple,
            // so we heuristically project `feed_scale_factor` onto a
            // wood-equivalent Janka by inverting the wood-class
            // `factor = (janka / 600)^0.4` formula.
            //
            // This is documented as a heuristic, not a physical
            // hardness — the alternative (refusing the lookup) would
            // silently break the vendor LUT fallback that every Custom
            // user depends on.
            //
            // S2-8 Option B note: `Material::wood_hardness_lbf()`
            // correctly returns `None` for `Custom`; this heuristic
            // Janka is local to the LUT matcher and does NOT round-trip
            // back through the material API.
            let janka = feed_scale_factor * 600.0;
            let family = if janka <= 800.0 {
                MaterialFamily::Softwood
            } else {
                MaterialFamily::Hardwood
            };
            (family, HardnessKind::Janka, janka)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::machine::MachineProfile;
    use crate::material::{PlasticFamily, SheetGoodKind, WoodSpecies};

    fn make_input<'a>(
        geom: ToolGeometryHint,
        material: &'a Material,
        machine: &'a MachineProfile,
        op: OperationFamily,
    ) -> FeedsInput<'a> {
        FeedsInput {
            tool_diameter: 6.0,
            flute_count: 2,
            flute_length: 18.0,
            shank_diameter: None,
            tool_geometry: geom,
            material,
            machine,
            operation: op,
            pass_role: PassRole::Roughing,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: None,
            setup: Default::default(),
        }
    }

    #[test]
    fn test_flat_softwood_maps_correctly() {
        let mat = Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        };
        let mach = MachineProfile::shapeoko_vfd();
        let input = make_input(
            ToolGeometryHint::Flat,
            &mat,
            &mach,
            OperationFamily::Adaptive,
        );
        let query = to_lookup_query(&input);
        assert_eq!(query.tool_family, ToolFamily::FlatEnd);
        assert_eq!(query.material_family, MaterialFamily::Softwood);
        assert_eq!(query.hardness_value, Some(600.0));
        assert_eq!(query.operation_family, LutOperationFamily::Adaptive);
    }

    #[test]
    fn test_ball_hardwood_maps_correctly() {
        let mat = Material::SolidWood {
            species: WoodSpecies::HardMaple,
        };
        let mach = MachineProfile::shapeoko_vfd();
        let input = make_input(
            ToolGeometryHint::Ball,
            &mat,
            &mach,
            OperationFamily::Parallel,
        );
        let query = to_lookup_query(&input);
        assert_eq!(query.tool_family, ToolFamily::BallNose);
        assert_eq!(query.material_family, MaterialFamily::Hardwood);
        assert_eq!(query.hardness_value, Some(1450.0));
    }

    #[test]
    fn test_acrylic_maps_to_canonical_pmma_hardness() {
        // Phase 1E consolidation: vendor_normalize now routes through
        // PlasticFamily::hardness() instead of an inline table. PMMA
        // is reported in Rockwell M (MakeItFrom citation) — the
        // hardness number is the literature value, not the historical
        // inline ShoreD 85. The LUT currently only carries ShoreD as
        // its scalar kind for plastics, so we surface the RockwellM
        // value via ShoreD — see TODO in material_to_lut to plumb a
        // RockwellM HardnessKind end-to-end.
        let mat = Material::Plastic {
            family: PlasticFamily::Acrylic,
        };
        let mach = MachineProfile::shapeoko_vfd();
        let input = make_input(
            ToolGeometryHint::Flat,
            &mat,
            &mach,
            OperationFamily::Contour,
        );
        let query = to_lookup_query(&input);
        assert_eq!(query.material_family, MaterialFamily::Acrylic);
        assert_eq!(query.hardness_kind, Some(HardnessKind::ShoreD));
        assert_eq!(query.hardness_value, Some(93.0));
    }

    #[test]
    fn test_mdf_maps_correctly() {
        let mat = Material::SheetGood {
            kind: SheetGoodKind::Mdf,
        };
        let mach = MachineProfile::shapeoko_vfd();
        let input = make_input(ToolGeometryHint::Flat, &mat, &mach, OperationFamily::Pocket);
        let query = to_lookup_query(&input);
        assert_eq!(query.material_family, MaterialFamily::Mdf);
        assert_eq!(query.hardness_value, Some(1100.0));
    }
}
