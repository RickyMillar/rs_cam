//! G10 — the milling plunge is a named fraction of the side feed that
//! ships.
//!
//! Pre-registration: `planning/extrapolation_2026-09-24/G10_PLAN.md` (§1 F1,
//! §3 A1-A4, §3 A10 item 1, §8 D1).
//!
//! The rule (rulings Q4 and Q5, 2026-09-25): `plunge = min(fraction x F,
//! tip cap, F)`, rounded down to 1 mm/min, where F is the side feed that
//! ships and the fraction comes from `feeds::extrapolation::PLUNGE_RULES`:
//!
//! - flat end mill (the G6 drill range and flutes): 1 / Z;
//! - ball nose 3.175-25.4 mm, 2 flutes: 0.50;
//! - tapered ball nose, tip 0.5-1.5875 mm (D1 strict), 2 flutes: 0.50;
//! - 60° V-bit, 2 flutes: 1 / 3.
//!
//! The ball and tapered-ball tip cap is 150 mm/min per mm of tip (a named
//! repo rule). Outside every rule the plunge is the named repo rule, the
//! material base, and the card says "no source; repo rule". A drill cycle
//! plunges at its feed.
//!
//! Before G10 the plunge was the material base for every family (with the
//! tip cap on a ball or tapered ball), so arms (a)-(d), (h), (i) and (j)
//! fail on the pre-G10 code: a 6 mm 2F flat in hardwood plunged about 700
//! mm/min whatever its feed, and no `PlungeBasis` or `PlungeReDerived`
//! existed.
//!
//! The arms:
//!
//! - (a) a 6 mm 2F flat Pocket in hardwood: floor(F / 2), `Claimed`;
//! - (b) the same at 3 flutes: floor(F / 3);
//! - (c) a 6 mm ball: min(floor(0.5 F), 900), and the binding is named;
//! - (d) a 12.7 mm 60° V-bit: floor(F / 3), and no `PlungeClampedToFeed`;
//! - (e) the refusals fall back to `MaterialBase` and say "no source; repo
//!   rule";
//! - (f) a 1 mm tapered tip: the tip cap binds, at 150 mm/min;
//! - (g) a drill cycle plunges at its feed;
//! - (h) where pass 9 moves the feed, the funnel runs the rule again and
//!   files one `PlungeReDerived`;
//! - (i) an explored feed of 1200 on a 6 mm ball: 600;
//! - (j) the provenance: `PublishedRule` on a claim, `RepoRule` on a bull;
//! - (k) the flat rule's range is the `DRILL_RULES` range.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::DressupConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::feeds::extrapolation::{DRILL_RULES, PLUNGE_RULES, PlungeKey};
use rs_cam_core::feeds::plunge::{PLUNGE_BASE_RULE_TEXT, resolve_plunge};
use rs_cam_core::feeds::suggest::{
    ApplyContext, ApplyScope, SuggestContext, SuggestForOperationInput, SuggestWarning,
    SuggestedParams, apply, feeds_input_for_operation, feeds_preview_for_operation,
    suggest_for_operation,
};
use rs_cam_core::feeds::{
    FeedsProvenance, FeedsResult, PlungeBasis, PlungeBinding, ProvenanceSource, SpindleStrategy,
    ValueProvenance, calculate, embedded_vendor_lut,
};
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, PlasticFamily, SheetGoodKind, WoodSpecies};

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

fn mdf() -> Material {
    Material::SheetGood {
        kind: SheetGoodKind::Mdf,
    }
}

fn acrylic() -> Material {
    Material::Plastic {
        family: PlasticFamily::Acrylic,
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

fn flat(diameter: f64, flutes: u32) -> ToolConfig {
    tool_of(ToolType::EndMill, diameter, flutes)
}

fn ball(diameter: f64) -> ToolConfig {
    tool_of(ToolType::BallNose, diameter, 2)
}

/// A V-bit of `diameter` and included angle `angle_deg`, 2 flutes.
fn vbit(diameter: f64, angle_deg: f64) -> ToolConfig {
    let mut t = tool_of(ToolType::VBit, diameter, 2);
    t.included_angle = angle_deg;
    t.cutting_length = 19.05;
    t.stickout = 27.05;
    t
}

/// A tapered ball: `diameter` is the tip, on a 6 mm shank.
fn tapered(tip_mm: f64) -> ToolConfig {
    let mut t = ToolConfig::new_default(ToolId(1), ToolType::TaperedBallNose);
    t.diameter = tip_mm;
    t.taper_half_angle = 5.7;
    t.shaft_diameter = 6.0;
    t.shank_diameter = 6.0;
    t.cutting_length = 20.0;
    t.flute_count = 2;
    t
}

fn op(op_type: OperationType) -> OperationConfig {
    OperationConfig::new_default(op_type)
}

fn router() -> MachineProfile {
    MachineProfile::generic_wood_router()
}

fn suggest_in(
    operation: &OperationConfig,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
    dressups: Option<&DressupConfig>,
) -> SuggestedParams {
    suggest_for_operation(SuggestForOperationInput {
        operation,
        tool,
        machine,
        material,
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::MatchChart,
        context: SuggestContext {
            dressups,
            ..SuggestContext::default()
        },
    })
    .unwrap_or_else(|e| panic!("Suggest must serve {:?}: {e}", operation.op_type()))
}

fn suggest(operation: &OperationConfig, tool: &ToolConfig, material: &Material) -> SuggestedParams {
    suggest_in(operation, tool, material, &router(), None)
}

/// The calculator result for one cell, with no validation (a refused
/// pairing still reports its plunge basis).
fn calculated(operation: &OperationConfig, tool: &ToolConfig, material: &Material) -> FeedsResult {
    let machine = router();
    let input = feeds_input_for_operation(
        operation,
        tool,
        material,
        &machine,
        embedded_vendor_lut(),
        SpindleStrategy::MatchChart,
    );
    calculate(&input)
}

/// The rule id of a claimed basis.
fn rule_id(basis: &PlungeBasis) -> &'static str {
    basis
        .claim()
        .unwrap_or_else(|| panic!("expected a G10 claim, got {basis:?}"))
        .rule
        .id
}

fn clamped_to_feed(warnings: &[SuggestWarning]) -> bool {
    warnings
        .iter()
        .any(|w| matches!(w, SuggestWarning::PlungeClampedToFeed { .. }))
}

fn re_derived(warnings: &[SuggestWarning]) -> Vec<(f64, f64, PlungeBinding)> {
    warnings
        .iter()
        .filter_map(|w| match w {
            SuggestWarning::PlungeReDerived {
                from_mm_min,
                to_mm_min,
                binding,
            } => Some((*from_mm_min, *to_mm_min, *binding)),
            _ => None,
        })
        .collect()
}

/// (a) A 6 mm 2F flat Pocket in hardwood on the generic router plunges at
/// floor(F / 2), and the basis is the flat claim.
#[test]
fn a_flat_plunges_at_feed_over_two_g10() {
    let s = suggest(&op(OperationType::Pocket), &flat(6.0, 2), &hardwood());
    assert_eq!(rule_id(&s.feeds_result.plunge), "g10_plunge_flat");
    let feed = s.operation.feed_rate();
    assert_eq!(
        s.operation.plunge_rate(),
        (feed / 2.0).floor(),
        "plunge {} at feed {feed}",
        s.operation.plunge_rate()
    );
    let (headline, _) = s.feeds_result.plunge.card_text();
    assert!(
        headline.starts_with("Plunge = feed / 2 (G10 plunge claim"),
        "{headline}"
    );
}

/// (b) The same cell at 3 flutes: floor(F / 3).
#[test]
fn a_three_flute_flat_plunges_at_feed_over_three_g10() {
    let s = suggest(&op(OperationType::Pocket), &flat(6.0, 3), &hardwood());
    assert_eq!(rule_id(&s.feeds_result.plunge), "g10_plunge_flat");
    let feed = s.operation.feed_rate();
    assert_eq!(
        s.operation.plunge_rate(),
        (feed / 3.0).floor(),
        "feed {feed}"
    );
}

/// (c) A 6 mm ball: min(floor(0.5 F), 900), and the binding is named on
/// the card.
#[test]
fn a_ball_plunges_at_half_the_feed_under_its_tip_cap_g10() {
    let s = suggest(&op(OperationType::DropCutter), &ball(6.0), &hardwood());
    assert_eq!(rule_id(&s.feeds_result.plunge), "g10_plunge_ball");
    let cap = s
        .feeds_result
        .plunge
        .tip_cap()
        .expect("a ball has a tip cap");
    assert!((cap.cap_mm_min - 900.0).abs() < 1e-9, "{cap:?}");
    let feed = s.operation.feed_rate();
    let expected = (0.5 * feed).floor().min(900.0);
    assert_eq!(s.operation.plunge_rate(), expected, "feed {feed}");
    let binding = if (0.5 * feed).floor() <= 900.0 {
        PlungeBinding::Rule
    } else {
        PlungeBinding::TipCap
    };
    assert_eq!(
        resolve_plunge(&s.feeds_result.plunge, feed),
        Some((expected, binding))
    );
    let (headline, _) = s.feeds_result.plunge.card_text_at(feed);
    assert!(
        headline.contains(&format!("set by {}", binding.label())),
        "{headline}"
    );
}

/// (d) A 12.7 mm 60° V-bit: floor(F / 3). The rule sits under the feed, so
/// the plunge-to-feed clamp does not fire.
#[test]
fn a_sixty_degree_vbit_plunges_at_a_third_of_the_feed_g10() {
    let s = suggest(&op(OperationType::VCarve), &vbit(12.7, 60.0), &mdf());
    assert_eq!(rule_id(&s.feeds_result.plunge), "g10_plunge_vbit60");
    let feed = s.operation.feed_rate();
    assert_eq!(
        s.operation.plunge_rate(),
        (feed / 3.0).floor(),
        "feed {feed}"
    );
    assert!(
        !clamped_to_feed(&s.warnings),
        "the V-bit plunge was clamped to the feed: {:?}",
        s.warnings
    );
}

/// (e) Outside every rule the plunge is the named repo rule: a flat above
/// the range, a 4-flute flat, a bull, a 90° V-bit, a flat in acrylic, and a
/// tapered tip of 3.175 mm (D1 strict).
///
/// The plan named a 6.35 mm flat; the G6 range widened to 3.0-12.7 mm on
/// 2026-09-25, so 6.35 mm is inside it and the arm uses 15.875 mm (5/8 in).
#[test]
fn a_refused_tool_plunges_at_the_named_repo_rule_g10() {
    for (name, operation, tool, material) in [
        (
            "flat 15.875 mm",
            op(OperationType::Pocket),
            flat(15.875, 2),
            hardwood(),
        ),
        (
            "flat 6 mm 4F",
            op(OperationType::Pocket),
            flat(6.0, 4),
            hardwood(),
        ),
        (
            "bull 6 mm",
            op(OperationType::Adaptive),
            tool_of(ToolType::BullNose, 6.0, 2),
            hardwood(),
        ),
        (
            "V-bit 90°",
            op(OperationType::VCarve),
            vbit(12.7, 90.0),
            mdf(),
        ),
        (
            "flat 6 mm acrylic",
            op(OperationType::Pocket),
            flat(6.0, 2),
            acrylic(),
        ),
        (
            "tapered tip 3.175 mm",
            op(OperationType::DropCutter),
            tapered(3.175),
            hardwood(),
        ),
    ] {
        let result = calculated(&operation, &tool, &material);
        let PlungeBasis::MaterialBase {
            base_mm_min,
            tip_cap,
            ..
        } = &result.plunge
        else {
            panic!(
                "{name}: expected the named repo rule, got {:?}",
                result.plunge
            );
        };
        let (headline, detail) = result.plunge.card_text();
        assert!(
            headline.contains("no source; repo rule"),
            "{name}: {headline}"
        );
        assert!(
            detail.starts_with(PLUNGE_BASE_RULE_TEXT),
            "{name}: {detail}"
        );
        let feed = result.feed_rate_mm_min;
        let mut expected = *base_mm_min;
        if let Some(cap) = tip_cap {
            expected = expected.min(cap.cap_mm_min);
        }
        let expected = expected.min(feed).floor();
        assert_eq!(result.plunge_rate_mm_min, expected, "{name}: feed {feed}");
    }
}

/// (f) A 1 mm tapered tip in wood: inside the tapered claim, and the tip
/// cap binds at 150 mm/min.
#[test]
fn a_one_millimetre_tapered_tip_is_held_by_the_tip_cap_g10() {
    let s = suggest(&op(OperationType::DropCutter), &tapered(1.0), &hardwood());
    assert_eq!(rule_id(&s.feeds_result.plunge), "g10_plunge_tapered");
    let feed = s.operation.feed_rate();
    assert!(
        0.5 * feed > 150.0,
        "the cap must be the smaller term: {feed}"
    );
    assert_eq!(
        resolve_plunge(&s.feeds_result.plunge, feed),
        Some((150.0, PlungeBinding::TipCap))
    );
    assert!(s.operation.plunge_rate() <= 150.0);
    assert_eq!(s.operation.plunge_rate(), 150.0);
}

/// (g) A drill cycle plunges at its feed, as before G10.
#[test]
fn a_drill_cycle_plunges_at_its_feed_g10() {
    for op_type in [OperationType::Drill, OperationType::AlignmentPinDrill] {
        let s = suggest(&op(op_type), &flat(6.0, 2), &softwood());
        assert_eq!(
            s.feeds_result.plunge,
            PlungeBasis::DrillCycle,
            "{op_type:?}"
        );
        assert_eq!(
            s.feeds_result.plunge_rate_mm_min, s.feeds_result.feed_rate_mm_min,
            "{op_type:?}"
        );
        assert!(re_derived(&s.warnings).is_empty(), "{op_type:?}");
    }
}

/// (h) Where pass 9 re-derives the feed at the final depth, the funnel runs
/// the plunge rule again at that feed and files one `PlungeReDerived`.
///
/// The grid is the flat claim's cells on the dial's roughing doors. Pass 9
/// fires only where the final depth crosses a step of the depth ladder, so
/// the arm checks every cell where it fires and requires at least one.
#[test]
fn the_funnel_runs_the_rule_again_at_the_rescaled_feed_g10() {
    let mut fired = 0;
    for op_type in [
        OperationType::Pocket,
        OperationType::Adaptive,
        OperationType::Adaptive3d,
    ] {
        for (d, z) in [(3.175, 2), (6.0, 2), (6.0, 3)] {
            for material in [hardwood(), softwood(), mdf()] {
                let tool = flat(d, z);
                let s = suggest(&op(op_type), &tool, &material);
                let rescaled = s
                    .warnings
                    .iter()
                    .any(|w| matches!(w, SuggestWarning::FeedRescaledToFinalGeometry { .. }));
                if !rescaled {
                    continue;
                }
                let feed = s.operation.feed_rate();
                if (feed - s.feeds_result.feed_rate_mm_min).abs() < 1.0 {
                    continue;
                }
                fired += 1;
                let cell = format!("{op_type:?} {d} mm {z}F {material:?}");
                assert_eq!(rule_id(&s.feeds_result.plunge), "g10_plunge_flat", "{cell}");
                let expected = (feed / f64::from(z)).floor();
                assert_eq!(s.operation.plunge_rate(), expected, "{cell}: feed {feed}");
                let records = re_derived(&s.warnings);
                assert_eq!(records.len(), 1, "{cell}: {records:?}");
                let (from, to, binding) = records[0];
                assert_eq!(to, expected, "{cell}");
                assert_eq!(from, s.feeds_result.plunge_rate_mm_min, "{cell}");
                assert_eq!(binding, PlungeBinding::Rule, "{cell}");
            }
        }
    }
    assert!(
        fired > 0,
        "no cell of the grid re-derived its feed at pass 9"
    );
}

/// (i) An explored feed of 1200 on a 6 mm ball: the plunge is 600.
#[test]
fn an_explored_feed_moves_the_plunge_with_it_g10() {
    let tool = ball(6.0);
    let material = hardwood();
    let machine = router();
    let operation = op(OperationType::DropCutter);
    let preview = feeds_preview_for_operation(
        &operation,
        &tool,
        &material,
        &machine,
        embedded_vendor_lut(),
        SpindleStrategy::MatchChart,
    );
    let rec = preview
        .applicable()
        .expect("the ball cell validates")
        .with_explored_speeds(1200.0, 14_000.0);
    let mut applied = operation.clone();
    let mut prov = FeedsProvenance::default();
    let warnings = apply(
        &rec,
        ApplyScope::Speeds,
        &mut applied,
        &mut prov,
        ApplyContext {
            tool: &tool,
            machine: &machine,
            material: &material,
            pass_role: operation.feeds_style().1,
            suggest: SuggestContext::default(),
        },
    );
    assert_eq!(applied.feed_rate(), 1200.0);
    assert_eq!(applied.plunge_rate(), 600.0);
    let records = re_derived(&warnings);
    assert_eq!(records.len(), 1, "{warnings:?}");
    assert_eq!(records[0].1, 600.0);
    assert_eq!(records[0].2, PlungeBinding::Rule);
}

/// (j) The provenance: the flat claim stamps `PublishedRule` with its id;
/// the bull's named rule stamps `RepoRule`.
#[test]
fn the_plunge_provenance_names_its_rule_g10() {
    let s = suggest(&op(OperationType::Pocket), &flat(6.0, 2), &hardwood());
    assert_eq!(
        s.provenance.plunge_rate,
        Some(ValueProvenance::published_rule("g10_plunge_flat"))
    );

    let s = suggest(
        &op(OperationType::Adaptive),
        &tool_of(ToolType::BullNose, 6.0, 2),
        &hardwood(),
    );
    assert!(
        matches!(s.feeds_result.plunge, PlungeBasis::MaterialBase { .. }),
        "{:?}",
        s.feeds_result.plunge
    );
    let stamp = s.provenance.plunge_rate.expect("the plunge is stamped");
    assert_eq!(stamp.source, ProvenanceSource::RepoRule, "{stamp:?}");
    assert_eq!(stamp.reference.as_deref(), Some("material_plunge_base"));
}

/// (k) The flat rule reads the G6 drill rule's range and flutes.
#[test]
fn the_flat_claim_range_is_the_drill_range_g10() {
    let drill = DRILL_RULES.first().expect("one drill rule");
    let flat_rule = PLUNGE_RULES
        .iter()
        .find(|r| r.id == "g10_plunge_flat")
        .expect("the flat rule");
    assert_eq!(
        flat_rule.key,
        PlungeKey::Diameter {
            lo_mm: drill.range_mm.0,
            hi_mm: drill.range_mm.1,
        }
    );
    assert_eq!(flat_rule.flutes, drill.flutes);
}
