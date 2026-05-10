use super::AppEvent;
use super::sim_debug::semantic_kind_color;
use crate::render::toolpath_render::palette_color;
use crate::state::runtime::GuiState;
use crate::state::simulation::{ActiveSemanticItem, SimulationAnalyticsTab, SimulationState};
use egui_plot::{Line, Plot, PlotPoints, Polygon};
use rs_cam_core::session::ProjectSession;
use rs_cam_core::simulation_cut::SimulationCutSample;
use rs_cam_core::tool_load::{Confidence, ToolLoadReport};

/// Per-toolpath line in the signal plot: a colour plus a sequence of
/// `(global_move_index, [x, y])` points decimated for the current X span.
type GroupPoints = Vec<(egui::Color32, Vec<(usize, [f64; 2])>)>;

/// One signal plot track: label, value extractor over `SimulationCutSample`,
/// stroke colour, and an optional `(min, max)` envelope for shading.
type SignalTrack = (
    &'static str,
    fn(&SimulationCutSample) -> Option<f64>,
    egui::Color32,
    Option<(f64, f64)>,
);

/// Bottom panel in simulation workspace: transport controls, timeline scrubber, speed control.
pub fn draw(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    session: &ProjectSession,
    gui: &GuiState,
    events: &mut Vec<AppEvent>,
) {
    // Recomputed each frame by the widgets below. `update_live_sim()` runs
    // after UI/event handling and uses this to defer expensive stock replay
    // while the pointer is actively dragging a scrubber.
    sim.playback.scrub_drag_active = false;
    let max_feed = session.machine().max_feed_mm_min;
    if sim.playback.playing || sim.playback.display_mesh_preview {
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new("⚡ Mesh quality reduced for playback — pause for full render")
                    .small()
                    .strong()
                    .color(egui::Color32::from_rgb(255, 220, 130)),
            );
        });
        ui.add_space(2.0);
    }
    sim.sync_debug_state(gui, max_feed);
    let active_semantic = sim.active_semantic_item(gui, max_feed);
    let current_boundary = sim.current_boundary().cloned();

    // Compute the project tool-load report once for this frame and pass
    // it to every sub-draw that needs it. Without this memo the report is
    // built 3-4× per frame: once for the verdict HUD, once for the
    // boundary-timeline markers, once for the safety-marker click hit
    // test, and once more on each hover for the marker tooltip. On a
    // wanaka-sized job (8 TPs, ~600k samples) that's the worst hot path
    // in the bottom panel. Right panel (sim_diagnostics) already memoes
    // per its own draw.
    let load_report = sim.cached_load_report(session, gui.edit_counter);

    // Boundary timeline always shows the whole project — user wants the
    // full picture (Pin Drill, Back Rough, ...) at a glance regardless
    // of which TP is focused below. The signal-spine graphs scope to
    // the focused TP independently. The two widgets use different X
    // coordinate spaces; markers on each are accurate within their own
    // widget but won't visually align with the other.
    draw_transport_and_scrubber(ui, sim, session, gui, events);
    draw_verdict_hud(ui, sim, gui, max_feed, &load_report, events);
    draw_boundary_timeline(
        ui,
        sim,
        gui,
        max_feed,
        &load_report,
        &current_boundary,
        &active_semantic,
        events,
    );
    draw_signal_spine(ui, sim, session, gui, &load_report, events);
}

fn draw_verdict_hud(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    gui: &GuiState,
    max_feed: f64,
    load_report: &ToolLoadReport,
    _events: &mut Vec<AppEvent>,
) {
    let issue_count = sim.issues(gui, max_feed).len();

    let (ok, warn, bad, unmodeled, collision_count, trace_count) = {
        let counts = verdict_counts(load_report);
        let collision_count = sim.checks.rapid_collisions.len() + sim.checks.holder_collision_count;
        let trace_count = gui
            .toolpath_rt
            .values()
            .filter(|rt| rt.debug_trace.is_some() || rt.semantic_trace.is_some())
            .count();
        (
            counts.0,
            counts.1,
            counts.2,
            counts.3,
            collision_count,
            trace_count,
        )
    };

    egui::Frame::default()
        .fill(egui::Color32::from_rgb(30, 32, 42))
        .inner_margin(egui::Margin::symmetric(6.0, 4.0))
        .rounding(4.0)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                info_pill(
                    ui,
                    format!("✓ load {ok}"),
                    egui::Color32::from_rgb(85, 180, 110),
                    "Toolpaths within modeled load limits.",
                );
                info_pill(
                    ui,
                    format!("⚠ unmodeled {unmodeled}"),
                    egui::Color32::from_rgb(210, 170, 80),
                    "Load criteria the gate could not model (drill cycles, no vendor data, etc.).",
                );
                info_pill(
                    ui,
                    format!("✕ exceeds {bad}"),
                    egui::Color32::from_rgb(220, 90, 90),
                    "Load-limit exceedances. Click the red lines on the boundary timeline below to navigate.",
                );
                info_pill(
                    ui,
                    format!("~ approx {warn}"),
                    egui::Color32::from_rgb(120, 150, 220),
                    "Approximate or advisory verdicts.",
                );
                let collision_color = if collision_count == 0 {
                    egui::Color32::from_rgb(120, 210, 140)
                } else {
                    egui::Color32::from_rgb(255, 120, 110)
                };
                info_pill(
                    ui,
                    format!("collisions {collision_count}"),
                    collision_color,
                    "Rapid/holder collisions. Click the red lines on the boundary timeline below to navigate.",
                );
                info_pill(
                    ui,
                    format!("issues {issue_count}"),
                    egui::Color32::from_rgb(230, 190, 90),
                    "Air cuts and low-engagement clusters detected during simulation.",
                );
                info_pill(
                    ui,
                    format!("traces {trace_count}"),
                    egui::Color32::from_rgb(150, 170, 230),
                    "Generator traces recorded for inspection.",
                );
            });
        });
}

fn verdict_counts(report: &ToolLoadReport) -> (usize, usize, usize, usize) {
    use rs_cam_core::tool_load::verdict::LoadState;
    let mut ok = 0;
    let mut warn = 0;
    let mut bad = 0;
    let mut unmodeled = 0;
    for tp in &report.per_toolpath {
        for (state, confidence) in [
            (tp.chipload.state(), tp.chipload.confidence()),
            (tp.power.state(), tp.power.confidence()),
            (tp.deflection.state(), tp.deflection.confidence()),
        ] {
            match state {
                LoadState::Within => ok += 1,
                LoadState::Unmodeled => unmodeled += 1,
                LoadState::Exceeds => bad += 1,
            }
            if matches!(confidence, Some(Confidence::Approximate(_))) {
                warn += 1;
            }
        }
    }
    (ok, warn, bad, unmodeled)
}

fn info_pill(ui: &mut egui::Ui, text: String, color: egui::Color32, hover: &str) {
    ui.label(egui::RichText::new(text).small().color(color))
        .on_hover_text(hover);
}

fn draw_signal_spine(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    session: &ProjectSession,
    gui: &GuiState,
    load_report: &ToolLoadReport,
    events: &mut Vec<AppEvent>,
) {
    // Signal graphs only render when cut-metric capture was on for the
    // last sim run. If there's no trace, hide this section entirely —
    // the user enables capture from the left panel's "Setup & run" and
    // re-runs to populate it.
    // Acquire the trace as an Arc so we can keep an immutable handle for
    // reads while still calling `&mut sim` methods (e.g. ensure_built).
    let trace_arc = sim
        .results
        .as_ref()
        .and_then(|r| r.cut_trace.as_ref())
        .map(std::sync::Arc::clone);
    let Some(trace_arc) = trace_arc else { return };
    sim.debug.span_aggregates.ensure_built(&trace_arc);
    let trace = trace_arc.as_ref();
    let total_moves = sim.total_moves();
    if total_moves == 0 {
        return;
    }

    // Group cutting samples by toolpath using the per-trace cache. Without
    // this, the per-frame `for sample in samples.iter()` + linear
    // `groups.iter_mut().find()` was O(samples × toolpaths) and hundreds
    // of thousands of ops on a real job.
    let boundaries: Vec<(crate::state::toolpath::ToolpathId, usize, egui::Color32)> = sim
        .boundaries()
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let pc = palette_color(i);
            let color = egui::Color32::from_rgb(
                (pc[0] * 255.0) as u8,
                (pc[1] * 255.0) as u8,
                (pc[2] * 255.0) as u8,
            );
            (b.id, b.start_move, color)
        })
        .collect();

    let mut groups: Vec<ToolpathGroup> = boundaries
        .iter()
        .map(|(id, start, color)| {
            let indices = sim.debug.span_aggregates.cutting_indices_for(*id);
            let samples: Vec<&SimulationCutSample> = indices
                .iter()
                .filter_map(|&idx| trace.samples.get(idx))
                .collect();
            ToolpathGroup {
                toolpath_id: *id,
                start_move: *start,
                color: *color,
                samples,
            }
        })
        .collect();

    // Inspector pin / playback-derived focus filters the per-TP groups so
    // graphs show only the selected toolpath when one is focused. The
    // boundary timeline above stays project-wide regardless.
    let focused_id = sim.focused_toolpath();
    if let Some(id) = focused_id {
        groups.retain(|g| g.toolpath_id == id);
    }

    if groups.iter().all(|g| g.samples.is_empty()) {
        ui.label(
            egui::RichText::new("No cutting samples captured.")
                .small()
                .italics(),
        );
        return;
    }

    ui.add_space(4.0);
    let active_x = Some(sim.playback.current_move as f64);
    let chipload_envelopes = sim.cached_chipload_envelopes(session, gui.edit_counter);
    let envelope = focused_id
        .and_then(|id| chipload_envelopes.get(&id.0))
        .map(|range| (range.start, range.end));

    // F6.1 — timeline point markers are reserved for Critical/Risky gate
    // trips only. The per-`(toolpath_id, semantic_item_id)` aggregator
    // dots that used to render here (one per `trace.hotspots` entry, ~850
    // on wanaka TP 1) were reporting buckets, not problem flags, and
    // drowned the real signal in a sea of orange. The diagnostics-panel
    // table is the home for the per-bucket data; the timeline is the
    // safety channel.
    //
    // See planning/OPTIMIZER_UX_DIALIN_FIXES.md F6 for the broader
    // reframe (graph panels with bands instead of dots for continuous
    // data) — this is just the dot-removal slice.
    let mut hotspots: Vec<HotspotMarker> = Vec::new();

    // Add tool-load gate markers — one dot per Exceeds verdict at the
    // gate's actual worst-sample move. Reuses the timeline's already-
    // memoed `load_report` (passed in) instead of building a second copy
    // per frame.
    for (i, verdict) in load_report.per_toolpath.iter().enumerate() {
        if let Some(focus) = focused_id
            && verdict.toolpath_id != focus.0
        {
            continue;
        }
        if let Some(global_move) = first_exceeded_tool_load_move(sim, trace, verdict) {
            hotspots.push(HotspotMarker {
                index: 10000 + i,
                global_move,
            });
        }
    }

    let display_x = sim.hovered_x;
    let mut new_hovered: Option<f64> = None;
    let mut clicked_hotspot: Option<(usize, usize)> = None;
    let mut signal_drag_active = false;
    let total_moves_f = total_moves as f64;

    // X-axis range for the tracks. When a TP is focused, zoom the X axis to
    // just that TP's move range so the data fills the plot width. The
    // boundary timeline above stays whole-project regardless.
    let x_range: (f64, f64) = focused_id
        .and_then(|id| sim.boundaries().iter().find(|b| b.id == id))
        .map(|b| (b.start_move as f64, b.end_move as f64))
        .unwrap_or((0.0, total_moves_f));

    // DepthPass bands behind every track. Computed once and reused so each
    // track polygon renders at exactly the same X positions as the others
    // and as the timeline ribbon. Each entry is (global_start, global_end,
    // is_scope_selected). When the chip-row scope picks a DepthPass on the
    // focused TP, that band is brighter; otherwise alternating shades give
    // pass boundaries a faint visual rhythm without explicit lines.
    let pass_bands: Vec<(f64, f64, bool)> = focused_id
        .and_then(|id| {
            let boundary = sim.boundaries().iter().find(|b| b.id == id)?;
            let rt = gui.toolpath_rt.get(&id.0)?;
            let result = rt.result.as_ref()?;
            if !result.spans_valid() {
                return None;
            }
            let spans = result.spans();
            let scope_span_id = sim.debug.span_scope.span_id;
            let bands: Vec<(f64, f64, bool)> = spans
                .iter()
                .enumerate()
                .filter(|(_, s)| matches!(s.kind, rs_cam_core::toolpath_spans::SpanKind::DepthPass))
                .map(|(idx, s)| {
                    let g_start = (boundary.start_move + s.start_move) as f64;
                    let g_end = (boundary.start_move + s.end_move) as f64;
                    let selected = scope_span_id == Some(idx as u32);
                    (g_start, g_end, selected)
                })
                .collect();
            (!bands.is_empty()).then_some(bands)
        })
        .unwrap_or_default();

    let tracks: [SignalTrack; 5] = [
        (
            "chipload",
            // Filter air-cut samples (radial_engagement < 0.02) from the
            // chipload track. Same threshold the gate's verdict uses
            // (`tool_load::chipload::evaluate`). Plotting them shows a
            // near-zero static line that reads as "static chipload during
            // air" — visually misleading because air cuts have no real
            // chip. Drawing only meaningful samples lets the eye focus on
            // the engaged-cut chipload distribution.
            |s| {
                if s.radial_engagement < 0.02 {
                    None
                } else {
                    s.effective_chip_thickness_mm
                }
            },
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

    // Header row showing what's currently in focus. Focus follows the
    // playing toolpath — clicking a row in the left panel jumps playback
    // there, and the focus naturally moves with playback.
    if let Some(id) = focused_id {
        let focus_name = sim
            .boundaries()
            .iter()
            .find(|b| b.id == id)
            .map_or_else(|| format!("TP {}", id.0 + 1), |b| b.name.clone());
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Playing: ")
                    .small()
                    .color(egui::Color32::from_rgb(140, 140, 155)),
            );
            ui.label(egui::RichText::new(&focus_name).small().strong());
        });
    }

    // Stacked scroll area: each track is taller (90 px) and gets vertical
    // separation, so the user can read 2–3 at once and scroll to the rest
    // without losing their X-axis lock.
    egui::ScrollArea::vertical()
        .id_salt("signal_spine_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (label, value_fn, color, env) in tracks {
                draw_signal_track(
                    ui,
                    label,
                    &groups,
                    value_fn,
                    color,
                    active_x,
                    display_x,
                    &mut new_hovered,
                    env,
                    &hotspots,
                    x_range,
                    &pass_bands,
                    &mut clicked_hotspot,
                    &mut signal_drag_active,
                    events,
                );
                ui.add_space(8.0);
            }
        });

    if signal_drag_active {
        sim.playback.scrub_drag_active = true;
    }
    sim.hovered_x = new_hovered;
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

struct ToolpathGroup<'a> {
    toolpath_id: crate::state::toolpath::ToolpathId,
    start_move: usize,
    color: egui::Color32,
    samples: Vec<&'a SimulationCutSample>,
}

#[derive(Clone, Copy)]
struct HotspotMarker {
    index: usize,
    global_move: usize,
}

const SIGNAL_MAX_POINTS: usize = 1600;

#[allow(clippy::too_many_arguments)]
fn draw_signal_track(
    ui: &mut egui::Ui,
    label: &str,
    groups: &[ToolpathGroup<'_>],
    value_fn: impl Fn(&SimulationCutSample) -> Option<f64>,
    color: egui::Color32,
    active_x: Option<f64>,
    display_x: Option<f64>,
    new_hovered: &mut Option<f64>,
    envelope: Option<(f64, f64)>,
    hotspots: &[HotspotMarker],
    x_range: (f64, f64),
    pass_bands: &[(f64, f64, bool)],
    clicked_hotspot: &mut Option<(usize, usize)>,
    scrub_drag_active: &mut bool,
    events: &mut Vec<AppEvent>,
) {
    let (x_min, x_max) = x_range;
    let x_span = (x_max - x_min).max(1.0);
    // Build per-toolpath point lists in global-move space, decimating each
    // independently so the global cap applies fairly across toolpaths.
    let per_group_cap = (SIGNAL_MAX_POINTS / groups.len().max(1)).max(64);
    // Decimate by **max-per-bucket** rather than stride sampling. Stride
    // sampling drops single-sample spikes (the chipload trace's full-slot
    // peaks at every region entry), making the gate's reported peak
    // invisible on the graph. Max-per-bucket preserves the worst-case
    // sample per X-bucket, so a single-sample chipload spike at sample
    // 86603 actually appears as a vertical bar in the rendered line.
    let group_points: GroupPoints = groups
        .iter()
        .filter_map(|group| {
            let pts: Vec<(usize, [f64; 2])> = group
                .samples
                .iter()
                .filter_map(|s| {
                    let global_move = group.start_move + s.move_index;
                    value_fn(s).map(|y| (global_move, [global_move as f64, y]))
                })
                .collect();
            if pts.is_empty() {
                return None;
            }
            if pts.len() <= per_group_cap {
                return Some((group.color, pts));
            }
            // Bucket the points and keep the max-Y point per bucket.
            // Buckets are equal-width in sample-index space (which maps
            // 1-to-1 to X position in the plot for this group). Result
            // size is ≤ per_group_cap by construction.
            let bucket_size = pts.len().div_ceil(per_group_cap);
            let mut decimated: Vec<(usize, [f64; 2])> = Vec::with_capacity(per_group_cap + 1);
            let mut chunk_iter = pts.chunks(bucket_size);
            for chunk in chunk_iter.by_ref() {
                if let Some(peak) = chunk.iter().max_by(|a, b| {
                    a.1[1]
                        .partial_cmp(&b.1[1])
                        .unwrap_or(std::cmp::Ordering::Equal)
                }) {
                    decimated.push(*peak);
                }
            }
            Some((group.color, decimated))
        })
        .collect();
    if group_points.is_empty() {
        return;
    }

    let min_y = group_points
        .iter()
        .flat_map(|(_, pts)| pts.iter().map(|(_, p)| p[1]))
        .fold(f64::INFINITY, f64::min);
    let max_y = group_points
        .iter()
        .flat_map(|(_, pts)| pts.iter().map(|(_, p)| p[1]))
        .fold(f64::NEG_INFINITY, f64::max);

    // All five tracks share this link group so X-zoom/pan happens in lockstep.
    // Y is independent (each metric has its own scale).
    // Track label above the plot — the embedded plot legend isn't very
    // visible at this size, and a leading label is more scannable when
    // tracks are stacked vertically in a scroll area.
    ui.label(egui::RichText::new(label).small().strong().color(color));

    // The link group ID encodes the X range, so changing focus (which
    // changes x_range) creates a *fresh* link group. Without this, egui_plot
    // persists the previous wider X range across frames and `include_x` only
    // expands — so small TPs would render squished inside the stale range.
    let link_group = ui
        .id()
        .with(("signal_spine_x_link", x_min.to_bits(), x_max.to_bits()));
    let response = Plot::new(format!("signal_track_{label}"))
        .height(90.0)
        // Wheel zooms horizontally; drag is *not* used by the plot — we
        // intercept it below as a scrub gesture instead so dragging across
        // a track scrubs playback rather than panning the graph (the prior
        // behaviour confused users since "drag" looked like a scrub).
        .allow_zoom([true, false])
        .allow_drag([false, false])
        .allow_scroll([false, false])
        .allow_boxed_zoom(false)
        .link_axis(link_group, [true, false])
        .link_cursor(link_group, [true, false].into())
        .show_axes([true, true])
        .show_grid([true, true])
        .include_x(x_min)
        .include_x(x_max)
        .show(ui, |plot_ui| {
            // DepthPass bands first so the data lines and envelope shading
            // render on top of them. Bands have transparent strokes and
            // disabled hover so they don't show up in legend hover or
            // intercept clicks.
            if !pass_bands.is_empty() {
                let band_y_min = min_y - (max_y - min_y).abs() * 0.1;
                let band_y_max = max_y + (max_y - min_y).abs() * 0.1;
                let band_stroke = egui::Stroke::new(0.0, egui::Color32::TRANSPARENT);
                for (idx, (g_start, g_end, selected)) in pass_bands.iter().enumerate() {
                    let fill = if *selected {
                        egui::Color32::from_rgba_premultiplied(70, 95, 160, 28)
                    } else if idx % 2 == 0 {
                        egui::Color32::from_rgba_premultiplied(50, 60, 90, 12)
                    } else {
                        egui::Color32::from_rgba_premultiplied(35, 45, 70, 8)
                    };
                    plot_ui.polygon(
                        Polygon::new(PlotPoints::from(vec![
                            [*g_start, band_y_min],
                            [*g_end, band_y_min],
                            [*g_end, band_y_max],
                            [*g_start, band_y_max],
                        ]))
                        .fill_color(fill)
                        .stroke(band_stroke)
                        .allow_hover(false)
                        .name(""),
                    );
                }
            }

            // One Line per toolpath, further split into contiguous runs
            // wherever consecutive surviving samples have a sample-index
            // gap > MAX_LINE_BRIDGE_GAP. A gap means the value_fn returned
            // None for the in-between samples (e.g. air-cut on chipload,
            // or zero-DOC on axial DOC) — drawing one Line across the gap
            // bridges those samples with a misleading diagonal segment.
            // Splitting at the gap makes air-cut sections render as
            // explicit blanks, matching the user's mental model: "samples
            // with no signal don't have a line".
            //
            // Threshold > 1 (not > 0) absorbs decimation-induced gaps:
            // when the per-bucket-max decimator picks one point per
            // bucket, surviving consecutive points are bucket_size apart
            // even with no air cuts. Use a slightly looser threshold to
            // accommodate that without bridging real air-cut runs.
            const MAX_LINE_BRIDGE_GAP: usize = 16;
            for (_tp_color, pts) in &group_points {
                let mut run_start = 0usize;
                for (i, pair) in pts.windows(2).enumerate() {
                    // SAFETY: windows(2) guarantees len == 2
                    #[allow(clippy::indexing_slicing)]
                    let (prev_move, cur_move) = (pair[0].0, pair[1].0);
                    if cur_move.saturating_sub(prev_move) > MAX_LINE_BRIDGE_GAP {
                        // i is the index of pair[0]; the break is between i and i+1.
                        let end = i + 1;
                        // SAFETY: end <= pts.len() because windows(2) yields
                        // indices i in 0..pts.len()-1.
                        #[allow(clippy::indexing_slicing)]
                        let xy: Vec<[f64; 2]> =
                            pts[run_start..end].iter().map(|(_, p)| *p).collect();
                        if xy.len() >= 2 {
                            plot_ui.line(Line::new(PlotPoints::from(xy)).name(label).color(color));
                        }
                        run_start = end;
                    }
                }
                // SAFETY: run_start is monotonically advanced by the loop
                // above using indices bounded by pts.len(), so the suffix
                // slice is always in range.
                #[allow(clippy::indexing_slicing)]
                let xy: Vec<[f64; 2]> = pts[run_start..].iter().map(|(_, p)| *p).collect();
                if xy.len() >= 2 {
                    plot_ui.line(Line::new(PlotPoints::from(xy)).name(label).color(color));
                }
            }

            if let Some((cl_min, cl_max)) = envelope {
                // Shade the out-of-bounds zones so users can see at a
                // glance which segments are breaking the envelope.
                // Above-max → red (breakage). Below-min → amber (burn).
                // The clear band between cl_min and cl_max is the safe
                // chipload zone. Stroke is transparent — fill only.
                let transparent = egui::Stroke::new(0.0, egui::Color32::TRANSPARENT);
                if cl_max < max_y {
                    let breakage_top = max_y.max(cl_max);
                    plot_ui.polygon(
                        Polygon::new(PlotPoints::from(vec![
                            [x_min, cl_max],
                            [x_max, cl_max],
                            [x_max, breakage_top],
                            [x_min, breakage_top],
                        ]))
                        .fill_color(egui::Color32::from_rgba_premultiplied(70, 18, 18, 90))
                        .stroke(transparent)
                        .allow_hover(false)
                        .name("breakage zone"),
                    );
                }
                if cl_min > min_y {
                    let burn_bottom = min_y.min(cl_min);
                    plot_ui.polygon(
                        Polygon::new(PlotPoints::from(vec![
                            [x_min, burn_bottom],
                            [x_max, burn_bottom],
                            [x_max, cl_min],
                            [x_min, cl_min],
                        ]))
                        .fill_color(egui::Color32::from_rgba_premultiplied(90, 60, 12, 80))
                        .stroke(transparent)
                        .allow_hover(false)
                        .name("burn zone"),
                    );
                }
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

            if let Some(x) = active_x {
                plot_ui.line(
                    Line::new(PlotPoints::from(vec![[x, min_y], [x, max_y]]))
                        .color(egui::Color32::from_rgb(245, 245, 245))
                        .name("playback"),
                );
            }

            if let Some(x) = display_x {
                plot_ui.line(
                    Line::new(PlotPoints::from(vec![[x, min_y], [x, max_y]]))
                        .color(egui::Color32::from_rgb(200, 240, 100))
                        .style(egui_plot::LineStyle::Dashed { length: 4.0 }),
                );
                if let Some((_, point)) = nearest_in_groups(x, &group_points) {
                    plot_ui.text(egui_plot::Text::new(
                        egui_plot::PlotPoint::new(point[0], point[1]),
                        format!("{label}: {:.3} @ move {}", point[1], point[0] as usize),
                    ));
                }
            }

            if !hotspots.is_empty() {
                let hotspot_pts: Vec<[f64; 2]> = hotspots
                    .iter()
                    .filter_map(|hs| {
                        nearest_in_groups(hs.global_move as f64, &group_points).map(|(_, pt)| pt)
                    })
                    .collect();
                if !hotspot_pts.is_empty() {
                    plot_ui.points(
                        egui_plot::Points::new(PlotPoints::from(hotspot_pts))
                            .color(super::theme::ERROR)
                            .radius(4.0)
                            .name("gate trips"),
                    );
                }
            }

            // Only react when the pointer is actually over the plot rect AND
            // within the data X range. This prevents the playhead jumping when
            // the user moves the mouse past the left/right edges of the plot
            // (which still reports a valid pointer_coordinate well outside the
            // data bounds).
            let pointer_in_rect = plot_ui.response().hovered();
            let dragged = plot_ui.response().dragged();
            if dragged {
                *scrub_drag_active = true;
            }
            if (pointer_in_rect || dragged)
                && let Some(pointer) = plot_ui.pointer_coordinate()
                && pointer.x >= x_min
                && pointer.x <= x_max
            {
                *new_hovered = Some(pointer.x);
                // Click: hotspot-snap if the pointer is close to one,
                // otherwise jump to the position. Drag: pure positional
                // scrub (no hotspot snapping — feels janky on drag).
                if plot_ui.response().clicked() {
                    let tolerance = (x_span * 0.02).max(5.0);
                    let nearest_hotspot = hotspots.iter().min_by(|a, b| {
                        (a.global_move as f64 - pointer.x)
                            .abs()
                            .total_cmp(&(b.global_move as f64 - pointer.x).abs())
                    });
                    if let Some(hs) = nearest_hotspot
                        && (hs.global_move as f64 - pointer.x).abs() <= tolerance
                    {
                        *clicked_hotspot = Some((hs.index, hs.global_move));
                    } else if let Some((global_move, _)) =
                        nearest_in_groups(pointer.x, &group_points)
                    {
                        events.push(AppEvent::SimJumpToMove(global_move));
                    }
                } else if dragged
                    && let Some((global_move, _)) = nearest_in_groups(pointer.x, &group_points)
                {
                    events.push(AppEvent::SimJumpToMove(global_move));
                }
            }
        });
    response.response.on_hover_text(
        "Hover to read all five tracks at the same X. Click or drag to scrub. Scroll to zoom. Each toolpath renders as its own line segment.",
    );
}

/// Find the global-move sample (across all toolpath groups) whose X is
/// closest to the target. Returns `(global_move, [x, y])`.
fn nearest_in_groups(target_x: f64, group_points: &GroupPoints) -> Option<(usize, [f64; 2])> {
    group_points
        .iter()
        .flat_map(|(_, pts)| pts.iter())
        .min_by(|a, b| {
            (a.1[0] - target_x)
                .abs()
                .total_cmp(&(b.1[0] - target_x).abs())
        })
        .copied()
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
            .add(egui::Button::new("◄").min_size(btn_size))
            .on_hover_text("Step back (Left arrow)")
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
            .on_hover_text("Step forward (Right arrow)")
            .clicked()
        {
            events.push(AppEvent::SimStepForward);
        }

        // Pass-jump buttons removed — the span ribbon below the boundary
        // timeline replaces them with a clickable visual navigator.

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

            ui.separator();

            // Playback speed as a multiplier (×) where 1× = real-time
            // playback for *this* project. The baseline is derived from the
            // project's actual move rate (total_moves / total_time_s) so
            // wanaka's ~200 mv/s real-time and a small project's ~50 mv/s
            // real-time both map to "1× = real time".
            //
            // Internally `playback.speed` stays in moves-per-second.
            let real_time_mv_s = if total_time > 0.0 {
                (sim.total_moves() as f64 / total_time) as f32
            } else {
                100.0
            };
            let real_time_mv_s = real_time_mv_s.max(1.0);
            let mut multiplier = sim.playback.speed / real_time_mv_s;
            ui.label(
                egui::RichText::new("Speed:")
                    .small()
                    .color(egui::Color32::from_rgb(140, 140, 150)),
            )
            .on_hover_text(format!(
                "Playback speed multiplier. 1× = real-time playback for this project ({:.0} moves/sec). [ and ] keys to adjust.",
                real_time_mv_s
            ));
            let resp = ui.add(
                egui::Slider::new(&mut multiplier, 0.1..=1000.0)
                    .logarithmic(true)
                    .suffix("×")
                    .show_value(true),
            );
            if resp.changed() {
                sim.playback.speed = (multiplier * real_time_mv_s).max(1.0);
            }
        }
    });
}

/// Row 2: Custom-painted per-op timeline bar with collision markers and
/// optional semantic annotation band. Always project-wide — segments,
/// markers, and the playhead all live in the global move space across
/// every toolpath in the project, regardless of which TP is focused
/// in the panel below.
#[allow(clippy::too_many_arguments)]
fn draw_boundary_timeline(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    gui: &GuiState,
    max_feed: f64,
    load_report: &ToolLoadReport,
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

        // Inspector focus filters all markers on the boundary timeline. When
        // a TP is focused, only its markers are drawn; otherwise all are
        // shown. The boundary segments themselves (above) stay project-wide.
        let focused_id = sim.focused_toolpath();
        let in_focus = |move_idx: usize| -> bool {
            let Some(focus) = focused_id else {
                return true;
            };
            sim.boundaries()
                .iter()
                .find(|b| move_idx >= b.start_move && move_idx <= b.end_move)
                .map(|b| b.id == focus)
                .unwrap_or(false)
        };

        if let Some(ref report) = sim.checks.collision_report {
            let holder_color = egui::Color32::from_rgb(255, 50, 50);
            for col in &report.collisions {
                if !in_focus(col.move_idx) {
                    continue;
                }
                let x = rect.min.x + (col.move_idx as f32 / total_moves) * total_width;
                painter.line_segment(
                    [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
                    egui::Stroke::new(2.0, holder_color),
                );
            }
        }

        let rapid_color = egui::Color32::from_rgb(255, 160, 40);
        for &idx in &sim.checks.rapid_collision_move_indices {
            if !in_focus(idx) {
                continue;
            }
            let x = rect.min.x + (idx as f32 / total_moves) * total_width;
            painter.line_segment(
                [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
                egui::Stroke::new(1.5, rapid_color),
            );
        }

        draw_tool_load_timeline_markers(
            &painter,
            rect,
            total_moves,
            total_width,
            sim,
            load_report,
            focused_id,
        );

        // Hover tooltip: when the pointer is near a timeline marker, show
        // what the marker is. Uses a single tooltip per hover frame, picking
        // the closest visible marker within ~6 px.
        if response.hovered()
            && let Some(pos) = response.hover_pos()
            && let Some(tip) = nearest_marker_tooltip(
                pos.x,
                rect,
                total_moves,
                total_width,
                sim,
                load_report,
                focused_id,
            )
        {
            egui::show_tooltip_at_pointer(
                ui.ctx(),
                ui.layer_id(),
                egui::Id::new("sim_timeline_marker_tip"),
                |ui| {
                    ui.label(tip);
                },
            );
        }

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
        if response.dragged() {
            sim.playback.scrub_drag_active = true;
        }
        if (response.dragged() || response.clicked())
            && let Some(pos) = response.interact_pointer_pos()
        {
            if response.clicked()
                && let Some(target) = nearest_safety_marker_move(
                    pos.x,
                    rect,
                    total_moves,
                    total_width,
                    sim,
                    load_report,
                )
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

    // Span ribbon: subdivides the scope toolpath's segment of the X axis
    // into DepthPass blocks (and Region sub-blocks when a pass is selected).
    // Click → scrub to the span's start AND set the chip-row scope. This
    // replaces the «Z Z» pass-jump buttons in the transport row with a
    // visual, hover-aware navigator.
    draw_span_ribbon(ui, sim, gui, events);

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

/// Paint a 14px ribbon under the boundary timeline showing structural
/// span subdivisions for the scope toolpath. DepthPass spans render as
/// primary colored blocks when present; operations without DepthPass spans
/// render Region spans as the primary blocks instead. When a DepthPass is
/// selected, its Region children render as lighter sub-blocks. Hover shows
/// the span label; click sets the chip-row scope to that span and scrubs
/// playback to its start move.
fn draw_span_ribbon(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    gui: &GuiState,
    events: &mut Vec<AppEvent>,
) {
    use rs_cam_core::toolpath_spans::{SpanKind, SpanPayload};

    if sim.total_moves() == 0 || sim.boundaries().is_empty() {
        return;
    }

    let scope_tp = sim
        .debug
        .span_scope
        .toolpath_id
        .or_else(|| sim.current_boundary().map(|b| b.id));
    let Some(tp_id) = scope_tp else { return };

    // Find the boundary entry for this toolpath so we can convert its
    // toolpath-local move indices into the project-global X space.
    let Some(boundary) = sim.boundaries().iter().find(|b| b.id == tp_id).cloned() else {
        return;
    };

    let Some(rt) = gui.toolpath_rt.get(&tp_id.0) else {
        return;
    };
    let Some(result) = rt.result.as_ref() else {
        return;
    };
    if !result.spans_valid() {
        return;
    }
    let spans = result.spans();
    if spans.is_empty() {
        return;
    }

    let total_width = ui.available_width();
    let height = 18.0;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(total_width, height), egui::Sense::click());
    let painter = ui.painter_at(rect);

    // Background — dim track so DepthPass blocks have something to sit on
    // before they paint. Visible at the edges of the focused toolpath
    // (outside the boundary segment).
    painter.rect_filled(
        rect,
        2.0,
        egui::Color32::from_rgba_unmultiplied(20, 22, 30, 220),
    );

    let total_moves = sim.total_moves().max(1) as f32;
    let scope_span_id = sim.debug.span_scope.span_id;
    let global_x = |local_move: usize| -> f32 {
        let global = (boundary.start_move + local_move) as f32;
        rect.min.x + (global / total_moves) * total_width
    };

    // Compute the playhead's current span id (innermost). This drives the
    // "which block am I in?" highlight when nothing is locked, and gives
    // us a subtle accent when something IS locked but playback has moved
    // elsewhere.
    let playhead_span_id: Option<u32> = (|| {
        let global = sim.playback.current_move;
        if global < boundary.start_move || global >= boundary.end_move {
            return None;
        }
        let local = global - boundary.start_move;
        spans
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.is_boundary() && s.contains(local))
            .min_by_key(|(_, s)| s.move_count())
            .map(|(idx, _)| idx as u32)
    })();

    let has_depth_passes = spans
        .iter()
        .any(|span| !span.is_boundary() && matches!(span.kind, SpanKind::DepthPass));
    let primary_kind = if has_depth_passes {
        SpanKind::DepthPass
    } else {
        SpanKind::Region
    };

    // Selected DepthPass id — when scope is locked to a Region, its parent
    // pass also gets the Region sub-block tier rendered below. Toolpaths
    // without DepthPass spans draw Region spans directly as primary blocks.
    let selected_dp_id: Option<u32> = if has_depth_passes {
        scope_span_id.and_then(|sid| {
            let target = spans.get(sid as usize)?;
            if matches!(target.kind, SpanKind::DepthPass) {
                return Some(sid);
            }
            spans
                .iter()
                .enumerate()
                .find(|(_, s)| {
                    matches!(s.kind, SpanKind::DepthPass)
                        && s.start_move <= target.start_move
                        && s.end_move >= target.end_move
                })
                .map(|(idx, _)| idx as u32)
        })
    } else {
        None
    };

    // Paint primary blocks with strong alternating contrast so every section
    // is visible at rest. Locked = bright cyan; playhead-current = gentle
    // highlight; otherwise alternating blue-grey.
    const COLOR_EVEN: egui::Color32 = egui::Color32::from_rgb(120, 145, 185);
    const COLOR_ODD: egui::Color32 = egui::Color32::from_rgb(70, 100, 145);
    const COLOR_LOCKED: egui::Color32 = egui::Color32::from_rgb(220, 240, 255);
    const COLOR_PLAYHEAD: egui::Color32 = egui::Color32::from_rgb(160, 200, 235);
    const COLOR_HOVER_OUTLINE: egui::Color32 = egui::Color32::from_rgb(255, 255, 255);

    let mut primary_index_seq = 0u32;
    let mut click_target: Option<(u32, usize)> = None;
    let mut hover_label: Option<String> = None;
    let pointer_x = response.hover_pos().map(|p| p.x);

    for (sid, span) in spans
        .iter()
        .enumerate()
        .filter(|(_, s)| !s.is_boundary() && s.kind == primary_kind)
    {
        let sid_u32 = sid as u32;
        let x_start = global_x(span.start_move);
        let x_end = global_x(span.end_move);
        let primary_idx = match &span.payload {
            Some(SpanPayload::DepthPass { pass_index, .. }) => *pass_index,
            Some(SpanPayload::Region { region_id }) if !has_depth_passes => *region_id,
            _ => {
                let n = primary_index_seq;
                primary_index_seq += 1;
                n
            }
        };

        let is_locked = Some(sid_u32) == scope_span_id || Some(sid_u32) == selected_dp_id;
        let is_playhead = !is_locked && Some(sid_u32) == playhead_span_id;
        let hovered = pointer_x.is_some_and(|px| px >= x_start && px <= x_end);

        let color = if is_locked {
            COLOR_LOCKED
        } else if is_playhead {
            COLOR_PLAYHEAD
        } else if primary_idx % 2 == 0 {
            COLOR_EVEN
        } else {
            COLOR_ODD
        };

        let block = egui::Rect::from_min_max(
            egui::pos2(x_start, rect.min.y),
            egui::pos2(x_end, rect.max.y),
        );
        painter.rect_filled(block, 0.0, color);

        // Hover outline — thin white ring around the block under the cursor
        // so the user has clear visual feedback that the block is clickable.
        if hovered {
            painter.rect_stroke(block, 0.0, egui::Stroke::new(1.5, COLOR_HOVER_OUTLINE));
        }

        // Thin separator on the right edge so consecutive passes don't blur.
        painter.line_segment(
            [egui::pos2(x_end, rect.min.y), egui::pos2(x_end, rect.max.y)],
            egui::Stroke::new(0.5, egui::Color32::from_rgb(15, 17, 24)),
        );

        if hovered {
            hover_label = Some(format!(
                "{} · moves {}–{}",
                ribbon_span_label(span, primary_idx),
                span.start_move,
                span.end_move
            ));
            if response.clicked() {
                click_target = Some((sid_u32, boundary.start_move + span.start_move));
            }
        }
    }

    // Region sub-blocks: only the children of the selected DepthPass, painted
    // as a lighter overlay in the bottom half of the ribbon.
    if let Some(dp_id) = selected_dp_id
        && let Some(dp_span) = spans.get(dp_id as usize)
    {
        let region_y = rect.min.y + height * 0.55;
        for (sid, span) in spans.iter().enumerate().filter(|(_, s)| {
            matches!(s.kind, SpanKind::Region)
                && s.start_move >= dp_span.start_move
                && s.end_move <= dp_span.end_move
        }) {
            let sid_u32 = sid as u32;
            let x_start = global_x(span.start_move);
            let x_end = global_x(span.end_move);
            let color = if Some(sid_u32) == scope_span_id {
                egui::Color32::from_rgb(240, 200, 120)
            } else {
                egui::Color32::from_rgba_unmultiplied(220, 180, 110, 180)
            };
            let block = egui::Rect::from_min_max(
                egui::pos2(x_start, region_y),
                egui::pos2(x_end, rect.max.y),
            );
            painter.rect_filled(block, 0.0, color);

            if let Some(px) = pointer_x
                && px >= x_start
                && px <= x_end
                && pointer_x.is_some_and(|p| {
                    let row_pos = response.hover_pos().map(|h| h.y).unwrap_or(0.0);
                    let _ = p;
                    row_pos >= region_y
                })
            {
                let region_id = match &span.payload {
                    Some(SpanPayload::Region { region_id }) => *region_id,
                    _ => sid_u32,
                };
                hover_label = Some(format!(
                    "Region {region_id} · moves {}–{}",
                    span.start_move, span.end_move
                ));
                if response.clicked() {
                    click_target = Some((sid_u32, boundary.start_move + span.start_move));
                }
            }
        }
    }

    // Playhead overlay so the user can see how its position relates to the
    // active span.
    let playhead_x = rect.min.x + (sim.playback.current_move as f32 / total_moves) * total_width;
    if playhead_x >= rect.min.x && playhead_x <= rect.max.x {
        painter.line_segment(
            [
                egui::pos2(playhead_x, rect.min.y),
                egui::pos2(playhead_x, rect.max.y),
            ],
            egui::Stroke::new(1.5, egui::Color32::WHITE),
        );
    }

    if let Some(tip) = hover_label {
        egui::show_tooltip_at_pointer(
            ui.ctx(),
            ui.layer_id(),
            egui::Id::new("sim_span_ribbon_tip"),
            |ui| {
                ui.label(tip);
            },
        );
    }

    if let Some((sid, jump_move)) = click_target {
        sim.debug.span_scope.toolpath_id = Some(tp_id);
        // Toggle: clicking the already-selected span clears it back to the
        // toolpath-wide scope; clicking a fresh span selects it.
        sim.debug.span_scope.span_id = if sim.debug.span_scope.span_id == Some(sid) {
            None
        } else {
            Some(sid)
        };
        events.push(AppEvent::SimJumpToMove(jump_move));
    }
}

fn ribbon_span_label(span: &rs_cam_core::toolpath_spans::Span, fallback_index: u32) -> String {
    use rs_cam_core::toolpath_spans::{SpanKind, SpanPayload};

    if !span.label.is_empty() {
        return span.label.clone().into_owned();
    }
    match (&span.kind, &span.payload) {
        (
            SpanKind::DepthPass,
            Some(SpanPayload::DepthPass {
                z_level,
                pass_index,
            }),
        ) => format!("DepthPass {pass_index} · z={z_level:.2}"),
        (SpanKind::Region, Some(SpanPayload::Region { region_id })) => {
            format!("Region {region_id}")
        }
        (SpanKind::DepthPass, _) => format!("DepthPass {fallback_index}"),
        (SpanKind::Region, _) => format!("Region {fallback_index}"),
        _ => format!("{:?} {fallback_index}", span.kind),
    }
}

fn nearest_safety_marker_move(
    pointer_x: f32,
    rect: egui::Rect,
    total_moves: f32,
    total_width: f32,
    sim: &SimulationState,
    load_report: &ToolLoadReport,
) -> Option<usize> {
    let rapid = sim.checks.rapid_collision_move_indices.iter().copied();
    let tool_load = tool_load_marker_moves(sim, load_report);
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

fn tool_load_marker_moves(sim: &SimulationState, load_report: &ToolLoadReport) -> Vec<usize> {
    let Some(trace) = sim
        .results
        .as_ref()
        .and_then(|results| results.cut_trace.as_deref())
    else {
        return Vec::new();
    };
    load_report
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
    load_report: &ToolLoadReport,
    focused_id: Option<crate::state::toolpath::ToolpathId>,
) {
    let Some(trace) = sim
        .results
        .as_ref()
        .and_then(|results| results.cut_trace.as_deref())
    else {
        return;
    };

    for verdict in &load_report.per_toolpath {
        if let Some(focus) = focused_id
            && verdict.toolpath_id != focus.0
        {
            continue;
        }
        if let Some(global_move) = first_exceeded_tool_load_move(sim, trace, verdict) {
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

/// Find the marker closest to the pointer (within ~6 px) and compose a
/// human-readable tooltip. Honors `focused_id` so the tooltip only fires
/// for markers that are actually drawn under the current focus filter.
fn nearest_marker_tooltip(
    pointer_x: f32,
    rect: egui::Rect,
    total_moves: f32,
    total_width: f32,
    sim: &SimulationState,
    load_report: &ToolLoadReport,
    focused_id: Option<crate::state::toolpath::ToolpathId>,
) -> Option<String> {
    const HOVER_PX: f32 = 6.0;
    let in_focus = |move_idx: usize| -> bool {
        let Some(focus) = focused_id else {
            return true;
        };
        sim.boundaries()
            .iter()
            .find(|b| move_idx >= b.start_move && move_idx <= b.end_move)
            .map(|b| b.id == focus)
            .unwrap_or(false)
    };
    let tp_name_for_move = |move_idx: usize| -> String {
        sim.boundaries()
            .iter()
            .find(|b| move_idx >= b.start_move && move_idx <= b.end_move)
            .map_or_else(|| "(no toolpath)".to_owned(), |b| b.name.clone())
    };
    let x_for_move =
        |move_idx: usize| -> f32 { rect.min.x + (move_idx as f32 / total_moves) * total_width };

    let mut best: Option<(f32, String)> = None;
    let mut consider = |move_idx: usize, label: String| {
        let dx = (x_for_move(move_idx) - pointer_x).abs();
        if dx <= HOVER_PX && best.as_ref().is_none_or(|(prev, _)| dx < *prev) {
            best = Some((dx, label));
        }
    };

    // Holder collisions
    if let Some(report) = sim.checks.collision_report.as_ref() {
        for col in &report.collisions {
            if !in_focus(col.move_idx) {
                continue;
            }
            consider(
                col.move_idx,
                format!(
                    "{}: holder collision at move {} — click to navigate",
                    tp_name_for_move(col.move_idx),
                    col.move_idx
                ),
            );
        }
    }

    // Rapid collisions
    for &idx in &sim.checks.rapid_collision_move_indices {
        if !in_focus(idx) {
            continue;
        }
        consider(
            idx,
            format!(
                "{}: rapid collision at move {} — click to navigate",
                tp_name_for_move(idx),
                idx
            ),
        );
    }

    // Tool-load exceedance markers
    let sim_trace = sim.results.as_ref().and_then(|r| r.cut_trace.as_deref());
    if let Some(trace) = sim_trace {
        for verdict in &load_report.per_toolpath {
            if let Some(focus) = focused_id
                && verdict.toolpath_id != focus.0
            {
                continue;
            }
            if let Some(move_idx) = first_exceeded_tool_load_move(sim, trace, verdict) {
                use rs_cam_core::tool_load::verdict::{
                    ChipSide, ChiploadVerdict, DeflectionVerdict, PowerVerdict,
                };
                let reason = if let ChiploadVerdict::Exceeds {
                    side, triggering, ..
                } = &verdict.chipload
                {
                    let label = match side {
                        ChipSide::Low => "BurnRisk",
                        ChipSide::High => "BreakageRisk",
                    };
                    format!(
                        "chipload {label} peak {:.4}",
                        triggering.observed_mm_per_tooth
                    )
                } else if let PowerVerdict::Exceeds { peak_kw, .. } = &verdict.power {
                    format!("power SpindlePowerExceeded peak {peak_kw:.3} kW")
                } else if let DeflectionVerdict::Exceeds { peak_mm, .. } = &verdict.deflection {
                    format!("deflection LongToolStiffnessUnsafe peak {peak_mm:.4} mm")
                } else {
                    "tool-load exceeds".to_owned()
                };
                consider(
                    move_idx,
                    format!(
                        "{}: {} at move {} — click to navigate",
                        tp_name_for_move(move_idx),
                        reason,
                        move_idx
                    ),
                );
            }
        }
    }

    best.map(|(_, label)| label)
}

fn first_exceeded_tool_load_move(
    sim: &SimulationState,
    trace: &rs_cam_core::simulation_cut::SimulationCutTrace,
    verdict: &rs_cam_core::tool_load::ToolpathLoadVerdict,
) -> Option<usize> {
    use rs_cam_core::tool_load::verdict::{ChiploadVerdict, DeflectionVerdict, PowerVerdict};
    let sample_index = if let ChiploadVerdict::Exceeds { triggering, .. } = &verdict.chipload {
        triggering.evidence.sample_range.start
    } else if let PowerVerdict::Exceeds { evidence, .. } = &verdict.power {
        evidence.sample_range.start
    } else if let DeflectionVerdict::Exceeds { evidence, .. } = &verdict.deflection {
        evidence.sample_range.start
    } else {
        return None;
    };
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
