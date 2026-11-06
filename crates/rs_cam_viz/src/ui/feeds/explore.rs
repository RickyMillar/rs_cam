//! The feed-versus-RPM nomogram, inline on the Feeds tab (W3i).
//!
//! One chart, answering one question: *where does this operation's feed and
//! RPM land relative to the engine's recommendation, the vendor band and the
//! machine's limits?* The scope is one operation. The chart is a live map:
//! the Feed, Spindle and Chipload rows above it move the point; the chart
//! itself writes nothing.
//!
//! W3i (operator design, 2026-10-30) deleted the Explore window that held
//! this chart, its drag-to-explore controls and its colour legend.
//!
//! W3k (operator, 2026-10-31) deleted the plot hover that held the legend,
//! the headline and the sources: it was too large and covered the chart.
//! Short labels on the chart ("Max feed", "Rubbing", "Now") name each
//! limit and mark, and the hover is one line with the RPM, the feed and
//! the chipload under the pointer. The chart has no zoom and no pan.
//!
//! P5d (operator, 2026-11-04: "the graphs should match"): the marks follow
//! the rule of the envelope plot. The white dot is the setpoint that ⚡
//! writes; a hollow ghost marks the stored speeds during a draft; the
//! recommendation (the ring and the suggested line) is quiet ACCENT and
//! does not move. The hover names the ring; no label paints for it.
//!
//! Every quantity here is a **commanded** advance per tooth,
//! `feed / (rpm · flutes)` — this is a pre-simulation surface and has no
//! measured value to show (Checkpoint H2, 2026-08-08).
//!
//! # Two charts were deleted here, 2026-09-15
//!
//! `draw_chart_a` plotted advance/tooth against TOOL DIAMETER and
//! `draw_chart_b` against MATERIAL HARDNESS. The operator is cutting one job,
//! with one tool, in one material, so both described how the vendor table
//! varies across tools and materials that are not on the machine. That is
//! reference data, and the Feeds tab's `Vendor data…` table presents it in
//! the right form: every row, real columns, in its own window.
//!
//! They were also the two that did not work. Chart B drew an EMPTY frame
//! with a y-axis running −0.1 to −0.5 while its series held 36 real points
//! at y 0.028–0.417: it pinned no bounds, and `egui_plot` persists a bad
//! auto-range per plot id for the life of the session. Chart A painted its
//! title over its own axis label, collided its point labels, and drew series
//! outside its frame. Evidence:
//! `planning/feeds_rework_2026-09-15/FINDINGS.md`, F-4 to F-6.
//!
//! # Lines, not shading (G-CHARTLINES, 2026-09-23)
//!
//! Operator ruling: *"Always just have the straight line, center or
//! suggested, then add more faint bands around it, no shading, but only if
//! they exist".* So the nomogram draws these items:
//!
//! - ONE solid iso-advance line at the suggested advance per tooth. It goes
//!   through the `Recommended` ring.
//! - Two fainter lines in the same colour at the vendor minimum and
//!   maximum, ONLY when the matched row publishes both. A max-only row or a
//!   one-point row gets no band lines, and no text claims a band. The rule
//!   is [`vendor_band`], which reads `FeedsExplain::published_chipload_band`.
//! - Thin lines, not shaded wedges, for the rubbing floor and the deflection
//!   ceiling.
//!
//! No item on this chart is a filled region. Until this ruling the band was a
//! shaded wedge from `0.7 × max` to `max` on a max-only row, and that lower
//! limit was invented. `feeds_charts_draw_lines_not_shading_g_chartlines`
//! holds the rule.

use egui_plot::{Line, MarkerShape, Plot, PlotPoints, Points};
use rs_cam_core::feeds::FeedsExplain;
use rs_cam_core::feeds::efficiency::CutEfficiency;
use rs_cam_core::feeds::suggest::FeedsPreview;

use super::envelope::{GHOST_RADIUS, OUTLINE_WIDTH, RECOMMENDATION_WIDTH, recommendation_color};
use super::shared::{CurrentValues, OpFeeds, chart, nice_steps, vendor_band, wash};
/// The chart label kit (W3k). P17 moved it to `ui/components/chart/labels.rs`;
/// the nomogram keeps the names.
pub(crate) use crate::ui::components::chart::labels::{
    ChartLabel, DrawnLine, LABEL_GAP, LABEL_PADDING, LabelAnchor, LabelGround, paint_labels,
    place_labels,
};
#[cfg(test)]
pub(crate) use crate::ui::components::chart::labels::{MAX_LABELS, PlacedLabel, candidates};
use crate::ui::{AppEvent, tokens};

/// Ranges that belong to an axis are drawn ON that axis, at this weight.
///
/// The machine's limits and the vendor's RPM window are facts about one
/// axis. Drawn into the plot body they were a labelled hexagon and a
/// full-height wash, both competing with the vendor band for the same
/// pixels.
pub(crate) const AXIS_MARK_WIDTH: f32 = 4.0;

/// The alpha of the chipload corridor's two lines.
///
/// The rubbing floor and the deflection ceiling ARE verdicts — below one the
/// tool burnishes instead of cutting, above the other tooth force deflects
/// or chips it — so they take the machine wall's hue rather than a data
/// scale, for the reason `DESIGN_SPEC.md` §2.6 makes the envelope the one
/// exception. Lighter than the wall's own 200, because a limit the machine
/// cannot pass is the harder stop of the two.
///
/// They were shaded wedges until 2026-09-23 (G-CHARTLINES).
const CORRIDOR_LINE_ALPHA: u8 = 150;

/// The width of the corridor's two lines.
const CORRIDOR_LINE_WIDTH: f32 = 1.0;

/// The alpha of the two vendor-limit lines. They are the suggested line's
/// colour at this alpha, so they read as fainter lines around it.
pub(crate) const BAND_LIMIT_ALPHA: u8 = 90;

/// The width of the two vendor-limit lines.
pub(crate) const BAND_LIMIT_WIDTH: f32 = 1.0;

/// The height of the inline chart: it fits the 340-point inspector under
/// the speed rows (W3i). The window drew it at 280.
const CHART_HEIGHT: f32 = 180.0;

/// The room of the feed axis ticks: four digits at the small text size.
const Y_AXIS_MIN_WIDTH: f32 = 32.0;

/// The title of the feed axis, rotated at the left of the plot.
const FEED_AXIS_TITLE: &str = "Feed (mm/min)";

/// The gap of an egui_plot axis title, as a fraction of its row height
/// (egui_plot 0.37 `axis.rs`, `AXIS_LABEL_GAP`; the crate keeps it
/// private). The rotated y title paints this far left of the plot.
const AXIS_LABEL_GAP: f32 = 0.25;

/// The nomogram of this operation, inline on the Passes tab (P5b: the
/// Feeds tab merged into Passes). The recommendation card draws under the
/// speed rows ([`draw_recommendation_inline`]).
///
/// `candidate` (P5b live link; P5d D3): the speeds that ⚡ writes for the
/// draft of the envelope plot, a lookup in the `SpeedsMap`. While it is
/// `Some`, the white dot sits at it and a ghost marks the stored speeds;
/// the recommendation does not move.
pub(crate) fn draw_inline(
    ui: &mut egui::Ui,
    op: &OpFeeds<'_>,
    candidate: Option<rs_cam_core::feeds::envelope::SpeedsSample>,
) {
    let current = super::compare::current_values_for_operation(op.operation, op.tool);
    let efficiency = rs_cam_core::feeds::efficiency::cut_efficiency(
        op.operation,
        op.tool,
        op.material,
        op.machine,
        op.preview.recommended(),
    );
    draw_chart_c(ui, &current, op.preview, efficiency.as_ref(), candidate);
}

/// The recommendation of this operation: the apply buttons, the problem
/// rows and one `Details` (W3i). `policy` draws the spindle policy row at
/// the top of the `Details`.
pub(crate) fn draw_recommendation_inline(
    ui: &mut egui::Ui,
    op: &OpFeeds<'_>,
    policy: impl FnOnce(&mut egui::Ui, &mut Vec<AppEvent>),
    events: &mut Vec<AppEvent>,
) {
    let current = super::compare::current_values_for_operation(op.operation, op.tool);
    super::compare::draw_recommendation(ui, op, &current, policy, events);
}

// ────────────────────────────────────────────────────────────────────
// Chart C — Feed vs RPM (the nomogram)
// ────────────────────────────────────────────────────────────────────

/// The chipload corridor, resolved against this chart's own axes.
///
/// The vendor band says where the vendor tested. The corridor says where the
/// CUT is physical: below the rubbing floor the tool burnishes instead of
/// cutting, above the deflection ceiling tooth force deflects or chips it.
/// Both bounds are constant-chipload rays, so the chart draws each one as a
/// thin line from the origin. It drew them as shaded wedges until 2026-09-23.
///
/// The floor always has a place on the chart, because its ray leaves the
/// origin. The ceiling does not, and holding that difference is the whole
/// reason this type exists rather than two bare `f64`s.
#[derive(Debug, Clone, Copy)]
struct Corridor {
    /// Advance per tooth (mm) below which the tool burnishes.
    floor_mm: f64,
    ceiling: Ceiling,
}

/// Where the deflection ceiling sits relative to the drawn chart.
#[derive(Debug, Clone, Copy)]
enum Ceiling {
    /// The deflection model refuses for this tool — no stickout, no axial
    /// DOC, no lateral engagement, or an edge force already over the budget.
    NotModelled,
    /// Modelled, and past the top of the plot. **Nothing is drawn.**
    ///
    /// This is the ORDINARY case, and it is not an error. Measured on the
    /// reference fixture (6 mm two-flute flat, 45 mm stickout, 4.20 DOC,
    /// 2.10 WOC, generic softwood) the ceiling is **2.4315 mm/tooth**
    /// against a chart that draws **0.12353 mm/tooth** at most — about
    /// twenty times off scale. A stubby carbide cutter in wood is not
    /// deflection-limited; the binding constraints are the feed cap, the
    /// rubbing floor and rigidity.
    ///
    /// This pair was 9.36 against 0.1176 until 2026-09-18. Neither number
    /// reproduced: the ceiling was 3.85x the measured value, and 0.1176 is
    /// the chart top at 18000 RPM while this cut runs 17000.
    /// `the_corridor_bounds_the_band_g_corridor` measures the pair; the
    /// derivation is in `planning/load_model_2026-09-16/CORRIDOR_REPAIR.md`.
    ///
    /// A line clamped to the top edge would read as *"you are near the
    /// force limit"*, false by two orders of magnitude. That is
    /// absence-rendered-as-a-reading, the failure this window has already
    /// shipped once — a readout that printed `37891 RPM · BURN risk` for a
    /// pointer that was not over the chart. So the line is absent and
    /// the hover of the card's chip row carries the ceiling number.
    OffScale,
    /// Modelled and inside the plot's range, so the ceiling line IS drawn.
    ///
    /// **The diameter is the lever here, not the stickout.**
    /// `ToolDefinition::tip_deflection_mm` models a STEPPED cantilever: a
    /// fixed 25 mm flute section, below a 6.35 mm shank that fills the rest
    /// of the stickout. Since T-17 the flute section bends on
    /// `ENDMILL_EQUIVALENT_DIAMETER_FRACTION` (0.80) of the cutting
    /// diameter, not on the raw diameter. `I` goes with the fourth power of
    /// the diameter, so at Ø2 mm the shank is stiffer per unit length by
    ///
    /// ```text
    /// (6.35 / (0.80 * 2.0))^4 = 3.96875^4 = 248.09
    /// ```
    ///
    /// — about **248x**, not the 101x the raw 2 mm diameter would give. The
    /// flute section therefore carries nearly all of the compliance, and its
    /// length does not change with the stickout. Measured at Ø2 mm: 40 mm of
    /// stickout gives 2.618e-2 mm/N and 120 mm gives 3.767e-2 mm/N — 3x the
    /// length for 1.44x the compliance, not 27x.
    ///
    /// So a THIN tool brings the ceiling down onto the chart; a long one
    /// barely moves it. Any prose that says the compliance goes with the
    /// cube of stickout describes a plain cantilever, not this model.
    /// Derivation: `planning/load_model_2026-09-16/CORRIDOR_REPAIR.md` §6.
    OnChart { ceiling_mm: f64 },
}

impl Corridor {
    /// Resolve the corridor against the plot's feed axis.
    ///
    /// A constant advance per tooth is a RAY through the origin, not a
    /// height, so "inside the y-range" has to be asked at one RPM. It is
    /// asked at the RPM the recommendation sits at, which is the RPM the
    /// operator is being shown, and the question is put in the chart's own
    /// unit: the ceiling against the highest advance per tooth the plot can
    /// draw, `feed_axis_max / (rpm · flutes)`.
    ///
    /// A non-positive RPM or flute count leaves that quantity undefined, and
    /// an undefined comparison abstains: the ceiling is reported off scale
    /// and no line is drawn.
    fn resolve(efficiency: &CutEfficiency, rpm: f64, flutes: f64, feed_axis_max: f64) -> Self {
        let chart_top_mm = if rpm > 0.0 && flutes > 0.0 {
            feed_axis_max / (rpm * flutes)
        } else {
            0.0
        };
        let ceiling = match efficiency.deflection_ceiling_mm {
            None => Ceiling::NotModelled,
            Some(ceiling_mm) if ceiling_mm.is_finite() && ceiling_mm <= chart_top_mm => {
                Ceiling::OnChart { ceiling_mm }
            }
            Some(_) => Ceiling::OffScale,
        };
        Self {
            floor_mm: efficiency.rubbing_floor_mm,
            ceiling,
        }
    }
}

/// The nomogram: the live operating point of the operation against the
/// engine's recommendation, the vendor lines, the corridor and the machine
/// limits. W3k: a short label beside each limit says what it is; the hover
/// is one line with the RPM, the feed and the chipload under the pointer.
///
/// W3k (operator, 2026-10-31): the chart has no navigation. The plot senses
/// hover only, every zoom, pan and reset flag is off, and the bounds are set
/// on every frame (see [`chart_bounds`]).
pub(crate) fn draw_chart_c(
    ui: &mut egui::Ui,
    current: &CurrentValues,
    preview: &FeedsPreview,
    efficiency: Option<&CutEfficiency>,
    candidate: Option<rs_cam_core::feeds::envelope::SpeedsSample>,
) {
    let explain = preview.explain();
    let flutes = explain.flute_count.max(1) as f64;
    let env = &explain.machine;

    // RPM/Feed axis bounds — show a bit past the machine cap so the
    // wall is visible.
    let rpm_axis_max = env.spindle_max_rpm * 1.15;
    let feed_axis_max = env.max_feed_mm_min * 1.05;
    let (x_bounds, y_bounds) = chart_bounds(rpm_axis_max, feed_axis_max);

    // The corridor is resolved here, where the axes are known, and nowhere
    // else: whether the deflection ceiling has a place on this chart is a
    // question about this chart.
    let corridor = efficiency
        .map(|eff| Corridor::resolve(eff, explain.recommended.rpm, flutes, feed_axis_max));

    // The vendor band, ONLY when the row publishes both limits. A max-only
    // row and a one-point row give `None`, and the chart draws no band line.
    let band = vendor_band(explain);
    // The suggested advance per tooth: the ray through the recommended
    // ring. `None` when the recommendation has no RPM to divide by. P5d D3:
    // it stays at the recommendation during a draft (no blue mark moves).
    let suggested = suggested_advance_mm(explain);

    // The marks, in plot coordinates: the label layout keeps clear of each.
    // P5d D3, one rule on both charts: the white dot is the setpoint that
    // ⚡ writes (the draft's speeds while a draft exists, else the stored
    // speeds); a hollow ghost marks the stored speeds only during a draft;
    // the quiet ACCENT ring is the recommendation and never moves.
    let recommended = [
        explain.recommended.rpm,
        explain.recommended.feed_rate_mm_min,
    ];
    let stored = match (current.spindle_rpm, current.feed_rate_mm_min) {
        (Some(rpm), Some(feed)) => Some([f64::from(rpm), feed]),
        _ => None,
    };
    let now = candidate.map(|c| [c.rpm, c.feed_mm_min]).or(stored);
    let ghost = candidate.and(stored);
    let target_chipload = explain.recommended.derates.target_chip_load_mm;
    let target = (target_chipload > 0.0 && explain.recommended.rpm > 0.0)
        .then_some([
            explain.recommended.rpm,
            target_chipload * explain.recommended.rpm * flutes,
        ])
        .filter(|t| (t[1] - explain.recommended.feed_rate_mm_min).abs() > 1.0);

    let mut hover_readout: Option<String> = None;
    // egui_plot 0.37 (`axis.rs`, `AXIS_LABEL_GAP` = 0.25) paints the
    // rotated feed axis title a quarter of its galley height left of the
    // plot. At the left edge of the inspector the scroll area clipped the
    // tops of its glyphs ("ieea"). The plot starts that far in, so the
    // title paints whole. The indent adds no text row (IC1). The galley is
    // the one egui_plot lays out: the Body style, no wrap.
    let title_overhang = axis_title_overhang(ui, FEED_AXIS_TITLE);
    // The screen room that a wall label needs above the feed wall: the
    // label box, its gap to the line, the wall's half width and the one
    // point the layout keeps inside the frame.
    let wall_label_room = {
        let font = egui::FontId::proportional(tokens::SIZE_CAPTION);
        let text_height = ui
            .painter()
            .layout_no_wrap(LABEL_MAX_FEED.to_owned(), font, tokens::DANGER)
            .size()
            .y;
        f64::from(text_height + 2.0 * LABEL_PADDING.y + LABEL_GAP + WALL_WIDTH / 2.0 + 1.5)
    };
    let plot_response = ui
        .horizontal(|ui| {
            ui.add_space(title_overhang);
            Plot::new("feeds_tab_nomogram")
                .height(CHART_HEIGHT)
                .x_axis_label("RPM")
                .y_axis_label(FEED_AXIS_TITLE)
                // The four-digit feed ticks need this room at the 240-point rail;
                // the default width clipped them.
                .y_axis_min_width(Y_AXIS_MIN_WIDTH)
                // P5c: the default log-10 grid named only 0 on the feed
                // axis (a step of 1000 mm/min is closer than its label
                // spacing on a 180-point chart). A 1-2-5 step near a third
                // of the range gives three or four names, as on the
                // envelope plot. The axis grows past `Y_AXIS_MIN_WIDTH`
                // when a name needs more room.
                .y_grid_spacer(egui_plot::uniform_grid_spacer(|i| nice_steps(&i)))
                // P5d D5: the RPM axis reads the same rule. The default
                // log-10 grid named only "0" at the 240-point rail.
                .x_grid_spacer(egui_plot::uniform_grid_spacer(|i| nice_steps(&i)))
                // W3k: no navigation. W3i turned off drag, zoom and scroll,
                // but egui_plot has three more doors, each on by default: a
                // drag on the axis ticks zooms that axis, a right drag zooms
                // to a box, and a double click resets. A zoom sets
                // `auto_bounds` false in the plot memory, which egui keeps
                // per plot id. Measured headless: a short drag on the RPM
                // ticks moved the x axis to about -100000, so every pointer
                // position read outside the axes and the readout abstained
                // on the whole chart until a double click.
                .allow_drag(false)
                .allow_zoom(false)
                .allow_scroll(false)
                .allow_boxed_zoom(false)
                .allow_axis_zoom_drag(false)
                .allow_double_click_reset(false)
                .sense(egui::Sense::hover())
                .show(ui, |plot_ui| {
                    // W3k: the bounds are set on every frame, so a bound kept
                    // in the plot memory from an older build cannot stay.
                    plot_ui.set_plot_bounds_x(x_bounds.clone());
                    // The feed wall is the top line of the chart. The plot
                    // keeps room above it for its label, so "Max feed" has
                    // a place on a narrow rail (lead, 2026-10-31).
                    let plot_height = f64::from(plot_ui.transform().frame().height());
                    plot_ui.set_plot_bounds_y(with_wall_label_room(
                        y_bounds.clone(),
                        env.max_feed_mm_min,
                        plot_height,
                        wall_label_room,
                    ));
                    // 0. The corridor: the two bounds that turn the vendor band into
                    //    a place the cut can live. Each is a thin line in the machine
                    //    wall's hue. They were shaded wedges until 2026-09-23
                    //    (G-CHARTLINES: "no shading"). W3k: "Rubbing" and "Chip
                    //    limit" label them.
                    if let Some(corridor) = corridor {
                        if corridor.floor_mm.is_finite() && corridor.floor_mm > 0.0 {
                            plot_ui.line(
                                Line::new(
                                    "",
                                    iso_line(
                                        corridor.floor_mm,
                                        env.spindle_max_rpm,
                                        flutes,
                                        env.max_feed_mm_min,
                                    ),
                                )
                                .color(wash(tokens::DANGER, CORRIDOR_LINE_ALPHA))
                                .width(CORRIDOR_LINE_WIDTH)
                                .allow_hover(false),
                            );
                        }
                        // The ceiling line is drawn ONLY when the ceiling lands
                        // inside the plot. It is never clamped to the top edge —
                        // see [`Ceiling::OffScale`].
                        if let Ceiling::OnChart { ceiling_mm } = corridor.ceiling {
                            plot_ui.line(
                                Line::new(
                                    "",
                                    iso_line(
                                        ceiling_mm,
                                        env.spindle_max_rpm,
                                        flutes,
                                        env.max_feed_mm_min,
                                    ),
                                )
                                .color(wash(tokens::DANGER, CORRIDOR_LINE_ALPHA))
                                .width(CORRIDOR_LINE_WIDTH)
                                .allow_hover(false),
                            );
                        }
                    }
                    // 1. The suggested advance per tooth: ONE solid iso-advance line,
                    //    clipped at the machine RPM cap and feed cap. It goes through
                    //    the `Recommended` ring. P5d D3: a recommendation mark, so it
                    //    is quiet ACCENT at 1 point, as the ring is.
                    if let Some(cl) = suggested {
                        plot_ui.line(
                            Line::new(
                                "",
                                iso_line(cl, env.spindle_max_rpm, flutes, env.max_feed_mm_min),
                            )
                            .color(recommendation_color())
                            .width(RECOMMENDATION_WIDTH)
                            .allow_hover(false),
                        );
                    }
                    // 2. The vendor limits: two fainter lines in the same colour,
                    //    ONLY when the row publishes both. No fill between them.
                    //
                    //    Operator ruling, 2026-09-23: "Always just have the straight
                    //    line, center or suggested, then add more faint bands around
                    //    it, no shading, but only if they exist". A row that
                    //    publishes one value gets no band lines. The chart does not
                    //    make a band from one value.
                    if let Some((lo, hi)) = band {
                        for limit in [lo, hi] {
                            plot_ui.line(
                                Line::new(
                                    "",
                                    iso_line(
                                        limit,
                                        env.spindle_max_rpm,
                                        flutes,
                                        env.max_feed_mm_min,
                                    ),
                                )
                                .color(wash(chart::BAND, BAND_LIMIT_ALPHA))
                                .width(BAND_LIMIT_WIDTH)
                                .allow_hover(false),
                            );
                        }
                    }

                    // 3. The machine walls: the spindle maximum and the feed
                    //    maximum, and the spindle minimum as an amber wall of the
                    //    same style (lead, 2026-10-31: the axis band left its
                    //    label floating above it). W3k (lead, 2026-10-31): the inline chart draws no
                    //    "past the cap" axis marks. The red bar beyond Max RPM and
                    //    the red tick above Max feed at RPM 0 said what the walls
                    //    already say, and read as stray marks. The vendor RPM
                    //    window mark on the axis went too: it had no label.
                    //    `draw_machine_envelope`
                    //    keeps them for the project scatter.
                    for wall in walls(env) {
                        plot_ui.line(
                            Line::new("", PlotPoints::from(wall.to_vec()))
                                .color(wash(tokens::DANGER, WALL_ALPHA))
                                .width(WALL_WIDTH)
                                .allow_hover(false),
                        );
                    }
                    if let Some(wall) = min_wall(env) {
                        plot_ui.line(
                            Line::new("", PlotPoints::from(wall.to_vec()))
                                .color(tokens::CAUTION)
                                .width(WALL_WIDTH)
                                .allow_hover(false),
                        );
                    }

                    // 5. The target (pre-derate) operating point: where the
                    //    recommendation would sit without the depth, power and
                    //    feed-clamp derates (ruling R4 removed the safety factor
                    //    and the workholding factor). A dashed line runs from it
                    //    down to the recommendation. W3k: a neutral ring, so the
                    //    one blue ring on the chart is the recommendation.
                    if let Some(t) = target {
                        plot_ui.points(
                            Points::new("", vec![t])
                                .shape(MarkerShape::Circle)
                                .filled(false)
                                .radius(TARGET_RADIUS)
                                .color(tokens::TEXT_MUTED)
                                .allow_hover(false),
                        );
                        plot_ui.line(
                            Line::new("", PlotPoints::from(vec![t, recommended]))
                                .color(wash(tokens::TEXT_MUTED, 180))
                                .style(egui_plot::LineStyle::Dashed { length: 4.0 })
                                .width(1.5_f32)
                                .allow_hover(false),
                        );
                    }

                    // 6. The marks of P5d D3 (the recommended ring, the ghost
                    //    and the white dot) paint after the plot, at an exact
                    //    1-point outline: egui_plot strokes an outline marker
                    //    at a fifth of its radius.

                    // 7. The hover readout: one line, the RPM, the feed and the
                    //    chipload under the pointer (W3k).
                    //
                    // The pointer must be ON the chart. `pointer_coordinate` also
                    // answers for a pointer outside the drawn bounds, and the
                    // readout below then reports an operating point that does not
                    // exist: measured at `37891 RPM · 0 mm/min · BURN risk (below
                    // band)`, painted in the danger colour, on an axis whose maximum
                    // is 27 600. That is a hazard verdict manufactured from an
                    // absence, which is the failure shape this repo cares about most.
                    //
                    // The strip BETWEEN the spindle cap and the axis maximum is a
                    // real place to hover — it is how `· past spindle cap` is read —
                    // so the guard is the axis bound, not the machine's.
                    if let Some(hover) = plot_ui
                        .pointer_coordinate()
                        .filter(|p| (0.0..=rpm_axis_max).contains(&p.x))
                        .filter(|p| (0.0..=feed_axis_max).contains(&p.y))
                    {
                        // Read the CLAMPED coordinates once and derive everything
                        // from them, readout and cap-note alike — see
                        // `hover_readout_chipload`.
                        let (rpm, feed) = (hover.x.max(0.0), hover.y.max(0.0));
                        let cl = hover_readout_chipload(rpm, feed, flutes);
                        let cap_note = if rpm > env.spindle_max_rpm {
                            " \u{00B7} past spindle cap"
                        } else if feed > env.max_feed_mm_min {
                            " \u{00B7} past feed cap"
                        } else if rpm < env.spindle_min_rpm {
                            " \u{00B7} below spindle min"
                        } else {
                            ""
                        };
                        hover_readout = Some(format!(
                            "{rpm:.0} RPM \u{00B7} {feed:.0} mm/min \u{00B7} {cl:.4} mm/t{cap_note}"
                        ));
                    }
                })
        })
        .inner;

    // W3k: the labels. They paint after the plot, in screen space, so each
    // one has a background and the layout can test it against the others
    // and against every drawn line.
    let transform = &plot_response.transform;
    let frame = *transform.frame();
    let at = |p: [f64; 2]| transform.position_from_point(&egui_plot::PlotPoint::new(p[0], p[1]));
    let ring = |p: [f64; 2], radius: f32| {
        egui::Rect::from_center_size(at(p), egui::Vec2::splat(2.0 * radius + MARK_CLEARANCE))
    };
    // The marks (P5d D3): the quiet ring at the recommendation (operator,
    // 2026-11-04: "recommendations should be subtle"), the hollow ghost at
    // the stored speeds during a draft, then the white dot at the setpoint
    // (ruling R23: an operator value is neutral ink, not a verdict). The
    // dot shows inside the ring when the operation runs at the
    // recommendation.
    let painter = ui.painter_at(frame);
    let _ = painter.circle_stroke(
        at(recommended),
        RECOMMENDED_RADIUS,
        egui::Stroke::new(RECOMMENDATION_WIDTH, recommendation_color()),
    );
    if let Some(point) = ghost {
        let _ = painter.circle_stroke(
            at(point),
            GHOST_RADIUS,
            egui::Stroke::new(OUTLINE_WIDTH, tokens::TEXT_MUTED),
        );
    }
    if let Some(point) = now {
        let _ = painter.circle_filled(at(point), NOW_RADIUS, tokens::TEXT_STRONG);
    }
    let mut keep_clear = vec![ring(recommended, RECOMMENDED_RADIUS)];
    keep_clear.extend(now.map(|p| ring(p, NOW_RADIUS)));
    keep_clear.extend(ghost.map(|p| ring(p, GHOST_RADIUS)));
    keep_clear.extend(target.map(|p| ring(p, TARGET_RADIUS)));
    // The crossing of the two machine walls.
    keep_clear.push(ring(
        [env.spindle_max_rpm, env.max_feed_mm_min],
        CROSSING_CLEARANCE,
    ));
    let iso_screen = |cl: f64| -> Vec<egui::Pos2> {
        clip_iso_line(cl, env.spindle_max_rpm, flutes, env.max_feed_mm_min)
            .into_iter()
            .map(at)
            .collect()
    };
    // Every drawn line, in screen space, with half its width. No label
    // crosses one.
    let mut lines: Vec<DrawnLine> = Vec::new();
    let mut push_line = |points: Vec<egui::Pos2>, width: f32, soft: bool| {
        lines.push(DrawnLine {
            points,
            half_width: width / 2.0,
            soft,
        });
    };
    let floor_line = corridor
        .filter(|c| c.floor_mm.is_finite() && c.floor_mm > 0.0)
        .map(|c| iso_screen(c.floor_mm));
    let ceiling_line = corridor.and_then(|c| match c.ceiling {
        Ceiling::OnChart { ceiling_mm } => Some(iso_screen(ceiling_mm)),
        Ceiling::NotModelled | Ceiling::OffScale => None,
    });
    if let Some(l) = &floor_line {
        push_line(l.clone(), CORRIDOR_LINE_WIDTH, false);
    }
    if let Some(l) = &ceiling_line {
        push_line(l.clone(), CORRIDOR_LINE_WIDTH, false);
    }
    if let Some(cl) = suggested {
        push_line(iso_screen(cl), RECOMMENDATION_WIDTH, false);
    }
    if let Some((lo, hi)) = band {
        push_line(iso_screen(lo), BAND_LIMIT_WIDTH, true);
        push_line(iso_screen(hi), BAND_LIMIT_WIDTH, true);
    }
    let [rpm_wall, feed_wall] = walls(env);
    push_line(
        rpm_wall.iter().copied().map(at).collect(),
        WALL_WIDTH,
        false,
    );
    push_line(
        feed_wall.iter().copied().map(at).collect(),
        WALL_WIDTH,
        false,
    );
    let min_line: Option<Vec<egui::Pos2>> =
        min_wall(env).map(|wall| wall.iter().copied().map(at).collect());
    if let Some(line) = &min_line {
        push_line(line.clone(), WALL_WIDTH, false);
    }
    if let Some(t) = target {
        push_line(vec![at(t), at(recommended)], 1.5, false);
    }

    // The labels in priority order: the walls, the corridor, Now, Vendor
    // range, Before derate. `MAX_LABELS` of them paint at most. P5d D3: the
    // recommended ring has no painted label; the hover names it.
    let mut specs = vec![ChartLabel {
        text: LABEL_MAX_FEED.to_owned(),
        color: tokens::DANGER,
        anchor: LabelAnchor::Line(feed_wall.iter().copied().map(at).collect()),
        ground: LabelGround::Overlay,
    }];
    specs.push(ChartLabel {
        text: LABEL_MAX_RPM.to_owned(),
        color: tokens::DANGER,
        anchor: LabelAnchor::Line(rpm_wall.iter().copied().map(at).collect()),
        ground: LabelGround::Overlay,
    });
    if let Some(l) = floor_line {
        specs.push(ChartLabel {
            text: LABEL_RUBBING.to_owned(),
            color: tokens::DANGER,
            anchor: LabelAnchor::Line(l),
            ground: LabelGround::Overlay,
        });
    }
    if let Some(l) = ceiling_line {
        specs.push(ChartLabel {
            text: LABEL_CHIP_LIMIT.to_owned(),
            color: tokens::DANGER,
            anchor: LabelAnchor::Line(l),
            ground: LabelGround::Overlay,
        });
    }
    if let Some(line) = min_line {
        specs.push(ChartLabel {
            text: LABEL_MIN_RPM.to_owned(),
            color: tokens::CAUTION,
            anchor: LabelAnchor::Line(line),
            ground: LabelGround::Overlay,
        });
    }
    // The current dot takes a label only where it does not sit in the ring.
    // It ranks under the limits (lead, 2026-10-31): on a crowded chart the
    // white dot keeps its place and its label drops first.
    if let Some(point) = now
        && at(point).distance(at(recommended)) > RECOMMENDED_RADIUS + NOW_RADIUS
    {
        specs.push(ChartLabel {
            text: LABEL_NOW.to_owned(),
            color: tokens::TEXT_STRONG,
            anchor: LabelAnchor::Mark {
                center: at(point),
                radius: NOW_RADIUS,
            },
            ground: LabelGround::Overlay,
        });
    }
    // The vendor range: ONE label for its two lines, beside either one
    // (the lower line first: the space under it is open more often).
    if let Some((lo, hi)) = band {
        specs.push(ChartLabel {
            text: LABEL_VENDOR_RANGE.to_owned(),
            color: chart::BAND,
            anchor: LabelAnchor::Lines(vec![iso_screen(lo), iso_screen(hi)]),
            ground: LabelGround::Overlay,
        });
    }
    if let Some(t) = target {
        specs.push(ChartLabel {
            text: LABEL_BEFORE_DERATE.to_owned(),
            color: tokens::TEXT_MUTED,
            anchor: LabelAnchor::Mark {
                center: at(t),
                radius: TARGET_RADIUS,
            },
            ground: LabelGround::Overlay,
        });
    }
    let placed = place_labels(ui, frame, &keep_clear, &lines, specs);
    paint_labels(ui, frame, &placed);
    #[cfg(test)]
    tests::record(frame, &placed, tokens::SIZE_CAPTION);

    // W3k: the hover is one line at the pointer, only on the axes. P5d D3:
    // on the recommended ring, a line above it names the ring.
    let on_ring = plot_response
        .response
        .hover_pos()
        .is_some_and(|p| p.distance(at(recommended)) <= RECOMMENDED_RADIUS + MARK_CLEARANCE);
    if let Some(text) = hover_readout {
        let _ = plot_response.response.on_hover_ui_at_pointer(|ui| {
            if on_ring {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(LABEL_RECOMMENDED)
                            .small()
                            .color(tokens::TEXT_BODY),
                    )
                    .extend(),
                );
            }
            ui.add(
                egui::Label::new(egui::RichText::new(text).small().color(tokens::TEXT_STRONG))
                    .extend(),
            );
        });
    }
}

/// The screen room (points) that egui_plot paints a rotated axis `title`
/// outside the plot (`AXIS_LABEL_GAP`). A plot at the inspector edge starts
/// this far in, so the title paints whole. Shared with the envelope plot.
pub(crate) fn axis_title_overhang(ui: &egui::Ui, title: &str) -> f32 {
    let title_height = egui::WidgetText::from(title)
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Body,
        )
        .size()
        .y;
    (AXIS_LABEL_GAP * title_height).ceil()
}

/// The plot bounds: the axis range and a margin on each side.
///
/// The margin is egui_plot's own default (`margin_fraction`, 0.05). Until
/// W3k the plot found its bounds from `include_x` and `include_y` with that
/// margin; the explicit bounds keep the same look. A degenerate machine
/// (a zero cap) still gets a range, so the plot never divides by zero.
fn chart_bounds(
    rpm_axis_max: f64,
    feed_axis_max: f64,
) -> (std::ops::RangeInclusive<f64>, std::ops::RangeInclusive<f64>) {
    let range = |max: f64| {
        let max = if max.is_finite() && max > 0.0 {
            max
        } else {
            1.0
        };
        (-PLOT_MARGIN * max)..=((1.0 + PLOT_MARGIN) * max)
    };
    (range(rpm_axis_max), range(feed_axis_max))
}

/// The margin of each axis, a fraction of its range (egui_plot's default).
const PLOT_MARGIN: f64 = 0.05;

/// The feed bounds with room above the feed wall at `feed_max` for one wall
/// label: the top bound rises until the screen gap between the wall and the
/// top of the plot is at least `room_px` on a plot `plot_height_px` high.
/// The bottom bound does not move; a top bound that is already high enough
/// does not move either.
///
/// With bottom `b`, top `t` and the wall at `f`, the gap is
/// `(t - f) / (t - b) x height`. The gap is `room` at
/// `t = (f - k b) / (1 - k)`, with `k = room / height`.
fn with_wall_label_room(
    bounds: std::ops::RangeInclusive<f64>,
    feed_max: f64,
    plot_height_px: f64,
    room_px: f64,
) -> std::ops::RangeInclusive<f64> {
    let (bottom, top) = (*bounds.start(), *bounds.end());
    let k = room_px / plot_height_px;
    // A room of half the plot or more would leave no chart under the wall:
    // keep the bounds then.
    if !(k.is_finite() && k > 0.0 && k < 0.5) || !feed_max.is_finite() {
        return bounds;
    }
    let needed = (feed_max - k * bottom) / (1.0 - k);
    bottom..=top.max(needed)
}

/// The spindle minimum wall, in plot coordinates: an upright line at the
/// machine's minimum RPM up to the feed maximum. `None` for a machine with
/// no minimum.
///
/// The upright walls stop at the feed wall (lead, 2026-10-31): the three
/// walls close the machine envelope, and the band above the feed wall is
/// free for the "Max feed" label, which had no place on the 240 rail.
fn min_wall(env: &rs_cam_core::feeds::MachineEnvelope) -> Option<[[f64; 2]; 2]> {
    (env.spindle_min_rpm > 0.0).then_some([
        [env.spindle_min_rpm, 0.0],
        [env.spindle_min_rpm, env.max_feed_mm_min],
    ])
}

/// The two machine walls, in plot coordinates: the spindle maximum (an
/// upright line up to the feed maximum, see [`min_wall`]) and the feed
/// maximum (a level line from RPM 0 to the spindle maximum). The walls meet
/// at the corner and close the machine envelope (lead, 2026-10-31).
fn walls(env: &rs_cam_core::feeds::MachineEnvelope) -> [[[f64; 2]; 2]; 2] {
    [
        [
            [env.spindle_max_rpm, 0.0],
            [env.spindle_max_rpm, env.max_feed_mm_min],
        ],
        [
            [0.0, env.max_feed_mm_min],
            [env.spindle_max_rpm, env.max_feed_mm_min],
        ],
    ]
}

/// The alpha of the machine walls: the danger hue, as `draw_machine_envelope`
/// draws them on the project scatter.
const WALL_ALPHA: u8 = 200;

/// The width of the machine walls, in points.
const WALL_WIDTH: f32 = 2.0;

/// The radius of the recommended ring, in points.
const RECOMMENDED_RADIUS: f32 = 7.0;

/// The radius of the current dot, in points: it shows inside the ring.
const NOW_RADIUS: f32 = 3.5;

/// The radius of the pre-derate target ring, in points.
const TARGET_RADIUS: f32 = 6.0;

/// The room a label keeps around a mark, in points.
const MARK_CLEARANCE: f32 = 4.0;

/// The half side of the square that a label keeps clear at the crossing of
/// the two machine walls, in points.
const CROSSING_CLEARANCE: f32 = 4.0;

/// The chart labels (W3k, operator 2026-10-31: "labels instead of hover
/// with really simple terms"). Each names one limit or mark in one or two
/// words. The chart has no power line, so there is no power label.
pub(crate) const LABEL_RECOMMENDED: &str = "Recommended";
/// The machine feed wall.
pub(crate) const LABEL_MAX_FEED: &str = "Max feed";
/// The machine spindle wall.
pub(crate) const LABEL_MAX_RPM: &str = "Max RPM";
/// The spindle minimum wall (amber, upright, at the machine minimum RPM).
pub(crate) const LABEL_MIN_RPM: &str = "Min RPM";
/// The rubbing floor: below it the tool burnishes instead of cutting.
pub(crate) const LABEL_RUBBING: &str = "Rubbing";
/// The deflection ceiling: above it tooth force deflects or chips the tool.
pub(crate) const LABEL_CHIP_LIMIT: &str = "Chip limit";
/// The two faint lines of the vendor's published chipload range.
pub(crate) const LABEL_VENDOR_RANGE: &str = "Vendor range";
/// The operation's current feed and spindle speed.
pub(crate) const LABEL_NOW: &str = "Now";
/// The pre-derate target of the recommendation.
pub(crate) const LABEL_BEFORE_DERATE: &str = "Before derate";

/// The suggested advance per tooth (mm/tooth): the recommended feed over
/// the recommended RPM and the flute count.
///
/// This is the one solid line on the nomogram, and it goes through the
/// `Recommended` ring. `None` when the RPM is not positive or the value
/// is not finite, because then there is no ray to draw.
fn suggested_advance_mm(explain: &FeedsExplain) -> Option<f64> {
    let r = &explain.recommended;
    let flutes = explain.flute_count.max(1) as f64;
    if r.rpm <= 0.0 {
        return None;
    }
    let cl = r.feed_rate_mm_min / (r.rpm * flutes);
    (cl.is_finite() && cl > 0.0).then_some(cl)
}

/// One iso-advance line: `feed = cl · rpm · flutes`, from the origin,
/// clipped to the chart's RPM and feed extents.
///
/// The chart draws every advance-per-tooth quantity as one of these lines.
/// It draws no filled region between two of them (G-CHARTLINES).
fn iso_line(cl: f64, rpm_max: f64, flutes: f64, feed_cap: f64) -> PlotPoints<'static> {
    PlotPoints::from(clip_iso_line(cl, rpm_max, flutes, feed_cap))
}

fn clip_iso_line(cl: f64, rpm_max: f64, flutes: f64, feed_cap: f64) -> Vec<[f64; 2]> {
    let mut out = vec![[0.0, 0.0]];
    let feed_at_rpm_max = cl * rpm_max * flutes;
    if feed_at_rpm_max <= feed_cap {
        out.push([rpm_max, feed_at_rpm_max]);
    } else {
        // Slope binds before RPM cap — find RPM where feed hits cap.
        let rpm_cross = if cl > 0.0 && flutes > 0.0 {
            feed_cap / (cl * flutes)
        } else {
            rpm_max
        };
        // The line stops where it meets the feed cap. It does not run on
        // along the cap: that segment is the machine wall, not this line.
        out.push([rpm_cross, feed_cap]);
    }
    out
}

/// Advance/tooth for the nomogram's hover readout — **display only**.
///
/// The bug this exists to hold (ledgered in
/// `planning/review_2026-08-04/ORCHESTRATION_LOG.md`, W10-LV item 10, as
/// a cosmetic wart): the readout printed the RPM and feed through
/// `.max(0.0)` but divided the RAW pointer coordinates, so dragging
/// below the feed axis produced a line that read
/// `18000 RPM · 0 mm/min → −0.0928 mm/tooth`. Three quantities on one
/// line, two of them clamped and one not — a negative advance per tooth
/// is not a thing, and the number did not correspond to the RPM and feed
/// printed beside it.
///
/// The fix is agreement, not a second clamp: the caller clamps once and
/// this function is handed the same values the label prints. Zero RPM
/// (or zero flutes) yields `0.0` rather than an infinity, which is what
/// the pre-existing `hover.x > 0.0` guard already did.
///
/// **Nothing downstream reads this.** It formats a tooltip; no recipe,
/// gate or export consumes it.
fn hover_readout_chipload(rpm: f64, feed_mm_min: f64, flutes: f64) -> f64 {
    let divisor = rpm * flutes;
    if divisor > 0.0 {
        (feed_mm_min / divisor).max(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
#[path = "explore_tests.rs"]
mod tests;
