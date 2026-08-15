//! Wanaka real-shape regression sentry for the combined-Suggest pipeline.
//!
//! Unlike the in-crate `feeds::suggest::tests` block (24 unit tests with
//! hand-tuned synthetic operating points), this test loads a real
//! operator project — `tests/fixtures/wanaka_2026-08-16_f530995a.toml`, a
//! dated snapshot of `planning/airrun_2026-06-01/wanaka.toml` (see
//! `wanaka_project_path` for the 2026-08-16 fixture re-pin and why the
//! live play-file is no longer read) — via `ProjectSession::load`
//! and runs every enabled toolpath through the same
//! `suggest_for_operation` invocation the CLI's `--apply-suggest`
//! flag uses (`crates/rs_cam_cli/src/project.rs::apply_suggested_feeds_to_session`).
//!
//! What this sentry catches:
//!
//! - **Back Rough / 3D Rough 6** (Adaptive3D, 6 mm carbide endmill on
//!   HardMaple): re-baselined 2026-07-06 for the unified load model
//!   (2026-06-20 recalibration, Ks=49.95/F_edge=5.30 anchored to
//!   GenericHardwood). That recalibration made the closed-form
//!   deflection back-off fire only for long/thin tools; this 6 mm
//!   carbide stub is chipload/power-bound, not deflection-bound, so
//!   `DppCappedByDeflection` must NOT fire. Instead the axial-DOC
//!   envelope (`binding: "vendor_ap"`) clamps the calculator's 9 mm
//!   DPP straight to the matched row's ap ceiling via
//!   `AxialDocClampedByEnvelope`, and the deflection solve never runs.
//!   **Re-baselined 2026-08-16 (G-WANAKA-DPP):** that ceiling is 4.2 mm,
//!   not 5.4 — Checkpoint K-(a4) (`b7234d2f`) moved the matched row from
//!   the adaptive to the pocket family. Full record at the assertion.
//!   **Re-baselined 2026-08-13 (Checkpoint
//!   J-1):** `FeedRaisedForChipload` used to fire here, lifting feed
//!   911 → 6000 mm/min (the derived cutting ceiling) with
//!   `cap_hit == Some(MaxFeed)`. Suggest pass 8 is retired; the feed now
//!   stays at the calculator's own value and neither chipload-lift
//!   warning fires.
//! - **3D Finish 6** (DropCutter, 2 mm-tip tapered ball): scallop-height
//!   target resolves to ~0.03 mm stepover (~4.6 M moves on the Wanaka
//!   stock envelope) — `StepoverRaisedForRuntime` must fire raising
//!   stepover toward ~0.23 mm. The `FeedRaisedForChipload` half of this
//!   case was re-baselined 2026-08-13 (Checkpoint J-1) to "must NOT
//!   fire". That inversion was UNVERIFIED by execution until 2026-08-16:
//!   the operator had disabled toolpath 11 in their working copy, so
//!   `wanaka_suggest_baseline` never reached this block. The G-WANAKA-DPP
//!   fixture re-pin restores it — **measured, not asserted-in-hope**:
//!   stepover 0.030 → 0.22781 mm and neither lift warning fires.
//! - **Pin Drill / Holes** (drill family): chipload is `NotApplicable` —
//!   neither `FeedRaisedForChipload` nor `DppCappedByDeflection` may
//!   leak through.
//! - **Rivers / Lakes** (V-bit `project_curve`): `FeedRaisedForChipload`
//!   must NOT fire. Pre-2026-08-13 this held because V-bit chipload was
//!   `NotApplicable` and `project_curve`'s arc-fit ratio was `Default`
//!   rather than `Calibrated`; since the retirement it holds for every op
//!   family, so this case no longer distinguishes anything. Kept because
//!   the negative catch-all below still guards the variant list.
//!
//! And a negative catch-all: any `SuggestWarning` variant that
//! shouldn't appear on this project — e.g. an unexpected
//! `StepoverClampedToToolDiameter` — fails the test loudly so a
//! future regression that quietly leaks a new variant gets flagged.
//!
//! If the snapshot fixture is intentionally re-taken this test will fail
//! and force a deliberate re-baseline — that's the design. Edits to the
//! operator's live play-file no longer reach it; see `wanaka_project_path`.
//!
//! ## Pin convention (R4, tech-debt review 2026-06-10)
//!
//! This file was repinned twice in one day (F3 then F4) — every
//! exact-number pin makes a human ratify a new number on every
//! intentional behavior change, and a wrongly-ratified number is
//! invisible. So assertions here follow three rules:
//!
//! 1. **Literature-backed values**: pin the exact band and cite the
//!    source (the `_litmatrix_*` suites are the model).
//! 2. **Behavioral outcomes** (the default): assert *identity and
//!    direction*, not magnitude — which warning fires, which cap binds
//!    (`cap_hit == Some(MaxFeed)`), monotonic relations
//!    (`raised > requested`), relative bands against model outputs
//!    (`obs_after ∈ (0.8×target, target)`), and named constants
//!    (`DEFAULT_CUTTING_FEED_CAP_MM_MIN`) instead of literals.
//! 3. **Determinism sentries** (rare): a raw numeric pin is allowed
//!    only with a comment naming what legitimately re-baselines it
//!    (see the 4.2 mm envelope-clamped DPP pin below, which since
//!    2026-08-16 also derives its expectation from the matched LUT row
//!    and pins that row's identity — so the literal is a cross-check
//!    rather than the whole guarantee).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use rs_cam_core::ids::ToolpathId;
use std::path::PathBuf;

use rs_cam_core::compute::tool_config::ToolId;
use rs_cam_core::feeds::{
    embedded_vendor_lut,
    suggest::{
        StockContext, SuggestContext, SuggestForOperationInput, SuggestWarning, SuggestedParams,
        suggest_for_operation,
    },
};
use rs_cam_core::machine::DEFAULT_CUTTING_FEED_CAP_MM_MIN;
use rs_cam_core::session::ProjectSession;

/// Resolve the wanaka project this sentry runs against.
///
/// ## FIXTURE RE-PIN 2026-08-16 (G-WANAKA-DPP)
///
/// **Old source:** `planning/airrun_2026-06-01/wanaka.toml` — the
/// operator's live play-file, loaded directly. The rationale that stood
/// here was "this sentry exists precisely to fail when that project
/// drifts".
///
/// **New source:** `tests/fixtures/wanaka_2026-08-16_f530995a.toml` — a
/// byte-identical snapshot of that play-file taken from
/// `git show f530995a:planning/airrun_2026-06-01/wanaka.toml`
/// (blob `fa04baaa`).
///
/// **Why:** the old rationale conflated two different failures under one
/// red. This is a *feeds-pipeline* sentry: every assertion below is about
/// what Suggest computes, and the project is the fixed geometry those
/// computations are pinned against. But the file it read is a document the
/// operator edits between machining sessions, so ordinary play — here,
/// disabling toolpath 11 and adding a toolpath 15 — turned the sentry red
/// with `Toolpath id 11 missing from suggest cases` and kept it red for
/// weeks. A test whose fixture a human edits for unrelated reasons cannot
/// distinguish "Suggest regressed" from "the operator tried something", and
/// during that window it verified neither: the whole tp-11 block was
/// unreachable (see its own NOT EXERCISED note, now retired).
///
/// Drift of the play-file is a *fixture-provenance* event, not a feeds
/// regression, so it no longer lands here. Re-snapshot deliberately (new
/// dated file, new commit sha in the name, re-ratify the numbers below)
/// when the play-file's geometry should become the new reference.
fn wanaka_project_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("wanaka_2026-08-16_f530995a.toml")
}

/// Run Suggest for every enabled toolpath through the canonical
/// `ProjectSession::cutter_op_profile` — the same assembly the GUI
/// Suggest button, the MCP rationale endpoint, and (post-T16) the CLI
/// `--apply-suggest` path use. Pre-T16 this helper mirrored the CLI's
/// hand-rolled context with `SpindleStrategy::default()`; all surfaces
/// now read `post_config().spindle_strategy`. Wanaka carries no
/// explicit strategy (defaults to `MatchChart`), so the baseline
/// numbers asserted below are unchanged by the rerouting.
///
/// Returns `(toolpath_id, toolpath_name, suggested)` tuples in
/// session order so individual assertions can find their case by id.
fn run_suggest_for_enabled(session: &ProjectSession) -> Vec<(ToolpathId, String, SuggestedParams)> {
    let mut out = Vec::new();
    for tc in session.toolpath_configs() {
        if !tc.enabled {
            continue;
        }
        let profile = session
            .cutter_op_profile(tc)
            .unwrap_or_else(|| panic!("Tool {} missing for toolpath {}", tc.tool_id, tc.id));
        if let Err(e) = &profile.feasibility {
            panic!(
                "Suggest refused enabled toolpath {} ({}): {e:?}",
                tc.id, tc.name
            );
        }
        // Feasibility Ok ⟺ both Some (`CutterOpProfile::for_combo`).
        let operation = profile
            .suggested_operation
            .unwrap_or_else(|| panic!("feasible combo without suggested operation, tp {}", tc.id));
        let feeds_result = profile
            .feeds
            .unwrap_or_else(|| panic!("feasible combo without feeds result, tp {}", tc.id));
        out.push((
            tc.id,
            tc.name.clone(),
            SuggestedParams {
                operation,
                feeds_result,
                warnings: profile.warnings,
                provenance: rs_cam_core::feeds::FeedsProvenance::default(),
            },
        ));
    }
    out
}

/// Find one toolpath in the suggest output by id; panic with a helpful
/// message if it is absent (likely disabled or missing from the project).
fn find_case(
    cases: &[(ToolpathId, String, SuggestedParams)],
    id: ToolpathId,
) -> &(ToolpathId, String, SuggestedParams) {
    cases.iter().find(|(tid, _, _)| *tid == id).unwrap_or_else(|| {
        panic!(
            "Toolpath id {id} missing from suggest cases — wanaka.toml shape changed? Got ids: {:?}",
            cases.iter().map(|(i, n, _)| (*i, n.as_str())).collect::<Vec<_>>()
        )
    })
}

/// **RE-BASELINED 2026-08-13 — Checkpoint J-1.** `assert_has_feed_raised`
/// stood here and returned the six `FeedRaisedForChipload` fields; every
/// Adaptive3d / DropCutter case in this file called it. Suggest pass 8 is
/// retired, so the helper inverts and `assert_no_feed_raised` becomes the
/// only one — it applies to **every** op family now, not just the ones
/// whose chipload was `NotApplicable`.
fn assert_no_feed_raised(warnings: &[SuggestWarning], context: &str) {
    assert!(
        !warnings.iter().any(|w| matches!(
            w,
            SuggestWarning::FeedRaisedForChipload { .. }
                | SuggestWarning::ChiploadStillLowAfterRecalibration { .. }
        )),
        "{context}: the retired chipload-lift warnings must NOT fire — no Suggest pass has \
         constructed either since 2026-08-13 (Checkpoint J-1/J-5), got warnings: {warnings:?}"
    );
}

fn assert_no_dpp_capped(warnings: &[SuggestWarning], context: &str) {
    assert!(
        !warnings
            .iter()
            .any(|w| matches!(w, SuggestWarning::DppCappedByDeflection { .. })),
        "{context}: DppCappedByDeflection must NOT fire (drill / V-bit), got warnings: {warnings:?}"
    );
}

#[test]
fn wanaka_suggest_baseline() {
    let path = wanaka_project_path();
    assert!(
        path.exists(),
        "Wanaka snapshot fixture not found at {} — the real-shape sentry needs its dated snapshot on disk",
        path.display()
    );
    let session = ProjectSession::load(&path).expect("Load wanaka.toml as ProjectSession");

    let cases = run_suggest_for_enabled(&session);
    assert!(
        !cases.is_empty(),
        "Expected ≥1 enabled toolpath in wanaka.toml, got 0 — project shape regression"
    );

    // Baseline snapshot (visible with `cargo test -- --nocapture`).
    // Useful to recover the on-disk Wanaka numbers without having to
    // wire a separate `apply_suggest_save` invocation.
    println!("wanaka_suggest baseline:");
    for (id, name, suggested) in &cases {
        println!(
            "  tp {} {:?}: feed={:.0} plunge={:.0} dpp={:?} stepover={:?}",
            id,
            name,
            suggested.operation.feed_rate(),
            suggested.operation.plunge_rate(),
            suggested.operation.depth_per_pass(),
            suggested.operation.stepover(),
        );
        for w in &suggested.warnings {
            println!("       {w:?}");
        }
    }

    // ── Toolpath 4: Back Rough (Adaptive3D, 6 mm carbide endmill, HardMaple)
    {
        let (_id, name, suggested) = find_case(&cases, ToolpathId(4));
        let ctx = format!("Back Rough (tp {_id} / {name})");

        // Unified-load-model re-baseline (2026-07-06): the axial-DOC
        // envelope pass runs FIRST in enforce_invariants. On Wanaka's
        // Back Rough the vendor `ap_max` row binds and clamps the
        // calculator's 9 mm initial DPP straight to the row's ap ceiling
        // via `AxialDocClampedByEnvelope { binding: "vendor_ap" }`. Since
        // the 2026-06-20 unified-load-model recalibration (Ks=49.95/
        // F_edge=5.30 anchored to GenericHardwood), the closed-form
        // deflection back-off only fires for long/thin tools — this 6 mm
        // carbide stub is chipload/power-bound, so the back-off never runs
        // and `DppCappedByDeflection` must NOT fire.
        let envelope_clamp = suggested
            .warnings
            .iter()
            .find_map(|w| match w {
                SuggestWarning::AxialDocClampedByEnvelope {
                    op_kind: "adaptive3d",
                    commanded_mm,
                    clamped_mm,
                    binding,
                    ..
                } => Some((*commanded_mm, *clamped_mm, *binding)),
                _ => None,
            })
            .unwrap_or_else(|| {
                panic!(
                    "{ctx}: AxialDocClampedByEnvelope must fire — calculator's 9 mm DPP \
                     should hit the vendor ap bound, got warnings {:?}",
                    suggested.warnings
                )
            });
        let (envelope_commanded, envelope_clamped, envelope_binding) = envelope_clamp;
        assert!(
            (envelope_commanded - 9.0).abs() < 1e-6,
            "{ctx}: calculator pre-envelope DPP must be 9.0 mm, got {envelope_commanded}"
        );
        assert_eq!(
            envelope_binding, "vendor_ap",
            "{ctx}: the vendor ap_max row must bind (not deflection/scallop), got {envelope_binding}"
        );
        assert!(
            envelope_clamped > 0.0 && envelope_clamped < envelope_commanded,
            "{ctx}: envelope-clamped DPP must drop below the calculator's initial 9.0 mm, got {envelope_clamped}"
        );
        // ── RE-BASELINED 2026-08-16 (G-WANAKA-DPP): 5.4 mm → 4.2 mm ────
        //
        // OLD: `(envelope_clamped - 5.4).abs() < 0.5`. Provenance as this
        // file stated it: "the vendor_ap envelope's output on this exact
        // geometry/tool/LUT row"; re-baseline only on the unified-load-
        // model recalibration or a deliberate LUT ap_max change. 5.4 is
        // `min(ap_max_factor 0.9 × Ø6, ap_max_mm 5.5)` on the LUT row
        // `amana-flat-hardwood-adaptive-6000-2f`.
        //
        // NEW: 4.2 mm, observed as 4.199999999999999 — which is exactly
        // `min(0.7 × 6.0, 4.2)` in IEEE-754 double (0.7 × 6.0 rounds to
        // 4.199999999999999 and wins the min against 4.2). That is the ap
        // ceiling of the LUT row `amana-flat-hardwood-pocket-6000-2f`
        // (`ap_max_factor 0.7`, `ap_max_mm 4.2`, `ap_rule "0.25xD to
        // 0.7xD"`, Amana Spektra Spiral Plunge v24). Both the old and the
        // new value are bit-exact reproductions of their row's ceiling, so
        // the attribution is arithmetic, not inference.
        //
        // MECHANISM: `b7234d2f` — "feat(a7)!: K-(a4) — route the LUT query
        // ONCE; both consumers call `lut_query_for`" (A-7, Checkpoint
        // K-(a4), ruled binding and declared NUMBER-MOVING for Adaptive3d).
        // Before a4, Suggest queried the *declared* family, so an
        // `Adaptive3d` op matched the `adaptive` row; the gate/optimizer
        // side already rerouted Adaptive3d → Pocket. a4 hoisted that
        // routing into `feeds::vendor_normalize::lut_query_for` and made
        // both sides call it, so Suggest's matched row moved adaptive →
        // pocket. The matched row reaches this envelope through
        // `FeedsResult::matched_lut_row` → `SuggestContext::matched_lut_row`
        // → `pick_axial_envelope` → `cutter_axial_constraints` →
        // `max_doc_vendor` = `min(ap_max_factor × D, ap_max_mm)`.
        //
        // The same commit's companion `16786d3b` re-pinned the two
        // Adaptive3d feeds in `arc_fit_disposition_a5.rs` on this exact
        // row pair (band 0.038–0.070 → 0.032–0.055) and named the row ids.
        // This file was NOT re-pinned then because it was already red on
        // the play-file drift and its verification never reached here.
        //
        // R4 convention: this stays a DETERMINISM SENTRY (rule 3), but the
        // number is no longer a bare literal — the assertion below derives
        // the expectation from the matched row and the row-identity check
        // above states which row that must be. Re-baseline on a deliberate
        // LUT ap change or a deliberate re-route; if it moves for any other
        // reason, one of those two guards will say which.
        let matched = suggested
            .feeds_result
            .matched_lut_row
            .as_ref()
            .unwrap_or_else(|| panic!("{ctx}: Suggest matched no vendor LUT row"));
        assert_eq!(
            matched.observation_id, "amana-flat-hardwood-pocket-6000-2f",
            "{ctx}: post-K-(a4) an Adaptive3d Suggest query must resolve the POCKET row. \
             If this reads `amana-flat-hardwood-adaptive-6000-2f` again, the shared routing \
             through `vendor_normalize::lut_query_for` has regressed (b7234d2f)"
        );
        let row_ap_ceiling = match (matched.ap_max_factor, matched.ap_max_mm) {
            (Some(f), Some(a)) => (f * 6.0).min(a),
            (Some(f), None) => f * 6.0,
            (None, Some(a)) => a,
            (None, None) => panic!("{ctx}: matched row carries no ap ceiling at all"),
        };
        assert!(
            (envelope_clamped - row_ap_ceiling).abs() < 1e-9,
            "{ctx}: the clamp must be the matched row's own ap ceiling \
             (min(ap_max_factor × Ø6, ap_max_mm) = {row_ap_ceiling}), got {envelope_clamped}"
        );
        assert!(
            (envelope_clamped - 4.2).abs() < 0.05,
            "{ctx}: envelope-clamped DPP must land near 4.2 mm (regression baseline, \
             re-baselined from 5.4 on 2026-08-16 — mechanism b7234d2f / Checkpoint K-(a4)), \
             got {envelope_clamped}"
        );
        // This assertion catches the deflection back-off leaking back
        // into stub-tool roughing — a regression that would silently
        // reintroduce the old ~3.69 mm DppCappedByDeflection chain the
        // unified load model retired.
        assert!(
            !suggested
                .warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::DppCappedByDeflection { .. })),
            "{ctx}: DppCappedByDeflection must NOT fire — the vendor_ap envelope binds first \
             and this chipload/power-bound stub tool never reaches the deflection back-off, \
             got {:?}",
            suggested.warnings
        );

        // ── RE-BASELINED 2026-08-13 (Checkpoint J-1) ───────────────────
        //
        // This block required the arc-fit feed-up to fire and pinned its
        // outcome. Measured at `5c4e847c` on this real project, printed by
        // this test's own baseline dump:
        //
        //   FeedRaisedForChipload { requested_mm_per_min: 911.0,
        //     raised_mm_per_min: 6000.0, cap_hit: Some(MaxFeed) }
        //   ChiploadStillLowAfterRecalibration { blocking_cap: MaxFeed }
        //
        // i.e. the calculator's 911 mm/min was lifted 6.59× to the derived
        // 6000 mm/min cutting ceiling — the uncapped solve wanted 6912 —
        // and the pass then reported that even 6000 fell short of a target
        // stated in a quantity the gate stopped reporting on 2026-08-06.
        //
        // Post-retirement Suggest ships 911 mm/min and says nothing. The
        // assertions invert to that.
        assert_no_feed_raised(&suggested.warnings, &ctx);
        // v3.0d (RPM idempotency fix): apply_feeds_result_to_op now
        // writes the calculator's chosen RPM (16000 on this case) into
        // the operation before enforce_invariants runs, so the
        // recalibration's closed-form solve consumes 16000 (not the
        // op's pre-Suggest 12194). The target_feed scales linearly
        // with RPM: pre-fix landed 5268 mm/min on the median target
        // (0.054 mm/tooth), post-fix lands 6912 mm/min on the same
        // target. The wanaka project's machine.max_feed is 10_000
        // mm/min, so no cap binds.
        // F4 (2026-06-10): the wanaka machine's 10_000 mm/min is its
        // TRAVEL rate; the cutting ceiling derives to 6_000
        // (DEFAULT_CUTTING_FEED_CAP). The v3.0d solve wants 6912
        // mm/min on the median target — pre-F4 it got it (cutting at
        // a feed chosen against the gantry travel spec); post-F4 the
        // recalibration caps at the cutting ceiling and honestly
        // reports the shortfall. A profile that genuinely cuts faster
        // sets max_cutting_feed_mm_min explicitly.
        // The feed Suggest now ships is the calculator's own. Pinned by
        // direction + the retired lift's ceiling rather than by magnitude
        // (pin convention rule 2): whatever the calculator chooses, it must
        // sit strictly below the ceiling the lift used to slam into —
        // otherwise the lift is still happening somewhere.
        assert!(
            suggested.operation.feed_rate() < DEFAULT_CUTTING_FEED_CAP_MM_MIN,
            "{ctx}: the shipped feed must sit below the derived cutting ceiling \
             ({DEFAULT_CUTTING_FEED_CAP_MM_MIN}) the retired lift used to clamp against, \
             got {}",
            suggested.operation.feed_rate()
        );

        // v3.3c (StrategyAndFeeds default): the strategy-aware
        // orchestrator rewrites entry_style plunge → helix on this
        // case (reason "deflection_predict_at_dpp_with_helix_headroom"),
        // replacing the v1.3 PlungeEntryUnstableAtDpp warning with a
        // positive StrategyRewrote rewrite. This is deterministic on
        // Wanaka's geometry regardless of which axial constraint binds
        // the DPP.
        let rewrote = suggested.warnings.iter().any(|w| {
            matches!(
                w,
                SuggestWarning::StrategyRewrote {
                    param: "entry_style",
                    ..
                }
            )
        });
        assert!(
            rewrote,
            "{ctx}: StrategyRewrote(entry_style) must fire under v3.3c default, got {:?}",
            suggested.warnings
        );
        assert!(
            !suggested
                .warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::PlungeEntryUnstableAtDpp { .. })),
            "{ctx}: PlungeEntryUnstableAtDpp must NOT fire — the rewrite handled it, got {:?}",
            suggested.warnings
        );

        // No surprising variants on this case.
        for w in &suggested.warnings {
            match w {
                SuggestWarning::DppCappedByDeflection { .. }
                | SuggestWarning::FeedRaisedForChipload { .. }
                | SuggestWarning::PlungeClampedToFeed { .. }
                | SuggestWarning::StepoverClampedToToolDiameter { .. }
                | SuggestWarning::RoughingDepthClampedToRigidity { .. }
                | SuggestWarning::DepthClampedToCuttingLength { .. }
                | SuggestWarning::PlungeEntryUnstableAtDpp { .. }
                | SuggestWarning::StrategyRewrote { .. }
                | SuggestWarning::AxialDocClampedByEnvelope { .. }
                | SuggestWarning::ChiploadStillLowAfterRecalibration { .. } => {}
                other => {
                    panic!("{ctx}: unexpected SuggestWarning variant slipped through: {other:?}")
                }
            }
        }
    }

    // ── Toolpath 10: 3D Rough 6 — same op family + tool as Back Rough.
    {
        let (_id, name, suggested) = find_case(&cases, ToolpathId(10));
        let ctx = format!("3D Rough 6 (tp {_id} / {name})");

        // Same unified-load-model re-baseline as Back Rough above: the
        // vendor_ap envelope clamps DPP first (9.0 mm → the matched
        // pocket row's 4.2 mm ap ceiling since Checkpoint K-(a4)) and
        // the deflection back-off never runs for this chipload/power-
        // bound stub tool. See the Back Rough block for the full
        // rationale and the 2026-08-16 re-baseline record. This block
        // deliberately pins direction only (pin convention rule 2); the
        // magnitude sentry lives on Back Rough alone so one re-baseline
        // ratifies one number.
        let (envelope_commanded, envelope_clamped) = suggested
            .warnings
            .iter()
            .find_map(|w| match w {
                SuggestWarning::AxialDocClampedByEnvelope {
                    op_kind: "adaptive3d",
                    commanded_mm,
                    clamped_mm,
                    ..
                } => Some((*commanded_mm, *clamped_mm)),
                _ => None,
            })
            .unwrap_or_else(|| {
                panic!(
                    "{ctx}: AxialDocClampedByEnvelope must fire on Adaptive3d, got {:?}",
                    suggested.warnings
                )
            });
        assert!(
            (envelope_commanded - 9.0).abs() < 1e-6,
            "{ctx}: calculator pre-envelope DPP must be 9.0 mm, got {envelope_commanded}"
        );
        assert!(
            envelope_clamped > 0.0 && envelope_clamped < envelope_commanded,
            "{ctx}: envelope-clamped DPP must drop below the calculator's initial 9.0 mm, got {envelope_clamped}"
        );
        assert!(
            !suggested
                .warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::DppCappedByDeflection { .. })),
            "{ctx}: DppCappedByDeflection must NOT fire — same chipload/power-bound stub tool \
             as Back Rough, got {:?}",
            suggested.warnings
        );

        // RE-BASELINED 2026-08-13 (Checkpoint J-1). Same Wanaka geometry as
        // Back Rough, and at `5c4e847c` the identical lift: 911 → 6000
        // mm/min, `cap_hit: Some(MaxFeed)`, still-low alongside. Retired.
        assert_no_feed_raised(&suggested.warnings, &ctx);
        assert!(
            suggested.operation.feed_rate() < DEFAULT_CUTTING_FEED_CAP_MM_MIN,
            "{ctx}: the shipped feed must sit below the derived cutting ceiling \
             ({DEFAULT_CUTTING_FEED_CAP_MM_MIN}), got {}",
            suggested.operation.feed_rate()
        );
    }

    // ── Toolpath 11: 3D Finish 6 (DropCutter, 2 mm-tip tapered ball)
    //
    // The scallop-height target drives stepover to ~0.03 mm; the runtime
    // back-off raises it to ~0.23 mm.
    //
    // F3.1 (2026-06-10): pre-F3 the tapered-ball lookup matched the
    // Whiteside Fusion360 preset row (fabricated 0.1016 mm/tooth
    // "range"), so feed recalibration chased an unreachable target and
    // slammed into machine.max_feed → ChiploadStillLowAfterRecalibration.
    // With the preset rows demoted, the calibrated Amana row wins
    // (scaled target ~0.0053 mm/tooth) and the recalibration REACHES the
    // target inside the machine envelope: feed 950 → ~1350 mm/min, no
    // cap hit, no still-low warning.
    {
        let (_id, name, suggested) = find_case(&cases, ToolpathId(11));
        let ctx = format!("3D Finish 6 (tp {_id} / {name})");

        let stepover_hit = suggested.warnings.iter().find_map(|w| match w {
            SuggestWarning::StepoverRaisedForRuntime {
                requested_mm,
                raised_mm,
                ..
            } => Some((*requested_mm, *raised_mm)),
            _ => None,
        });
        let (requested_step, raised_step) = stepover_hit.unwrap_or_else(|| {
            panic!(
                "{ctx}: StepoverRaisedForRuntime must fire, got warnings: {:?}",
                suggested.warnings
            )
        });
        assert!(
            requested_step < raised_step,
            "{ctx}: raised stepover must exceed requested, got {requested_step} → {raised_step}"
        );
        assert!(
            requested_step < 0.10,
            "{ctx}: requested stepover must be the scallop-height target (~0.03 mm), got {requested_step}"
        );
        assert!(
            raised_step > 0.10,
            "{ctx}: raised stepover must clear 0.10 mm for runtime, got {raised_step}"
        );

        // RE-BASELINED 2026-08-13 (Checkpoint J-1). Pre-fix this pinned
        // the DropCutter lift reaching its target uncapped (`cap_hit ==
        // None`, `obs_after >= 0.95 × lut_target`, no still-low). Retired
        // with pass 8. The A-5i log entry carried this block as a NOT
        // EXERCISED row: at `5c4e847c` the test never got here, because
        // the operator had disabled toolpath 11 in their working copy of
        // `wanaka.toml` and `find_case(ToolpathId(11))` panicked first.
        // **That row is discharged 2026-08-16 (G-WANAKA-DPP):** the
        // fixture re-pin reads the committed snapshot, toolpath 11 is
        // enabled there, and the inversion is now verified by execution —
        // measured warnings on this case are `FinishEnvelopeAdvisory` +
        // `StepoverRaisedForRuntime { 0.03 → 0.22781 }` and nothing else.
        assert_no_feed_raised(&suggested.warnings, &ctx);
    }

    // ── Toolpath 14: Pin Drill (drill family — chipload NotApplicable)
    {
        let (_id, name, suggested) = find_case(&cases, ToolpathId(14));
        let ctx = format!("Pin Drill (tp {_id} / {name})");
        assert_no_feed_raised(&suggested.warnings, &ctx);
        assert_no_dpp_capped(&suggested.warnings, &ctx);
        assert!(
            !suggested
                .warnings
                .iter()
                .any(|w| matches!(w, SuggestWarning::ChiploadStillLowAfterRecalibration { .. })),
            "{ctx}: ChiploadStillLowAfterRecalibration must NOT fire on drill, got {:?}",
            suggested.warnings
        );
    }

    // ── Toolpath 7: Holes (drill — same as Pin Drill)
    {
        let (_id, name, suggested) = find_case(&cases, ToolpathId(7));
        let ctx = format!("Holes (tp {_id} / {name})");
        assert_no_feed_raised(&suggested.warnings, &ctx);
        assert_no_dpp_capped(&suggested.warnings, &ctx);
    }

    // ── Toolpath 5: Rivers (back) — Project Curve, 60° V-bit (chipload NotApplicable)
    {
        let (_id, name, suggested) = find_case(&cases, ToolpathId(5));
        let ctx = format!("Rivers (tp {_id} / {name})");
        assert_no_feed_raised(&suggested.warnings, &ctx);
    }

    // ── Toolpath 6: Lakes (back, inside) — Project Curve, 60° V-bit
    {
        let (_id, name, suggested) = find_case(&cases, ToolpathId(6));
        let ctx = format!("Lakes (tp {_id} / {name})");
        assert_no_feed_raised(&suggested.warnings, &ctx);
    }

    // ── Negative catch-all: no toolpath should emit a SuggestWarning
    // variant outside the v2 surface. If a future variant gets added
    // and starts firing on Wanaka without an intentional re-baseline,
    // this test forces the conversation.
    for (id, name, suggested) in &cases {
        for w in &suggested.warnings {
            match w {
                SuggestWarning::PlungeClampedToFeed { .. }
                | SuggestWarning::StepoverClampedToToolDiameter { .. }
                | SuggestWarning::RoughingDepthClampedToRigidity { .. }
                | SuggestWarning::DepthClampedToCuttingLength { .. }
                | SuggestWarning::PlungeEntryUnstableAtDpp { .. }
                | SuggestWarning::DppCappedByDeflection { .. }
                | SuggestWarning::StepoverRaisedForRuntime { .. }
                | SuggestWarning::FeedRaisedForChipload { .. }
                | SuggestWarning::ChiploadStillLowAfterRecalibration { .. }
                | SuggestWarning::StrategyRewrote { .. } => {}
                // v3.3c: must NOT fire on Wanaka — both 3D-rough
                // toolpaths pin `clearing_strategy = "agent_search"`,
                // and heuristic-B pinning suppresses the warn-only
                // recommendation. If this trips, pinning regressed.
                SuggestWarning::StrategyRecommendedNotApplied { param, .. } => panic!(
                    "tp {id} ({name}): StrategyRecommendedNotApplied({param}) fired on Wanaka — \
                     heuristic-B pinning must suppress it (clearing_strategy is pinned to agent_search)"
                ),
                // Phase 3 axial-envelope warnings: warning-only on
                // Wanaka (no auto-mutation has shipped yet, so Adaptive3d
                // DPP clamps land as `AxialDocClampedByEnvelope` /
                // `AxialEnvelopeSafeBandEmpty`; finish ops can emit
                // advisories without mutating any field). They're allowed
                // to fire — the regression net for these lives in the
                // `feeds::cutter_constraints::tests` unit suite.
                SuggestWarning::AxialEnvelopeSafeBandEmpty { .. }
                | SuggestWarning::AxialDocClampedByEnvelope { .. }
                | SuggestWarning::AxialDocBelowBurnFloor { .. }
                | SuggestWarning::ProjectCurveDepthInfeasible { .. }
                | SuggestWarning::FinishEnvelopeAdvisory { .. } => {} // No `_` arm — adding a new variant to the enum will
                                                                      // force this match to be updated, which forces the
                                                                      // baseline owner to decide whether the variant should
                                                                      // ever fire on Wanaka.
            }
            // Print a one-line breadcrumb when the catch-all fires
            // anything unexpected via the explicit-arms form above.
            let _ = (id, name, w);
        }
    }
}

/// v3.0d (2026-06-04) idempotency sentry: running Suggest a second
/// time on the operation Suggest just produced must not move feed,
/// DPP, stepover, or RPM. The v3 design doc lists this as a stated
/// requirement; pre-v3.0d the recalibration's chipload solve consumed
/// `operation.spindle_rpm()` directly, but `apply_feeds_result_to_op`
/// only wrote the feed/plunge/stepover/DPP — RPM stayed pinned to
/// whatever the loaded project had pre-Suggest. A first Suggest run
/// produced one feed; loading the result and re-running produced a
/// different feed (target_feed scales linearly with RPM).
///
/// This sentry runs Suggest twice on the wanaka Back Rough toolpath
/// and asserts feed / DPP / stepover / RPM match between iterations
/// (within float rounding tolerance). Drill / V-bit ops are excluded
/// from the second-pass check because their chipload pass is
/// `NotApplicable` — there's no recalibration to be non-idempotent.
#[test]
fn wanaka_suggest_idempotent_on_second_run() {
    use rs_cam_core::feeds::{
        SpindleStrategy, embedded_vendor_lut,
        suggest::{SuggestContext, SuggestForOperationInput, suggest_for_operation},
    };

    let path = wanaka_project_path();
    let session = ProjectSession::load(&path).expect("Load wanaka.toml");
    let lut = embedded_vendor_lut();
    let machine = session.machine().clone();
    let material = session.stock_config().material.clone();
    let workholding = session.stock_config().workholding_rigidity;
    let stock_ctx = rs_cam_core::feeds::suggest::StockContext::from_stock_bbox(
        session.stock_bbox(),
        session.stock_config().padding,
    );
    let model_bboxes = session.collect_model_bboxes();

    // Pick Back Rough — the case the v3 work was calibrated against.
    let tc = session
        .toolpath_configs()
        .iter()
        .find(|t| t.id == ToolpathId(4))
        .expect("Wanaka Back Rough (tp 4) missing");
    let tool = session
        .get_tool(ToolId(tc.tool_id))
        .expect("Back Rough tool missing");
    let model_bbox = model_bboxes
        .iter()
        .find(|(id, _)| *id == tc.model_id)
        .map(|(_, b)| b);
    let context = SuggestContext {
        model_bbox,
        stock: Some(&stock_ctx),
        ..SuggestContext::default()
    };

    let first = suggest_for_operation(SuggestForOperationInput {
        operation: &tc.operation,
        tool,
        machine: &machine,
        material: &material,
        workholding,
        lut,
        spindle_strategy: SpindleStrategy::default(),
        context,
    })
    .expect("first Suggest");

    let second = suggest_for_operation(SuggestForOperationInput {
        operation: &first.operation,
        tool,
        machine: &machine,
        material: &material,
        workholding,
        lut,
        spindle_strategy: SpindleStrategy::default(),
        context,
    })
    .expect("second Suggest");

    let f1 = first.operation.feed_rate();
    let f2 = second.operation.feed_rate();
    let dpp1 = first.operation.depth_per_pass().unwrap_or(f64::NAN);
    let dpp2 = second.operation.depth_per_pass().unwrap_or(f64::NAN);
    let so1 = first.operation.stepover().unwrap_or(f64::NAN);
    let so2 = second.operation.stepover().unwrap_or(f64::NAN);
    let rpm1 = first.operation.spindle_rpm().unwrap_or(0);
    let rpm2 = second.operation.spindle_rpm().unwrap_or(0);

    assert!(
        (f1 - f2).abs() < 1.0,
        "feed must be idempotent across re-Suggest: first {f1} vs second {f2}"
    );
    assert!(
        (dpp1 - dpp2).abs() < 1e-6,
        "DPP must be idempotent: first {dpp1} vs second {dpp2}"
    );
    assert!(
        (so1 - so2).abs() < 1e-6,
        "stepover must be idempotent: first {so1} vs second {so2}"
    );
    assert_eq!(
        rpm1, rpm2,
        "spindle_rpm must be idempotent: first {rpm1} vs second {rpm2}"
    );
}

/// Phase 4 (T10) dedup gate: `ProjectSession::cutter_op_profile` must
/// produce byte-identical Suggest output to the hand-rolled
/// `SuggestContext` assembly the GUI feeds modal and MCP
/// `get_suggest_rationale` previously carried (model bbox by
/// `tc.model_id`, stock context, default policy, post-config spindle
/// strategy, embedded LUT). If this drifts, the rationale surfaces
/// silently diverge from the Suggest button.
#[test]
fn session_cutter_op_profile_matches_gui_rationale_assembly() {
    use rs_cam_core::feeds::suggest::SuggestPolicy;

    let path = wanaka_project_path();
    let session = ProjectSession::load(&path).expect("Load wanaka.toml");
    let lut = embedded_vendor_lut();
    let machine = session.machine();
    let stock = session.stock_config();
    let stock_ctx = StockContext::from_stock_bbox(session.stock_bbox(), stock.padding);
    let model_bboxes = session.collect_model_bboxes();

    let mut checked = 0usize;
    for tc in session.toolpath_configs() {
        if !tc.enabled {
            continue;
        }
        let tool = session
            .get_tool(ToolId(tc.tool_id))
            .unwrap_or_else(|| panic!("Tool {} missing for toolpath {}", tc.tool_id, tc.id));

        // The exact context the GUI modal / MCP rationale path built
        // by hand before the dedup (feeds_modal.rs / app/mcp.rs).
        let model_bbox = model_bboxes
            .iter()
            .find(|(id, _)| *id == tc.model_id)
            .map(|(_, b)| b);
        let context = SuggestContext {
            model_bbox,
            stock: Some(&stock_ctx),
            upstream_leftover_stock_mm: None,
            neighboring_strategy_hint: None,
            chipload_bounds: None,
            matched_lut_row: None,
            effective_diameter_mm: 0.0,
            policy: SuggestPolicy::default(),
        };
        let direct = suggest_for_operation(SuggestForOperationInput {
            operation: &tc.operation,
            tool,
            machine,
            material: &stock.material,
            workholding: stock.workholding_rigidity,
            lut,
            spindle_strategy: session.post_config().spindle_strategy,
            context,
        })
        .unwrap_or_else(|e| panic!("Suggest refused toolpath {} ({}): {e:?}", tc.id, tc.name));

        let profile = session
            .cutter_op_profile(tc)
            .unwrap_or_else(|| panic!("profile tool lookup failed for toolpath {}", tc.id));

        assert!(
            profile.feasibility.is_ok(),
            "toolpath {} ({}): profile feasibility diverged from direct Ok",
            tc.id,
            tc.name
        );
        assert_eq!(
            format!("{:?}", profile.warnings),
            format!("{:?}", direct.warnings),
            "toolpath {} ({}): profile warnings diverged from the hand-rolled assembly",
            tc.id,
            tc.name
        );
        assert_eq!(
            format!("{:?}", profile.suggested_operation),
            format!("{:?}", Some(&direct.operation)),
            "toolpath {} ({}): profile suggested operation diverged",
            tc.id,
            tc.name
        );
        checked += 1;
    }
    assert!(
        checked >= 5,
        "expected to exercise the wanaka toolpath set, only checked {checked}"
    );
}
