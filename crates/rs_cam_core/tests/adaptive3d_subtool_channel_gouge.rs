//! Adaptive3d sub-tool-width deep-channel gouge — synthetic fast repro.
//!
//! ## The bug (root-caused 2026-06-16, see
//! `planning/ADAPTIVE3D_DEEP_CHANNEL_GOUGE_2026-06-16.md`)
//!
//! On the real `wanaka` terrain, every 3D rough (agent OR spiral) carved
//! the coastline "river" channel ~6 mm too deep — a visible gouge,
//! method-independent, always at the same channel floor. It is a REAL
//! over-cut, not a metric artifact (the dexel probe showed real material
//! `ray_top = 11.18` inside the cutter footprint right before the peak
//! move, which cut to z=5.05 → ~6 mm removal).
//!
//! Mechanism: a flat end mill follows down into a sub-tool-width, deep
//! valley. The cut Z at each path point comes from the **point** leave
//! surface (`surface_z_at` in `build_material_bool_grid`, `surface_z_at_world`
//! in the `lift` closure), with NO cutter-radius compensation. The radius-3
//! cutter centred on the narrow floor laps its footprint onto the tall,
//! still-uncleared valley walls and gouges them down to the floor Z — far
//! more than the commanded `depth_per_pass`.
//!
//! This is distinct from F-027 (planner/sim XY-grid mismatch at the model
//! edge) and F-031 (planner↔dressup helix entry-style parity gap): both of
//! those were parity artifacts where the planner *believed* a cell was
//! cleared. Here the planner genuinely drives the tool into a feature it
//! physically can't fit; a 6 mm flat tool cannot rough a sub-tool-width,
//! several-mm-deep notch without gouging its walls.
//!
//! ## Synthetic fixture
//!
//! A heightfield "terrain": a high plateau (keep-surface just below stock
//! top) with one deep valley running along X. The valley has a WIDE gentle
//! funnel at the rim (so the tool is driven down into it by surface-
//! following) narrowing into a STEEP, sub-tool-width V at the bottom (so
//! the cutter footprint laps the tall walls while its centre sits on the
//! deep floor). Stock top at world Z=0, 6 mm flat end mill, DPP 3.
//!
//! ## What this test established (2026-06-16)
//!
//! It was written to reproduce the wanaka gouge under the prior root-cause
//! hypothesis (flat tool draped into a sub-tool notch via a point-surface
//! lift, fixable by cutter-radius dilation). **It REFUTES that hypothesis:**
//! the rough does NOT gouge this sub-tool valley. The tool descends only to
//! z=-9 (not the -12 floor — `point_drop_cutter` stops a flat endmill at its
//! rest height), the deepest samples carry the lowest axial, and peak
//! steady-state axial is ~2.46 mm (< DPP). The `SurfaceHeightmap` is already
//! cutter-radius-dilated by construction (drop-cutter rest height), so the
//! lift/mask the engine uses cannot drive a flat tool below where it rests.
//!
//! It is kept as a SENTRY: a flat-tool rough of a deep sub-tool-width valley
//! on a clean mesh must not produce steady-state axial over `DPP + 0.5`. The
//! real wanaka gouge comes from a planner↔simulator stock-state parity gap
//! (uncleared stock above the mesh surface swept in one bite), not from this
//! geometry — see `planning/ADAPTIVE3D_DEEP_CHANNEL_GOUGE_2026-06-16.md`.
//!
//! ## Acceptance bar
//!
//! Whole-toolpath worst-case `axial_engagement_mm` across all steady-state
//! (non-plunge, non-transit) cutting samples ≤ `depth_per_pass + 0.5`.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;
use common::make_endmill_6mm;
use rs_cam_core::ids::ToolpathId;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rs_cam_core::compute::catalog::{OperationConfig, OperationType};
use rs_cam_core::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig, StockSource};
use rs_cam_core::compute::operation_configs::{
    Adaptive3dConfig, Adaptive3dEntryStyle, ClearingStrategy, RegionOrdering,
};
use rs_cam_core::compute::stock_config::StockConfig;
use rs_cam_core::debug_trace::ToolpathDebugOptions;
use rs_cam_core::gcode::CoolantMode;
use rs_cam_core::geo::P3;
use rs_cam_core::material::{Material, WoodSpecies};
use rs_cam_core::mesh::TriangleMesh;
use rs_cam_core::session::{LoadedModel, ProjectSession, SimulationOptions, ToolpathConfig};
use rs_cam_core::simulation_cut::CutKinematics;

const PLATEAU_Z: f64 = -0.5;
const FLOOR_Z: f64 = -12.0;
/// Valley centred at this Y, running along X.
const VALLEY_Y: f64 = 30.0;
/// Rim half-width: |y - VALLEY_Y| beyond this is flat plateau.
const RIM_HALF: f64 = 12.0;
/// Below this half-width the wall goes steep + sub-tool-width.
const STEEP_HALF: f64 = 3.0;
/// Keep-surface Z at the steep/funnel transition.
const FUNNEL_Z: f64 = -4.0;

/// Heightfield keep-surface: high plateau with one deep valley along X.
/// Wide gentle funnel (rim → FUNNEL_Z) over a steep sub-tool-width V
/// (FUNNEL_Z → FLOOR_Z within +/- STEEP_HALF of the centreline).
fn keep_surface_z(_x: f64, y: f64) -> f64 {
    let dy = (y - VALLEY_Y).abs();
    if dy >= RIM_HALF {
        PLATEAU_Z
    } else if dy >= STEEP_HALF {
        // Gentle funnel: plateau at the rim down to FUNNEL_Z at STEEP_HALF.
        let t = (dy - STEEP_HALF) / (RIM_HALF - STEEP_HALF);
        FUNNEL_Z + (PLATEAU_Z - FUNNEL_Z) * t
    } else {
        // Steep sub-tool-width V: FUNNEL_Z at STEEP_HALF down to FLOOR_Z
        // at the centreline.
        let t = dy / STEEP_HALF;
        FLOOR_Z + (FUNNEL_Z - FLOOR_Z) * t
    }
}

/// Triangulate the keep-surface over `[0, span] x [0, span]` at `step` mm.
fn heightfield_mesh(span: f64, step: f64) -> TriangleMesh {
    let n = (span / step).round() as usize;
    let cols = n + 1;
    let mut verts: Vec<P3> = Vec::with_capacity(cols * cols);
    for iy in 0..cols {
        let y = iy as f64 * step;
        for ix in 0..cols {
            let x = ix as f64 * step;
            verts.push(P3::new(x, y, keep_surface_z(x, y)));
        }
    }
    let mut tris: Vec<[u32; 3]> = Vec::with_capacity(n * n * 2);
    for iy in 0..n {
        for ix in 0..n {
            let a = (iy * cols + ix) as u32;
            let b = (iy * cols + ix + 1) as u32;
            let c = ((iy + 1) * cols + ix) as u32;
            let d = ((iy + 1) * cols + ix + 1) as u32;
            tris.push([a, b, d]);
            tris.push([a, d, c]);
        }
    }
    TriangleMesh::from_raw(verts, tris)
}

fn build_session() -> ProjectSession {
    let mut session = ProjectSession::new_empty();

    // Stock: 60x60, top at world Z=0, deep enough for the FLOOR_Z=-12 valley.
    let stock = StockConfig {
        x: 60.0,
        y: 60.0,
        z: 16.0,
        origin_x: 0.0,
        origin_y: 0.0,
        origin_z: -16.0,
        auto_from_model: false,
        material: Material::SolidWood {
            species: WoodSpecies::GenericHardwood,
        },
        ..StockConfig::default()
    };
    session.set_stock_config(stock);

    let tool_idx = session.add_tool(make_endmill_6mm());
    let tool_id = session.tools()[tool_idx].id.0;

    let mesh = heightfield_mesh(60.0, 0.5);
    let model = LoadedModel {
        id: 0,
        name: "subtool_valley".to_owned(),
        mesh: Some(Arc::new(mesh)),
        polygons: None,
        drill_targets: std::sync::Arc::new(Vec::new()),
        layers: std::sync::Arc::new(Vec::new()),
        path: PathBuf::from("synthetic://subtool_valley.stl"),
        kind: None,
        units: None,
        enriched_mesh: None,
        winding_report: None,
        load_error: None,
    };
    let model_id = session.add_model(model);

    let adaptive3d = Adaptive3dConfig {
        trochoid_cap_mult: 1.6,
        engagement_measure: rs_cam_core::adaptive::EngagementMeasure::DiskArea,
        stepover: 1.2,
        depth_per_pass: 3.0,
        stock_to_leave_axial: 0.0,
        stock_to_leave_radial: 0.0,
        feed_rate: 2500.0,
        plunge_rate: 500.0,
        tolerance: 0.25,
        min_cutting_radius: 0.0,
        entry_style: Adaptive3dEntryStyle::Plunge,
        ramp_angle_deg: 3.0,
        helix_radius_factor: 0.4,
        helix_pitch: 1.0,
        fine_stepdown: 0.0,
        detect_flat_areas: false,
        region_ordering: RegionOrdering::Global,
        clearing_strategy: ClearingStrategy::ContourParallel,
        z_blend: false,
        mill_shallow_areas: false,
        shallow_angle_deg: None,
        shallow_stepdown: None,
        spindle_rpm: Some(18_000),
        min_region_cut_length_mm: 0.0,
        max_stay_down_distance_mm: Some(0.0),
        stay_down_clearance_mm: 0.5,
    };

    let tc = ToolpathConfig {
        id: ToolpathId(0),
        name: "subtool rough".to_owned(),
        enabled: true,
        operation: OperationConfig::Adaptive3d(adaptive3d),
        dressups: DressupConfig::for_op(OperationType::Adaptive3d),
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
        rest_analysis: rs_cam_core::compute::config::RestAnalysisConfig::default(),
    };
    session
        .add_toolpath(0, tc)
        .expect("add adaptive3d toolpath");

    session
}

#[test]
fn subtool_valley_floor_pass_does_not_gouge_walls() {
    let mut session = build_session();
    let cancel = AtomicBool::new(false);
    session
        .generate_toolpath(0, &cancel)
        .expect("generate adaptive3d toolpath");

    let opts = SimulationOptions {
        resolution: 0.5,
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

    let commanded_dpp = 3.0_f64;
    let limit = commanded_dpp + 0.5;

    let mut steady: Vec<&rs_cam_core::simulation_cut::SimulationCutSample> = cut_trace
        .samples
        .iter()
        .filter(|s| s.is_cutting && s.cut_kinematics != CutKinematics::Plunge && !s.in_transit_span)
        .collect();
    steady.sort_by(|a, b| {
        a.axial_engagement_mm
            .partial_cmp(&b.axial_engagement_mm)
            .unwrap()
    });

    assert!(!steady.is_empty(), "expected steady-state cutting samples");

    // Diagnostic: did the tool descend into the deep valley at all, and what
    // is the max axial per Z-level? (Distinguishes "drop-cutter protected the
    // narrow valley → never cut deep" from "cut deep but no gouge".)
    {
        use std::collections::BTreeMap;
        let mut by_z: BTreeMap<i32, (usize, f64)> = BTreeMap::new();
        for s in cut_trace
            .samples
            .iter()
            .filter(|s| s.is_cutting && s.cut_kinematics != CutKinematics::Plunge)
        {
            let zk = s.position[2].round() as i32;
            let e = by_z.entry(zk).or_insert((0, 0.0));
            e.0 += 1;
            if s.axial_engagement_mm > e.1 {
                e.1 = s.axial_engagement_mm;
            }
        }
        let min_z = cut_trace
            .samples
            .iter()
            .filter(|s| s.is_cutting)
            .map(|s| s.position[2])
            .fold(f64::INFINITY, f64::min);
        eprintln!(
            "deepest cutting sample z = {min_z:.2} (valley floor surface = {FLOOR_Z}); \
             max-axial by z-level (z: count, maxAxial):"
        );
        for (z, (c, a)) in &by_z {
            eprintln!("  z≈{z}: {c} samples, maxAxial={a:.3}");
        }
    }

    let peak = steady.last().unwrap();
    eprintln!(
        "subtool-valley: {} steady samples, peak axial = {:.3} mm at \
         (x={:.2}, y={:.2}, z={:.2}); commanded DPP = {commanded_dpp}",
        steady.len(),
        peak.axial_engagement_mm,
        peak.position[0],
        peak.position[1],
        peak.position[2],
    );
    // Top 8 by axial for diagnostics.
    for s in steady.iter().rev().take(8) {
        eprintln!(
            "  axial={:.3}  pos=({:.2},{:.2},{:.2})  kin={:?}",
            s.axial_engagement_mm, s.position[0], s.position[1], s.position[2], s.cut_kinematics,
        );
    }

    assert!(
        peak.axial_engagement_mm <= limit,
        "sentry: a flat-tool rough of a deep sub-tool-width valley on a CLEAN mesh produced \
         steady-state peak axial_engagement_mm = {:.3} mm (> DPP + 0.5 = {limit:.3}). The \
         drop-cutter surface heightmap should stop the flat tool at its rest height and prevent \
         this. If this fires, the mesh-based surface protection regressed — but note the real \
         wanaka gouge is a planner↔sim stock-state parity gap, not this geometry.",
        peak.axial_engagement_mm,
    );
}
