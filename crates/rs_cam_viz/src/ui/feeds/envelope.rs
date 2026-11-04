//! The safe envelope plot of the Passes tab (feeds contract P5b, K2b; P5c
//! operator review).
//!
//! The P5 `SafeEnvelope` of the selected op is a region of the (depth,
//! width) plane. This file draws it in the inspector, above the depth and
//! stepover rows (operator ruling 2026-11-03, D1):
//!
//! - A plane op gets a small plot: the width (ae) across, the depth (ap)
//!   up. The frame is the hard stops (P5c): the width axis ends at the tool
//!   diameter, and the depth axis never goes past the cutting length or the
//!   flute guard. The plot draws only the limits that matter at the marks
//!   (`EnvelopeScene::of`), each with its short name on the line (P5c
//!   round 2). The hover names a limit too. No item is a filled region
//!   (G-CHARTLINES, decision D2).
//! - An op with one free value (a Trace or Project Curve depth, a drill
//!   peck, a surface width) gets a bar of one row: it ends at the binding
//!   limit, and the limit is named at its end.
//! - An op with no free value draws nothing.
//!
//! The marks (ruling R23; P5c round 2 after the operator's live review):
//! the white dot is the setpoint. A click or a drag moves the dot itself:
//! while a candidate exists, the dot sits at the candidate (a draft of the
//! setpoint) and a hollow ghost marks the stored point. Esc returns the dot
//! to the stored point; the ⚡ applies it. Blue marks only the
//! recommendation: the proposal diamond, drawn when the proposal is not at
//! the stored point. A point outside warns; nothing refuses it (ruling Q6).
//!
//! The live link (operator ruling 2026-11-03): a click or a drag moves the
//! candidate (GUI state, `state::panels::EnvelopeCandidates`); the hover
//! readout and the nomogram read the `SpeedsMap` at the candidate on every
//! frame, a lookup with no Suggest run. Nothing is written until the header
//! ⚡ sends `AppEvent::ApplyFeedsPick`. A drag that leaves the plot keeps
//! the candidate on the frame edge; the view range does not follow the
//! candidate, so the axes stay still during a drag (P5c).
//!
//! The header is the one text row this section adds; the readout is the
//! hover only.

use egui_plot::{Line, MarkerShape, Plot, PlotPoints, Points};
use rs_cam_core::compute::catalog::{Axial, OperationConfig, Radial};
use rs_cam_core::feeds::envelope::{
    AxisSpan, CutPoint, Edge, EnvelopeAxes, Limit, LimitKind, PickRequest, Placement, SafeEnvelope,
    SpeedsSample,
};

use super::shared::{
    ChartLabel, DrawnLine, LabelAnchor, axis_title_overhang, nice_steps, paint_labels, place_labels,
};
use crate::state::panels::EnvelopeCandidates;
use crate::ui::{AppEvent, tokens};

/// The height of the plane plot. Smaller than the nomogram (180): the two
/// charts share the Passes tab with the op rows.
const PLOT_HEIGHT: f32 = 140.0;

/// The height of the one-value bar.
const BAR_HEIGHT: f32 = 18.0;

/// The title of the depth axis, rotated at the left of the plot.
const DEPTH_AXIS_TITLE: &str = "Depth (mm)";

/// The title of the width axis.
const WIDTH_AXIS_TITLE: &str = "Width (mm)";

/// The room of the depth axis ticks.
const Y_AXIS_MIN_WIDTH: f32 = 28.0;

/// The spacing range of the width tick labels, in points (egui_plot fades a
/// label in across it and hides it below the low end).
const X_LABEL_SPACING: std::ops::RangeInclusive<f32> = 45.0..=60.0;

/// The width of a limit line, in points (P5c round 2: twice the 1-point
/// grid line, so a limit reads above the grid).
const LINE_WIDTH: f32 = 2.0;

/// The width of the outline of the ghost and the proposal diamond, in
/// points.
const OUTLINE_WIDTH: f32 = 1.5;

/// The radius of the hollow ghost at the stored point, in points.
const GHOST_RADIUS: f32 = 4.5;

/// The room at each end of the bar, in points: the half width of the
/// widest mark (the proposal diamond) and its outline.
const BAR_END_ROOM: f32 = 7.0;

/// The room a label keeps around a mark, in points.
const MARK_CLEARANCE: f32 = 4.0;

/// The width of the safe span of the bar, in points (a span, not a limit).
const SPAN_WIDTH: f32 = 1.0;

/// The radius of the proposal diamond, in points.
const PROPOSAL_RADIUS: f32 = 6.0;

/// The radius of the operator's dot, in points (the nomogram's `Now`).
const DOT_RADIUS: f32 = 3.5;

/// The margin of the depth axis past the highest mark, a share of the mark.
const AXIS_MARGIN: f64 = 0.05;

/// The room of the view past the lowest depth ceiling, a factor (a layout
/// choice: the ceiling sits at about three quarters of the axis).
const VIEW_HEADROOM: f64 = 1.3;

/// A limit line is near a mark when its distance is at most this share of
/// the view (a layout choice: a fifth of the plot). A line that is not
/// near, does not bind and is not crossed is not drawn.
const NEAR_SHARE: f64 = 0.2;

/// The hover names a limit line when the pointer is this near it, in
/// points.
const HOVER_NEAR_PX: f32 = 8.0;

/// The margin below which a point is outside a limit (the core
/// `placement_of` rule).
const OUTSIDE_EPS: f64 = 1e-9;

/// The ⚡ of the header (design rule 8: the Suggest mark).
pub const APPLY_GLYPH: &str = "\u{26A1}";

/// The section title of a plane op.
pub const TITLE_PLANE: &str = "Safe cut";
/// The section title of an op whose free value is the depth.
pub const TITLE_DEPTH: &str = "Safe depth";
/// The section title of an op whose free value is the width.
pub const TITLE_WIDTH: &str = "Safe width";

/// The name of a limit on the envelope surfaces: the hover, the readout,
/// the bar end and the ⚡ hover. P5c: each name reads as a limit (the
/// operator read "Vendor" as a recommendation).
#[must_use]
pub const fn short_label(kind: LimitKind) -> &'static str {
    match kind {
        LimitKind::FluteGuard => "Flute guard",
        LimitKind::CuttingLength => "Cutting length",
        LimitKind::VendorAxial => "Vendor max DOC",
        LimitKind::Rigidity => "Rigidity max DOC",
        LimitKind::Slot => "Slot max DOC",
        LimitKind::Power => "Spindle power",
        LimitKind::Deflection => "Deflection",
        LimitKind::ChipBand => "Chip band",
        LimitKind::RubbingFloor => "Rubbing",
        LimitKind::FeedCap => "Max feed",
        LimitKind::Diameter => "Tool diameter",
        LimitKind::Runtime => "Min width (move budget)",
        LimitKind::EngagementFloor => "Min engagement",
    }
}

/// A limit that is the plot frame, not a line in it (P5c): the diameter
/// ends the width axis, the cutting length and the flute guard end the
/// depth axis, and the engagement floor is the origin.
const fn is_frame(kind: LimitKind) -> bool {
    match kind {
        LimitKind::Diameter
        | LimitKind::CuttingLength
        | LimitKind::FluteGuard
        | LimitKind::EngagementFloor => true,
        LimitKind::VendorAxial
        | LimitKind::Rigidity
        | LimitKind::Slot
        | LimitKind::Power
        | LimitKind::Deflection
        | LimitKind::ChipBand
        | LimitKind::RubbingFloor
        | LimitKind::FeedCap
        | LimitKind::Runtime => false,
    }
}

/// What the envelope section shows for one op.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvelopeShape {
    /// A plot of the (ae, ap) plane.
    Plane,
    /// A bar of the depth.
    DepthBar,
    /// A bar of the width.
    WidthBar,
    /// No free value: no section.
    None,
}

impl EnvelopeShape {
    /// The shape of `axes`.
    #[must_use]
    pub const fn of(axes: &EnvelopeAxes) -> Self {
        match axes {
            EnvelopeAxes::Plane { .. } => Self::Plane,
            EnvelopeAxes::AxialBar { .. } => Self::DepthBar,
            EnvelopeAxes::RadialBar { .. } => Self::WidthBar,
            EnvelopeAxes::Fixed { .. } => Self::None,
        }
    }

    /// The section title, or `None` when there is no section.
    #[must_use]
    pub const fn title(self) -> Option<&'static str> {
        match self {
            Self::Plane => Some(TITLE_PLANE),
            Self::DepthBar => Some(TITLE_DEPTH),
            Self::WidthBar => Some(TITLE_WIDTH),
            Self::None => None,
        }
    }
}

/// One limit line of the plot, in plot coordinates `[ae, ap]`.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneEdge {
    /// The limit.
    pub kind: LimitKind,
    /// The line, in `[ae, ap]` (mm).
    pub points: Vec<[f64; 2]>,
    /// The subject (the candidate, else Now) sits outside this limit.
    pub crossed: bool,
}

/// What the plot draws: a pure function of the envelope and the candidate,
/// so a test reads it without a renderer.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvelopeScene {
    /// The shape of the section.
    pub shape: EnvelopeShape,
    /// The width range of the plot (mm): `0..D`, the hard stop.
    pub ae_range: (f64, f64),
    /// The depth range of the plot (mm), never past the cutting length or
    /// the flute guard.
    pub ap_range: (f64, f64),
    /// One line per drawn limit: the limit that binds the subject, a
    /// crossed limit, and a limit near a mark. The frame limits draw no
    /// line.
    pub edges: Vec<SceneEdge>,
    /// The operator's stated point ("Now"), and whether it sits outside.
    pub dot: Option<(CutPoint, bool)>,
    /// The candidate of a click or a drag (GUI state), if one exists.
    pub candidate: Option<CutPoint>,
    /// Where the candidate sits.
    pub candidate_placement: Option<Placement>,
    /// The envelope's proposal.
    pub proposal: CutPoint,
    /// The proposal is drawn: it is not at Now.
    pub proposal_shown: bool,
    /// The limit that binds the subject (the candidate, else Now, else the
    /// proposal): the worst crossed limit when it sits outside, else the
    /// drawn limit with the smallest margin.
    pub binding: Option<LimitKind>,
}

impl EnvelopeScene {
    /// The scene of `env` with the candidate `candidate`.
    #[must_use]
    pub fn of(env: &SafeEnvelope, candidate: Option<CutPoint>) -> Self {
        let shape = EnvelopeShape::of(&env.axes);
        let proposal = env.proposal.get();
        let dot = env.stated.map(|p| {
            let outside = matches!(env.operator, Some(Placement::Outside { .. }));
            (p, outside)
        });
        let now = dot.map(|(p, _)| p);
        let proposal_shown = now.is_none_or(|p| !same_point(p, proposal));
        let subject = candidate.or(now).unwrap_or(proposal);
        let (ae_range, ap_range) = view_ranges(env, proposal, now);

        let lines: Vec<&Limit> = env.boundary.iter().filter(|l| !is_frame(l.kind)).collect();
        let binding = binding_at(&lines, subject);
        let marks: Vec<CutPoint> = [Some(subject), now, proposal_shown.then_some(proposal)]
            .into_iter()
            .flatten()
            .collect();
        let mut edges: Vec<SceneEdge> = lines
            .iter()
            .filter_map(|l| {
                let points = edge_points(l, ae_range, ap_range)?;
                let crossed = l.margin(subject).is_some_and(|m| m < -OUTSIDE_EPS);
                let binds = Some(l.kind) == binding;
                // The move budget is drawn only when it binds (P5c item 5).
                if l.kind == LimitKind::Runtime && !binds && !crossed {
                    return None;
                }
                if !in_view(&points, ae_range, ap_range) {
                    return None;
                }
                let near = marks
                    .iter()
                    .any(|m| view_distance(*m, &points, ae_range, ap_range) <= NEAR_SHARE);
                (binds || crossed || near).then_some(SceneEdge {
                    kind: l.kind,
                    points,
                    crossed,
                })
            })
            .collect();
        // A crossed line paints over a neutral one.
        edges.sort_by_key(|e| e.crossed);
        Self {
            shape,
            ae_range,
            ap_range,
            edges,
            dot,
            candidate,
            candidate_placement: candidate.map(|c| env.placement_of(c)),
            proposal,
            proposal_shown,
            binding,
        }
    }

    /// The candidate sits outside the envelope.
    #[must_use]
    pub const fn candidate_outside(&self) -> bool {
        matches!(self.candidate_placement, Some(Placement::Outside { .. }))
    }

    // The colour rule of the envelope plot and the bar (P5c item 6; round 2
    // after the operator's live review, 2026-11-04):
    //
    // - The white dot is the setpoint (`tokens::TEXT_STRONG`). A drag moves
    //   the dot itself: while a candidate exists the dot sits at the
    //   candidate (a draft of the setpoint), and a hollow ghost
    //   (`tokens::TEXT_MUTED` outline, no fill) marks the stored point.
    // - Blue (`tokens::ACCENT`) is only the recommendation: the proposal
    //   diamond, drawn when the proposal is not at the stored point. Blue
    //   never marks the thing the operator drags.
    // - A limit line is `tokens::TEXT_BODY`, `LINE_WIDTH` (2 points) wide,
    //   over a grid in `tokens::BORDER`: each line reads above the grid. Its
    //   label is the same colour.
    // - A line that the dot crosses, and the dot when it sits outside, are
    //   `tokens::CAUTION` (amber).
    // - No `tokens::DANGER` on this plot: an outside point warns, it is
    //   not refused (ruling Q6).

    /// Where the white dot is: the candidate (a draft of the setpoint),
    /// else the stored point.
    #[must_use]
    pub fn setpoint(&self) -> Option<CutPoint> {
        self.candidate.or(self.dot.map(|(p, _)| p))
    }

    /// The hollow ghost at the stored point: only while a candidate exists.
    #[must_use]
    pub fn ghost(&self) -> Option<CutPoint> {
        self.candidate.and(self.dot.map(|(p, _)| p))
    }

    /// The colour of the proposal mark.
    #[must_use]
    pub const fn proposal_color(&self) -> egui::Color32 {
        tokens::ACCENT
    }

    /// The colour of the white dot: `tokens::CAUTION` when the dot (the
    /// candidate, else the stored point) sits outside.
    #[must_use]
    pub fn dot_color(&self) -> Option<egui::Color32> {
        let outside = match self.candidate {
            Some(_) => Some(self.candidate_outside()),
            None => self.dot.map(|(_, outside)| outside),
        }?;
        Some(if outside {
            tokens::CAUTION
        } else {
            tokens::TEXT_STRONG
        })
    }

    /// The colour of the line of `edge`, and of its label.
    #[must_use]
    pub const fn edge_color(&self, edge: &SceneEdge) -> egui::Color32 {
        if edge.crossed {
            tokens::CAUTION
        } else {
            tokens::TEXT_BODY
        }
    }

    /// The pick that the ⚡ sends: the candidate, else the proposal. A
    /// drill takes its free value as its peck.
    #[must_use]
    pub const fn pick(&self, peck: bool) -> PickRequest {
        match (self.candidate, peck) {
            (None, _) => PickRequest::Proposal,
            (Some(c), true) => PickRequest::Peck(c.ap_mm),
            (Some(c), false) => PickRequest::Point {
                ap_mm: c.ap_mm,
                ae_mm: c.ae_mm,
            },
        }
    }

    /// `p` moved onto the frame, for a mark that sits past it.
    #[must_use]
    pub const fn on_frame(&self, p: CutPoint) -> CutPoint {
        CutPoint {
            ap_mm: p.ap_mm.clamp(self.ap_range.0, self.ap_range.1),
            ae_mm: p.ae_mm.clamp(self.ae_range.0, self.ae_range.1),
        }
    }
}

/// Two points are one mark (a micrometre apart at most).
fn same_point(a: CutPoint, b: CutPoint) -> bool {
    (a.ap_mm - b.ap_mm).abs() < 1e-6 && (a.ae_mm - b.ae_mm).abs() < 1e-6
}

/// The `mm` of an `AeCeiling` edge.
const fn ae_ceiling_mm(l: &Limit) -> Option<f64> {
    if let Edge::AeCeiling { mm } = l.edge {
        Some(mm)
    } else {
        None
    }
}

/// The `mm` of an `ApCeiling` edge.
const fn ap_ceiling_mm(l: &Limit) -> Option<f64> {
    if let Edge::ApCeiling { mm } = l.edge {
        Some(mm)
    } else {
        None
    }
}

/// The view of the plot: `(ae_range, ap_range)`. P5c: it reads the hard
/// stops, the proposal and Now, and never the candidate, so a drag does
/// not move the axes.
///
/// - Width: `0..D`. The tool diameter is the frame (no cut is wider than a
///   slot). With no diameter limit, the span of the axis.
/// - Depth: up to a share past the lowest depth ceiling and past the marks,
///   never past the cutting length or the flute guard (the lower of the
///   two). A ceiling far above the cut then does not squeeze the marks into
///   the corner.
fn view_ranges(
    env: &SafeEnvelope,
    proposal: CutPoint,
    now: Option<CutPoint>,
) -> ((f64, f64), (f64, f64)) {
    let (ae_span, ap_span) = spans(&env.axes);
    let positive = |v: f64| (v.is_finite() && v > 0.0).then_some(v);
    let ae_max = env
        .limit(LimitKind::Diameter)
        .and_then(ae_ceiling_mm)
        .and_then(positive)
        .or_else(|| positive(ae_span.max_mm))
        .unwrap_or(1.0);
    let hard_top = env
        .boundary
        .iter()
        .filter(|l| matches!(l.kind, LimitKind::CuttingLength | LimitKind::FluteGuard))
        .filter_map(ap_ceiling_mm)
        .filter_map(positive)
        .fold(None, |acc: Option<f64>, v| {
            Some(acc.map_or(v, |a| a.min(v)))
        })
        .or_else(|| positive(ap_span.max_mm));
    let mark = now.map_or(proposal.ap_mm, |n| n.ap_mm.max(proposal.ap_mm));
    let ceiling = env.ap_ceiling_at(proposal.ae_mm).map_or(0.0, |c| c.0);
    let want = (ceiling.max(mark) * VIEW_HEADROOM).max(mark * (1.0 + AXIS_MARGIN));
    let ap_max = match (positive(want), hard_top) {
        (Some(w), Some(h)) => w.min(h),
        (Some(w), None) => w,
        (None, Some(h)) => h,
        (None, None) => 1.0,
    };
    ((0.0, ae_max), (0.0, ap_max))
}

/// The limit with the smallest margin at `p` among `lines`; the worst
/// crossed one when `p` sits outside.
fn binding_at(lines: &[&Limit], p: CutPoint) -> Option<LimitKind> {
    lines
        .iter()
        .filter_map(|l| l.margin(p).map(|m| (m, l.kind)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, kind)| kind)
}

/// Some segment of `points` meets the view rect.
fn in_view(points: &[[f64; 2]], ae: (f64, f64), ap: (f64, f64)) -> bool {
    let eps = 1e-9;
    points.windows(2).any(|w| match w {
        [a, b] => {
            a[0].min(b[0]) <= ae.1 + eps
                && a[0].max(b[0]) >= ae.0 - eps
                && a[1].min(b[1]) <= ap.1 + eps
                && a[1].max(b[1]) >= ap.0 - eps
        }
        _ => false,
    })
}

/// The distance from `p` to the line `points`, in shares of the view (each
/// axis divided by its range).
fn view_distance(p: CutPoint, points: &[[f64; 2]], ae: (f64, f64), ap: (f64, f64)) -> f64 {
    let sx = (ae.1 - ae.0).max(f64::EPSILON);
    let sy = (ap.1 - ap.0).max(f64::EPSILON);
    let q = [p.ae_mm / sx, p.ap_mm / sy];
    points
        .windows(2)
        .filter_map(|w| match w {
            [a, b] => Some(segment_distance(
                q,
                [a[0] / sx, a[1] / sy],
                [b[0] / sx, b[1] / sy],
            )),
            _ => None,
        })
        .fold(f64::INFINITY, f64::min)
}

/// The distance from `q` to the segment `a`-`b`.
fn segment_distance(q: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let len2 = d[0] * d[0] + d[1] * d[1];
    let t = if len2 > 0.0 {
        (((q[0] - a[0]) * d[0] + (q[1] - a[1]) * d[1]) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let c = [a[0] + t * d[0], a[1] + t * d[1]];
    ((q[0] - c[0]).powi(2) + (q[1] - c[1]).powi(2)).sqrt()
}

/// The (ae, ap) spans of the axes; a fixed axis is a span of one value.
const fn spans(axes: &EnvelopeAxes) -> (AxisSpan, AxisSpan) {
    const fn one(v: f64) -> AxisSpan {
        AxisSpan {
            min_mm: v,
            max_mm: v,
        }
    }
    match *axes {
        EnvelopeAxes::Plane { ap, ae } => (ae, ap),
        EnvelopeAxes::AxialBar { ap, ae_mm } => (one(ae_mm), ap),
        EnvelopeAxes::RadialBar { ae, ap_mm } => (ae, one(ap_mm)),
        EnvelopeAxes::Fixed { ap_mm, ae_mm } => (one(ae_mm), one(ap_mm)),
    }
}

/// The line of one limit on the plot, in `[ae, ap]`. `None` for a limit
/// that has no edge on the plane (judged per point, or not modelled).
fn edge_points(l: &Limit, ae: (f64, f64), ap: (f64, f64)) -> Option<Vec<[f64; 2]>> {
    match &l.edge {
        Edge::ApCeiling { mm } | Edge::ApFloor { mm } => Some(vec![[ae.0, *mm], [ae.1, *mm]]),
        Edge::ApCeilingAbove { ae_mm, mm } => {
            Some(vec![[*ae_mm, ap.1], [*ae_mm, *mm], [ae.1, *mm]])
        }
        Edge::ApCurve { points } => (points.len() >= 2).then(|| points.clone()),
        Edge::AeCeiling { mm } | Edge::AeFloor { mm } => Some(vec![[*mm, ap.0], [*mm, ap.1]]),
        Edge::PerPoint | Edge::Unmodelled { .. } => None,
    }
}

/// The hover readout at `p`: one line. Inside, the speeds from the
/// `SpeedsMap` (a lookup); outside, the crossed limit and no speeds (it
/// abstains, as the nomogram readout does).
#[must_use]
pub fn readout(env: &SafeEnvelope, p: CutPoint) -> String {
    let head = format!("ap {:.2} \u{00B7} ae {:.2}", p.ap_mm, p.ae_mm);
    match env.placement_of(p) {
        Placement::Outside { binding, .. } => format!("{head} \u{00B7} {}", short_label(binding)),
        Placement::Inside { .. } => match env.speeds.speeds_at(p.ap_mm, p.ae_mm) {
            Some(s) => format!(
                "{head} \u{00B7} {:.0} rpm \u{00B7} {:.0} mm/min \u{00B7} {:.2} kW",
                s.rpm, s.feed_mm_min, s.power_kw
            ),
            None => head,
        },
    }
}

/// P5b live link: the speeds the nomogram draws for the candidate of a
/// toolpath, a lookup in the `SpeedsMap` at the candidate's (ap, ae). `None`
/// when there is no candidate: the nomogram draws the stored cut.
#[must_use]
pub fn candidate_speeds(env: &SafeEnvelope, candidate: Option<CutPoint>) -> Option<SpeedsSample> {
    let c = candidate?;
    env.speeds.speeds_at(c.ap_mm, c.ae_mm)
}

/// The Unsourced rules that the proposal reads (P5 D4: a `Partial`
/// recommendation): the proposal's basis and the bases of the speeds at
/// the grid point nearest to it.
#[must_use]
pub fn unsourced_rules(env: &SafeEnvelope) -> Vec<String> {
    let p = env.proposal.get();
    let mut out: Vec<String> = Vec::new();
    let mut take = |b: &rs_cam_core::feeds::Basis| {
        for c in b.unsourced_rules() {
            let name = c.name.to_string();
            if !out.contains(&name) {
                out.push(name);
            }
        }
    };
    take(env.proposal.basis());
    let nearest = env.speeds.points.iter().min_by(|a, b| {
        let d = |q: &rs_cam_core::feeds::envelope::SpeedsPoint| {
            (q.ap_mm - p.ap_mm).powi(2) + (q.ae_mm - p.ae_mm).powi(2)
        };
        d(a).total_cmp(&d(b))
    });
    if let Some(q) = nearest {
        take(q.rpm.basis());
        take(q.chip.basis());
        take(q.plunge.basis());
    }
    out
}

/// What [`draw_envelope`] gives back: where the plot is, so a test can aim
/// a click.
#[derive(Clone, Copy, Default)]
pub struct EnvelopeResponse {
    /// The plot transform of the plane plot (screen <-> plot), when one
    /// was drawn.
    pub transform: Option<egui_plot::PlotTransform>,
    /// The screen rect of the plot frame or the bar, when one was drawn.
    pub frame: Option<egui::Rect>,
    /// The width range of the frame (mm).
    pub ae_range: (f64, f64),
    /// The depth range of the frame (mm).
    pub ap_range: (f64, f64),
    /// The ⚡ was clicked this frame.
    pub applied: bool,
}

impl EnvelopeResponse {
    /// The screen position of `p` on the plot or the bar.
    #[must_use]
    pub fn screen_of(&self, p: CutPoint, shape: EnvelopeShape) -> Option<egui::Pos2> {
        let f = self.frame?;
        let tx = |v: f64, r: (f64, f64)| ((v - r.0) / (r.1 - r.0)) as f32;
        match shape {
            EnvelopeShape::Plane => self
                .transform
                .map(|t| t.position_from_point(&egui_plot::PlotPoint::new(p.ae_mm, p.ap_mm))),
            EnvelopeShape::DepthBar => Some(egui::pos2(
                f.left() + tx(p.ap_mm, self.ap_range) * f.width(),
                f.center().y,
            )),
            EnvelopeShape::WidthBar => Some(egui::pos2(
                f.left() + tx(p.ae_mm, self.ae_range) * f.width(),
                f.center().y,
            )),
            EnvelopeShape::None => None,
        }
    }
}

/// The envelope section of the Passes tab: the header with the ⚡ chip, then
/// the plot or the bar.
///
/// - `envelope`: the memo's envelope (`FeedsMemo::envelope`); `None` with
///   a `refusal`: the chip is disabled and the refusal is its hover (an
///   `Unbacked` cell has no numbers to apply; the job still generates).
/// - `candidates`: the candidates (GUI state).
pub fn draw_envelope(
    ui: &mut egui::Ui,
    toolpath_id: rs_cam_core::ToolpathId,
    operation: &OperationConfig,
    envelope: Option<&SafeEnvelope>,
    refusal: Option<&str>,
    candidates: &mut EnvelopeCandidates,
    events: &mut Vec<AppEvent>,
) -> EnvelopeResponse {
    let Some(env) = envelope else {
        if let Some(reason) = refusal {
            // No envelope: the title follows the op's own cut (K1).
            let title = match operation.cut_spec().radial {
                Radial::Width(_) | Radial::Scallop(_) => TITLE_PLANE,
                Radial::Slot
                | Radial::Groove
                | Radial::AngularStep(_)
                | Radial::SurfaceFollow
                | Radial::Axial => TITLE_DEPTH,
            };
            let _ = header(ui, title, None, reason);
        }
        return EnvelopeResponse::default();
    };
    let shape = EnvelopeShape::of(&env.axes);
    let Some(title) = shape.title() else {
        return EnvelopeResponse::default();
    };
    // Esc drops the candidate (live-link ruling).
    if candidates.get(toolpath_id).is_some()
        && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
    {
        let _ = candidates.clear(toolpath_id);
    }
    let scene = EnvelopeScene::of(env, candidates.get(toolpath_id));
    let peck = matches!(operation.cut_spec().axial, Axial::Peck(_));
    let unsourced = unsourced_rules(env);
    let chip_color = if unsourced.is_empty() && !scene.candidate_outside() {
        tokens::ACCENT
    } else {
        tokens::CAUTION
    };
    let hover = apply_hover(&scene, &unsourced);
    let applied = header(ui, title, Some(chip_color), &hover);
    if applied {
        events.push(AppEvent::ApplyFeedsPick {
            toolpath_id,
            pick: scene.pick(peck),
        });
    }
    let (frame, transform) = match shape {
        EnvelopeShape::Plane => {
            let t = draw_plane(ui, toolpath_id, env, &scene, candidates);
            (Some(*t.frame()), Some(t))
        }
        EnvelopeShape::DepthBar | EnvelopeShape::WidthBar => {
            (draw_bar(ui, toolpath_id, env, &scene, candidates), None)
        }
        EnvelopeShape::None => (None, None),
    };
    EnvelopeResponse {
        transform,
        frame,
        ae_range: scene.ae_range,
        ap_range: scene.ap_range,
        applied,
    }
}

/// The hover of the ⚡ chip: what it writes, and the Unsourced rules.
fn apply_hover(scene: &EnvelopeScene, unsourced: &[String]) -> String {
    let what = match scene.candidate {
        Some(c) => format!(
            "Apply ap {:.2} \u{00D7} ae {:.2} and its speeds.",
            c.ap_mm, c.ae_mm
        ),
        None => "Apply the proposal and its speeds.".to_owned(),
    };
    let outside = match scene.candidate_placement {
        Some(Placement::Outside { binding, .. }) => {
            format!(" Outside: {}, a warning.", short_label(binding))
        }
        Some(Placement::Inside { .. }) | None => String::new(),
    };
    let rules = if unsourced.is_empty() {
        String::new()
    } else {
        format!(" Unsourced: {}.", unsourced.join(", "))
    };
    format!("{what}{outside}{rules}")
}

/// The header row: the small-caps title and the ⚡ chip at the right. A
/// `None` colour draws the chip disabled, with `hover` as the reason.
/// True when the chip was clicked.
fn header(ui: &mut egui::Ui, title: &str, chip: Option<egui::Color32>, hover: &str) -> bool {
    ui.add_space(tokens::SPACE_2);
    ui.horizontal(|ui| {
        let _ = ui.add(egui::Label::new(small_caps(title)).wrap_mode(egui::TextWrapMode::Truncate));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Clear of the inspector's scroll bar.
            ui.add_space(tokens::SPACE_2);
            let color = chip.unwrap_or(tokens::TEXT_FAINT);
            let button =
                egui::Button::new(egui::RichText::new(APPLY_GLYPH).color(color)).frame(false);
            let response = ui.add_enabled(chip.is_some(), button);
            let response = if chip.is_some() {
                response.on_hover_text(hover)
            } else {
                response.on_disabled_hover_text(hover)
            };
            response.clicked()
        })
        .inner
    })
    .inner
}

/// The small-caps text of a section title (the kit's `SectionHeader`
/// style; the header row also holds the chip).
fn small_caps(title: &str) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        &title.to_uppercase(),
        0.0,
        egui::TextFormat {
            font_id: egui::FontId::proportional(tokens::SIZE_CAPTION),
            color: tokens::TEXT_FAINT,
            extra_letter_spacing: tokens::SIZE_CAPTION * 0.07,
            ..Default::default()
        },
    );
    job
}

/// The plane plot. Returns the plot transform.
fn draw_plane(
    ui: &mut egui::Ui,
    toolpath_id: rs_cam_core::ToolpathId,
    env: &SafeEnvelope,
    scene: &EnvelopeScene,
    candidates: &mut EnvelopeCandidates,
) -> egui_plot::PlotTransform {
    let overhang = axis_title_overhang(ui, DEPTH_AXIS_TITLE);
    let (ae_range, ap_range) = (scene.ae_range, scene.ap_range);
    let xy = |p: CutPoint| {
        let p = scene.on_frame(p);
        vec![[p.ae_mm, p.ap_mm]]
    };
    let response = ui
        .horizontal(|ui| {
            ui.add_space(overhang);
            // The id is unique per toolpath, and the bounds are set on
            // every frame: egui_plot keeps a bound per plot id (the
            // nomogram's bounds trap).
            Plot::new(("envelope_plot", toolpath_id.raw()))
                .height(PLOT_HEIGHT)
                // P5c: the width ticks are a few characters wide. The
                // default spacing of an x label (60 points) hid every width
                // tick at the 240-point rail; 45 points keeps the 2 mm
                // ticks and hides the 1 mm ones.
                .custom_x_axes(vec![
                    egui_plot::AxisHints::new_x()
                        .label(WIDTH_AXIS_TITLE)
                        .formatter(tick_text)
                        .label_spacing(X_LABEL_SPACING),
                ])
                .y_axis_label(DEPTH_AXIS_TITLE)
                .y_axis_formatter(tick_text)
                .y_axis_min_width(Y_AXIS_MIN_WIDTH)
                .allow_drag(false)
                .allow_zoom(false)
                .allow_scroll(false)
                .allow_boxed_zoom(false)
                .allow_axis_zoom_drag(false)
                .allow_double_click_reset(false)
                .show_x(false)
                .show_y(false)
                // A mm axis of a few units: the default log-10 grid names
                // only 0 on it. A 1-2-5 step gives three or four names.
                .x_grid_spacer(egui_plot::uniform_grid_spacer(|i| nice_steps(&i)))
                .y_grid_spacer(egui_plot::uniform_grid_spacer(|i| nice_steps(&i)))
                // P5c round 2: the grid in `tokens::BORDER`, under the limit
                // lines (`tokens::TEXT_BODY`, 2 points). The default grid
                // takes the text colour, and its strongest lines were
                // brighter than the limits.
                .grid_color(tokens::BORDER)
                .sense(egui::Sense::click_and_drag())
                .show(ui, |plot_ui| {
                    plot_ui.set_plot_bounds_x(ae_range.0..=ae_range.1);
                    plot_ui.set_plot_bounds_y(ap_range.0..=ap_range.1);
                    for edge in &scene.edges {
                        plot_ui.line(
                            Line::new("", PlotPoints::from(edge.points.clone()))
                                .color(scene.edge_color(edge))
                                .width(LINE_WIDTH)
                                .allow_hover(false),
                        );
                    }
                    if scene.proposal_shown {
                        plot_ui.points(
                            Points::new("", xy(scene.proposal))
                                .shape(MarkerShape::Diamond)
                                .filled(false)
                                .radius(PROPOSAL_RADIUS)
                                .color(scene.proposal_color())
                                .allow_hover(false),
                        );
                    }
                    if let Some(g) = scene.ghost() {
                        plot_ui.points(
                            Points::new("", xy(g))
                                .shape(MarkerShape::Circle)
                                .filled(false)
                                .radius(GHOST_RADIUS)
                                .color(tokens::TEXT_MUTED)
                                .allow_hover(false),
                        );
                    }
                    if let (Some(p), Some(color)) = (scene.setpoint(), scene.dot_color()) {
                        plot_ui.points(
                            Points::new("", xy(p))
                                .shape(MarkerShape::Circle)
                                .filled(true)
                                .radius(DOT_RADIUS)
                                .color(color)
                                .allow_hover(false),
                        );
                    }
                })
        })
        .inner;
    let transform = response.transform;
    let frame = *transform.frame();
    // The pointer as a point of the view: a pointer past the frame (a drag
    // that left the plot) is held on the frame edge.
    let to_point = |pos: egui::Pos2| {
        let v = transform.value_from_position(pos);
        CutPoint {
            ap_mm: v.y.clamp(ap_range.0, ap_range.1),
            ae_mm: v.x.clamp(ae_range.0, ae_range.1),
        }
    };
    // A click or a drag moves the candidate (no write). egui keeps the
    // drag on the plot until the release, wherever the pointer goes.
    if (response.response.clicked() || response.response.dragged())
        && let Some(pos) = response.response.interact_pointer_pos()
    {
        candidates.set(toolpath_id, to_point(pos));
    }

    // The labels (P5c round 2: the operator could not tell the lines apart
    // without them): the short name of each drawn limit on its line, in the
    // colour of the line, clear of the marks. The plot draws at most two
    // lines, so it paints at most two labels.
    let at_screen =
        |p: [f64; 2]| transform.position_from_point(&egui_plot::PlotPoint::new(p[0], p[1]));
    let mark_rect = |p: CutPoint, r: f32| {
        let p = scene.on_frame(p);
        egui::Rect::from_center_size(
            at_screen([p.ae_mm, p.ap_mm]),
            egui::Vec2::splat(2.0 * r + MARK_CLEARANCE),
        )
    };
    let mut keep_clear: Vec<egui::Rect> = Vec::new();
    keep_clear.extend(scene.setpoint().map(|p| mark_rect(p, DOT_RADIUS)));
    keep_clear.extend(scene.ghost().map(|p| mark_rect(p, GHOST_RADIUS)));
    if scene.proposal_shown {
        keep_clear.push(mark_rect(scene.proposal, PROPOSAL_RADIUS));
    }
    let screen_edges: Vec<(&SceneEdge, Vec<egui::Pos2>)> = scene
        .edges
        .iter()
        .map(|e| (e, e.points.iter().copied().map(at_screen).collect()))
        .collect();
    let lines: Vec<DrawnLine> = screen_edges
        .iter()
        .map(|(_, pts)| DrawnLine {
            points: pts.clone(),
            half_width: LINE_WIDTH / 2.0,
            soft: false,
        })
        .collect();
    let specs: Vec<ChartLabel> = screen_edges
        .into_iter()
        .map(|(e, pts)| ChartLabel {
            text: short_label(e.kind),
            color: scene.edge_color(e),
            anchor: LabelAnchor::Line(pts),
        })
        .collect();
    let placed = place_labels(ui, frame, &keep_clear, &lines, specs);
    paint_labels(ui, frame, &placed);

    // The hover: the name of the limit line near the pointer, then the
    // readout line.
    if let Some(pos) = response.response.hover_pos()
        && frame.contains(pos)
    {
        let at =
            |p: &[f64; 2]| transform.position_from_point(&egui_plot::PlotPoint::new(p[0], p[1]));
        let near = scene
            .edges
            .iter()
            .map(|e| {
                let pts: Vec<egui::Pos2> = e.points.iter().map(at).collect();
                let d = pts
                    .windows(2)
                    .filter_map(|w| match w {
                        [a, b] => Some(screen_segment_distance(pos, *a, *b)),
                        _ => None,
                    })
                    .fold(f32::INFINITY, f32::min);
                (d, e.kind)
            })
            .filter(|(d, _)| *d <= HOVER_NEAR_PX)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, kind)| short_label(kind));
        let text = readout(env, to_point(pos));
        let _ = response.response.on_hover_ui_at_pointer(|ui| {
            if let Some(name) = near {
                ui.add(
                    egui::Label::new(egui::RichText::new(name).small().color(tokens::TEXT_BODY))
                        .extend(),
                );
            }
            ui.add(
                egui::Label::new(egui::RichText::new(text).small().color(tokens::TEXT_STRONG))
                    .extend(),
            );
        });
    }
    transform
}

/// The text of an axis tick: a whole number with no decimal, else one
/// decimal. The default egui_plot text rounded the 2.5 mm tick to "2"
/// (P5c screenshot).
fn tick_text(mark: egui_plot::GridMark, _range: &std::ops::RangeInclusive<f64>) -> String {
    let v = mark.value;
    if (v - v.round()).abs() < 1e-6 {
        format!("{v:.0}")
    } else {
        format!("{v:.1}")
    }
}

/// The distance from `q` to the screen segment `a`-`b`, in points.
fn screen_segment_distance(q: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let d = b - a;
    let len2 = d.length_sq();
    let t = if len2 > 0.0 {
        ((q - a).dot(d) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    q.distance(a + t * d)
}

/// The one-value bar: a line from the low end to the binding limit, the
/// limit named at its end, the marks on it (the rules of the plot: Now
/// dot at the setpoint draft, ghost at the stored point while a candidate
/// exists, proposal diamond when it is not at Now). Returns the bar rect.
fn draw_bar(
    ui: &mut egui::Ui,
    toolpath_id: rs_cam_core::ToolpathId,
    env: &SafeEnvelope,
    scene: &EnvelopeScene,
    candidates: &mut EnvelopeCandidates,
) -> Option<egui::Rect> {
    let depth = scene.shape == EnvelopeShape::DepthBar;
    let range = if depth {
        scene.ap_range
    } else {
        scene.ae_range
    };
    let value = |p: CutPoint| if depth { p.ap_mm } else { p.ae_mm };
    // The fixed value of the bar: the subject's.
    let subject = scene
        .candidate
        .or(scene.dot.map(|(p, _)| p))
        .unwrap_or(scene.proposal);
    let fixed = if depth { subject.ae_mm } else { subject.ap_mm };
    // The end of the bar: the lowest ceiling of the free axis.
    let end = if depth {
        env.ap_ceiling_at(fixed)
    } else {
        env.boundary
            .iter()
            .filter_map(|l| ae_ceiling_mm(l).map(|mm| (mm, l.kind)))
            .min_by(|a, b| a.0.total_cmp(&b.0))
    };
    // The colour rule of the plot: the end is neutral, amber when the
    // subject crosses it.
    let crossed = end.is_some_and(|(mm, _)| value(subject) > mm);
    let end_color = if crossed {
        tokens::CAUTION
    } else {
        tokens::TEXT_BODY
    };
    let font = egui::FontId::proportional(tokens::SIZE_CAPTION);
    let label = end.map(|(_, kind)| short_label(kind));
    let label_width = label.map_or(0.0, |t| {
        ui.painter()
            .layout_no_wrap(t.to_owned(), font.clone(), end_color)
            .size()
            .x
            + tokens::SPACE_2
    });
    let width = ui.available_width();
    let (row, response) =
        ui.allocate_exact_size(egui::vec2(width, BAR_HEIGHT), egui::Sense::click_and_drag());
    let bar = egui::Rect::from_min_max(
        row.min + egui::vec2(BAR_END_ROOM, 0.0),
        egui::pos2(row.right() - label_width - BAR_END_ROOM, row.bottom()),
    );
    let x_of = |v: f64| {
        let t = ((v - range.0) / (range.1 - range.0)).clamp(0.0, 1.0) as f32;
        bar.left() + t * bar.width()
    };
    let v_of = |x: f32| {
        let t = f64::from(((x - bar.left()) / bar.width()).clamp(0.0, 1.0));
        range.0 + t * (range.1 - range.0)
    };
    let point_at = |x: f32| {
        let v = v_of(x);
        if depth {
            CutPoint {
                ap_mm: v,
                ae_mm: fixed,
            }
        } else {
            CutPoint {
                ap_mm: fixed,
                ae_mm: v,
            }
        }
    };
    let painter = ui.painter_at(row);
    let y = bar.center().y;
    let tick = BAR_HEIGHT / 4.0;
    let end_x = end.map_or(bar.right(), |(mm, _)| x_of(mm));
    // The safe span of the free value: a neutral line from the low end to
    // the binding limit.
    let span = egui::Stroke::new(SPAN_WIDTH, tokens::TEXT_MUTED);
    let _ = painter.line_segment([egui::pos2(bar.left(), y), egui::pos2(end_x, y)], span);
    let _ = painter.line_segment(
        [
            egui::pos2(bar.left(), y - tick),
            egui::pos2(bar.left(), y + tick),
        ],
        span,
    );
    if end.is_some() {
        let _ = painter.line_segment(
            [egui::pos2(end_x, y - tick), egui::pos2(end_x, y + tick)],
            egui::Stroke::new(LINE_WIDTH, end_color),
        );
    }
    if let Some(text) = label {
        let _ = painter.text(
            egui::pos2(end_x + tokens::SPACE_2, y),
            egui::Align2::LEFT_CENTER,
            text,
            font,
            end_color,
        );
    }
    if scene.proposal_shown {
        let c = egui::pos2(x_of(value(scene.proposal)), y);
        let r = PROPOSAL_RADIUS;
        let _ = painter.add(egui::Shape::closed_line(
            vec![
                c + egui::vec2(0.0, -r),
                c + egui::vec2(r, 0.0),
                c + egui::vec2(0.0, r),
                c + egui::vec2(-r, 0.0),
            ],
            egui::Stroke::new(OUTLINE_WIDTH, scene.proposal_color()),
        ));
    }
    if let Some(g) = scene.ghost() {
        let _ = painter.circle_stroke(
            egui::pos2(x_of(value(g)), y),
            GHOST_RADIUS,
            egui::Stroke::new(OUTLINE_WIDTH, tokens::TEXT_MUTED),
        );
    }
    if let (Some(p), Some(color)) = (scene.setpoint(), scene.dot_color()) {
        let _ = painter.circle_filled(egui::pos2(x_of(value(p)), y), DOT_RADIUS, color);
    }
    if (response.clicked() || response.dragged())
        && let Some(pos) = response.interact_pointer_pos()
    {
        candidates.set(toolpath_id, point_at(pos.x));
    }
    if let Some(pos) = response.hover_pos() {
        let text = readout(env, point_at(pos.x));
        let _ = response.on_hover_ui_at_pointer(|ui| {
            ui.add(
                egui::Label::new(egui::RichText::new(text).small().color(tokens::TEXT_STRONG))
                    .extend(),
            );
        });
    }
    Some(bar)
}
