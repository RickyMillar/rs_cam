//! **Checkpoint K (c2) + (d2) — a recipe the engine's own clamp parked
//! on the band ceiling reports CLAMPED, not EXCEEDED, and says so on the
//! record.**
//!
//! The scenario is A-6's rider 1, unchanged and still true: on a sub-Ø2
//! tool the whole derated chipload band can sit below the 0.025 mm/tooth
//! chip-formation floor, so `feeds::effective_rubbing_floor` returns the
//! band **maximum** and Suggest's Step-9b clamp parks the commanded
//! advance exactly on the breakage-side bound. That ruling (2026-08-06)
//! was correct — clamping *up* to the global floor was measured at 3.47×
//! the band maximum — and it is what made the boundary comparison
//! load-bearing.
//!
//! What Checkpoint K changed is the reporting:
//!
//! - **(b1)** the comparison absorbs the multiply→divide reconstruction,
//!   so the verdict stops flipping on the last bit
//!   (`chipload_boundary_g_chip_ulp.rs`);
//! - **(c2)** the `Within` arm carries a `ceiling_advisory` saying the
//!   recipe is *on* the ceiling because the engine put it there;
//! - **(d2)** `FeedExplanation`'s commanded stage carries `clamped_to`,
//!   naming the clamp in the record built for exactly this.
//!
//! The two conditions are asserted **separately** below: proximity alone
//! must not produce the advisory, and a genuine exceedance must not be
//! demoted by it. That is the whole reason (c2) is only correct with
//! (b1).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use rs_cam_core::feeds::{
    ChiploadBounds, ClampReason, RUBBING_FLOOR_MM_TOOTH, effective_rubbing_floor,
    recipe_parked_by_rubbing_floor,
};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine_kinematics::PredictedFeedMap;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::simulation_cut::{
    Engagement, SimulationCutSample, SimulationCutSummary, SimulationCutTrace,
};
use rs_cam_core::tool::{TaperedBallEndmill, ToolDefinition};
use rs_cam_core::tool_load::verdict::{ChipSide, ChiploadVerdict};
use rs_cam_core::tool_load::{ToleranceBands, ToolpathLoadContext};

const RPM: u32 = 18_000;
const FLUTES: u32 = 2;
const AXIAL_DOC: f64 = 0.35;

fn b3_tool() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(TaperedBallEndmill::new(1.0, 5.26, 6.0, 20.0)),
        6.0,
        30.0,
        20.0,
        40.0,
        FLUTES,
        ToolMaterial::Carbide,
    )
}

fn trace(feed: f64) -> SimulationCutTrace {
    let id = ToolpathId(0);
    let mut predicted_feeds = PredictedFeedMap::new();
    predicted_feeds.insert((id, 0), feed);
    SimulationCutTrace {
        predicted_feeds,
        sample_step_mm: 1.0,
        summary: SimulationCutSummary {
            sample_count: 1,
            toolpath_count: 1,
            cutting_runtime_s: 1.0,
            total_runtime_s: 1.0,
            average_engagement: 0.5,
            peak_axial_doc_mm: AXIAL_DOC,
            ..SimulationCutSummary::default()
        },
        samples: vec![SimulationCutSample {
            toolpath_id: id,
            move_index: 0,
            sample_index: 0,
            segment_time_s: 0.1,
            is_cutting: true,
            feed_rate_mm_min: feed,
            spindle_rpm: RPM,
            flute_count: FLUTES,
            axial_doc_mm: AXIAL_DOC,
            axial_engagement_mm: AXIAL_DOC,
            arc_engagement_radians: Some(1.0),
            effective_chip_thickness_mm: Some(0.01),
            engagement: Engagement::with_radial_woc(0.5),
            removed_volume_est_mm3: 0.1,
            mrr_mm3_s: 1.0,
            ..SimulationCutSample::test_fixture()
        }],
        ..SimulationCutTrace::test_fixture()
    }
}

fn verdict_and_explanation(
    feed: f64,
) -> (
    ChiploadVerdict,
    Option<Box<rs_cam_core::feeds::FeedExplanation>>,
) {
    let tool = b3_tool();
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    let t = trace(feed);
    let tolerance = ToleranceBands::default();
    let full = rs_cam_core::tool_load::evaluate_toolpath(
        &ToolpathLoadContext {
            toolpath_id: ToolpathId(0),
            tool: &tool,
            material: &material,
            operation_family: LutOperationFamily::Scallop,
            pass_role: LutPassRole::Finish,
            operation_feed_rate_mm_min: feed,
            operation_kind: OperationType::Scallop,
            spans: None,
            drill_op: None,
        },
        Some(&t),
        None,
        &tolerance,
    );
    (full.chipload, full.feed_explanation)
}

/// Read the gate's own band by probing it, so nothing here mirrors the
/// query construction.
fn gate_band_max() -> f64 {
    match verdict_and_explanation(100.0).0 {
        ChiploadVerdict::Within {
            approach_to_max, ..
        } => approach_to_max.bounds.max_mm_per_tooth,
        other => panic!("probe must land Within: {other:?}"),
    }
}

fn divisor() -> f64 {
    f64::from(RPM) * f64::from(FLUTES)
}

#[test]
fn the_floor_clamped_recipe_reports_clamped_not_exceeded() {
    let band_max = gate_band_max();
    assert!(
        band_max < RUBBING_FLOOR_MM_TOOTH,
        "fixture precondition: the whole derated band must sit below the {RUBBING_FLOOR_MM_TOOTH} \
         global floor for the clamp to collapse onto the ceiling; band max {band_max:.6}"
    );

    // Step-9b's own arithmetic, on the gate's own band.
    let feed = band_max * divisor();
    let (verdict, explanation) = verdict_and_explanation(feed);

    let ChiploadVerdict::Within {
        approach_to_max,
        ceiling_advisory,
        burn_advisory,
        ..
    } = &verdict
    else {
        panic!(
            "**c2 regression** — a recipe the engine's own rubbing-floor clamp parked on the \
             band ceiling must not read as an exceedance: {verdict:?}"
        );
    };
    let advisory = ceiling_advisory
        .as_deref()
        .expect("the ceiling advisory must be present on a floor-clamped recipe");
    println!(
        "c2: observed {:.17} vs ceiling {:.17} → Within + ceiling_advisory (burn_advisory {})",
        advisory.observed_mm_per_tooth,
        advisory.bounds.max_mm_per_tooth,
        burn_advisory.is_some()
    );
    assert_eq!(
        advisory.observed_mm_per_tooth, approach_to_max.observed_mm_per_tooth,
        "the advisory must carry the same reading the Within arm reports, not a second number"
    );

    // (d2) — and the record says why the commanded advance is there.
    let explanation = explanation.expect("the gate must publish a feed explanation");
    let clamp = explanation
        .commanded
        .clamped_to
        .expect("the commanded stage must record the clamp that placed this advance");
    println!("d2: commanded stage clamped_to = {}", clamp.label());
    assert!(
        clamp.parks_on_band_ceiling(),
        "the clamp must be the band-ceiling arm, not the ordinary global-floor arm: {clamp:?}"
    );
    match clamp {
        ClampReason::RubbingFloorCappedToBandCeiling {
            floor_mm_per_tooth,
            global_floor_mm_per_tooth,
        } => {
            assert!(
                (floor_mm_per_tooth - band_max).abs() < 1e-15,
                "the recorded floor must BE the band ceiling: {floor_mm_per_tooth:.17} vs \
                 {band_max:.17}"
            );
            assert_eq!(global_floor_mm_per_tooth, RUBBING_FLOOR_MM_TOOTH);
        }
        ClampReason::RubbingFloor { .. } => unreachable!("guarded above"),
    }
    assert!(
        clamp.label().contains("band ceiling"),
        "every renderer prints this string; it must name the ceiling: {}",
        clamp.label()
    );
}

/// **The precondition, asserted.** Proximity alone does not produce the
/// advisory, and a genuine exceedance is not demoted by it.
#[test]
fn the_advisory_needs_both_conditions_and_never_demotes_a_real_exceedance() {
    let band_max = gate_band_max();

    // (a) A feed well INSIDE the band: not at the ceiling, so no
    // advisory, even though the recipe would still be identified as
    // "clamped" if it sat on the floor. Half the band maximum is far
    // outside the boundary epsilon.
    let inside = verdict_and_explanation(band_max * 0.5 * divisor()).0;
    match &inside {
        ChiploadVerdict::Within {
            ceiling_advisory, ..
        } => assert!(
            ceiling_advisory.is_none(),
            "a reading at half the ceiling must NOT carry the ceiling advisory — the (c2) \
             gate is epsilon-proximity, not 'near'"
        ),
        other => panic!("a feed inside the band must be Within: {other:?}"),
    }

    // (b) A genuine 5 %-over feed still Exceeds. If (c2) had been
    // written on proximity alone, or without (b1)'s bounded epsilon,
    // this is the verdict it would have swallowed.
    let over = verdict_and_explanation(band_max * 1.05 * divisor()).0;
    assert!(
        matches!(
            over,
            ChiploadVerdict::Exceeds {
                side: ChipSide::High,
                ..
            }
        ),
        "**c2 over-reach** — a 5 %-over feed must still Exceed, got {over:?}"
    );
}

/// The post-hoc identifier is exactly `effective_rubbing_floor` plus the
/// boundary epsilon — one decision, consulted twice, so Step-9b and the
/// gate cannot drift apart.
#[test]
fn the_clamp_identifier_agrees_with_the_floor_it_reads() {
    let band = ChiploadBounds {
        min_mm_per_tooth: 0.005_762_689_177_314_92,
        max_mm_per_tooth: 0.011_525_378_354_629_83,
    };
    let floor = effective_rubbing_floor(Some(band));
    assert_eq!(floor, band.max_mm_per_tooth, "the floor==ceiling regime");

    assert!(
        recipe_parked_by_rubbing_floor(floor, Some(band))
            .is_some_and(|c| c.parks_on_band_ceiling())
    );
    // 1 ulp either side is still "on" the floor — the same absorption
    // the gate applies, so the identification cannot disagree with the
    // verdict on the very reconstruction that motivated both.
    for bits in [floor.to_bits() + 1, floor.to_bits() - 1] {
        assert!(
            recipe_parked_by_rubbing_floor(f64::from_bits(bits), Some(band)).is_some(),
            "±1 ulp off the floor must still identify as clamped"
        );
    }
    // Half the floor is not.
    assert!(recipe_parked_by_rubbing_floor(floor * 0.5, Some(band)).is_none());
    assert!(recipe_parked_by_rubbing_floor(0.0, Some(band)).is_none());

    // A band with room above the global floor gets the ordinary arm,
    // which (c2) deliberately does NOT accept.
    let roomy = ChiploadBounds {
        min_mm_per_tooth: 0.034,
        max_mm_per_tooth: 0.059,
    };
    let roomy_floor = effective_rubbing_floor(Some(roomy));
    assert_eq!(roomy_floor, RUBBING_FLOOR_MM_TOOTH);
    let reason = recipe_parked_by_rubbing_floor(roomy_floor, Some(roomy))
        .expect("a recipe sitting on the global floor is still clamped");
    assert!(
        !reason.parks_on_band_ceiling(),
        "the ordinary global-floor clamp must not license the ceiling advisory: {reason:?}"
    );
}
