//! E3 — render the PRODUCTION unified-finish toolpath on the mountains,
//! all three bands, so the operator can see the contour behaviour the
//! raster-only review pages never drew
//! (`planning/metrology_2026-09-02/FINDINGS.md` §M6 follow-up).
//!
//! The operator expected topo-map loops and saw bent rasters — because
//! every review SVG so far drew ONLY the Shallow band's raster arms. On
//! tier-0, 52 % of the ground is MidSteep/VerySteep and cuts as scallop
//! ring cascades and waterlines. This instrument runs the REAL generator
//! (`unified_finish_toolpath_with_cancel`) with the mt2 R1.5 op's exact
//! params and dumps SVGs coloured by band at each segment midpoint:
//! Shallow raster = blue, MidSteep scallop = orange, VerySteep waterline
//! = red.
//!
//! Visual evidence only — no bars, no verdict.
//!
//! `#[ignore]` — needs the operator's wanaka mesh. SKIPs when absent.

#![allow(
    clippy::print_stderr,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines
)]

use std::fmt::Write as _;
use std::path::Path;

use rs_cam_core::finish::classify_probe::ClassificationSampler;
use rs_cam_core::finish::finish_planner::FinishPlannerParams;
use rs_cam_core::finish::finish_setup::build_classification_surface_with_sampler_and_cancel;
use rs_cam_core::finish::unified_finish::{
    UnifiedFinishParams, unified_finish_classification_resolution,
    unified_finish_toolpath_with_cancel,
};
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::tool::{MillingCutter, TaperedBallEndmill};
use rs_cam_core::toolpath::{MoveIntent, MoveType};

const WANAKA_MESH: &str = "/home/ricky/Downloads/wanaka200/rivmap_export/terrain.stl";

// mt2 R1.5 unified op params, verbatim
// (`planning/multitool_2026-08-23/wanaka200_mt2.toml`, tool_id 3).
const SCALLOP_HEIGHT: f64 = 0.03;
const TOLERANCE: f64 = 0.05;
const RASTER_STEPOVER: f64 = 0.596_992_462_263_972;
const Z_STEP: f64 = 0.596_992_462_263_972;
const SAMPLING: f64 = 0.5;
const FEED: f64 = 913.0;
const PLUNGE: f64 = 270.0;
const OVERLAP_MM: f64 = 2.0;
const HOOKUP_MM: f64 = 25.0;

/// Band colour by slope angle (deg) at a segment midpoint.
fn band_color(theta_deg: f64) -> &'static str {
    if theta_deg >= 75.0 {
        "#ff5252" // VerySteep — waterline
    } else if theta_deg >= 45.0 {
        "#ffb347" // MidSteep — scallop rings
    } else {
        "#7fd4ff" // Shallow — raster
    }
}

#[test]
#[ignore = "evidence run — needs the operator's wanaka mesh (not in repo)"]
fn wanaka_unified_topo_dump_e3() {
    eprintln!(
        "\n========== E3 — THE PRODUCTION UNIFIED FINISH, ALL THREE BANDS ==========\n\
         Visual evidence only: the real generator, mt2 R1.5 params, whole board.\n"
    );
    let mesh_path = Path::new(WANAKA_MESH);
    if !mesh_path.exists() {
        eprintln!("SKIP: {WANAKA_MESH} not present on this machine.");
        return;
    }
    let started = std::time::Instant::now();
    let mesh = TriangleMesh::from_stl_scaled(mesh_path, 1.0).expect("load wanaka terrain");
    let index = SpatialIndex::build_auto(&mesh);
    let r15 = TaperedBallEndmill::new(3.0, 2.8, 6.0, 30.5);
    let never_cancel = || false;

    let mut params = UnifiedFinishParams {
        scallop_height: SCALLOP_HEIGHT,
        tolerance: TOLERANCE,
        raster_stepover: RASTER_STEPOVER,
        z_step: Z_STEP,
        sampling: SAMPLING,
        feed_rate: FEED,
        plunge_rate: PLUNGE,
        safe_z: mesh.bbox.max.z + 5.0,
        intra_region_hookup_mm: HOOKUP_MM,
        ..UnifiedFinishParams::default()
    };
    params.classification_sampler = ClassificationSampler::PRODUCTION;
    let mut planner = FinishPlannerParams::for_tool(r15.cusp_radius_mm());
    planner.overlap_mm = OVERLAP_MM;

    eprintln!("generating (this is the real op — expect minutes)…");
    let (toolpath, _annotations, report) = unified_finish_toolpath_with_cancel(
        &mesh,
        &index,
        &r15,
        mesh.bbox.max.z,
        mesh.bbox.min.z - 0.5,
        &params,
        &planner,
        None,
        None,
        None,
        None,
        &never_cancel,
    )
    .expect("unified finish");
    eprintln!(
        "generated: {} moves, {} regions, {:.0} s",
        toolpath.moves.len(),
        report.decompose.region_count,
        started.elapsed().as_secs_f64()
    );

    // Slope lookup for colouring.
    let surface = build_classification_surface_with_sampler_and_cancel(
        &mesh,
        &index,
        &r15,
        unified_finish_classification_resolution(&r15, TOLERANCE),
        ClassificationSampler::PRODUCTION,
        &never_cancel,
    )
    .expect("classification surface");
    let slope = &surface.slope_map;
    let theta_at = |x: f64, y: f64| -> f64 {
        let col = ((x - slope.origin_x) / slope.cell_size)
            .round()
            .clamp(0.0, (slope.cols - 1) as f64) as usize;
        let row = ((y - slope.origin_y) / slope.cell_size)
            .round()
            .clamp(0.0, (slope.rows - 1) as f64) as usize;
        slope.angles[row * slope.cols + col].to_degrees()
    };

    // SVG: cutting segments coloured by band, batched per colour.
    let dump = |path: &std::path::Path, window: [f64; 4], stroke: f64| {
        let [x0, y0, x1, y1] = window;
        let (w, h) = (x1 - x0, y1 - y0);
        let ph = 1000.0 * h / w.max(1e-9);
        let mut per_color: std::collections::HashMap<&'static str, String> =
            std::collections::HashMap::new();
        for i in 1..toolpath.moves.len() {
            let m = &toolpath.moves[i];
            if matches!(m.move_type, MoveType::Rapid) {
                continue;
            }
            if !matches!(m.intent, MoveIntent::ClearingCut | MoveIntent::FinishingCut) {
                continue;
            }
            let a = toolpath.moves[i - 1].target;
            let b = m.target;
            let inside = |px: f64, py: f64| px >= x0 && px <= x1 && py >= y0 && py <= y1;
            if !(inside(a.x, a.y) && inside(b.x, b.y)) {
                continue;
            }
            let color = band_color(theta_at((a.x + b.x) / 2.0, (a.y + b.y) / 2.0));
            let d = per_color.entry(color).or_default();
            let _ = write!(
                d,
                "M{:.2},{:.2}L{:.2},{:.2}",
                a.x - x0,
                y1 - a.y,
                b.x - x0,
                y1 - b.y
            );
        }
        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w:.1} {h:.1}\" \
             width=\"1000\" height=\"{ph:.0}\">\n\
             <rect width=\"100%\" height=\"100%\" fill=\"#101418\"/>\n"
        );
        for (color, d) in &per_color {
            let _ = writeln!(
                svg,
                "<path fill=\"none\" stroke=\"{color}\" stroke-width=\"{stroke}\" d=\"{d}\"/>"
            );
        }
        svg.push_str("</svg>\n");
        let _ = std::fs::write(path, svg);
    };

    let out = std::path::Path::new("target/unified_topo_e3");
    let _ = std::fs::create_dir_all(out);
    let board = [
        mesh.bbox.min.x,
        mesh.bbox.min.y,
        mesh.bbox.max.x,
        mesh.bbox.max.y,
    ];
    dump(&out.join("unified_full.svg"), board, 0.08);
    // Crop 1: the E2 review crop, for comparability.
    dump(
        &out.join("unified_crop_flats.svg"),
        [49.75, 123.0625, 89.75, 163.0625],
        0.05,
    );
    // Crop 2: around the highest peak — the steep massif.
    // Highest vertex at least 30 mm from the board rim — the true rim is
    // a wall and the global max sits on it.
    let mut peak = (0.0, 0.0, f64::NEG_INFINITY);
    for v in &mesh.vertices {
        let interior = v.x > mesh.bbox.min.x + 30.0
            && v.x < mesh.bbox.max.x - 30.0
            && v.y > mesh.bbox.min.y + 30.0
            && v.y < mesh.bbox.max.y - 30.0;
        if interior && v.z > peak.2 {
            peak = (v.x, v.y, v.z);
        }
    }
    dump(
        &out.join("unified_crop_peak.svg"),
        [peak.0 - 20.0, peak.1 - 20.0, peak.0 + 20.0, peak.1 + 20.0],
        0.05,
    );
    eprintln!(
        "SVGs written to target/unified_topo_e3/ (peak crop at ({:.1}, {:.1})); total {:.0} s",
        peak.0,
        peak.1,
        started.elapsed().as_secs_f64()
    );
}
