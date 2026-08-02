//! **M5's bench for the 2D offset primitive** — fixtures, cascade runner,
//! per-ring attribution, and a tolerance-free geometric oracle.
//!
//! `offset_polygon` is the single 2D offsetting primitive in this workspace
//! (pocket, adaptive, adaptive3d, rest, boundary, profile, trace, zigzag,
//! inlay, project-curve, scallop, region_set, and the viz worker all call it).
//! Scallop compensates for its vertex inflation by decimating after every
//! ring; nothing else does. M5 asks whether the inflation can be fixed at the
//! source, and this module is the instrument that answer has to come from.
//!
//! # The oracle, and why it needs no tolerance dial
//!
//! Erosion composes: eroding a set by `d` twice is eroding it once by `2d`.
//! And the boundary of the erosion of `P` by `d` is exactly the level set
//! `{ p : dist(p, ∂P) = d }` — *every* point on it is at distance `d` from
//! the original boundary, on any shape, convex or not.
//!
//! So for a cascade of `k` offsets of `step` each, the truth is known in
//! closed form without ever computing it: **every vertex of ring `k` must sit
//! at distance `k · step` from the ORIGINAL boundary.** [`erosion_error`]
//! measures exactly that. It needs no reference implementation, no tuned
//! tolerance, and it is valid for the captured real fixtures as well as the
//! synthetic ones.
//!
//! [`erosion_area`] is the second half: rasterised truth for how much area
//! the erosion should have, which catches a candidate that keeps its vertices
//! at the right distance while losing (or inventing) whole regions.

use std::time::Instant;

use rs_cam_core::geo::P2;
use rs_cam_core::polygon::{Polygon2, offset_polygon};

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// One shape plus the cascade dials it should be driven at.
pub struct OffsetFixture {
    pub name: &'static str,
    /// What this fixture isolates. Printed in every table.
    pub what: &'static str,
    pub poly: Polygon2,
    /// Inward offset per ring (mm).
    pub step: f64,
    /// The vertex spacing this fixture was *authored* at — the density its
    /// source (mesh sampling grid, marching squares, or the generator below)
    /// can actually resolve. Drop-only decimation is floored at
    /// `0.75 × authored_spacing`, which is what scallop does with its
    /// heightmap cell.
    pub authored_spacing: f64,
}

impl OffsetFixture {
    pub fn vertices(&self) -> usize {
        ring_vertex_count(&self.poly)
    }
}

/// Total vertex count of a polygon: exterior plus every hole.
pub fn ring_vertex_count(p: &Polygon2) -> usize {
    p.exterior.len() + p.holes.iter().map(Vec::len).sum::<usize>()
}

pub fn total_vertices(polys: &[Polygon2]) -> usize {
    polys.iter().map(ring_vertex_count).sum()
}

/// A convex control. Nothing here can arc-join on an inward offset, so any
/// vertex growth on this fixture is machinery, not geometry.
pub fn square(size: f64) -> Polygon2 {
    Polygon2::rectangle(0.0, 0.0, size, size)
}

/// **The stress fixture.** `r(θ) = base + amp·sin(lobes·θ)`, sampled at
/// `samples` points: every valley between lobes is a reflex corner that
/// survives being eroded, so the cascade never runs out of concavity the way
/// a comb does when its fingers collapse.
pub fn rosette(base: f64, amp: f64, lobes: usize, samples: usize) -> Polygon2 {
    let pts = (0..samples)
        .map(|i| {
            let t = std::f64::consts::TAU * i as f64 / samples as f64;
            let r = base + amp * (lobes as f64 * t).sin();
            P2::new(r * t.cos(), r * t.sin())
        })
        .collect();
    Polygon2::new(pts)
}

/// A comb: rectangle `w × h` with `teeth` slots cut in from the top, each
/// `slot_w` wide and `slot_d` deep. The plan's "comb" fixture — concave, and
/// with a collapse deadline (the fingers between slots die at
/// `finger_w / (2·step)` rings), which is itself worth measuring.
pub fn comb(w: f64, h: f64, teeth: usize, slot_w: f64, slot_d: f64) -> Polygon2 {
    let mut pts = vec![P2::new(0.0, 0.0), P2::new(w, 0.0), P2::new(w, h)];
    let pitch = w / (teeth as f64 + 1.0);
    for i in (1..=teeth).rev() {
        let cx = pitch * i as f64;
        let (x0, x1) = (cx - slot_w * 0.5, cx + slot_w * 0.5);
        pts.push(P2::new(x1, h));
        pts.push(P2::new(x1, h - slot_d));
        pts.push(P2::new(x0, h - slot_d));
        pts.push(P2::new(x0, h));
    }
    pts.push(P2::new(0.0, h));
    Polygon2::new(pts)
}

/// Two-level comb: slots cut into the sides of the fingers of a comb. This is
/// the closest synthetic stand-in for the dendritic mid-steep bands scallop
/// meets on terrain — branching concavity at two scales.
pub fn dendrite(w: f64, h: f64, teeth: usize, slot_w: f64, slot_d: f64) -> Polygon2 {
    let mut pts = vec![P2::new(0.0, 0.0), P2::new(w, 0.0), P2::new(w, h)];
    let pitch = w / (teeth as f64 + 1.0);
    let notch_w = slot_w * 0.35;
    let notch_d = slot_w * 0.6;
    for i in (1..=teeth).rev() {
        let cx = pitch * i as f64;
        let (x0, x1) = (cx - slot_w * 0.5, cx + slot_w * 0.5);
        // Right wall of the slot, with two notches biting into the finger.
        pts.push(P2::new(x1, h));
        for lvl in 0..2 {
            let y = h - slot_d * (0.3 + 0.35 * lvl as f64);
            pts.push(P2::new(x1, y + notch_w * 0.5));
            pts.push(P2::new(x1 + notch_d, y + notch_w * 0.5));
            pts.push(P2::new(x1 + notch_d, y - notch_w * 0.5));
            pts.push(P2::new(x1, y - notch_w * 0.5));
        }
        pts.push(P2::new(x1, h - slot_d));
        pts.push(P2::new(x0, h - slot_d));
        for lvl in (0..2).rev() {
            let y = h - slot_d * (0.3 + 0.35 * lvl as f64);
            pts.push(P2::new(x0, y - notch_w * 0.5));
            pts.push(P2::new(x0 - notch_d, y - notch_w * 0.5));
            pts.push(P2::new(x0 - notch_d, y + notch_w * 0.5));
            pts.push(P2::new(x0, y + notch_w * 0.5));
        }
        pts.push(P2::new(x0, h));
    }
    pts.push(P2::new(0.0, h));
    Polygon2::new(pts)
}

/// Square with a grid of circular holes: the `Shape::parallel_offset` path,
/// multiple disjoint outputs once the growing holes cut the field apart.
pub fn holed(size: f64, holes_per_side: usize, radius: f64, hole_verts: usize) -> Polygon2 {
    let mut poly = square(size);
    let pitch = size / (holes_per_side as f64 + 1.0);
    for r in 1..=holes_per_side {
        for c in 1..=holes_per_side {
            let (cx, cy) = (pitch * c as f64, pitch * r as f64);
            // Holes wind CW (negative area) per `Polygon2`'s contract.
            let ring = (0..hole_verts)
                .map(|i| {
                    let t = -std::f64::consts::TAU * i as f64 / hole_verts as f64;
                    P2::new(cx + radius * t.cos(), cy + radius * t.sin())
                })
                .collect();
            poly.holes.push(ring);
        }
    }
    poly
}

/// A square whose edges carry hundreds of sub-micron segments, half of them
/// below cavalier's own `pos_equal_eps` (1e-5). The plan's "near-degenerate
/// short edges" case.
pub fn short_edges(size: f64, per_edge: usize) -> Polygon2 {
    let corners = [
        (P2::new(0.0, 0.0), P2::new(size, 0.0)),
        (P2::new(size, 0.0), P2::new(size, size)),
        (P2::new(size, size), P2::new(0.0, size)),
        (P2::new(0.0, size), P2::new(0.0, 0.0)),
    ];
    let mut pts = Vec::new();
    for (a, b) in corners {
        for i in 0..per_edge {
            let t = i as f64 / per_edge as f64;
            let x = a.x + (b.x - a.x) * t;
            let y = a.y + (b.y - a.y) * t;
            pts.push(P2::new(x, y));
            // A sub-epsilon companion vertex: 4 nm out along the edge, and a
            // 2 nm perpendicular wobble so it is not an exact repeat.
            let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
            pts.push(P2::new(
                x + (b.x - a.x) * 4.0e-8 + sign * (b.y - a.y) * 2.0e-8,
                y + (b.y - a.y) * 4.0e-8 - sign * (b.x - a.x) * 2.0e-8,
            ));
        }
    }
    Polygon2::new(pts)
}

/// Load a polygon captured in the `{distance, exterior, holes, closed}` JSON
/// shape used by `test_data/cavalier_panic_polygon_r1.json`.
pub fn load_captured(path: &std::path::Path) -> Option<(Polygon2, f64)> {
    let raw = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let distance = v["distance"].as_f64().unwrap_or(0.0);
    let ring = |val: &serde_json::Value| -> Vec<P2> {
        val.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|p| Some(P2::new(p[0].as_f64()?, p[1].as_f64()?)))
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut poly = Polygon2::new(ring(&v["exterior"]));
    poly.closed = v["closed"].as_bool().unwrap_or(true);
    poly.holes = v["holes"]
        .as_array()
        .map(|a| a.iter().map(ring).collect())
        .unwrap_or_default();
    (poly.exterior.len() >= 3).then_some((poly, distance))
}

/// Serialise a polygon back into that same capture shape.
pub fn capture_json(poly: &Polygon2, distance: f64) -> String {
    let ring = |r: &[P2]| -> serde_json::Value {
        serde_json::Value::Array(r.iter().map(|p| serde_json::json!([p.x, p.y])).collect())
    };
    serde_json::json!({
        "distance": distance,
        "closed": poly.closed,
        "exterior": ring(&poly.exterior),
        "holes": serde_json::Value::Array(poly.holes.iter().map(|h| ring(h)).collect()),
    })
    .to_string()
}

/// Path of the mid-steep band polygon captured from `tests/fixtures/terrain.stl`
/// by `offset_growth_m5::capture_terrain_mid_steep_polygon`.
pub fn terrain_capture_path() -> std::path::PathBuf {
    super::repo_root().join("test_data/m5_terrain_mid_steep_polygon.json")
}

/// The captured WANAKA Back Rough terrain slice (86 vertices, 13 holes) — a
/// real polygon with real holes, already in the tree as the R1 panic asset.
pub fn wanaka_capture_path() -> std::path::PathBuf {
    super::repo_root().join("test_data/cavalier_panic_polygon_r1.json")
}

/// Every fixture the plan's research list names. Captured fixtures are
/// included only when their asset exists; callers report the gap rather than
/// silently measuring five shapes and calling it seven.
pub fn fixtures() -> Vec<OffsetFixture> {
    let mut v = vec![
        OffsetFixture {
            name: "square",
            what: "convex control — no reflex corners at all",
            poly: square(100.0),
            step: 0.1,
            authored_spacing: 0.5,
        },
        OffsetFixture {
            name: "rosette-24",
            what: "STRESS: 24 lobes, reflex valleys that survive erosion",
            poly: rosette(60.0, 8.0, 24, 720),
            step: 0.1,
            authored_spacing: 0.5,
        },
        OffsetFixture {
            name: "comb-16",
            what: "comb: 16 slots, 6 mm fingers that collapse at ring ~30",
            poly: comb(200.0, 80.0, 16, 6.0, 40.0),
            step: 0.1,
            authored_spacing: 0.5,
        },
        OffsetFixture {
            name: "dendrite",
            what: "branching concavity at two scales",
            poly: dendrite(200.0, 80.0, 12, 8.0, 40.0),
            step: 0.05,
            authored_spacing: 0.5,
        },
        OffsetFixture {
            name: "holed-9",
            what: "holes + disjoint outputs (Shape path)",
            poly: holed(200.0, 3, 15.0, 64),
            step: 0.1,
            authored_spacing: 1.4,
        },
        OffsetFixture {
            name: "short-edges",
            what: "near-degenerate: 1600 sub-eps segments",
            poly: short_edges(100.0, 200),
            step: 0.1,
            authored_spacing: 0.25,
        },
    ];
    if let Some((poly, _)) = load_captured(&wanaka_capture_path()) {
        v.push(OffsetFixture {
            name: "wanaka-slice",
            what: "CAPTURED REAL: Back Rough terrain slice, 13 holes",
            poly,
            step: 0.1,
            // Median segment 1.63 mm, p05 0.23 mm — a marching-squares slice
            // of a drop-cutter grid, conditioned. 0.5 mm is the density the
            // sparse end of it actually resolves.
            authored_spacing: 0.5,
        });
    }
    if let Some((poly, _)) = load_captured(&terrain_capture_path()) {
        v.push(OffsetFixture {
            name: "terrain-midsteep",
            what: "CAPTURED REAL: mid-steep band from terrain.stl",
            poly,
            step: 0.1,
            // Marching squares on the 0.75 mm classification grid: every
            // segment is 0.53-0.75 mm, so that IS the authored density.
            authored_spacing: 0.75,
        });
    }
    v
}

// ---------------------------------------------------------------------------
// Cleanup policies — the candidates, all reachable from one switch
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cleanup {
    /// What every consumer except scallop does today: keep whatever cavalier
    /// hands back, flattened to chords.
    Raw,
    /// Scallop's shipped compensation: drop any vertex within `min_spacing`
    /// of the last KEPT one. Never moves a point, never adds one.
    DropOnly { min_spacing: f64 },
}

impl Cleanup {
    pub fn label(self) -> String {
        match self {
            Self::Raw => "raw (production today)".to_owned(),
            Self::DropOnly { min_spacing } => format!("drop-only decimate @ {min_spacing:.3} mm"),
        }
    }

    /// Apply to one offset result. `None` culls the polygon (a sliver that
    /// cannot keep three points), matching scallop's `decimate_ring_polygon`.
    pub fn apply(self, poly: &Polygon2) -> Option<Polygon2> {
        match self {
            Self::Raw => Some(poly.clone()),
            Self::DropOnly { min_spacing } => decimate_ring_polygon(poly, min_spacing),
        }
    }
}

/// Scallop's `decimate_ring_polygon`, reproduced here so the harness can run
/// it on any fixture — the production one is private to `scallop.rs`. Kept
/// line-for-line identical to it; `drop_only_decimation_never_adds_a_point`
/// pins the property both copies rely on.
pub fn decimate_ring_polygon(poly: &Polygon2, min_spacing: f64) -> Option<Polygon2> {
    let min_spacing = min_spacing.max(1e-3);
    let exterior = decimate_closed_ring(&poly.exterior, min_spacing)?;
    let holes: Vec<Vec<P2>> = poly
        .holes
        .iter()
        .filter_map(|h| decimate_closed_ring(h, min_spacing))
        .collect();
    let mut out = Polygon2::new(exterior);
    out.holes = holes;
    Some(out)
}

pub fn decimate_closed_ring(ring: &[P2], min_spacing: f64) -> Option<Vec<P2>> {
    if ring.len() < 3 {
        return None;
    }
    let min_sq = min_spacing * min_spacing;
    let mut out: Vec<P2> = Vec::new();
    let mut last_kept = *ring.first()?;
    out.push(last_kept);
    for p in ring.iter().skip(1) {
        let dx = p.x - last_kept.x;
        let dy = p.y - last_kept.y;
        if dx * dx + dy * dy >= min_sq {
            out.push(*p);
            last_kept = *p;
        }
    }
    if out.len() >= 2
        && let (Some(first), Some(last)) = (out.first().copied(), out.last().copied())
    {
        let dx = first.x - last.x;
        let dy = first.y - last.y;
        if dx * dx + dy * dy < min_sq * 0.25 {
            out.pop();
        }
    }
    (out.len() >= 3).then_some(out)
}

// ---------------------------------------------------------------------------
// The cascade runner
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    /// The offset came back empty — the intended exit.
    Collapsed,
    /// Ran the requested number of rings.
    RingLimit,
    /// Blew the vertex cap. This is the failure the research is about.
    VertexCap,
    /// Blew the wall-clock budget.
    TimeBudget,
}

impl StopReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::Collapsed => "collapsed",
            Self::RingLimit => "ring limit",
            Self::VertexCap => "VERTEX CAP",
            Self::TimeBudget => "TIME BUDGET",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RingStat {
    pub ring: usize,
    pub polys: usize,
    pub verts: usize,
    /// Wall-clock for this ring's offsets + cleanup.
    pub secs: f64,
    /// Total absolute exterior area, holes subtracted.
    pub area: f64,
}

pub struct CascadeBudget {
    pub max_rings: usize,
    pub vertex_cap: usize,
    pub seconds: f64,
}

impl Default for CascadeBudget {
    fn default() -> Self {
        Self {
            max_rings: 200,
            vertex_cap: 120_000,
            seconds: 60.0,
        }
    }
}

pub struct Cascade {
    pub stats: Vec<RingStat>,
    pub stopped: StopReason,
    pub total_secs: f64,
    pub rings: Vec<Vec<Polygon2>>,
}

impl Cascade {
    pub fn last(&self) -> Option<&RingStat> {
        self.stats.last()
    }

    /// Geometric mean vertex growth per ring over the whole run, as a
    /// percentage. A linear (healthy) cascade sits at ~0%.
    pub fn geometric_growth_pct(&self) -> f64 {
        let (Some(first), Some(last)) = (self.stats.first(), self.stats.last()) else {
            return 0.0;
        };
        let n = self.stats.len();
        if n < 2 || first.verts == 0 {
            return 0.0;
        }
        let ratio = last.verts as f64 / first.verts as f64;
        (ratio.powf(1.0 / (n - 1) as f64) - 1.0) * 100.0
    }

    /// Worst single-ring vertex growth, as a percentage.
    pub fn worst_ring_growth_pct(&self) -> f64 {
        self.stats
            .windows(2)
            .filter(|w| w[0].verts > 0)
            .map(|w| (w[1].verts as f64 / w[0].verts as f64 - 1.0) * 100.0)
            .fold(f64::NEG_INFINITY, f64::max)
            .max(0.0)
    }
}

/// Run `fixture` through repeated inward offsets with `cleanup` applied after
/// every one. `keep_rings` retains the ring geometry for the oracle (memory:
/// an uncleaned rosette cascade is hundreds of thousands of points).
pub fn cascade(
    fixture: &OffsetFixture,
    cleanup: Cleanup,
    budget: &CascadeBudget,
    keep_rings: bool,
) -> Cascade {
    let t0 = Instant::now();
    let mut current = vec![fixture.poly.clone()];
    let mut stats = Vec::new();
    let mut rings = Vec::new();
    let mut stopped = StopReason::Collapsed;

    for ring in 1..=budget.max_rings {
        let t = Instant::now();
        let mut next = Vec::new();
        for p in &current {
            for off in offset_polygon(p, fixture.step) {
                if let Some(kept) = cleanup.apply(&off) {
                    next.push(kept);
                }
            }
        }
        let secs = t.elapsed().as_secs_f64();
        if next.is_empty() {
            stopped = StopReason::Collapsed;
            break;
        }
        let verts = total_vertices(&next);
        let area: f64 = next.iter().map(signed_area_with_holes).sum();
        stats.push(RingStat {
            ring,
            polys: next.len(),
            verts,
            secs,
            area,
        });
        if keep_rings {
            rings.push(next.clone());
        }
        current = next;

        if verts > budget.vertex_cap {
            stopped = StopReason::VertexCap;
            break;
        }
        if t0.elapsed().as_secs_f64() > budget.seconds {
            stopped = StopReason::TimeBudget;
            break;
        }
        if ring == budget.max_rings {
            stopped = StopReason::RingLimit;
        }
    }

    Cascade {
        stats,
        stopped,
        total_secs: t0.elapsed().as_secs_f64(),
        rings,
    }
}

pub fn signed_area_with_holes(p: &Polygon2) -> f64 {
    let ext = rs_cam_core::polygon::shoelace_area(&p.exterior).abs();
    let holes: f64 = p
        .holes
        .iter()
        .map(|h| rs_cam_core::polygon::shoelace_area(h).abs())
        .sum();
    (ext - holes).max(0.0)
}

// ---------------------------------------------------------------------------
// The oracle: distance to the ORIGINAL boundary
// ---------------------------------------------------------------------------

pub struct ErosionError {
    pub samples: usize,
    pub max_abs_mm: f64,
    pub p50_mm: f64,
    pub p95_mm: f64,
    /// Signed mean: positive means the ring sits FURTHER from the original
    /// boundary than it should (the cascade under-cuts), negative means it
    /// has crept inside the true erosion (it over-cuts).
    pub mean_signed_mm: f64,
}

/// Distance from `p` to the nearest edge of `poly` (exterior and holes).
pub fn distance_to_boundary(poly: &Polygon2, p: &P2) -> f64 {
    let mut best = f64::INFINITY;
    let mut scan = |ring: &[P2]| {
        for i in 0..ring.len() {
            let a = ring[i];
            let b = ring[(i + 1) % ring.len()];
            let d = point_segment_distance(p, &a, &b);
            if d < best {
                best = d;
            }
        }
    };
    scan(&poly.exterior);
    for h in &poly.holes {
        scan(h);
    }
    best
}

pub fn point_segment_distance(p: &P2, a: &P2, b: &P2) -> f64 {
    let (vx, vy) = (b.x - a.x, b.y - a.y);
    let len_sq = vx * vx + vy * vy;
    let t = if len_sq <= f64::MIN_POSITIVE {
        0.0
    } else {
        (((p.x - a.x) * vx + (p.y - a.y) * vy) / len_sq).clamp(0.0, 1.0)
    };
    let (cx, cy) = (a.x + vx * t, a.y + vy * t);
    ((p.x - cx).powi(2) + (p.y - cy).powi(2)).sqrt()
}

/// Every vertex of `rings` must lie at `nominal` mm from `∂original`. This
/// reports how badly that is violated. See the module header for why this is
/// exact rather than approximate.
pub fn erosion_error(original: &Polygon2, rings: &[Polygon2], nominal: f64) -> ErosionError {
    let mut errs = Vec::new();
    let mut signed_sum = 0.0;
    for poly in rings {
        let mut push = |ring: &[P2]| {
            for p in ring {
                let e = distance_to_boundary(original, p) - nominal;
                signed_sum += e;
                errs.push(e);
            }
        };
        push(&poly.exterior);
        for h in &poly.holes {
            push(h);
        }
    }
    if errs.is_empty() {
        return ErosionError {
            samples: 0,
            max_abs_mm: 0.0,
            p50_mm: 0.0,
            p95_mm: 0.0,
            mean_signed_mm: 0.0,
        };
    }
    let n = errs.len();
    let mean_signed_mm = signed_sum / n as f64;
    let mut abs: Vec<f64> = errs.iter().map(|e| e.abs()).collect();
    abs.sort_by(f64::total_cmp);
    ErosionError {
        samples: n,
        max_abs_mm: *abs.last().unwrap_or(&0.0),
        p50_mm: abs[n / 2],
        p95_mm: abs[(n * 95 / 100).min(n - 1)],
        mean_signed_mm,
    }
}

/// Rasterised truth for the area of `original` eroded by `d`, at `cell` mm
/// resolution. Slow by construction (distance transform by brute force);
/// use it on modest fixtures, in release.
pub fn erosion_area(original: &Polygon2, d: f64, cell: f64) -> f64 {
    let (mut lo_x, mut lo_y, mut hi_x, mut hi_y) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for p in &original.exterior {
        lo_x = lo_x.min(p.x);
        lo_y = lo_y.min(p.y);
        hi_x = hi_x.max(p.x);
        hi_y = hi_y.max(p.y);
    }
    if !lo_x.is_finite() {
        return 0.0;
    }
    let cols = (((hi_x - lo_x) / cell).ceil() as usize).max(1);
    let rows = (((hi_y - lo_y) / cell).ceil() as usize).max(1);
    let mut inside = 0usize;
    for r in 0..rows {
        for c in 0..cols {
            let p = P2::new(
                lo_x + (c as f64 + 0.5) * cell,
                lo_y + (r as f64 + 0.5) * cell,
            );
            if original.contains_point(&p) && distance_to_boundary(original, &p) >= d {
                inside += 1;
            }
        }
    }
    inside as f64 * cell * cell
}

// ---------------------------------------------------------------------------
// Attribution: where does each new vertex come from?
// ---------------------------------------------------------------------------

use cavalier_contours::polyline::{PlineCreation, PlineSource, PlineSourceMut, Polyline};
use cavalier_contours::shape_algorithms::Shape;

/// cavalier's own default `pos_equal_eps`, and the epsilon `polygon.rs` feeds
/// `remove_repeat_pos`.
pub const PLINE_POS_EQUAL_EPS: f64 = 1e-5;

/// One offset call, classified.
///
/// The classes are measured on the flattened output — i.e. on exactly the
/// vertices `Polygon2::from_pline` keeps — except `arc_*`, which can only be
/// seen BEFORE flattening and is the reason this instrument talks to
/// cavalier directly instead of going through `offset_polygon`.
#[derive(Clone, Copy, Debug, Default)]
pub struct RingAttribution {
    pub in_verts: usize,
    pub out_verts: usize,
    pub out_plines: usize,
    /// Output segments cavalier emitted as ARCS (bulge != 0). Each one is a
    /// join it inserted where the offset opened a gap.
    pub arc_segments: usize,
    /// Vertices that touch at least one arc segment. These are the points
    /// `from_pline` turns into a chord and hands back as if they had always
    /// been corners.
    pub arc_vertices: usize,
    /// Consecutive output vertices within `PLINE_POS_EQUAL_EPS`.
    pub duplicates: usize,
    /// Output vertices whose perpendicular deviation from the chord through
    /// their neighbours is under 1 nm — removable with no geometric effect
    /// whatsoever.
    pub collinear_1nm: usize,
    /// Same at 1 µm — removable well inside any operation tolerance in this
    /// workspace (the coarsest post quantum is 1 µm; finish tolerances are
    /// 10–100 µm).
    pub collinear_1um: usize,
}

impl RingAttribution {
    pub fn added(&self) -> i64 {
        self.out_verts as i64 - self.in_verts as i64
    }
    /// Vertices a lossless cleanup could remove.
    pub fn removable_lossless(&self) -> usize {
        self.duplicates + self.collinear_1nm
    }
}

fn poly_to_plines(poly: &Polygon2) -> Vec<Polyline<f64>> {
    let mut out = Vec::new();
    let mut ext = Polyline::with_capacity(poly.exterior.len(), true);
    for p in &poly.exterior {
        ext.add(p.x, p.y, 0.0);
    }
    let ext = ext.remove_repeat_pos(PLINE_POS_EQUAL_EPS).unwrap_or(ext);
    if ext.vertex_count() >= 3 {
        out.push(ext);
    }
    for hole in &poly.holes {
        let mut h = Polyline::with_capacity(hole.len(), true);
        for p in hole {
            h.add(p.x, p.y, 0.0);
        }
        let h = h.remove_repeat_pos(PLINE_POS_EQUAL_EPS).unwrap_or(h);
        if h.vertex_count() >= 3 {
            out.push(h);
        }
    }
    out
}

/// Offset once through the SAME cavalier entry points `polygon.rs` uses, and
/// classify the result. `polygon.rs` picks `Polyline::parallel_offset` when
/// there are no holes and `Shape::parallel_offset` when there are; this
/// mirrors that exactly so the attribution describes shipped behaviour.
pub fn attribute_offset(poly: &Polygon2, distance: f64) -> RingAttribution {
    // `offset_polygon` repairs self-intersecting input into pieces BEFORE
    // cavalier ever sees it (R1.5), and the captured terrain slices are
    // self-intersecting. Mirror that or the instrument describes a different
    // call than the one production makes.
    let pieces: Vec<Polygon2> = if poly.has_self_intersection() {
        poly.repaired()
    } else {
        vec![poly.clone()]
    };
    if pieces.len() != 1 {
        let mut agg = RingAttribution {
            in_verts: ring_vertex_count(poly),
            ..Default::default()
        };
        for piece in &pieces {
            let a = attribute_offset_one(piece, distance);
            agg.out_verts += a.out_verts;
            agg.out_plines += a.out_plines;
            agg.arc_segments += a.arc_segments;
            agg.arc_vertices += a.arc_vertices;
            agg.duplicates += a.duplicates;
            agg.collinear_1nm += a.collinear_1nm;
            agg.collinear_1um += a.collinear_1um;
        }
        return agg;
    }
    attribute_offset_one(pieces.first().unwrap_or(poly), distance)
}

fn attribute_offset_one(poly: &Polygon2, distance: f64) -> RingAttribution {
    let plines = poly_to_plines(poly);
    let in_verts: usize = plines.iter().map(PlineSource::vertex_count).sum();
    if plines.is_empty() {
        return RingAttribution::default();
    }

    let outputs: Vec<Polyline<f64>> = if poly.holes.is_empty() {
        plines
            .first()
            .map(|p| p.parallel_offset(distance))
            .unwrap_or_default()
    } else {
        let shape = Shape::from_plines(plines);
        let result = shape.parallel_offset(distance, Default::default());
        result
            .ccw_plines
            .into_iter()
            .map(|ip| ip.polyline)
            .chain(result.cw_plines.into_iter().map(|ip| ip.polyline))
            .collect()
    };

    let mut a = RingAttribution {
        in_verts,
        out_plines: outputs.len(),
        ..Default::default()
    };
    for pl in &outputs {
        let verts: Vec<(f64, f64, f64)> = pl.iter_vertexes().map(|v| (v.x, v.y, v.bulge)).collect();
        let n = verts.len();
        a.out_verts += n;
        if n < 2 {
            continue;
        }
        let mut touches_arc = vec![false; n];
        for (i, v) in verts.iter().enumerate() {
            if v.2.abs() > 1e-12 {
                a.arc_segments += 1;
                touches_arc[i] = true;
                touches_arc[(i + 1) % n] = true;
            }
        }
        a.arc_vertices += touches_arc.iter().filter(|t| **t).count();
        for i in 0..n {
            let prev = verts[(i + n - 1) % n];
            let cur = verts[i];
            let next = verts[(i + 1) % n];
            let d = ((cur.0 - prev.0).powi(2) + (cur.1 - prev.1).powi(2)).sqrt();
            if d < PLINE_POS_EQUAL_EPS {
                a.duplicates += 1;
                continue;
            }
            // Deviation of `cur` from the chord prev→next, i.e. what
            // dropping it would cost — measured on the FLATTENED geometry,
            // which is what production keeps.
            let dev = point_segment_distance(
                &P2::new(cur.0, cur.1),
                &P2::new(prev.0, prev.1),
                &P2::new(next.0, next.1),
            );
            if dev < 1e-6 {
                a.collinear_1nm += 1;
            }
            if dev < 1e-3 {
                a.collinear_1um += 1;
            }
        }
    }
    a
}

// ---------------------------------------------------------------------------
// Artifacts
// ---------------------------------------------------------------------------

/// `target/m5_offset/` — where every table, CSV, and SVG this bench writes
/// lands.
pub fn out_dir() -> std::path::PathBuf {
    let dir = super::repo_root().join("target/m5_offset");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Draw the original plus the first `limit` rings of a cascade. M4's lesson,
/// applied: a growth curve is a number, and a number is not a shape.
pub fn write_rings_svg(original: &Polygon2, rings: &[Vec<Polygon2>], limit: usize, name: &str) {
    let (mut lo_x, mut lo_y, mut hi_x, mut hi_y) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for p in &original.exterior {
        lo_x = lo_x.min(p.x);
        lo_y = lo_y.min(p.y);
        hi_x = hi_x.max(p.x);
        hi_y = hi_y.max(p.y);
    }
    if !lo_x.is_finite() {
        return;
    }
    let (w, h) = ((hi_x - lo_x).max(1e-6), (hi_y - lo_y).max(1e-6));
    let scale = 1000.0 / w.max(h);
    let sx = |x: f64| (x - lo_x) * scale;
    let sy = |y: f64| (hi_y - y) * scale;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{:.0}\" height=\"{:.0}\" \
         viewBox=\"0 0 {:.0} {:.0}\"><rect width=\"100%\" height=\"100%\" fill=\"#111\"/>",
        w * scale,
        h * scale,
        w * scale,
        h * scale
    );
    let ring_path = |ring: &[P2], stroke: &str, width: f64, svg: &mut String| {
        if ring.len() < 3 {
            return;
        }
        svg.push_str(&format!(
            "<path d=\"M {:.3} {:.3}",
            sx(ring[0].x),
            sy(ring[0].y)
        ));
        for p in ring.iter().skip(1) {
            svg.push_str(&format!(" L {:.3} {:.3}", sx(p.x), sy(p.y)));
        }
        svg.push_str(&format!(
            " Z\" fill=\"none\" stroke=\"{stroke}\" stroke-width=\"{width}\"/>"
        ));
    };
    for (i, layer) in rings.iter().take(limit).enumerate() {
        let t = i as f64 / limit.max(1) as f64;
        let stroke = format!("hsl({:.0},80%,60%)", 200.0 - 200.0 * t);
        for poly in layer {
            ring_path(&poly.exterior, &stroke, 0.8, &mut svg);
            for hole in &poly.holes {
                ring_path(hole, &stroke, 0.8, &mut svg);
            }
        }
    }
    ring_path(&original.exterior, "#fff", 1.6, &mut svg);
    for hole in &original.holes {
        ring_path(hole, "#fff", 1.6, &mut svg);
    }
    svg.push_str("</svg>");
    let path = out_dir().join(format!("{name}.svg"));
    let _ = std::fs::write(path, svg);
}
