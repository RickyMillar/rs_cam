//! H3 Checkpoint B — the resolution-explicit A/B harness.
//!
//! Oracle: `planning/review_2026-07-29/TECH_DEBT_RESEARCH_AND_FIX_PLAN.md`
//! §H3 ("Research harness" / "Required metrics" / "Candidate architectures")
//! and §3.3 Checkpoint B. The evidence this file produces is written up in
//! `planning/review_2026-07-29/CHECKPOINT_B_EVIDENCE.md`.
//!
//! ## What varies and what does not
//!
//! Exactly ONE variable: the cell size of the finish GENERATION surface, and
//! it is varied by naming a [`FinishResolutionPolicy`] — never by moving the
//! tolerance, the tool, the planner dials or the op parameters. The policy is
//! passed to the three generators through their `*_with_resolution` seams, so
//! every arm walks the shipped code path with the shipped defaults except for
//! that one number, and each arm's surface still reports its own
//! `resolution.mode()` / `cell_source` provenance rather than collapsing to
//! `Explicit` (which is what driving the cell-size adapter directly would do
//! to the two NAMED arms).
//!
//! ## Arms (Ø1-tip / Ø6-shank / 7° tapered ball, tolerance pinned at 0.10)
//!
//! | arm | policy | cell |
//! |---|---|---|
//! | `envelope/4` | `LegacyEnvelopeQuarter` — scallop's + steep/shallow's shipped default | 0.750 mm |
//! | `geo-mean` | `GeoMeanEnvelopeCusp` — ramp-finish's shipped default since PR-8a | 0.306 mm |
//! | `cusp/4` | `CuspQuarter` | 0.125 mm |
//! | `tolerance` | `Explicit(tolerance)` | 0.100 mm |
//!
//! The one `Explicit` arm is explicit HONESTLY: it has no tool scale behind
//! it — it is "what the `.max(tolerance)` floor would give if it bound".
//! (Until PR-8a the geo-mean arm was `Explicit` too, a bisection probe; the
//! Checkpoint B decision turned that probe into a named mode, so the arm now
//! carries the provenance of a policy a production op selects.)
//!
//! ## §A.0 compliance — topology, not area
//!
//! The plan's §A.0 correction is that AREA is not a safe invariant for these
//! fixes: its direction flips between fixtures. So the slope-label comparison
//! below reports **region count** (connected components), **per-class cell
//! coverage** and **disagreement cell count** against a pinned fine reference
//! grid. Areas appear only as a derived, explicitly-caveated column.
//!
//! ## Measurement domains (M1)
//!
//! * `residual` — Z deviation of an emitted CUTTING move endpoint from the
//!   TOOL-CENTRE offset surface of the same cutter, bilinearly sampled from a
//!   pinned reference grid at [`REFERENCE_CELL_MM`]. Negative = tool centre
//!   BELOW the reference offset surface = gouge. This is the heightmap-sample
//!   domain, not a dexel-stock domain and not a 3D-surface-normal distance.
//! * `cusp` — flat-ground cusp implied by the measured spacing between
//!   ADJACENT-RING points (`h = R - sqrt(R² - (d/2)²)`, R = the cutter's cusp
//!   radius). It is an estimate on the tool-centre field, not a measured
//!   stock cusp; it is comparable BETWEEN ARMS, which is all Checkpoint B
//!   needs.
//! * `standing material` — `ScallopReport::uncut_core_mm2`, the same number
//!   `ToolpathStats::standing_material_mm2` carries (projected XY area, ring
//!   cascade residual stage — see `standing_material_channel_am9.rs`).
//! * `rapid grazes` — a REFERENCE-FIELD probe, not the dexel simulator: a
//!   rapid sampled at reference-cell spacing whose Z drops below the
//!   reference tool-centre surface. It is a lower bound on collisions and is
//!   NOT `SimulationMetrics::rapid_collision_count`.
//!
//! ## Test layout
//!
//! Default CI runs a fast representative subset (one fixture, scallop, two
//! arms, plus the ball control and the determinism guard). The FULL grid is
//! `#[ignore]`d and printed as markdown; its numbers are the evidence file's.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::too_many_arguments
)]

use std::time::Instant;

use rs_cam_core::finish_setup::{
    FinishResolutionMode, FinishResolutionPolicy, FinishSurface,
    build_finish_surface_with_policy_and_cancel,
};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::ramp_finish::{
    RampFinishParams, ramp_finish_generation_resolution,
    ramp_finish_toolpath_structured_annotated_with_resolution,
};
use rs_cam_core::scallop::{
    ScallopParams, ScallopRingBudget, ScallopRuntimeAnnotation, scallop_generation_resolution,
    scallop_toolpath_structured_annotated_with_resolution,
    scallop_toolpath_structured_annotated_with_resolution_and_ring_budget,
};
use rs_cam_core::steep_shallow::{
    SteepShallowParams, steep_shallow_toolpath_split_with_resolution,
};
use rs_cam_core::tool::{BallEndmill, MillingCutter, TaperedBallEndmill};
use rs_cam_core::toolpath::Toolpath;

// ── Pinned experiment constants ─────────────────────────────────────────

/// Held fixed on every arm. Chosen so the `.max(tolerance)` floor never
/// binds on the two NAMED arms (0.75 and 0.125 both exceed it) — otherwise
/// two arms would silently become the same grid and the A/B would compare
/// nothing.
const TOLERANCE_MM: f64 = 0.10;

/// The pinned HIGH-RES reference field every residual and every label is
/// scored against. Finer than the finest arm (0.100) by 2×, and finer than
/// `tip/8` (0.0625). It is an `Explicit` cell on purpose: the reference is
/// not a candidate policy, it is a ruler.
const REFERENCE_CELL_MM: f64 = 0.05;

/// Slope-class boundaries, matching `UnifiedFinishConfig`'s shipped defaults
/// (`steep_threshold_deg` 45, `waterline_threshold_deg` 75).
const STEEP_DEG: f64 = 45.0;
const VERY_STEEP_DEG: f64 = 75.0;

/// The project's finishing tool: Ø1 tip, 7° half angle, Ø6 shank.
/// `envelope_radius_mm()` = 3.0, `cusp_radius_mm()` = 0.5 — a 6× split.
fn taper() -> TaperedBallEndmill {
    TaperedBallEndmill::new(1.0, 7.0, 6.0, 25.0)
}

/// Ball control: `cusp_radius_mm() == envelope_radius_mm()`, so every
/// tool-scaled arm collapses onto one cell.
fn ball() -> BallEndmill {
    BallEndmill::new(3.0, 25.0)
}

// ── Arms ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
struct Arm {
    name: &'static str,
    policy: FinishResolutionPolicy,
}

/// The four arms, in coarse→fine order. Only `cell_mm` differs.
///
/// PR-8a: the intermediate arm is no longer an `Explicit` bisection probe —
/// it is the NAMED [`FinishResolutionMode::GeoMeanEnvelopeCusp`] policy that
/// `ramp_finish` now ships, so the arm carries its own provenance and the
/// evidence tables describe a mode a production op selects rather than a
/// number this harness invented. The cell value is identical (the mode's
/// formula IS the geometric mean this arm always used).
fn arms(cutter: &dyn MillingCutter) -> Vec<Arm> {
    let envelope_quarter = FinishResolutionPolicy::legacy_envelope_quarter(cutter, TOLERANCE_MM);
    let cusp_quarter = FinishResolutionPolicy::cusp_quarter(cutter, TOLERANCE_MM);
    vec![
        Arm {
            name: "envelope/4 (legacy)",
            policy: envelope_quarter,
        },
        Arm {
            name: "geo-mean (intermediate)",
            policy: FinishResolutionPolicy::geo_mean_envelope_cusp(cutter, TOLERANCE_MM),
        },
        Arm {
            name: "cusp/4",
            policy: cusp_quarter,
        },
        Arm {
            name: "tolerance",
            policy: FinishResolutionPolicy::explicit(TOLERANCE_MM),
        },
    ]
}

// ── Fixtures ────────────────────────────────────────────────────────────

/// Fixture half-extent (mm). 16 × 16 mm of model; the generation grid adds
/// one ENVELOPE radius (3 mm) of padding per side, so grids span 22 mm.
const HALF: f64 = 8.0;
/// Mesh vertex spacing. Fine enough that a 2 mm feature carries 4 samples
/// across — i.e. the FIXTURE is not the thing limiting feature fidelity.
const MESH_STEP: f64 = 0.25;

/// Build a height-field mesh from `z(x, y)` over `[-HALF, HALF]²`.
fn height_field(z: impl Fn(f64, f64) -> f64) -> TriangleMesh {
    let n = ((2.0 * HALF) / MESH_STEP).round() as usize + 1;
    let mut vertices = Vec::with_capacity(n * n);
    for j in 0..n {
        let y = -HALF + j as f64 * MESH_STEP;
        for i in 0..n {
            let x = -HALF + i as f64 * MESH_STEP;
            vertices.push(P3::new(x, y, z(x, y)));
        }
    }
    let mut triangles = Vec::with_capacity((n - 1) * (n - 1) * 2);
    for j in 0..(n - 1) {
        for i in 0..(n - 1) {
            let a = (j * n + i) as u32;
            let b = a + 1;
            let c = ((j + 1) * n + i) as u32;
            let d = c + 1;
            triangles.push([a, c, b]);
            triangles.push([b, c, d]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
}

/// A 2 mm-wide, 4 mm-tall triangular ridge on a flat base: flanks at
/// atan(4/1) ≈ 76°, i.e. VerySteep, on a feature NARROWER than the 6 mm
/// envelope and ~2.7 envelope-cells wide. This is the fixture the coarse
/// grid physically cannot represent.
fn narrow_ridge() -> TriangleMesh {
    height_field(|x, _y| 4.0 * (1.0 - x.abs()).max(0.0))
}

/// The inverse: a 2 mm-wide, 4 mm-deep V-groove in a plateau. Same wall
/// angle, but now the coarse grid's failure mode is a BRIDGED valley (the
/// tool-centre surface floats over it) rather than a clipped crest.
fn narrow_valley() -> TriangleMesh {
    height_field(|x, _y| 4.0 - 4.0 * (1.0 - x.abs()).max(0.0))
}

/// Three slope bands across X: ~6° shallow, 60° mid-steep, 85° very steep,
/// Y-invariant. Every class is present and each is wide enough to be a
/// region rather than a sliver — the fixture that makes class COVERAGE (not
/// just existence) meaningful.
fn mixed_slope_ribbon() -> TriangleMesh {
    // Profile knots: (x, z). tan(6°)=0.105, tan(60°)=1.732, tan(85°)=11.43.
    const KNOTS: [(f64, f64); 6] = [
        (-8.0, 0.0),
        (-3.0, 0.526),     // 5 mm of ~6°
        (-1.0, 4.0),       // 2 mm of 60°
        (-0.65, 8.0),      // 0.35 mm of 85°
        (0.65, 8.0),       // flat crest
        (8.0, 8.0 - 0.79), // long ~6° fall-off
    ];
    height_field(|x, _y| {
        if x <= KNOTS[0].0 {
            return KNOTS[0].1;
        }
        for w in KNOTS.windows(2) {
            let (x0, z0) = w[0];
            let (x1, z1) = w[1];
            if x <= x1 {
                let t = (x - x0) / (x1 - x0);
                return z0 + t * (z1 - z0);
            }
        }
        KNOTS[KNOTS.len() - 1].1
    })
}

/// Two disconnected domes plus a conical pit: disconnected non-shallow
/// territory AND a hole, so region COUNT (§A.0's invariant) can move
/// independently of total area.
fn disconnected_patches() -> TriangleMesh {
    height_field(|x, y| {
        let dome = |cx: f64, cy: f64| {
            let r = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
            if r < 2.5 { 3.0 * (1.0 - r / 2.5) } else { 0.0 }
        };
        let pit = {
            let r = (x * x + y * y).sqrt();
            if r < 1.6 { -3.0 * (1.0 - r / 1.6) } else { 0.0 }
        };
        dome(-4.5, -4.5) + dome(4.5, 4.5) + pit
    })
}

struct Fixture {
    name: &'static str,
    mesh: TriangleMesh,
    index: SpatialIndex,
}

impl Fixture {
    fn new(name: &'static str, mesh: TriangleMesh) -> Self {
        // Bucket size ~ the envelope diameter: a drop-cutter query touches a
        // bounded number of buckets at every arm resolution.
        let index = SpatialIndex::build(&mesh, 2.0);
        Self { name, mesh, index }
    }
}

fn fixtures() -> Vec<Fixture> {
    vec![
        Fixture::new("narrow ridge", narrow_ridge()),
        Fixture::new("narrow valley", narrow_valley()),
        Fixture::new("mixed-slope ribbon", mixed_slope_ribbon()),
        Fixture::new("patches + hole", disconnected_patches()),
    ]
}

// ── Op parameters, pinned across every arm ──────────────────────────────

fn scallop_params() -> ScallopParams {
    ScallopParams {
        scallop_height: 0.02,
        tolerance: TOLERANCE_MM,
        continuous: false,
        ..Default::default()
    }
}

fn ramp_finish_params() -> RampFinishParams {
    RampFinishParams {
        max_stepdown: 0.5,
        tolerance: TOLERANCE_MM,
        ..Default::default()
    }
}

fn steep_shallow_params() -> SteepShallowParams {
    SteepShallowParams {
        threshold_angle: STEEP_DEG,
        stepover: 0.5,
        z_step: 0.5,
        tolerance: TOLERANCE_MM,
        ..Default::default()
    }
}

// ── Measurement helpers ─────────────────────────────────────────────────

/// FNV-1a over the `Debug` rendering of the move list (round-trips every
/// f64 exactly). Same construction as `finish_resolution_policy_pr3.rs`.
fn fingerprint(tp: &Toolpath) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in format!("{:?}", tp.moves).bytes() {
        h ^= u64::from(byte);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Bilinear sample of a surface's tool-centre Z at `(x, y)`.
/// `None` when the point is off-grid or any of the four corners is an
/// UNCOVERED cell (those carry the `min_z` clamp, not a surface).
fn sample(surface: &FinishSurface, x: f64, y: f64) -> Option<f64> {
    let hm = &surface.heightmap;
    let fx = (x - hm.origin_x) / hm.cell_size;
    let fy = (y - hm.origin_y) / hm.cell_size;
    if fx < 0.0 || fy < 0.0 {
        return None;
    }
    let (c0, r0) = (fx.floor() as usize, fy.floor() as usize);
    if c0 + 1 >= hm.cols || r0 + 1 >= hm.rows {
        return None;
    }
    let (tx, ty) = (fx - c0 as f64, fy - r0 as f64);
    let at = |r: usize, c: usize| -> Option<f64> {
        let idx = r * hm.cols + c;
        if hm.covered_flags()[idx] {
            Some(hm.z_or_bbox_floor_values()[idx])
        } else {
            None
        }
    };
    let z00 = at(r0, c0)?;
    let z01 = at(r0, c0 + 1)?;
    let z10 = at(r0 + 1, c0)?;
    let z11 = at(r0 + 1, c0 + 1)?;
    Some(
        z00 * (1.0 - tx) * (1.0 - ty)
            + z01 * tx * (1.0 - ty)
            + z10 * (1.0 - tx) * ty
            + z11 * tx * ty,
    )
}

fn quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let idx = ((sorted.len() - 1) as f64 * q).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

#[derive(Default)]
struct PathMetrics {
    moves: usize,
    cut_moves: usize,
    min_segment_mm: f64,
    degenerate_segments: usize,
    /// Sampled cutting-move endpoints scored against the reference field.
    residual_samples: usize,
    residual_p50_abs: f64,
    residual_p95_abs: f64,
    residual_p99_abs: f64,
    residual_max_abs: f64,
    /// Most-negative deviation: the deepest gouge below the reference
    /// tool-centre surface.
    deepest_gouge_mm: f64,
    gouge_over_50um: usize,
    rapid_grazes: usize,
}

/// Score an emitted toolpath against the pinned reference field.
fn path_metrics(tp: &Toolpath, reference: Option<&FinishSurface>) -> PathMetrics {
    let mut m = PathMetrics {
        moves: tp.moves.len(),
        min_segment_mm: f64::INFINITY,
        ..Default::default()
    };
    let mut devs: Vec<f64> = Vec::new();
    let mut prev: Option<P3> = None;
    for mv in &tp.moves {
        let cutting = mv.move_type.is_cutting();
        if cutting {
            m.cut_moves += 1;
            if let Some(p) = prev {
                let d = ((mv.target.x - p.x).powi(2)
                    + (mv.target.y - p.y).powi(2)
                    + (mv.target.z - p.z).powi(2))
                .sqrt();
                if d > 1e-9 {
                    m.min_segment_mm = m.min_segment_mm.min(d);
                } else {
                    m.degenerate_segments += 1;
                }
            }
            if let Some(z_ref) = reference.and_then(|r| sample(r, mv.target.x, mv.target.y)) {
                devs.push(mv.target.z - z_ref);
            }
        } else if let (Some(r), Some(p)) = (reference, prev) {
            // Rapid: sample the straight line at reference-cell spacing.
            let d = ((mv.target.x - p.x).powi(2) + (mv.target.y - p.y).powi(2)).sqrt();
            let steps = ((d / REFERENCE_CELL_MM).ceil() as usize).clamp(1, 4096);
            for s in 0..=steps {
                let t = s as f64 / steps as f64;
                let x = p.x + (mv.target.x - p.x) * t;
                let y = p.y + (mv.target.y - p.y) * t;
                let z = p.z + (mv.target.z - p.z) * t;
                if sample(r, x, y).is_some_and(|z_ref| z < z_ref - 1e-6) {
                    m.rapid_grazes += 1;
                    break;
                }
            }
        }
        prev = Some(mv.target);
    }
    if !m.min_segment_mm.is_finite() {
        m.min_segment_mm = f64::NAN;
    }
    m.residual_samples = devs.len();
    m.deepest_gouge_mm = devs.iter().copied().fold(0.0_f64, f64::min);
    m.gouge_over_50um = devs.iter().filter(|d| **d < -0.05).count();
    let mut abs: Vec<f64> = devs.iter().map(|d| d.abs()).collect();
    abs.sort_by(|a, b| a.partial_cmp(b).expect("no NaN deviations"));
    m.residual_p50_abs = quantile(&abs, 0.50);
    m.residual_p95_abs = quantile(&abs, 0.95);
    m.residual_p99_abs = quantile(&abs, 0.99);
    m.residual_max_abs = abs.last().copied().unwrap_or(f64::NAN);
    m
}

/// Flat-ground cusp implied by the measured spacing between points on
/// ADJACENT RINGS. Returns (p50, p95) in mm, or `None` when fewer than two
/// rings were emitted.
fn measured_cusp(
    tp: &Toolpath,
    annotations: &[ScallopRuntimeAnnotation],
    cusp_radius: f64,
) -> Option<(f64, f64)> {
    if annotations.len() < 2 {
        return None;
    }
    // Ring id per move index, from the ring-start annotations.
    let mut ring_of = vec![usize::MAX; tp.moves.len()];
    for (n, ann) in annotations.iter().enumerate() {
        let end = annotations
            .get(n + 1)
            .map_or(tp.moves.len(), |next| next.move_index);
        for slot in ring_of.iter_mut().take(end).skip(ann.move_index) {
            *slot = n;
        }
    }
    let pts: Vec<(f64, f64, usize)> = tp
        .moves
        .iter()
        .enumerate()
        .filter(|(i, mv)| mv.move_type.is_cutting() && ring_of[*i] != usize::MAX)
        .map(|(i, mv)| (mv.target.x, mv.target.y, ring_of[i]))
        .collect();
    if pts.len() < 2 {
        return None;
    }
    // Uniform bucket grid, cell = 4× the flat stepover budget, so the
    // nearest other-ring point is always within the 3×3 neighbourhood.
    let bucket = (cusp_radius).max(0.25);
    let (mut minx, mut miny) = (f64::INFINITY, f64::INFINITY);
    for (x, y, _) in &pts {
        minx = minx.min(*x);
        miny = miny.min(*y);
    }
    let key = |x: f64, y: f64| -> (i64, i64) {
        (
            ((x - minx) / bucket).floor() as i64,
            ((y - miny) / bucket).floor() as i64,
        )
    };
    let mut grid: std::collections::HashMap<(i64, i64), Vec<usize>> =
        std::collections::HashMap::new();
    for (i, (x, y, _)) in pts.iter().enumerate() {
        grid.entry(key(*x, *y)).or_default().push(i);
    }
    let mut cusps: Vec<f64> = Vec::with_capacity(pts.len());
    for (x, y, ring) in &pts {
        let (kx, ky) = key(*x, *y);
        let mut best = f64::INFINITY;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for j in grid.get(&(kx + dx, ky + dy)).map_or(&[][..], |v| &v[..]) {
                    let (ox, oy, oring) = pts[*j];
                    if oring == *ring {
                        continue;
                    }
                    let d = ((x - ox).powi(2) + (y - oy).powi(2)).sqrt();
                    best = best.min(d);
                }
            }
        }
        if best.is_finite() {
            let half = (best / 2.0).min(cusp_radius);
            cusps.push(cusp_radius - (cusp_radius * cusp_radius - half * half).sqrt());
        }
    }
    if cusps.is_empty() {
        return None;
    }
    cusps.sort_by(|a, b| a.partial_cmp(b).expect("no NaN cusps"));
    Some((quantile(&cusps, 0.5), quantile(&cusps, 0.95)))
}

// ── §A.0: label-grid TOPOLOGY ───────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Class {
    Shallow,
    MidSteep,
    VerySteep,
}

fn class_of(angle_rad: f64) -> Class {
    let deg = angle_rad.to_degrees();
    if deg < STEEP_DEG {
        Class::Shallow
    } else if deg < VERY_STEEP_DEG {
        Class::MidSteep
    } else {
        Class::VerySteep
    }
}

/// Connected components (4-connectivity) of `class` over COVERED cells.
fn region_count(surface: &FinishSurface, class: Class) -> usize {
    let (rows, cols) = (surface.rows(), surface.cols());
    let hm = &surface.heightmap;
    let member: Vec<bool> = (0..rows * cols)
        .map(|i| hm.covered_flags()[i] && class_of(surface.slope_map.angles[i]) == class)
        .collect();
    let mut seen = vec![false; rows * cols];
    let mut count = 0usize;
    let mut stack: Vec<usize> = Vec::new();
    for start in 0..rows * cols {
        if !member[start] || seen[start] {
            continue;
        }
        count += 1;
        seen[start] = true;
        stack.push(start);
        while let Some(idx) = stack.pop() {
            let (r, c) = (idx / cols, idx % cols);
            let push = |r: usize, c: usize, stack: &mut Vec<usize>, seen: &mut Vec<bool>| {
                let n = r * cols + c;
                if member[n] && !seen[n] {
                    seen[n] = true;
                    stack.push(n);
                }
            };
            if r > 0 {
                push(r - 1, c, &mut stack, &mut seen);
            }
            if r + 1 < rows {
                push(r + 1, c, &mut stack, &mut seen);
            }
            if c > 0 {
                push(r, c - 1, &mut stack, &mut seen);
            }
            if c + 1 < cols {
                push(r, c + 1, &mut stack, &mut seen);
            }
        }
    }
    count
}

struct Topology {
    cells: usize,
    covered_cells: usize,
    /// Fraction of covered cells in each class, on this arm's OWN grid.
    coverage: [f64; 3],
    /// Connected components of MidSteep / VerySteep on this arm's own grid.
    regions_mid: usize,
    regions_very: usize,
    /// Cells of the REFERENCE grid whose class this arm disagrees with,
    /// and the total compared.
    disagree: usize,
    compared: usize,
    /// Derived, caveated: XY-projected mm² of non-shallow territory.
    non_shallow_mm2: f64,
}

fn topology(arm: &FinishSurface, reference: &FinishSurface) -> Topology {
    let hm = &arm.heightmap;
    let mut counts = [0usize; 3];
    let mut covered_cells = 0usize;
    for i in 0..hm.rows * hm.cols {
        if !hm.covered_flags()[i] {
            continue;
        }
        covered_cells += 1;
        counts[match class_of(arm.slope_map.angles[i]) {
            Class::Shallow => 0,
            Class::MidSteep => 1,
            Class::VerySteep => 2,
        }] += 1;
    }
    let denom = covered_cells.max(1) as f64;
    // Disagreement: walk the REFERENCE grid, look the arm's label up by
    // nearest cell. Deliberately reference-driven — the fine grid is the
    // ruler, so every arm is scored on the same population of points.
    let rhm = &reference.heightmap;
    let (mut disagree, mut compared) = (0usize, 0usize);
    for r in 0..rhm.rows {
        for c in 0..rhm.cols {
            let idx = r * rhm.cols + c;
            if !rhm.covered_flags()[idx] {
                continue;
            }
            let x = rhm.origin_x + c as f64 * rhm.cell_size;
            let y = rhm.origin_y + r as f64 * rhm.cell_size;
            let ac = ((x - hm.origin_x) / hm.cell_size).round();
            let ar = ((y - hm.origin_y) / hm.cell_size).round();
            if ac < 0.0 || ar < 0.0 {
                continue;
            }
            let (ac, ar) = (ac as usize, ar as usize);
            if ac >= hm.cols || ar >= hm.rows {
                continue;
            }
            let aidx = ar * hm.cols + ac;
            if !hm.covered_flags()[aidx] {
                continue;
            }
            compared += 1;
            if class_of(arm.slope_map.angles[aidx]) != class_of(reference.slope_map.angles[idx]) {
                disagree += 1;
            }
        }
    }
    Topology {
        cells: hm.rows * hm.cols,
        covered_cells,
        coverage: [
            counts[0] as f64 / denom,
            counts[1] as f64 / denom,
            counts[2] as f64 / denom,
        ],
        regions_mid: region_count(arm, Class::MidSteep),
        regions_very: region_count(arm, Class::VerySteep),
        disagree,
        compared,
        non_shallow_mm2: (counts[1] + counts[2]) as f64 * hm.cell_size * hm.cell_size,
    }
}

// ── Runners ─────────────────────────────────────────────────────────────

fn build(
    fixture: &Fixture,
    cutter: &dyn MillingCutter,
    policy: FinishResolutionPolicy,
) -> (FinishSurface, f64) {
    let cancel = || false;
    let t0 = Instant::now();
    let s = build_finish_surface_with_policy_and_cancel(
        &fixture.mesh,
        &fixture.index,
        cutter,
        policy,
        &cancel,
    )
    .expect("surface");
    (s, t0.elapsed().as_secs_f64())
}

struct OpRun {
    toolpath: Toolpath,
    seconds: f64,
    fingerprint: u64,
    uncut_core_mm2: Option<f64>,
    rings: Option<usize>,
    cusp: Option<(f64, f64)>,
}

fn run_scallop(
    fixture: &Fixture,
    cutter: &dyn MillingCutter,
    policy: FinishResolutionPolicy,
) -> OpRun {
    let cancel = || false;
    let params = scallop_params();
    let t0 = Instant::now();
    let (tp, anns, report) = scallop_toolpath_structured_annotated_with_resolution(
        &fixture.mesh,
        &fixture.index,
        cutter,
        &params,
        None,
        None,
        policy,
        &cancel,
    )
    .expect("scallop");
    let seconds = t0.elapsed().as_secs_f64();
    let cusp = measured_cusp(&tp, &anns, cutter.cusp_radius_mm());
    // Wave D3: the ring count is the REPORT's, not a count of the runtime
    // annotations this harness happened to also receive. The equality below
    // is what keeps the report honest now that it is the source.
    assert_eq!(
        report.ring_count,
        anns.len(),
        "ScallopReport::ring_count must equal the emitted ring annotations"
    );
    OpRun {
        fingerprint: fingerprint(&tp),
        toolpath: tp,
        seconds,
        uncut_core_mm2: Some(report.uncut_core_mm2),
        rings: Some(report.ring_count),
        cusp,
    }
}

fn run_ramp_finish(
    fixture: &Fixture,
    cutter: &dyn MillingCutter,
    policy: FinishResolutionPolicy,
) -> OpRun {
    let cancel = || false;
    let params = ramp_finish_params();
    let t0 = Instant::now();
    let (tp, _anns, _clamp) = ramp_finish_toolpath_structured_annotated_with_resolution(
        &fixture.mesh,
        &fixture.index,
        cutter,
        &params,
        None,
        None,
        policy,
        &cancel,
    )
    .expect("ramp finish");
    let seconds = t0.elapsed().as_secs_f64();
    OpRun {
        fingerprint: fingerprint(&tp),
        toolpath: tp,
        seconds,
        uncut_core_mm2: None,
        rings: None,
        cusp: None,
    }
}

fn run_steep_shallow(
    fixture: &Fixture,
    cutter: &dyn MillingCutter,
    policy: FinishResolutionPolicy,
) -> OpRun {
    let cancel = || false;
    let params = steep_shallow_params();
    let t0 = Instant::now();
    let (tp, split) = steep_shallow_toolpath_split_with_resolution(
        &fixture.mesh,
        &fixture.index,
        cutter,
        &params,
        None,
        policy,
        &cancel,
    )
    .expect("steep/shallow");
    let seconds = t0.elapsed().as_secs_f64();
    // The split is the op's own steep/shallow TOPOLOGY report: how many
    // moves each half got. Carried as "rings" so the table has one column.
    let steep_moves = split.steep.end - split.steep.start;
    OpRun {
        fingerprint: fingerprint(&tp),
        toolpath: tp,
        seconds,
        uncut_core_mm2: None,
        rings: Some(steep_moves),
        cusp: None,
    }
}

/// The three generation-surface consumers, as (label, runner) pairs.
#[allow(clippy::type_complexity)]
const OPS: [(
    &str,
    fn(&Fixture, &dyn MillingCutter, FinishResolutionPolicy) -> OpRun,
); 3] = [
    ("Scallop", run_scallop),
    ("RampFinish", run_ramp_finish),
    ("SteepShallow", run_steep_shallow),
];

// ── Fast subset (default CI) ────────────────────────────────────────────

/// The arms must actually be different grids, and the two NAMED arms must
/// keep their own provenance rather than degrading to `Explicit` — that is
/// what makes every table row below attributable to a policy.
#[test]
fn arms_are_distinct_grids_with_honest_provenance() {
    let t = taper();
    let a = arms(&t);
    assert_eq!(a.len(), 4);
    assert_eq!(
        a[0].policy.mode(),
        FinishResolutionMode::LegacyEnvelopeQuarter
    );
    assert_eq!(
        a[1].policy.mode(),
        FinishResolutionMode::GeoMeanEnvelopeCusp
    );
    assert_eq!(a[2].policy.mode(), FinishResolutionMode::CuspQuarter);
    assert!((a[0].policy.cell_mm() - 0.75).abs() < 1e-12);
    assert!((a[1].policy.cell_mm() - (0.75_f64 * 0.125).sqrt()).abs() < 1e-12);
    assert!((a[2].policy.cell_mm() - 0.125).abs() < 1e-12);
    assert!((a[3].policy.cell_mm() - TOLERANCE_MM).abs() < 1e-12);
    for arm in &a {
        assert!(
            !arm.policy.tolerance_floor_applied(),
            "{}: the tolerance floor must NOT bind, or two arms silently \
             become one grid",
            arm.name
        );
    }
    // Strictly coarse → fine, and the reference is finer than all of them.
    for w in a.windows(2) {
        assert!(
            w[0].policy.cell_mm() > w[1].policy.cell_mm() || w[0].name == "cusp/4",
            "{} vs {} out of order",
            w[0].name,
            w[1].name
        );
    }
    for arm in &a {
        assert!(REFERENCE_CELL_MM < arm.policy.cell_mm());
    }
    // Scallop's SHIPPED choice is arm 0 — the A/B's baseline is the default.
    assert_eq!(
        scallop_generation_resolution(&t, TOLERANCE_MM),
        a[0].policy,
        "the legacy arm must BE what scallop ships today"
    );
    // PR-8a: and ramp-finish's SHIPPED choice is arm 1. The evidence arm and
    // the production selector are now the same value by identity, so the
    // §3.2 table cannot drift away from what the op does.
    assert_eq!(
        ramp_finish_generation_resolution(&t, TOLERANCE_MM),
        a[1].policy,
        "the intermediate arm must BE what ramp_finish ships since PR-8a"
    );
}

/// **PR-8a's quality gate, restated by PR-8b.**
///
/// As landed, this gate asserted that the legacy cell reproduced
/// `CHECKPOINT_B_EVIDENCE.md` §3.2's two gouges (−2.3939 mm on the narrow
/// valley, −0.1621 mm on the narrow ridge) and that the shipped geo-mean
/// cell eliminated them. **PR-8b's reach clamp then eliminated them on the
/// LEGACY arm too**, because it clamps against an exact per-point
/// drop-cutter query and is therefore resolution-independent by
/// construction. The red half of the original gate is no longer
/// reproducible, and pinning a defect that a later commit fixed by a
/// different route would be a lie by omission.
///
/// So the assertions moved to the mechanism PR-8a actually buys, which
/// PR-8b does NOT subsume: **chord length**. The generation cell is the ramp
/// sampling step (`step_len = cell_size * 2`), and a ramp point is a
/// straight chord to the next one. The clamp guarantees the ENDPOINTS sit at
/// or above the reachable surface; it says nothing about the surface a
/// 1.06 mm chord cuts across on a 76° flank between them, and the residual
/// instrument — which scores endpoints — cannot see that either. Halving the
/// chord is the unsubsumed win, and it is asserted directly.
///
/// The gouge assertion is KEPT on the shipped arm. It is now redundant with
/// PR-8b on these fixtures, and it stays because it is the property the op
/// must have, not because it is the property that is hard to satisfy.
#[test]
fn ramp_finish_geo_mean_policy_halves_the_descent_chords() {
    let t = taper();
    let legacy = FinishResolutionPolicy::legacy_envelope_quarter(&t, TOLERANCE_MM);
    let shipped = ramp_finish_generation_resolution(&t, TOLERANCE_MM);
    assert_ne!(legacy, shipped, "the two arms must be different grids");
    let cell_ratio = legacy.cell_mm() / shipped.cell_mm();
    assert!((cell_ratio - (0.75_f64 / 0.125).sqrt()).abs() < 1e-12);

    for (name, mesh) in [
        ("narrow valley", narrow_valley()),
        ("narrow ridge", narrow_ridge()),
    ] {
        let fixture = Fixture::new("fixture", mesh);
        let (reference, _) = build(
            &fixture,
            &t,
            FinishResolutionPolicy::explicit(REFERENCE_CELL_MM),
        );

        let before_m = path_metrics(&run_ramp_finish(&fixture, &t, legacy).toolpath, None);
        let after = run_ramp_finish(&fixture, &t, shipped);
        let after_m = path_metrics(&after.toolpath, Some(&reference));

        // §3.2's own `min seg mm` column: the shortest emitted chord, which
        // that table recorded tracking the cell at ~1.5-1.9x. It is the
        // shortest and not the mean because `simplify_path_3d` collapses the
        // collinear stretches, so the mean is dominated by long straight
        // runs and moves only 1.25x (measured) — a mean-chord gate would
        // have been the wrong instrument and is recorded here as the one
        // that was tried.
        let chord_ratio = before_m.min_segment_mm / after_m.min_segment_mm;
        assert!(
            chord_ratio > 1.8,
            "{name}: shortest cutting chord {:.4} -> {:.4} mm is only \
             {chord_ratio:.2}x finer on a {cell_ratio:.2}x finer cell — the \
             cell has stopped driving the sampling step, which is the whole \
             mechanism PR-8a rests on",
            before_m.min_segment_mm,
            after_m.min_segment_mm
        );
        for (label, m, cell) in [
            ("legacy", &before_m, legacy.cell_mm()),
            ("shipped", &after_m, shipped.cell_mm()),
        ] {
            let per_cell = m.min_segment_mm / cell;
            assert!(
                (1.0..2.5).contains(&per_cell),
                "{name} {label}: shortest chord is {per_cell:.2}x its own \
                 cell, outside the 1.5-1.9x band §3.2 measured"
            );
        }
        assert!(
            after_m.residual_samples > 0,
            "{name}: nothing was scored on the shipped arm"
        );
        assert_eq!(
            after_m.deepest_gouge_mm, 0.0,
            "{name}: the shipped policy must leave NO cutting point below the \
             reference tool-centre surface; deepest {:.4} mm",
            after_m.deepest_gouge_mm
        );
        assert_eq!(after_m.gouge_over_50um, 0, "{name}");
        println!(
            "{name}: shortest cutting chord {:.4} -> {:.4} mm \
             ({chord_ratio:.2}x, cell {cell_ratio:.2}x); shipped-arm deepest \
             gouge {:.4} mm over {} scored points",
            before_m.min_segment_mm,
            after_m.min_segment_mm,
            after_m.deepest_gouge_mm,
            after_m.residual_samples,
        );
    }
}

/// Non-vacuity for the whole experiment: on the narrow ridge, the coarse and
/// fine arms must emit DIFFERENT toolpaths and DIFFERENT label topology.
/// If this ever passes with equal fingerprints, every table in the evidence
/// file is comparing a grid with itself.
#[test]
fn coarse_and_fine_arms_differ_on_the_narrow_ridge() {
    let fixture = Fixture::new("narrow ridge", narrow_ridge());
    let t = taper();
    let a = arms(&t);

    let coarse = run_scallop(&fixture, &t, a[0].policy);
    let fine = run_scallop(&fixture, &t, a[2].policy);
    assert!(!coarse.toolpath.moves.is_empty() && !fine.toolpath.moves.is_empty());
    assert_ne!(
        coarse.fingerprint, fine.fingerprint,
        "envelope/4 and cusp/4 produced byte-identical scallop output — the \
         resolution is not reaching generation"
    );

    let (coarse_s, _) = build(&fixture, &t, a[0].policy);
    let (fine_s, _) = build(&fixture, &t, a[2].policy);
    assert_eq!(coarse_s.cell_source, a[0].policy.cell_source());
    assert_eq!(fine_s.cell_source, a[2].policy.cell_source());
    // §A.0: state the difference as TOPOLOGY, not area.
    let coarse_very = region_count(&coarse_s, Class::VerySteep);
    let fine_very = region_count(&fine_s, Class::VerySteep);
    println!("narrow ridge VerySteep regions: envelope/4 {coarse_very}, cusp/4 {fine_very}");
    assert!(
        fine_very != coarse_very || fine_s.cols() != coarse_s.cols(),
        "the two grids must differ somewhere observable"
    );
}

/// Determinism guard: the legacy arm run twice must be byte-identical.
/// Without this, every A/B delta below could be run-to-run noise.
#[test]
fn legacy_arm_is_deterministic() {
    let fixture = Fixture::new("narrow ridge", narrow_ridge());
    let t = taper();
    let policy = arms(&t)[0].policy;
    let a = run_scallop(&fixture, &t, policy);
    let b = run_scallop(&fixture, &t, policy);
    assert_eq!(
        a.fingerprint, b.fingerprint,
        "the same arm produced two different toolpaths — the harness cannot \
         attribute any delta to resolution"
    );
    assert_eq!(a.toolpath.moves.len(), b.toolpath.moves.len());
    assert_eq!(a.uncut_core_mm2, b.uncut_core_mm2);
}

/// Ball control (H3 acceptance gate "Ball fixtures remain unchanged where
/// the policy resolves to the legacy cell size"): on a ball the cusp IS the
/// envelope, so the two NAMED arms are the same grid and must produce
/// byte-identical output. Stated as an EQUALITY so a future divergence is a
/// red test, not a shrug.
#[test]
fn ball_control_collapses_the_two_named_arms() {
    let fixture = Fixture::new("narrow ridge", narrow_ridge());
    let b = ball();
    let legacy = FinishResolutionPolicy::legacy_envelope_quarter(&b, TOLERANCE_MM);
    let cusp = FinishResolutionPolicy::cusp_quarter(&b, TOLERANCE_MM);
    assert!((legacy.cell_mm() - cusp.cell_mm()).abs() < 1e-12);
    assert_ne!(legacy.mode(), cusp.mode(), "the MODES still differ");

    let a = run_scallop(&fixture, &b, legacy);
    let c = run_scallop(&fixture, &b, cusp);
    assert!(!a.toolpath.moves.is_empty(), "non-vacuity");
    assert_eq!(
        a.fingerprint, c.fingerprint,
        "a ball's two named arms resolve to one cell, so they must emit one \
         toolpath"
    );
}

/// The controls: UnifiedFinish's VerySteep waterline and Shallow raster
/// bands consume NO generation surface, so the H3 variable cannot reach
/// them. Asserted structurally — the whole crate has exactly three
/// generation-surface consumers, and each has its own policy function.
///
/// (A code census, not a runtime probe: there is no API through which a
/// generation resolution could be handed to the waterline or raster band,
/// which is precisely the claim.)
#[test]
fn only_three_consumers_can_see_the_generation_resolution() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut consumers: Vec<String> = Vec::new();
    let mut stack = vec![src.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read src") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).expect("read rs");
                if path.file_name().is_some_and(|f| f == "finish_setup.rs") {
                    continue;
                }
                if text.contains("build_finish_surface_with_policy_and_cancel(")
                    || text.contains("build_finish_surface_with_cell_size_and_cancel(")
                {
                    consumers.push(
                        path.strip_prefix(&src)
                            .expect("under src")
                            .display()
                            .to_string(),
                    );
                }
            }
        }
    }
    consumers.sort();
    consumers.dedup();
    assert_eq!(
        consumers,
        vec![
            "ramp_finish.rs".to_owned(),
            "scallop.rs".to_owned(),
            "steep_shallow.rs".to_owned()
        ],
        "the set of generation-surface consumers changed — UnifiedFinish's \
         waterline/raster bands were resolution-INSENSITIVE because they \
         never build one, and this control is what keeps that true"
    );
}

// ── Full grid (#[ignore], the evidence run) ─────────────────────────────

/// The Checkpoint B evidence run. `cargo test -p rs_cam_core --test
/// checkpoint_b_resolution_ab -- --ignored --nocapture`.
///
/// Prints markdown tables straight into
/// `planning/review_2026-07-29/CHECKPOINT_B_EVIDENCE.md`'s shape.
#[test]
#[ignore = "full A/B grid — minutes; the Checkpoint B evidence run"]
fn full_resolution_ab_grid() {
    let t = taper();
    let arms = arms(&t);

    println!("\n## Arms (tolerance {TOLERANCE_MM} mm, reference cell {REFERENCE_CELL_MM} mm)\n");
    println!("| arm | mode | cell mm | cell source |");
    println!("|---|---|---|---|");
    for arm in &arms {
        println!(
            "| {} | {:?} | {:.4} | {:?} |",
            arm.name,
            arm.policy.mode(),
            arm.policy.cell_mm(),
            arm.policy.cell_source()
        );
    }

    for fixture in fixtures() {
        println!("\n# Fixture: {}\n", fixture.name);
        let (reference, ref_secs) = build(
            &fixture,
            &t,
            FinishResolutionPolicy::explicit(REFERENCE_CELL_MM),
        );
        println!(
            "reference grid {}×{} = {} cells, built in {:.2} s\n",
            reference.rows(),
            reference.cols(),
            reference.rows() * reference.cols(),
            ref_secs
        );
        let ref_topo = topology(&reference, &reference);
        println!(
            "reference topology: covered {} cells, coverage shallow {:.3} / mid {:.3} / very {:.3}, \
             regions mid {} / very {}, non-shallow {:.1} mm² (AREA IS CAVEATED — §A.0)\n",
            ref_topo.covered_cells,
            ref_topo.coverage[0],
            ref_topo.coverage[1],
            ref_topo.coverage[2],
            ref_topo.regions_mid,
            ref_topo.regions_very,
            ref_topo.non_shallow_mm2
        );

        println!("\n## {} — label-grid topology (§A.0)\n", fixture.name);
        println!(
            "| arm | cell mm | grid | cells | build s | cov shallow | cov mid | cov very | regions mid | regions very | disagree vs ref | non-shallow mm² (caveat) |"
        );
        println!("|---|---|---|---|---|---|---|---|---|---|---|---|");
        for arm in &arms {
            let (surface, secs) = build(&fixture, &t, arm.policy);
            let topo = topology(&surface, &reference);
            println!(
                "| {} | {:.4} | {}×{} | {} | {:.2} | {:.3} | {:.3} | {:.3} | {} | {} | {} / {} = {:.1}% | {:.1} |",
                arm.name,
                arm.policy.cell_mm(),
                surface.rows(),
                surface.cols(),
                topo.cells,
                secs,
                topo.coverage[0],
                topo.coverage[1],
                topo.coverage[2],
                topo.regions_mid,
                topo.regions_very,
                topo.disagree,
                topo.compared,
                100.0 * topo.disagree as f64 / topo.compared.max(1) as f64,
                topo.non_shallow_mm2
            );
        }

        for (op_name, run) in OPS {
            println!("\n## {} — {}\n", fixture.name, op_name);
            println!(
                "| arm | cell mm | gen s | moves | cutting | min seg mm | degenerate | |res| p50 | p95 | p99 | max | deepest gouge | gouge>50µm | rapid grazes | rings/steep moves | uncut core mm² | cusp p50/p95 | fingerprint |"
            );
            println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
            for arm in &arms {
                let r = run(&fixture, &t, arm.policy);
                let m = path_metrics(&r.toolpath, Some(&reference));
                println!(
                    "| {} | {:.4} | {:.2} | {} | {} | {:.4} | {} | {:.4} | {:.4} | {:.4} | {:.4} | {:.4} | {} | {} | {} | {} | {} | {:016x} |",
                    arm.name,
                    arm.policy.cell_mm(),
                    r.seconds,
                    m.moves,
                    m.cut_moves,
                    m.min_segment_mm,
                    m.degenerate_segments,
                    m.residual_p50_abs,
                    m.residual_p95_abs,
                    m.residual_p99_abs,
                    m.residual_max_abs,
                    m.deepest_gouge_mm,
                    m.gouge_over_50um,
                    m.rapid_grazes,
                    r.rings.map_or("-".to_owned(), |v| v.to_string()),
                    r.uncut_core_mm2
                        .map_or("-".to_owned(), |v| format!("{v:.2}")),
                    r.cusp
                        .map_or("-".to_owned(), |(a, b)| format!("{a:.4}/{b:.4}")),
                    r.fingerprint
                );
            }
        }
    }
    println!(
        "\n(commanded scallop cusp height: {} mm)",
        scallop_params().scallop_height
    );
}

/// **PR-8b's gate**, on the fixture `CHECKPOINT_B_EVIDENCE.md` §8.2 logged
/// as an adjacent defect: "RampFinish gouges 4.2 mm on `patches + hole` at
/// every resolution ... a genuine reach failure with **no diagnostic
/// channel**; a user would ship it."
///
/// The RED EVIDENCE is the clamp's own `max_lift_mm`: it is exactly how far
/// below the reachable surface the unclamped descent went, measured at the
/// moment it was prevented, so the defect stays visible in the fixture that
/// fixed it rather than only in prose. Asserted at the §8.2 magnitude.
#[test]
fn ramp_finish_reach_clamp_removes_the_cone_fixture_gouge() {
    let t = taper();
    let fixture = Fixture::new("patches + hole", disconnected_patches());
    let policy = ramp_finish_generation_resolution(&t, TOLERANCE_MM);
    let params = RampFinishParams {
        max_stepdown: 0.5,
        tolerance: TOLERANCE_MM,
        ..Default::default()
    };
    let cancel = || false;
    let (tp, _anns, clamp) = ramp_finish_toolpath_structured_annotated_with_resolution(
        &fixture.mesh,
        &fixture.index,
        &t,
        &params,
        None,
        None,
        policy,
        &cancel,
    )
    .expect("ramp finish");

    // RED, kept live: the descent really did ask for 4.2 mm of unreachable
    // depth, on 74% of its points, and the ladder bottom really was the mesh
    // bbox floor rather than anything holdable.
    assert!(
        clamp.max_lift_mm > 4.0,
        "the §8.2 defect is no longer reproduced by this fixture (max lift \
         {:.4} mm) — the gate is measuring nothing",
        clamp.max_lift_mm
    );
    assert!(clamp.clamped_points * 2 > clamp.ramp_points);
    assert!((clamp.requested_bottom_z_mm - fixture.mesh.bbox.min.z).abs() < 1e-9);
    assert!(
        clamp.holdable_bottom_z_mm > clamp.requested_bottom_z_mm + 0.5,
        "the ladder bottom must have been lifted off the mesh bbox floor"
    );
    assert!(!clamp.is_inert());

    // GREEN: nothing survives below the reference tool-centre surface beyond
    // the RULER's own error. The residual bound is the reference field's
    // bilinear interpolation error at 0.05 mm (§5.1), not the path's — the
    // clamp queries the mesh exactly, with no grid in between, which is why
    // it holds at every resolution.
    let (reference, _) = build(
        &fixture,
        &t,
        FinishResolutionPolicy::explicit(REFERENCE_CELL_MM),
    );
    let m = path_metrics(&tp, Some(&reference));
    assert!(
        m.residual_samples > 50,
        "non-vacuity: {} scored",
        m.residual_samples
    );
    assert!(
        m.deepest_gouge_mm > -0.05,
        "deepest gouge {:.4} mm — §8.2's 4.2 mm defect is not fixed",
        m.deepest_gouge_mm
    );
    println!(
        "patches + hole: clamp lifted {} / {} points by up to {:.4} mm, \
         ladder bottom {:.4} -> {:.4}; deepest surviving gouge {:.4} mm \
         over {} scored points",
        clamp.clamped_points,
        clamp.ramp_points,
        clamp.max_lift_mm,
        clamp.requested_bottom_z_mm,
        clamp.holdable_bottom_z_mm,
        m.deepest_gouge_mm,
        m.residual_samples,
    );
}

// ── PR-8c: the max_rings experiment (EVIDENCE ONLY) ─────────────────────

/// **Research, not a gate.** Checkpoint B's ruling: "`max_rings` budget
/// derived from SELECTED stepover as an H3-scoped EXPERIMENT first (adopt
/// only if standing -> 0 with no over-cut regression; remember v3: naive cap
/// raise was 34× worse)."
///
/// §3.1 finding 3 is what this answers: at `cusp/4` scallop leaves 19.32 mm²
/// standing on the narrow ridge and 33.24 mm² on the mixed ribbon where
/// `envelope/4` leaves none, because `max_rings` is budgeted from the
/// FLAT-GROUND (widest) stepover while the loop selects a smaller one per
/// ring on sloped terrain.
///
/// Three budgets are run at the cusp/4 arm — the one that exhibits the
/// truncation — on all four fixtures:
///
/// * `FlatGroundStepover` — shipped baseline.
/// * `ReachPolicyStepover` — the ruling's "selected stepover", read through
///   `reach::suggested_offset_stepover_mm` for consistency with PR-6a.
/// * `LoopClampFloor` — the NAIVE raise, as the control. v3 measured this at
///   +92% time and 34× deep over-cut on wanaka ×2 and the `scallop.rs`
///   comment records it; reproducing it here on these fixtures is what makes
///   the other two rows interpretable instead of merely reported.
///
/// **Both halves of the ruling's adopt condition are measured.** Standing
/// material comes from `ScallopReport::uncut_core_mm2`. Over-cut is scored
/// against the pinned 0.05 mm reference field — and it is reported with §5.1's
/// caveat attached, which is load-bearing here: scallop's ring Z is an exact
/// per-point drop-cutter query, so its residual column is dominated by the
/// REFERENCE grid's own interpolation error at whatever points the path
/// happens to visit, and a denser path visits more sharp-feature cells. The
/// column is therefore usable to detect a LARGE regression (v3's 34×) and
/// must not be read as an absolute over-cut count. Ring count and generation
/// time carry the rest of the cost story.
///
/// Prints markdown for the evidence addendum. `#[ignore]`: minutes.
#[test]
#[ignore = "PR-8c max_rings experiment — minutes; evidence only, adopts nothing"]
fn max_rings_budget_experiment() {
    let t = taper();
    let cusp_arm = FinishResolutionPolicy::cusp_quarter(&t, TOLERANCE_MM);
    let params = scallop_params();
    let cancel = || false;

    println!("\n## max_rings experiment — cusp/4 arm, tapered Ø1/7°/Ø6\n");
    println!(
        "| fixture | budget | gen s | rings emitted | cascade rings | moves | \
         uncut core mm² | deepest gouge | gouge>50µm | min seg mm |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|");
    for fixture in fixtures() {
        let (reference, _) = build(
            &fixture,
            &t,
            FinishResolutionPolicy::explicit(REFERENCE_CELL_MM),
        );
        for (label, budget) in [
            (
                "flat-ground (shipped)",
                ScallopRingBudget::FlatGroundStepover,
            ),
            ("reach policy", ScallopRingBudget::ReachPolicyStepover),
            (
                "loop clamp floor (v3 control)",
                ScallopRingBudget::LoopClampFloor,
            ),
        ] {
            let t0 = Instant::now();
            let (tp, _anns, report) =
                scallop_toolpath_structured_annotated_with_resolution_and_ring_budget(
                    &fixture.mesh,
                    &fixture.index,
                    &t,
                    &params,
                    None,
                    None,
                    cusp_arm,
                    budget,
                    &cancel,
                )
                .expect("scallop");
            let secs = t0.elapsed().as_secs_f64();
            let m = path_metrics(&tp, Some(&reference));
            println!(
                "| {} | {label} | {secs:.2} | {} | {} | {} | {:.2} | {:.4} | {} | {:.4} |",
                fixture.name,
                report.ring_count,
                report.cascade_ring_count,
                m.moves,
                report.uncut_core_mm2,
                m.deepest_gouge_mm,
                m.gouge_over_50um,
                m.min_segment_mm,
            );
        }
    }
    println!(
        "\n(commanded cusp height {} mm; reference cell {REFERENCE_CELL_MM} mm; \
         residual column carries §5.1's scallop caveat)",
        params.scallop_height
    );
}

/// The experiment's own non-vacuity guard, FAST and not ignored: the three
/// budgets must be three different ring caps on the shipped tool, or every
/// row of the table above is the same run three times.
#[test]
fn the_three_ring_budgets_are_three_different_numbers() {
    let t = taper();
    let params = scallop_params();
    let cusp_r = t.cusp_radius_mm();
    let clamp_floor = cusp_r * 0.05;
    let flat = rs_cam_core::scallop_math::stepover_from_scallop_flat(cusp_r, params.scallop_height)
        .max(clamp_floor);
    let reach = rs_cam_core::reach::suggested_offset_stepover_mm(&t, 0.0).max(clamp_floor);
    assert!(
        clamp_floor < reach && reach < flat,
        "the budgets must ORDER floor < reach < flat for the experiment to be \
         a ladder: {clamp_floor:.4} / {reach:.4} / {flat:.4}"
    );
    // And the shipped budget must still be the flat-ground one: the seam is
    // additive, so the default cannot have moved.
    assert!(
        (flat - 0.2800).abs() < 0.001,
        "flat-ground stepover {flat:.4}"
    );
    println!(
        "ring-budget stepovers: flat-ground {flat:.4} mm (shipped), reach \
         policy {reach:.4} mm, loop clamp floor {clamp_floor:.4} mm"
    );
}
