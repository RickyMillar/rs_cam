//! Maps rs_cam_core types to vendor LUT query types.

use super::vendor_lookup::LookupQuery;
use super::vendor_lut::*;
use super::{FeedsInput, OperationFamily, PassRole, ToolGeometryHint};
use crate::material::*;

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
        diameter_mm: input.tool_diameter,
        flute_count: input.flute_count,
        material_family,
        hardness_kind: Some(hardness_kind),
        hardness_value: Some(hardness_value),
        operation_family,
        pass_role,
    }
}

fn material_to_lut(material: &Material) -> (MaterialFamily, HardnessKind, f64) {
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
        Material::Plywood { grade } => match grade {
            PlywoodGrade::Softwood => (MaterialFamily::PlywoodSoftwood, HardnessKind::Janka, 600.0),
            PlywoodGrade::BalticBirch => {
                (MaterialFamily::PlywoodHardwood, HardnessKind::Janka, 1200.0)
            }
            PlywoodGrade::HardwoodFaced => {
                (MaterialFamily::PlywoodHardwood, HardnessKind::Janka, 1000.0)
            }
        },
        Material::SheetGood { kind } => match kind {
            SheetGoodKind::Mdf => (MaterialFamily::Mdf, HardnessKind::Janka, 1100.0),
            SheetGoodKind::Hdf => (MaterialFamily::Hdf, HardnessKind::Janka, 1300.0),
            SheetGoodKind::Particleboard => {
                (MaterialFamily::Particleboard, HardnessKind::Janka, 750.0)
            }
        },
        Material::Plastic { family } => match family {
            PlasticFamily::Acrylic => (MaterialFamily::Acrylic, HardnessKind::ShoreD, 85.0),
            PlasticFamily::Hdpe => (MaterialFamily::Hdpe, HardnessKind::ShoreD, 65.0),
            PlasticFamily::Delrin => (MaterialFamily::Delrin, HardnessKind::ShoreD, 85.0),
            PlasticFamily::Polycarbonate => {
                (MaterialFamily::Polycarbonate, HardnessKind::ShoreD, 80.0)
            }
            PlasticFamily::Generic => (MaterialFamily::Acrylic, HardnessKind::ShoreD, 80.0),
        },
        Material::Foam { .. } => {
            // Foam has no LUT data — will fall through to formula
            (MaterialFamily::Softwood, HardnessKind::Janka, 200.0)
        }
        Material::Custom { hardness_index, .. } => {
            // Map custom to softwood/hardwood based on hardness
            let janka = hardness_index * 600.0; // reverse of hardness_index formula
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
mod tests {
    use super::*;
    use crate::machine::MachineProfile;

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
    fn test_acrylic_maps_correctly() {
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
        assert_eq!(query.hardness_value, Some(85.0));
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
