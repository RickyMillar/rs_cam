//! Behavioural-parity test for Item B of the tool-load fidelity plan.
//!
//! Calculator (`feeds::calculate`) and gate (`tool_load::chipload::evaluate`)
//! both query the vendor LUT via `find_best_row`. The plan requires that —
//! for the same conceptual input — both pick the same observation_id, so a
//! suggested feed can't disagree with the gate's verdict bounds.
//!
//! This test sweeps a set of canonical (tool, material, op, pass) tuples,
//! routes each through both consumers' query-construction code, and asserts
//! the matched row's observation_id agrees.
//!
//! Cases that intentionally diverge (project_curve, where the gate maps to
//! parallel/finish to reach ball-nose data) are NOT tested here — that
//! divergence is a feature, not a parity bug.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::feeds::vendor_lookup::{LookupQuery, find_best_row};
use rs_cam_core::feeds::vendor_lut::{
    HardnessKind, LutOperationFamily, LutPassRole, MaterialFamily, ToolFamily, VendorLut,
};
use rs_cam_core::feeds::{FeedsInput, OperationFamily, PassRole, SetupContext, ToolGeometryHint};
use rs_cam_core::feeds::{vendor_lookup, vendor_normalize};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::tool::{BallEndmill, FlatEndmill, MillingCutter, ToolDefinition};

struct Case {
    name: &'static str,
    diameter_mm: f64,
    flute_count: u32,
    tool_geometry: ToolGeometryHint,
    cutter: Box<dyn MillingCutter>,
    species: WoodSpecies,
    operation: OperationFamily,
    pass_role: PassRole,
    /// The op_family/pass_role tuple the gate sees after `routed_lookup_family`.
    /// For non-project_curve operations this matches `(operation, pass_role)`.
    gate_op_family: LutOperationFamily,
    gate_pass_role: LutPassRole,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "softwood-flat-6mm-pocket-rough",
            diameter_mm: 6.0,
            flute_count: 2,
            tool_geometry: ToolGeometryHint::Flat,
            cutter: Box::new(FlatEndmill::new(6.0, 18.0)),
            species: WoodSpecies::GenericSoftwood,
            operation: OperationFamily::Pocket,
            pass_role: PassRole::Roughing,
            gate_op_family: LutOperationFamily::Pocket,
            gate_pass_role: LutPassRole::Roughing,
        },
        Case {
            name: "softwood-flat-6mm-adaptive-rough",
            diameter_mm: 6.0,
            flute_count: 2,
            tool_geometry: ToolGeometryHint::Flat,
            cutter: Box::new(FlatEndmill::new(6.0, 18.0)),
            species: WoodSpecies::GenericSoftwood,
            operation: OperationFamily::Adaptive,
            pass_role: PassRole::Roughing,
            gate_op_family: LutOperationFamily::Adaptive,
            gate_pass_role: LutPassRole::Roughing,
        },
        Case {
            name: "hardwood-flat-3mm-contour-finish",
            diameter_mm: 3.175,
            flute_count: 2,
            tool_geometry: ToolGeometryHint::Flat,
            cutter: Box::new(FlatEndmill::new(3.175, 12.0)),
            species: WoodSpecies::HardMaple,
            operation: OperationFamily::Contour,
            pass_role: PassRole::Finish,
            gate_op_family: LutOperationFamily::Contour,
            gate_pass_role: LutPassRole::Finish,
        },
        Case {
            name: "softwood-ball-3mm-parallel-finish",
            diameter_mm: 3.175,
            flute_count: 2,
            tool_geometry: ToolGeometryHint::Ball,
            cutter: Box::new(BallEndmill::new(3.175, 12.0)),
            species: WoodSpecies::GenericSoftwood,
            operation: OperationFamily::Parallel,
            pass_role: PassRole::Finish,
            gate_op_family: LutOperationFamily::Parallel,
            gate_pass_role: LutPassRole::Finish,
        },
        Case {
            name: "hardwood-ball-6mm-scallop-finish",
            diameter_mm: 6.0,
            flute_count: 2,
            tool_geometry: ToolGeometryHint::Ball,
            cutter: Box::new(BallEndmill::new(6.0, 18.0)),
            species: WoodSpecies::HardMaple,
            operation: OperationFamily::Scallop,
            pass_role: PassRole::Finish,
            gate_op_family: LutOperationFamily::Scallop,
            gate_pass_role: LutPassRole::Finish,
        },
    ]
}

#[test]
fn calculator_and_gate_match_same_observation_id() {
    let lut = VendorLut::embedded();
    let machine = MachineProfile::shapeoko_vfd();

    let mut mismatches: Vec<String> = Vec::new();

    for case in cases() {
        let material = Material::SolidWood {
            species: case.species,
        };

        // --- Calculator path ---
        let input = FeedsInput {
            tool_diameter: case.diameter_mm,
            flute_count: case.flute_count,
            flute_length: 18.0,
            shank_diameter: None,
            tool_geometry: case.tool_geometry,
            material: &material,
            machine: &machine,
            operation: case.operation,
            pass_role: case.pass_role,
            axial_depth_mm: None,
            radial_width_mm: None,
            target_scallop_mm: None,
            vendor_lut: Some(&lut),
            setup: SetupContext::default(),
        };
        let calc_query = vendor_normalize::to_lookup_query(&input);
        let calc_result = find_best_row(&lut, &calc_query);

        // --- Gate path ---
        let tool = ToolDefinition::new(
            case.cutter,
            6.35,
            30.0,
            20.0,
            60.0,
            case.flute_count,
            ToolMaterial::Carbide,
        );
        let (material_family, hardness_kind, hardness_value) = material_to_lut_for_test(&material);
        // Mirror the gate's construction: lookup_diameter_at(axial_doc).
        // For Flat/Ball tools axial_doc doesn't change the diameter, so any
        // representative value works. Use the diameter as the axial DOC —
        // matches the calculator's `lookup_diameter_for_input` default.
        let axial_doc = case.diameter_mm;
        let gate_query = LookupQuery {
            tool_family: tool_family_for_geom(case.tool_geometry),
            tool_subfamily: None,
            diameter_mm: tool.lookup_diameter_at(axial_doc),
            flute_count: case.flute_count,
            material_family,
            hardness_kind: Some(hardness_kind),
            hardness_value: Some(hardness_value),
            operation_family: case.gate_op_family,
            pass_role: case.gate_pass_role,
        };
        let gate_result = find_best_row(&lut, &gate_query);

        // --- Compare ---
        match (&calc_result, &gate_result) {
            (Some(c), Some(g)) if c.observation_id == g.observation_id => { /* parity */ }
            (None, None) => { /* both refused — also parity */ }
            (c, g) => {
                mismatches.push(format!(
                    "case '{}': calc={:?}, gate={:?}",
                    case.name,
                    c.as_ref().map(|r| r.observation_id.as_str()),
                    g.as_ref().map(|r| r.observation_id.as_str()),
                ));
            }
        }
    }

    assert!(
        mismatches.is_empty(),
        "calculator and gate disagree on canonical inputs:\n  - {}",
        mismatches.join("\n  - ")
    );
}

#[test]
fn diameter_match_score_surfaces_for_consumers() {
    // Phase 3's suggest module wants to flag matches that won on tie-breakers
    // despite a poor diameter fit. The score is exposed on `LookupResult`.
    let lut = VendorLut::embedded();
    let exact_query = LookupQuery {
        tool_family: ToolFamily::FlatEnd,
        tool_subfamily: None,
        diameter_mm: 6.0,
        flute_count: 2,
        material_family: MaterialFamily::Softwood,
        hardness_kind: Some(HardnessKind::Janka),
        hardness_value: Some(600.0),
        operation_family: LutOperationFamily::Adaptive,
        pass_role: LutPassRole::Roughing,
    };
    let exact = vendor_lookup::lookup_best(&lut, &exact_query)
        .expect("6mm flat softwood adaptive rough should match");

    let derated_query = LookupQuery {
        diameter_mm: 4.5, // off by 25% from 6mm
        ..exact_query
    };
    let derated = vendor_lookup::lookup_best(&lut, &derated_query)
        .expect("4.5mm should still match within the 0.5x-2x ratio band");

    // Exact match should score higher on diameter than the derated one.
    assert!(
        exact.diameter_match_score >= derated.diameter_match_score,
        "exact diameter should score >= derated: exact={}, derated={}",
        exact.diameter_match_score,
        derated.diameter_match_score,
    );
    // The score lives in [0, 200].
    assert!((0..=200).contains(&exact.diameter_match_score));
    assert!((0..=200).contains(&derated.diameter_match_score));
}

// --- helpers ---

fn tool_family_for_geom(hint: ToolGeometryHint) -> ToolFamily {
    match hint {
        ToolGeometryHint::Flat => ToolFamily::FlatEnd,
        ToolGeometryHint::Ball => ToolFamily::BallNose,
        ToolGeometryHint::Bull { .. } => ToolFamily::BullNose,
        ToolGeometryHint::VBit { .. } => ToolFamily::ChamferVbit,
        ToolGeometryHint::TaperedBall { .. } => ToolFamily::TaperedBallNose,
    }
}

fn material_to_lut_for_test(material: &Material) -> (MaterialFamily, HardnessKind, f64) {
    // Mirrors `vendor_normalize::material_to_lut` (pub(crate)) for test access.
    match material {
        Material::SolidWood { species } => {
            let janka = species.janka_lbf();
            let family = if janka >= 1000.0 {
                MaterialFamily::Hardwood
            } else {
                MaterialFamily::Softwood
            };
            (family, HardnessKind::Janka, janka)
        }
        _ => panic!("test cases only use solid wood"),
    }
}
