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
//!   commanded feed-per-tooth an operator had set. At the time it was
//!   not: it was an arc-mean CHIP thickness renormalised to the matched
//!   row's nominal arc and evaluated at the kinematically-predicted
//!   feed. The live validation of 2026-07-30 filed exactly that
//!   confusion and could not tell it from a real pass.
//! - **T1.5** (census P-10). The commanded feed-per-tooth vs the matched
//!   band maximum was the ONLY same-unit, same-stage comparison the
//!   pipeline could make. It was computed at `narrate.rs:426` and thrown
//!   away. On the live operation it read **7.8×**.
//! - **T1.6.** A row scaled 0.99× and a row scaled 0.38× both reported
//!   as `vendor_lut`; a `semi_finish` row winning a `finish` query was
//!   invisible.
//!
//! ## Re-pinned 2026-08-06 — the quantity changed, the disclosure rule did not
//!
//! `CHIPLOAD_LITERATURE_VERDICT.md` established the vendor column is a
//! linear advance per tooth, so the gate's chip-geometry stage was
//! deleted and the observation became `effective_feed / (rpm · flutes)`.
//! Two T1.2 assertions below were pinned to the OLD quantity's wording
//! and are restated, not dropped:
//!
//! - `contains("chip thickness")` → `contains("feed-per-tooth")`. The
//!   rule T1.2 encodes is *"name the quantity"*, and the quantity moved.
//! - `contains("arc factor")` → asserted **ABSENT**. A message that
//!   still named an arc-normalisation multiplier would be describing a
//!   step that no longer runs, which is the exact "stale rationale
//!   outliving the code" class the programme's P11 names. Asserting its
//!   absence is stronger than deleting the assertion.
//!
//! T1.5's `"same unit, same stage"` clause is unchanged and is now
//! *more* true, not less: the gate's own observation joined the same
//! unit. What the alarm loses is its **uniqueness** — it is no longer
//! the only legitimate comparison — which is recorded here and is why
//! `FeedExplanation::gate_over_band_max` exists beside it.
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
use rs_cam_core::machine::kinematics::PredictedFeedMap;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::stock::simulation_cut::{
    CutKinematics, Engagement, SimulationCutSample, SimulationCutTrace,
};
use rs_cam_core::tool::{TaperedBallEndmill, ToolDefinition};
use rs_cam_core::tool_load::verdict::ToolpathLoadVerdict;
use rs_cam_core::tool_load::{ToleranceBands, ToolpathLoadContext, evaluate_toolpath};

/// RE-PREMISED at extrapolation P1 step 3 (2026-09-24). The fixture ran a
/// 1.0 mm tip on a hardwood Scallop. The G1 size claim refuses that cell
/// (no Scallop row near a 1.0 mm tip), so the gate no longer judges it. The
/// fixture now runs a 1.1 mm tip on a Parallel finish (`DropCutter`), a
/// G1 form A claim: the anchor is `amana-tapered-hardwood-parallel-1000-2f-zrn-v8`
/// (score 1797, tied with the SpeTool 1.0 mm row and kept by id), and the
/// Amana v8 2-flute hardwood series brackets 1.1 mm between the 1.0 mm row
/// (mid 0.034925) and the 1.5875 mm row (mid 0.1016). With
/// `t = ln(1.1) / ln(1.5875) = 0.206396`, the scale is
/// `(0.1016 / 0.034925)^t = 1.246348491430`, so the band is
/// 0.023743-0.063315 mm/tooth (Janka 1450 on both sides, hardness x1.0).
/// The commanded 0.0714 mm/tooth is 1.128x the band maximum, so the T1.5
/// alarm still fires, and the row is scaled, so T1.6 still discloses it.
const TIP_DIAMETER_MM: f64 = 1.1;
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
            operation_family: LutOperationFamily::Parallel,
            pass_role: LutPassRole::Finish,
            operation_feed_rate_mm_min: COMMANDED_FEED_MM_MIN,
            operation_kind: OperationType::DropCutter,
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

    // Case-insensitive: the same clause appears sentence-initial on the
    // trip arm ("Feed-per-tooth too high ...") and mid-sentence on the
    // Within arm ("Observed feed-per-tooth within band ...").
    assert!(
        chipload.message.to_lowercase().contains("feed-per-tooth"),
        "the message must name the quantity as a FEED-PER-TOOTH (linear advance), \
         not a bare \"chipload\": {}",
        chipload.message
    );
    // RESTATED 2026-08-06 (was: must CONTAIN "chip thickness"). The gate
    // stopped observing a chip thickness; a message that still said so
    // would be a stale rationale outliving the code.
    assert!(
        !chipload.message.to_lowercase().contains("chip thickness"),
        "the message must NOT name a chip thickness — the gate has not observed \
         one since the 2026-08-06 unit conversion: {}",
        chipload.message
    );
    assert!(
        chipload.message.contains("median of steady-state samples")
            || chipload.message.contains("peak steady-state sample"),
        "the message must name WHICH order statistic it quotes: {}",
        chipload.message
    );
    // RESTATED 2026-08-06 (was: must CONTAIN "arc factor"). There is no
    // arc normalisation left to name.
    assert!(
        !chipload.message.contains("arc factor"),
        "the message must NOT name an arc-normalisation multiplier — D9 is \
         deleted and no such step runs: {}",
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
fn the_gate_observation_is_now_on_the_same_axis_as_the_alarm() {
    // The conversion's operator-facing payoff, asserted. Before
    // 2026-08-06 the COMMANDED-vs-band alarm and the gate's own verdict
    // were on different axes, so an operator could read "7.8x over the
    // band" and "Within" and have no way to relate them. They are now
    // the same measure at two feeds, and their ratio is the kinematic
    // throttle.
    let v = verdict();
    let e = v
        .feed_explanation
        .as_deref()
        .expect("the fixture reaches a modelled verdict");
    let commanded_ratio = e.commanded_over_band_max().expect("band max is positive");
    let achieved_ratio = e.gate_over_band_max().expect("gate observation is finite");
    eprintln!(
        "  commanded {commanded_ratio:.2}x band max -> achieved {achieved_ratio:.2}x \
         (throttle {:.4})",
        achieved_ratio / commanded_ratio
    );
    assert_eq!(
        e.commanded.unit(),
        e.gate.unit(),
        "the alarm's axis and the gate's axis must be the same unit"
    );
    assert!(
        (achieved_ratio / commanded_ratio - PREDICTED_FEED_FRACTION).abs() < 1e-6,
        "the two ratios must differ by exactly the achieved-feed ratio"
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
        "this fixture's Ø1.1-tip tapered ball sits off the row's printed \
         diameter (a G1 form A claim, x1.2463), so the scaling must be disclosed: {}",
        chipload.message
    );
    // The query is a Finish role (`verdict()`). Until R5 (2026-09-23) the
    // only tapered scallop row was SemiFinish, so the substitution had to
    // be disclosed. The printed rows that win now (Amana ZrN v8 after
    // extrapolation P1) carry `finish`, so no substitution exists. The
    // Onsrud 77-100 rows are filed under pocket/roughing since A3 step 3,
    // and a G3 family rule carries them (see the arm below). The
    // rule stays: a row of another role must say so; a row of the same
    // role must not claim a substitution. The winning row is read from the
    // message, so the arm follows the table.
    let row_id = chipload
        .message
        .split("(row ")
        .nth(1)
        .and_then(|rest| rest.split(';').next())
        .expect("the message names the matched row as `(row <id>;`");
    let row = rs_cam_core::feeds::EMBEDDED_LUT
        .observations
        .iter()
        .find(|o| o.observation_id == row_id)
        .unwrap_or_else(|| panic!("the named row {row_id:?} is not in the embedded LUT"));
    // A3 (G3): a row that a family rule carries from another operation
    // family states the rule in place of a role substitution. Since A3
    // step 3 the Onsrud 77-100 rows are filed only under pocket/roughing,
    // so an Onsrud winner here is such a row.
    if row.operation_family != LutOperationFamily::Parallel {
        assert!(
            chipload.message.contains("G3 family rule") && !chipload.message.contains("pass role"),
            "row {row_id} is filed under {:?}; the message must state the family rule and no \
             role substitution: {}",
            row.operation_family,
            chipload.message
        );
        return;
    }
    let substituted = row.pass_role != LutPassRole::Finish;
    assert_eq!(
        chipload.message.contains("pass role"),
        substituted,
        "row {row_id} has pass role {:?} against a Finish query; a substitution \
         must be disclosed and only then: {}",
        row.pass_role,
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
