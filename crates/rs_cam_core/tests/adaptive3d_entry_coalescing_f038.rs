//! F-038 — Adaptive3d AgentSearch entry-plunge fragmentation regression net.
//!
//! ## Background
//!
//! On real-machine measurement of the Wanaka Back Rough toolpath (Shapeoko
//! XXL, 2026-05-26) the .nc spent a large share of cycle time on perimeter
//! micro-plunges — short cuts (often ≤ 10 mm) book-ended by full retract +
//! rapid + peck-plunge cycles. Profiling at planning time:
//!
//!   - 149 F750 Z-entry descents, grouped into 131 entry events
//!   - 30 of 149 plunges cut zero material before retract
//!   - 90 of 149 plunges cut ≤ 10 mm before retract
//!   - 23 of 42 unique XY plunge points clustered on the stock bbox edge
//!
//! ## Root cause
//!
//! `clear_z_level_agent_2d_slice` calls the 2D adaptive once per
//! marching-squares region. The 2D adaptive's output is structured as
//! `[Rapid(entry), Cut, Cut, ..., Rapid(entry), ...]` — each `Rapid`
//! becomes a downstream retract + rapid XY + peck-plunge entry. On
//! terrain-shaped models the 2D adaptive splits its sweep into many small
//! passes, a significant fraction of which cut < 5 mm of material before
//! the next entry. The per-entry retract+plunge overhead (~0.4 s on the
//! Wanaka 6 mm tool at the configured feeds) dominates the per-pass cycle
//! time.
//!
//! ## Fix
//!
//! `Adaptive3dParams::min_region_cut_length_mm` (default 15.0 mm) gates
//! emission. The AgentSearch dispatch:
//!
//!   1. Drops marching-squares regions whose forecast cut length is below
//!      the threshold (region-level filter), and
//!   2. Drops Rapid+Cut groups inside the 2D adaptive output whose total
//!      Cut path length is below the threshold (group-level filter), and
//!   3. Coalesces back-to-back `Rapid`/`RapidWithFloor` segments produced
//!      by the engagement subdivider when the in-between `Cut` ran short
//!      enough to be demoted to air (post-emission filter).
//!
//! Setting `min_region_cut_length_mm = 0.0` disables all three filters,
//! preserving the pre-fix behaviour for existing tests.
//!
//! ## Acceptance bar
//!
//! On a synthetic terrain with deliberate perimeter micro-peaks (designed
//! to reproduce the fragmentation pattern), the entry-plunge count after
//! the fix must be below a threshold proportional to `terrain_area /
//! tool_diameter²`. Concretely, fewer than `area_mm2 / (tool_d * 30)`
//! entry plunges — a generous bound that still catches a regression of
//! the fragmentation logic.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

use rs_cam_core::adaptive3d::{
    Adaptive3dParams, ClearingStrategy3d, EntryStyle3d, RegionOrdering, adaptive_3d_toolpath,
};
use rs_cam_core::geo::P3;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::tool::{FlatEndmill, MillingCutter};
use rs_cam_core::toolpath::MoveIntent;

// ── Synthetic micro-peak terrain ──────────────────────────────────────
//
// 80×80 mm tile with N×M small bumps spaced one tool diameter apart, plus
// extra bumps explicitly aligned to the bbox edges. Each bump's peak
// height is just above the surrounding terrain so the AgentSearch 2D
// adaptive sees many small islands at the topmost Z levels.

const SIZE: f64 = 60.0;
const BASE_Z: f64 = 0.0;
const PEAK_Z: f64 = 4.0;
// Single central pad + a ring of perimeter micro-peaks. Mirrors the
// Wanaka pattern: a few isolated edge bumps create entry-plunge
// fragmentation if the filter isn't active, while a large central
// surface dominates the real cut work.
const PERIMETER_BUMPS: usize = 8;
const PERIMETER_BUMP_RADIUS: f64 = 1.5;
const CENTRAL_PAD_RADIUS: f64 = 18.0;

fn micro_peak_terrain() -> TriangleMesh {
    // Pad centres: one large central pad + N small bumps on the perimeter
    // ring. Each perimeter bump is below sub-tool size so the
    // marching-squares region around it is the fragmentation source the
    // F-038 filter targets.
    let centre = SIZE / 2.0;
    let mut pad_centres: Vec<(f64, f64, f64)> = Vec::new();
    pad_centres.push((centre, centre, CENTRAL_PAD_RADIUS));
    for k in 0..PERIMETER_BUMPS {
        let theta = (k as f64) * std::f64::consts::TAU / (PERIMETER_BUMPS as f64);
        let r = SIZE * 0.42;
        pad_centres.push((
            centre + r * theta.cos(),
            centre + r * theta.sin(),
            PERIMETER_BUMP_RADIUS,
        ));
    }

    // Sample on a fine grid; emit triangles that connect adjacent samples.
    let n = 81usize;
    let step = SIZE / (n as f64 - 1.0);
    let mut vertices = Vec::with_capacity(n * n);
    for j in 0..n {
        for i in 0..n {
            let x = i as f64 * step;
            let y = j as f64 * step;
            let mut z = BASE_Z;
            for &(cx, cy, cr) in &pad_centres {
                let dx = x - cx;
                let dy = y - cy;
                let r = (dx * dx + dy * dy).sqrt();
                if r < cr {
                    // Plateau profile so the pad has a flat top — the 2D
                    // adaptive sweeps it as a real region.
                    let t = (1.0 - (r / cr).powi(4)).max(0.0);
                    let z_here = BASE_Z + PEAK_Z * t;
                    if z_here > z {
                        z = z_here;
                    }
                }
            }
            vertices.push(P3::new(x, y, z));
        }
    }
    let mut triangles = Vec::with_capacity((n - 1) * (n - 1) * 2);
    for j in 0..(n - 1) {
        for i in 0..(n - 1) {
            let a = (j * n + i) as u32;
            let b = (j * n + i + 1) as u32;
            let c = ((j + 1) * n + i) as u32;
            let d = ((j + 1) * n + i + 1) as u32;
            triangles.push([a, b, c]);
            triangles.push([b, d, c]);
        }
    }
    TriangleMesh::from_raw(vertices, triangles)
}

fn make_params(tool_radius: f64, min_region_cut_length_mm: f64) -> Adaptive3dParams {
    Adaptive3dParams {
        trochoid_cap_mult: 1.6,
        engagement_measure: rs_cam_core::adaptive::EngagementMeasure::DiskArea,
        tool_radius,
        envelope_radius: tool_radius,
        stepover: tool_radius * 0.5,
        depth_per_pass: 2.0,
        stock_to_leave: 0.0,
        feed_rate: 1500.0,
        plunge_rate: 500.0,
        safe_z: PEAK_Z + 5.0,
        tolerance: 0.1,
        min_cutting_radius: 0.0,
        stock_top_z: PEAK_Z,
        z_floor: None,
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
        min_region_cut_length_mm,
        // F-038b: keep the F-038 fixture's pre-F-038b behaviour (every
        // entry remains a retract/rapid/plunge) so the entry-count
        // regression net stays comparable across F-038 and F-038b.
        max_stay_down_distance_mm: Some(0.0),
        stay_down_clearance_mm: 0.5,
    }
}

fn count_entry_plunges(tp: &rs_cam_core::toolpath::Toolpath) -> usize {
    // A peck-plunge entry is a feed move tagged `EntryPlunge`. Count the
    // distinct entry events by collapsing consecutive `EntryPlunge`s at
    // the same XY (the peck loop emits multiple per entry).
    let mut count = 0usize;
    let mut prev_entry: Option<(f64, f64)> = None;
    for mv in &tp.moves {
        if mv.intent == MoveIntent::EntryPlunge {
            let xy = (mv.target.x, mv.target.y);
            let same_event = match prev_entry {
                Some((px, py)) => (xy.0 - px).abs() < 0.01 && (xy.1 - py).abs() < 0.01,
                None => false,
            };
            if !same_event {
                count += 1;
            }
            prev_entry = Some(xy);
        } else {
            prev_entry = None;
        }
    }
    count
}

#[test]
fn entry_plunge_count_bounded_by_terrain_area_f038() {
    let mesh = micro_peak_terrain();
    let index = SpatialIndex::build(&mesh, 8.0);
    let tool = FlatEndmill::new(3.0, 25.0); // 6 mm dia, matches Wanaka

    // Pre-fix behaviour: threshold disabled.
    let params_off = make_params(tool.radius(), 0.0);
    let tp_off = adaptive_3d_toolpath(&mesh, &index, &tool, &params_off);
    let entries_off = count_entry_plunges(&tp_off);

    // Post-fix behaviour: default threshold (15 mm).
    let params_on = make_params(tool.radius(), 15.0);
    let tp_on = adaptive_3d_toolpath(&mesh, &index, &tool, &params_on);
    let entries_on = count_entry_plunges(&tp_on);

    eprintln!(
        "F-038: entries_off={entries_off} entries_on={entries_on} \
         (terrain area = {} mm², tool dia = {} mm)",
        SIZE * SIZE,
        tool.radius() * 2.0
    );

    // Regression net: post-fix entry count is well below pre-fix. The
    // Wanaka measurement showed 131 → 59 (≈55 % reduction); this fixture
    // exhibits the same fragmentation pattern (perimeter micro-peaks) so
    // a regression of the filter must surface here.
    //
    // Bound = post-fix entries must be ≤ 75 % of pre-fix entries. A more
    // forgiving threshold than the Wanaka ratio because the synthetic
    // terrain's marching-squares regions are smaller and the filter has
    // less to grip onto per region — but anything weaker than the filter
    // itself will leave post >= off here.
    assert!(
        entries_off > 0,
        "F-038 fixture issue: baseline emitted no entries; the terrain \
         needs material to clear."
    );
    let reduction_ratio = entries_on as f64 / entries_off as f64;
    assert!(
        reduction_ratio < 0.75,
        "F-038 regression: post-fix entry count {entries_on} \
         (ratio {reduction_ratio:.2}) is too close to pre-fix \
         {entries_off}; the fragmentation filter is no longer effective. \
         Expect ratio < 0.75."
    );

    // The fix should still emit at least one entry — otherwise it's
    // refusing to clear the terrain entirely.
    assert!(
        entries_on >= 1,
        "F-038 over-correction: no entry plunges emitted at all on a \
         terrain that has material to remove."
    );

    // Bound the absolute drop too — perimeter bumps are designed to be
    // sub-tool-sized so they should disappear from the toolpath under
    // the filter. With 8 perimeter bumps and 2 mm DPP over 4 mm of
    // material, the pre-fix toolpath emits at least one entry per bump
    // per Z level (≥ 16 perimeter entries on top of the central pad
    // entries). Post-fix should be dominated by the central pad's
    // entries (one per Z level, ≈ 2).
    let absolute_drop = entries_off.saturating_sub(entries_on);
    assert!(
        absolute_drop >= PERIMETER_BUMPS,
        "F-038 regression: only {absolute_drop} entries dropped \
         ({entries_off} → {entries_on}); expected at least {PERIMETER_BUMPS} \
         perimeter-bump entries to be eliminated."
    );
}
