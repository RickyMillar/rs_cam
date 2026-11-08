//! C12 of the adaptive research 2026-10-23 (integration step 7.7): the
//! research judge of the 3D geodesic arm, moved onto the product strategy
//! `ClearingStrategy3d::GeodesicAdaptive`. No `research` feature and no
//! environment variable: the judge reads only the product output (the
//! toolpath, its annotations and `Move::feed_cap`).
//!
//! The research file (`tests/adaptive3d_geodesic_arm_c12.rs` of 3682d1dcc)
//! judged eight option sets with the planner's research records (turn and
//! departure summaries, entry causes, joins, openings, core zones, split
//! and route counters). Those records were planner instruments and were
//! deleted in step 7.4a. The step 7.7 report lists each dropped test and
//! assertion with its reason. The notes are
//! `planning/adaptive_research_2026-10-23/P3_C12_geodesic_3d_arm.md`,
//! `P3_C18_3d_oracle.md` and `P3_C22_3d_turns.md`.
//!
//! ## The fixture
//!
//! A height field over `[-40, 40]²` at a 0.5 mm step: a rim at Z 10, one
//! rectangular pocket `|x| <= 34, |y| <= 24` with its floor at Z 4, and one
//! round island of radius 5 at the origin, its top at Z 10. A 6 mm flat end
//! mill, stepover 2, tolerance 0.1, Depth/Pass 3, stock-to-leave 0.5,
//! stock top Z 10: the levels are Z 7 and the floor level (Z 4.5). The step
//! split and the keep-down route are the strategy's own (`None`:
//! `StockDepth`, `CutFeed`) unless a test sets them.
//!
//! ## The oracle (C18, C43)
//!
//! The dexel simulation of the emitted toolpath (cell 0.5 mm, metrics on)
//! screens the samples; the exact load of the emitted move judges them. A
//! judged sample is read on the exact geometry ([`exact_width_at`], a
//! height lattice) when the simulation reads its radial, or its box
//! `radial · axial_engagement_mm / d`, over the screen bound
//! `ceiling + (planner cell + sim cell + tolerance) / D` (0.546). It fails
//! only when the exact load peak of its move is over the exact bound
//! `ceiling + (planner cell + tolerance + exact pitch) / D` (0.471). The
//! load is the codebase's one load `L`
//! (`tool_load::load_fraction::chip_load_fraction`) over the sideways
//! strips of the disc, each strip at the deepest stock over the cutter in
//! it (step 7.7: the research judge summed its own per-strip chip area).
//! The ceiling is `radial_woc_fraction_from_leading_arc(
//! pass_engagement_limit(s, R))` (0.3626 at s 2, R 3).
//!
//! The cutting samples the oracle does not judge, each counted:
//!
//! - Entry moves (`EntryPlunge`, `EntryHelix`, `EntryRamp`).
//! - Vertical moves (no XY travel).
//! - Capped moves (`Move::feed_cap`): the planned clearing turns and the
//!   plunge departures. The planner admits them over the ceiling and
//!   lowers their feed to hold the removal rate (design part F); they are
//!   judged by the feed rule (feed at or under the cap, cap under the
//!   commanded feed), not by the load bound.
//! - The waterline cleanup of each level (its `WaterlineCleanup`
//!   annotation up to the next level annotation): the shipped wall pass.
//!   Printed, not asserted.
//! - The departure of a straight plunge ([`departing_moves`]), as the 2D
//!   walker exempts it.
//! - The perimeter-sweep band, for a control on the slice pipeline only
//!   (AgentSearch until removal step R4; no strategy left takes it): the
//!   product strategy cuts no perimeter sweep where its 2D planner gets the
//!   grown region (C31), so it is judged in the band.

#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::print_stderr,
    reason = "test code: a panic is how a test reports a failure; the test prints its measurements for the reader"
)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::SQRT_2;

use common::meshes::height_field;

use rs_cam_core::adaptive::pass_engagement_limit;
use rs_cam_core::adaptive3d::{
    Adaptive3dDepth, Adaptive3dGeometry, Adaptive3dLinking, Adaptive3dOutput, Adaptive3dParams,
    Adaptive3dRuntimeEvent, ClearingStrategy3d, EntryStyle3d, KeepDownRoute, RegionOrdering,
    StepSplit, adaptive_3d_toolpath_output_with_cancel,
};
use rs_cam_core::dexel_stock::{StockCutDirection, TriDexelStock};
use rs_cam_core::geo::P3;
use rs_cam_core::ids::ToolpathId;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::ops::adaptive_shared::radial_woc_fraction_from_leading_arc;
use rs_cam_core::stock::simulation_cut::SimulationCutSample;
use rs_cam_core::tool::{FlatEndmill, MillingCutter};
use rs_cam_core::tool_load::load_fraction::chip_load_fraction;
use rs_cam_core::toolpath::{Move, MoveIntent, MoveType, Toolpath};

const TOOL_RADIUS: f64 = 3.0;
const STEPOVER: f64 = 2.0;
const TOLERANCE: f64 = 0.1;
const DEPTH_PER_PASS: f64 = 3.0;
const STOCK_TO_LEAVE: f64 = 0.5;
const TOP_Z: f64 = 10.0;
const FLOOR_Z: f64 = 4.0;
const MESH_HALF: f64 = 40.0;
const MESH_STEP: f64 = 0.5;
const POCKET_HALF_X: f64 = 34.0;
const POCKET_HALF_Y: f64 = 24.0;
const ISLAND_RADIUS: f64 = 5.0;
/// The simulation cell and sample step, mm (`common::adaptive_islands`).
const SIM_CELL: f64 = 0.5;
/// Lattice pitch (mm) of the exact reading: a tenth of the simulation
/// cell, as in `a_clearing_cut_holds_the_pass_load_g_adaptpassload`.
const EXACT_PITCH_MM: f64 = SIM_CELL / 10.0;
/// The inset margin of the perimeter sweep beyond the tool radius
/// (`adaptive3d/clearing.rs`, `PERIMETER_INSET_MARGIN_MM`).
const PERIMETER_INSET_MARGIN_MM: f64 = 0.25;
/// The shelf of the width-read fixture: x 12 to 30, y -12 to 12.
const SHELF_Z: f64 = 6.5;
/// The channel fixture: half length, and the shipped region smoothing
/// inset (`adaptive3d/clearing.rs`, `SIMPLIFY_INSET`) its width is built
/// from.
const CHANNEL_HALF_LENGTH: f64 = 30.0;
const SIMPLIFY_INSET: f64 = 0.3;

/// The pocket plate: a rim at Z 10, the pocket floor at Z 4, the island top
/// at Z 10.
fn pocket_with_island() -> TriangleMesh {
    height_field(MESH_HALF, MESH_STEP, |x, y| {
        if x.hypot(y) <= ISLAND_RADIUS {
            TOP_Z
        } else if x.abs() <= POCKET_HALF_X && y.abs() <= POCKET_HALF_Y {
            FLOOR_Z
        } else {
            TOP_Z
        }
    })
}

/// The pocket with a shelf at Z 6.5 on its +x side (§16.9, D30).
fn shelf() -> TriangleMesh {
    height_field(MESH_HALF, MESH_STEP, |x, y| {
        if x.hypot(y) <= ISLAND_RADIUS {
            TOP_Z
        } else if (12.0..=30.0).contains(&x) && y.abs() <= 12.0 {
            SHELF_Z
        } else if x.abs() <= POCKET_HALF_X && y.abs() <= POCKET_HALF_Y {
            FLOOR_Z
        } else {
            TOP_Z
        }
    })
}

/// §19.3 (D43): the pocket with no island and a flat face 1.5 mm above the
/// floor over its middle third (`|x| <= 11`).
fn stepped_face() -> TriangleMesh {
    height_field(MESH_HALF, MESH_STEP, |x, y| {
        if x.abs() <= POCKET_HALF_X && y.abs() <= POCKET_HALF_Y {
            if x.abs() <= 11.0 {
                FLOOR_Z + 1.5
            } else {
                FLOOR_Z
            }
        } else {
            TOP_Z
        }
    })
}

fn channel_half_width() -> f64 {
    (2.0 * TOOL_RADIUS + 2.0 * (SIMPLIFY_INSET + planner_cell()) + STEPOVER) / 2.0
}

/// A straight channel about one stepover of tool-centre width, too narrow
/// for the field's seed (a centre clearance of R).
fn channel() -> TriangleMesh {
    let hw = channel_half_width();
    height_field(MESH_HALF, MESH_STEP, |x, y| {
        if x.abs() <= CHANNEL_HALF_LENGTH && y.abs() <= hw {
            FLOOR_Z
        } else {
            TOP_Z
        }
    })
}

/// The planner cell of both the 3D stock and the 2D slice planner:
/// `max(R / 6, tolerance)` (`adaptive/path.rs`, `planner_cell_size`).
fn planner_cell() -> f64 {
    (TOOL_RADIUS / 6.0).max(TOLERANCE)
}

/// The pass ceiling as a radial fraction (0.3626 at s 2, R 3).
fn ceiling() -> f64 {
    radial_woc_fraction_from_leading_arc(pass_engagement_limit(STEPOVER, TOOL_RADIUS))
}

/// The screen bound of a simulated sample: the ceiling plus the
/// discretisation tolerance of `common::adaptive_islands` (planner cell,
/// sim cell, emission tolerance).
fn sim_bound() -> f64 {
    ceiling() + (planner_cell() + SIM_CELL + TOLERANCE) / (2.0 * TOOL_RADIUS)
}

/// The bound of the exact reading: the planner cell (the 2D planner admits
/// a sub-point sliver of one cell, `adaptive/CLAUDE.md`), the emission
/// tolerance, and the exact lattice pitch in place of the sim cell.
fn exact_bound() -> f64 {
    ceiling() + (planner_cell() + TOLERANCE + EXACT_PITCH_MM) / (2.0 * TOOL_RADIUS)
}

/// The cutter-centre distance from `(x, y)` to the nearest wall: the pocket
/// sides or the island.
fn wall_distance(x: f64, y: f64) -> f64 {
    let sides = (POCKET_HALF_X - x.abs()).min(POCKET_HALF_Y - y.abs());
    let island = x.hypot(y) - ISLAND_RADIUS;
    sides.min(island)
}

/// True when a cutter centre at `(x, y)` can stand on a perimeter sweep of
/// a region: the region boundary is at `R` from the wall, within the
/// material grid's cell plus the marching-squares half cell, and the mesh
/// wall slopes over one mesh step (`√2` steps on the island).
fn in_perimeter_band(x: f64, y: f64) -> bool {
    let d = wall_distance(x, y);
    let centre = 2.0 * TOOL_RADIUS + PERIMETER_INSET_MARGIN_MM;
    let quantum = 1.5 * planner_cell();
    d >= centre - quantum && d <= centre + SQRT_2 * MESH_STEP + quantum
}

fn params(strategy: ClearingStrategy3d) -> Adaptive3dParams {
    Adaptive3dParams {
        entry_clearance_mm: 0.5,
        ramp_feed_rate: None,
        geometry: Adaptive3dGeometry {
            tool_radius: TOOL_RADIUS,
            envelope_radius: TOOL_RADIUS,
            stepover: STEPOVER,
            tolerance: TOLERANCE,
            segment_merge_tolerance: None,
            min_cutting_radius: 0.0,
            boundary: None,
            world_stock_xy_bbox: None,
            centre_boundary: Vec::new(),
            gouge_probe: None,
        },
        depth: Adaptive3dDepth {
            depth_per_pass: DEPTH_PER_PASS,
            stock_to_leave: STOCK_TO_LEAVE,
            stock_top_z: TOP_Z,
            z_floor: None,
            detect_flat_areas: false,
            rest_top_z: None,
        },
        linking: Adaptive3dLinking {
            region_ordering: RegionOrdering::Global,
            min_region_cut_length_mm: 0.0,
            max_stay_down_distance_mm: None,
            stay_down_clearance_mm: 0.5,
            step_split: None,
            keep_down_route: None,
            level_contour: None,
            rest_region_filter: None,
        },
        feed_rate: 1000.0,
        plunge_rate: 500.0,
        safe_z: 15.0,
        entry_style: EntryStyle3d::Plunge,
        initial_stock: None,
        clearing_strategy: strategy,
        trochoid_cap_mult: 1.6,
        engagement_measure: rs_cam_core::adaptive::EngagementMeasure::DiskArea,
    }
}

/// The product strategy's parameters.
fn geodesic() -> Adaptive3dParams {
    params(ClearingStrategy3d::GeodesicAdaptive)
}

fn plan(mesh: &TriangleMesh, index: &SpatialIndex, params: &Adaptive3dParams) -> Adaptive3dOutput {
    let cutter = FlatEndmill::new(2.0 * TOOL_RADIUS, 25.0);
    adaptive_3d_toolpath_output_with_cancel(mesh, index, &cutter, params, &|| false, None)
        .expect("the plan is not cancelled")
}

/// What the simulation of one plan reads.
#[derive(Debug, Default)]
struct Readout {
    moves: usize,
    judged: usize,
    /// Each judged sample over the screen, with its move's exact peak.
    screened: Vec<String>,
    /// The screened samples whose move the exact reading also puts over.
    over: Vec<String>,
    entry: usize,
    vertical: usize,
    departing: usize,
    waterline: usize,
    waterline_screened: usize,
    waterline_exact_peak: f64,
    perimeter_band: usize,
    perimeter_band_over: usize,
    peak_judged: f64,
    peak_exact: f64,
    judged_levels: BTreeSet<i64>,
    /// Samples on a capped move (a planned turn or a plunge departure),
    /// those over the screen, and their sim peak. For the screened capped
    /// moves: the peak of the exact load times the move's cap over the
    /// commanded feed (its removal rate as a fraction of the steady
    /// rate). Printed, not asserted.
    capped: usize,
    capped_screened: usize,
    capped_peak: f64,
    capped_rate_peak: f64,
    /// D19c: for each move over the exact bound, the moves round it
    /// ([`probe`]).
    probes: Vec<String>,
    /// C22a: each fed link sample that removes more than the sim triage's
    /// bar for a stepped op (`stock::sim_triage::link_cut_observation`):
    /// the commanded Depth/Pass plus one sim cell, on the larger of the
    /// axial engagement and the plunge descent.
    link_over: Vec<String>,
}

fn is_entry(intent: MoveIntent) -> bool {
    matches!(
        intent,
        MoveIntent::EntryPlunge | MoveIntent::EntryHelix | MoveIntent::EntryRamp
    )
}

/// The vertical moves that enter the stock after a descent: the count of
/// entries of a toolpath.
fn entry_count(tp: &Toolpath) -> usize {
    let mut count = 0;
    let mut in_entry = false;
    for m in &tp.moves {
        let entry = is_entry(m.intent);
        if entry && !in_entry {
            count += 1;
        }
        in_entry = entry;
    }
    count
}

/// For each move, true when it lies in the waterline cleanup of a level:
/// from a `WaterlineCleanup` annotation up to the next level or region
/// annotation.
fn waterline_moves(out: &Adaptive3dOutput) -> Vec<bool> {
    let n = out.toolpath.moves.len();
    let mut mask = vec![false; n];
    let mut start: Option<usize> = None;
    for a in &out.annotations {
        let bounds = matches!(
            a.event,
            Adaptive3dRuntimeEvent::GlobalZLevel { .. }
                | Adaptive3dRuntimeEvent::RegionZLevel { .. }
                | Adaptive3dRuntimeEvent::RegionStart { .. }
                | Adaptive3dRuntimeEvent::WaterlineCleanup
        );
        if !bounds {
            continue;
        }
        if let Some(s) = start.take() {
            for f in mask.iter_mut().take(a.move_index.min(n)).skip(s) {
                *f = true;
            }
        }
        if matches!(a.event, Adaptive3dRuntimeEvent::WaterlineCleanup) {
            start = Some(a.move_index);
        }
    }
    if let Some(s) = start {
        for f in mask.iter_mut().skip(s) {
            *f = true;
        }
    }
    mask
}

/// For each move, true when it departs a straight plunge as the 2D walker
/// exempts it (`adaptive/path.rs`): from a vertical `EntryPlunge`, each
/// move with XY travel while it starts within `R` of the plunge; the first
/// one that starts at `R` or more ends the exemption until the next
/// plunge. A helix or ramp entry, or a rapid with XY travel, also ends it.
/// Vertical moves do not change it.
fn departing_moves(tp: &Toolpath) -> Vec<bool> {
    let mut out = Vec::with_capacity(tp.moves.len());
    let mut origin: Option<(f64, f64)> = None;
    let mut prev = None::<P3>;
    for m in &tp.moves {
        let travel = prev.is_some_and(|p| (p.x - m.target.x).hypot(p.y - m.target.y) >= 1e-9);
        let mut departing = false;
        match m.intent {
            MoveIntent::EntryPlunge | MoveIntent::EntryApproach
                if prev.is_some_and(|p| {
                    (p.x - m.target.x).hypot(p.y - m.target.y) < 1e-6 && m.target.z < p.z
                }) =>
            {
                origin = Some((m.target.x, m.target.y));
            }
            MoveIntent::EntryHelix | MoveIntent::EntryRamp => origin = None,
            _ if matches!(m.move_type, MoveType::Rapid) => {
                if travel {
                    origin = None;
                }
            }
            MoveIntent::Drilling
            | MoveIntent::EntryPlunge
            | MoveIntent::EntryApproach
            | MoveIntent::ClearingCut
            | MoveIntent::FinishingCut
            | MoveIntent::Linking
            | MoveIntent::Retract
            | MoveIntent::LeadIn
            | MoveIntent::LeadOut
            | MoveIntent::ObstacleClearance
            | MoveIntent::Unknown => {
                if let (Some(p), Some((hx, hy)), true) = (prev, origin, travel) {
                    if (p.x - hx).hypot(p.y - hy) < TOOL_RADIUS - 1e-9 {
                        departing = true;
                    } else {
                        origin = None;
                    }
                }
            }
        }
        out.push(departing);
        prev = Some(m.target);
    }
    out
}

/// The points along move `mv` from `from`, every `pitch` / 2 mm (arcs
/// followed). A copy of `move_points` of
/// `a_clearing_cut_holds_the_pass_load_g_adaptpassload`.
fn move_points(from: P3, mv: &Move, pitch: f64) -> Vec<(f64, f64)> {
    let arc = match mv.move_type {
        MoveType::ArcCW { i, j, .. } => Some((i, j, true)),
        MoveType::ArcCCW { i, j, .. } => Some((i, j, false)),
        MoveType::Rapid | MoveType::Linear { .. } => None,
    };
    let len = (mv.target.x - from.x).hypot(mv.target.y - from.y) * 2.0;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a move of the fixture is under 200 mm, so the point count is small and positive"
    )]
    let k = ((len / pitch).ceil() as usize).max(1);
    (0..=k)
        .map(|s| {
            let t = s as f64 / k as f64;
            match arc {
                Some((i, j, cw)) => {
                    let (cx, cy) = (from.x + i, from.y + j);
                    let rad = i.hypot(j);
                    let a0 = (from.y - cy).atan2(from.x - cx);
                    let a1 = (mv.target.y - cy).atan2(mv.target.x - cx);
                    let tau = std::f64::consts::TAU;
                    let sweep = if cw {
                        -((a0 - a1).rem_euclid(tau))
                    } else {
                        (a1 - a0).rem_euclid(tau)
                    };
                    let a = a0 + t * sweep;
                    (cx + rad * a.cos(), cy + rad * a.sin())
                }
                None => (
                    from.x + t * (mv.target.x - from.x),
                    from.y + t * (mv.target.y - from.y),
                ),
            }
        })
        .collect()
}

/// The XY points of `move_points` with the tip Z of each (Z linear along
/// the move, as the emitter interpolates it).
fn move_points_z(from: P3, mv: &Move, pitch: f64) -> Vec<(f64, f64, f64)> {
    let pts = move_points(from, mv, pitch);
    let last = (pts.len() - 1).max(1) as f64;
    pts.into_iter()
        .enumerate()
        .map(|(s, (x, y))| (x, y, from.z + (s as f64 / last) * (mv.target.z - from.z)))
        .collect()
}

/// True when no point of move `mv` (from `from`) comes within `reach` of
/// `at` in XY: the move cannot touch the lattice round `at`.
fn move_is_far(from: P3, mv: &Move, at: (f64, f64), reach: f64) -> bool {
    match mv.move_type {
        MoveType::ArcCW { i, j, .. } | MoveType::ArcCCW { i, j, .. } => {
            let (cx, cy) = (from.x + i, from.y + j);
            ((at.0 - cx).hypot(at.1 - cy) - i.hypot(j)).abs() > reach
        }
        MoveType::Rapid | MoveType::Linear { .. } => segment_distance(at, from, mv.target) > reach,
    }
}

/// The exact reading of the sub-segment `from → to` of move `m` (C43): a
/// height lattice at [`EXACT_PITCH_MM`] round the sample. Each point starts
/// at the stock top `TOP_Z`. Every earlier move, then move `m` up to
/// `from`, lowers it to the tip Z where the cutter passes. In the disc at
/// the sub-segment's midpoint, a point has stock above the cutter when its
/// top is more than 0.01 mm over the tip Z there plus the cutter's height
/// at that radius. Returns `(w, l)`:
///
/// - `w`: the sideways extent of those points, over D;
/// - `l`: the load, `chip_load_fraction` over the sideways strips of one
///   pitch, each at the deepest height over the cutter of its points (the
///   one load `L` of the codebase, step 7.1). At a full pass across the
///   width `l = w`.
fn exact_width_at(
    tp: &Toolpath,
    m: usize,
    from: (f64, f64),
    to: (f64, f64),
    cutter: &dyn MillingCutter,
    depth_per_pass: f64,
) -> (f64, f64) {
    let r = TOOL_RADIUS;
    let pitch = EXACT_PITCH_MM;
    let half = 2.0 * r;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the lattice spans 4R at the pitch: 481 points"
    )]
    let n = (2.0 * half / pitch) as usize + 1;
    let (x0, y0) = (to.0 - half, to.1 - half);
    let mut top = vec![TOP_Z; n * n];
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "each index is clamped to the lattice before the cast"
    )]
    let stamp = |top: &mut Vec<f64>, px: f64, py: f64, pz: f64| {
        if (px - to.0).abs() > half + r || (py - to.1).abs() > half + r {
            return;
        }
        let i0 = ((px - r - x0) / pitch).floor().max(0.0) as usize;
        let i1 = (((px + r - x0) / pitch).ceil().max(0.0) as usize).min(n - 1);
        let j0 = ((py - r - y0) / pitch).floor().max(0.0) as usize;
        let j1 = (((py + r - y0) / pitch).ceil().max(0.0) as usize).min(n - 1);
        for j in j0..=j1 {
            for i in i0..=i1 {
                let (x, y) = (x0 + i as f64 * pitch, y0 + j as f64 * pitch);
                if (x - px).hypot(y - py) <= r {
                    let t = &mut top[j * n + i];
                    *t = t.min(pz);
                }
            }
        }
    };
    let mut prev = tp.moves[0].target;
    for mv in tp.moves.iter().take(m) {
        if !move_is_far(prev, mv, to, half + r) {
            for (px, py, pz) in move_points_z(prev, mv, pitch) {
                stamp(&mut top, px, py, pz);
            }
        }
        prev = mv.target;
    }
    let mid = (0.5 * (from.0 + to.0), 0.5 * (from.1 + to.1));
    let mut tip = tp.moves[m].target.z;
    let mut nearest = f64::INFINITY;
    for (px, py, pz) in move_points_z(prev, &tp.moves[m], pitch) {
        let d = (px - mid.0).hypot(py - mid.1);
        if d < nearest {
            nearest = d;
            tip = pz;
        }
    }
    for (px, py, pz) in move_points_z(prev, &tp.moves[m], pitch) {
        stamp(&mut top, px, py, pz);
        if (px - from.0).hypot(py - from.1) < pitch {
            break;
        }
    }
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let l = dx.hypot(dy).max(1e-9);
    let (sx, sy) = (-dy / l, dx / l);
    let mut strips: BTreeMap<i64, f64> = BTreeMap::new();
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for j in 0..n {
        for i in 0..n {
            let (x, y) = (x0 + i as f64 * pitch, y0 + j as f64 * pitch);
            let rad = (x - mid.0).hypot(y - mid.1);
            if rad > r {
                continue;
            }
            let above = top[j * n + i] - (tip + cutter.height_at_radius(rad).unwrap_or(0.0));
            if above > 0.01 {
                let u = (x - mid.0) * sx + (y - mid.1) * sy;
                lo = lo.min(u);
                hi = hi.max(u);
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "u is within the disc: the index is within ±R / pitch"
                )]
                let b = (u / pitch).round() as i64;
                let h = strips.entry(b).or_insert(0.0);
                *h = h.max(above);
            }
        }
    }
    if hi < lo {
        return (0.0, 0.0);
    }
    let load = chip_load_fraction(depth_per_pass, 2.0 * r, strips, pitch);
    ((hi - lo) / (2.0 * r), load)
}

/// The exact peaks of move `m`: [`exact_width_at`] at each of its samples,
/// from the previous sample (or the move start). Returns `(width, load)`.
fn exact_peak(
    tp: &Toolpath,
    samples: &[SimulationCutSample],
    m: usize,
    cutter: &dyn MillingCutter,
    depth_per_pass: f64,
) -> (f64, f64) {
    let start = tp.moves[m - 1].target;
    let mut last = (start.x, start.y);
    let (mut width, mut load) = (0.0_f64, 0.0_f64);
    for s in samples.iter().filter(|s| s.move_index == m) {
        let at = (s.position[0], s.position[1]);
        if (at.0 - last.0).hypot(at.1 - last.1) > 1e-6 {
            let (w, l) = exact_width_at(tp, m, last, at, cutter, depth_per_pass);
            width = width.max(w);
            load = load.max(l);
        }
        last = at;
    }
    (width, load)
}

/// The dexel simulation of `tp` on the planner's stock box (the mesh box
/// grown by R in XY, from the mesh bottom to the stock top).
fn simulate(mesh: &TriangleMesh, tp: &Toolpath) -> Vec<SimulationCutSample> {
    let cutter = FlatEndmill::new(2.0 * TOOL_RADIUS, 25.0);
    let b = &mesh.bbox;
    let mut stock = TriDexelStock::from_stock(
        b.min.x - TOOL_RADIUS,
        b.min.y - TOOL_RADIUS,
        b.max.x + TOOL_RADIUS,
        b.max.y + TOOL_RADIUS,
        b.min.z,
        TOP_Z,
        SIM_CELL,
    );
    stock
        .simulate_toolpath_with_metrics_with_cancel(
            tp,
            &cutter,
            StockCutDirection::FromTop,
            ToolpathId::from_raw(0),
            12_000,
            2,
            3000.0,
            SIM_CELL,
            None,
            &[],
            &[],
            false,
            &|| false,
        )
        .expect("the simulation is not cancelled")
}

/// `band_exempt`: whether the perimeter-sweep band is exempt (the
/// slice-pipeline control only, C31; no strategy left takes it). `depth_per_pass`: the op's Depth/Pass,
/// the `d` of the load (C43). `feed_rate`: the commanded feed.
fn read(
    mesh: &TriangleMesh,
    out: &Adaptive3dOutput,
    band_exempt: bool,
    depth_per_pass: f64,
    feed_rate: f64,
) -> Readout {
    let toolpath = &out.toolpath;
    let cutter = FlatEndmill::new(2.0 * TOOL_RADIUS, 25.0);
    let samples = simulate(mesh, toolpath);
    let screen = sim_bound();
    let exact = exact_bound();
    let waterline = waterline_moves(out);
    let departures = departing_moves(toolpath);
    let mut r = Readout {
        moves: toolpath.moves.len(),
        ..Readout::default()
    };
    let mut screened: Vec<(usize, [f64; 3], f64)> = Vec::new();
    let mut waterline_screened: BTreeSet<usize> = BTreeSet::new();
    let mut capped_screened: BTreeSet<usize> = BTreeSet::new();
    for s in &samples {
        let m = s.move_index;
        let Some(mv) = toolpath.moves.get(m) else {
            continue;
        };
        if !s.is_cutting || matches!(mv.move_type, MoveType::Rapid) {
            continue;
        }
        let [x, y, z] = s.position;
        let radial = s.engagement.radial_woc_fraction;
        if matches!(mv.intent, MoveIntent::Linking) {
            let removed = s.axial_engagement_mm.max(s.plunge_descent_mm);
            if removed > depth_per_pass + SIM_CELL {
                r.link_over.push(format!(
                    "move {m} at ({x:.2}, {y:.2}, {z:.2}) removes {removed:.2} mm"
                ));
            }
        }
        if is_entry(mv.intent) {
            r.entry += 1;
            continue;
        }
        let Some(start) = m.checked_sub(1).map(|j| toolpath.moves[j].target) else {
            continue;
        };
        if (mv.target.x - start.x).hypot(mv.target.y - start.y) < 1e-9 {
            r.vertical += 1;
            continue;
        }
        // A planned turn or a plunge departure: admitted over the ceiling
        // and fed down (design part F); the feed rule judges it.
        if mv.feed_cap.is_some() {
            r.capped += 1;
            r.capped_peak = r.capped_peak.max(radial);
            let sim_box = radial * s.axial_engagement_mm / depth_per_pass;
            if radial > screen || sim_box > screen {
                r.capped_screened += 1;
                capped_screened.insert(m);
            }
            continue;
        }
        if waterline[m] {
            r.waterline += 1;
            if radial > screen {
                r.waterline_screened += 1;
                waterline_screened.insert(m);
            }
            continue;
        }
        if departures[m] {
            r.departing += 1;
            continue;
        }
        if band_exempt && in_perimeter_band(x, y) {
            r.perimeter_band += 1;
            if radial > screen {
                r.perimeter_band_over += 1;
            }
            continue;
        }
        r.judged += 1;
        r.peak_judged = r.peak_judged.max(radial);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a level Z of the fixture is under 20 mm"
        )]
        r.judged_levels.insert((z * 10.0).round() as i64);
        // C43: the sim's box (deepest engaged height times the width) is
        // never under the exact load, so it screens beside the radial.
        let sim_box = radial * s.axial_engagement_mm / depth_per_pass;
        if radial > screen || sim_box > screen {
            screened.push((m, s.position, radial));
        }
    }
    let mut peak_of: BTreeMap<usize, (f64, f64)> = BTreeMap::new();
    for &(m, [x, y, z], radial) in &screened {
        let (width, peak) = *peak_of
            .entry(m)
            .or_insert_with(|| exact_peak(toolpath, &samples, m, &cutter, depth_per_pass));
        r.peak_exact = r.peak_exact.max(peak);
        let line = format!(
            "move {m} {:?} at ({x:.2}, {y:.2}, {z:.2}) wall distance {:.2}: sim radial \
             {radial:.3}, exact width peak of the move {width:.3}, exact load peak {peak:.3}",
            toolpath.moves[m].intent,
            wall_distance(x, y)
        );
        if peak > exact {
            r.over.push(line.clone());
            if !r
                .probes
                .iter()
                .any(|p| p.starts_with(&format!("probe move {m} ")))
            {
                r.probes.push(probe(toolpath, m, (x, y, z)));
            }
        }
        r.screened.push(line);
    }
    for &m in &waterline_screened {
        r.waterline_exact_peak = r
            .waterline_exact_peak
            .max(exact_peak(toolpath, &samples, m, &cutter, depth_per_pass).1);
    }
    for &m in &capped_screened {
        let load = exact_peak(toolpath, &samples, m, &cutter, depth_per_pass).1;
        let cap = toolpath.moves[m].feed_cap.unwrap_or(feed_rate);
        r.capped_rate_peak = r.capped_rate_peak.max(load * cap / feed_rate);
    }
    r
}

/// The XY distance from `p` to the segment `a`-`b`.
fn segment_distance(p: (f64, f64), a: P3, b: P3) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 {
        (((p.0 - a.x) * dx + (p.1 - a.y) * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (a.x + t * dx - p.0).hypot(a.y + t * dy - p.1)
}

/// D19c probe: the moves round an over move `m` whose sample is at `at`:
/// the twelve moves before `m` and the two after it, and every earlier
/// move whose XY segment passes within R of the sample (counts at, above
/// and below its level, and the last eight).
fn probe(tp: &Toolpath, m: usize, at: (f64, f64, f64)) -> String {
    let kind = |mv: &Move| match mv.move_type {
        MoveType::Rapid => "rapid".to_owned(),
        MoveType::Linear { feed_rate } => format!("feed {feed_rate:.0}"),
        MoveType::ArcCW { .. } | MoveType::ArcCCW { .. } => "arc".to_owned(),
    };
    let mut out = format!(
        "probe move {m} at ({:.2}, {:.2}, {:.2}):\n",
        at.0, at.1, at.2
    );
    for k in m.saturating_sub(12)..(m + 3).min(tp.moves.len()) {
        let mv = &tp.moves[k];
        let from = k.checked_sub(1).map_or(mv.target, |j| tp.moves[j].target);
        out.push_str(&format!(
            "    {k}{} {:?} {} to ({:.3}, {:.3}, {:.3}) xy {:.3}\n",
            if k == m { " <<" } else { "" },
            mv.intent,
            kind(mv),
            mv.target.x,
            mv.target.y,
            mv.target.z,
            (mv.target.x - from.x).hypot(mv.target.y - from.y)
        ));
    }
    let (mut at_level, mut above, mut below) = (0usize, 0usize, 0usize);
    let mut near: Vec<String> = Vec::new();
    for k in 1..m {
        let (a, b) = (tp.moves[k - 1].target, tp.moves[k].target);
        if matches!(tp.moves[k].move_type, MoveType::Rapid)
            || segment_distance((at.0, at.1), a, b) > TOOL_RADIUS
        {
            continue;
        }
        let (lo, hi) = (a.z.min(b.z), a.z.max(b.z));
        if (lo - at.2).abs() <= 0.01 && (hi - at.2).abs() <= 0.01 {
            at_level += 1;
        } else if lo > at.2 + 0.01 {
            above += 1;
        } else {
            below += 1;
        }
        near.push(format!(
            "{k} {:?} ({:.2}, {:.2}, {:.2}) to ({:.2}, {:.2}, {:.2})",
            tp.moves[k].intent, a.x, a.y, a.z, b.x, b.y, b.z
        ));
    }
    out.push_str(&format!(
        "    earlier feed moves within R: {at_level} at the level, {above} above it, {below} \
         across or below it; the last eight:\n"
    ));
    for line in near.iter().rev().take(8).rev() {
        out.push_str(&format!("      {line}\n"));
    }
    out
}

fn summary(label: &str, r: &Readout) -> String {
    format!(
        "{label}: {} moves, {} judged at levels {:?} (Z x 10), {} over the screen {:.3} \
         (sim peak {:.3}), {} over the exact bound {:.3} (exact peak {:.3}); entry {}, \
         vertical {}, departing {}, waterline {} ({} over the screen, exact peak {:.3}), \
         perimeter band {} ({} over the screen), capped {} ({} over the screen, sim peak \
         {:.3}, exact load x cap / feed peak {:.3})",
        r.moves,
        r.judged,
        r.judged_levels,
        r.screened.len(),
        sim_bound(),
        r.peak_judged,
        r.over.len(),
        exact_bound(),
        r.peak_exact,
        r.entry,
        r.vertical,
        r.departing,
        r.waterline,
        r.waterline_screened,
        r.waterline_exact_peak,
        r.perimeter_band,
        r.perimeter_band_over,
        r.capped,
        r.capped_screened,
        r.capped_peak,
        r.capped_rate_peak
    )
}

/// The feed rule of the capped moves (design part F): each capped move is
/// fed at or under its cap, and its cap is under the commanded feed.
fn cap_faults(tp: &Toolpath, feed_rate: f64) -> Vec<String> {
    let mut faults = Vec::new();
    for (m, mv) in tp.moves.iter().enumerate() {
        let Some(cap) = mv.feed_cap else {
            continue;
        };
        let feed = mv.move_type.feed_rate().unwrap_or(f64::INFINITY);
        if feed > cap {
            faults.push(format!("move {m}: feed {feed} over its cap {cap}"));
        }
        if cap >= feed_rate {
            faults.push(format!(
                "move {m}: cap {cap} not under the commanded feed {feed_rate}"
            ));
        }
    }
    faults
}

/// The number of distinct moves in the over lines of a [`Readout`]
/// ("move N …").
fn over_moves(lines: &[String]) -> usize {
    lines
        .iter()
        .filter_map(|l| {
            l.split_whitespace()
                .nth(1)
                .and_then(|n| n.parse::<usize>().ok())
        })
        .collect::<BTreeSet<usize>>()
        .len()
}

/// The two-level judge. The Contour Parallel arm on the same fixture is
/// printed as a control and not asserted (AgentSearch until removal step
/// R4). For Geodesic Adaptive:
///
/// 1. it cuts judged samples at two distinct Z levels;
/// 2. no judged sample is over on the exact geometry (each screened
///    sample, and a probe of each over move, is printed first);
/// 3. no fed link removes more than the sim triage's bar (C22a);
/// 4. the capped moves keep the feed rule ([`cap_faults`]), and at least
///    one move is capped.
#[test]
#[ignore = "heavy: two 3D plans and two dexel simulations with exact readings; run by name \
            with --ignored"]
fn geodesic_adaptive_holds_the_pass_load_at_two_levels() {
    let mesh = pocket_with_island();
    let index = SpatialIndex::build(&mesh, 10.0);

    let control_params = params(ClearingStrategy3d::ContourParallel);
    let control = plan(&mesh, &index, &control_params);
    let c = read(
        &mesh,
        &control,
        false,
        DEPTH_PER_PASS,
        control_params.feed_rate,
    );
    eprintln!("{}", summary("control ContourParallel", &c));

    let p = geodesic();
    let out = plan(&mesh, &index, &p);
    let r = read(&mesh, &out, false, p.depth.depth_per_pass, p.feed_rate);
    let label = "geodesic_adaptive";
    eprintln!(
        "{}; {} entries",
        summary(label, &r),
        entry_count(&out.toolpath)
    );
    for line in &r.screened {
        eprintln!("  {label} screened: {line}");
    }
    for probe in &r.probes {
        eprintln!("  {label} {probe}");
    }
    let mut failures: Vec<String> = Vec::new();
    if r.judged_levels.len() < 2 {
        failures.push(format!(
            "judged cutting samples at {:?} (Z x 10); the arm must cut two levels",
            r.judged_levels
        ));
    }
    if !r.over.is_empty() {
        failures.push(format!(
            "{} judged samples read over the exact bound {:.3} (exact peak {:.3}); the first: {}",
            r.over.len(),
            exact_bound(),
            r.peak_exact,
            r.over.first().map_or("", String::as_str)
        ));
    }
    if !r.link_over.is_empty() {
        failures.push(format!("fed links cut stock: {:#?}", r.link_over));
    }
    let faults = cap_faults(&out.toolpath, p.feed_rate);
    if !faults.is_empty() {
        failures.push(format!("the capped moves break the feed rule: {faults:#?}"));
    }
    if r.capped == 0 {
        failures.push("no capped move: the feed rule tests nothing".to_owned());
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// C22, the cheap check: on one level of the same pocket, the plan feeds
/// its clearing turns and its plunge departures down at the level.
///
/// - One level: Depth/Pass is the full pocket depth, so the only level is
///   the floor level (`FLOOR_Z + STOCK_TO_LEAVE`).
/// - The capped moves keep the feed rule ([`cap_faults`]).
/// - At least one capped move is a turn (not a departure of a plunge) and
///   at least one is a departure ([`departing_moves`]), so both kinds are
///   tested.
/// - Each capped move that is not a departure runs at the level: both of
///   its ends are within the emission tolerance of the floor level. A turn
///   that left the level would be an entry or a lift that the 2D plan does
///   not have.
///
/// No simulation runs.
#[test]
fn geodesic_adaptive_feeds_its_turns_down_at_the_level() {
    let mesh = pocket_with_island();
    let index = SpatialIndex::build(&mesh, 10.0);
    let level = FLOOR_Z + STOCK_TO_LEAVE;
    let mut p = geodesic();
    p.depth.depth_per_pass = TOP_Z - FLOOR_Z;
    let out = plan(&mesh, &index, &p);
    let tp = &out.toolpath;
    let departing = departing_moves(tp);
    let faults = cap_faults(tp, p.feed_rate);
    assert!(
        faults.is_empty(),
        "the capped moves break the feed rule: {faults:#?}"
    );
    let (mut turns, mut departures) = (0usize, 0usize);
    let mut off_level = Vec::new();
    for (m, mv) in tp.moves.iter().enumerate().skip(1) {
        if mv.feed_cap.is_none() {
            continue;
        }
        if departing[m] {
            departures += 1;
            continue;
        }
        turns += 1;
        let (a, b) = (tp.moves[m - 1].target, mv.target);
        if (a.z - level).abs() > TOLERANCE || (b.z - level).abs() > TOLERANCE {
            off_level.push(format!("move {m}: Z {:.3} to {:.3}", a.z, b.z));
        }
    }
    eprintln!(
        "geodesic_adaptive one level: {} moves, {} entries, capped turn moves {turns}, capped \
         departure moves {departures}",
        tp.moves.len(),
        entry_count(tp)
    );
    assert!(
        turns > 0,
        "no capped turn move: the turn rule tests nothing"
    );
    assert!(
        departures > 0,
        "no capped departure move: the departure rule tests nothing"
    );
    assert!(
        off_level.is_empty(),
        "capped turn moves off the level Z {level}: {off_level:#?}"
    );
}

/// C22a (P3_C22_3d_turns.md §14): with the F-038 group filter on at the op
/// default (`Adaptive3dConfig::default().min_region_cut_length_mm`), no
/// fed link of the plan removes more than the sim triage's bar for a
/// stepped op (one Depth/Pass plus one sim cell). The filter drops short
/// groups after the 2D replay admitted the plan's links; the replay after
/// the filter must refuse the links that crossed a dropped group's ground.
/// The other tests run with the filter off. Two levels.
#[test]
#[ignore = "heavy: one 3D plan and one dexel simulation; run by name with --ignored"]
fn no_held_link_cuts_standing_stock_with_the_group_filter_on() {
    let mesh = pocket_with_island();
    let index = SpatialIndex::build(&mesh, 10.0);
    let threshold = rs_cam_core::compute::operation_configs::Adaptive3dConfig::default()
        .min_region_cut_length_mm;
    assert!(threshold > 0.0, "the op default turns the F-038 filter off");
    let mut p = geodesic();
    p.linking.min_region_cut_length_mm = threshold;
    let out = plan(&mesh, &index, &p);
    let r = read(&mesh, &out, false, p.depth.depth_per_pass, p.feed_rate);
    eprintln!(
        "geodesic_adaptive with the group filter at {threshold} mm: {}",
        summary("geodesic_adaptive", &r)
    );
    assert!(
        r.link_over.is_empty(),
        "fed links cut standing stock: {:#?}",
        r.link_over
    );
}

/// §16 Part A (P3_C22_3d_turns.md §16.7): the stock-read split holds the
/// exact bound where it acts. One level from the top to the floor drapes
/// over the island and gives the plan its large-Z steps. With the
/// stock-read split and the routed keep-down travel OFF, the simulation
/// must read no judged sample over the exact bound.
#[test]
#[ignore = "heavy: one one-level 3D plan and one dexel simulation with exact readings; run by \
            name with --ignored (§16 Part A)"]
fn the_stock_read_split_holds_the_exact_bound_on_draped_steps() {
    let mesh = pocket_with_island();
    let index = SpatialIndex::build(&mesh, 10.0);
    let mut p = geodesic();
    p.depth.depth_per_pass = TOP_Z - FLOOR_Z;
    p.linking.step_split = Some(StepSplit::StockDepth);
    p.linking.keep_down_route = Some(KeepDownRoute::Off);
    let out = plan(&mesh, &index, &p);
    let r = read(&mesh, &out, false, p.depth.depth_per_pass, p.feed_rate);
    let label = "geodesic_adaptive one level";
    eprintln!("{}", summary(label, &r));
    for line in &r.screened {
        eprintln!("  {label} screened: {line}");
    }
    for probe in &r.probes {
        eprintln!("  {label} {probe}");
    }
    assert!(
        r.over.is_empty(),
        "{} judged samples read over the exact bound {:.3} (exact peak {:.3}); the first: {}",
        r.over.len(),
        exact_bound(),
        r.peak_exact,
        r.over.first().map_or("", String::as_str)
    );
}

/// §16.9 (D30): the pocket with a SHELF at Z 6.5 on its +x side. One level
/// from the top to the floor drapes the cut up onto the shelf, where the
/// tool's disc meets a band of stock spanning the whole disc: a step
/// within one pass deep but wider than c·D. The simulation must read no
/// more MOVES over the exact bound under the stock-read split than the
/// same plan under the historical path-shape split (the shelf over-loads
/// under both: G-FACESLAB). Counted by move: one place cut by one long
/// move reads more samples than the same place cut by two short ones.
/// Router off.
#[test]
#[ignore = "heavy: two one-level 3D plans and two dexel simulations with exact readings; run \
            by name with --ignored (§16.9 width read)"]
fn the_stock_read_split_adds_no_over_move_on_the_shelf() {
    let mesh = shelf();
    let index = SpatialIndex::build(&mesh, 10.0);
    let run = |split: StepSplit| {
        let mut p = geodesic();
        p.depth.depth_per_pass = TOP_Z - FLOOR_Z;
        p.linking.step_split = Some(split);
        p.linking.keep_down_route = Some(KeepDownRoute::Off);
        let out = plan(&mesh, &index, &p);
        read(&mesh, &out, false, p.depth.depth_per_pass, p.feed_rate)
    };
    let base = run(StepSplit::PathShape);
    eprintln!("{}", summary("shelf, path-shape base", &base));
    for line in &base.over {
        eprintln!("  base over: {line}");
    }
    let r = run(StepSplit::StockDepth);
    eprintln!("{}", summary("shelf, stock-read split", &r));
    for line in &r.screened {
        eprintln!("  screened: {line}");
    }
    for probe in &r.probes {
        eprintln!("  {probe}");
    }
    let (moves, base_moves) = (over_moves(&r.over), over_moves(&base.over));
    assert!(
        moves <= base_moves,
        "{moves} moves over the exact bound {:.3} under the stock-read split against \
         {base_moves} under the path-shape split (exact peak {:.3}); the first: {}",
        exact_bound(),
        r.peak_exact,
        r.over.first().map_or("", String::as_str)
    );
}

/// C38 §16: Geodesic Adaptive clears the body only. On the channel fixture
/// (too narrow for the field's seed), with the F-038 filter at the op
/// default, the plan cuts no clearing move inside the channel's material
/// below the top: it leaves the channel to the rest pass (the D44 sentries
/// read the regions it states).
#[test]
fn geodesic_adaptive_leaves_the_channel_to_the_edge_pass() {
    let mesh = channel();
    let index = SpatialIndex::build(&mesh, 10.0);
    let mut p = geodesic();
    p.linking.min_region_cut_length_mm =
        rs_cam_core::compute::operation_configs::Adaptive3dConfig::default()
            .min_region_cut_length_mm;
    let out = plan(&mesh, &index, &p);
    let tp = &out.toolpath;
    let hw = channel_half_width();
    let inside =
        |q: P3| q.x.abs() <= CHANNEL_HALF_LENGTH && q.y.abs() <= hw && q.z < TOP_Z - TOLERANCE;
    let mut cuts = Vec::new();
    for m in 1..tp.moves.len() {
        let (a, mv) = (tp.moves[m - 1].target, &tp.moves[m]);
        if mv.move_type.is_cutting()
            && mv.intent == MoveIntent::ClearingCut
            && (a.x - mv.target.x).hypot(a.y - mv.target.y) > 1e-6
            && inside(a)
            && inside(mv.target)
        {
            cuts.push(format!(
                "move {m}: ({:.2}, {:.2}, {:.2}) to ({:.2}, {:.2}, {:.2})",
                a.x, a.y, a.z, mv.target.x, mv.target.y, mv.target.z
            ));
        }
    }
    eprintln!(
        "channel: half width {hw:.3} mm; {} moves, {} entries; {} clearing cuts inside the channel",
        tp.moves.len(),
        entry_count(tp),
        cuts.len()
    );
    assert!(
        cuts.is_empty(),
        "Geodesic Adaptive cut inside the channel: {cuts:#?}"
    );
}

/// §19.3 Parts 1–2 (D43), "no plunge into stock deeper than a pass": on the
/// stepped face and on the shelf, one level from the top, under the path
/// rule and under the stock-read rule, every vertical entry stroke
/// (`EntryPlunge` with no XY travel) that removes stock in the simulation
/// is at most one Depth/Pass long (plus one sim cell, the simulation's own
/// quantum). The stroke rule is the shipped peck's (`adaptive3d/path.rs`,
/// `emit_peck_plunge`).
#[test]
#[ignore = "heavy: four one-level 3D plans and four dexel simulations; run by name with \
            --ignored (§19.3 plunge depth)"]
fn no_entry_plunges_more_than_a_pass_into_stock() {
    let mut failures: Vec<String> = Vec::new();
    for (name, mesh) in [("stepped face", stepped_face()), ("shelf", shelf())] {
        let index = SpatialIndex::build(&mesh, 10.0);
        for split in [StepSplit::PathShape, StepSplit::StockDepth] {
            let mut p = geodesic();
            p.depth.depth_per_pass = TOP_Z - FLOOR_Z;
            p.linking.step_split = Some(split);
            p.linking.keep_down_route = Some(KeepDownRoute::Off);
            let dpp = p.depth.depth_per_pass;
            let out = plan(&mesh, &index, &p);
            let tp = &out.toolpath;
            let samples = simulate(&mesh, tp);
            // The vertical strokes that remove stock in the simulation, by
            // their length (the move's Z drop): the move's geometry, not a
            // sample, is the stroke.
            let mut stroke: BTreeMap<usize, (f64, [f64; 3])> = BTreeMap::new();
            for s in &samples {
                let m = s.move_index;
                let (Some(mv), Some(prev)) = (
                    tp.moves.get(m),
                    m.checked_sub(1).and_then(|j| tp.moves.get(j)),
                ) else {
                    continue;
                };
                let from = prev.target;
                let vertical = (mv.target.x - from.x).hypot(mv.target.y - from.y) < 1e-9;
                if mv.intent != MoveIntent::EntryPlunge
                    || !vertical
                    || !s.is_cutting
                    || s.removed_volume_est_mm3 <= 0.0
                {
                    continue;
                }
                stroke
                    .entry(m)
                    .or_insert((from.z - mv.target.z, s.position));
            }
            let deepest = stroke.values().map(|v| v.0).fold(0.0_f64, f64::max);
            for (m, (depth, at)) in &stroke {
                if *depth > dpp + SIM_CELL {
                    failures.push(format!(
                        "{name}, {split:?}: move {m} plunges {depth:.3} mm into stock at \
                         ({:.2}, {:.2}), over the pass {dpp} (+ sim cell {SIM_CELL})",
                        at[0], at[1]
                    ));
                }
            }
            eprintln!(
                "{name}, {split:?}: {} moves, {} entries; plunge strokes into stock {}, deepest \
                 {deepest:.3} mm",
                tp.moves.len(),
                entry_count(tp),
                stroke.len()
            );
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
