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
//!   with its law, its mm and its basis. No item is a filled region
//!   (G-CHARTLINES, decision D2).
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

use egui_plot::{Line, Plot, PlotPoints};
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

/// The spacing range of the depth tick labels, in points: egui_plot 0.37's
/// own y default (`axis.rs`, `AxisHints::new`), set here so the edge mark
/// rule reads the value in force.
const Y_LABEL_SPACING: std::ops::RangeInclusive<f32> = 20.0..=30.0;

/// The spacing range of the grid lines, in points: egui_plot 0.37's own
/// default (`plot.rs`, `Plot::new`), set here so a grid spacer can turn
/// `GridInput::base_step_size` back into plot units per point.
const GRID_SPACING: std::ops::RangeInclusive<f32> = 8.0..=300.0;

/// The width of the binding or crossed limit line, in points (P5c round 2:
/// twice the 1-point grid line, so a limit reads above the grid).
const LINE_WIDTH: f32 = 2.0;

/// The width of every other drawn limit line, in points (P5d D4: still
/// above the 1-point `BORDER` grid, under the binding line).
const MUTED_LINE_WIDTH: f32 = 1.5;

/// The width of a frame edge line (the width D, the hard depth top), in
/// points: the width of a grid line, one step stronger in colour (P5d D5).
const FRAME_EDGE_WIDTH: f32 = 1.0;

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
const DOT_RADIUS: f32 = 3.5;

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

/// The short name of a limit (P5d D4). [`limit_name`] adds the share of D
/// to the vendor depth; this is its name when D is unknown. The hover keeps
/// the full law, the mm and the basis.
#[must_use]
pub const fn short_label(kind: LimitKind) -> &'static str {
    match kind {
        LimitKind::FluteGuard => "Flute guard",
        LimitKind::CuttingLength => "Cut length",
        LimitKind::VendorAxial => "Vendor DOC",
        LimitKind::VendorDepthTable => "Depth table",
        LimitKind::Rigidity => "Rigidity DOC",
        LimitKind::Slot => "Slot DOC",
        LimitKind::Power => "Spindle power",
        LimitKind::Deflection => "Deflection",
        LimitKind::ChipBand => "Chip band",
        LimitKind::RubbingFloor => "Rubbing",
        LimitKind::FeedCap => "Max feed",
        LimitKind::Diameter => "Tool diameter",
        LimitKind::Runtime => "Min width (moves)",
        LimitKind::EngagementFloor => "Min engagement",
    }
}

/// The name of a limit of `env` on every envelope surface: the line label,
/// the hover, the readout, the bar end and the ⚡ hover (P5d D4). The
/// vendor depth names its depth as a share of D ("Vendor DOC 1×D"): the
/// vendor prints a depth per pass, not the tool length.
#[must_use]
pub fn limit_name(env: &SafeEnvelope, kind: LimitKind) -> String {
    let d = env
        .limit(LimitKind::Diameter)
        .and_then(ae_ceiling_mm)
        .filter(|d| d.is_finite() && *d > 0.0);
    // The vendor depth and the vendor depth table (P11b) name their depth
    // as a share of D when the edge has one value.
    let depth = env.limit(kind).and_then(|l| edge_mm(&l.edge));
    match (kind, d, depth) {
        (LimitKind::VendorAxial | LimitKind::VendorDepthTable, Some(d), Some(mm)) => {
            format!("{} {}\u{00D7}D", short_label(kind), trim_decimals(mm / d))
        }
        _ => short_label(kind).to_owned(),
    }
}

/// `v` with at most two decimals and no trailing zero ("1", "0.5").
fn trim_decimals(v: f64) -> String {
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
        | Edge::ApCeilingAbove { mm, .. }
        | Edge::ApFloor { mm }
        | Edge::AeCeiling { mm }
        | Edge::AeFloor { mm } => Some(mm),
        Edge::ApCurve { .. } | Edge::PerPoint | Edge::Unmodelled { .. } => None,
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
const fn is_frame(kind: LimitKind) -> bool {
    match kind {
        LimitKind::Diameter
        | LimitKind::CuttingLength
        | LimitKind::FluteGuard
        | LimitKind::EngagementFloor => true,
        LimitKind::VendorAxial
        | LimitKind::VendorDepthTable
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
    /// The envelope's proposal.
    pub proposal: CutPoint,
    /// The proposal is drawn: it is not at the setpoint (P5d D3).
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
        let shape = EnvelopeShape::of(&env.axes);
        let proposal = env.proposal.get();
        let dot = env.stated.map(|p| {
            let outside = matches!(env.operator, Some(Placement::Outside { .. }));
            (p, outside)
        });
        let now = dot.map(|(p, _)| p);
        let setpoint = candidate.or(now);
        let proposal_shown = setpoint.is_none_or(|p| !same_point(p, proposal));
        let subject = setpoint.unwrap_or(proposal);
        let (ae_range, ap_range, top) = view_ranges(env, proposal, now);

        let lines: Vec<&Limit> = env.boundary.iter().filter(|l| !is_frame(l.kind)).collect();
        let binding = binding_at(&lines, subject);
        // P5d D1: every limit whose edge enters the view draws (the P5c
        // rule drew only a limit near a mark).
        let mut edges: Vec<SceneEdge> = lines
            .iter()
            .filter_map(|l| {
                let points = edge_points(l, ae_range, ap_range)?;
                in_view(&points, ae_range, ap_range).then(|| SceneEdge {
                    kind: l.kind,
                    crossed: l.margin(subject).is_some_and(|m| m < -OUTSIDE_EPS),
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
    proposal: CutPoint,
    now: Option<CutPoint>,
) -> ((f64, f64), (f64, f64), Option<LimitKind>) {
    let (ae_span, ap_span) = spans(&env.axes);
    let positive = |v: f64| (v.is_finite() && v > 0.0).then_some(v);
    let ae_max = env
        .limit(LimitKind::Diameter)
        .and_then(ae_ceiling_mm)
        .and_then(positive)
        .or_else(|| positive(ae_span.max_mm))
        .unwrap_or(1.0);
    let hard: Option<(f64, LimitKind)> = env
        .boundary
        .iter()
        .filter(|l| matches!(l.kind, LimitKind::CuttingLength | LimitKind::FluteGuard))
        .filter_map(|l| ap_ceiling_mm(l).and_then(positive).map(|mm| (mm, l.kind)))
        .min_by(|a, b| a.0.total_cmp(&b.0));
    let hard_top = hard.map(|h| h.0).or_else(|| positive(ap_span.max_mm));
    let under_top = |mm: f64| hard_top.is_none_or(|h| mm < h);
    let tallest = env
        .boundary
        .iter()
        .filter(|l| !is_frame(l.kind))
        .filter_map(|l| match l.edge {
            Edge::ApCeiling { mm } | Edge::ApCeilingAbove { mm, .. } => positive(mm),
            Edge::ApCurve { .. }
            | Edge::ApFloor { .. }
            | Edge::AeCeiling { .. }
            | Edge::AeFloor { .. }
            | Edge::PerPoint
            | Edge::Unmodelled { .. } => None,
        })
        .filter(|mm| under_top(*mm))
        .chain(now.map(|n| n.ap_mm))
        .chain(std::iter::once(proposal.ap_mm))
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
        Placement::Outside { binding, .. } => {
            format!("{head} \u{00B7} {}", limit_name(env, binding))
        }
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
    let hover = apply_hover(env, &scene, &unsourced);
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
fn apply_hover(env: &SafeEnvelope, scene: &EnvelopeScene, unsourced: &[String]) -> String {
    let what = match scene.candidate {
        Some(c) => format!(
            "Apply ap {:.2} \u{00D7} ae {:.2} and its speeds.",
            c.ap_mm, c.ae_mm
        ),
        None => "Apply the proposal and its speeds.".to_owned(),
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
    let hard_top = scene.top.is_some();
    // P5d D5: a 1-2-5 grid (a mm axis of a few units: the default log-10
    // grid names only 0 on it), plus a named mark at the width frame D and
    // at a hard depth top: D (6 mm) is not on a 2.5 mm step. A named mark
    // within one label of the edge goes (lead review: 5 beside 6.35), and
    // the edge names itself in full.
    let ae_edge = ae_range.1;
    let ap_edge = hard_top.then_some(ap_range.1);
    let x_spacer = egui_plot::uniform_grid_spacer(|i| nice_steps(&i));
    let y_spacer = egui_plot::uniform_grid_spacer(|i| nice_steps(&i));
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
                        .formatter(move |m, _| tick_text(m.value, Some(ae_edge)))
                        .label_spacing(X_LABEL_SPACING),
                ])
                .custom_y_axes(vec![
                    egui_plot::AxisHints::new_y()
                        .label(DEPTH_AXIS_TITLE)
                        .formatter(move |m, _| tick_text(m.value, ap_edge))
                        .label_spacing(Y_LABEL_SPACING),
                ])
                .y_axis_min_width(Y_AXIS_MIN_WIDTH)
                .grid_spacing(GRID_SPACING)
                .allow_drag(false)
                .allow_zoom(false)
                .allow_scroll(false)
                .allow_boxed_zoom(false)
                .allow_axis_zoom_drag(false)
                .allow_double_click_reset(false)
                .show_x(false)
                .show_y(false)
                .x_grid_spacer(move |i| {
                    let per_pt = i.base_step_size / f64::from(*GRID_SPACING.start());
                    let room = f64::from(*X_LABEL_SPACING.start());
                    with_edge_mark(x_spacer(i), ae_edge, per_pt, room)
                })
                .y_grid_spacer(move |i| {
                    let per_pt = i.base_step_size / f64::from(*GRID_SPACING.start());
                    let room = f64::from(*Y_LABEL_SPACING.start());
                    let marks = y_spacer(i);
                    match ap_edge {
                        Some(edge) => with_edge_mark(marks, edge, per_pt, room),
                        None => marks,
                    }
                })
                // P5c round 2: the grid in `tokens::BORDER`, under the limit
                // lines. The default grid takes the text colour, and its
                // strongest lines were brighter than the limits.
                .grid_color(tokens::BORDER)
                .sense(egui::Sense::click_and_drag())
                .show(ui, |plot_ui| {
                    // P5d D5: the drawn bounds pass the width frame D, and
                    // the depth top when it is a hard stop, by the half
                    // width of the widest mark: egui_plot then paints the
                    // tick at the edge, and a mark on the edge paints whole.
                    // The clamp and the frame stay at the ranges.
                    let frame = *plot_ui.transform().frame();
                    let x_pad = edge_pad(ae_range, frame.width());
                    let y_pad = if scene.top.is_some() {
                        edge_pad(ap_range, frame.height())
                    } else {
                        0.0
                    };
                    plot_ui.set_plot_bounds_x(ae_range.0..=ae_range.1 + x_pad);
                    plot_ui.set_plot_bounds_y(ap_range.0..=ap_range.1 + y_pad);
                    for edge in frame_edges(scene) {
                        plot_ui.line(
                            Line::new("", PlotPoints::from(edge))
                                .color(tokens::TEXT_FAINT)
                                .width(FRAME_EDGE_WIDTH)
                                .allow_hover(false),
                        );
                    }
                    for edge in scene.drawn_edges(frame.size()) {
                        plot_ui.line(
                            Line::new("", PlotPoints::from(edge.points.clone()))
                                .color(scene.edge_color(edge))
                                .width(scene.edge_width(edge))
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
    let at_screen =
        |p: [f64; 2]| transform.position_from_point(&egui_plot::PlotPoint::new(p[0], p[1]));
    let at_mark = |p: CutPoint| {
        let p = scene.on_frame(p);
        at_screen([p.ae_mm, p.ap_mm])
    };

    // The marks (P5d D3), over the lines: the quiet proposal diamond, the
    // ghost at the stored point, then the white dot.
    let painter = ui.painter_at(frame);
    if scene.proposal_shown {
        let _ = painter.add(diamond(
            at_mark(scene.proposal),
            egui::Stroke::new(RECOMMENDATION_WIDTH, scene.proposal_color()),
        ));
    }
    if let Some(g) = scene.ghost() {
        let _ = painter.circle_stroke(
            at_mark(g),
            GHOST_RADIUS,
            egui::Stroke::new(OUTLINE_WIDTH, tokens::TEXT_MUTED),
        );
    }
    if let (Some(p), Some(color)) = (scene.setpoint(), scene.dot_color()) {
        let _ = painter.circle_filled(at_mark(p), DOT_RADIUS, color);
    }

    // A click or a drag moves the candidate (no write). egui keeps the
    // drag on the plot until the release, wherever the pointer goes.
    if (response.response.clicked() || response.response.dragged())
        && let Some(pos) = response.response.interact_pointer_pos()
    {
        candidates.set(toolpath_id, to_point(pos));
    }

    // The labels (P5d D1, D4): the name of each drawn limit on its line, in
    // the colour of the line, clear of the marks; the binding and crossed
    // lines first. The frame top names its hard stop.
    let mark_rect = |p: CutPoint, r: f32| {
        egui::Rect::from_center_size(at_mark(p), egui::Vec2::splat(2.0 * r + MARK_CLEARANCE))
    };
    let mut keep_clear: Vec<egui::Rect> = Vec::new();
    keep_clear.extend(scene.setpoint().map(|p| mark_rect(p, DOT_RADIUS)));
    keep_clear.extend(scene.ghost().map(|p| mark_rect(p, GHOST_RADIUS)));
    if scene.proposal_shown {
        keep_clear.push(mark_rect(scene.proposal, PROPOSAL_RADIUS));
    }
    let mut drawn = scene.drawn_edges(frame.size());
    drawn.sort_by_key(|e| scene.edge_width(e) < LINE_WIDTH);
    let screen_edges: Vec<(&SceneEdge, Vec<egui::Pos2>)> = drawn
        .into_iter()
        .map(|e| (e, e.points.iter().copied().map(at_screen).collect()))
        .collect();
    let mut lines: Vec<DrawnLine> = screen_edges
        .iter()
        .map(|(e, pts)| DrawnLine {
            points: pts.clone(),
            half_width: scene.edge_width(e) / 2.0,
            soft: false,
        })
        .collect();
    let frame_lines: Vec<Vec<egui::Pos2>> = frame_edges(scene)
        .into_iter()
        .map(|edge| edge.into_iter().map(at_screen).collect())
        .collect();
    lines.extend(frame_lines.iter().map(|points| DrawnLine {
        points: points.clone(),
        half_width: FRAME_EDGE_WIDTH / 2.0,
        soft: false,
    }));
    // A step (the slot depth above a width: two segments) takes its label
    // beside either segment; its chord runs through the plot, not along the
    // line. A level line or a curve takes it along its own run.
    let mut specs: Vec<ChartLabel> = screen_edges
        .into_iter()
        .map(|(e, pts)| ChartLabel {
            text: e.label.clone(),
            color: scene.edge_color(e),
            anchor: if pts.len() == 3 {
                LabelAnchor::Lines(pts.windows(2).map(<[egui::Pos2]>::to_vec).collect())
            } else {
                LabelAnchor::Line(pts)
            },
        })
        .collect();
    // The top edge is the last frame edge when the top is a hard stop.
    if let (Some(name), Some(top)) = (scene.top_label(), frame_lines.get(1)) {
        specs.push(ChartLabel {
            text: name.to_owned(),
            color: tokens::TEXT_MUTED,
            anchor: LabelAnchor::Line(top.clone()),
        });
    }
    let placed = place_labels(ui, frame, &keep_clear, &lines, specs);
    paint_labels(ui, frame, &placed);

    // The hover: the limit line near the pointer (its name, law, mm and
    // basis; a line that runs along the frame too), then the readout line.
    if let Some(pos) = response.response.hover_pos()
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

/// The plot units that put the frame edge `BAR_END_ROOM` points inside a
/// plot `px` points across (P5d D5): the room of the widest mark.
fn edge_pad(range: (f64, f64), px: f32) -> f64 {
    let room = f64::from(BAR_END_ROOM);
    let px = f64::from(px);
    if px > 2.0 * room {
        (range.1 - range.0) * room / (px - room)
    } else {
        0.0
    }
}

/// `marks` with the frame edge `edge` named as a large step (P5d D5):
/// egui_plot names a mark by its step size, so a mark on a fine step at
/// the edge (6 mm on a 0.25 mm step) takes the large step, and an edge off
/// every step gets a mark of its own. A named mark (its step is wider than
/// `room` points) nearer the edge than `room` points goes, so its label
/// cannot touch the edge label (lead review: "5" beside "6.35"); a fine
/// mark keeps its grid line, it has no label. `per_pt` is plot units per
/// point.
fn with_edge_mark(
    mut marks: Vec<egui_plot::GridMark>,
    edge: f64,
    per_pt: f64,
    room: f64,
) -> Vec<egui_plot::GridMark> {
    let step = marks.iter().map(|m| m.step_size).fold(0.0_f64, f64::max);
    if step <= 0.0 || !(per_pt.is_finite() && per_pt > 0.0) {
        return marks;
    }
    let near = |m: &egui_plot::GridMark| (m.value - edge).abs() <= 1e-9 * edge.abs().max(1.0);
    let crowds = |m: &egui_plot::GridMark| {
        m.step_size / per_pt > room && (m.value - edge).abs() / per_pt < room
    };
    marks.retain(|m| !near(m) && !crowds(m));
    marks.push(egui_plot::GridMark {
        value: edge,
        step_size: step,
    });
    marks
}

/// The frame edge lines of the plot (P5d D5): the width D, and the depth
/// top when it is a hard stop (the second line).
fn frame_edges(scene: &EnvelopeScene) -> Vec<Vec<[f64; 2]>> {
    let (ae, ap) = (scene.ae_range, scene.ap_range);
    let mut out = vec![vec![[ae.1, ap.0], [ae.1, ap.1]]];
    if scene.top.is_some() {
        out.push(vec![[ae.0, ap.1], [ae.1, ap.1]]);
    }
    out
}

/// The proposal diamond at `c`: an outline, no fill.
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

/// The text of an axis tick: a whole number with no decimal, else one
/// decimal. The default egui_plot text rounded the 2.5 mm tick to "2"
/// (P5c screenshot). The frame edge `edge` names itself with up to two
/// decimals ("6.35", lead review of D5).
fn tick_text(v: f64, edge: Option<f64>) -> String {
    if edge.is_some_and(|e| (v - e).abs() <= 1e-9 * e.abs().max(1.0)) {
        return trim_decimals(v);
    }
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
    if scene.proposal_shown {
        let _ = painter.add(diamond(
            egui::pos2(x_of(value(scene.proposal)), y),
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
