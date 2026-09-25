//! G6 (drill, ruling B5) — a flat end mill plunge is the side chip / Z.
//!
//! Pre-registration: `planning/extrapolation_2026-09-24/B5_PLAN.md` §1 and
//! the orchestrator's decisions at its top.
//!
//! The Amana Spektra Spiral Plunge chart v24 prints "Ramp Down" = Feed Rate
//! IPM / # of flutes at 18 000 RPM. The feed rate is RPM x side chip x Z, so
//! the axial advance per tooth is the side chip / Z. The drill rule
//! (`feeds::extrapolation::drill`) reads the tool's own Spektra side row
//! (`amana_flat_end.json`, filed under (Pocket, Roughing)) and divides it
//! by Z. The chip is held per tooth; the engine drill RPM cap (14 000 at
//! D <= 6 mm) replaces the chart's 18 000, so the feed is under the printed
//! Ramp Down.
//!
//! How the numbers were derived (python3 over
//! `planning/extrapolation_2026-09-24/fetch/G6/verified_rows.json`, the 8
//! `x-g6-amana-spektra-*-rampdown` rows, and over
//! `data/vendor_lut/observations/amana_flat_end.json`):
//!
//! | D (mm) | Z | column | Ramp Down (in/min) | verified chip | side row | side / Z | ratio |
//! |---|---|---|---|---|---|---|---|
//! | 3.175 | 2 | Wood/Plywood | 72.5 | 0.0512 | 0.1016 | 0.0508 | 0.9922 |
//! | 3.175 | 2 | MDF/Laminate | 90 | 0.0635 | 0.127 | 0.0635 | 1.0000 |
//! | 6.0 | 2 | Wood/Plywood | 90 | 0.0635 | 0.127 | 0.0635 | 1.0000 |
//! | 6.0 | 2 | MDF/Laminate | 107.5 | 0.0758 | 0.1524 | 0.0762 | 1.0053 |
//! | 3.175 | 3 | Wood/Plywood | 72 | 0.0339 | 0.1016 | 0.03387 | 0.9990 |
//! | 3.175 | 3 | MDF/Laminate | 90 | 0.0423 | 0.127 | 0.04233 | 1.0008 |
//! | 6.0 | 3 | Wood/Plywood | 90 | 0.0423 | 0.127 | 0.04233 | 1.0008 |
//! | 6.0 | 3 | MDF/Laminate | 109 | 0.0513 | 0.1524 | 0.0508 | 0.9903 |
//!
//! The worst ratio is 0.9903 (Amana rounds the printed in/min), so the bar
//! is 1 %. At 14 000 RPM the feed is 14 000 x chip x Z: 1422.4, 1778.0 or
//! 2133.6 mm/min, which is 0.77-0.78x the printed Ramp Down (x 25.4). The
//! MDF 3.175 mm cell sits at 1778.0 / 3.175 = 560 mm/min per mm, under the
//! 720 sheet-goods ceiling.
//!
//! The arms:
//!
//! - (a) each verified cell ships through the drill claim, its chip is the
//!   verified chip within 1 %, its RPM is the cap, and its feed is under
//!   the printed Ramp Down;
//! - (b) the Suggest recipe and the gate's envelope resolver read one row,
//!   and the scaled point is still a point;
//! - (c) a flat query outside the claim (3.0 mm, 6.35 mm, 1 or 4 flutes)
//!   refuses with a text that names G6;
//! - (d) a ball, bull, tapered-ball or V-bit plunge refuses with G6, and a
//!   bull query never reads a Spektra row;
//! - (e) a plunge in plastic or aluminium refuses (decision 3);
//! - (f) the MDF 3.175 mm cell reads Within on the plunge-feed gate.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::extrapolation::{DRILL_RULE_TEXT, Gap};
use rs_cam_core::feeds::suggest::{
    StockContext, SuggestContext, SuggestParamsInput, SuggestedParams, suggest_params,
};
use rs_cam_core::feeds::vendor_lookup::{
    LookupQuery, PrintedChipload, find_best_chip_envelope_row, find_best_row_for_geometry,
};
use rs_cam_core::feeds::vendor_lut::{
    HardnessKind, LutOperationFamily, LutPassRole, MaterialFamily, ToolFamily,
};
use rs_cam_core::feeds::{
    EMBEDDED_LUT, FeedsError, FeedsSupport, SpindleStrategy, ToolGeometryHint, embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{
    AluminumAlloy, Material, PlasticFamily, PlywoodGrade, SheetGoodKind, WoodSpecies,
};
use rs_cam_core::tool_load::drill_gates::{DrillGateOutcome, classify_plunge_feed};

/// The relative bar on the chip against the verified chip (see the table).
const CHIP_TOL: f64 = 0.01;

/// The engine drill RPM cap at D <= 6 mm (repo rule).
const RPM_CAP: f64 = 14_000.0;

/// One verified Ramp Down cell of `fetch/G6/verified_rows.json`.
struct Verified {
    diameter_mm: f64,
    flutes: u32,
    mdf: bool,
    ramp_down_ipm: f64,
    chip_mm: f64,
}

/// The 8 verified cells (python3 over `verified_rows.json`).
const VERIFIED: [Verified; 8] = [
    Verified {
        diameter_mm: 3.175,
        flutes: 2,
        mdf: false,
        ramp_down_ipm: 72.5,
        chip_mm: 0.0512,
    },
    Verified {
        diameter_mm: 3.175,
        flutes: 2,
        mdf: true,
        ramp_down_ipm: 90.0,
        chip_mm: 0.0635,
    },
    Verified {
        diameter_mm: 6.0,
        flutes: 2,
        mdf: false,
        ramp_down_ipm: 90.0,
        chip_mm: 0.0635,
    },
    Verified {
        diameter_mm: 6.0,
        flutes: 2,
        mdf: true,
        ramp_down_ipm: 107.5,
        chip_mm: 0.0758,
    },
    Verified {
        diameter_mm: 3.175,
        flutes: 3,
        mdf: false,
        ramp_down_ipm: 72.0,
        chip_mm: 0.0339,
    },
    Verified {
        diameter_mm: 3.175,
        flutes: 3,
        mdf: true,
        ramp_down_ipm: 90.0,
        chip_mm: 0.0423,
    },
    Verified {
        diameter_mm: 6.0,
        flutes: 3,
        mdf: false,
        ramp_down_ipm: 90.0,
        chip_mm: 0.0423,
    },
    Verified {
        diameter_mm: 6.0,
        flutes: 3,
        mdf: true,
        ramp_down_ipm: 109.0,
        chip_mm: 0.0513,
    },
];

fn hardwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    }
}

fn softwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    }
}

fn plywood() -> Material {
    Material::Plywood {
        grade: PlywoodGrade::BalticBirch,
    }
}

fn mdf() -> Material {
    Material::SheetGood {
        kind: SheetGoodKind::Mdf,
    }
}

/// The materials that read one chart column: the Wood/Plywood column
/// serves softwood, hardwood and plywood; the MDF/Laminate column MDF.
fn materials_of(v: &Verified) -> Vec<(&'static str, Material)> {
    if v.mdf {
        vec![("mdf", mdf())]
    } else {
        vec![
            ("softwood", softwood()),
            ("hardwood", hardwood()),
            ("plywood-hardwood", plywood()),
        ]
    }
}

/// The side row the claim must read, by the LUT's id scheme.
fn side_row_id(material: &str, d: f64, z: u32) -> String {
    let size = if (d - 3.175).abs() < 1e-9 {
        "3175"
    } else {
        "6000"
    };
    format!("amana-flat-{material}-pocket-{size}-{z}f-spektra")
}

fn tool_of(kind: ToolType, diameter: f64, flutes: u32) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(1), kind);
    t.diameter = diameter;
    t.flute_count = flutes;
    t.cutting_length = (diameter * 4.0).max(12.0);
    t.shank_diameter = diameter.max(3.175);
    t.shaft_diameter = diameter.max(3.175);
    t.stickout = t.cutting_length + 8.0;
    if matches!(kind, ToolType::BullNose) {
        t.corner_radius = 0.5;
        t.corner_radius_mm = 0.5;
    }
    t
}

fn suggest(
    op: OperationType,
    tool: &ToolConfig,
    material: &Material,
) -> Result<SuggestedParams, FeedsError> {
    let machine = MachineProfile::generic_wood_router();
    let stock = StockContext {
        stock_top_z: 0.0,
        stock_bottom_z: -18.0,
        stock_z: 18.0,
        stock_padding: 2.0,
    };
    suggest_params(SuggestParamsInput {
        op_type: op,
        tool,
        machine: &machine,
        material,
        lut: &EMBEDDED_LUT,
        stock_ctx: &stock,
        spindle_strategy: SpindleStrategy::default(),
        context: SuggestContext::default(),
    })
}

/// The reason of an `Unbacked` refusal; any other outcome fails the test.
fn refusal(result: Result<SuggestedParams, FeedsError>, cell: &str) -> String {
    match result {
        Err(FeedsError::Unbacked { reason, .. }) => reason.into_owned(),
        Err(other) => panic!("{cell}: expected Unbacked, got {other:?}"),
        Ok(p) => panic!(
            "{cell}: expected a refusal, got a recipe on {:?}",
            p.feeds_result.support
        ),
    }
}

fn drill_query(tool_family: ToolFamily, d: f64, z: u32, material: MaterialFamily) -> LookupQuery {
    LookupQuery {
        tool_family,
        tool_subfamily: None,
        diameter_mm: d,
        flute_count: z,
        material_family: material,
        hardness_kind: Some(HardnessKind::Janka),
        hardness_value: Some(1450.0),
        operation_family: LutOperationFamily::Drill,
        pass_role: LutPassRole::Roughing,
    }
}

/// (a) Each verified cell ships through the drill claim, on both drill
/// operations and in every material of its column.
#[test]
fn each_verified_cell_ships_the_side_chip_over_z_g6() {
    let mut checked = 0usize;
    for v in &VERIFIED {
        let printed_feed = v.ramp_down_ipm * 25.4;
        for (material_name, material) in materials_of(v) {
            for op in [OperationType::Drill, OperationType::AlignmentPinDrill] {
                let cell = format!("{op:?} {} mm {}F {material_name}", v.diameter_mm, v.flutes);
                let tool = tool_of(ToolType::EndMill, v.diameter_mm, v.flutes);
                let s = suggest(op, &tool, &material)
                    .unwrap_or_else(|e| panic!("{cell}: the G6 claim must serve it: {e}"));
                let r = &s.feeds_result;
                let FeedsSupport::DrillTransferred { drill, size } = &r.support else {
                    panic!("{cell}: expected DrillTransferred, got {:?}", r.support);
                };
                assert!(size.is_none(), "{cell}: the row is at the printed size");
                assert_eq!(drill.gap, Gap::Drill, "{cell}");
                assert_eq!(drill.rule, DRILL_RULE_TEXT, "{cell}");
                assert_eq!(drill.flutes, v.flutes, "{cell}");
                let row = r
                    .matched_lut_row
                    .as_ref()
                    .unwrap_or_else(|| panic!("{cell}: the recipe names its row"));
                assert_eq!(
                    row.observation_id,
                    side_row_id(material_name, v.diameter_mm, v.flutes),
                    "{cell}"
                );
                assert_eq!(drill.side_row, row.observation_id, "{cell}");
                // The chip: the side row / Z, against the verified chip.
                let chip = row
                    .chip_load_max_mm
                    .unwrap_or_else(|| panic!("{cell}: the claimed row prints a point"));
                assert!(
                    (chip / v.chip_mm - 1.0).abs() < CHIP_TOL,
                    "{cell}: chip {chip} is not the verified {} within 1 %",
                    v.chip_mm
                );
                assert_eq!(r.chipload_point_mm, Some(chip), "{cell}");
                // The RPM: the chart's 18 000 is replaced by the cap.
                assert_eq!(row.rpm_nominal, Some(18_000.0), "{cell}");
                assert!(
                    (r.rpm - RPM_CAP).abs() < 1e-9,
                    "{cell}: the drill RPM cap must bind, got {}",
                    r.rpm
                );
                // The feed: the chip is held per tooth at the cap, and the
                // feed rounds down on apply.
                let feed = s.operation.feed_rate();
                let held = RPM_CAP * chip * f64::from(v.flutes);
                assert!(
                    feed <= held + 1e-9 && feed >= held.floor() - 1.0,
                    "{cell}: feed {feed} is not 14 000 x {chip} x {} = {held}",
                    v.flutes
                );
                assert!(
                    feed < printed_feed,
                    "{cell}: feed {feed} must sit under the printed Ramp Down {printed_feed}"
                );
                assert!(
                    (s.operation.plunge_rate() - feed).abs() < 1.0,
                    "{cell}: a drill's plunge is its feed"
                );
                checked += 1;
            }
        }
    }
    // 4 Wood/Plywood cells x 3 materials + 4 MDF cells, on 2 operations.
    assert_eq!(checked, (4 * 3 + 4) * 2);
}

/// (b) The recipe resolver and the gate's envelope resolver read one row
/// for a claimed drill query, and the scaled point is still a point.
#[test]
fn the_gate_reads_the_same_scaled_point_g6() {
    let lut = embedded_vendor_lut();
    for v in &VERIFIED {
        let material = if v.mdf {
            MaterialFamily::Mdf
        } else {
            MaterialFamily::Hardwood
        };
        let q = drill_query(ToolFamily::FlatEnd, v.diameter_mm, v.flutes, material);
        let recipe = find_best_row_for_geometry(lut, &q, &ToolGeometryHint::Flat)
            .unwrap_or_else(|| panic!("{} mm {}F: no row", v.diameter_mm, v.flutes));
        let gate = find_best_chip_envelope_row(lut, &q, &ToolGeometryHint::Flat)
            .unwrap_or_else(|| panic!("{} mm {}F: no envelope row", v.diameter_mm, v.flutes));
        assert_eq!(recipe.observation_id, gate.observation_id);
        let claim = gate
            .drill_basis
            .claim()
            .unwrap_or_else(|| panic!("{}: the drill rule reads it", gate.observation_id));
        assert!((claim.scale - 1.0 / f64::from(v.flutes)).abs() < 1e-15);
        assert_eq!(gate.chip_load_min_mm, None, "a point carries no minimum");
        let PrintedChipload::Point { value_mm } = gate.printed_chipload() else {
            panic!("{}: a scaled point stays a point", gate.observation_id);
        };
        assert!(
            (value_mm / v.chip_mm - 1.0).abs() < CHIP_TOL,
            "{}: point {value_mm} against verified {}",
            gate.observation_id,
            v.chip_mm
        );
    }
}

/// (c) A flat plunge outside the claim refuses, and the text names G6.
#[test]
fn a_flat_plunge_outside_the_claim_refuses_with_g6() {
    for (d, z) in [(2.0, 2), (15.875, 2), (6.0, 1), (6.0, 4)] {
        let cell = format!("Drill {d} mm {z}F hardwood");
        let reason = refusal(
            suggest(
                OperationType::Drill,
                &tool_of(ToolType::EndMill, d, z),
                &hardwood(),
            ),
            &cell,
        );
        assert!(reason.contains("G6"), "{cell}: {reason}");
        assert!(!reason.contains("2.5"), "{cell}: {reason}");
    }
}

/// (d) Every other tool family refuses with G6. A bull query never reads a
/// Spektra flat row, although the lookup's tool fallback pairs bull and
/// flat.
#[test]
fn every_other_cutter_refuses_and_a_bull_never_reads_spektra_g6() {
    for kind in [
        ToolType::BallNose,
        ToolType::BullNose,
        ToolType::TaperedBallNose,
        ToolType::VBit,
    ] {
        // A tapered ball and a V-bit keep their default geometry (a 3.175
        // mm tip on a 6.35 mm shank; a 12.7 mm 90 deg cone): a 6 mm tip on a
        // 6 mm shank has no taper.
        let tool = match kind {
            ToolType::TaperedBallNose | ToolType::VBit => {
                let mut t = ToolConfig::new_default(ToolId(1), kind);
                t.flute_count = 2;
                t
            }
            _ => tool_of(kind, 6.0, 2),
        };
        for op in [OperationType::Drill, OperationType::AlignmentPinDrill] {
            let cell = format!("{op:?} {kind:?} {} mm hardwood", tool.diameter);
            let reason = refusal(suggest(op, &tool, &hardwood()), &cell);
            assert!(reason.contains("G6"), "{cell}: {reason}");
        }
    }
    let lut = embedded_vendor_lut();
    for material in [
        MaterialFamily::Softwood,
        MaterialFamily::Hardwood,
        MaterialFamily::PlywoodHardwood,
        MaterialFamily::Mdf,
    ] {
        for d in [3.175, 6.0] {
            let q = drill_query(ToolFamily::BullNose, d, 2, material);
            assert!(
                find_best_row_for_geometry(lut, &q, &ToolGeometryHint::Flat).is_none(),
                "a bull plunge must not read a row: {material:?} {d} mm"
            );
        }
    }
}

/// (e) Decision 3: a plunge in a material with no Spektra row refuses too.
#[test]
fn a_plunge_in_plastic_or_aluminium_refuses_g6() {
    let tool = tool_of(ToolType::EndMill, 6.0, 2);
    for (name, material) in [
        (
            "acrylic",
            Material::Plastic {
                family: PlasticFamily::Acrylic,
            },
        ),
        (
            "aluminium",
            Material::Aluminum {
                alloy: AluminumAlloy::Alloy6061T6,
            },
        ),
    ] {
        let reason = refusal(suggest(OperationType::Drill, &tool, &material), name);
        assert!(reason.contains("G6"), "{name}: {reason}");
    }
}

/// (f) The MDF 3.175 mm 2-flute cell reads Within on the plunge-feed gate:
/// 1778 / 3.175 = 560 mm/min per mm, under the 720 sheet-goods ceiling.
#[test]
fn the_mdf_eighth_inch_cell_is_within_the_plunge_envelope_g6() {
    let material = mdf();
    let tool = tool_of(ToolType::EndMill, 3.175, 2);
    let s = suggest(OperationType::Drill, &tool, &material).expect("the G6 claim serves it");
    let feed = s.operation.feed_rate();
    match classify_plunge_feed(feed, 3.175, &material) {
        DrillGateOutcome::Within {
            observed,
            envelope_hi,
            ..
        } => {
            assert!(
                (observed - feed / 3.175).abs() < 1e-9,
                "observed {observed}"
            );
            assert_eq!(envelope_hi, Some(720.0));
            assert!(
                observed < 720.0 && observed > 555.0,
                "about 560 per mm: {observed}"
            );
        }
        other => panic!("the MDF 3.175 mm plunge must read Within, got {other:?}"),
    }
}
