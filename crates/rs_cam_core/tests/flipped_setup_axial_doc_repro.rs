//! F-024 follow-up — axial-engagement Z-frame mismatch on NON-IDENTITY
//! (flipped / face_up=Bottom) setups.
//!
//! F-024 fixed the per-setup dexel grid Z-frame for IDENTITY setups: the
//! grid now spans the world-frame stock bbox (matching the toolpath the
//! simulator stamps), so per-sample `axial_engagement_mm` reads the
//! commanded DOC instead of the full stock height. See
//! `dexel_stock_z_frame_f024.rs` (passes).
//!
//! But the live wanaka Back Rough runs on a `face_up=Bottom` setup and
//! reports peak axial DOC 12-20 mm vs a 3 mm commanded — because the
//! NON-IDENTITY path still roots the per-setup grid at the zero-origin
//! local bbox (`session/compute.rs` ~1206-1217: "Non-identity setups
//! continue to use the zero-origin effective bbox"), which can disagree
//! with the frame the toolpath is stamped in.
//!
//! This is the SYNTHETIC, fast repro the prior session's
//! `wanaka_axial_doc.rs` lacked (that one loads the 220k-tri wanaka mesh
//! and is `#[ignore]`). Same AS001 pocket as F-024, but routed to a
//! flipped setup. The acceptance bar is identical to F-024: first-pass
//! cutting samples must read `axial_engagement_mm <= 3.0 mm`.
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
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P2;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::session::{LoadedModel, ProjectSession, SimulationOptions, ToolpathConfig};
use rs_cam_core::simulation_cut::CutKinematics;

fn rounded_rect_with_island() -> Polygon2 {
    let exterior = vec![
        P2::new(5.0, 5.0),
        P2::new(75.0, 5.0),
        P2::new(75.0, 55.0),
        P2::new(5.0, 55.0),
    ];
    let mut hole = Vec::with_capacity(64);
    let (cx, cy, r, n) = (40.0, 30.0, 10.0, 64);
    for i in 0..n {
        let t = (i as f64) * TAU / (n as f64);
        hole.push(P2::new(cx + r * (-t).cos(), cy + r * (-t).sin()));
    }
    Polygon2::with_holes(exterior, vec![hole])
}

/// Same AS001 pocket as F-024 (100x100x12 hardwood, stock top at world
/// Z=0, 6 mm flat, depth 6, DPP 2 → first pass at Z=-2), but the toolpath
/// is placed on a `face_up=Bottom` (flipped, non-identity) setup.
fn build_flipped_pocket_session() -> ProjectSession {
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

    let model = LoadedModel {
        id: 0,
        name: "as001_pocket".to_owned(),
        mesh: None,
        polygons: Some(Arc::new(vec![rounded_rect_with_island()])),
        drill_targets: std::sync::Arc::new(Vec::new()),
        layers: std::sync::Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://as001_pocket.svg"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    };
    let model_id = session.add_model(model);

    // The repro lever: a flipped setup (index 1). new_empty() already made
    // identity setup 0; route the pocket onto the Bottom-face setup.
    let flipped_setup = session.add_setup("Flipped".to_owned(), FaceUp::Bottom);

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
        id: ToolpathId(0),
        name: "Pocket (flipped)".to_owned(),
        enabled: true,
        operation: OperationConfig::Pocket(pocket),
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
    session
        .add_toolpath(flipped_setup, tc)
        .expect("add pocket toolpath to flipped setup");

    session
}

/// First-pass cutting samples on a flipped setup must read
/// `axial_engagement_mm <= 3.0 mm` (commanded 2.0 + grid margin) — the
/// same F-024 bar that the identity sentry already meets. Pre-fix the
/// non-identity grid frame inflates these to ~the full stock height.
#[test]
fn flipped_setup_first_pass_axial_engagement_within_commanded_doc() {
    let mut session = build_flipped_pocket_session();
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

    // Non-plunge cutting samples (any Z — the flipped frame may emit the
    // first pass at a transformed Z, so don't band-filter on world Z=-2).
    let mut axials: Vec<f64> = cut_trace
        .samples
        .iter()
        .filter(|s| s.is_cutting && s.cut_kinematics != CutKinematics::Plunge)
        .map(|s| s.axial_engagement_mm)
        .collect();
    axials.sort_by(|a, b| a.partial_cmp(b).unwrap());

    assert!(
        !axials.is_empty(),
        "expected at least one linear/arc/helix cutting sample"
    );

    let peak = *axials.last().unwrap();
    eprintln!(
        "flipped-setup peak axial engagement = {peak:.4} mm across {} samples",
        axials.len()
    );
    assert!(
        peak <= 3.0,
        "F-024 follow-up: first-pass axial engagement on a FLIPPED (face_up=Bottom) setup should \
         be <= 3.0 mm (commanded 2.0 + grid margin); got peak = {peak:.4} mm. Pre-fix the \
         non-identity per-setup dexel grid is rooted at the zero-origin local bbox, disagreeing \
         with the toolpath stamping frame, so axial reads the full stock height."
    );
}
