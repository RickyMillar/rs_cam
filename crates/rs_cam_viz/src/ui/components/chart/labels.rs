//! The label kit of the chart family: one label per line or mark, placed
//! along the line and never across another drawn line.
//!
//! P17 moved the kit here from `ui/feeds/explore.rs` (W3k). The nomogram,
//! the envelope plot and [`super::PlaneChart`] share it; the histogram reads
//! [`place_label`] for its axis labels. A module of the kit imports nothing
//! from `ui/feeds`.
//!
//! # What the kit does
//!
//! - [`place_labels`] sizes each [`ChartLabel`] and lays the set out with
//!   [`layout_labels`]: a label takes the first place along its line (or
//!   beside its mark) that is inside the frame, clear of the `keep_clear`
//!   rects, clear of every [`DrawnLine`] and clear of the labels placed
//!   before it. At most [`MAX_LABELS`] stay; the least important go first.
//! - [`paint_labels`] paints a background chip, the text at the angle of
//!   its line, and a leader to a mark the layout moved the label off.
//! - [`place_label`] paints one axis label (the histogram's rule): it moves
//!   a colliding label by its [`Nudge`] and leaves it out when it still
//!   collides or leaves the bounds.
//!
//! # What the kit does NOT do
//!
//! It does not pick a colour: a label takes the colour its caller gives
//! it, the colour of the line it names.

use crate::ui::tokens;

/// A token at a given alpha. The hue stays a token; only the alpha is
/// written at the call site.
fn wash(base: egui::Color32, alpha: u8) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), alpha)
}

/// The gap between a label and its line or mark, in points.
pub(crate) const LABEL_GAP: f32 = 3.0;

/// The padding of a label's background, in points.
pub(crate) const LABEL_PADDING: egui::Vec2 = egui::Vec2::new(2.0, 1.0);

/// The alpha of a label's background. The background is the overlay
/// surface, one step lighter than the plot ground, so a label reads as a
/// chip in front of the grid (lead, 2026-10-31: the sunken background was
/// the plot's own colour and did not show).
const LABEL_BACKGROUND_ALPHA: u8 = 235;

/// The alpha of a leader line from a moved mark label to its mark.
const LEADER_ALPHA: u8 = 170;

/// How far from its mark a mark label may go, in points, when the places
/// beside the mark are taken (a ring between lines in a narrow plot).
const MARK_SCAN: f32 = 48.0;

/// The most labels the chart paints (lead, 2026-10-31: "at most ~6"; seven
/// leave room for "Vendor range" beside the six of a chart with a "Now"
/// dot). The least important labels go first.
pub(crate) const MAX_LABELS: usize = 7;

/// The ground a label paints on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LabelGround {
    /// A chip of the overlay surface behind the text (the nomogram: a
    /// label reads in front of the egui_plot grid).
    #[default]
    Overlay,
    /// No chip: the text sits beside its line on the plot ground, as the
    /// histogram paints its labels (P17, lead review). A label that had to
    /// cross a soft line keeps a chip, so the line does not run through
    /// its text.
    None,
    /// A chip of the chart ground (the panel fill, `tokens::SURFACE_BASE`)
    /// behind the text (P17b item 2): a label ON its line masks the line
    /// under the letters only, so the line runs into the text and out the
    /// other side.
    Ground,
}

/// One label to place: its text, its colour, what it names and its ground.
pub(crate) struct ChartLabel {
    pub(crate) text: String,
    pub(crate) color: egui::Color32,
    pub(crate) anchor: LabelAnchor,
    pub(crate) ground: LabelGround,
}

/// What a label names, in screen space.
pub(crate) enum LabelAnchor {
    /// A line: the label sits beside a point of it, off the line by its
    /// gap, on the side that is clear.
    Line(Vec<egui::Pos2>),
    /// A mark: the label sits beside it, clear of its radius.
    Mark { center: egui::Pos2, radius: f32 },
    /// Any one of several lines (the two lines of the vendor range): the
    /// places of the first, then of the next.
    Lines(Vec<Vec<egui::Pos2>>),
    /// The end of a line (P17b item 2): the label sits ON the line at its
    /// last point. A level or shallow line takes the text along it, ending
    /// `END_INSET` before the last point (a full-width limit reads
    /// right-aligned inside the frame); a steep line takes the text
    /// horizontal beside its top end, touching the line, hanging down from
    /// it. The next place is the first point, then beside the line (the
    /// `Line` places of each segment). Several polylines (a step): the end
    /// of each, then beside each. The layout does not test a label against
    /// its own line.
    LineEnd(Vec<Vec<egui::Pos2>>),
}

impl LabelAnchor {
    /// The polylines a `LineEnd` label sits on: the layout does not test
    /// the label against them.
    fn own_lines(&self) -> &[Vec<egui::Pos2>] {
        match self {
            Self::LineEnd(lines) => lines,
            Self::Line(_) | Self::Lines(_) | Self::Mark { .. } => &[],
        }
    }
}

/// True when the drawn `line` is one of `own` (the same points, in either
/// order: a label may name a line from its other end, P17b).
fn is_own_line(line: &DrawnLine, own: &[Vec<egui::Pos2>]) -> bool {
    let same = |a: &egui::Pos2, b: &egui::Pos2| a.distance(*b) < 0.5;
    own.iter().any(|o| {
        o.len() == line.points.len()
            && (o.iter().zip(&line.points).all(|(a, b)| same(a, b))
                || o.iter().rev().zip(&line.points).all(|(a, b)| same(a, b)))
    })
}

/// The room between the end of a line and the end of a label on it, in
/// points: past the padding, so the chip clears a frame edge line or a
/// wall that the line ends on (a 2 pt line grown by the layout's 0.5).
const END_INSET: f32 = 2.5;

/// The room between a steep line and the chip beside its end, in points:
/// the half width of a 2 pt line, so the chip touches the line's edge and
/// the text starts `LABEL_PADDING.x` past it.
const SIDE_TOUCH: f32 = 1.0;

/// The drop of a chip that hangs from a steep end, in points (P29-10 10g,
/// lead ruling R8). Two needs set it:
///
/// - The chip clears a 2 pt line that meets the end (the feed cap on top
///   of an RPM wall): the layout refuses a chip within `half_width + 0.5`
///   = 1.5 pt of a line ([`line_hits_quad`]).
/// - The text shares the row of the meeting line's label, whose text is
///   centred on that line: with `LABEL_PADDING.y` the text top must be
///   less than half a text row (6.5 pt) minus the 3 pt row overlap under
///   the line, that is under 3.5 pt.
///
/// `2 x SIDE_TOUCH` (the width of a 2 pt line, 2 pt) meets both. The
/// general gap `LABEL_GAP` (3 pt) put the text of "Max RPM" 0.5 pt below
/// the row of "Max feed": a text line of its own.
const END_HANG: f32 = 2.0 * SIDE_TOUCH;

/// One drawn line in screen space, for the label layout.
pub(crate) struct DrawnLine {
    pub(crate) points: Vec<egui::Pos2>,
    pub(crate) half_width: f32,
    /// A faint line (the two vendor range lines, alpha `BAND_LIMIT_ALPHA`).
    /// A label crosses one only when it has no place that crosses no line:
    /// the label's background then covers the faint line for its length.
    pub(crate) soft: bool,
}

/// One placed label.
#[derive(Debug, Clone)]
pub(crate) struct PlacedLabel {
    pub(crate) text: String,
    pub(crate) color: egui::Color32,
    /// The background; the text is centred in it, at its angle.
    pub(crate) quad: Quad,
    /// The centre and radius of the mark a mark label names.
    pub(crate) mark: Option<(egui::Pos2, f32)>,
    /// The line a line label names (for `Lines`, the one it sits beside).
    /// The fit test holds each line label within reach of it.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "only the label fit test reads the named line")
    )]
    pub(crate) line: Option<Vec<egui::Pos2>>,
    /// The ground the label asked for.
    pub(crate) ground: LabelGround,
    /// The layout found no place clear of every line, so the label sits
    /// across a soft line: it keeps a chip on any ground.
    pub(crate) crossed_soft: bool,
}

/// The steepest line that a label follows at its angle, in radians. A
/// steeper line takes an upright label beside it: text tipped further
/// than this is hard to read.
const MAX_LABEL_ANGLE: f32 = 40.0 * std::f32::consts::PI / 180.0;

/// A label background: a rect of half extents `half` about `center`,
/// turned `angle` radians clockwise (the egui text angle).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Quad {
    pub(crate) center: egui::Pos2,
    pub(crate) half: egui::Vec2,
    pub(crate) angle: f32,
}

impl Quad {
    /// An upright quad on `rect`.
    pub(crate) fn upright(rect: egui::Rect) -> Self {
        Self {
            center: rect.center(),
            half: rect.size() / 2.0,
            angle: 0.0,
        }
    }

    /// The unit vectors of the quad's own x and y axes.
    fn axes(&self) -> (egui::Vec2, egui::Vec2) {
        let (sin, cos) = self.angle.sin_cos();
        (egui::vec2(cos, sin), egui::vec2(-sin, cos))
    }

    /// The four corners, in order around the quad.
    pub(crate) fn corners(&self) -> [egui::Pos2; 4] {
        let (u, v) = self.axes();
        let (x, y) = (u * self.half.x, v * self.half.y);
        let c = self.center;
        [c - x - y, c + x - y, c + x + y, c - x + y]
    }

    /// True when the two quads touch (separating axis test).
    pub(crate) fn intersects(&self, other: &Self) -> bool {
        let (a, b) = (self.corners(), other.corners());
        let (u0, v0) = self.axes();
        let (u1, v1) = other.axes();
        [u0, v0, u1, v1].iter().all(|axis| {
            let span = |pts: &[egui::Pos2; 4]| {
                pts.iter()
                    .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), p| {
                        let d = p.to_vec2().dot(*axis);
                        (lo.min(d), hi.max(d))
                    })
            };
            let ((a0, a1), (b0, b1)) = (span(&a), span(&b));
            a0 <= b1 && b0 <= a1
        })
    }

    /// True when the segment `a`-`b`, grown by `grow` on each side, touches
    /// the quad.
    pub(crate) fn hits_segment(&self, a: egui::Pos2, b: egui::Pos2, grow: f32) -> bool {
        let (u, v) = self.axes();
        let local = |p: egui::Pos2| {
            let d = p - self.center;
            egui::pos2(d.dot(u), d.dot(v))
        };
        let rect = egui::Rect::from_center_size(egui::Pos2::ZERO, 2.0 * self.half).expand(grow);
        segment_hits_rect(local(a), local(b), rect)
    }

    /// The distance from the polyline `points` to the nearest point of
    /// the quad's outline; 0 where they touch.
    pub(crate) fn distance_to_line(&self, points: &[egui::Pos2]) -> f32 {
        let corners = self.corners();
        let seg_dist = |p: egui::Pos2, a: egui::Pos2, b: egui::Pos2| {
            let ab = b - a;
            let t = if ab.length_sq() > 0.0 {
                ((p - a).dot(ab) / ab.length_sq()).clamp(0.0, 1.0)
            } else {
                0.0
            };
            (a + ab * t).distance(p)
        };
        points
            .windows(2)
            .filter_map(|w| match w {
                [a, b] => Some((*a, *b)),
                _ => None,
            })
            .map(|(a, b)| {
                if self.hits_segment(a, b, 0.0) {
                    return 0.0;
                }
                let from_corners = corners
                    .iter()
                    .map(|c| seg_dist(*c, a, b))
                    .fold(f32::INFINITY, f32::min);
                let edges = corners.iter().zip(corners.iter().cycle().skip(1));
                let from_ends = edges
                    .flat_map(|(c0, c1)| [seg_dist(a, *c0, *c1), seg_dist(b, *c0, *c1)])
                    .fold(f32::INFINITY, f32::min);
                from_corners.min(from_ends)
            })
            .fold(f32::INFINITY, f32::min)
    }

    /// True when every corner is inside `rect`.
    pub(crate) fn inside(&self, rect: egui::Rect) -> bool {
        self.corners().iter().all(|c| rect.contains(*c))
    }
}

/// The places along a line that a label tries: fractions of its length in
/// steps of this size.
const LINE_STEP: f32 = 0.025;

/// The fractions along a line that a label tries, those near 0.6 first: a
/// ray from the origin is crowded near the origin.
fn line_fractions() -> Vec<f32> {
    let steps = (1.0 / LINE_STEP).round() as i32;
    let mut out: Vec<f32> = (2..steps - 1).map(|i| i as f32 * LINE_STEP).collect();
    out.sort_by(|a, b| (a - 0.6).abs().total_cmp(&(b - 0.6).abs()));
    out
}

/// True when the segment `a`-`b` touches `rect` (Liang-Barsky clipping).
pub(crate) fn segment_hits_rect(a: egui::Pos2, b: egui::Pos2, rect: egui::Rect) -> bool {
    if rect.contains(a) || rect.contains(b) {
        return true;
    }
    let d = b - a;
    let (mut t0, mut t1) = (0.0_f32, 1.0_f32);
    for (p, q) in [
        (-d.x, a.x - rect.min.x),
        (d.x, rect.max.x - a.x),
        (-d.y, a.y - rect.min.y),
        (d.y, rect.max.y - a.y),
    ] {
        if p.abs() < f32::EPSILON {
            if q < 0.0 {
                return false;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                if t > t1 {
                    return false;
                }
                t0 = t0.max(t);
            } else {
                if t < t0 {
                    return false;
                }
                t1 = t1.min(t);
            }
        }
    }
    t0 <= t1
}

/// True when `quad` touches the drawn line, its width included.
pub(crate) fn line_hits_quad(line: &DrawnLine, quad: &Quad) -> bool {
    line.points
        .windows(2)
        .any(|w| matches!(w, [a, b] if quad.hits_segment(*a, *b, line.half_width + 0.5)))
}

/// Place the labels. `labels` is in priority order, the most important
/// first. A label takes the first candidate place that is inside `frame`,
/// clear of `keep_clear`, clear of every line of `lines` and clear of each
/// label placed before it.
///
/// A greedy pass in priority order can block a label that has few places
/// (a wall label in a narrow plot) with one that has many (a mark label).
/// So when a label finds no place, the pass runs again with that label
/// earlier, and the layout that places the more important labels wins. A
/// label with no place in the best layout is dropped: the least important
/// labels are last, so they go first when the plot is small. The result is
/// in priority order.
pub(crate) fn layout_labels(
    frame: egui::Rect,
    keep_clear: &[egui::Rect],
    lines: &[DrawnLine],
    labels: &[(ChartLabel, egui::Vec2)],
) -> Vec<PlacedLabel> {
    let mut order: Vec<usize> = (0..labels.len()).collect();
    let mut best: Option<(Vec<bool>, Vec<PlacedLabel>)> = None;
    for _ in 0..=labels.len() {
        let placed = greedy_layout(frame, keep_clear, lines, labels, &order);
        let got: Vec<bool> = labels
            .iter()
            .map(|(l, _)| placed.iter().any(|p| p.text == l.text))
            .collect();
        let first_missing = got.iter().position(|g| !g);
        if best.as_ref().is_none_or(|(b, _)| got > *b) {
            best = Some((got, placed));
        }
        let Some(missing) = first_missing else {
            break;
        };
        // Run again with the missing label one place earlier than before.
        let Some(at) = order.iter().position(|i| *i == missing) else {
            break;
        };
        if at == 0 {
            break;
        }
        order.swap(at, at - 1);
    }
    best.map(|(_, placed)| placed).unwrap_or_default()
}

/// One greedy pass of [`layout_labels`] in `order`. The result is in
/// priority order.
pub(crate) fn greedy_layout(
    frame: egui::Rect,
    keep_clear: &[egui::Rect],
    lines: &[DrawnLine],
    labels: &[(ChartLabel, egui::Vec2)],
    order: &[usize],
) -> Vec<PlacedLabel> {
    let inside = frame.shrink(1.0);
    let mut placed: Vec<(usize, PlacedLabel)> = Vec::new();
    for &i in order {
        let Some((label, size)) = labels.get(i) else {
            continue;
        };
        let own = label.anchor.own_lines();
        let fits = |quad: &Quad, cross_soft: bool| {
            quad.inside(inside)
                && !keep_clear
                    .iter()
                    .any(|r| quad.intersects(&Quad::upright(*r)))
                && !lines.iter().any(|l| {
                    !(cross_soft && l.soft) && !is_own_line(l, own) && line_hits_quad(l, quad)
                })
                && !placed.iter().any(|(_, p)| {
                    let mut grown = p.quad;
                    grown.half += egui::Vec2::splat(1.0);
                    grown.intersects(quad)
                })
        };
        let all = candidates(&label.anchor, *size);
        let found = all
            .iter()
            .find(|q| fits(q, false))
            .map(|q| (*q, false))
            .or_else(|| all.iter().find(|q| fits(q, true)).map(|q| (*q, true)));
        if let Some((quad, crossed_soft)) = found {
            placed.push((
                i,
                PlacedLabel {
                    text: label.text.clone(),
                    color: label.color,
                    quad,
                    ground: label.ground,
                    crossed_soft,
                    mark: match label.anchor {
                        LabelAnchor::Mark { center, radius } => Some((center, radius)),
                        LabelAnchor::Line(_) | LabelAnchor::Lines(_) | LabelAnchor::LineEnd(_) => {
                            None
                        }
                    },
                    line: match &label.anchor {
                        LabelAnchor::Mark { .. } => None,
                        LabelAnchor::Line(points) => Some(points.clone()),
                        LabelAnchor::Lines(lines) | LabelAnchor::LineEnd(lines) => lines
                            .iter()
                            .min_by(|a, b| {
                                quad.distance_to_line(a)
                                    .total_cmp(&quad.distance_to_line(b))
                            })
                            .cloned(),
                    },
                },
            ));
        }
    }
    placed.sort_by_key(|(i, _)| *i);
    placed.into_iter().map(|(_, p)| p).collect()
}

/// The candidate places of one label of `size`, best first.
pub(crate) fn candidates(anchor: &LabelAnchor, size: egui::Vec2) -> Vec<Quad> {
    let place =
        |point: egui::Pos2, align: egui::Align2| Quad::upright(align.anchor_size(point, size));
    let g = LABEL_GAP;
    match anchor {
        LabelAnchor::Mark { center, radius } => {
            let c = *center;
            let d = radius + g;
            // Level with the mark first, then tucked under or over it, so
            // the label reads as part of the mark (lead, 2026-10-31).
            let mut out = vec![
                place(c + egui::vec2(d, 0.0), egui::Align2::LEFT_CENTER),
                place(c - egui::vec2(d, 0.0), egui::Align2::RIGHT_CENTER),
                place(c + egui::vec2(2.0, d), egui::Align2::LEFT_TOP),
                place(c + egui::vec2(-2.0, d), egui::Align2::RIGHT_TOP),
                place(c + egui::vec2(2.0, -d), egui::Align2::LEFT_BOTTOM),
                place(c + egui::vec2(-2.0, -d), egui::Align2::RIGHT_BOTTOM),
                place(c + egui::vec2(0.0, d), egui::Align2::CENTER_TOP),
                place(c - egui::vec2(0.0, d), egui::Align2::CENTER_BOTTOM),
            ];
            // Where the near places are taken: every place on a 4-point
            // grid within `MARK_SCAN` of the mark, nearest first.
            let mut grid: Vec<(f32, Quad)> = Vec::new();
            let steps = (MARK_SCAN / 4.0) as i32;
            for i in -steps - (size.x / 4.0) as i32..=steps {
                for j in -steps - (size.y / 4.0) as i32..=steps {
                    let rect = egui::Rect::from_min_size(
                        c + egui::vec2(i as f32 * 4.0, j as f32 * 4.0),
                        size,
                    );
                    let dist = rect.distance_to_pos(c);
                    if dist >= d && dist <= MARK_SCAN {
                        grid.push((dist, Quad::upright(rect)));
                    }
                }
            }
            grid.sort_by(|a, b| a.0.total_cmp(&b.0));
            out.extend(grid.into_iter().map(|(_, q)| q));
            out
        }
        LabelAnchor::Lines(lines) => lines
            .iter()
            .flat_map(|line| candidates(&LabelAnchor::Line(line.clone()), size))
            .collect(),
        LabelAnchor::LineEnd(lines) => {
            // The end of each polyline, then its start, then touching the
            // side of each steep segment, then on each flat segment, then
            // beside each segment of each.
            let mut out: Vec<Quad> = lines
                .iter()
                .filter_map(|line| end_places(line, true, size))
                .chain(
                    lines
                        .iter()
                        .filter_map(|line| end_places(line, false, size)),
                )
                .flatten()
                .collect();
            out.extend(lines.iter().flat_map(|line| side_places(line, size)));
            out.extend(lines.iter().flat_map(|line| on_places(line, size)));
            out.extend(lines.iter().flat_map(|line| {
                line.windows(2)
                    .flat_map(|w| candidates(&LabelAnchor::Line(w.to_vec()), size))
                    .collect::<Vec<_>>()
            }));
            out
        }
        LabelAnchor::Line(points) => {
            let mut out = Vec::new();
            let (Some(first), Some(last)) = (points.first(), points.last()) else {
                return out;
            };
            let along = *last - *first;
            if along.length() < 1.0 {
                return out;
            }
            // Point the direction right, so the text reads left to right.
            let dir = if along.x < 0.0 { -along } else { along }.normalized();
            let angle = dir.y.atan2(dir.x);
            // The two normals, the upper one first (an upright line: the
            // left one first).
            let n = egui::vec2(dir.y, -dir.x);
            let up = if n.y < 0.0 || (n.y.abs() < f32::EPSILON && n.x < 0.0) {
                n
            } else {
                -n
            };
            let half = size / 2.0;
            // A line no steeper than `MAX_LABEL_ANGLE` takes the label at
            // its own angle, so the label reads as part of it; a steeper
            // one takes an upright label beside it. `reach` is the distance
            // from the line to the label centre that puts the near side of
            // the label `LABEL_GAP` off the line.
            let (label_angle, reach) = if angle.abs() <= MAX_LABEL_ANGLE {
                (angle, half.y + g)
            } else {
                (0.0, up.x.abs() * half.x + up.y.abs() * half.y + g)
            };
            for t in line_fractions() {
                let point = *first + along * t;
                for side in [up, -up] {
                    out.push(Quad {
                        center: point + side * reach,
                        half,
                        angle: label_angle,
                    });
                }
            }
            out
        }
    }
}

/// The places of a label of `size` at one end of `line` (`last`: the last
/// point, else the first): on the line, along its end segment, when the
/// segment is no steeper than `MAX_LABEL_ANGLE`; else horizontal beside
/// the end, touching the line, hanging down from a top end (up from a
/// bottom end), the side nearer the frame centre first: the two sides.
/// `None` for a line of fewer than two points or no length.
fn end_places(line: &[egui::Pos2], last: bool, size: egui::Vec2) -> Option<Vec<Quad>> {
    let (end, prev) = if last {
        (*line.last()?, *line.get(line.len().checked_sub(2)?)?)
    } else {
        (*line.first()?, *line.get(1)?)
    };
    // The unit vector from the end into the line.
    let inward = prev - end;
    if inward.length() < 1.0 {
        return None;
    }
    let inward = inward.normalized();
    let half = size / 2.0;
    // The text reads left to right: the angle of the segment pointed right.
    let dir = if inward.x < 0.0 { -inward } else { inward };
    let angle = dir.y.atan2(dir.x);
    if angle.abs() <= MAX_LABEL_ANGLE {
        // On the line: the chip's far corner clears the end by `END_INSET`
        // (a turned chip reaches past its centre line by `half.y sin`).
        let reach = half.x + END_INSET + half.y * angle.sin().abs();
        return Some(vec![Quad {
            center: end + inward * reach,
            half,
            angle,
        }]);
    }
    // Steep: horizontal text beside the end, the chip's near edge on the
    // edge of a 2 pt line (touching it), hanging into the line's own
    // extent (down from a top end), `END_HANG` past the end.
    let y = if inward.y > 0.0 {
        end.y + END_HANG + half.y
    } else {
        end.y - END_HANG - half.y
    };
    let dx = SIDE_TOUCH + half.x;
    Some(vec![
        Quad {
            center: egui::pos2(end.x + dx, y),
            half,
            angle: 0.0,
        },
        Quad {
            center: egui::pos2(end.x - dx, y),
            half,
            angle: 0.0,
        },
    ])
}

/// The places of a label of `size` touching the side of each steep
/// segment of `line` (steeper than `MAX_LABEL_ANGLE`): horizontal text,
/// the chip's near edge on the edge of the line as at a steep end, at the
/// fractions of [`line_fractions`], both sides. A wall whose two ends are
/// taken keeps its name against it (P17b: "Max RPM" floated free of its
/// wall).
fn side_places(line: &[egui::Pos2], size: egui::Vec2) -> Vec<Quad> {
    let half = size / 2.0;
    let mut out = Vec::new();
    for w in line.windows(2) {
        let [a, b] = w else { continue };
        let along = *b - *a;
        if along.length() < 1.0 {
            continue;
        }
        let dir = if along.x < 0.0 { -along } else { along };
        if dir.y.atan2(dir.x).abs() <= MAX_LABEL_ANGLE {
            continue;
        }
        // A slanted segment moves across the chip's height by `slant`; the
        // chip clears it by that much more, so it never lies on its own
        // line (the layout does not test a label against its own line).
        let slant = half.y * (along.x / along.y).abs();
        let dx = SIDE_TOUCH + half.x + slant;
        for t in line_fractions() {
            let p = *a + along * t;
            for side in [dx, -dx] {
                out.push(Quad {
                    center: egui::pos2(p.x + side, p.y),
                    half,
                    angle: 0.0,
                });
            }
        }
    }
    out
}

/// The places of a label of `size` ON each flat segment of `line` (no
/// steeper than `MAX_LABEL_ANGLE`), along the segment, at the fractions of
/// [`line_fractions`]. A flat line whose two ends are taken keeps its name
/// on it (P29-7 7c: on the rivmap envelope the depth-table line took the
/// places near the ends of the vendor DOC line, and its name floated off
/// the line). The layout does not test a label against its own line.
fn on_places(line: &[egui::Pos2], size: egui::Vec2) -> Vec<Quad> {
    let half = size / 2.0;
    let mut out = Vec::new();
    for w in line.windows(2) {
        let [a, b] = w else { continue };
        let along = *b - *a;
        if along.length() < 1.0 {
            continue;
        }
        let dir = if along.x < 0.0 { -along } else { along };
        let angle = dir.y.atan2(dir.x);
        if angle.abs() > MAX_LABEL_ANGLE {
            continue;
        }
        for t in line_fractions() {
            out.push(Quad {
                center: *a + along * t,
                half,
                angle,
            });
        }
    }
    out
}

/// Size each label and place them with [`layout_labels`]; at most
/// `MAX_LABELS` stay. Shared with the envelope plot.
pub(crate) fn place_labels(
    ui: &egui::Ui,
    frame: egui::Rect,
    keep_clear: &[egui::Rect],
    lines: &[DrawnLine],
    specs: Vec<ChartLabel>,
) -> Vec<PlacedLabel> {
    let font = egui::FontId::proportional(tokens::SIZE_CAPTION);
    let sized: Vec<(ChartLabel, egui::Vec2)> = specs
        .into_iter()
        .map(|spec| {
            let size = ui
                .painter()
                .layout_no_wrap(spec.text.clone(), font.clone(), spec.color)
                .size();
            (spec, size + 2.0 * LABEL_PADDING)
        })
        .collect();
    let mut placed = layout_labels(frame, keep_clear, lines, &sized);
    placed.truncate(MAX_LABELS);
    placed
}

/// Paint the placed labels: a background chip on the `Overlay` ground (or
/// where the label crosses a soft line), the text at its angle, and a
/// leader to a mark that the layout moved.
pub(crate) fn paint_labels(ui: &egui::Ui, frame: egui::Rect, placed: &[PlacedLabel]) {
    let font = egui::FontId::proportional(tokens::SIZE_CAPTION);
    let painter = ui.painter_at(frame);
    for label in placed {
        // A mark label that the layout had to move off its mark takes a
        // thin leader line to the mark, so the reader sees what it names.
        if let Some((center, radius)) = label.mark {
            let near = egui::Rect::from_center_size(label.quad.center, 2.0 * label.quad.half)
                .clamp(center);
            let off = near - center;
            if off.length() > radius + LABEL_GAP + 2.0 {
                let start = center + off.normalized() * radius;
                let _ = painter.line_segment(
                    [start, near],
                    egui::Stroke::new(1.0, wash(label.color, LEADER_ALPHA)),
                );
            }
        }
        let chip = match label.ground {
            LabelGround::Ground => Some(tokens::SURFACE_BASE),
            LabelGround::Overlay => Some(wash(tokens::SURFACE_OVERLAY, LABEL_BACKGROUND_ALPHA)),
            LabelGround::None => label
                .crossed_soft
                .then_some(wash(tokens::SURFACE_OVERLAY, LABEL_BACKGROUND_ALPHA)),
        };
        if let Some(fill) = chip {
            let _ = painter.add(egui::Shape::convex_polygon(
                label.quad.corners().to_vec(),
                fill,
                egui::Stroke::NONE,
            ));
        }
        let galley = painter.layout_no_wrap(label.text.clone(), font.clone(), label.color);
        let pos = label.quad.center - galley.size() / 2.0;
        let _ = painter.add(
            egui::epaint::TextShape::new(pos, galley, label.color)
                .with_angle_and_anchor(label.quad.angle, egui::Align2::CENTER_CENTER),
        );
    }
}

/// Which way a colliding label may move before it is left out.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Nudge {
    None,
    Left,
    Right,
}

/// Paint one axis label when it does not collide with a label already
/// placed, and keep it inside `bounds`. A label with a `Nudge` moves clear
/// of the label it collides with, in that direction, and is left out when
/// it still collides or leaves `bounds`.
#[expect(
    clippy::too_many_arguments,
    reason = "a private painter helper; the nine inputs are one label's position, text, style and nudge, and a struct for them would be used once."
)]
pub(crate) fn place_label(
    painter: &egui::Painter,
    placed: &mut Vec<egui::Rect>,
    bounds: egui::Rect,
    anchor: egui::Pos2,
    align: egui::Align2,
    text: &str,
    font: &egui::FontId,
    colour: egui::Color32,
    nudge: Nudge,
) {
    let galley = painter.layout_no_wrap(text.to_owned(), font.clone(), colour);
    let mut label = align.anchor_size(anchor, galley.size());
    if label.left() < bounds.left() {
        label = label.translate(egui::vec2(bounds.left() - label.left(), 0.0));
    }
    if label.right() > bounds.right() {
        label = label.translate(egui::vec2(bounds.right() - label.right(), 0.0));
    }
    let pad = egui::vec2(tokens::SPACE_1, 0.0);
    // At most one move per placed label, so the loop ends.
    for _ in 0..=placed.len() {
        let padded = label.expand2(pad);
        let Some(other) = placed.iter().find(|other| other.intersects(padded)) else {
            placed.push(label);
            painter.galley(label.min, galley, colour);
            return;
        };
        let shift = match nudge {
            Nudge::None => return,
            Nudge::Left => other.left() - tokens::SPACE_3 - label.right(),
            Nudge::Right => other.right() + tokens::SPACE_3 - label.left(),
        };
        label = label.translate(egui::vec2(shift, 0.0));
        if label.left() < bounds.left() || label.right() > bounds.right() {
            return;
        }
    }
}
