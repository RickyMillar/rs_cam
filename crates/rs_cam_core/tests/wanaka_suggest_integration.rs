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
//!   not 5.4 — Checkpoint K-(a4) (`6604303c`) moved the matched row from
//!   the adaptive to the pocket family. Full record at the assertion.
//!   **Re-baselined 2026-08-13 (Checkpoint
//!   J-1):** `FeedRaisedForChipload` used to fire here, lifting feed
//!   911 → 6000 mm/min (the derived cutting ceiling) with
//!   `cap_hit == Some(MaxFeed)`. Suggest pass 8 is retired; the feed now
//!   stays at the calculator's own value and neither chipload-lift
//!   warning fires.
//! - **3D Finish 6**: REFUSED on 2026-09-24 by the micro-tool size rule
//!   (ruling R1 applied to size), and VENDOR-BACKED again the same day by
//!   ruling A1 (the row is read at the 1.0 mm tip) and the printed Amana
//!   ZrN v8 1.0 mm tip row (extrapolation P1). The text below is its
//!   history before the refusal.
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
    FeedsError, embedded_vendor_lut,
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
/// `git show 008ab8ce:planning/airrun_2026-06-01/wanaka.toml`
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
///
/// Feeds matrix ruling R1 (2026-09-23): a cell the R1 judgement calls
/// CLUELESS refuses with `FeedsError::Unbacked`. On this project both
/// drill toolpaths (7 Holes, 14 Pin Drill: a 6 mm flat end mill in
/// hardwood) refuse. The helper records them in `refused`; every other
/// refusal still panics.
fn run_suggest_for_enabled(session: &ProjectSession) -> SuggestRun {
    let mut out = Vec::new();
    let mut refused = Vec::new();
    for tc in session.toolpath_configs() {
        if !tc.enabled {
            continue;
        }
        let profile = session
            .cutter_op_profile(tc)
            .unwrap_or_else(|| panic!("Tool {} missing for toolpath {}", tc.tool_id, tc.id));
        match &profile.feasibility {
            Err(e @ FeedsError::Unbacked { .. }) => {
                refused.push((tc.id, tc.name.clone(), e.to_string()));
                continue;
            }
            Err(e) => panic!(
                "Suggest refused enabled toolpath {} ({}): {e:?}",
                tc.id, tc.name
            ),
            Ok(()) => {}
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
    SuggestRun {
        cases: out,
        refused,
    }
}

/// The recipes Suggest shipped, and the toolpaths it refused for lack of
/// a published basis (`FeedsError::Unbacked`, as text).
struct SuggestRun {
    cases: Vec<(ToolpathId, String, SuggestedParams)>,
    refused: Vec<(ToolpathId, String, String)>,
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

#[test]
fn wanaka_suggest_baseline() {
    let path = wanaka_project_path();
    assert!(
        path.exists(),
        "Wanaka snapshot fixture not found at {} — the real-shape sentry needs its dated snapshot on disk",
        path.display()
    );
    let session = ProjectSession::load(&path).expect("Load wanaka.toml as ProjectSession");

    let SuggestRun { cases, refused } = run_suggest_for_enabled(&session);
    assert!(
        !cases.is_empty(),
        "Expected ≥1 enabled toolpath in wanaka.toml, got 0 — project shape regression"
    );
    // Ruling R1 (2026-09-23): the two drill toolpaths refuse, with the
    // judgement's drill reason. Any other refusal is a regression.
    //
    // History: ruling R1 applied to size (2026-09-24) refused tp 11 "3D
    // Finish 6" too, a 1.0 mm-tip tapered ball DropCutter in hardwood. Its
    // nearest chart row was 3.175 mm or larger, more than 2x its engaged
    // diameter. The same day, ruling A1 moved the lookup key to the tip and
    // extrapolation P1 loaded the printed 1.0 mm tip rows, so tp 11 ships
    // again (its arm is below).
    let mut refused_ids: Vec<ToolpathId> = refused.iter().map(|(id, _, _)| *id).collect();
    refused_ids.sort_by_key(|id| id.0);
    assert_eq!(
        refused_ids,
        vec![ToolpathId(7), ToolpathId(14)],
        "the Unbacked refusals must be the two drill toolpaths (Holes, Pin Drill): {refused:?}"
    );
    for (id, name, text) in &refused {
        assert!(
            text.contains("plunge drill") && text.contains("2.5") && !text.contains('{'),
            "tp {id} ({name}): the refusal must carry the drill reason as a sentence: {text}"
        );
    }

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
        // MECHANISM: `6604303c` — "feat(a7)!: K-(a4) — route the LUT query
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
        // The same commit's companion `c9c82398` re-pinned the two
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
            matched.observation_id, "amana-flat-hardwood-pocket-6000-2f-spektra",
            "{ctx}: post-K-(a4) an Adaptive3d Suggest query must resolve the POCKET row. \
             If this reads `amana-flat-hardwood-adaptive-6000-2f` again, the shared routing \
             through `vendor_normalize::lut_query_for` has regressed (6604303c)"
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
        // Re-baselined 2026-09-23 (feeds matrix R5): the printed Spektra
        // pocket row carries the chart's own depth condition, ap_max_factor
        // 1.0 (1 x D = 6.0 mm). The 4.2 mm baseline of 2026-08-16 was the
        // 0.7 x D ceiling of the unprinted row, now derived/c.
        assert!(
            (envelope_clamped - 6.0).abs() < 0.05,
            "{ctx}: envelope-clamped DPP must land near 6.0 mm (the printed row's 1 x D \
             ceiling; re-baselined from 4.2 on 2026-09-23 for R5, from 5.4 on 2026-08-16 \
             for 6604303c / Checkpoint K-(a4)), got {envelope_clamped}"
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
        // outcome. Measured at `719b92d7` on this real project, printed by
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

        // G-SUGGEST-NOCLAMP (2026-08-19), pinned POSITIVELY rather than
        // merely tolerated: this case is the reason the pass exists. The
        // axial envelope clamps DPP 9.0 → 4.2 mm, which crosses the 1×D
        // depth-tier boundary downward and so REMOVES a derate the
        // calculator had already folded into the feed. The direction is the
        // assertion — a rescale that came out ≤ 1 would mean the tier
        // crossing was read backwards.
        let (factor_c, factor_f, from, to) = suggested
            .warnings
            .iter()
            .find_map(|w| match w {
                SuggestWarning::FeedRescaledToFinalGeometry {
                    factor_at_calculator,
                    factor_at_final,
                    requested_mm_per_min,
                    rescaled_mm_per_min,
                    ..
                } => Some((
                    *factor_at_calculator,
                    *factor_at_final,
                    *requested_mm_per_min,
                    *rescaled_mm_per_min,
                )),
                _ => None,
            })
            .unwrap_or_else(|| {
                panic!(
                    "{ctx}: the DPP clamp fired, so the feed must have been re-derived at the \
                     clamped depth — see tests/suggest_feed_matches_final_geometry.rs. \
                     Warnings: {:?}",
                    suggested.warnings
                )
            });
        assert!(
            factor_f > factor_c,
            "{ctx}: clamping DPP 9.0 → 4.2 on a Ø6 tool crosses the 1×D depth tier downward, \
             which removes a derate — the final geometry factor must exceed the calculator's, \
             got {factor_c} → {factor_f}"
        );
        assert!(
            to > from,
            "{ctx}: the removed derate must make the shipped feed FASTER than the one the \
             calculator froze at 9.0 mm, got {from} → {to}"
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
                | SuggestWarning::FeedRescaledToFinalGeometry { .. }
                | SuggestWarning::ChiploadStillLowAfterRecalibration { .. }
                // Ruling R4 WP3 (2026-09-24): the aggressiveness dial. Pinned
                // below as the Wanaka baseline for the dial.
                | SuggestWarning::EngagementReducedForAggressiveness { .. }
                // Ruling R4 Q10 (2026-09-24): the RPM follows the feed ceiling.
                | SuggestWarning::RpmLoweredForFeedCeiling { .. }
                | SuggestWarning::AggressivenessNotApplied { .. } => {}
                other => {
                    panic!("{ctx}: unexpected SuggestWarning variant slipped through: {other:?}")
                }
            }
        }

        // The dial's Wanaka baseline (ruling R4 WP3, measured 2026-09-24).
        // Default dial 0.85 x long-tool share 0.75 (45 mm stickout on Ø6,
        // 7.5 x D) = load target 0.6375. One common scale 0.68167 on the
        // base the clamps left: depth 6.0 -> 4.09 mm, stepover 1.2 ->
        // 0.818 mm. Force 58.44 -> 37.26 N (0.638 x) and power 0.1541 ->
        // 0.0813 kW (0.528 x), both at or under 0.6375 x, so the target is
        // met. The chipload does not change.
        let dial = suggested
            .warnings
            .iter()
            .find_map(|w| match w {
                SuggestWarning::EngagementReducedForAggressiveness {
                    aggressiveness,
                    ld_factor,
                    target_share,
                    scale,
                    dpp_from,
                    dpp_to,
                    stepover_from,
                    stepover_to,
                    force_n_before,
                    force_n_after,
                    power_kw_before,
                    power_kw_after,
                    target_met,
                    ..
                } => Some((
                    *aggressiveness,
                    *ld_factor,
                    *target_share,
                    *scale,
                    (*dpp_from, *dpp_to),
                    (*stepover_from, *stepover_to),
                    (*force_n_before, *force_n_after),
                    (*power_kw_before, *power_kw_after),
                    *target_met,
                )),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{ctx}: the dial record must fire on Back Rough"));
        let (k, ld, share, scale, dpp, so, force, power, met) = dial;
        let near = |a: Option<f64>, b: f64, tol: f64| a.is_some_and(|a| (a - b).abs() <= tol);
        assert!(
            (k - 0.85).abs() < 1e-12 && (ld - 0.75).abs() < 1e-12,
            "{ctx}: {dial:?}"
        );
        assert!((share - 0.6375).abs() < 1e-12, "{ctx}: {dial:?}");
        assert!((scale - 0.68167).abs() < 1e-4, "{ctx}: {dial:?}");
        assert!(
            near(dpp.0, 6.0, 1e-9) && near(dpp.1, 4.09, 1e-6),
            "{ctx}: {dial:?}"
        );
        assert!(
            near(so.0, 1.2, 1e-9) && near(so.1, 0.818, 1e-6),
            "{ctx}: {dial:?}"
        );
        assert!(
            near(force.0, 58.44, 0.05) && near(force.1, 37.26, 0.05),
            "{ctx}: {dial:?}"
        );
        assert!(
            near(power.0, 0.1541, 5e-4) && near(power.1, 0.0813, 5e-4),
            "{ctx}: {dial:?}"
        );
        assert!(met, "{ctx}: the Back Rough target must be met: {dial:?}");
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
        // Back Rough, and at `719b92d7` the identical lift: 911 → 6000
        // mm/min, `cap_hit: Some(MaxFeed)`, still-low alongside. Retired.
        assert_no_feed_raised(&suggested.warnings, &ctx);
        assert!(
            suggested.operation.feed_rate() < DEFAULT_CUTTING_FEED_CAP_MM_MIN,
            "{ctx}: the shipped feed must sit below the derived cutting ceiling \
             ({DEFAULT_CUTTING_FEED_CAP_MM_MIN}), got {}",
            suggested.operation.feed_rate()
        );
    }

    // ── Toolpath 11: 3D Finish 6 (DropCutter, 1.0 mm-tip tapered ball)
    //
    // Extrapolation P1 acceptance (rulings A1 and B1, 2026-09-24). The
    // DropCutter routes to Parallel / Finish. Ruling A1 keys the lookup at
    // the 1.0 mm tip, not at the engaged cone diameter. Two printed rows
    // score 1825 at that key: `amana-tapered-hardwood-parallel-1000-2f-zrn-v8`
    // (Amana ZrN v8 "Wood" column, derived b for hardwood, ruling A4) and
    // `spetool-tapered-hardwood-parallel-1000-2f`. The tie goes to the
    // smaller id (`vendor_lookup::beats`), so the Amana row answers.
    //
    // Score: 1000 base + 220 family + 70 derived + 30 grade b + 80 flutes
    // + 200 diameter (ratio 1.0) + 80 hardness (Janka 1450 on 1450) + 45
    // pass role + 100 material = 1825.
    //
    // The band is the row's printed 0.00075-0.002 in/tooth, 0.01905-0.0508
    // mm/tooth. The diameter ratio is 1.0 and the hardness ratio is 1450 /
    // 1450, so both scales are 1.0 and `matched_lut_row` carries the printed
    // band unchanged. That is the band before the depth de-rate, which
    // `chipload_bounds` applies later.
    //
    // Before the size refusal this block also pinned the runtime stepover
    // back-off (0.03 -> 0.22781 mm). That number was measured on the old
    // row and is not re-pinned here; `StepoverRaisedForRuntime` stays an
    // allowed variant in the catch-all below.
    {
        let (_id, name, suggested) = find_case(&cases, ToolpathId(11));
        let ctx = format!("3D Finish 6 (tp {_id} / {name})");
        assert_eq!(
            suggested.feeds_result.support,
            rs_cam_core::feeds::FeedsSupport::VendorBacked,
            "{ctx}: the printed 1.0 mm tip row answers, so the cell is vendor-backed"
        );
        let matched = suggested
            .feeds_result
            .matched_lut_row
            .as_ref()
            .unwrap_or_else(|| panic!("{ctx}: Suggest matched no vendor LUT row"));
        assert_eq!(
            matched.observation_id, "amana-tapered-hardwood-parallel-1000-2f-zrn-v8",
            "{ctx}: ruling A1 reads the row at the 1.0 mm tip; the Amana v8 row wins the \
             1825 tie with SpeTool on the id"
        );
        assert!(
            (matched.row_diameter_mm - 1.0).abs() < 1e-12,
            "{ctx}: the row is printed at the 1.0 mm tip, got {}",
            matched.row_diameter_mm
        );
        assert!(
            (matched.chipload_diameter_ratio_raw - 1.0).abs() < 1e-12
                && (matched.chipload_diameter_scale - 1.0).abs() < 1e-12
                && (matched.chipload_hardness_scale - 1.0).abs() < 1e-12,
            "{ctx}: the key is the tip and the hardness is the row's own, so no scale \
             applies: raw {} d-scale {} h-scale {}",
            matched.chipload_diameter_ratio_raw,
            matched.chipload_diameter_scale,
            matched.chipload_hardness_scale
        );
        assert!(
            matched
                .chip_load_min_mm
                .is_some_and(|v| (v - 0.01905).abs() < 1e-12)
                && matched
                    .chip_load_max_mm
                    .is_some_and(|v| (v - 0.0508).abs() < 1e-12),
            "{ctx}: the band before any de-rate must be the printed 0.01905-0.0508 \
             mm/tooth, got {:?}-{:?}",
            matched.chip_load_min_mm,
            matched.chip_load_max_mm
        );
        assert_no_feed_raised(&suggested.warnings, &ctx);
    }

    // ── Toolpath 14: Pin Drill and toolpath 7: Holes (drill family)
    //
    // Until 2026-09-23 these two blocks asserted that no chipload-lift or
    // DppCappedByDeflection warning fired on a drill. Ruling R1 refuses
    // every drill cell in the judged woods (no published figure; the 2.5
    // multiplier is unsourced), so both toolpaths are in `refused` (asserted
    // above) and ship no warnings at all.
    for id in [ToolpathId(14), ToolpathId(7)] {
        assert!(
            cases.iter().all(|(tid, _, _)| *tid != id),
            "tp {id}: a refused drill must not also ship a recipe"
        );
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
            // This match has NO `_` arm, deliberately: adding a variant to
            // the enum forces it to be updated, which forces the baseline
            // owner to decide whether that variant should ever fire on
            // Wanaka. It did exactly that for G-SUGGEST-NOCLAMP below.
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
                // T-12 (2026-09-16): the forcing arm fired, and this is the
                // deliberate answer it demanded. ALLOWED on Wanaka.
                //
                // The variant reports a cut-geometry value the calculator
                // produced that the operation has no field to hold. Before
                // T-12 the funnel discarded that value and returned success.
                // It is warning-only and mutates nothing.
                //
                // Census on this fixture, all of them correct:
                //
                //   tp 7  (Holes)             Drill             depth + stepover
                //   tp 14 (Pin Drill)         AlignmentPinDrill depth + stepover
                //   tp 5  (Rivers)            ProjectCurve      depth + stepover
                //   tp 6  (Lakes)             ProjectCurve      depth + stepover
                //   tp 11 (3D Finish 6)       DropCutter        depth only
                //
                // A drill's depth IS the hole and it has no stepover. A
                // ProjectCurve follows the curve. A DropCutter takes its
                // depth from the model surface — and note it fires for the
                // depth but NOT the stepover, which it does hold. The
                // warning is per-field and discriminating, not blanket.
                //
                // Allowed rather than suppressed: the value genuinely was
                // not applied, and the derate work needs to read exactly
                // this signal to decide whether a shallower pass is a lever
                // the operation can offer. Suppressing it here would
                // re-create the silence T-12 exists to remove.
                SuggestWarning::CutGeometryFieldNotHeld { .. } => {}
                // Ruling R4 WP3 (2026-09-24): the aggressiveness dial
                // (default 0.85) files one record per roughing Suggest,
                // and a "no dial action" record per finish or drill. Both
                // are EXPECTED on Wanaka. The mechanism is pinned by
                // tests/the_dial_holds_the_load_and_never_cuts_the_feed_fm7.rs.
                SuggestWarning::EngagementReducedForAggressiveness { .. }
                | SuggestWarning::AggressivenessNotApplied { .. }
                // Ruling R4 Q10 (2026-09-24): allowed where the ceiling binds.
                | SuggestWarning::RpmLoweredForFeedCeiling { .. } => {}
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
                | SuggestWarning::FinishEnvelopeAdvisory { .. } => {}
                // G-SUGGEST-NOCLAMP: the forcing arm fired as designed when
                // Suggest pass 9 landed on 2026-08-19, and this is the
                // deliberate re-baseline it demanded. Both variants are now
                // EXPECTED on Wanaka and allowed here:
                //
                //   * `FeedRescaledToFinalGeometry` on the two Adaptive3d
                //     roughs (tp 4, tp 10), whose DPP the axial envelope
                //     clamps 9.0 → 4.2 mm — a depth-tier crossing that
                //     removes a derate the calculator had already applied.
                //     Pinned positively, with its direction, in the tp 4
                //     block above; allowed generically here.
                //   * `FeedClampedToChiploadFloor` on the finish pass, whose
                //     whole derated band sits under the 0.025 mm/tooth
                //     chip-formation floor. Since ruling R4 Q9 the floor is
                //     the band minimum, and since WP2a the warning does not
                //     lift the feed; it fires only under the band minimum.
                //
                // Neither is tolerated blindly: the mechanism both speak for
                // is asserted in tests/suggest_feed_matches_final_geometry.rs.
                SuggestWarning::FeedRescaledToFinalGeometry { .. }
                | SuggestWarning::FeedClampedToChiploadFloor { .. } => {}
                // T-4 (2026-09-18): the forcing arm fired when the
                // deflection predictor's refusal became a type. Neither
                // variant may fire on Wanaka, and the reason is
                // structural rather than a tolerance:
                //
                //   * `DeflectionBackoffUnmodeled` needs the back-off to
                //     run, and the back-off needs `depth_per_pass()`.
                //     `DrillConfig`, `AlignmentPinDrillConfig` and
                //     `ProjectCurveConfig` carry no such field, so the
                //     drill and curve toolpaths never enter the loop.
                //     Every remaining Wanaka toolpath cuts hard maple or
                //     white oak with a real carbide cutter, and both
                //     materials carry a measured `Kc`.
                //   * `DeflectionBackoffFigureIsAFloor` needs a V-bit on
                //     a ROUGHING operation. `VCarve` is `PassRole::
                //     Finish`, so the natural V-bit operation never
                //     enters the loop either.
                //
                // If one fires, the fixture gained a tool or a material
                // the closed-form model does not cover. That is a real
                // finding about the fixture; re-derive it rather than
                // widening this arm.
                SuggestWarning::DeflectionBackoffUnmodeled { dpp_mm, reason } => panic!(
                    "tp {id} ({name}): DeflectionBackoffUnmodeled fired on Wanaka at DPP \
                     {dpp_mm:.2} mm — {} ({reason:?}). Every Wanaka roughing toolpath runs a \
                     modelled tool in a modelled material; a refusal here means the fixture \
                     changed.",
                    reason.clause()
                ),
                SuggestWarning::DeflectionBackoffFigureIsAFloor {
                    dpp_mm,
                    predicted_um,
                    caveat,
                } => panic!(
                    "tp {id} ({name}): DeflectionBackoffFigureIsAFloor fired on Wanaka at DPP \
                     {dpp_mm:.2} mm ({predicted_um:.0} µm) — {}. No Wanaka roughing toolpath \
                     runs a V-bit.",
                    caveat.clause()
                ),
                // T-15 (2026-09-18): the forcing arm fired when Suggest pass
                // 10 landed. The answer is a REFUSAL, and it is argued from
                // the fixture rather than tolerated.
                //
                // Pass 10 runs on tp 4 and tp 10 — the two Adaptive3d roughs
                // whose DPP the axial envelope clamps 9.0 → 4.2 mm, the same
                // crossing pass 9 fires on above. It clamps only when the
                // SHIPPED point draws more power than the calculator's point
                // was checked at, and here it cannot:
                //
                //   P_ship / P_calc = depth_ratio × (1 + (tier_ratio − 1) ×
                //                     shear_share)
                //
                // The crossing is 0.75 → 1.00, so `tier_ratio` is 1.333 and
                // the bracket is at most 1.333. The depth ratio is
                // 4.2 / 9.0 = 0.467. Even at a shear share of 1.0 the product
                // is 0.62, so the load FALLS: the depth drop of 53 % dominates
                // the feed rise of 33 %.
                //
                // If this fires, either the axial envelope now clamps to a
                // depth close to the boundary it crosses, or the Wanaka
                // machine profile lost power. Both are real findings about the
                // fixture. Re-derive it rather than widening this arm.
                SuggestWarning::PowerRecheckedAfterRescale {
                    rescaled_mm_per_min,
                    shipped_mm_per_min,
                    required_kw_at_rescaled,
                    available_kw,
                    fits_at_any_feed,
                } => panic!(
                    "tp {id} ({name}): PowerRecheckedAfterRescale fired on Wanaka — pass 9 \
                     wrote {rescaled_mm_per_min:.1} mm/min, which draws \
                     {required_kw_at_rescaled:.3} kW against a {available_kw:.3} kW ceiling, \
                     so pass 10 shipped {shipped_mm_per_min:.1} mm/min \
                     (fits_at_any_feed {fits_at_any_feed}). The Wanaka tier crossing takes the \
                     depth 9.0 → 4.2 mm against a 1.333x feed rise, so the load must FALL."
                ),
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
            calculator_operating_point: None,
            policy: SuggestPolicy::default(),
        };
        let direct = suggest_for_operation(SuggestForOperationInput {
            operation: &tc.operation,
            tool,
            machine,
            material: &stock.material,
            lut,
            spindle_strategy: session.post_config().spindle_strategy,
            context,
        });

        let profile = session
            .cutter_op_profile(tc)
            .unwrap_or_else(|| panic!("profile tool lookup failed for toolpath {}", tc.id));

        // Ruling R1 (2026-09-23): the two drills refuse. Both assemblies
        // must refuse with the same text; nothing else is comparable.
        let direct = match direct {
            Ok(direct) => direct,
            Err(e @ FeedsError::Unbacked { .. }) => {
                let profile_text = match &profile.feasibility {
                    Err(p) => p.to_string(),
                    Ok(()) => panic!(
                        "toolpath {} ({}): direct refused ({e}) but the profile is feasible",
                        tc.id, tc.name
                    ),
                };
                assert_eq!(
                    profile_text,
                    e.to_string(),
                    "toolpath {} ({}): the profile's refusal diverged from direct",
                    tc.id,
                    tc.name
                );
                checked += 1;
                continue;
            }
            Err(e) => panic!("Suggest refused toolpath {} ({}): {e:?}", tc.id, tc.name),
        };

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
