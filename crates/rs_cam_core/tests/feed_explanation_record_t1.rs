//! **T1.1 — the production feed-explanation record.**
//!
//! Census `planning/review_2026-08-04/FEEDS_CENSUS.md` tier-1 item T1.1,
//! ruled at Checkpoint B Q2 ("stage-labelled explanation records ...
//! proceed now"). Reference semantics:
//! `tests/feed_explanation_snapshot_b3.rs`, the test-only assembler
//! whose five stages this record now carries in production.
//!
//! The fixture is the same shape as the B3 assembler's — the live
//! 2026-07-30 scallop operation, synthesised in-test, with a hand-built
//! trace so no generator, arc fitter, lead-in or depth planner
//! participates. `planning/airrun_2026-06-01/wanaka.toml` is NOT an
//! input (plan §2 rule 9).
//!
//! ## Re-pinned 2026-08-06 — four stages, not five
//!
//! `CHIPLOAD_LITERATURE_VERDICT.md` settled Checkpoint B item T4.1: the
//! vendor chipload column is a linear **advance per tooth** in every
//! source family in the shipped LUT, so the gate's chip-geometry stage
//! was converting a published advance into a chip nothing published. It
//! was deleted. The record's stages are now
//! `1 commanded → 2 band → 3 achieved feed → 4 gate observation`, and
//! stages 1, 2 and 4 share one unit.
//!
//! Two assertions in this file were written to be TRUE OF THE DEFECT and
//! are therefore inverted rather than deleted, so the change of contract
//! is visible in the diff instead of vanishing:
//!
//! - `the_record_never_collapses_the_stages_to_one_number` asserted
//!   `gate.unit().contains("chip")`. It now asserts the units are equal
//!   and that the stages are nonetheless distinct **numbers** — which is
//!   the honest form of the original rule: the defect was never that the
//!   units differed, it was that the record might collapse three values
//!   into one.
//! - `a_modelled_verdict_carries_a_fully_populated_explanation` asserted
//!   `assert_ne!(gate.unit(), commanded.unit())` with the message *"that
//!   they do not is the entire finding this record exists to carry"*.
//!   The finding was real and is now **fixed**, so the assertion becomes
//!   `assert_eq!` and says why.
//!
//! What these tests pin:
//!
//! 1. [`a_modelled_verdict_carries_a_fully_populated_explanation`] — all
//!    four stages present and distinctly named, which is T1.1's
//!    acceptance bar.
//! 2. [`the_record_reproduces_the_census_identity`] — stage 1 × stage 3
//!    predicts stage 4. Same identity as before, minus the term the
//!    literature verdict removed.
//! 3. [`the_record_surfaces_the_commanded_over_band_ratio`] — T1.5's
//!    number, plus its achieved-feed twin, and the throttle between them.
//! 4. [`the_record_discloses_scaling_on_an_unextrapolated_row`] — T1.6:
//!    a scaled row must be distinguishable from an unscaled one even
//!    when the ±40 % extrapolation flag is clear.
//! 5. [`a_refused_verdict_carries_no_explanation`] — the negative: an
//!    `Unmodeled` verdict has no stages, and the record must be absent
//!    rather than fabricated.
//! 6. [`the_record_never_collapses_the_stages_to_one_number`] — the
//!    rule the type is written under, asserted: stages 1, 2 and 4 are
//!    genuinely different values here.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine_kinematics::PredictedFeedMap;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::simulation_cut::{
    CutKinematics, Engagement, SimulationCutSample, SimulationCutTrace,
};
use rs_cam_core::tool::{TaperedBallEndmill, ToolDefinition};
use rs_cam_core::tool_load::{ToleranceBands, ToolpathLoadContext, evaluate_toolpath};

const TIP_DIAMETER_MM: f64 = 1.0;
const TAPER_HALF_ANGLE_DEG: f64 = 5.26;
const SHANK_DIAMETER_MM: f64 = 6.0;
const RPM: u32 = 18_000;
const FLUTES: u32 = 2;
/// Chosen so `feed / (rpm · flutes)` lands on the live narration nominal.
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

fn material() -> Material {
    Material::SolidWood {
        species: WoodSpecies::HardMaple,
    }
}

fn trace(with_predicted_feeds: bool) -> SimulationCutTrace {
    let tool = tool();
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
    if with_predicted_feeds {
        for s in &samples {
            predicted_feeds.insert(
                (s.toolpath_id, s.move_index),
                COMMANDED_FEED_MM_MIN * PREDICTED_FEED_FRACTION,
            );
        }
    }

    SimulationCutTrace {
        sample_step_mm: 1.0,
        samples,
        predicted_feeds,
        ..SimulationCutTrace::test_fixture()
    }
}

fn verdict_for(
    with_predicted_feeds: bool,
    with_trace: bool,
) -> rs_cam_core::tool_load::verdict::ToolpathLoadVerdict {
    let tool = tool();
    let material = material();
    let built = trace(with_predicted_feeds);
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
        if with_trace { Some(&built) } else { None },
        None,
        &ToleranceBands::default(),
    )
}

#[test]
fn a_modelled_verdict_carries_a_fully_populated_explanation() {
    let verdict = verdict_for(true, true);
    let explanation = verdict
        .feed_explanation
        .as_deref()
        .expect("a modelled chipload verdict must carry a feed explanation");

    // Stage 1 — commanded.
    assert!(explanation.commanded.feed_per_tooth_mm > 0.0);
    assert_eq!(explanation.commanded.spindle_rpm, RPM);
    assert_eq!(explanation.commanded.flute_count, FLUTES);
    assert_eq!(
        explanation.commanded.unit(),
        "mm of linear advance per tooth"
    );

    // Stage 2 — band + provenance.
    assert!(
        !explanation.band.observation_id.is_empty(),
        "the band stage must name the row it came from"
    );
    assert!(explanation.band.max_mm_per_tooth > 0.0);
    assert!(explanation.band.row_diameter_mm > 0.0);
    assert_eq!(explanation.band.queried_pass_role, LutPassRole::Finish);

    // Stage 2 — the row's `ae` window survives as report-only
    // provenance. It is NOT consumed by anything (verdict §2.3: on wood
    // rows these are repo-authored application windows).
    assert!(
        explanation.band.ae_window_mm.is_some(),
        "the fixture's row publishes an ae window, so the record must carry it \
         — inert, but visible"
    );

    // Stage 3 — achieved feed.
    assert!(explanation.achieved_feed.predicted_feeds_present);
    let ratio = explanation
        .achieved_feed
        .median_ratio
        .expect("a populated predicted-feed map must yield a ratio");
    assert!(
        (ratio - PREDICTED_FEED_FRACTION).abs() < 1e-6,
        "achieved/commanded ratio should be {PREDICTED_FEED_FRACTION}, got {ratio}"
    );

    // Stage 4 — gate observation.
    assert!(explanation.gate.value_mm > 0.0);
    assert!(explanation.gate.sample_count > 0);
    // INVERTED 2026-08-06. This assertion previously read `assert_ne!`
    // with the message "that they do not is the entire finding this
    // record exists to carry". The finding was correct and has been
    // fixed: the gate now observes the same quantity as stage 1 and the
    // same quantity stage 2 publishes, so a comparison between them is
    // legitimate for the first time. Restated rather than deleted,
    // because the contract change is the point.
    assert_eq!(
        explanation.gate.unit(),
        explanation.commanded.unit(),
        "since the 2026-08-06 unit conversion stage 4 and stage 1 MUST claim \
         the same unit — the gate reports the commanded quantity at the \
         achieved feed"
    );
    assert_eq!(
        explanation.gate.unit(),
        explanation.band.unit_family(),
        "and the same unit family the vendor band publishes, which is what \
         makes the verdict's own comparison valid"
    );

    eprintln!(
        "\n  stage 1 commanded fpt   {:.6} {}\n  stage 2 band            {:?} .. {:.6} ({}) row {}\n  \
         stage 2 ae window       {:?} (report-only)\n  stage 3 feed ratio      {:?}\n  stage 4 gate            {:.9} {} [{}]\n  \
         multiplier: {}",
        explanation.commanded.feed_per_tooth_mm,
        explanation.commanded.unit(),
        explanation.band.min_mm_per_tooth,
        explanation.band.max_mm_per_tooth,
        explanation.band.unit(),
        explanation.band.observation_id,
        explanation.band.ae_window_mm,
        explanation.achieved_feed.median_ratio,
        explanation.gate.value_mm,
        explanation.gate.unit(),
        explanation.gate.statistic.label(),
        explanation.multiplier_clause(),
    );
}

#[test]
fn the_record_reproduces_the_census_identity() {
    // FEEDS_CENSUS.md §4.3, minus the term the literature verdict
    // deleted: gate_observed = commanded_fpt × achieved/commanded feed.
    // The B3 assembler proved the original through a test-local copy of
    // the stages; here it is proved through the shipped record, which is
    // what makes the record trustworthy as a report rather than a
    // decoration.
    let verdict = verdict_for(true, true);
    let explanation = verdict.feed_explanation.as_deref().unwrap();
    let predicted = explanation
        .predicted_gate_observation_mm()
        .expect("the single multiplier is measured on this fixture");
    let observed = explanation.gate.value_mm;
    let residual = observed / predicted - 1.0;
    eprintln!(
        "  identity 1x3 = {predicted:.9}   observed = {observed:.9}   residual {:+.4} %",
        100.0 * residual
    );
    assert!(
        residual.abs() < 0.01,
        "the record's own stages must explain its own stage 4 to within 1 %; \
         predicted {predicted:.9}, observed {observed:.9}, residual {:+.4} %",
        100.0 * residual
    );
}

#[test]
fn the_record_surfaces_the_commanded_over_band_ratio() {
    // T1.5 / census P-10. `narrate.rs:426` computed the commanded
    // feed-per-tooth and printed it; nothing ever divided it by the band
    // it was going to be judged against. On the live operation that
    // ratio was 7.8×.
    let verdict = verdict_for(true, true);
    let explanation = verdict.feed_explanation.as_deref().unwrap();
    let ratio = explanation
        .commanded_over_band_max()
        .expect("the fixture's band has a positive maximum");
    eprintln!(
        "  commanded {:.6} mm/tooth vs band max {:.6} mm/tooth = {ratio:.2}x",
        explanation.commanded.feed_per_tooth_mm, explanation.band.max_mm_per_tooth
    );
    assert!(
        ratio > 1.0,
        "this fixture reproduces the live operation, which ran well above its \
         matched band on the commanded axis; got {ratio:.2}x"
    );
    // Both sides of this ratio must be the SAME unit, or the number is
    // meaningless.
    assert_eq!(explanation.commanded.unit(), explanation.band.unit_family());

    // And its achieved-feed twin, new on 2026-08-06. The pair, and the
    // throttle between them, is the operator-actionable statement the
    // verdict document asks for: 7.8x commanded, 1.0x achieved, -87 %
    // kinematic throttle.
    let achieved_ratio = explanation
        .gate_over_band_max()
        .expect("the gate produced a finite observation on this fixture");
    let throttle = achieved_ratio / ratio;
    eprintln!(
        "  achieved {:.6} mm/tooth vs band max = {achieved_ratio:.2}x; \
         throttle achieved/commanded = {throttle:.4}",
        explanation.gate.value_mm
    );
    assert!(
        (throttle - PREDICTED_FEED_FRACTION).abs() < 1e-6,
        "the two band ratios must differ by exactly stage 3 (the achieved-feed \
         ratio {PREDICTED_FEED_FRACTION}); got {throttle}"
    );
}

#[test]
fn the_record_discloses_scaling_on_an_unextrapolated_row() {
    // T1.6. Pre-fix the only scaling signal on a verdict was
    // `is_extrapolated`, which fires past ±40 %. A row scaled 0.99× and
    // a row scaled 0.38× both reported as plain `vendor_lut`.
    let verdict = verdict_for(true, true);
    let explanation = verdict.feed_explanation.as_deref().unwrap();
    assert!(
        explanation.band.diameter_scale > 0.0 && explanation.band.hardness_scale > 0.0,
        "both scale factors must be carried, not just the extrapolation flag"
    );
    eprintln!(
        "  row {} | d-scale x{:.4} | h-scale x{:.4} | extrapolated {} | scaled {} | \
         pass role queried {:?} vs row {:?} (substituted: {})",
        explanation.band.observation_id,
        explanation.band.diameter_scale,
        explanation.band.hardness_scale,
        explanation.band.is_extrapolated,
        explanation.band.is_scaled(),
        explanation.band.queried_pass_role,
        explanation.band.row_pass_role,
        explanation.band.pass_role_substituted(),
    );
    assert!(
        explanation.band.is_scaled(),
        "this fixture's Ø1-tip tapered ball is nowhere near the row's \
         calibrated diameter, so the band must report as scaled"
    );
}

#[test]
fn a_refused_verdict_carries_no_explanation() {
    // The negative. With no trace the chipload gate returns
    // `Unmodeled(SimulationRequired)` — there are no stages to label,
    // and a record full of zeros would be worse than no record.
    let verdict = verdict_for(false, false);
    assert!(
        verdict.chipload.is_unmodeled(),
        "the no-trace fixture must produce an Unmodeled chipload verdict"
    );
    assert!(
        verdict.feed_explanation.is_none(),
        "an Unmodeled verdict must carry no feed explanation, not a fabricated one"
    );
}

#[test]
fn the_record_never_collapses_the_stages_to_one_number() {
    // The rule the type is written under, asserted rather than asked
    // for in a comment: on a real operation the three headline stages
    // are genuinely different numbers, and the record keeps them apart.
    // Note the fixture carries a predicted-feed map, so stage 4 differs
    // from stage 1 by a MEASURED throttle; without one they would be
    // equal by construction and this test would be vacuous.
    let verdict = verdict_for(true, true);
    let e = verdict.feed_explanation.as_deref().unwrap();
    let commanded = e.commanded.feed_per_tooth_mm;
    let band_max = e.band.max_mm_per_tooth;
    let observed = e.gate.value_mm;
    assert!(
        (commanded - band_max).abs() > 1e-9
            && (commanded - observed).abs() > 1e-9
            && (band_max - observed).abs() > 1e-9,
        "stages 1 ({commanded:.9}), 2 ({band_max:.9}) and 4 ({observed:.9}) must \
         remain distinct — collapsing them is the defect"
    );
    // INVERTED 2026-08-06: stages 1 and 4 now share a unit, so the
    // record's protection is no longer "they are labelled differently"
    // but "they are measured at different points in the pipeline". The
    // assertion above is the one that matters and it is unchanged; what
    // follows is the restated version of the old `contains("chip")`
    // check.
    assert_eq!(
        e.gate.unit(),
        e.commanded.unit(),
        "stage 4 and stage 1 are the same quantity at different feeds"
    );
    assert!(
        e.commanded.unit().contains("advance"),
        "stage 1's unit must name it as a linear advance"
    );
}
