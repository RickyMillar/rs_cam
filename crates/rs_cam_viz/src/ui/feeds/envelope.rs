//! The safe envelope plot of the Passes tab (feeds contract P5b, K2b).
//!
//! The P5 `SafeEnvelope` of the selected op is a region of the (depth,
//! width) plane. This file draws it in the inspector, above the depth and
//! stepover rows (operator ruling 2026-11-03, D1):
//!
//! - A plane op gets a small plot: the width (ae) across, the depth (ap)
//!   up. One line per limit edge, the short limit name on the line. The
//!   edge that binds the ring is solid, the other edges are faint. No item
//!   is a filled region (G-CHARTLINES, decision D2).
//! - An op with one free value (a Trace or Project Curve depth, a drill
//!   peck, a surface width) gets a bar of one row: it ends at the binding
//!   limit, and the limit is named at its end.
//! - An op with no free value draws nothing.
//!
//! The marks (ruling R23): the operator's stated point is a neutral dot;
//! the ring is the proposal (`tokens::ACCENT`), or the candidate when the
//! operator has clicked or dragged one. A point outside the envelope takes
//! `tokens::CAUTION`, and so does the label of the edge it crosses. A point
//! outside warns; nothing refuses it (ruling Q6).
//!
//! The live link (operator ruling 2026-11-03): a click or a drag moves the
//! candidate ring (GUI state, `state::panels::EnvelopeCandidates`); the
//! hover readout and the nomogram read the `SpeedsMap` at the candidate on
//! every frame, a lookup with no Suggest run. Nothing is written until the
//! header ⚡ sends `AppEvent::ApplyFeedsPick`. Esc drops the candidate.
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
    ChartLabel, DrawnLine, LabelAnchor, axis_title_overhang, paint_labels, place_labels, wash,
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

/// The width of the binding edge, in points.
const BINDING_WIDTH: f32 = 1.5;

/// The width of a faint edge, in points.
const FAINT_WIDTH: f32 = 1.0;

/// The alpha of the binding edge.
const BINDING_ALPHA: u8 = 220;

/// The alpha of a faint edge.
const FAINT_ALPHA: u8 = 80;

/// The radius of the ring, in points (the nomogram's `Recommended`).
const RING_RADIUS: f32 = 7.0;

/// The radius of the operator's dot, in points (the nomogram's `Now`).
const DOT_RADIUS: f32 = 3.5;

/// The room a label keeps around a mark, in points.
const MARK_CLEARANCE: f32 = 4.0;

/// The margin of each axis past the envelope span, a fraction of the span.
const AXIS_MARGIN: f64 = 0.05;

/// The room of the view past the lowest ceiling of a free axis, a factor
/// (a layout choice: the ceiling sits at about three quarters of the
/// axis).
const VIEW_HEADROOM: f64 = 1.3;

/// The ⚡ of the header (design rule 8: the Suggest mark).
pub const APPLY_GLYPH: &str = "\u{26A1}";

/// The section title of a plane op.
pub const TITLE_PLANE: &str = "Safe cut";
/// The section title of an op whose free value is the depth.
pub const TITLE_DEPTH: &str = "Safe depth";
/// The section title of an op whose free value is the width.
pub const TITLE_WIDTH: &str = "Safe width";

/// The short name of a limit on the plot (one word where it can be).
#[must_use]
pub const fn short_label(kind: LimitKind) -> &'static str {
    match kind {
        LimitKind::FluteGuard => "Flute",
        LimitKind::CuttingLength => "Cut length",
        LimitKind::VendorAxial => "Vendor",
        LimitKind::Rigidity => "Rigidity",
        LimitKind::Slot => "Slot",
        LimitKind::Power => "Power",
        LimitKind::Deflection => "Deflection",
        LimitKind::ChipBand => "Chip band",
        LimitKind::RubbingFloor => "Rubbing",
        LimitKind::FeedCap => "Max feed",
        LimitKind::Diameter => "Diameter",
        LimitKind::Runtime => "Moves",
        LimitKind::EngagementFloor => "Min",
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

/// One edge of the plot, in plot coordinates `[ae, ap]`.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneEdge {
    /// The limit.
    pub kind: LimitKind,
    /// The line, in `[ae, ap]` (mm).
    pub points: Vec<[f64; 2]>,
    /// The edge binds the ring: it is solid.
    pub binding: bool,
}

/// What the plot draws: a pure function of the envelope and the candidate,
/// so a test reads it without a renderer.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvelopeScene {
    /// The shape of the section.
    pub shape: EnvelopeShape,
    /// The width range of the plot (mm).
    pub ae_range: (f64, f64),
    /// The depth range of the plot (mm).
    pub ap_range: (f64, f64),
    /// One line per drawn limit edge.
    pub edges: Vec<SceneEdge>,
    /// The operator's stated point, and whether it sits outside.
    pub dot: Option<(CutPoint, bool)>,
    /// The ring: the candidate, else the proposal.
    pub ring: CutPoint,
    /// The ring is a candidate (not the proposal).
    pub ring_is_candidate: bool,
    /// Where the ring sits.
    pub ring_placement: Placement,
    /// The limit that binds the ring: the crossed limit when the ring sits
    /// outside, else the limit that lowered the proposal.
    pub binding: Option<LimitKind>,
}

impl EnvelopeScene {
    /// The scene of `env` with the candidate `candidate`.
    #[must_use]
    pub fn of(env: &SafeEnvelope, candidate: Option<CutPoint>) -> Self {
        let shape = EnvelopeShape::of(&env.axes);
        let proposal = env.proposal.get();
        let ring = candidate.unwrap_or(proposal);
        let ring_placement = env.placement_of(ring);
        let binding = match ring_placement {
            Placement::Outside { binding, .. } => Some(binding),
            Placement::Inside { .. } => match candidate {
                Some(c) => binding_at(env, c),
                None => env.proposal_binding,
            },
        };
        let dot = env.stated.map(|p| {
            let outside = matches!(env.operator, Some(Placement::Outside { .. }));
            (p, outside)
        });
        let (ae_span, ap_span) = spans(&env.axes);
        // The plot shows the region where the choice is: up to a share past
        // the lowest ceiling of each free axis and past every mark, never
        // past the span. A ceiling far above the cut (the flute length)
        // then does not squeeze the marks into the corner.
        let marks = [Some(proposal), Some(ring), dot.map(|(p, _)| p)];
        let mark_max =
            |f: fn(&CutPoint) -> f64| marks.iter().flatten().map(f).fold(0.0_f64, f64::max);
        let ap_ceiling = env.ap_ceiling_at(proposal.ae_mm).map_or(0.0, |c| c.0);
        let ae_ceiling = env
            .boundary
            .iter()
            .filter_map(|l| match l.edge {
                Edge::AeCeiling { mm } => Some(mm),
                Edge::ApCeiling { .. }
                | Edge::ApCeilingAbove { .. }
                | Edge::ApCurve { .. }
                | Edge::ApFloor { .. }
                | Edge::AeFloor { .. }
                | Edge::PerPoint
                | Edge::Unmodelled { .. } => None,
            })
            .fold(f64::INFINITY, f64::min);
        let ae_ceiling = if ae_ceiling.is_finite() {
            ae_ceiling
        } else {
            0.0
        };
        let hi = |ceiling: f64, mark: f64, span: f64| {
            let want = (ceiling.max(mark) * VIEW_HEADROOM).max(mark * (1.0 + AXIS_MARGIN));
            if want > 0.0 {
                want.min(span.max(mark) * (1.0 + AXIS_MARGIN))
            } else {
                span * (1.0 + AXIS_MARGIN)
            }
        };
        let ae_range = (0.0, hi(ae_ceiling, mark_max(|p| p.ae_mm), ae_span.max_mm));
        let ap_range = (0.0, hi(ap_ceiling, mark_max(|p| p.ap_mm), ap_span.max_mm));
        let edges = env
            .boundary
            .iter()
            // The engagement floor is the plot origin: no edge, no label.
            .filter(|l| l.kind != LimitKind::EngagementFloor)
            .filter_map(|l| {
                edge_points(l, ae_range, ap_range).map(|points| SceneEdge {
                    kind: l.kind,
                    points,
                    binding: Some(l.kind) == binding,
                })
            })
            .collect();
        Self {
            shape,
            ae_range,
            ap_range,
            edges,
            dot,
            ring,
            ring_is_candidate: candidate.is_some(),
            ring_placement,
            binding,
        }
    }

    /// The ring sits outside the envelope.
    #[must_use]
    pub const fn ring_outside(&self) -> bool {
        matches!(self.ring_placement, Placement::Outside { .. })
    }

    /// The colour of the ring.
    #[must_use]
    pub const fn ring_color(&self) -> egui::Color32 {
        if self.ring_outside() {
            tokens::CAUTION
        } else {
            tokens::ACCENT
        }
    }

    /// The colour of the operator's dot.
    #[must_use]
    pub fn dot_color(&self) -> Option<egui::Color32> {
        self.dot.map(|(_, outside)| {
            if outside {
                tokens::CAUTION
            } else {
                tokens::TEXT_STRONG
            }
        })
    }

    /// The colour of the label of `edge`.
    #[must_use]
    pub const fn label_color(&self, edge: &SceneEdge) -> egui::Color32 {
        if edge.binding && self.ring_outside() {
            tokens::CAUTION
        } else if edge.binding {
            tokens::DANGER
        } else {
            tokens::TEXT_MUTED
        }
    }

    /// The pick that the ⚡ sends: the candidate, else the proposal. A
    /// drill takes its free value as its peck.
    #[must_use]
    pub const fn pick(&self, peck: bool) -> PickRequest {
        match (self.ring_is_candidate, peck) {
            (false, _) => PickRequest::Proposal,
            (true, true) => PickRequest::Peck(self.ring.ap_mm),
            (true, false) => PickRequest::Point {
                ap_mm: self.ring.ap_mm,
                ae_mm: self.ring.ae_mm,
            },
        }
    }
}

/// The limit with the smallest margin at an inside point `p`.
fn binding_at(env: &SafeEnvelope, p: CutPoint) -> Option<LimitKind> {
    env.boundary
        .iter()
        .filter(|l| !matches!(l.edge, Edge::PerPoint | Edge::Unmodelled { .. }))
        .filter_map(|l| l.margin(p).map(|m| (m, l.kind)))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, kind)| kind)
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
/// - `candidates`: the candidate rings (GUI state).
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
    // Esc puts the ring back (live-link ruling).
    if candidates.get(toolpath_id).is_some()
        && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
    {
        let _ = candidates.clear(toolpath_id);
    }
    let scene = EnvelopeScene::of(env, candidates.get(toolpath_id));
    let peck = matches!(operation.cut_spec().axial, Axial::Peck(_));
    let unsourced = unsourced_rules(env);
    let chip_color = if unsourced.is_empty() && !scene.ring_outside() {
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
    let what = if scene.ring_is_candidate {
        format!(
            "Apply ap {:.2} \u{00D7} ae {:.2} and its speeds.",
            scene.ring.ap_mm, scene.ring.ae_mm
        )
    } else {
        "Apply the proposal and its speeds.".to_owned()
    };
    let outside = match scene.ring_placement {
        Placement::Outside { binding, .. } => {
            format!(" Outside: {}, a warning.", binding.label())
        }
        Placement::Inside { .. } => String::new(),
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
    let response = ui
        .horizontal(|ui| {
            ui.add_space(overhang);
            // The id is unique per toolpath, and the bounds are set on
            // every frame: egui_plot keeps a bound per plot id (the
            // nomogram's bounds trap).
            Plot::new(("envelope_plot", toolpath_id.raw()))
                .height(PLOT_HEIGHT)
                .x_axis_label(WIDTH_AXIS_TITLE)
                .y_axis_label(DEPTH_AXIS_TITLE)
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
                .sense(egui::Sense::click_and_drag())
                .show(ui, |plot_ui| {
                    plot_ui.set_plot_bounds_x(ae_range.0..=ae_range.1);
                    plot_ui.set_plot_bounds_y(ap_range.0..=ap_range.1);
                    for edge in &scene.edges {
                        let (alpha, width) = if edge.binding {
                            (BINDING_ALPHA, BINDING_WIDTH)
                        } else {
                            (FAINT_ALPHA, FAINT_WIDTH)
                        };
                        let hue = if edge.binding && scene.ring_outside() {
                            tokens::CAUTION
                        } else {
                            tokens::DANGER
                        };
                        plot_ui.line(
                            Line::new("", PlotPoints::from(edge.points.clone()))
                                .color(wash(hue, alpha))
                                .width(width)
                                .allow_hover(false),
                        );
                    }
                    if let (Some((p, _)), Some(color)) = (scene.dot, scene.dot_color()) {
                        plot_ui.points(
                            Points::new("", vec![[p.ae_mm, p.ap_mm]])
                                .shape(MarkerShape::Circle)
                                .filled(true)
                                .radius(DOT_RADIUS)
                                .color(color)
                                .allow_hover(false),
                        );
                    }
                    plot_ui.points(
                        Points::new("", vec![[scene.ring.ae_mm, scene.ring.ap_mm]])
                            .shape(MarkerShape::Circle)
                            .filled(false)
                            .radius(RING_RADIUS)
                            .color(scene.ring_color())
                            .allow_hover(false),
                    );
                })
        })
        .inner;
    let transform = response.transform;
    let frame = *transform.frame();
    let to_point = |pos: egui::Pos2| {
        let v = transform.value_from_position(pos);
        CutPoint {
            ap_mm: v.y.clamp(ap_range.0, ap_range.1),
            ae_mm: v.x.clamp(ae_range.0, ae_range.1),
        }
    };
    // A click or a drag moves the candidate (no write).
    if (response.response.clicked() || response.response.dragged())
        && let Some(pos) = response.response.interact_pointer_pos()
    {
        candidates.set(toolpath_id, to_point(pos));
    }

    // The labels: the short limit name on each edge, at most `MAX_LABELS`.
    let at = |p: [f64; 2]| transform.position_from_point(&egui_plot::PlotPoint::new(p[0], p[1]));
    let ring_rect = |p: CutPoint, r: f32| {
        egui::Rect::from_center_size(
            at([p.ae_mm, p.ap_mm]),
            egui::Vec2::splat(2.0 * r + MARK_CLEARANCE),
        )
    };
    let mut keep_clear = vec![ring_rect(scene.ring, RING_RADIUS)];
    keep_clear.extend(scene.dot.map(|(p, _)| ring_rect(p, DOT_RADIUS)));
    let screen_edges: Vec<(&SceneEdge, Vec<egui::Pos2>)> = scene
        .edges
        .iter()
        .map(|e| (e, e.points.iter().copied().map(at).collect()))
        .collect();
    let lines: Vec<DrawnLine> = screen_edges
        .iter()
        .map(|(e, pts)| DrawnLine {
            points: pts.clone(),
            half_width: if e.binding {
                BINDING_WIDTH
            } else {
                FAINT_WIDTH
            } / 2.0,
            soft: !e.binding,
        })
        .collect();
    // The binding edge first: it keeps its label on a crowded plot.
    let mut specs: Vec<(bool, ChartLabel)> = screen_edges
        .into_iter()
        .map(|(e, pts)| {
            (
                e.binding,
                ChartLabel {
                    text: short_label(e.kind),
                    color: scene.label_color(e),
                    anchor: LabelAnchor::Line(pts),
                },
            )
        })
        .collect();
    specs.sort_by_key(|(binding, _)| !*binding);
    // One label per limit: a limit with two edges names one of them.
    let mut named: Vec<&'static str> = Vec::new();
    specs.retain(|(_, s)| {
        let fresh = !named.contains(&s.text);
        if fresh {
            named.push(s.text);
        }
        fresh
    });
    let placed = place_labels(
        ui,
        frame,
        &keep_clear,
        &lines,
        specs.into_iter().map(|(_, s)| s).collect(),
    );
    paint_labels(ui, frame, &placed);

    // The hover: one line at the pointer.
    if let Some(pos) = response.response.hover_pos()
        && frame.contains(pos)
    {
        let text = readout(env, to_point(pos));
        let _ = response.response.on_hover_ui_at_pointer(|ui| {
            ui.add(
                egui::Label::new(egui::RichText::new(text).small().color(tokens::TEXT_STRONG))
                    .extend(),
            );
        });
    }
    transform
}

/// The grid steps of an axis: a 1-2-5 step near a third of the range for
/// the named lines, half and a tenth of it for the fainter ones.
fn nice_steps(input: &egui_plot::GridInput) -> [f64; 3] {
    let range = (input.bounds.1 - input.bounds.0).abs();
    let raw = if range.is_finite() && range > 0.0 {
        range / 3.0
    } else {
        1.0
    };
    let power = 10_f64.powf(raw.log10().floor());
    let large = [1.0, 2.0, 5.0, 10.0]
        .iter()
        .map(|m| m * power)
        .find(|s| *s >= raw)
        .unwrap_or(10.0 * power);
    [large / 10.0, large / 2.0, large]
}

/// The one-value bar: a line from the low end to the binding limit, the
/// limit named at its end, the dot and the ring on it. Returns the bar
/// rect.
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
    // The end of the bar: the lowest ceiling of the free axis.
    let fixed = if depth {
        scene.ring.ae_mm
    } else {
        scene.ring.ap_mm
    };
    let end = if depth {
        env.ap_ceiling_at(fixed)
    } else {
        env.boundary
            .iter()
            .filter_map(|l| match l.edge {
                Edge::AeCeiling { mm } => Some((mm, l.kind)),
                Edge::ApCeiling { .. }
                | Edge::ApCeilingAbove { .. }
                | Edge::ApCurve { .. }
                | Edge::ApFloor { .. }
                | Edge::AeFloor { .. }
                | Edge::PerPoint
                | Edge::Unmodelled { .. } => None,
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
    };
    let font = egui::FontId::proportional(tokens::SIZE_CAPTION);
    let label = end.map(|(_, kind)| short_label(kind));
    let label_color = if scene.ring_outside() {
        tokens::CAUTION
    } else {
        tokens::DANGER
    };
    let label_width = label.map_or(0.0, |t| {
        ui.painter()
            .layout_no_wrap(t.to_owned(), font.clone(), label_color)
            .size()
            .x
            + tokens::SPACE_2
    });
    let width = ui.available_width();
    let (row, response) =
        ui.allocate_exact_size(egui::vec2(width, BAR_HEIGHT), egui::Sense::click_and_drag());
    let bar = egui::Rect::from_min_max(
        row.min + egui::vec2(RING_RADIUS, 0.0),
        egui::pos2(row.right() - label_width - RING_RADIUS, row.bottom()),
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
    let solid = egui::Stroke::new(BINDING_WIDTH, wash(label_color, BINDING_ALPHA));
    // The safe span of the free value: a neutral line from the low end to
    // the binding limit.
    let span = egui::Stroke::new(BINDING_WIDTH, tokens::TEXT_MUTED);
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
            solid,
        );
    }
    if let (Some(text), Some((_, _))) = (label, end) {
        let _ = painter.text(
            egui::pos2(end_x + tokens::SPACE_2, y),
            egui::Align2::LEFT_CENTER,
            text,
            font,
            label_color,
        );
    }
    if let (Some((p, _)), Some(color)) = (scene.dot, scene.dot_color()) {
        let _ = painter.circle_filled(egui::pos2(x_of(value(p)), y), DOT_RADIUS, color);
    }
    let _ = painter.circle_stroke(
        egui::pos2(x_of(value(scene.ring)), y),
        RING_RADIUS,
        egui::Stroke::new(1.5, scene.ring_color()),
    );
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
