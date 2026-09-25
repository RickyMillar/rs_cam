//! G10 Q2 — with no G6 chip, a helix or ramp entry holds its vertical rate
//! at the plunge that ships; the entry rules are notes on the ramp record.
//!
//! Pre-registration: `planning/extrapolation_2026-09-24/G10_PLAN.md` (§3 A5,
//! §3 A10 item 2).
//!
//! The rule (ruling Q2, a repo rule with no source): off the G6 drill claim,
//! `ramp feed = min(F, plunge / tan θ)`, rounded down to 1 mm/min. θ is the
//! entry slope of the operation that ships: `tan(angle)` for a ramp,
//! `pitch / (2π r)` for a helix. A straight plunge or an unknown entry
//! writes `None`. The G6 arm (`a_ramp_feed_is_the_g6_chip_over_the_entry_slope_g6ramp`)
//! is unchanged.
//!
//! The notes (rulings Q6-Q10, Q12) ride on the `RampFeed` record: the ramp
//! angle or the helix (a repo rule or an operator value), the core or the
//! centre pip from the tool profile, the clearance (operator rule 0.5 mm),
//! and a caution on a straight plunge in a Roughing pass.
//!
//! Before G10 an entry off the G6 claim wrote `None` (the entry used the
//! plunge rate) and the record carried no notes, so every arm below but
//! the `None` arm fails on the pre-G10 code.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{DressupConfig, DressupEntryStyle};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::ramp::{EntryNotes, PlungeSlopeRamp, RampArm, RampFallback, entry_notes};
use rs_cam_core::feeds::suggest::{
    SuggestContext, SuggestForOperationInput, SuggestWarning, SuggestedParams,
    suggest_for_operation,
};
use rs_cam_core::feeds::{PassRole, RampFeed, SpindleStrategy, embedded_vendor_lut};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, WoodSpecies};

fn softwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericSoftwood,
    }
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

fn pocket() -> OperationConfig {
    OperationConfig::new_default(OperationType::Pocket)
}

fn ramp_dressup(angle_deg: f64) -> DressupConfig {
    DressupConfig {
        entry_style: DressupEntryStyle::Ramp,
        ramp_angle: angle_deg,
        ..DressupConfig::default()
    }
}

fn helix_dressup(radius_mm: f64, pitch_mm: f64) -> DressupConfig {
    DressupConfig {
        entry_style: DressupEntryStyle::Helix,
        helix_radius: Some(radius_mm),
        helix_pitch: pitch_mm,
        ..DressupConfig::default()
    }
}

fn suggest(
    operation: &OperationConfig,
    tool: &ToolConfig,
    dressups: Option<&DressupConfig>,
) -> SuggestedParams {
    suggest_for_operation(SuggestForOperationInput {
        operation,
        tool,
        machine: &MachineProfile::generic_wood_router(),
        material: &softwood(),
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::MatchChart,
        context: SuggestContext {
            dressups,
            ..SuggestContext::default()
        },
    })
    .unwrap_or_else(|e| panic!("Suggest must serve {:?}: {e}", operation.op_type()))
}

/// The one `RampFeed` record of a Suggest, with its notes.
fn record(s: &SuggestedParams) -> (&RampFeed, &EntryNotes) {
    let all: Vec<_> = s
        .warnings
        .iter()
        .filter_map(|w| match w {
            SuggestWarning::RampFeed { record, notes, .. } => Some((record, notes)),
            _ => None,
        })
        .collect();
    assert_eq!(all.len(), 1, "one RampFeed record per Suggest: {all:?}");
    all[0]
}

fn plunge_slope(r: &RampFeed) -> &PlungeSlopeRamp {
    match r {
        RampFeed::PlungeSlope(s) => s,
        RampFeed::Sourced(s) => panic!("expected the plunge slope, got {s:?}"),
        RampFeed::NoEntryFeed { reason, .. } => panic!("expected the plunge slope, got {reason:?}"),
    }
}

/// A 6 mm bull Pocket with a 3° dressup ramp: the ramp ships
/// floor(min(F, plunge / tan 3°)), and the cut feed is the smaller term.
#[test]
fn a_bull_ramp_ships_the_cut_feed_under_the_plunge_limit_g10() {
    let dressups = ramp_dressup(3.0);
    let s = suggest(
        &pocket(),
        &tool_of(ToolType::BullNose, 6.0, 2),
        Some(&dressups),
    );
    let (r, notes) = record(&s);
    let ramp = plunge_slope(r);
    let feed = s.operation.feed_rate();
    let plunge = s.operation.plunge_rate();
    let tan3 = 3.0_f64.to_radians().tan();
    let expected = feed.min(plunge / tan3).floor();
    assert_eq!(s.operation.ramp_feed_rate(), Some(expected));
    assert_eq!(ramp.arm, RampArm::CutFeed);
    assert!((ramp.plunge_mm_min - plunge).abs() < 1e-9);
    assert!((ramp.plunge_term_mm_min - plunge / tan3).abs() < 1e-6);
    let (face, detail) = r.card_text();
    assert!(
        face.contains("plunge limit") && face.contains("3.00°"),
        "{face}"
    );
    assert!(
        detail.starts_with("no source; vertical rate = plunge (repo rule)"),
        "{detail}"
    );

    // The notes: the angle (the default 3°, a repo rule) and the clearance.
    let lines: Vec<&str> = notes
        .as_slice()
        .iter()
        .map(|n| n.headline.as_str())
        .collect();
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("Ramp angle 3.00°: repo rule")),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("operator rule 0.5 mm")),
        "{lines:?}"
    );
}

/// A 3.175 mm ball Pocket with a dressup helix r 0.9525 mm (0.3 x D, the
/// Part B rule), pitch 1 mm: tan θ = 1 / (2π x 0.9525), θ 9.49°. The plunge
/// limit is under the feed, so it sets the ramp.
#[test]
fn a_steep_ball_helix_ships_the_plunge_limit_g10() {
    let radius_mm = 0.3 * 3.175;
    let dressups = helix_dressup(radius_mm, 1.0);
    let s = suggest(
        &pocket(),
        &tool_of(ToolType::BallNose, 3.175, 2),
        Some(&dressups),
    );
    let (r, _) = record(&s);
    let ramp = plunge_slope(r);
    assert!((ramp.theta_deg - 9.49).abs() < 0.01, "{}", ramp.theta_deg);
    assert_eq!(ramp.arm, RampArm::PlungeTerm);
    let tan = 1.0 / (std::f64::consts::TAU * radius_mm);
    let expected = (s.operation.plunge_rate() / tan).floor();
    assert!(expected < s.operation.feed_rate());
    assert_eq!(s.operation.ramp_feed_rate(), Some(expected));
}

/// A straight plunge (`EntryOff`) and an unknown entry (`EntryUnknown`)
/// write `None`.
#[test]
fn a_plunge_or_unknown_entry_writes_none_g10() {
    let off = DressupConfig {
        entry_style: DressupEntryStyle::None,
        ..DressupConfig::default()
    };
    let bull = tool_of(ToolType::BullNose, 6.0, 2);
    for (name, dressups, reason) in [
        ("entry off", Some(&off), RampFallback::EntryOff),
        ("entry unknown", None, RampFallback::EntryUnknown),
    ] {
        let s = suggest(&pocket(), &bull, dressups);
        let (r, _) = record(&s);
        match r {
            RampFeed::NoEntryFeed { reason: got, .. } => assert_eq!(*got, reason, "{name}"),
            other => panic!("{name}: expected no entry feed, got {other:?}"),
        }
        assert_eq!(s.operation.ramp_feed_rate(), None, "{name}");
    }
}

/// The helix notes come from the tool profile: a 6 mm ball at r 1.8 leaves
/// a pip 3 - sqrt(3² - 1.8²) = 0.60 mm high (the ball profile height at r;
/// the plan's 0.29 does not follow from the profile); a 3.175 mm flat at r
/// 2.0 would leave a core 2 x (2.0 - 1.5875) = 0.825 mm wide, so since G10
/// Part B the engine caps r at the flat bottom 1.5875 mm and the card says
/// so (plan R8).
#[test]
fn the_helix_notes_read_the_tool_profile_g10() {
    let helix = helix_dressup(1.8, 1.0);
    let notes = entry_notes(
        &pocket(),
        Some(&helix),
        &tool_of(ToolType::BallNose, 6.0, 2),
        PassRole::Roughing,
    );
    let pip = notes
        .as_slice()
        .iter()
        .find(|n| n.headline.contains("centre pip"))
        .unwrap_or_else(|| panic!("no pip note: {notes:?}"));
    assert!(pip.headline.contains("pip 0.60 mm"), "{}", pip.headline);
    assert!(!pip.caution);

    let helix = helix_dressup(2.0, 1.0);
    let notes = entry_notes(
        &pocket(),
        Some(&helix),
        &tool_of(ToolType::EndMill, 3.175, 2),
        PassRole::Roughing,
    );
    let cap = notes
        .as_slice()
        .iter()
        .find(|n| n.headline.contains("capped r"))
        .unwrap_or_else(|| panic!("no cap note: {notes:?}"));
    // r 2.0 capped at 3.175 / 2 = 1.5875; the card prints two decimals.
    assert!(
        cap.headline
            .contains("capped r 2.00 → 1.59 mm (no-core rule, geometry)"),
        "{}",
        cap.headline
    );
    // The core it would have left: 2 x (2.0 - 3.175 / 2) = 0.825.
    assert!(
        cap.detail.contains("core Ø 0.83 mm") || cap.detail.contains("core Ø 0.82 mm"),
        "{}",
        cap.detail
    );
    assert!(!cap.caution, "the engine leaves no core");
    assert!(
        !notes
            .as_slice()
            .iter()
            .any(|n| n.headline.contains("core Ø")),
        "no core line after the cap: {notes:?}"
    );
    // The helix line states the radius the engine emits: 1.5875 / 3.175 =
    // 0.50 x D.
    assert!(
        notes.as_slice()[0]
            .headline
            .starts_with("Helix r 1.59 mm (0.50 x D), pitch 1.00 mm"),
        "{notes:?}"
    );
}

/// A Roughing op that enters with a straight plunge carries the Q12
/// caution; a Finish pass does not.
#[test]
fn a_straight_roughing_plunge_carries_the_q12_caution_g10() {
    let off = DressupConfig {
        entry_style: DressupEntryStyle::None,
        ..DressupConfig::default()
    };
    let tool = tool_of(ToolType::EndMill, 6.0, 2);
    let s = suggest(&pocket(), &tool, Some(&off));
    let (_, notes) = record(&s);
    let caution = notes
        .as_slice()
        .iter()
        .find(|n| n.caution && n.headline.starts_with("Straight plunge entry"))
        .unwrap_or_else(|| panic!("no Q12 caution: {notes:?}"));
    assert!(
        caution.headline.contains("down-cut or compression"),
        "{}",
        caution.headline
    );
    assert!(caution.detail.contains("ruling Q12"), "{}", caution.detail);

    let finish = entry_notes(&pocket(), Some(&off), &tool, PassRole::Finish);
    assert!(finish.is_empty(), "{finish:?}");
}
