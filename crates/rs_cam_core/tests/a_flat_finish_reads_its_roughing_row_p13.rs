//! Feeds contract P13 item 4 (operator ruling D13 item 4, lead rulings
//! 2026-11-05): law B, the anchored series law.
//!
//! A flat end mill on Trace, Ramp, Radial and Horizontal finishing has no
//! vendor row of its own family. Until P13 these cells took the Shapeoko
//! chip formula at the Shapeoko surface speed (FM1 6 mm hardwood Trace:
//! 973 mm/min against the 4 572 of the roughing row; derate audit M1). The
//! step 0 study (`planning/feeds_contract_2026-11-03/research_p13_items45/`)
//! found that a pooled law over the vendor rows misses a held-out vendor by
//! about 2x, and that the chart series is the missing term. So the cell reads
//! the tool's own (Pocket, Roughing) row at x1.00, through the same lookup
//! and the same speeds chain as the Pocket cell of that tool (no finish
//! relation; the thinning rules apply as shipped).
//!
//! Where the tool has no roughing row, the formula chip stays and a warning
//! names it (`FeedsWarning::FormulaChipNoPrintedRow`; D7: warn, Q6: never
//! refuse).
//!
//! Red before P13 items 4-5: the four finishing cells read the formula
//! (`FormulaFallback`), and no warning names the formula chip.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::FeedsInput;
use rs_cam_core::feeds::vendor_lut::VendorLut;
use rs_cam_core::feeds::{
    ChiploadSource, FeedsResult, FeedsWarning, SpindleStrategy, calculate, embedded_vendor_lut,
};
use rs_cam_core::machine::{MachineProfile, PowerModel};
use rs_cam_core::material::{Material, WoodSpecies};

const D_MM: f64 = 6.0;

/// The four flat-end finishing ops of law B.
const LAW_B_OPS: [OperationType; 4] = [
    OperationType::Trace,
    OperationType::RampFinish,
    OperationType::RadialFinish,
    OperationType::HorizontalFinish,
];

fn tool(kind: ToolType) -> ToolConfig {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(3), kind);
    tool.diameter = D_MM;
    tool.set_cutting_length(ToolFact::Stated(25.0));
    tool.set_stickout(ToolFact::Stated(45.0));
    tool.set_flute_count(ToolFact::Stated(2));
    tool
}

/// No power, feed or rpm ceiling binds, so the rpm is the row's own rpm.
fn open_machine() -> MachineProfile {
    MachineProfile {
        power: PowerModel::ConstantPower { power_kw: 50.0 },
        max_feed_mm_min: 1.0e7,
        max_cutting_feed_mm_min: Some(1.0e7),
        ..MachineProfile::default()
    }
}

fn result_of(op: &OperationConfig, tool: &ToolConfig, lut: &VendorLut) -> FeedsResult {
    let material = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    let machine = open_machine();
    let input = FeedsInput::of_operation(
        op,
        tool,
        &material,
        &machine,
        lut,
        SpindleStrategy::MatchChart,
    );
    calculate(&input)
}

fn row_id(r: &FeedsResult) -> Option<String> {
    r.matched_lut_row
        .as_ref()
        .map(|row| row.observation_id.clone())
}

fn names_the_formula_chip(r: &FeedsResult) -> bool {
    format!("{:?}", r.warnings).contains("FormulaChipNoPrintedRow")
}

#[test]
fn a_flat_finish_reads_the_roughing_row_of_its_tool_p13() {
    let em = tool(ToolType::EndMill);
    let lut = embedded_vendor_lut();
    let pocket = result_of(
        &rs_cam_core::compute::fixture_values::fixture_op(OperationType::Pocket),
        &em,
        lut,
    );
    let anchor = pocket
        .matched_lut_row
        .as_ref()
        .expect("fixture: the 6 mm hardwood Pocket reads a vendor row");
    let printed = anchor
        .printed_chipload()
        .point_mm()
        .expect("fixture: the anchor prints one chip");
    let row_rpm = anchor
        .rpm_nominal
        .expect("fixture: the anchor prints an rpm");
    assert!(
        (pocket.chip_load_mm - printed).abs() < 1e-12,
        "fixture: the Pocket chip is the printed point"
    );
    for kind in LAW_B_OPS {
        let op = rs_cam_core::compute::fixture_values::fixture_op(kind);
        let r = result_of(&op, &em, lut);
        assert_eq!(
            row_id(&r).as_deref(),
            Some(anchor.observation_id.as_str()),
            "{kind:?}: law B reads the tool's own (Pocket, Roughing) row"
        );
        assert!(
            matches!(r.chipload_source, ChiploadSource::VendorLut { .. }),
            "{kind:?}: the chip comes from the row, not the formula: {:?}",
            r.chipload_source
        );
        assert!(
            (r.chip_load_mm - pocket.chip_load_mm).abs() < 1e-12,
            "{kind:?}: x1.00 of the roughing row ({}), got {}",
            pocket.chip_load_mm,
            r.chip_load_mm
        );
        assert!(
            (r.rpm - row_rpm).abs() < 1e-9,
            "{kind:?}: the rpm is the row's rpm {row_rpm} (MatchChart), not the surface \
             speed rpm; got {}",
            r.rpm
        );
        assert!(!names_the_formula_chip(&r), "{kind:?}: a row matched");
    }
}

#[test]
fn a_flat_finish_with_no_printed_row_keeps_the_formula_and_warns_p13() {
    let empty = VendorLut {
        observations: Vec::new(),
    };
    let em = tool(ToolType::EndMill);
    for kind in LAW_B_OPS {
        let op = rs_cam_core::compute::fixture_values::fixture_op(kind);
        let r = result_of(&op, &em, &empty);
        assert!(
            matches!(r.chipload_source, ChiploadSource::FormulaFallback),
            "{kind:?}: no row, so the formula chip"
        );
        assert!(
            names_the_formula_chip(&r),
            "{kind:?}: the warning names the formula chip: {:?}",
            r.warnings
        );
        let w = r
            .warnings
            .iter()
            .find_map(|w| {
                if let FeedsWarning::FormulaChipNoPrintedRow {
                    formula_chipload_mm,
                } = w
                {
                    Some(*formula_chipload_mm)
                } else {
                    None
                }
            })
            .expect("the variant");
        assert!(
            (w - r.chip_load_mm).abs() < 1e-12,
            "{kind:?}: it states the chip"
        );
    }
    // The partner: a ball nose is not on law B; its formula cells are
    // unchanged and carry no law B warning.
    let ball = tool(ToolType::BallNose);
    let r = result_of(
        &rs_cam_core::compute::fixture_values::fixture_op(OperationType::Trace),
        &ball,
        &empty,
    );
    assert!(
        !names_the_formula_chip(&r),
        "a ball nose is out of scope (B3)"
    );
}
