//! Wanaka real-shape regression sentry for the combined-Suggest pipeline.
//!
//! Unlike the in-crate `feeds::suggest::tests` block (24 unit tests with
//! hand-tuned synthetic operating points), this test loads the actual
//! `planning/airrun_2026-06-01/wanaka.toml` via `ProjectSession::load`
//! and runs every enabled toolpath through the same
//! `suggest_for_operation` invocation the CLI's `--apply-suggest`
//! flag uses (`crates/rs_cam_cli/src/project.rs::apply_suggested_feeds_to_session`).
//!
//! What this sentry catches:
//!
//! - **Back Rough / 3D Rough 6** (Adaptive3D, 6 mm carbide endmill on
//!   HardMaple): `DppCappedByDeflection` must fire (9 mm DPP → ~3.69 mm),
//!   and `FeedRaisedForChipload` must follow (feed ~911 → ~3700 mm/min
//!   so the predicted observed chipload clears the LUT min).
//! - **3D Finish 6** (DropCutter, 2 mm-tip tapered ball): scallop-height
//!   target resolves to ~0.03 mm stepover (~4.6 M moves on the Wanaka
//!   stock envelope) — `StepoverRaisedForRuntime` must fire raising
//!   stepover toward ~0.23 mm, and `FeedRaisedForChipload` must hit
//!   `cap_hit == Some(MaxFeed)` (the machine ceiling clamps before the
//!   LUT min is reached, so `ChiploadStillLowAfterRecalibration` fires
//!   alongside with `blocking_cap == MaxFeed`).
//! - **Pin Drill / Holes** (drill family): chipload is `NotApplicable` —
//!   neither `FeedRaisedForChipload` nor `DppCappedByDeflection` may
//!   leak through.
//! - **Rivers / Lakes** (V-bit `project_curve`): V-bit chipload is
//!   `NotApplicable` and `arc_fit_ratio` is `Default` (not Calibrated)
//!   for `project_curve` — `FeedRaisedForChipload` must NOT fire.
//!
//! And a negative catch-all: any `SuggestWarning` variant that
//! shouldn't appear on this project — e.g. an unexpected
//! `StepoverClampedToToolDiameter` — fails the test loudly so a
//! future regression that quietly leaks a new variant gets flagged.
//!
//! If `wanaka.toml` is intentionally mutated this test will fail and
//! force a deliberate re-baseline — that's the design.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

use std::path::PathBuf;

use rs_cam_core::compute::tool_config::ToolId;
use rs_cam_core::feeds::{
    SpindleStrategy, embedded_vendor_lut,
    suggest::{
        FeedRecalibrationCap, StockContext, SuggestContext, SuggestForOperationInput,
        SuggestPolicy, SuggestWarning, SuggestedParams, suggest_for_operation,
    },
};
use rs_cam_core::session::ProjectSession;

/// Resolve the on-disk wanaka project path. We deliberately load the
/// canonical project at `planning/airrun_2026-06-01/wanaka.toml`
/// rather than copying it into `tests/fixtures/` — this sentry exists
/// precisely to fail when that project drifts.
fn wanaka_project_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("planning")
        .join("airrun_2026-06-01")
        .join("wanaka.toml")
}

/// Run `suggest_for_operation` for every enabled toolpath using the
/// exact context construction the CLI's `--apply-suggest` path uses
/// (see `crates/rs_cam_cli/src/project.rs::apply_suggested_feeds_to_session`).
///
/// Returns `(toolpath_id, toolpath_name, suggested)` tuples in
/// session order so individual assertions can find their case by id.
fn run_suggest_for_enabled(session: &ProjectSession) -> Vec<(usize, String, SuggestedParams)> {
    let lut = embedded_vendor_lut();
    let machine = session.machine().clone();
    let material = session.stock_config().material.clone();
    let workholding = session.stock_config().workholding_rigidity;
    let stock_ctx =
        StockContext::from_stock_bbox(session.stock_bbox(), session.stock_config().padding);
    let model_bboxes = session.collect_model_bboxes();

    let mut out = Vec::new();
    for tc in session.toolpath_configs() {
        if !tc.enabled {
            continue;
        }
        let tool = session
            .get_tool(ToolId(tc.tool_id))
            .unwrap_or_else(|| panic!("Tool {} missing for toolpath {}", tc.tool_id, tc.id));
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
        let suggested = suggest_for_operation(SuggestForOperationInput {
            operation: &tc.operation,
            tool,
            machine: &machine,
            material: &material,
            workholding,
            lut,
            spindle_strategy: SpindleStrategy::default(),
            context,
        })
        .unwrap_or_else(|e| {
            panic!(
                "Suggest refused enabled toolpath {} ({}): {e:?}",
                tc.id, tc.name
            )
        });
        out.push((tc.id, tc.name.clone(), suggested));
    }
    out
}

/// Find one toolpath in the suggest output by id; panic with a helpful
/// message if it is absent (likely disabled or missing from the project).
fn find_case(
    cases: &[(usize, String, SuggestedParams)],
    id: usize,
) -> &(usize, String, SuggestedParams) {
    cases.iter().find(|(tid, _, _)| *tid == id).unwrap_or_else(|| {
        panic!(
            "Toolpath id {id} missing from suggest cases — wanaka.toml shape changed? Got ids: {:?}",
            cases.iter().map(|(i, n, _)| (*i, n.as_str())).collect::<Vec<_>>()
        )
    })
}

fn assert_has_dpp_capped(warnings: &[SuggestWarning], context: &str) -> (f64, f64) {
    let hit = warnings.iter().find_map(|w| match w {
        SuggestWarning::DppCappedByDeflection {
            requested_mm,
            capped_mm,
            ..
        } => Some((*requested_mm, *capped_mm)),
        _ => None,
    });
    hit.unwrap_or_else(|| {
        panic!("{context}: DppCappedByDeflection must fire, got warnings: {warnings:?}")
    })
}

fn assert_has_feed_raised(
    warnings: &[SuggestWarning],
    context: &str,
) -> (f64, f64, f64, f64, f64, Option<FeedRecalibrationCap>) {
    let hit = warnings.iter().find_map(|w| match w {
        SuggestWarning::FeedRaisedForChipload {
            requested_mm_per_min,
            raised_mm_per_min,
            predicted_observed_chipload_before,
            predicted_observed_chipload_after,
            lut_target_mm_per_tooth,
            cap_hit,
        } => Some((
            *requested_mm_per_min,
            *raised_mm_per_min,
            *predicted_observed_chipload_before,
            *predicted_observed_chipload_after,
            *lut_target_mm_per_tooth,
            *cap_hit,
        )),
        _ => None,
    });
    hit.unwrap_or_else(|| {
        panic!("{context}: FeedRaisedForChipload must fire, got warnings: {warnings:?}")
    })
}

fn assert_no_feed_raised(warnings: &[SuggestWarning], context: &str) {
    assert!(
        !warnings
            .iter()
            .any(|w| matches!(w, SuggestWarning::FeedRaisedForChipload { .. })),
        "{context}: FeedRaisedForChipload must NOT fire (chipload NotApplicable here), got warnings: {warnings:?}"
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
        "Wanaka project not found at {} — the real-shape sentry needs the canonical project on disk",
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
        let (_id, name, suggested) = find_case(&cases, 4);
        let ctx = format!("Back Rough (tp {_id} / {name})");

        // Phase 3 (`planning/cutter_axial_constraints_2026-06-06.md`):
        // the axial-DOC envelope pass runs FIRST in enforce_invariants. On
        // Wanaka's Back Rough the matched LUT row (Amana 1×D rule, ap_max_factor=1.0)
        // clamps the calculator's 9 mm initial DPP down to 6 mm before the
        // existing closed-form deflection back-off runs. The back-off then
        // takes that 6 mm to ~3-3.7 mm via the 200 µm gate. Verify both
        // halves of the chain are present:
        //   1. AxialDocClampedByEnvelope reads commanded=9.0 (the calculator's
        //      pre-envelope target) — confirms the calculator emitted 9 mm.
        //   2. The post-back-off DPP still lands in the 3.0–4.0 mm regression
        //      band (close to the pre-Phase-3 3.69 mm baseline).
        let envelope_clamp = suggested
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
                    "{ctx}: AxialDocClampedByEnvelope must fire — calculator's 9 mm DPP \
                     should hit the LUT ap bound, got warnings {:?}",
                    suggested.warnings
                )
            });
        let (envelope_commanded, envelope_clamped) = envelope_clamp;
        assert!(
            (envelope_commanded - 9.0).abs() < 1e-6,
            "{ctx}: calculator pre-envelope DPP must be 9.0 mm, got {envelope_commanded}"
        );
        assert!(
            envelope_clamped > 0.0 && envelope_clamped < 9.0,
            "{ctx}: envelope-clamped DPP must drop below the calculator's initial 9.0 mm, got {envelope_clamped}"
        );

        let (_requested_dpp, capped_dpp) = assert_has_dpp_capped(&suggested.warnings, &ctx);
        assert!(
            capped_dpp > 0.0 && capped_dpp < 6.0,
            "{ctx}: capped DPP must drop below tool diameter (6 mm), got {capped_dpp}"
        );
        assert!(
            (capped_dpp - 3.69).abs() < 0.5,
            "{ctx}: capped DPP must land near 3.69 mm (regression baseline), got {capped_dpp}"
        );

        let (feed_before, feed_after, obs_before, obs_after, lut_target, cap_hit) =
            assert_has_feed_raised(&suggested.warnings, &ctx);
        assert!(
            feed_after > feed_before,
            "{ctx}: feed must rise from {feed_before} to satisfy chipload, got {feed_after}"
        );
        // v3.0d (RPM idempotency fix): apply_feeds_result_to_op now
        // writes the calculator's chosen RPM (16000 on this case) into
        // the operation before enforce_invariants runs, so the
        // recalibration's closed-form solve consumes 16000 (not the
        // op's pre-Suggest 12194). The target_feed scales linearly
        // with RPM: pre-fix landed 5268 mm/min on the median target
        // (0.054 mm/tooth), post-fix lands 6912 mm/min on the same
        // target. The wanaka project's machine.max_feed is 10_000
        // mm/min, so no cap binds.
        assert!(
            feed_after > 6500.0 && feed_after < 7500.0,
            "{ctx}: post-recal feed must land in ~6500-7500 mm/min on Wanaka case (v3.0d RPM-idempotent median target), got {feed_after}"
        );
        assert_eq!(
            cap_hit, None,
            "{ctx}: no cap should bind (machine.max_feed is 10_000 mm/min on this project's machine), got {cap_hit:?}"
        );
        // When uncapped, the closed-form solve lands observed at the
        // policy target (median of the LUT band under v3.0c default).
        assert!(
            (obs_after - lut_target).abs() < 1e-6 || obs_after >= lut_target * 0.95,
            "{ctx}: predicted observed chipload after must be ≥ ~95% of LUT target, got {obs_after} vs lut_target {lut_target}"
        );
        assert!(
            obs_before < lut_target,
            "{ctx}: pre-recal observed must be below LUT target (otherwise loop wouldn't fire), got {obs_before} vs {lut_target}"
        );

        // v3.3c (StrategyAndFeeds default): the strategy-aware
        // orchestrator rewrites entry_style Plunge → Ramp on this
        // case, replacing the v1.3 PlungeEntryUnstableAtDpp warning
        // with a positive StrategyRewrote rewrite.
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
                | SuggestWarning::AxialDocClampedByEnvelope { .. } => {}
                other => {
                    panic!("{ctx}: unexpected SuggestWarning variant slipped through: {other:?}")
                }
            }
        }
    }

    // ── Toolpath 10: 3D Rough 6 — same op family + tool as Back Rough.
    {
        let (_id, name, suggested) = find_case(&cases, 10);
        let ctx = format!("3D Rough 6 (tp {_id} / {name})");

        // Phase 3 envelope clamps DPP first; back-off then runs on the
        // clamped value. See the Back Rough test above for the rationale.
        let envelope_commanded = suggested
            .warnings
            .iter()
            .find_map(|w| match w {
                SuggestWarning::AxialDocClampedByEnvelope {
                    op_kind: "adaptive3d",
                    commanded_mm,
                    ..
                } => Some(*commanded_mm),
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
        let (_requested_dpp, capped_dpp) = assert_has_dpp_capped(&suggested.warnings, &ctx);
        assert!(
            capped_dpp > 0.0 && capped_dpp < 6.0,
            "{ctx}: capped DPP must drop below tool diameter, got {capped_dpp}"
        );

        let (_, feed_after, obs_before, obs_after, lut_target, _) =
            assert_has_feed_raised(&suggested.warnings, &ctx);
        // v3.0d: same Wanaka geometry as Back Rough → same operating
        // point (DPP 3.69 mm, feed 6912 mm/min on the median target).
        assert!(
            feed_after > 6500.0 && feed_after < 7500.0,
            "{ctx}: post-recal feed must land in ~6500-7500 mm/min (v3.0d RPM-idempotent median), got {feed_after}"
        );
        assert!(
            obs_before < lut_target,
            "{ctx}: pre-recal observed must be below LUT target, got {obs_before}"
        );
        assert!(
            obs_after >= lut_target * 0.95,
            "{ctx}: post-recal observed must be ≥ ~95% of LUT target, got {obs_after}"
        );
    }

    // ── Toolpath 11: 3D Finish 6 (DropCutter, 2 mm-tip tapered ball)
    //
    // The scallop-height target drives stepover to ~0.03 mm; the runtime
    // back-off raises it to ~0.23 mm. Feed then hits machine.max_feed
    // before the LUT min is reached → ChiploadStillLowAfterRecalibration.
    {
        let (_id, name, suggested) = find_case(&cases, 11);
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

        let (_, _, _, _, _, cap_hit) = assert_has_feed_raised(&suggested.warnings, &ctx);
        assert_eq!(
            cap_hit,
            Some(FeedRecalibrationCap::MaxFeed),
            "{ctx}: feed must hit MaxFeed cap on the Shapeoko machine envelope, got {cap_hit:?}"
        );

        // Paired ChiploadStillLowAfterRecalibration with blocking_cap == MaxFeed.
        let still_low = suggested.warnings.iter().find_map(|w| match w {
            SuggestWarning::ChiploadStillLowAfterRecalibration {
                predicted_observed_mm_per_tooth,
                lut_target_mm_per_tooth,
                blocking_cap,
                ..
            } => Some((
                *predicted_observed_mm_per_tooth,
                *lut_target_mm_per_tooth,
                *blocking_cap,
            )),
            _ => None,
        });
        let (still_low_obs, still_low_lut, blocking_cap) = still_low.unwrap_or_else(|| {
            panic!(
                "{ctx}: ChiploadStillLowAfterRecalibration must fire, got warnings: {:?}",
                suggested.warnings
            )
        });
        assert_eq!(
            blocking_cap,
            FeedRecalibrationCap::MaxFeed,
            "{ctx}: blocking_cap must be MaxFeed, got {blocking_cap:?}"
        );
        assert!(
            still_low_obs < still_low_lut,
            "{ctx}: still-low observed must be below LUT min ({still_low_lut}), got {still_low_obs}"
        );
    }

    // ── Toolpath 14: Pin Drill (drill family — chipload NotApplicable)
    {
        let (_id, name, suggested) = find_case(&cases, 14);
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
        let (_id, name, suggested) = find_case(&cases, 7);
        let ctx = format!("Holes (tp {_id} / {name})");
        assert_no_feed_raised(&suggested.warnings, &ctx);
        assert_no_dpp_capped(&suggested.warnings, &ctx);
    }

    // ── Toolpath 5: Rivers (back) — Project Curve, 60° V-bit (chipload NotApplicable)
    {
        let (_id, name, suggested) = find_case(&cases, 5);
        let ctx = format!("Rivers (tp {_id} / {name})");
        assert_no_feed_raised(&suggested.warnings, &ctx);
    }

    // ── Toolpath 6: Lakes (back, inside) — Project Curve, 60° V-bit
    {
        let (_id, name, suggested) = find_case(&cases, 6);
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
        .find(|t| t.id == 4)
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
