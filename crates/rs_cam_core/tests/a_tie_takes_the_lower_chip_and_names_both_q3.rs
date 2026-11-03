//! Ruling Q3 (feeds contract P5, lead decision D2): a score tie between
//! vendor rows takes the lower printed chip, and the result names both
//! rows.
//!
//! The case is op 5 of the blind review (report 02 (a),
//! `planning/feeds_blind_review_2026-11-03/rootcause/02_op5_op18_slot_profile.md`):
//! a 6 mm 2-flute flat end mill in hardwood on a Profile (Contour,
//! Roughing). The two 6.35 mm Onsrud Contour rows tie on the score, the
//! diameter, the transfer and the series (both specialty). Until P5 the id
//! decided, and the compression row 60-100MW (0.3556-0.4064 mm/tooth) won
//! over the downcut row 60-200 (0.127-0.1778) on the string order only.
//!
//! The order is: the series rule, then the lower chip, then the id (D2).
//!
//! Red before P5 (shown in the P5 report).

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::feeds_result_for_operation;
use rs_cam_core::feeds::{SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

const COMPRESSION_ROW: &str = "onsrud-hardwood-60-100mw-1_4-finish";
const DOWNCUT_ROW: &str = "onsrud-hardwood-60-200-1_4-finish";

#[test]
fn a_tie_takes_the_lower_chip_and_names_both_rows_q3() {
    let mut tool = ToolConfig::new_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.flute_count = 2;
    let op = rs_cam_core::compute::fixture_values::fixture_op(OperationType::Profile);
    let result = feeds_result_for_operation(
        &op,
        &tool,
        &Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        &MachineProfile::default(),
        embedded_vendor_lut(),
        SpindleStrategy::default(),
    )
    .expect("a 6 mm end mill on a Profile is backed");
    let row = result.matched_lut_row.expect("a vendor row matches");
    assert_eq!(
        row.observation_id, DOWNCUT_ROW,
        "the lower printed chip does not win the tie"
    );
    assert_eq!(
        row.tied_with
            .as_ref()
            .map(rs_cam_core::feeds::sourced::RowRef::id),
        Some(COMPRESSION_ROW),
        "the result does not name the row that tied"
    );
}
