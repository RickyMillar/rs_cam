//! Chamfer Clearance (operator ruling 2026-10-23; the industry default is
//! the Fusion "Chamfer Clearance", `planning/geometry_selection_2026-10-16/
//! INDUSTRY_DEFAULTS.md` Q1): THE wall trim of the model edges of a
//! Chamfer.
//!
//! A convex top edge of a STEP model can end where it meets a higher wall
//! (an inside corner: the top edge of a lower step runs into the wall of a
//! higher step). A V-bit along the full edge cuts into that wall with its
//! cone. The trim stops the edge short of the wall. It does not block the
//! op, and it does not refuse.
//!
//! # The rule
//!
//! The tip of the V-bit runs `t` (the tip offset) into the air from the
//! edge, at `z - (w + t) / tan(a)` below the edge Z `z`. At a point of the
//! edge, the tool is CLEAR when the drop-cutter CL Z of the tool on the
//! model mesh at that tip position is no higher than `z - t / tan(a)`, plus
//! [`MODEL_GOUGE_Z_TOL_MM`]. That is the test of the gouge guard
//! (`ModelGougeRule::Bevel`: the tip may go `w / tan(a)` below the CL Z),
//! with the full tool in place of the reduced probe. The trim keeps the
//! clear stretches of each edge:
//!
//! - An open edge keeps each clear stretch as a run.
//! - A closed edge that is clear along all of its length stays closed. A
//!   closed edge with a stretch that is not clear becomes open runs.
//! - An edge with no clear stretch is not cut: its item is `Skipped`, with
//!   a [`NoteCause::AtAWall`] note.
//!
//! # The clearance
//!
//! The trim distance comes from the tool and the model, not from a typed
//! value. The full tool cone (radius `R`) must not reach the wall: on the
//! fixture `stepped_block.step` the wall is 10 mm high, so the cone reaches
//! it at its full radius, and the tool axis stops `sqrt(R^2 - t^2)` along
//! the edge from the wall corner. Each run end then moves a further
//! [`CAD_ROUNDING_MM`] away from the wall: that is the one length in the
//! repo below which two lengths are one value (lead ruling L-13), so a run
//! end is never ON the limit. The guard probes the tool less
//! `MODEL_GOUGE_TOL_MM`, which is inside the full tool, so a run that is
//! clear here is clear for the guard. The guard still reads the emitted
//! motion.
//!
//! # The search
//!
//! The samples along each edge are at most `(w + t) / 2` apart. A wall at
//! the edge Z that stands within `(w + t) sqrt(3) / 2` of the tool axis
//! line blocks a stretch at least `w + t` long, so a sample finds it. A
//! bisection then finds each limit between a clear sample and a sample
//! that is not clear, to [`CAD_ROUNDING_MM`].
//!
//! # The readers
//!
//! The prologue runs the trim after the view (`execute::prologue`), so the
//! generation and the buried check (the band of each edge) read the
//! trimmed runs. The selection status runs the same trim on the same mesh
//! and tool, so the status and the generation give one answer.

use std::collections::BTreeMap;

use crate::compute::model_gouge::MODEL_GOUGE_Z_TOL_MM;
use crate::geo::{P2, V2};
use crate::mesh::{SpatialIndex, TriangleMesh};
use crate::polygon::Polygon2;
use crate::selection::{
    CAD_ROUNDING_MM, EdgeLine, EdgeMaterial, ItemKind, ItemRef, NoteCause, OpItems, RefusalCause,
    ResolvedSelection, SelectionFrame, SelectionNote, SelectionRefusal,
};
use crate::surface::dropcutter::point_drop_cutter;
use crate::tool::MillingCutter;

/// The inputs of THE wall trim: the model mesh in the frame of the edges,
/// the V-bit and the bevel of the op.
pub struct EdgeClearProbe<'a> {
    pub mesh: &'a TriangleMesh,
    pub index: &'a SpatialIndex,
    /// The V-bit of the op, at its full size.
    pub cutter: &'a dyn MillingCutter,
    /// The half angle of the V-bit (radians).
    pub half_angle_rad: f64,
    /// The chamfer width `w` (mm).
    pub chamfer_width: f64,
    /// The tip offset `t` (mm): the tip runs this far into the air.
    pub tip_offset: f64,
}

impl EdgeClearProbe<'_> {
    /// The highest CL Z of the tool that leaves it clear at an edge of Z
    /// `z`: `z - t / tan(a)`.
    fn clear_cl_z(&self, z: f64) -> f64 {
        z - self.tip_offset / self.half_angle_rad.tan()
    }

    /// The sample spacing along an edge (see the module doc).
    fn spacing(&self) -> f64 {
        ((self.chamfer_width + self.tip_offset) / 2.0).max(CAD_ROUNDING_MM)
    }

    /// True when the tool is clear with its axis `t` into the air from the
    /// edge point `p` (unit air normal `air`), for an edge at Z `z`.
    fn clear_at(&self, p: P2, air: V2, z: f64) -> bool {
        let q = p + air * self.tip_offset;
        let cl = point_drop_cutter(q.x, q.y, self.mesh, self.index, self.cutter);
        !cl.contacted || cl.z - self.clear_cl_z(z) <= MODEL_GOUGE_Z_TOL_MM
    }
}

/// What the trim did to one edge line.
#[derive(Debug, Clone)]
pub(crate) struct LineTrim {
    /// The runs that stay. Empty when the tool is clear nowhere.
    pub runs: Vec<EdgeLine>,
    /// The length (mm, along the edge) that each stopped end gives up: one
    /// entry per run end that stops short of a wall.
    pub stops: Vec<f64>,
}

/// THE wall trim of the edge lines of an `Edges` view (see the module
/// doc). It puts the trimmed runs in place of the lines of `items`, marks
/// an item with no run left `Skipped`, and returns the notes: one
/// [`NoteCause::StopsShort`] note for the ends that stop short, and one
/// [`NoteCause::AtAWall`] note per kind for the items with no run left.
/// A user-picked edge gets the same trim: the trim keeps the tool out of
/// the model, it is not a default.
///
/// # Errors
///
/// `Empty` (with the notes) when no run and no drawing area is left.
pub fn clear_edge_walls<F: SelectionFrame>(
    resolved: &ResolvedSelection<F>,
    items: &mut OpItems,
    probe: &EdgeClearProbe<'_>,
) -> Result<Vec<SelectionNote>, SelectionRefusal> {
    let Some(edges) = items.edges() else {
        return Ok(Vec::new());
    };
    if edges.lines().is_empty() {
        return Ok(Vec::new());
    }
    let areas_left = !edges.regions().as_slice().is_empty();
    let mut kept: Vec<EdgeLine> = Vec::new();
    // For each item: (runs left, ends stopped), in item order.
    let mut by_item: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
    let mut stops: Vec<f64> = Vec::new();
    for line in edges.lines() {
        let trim = trim_line(line, probe);
        let entry = by_item.entry(line.item).or_insert((0, 0));
        entry.0 += trim.runs.len();
        entry.1 += trim.stops.len();
        stops.extend(trim.stops);
        kept.extend(trim.runs);
    }
    let mut notes = Vec::new();
    let item_of = |at: usize| resolved.items().get(at);
    let stopped: Vec<usize> = by_item
        .iter()
        .filter(|(_, (runs, ends))| *runs > 0 && *ends > 0)
        .map(|(at, _)| *at)
        .collect();
    if let Some(r) = stopped.first().and_then(|&at| item_of(at)) {
        notes.push(SelectionNote {
            kind: r.item().kind(),
            layer: r.item().layer().map(str::to_owned),
            count: stopped.len(),
            first: ItemRef::new(r.identity().clone()),
            item_use: crate::compute::catalog::ItemUse::Edges,
            cause: NoteCause::StopsShort {
                ends: stops.len(),
                min_mm: stops.iter().copied().fold(f64::INFINITY, f64::min),
                max_mm: stops.iter().copied().fold(0.0, f64::max),
            },
        });
    }
    let mut skipped: Vec<(usize, ItemKind)> = Vec::new();
    let mut at_a_wall: BTreeMap<ItemKind, (usize, usize)> = BTreeMap::new();
    for (&at, &(runs, _)) in &by_item {
        if runs > 0 {
            continue;
        }
        let Some(r) = item_of(at) else {
            continue;
        };
        let kind = r.item().kind();
        skipped.push((at, kind));
        at_a_wall.entry(kind).or_insert((0, at)).0 += 1;
    }
    for (kind, (count, first_at)) in at_a_wall {
        let Some(r) = item_of(first_at) else {
            continue;
        };
        notes.push(SelectionNote {
            kind,
            layer: r.item().layer().map(str::to_owned),
            count,
            first: ItemRef::new(r.identity().clone()),
            item_use: crate::compute::catalog::ItemUse::Edges,
            cause: NoteCause::AtAWall,
        });
    }
    let none_left = kept.is_empty() && !areas_left;
    items.replace_edge_lines(kept, &skipped);
    if none_left {
        return Err(SelectionRefusal::new(
            None,
            RefusalCause::Empty {
                skipped: notes,
                rims: Vec::new(),
            },
        ));
    }
    Ok(notes)
}

/// One edge line, read by arc length: the points, the arc length at each
/// point, and the air normal of each segment.
struct Edge<'a> {
    points: &'a [P2],
    closed: bool,
    /// The arc length at each point, and the total length at the end.
    at: Vec<f64>,
    /// The unit air normal of each segment.
    air: Vec<V2>,
}

impl Edge<'_> {
    fn length(&self) -> f64 {
        self.at.last().copied().unwrap_or(0.0)
    }

    fn segment_count(&self) -> usize {
        self.air.len()
    }

    /// The end points of segment `i`.
    fn segment(&self, i: usize) -> Option<(P2, P2)> {
        let a = *self.points.get(i)?;
        let b = if i + 1 == self.points.len() {
            *self.points.first()?
        } else {
            *self.points.get(i + 1)?
        };
        Some((a, b))
    }

    /// The point at arc length `s` (clamped to the edge), and the index of
    /// its segment.
    fn point_at(&self, s: f64) -> Option<(P2, usize)> {
        let n = self.segment_count();
        let i = self
            .at
            .partition_point(|&a| a <= s)
            .saturating_sub(1)
            .min(n.checked_sub(1)?);
        let (a, b) = self.segment(i)?;
        let len = self.at.get(i + 1)? - self.at.get(i)?;
        let u = if len > 0.0 {
            ((s - self.at.get(i)?) / len).clamp(0.0, 1.0)
        } else {
            0.0
        };
        Some((a + (b - a) * u, i))
    }

    /// True when the tool is clear at arc length `s`. At a vertex both
    /// segments must be clear. A segment of zero length has no air side,
    /// so it states nothing.
    fn clear(&self, s: f64, z: f64, probe: &EdgeClearProbe<'_>) -> bool {
        let Some((p, i)) = self.point_at(s) else {
            return true;
        };
        let mut segments = vec![i];
        let n = self.segment_count();
        let start = self.at.get(i).copied().unwrap_or(0.0);
        let end = self.at.get(i + 1).copied().unwrap_or(0.0);
        if (s - start).abs() <= f64::EPSILON * start.abs().max(1.0) {
            if i > 0 {
                segments.push(i - 1);
            } else if self.closed {
                segments.push(n - 1);
            }
        }
        if (end - s).abs() <= f64::EPSILON * end.abs().max(1.0) {
            if i + 1 < n {
                segments.push(i + 1);
            } else if self.closed {
                segments.push(0);
            }
        }
        segments
            .into_iter()
            .filter_map(|j| self.air.get(j))
            .filter(|air| air.norm_squared() > 0.0)
            .all(|&air| probe.clear_at(p, air, z))
    }

    /// The points of the edge from arc length `s0` to `s1` (`s1 > s0`; on a
    /// closed edge `s1` may pass the end once).
    fn sub_path(&self, s0: f64, s1: f64) -> Vec<P2> {
        let total = self.length();
        let wrap = |s: f64| if s > total { s - total } else { s };
        let mut out = Vec::new();
        if let Some((p, _)) = self.point_at(wrap(s0)) {
            out.push(p);
        }
        let laps: &[f64] = if self.closed { &[0.0, 1.0] } else { &[0.0] };
        for &lap in laps {
            for (k, &a) in self.at.iter().enumerate().take(self.points.len()) {
                let s = a + lap * total;
                if s > s0
                    && s < s1
                    && let Some(&p) = self.points.get(k)
                {
                    out.push(p);
                }
            }
        }
        if let Some((p, _)) = self.point_at(wrap(s1)) {
            out.push(p);
        }
        out
    }
}

/// The trim of one edge line (see the module doc).
pub(crate) fn trim_line(line: &EdgeLine, probe: &EdgeClearProbe<'_>) -> LineTrim {
    let unchanged = || LineTrim {
        runs: vec![line.clone()],
        stops: Vec::new(),
    };
    let pts = &line.line.exterior;
    let closed = line.line.closed;
    let segment_count = if closed {
        pts.len()
    } else {
        pts.len().saturating_sub(1)
    };
    if segment_count == 0 {
        return unchanged();
    }
    // The air side of each segment: the material side the model states,
    // read on the winding for a closed edge (left of a CCW ring is inside).
    let ccw = line.line.signed_area() > 0.0;
    let material_left = match line.material {
        EdgeMaterial::Left => true,
        EdgeMaterial::Right => false,
        EdgeMaterial::Inside => ccw,
        EdgeMaterial::Outside => !ccw,
    };
    let mut at = vec![0.0];
    let mut air = Vec::with_capacity(segment_count);
    for i in 0..segment_count {
        let (Some(&a), Some(&b)) = (pts.get(i), pts.get((i + 1) % pts.len())) else {
            return unchanged();
        };
        let len = (b - a).norm();
        let d = if len > 0.0 {
            (b - a) / len
        } else {
            V2::zeros()
        };
        let left = V2::new(-d.y, d.x);
        air.push(if material_left { -left } else { left });
        at.push(at.last().copied().unwrap_or(0.0) + len);
    }
    let edge = Edge {
        points: pts,
        closed,
        at,
        air,
    };
    let total = edge.length();
    if total <= CAD_ROUNDING_MM {
        return unchanged();
    }
    let z = line.z;
    let Some(n) = crate::util::steps::steps(total, probe.spacing()) else {
        return unchanged();
    };
    let n = n.max(1);
    #[expect(
        clippy::cast_precision_loss,
        reason = "n is a sample count, far below 2^52"
    )]
    let step = total / n as f64;
    #[expect(
        clippy::cast_precision_loss,
        reason = "k is a sample index, far below 2^52"
    )]
    let samples: Vec<(f64, bool)> = (0..=n)
        .map(|k| {
            let s = if k == n { total } else { k as f64 * step };
            (s, edge.clear(s, z, probe))
        })
        .collect();
    if samples.iter().all(|&(_, clear)| clear) {
        return unchanged();
    }
    if samples.iter().all(|&(_, clear)| !clear) {
        return LineTrim {
            runs: Vec::new(),
            stops: Vec::new(),
        };
    }
    // A closed edge: walk from a blocked sample, so each clear stretch has
    // a blocked sample on both sides. `offset` is the arc length of the
    // walk start; the walk ends at the same sample (walk length `total`).
    let offset = if closed {
        samples
            .iter()
            .take(n)
            .find(|&&(_, c)| !c)
            .map_or(0.0, |&(s, _)| s)
    } else {
        0.0
    };
    let wrap = |s: f64| if s >= total { s - total } else { s };
    let walk: Vec<(f64, bool)> = if closed {
        let mut walk: Vec<(f64, bool)> = samples
            .iter()
            .take(n)
            .map(|&(s, c)| (wrap(s - offset + total), c))
            .collect();
        walk.sort_by(|a, b| a.0.total_cmp(&b.0));
        walk.push((total, false));
        walk
    } else {
        samples
    };
    let clear_walk = |w: f64| edge.clear(wrap(w + offset), z, probe);
    // The limit between the clear walk position `c` and the blocked walk
    // position `b`, to the CAD rounding, on the clear side.
    let limit = |mut c: f64, mut b: f64| {
        while (b - c).abs() > CAD_ROUNDING_MM {
            let m = 0.5 * (b + c);
            if clear_walk(m) {
                c = m;
            } else {
                b = m;
            }
        }
        c
    };
    // The clear intervals along the walk. Each end that meets a blocked
    // stretch moves the CAD rounding away from it (the module doc).
    let mut intervals: Vec<(f64, f64)> = Vec::new();
    let mut open_at: Option<f64> = walk.first().and_then(|&(w, c)| c.then_some(w));
    for pair in walk.windows(2) {
        let &[(wp, cp), (wt, ct)] = pair else {
            continue;
        };
        match (cp, ct) {
            (false, true) => open_at = Some(limit(wt, wp) + CAD_ROUNDING_MM),
            (true, false) => {
                if let Some(w0) = open_at.take() {
                    intervals.push((w0, limit(wp, wt) - CAD_ROUNDING_MM));
                }
            }
            _ => {}
        }
    }
    if let Some(w0) = open_at {
        intervals.push((w0, total));
    }
    intervals.retain(|&(w0, w1)| w1 - w0 > CAD_ROUNDING_MM);
    let material = if material_left {
        EdgeMaterial::Left
    } else {
        EdgeMaterial::Right
    };
    let stops = stop_lengths(&intervals, total, closed);
    let runs = intervals
        .iter()
        .map(|&(w0, w1)| edge.sub_path(w0 + offset, w1 + offset))
        .filter(|points| points.len() >= 2)
        .map(|points| EdgeLine {
            line: Polygon2::open_path(points),
            z,
            material,
            item: line.item,
        })
        .collect();
    LineTrim { runs, stops }
}

/// The stop of each run end that meets a blocked stretch, from the clear
/// `intervals` along the walk (in order; `total` is the walk length):
///
/// - At an end of an open edge, the end gives up the whole stretch to the
///   edge end: the wall stands at the edge end.
/// - Between two runs (and on a closed edge), the two ends share the
///   stretch between them: each gives up half of it.
fn stop_lengths(intervals: &[(f64, f64)], total: f64, closed: bool) -> Vec<f64> {
    let mut stops = Vec::new();
    for (i, &(w0, w1)) in intervals.iter().enumerate() {
        let prev_end = match i.checked_sub(1).and_then(|j| intervals.get(j)) {
            Some(&(_, e)) => Some(e),
            None if closed => intervals.last().map(|&(_, e)| e - total),
            None => None,
        };
        let next_start = match intervals.get(i + 1) {
            Some(&(s, _)) => Some(s),
            None if closed => intervals.first().map(|&(s, _)| s + total),
            None => None,
        };
        match prev_end {
            Some(e) => stops.push(0.5 * (w0 - e)),
            None if w0 > 0.0 => stops.push(w0),
            None => {}
        }
        match next_start {
            Some(s) => stops.push(0.5 * (s - w1)),
            None if w1 < total => stops.push(total - w1),
            None => {}
        }
    }
    stops
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        clippy::indexing_slicing,
        reason = "test code: a panic is how a test reports a failure"
    )]

    use super::*;
    use crate::geo::P3;
    use crate::tool::VBitEndmill;

    /// A closed axis-aligned box: 8 vertices, 12 triangles, with its
    /// vertex numbers from `base`.
    fn push_box(lo: P3, hi: P3, vertices: &mut Vec<P3>, triangles: &mut Vec<[u32; 3]>) {
        let base = u32::try_from(vertices.len()).unwrap();
        for (x, y, z) in [
            (false, false, false),
            (true, false, false),
            (true, true, false),
            (false, true, false),
            (false, false, true),
            (true, false, true),
            (true, true, true),
            (false, true, true),
        ] {
            vertices.push(P3::new(
                if x { hi.x } else { lo.x },
                if y { hi.y } else { lo.y },
                if z { hi.z } else { lo.z },
            ));
        }
        for t in [
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ] {
            triangles.push([base + t[0], base + t[1], base + t[2]]);
        }
    }

    /// A closed rim that meets a wall along part of its length becomes one
    /// open run (point 3 of the brief): a plate X 0..40, Y 0..20 to Z 10,
    /// with a block X 0..10 standing to Z 20 on it. The rim of the Z 10
    /// face (X 10..40) runs along the block wall at X 10, so the run keeps
    /// the rest of the rim and stops `sqrt(R^2 - t^2)` plus the CAD
    /// rounding from the wall on both long sides.
    #[test]
    fn a_closed_rim_at_a_wall_becomes_one_open_run_chamferclear() {
        let (mut vertices, mut triangles) = (Vec::new(), Vec::new());
        push_box(
            P3::new(0.0, 0.0, 0.0),
            P3::new(40.0, 20.0, 10.0),
            &mut vertices,
            &mut triangles,
        );
        push_box(
            P3::new(0.0, 0.0, 10.0),
            P3::new(10.0, 20.0, 20.0),
            &mut vertices,
            &mut triangles,
        );
        let mesh = TriangleMesh::from_raw(vertices, triangles);
        let index = SpatialIndex::build_auto(&mesh);
        let cutter = VBitEndmill::new(12.0, 90.0, 10.0).unwrap();
        let (r, t) = (6.0_f64, 0.1_f64);
        let probe = EdgeClearProbe {
            mesh: &mesh,
            index: &index,
            cutter: &cutter,
            half_angle_rad: 45.0_f64.to_radians(),
            chamfer_width: 1.0,
            tip_offset: t,
        };
        let ring = EdgeLine {
            line: Polygon2::new(vec![
                P2::new(10.0, 0.0),
                P2::new(40.0, 0.0),
                P2::new(40.0, 20.0),
                P2::new(10.0, 20.0),
            ]),
            z: 10.0,
            material: EdgeMaterial::Inside,
            item: 0,
        };
        let trim = trim_line(&ring, &probe);
        assert_eq!(trim.runs.len(), 1, "{:?}", trim.runs);
        let run = &trim.runs[0];
        assert!(!run.line.closed);
        assert_eq!(run.material, EdgeMaterial::Left, "left of a CCW ring");
        let pts = &run.line.exterior;
        let (first, last) = (pts[0], *pts.last().unwrap());
        let clearance = (r * r - t * t).sqrt();
        for (end, y) in [(first, 0.0), (last, 20.0)] {
            assert!((end.y - y).abs() < 1e-9, "{end:?}");
            let short = end.x - 10.0;
            assert!(
                short >= clearance + CAD_ROUNDING_MM - 1e-6
                    && short <= clearance + 3.0 * CAD_ROUNDING_MM,
                "{short} against {clearance}"
            );
        }
        assert!(pts.contains(&P2::new(40.0, 0.0)) && pts.contains(&P2::new(40.0, 20.0)));
        assert_eq!(trim.stops.len(), 2);

        // A rim with no wall is not trimmed: the plate X 10..40 alone.
        let (mut vertices, mut triangles) = (Vec::new(), Vec::new());
        push_box(
            P3::new(10.0, 0.0, 0.0),
            P3::new(40.0, 20.0, 10.0),
            &mut vertices,
            &mut triangles,
        );
        let plate = TriangleMesh::from_raw(vertices, triangles);
        let plate_index = SpatialIndex::build_auto(&plate);
        let free = EdgeClearProbe {
            mesh: &plate,
            index: &plate_index,
            ..probe
        };
        let trim = trim_line(&ring, &free);
        assert_eq!(trim.runs.len(), 1);
        assert!(trim.runs[0].line.closed, "a clear rim stays closed");
        assert!(trim.stops.is_empty());
    }
}
