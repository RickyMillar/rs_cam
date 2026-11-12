//! P20b (feeds contract, `planning/feeds_contract_2026-11-03/phases/P20.md`
//! audit row 18 and section 3): one vertical-rate limit,
//! `feeds::vertical::VerticalRateLimit`, bounds every plunge, every drill
//! feed and the vertical part of every G6 ramp. It holds two limits:
//!
//! - the G6 axial chip x rpm x Z, where a G6 claim covers the tool. Until
//!   P20b only a Project Curve read it (lead decision Q8a); a Profile with
//!   the same tool shipped the G10 rule, feed / 2, above the G6 vertical
//!   rate (wanaka, 6 mm 2F flat in white oak: 1582 against 1347 mm/min);
//! - the machine Z axis rate (`MachineKinematics::max_rate_xyz_mm_min[2]`,
//!   GRBL `$112`). Until P20b no feeds producer read it.
//!
//! The arms:
//!
//! - (a) a 6 mm 2F drill in MDF at the chart 18 000 rpm on a machine with Z
//!   2500 mm/min (the wanaka machine's `$112`, P20.md section 0): the feed
//!   0.0762 x 18 000 x 2 = 2743.2 is capped at 2500, and the RPM follows
//!   down to 18 000 x 2500 / 2743.2 to hold the chip;
//! - (b) a Profile plunge is `min(feed / 2, axial chip x rpm x Z, Z axis
//!   rate)`: the axial chip binds on the Z 2500 machine; on a machine whose
//!   Z rate is half the G6 vertical rate, the Z axis rate binds;
//! - (c) a 60° ramp whose vertical part is above the Z axis rate ships the
//!   Z axis rate / tan 60°;
//! - (d) a machine with no kinematics has no Z cap, and the card says so.
//!
//! Every expected value comes from the inputs or from a run of the same
//! cell on a machine with no kinematics.
//!
//! Red: remove the `ZAxisRate` limit from `VerticalRateLimit::bound`, or the
//! axial chip from `VerticalRateLimit::new`.

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a panic is how a test reports a failure"
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{DressupConfig, DressupEntryStyle};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolFact, ToolId, ToolType};
use rs_cam_core::feeds::plunge::{PlungeBinding, resolve_plunge_at};
use rs_cam_core::feeds::ramp::{RampArm, RampBasis};
use rs_cam_core::feeds::suggest::{
    NoProject, SuggestContext, SuggestForOperationInput, SuggestWarning, SuggestedParams,
    suggest_for_operation,
};
use rs_cam_core::feeds::vertical::ZAxisRate;
use rs_cam_core::feeds::{FeedsWarning, RampFeed, SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::machine::kinematics::MachineKinematics;
use rs_cam_core::material::{Material, SheetGoodKind, WoodSpecies};

/// The wanaka machine's Z axis rate, `$112` (P20.md section 0).
const WANAKA_Z_MM_MIN: f64 = 2500.0;

fn flat_6_2f() -> ToolConfig {
    let mut t = ToolConfig::test_default(ToolId::from_raw(1), ToolType::EndMill);
    t.diameter = 6.0;
    t.set_flute_count(ToolFact::Stated(2));
    t.set_cutting_length(ToolFact::Stated(24.0));
    t.set_shank_diameter(ToolFact::Stated(6.0));
    t.shaft_diameter = 6.0;
    t.set_stickout(ToolFact::Stated(32.0));
    t
}

fn mdf() -> Material {
    Material::SheetGood {
        kind: SheetGoodKind::Mdf,
    }
}

fn white_oak() -> Material {
    Material::SolidWood {
        species: WoodSpecies::WhiteOak,
    }
}

fn softwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    }
}

/// The generic router with no kinematics: no Z axis rate.
fn no_z() -> MachineProfile {
    let m = MachineProfile::generic_wood_router();
    assert!(m.kinematics.is_none(), "the fixture states no kinematics");
    m
}

/// The generic router with a stated Z axis rate. X and Y take the travel
/// rate, so the cutting ceiling is the same as on [`no_z`].
fn with_z(z_mm_min: f64) -> MachineProfile {
    let mut m = MachineProfile::generic_wood_router();
    let xy = m.max_feed_mm_min;
    let mut k = MachineKinematics::generic_wood_router();
    k.max_rate_xyz_mm_min = Some([xy, xy, z_mm_min]);
    m.kinematics = Some(k);
    assert_eq!(
        m.cutting_feed_ceiling_mm_min(),
        no_z().cutting_feed_ceiling_mm_min()
    );
    m
}

fn suggest(
    op: &OperationConfig,
    material: &Material,
    machine: &MachineProfile,
    dressups: Option<&DressupConfig>,
) -> SuggestedParams {
    suggest_for_operation(SuggestForOperationInput {
        operation: op,
        tool: &flat_6_2f(),
        machine,
        material,
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::MatchChart,
        context: SuggestContext::no_project(NoProject {
            dressups,
            ..NoProject::NONE
        }),
    })
    .unwrap_or_else(|e| panic!("Suggest serves {:?}: {e}", op.op_type()))
}

/// The G6 axial chip (mm/tooth) and Z of a result.
fn g6_chip(s: &SuggestedParams) -> (f64, u32) {
    match &s.feeds_result.ramp {
        RampBasis::Sourced {
            axial_chip_mm,
            drill,
            ..
        } => (*axial_chip_mm, drill.flutes),
        RampBasis::NoChip { reason } => panic!("a G6 claim covers the 6 mm 2F flat: {reason:?}"),
    }
}

fn shipped_rpm(s: &SuggestedParams) -> f64 {
    f64::from(s.operation.spindle_rpm().expect("Suggest writes the RPM"))
}

/// (a) The drill feed is capped at the Z axis rate, and the RPM follows
/// down to hold the chip.
#[test]
fn a_drill_feed_above_the_z_rate_ships_the_z_rate_and_the_rpm_follows_p20b() {
    let drill = OperationConfig::new_default(OperationType::Drill);
    let free = suggest(&drill, &mdf(), &no_z(), None);
    let capped = suggest(&drill, &mdf(), &with_z(WANAKA_Z_MM_MIN), None);

    // The free run: the chart chip at the chart RPM, 0.0762 x 18 000 x 2.
    let free_feed = free.feeds_result.feed_rate_mm_min;
    let free_rpm = free.feeds_result.rpm;
    let free_chip = free_feed / (free_rpm * 2.0);
    assert!(
        free_feed > WANAKA_Z_MM_MIN,
        "non-vacuity: the drill feed {free_feed} is above the Z rate"
    );

    // The capped run: the feed is the Z rate, the plunge is the feed.
    let r = &capped.feeds_result;
    assert_eq!(r.feed_rate_mm_min, WANAKA_Z_MM_MIN);
    assert_eq!(r.plunge_rate_mm_min, WANAKA_Z_MM_MIN);
    let want_rpm = no_z().clamp_rpm(free_rpm * WANAKA_Z_MM_MIN / free_feed);
    assert!(
        (r.rpm - want_rpm).abs() < 1e-9,
        "the RPM follows down: {} vs {want_rpm}",
        r.rpm
    );
    assert!(
        (r.feed_rate_mm_min / (r.rpm * 2.0) - free_chip).abs() < 1e-12,
        "the chip holds"
    );
    let record = r
        .warnings
        .iter()
        .find_map(|w| {
            if let FeedsWarning::DrillFeedAtZAxisRate {
                requested,
                z_rate_mm_min,
                rpm_from,
                rpm_to,
            } = w
            {
                Some((*requested, *z_rate_mm_min, *rpm_from, *rpm_to))
            } else {
                None
            }
        })
        .expect("the card names the Z axis rate");
    assert_eq!(record, (free_feed, WANAKA_Z_MM_MIN, free_rpm, r.rpm));

    // The apply writes the capped feed and the lower RPM.
    assert_eq!(capped.operation.feed_rate(), Some(WANAKA_Z_MM_MIN));
    assert!(
        (shipped_rpm(&capped) - want_rpm).abs() <= 1.0,
        "the shipped RPM {} is the RPM that holds the chip {want_rpm}",
        shipped_rpm(&capped)
    );
}

/// (b) A Profile plunge: the axial chip binds where the Z rate is above
/// it; the Z axis rate binds where it is below.
#[test]
fn a_profile_plunge_is_the_smallest_vertical_limit_p20b() {
    let profile = OperationConfig::new_default(OperationType::Profile);
    let s = suggest(&profile, &white_oak(), &with_z(WANAKA_Z_MM_MIN), None);
    let feed = s.operation.feed_rate().expect("Suggest writes the feed");
    let rpm = shipped_rpm(&s);
    let plunge = s
        .operation
        .plunge_rate()
        .expect("Suggest writes the plunge");
    let fraction = s
        .feeds_result
        .plunge
        .claim()
        .expect("the G10 flat claim covers the 6 mm 2F flat")
        .fraction;
    let (chip, z) = g6_chip(&s);
    let unlifted = s.feeds_result.unlifted_feed();
    let rule = fraction * unlifted.mm_min();
    let axial = chip * rpm * f64::from(z);
    assert!(
        axial < rule && axial < WANAKA_Z_MM_MIN,
        "non-vacuity: the axial chip binds ({axial} vs rule {rule}, Z {WANAKA_Z_MM_MIN})"
    );
    let want = rule.min(axial).min(WANAKA_Z_MM_MIN).min(feed).floor();
    assert_eq!(plunge, want, "feed {feed}, rpm {rpm}");
    assert_eq!(
        resolve_plunge_at(&s.feeds_result.plunge, unlifted, rpm),
        Some((want, PlungeBinding::AxialChip))
    );

    // A Z axis rate at half the G6 vertical rate binds instead.
    let low_z = axial / 2.0;
    let s = suggest(&profile, &white_oak(), &with_z(low_z), None);
    let rpm = shipped_rpm(&s);
    let unlifted = s.feeds_result.unlifted_feed();
    assert_eq!(
        resolve_plunge_at(&s.feeds_result.plunge, unlifted, rpm),
        Some((low_z.floor(), PlungeBinding::ZAxisRate))
    );
    assert_eq!(s.operation.plunge_rate(), Some(low_z.floor()));
    let (headline, detail) = s.feeds_result.plunge.card_text_at(unlifted, rpm);
    assert!(
        headline.contains("set by the machine Z axis rate"),
        "{headline}"
    );
    assert!(detail.contains("$112"), "{detail}");
}

/// The one `RampFeed` record of an apply.
fn ramp_record(s: &SuggestedParams) -> &RampFeed {
    let all: Vec<&RampFeed> = s
        .warnings
        .iter()
        .filter_map(|w| {
            if let SuggestWarning::RampFeed { record, .. } = w {
                Some(record)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(all.len(), 1, "one RampFeed record: {all:?}");
    all[0]
}

/// (c) A 60° ramp whose vertical part is above the Z axis rate ships the Z
/// axis rate / tan 60°; the operator's angle holds (Q11).
#[test]
fn a_ramp_above_the_z_rate_ships_the_z_rate_over_tan_theta_p20b() {
    let dressups = DressupConfig {
        entry_style: DressupEntryStyle::Ramp,
        ramp_angle: 60.0,
        ..DressupConfig::default()
    };
    let tan60 = 60.0_f64.to_radians().tan();
    let pocket = OperationConfig::new_default(OperationType::Pocket);
    let free = suggest(&pocket, &softwood(), &no_z(), Some(&dressups));
    let free_ramp = match ramp_record(&free) {
        RampFeed::Sourced(s) => s.clone(),
        RampFeed::PlungeSlope(s) => panic!("expected a G6 ramp, got {s:?}"),
        RampFeed::NoEntryFeed { reason, .. } => panic!("expected a G6 ramp, got {reason:?}"),
    };
    assert_eq!(free_ramp.arm, RampArm::ChipTerm, "the free 60° ramp");
    // The vertical part of the free ramp is the G6 vertical rate. A Z rate
    // at half of it binds.
    let low_z = free_ramp.vertical_mm_min / 2.0;
    let capped = suggest(&pocket, &softwood(), &with_z(low_z), Some(&dressups));
    let ramp = match ramp_record(&capped) {
        RampFeed::Sourced(s) => s.clone(),
        RampFeed::PlungeSlope(s) => panic!("expected a G6 ramp, got {s:?}"),
        RampFeed::NoEntryFeed { reason, .. } => panic!("expected a G6 ramp, got {reason:?}"),
    };
    assert_eq!(ramp.arm, RampArm::ZAxisRate);
    assert_eq!(ramp.z_axis, ZAxisRate::Stated(low_z));
    let want = (low_z / tan60).floor();
    assert_eq!(ramp.value_mm_min, want);
    assert_eq!(capped.operation.ramp_feed_rate(), Some(want));
    let (face, detail) = ramp_record(&capped).card_text();
    assert!(face.contains("machine Z axis rate"), "{face}");
    assert!(detail.contains("$112"), "{detail}");
}

/// (d) No kinematics: no Z cap, and the plunge and ramp cards say so.
#[test]
fn a_machine_with_no_kinematics_has_no_z_cap_and_says_so_p20b() {
    let dressups = DressupConfig {
        entry_style: DressupEntryStyle::Ramp,
        ramp_angle: 3.0,
        ..DressupConfig::default()
    };
    let pocket = OperationConfig::new_default(OperationType::Pocket);
    let s = suggest(&pocket, &softwood(), &no_z(), Some(&dressups));
    assert_eq!(
        s.feeds_result.plunge.vertical_limit().z_axis(),
        ZAxisRate::NotStated
    );
    let (_, plunge_detail) = s.feeds_result.plunge.card_text();
    assert!(
        plunge_detail.contains("no Z cap applies"),
        "{plunge_detail}"
    );
    let (_, ramp_detail) = ramp_record(&s).card_text();
    assert!(ramp_detail.contains("no Z cap applies"), "{ramp_detail}");
    let drill = suggest(
        &OperationConfig::new_default(OperationType::Drill),
        &mdf(),
        &no_z(),
        None,
    );
    let (_, drill_detail) = drill.feeds_result.plunge.card_text();
    assert!(drill_detail.contains("no Z cap applies"), "{drill_detail}");
    assert!(
        !drill
            .feeds_result
            .warnings
            .iter()
            .any(|w| matches!(w, FeedsWarning::DrillFeedAtZAxisRate { .. }))
    );
}
