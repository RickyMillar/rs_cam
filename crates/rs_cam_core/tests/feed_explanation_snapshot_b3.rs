//! **B3 — the feed-explanation snapshot assembler.**
//!
//! `planning/review_2026-08-04/TECH_DEBT_2_RESEARCH_AND_FIX_PLAN.md` §H1
//! authorises a test-only "feed explanation snapshot" assembler for the
//! feeds census, with one constraint: *it must label stages rather than
//! choose a winner in prose*. This file is that assembler. It asserts
//! **relationships between stages**, never that a stage is right.
//!
//! ## What the operator saw (live session 2026-07-30)
//!
//! Four chipload numbers for one operation, no two agreeing
//! (`planning/review_2026-07-29/ORCHESTRATION_LOG.md:1284-1290`):
//!
//! | stage | mm/tooth |
//! |---|---|
//! | narration nominal | 0.0714 |
//! | `feeds.chipload_clamped_to_floor` pre → post | 0.0044 → 0.0250 |
//! | gate observed | 0.000737 |
//! | gate band | 0.00458 – 0.00916 |
//!
//! `planning/review_2026-08-04/FEEDS_CENSUS.md` §4.3 reconciles the gate
//! observation **by hand**, to +0.24 %:
//!
//! ```text
//! gate_observed = commanded_fpt
//!               × mean_chip_factor(LUT nominal arc)   ← 0.080335 on that row
//!               × predicted_feed / commanded_feed     ← 0.1282 (−87.18 %)
//! ```
//!
//! A hand-checked arithmetic identity is not evidence a gate change can be
//! re-measured against. This file re-derives the same identity **through
//! the shipped code**, with no generator, arc fitter, lead-in, dressup or
//! depth planner participating — the trace is hand-built, so a reproduction
//! cannot be attributed to any of them.
//!
//! ## The five labelled stages
//!
//! [`FeedExplanation`] carries them. Each is sourced from exactly one
//! production symbol, named in its field doc. No field is computed from
//! another except where the census claims a relationship, and those claims
//! are the assertions.
//!
//! ## What each test establishes
//!
//! 1. [`stage_3_lut_arc_factor_matches_shipped_chip_geometry`] — the census
//!    read `tool_load::chipload::mean_chip_factor` (a private mirror) rather
//!    than the production chip model. This drives
//!    `MillingCutter::chip_geometry` across an arc sweep and shows the mirror
//!    is exact. Without this the whole reconciliation rests on a copy.
//! 2. [`the_gate_observation_reconciles_to_the_three_labelled_stages`] — the
//!    census's identity, live.
//! 3. [`the_sample_engagement_arc_cancels_out_of_the_gate_observation`] —
//!    the census's strongest structural claim: after D9 the sample's own arc
//!    algebraically cancels, so "observed chipload" carries no information
//!    about sample engagement. Two fixtures differing **only** in sample arc
//!    must produce the same observation.
//! 4. [`the_predicted_feed_factor_is_live`] — non-vacuity. With
//!    `predicted_feeds` empty the observation must move by exactly the feed
//!    ratio, proving stage 4 is not inert in this fixture.
//! 5. [`the_gate_observation_and_the_band_are_not_the_same_quantity`] — the
//!    unit finding (census F-1), stated as a measured ratio, with **no claim
//!    about which side is correct**. That is Checkpoint B item T4.1.
//! 6. [`the_commanded_feed_per_tooth_is_never_compared_to_the_band`] —
//!    census P-10. The only same-unit comparison available is not made by
//!    any verdict, and here it is far above the band.
//! 7. [`the_two_doc_ratio_diameters_only_diverge_for_v_bit_geometry`] —
//!    census §9 item 5 / P-5. The census asserted, statically, that
//!    Suggest's post-mutation band re-derivation and the gate's derating
//!    can diverge "on ball tools at shallow DOC". Measured here through
//!    `feeds::calculate`, that is **false**; the reachable case is
//!    narrower and is a different geometry.
//!
//! **Nothing here proposes a fix.** Every number this file prints is
//! reported under the stage that produced it.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::dexel_stock::effective_chip_thickness_mm;
use rs_cam_core::feeds::vendor_lookup::{LookupQuery, LookupResult, find_best_chip_envelope_row};
use rs_cam_core::feeds::vendor_lut::{
    HardnessKind, LutOperationFamily, LutPassRole, MaterialFamily, ToolFamily,
};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine_kinematics::PredictedFeedMap;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::simulation_cut::{
    CutKinematics, Engagement, SimulationCutSample, SimulationCutTrace,
};
use rs_cam_core::tool::{EngagementMode, MillingCutter, TaperedBallEndmill, ToolDefinition};
use rs_cam_core::tool_load::verdict::{ChipBoundsSource, ChiploadVerdict};
use rs_cam_core::tool_load::{GateEnv, ToleranceBands, ToolpathLoadContext, chipload};

// ── The fixture, pinned to the live operation's shape ───────────────────
//
// Synthesised in-test. `planning/airrun_2026-06-01/wanaka.toml` is a
// read-only play-file (plan §2 rule 9) and is NOT an input here.

/// Ball-tip diameter of the tapered cutter (mm).
const TIP_DIAMETER_MM: f64 = 1.0;
/// Taper half-angle (degrees).
const TAPER_HALF_ANGLE_DEG: f64 = 5.26;
/// Shank diameter (mm) — caps the tapered cone's growth.
const SHANK_DIAMETER_MM: f64 = 6.0;

const RPM: u32 = 18_000;
const FLUTES: u32 = 2;
/// Commanded feed chosen so `feed / (rpm · flutes)` lands on the live
/// narration nominal of 0.0714 mm/tooth.
const COMMANDED_FEED_MM_MIN: f64 = 0.0714 * RPM as f64 * FLUTES as f64;
/// Live `modulation_summary.median_feed_delta_pct = −87.18`.
const PREDICTED_FEED_FRACTION: f64 = 0.128_2;

/// Axial engagement of every sample (mm). Sits in the tapered ball's
/// spherical region and clear of the ±0.05 mm ball/cone transition zone
/// that `TaperedBallEndmill::chip_geometry` refuses.
const SAMPLE_AXIAL_DOC_MM: f64 = 0.35;

/// A sample engagement arc deliberately far from the matched row's nominal
/// arc, so the D9 normalisation is doing real work in this fixture.
const SAMPLE_ARC_A_RAD: f64 = 1.5;
/// A second, very different arc. Test 3 requires the observation to be
/// identical under both.
const SAMPLE_ARC_B_RAD: f64 = 0.4;

const SAMPLE_COUNT: usize = 9;
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

/// Hard maple: Janka 1450, the exact hardness the matched row publishes,
/// so the hardness scale is 1.00 and the diameter scale is the only one
/// moving — mirroring the live evidence string
/// `"diameter scale x0.42, hardness scale x1.00"`.
fn material() -> Material {
    Material::SolidWood {
        species: WoodSpecies::HardMaple,
    }
}

// ── Stage record ────────────────────────────────────────────────────────

/// One labelled explanation of "what chipload is this operation running
/// at", assembled from every stage that answers the question.
///
/// The stages are deliberately **not** reduced to a single number. The
/// census's finding is that they answer different questions in different
/// units; collapsing them here would reproduce the defect.
#[derive(Debug, Clone)]
struct FeedExplanation {
    /// **Stage 1 — commanded.** `feed / (rpm · flutes)`, the quantity
    /// `narrate.rs:426` prints as "nominal chipload". Unit: mm of linear
    /// *advance* per tooth.
    commanded_fpt_mm: f64,
    /// **Stage 2 — LUT band.** `chip_load_min/max_mm` off the matched row
    /// (`vendor_lookup::find_best_chip_envelope_row`), already scaled for
    /// diameter and hardness. Unit: vendor advance per tooth.
    band_min_mm: Option<f64>,
    band_max_mm: f64,
    /// **Stage 2 provenance.** Row id, the two scales, and the ±40 %
    /// extrapolation flag, as the gate classifies them.
    row_id: String,
    diameter_scale: f64,
    hardness_scale: f64,
    extrapolated: bool,
    bounds_source: ChipBoundsSource,
    /// **Stage 3 — LUT arc factor.** `mean_chip / feed_per_tooth` at the
    /// engagement arc the matched row was authored against, measured
    /// through `MillingCutter::chip_geometry`. Dimensionless.
    lut_arc_rad: f64,
    lut_arc_factor: f64,
    /// **Stage 4 — achieved-feed factor.** `predicted / commanded`, the
    /// substitution `tool_load::effective_feed_for_sample` performs.
    /// Dimensionless. `1.0` when the trace carries no predicted feeds.
    predicted_feed_factor: f64,
    /// **Stage 5 — gate observation.** What `tool_load::chipload::evaluate`
    /// reports. Unit: mm of *chip*, at the LUT row's nominal arc, at the
    /// achieved feed, taken as the median over the steady-state set.
    gate_observed_mm: f64,
    /// Which arm the verdict landed on, and whether the low side was
    /// demoted to an advisory (F3.3).
    verdict_label: &'static str,
    burn_advisory: bool,
}

impl FeedExplanation {
    /// The census §4.3 identity, evaluated from the labelled stages. This
    /// is a *prediction of stage 5 from stages 1, 3 and 4* — it is not
    /// itself an observation.
    fn predicted_gate_observation(&self) -> f64 {
        self.commanded_fpt_mm * self.lut_arc_factor * self.predicted_feed_factor
    }

    fn report(&self, title: &str) {
        eprintln!("\n── feed explanation: {title} ─────────────────────────");
        eprintln!(
            "  stage 1  commanded feed-per-tooth   {:.6} mm/tooth   (advance)",
            self.commanded_fpt_mm
        );
        eprintln!(
            "  stage 2  LUT band                   {:.6} .. {:.6} mm/tooth (vendor advance)",
            self.band_min_mm.unwrap_or(f64::NAN),
            self.band_max_mm
        );
        eprintln!(
            "           row {} | d-scale x{:.4} | h-scale x{:.4} | extrapolated {} | source {:?}",
            self.row_id,
            self.diameter_scale,
            self.hardness_scale,
            self.extrapolated,
            self.bounds_source
        );
        eprintln!(
            "  stage 3  LUT arc {:.6} rad -> factor {:.6}          (dimensionless)",
            self.lut_arc_rad, self.lut_arc_factor
        );
        eprintln!(
            "  stage 4  achieved/commanded feed    {:.6}            (dimensionless)",
            self.predicted_feed_factor
        );
        eprintln!(
            "  stage 5  gate observation           {:.9} mm        (chip, at LUT arc, at achieved feed)",
            self.gate_observed_mm
        );
        eprintln!(
            "           verdict {} | burn_advisory {}",
            self.verdict_label, self.burn_advisory
        );
        eprintln!(
            "  identity 1x3x4 = {:.9}   observed = {:.9}   residual {:+.4} %",
            self.predicted_gate_observation(),
            self.gate_observed_mm,
            100.0 * (self.gate_observed_mm / self.predicted_gate_observation() - 1.0)
        );
    }
}

// ── Assembly ────────────────────────────────────────────────────────────

/// Stage 2 — resolve the row the gate will match, through the same public
/// entry point the gate's private `matched_chip_envelope` uses.
fn matched_row(tool: &ToolDefinition, lookup_diameter_mm: f64) -> LookupResult {
    let query = LookupQuery {
        tool_family: ToolFamily::TaperedBallNose,
        tool_subfamily: None,
        diameter_mm: lookup_diameter_mm,
        flute_count: FLUTES,
        material_family: MaterialFamily::Hardwood,
        hardness_kind: Some(HardnessKind::Janka),
        hardness_value: Some(1450.0),
        operation_family: LutOperationFamily::Scallop,
        pass_role: LutPassRole::Finish,
    };
    find_best_chip_envelope_row(
        rs_cam_core::feeds::embedded_vendor_lut(),
        &query,
        &tool.to_geometry_hint(),
    )
    .expect("the embedded LUT publishes a tapered-ball scallop row in hardwood")
}

/// Stage 3 — the engagement arc the row was authored against, from its
/// calibrated `ae` window. Mirrors `chipload::lut_nominal_arc_rad`, which
/// is private; the *factor* below is then measured through production
/// code rather than mirrored.
fn lut_nominal_arc_rad(row: &LookupResult) -> f64 {
    let ae_mid = (row.ae_min_mm.expect("row publishes ae_min")
        + row.ae_max_mm.expect("row publishes ae_max"))
        * 0.5;
    (1.0 - 2.0 * ae_mid / row.row_diameter_mm)
        .clamp(-1.0, 1.0)
        .acos()
}

/// Stage 3 — `mean_chip / feed_per_tooth` at `arc`, measured by driving
/// the **production** chip model at unit feed per tooth. Independent of
/// `tool_load::chipload::mean_chip_factor`.
fn arc_factor_via_chip_geometry(cutter: &dyn MillingCutter, arc_rad: f64) -> f64 {
    cutter
        .chip_geometry(
            SAMPLE_AXIAL_DOC_MM,
            arc_rad,
            1.0, // unit feed per tooth ⇒ mean_chip IS the factor
            FLUTES,
            EngagementMode::Climb,
        )
        .expect("chip geometry supported at this DOC / arc")
        .mean_chip_thickness_mm
}

/// Build the trace. Every sample's `effective_chip_thickness_mm` comes
/// from the shipped `dexel_stock::effective_chip_thickness_mm` driven by
/// the real cutter — no chip value is hand-written into this fixture.
fn trace(sample_arc_rad: f64, with_predicted_feeds: bool) -> SimulationCutTrace {
    let tool = tool();
    let commanded_fpt = COMMANDED_FEED_MM_MIN / (f64::from(RPM) * f64::from(FLUTES));
    let chip = effective_chip_thickness_mm(
        &tool,
        SAMPLE_AXIAL_DOC_MM,
        Some(sample_arc_rad),
        commanded_fpt,
        FLUTES,
    )
    .expect("the production chip model resolves at this DOC / arc");

    let samples: Vec<SimulationCutSample> = (0..SAMPLE_COUNT)
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
            arc_engagement_radians: Some(sample_arc_rad),
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

/// Run the shipped gate over the fixture and assemble the five stages.
fn explain(sample_arc_rad: f64, with_predicted_feeds: bool) -> FeedExplanation {
    let tool = tool();
    let material = material();
    let trace = trace(sample_arc_rad, with_predicted_feeds);
    let tolerance = ToleranceBands::default();

    let verdict = chipload::evaluate(
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
        &GateEnv {
            sim_trace: Some(&trace),
            machine: None,
            tolerance: &tolerance,
        },
    );

    // The gate's own lookup diameter: the engaged diameter at the peak
    // steady-state axial DOC.
    let lookup_diameter = tool.lookup_diameter_at(SAMPLE_AXIAL_DOC_MM);
    let row = matched_row(&tool, lookup_diameter);
    let lut_arc = lut_nominal_arc_rad(&row);

    let (gate_observed_mm, band_min, band_max, source, verdict_label, burn_advisory) =
        match &verdict {
            ChiploadVerdict::Within {
                approach_to_min,
                approach_to_max,
                burn_advisory,
                ..
            } => {
                // The low-side observation is the median statistic — the same
                // one the live session read. Prefer the advisory (which is the
                // demoted trip), else the approach metric.
                let m = burn_advisory
                    .as_deref()
                    .or(approach_to_min.as_ref())
                    .unwrap_or(approach_to_max);
                (
                    m.observed_mm_per_tooth,
                    m.bounds.min_mm_per_tooth,
                    m.bounds.max_mm_per_tooth,
                    m.bounds.source,
                    "Within",
                    burn_advisory.is_some(),
                )
            }
            ChiploadVerdict::Exceeds { triggering, .. } => (
                triggering.observed_mm_per_tooth,
                triggering.bounds.min_mm_per_tooth,
                triggering.bounds.max_mm_per_tooth,
                triggering.bounds.source,
                "Exceeds",
                false,
            ),
            ChiploadVerdict::Unmodeled { reason } => {
                panic!(
                    "the fixture must reach a modelled verdict — the assembler cannot explain a \
                 refusal. Got Unmodeled({reason:?})"
                )
            }
        };

    FeedExplanation {
        commanded_fpt_mm: COMMANDED_FEED_MM_MIN / (f64::from(RPM) * f64::from(FLUTES)),
        band_min_mm: band_min,
        band_max_mm: band_max,
        row_id: row.observation_id.clone(),
        diameter_scale: row.chipload_diameter_scale,
        hardness_scale: row.chipload_hardness_scale,
        extrapolated: row.is_extrapolated,
        bounds_source: source,
        lut_arc_rad: lut_arc,
        lut_arc_factor: arc_factor_via_chip_geometry(&tool, lut_arc),
        predicted_feed_factor: if with_predicted_feeds {
            PREDICTED_FEED_FRACTION
        } else {
            1.0
        },
        gate_observed_mm,
        verdict_label,
        burn_advisory,
    }
}

// ── 1. The mirror ───────────────────────────────────────────────────────

/// FEEDS_CENSUS.md §9 item 2. The census read the arc factor off
/// `tool_load::chipload::mean_chip_factor` — a private mirror of the
/// production chip model. If the mirror had drifted, the entire §4.3
/// reconciliation would be arithmetic about the wrong function.
///
/// Drives `MillingCutter::chip_geometry` at unit feed per tooth across an
/// arc sweep and compares against the mirror's closed form. Also pins the
/// two anchor values the mirror's own docstring advertises.
#[test]
fn stage_3_lut_arc_factor_matches_shipped_chip_geometry() {
    /// The mirror, verbatim from `tool_load::chipload::mean_chip_factor`.
    fn mirror(arc_rad: f64) -> f64 {
        let arc = arc_rad.clamp(1e-9, std::f64::consts::PI);
        let h_max = if arc >= std::f64::consts::PI {
            1.0
        } else {
            arc.sin().abs()
        };
        (2.0 * h_max / arc) * (1.0 - (arc * 0.5).cos())
    }

    let tool = tool();
    for &arc in &[
        0.2_f64,
        0.4,
        0.58,
        SAMPLE_ARC_B_RAD,
        1.0,
        1.0843860798928202,
        SAMPLE_ARC_A_RAD,
        2.0,
        3.0,
        std::f64::consts::PI,
    ] {
        let shipped = arc_factor_via_chip_geometry(&tool, arc);
        let mirrored = mirror(arc);
        assert!(
            (shipped - mirrored).abs() < 1e-12,
            "arc {arc}: production chip_geometry gives {shipped}, the chipload gate's private \
             mean_chip_factor mirror gives {mirrored}. The census's arithmetic is written against \
             the mirror; a drift here invalidates it."
        );
    }

    // Anchors the mirror's docstring advertises, as closed forms rather
    // than decimals — at these two arcs the factor has an exact value:
    //   arc = π   ⇒ (2·1/π)·(1 − cos(π/2))   = 2/π          ≈ 0.6366
    //   arc = π/2 ⇒ (2·1/(π/2))·(1 − cos(π/4)) = (4/π)(1 − √2/2) ≈ 0.3729
    assert!(
        (mirror(std::f64::consts::PI) - std::f64::consts::FRAC_2_PI).abs() < 1e-12,
        "slot anchor moved: mean_chip_factor(π) must be exactly 2/π"
    );
    let half_immersion = (4.0 / std::f64::consts::PI) * (1.0 - std::f64::consts::FRAC_1_SQRT_2);
    assert!(
        (mirror(std::f64::consts::FRAC_PI_2) - half_immersion).abs() < 1e-12,
        "half-immersion anchor moved: mean_chip_factor(π/2) must be exactly (4/π)(1 − √2/2)"
    );
    eprintln!(
        "CONFIRMED: mean_chip_factor is exact against MillingCutter::chip_geometry across \
         10 arcs (max |Δ| < 1e-12)."
    );
}

// ── 2. The identity ─────────────────────────────────────────────────────

/// FEEDS_CENSUS.md §4.3, live. The gate's observation is the commanded
/// feed-per-tooth times the matched row's arc factor times the
/// achieved/commanded feed ratio — three labelled stages, no fourth term.
#[test]
fn the_gate_observation_reconciles_to_the_three_labelled_stages() {
    let e = explain(SAMPLE_ARC_A_RAD, true);
    e.report("live-shaped fixture (arc A, predicted feeds on)");

    let residual = e.gate_observed_mm / e.predicted_gate_observation() - 1.0;
    assert!(
        residual.abs() < 0.01,
        "the census claims gate_observed = commanded_fpt x lut_arc_factor x feed_factor. \
         Predicted {:.9}, observed {:.9}, residual {:+.4} %. A residual above 1 % means a term \
         the census did not name is participating.",
        e.predicted_gate_observation(),
        e.gate_observed_mm,
        100.0 * residual
    );

    // The verdict shape the live session saw: a weakly-provenanced low-side
    // trip demoted to an advisory (F3.3), not a hard refusal.
    assert!(
        e.extrapolated,
        "the fixture is meant to reproduce an extrapolated row"
    );
    assert_eq!(e.verdict_label, "Within");
    assert!(
        e.burn_advisory,
        "an observation this far below the floor must surface as a burn advisory (F3.3 + H4)"
    );
}

// ── 3. The structural claim ─────────────────────────────────────────────

/// FEEDS_CENSUS.md §4.3's strongest claim: after D9 the sample's own
/// engagement arc cancels algebraically —
/// `cl_norm = fz · f(arc_sample) · f_lut / f(arc_sample) = fz · f_lut` —
/// so the gate's "observed chipload" carries **no information about the
/// sample's engagement**, which is the thing the metric is named for.
///
/// Two fixtures differing only in sample arc (1.5 rad vs 0.4 rad — a
/// factor of 3.4 in the raw chip thickness) must report the same
/// observation.
#[test]
fn the_sample_engagement_arc_cancels_out_of_the_gate_observation() {
    let a = explain(SAMPLE_ARC_A_RAD, true);
    let b = explain(SAMPLE_ARC_B_RAD, true);
    a.report("arc A = 1.5 rad");
    b.report("arc B = 0.4 rad");

    // Non-vacuity: the two fixtures really do present different chips.
    let tool = tool();
    let raw_a = arc_factor_via_chip_geometry(&tool, SAMPLE_ARC_A_RAD);
    let raw_b = arc_factor_via_chip_geometry(&tool, SAMPLE_ARC_B_RAD);
    assert!(
        raw_a / raw_b > 2.0,
        "the two arcs must produce materially different raw chips or this test proves nothing \
         (got {raw_a} vs {raw_b})"
    );

    let delta = (a.gate_observed_mm - b.gate_observed_mm).abs()
        / a.gate_observed_mm.max(b.gate_observed_mm);
    assert!(
        delta < 1e-9,
        "the sample arc must cancel: arc A gives {:.12}, arc B gives {:.12} (relative Δ {delta:.3e}) \
         from raw chips differing by {:.2}x",
        a.gate_observed_mm,
        b.gate_observed_mm,
        raw_a / raw_b
    );
    eprintln!(
        "CONFIRMED: raw chip differs {:.2}x between the two arcs; the gate's observation is \
         identical to {delta:.1e}. The reported 'chipload' is fz x f(LUT row) x feed ratio.",
        raw_a / raw_b
    );
}

// ── 4. Non-vacuity of stage 4 ───────────────────────────────────────────

/// The predicted-feed factor must be live in this fixture, not inert.
/// With `predicted_feeds` empty the gate falls back to commanded feed
/// (`effective_feed_for_sample`), so the observation must rise by exactly
/// `1 / PREDICTED_FEED_FRACTION`.
#[test]
fn the_predicted_feed_factor_is_live() {
    let on = explain(SAMPLE_ARC_A_RAD, true);
    let off = explain(SAMPLE_ARC_A_RAD, false);
    off.report("predicted feeds OFF (commanded feed)");

    assert!(
        (off.predicted_feed_factor - 1.0).abs() < 1e-12,
        "the OFF arm must report a unit feed factor"
    );
    let ratio = on.gate_observed_mm / off.gate_observed_mm;
    assert!(
        (ratio - PREDICTED_FEED_FRACTION).abs() < 1e-9,
        "turning predicted feeds off must scale the observation by exactly \
         1 / {PREDICTED_FEED_FRACTION}; got {ratio}"
    );
    // Both arms still reconcile to their own stages.
    assert!((off.gate_observed_mm / off.predicted_gate_observation() - 1.0).abs() < 0.01);
}

// ── 5. The unit finding, measured — no verdict on which side is right ────

/// FEEDS_CENSUS.md F-1. The gate's observation is an arc-mean **chip
/// thickness**; the band it is compared against is a vendor **advance per
/// tooth**. This test measures the ratio between the two conventions on
/// this row and asserts only that it is far from 1.
///
/// **It does not claim either side is wrong.** Whether a vendor chipload
/// column is an advance or a mean chip is Checkpoint B item T4.1, a
/// literature question. This test exists so that when T4.1 is answered the
/// magnitude is already measured.
#[test]
fn the_gate_observation_and_the_band_are_not_the_same_quantity() {
    let e = explain(SAMPLE_ARC_A_RAD, true);
    let convention_ratio = 1.0 / e.lut_arc_factor;
    assert!(
        convention_ratio > 2.0,
        "if the two conventions ever coincide this test is vacuous; got {convention_ratio}"
    );
    eprintln!(
        "MEASURED (no verdict): the gate reports mm of CHIP; the band publishes mm of ADVANCE. \
         On row {} the two conventions differ by {convention_ratio:.2}x \
         (arc {:.5} rad, factor {:.6}). Which side is correct is Checkpoint B item T4.1.",
        e.row_id, e.lut_arc_rad, e.lut_arc_factor
    );

    // Restate the live shape: the observation sits below the floor while
    // the verdict stays Within on provenance alone (F3.3).
    let min = e.band_min_mm.expect("row publishes a floor");
    assert!(
        e.gate_observed_mm < min,
        "the fixture must reproduce the below-floor observation the live session read"
    );
}

// ── 6. The comparison nobody makes ──────────────────────────────────────

/// FEEDS_CENSUS.md P-10. Commanded feed-per-tooth and the vendor band are
/// the **same unit at compatible stages** — the one pair among the four
/// live numbers whose comparison is unambiguously valid. No verdict makes
/// it. Here it is far above the band, and the gate says `Within`.
#[test]
fn the_commanded_feed_per_tooth_is_never_compared_to_the_band() {
    let e = explain(SAMPLE_ARC_A_RAD, true);
    let overshoot = e.commanded_fpt_mm / e.band_max_mm;
    assert!(
        overshoot > 2.0,
        "the fixture must reproduce a commanded feed-per-tooth well above the band max, or the \
         missing comparison has nothing to surface; got {overshoot}x"
    );
    assert_eq!(
        e.verdict_label, "Within",
        "and the gate must nonetheless report Within — that IS the finding"
    );
    eprintln!(
        "MEASURED: commanded {:.6} mm/tooth vs band max {:.6} mm/tooth = {overshoot:.2}x over, \
         same unit, same stage — and the verdict is Within. No production consumer makes this \
         comparison (census P-10).",
        e.commanded_fpt_mm, e.band_max_mm
    );
}

// ── 7. P-5 reachability, measured ───────────────────────────────────────

/// FEEDS_CENSUS.md §9 item 5 and finding P-5.
///
/// `feeds::calculate` binds the identifier `effective_d` twice: once at
/// LUT semantics (`ToolGeometryHint::engaged_diameter_at_doc`, the
/// denominator of the band derating it performs itself) and once, shadowing
/// it, at chip-thinning semantics (`feeds::effective_diameter`, which is
/// what gets published as `FeedsResult::effective_diameter_mm`). The
/// published one is the denominator
/// `suggest::recompute_chipload_bounds_for_dpp` divides by after the axial
/// envelope mutates DPP, while `tool_load::chipload` divides by the LUT one.
///
/// The census inferred from that shadowing that the two could produce
/// different DOC-derate scales "on a ball nose at shallow DOC, up to 2x".
/// This test measures the derate both ways through shipped code — the
/// published `effective_diameter_mm` against `lookup_diameter_at` at the
/// same resolved DOC — over a DOC sweep on every cutter shape.
///
/// **Result: ball, bull, flat and tapered-ball never diverge.** Only V-bit
/// geometry can, because `engaged_diameter_at_doc` omits the tip flat that
/// `vbit_width_at_depth` includes. The non-vacuity assertion at the end
/// requires the V-bit case to actually appear, so this test cannot pass by
/// finding nothing anywhere.
#[test]
fn the_two_doc_ratio_diameters_only_diverge_for_v_bit_geometry() {
    use rs_cam_core::feeds::geometry::doc_derating_scale;
    use rs_cam_core::feeds::{
        FeedsInput, OperationFamily, PassRole, SetupContext, SpindleStrategy, ToolGeometryHint,
        calculate,
    };
    use rs_cam_core::machine::MachineProfile;
    use rs_cam_core::tool::{BallEndmill, BullNoseEndmill, FlatEndmill, VBitEndmill};

    let machine = MachineProfile::shapeoko_vfd();
    let mat = material();

    let cases: Vec<(&str, ToolGeometryHint, Box<dyn MillingCutter>, f64)> = vec![
        (
            "flat Ø6",
            ToolGeometryHint::Flat,
            Box::new(FlatEndmill::new(6.0, 40.0)),
            6.0,
        ),
        (
            "ball Ø6",
            ToolGeometryHint::Ball,
            Box::new(BallEndmill::new(6.0, 40.0)),
            6.0,
        ),
        (
            "bull Ø6 r1",
            ToolGeometryHint::Bull { corner_radius: 1.0 },
            Box::new(BullNoseEndmill::new(6.0, 1.0, 40.0)),
            6.0,
        ),
        (
            "tapered ball Ø1 tip",
            ToolGeometryHint::TaperedBall {
                tip_radius: TIP_DIAMETER_MM * 0.5,
                taper_angle_deg: TAPER_HALF_ANGLE_DEG,
            },
            Box::new(TaperedBallEndmill::new(
                TIP_DIAMETER_MM,
                TAPER_HALF_ANGLE_DEG,
                SHANK_DIAMETER_MM,
                20.0,
            )),
            TIP_DIAMETER_MM,
        ),
        (
            "v-bit 20° Ø6, 1.0 mm tip flat",
            ToolGeometryHint::VBit {
                included_angle: 20.0,
                tip_diameter: 1.0,
            },
            {
                // `VBitEndmill::new` defaults the flat to 0.0; the field is
                // public and set after construction (its own doc says so),
                // so cutter and hint describe the *same* truncated bit.
                let mut v = VBitEndmill::new(6.0, 20.0, 40.0);
                v.tip_diameter = 1.0;
                Box::new(v)
            },
            6.0,
        ),
    ];

    let mut divergent_shapes: Vec<&str> = Vec::new();
    for (label, hint, cutter, nominal_d) in &cases {
        let mut worst = 0.0_f64;
        let mut worst_at = 0.0_f64;
        for step in 1..=120 {
            let commanded_ap = f64::from(step) * 0.1;
            let result = calculate(&FeedsInput {
                tool_diameter: *nominal_d,
                flute_count: FLUTES,
                flute_length: 40.0,
                shank_diameter: Some(SHANK_DIAMETER_MM),
                tool_geometry: *hint,
                material: &mat,
                machine: &machine,
                operation: OperationFamily::Adaptive,
                pass_role: PassRole::Roughing,
                axial_depth_mm: Some(commanded_ap),
                // Well below the slotting threshold so Step 4b never
                // re-caps DOC behind our back.
                radial_width_mm: Some(nominal_d * 0.3),
                target_scallop_mm: None,
                vendor_lut: None,
                setup: SetupContext::default(),
                spindle_strategy: SpindleStrategy::default(),
            });
            // The DOC that actually survived every clamp inside calculate.
            let ap = result.axial_depth_mm;
            // Denominator A: what Suggest's post-mutation re-derivation uses.
            let scale_suggest = doc_derating_scale(ap / result.effective_diameter_mm.max(1e-9));
            // Denominator B: what the post-sim chipload gate uses.
            let scale_gate = doc_derating_scale(ap / cutter.lookup_diameter_at(ap).max(1e-9));
            let delta = (scale_suggest - scale_gate).abs();
            if delta > worst {
                worst = delta;
                worst_at = ap;
            }
        }
        if worst > 1e-12 {
            divergent_shapes.push(label);
            eprintln!("  {label:32} DIVERGES — worst |Δscale| {worst:.4} at DOC {worst_at:.2} mm");
        } else {
            eprintln!("  {label:32} identical derate across the whole DOC sweep");
        }
    }

    assert_eq!(
        divergent_shapes,
        vec!["v-bit 20° Ø6, 1.0 mm tip flat"],
        "the census predicted a ball-nose divergence and did not predict a V-bit one; measured \
         divergent shapes are {divergent_shapes:?}"
    );
    eprintln!(
        "MEASURED: P-5's numeric divergence is NOT reachable on ball / bull / flat / tapered-ball \
         geometry. Only V-bit diverges, because engaged_diameter_at_doc omits the tip flat that \
         vbit_width_at_depth includes. Combined with pick_axial_envelope setting dpp_mutated on \
         Adaptive3d ONLY (suggest.rs:1348-1356; VCarve declines it, the finish-3D family is \
         warning-only), the reachable case is Adaptive3d + V-bit."
    );
}
