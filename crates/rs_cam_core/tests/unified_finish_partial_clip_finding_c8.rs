//! C8 sentry — a finish band that is only PARTLY machined must say so.
//!
//! `ANTIPATTERNS_BACKLOG.md` P8: "Partial height clipping is unreported
//! (only total band collapse produces the D1 finding)."
//!
//! Wave D1 built the instrument: the `VerySteep` arm compares the Z levels a
//! band's own surface span asks for against the ones the resolved heights
//! allow, on EVERY band. It then threw the measurement away unless the
//! region emitted no cutting at all, reasoning that a band which still cuts
//! is "a different (and much quieter) problem" and reporting it would bury
//! the total collapse.
//!
//! That is an argument about SEVERITY, not about whether to report. A band
//! that machined the top 2 mm of an 8.5 mm groove and stopped is an
//! unfinished feature, and it was silent on every surface. C8 keeps the two
//! apart — separate collections, separate findings, `Caution` vs `Info` —
//! so the loud one still cannot be buried, and the quiet one exists.
//!
//! ## Contract
//!
//! * **Kind**: GATE (default CI, synthetic, seconds).
//! * **Fixture**: the M2.1 / Wave-D1 `two_groove_plateau`, whose very-steep
//!   groove runs from z = 0 down to z ≈ −8.57. Three arms differ ONLY in
//!   `HeightsConfig`:
//!   - `bottom_z = −4.0` — the ladder is SHORTENED, cutting survives → the
//!     new finding;
//!   - `bottom_z = −9.0` (M2.1's pin) — the whole groove is laddered →
//!     nothing reported, the non-vacuity control;
//!   - `HeightsConfig::default()` — Auto/Auto collapses the ladder entirely
//!     → the Wave-D1 finding, and NOT this one. The two are disjoint by
//!     construction and this arm proves it.
//! * **Report-only**: generation still succeeds, no verdict moves, and no
//!   gate consumes the number.
//!
//! ## Red-first evidence
//!
//! Before C8, on the `bottom_z = −4.0` arm: `session.generate_toolpath`
//! succeeded and cut, `result.stats` carried no clip channel at all (the
//! field did not exist), and `narrate_toolpath` /
//! `diagnose_toolpath_with_trace` contained ZERO occurrences of "partly
//! machined" or `geom.clipped_band`. The measurement was taken inside
//! `unified_finish.rs` and discarded at the end of the loop iteration.
//!
//! ## Why this cannot pass vacuously
//!
//! Every arm asserts `cutting_distance > 0.0` before reading the channel,
//! and the clipped arm additionally asserts `resolved_levels <
//! planned_levels` — so a fixture that stopped producing a very-steep band,
//! or heights that stopped clipping, fail rather than report "clean".

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout
)]

mod common;

use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::HeightsConfig;
use rs_cam_core::compute::operation_configs::UnifiedFinishConfig;
use rs_cam_core::diagnostics::{Diagnostic, Severity, ids};
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::session::ProjectSession;

use common::meshes::extrude_profile;
use common::session::{mesh_model, pinned_heights, single_op_session_with};
use common::tools::tapered_ball_tool_config;

// ── Fixture ─────────────────────────────────────────────────────────────

/// 30 × 30 mm plateau, a 60° mid-steep groove and an 85° very-steep groove
/// whose floor sits at z ≈ −8.5725. Identical profile to
/// `unified_finish_dropped_band_finding_d1.rs` and
/// `unified_finish_tapered_end_to_end_m21.rs` — the three files must agree
/// about the mesh or their findings are not comparable.
fn two_groove_plateau() -> TriangleMesh {
    let profile = [
        (-15.0_f64, 0.0_f64),
        (-6.0, 0.0),
        (-5.0, -1.732_050_8),
        (-4.0, 0.0),
        (4.0, 0.0),
        (4.75, -8.572_539),
        (5.5, 0.0),
        (15.0, 0.0),
    ];
    extrude_profile(&profile, -15.0, 15.0)
}

fn stock() -> StockConfig {
    StockConfig {
        x: 34.0,
        y: 34.0,
        z: 9.0,
        origin_x: -17.0,
        origin_y: -17.0,
        origin_z: -9.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn session_with(heights: HeightsConfig) -> ProjectSession {
    let mut session = single_op_session_with(
        stock(),
        tapered_ball_tool_config(1.0, 7.0, 6.0),
        mesh_model(two_groove_plateau(), "two_groove_plateau"),
        "Unified finish",
        OperationConfig::UnifiedFinish(UnifiedFinishConfig::default()),
        |cfg| cfg.heights = heights,
    );
    common::session::generate(&mut session, 0);
    session
}

/// `bottom_z` pinned partway down the very-steep groove: the ladder is
/// shortened, but the levels above the pin still cut.
fn partly_clipping_heights() -> HeightsConfig {
    pinned_heights(0.0, -4.0)
}

fn clipped_diagnostic(diags: &[Diagnostic]) -> Option<&Diagnostic> {
    diags
        .iter()
        .find(|d| d.id.as_str() == ids::GEOM_CLIPPED_BAND)
}

fn dropped_diagnostic(diags: &[Diagnostic]) -> Option<&Diagnostic> {
    diags
        .iter()
        .find(|d| d.id.as_str() == ids::GEOM_UNMACHINED_BAND)
}

// ── Gates ────────────────────────────────────────────────────────────────

/// GATE 1 — a shortened-but-cutting ladder is reported on every surface:
/// the typed channel, narration, and diagnostics.
#[test]
fn a_partly_clipped_band_is_measured_and_reported_everywhere() {
    let session = session_with(partly_clipping_heights());
    let result = session.get_result(0).expect("generated result");

    // Non-vacuity first: this arm must actually CUT, or it is testing the
    // Wave-D1 total-collapse case under a different name.
    assert!(
        result.stats.cutting_distance > 0.0,
        "the partly-clipped arm must still cut, or it proves nothing"
    );
    assert!(
        result.stats.dropped_band.is_none(),
        "a band that still cuts is not DROPPED — the two findings are \
         disjoint by construction: {:?}",
        result.stats.dropped_band
    );

    // (a) The typed channel.
    let clipped = result
        .stats
        .clipped_band
        .as_deref()
        .expect("bottom_z = -4.0 shortens the VerySteep ladder — this MUST be measured");
    println!(
        "C8 partial clip: band={} regions={} area={:.2} mm² clip={} @ {:.3} mm; \
         requested {:.3}..{:.3} -> delivered {:.3}..{:.3}; \
         levels {} -> {}; worst lost {:.3} mm",
        clipped.band_label,
        clipped.region_count,
        clipped.area_mm2,
        clipped.clip_label,
        clipped.clip_z_mm,
        clipped.requested_bottom_z_mm,
        clipped.requested_top_z_mm,
        clipped.delivered_bottom_z_mm,
        clipped.delivered_top_z_mm,
        clipped.planned_levels,
        clipped.resolved_levels,
        clipped.max_lost_height_mm,
    );

    assert_eq!(clipped.band_label, "VerySteep");
    assert_eq!(
        clipped.clip_label, "bottom_z",
        "the FLOOR is what bit — that is the dial an operator can raise"
    );
    assert!(
        (clipped.clip_z_mm - (-4.0)).abs() < 1e-9,
        "the reported clip height must be the operator's own number: {}",
        clipped.clip_z_mm
    );

    // The whole point of the finding: requested vs delivered, not just a
    // level count.
    assert!(
        clipped.resolved_levels < clipped.planned_levels,
        "a partial clip means FEWER levels laddered: {} vs {}",
        clipped.resolved_levels,
        clipped.planned_levels
    );
    assert!(
        clipped.resolved_levels > 0,
        "…but not zero, which would be the Wave-D1 case"
    );
    assert!(
        clipped.requested_bottom_z_mm < clipped.delivered_bottom_z_mm - 1e-9,
        "the delivered FLOOR must sit above the requested one: {} vs {}",
        clipped.delivered_bottom_z_mm,
        clipped.requested_bottom_z_mm
    );
    assert!(
        clipped.max_lost_height_mm > 1.0,
        "the groove floor is at z ≈ -8.57 and the pin is at -4.0, so several \
         mm of wall are unfinished: {}",
        clipped.max_lost_height_mm
    );
    assert!(
        clipped.area_mm2 > 0.0,
        "the clipped band must have real area"
    );

    // (b) Narration — the agent-facing surface.
    let narration = session.narrate_toolpath(0).expect("narrate");
    assert!(
        narration.contains("Partly machined band:"),
        "narration must carry the channel:\n{narration}"
    );
    assert!(
        narration.contains("unfinished"),
        "narration must say what was left, not only that something was \
         clipped:\n{narration}"
    );
    assert!(
        narration.contains("Report-only"),
        "the report-only contract must travel with the number:\n{narration}"
    );

    // (c) Diagnostics.
    let diags = session
        .diagnose_toolpath_with_trace(0, None)
        .expect("diagnose");
    let diag = clipped_diagnostic(&diags).expect("the clipped-band diagnostic must be raised");
    assert_eq!(
        diag.severity,
        Severity::Info,
        "a partly machined band is INFO — its loud sibling is the dropped \
         band, and a Caution here would flatten the distinction"
    );
    assert!(
        dropped_diagnostic(&diags).is_none(),
        "the total-collapse diagnostic must NOT fire on a band that cut"
    );
    assert!(
        diag.message.contains("bottom_z"),
        "the message must name the dial: {}",
        diag.message
    );
}

/// GATE 2 — non-vacuity. Pin the heights the way M2.1 does and the same
/// fixture ladders the whole groove: nothing clipped, and narration says so
/// rather than staying silent.
#[test]
fn fully_pinned_heights_clip_nothing_and_say_so() {
    let session = session_with(pinned_heights(0.0, -9.0));
    let result = session.get_result(0).expect("generated result");
    assert!(
        result.stats.cutting_distance > 0.0,
        "control must actually cut, or it proves nothing"
    );
    assert!(
        result.stats.clipped_band.is_none(),
        "the whole groove is inside the pinned range — nothing may be \
         reported clipped: {:?}",
        result.stats.clipped_band
    );

    let narration = session.narrate_toolpath(0).expect("narrate");
    assert!(
        narration.contains("Partly machined band: none"),
        "narration must state the measured-clean case explicitly:\n{narration}"
    );

    let diags = session
        .diagnose_toolpath_with_trace(0, None)
        .expect("diagnose");
    assert!(
        clipped_diagnostic(&diags).is_none(),
        "a clean run must not raise the diagnostic"
    );
}

/// GATE 3 — the two findings are DISJOINT. Auto/Auto heights collapse the
/// ladder entirely: that is Wave D1's finding, and this one must stay quiet.
/// Without this arm, "partial clip" could quietly become a synonym for
/// "clip" and swallow the case that leaves a feature at full stock.
#[test]
fn a_totally_collapsed_band_is_dropped_not_clipped() {
    let session = session_with(HeightsConfig::default());
    let result = session.get_result(0).expect("generated result");

    assert!(
        result.stats.dropped_band.is_some(),
        "Auto heights collapse the VerySteep ladder — Wave D1's finding must \
         still fire"
    );
    assert!(
        result.stats.clipped_band.is_none(),
        "a band that emitted NO cutting is dropped, never merely clipped: {:?}",
        result.stats.clipped_band
    );

    let narration = session.narrate_toolpath(0).expect("narrate");
    assert!(
        narration.contains("Unmachined band:") && !narration.contains("Unmachined band: none"),
        "the loud finding must still be loud:\n{narration}"
    );
    assert!(
        narration.contains("Partly machined band: none"),
        "…and the quiet one must state its own clean case, not be absent:\n{narration}"
    );
}

/// GATE 4 — the X-19 silent-zero control. An operation that plans no bands
/// reports **not measured**, never "nothing clipped".
#[test]
fn an_operation_with_no_bands_reports_not_measured() {
    use rs_cam_core::compute::operation_configs::DropCutterConfig;

    let mut session = single_op_session_with(
        stock(),
        tapered_ball_tool_config(1.0, 7.0, 6.0),
        mesh_model(two_groove_plateau(), "two_groove_plateau"),
        "Drop cutter",
        OperationConfig::DropCutter(DropCutterConfig::default()),
        |cfg| cfg.heights = pinned_heights(0.0, -9.0),
    );
    common::session::generate(&mut session, 0);

    let result = session.get_result(0).expect("generated result");
    assert!(
        result.stats.clipped_band.is_none(),
        "a drop-cutter raster plans no bands: {:?}",
        result.stats.clipped_band
    );
    let narration = session.narrate_toolpath(0).expect("narrate");
    assert!(
        narration.contains("Partly machined band: not measured"),
        "an operation that plans no bands must say NOT MEASURED — absence of \
         a number is not a zero (X-19):\n{narration}"
    );
}
