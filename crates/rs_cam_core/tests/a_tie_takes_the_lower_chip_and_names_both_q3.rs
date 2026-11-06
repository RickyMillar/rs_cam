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
//!
//! Feeds contract P16 item 8: `LookupResult::tied_with` is now the vendor
//! band (`LookupResult::band`). The Q3 order still picks the score winner.
//! The downcut series 60-200 (`onsrud_ocr.json`) prints 1/4 in (6.35 mm)
//! and up, so the 6 mm query is below the chart: the G1 claim extrapolates
//! (form B or C), and such a row is not a band member (band rule 4). The
//! compression series 60-100MW prints 3/16 in (4.7625 mm) and 1/4 in, which
//! bracket 6 mm: a form A member. Its chip (0.3704 mm in the FM1 walk of
//! 2026-11-06) is above the winner's chip (0.1492 mm), so the no-rise rule
//! ships the winner (`BandRule::WinnerShips`) and the band names the
//! compression row.

#![expect(
    clippy::expect_used,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::ToolFact;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::suggest::feeds_result_for_operation;
use rs_cam_core::feeds::vendor_lookup::{BandExclusion, BandRule};
use rs_cam_core::feeds::{SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

const COMPRESSION_ROW: &str = "onsrud-hardwood-60-100mw-1_4-finish";
const DOWNCUT_ROW: &str = "onsrud-hardwood-60-200-1_4-finish";

#[test]
fn a_tie_takes_the_lower_chip_and_names_both_rows_q3() {
    let mut tool = ToolConfig::test_default(ToolId::from_raw(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.set_flute_count(ToolFact::Stated(2));
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
    let band = &row.band;
    assert_eq!(
        band.score_winner.id(),
        DOWNCUT_ROW,
        "the Q3 order picks the winner"
    );
    assert_eq!(
        band.rule,
        BandRule::WinnerShips,
        "every member is above the winner's chip: the winner ships"
    );
    assert_eq!(
        band.winner_outside,
        Some(BandExclusion::Size),
        "the downcut row is extrapolated, so it is not a member"
    );
    let ids: Vec<&str> = band.members.iter().map(|m| m.row.id()).collect();
    assert!(
        ids.contains(&COMPRESSION_ROW),
        "the band does not name the compression row: {ids:?}"
    );
    assert!(
        band.members
            .iter()
            .all(|m| m.chip_mid_mm > row.chip_load_mm),
        "a member at or below the winner's chip must ship"
    );
}
