//! G-INLAYSPLIT — the inlay pocket and the inlay plug are two ops, and the
//! plug fits the pocket at every V angle (ruling R2, W1.4).
//!
//! # The defect (D3)
//!
//! `planning/op_architecture_2026-10-14/AUDIT.md` lists D3: the Inlay op
//! emitted the female pocket AND the male plug in one toolpath, at the same
//! stock top and in the same frame. The simulation therefore cut the plug
//! ring into the workpiece around the design. The flat floor clear planned
//! for a flat end mill that the op did not hold, the plug was not mirrored,
//! and the glue gap was `glue_gap / tan² α`, right only for a 90° bit.
//!
//! # Red proof (claim a, on the code before the split)
//!
//! The first draft of this file held claim (a) only. It ran on the
//! unchanged tree (an L design, 90° bit, default config, 0.2 mm cells):
//! **27 857 cells outside the design were cut, the deepest 3.000 mm**.
//!
//! # The claims
//!
//! - (a) `Inlay` alone cuts no cell outside the design (plus one cell). Its
//!   wall makes the bit angle with the vertical. (With the Finish-role
//!   lead-in this claim failed: 2 089 cells 0.2-0.8 mm outside, 0.067 mm
//!   deep. Both inlay ops now strip leads and entries.) The band at the wall foot
//!   is at the pocket depth. The floor far from the walls stays uncut.
//! - (b) `InlayPlug` on the same design leaves its face (the uncut top)
//!   only inside the MIRRORED design, never in the part of the design that
//!   the mirror moves away.
//! - (c) For 60° and 90° bits, the emitted female wall and the emitted
//!   male wall, after the plug is turned over and seated at its start
//!   depth, are the same line (parallel and coincident), and the plug face
//!   sits exactly `glue_gap` above the pocket floor.
//! - (e) The floor recipe in the Inlay help text works: Inlay plus a flat
//!   Pocket with a Model Outline boundary, offset `-pocket_depth · tan α`,
//!   containment Inside, gives a floor flat to within the glue gap and
//!   walls at the bit angle.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
// The measured numbers ARE the record.
#![allow(clippy::print_stderr)]

mod common;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use common::session::{generate, polygon_model, single_op_session, toolpath_config};
use rs_cam_core::compute::StockConfig;
use rs_cam_core::compute::catalog::OperationConfig;
use rs_cam_core::compute::config::{BoundaryConfig, BoundaryContainment, BoundarySource};
use rs_cam_core::compute::operation_configs::{
    INLAY_GLUE_GAP_DEFAULT_MM, InlayConfig, InlayPlugConfig, PocketConfig,
};
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::geo::{P2, P3};
use rs_cam_core::ops::inlay::{
    InlayPlugParams, InlayPocketParams, floor_inset, inlay_plug_toolpath_with_cancel,
    inlay_pocket_toolpath_with_cancel, mirror_design, plug_start_depth,
};
use rs_cam_core::polygon::Polygon2;
use rs_cam_core::selection::Selection;
use rs_cam_core::session::{
    Command, ProjectSession, ProjectSessionBuilder, SetSimulationResolutionArgs, SimulationOptions,
    SimulationResolution,
};
use rs_cam_core::toolpath::{MoveType, Toolpath};

/// Simulation cell (mm) of claims (a), (b) and (e). Each of their
/// tolerances is one or two cells, and each judged feature is many cells
/// wide (the wall band 12 cells, the floor band 18, the mirrored upright
/// 120), so 0.25 mm resolves every claim. Claim (c) reads the emitted
/// points and needs no simulation.
const CELL_MM: f64 = 0.25;

/// The L: a 60 x 30 bar along the bottom and a 30 x 30 upright on the left.
/// It is not symmetric about its vertical centre line `x = 0`.
fn l_design() -> Polygon2 {
    Polygon2::new(vec![
        P2::new(-30.0, -30.0),
        P2::new(30.0, -30.0),
        P2::new(30.0, 0.0),
        P2::new(0.0, 0.0),
        P2::new(0.0, 30.0),
        P2::new(-30.0, 30.0),
    ])
}

fn board() -> StockConfig {
    StockConfig {
        x: 90.0,
        y: 90.0,
        z: 12.0,
        origin_x: -45.0,
        origin_y: -45.0,
        origin_z: -12.0,
        auto_from_model: false,
        ..StockConfig::default()
    }
}

fn vbit(included_angle_deg: f64) -> ToolConfig {
    ToolConfig {
        diameter: 12.7,
        included_angle: included_angle_deg,
        ..ToolConfig::new_default(ToolId::from_raw(0), ToolType::VBit)
    }
}

fn simulate(session: &mut ProjectSession) -> Arc<TriDexelStock> {
    let _ = session
        .apply(Command::SetSimulationResolution(
            SetSimulationResolutionArgs {
                resolution: SimulationResolution::Fixed(CELL_MM),
            },
        ))
        .expect("a positive cell");
    let cancel = AtomicBool::new(false);
    let _ = session
        .run_simulation(
            &SimulationOptions {
                resolution: rs_cam_core::session::SimulationResolution::Fixed(CELL_MM),
                adaptive_feed_modulation: false,
                ..SimulationOptions::default()
            },
            &cancel,
        )
        .expect("simulation");
    session
        .simulation_result()
        .and_then(|sim| sim.checkpoints.last().map(|c| Arc::clone(&c.mesh_stock)))
        .expect("the simulation carved the ops")
}

/// Every cell of the stock top: `(x, y, z)`.
fn cells(stock: &TriDexelStock) -> Vec<(f64, f64, f64)> {
    let grid = &stock.z_grid;
    let mut out = Vec::new();
    for r in 0..grid.rows {
        for c in 0..grid.cols {
            let (x, y) = grid.cell_to_world(r, c);
            if let Some(top) = grid.top_z_at(r, c) {
                out.push((x, y, f64::from(top)));
            }
        }
    }
    out
}

/// Least squares `y = a + b x`.
fn fit(samples: &[(f64, f64)]) -> (f64, f64) {
    let n = samples.len() as f64;
    let (sx, sy) = samples
        .iter()
        .fold((0.0, 0.0), |(a, b), (x, y)| (a + x, b + y));
    let (mx, my) = (sx / n, sy / n);
    let (mut sxy, mut sxx) = (0.0, 0.0);
    for (x, y) in samples {
        sxy += (x - mx) * (y - my);
        sxx += (x - mx) * (x - mx);
    }
    let b = sxy / sxx;
    (my - b * mx, b)
}

/// Cells along the bottom edge of the L (`y = -30`), away from its ends:
/// `(d, depth)` with `d = y + 30`, the distance in from that edge.
fn bottom_edge_profile(cells: &[(f64, f64, f64)]) -> Vec<(f64, f64)> {
    cells
        .iter()
        .filter(|(x, y, _)| x.abs() < 15.0 && *y > -30.0 && *y < -15.0)
        .map(|(_, y, z)| (y + 30.0, -z))
        .collect()
}

// ── (a) the pocket op cuts only the pocket walls ─────────────────────────

#[test]
fn a_the_inlay_cuts_only_the_pocket_walls() {
    let cfg = InlayConfig::default();
    let half = 45.0_f64.to_radians();
    let mut session = single_op_session(
        board(),
        vbit(90.0),
        polygon_model(vec![l_design()], "l"),
        "inlay",
        OperationConfig::Inlay(cfg.clone()),
    );
    generate(&mut session, 0);
    let stock = simulate(&mut session);
    let all = cells(&stock);
    let design = l_design();

    // Nothing outside the design (plus one cell).
    let mut outside_cut = 0usize;
    let mut worst = 0.0_f64;
    for (x, y, z) in &all {
        let p = P2::new(*x, *y);
        if design.contains_point(&p) || design.distance_to_boundary(&p) <= CELL_MM {
            continue;
        }
        if *z < -0.05 {
            outside_cut += 1;
            worst = worst.max(-z);
        }
    }
    eprintln!("(a) outside cut cells = {outside_cut}, worst = {worst:.3} mm");
    assert_eq!(
        outside_cut, 0,
        "the inlay cut {outside_cut} cells outside the design, worst {worst:.3} mm"
    );

    // The wall makes the bit angle with the vertical.
    let d_n = floor_inset(cfg.pocket_depth, half);
    let profile = bottom_edge_profile(&all);
    let wall: Vec<(f64, f64)> = profile
        .iter()
        .copied()
        .filter(|(d, _)| *d > 0.15 * d_n && *d < 0.85 * d_n)
        .collect();
    assert!(wall.len() > 50, "too few wall cells: {}", wall.len());
    let (a, b) = fit(&wall);
    let angle = (1.0 / b).atan().to_degrees();
    eprintln!("(a) wall fit depth = {a:.4} + {b:.4} d, angle from vertical {angle:.3} deg");
    assert!(
        (angle - 45.0).abs() < 1.0,
        "wall angle {angle:.3} deg, bit half angle 45 deg"
    );
    assert!(
        a.abs() < 2.0 * CELL_MM,
        "the wall starts {a:.3} mm off the outline"
    );

    // The band at the wall foot is at the pocket depth.
    let band: Vec<f64> = profile
        .iter()
        .filter(|(d, _)| *d > d_n + CELL_MM && *d < d_n + cfg.floor_overlap - CELL_MM)
        .map(|(_, t)| *t)
        .collect();
    assert!(!band.is_empty());
    let (lo, hi) = band
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), t| {
            (lo.min(*t), hi.max(*t))
        });
    eprintln!(
        "(a) foot band depth {lo:.3}..{hi:.3} mm (pocket depth {})",
        cfg.pocket_depth
    );
    // The simulated top reads about 0.025 mm shallow of the emitted Z on
    // this grid (the wall fit intercept shows the same offset), so the
    // depth tolerance is 0.05 mm.
    assert!(
        (hi - cfg.pocket_depth).abs() < 0.05,
        "the deepest cut is {hi:.3} mm"
    );
    assert!(
        cfg.pocket_depth - lo < INLAY_GLUE_GAP_DEFAULT_MM,
        "a ridge reaches {lo:.3} mm"
    );

    // The floor far from the walls stays uncut: the op is V walls only.
    let reach = d_n + cfg.floor_overlap + cfg.pocket_depth * half.tan() + CELL_MM;
    let far = profile.iter().filter(|(d, _)| *d > reach).count();
    let far_cut = profile
        .iter()
        .filter(|(d, t)| *d > reach && *t > 0.05)
        .count();
    assert!(far > 0, "vacuous: no floor cell beyond {reach:.2} mm");
    assert_eq!(
        far_cut, 0,
        "the V-bit cut {far_cut} floor cells beyond the band"
    );
}

// ── (b) the plug op cuts the mirrored design ─────────────────────────────

#[test]
fn b_the_plug_face_is_the_mirrored_design() {
    let cfg = InlayPlugConfig::default();
    let half = 45.0_f64.to_radians();
    let mut session = single_op_session(
        board(),
        vbit(90.0),
        polygon_model(vec![l_design()], "l"),
        "plug",
        OperationConfig::InlayPlug(cfg.clone()),
    );
    generate(&mut session, 0);
    let stock = simulate(&mut session);
    let design = l_design();
    let mirror = mirror_design(std::slice::from_ref(&design))
        .pop()
        .expect("one polygon");
    let face_inset = cfg.start_depth() * half.tan();

    let (mut face_in_mirror_only, mut face_in_design_only, mut face_cells) = (0, 0, 0);
    let mut face_outside_mirror = 0;
    for (x, y, z) in cells(&stock) {
        if x.abs() > 30.0 || y.abs() > 30.0 || z < -0.05 {
            continue;
        }
        let p = P2::new(x, y);
        face_cells += 1;
        let in_mirror = mirror.contains_point(&p);
        let in_design = design.contains_point(&p);
        if !in_mirror && mirror.distance_to_boundary(&p) > CELL_MM {
            face_outside_mirror += 1;
        }
        if in_mirror && !in_design {
            face_in_mirror_only += 1;
        }
        if in_design && !in_mirror && mirror.distance_to_boundary(&p) > face_inset + CELL_MM {
            face_in_design_only += 1;
        }
    }
    eprintln!(
        "(b) face cells {face_cells}: outside the mirror {face_outside_mirror}, \
         mirror-only {face_in_mirror_only}, design-only {face_in_design_only}"
    );
    assert!(
        face_in_mirror_only > 1000,
        "vacuous: the mirrored upright has no face"
    );
    assert_eq!(
        face_outside_mirror, 0,
        "the plug face leaves the mirrored design"
    );
    assert_eq!(face_in_design_only, 0, "the plug is not mirrored");
}

// ── (c) the fit at 60° and 90° ───────────────────────────────────────────

/// Points on the emitted cutting path: every feed segment whose two ends
/// are below the top, sampled every 0.05 mm. The kernel drops collinear
/// samples, so the move targets alone do not show a straight wall.
fn cut_points(tp: &Toolpath) -> Vec<P3> {
    let mut out = Vec::new();
    for pair in tp.moves.windows(2) {
        let (a, b) = (pair[0].target, pair[1].target);
        if !matches!(pair[1].move_type, MoveType::Linear { .. }) || a.z >= -1e-9 || b.z >= -1e-9 {
            continue;
        }
        let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2) + (b.z - a.z).powi(2)).sqrt();
        let n = (len / 0.05).ceil().max(1.0) as usize;
        for k in 0..=n {
            let t = k as f64 / n as f64;
            out.push(P3::new(
                a.x + t * (b.x - a.x),
                a.y + t * (b.y - a.y),
                a.z + t * (b.z - a.z),
            ));
        }
    }
    out
}

fn fit_case(included_angle_deg: f64) {
    let half = (included_angle_deg / 2.0).to_radians();
    let tan_a = half.tan();
    let pocket_depth = 3.0;
    let glue_gap = 0.15;
    let square = Polygon2::rectangle(-20.0, -20.0, 20.0, 20.0);

    let female = rs_cam_core::interrupt::run_uncancellable(|cancel| {
        inlay_pocket_toolpath_with_cancel(
            &square,
            &InlayPocketParams {
                half_angle: half,
                pocket_depth,
                floor_overlap: 1.0,
                floor_ridge: 0.05,
                tip_flat: 0.0,
                stepover: 0.25,
                feed_rate: 1000.0,
                plunge_rate: 500.0,
                safe_z: 10.0,
                tolerance: 0.05,
                top_z: 0.0,
            },
            cancel,
        )
    });
    let plug_params = InlayPlugParams {
        half_angle: half,
        pocket_depth,
        glue_gap,
        proud_height: 2.0,
        outer_margin: 2.0,
        depth_per_pass: 10.0,
        stepover: 0.25,
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: 10.0,
        tolerance: 0.05,
        top_z: 0.0,
    };
    let male = rs_cam_core::interrupt::run_uncancellable(|cancel| {
        inlay_plug_toolpath_with_cancel(std::slice::from_ref(&square), &plug_params, cancel)
    });
    let start = plug_start_depth(pocket_depth, glue_gap);
    let d_n = pocket_depth * tan_a;

    // The female wall near the bottom edge: (d inside, depth t).
    let f_wall: Vec<(f64, f64)> = cut_points(&female)
        .iter()
        .filter(|p| p.x.abs() < 10.0 && p.y < 0.0)
        .map(|p| (p.y + 20.0, -p.z))
        .filter(|(d, _)| *d > 0.1 * d_n && *d < 0.9 * d_n)
        .collect();
    let floor = cut_points(&female)
        .iter()
        .map(|p| -p.z)
        .fold(0.0_f64, f64::max);

    // The male wall near the bottom edge, turned over and seated: a plug
    // point at depth `u` and signed distance `e` (positive outside) lands
    // at the pocket depth `t = S - u`, `-e` inside the outline.
    let m_pts = cut_points(&male);
    let m_wall: Vec<(f64, f64)> = m_pts
        .iter()
        .filter(|p| p.x.abs() < 10.0 && p.y < 0.0 && p.y > -30.0)
        .map(|p| {
            let e = -20.0 - p.y; // outside the bottom edge is y < -20
            (-e, start - (-p.z))
        })
        .filter(|(d, t)| *d > 0.1 * start * tan_a && *d < 0.9 * start * tan_a && *t > 0.0)
        .collect();
    // The plug face: no cut more than one sample inside the face edge.
    let face_cut = m_pts
        .iter()
        .filter(|p| square.contains_point(&P2::new(p.x, p.y)))
        .filter(|p| square.distance_to_boundary(&P2::new(p.x, p.y)) > start * tan_a + 0.06)
        .count();

    assert!(
        f_wall.len() > 50 && m_wall.len() > 50,
        "vacuous wall samples"
    );
    let (fa, fb) = fit(&f_wall);
    let (ma, mb) = fit(&m_wall);
    let plug_face_t = start; // the uncut plug face, turned over
    let gap = floor - plug_face_t;
    eprintln!(
        "(c) {included_angle_deg} deg: female t = {fa:.6} + {fb:.6} d, male t = {ma:.6} + {mb:.6} d, \
         1/tan a = {:.6}; floor {floor:.6}, plug face {plug_face_t:.6}, gap {gap:.6}",
        1.0 / tan_a
    );
    assert!(
        (fb - 1.0 / tan_a).abs() < 1e-6,
        "the female wall is not at the bit angle"
    );
    assert!((mb - fb).abs() < 1e-6, "the walls are not parallel");
    assert!(
        (ma - fa).abs() < 1e-6,
        "the walls are parallel but not coincident"
    );
    assert!(
        fa.abs() < 1e-6,
        "the female wall does not start on the outline"
    );
    assert_eq!(face_cut, 0, "the plug op cut its own face");
    assert!(
        (floor - pocket_depth).abs() < 1e-9,
        "the pocket floor is at {floor}"
    );
    assert!(
        (gap - glue_gap).abs() < 1e-9,
        "the glue gap is {gap}, not {glue_gap}"
    );
    // The deepest plug cut is the start depth plus the proud height.
    let plug_floor = m_pts.iter().map(|p| -p.z).fold(0.0_f64, f64::max);
    assert!((plug_floor - (start + plug_params.proud_height)).abs() < 1e-9);
}

#[test]
fn c_the_plug_fits_the_pocket_with_a_60_degree_bit() {
    fit_case(60.0);
}

#[test]
fn c_the_plug_fits_the_pocket_with_a_90_degree_bit() {
    fit_case(90.0);
}

// ── (e) the floor recipe of the help text ────────────────────────────────

#[test]
fn e_inlay_plus_the_clipped_floor_pocket_make_the_whole_pocket() {
    let inlay = InlayConfig::default();
    let half = 45.0_f64.to_radians();
    let d_n = floor_inset(inlay.pocket_depth, half);

    let mut builder = ProjectSessionBuilder::new().stock(board());
    let vbit_idx = builder.add_tool(vbit(90.0));
    let vbit_id = builder.tools()[vbit_idx].id.raw();
    let mill_idx = builder.add_tool(ToolConfig {
        diameter: 4.0,
        ..ToolConfig::new_default(ToolId::from_raw(1), ToolType::EndMill)
    });
    let mill_id = builder.tools()[mill_idx].id.raw();
    let model_id = builder.add_model(polygon_model(vec![l_design()], "l"));

    // The floor Pocket, as the Inlay help text says.
    let pocket = PocketConfig {
        floor: rs_cam_core::compute::floor::Floor::top(inlay.pocket_depth),
        depth_per_pass: 1.5,
        stepover: 1.6,
        ..PocketConfig::default()
    };
    let mut floor = toolpath_config(
        "floor",
        OperationConfig::Pocket(pocket),
        rs_cam_core::ids::ToolId::from_raw(mill_id),
        model_id,
    );
    floor.boundary_inherit = false;
    floor.boundary = BoundaryConfig {
        enabled: true,
        source: BoundarySource::Selection {
            selection: Selection::closed_shapes_of(model_id),
            holes: false,
        },
        containment: BoundaryContainment::Inside,
        offset: -d_n,
    };
    let _ = builder
        .add_toolpath(0, floor)
        .expect("add the floor pocket");
    let walls = toolpath_config(
        "walls",
        OperationConfig::Inlay(inlay.clone()),
        rs_cam_core::ids::ToolId::from_raw(vbit_id),
        model_id,
    );
    let _ = builder.add_toolpath(0, walls).expect("add the inlay");
    let mut session = builder.build();
    generate(&mut session, 0);
    generate(&mut session, 1);
    let stock = simulate(&mut session);

    let design = l_design();
    let (mut floor_cells, mut floor_high, mut worst_floor) = (0usize, 0usize, 0.0_f64);
    let (mut wall_cells, mut wall_bad, mut worst_wall) = (0usize, 0usize, 0.0_f64);
    let mut deeper = 0usize;
    for (x, y, z) in cells(&stock) {
        let p = P2::new(x, y);
        if !design.contains_point(&p) {
            continue;
        }
        let d = design.distance_to_boundary(&p);
        let t = -z;
        if t > inlay.pocket_depth + 0.02 {
            deeper += 1;
        }
        if d > d_n + CELL_MM {
            floor_cells += 1;
            let high = inlay.pocket_depth - t;
            worst_floor = worst_floor.max(high);
            if high >= INLAY_GLUE_GAP_DEFAULT_MM {
                floor_high += 1;
            }
        } else if d < d_n - CELL_MM && d > CELL_MM {
            // Judge the wall away from the convex corners, where the
            // distance to the outline is not the distance to one edge.
            let near_corner = [
                (-30.0, -30.0),
                (30.0, -30.0),
                (30.0, 0.0),
                (0.0, 30.0),
                (-30.0, 30.0),
            ]
            .iter()
            .any(|(cx, cy)| ((x - cx).powi(2) + (y - cy).powi(2)).sqrt() < 2.0 * d_n);
            if near_corner {
                continue;
            }
            wall_cells += 1;
            let err = (t - d / half.tan()).abs();
            worst_wall = worst_wall.max(err);
            if err > 2.0 * CELL_MM {
                wall_bad += 1;
            }
        }
    }
    eprintln!(
        "(e) floor cells {floor_cells}, {floor_high} at or above the glue gap, worst {worst_floor:.3} mm; \
         wall cells {wall_cells}, {wall_bad} off the bit line, worst {worst_wall:.3} mm; deeper {deeper}"
    );
    assert!(
        floor_cells > 10_000 && wall_cells > 1_000,
        "vacuous fixture"
    );
    assert_eq!(deeper, 0, "a cell is below the pocket floor");
    assert_eq!(
        floor_high, 0,
        "{floor_high} floor cells stand at or above the glue gap (worst {worst_floor:.3} mm): \
         the plug face would land on them"
    );
    assert_eq!(
        wall_bad, 0,
        "the floor Pocket cut into the V wall (worst {worst_wall:.3} mm)"
    );
}
