//! Phase 0B correctness golden: per-toolpath simulation aggregates, pinned.
//!
//! `planning/perf_review_2026-08-19/PERF_REVIEW.md` §0B. This is the net that
//! lets the metric-neutral simulation fixes claim to be metric-neutral:
//!
//! - **S2** (tile max-top early-out) is an *exact* skip — `h(r) ≥ 0` means a
//!   tile whose maximum top is already at or below the stamp's minimum depth
//!   cannot lose material. If it is exact, every number in the golden holds.
//! - **S3** (row-band parallel stamping) claims **bit-identical** results,
//!   because per-cell mutation order is preserved and `ray_blend_above` with
//!   `f < 1` is non-commutative. If it is bit-identical, every number holds.
//! - **S1** (swept-volume stamping) is deliberately NOT metric-neutral: it
//!   changes `pre_fresh` per sample. This golden is the thing that gets
//!   re-baselined for S1, in its own commit, with the reason in the message.
//!
//! ```text
//! cargo test -p rs_cam_core --test perf_golden_sim_metrics
//! UPDATE_PERF_GOLDENS=1 cargo test -p rs_cam_core --test perf_golden_sim_metrics
//! ```
//!
//! # Why the aggregates and not a trace hash
//!
//! A hash over the whole trace would fail on any change at all, including
//! ones that move no measured quantity (a field reorder, a new `Option`
//! defaulting to `None`). What the review needs to know is whether the
//! *measurements* moved, so the golden records the measurements — under
//! explicit per-field tolerances, and with the air-cut percentage recorded
//! under **both** denominators, because there are two and they are not the
//! same number (`simulation_cut.rs`'s `AirCutRatios`).
//!
//! # Tolerances
//!
//! Counts (`sample_count`, `move_count`, `collision_count`,
//! `rapid_collision_count`, `holder_collision_count`) are compared EXACTLY —
//! an off-by-one in a count is never rounding.
//!
//! Everything float is compared with a RELATIVE tolerance. Quantities that
//! come out of a single arithmetic expression get [`TIGHT_REL`]; quantities
//! accumulated across tens of thousands of samples get [`LOOSE_REL`], because
//! float addition is not associative and any change to iteration order — the
//! thing S3 explicitly does — reassociates the sum.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr,
    clippy::print_stdout
)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, HeightsConfig, RestAnalysisConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::{
    PocketConfig, PocketPattern, ProfileConfig, ZigzagConfig,
};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::profile::ProfileSide;
use rs_cam_core::session::{
    LoadedModel, ProjectEvidence, ProjectSession, SimulationOptions, ToolpathConfig,
};
use rs_cam_core::simulation_cut::AirCutRatios;
use serde::{Deserialize, Serialize};

/// Single-expression quantities: identical unless the arithmetic changed.
const TIGHT_REL: f64 = 1e-9;
/// Quantities summed across every sample in the trace. Reassociation of a
/// 10⁴-term float sum moves the last few ULPs; 1e-3 relative is far below
/// any change a human would call a metric change, and far above float noise.
const LOOSE_REL: f64 = 1e-3;

/// Simulation cell size. Pinned, not defaulted — every engagement and
/// air-cut number in the golden moves with it.
const SIM_RESOLUTION_MM: f64 = 1.0;

// ── The golden record ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolpathAggregate {
    name: String,
    op_kind: String,
    // Exact.
    sample_count: usize,
    move_count: usize,
    collision_count: usize,
    rapid_collision_count: usize,
    metrics_not_applicable: bool,
    // Accumulated across samples → LOOSE_REL.
    cutting_distance_mm: f64,
    rapid_distance_mm: f64,
    total_runtime_s: f64,
    cutting_runtime_s: f64,
    rapid_runtime_s: f64,
    air_cut_time_s: f64,
    low_engagement_time_s: f64,
    total_removed_volume_est_mm3: f64,
    average_engagement: f64,
    average_mrr_mm3_s: f64,
    // Derived from the above in one expression → TIGHT_REL against the
    // recomputation, LOOSE_REL against the golden (their inputs are loose).
    air_cut_pct_of_total_runtime: f64,
    air_cut_pct_of_cutting_time: f64,
    // Per-sample maxima: a max is order-independent, so these are TIGHT.
    peak_chipload_mm_per_tooth: f64,
    peak_axial_doc_mm: f64,
    peak_plunge_descent_mm: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProjectAggregate {
    resolution_mm: f64,
    // Exact.
    toolpath_count: usize,
    total_sample_count: usize,
    project_collision_count: usize,
    project_rapid_collision_count: usize,
    holder_collision_total: usize,
    triage_safety_count: usize,
    triage_action_count: usize,
    // Loose.
    project_air_cut_pct_of_total_runtime: f64,
    project_air_cut_pct_of_cutting_time: f64,
    project_average_engagement: f64,
    project_total_removed_volume_est_mm3: f64,
    per_toolpath: Vec<ToolpathAggregate>,
}

fn golden_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("perf_golden_sim_metrics.json")
}

// ── The fixture ─────────────────────────────────────────────────────────

/// Three 2.5D operations over one rectangular region, on one 6 mm end mill.
///
/// Nothing here is loaded from disk and nothing is random: the polygon is a
/// literal, the tool is built field by field, and every operation dial is
/// spelled out. A default that moves must not silently move the golden.
fn fixture_session() -> ProjectSession {
    let mut session = ProjectSession::new_empty();

    // 2D ops cut at negative Z (`project_2d_stock_z_frame`), so `origin_z`
    // is negative and the stock top lands on the world z = 0 plane.
    session.set_stock_config(StockConfig {
        x: 100.0,
        y: 80.0,
        z: 12.0,
        origin_x: -10.0,
        origin_y: -10.0,
        origin_z: -12.0,
        auto_from_model: false,
        ..StockConfig::default()
    });

    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.shank_diameter = 6.35;
    tool.shank_length = 20.0;
    tool.stickout = 45.0;
    tool.flute_count = 2;
    tool.name = "End Mill 6mm".to_owned();
    let tool_idx = session.add_tool(tool);
    let tool_id = session.tools()[tool_idx].id.0;

    let poly = Polygon2::new(vec![
        P2::new(5.0, 5.0),
        P2::new(75.0, 5.0),
        P2::new(75.0, 55.0),
        P2::new(5.0, 55.0),
    ]);
    let model_id = session.add_model(LoadedModel {
        id: 0,
        name: "perf_rect".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![poly])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://perf_rect.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    });

    let ops: Vec<(&str, OperationConfig)> = vec![
        (
            "Pocket",
            OperationConfig::Pocket(PocketConfig {
                stepover: 3.0,
                depth: 6.0,
                depth_per_pass: 3.0,
                feed_rate: 1000.0,
                plunge_rate: 400.0,
                climb: true,
                pattern: PocketPattern::Contour,
                angle: 0.0,
                finishing_passes: 0,
                spindle_rpm: Some(18_000),
            }),
        ),
        (
            "Zigzag",
            OperationConfig::Zigzag(ZigzagConfig {
                stepover: 3.0,
                depth: 3.0,
                depth_per_pass: 3.0,
                feed_rate: 1000.0,
                plunge_rate: 400.0,
                angle: 45.0,
                spindle_rpm: Some(18_000),
            }),
        ),
        (
            "Profile",
            OperationConfig::Profile(ProfileConfig {
                side: ProfileSide::Outside,
                depth: 6.0,
                depth_per_pass: 3.0,
                feed_rate: 1000.0,
                plunge_rate: 400.0,
                climb: true,
                tab_count: 0,
                tab_width: 6.0,
                tab_height: 2.0,
                finishing_passes: 0,
                compensation: rs_cam_core::compute::operation_configs::CompensationType::InComputer,
                spindle_rpm: Some(18_000),
            }),
        ),
    ];

    for (name, op) in ops {
        let op_type = op.op_type();
        let cfg = ToolpathConfig {
            id: ToolpathId(0),
            name: name.to_owned(),
            enabled: true,
            operation: op,
            dressups: DressupConfig::for_op(op_type),
            heights: HeightsConfig::default(),
            tool_id,
            model_id,
            pre_gcode: None,
            post_gcode: None,
            boundary: BoundaryConfig::default(),
            boundary_inherit: true,
            stock_source: StockSource::default(),
            coolant: CoolantMode::Off,
            face_selection: None,
            debug_options: ToolpathDebugOptions::default(),
            feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
            rest_analysis: RestAnalysisConfig::default(),
        };
        session.add_toolpath(0, cfg).expect("add toolpath");
    }

    session
}

/// Every simulation dial spelled out. `adaptive_feed_modulation` defaults to
/// `true` and would put a feed-modulation pass between the toolpath and the
/// numbers; the golden measures the sim, so it is off.
fn sim_options() -> SimulationOptions {
    SimulationOptions {
        resolution: SIM_RESOLUTION_MM,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    }
}

fn measure() -> ProjectAggregate {
    let cancel = AtomicBool::new(false);
    let mut session = fixture_session();
    for i in 0..session.toolpath_count() {
        session
            .generate_toolpath(i, &cancel)
            .expect("generation must succeed");
    }
    session
        .run_simulation(&sim_options(), &cancel)
        .expect("simulation completes");

    let holder_collisions = session.holder_collision_counts(&cancel);
    let holder_collision_total: usize = holder_collisions.iter().map(|(_, n)| *n).sum();

    let sim = session.simulation_result().expect("simulation result");
    let evidence = ProjectEvidence::from_simulation_with_holder_collisions(sim, holder_collisions);
    let diagnostics = session.diagnostics_with_evidence(&evidence);
    let triage = session.simulation_triage_with_diagnostics(&evidence, &diagnostics);

    let trace = sim
        .cut_trace
        .as_ref()
        .expect("metrics_enabled means a cut trace");

    let per_toolpath = diagnostics
        .per_toolpath
        .iter()
        .map(|d| {
            let summary = trace
                .toolpath_summaries
                .iter()
                .find(|s| s.toolpath_id == d.toolpath_id);
            ToolpathAggregate {
                name: d.name.clone(),
                op_kind: d.op_kind.clone(),
                sample_count: summary.map_or(0, |s| s.sample_count),
                move_count: d.move_count,
                collision_count: d.collision_count,
                rapid_collision_count: d.rapid_collision_count,
                metrics_not_applicable: summary.is_some_and(|s| s.metrics_not_applicable),
                cutting_distance_mm: d.cutting_distance_mm,
                rapid_distance_mm: d.rapid_distance_mm,
                total_runtime_s: summary.map_or(0.0, |s| s.total_runtime_s),
                cutting_runtime_s: summary.map_or(0.0, |s| s.cutting_runtime_s),
                rapid_runtime_s: summary.map_or(0.0, |s| s.rapid_runtime_s),
                air_cut_time_s: summary.map_or(0.0, |s| s.air_cut_time_s),
                low_engagement_time_s: summary.map_or(0.0, |s| s.low_engagement_time_s),
                total_removed_volume_est_mm3: summary
                    .map_or(0.0, |s| s.total_removed_volume_est_mm3),
                average_engagement: summary.map_or(0.0, |s| s.average_engagement),
                average_mrr_mm3_s: summary.map_or(0.0, |s| s.average_mrr_mm3_s),
                air_cut_pct_of_total_runtime: summary
                    .map_or(0.0, AirCutRatios::air_cut_pct_of_total_runtime),
                air_cut_pct_of_cutting_time: summary
                    .map_or(0.0, AirCutRatios::air_cut_pct_of_cutting_time),
                peak_chipload_mm_per_tooth: summary.map_or(0.0, |s| s.peak_chipload_mm_per_tooth),
                peak_axial_doc_mm: summary.map_or(0.0, |s| s.peak_axial_doc_mm),
                peak_plunge_descent_mm: summary.map_or(0.0, |s| s.peak_plunge_descent_mm),
            }
        })
        .collect();

    ProjectAggregate {
        resolution_mm: SIM_RESOLUTION_MM,
        toolpath_count: trace.summary.toolpath_count,
        total_sample_count: trace.summary.sample_count,
        project_collision_count: diagnostics.collision_count,
        project_rapid_collision_count: diagnostics.rapid_collision_count,
        holder_collision_total,
        triage_safety_count: triage.safety.len(),
        triage_action_count: triage.actions.len(),
        project_air_cut_pct_of_total_runtime: diagnostics.air_cut_pct_of_total_runtime,
        project_air_cut_pct_of_cutting_time: diagnostics.air_cut_pct_of_cutting_time,
        project_average_engagement: diagnostics.average_engagement,
        project_total_removed_volume_est_mm3: trace.summary.total_removed_volume_est_mm3,
        per_toolpath,
    }
}

// ── Comparison ──────────────────────────────────────────────────────────

fn close(actual: f64, expected: f64, rel: f64, what: &str, failures: &mut Vec<String>) {
    if actual == expected {
        return;
    }
    if actual.is_nan() != expected.is_nan() {
        failures.push(format!("{what}: NaN mismatch — got {actual}, golden {expected}"));
        return;
    }
    let scale = expected.abs().max(actual.abs()).max(1e-12);
    let err = (actual - expected).abs() / scale;
    if err > rel {
        failures.push(format!(
            "{what}: got {actual:.10}, golden {expected:.10} (rel err {err:.3e} > {rel:.0e})"
        ));
    }
}

fn exact<T: PartialEq + std::fmt::Debug + Copy>(
    actual: T,
    expected: T,
    what: &str,
    failures: &mut Vec<String>,
) {
    if actual != expected {
        failures.push(format!("{what}: got {actual:?}, golden {expected:?} (exact)"));
    }
}

fn compare(actual: &ProjectAggregate, golden: &ProjectAggregate) -> Vec<String> {
    let mut f = Vec::new();

    exact(actual.resolution_mm, golden.resolution_mm, "resolution_mm", &mut f);
    exact(actual.toolpath_count, golden.toolpath_count, "toolpath_count", &mut f);
    exact(
        actual.total_sample_count,
        golden.total_sample_count,
        "total_sample_count",
        &mut f,
    );
    exact(
        actual.project_collision_count,
        golden.project_collision_count,
        "project_collision_count",
        &mut f,
    );
    exact(
        actual.project_rapid_collision_count,
        golden.project_rapid_collision_count,
        "project_rapid_collision_count",
        &mut f,
    );
    exact(
        actual.holder_collision_total,
        golden.holder_collision_total,
        "holder_collision_total",
        &mut f,
    );
    exact(
        actual.triage_safety_count,
        golden.triage_safety_count,
        "triage_safety_count",
        &mut f,
    );
    exact(
        actual.triage_action_count,
        golden.triage_action_count,
        "triage_action_count",
        &mut f,
    );

    close(
        actual.project_air_cut_pct_of_total_runtime,
        golden.project_air_cut_pct_of_total_runtime,
        LOOSE_REL,
        "project_air_cut_pct_of_total_runtime",
        &mut f,
    );
    close(
        actual.project_air_cut_pct_of_cutting_time,
        golden.project_air_cut_pct_of_cutting_time,
        LOOSE_REL,
        "project_air_cut_pct_of_cutting_time",
        &mut f,
    );
    close(
        actual.project_average_engagement,
        golden.project_average_engagement,
        LOOSE_REL,
        "project_average_engagement",
        &mut f,
    );
    close(
        actual.project_total_removed_volume_est_mm3,
        golden.project_total_removed_volume_est_mm3,
        LOOSE_REL,
        "project_total_removed_volume_est_mm3",
        &mut f,
    );

    if actual.per_toolpath.len() != golden.per_toolpath.len() {
        f.push(format!(
            "per_toolpath length: got {}, golden {}",
            actual.per_toolpath.len(),
            golden.per_toolpath.len()
        ));
        return f;
    }

    for (a, g) in actual.per_toolpath.iter().zip(golden.per_toolpath.iter()) {
        let tag = format!("[{}]", g.name);
        exact(a.name.as_str(), g.name.as_str(), &format!("{tag}.name"), &mut f);
        exact(
            a.op_kind.as_str(),
            g.op_kind.as_str(),
            &format!("{tag}.op_kind"),
            &mut f,
        );
        exact(a.sample_count, g.sample_count, &format!("{tag}.sample_count"), &mut f);
        exact(a.move_count, g.move_count, &format!("{tag}.move_count"), &mut f);
        exact(
            a.collision_count,
            g.collision_count,
            &format!("{tag}.collision_count"),
            &mut f,
        );
        exact(
            a.rapid_collision_count,
            g.rapid_collision_count,
            &format!("{tag}.rapid_collision_count"),
            &mut f,
        );
        exact(
            a.metrics_not_applicable,
            g.metrics_not_applicable,
            &format!("{tag}.metrics_not_applicable"),
            &mut f,
        );

        for (av, gv, name) in [
            (a.cutting_distance_mm, g.cutting_distance_mm, "cutting_distance_mm"),
            (a.rapid_distance_mm, g.rapid_distance_mm, "rapid_distance_mm"),
            (a.total_runtime_s, g.total_runtime_s, "total_runtime_s"),
            (a.cutting_runtime_s, g.cutting_runtime_s, "cutting_runtime_s"),
            (a.rapid_runtime_s, g.rapid_runtime_s, "rapid_runtime_s"),
            (a.air_cut_time_s, g.air_cut_time_s, "air_cut_time_s"),
            (
                a.low_engagement_time_s,
                g.low_engagement_time_s,
                "low_engagement_time_s",
            ),
            (
                a.total_removed_volume_est_mm3,
                g.total_removed_volume_est_mm3,
                "total_removed_volume_est_mm3",
            ),
            (a.average_engagement, g.average_engagement, "average_engagement"),
            (a.average_mrr_mm3_s, g.average_mrr_mm3_s, "average_mrr_mm3_s"),
            (
                a.air_cut_pct_of_total_runtime,
                g.air_cut_pct_of_total_runtime,
                "air_cut_pct_of_total_runtime",
            ),
            (
                a.air_cut_pct_of_cutting_time,
                g.air_cut_pct_of_cutting_time,
                "air_cut_pct_of_cutting_time",
            ),
        ] {
            close(av, gv, LOOSE_REL, &format!("{tag}.{name}"), &mut f);
        }

        // Maxima are order-independent reductions — a reassociation cannot
        // move them, so a change here is a real change.
        for (av, gv, name) in [
            (
                a.peak_chipload_mm_per_tooth,
                g.peak_chipload_mm_per_tooth,
                "peak_chipload_mm_per_tooth",
            ),
            (a.peak_axial_doc_mm, g.peak_axial_doc_mm, "peak_axial_doc_mm"),
            (
                a.peak_plunge_descent_mm,
                g.peak_plunge_descent_mm,
                "peak_plunge_descent_mm",
            ),
        ] {
            close(av, gv, TIGHT_REL, &format!("{tag}.{name}"), &mut f);
        }
    }

    f
}

// ── Tests ───────────────────────────────────────────────────────────────

#[test]
fn sim_metrics_match_golden() {
    let actual = measure();
    let path = golden_path();

    if std::env::var("UPDATE_PERF_GOLDENS").is_ok_and(|v| v != "0" && !v.is_empty()) {
        let json = serde_json::to_string_pretty(&actual).expect("serialize aggregate");
        std::fs::create_dir_all(path.parent().expect("fixtures dir has a parent"))
            .expect("create fixtures dir");
        std::fs::write(&path, format!("{json}\n")).expect("write golden");
        eprintln!("UPDATE_PERF_GOLDENS: wrote {}", path.display());
        return;
    }

    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "golden {} unreadable ({e}); regenerate with \
             UPDATE_PERF_GOLDENS=1 cargo test -p rs_cam_core --test perf_golden_sim_metrics",
            path.display()
        )
    });
    let golden: ProjectAggregate = serde_json::from_str(&raw).expect("parse golden JSON");

    let failures = compare(&actual, &golden);
    assert!(
        failures.is_empty(),
        "simulation metrics moved against the Phase 0 golden ({} field(s)).\n{}\n\n\
         If this is a DELIBERATE re-baseline (e.g. PERF_REVIEW S1 swept-volume \
         stamping), regenerate with UPDATE_PERF_GOLDENS=1 and say so in the \
         commit message.",
        failures.len(),
        failures.join("\n")
    );
}

/// The golden is only worth anything if the fixture actually cuts. A
/// simulation that removed nothing would pin a page of zeros and pass
/// forever — the vacuity failure `gate_population_vacuity_xvac` exists for,
/// applied to a golden instead of a gate.
#[test]
fn golden_fixture_is_not_vacuous() {
    let a = measure();
    assert_eq!(a.per_toolpath.len(), 3, "fixture must carry three toolpaths");
    assert!(
        a.total_sample_count > 500,
        "fixture produced only {} cut samples — too few to detect a metric change",
        a.total_sample_count
    );
    assert!(
        a.project_total_removed_volume_est_mm3 > 100.0,
        "fixture removed only {:.3} mm³ — the golden would be pinning air",
        a.project_total_removed_volume_est_mm3
    );
    for tp in &a.per_toolpath {
        assert!(
            tp.sample_count > 0,
            "[{}] produced no cut samples; it contributes nothing to the golden",
            tp.name
        );
        assert!(
            tp.cutting_distance_mm > 0.0,
            "[{}] emitted no cutting distance",
            tp.name
        );
    }
}
