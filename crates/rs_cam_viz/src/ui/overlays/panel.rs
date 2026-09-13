//! The Overlays panel — one surface that lists every viewport overlay.
//!
//! It is a **viewer, not an editor** (UX §6.6). Choosing a rest source or
//! setting a boundary offset stays in the properties panel; a disabled row's
//! compute button runs the thing its reason names and nothing else.
//!
//! It **caches nothing**. Every flag is read from [`AppState`] each frame,
//! because the multi-tool planner writes `show_tier_preview` on its own and
//! the workspace switch rewrites a dozen flags behind the panel's back
//! (audit §5, rule 2).

use crate::state::AppState;
use crate::ui::AppEvent;
use crate::ui::theme;
use crate::ui_command::{NoArgs, UiCommand};

use super::registry::{self, Legend, OverlayAction, OverlayGroup, OverlayRow, OverlaySurface};

/// Width of the docked column, and the floating window's default width.
const PANEL_WIDTH: f32 = 232.0;

/// The 3D view's floor, in points. The docked Overlays column refuses to dock
/// rather than take the viewport below this.
///
/// P6, 2026-09-08: `screenshot_gui` at the window's own 1400 x 900 came back
/// with **no 3D viewport at all** — the operation list and the inspector
/// filled the frame. `draw_docked` took `exact_size(PANEL_WIDTH)` out of
/// whatever the two side panels had left, and `app.rs` then handed
/// `ui.available_size()` straight to `allocate_exact_size`, so once the side
/// panels had been dragged wide (egui remembers a resizable panel's width
/// across frames and windows, and does not shrink it when the window does)
/// the remainder went to zero and the viewport vanished silently. A viewport
/// that can reach zero width is a screenshot surface that can return a
/// picture of nothing.
pub const MIN_VIEWPORT_WIDTH: f32 = 320.0;

/// The docked form: a column inside the viewport. Called BEFORE the 3D view
/// claims the remaining space, so the column takes width from it rather than
/// covering it.
///
/// Returns **true when it actually docked**. It refuses in two cases: the
/// panel is closed or unpinned, and — P6 — the space left over would put the
/// 3D view under [`MIN_VIEWPORT_WIDTH`]. The caller turns a refusal into the
/// floating form, so a pinned panel on a narrow window becomes a window over
/// the viewport instead of erasing it.
pub fn draw_docked(ui: &mut egui::Ui, state: &mut AppState, events: &mut Vec<AppEvent>) -> bool {
    if !state.overlays.open || !state.overlays.pinned {
        return false;
    }
    if ui.available_width() - PANEL_WIDTH < MIN_VIEWPORT_WIDTH {
        return false;
    }
    egui::Panel::left("overlays_panel")
        .resizable(false)
        .exact_size(PANEL_WIDTH)
        .frame(
            egui::Frame::default()
                .fill(theme::CARD_FILL)
                .inner_margin(egui::Margin::same(6)),
        )
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| body(ui, state, events));
        });
    true
}

/// The floating form: a window over the 3D view, anchored to the viewport's
/// top-left so it does not open in the middle of the screen. A no-op unless
/// the panel is open and unpinned.
pub fn draw_floating(
    ui: &mut egui::Ui,
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
    viewport_rect: egui::Rect,
    docked: bool,
) {
    if !state.overlays.open || docked {
        return;
    }
    let anchor = viewport_rect.min + egui::vec2(8.0, 8.0);
    egui::Window::new("Overlays")
        .id(egui::Id::new("overlays_window"))
        .default_pos(anchor)
        .default_width(PANEL_WIDTH)
        .resizable(true)
        .collapsible(false)
        .show(ui.ctx(), |ui| {
            egui::ScrollArea::vertical()
                .max_height(560.0)
                .show(ui, |ui| body(ui, state, events));
        });
}

/// The panel's contents, identical in both forms.
fn body(ui: &mut egui::Ui, state: &mut AppState, events: &mut Vec<AppEvent>) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("OVERLAYS")
                .small()
                .strong()
                .color(theme::TEXT_HEADING),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .small_button("\u{2715}")
                .on_hover_text("Close the Overlays panel (shortcut: O)")
                .clicked()
            {
                state.overlays.open = false;
            }
            let pin_label = if state.overlays.pinned {
                "\u{1F4CC} unpin"
            } else {
                "\u{1F4CC} pin"
            };
            if ui
                .small_button(pin_label)
                .on_hover_text(if state.overlays.pinned {
                    "Float the panel over the viewport (shortcut: Shift+O)"
                } else {
                    "Dock the panel as a column inside the viewport (shortcut: Shift+O)"
                })
                .clicked()
            {
                state.overlays.pinned = !state.overlays.pinned;
            }
        });
    });
    ui.separator();

    for group in OverlayGroup::ALL {
        let open = group_open(state, group);
        let header = egui::CollapsingHeader::new(group.label())
            .id_salt(group.label())
            .open(Some(open))
            .show(ui, |ui| {
                for row in registry::rows_in(group) {
                    draw_row(ui, state, events, row);
                    if row.id == "simulated_stock" {
                        draw_stock_opacity(ui, state);
                    }
                }
                if group == OverlayGroup::Toolpath {
                    ui.label(
                        egui::RichText::new(
                            "Per-toolpath visibility is on each operation row: \
                             eye, C, R, and the bullseye for isolation.",
                        )
                        .small()
                        .color(theme::TEXT_FAINT),
                    );
                }
            });
        if header.header_response.clicked() {
            set_group_open(state, group, !open);
        }
    }

    let legends = registry::active_legends(state);
    if !legends.is_empty() {
        ui.separator();
        ui.label(
            egui::RichText::new("LEGEND")
                .small()
                .strong()
                .color(theme::TEXT_HEADING),
        );
        for legend in legends {
            draw_legend(ui, state, legend);
        }
    }
}

fn group_open(state: &AppState, group: OverlayGroup) -> bool {
    let groups = &state.overlays.groups;
    match group {
        OverlayGroup::Geometry => groups.geometry,
        OverlayGroup::Toolpath => groups.toolpath,
        OverlayGroup::Regions => groups.regions,
        OverlayGroup::Analysis => groups.analysis,
    }
}

fn set_group_open(state: &mut AppState, group: OverlayGroup, open: bool) {
    let groups = &mut state.overlays.groups;
    match group {
        OverlayGroup::Geometry => groups.geometry = open,
        OverlayGroup::Toolpath => groups.toolpath = open,
        OverlayGroup::Regions => groups.regions = open,
        OverlayGroup::Analysis => groups.analysis = open,
    }
}

/// The simulated stock's opacity, a slider rather than a registry row —
/// named as the one gap against the audit's KEEP set. It moved here from
/// `Inspector ▸ View`, where it sat behind a header closed by default.
fn draw_stock_opacity(ui: &mut egui::Ui, state: &mut AppState) {
    let enabled = state.viewport.show_sim_stock && state.simulation.has_results();
    ui.horizontal(|ui| {
        ui.add_space(18.0);
        ui.label(
            egui::RichText::new("Opacity")
                .small()
                .color(theme::TEXT_MUTED),
        );
        ui.add_enabled(
            enabled,
            egui::Slider::new(&mut state.simulation.stock_opacity, 0.0..=1.0).show_value(true),
        )
        .on_hover_text(
            "Affects the simulated stock only \u{2014} the solid stock block and \
             the height planes are pinned at 0.15.",
        );
    });
}

/// One row: a checkbox (or a radio, for a colour choice), its hover, and —
/// when it cannot draw — one reason line plus the compute button the reason
/// names.
fn draw_row(
    ui: &mut egui::Ui,
    state: &mut AppState,
    events: &mut Vec<AppEvent>,
    row: &'static OverlayRow,
) {
    let precondition = (row.precondition)(state);
    let ready = precondition.is_ready();
    let mut on = (row.get)(state);
    let hover = match precondition.reason() {
        Some(reason) => format!("{}\n\n\u{00B7} {reason}", row.hover),
        None => row.hover.to_owned(),
    };

    let response = if row.radio {
        let resp = ui.add_enabled(ready, egui::RadioButton::new(on, row.label));
        if resp.clicked() {
            registry::set_overlay(state, row, true);
        }
        resp
    } else {
        let resp = ui.add_enabled(ready, egui::Checkbox::new(&mut on, row.label));
        if resp.changed() {
            registry::set_overlay(state, row, on);
        }
        resp
    };
    response.on_hover_text(hover);

    if let Some(reason) = precondition.reason() {
        ui.horizontal(|ui| {
            ui.add_space(18.0);
            ui.label(
                egui::RichText::new(format!("\u{00B7} {reason}"))
                    .small()
                    .color(theme::TEXT_FAINT),
            );
        });
        if let Some(action) = precondition.compute() {
            ui.horizontal(|ui| {
                ui.add_space(18.0);
                if ui.small_button(action.label()).clicked() {
                    run_action(state, events, action);
                }
            });
        }
    }
}

/// Run a disabled row's compute affordance.
fn run_action(state: &mut AppState, events: &mut Vec<AppEvent>, action: OverlayAction) {
    match action {
        OverlayAction::RunSimulation => events.push(AppEvent::RunSimulation),
        OverlayAction::RunCollisionCheck => events.push(AppEvent::RunCollisionCheck),
        OverlayAction::OpenPlanner => {
            events.push(AppEvent::Ui(UiCommand::OpenMultitoolPlanner(NoArgs)));
        }
        OverlayAction::GenerateAll => events.push(AppEvent::GenerateAll),
        OverlayAction::RecordGeneratorTrace => {
            events.push(AppEvent::SetGeneratorTraceCaptureAll(true));
            events.push(AppEvent::GenerateAll);
        }
        // The authoring home, not a dial the panel writes itself.
        OverlayAction::OpenRestAnalysis => {
            if let crate::state::selection::Selection::Toolpath(id) = state.selection {
                state.gui.pending_toolpath_tab = Some((id, "geometry".to_owned()));
            }
        }
    }
}

// ── legends ────────────────────────────────────────────────────────────────

/// A horizontal gradient bar plus its end labels.
///
/// `sample` maps `0..=1` along the bar to the colour the 3D overlay paints.
/// Every caller passes the overlay's OWN colour function, so a legend cannot
/// drift from what is drawn.
fn gradient_strip(ui: &mut egui::Ui, left: &str, right: &str, sample: impl Fn(f32) -> [f32; 3]) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(left).small().color(theme::TEXT_MUTED));
        let (rect, _resp) = ui.allocate_exact_size(egui::vec2(96.0, 10.0), egui::Sense::hover());
        let painter = ui.painter();
        const SEGMENTS: u32 = 24;
        for i in 0..SEGMENTS {
            let t0 = i as f32 / SEGMENTS as f32;
            let t1 = (i + 1) as f32 / SEGMENTS as f32;
            let [r, g, b] = sample(t0);
            let seg = egui::Rect::from_min_max(
                egui::pos2(rect.left() + t0 * rect.width(), rect.top()),
                egui::pos2(rect.left() + t1 * rect.width(), rect.bottom()),
            );
            painter.rect_filled(seg, 0.0, rgb(r, g, b));
        }
        ui.label(egui::RichText::new(right).small().color(theme::TEXT_MUTED));
    });
}

fn rgb(r: f32, g: f32, b: f32) -> egui::Color32 {
    egui::Color32::from_rgb(
        (r.clamp(0.0, 1.0) * 255.0) as u8,
        (g.clamp(0.0, 1.0) * 255.0) as u8,
        (b.clamp(0.0, 1.0) * 255.0) as u8,
    )
}

/// A row of discrete labelled swatches, for a legend whose scale is a set of
/// classes rather than a continuum.
fn swatch_row(ui: &mut egui::Ui, title: &str, swatches: &[(&str, [f32; 3])]) {
    ui.label(egui::RichText::new(title).small().color(theme::TEXT_MUTED));
    ui.horizontal_wrapped(|ui| {
        for (label, [r, g, b]) in swatches {
            let (rect, resp) = ui.allocate_exact_size(egui::vec2(12.0, 10.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 0.0, rgb(*r, *g, *b));
            resp.on_hover_text(*label);
            ui.label(egui::RichText::new(*label).small().color(theme::TEXT_FAINT));
        }
    });
}

fn draw_legend(ui: &mut egui::Ui, state: &AppState, legend: Legend) {
    match legend {
        Legend::RestHeatmap(threshold, peak) => {
            use rs_cam_core::rest_heatmap_mesh::rest_ramp_color;
            let threshold = threshold as f32;
            let peak = peak.max(threshold + 1e-6);
            gradient_strip(
                ui,
                &format!("Rest {threshold:.2} mm"),
                &format!("{peak:.2} mm"),
                |t| rest_ramp_color(threshold + t * (peak - threshold), threshold, peak),
            );
        }
        Legend::Reach(ramp) => {
            use rs_cam_core::reach_map::reach_color;
            // The strip draws the DEPTH band only — the bar to the deepest
            // gap, on the ramp's own log scale — because that is the part
            // with structure in it. Green and grey are single colours and
            // are named on the line under it rather than given ramp width.
            let bar = ramp.bar_mm();
            let top = ramp.max_gap_mm.max(bar);
            let ratio = (top / bar).max(1.0);
            gradient_strip(
                ui,
                &format!("miss {bar:.3} mm"),
                &format!("{top:.2} mm"),
                move |t| {
                    // Inverse of `ReachRamp::depth_t`, nudged past the bar so
                    // t = 0 samples the first MISS colour and not the green.
                    let gap = bar * ratio.powf(f64::from(t)) * 1.001;
                    reach_color(gap as f32, f32::NAN, ramp)
                },
            );
            ui.label(
                egui::RichText::new(format!(
                    "green \u{2264} {:.3} mm \u{00B7} grey = unresolved \u{00B7} mid {:.2} mm \u{00B7} log scale",
                    ramp.tolerance_mm,
                    ramp.mid_stop_mm(),
                ))
                .small()
                .color(theme::TEXT_FAINT),
            );
            // P5.3 - the moves are dimmed at DRAW TIME while the shading is
            // on, so the row still reads ON in the panel above. Said here,
            // because an operator seeing Cutting moves ticked and faint lines
            // on screen would otherwise be looking at a contradiction.
            if state.viewport.show_cutting || state.viewport.show_rapids {
                ui.label(
                    egui::RichText::new("moves dimmed while reach map is on")
                        .small()
                        .color(theme::TEXT_FAINT),
                );
            }
            if let Some(map) = state.gui.reach_overlay.ready_map() {
                let measured = map.is_measured();
                ui.label(
                    egui::RichText::new(if measured {
                        format!(
                            "unreachable {:.1}% {} \u{00B7} worst gap {:.3} mm",
                            map.unreachable_pct(),
                            map.area_basis_note(),
                            map.max_gap_mm
                        )
                    } else {
                        "reach: not measured".to_owned()
                    })
                    .small()
                    .color(theme::TEXT_FAINT),
                );
                // The grid, always, and on its own line: two percentages are
                // comparable only on one grid (F5, 2026-09-08). It also
                // carries the "the bar is under the floor" sentence, which is
                // what the wanaka red terrain needed said.
                if measured {
                    let below = map.tolerance_below_floor();
                    ui.label(
                        egui::RichText::new(map.grid_note())
                            .small()
                            .color(if below {
                                theme::WARNING
                            } else {
                                theme::TEXT_FAINT
                            }),
                    );
                    if below {
                        ui.label(
                            egui::RichText::new(map.over_statement_note())
                                .small()
                                .color(theme::WARNING),
                        );
                    }
                }
            }
        }
        Legend::Deviation => {
            use crate::render::sim_render::deviation_colors;
            gradient_strip(ui, "Over-cut \u{2212}1 mm", "+1 mm remaining", |t| {
                let mm = -1.0 + 2.0 * t;
                deviation_colors(&[mm]).first().copied().unwrap_or([0.0; 3])
            });
        }
        Legend::ByHeight => {
            use rs_cam_core::stock_mesh::height_gradient_colors;
            // The function normalises over the vertices it is handed, so one
            // synthetic ramp of Z values reproduces the mesh's own scale.
            let mut vertices = Vec::with_capacity(3 * 25);
            for i in 0..25 {
                vertices.extend_from_slice(&[0.0, 0.0, i as f32 / 24.0]);
            }
            let colors = height_gradient_colors(&vertices);
            gradient_strip(ui, "Height low", "high", move |t| {
                let index = ((t * 24.0).round() as usize).min(24);
                colors.get(index).copied().unwrap_or([0.0; 3])
            });
        }
        Legend::Engagement => {
            use crate::render::toolpath_render::engagement_color;
            gradient_strip(ui, "Load heavy", "light", |t| {
                engagement_color(f64::from(t) * 1.5, 1.0)
            });
        }
        Legend::AdvancePerTooth => {
            use crate::render::toolpath_render::advance_per_tooth_segment_color;
            use rs_cam_core::feeds::{AdvancePerToothMm, VendorChiploadBand};
            // The band is per matched vendor row, so the legend labels the
            // CLASSES rather than absolute mm/tooth — and it reads them out
            // of the same classifier the lines use. A probe band supplies the
            // class boundaries; the RGB comes from the shipped function.
            let band = VendorChiploadBand::from_advance_range(&(0.05..0.10));
            let at = |mm: f64| {
                advance_per_tooth_segment_color(Some(&band), Some(AdvancePerToothMm::new(mm)))
            };
            swatch_row(
                ui,
                "Advance / tooth vs the matched vendor band",
                &[
                    ("below", at(0.01)),
                    ("within", at(0.075)),
                    ("near max", at(0.099)),
                    ("above", at(0.2)),
                    (
                        "no band",
                        advance_per_tooth_segment_color(None, Some(AdvancePerToothMm::new(0.05))),
                    ),
                ],
            );
        }
        Legend::TierMap(tier_count) => {
            use rs_cam_core::rest_heatmap_mesh::tier_fill_color;
            ui.label(
                egui::RichText::new("Tier map \u{2014} one colour per tool tier")
                    .small()
                    .color(theme::TEXT_MUTED),
            );
            ui.horizontal_wrapped(|ui| {
                // Tier 0 has no colour: it is the coarse tool's complement
                // and the overlay deliberately draws nothing there.
                for tier in 1..=tier_count {
                    let Ok(tier) = u8::try_from(tier) else {
                        break;
                    };
                    let Some([r, g, b]) = tier_fill_color(tier) else {
                        continue;
                    };
                    let (rect, resp) =
                        ui.allocate_exact_size(egui::vec2(12.0, 10.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 0.0, rgb(r, g, b));
                    resp.on_hover_text(format!("tier {tier}"));
                }
            });
        }
    }
}

/// The `Overlays (n)` toolbar button. Returns nothing: it toggles the panel
/// directly, because the panel's open state is not undoable UI history.
pub fn toolbar_button(ui: &mut egui::Ui, state: &mut AppState) -> egui::Response {
    let count = registry::non_default_count(state);
    let label = if count == 0 {
        "Overlays".to_owned()
    } else {
        format!("Overlays ({count})")
    };
    let response = ui.small_button(label);
    if response.clicked() {
        state.overlays.open = !state.overlays.open;
    }
    response.clone().on_hover_text(
        "Every viewport overlay, grouped, with a reason on anything that \
         cannot draw (shortcut: O; Shift+O pins it).",
    );
    response
}

/// Which surface the `,` and `.` shortcuts step (UX §6.8).
pub const COMMA_SURFACE: OverlaySurface = OverlaySurface::Model;
pub const PERIOD_SURFACE: OverlaySurface = OverlaySurface::Stock;
