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
//!
//! # Two arms, and why the second one exists
//!
//! The original fixture is **2.5D**: pocket + zigzag + profile on a polygon
//! model, one flat end mill. It exercises the stamp kernel hard, and it
//! contains no 3D finishing kinematics whatsoever — every cutting sample it
//! emits is `Linear` or `Plunge`. A net built only from that arm would pin
//! the straight-and-level path and let a bug in the arc or helix branch
//! (`dexel_stock/simulation.rs`'s `MoveType::ArcCW`/`ArcCCW` linearisation,
//! and `classify_cut_kinematics`'s XY+Z case) through in silence — and those
//! are exactly the branches S1 (swept-volume stamping), S2 (tile early-out)
//! and S3 (row-band parallel stamping) reshape.
//!
//! So there is a second arm: a **3D** fixture — synthetic hemisphere mesh,
//! ball-nose cutter, drop-cutter + waterline with arc fitting on — and both
//! arms record the per-kinematics block. The 3D arm's own test
//! [`three_d_arm_covers_arc_and_helix_kinematics`] asserts the `Arc` and
//! `Helix` classes are actually populated, so the arm cannot quietly decay
//! into another Linear-only fixture the day a generator stops emitting arcs.
//!
//! ```text
//! cargo test -p rs_cam_core --test perf_golden_sim_metrics
//! ```

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
use rs_cam_core::compute::config::HeightMode;
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, HeightsConfig, RestAnalysisConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::{
    DropCutterConfig, PocketConfig, PocketPattern, ProfileConfig, WaterlineConfig, ZigzagConfig,
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
use rs_cam_core::simulation_cut::{AirCutRatios, CutKinematics, KinematicsSummary};
use serde::{Deserialize, Serialize};

/// Single-expression quantities: identical unless the arithmetic changed.
const TIGHT_REL: f64 = 1e-9;
/// Quantities summed across every sample in the trace. Reassociation of a
/// 10⁴-term float sum moves the last few ULPs; 1e-3 relative is far below
/// any change a human would call a metric change, and far above float noise.
const LOOSE_REL: f64 = 1e-3;

/// Simulation cell size for the 2.5D arm. Pinned, not defaulted — every
/// engagement and air-cut number in the golden moves with it.
const SIM_RESOLUTION_MM: f64 = 1.0;

/// Simulation cell size for the 3D arm. Finer than the 2.5D arm because the
/// fixture is smaller and because a ball tip against a curved surface needs a
/// cell well under the tip radius before radial engagement is measurable at
/// all (`PERP_COVERAGE_GATE`'s `cell ≲ √(2·R_tip·d − d²)` condition).
const SIM_RESOLUTION_3D_MM: f64 = 0.5;

// ── The golden record ───────────────────────────────────────────────────

/// One `CutKinematics` class's row of the per-toolpath `per_kinematics` map.
///
/// This block is the whole reason the 3D arm exists: it is the only place the
/// trace says *which kind of motion* produced a measurement, so it is the only
/// place a golden can prove an arc or a helix was measured rather than
/// silently skipped.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct KinematicsAggregate {
    /// `CutKinematics` rendered with `Debug` — `"Linear"`, `"Arc"`, …
    kind: String,
    // Exact.
    sample_count: usize,
    // Accumulated → LOOSE_REL.
    cutting_runtime_s: f64,
    average_radial_woc_fraction: f64,
    average_leading_edge_speed_mm_min: f64,
    /// `None` means **not measured** — no sample in this class carried an
    /// arc. The None-ness is compared exactly; only the value is toleranced.
    average_arc_radians: Option<f64>,
    // Per-sample maxima → TIGHT_REL.
    peak_radial_woc_fraction: f64,
    peak_axial_doc_mm: f64,
    peak_plunge_descent_mm: f64,
    peak_chip_thickness_mm: Option<f64>,
}

fn kinematics_aggregates(
    per_kinematics: &std::collections::BTreeMap<CutKinematics, KinematicsSummary>,
) -> Vec<KinematicsAggregate> {
    per_kinematics
        .iter()
        .map(|(kind, s)| KinematicsAggregate {
            kind: format!("{kind:?}"),
            sample_count: s.sample_count,
            cutting_runtime_s: s.cutting_runtime_s,
            average_radial_woc_fraction: s.average_radial_woc_fraction,
            average_leading_edge_speed_mm_min: s.average_leading_edge_speed_mm_min,
            average_arc_radians: s.average_arc_radians,
            peak_radial_woc_fraction: s.peak_radial_woc_fraction,
            peak_axial_doc_mm: s.peak_axial_doc_mm,
            peak_plunge_descent_mm: s.peak_plunge_descent_mm,
            peak_chip_thickness_mm: s.peak_chip_thickness_mm,
        })
        .collect()
}

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
    /// Axis-aware breakdown. `#[serde(default)]` so a pre-3D-arm golden JSON
    /// still parses — and then fails loudly on the length comparison rather
    /// than on a deserialisation error, which is the more informative failure.
    #[serde(default)]
    per_kinematics: Vec<KinematicsAggregate>,
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

fn golden_path(stem: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(format!("{stem}.json"))
}

// ── The fixtures ────────────────────────────────────────────────────────

/// Three 2.5D operations over one rectangular region, on one 6 mm end mill.
///
/// Nothing here is loaded from disk and nothing is random: the polygon is a
/// literal, the tool is built field by field, and every operation dial is
/// spelled out. A default that moves must not silently move the golden.
fn fixture_session_2d() -> ProjectSession {
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
        session
            .add_toolpath(0, toolpath_config(name, op, tool_id, model_id))
            .expect("add toolpath");
    }

    session
}

/// The 19-field `ToolpathConfig` literal, once, shared by both arms.
fn toolpath_config(
    name: &str,
    op: OperationConfig,
    tool_id: usize,
    model_id: usize,
) -> ToolpathConfig {
    let op_type = op.op_type();
    ToolpathConfig {
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
    }
}

/// Radius of the synthetic hemisphere the 3D arm finishes.
const HEMI_RADIUS_MM: f64 = 10.0;
/// Half-extent of the stock around it (2 mm of flat margin per side).
const HEMI_STOCK_HALF_MM: f64 = HEMI_RADIUS_MM + 2.0;

/// Two 3D finishing operations over a synthetic hemisphere, on one 6 mm
/// ball-nose cutter.
///
/// The point of this arm is the *kinematics*, not the shape:
///
/// * **drop cutter** rakes the dome in XY while Z tracks the surface, so
///   almost every cutting move changes X/Y **and** Z —
///   `classify_cut_kinematics` calls that `Helix`, and it is the class the
///   2.5D arm never produces.
/// * **waterline** cuts constant-Z contours around a dome, i.e. near-perfect
///   circles, and `arc_fitting` is turned on explicitly so the emitter
///   actually issues `MoveType::ArcCW`/`ArcCCW` — the branch that runs the
///   arc lineariser and stamps through `CutKinematics::Arc`.
///
/// Surface operations carry no depth dial, so waterline's Z band has to be
/// pinned by hand (`HeightMode::Manual`); left on `Auto` the band collapses
/// to zero height and the op emits nothing.
fn fixture_session_3d() -> ProjectSession {
    let mut session = ProjectSession::new_empty();

    // The dome's base sits on z = 0 and its apex at z = HEMI_RADIUS_MM, so
    // the stock is exactly as tall as the dome and its top plane touches the
    // apex. Nothing is auto-derived: `auto_from_model` off.
    session.set_stock_config(StockConfig {
        x: 2.0 * HEMI_STOCK_HALF_MM,
        y: 2.0 * HEMI_STOCK_HALF_MM,
        z: HEMI_RADIUS_MM,
        origin_x: -HEMI_STOCK_HALF_MM,
        origin_y: -HEMI_STOCK_HALF_MM,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    });

    let mut tool = ToolConfig::new_default(ToolId(0), ToolType::BallNose);
    tool.diameter = 6.0;
    tool.cutting_length = 25.0;
    tool.shank_diameter = 6.35;
    tool.shank_length = 20.0;
    tool.stickout = 45.0;
    tool.flute_count = 2;
    tool.name = "Ball Nose 6mm".to_owned();
    let tool_idx = session.add_tool(tool);
    let tool_id = session.tools()[tool_idx].id.0;

    // 8 divisions → 32 vertices per ring, 8 rings: 992 triangles. Small
    // enough to simulate twice per test run, curved enough that the
    // waterline contours fit arcs.
    let model_id = session.add_model(LoadedModel {
        id: 0,
        name: "perf_hemisphere".to_owned(),
        mesh: Some(Arc::new(rs_cam_core::mesh::make_test_hemisphere(
            HEMI_RADIUS_MM,
            8,
        ))),
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://perf_hemisphere.stl"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    });

    let drop_cutter = OperationConfig::DropCutter(DropCutterConfig {
        stepover: 1.5,
        feed_rate: 1000.0,
        plunge_rate: 400.0,
        min_z: 0.0,
        slope_from: 0.0,
        slope_to: 90.0,
        spindle_rpm: Some(18_000),
        scallop_height: None,
    });
    let waterline = OperationConfig::Waterline(WaterlineConfig {
        z_step: 1.5,
        sampling: 0.4,
        feed_rate: 1000.0,
        plunge_rate: 400.0,
        continuous: false,
        spindle_rpm: Some(18_000),
    });

    let mut dc = toolpath_config("DropCutter", drop_cutter, tool_id, model_id);
    dc.dressups.arc_fitting = true;
    session.add_toolpath(0, dc).expect("add drop cutter");

    let mut wl = toolpath_config("Waterline", waterline, tool_id, model_id);
    wl.dressups.arc_fitting = true;
    wl.heights = HeightsConfig {
        top_z: HeightMode::Manual(HEMI_RADIUS_MM),
        bottom_z: HeightMode::Manual(0.0),
        ..HeightsConfig::default()
    };
    session.add_toolpath(0, wl).expect("add waterline");

    session
}

/// Every simulation dial spelled out. `adaptive_feed_modulation` defaults to
/// `true` and would put a feed-modulation pass between the toolpath and the
/// numbers; the golden measures the sim, so it is off.
fn sim_options(resolution: f64) -> SimulationOptions {
    SimulationOptions {
        resolution,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    }
}

fn measure(mut session: ProjectSession, resolution: f64) -> ProjectAggregate {
    let cancel = AtomicBool::new(false);
    for i in 0..session.toolpath_count() {
        session
            .generate_toolpath(i, &cancel)
            .expect("generation must succeed");
    }
    session
        .run_simulation(&sim_options(resolution), &cancel)
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
                per_kinematics: summary
                    .map(|s| kinematics_aggregates(&s.per_kinematics))
                    .unwrap_or_default(),
            }
        })
        .collect();

    ProjectAggregate {
        resolution_mm: resolution,
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
        failures.push(format!(
            "{what}: NaN mismatch — got {actual}, golden {expected}"
        ));
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
        failures.push(format!(
            "{what}: got {actual:?}, golden {expected:?} (exact)"
        ));
    }
}

fn compare(actual: &ProjectAggregate, golden: &ProjectAggregate) -> Vec<String> {
    let mut f = Vec::new();

    exact(
        actual.resolution_mm,
        golden.resolution_mm,
        "resolution_mm",
        &mut f,
    );
    exact(
        actual.toolpath_count,
        golden.toolpath_count,
        "toolpath_count",
        &mut f,
    );
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
        exact(
            a.name.as_str(),
            g.name.as_str(),
            &format!("{tag}.name"),
            &mut f,
        );
        exact(
            a.op_kind.as_str(),
            g.op_kind.as_str(),
            &format!("{tag}.op_kind"),
            &mut f,
        );
        exact(
            a.sample_count,
            g.sample_count,
            &format!("{tag}.sample_count"),
            &mut f,
        );
        exact(
            a.move_count,
            g.move_count,
            &format!("{tag}.move_count"),
            &mut f,
        );
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
            (
                a.cutting_distance_mm,
                g.cutting_distance_mm,
                "cutting_distance_mm",
            ),
            (
                a.rapid_distance_mm,
                g.rapid_distance_mm,
                "rapid_distance_mm",
            ),
            (a.total_runtime_s, g.total_runtime_s, "total_runtime_s"),
            (
                a.cutting_runtime_s,
                g.cutting_runtime_s,
                "cutting_runtime_s",
            ),
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
            (
                a.average_engagement,
                g.average_engagement,
                "average_engagement",
            ),
            (
                a.average_mrr_mm3_s,
                g.average_mrr_mm3_s,
                "average_mrr_mm3_s",
            ),
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
            (
                a.peak_axial_doc_mm,
                g.peak_axial_doc_mm,
                "peak_axial_doc_mm",
            ),
            (
                a.peak_plunge_descent_mm,
                g.peak_plunge_descent_mm,
                "peak_plunge_descent_mm",
            ),
        ] {
            close(av, gv, TIGHT_REL, &format!("{tag}.{name}"), &mut f);
        }

        compare_kinematics(&a.per_kinematics, &g.per_kinematics, &tag, &mut f);
    }

    f
}

/// `None` on a `KinematicsSummary` field means **not measured** (the class
/// carried no sample reporting that quantity), which is a different statement
/// from "measured zero". So the None-ness is compared EXACTLY and only the
/// contained value gets a tolerance.
fn close_opt(
    actual: Option<f64>,
    expected: Option<f64>,
    rel: f64,
    what: &str,
    failures: &mut Vec<String>,
) {
    match (actual, expected) {
        (None, None) => {}
        (Some(a), Some(g)) => close(a, g, rel, what, failures),
        (a, g) => failures.push(format!(
            "{what}: measured-ness changed — got {a:?}, golden {g:?} \
             (None means NOT MEASURED, not zero)"
        )),
    }
}

fn compare_kinematics(
    actual: &[KinematicsAggregate],
    golden: &[KinematicsAggregate],
    tag: &str,
    f: &mut Vec<String>,
) {
    if actual.len() != golden.len() {
        let a_kinds: Vec<&str> = actual.iter().map(|k| k.kind.as_str()).collect();
        let g_kinds: Vec<&str> = golden.iter().map(|k| k.kind.as_str()).collect();
        f.push(format!(
            "{tag}.per_kinematics: got {a_kinds:?}, golden {g_kinds:?} \
             (a class appearing or disappearing is a real change)"
        ));
        return;
    }
    for (a, g) in actual.iter().zip(golden.iter()) {
        let kt = format!("{tag}.per_kinematics[{}]", g.kind);
        exact(a.kind.as_str(), g.kind.as_str(), &format!("{kt}.kind"), f);
        exact(
            a.sample_count,
            g.sample_count,
            &format!("{kt}.sample_count"),
            f,
        );
        for (av, gv, name) in [
            (
                a.cutting_runtime_s,
                g.cutting_runtime_s,
                "cutting_runtime_s",
            ),
            (
                a.average_radial_woc_fraction,
                g.average_radial_woc_fraction,
                "average_radial_woc_fraction",
            ),
            (
                a.average_leading_edge_speed_mm_min,
                g.average_leading_edge_speed_mm_min,
                "average_leading_edge_speed_mm_min",
            ),
        ] {
            close(av, gv, LOOSE_REL, &format!("{kt}.{name}"), f);
        }
        close_opt(
            a.average_arc_radians,
            g.average_arc_radians,
            LOOSE_REL,
            &format!("{kt}.average_arc_radians"),
            f,
        );
        for (av, gv, name) in [
            (
                a.peak_radial_woc_fraction,
                g.peak_radial_woc_fraction,
                "peak_radial_woc_fraction",
            ),
            (
                a.peak_axial_doc_mm,
                g.peak_axial_doc_mm,
                "peak_axial_doc_mm",
            ),
            (
                a.peak_plunge_descent_mm,
                g.peak_plunge_descent_mm,
                "peak_plunge_descent_mm",
            ),
        ] {
            close(av, gv, TIGHT_REL, &format!("{kt}.{name}"), f);
        }
        close_opt(
            a.peak_chip_thickness_mm,
            g.peak_chip_thickness_mm,
            TIGHT_REL,
            &format!("{kt}.peak_chip_thickness_mm"),
            f,
        );
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

fn check_against_golden(actual: &ProjectAggregate, stem: &str) {
    let path = golden_path(stem);

    if std::env::var("UPDATE_PERF_GOLDENS").is_ok_and(|v| v != "0" && !v.is_empty()) {
        let json = serde_json::to_string_pretty(actual).expect("serialize aggregate");
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

    let failures = compare(actual, &golden);
    assert!(
        failures.is_empty(),
        "simulation metrics moved against the Phase 0 golden `{stem}` \
         ({} field(s)).\n{}\n\n\
         If this is a DELIBERATE re-baseline (e.g. PERF_REVIEW S1 swept-volume \
         stamping), regenerate with UPDATE_PERF_GOLDENS=1 and say so in the \
         commit message.",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn sim_metrics_match_golden() {
    check_against_golden(
        &measure(fixture_session_2d(), SIM_RESOLUTION_MM),
        "perf_golden_sim_metrics",
    );
}

#[test]
fn sim_metrics_3d_match_golden() {
    check_against_golden(
        &measure(fixture_session_3d(), SIM_RESOLUTION_3D_MM),
        "perf_golden_sim_metrics_3d",
    );
}

/// The golden is only worth anything if the fixture actually cuts. A
/// simulation that removed nothing would pin a page of zeros and pass
/// forever — the vacuity failure `gate_population_vacuity_xvac` exists for,
/// applied to a golden instead of a gate.
fn assert_not_vacuous(a: &ProjectAggregate, toolpaths: usize, min_samples: usize, min_mm3: f64) {
    assert_eq!(
        a.per_toolpath.len(),
        toolpaths,
        "fixture must carry {toolpaths} toolpaths"
    );
    assert!(
        a.total_sample_count > min_samples,
        "fixture produced only {} cut samples — too few to detect a metric change",
        a.total_sample_count
    );
    assert!(
        a.project_total_removed_volume_est_mm3 > min_mm3,
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

#[test]
fn golden_fixture_is_not_vacuous() {
    assert_not_vacuous(
        &measure(fixture_session_2d(), SIM_RESOLUTION_MM),
        3,
        500,
        100.0,
    );
}

#[test]
fn golden_3d_fixture_is_not_vacuous() {
    assert_not_vacuous(
        &measure(fixture_session_3d(), SIM_RESOLUTION_3D_MM),
        2,
        500,
        50.0,
    );
}

/// The reason the 3D arm exists, asserted rather than assumed.
///
/// A golden that pinned only `Linear` samples would sit there green while a
/// bug in the arc lineariser or in the XY+Z stamp path shipped. This test
/// fails the day the fixture stops producing arcs or helical cuts — which is
/// the day the golden above stops covering S1/S2/S3's arc and helix
/// branches, whether or not any pinned number moved.
#[test]
fn three_d_arm_covers_arc_and_helix_kinematics() {
    let a = measure(fixture_session_3d(), SIM_RESOLUTION_3D_MM);

    let mut totals: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for tp in &a.per_toolpath {
        for k in &tp.per_kinematics {
            *totals.entry(k.kind.as_str()).or_default() += k.sample_count;
        }
    }
    eprintln!("3D arm per-kinematics sample counts: {totals:?}");

    for want in ["Arc", "Helix"] {
        let n = totals.get(want).copied().unwrap_or(0);
        assert!(
            n > 0,
            "3D golden arm produced no `{want}` cutting samples ({totals:?}). \
             The arm exists precisely to cover that stamp branch; a golden \
             without it pins only straight-and-level motion."
        );
    }
}
