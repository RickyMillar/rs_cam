//! Spiral construction: bridge the coverage rings into one spiral, lift it to
//! the surface and fill the report. Moved out of `finish/conformal_spiral.rs`
//! by P4; the item bodies are unchanged.

use std::f64::consts::{PI, TAU};

use super::{
    CentreCurve, EPS_CROSS, EPS_DISK, EPS_VEC, FlatLocator, LiftStats, Ring, Sample, SegGrid,
    SpiralParams, SpiralReport, SpiralResult, lift, median_sorted,
};
use crate::finish::direction_field::RegionMesh;
use crate::geo::{P3, V3, polyline_length};
use crate::mesh::QueryScratch;

// ---------------------------------------------------------------------------
// [SOURCE-2024 Eq. A-11 / A-12] The bridge blend σ(t)
// ---------------------------------------------------------------------------

/// **[SOURCE-2024 arXiv:2309.10655 v2, Eq. A-11 with A-12]**, verbatim:
///
/// ```text
/// σ(t) = 2π · v(t)^p / ( v(t)^p + v(2π − t)^p )
/// v(t) = (1/p − 1/2)·((π − t)/π)³ + (1/p)·((t − π)/π) + 1/2 ,  t ∈ [0, 2π]
/// ```
///
/// `p ≥ 2` is the grading parameter; **no value is given in either paper**
/// (**G-SIGMA-REFINEMENT**), so [`super::DEFAULT_BLEND_P`] is a [REPO] choice.
///
/// Properties the bridge relies on, all in the source: `σ` is a bijection of
/// `[0, 2π]` onto itself, strictly increasing, `C^∞`, and — the load-bearing
/// one — **`σ′(0) = σ′(2π) = 0`**, which is what makes the transition curve of
/// **[SOURCE-2025 Eq. 9]** tangent to both parallel lines it joins.
///
/// In its own paper σ is a *corner-grading reparameterisation of a boundary*,
/// not a bridge blend; the 2025 paper reuses its shape. That repurposing is
/// the 2025 paper's, not this module's, and the "refining" it mentions is
/// specified in neither text — so σ is used here unrefined.
#[must_use]
pub fn blend_sigma(t: f64, p: f64) -> f64 {
    let p = p.max(2.0);
    let v = |x: f64| -> f64 {
        let a = (1.0 / p - 0.5) * ((PI - x) / PI).powi(3);
        let b = (1.0 / p) * ((x - PI) / PI);
        (a + b + 0.5).clamp(0.0, 1.0)
    };
    let num = v(t).powf(p);
    let den = num + v(TAU - t).powf(p);
    if !den.is_finite() || den <= f64::MIN_POSITIVE {
        return 0.0;
    }
    TAU * num / den
}

// ---------------------------------------------------------------------------
// [SOURCE-2025 §2.2.2, Eqs. 7–9] Bridging the rings into one spiral
// ---------------------------------------------------------------------------

/// **[SOURCE-2025 Eq. 8]** `S^S = imag(S^R) · e^{i·real(S^R)}` — roll one
/// rectangle point back to the disk. (Eq. 7 is its inverse,
/// `arg(z) + i·|z|`; only this direction is needed, because the rings are
/// *constructed* in rectangle coordinates rather than measured from the disk.)
fn roll_to_disk(real: f64, imag: f64) -> (f64, f64) {
    (imag * real.cos(), imag * real.sin())
}

/// Everything the report needs about one built spiral.
pub(super) struct SpiralMeta {
    /// The emitted spiral's tool-centre polyline, kept so the independent
    /// coverage audit can test against the path that will actually be cut.
    pub(super) centre: Vec<P3>,
    start_angle: f64,
    candidates: usize,
    bridge_count: usize,
    bridge_length_mm: f64,
    repair_steps: usize,
    uncovered_after_bridging: usize,
}

/// The rectangle-domain construction, before any lifting.
struct SpiralDomain {
    disk: Vec<(f64, f64)>,
    /// `true` for a point emitted by a bridge. Segment `k → k+1` is a bridge
    /// segment exactly when `is_bridge[k + 1]`.
    is_bridge: Vec<bool>,
}

/// Round an angular shift onto the shared lattice.
fn shift_cells(shift: f64, dtheta: f64, floor: usize) -> usize {
    let raw = (shift / dtheta).round();
    if !raw.is_finite() || raw < floor as f64 {
        floor
    } else {
        raw as usize
    }
}

/// Build the spiral in the `(angle, radius)` rectangle and roll it to the disk.
///
/// **[SOURCE-2025 Eqs. 7–9]**, with the **[REPO]** bookkeeping resolution of
/// the module header's **G-BRIDGE-BOOKKEEPING**: run `i` spans
/// `2π + line_shift_i` of angle at radius `R_i`, then a bridge of angular span
/// `D_i` descends to `R_{i+1}` along Eq. 9. `D_i` follows the paper's own
/// near-centre rule (Pseudocode A-2 lines 6–9). The last ring gets its run and
/// **no trailing bridge**.
///
/// Both runs and bridges are sampled on **one** lattice `a₀ + j·2π/N_C`; see
/// [`SpiralParams::n_angular_samples`] for why that matters to the
/// self-intersection measurement.
fn build_spiral_domain(
    rings: &[Ring],
    a0: f64,
    line_shift_cells: &[usize],
    params: &SpiralParams,
) -> SpiralDomain {
    let n = params.n_angular_samples.max(3);
    let dtheta = TAU / (n as f64);
    let mut disk: Vec<(f64, f64)> = Vec::new();
    let mut is_bridge: Vec<bool> = Vec::new();
    let mut push = |real: f64, imag: f64, bridge: bool| {
        let p = roll_to_disk(real, imag);
        if let Some(&last) = disk.last()
            && (last.0 - p.0).hypot(last.1 - p.1) <= EPS_DISK
        {
            return;
        }
        disk.push(p);
        is_bridge.push(bridge);
    };

    let mut real = a0;
    for (i, ring) in rings.iter().enumerate() {
        let run = n + line_shift_cells.get(i).copied().unwrap_or(0);
        let first = usize::from(i > 0);
        for j in first..=run {
            push(real + (j as f64) * dtheta, ring.radius, false);
        }
        real += (run as f64) * dtheta;

        let Some(next) = rings.get(i + 1) else {
            continue;
        };
        let shift = if ring.radius > params.near_centre_radius {
            params.initial_bridge_shift
        } else {
            params.near_centre_bridge_shift
        };
        let bc = shift_cells(shift, dtheta, 1);
        let (r0, r1) = (ring.radius, next.radius);
        for j in 1..=bc {
            let t = (j as f64) / (bc as f64);
            let imag = r0 + (r1 - r0) * blend_sigma(TAU * t, params.blend_p) / TAU;
            push(real + (j as f64) * dtheta, imag, true);
        }
        real += (bc as f64) * dtheta;
    }
    SpiralDomain { disk, is_bridge }
}

/// A lifted disk polyline: disk domain, bridge flags, 3D contact points and
/// 3D tool-centre points, all four index-aligned.
pub(super) struct Lifted {
    disk: Vec<(f64, f64)>,
    flags: Vec<bool>,
    pub(super) contact: Vec<P3>,
    pub(super) centre: Vec<P3>,
}

/// Lift a disk polyline, **keeping the disk points index-aligned** with the
/// 3D output: a point that cannot be located even after the
/// [`super::PULLBACK_LADDER`] is dropped from all four lists at once, so
/// [`SpiralResult::spiral_disk`] and [`SpiralResult::spiral_contact`] can
/// never drift apart.
#[allow(clippy::too_many_arguments)]
pub(super) fn lift_disk_aligned(
    locator: &FlatLocator,
    region: &RegionMesh,
    normals: &[V3],
    disk: &[(f64, f64)],
    flags: &[bool],
    radius: f64,
    scratch: &mut QueryScratch,
    hits: &mut Vec<usize>,
    stats: &mut LiftStats,
) -> Lifted {
    let mut kept_disk: Vec<(f64, f64)> = Vec::with_capacity(disk.len());
    let mut kept_flags: Vec<bool> = Vec::with_capacity(disk.len());
    let mut contact: Vec<P3> = Vec::with_capacity(disk.len());
    let mut centre: Vec<P3> = Vec::with_capacity(disk.len());
    for (i, &(x, y)) in disk.iter().enumerate() {
        let Some((loc, pulled)) = locator.locate(x, y, scratch, hits) else {
            stats.unlocated += 1;
            continue;
        };
        if pulled {
            stats.pulled_back += 1;
        }
        let (p, n) = lift(region, normals, &loc);
        kept_disk.push((x, y));
        kept_flags.push(flags.get(i).copied().unwrap_or(false));
        contact.push(p);
        centre.push(P3::from(p.coords + n * radius));
    }
    Lifted {
        disk: kept_disk,
        flags: kept_flags,
        contact,
        centre,
    }
}

/// Build the spiral for every start angle in the sweep and keep the shortest,
/// then run the §2.2.2 step-2 coverage repair on the winner.
///
/// **[SOURCE-2025 §2.2.2, A-2 lines 3 and 30–35]** sweeps `real(P_Start^1)`
/// over `[0, 2π)` and keeps the minimum-total-3D-length spiral.
///
/// **[REPO ordering]** The paper's pseudocode repairs coverage inside the
/// sweep. Here the sweep runs with the initial shifts and the repair runs once
/// on the winner, which is materially the same because in the simply-connected
/// case the repair is a no-op by construction — each run already traverses its
/// whole ring at that ring's own radius — and `bridge_repair_steps` reports
/// whether that held.
pub(super) fn build_best_spiral(
    locator: &FlatLocator,
    normals: &[V3],
    region: &RegionMesh,
    rings: &[Ring],
    samples: &[Sample],
    params: &SpiralParams,
    report: &mut SpiralReport,
) -> (SpiralResult, SpiralMeta) {
    let radius = params.ball_radius_mm;
    // The repair must judge by the same criterion the search used, or it would
    // chase a bar the rings were never placed against.
    let search_radius = params.ring_search_radius_mm();
    let mut scratch = QueryScratch::new();
    let mut hits: Vec<usize> = Vec::new();
    let mut stats = LiftStats::default();
    let dtheta = params.lattice_step();

    // Initial along-line shifts. The paper's secondary constant applies to the
    // ring *after* an outer bridge; it is 0 by default here — see the module
    // header's G-BRIDGE-BOOKKEEPING.
    let secondary = shift_cells(params.secondary_line_shift, dtheta, 0);
    let mut line_shift_cells: Vec<usize> = (0..rings.len())
        .map(|i| {
            let after_outer = i > 0
                && rings
                    .get(i - 1)
                    .is_some_and(|prev| prev.radius > params.near_centre_radius);
            if after_outer { secondary } else { 0 }
        })
        .collect();

    let step = if params.start_angle_step > 1e-6 {
        params.start_angle_step
    } else {
        TAU
    };
    let candidates = ((TAU / step).floor() as usize).max(1);

    // Start-angle sweep: keep the minimum-total-3D-length spiral.
    let mut best_angle = 0.0_f64;
    let mut best_len = f64::INFINITY;
    let mut kept_disk: Vec<(f64, f64)> = Vec::new();
    let mut kept_flags: Vec<bool> = Vec::new();
    let mut contact: Vec<P3> = Vec::new();
    let mut centre: Vec<P3> = Vec::new();
    for k in 0..candidates {
        // **[REPO] lattice discretisation of the sweep.** The start angle is
        // snapped onto the same `2π/N_C` lattice as everything else. Without
        // this, a run's chords sit out of phase with the ring chords that made
        // the coverage decision, and a band sample within one chord sagitta of
        // the `K_c` boundary reads uncovered — which the repair loop *cannot*
        // fix, because growing the along-line shift re-traverses the same
        // lattice at the same phase. Snapping is also arguably the more
        // faithful reading: the paper's own π/50 step is exactly one cell of a
        // 100-point lattice.
        let a0 = ((k as f64) * step / dtheta).round() * dtheta;
        let dom = build_spiral_domain(rings, a0, &line_shift_cells, params);
        let lifted = lift_disk_aligned(
            locator,
            region,
            normals,
            &dom.disk,
            &dom.is_bridge,
            radius,
            &mut scratch,
            &mut hits,
            &mut stats,
        );
        let len = polyline_length(&lifted.contact);
        if len < best_len {
            best_len = len;
            best_angle = a0;
            kept_disk = lifted.disk;
            kept_flags = lifted.flags;
            contact = lifted.contact;
            centre = lifted.centre;
        }
    }

    // SOURCE-2025 §2.2.2 step 2: grow the along-line shift until every ring's
    // band is still swept by the *bridged* spiral's centre curve.
    let mut repair_steps = 0usize;
    let mut uncovered_after;
    let mut previous_uncovered = usize::MAX;
    loop {
        let cc = CentreCurve::new(&centre, radius);
        let mut worst: Option<usize> = None;
        let mut uncovered = 0usize;
        for (i, ring) in rings.iter().enumerate() {
            let missing = ring
                .band
                .iter()
                .filter(|&&s| {
                    samples
                        .get(s)
                        .is_some_and(|p| !cc.covers(p.at, search_radius))
                })
                .count();
            if missing > 0 {
                uncovered += missing;
                if worst.is_none() {
                    worst = Some(i);
                }
            }
        }
        uncovered_after = uncovered;
        let Some(i) = worst else {
            break;
        };
        if repair_steps >= params.max_bridge_repair_steps {
            break;
        }
        // Stall guard: a shift that removes nothing will never remove
        // anything, and 200 fruitless full rebuilds is a hang, not a repair.
        if uncovered >= previous_uncovered {
            break;
        }
        previous_uncovered = uncovered;
        let bump = shift_cells(params.shift_step, dtheta, 1);
        if let Some(slot) = line_shift_cells.get_mut(i) {
            *slot += bump;
        }
        repair_steps += 1;
        let dom = build_spiral_domain(rings, best_angle, &line_shift_cells, params);
        let lifted = lift_disk_aligned(
            locator,
            region,
            normals,
            &dom.disk,
            &dom.is_bridge,
            radius,
            &mut scratch,
            &mut hits,
            &mut stats,
        );
        kept_disk = lifted.disk;
        kept_flags = lifted.flags;
        contact = lifted.contact;
        centre = lifted.centre;
    }

    report.ring_points_pulled_back += stats.pulled_back;
    report.ring_points_unlocated += stats.unlocated;

    // A segment is a bridge segment exactly when its later endpoint is one.
    let mut bridge_length = 0.0_f64;
    for k in 0..contact.len().saturating_sub(1) {
        if !kept_flags.get(k + 1).copied().unwrap_or(false) {
            continue;
        }
        let (Some(&a), Some(&b)) = (contact.get(k), contact.get(k + 1)) else {
            continue;
        };
        bridge_length += (b - a).norm();
    }

    let result = SpiralResult {
        spiral_contact: contact,
        spiral_disk: kept_disk,
        rings_contact: rings.iter().map(|r| r.contact.clone()).collect(),
    };
    let meta = SpiralMeta {
        centre,
        start_angle: best_angle,
        candidates,
        bridge_count: rings.len().saturating_sub(1),
        bridge_length_mm: bridge_length,
        repair_steps,
        uncovered_after_bridging: uncovered_after,
    };
    (result, meta)
}

// ---------------------------------------------------------------------------
// [REPO] Measurements on the finished spiral
// ---------------------------------------------------------------------------

/// Count **proper** segment-segment crossings of the spiral in the disk
/// domain.
///
/// Measured rather than inferred. The construction argues that concentric
/// runs at distinct radii, joined by radius-monotone bridges, cannot cross —
/// but that argument is about the continuum, and the emitted path is chords.
/// Only crossings in the open interior of both segments count: a run's first
/// and last points coincide (a full turn), and every bridge shares an endpoint
/// with the runs on either side, so endpoint contact is expected and is not a
/// crossing.
fn count_disk_self_intersections(disk: &[(f64, f64)]) -> usize {
    let n = disk.len().saturating_sub(1);
    if n < 2 {
        return 0;
    }
    let mut boxes: Vec<[f64; 4]> = Vec::with_capacity(n);
    for k in 0..n {
        let (Some(&a), Some(&b)) = (disk.get(k), disk.get(k + 1)) else {
            continue;
        };
        boxes.push([a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1)]);
    }
    // One cell per few segments keeps the candidate lists short on a disk of
    // radius 1 without letting the grid explode.
    let target = (2.0 / (n as f64).sqrt()).max(1e-6);
    let grid = SegGrid::build(&boxes, 0.0, target);
    let mut count = 0usize;
    for (k, bx) in boxes.iter().enumerate() {
        for j in grid.in_box(bx) {
            let j = j as usize;
            if j <= k + 1 {
                continue;
            }
            let (Some(&a), Some(&b), Some(&c), Some(&d)) =
                (disk.get(k), disk.get(k + 1), disk.get(j), disk.get(j + 1))
            else {
                continue;
            };
            if segments_properly_cross(a, b, c, d) {
                count += 1;
            }
        }
    }
    count
}

/// 2D cross product of `(b − a)` and `(c − a)`.
fn cross2(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

/// Proper crossing only: both orientation pairs must have strictly opposite
/// signs, with `EPS_CROSS` treating a touching or collinear configuration as
/// no crossing.
fn segments_properly_cross(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let d1 = cross2(a, b, c);
    let d2 = cross2(a, b, d);
    let d3 = cross2(c, d, a);
    let d4 = cross2(c, d, b);
    if d1.abs() <= EPS_CROSS
        || d2.abs() <= EPS_CROSS
        || d3.abs() <= EPS_CROSS
        || d4.abs() <= EPS_CROSS
    {
        return false;
    }
    (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
}

/// Fill the spiral-side report rows.
pub(super) fn finish_report(
    result: &SpiralResult,
    rings: &[Ring],
    meta: &SpiralMeta,
    _params: &SpiralParams,
    report: &mut SpiralReport,
) {
    report.start_angle_rad = meta.start_angle;
    report.start_angle_candidates = meta.candidates;
    report.bridge_count = meta.bridge_count;
    report.total_bridge_length_mm = meta.bridge_length_mm;
    report.bridge_repair_steps = meta.repair_steps;
    report.uncovered_after_bridging = meta.uncovered_after_bridging;

    report.spiral_points = result.spiral_contact.len();
    report.spiral_length_mm = polyline_length(&result.spiral_contact);
    let ring_total: f64 = rings.iter().map(|r| polyline_length(&r.contact)).sum();
    report.bridge_overhead_pct = if ring_total > EPS_VEC {
        100.0 * (report.spiral_length_mm - ring_total) / ring_total
    } else {
        0.0
    };
    // Structural: one polyline, no lift anywhere in the construction.
    report.retract_count = 0;

    let mut steps: Vec<f64> = result
        .spiral_contact
        .windows(2)
        .filter_map(|w| Some((*w.first()?, *w.get(1)?)))
        .map(|(a, b)| (b - a).norm())
        .collect();
    steps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    report.max_consecutive_step_mm = steps.last().copied().unwrap_or(0.0);
    report.median_consecutive_step_mm = median_sorted(&steps);

    report.disk_self_intersections = count_disk_self_intersections(&result.spiral_disk);
}
