//! Red-first sentries for the drill reporting defects W6 audited and
//! Checkpoint D approved (2026-08-04).
//!
//! `planning/review_2026-08-04/DRILL_GATE_EVIDENCE_AUDIT.md` §7 and §10.
//! Every test here asserts a *reporting* property. **No test in this
//! file asserts a threshold value**, and no fix driven by these tests
//! is permitted to move one — the audit's own summary is that "the
//! drill gates measure the right quantities in the right units, and
//! then tell the operator something that is not true about the answer".
//!
//! Each test quotes the shipped wrong output it was written against, so
//! the defect stays legible after the fix.
//!
//! Coverage: R-1 (Exceeds text on a below-threshold observation), R-3
//! (Within displaying a bound it was not compared against), R-4 (the
//! remedy that contradicts the cycle), R-5 (evacuation score 1.00 beside
//! "INADEQUATE"), R-6 (the ratio computed twice and exposed zero
//! times), R-7 (evidence pointing at sample 0), R-2 (the summary models
//! a cycle the emitter does not emit).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use rs_cam_core::diagnostics::adapters::from_tool_load::diagnostics_from_load_verdict;
use rs_cam_core::diagnostics::{DiagnosticEvidence, Severity, ids};
use rs_cam_core::drill::{DrillCycle, DrillParams, drill_toolpath};
use rs_cam_core::drill_metrics::{build_drill_toolpath_summary, emit_drill_samples};
use rs_cam_core::drill_op::{DrillHole, DrillOp, HoleSource, ToolProfile};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::material::Material;
use rs_cam_core::tool_load::drill_gates::{DrillGateOutcome, DrillGatesVerdict};
use rs_cam_core::tool_load::verdict::{
    ChiploadVerdict, DeflectionVerdict, PowerVerdict, ToolpathLoadVerdict, UnmodeledReason,
};

/// The R-plane every shipped drill op actually gets:
/// `effective_safe_z` = `max(cfg.retract_z, stock_top + 5.0)`, and the
/// `DrillConfig` default `retract_z` is 2.0, so it is always
/// `stock_top + 5.0` on a default project.
const R_PLANE_ABOVE_STOCK_MM: f64 = 5.0;

fn op(cycle: DrillCycle, diameter: f64, depth: f64, feed: f64) -> DrillOp {
    DrillOp {
        holes: vec![DrillHole {
            xy: [0.0, 0.0],
            top_z: 0.0,
            bottom_z: -depth,
        }],
        hole_source: HoleSource::ModelDerived,
        tool_profile: ToolProfile::StandardTwist,
        tool_diameter_mm: diameter,
        cycle,
        feed_rate_mm_min: feed,
        spindle_rpm: 18_000,
        flute_count: 2,
        material: Material::default(),
    }
}

fn gates(d: &DrillOp) -> DrillGatesVerdict {
    let samples = emit_drill_samples(ToolpathId(0), d);
    let summary = build_drill_toolpath_summary(ToolpathId(0), d, &samples);
    rs_cam_core::tool_load::drill_gates::evaluate(d, &summary)
}

fn verdict_with(drill: DrillGatesVerdict) -> ToolpathLoadVerdict {
    ToolpathLoadVerdict {
        toolpath_id: ToolpathId(0),
        chipload: ChiploadVerdict::Unmodeled {
            reason: UnmodeledReason::NotApplicableForOp("drill cycle".to_owned()),
        },
        power: PowerVerdict::Unmodeled {
            reason: UnmodeledReason::NotApplicableForOp("drill cycle".to_owned()),
        },
        deflection: DeflectionVerdict::Unmodeled {
            reason: UnmodeledReason::NotApplicableForOp("drill cycle".to_owned()),
        },
        drill_gates: Some(drill),
        modulation_summary: None,
        feed_explanation: None,
    }
}

fn message_for(d: &DrillOp, id: &str) -> String {
    let v = verdict_with(gates(d));
    diagnostics_from_load_verdict(&v)
        .into_iter()
        .find(|diag| diag.id.as_str() == id)
        .unwrap_or_else(|| panic!("no `{id}` diagnostic emitted"))
        .message
}

fn diagnostic_for(d: &DrillOp, id: &str) -> (Severity, String, Option<DiagnosticEvidence>) {
    let v = verdict_with(gates(d));
    let diag = diagnostics_from_load_verdict(&v)
        .into_iter()
        .find(|diag| diag.id.as_str() == id)
        .unwrap_or_else(|| panic!("no `{id}` diagnostic emitted"));
    (diag.severity, diag.message, diag.evidence)
}

// ── R-1 · "exceeds" printed about a value below its own threshold ────

/// The audit's money finding. Fixture D2: Ø3, 25 mm, `Peck(22)`,
/// softwood (`t = 8.0`). The evacuation-credited D/d is 22/3 = 7.33,
/// which lands in the Elevated band `[0.75t, t)` — i.e. **strictly
/// below** the threshold. Shipped output, verbatim:
///
/// ```text
/// Chip welding (D/d) exceeds: 7.33 vs 8.00   — Severity::Caution
/// ```
///
/// Nothing was exceeded. `DrillGateOutcome::Exceeds::threshold` is
/// documented as "The violated bound"; at Elevated no bound is
/// violated. The Elevated band is 25 % of the threshold wide by
/// construction, so this is not a corner case — F1's own note records
/// the live WANAKA pin drill sitting inside it.
#[test]
fn elevated_chip_welding_must_not_claim_exceedance_below_its_threshold() {
    let d = op(DrillCycle::Peck(22.0), 3.0, 25.0, 300.0);
    let (severity, message, _) = diagnostic_for(&d, ids::DRILL_CHIP_WELDING);

    assert_eq!(
        severity,
        Severity::Caution,
        "fixture must land in the advisory band, not Critical: {message}"
    );
    let observed = match gates(&d).chip_welding {
        DrillGateOutcome::Exceeds {
            observed,
            threshold,
            ..
        } => {
            assert!(
                observed < threshold,
                "fixture must be BELOW the threshold to exercise R-1 \
                 (observed {observed:.2}, threshold {threshold:.2})"
            );
            observed
        }
        other => panic!("fixture must reach the Elevated band, got {other:?}"),
    };
    assert!(
        !message.contains("exceeds"),
        "R-1: an advisory reading below its own threshold must not be \
         worded as an exceedance. observed {observed:.2} < threshold. \
         Shipped text was `Chip welding (D/d) exceeds: 7.33 vs 8.00`; \
         got `{message}`"
    );
}

/// The same string ships on the plunge-feed **low** side, where the
/// observation is below the floor. There the number *is* the violated
/// bound, but the verb points the wrong way:
///
/// ```text
/// Plunge feed envelope exceeds: 16.67 vs 50.00
/// ```
///
/// Fixture D5: Ø6, 12 mm, `Peck(2)`, feed 100 → 16.67 mm/min per mm Ø
/// against the wood envelope (50, 400).
#[test]
fn below_floor_plunge_feed_must_not_be_worded_as_an_exceedance() {
    let d = op(DrillCycle::Peck(2.0), 6.0, 12.0, 100.0);
    let message = message_for(&d, ids::DRILL_PLUNGE_FEED);
    assert!(
        !message.contains("exceeds"),
        "R-1: a reading BELOW the envelope floor is not an exceedance. \
         Shipped text was `Plunge feed envelope exceeds: 16.67 vs 50.00`; \
         got `{message}`"
    );
}

// ── R-3 · a Within verdict displaying a bound it was not compared to ──

/// `Low` is decided at `0.75t`, but the outcome carried `t` for all
/// three bands. A Ø4 × 23.6 mm softwood hole shipped as:
///
/// ```text
/// Chip welding (D/d) within (5.90)
/// evidence GeometryCompare { lhs 5.90, rhs "threshold" 8.00 }
/// ```
///
/// Implied headroom 26 %. Real headroom to the next band
/// (`0.75 × 8 = 6.0`): **1.7 %**. The plunge gate already solved this
/// in the same file by carrying `envelope_lo` / `envelope_hi` beside
/// `threshold`, and documents exactly why.
#[test]
fn within_chip_welding_displays_the_boundary_that_decided_it() {
    let d = op(DrillCycle::Simple, 4.0, 23.6, 300.0);
    let outcome = gates(&d).chip_welding;
    match outcome {
        DrillGateOutcome::Within {
            observed,
            envelope_hi,
            ..
        } => {
            assert!(
                (observed - 5.9).abs() < 0.01,
                "fixture drifted: expected D/d 5.90, got {observed:.3}"
            );
            let advisory = envelope_hi.expect(
                "R-3: the chip-welding gate must carry the advisory boundary \
                 (0.75×t) that actually decided `Low`, the way the plunge gate \
                 carries envelope_lo/envelope_hi. Shipped output displayed \
                 `threshold 8.00` — a 26 % implied headroom on a reading with \
                 1.7 % real headroom.",
            );
            assert!(
                (advisory - 6.0).abs() < 1e-9,
                "R-3: the boundary that decided this verdict is 0.75 × 8.0 = 6.0, \
                 got {advisory}"
            );
        }
        other => panic!("fixture must read Within, got {other:?}"),
    }
}

// ── R-7 · evidence that points at sample 0 ────────────────────────────

/// Drill `Exceeds` diagnostics shipped
/// `SampleRange { sample_start: 0, sample_end: 0 }`. Drill samples are
/// real and are keyed `(hole_id, peck_index)`, so this points the
/// operator at a specific sample that has nothing to do with the
/// finding. The honest evidence for a closed-form ratio is the
/// comparison itself.
#[test]
fn drill_exceedance_evidence_must_not_point_at_sample_zero() {
    let d = op(DrillCycle::Simple, 4.0, 40.0, 300.0);
    let (_, message, evidence) = diagnostic_for(&d, ids::DRILL_CHIP_WELDING);
    match evidence.expect("drill exceedance must carry evidence") {
        DiagnosticEvidence::SampleRange {
            sample_start,
            sample_end,
            ..
        } => panic!(
            "R-7: shipped a fabricated SampleRange {sample_start}..{sample_end} \
             for a closed-form ratio ({message})"
        ),
        DiagnosticEvidence::GeometryCompare { .. } => {}
        other => panic!("unexpected evidence shape: {other:?}"),
    }
}

// ── R-5 · "1.00 (1=cleared)" printed beside "INADEQUATE" ──────────────

/// `chip_evacuation_score` returns a hard 1.0 for every `Peck` cycle
/// regardless of peck depth, so `avg_chip_evacuation_score` is 1.00 for
/// any pecking op. On the D2 fixture narrate emits, two lines apart:
///
/// ```text
/// ℹ drill cycle — … peck pattern INADEQUATE — reduce peck depth.
/// ℹ drill cycle time: … mean chip-evacuation score 1.00 (0=trapped, 1=cleared).
/// ```
///
/// The score's `Peck` arm ignores the depth the verdict is about.
#[test]
fn evacuation_score_must_not_read_fully_cleared_on_an_inadequate_peck() {
    let d = op(DrillCycle::Peck(22.0), 3.0, 25.0, 300.0);
    let samples = emit_drill_samples(ToolpathId(0), &d);
    let summary = build_drill_toolpath_summary(ToolpathId(0), &d, &samples);
    assert!(
        !summary.peck_pattern_adequate,
        "fixture must be the inadequate one"
    );
    assert!(
        summary.avg_chip_evacuation_score < 1.0,
        "R-5: a 7.33×D single peck cannot score 1.00 = fully cleared while \
         the sibling verdict on the same summary says the peck pattern is \
         INADEQUATE. Got {:.2}",
        summary.avg_chip_evacuation_score
    );
    // A genuinely shallow peck still scores high — the arm must fall
    // off with per-peck D/d, not be flattened.
    let shallow = op(DrillCycle::Peck(1.0), 6.0, 18.0, 300.0);
    let shallow_samples = emit_drill_samples(ToolpathId(0), &shallow);
    let shallow_summary = build_drill_toolpath_summary(ToolpathId(0), &shallow, &shallow_samples);
    assert!(
        shallow_summary.avg_chip_evacuation_score > 0.8,
        "a 0.17×D peck evacuates well and must keep a high score; got {:.2}",
        shallow_summary.avg_chip_evacuation_score
    );
    assert!(
        shallow_summary.avg_chip_evacuation_score > summary.avg_chip_evacuation_score,
        "the score must be monotone in per-peck depth"
    );
}

// ── R-2 · the summary models a cycle the emitter does not emit ────────

/// The audit's one MODEL_FIX with hard arithmetic (§3). The summary
/// re-expands the *config*, rooted at `hole.top_z`; the emitter's peck
/// grid is rooted at the **R-plane** (`effective_safe_z` = stock top +
/// 5 mm on every default project), which is the Fanuc G83 convention
/// and is correct.
///
/// On shipped defaults (depth 10, `Peck(3)`, Ø6, feed 300, R = +5) the
/// emitter produces **5** feed-down moves; `peck_count` reported **4**.
///
/// The correct data is already emitted and unread: `RegionSpanRole::
/// DrillPeck` spans are derived geometrically from the toolpath, not
/// from labels. This test selects its population through the span role
/// — never through `Span::label`, whose contract forbids downstream
/// dependence — so it cannot pass by agreeing with a label.
#[test]
fn summary_peck_count_matches_the_emitted_drill_peck_spans() {
    use rs_cam_core::compute::spans::spans_from_drill_holes;
    use rs_cam_core::toolpath_spans::RegionSpanRole;

    let depth = 10.0;
    let peck = 3.0;
    let feed = 300.0;
    let top_z = 0.0;
    let retract_z = top_z + R_PLANE_ABOVE_STOCK_MM;

    let tp = drill_toolpath(
        &[[0.0, 0.0]],
        &DrillParams {
            depth,
            top_z,
            cycle: DrillCycle::Peck(peck),
            feed_rate: feed,
            safe_z: 25.0,
            retract_z,
        },
    );
    let spans = spans_from_drill_holes(&tp);
    let emitted_pecks = spans
        .iter()
        .filter(|s| s.has_region_role(RegionSpanRole::DrillPeck))
        .count();

    let d = op(DrillCycle::Peck(peck), 6.0, depth, feed);
    let samples = emit_drill_samples(ToolpathId(0), &d);
    let summary = build_drill_toolpath_summary(ToolpathId(0), &d, &samples);

    assert_eq!(
        summary.peck_count, emitted_pecks,
        "R-2: the summary models a cycle the emitter does not emit. The \
         emitter's peck grid is rooted at the R-plane (+{R_PLANE_ABOVE_STOCK_MM} \
         mm), the summary's at the stock top, so the first feed-down — the \
         one that is 100 % air — is not represented. Shipped: peck_count 4 \
         vs {emitted_pecks} DrillPeck spans."
    );
}

/// The same divergence in the time domain: `feed_time_s` is the sum of
/// the *modelled* descents over the feed rate, so it omits both the
/// leading air descent and the 0.5 mm re-entry clearance the emitter
/// re-cuts on every peck after a full retract. Measured against the
/// emitted toolpath's own feed distance.
#[test]
fn summary_feed_time_matches_the_emitted_feed_distance() {
    use rs_cam_core::toolpath::MoveType;

    let depth = 10.0;
    let peck = 3.0;
    let feed = 300.0;
    let top_z = 0.0;

    let tp = drill_toolpath(
        &[[0.0, 0.0]],
        &DrillParams {
            depth,
            top_z,
            cycle: DrillCycle::Peck(peck),
            feed_rate: feed,
            safe_z: 25.0,
            retract_z: top_z + R_PLANE_ABOVE_STOCK_MM,
        },
    );
    let mut prev: Option<f64> = None;
    let mut fed_mm = 0.0;
    for m in &tp.moves {
        if let (MoveType::Linear { .. }, Some(prev_z)) = (&m.move_type, prev) {
            fed_mm += (prev_z - m.target.z).abs();
        }
        prev = Some(m.target.z);
    }
    let emitted_feed_time_s = fed_mm / feed * 60.0;

    let d = op(DrillCycle::Peck(peck), 6.0, depth, feed);
    let samples = emit_drill_samples(ToolpathId(0), &d);
    let summary = build_drill_toolpath_summary(ToolpathId(0), &d, &samples);

    assert!(
        (summary.feed_time_s - emitted_feed_time_s).abs() < 1e-6,
        "R-2: reported cycle time {:.3} s vs the emitted toolpath's actual \
         {:.3} s of feed motion ({fed_mm:.1} mm at {feed} mm/min). Shipped \
         understatement on these defaults was 33 %.",
        summary.feed_time_s,
        emitted_feed_time_s
    );
}

/// Guard rail for the R-2 fix: rooting the model at the R-plane must
/// not move any gate number. The quantities the three gates read are
/// the deepest single **cutting** descent and the total hole depth —
/// neither is affected by where the feed move starts in air.
#[test]
fn r_plane_rooting_moves_no_gate_number() {
    for (cycle, dia, depth) in [
        (DrillCycle::Peck(3.0), 6.0, 10.0),
        (DrillCycle::Peck(22.0), 3.0, 25.0),
        (DrillCycle::Simple, 4.0, 40.0),
        (DrillCycle::ChipBreak(2.0, 0.5), 6.0, 24.0),
    ] {
        let d = op(cycle, dia, depth, 300.0);
        let g = gates(&d);
        let samples = emit_drill_samples(ToolpathId(0), &d);
        let summary = build_drill_toolpath_summary(ToolpathId(0), &d, &samples);

        // Total-hole D/d is a pure config ratio and must be exact.
        assert!(
            (summary.max_depth_to_diameter - depth / dia).abs() < 1e-9,
            "{cycle:?}: total-hole D/d drifted"
        );
        // The peck-adequacy observation is the nominal peck (or the
        // whole hole when it is shallower), never the air-inflated
        // first descent.
        let expected_peck_dtd = match cycle {
            DrillCycle::Simple | DrillCycle::Dwell(_) => depth / dia,
            DrillCycle::Peck(p) | DrillCycle::ChipBreak(p, _) => p.min(depth) / dia,
        };
        assert!(
            (g.peck_adequacy.observed() - expected_peck_dtd).abs() < 1e-9,
            "{cycle:?}: peck-adequacy observation moved — {} vs {expected_peck_dtd}",
            g.peck_adequacy.observed()
        );
    }
}
