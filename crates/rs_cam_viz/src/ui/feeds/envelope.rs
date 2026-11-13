//! The safe envelope plot of the Passes tab (feeds contract P5b, K2b; P5c
//! and P5d operator reviews).
//!
//! The P5 `SafeEnvelope` of the selected op is a region of the (depth,
//! width) plane. This file draws it in the inspector, above the depth and
//! stepover rows (operator ruling 2026-11-03, D1):
//!
//! - A plane op gets a small plot: the width (ae) across, the depth (ap)
//!   up. The frame is the hard stops (P5c): the width axis ends at the tool
//!   diameter, and the depth axis never goes past the cutting length or the
//!   flute guard. P5d: every limit whose edge enters the view draws a line
//!   with its short name (`EnvelopeScene::drawn_edges`); the depth range is
//!   large enough to hold the flat depth limits. The hover names a limit
//!   with its law, its mm and its basis. P17 (operator 2026-11-06, "the
//!   safe region as a soft fill"): the safe region is ONE soft fill in the
//!   histogram's `WASH_OK` behind every line; it overrides G-CHARTLINES
//!   (decision D2, lines only) for this plot alone. The nomogram keeps
//!   lines only.
//! - P17: the plot paints through the chart kit
//!   (`components::chart::PlaneChart`), in the histogram's visual language,
//!   and names no `egui_plot`. The width axis reads as a share of D
//!   (`0..=100 %`, the frame edge "100 %"); the scene stays in mm.
//! - An op with one free value (a Trace or Project Curve depth, a drill
//!   peck, a surface width) gets a bar of one row: it ends at the binding
//!   limit, and the limit is named at its end.
//! - An op with no free value draws nothing.
//!
//! The marks (P5d D3, one rule on both charts): the white dot is the
//! setpoint that the ⚡ writes. A click or a drag moves the dot itself:
//! while a candidate exists, the dot sits at the candidate (a draft of the
//! setpoint) and a hollow ghost marks the stored point. Esc returns the dot
//! to the stored point. The recommendation is quiet ACCENT
//! ([`recommendation_color`], 1 pt): the proposal diamond, drawn when the
//! proposal is not at the setpoint. A point outside warns; nothing refuses
//! it (ruling Q6).
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

use rs_cam_core::compute::catalog::{Axial, OperationConfig, Radial};
use rs_cam_core::feeds::envelope::{
    AxisSpan, CutPoint, Edge, EnvelopeAxes, FramePart, Limit, LimitKind, PickRequest, Placement,
    SafeEnvelope, SpeedsSample,
};

use crate::state::panels::EnvelopeCandidates;
use crate::ui::components::chart::{
    self, Axis, ChartLine, ChartMap, Label, LabelAt, LabelGround, Mark, PlaneChart,
};
use crate::ui::components::histogram::WASH_OK;
use crate::ui::{AppEvent, tokens};

/// The height of the plane plot, the gutters included (P17, lead review:
/// 160, so the frame is about 115 points, the height the egui_plot frame
/// had). Smaller than the nomogram (180): the two charts share the Passes
/// tab with the op rows. The sim chart (`ui/sim_cut_chart.rs`) reads the
/// `pub(crate)` items of this file, so the two planes draw alike.
pub(crate) const PLOT_HEIGHT: f32 = 160.0;

/// The height of the one-value bar.
const BAR_HEIGHT: f32 = 18.0;

/// The title of the depth axis, turned at the left of the plot (the kit's
/// small caps: upper case, the unit in its own case).
pub const DEPTH_AXIS_TITLE: &str = "DEPTH (mm)";

/// The title of the width axis: the width as a share of D (P17).
pub const WIDTH_AXIS_TITLE: &str = "WIDTH (% OF D)";

/// The name of the width frame edge, the tool diameter (P17: the axis is
/// a share of D, so the edge is always "100 %").
pub const WIDTH_EDGE_TEXT: &str = "100 %";

/// The columns of the safe-region fill across the width range (P17).
const FILL_COLUMNS: usize = 64;

/// The width of the binding or crossed limit line, in points (P5c round 2:
/// twice the 1-point reference line, so a limit reads above it).
pub(crate) const LINE_WIDTH: f32 = 2.0;

/// The width of every other drawn limit line, in points (P5d D4: still
/// above the 1-point reference line, under the binding line).
const MUTED_LINE_WIDTH: f32 = 1.5;

/// The width of a frame edge line (the width D, the hard depth top), in
/// points: the width of a reference line, one step stronger in colour (P5d
/// D5).
pub(crate) const FRAME_EDGE_WIDTH: f32 = 1.0;

/// The width of a recommendation mark outline, in points (P5d D3): the
/// width of the faint vendor range lines of the nomogram.
pub const RECOMMENDATION_WIDTH: f32 = super::explore::BAND_LIMIT_WIDTH;

/// The colour of a recommendation mark on both charts (P5d D3): ACCENT at
/// the alpha of the faint vendor range lines of the nomogram. The
/// operator's own point is the strong mark.
#[must_use]
pub fn recommendation_color() -> egui::Color32 {
    super::shared::wash(tokens::ACCENT, super::explore::BAND_LIMIT_ALPHA)
}

/// The width of the outline of the ghost, in points (both charts).
pub const OUTLINE_WIDTH: f32 = 1.5;

/// The radius of the hollow ghost at the stored point, in points (both
/// charts, P5d D3).
pub const GHOST_RADIUS: f32 = 4.5;

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
pub(crate) const DOT_RADIUS: f32 = 3.5;

/// The room of the view past the tallest flat depth limit and the marks, a
/// factor (a layout choice: that limit sits at about three quarters of the
/// axis).
pub const VIEW_HEADROOM: f64 = 1.3;

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

/// The radius of a planned-cut mark (half its side), in points (P29-10
/// 10g): under the operator's dot, over a heat cell's corner.
pub const PLANNED_RADIUS: f32 = 3.0;

/// The outline of a planned-cut mark, in points: the muted line width.
pub const PLANNED_WIDTH: f32 = 1.0;

/// A hover takes a planned mark within this distance, in points.
const MARK_HIT_PX: f32 = 6.0;

/// The short name of a limit (P5d D4). [`limit_name`] adds the share of D
/// to the vendor depth; this is its name when D is unknown. The hover keeps
/// the full law, the mm and the basis. P29-10 10g (lead ruling R4): core
/// writes the name (`LimitId::text().chart`, read through
/// `LimitKind::chart_name`).
#[must_use]
pub const fn short_label(kind: LimitKind) -> &'static str {
    kind.chart_name()
}

/// The name of a limit of `env` on every envelope surface: the line label,
/// the hover, the readout, the bar end and the ⚡ hover (P5d D4). The
/// vendor depth names its depth as a share of D ("Vendor DOC 1×D"): the
/// vendor prints a depth per pass, not the tool length.
#[must_use]
pub fn limit_name(env: &SafeEnvelope, kind: LimitKind) -> String {
    let d = diameter_mm(env);
    // The vendor depth and the vendor depth table (P11b) name their depth
    // as a share of D when the edge has one value.
    let depth = env.limit(kind).and_then(|l| edge_mm(&l.edge));
    match (kind.names_depth_in_diameters(), d, depth) {
        (true, Some(d), Some(mm)) => {
            format!("{} {}\u{00D7}D", short_label(kind), trim_decimals(mm / d))
        }
        (true, _, _) | (false, _, _) => short_label(kind).to_owned(),
    }
}

/// The tool diameter of `env`, the width frame D, when it has one.
fn diameter_mm(env: &SafeEnvelope) -> Option<f64> {
    env.limit(LimitKind::Diameter)
        .and_then(ae_ceiling_max_mm)
        .filter(|d| d.is_finite() && *d > 0.0)
}

/// P19b: the widest value of an `AeCeiling` or a ceiling `AeCurve` (the
/// engaged diameter of a ball, a bull nose, a V-bit or a taper at its
/// deepest sample), for the width frame.
fn ae_ceiling_max_mm(l: &Limit) -> Option<f64> {
    match &l.edge {
        Edge::AeCeiling { mm } => Some(*mm),
        Edge::AeCurve {
            bound: rs_cam_core::feeds::envelope::CurveBound::Ceiling,
            points,
        } => points.iter().map(|p| p[1]).reduce(f64::max),
        Edge::AeCurve {
            bound: rs_cam_core::feeds::envelope::CurveBound::Floor,
            ..
        }
        | Edge::ApCeiling { .. }
        | Edge::ApCurve { .. }
        | Edge::ApFloor { .. }
        | Edge::AeFloor { .. }
        | Edge::PerPoint
        | Edge::Unmodelled { .. } => None,
    }
}

/// `v` with at most two decimals and no trailing zero ("1", "0.5").
pub(crate) fn trim_decimals(v: f64) -> String {
    let text = format!("{v:.2}");
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text
    }
}

/// The mm of an edge that has one value.
const fn edge_mm(edge: &Edge) -> Option<f64> {
    match *edge {
        Edge::ApCeiling { mm }
        | Edge::ApFloor { mm }
        | Edge::AeCeiling { mm }
        | Edge::AeFloor { mm } => Some(mm),
        Edge::ApCurve { .. } | Edge::AeCurve { .. } | Edge::PerPoint | Edge::Unmodelled { .. } => {
            None
        }
    }
}

/// The basis of a limit in a few words, for the hover.
fn basis_text(basis: &rs_cam_core::feeds::Basis) -> String {
    use rs_cam_core::feeds::Basis;
    match basis {
        Basis::Operator { law } | Basis::Derived { law, .. } => (*law).to_owned(),
        Basis::Printed(row) => format!("vendor row {}", row.id()),
        Basis::Claim(claim) => claim.rule.clone(),
        Basis::Rule(rule) => rule.name.to_string(),
        Basis::Machine(field) => field.name().to_owned(),
        Basis::ToolFact(fact) => match fact.legacy_row() {
            Some(row) => format!(
                "{} ({}, plans at {})",
                fact.fact().name(),
                fact.kind().label(),
                row.name
            ),
            None => format!("{} ({})", fact.fact().name(), fact.kind().label()),
        },
    }
}

/// The hover line of one limit: its name, its law, its mm and its basis.
fn limit_hover(env: &SafeEnvelope, l: &Limit) -> String {
    let mm = edge_mm(&l.edge).map_or(String::new(), |mm| format!(" \u{00B7} {mm:.2} mm"));
    format!(
        "{} \u{00B7} {}{mm} \u{00B7} {}",
        limit_name(env, l.kind),
        l.law,
        basis_text(&l.basis)
    )
}

/// A limit that is the plot frame, not a line in it (P5c): the diameter
/// ends the width axis, the cutting length and the flute guard end the
/// depth axis, and the engagement floor is the origin.
/// P29-10 10g: core says which limit is a part of the frame
/// (`LimitKind::frame`).
const fn is_frame(kind: LimitKind) -> bool {
    kind.frame().is_some()
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
    /// The label on the line ([`limit_name`]).
    pub label: String,
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
    /// the flute guard (P5d D2).
    pub ap_range: (f64, f64),
    /// The hard stop at the top of the depth range, when the range reaches
    /// it (P5d D2): the frame top then carries its name.
    pub top: Option<LimitKind>,
    /// One line per non-frame limit whose edge enters the view (P5d D1).
    /// [`Self::drawn_edges`] drops a line that runs along the frame.
    pub edges: Vec<SceneEdge>,
    /// The operator's stated point ("Now"), and whether it sits outside.
    pub dot: Option<(CutPoint, bool)>,
    /// The candidate of a click or a drag (GUI state), if one exists.
    pub candidate: Option<CutPoint>,
    /// Where the candidate sits.
    pub candidate_placement: Option<Placement>,
    /// The envelope's proposal. P16: `None` when the envelope proposes no
    /// depth (no force line, or the depth is known after the simulation);
    /// the plot then draws no proposal mark, and the header note
    /// (`SafeEnvelope::notes`) gives the reason.
    pub proposal: Option<CutPoint>,
    /// The proposal is drawn: it exists and it is not at the setpoint (P5d
    /// D3).
    pub proposal_shown: bool,
    /// The limit that binds the subject (the candidate, else Now, else the
    /// proposal): the worst crossed limit when it sits outside, else the
    /// limit with the smallest margin.
    pub binding: Option<LimitKind>,
}

impl EnvelopeScene {
    /// The scene of `env` with the candidate `candidate`.
    #[must_use]
    pub fn of(env: &SafeEnvelope, candidate: Option<CutPoint>) -> Self {
        Self::with_depth_mark(env, candidate, None)
    }

    /// [`Self::of`] with one more mark for the view range (P17, the sim
    /// chart): `depth_mark` is the deepest cut sample, so the depth range
    /// holds every sample (`VIEW_HEADROOM` over it, never past the hard
    /// top, the rule of `view_ranges`). The limit lines and the fill then
    /// draw on that range. The mark moves no dot.
    #[must_use]
    pub(crate) fn with_depth_mark(
        env: &SafeEnvelope,
        candidate: Option<CutPoint>,
        depth_mark: Option<f64>,
    ) -> Self {
        let shape = EnvelopeShape::of(&env.axes);
        let proposal = env.proposal.get().point();
        let dot = env.stated.map(|p| {
            let outside = matches!(env.operator, Some(Placement::Outside { .. }));
            (p, outside)
        });
        let now = dot.map(|(p, _)| p);
        let setpoint = candidate.or(now);
        let proposal_shown = proposal.is_some_and(|q| setpoint.is_none_or(|p| !same_point(p, q)));
        let subject = setpoint.or(proposal);
        let view_mark = match (now, depth_mark.filter(|d| d.is_finite() && *d > 0.0)) {
            (Some(p), Some(d)) => Some(CutPoint {
                ap_mm: p.ap_mm.max(d),
                ae_mm: p.ae_mm,
            }),
            (None, Some(d)) => Some(CutPoint {
                ap_mm: d,
                ae_mm: 0.0,
            }),
            (now, None) => now,
        };
        let (ae_range, ap_range, top) = view_ranges(env, proposal, view_mark);

        let lines: Vec<&Limit> = env.boundary.iter().filter(|l| !is_frame(l.kind)).collect();
        let binding = subject.and_then(|s| binding_at(&lines, s));
        // P5d D1: every limit whose edge enters the view draws (the P5c
        // rule drew only a limit near a mark).
        let mut edges: Vec<SceneEdge> = lines
            .iter()
            .filter_map(|l| {
                let points = edge_points(l, ae_range, ap_range)?;
                in_view(&points, ae_range, ap_range).then(|| SceneEdge {
                    kind: l.kind,
                    crossed: subject
                        .and_then(|s| l.margin(s))
                        .is_some_and(|m| m < -OUTSIDE_EPS),
                    label: limit_name(env, l.kind),
                    points,
                })
            })
            .collect();
        // A crossed line paints over a neutral one.
        edges.sort_by_key(|e| e.crossed);
        Self {
            shape,
            ae_range,
            ap_range,
            top,
            edges,
            dot,
            candidate,
            candidate_placement: candidate.map(|c| env.placement_of(c)),
            proposal,
            proposal_shown,
            binding,
        }
    }

    /// The lines that the plot draws on a frame of `plot` points (P5d D1):
    /// every edge but one that runs within `MARK_CLEARANCE` of one side of
    /// the frame (the move budget at ae 0.043 of 6 mm). That edge is the
    /// axis, not a line in the plot; the hover names it.
    #[must_use]
    pub fn drawn_edges(&self, plot: egui::Vec2) -> Vec<&SceneEdge> {
        let span = |r: (f64, f64)| (r.1 - r.0).max(f64::EPSILON);
        let (w, h) = (f64::from(plot.x), f64::from(plot.y));
        let clear = f64::from(MARK_CLEARANCE);
        self.edges
            .iter()
            .filter(|e| {
                let px: Vec<(f64, f64)> = e
                    .points
                    .iter()
                    .map(|p| {
                        (
                            (p[0] - self.ae_range.0) / span(self.ae_range) * w,
                            (p[1] - self.ap_range.0) / span(self.ap_range) * h,
                        )
                    })
                    .collect();
                let hugs = px.iter().all(|p| p.0 <= clear)
                    || px.iter().all(|p| p.0 >= w - clear)
                    || px.iter().all(|p| p.1 <= clear)
                    || px.iter().all(|p| p.1 >= h - clear);
                !hugs
            })
            .collect()
    }

    /// The candidate sits outside the envelope.
    #[must_use]
    pub const fn candidate_outside(&self) -> bool {
        matches!(self.candidate_placement, Some(Placement::Outside { .. }))
    }

    // The colour rule of the envelope plot and the bar (P5c item 6; P5d D3
    // and D4 after the operator's second review, 2026-11-04):
    //
    // - The white dot is the setpoint (`tokens::TEXT_STRONG`). A drag moves
    //   the dot itself: while a candidate exists the dot sits at the
    //   candidate (a draft of the setpoint), and a hollow ghost
    //   (`tokens::TEXT_MUTED` outline, no fill) marks the stored point.
    // - The recommendation is quiet ACCENT (`recommendation_color`, 1 pt):
    //   the proposal diamond, drawn when the proposal is not at the
    //   setpoint. No blue mark moves during a drag.
    // - The binding limit line is `tokens::TEXT_BODY`, `LINE_WIDTH` (2
    //   points); every other drawn limit is `tokens::TEXT_MUTED`,
    //   `MUTED_LINE_WIDTH`. Each reads above the `tokens::BORDER` grid. A
    //   label takes the colour of its line.
    // - A line that the dot crosses, and the dot when it sits outside, are
    //   `tokens::CAUTION` (amber), `LINE_WIDTH`.
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

    /// The colour of the proposal mark: the quiet recommendation colour.
    #[must_use]
    pub fn proposal_color(&self) -> egui::Color32 {
        recommendation_color()
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

    /// The colour of the line of `edge`, and of its label (P5d D4).
    #[must_use]
    pub fn edge_color(&self, edge: &SceneEdge) -> egui::Color32 {
        if edge.crossed {
            tokens::CAUTION
        } else if Some(edge.kind) == self.binding {
            tokens::TEXT_BODY
        } else {
            tokens::TEXT_MUTED
        }
    }

    /// The width of the line of `edge`, in points (P5d D4).
    #[must_use]
    pub fn edge_width(&self, edge: &SceneEdge) -> f32 {
        if edge.crossed || Some(edge.kind) == self.binding {
            LINE_WIDTH
        } else {
            MUTED_LINE_WIDTH
        }
    }

    /// The label of the frame top, when the depth range reaches a hard stop.
    #[must_use]
    pub const fn top_label(&self) -> Option<&'static str> {
        match self.top {
            Some(kind) => Some(short_label(kind)),
            None => None,
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

    /// The width `ae_mm` as a share of the width range, in percent (P17:
    /// the x axis of the plot). The range is `0..D`, so this is `% of D`.
    #[must_use]
    pub fn ae_share(&self, ae_mm: f64) -> f64 {
        let span = self.ae_range.1 - self.ae_range.0;
        if span > 0.0 {
            (ae_mm - self.ae_range.0) / span * 100.0
        } else {
            0.0
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

/// The view of the plot: `(ae_range, ap_range, top)`. It reads the hard
/// stops, the flat depth limits, the proposal and Now, and never the
/// candidate, so a drag does not move the axes (P5c).
///
/// - Width: `0..D`. The tool diameter is the frame (no cut is wider than a
///   slot). With no diameter limit, the span of the axis.
/// - Depth (P5d D2): `min(hard top, VIEW_HEADROOM x max(every flat depth
///   ceiling of a non-frame limit under the hard top, the marks))`. The
///   hard top is the lower of the cutting length and the flute guard. A
///   curve does not set the range. `top` names the hard stop when the
///   range reaches it.
fn view_ranges(
    env: &SafeEnvelope,
    proposal: Option<CutPoint>,
    now: Option<CutPoint>,
) -> ((f64, f64), (f64, f64), Option<LimitKind>) {
    let (ae_span, ap_span) = spans(&env.axes);
    let positive = |v: f64| (v.is_finite() && v > 0.0).then_some(v);
    let ae_max = env
        .limit(LimitKind::Diameter)
        .and_then(ae_ceiling_max_mm)
        .and_then(positive)
        .or_else(|| positive(ae_span.max_mm))
        .unwrap_or(1.0);
    let hard: Option<(f64, LimitKind)> = env
        .boundary
        .iter()
        .filter(|l| l.kind.frame() == Some(FramePart::DepthTop))
        .filter_map(|l| ap_ceiling_mm(l).and_then(positive).map(|mm| (mm, l.kind)))
        .min_by(|a, b| a.0.total_cmp(&b.0));
    let hard_top = hard.map(|h| h.0).or_else(|| positive(ap_span.max_mm));
    let under_top = |mm: f64| hard_top.is_none_or(|h| mm < h);
    let tallest = env
        .boundary
        .iter()
        .filter(|l| !is_frame(l.kind))
        .filter_map(|l| match l.edge {
            Edge::ApCeiling { mm } => positive(mm),
            Edge::ApCurve { .. }
            | Edge::ApFloor { .. }
            | Edge::AeCeiling { .. }
            | Edge::AeFloor { .. }
            | Edge::AeCurve { .. }
            | Edge::PerPoint
            | Edge::Unmodelled { .. } => None,
        })
        .filter(|mm| under_top(*mm))
        .chain(now.map(|n| n.ap_mm))
        .chain(proposal.map(|p| p.ap_mm))
        .fold(0.0_f64, f64::max);
    let want = positive(tallest * VIEW_HEADROOM);
    let (ap_max, at_top) = match (want, hard_top) {
        (Some(w), Some(h)) if w < h => (w, false),
        (Some(_) | None, Some(h)) => (h, true),
        (Some(w), None) => (w, false),
        (None, None) => (1.0, false),
    };
    let top = at_top.then_some(hard.map(|h| h.1)).flatten();
    ((0.0, ae_max), (0.0, ap_max), top)
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
        Edge::ApCurve { points } => (points.len() >= 2).then(|| points.clone()),
        Edge::AeCeiling { mm } | Edge::AeFloor { mm } => Some(vec![[*mm, ap.0], [*mm, ap.1]]),
        // P19b: the core samples are `[ap, ae]`; the plot is `[ae, ap]`.
        // The line stops at the top of the view, so its end (where the
        // label sits) is in view: a sample above the top is cut at the
        // top, and the last value holds up to the top when the samples end
        // under it.
        Edge::AeCurve { bound, points } => {
            let mut line: Vec<[f64; 2]> = points
                .iter()
                .filter(|p| p[0] <= ap.1)
                .map(|p| [p[1], p[0]])
                .collect();
            let at_top = rs_cam_core::feeds::envelope::ae_curve_at(*bound, points, ap.1)
                .filter(|v| v.is_finite());
            if let Some(ae_top) = at_top
                && line.last().is_some_and(|last| last[1] < ap.1)
            {
                line.push([ae_top, ap.1]);
            }
            (line.len() >= 2).then_some(line)
        }
        Edge::PerPoint | Edge::Unmodelled { .. } => None,
    }
}

/// The hover readout at `p`: one line. Inside, the speeds from the
/// `SpeedsMap` (a lookup); outside, the crossed limit and no speeds (it
/// abstains, as the nomogram readout does). P17: the width carries its
/// share of D ("ae 1.20 mm (20 % D)") when the envelope has a diameter.
#[must_use]
pub fn readout(env: &SafeEnvelope, p: CutPoint) -> String {
    let share = diameter_mm(env).map_or(String::new(), |d| {
        format!(" ({:.0} % D)", p.ae_mm / d * 100.0)
    });
    let head = format!("ap {:.2} mm \u{00B7} ae {:.2} mm{share}", p.ap_mm, p.ae_mm);
    match env.placement_of(p) {
        Placement::Outside { binding, .. } => {
            format!("{head} \u{00B7} {}", limit_name(env, binding))
        }
        Placement::Inside { .. } => match env.speeds.speeds_at(p.ap_mm, p.ae_mm) {
            Some(s) => {
                // P20d: an unmodelled power prints as such, never as 0.
                let power = s.power_kw.map_or_else(
                    || "power not modelled".to_owned(),
                    |kw| format!("{kw:.2} kW"),
                );
                format!(
                    "{head} \u{00B7} {:.0} rpm \u{00B7} {:.0} mm/min \u{00B7} {power}",
                    s.rpm, s.feed_mm_min
                )
            }
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
    // P16: with no proposed depth the nearest point is by width alone.
    let p_ap = p.ap_mm();
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
            p_ap.map_or(0.0, |ap| (q.ap_mm - ap).powi(2)) + (q.ae_mm - p.ae_mm).powi(2)
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
    /// The map of the plane plot (screen <-> plot; P17: the kit's
    /// `ChartMap`, its x in % of D), when one was drawn.
    pub transform: Option<ChartMap>,
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
                .map(|m| m.to_screen(f64::from(tx(p.ae_mm, self.ae_range)) * 100.0, p.ap_mm)),
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
            let _ = header(ui, title, None, reason, &[]);
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
    let hover = apply_hover(env, &scene, &unsourced);
    let applied = header(ui, title, Some(chip_color), &hover, &env.notes);
    if applied {
        events.push(AppEvent::ApplyFeedsPick {
            toolpath_id,
            pick: scene.pick(peck),
        });
    }
    let (frame, transform) = match shape {
        EnvelopeShape::Plane => {
            let m = draw_plane(ui, toolpath_id, env, &scene, candidates);
            (Some(m.frame), Some(m))
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
fn apply_hover(env: &SafeEnvelope, scene: &EnvelopeScene, unsourced: &[String]) -> String {
    let what = match scene.candidate {
        Some(c) => format!(
            "Apply ap {:.2} \u{00D7} ae {:.2} and its speeds.",
            c.ap_mm, c.ae_mm
        ),
        // P29-7 7c, P29-10 10f: the hover reads the binding planned row.
        None => proposal_hover(env),
    };
    let outside = match scene.candidate_placement {
        Some(Placement::Outside { binding, .. }) => {
            format!(" Outside: {}, a warning.", limit_name(env, binding))
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

/// The ⚡ hover with no candidate (P29-7 7c, P29-10 10f): the hover reads
/// the binding planned row, the row of the declared cut that sets the
/// limit that sets the proposal ("The proposal stands on: planner cap ·
/// depth 100 % of force cap."). Core wrote its face; the row detail is on
/// the card's "Limits" drawer.
#[must_use]
pub fn proposal_hover(env: &SafeEnvelope) -> String {
    match binding_planned_row(env) {
        Some(row) => format!(
            "Apply the proposal and its speeds. The proposal stands on: {}.",
            row.face()
        ),
        None => match env.proposal_binding {
            Some(kind) => format!(
                "Apply the proposal and its speeds. The proposal stands on the {}.",
                limit_name(env, kind)
            ),
            None => "Apply the proposal and its speeds.".to_owned(),
        },
    }
}

/// The binding planned row (P29-10 10f): the planned row of the limit that
/// sets the proposal (`SafeEnvelope::proposal_binding`) on the declared cut
/// that sets that limit (`Limit::binding_cut`). `None` with no binding
/// limit, or when no planned row names that limit and cut.
#[must_use]
pub fn binding_planned_row(env: &SafeEnvelope) -> Option<&rs_cam_core::limits::CheckRow> {
    let kind = env.proposal_binding?;
    let cut = env.limit(kind)?.binding_cut?;
    let limit = rs_cam_core::limits::LimitId::of_limit_kind(kind);
    env.rows()
        .iter()
        .find(|row| row.cut() == Some(cut) && row.check().limit == limit)
}

/// The header row: the small-caps title and the ⚡ chip at the right. A
/// `None` colour draws the chip disabled, with `hover` as the reason.
/// True when the chip was clicked.
///
/// P13c (ruling O3): the envelope notes (core `SafeEnvelope::notes`, the
/// text MCP and the CLI print) sit on this row as one caution: the short
/// face of the first note (`EnvelopeNote::face`, P29-10 10b; it fits at 340
/// points) with the whole core text of every note on hover, so the warning
/// adds no row.
fn header(
    ui: &mut egui::Ui,
    title: &str,
    chip: Option<egui::Color32>,
    hover: &str,
    notes: &[rs_cam_core::feeds::envelope::EnvelopeNote],
) -> bool {
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
            if let Some(first) = notes.first() {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    let _ = ui
                        .add(
                            egui::Label::new(
                                egui::RichText::new(format!("⚠ {}", first.face()))
                                    .color(tokens::CAUTION)
                                    .size(tokens::SIZE_CAPTION),
                            )
                            .wrap_mode(egui::TextWrapMode::Truncate),
                        )
                        .on_hover_text(
                            notes
                                .iter()
                                .map(|n| n.text())
                                .collect::<Vec<_>>()
                                .join("\n"),
                        );
                });
            }
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

/// The plane plot, on the chart kit (P17). Returns the chart map.
///
/// The scene is in mm; the kit's x axis is the width as a share of D
/// (`0..=100 %`), so every x goes through [`EnvelopeScene::ae_share`].
fn draw_plane(
    ui: &mut egui::Ui,
    toolpath_id: rs_cam_core::ToolpathId,
    env: &SafeEnvelope,
    scene: &EnvelopeScene,
    candidates: &mut EnvelopeCandidates,
) -> ChartMap {
    let (ae_range, ap_range) = (scene.ae_range, scene.ap_range);
    let sx = |ae: f64| scene.ae_share(ae);
    let pt = |p: [f64; 2]| [sx(p[0]), p[1]];
    let pts = |points: &[[f64; 2]]| points.iter().copied().map(pt).collect::<Vec<_>>();
    let at_mark = |p: CutPoint| {
        let p = scene.on_frame(p);
        [sx(p.ae_mm), p.ap_mm]
    };

    // P5d D5: the width frame D names itself "100 %", and a hard depth top
    // names its mm in full ("6.35", lead review of D5).
    let mut y_axis = Axis::new(ap_range, DEPTH_AXIS_TITLE);
    if scene.top.is_some() {
        y_axis = y_axis.edge(trim_decimals(ap_range.1));
    }
    let mut plot = PlaneChart::new(
        Axis::new((0.0, 100.0), WIDTH_AXIS_TITLE).edge(WIDTH_EDGE_TEXT),
        y_axis,
        PLOT_HEIGHT,
        egui::Sense::click_and_drag(),
    )
    // P17: the safe region, one soft fill behind every line.
    .fill(
        safe_region_quads(env, scene)
            .into_iter()
            .map(|q| q.map(pt))
            .collect(),
        tokens::OK.gamma_multiply(WASH_OK),
    );
    // The frame edges (D5), under the limit lines.
    let frame_lines = frame_edges(scene);
    for edge in &frame_lines {
        plot = plot.line(ChartLine::new(
            pts(edge),
            egui::Stroke::new(FRAME_EDGE_WIDTH, tokens::TEXT_FAINT),
        ));
    }
    // The limit lines (P5d D1, D4): every edge but one that hugs the
    // frame, a test on the frame the kit is about to take.
    let mut drawn = scene.drawn_edges(plot.frame(ui).size());
    for edge in &drawn {
        plot = plot.line(ChartLine::new(
            pts(&edge.points),
            egui::Stroke::new(scene.edge_width(edge), scene.edge_color(edge)),
        ));
    }

    // P29-10 10g: the planned cuts at the proposal, under the D3 marks:
    // each declared cut a hollow square in the tone of its rows.
    // A mark at the operator's dot is under it: the dot names that point,
    // and its hover names the cut (`outside_hover`), so the mark is left out.
    let at_dot = |m: &rs_cam_core::feeds::plane::PlaneMark| {
        scene.setpoint().is_some_and(|p| {
            same_point(
                p,
                CutPoint {
                    ap_mm: m.at.ap_mm,
                    ae_mm: m.at.ae_mm,
                },
            )
        })
    };
    let marks: Vec<_> = planned_marks(env)
        .into_iter()
        .filter(|(m, _)| !at_dot(m))
        .collect();
    for (mark, color) in &marks {
        plot = plot.mark(Mark::square(
            at_mark(CutPoint {
                ap_mm: mark.at.ap_mm,
                ae_mm: mark.at.ae_mm,
            }),
            PLANNED_RADIUS,
            egui::Stroke::new(PLANNED_WIDTH, *color),
        ));
    }
    // The marks (P5d D3), over the lines: the quiet proposal diamond, the
    // ghost at the stored point, then the white dot.
    if let (true, Some(proposal)) = (scene.proposal_shown, scene.proposal) {
        plot = plot.mark(Mark::diamond(
            at_mark(proposal),
            PROPOSAL_RADIUS,
            egui::Stroke::new(RECOMMENDATION_WIDTH, scene.proposal_color()),
        ));
    }
    if let Some(g) = scene.ghost() {
        plot = plot.mark(Mark::ring(
            at_mark(g),
            GHOST_RADIUS,
            egui::Stroke::new(OUTLINE_WIDTH, tokens::TEXT_MUTED),
        ));
    }
    if let (Some(p), Some(color)) = (scene.setpoint(), scene.dot_color()) {
        plot = plot.mark(Mark::dot(at_mark(p), DOT_RADIUS, color));
    }

    // The labels (P5d D1, D4; P17b item 2): the name of each drawn limit
    // ON its line at its end, in the colour of the line, on a chip of the
    // chart ground (the line runs into the text and out the other side),
    // clear of the marks; the binding and crossed lines first. A step
    // gives its polyline whole: the label sits at its last point, else
    // beside one of its segments. The frame top names its hard stop at
    // its right end, last.
    drawn.sort_by_key(|e| scene.edge_width(e) < LINE_WIDTH);
    for edge in &drawn {
        plot = plot.label(
            Label::new(
                edge.label.clone(),
                scene.edge_color(edge),
                LabelAt::line_end(pts(&edge.points)),
            )
            .ground(LabelGround::Ground),
        );
    }
    if let (Some(name), Some(top)) = (scene.top_label(), frame_lines.get(1)) {
        plot = plot.label(
            Label::new(name, tokens::TEXT_MUTED, LabelAt::line_end(pts(top)))
                .ground(LabelGround::Ground),
        );
    }

    let chart::ChartResponse { response, map } = plot.show(ui);
    let frame = map.frame;
    // The pointer as a point of the view: a pointer past the frame (a drag
    // that left the plot) is held on the frame edge.
    let to_point = |pos: egui::Pos2| {
        let (x, y) = map.from_screen(pos);
        CutPoint {
            ap_mm: y.clamp(ap_range.0, ap_range.1),
            ae_mm: (ae_range.0 + x / 100.0 * (ae_range.1 - ae_range.0))
                .clamp(ae_range.0, ae_range.1),
        }
    };
    let at_screen = |p: [f64; 2]| map.to_screen(sx(p[0]), p[1]);

    // A click or a drag moves the candidate (no write). egui keeps the
    // drag on the plot until the release, wherever the pointer goes.
    if (response.clicked() || response.dragged())
        && let Some(pos) = response.interact_pointer_pos()
    {
        candidates.set(toolpath_id, to_point(pos));
    }

    // The hover: the limit line near the pointer (its name, law, mm and
    // basis; a line that runs along the frame too), then the readout line.
    if let Some(pos) = response.hover_pos()
        && frame.contains(pos)
    {
        let near = scene
            .edges
            .iter()
            .map(|e| {
                let pts: Vec<egui::Pos2> = e.points.iter().copied().map(at_screen).collect();
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
            .and_then(|(_, kind)| env.limit(kind))
            .map(|l| limit_hover(env, l));
        // P29-10 10g: a planned mark near the pointer names its cut and its
        // rows; the operator's dot names why it sits outside (ruling R6).
        let on_mark = marks
            .iter()
            .map(|(m, _)| {
                let p = scene.on_frame(CutPoint {
                    ap_mm: m.at.ap_mm,
                    ae_mm: m.at.ae_mm,
                });
                (at_screen([p.ae_mm, p.ap_mm]).distance(pos), m)
            })
            .filter(|(d, _)| *d <= MARK_HIT_PX)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, m)| mark_hover(env, m));
        let on_dot = scene
            .dot
            .filter(|_| scene.candidate.is_none())
            .map(|(p, _)| scene.on_frame(p))
            .filter(|p| at_screen([p.ae_mm, p.ap_mm]).distance(pos) <= MARK_HIT_PX)
            .and_then(|_| outside_hover(env));
        let near = on_dot.or(on_mark).or(near);
        let text = readout(env, to_point(pos));
        let _ = response.on_hover_ui_at_pointer(|ui| {
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
    map
}

/// P29-10 10g: the planned cuts of the plot, at the proposal (the point of
/// the planned rows, `SafeEnvelope::marks_at_proposal`), each with its
/// colour: `CAUTION` when a row of its cut exceeds (either tone; no
/// `DANGER` on this plot, ruling Q6), else `TEXT_MUTED`. The tone is the
/// tone of the rows, never the position of the mark (lead ruling R3). A
/// mark in `CAUTION` comes last, so it paints on top.
#[must_use]
pub fn planned_marks(
    env: &SafeEnvelope,
) -> Vec<(rs_cam_core::feeds::plane::PlaneMark, egui::Color32)> {
    let mut out: Vec<_> = env
        .marks_at_proposal()
        .map(|marks| {
            marks
                .marks()
                .iter()
                .map(|m| (*m, mark_color(env.rows(), m.cut)))
                .collect()
        })
        .unwrap_or_default();
    // Two cuts at one point (a slot and a helix at the tool diameter): the
    // mark of a row that exceeds paints last, on top.
    out.sort_by_key(|(_, color)| *color == tokens::CAUTION);
    out
}

/// The colour of the mark of `cut`: `CAUTION` when one of its planned rows
/// exceeds, else `TEXT_MUTED`.
#[must_use]
pub fn mark_color(
    rows: &[rs_cam_core::limits::CheckRow],
    cut: rs_cam_core::limits::CutKind,
) -> egui::Color32 {
    if rows
        .iter()
        .any(|r| r.cut() == Some(cut) && r.tone().exceeds())
    {
        tokens::CAUTION
    } else {
        tokens::TEXT_MUTED
    }
}

/// The hover of a planned mark: the cut and its point, then each planned
/// row of the cut that holds a number, with its own bound (lead ruling R3:
/// the row is the truth; an entry reads its own chip, a planner cap its own
/// width, so the row's bound can differ from the force line of the body).
#[must_use]
pub fn mark_hover(env: &SafeEnvelope, mark: &rs_cam_core::feeds::plane::PlaneMark) -> String {
    let mut lines = vec![format!(
        "{} \u{00B7} ap {:.2} \u{00D7} ae {:.2} mm",
        mark.cut.label(),
        mark.at.ap_mm,
        mark.at.ae_mm
    )];
    for row in env
        .rows()
        .iter()
        .filter(|r| r.cut() == Some(mark.cut) && r.holds_a_number())
    {
        let check = row.check();
        let bound = check
            .bound
            .map_or(String::new(), |b| format!(" (bound {b:.2} {})", check.unit));
        lines.push(format!("{}{bound}", row.face_in_group()));
    }
    lines.join("\n")
}

/// P29-10 10g (lead ruling R6): the hover of the operator's dot when it
/// sits outside: the limit that places it outside and the declared cut
/// whose ceiling does (`SafeEnvelope::outside_cut`). `None` inside.
#[must_use]
pub fn outside_hover(env: &SafeEnvelope) -> Option<String> {
    let Some(Placement::Outside { binding, .. }) = env.operator else {
        return None;
    };
    Some(match env.outside_cut() {
        Some(cut) => format!(
            "Now is outside: {} of the {}",
            limit_name(env, binding),
            cut.label()
        ),
        None => format!("Now is outside: {}", limit_name(env, binding)),
    })
}

/// The safe region of the plot (P17, operator 2026-11-06): `FILL_COLUMNS`
/// quads in `[ae, ap]` (mm) across `[the largest width floor, D]`. The top
/// of a column is `min(the depth ceiling at its width, the top of the
/// view)`, read from `SafeEnvelope::ap_ceiling_at`; a width with no
/// ceiling reaches the top of the view. The bottom is the largest depth
/// floor, else 0. P19b: a width bound that moves with the depth (an
/// `AeCurve`) raises the bottom of a column to the smallest depth where
/// the column width meets it (`Limit::depth_floor_at_width`); a column
/// that meets it at no depth in view is a quad of no height.
#[must_use]
pub fn safe_region_quads(env: &SafeEnvelope, scene: &EnvelopeScene) -> Vec<[[f64; 2]; 4]> {
    let (ae_range, ap_range) = (scene.ae_range, scene.ap_range);
    let floor_of = |pick: fn(&Edge) -> Option<f64>| {
        env.boundary
            .iter()
            .filter_map(|l| pick(&l.edge))
            .fold(0.0_f64, f64::max)
    };
    let ae_lo = floor_of(|e| match *e {
        Edge::AeFloor { mm } => Some(mm),
        Edge::ApCeiling { .. }
        | Edge::ApFloor { .. }
        | Edge::AeCeiling { .. }
        | Edge::ApCurve { .. }
        | Edge::AeCurve { .. }
        | Edge::PerPoint
        | Edge::Unmodelled { .. } => None,
    })
    .max(ae_range.0);
    let ap_lo = floor_of(|e| match *e {
        Edge::ApFloor { mm } => Some(mm),
        Edge::ApCeiling { .. }
        | Edge::AeFloor { .. }
        | Edge::AeCeiling { .. }
        | Edge::ApCurve { .. }
        | Edge::AeCurve { .. }
        | Edge::PerPoint
        | Edge::Unmodelled { .. } => None,
    })
    .max(ap_range.0);
    let ae_hi = ae_range.1;
    if ae_hi <= ae_lo || !(ae_hi - ae_lo).is_finite() {
        return Vec::new();
    }
    let top_at = |ae: f64| {
        env.ap_ceiling_at(ae)
            .map_or(ap_range.1, |(mm, _)| mm.min(ap_range.1))
            .max(ap_lo)
    };
    let bottom_at = |ae: f64| {
        env.boundary
            .iter()
            .filter_map(|l| l.depth_floor_at_width(ae))
            .fold(ap_lo, f64::max)
            .min(top_at(ae))
    };
    let column = |i: usize| ae_lo + (ae_hi - ae_lo) * i as f64 / FILL_COLUMNS as f64;
    (0..FILL_COLUMNS)
        .map(|i| {
            let (a, b) = (column(i), column(i + 1));
            [
                [a, bottom_at(a)],
                [b, bottom_at(b)],
                [b, top_at(b)],
                [a, top_at(a)],
            ]
        })
        .collect()
}

/// The frame edge lines of the plot (P5d D5): the width D, and the depth
/// top when it is a hard stop (the second line).
pub(crate) fn frame_edges(scene: &EnvelopeScene) -> Vec<Vec<[f64; 2]>> {
    let (ae, ap) = (scene.ae_range, scene.ap_range);
    let mut out = vec![vec![[ae.1, ap.0], [ae.1, ap.1]]];
    if scene.top.is_some() {
        out.push(vec![[ae.0, ap.1], [ae.1, ap.1]]);
    }
    out
}

/// The proposal diamond at `c` on the bar: an outline, no fill.
fn diamond(c: egui::Pos2, stroke: egui::Stroke) -> egui::Shape {
    let r = PROPOSAL_RADIUS;
    egui::Shape::closed_line(
        vec![
            c + egui::vec2(0.0, -r),
            c + egui::vec2(r, 0.0),
            c + egui::vec2(0.0, r),
            c + egui::vec2(-r, 0.0),
        ],
        stroke,
    )
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
/// exists, the quiet proposal diamond when it is not at the setpoint).
/// Returns the bar rect.
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
    // The fixed value of the bar: the subject's. P16: with no proposed
    // depth and no stated point, the proposal width at depth 0 (no mark).
    let subject = scene
        .candidate
        .or(scene.dot.map(|(p, _)| p))
        .or(scene.proposal)
        .unwrap_or(CutPoint {
            ap_mm: 0.0,
            ae_mm: env.proposal.get().ae_mm,
        });
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
    let font = chart::caption_font();
    let label = end.map(|(_, kind)| limit_name(env, kind));
    let label_width = label.as_ref().map_or(0.0, |t| {
        ui.painter()
            .layout_no_wrap(t.clone(), font.clone(), end_color)
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
    // P29-10 10g: the planned cuts on the bar, at their free value, in the
    // tone of their rows (the plot's rule).
    // A mark at the dot is under it (the plot's rule): left out.
    let at_dot = |m: &rs_cam_core::feeds::plane::PlaneMark| {
        scene.setpoint().is_some_and(|p| {
            same_point(
                p,
                CutPoint {
                    ap_mm: m.at.ap_mm,
                    ae_mm: m.at.ae_mm,
                },
            )
        })
    };
    for (mark, color) in planned_marks(env).into_iter().filter(|(m, _)| !at_dot(m)) {
        let v = if depth { mark.at.ap_mm } else { mark.at.ae_mm };
        let _ = painter.rect_stroke(
            egui::Rect::from_center_size(
                egui::pos2(x_of(v), y),
                egui::Vec2::splat(2.0 * PLANNED_RADIUS),
            ),
            0.0,
            egui::Stroke::new(PLANNED_WIDTH, color),
            egui::StrokeKind::Middle,
        );
    }
    if let (true, Some(proposal)) = (scene.proposal_shown, scene.proposal) {
        let _ = painter.add(diamond(
            egui::pos2(x_of(value(proposal)), y),
            egui::Stroke::new(RECOMMENDATION_WIDTH, scene.proposal_color()),
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
