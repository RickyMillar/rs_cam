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
//! A **sentry**. A-1 committed it green as a characterization of the
//! shipped defect; A-2 inverted it on 2026-08-08 under Checkpoint H, and
//! the assertion that used to read "the display measure splits one
//! recipe across two colour classes" now reads that it must **not**.
//!
//! The pre-fix numbers are not reconstructed from history — they are
//! recorded permanently below, in [`PRE_FIX`], and
//! [`the_retired_measure_still_reproduces_the_defect`] keeps computing
//! the retired quantity from production code so the record cannot rot
//! into a comment that is no longer true.
//!
//! The red run this inversion was verified against is quoted in
//! `planning/review_2026-08-08/ORCHESTRATION_LOG.md`, A-2's entry.
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
//! 2026-08-06).
//!
//! Until 2026-08-08 the **viewport heat-map coloured by the third against
//! a band for the first**: `rs_cam_viz::app::gpu_upload::build_chipload_per_move`
//! took `max(effective_chip_thickness_mm)` per move and handed it to
//! `rs_cam_viz::render::toolpath_render::chipload_segment_color` beside
//! `rs_cam_core::tool_load::chipload_envelopes_for_session`'s advance
//! band. Three further viz surfaces shared or blended the defect (the
//! sim-timeline "chipload" track, its "load vs limit" normalised summary,
//! and the Cut-Metrics "Chipload" row) — see
//! `planning/review_2026-08-08/HEATMAP_VOCAB_CENSUS.md` §2.
//!
//! Checkpoint H moved all four onto
//! [`rs_cam_core::tool_load::display::achieved_advance_per_tooth`], and
//! moved the five colour classes into
//! [`rs_cam_core::feeds::VendorChiploadBand::classify`] so this test can
//! assert on the **production** classifier instead of a mirror of it.
//! That relocation is what makes the assertions below load-bearing: a
//! future edit to the viewport's measure has to come through the two
//! functions this file calls.
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
use rs_cam_core::feeds::{
    AdvancePerToothMm, ArcMeanChipThicknessMm, ChiploadBandClass, VendorChiploadBand,
};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::machine_kinematics::PredictedFeedMap;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::simulation_cut::{
    CutKinematics, Engagement, SimulationCutSample, SimulationCutTrace,
};
use rs_cam_core::tool::{FlatEndmill, ToolDefinition};
use rs_cam_core::tool_load::display;
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

/// The pre-fix record, kept permanently. Measured on `becf1cb`
/// (A-1's characterization run) and reproduced by
/// [`the_retired_measure_still_reproduces_the_defect`] every time this
/// file runs, so it is a live number rather than a comment.
///
/// ```text
///   two-arc fixture — Ø6 flat, 2F, 18000 RPM, feed 2520 mm/min, DOC 1.5 mm
///     arm A  arc 0.80 rad (ae/D 0.1516)
///     arm B  arc 3.14 rad (ae/D 1.0000)
///     commanded advance/tooth      A 0.070000  B 0.070000
///     gate observed advance/tooth  A 0.043400  B 0.043400
///     arc-mean chip thickness      A 0.009910  B 0.044563   (ratio 4.497×)
///     band (advance/tooth)         Some(0.032) .. 0.055000
///
///   heat-map colour classes against band Some(0.032)..0.055000
///     current measure (arc-mean chip):  A RubbingBlue   B WithinGreen
///     review's measure (achieved a/t):  A WithinGreen   B WithinGreen
/// ```
mod pre_fix {
    /// Arm A's arc-mean chip thickness, the value the shipped heat-map
    /// coloured by. Painted `BelowBand` ("Under-engaged — rubbing risk")
    /// against a band whose floor is 0.032.
    pub const CHIP_A_MM: f64 = 0.009_910;
    /// Arm B's, on the identical recipe.
    pub const CHIP_B_MM: f64 = 0.044_563;
    /// The ratio the census quotes.
    pub const CHIP_RATIO: f64 = 4.497;
    /// The gate's observation, bit-equal on both arms.
    pub const GATE_OBSERVED_MM: f64 = 0.043_400;
}

struct Arm {
    /// Commanded advance per tooth, read back off the arm's own samples
    /// rather than assumed, so the fixture's premise is measured.
    commanded_mm_per_tooth: f64,
    /// Arc-mean chip thickness — the **retired** display measure, still
    /// computed here so the pre-fix record stays live.
    chip: ArcMeanChipThicknessMm,
    /// What the viewport heat-map actually colours by today, taken from
    /// the production builder
    /// [`display::advance_per_tooth_per_move`] rather than recomputed.
    displayed: AdvancePerToothMm,
    /// The gate's observation — achieved advance per tooth.
    gate_observed_mm_per_tooth: f64,
    /// The band the gate compared against, in advance per tooth.
    bounds: ChipBounds,
}

impl Arm {
    /// The band as the display path sees it. Asserted equal across the
    /// pair before any colour is compared.
    fn band(&self) -> VendorChiploadBand {
        VendorChiploadBand::new(
            AdvancePerToothMm::new(self.bounds.min_mm_per_tooth.unwrap_or(0.0)),
            AdvancePerToothMm::new(self.bounds.max_mm_per_tooth),
        )
    }
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

    // The production display map — one call, both arms, exactly the map
    // the viewport hands to the colour function.
    let per_move = display::advance_per_tooth_per_move(Some(&trace));

    let read = |toolpath_id: ToolpathId, chip_mm: f64| -> Arm {
        let displayed = *per_move
            .get(&toolpath_id)
            .and_then(|m| m.values().next())
            .expect("the production display builder covers every cutting sample");
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
            chip: ArcMeanChipThicknessMm::new(chip_mm),
            displayed,
            gate_observed_mm_per_tooth: observed,
            bounds,
        }
    };

    (read(TP_A, chip_a), read(TP_B, chip_b))
}

/// The colour classes are no longer mirrored here.
///
/// A-1 had to transcribe `rs_cam_viz::render::toolpath_render`'s five
/// thresholds into this file, because the function was private to a
/// crate this test cannot reach — and said so, flagging the
/// reconciliation as A-2's. A-2 did the reconciliation the other way
/// round: the classification moved **into core** as
/// [`VendorChiploadBand::classify`], so the viewport's colour and this
/// test's assertion are now the same call. `toolpath_render` keeps only
/// the class → RGB map, pinned by its own unit tests, and no RGB triple
/// or threshold moved in the process.
///
/// The retired measure — an arc-mean chip thickness pushed through that
/// same classifier — has no production caller any more, so
/// [`the_retired_measure_still_reproduces_the_defect`] reconstructs it
/// explicitly, and its docstring says that is what it is doing.
fn class_of(arm: &Arm, value: AdvancePerToothMm) -> ChiploadBandClass {
    arm.band().classify(value)
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
         \x20   DISPLAYED (heat-map)         A {:.6}  B {:.6}\n\
         \x20   band (advance/tooth)         {:?} .. {:.6}\n",
        radial_woc_fraction_for_arc(ARC_A_RAD),
        radial_woc_fraction_for_arc(ARC_B_RAD),
        a.commanded_mm_per_tooth,
        b.commanded_mm_per_tooth,
        a.gate_observed_mm_per_tooth,
        b.gate_observed_mm_per_tooth,
        a.chip.mm(),
        b.chip.mm(),
        b.chip.mm() / a.chip.mm(),
        a.displayed.mm(),
        b.displayed.mm(),
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

    // 4. The arc-mean chip thickness moves, and substantially. This is
    //    correct physics, not the defect — the defect was comparing it to
    //    a band published in another quantity.
    assert!(
        b.chip.mm() > a.chip.mm() * 1.5,
        "arc-mean chip thickness must move with the arc: A {} vs B {}",
        a.chip.mm(),
        b.chip.mm()
    );

    // 5. **The inversion.** The quantity the viewport now displays is the
    //    gate's own observation, taken from the production builder — not
    //    recomputed here. Bit-equality across the pair, and bit-equality
    //    with the gate.
    assert_eq!(
        a.displayed, b.displayed,
        "the displayed measure must not move when only the engagement arc moves"
    );
    assert_eq!(
        a.displayed.mm(),
        a.gate_observed_mm_per_tooth,
        "the heat-map and the gate must be reading one expression, not two \
         transcriptions of one intention"
    );
    assert_eq!(b.displayed.mm(), b.gate_observed_mm_per_tooth);
}

#[test]
fn the_heat_map_paints_one_colour_for_one_advance_per_tooth() {
    let (a, b) = evaluate_pair();

    // What the viewport colours by, straight out of the production
    // builder and the production classifier.
    let displayed_a = class_of(&a, a.displayed);
    let displayed_b = class_of(&b, b.displayed);

    eprintln!(
        "\n  heat-map colour classes against band {:?}..{:.6}\n\
         \x20   displayed measure (achieved a/t):  A {displayed_a:?}   B {displayed_b:?}\n\
         \x20   retired measure (arc-mean chip):   A {:?}   B {:?}\n",
        a.bounds.min_mm_per_tooth,
        a.bounds.max_mm_per_tooth,
        a.band().classify(AdvancePerToothMm::new(a.chip.mm())),
        b.band().classify(AdvancePerToothMm::new(b.chip.mm())),
    );

    // Both arms must be judged against the same band, or a colour
    // difference would be a band difference and prove nothing.
    assert_eq!(a.bounds, b.bounds);

    // **THE BAR (inverted from A-1's characterization at Checkpoint H).**
    // Two toolpaths whose operator-visible recipe is byte-identical must
    // be painted identically. A-1 committed the opposite assertion
    // (`assert_ne!`) as a characterization of the shipped defect; this is
    // the assertion that fails if the defect returns.
    assert_eq!(
        displayed_a, displayed_b,
        "one recipe, one colour: the displayed measure must not carry an \
         engagement-arc term the vendor band does not carry"
    );

    // And it must be the correct colour, not merely a consistent one —
    // an all-grey heat-map would also pass the assertion above.
    assert_eq!(
        displayed_a,
        ChiploadBandClass::Within,
        "arm A is comfortably mid-band in the band's own unit and must be \
         painted as such; pre-fix it was painted BelowBand (rubbing risk)"
    );
    assert_eq!(displayed_b, ChiploadBandClass::Within);
}

/// The pre-fix reproduction, kept permanently and kept **live**.
///
/// Rule 1 of the programme's operating rules: "the pre-fix reproduction
/// stays in the file permanently". A number in a comment decays; this
/// recomputes the retired quantity from the production chip model and
/// pushes it through the production classifier, so if either ever moves,
/// the recorded values in [`pre_fix`] stop matching and this test says so
/// rather than the census quietly becoming wrong.
///
/// **This is not an assertion about shipped behaviour.** No production
/// caller feeds an arc-mean chip thickness to
/// [`VendorChiploadBand::classify`] any more; the `AdvancePerToothMm`
/// wrapper below is written by hand, deliberately, and is the only place
/// in the repository that performs that (meaningless) conversion. It
/// exists to demonstrate what the operator used to see.
#[test]
fn the_retired_measure_still_reproduces_the_defect() {
    let (a, b) = evaluate_pair();

    // 1. The recorded pre-fix numbers still hold.
    assert!(
        (a.chip.mm() - pre_fix::CHIP_A_MM).abs() < 5e-7,
        "recorded pre-fix arm-A chip thickness {} no longer matches the \
         production chip model's {} — update the census, do not delete the record",
        pre_fix::CHIP_A_MM,
        a.chip.mm()
    );
    assert!((b.chip.mm() - pre_fix::CHIP_B_MM).abs() < 5e-7);
    assert!((b.chip.mm() / a.chip.mm() - pre_fix::CHIP_RATIO).abs() < 5e-4);
    assert!((a.gate_observed_mm_per_tooth - pre_fix::GATE_OBSERVED_MM).abs() < 5e-7);

    // 2. The retired measure — reconstructed by hand — still splits one
    //    recipe across two colour classes. This is the defect, preserved.
    let retired_a = a.band().classify(AdvancePerToothMm::new(a.chip.mm()));
    let retired_b = b.band().classify(AdvancePerToothMm::new(b.chip.mm()));
    assert_ne!(
        retired_a, retired_b,
        "the retired measure must keep reproducing F-HEATMAP; if it stops, the \
         fixture no longer demonstrates what was fixed and must be re-derived, \
         not deleted"
    );
    assert_eq!(
        retired_a,
        ChiploadBandClass::BelowBand,
        "pre-fix, the light-arc pass was painted as rubbing risk"
    );
    assert_eq!(retired_b, ChiploadBandClass::Within);

    // 3. And the direction is the one `tool_load/mod.rs`'s caveat named:
    //    the chip reads LOW against an advance band. The caveat quantifies
    //    the full-slot case at 1.6× (`1 / 0.6366`); arm A is further out
    //    still, because the chip factor collapses with the arc.
    assert!(a.chip.mm() < a.gate_observed_mm_per_tooth);
    assert!(b.chip.mm() > b.gate_observed_mm_per_tooth * 0.9);
}
