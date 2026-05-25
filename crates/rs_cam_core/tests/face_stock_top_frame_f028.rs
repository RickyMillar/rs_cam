//! F-028 — Face op cuts at world Z=-depth instead of world Z=stock_top-depth.
//!
//! ## Background
//!
//! The face op planner (`crate::face::face_toolpath`) used to hardcode
//! `start_z = 0.0` for its depth stepping, emitting cuts at world
//! Z=[-depth, 0]. This produced the correct stock-engaging behavior for
//! the AS001 convention (stock origin chosen so the world stock top
//! sits at world Z=0), but on projects that load a 3D model with
//! `auto_from_model = true` the F-026 load path sets
//! `origin_z = bbox.min.z` — so the stock top is at world
//! Z=`bbox.min.z + stock.z` (e.g. 15 mm for `ux_step_plate_mdf.toml`,
//! whose plate model spans world Z=[0, 10] and padding=5).
//!
//! Pre-F-028 the face op emitted cuts at world Z=[-1, -0.5] but the
//! stock occupied world Z=[0, 15]. Every cutting move sat *below* the
//! dexel grid: `ray_blend_above` cleared the entire ray length and
//! per-sample `axial_engagement_mm` read ≈ `stock.z - depth_per_pass`.
//! Round-05 reported peak axial = 9.14 mm on a 0.5 mm-DOC pass; after
//! F-026 grew the stock 12→15 mm round-06 reported peak axial = 11.42
//! mm — the bug *got worse* because peak_axial scales with stock
//! height when the cutter sits below the entire dexel grid.
//!
//! ## Fix
//!
//! Three coordinated changes:
//!
//! - `HeightsConfig::resolve` — `top_z.Auto` now defaults to
//!   `ctx.stock_top_z` instead of `0.0`, so 2D ops anchor their depth
//!   stepping at the actual stock top in the operation's emission frame.
//! - `session/compute.rs::compute` — for identity setups
//!   (`face_up=Top`, `z_rotation=Deg0`) the `HeightContext` now carries
//!   the *world* stock bbox (mirroring the GUI viz controller's existing
//!   pattern in `controller/events/compute.rs` ~line 244-246), so
//!   `heights.top_z` resolves to the world stock top. Non-identity
//!   setups still use the zero-rooted local bbox; their toolpaths emit
//!   in setup-local frame and the session's `local_to_global` transform
//!   translates back to world.
//! - `face::face_toolpath` — anchors its depth stepping at the new
//!   `FaceParams::stock_top_z` field (populated from `heights.top_z`),
//!   so cuts emit at `stock_top - depth_in_pass` in the correct frame.
//!
//! ## Acceptance bars (from F-028 finding)
//!
//! On `ux_step_plate_mdf.toml` with AS004 face params (depth=1,
//! depth_per_pass=0.5, stepover=3.0, feed=2400), driven through
//! `ProjectSession::run_simulation`:
//!
//! 1. `peak_axial_doc_mm` for any pass ≤ `depth_per_pass + 0.1 mm`
//!    margin (i.e. ≤ 0.6 mm for 0.5 mm DOC).
//! 2. `deflection.peak_mm < 0.2` (off the Exceeds gate).
//! 3. `rapid_collision_count == 0`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::OperationConfig;
use rs_cam_core::compute::catalog::OperationType;
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::FaceConfig;
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::face::FaceDirection;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::session::{ProjectSession, SimulationOptions, ToolpathConfig};
use rs_cam_core::tool_load::DeflectionVerdict;

fn ux_step_plate_mdf_path() -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // tests/ lives under crates/rs_cam_core; test_data is at repo root
    p.push("..");
    p.push("..");
    p.push("test_data");
    p.push("ux_step_plate_mdf.toml");
    p
}

/// Load `ux_step_plate_mdf.toml`, attach an AS004-shape face op to the
/// existing setup using the existing 6mm endmill, and return the
/// session. Drives through `ProjectSession::load` — the same entry the
/// MCP / GUI take.
fn build_as004_face_session() -> ProjectSession {
    let toml_path = ux_step_plate_mdf_path();
    let mut session = ProjectSession::load(&toml_path).expect("load ux_step_plate_mdf");

    // The fixture ships two tools (6mm + 3mm endmills). Pick the 6mm.
    let tool_id = session
        .tools()
        .iter()
        .find(|t| t.name.starts_with("End Mill 6mm"))
        .map(|t| t.id.0)
        .expect("ux_step_plate_mdf has End Mill 6mm");

    let model_id = session
        .models()
        .first()
        .map(|m| m.id)
        .expect("ux_step_plate_mdf has at least one model");

    // AS004 face params (from `planning/toolpath_acceptance/cases_agent_smoke.csv`).
    let face = FaceConfig {
        stepover: 3.0,
        depth: 1.0,
        depth_per_pass: 0.5,
        feed_rate: 2400.0,
        plunge_rate: 500.0,
        stock_offset: 5.0,
        direction: FaceDirection::Zigzag,
        spindle_rpm: Some(18_000),
    };

    let tc = ToolpathConfig {
        id: 0,
        name: "Face (AS004)".to_owned(),
        enabled: true,
        operation: OperationConfig::Face(face),
        dressups: DressupConfig::for_op(OperationType::Face),
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
    };

    // The fixture loads with one setup at index 0 (identity / face_up=Top).
    session
        .add_toolpath(0, tc)
        .expect("add face toolpath to setup 0");

    session
}

/// Acceptance bar 1: `peak_axial_doc_mm` should not exceed the
/// commanded `depth_per_pass` plus a small discretisation margin.
///
/// Pre-F-028: peak_axial read ≈ stock.z - depth_per_pass (9.14 mm at
/// stock=12, 11.42 mm at stock=15).
/// Post-F-028: face emits cuts inside the stock and peak_axial reads
/// at most the commanded DOC plus grid discretisation slack.
#[test]
fn as004_face_peak_axial_within_commanded_doc() {
    let mut session = build_as004_face_session();
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate face toolpath");

    let opts = SimulationOptions {
        resolution: 0.5,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
    };
    session
        .run_simulation(&opts, &cancel)
        .expect("simulation completes");

    let sim = session.simulation_result().expect("simulation result");
    let cut_trace = sim.cut_trace.as_ref().expect("metric cut trace");
    let summary = cut_trace
        .toolpath_summaries
        .iter()
        .find(|s| s.toolpath_id == 0)
        .expect("toolpath summary for face toolpath");

    let peak_axial = summary.peak_axial_doc_mm;
    assert!(
        peak_axial <= 0.6,
        "F-028: AS004 face peak_axial_doc_mm should be <= 0.6 mm (commanded \
         depth_per_pass=0.5 + 0.1 mm margin); got {peak_axial:.4} mm. Pre-fix \
         this read ≈ stock.z - depth_per_pass because face cut at world \
         Z=-0.5 below the dexel grid [0, 15]."
    );
}

/// Acceptance bar 2: deflection.peak_mm < 0.2.
///
/// Pre-F-028: deflection over-fired Exceeds at 0.204-0.243 mm because
/// it consumed the inflated peak_axial.
#[test]
fn as004_face_deflection_within_safe_band() {
    let mut session = build_as004_face_session();
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate face toolpath");

    let opts = SimulationOptions {
        resolution: 0.5,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
    };
    session
        .run_simulation(&opts, &cancel)
        .expect("simulation completes");

    let report = session.tool_load_report();
    let verdict = report
        .per_toolpath
        .iter()
        .find(|v| v.toolpath_id == 0)
        .expect("verdict for face toolpath");

    let peak_mm = match &verdict.deflection {
        DeflectionVerdict::Within { peak_mm, .. } | DeflectionVerdict::Exceeds { peak_mm, .. } => {
            *peak_mm
        }
        DeflectionVerdict::Unmodeled { reason } => {
            panic!(
                "expected deflection to be modeled on AS004 face (6mm endmill, MDF, 0.5mm \
                 DOC); got Unmodeled({reason:?})"
            );
        }
    };

    assert!(
        peak_mm < 0.2,
        "F-028: AS004 face deflection peak should be < 200 µm; got {peak_mm:.6} mm. \
         Pre-fix this fired Exceeds at ~0.204-0.243 mm because the axial-engagement \
         metric reported ≈ stock height instead of the commanded DOC."
    );
    assert!(
        !verdict.deflection.is_exceeded(),
        "F-028: deflection verdict should not be Exceeds; got {:?}",
        verdict.deflection.state()
    );
}

/// Acceptance bar 3: `rapid_collision_count == 0` on the AS004 face
/// (a single face pass at 0.5 mm DOC over a 100x60 plate has no
/// reason to collide with the stock).
#[test]
fn as004_face_no_rapid_collisions() {
    let mut session = build_as004_face_session();
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate face toolpath");

    let opts = SimulationOptions {
        resolution: 0.5,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
    };
    session
        .run_simulation(&opts, &cancel)
        .expect("simulation completes");

    let diag = session.diagnostics();
    let tp_diag = diag
        .per_toolpath
        .iter()
        .find(|d| d.toolpath_id == 0)
        .expect("per-toolpath diagnostic for face toolpath");

    assert_eq!(
        tp_diag.rapid_collision_count, 0,
        "F-028: AS004 face should have zero rapid collisions; got {}. Pre-fix \
         the face toolpath emitted at world Z=-0.5 (below stock) so retracts and \
         rapids registered against the dexel grid [0, 15] as collisions.",
        tp_diag.rapid_collision_count
    );
}
