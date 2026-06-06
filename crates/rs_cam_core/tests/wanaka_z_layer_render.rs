//! Wanaka Back Rough first/last Z-layer SVG diagnostic.
//!
//! Loads the Wanaka terrain mesh directly, runs adaptive3d with
//! Back-Rough-matching params, partitions the toolpath by Z level
//! (via runtime annotations), and renders the FIRST (topmost) and
//! LAST (deepest) Z layers side-by-side as a single SVG file at
//! `target/wanaka_first_last_z.svg`.
//!
//! Iteration test ground for the adaptive3d ContourParallelHybrid
//! integration (#162). Re-run after each algorithm change to see
//! whether the bottom layer (complex 3D terrain) cleans up and the
//! top layer (effectively a pocket) stays correct.
//!
//! Skipped by default (requires the local Wanaka STL):
//!   `cargo test -p rs_cam_core --test wanaka_z_layer_render -- --ignored --nocapture`

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr,
    clippy::print_stdout
)]

use std::fs;
use std::path::Path;

use rs_cam_core::{
    adaptive3d::{
        Adaptive3dParams, ClearingStrategy3d, EntryStyle3d, RegionOrdering,
        adaptive_3d_toolpath_annotated,
    },
    mesh::{SpatialIndex, TriangleMesh},
    tool::{FlatEndmill, MillingCutter},
    toolpath::{MoveType, Toolpath},
};

const STL_PATH: &str = "/home/ricky/Downloads/wanaka100/rivmap_export/terrain.stl";

fn parse_z_level_label(label: &str) -> Option<f64> {
    if let Some(rest) = label.strip_prefix("Adaptive Z ") {
        let z_str = rest.split(' ').next()?;
        z_str.parse::<f64>().ok()
    } else {
        None
    }
}

/// Returns Vec<(z_level, agent_first_move, agent_end_move_exclusive)>
fn partition_by_z_level(annotations: &[(usize, String)]) -> Vec<(f64, usize, usize)> {
    let z_ann: Vec<(usize, f64, usize)> = annotations
        .iter()
        .enumerate()
        .filter_map(|(ann_idx, (move_idx, label))| {
            parse_z_level_label(label).map(|z| (ann_idx, z, *move_idx))
        })
        .collect();

    let mut out: Vec<(f64, usize, usize)> = Vec::new();
    for (i, &(z_ann_idx, z, agent_start)) in z_ann.iter().enumerate() {
        let (next_ann_idx, next_z_start) = z_ann
            .get(i + 1)
            .map(|t| (t.0, t.2))
            .unwrap_or((annotations.len(), usize::MAX));
        let waterline_or_end = annotations[z_ann_idx + 1..next_ann_idx]
            .iter()
            .find_map(|(move_idx, label)| {
                if label == "Waterline cleanup" {
                    Some(*move_idx)
                } else {
                    None
                }
            })
            .unwrap_or(next_z_start);
        out.push((z, agent_start, waterline_or_end));
    }
    out
}

/// Project the toolpath moves in [start, end) onto XY and render as SVG.
/// Cut moves are green; rapids are red-dashed.
fn render_layer_svg(
    toolpath: &Toolpath,
    start: usize,
    end: usize,
    z_level: f64,
    bbox: (f64, f64, f64, f64),
    title: &str,
) -> String {
    let (xmin, ymin, xmax, ymax) = bbox;
    let pad = 5.0_f64;
    let view_x = xmin - pad;
    let view_y = ymin - pad;
    let view_w = (xmax - xmin) + 2.0 * pad;
    let view_h = (ymax - ymin) + 2.0 * pad;

    let mut svg = String::new();
    svg.push_str(&format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='{view_x} {} {view_w} {view_h}'>\n",
        -ymax - pad
    ));
    svg.push_str("<g transform='scale(1,-1)'>\n");
    svg.push_str(&format!(
        "<rect x='{view_x}' y='{view_y}' width='{view_w}' height='{view_h}' fill='#fafafa'/>\n"
    ));
    svg.push_str(&format!(
        "<text x='{:.1}' y='{:.1}' font-family='monospace' font-size='3' fill='#222' transform='scale(1,-1)'>{title}</text>\n",
        view_x + 1.0,
        -(view_y + view_h - 2.0)
    ));

    // Walk moves and collect segments.
    let end_clamped = end.min(toolpath.moves.len());
    let mut prev = if start > 0 {
        toolpath.moves.get(start - 1).map(|m| m.target)
    } else {
        toolpath.moves.first().map(|m| m.target)
    };

    let mut cut_count = 0usize;
    let mut rapid_count = 0usize;

    for m in toolpath.moves[start..end_clamped].iter() {
        if let Some(from) = prev {
            let to = m.target;
            let (color, dasharray) = match m.move_type {
                MoveType::Rapid => {
                    rapid_count += 1;
                    ("red", " stroke-dasharray='0.5,0.5'")
                }
                MoveType::Linear { .. } | MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => {
                    cut_count += 1;
                    ("green", "")
                }
            };
            svg.push_str(&format!(
                "<line x1='{:.3}' y1='{:.3}' x2='{:.3}' y2='{:.3}' stroke='{color}' stroke-width='0.15'{dasharray}/>\n",
                from.x, from.y, to.x, to.y
            ));
        }
        prev = Some(m.target);
    }

    let move_count = end_clamped.saturating_sub(start);
    svg.push_str(&format!(
        "<text x='{:.1}' y='{:.1}' font-family='monospace' font-size='2.5' fill='#444' transform='scale(1,-1)'>Z={z_level:.2}  moves={move_count}  cuts={cut_count}  rapids={rapid_count}</text>\n",
        view_x + 1.0,
        -(view_y + view_h - 5.5)
    ));

    svg.push_str("</g>\n</svg>\n");
    svg
}

fn write_side_by_side(svg_first: &str, svg_last: &str, out_path: &Path) {
    // Combine two SVGs side-by-side as a single SVG. Simplest: place both as
    // nested <svg> elements with x-offsets. Caller renders to PNG via convert.
    let combined = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 200 100'>\n\
         <g transform='translate(0,0) scale(1,1)'>\n{}</g>\n\
         <g transform='translate(100,0) scale(1,1)'>\n{}</g>\n\
         </svg>\n",
        svg_first, svg_last
    );
    fs::write(out_path, combined).expect("write svg");
}

#[ignore = "wanaka diagnostic — run with --ignored"]
#[test]
fn wanaka_back_rough_first_and_last_z_layers() {
    if !Path::new(STL_PATH).exists() {
        eprintln!("skip: {STL_PATH} not present");
        return;
    }

    // ── Load mesh ─────────────────────────────────────────────────
    let mesh = TriangleMesh::from_stl(Path::new(STL_PATH)).expect("load wanaka terrain.stl");
    let si = SpatialIndex::build_auto(&mesh);

    // ── Cutter: tool_id=3 in wanaka_full_tuned.toml is an end mill. ─
    // Diameter approximated from Back Rough params (stepover=2.53 ≈ 84% of
    // 6mm radius), and the project uses ½" (≈ 6.35mm) end mills throughout.
    let cutter = FlatEndmill::new(6.0, 25.0);

    // ── Adaptive3d params (Back Rough from wanaka_full_tuned.toml) ─
    let mesh_bbox = mesh.bbox;
    let stock_top_z: f64 = mesh_bbox.max.z;
    let stock_bottom_z: f64 = mesh_bbox.min.z;
    eprintln!(
        "Wanaka mesh bbox: x=[{:.2}..{:.2}] y=[{:.2}..{:.2}] z=[{:.2}..{:.2}]",
        mesh_bbox.min.x,
        mesh_bbox.max.x,
        mesh_bbox.min.y,
        mesh_bbox.max.y,
        mesh_bbox.min.z,
        mesh_bbox.max.z,
    );

    // Override stock_to_leave from the project's 4mm down to 0.5mm so
    // multiple Z layers fit; the project's 4mm leaves only the topmost
    // layer for the back-rough roughing pass before the finishing op.
    let params = Adaptive3dParams {
        tool_radius: cutter.radius(),
        envelope_radius: cutter.radius(),
        stepover: 2.53,
        depth_per_pass: 1.5,
        stock_to_leave: 0.5,
        feed_rate: 4000.0,
        plunge_rate: 750.0,
        safe_z: stock_top_z + 16.0,
        tolerance: 0.1,
        min_cutting_radius: 0.0,
        stock_top_z,
        entry_style: EntryStyle3d::Plunge,
        fine_stepdown: None,
        detect_flat_areas: false,
        max_stay_down_dist: None,
        region_ordering: RegionOrdering::Global,
        initial_stock: None,
        clearing_strategy: ClearingStrategy3d::AgentSearch,
        z_blend: false,
        boundary: None,
        mill_shallow_areas: false,
        shallow_angle_rad: None,
        shallow_stepdown: None,
        world_stock_xy_bbox: None,
        min_region_cut_length_mm: 0.0,
        max_stay_down_distance_mm: Some(0.0),
        stay_down_clearance_mm: 0.5,
    };

    let _ = stock_bottom_z;

    // ── Generate ───────────────────────────────────────────────────
    eprintln!("generating adaptive3d Back Rough...");
    let (toolpath, annotations) = adaptive_3d_toolpath_annotated(&mesh, &si, &cutter, &params);
    eprintln!(
        "toolpath: {} moves, {} annotations",
        toolpath.moves.len(),
        annotations.len()
    );

    let z_partitions = partition_by_z_level(&annotations);
    eprintln!("Z partitions ({} layers):", z_partitions.len());
    for (z, start, end) in &z_partitions {
        eprintln!(
            "  Z={:.2}  moves [{}..{}]  count={}",
            z,
            start,
            end,
            end - start
        );
    }
    assert!(
        z_partitions.len() >= 2,
        "need at least 2 Z layers, got {}",
        z_partitions.len()
    );

    // Headline travel metric: total rapid distance across the whole
    // toolpath (matches the MCP rapid_distance_mm). Region ordering
    // should reduce this without changing cut distance.
    let mut total_rapid = 0.0_f64;
    let mut total_cut = 0.0_f64;
    let mut prev: Option<rs_cam_core::geo::P3> = None;
    for m in &toolpath.moves {
        if let Some(p) = prev {
            let d = ((m.target.x - p.x).powi(2)
                + (m.target.y - p.y).powi(2)
                + (m.target.z - p.z).powi(2))
            .sqrt();
            match m.move_type {
                MoveType::Rapid => total_rapid += d,
                _ => total_cut += d,
            }
        }
        prev = Some(m.target);
    }
    eprintln!(
        "TOTAL: {} moves, cut={:.1}mm, rapid={:.1}mm",
        toolpath.moves.len(),
        total_cut,
        total_rapid
    );

    let first = z_partitions.first().expect("first");
    let last = z_partitions.last().expect("last");

    let bbox = (
        mesh_bbox.min.x,
        mesh_bbox.min.y,
        mesh_bbox.max.x,
        mesh_bbox.max.y,
    );

    let svg_first = render_layer_svg(
        &toolpath,
        first.1,
        first.2,
        first.0,
        bbox,
        "first layer (top, ~pocket)",
    );
    let svg_last = render_layer_svg(
        &toolpath,
        last.1,
        last.2,
        last.0,
        bbox,
        "last layer (bottom, complex 3D)",
    );

    // Write outputs to the workspace target dir via CARGO_MANIFEST_DIR.
    let out_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.join("target"))
        .expect("workspace target dir");
    let first_path = out_dir.join("wanaka_first_z.svg");
    let last_path = out_dir.join("wanaka_last_z.svg");
    let combined_path = out_dir.join("wanaka_first_last_z.svg");
    fs::write(&first_path, &svg_first).expect("write first");
    fs::write(&last_path, &svg_last).expect("write last");
    write_side_by_side(&svg_first, &svg_last, &combined_path);

    eprintln!("wrote {}", first_path.display());
    eprintln!("wrote {}", last_path.display());
    eprintln!("wrote {} (side by side)", combined_path.display());
}
