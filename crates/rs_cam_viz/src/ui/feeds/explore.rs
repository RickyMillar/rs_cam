//! Explore — the feed-versus-RPM nomogram.
//!
//! One chart, answering one question: *if I move feed and RPM, where do I
//! land relative to the vendor band and my machine's limits?* A nomogram you
//! open, drag and close is a focused tool, which is a legitimate use of a
//! window. The scope is one operation.
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
//! reference data, and the inspector's `Vendor Cutting Data` table already
//! presents it in the right form — 174 rows, real columns, collapsed by
//! default.
//!
//! They were also the two that did not work. Chart B drew an EMPTY frame
//! with a y-axis running −0.1 to −0.5 while its series held 36 real points
//! at y 0.028–0.417: it pinned no bounds, and `egui_plot` persists a bad
//! auto-range per plot id for the life of the session. Chart A painted its
//! title over its own axis label, collided its point labels, and drew series
//! outside its frame. Evidence:
//! `planning/feeds_rework_2026-09-15/FINDINGS.md`, F-4 to F-6.

use egui_plot::{Line, MarkerShape, Plot, PlotPoints, Points, Polygon};
use rs_cam_core::feeds::FeedsExplain;

use super::shared::{CurrentValues, chart, draw_machine_envelope, vendor_band, wash};
use crate::state::AppState;
use crate::ui::{AppEvent, theme, tokens};
use crate::ui_command::UiCommand;

/// Ranges that belong to an axis are drawn ON that axis, at this weight.
///
/// The machine's limits and the vendor's RPM window are facts about one
/// axis. Drawn into the plot body they were a labelled hexagon and a
/// full-height wash, both competing with the vendor band for the same
/// pixels.
pub(crate) const AXIS_MARK_WIDTH: f32 = 4.0;

pub(crate) fn draw_spindle_strategy_row(
    ui: &mut egui::Ui,
    current: rs_cam_core::feeds::SpindleStrategy,
    machine: &rs_cam_core::machine::MachineProfile,
    events: &mut Vec<AppEvent>,
) {
    use rs_cam_core::feeds::SpindleStrategy;
    let (_, max_rpm) = machine.rpm_range();
    ui.horizontal(|ui| {
        ui.add(
            egui::Label::new(
                egui::RichText::new("Spindle policy:")
                    .small()
                    .color(theme::TEXT_DIM),
            )
            .wrap(),
        );
        let mut next = current;
        // MatchChart radio
        if ui
            .radio(current == SpindleStrategy::MatchChart, "Match chart")
            .on_hover_text(
                "Use the LUT row's chart-published RPM verbatim. \
                 Tightest match to the vendor band's own test conditions.",
            )
            .clicked()
        {
            next = SpindleStrategy::MatchChart;
        }
        if ui
            .radio(
                current == SpindleStrategy::MaxSpeed,
                "Max speed (constant advance/tooth)",
            )
            .on_hover_text(format!(
                "Push RPM up to the spindle ceiling ({:.0} RPM × \
                 {:.0}% headroom), scaling feed proportionally to \
                 keep the commanded advance/tooth constant. Capped by vendor rpm_max \
                 when published, and by a {:.1}× hard limit over the \
                 chart RPM. Power-limit derate still applies on top \
                 — if the higher operating point exceeds spindle \
                 power, feed comes back down.",
                max_rpm,
                rs_cam_core::feeds::SPINDLE_CEILING_HEADROOM * 100.0,
                rs_cam_core::feeds::MAX_SPINDLE_SPEEDUP,
            ))
            .clicked()
        {
            next = SpindleStrategy::MaxSpeed;
        }
        if next != current {
            events.push(AppEvent::SetSpindleStrategy(next));
        }
        ui.add_space(8.0);
        ui.add(
            egui::Label::new(
                egui::RichText::new(format!("(spindle max {:.0} RPM)", max_rpm))
                    .small()
                    .color(theme::TEXT_DIM),
            )
            .wrap(),
        );
    });
}

/// Draw the modal's single Explore scope: spindle policy and charts.
pub(crate) fn draw_modal_body(
    ui: &mut egui::Ui,
    state: &AppState,
    toolpath_id: crate::state::toolpath::ToolpathId,
    modal: &crate::state::FeedsModalState,
    events: &mut Vec<AppEvent>,
) {
    draw_spindle_strategy_row(
        ui,
        state.session.post_config().spindle_strategy,
        state.session.machine(),
        events,
    );
    ui.separator();
    let Some(preview) = super::compare::compute_preview(state, toolpath_id) else {
        return;
    };
    let Some(current) = super::compare::read_current_values(state, toolpath_id) else {
        return;
    };
    draw_chart_c(
        ui,
        &current,
        preview.explain(),
        preview.refusal(),
        toolpath_id,
        modal,
        events,
    );
}

// ────────────────────────────────────────────────────────────────────
// Chart C — Feed vs RPM (the nomogram)
// ────────────────────────────────────────────────────────────────────

/// The chart's claim, in a sentence, above the chart.
///
/// The operator described the reading they want to get from this window:
///
/// > "Ah yep, vendor says this chipload, we can ramp it up. But it has
/// > recommended slightly less load due to machine being hobby grade."
///
/// That is a sentence, and a reader should not have to assemble it from
/// three marks and a legend. The chart shows WHERE; this line says WHAT and
/// WHY, and it is the first thing under the heading.
///
/// It also carries the surface's honest abstention. When no vendor row
/// matched there is no band to read, and a chart with no band and no
/// sentence looks like a chart whose band is merely off screen.
fn draw_headline(ui: &mut egui::Ui, explain: &FeedsExplain) {
    let r = &explain.recommended;
    let flutes = explain.flute_count.max(1) as f64;
    let effective = if r.rpm > 0.0 {
        r.feed_rate_mm_min / (r.rpm * flutes)
    } else {
        0.0
    };
    let band = vendor_band(explain);

    let mut text = match band {
        Some((lo, hi)) => {
            format!("Vendor {lo:.4}\u{2013}{hi:.4} mm/tooth · running {effective:.4}")
        }
        None => format!("No vendor range for this cut · running {effective:.4} mm/tooth"),
    };

    // The derate is the "why" half of the sentence, and it is only worth a
    // clause when it actually moved the number.
    let combined = r.derates.combined_factor();
    let pct = ((1.0 - combined) * 100.0).max(0.0);
    let color = if pct >= 1.0 {
        let mut reasons: Vec<&str> = Vec::new();
        if r.derates.workholding < 0.999 {
            reasons.push("workholding");
        }
        if r.derates.ld_overhang < 0.999 {
            reasons.push("tool overhang");
        }
        if r.derates.depth_tier < 0.999 {
            reasons.push("cut depth");
        }
        if r.derates.power_limit < 0.999 {
            reasons.push("spindle power");
        }
        if r.derates.feed_clamp < 0.999 {
            reasons.push("machine feed cap");
        }
        if r.derates.safety_factor < 0.999 {
            reasons.push("machine safety margin");
        }
        let why = if reasons.is_empty() {
            "machine limits".to_owned()
        } else {
            reasons.join(", ")
        };
        text.push_str(&format!(" · held back {pct:.0}% for {why}"));
        theme::WARNING_MILD
    } else {
        text.push_str(" · nothing held it back");
        theme::TEXT_DIM
    };

    ui.add(egui::Label::new(egui::RichText::new(text).small().color(color)).wrap());
}

pub(crate) fn draw_chart_c(
    ui: &mut egui::Ui,
    current: &CurrentValues,
    explain: &FeedsExplain,
    refusal: Option<&rs_cam_core::feeds::FeedsError>,
    toolpath_id: crate::state::toolpath::ToolpathId,
    modal: &crate::state::FeedsModalState,
    events: &mut Vec<AppEvent>,
) {
    ui.label(
        egui::RichText::new("Feed vs RPM (drag the red point to explore)")
            .strong()
            .color(theme::TEXT_STRONG),
    );
    draw_headline(ui, explain);
    ui.add_space(2.0);
    let flutes = explain.flute_count.max(1) as f64;
    let env = &explain.machine;

    // RPM/Feed axis bounds — show a bit past the machine cap so the
    // wall is visible.
    let rpm_axis_max = env.spindle_max_rpm * 1.15;
    let feed_axis_max = env.max_feed_mm_min * 1.05;

    // Vendor band — three iso-chipload diagonals (min, mid, max).
    let band = explain.matched_row.as_ref().and_then(|row| {
        match (row.chip_load_min_mm, row.chip_load_max_mm) {
            (Some(lo), Some(hi)) => Some((lo, hi)),
            (None, Some(hi)) => Some((hi * 0.7, hi)),
            _ => None,
        }
    });

    // Vendor RPM column.
    let vendor_rpm = explain
        .matched_row
        .as_ref()
        .map(|row| (row.rpm_min, row.rpm_max, row.rpm_nominal));

    // Compute drag-to-explore preview point. We capture the operating
    // point from `modal.explore` if set, otherwise show the current op.
    let explore = modal.explore;
    let display_point = explore.unwrap_or(crate::state::NomogramExplore {
        rpm: current.spindle_rpm.map(f64::from).unwrap_or(0.0),
        feed_mm_min: current.feed_rate_mm_min,
    });

    let mut hover_readout: Option<(String, egui::Color32)> = None;
    let plot_response = Plot::new("feeds_modal_nomogram")
        .height(280.0)
        .include_x(0.0)
        .include_y(0.0)
        .include_x(rpm_axis_max)
        .include_y(feed_axis_max)
        .x_axis_label("RPM")
        .y_axis_label("feed (mm/min)")
        .allow_drag(false)
        .allow_zoom(false)
        .allow_scroll(false)
        .show(ui, |plot_ui| {
            // 1. Band wedge polygon — clipped at machine RPM cap and
            //    feed cap so the band visually stops at the wall.
            // §2.6 rule 2: the vendor band is a RANGE, not a verdict. It
            // shared its green with `OK` at six sites and four alphas, so a
            // reader could not tell the band from a pass. It is one chart
            // series now.
            if let Some((lo, hi)) = band {
                let band_poly =
                    wedge_polygon(lo, hi, env.spindle_max_rpm, flutes, env.max_feed_mm_min);
                plot_ui.polygon(
                    Polygon::new("", band_poly)
                        .fill_color(wash(chart::BAND, 50))
                        .stroke(egui::Stroke::new(1.0_f32, wash(chart::BAND, 120)))
                        .name(format!("Vendor band {lo:.4}–{hi:.4} mm/tooth")),
                );
                // W6: the inline "VENDOR BAND 0.0500–0.0850 mm advance/tooth"
                // text that used to sit here is deleted. It restated the
                // legend's first row word for word, and it was painted at a
                // fixed fraction of the band's own extent, so on a wide band
                // it landed on top of the iso-advance lines it was labelling.
            }
            // 2. The band's EDGES are the min and max iso-advance lines,
            //    and its midpoint is the mid line. Drawing all three on top
            //    of the wedge drew the same fact three times, in a
            //    three-step colour scale, under a name the product owner
            //    could not read: "I don't really know what the iso advance
            //    and similar is" (2026-09-16).
            //
            //    The ±5 % admitted ribbons went with them. They are the
            //    optimizer's tolerance, which is detail about a different
            //    tool's behaviour, not about this cut. The band's legend row
            //    carries the numbers.
            //
            //    What is left is one region: the vendor's chipload range.

            // 3. Machine envelope — shaded forbidden zones, walls, labels.
            draw_machine_envelope(plot_ui, env, rpm_axis_max, feed_axis_max);

            // 4. The vendor's RPM window, marked ON the RPM axis.
            //
            //    It used to be a full-height column washed across the plot,
            //    a second translucent region competing with the band for the
            //    same pixels. A range along one axis is a fact about that
            //    axis: "the max and min lines should be on the axis, not
            //    marked on the chart as points" (2026-09-16).
            if let Some((Some(lo), Some(hi), _)) = vendor_rpm {
                plot_ui.line(
                    Line::new("", PlotPoints::from(vec![[lo, 0.0], [hi, 0.0]]))
                        .color(wash(chart::RPM_RANGE, 220))
                        .width(AXIS_MARK_WIDTH)
                        .name(format!("Vendor RPM {lo:.0}–{hi:.0}")),
                );
            }

            // 5. Current operating point.
            if let Some(rpm) = current.spindle_rpm {
                plot_ui.points(
                    Points::new("", vec![[f64::from(rpm), current.feed_rate_mm_min]])
                        .shape(MarkerShape::Circle)
                        .filled(true)
                        .radius(6.0_f32)
                        .color(theme::ERROR)
                        .name("Current"),
                );
            }
            // Target (pre-derate) operating point. Shows where the
            // recommendation *would* sit if the safety factor and
            // overhang / workholding / power / feed-clamp derates
            // weren't applied. The arrow from target → recommended
            // makes the derate visually obvious.
            let target_chipload = explain.recommended.derates.target_chip_load_mm;
            if target_chipload > 0.0 && explain.recommended.rpm > 0.0 {
                let target_feed = target_chipload * explain.recommended.rpm * flutes;
                if (target_feed - explain.recommended.feed_rate_mm_min).abs() > 1.0 {
                    plot_ui.points(
                        Points::new("", vec![[explain.recommended.rpm, target_feed]])
                            .shape(MarkerShape::Circle)
                            .filled(false)
                            .radius(7.0_f32)
                            .color(tokens::DIAGRAM_INK)
                            .name(format!("Target pre-derate ({target_chipload:.4} mm/tooth)")),
                    );
                    // Arrow from target down to derated recommendation.
                    plot_ui.line(
                        Line::new(
                            "",
                            PlotPoints::from(vec![
                                [explain.recommended.rpm, target_feed],
                                [
                                    explain.recommended.rpm,
                                    explain.recommended.feed_rate_mm_min,
                                ],
                            ]),
                        )
                        .color(wash(tokens::DIAGRAM_INK, 180))
                        .style(egui_plot::LineStyle::Dashed { length: 4.0 })
                        .width(1.5_f32)
                        .name("derate chain"),
                    );
                    // Inline label on the derate arrow.
                    let mid_feed = (target_feed + explain.recommended.feed_rate_mm_min) * 0.5;
                    let derate_pct = (1.0 - explain.recommended.derates.combined_factor()) * 100.0;
                    plot_ui.text(
                        egui_plot::Text::new(
                            "",
                            egui_plot::PlotPoint::new(explain.recommended.rpm, mid_feed),
                            egui::RichText::new(format!(" −{derate_pct:.0}% derate"))
                                .small()
                                .color(wash(tokens::DIAGRAM_INK, 220)),
                        )
                        .anchor(egui::Align2::LEFT_CENTER),
                    );
                }
            }

            // Recommended operating point (after derates).
            plot_ui.points(
                Points::new(
                    "",
                    vec![[
                        explain.recommended.rpm,
                        explain.recommended.feed_rate_mm_min,
                    ]],
                )
                .shape(MarkerShape::Diamond)
                .filled(true)
                .radius(6.0_f32)
                .color(tokens::DIAGRAM_INK)
                .name("Recommended (after derates)"),
            );

            // 6. Drag-to-explore overlay (Phase 3).
            if explore.is_some() {
                plot_ui.points(
                    Points::new("", vec![[display_point.rpm, display_point.feed_mm_min]])
                        .shape(MarkerShape::Cross)
                        .filled(true)
                        .radius(8.0_f32)
                        .color(chart::EXPLORE)
                        .name("Explore"),
                );
                // Proposed-move line from current → explore.
                if let Some(rpm) = current.spindle_rpm {
                    plot_ui.line(
                        Line::new(
                            "",
                            PlotPoints::from(vec![
                                [f64::from(rpm), current.feed_rate_mm_min],
                                [display_point.rpm, display_point.feed_mm_min],
                            ]),
                        )
                        .color(wash(chart::EXPLORE, 180))
                        .style(egui_plot::LineStyle::Dashed { length: 5.0 })
                        .width(1.2_f32)
                        .name("proposal"),
                    );
                }
            }

            // 7. Hover readout — shows the resulting chipload, the
            //    chart bucket the hover lands in (within / admit /
            //    BURN / BREAK), and the machine-cap clearance at the
            //    pointer's RPM/feed. Same text as the verdict line so
            //    the user can learn the chart by hovering.
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
                let (verdict, color) = chipload_verdict(cl, explain);
                let cap_note = if rpm > env.spindle_max_rpm {
                    " · past spindle cap".to_owned()
                } else if feed > env.max_feed_mm_min {
                    " · past feed cap".to_owned()
                } else if rpm < env.spindle_min_rpm {
                    " · below spindle min".to_owned()
                } else {
                    String::new()
                };
                // W6: the readout is RETURNED, not painted into the plot.
                //
                // It is a two-line sentence with up to four clauses, and it
                // was anchored inside the drawing at a fixed fraction of the
                // axes. At the top-left of this chart that is the feed wall
                // and the forbidden zone above it, so the sentence printed
                // straight through them. A sentence that long is prose, and
                // prose belongs on a line of its own, under the picture.
                hover_readout = Some((
                    format!(
                        "{rpm:.0} RPM · {feed:.0} mm/min → commanded advance/tooth \
                         {cl:.4} mm/tooth · {verdict}{cap_note}"
                    ),
                    color,
                ));
            }
        });

    // The readout line, under the plot and above the legend. It holds its
    // height whether or not the pointer is over the chart, so the legend
    // below does not jump as the pointer crosses the plot edge.
    ui.add(
        egui::Label::new(match &hover_readout {
            Some((text, color)) => egui::RichText::new(text).small().color(*color),
            None => egui::RichText::new("Hover the chart to read an operating point.")
                .small()
                .color(theme::TEXT_FAINT),
        })
        .wrap(),
    );

    // Band legend below the chart — colour swatch + numeric range for
    // every overlay on the plot. This is the single best lever for
    // "what does the green/yellow/blue/red mean".
    draw_chart_c_legend(ui, current, explain);

    // Click inside the plot sets the explore point. Egui_plot only
    // surfaces the pointer coordinate while the response is hovered;
    // we route a single click on press → release.
    if explore.is_some()
        && plot_response.response.clicked()
        && let Some(coord) = plot_response.response.interact_pointer_pos()
    {
        // Translate screen-space click back to plot coordinates.
        let plot_coord = plot_response.transform.value_from_position(coord);
        let new_rpm = plot_coord.x.clamp(env.spindle_min_rpm, env.spindle_max_rpm);
        let new_feed = plot_coord.y.clamp(0.0, env.max_feed_mm_min);
        events.push(AppEvent::Ui(UiCommand::SetFeedsExplore(Some(
            crate::state::NomogramExplore {
                rpm: new_rpm,
                feed_mm_min: new_feed,
            },
        ))));
    }

    // Phase 3 — Explore controls.
    draw_explore_controls(ui, current, explain, refusal, toolpath_id, modal, events);
}

/// Band legend rendered under Chart C. Each row is `[swatch] label —
/// numeric range`. Compact, fits in two columns.
fn draw_chart_c_legend(ui: &mut egui::Ui, current: &CurrentValues, explain: &FeedsExplain) {
    let env = &explain.machine;
    let band = explain.matched_row.as_ref().and_then(|row| {
        match (row.chip_load_min_mm, row.chip_load_max_mm) {
            (Some(lo), Some(hi)) => Some((lo, hi)),
            (None, Some(hi)) => Some((hi * 0.7, hi)),
            _ => None,
        }
    });
    let vendor_rpm = explain
        .matched_row
        .as_ref()
        .map(|row| (row.rpm_min, row.rpm_max, row.rpm_nominal));

    let mut entries: Vec<LegendEntry> = Vec::new();

    if let Some((lo, hi)) = band {
        // One row for one region.
        //
        // Three `iso-advance min / mid / max` rows used to follow it. They
        // named the band's own edges and midpoint — the same fact the wedge
        // already draws — in a term the product owner could not read: "I
        // don't really know what the iso advance and similar is". A fourth
        // row named the optimizer's ±5 % admit window, which is detail about
        // a different tool. It survives as a clause here, where it costs no
        // row of its own.
        entries.push(LegendEntry::new(
            LegendSwatch::FilledSquare,
            wash(chart::BAND, 200),
            "Vendor range",
            format!("{lo:.4}\u{2013}{hi:.4} mm/tooth (\u{00B1}5 % accepted)"),
        ));
    }

    if let Some((lo, hi, nominal)) = vendor_rpm {
        let value = match (lo, hi, nominal) {
            (Some(lo), Some(hi), _) => format!("{lo:.0}–{hi:.0} RPM"),
            (None, None, Some(nom)) => format!("{nom:.0} RPM (nominal)"),
            (Some(v), None, _) | (None, Some(v), _) => format!("{v:.0} RPM"),
            _ => "—".to_owned(),
        };
        entries.push(LegendEntry::new(
            LegendSwatch::FilledSquare,
            wash(chart::RPM_RANGE, 200),
            "Vendor RPM",
            value,
        ));
    }

    entries.push(LegendEntry::new(
        LegendSwatch::FilledSquare,
        wash(tokens::DANGER, 150),
        "Machine limit",
        // ui-string-columns: air either side of "or" separates two limits
        // in a legend swatch; a single space runs them together.
        format!(
            "> {} RPM   or   > {} mm/min",
            env.spindle_max_rpm as i64, env.max_feed_mm_min as i64
        ),
    ));
    if env.spindle_min_rpm > 0.0 {
        entries.push(LegendEntry::new(
            LegendSwatch::FilledSquare,
            wash(tokens::CAUTION, 150),
            "Under spindle min",
            format!("< {} RPM", env.spindle_min_rpm as i64),
        ));
    }
    if let Some(rpm) = current.spindle_rpm {
        entries.push(LegendEntry::new(
            LegendSwatch::Circle,
            // Ruling R23: the operator's own value is not a verdict.
            crate::ui::tokens::TEXT_STRONG,
            "● Now",
            format!("{rpm} RPM · {:.0} mm/min", current.feed_rate_mm_min),
        ));
    }
    let target_cl = explain.recommended.derates.target_chip_load_mm;
    if target_cl > 0.0 && explain.recommended.rpm > 0.0 {
        let target_feed = target_cl * explain.recommended.rpm * explain.flute_count.max(1) as f64;
        if (target_feed - explain.recommended.feed_rate_mm_min).abs() > 1.0 {
            entries.push(LegendEntry::new(
                LegendSwatch::Circle,
                tokens::DIAGRAM_INK,
                "○ Vendor target",
                format!("{target_cl:.4} mm/tooth · {target_feed:.0} mm/min"),
            ));
        }
    }
    entries.push(LegendEntry::new(
        LegendSwatch::Diamond,
        tokens::DIAGRAM_INK,
        "◆ Recommended",
        format!(
            "{:.0} RPM · {:.0} mm/min · {:.4} mm/tooth",
            explain.recommended.rpm,
            explain.recommended.feed_rate_mm_min,
            if explain.recommended.rpm > 0.0 {
                explain.recommended.feed_rate_mm_min
                    / (explain.recommended.rpm * explain.flute_count.max(1) as f64)
            } else {
                0.0
            }
        ),
    ));

    ui.add_space(4.0);
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(
            egui::RichText::new("What the colours mean")
                .small()
                .strong()
                .color(theme::TEXT_DIM),
        );
        ui.add_space(2.0);
        for entry in &entries {
            legend_row(ui, entry);
        }
    });
}

/// One legend line: `swatch · label · value`.
///
/// # Why this is not a `Grid`
///
/// It was, and it broke badly enough that the operator called the window
/// "cooked": the legend rendered `vendor band` as
///
/// ```text
/// ven
/// dor
/// ban
/// d
/// ```
///
/// one letter per line. A `Grid` cell has no known width on its first layout
/// pass, and the two obvious wrap modes are BOTH wrong there:
///
/// - `TextWrapMode::Extend`, the Grid cell default, asks for INFINITE width.
///   That is the UR1 defect this crate bans — the content widens its own
///   container until the panel clips it.
/// - `.wrap()`, the obvious correction, has no width to wrap against, so it
///   collapses to the narrowest legal break. For a long token that is one
///   character.
///
/// There is no third wrap mode that fixes it, because the problem is the
/// column negotiation, not the label. A legend is a list of lines, so it is
/// laid out as a list of lines: one `horizontal_wrapped` per entry, wrapping
/// against the frame's real width.
fn legend_row(ui: &mut egui::Ui, entry: &LegendEntry) {
    ui.horizontal_wrapped(|ui| {
        draw_legend_swatch(ui, entry.swatch, entry.color);
        ui.add(
            egui::Label::new(
                egui::RichText::new(entry.label)
                    .small()
                    .color(theme::TEXT_STRONG),
            )
            .wrap(),
        );
        ui.add(
            egui::Label::new(
                egui::RichText::new(&entry.value)
                    .small()
                    .color(theme::TEXT_DIM),
            )
            .wrap(),
        );
    });
}

#[derive(Debug, Clone, Copy)]
enum LegendSwatch {
    FilledSquare,
    Circle,
    Diamond,
}

struct LegendEntry {
    swatch: LegendSwatch,
    color: egui::Color32,
    label: &'static str,
    value: String,
}

impl LegendEntry {
    fn new(swatch: LegendSwatch, color: egui::Color32, label: &'static str, value: String) -> Self {
        Self {
            swatch,
            color,
            label,
            value,
        }
    }
}

fn draw_legend_swatch(ui: &mut egui::Ui, swatch: LegendSwatch, color: egui::Color32) {
    let size = egui::vec2(14.0, 10.0);
    let (rect, _resp) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter();
    match swatch {
        LegendSwatch::FilledSquare => {
            painter.rect_filled(rect, 2.0, color);
            painter.rect_stroke(
                rect,
                2.0,
                egui::Stroke::new(0.5_f32, color.linear_multiply(1.4)),
                egui::StrokeKind::Middle,
            );
        }
        LegendSwatch::Circle => {
            painter.circle_filled(rect.center(), 4.0, color);
        }
        LegendSwatch::Diamond => {
            let c = rect.center();
            let r = 5.0;
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(c.x, c.y - r),
                    egui::pos2(c.x + r, c.y),
                    egui::pos2(c.x, c.y + r),
                    egui::pos2(c.x - r, c.y),
                ],
                color,
                egui::Stroke::NONE,
            ));
        }
    }
}

/// Build the polygon for a chipload band wedge between `cl_lo` and
/// `cl_hi`, clipped to the chart's RPM and feed extents.
fn wedge_polygon(
    cl_lo: f64,
    cl_hi: f64,
    rpm_max: f64,
    flutes: f64,
    feed_cap: f64,
) -> PlotPoints<'static> {
    // The wedge has two sides:
    //   lower: feed = cl_lo * rpm * flutes
    //   upper: feed = cl_hi * rpm * flutes
    // We clip both at rpm_max and feed_cap.
    let lo_line = clip_iso_line(cl_lo, rpm_max, flutes, feed_cap);
    let hi_line = clip_iso_line(cl_hi, rpm_max, flutes, feed_cap);
    // Stitch into a polygon: low-line forward, high-line reversed.
    let mut pts: Vec<[f64; 2]> = lo_line;
    pts.extend(hi_line.into_iter().rev());
    PlotPoints::from(pts)
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
        out.push([rpm_cross, feed_cap]);
        out.push([rpm_max, feed_cap]);
    }
    out
}

// ── Drag-to-explore controls (Phase 3) ──────────────────────────────

fn draw_explore_controls(
    ui: &mut egui::Ui,
    current: &CurrentValues,
    explain: &FeedsExplain,
    refusal: Option<&rs_cam_core::feeds::FeedsError>,
    toolpath_id: crate::state::toolpath::ToolpathId,
    modal: &crate::state::FeedsModalState,
    events: &mut Vec<AppEvent>,
) {
    ui.add_space(4.0);
    let env = &explain.machine;
    let flutes = explain.flute_count.max(1) as f64;
    let active = modal.explore.is_some();
    let mut explore = modal.explore.unwrap_or(crate::state::NomogramExplore {
        rpm: current
            .spindle_rpm
            .map(f64::from)
            .unwrap_or(explain.recommended.rpm),
        feed_mm_min: current.feed_rate_mm_min.max(1.0),
    });
    let initial = explore;
    ui.horizontal(|ui| {
        if !active && ui.button("⊕ Start exploring").clicked() {
            // Activate explore at the current point.
            events.push(AppEvent::Ui(UiCommand::SetFeedsExplore(Some(explore))));
            return;
        }
        if active {
            ui.add(
                egui::Label::new(
                    egui::RichText::new("Explore:")
                        .small()
                        .strong()
                        .color(theme::TEXT_STRONG),
                )
                .wrap(),
            );
            ui.add(
                egui::Slider::new(&mut explore.rpm, env.spindle_min_rpm..=env.spindle_max_rpm)
                    .text("RPM")
                    .integer(),
            );
            ui.add(
                egui::Slider::new(&mut explore.feed_mm_min, 0.0..=env.max_feed_mm_min)
                    .text("feed mm/min")
                    .step_by(10.0),
            );
        }
    });
    if active {
        let preview_chipload = if explore.rpm > 0.0 {
            explore.feed_mm_min / (explore.rpm * flutes)
        } else {
            0.0
        };
        let (verdict_text, color) = chipload_verdict(preview_chipload, explain);
        let preview_power = preview_power_kw(explain, explore.feed_mm_min);
        let power_pct = if env.max_power_kw > 0.0 {
            (preview_power / env.max_power_kw).clamp(0.0, 2.0) * 100.0
        } else {
            0.0
        };
        ui.horizontal(|ui| {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(format!(
                        "→ commanded advance/tooth {preview_chipload:.4} mm/tooth · {verdict_text}"
                    ))
                    .small()
                    .color(color),
                )
                .wrap(),
            );
            ui.separator();
            ui.add(
                egui::Label::new(
                    egui::RichText::new(format!(
                        "power {preview_power:.2} kW ({power_pct:.0}% of cap)"
                    ))
                    .small()
                    .color(if power_pct > 90.0 {
                        theme::ERROR
                    } else if power_pct > 70.0 {
                        theme::WARNING_MILD
                    } else {
                        theme::SUCCESS
                    }),
                )
                .wrap(),
            );
        });
        ui.horizontal(|ui| {
            // I-3: the explore apply is an apply affordance like any other, so
            // a refused pairing replaces it with the refusal rather than
            // offering a write the panel would never offer. Exploring the
            // nomogram stays available — reading it is not writing.
            if let Some(err) = refusal {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!("Cannot apply — {err}"))
                            .small()
                            .color(theme::ERROR),
                    )
                    .wrap(),
                );
            } else if ui
                .button("✓ Apply explored values")
                .on_hover_text(
                    "Overwrite feed and RPM with the explored values, after the safety \
                     clamps (plunge is pulled down to the new feed if it would exceed it). \
                     Does not change the cut (DOC/WOC). The explored feed itself is never \
                     re-solved.",
                )
                .clicked()
            {
                events.push(AppEvent::ApplyFeedsExplore {
                    toolpath_id,
                    feed_mm_min: explore.feed_mm_min,
                    rpm: explore.rpm,
                });
                events.push(AppEvent::Ui(UiCommand::SetFeedsExplore(None)));
            }
            if ui
                .button("⟲ Reset to current")
                .on_hover_text("Snap explored point back to the current operating values.")
                .clicked()
            {
                let snap = crate::state::NomogramExplore {
                    rpm: current
                        .spindle_rpm
                        .map(f64::from)
                        .unwrap_or(explain.recommended.rpm),
                    feed_mm_min: current.feed_rate_mm_min.max(1.0),
                };
                events.push(AppEvent::Ui(UiCommand::SetFeedsExplore(Some(snap))));
            }
            if ui
                .button("→ Snap to recommended")
                .on_hover_text("Snap explored point to the recommendation.")
                .clicked()
            {
                let snap = crate::state::NomogramExplore {
                    rpm: explain.recommended.rpm,
                    feed_mm_min: explain.recommended.feed_rate_mm_min,
                };
                events.push(AppEvent::Ui(UiCommand::SetFeedsExplore(Some(snap))));
            }
            if ui.button("✕ Close explore").clicked() {
                events.push(AppEvent::Ui(UiCommand::SetFeedsExplore(None)));
            }
        });
        // Persist slider edits — only emit when something actually
        // changed to avoid event spam per frame.
        if (explore.rpm - initial.rpm).abs() > 0.5
            || (explore.feed_mm_min - initial.feed_mm_min).abs() > 0.5
        {
            events.push(AppEvent::Ui(UiCommand::SetFeedsExplore(Some(explore))));
        }
    }
}

/// Estimate power at the explored feed by scaling the recommendation's
/// power linearly with feed. Good enough as an interactive preview —
/// not the same as the calc, but matches the optimizer's first-order
/// model for adaptive ops.
fn preview_power_kw(explain: &FeedsExplain, explore_feed: f64) -> f64 {
    let rec = &explain.recommended;
    if rec.feed_rate_mm_min <= 0.0 {
        return 0.0;
    }
    (rec.power_kw * (explore_feed / rec.feed_rate_mm_min)).max(0.0)
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

fn chipload_verdict(cl: f64, explain: &FeedsExplain) -> (&'static str, egui::Color32) {
    let band = explain.matched_row.as_ref().and_then(|row| {
        match (row.chip_load_min_mm, row.chip_load_max_mm) {
            (Some(lo), Some(hi)) => Some((lo, hi)),
            (None, Some(hi)) => Some((hi * 0.7, hi)),
            _ => None,
        }
    });
    let Some((lo, hi)) = band else {
        return ("no band", theme::TEXT_DIM);
    };
    if cl < lo * 0.95 {
        ("BURN risk (below band)", theme::ERROR)
    } else if cl < lo {
        ("below band (5% admit)", theme::WARNING_MILD)
    } else if cl <= hi {
        ("within band", theme::SUCCESS)
    } else if cl <= hi * 1.05 {
        ("above band (5% admit)", theme::WARNING_MILD)
    } else {
        ("BREAK risk (above band)", theme::ERROR)
    }
}
