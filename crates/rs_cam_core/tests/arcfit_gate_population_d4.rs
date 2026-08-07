//! **Checkpoint D Q3 (item D-4) — fitted arcs belong in the gate population.**
//!
//! Oracle: `planning/review_2026-08-04/SIMULATION_ISSUE_CHANNEL_CENSUS.md`
//! §7.2–§7.5 and §6.4's sub-finding; ruling recorded in
//! `planning/review_2026-08-04/ORCHESTRATION_LOG.md` "Checkpoint D — RULED
//! 2026-08-04", Q3.
//!
//! ## The defect these sentries pin
//!
//! `arcfit::fit_arcs` tagged every emitted arc
//! `Span::new(pos, pos + 1, SpanKind::DressupArtifact).with_label("arc-fit")`.
//! `toolpath_spans::transit_moves_bitmap` lists `DressupArtifact` among the
//! transit kinds, the dexel stamper copies that into
//! `SimulationCutSample::in_transit_span`, and
//! `tool_load::locality::is_phantom_transit` treats a `DressupArtifact`
//! ancestor as phantom transit. Net effect: **every arc-fitted cutting move
//! was dropped** from the chipload trip loop and LUT fold
//! (`tool_load/chipload.rs:386,582`), the deflection gate
//! (`tool_load/deflection.rs:240`), the power gate (`tool_load/power.rs:222`),
//! the viewport chipload band (`tool_load/mod.rs:236`) and both peak
//! accumulators (`simulation_cut.rs:1051,1134`).
//!
//! The written rationale for the exclusion
//! (`locality.rs:156-179`, `planning/archive/P3_TRANSIT_PEAK_DOC_RCA.md`)
//! covers a **dogbone bridge**: a corner-relief motion that flies over
//! adjacent uncleared stock, where the dexel reads `stock_top − cutter_z`
//! rather than engagement. A fitted arc is not a bridge — it is the same
//! cut, re-represented within `arc_tolerance`, engaging the same material.
//! The classification was over-broad by construction, not by decision.
//!
//! ## Why this is a POPULATION bar, not a verdict bar
//!
//! W5's brief (census §7.5) required this explicitly: a verdict bar passes
//! vacuously, because dropping samples from a gate usually *lowers* its peak
//! and therefore keeps the verdict `Within`. The bar below counts **how many
//! of the generator's own cutting samples reach the gate population**, on
//! the same fixture with `arc_fitting` off and on. It is expressed through
//! `SimulationCutSample::source_intent` (R-11, `ebe77de`), which is a SOURCE
//! key: it is what the generator emitted, so it survives arc-fitting and
//! every other post-transform relabel — exactly the handle a population bar
//! needs when the two arms have different move counts.
//!
//! ## Red-first record
//!
//! Measured at the parent revision `e93d748`, with `fit_arcs` still tagging
//! `SpanKind::DressupArtifact`:
//!
//! ```text
//! arc_fitting off: 270/270 FinishingCut samples in gate population (100.0%)
//! arc_fitting on : 0/151 FinishingCut samples in gate population (0.0%) across 1 fitted arc
//! narrated peak: sample 286 move 2 axial_doc 1.0000 mm, in_transit_span true
//! 30 dogbone samples, all excluded from the gate population
//! ```
//!
//! * `fitted_arcs_stay_in_the_gate_population` FAILS — 100.0 percentage
//!   points of the generator's own cutting samples leave the gate
//!   population when a downstream transform re-represents the geometry.
//! * `the_two_published_peaks_agree_on_an_arc_fitted_op` FAILS — the sample
//!   narration reports as the peak axial DOC carries
//!   `in_transit_span == true`, so `SummaryAccumulator` skips exactly the
//!   event narration publishes.
//! * `dogbone_bridge_samples_stay_out_of_the_gate_population` PASSES on the
//!   parent and must keep passing: it is the guard that the narrowing did
//!   not simply delete the exclusion.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use std::f64::consts::TAU;

use rs_cam_core::arcfit::fit_arcs;
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::dressup::apply_dogbones;
use rs_cam_core::geo::{BoundingBox3, P3};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::simulation_cut::SimulationCutSample;
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::tool_load::locality::{SpanLookup, is_steady_state_for_gate};
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};
use rs_cam_core::toolpath_spans::{AnnotatedToolpath, Span, SpanKind};

/// Ø2 flat end mill — 1.0 mm envelope radius. Arc-fit's large-radius cap is
/// `LARGE_ARC_RADIUS_MULTIPLIER` (30) × this = 30 mm, comfortably above the
/// 8 mm circle below, so the cap never decides an outcome here.
const TOOL_RADIUS_MM: f64 = 1.0;
/// Matches `DressupConfig::default()`.
const ARC_TOLERANCE_MM: f64 = 0.05;
/// Stock top; cuts sit 1 mm below it.
const STOCK_TOP_Z: f64 = 10.0;
const CUT_Z: f64 = 9.0;
const SAMPLE_STEP_MM: f64 = 0.25;

fn build_stock() -> TriDexelStock {
    let bbox = BoundingBox3 {
        min: P3::new(-12.0, -12.0, 0.0),
        max: P3::new(12.0, 12.0, STOCK_TOP_Z),
    };
    TriDexelStock::from_bounds(&bbox, 0.25)
}

/// A curve-heavy finishing pass: three quarters of a circle of radius 8 mm
/// walked as 135 short chords, every chord tagged `FinishingCut` at one
/// feed. This is the shape arc-fit exists for — the whole cut collapses into
/// arcs — and therefore the shape on which the `DressupArtifact` exclusion
/// removed the entire operation from every gate.
///
/// Three quarters, not a full turn: `fit_arc_through_points` refuses a run
/// whose start and end coincide (`arcfit.rs:362-366` — GRBL needs the R-form
/// for full circles and the fitter never emits it), so a closed loop would
/// weaken the fixture for a reason unrelated to what is being measured.
fn circular_finishing_pass() -> AnnotatedToolpath {
    const R: f64 = 8.0;
    const SEGMENTS: usize = 135;
    const SWEEP: f64 = 0.75 * TAU;

    let end_x = R * SWEEP.cos();
    let end_y = R * SWEEP.sin();

    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(R, 0.0, STOCK_TOP_Z + 2.0), MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(R, 0.0, CUT_Z), 200.0, MoveIntent::EntryPlunge);
    for k in 1..=SEGMENTS {
        let theta = SWEEP * (k as f64) / (SEGMENTS as f64);
        tp.feed_to_with_intent(
            P3::new(R * theta.cos(), R * theta.sin(), CUT_Z),
            1200.0,
            MoveIntent::FinishingCut,
        );
    }
    // Straight up, so the retract cannot cut and cannot colour either peak.
    tp.feed_to_with_intent(
        P3::new(end_x, end_y, STOCK_TOP_Z + 2.0),
        300.0,
        MoveIntent::Retract,
    );

    let n = tp.moves.len();
    AnnotatedToolpath::with_spans(tp, vec![Span::new(0, n, SpanKind::Operation)])
}

/// A square profile with inside corners — `apply_dogbones` inserts a real
/// overcut/return pair at each, tagged `DressupArtifact` / `"dogbone"`. This
/// is the motion the exclusion was written for.
fn square_profile_with_dogbones() -> AnnotatedToolpath {
    let mut tp = Toolpath::new();
    tp.rapid_to_with_intent(P3::new(-6.0, -6.0, STOCK_TOP_Z + 2.0), MoveIntent::Linking);
    tp.feed_to_with_intent(P3::new(-6.0, -6.0, CUT_Z), 200.0, MoveIntent::EntryPlunge);
    for corner in [(6.0, -6.0), (6.0, 6.0), (-6.0, 6.0), (-6.0, -6.0)] {
        tp.feed_to_with_intent(
            P3::new(corner.0, corner.1, CUT_Z),
            1200.0,
            MoveIntent::FinishingCut,
        );
    }
    let n = tp.moves.len();
    let annotated = AnnotatedToolpath::with_spans(tp, vec![Span::new(0, n, SpanKind::Operation)]);
    apply_dogbones(annotated, TOOL_RADIUS_MM, 170.0)
}

fn simulate(annotated: &AnnotatedToolpath) -> Vec<SimulationCutSample> {
    // Wired exactly as `compute/simulate.rs:675-701` wires production: span
    // paths and the transit bitmap both come off the annotated toolpath, so
    // whatever `transit_moves_bitmap` says about a span kind is what the
    // samples carry.
    let span_paths = annotated.span_paths_by_move();
    let transit = annotated.transit_moves_bitmap();
    let mut stock = build_stock();
    let cutter = FlatEndmill::new(TOOL_RADIUS_MM * 2.0, 25.0);
    let never_cancel = || false;
    stock
        .simulate_toolpath_with_metrics_with_cancel(
            &annotated.toolpath,
            &cutter,
            StockCutDirection::FromTop,
            ToolpathId(0),
            18_000,
            2,
            5000.0,
            SAMPLE_STEP_MM,
            None,
            &span_paths,
            &transit,
            true,
            &never_cancel,
        )
        .expect("simulation should complete")
}

/// How many of the generator's own cutting samples for `intent` reach the
/// gate population, and how many there were.
#[derive(Debug, Clone, Copy)]
struct Population {
    emitted: usize,
    in_gate: usize,
}

impl Population {
    fn fraction(self) -> f64 {
        if self.emitted == 0 {
            0.0
        } else {
            self.in_gate as f64 / self.emitted as f64
        }
    }
}

/// Select by SOURCE role (R-11), never by a post-transform label: arc-fit
/// rewrites move indices and span kinds, so any population keyed on those
/// would be measuring the transform rather than the cut.
fn gate_population(
    samples: &[SimulationCutSample],
    annotated: &AnnotatedToolpath,
    intent: MoveIntent,
) -> Population {
    let lookup = SpanLookup::new(&annotated.spans);
    let mine = samples
        .iter()
        .filter(|s| s.is_cutting && s.source_intent == Some(intent));
    let emitted = mine.clone().count();
    let in_gate = mine
        .filter(|s| is_steady_state_for_gate(s, Some(&lookup)))
        .count();
    Population { emitted, in_gate }
}

fn count_arcs(tp: &Toolpath) -> usize {
    tp.moves
        .iter()
        .filter(|m| {
            matches!(
                m.move_type,
                MoveType::ArcCW { .. } | MoveType::ArcCCW { .. }
            )
        })
        .count()
}

// ── The population bar ───────────────────────────────────────────────────

/// **D-4 population bar.** Same cut, `arc_fitting` off and on. The share of
/// the generator's `FinishingCut` samples that reach the gate population
/// must not depend on whether a downstream transform re-represented the
/// geometry as arcs.
///
/// Absolute sample counts legitimately differ between the arms — the
/// simulator samples by distance along each move, and 180 chords of 0.279 mm
/// each are sampled differently from the few long arcs they collapse into.
/// That is why the bar is a fraction and why the population is selected by
/// `source_intent` rather than by move index.
#[test]
fn fitted_arcs_stay_in_the_gate_population() {
    let unfitted = circular_finishing_pass();
    let fitted = fit_arcs(circular_finishing_pass(), ARC_TOLERANCE_MM, TOOL_RADIUS_MM);

    // Non-vacuity 1: the "on" arm must actually contain fitted arcs. Without
    // this the bar would pass on any fixture arc-fit declined to touch.
    let arcs = count_arcs(&fitted.toolpath);
    assert!(
        arcs > 0,
        "fixture must exercise arc-fitting; it produced no arcs"
    );

    let off = gate_population(&simulate(&unfitted), &unfitted, MoveIntent::FinishingCut);
    let on = gate_population(&simulate(&fitted), &fitted, MoveIntent::FinishingCut);

    println!(
        "arc_fitting off: {}/{} FinishingCut samples in gate population ({:.1}%)",
        off.in_gate,
        off.emitted,
        off.fraction() * 100.0
    );
    println!(
        "arc_fitting on : {}/{} FinishingCut samples in gate population ({:.1}%) \
         across {arcs} fitted arcs",
        on.in_gate,
        on.emitted,
        on.fraction() * 100.0
    );

    // Non-vacuity 2: the control arm must have a population at all.
    assert!(
        off.emitted > 0 && off.fraction() > 0.95,
        "control arm is broken: {off:?} — without arc-fitting essentially \
         every FinishingCut sample should reach the gate"
    );
    assert!(
        on.emitted > 0,
        "arc-fitted arm emitted no FinishingCut samples: {on:?}"
    );

    // The bar. Tolerance is 5 percentage points, which covers the handful of
    // samples at the arc/linear seams either arm can classify differently.
    let delta = (on.fraction() - off.fraction()).abs();
    assert!(
        delta <= 0.05,
        "arc-fitting moved {:.1} percentage points of the generator's cutting \
         samples out of the gate population (off {:.1}%, on {:.1}%). Fitted \
         arcs are the same cut re-represented; they belong in the population. \
         See census §7.4.",
        delta * 100.0,
        off.fraction() * 100.0,
        on.fraction() * 100.0
    );
}

// ── The dogbone guard (positive test for what stays excluded) ────────────

/// **The exclusion the P3 RCA earned must survive the narrowing.** A dogbone
/// overcut flies over neighbouring uncleared stock; the dexel reads
/// `stock_top − cutter_z` there, not engagement. Those samples stay out of
/// the gate population.
///
/// This test passes on the parent revision too — that is the point. It is
/// the guard against "fixing" §7 by deleting `DressupArtifact` from the
/// transit set, which census §7.5 warns against by name.
#[test]
fn dogbone_bridge_samples_stay_out_of_the_gate_population() {
    let annotated = square_profile_with_dogbones();

    let dogbones: Vec<&Span> = annotated
        .spans
        .iter()
        .filter(|s| s.kind == SpanKind::DressupArtifact)
        .collect();
    assert!(
        !dogbones.is_empty(),
        "fixture must produce real dogbone spans"
    );
    for d in &dogbones {
        assert_eq!(d.label, "dogbone", "only dogbones may be DressupArtifact");
    }
    let dogbone_moves: Vec<usize> = dogbones.iter().flat_map(|d| d.range()).collect();

    let transit = annotated.transit_moves_bitmap();
    for &m in &dogbone_moves {
        assert!(
            transit[m],
            "move {m} sits in a dogbone span and must be marked transit"
        );
    }

    let samples = simulate(&annotated);
    let lookup = SpanLookup::new(&annotated.spans);
    let mut checked = 0_usize;
    for s in &samples {
        if dogbone_moves.contains(&s.move_index) {
            checked += 1;
            assert!(
                !is_steady_state_for_gate(s, Some(&lookup)),
                "dogbone sample {} (move {}) reached the gate population",
                s.sample_index,
                s.move_index
            );
        }
    }
    assert!(
        checked > 0,
        "no sample landed on a dogbone move — the guard would be vacuous"
    );
    println!("{checked} dogbone samples, all excluded from the gate population");
}

// ── The two published peaks (census §6.4 sub-finding) ────────────────────

/// **Census §6.4's sub-finding, made into a test.** Two shipped surfaces
/// publish a "peak axial DOC" under different predicates:
///
/// * `SummaryAccumulator::observe` (`simulation_cut.rs:1134-1141`) —
///   `!sample.in_transit_span`, over `axial_engagement_mm`;
/// * `narrate::append_peak_doc_anomaly` (`narrate.rs:1444-1457`) —
///   `is_cutting` only, over `axial_doc_mm`, **no transit filter**.
///
/// On an arc-fitted op the `DressupArtifact` tag set `in_transit_span` on
/// every cutting sample, so narration reported a peak the summary had
/// silently skipped. W5 predicted the two could disagree "on exactly this
/// class of event". This asserts the disagreement is closed: the sample
/// narration selects is one the summary also sees.
#[test]
fn the_two_published_peaks_agree_on_an_arc_fitted_op() {
    let fitted = fit_arcs(circular_finishing_pass(), ARC_TOLERANCE_MM, TOOL_RADIUS_MM);
    assert!(count_arcs(&fitted.toolpath) > 0, "fixture must fit arcs");
    let samples = simulate(&fitted);

    // Narration's peak, mirrored exactly.
    let narrated = samples
        .iter()
        .filter(|s| s.is_cutting)
        .max_by(|a, b| {
            a.axial_doc_mm
                .partial_cmp(&b.axial_doc_mm)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .expect("fixture emits cutting samples");

    println!(
        "narrated peak: sample {} move {} axial_doc {:.4} mm, in_transit_span {}",
        narrated.sample_index, narrated.move_index, narrated.axial_doc_mm, narrated.in_transit_span
    );

    assert!(
        !narrated.in_transit_span,
        "the sample narration publishes as the peak axial DOC (sample {}, \
         move {}, {:.4} mm) is one SummaryAccumulator skips, so the two \
         published peaks disagree on it — census §6.4 sub-finding",
        narrated.sample_index, narrated.move_index, narrated.axial_doc_mm
    );

    // And the summary's own peak, computed with the shipped predicate, is
    // then a real reading rather than zero.
    let summary_peak = samples
        .iter()
        .filter(|s| !s.in_transit_span)
        .map(|s| s.axial_engagement_mm.max(0.0))
        .fold(0.0_f64, f64::max);
    assert!(
        summary_peak > 0.0,
        "summary peak axial DOC is 0 on a real cut — every sample was \
         filtered as transit"
    );
    println!("summary peak axial engagement: {summary_peak:.4} mm");
}

// ── The modelled verdict delta, on the synthetic fixture ─────────────────

/// **The verdict half of the ruling's evidence, where the gates actually
/// run.**
///
/// `verdict_delta_probe_on_the_committed_fixture` below measures the real
/// project fixture, but every gate there returns
/// `Unmodeled(StaleSimulation)` in BOTH arms — so it can report metric
/// deltas and population deltas, and nothing about verdicts. A table of
/// unmodelled verdicts is not a verdict-delta table.
///
/// This test closes that gap on the committed synthetic fixture: one
/// arc-fitted cutting pass, one hand-built `ToolpathLoadContext`, and the
/// shipped `tool_load::evaluate_toolpath` run twice — once on the trace as
/// the fixed code produces it, once on the same samples carrying the
/// parent's classification. Both traces are built through
/// `SimulationCutTrace::from_samples`, so the published peaks come from the
/// real `SummaryAccumulator` rather than a re-implementation of it here.
///
/// The assertions are on the INSTRUMENT, not on a verdict: the two arms
/// must genuinely differ in population, and the gates must reach a modelled
/// verdict at least once — otherwise the table is vacuous and says so. What
/// the verdicts actually do is printed and recorded, whichever way it falls.
#[test]
fn modelled_gate_verdicts_across_the_two_classifications() {
    use rs_cam_core::compute::catalog::OperationType;
    use rs_cam_core::compute::tool_config::ToolMaterial;
    use rs_cam_core::feeds::vendor_lut::{LutOperationFamily, LutPassRole};
    use rs_cam_core::material::Material;
    use rs_cam_core::simulation_cut::SimulationCutTrace;
    use rs_cam_core::tool::ToolDefinition;
    use rs_cam_core::tool_load::{ToleranceBands, ToolpathLoadContext, evaluate_toolpath};

    let fitted = fit_arcs(circular_finishing_pass(), ARC_TOLERANCE_MM, TOOL_RADIUS_MM);
    assert!(count_arcs(&fitted.toolpath) > 0, "fixture must fit arcs");
    let samples = simulate(&fitted);

    // Which moves are arc-fit spans — the only thing the fix changes.
    let mut refit = vec![false; fitted.toolpath.moves.len()];
    for span in fitted.spans.iter() {
        if span.kind == SpanKind::GeometryRefit {
            for m in span.range() {
                if let Some(slot) = refit.get_mut(m) {
                    *slot = true;
                }
            }
        }
    }

    // Parent arm: restore the transit flag the `DressupArtifact` tag used to
    // set. See the committed-fixture probe below for why this reproduces the
    // parent exactly (and for the one class where it would not).
    let parent_samples: Vec<SimulationCutSample> = samples
        .iter()
        .cloned()
        .map(|mut s| {
            if refit.get(s.move_index).copied().unwrap_or(false) {
                s.in_transit_span = true;
            }
            s
        })
        .collect();

    let flipped = parent_samples
        .iter()
        .zip(samples.iter())
        .filter(|(a, b)| a.in_transit_span != b.in_transit_span)
        .count();
    assert!(
        flipped > 0,
        "the two arms carry identical samples — nothing is being compared"
    );

    let trace_fixed = SimulationCutTrace::from_samples(SAMPLE_STEP_MM, samples);
    let trace_parent = SimulationCutTrace::from_samples(SAMPLE_STEP_MM, parent_samples);

    let tool = ToolDefinition::new(
        Box::new(FlatEndmill::new(TOOL_RADIUS_MM * 2.0, 25.0)),
        TOOL_RADIUS_MM * 2.0,
        30.0,
        20.0,
        30.0,
        2,
        ToolMaterial::Carbide,
    );
    let material = Material::default();
    let ctx = ToolpathLoadContext {
        toolpath_id: ToolpathId(0),
        tool: &tool,
        material: &material,
        operation_family: LutOperationFamily::Contour,
        pass_role: LutPassRole::Finish,
        operation_feed_rate_mm_min: 1200.0,
        operation_kind: OperationType::Profile,
        // The gates build their `SpanLookup` from here — without it
        // `is_phantom_transit` degrades to the flag-only fallback and the
        // ancestry half of the predicate is never exercised.
        spans: Some(&fitted.spans),
        drill_op: None,
    };
    let bands = ToleranceBands::default();

    // A machine profile, so the power gate models a verdict too rather than
    // declining with `NotImplemented("machine profile not provided")`.
    let machine = rs_cam_core::machine::MachineProfile::default();
    let vp = evaluate_toolpath(&ctx, Some(&trace_parent), Some(&machine), &bands);
    let vf = evaluate_toolpath(&ctx, Some(&trace_fixed), Some(&machine), &bands);

    let sp = trace_parent
        .toolpath_summaries
        .iter()
        .find(|s| s.toolpath_id == ToolpathId(0));
    let sf = trace_fixed
        .toolpath_summaries
        .iter()
        .find(|s| s.toolpath_id == ToolpathId(0));

    println!("\n=== D-4 MODELLED verdict delta — synthetic arc-fitted pass ===");
    println!("samples re-marked transit for the parent arm: {flipped}");
    if let (Some(p), Some(f)) = (sp, sf) {
        println!(
            "  peak_axial_doc_mm:          parent {:.4}  →  fixed {:.4}",
            p.peak_axial_doc_mm, f.peak_axial_doc_mm
        );
        println!(
            "  peak_chipload_mm_per_tooth: parent {:.6}  →  fixed {:.6}",
            p.peak_chipload_mm_per_tooth, f.peak_chipload_mm_per_tooth
        );
    }
    println!("  chipload   parent: {:?}", vp.chipload);
    println!("  chipload   fixed : {:?}", vf.chipload);
    println!("  power      parent: {:?}", vp.power);
    println!("  power      fixed : {:?}", vf.power);
    println!("  deflection parent: {:?}", vp.deflection);
    println!("  deflection fixed : {:?}", vf.deflection);

    let changed = [
        (
            "chipload",
            format!("{:?}", vp.chipload),
            format!("{:?}", vf.chipload),
        ),
        (
            "power",
            format!("{:?}", vp.power),
            format!("{:?}", vf.power),
        ),
        (
            "deflection",
            format!("{:?}", vp.deflection),
            format!("{:?}", vf.deflection),
        ),
    ];
    for (name, before, after) in &changed {
        if before != after {
            println!("  *** {name} CHANGED ***");
        }
    }

    // Non-vacuity on the INSTRUMENT: at least one gate must reach a modelled
    // verdict, or this table is three rows of "the gate declined" and proves
    // nothing either way.
    let modelled = [
        format!("{:?}", vf.chipload),
        format!("{:?}", vf.power),
        format!("{:?}", vf.deflection),
    ]
    .iter()
    .any(|d| !d.starts_with("Unmodeled"));
    assert!(
        modelled,
        "every gate declined on the fixed arm — the fixture cannot show a \
         verdict delta, so it must be repaired rather than reported"
    );
}

// ── The verdict-delta probe on a committed fixture ───────────────────────

/// **The measured verdict-delta instrument the Q3 ruling required.**
///
/// Runs the full session pipeline on the committed
/// `tests/fixtures/test_job.toml` — the same fixture W5's Rivers probe used,
/// whose `Project Curve 6` op carries 630 arc-fit spans (census §6.4) — and
/// prints, per toolpath, both arms side by side: the three tool-load gate
/// verdicts, the two published peaks, and the gate population.
///
/// ## Both arms come from ONE simulation, on purpose
///
/// The naive method — run the probe at the parent revision, run it again
/// with the fix, diff the two tables — is not sound on this tree. Several
/// agents commit to it concurrently, and a generation change landing between
/// the two runs would show up as a verdict delta this fix did not cause.
///
/// So the probe generates and simulates once, then derives the parent's
/// classification from the same trace: every sample whose move sits inside a
/// [`SpanKind::GeometryRefit`] span gets `in_transit_span = true`, which is
/// exactly what `transit_moves_bitmap` did when those spans were
/// `DressupArtifact`. Both arms then go through the shipped
/// `gcode::project_load_report`, which is the same assembly site
/// `ProjectSession::tool_load_report` uses.
///
/// **Equivalence, stated.** At the parent an arc-fitted sample was phantom
/// for two independent reasons: `DressupArtifact` ancestry, and the
/// `in_transit_span` flag. `is_phantom_transit` ORs them, so restoring the
/// flag reproduces the parent verdict for every sample **except** one class:
/// an arc nested inside an `Entry` span, where the parent's ancestry test
/// wins over the flag's `&& !Entry` guard. The probe counts that class and
/// prints it; when it is zero the emulation is exact.
///
/// It asserts nothing about verdict values on purpose: the deliverable is a
/// table. Asserting a verdict here would be the vacuous verdict bar the
/// ruling explicitly rejected. The non-vacuity assertions are on the
/// fixture: it must still generate, still simulate, and still fit arcs.
///
/// ```text
/// cargo test -p rs_cam_core --test arcfit_gate_population_d4 -- --ignored --nocapture
/// ```
#[test]
#[ignore = "expensive: generates + simulates the committed terrain fixture; run explicitly with --ignored"]
fn verdict_delta_probe_on_the_committed_fixture() {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;

    use rs_cam_core::ids::ToolpathId;
    use rs_cam_core::session::{ProjectSession, SimulationOptions};

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push("fixtures");
    path.push("test_job.toml");
    let mut session = ProjectSession::load(&path).expect("test_job.toml loads");

    let cancel = AtomicBool::new(false);
    let opts = SimulationOptions {
        resolution: 0.5,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    };
    let _ = session.generate_all(&[], &cancel);
    session
        .run_simulation(&opts, &cancel)
        .expect("first simulation completes");
    // `FromRemainingStock` ops need the simulated upstream stock, so
    // regenerate after the first sim and re-simulate (same shape as the M1
    // Rivers probe).
    let _ = session.generate_all(&[], &cancel);
    session
        .run_simulation(&opts, &cancel)
        .expect("second simulation completes");

    // `SimulationResult::cut_trace` is an `Arc`; deref before cloning so the
    // parent arm gets its own mutable copy rather than a second handle.
    let trace_fixed = session
        .simulation_result()
        .and_then(|s| s.cut_trace.as_deref())
        .expect("metric cut trace")
        .clone();

    // Per-toolpath mask of moves that sit inside an arc-fit span, plus the
    // Entry-nesting count that bounds the emulation's exactness.
    let mut refit_moves: HashMap<ToolpathId, Vec<bool>> = HashMap::new();
    let mut arc_spans_by_id: HashMap<ToolpathId, usize> = HashMap::new();
    let mut arcs_nested_in_entry = 0_usize;
    for (index, tc) in session.toolpath_configs().iter().enumerate() {
        let Some(result) = session.get_result(index) else {
            continue;
        };
        let annotated = result.annotated();
        let mut mask = vec![false; annotated.toolpath.moves.len()];
        let mut count = 0_usize;
        for span in annotated.spans.iter() {
            if span.kind != SpanKind::GeometryRefit {
                continue;
            }
            count += 1;
            for m in span.range() {
                if let Some(slot) = mask.get_mut(m) {
                    *slot = true;
                }
                if annotated
                    .span_path_at(m)
                    .iter()
                    .filter_map(|id| annotated.spans.get(id.0 as usize))
                    .any(|s| s.kind == SpanKind::Entry)
                {
                    arcs_nested_in_entry += 1;
                }
            }
        }
        arc_spans_by_id.insert(tc.id, count);
        refit_moves.insert(tc.id, mask);
    }

    // The parent classification, restored on the same samples.
    let mut trace_parent = trace_fixed.clone();
    let mut flipped = 0_usize;
    for sample in &mut trace_parent.samples {
        let in_refit = refit_moves
            .get(&sample.toolpath_id)
            .and_then(|m| m.get(sample.move_index))
            .copied()
            .unwrap_or(false);
        if in_refit && !sample.in_transit_span {
            sample.in_transit_span = true;
            flipped += 1;
        }
    }

    // `gcode::sim_trace_is_fresh` (gcode/mod.rs:301-311) returns false as soon
    // as ANY enabled toolpath has no compute result, and every gate then
    // reports `Unmodeled(StaleSimulation)` instead of a real verdict — which
    // would make the whole table read "no change" for a reason that has
    // nothing to do with this fix. On this fixture some ops never become
    // generatable. Disable exactly those, symmetrically for both arms; they
    // contributed no samples to either trace.
    let ungenerated: Vec<usize> = (0..session.toolpath_configs().len())
        .filter(|i| session.get_result(*i).is_none())
        .collect();
    for i in &ungenerated {
        if let Some(tc) = session.toolpath_configs_mut().get_mut(*i) {
            tc.enabled = false;
        }
    }
    println!(
        "disabled {} un-generated toolpath(s): {ungenerated:?}",
        ungenerated.len()
    );
    println!(
        "sim_trace_is_fresh: fixed={} parent={}",
        rs_cam_core::gcode::sim_trace_is_fresh(&session, &trace_fixed),
        rs_cam_core::gcode::sim_trace_is_fresh(&session, &trace_parent)
    );

    let report_fixed = rs_cam_core::gcode::project_load_report(&session, Some(&trace_fixed));
    let report_parent = rs_cam_core::gcode::project_load_report(&session, Some(&trace_parent));

    println!("\n=== D-4 verdict-delta table — committed test_job.toml ===");
    println!(
        "samples re-marked transit for the PARENT arm: {flipped}; \
         arc moves nested inside an Entry span: {arcs_nested_in_entry} \
         (0 ⇒ the parent emulation is exact)"
    );

    let mut total_arc_spans = 0_usize;
    let mut changed_rows = 0_usize;
    for (index, tc) in session.toolpath_configs().iter().enumerate() {
        let id = tc.id;
        let Some(result) = session.get_result(index) else {
            continue;
        };
        let annotated = result.annotated();
        let arc_spans = arc_spans_by_id.get(&id).copied().unwrap_or(0);
        total_arc_spans += arc_spans;

        let lookup = SpanLookup::new(&annotated.spans);
        let cutting_fixed: Vec<&SimulationCutSample> = trace_fixed
            .samples
            .iter()
            .filter(|s| s.toolpath_id == id && s.is_cutting)
            .collect();
        let cutting_parent: Vec<&SimulationCutSample> = trace_parent
            .samples
            .iter()
            .filter(|s| s.toolpath_id == id && s.is_cutting)
            .collect();
        let in_gate_fixed = cutting_fixed
            .iter()
            .filter(|s| is_steady_state_for_gate(s, Some(&lookup)))
            .count();
        let in_gate_parent = cutting_parent
            .iter()
            .filter(|s| is_steady_state_for_gate(s, Some(&lookup)))
            .count();

        // The two published peaks, recomputed under each arm's filter.
        // Mirrors `SummaryAccumulator::observe` (`simulation_cut.rs:1163`).
        let peaks = |samples: &[&SimulationCutSample]| -> (f64, f64) {
            let mut doc = 0.0_f64;
            let mut cl = 0.0_f64;
            for s in samples {
                if !s.in_transit_span {
                    doc = doc.max(s.axial_engagement_mm.max(0.0));
                    cl = cl.max(s.chipload_mm_per_tooth.max(0.0));
                }
            }
            (doc, cl)
        };
        // Peaks are accumulated over ALL samples, not only cutting ones.
        let all_fixed: Vec<&SimulationCutSample> = trace_fixed
            .samples
            .iter()
            .filter(|s| s.toolpath_id == id)
            .collect();
        let all_parent: Vec<&SimulationCutSample> = trace_parent
            .samples
            .iter()
            .filter(|s| s.toolpath_id == id)
            .collect();
        let (doc_fixed, cl_fixed) = peaks(&all_fixed);
        let (doc_parent, cl_parent) = peaks(&all_parent);

        let vf = report_fixed
            .per_toolpath
            .iter()
            .find(|v| v.toolpath_id == id);
        let vp = report_parent
            .per_toolpath
            .iter()
            .find(|v| v.toolpath_id == id);

        let row_changed = in_gate_fixed != in_gate_parent
            || (doc_fixed - doc_parent).abs() > 1e-9
            || (cl_fixed - cl_parent).abs() > 1e-12;
        let verdicts_changed = match (vf, vp) {
            (Some(a), Some(b)) => {
                format!("{:?}", a.chipload) != format!("{:?}", b.chipload)
                    || format!("{:?}", a.power) != format!("{:?}", b.power)
                    || format!("{:?}", a.deflection) != format!("{:?}", b.deflection)
            }
            _ => false,
        };
        if row_changed || verdicts_changed {
            changed_rows += 1;
        }

        println!(
            "\nTP{index} id={id} {:?}  [arc-fit spans: {arc_spans}]{}",
            tc.name,
            if verdicts_changed {
                "  *** VERDICT CHANGED ***"
            } else {
                ""
            }
        );
        println!(
            "  gate population (cutting):  parent {in_gate_parent}/{}  →  fixed {in_gate_fixed}/{}",
            cutting_parent.len(),
            cutting_fixed.len()
        );
        println!("  peak_axial_doc_mm:          parent {doc_parent:.4}  →  fixed {doc_fixed:.4}");
        println!("  peak_chipload_mm_per_tooth: parent {cl_parent:.6}  →  fixed {cl_fixed:.6}");
        if let (Some(a), Some(b)) = (vf, vp) {
            println!("  chipload   parent: {:?}", b.chipload);
            println!("  chipload   fixed : {:?}", a.chipload);
            println!("  power      parent: {:?}", b.power);
            println!("  power      fixed : {:?}", a.power);
            println!("  deflection parent: {:?}", b.deflection);
            println!("  deflection fixed : {:?}", a.deflection);
        }
    }

    println!("\nrows with any change: {changed_rows}");

    // Non-vacuity: if the fixture stops exercising arc-fitting, this probe
    // measures nothing and must say so rather than print a clean table.
    assert!(
        total_arc_spans > 0,
        "fixture produced no arc-fit spans — the probe is vacuous"
    );
    assert!(
        flipped > 0,
        "no sample changed classification — the two arms are the same trace"
    );
}
