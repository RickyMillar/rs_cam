//! **A recipe below the rubbing floor READS "below the rubbing floor", and
//! a recipe above the band reads Exceeds.** Re-blessed for ruling R4 WP2a
//! and WP2b.
//!
//! Until 2026-09-23 this file pinned the other side of the floor lift
//! (Checkpoint K (c2) and (d2)). Suggest's Step-9b clamp parked a sub-Ø2
//! tool's advance exactly on the band ceiling, and the gate reported that
//! recipe CLAMPED, not EXCEEDED. Ruling R4 WP2a
//! (`planning/feeds_matrix_2026-09-23/R4_AGGRESSIVENESS_SPEC.md` §3.6)
//! removed the lift, so no recipe is clamped. WP2b deleted the machinery
//! that explained the lift: `ClampReason`, `CommandedStage::clamped_to`,
//! `recipe_parked_by_rubbing_floor` and the gate's ceiling advisory. The
//! tests of the advisory precondition and of the clamp identifier went
//! with them.
//!
//! What stays, and why:
//!
//! 1. **Below the floor** (the calculator on an explicit synthetic sub-floor
//!    row, the `sub_floor_lut()` pattern of 827f383e): the recipe ships its
//!    computed advance, and the feeds adapter renders the Caution
//!    `feeds.chipload_below_floor` "Advance per tooth below the rubbing
//!    floor ... The feed is not raised". This is the operator's only record
//!    of a sub-floor recipe, so it must not read as a clamp.
//! 2. **Above the band** (the gate on the shipped row): a feed 5 % over the
//!    band maximum reads `Exceeds(High)` and renders as too high; a feed ON
//!    the ceiling reads `Within`. This keeps the boundary contract: a real
//!    exceedance trips, and a feed on the bound does not.
//!
//! Ruling R4 Q9 (2026-09-24) moved the floor to `min(0.025, band min)`. The
//! synthetic band is 0.00378–0.00756, so the computed advance is inside the
//! band and does not warn. Arm 1 therefore runs on a machine with a
//! 50 mm/min cutting ceiling. That puts the advance under the band minimum
//! (50 / (2 x 8000 rpm) = 0.0031 at the lowest RPM of the generic router,
//! and less at any higher RPM).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::diagnostics::adapters::from_feeds::diagnostics_from_feeds_result;
use rs_cam_core::diagnostics::ids;
use rs_cam_core::feeds::vendor_lut::{
    EvidenceGrade, LutOperationFamily, LutPassRole, ObservationKind, VendorLut,
};
use rs_cam_core::feeds::{
    FeedsInput, FeedsResult, OperationFamily, PassRole, RUBBING_FLOOR_MM_TOOTH, SetupContext,
    SpindleStrategy, ToolGeometryHint, calculate, embedded_vendor_lut,
};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine::MachineProfile;
use rs_cam_core::machine::kinematics::PredictedFeedMap;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::stock::simulation_cut::{
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

/// The same evaluation, rendered through the **core diagnostics
/// adapter** — the one surface the CLI report and MCP `get_diagnostics`
/// both read. Rule 3 (render before verdict): the point is what an
/// operator SEES, so the test prints it.
fn rendered_diagnostics(feed: f64) -> Vec<String> {
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
    rs_cam_core::diagnostics::adapters::from_tool_load::diagnostics_from_load_verdict(&full)
        .into_iter()
        .map(|d| format!("[{:?}] {:?}: {}", d.severity, d.id, d.message))
        .collect()
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

/// A one-row LUT whose derated band sits wholly below the 0.025 mm/tooth
/// floor: the printed Onsrud 77-100 1/8 in scallop row, cloned, re-labelled
/// derived/c, with the ledger's B3 band (0.00378-0.00756 mm/tooth) as its
/// bounds. Same fixture as `rubbing_floor_warns_and_never_lifts.rs`.
fn sub_floor_lut() -> VendorLut {
    let mut row = embedded_vendor_lut()
        .observations
        .iter()
        .find(|o| o.observation_id == "onsrud-hardwood-77-100-1_8-scallop")
        .expect("the printed Onsrud 77-100 scallop row exists")
        .clone();
    row.observation_id = "synthetic-b3-sub-floor-scallop".to_owned();
    row.diameter_mm = Some(1.0);
    row.flute_count = 2;
    row.chipload_min_mm_tooth = Some(0.003_78);
    row.chipload_max_mm_tooth = Some(0.007_56);
    row.row_kind = ObservationKind::Derived;
    row.evidence_grade = EvidenceGrade::C;
    row.notes = Some(
        "test fixture: the ledger B3 band on a cloned printed row; not a vendor figure".to_owned(),
    );
    VendorLut {
        observations: vec![row],
    }
}

/// The B3 scallop through the calculator, on the sub-floor LUT, on a machine
/// whose 50 mm/min cutting ceiling pushes the advance under the band minimum.
fn b3_recipe() -> FeedsResult {
    let lut = sub_floor_lut();
    let mut machine = MachineProfile::generic_wood_router();
    machine.max_cutting_feed_mm_min = Some(50.0);
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    calculate(&FeedsInput {
        tool_diameter: 1.0,
        flute_count: FLUTES,
        flute_length: 20.0,
        shank_diameter: Some(6.0),
        tool_geometry: ToolGeometryHint::TaperedBall {
            tip_radius: 0.5,
            taper_angle_deg: 5.26,
        },
        material: &material,
        machine: &machine,
        operation: OperationFamily::Scallop,
        operation_kind: None,
        pass_role: PassRole::Finish,
        axial_depth_mm: Some(AXIAL_DOC),
        radial_width_mm: None,
        target_scallop_mm: Some(0.01),
        vendor_lut: Some(&lut),
        setup: SetupContext::default(),
        spindle_strategy: SpindleStrategy::MatchChart,
    })
}

/// Arm 1. The band minimum of the fixture is below 0.025, so the floor is the
/// band minimum (ruling R4 Q9); the 50 mm/min cutting ceiling takes the
/// advance under it, so the recipe warns.
#[test]
fn a_recipe_below_the_floor_reads_below_the_floor_and_is_not_clamped() {
    let result = b3_recipe();
    let band = result
        .chipload_bounds
        .expect("the synthetic row publishes both limits");
    assert!(
        band.max_mm_per_tooth < RUBBING_FLOOR_MM_TOOTH,
        "fixture precondition: the derated band max {:.6} sits below the floor",
        band.max_mm_per_tooth
    );
    let fpt = result.feed_rate_mm_min / (result.rpm * f64::from(FLUTES));
    assert!(
        result.feed_rate_mm_min <= 50.0 + 1e-9,
        "the feed {:.3} must stay at or under the 50 mm/min ceiling: no lift",
        result.feed_rate_mm_min
    );
    assert!(
        fpt < band.min_mm_per_tooth,
        "precondition: the advance {fpt:.9} must sit under the band minimum {:.9}",
        band.min_mm_per_tooth
    );

    let rendered: Vec<String> = diagnostics_from_feeds_result(ToolpathId(0), &result)
        .into_iter()
        .map(|d| format!("[{:?}] {}: {}", d.severity, d.id.as_str(), d.message))
        .collect();
    println!("=== below the floor, as the operator reads it ===");
    for line in &rendered {
        println!("{line}");
    }
    let floor_line = rendered
        .iter()
        .find(|l| l.contains(ids::FEEDS_CHIPLOAD_BELOW_FLOOR))
        .unwrap_or_else(|| panic!("no feeds.chipload_below_floor finding:\n{rendered:#?}"));
    assert!(
        floor_line.starts_with("[Caution]")
            && floor_line.contains("below the rubbing floor")
            && floor_line.contains("The feed is not raised")
            && floor_line.contains("published vendor band minimum"),
        "the finding must read as below the floor, not as a clamp: {floor_line}"
    );
    assert!(
        !floor_line.to_lowercase().contains("clamp"),
        "the floor finding must not word the recipe as clamped: {floor_line}"
    );
}

/// Arm 2. On the shipped row the gate's band is above the floor. A feed ON
/// the ceiling is `Within`; a feed 5 % over reads `Exceeds(High)` and
/// renders as an exceedance.
#[test]
fn a_recipe_above_the_band_reads_exceeds_and_the_ceiling_reads_within() {
    let band_max = gate_band_max();
    println!("gate band max on the shipped row: {band_max:.6}");

    let (on_ceiling, _) = verdict_and_explanation(band_max * divisor());
    assert!(
        matches!(on_ceiling, ChiploadVerdict::Within { .. }),
        "a feed ON the ceiling must be Within (b1): {on_ceiling:?}"
    );

    let over = rendered_diagnostics(band_max * 1.05 * divisor());
    let over_text = over.join("\n");
    println!("=== 5 % over the ceiling ===\n{over_text}");
    assert!(
        over_text.contains("too high") || over_text.contains("breakage"),
        "the genuine exceedance must read as an exceedance:\n{over_text}"
    );
    assert!(
        !over_text.contains("CLAMPED"),
        "a real exceedance must never be worded as a clamp:\n{over_text}"
    );
    assert!(matches!(
        verdict_and_explanation(band_max * 1.05 * divisor()).0,
        ChiploadVerdict::Exceeds {
            side: ChipSide::High,
            ..
        }
    ));
}
