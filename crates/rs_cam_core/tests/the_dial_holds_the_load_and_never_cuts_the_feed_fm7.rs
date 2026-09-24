//! FM7 — the aggressiveness dial holds the load and never cuts the feed.
//!
//! Ruling R4 WP3 (`planning/feeds_matrix_2026-09-23/R4_AGGRESSIVENESS_SPEC.md`
//! §2.3, rulings of 2026-09-24): `MachineProfile::aggressiveness` replaces the
//! hidden 0.75 feed factor. Suggest pass 6b holds the predicted lateral force
//! and spindle power at `k_eff = aggressiveness x long-tool share` of their
//! values at the base engagement, by ONE common scale on the depth per pass
//! and the stepover. The chipload does not move.
//!
//! The arms:
//!
//! - (a) k = 0.85, Ø6 hardwood pocket, short tool: the feed is the band
//!   chipload x depth ladder x rpm x flutes with no machine factor; depth and
//!   stepover carry one common scale; force after <= 0.85 x force before; the
//!   warning carries the pairs and `target_met`.
//! - (b) k = 1.00: nothing moves and no dial record.
//! - (c) k = 1.20: the engagement rises inside the clamps (rigidity depth
//!   cap, stepover <= D) and the Caution text is present.
//! - (d) a Finish role and a drill get the "no dial action" record and the
//!   geometry of k = 1.00.
//! - (e) a 7.5 x D tool: `k_eff = 0.85 x 0.75 = 0.6375`, and the feed is the
//!   feed of the short tool (the share is not a feed factor).
//! - (f) over the FM0 grid, no warning that carries a `_from` / `_to` pair or
//!   a factor is filed without the pair (the operator's rule: no invisible
//!   calculations or de-rates). Exhaustive matches, no wildcard: a new
//!   variant forces a decision here.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::{ToolConfig, ToolId, ToolType};
use rs_cam_core::diagnostics::adapters::from_feeds::diagnostic_from_suggest_warning;
use rs_cam_core::diagnostics::{Severity, ids};
use rs_cam_core::feeds::rationale::{AGGRESSIVENESS_ABOVE_BASE_TEXT, SuggestRationale};
use rs_cam_core::feeds::suggest::{
    AggressivenessSkip, SuggestContext, SuggestForOperationInput, SuggestWarning, SuggestedParams,
    suggest_for_operation,
};
use rs_cam_core::feeds::{
    FeedsWarning, OperationFamily, PassRole, SpindleStrategy, embedded_vendor_lut,
};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::material::{Material, PlywoodGrade, SheetGoodKind, WoodSpecies};

const D_MM: f64 = 6.0;
/// 20 mm on Ø6 is 3.3 x D: no long-tool share.
const SHORT_STICKOUT_MM: f64 = 20.0;
/// 45 mm on Ø6 is 7.5 x D: the 0.75 long-tool share.
const LONG_STICKOUT_MM: f64 = 45.0;
const REL_EPS: f64 = 1e-9;

fn hardwood() -> Material {
    Material::SolidWood {
        species: WoodSpecies::GenericHardwood,
    }
}

fn endmill(stickout: f64) -> ToolConfig {
    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = D_MM;
    tool.shaft_diameter = D_MM;
    tool.shank_diameter = D_MM;
    tool.cutting_length = 22.0;
    tool.stickout = stickout;
    tool.flute_count = 2;
    tool
}

/// The generic router with the dial at `k`. The travel rate is 6000 mm/min
/// so the cutting ceiling (min(6000, the 6000 default cap)) sits above the
/// pocket's 18 000 x 0.127 x 2 = 4572 mm/min: arm (a) reads a feed that no
/// ceiling touched. Before ruling R4 the 0.75 factors kept it under 4000.
/// The spindle is a 3 kW constant-power synthetic so the Step 6 power ladder
/// does not act either: this file tests the dial, not the power rungs.
fn machine(k: f64) -> MachineProfile {
    let mut m = MachineProfile::generic_wood_router();
    m.max_feed_mm_min = 6000.0;
    m.power = rs_cam_core::machine::PowerModel::ConstantPower { power_kw: 3.0 };
    m.aggressiveness = k;
    m
}

fn suggest(
    op: OperationType,
    tool: &ToolConfig,
    material: &Material,
    machine: &MachineProfile,
) -> Option<SuggestedParams> {
    let operation = OperationConfig::new_default(op);
    suggest_for_operation(SuggestForOperationInput {
        operation: &operation,
        tool,
        machine,
        material,
        lut: embedded_vendor_lut(),
        spindle_strategy: SpindleStrategy::MatchChart,
        context: SuggestContext::default(),
    })
    .ok()
}

/// The dial record's fields, when it was filed.
#[derive(Debug, Clone, Copy)]
struct Dial {
    target_share: f64,
    ld_factor: f64,
    scale: f64,
    dpp: (Option<f64>, Option<f64>),
    stepover: (Option<f64>, Option<f64>),
    force: (Option<f64>, Option<f64>),
    power: (Option<f64>, Option<f64>),
    target_met: bool,
}

fn dial(warnings: &[SuggestWarning]) -> Option<Dial> {
    warnings.iter().find_map(|w| match w {
        SuggestWarning::EngagementReducedForAggressiveness {
            target_share,
            ld_factor,
            scale,
            dpp_from,
            dpp_to,
            stepover_from,
            stepover_to,
            force_n_before,
            force_n_after,
            power_kw_before,
            power_kw_after,
            target_met,
            ..
        } => Some(Dial {
            target_share: *target_share,
            ld_factor: *ld_factor,
            scale: *scale,
            dpp: (*dpp_from, *dpp_to),
            stepover: (*stepover_from, *stepover_to),
            force: (*force_n_before, *force_n_after),
            power: (*power_kw_before, *power_kw_after),
            target_met: *target_met,
        }),
        _ => None,
    })
}

fn not_applied(warnings: &[SuggestWarning]) -> Option<AggressivenessSkip> {
    warnings.iter().find_map(|w| match w {
        SuggestWarning::AggressivenessNotApplied { reason, .. } => Some(*reason),
        _ => None,
    })
}

fn pocket(k: f64, stickout: f64) -> SuggestedParams {
    suggest(
        OperationType::Pocket,
        &endmill(stickout),
        &hardwood(),
        &machine(k),
    )
    .expect("a Ø6 end mill pocket in hardwood ships a recipe")
}

/// (a) k = 0.85 on a short tool.
#[test]
fn the_dial_scales_the_engagement_and_holds_the_chip_fm7() {
    let s = pocket(0.85, SHORT_STICKOUT_MM);
    let d = dial(&s.warnings).unwrap_or_else(|| {
        panic!(
            "k = 0.85 must file the dial record. warnings: {:?}",
            s.warnings
        )
    });
    assert!((d.target_share - 0.85).abs() < 1e-12, "{d:?}");
    assert_eq!(d.ld_factor, 1.0, "3.3 x D takes no long-tool share");

    // The pairs exist, and the scale is one number on both levers.
    let (dpp_from, dpp_to) = (d.dpp.0.unwrap(), d.dpp.1.unwrap());
    let (so_from, so_to) = (d.stepover.0.unwrap(), d.stepover.1.unwrap());
    assert!(d.scale > 0.0 && d.scale < 1.0, "{d:?}");
    // The stepover rounds DOWN to 0.001 mm; the depth rounds down and then
    // snaps to the generator's staircase, which is at or below the value.
    assert!(
        so_to <= d.scale * so_from + 1e-9 && so_to >= d.scale * so_from - 1e-3 - 1e-9,
        "the stepover {so_to} is not scale {} x {so_from}",
        d.scale
    );
    assert!(
        dpp_to <= d.scale * dpp_from + 1e-9,
        "the depth {dpp_to} is above scale {} x {dpp_from}",
        d.scale
    );
    assert_eq!(s.operation.depth_per_pass(), Some(dpp_to));
    assert_eq!(s.operation.stepover(), Some(so_to));

    // The load is at or below the target.
    let (f0, f1) = (d.force.0.unwrap(), d.force.1.unwrap());
    assert!(
        f1 <= 0.85 * f0 * (1.0 + REL_EPS),
        "force {f1} N is above 0.85 x {f0} N"
    );
    if let (Some(p0), Some(p1)) = d.power {
        assert!(
            p1 <= 0.85 * p0 * (1.0 + REL_EPS),
            "power {p1} above 0.85 x {p0}"
        );
    }
    assert!(d.target_met, "{d:?}");

    // The feed carries no machine factor: target x depth ladder x rpm x
    // flutes, on a cut that no ceiling and no power limit touched.
    let r = &s.feeds_result;
    let dr = &r.derates;
    assert_eq!(dr.power_limit, 1.0, "precondition: no power limit");
    assert_eq!(dr.feed_clamp, 1.0, "precondition: no machine ceiling");
    let expected = dr.target_chip_load_mm * dr.depth_tier * r.rpm * 2.0;
    assert!(
        (r.feed_rate_mm_min - expected).abs() <= expected * REL_EPS,
        "calculator feed {} != chip {} x tier {} x rpm {} x 2 = {expected}",
        r.feed_rate_mm_min,
        dr.target_chip_load_mm,
        dr.depth_tier,
        r.rpm
    );
    assert!(
        s.operation.feed_rate() <= r.feed_rate_mm_min + 1e-9
            && s.operation.feed_rate() > r.feed_rate_mm_min - 1.0,
        "the shipped feed {} is not the calculator feed {} rounded down",
        s.operation.feed_rate(),
        r.feed_rate_mm_min
    );

    // The dial never cuts the feed: the same cell at k = 1.00 ships it too.
    let full = pocket(1.0, SHORT_STICKOUT_MM);
    assert_eq!(
        s.operation.feed_rate(),
        full.operation.feed_rate(),
        "the dial moved the feed"
    );

    // The finding: Info when the target is met and k <= 1.0.
    let w = s
        .warnings
        .iter()
        .find(|w| matches!(w, SuggestWarning::EngagementReducedForAggressiveness { .. }))
        .unwrap();
    let finding = diagnostic_from_suggest_warning(ToolpathId(0), w).unwrap();
    assert_eq!(finding.id.as_str(), ids::FEEDS_AGGRESSIVENESS_ENGAGEMENT);
    assert!(ids::ALL.contains(&ids::FEEDS_AGGRESSIVENESS_ENGAGEMENT));
    assert_eq!(finding.severity, Severity::Info);

    // The rationale rows: one for the depth, one for the stepover.
    let rows = SuggestRationale::from_warnings(std::slice::from_ref(w)).entries;
    assert_eq!(rows.len(), 2, "{rows:?}");
}

/// (b) k = 1.00: nothing moves and there is no dial record.
#[test]
fn at_one_the_dial_does_nothing_fm7() {
    let full = pocket(1.0, SHORT_STICKOUT_MM);
    assert!(dial(&full.warnings).is_none(), "{:?}", full.warnings);
    assert!(not_applied(&full.warnings).is_none(), "{:?}", full.warnings);
    // The base the k = 0.85 record starts from is this geometry.
    let reduced = pocket(0.85, SHORT_STICKOUT_MM);
    let d = dial(&reduced.warnings).unwrap();
    assert_eq!(full.operation.depth_per_pass(), d.dpp.0);
    assert_eq!(full.operation.stepover(), d.stepover.0);
}

/// (c) k = 1.20: the engagement rises inside the clamps, with a Caution.
#[test]
fn above_one_the_engagement_rises_inside_the_clamps_fm7() {
    let m = machine(1.2);
    let s = pocket(1.2, SHORT_STICKOUT_MM);
    let d = dial(&s.warnings).unwrap_or_else(|| panic!("{:?}", s.warnings));
    assert!((d.target_share - 1.2).abs() < 1e-12);
    let (dpp_from, dpp_to) = (d.dpp.0.unwrap(), d.dpp.1.unwrap());
    let (so_from, so_to) = (d.stepover.0.unwrap(), d.stepover.1.unwrap());
    assert!(
        dpp_to >= dpp_from - 1e-9 && so_to >= so_from - 1e-9,
        "{d:?}"
    );
    assert!(
        dpp_to > dpp_from + 1e-9 || so_to > so_from + 1e-9,
        "at least one lever must rise: {d:?}"
    );
    // The clamps: the roughing rigidity cap and one diameter.
    let cap = m.rigidity.doc_roughing_factor * D_MM;
    assert!(
        dpp_to <= cap + 1e-6,
        "depth {dpp_to} above the rigidity cap {cap}"
    );
    assert!(so_to <= D_MM + 1e-9, "stepover {so_to} above D");
    let (f0, f1) = (d.force.0.unwrap(), d.force.1.unwrap());
    assert!(
        f1 <= 1.2 * f0 * (1.0 + REL_EPS),
        "force {f1} above 1.2 x {f0}"
    );

    let w = s
        .warnings
        .iter()
        .find(|w| matches!(w, SuggestWarning::EngagementReducedForAggressiveness { .. }))
        .unwrap();
    let rows = SuggestRationale::from_warnings(std::slice::from_ref(w)).entries;
    assert!(
        rows.iter().all(|r| r
            .detail
            .as_deref()
            .is_some_and(|t| t.contains(AGGRESSIVENESS_ABOVE_BASE_TEXT))),
        "the Caution text must be on every row: {rows:?}"
    );
    let finding = diagnostic_from_suggest_warning(ToolpathId(0), w).unwrap();
    assert_eq!(finding.severity, Severity::Caution);
    // The feed does not move above 1.0 either.
    let full = pocket(1.0, SHORT_STICKOUT_MM);
    assert_eq!(s.operation.feed_rate(), full.operation.feed_rate());
}

/// (d) a Finish role and a drill: the "no dial action" record, and the
/// geometry of k = 1.00.
#[test]
fn a_finish_and_a_drill_get_no_dial_action_fm7() {
    let materials = [
        hardwood(),
        Material::Plywood {
            grade: PlywoodGrade::Softwood,
        },
    ];
    let (mut finish, mut drill) = (0usize, 0usize);
    for &op in OperationType::ALL {
        let (family, role) = OperationConfig::new_default(op).feeds_style();
        let is_drill = family == OperationFamily::Drill;
        if !(is_drill || role == PassRole::Finish) {
            continue;
        }
        for &tool_type in ToolType::ALL {
            let mut tool = ToolConfig::new_default(ToolId(0), tool_type);
            // Ruling B5 (G6): a drill ships only through the drill claim (a
            // 2- or 3-flute flat end mill at 3.175-6.0 mm). The default
            // 6.35 mm end mill is outside it, so the drill cells use 6.0 mm.
            // Before B5 the plywood cells shipped the unjudged formula.
            if is_drill && tool_type == ToolType::EndMill {
                tool.diameter = 6.0;
            }
            tool.stickout = 3.0 * tool.diameter;
            for material in &materials {
                let Some(dialled) = suggest(op, &tool, material, &machine(0.85)) else {
                    continue;
                };
                let full = suggest(op, &tool, material, &machine(1.0)).unwrap();
                let expected = if is_drill {
                    AggressivenessSkip::Drill
                } else {
                    AggressivenessSkip::FinishRole
                };
                assert_eq!(
                    not_applied(&dialled.warnings),
                    Some(expected),
                    "{op:?} x {tool_type:?}: {:?}",
                    dialled.warnings
                );
                assert!(dial(&dialled.warnings).is_none());
                assert_eq!(
                    dialled.operation.depth_per_pass(),
                    full.operation.depth_per_pass()
                );
                assert_eq!(dialled.operation.stepover(), full.operation.stepover());
                assert_eq!(dialled.operation.feed_rate(), full.operation.feed_rate());
                if is_drill {
                    drill += 1;
                } else {
                    finish += 1;
                }
            }
        }
    }
    assert!(finish > 0, "non-vacuity: no Finish cell shipped a recipe");
    assert!(drill > 0, "non-vacuity: no drill cell shipped a recipe");
    assert_eq!(
        AggressivenessSkip::FinishRole.card_text(),
        "Finish: no dial action; deflection decides."
    );
}

/// (e) a 7.5 x D tool: the share lowers the target, not the feed.
#[test]
fn a_long_tool_lowers_the_target_not_the_feed_fm7() {
    let long = pocket(0.85, LONG_STICKOUT_MM);
    let short = pocket(0.85, SHORT_STICKOUT_MM);
    let d = dial(&long.warnings).unwrap_or_else(|| panic!("{:?}", long.warnings));
    assert_eq!(d.ld_factor, 0.75);
    assert!(
        (d.target_share - 0.6375).abs() < 1e-12,
        "k_eff = 0.85 x 0.75 = 0.6375, got {}",
        d.target_share
    );
    assert_eq!(long.feeds_result.derates.ld_overhang, 0.75);
    assert!(
        (long.feeds_result.feed_rate_mm_min - short.feeds_result.feed_rate_mm_min).abs()
            <= short.feeds_result.feed_rate_mm_min * REL_EPS,
        "the long-tool share multiplied the feed: {} vs {}",
        long.feeds_result.feed_rate_mm_min,
        short.feeds_result.feed_rate_mm_min
    );
    assert!(
        long.feeds_result
            .warnings
            .iter()
            .any(|w| matches!(w, FeedsWarning::LongToolDerate { factor, .. } if *factor == 0.75)),
        "the share is on the record: {:?}",
        long.feeds_result.warnings
    );
    if let (Some(f0), Some(f1)) = d.force {
        assert!(f1 <= 0.6375 * f0 * (1.0 + REL_EPS) || !d.target_met);
    }
}

fn paired(a: Option<f64>, b: Option<f64>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.is_finite() && b.is_finite(),
        (None, None) => true,
        _ => false,
    }
}

fn finite(v: &[f64]) -> bool {
    v.iter().all(|x| x.is_finite())
}

/// `None` when the warning is well formed, else the defect.
fn feeds_warning_defect(w: &FeedsWarning) -> Option<&'static str> {
    let ok = match w {
        FeedsWarning::FeedRateClamped { requested, actual } => finite(&[*requested, *actual]),
        FeedsWarning::RpmLoweredForFeedCeiling {
            rpm_from,
            rpm_to,
            feed_ceiling_mm_min,
            rpm_floor,
            ..
        } => finite(&[*rpm_from, *rpm_to, *feed_ceiling_mm_min, *rpm_floor]) && rpm_to < rpm_from,
        FeedsWarning::PowerLimited {
            required_kw,
            available_kw,
        } => finite(&[*required_kw, *available_kw]),
        FeedsWarning::PowerLadderReducedCut {
            rpm_from,
            rpm_to,
            axial_from,
            axial_to,
            radial_from,
            radial_to,
            feed_factor,
            required_kw_before,
            required_kw_after,
            available_kw,
        } => {
            paired(*rpm_from, *rpm_to)
                && paired(*axial_from, *axial_to)
                && paired(*radial_from, *radial_to)
                && feed_factor.is_none_or(|f| f > 0.0 && f <= 1.0)
                && finite(&[*required_kw_before, *required_kw_after, *available_kw])
        }
        FeedsWarning::ShankTooLarge { shank_mm, max_mm } => finite(&[*shank_mm, *max_mm]),
        FeedsWarning::DocExceedsFlute { requested, capped } => finite(&[*requested, *capped]),
        FeedsWarning::SlottingDetected { doc_reduced_to } => doc_reduced_to.is_finite(),
        FeedsWarning::ScallopInvalid {
            target,
            max_possible,
        } => finite(&[*target, *max_possible]),
        FeedsWarning::ChiploadBelowRubbingFloor {
            commanded, floor, ..
        } => finite(&[*commanded, *floor]),
        FeedsWarning::LongToolDerate {
            stickout_mm,
            diameter_mm,
            ratio,
            factor,
        } => finite(&[*stickout_mm, *diameter_mm, *ratio]) && *factor > 0.0 && *factor < 1.0,
        FeedsWarning::VendorRowPublishesNoChipload {
            formula_chipload_mm,
            ..
        } => formula_chipload_mm.is_finite(),
        FeedsWarning::NoVendorRowsForRoutedOperation { .. } => true,
        FeedsWarning::DrillFeedClampedToEnvelope {
            requested,
            actual,
            envelope_lo,
            envelope_hi,
        } => finite(&[*requested, *actual, *envelope_lo, *envelope_hi]),
    };
    (!ok).then_some("a FeedsWarning pair or factor is missing or not finite")
}

/// `None` when the record is well formed, else the defect.
fn suggest_warning_defect(w: &SuggestWarning) -> Option<&'static str> {
    let ok = match w {
        SuggestWarning::PlungeClampedToFeed { requested, capped }
        | SuggestWarning::StepoverClampedToToolDiameter { requested, capped }
        | SuggestWarning::RoughingDepthClampedToRigidity { requested, capped }
        | SuggestWarning::DepthClampedToCuttingLength { requested, capped } => {
            finite(&[*requested, *capped])
        }
        SuggestWarning::PlungeEntryUnstableAtDpp { dpp_mm, .. }
        | SuggestWarning::DeflectionBackoffUnmodeled { dpp_mm, .. }
        | SuggestWarning::DeflectionBackoffFigureIsAFloor { dpp_mm, .. } => dpp_mm.is_finite(),
        SuggestWarning::DppCappedByDeflection {
            requested_mm,
            capped_mm,
            ..
        } => finite(&[*requested_mm, *capped_mm]),
        SuggestWarning::StepoverRaisedForRuntime {
            requested_mm,
            raised_mm,
            ..
        } => finite(&[*requested_mm, *raised_mm]),
        SuggestWarning::FeedRaisedForChipload {
            requested_mm_per_min,
            raised_mm_per_min,
            ..
        } => finite(&[*requested_mm_per_min, *raised_mm_per_min]),
        SuggestWarning::ChiploadStillLowAfterRecalibration { .. } => true,
        SuggestWarning::StrategyRewrote { .. }
        | SuggestWarning::StrategyRecommendedNotApplied { .. } => true,
        SuggestWarning::AxialEnvelopeSafeBandEmpty { .. } => true,
        SuggestWarning::AxialDocClampedByEnvelope {
            commanded_mm,
            clamped_mm,
            ..
        } => finite(&[*commanded_mm, *clamped_mm]),
        SuggestWarning::AxialDocBelowBurnFloor {
            commanded_mm,
            floor_mm,
            ..
        } => finite(&[*commanded_mm, *floor_mm]),
        SuggestWarning::ProjectCurveDepthInfeasible { .. }
        | SuggestWarning::FinishEnvelopeAdvisory { .. } => true,
        SuggestWarning::FeedRescaledToFinalGeometry {
            requested_mm_per_min,
            rescaled_mm_per_min,
            factor_at_calculator,
            factor_at_final,
            ..
        } => {
            finite(&[*requested_mm_per_min, *rescaled_mm_per_min])
                && *factor_at_calculator > 0.0
                && *factor_at_final > 0.0
        }
        SuggestWarning::FeedClampedToChiploadFloor {
            requested_mm_per_tooth,
            floor_mm_per_tooth,
            ..
        } => finite(&[*requested_mm_per_tooth, *floor_mm_per_tooth]),
        SuggestWarning::PowerRecheckedAfterRescale {
            rescaled_mm_per_min,
            shipped_mm_per_min,
            required_kw_at_rescaled,
            available_kw,
            ..
        } => finite(&[
            *rescaled_mm_per_min,
            *shipped_mm_per_min,
            *required_kw_at_rescaled,
            *available_kw,
        ]),
        SuggestWarning::CutGeometryFieldNotHeld { recommended_mm, .. } => {
            recommended_mm.is_finite()
        }
        SuggestWarning::EngagementReducedForAggressiveness {
            aggressiveness,
            ld_factor,
            target_share,
            scale,
            dpp_from,
            dpp_to,
            stepover_from,
            stepover_to,
            force_n_before,
            force_n_after,
            power_kw_before,
            power_kw_after,
            section_mm2_before,
            section_mm2_after,
            target_met,
            shortfall,
            applied: _,
        } => {
            paired(*dpp_from, *dpp_to)
                && paired(*stepover_from, *stepover_to)
                && paired(*force_n_before, *force_n_after)
                && paired(*power_kw_before, *power_kw_after)
                && paired(*section_mm2_before, *section_mm2_after)
                && (target_share - aggressiveness * ld_factor).abs() < 1e-12
                && scale.is_finite()
                && *scale >= 0.0
                && (*target_met == shortfall.is_none())
        }
        SuggestWarning::RpmLoweredForFeedCeiling {
            rpm_from,
            rpm_to,
            feed_ceiling_mm_min,
            rpm_floor,
            ..
        } => finite(&[*rpm_from, *rpm_to, *feed_ceiling_mm_min, *rpm_floor]) && rpm_to < rpm_from,
        SuggestWarning::AggressivenessNotApplied { aggressiveness, .. } => {
            aggressiveness.is_finite()
        }
        SuggestWarning::RampFeed {
            from_mm_min,
            record,
        } => from_mm_min.is_none_or(f64::is_finite) && record.value().is_none_or(f64::is_finite),
    };
    (!ok).then_some("a SuggestWarning pair or factor is missing or not finite")
}

/// (f) over the FM0 grid: every pair is whole and every factor is stated.
#[test]
fn no_warning_carries_half_a_pair_fm7() {
    let materials = [
        hardwood(),
        Material::SolidWood {
            species: WoodSpecies::GenericSoftwood,
        },
        Material::SheetGood {
            kind: SheetGoodKind::Mdf,
        },
        Material::Plywood {
            grade: PlywoodGrade::BalticBirch,
        },
    ];
    let m = MachineProfile::generic_wood_router();
    let (mut cells, mut dial_records) = (0usize, 0usize);
    for &op in OperationType::ALL {
        for &tool_type in ToolType::ALL {
            let tool = ToolConfig::new_default(ToolId(0), tool_type);
            for material in &materials {
                let Some(s) = suggest(op, &tool, material, &m) else {
                    continue;
                };
                cells += 1;
                for w in &s.feeds_result.warnings {
                    if let Some(defect) = feeds_warning_defect(w) {
                        panic!("{op:?} x {tool_type:?}: {defect}: {w:?}");
                    }
                }
                for w in &s.warnings {
                    if let Some(defect) = suggest_warning_defect(w) {
                        panic!("{op:?} x {tool_type:?}: {defect}: {w:?}");
                    }
                    if matches!(w, SuggestWarning::EngagementReducedForAggressiveness { .. }) {
                        dial_records += 1;
                    }
                }
            }
        }
    }
    assert!(cells > 0, "non-vacuity: no cell shipped a recipe");
    assert!(
        dial_records > 0,
        "non-vacuity: the default dial 0.85 filed no record on {cells} cells"
    );
}
