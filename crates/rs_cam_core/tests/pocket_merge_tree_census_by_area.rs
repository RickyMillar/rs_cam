//! Research probe: the pockets that a join tree finds on the tool-CL
//! surface of a 3D Rough, against the regions that By Area finds today.
//!
//! # The question
//!
//! On terrain (rivmap100) the By Area detector
//! (`adaptive3d::clearing::detect_material_regions_labeled`) finds one
//! region, because it flood-fills the remaining material and all of it
//! connects. The operator wants By Area to find the real pockets: every
//! valley that the roughing tool fits into, fewer than about ten of them,
//! cut depth-first. This probe measures what
//! `surface::merge_tree::build_pocket_tree` finds for a grid of dials.
//!
//! # What it does
//!
//! 1. It loads the project and walks the generation plan up to the first
//!    3D Rough (generate, simulate, generate), as the CLI does.
//! 2. It reads the By Area map of that result: the current detector.
//! 3. It builds the drop-cutter height grid of the rough's tool with
//!    `SurfaceHeightmap::from_mesh`, on the grid that `adaptive3d/path.rs`
//!    builds: cell `max(engagement radius / 6, tolerance)`, the mesh box
//!    plus the radius joined with the stock box, floor at the mesh bottom.
//!    Cells that the mesh does not cover are masked.
//! 4. It runs the tree for each `(h, min_area)` pair and writes a PNG top
//!    view and a CSV per pair, plus `summary.csv`.
//!
//! The PNGs carry colour only. `annotate.py` (in the output folder, not in
//! the repo) prints the pocket ids and the cut order from the CSVs.
//!
//! # How to run
//!
//! `cargo test -p rs_cam_core -q --test pocket_merge_tree_census_by_area
//! -- --ignored`. Set `MERGE_TREE_PROJECT` and `MERGE_TREE_OUT` to change
//! the project and the output folder. The probe asserts nothing about the
//! counts: it is an instrument, not a gate.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::cutter::build_cutter;
use rs_cam_core::mesh::SpatialIndex;
use rs_cam_core::session::generation_plan::{Scope, Step, plan};
use rs_cam_core::session::{
    Command, ProjectSession, SetSimulationResolutionArgs, SimulationOptions, SimulationResolution,
};
use rs_cam_core::surface::flow_accum::FlowField;
use rs_cam_core::surface::merge_tree::{MergeTreeParams, PocketTree, build_pocket_tree};
use rs_cam_core::surface::slope::SurfaceHeightmap;
use rs_cam_core::tool::MillingCutter;

const DEFAULT_PROJECT: &str = "/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/61c87c01-64ec-4b8d-a255-a09cc23316a1/scratchpad/demo/rivmap100_ladder_demo.toml";
const DEFAULT_OUT: &str = "/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/61c87c01-64ec-4b8d-a255-a09cc23316a1/scratchpad/merge_tree";
/// The simulation cell of the plan walk (the rough-score default).
const SIM_CELL_MM: f64 = 0.5;
const H_MM: [f64; 4] = [1.0, 2.0, 4.0, 8.0];
const AREA_MM2: [f64; 3] = [100.0, 400.0, 1600.0];
/// Pixels per grid cell in the PNGs.
const PX: u32 = 3;

fn walk_to(session: &mut ProjectSession, rough_index: usize) {
    let cancel = AtomicBool::new(false);
    let rough_id = session.toolpath_configs()[rough_index].id;
    for step in plan(session, Scope::Ancestors(rough_id)) {
        match step {
            Step::Simulate { .. } => {
                let opts = SimulationOptions {
                    resolution: SIM_CELL_MM,
                    metrics_enabled: false,
                    adaptive_feed_modulation: false,
                    ..SimulationOptions::default()
                };
                let _ = session.run_simulation(&opts, &cancel).expect("simulate");
            }
            Step::Generate { index, .. } => {
                session
                    .generate_toolpath(index, &cancel)
                    .unwrap_or_else(|e| panic!("generate {index}: {e}"));
            }
        }
    }
}

/// A distinct colour per pocket id (golden-angle hue walk).
fn pocket_rgb(id: usize) -> [f64; 3] {
    let h = (id as f64 * 137.508) % 360.0;
    let (s, v) = (0.65, 0.95);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [r + m, g + m, b + m]
}

/// The grid a label image is drawn on.
struct Canvas<'a> {
    nx: usize,
    ny: usize,
    z: &'a [f64],
    masked: &'a [bool],
    z_lo: f64,
    z_hi: f64,
}

/// Draw `label` (one entry per cell) as a top view: one colour per label,
/// shaded by Z, a black line at each label change, grey where no label.
/// Row 0 is the bottom of the image (world Y up).
fn render(canvas: &Canvas<'_>, label: &dyn Fn(usize) -> Option<usize>, path: &PathBuf) {
    let (nx, ny) = (canvas.nx, canvas.ny);
    let mut img = image::RgbImage::new(nx as u32 * PX, ny as u32 * PX);
    let span = (canvas.z_hi - canvas.z_lo).max(1e-9);
    for r in 0..ny {
        for c in 0..nx {
            let i = r * nx + c;
            let shade = 0.45 + 0.55 * ((canvas.z[i] - canvas.z_lo) / span).clamp(0.0, 1.0);
            let base = match label(i) {
                _ if canvas.masked[i] => [0.25, 0.25, 0.25],
                None => [0.7, 0.7, 0.7],
                Some(id) => pocket_rgb(id),
            };
            let rgb = image::Rgb(base.map(|v| (v * shade * 255.0).round() as u8));
            let edge = |j: usize| label(j) != label(i);
            let right = c + 1 < nx && edge(i + 1);
            let up = r + 1 < ny && edge(i + nx);
            for dy in 0..PX {
                for dx in 0..PX {
                    let px = c as u32 * PX + dx;
                    let py = (ny - 1 - r) as u32 * PX + (PX - 1 - dy);
                    let on_edge = (right && dx == PX - 1) || (up && dy == PX - 1);
                    let v = if on_edge { image::Rgb([0, 0, 0]) } else { rgb };
                    img.put_pixel(px, py, v);
                }
            }
        }
    }
    img.save(path).expect("write png");
}

/// The labelled cell of `id` nearest to the centroid of its cells, as image
/// pixel coordinates (the centre of the cell).
fn anchor_px(labels: &[Option<usize>], nx: usize, ny: usize, id: usize) -> (u32, u32) {
    let cells: Vec<usize> = (0..labels.len())
        .filter(|&i| labels[i] == Some(id))
        .collect();
    let n = cells.len().max(1) as f64;
    let cx = cells.iter().map(|&i| (i % nx) as f64).sum::<f64>() / n;
    let cy = cells.iter().map(|&i| (i / nx) as f64).sum::<f64>() / n;
    let best = cells
        .iter()
        .copied()
        .min_by(|&a, &b| {
            let da = ((a % nx) as f64 - cx).powi(2) + ((a / nx) as f64 - cy).powi(2);
            let db = ((b % nx) as f64 - cx).powi(2) + ((b / nx) as f64 - cy).powi(2);
            da.total_cmp(&db)
        })
        .unwrap_or(0);
    let (c, r) = (best % nx, best / nx);
    (c as u32 * PX + PX / 2, (ny - 1 - r) as u32 * PX + PX / 2)
}

fn pocket_csv(tree: &PocketTree, nx: usize, ny: usize) -> String {
    let order = tree.cut_order();
    let mut s = String::from(
        "id,cut_order,parent,children,min_z,saddle_z,depth_mm,area_mm2,basin_area_mm2,\
         x_min,y_min,x_max,y_max,cells,label_px_x,label_px_y\n",
    );
    for p in &tree.pockets {
        let rank = order.iter().position(|&o| o == p.id).map_or(0, |k| k + 1);
        let (px, py) = anchor_px(&tree.labels, nx, ny, p.id);
        let kids: Vec<String> = p.children.iter().map(ToString::to_string).collect();
        let _ = writeln!(
            s,
            "{},{},{},{},{:.3},{:.3},{:.3},{:.1},{:.1},{:.2},{:.2},{:.2},{:.2},{},{},{}",
            p.id,
            rank,
            p.parent.map_or(String::new(), |q| q.to_string()),
            kids.join(" "),
            p.min_z,
            p.saddle_z,
            p.depth_mm,
            p.area_mm2,
            p.basin_area_mm2,
            p.bbox_xy[0],
            p.bbox_xy[1],
            p.bbox_xy[2],
            p.bbox_xy[3],
            p.cell_count,
            px,
            py,
        );
    }
    s
}

#[test]
#[ignore = "research probe; reads the rivmap100 copy in the scratchpad"]
fn pocket_merge_tree_census_on_the_rough() {
    let project = PathBuf::from(
        std::env::var("MERGE_TREE_PROJECT").unwrap_or_else(|_| DEFAULT_PROJECT.to_owned()),
    );
    let out =
        PathBuf::from(std::env::var("MERGE_TREE_OUT").unwrap_or_else(|_| DEFAULT_OUT.to_owned()));
    std::fs::create_dir_all(&out).expect("output folder");
    let mut notes = String::new();

    // 1. The session and the plan walk up to the rough.
    let mut session = ProjectSession::load(&project).expect("load project");
    let _ = session
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(SIM_CELL_MM),
            },
        ))
        .expect("a positive cell");
    let rough_index = session
        .toolpath_configs()
        .iter()
        .position(|tc| tc.enabled && matches!(tc.operation, OperationConfig::Adaptive3d(_)))
        .expect("the project has an enabled 3D Rough");
    let tc = session.toolpath_configs()[rough_index].clone();
    let OperationConfig::Adaptive3d(cfg) = &tc.operation else {
        unreachable!("matched above");
    };
    let t_walk = Instant::now();
    walk_to(&mut session, rough_index);
    let walk_s = t_walk.elapsed().as_secs_f64();

    // 2. The current detector.
    let area = session
        .get_result(rough_index)
        .expect("the rough generated")
        .annotated()
        .area_regions
        .clone();

    // 3. The tool-CL height grid, as adaptive3d/path.rs builds it.
    let tool = session
        .tools()
        .iter()
        .find(|t| t.id.0 == tc.tool_id)
        .expect("the rough's tool");
    let cutter = build_cutter(tool);
    let mesh = session
        .models()
        .iter()
        .find(|m| m.id == tc.model_id)
        .and_then(|m| m.mesh.clone())
        .expect("the rough's mesh");
    let index = SpatialIndex::build_auto(&mesh);
    let r = cutter.engagement_radius_mm(cfg.depth_per_pass).max(0.01);
    let cell = (r / 6.0).max(cfg.tolerance);
    let stock = session.stock_bbox();
    let bb = &mesh.bbox;
    let x0 = (bb.min.x - r).min(stock.min.x);
    let y0 = (bb.min.y - r).min(stock.min.y);
    let x1 = (bb.max.x + r).max(stock.max.x);
    let y1 = (bb.max.y + r).max(stock.max.y);
    let cols = ((x1 - x0) / cell).ceil() as usize + 1;
    let rows = ((y1 - y0) / cell).ceil() as usize + 1;
    let t_hm = Instant::now();
    let hm =
        SurfaceHeightmap::from_mesh(&mesh, &index, &cutter, x0, y0, rows, cols, cell, bb.min.z);
    let hm_ms = t_hm.elapsed().as_secs_f64() * 1e3;
    let masked: Vec<bool> = hm.covered_flags().iter().map(|c| !c).collect();
    let field = FlowField {
        nx: cols,
        ny: rows,
        ox: x0,
        oy: y0,
        cell,
        z: hm.z_or_bbox_floor_values().to_vec(),
        nodata: masked.clone(),
    };
    // The material top: the By Area map records the highest material top
    // over its cells (the faced stock); without a map, the stock box top.
    let top_z = area.as_ref().map_or(stock.max.z, |a| a.top_z);
    let covered_z: Vec<f64> = (0..field.len())
        .filter(|&i| !masked[i])
        .map(|i| field.z[i])
        .collect();
    let z_lo = covered_z.iter().copied().fold(f64::INFINITY, f64::min);
    let z_hi = covered_z.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let _ = writeln!(
        notes,
        "project,{}\nrough,{} (index {rough_index})\ntool,{} D{} r_eng {r:.3}\n\
         grid,{cols}x{rows} cell {cell:.3} mm origin ({x0:.3} {y0:.3})\n\
         heightmap_build_ms,{hm_ms:.0}\nplan_walk_s,{walk_s:.1}\n\
         mesh_z,{:.3} .. {:.3}\ncl_z_covered,{z_lo:.3} .. {z_hi:.3}\n\
         stock_box_z,{:.3} .. {:.3}\ntop_z_used,{top_z:.3}\nmasked_cells,{}",
        project.display(),
        tc.name,
        tool.name,
        tool.diameter,
        bb.min.z,
        bb.max.z,
        stock.min.z,
        stock.max.z,
        masked.iter().filter(|m| **m).count(),
    );

    let canvas = Canvas {
        nx: cols,
        ny: rows,
        z: &field.z,
        masked: &masked,
        z_lo,
        z_hi,
    };

    // 4. The current detector's regions, on its own grid.
    if let Some(a) = area.as_ref() {
        let _ = writeln!(
            notes,
            "by_area_grid,{}x{} cell {:.3} origin ({:.3} {:.3}) top_z {:.3}\nby_area_regions,{}",
            a.cols,
            a.rows,
            a.cell_mm,
            a.origin_x,
            a.origin_y,
            a.top_z,
            a.regions.len()
        );
        let mut s =
            String::from("order,cells,area_mm2,x_min,y_min,x_max,y_max,surf_z_min,surf_z_max\n");
        for reg in &a.regions {
            let _ = writeln!(
                s,
                "{},{},{:.1},{:.2},{:.2},{:.2},{:.2},{:.3},{:.3}",
                reg.order,
                reg.cell_count,
                reg.cell_count as f64 * a.cell_mm * a.cell_mm,
                reg.bbox_xy[0],
                reg.bbox_xy[1],
                reg.bbox_xy[2],
                reg.bbox_xy[3],
                reg.surface_z_range[0],
                reg.surface_z_range[1],
            );
        }
        std::fs::write(out.join("current_by_area_regions.csv"), s).expect("write csv");
        if a.rows == rows && a.cols == cols {
            let labels = &a.labels;
            render(
                &canvas,
                &|i| (labels[i] > 0).then(|| usize::from(labels[i])),
                &out.join("current_by_area_regions.png"),
            );
        } else {
            let _ = writeln!(notes, "by_area_grid_differs,no png");
        }
    } else {
        let _ = writeln!(notes, "by_area_regions,none (the rough is not By Area)");
    }

    // 5. The sweep.
    let mut summary = String::from(
        "h_mm,min_area_mm2,pockets,leaves,internal,roots,leaf_area_mm2,material_area_mm2,\
         leaf_depth_min,leaf_depth_max,root_depth,build_ms,png,csv\n",
    );
    let mut run = |h: f64, a: f64, field: &FlowField, canvas: &Canvas<'_>, tag: &str| {
        let t = Instant::now();
        let tree = build_pocket_tree(
            field,
            top_z,
            &MergeTreeParams {
                persistence_h_mm: h,
                min_area_mm2: a,
            },
        );
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let name = format!("pockets_h{h}_a{a}{tag}");
        let png = out.join(format!("{name}.png"));
        let csv = out.join(format!("{name}.csv"));
        render(canvas, &|i| tree.labels[i], &png);
        std::fs::write(&csv, pocket_csv(&tree, field.nx, field.ny)).expect("write csv");
        let leaves: Vec<_> = tree
            .pockets
            .iter()
            .filter(|p| p.children.is_empty())
            .collect();
        let roots = tree.roots();
        let leaf_area: f64 = leaves.iter().map(|p| p.area_mm2).sum();
        let all_area: f64 = tree.pockets.iter().map(|p| p.area_mm2).sum();
        let non_root: Vec<_> = tree.pockets.iter().filter(|p| p.parent.is_some()).collect();
        let dmin = non_root
            .iter()
            .map(|p| p.depth_mm)
            .fold(f64::INFINITY, f64::min);
        let dmax = non_root
            .iter()
            .map(|p| p.depth_mm)
            .fold(f64::NEG_INFINITY, f64::max);
        let root_depth = roots
            .first()
            .and_then(|&r| tree.pockets.get(r))
            .map_or(0.0, |p| p.depth_mm);
        let _ = writeln!(
            summary,
            "{h},{a},{},{},{},{},{leaf_area:.0},{all_area:.0},{dmin:.2},{dmax:.2},{root_depth:.2},{ms:.1},{},{}",
            tree.pockets.len(),
            leaves.len(),
            tree.pockets.len() - leaves.len(),
            roots.len(),
            png.display(),
            csv.display(),
        );
    };
    for h in H_MM {
        for a in AREA_MM2 {
            run(h, a, &field, &canvas, "");
        }
    }

    // One run at half the cell, to show how the count moves with the grid.
    let fine = cell / 2.0;
    let fcols = ((x1 - x0) / fine).ceil() as usize + 1;
    let frows = ((y1 - y0) / fine).ceil() as usize + 1;
    let t_fine = Instant::now();
    let fhm =
        SurfaceHeightmap::from_mesh(&mesh, &index, &cutter, x0, y0, frows, fcols, fine, bb.min.z);
    let fine_ms = t_fine.elapsed().as_secs_f64() * 1e3;
    let fmasked: Vec<bool> = fhm.covered_flags().iter().map(|c| !c).collect();
    let ffield = FlowField {
        nx: fcols,
        ny: frows,
        ox: x0,
        oy: y0,
        cell: fine,
        z: fhm.z_or_bbox_floor_values().to_vec(),
        nodata: fmasked.clone(),
    };
    let fcanvas = Canvas {
        nx: fcols,
        ny: frows,
        z: &ffield.z,
        masked: &fmasked,
        z_lo,
        z_hi,
    };
    let _ = writeln!(
        notes,
        "fine_grid,{fcols}x{frows} cell {fine:.3} build_ms {fine_ms:.0}"
    );
    run(2.0, 400.0, &ffield, &fcanvas, "_fine");

    std::fs::write(out.join("summary.csv"), summary).expect("write summary");
    std::fs::write(out.join("notes.csv"), notes).expect("write notes");
}
