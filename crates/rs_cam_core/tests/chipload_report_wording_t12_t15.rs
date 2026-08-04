//! **T1.2 / T1.5 / T1.6 — what the chipload report actually says.**
//!
//! Census `planning/review_2026-08-04/FEEDS_CENSUS.md` tier-1 items,
//! ruled at Checkpoint B Q2. Companion to
//! `tests/feed_explanation_record_t1.rs`, which pins the record; this
//! file pins the **surfaces that read it**.
//!
//! ## The three defects, as reported
//!
//! - **T1.2.** `"Chipload within band (0.0007 mm/tooth)"` named neither
//!   the statistic nor the unit, and read as the same quantity as the
//!   commanded feed-per-tooth an operator had set. It is not: it is an
//!   arc-mean CHIP thickness renormalised to the matched row's nominal
//!   arc and evaluated at the kinematically-predicted feed. The live
//!   validation of 2026-07-30 filed exactly that confusion and could not
//!   tell it from a real pass.
//! - **T1.5** (census P-10). The commanded feed-per-tooth vs the matched
//!   band maximum is the ONLY same-unit, same-stage comparison the
//!   pipeline can make. It was computed at `narrate.rs:426` and thrown
//!   away. On the live operation it read **7.8×**.
//! - **T1.6.** A row scaled 0.99× and a row scaled 0.38× both reported
//!   as `vendor_lut`; a `semi_finish` row winning a `finish` query was
//!   invisible.
//!
//! Nothing here moves a verdict, a threshold or a severity. The T1.5
//! diagnostic is `Info` per the standing B6 ruling and deliberately
//! supersedes nothing — it reports a commanded value against an authored
//! band and observes nothing about the cut, so it must not silence a
//! sim-backed gate.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::diagnostics::adapters::from_tool_load::diagnostics_from_load_verdict;
use rs_cam_core::diagnostics::{Severity, ids};
use rs_cam_core::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine_kinematics::PredictedFeedMap;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::simulation_cut::{
    CutKinematics, Engagement, SimulationCutSample, SimulationCutTrace,
};
use rs_cam_core::tool::{TaperedBallEndmill, ToolDefinition};
use rs_cam_core::tool_load::verdict::ToolpathLoadVerdict;
use rs_cam_core::tool_load::{ToleranceBands, ToolpathLoadContext, evaluate_toolpath};

const TIP_DIAMETER_MM: f64 = 1.0;
const TAPER_HALF_ANGLE_DEG: f64 = 5.26;
const SHANK_DIAMETER_MM: f64 = 6.0;
const RPM: u32 = 18_000;
const FLUTES: u32 = 2;
const COMMANDED_FEED_MM_MIN: f64 = 0.0714 * RPM as f64 * FLUTES as f64;
const PREDICTED_FEED_FRACTION: f64 = 0.128_2;
const SAMPLE_AXIAL_DOC_MM: f64 = 0.30;
const SAMPLE_ARC_RAD: f64 = 1.2;
const TOOLPATH: ToolpathId = ToolpathId(0);

fn tool() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(TaperedBallEndmill::new(
            TIP_DIAMETER_MM,
            TAPER_HALF_ANGLE_DEG,
            SHANK_DIAMETER_MM,
            20.0,
        )),
        SHANK_DIAMETER_MM,
        30.0,
        20.0,
        40.0,
        FLUTES,
        ToolMaterial::Carbide,
    )
}

fn verdict() -> ToolpathLoadVerdict {
    let tool = tool();
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    let commanded_fpt = COMMANDED_FEED_MM_MIN / (f64::from(RPM) * f64::from(FLUTES));
    let chip = rs_cam_core::dexel_stock::effective_chip_thickness_mm(
        &tool,
        SAMPLE_AXIAL_DOC_MM,
        Some(SAMPLE_ARC_RAD),
        commanded_fpt,
        FLUTES,
    )
    .expect("the production chip model resolves at this DOC / arc");

    let samples: Vec<SimulationCutSample> = (0..12)
        .map(|i| SimulationCutSample {
            toolpath_id: TOOLPATH,
            move_index: i + 1,
            sample_index: i,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: CutKinematics::Linear,
            feed_rate_mm_min: COMMANDED_FEED_MM_MIN,
            spindle_rpm: RPM,
            flute_count: FLUTES,
            axial_doc_mm: SAMPLE_AXIAL_DOC_MM,
            axial_engagement_mm: SAMPLE_AXIAL_DOC_MM,
            arc_engagement_radians: Some(SAMPLE_ARC_RAD),
            chipload_mm_per_tooth: commanded_fpt,
            effective_chip_thickness_mm: Some(chip),
            engagement: Engagement::with_radial_woc(0.4),
            removed_volume_est_mm3: 0.1,
            mrr_mm3_s: 1.0,
            ..SimulationCutSample::test_fixture()
        })
        .collect();

    let mut predicted_feeds = PredictedFeedMap::new();
    for s in &samples {
        predicted_feeds.insert(
            (s.toolpath_id, s.move_index),
            COMMANDED_FEED_MM_MIN * PREDICTED_FEED_FRACTION,
        );
    }
    let trace = SimulationCutTrace {
        sample_step_mm: 1.0,
        samples,
        predicted_feeds,
        ..SimulationCutTrace::test_fixture()
    };

    evaluate_toolpath(
        &ToolpathLoadContext {
            toolpath_id: TOOLPATH,
            tool: &tool,
            material: &material,
            operation_family: LutOperationFamily::Scallop,
            pass_role: LutPassRole::Finish,
            operation_feed_rate_mm_min: COMMANDED_FEED_MM_MIN,
            operation_kind: OperationType::Scallop,
            spans: None,
            drill_op: None,
        },
        Some(&trace),
        None,
        &ToleranceBands::default(),
    )
}

#[test]
fn the_chipload_message_names_its_statistic_unit_and_multipliers() {
    // T1.2. The bar the census set: "assert no bare `chipload` label
    // remains in the touched path".
    let diagnostics = diagnostics_from_load_verdict(&verdict());
    let chipload = diagnostics
        .iter()
        .find(|d| {
            d.id.as_str() == ids::LOAD_CHIPLOAD_WITHIN
                || d.id.as_str() == ids::LOAD_CHIPLOAD_LOW
                || d.id.as_str() == ids::LOAD_CHIPLOAD_HIGH
        })
        .expect("the fixture must produce a chipload diagnostic");
    eprintln!("\n  chipload message:\n    {}\n", chipload.message);

    assert!(
        chipload.message.contains("chip thickness"),
        "the message must name the quantity as a CHIP THICKNESS, not a bare \
         \"chipload\": {}",
        chipload.message
    );
    assert!(
        chipload.message.contains("median of steady-state samples")
            || chipload.message.contains("peak steady-state sample"),
        "the message must name WHICH order statistic it quotes: {}",
        chipload.message
    );
    assert!(
        chipload.message.contains("arc factor"),
        "the message must name the arc-normalisation multiplier separating it \
         from the commanded value: {}",
        chipload.message
    );
    assert!(
        chipload.message.contains("achieved/commanded feed"),
        "the message must name the achieved-feed multiplier: {}",
        chipload.message
    );
    assert!(
        chipload.message.contains("commanded"),
        "the message must quote the commanded feed-per-tooth so the two \
         numbers appear together: {}",
        chipload.message
    );
}

#[test]
fn the_commanded_over_band_alarm_is_surfaced() {
    // T1.5 / census P-10 — the comparison that was computed and dropped.
    let diagnostics = diagnostics_from_load_verdict(&verdict());
    let alarm = diagnostics
        .iter()
        .find(|d| d.id.as_str() == ids::LOAD_CHIPLOAD_COMMANDED_ABOVE_BAND)
        .expect(
            "the fixture reproduces the live operation, which ran well above its \
             matched band on the COMMANDED axis — that must now produce a finding",
        );
    eprintln!("  commanded-vs-band message:\n    {}\n", alarm.message);
    assert_eq!(
        alarm.severity,
        Severity::Info,
        "B6's Info ruling stands: this reports a commanded value against an \
         authored band and observes nothing about the cut"
    );
    assert!(
        alarm.supersedes.is_empty(),
        "this report must silence nothing — it is not sim-backed evidence"
    );
    assert!(
        alarm.message.contains("same unit, same stage"),
        "the message must say WHY this comparison is legitimate, since every \
         neighbouring one is not: {}",
        alarm.message
    );
    assert!(
        alarm.evidence.is_some(),
        "the alarm must cite the band it compared against"
    );
}

#[test]
fn the_alarm_is_absent_when_the_commanded_feed_sits_inside_the_band() {
    // Non-vacuity for T1.5: the finding must be a measurement, not a
    // constant. Same fixture, commanded feed scaled down so the ratio
    // falls below 1.0 — the diagnostic must disappear.
    let mut v = verdict();
    if let Some(e) = v.feed_explanation.as_deref_mut() {
        // Drive the commanded stage under the band maximum, leaving
        // every other stage untouched.
        e.commanded.feed_per_tooth_mm = e.band.max_mm_per_tooth * 0.5;
    }
    let diagnostics = diagnostics_from_load_verdict(&v);
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.id.as_str() == ids::LOAD_CHIPLOAD_COMMANDED_ABOVE_BAND),
        "a commanded feed-per-tooth inside the band must produce NO alarm — \
         otherwise the finding is a constant, not a measurement"
    );
}

#[test]
fn every_chipload_message_discloses_row_scaling_and_pass_role() {
    // T1.6. Pre-fix only `is_extrapolated` hinted at scaling, and pass
    // role substitution was entirely invisible.
    let diagnostics = diagnostics_from_load_verdict(&verdict());
    let chipload = diagnostics
        .iter()
        .find(|d| {
            d.id.as_str() == ids::LOAD_CHIPLOAD_WITHIN
                || d.id.as_str() == ids::LOAD_CHIPLOAD_LOW
                || d.id.as_str() == ids::LOAD_CHIPLOAD_HIGH
        })
        .unwrap();
    assert!(
        chipload.message.contains("row "),
        "the message must name the matched row: {}",
        chipload.message
    );
    assert!(
        chipload.message.contains("scaled"),
        "this fixture's Ø1-tip tapered ball is far from the row's calibrated \
         diameter, so the scaling must be disclosed: {}",
        chipload.message
    );
    assert!(
        chipload.message.contains("pass role"),
        "the fixture's SemiFinish row won a Finish query — that substitution \
         must be disclosed: {}",
        chipload.message
    );
}

#[test]
fn a_drill_verdict_produces_neither_a_record_nor_an_alarm() {
    // The negative, at the adapter boundary: drill ops have no
    // continuous chipload, so there is nothing to explain and nothing
    // to compare. A fabricated finding here would be worse than none.
    let tool = tool();
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    let v = evaluate_toolpath(
        &ToolpathLoadContext {
            toolpath_id: TOOLPATH,
            tool: &tool,
            material: &material,
            operation_family: LutOperationFamily::Drill,
            pass_role: LutPassRole::Roughing,
            operation_feed_rate_mm_min: COMMANDED_FEED_MM_MIN,
            operation_kind: OperationType::Drill,
            spans: None,
            drill_op: None,
        },
        None,
        None,
        &ToleranceBands::default(),
    );
    assert!(v.feed_explanation.is_none());
    let diagnostics = diagnostics_from_load_verdict(&v);
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.id.as_str() == ids::LOAD_CHIPLOAD_COMMANDED_ABOVE_BAND),
        "a drill verdict must produce no commanded-vs-band alarm"
    );
}
