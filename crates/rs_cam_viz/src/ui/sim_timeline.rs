use super::AppEvent;
use super::sim_debug::semantic_kind_color;
use crate::render::toolpath_render::palette_color;
use crate::state::runtime::GuiState;
use crate::state::simulation::{ActiveSemanticItem, SimulationAnalyticsTab, SimulationState};
use egui_plot::{Line, Plot, PlotPoints};
use rs_cam_core::session::ProjectSession;
use rs_cam_core::simulation_cut::SimulationCutSample;
use rs_cam_core::tool_load::{Confidence, ToolLoadReport, Verdict};

/// Bottom panel in simulation workspace: transport controls, timeline scrubber, speed control.
pub fn draw(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    session: &ProjectSession,
    gui: &GuiState,
    events: &mut Vec<AppEvent>,
) {
    let max_feed = session.machine().max_feed_mm_min;
    sim.sync_debug_state(gui, max_feed);
    let active_semantic = sim.active_semantic_item(gui, max_feed);
    let current_boundary = sim.current_boundary().cloned();

    draw_transport_and_scrubber(ui, sim, session, gui, events);
    draw_verdict_hud(ui, sim, session, gui, max_feed, events);
    draw_boundary_timeline(
        ui,
        sim,
        gui,
        session,
        max_feed,
        &current_boundary,
        &active_semantic,
        events,
    );
    draw_signal_spine(ui, sim, session, events);
    draw_speed_controls(ui, sim);
}

fn draw_verdict_hud(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    session: &ProjectSession,
    gui: &GuiState,
    max_feed: f64,
    events: &mut Vec<AppEvent>,
) {
    // sim.issues() takes &mut self, so call it first before taking any
    // immutable borrows on sim.results below.
    let issue_count = sim.issues(gui, max_feed).len();

    let (ok, warn, bad, unmodeled, collision_count, trace_count, unmodeled_target, exceeded_target, collision_target) = {
        let sim_trace = sim.results.as_ref().and_then(|r| r.cut_trace.as_deref());
        let report = rs_cam_core::gcode::project_load_report(session, sim_trace);
        let counts = verdict_counts(&report);
        let collision_count =
            sim.checks.rapid_collisions.len() + sim.checks.holder_collision_count;
        let trace_count = gui
            .toolpath_rt
            .values()
            .filter(|rt| rt.debug_trace.is_some() || rt.semantic_trace.is_some())
            .count();
        let unmodeled_target = first_unmodeled_move(sim, &report);
        let exceeded_target = first_exceeded_overall(sim, sim_trace, &report);
        let collision_target = sim.checks.rapid_collision_move_indices.first().copied();
        (
            counts.0,
            counts.1,
            counts.2,
            counts.3,
            collision_count,
            trace_count,
            unmodeled_target,
            exceeded_target,
            collision_target,
        )
    };

    let mut click_unmodeled = false;
    let mut click_exceeds = false;
    let mut click_collision = false;
    let mut click_issues = false;

    egui::Frame::default()
        .fill(egui::Color32::from_rgb(30, 32, 42))
        .inner_margin(egui::Margin::symmetric(6.0, 4.0))
        .rounding(4.0)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                verdict_pill(
                    ui,
                    format!("✓ load {ok}"),
                    egui::Color32::from_rgb(85, 180, 110),
                    "Toolpaths within modeled load limits",
                );
                click_unmodeled = verdict_pill(
                    ui,
                    format!("⚠ unmodeled {unmodeled}"),
                    egui::Color32::from_rgb(210, 170, 80),
                    "Load could not be modeled honestly; click to jump to the first unmodeled toolpath",
                )
                .clicked();
                click_exceeds = verdict_pill(
                    ui,
                    format!("✕ exceeds {bad}"),
                    egui::Color32::from_rgb(220, 90, 90),
                    "Known load-limit exceedances; click to jump to the first one",
                )
                .clicked();
                verdict_pill(
                    ui,
                    format!("~ approx {warn}"),
                    egui::Color32::from_rgb(120, 150, 220),
                    "Approximate or advisory verdicts",
                );
                click_collision = verdict_pill(
                    ui,
                    format!("collisions {collision_count}"),
                    if collision_count == 0 {
                        egui::Color32::from_rgb(120, 210, 140)
                    } else {
                        egui::Color32::from_rgb(255, 120, 110)
                    },
                    "Rapid/holder collisions; click to jump to the first one",
                )
                .clicked();
                click_issues = verdict_pill(
                    ui,
                    format!("issues {issue_count}"),
                    egui::Color32::from_rgb(230, 190, 90),
                    "Air cuts / low-engagement clusters; click to step through",
                )
                .clicked();
                verdict_pill(
                    ui,
                    format!("trace artifacts {trace_count}"),
                    egui::Color32::from_rgb(150, 170, 230),
                    "Recorded semantic/performance traces",
                );
            });
        });

    if click_unmodeled && let Some(t) = unmodeled_target {
        events.push(AppEvent::SimJumpToMove(t));
    }
    if click_exceeds && let Some(t) = exceeded_target {
        events.push(AppEvent::SimJumpToMove(t));
    }
    if click_collision && let Some(t) = collision_target {
        events.push(AppEvent::SimJumpToMove(t));
    }
    if click_issues
        && let Some(target) = sim.focus_issue_delta(gui, max_feed, 1)
    {
        events.push(AppEvent::SimJumpToMove(target.move_index));
    }
}

fn first_unmodeled_move(sim: &SimulationState, report: &ToolLoadReport) -> Option<usize> {
    let tp = report.per_toolpath.iter().find(|tp| {
        matches!(tp.chipload, Verdict::Unmodeled { .. })
            || matches!(tp.power, Verdict::Unmodeled { .. })
            || matches!(tp.deflection, Verdict::Unmodeled { .. })
    })?;
    sim.boundaries()
        .iter()
        .find(|b| b.id.0 == tp.toolpath_id)
        .map(|b| b.start_move)
}

fn first_exceeded_overall(
    sim: &SimulationState,
    trace: Option<&rs_cam_core::simulation_cut::SimulationCutTrace>,
    report: &ToolLoadReport,
) -> Option<usize> {
    let trace = trace?;
    report
        .per_toolpath
        .iter()
        .find_map(|verdict| first_exceeded_tool_load_move(sim, trace, verdict))
}

fn verdict_counts(report: &ToolLoadReport) -> (usize, usize, usize, usize) {
    let mut ok = 0;
    let mut warn = 0;
    let mut bad = 0;
    let mut unmodeled = 0;
    for tp in &report.per_toolpath {
        for verdict in [&tp.chipload, &tp.power, &tp.deflection] {
            match verdict {
                Verdict::Within { .. } => ok += 1,
                Verdict::Unmodeled { .. } => unmodeled += 1,
                Verdict::Exceeds { .. } => bad += 1,
            }
            if verdict_is_approximate(verdict) {
                warn += 1;
            }
        }
    }
    (ok, warn, bad, unmodeled)
}

fn verdict_is_approximate(verdict: &Verdict) -> bool {
    matches!(
        verdict,
        Verdict::Within {
            confidence: Confidence::Approximate(_),
            ..
        } | Verdict::Exceeds {
            confidence: Confidence::Approximate(_),
            ..
        }
    )
}

fn verdict_pill(
    ui: &mut egui::Ui,
    text: String,
    color: egui::Color32,
    hover: &str,
) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).small().color(color))
            .fill(egui::Color32::from_rgb(42, 44, 56)),
    )
    .on_hover_text(hover)
}

fn draw_signal_spine(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    session: &ProjectSession,
    events: &mut Vec<AppEvent>,
) {
    let Some(trace) = sim.results.as_ref().and_then(|r| r.cut_trace.as_deref()) else {
        ui.label(
            egui::RichText::new("Run simulation with Cut Metrics to see chipload, engagement, DOC, MRR, and feed tracks.")
                .small()
                .italics()
                .color(egui::Color32::from_rgb(130, 130, 145)),
        );
        return;
    };
    let cutting: Vec<&SimulationCutSample> =
        trace.samples.iter().filter(|s| s.is_cutting).collect();
    if cutting.is_empty() {
        ui.label(
            egui::RichText::new("No cutting samples captured.")
                .small()
                .italics(),
        );
        return;
    }

    ui.add_space(4.0);
    let active_time = sim
        .current_cut_sample()
        .map(|sample| sample.sample.cumulative_time_s);
    let envelope = sim
        .current_boundary()
        .and_then(|b| chipload_envelope_for_toolpath(session, Some(trace), b.id));

    // Build hotspot markers once per frame: time, index in trace.hotspots,
    // and the global move to scrub to on click. Dots render on every track
    // and clicking near one focuses the hotspot in the Inspector.
    let hotspots: Vec<HotspotMarker> = trace
        .hotspots
        .iter()
        .enumerate()
        .filter_map(|(idx, hs)| {
            let sample = trace.samples.get(hs.sample_index_start)?;
            let global_move = sim
                .global_move_for_local(
                    crate::state::toolpath::ToolpathId(hs.toolpath_id),
                    hs.move_start,
                )
                .unwrap_or(hs.move_start);
            Some(HotspotMarker {
                time: sample.cumulative_time_s,
                index: idx,
                global_move,
            })
        })
        .collect();

    // Linked crosshair: read last frame's hovered time for display, accumulate
    // this frame's pointer into a fresh local, then write back at the end.
    // One-frame lag is intentional and imperceptible.
    let display_time = sim.hovered_time_s;
    let mut new_hovered: Option<f64> = None;
    let mut clicked_hotspot: Option<(usize, usize)> = None;

    let tracks: [(&str, fn(&SimulationCutSample) -> Option<f64>, egui::Color32, Option<(f64, f64)>); 5] = [
        (
            "chipload",
            |s| s.effective_chip_thickness_mm,
            egui::Color32::from_rgb(230, 200, 60),
            envelope,
        ),
        (
            "arc engagement",
            |s| s.arc_engagement_radians,
            egui::Color32::from_rgb(80, 200, 120),
            None,
        ),
        (
            "axial DOC",
            |s| (s.axial_doc_mm > 0.0).then_some(s.axial_doc_mm),
            egui::Color32::from_rgb(110, 150, 230),
            None,
        ),
        (
            "MRR",
            |s| (s.mrr_mm3_s > 0.0).then_some(s.mrr_mm3_s),
            egui::Color32::from_rgb(210, 130, 230),
            None,
        ),
        (
            "feed",
            |s| Some(s.feed_rate_mm_min),
            egui::Color32::from_rgb(150, 190, 230),
            None,
        ),
    ];

    for (label, value_fn, color, env) in tracks {
        draw_signal_track(
            ui,
            label,
            &cutting,
            value_fn,
            color,
            active_time,
            display_time,
            &mut new_hovered,
            env,
            &hotspots,
            &mut clicked_hotspot,
            sim,
            events,
        );
    }

    sim.hovered_time_s = new_hovered;
    if let Some((hotspot_index, global_move)) = clicked_hotspot {
        if let Some(hs) = trace.hotspots.get(hotspot_index) {
            sim.debug.focused_hotspot = Some((
                crate::state::toolpath::ToolpathId(hs.toolpath_id),
                hotspot_index,
            ));
        }
        events.push(AppEvent::SimJumpToMove(global_move));
    }
}

#[derive(Clone, Copy)]
struct HotspotMarker {
    time: f64,
    index: usize,
    global_move: usize,
}

const SIGNAL_MAX_POINTS: usize = 1600;

#[allow(clippy::too_many_arguments)]
fn draw_signal_track(
    ui: &mut egui::Ui,
    label: &str,
    samples: &[&SimulationCutSample],
    value_fn: impl Fn(&SimulationCutSample) -> Option<f64>,
    color: egui::Color32,
    active_time: Option<f64>,
    display_time: Option<f64>,
    new_hovered: &mut Option<f64>,
    envelope: Option<(f64, f64)>,
    hotspots: &[HotspotMarker],
    clicked_hotspot: &mut Option<(usize, usize)>,
    sim: &SimulationState,
    events: &mut Vec<AppEvent>,
) {
    let mut point_samples: Vec<(&SimulationCutSample, [f64; 2])> = samples
        .iter()
        .filter_map(|sample| value_fn(sample).map(|y| (*sample, [sample.cumulative_time_s, y])))
        .collect();
    if point_samples.is_empty() {
        return;
    }
    if point_samples.len() > SIGNAL_MAX_POINTS {
        let stride = point_samples.len().div_ceil(SIGNAL_MAX_POINTS);
        point_samples = point_samples
            .into_iter()
            .enumerate()
            .filter_map(|(i, point)| (i % stride == 0).then_some(point))
            .collect();
    }
    let points: Vec<[f64; 2]> = point_samples.iter().map(|(_, point)| *point).collect();
    let min_y = points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
    let max_y = points
        .iter()
        .map(|p| p[1])
        .fold(f64::NEG_INFINITY, f64::max);
    let x_min = points
        .first()
        .map(|p| p[0])
        .unwrap_or(0.0);
    let x_max = points
        .last()
        .map(|p| p[0])
        .unwrap_or(x_min);

    let response = Plot::new(format!("signal_track_{label}"))
        .height(46.0)
        .allow_zoom([true, false])
        .allow_drag([true, false])
        .show_axes([false, false])
        .show(ui, |plot_ui| {
            plot_ui.line(Line::new(PlotPoints::from(points)).name(label).color(color));

            if let Some((cl_min, cl_max)) = envelope {
                let band_color = egui::Color32::from_rgb(220, 90, 90);
                plot_ui.line(
                    Line::new(PlotPoints::from(vec![[x_min, cl_min], [x_max, cl_min]]))
                        .color(band_color)
                        .style(egui_plot::LineStyle::Dashed { length: 6.0 })
                        .name("cl_min"),
                );
                plot_ui.line(
                    Line::new(PlotPoints::from(vec![[x_min, cl_max], [x_max, cl_max]]))
                        .color(band_color)
                        .style(egui_plot::LineStyle::Dashed { length: 6.0 })
                        .name("cl_max"),
                );
            }

            if let Some(t) = active_time {
                plot_ui.line(
                    Line::new(PlotPoints::from(vec![[t, min_y], [t, max_y]]))
                        .color(egui::Color32::from_rgb(245, 245, 245))
                        .name("playback"),
                );
            }

            // Linked hover crosshair from last frame — every track shows it
            // at the same time, regardless of which track the pointer is over.
            if let Some(t) = display_time {
                plot_ui.line(
                    Line::new(PlotPoints::from(vec![[t, min_y], [t, max_y]]))
                        .color(egui::Color32::from_rgb(200, 240, 100))
                        .style(egui_plot::LineStyle::Dashed { length: 4.0 }),
                );
                if let Some((_, point)) = nearest_point_to_time(t, &point_samples) {
                    plot_ui.text(egui_plot::Text::new(
                        egui_plot::PlotPoint::new(point[0], point[1]),
                        format!("{label}: {:.3} @ {:.2}s", point[1], point[0]),
                    ));
                }
            }

            // Hotspot dots: position each at the y-value of the nearest data
            // point on this track so the dot sits visually on the line.
            if !hotspots.is_empty() {
                let hotspot_pts: Vec<[f64; 2]> = hotspots
                    .iter()
                    .filter_map(|hs| {
                        nearest_point_to_time(hs.time, &point_samples).map(|(_, pt)| pt)
                    })
                    .collect();
                if !hotspot_pts.is_empty() {
                    plot_ui.points(
                        egui_plot::Points::new(PlotPoints::from(hotspot_pts))
                            .color(egui::Color32::from_rgb(255, 145, 70))
                            .radius(4.0)
                            .name("hotspots"),
                    );
                }
            }

            if let Some(pointer) = plot_ui.pointer_coordinate() {
                *new_hovered = Some(pointer.x);
                if plot_ui.response().clicked() {
                    // Prefer hotspot click when the pointer is within tolerance
                    // (5% of the visible x range, or 0.25s, whichever is larger).
                    let tolerance = ((x_max - x_min).abs() * 0.05).max(0.25);
                    let nearest_hotspot = hotspots.iter().min_by(|a, b| {
                        (a.time - pointer.x)
                            .abs()
                            .total_cmp(&(b.time - pointer.x).abs())
                    });
                    if let Some(hs) = nearest_hotspot
                        && (hs.time - pointer.x).abs() <= tolerance
                    {
                        *clicked_hotspot = Some((hs.index, hs.global_move));
                    } else if let Some((sample, _)) =
                        nearest_point_to_time(pointer.x, &point_samples)
                        && let Some(global_move) = sim.global_move_for_local(
                            crate::state::toolpath::ToolpathId(sample.toolpath_id),
                            sample.move_index,
                        )
                    {
                        events.push(AppEvent::SimJumpToMove(global_move));
                    }
                }
            }
        });
    response
        .response
        .on_hover_text("Hover any track to read all five at the same time. Click to scrub.");
}

fn nearest_point_to_time<'a>(
    t: f64,
    point_samples: &'a [(&'a SimulationCutSample, [f64; 2])],
) -> Option<(&'a SimulationCutSample, [f64; 2])> {
    point_samples
        .iter()
        .min_by(|left, right| (left.1[0] - t).abs().total_cmp(&(right.1[0] - t).abs()))
        .copied()
}

fn chipload_envelope_for_toolpath(
    session: &ProjectSession,
    sim_trace: Option<&rs_cam_core::simulation_cut::SimulationCutTrace>,
    toolpath_id: crate::state::toolpath::ToolpathId,
) -> Option<(f64, f64)> {
    let suggestions = rs_cam_core::tool_load::suggest::project_suggestions(session, sim_trace);
    let suggested = suggestions
        .into_iter()
        .find(|s| s.toolpath_id == toolpath_id.0)?
        .suggested
        .ok()?;
    Some((
        suggested.chipload_envelope.start,
        suggested.chipload_envelope.end,
    ))
}

/// Row 1: Transport buttons, timeline scrubber slider, and time display.
fn draw_transport_and_scrubber(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    session: &ProjectSession,
    gui: &GuiState,
    events: &mut Vec<AppEvent>,
) {
    ui.horizontal(|ui| {
        let btn_size = egui::vec2(32.0, 24.0);
        if ui
            .add(egui::Button::new("|◄").min_size(btn_size))
            .on_hover_text("Jump to start (Home)")
            .clicked()
        {
            events.push(AppEvent::SimJumpToStart);
        }
        if ui
            .add(egui::Button::new("◄").min_size(btn_size))
            .on_hover_text("Step back (Left)")
            .clicked()
        {
            events.push(AppEvent::SimStepBackward);
        }
        let play_label = if sim.playback.playing {
            "❚❚"
        } else {
            "▶"
        };
        let play_tip = if sim.playback.playing {
            "Pause (Space)"
        } else {
            "Play (Space)"
        };
        if ui
            .add(egui::Button::new(play_label).min_size(btn_size))
            .on_hover_text(play_tip)
            .clicked()
        {
            events.push(AppEvent::ToggleSimPlayback);
        }
        if ui
            .add(egui::Button::new("►").min_size(btn_size))
            .on_hover_text("Step forward (Right)")
            .clicked()
        {
            events.push(AppEvent::SimStepForward);
        }
        if ui
            .add(egui::Button::new("►|").min_size(btn_size))
            .on_hover_text("Jump to end (End)")
            .clicked()
        {
            events.push(AppEvent::SimJumpToEnd);
        }

        if sim.total_moves() > 0 {
            ui.separator();

            let (elapsed_time, total_time) = estimate_times(sim, session, gui);
            let elapsed_str = format_time(elapsed_time);
            let total_str = format_time(total_time);
            ui.label(
                egui::RichText::new(format!("{} / {}", elapsed_str, total_str))
                    .monospace()
                    .color(egui::Color32::from_rgb(160, 200, 240)),
            );
        }
    });

    if sim.setup_boundaries().len() > 1 {
        let setups: Vec<_> = sim
            .setup_boundaries()
            .iter()
            .map(|s| (s.setup_name.clone(), s.start_move))
            .collect();
        let total = sim.total_moves();
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new("Setups:")
                    .small()
                    .color(egui::Color32::from_rgb(140, 140, 150)),
            );
            for (name, start_move) in &setups {
                let is_current = sim.playback.current_move >= *start_move;
                let color = if is_current {
                    egui::Color32::from_rgb(160, 170, 200)
                } else {
                    egui::Color32::from_rgb(110, 110, 120)
                };
                let pct = if total > 0 {
                    *start_move as f32 / total as f32 * 100.0
                } else {
                    0.0
                };
                let button = egui::Button::new(
                    egui::RichText::new(format!("{} {:.0}%", name, pct))
                        .small()
                        .color(color),
                )
                .selected(is_current);
                if ui.add(button).clicked() {
                    sim.playback.current_move = *start_move;
                    sim.playback.playing = false;
                }
            }
        });
    }
}

/// Row 2: Custom-painted per-op timeline bar with collision markers and
/// optional semantic annotation band.
fn draw_boundary_timeline(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    gui: &GuiState,
    session: &ProjectSession,
    max_feed: f64,
    current_boundary: &Option<crate::state::simulation::ToolpathBoundary>,
    active_semantic: &Option<ActiveSemanticItem>,
    events: &mut Vec<AppEvent>,
) {
    if sim.total_moves() > 0 && !sim.boundaries().is_empty() {
        let total_width = ui.available_width();
        let height = 32.0;
        let rounding = 6.0;
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(total_width, height),
            egui::Sense::click_and_drag(),
        );

        let painter = ui.painter_at(rect);
        let total_moves = sim.total_moves().max(1) as f32;

        // Subtle border around the timeline bar
        painter.rect_stroke(
            rect,
            rounding,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(55, 55, 65)),
        );

        for (i, boundary) in sim.boundaries().iter().enumerate() {
            let op_moves = boundary.end_move.saturating_sub(boundary.start_move);
            let x_start = rect.min.x + (boundary.start_move as f32 / total_moves) * total_width;
            let x_end = rect.min.x + (boundary.end_move as f32 / total_moves) * total_width;

            let pc = palette_color(i);
            let dim_color = egui::Color32::from_rgb(
                (pc[0] * 50.0) as u8,
                (pc[1] * 50.0) as u8,
                (pc[2] * 50.0) as u8,
            );
            let color = egui::Color32::from_rgb(
                (pc[0] * 255.0) as u8,
                (pc[1] * 255.0) as u8,
                (pc[2] * 255.0) as u8,
            );

            let seg_rect = egui::Rect::from_min_max(
                egui::pos2(x_start, rect.min.y),
                egui::pos2(x_end, rect.max.y),
            );
            painter.rect_filled(seg_rect, rounding, dim_color);

            let progress = if sim.playback.current_move >= boundary.end_move {
                1.0
            } else if sim.playback.current_move <= boundary.start_move {
                0.0
            } else {
                (sim.playback.current_move - boundary.start_move) as f32 / op_moves.max(1) as f32
            };
            let fill_width = (x_end - x_start) * progress;
            let fill_rect = egui::Rect::from_min_size(
                egui::pos2(x_start, rect.min.y),
                egui::vec2(fill_width, height),
            );
            painter.rect_filled(fill_rect, rounding, color);
        }

        if let Some(ref report) = sim.checks.collision_report {
            let holder_color = egui::Color32::from_rgb(255, 50, 50);
            for col in &report.collisions {
                let x = rect.min.x + (col.move_idx as f32 / total_moves) * total_width;
                painter.line_segment(
                    [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
                    egui::Stroke::new(2.0, holder_color),
                );
            }
        }

        let rapid_color = egui::Color32::from_rgb(255, 160, 40);
        for &idx in &sim.checks.rapid_collision_move_indices {
            let x = rect.min.x + (idx as f32 / total_moves) * total_width;
            painter.line_segment(
                [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
                egui::Stroke::new(1.5, rapid_color),
            );
        }

        draw_tool_load_timeline_markers(&painter, rect, total_moves, total_width, sim, session);

        // Playhead line
        let pos_x = rect.min.x + (sim.playback.current_move as f32 / total_moves) * total_width;
        painter.line_segment(
            [
                egui::pos2(pos_x, rect.min.y - 1.0),
                egui::pos2(pos_x, rect.max.y + 1.0),
            ],
            egui::Stroke::new(2.0, egui::Color32::WHITE),
        );

        // Playhead diamond handle at the top
        let diamond_center = egui::pos2(pos_x, rect.min.y);
        let diamond_size = 4.0;
        let diamond_points = vec![
            egui::pos2(diamond_center.x, diamond_center.y - diamond_size),
            egui::pos2(diamond_center.x + diamond_size, diamond_center.y),
            egui::pos2(diamond_center.x, diamond_center.y + diamond_size),
            egui::pos2(diamond_center.x - diamond_size, diamond_center.y),
        ];
        painter.add(egui::Shape::convex_polygon(
            diamond_points,
            egui::Color32::WHITE,
            egui::Stroke::NONE,
        ));

        // Click or drag to seek. If the pointer is near an actionable safety
        // marker, focus the Safety tab and jump to that finding instead.
        if (response.dragged() || response.clicked())
            && let Some(pos) = response.interact_pointer_pos()
        {
            if response.clicked()
                && let Some(target) =
                    nearest_safety_marker_move(pos.x, rect, total_moves, total_width, sim, session)
            {
                sim.analytics_tab = SimulationAnalyticsTab::Safety;
                sim.playback.current_move = target;
                sim.playback.playing = false;
                events.push(AppEvent::SimJumpToMove(target));
            } else {
                let frac = ((pos.x - rect.min.x) / total_width).clamp(0.0, 1.0);
                sim.playback.current_move = (frac * total_moves) as usize;
                sim.playback.playing = false;
            }
        }
    }

    if sim.debug.enabled
        && let Some(boundary) = current_boundary.as_ref()
    {
        draw_semantic_band(
            ui,
            sim,
            gui,
            max_feed,
            boundary,
            active_semantic.as_ref(),
            events,
        );
    }
}

fn nearest_safety_marker_move(
    pointer_x: f32,
    rect: egui::Rect,
    total_moves: f32,
    total_width: f32,
    sim: &SimulationState,
    session: &ProjectSession,
) -> Option<usize> {
    let rapid = sim.checks.rapid_collision_move_indices.iter().copied();
    let tool_load = tool_load_marker_moves(sim, session);
    rapid
        .chain(tool_load)
        .map(|move_index| {
            let x = rect.min.x + (move_index as f32 / total_moves) * total_width;
            (move_index, (pointer_x - x).abs())
        })
        .filter(|(_, distance)| *distance <= 7.0)
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .map(|(move_index, _)| move_index)
}

fn tool_load_marker_moves(sim: &SimulationState, session: &ProjectSession) -> Vec<usize> {
    let sim_trace = sim
        .results
        .as_ref()
        .and_then(|results| results.cut_trace.as_deref());
    let report = rs_cam_core::gcode::project_load_report(session, sim_trace);
    let Some(trace) = sim_trace else {
        return Vec::new();
    };
    report
        .per_toolpath
        .iter()
        .filter_map(|verdict| first_exceeded_tool_load_move(sim, trace, verdict))
        .collect()
}

fn draw_tool_load_timeline_markers(
    painter: &egui::Painter,
    rect: egui::Rect,
    total_moves: f32,
    total_width: f32,
    sim: &SimulationState,
    session: &ProjectSession,
) {
    let sim_trace = sim
        .results
        .as_ref()
        .and_then(|results| results.cut_trace.as_deref());
    let report = rs_cam_core::gcode::project_load_report(session, sim_trace);
    let Some(trace) = sim_trace else {
        return;
    };

    for verdict in report.per_toolpath {
        if let Some(global_move) = first_exceeded_tool_load_move(sim, trace, &verdict) {
            let x = rect.min.x + (global_move as f32 / total_moves) * total_width;
            painter.line_segment(
                [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
                egui::Stroke::new(2.0, egui::Color32::from_rgb(230, 60, 70)),
            );
        } else if verdict.any_unmodeled()
            && let Some(boundary) = sim
                .boundaries()
                .iter()
                .find(|boundary| boundary.id.0 == verdict.toolpath_id)
        {
            let x = rect.min.x + (boundary.start_move as f32 / total_moves) * total_width;
            painter.line_segment(
                [egui::pos2(x, rect.min.y), egui::pos2(x, rect.center().y)],
                egui::Stroke::new(1.5, egui::Color32::from_rgb(250, 200, 80)),
            );
        }
    }
}

fn first_exceeded_tool_load_move(
    sim: &SimulationState,
    trace: &rs_cam_core::simulation_cut::SimulationCutTrace,
    verdict: &rs_cam_core::tool_load::ToolpathLoadVerdict,
) -> Option<usize> {
    let sample_index = [&verdict.chipload, &verdict.power, &verdict.deflection]
        .iter()
        .find_map(|criterion| match criterion {
            rs_cam_core::tool_load::Verdict::Exceeds { sample_range, .. } => {
                Some(sample_range.start)
            }
            rs_cam_core::tool_load::Verdict::Within { .. }
            | rs_cam_core::tool_load::Verdict::Unmodeled { .. } => None,
        })?;
    let sample = trace.samples.get(sample_index)?;
    let boundary_start = sim
        .boundaries()
        .iter()
        .find(|boundary| boundary.id.0 == sample.toolpath_id)
        .map(|boundary| boundary.start_move)
        .unwrap_or_default();
    Some(boundary_start + sample.move_index)
}

/// Row 3: Playback speed slider and preset buttons.
fn draw_speed_controls(ui: &mut egui::Ui, sim: &mut SimulationState) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Speed:")
                .small()
                .color(egui::Color32::from_rgb(130, 130, 145)),
        )
        .on_hover_text(
            "Keyboard: [ and ] to change speed, Space to play/pause, Left/Right to step, Home/End to jump.",
        );

        for &(label, speed) in &[
            ("100", 100.0),
            ("500", 500.0),
            ("1k", 1000.0),
            ("5k", 5000.0),
            ("10k", 10000.0),
            ("Max", 50000.0),
        ] {
            let is_selected = (sim.playback.speed - speed).abs() < 1.0;
            let btn = egui::Button::new(egui::RichText::new(label).small()).selected(is_selected);
            if ui.add(btn).clicked() {
                sim.playback.speed = speed;
            }
        }

        ui.separator();
        ui.add(
            egui::DragValue::new(&mut sim.playback.speed)
                .range(10.0..=50000.0)
                .speed(50.0)
                .suffix(" mv/s"),
        )
        .on_hover_text("Playback speed in moves per second.\n[ and ] to decrease/increase.");
    });
}

/// Estimate elapsed and total time (in seconds) based on feed rates.
fn estimate_times(sim: &SimulationState, session: &ProjectSession, gui: &GuiState) -> (f64, f64) {
    let mut total_secs = 0.0;
    let mut elapsed_secs = 0.0;

    for boundary in sim.boundaries() {
        if let Some(rt) = gui.toolpath_rt.get(&boundary.id.0)
            && let Some(result) = &rt.result
            && let Some((_, tc)) = session.find_toolpath_config_by_id(boundary.id.0)
        {
            let feed = tc.operation.feed_rate();
            let op_time = (result.stats.cutting_distance / feed) * 60.0;
            total_secs += op_time;

            // Estimate elapsed time for this op
            let op_moves = boundary.end_move.saturating_sub(boundary.start_move);
            let progress = if sim.playback.current_move >= boundary.end_move {
                1.0
            } else if sim.playback.current_move <= boundary.start_move {
                0.0
            } else {
                (sim.playback.current_move - boundary.start_move) as f64 / op_moves.max(1) as f64
            };
            elapsed_secs += op_time * progress;
        }
    }

    (elapsed_secs, total_secs)
}

fn format_time(secs: f64) -> String {
    let m = (secs / 60.0).floor() as u32;
    let s = (secs % 60.0) as u32;
    format!("{}:{:02}", m, s)
}

// SAFETY: item_index and depths[] from semantic index built from trace.items
#[allow(clippy::indexing_slicing)]
fn draw_semantic_band(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    gui: &GuiState,
    max_feed: f64,
    boundary: &crate::state::simulation::ToolpathBoundary,
    active_semantic: Option<&ActiveSemanticItem>,
    events: &mut Vec<AppEvent>,
) {
    let Some(rt) = gui.toolpath_rt.get(&boundary.id.0) else {
        return;
    };
    let Some(trace) = rt.semantic_trace.as_ref() else {
        return;
    };
    let Some(index) = sim.debug.semantic_indexes.get(&boundary.id).cloned() else {
        return;
    };

    let local_total = boundary.end_move.saturating_sub(boundary.start_move).max(1);
    let total_width = ui.available_width();
    let height = 10.0;
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new("Semantic timeline")
            .small()
            .color(egui::Color32::from_rgb(140, 140, 155)),
    );
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(total_width, height), egui::Sense::click());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, egui::Color32::from_rgb(30, 30, 40));

    let mut segments = index.move_item_indices.clone();
    segments.sort_by_key(|item_index| index.depths[*item_index]);
    for item_index in segments {
        let item = &trace.items[item_index];
        let (Some(move_start), Some(move_end)) = (item.move_start, item.move_end) else {
            continue;
        };
        let x_start = rect.min.x + (move_start as f32 / local_total as f32) * total_width;
        let x_end = rect.min.x + ((move_end + 1) as f32 / local_total as f32) * total_width;
        let seg_rect = egui::Rect::from_min_max(
            egui::pos2(x_start, rect.min.y),
            egui::pos2(x_end.max(x_start + 1.0), rect.max.y),
        );
        let color = semantic_kind_color(&item.kind);
        painter.rect_filled(seg_rect, 1.0, color.linear_multiply(0.85));
        if active_semantic
            .is_some_and(|active| active.toolpath_id == boundary.id && active.item.id == item.id)
        {
            painter.rect_stroke(seg_rect, 1.0, egui::Stroke::new(1.5, egui::Color32::WHITE));
        }
    }

    if let Some(debug_trace) = rt.debug_trace.as_ref() {
        for annotation in &debug_trace.annotations {
            let x = rect.min.x + (annotation.move_index as f32 / local_total as f32) * total_width;
            painter.line_segment(
                [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
                egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 210, 120)),
            );
        }
    }

    if let Some(cut_trace) = sim
        .results
        .as_ref()
        .and_then(|results| results.cut_trace.as_ref())
    {
        for issue in cut_trace
            .issues
            .iter()
            .filter(|issue| issue.toolpath_id == boundary.id.0)
        {
            let x = rect.min.x + (issue.move_index as f32 / local_total as f32) * total_width;
            let color = match issue.kind {
                rs_cam_core::simulation_cut::SimulationCutIssueKind::AirCut => {
                    egui::Color32::from_rgb(255, 120, 80)
                }
                rs_cam_core::simulation_cut::SimulationCutIssueKind::LowEngagement => {
                    egui::Color32::from_rgb(250, 200, 100)
                }
            };
            painter.circle_filled(egui::pos2(x, rect.center().y), 2.5, color);
        }
    }

    let local_move = sim
        .playback
        .current_move
        .saturating_sub(boundary.start_move)
        .min(local_total);
    let pos_x = rect.min.x + (local_move as f32 / local_total as f32) * total_width;
    painter.line_segment(
        [
            egui::pos2(pos_x, rect.min.y - 1.0),
            egui::pos2(pos_x, rect.max.y + 1.0),
        ],
        egui::Stroke::new(1.5, egui::Color32::WHITE),
    );

    if response.clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        if let Some(debug_trace) = rt.debug_trace.as_ref() {
            let nearest_annotation = debug_trace
                .annotations
                .iter()
                .enumerate()
                .map(|(index, annotation)| {
                    let x = rect.min.x
                        + (annotation.move_index as f32 / local_total as f32) * total_width;
                    (index, annotation, (pointer.x - x).abs())
                })
                .filter(|(_, _, distance)| *distance <= 5.0)
                .min_by(|left, right| left.2.total_cmp(&right.2));
            if let Some((annotation_index, annotation, _)) = nearest_annotation
                && let Some(target) = sim.trace_target_for_annotation(boundary.id, annotation)
            {
                sim.debug.focused_issue_index = None;
                sim.debug.focused_hotspot = None;
                sim.analytics_tab = SimulationAnalyticsTab::DebugTrace;
                sim.clear_pinned_semantic_item();
                let _ = annotation_index;
                events.push(AppEvent::SimJumpToMove(target.move_index));
                return;
            }
        }

        if let Some(cut_trace) = sim
            .results
            .as_ref()
            .and_then(|results| results.cut_trace.as_ref())
        {
            let nearest_issue = cut_trace
                .issues
                .iter()
                .filter(|issue| issue.toolpath_id == boundary.id.0)
                .map(|issue| {
                    let x =
                        rect.min.x + (issue.move_index as f32 / local_total as f32) * total_width;
                    (issue.clone(), (pointer.x - x).abs())
                })
                .filter(|(_, distance)| *distance <= 6.0)
                .min_by(|left, right| left.1.total_cmp(&right.1));
            if let Some((issue, _)) = nearest_issue
                && let Some(target) = sim.trace_target_for_cut_issue(&issue)
            {
                if let Some(item_id) = target.semantic_item_id {
                    sim.pin_semantic_item(boundary.id, item_id);
                }
                sim.debug.focused_issue_index = None;
                sim.debug.focused_hotspot = None;
                sim.analytics_tab = SimulationAnalyticsTab::CutQuality;
                events.push(AppEvent::SimJumpToMove(target.move_index));
                return;
            }
        }

        let semantic_hit = index
            .move_item_indices
            .iter()
            .copied()
            .filter_map(|item_index| {
                let item = trace.items.get(item_index)?;
                let (Some(move_start), Some(move_end)) = (item.move_start, item.move_end) else {
                    return None;
                };
                let x_start = rect.min.x + (move_start as f32 / local_total as f32) * total_width;
                let x_end = rect.min.x + ((move_end + 1) as f32 / local_total as f32) * total_width;
                (pointer.x >= x_start && pointer.x <= x_end).then_some((
                    item_index,
                    index.depths[item_index],
                    move_end.saturating_sub(move_start),
                ))
            })
            .max_by(|left, right| left.1.cmp(&right.1).then_with(|| right.2.cmp(&left.2)))
            .map(|(item_index, _, _)| item_index);

        if let Some(item_index) = semantic_hit {
            let item = &trace.items[item_index];
            sim.pin_semantic_item(boundary.id, item.id);
            sim.debug.focused_issue_index = None;
            sim.debug.focused_hotspot = None;
            if let Some(target) =
                sim.trace_target_for_item(gui, max_feed, boundary.id, item.id, false)
            {
                events.push(AppEvent::SimJumpToMove(target.move_index));
            }
            return;
        }

        let frac = ((pointer.x - rect.min.x) / total_width).clamp(0.0, 1.0);
        let local_move = (frac * local_total as f32) as usize;
        sim.clear_pinned_semantic_item();
        sim.debug.focused_issue_index = None;
        sim.debug.focused_hotspot = None;
        events.push(AppEvent::SimJumpToMove(boundary.start_move + local_move));
    }
}

