//! The walls of a pin (P6, 2026-10-12).
//!
//! A dowel hole needs solid material around it: a wall to the stock edge,
//! a wall to the part, and a wall to each keep-out zone and fixture. This
//! module measures those walls for a stored set ([`pin_walls`],
//! [`pin_wall_findings`]) and gives the placer the clear runs of a line
//! ([`clear_runs`]).
//!
//! Every obstacle is a 2D outline in **stock-local** mm (X0Y0 at the stock
//! min corner), the frame of `StockConfig::alignment_pins`. The caller
//! converts from the world frame (`StockConfig` subtracts its origin).
//!
//! A wall is the distance from the pin's edge to the nearest point of a
//! target. A negative wall means that the pin cuts into the target.

use crate::polygon::Polygon2;

use super::{AlignmentPin, PIN_GEOM_EPS_MM, PIN_WALL_MM, PinFinding, PinFindingKind};

/// What an obstacle is, for the wall report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PinObstacleKind {
    /// The footprint of a model that the operator calls the part.
    Part,
    /// A keep-out zone of a setup.
    KeepOut,
    /// A workholding fixture of a setup (its footprint includes the
    /// fixture's own clearance).
    Fixture,
}

impl PinObstacleKind {
    fn label(self) -> &'static str {
        match self {
            PinObstacleKind::Part => "the part",
            PinObstacleKind::KeepOut => "the keep-out zone",
            PinObstacleKind::Fixture => "the fixture",
        }
    }
}

/// One 2D outline that a pin must keep a wall to.
///
/// A closed outline is a solid region: a pin inside it cuts into it. Its
/// holes are not read, so a pin in a hole of a part outline also counts as
/// inside the part (the safe reading). An open outline (a drawn line, for
/// example a river) is only its line.
#[derive(Debug, Clone, PartialEq)]
pub struct PinObstacle {
    pub kind: PinObstacleKind,
    /// The name the operator reads: the model, keep-out or fixture name.
    pub name: String,
    pub points: Vec<(f64, f64)>,
    pub closed: bool,
}

impl PinObstacle {
    /// The exterior of `polygon` (holes are not read).
    #[must_use]
    pub fn from_polygon(
        kind: PinObstacleKind,
        name: impl Into<String>,
        polygon: &Polygon2,
    ) -> Self {
        Self {
            kind,
            name: name.into(),
            points: polygon.exterior.iter().map(|p| (p.x, p.y)).collect(),
            closed: polygon.closed,
        }
    }

    /// An axis-aligned rectangle.
    #[must_use]
    pub fn rectangle(
        kind: PinObstacleKind,
        name: impl Into<String>,
        min: (f64, f64),
        max: (f64, f64),
    ) -> Self {
        Self {
            kind,
            name: name.into(),
            points: vec![min, (max.0, min.1), max, (min.0, max.1)],
            closed: true,
        }
    }

    /// The same obstacle moved by `(dx, dy)` mm.
    #[must_use]
    pub fn translated(&self, dx: f64, dy: f64) -> Self {
        Self {
            points: self.points.iter().map(|(x, y)| (x + dx, y + dy)).collect(),
            ..self.clone()
        }
    }

    /// The edges of the outline. A closed outline also has the edge from
    /// the last point back to the first.
    fn edges(&self) -> impl Iterator<Item = ((f64, f64), (f64, f64))> + '_ {
        let n = self.points.len();
        let count = match (self.closed, n) {
            (_, 0) => 0,
            (_, 1) => 1,
            (true, _) => n,
            (false, _) => n - 1,
        };
        (0..count).filter_map(move |i| {
            let a = *self.points.get(i)?;
            let b = *self.points.get((i + 1) % n)?;
            Some((a, b))
        })
    }

    /// Even-odd containment. An open outline contains nothing.
    fn contains(&self, p: (f64, f64)) -> bool {
        if !self.closed || self.points.len() < 3 {
            return false;
        }
        let mut inside = false;
        for (a, b) in self.edges() {
            if (a.1 > p.1) != (b.1 > p.1) {
                let x = a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0);
                if p.0 < x {
                    inside = !inside;
                }
            }
        }
        inside
    }

    /// The signed distance from `p` to the obstacle: negative inside a
    /// closed outline.
    #[must_use]
    pub fn signed_distance(&self, p: (f64, f64)) -> f64 {
        let d = self
            .edges()
            .map(|(a, b)| point_segment_distance(p, a, b))
            .fold(f64::INFINITY, f64::min);
        if self.contains(p) { -d } else { d }
    }

    /// The parameters `t` at which the line `o + t u` is closer than
    /// `delta` to the obstacle or inside it, as intervals (not merged).
    fn blocked_on_line(&self, o: (f64, f64), u: (f64, f64), delta: f64, out: &mut Vec<(f64, f64)>) {
        // The signed distance of a point from the line. An edge whose two
        // ends are farther than `delta` on one side cannot block the line
        // and does not cross it, so most edges cost two dot products.
        let nrm = (-u.1, u.0);
        let side = |p: (f64, f64)| dot(sub(p, o), nrm);
        let near = |a: (f64, f64), b: (f64, f64)| {
            let (sa, sb) = (side(a), side(b));
            !((sa > delta && sb > delta) || (sa < -delta && sb < -delta))
        };
        for (a, b) in self.edges().filter(|&(a, b)| near(a, b)) {
            if let Some(iv) = capsule_on_line(o, u, a, b, delta) {
                out.push(iv);
            }
        }
        if !self.closed || self.points.len() < 3 {
            return;
        }
        // The line inside the outline: the edge crossings, paired even-odd.
        let mut ts: Vec<f64> = self
            .edges()
            .filter(|&(a, b)| near(a, b))
            .filter_map(|(a, b)| line_segment_crossing(o, u, a, b))
            .collect();
        ts.sort_by(f64::total_cmp);
        for [t0, t1] in ts.as_chunks::<2>().0 {
            out.push((*t0, *t1));
        }
    }
}

fn dot(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.0 + a.1 * b.1
}

fn sub(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 - b.0, a.1 - b.1)
}

fn point_segment_distance(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let ab = sub(b, a);
    let len2 = dot(ab, ab);
    let s = if len2 > 0.0 {
        (dot(sub(p, a), ab) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let q = (a.0 + s * ab.0, a.1 + s * ab.1);
    (p.0 - q.0).hypot(p.1 - q.1)
}

/// The `t` range where `lo < c0 + c1 t < hi`, or `None`.
fn linear_band(c0: f64, c1: f64, lo: f64, hi: f64) -> Option<(f64, f64)> {
    if c1.abs() <= f64::EPSILON {
        return (c0 > lo && c0 < hi).then_some((f64::NEG_INFINITY, f64::INFINITY));
    }
    let (t0, t1) = ((lo - c0) / c1, (hi - c0) / c1);
    Some((t0.min(t1), t0.max(t1)))
}

fn intersect(a: (f64, f64), b: (f64, f64)) -> Option<(f64, f64)> {
    let iv = (a.0.max(b.0), a.1.min(b.1));
    (iv.0 < iv.1).then_some(iv)
}

/// The `t` range where the unit-speed line `o + t u` is closer than
/// `delta` to the segment `ab`. The capsule is convex, so the range is
/// one interval: the hull of the two end discs and the side band.
fn capsule_on_line(
    o: (f64, f64),
    u: (f64, f64),
    a: (f64, f64),
    b: (f64, f64),
    delta: f64,
) -> Option<(f64, f64)> {
    let disc = |c: (f64, f64)| {
        // |o + t u - c|^2 < delta^2, with |u| = 1.
        let w = sub(o, c);
        let half_b = dot(w, u);
        let disc = half_b * half_b - (dot(w, w) - delta * delta);
        (disc > 0.0).then(|| (-half_b - disc.sqrt(), -half_b + disc.sqrt()))
    };
    let ab = sub(b, a);
    let len = dot(ab, ab).sqrt();
    let band = (len > 0.0)
        .then(|| {
            let e = (ab.0 / len, ab.1 / len);
            let nrm = (-e.1, e.0);
            let w = sub(o, a);
            let along = linear_band(dot(w, e), dot(u, e), 0.0, len)?;
            let across = linear_band(dot(w, nrm), dot(u, nrm), -delta, delta)?;
            intersect(along, across)
        })
        .flatten();
    [disc(a), disc(b), band]
        .into_iter()
        .flatten()
        .reduce(|x, y| (x.0.min(y.0), x.1.max(y.1)))
}

/// The `t` at which the unit-speed line `o + t u` crosses the segment
/// `ab`, half-open at `b` so a vertex on the line counts once.
fn line_segment_crossing(
    o: (f64, f64),
    u: (f64, f64),
    a: (f64, f64),
    b: (f64, f64),
) -> Option<f64> {
    let nrm = (-u.1, u.0);
    let sa = dot(sub(a, o), nrm);
    let sb = dot(sub(b, o), nrm);
    if (sa > 0.0) == (sb > 0.0) {
        return None;
    }
    let s = sa / (sa - sb);
    let p = (a.0 + s * (b.0 - a.0), a.1 + s * (b.1 - a.1));
    Some(dot(sub(p, o), u))
}

/// The clear runs of a line: the parameters `t` of the unit-speed line
/// `o + t u` at which a pin of radius `radius` has a wall of at least
/// `wall_mm` to every stock edge and every obstacle.
///
/// The runs are closed and sorted. A run can be a single point (a pin
/// that fits exactly). The result is exact for the outlines as given; it
/// does not sample.
pub(super) fn clear_runs(
    stock_w: f64,
    stock_d: f64,
    obstacles: &[PinObstacle],
    o: (f64, f64),
    u: (f64, f64),
    radius: f64,
    wall_mm: f64,
) -> Vec<(f64, f64)> {
    let delta = radius + wall_mm;
    // The stock inset by `delta`, as slabs in x and in y.
    let Some(x_band) = closed_band(o.0, u.0, delta, stock_w - delta) else {
        return Vec::new();
    };
    let Some(y_band) = closed_band(o.1, u.1, delta, stock_d - delta) else {
        return Vec::new();
    };
    let inside = (x_band.0.max(y_band.0), x_band.1.min(y_band.1));
    if inside.0 > inside.1 + PIN_GEOM_EPS_MM {
        return Vec::new();
    }

    let mut blocked = Vec::new();
    for obstacle in obstacles {
        obstacle.blocked_on_line(o, u, delta, &mut blocked);
    }
    // A blocked range is open: a pin at exactly `delta` from an obstacle
    // has exactly the wall it needs. Shrink each range by the slop so a
    // run of one point survives the rounding.
    let mut blocked: Vec<(f64, f64)> = blocked
        .into_iter()
        .map(|(t0, t1)| (t0 + PIN_GEOM_EPS_MM, t1 - PIN_GEOM_EPS_MM))
        .filter(|(t0, t1)| t0 < t1)
        .collect();
    blocked.sort_by(|a, b| a.0.total_cmp(&b.0));

    let mut runs = Vec::new();
    let mut start = inside.0;
    for (t0, t1) in blocked {
        if t1 < start {
            continue;
        }
        if t0 > inside.1 {
            break;
        }
        if t0 >= start {
            runs.push((start, t0));
        }
        start = start.max(t1);
    }
    if start <= inside.1 {
        runs.push((start, inside.1));
    }
    runs
}

/// The `t` range where `lo <= c0 + c1 t <= hi`, or `None`.
fn closed_band(c0: f64, c1: f64, lo: f64, hi: f64) -> Option<(f64, f64)> {
    if lo > hi + PIN_GEOM_EPS_MM {
        return None;
    }
    if c1.abs() <= f64::EPSILON {
        return (c0 >= lo - PIN_GEOM_EPS_MM && c0 <= hi + PIN_GEOM_EPS_MM)
            .then_some((f64::NEG_INFINITY, f64::INFINITY));
    }
    let (t0, t1) = ((lo - c0) / c1, (hi - c0) / c1);
    Some((t0.min(t1), t0.max(t1)))
}

/// The wall from a pin to one named target.
#[derive(Debug, Clone, PartialEq)]
pub struct NamedWall {
    pub kind: PinObstacleKind,
    pub name: String,
    pub wall_mm: f64,
}

/// The walls of one pin: to the stock edge, to the part, and to the
/// keep-outs and fixtures. A `None` wall means that no such target exists.
#[derive(Debug, Clone, PartialEq)]
pub struct PinWalls {
    /// The smallest wall to the four stock edges. Negative when the pin
    /// hangs off the stock.
    pub stock_edge_mm: f64,
    /// The smallest wall to a part footprint.
    pub part: Option<NamedWall>,
    /// The smallest wall to a keep-out zone or a fixture.
    pub keep_out: Option<NamedWall>,
}

impl PinWalls {
    /// The smallest of the walls.
    #[must_use]
    pub fn min_mm(&self) -> f64 {
        [&self.part, &self.keep_out]
            .into_iter()
            .flatten()
            .map(|w| w.wall_mm)
            .fold(self.stock_edge_mm, f64::min)
    }
}

/// The walls of `pin` on a `stock_w` x `stock_d` stock with `obstacles`
/// (stock-local).
#[must_use]
pub fn pin_walls(
    pin: &AlignmentPin,
    stock_w: f64,
    stock_d: f64,
    obstacles: &[PinObstacle],
) -> PinWalls {
    let r = 0.5 * pin.diameter;
    let p = (pin.x, pin.y);
    let stock_edge_mm = [pin.x, stock_w - pin.x, pin.y, stock_d - pin.y]
        .into_iter()
        .fold(f64::INFINITY, f64::min)
        - r;
    let nearest = |wanted: &dyn Fn(PinObstacleKind) -> bool| {
        obstacles
            .iter()
            .filter(|o| wanted(o.kind))
            .map(|o| NamedWall {
                kind: o.kind,
                name: o.name.clone(),
                wall_mm: o.signed_distance(p) - r,
            })
            .min_by(|a, b| a.wall_mm.total_cmp(&b.wall_mm))
    };
    PinWalls {
        stock_edge_mm,
        part: nearest(&|k| k == PinObstacleKind::Part),
        keep_out: nearest(&|k| k != PinObstacleKind::Part),
    }
}

/// One finding for each pin wall that is thinner than [`PIN_WALL_MM`].
///
/// The pins are numbered from 1, in stored order. A pin that hangs off the
/// stock gets no edge finding here: `PinFindingKind::OutOfBounds` states
/// it already.
#[must_use]
pub fn pin_wall_findings(
    pins: &[AlignmentPin],
    stock_w: f64,
    stock_d: f64,
    obstacles: &[PinObstacle],
) -> Vec<PinFinding> {
    let mut out = Vec::new();
    for (i, pin) in pins.iter().enumerate() {
        let number = i + 1;
        let walls = pin_walls(pin, stock_w, stock_d, obstacles);
        let need = PIN_WALL_MM - PIN_GEOM_EPS_MM;
        if walls.stock_edge_mm >= 0.0 && walls.stock_edge_mm < need {
            out.push(PinFinding {
                kind: PinFindingKind::ThinWall,
                text: format!(
                    "Alignment pin {number} leaves a wall of {:.1} mm to the stock edge, and \
                     {PIN_WALL_MM:.1} mm is needed.",
                    walls.stock_edge_mm
                ),
            });
        }
        for wall in [walls.part, walls.keep_out].into_iter().flatten() {
            if wall.wall_mm >= need {
                continue;
            }
            let target = format!("{} '{}'", wall.kind.label(), wall.name);
            let text = if wall.wall_mm < 0.0 {
                format!(
                    "Alignment pin {number} cuts into {target}: its wall is {:.1} mm, and \
                     {PIN_WALL_MM:.1} mm is needed.",
                    wall.wall_mm
                )
            } else {
                format!(
                    "Alignment pin {number} leaves a wall of {:.1} mm to {target}, and \
                     {PIN_WALL_MM:.1} mm is needed.",
                    wall.wall_mm
                )
            };
            out.push(PinFinding {
                kind: PinFindingKind::ThinWall,
                text,
            });
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> PinObstacle {
        PinObstacle::rectangle(PinObstacleKind::Part, "part", (x0, y0), (x1, y1))
    }

    #[test]
    fn the_runs_of_a_line_through_a_rim_are_the_two_bands() {
        // 65 x 65 stock, a 41 x 41 part centred, a 6 mm pin, a 2 mm wall:
        // the pin centre needs 5 mm to the edge and 5 mm to the part.
        let part = [square(12.0, 12.0, 53.0, 53.0)];
        let runs = clear_runs(65.0, 65.0, &part, (0.0, 32.5), (1.0, 0.0), 3.0, 2.0);
        assert_eq!(runs.len(), 2, "{runs:?}");
        assert!((runs[0].0 - 5.0).abs() < 1e-6 && (runs[0].1 - 7.0).abs() < 1e-6);
        assert!((runs[1].0 - 58.0).abs() < 1e-6 && (runs[1].1 - 60.0).abs() < 1e-6);
    }

    #[test]
    fn a_pin_that_fits_exactly_is_a_run_of_one_point() {
        let part = [square(20.0, 0.0, 120.0, 150.0)];
        let runs = clear_runs(140.0, 150.0, &part, (0.0, 75.0), (1.0, 0.0), 8.0, 2.0);
        assert_eq!(runs.len(), 2, "{runs:?}");
        assert!((runs[0].0 - 10.0).abs() < 1e-6 && (runs[0].1 - 10.0).abs() < 1e-6);
    }

    #[test]
    fn the_runs_agree_with_the_point_walls() {
        let obstacles = [
            square(12.0, 12.0, 53.0, 53.0),
            PinObstacle {
                kind: PinObstacleKind::KeepOut,
                name: "a line".into(),
                points: vec![(0.0, 0.0), (30.0, 9.0)],
                closed: false,
            },
        ];
        for y in [4.0, 6.0, 8.5, 32.5, 58.0] {
            let runs = clear_runs(65.0, 65.0, &obstacles, (0.0, y), (1.0, 0.0), 3.0, 2.0);
            for i in 0..=650 {
                let x = f64::from(i) * 0.1;
                let pin = AlignmentPin::new(x, y, 6.0);
                let clear = pin_walls(&pin, 65.0, 65.0, &obstacles).min_mm() >= 2.0 - 1e-6;
                let in_run = runs.iter().any(|r| x >= r.0 - 1e-6 && x <= r.1 + 1e-6);
                assert_eq!(clear, in_run, "y {y}, x {x}: runs {runs:?}");
            }
        }
    }

    #[test]
    fn a_pin_inside_the_part_has_a_negative_wall() {
        let part = [square(12.0, 12.0, 53.0, 53.0)];
        let walls = pin_walls(&AlignmentPin::new(20.0, 32.5, 6.0), 65.0, 65.0, &part);
        let wall = walls.part.unwrap().wall_mm;
        assert!((wall - (-11.0)).abs() < 1e-9, "{wall}");
        assert!(walls.keep_out.is_none());
    }
}
