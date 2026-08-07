//! **A-1 / F-HEATMAP — the synthetic two-arc fixture.**
//!
//! Commissioned by `planning/review_2026-08-08/TECH_DEBT_3_RESEARCH_AND_FIX_PLAN.md`
//! §2 A-1, which takes it verbatim from the operator's own review
//! (`planning/review_2026-08-04/FEEDS_SPEEDS_ARCHITECTURE_REVIEW_2026-08-07.md`,
//! [high] "The viewport chipload graph compares incompatible units",
//! *Acceptance*):
//!
//! > A synthetic two-arc fixture must demonstrate that changing only
//! > engagement arc changes the chip-thickness graph but not the
//! > feed-per-tooth band comparison.
//!
//! ## What this file is, today
//!
//! A **characterization** of the shipped 2026-08-08 behaviour, not a
//! bar. Every assertion below passes on the current tree and is meant
//! to. It exists so that A-2 can flip the display-measure assertions
//! red-first against the *old* measure, with the pre-fix numbers
//! already recorded here rather than reconstructed afterwards.
//!
//! ## The three quantities
//!
//! | quantity | expression | arc-sensitive? |
//! |---|---|---|
//! | commanded advance/tooth | `feed / (rpm · flutes)` | **no** |
//! | achieved advance/tooth | `effective_feed / (rpm · flutes)` | **no** |
//! | arc-mean chip thickness | `MillingCutter::chip_geometry(..).mean_chip_thickness_mm` | **yes** |
//!
//! The vendor LUT band is published in the first two units
//! (`CHIPLOAD_LITERATURE_VERDICT.md` §2). The post-simulation chipload
//! gate observes the second (`tool_load::chipload`'s header, since
//! 2026-08-06). The **viewport heat-map colours by the third against a
//! band for the first** — `rs_cam_viz::app::gpu_upload::build_chipload_per_move`
//! takes `max(effective_chip_thickness_mm)` per move and hands it to
//! `rs_cam_viz::render::toolpath_render::chipload_segment_color` beside
//! `rs_cam_core::tool_load::chipload_envelopes_for_session`'s advance
//! band. Two further viz surfaces share the defect (the sim-timeline
//! "chipload" track and its "load vs limit" normalised summary) — see
//! `planning/review_2026-08-08/HEATMAP_VOCAB_CENSUS.md`.
//!
//! ## Why the fixture is decisive
//!
//! Two toolpaths, identical in every commanded number — same tool, same
//! material, same feed, same RPM, same flute count, same axial DOC —
//! differing **only** in engagement arc, with the radial engagement
//! fraction set to the value that arc implies
//! (`ae/D = (1 − cos(arc/2)) / 2`). The gate's observation is bit-equal
//! across the pair. The heat-map's observation is not, and the two land
//! in **different colour classes against the same band**. A colour that
//! moves when nothing the operator commanded moved cannot be used to
//! accept or reject a recommendation, which is precisely the operator
//! complaint the review traced to this code.
//!
//! Nothing here is a threshold, a verdict or a recommendation. This file
//! reads production functions and asserts relationships between them.

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
use rs_cam_core::tool::{FlatEndmill, ToolDefinition};
use rs_cam_core::tool_load::verdict::{ChipBounds, ChiploadVerdict};
use rs_cam_core::tool_load::{ToleranceBands, ToolpathLoadContext, evaluate_toolpath};

const DIAMETER_MM: f64 = 6.0;
const FLUTES: u32 = 2;
const RPM: u32 = 18_000;
/// Commanded advance per tooth. Both arms carry this exact number.
/// Chosen so the *achieved* value lands mid-band rather than on a
/// boundary — the fixture must not be reading G-CHIP-ULP by accident.
const COMMANDED_FPT_MM: f64 = 0.070;
const COMMANDED_FEED_MM_MIN: f64 = COMMANDED_FPT_MM * RPM as f64 * FLUTES as f64;
/// F-035 kinematics substitution — the machine achieves this fraction of
/// the commanded feed. Identical on both arms, so it cannot be the
/// source of any divergence found below.
const PREDICTED_FEED_FRACTION: f64 = 0.62;
const AXIAL_DOC_MM: f64 = 1.5;
const SAMPLES_PER_ARM: usize = 24;

/// Arm A — a light radial pass. `ae/D ≈ 0.152`.
const ARC_A_RAD: f64 = 0.8;
/// Arm B — a full slot. `ae/D = 1.0`, the case
/// `tool_load/mod.rs`'s own caveat quantifies at 1.6×.
const ARC_B_RAD: f64 = std::f64::consts::PI;

const TP_A: ToolpathId = ToolpathId(1);
const TP_B: ToolpathId = ToolpathId(2);

fn tool() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(FlatEndmill::new(DIAMETER_MM, 25.0)),
        DIAMETER_MM,
        50.0,
        25.0,
        60.0,
        FLUTES,
        ToolMaterial::Carbide,
    )
}

fn material() -> Material {
    Material::SolidWood {
        species: WoodSpecies::HardMaple,
    }
}

/// Radial engagement fraction implied by an engagement arc:
/// `cos(arc) = 1 − 2·ae/D`, so `ae/D = (1 − cos arc) / 2`. Sanity: a
/// full slot (`arc = π`) gives 1.0, half immersion (`arc = π/2`) gives
/// 0.5. Used so the two arms are physically coherent rather than an arc
/// bolted onto an unrelated width — the divergence must not be an
/// artifact of a self-contradictory sample.
fn radial_woc_fraction_for_arc(arc_rad: f64) -> f64 {
    (1.0 - arc_rad.cos()) / 2.0
}

/// One arm: `SAMPLES_PER_ARM` steady-state cutting samples that differ
/// from the other arm only in engagement arc and in the two quantities
/// derived from it.
fn arm(toolpath_id: ToolpathId, arc_rad: f64) -> (Vec<SimulationCutSample>, f64) {
    let tool = tool();
    let chip = rs_cam_core::dexel_stock::effective_chip_thickness_mm(
        &tool,
        AXIAL_DOC_MM,
        Some(arc_rad),
        COMMANDED_FPT_MM,
        FLUTES,
    )
    .expect("the production chip model resolves at this DOC / arc");
    let radial = radial_woc_fraction_for_arc(arc_rad);
    let samples = (0..SAMPLES_PER_ARM)
        .map(|i| SimulationCutSample {
            toolpath_id,
            move_index: i + 1,
            sample_index: i,
            segment_time_s: 0.1,
            is_cutting: true,
            cut_kinematics: CutKinematics::Linear,
            feed_rate_mm_min: COMMANDED_FEED_MM_MIN,
            spindle_rpm: RPM,
            flute_count: FLUTES,
            axial_doc_mm: AXIAL_DOC_MM,
            axial_engagement_mm: AXIAL_DOC_MM,
            arc_engagement_radians: Some(arc_rad),
            chipload_mm_per_tooth: COMMANDED_FPT_MM,
            effective_chip_thickness_mm: Some(chip),
            engagement: Engagement::with_radial_woc(radial),
            removed_volume_est_mm3: 0.1,
            mrr_mm3_s: 1.0,
            ..SimulationCutSample::test_fixture()
        })
        .collect();
    (samples, chip)
}

struct Arm {
    /// Commanded advance per tooth, read back off the arm's own samples
    /// rather than assumed, so the fixture's premise is measured.
    commanded_mm_per_tooth: f64,
    /// Arc-mean chip thickness — what the heat-map colours by.
    chip_mm: f64,
    /// The gate's observation — achieved advance per tooth.
    gate_observed_mm_per_tooth: f64,
    /// The band the gate compared against, in advance per tooth.
    bounds: ChipBounds,
}

fn evaluate_pair() -> (Arm, Arm) {
    let tool = tool();
    let material = material();
    let (mut samples, chip_a) = arm(TP_A, ARC_A_RAD);
    let (samples_b, chip_b) = arm(TP_B, ARC_B_RAD);
    samples.extend(samples_b);

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

    let read = |toolpath_id: ToolpathId, chip_mm: f64| -> Arm {
        let commanded_mm_per_tooth = trace
            .samples
            .iter()
            .find(|s| s.toolpath_id == toolpath_id)
            .map(|s| s.feed_rate_mm_min / (f64::from(s.spindle_rpm) * f64::from(s.flute_count)))
            .expect("each arm contributes samples");
        let verdict = evaluate_toolpath(
            &ToolpathLoadContext {
                toolpath_id,
                tool: &tool,
                material: &material,
                operation_family: LutOperationFamily::Pocket,
                pass_role: LutPassRole::Roughing,
                operation_feed_rate_mm_min: COMMANDED_FEED_MM_MIN,
                operation_kind: OperationType::Pocket,
                spans: None,
                drill_op: None,
            },
            Some(&trace),
            None,
            &ToleranceBands::default(),
        );
        let (observed, bounds) = match &verdict.chipload {
            ChiploadVerdict::Within {
                approach_to_max, ..
            } => (
                approach_to_max.observed_mm_per_tooth,
                approach_to_max.bounds.clone(),
            ),
            ChiploadVerdict::Exceeds { triggering, .. } => {
                (triggering.observed_mm_per_tooth, triggering.bounds.clone())
            }
            ChiploadVerdict::Unmodeled { reason } => panic!(
                "the fixture must reach a modelled chipload verdict; got Unmodeled({reason:?})"
            ),
        };
        Arm {
            commanded_mm_per_tooth,
            chip_mm,
            gate_observed_mm_per_tooth: observed,
            bounds,
        }
    };

    (read(TP_A, chip_a), read(TP_B, chip_b))
}

/// The five colour classes of
/// `rs_cam_viz::render::toolpath_render::chipload_segment_color`
/// (verified 2026-08-08 at `toolpath_render.rs:720-747`). Mirrored here
/// rather than imported because the function is private to a crate this
/// wave does not own; A-2 owns the reconciliation.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum HeatMapClass {
    /// `< cl_min` — painted blue, captioned "Under-engaged — rubbing risk".
    RubbingBlue,
    /// `[cl_min, 1.1·cl_min)` — blue→green blend.
    BlendBlueGreen,
    /// `[1.1·cl_min, 0.9·cl_max)` — green.
    WithinGreen,
    /// `[0.9·cl_max, cl_max]` — orange.
    NearMaxOrange,
    /// `> cl_max` — red.
    OverMaxRed,
}

fn heat_map_class(bounds: &ChipBounds, value_mm: f64) -> HeatMapClass {
    // `chipload_envelopes_for_session` hands the renderer a
    // `Range<f64>`; a row with no published minimum contributes 0.0 as
    // the range start, which is what this mirrors.
    let cl_min = bounds.min_mm_per_tooth.unwrap_or(0.0);
    let cl_max = bounds.max_mm_per_tooth;
    if value_mm < cl_min {
        HeatMapClass::RubbingBlue
    } else if value_mm < cl_min * 1.1 {
        HeatMapClass::BlendBlueGreen
    } else if value_mm < cl_max * 0.9 {
        HeatMapClass::WithinGreen
    } else if value_mm <= cl_max {
        HeatMapClass::NearMaxOrange
    } else {
        HeatMapClass::OverMaxRed
    }
}

#[test]
fn changing_only_the_arc_moves_the_chip_thickness_and_not_the_advance_per_tooth() {
    let (a, b) = evaluate_pair();

    eprintln!(
        "\n  two-arc fixture — Ø{DIAMETER_MM} flat, {FLUTES}F, {RPM} RPM, \
         feed {COMMANDED_FEED_MM_MIN:.0} mm/min, DOC {AXIAL_DOC_MM} mm\n\
         \x20   arm A  arc {ARC_A_RAD:.2} rad (ae/D {:.4})\n\
         \x20   arm B  arc {ARC_B_RAD:.2} rad (ae/D {:.4})\n\
         \x20   commanded advance/tooth      A {:.6}  B {:.6}\n\
         \x20   gate observed advance/tooth  A {:.6}  B {:.6}\n\
         \x20   arc-mean chip thickness      A {:.6}  B {:.6}   (ratio {:.3}×)\n\
         \x20   band (advance/tooth)         {:?} .. {:.6}\n",
        radial_woc_fraction_for_arc(ARC_A_RAD),
        radial_woc_fraction_for_arc(ARC_B_RAD),
        a.commanded_mm_per_tooth,
        b.commanded_mm_per_tooth,
        a.gate_observed_mm_per_tooth,
        b.gate_observed_mm_per_tooth,
        a.chip_mm,
        b.chip_mm,
        b.chip_mm / a.chip_mm,
        a.bounds.min_mm_per_tooth,
        a.bounds.max_mm_per_tooth,
    );

    // 1. The fixture's premise, measured off the samples rather than
    //    assumed: the two arms were commanded identically.
    assert_eq!(
        a.commanded_mm_per_tooth, b.commanded_mm_per_tooth,
        "the two arms must differ only in engagement arc"
    );

    // 2. The gate's observation — the review's proposed display measure —
    //    does not move. Bit-equality, not a tolerance: `effective_feed /
    //    (rpm · flutes)` contains no arc term at all.
    assert_eq!(
        a.gate_observed_mm_per_tooth, b.gate_observed_mm_per_tooth,
        "achieved advance per tooth must be independent of engagement arc"
    );

    // 3. The two arms matched the same LUT row, so any colour difference
    //    below is a difference in the observed value, not in the band.
    assert_eq!(
        a.bounds, b.bounds,
        "both arms must be judged against the same vendor band"
    );

    // 4. The heat-map's quantity moves, and substantially.
    assert!(
        b.chip_mm > a.chip_mm * 1.5,
        "arc-mean chip thickness must move with the arc: A {} vs B {}",
        a.chip_mm,
        b.chip_mm
    );
}

#[test]
fn the_heat_map_paints_two_colours_for_one_advance_per_tooth() {
    let (a, b) = evaluate_pair();

    // What the viewport actually colours by today: the per-move max
    // `effective_chip_thickness_mm`, judged against the advance band.
    let displayed_a = heat_map_class(&a.bounds, a.chip_mm);
    let displayed_b = heat_map_class(&b.bounds, b.chip_mm);

    // What the same code path would show under the review's proposed
    // measure — the gate's own observation, which is what the band is
    // published in.
    let honest_a = heat_map_class(&a.bounds, a.gate_observed_mm_per_tooth);
    let honest_b = heat_map_class(&b.bounds, b.gate_observed_mm_per_tooth);

    eprintln!(
        "\n  heat-map colour classes against band {:?}..{:.6}\n\
         \x20   current measure (arc-mean chip):  A {displayed_a:?}   B {displayed_b:?}\n\
         \x20   review's measure (achieved a/t):  A {honest_a:?}   B {honest_b:?}\n",
        a.bounds.min_mm_per_tooth, a.bounds.max_mm_per_tooth,
    );

    // CHARACTERIZATION (pre-fix). Two toolpaths whose operator-visible
    // recipe is byte-identical are painted differently, because the
    // measure carries an arc term the band does not.
    assert_ne!(
        displayed_a, displayed_b,
        "pre-fix: the shipped heat-map measure must be shown to split one \
         recipe across two colour classes — if this ever stops holding, the \
         fixture no longer reproduces F-HEATMAP and must be re-derived, not \
         deleted"
    );

    // The review's measure does not split them. This is the assertion
    // A-2 keeps; the one above is the one A-2 inverts.
    assert_eq!(
        honest_a, honest_b,
        "the achieved advance per tooth must colour one recipe one colour"
    );

    // And the direction is the one `tool_load/mod.rs`'s caveat names:
    // the chip reads LOW against an advance band, so the lighter arc is
    // painted as rubbing risk when the gate says it is not. The caveat
    // quantifies the full-slot case at 1.6× (`1 / 0.6366`); arm A is
    // further out still, because the chip factor collapses with the arc.
    assert_eq!(
        displayed_a,
        HeatMapClass::RubbingBlue,
        "pre-fix: the light-arc pass is painted as rubbing risk"
    );
    assert_eq!(
        honest_a,
        HeatMapClass::WithinGreen,
        "the same pass, measured in the band's own unit, is comfortably in band"
    );
}
