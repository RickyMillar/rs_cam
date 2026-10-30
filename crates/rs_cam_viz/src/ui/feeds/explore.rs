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
//! Short labels on the chart ("Max feed", "Rubbing", "Recommended") name
//! each limit and mark, and the hover is one line with the RPM, the feed
//! and the chipload under the pointer. The chart has no zoom and no pan.
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

use super::shared::{CurrentValues, OpFeeds, chart, vendor_band, wash};
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

/// The width of the ONE solid line at the suggested advance per tooth.
const SUGGESTED_LINE_WIDTH: f32 = 1.5;

/// The alpha of the two vendor-limit lines. They are the suggested line's
/// colour at this alpha, so they read as fainter lines around it.
const BAND_LIMIT_ALPHA: u8 = 90;

/// The width of the two vendor-limit lines.
const BAND_LIMIT_WIDTH: f32 = 1.0;

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

/// The Feeds tab's chart block: the nomogram of this operation, then the
/// recommendation (the apply buttons, the problem rows and one `Details`).
///
/// `policy` draws the spindle policy row at the top of the `Details`.
pub(crate) fn draw_inline(
    ui: &mut egui::Ui,
    op: &OpFeeds<'_>,
    policy: impl FnOnce(&mut egui::Ui, &mut Vec<AppEvent>),
    events: &mut Vec<AppEvent>,
) {
    let current = super::compare::current_values_for_operation(op.operation, op.tool);
    let efficiency = rs_cam_core::feeds::efficiency::cut_efficiency(
        op.operation,
        op.tool,
        op.material,
        op.machine,
        op.preview.recommended(),
    );
    draw_chart_c(ui, &current, op.preview, efficiency.as_ref());
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
    // ring. `None` when the recommendation has no RPM to divide by.
    let suggested = suggested_advance_mm(explain);

    // The marks, in plot coordinates: the label layout keeps clear of each.
    let recommended = [
        explain.recommended.rpm,
        explain.recommended.feed_rate_mm_min,
    ];
    let now = match (current.spindle_rpm, current.feed_rate_mm_min) {
        (Some(rpm), Some(feed)) => Some([f64::from(rpm), feed]),
        _ => None,
    };
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
    let title_height = egui::WidgetText::from(FEED_AXIS_TITLE)
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Extend),
            f32::INFINITY,
            egui::TextStyle::Body,
        )
        .size()
        .y;
    let title_overhang = (AXIS_LABEL_GAP * title_height).ceil();
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
                    plot_ui.set_plot_bounds_y(y_bounds.clone());
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
                    //    the `Recommended` ring.
                    //
                    //    §2.6 rule 2: the vendor data is a RANGE or a VALUE, not a
                    //    verdict, so these lines take a chart series colour and not
                    //    the `OK` green.
                    if let Some(cl) = suggested {
                        plot_ui.line(
                            Line::new(
                                "",
                                iso_line(cl, env.spindle_max_rpm, flutes, env.max_feed_mm_min),
                            )
                            .color(chart::BAND)
                            .width(SUGGESTED_LINE_WIDTH)
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
                    for wall in walls(env, rpm_axis_max, feed_axis_max) {
                        plot_ui.line(
                            Line::new("", PlotPoints::from(wall.to_vec()))
                                .color(wash(tokens::DANGER, WALL_ALPHA))
                                .width(WALL_WIDTH)
                                .allow_hover(false),
                        );
                    }
                    if let Some(wall) = min_wall(env, feed_axis_max) {
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

                    // 6. The current operating point: the operator's own values.
                    //    Ruling R23: an operator value is not a verdict, so the
                    //    dot is neutral ink, not the danger red (W3k).
                    if let Some(point) = now {
                        plot_ui.points(
                            Points::new("", vec![point])
                                .shape(MarkerShape::Circle)
                                .filled(true)
                                .radius(NOW_RADIUS)
                                .color(tokens::TEXT_STRONG)
                                .allow_hover(false),
                        );
                    }

                    // 7. The recommended operating point (after derates): an
                    //    ACCENT outline ring (operator, 2026-10-31: "a blue outline
                    //    dot"). The current dot shows inside it when the operation
                    //    runs at the recommendation.
                    plot_ui.points(
                        Points::new("", vec![recommended])
                            .shape(MarkerShape::Circle)
                            .filled(false)
                            .radius(RECOMMENDED_RADIUS)
                            .color(tokens::ACCENT)
                            .allow_hover(false),
                    );

                    // 8. The hover readout: one line, the RPM, the feed and the
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
    let mut keep_clear = vec![ring(recommended, RECOMMENDED_RADIUS)];
    keep_clear.extend(now.map(|p| ring(p, NOW_RADIUS)));
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
        push_line(iso_screen(cl), SUGGESTED_LINE_WIDTH, false);
    }
    if let Some((lo, hi)) = band {
        push_line(iso_screen(lo), BAND_LIMIT_WIDTH, true);
        push_line(iso_screen(hi), BAND_LIMIT_WIDTH, true);
    }
    let [rpm_wall, feed_wall] = walls(env, rpm_axis_max, feed_axis_max);
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
        min_wall(env, feed_axis_max).map(|wall| wall.iter().copied().map(at).collect());
    if let Some(line) = &min_line {
        push_line(line.clone(), WALL_WIDTH, false);
    }
    if let Some(t) = target {
        push_line(vec![at(t), at(recommended)], 1.5, false);
    }

    // The labels in priority order: Recommended, the walls, the corridor,
    // Now, Vendor range, Before derate. `MAX_LABELS` of them paint at most.
    let mut specs = vec![ChartLabel {
        text: LABEL_RECOMMENDED,
        color: tokens::ACCENT,
        anchor: LabelAnchor::Mark {
            center: at(recommended),
            radius: RECOMMENDED_RADIUS,
        },
    }];
    specs.push(ChartLabel {
        text: LABEL_MAX_FEED,
        color: tokens::DANGER,
        anchor: LabelAnchor::Line(feed_wall.iter().copied().map(at).collect()),
    });
    specs.push(ChartLabel {
        text: LABEL_MAX_RPM,
        color: tokens::DANGER,
        anchor: LabelAnchor::Line(rpm_wall.iter().copied().map(at).collect()),
    });
    if let Some(l) = floor_line {
        specs.push(ChartLabel {
            text: LABEL_RUBBING,
            color: tokens::DANGER,
            anchor: LabelAnchor::Line(l),
        });
    }
    if let Some(l) = ceiling_line {
        specs.push(ChartLabel {
            text: LABEL_CHIP_LIMIT,
            color: tokens::DANGER,
            anchor: LabelAnchor::Line(l),
        });
    }
    if let Some(line) = min_line {
        specs.push(ChartLabel {
            text: LABEL_MIN_RPM,
            color: tokens::CAUTION,
            anchor: LabelAnchor::Line(line),
        });
    }
    // The current dot takes a label only where it does not sit in the ring.
    // It ranks under the limits (lead, 2026-10-31): on a crowded chart the
    // white dot keeps its place and its label drops first.
    if let Some(point) = now
        && at(point).distance(at(recommended)) > RECOMMENDED_RADIUS + NOW_RADIUS
    {
        specs.push(ChartLabel {
            text: LABEL_NOW,
            color: tokens::TEXT_STRONG,
            anchor: LabelAnchor::Mark {
                center: at(point),
                radius: NOW_RADIUS,
            },
        });
    }
    // The vendor range: ONE label for its two lines, beside either one
    // (the lower line first: the space under it is open more often).
    if let Some((lo, hi)) = band {
        specs.push(ChartLabel {
            text: LABEL_VENDOR_RANGE,
            color: chart::BAND,
            anchor: LabelAnchor::Lines(vec![iso_screen(lo), iso_screen(hi)]),
        });
    }
    if let Some(t) = target {
        specs.push(ChartLabel {
            text: LABEL_BEFORE_DERATE,
            color: tokens::TEXT_MUTED,
            anchor: LabelAnchor::Mark {
                center: at(t),
                radius: TARGET_RADIUS,
            },
        });
    }
    let font = egui::FontId::proportional(tokens::SIZE_CAPTION);
    let sized: Vec<(ChartLabel, egui::Vec2)> = specs
        .into_iter()
        .map(|spec| {
            let size = ui
                .painter()
                .layout_no_wrap(spec.text.to_owned(), font.clone(), spec.color)
                .size();
            (spec, size + 2.0 * LABEL_PADDING)
        })
        .collect();
    let mut placed = layout_labels(frame, &keep_clear, &lines, &sized);
    placed.truncate(MAX_LABELS);
    let painter = ui.painter_at(frame);
    for label in &placed {
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
        let _ = painter.add(egui::Shape::convex_polygon(
            label.quad.corners().to_vec(),
            wash(tokens::SURFACE_OVERLAY, LABEL_BACKGROUND_ALPHA),
            egui::Stroke::NONE,
        ));
        let galley = painter.layout_no_wrap(label.text.to_owned(), font.clone(), label.color);
        let pos = label.quad.center - galley.size() / 2.0;
        let _ = painter.add(
            egui::epaint::TextShape::new(pos, galley, label.color)
                .with_angle_and_anchor(label.quad.angle, egui::Align2::CENTER_CENTER),
        );
    }
    #[cfg(test)]
    tests::record(frame, &placed, font.size);

    // W3k: the hover is one line at the pointer, only on the axes.
    if let Some(text) = hover_readout {
        let _ = plot_response.response.on_hover_ui_at_pointer(|ui| {
            ui.add(
                egui::Label::new(egui::RichText::new(text).small().color(tokens::TEXT_STRONG))
                    .extend(),
            );
        });
    }
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

/// The two machine walls, in plot coordinates: the spindle maximum (an
/// upright line to the top of the feed axis) and the feed maximum (a level
/// line to the end of the RPM axis).
/// The spindle minimum wall, in plot coordinates: an upright line at the
/// machine's minimum RPM to the top of the feed axis. `None` for a machine
/// with no minimum.
fn min_wall(
    env: &rs_cam_core::feeds::MachineEnvelope,
    feed_axis_max: f64,
) -> Option<[[f64; 2]; 2]> {
    (env.spindle_min_rpm > 0.0).then_some([
        [env.spindle_min_rpm, 0.0],
        [env.spindle_min_rpm, feed_axis_max],
    ])
}

fn walls(
    env: &rs_cam_core::feeds::MachineEnvelope,
    rpm_axis_max: f64,
    feed_axis_max: f64,
) -> [[[f64; 2]; 2]; 2] {
    [
        [
            [env.spindle_max_rpm, 0.0],
            [env.spindle_max_rpm, feed_axis_max],
        ],
        [
            [0.0, env.max_feed_mm_min],
            [rpm_axis_max, env.max_feed_mm_min],
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

/// The gap between a label and its line or mark, in points.
const LABEL_GAP: f32 = 3.0;

/// The padding of a label's background, in points.
const LABEL_PADDING: egui::Vec2 = egui::Vec2::new(2.0, 1.0);

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
const MAX_LABELS: usize = 7;

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

/// One label to place: its text, its colour and what it names.
struct ChartLabel {
    text: &'static str,
    color: egui::Color32,
    anchor: LabelAnchor,
}

/// What a label names, in screen space.
enum LabelAnchor {
    /// A line: the label sits beside a point of it, off the line by its
    /// gap, on the side that is clear.
    Line(Vec<egui::Pos2>),
    /// A mark: the label sits beside it, clear of its radius.
    Mark { center: egui::Pos2, radius: f32 },
    /// Any one of several lines (the two lines of the vendor range): the
    /// places of the first, then of the next.
    Lines(Vec<Vec<egui::Pos2>>),
}

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
    pub(crate) text: &'static str,
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
fn line_hits_quad(line: &DrawnLine, quad: &Quad) -> bool {
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
fn layout_labels(
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
fn greedy_layout(
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
        let fits = |quad: &Quad, cross_soft: bool| {
            quad.inside(inside)
                && !keep_clear
                    .iter()
                    .any(|r| quad.intersects(&Quad::upright(*r)))
                && !lines
                    .iter()
                    .any(|l| !(cross_soft && l.soft) && line_hits_quad(l, quad))
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
            .or_else(|| all.iter().find(|q| fits(q, true)))
            .copied();
        if let Some(quad) = found {
            placed.push((
                i,
                PlacedLabel {
                    text: label.text,
                    color: label.color,
                    quad,
                    mark: match label.anchor {
                        LabelAnchor::Mark { center, radius } => Some((center, radius)),
                        LabelAnchor::Line(_) | LabelAnchor::Lines(_) => None,
                    },
                    line: match &label.anchor {
                        LabelAnchor::Mark { .. } => None,
                        LabelAnchor::Line(points) => Some(points.clone()),
                        LabelAnchor::Lines(lines) => lines
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
fn candidates(anchor: &LabelAnchor, size: egui::Vec2) -> Vec<Quad> {
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
