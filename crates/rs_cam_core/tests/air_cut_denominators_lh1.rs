//! LH-1 sentry — "air cut %" must never travel without its denominator.
//!
//! `planning/review_2026-07-29/MEASUREMENT_DOMAINS.md` §0 LH-1 / §2 X-3.
//!
//! `air_cut_time_s` is a duration. Turning it into a percentage requires
//! choosing what it is a percentage *of*, and this codebase chose both:
//!
//! - **total runtime** (cutting + rapids) in `ProjectDiagnostics`, the GUI
//!   banner and chips, the CLI verdict, and every
//!   `OperationType::air_cut_high_threshold_pct` band;
//! - **cutting runtime** (rapids excluded) in the MCP `narrate_toolpath`
//!   line and in `CLAUDE.md`'s metric caveats.
//!
//! On a retract-heavy toolpath the two are far apart, so a reader who takes
//! "air cut 12%" from one surface and compares it to a threshold tuned on the
//! other is comparing two different measures. Wave D2 changed **no number**:
//! it published both under names that say which is which. This file is the
//! net that keeps them named.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::collections::BTreeMap;

use rs_cam_core::ToolpathId;
use rs_cam_core::simulation_cut::{
    AirCutRatios, SimulationCutSummary, SimulationCutTrace, SimulationToolpathCutSummary,
};

/// A retract-heavy toolpath: 100 s total, 25 s of it cutting feed (75 s of
/// rapids/retracts), 10 s of that cutting time in air.
///
/// Air cut is 10% of total runtime and 40% of cutting time — a 4× gap. Any
/// surface that shows one of these without saying which is lying by omission.
fn retract_heavy_summary() -> SimulationToolpathCutSummary {
    SimulationToolpathCutSummary {
        toolpath_id: ToolpathId(7),
        sample_count: 100,
        total_runtime_s: 100.0,
        cutting_runtime_s: 25.0,
        rapid_runtime_s: 75.0,
        air_cut_time_s: 10.0,
        low_engagement_time_s: 2.0,
        average_engagement: 0.31,
        peak_chipload_mm_per_tooth: 0.05,
        peak_axial_doc_mm: 2.0,
        peak_plunge_descent_mm: 0.0,
        total_removed_volume_est_mm3: 500.0,
        average_mrr_mm3_s: 20.0,
        metrics_not_applicable: false,
        per_kinematics: BTreeMap::new(),
        runtime_by_intent: None,
    }
}

fn retract_heavy_project_summary() -> SimulationCutSummary {
    SimulationCutSummary {
        sample_count: 100,
        toolpath_count: 1,
        total_runtime_s: 100.0,
        cutting_runtime_s: 25.0,
        rapid_runtime_s: 75.0,
        air_cut_time_s: 10.0,
        low_engagement_time_s: 2.0,
        average_engagement: 0.31,
        ..SimulationCutSummary::default()
    }
}

/// The two readings are genuinely different numbers on a realistic fixture —
/// which is the entire reason they need different names.
#[test]
fn the_two_air_cut_denominators_disagree() {
    let s = retract_heavy_summary();

    let of_total = s.air_cut_pct_of_total_runtime();
    let of_cutting = s.air_cut_pct_of_cutting_time();

    assert!((of_total - 10.0).abs() < 1e-9, "got {of_total}");
    assert!((of_cutting - 40.0).abs() < 1e-9, "got {of_cutting}");
    assert!(
        of_cutting > of_total * 3.0,
        "the fixture must separate the two measures or it proves nothing: \
         {of_cutting} vs {of_total}"
    );

    // The ordering is structural, not fixture luck: total runtime is always
    // ≥ cutting runtime, so the cutting-time reading is always the larger.
    // Applying a total-runtime threshold to it over-fires by construction.
    assert!(of_cutting >= of_total);

    // Degenerate cases stay at zero rather than producing NaN/inf.
    let idle = SimulationToolpathCutSummary {
        total_runtime_s: 0.0,
        cutting_runtime_s: 0.0,
        air_cut_time_s: 0.0,
        ..retract_heavy_summary()
    };
    assert!((idle.air_cut_pct_of_total_runtime()).abs() < 1e-12);
    assert!((idle.air_cut_pct_of_cutting_time()).abs() < 1e-12);
}

/// The MCP wire (`ProjectDiagnostics`) publishes both, named, and the legacy
/// key keeps its total-runtime value so existing agents keep working.
#[test]
fn project_diagnostics_wire_names_both_denominators() {
    let diag = rs_cam_core::session::ProjectDiagnostics {
        total_runtime_s: 100.0,
        air_cut_percentage: 10.0,
        air_cut_pct_of_total_runtime: 10.0,
        air_cut_pct_of_cutting_time: 40.0,
        average_engagement: 0.31,
        collision_count: 0,
        rapid_collision_count: 0,
        per_toolpath: Vec::new(),
        verdict: "OK".to_owned(),
        verdicts: Vec::new(),
    };

    let json = serde_json::to_value(&diag).expect("ProjectDiagnostics serialises");
    let obj = json.as_object().expect("object");

    assert_eq!(
        obj.get("air_cut_pct_of_total_runtime")
            .and_then(serde_json::Value::as_f64),
        Some(10.0)
    );
    assert_eq!(
        obj.get("air_cut_pct_of_cutting_time")
            .and_then(serde_json::Value::as_f64),
        Some(40.0)
    );
    // Legacy key: same value as the total-runtime reading, never the other.
    assert_eq!(
        obj.get("air_cut_percentage")
            .and_then(serde_json::Value::as_f64),
        Some(10.0),
        "the legacy key must keep its (total-runtime) value — renaming its \
         meaning would silently move every consumer's threshold"
    );
    assert_eq!(
        obj.len(),
        10,
        "serialize_struct arity must match the field count, or serde formats \
         that count fields (bincode, MessagePack) truncate: {obj:?}"
    );
}

/// The agent-facing narration line names BOTH denominators. It reports the
/// cutting-time reading (as it always has) and prints the total-runtime one
/// beside it, because that is the number the GUI banner and every
/// `air_cut_high_threshold_pct` band are compared against.
#[test]
fn narration_air_cut_line_names_its_denominators() {
    use rs_cam_core::compute::cutter::build_cutter;
    use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use rs_cam_core::geo::P3;
    use rs_cam_core::narrate::{ToolpathNarrationContext, narrate_toolpath_with_context};
    use rs_cam_core::toolpath::Toolpath;
    use rs_cam_core::toolpath_spans::AnnotatedToolpath;

    let mut toolpath = Toolpath::new();
    toolpath.rapid_to(P3::new(0.0, 0.0, 10.0));
    toolpath.feed_to(P3::new(10.0, 0.0, 2.0), 1000.0);

    let trace = SimulationCutTrace {
        summary: retract_heavy_project_summary(),
        toolpath_summaries: vec![retract_heavy_summary()],
        ..SimulationCutTrace::test_fixture()
    };

    let tool = build_cutter(&ToolConfig::new_default(ToolId(0), ToolType::EndMill));
    let context = ToolpathNarrationContext {
        toolpath_id: Some(ToolpathId(7)),
        toolpath_name: Some("Back Rough"),
        operation_label: Some("adaptive3d"),
        operation_kind: Some(rs_cam_core::compute::catalog::OperationType::Adaptive3d),
        depth_per_pass_mm: Some(3.0),
        stepover_mm: Some(0.84),
        tool_diameter_mm: Some(6.0),
        feed_rate_mm_min: Some(1000.0),
        spindle_rpm: Some(18_000),
        flute_count: Some(2),
        is_drill_cycle: false,
        material: None,
        standing_material_mm2: None,
        dropped_band: None,
        tip_float: None,
    };

    let report = narrate_toolpath_with_context(
        &AnnotatedToolpath::new(toolpath),
        None,
        Some(&trace),
        None,
        &tool,
        &context,
    );

    let line = report
        .lines()
        .find(|l| l.contains("air-cut"))
        .unwrap_or_else(|| panic!("no air-cut line in narration:\n{report}"));

    assert!(
        line.contains("40.0% of CUTTING time"),
        "the cutting-time reading must be named: {line}"
    );
    assert!(
        line.contains("10.0% of TOTAL runtime"),
        "the total-runtime reading — the one the thresholds use — must ship \
         beside it, named: {line}"
    );
}

/// Grep-style sentry (the plan's §6 slice-3 gate, `MEASUREMENT_DOMAINS.md`
/// :564): every surface that turns `air_cut_time_s` into a percentage must go
/// through a named accessor, and every label that shows one must say which
/// denominator it used.
///
/// A source-text check is the honest instrument here: the offending sites are
/// `format!` strings inside egui closures and `serde_json::json!` literals,
/// none of which is reachable from a unit test. If this test fails because a
/// file moved, retarget it — do not delete it.
#[test]
fn no_surface_divides_air_cut_by_a_runtime_without_naming_it() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root")
        .to_path_buf();

    // (file, what its air-cut label must say)
    let surfaces: [(&str, &[&str]); 5] = [
        (
            "crates/rs_cam_viz/src/ui/sim_diagnostics.rs",
            &["% of total runtime)", "{pct:.0}% of total runtime"],
        ),
        (
            "crates/rs_cam_viz/src/controller/events/compute.rs",
            &["air_cut_pct_of_total_runtime", "air_cut_pct_of_cutting_time"],
        ),
        (
            "crates/rs_cam_viz/src/app/mcp.rs",
            &["air_cut_pct_of_total_runtime", "air_cut_pct_of_cutting_time"],
        ),
        (
            "crates/rs_cam_cli/src/main.rs",
            &["% of total runtime", "% of cutting time"],
        ),
        (
            "crates/rs_cam_cli/src/project.rs",
            &["air cutting of total runtime"],
        ),
    ];

    for (rel, required) in surfaces {
        let path = repo.join(rel);
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("LH-1 sentry needs retargeting: {rel}: {e}"));

        assert!(
            !src.contains("air_cut_time_s / "),
            "{rel} divides air_cut_time_s by hand — use \
             AirCutRatios::air_cut_pct_of_total_runtime / _of_cutting_time so the \
             denominator is named at the call site (MEASUREMENT_DOMAINS.md LH-1)"
        );
        for needle in required {
            assert!(
                src.contains(needle),
                "{rel} must name its air-cut denominator; expected to find \
                 {needle:?}"
            );
        }
    }
}
