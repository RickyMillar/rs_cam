use super::AppEvent;
use crate::compute::LaneSnapshot;
use crate::render::camera::{ProjectionMode, ViewPreset};
use crate::state::Workspace;
use crate::state::viewport::{RenderMode, ToolpathColorMode, ViewportState};
use crate::ui::automation;
use crate::ui::theme;

// SAFETY: viewport overlay needs the full UI context (workspace, sim flag,
// projection, isolation label, mutable viewport state, lane snapshots, and
// event sink). Bundling them into a struct would just rename the same data
// for one call site.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    ui: &mut egui::Ui,
    workspace: Workspace,
    _sim_active: bool,
    projection: ProjectionMode,
    isolated_name: Option<&str>,
    viewport: &mut ViewportState,
    lanes: &[LaneSnapshot; 3],
    events: &mut Vec<AppEvent>,
    // Rest-depth heatmap legend info for the selected toolpath:
    // `(threshold_mm, peak_rest_mm)`. `None` when nothing's selected or the
    // selection carries no `rest_grid` (only the pencil rest-depth detector
    // populates one) — used both to grey out the "Rest heatmap" checkbox and
    // to draw the gradient legend when the overlay is actually showing.
    selected_rest_grid_info: Option<(f64, f32)>,
) {
    let has_rest_grid = selected_rest_grid_info.is_some();
    ui.horizontal_wrapped(|ui| {
        // ── View dropdown: presets + reset ──────────────────────
        ui.menu_button("View ▼", |ui| {
            if ui.button("Top").clicked() {
                events.push(AppEvent::SetViewPreset(ViewPreset::Top));
                ui.close();
            }
            if ui.button("Front").clicked() {
                events.push(AppEvent::SetViewPreset(ViewPreset::Front));
                ui.close();
            }
            if ui.button("Right").clicked() {
                events.push(AppEvent::SetViewPreset(ViewPreset::Right));
                ui.close();
            }
            if ui.button("Iso").clicked() {
                events.push(AppEvent::SetViewPreset(ViewPreset::Isometric));
                ui.close();
            }
            ui.separator();
            if ui.button("Reset view").clicked() {
                events.push(AppEvent::ResetView);
                ui.close();
            }
        });

        // ── Render mode: Shaded / Wire ──────────────────────────
        let shade_label = match viewport.render_mode {
            RenderMode::Shaded => "Shaded ▼",
            RenderMode::Wireframe => "Wire ▼",
        };
        ui.menu_button(shade_label, |ui| {
            if ui
                .selectable_label(viewport.render_mode == RenderMode::Shaded, "Shaded")
                .clicked()
            {
                viewport.render_mode = RenderMode::Shaded;
                ui.close();
            }
            if ui
                .selectable_label(viewport.render_mode == RenderMode::Wireframe, "Wireframe")
                .clicked()
            {
                viewport.render_mode = RenderMode::Wireframe;
                ui.close();
            }
        });

        // ── Projection: Persp / Ortho ───────────────────────────
        let proj_label = match projection {
            ProjectionMode::Perspective => "Persp ▼",
            ProjectionMode::Orthographic => "Ortho ▼",
        };
        ui.menu_button(proj_label, |ui| {
            if ui
                .selectable_label(
                    matches!(projection, ProjectionMode::Perspective),
                    "Perspective",
                )
                .clicked()
            {
                if !matches!(projection, ProjectionMode::Perspective) {
                    events.push(AppEvent::ToggleProjection);
                }
                ui.close();
            }
            if ui
                .selectable_label(
                    matches!(projection, ProjectionMode::Orthographic),
                    "Orthographic",
                )
                .clicked()
            {
                if !matches!(projection, ProjectionMode::Orthographic) {
                    events.push(AppEvent::ToggleProjection);
                }
                ui.close();
            }
        });

        // ── Show dropdown: all visibility toggles in one popover ─
        let show_menu = ui.menu_button("Show ▼", |ui| {
            ui.set_min_width(180.0);
            ui.checkbox(&mut viewport.show_grid, "Grid");
            ui.checkbox(&mut viewport.show_stock, "Stock");
            ui.checkbox(&mut viewport.show_fixtures, "Fixtures");
            ui.checkbox(&mut viewport.show_polygons, "Curves (DXF/SVG)");
            ui.separator();
            ui.checkbox(&mut viewport.show_cutting, "Paths (cutting)");
            ui.checkbox(&mut viewport.show_rapids, "Rapids");
            ui.checkbox(&mut viewport.show_collisions, "Collisions");

            // SpanKind filter — hides cut segments by their innermost
            // SpanKind (Entry / LeadOut / LinkBridge / DressupArtifact).
            // Only takes effect in Palette color mode; the Engagement /
            // Chipload modes don't read spans.
            let f = &mut viewport.span_kind_filter;
            let any_hidden = !f.all_visible();
            ui.menu_button(
                if any_hidden {
                    "By SpanKind ▾ (some hidden)"
                } else {
                    "By SpanKind ▾"
                },
                |ui| {
                    ui.set_min_width(180.0);
                    ui.checkbox(&mut f.show_entry, "Entry")
                        .on_hover_text("Plunge / ramp / helix lead-in segments");
                    ui.checkbox(&mut f.show_lead_out, "LeadOut")
                        .on_hover_text("Lead-out / retract transition segments");
                    ui.checkbox(&mut f.show_link_bridge, "LinkBridge")
                        .on_hover_text("Linker bridges inserted between regions");
                    ui.checkbox(&mut f.show_dressup, "DressupArtifact")
                        .on_hover_text(
                            "Dogbones, arc-fit replacements, other dressup-introduced segments",
                        );
                    if any_hidden && ui.button("Reset").clicked() {
                        *f = crate::state::viewport::SpanKindFilter::default();
                        ui.close();
                    }
                },
            );

            ui.separator();
            // Only meaningful when the selected toolpath actually carries a
            // `rest_grid` (populated solely by the pencil rest-depth
            // detector) — grey the checkbox out otherwise, same pattern as
            // other conditionally-meaningful entries in this menu.
            ui.add_enabled(
                has_rest_grid,
                egui::Checkbox::new(&mut viewport.show_rest_heatmap, "Rest heatmap"),
            )
            .on_hover_text(if has_rest_grid {
                "Rest-depth heatmap from the pencil rest-depth detector (detector #4)"
            } else {
                "Select a pencil rest-depth toolpath to enable"
            });

            ui.separator();
            ui.checkbox(&mut viewport.show_tool_profile_preview, "Tool-profile ghost");
            ui.horizontal(|ui| {
                ui.label("Toolpath color:");
                egui::ComboBox::from_id_salt("toolpath_color_mode")
                    .selected_text(match viewport.toolpath_color_mode {
                        ToolpathColorMode::Normal => "Palette",
                        ToolpathColorMode::Engagement => "Engagement",
                        ToolpathColorMode::Chipload => "Chipload",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut viewport.toolpath_color_mode,
                            ToolpathColorMode::Normal,
                            "Palette",
                        )
                        .on_hover_text("Per-toolpath palette color with Z-depth blending");
                        ui.selectable_value(
                            &mut viewport.toolpath_color_mode,
                            ToolpathColorMode::Engagement,
                            "Engagement",
                        )
                        .on_hover_text(
                            "Color cutting moves by feed rate: green→yellow→red for light→heavy load",
                        );
                        ui.selectable_value(
                            &mut viewport.toolpath_color_mode,
                            ToolpathColorMode::Chipload,
                            "Chipload",
                        )
                        .on_hover_text(
                            "Color each segment by per-sample chipload vs the matched vendor row's window",
                        );
                    });
            });
        });
        // Record the Show ▼ button so the automation harness can locate the
        // collision-toggle entry point without expanding the popover.
        automation::record(
            ui,
            "overlay_collision_check",
            &show_menu.response,
            "Show ▼ (collisions)",
        );

        // ── Isolate button ──────────────────────────────────────
        if let Some(name) = isolated_name {
            // Active state: show the isolated toolpath name + clear button
            ui.label(
                egui::RichText::new(format!("\u{25CE} {}", name))
                    .color(theme::WARNING)
                    .strong(),
            )
            .on_hover_text("Currently showing only this toolpath. Click ✕ to clear.");
            if ui
                .small_button("✕")
                .on_hover_text("Clear isolation (show all toolpaths)")
                .clicked()
            {
                events.push(AppEvent::ClearIsolation);
            }
        } else if ui
            .small_button("Isolate")
            .on_hover_text("Show only the selected toolpath (shortcut: I)")
            .clicked()
        {
            events.push(AppEvent::ToggleIsolateToolpath);
        }

        // ── Compute activity indicator (right side) ─────────────
        let active_lanes: Vec<_> = lanes.iter().filter(|lane| lane.is_active()).collect();
        if !active_lanes.is_empty() {
            ui.separator();
            let label = active_lanes
                .iter()
                .map(|lane| {
                    lane.current_job
                        .clone()
                        .unwrap_or_else(|| "Working".to_owned())
                })
                .collect::<Vec<_>>()
                .join(" | ");
            ui.label(egui::RichText::new(label).color(theme::WARNING));
            let cancel_resp = ui.small_button("Cancel All");
            automation::record(ui, "overlay_cancel_all", &cancel_resp, "Cancel All");
            if cancel_resp.clicked() {
                events.push(AppEvent::CancelCompute);
            }
        }

        // ── Workspace-specific actions (right-aligned) ──────────
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| match workspace {
            // Readiness has no viewport, so this overlay never renders there.
            Workspace::Setup | Workspace::Readiness => {}
            Workspace::Toolpaths => {
                if ui.small_button("Generate All").clicked() {
                    events.push(AppEvent::GenerateAll);
                }
            }
            Workspace::Simulation => {
                if ui.small_button("Reset").clicked() {
                    events.push(AppEvent::ResetSimulation);
                }
                if ui.small_button("Re-run").clicked() {
                    events.push(AppEvent::RunSimulation);
                }
            }
        });
    });

    // ── Rest heatmap legend ──────────────────────────────────────
    // Only drawn while the overlay is actually visible: checkbox on AND a
    // rest_grid present on the selection.
    if viewport.show_rest_heatmap
        && let Some((threshold, peak)) = selected_rest_grid_info
    {
        draw_rest_heatmap_legend(ui, threshold, peak);
    }
}

/// Compact horizontal gradient bar for the rest-depth heatmap overlay,
/// labeled with the mask threshold on the left and the grid's peak rest
/// depth on the right. Reuses `rs_cam_core::rest_heatmap_mesh::rest_ramp_color`
/// — the exact function that colors the 3D overlay mesh — rather than a
/// hand-rolled copy, so the legend can never drift out of sync with what's
/// actually drawn.
///
/// `peak` stands in for the mesh's true p95-of-above-threshold normalization
/// value: recomputing a percentile GUI-side isn't cheap (would mean walking
/// every cell of the grid each frame just for a label), and the grid's max is
/// already read here for free. The legend's rightmost stop may therefore
/// look slightly less saturated than the true hottest cell in the 3D view
/// when a single outlier cell sits well above the true p95 — a documented
/// simplification, not a threshold disagreement (the left edge, the
/// mask/region boundary, is exact).
fn draw_rest_heatmap_legend(ui: &mut egui::Ui, threshold: f64, peak: f32) {
    use rs_cam_core::rest_heatmap_mesh::rest_ramp_color;

    let threshold_f32 = threshold as f32;
    let peak = peak.max(threshold_f32 + 1e-6);

    ui.horizontal(|ui| {
        ui.label(format!("Rest heatmap  {threshold:.2} mm"));
        let (rect, _resp) = ui.allocate_exact_size(egui::vec2(120.0, 12.0), egui::Sense::hover());
        let painter = ui.painter();
        const SEGMENTS: u32 = 24;
        for i in 0..SEGMENTS {
            let t0 = f64::from(i) / f64::from(SEGMENTS);
            let t1 = f64::from(i + 1) / f64::from(SEGMENTS);
            let rest0 = threshold_f32 + t0 as f32 * (peak - threshold_f32);
            let [r, g, b] = rest_ramp_color(rest0, threshold_f32, peak);
            let x0 = rect.left() + t0 as f32 * rect.width();
            let x1 = rect.left() + t1 as f32 * rect.width();
            let seg =
                egui::Rect::from_min_max(egui::pos2(x0, rect.top()), egui::pos2(x1, rect.bottom()));
            painter.rect_filled(
                seg,
                0.0,
                egui::Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8),
            );
        }
        ui.label(format!("{peak:.2} mm"));
    });
}
