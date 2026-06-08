//! F-024 — Z-frame mismatch in dexel stock grid for identity setups.
//!
//! Reproduces the AS001-shape pocket: hardwood stock, 6mm flat endmill,
//! `stock_origin_z = -12`, depth = 6, depth_per_pass = 2.
//!
//! Pre-fix (round-03 evidence): for identity setups the per-setup dexel
//! grid was rooted at world origin `(0,0,0)..(stock_x, stock_y, stock_z)`
//! (Z=[0, 12]), but the toolpath emits cuts in world frame (Z = -2 for a
//! 2 mm-DOC first pass). The cutter Z=-2 sat below every dexel ray, so
//! `ray_blend_above` cleared the entire ray length and the per-sample
//! `axial_engagement_mm` read the full stock height (~10-12 mm) rather
//! than the commanded 2 mm. The deflection gate consumed the inflated
//! axial value and reported 374-573 µm tip deflection on cases that
//! should have read well under 50 µm.
//!
//! Post-fix: identity setups now pass `local_stock_bbox = None` from
//! `session/compute.rs`, so `run_simulation` falls back to
//! `request.stock_bbox` (world frame) for the per-setup grid. The grid
//! Z range matches the toolpath frame, axial-engagement reads the
//! commanded DOC, and the deflection gate fires on real overload only.
//!
//! Acceptance bars from F-024:
//! 1. Unit: per-sample `axial_engagement_mm` for any linear/arc/helix
//!    cutting sample on the first pass <= 3.0 mm (commanded 2.0 + grid
//!    discretisation margin).
//! 2. Integration: deflection `peak_mm < 0.2` (off the Exceeds band).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;
use common::make_endmill_6mm;

use std::f64::consts::TAU;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{PocketConfig, PocketPattern};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, SimulationOptions, ToolpathConfig};
use rs_cam_core::simulation_cut::CutKinematics;
use rs_cam_core::tool_load::DeflectionVerdict;

fn rounded_rect_with_island() -> Polygon2 {
    let exterior = vec![
        P2::new(5.0, 5.0),
        P2::new(75.0, 5.0),
        P2::new(75.0, 55.0),
        P2::new(5.0, 55.0),
    ];

    let mut hole = Vec::with_capacity(64);
    let cx = 40.0;
    let cy = 30.0;
    let r = 10.0;
    let n = 64;
    for i in 0..n {
        let t = (i as f64) * TAU / (n as f64);
        // CW for holes (negative area)
        hole.push(P2::new(cx + r * (-t).cos(), cy + r * (-t).sin()));
    }

    Polygon2::with_holes(exterior, vec![hole])
}

/// Build an AS001-shape pocket: 100x100x12 hardwood stock with origin
/// at (-10, -10, -12) so stock top sits at world Z=0; pocket depth 6,
/// `depth_per_pass = 2` so the first pass cuts at Z=-2.
fn build_as001_pocket_session() -> ProjectSession {
    let mut session = ProjectSession::new_empty();

    let stock = StockConfig {
        x: 100.0,
        y: 100.0,
        z: 12.0,
        origin_x: -10.0,
        origin_y: -10.0,
        origin_z: -12.0,
        auto_from_model: false,
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..StockConfig::default()
    };
    session.set_stock_config(stock);

    let tool_idx = session.add_tool(make_endmill_6mm());
    let tool_id = session.tools()[tool_idx].id.0;

    let polygon = rounded_rect_with_island();
    let model = LoadedModel {
        id: 0,
        name: "as001_pocket".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![polygon])),
        path: PathBuf::from("synthetic://as001_pocket.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    };
    let model_id = session.add_model(model);

    let pocket = PocketConfig {
        stepover: 2.0,
        depth: 6.0,
        depth_per_pass: 2.0,
        feed_rate: 770.0,
        plunge_rate: 385.0,
        climb: true,
        pattern: PocketPattern::Contour,
        angle: 0.0,
        finishing_passes: 0,
        spindle_rpm: Some(18_000),
    };

    let tc = ToolpathConfig {
        id: 0,
        name: "Pocket".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(pocket),
        // Same dressup envelope as `add_toolpath` defaults (Ramp entry +
        // link moves + arc fit + TSP). Matches the smoke-acceptance AS001.
        dressups: DressupConfig::for_op(OperationType::Pocket),
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
    };
    session.add_toolpath(0, tc).expect("add pocket toolpath");

    session
}

/// Acceptance test 1 (unit / sim layer).
///
/// Per-sample `axial_engagement_mm` for any linear/arc/helix cutting
/// sample on the AS001 pocket first pass must be <= 3.0 mm. Pre-fix
/// these samples read ~10-12 mm (full stock height); post-fix they
/// read the commanded 2.0 mm (plus a small discretisation margin).
#[test]
fn as001_pocket_first_pass_axial_engagement_within_commanded_doc() {
    let mut session = build_as001_pocket_session();
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate pocket toolpath");

    let opts = SimulationOptions {
        resolution: 1.0,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    };
    session
        .run_simulation(&opts, &cancel)
        .expect("simulation completes");

    let sim = session.simulation_result().expect("simulation result");
    let cut_trace = sim.cut_trace.as_ref().expect("metric cut trace");

    // Collect non-plunge cutting samples for the first-pass Z band
    // (centred near Z=-2 with a small tolerance). Pre-fix these samples
    // would all read `axial_engagement_mm` ~= stock_z (= 12 mm).
    let mut first_pass_axials: Vec<f64> = cut_trace
        .samples
        .iter()
        .filter(|s| s.is_cutting && s.cut_kinematics != CutKinematics::Plunge)
        .filter(|s| (s.position[2] - (-2.0)).abs() < 0.5)
        .map(|s| s.axial_engagement_mm)
        .collect();
    first_pass_axials.sort_by(|a, b| a.partial_cmp(b).unwrap());

    assert!(
        !first_pass_axials.is_empty(),
        "expected at least one linear/arc/helix cutting sample near Z=-2 on the first pass"
    );

    let peak = *first_pass_axials.last().unwrap();
    assert!(
        peak <= 3.0,
        "F-024: first-pass axial engagement should be <= 3.0 mm (commanded 2.0 mm + grid \
         discretisation margin); got peak = {peak:.4} mm across {} samples. Pre-fix this read \
         the full stock height (~10-12 mm) because the dexel grid was rooted at zero-local Z \
         while the toolpath emitted negative world Z.",
        first_pass_axials.len()
    );
}

/// Acceptance test 2 (integration / deflection gate).
///
/// Full sim through the tool-load gate must report deflection
/// `peak_mm < 0.2` on the AS001 pocket. Pre-fix the gate fired
/// `Exceeds` at 374 µm because it consumed the inflated axial
/// engagement; post-fix it should land in the `Within` band well
/// below the 200 µm `Exceeds` threshold.
#[test]
fn as001_pocket_deflection_gate_within_safe_band() {
    let mut session = build_as001_pocket_session();
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate pocket toolpath");

    let opts = SimulationOptions {
        resolution: 1.0,
        skip_ids: Vec::new(),
        metrics_enabled: true,
        auto_resolution: false,
        use_predicted_feed_in_gates: false,
        adaptive_feed_modulation: false,
        modulation_strategy: rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
        modulation_aggressiveness: 1.0,
    };
    session
        .run_simulation(&opts, &cancel)
        .expect("simulation completes");

    let report = session.tool_load_report();
    let verdict = report
        .per_toolpath
        .iter()
        .find(|v| v.toolpath_id == 0)
        .expect("verdict for pocket toolpath");

    let peak_mm = match &verdict.deflection {
        DeflectionVerdict::Within { peak_mm, .. } | DeflectionVerdict::Exceeds { peak_mm, .. } => {
            *peak_mm
        }
        DeflectionVerdict::Unmodeled { reason } => {
            panic!(
                "expected deflection to be modeled on AS001 pocket (6mm endmill, hardwood, 2mm \
                 DOC); got Unmodeled({reason:?})"
            );
        }
    };

    assert!(
        peak_mm < 0.2,
        "F-024: deflection peak should be < 200 µm on a 6mm endmill cutting 2mm DOC in hardwood; \
         got peak_mm = {peak_mm:.6} mm. Pre-fix this gate fired Exceeds at ~0.374 mm because the \
         axial-engagement metric reported the full stock height instead of the commanded DOC."
    );

    // Sanity guard: the verdict should not be Exceeds.
    assert!(
        !verdict.deflection.is_exceeded(),
        "F-024: deflection verdict should not be Exceeds; got {:?}",
        verdict.deflection.state()
    );
}
