//! Feeds contract P23c (`planning/feeds_contract_2026-11-03/phases/P23c.md`;
//! operator rulings 2026-11-10): in the vendor band, a row printed at the
//! tool's size (`SizeBasis::Exact`) outranks a member whose value is a size
//! claim (a form A interpolation). Among rows of one class the lowest chip
//! ships, as before. A score winner that is not a member is untouched: no
//! member above it ships (the no-rise rule).
//!
//! The SpeTool 2D/3D tapered chart serves the 3D finish families (a G3
//! family rule, the same witness as the Amana ZrN v8 rule), so its row at
//! the tool's tip size is a member of a Scallop band.
//!
//! The arms:
//!
//! - (a) the wanaka idx 16 cut (Scallop, tapered ball, tip 2.0 mm, 2
//!   flutes, white oak): the band holds the SpeTool row (exact, 0.003 in =
//!   0.0762 mm) and the Amana ZrN interpolation (claim, 0.06437); the exact
//!   row ships (`BandRule::ExactSize`), and the feed at 24000 RPM is
//!   `floor(0.0762 x 2 x 24000)` = 3657;
//! - (b) a band of exact rows only ships the lowest (`BandRule::Lowest`);
//! - (c) a band where the exact row is already the lowest ships it by
//!   `Lowest`;
//! - (d) a score winner outside the band still ships when the exact member
//!   is above it (`BandRule::WinnerShips`).
//!
//! Red: return the lowest member whatever its class in
//! `vendor_lookup::with_band`.

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

mod common;

use common::feeds_cells::{add_time_op, stock_ctx, tool_of};
use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolType};
use rs_cam_core::feeds::extrapolation::SizeBasis;
use rs_cam_core::feeds::suggest::{
    NoProject, SuggestContext, SuggestForOperationInput, suggest_for_operation,
};
use rs_cam_core::feeds::vendor_lookup::{BandMember, BandRule};
use rs_cam_core::feeds::{FeedsResult, SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

fn is_exact(member: &BandMember) -> bool {
    match &member.size_basis {
        SizeBasis::Exact => true,
        SizeBasis::Claim(_)
        | SizeBasis::NoDiameterAnchor
        | SizeBasis::NoChipload
        | SizeBasis::AngleKey { .. }
        | SizeBasis::Refused { .. } => false,
    }
}

fn run(
    op: &OperationConfig,
    tool: &ToolConfig,
    machine: &MachineProfile,
    species: WoodSpecies,
    strategy: SpindleStrategy,
) -> FeedsResult {
    suggest_for_operation(SuggestForOperationInput {
        operation: op,
        tool,
        machine,
        material: &Material::SolidWood { species },
        lut: embedded_vendor_lut(),
        spindle_strategy: strategy,
        context: SuggestContext::no_project(NoProject::NONE),
    })
    .unwrap_or_else(|e| panic!("Suggest serves {:?}: {e}", op.op_type()))
    .feeds_result
}

/// An FM1 cell on the default machine under `MatchChart`.
fn fm1_cell(op: OperationType, kind: ToolType, diameter: f64, species: WoodSpecies) -> FeedsResult {
    run(
        &add_time_op(op, &stock_ctx()),
        &tool_of(kind, diameter, 2),
        &MachineProfile::default(),
        species,
        SpindleStrategy::MatchChart,
    )
}

/// (a) The wanaka idx 16 cut: the exact SpeTool row outranks the lower
/// Amana interpolation.
#[test]
fn the_exact_spetool_row_outranks_the_lower_amana_interpolation_p23c() {
    // The machine values of
    // `planning/rivmap_block_job/nz_wanaka_21_operator.toml`.
    let machine = MachineProfile {
        max_feed_mm_min: 15_000.0,
        ..MachineProfile::shapeoko_vfd()
    }
    .with_axis_limits([15_000.0, 15_000.0, 2500.0], [1200.0, 1200.0, 550.0]);
    let r = run(
        &add_time_op(OperationType::Scallop, &stock_ctx()),
        &tool_of(ToolType::TaperedBallNose, 2.0, 2),
        &machine,
        WoodSpecies::WhiteOak,
        SpindleStrategy::ConstantChip {
            rpm_ceiling: Some(24_000),
        },
    );
    let band = r.vendor_band().expect("the cell has a band");
    let exact: Vec<&BandMember> = band.members.iter().filter(|m| is_exact(m)).collect();
    let claims: Vec<&BandMember> = band.members.iter().filter(|m| !is_exact(m)).collect();
    assert_eq!(exact.len(), 1, "one exact member: {:?}", band.members);
    assert_eq!(exact[0].row.0, "spetool-tapered-hardwood-parallel-2000-2f");
    // SpeTool prints 0.003 in for the 2.0 mm tip.
    assert!((exact[0].chip_mid_mm - 0.003 * 25.4).abs() < 1e-12);
    let lower_claim = claims
        .iter()
        .find(|m| m.chip_mid_mm < exact[0].chip_mid_mm)
        .unwrap_or_else(|| panic!("non-vacuity: a lower claim member: {:?}", band.members));
    assert!(
        lower_claim.row.0.contains("zrn-v8"),
        "{}",
        lower_claim.row.0
    );

    assert_eq!(band.rule, BandRule::ExactSize);
    assert_eq!(
        r.vendor_source.as_deref(),
        Some("spetool-tapered-hardwood-parallel-2000-2f")
    );
    assert!(
        (r.chip_load_mm - 0.0762).abs() < 1e-12,
        "{}",
        r.chip_load_mm
    );
    assert_eq!(r.rpm, 24_000.0);
    // No lift (a finish pass; ruling P4a): feed = chip x Z x RPM.
    assert_eq!(r.derates.radial_chip_thinning, 1.0);
    assert_eq!(r.feed_rate_mm_min.floor(), 3657.0, "{}", r.feed_rate_mm_min);
}

/// (b) and (c): rows of one class keep the lowest.
#[test]
fn rows_of_one_class_keep_the_lowest_p23c() {
    // (b) Two printed rows at 3.175 mm (Amana Spektra, Freud).
    let r = fm1_cell(
        OperationType::Pocket,
        ToolType::EndMill,
        3.175,
        WoodSpecies::GenericSoftwood,
    );
    let band = r.vendor_band().expect("the cell has a band");
    assert!(
        band.members.len() >= 2 && band.members.iter().all(is_exact),
        "non-vacuity: two exact members: {:?}",
        band.members
    );
    assert_eq!(band.rule, BandRule::Lowest);
    assert_eq!(
        r.vendor_source.as_deref(),
        Some(band.members[0].row.0.as_str())
    );
    assert!(band.members[0].chip_mid_mm < band.members[1].chip_mid_mm);

    // (c) An exact row below a claim: the exact row is the lowest.
    let r = fm1_cell(
        OperationType::Pocket,
        ToolType::EndMill,
        6.0,
        WoodSpecies::WhiteOak,
    );
    let band = r.vendor_band().expect("the cell has a band");
    assert!(
        is_exact(&band.members[0]) && band.members.iter().any(|m| !is_exact(m)),
        "non-vacuity: an exact member below a claim: {:?}",
        band.members
    );
    assert_eq!(band.rule, BandRule::Lowest);
    assert_eq!(
        r.vendor_source.as_deref(),
        Some(band.members[0].row.0.as_str())
    );
}

/// (d) A score winner outside the band is untouched: an exact member above
/// it does not ship.
#[test]
fn a_winner_outside_the_band_still_ships_under_an_exact_member_p23c() {
    let r = fm1_cell(
        OperationType::Profile,
        ToolType::EndMill,
        3.175,
        WoodSpecies::WhiteOak,
    );
    let band = r.vendor_band().expect("the cell has a band");
    assert!(band.winner_outside.is_some(), "the winner is not a member");
    let winner_chip = band.score_winner_chip_mm.expect("the winner has a chip");
    assert!(
        band.members
            .iter()
            .any(|m| is_exact(m) && m.chip_mid_mm > winner_chip),
        "non-vacuity: an exact member above the winner: {:?}",
        band.members
    );
    assert_eq!(band.rule, BandRule::WinnerShips);
    assert_eq!(
        r.vendor_source.as_deref(),
        Some(band.score_winner.0.as_str())
    );
    assert!((r.chip_load_mm - winner_chip).abs() < 1e-12);
}
