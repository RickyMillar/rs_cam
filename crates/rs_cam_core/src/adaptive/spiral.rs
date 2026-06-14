//! Inside-out contour-spiral path generation (Stage 1 of the adaptive
//! algorithm review, `planning/ADAPTIVE_CLEARING_ALGO_REVIEW_2026-06-12.md`
//! §3). Constructive alternative to the reactive per-step agent in
//! `path.rs`: derive the global structure of the region first (EDT offset
//! family), then emit a single stay-down pass whose radial engagement is
//! bounded *by construction* (wrap spacing = stepover).
//!
//! Shape of the output: after the shared helical starter pocket opens a
//! 2 × R disc at the EDT maximum, wraps are iso-contours of the
//! machinable-region EDT at stepover increments, traced inside-out as ONE
//! continuous `Cut` — each wrap cuts with the cleared region on its inner
//! side (radial WOC = stepover), and the only discontinuities are short
//! radial seam steps (≤ ~stepover) between consecutive wraps. One plunge
//! per region instead of per-ring restarts.
//!
//! Stage 2 (same review, §5): trochoidal inserts. Where the predicted
//! leading-arc engagement along a wrap exceeds the cap — concave-corner
//! wrap-around, EDT side-branch first contact — the straight traversal
//! switches to circular loops biased toward the cleared side, bounding
//! the instantaneous bite by the loop pitch instead of the local
//! material width.
//!
//! Out of scope here (handled by the shared machinery or later stages):
//! - residue, side lobes not containing the EDT max, and island-collar
//!   rings — the `ContourParallelHybrid` residue cleanup mops them;
//! - narrow regions — the caller's narrow gate routes those to
//!   contour-parallel before this module is reached, and regions whose
//!   EDT max can't fit the starter pocket fall back to the agent;
//! - true cycloid advance (the residual ~1% loop-tangent transient) and
//!   medial-axis trochoids for slot-class regions.

use crate::geo::P2;
use crate::interrupt::{CancelCheck, Cancelled, check_cancel};
use crate::polygon::Polygon2;

use super::AdaptiveRuntimeEvent;
use super::material_grid::MaterialGrid;
use super::path::AdaptiveSegment;

/// Resampling spacing along wraps, in grid cells.
const WRAP_SAMPLE_CELLS: f64 = 1.5;
/// EDT floor (in cells) for the outermost wrap — a hair inside the
/// machinable boundary so marching squares still closes the loop.
const OUTER_WRAP_FLOOR_CELLS: f64 = 0.5;

/// Generate inside-out spiral wraps over the machinable region.
///
/// Returns `false` when the spiral is not applicable (no starter pocket
/// position, degenerate EDT) — the caller falls back to the agent loop.
/// On success, appends one `Rapid`-free continuous `Cut` (the caller's
/// helical starter already positioned the tool) plus a `PassEntry`
/// marker, stamps the grid along the path, and updates `last_pos`.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::indexing_slicing)] // bounded indexing over grid/loop buffers
pub(super) fn spiral_passes(
    grid: &mut MaterialGrid,
    machinable_mask: &[bool],
    tool_radius: f64,
    stepover: f64,
    starter_end: P2,
    segments: &mut Vec<AdaptiveSegment>,
    last_pos: &mut Option<P2>,
    // Stage 4 — when `Some`, the predicted leading-arc engagement (α/2π)
    // computed at every emitted cut point is collected here, 1:1 with the
    // emitted Cut path. Consumed by the 3D assembly to build the
    // planner-engagement sampler the feed modulator reads (see
    // planning/ADAPTIVE_CLEARING_ALGO_REVIEW_2026-06-12.md §"Stage 4").
    eng_sink: Option<&mut Vec<(P2, f64)>>,
    cancel: &dyn CancelCheck,
) -> Result<bool, Cancelled> {
    let cell = grid.cell_size;
    let (rows, cols) = (grid.rows, grid.cols);

    // EDT over the machinable mask: distance (world units) from each
    // machinable cell to the nearest non-machinable cell. The mask grid
    // carries a non-machinable margin border, so the transform is
    // well-defined.
    let inverse: Vec<bool> = machinable_mask.iter().map(|&m| !m).collect();
    let mut edt = crate::contour_extract::distance_transform_2d(&inverse, rows, cols);
    for d in &mut edt {
        *d *= cell;
    }

    // Spiral center: the EDT maximum (same landmark the helical starter
    // pocket targets via the material-grid distance field).
    let mut d_max = 0.0f64;
    let mut center = starter_end;
    for row in 0..rows {
        for col in 0..cols {
            let d = edt[row * cols + col];
            if d > d_max {
                d_max = d;
                center = P2::new(
                    grid.origin_x + col as f64 * cell,
                    grid.origin_y + row as f64 * cell,
                );
            }
        }
    }
    if d_max <= stepover {
        return Ok(false);
    }

    // Wrap offsets from the center outward. The starter pocket cleared a
    // 2R disc, so the first wrap rides flush with its edge (cutter at
    // ρ = R clears to 2R — zero bite, establishes position); subsequent
    // wraps bite exactly one stepover.
    let first_offset = tool_radius.min(d_max);
    let floor = (cell * OUTER_WRAP_FLOOR_CELLS).min(d_max * 0.5);
    let mut offsets: Vec<f64> = Vec::new();
    let mut off = first_offset;
    while d_max - off > floor {
        offsets.push(off);
        off += stepover;
    }
    // Outermost wrap hugs the machinable boundary.
    offsets.push(d_max - floor);
    if offsets.is_empty() {
        return Ok(false);
    }

    let sample_step = cell * WRAP_SAMPLE_CELLS;
    let mut path: Vec<P2> = Vec::new();
    // Stage 4 — predicted leading-arc engagement per emitted path point,
    // kept 1:1 with `path`. Only materialised when a sink was supplied.
    let mut path_engs: Vec<f64> = Vec::new();
    let collect_eng = eng_sink.is_some();
    let mut cur = starter_end;

    // Stage 2: load excursions — concave-corner wrap-around and EDT
    // side-branch first contact — are absorbed by trochoidal inserts.
    // Where the predicted leading-arc engagement exceeds the cap, the
    // straight traversal is replaced by circular loops biased toward the
    // cleared side, advancing at a pitch that bounds the radial bite.
    // (A drop-the-points filter was tried first and reverted: drops
    // cascade into the next wrap and swiss-cheese the coverage. The
    // trochoid keeps cutting — it just caps the instantaneous bite.)
    // Tuning measured by the property harness (2026-06-13): cap 1.2× /
    // pitch 0.6×s holds >96% of samples under 1.3×target with p99
    // ≈ 0.31–0.35 at ~2–2.7× the agent's cutting distance. Tightening to
    // 1.1× / 0.4×s only nudged p99 (structural ~1% at loop-tangent
    // instants) while inflating cutting distance another ~60% — the
    // residual transient is cycloid-advance / feed-modulation territory,
    // not pitch territory.
    let target = crate::adaptive_shared::target_engagement_fraction(stepover, tool_radius);
    let eng_cap = (target * 1.2).min(0.45);
    let troch = TrochoidParams {
        radius: stepover.max(tool_radius * 0.4),
        pitch: stepover * 0.6,
        cap: eng_cap,
    };

    for &offset in &offsets {
        check_cancel(cancel)?;
        let tau = d_max - offset;
        let Some(wrap) = extract_center_wrap(&edt, grid, tau, center) else {
            continue;
        };
        let wrap = resample_loop(&wrap, sample_step);
        if wrap.len() < 4 {
            continue;
        }
        // Seam: rotate the wrap to start at the point nearest the
        // current position, lap it fully, and close back on the seam.
        // Each lap cuts with the cleared region on its inner side
        // (radial WOC = stepover); the seam step is a short radial feed.
        let seam = nearest_index(&wrap, cur);
        let n = wrap.len();
        // `since_loop` is ∞ while cutting straight so the first heavy
        // sample emits a loop immediately; afterwards loops repeat every
        // `pitch` of heavy arc length.
        let mut since_loop = f64::INFINITY;
        let mut prev_p = cur;
        for k in 0..=n {
            let p = wrap[(seam + k) % n];
            let step_len = {
                let dx = p.x - prev_p.x;
                let dy = p.y - prev_p.y;
                (dx * dx + dy * dy).sqrt()
            };
            let dir = (p.y - prev_p.y).atan2(p.x - prev_p.x);
            let eng = super::search::compute_engagement_arc(grid, p.x, p.y, tool_radius, dir);
            if eng <= troch.cap {
                path.push(p);
                if collect_eng {
                    path_engs.push(eng);
                }
                grid.clear_circle(p.x, p.y, tool_radius);
                since_loop = f64::INFINITY;
            } else {
                since_loop = if since_loop.is_finite() {
                    since_loop + step_len
                } else {
                    troch.pitch
                };
                if since_loop >= troch.pitch {
                    let before = path.len();
                    emit_trochoid_loop(grid, tool_radius, troch.radius, p, &mut path);
                    if collect_eng {
                        // Trochoid loop points are bounded by the engagement
                        // cap by construction; tag each with the trigger
                        // engagement so the sampler reads ~cap there.
                        path_engs.resize(before, 0.0);
                        path_engs.resize(path.len(), eng);
                    }
                    since_loop = 0.0;
                }
            }
            prev_p = p;
        }
        cur = wrap[seam];
    }

    if path.len() < 4 {
        return Ok(false);
    }

    let entry = path[0];
    segments.push(AdaptiveSegment::Marker(AdaptiveRuntimeEvent::PassEntry {
        pass_index: 1,
        entry_x: entry.x,
        entry_y: entry.y,
    }));
    let end = path[path.len() - 1];
    if let Some(sink) = eng_sink {
        // 1:1 with `path` by construction; defensively zip to the shorter.
        sink.extend(path.iter().copied().zip(path_engs.iter().copied()));
    }
    segments.push(AdaptiveSegment::Cut(path));
    *last_pos = Some(end);
    Ok(true)
}

/// Trochoidal-insert tuning.
struct TrochoidParams {
    /// Loop radius (mm). Loops are tangent to the nominal wrap at the
    /// trigger point and extend toward the cleared side.
    radius: f64,
    /// Heavy-arc length between consecutive loops (mm) — the per-loop
    /// frontier advance, which bounds the bite per revolution.
    pitch: f64,
    /// Leading-arc engagement (α/2π) above which the straight traversal
    /// switches to loops.
    cap: f64,
}

/// Emit one trochoid loop tangent to the nominal point `p`, extending
/// toward the most-cleared side. The loop sweeps from cleared material
/// into the frontier, so the instantaneous bite is bounded by the
/// frontier advance (`pitch`) rather than the local material width.
/// Skipped (straight cut instead) when no cleared side exists nearby —
/// a full-material loop would just be a circular slot.
#[allow(clippy::indexing_slicing)] // fixed-size direction/loop sampling
fn emit_trochoid_loop(
    grid: &mut MaterialGrid,
    tool_radius: f64,
    loop_radius: f64,
    p: P2,
    path: &mut Vec<P2>,
) {
    // Pick the offset direction whose tool disk reads the least material.
    let mut best_dir = (0.0f64, 0.0f64);
    let mut best_fill = f64::INFINITY;
    for i in 0..16 {
        let theta = (i as f64 / 16.0) * std::f64::consts::TAU;
        let (dx, dy) = (theta.cos(), theta.sin());
        let fill = super::search::compute_engagement(
            grid,
            p.x + dx * loop_radius,
            p.y + dy * loop_radius,
            tool_radius,
        );
        if fill < best_fill {
            best_fill = fill;
            best_dir = (dx, dy);
        }
    }
    if best_fill > 0.8 {
        // Nowhere cleared in reach: looping would slot a full circle.
        path.push(p);
        grid.clear_circle(p.x, p.y, tool_radius);
        return;
    }
    let center = P2::new(
        p.x + best_dir.0 * loop_radius,
        p.y + best_dir.1 * loop_radius,
    );
    let theta0 = (p.y - center.y).atan2(p.x - center.x);
    let n = 20;
    for i in 0..=n {
        let theta = theta0 + (i as f64 / n as f64) * std::f64::consts::TAU;
        let q = P2::new(
            center.x + loop_radius * theta.cos(),
            center.y + loop_radius * theta.sin(),
        );
        path.push(q);
        grid.clear_circle(q.x, q.y, tool_radius);
    }
}

/// Marching-squares iso-contour of `edt > tau`, selecting the loop that
/// contains `center` (the spiral's nesting chain). Falls back to the
/// longest loop when containment fails numerically.
#[allow(clippy::indexing_slicing)] // padded-grid indices bounded by construction
fn extract_center_wrap(edt: &[f64], grid: &MaterialGrid, tau: f64, center: P2) -> Option<Vec<P2>> {
    let (rows, cols) = (grid.rows, grid.cols);
    let (prows, pcols) = (rows + 2, cols + 2);
    let mut padded = vec![false; prows * pcols];
    for row in 0..rows {
        for col in 0..cols {
            if edt[row * cols + col] > tau {
                padded[(row + 1) * pcols + (col + 1)] = true;
            }
        }
    }
    let loops = crate::contour_extract::marching_squares_bool_grid(
        &padded,
        prows,
        pcols,
        grid.origin_x - grid.cell_size,
        grid.origin_y - grid.cell_size,
        grid.cell_size,
    );
    if loops.is_empty() {
        return None;
    }
    let mut best_len = 0usize;
    let mut best: Option<&Vec<P2>> = None;
    for candidate in &loops {
        if candidate.len() < 4 {
            continue;
        }
        if Polygon2::new(candidate.clone()).contains_point(&center) {
            return Some(candidate.clone());
        }
        if candidate.len() > best_len {
            best_len = candidate.len();
            best = Some(candidate);
        }
    }
    best.cloned()
}

/// Resample a closed loop to roughly uniform `step` spacing.
#[allow(clippy::indexing_slicing)] // loop indices bounded by len
fn resample_loop(loop_pts: &[P2], step: f64) -> Vec<P2> {
    let n = loop_pts.len();
    if n < 3 {
        return loop_pts.to_vec();
    }
    let mut out = Vec::with_capacity(n);
    let mut carried = 0.0f64;
    out.push(loop_pts[0]);
    for i in 0..n {
        let a = loop_pts[i];
        let b = loop_pts[(i + 1) % n];
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-12 {
            continue;
        }
        let mut along = step - carried;
        while along < len {
            let t = along / len;
            out.push(P2::new(a.x + t * dx, a.y + t * dy));
            along += step;
        }
        carried = len - (along - step);
    }
    out
}

/// Index of the loop point nearest to `p`.
fn nearest_index(pts: &[P2], p: P2) -> usize {
    let mut best = 0usize;
    let mut best_d = f64::INFINITY;
    for (i, q) in pts.iter().enumerate() {
        let dx = q.x - p.x;
        let dy = q.y - p.y;
        let d = dx * dx + dy * dy;
        if d < best_d {
            best_d = d;
            best = i;
        }
    }
    best
}
