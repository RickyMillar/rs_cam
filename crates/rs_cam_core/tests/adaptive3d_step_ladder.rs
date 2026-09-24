//! The adaptive3d step ladder — a sim-free fixture sentry for Phase 2 of
//! `planning/adaptive3d_step_ladder_roughing_2026-09-24/PLAN.md`.
//!
//! # The fixture
//!
//! One synthetic height field, 90 × 85 mm, top at Z 0 (= the stock top),
//! 26 mm deep at its lowest point:
//!
//! - (a) pocket A: a deep flat-floor pocket, the "plunge deep" case;
//! - (b) pocket B: a 20° floor that rises into a 60° wall;
//! - (c) pockets C1 and C2: two L-shaped pockets whose bounding boxes
//!   overlap. A shelf at Z -12 joins them above their dividing walls.
//!
//! # The claims (plan Phase 2, A1–A5)
//!
//! The cut is read from the emitted toolpath with an independent top-height
//! replay of a flat end mill (no simulator). Each move takes the step-ladder
//! tier of the level marker before it (`LadderTier` on the runtime event).
//!
//! - A1: in the open floor of pocket A the coarse tier takes full coarse
//!   bites, and no finer level removes material there.
//! - A2: a clip-tier cut point sits at its level Z; the drape does not lift
//!   it by more than one planner cell.
//! - A3: the axial bite of each tier is at most its step plus one cell. The
//!   coarse bound uses the DEEPEST step on the raw bite. The base bound uses
//!   `depth_per_pass` on the area bite, or the one-step plan's own bite where
//!   that is larger (a drape on a steep wall, F3).
//! - A4: every clip-tier tool centre is at least the link margin (one tool
//!   diameter) from the keep-out cells of its level, so the band next to a
//!   wall is at least the margin wide; the next tier cuts in closed runs.
//! - A5: the final stock matches the one-step plan at the base step: no cut
//!   below the leave that the one-step plan does not make, no more material
//!   left above the leave, and pointwise
//!   within one base terrace. (The plan says "within the leave". That holds
//!   for the 1 mm base step, measured 0.000 mm; with a 5 mm base step the
//!   terraces on the 60° wall sit 2.6 mm apart, because the rings of the two
//!   plans differ there.)
//!
//! # Cost
//!
//! Generation only, in debug, a few seconds per plan.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::print_stderr
)]

mod common;
use common::meshes::height_field_grid;

use std::collections::BTreeMap;

use rs_cam_core::adaptive3d::{
    Adaptive3dDepth, Adaptive3dGeometry, Adaptive3dLinking, Adaptive3dParams,
    Adaptive3dRuntimeAnnotation, Adaptive3dRuntimeEvent, ClearingStrategy3d, EntryStyle3d,
    LadderTier, RegionOrdering, adaptive_3d_toolpath_structured_annotated_traced_with_cancel,
};
use rs_cam_core::geometry::grid_field::distance_transform_2d;
use rs_cam_core::mesh::{SpatialIndex, TriangleMesh};
use rs_cam_core::surface::dropcutter::point_drop_cutter;
use rs_cam_core::tool::FlatEndmill;
use rs_cam_core::toolpath::{MoveIntent, MoveType, Toolpath};

const TOOL_RADIUS: f64 = 3.0;
const STOCK_TOP: f64 = 0.0;
const LEAVE: f64 = 0.5;
const STEPOVER: f64 = 2.4;
const TOLERANCE: f64 = 0.1;
/// The planner grid cell: `max(tool_radius / 6, tolerance)` in `path.rs`.
const CELL: f64 = TOOL_RADIUS / 6.0;
/// One tool diameter: `LINK_MARGIN_TOOL_DIAMETERS` × the diameter.
const LINK_MARGIN: f64 = 2.0 * TOOL_RADIUS;
const X_MAX: f64 = 90.0;
const Y_MAX: f64 = 85.0;
/// The replay grid step. Finer than the planner cell, so the replay does
/// not share the planner's cell rounding.
const REPLAY_CELL: f64 = 0.25;
/// The area bite reads the `AREA_BITE_CELLS`-th highest excess in the
/// footprint (0.5 mm², two planner cells). The raw maximum reads one replay
/// cell, and on a vertical wall one cell is a sliver that no ring touched
/// at the upper levels: its whole column goes at the bottom, in the
/// one-step plan as much as in the ladder. The area bite is the axial depth
/// of a real cut.
const AREA_BITE_CELLS: usize = 8;

// ── The fixture surface ──────────────────────────────────────────────────

fn in_rect(x: f64, y: f64, x0: f64, x1: f64, y0: f64, y1: f64) -> bool {
    x >= x0 && x <= x1 && y >= y0 && y <= y1
}

/// Pocket A, the deep flat floor.
const A: (f64, f64, f64, f64) = (5.0, 35.0, 5.0, 35.0);
const FLOOR: f64 = -26.0;
const SHELF: f64 = -12.0;

fn surface_z(x: f64, y: f64) -> f64 {
    // (a) the deep flat-floor pocket.
    if in_rect(x, y, A.0, A.1, A.2, A.3) {
        return FLOOR;
    }
    // (b) a 20° floor from x = 45 up to x = 65, then a 60° wall up to Z 0.
    if in_rect(x, y, 45.0, 85.0, 5.0, 35.0) {
        let floor_top = FLOOR + 20.0_f64.to_radians().tan() * 20.0;
        let z = if x <= 65.0 {
            FLOOR + 20.0_f64.to_radians().tan() * (x - 45.0)
        } else {
            floor_top + 60.0_f64.to_radians().tan() * (x - 65.0)
        };
        return z.min(0.0);
    }
    // (c) two L-shaped pockets with overlapping bounding boxes, joined by
    // the shelf at Z -12 above their dividing walls.
    if in_rect(x, y, 5.0, 85.0, 45.0, 80.0) {
        let c1 = in_rect(x, y, 8.0, 50.0, 48.0, 60.0) || in_rect(x, y, 8.0, 20.0, 48.0, 77.0);
        let c2 = in_rect(x, y, 26.0, 82.0, 64.0, 77.0) || in_rect(x, y, 70.0, 82.0, 48.0, 77.0);
        return if c1 || c2 { FLOOR } else { SHELF };
    }
    0.0
}

fn fixture_mesh() -> (TriangleMesh, SpatialIndex) {
    let nx = X_MAX as usize + 1;
    let ny = Y_MAX as usize + 1;
    let mesh = height_field_grid(0.0, 1.0, nx, 0.0, 1.0, ny, surface_z);
    let index = SpatialIndex::build(&mesh, 5.0);
    (mesh, index)
}

fn cutter() -> FlatEndmill {
    FlatEndmill::new(2.0 * TOOL_RADIUS, 40.0)
}

fn params(coarse_steps: &[f64], dpp: f64, ordering: RegionOrdering) -> Adaptive3dParams {
    Adaptive3dParams {
        geometry: Adaptive3dGeometry {
            tool_radius: TOOL_RADIUS,
            envelope_radius: TOOL_RADIUS,
            stepover: STEPOVER,
            tolerance: TOLERANCE,
            min_cutting_radius: 0.0,
            boundary: None,
            world_stock_xy_bbox: Some((0.0, 0.0, X_MAX, Y_MAX)),
        },
        depth: Adaptive3dDepth {
            depth_per_pass: dpp,
            stock_to_leave: LEAVE,
            stock_top_z: STOCK_TOP,
            z_floor: None,
            detect_flat_areas: false,
            coarse_steps: coarse_steps.to_vec(),
        },
        linking: Adaptive3dLinking {
            region_ordering: ordering,
            min_region_cut_length_mm: 0.0,
            max_stay_down_distance_mm: Some(0.0),
            stay_down_clearance_mm: 0.5,
        },
        feed_rate: 1500.0,
        plunge_rate: 500.0,
        safe_z: STOCK_TOP + 5.0,
        entry_style: EntryStyle3d::Plunge,
        initial_stock: None,
        clearing_strategy: ClearingStrategy3d::ContourParallel,
        trochoid_cap_mult: 1.6,
        engagement_measure: rs_cam_core::adaptive::EngagementMeasure::DiskArea,
        z_blend: false,
    }
}

// ── One run and its replay ──────────────────────────────────────────────

/// The level a move belongs to.
#[derive(Debug, Clone, Copy)]
enum Tag {
    Level { z: f64, tier: LadderTier },
    Waterline,
}

/// A top-height grid over the stock, replayed from the emitted moves.
struct Replay {
    nx: usize,
    ny: usize,
    top: Vec<f64>,
}

impl Replay {
    fn new() -> Self {
        let nx = (X_MAX / REPLAY_CELL).round() as usize + 1;
        let ny = (Y_MAX / REPLAY_CELL).round() as usize + 1;
        Self {
            nx,
            ny,
            top: vec![STOCK_TOP; nx * ny],
        }
    }

    fn xy(&self, i: usize) -> (f64, f64) {
        (
            (i % self.nx) as f64 * REPLAY_CELL,
            (i / self.nx) as f64 * REPLAY_CELL,
        )
    }

    /// Stamp a flat end mill at `(x, y, z)`. Returns the raw axial bite
    /// (the highest material above the tool tip in the footprint), the area
    /// bite (see [`AREA_BITE_CELLS`]) and the removed volume.
    fn stamp(&mut self, x: f64, y: f64, z: f64, excess: &mut Vec<f64>) -> Stamp {
        excess.clear();
        let mut out = Stamp::default();
        let r = TOOL_RADIUS;
        let c0 = (((x - r) / REPLAY_CELL).floor().max(0.0)) as usize;
        let c1 = ((((x + r) / REPLAY_CELL).ceil()) as usize).min(self.nx - 1);
        let r0 = (((y - r) / REPLAY_CELL).floor().max(0.0)) as usize;
        let r1 = ((((y + r) / REPLAY_CELL).ceil()) as usize).min(self.ny - 1);
        if x + r < 0.0 || y + r < 0.0 || c0 > c1 || r0 > r1 {
            return out;
        }
        for row in r0..=r1 {
            for col in c0..=c1 {
                let cx = col as f64 * REPLAY_CELL;
                let cy = row as f64 * REPLAY_CELL;
                if (cx - x).powi(2) + (cy - y).powi(2) > r * r {
                    continue;
                }
                let i = row * self.nx + col;
                if self.top[i] > z {
                    let e = self.top[i] - z;
                    excess.push(e);
                    out.bite = out.bite.max(e);
                    out.vol += e * REPLAY_CELL * REPLAY_CELL;
                    if in_open_floor(cx, cy) {
                        out.floor_bite = out.floor_bite.max(e);
                        out.floor_vol += e * REPLAY_CELL * REPLAY_CELL;
                    }
                    self.top[i] = z;
                }
            }
        }
        if excess.len() >= AREA_BITE_CELLS {
            excess.sort_by(|a, b| b.total_cmp(a));
            out.area_bite = excess[AREA_BITE_CELLS - 1];
        }
        out
    }
}

/// What one stamp removed.
#[derive(Debug, Default, Clone, Copy)]
struct Stamp {
    /// The raw axial bite: the highest material above the tool tip.
    bite: f64,
    /// The area bite, see [`AREA_BITE_CELLS`].
    area_bite: f64,
    vol: f64,
    /// The same two numbers over the cells of the open floor of pocket A.
    floor_bite: f64,
    floor_vol: f64,
}

/// What the replay measured for one tier (or for the waterline cleanup).
#[derive(Debug, Default, Clone)]
struct TierStats {
    step: f64,
    clip: bool,
    levels: usize,
    cut_mm: f64,
    removed_mm3: f64,
    /// The largest raw axial bite of a cutting or linking feed.
    max_bite: f64,
    /// The largest area bite of a cutting or linking feed.
    max_area_bite: f64,
    /// Cut length in closed runs (a run of cutting feeds whose end is
    /// within one stepover of its start), and in all runs.
    closed_run_mm: f64,
    run_mm: f64,
    /// The largest axial bite of an entry plunge (reported, not bounded).
    max_entry_bite: f64,
    /// The volume removed from the cells of the open floor of pocket A, and
    /// the largest axial excess removed there.
    removed_in_open_floor_mm3: f64,
    max_bite_in_open_floor: f64,
    /// The largest `emitted z - level z` of a clip-tier cutting feed.
    max_lift: f64,
    min_below: f64,
    /// Where the largest bite happened: (x, y, z, intent, level z).
    max_bite_at: Option<(f64, f64, f64, MoveIntent, f64)>,
}

struct Run {
    tp: Toolpath,
    tags: Vec<Option<Tag>>,
    /// Keyed by tier index; `usize::MAX` is the waterline cleanup.
    stats: BTreeMap<usize, TierStats>,
    replay: Replay,
}

/// The open floor of pocket A: the pocket inset by the link margin, the
/// keep-out offset (one radius) and one more radius, so no band sits there.
/// A cell of the replay grid is in it when its centre is.
fn in_open_floor(x: f64, y: f64) -> bool {
    let inset = LINK_MARGIN + 2.0 * TOOL_RADIUS + 1.0;
    in_rect(x, y, A.0 + inset, A.1 - inset, A.2 + inset, A.3 - inset)
}

fn tag_moves(tp: &Toolpath, annotations: &[Adaptive3dRuntimeAnnotation]) -> Vec<Option<Tag>> {
    let mut sorted: Vec<&Adaptive3dRuntimeAnnotation> = annotations.iter().collect();
    sorted.sort_by_key(|a| a.move_index);
    let mut tags = vec![None; tp.moves.len()];
    let mut current: Option<Tag> = None;
    let mut next = 0usize;
    for (i, tag) in tags.iter_mut().enumerate() {
        while next < sorted.len() && sorted[next].move_index <= i {
            match &sorted[next].event {
                Adaptive3dRuntimeEvent::GlobalZLevel { z_level, tier, .. }
                | Adaptive3dRuntimeEvent::RegionZLevel { z_level, tier, .. } => {
                    current = Some(Tag::Level {
                        z: *z_level,
                        tier: *tier,
                    });
                }
                Adaptive3dRuntimeEvent::WaterlineCleanup => current = Some(Tag::Waterline),
                _ => {}
            }
            next += 1;
        }
        *tag = current;
    }
    tags
}

fn run(coarse_steps: &[f64], dpp: f64, ordering: RegionOrdering) -> Run {
    let (mesh, index) = fixture_mesh();
    let cutter = cutter();
    let p = params(coarse_steps, dpp, ordering);
    let (tp, annotations, _) = adaptive_3d_toolpath_structured_annotated_traced_with_cancel(
        &mesh,
        &index,
        &cutter,
        &p,
        &(|| false),
        None,
    )
    .expect("generation is not cancelled");
    let tags = tag_moves(&tp, &annotations);

    let mut replay = Replay::new();
    let mut stats: BTreeMap<usize, TierStats> = BTreeMap::new();
    let mut excess = Vec::new();
    let mut prev = None;
    // The open run of cutting feeds: (tier key, start, length).
    let mut open_run: Option<(usize, (f64, f64), f64)> = None;
    let close_run = |stats: &mut BTreeMap<usize, TierStats>,
                     run: Option<(usize, (f64, f64), f64)>,
                     end: (f64, f64)| {
        if let Some((key, start, len)) = run {
            let s = stats.entry(key).or_default();
            s.run_mm += len;
            if (start.0 - end.0).hypot(start.1 - end.1) <= STEPOVER {
                s.closed_run_mm += len;
            }
        }
    };
    for (m, tag) in tp.moves.iter().zip(&tags) {
        let from = prev.unwrap_or(m.target);
        prev = Some(m.target);
        let key_now = match tag {
            Some(Tag::Level { tier, .. }) => Some(tier.index),
            Some(Tag::Waterline) => Some(usize::MAX),
            None => None,
        };
        let cutting = m.intent == MoveIntent::ClearingCut && key_now.is_some();
        let step_xy = (m.target.x - from.x).hypot(m.target.y - from.y);
        let extend = cutting && matches!(open_run, Some((k, _, _)) if Some(k) == key_now);
        if extend {
            if let Some((_, _, len)) = open_run.as_mut() {
                *len += step_xy;
            }
        } else {
            close_run(&mut stats, open_run.take(), (from.x, from.y));
            if cutting {
                open_run = Some((key_now.unwrap_or(0), (from.x, from.y), step_xy));
            }
        }
        if matches!(m.move_type, MoveType::Rapid) {
            continue;
        }
        let key = match tag {
            Some(Tag::Level { tier, .. }) => tier.index,
            Some(Tag::Waterline) => usize::MAX,
            None => continue,
        };
        let s = stats.entry(key).or_default();
        if let Some(Tag::Level { z, tier }) = tag {
            s.step = tier.step_mm;
            s.clip = tier.clip;
            if tier.clip && m.intent == MoveIntent::ClearingCut {
                s.max_lift = s.max_lift.max(m.target.z - z);
                s.min_below = s.min_below.min(m.target.z - z);
            }
        }
        let dx = m.target.x - from.x;
        let dy = m.target.y - from.y;
        let dz = m.target.z - from.z;
        let len = (dx * dx + dy * dy + dz * dz).sqrt();
        if m.intent == MoveIntent::ClearingCut {
            s.cut_mm += len;
        }
        let n = ((len / (REPLAY_CELL * 0.5)).ceil() as usize).max(1);
        for k in 1..=n {
            let t = k as f64 / n as f64;
            let (x, y, z) = (from.x + t * dx, from.y + t * dy, from.z + t * dz);
            let st = replay.stamp(x, y, z, &mut excess);
            let (bite, area_bite) = (st.bite, st.area_bite);
            s.removed_mm3 += st.vol;
            s.removed_in_open_floor_mm3 += st.floor_vol;
            s.max_bite_in_open_floor = s.max_bite_in_open_floor.max(st.floor_bite);
            match m.intent {
                MoveIntent::EntryPlunge | MoveIntent::EntryHelix | MoveIntent::EntryRamp => {
                    s.max_entry_bite = s.max_entry_bite.max(bite);
                }
                _ => {
                    if bite > s.max_bite {
                        let lz = match tag {
                            Some(Tag::Level { z, .. }) => *z,
                            _ => f64::NAN,
                        };
                        s.max_bite_at = Some((x, y, z, m.intent, lz));
                    }
                    s.max_bite = s.max_bite.max(bite);
                    s.max_area_bite = s.max_area_bite.max(area_bite);
                }
            }
        }
    }
    let end = prev.map_or((0.0, 0.0), |p| (p.x, p.y));
    close_run(&mut stats, open_run.take(), end);
    // Level counts per tier.
    for a in &annotations {
        if let Adaptive3dRuntimeEvent::GlobalZLevel { tier, .. }
        | Adaptive3dRuntimeEvent::RegionZLevel { tier, .. } = &a.event
        {
            let s = stats.entry(tier.index).or_default();
            s.levels += 1;
            s.step = tier.step_mm;
            s.clip = tier.clip;
        }
    }
    Run {
        tp,
        tags,
        stats,
        replay,
    }
}

fn describe(label: &str, r: &Run) -> String {
    let mut out = format!("{label}: {} moves\n", r.tp.moves.len());
    for (k, s) in &r.stats {
        let name = if *k == usize::MAX {
            "waterline".to_owned()
        } else {
            format!(
                "tier {k} ({} {:.1} mm)",
                if s.clip { "clip" } else { "drape" },
                s.step
            )
        };
        out.push_str(&format!(
            "  {name}: levels {}, cut {:.0} mm ({:.0} % in closed runs), removed {:.0} mm3, \
             area bite {:.3}, raw bite {:.3}, max entry bite {:.3}, open-floor removal \
             {:.1} mm3 (max bite {:.3}), clip lift [{:.4}, {:.4}], raw bite at {:?}\n",
            s.levels,
            s.cut_mm,
            100.0 * s.closed_run_mm / s.run_mm.max(1e-9),
            s.removed_mm3,
            s.max_area_bite,
            s.max_bite,
            s.max_entry_bite,
            s.removed_in_open_floor_mm3,
            s.max_bite_in_open_floor,
            s.min_below,
            s.max_lift,
            s.max_bite_at,
        ));
    }
    out
}

// ── The keep-out distance (A4) ──────────────────────────────────────────

/// The drop-cutter rest height of the operation's cutter on the replay
/// grid: the same quantity the planner's surface heightmap holds.
fn rest_heights(mesh: &TriangleMesh, index: &SpatialIndex, grid: &Replay) -> Vec<f64> {
    let cutter = cutter();
    (0..grid.nx * grid.ny)
        .map(|i| {
            let (x, y) = grid.xy(i);
            let cl = point_drop_cutter(x, y, mesh, index, &cutter);
            if cl.contacted && cl.z.is_finite() {
                cl.z
            } else {
                f64::NEG_INFINITY
            }
        })
        .collect()
}

/// The mesh surface height on the replay grid (a 0.02 mm probe).
fn surface_heights(mesh: &TriangleMesh, index: &SpatialIndex, grid: &Replay) -> Vec<f64> {
    let probe = FlatEndmill::new(0.02, 40.0);
    (0..grid.nx * grid.ny)
        .map(|i| {
            let (x, y) = grid.xy(i);
            let cl = point_drop_cutter(x, y, mesh, index, &probe);
            if cl.contacted && cl.z.is_finite() {
                cl.z
            } else {
                f64::NEG_INFINITY
            }
        })
        .collect()
}

/// For each clip level: the smallest distance from a clip-tier cutting tool
/// centre to a keep-out cell of that level.
fn min_keep_out_distance(r: &Run, rest: &[f64]) -> Vec<(f64, usize, f64)> {
    let grid = &r.replay;
    let mut per_level: BTreeMap<(i64, usize), f64> = BTreeMap::new();
    let mut fields: BTreeMap<i64, Vec<f64>> = BTreeMap::new();
    let mut prev = None;
    for (m, tag) in r.tp.moves.iter().zip(&r.tags) {
        let from = prev.unwrap_or(m.target);
        prev = Some(m.target);
        let Some(Tag::Level { z, tier }) = tag else {
            continue;
        };
        if !tier.clip || m.intent != MoveIntent::ClearingCut {
            continue;
        }
        let key = (z * 1000.0).round() as i64;
        let field = fields.entry(key).or_insert_with(|| {
            let keep_out: Vec<bool> = rest.iter().map(|&h| h + LEAVE > *z).collect();
            distance_transform_2d(&keep_out, grid.ny, grid.nx)
                .into_iter()
                .map(|d| d * REPLAY_CELL)
                .collect()
        });
        let (dx, dy) = (m.target.x - from.x, m.target.y - from.y);
        let n = (((dx * dx + dy * dy).sqrt() / REPLAY_CELL).ceil() as usize).max(1);
        for k in 0..=n {
            let t = k as f64 / n as f64;
            let (x, y) = (from.x + t * dx, from.y + t * dy);
            let col = (x / REPLAY_CELL).round();
            let row = (y / REPLAY_CELL).round();
            if col < 0.0 || row < 0.0 || col as usize >= grid.nx || row as usize >= grid.ny {
                continue;
            }
            let d = field[row as usize * grid.nx + col as usize];
            let slot = per_level.entry((key, tier.index)).or_insert(f64::INFINITY);
            *slot = slot.min(d);
        }
    }
    per_level
        .into_iter()
        .map(|((key, tier), d)| (key as f64 / 1000.0, tier, d))
        .collect()
}

// ── The claims ──────────────────────────────────────────────────────────

fn assert_ladder_claims(label: &str, coarse: &[f64], dpp: f64, base_run: &Run) {
    let r = run(coarse, dpp, RegionOrdering::ByArea);
    let report = describe(label, &r);
    eprintln!("{report}");
    eprintln!("{}", describe("one-step baseline", base_run));

    let deepest = coarse.iter().copied().fold(dpp, f64::max);
    let base_index = coarse.len();

    // Non-vacuity: every tier ran and the coarse tier removed material.
    for t in 0..=base_index {
        let s = r
            .stats
            .get(&t)
            .unwrap_or_else(|| panic!("tier {t} never ran\n{report}"));
        assert!(s.levels > 0, "tier {t} has no level\n{report}");
    }
    let coarse_stats = &r.stats[&0];
    assert!(coarse_stats.clip, "tier 0 must clip\n{report}");
    assert!(
        !r.stats[&base_index].clip,
        "the base tier must drape\n{report}"
    );
    assert!(
        coarse_stats.removed_mm3 > 0.3 * r.stats.values().map(|s| s.removed_mm3).sum::<f64>(),
        "the coarse tier removed too little to matter\n{report}"
    );

    // A1: full coarse bites in the open floor, and no finer level cuts there.
    assert!(
        coarse_stats.max_bite_in_open_floor >= deepest - CELL,
        "A1: the coarse tier took no full {deepest} mm bite in the open floor\n{report}"
    );
    for t in 1..=base_index {
        let s = &r.stats[&t];
        assert!(
            s.removed_in_open_floor_mm3 < 1.0,
            "A1: tier {t} removed {:.2} mm3 in the open floor of pocket A\n{report}",
            s.removed_in_open_floor_mm3
        );
    }

    // A2: a clip-tier cut point sits at its level; the drape does not ride
    // the wall.
    for t in 0..base_index {
        let s = &r.stats[&t];
        assert!(
            s.max_lift <= CELL + 1e-9 && s.min_below >= -1e-6,
            "A2: tier {t} cut points left their level by [{:.4}, {:.4}] mm\n{report}",
            s.min_below,
            s.max_lift
        );
    }

    // A3: the bite of each coarse tier is bounded by its own step, and so by
    // the DEEPEST step. The coarse bound holds on the RAW bite, the strict
    // reading: a coarse tier never rides a wall.
    for t in 0..base_index {
        let s = &r.stats[&t];
        assert!(
            s.max_bite <= deepest + CELL,
            "A3: coarse tier {t} bit {:.3} mm, above the deepest step {deepest} + one cell\n\
             {report}",
            s.max_bite
        );
        assert!(
            s.max_bite <= s.step + CELL,
            "A3: coarse tier {t} bit {:.3} mm, above its own step {} + one cell\n{report}",
            s.max_bite,
            s.step
        );
    }
    // The base tier is bounded by depth_per_pass on the area bite. Where the
    // one-step plan itself bites deeper than that — the drape of a base
    // level on a steep wall takes the terrace the level above left (F3) —
    // the ladder must not bite deeper than the one-step plan does.
    let base = &r.stats[&base_index];
    let baseline_base = &base_run.stats[&0];
    let base_bound = (dpp + CELL).max(baseline_base.max_area_bite + CELL);
    eprintln!(
        "A3: base area bite {:.3} mm; bound {base_bound:.3} (dpp {dpp} + one cell, or the \
         one-step plan's own {:.3} + one cell)",
        base.max_area_bite, baseline_base.max_area_bite
    );
    assert!(
        base.max_area_bite <= base_bound + 1e-9,
        "A3: the base tier bit {:.3} mm (area), above {base_bound:.3} mm\n{report}",
        base.max_area_bite
    );
    assert!(
        base.max_bite <= baseline_base.max_bite + CELL,
        "A3: the base tier's raw bite {:.3} mm is above the one-step plan's {:.3} mm\n{report}",
        base.max_bite,
        baseline_base.max_bite
    );

    // A4: the band next to a wall is at least the link margin wide.
    let (mesh, index) = fixture_mesh();
    let rest = rest_heights(&mesh, &index, &r.replay);
    let distances = min_keep_out_distance(&r, &rest);
    assert!(
        !distances.is_empty(),
        "A4: no clip-tier cut to measure\n{report}"
    );
    for (z, tier, d) in &distances {
        eprintln!("A4: clip level Z {z:.2} tier {tier}: min keep-out distance {d:.3} mm");
        assert!(
            *d >= LINK_MARGIN - 2.0 * CELL,
            "A4: at Z {z:.2} a tier-{tier} tool centre came {d:.3} mm from the keep-out; the \
             link margin is {LINK_MARGIN} mm\n{report}"
        );
    }
    // The next tier cuts the band as closed loops, not as slivers.
    let next = &r.stats[&1];
    let closed = next.closed_run_mm / next.run_mm.max(1e-9);
    assert!(
        next.run_mm > 0.0 && closed >= 0.9,
        "A4: the tier after the coarse tier cut {:.0} % of its length in closed runs\n{report}",
        100.0 * closed
    );

    // A5: the final stock matches the one-step plan. (a) No cell is cut
    // below the leave. (b) The material left above `surface + leave` is not
    // more than the one-step plan leaves. (c) Pointwise, the two differ by
    // at most one base terrace: on a steep wall the terraces of a
    // `depth_per_pass` step sit where the rings of each plan put them.
    let surface = surface_heights(&mesh, &index, &r.replay);
    let (mut gouge, mut gouge_base, mut gouge_at) = (0.0f64, 0.0f64, 0usize);
    let (mut left_ladder, mut left_base) = (0.0f64, 0.0f64);
    for (i, &h) in surface.iter().enumerate() {
        if !h.is_finite() {
            continue;
        }
        // Uncut stock is no cut below the leave, even where the stock top
        // itself is below `surface + leave` (the plate top is the stock top).
        if r.replay.top[i] < STOCK_TOP && h + LEAVE - r.replay.top[i] > gouge {
            gouge = h + LEAVE - r.replay.top[i];
            gouge_at = i;
        }
        if base_run.replay.top[i] < STOCK_TOP {
            gouge_base = gouge_base.max(h + LEAVE - base_run.replay.top[i]);
        }
        left_ladder += (r.replay.top[i] - h - LEAVE).max(0.0) * REPLAY_CELL * REPLAY_CELL;
        left_base += (base_run.replay.top[i] - h - LEAVE).max(0.0) * REPLAY_CELL * REPLAY_CELL;
    }
    eprintln!(
        "A5: deepest cut below the leave {gouge:.3} mm at {:?} (one-step {gouge_base:.3}); \
         left above the leave: ladder {left_ladder:.0} mm3, one-step {left_base:.0} mm3",
        r.replay.xy(gouge_at)
    );
    // The replay reads a few wall cells below the leave in BOTH plans (the
    // footprint edge against the 1 mm mesh wall); the ladder must add none.
    assert!(
        gouge <= gouge_base + 0.05,
        "A5: the ladder cut {gouge:.3} mm below the leave, the one-step plan \
         {gouge_base:.3} mm\n{report}"
    );
    assert!(
        left_ladder <= 1.05 * left_base + 50.0,
        "A5: the ladder left {left_ladder:.0} mm3 above the leave, the one-step plan \
         {left_base:.0} mm3\n{report}"
    );
    let mut worst = 0.0f64;
    let (mut hi, mut lo) = ((0.0f64, 0usize), (0.0f64, 0usize));
    for (i, (a, b)) in r.replay.top.iter().zip(&base_run.replay.top).enumerate() {
        worst = worst.max((a - b).abs());
        if a - b > hi.0 {
            hi = (a - b, i);
        }
        if b - a > lo.0 {
            lo = (b - a, i);
        }
    }
    eprintln!(
        "A5: ladder higher by {:.3} at {:?}; ladder lower by {:.3} at {:?}",
        hi.0,
        r.replay.xy(hi.1),
        lo.0,
        r.replay.xy(lo.1)
    );
    eprintln!("A5: largest final-stock difference against the one-step plan: {worst:.3} mm");
    assert!(
        worst <= dpp + CELL,
        "A5: the final stock differs from the one-step plan by {worst:.3} mm, more than \
         one base terrace\n{report}"
    );
}

#[test]
fn ladder_10_over_base_5_meets_a1_to_a5() {
    let base = run(&[], 5.0, RegionOrdering::ByArea);
    assert_ladder_claims("ladder [10] + dpp 5", &[10.0], 5.0, &base);
}

#[test]
fn ladder_10_5_over_base_1_meets_a1_to_a5() {
    let base = run(&[], 1.0, RegionOrdering::ByArea);
    assert_ladder_claims("ladder [10, 5] + dpp 1", &[10.0, 5.0], 1.0, &base);
}

/// The empty ladder is today's plan: one drape tier, no clip level.
#[test]
fn empty_ladder_has_one_drape_tier() {
    let r = run(&[], 5.0, RegionOrdering::Global);
    assert_eq!(r.stats.keys().filter(|k| **k != usize::MAX).count(), 1);
    let s = &r.stats[&0];
    assert!(!s.clip && s.levels > 0);
}

// ── The deleted keys still load ─────────────────────────────────────────

/// The ruling of 2026-09-16 deleted Fine Stepdown and Mill Shallow with no
/// migration. A config that still carries their keys must still parse
/// (serde ignores an unknown key), and it gets the single-step ladder.
#[test]
fn a_config_with_the_deleted_keys_still_parses() {
    let src = r#"
        stepover = 2.0
        depth_per_pass = 3.0
        stock_to_leave_axial = 0.5
        feed_rate = 1500.0
        plunge_rate = 500.0
        tolerance = 0.1
        min_cutting_radius = 0.0
        entry_style = "plunge"
        fine_stepdown = 0.4
        detect_flat_areas = false
        region_ordering = "global"
        mill_shallow_areas = true
        shallow_angle_deg = 25.0
        shallow_stepdown = 0.6
    "#;
    let cfg: rs_cam_core::compute::operation_configs::Adaptive3dConfig =
        toml::from_str(src).expect("the old keys must not break the load");
    assert!(cfg.coarse_steps.is_empty());
    assert_eq!(cfg.deepest_step(), 3.0);

    let with_ladder: rs_cam_core::compute::operation_configs::Adaptive3dConfig =
        toml::from_str(&format!("{src}\ncoarse_steps = [10.0, 5.0]\n")).expect("ladder parses");
    assert_eq!(with_ladder.coarse_steps, vec![10.0, 5.0]);
    assert_eq!(with_ladder.deepest_step(), 10.0);
    // An empty ladder is not written back, so saved projects do not change.
    let emitted = toml::to_string(&cfg).expect("serialise");
    assert!(!emitted.contains("coarse_steps"), "{emitted}");
    assert!(!emitted.contains("fine_stepdown"), "{emitted}");
}

/// A pinned project file with `fine_stepdown = 0.0` in its 3D Rough loads.
#[test]
fn a_pinned_project_with_the_deleted_keys_loads() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/test_job.toml");
    let src = std::fs::read_to_string(&path).expect("read the fixture");
    assert!(
        src.contains("fine_stepdown"),
        "fixture drift: test_job.toml no longer carries a deleted key"
    );
    let session =
        rs_cam_core::session::ProjectSession::load(&path).expect("the project must still load");
    let rough = session
        .toolpath_configs()
        .iter()
        .find_map(|tc| match &tc.operation {
            rs_cam_core::compute::catalog::OperationConfig::Adaptive3d(cfg) => Some(cfg),
            _ => None,
        })
        .expect("test_job.toml holds a 3D Rough");
    assert!(rough.coarse_steps.is_empty());
}

// ── Observability (plan Phase 0, items 1-2) ─────────────────────────────

/// A debug trace shows each level with its tier and its own volume, and
/// one `ladder_tier` span per tier carries the tier totals.
#[test]
fn debug_trace_shows_each_level_with_its_tier_and_volume() {
    use rs_cam_core::trace::debug_trace::ToolpathDebugRecorder;
    let (mesh, index) = fixture_mesh();
    let cutter = cutter();
    let p = params(&[10.0], 5.0, RegionOrdering::Global);
    let recorder = ToolpathDebugRecorder::new("ladder", "3D Rough");
    let ctx = recorder.root_context();
    let (_tp, annotations, _) = adaptive_3d_toolpath_structured_annotated_traced_with_cancel(
        &mesh,
        &index,
        &cutter,
        &p,
        &(|| false),
        Some(&ctx),
    )
    .expect("generation is not cancelled");
    let trace = recorder.finish();

    let levels: Vec<_> = trace
        .spans
        .iter()
        .filter(|s| s.kind == "z_level_clear")
        .collect();
    let markers = annotations
        .iter()
        .filter(|a| matches!(a.event, Adaptive3dRuntimeEvent::GlobalZLevel { .. }))
        .count();
    assert!(!levels.is_empty(), "no z_level_clear span");
    assert_eq!(levels.len(), markers, "one span per level marker");
    for s in &levels {
        for key in [
            "tier_index",
            "tier_clip",
            "tier_step_mm",
            "planner_removed_mm3",
            "planner_max_bite_mm",
            "planner_cut_mm",
            "planner_rapid_segments",
        ] {
            assert!(s.counters.contains_key(key), "{} has no `{key}`", s.label);
        }
    }
    let clip_volume: f64 = levels
        .iter()
        .filter(|s| s.counters["tier_clip"] == 1.0)
        .map(|s| s.counters["planner_removed_mm3"])
        .sum();
    assert!(
        clip_volume > 0.0,
        "the clip levels report no removed volume"
    );
    for s in levels.iter().filter(|s| s.counters["tier_clip"] == 1.0) {
        assert!(
            s.counters["planner_max_bite_mm"] <= 10.0 + CELL,
            "{}: planner bite {} above the coarse step",
            s.label,
            s.counters["planner_max_bite_mm"]
        );
    }

    let tiers: Vec<_> = trace
        .spans
        .iter()
        .filter(|s| s.kind == "ladder_tier")
        .collect();
    assert_eq!(tiers.len(), 2, "one ladder_tier span per tier");
    for t in &tiers {
        for key in [
            "levels",
            "planner_cut_mm",
            "planner_entries",
            "planner_retracts",
            "planner_removed_mm3",
        ] {
            assert!(t.counters.contains_key(key), "{} has no `{key}`", t.label);
        }
    }
    let waterline = trace
        .spans
        .iter()
        .find(|s| s.kind == "waterline_cleanup")
        .expect("Global runs the waterline cleanup");
    assert!(waterline.counters.contains_key("planner_cut_mm"));
}

// ── D7: the deepest step reaches the static checks ──────────────────────

/// A coarse step deeper than the cutting length must trip the shank check,
/// although `depth_per_pass` itself is inside it. Before D7 the check read
/// `depth_per_pass` and saw nothing.
#[test]
fn the_shank_check_reads_the_deepest_step() {
    use rs_cam_core::compute::catalog::OperationConfig;
    use rs_cam_core::compute::operation_configs::Adaptive3dConfig;
    use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
    use rs_cam_core::diagnostics::adapters::from_static_checks::diagnostics_from_static_checks;
    use rs_cam_core::diagnostics::ids;
    use rs_cam_core::ids::ToolpathId;

    let mut tool = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    tool.diameter = 6.0;
    tool.cutting_length = 12.0;
    let cfg = Adaptive3dConfig {
        depth_per_pass: 5.0,
        coarse_steps: vec![15.0],
        ..Adaptive3dConfig::default()
    };
    let op = OperationConfig::Adaptive3d(cfg);
    assert_eq!(op.depth_per_pass(), Some(5.0));
    assert_eq!(op.deepest_axial_step(), Some(15.0));
    let found = diagnostics_from_static_checks(ToolpathId(1), &op, &tool, None);
    let hit = found
        .iter()
        .find(|d| d.id.as_str() == ids::GEOM_DPP_EXCEEDS_CUTTING_LENGTH)
        .unwrap_or_else(|| panic!("no shank finding for a 15 mm coarse step: {found:#?}"));
    assert!(
        hit.message.contains("deepest step"),
        "the message must name the ladder step: {}",
        hit.message
    );
}
