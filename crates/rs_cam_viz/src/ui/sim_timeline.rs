use super::AppEvent;
use super::components::{CountPill, FreshnessGate};
use super::readiness::{self, CycleTimeBasisExt};
use super::sim_debug::semantic_kind_color;
use super::sim_diagnostics::CutMetricSpec;
use crate::render::toolpath_render::palette_color;
use crate::state::freshness::simulation_freshness;
use crate::state::runtime::GuiState;
use crate::state::simulation::{ActiveSemanticItem, CUT_METRIC_ORDER, SimulationState};
use crate::ui_command::{NoArgs, SimJumpToMoveArgs, UiCommand};
use egui_plot::{Line, Plot, PlotPoints, Polygon};
use rs_cam_core::session::ProjectSession;
use rs_cam_core::stock::simulation_cut::SimulationCutSample;
use rs_cam_core::tool_load::{DistributionMetric, DistributionOutcome, ToolLoadReport};

/// Per-toolpath line in the signal plot: a colour plus a sequence of
/// `(global_move_index, [x, y])` points decimated for the current X span.
type GroupPoints = Vec<(egui::Color32, Vec<(usize, [f64; 2])>)>;

/// One unbanded signal track: label and value extractor over
/// `SimulationCutSample`. An unbanded track has no limit lines.
type SignalTrack = (&'static str, fn(&SimulationCutSample) -> Option<f64>);

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
                    .color(crate::ui::tokens::CAUTION),
            );
        });
        ui.add_space(2.0);
    }
    // W0.5/TIM-009 — the spine + strips keep rendering the last trace, so
    // flag staleness here too; otherwise the bottom panel's concrete metrics
    // read as fresh after an edit while only the left/right panels say stale.
    // W4: one function, and it carries the "nothing to show" case, so the
    // second `has_results()` read goes.
    if simulation_freshness(session, sim).is_stale() {
        FreshnessGate::banner(ui);
        ui.add_space(2.0);
    }
    sim.sync_debug_state(gui);
    let active_semantic = sim.active_semantic_item(gui);
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

    // DC6 — ONE bar. The transport controls and the project chips used to sit
    // as two loose rows at the bottom left, beside nothing they drive. They
    // are one frame now, at the head of the panel that holds the timeline
    // they scrub and directly under the viewport they play. Every transport
    // command keeps its route; only the two containers became one.
    egui::Frame::default()
        .fill(crate::ui::tokens::SURFACE_BASE)
        .corner_radius(crate::ui::tokens::RADIUS_SM)
        .inner_margin(egui::Margin::symmetric(
            crate::ui::tokens::SPACE_3 as i8,
            crate::ui::tokens::SPACE_2 as i8,
        ))
        .show(ui, |ui| {
            ui.set_min_height(crate::ui::tokens::ROW_ACTION);
            ui.horizontal_wrapped(|ui| {
                draw_transport_and_scrubber(ui, sim, session, gui, events);
                ui.separator();
                draw_verdict_hud(ui, sim, gui, max_feed, &load_report, events);
            });
        });
    ui.add_space(crate::ui::tokens::SPACE_2);

    // Boundary timeline always shows the whole project — user wants the
    // full picture (Pin Drill, Back Rough, ...) at a glance regardless
    // of which TP is focused below. The time-series drawer scopes to
    // the focused TP independently. The two widgets use different X
    // coordinate spaces; markers on each are accurate within their own
    // widget but won't visually align with the other.
    draw_boundary_timeline(
        ui,
        sim,
        gui,
        &load_report,
        &current_boundary,
        &active_semantic,
        events,
    );
    draw_time_series(ui, sim, session, gui, &load_report, events);
}

fn draw_verdict_hud(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    gui: &GuiState,
    max_feed: f64,
    load_report: &ToolLoadReport,
    events: &mut Vec<AppEvent>,
) {
    // Curated count only (density pass V1): the raw issues() length is
    // dominated by air-cut / low-engagement runs (tens of thousands on a
    // real job) — a headline "issues 46751" pill reads as catastrophe.
    // Hotspots are the one issue kind that is both curated and not already
    // pilled (collisions have their own pill).
    //
    // D10 (census §3.5): this comment used to say "per-sample emission
    // noise". It is not per-sample — `cut_trace.issues` has been run-length
    // coalesced since April 2026, and on the census fixture the two
    // populations differ by 43x (35,287 flagged samples became 821
    // segments). Sizing a fix off the wrong population is exactly what the
    // census set out to prevent. What actually drives the count is
    // TRANSITION DENSITY in ordinary cutting: every time the cutter leaves
    // and re-enters material a run breaks and a new segment opens. An
    // all-air toolpath is the CHEAP case (one long run); a normal pocket is
    // the expensive one.
    let hotspot_count = sim.issue_hotspot_count(gui, max_feed);

    // TIM-005 — the pills *are* the navigation, not a sign pointing at the
    // markers below. Pre-compute the first offending move for the exceeds and
    // collisions pills so a click seeks the playhead there. Prose
    // "click the red lines…" instructions are retired; hover keeps only a
    // short factual definition.
    //
    // These clicks used to ALSO write `SimulationState::analytics_tab`, as a
    // "drill into the Safety tab" half. Nothing ever read that field: the
    // diagnostics panel has no tabs — it is collapsing headers plus a
    // focused-issue / focused-hotspot card — so there was no section chooser
    // to point at, and six clicks carried a promise the app could not keep
    // (audit §4.4, fix §6.12). The field is deleted; the seek is what these
    // clicks always really did.
    let first_exceed_move = tool_load_marker_moves(sim, load_report).into_iter().min();
    let first_collision_move = std::iter::empty::<usize>()
        .chain(
            sim.checks
                .collision_report
                .as_ref()
                .into_iter()
                .flat_map(|r| r.collisions.iter().map(|c| c.move_index)),
        )
        .chain(sim.checks.rapid_collision_move_indices.iter().copied())
        .min();

    // W0.4 — read the same per-toolpath rollup the Inspector overview uses
    // (`ToolLoadReport::summary()`) so the two project rollups can't print
    // different numbers for the same load concept. The HUD previously folded
    // per-(toolpath × gate) criteria, which is irreconcilable with the
    // Inspector's per-toolpath counts and mislabeled them as "Toolpaths".
    let summary = load_report.summary(|_| None);
    let (ok, bad, unmodeled, total_tp) = (
        summary.within,
        summary.exceeds,
        summary.fully_unmodeled,
        summary.total_toolpaths,
    );
    let collision_count = sim.checks.total_collision_count();
    let trace_count = gui
        .toolpath_rt
        .values()
        .filter(|rt| rt.debug_trace.is_some() || rt.semantic_trace.is_some())
        .count();

    // DC6 — the chips carry no frame of their own any more. The playback bar
    // in `draw` is the one container, so a box inside a box no longer reads as
    // a second, separate strip.
    ui.horizontal_wrapped(|ui| {
        // Load buckets are verdicts (pass/fail of a modeled limit);
        // collisions/hotspots/traces are observations (tallies). One
        // CountPill renderer, the same summary() producer the Inspector
        // reads — the two rollups cannot diverge (W0.4 + P4-001/002).
        ui.add(
            CountPill::verdict("\u{2713} load", ok)
                .denom(total_tp)
                .color(crate::ui::tokens::OK)
                .hover("Toolpaths within modeled load limits (of total modeled)."),
        );
        // Exceeds is a navigation control when any TP exceeds: click
        // seeks to the first exceedance marker (TIM-005).
        let exceeds_pill = CountPill::verdict("\u{2715} exceeds", bad)
            .denom(total_tp)
            .color(crate::ui::tokens::DANGER)
            .hover("Toolpaths exceeding a modeled load limit.");
        if let Some(move_idx) = first_exceed_move.filter(|_| bad > 0) {
            if ui.add(exceeds_pill.actionable()).clicked() {
                events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
                    move_index: move_idx,
                })));
            }
        } else {
            // Self-hide at zero like the Inspector copy (V4) — the
            // 2026-06-12 sweep caught "✗ exceeds 0/7" still shipping
            // in the HUD while the Inspector hid it.
            ui.add(exceeds_pill.hide_when_zero());
        }
        ui.add(
            CountPill::verdict("\u{26A0} unmodeled", unmodeled)
                .denom(total_tp)
                // UP4: an ABSTENTION, not a caution — the gate
                // could not model these toolpaths, so there is no
                // verdict to show (`DESIGN_SPEC.md` §2.6).
                .color(crate::ui::tokens::UNKNOWN)
                .hide_when_zero()
                .hover(
                    "Toolpaths the gate could not model (drill cycles, no vendor data, \
                     etc.).",
                ),
        );
        // Collisions is likewise a navigation control: click seeks
        // to the first collision (TIM-005).
        // Zero-count chips self-hide (density pass V4) — at zero the
        // within-pill plus the readiness checks carry the all-clear.
        let collisions_pill = CountPill::observation("collisions", collision_count)
            .color(crate::ui::tokens::DANGER)
            .hide_when_zero()
            .hover("Rapid/holder collisions detected during simulation.");
        if let Some(move_idx) = first_collision_move.filter(|_| collision_count > 0) {
            if ui.add(collisions_pill.actionable()).clicked() {
                events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
                    move_index: move_idx,
                })));
            }
        } else {
            ui.add(collisions_pill);
        }
        ui.add(
            CountPill::observation("hotspots", hotspot_count)
                .color(crate::ui::tokens::CAUTION)
                .hide_when_zero()
                .hover(
                    "Sustained high-load clusters — triage in the Inspector's \
                     Top hotspots list.",
                ),
        );
        // Generator traces are debug-grade provenance: only pill them
        // when at least one exists (density pass 2026-06-11).
        if trace_count > 0 {
            ui.add(
                CountPill::observation("traces", trace_count)
                    .color(crate::ui::tokens::INFO)
                    .hover("Generator traces recorded for inspection."),
            );
        }
    });
}

/// Package E (PLAN §3.3): the time-series drawer.
///
/// The drawer is closed by default. The Inspector's "Cut metrics" section
/// owns the one toggle, `SimulationState::time_series_open`. Closed, this
/// function draws nothing and the bottom panel holds the transport bar and
/// the boundary timeline only. There is no placeholder here: the right
/// section states `NotMeasured` when no metric exists.
///
/// Open, the drawer draws every track in one scroll area:
///
/// - One banded track per cut-metric card of the focused toolpath. Its
///   points are the gate population the card bins
///   (`CutMetricCard::series`), in the card's display unit, with the
///   gate's own limit lines. The drawer and the card show one number.
/// - Three unbanded tracks from the raw samples: arc-mean chip thickness,
///   MRR and commanded feed. Arc-mean chip thickness has no band on purpose
///   (Checkpoint H3).
///
/// The normalised "advance/tooth vs band max" summary track and the
/// "Signal graphs" disclosure are gone. The chipload card carries the
/// summary.
fn draw_time_series(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    session: &ProjectSession,
    gui: &GuiState,
    load_report: &ToolLoadReport,
    events: &mut Vec<AppEvent>,
) {
    if !sim.time_series_open {
        // No track is under the pointer while the drawer is closed.
        sim.hovered_x = None;
        return;
    }
    // The card footer asks for one track. The request lives for one frame.
    let scroll_to = sim.time_series_scroll_to.take();
    // Acquire the trace as an Arc so we can keep an immutable handle for
    // reads while still calling `&mut sim` methods (e.g. ensure_built).
    let trace_arc = sim
        .results
        .as_ref()
        .and_then(|r| r.cut_trace.as_ref())
        .map(std::sync::Arc::clone);
    let Some(trace_arc) = trace_arc else {
        return;
    };
    sim.debug.span_aggregates.ensure_built(&trace_arc);
    let trace = trace_arc.as_ref();
    let total_moves = sim.total_moves();
    if total_moves == 0 {
        return;
    }
    // TIM-009 — whole-drawer stale skin. When the trace no longer matches
    // the current params, desaturate the track colours and drop the
    // gate-trip drill (its dots point at moves that may no longer exist).
    let stale = simulation_freshness(session, sim).is_stale();

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
            let color = crate::ui::tokens::from_linear_rgb(pc);
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

    // Playback-derived focus filters the per-TP groups so the tracks show
    // only the playing toolpath. The boundary timeline above stays
    // project-wide regardless.
    let focused_id = sim.focused_toolpath();
    if let Some(id) = focused_id {
        groups.retain(|g| g.toolpath_id == id);
    }
    if groups.iter().all(|g| g.samples.is_empty()) {
        return;
    }
    // Stale skin: grey the per-toolpath line colours so the drawer reads as
    // "last run, not current" (TIM-009).
    if stale {
        for g in &mut groups {
            g.color = desaturate(g.color);
        }
    }

    ui.add_space(4.0);
    let active_x = Some(sim.playback.current_move as f64);

    // F6.1 — timeline point markers are reserved for gate trips only: one
    // dot per Exceeds verdict at the gate's actual worst-sample move. Reuses
    // the timeline's memoed `load_report` (passed in).
    let mut hotspots: Vec<HotspotMarker> = Vec::new();
    for verdict in &load_report.per_toolpath {
        if let Some(focus) = focused_id
            && verdict.toolpath_id != focus
        {
            continue;
        }
        if let Some(global_move) = first_exceeded_tool_load_move(sim, trace, verdict) {
            hotspots.push(HotspotMarker { global_move });
        }
    }
    // Stale data → drop the gate-trip dots so they can't be clicked into a
    // move that no longer exists (TIM-009).
    if stale {
        hotspots.clear();
    }

    let display_x = sim.hovered_x;
    let mut new_hovered: Option<f64> = None;
    let mut clicked_hotspot: Option<usize> = None;
    let mut signal_drag_active = false;
    let total_moves_f = total_moves as f64;

    // X-axis range for the tracks. When a TP is focused, zoom the X axis to
    // just that TP's move range so the data fills the plot width. The
    // boundary timeline above stays whole-project regardless.
    let focused_start = focused_id
        .and_then(|id| sim.boundaries().iter().find(|b| b.id == id))
        .map(|b| (b.start_move, b.end_move));
    let x_range: (f64, f64) = focused_start
        .map(|(start, end)| (start as f64, end as f64))
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
            let rt = gui.toolpath_rt.get(&id)?;
            let result = rt.result.as_ref()?;
            if !result.spans_valid() {
                return None;
            }
            let spans = result.spans();
            let scope_span_id = sim.debug.span_scope.span_id;
            let bands: Vec<(f64, f64, bool)> = spans
                .iter()
                .enumerate()
                .filter(|(_, s)| {
                    matches!(
                        s.kind,
                        rs_cam_core::trace::toolpath_spans::SpanKind::DepthPass
                    )
                })
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

    // The banded tracks: the cut-metric set of the focused toolpath, from
    // the same memo the Inspector cards read.
    let metric_set = focused_id.map(|id| sim.cached_cut_metrics(session, gui.edit_counter, id));
    let mut banded: Vec<BandedTrack> = Vec::new();
    if let (Some(set), Some((start_move, _))) = (metric_set.as_ref(), focused_start) {
        for card in &set.cards {
            let DistributionOutcome::Measured(distribution) = &card.outcome else {
                continue;
            };
            if card.series.is_empty() {
                continue;
            }
            let spec = CutMetricSpec::of(card.metric);
            let points: Vec<(usize, [f64; 2])> = card
                .series
                .iter()
                .map(|&(local_move, value)| {
                    let global_move = start_move + local_move;
                    (global_move, [global_move as f64, value * spec.scale])
                })
                .collect();
            let advisory = distribution
                .bound_source
                .as_ref()
                .is_some_and(|source| !source.gates_export());
            banded.push(BandedTrack {
                metric: card.metric,
                label: format!("{} ({})", spec.title, spec.unit),
                points: decimate_max(points, SIGNAL_MAX_POINTS),
                bounds: TrackBounds {
                    floor: distribution.histogram.floor.map(|v| v * spec.scale),
                    ceiling: distribution.histogram.ceiling.map(|v| v * spec.scale),
                    advisory,
                },
            });
        }
    }

    // The tracks keep the card order: `CUT_METRIC_ORDER`, which is the
    // `criteria()` order with engagement last.
    banded.sort_by_key(|track| {
        CUT_METRIC_ORDER
            .iter()
            .position(|metric| *metric == track.metric)
            .unwrap_or(usize::MAX)
    });

    // UP4: the tracks are SERIES in a chart, a category. They walk
    // `SPAN_SCALE`, not a hand-assembled colour wheel.
    let raw_tracks: [SignalTrack; 3] = [
        (
            // Checkpoint H3 (2026-08-08): this track keeps the arc-mean
            // chip thickness — a real engagement/force signal — but is
            // **unbanded**, and says which quantity it is. The banded
            // comparison lives on the chipload track, in the band's unit.
            "arc-mean chip thickness (mm)",
            // Filter air-cut samples (radial_engagement < 0.02). Same
            // threshold the gate's verdict uses. Air cuts have no chip.
            |s| {
                if s.engagement.radial_woc_fraction < 0.02 {
                    None
                } else {
                    s.effective_chip_thickness_mm
                }
            },
        ),
        ("MRR (mm\u{00b3}/s)", |s| {
            (s.mrr_mm3_s > 0.0).then_some(s.mrr_mm3_s)
        }),
        ("commanded feed (mm/min)", |s| Some(s.feed_rate_mm_min)),
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
                    .color(crate::ui::tokens::TEXT_MUTED),
            );
            ui.label(egui::RichText::new(&focus_name).small().strong());
        });
    }

    let mut ctx = TrackContext {
        active_x,
        display_x,
        new_hovered: &mut new_hovered,
        hotspots: &hotspots,
        x_range,
        pass_bands: &pass_bands,
        clicked_hotspot: &mut clicked_hotspot,
        scrub_drag_active: &mut signal_drag_active,
        events: &mut *events,
    };
    // One scroll area for every track. Each track is 90 px with vertical
    // separation, so the operator can read two or three at once and scroll
    // to the rest without losing the X-axis lock.
    egui::ScrollArea::vertical()
        .id_salt("signal_spine_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (index, track) in banded.iter().enumerate() {
                let colour = track_colour(index);
                let colour = if stale { desaturate(colour) } else { colour };
                let group_points: GroupPoints = vec![(colour, track.points.clone())];
                draw_signal_track(
                    ui,
                    &track.label,
                    &group_points,
                    colour,
                    track.bounds,
                    scroll_to == Some(track.metric),
                    &mut ctx,
                );
                ui.add_space(8.0);
            }
            for (offset, (label, value_fn)) in raw_tracks.into_iter().enumerate() {
                let colour = track_colour(banded.len() + offset);
                let colour = if stale { desaturate(colour) } else { colour };
                let group_points = sample_points(&groups, value_fn);
                draw_signal_track(
                    ui,
                    label,
                    &group_points,
                    colour,
                    TrackBounds::default(),
                    false,
                    &mut ctx,
                );
                ui.add_space(8.0);
            }
        });

    if signal_drag_active {
        sim.playback.scrub_drag_active = true;
    }
    sim.hovered_x = new_hovered;
    if let Some(global_move) = clicked_hotspot {
        // Gate-trip dot drill (TIM-010): seek to the offending move. The
        // per-toolpath gate detail is in the diagnostics panel's own
        // sections, which no click selects — see the `analytics_tab` note at
        // the head of this file's HUD.
        events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
            move_index: global_move,
        })));
    }
}

/// One banded track: a cut-metric card's population, in its display unit.
struct BandedTrack {
    metric: DistributionMetric,
    label: String,
    points: Vec<(usize, [f64; 2])>,
    bounds: TrackBounds,
}

/// The limit lines of one track, in the track's own unit. A line is drawn
/// only for a bound the gate has. No zone is shaded (PLAN §7 Q1).
#[derive(Clone, Copy, Debug, Default)]
struct TrackBounds {
    floor: Option<f64>,
    ceiling: Option<f64>,
    /// Draw the lines dashed: the bound is a rule of thumb.
    advisory: bool,
}

/// The state every track of one drawer frame shares.
struct TrackContext<'a> {
    active_x: Option<f64>,
    display_x: Option<f64>,
    new_hovered: &'a mut Option<f64>,
    hotspots: &'a [HotspotMarker],
    x_range: (f64, f64),
    pass_bands: &'a [(f64, f64, bool)],
    clicked_hotspot: &'a mut Option<usize>,
    scrub_drag_active: &'a mut bool,
    events: &'a mut Vec<AppEvent>,
}

/// The series colour of track `index`, from the `SPAN_SCALE` token ramp.
fn track_colour(index: usize) -> egui::Color32 {
    let scale = &crate::ui::tokens::SPAN_SCALE;
    scale
        .get(index % scale.len())
        .copied()
        .unwrap_or(crate::ui::tokens::TEXT_MUTED)
}

struct ToolpathGroup<'a> {
    toolpath_id: crate::state::toolpath::ToolpathId,
    start_move: usize,
    color: egui::Color32,
    samples: Vec<&'a SimulationCutSample>,
}

/// A clickable tool-load gate-trip dot on the signal tracks. Carries only
/// the offending move — clicking jumps there.
///
/// TIM-010: this previously held an `index` field overloaded with a
/// `10_000+i` synthetic value, then tried `trace.hotspots.get(index)` on
/// click — always None, so the "drill into hotspot" half was dead. These
/// markers are gate trips, not engagement hotspots; there is no trace
/// hotspot to focus, so the jump *is* the drill.
#[derive(Clone, Copy)]
struct HotspotMarker {
    global_move: usize,
}

const SIGNAL_MAX_POINTS: usize = 1600;

/// Build per-toolpath point lists in global-move space, decimating each
/// independently so the global cap applies fairly across toolpaths.
fn sample_points(
    groups: &[ToolpathGroup<'_>],
    value_fn: impl Fn(&SimulationCutSample) -> Option<f64>,
) -> GroupPoints {
    let per_group_cap = (SIGNAL_MAX_POINTS / groups.len().max(1)).max(64);
    groups
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
            Some((group.color, decimate_max(pts, per_group_cap)))
        })
        .collect()
}

/// Decimate by **max-per-bucket** rather than stride sampling. Stride
/// sampling drops single-sample spikes (the full-slot peaks at every region
/// entry), which makes the gate's reported peak invisible on the graph.
/// Max-per-bucket keeps the worst-case sample per X-bucket. The result
/// holds at most `cap` points.
fn decimate_max(pts: Vec<(usize, [f64; 2])>, cap: usize) -> Vec<(usize, [f64; 2])> {
    if pts.len() <= cap.max(1) {
        return pts;
    }
    let bucket_size = pts.len().div_ceil(cap.max(1));
    pts.chunks(bucket_size)
        .filter_map(|chunk| {
            chunk
                .iter()
                .max_by(|a, b| a.1[1].total_cmp(&b.1[1]))
                .copied()
        })
        .collect()
}

/// Draw one track: its label, then the plot. `scroll_here` scrolls the
/// drawer so the label is at the top.
fn draw_signal_track(
    ui: &mut egui::Ui,
    label: &str,
    group_points: &GroupPoints,
    color: egui::Color32,
    bounds: TrackBounds,
    scroll_here: bool,
    ctx: &mut TrackContext<'_>,
) {
    if group_points.is_empty() {
        return;
    }
    let (x_min, x_max) = ctx.x_range;
    let x_span = (x_max - x_min).max(1.0);
    let values = || {
        group_points
            .iter()
            .flat_map(|(_, pts)| pts.iter().map(|(_, p)| p[1]))
            .chain(bounds.floor)
            .chain(bounds.ceiling)
    };
    let min_y = values().fold(f64::INFINITY, f64::min);
    let max_y = values().fold(f64::NEG_INFINITY, f64::max);

    // Track label above the plot — the embedded plot legend isn't very
    // visible at this size, and a leading label is more scannable when
    // tracks are stacked vertically in a scroll area.
    let label_response = ui.label(egui::RichText::new(label).small().strong().color(color));
    if scroll_here {
        label_response.scroll_to_me(Some(egui::Align::TOP));
    }

    // The link group ID encodes the X range, so changing focus (which
    // changes x_range) creates a *fresh* link group. Without this, egui_plot
    // persists the previous wider X range across frames and `include_x` only
    // expands — so small TPs would render squished inside the stale range.
    let link_group = ui
        .id()
        .with(("signal_spine_x_link", x_min.to_bits(), x_max.to_bits()));
    let active_x = ctx.active_x;
    let display_x = ctx.display_x;
    let hotspots = ctx.hotspots;
    let pass_bands = ctx.pass_bands;
    let mut hovered: Option<f64> = None;
    let mut clicked_hotspot: Option<usize> = None;
    let mut dragged_now = false;
    let mut seek: Option<usize> = None;
    let response = Plot::new(format!("signal_track_{label}"))
        .height(90.0)
        // Wheel zooms horizontally; drag is *not* used by the plot — we
        // intercept it below as a scrub gesture instead so dragging across
        // a track scrubs playback rather than panning the graph.
        .allow_zoom([true, false])
        .allow_drag([false, false])
        .allow_scroll([false, false])
        .allow_boxed_zoom(false)
        .link_axis(link_group, [true, false])
        .link_cursor(link_group, [true, false])
        .show_axes([true, true])
        .show_grid([true, true])
        .include_x(x_min)
        .include_x(x_max)
        .show(ui, |plot_ui| {
            // DepthPass bands first so the data lines and limit lines render
            // on top of them. Bands have transparent strokes and disabled
            // hover so they don't show up in legend hover or intercept clicks.
            if !pass_bands.is_empty() {
                let band_y_min = min_y - (max_y - min_y).abs() * 0.1;
                let band_y_max = max_y + (max_y - min_y).abs() * 0.1;
                let band_stroke = egui::Stroke::new(0.0_f32, egui::Color32::TRANSPARENT);
                for (idx, (g_start, g_end, selected)) in pass_bands.iter().enumerate() {
                    // UP4: three washes over one plot ground. They stay
                    // PREMULTIPLIED — `tokens::accent_wash` is unmultiplied,
                    // and at these alphas it would leave the selected band
                    // dimmer than its unselected neighbours. The base colours
                    // are the tokens.
                    let wash = |c: egui::Color32, a: u8| {
                        egui::Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), a)
                    };
                    let fill = if *selected {
                        wash(crate::ui::tokens::ACCENT_QUIET, 28)
                    } else if idx % 2 == 0 {
                        wash(crate::ui::tokens::SURFACE_OVERLAY, 12)
                    } else {
                        wash(crate::ui::tokens::SURFACE_RAISED, 8)
                    };
                    plot_ui.polygon(
                        Polygon::new(
                            "",
                            PlotPoints::from(vec![
                                [*g_start, band_y_min],
                                [*g_end, band_y_min],
                                [*g_end, band_y_max],
                                [*g_start, band_y_max],
                            ]),
                        )
                        .fill_color(fill)
                        .stroke(band_stroke)
                        .allow_hover(false)
                        .name(""),
                    );
                }
            }

            // One Line per toolpath, further split into contiguous runs
            // wherever consecutive surviving samples have a move gap >
            // MAX_LINE_BRIDGE_GAP. A gap means the value was absent for the
            // in-between samples (for example an air cut); one Line across
            // the gap would bridge them with a misleading diagonal.
            //
            // Threshold > 1 (not > 0) absorbs decimation-induced gaps.
            const MAX_LINE_BRIDGE_GAP: usize = 16;
            for (_tp_color, pts) in group_points {
                let mut run_start = 0usize;
                for (i, pair) in pts.windows(2).enumerate() {
                    let (Some(prev), Some(cur)) = (pair.first(), pair.get(1)) else {
                        continue;
                    };
                    if cur.0.saturating_sub(prev.0) > MAX_LINE_BRIDGE_GAP {
                        let end = i + 1;
                        let xy: Vec<[f64; 2]> = pts
                            .get(run_start..end)
                            .unwrap_or_default()
                            .iter()
                            .map(|(_, p)| *p)
                            .collect();
                        if xy.len() >= 2 {
                            plot_ui
                                .line(Line::new("", PlotPoints::from(xy)).name(label).color(color));
                        }
                        run_start = end;
                    }
                }
                let xy: Vec<[f64; 2]> = pts
                    .get(run_start..)
                    .unwrap_or_default()
                    .iter()
                    .map(|(_, p)| *p)
                    .collect();
                if xy.len() >= 2 {
                    plot_ui.line(Line::new("", PlotPoints::from(xy)).name(label).color(color));
                }
            }

            // The gate's own limit lines, in this track's unit. The same
            // numbers the card draws; a line only for a bound that exists.
            for (bound, colour, name) in [
                (bounds.floor, crate::ui::tokens::CAUTION, "floor"),
                (bounds.ceiling, crate::ui::tokens::DANGER, "ceiling"),
            ] {
                let Some(value) = bound else {
                    continue;
                };
                let mut line =
                    Line::new("", PlotPoints::from(vec![[x_min, value], [x_max, value]]))
                        .color(colour)
                        .name(name);
                if bounds.advisory {
                    line = line.style(egui_plot::LineStyle::Dashed { length: 6.0 });
                }
                plot_ui.line(line);
            }

            if let Some(x) = active_x {
                plot_ui.line(
                    Line::new("", PlotPoints::from(vec![[x, min_y], [x, max_y]]))
                        .color(crate::ui::tokens::TEXT_STRONG)
                        .name("playback"),
                );
            }

            if let Some(x) = display_x {
                plot_ui.line(
                    Line::new("", PlotPoints::from(vec![[x, min_y], [x, max_y]]))
                        .color(crate::ui::tokens::ACCENT)
                        .style(egui_plot::LineStyle::Dashed { length: 4.0 }),
                );
                if let Some((_, point)) = nearest_in_groups(x, group_points) {
                    plot_ui.text(egui_plot::Text::new(
                        "",
                        egui_plot::PlotPoint::new(point[0], point[1]),
                        format!("{label}: {:.3} @ move {}", point[1], point[0] as usize),
                    ));
                }
            }

            if !hotspots.is_empty() {
                let hotspot_pts: Vec<[f64; 2]> = hotspots
                    .iter()
                    .filter_map(|hs| {
                        nearest_in_groups(hs.global_move as f64, group_points).map(|(_, pt)| pt)
                    })
                    .collect();
                if !hotspot_pts.is_empty() {
                    plot_ui.points(
                        egui_plot::Points::new("", PlotPoints::from(hotspot_pts))
                            .color(super::theme::ERROR)
                            .radius(4.0_f32)
                            .name("gate trips"),
                    );
                }
            }

            // Only react when the pointer is actually over the plot rect AND
            // within the data X range. This prevents the playhead jumping when
            // the user moves the mouse past the left/right edges of the plot.
            let pointer_in_rect = plot_ui.response().hovered();
            let dragged = plot_ui.response().dragged();
            if dragged {
                dragged_now = true;
            }
            if (pointer_in_rect || dragged)
                && let Some(pointer) = plot_ui.pointer_coordinate()
                && pointer.x >= x_min
                && pointer.x <= x_max
            {
                hovered = Some(pointer.x);
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
                        clicked_hotspot = Some(hs.global_move);
                    } else if let Some((global_move, _)) =
                        nearest_in_groups(pointer.x, group_points)
                    {
                        seek = Some(global_move);
                    }
                } else if dragged
                    && let Some((global_move, _)) = nearest_in_groups(pointer.x, group_points)
                {
                    seek = Some(global_move);
                }
            }
        });
    if dragged_now {
        *ctx.scrub_drag_active = true;
    }
    if hovered.is_some() {
        *ctx.new_hovered = hovered;
    }
    if clicked_hotspot.is_some() {
        *ctx.clicked_hotspot = clicked_hotspot;
    }
    if let Some(move_index) = seek {
        ctx.events
            .push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
                move_index,
            })));
    }
    // TIM-004 — the primary interactivity disclosure is the cursor change
    // (the track is scrubbable), not a wall of prose; keep one short line.
    if response.response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    response
        .response
        .on_hover_text("Click or drag to scrub · scroll to zoom");
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

/// Blend a colour toward its own luminance (neutral grey) and dim it, for the
/// stale-skin treatment so a last-run trace reads as not-current (TIM-009).
fn desaturate(c: egui::Color32) -> egui::Color32 {
    let lum = (0.3 * c.r() as f32 + 0.59 * c.g() as f32 + 0.11 * c.b() as f32) as u8;
    let mix = |ch: u8| (((ch as u16) + (lum as u16) * 2) / 3) as u8;
    egui::Color32::from_rgb(mix(c.r()), mix(c.g()), mix(c.b())).linear_multiply(0.7)
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
            events.push(AppEvent::Ui(UiCommand::SimStepBackward(NoArgs)));
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
            events.push(AppEvent::Ui(UiCommand::ToggleSimPlayback(NoArgs)));
        }
        if ui
            .add(egui::Button::new("►").min_size(btn_size))
            .on_hover_text("Step forward (Right arrow)")
            .clicked()
        {
            events.push(AppEvent::Ui(UiCommand::SimStepForward(NoArgs)));
        }

        // Pass-jump buttons removed — the span ribbon below the boundary
        // timeline replaces them with a clickable visual navigator.

        if sim.total_moves() > 0 {
            ui.separator();

            let (elapsed_time, cycle) = estimate_times(sim, session, gui);
            let total_time = cycle.seconds;
            let elapsed_str = readiness::format_cycle_time(elapsed_time);
            let total_str = readiness::format_cycle_time(total_time);
            ui.label(
                egui::RichText::new(format!("{} / {}", elapsed_str, total_str))
                    .monospace()
                    .color(crate::ui::tokens::TEXT_STRONG),
            );
            // Name the basis next to the clock (G-TIMEEST). The readout drives
            // the speed baseline below, so an operator who mistrusts one has to
            // be able to see why the other moved.
            let (basis_tag, basis_tip, basis_color) = match cycle.basis {
                Some(basis) => (
                    basis.qualifier(),
                    match basis.remedy() {
                        Some(remedy) => format!("{}\n\n{remedy}", basis.caveat()),
                        None => basis.caveat().to_owned(),
                    },
                    if basis == readiness::CycleTimeBasis::MachineModel {
                        super::theme::TEXT_MUTED
                    } else {
                        super::theme::WARNING
                    },
                ),
                None => (
                    "no estimate",
                    "No computed toolpath in this simulation carries a time estimate.".to_owned(),
                    super::theme::WARNING,
                ),
            };
            ui.label(
                egui::RichText::new(format!("({basis_tag})"))
                    .small()
                    .color(basis_color),
            )
            .on_hover_text(basis_tip);

            ui.separator();

            // Playback speed as a multiplier (×) where 1× = real-time
            // playback for *this* project. The baseline is derived from the
            // project's actual move rate (total_moves / total_time_s) so
            // wanaka's ~200 mv/s real-time and a small project's ~50 mv/s
            // real-time both map to "1× = real time".
            //
            // G-TIMEEST: `total_time` above used to be the cutting-only
            // estimate, which made this baseline ~7× too FAST on a measured
            // job — 1× played a three-hour cut as a 25-minute one while the
            // tooltip claimed real time. It now shares `estimate_times`' one
            // source. The two quantities are the same population by
            // construction: `estimate_times` sums over `sim.boundaries()`,
            // which is what `sim.total_moves()` counts.
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
                    .color(crate::ui::tokens::TEXT_MUTED),
            )
            .on_hover_text(format!(
                "Playback speed multiplier. 1× = real-time playback for this project ({:.0} moves/sec, {}). [ and ] keys to adjust.",
                real_time_mv_s, basis_tag
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

/// Row 2: the **Tier-2 time axis** (pass-2 timeline §2) — the single widget
/// that owns the project-global move axis. It stacks the op segments, the span
/// sub-band, and (in debug) the semantic sub-band as Y-regions of ONE allocated
/// rect, painted with ONE shared playhead and governed by ONE mode-aware click
/// contract: drag anywhere scrubs; a click's side-effect is decided by which
/// sub-band Y-region it lands in. This kills the three-divergent-axes /
/// three-orphan-playheads problem (TIM-001/002).
#[allow(clippy::too_many_arguments)]
fn draw_boundary_timeline(
    ui: &mut egui::Ui,
    sim: &mut SimulationState,
    gui: &GuiState,
    load_report: &ToolLoadReport,
    current_boundary: &Option<crate::state::simulation::ToolpathBoundary>,
    active_semantic: &Option<ActiveSemanticItem>,
    events: &mut Vec<AppEvent>,
) {
    if sim.total_moves() == 0 || sim.boundaries().is_empty() {
        return;
    }

    // Decide which sub-bands are present so their slot heights can be reserved
    // before the single allocate.
    let span_scope_tp = span_subband_present(sim, gui);
    let semantic_present = sim.debug.enabled
        && current_boundary
            .as_ref()
            .is_some_and(|b| semantic_subband_present(sim, gui, b));

    let op_h = 28.0_f32;
    let span_h = 12.0_f32;
    let sem_h = 10.0_f32;
    let gap = 2.0_f32;
    let mut total_h = op_h;
    if span_scope_tp.is_some() {
        total_h += gap + span_h;
    }
    if semantic_present {
        total_h += gap + sem_h;
    }

    let total_width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(total_width, total_h),
        egui::Sense::click_and_drag(),
    );
    let painter = ui.painter_at(rect);
    let total_moves = sim.total_moves().max(1) as f32;
    let global_x =
        |move_idx: usize| -> f32 { rect.min.x + (move_idx as f32 / total_moves) * total_width };

    let op_rect = egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.min.y + op_h));
    let mut next_y = op_rect.max.y + gap;
    let span_rect = span_scope_tp.map(|_| {
        let r = egui::Rect::from_min_max(
            egui::pos2(rect.min.x, next_y),
            egui::pos2(rect.max.x, next_y + span_h),
        );
        next_y = r.max.y + gap;
        r
    });
    let sem_rect = semantic_present.then(|| {
        egui::Rect::from_min_max(
            egui::pos2(rect.min.x, next_y),
            egui::pos2(rect.max.x, next_y + sem_h),
        )
    });

    // ── Op band ──
    let rounding = 6.0;
    painter.rect_stroke(
        op_rect,
        rounding,
        egui::Stroke::new(1.0_f32, crate::ui::tokens::HAIRLINE),
        egui::StrokeKind::Middle,
    );
    for (i, boundary) in sim.boundaries().iter().enumerate() {
        let op_moves = boundary.end_move.saturating_sub(boundary.start_move);
        let x_start = global_x(boundary.start_move);
        let x_end = global_x(boundary.end_move);
        let pc = palette_color(i);
        let color = crate::ui::tokens::from_linear_rgb(pc);
        // The unplayed remainder of the band: the SAME palette colour dimmed,
        // which is what the second literal here used to spell out by hand.
        let dim_color = color.linear_multiply(50.0 / 255.0);
        let seg_rect = egui::Rect::from_min_max(
            egui::pos2(x_start, op_rect.min.y),
            egui::pos2(x_end, op_rect.max.y),
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
            egui::pos2(x_start, op_rect.min.y),
            egui::vec2(fill_width, op_h),
        );
        painter.rect_filled(fill_rect, rounding, color);
    }

    // Inspector focus filters the op-band markers (segments stay project-wide).
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
        let holder_color = crate::ui::tokens::DANGER;
        for col in &report.collisions {
            if !in_focus(col.move_index) {
                continue;
            }
            let x = global_x(col.move_index);
            painter.line_segment(
                [egui::pos2(x, op_rect.min.y), egui::pos2(x, op_rect.max.y)],
                egui::Stroke::new(2.0_f32, holder_color),
            );
        }
    }
    // UP4: a rapid strike is a collision, so it takes DANGER like the
    // holder strike above. The two stay apart by stroke width — 2.0 for the
    // holder, 1.5 here — which was already the case.
    let rapid_color = crate::ui::tokens::DANGER;
    for &idx in &sim.checks.rapid_collision_move_indices {
        if !in_focus(idx) {
            continue;
        }
        let x = global_x(idx);
        painter.line_segment(
            [egui::pos2(x, op_rect.min.y), egui::pos2(x, op_rect.max.y)],
            egui::Stroke::new(1.5_f32, rapid_color),
        );
    }
    draw_tool_load_timeline_markers(
        &painter,
        op_rect,
        total_moves,
        total_width,
        sim,
        load_report,
        focused_id,
    );

    // ── Span sub-band (indented, thinner — telegraphs "scope a pass") ──
    if let (Some(tp_id), Some(span_rect)) = (span_scope_tp, span_rect) {
        paint_span_subband(
            ui,
            span_rect,
            &response,
            sim,
            gui,
            tp_id,
            total_moves,
            total_width,
            events,
        );
    }
    // ── Semantic sub-band (debug-only, now project-global X) ──
    if let (Some(sem_rect), Some(boundary)) = (sem_rect, current_boundary.as_ref()) {
        paint_semantic_subband(
            ui,
            sem_rect,
            &response,
            sim,
            gui,
            boundary,
            active_semantic.as_ref(),
            total_moves,
            total_width,
            events,
        );
    }

    // ── One playhead, full height through every band ──
    let pos_x = global_x(sim.playback.current_move);
    painter.line_segment(
        [
            egui::pos2(pos_x, rect.min.y - 1.0),
            egui::pos2(pos_x, rect.max.y + 1.0),
        ],
        egui::Stroke::new(2.0_f32, egui::Color32::WHITE),
    );
    let diamond_center = egui::pos2(pos_x, rect.min.y);
    let diamond_size = 4.0;
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(diamond_center.x, diamond_center.y - diamond_size),
            egui::pos2(diamond_center.x + diamond_size, diamond_center.y),
            egui::pos2(diamond_center.x, diamond_center.y + diamond_size),
            egui::pos2(diamond_center.x - diamond_size, diamond_center.y),
        ],
        egui::Color32::WHITE,
        egui::Stroke::NONE,
    ));

    // ── Op-band hover tooltip (markers) ──
    if response.hovered()
        && let Some(pos) = response.hover_pos()
        && (op_rect.min.y..=op_rect.max.y).contains(&pos.y)
        && let Some(tip) = nearest_marker_tooltip(
            pos.x,
            op_rect,
            total_moves,
            total_width,
            sim,
            load_report,
            focused_id,
        )
    {
        egui::Tooltip::always_open(
            ui.ctx().clone(),
            ui.layer_id(),
            egui::Id::new("sim_timeline_marker_tip"),
            egui::PopupAnchor::Pointer,
        )
        .show(|ui| {
            ui.label(tip);
        });
    }

    // ── Click contract ──
    // Drag anywhere = positional scrub (the one universal gesture). A click's
    // side-effect is decided by Y-region: the span / semantic sub-bands handle
    // their own clicks inside their paint fns above, so here we only resolve a
    // click that lands in the op band (safety-marker focus, else seek).
    if response.dragged() {
        sim.playback.scrub_drag_active = true;
        if let Some(pos) = response.interact_pointer_pos() {
            let frac = ((pos.x - rect.min.x) / total_width).clamp(0.0, 1.0);
            sim.playback.current_move = (frac * total_moves) as usize;
            sim.playback.playing = false;
        }
    } else if response.clicked()
        && let Some(pos) = response.interact_pointer_pos()
        && (op_rect.min.y..=op_rect.max.y).contains(&pos.y)
    {
        if let Some(target) =
            nearest_safety_marker_move(pos.x, op_rect, total_moves, total_width, sim, load_report)
        {
            sim.playback.current_move = target;
            sim.playback.playing = false;
            events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
                move_index: target,
            })));
        } else {
            let frac = ((pos.x - rect.min.x) / total_width).clamp(0.0, 1.0);
            sim.playback.current_move = (frac * total_moves) as usize;
            sim.playback.playing = false;
        }
    }
}

/// Should a span sub-band render, and for which toolpath? Mirrors the guards
/// `paint_span_subband` needs so the Tier-2 widget can reserve its slot height
/// before the single allocate.
fn span_subband_present(
    sim: &SimulationState,
    gui: &GuiState,
) -> Option<crate::state::toolpath::ToolpathId> {
    if sim.total_moves() == 0 || sim.boundaries().is_empty() {
        return None;
    }
    let tp_id = sim
        .debug
        .span_scope
        .toolpath_id
        .or_else(|| sim.current_boundary().map(|b| b.id))?;
    sim.boundaries().iter().find(|b| b.id == tp_id)?;
    let rt = gui.toolpath_rt.get(&tp_id)?;
    let result = rt.result.as_ref()?;
    if !result.spans_valid() || result.spans().is_empty() {
        return None;
    }
    Some(tp_id)
}

/// Paint the **span sub-band** of the Tier-2 time axis: structural span
/// subdivisions for the scope toolpath, in the shared project-global X space.
/// DepthPass spans render as primary blocks (Region spans when no DepthPasses);
/// a selected DepthPass shows its Region children as a lighter sub-tier. Hover
/// shows the span label; a click (when the pointer is in this sub-band's Y
/// region) sets the chip-row scope and seeks to the span start. No own
/// playhead — the Tier-2 widget paints one shared playhead over every band.
#[allow(clippy::too_many_arguments)]
fn paint_span_subband(
    ui: &egui::Ui,
    rect: egui::Rect,
    response: &egui::Response,
    sim: &mut SimulationState,
    gui: &GuiState,
    tp_id: crate::state::toolpath::ToolpathId,
    total_moves: f32,
    total_width: f32,
    events: &mut Vec<AppEvent>,
) {
    use rs_cam_core::trace::toolpath_spans::{SpanKind, SpanPayload};

    let Some(boundary) = sim.boundaries().iter().find(|b| b.id == tp_id).cloned() else {
        return;
    };
    let Some(rt) = gui.toolpath_rt.get(&tp_id) else {
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

    let height = rect.height();
    let painter = ui.painter_at(rect);

    // Background — dim track so DepthPass blocks have something to sit on
    // before they paint. Visible at the edges of the focused toolpath
    // (outside the boundary segment).
    painter.rect_filled(
        rect,
        2.0,
        egui::Color32::from_rgba_unmultiplied(
            crate::ui::tokens::DIAGRAM_CANVAS.r(),
            crate::ui::tokens::DIAGRAM_CANVAS.g(),
            crate::ui::tokens::DIAGRAM_CANVAS.b(),
            220,
        ),
    );

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
    // UP4: the ribbon blocks are a CATEGORY plus two states, so they walk
    // `SPAN_SCALE` and keep their original brightness order — odd darkest,
    // then even, then the playhead-current block, then the locked one.
    // SAFETY: every index is a literal in `0..6` and `SPAN_SCALE` has six
    // entries, so each one is in bounds at compile time.
    #[allow(clippy::indexing_slicing)]
    const COLOR_EVEN: egui::Color32 = crate::ui::tokens::SPAN_SCALE[1];
    #[allow(clippy::indexing_slicing)]
    const COLOR_ODD: egui::Color32 = crate::ui::tokens::SPAN_SCALE[0];
    #[allow(clippy::indexing_slicing)]
    const COLOR_LOCKED: egui::Color32 = crate::ui::tokens::SPAN_SCALE[5];
    #[allow(clippy::indexing_slicing)]
    const COLOR_PLAYHEAD: egui::Color32 = crate::ui::tokens::SPAN_SCALE[3];
    const COLOR_HOVER_OUTLINE: egui::Color32 = crate::ui::tokens::TEXT_STRONG;

    let mut primary_index_seq = 0u32;
    let mut click_target: Option<(u32, usize)> = None;
    let mut hover_label: Option<String> = None;
    // Gate hover/click to this sub-band's Y region so the shared Tier-2
    // response only drives span scoping when the pointer is actually here.
    let pointer_x = response
        .hover_pos()
        .filter(|p| (rect.min.y..=rect.max.y).contains(&p.y))
        .map(|p| p.x);

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
            Some(SpanPayload::Region { region_id, .. }) if !has_depth_passes => *region_id,
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
            painter.rect_stroke(
                block,
                0.0,
                egui::Stroke::new(1.5_f32, COLOR_HOVER_OUTLINE),
                egui::StrokeKind::Middle,
            );
        }

        // Thin separator on the right edge so consecutive passes don't blur.
        painter.line_segment(
            [egui::pos2(x_end, rect.min.y), egui::pos2(x_end, rect.max.y)],
            egui::Stroke::new(0.5_f32, crate::ui::tokens::HAIRLINE),
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
            // UP4: Region sub-blocks are another span KIND, so they take
            // two lighter steps of the same category ramp rather than the
            // amber pair they carried.
            // SAFETY: both indices are literals in `0..6`.
            #[allow(clippy::indexing_slicing)]
            let color = if Some(sid_u32) == scope_span_id {
                crate::ui::tokens::SPAN_SCALE[4]
            } else {
                let c = crate::ui::tokens::SPAN_SCALE[2];
                egui::Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 180)
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
                    Some(SpanPayload::Region { region_id, .. }) => *region_id,
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

    // (No own playhead — the Tier-2 widget paints one shared full-height
    // playhead over the op band and every sub-band.)

    if let Some(tip) = hover_label {
        egui::Tooltip::always_open(
            ui.ctx().clone(),
            ui.layer_id(),
            egui::Id::new("sim_span_ribbon_tip"),
            egui::PopupAnchor::Pointer,
        )
        .show(|ui| {
            ui.label(tip);
        });
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
        events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
            move_index: jump_move,
        })));
    }
}

fn ribbon_span_label(
    span: &rs_cam_core::trace::toolpath_spans::Span,
    fallback_index: u32,
) -> String {
    use rs_cam_core::trace::toolpath_spans::{SpanKind, SpanPayload};

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
        (SpanKind::Region, Some(SpanPayload::Region { region_id, .. })) => {
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
            && verdict.toolpath_id != focus
        {
            continue;
        }
        if let Some(global_move) = first_exceeded_tool_load_move(sim, trace, verdict) {
            let x = rect.min.x + (global_move as f32 / total_moves) * total_width;
            painter.line_segment(
                [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
                egui::Stroke::new(2.0_f32, crate::ui::tokens::DANGER),
            );
        } else if verdict.any_unmodeled()
            && let Some(boundary) = sim
                .boundaries()
                .iter()
                .find(|boundary| boundary.id == verdict.toolpath_id)
        {
            let x = rect.min.x + (boundary.start_move as f32 / total_moves) * total_width;
            painter.line_segment(
                [egui::pos2(x, rect.min.y), egui::pos2(x, rect.center().y)],
                // UP4: `any_unmodeled` is an ABSTENTION. The marker wore
                // the caution amber; the gate returned no verdict at all.
                egui::Stroke::new(1.5_f32, crate::ui::tokens::UNKNOWN),
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
            if !in_focus(col.move_index) {
                continue;
            }
            consider(
                col.move_index,
                format!(
                    "{}: holder collision at move {} — click to navigate",
                    tp_name_for_move(col.move_index),
                    col.move_index
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
                && verdict.toolpath_id != focus
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
                        "advance/tooth {label} peak {:.4} mm/tooth",
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
    trace: &rs_cam_core::stock::simulation_cut::SimulationCutTrace,
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
        .find(|boundary| boundary.id == sample.toolpath_id)
        .map(|boundary| boundary.start_move)
        .unwrap_or_default();
    Some(boundary_start + sample.move_index)
}

/// Elapsed seconds at the playhead, plus the project's total cycle time and
/// the basis it was measured on.
///
/// G-TIMEEST: this used to hold its own `cutting_distance / feed_rate()` — one
/// of four copies of that formula, all of them printing a cutting-only figure
/// under the word "time". The per-op number now comes from the single
/// [`readiness::toolpath_cycle_time`] decision; only the *population* is local
/// (the simulated boundaries, so the derived playback baseline divides by the
/// same set `sim.total_moves()` counts).
///
/// Elapsed is still interpolated linearly in MOVES within an op, which is its
/// own approximation — moves are not equal-duration — but it only affects the
/// left half of the readout, never the total or the speed baseline.
fn estimate_times(
    sim: &SimulationState,
    session: &ProjectSession,
    gui: &GuiState,
) -> (f64, readiness::CycleTime) {
    let trace = sim.results.as_ref().and_then(|r| r.cut_trace.as_ref());
    let mut total = readiness::CycleTime::NONE;
    let mut elapsed_secs = 0.0;

    for boundary in sim.boundaries() {
        if let Some(rt) = gui.toolpath_rt.get(&boundary.id)
            && let Some(result) = &rt.result
            && let Some((_, tc)) = session.find_toolpath_config_by_id(boundary.id)
        {
            let op = readiness::toolpath_cycle_time(
                session,
                trace,
                boundary.id,
                result.stats.cutting_distance,
                tc.operation.feed_rate(),
            );
            total.fold(op);

            // Estimate elapsed time for this op
            let op_moves = boundary.end_move.saturating_sub(boundary.start_move);
            let progress = if sim.playback.current_move >= boundary.end_move {
                1.0
            } else if sim.playback.current_move <= boundary.start_move {
                0.0
            } else {
                (sim.playback.current_move - boundary.start_move) as f64 / op_moves.max(1) as f64
            };
            elapsed_secs += op.seconds * progress;
        }
    }

    (elapsed_secs, total)
}

/// Whether the debug semantic sub-band should render for `boundary`.
fn semantic_subband_present(
    sim: &SimulationState,
    gui: &GuiState,
    boundary: &crate::state::simulation::ToolpathBoundary,
) -> bool {
    let Some(rt) = gui.toolpath_rt.get(&boundary.id) else {
        return false;
    };
    rt.semantic_trace.is_some() && sim.debug.semantic_indexes.contains_key(&boundary.id)
}

/// Paint the **semantic sub-band** (debug-only) of the Tier-2 time axis. This
/// was the one strip on a divergent toolpath-LOCAL X axis with its own
/// playhead; it now renders in the shared project-global X space (each item
/// offset by `boundary.start_move`) and shares the single Tier-2 playhead and
/// click contract (TIM-001/002). Click side-effects (annotation → DebugTrace,
/// cut-issue → CutQuality, semantic item → pin) are unchanged.
// SAFETY: item_index and depths[] from semantic index built from trace.items
#[allow(clippy::indexing_slicing)]
#[allow(clippy::too_many_arguments)]
fn paint_semantic_subband(
    ui: &egui::Ui,
    rect: egui::Rect,
    response: &egui::Response,
    sim: &mut SimulationState,
    gui: &GuiState,
    boundary: &crate::state::simulation::ToolpathBoundary,
    active_semantic: Option<&ActiveSemanticItem>,
    total_moves: f32,
    total_width: f32,
    events: &mut Vec<AppEvent>,
) {
    let Some(rt) = gui.toolpath_rt.get(&boundary.id) else {
        return;
    };
    let Some(trace) = rt.semantic_trace.as_ref() else {
        return;
    };
    let Some(index) = sim.debug.semantic_indexes.get(&boundary.id).cloned() else {
        return;
    };

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, crate::ui::tokens::SURFACE_BASE);
    let global_x = |local: usize| -> f32 {
        rect.min.x + ((boundary.start_move + local) as f32 / total_moves) * total_width
    };

    let mut segments = index.move_item_indices.clone();
    segments.sort_by_key(|item_index| index.depths[*item_index]);
    for item_index in segments {
        let item = &trace.items[item_index];
        let (Some(move_start), Some(move_end)) = (item.move_start, item.move_end) else {
            continue;
        };
        let x_start = global_x(move_start);
        let x_end = global_x(move_end + 1);
        let seg_rect = egui::Rect::from_min_max(
            egui::pos2(x_start, rect.min.y),
            egui::pos2(x_end.max(x_start + 1.0), rect.max.y),
        );
        let color = semantic_kind_color(&item.kind);
        painter.rect_filled(seg_rect, 1.0, color.linear_multiply(0.85));
        if active_semantic
            .is_some_and(|active| active.toolpath_id == boundary.id && active.item.id == item.id)
        {
            painter.rect_stroke(
                seg_rect,
                1.0,
                egui::Stroke::new(1.5_f32, egui::Color32::WHITE),
                egui::StrokeKind::Middle,
            );
        }
    }

    if let Some(debug_trace) = rt.debug_trace.as_ref() {
        for annotation in &debug_trace.annotations {
            let x = global_x(annotation.move_index);
            painter.line_segment(
                [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
                // UP4: a generator annotation is a mark on a DRAWING, not
                // a caution. `DIAGRAM_INK` is the token for the thing the
                // drawing is describing (`DESIGN_SPEC.md` §2.8).
                egui::Stroke::new(1.0_f32, crate::ui::tokens::DIAGRAM_INK),
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
            .filter(|issue| issue.toolpath_id == boundary.id)
        {
            let x = global_x(issue.move_index);
            // UP4: these two dots distinguish the KIND of a sample-level
            // finding, and they did it with an orange and an amber — the
            // verdict palette spent on a category (`DESIGN_SPEC.md` §2.6
            // principle 1). Neither dot is a judgement on the toolpath; the
            // verdict lives in the load report. They take two steps of the
            // chart-series ramp instead.
            // SAFETY: both indices are literals in `0..4` and `CHART_SERIES`
            // has four entries.
            #[allow(clippy::indexing_slicing)]
            let color = match issue.kind {
                rs_cam_core::stock::simulation_cut::SimulationCutIssueKind::AirCut => {
                    crate::ui::tokens::CHART_SERIES[1]
                }
                rs_cam_core::stock::simulation_cut::SimulationCutIssueKind::LowEngagement => {
                    crate::ui::tokens::CHART_SERIES[3]
                }
            };
            painter.circle_filled(egui::pos2(x, rect.center().y), 2.5, color);
        }
    }

    // No own playhead — the shared Tier-2 playhead covers this band.

    if response.clicked()
        && let Some(pointer) = response.interact_pointer_pos()
        && (rect.min.y..=rect.max.y).contains(&pointer.y)
    {
        if let Some(debug_trace) = rt.debug_trace.as_ref() {
            let nearest_annotation = debug_trace
                .annotations
                .iter()
                .enumerate()
                .map(|(index, annotation)| {
                    let x = global_x(annotation.move_index);
                    (index, annotation, (pointer.x - x).abs())
                })
                .filter(|(_, _, distance)| *distance <= 5.0)
                .min_by(|left, right| left.2.total_cmp(&right.2));
            if let Some((annotation_index, annotation, _)) = nearest_annotation
                && let Some(target) = sim.trace_target_for_annotation(boundary.id, annotation)
            {
                sim.debug.focused_issue_index = None;
                sim.debug.focused_hotspot = None;
                sim.clear_pinned_semantic_item();
                let _ = annotation_index;
                events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
                    move_index: target.move_index,
                })));
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
                .filter(|issue| issue.toolpath_id == boundary.id)
                .map(|issue| {
                    let x = global_x(issue.move_index);
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
                events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
                    move_index: target.move_index,
                })));
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
                let x_start = global_x(move_start);
                let x_end = global_x(move_end + 1);
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
            if let Some(target) = sim.trace_target_for_item(gui, boundary.id, item.id, false) {
                events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
                    move_index: target.move_index,
                })));
            }
            return;
        }

        let frac = ((pointer.x - rect.min.x) / total_width).clamp(0.0, 1.0);
        let global_move = (frac * total_moves) as usize;
        sim.clear_pinned_semantic_item();
        sim.debug.focused_issue_index = None;
        sim.debug.focused_hotspot = None;
        events.push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
            move_index: global_move,
        })));
    }
}
