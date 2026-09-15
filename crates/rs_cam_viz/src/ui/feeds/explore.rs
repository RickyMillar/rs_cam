//! Explore — the nomogram and the two mini charts.
//!
//! - **Chart A** — advance/tooth vs diameter (the "tool size" axis)
//! - **Chart B** — advance/tooth vs hardness (the "material" axis)
//! - **Chart C** — feed vs RPM at constant advance/tooth (the nomogram)
//!
//! DC5a's plan keeps this a MODAL. A nomogram you open, drag and close is a
//! focused tool, which is a legitimate use of a window. The scope is still
//! one operation.
//!
//! Every quantity these charts plot is a **commanded** advance per tooth,
//! `feed / (rpm · flutes)` — these are pre-simulation surfaces and have no
//! measured value to show (Checkpoint H2, 2026-08-08).

use egui_plot::{Line, MarkerShape, Plot, PlotPoints, Points, Polygon};
use rs_cam_core::feeds::{FeedsExplain, vendor_lut::HardnessKind};

use super::shared::{CurrentValues, chart, draw_machine_envelope, wash};
use crate::state::AppState;
use crate::ui::{AppEvent, theme, tokens};
use crate::ui_command::UiCommand;

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
    ui.add_space(12.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| draw_chart_a(ui, &current, preview.explain()));
        ui.add_space(8.0);
        ui.vertical(|ui| draw_chart_b(ui, &current, preview.explain()));
    });
}

// ────────────────────────────────────────────────────────────────────
// Chart C — Feed vs RPM (the nomogram)
// ────────────────────────────────────────────────────────────────────

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
    let band_admit = band.map(|(lo, hi)| {
        // 5 % tolerance band (matches optimizer's breakage_tolerance default).
        let admit_hi = hi * 1.05;
        let admit_lo = (lo * 0.95).max(0.0);
        (admit_lo, lo, hi, admit_hi)
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
                // Inline label inside the band at the cap intersection
                // so the user can see "VENDOR BAND" on the chart itself.
                let mid_cl = (lo + hi) * 0.5;
                let mid_pts =
                    clip_iso_line(mid_cl, env.spindle_max_rpm, flutes, env.max_feed_mm_min);
                if let Some(end) = mid_pts.last() {
                    plot_ui.text(
                        egui_plot::Text::new(
                            "",
                            egui_plot::PlotPoint::new(end[0] * 0.55, end[1] * 0.5),
                            egui::RichText::new(format!(
                                "VENDOR BAND\n{lo:.4}\u{2013}{hi:.4} mm advance/tooth"
                            ))
                            .small()
                            .color(wash(chart::BAND, 220)),
                        )
                        .anchor(egui::Align2::CENTER_CENTER),
                    );
                }
            }
            if let Some((admit_lo, lo, hi, admit_hi)) = band_admit {
                // Two thinner ribbons in the 5 % tolerance zones —
                // also clipped at the machine caps.
                let low_admit = wedge_polygon(
                    admit_lo,
                    lo,
                    env.spindle_max_rpm,
                    flutes,
                    env.max_feed_mm_min,
                );
                let high_admit = wedge_polygon(
                    hi,
                    admit_hi,
                    env.spindle_max_rpm,
                    flutes,
                    env.max_feed_mm_min,
                );
                // The admitted wedge is a second RANGE beside the band, so
                // it takes the neighbouring series rather than the caution
                // amber it used to borrow.
                let warn = wash(chart::ADMITTED, 50);
                let warn_edge = wash(chart::ADMITTED, 100);
                plot_ui.polygon(
                    Polygon::new("", low_admit)
                        .fill_color(warn)
                        .stroke(egui::Stroke::new(1.0_f32, warn_edge))
                        .name("Band-admitted (low)"),
                );
                plot_ui.polygon(
                    Polygon::new("", high_admit)
                        .fill_color(warn)
                        .stroke(egui::Stroke::new(1.0_f32, warn_edge))
                        .name("Band-admitted (high)"),
                );
            }

            // 2. Three iso-chipload diagonals — each one is labelled
            //    inline at its right-end so the chipload value is
            //    visible without consulting a legend.
            if let Some((lo, hi)) = band {
                let mid = (lo + hi) * 0.5;
                // min / mid / max were amber / green / red — a traffic
                // light on what is an ORDERED SCALE, not three verdicts. The
                // band's own maximum is not an exceedance. Three ascending
                // steps of one scale now carry the order.
                for (cl, color, prefix) in [
                    (lo, chart::ISO_MIN, "min"),
                    (mid, chart::ISO_MID, "mid"),
                    (hi, chart::ISO_MAX, "max"),
                ] {
                    let line_pts =
                        clip_iso_line(cl, env.spindle_max_rpm, flutes, env.max_feed_mm_min);
                    plot_ui.line(
                        Line::new("", PlotPoints::from(line_pts.clone()))
                            .color(color)
                            .width(1.5_f32)
                            .name(format!("chipload {prefix} {cl:.4} mm/tooth")),
                    );
                    // Drop an inline value tag at the last point of the
                    // clipped line. The line ends either at the RPM cap
                    // (sloped) or at the feed cap (horizontal); either
                    // way the last point is in-frame.
                    if let Some(end) = line_pts.last() {
                        plot_ui.text(
                            egui_plot::Text::new(
                                "",
                                egui_plot::PlotPoint::new(end[0], end[1]),
                                egui::RichText::new(format!(" {prefix} {cl:.4}"))
                                    .small()
                                    .color(color),
                            )
                            .anchor(egui::Align2::LEFT_BOTTOM),
                        );
                    }
                }
            }

            // 3. Machine envelope — shaded forbidden zones, walls, labels.
            draw_machine_envelope(plot_ui, env, rpm_axis_max, feed_axis_max);

            // 4. Vendor RPM column.
            if let Some((Some(lo), Some(hi), _)) = vendor_rpm {
                plot_ui.polygon(
                    Polygon::new(
                        "",
                        PlotPoints::from(vec![
                            [lo, 0.0],
                            [hi, 0.0],
                            [hi, feed_axis_max],
                            [lo, feed_axis_max],
                        ]),
                    )
                    .fill_color(wash(chart::RPM_RANGE, 25))
                    .stroke(egui::Stroke::new(1.0_f32, wash(chart::RPM_RANGE, 80)))
                    .name("Vendor RPM range"),
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
            if let Some(hover) = plot_ui.pointer_coordinate() {
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
                let label = format!(
                    "{rpm:.0} RPM · {feed:.0} mm/min\n→ commanded advance/tooth \
                     {cl:.4} mm/tooth · {verdict}{cap_note}",
                );
                // Anchor the readout near the top-left of the chart so
                // it stays out of the band area.
                plot_ui.text(
                    egui_plot::Text::new(
                        "",
                        egui_plot::PlotPoint::new(rpm_axis_max * 0.02, feed_axis_max * 0.97),
                        egui::RichText::new(label)
                            .small()
                            .background_color(tokens::SCRIM)
                            .color(color),
                    )
                    .anchor(egui::Align2::LEFT_TOP),
                );
            }
        });

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
        let mid = (lo + hi) * 0.5;
        entries.push(LegendEntry::new(
            LegendSwatch::FilledSquare,
            wash(chart::BAND, 200),
            "Vendor band",
            format!("{lo:.4}–{hi:.4} mm/tooth"),
        ));
        entries.push(LegendEntry::new(
            LegendSwatch::FilledSquare,
            wash(chart::ADMITTED, 180),
            "+5% tolerance",
            format!(
                "{:.4}–{lo:.4}  ·  {hi:.4}–{:.4} mm/tooth",
                lo * 0.95,
                hi * 1.05
            ),
        ));
        entries.push(LegendEntry::new(
            LegendSwatch::Line,
            chart::ISO_MIN,
            "iso-advance min",
            format!("{lo:.4} mm/tooth"),
        ));
        entries.push(LegendEntry::new(
            LegendSwatch::Line,
            chart::ISO_MID,
            "iso-advance mid",
            format!("{mid:.4} mm/tooth"),
        ));
        entries.push(LegendEntry::new(
            LegendSwatch::Line,
            chart::ISO_MAX,
            "iso-advance max",
            format!("{hi:.4} mm/tooth"),
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
            "Vendor RPM range",
            value,
        ));
    }

    entries.push(LegendEntry::new(
        LegendSwatch::FilledSquare,
        wash(tokens::DANGER, 150),
        "Machine forbidden",
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
            "Below spindle min",
            format!("< {} RPM", env.spindle_min_rpm as i64),
        ));
    }
    if let Some(rpm) = current.spindle_rpm {
        entries.push(LegendEntry::new(
            LegendSwatch::Circle,
            // Ruling R23: the operator's own value is not a verdict.
            crate::ui::tokens::TEXT_STRONG,
            "● Current",
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
                "○ Target (pre-derate)",
                format!("{target_cl:.4} mm/tooth · {target_feed:.0} mm/min"),
            ));
        }
    }
    entries.push(LegendEntry::new(
        LegendSwatch::Diamond,
        tokens::DIAGRAM_INK,
        "◆ Recommended (effective)",
        format!(
            "{:.0} RPM · {:.0} mm/min · chipload {:.4} mm/tooth",
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
        // Two-column grid: swatch+label on the left, value on the right.
        egui::Grid::new("feeds_modal_chart_c_legend")
            .num_columns(2)
            .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
            .min_row_height(crate::ui::tokens::ROW_DENSE)
            .show(ui, |ui| {
                for entry in &entries {
                    ui.horizontal(|ui| {
                        draw_legend_swatch(ui, entry.swatch, entry.color);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(entry.label)
                                    .small()
                                    .color(theme::TEXT_STRONG),
                            )
                            .wrap(),
                        );
                    });
                    ui.label(
                        egui::RichText::new(&entry.value)
                            .small()
                            .color(theme::TEXT_DIM),
                    );
                    ui.end_row();
                }
            });
    });
}

#[derive(Debug, Clone, Copy)]
enum LegendSwatch {
    FilledSquare,
    Line,
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
        LegendSwatch::Line => {
            let y = rect.center().y;
            painter.line_segment(
                [
                    egui::pos2(rect.left() + 1.0, y),
                    egui::pos2(rect.right() - 1.0, y),
                ],
                egui::Stroke::new(2.0_f32, color),
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

// ────────────────────────────────────────────────────────────────────
// Chart A — Advance/tooth vs Diameter
// ────────────────────────────────────────────────────────────────────

pub(crate) fn draw_chart_a(ui: &mut egui::Ui, current: &CurrentValues, explain: &FeedsExplain) {
    ui.label(
        egui::RichText::new("Advance/tooth vs Diameter")
            .strong()
            .color(theme::TEXT_STRONG),
    );
    let rows = explain.rows_by_diameter();
    if rows.is_empty() {
        ui.label(
            egui::RichText::new("No matching vendor rows for this material.")
                .small()
                .color(theme::TEXT_DIM),
        );
        return;
    }
    // Pre-compute the raw vendor min/max curves so the legend below the
    // chart can show numeric ranges.
    let mut min_pts: Vec<[f64; 2]> = Vec::new();
    let mut max_pts: Vec<[f64; 2]> = Vec::new();
    for row in &rows {
        let d = row.row_diameter_mm;
        // Reverse the diameter scaling so the chart shows the raw
        // vendor row values, not the scaled-for-this-tool values. The
        // scaling factor is the same across all rows in the cohort
        // (same query hardness).
        let raw_min = row.chip_load_min_mm.unwrap_or(0.0) / row.chipload_diameter_scale.max(1e-6);
        let raw_max = row.chip_load_max_mm.unwrap_or(0.0) / row.chipload_diameter_scale.max(1e-6);
        if raw_min > 0.0 {
            min_pts.push([d, raw_min]);
        }
        if raw_max > 0.0 {
            max_pts.push([d, raw_max]);
        }
    }
    min_pts.sort_by(|a, b| a[0].total_cmp(&b[0]));
    max_pts.sort_by(|a, b| a[0].total_cmp(&b[0]));

    let band_color = wash(chart::BAND, 50);
    let min_color = chart::ISO_MIN;
    let max_color = chart::ISO_MAX;

    Plot::new("feeds_modal_chart_a")
        .height(180.0)
        .width(360.0)
        .x_axis_label("diameter (mm)")
        .y_axis_label("commanded advance/tooth (mm)")
        .show(ui, |plot_ui| {
            // Shaded band between min and max — only when both curves
            // share at least two diameters (otherwise the polygon is
            // degenerate and adds visual noise).
            if min_pts.len() >= 2 && max_pts.len() >= 2 {
                let mut band_poly: Vec<[f64; 2]> = min_pts.clone();
                band_poly.extend(max_pts.iter().rev().copied());
                plot_ui.polygon(
                    Polygon::new("", PlotPoints::from(band_poly))
                        .fill_color(band_color)
                        .stroke(egui::Stroke::new(0.0_f32, egui::Color32::TRANSPARENT))
                        .name("vendor band (this material)"),
                );
            }
            plot_ui.line(
                Line::new("", PlotPoints::from(min_pts.clone()))
                    .color(min_color)
                    .width(1.5_f32)
                    .name("vendor min"),
            );
            plot_ui.line(
                Line::new("", PlotPoints::from(max_pts.clone()))
                    .color(max_color)
                    .width(1.5_f32)
                    .name("vendor max"),
            );
            plot_ui.points(
                Points::new("", min_pts.clone())
                    .shape(MarkerShape::Square)
                    .radius(3.0_f32)
                    .color(min_color),
            );
            plot_ui.points(
                Points::new("", max_pts.clone())
                    .shape(MarkerShape::Square)
                    .radius(3.0_f32)
                    .color(max_color),
            );
            // Inline value labels next to each calibrated diameter so
            // the user can read the band values straight from the chart.
            for p in &min_pts {
                plot_ui.text(
                    egui_plot::Text::new(
                        "",
                        egui_plot::PlotPoint::new(p[0], p[1]),
                        egui::RichText::new(format!("{:.3}", p[1]))
                            .small()
                            .color(min_color),
                    )
                    .anchor(egui::Align2::CENTER_TOP),
                );
            }
            for p in &max_pts {
                plot_ui.text(
                    egui_plot::Text::new(
                        "",
                        egui_plot::PlotPoint::new(p[0], p[1]),
                        egui::RichText::new(format!("{:.3}", p[1]))
                            .small()
                            .color(max_color),
                    )
                    .anchor(egui::Align2::CENTER_BOTTOM),
                );
            }

            // Your tool vertical line spanning the band height.
            let y_top = max_pts.iter().map(|p| p[1]).fold(0.0_f64, f64::max) * 1.15;
            plot_ui.line(
                Line::new(
                    "",
                    PlotPoints::from(vec![
                        [explain.tool_diameter_mm, 0.0],
                        [explain.tool_diameter_mm, y_top.max(0.05)],
                    ]),
                )
                .color(theme::TEXT_DIM)
                .style(egui_plot::LineStyle::Dashed { length: 4.0 })
                .name(format!("your tool {:.2} mm", explain.tool_diameter_mm)),
            );
            plot_ui.text(
                egui_plot::Text::new(
                    "",
                    egui_plot::PlotPoint::new(explain.tool_diameter_mm, y_top.max(0.05)),
                    egui::RichText::new(format!(" {:.2} mm", explain.tool_diameter_mm))
                        .small()
                        .color(theme::TEXT_DIM),
                )
                .anchor(egui::Align2::LEFT_TOP),
            );

            // Current and recommended chipload at your diameter.
            plot_ui.points(
                Points::new("", vec![[explain.tool_diameter_mm, current.chipload_mm()]])
                    .shape(MarkerShape::Circle)
                    .filled(true)
                    .radius(5.0_f32)
                    .color(theme::ERROR)
                    .name("Current"),
            );
            plot_ui.points(
                Points::new(
                    "",
                    vec![[explain.tool_diameter_mm, explain.recommended.chip_load_mm]],
                )
                .shape(MarkerShape::Diamond)
                .filled(true)
                .radius(5.0_f32)
                .color(tokens::DIAGRAM_INK)
                .name("Recommended"),
            );
        });

    // Per-chart band summary.
    let cl_min_for_your_tool = explain
        .matched_row
        .as_ref()
        .and_then(|r| r.chip_load_min_mm);
    let cl_max_for_your_tool = explain
        .matched_row
        .as_ref()
        .and_then(|r| r.chip_load_max_mm);
    let band_text = match (cl_min_for_your_tool, cl_max_for_your_tool) {
        (Some(lo), Some(hi)) => format!(
            "{lo:.4}–{hi:.4} mm/tooth at your {:.2} mm tool",
            explain.tool_diameter_mm
        ),
        (None, Some(hi)) => format!("≤ {hi:.4} mm/tooth at your tool"),
        _ => "no scaled band available".to_owned(),
    };
    draw_mini_chart_legend(
        ui,
        "feeds_modal_chart_a_legend",
        &[
            MiniLegend {
                swatch: LegendSwatch::FilledSquare,
                color: band_color,
                label: "vendor band",
                value: band_text,
            },
            MiniLegend {
                swatch: LegendSwatch::Line,
                color: min_color,
                label: "vendor min line",
                value: format!("{} calibrated diameters", min_pts.len()),
            },
            MiniLegend {
                swatch: LegendSwatch::Line,
                color: max_color,
                label: "vendor max line",
                value: format!("{} calibrated diameters", max_pts.len()),
            },
            MiniLegend {
                swatch: LegendSwatch::Circle,
                // Ruling R23: the operator's own value is not a verdict.
                color: crate::ui::tokens::TEXT_STRONG,
                label: "● current",
                value: format!("{:.4} mm/tooth", current.chipload_mm()),
            },
            MiniLegend {
                swatch: LegendSwatch::Diamond,
                color: tokens::DIAGRAM_INK,
                label: "◆ recommended",
                value: format!("{:.4} mm/tooth", explain.recommended.chip_load_mm),
            },
        ],
    );
}

// ────────────────────────────────────────────────────────────────────
// Chart B — Advance/tooth vs Hardness
// ────────────────────────────────────────────────────────────────────

/// Mini-legend row used under Charts A and B.
struct MiniLegend {
    swatch: LegendSwatch,
    color: egui::Color32,
    label: &'static str,
    value: String,
}

fn draw_mini_chart_legend(ui: &mut egui::Ui, id: &str, entries: &[MiniLegend]) {
    ui.add_space(2.0);
    egui::Frame::group(ui.style()).show(ui, |ui| {
        egui::Grid::new(id)
            .num_columns(2)
            .spacing([crate::ui::tokens::SPACE_3, crate::ui::tokens::SPACE_2])
            .min_row_height(crate::ui::tokens::ROW_DENSE)
            .show(ui, |ui| {
                for entry in entries {
                    ui.horizontal(|ui| {
                        draw_legend_swatch(ui, entry.swatch, entry.color);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(entry.label)
                                    .small()
                                    .color(theme::TEXT_STRONG),
                            )
                            .wrap(),
                        );
                    });
                    ui.label(
                        egui::RichText::new(&entry.value)
                            .small()
                            .color(theme::TEXT_DIM),
                    );
                    ui.end_row();
                }
            });
    });
}

pub(crate) fn draw_chart_b(ui: &mut egui::Ui, current: &CurrentValues, explain: &FeedsExplain) {
    ui.label(
        egui::RichText::new("Advance/tooth vs Hardness")
            .strong()
            .color(theme::TEXT_STRONG),
    );
    let rows = explain.rows_by_hardness();
    if rows.is_empty() {
        ui.label(
            egui::RichText::new("No matching vendor rows at this diameter.")
                .small()
                .color(theme::TEXT_DIM),
        );
        return;
    }
    // Each sibling row was matched with a hardness_scale of
    // (row.hardness / query.hardness). To plot against actual row
    // hardness we recover it from `query_hardness_value` × scale.
    let q_hardness = explain.query_hardness_value.unwrap_or(0.0);

    let band_color = wash(chart::BAND, 50);
    let min_color = chart::ISO_MIN;
    let max_color = chart::ISO_MAX;
    let mut min_pts: Vec<[f64; 2]> = Vec::new();
    let mut max_pts: Vec<[f64; 2]> = Vec::new();
    for row in &rows {
        let row_hardness = if row.chipload_hardness_scale > 0.0 {
            q_hardness * row.chipload_hardness_scale
        } else {
            q_hardness
        };
        let raw_min = row.chip_load_min_mm.unwrap_or(0.0) / row.chipload_hardness_scale.max(1e-6);
        let raw_max = row.chip_load_max_mm.unwrap_or(0.0) / row.chipload_hardness_scale.max(1e-6);
        if raw_min > 0.0 {
            min_pts.push([row_hardness, raw_min]);
        }
        if raw_max > 0.0 {
            max_pts.push([row_hardness, raw_max]);
        }
    }
    min_pts.sort_by(|a, b| a[0].total_cmp(&b[0]));
    max_pts.sort_by(|a, b| a[0].total_cmp(&b[0]));

    Plot::new("feeds_modal_chart_b")
        .height(180.0)
        .width(360.0)
        .x_axis_label(hardness_axis_label(explain.query_hardness_kind))
        .y_axis_label("commanded advance/tooth (mm)")
        .show(ui, |plot_ui| {
            if min_pts.len() >= 2 && max_pts.len() >= 2 {
                let mut band_poly: Vec<[f64; 2]> = min_pts.clone();
                band_poly.extend(max_pts.iter().rev().copied());
                plot_ui.polygon(
                    Polygon::new("", PlotPoints::from(band_poly))
                        .fill_color(band_color)
                        .stroke(egui::Stroke::new(0.0_f32, egui::Color32::TRANSPARENT))
                        .name("vendor band (this diameter)"),
                );
            }
            plot_ui.line(
                Line::new("", PlotPoints::from(min_pts.clone()))
                    .color(min_color)
                    .width(1.5_f32)
                    .name("vendor min"),
            );
            plot_ui.line(
                Line::new("", PlotPoints::from(max_pts.clone()))
                    .color(max_color)
                    .width(1.5_f32)
                    .name("vendor max"),
            );
            plot_ui.points(
                Points::new("", min_pts.clone())
                    .shape(MarkerShape::Square)
                    .radius(3.0_f32)
                    .color(min_color),
            );
            plot_ui.points(
                Points::new("", max_pts.clone())
                    .shape(MarkerShape::Square)
                    .radius(3.0_f32)
                    .color(max_color),
            );
            // Inline value tags.
            for p in &min_pts {
                plot_ui.text(
                    egui_plot::Text::new(
                        "",
                        egui_plot::PlotPoint::new(p[0], p[1]),
                        egui::RichText::new(format!("{:.3}", p[1]))
                            .small()
                            .color(min_color),
                    )
                    .anchor(egui::Align2::CENTER_TOP),
                );
            }
            for p in &max_pts {
                plot_ui.text(
                    egui_plot::Text::new(
                        "",
                        egui_plot::PlotPoint::new(p[0], p[1]),
                        egui::RichText::new(format!("{:.3}", p[1]))
                            .small()
                            .color(max_color),
                    )
                    .anchor(egui::Align2::CENTER_BOTTOM),
                );
            }

            // Your material vertical line spanning the band height.
            let y_top = max_pts.iter().map(|p| p[1]).fold(0.0_f64, f64::max) * 1.15;
            if q_hardness > 0.0 {
                plot_ui.line(
                    Line::new(
                        "",
                        PlotPoints::from(vec![[q_hardness, 0.0], [q_hardness, y_top.max(0.05)]]),
                    )
                    .color(theme::TEXT_DIM)
                    .style(egui_plot::LineStyle::Dashed { length: 4.0 })
                    .name(format!("your material ({q_hardness:.0})")),
                );
                plot_ui.text(
                    egui_plot::Text::new(
                        "",
                        egui_plot::PlotPoint::new(q_hardness, y_top.max(0.05)),
                        egui::RichText::new(format!(" {q_hardness:.0}"))
                            .small()
                            .color(theme::TEXT_DIM),
                    )
                    .anchor(egui::Align2::LEFT_TOP),
                );
            }
            // Current and recommended.
            plot_ui.points(
                Points::new("", vec![[q_hardness, current.chipload_mm()]])
                    .shape(MarkerShape::Circle)
                    .filled(true)
                    .radius(5.0_f32)
                    .color(theme::ERROR)
                    .name("Current"),
            );
            plot_ui.points(
                Points::new("", vec![[q_hardness, explain.recommended.chip_load_mm]])
                    .shape(MarkerShape::Diamond)
                    .filled(true)
                    .radius(5.0_f32)
                    .color(tokens::DIAGRAM_INK)
                    .name("Recommended"),
            );
        });

    let cl_min = explain
        .matched_row
        .as_ref()
        .and_then(|r| r.chip_load_min_mm);
    let cl_max = explain
        .matched_row
        .as_ref()
        .and_then(|r| r.chip_load_max_mm);
    let band_text = match (cl_min, cl_max) {
        (Some(lo), Some(hi)) => {
            format!("{lo:.4}–{hi:.4} mm/tooth at your material ({q_hardness:.0})")
        }
        (None, Some(hi)) => format!("≤ {hi:.4} mm/tooth at your material"),
        _ => "no scaled band available".to_owned(),
    };
    draw_mini_chart_legend(
        ui,
        "feeds_modal_chart_b_legend",
        &[
            MiniLegend {
                swatch: LegendSwatch::FilledSquare,
                color: band_color,
                label: "vendor band",
                value: band_text,
            },
            MiniLegend {
                swatch: LegendSwatch::Line,
                color: min_color,
                label: "vendor min line",
                value: format!("{} materials sampled", min_pts.len()),
            },
            MiniLegend {
                swatch: LegendSwatch::Line,
                color: max_color,
                label: "vendor max line",
                value: format!("{} materials sampled", max_pts.len()),
            },
            MiniLegend {
                swatch: LegendSwatch::Circle,
                // Ruling R23: the operator's own value is not a verdict.
                color: crate::ui::tokens::TEXT_STRONG,
                label: "● current",
                value: format!("{:.4} mm/tooth", current.chipload_mm()),
            },
            MiniLegend {
                swatch: LegendSwatch::Diamond,
                color: tokens::DIAGRAM_INK,
                label: "◆ recommended",
                value: format!("{:.4} mm/tooth", explain.recommended.chip_load_mm),
            },
        ],
    );
}

fn hardness_axis_label(kind: Option<HardnessKind>) -> &'static str {
    match kind {
        Some(HardnessKind::Janka) => "Janka (lbf)",
        Some(HardnessKind::ShoreD) => "Shore D",
        Some(HardnessKind::Hb) => "Brinell (HB)",
        None => "hardness",
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::hover_readout_chipload;

    /// The nomogram hover readout must never print a negative advance
    /// per tooth.
    ///
    /// Pre-fix reproduction, preserved as the first case: the readout
    /// divided the RAW pointer coordinates while printing the CLAMPED
    /// ones, so a drag below the feed axis at 18000 RPM on a 2-flute
    /// cutter printed `18000 RPM · 0 mm/min → −0.0928 mm/tooth`
    /// (W10-LV item 10). The literal −0.0928 is reproduced here from the
    /// feed that produces it, so the case is the observed one and not a
    /// paraphrase: −0.0928 × 18000 × 2 = −3340.8 mm/min.
    ///
    /// Display-only: this function formats a tooltip. No recipe number
    /// moves.
    #[test]
    fn hover_readout_never_prints_a_negative_chipload() {
        // The exact reported reading, through the pre-fix arithmetic.
        let raw: f64 = -3340.8 / (18000.0 * 2.0);
        assert!(
            (raw - -0.0928).abs() < 1e-4,
            "fixture must reproduce the reported −0.0928 mm/tooth, got {raw}"
        );

        // Same pointer position, through the shipped path: the caller
        // clamps the feed to 0 and hands this function the same value it
        // prints.
        assert_eq!(hover_readout_chipload(18000.0, 0.0, 2.0), 0.0);

        // And the clamp is defended at the function too, so a caller
        // that forgets cannot resurrect the wart.
        assert_eq!(hover_readout_chipload(18000.0, -3340.8, 2.0), 0.0);

        // Zero RPM / zero flutes must be 0.0, not an infinity or a NaN —
        // both reach `{:.4}` in the label.
        assert_eq!(hover_readout_chipload(0.0, 2520.0, 2.0), 0.0);
        assert_eq!(hover_readout_chipload(0.0, 0.0, 2.0), 0.0);
        assert_eq!(hover_readout_chipload(18000.0, 2520.0, 0.0), 0.0);

        // The ordinary case is untouched: 2520 / (18000 × 2) = 0.07.
        let ok = hover_readout_chipload(18000.0, 2520.0, 2.0);
        assert!((ok - 0.07).abs() < 1e-12, "got {ok}");
    }
}
