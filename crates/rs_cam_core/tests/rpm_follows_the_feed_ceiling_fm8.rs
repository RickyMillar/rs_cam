//! FM8 — the RPM follows the feed ceiling down and the chipload holds.
//!
//! Ruling R4 Q10 (operator, 2026-09-24): when the machine cutting-feed
//! ceiling binds, `feeds::calculate` Step 7 lowers the RPM so the advance per
//! tooth stays at the band value, instead of the chip thinning at the same
//! RPM. The descent stops at the matched row's `rpm_min`, or at the
//! machine's minimum RPM when the row has none, and at the RPM where the cut
//! still fits the spindle power. What the descent cannot absorb is cut from
//! the feed at the ceiling, as before. The record is
//! `FeedsWarning::RpmLoweredForFeedCeiling` (diagnostic id
//! `feeds.rpm_lowered_for_ceiling`, Info) and, through Suggest, the
//! rationale row `SuggestWarning::RpmLoweredForFeedCeiling` on the RPM entry.
//!
//! The arms:
//!
//! - (a) a ceiling-bound banded cell: the RPM comes down, the chipload is
//!   the unbound chipload and inside the band, no `FeedRateClamped`;
//! - (b) a row `rpm_min` stops the descent: the chip thins at the ceiling,
//!   `FeedRateClamped` fires, and the warning says the row's `rpm_min`
//!   stopped it;
//! - (c) a cell the ceiling does not bind is unchanged;
//! - (d) through Suggest, on the Ø6 hardwood pocket (the printed Spektra row
//!   0.127 mm/tooth at 18 000 rpm, 2 flutes, raw feed 4572 mm/min) on the
//!   generic router's 4000 mm/min ceiling: the RPM is
//!   floor(4000 / 0.254) = 15 748, the feed 0.254 x 15 748 = 3999.992,
//!   shipped floored to 3999; the rationale row sits on the RPM entry, and
//!   the diagnostic is Info.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::diagnostics::adapters::from_feeds::diagnostics_from_feeds_result;
use rs_cam_core::diagnostics::{Severity, ids};
use rs_cam_core::feeds::rationale::{RationaleParam, SuggestRationale};
use rs_cam_core::feeds::suggest::{
    StockContext, SuggestContext, SuggestParamsInput, SuggestWarning, suggest_params,
};
use rs_cam_core::feeds::vendor_lut::VendorLut;
use rs_cam_core::feeds::{
    EMBEDDED_LUT, FeedsInput, FeedsResult, FeedsWarning, OperationFamily, PassRole, RpmFloorSource,
    SetupContext, SpindleScaleReason, SpindleStrategy, ToolGeometryHint, calculate,
    embedded_vendor_lut,
};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

const ROW_ID: &str = "amana-ball-hardwood-pocket-6350-2f-v7";

/// A machine whose cutting ceiling is `ceiling` and whose travel does not
/// bind below it.
fn machine_with_ceiling(ceiling: f64) -> MachineProfile {
    let mut m = MachineProfile::generic_wood_router();
    m.max_feed_mm_min = 20_000.0;
    m.max_cutting_feed_mm_min = Some(ceiling);
    m.power = rs_cam_core::machine::PowerModel::ConstantPower { power_kw: 5.0 };
    m
}

/// Ø6.35 ball 2F pocket rough in white oak on the printed Amana ball-nose v7
/// row (0.127-0.1778 mm/tooth, 18 000 rpm): a banded cell.
fn ball_pocket(machine: &MachineProfile, lut: &VendorLut) -> FeedsResult {
    let material = Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    };
    calculate(&FeedsInput {
        tool_diameter: 6.35,
        flute_count: 2,
        flute_length: 22.0,
        shank_diameter: None,
        tool_geometry: ToolGeometryHint::Ball,
        material: &material,
        machine,
        operation: OperationFamily::Pocket,
        operation_kind: None,
        pass_role: PassRole::Roughing,
        axial_depth_mm: None,
        radial_width_mm: None,
        target_scallop_mm: None,
        vendor_lut: Some(lut),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MatchChart,
    })
}

fn fpt(r: &FeedsResult) -> f64 {
    r.feed_rate_mm_min / (r.rpm * 2.0)
}

fn lowered(r: &FeedsResult) -> Option<(f64, f64, f64, f64, RpmFloorSource, bool)> {
    r.warnings.iter().find_map(|w| match w {
        FeedsWarning::RpmLoweredForFeedCeiling {
            rpm_from,
            rpm_to,
            feed_ceiling_mm_min,
            rpm_floor,
            floor_source,
            held,
        } => Some((
            *rpm_from,
            *rpm_to,
            *feed_ceiling_mm_min,
            *rpm_floor,
            *floor_source,
            *held,
        )),
        _ => None,
    })
}

fn clamped(r: &FeedsResult) -> bool {
    r.warnings
        .iter()
        .any(|w| matches!(w, FeedsWarning::FeedRateClamped { .. }))
}

/// The same cell with no ceiling in reach: the reference point.
fn unbound() -> FeedsResult {
    let r = ball_pocket(&machine_with_ceiling(20_000.0), embedded_vendor_lut());
    assert_eq!(
        r.matched_lut_row
            .as_ref()
            .map(|m| m.observation_id.as_str()),
        Some(ROW_ID),
        "precondition: the cell resolves the banded v7 row"
    );
    r
}

/// (c) the ceiling does not bind: nothing moves and nothing is recorded.
#[test]
fn an_unbound_cell_is_unchanged_fm8() {
    let r = unbound();
    assert!(lowered(&r).is_none(), "{:?}", r.warnings);
    assert!(!clamped(&r));
    assert_ne!(
        r.derates.spindle_scale_reason,
        SpindleScaleReason::FeedCeiling
    );
}

/// (a) a ceiling at 75 % of the unbound feed: the RPM comes down to
/// floor(0.75 x the unbound RPM) and the chip holds.
#[test]
fn a_bound_ceiling_lowers_the_rpm_and_holds_the_chip_fm8() {
    let base = unbound();
    let ceiling = 0.75 * base.feed_rate_mm_min;
    let machine = machine_with_ceiling(ceiling);
    let expected_rpm = (ceiling / (base.feed_rate_mm_min / base.rpm)).floor();
    assert!(
        expected_rpm >= machine.rpm_range().0,
        "precondition: the descent stays above the machine minimum ({expected_rpm})"
    );
    let r = ball_pocket(&machine, embedded_vendor_lut());

    let (from, to, c, _, source, held) = lowered(&r).unwrap_or_else(|| panic!("{:?}", r.warnings));
    assert_eq!(from, base.rpm);
    assert_eq!(
        to, expected_rpm,
        "the RPM is the whole rev/min under the held chip"
    );
    assert_eq!(r.rpm, expected_rpm);
    assert_eq!(c, ceiling);
    assert_eq!(
        source,
        RpmFloorSource::MachineMinimum,
        "the v7 row has no rpm_min"
    );
    assert!(held, "the chip must hold in full");
    assert!(
        !clamped(&r),
        "a held chip is not a feed clamp: {:?}",
        r.warnings
    );
    assert!(r.feed_rate_mm_min <= ceiling + 1e-9);
    assert!(
        (fpt(&r) - fpt(&base)).abs() <= fpt(&base) * 1e-9,
        "the advance per tooth moved: {} -> {}",
        fpt(&base),
        fpt(&r)
    );
    let band = r.chipload_bounds.expect("a banded cell");
    assert!(
        fpt(&r) >= band.min_mm_per_tooth && fpt(&r) <= band.max_mm_per_tooth,
        "the chip {} must sit in the band {band:?}",
        fpt(&r)
    );
    assert_eq!(r.derates.feed_clamp, 1.0);
    assert_eq!(
        r.derates.spindle_scale_reason,
        SpindleScaleReason::FeedCeiling
    );

    let finding = diagnostics_from_feeds_result(ToolpathId(0), &r)
        .into_iter()
        .find(|d| d.id.as_str() == ids::FEEDS_RPM_LOWERED_FOR_CEILING)
        .expect("the finding");
    assert_eq!(finding.severity, Severity::Info);
    assert!(
        finding.message.starts_with(&format!(
            "RPM lowered {:.0} -> {:.0} to hold the chip at the",
            from, to
        )),
        "{}",
        finding.message
    );
    assert!(ids::ALL.contains(&ids::FEEDS_RPM_LOWERED_FOR_CEILING));
}

/// (b) a row `rpm_min` at 90 % of the chart RPM stops a descent that wants
/// 75 %: the RPM stops at the row minimum, the feed sits on the ceiling with
/// a thinner chip, and the warning names the row's `rpm_min`.
#[test]
fn a_row_rpm_min_stops_the_descent_fm8() {
    let base = unbound();
    let mut row = embedded_vendor_lut()
        .observations
        .iter()
        .find(|o| o.observation_id == ROW_ID)
        .expect("the v7 row exists")
        .clone();
    let rpm_min = (0.9 * base.rpm).round();
    row.rpm_min = Some(rpm_min);
    let lut = VendorLut {
        observations: vec![row],
    };
    let ceiling = 0.75 * base.feed_rate_mm_min;
    let r = ball_pocket(&machine_with_ceiling(ceiling), &lut);

    let (_, to, _, floor, source, held) = lowered(&r).unwrap_or_else(|| panic!("{:?}", r.warnings));
    assert_eq!(source, RpmFloorSource::RowRpmMin);
    assert_eq!(floor, rpm_min);
    assert_eq!(to, rpm_min, "the descent stops on the row minimum");
    assert!(!held);
    assert!(clamped(&r), "the rest is cut from the feed at the ceiling");
    assert!((r.feed_rate_mm_min - ceiling).abs() < 1e-9);
    assert!(fpt(&r) < fpt(&base), "the chip thins below the row minimum");
    let text = diagnostics_from_feeds_result(ToolpathId(0), &r)
        .into_iter()
        .find(|d| d.id.as_str() == ids::FEEDS_RPM_LOWERED_FOR_CEILING)
        .expect("the finding")
        .message;
    assert!(text.contains("the vendor row's rpm_min"), "{text}");
}

/// (d) through Suggest on the generic router's 4000 mm/min ceiling.
#[test]
fn suggest_ships_the_lowered_rpm_with_a_rationale_row_fm8() {
    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.shank_diameter = 6.0;
    tool.shaft_diameter = 6.0;
    tool.cutting_length = 22.0;
    tool.stickout = 20.0;
    tool.flute_count = 2;
    let machine = MachineProfile::generic_wood_router();
    let material = Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    };
    let stock = StockContext {
        stock_top_z: 0.0,
        stock_bottom_z: -18.0,
        stock_z: 18.0,
        stock_padding: 2.0,
    };
    let s = suggest_params(SuggestParamsInput {
        op_type: OperationType::Pocket,
        tool: &tool,
        machine: &machine,
        material: &material,
        lut: &EMBEDDED_LUT,
        stock_ctx: &stock,
        spindle_strategy: SpindleStrategy::MatchChart,
        context: SuggestContext::default(),
    })
    .expect("a Ø6 end mill pocket in hardwood ships");

    let (from, to) = s
        .warnings
        .iter()
        .find_map(|w| match w {
            SuggestWarning::RpmLoweredForFeedCeiling {
                rpm_from, rpm_to, ..
            } => Some((*rpm_from, *rpm_to)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{:?}", s.warnings));
    assert_eq!(from, 18_000.0);
    assert_eq!(to, 15_748.0, "floor(4000 / 0.254)");
    assert_eq!(s.operation.spindle_rpm(), Some(15_748));
    assert_eq!(
        s.operation.feed_rate(),
        3999.0,
        "0.254 x 15 748 = 3999.992, floored"
    );

    let rows = SuggestRationale::from_warnings(&s.warnings).entries;
    assert!(
        rows.iter()
            .any(|e| e.param == RationaleParam::Rpm && e.to_value == Some(15_748.0)),
        "the rationale row sits on the RPM entry: {rows:?}"
    );
}
