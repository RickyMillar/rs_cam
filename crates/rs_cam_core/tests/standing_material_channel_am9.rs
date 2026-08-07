//! A/M9 sentry — standing material must have a diagnostic CHANNEL, not a
//! `tracing::warn!` nobody installs a subscriber for.
//!
//! Background (`planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md`
//! §A/M9; `MEASUREMENT_DOMAINS.md` rows 37-42 and X-19): the scallop ring
//! cascade has always measured the region interior it failed to reach
//! (`ScallopReport::uncut_core_mm2`), and `0d1f307` carried that figure to
//! `ToolpathStats` plus the `geom.standing_material` diagnostic. Two gaps
//! remained, and this file pins both shut:
//!
//! 1. `narrate_toolpath` — the agent-facing surface — never mentioned it, so
//!    an agent narrating a truncated cascade saw nothing at all.
//! 2. `ToolpathStats::truncated_core_mm2` was a bare `f64` whose `0.0`
//!    meant BOTH "a cascade ran and left nothing" and "no cascade ran, so
//!    nothing was measured" (X-19, the silent-zero trap). Any "% left
//!    standing" a reader built on the second case was unfounded.
//!
//! Everything here runs the REAL production path — `ProjectSession::
//! generate_toolpath`, the entry point the GUI worker and the CLI share.
//!
//! Report-only by design: nothing gates on the figure and no verdict moves.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

mod common;

use common::meshes::sawtooth_plate;
use common::session::{
    generate, mesh_model, polygon_model, single_op_session, square_polygon, stock_over,
};
use common::tools::{ball_tool_config, endmill_tool_config, tapered_ball_tool_config};

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    TRUNCATED_CORE_DOMAIN, TRUNCATED_CORE_RESOLUTION, TRUNCATED_CORE_STAGE,
};
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern, ScallopConfig};
use rs_cam_core::compute::tool_config::ToolConfig;
use rs_cam_core::diagnostics::{Diagnostic, ids};
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::mesh::make_test_flat;
use rs_cam_core::session::ProjectSession;

// ── Fixtures ────────────────────────────────────────────────────────────
//
// C6: the tools, the corrugation, the model/stock wrappers and the one-op
// session builder this file invented now live in `tests/common/` — this file
// was the accidental template every later wave copied, so it is the one that
// had to prove the shared version is the same fixture. The corrugation is
// pinned bit-for-bit against this file's original generator by
// `common_fixtures_smoke_c6::sawtooth_plate_reproduces_its_donor`.

fn ball_tool() -> ToolConfig {
    ball_tool_config(3.0)
}

/// Tapered ball: the class where `radius()` (shank) and `cusp_radius()`
/// (tip sphere) diverge. Used on the CONTROL fixture — the cheap half of
/// the ball/tapered pair — to prove the channel is wired for it too.
fn tapered_ball_tool() -> ToolConfig {
    tapered_ball_tool_config(1.0, 10.0, 6.0)
}

fn scallop_op(scallop_height: f64) -> OperationConfig {
    OperationConfig::Scallop(ScallopConfig {
        scallop_height,
        tolerance: 0.1,
        ..ScallopConfig::default()
    })
}

/// The deliberately-truncated cascade of the A/M9 acceptance gate.
///
/// The corrugation is what does the truncating. `max_rings` is budgeted from
/// the FLAT-ground stepover, but `ring_stepover` takes the MIN across each
/// ring's samples, and a sharp convex apex on the tool-CENTRE surface shrinks
/// the cusp-limited advance by ~25%. Every ring therefore advances slower than
/// the budget assumed, and the cascade runs out of rings with the region
/// interior still standing — the mechanism `scallop.rs` documents and wanaka
/// hit for real. The 2 mm period is ~1.3× the tool diameter on purpose: a ball
/// bridges any feature below its own radius and the offset surface would read
/// flat (`finish_setup.rs`'s documented blind spot).
///
/// Measured at A/M9: `max_rings = 119`, 120 rings emitted, 13.3 mm² left
/// standing. **Re-measured at wave 14 (arc-carrying cascade): 0.70 mm².**
///
/// The cascade did not change its ring budget; it changed where its rings
/// put their vertices. `ring_stepover` takes the MIN over the ring's
/// samples, and the chord-flattened cascade's arc-join debris CLUSTERED at
/// reflex corners — the highest-curvature, lowest-stepover places on the
/// ring — so the minimum was being drawn from a sample set biased towards
/// the tightest ground. The arc cascade's vertices are spread by the flatten
/// policy instead, so the same minimum reads ~25% less conservative and the
/// cascade reaches 19× further into the interior on this fixture.
///
/// That is an improvement in reach and a WEAKENING of an already-known
/// defect's accidental safety margin (`CHECKPOINT_B_EVIDENCE.md`: "the fix is
/// in `ring_stepover`, not in the budget"). It is the M4 envelope oracle's
/// job to say whether the wider stepover holds the cusp — not this test's.
/// This one only has to prove the truncation is REPORTED, and 0.70 mm²
/// against a control that reads exactly 0.0 does that.
fn truncated_cascade_session() -> ProjectSession {
    let half = 25.0;
    single_op_session(
        stock_over(half, 1.5),
        ball_tool(),
        mesh_model(sawtooth_plate(half, 2.0, 1.0), "corrugation"),
        "Scallop",
        scallop_op(0.005),
    )
}

/// Control: the same operation on flat ground, where the cascade collapses
/// well inside its cap. Cheap, and the other half of the X-19 distinction.
fn collapsing_cascade_session(tool: ToolConfig) -> ProjectSession {
    single_op_session(
        stock_over(25.0, 1.0),
        tool,
        mesh_model(make_test_flat(50.0), "plate"),
        "Scallop",
        scallop_op(0.1),
    )
}

/// Control: an operation family that runs no ring cascade at all, so the
/// measure does not exist for it.
fn no_cascade_session() -> ProjectSession {
    let op = OperationConfig::Pocket(PocketConfig {
        stepover: 2.0,
        depth: 2.0,
        depth_per_pass: 2.0,
        feed_rate: 800.0,
        plunge_rate: 400.0,
        climb: true,
        pattern: PocketPattern::Contour,
        angle: 0.0,
        finishing_passes: 0,
        spindle_rpm: Some(18_000),
    });
    single_op_session(
        stock_over(15.0, 10.0),
        endmill_tool_config(3.0),
        polygon_model(vec![square_polygon(10.0)], "square"),
        "Pocket",
        op,
    )
}

fn measured(session: &ProjectSession) -> Option<f64> {
    session
        .get_result(0)
        .expect("generated result")
        .stats
        .truncated_core_mm2
}

fn standing_diagnostic(diags: &[Diagnostic]) -> Option<&Diagnostic> {
    diags
        .iter()
        .find(|d| d.id.as_str() == ids::GEOM_STANDING_MATERIAL)
}

/// Every user-visible standing-material string must declare what it
/// measured, when, and at what resolution (M1). A bare `mm²` is exactly the
/// unlabelled area the audit found being compared across domains.
fn declares_domain_stage_and_resolution(text: &str) {
    for needle in [
        TRUNCATED_CORE_DOMAIN,
        TRUNCATED_CORE_STAGE,
        TRUNCATED_CORE_RESOLUTION,
    ] {
        assert!(
            text.contains(needle),
            "standing-material text must declare {needle:?}:\n{text}"
        );
    }
}

// ── Acceptance ──────────────────────────────────────────────────────────

/// A/M9 acceptance gate, end to end on one generation: a deliberately
/// truncated cascade produces a non-zero standing-material figure that is
/// visible in `ToolpathStats`, in `narrate_toolpath`, in the diagnostics
/// list and in the MCP per-toolpath summary — each declaring domain, stage
/// and resolution — and it changes no verdict.
///
/// One test, one generation: the fixture costs ~9 s, and splitting the
/// assertions would multiply that by the assertion count.
#[test]
fn truncated_cascade_is_visible_on_every_surface() {
    let mut session = truncated_cascade_session();
    generate(&mut session, 0);

    // 1. Stats — `Some`, because a cascade ran and MEASURED this.
    let area = measured(&session)
        .expect("a scallop cascade measured its residual — this must not be `None`");
    println!("truncated cascade: standing material {area:.1} mm²");
    assert!(
        area > 0.1,
        "the corrugation forces every ring below the flat-ground budget, so the \
         cascade cannot reach the interior; got {area} mm². The bar is 0.1 mm² \
         rather than the 1.0 mm² A/M9 set because wave 14's arc-carrying \
         cascade cut this fixture's truncation from 13.3 mm² to 0.70 mm² — see \
         `truncated_cascade_session` for why. The control on flat ground reads \
         exactly 0.0, so the separation is still categorical."
    );

    // 2. Narration — the agent-facing surface that was silent before A/M9.
    let narration = session.narrate_toolpath(0).expect("narrate");
    assert!(
        narration.contains("Standing material:"),
        "narration must carry the figure:\n{narration}"
    );
    assert!(
        narration.contains(&format!("{area:.0} mm²")),
        "narration must print the measured area itself, not just a label:\n{narration}"
    );
    declares_domain_stage_and_resolution(&narration);

    // 3. Diagnostics — the GUI ribbon and MCP `get_toolpath_diagnostics`.
    let diags = session
        .diagnose_toolpath_with_trace(0, None)
        .expect("diagnose");
    let d = standing_diagnostic(&diags).expect("geom.standing_material diagnostic");
    assert!(
        d.message.contains(&format!("{area:.0} mm²")),
        "diagnostic must print the measured area: {}",
        d.message
    );
    declares_domain_stage_and_resolution(&d.message);

    // 4. Report-only (the A/M9 gate: report before enforcing).
    assert_ne!(
        d.severity,
        rs_cam_core::diagnostics::Severity::Blocking,
        "standing material must not block export yet"
    );
    assert_eq!(d.category, rs_cam_core::diagnostics::Category::Geometry);
    assert!(
        session.get_result(0).expect("result").stats.move_count > 0,
        "generation still succeeds — this is a report, not a rejection"
    );

    // 5. The MCP per-toolpath summary carries the number, not just prose.
    let project = session.diagnostics();
    let summary = project
        .per_toolpath
        .iter()
        .find(|t| t.toolpath_id == ToolpathId(0) || t.name == "Scallop")
        .expect("per-toolpath summary");
    assert_eq!(
        summary.truncated_core_mm2,
        Some(area),
        "the summary must carry the same measurement, not a re-derivation"
    );
}

/// X-19, half one: `Some(0.0)` means "measured, nothing standing". A
/// cascade that collapses inside its cap raises no diagnostic and narrates
/// as a measured none — not as silence.
#[test]
fn collapsing_cascade_reports_a_measured_zero() {
    let mut session = collapsing_cascade_session(ball_tool());
    generate(&mut session, 0);

    assert_eq!(
        measured(&session),
        Some(0.0),
        "a collapsing cascade MEASURED zero; `None` would claim it was never measured"
    );

    let narration = session.narrate_toolpath(0).expect("narrate");
    assert!(
        narration.contains("Standing material: none"),
        "a measured zero must read as a measurement:\n{narration}"
    );
    assert!(
        standing_diagnostic(
            &session
                .diagnose_toolpath_with_trace(0, None)
                .expect("diagnose")
        )
        .is_none(),
        "nothing is standing — the list stays quiet"
    );
}

/// The tapered-ball control. Cheap half of the ball/tapered pair: the tool
/// class whose cusp and envelope radii diverge must reach the same channel.
#[test]
fn tapered_ball_reaches_the_same_channel() {
    let mut session = collapsing_cascade_session(tapered_ball_tool());
    generate(&mut session, 0);

    assert_eq!(
        measured(&session),
        Some(0.0),
        "the tapered cascade ran and measured — the channel is not ball-only"
    );
    assert!(
        session
            .narrate_toolpath(0)
            .expect("narrate")
            .contains("Standing material: none")
    );
}

/// X-19, half two: an operation with no ring cascade reports `None` (not
/// measured), never `0.0`. Any ratio built on the latter is unfounded, and
/// narration must say so out loud rather than omitting the line.
#[test]
fn operation_without_a_cascade_reports_not_measured() {
    let mut session = no_cascade_session();
    generate(&mut session, 0);

    assert_eq!(
        measured(&session),
        None,
        "a pocket runs no ring cascade — the measure does not exist for it"
    );

    let narration = session.narrate_toolpath(0).expect("narrate");
    assert!(
        narration.contains("Standing material: not measured"),
        "narration must say the measure is absent rather than imply zero:\n{narration}"
    );
    assert!(
        standing_diagnostic(
            &session
                .diagnose_toolpath_with_trace(0, None)
                .expect("diagnose")
        )
        .is_none(),
        "an unmeasured cascade is not a defect claim"
    );

    let project = session.diagnostics();
    assert!(
        project
            .per_toolpath
            .iter()
            .all(|t| t.truncated_core_mm2.is_none()),
        "the MCP summary must serialise `null`, not 0.0, for an unmeasured op"
    );
}

// ── Wave 16 / Checkpoint E ──────────────────────────────────────────────

/// A6 — the rename's compatibility contract, proved on the real wire.
///
/// `ToolpathStats::standing_material_mm2` became `truncated_core_mm2` on
/// 2026-08-04 because the old name asserted the M4 oracle's *standing*
/// ("reached, left high") for a number that measures its *untouched*
/// ("never reached"). The ruling asked for the rename to be a
/// non-event for anything already reading the figure.
///
/// The audit behind that ruling assumed a `Deserialize` surface (project
/// files), and there is none — `ToolpathStats` has never been serde at all,
/// and every wire that carries the value is `Serialize`-only. So the
/// read-side mechanism the ruling names, `#[serde(alias)]`, is demonstrated
/// here on the CONSUMER side, which is where it can actually run: a reader
/// that adopts the alias parses BOTH an old document and the current wire
/// into the new field name. What production does is the emit-side
/// equivalent — keep publishing the old key beside the new one.
///
/// Gate, not characterisation. Fixed: the truncated-cascade fixture and its
/// measured area. Domain: XY-projected mm², generation stage.
#[test]
fn the_old_key_still_loads_and_the_wire_still_emits_it() {
    #[derive(serde::Deserialize)]
    struct LegacyReader {
        /// Exactly the migration a consumer performs.
        #[serde(alias = "standing_material_mm2")]
        truncated_core_mm2: Option<f64>,
    }

    let mut session = truncated_cascade_session();
    generate(&mut session, 0);
    let area = measured(&session).expect("a cascade measured its residual");

    // 1. A pre-rename document — only the old key exists — loads into the
    //    new name. This is the "old projects load" half of the ruling.
    //
    //    A hand-written literal, not the measured area: what is under test is
    //    the KEY mapping, and routing a 17-significant-digit `f64` out through
    //    JSON and back tests the parser's last ULP instead (it moved one, the
    //    first time this was written that way).
    let parsed: LegacyReader = serde_json::from_str(r#"{"standing_material_mm2": 13.75}"#)
        .expect("old key must still load");
    assert_eq!(
        parsed.truncated_core_mm2,
        Some(13.75),
        "a document written before the rename must deserialize unchanged"
    );

    // 2. The live wire emits BOTH keys, same value, so a script that never
    //    migrates keeps working and one that does gets the honest name.
    let project = session.diagnostics();
    let summary = project
        .per_toolpath
        .iter()
        .find(|t| t.toolpath_id == ToolpathId(0))
        .expect("per-toolpath summary");
    let json = serde_json::to_value(summary).expect("summary serialises");
    assert_eq!(
        json.get("truncated_core_mm2").and_then(|v| v.as_f64()),
        Some(area),
        "the new key must carry the measurement:\n{json}"
    );
    assert_eq!(
        json.get("standing_material_mm2").and_then(|v| v.as_f64()),
        Some(area),
        "the pre-rename key must still be emitted with the SAME value — a \
         consumer that never migrates must not silently read `null`:\n{json}"
    );

    // 3. The cost of (2), pinned rather than left to be discovered: a reader
    //    that adopts the alias AND reads the current wire sees the same field
    //    twice, and serde rejects that. The alias is for OLD documents; on the
    //    current wire a consumer takes `truncated_core_mm2` plainly. Written
    //    as an assertion because the tempting "fix" — dropping the legacy key
    //    — is exactly the compatibility break A6 was ruled to avoid.
    let via_alias: Result<LegacyReader, _> = serde_json::from_value(json);
    let err = via_alias
        .err()
        .expect("the alias cannot ALSO be used on the dual-key wire")
        .to_string();
    assert!(
        err.contains("duplicate field"),
        "expected serde's duplicate-field rejection, got: {err}"
    );
}

/// B8 — the untouched/standing split reaches the diagnostic message and the
/// MCP per-toolpath summary, not only narration.
///
/// Wave 15 shipped the split (`ScallopReport::untouched_mm2` /
/// `standing_mm2`) to narration and time-boxed the other two surfaces out.
/// The two halves are different measurements — one an exact hole-aware
/// polygon area, one an estimator — so the assertion here is not just that
/// numbers appear but that the message keeps them apart.
///
/// Gate, not characterisation.
#[test]
fn the_untouched_standing_split_reaches_the_diagnostic_and_the_summary() {
    let mut session = truncated_cascade_session();
    generate(&mut session, 0);

    let stats = &session.get_result(0).expect("result").stats;
    let untouched = stats
        .untouched_material_mm2
        .expect("wave 15 measured the hole-aware half on a truncated cascade");

    let diags = session
        .diagnose_toolpath_with_trace(0, None)
        .expect("diagnose");
    let d = standing_diagnostic(&diags).expect("geom.standing_material diagnostic");
    assert!(
        d.message.contains("never reached"),
        "the diagnostic must name the untouched half in the oracle's words:\n{}",
        d.message
    );
    assert!(
        d.message.contains("reached but left high"),
        "the diagnostic must name the standing half too — the whole point of \
         the split is that the two are not the same material:\n{}",
        d.message
    );
    assert!(
        d.message.contains("do not add them"),
        "an exact area and an estimator must not be presented as summable:\n{}",
        d.message
    );
    assert!(
        d.message
            .contains(&format!("{untouched:.0} mm² never reached")),
        "the diagnostic must print the measured hole-aware area itself:\n{}",
        d.message
    );

    // The MCP per-toolpath summary carries both halves as numbers.
    let project = session.diagnostics();
    let summary = project
        .per_toolpath
        .iter()
        .find(|t| t.toolpath_id == ToolpathId(0))
        .expect("per-toolpath summary");
    assert_eq!(
        summary.untouched_material_mm2,
        Some(untouched),
        "the summary must carry the same measurement, not a re-derivation"
    );
    assert_eq!(
        summary.reached_uncut_estimate_mm2, stats.reached_uncut_estimate_mm2,
        "and the estimator half travels with it, `None` included"
    );
}
