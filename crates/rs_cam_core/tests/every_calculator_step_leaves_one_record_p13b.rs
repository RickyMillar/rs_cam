//! Feeds contract P13b (K8, ruling Q7): every step of the feeds calculator
//! that moves a number leaves one step record, and the records reproduce
//! the shipped value.
//!
//! `feeds::calculator::calculate_with` notes the values after each step
//! call (`FeedsResult::steps`). On a grid of cells in the style of FM1
//! (every tool kind, five ops, three species, three machines) the claims
//! are:
//!
//! - The last record of each quantity holds the value of its result field
//!   (rpm, chip, feed, depth, width, plunge, power).
//! - Each record's `value_in` is the `value_out` of the record before it of
//!   the same quantity (the first is `None`), and `value_out` differs from
//!   `value_in`: a record is a move.
//! - A step leaves at most one record per quantity.
//! - `factor` is `value_out / value_in`.
//! - Non-vacuity: some cell has a record after the first one of its
//!   quantity (a later step moved a value).
//!
//! Red at HEAD 255d8db90: the test does not compile (`FeedsResult::steps`
//! and `trace::lineage::StepRecord` do not exist).

#![expect(
    clippy::panic,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::suggest::feeds_input_for_operation;
use rs_cam_core::feeds::{FeedsResult, SpindleStrategy, calculate, embedded_vendor_lut};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::trace::lineage::{StepQuantity, records_of};

fn tool(kind: ToolType, d: f64) -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), kind);
    t.diameter = d;
    t.set_flute_count(ToolFact::Stated(2));
    t.set_cutting_length(ToolFact::Stated((3.0 * d).max(12.0)));
    t.set_shank_diameter(ToolFact::Stated(d));
    t.shaft_diameter = d;
    t.set_stickout(ToolFact::Stated((3.0 * d).max(12.0) + 8.0));
    t
}

fn field(r: &FeedsResult, q: StepQuantity) -> Option<f64> {
    match q {
        StepQuantity::Rpm => Some(r.rpm),
        StepQuantity::Chip => Some(r.chip_load_mm),
        StepQuantity::Feed => Some(r.feed_rate_mm_min),
        StepQuantity::Depth => Some(r.axial_depth_mm),
        StepQuantity::Width => Some(r.radial_width_mm),
        StepQuantity::Plunge => Some(r.plunge_rate_mm_min),
        StepQuantity::Power => Some(r.power_kw),
        // The ramp needs the shipped feed: the apply funnel sets it.
        StepQuantity::Ramp => None,
    }
}

/// Check one result; return true when some quantity has more than one
/// record.
fn check(cell: &str, r: &FeedsResult) -> bool {
    let mut moved_later = false;
    for q in StepQuantity::ALL {
        let records: Vec<_> = records_of(&r.steps, q).collect();
        let Some(expected) = field(r, q) else {
            assert!(
                records.is_empty(),
                "{cell}: the calculator records no {q:?}"
            );
            continue;
        };
        let Some(last) = records.last() else {
            panic!("{cell}: no record of {q:?} (value {expected})");
        };
        assert_eq!(
            last.value_out.to_bits(),
            expected.to_bits(),
            "{cell}: the last {q:?} record ({}) is not the result ({expected})",
            last.value_out
        );
        let mut before: Option<f64> = None;
        let mut steps_seen: Vec<&str> = Vec::new();
        for rec in &records {
            assert_eq!(
                rec.value_in.map(f64::to_bits),
                before.map(f64::to_bits),
                "{cell}: {q:?} at {} does not start where the record before it ends",
                rec.step
            );
            assert_ne!(
                Some(rec.value_out.to_bits()),
                rec.value_in.map(f64::to_bits),
                "{cell}: {q:?} at {} is not a move",
                rec.step
            );
            assert!(
                !steps_seen.contains(&rec.step.as_str()),
                "{cell}: {} left two {q:?} records",
                rec.step
            );
            steps_seen.push(&rec.step);
            if let (Some(vin), Some(f)) = (rec.value_in, rec.factor)
                && vin != 0.0
            {
                assert!(
                    (f - rec.value_out / vin).abs() <= 1e-12 * f.abs().max(1.0),
                    "{cell}: {q:?} at {}: factor {f} is not out / in",
                    rec.step
                );
            }
            before = Some(rec.value_out);
        }
        moved_later |= records.len() > 1;
    }
    moved_later
}

#[test]
fn every_calculator_step_leaves_one_record_p13b() {
    let machines = [
        ("generic", MachineProfile::generic_wood_router()),
        ("shapeoko_vfd", MachineProfile::shapeoko_vfd()),
        ("shapeoko_makita", MachineProfile::shapeoko_makita()),
    ];
    let species = [
        WoodSpecies::GenericSoftwood,
        WoodSpecies::WhiteOak,
        WoodSpecies::Ipe,
    ];
    let ops = [
        OperationType::Pocket,
        OperationType::Profile,
        OperationType::Adaptive,
        OperationType::DropCutter,
        OperationType::Drill,
    ];
    let mut cells = 0;
    let mut moved_later = 0;
    for (machine_name, machine) in &machines {
        for sp in species {
            let material = Material::SolidWood { species: sp };
            for kind in ToolType::ALL {
                for d in [3.175, 6.0, 12.0] {
                    let t = tool(*kind, d);
                    for op_type in ops {
                        let op = rs_cam_core::compute::fixture_values::fixture_op(op_type);
                        let r = calculate(&feeds_input_for_operation(
                            &op,
                            &t,
                            &material,
                            machine,
                            embedded_vendor_lut(),
                            SpindleStrategy::MatchChart,
                        ));
                        let cell = format!("{machine_name} {sp:?} {kind:?} D{d} {op_type:?}");
                        cells += 1;
                        if check(&cell, &r) {
                            moved_later += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(cells > 0);
    assert!(
        moved_later > 0,
        "no cell has a step after the first of a quantity: the claim is vacuous"
    );
}
