//! G-BOUNDARYPHANTOM — the planner keeps the tool centre inside the
//! containment boundary that the post-generation clip enforces.
//!
//! The session clips every 3D Rough to the machining boundary AFTER
//! generation (`ProjectSession::apply_boundary_clip*`): the region inset by
//! the tool radius for `Inside` containment, outset for `Outside`. Before
//! this module the planner planned and STAMPED cuts whose tool centre lay
//! between the region edge and that containment line. The clip then deleted
//! them, but their stamps stayed in the planner stock. A later entry read
//! that phantom stock as its rapid floor and rapided into material the
//! shipped path never cut (rivmap350 "3D Rough 8": eight rapids 4 to 11 mm
//! into stock along the containment line).
//!
//! [`CentreClip`] holds the clip's own polygons, shrunk by
//! [`EDGE_MARGIN_MM`], and splits each planned segment the same way the
//! clip will. A piece outside is never pushed and never stamped. A piece
//! that starts away from the tool gets its own planned entry, so its rapid
//! floor reads the planner stock. The shipped path then stays inside the
//! clip polygons and the clip removes nothing that the planner stamped.

use crate::geo::{P2, P3};
use crate::geometry::boundary::maximal_inside_intervals;
use crate::polygon::Polygon2;

use super::clearing::PlannerCursor;
use super::path::Adaptive3dSegment;

/// The planner keeps a tool centre this far inside the clip polygons.
///
/// The clip classifies a point that lies ON its polygon edge by the ray
/// crossing rule, and a cut end that the planner placed exactly on the edge
/// can fall either way after the dressups re-segment it. Inside by a
/// hundredth of a millimetre, every end the planner emits is inside the
/// clip polygon without doubt.
pub(super) const EDGE_MARGIN_MM: f64 = 0.01;

/// A piece shorter than this in XY is not cut (it cuts no material the
/// entry descent before it does not cut).
const MIN_PIECE_MM: f64 = 1e-6;

/// Cells the accelerator marks around each boundary sample, in each axis.
/// An unmarked cell is then at least `(NEAR_CELLS - 0.25)` cells from every
/// edge (samples every half cell), so a chord shorter than
/// [`FAST_CHORD_CELLS`] between two unmarked cells cannot cross an edge.
const NEAR_CELLS: isize = 2;
const FAST_CHORD_CELLS: f64 = 3.0;

/// The containment polygons of the post-generation boundary clip, as the
/// planner applies them.
pub(super) struct CentreClip {
    polygons: Vec<Polygon2>,
    origin_x: f64,
    origin_y: f64,
    cell: f64,
    rows: usize,
    cols: usize,
    /// Per cell: `NEAR` (within `NEAR_CELLS` of an edge sample), else
    /// `INSIDE` or `OUTSIDE`, decided once per connected run of far cells.
    class: Vec<u8>,
}

const NEAR: u8 = 0;
const INSIDE: u8 = 1;
const OUTSIDE: u8 = 2;
const UNSET: u8 = 3;

impl CentreClip {
    /// The clip for `boundaries` (the clip's containment polygons) over the
    /// planner grid. `None` when there is nothing to clip to: the session
    /// clip passes an empty set through unclipped, and so does the planner.
    pub(super) fn new(
        boundaries: &[Polygon2],
        origin_x: f64,
        origin_y: f64,
        cell: f64,
        rows: usize,
        cols: usize,
    ) -> Option<Self> {
        if boundaries.is_empty() || !cell.is_finite() || cell <= 0.0 || rows == 0 || cols == 0 {
            return None;
        }
        let mut polygons: Vec<Polygon2> = Vec::new();
        for b in boundaries {
            let shrunk = crate::polygon::offset_polygon(b, EDGE_MARGIN_MM);
            // A polygon thinner than the margin collapses. It is still
            // clipped by the session; the planner then keeps none of it,
            // which can only leave the planner stock high (safe).
            polygons.extend(shrunk);
        }
        let mut clip = Self {
            polygons,
            origin_x,
            origin_y,
            cell,
            rows,
            cols,
            class: vec![UNSET; rows * cols],
        };
        clip.classify();
        Some(clip)
    }

    fn cell_of(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        let c = ((x - self.origin_x) / self.cell).floor();
        let r = ((y - self.origin_y) / self.cell).floor();
        if !(c.is_finite() && r.is_finite()) || c < 0.0 || r < 0.0 {
            return None;
        }
        let (r, c) = (r as usize, c as usize);
        (r < self.rows && c < self.cols).then_some((r, c))
    }

    fn exact_contains(&self, x: f64, y: f64) -> bool {
        let p = P2::new(x, y);
        self.polygons.iter().any(|b| b.contains_point(&p))
    }

    /// Mark the cells near an edge, then give every other cell the side of
    /// its connected run, read once at the run's first cell.
    #[allow(clippy::indexing_slicing)] // SAFETY: indices bounded by rows/cols checks
    fn classify(&mut self) {
        let step = self.cell * 0.5;
        for poly in &self.polygons {
            for ring in std::iter::once(&poly.exterior).chain(poly.holes.iter()) {
                let n = ring.len();
                for i in 0..n {
                    let a = ring[i];
                    let b = ring[(i + 1) % n];
                    let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
                    let samples = (len / step).ceil().max(1.0) as usize;
                    for k in 0..=samples {
                        let t = k as f64 / samples as f64;
                        let x = a.x + t * (b.x - a.x);
                        let y = a.y + t * (b.y - a.y);
                        let c = ((x - self.origin_x) / self.cell).floor() as isize;
                        let r = ((y - self.origin_y) / self.cell).floor() as isize;
                        for dr in -NEAR_CELLS..=NEAR_CELLS {
                            for dc in -NEAR_CELLS..=NEAR_CELLS {
                                let (rr, cc) = (r + dr, c + dc);
                                if rr >= 0
                                    && cc >= 0
                                    && (rr as usize) < self.rows
                                    && (cc as usize) < self.cols
                                {
                                    self.class[rr as usize * self.cols + cc as usize] = NEAR;
                                }
                            }
                        }
                    }
                }
            }
        }
        let mut stack: Vec<usize> = Vec::new();
        for seed in 0..self.class.len() {
            if self.class[seed] != UNSET {
                continue;
            }
            let (r, c) = (seed / self.cols, seed % self.cols);
            let x = self.origin_x + (c as f64 + 0.5) * self.cell;
            let y = self.origin_y + (r as f64 + 0.5) * self.cell;
            let side = if self.exact_contains(x, y) {
                INSIDE
            } else {
                OUTSIDE
            };
            self.class[seed] = side;
            stack.push(seed);
            while let Some(i) = stack.pop() {
                let (r, c) = (i / self.cols, i % self.cols);
                let mut visit = |rr: usize, cc: usize| {
                    let j = rr * self.cols + cc;
                    if self.class[j] == UNSET {
                        self.class[j] = side;
                        stack.push(j);
                    }
                };
                if r > 0 {
                    visit(r - 1, c);
                }
                if r + 1 < self.rows {
                    visit(r + 1, c);
                }
                if c > 0 {
                    visit(r, c - 1);
                }
                if c + 1 < self.cols {
                    visit(r, c + 1);
                }
            }
        }
    }

    fn far_class(&self, x: f64, y: f64) -> Option<u8> {
        let (r, c) = self.cell_of(x, y)?;
        let class = *self.class.get(r * self.cols + c)?;
        (class == INSIDE || class == OUTSIDE).then_some(class)
    }

    /// Whether the planner may put the tool centre at `(x, y)`.
    pub(super) fn contains(&self, x: f64, y: f64) -> bool {
        match self.far_class(x, y) {
            Some(class) => class == INSIDE,
            None => self.exact_contains(x, y),
        }
    }

    /// The parameter intervals of the chord `a`→`b` that lie inside, as the
    /// session clip computes them.
    fn intervals(&self, a: P3, b: P3) -> Vec<(f64, f64)> {
        let len = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
        if len < FAST_CHORD_CELLS * self.cell
            && let (Some(ca), Some(cb)) = (self.far_class(a.x, a.y), self.far_class(b.x, b.y))
            && ca == cb
        {
            return if ca == INSIDE {
                vec![(0.0, 1.0)]
            } else {
                Vec::new()
            };
        }
        maximal_inside_intervals(a, b, &self.polygons)
    }

    /// Whether the whole chord `a`→`b` lies inside.
    fn chord_inside(&self, a: P3, b: P3) -> bool {
        matches!(self.intervals(a, b).as_slice(), [(t0, t1)] if *t0 <= 1e-9 && *t1 >= 1.0 - 1e-9)
    }

    /// The inside pieces of `path`, in order. The flags say whether the
    /// first piece starts at `path[0]` and whether the last ends at the
    /// path's end.
    fn pieces(&self, path: &[P3]) -> (Vec<Vec<P3>>, bool, bool) {
        let mut pieces: Vec<Vec<P3>> = Vec::new();
        let mut current: Vec<P3> = Vec::new();
        let mut starts_at_start = false;
        let mut ends_at_end = false;
        for (k, pair) in path.windows(2).enumerate() {
            let [a, b] = pair else { continue };
            let intervals = self.intervals(*a, *b);
            if intervals.is_empty() {
                close_piece(&mut current, &mut pieces);
                ends_at_end = false;
                continue;
            }
            for &(t0, t1) in &intervals {
                if t0 > 1e-9 {
                    close_piece(&mut current, &mut pieces);
                    current.push(lerp(*a, *b, t0));
                } else if current.is_empty() {
                    if k == 0 {
                        starts_at_start = true;
                    }
                    current.push(*a);
                }
                if t1 >= 1.0 - 1e-9 {
                    current.push(*b);
                    ends_at_end = true;
                } else {
                    current.push(lerp(*a, *b, t1));
                    close_piece(&mut current, &mut pieces);
                    ends_at_end = false;
                }
            }
        }
        close_piece(&mut current, &mut pieces);
        if pieces.is_empty() {
            return (pieces, false, false);
        }
        // A dropped degenerate first piece does not start the path.
        let first_is_start = starts_at_start
            && pieces
                .first()
                .and_then(|p| p.first())
                .zip(path.first())
                .is_some_and(|(p, q)| (p.x - q.x).abs() < 1e-12 && (p.y - q.y).abs() < 1e-12);
        let last_is_end = ends_at_end
            && pieces
                .last()
                .and_then(|p| p.last())
                .zip(path.last())
                .is_some_and(|(p, q)| (p.x - q.x).abs() < 1e-12 && (p.y - q.y).abs() < 1e-12);
        (pieces, first_is_start, last_is_end)
    }
}

fn lerp(a: P3, b: P3, t: f64) -> P3 {
    P3::new(
        a.x + t * (b.x - a.x),
        a.y + t * (b.y - a.y),
        a.z + t * (b.z - a.z),
    )
}

/// Close the open piece. A piece of fewer than two points or no XY length
/// is dropped.
fn close_piece(current: &mut Vec<P3>, pieces: &mut Vec<Vec<P3>>) {
    let piece = std::mem::take(current);
    if piece.len() < 2 {
        return;
    }
    let len: f64 = piece
        .windows(2)
        .map(|w| match w {
            [a, b] => ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt(),
            _ => 0.0,
        })
        .sum();
    if len >= MIN_PIECE_MM {
        pieces.push(piece);
    }
}

/// Split one planned segment into the segments the planner may push.
///
/// - An entry (rapid) outside is dropped.
/// - A link stays a link when its whole chord from the planner position is
///   inside; else it becomes an entry at its target, or is dropped when the
///   target is outside.
/// - A cut keeps its inside pieces. A piece that does not continue from the
///   tool (it starts at a boundary crossing, or the segment before it was
///   dropped) gets an entry at its first point.
///
/// `cursor.clip_gap` records that something was dropped since the last
/// pushed segment, so the tool is not where the next segment starts.
pub(super) fn clip_segment(
    clip: &CentreClip,
    cursor: &mut PlannerCursor,
    segment: Adaptive3dSegment,
) -> Vec<Adaptive3dSegment> {
    match segment {
        Adaptive3dSegment::Marker(_) => vec![segment],
        Adaptive3dSegment::Rapid(entry) | Adaptive3dSegment::RapidWithFloor { entry, .. } => {
            if clip.contains(entry.x, entry.y) {
                cursor.clip_gap = false;
                vec![segment]
            } else {
                cursor.clip_gap = true;
                Vec::new()
            }
        }
        Adaptive3dSegment::Link(target) => {
            let linked = !cursor.clip_gap
                && cursor
                    .last_pos
                    .is_some_and(|from| clip.chord_inside(from, target));
            if linked {
                vec![segment]
            } else if clip.contains(target.x, target.y) {
                cursor.clip_gap = false;
                vec![Adaptive3dSegment::Rapid(target)]
            } else {
                cursor.clip_gap = true;
                Vec::new()
            }
        }
        Adaptive3dSegment::Cut(ref path) if path.len() < 2 => vec![segment],
        Adaptive3dSegment::Cut(path) => {
            let (pieces, first_is_start, last_is_end) = clip.pieces(&path);
            if pieces.is_empty() {
                cursor.clip_gap = true;
                return Vec::new();
            }
            let continues = first_is_start && !cursor.clip_gap;
            if continues && last_is_end && pieces.len() == 1 {
                return vec![Adaptive3dSegment::Cut(path)];
            }
            let mut out: Vec<Adaptive3dSegment> = Vec::with_capacity(pieces.len() * 2);
            for (i, piece) in pieces.into_iter().enumerate() {
                if (i > 0 || !continues)
                    && let Some(first) = piece.first()
                {
                    out.push(Adaptive3dSegment::Rapid(*first));
                }
                out.push(Adaptive3dSegment::Cut(piece));
            }
            cursor.clip_gap = !last_is_end;
            out
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )]

    use super::*;

    fn square_clip() -> CentreClip {
        // Containment 10..90, planner grid 0..100 at 0.5 mm.
        CentreClip::new(
            &[Polygon2::rectangle(10.0, 10.0, 90.0, 90.0)],
            0.0,
            0.0,
            0.5,
            200,
            200,
        )
        .unwrap()
    }

    #[test]
    fn the_accelerated_test_agrees_with_the_exact_one() {
        let clip = square_clip();
        for i in 0..400 {
            for j in 0..400 {
                let (x, y) = (i as f64 * 0.25 + 0.01, j as f64 * 0.25 + 0.03);
                assert_eq!(clip.contains(x, y), clip.exact_contains(x, y), "({x}, {y})");
            }
        }
    }

    #[test]
    fn a_cut_across_the_edge_keeps_only_the_inside_piece_with_an_entry() {
        let clip = square_clip();
        let mut cursor = PlannerCursor::default();
        let path = vec![
            P3::new(5.0, 50.0, 1.0),
            P3::new(20.0, 50.0, 1.0),
            P3::new(30.0, 50.0, 1.0),
        ];
        let out = clip_segment(&clip, &mut cursor, Adaptive3dSegment::Cut(path));
        assert_eq!(out.len(), 2);
        let Adaptive3dSegment::Rapid(entry) = out[0] else {
            panic!("an entry first");
        };
        assert!(
            (entry.x - (10.0 + EDGE_MARGIN_MM)).abs() < 1e-6,
            "{entry:?}"
        );
        let Adaptive3dSegment::Cut(ref piece) = out[1] else {
            panic!("then the cut");
        };
        assert!((piece.last().unwrap().x - 30.0).abs() < 1e-12);
        assert!(!cursor.clip_gap);
    }

    #[test]
    fn an_entry_outside_is_dropped_and_the_next_cut_gets_its_own_entry() {
        let clip = square_clip();
        let mut cursor = PlannerCursor::default();
        assert!(
            clip_segment(
                &clip,
                &mut cursor,
                Adaptive3dSegment::Rapid(P3::new(5.0, 5.0, 0.0))
            )
            .is_empty()
        );
        assert!(cursor.clip_gap);
        let path = vec![P3::new(20.0, 20.0, 0.0), P3::new(30.0, 20.0, 0.0)];
        let out = clip_segment(&clip, &mut cursor, Adaptive3dSegment::Cut(path));
        assert!(matches!(
            out.as_slice(),
            [Adaptive3dSegment::Rapid(_), Adaptive3dSegment::Cut(_)]
        ));
    }

    #[test]
    fn a_cut_wholly_inside_passes_unchanged() {
        let clip = square_clip();
        let mut cursor = PlannerCursor::default();
        let path = vec![P3::new(20.0, 20.0, 0.0), P3::new(80.0, 80.0, 0.0)];
        let out = clip_segment(&clip, &mut cursor, Adaptive3dSegment::Cut(path.clone()));
        assert!(matches!(out.as_slice(), [Adaptive3dSegment::Cut(p)] if *p == path));
    }
}
