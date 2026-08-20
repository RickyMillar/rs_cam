//! W5B-F4 — air-cut threshold recalibration evidence harness.
//!
//! `planning/perf_review_2026-08-19/DELTA_w5b_f4_aircut_DECISION.md`.
//!
//! # Why this exists
//!
//! Every shipped air-cut bar — `OperationType::air_cut_high_threshold_pct`,
//! the GUI's 20 % banner, the CLI's 40 % verdict — was fitted against the
//! per-stamp kernel's reading, and that reading was largely a **resolution
//! artifact**: on one unchanged toolpath it moved 0.31 % → 89.61 % across
//! cell sizes 0.25 → 1.0 mm while the swept kernel read a stable
//! 5.95 → 11.04 (`DELTA_sim_w5_s1_DECISION.md` §2, class (ii-b)). Since
//! `a4ff2a8c` the swept kernel is the default, so the bands are calibrated
//! for a retired instrument.
//!
//! The goldens and the wanaka 0D reference cover eight op kinds between
//! them. This harness covers the **rest of the families that have no
//! post-flip reading at all** — every 2D contour op besides `Profile`, every
//! 3D finish op besides `DropCutter` / `Waterline` / `Pencil`, and the
//! 2.5D clearing ops besides `Pocket` / `Zigzag` / `Adaptive3d`.
//!
//! # What it measures, and what it deliberately does not
//!
//! Each row is **one operation alone in its own session against fresh
//! stock**. That is an isolation choice, not a realism choice: it makes the
//! number a property of that operation's own emitted motion against a known
//! stock, rather than of whatever the previous op in a stack happened to
//! leave. Two consequences the reader must not misread:
//!
//! * A finish op measured here has NOT been preceded by a rough, so it is
//!   cutting the full stock envelope where a real stack would have it
//!   skimming. `[Waterline]` therefore does **not** reproduce the 3D
//!   golden's 54.26 %, which is measured second in a two-op session.
//! * `[DropCutter]` and `[Pocket]` are first in their goldens, so they DO
//!   reproduce, and that is the harness's own validity check.
//!
//! Each op is measured at **two cell sizes**. The point is not the absolute
//! value at either one: it is whether the swept instrument's stability
//! claim — air-cut % is a property of the toolpath, not of the grid — holds
//! per family. The per-stamp kernel's spread on that same test was 289× the
//! swept kernel's.
//!
//! No threshold is read or asserted here. This binary produces evidence;
//! the decision lives in the DELTA doc and belongs to the user.
//!
//! ```text
//! cargo test -p rs_cam_core --release --test air_cut_family_calibration_w5bf4 -- --nocapture
//! ```

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::print_stderr
)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{
    BoundaryConfig, DressupConfig, HeightMode, HeightsConfig, RestAnalysisConfig, StockSource,
};
use rs_cam_core::compute::operation_configs::{
    AdaptiveConfig, ChamferConfig, DropCutterConfig, FaceConfig, HorizontalFinishConfig,
    InlayConfig, PencilConfig, PocketConfig, PocketPattern, ProfileConfig, ProjectCurveConfig,
    RadialFinishConfig, RampFinishConfig, ScallopConfig, SpiralFinishConfig, SteepShallowConfig,
    TraceConfig, UnifiedFinishConfig, VCarveConfig, WaterlineConfig, ZigzagConfig,
};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::profile::ProfileSide;
use rs_cam_core::session::{LoadedModel, ProjectSession, SimulationOptions, ToolpathConfig};
use rs_cam_core::simulation_cut::AirCutRatios;

// ── Fixture geometry ────────────────────────────────────────────────────

/// Flat stock for the 2D / 2.5D arms. 2D ops cut at negative Z
/// (`project_2d_stock_z_frame`), so `origin_z` is negative and the stock
/// top lands on world z = 0.
const FLAT_X: f64 = 100.0;
const FLAT_Y: f64 = 80.0;
const FLAT_Z: f64 = 12.0;

/// Radius of the synthetic hemisphere the 3D arms finish, and the stock
/// half-extent around it (2 mm of flat margin per side). Same shape the
/// perf golden's 3D arm uses, so `[DropCutter]` cross-checks against it.
const HEMI_RADIUS_MM: f64 = 10.0;
const HEMI_STOCK_HALF_MM: f64 = HEMI_RADIUS_MM + 2.0;

/// Cell sizes for the flat arms. 1.0 is the 2.5D golden's own resolution.
const FLAT_CELLS_MM: [f64; 2] = [0.5, 1.0];
/// Cell sizes for the hemisphere arms. A ball tip against a curved surface
/// needs a cell well under the tip radius before radial engagement is
/// measurable at all (`PERP_COVERAGE_GATE`); 0.5 is the 3D golden's.
const HEMI_CELLS_MM: [f64; 2] = [0.25, 0.5];

// ── One measured row ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Row {
    family: &'static str,
    op: OperationType,
    cell_mm: f64,
    /// `air_cut_pct_of_total_runtime` — the denominator every shipped bar
    /// is stated against (`MEASUREMENT_DOMAINS.md` LH-1). NOT the
    /// cutting-time reading the MCP narration prints.
    air_pct_of_total: f64,
    air_pct_of_cutting: f64,
    average_engagement: f64,
    removed_mm3: f64,
    sample_count: usize,
    move_count: usize,
    metrics_not_applicable: bool,
    /// The op-kind band this row would be judged against, read (never
    /// written) from `OperationType::air_cut_high_threshold_pct`.
    threshold_pct: Option<f64>,
    /// Whether the air-cut metric ABSTAINS for this row. A gate handed a
    /// `NotMeasurable` metric does not fire, so a row that abstains needs no
    /// recalibration however extreme its percentage looks
    /// (`sim_measurability`, Checkpoint D Q2).
    abstains: bool,
    /// Whether the shipped band actually fires on this row: measurable,
    /// thresholded, and over. This column, not the percentage, is the one
    /// the decision's "verdict flips" list is built from.
    fires: bool,
}

fn sim_options(resolution: f64) -> SimulationOptions {
    SimulationOptions {
        resolution,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        // A feed-modulation pass between the toolpath and the numbers
        // would make this a measurement of two things.
        adaptive_feed_modulation: false,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    }
}

fn toolpath_config(
    name: &str,
    op: OperationConfig,
    tool_id: usize,
    model_id: usize,
    heights: HeightsConfig,
) -> ToolpathConfig {
    let op_type = op.op_type();
    ToolpathConfig {
        id: ToolpathId(0),
        name: name.to_owned(),
        enabled: true,
        operation: op,
        dressups: DressupConfig::for_op(op_type),
        heights,
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

/// Generate + simulate a single-toolpath session and read its row.
///
/// Returns `Err` with the generator's own message when an operation cannot
/// be built on this fixture. A family that cannot be measured is reported
/// as such rather than dropped — an absent row and a zero row are not the
/// same statement.
fn measure(family: &'static str, mut session: ProjectSession, cell_mm: f64) -> Result<Row, String> {
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .map_err(|e| format!("generate: {e}"))?;
    session
        .run_simulation(&sim_options(cell_mm), &cancel)
        .map_err(|e| format!("simulate: {e}"))?;

    let op = session.toolpath_configs()[0].operation.op_type();
    let move_count = session
        .get_result(0)
        .map_or(0, |r| r.toolpath().moves.len());
    let sim = session
        .simulation_result()
        .ok_or_else(|| "no simulation result".to_owned())?;
    let trace = sim
        .cut_trace
        .as_ref()
        .ok_or_else(|| "metrics_enabled but no cut trace".to_owned())?;
    let summary = trace
        .toolpath_summaries
        .first()
        .ok_or_else(|| "no toolpath summary — the op emitted no simulated motion".to_owned())?;

    // Same construction the shipped gate uses
    // (`session::compute::air_cut_offenders_for_toolpaths`): abstain first,
    // compare second. Reading the band here does not move it.
    let measurability =
        rs_cam_core::sim_measurability::MeasurabilityReport::from_trace(trace, Some(cell_mm));
    let abstains = measurability
        .for_metric(
            summary.toolpath_id,
            rs_cam_core::sim_measurability::SimMetric::AirCut,
        )
        .abstains();
    let threshold_pct = op.air_cut_high_threshold_pct();
    let air_pct_of_total = summary.air_cut_pct_of_total_runtime();
    let fires = !abstains && threshold_pct.is_some_and(|t| air_pct_of_total > t);

    Ok(Row {
        family,
        op,
        cell_mm,
        air_pct_of_total,
        air_pct_of_cutting: summary.air_cut_pct_of_cutting_time(),
        average_engagement: summary.average_engagement,
        removed_mm3: summary.total_removed_volume_est_mm3,
        sample_count: summary.sample_count,
        move_count,
        metrics_not_applicable: summary.metrics_not_applicable,
        threshold_pct,
        abstains,
        fires,
    })
}

// ── Session builders ────────────────────────────────────────────────────

fn flat_stock(session: &mut ProjectSession) {
    session.set_stock_config(StockConfig {
        x: FLAT_X,
        y: FLAT_Y,
        z: FLAT_Z,
        origin_x: -10.0,
        origin_y: -10.0,
        origin_z: -FLAT_Z,
        auto_from_model: false,
        ..StockConfig::default()
    });
}

fn add_tool(session: &mut ProjectSession, kind: ToolType, diameter: f64, name: &str) -> usize {
    let mut tool = ToolConfig::new_default(ToolId(0), kind);
    tool.diameter = diameter;
    tool.cutting_length = 25.0;
    tool.shank_diameter = 6.35;
    tool.shank_length = 20.0;
    tool.stickout = 45.0;
    tool.flute_count = 2;
    tool.name = name.to_owned();
    let idx = session.add_tool(tool);
    session.tools()[idx].id.0
}

fn add_rect_polygon(session: &mut ProjectSession) -> usize {
    let poly = Polygon2::new(vec![
        P2::new(5.0, 5.0),
        P2::new(75.0, 5.0),
        P2::new(75.0, 55.0),
        P2::new(5.0, 55.0),
    ]);
    session.add_model(LoadedModel {
        id: 0,
        name: "w5bf4_rect".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![poly])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://w5bf4_rect.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    })
}

/// A flat-stock session carrying exactly one 2D / 2.5D operation.
fn flat_session(op: OperationConfig, tool_kind: ToolType, tool_diameter: f64) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    flat_stock(&mut session);
    let tool_id = add_tool(&mut session, tool_kind, tool_diameter, "w5bf4 tool");
    let model_id = add_rect_polygon(&mut session);
    let name = format!("{:?}", op.op_type());
    session
        .add_toolpath(
            0,
            toolpath_config(&name, op, tool_id, model_id, HeightsConfig::default()),
        )
        .expect("add toolpath");
    session
}

fn hemi_stock(session: &mut ProjectSession) {
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
}

fn add_hemisphere(session: &mut ProjectSession) -> usize {
    session.add_model(LoadedModel {
        id: 0,
        name: "w5bf4_hemisphere".to_owned(),
        mesh: Some(Arc::new(rs_cam_core::mesh::make_test_hemisphere(
            HEMI_RADIUS_MM,
            8,
        ))),
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://w5bf4_hemisphere.stl"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    })
}

/// A hemisphere session carrying exactly one surface operation.
///
/// Surface ops carry no depth dial, so the Z band is pinned by hand — left
/// on `Auto` a waterline band collapses to zero height and the op emits
/// nothing (the perf golden's 3D arm documents the same trap).
fn hemi_session(op: OperationConfig, tool_kind: ToolType, tool_diameter: f64) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    hemi_stock(&mut session);
    let tool_id = add_tool(&mut session, tool_kind, tool_diameter, "w5bf4 ball");
    let model_id = add_hemisphere(&mut session);
    let name = format!("{:?}", op.op_type());
    let heights = HeightsConfig {
        top_z: HeightMode::Manual(HEMI_RADIUS_MM),
        bottom_z: HeightMode::Manual(0.0),
        ..HeightsConfig::default()
    };
    let mut tp = toolpath_config(&name, op, tool_id, model_id, heights);
    tp.dressups.arc_fitting = true;
    session.add_toolpath(0, tp).expect("add toolpath");
    session
}

/// Side length of the flat plate the `HorizontalFinish` arm skims, and the
/// stock depth above it.
const PLATE_MM: f64 = 24.0;
const PLATE_STOCK_Z_MM: f64 = 3.0;

/// A flat-plate session — the only fixture `HorizontalFinish` can be
/// measured on.
///
/// `HorizontalFinish` cuts near-flat areas only (`CLAUDE.md`: "useless on
/// terrain"). On the hemisphere it emits **nothing at all**, so measuring
/// it there would have produced an absent row, not a low one. The plate
/// sits at z = 0 with 3 mm of stock above it, so the op has real material
/// to take and the reading is a reading.
fn plate_session(op: OperationConfig) -> ProjectSession {
    let mut session = ProjectSession::new_empty();
    session.set_stock_config(StockConfig {
        x: PLATE_MM,
        y: PLATE_MM,
        z: PLATE_STOCK_Z_MM,
        origin_x: -PLATE_MM / 2.0,
        origin_y: -PLATE_MM / 2.0,
        origin_z: 0.0,
        auto_from_model: false,
        ..StockConfig::default()
    });
    let tool_id = add_tool(&mut session, ToolType::BallNose, 6.0, "w5bf4 ball");
    let model_id = session.add_model(LoadedModel {
        id: 0,
        name: "w5bf4_plate".to_owned(),
        mesh: Some(Arc::new(rs_cam_core::mesh::make_test_flat(PLATE_MM))),
        polygons: None,
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://w5bf4_plate.stl"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    });
    let heights = HeightsConfig {
        top_z: HeightMode::Manual(PLATE_STOCK_Z_MM),
        bottom_z: HeightMode::Manual(0.0),
        ..HeightsConfig::default()
    };
    let name = format!("{:?}", op.op_type());
    session
        .add_toolpath(0, toolpath_config(&name, op, tool_id, model_id, heights))
        .expect("add toolpath");
    session
}

/// A hemisphere session whose operation drives a **curve** model projected
/// onto the mesh — the `ProjectCurve` arm, and the only one that needs two
/// models in one session.
fn project_curve_session(cell_note: &str) -> ProjectSession {
    let _ = cell_note;
    let mut session = ProjectSession::new_empty();
    hemi_stock(&mut session);
    let tool_id = add_tool(&mut session, ToolType::BallNose, 3.0, "w5bf4 ball 3mm");
    let surface_model_id = add_hemisphere(&mut session);

    // A single open-ish ring inside the dome footprint: a "river" that
    // covers a small fraction of the stock, which is the geometry
    // `ProjectCurve`'s 97 % band was written for.
    let curve = Polygon2::new(vec![
        P2::new(-6.0, -3.0),
        P2::new(-2.0, 3.0),
        P2::new(2.0, -3.0),
        P2::new(6.0, 3.0),
    ]);
    let curve_model_id = session.add_model(LoadedModel {
        id: 1,
        name: "w5bf4_river".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![curve])),
        drill_targets: Arc::new(Vec::new()),
        layers: Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://w5bf4_river.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    });

    let op = OperationConfig::ProjectCurve(ProjectCurveConfig {
        depth: 1.0,
        point_spacing: 0.4,
        feed_rate: 800.0,
        plunge_rate: 400.0,
        surface_model_id: Some(rs_cam_core::compute::stock_config::ModelId(
            surface_model_id,
        )),
        spindle_rpm: Some(18_000),
        ..ProjectCurveConfig::default()
    });
    session
        .add_toolpath(
            0,
            toolpath_config(
                "ProjectCurve",
                op,
                tool_id,
                curve_model_id,
                HeightsConfig::default(),
            ),
        )
        .expect("add toolpath");
    session
}

// ── The measured population ─────────────────────────────────────────────

type FlatCase = (&'static str, fn() -> OperationConfig, ToolType, f64);

/// Every flat-stock case: family label, config builder, tool.
fn flat_cases() -> Vec<FlatCase> {
    vec![
        // 2.5D clearing / rough — band 40.
        (
            "2.5D clearing",
            || {
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
                })
            },
            ToolType::EndMill,
            6.0,
        ),
        (
            "2.5D clearing",
            || {
                OperationConfig::Zigzag(ZigzagConfig {
                    stepover: 3.0,
                    depth: 3.0,
                    depth_per_pass: 3.0,
                    feed_rate: 1000.0,
                    plunge_rate: 400.0,
                    angle: 45.0,
                    spindle_rpm: Some(18_000),
                })
            },
            ToolType::EndMill,
            6.0,
        ),
        (
            "2.5D clearing",
            || {
                OperationConfig::Face(FaceConfig {
                    stepover: 3.0,
                    depth: 2.0,
                    depth_per_pass: 2.0,
                    feed_rate: 1500.0,
                    plunge_rate: 500.0,
                    spindle_rpm: Some(18_000),
                    ..FaceConfig::default()
                })
            },
            ToolType::EndMill,
            6.0,
        ),
        (
            "2.5D clearing",
            || {
                OperationConfig::Adaptive(AdaptiveConfig {
                    stepover: 2.0,
                    depth: 6.0,
                    depth_per_pass: 3.0,
                    feed_rate: 1500.0,
                    plunge_rate: 500.0,
                    spindle_rpm: Some(18_000),
                    ..AdaptiveConfig::default()
                })
            },
            ToolType::EndMill,
            6.0,
        ),
        // 2D contour — band 40.
        (
            "2D contour",
            || {
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
                    compensation:
                        rs_cam_core::compute::operation_configs::CompensationType::InComputer,
                    spindle_rpm: Some(18_000),
                })
            },
            ToolType::EndMill,
            6.0,
        ),
        (
            "2D contour",
            || {
                OperationConfig::Trace(TraceConfig {
                    depth: 3.0,
                    depth_per_pass: 1.5,
                    feed_rate: 800.0,
                    plunge_rate: 400.0,
                    spindle_rpm: Some(18_000),
                    ..TraceConfig::default()
                })
            },
            ToolType::EndMill,
            6.0,
        ),
        (
            "2D contour",
            || {
                OperationConfig::VCarve(VCarveConfig {
                    max_depth: 4.0,
                    stepover: 0.5,
                    feed_rate: 800.0,
                    plunge_rate: 400.0,
                    spindle_rpm: Some(18_000),
                    ..VCarveConfig::default()
                })
            },
            ToolType::VBit,
            12.7,
        ),
        (
            "2D contour",
            || {
                OperationConfig::Chamfer(ChamferConfig {
                    chamfer_width: 1.5,
                    tip_offset: 0.1,
                    feed_rate: 800.0,
                    plunge_rate: 400.0,
                    spindle_rpm: Some(18_000),
                })
            },
            ToolType::VBit,
            12.7,
        ),
        (
            "2D contour",
            || {
                OperationConfig::Inlay(InlayConfig {
                    pocket_depth: 3.0,
                    feed_rate: 800.0,
                    plunge_rate: 400.0,
                    spindle_rpm: Some(18_000),
                    ..InlayConfig::default()
                })
            },
            ToolType::VBit,
            12.7,
        ),
    ]
}

type HemiCase = (&'static str, fn() -> OperationConfig, ToolType, f64);

/// Every hemisphere case. Ball nose throughout: `Scallop` and
/// `UnifiedFinish` require a ball tip, and using one tool across the arm
/// keeps the rows comparable.
fn hemi_cases() -> Vec<HemiCase> {
    vec![
        (
            "3D finish",
            || {
                OperationConfig::DropCutter(DropCutterConfig {
                    stepover: 1.5,
                    feed_rate: 1000.0,
                    plunge_rate: 400.0,
                    min_z: 0.0,
                    slope_from: 0.0,
                    slope_to: 90.0,
                    spindle_rpm: Some(18_000),
                    scallop_height: None,
                })
            },
            ToolType::BallNose,
            6.0,
        ),
        (
            "3D finish",
            || {
                OperationConfig::Waterline(WaterlineConfig {
                    z_step: 1.5,
                    sampling: 0.4,
                    feed_rate: 1000.0,
                    plunge_rate: 400.0,
                    continuous: false,
                    spindle_rpm: Some(18_000),
                })
            },
            ToolType::BallNose,
            6.0,
        ),
        (
            "3D finish",
            || {
                OperationConfig::Scallop(ScallopConfig {
                    scallop_height: 0.1,
                    feed_rate: 1000.0,
                    plunge_rate: 500.0,
                    spindle_rpm: Some(18_000),
                    ..ScallopConfig::default()
                })
            },
            ToolType::BallNose,
            6.0,
        ),
        (
            "3D finish",
            || {
                OperationConfig::UnifiedFinish(UnifiedFinishConfig {
                    feed_rate: 1000.0,
                    plunge_rate: 500.0,
                    spindle_rpm: Some(18_000),
                    ..UnifiedFinishConfig::default()
                })
            },
            ToolType::BallNose,
            6.0,
        ),
        (
            "3D finish",
            || {
                OperationConfig::SteepShallow(SteepShallowConfig {
                    feed_rate: 1000.0,
                    plunge_rate: 500.0,
                    spindle_rpm: Some(18_000),
                    ..SteepShallowConfig::default()
                })
            },
            ToolType::BallNose,
            6.0,
        ),
        (
            "3D finish",
            || {
                OperationConfig::RampFinish(RampFinishConfig {
                    feed_rate: 1000.0,
                    plunge_rate: 500.0,
                    spindle_rpm: Some(18_000),
                    ..RampFinishConfig::default()
                })
            },
            ToolType::BallNose,
            6.0,
        ),
        (
            "3D finish",
            || {
                OperationConfig::SpiralFinish(SpiralFinishConfig {
                    feed_rate: 1000.0,
                    plunge_rate: 500.0,
                    spindle_rpm: Some(18_000),
                    ..SpiralFinishConfig::default()
                })
            },
            ToolType::BallNose,
            6.0,
        ),
        (
            "3D finish",
            || {
                OperationConfig::RadialFinish(RadialFinishConfig {
                    feed_rate: 1000.0,
                    plunge_rate: 500.0,
                    spindle_rpm: Some(18_000),
                    ..RadialFinishConfig::default()
                })
            },
            ToolType::BallNose,
            6.0,
        ),
        (
            "3D finish",
            || {
                OperationConfig::Pencil(PencilConfig {
                    feed_rate: 1000.0,
                    plunge_rate: 500.0,
                    spindle_rpm: Some(18_000),
                    ..PencilConfig::default()
                })
            },
            ToolType::BallNose,
            3.0,
        ),
    ]
}

fn print_row(r: &Row) {
    println!(
        "W5BF4 | {:<14} | {:<17} | cell {:.2} | air_total {:>7.3} % | \
         air_cutting {:>7.3} % | avg_eng {:>6.4} | removed {:>10.2} mm3 | \
         samples {:>8} | moves {:>7} | n/a {:<5} | bar {:>6} | abstains {:<5} | fires {}",
        r.family,
        format!("{:?}", r.op),
        r.cell_mm,
        r.air_pct_of_total,
        r.air_pct_of_cutting,
        r.average_engagement,
        r.removed_mm3,
        r.sample_count,
        r.move_count,
        r.metrics_not_applicable,
        r.threshold_pct
            .map_or_else(|| "none".to_owned(), |t| format!("{t:.0}")),
        r.abstains,
        r.fires,
    );
}

/// Collect every row this harness can produce, printing failures rather
/// than swallowing them.
fn collect_rows() -> (Vec<Row>, Vec<String>) {
    let mut rows = Vec::new();
    let mut skipped = Vec::new();

    for cell in FLAT_CELLS_MM {
        for (family, build, kind, dia) in flat_cases() {
            let op = build();
            let label = format!("{:?}", op.op_type());
            match measure(family, flat_session(op, kind, dia), cell) {
                Ok(r) => {
                    print_row(&r);
                    rows.push(r);
                }
                Err(e) => skipped.push(format!("{label} @ cell {cell:.2}: {e}")),
            }
        }
    }

    for cell in HEMI_CELLS_MM {
        for (family, build, kind, dia) in hemi_cases() {
            let op = build();
            let label = format!("{:?}", op.op_type());
            match measure(family, hemi_session(op, kind, dia), cell) {
                Ok(r) => {
                    print_row(&r);
                    rows.push(r);
                }
                Err(e) => skipped.push(format!("{label} @ cell {cell:.2}: {e}")),
            }
        }
        match measure("sparse curve", project_curve_session("river"), cell) {
            Ok(r) => {
                print_row(&r);
                rows.push(r);
            }
            Err(e) => skipped.push(format!("ProjectCurve @ cell {cell:.2}: {e}")),
        }
        let horizontal = OperationConfig::HorizontalFinish(HorizontalFinishConfig {
            stepover: 1.0,
            feed_rate: 1000.0,
            plunge_rate: 500.0,
            spindle_rpm: Some(18_000),
            ..HorizontalFinishConfig::default()
        });
        match measure("3D finish", plate_session(horizontal), cell) {
            Ok(r) => {
                print_row(&r);
                rows.push(r);
            }
            Err(e) => skipped.push(format!("HorizontalFinish @ cell {cell:.2}: {e}")),
        }
    }

    (rows, skipped)
}

// ── The tests ───────────────────────────────────────────────────────────

/// Measure every family at two cell sizes and print the table.
///
/// The assertions are deliberately weak — this is an instrument, not a
/// gate. What they do guarantee is that the instrument cannot decay into a
/// vacuous one: a harness that measures nothing and passes is the failure
/// mode `CLAUDE.md` names ("a gate handed an empty population passes and
/// looks healthy"), and it is the one thing worth pinning here.
#[test]
fn air_cut_family_calibration_table() {
    let (rows, skipped) = collect_rows();

    for s in &skipped {
        println!("W5BF4 SKIPPED | {s}");
    }

    assert!(
        rows.len() >= 20,
        "harness measured only {} rows — a calibration table this thin is not \
         evidence. Skipped: {:#?}",
        rows.len(),
        skipped
    );

    for r in &rows {
        assert!(
            r.move_count > 0 && r.sample_count > 0,
            "{:?} @ cell {:.2} produced an empty population ({} moves, {} samples) \
             — its air-cut reading is not a measurement",
            r.op,
            r.cell_mm,
            r.move_count,
            r.sample_count
        );
        assert!(
            (0.0..=100.0).contains(&r.air_pct_of_total),
            "{:?} @ cell {:.2}: air_cut_pct_of_total_runtime {} is outside [0, 100]",
            r.op,
            r.cell_mm,
            r.air_pct_of_total
        );
    }

    // Both denominators ship and they are not interchangeable: the
    // cutting-time reading excludes rapids from the denominator, so it is
    // always ≥ the total-runtime one. If that inverts, one of the two
    // accessors is being read for the other.
    for r in &rows {
        assert!(
            r.air_pct_of_cutting >= r.air_pct_of_total - 1e-9,
            "{:?} @ cell {:.2}: cutting-time air {} < total-runtime air {} — the \
             denominators are crossed",
            r.op,
            r.cell_mm,
            r.air_pct_of_cutting,
            r.air_pct_of_total
        );
    }

    // Cover more than one family, so a change that breaks every 3D arm
    // cannot leave a green 2D-only table behind.
    let families: std::collections::BTreeSet<_> = rows.iter().map(|r| r.family).collect();
    assert!(
        families.len() >= 3,
        "only {} families measured: {:?}",
        families.len(),
        families
    );
}

/// Print what the triage's action list actually CONTAINS on the 2.5D
/// golden's own fixture, item by item.
///
/// The landing note (`DELTA_sim_w5b_landing.md` §2) attributes the golden's
/// `triage_action_count: 3 → 1` to "two air-cut actions dropping below
/// their threshold". The adapter that turns verdicts into diagnostics emits
/// **one** `project.air_cut_high` diagnostic per project — it folds every
/// offender into a single `Verdict` with an `offender_toolpath_ids` list
/// (`session/compute.rs`'s air-cut verdict;
/// `diagnostics/adapters/from_project_diagnostics.rs`) — so an air-cut
/// count of two should not be constructible. This prints the ids rather
/// than arguing about them.
///
/// Run under both kernels to see the pair:
///
/// ```text
/// RS_CAM_STAMP_DISPATCH=whole_path cargo test -p rs_cam_core --release \
///     --test air_cut_family_calibration_w5bf4 triage -- --nocapture
/// ```
#[test]
fn triage_action_composition_on_the_two_and_a_half_d_golden_fixture() {
    let mut session = ProjectSession::new_empty();
    flat_stock(&mut session);
    let tool_id = add_tool(&mut session, ToolType::EndMill, 6.0, "End Mill 6mm");
    let model_id = add_rect_polygon(&mut session);
    for (family, build, _, _) in flat_cases() {
        if family != "2.5D clearing" && family != "2D contour" {
            continue;
        }
        let op = build();
        // The golden's three ops, in its order, on its one end mill.
        if !matches!(
            op.op_type(),
            OperationType::Pocket | OperationType::Zigzag | OperationType::Profile
        ) {
            continue;
        }
        let name = format!("{:?}", op.op_type());
        session
            .add_toolpath(
                0,
                toolpath_config(&name, op, tool_id, model_id, HeightsConfig::default()),
            )
            .expect("add toolpath");
    }
    assert_eq!(session.toolpath_count(), 3, "the golden fixture has 3 ops");

    let cancel = AtomicBool::new(false);
    for i in 0..session.toolpath_count() {
        session.generate_toolpath(i, &cancel).expect("generate");
    }
    session
        .run_simulation(&sim_options(1.0), &cancel)
        .expect("simulate");

    let holder = session.holder_collision_counts(&cancel);
    let sim = session.simulation_result().expect("sim result");
    let evidence =
        rs_cam_core::session::ProjectEvidence::from_simulation_with_holder_collisions(sim, holder);
    let diagnostics = session.diagnostics_with_evidence(&evidence);
    let triage = session.simulation_triage_with_diagnostics(&evidence, &diagnostics);

    println!(
        "W5BF4 TRIAGE | actions {} | safety {} | advisories {}",
        triage.actions.len(),
        triage.safety.len(),
        triage.advisories.items.len()
    );
    for a in &triage.actions {
        println!(
            "W5BF4 TRIAGE ACTION | {} | {}",
            a.diagnostic.id.as_str(),
            a.diagnostic.message
        );
    }
    for v in &diagnostics.verdicts {
        println!("W5BF4 VERDICT | {:?} | {}", v.kind, v.headline);
    }

    let air_actions = triage
        .actions
        .iter()
        .filter(|a| a.diagnostic.id.as_str() == "project.air_cut_high")
        .count();
    assert!(
        air_actions <= 1,
        "the air-cut verdict folds every offender into one diagnostic; \
         {air_actions} would mean the adapter changed"
    );
}
