//! O6: chip-thickness convention must match the LUT cap convention.
//!
//! The chipload gate (`tool_load::chipload::evaluate`) compares each
//! steady-state sample's `effective_chip_thickness_mm` against the
//! vendor LUT row's `chip_load_max_mm`. If the simulator and the LUT
//! disagree about which chip-thickness convention the value represents
//! (peak instantaneous on the engagement arc vs. arc-average vs. some
//! equivalent rectangular), the gate trips on otherwise-healthy cuts.
//!
//! Symptom on wanaka Back Rough (TP1): the simulator currently exposes
//! `geometry.max_chip_thickness_mm` to the gate. At arc engagement
//! `≥ π/2` (a common occurrence at adaptive corner cleanup or any
//! ≥50% radial step), the formula's `h_max = feed_per_tooth × sin(arc)`
//! peaks at `feed_per_tooth` itself — the unscaled commanded chipload.
//! Hardwood pocket roughing LUT caps live around 0.025–0.060 mm; the
//! commanded 0.0875 mm/tooth on wanaka exceeds that immediately. The
//! gate fires `Exceeds(ChiploadBreakageRisk)` regardless of axial DOC,
//! and the optimizer refuses with `BipolarEngagement`.
//!
//! Fix direction (per planning/AGENTSEARCH_NEXT_SESSION.md, option A):
//! switch the simulator to expose the AVERAGE chip thickness across
//! the engagement arc (`geometry.mean_chip_thickness_mm`). The mean
//! formula is the integral-average of `feed × sin(φ)` over the arc.
//!
//! ## Re-pinned 2026-08-06 — O6's premise was wrong, and the gate no
//! ## longer reads a chip thickness at all
//!
//! O6 assumed the vendor LUT chipload column was a chip thickness, and
//! chose the arc-average as the convention that matched it.
//! `planning/review_2026-08-04/CHIPLOAD_LITERATURE_VERDICT.md` retrieved
//! the vendor documents and found the column is a linear **advance per
//! tooth** — `feed ÷ (rpm × cutting edges)` — in every source family in
//! the shipped LUT, printed as a defining identity by Onsrud, Freud,
//! Amana and Garr and numerically self-verifying on the Amana charts.
//! Neither convention O6 chose between was the right one.
//!
//! So the chipload gate stopped normalising and stopped reading
//! `effective_chip_thickness_mm`; it observes
//! `effective_feed / (rpm · flutes)` (`tool_load::chipload`'s header).
//! Consequences for this file:
//!
//! - The formula check survives unchanged and is still worth having —
//!   `effective_chip_thickness_mm` is still consumed by
//!   `is_bipolar_engagement`, the MCP per-sample peak and the narration
//!   histogram. Only its *rationale sentence* was wrong and is corrected.
//! - **Both gate-verdict tests carried a fixture whose commanded feed
//!   and declared chipload disagreed by 6.5×** (`feed_rate_mm_min: 1000`
//!   at 18 000 rpm × 2 flutes is 0.0278 mm/tooth, not the 0.18 the
//!   fixture declared). The old observation came from the chip model, so
//!   the inconsistency was invisible. It is fixed here, and the
//!   difference it makes is pinned as an exhibit rather than quietly
//!   corrected — see
//!   [`the_pre_conversion_safe_feed_was_three_times_the_vendor_maximum`].

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use rs_cam_core::ids::ToolpathId;
use std::f64::consts::FRAC_PI_2;

use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::tool_config::ToolMaterial;
use rs_cam_core::dexel_stock::effective_chip_thickness_mm;
use rs_cam_core::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::simulation_cut::{
    CutKinematics, SimulationCutSample, SimulationCutSummary, SimulationCutTrace,
};
use rs_cam_core::tool::{EngagementMode, FlatEndmill, MillingCutter, ToolDefinition};
use rs_cam_core::tool_load::chipload;
use rs_cam_core::tool_load::verdict::Confidence;

/// Wanaka-like nominal chipload (mm/tooth) — matches the value that
/// trips the gate end-to-end on wanaka Back Rough.
const WANAKA_FEED_PER_TOOTH_MM: f64 = 0.0875;
/// Wanaka Back Rough commanded depth-per-pass.
const WANAKA_AXIAL_DOC_MM: f64 = 3.0;
/// Wanaka tool: 1.587mm 2-flute. We use a 6.0 mm 2-flute for the repro
/// so the LUT diameter exactly matches the calibrated row (the
/// `flat_end / pocket / roughing / hardwood` 6 mm row at janka 1450) —
/// avoids any extrapolation scaling that would otherwise shift the
/// chipload bounds and obscure what this test is checking
/// (chip-thickness convention, peak vs arc-average).
fn wanaka_tool() -> ToolDefinition {
    ToolDefinition::new(
        Box::new(FlatEndmill::new(6.0, 20.0)),
        6.0,
        30.0,
        20.0,
        30.0,
        2,
        ToolMaterial::Carbide,
    )
}

/// Sample template at half-engagement (arc = π/2), wanaka-commanded
/// feed/RPM/flutes. The `effective_chip_thickness_mm` is filled in by
/// the simulator helper `effective_chip_thickness_mm(...)` — this is
/// the line the fix changes.
#[allow(dead_code)]
fn half_engagement_sample(
    cutter: &dyn MillingCutter,
    tp_id: usize,
    idx: usize,
) -> SimulationCutSample {
    let arc = FRAC_PI_2;
    let chipload = WANAKA_FEED_PER_TOOTH_MM;
    let exposed = effective_chip_thickness_mm(cutter, WANAKA_AXIAL_DOC_MM, Some(arc), chipload, 2);
    SimulationCutSample {
        toolpath_id: ToolpathId(tp_id),
        move_index: idx,
        sample_index: idx,
        segment_time_s: 0.1,
        is_cutting: true,
        cut_kinematics: CutKinematics::Linear,
        feed_rate_mm_min: 1000.0,
        spindle_rpm: 18000,
        flute_count: 2,
        axial_doc_mm: WANAKA_AXIAL_DOC_MM,
        axial_engagement_mm: WANAKA_AXIAL_DOC_MM,
        arc_engagement_radians: Some(arc),
        chipload_mm_per_tooth: chipload,
        effective_chip_thickness_mm: exposed,
        engagement: rs_cam_core::simulation_cut::Engagement::with_radial_woc(0.5),
        removed_volume_est_mm3: 0.1,
        mrr_mm3_s: 1.0,
        ..SimulationCutSample::test_fixture()
    }
}

fn trace(samples: Vec<SimulationCutSample>) -> SimulationCutTrace {
    SimulationCutTrace {
        sample_step_mm: 1.0,
        summary: SimulationCutSummary {
            sample_count: samples.len(),
            toolpath_count: 1,
            issue_count: 0,
            hotspot_count: 0,
            total_runtime_s: 1.0,
            cutting_runtime_s: 1.0,
            rapid_runtime_s: 0.0,
            air_cut_time_s: 0.0,
            low_engagement_time_s: 0.0,
            average_engagement: 0.5,
            peak_chipload_mm_per_tooth: WANAKA_FEED_PER_TOOTH_MM,
            peak_axial_doc_mm: WANAKA_AXIAL_DOC_MM,
            peak_plunge_descent_mm: 0.0,
            total_removed_volume_est_mm3: 1.0,
            average_mrr_mm3_s: 1.0,
            per_kinematics: std::collections::BTreeMap::new(),
            runtime_by_intent: None,
        },
        samples,
        ..SimulationCutTrace::test_fixture()
    }
}

/// Formula-only check: at arc = π/2 the chip-thickness value the
/// simulator exposes to the gate must reflect arc-AVERAGE chip
/// thickness, not peak instantaneous. The peak is feed_per_tooth itself
/// (= 0.0875 on wanaka), which exceeds every hardwood-roughing LUT cap
/// even though the cut is actually well-tuned.
#[test]
fn exposed_chip_thickness_at_half_engagement_uses_arc_average_convention() {
    let tool = FlatEndmill::new(6.35, 20.0);

    let exposed = effective_chip_thickness_mm(
        &tool,
        WANAKA_AXIAL_DOC_MM,
        Some(FRAC_PI_2),
        WANAKA_FEED_PER_TOOTH_MM,
        2,
    )
    .expect("flat endmill chip geometry supported at half engagement");

    // Closed-form arc-average for h(φ) = feed × sin(φ) integrated over
    // an arc symmetric about φ = π/2:
    //   mean = (2 feed / arc) × (1 - cos(arc/2))
    // For arc = π/2, feed = 0.0875:
    //   mean = (0.175 / (π/2)) × (1 - cos(π/4))
    //        ≈ 0.1114 × 0.2929 ≈ 0.03264 mm.
    let expected_mean =
        (2.0 * WANAKA_FEED_PER_TOOTH_MM / FRAC_PI_2) * (1.0 - (FRAC_PI_2 * 0.5).cos());
    assert!(
        (exposed - expected_mean).abs() < 1e-6,
        "exposed chip thickness must equal arc-average mean ({expected_mean:.5}); \
         got {exposed:.5}. The peak-instantaneous convention \
         (feed × sin(arc) = {peak:.5}) overstates the mean chip by ~2.6×. \
         (Neither convention is what the vendor LUT column publishes — that \
         is a linear advance per tooth; see this file's header. This check \
         is about the simulator's own exposed quantity, which other \
         consumers still read.)",
        peak = WANAKA_FEED_PER_TOOTH_MM,
    );

    // Also verify the trait-level chip_geometry exposes both
    // conventions distinctly so the simulator's pick is unambiguous.
    let geom = tool
        .chip_geometry(
            WANAKA_AXIAL_DOC_MM,
            FRAC_PI_2,
            WANAKA_FEED_PER_TOOTH_MM,
            2,
            EngagementMode::Slot,
        )
        .expect("flat geometry supported");
    assert!(
        (geom.max_chip_thickness_mm - WANAKA_FEED_PER_TOOTH_MM).abs() < 1e-9,
        "geometry struct still reports peak (max) — kept for consumers \
         that want the instantaneous value"
    );
    assert!(
        geom.mean_chip_thickness_mm < geom.max_chip_thickness_mm,
        "mean must be strictly below max at partial engagement"
    );
}

/// Gate-verdict check, on the axis the vendor actually publishes: a
/// sample whose **advance per tooth** lands inside the LUT's published
/// band must pass the chipload gate.
///
/// Re-pinned 2026-08-06. What changed, exactly:
///
/// | | before | after |
/// |---|---|---|
/// | fixture `feed_rate_mm_min` | 1000 (= 0.0278 mm/tooth) | 1620 (= 0.045 mm/tooth) |
/// | fixture `chipload_mm_per_tooth` | 0.18 — **inconsistent with the feed** | 0.045 — consistent |
/// | what the gate observed | arc-mean chip 0.0419 mm | advance 0.045 mm/tooth |
/// | band (HardMaple / Pocket / Roughing / Ø6) | 0.032 – 0.055 | unchanged |
/// | verdict | `Within(Validated)` | `Within(Validated)` |
///
/// The verdict does not move, and that is the point of keeping this
/// test: it is the wave's `VendorLut`-source control. What moved is that
/// the fixture now says one thing instead of two, and the number the
/// gate quotes is the number an operator can dial.
///
/// The old docstring computed the "published envelope" in feed terms as
/// `min_mean / M(arc_lut) ≈ 0.032 / 0.233 ≈ 0.137 mm/tooth`. That
/// division is exactly the invalid conversion the literature verdict
/// removed; the envelope in feed terms is simply 0.032 – 0.055.
#[test]
fn lut_nominal_engagement_sample_within_published_envelope_passes() {
    let verdict = gate_verdict_at(0.045, (1.0_f64 - 2.0 * 1.6 / 6.0).acos(), 0.27);
    match verdict {
        rs_cam_core::tool_load::ChiploadVerdict::Within {
            confidence: Confidence::Validated,
            ..
        } => {}
        other => panic!("unexpected verdict: {other:?}"),
    }
}

/// **The exhibit.** The pre-conversion fixture called 0.18 mm/tooth "a
/// feed that lands safely inside the LUT envelope", having derived that
/// claim by dividing the vendor band by an arc factor. Measured on the
/// axis the vendor publishes, 0.18 mm/tooth is **3.3× the band
/// maximum** — a hardwood pocket rough commanded at more than three
/// times the published limit, which the pre-fix gate reported as
/// `Within`.
///
/// This is the breakage-side twin of the B3 finding (which ran the other
/// way: an operation at 99.9 % of its ceiling reported as 16 % of its
/// floor). Both are the same defect; only the direction differs, and the
/// direction depends on the row's `ae` window, which verdict §2.3 shows
/// is repo-authored.
///
/// Kept permanently rather than deleted once green, per
/// `boundary_clip_escape_f1`'s precedent: the claim a future reader will
/// doubt is not "the fix works" but "the defect was real".
#[test]
fn the_pre_conversion_safe_feed_was_three_times_the_vendor_maximum() {
    let verdict = gate_verdict_at(0.18, (1.0_f64 - 2.0 * 1.6 / 6.0).acos(), 0.27);
    let rs_cam_core::tool_load::ChiploadVerdict::Exceeds {
        side: rs_cam_core::tool_load::verdict::ChipSide::High,
        triggering,
        ..
    } = verdict
    else {
        panic!(
            "0.18 mm/tooth of advance against a 0.032-0.055 band must now trip \
             Exceeds(High); got {verdict:?}"
        )
    };
    let max = triggering.bounds.max_mm_per_tooth;
    let over = triggering.observed_mm_per_tooth / max;
    eprintln!(
        "  EXHIBIT: the pre-conversion 'safe' feed 0.18 mm/tooth is {over:.2}x the \
         band maximum {max:.4} mm/tooth. The pre-fix gate reported Within."
    );
    assert!(
        over > 3.0,
        "the exhibit is only worth keeping if the overshoot is large; got {over:.2}x"
    );
}

/// Restatement of `slot_engagement_sample_at_safe_feed_passes_after_d9_normalization`.
///
/// D9 existed so a slot-engagement transient on terrain would not be
/// over-penalised: its raw arc-mean chip is `0.637 × feed`, far above a
/// band the row published at ~27 % radial, so the comparison had to be
/// renormalised. With the gate observing an advance per tooth the
/// problem does not arise — the observation carries **no engagement term
/// at all**, which the census proved algebraically (§4.3: the sample's
/// own arc cancelled even before the deletion) and which this test now
/// asserts directly.
///
/// Two samples at the same feed and wildly different arcs (slot π vs
/// 27 % radial) must produce the *identical* verdict and the *identical*
/// observed value. That is a stronger claim than the D9 test made, and
/// it is the honest one: the gate is no longer engagement-aware, and a
/// reader should be able to see that from a test rather than infer it.
#[test]
fn slot_engagement_no_longer_needs_normalisation_because_the_arc_is_gone() {
    use std::f64::consts::PI;

    let slot = gate_verdict_at(0.045, PI, 1.0);
    let narrow = gate_verdict_at(0.045, (1.0_f64 - 2.0 * 1.6 / 6.0).acos(), 0.27);

    // Non-vacuity: the two arcs really do present different raw chips.
    let cutter = FlatEndmill::new(6.0, 20.0);
    let chip_slot = effective_chip_thickness_mm(&cutter, WANAKA_AXIAL_DOC_MM, Some(PI), 0.045, 2)
        .expect("slot chip geometry supported");
    let chip_narrow = effective_chip_thickness_mm(
        &cutter,
        WANAKA_AXIAL_DOC_MM,
        Some((1.0_f64 - 2.0 * 1.6 / 6.0).acos()),
        0.045,
        2,
    )
    .expect("narrow chip geometry supported");
    assert!(
        chip_slot / chip_narrow > 2.0,
        "the two arcs must produce materially different raw chips or this proves \
         nothing (got {chip_slot:.5} vs {chip_narrow:.5})"
    );

    let observed = |v: &rs_cam_core::tool_load::ChiploadVerdict| match v {
        rs_cam_core::tool_load::ChiploadVerdict::Within {
            approach_to_max, ..
        } => approach_to_max.observed_mm_per_tooth,
        other => panic!("both arms must land Within; got {other:?}"),
    };
    let (o_slot, o_narrow) = (observed(&slot), observed(&narrow));
    assert!(
        (o_slot - o_narrow).abs() < 1e-12,
        "the observation must not depend on engagement at all: slot {o_slot:.12} \
         vs narrow {o_narrow:.12}, from raw chips differing by {:.2}x",
        chip_slot / chip_narrow
    );
    eprintln!(
        "  CONFIRMED: raw chip differs {:.2}x between slot and 27 % radial; the gate's \
         observation is identical to 1e-12. The gate reports advance per tooth.",
        chip_slot / chip_narrow
    );
}

/// Run the shipped gate on a one-sample trace whose commanded feed,
/// declared chipload and observed advance per tooth all agree.
///
/// The pre-conversion fixtures set `feed_rate_mm_min: 1000` beside
/// `chipload_mm_per_tooth: 0.18` — two statements about the same
/// operation that differ by 6.5×. This helper makes that impossible to
/// write.
fn gate_verdict_at(
    feed_per_tooth: f64,
    arc_rad: f64,
    radial_woc: f64,
) -> rs_cam_core::tool_load::ChiploadVerdict {
    const RPM: u32 = 18_000;
    const FLUTES: u32 = 2;
    let feed_rate_mm_min = feed_per_tooth * f64::from(RPM) * f64::from(FLUTES);
    let cutter = FlatEndmill::new(6.0, 20.0);
    let exposed = effective_chip_thickness_mm(
        &cutter,
        WANAKA_AXIAL_DOC_MM,
        Some(arc_rad),
        feed_per_tooth,
        FLUTES,
    )
    .expect("flat endmill chip geometry supported at this arc");

    let sample = SimulationCutSample {
        segment_time_s: 0.1,
        is_cutting: true,
        cut_kinematics: CutKinematics::Linear,
        feed_rate_mm_min,
        spindle_rpm: RPM,
        flute_count: FLUTES,
        axial_doc_mm: WANAKA_AXIAL_DOC_MM,
        axial_engagement_mm: WANAKA_AXIAL_DOC_MM,
        arc_engagement_radians: Some(arc_rad),
        chipload_mm_per_tooth: feed_per_tooth,
        effective_chip_thickness_mm: Some(exposed),
        engagement: rs_cam_core::simulation_cut::Engagement::with_radial_woc(radial_woc),
        removed_volume_est_mm3: 0.1,
        mrr_mm3_s: 1.0,
        ..SimulationCutSample::test_fixture()
    };
    let trace = trace(vec![sample]);
    let material = Material::SolidWood {
        species: WoodSpecies::HardMaple,
    };
    let tolerance = rs_cam_core::tool_load::ToleranceBands::default();
    chipload::evaluate(
        &rs_cam_core::tool_load::ToolpathLoadContext {
            toolpath_id: ToolpathId(0),
            tool: &wanaka_tool(),
            material: &material,
            operation_family: LutOperationFamily::Pocket,
            pass_role: LutPassRole::Roughing,
            operation_feed_rate_mm_min: feed_rate_mm_min,
            operation_kind: OperationType::Pocket,
            spans: None,
            drill_op: None,
        },
        &rs_cam_core::tool_load::GateEnv {
            sim_trace: Some(&trace),
            machine: None,
            tolerance: &tolerance,
        },
    )
}
