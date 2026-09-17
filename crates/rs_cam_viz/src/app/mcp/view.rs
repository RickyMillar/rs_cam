//! The MCP view surface: the reach map, the two screenshot routes and
//! `set_ui_view`, the one door that writes what the operator sees.
//!
//! Split out of `app/mcp.rs` (P4). Every item is a verbatim move; the
//! handlers stay reachable as `RsCamApp::mcp_*` because an inherent
//! `impl` compiles in any module of the crate.

use std::path::Path;

use rs_cam_mcp::server::{json_str, text};

use crate::app::RsCamApp;
use crate::mcp_bridge::McpResponse;
use crate::state::Workspace;
use crate::state::selection::Selection;
use crate::ui::AppEvent;
use crate::ui_command::{NoArgs, UiCommand};

use super::simulation::sim_mesh_in_world_frame;
use super::{ScreenshotToolpathOptions, parse_workspace, workspace_key};

impl RsCamApp {
    /// The model surface as a render background, shaded by the reach map
    /// for `index`'s tool.
    ///
    /// The mesh is the one the map was built on — the SETUP-TRANSFORMED
    /// mesh, which is also the frame the toolpath was emitted in, so the
    /// two composite layers register.
    fn reach_overlay_background(
        &self,
        index: usize,
    ) -> Result<rs_cam_core::stock::stock_mesh::StockMesh, String> {
        let session = &self.controller.state().session;
        let Some(spec) = session.reach_map_spec(index, None) else {
            return Err(format!(
                "Toolpath {index} has no reach map. A reach map answers for a FINISHING \
                 operation on a 3D mesh (drop_cutter, waterline, pencil, scallop, \
                 unified_finish, steep_shallow, ramp_finish, spiral_finish, radial_finish, \
                 horizontal_finish) that has both a model mesh and a tool."
            ));
        };
        // Fresh flag, never armed: this call is not on the generate lane,
        // so `cancel_generation` is not its owner.
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let cancel_fn = || cancel.load(std::sync::atomic::Ordering::SeqCst);
        let map = rs_cam_core::maps::reach_map_cache::cached_reach_map(&spec, &cancel_fn)
            .map_err(|e| format!("Reach map for toolpath {index} could not be built — {e}"))?;
        let gaps = map.vertex_gaps(spec.mesh.as_ref(), spec.index.as_ref());
        let floors = map.vertex_floors(spec.mesh.as_ref());
        Ok(rs_cam_core::maps::reach_map::reach_overlay_stock_mesh(
            spec.mesh.as_ref(),
            &gaps,
            &floors,
            map.ramp(),
        ))
    }

    /// P5 — the reach map as numbers.
    ///
    /// A pure READ: it builds (or fetches) the map and reports it. Nothing
    /// is emitted, no parameter moves, no result is invalidated — the reply
    /// says `modified: false` and that is a statement about this function,
    /// not a hope. It takes `&self` so that stays true by type.
    pub(super) fn mcp_reach_map(&self, spec: &rs_cam_mcp::server::ReachMapParam) -> String {
        let index = spec.index;
        let session = &self.controller.state().session;
        let Some(request) = session.reach_map_spec(index, spec.tolerance_mm) else {
            return json_str(serde_json::json!({
                "ok": false,
                "modified": false,
                "error": format!(
                    "reach_map: toolpath {index} has no reach map. A reach map answers for a \
                     FINISHING operation on a 3D mesh that has both a model mesh and a tool; \
                     a roughing pass, a 2D operation and a drill have no reach question."
                ),
            }));
        };
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let cancel_fn = || cancel.load(std::sync::atomic::Ordering::SeqCst);
        let map = match rs_cam_core::maps::reach_map_cache::cached_reach_map(&request, &cancel_fn) {
            Ok(map) => map,
            Err(e) => {
                return json_str(serde_json::json!({
                    "ok": false,
                    "modified": false,
                    "error": format!("reach_map: the walk could not finish — {e}"),
                }));
            }
        };
        let bins = spec.histogram_bins.unwrap_or(8).clamp(1, 64);
        let (edges, counts, not_measured) = map.gap_histogram(bins);
        json_str(serde_json::json!({
            "ok": true,
            // Plan-time read: this handler takes `&self`.
            "modified": false,
            // FIRST LINE, deliberately: the grid the answer sits on. Two
            // percentages from two calls are comparable only on one grid, and
            // an agent that reads the number before the grid cannot know
            // (F5, 2026-09-08).
            "grid_note": map.grid_note(),
            // P5.2: the base, on the wire, because a comparison against any
            // other instrument is meaningless until both sides share it.
            "area_basis": "true 3D surface area (each triangle by its own \
                           area, never its XY footprint), over the rim-eroded \
                           measured population only \u{2014} both numerator \
                           and denominator",
            "toolpath_index": index,
            "tool_id": map.tool_id,
            "model_id": map.model_id,
            "tolerance_mm": map.tolerance_mm,
            "tolerance_source": request.tolerance_source.key(),
            "tolerance_note": request.tolerance_source.describe(map.tolerance_mm),
            "is_measured": map.is_measured(),
            "unreachable_pct_of_measured_area": map.unreachable_pct(),
            "unresolved_pct_of_measured_area": map.unresolved_pct(),
            "tolerance_below_floor": map.tolerance_below_floor(),
            "max_gap_mm": map.max_gap_mm,
            "area_mm2": {
                "surface": map.surface_area_mm2,
                "measured": map.measured_area_mm2,
                "unreachable": map.unreachable_area_mm2,
                "unresolved": map.unresolved_area_mm2,
            },
            "grid": {
                "cell_mm": map.cell_mm,
                "nx": map.nx,
                "ny": map.ny,
                "cells": map.cells.len(),
                "not_measured_cells": not_measured,
                "cell_rule": "the TOOL's tip sphere and the model bbox, never the tolerance \
                              \u{2014} so a series of probes at moving tolerances is comparable",
            },
            "gap_histogram": {
                "upper_edges_mm": edges,
                "counts": counts,
            },
            "discretisation_floor_mm": map.discretisation_floor_mm,
            "profile_floor_mm": map.profile_floor_mm,
            "curvature_floor_p95_mm": map.curvature_floor_p95_mm,
            "rim_erosion_mm": map.rim_erosion_mm,
            "note": "Percentages are 3D-SURFACE-AREA weighted over the rim-eroded \
                     measured population (see `area_basis`); putting a planar or \
                     whole-board figure beside one of these compares two different \
                     questions \u{2014} on a 1.34 mean-sec-theta terrain that alone is \
                     5 points. Top-down measure. Undersides, walls and a band one envelope radius wide \
                     inside the part outline are NOT MEASURED \u{2014} read `is_measured` and \
                     the measured-against-surface areas before believing the percentage. \
                     `discretisation_floor_mm` is what this grid can resolve ON THIS SURFACE: \
                     the larger of the plane-only `profile_floor_mm` and the curvature term \
                     `curvature_floor_p95_mm`. When `tolerance_below_floor` is true the bar is \
                     under the arithmetic. The grid's gap bias is NON-NEGATIVE \u{2014} a \
                     minimum over a sampled CL set sits at or above the continuum minimum \
                     \u{2014} so `unreachable_pct_of_measured_area` OVER-states: the true \
                     unreachable share is AT OR BELOW it, and \
                     `unresolved_pct_of_measured_area` is the band the grid cannot classify \
                     either way. `reached` is the sound side: a cell called reached really is \
                     formed. Raise the tolerance (the operation's own cusp is the honest bar) \
                     rather than reading the residual as tool geometry.",
        }))
    }

    pub(super) fn mcp_screenshot_toolpath(
        &self,
        index: usize,
        path: &str,
        options: ScreenshotToolpathOptions,
    ) -> String {
        let ScreenshotToolpathOptions {
            width,
            height,
            show_stock,
            include_rapids,
            reach_overlay,
        } = options;
        // Find the toolpath result from GUI runtime
        let session = &self.controller.state().session;
        let gui = &self.controller.state().gui;
        let Some(tc) = session.toolpath_configs().get(index) else {
            return text(format!("Toolpath {index} not found."));
        };

        let result = gui
            .toolpath_rt
            .get(&tc.id)
            .and_then(|rt| rt.result.as_ref());
        let Some(result) = result else {
            return text(format!(
                "Toolpath {index} not generated. Run generate_toolpath first."
            ));
        };

        if path.ends_with(".png") {
            let w = width.unwrap_or(1200);
            let h = height.unwrap_or(800);
            // The two backgrounds are mutually exclusive and the reach map
            // wins: they answer different questions (what the machine has
            // removed against what this tool can form), and compositing one
            // over the other would leave the reader unable to say which
            // colour they were looking at.
            // F4: when the reach shading is the subject, the moves must not
            // bury it. See `CompositeSubject` for the measurement.
            let mut subject = rs_cam_core::export::fingerprint::CompositeSubject::Moves;
            let bg = if reach_overlay.unwrap_or(false) {
                match self.reach_overlay_background(index) {
                    Ok(mesh) => {
                        subject = rs_cam_core::export::fingerprint::CompositeSubject::Background;
                        Some(mesh)
                    }
                    Err(message) => return text(message),
                }
            } else if show_stock.unwrap_or(false) {
                self.controller
                    .state()
                    .simulation
                    .results
                    .as_ref()
                    .map(|sim| {
                        let mut m = sim_mesh_in_world_frame(&sim.mesh, session);
                        m.apply_height_gradient();
                        m
                    })
            } else {
                None
            };
            let pixels = rs_cam_core::export::fingerprint::render_toolpath_composite_subject(
                &result.annotated,
                bg.as_ref(),
                None,
                w,
                h,
                include_rapids.unwrap_or(true),
                subject,
            );
            let layer_note = match subject {
                rs_cam_core::export::fingerprint::CompositeSubject::Background => {
                    " \u{2014} reach shading at full brightness, moves drawn thin and \
                     dimmed so the shading reads from above"
                }
                rs_cam_core::export::fingerprint::CompositeSubject::Moves => "",
            };
            match image::save_buffer(Path::new(path), &pixels, w, h, image::ColorType::Rgba8) {
                Ok(()) => text(format!(
                    "Toolpath {index} exported to {path} ({w}x{h}, {} moves, \
                     {:.0}mm cutting){layer_note}",
                    result.toolpath().moves.len(),
                    result.stats.cutting_distance,
                )),
                Err(e) => text(format!("Failed to save PNG: {e}")),
            }
        } else {
            let bbox = session.stock_bbox();
            let bounds = [
                bbox.min.x, bbox.min.y, bbox.min.z, bbox.max.x, bbox.max.y, bbox.max.z,
            ];
            let html = rs_cam_core::export::viz::toolpath_standalone_3d_html(
                result.toolpath(),
                Some(bounds),
            );

            match std::fs::write(path, &html) {
                Ok(()) => text(format!(
                    "Toolpath view exported to {path} ({} moves, {:.0}mm cutting)",
                    result.toolpath().moves.len(),
                    result.stats.cutting_distance,
                )),
                Err(e) => text(format!("Failed to write: {e}")),
            }
        }
    }

    /// Start a full-window GUI capture. Deferred-response pattern
    /// (mirrors `pending.collision`): store the path + response sender
    /// in `pending.gui_screenshot`, optionally resize the window, and
    /// let the per-frame pump issue `ViewportCommand::Screenshot` once
    /// the resize has settled. `complete_mcp_gui_screenshot` finishes
    /// the response when the `egui::Event::Screenshot` result arrives
    /// 1-2 frames later.
    pub(super) fn mcp_screenshot_gui(
        &mut self,
        ctx: &egui::Context,
        path: &str,
        width: Option<f32>,
        height: Option<f32>,
        response_tx: tokio::sync::oneshot::Sender<McpResponse>,
    ) {
        if !path.ends_with(".png") {
            let _ = response_tx.send(McpResponse {
                result: Ok(text(format!(
                    "screenshot_gui only writes PNG — path must end in .png (got '{path}')"
                ))),
            });
            return;
        }
        let Some(pending) = self.controller.pending_mcp.as_mut() else {
            let _ = response_tx.send(McpResponse {
                result: Err("MCP compute tracking not initialized".to_owned()),
            });
            return;
        };
        if pending.gui_screenshot.is_some() {
            let _ = response_tx.send(McpResponse {
                result: Ok(text(
                    "Another screenshot_gui capture is already in flight — \
                     retry after it completes.",
                )),
            });
            return;
        }

        // Optional resize before capture (logical points). The capture is
        // deferred a few frames so the window system has applied the new
        // size. The size is NOT restored afterwards — it sticks (documented
        // in the tool description).
        //
        // Even without a resize, captures settle 2 frames: view mutations
        // queued by a preceding set_ui_view (one-shot tab overrides, modal
        // opens routed through the event queue) take an event-processing
        // frame plus a render frame to reach pixels. Capturing at 0 raced
        // that pipeline — the 2026-06-11 sweep needed a throwaway "flush
        // shot" per tab change (Batch 3 item 13).
        let frames_before_capture = if width.is_some() || height.is_some() {
            let current = ctx.input(|i| i.viewport().inner_rect).map(|r| r.size());
            let w = width.unwrap_or_else(|| current.map_or(1400.0, |s| s.x));
            let h = height.unwrap_or_else(|| current.map_or(900.0, |s| s.y));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w, h)));
            3
        } else {
            2
        };

        pending.gui_screenshot = Some(crate::mcp_bridge::PendingGuiScreenshot {
            path: path.to_owned(),
            frames_before_capture,
            capture_requested: false,
            // M-4's refusal clock starts here, not at the last frame — see
            // `PendingGuiScreenshot::park_refusal` for why an idle window must
            // not be refused.
            requested_at: std::time::Instant::now(),
            response_tx,
        });
        // Keep frames pumping while the app is headless-idle so the
        // capture actually happens.
        ctx.request_repaint();
    }

    /// Per-frame pump for an in-flight `screenshot_gui` capture. Issues
    /// the `ViewportCommand::Screenshot` once the resize-settle countdown
    /// drains, and keeps requesting repaints so frames pump while idle.
    pub(crate) fn pump_mcp_gui_screenshot(&mut self, ctx: &egui::Context) {
        // Checkpoint M-4. Read the refusal BEFORE arming the capture: arming it
        // on a loop that will never paint is exactly the hang this replaces.
        let refusal = {
            let Some(pending) = self
                .controller
                .pending_mcp
                .as_ref()
                .and_then(|p| p.gui_screenshot.as_ref())
            else {
                return;
            };
            pending.park_refusal(self.mcp_reads.frame_loop())
        };
        if let Some(reason) = refusal {
            // `take` the slot rather than leaving it armed — a refused request
            // is finished, and a stale slot would reject the caller's next
            // attempt with "another capture is already in flight".
            if let Some(slot) = self
                .controller
                .pending_mcp
                .as_mut()
                .and_then(|p| p.gui_screenshot.take())
            {
                tracing::warn!("{reason}");
                let _ = slot.response_tx.send(McpResponse {
                    result: Ok(text(reason)),
                });
            }
            return;
        }

        let Some(pending) = self
            .controller
            .pending_mcp
            .as_mut()
            .and_then(|p| p.gui_screenshot.as_mut())
        else {
            return;
        };
        if pending.should_capture_now() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        ctx.request_repaint();
    }

    /// Complete a pending `screenshot_gui` request with a captured frame.
    /// Returns `true` when an MCP capture consumed the screenshot event
    /// (suppressing the default F12 save-to-cwd path).
    pub(crate) fn complete_mcp_gui_screenshot(&mut self, image: &egui::ColorImage) -> bool {
        let Some(pending) = self.controller.pending_mcp.as_mut() else {
            return false;
        };
        // Only consume the event once this slot has actually issued its
        // Screenshot command (an F12 capture could land first otherwise).
        if !pending
            .gui_screenshot
            .as_ref()
            .is_some_and(|p| p.capture_requested)
        {
            return false;
        }
        let Some(slot) = pending.gui_screenshot.take() else {
            return false;
        };

        let (w, h) = (image.size[0] as u32, image.size[1] as u32);
        let pixels: Vec<u8> = image
            .pixels
            .iter()
            .flat_map(|c| [c.r(), c.g(), c.b(), c.a()])
            .collect();
        let result = match image::save_buffer(
            Path::new(&slot.path),
            &pixels,
            w,
            h,
            image::ColorType::Rgba8,
        ) {
            Ok(()) => text(format!("GUI window exported to {} ({w}x{h})", slot.path)),
            Err(e) => text(format!("Failed to save PNG: {e}")),
        };
        let _ = slot.response_tx.send(McpResponse { result: Ok(result) });
        true
    }

    /// Apply a `set_ui_view` navigation request. Mutations route through
    /// the same `AppEvent`s the GUI's own widgets push, so workspace
    /// switches and modal opens behave identically to user clicks (they
    /// land later this same frame via `handle_events`). Returns a JSON
    /// echo of the resulting view state.
    pub(super) fn mcp_set_ui_view(
        &mut self,
        workspace: Option<&str>,
        toolpath_index: Option<usize>,
        properties_tab: Option<&str>,
        select: Option<&str>,
        modal: Option<&str>,
        overlays: Option<&std::collections::BTreeMap<String, bool>>,
    ) -> String {
        // 1. Workspace.
        //
        // Applied SYNCHRONOUSLY, not through the event queue. A workspace
        // carries per-workspace overlay defaults (P6), and the event lands
        // later in this frame — so an `overlays` request in the same call
        // would be written first and then clobbered by the arriving
        // defaults, and every precondition below would have answered about
        // the OLD workspace. `UiCommand::SwitchWorkspace`'s handler calls the
        // same function, so this is the identical mutation, in the right
        // order.
        let mut workspace_applied: Option<&'static str> = None;
        if let Some(ws) = workspace {
            let Some(target) = parse_workspace(ws) else {
                return json_str(serde_json::json!({
                    "error": format!(
                        "Unknown workspace '{ws}'. Valid: setup, toolpaths, simulation, readiness"
                    )
                }));
            };
            crate::ui::overlays::registry::switch_workspace(self.controller.state_mut(), target);
            workspace_applied = Some(workspace_key(target));
        }

        // 2. Toolpath selection (0-based index -> semantic id).
        if let Some(idx) = toolpath_index {
            let Some(tp_id) = self
                .controller
                .state()
                .session
                .toolpath_configs()
                .get(idx)
                .map(|tc| tc.id)
            else {
                let count = self.controller.state().session.toolpath_count();
                return json_str(serde_json::json!({
                    "error": format!(
                        "Toolpath index {idx} out of range (project has {count} toolpaths)"
                    )
                }));
            };
            self.controller.state_mut().selection = Selection::Toolpath(tp_id);
            // The selection write is not the whole selection. Overlays whose
            // precondition reads a DERIVED per-selection artefact — the reach
            // map is the one — are answered from state the pump refreshes,
            // not from `selection` itself, and the pump runs after this call
            // returns. So `set_ui_view(toolpath_index: 13, overlays:
            // {reach_map: true})` in ONE call was refused with "select a
            // finishing operation" while the same overlays map in a SECOND
            // call applied (F3, 2026-09-08).
            //
            // Pumping it here is the same fix, and for the same reason, as
            // the synchronous workspace switch above: everything an
            // `overlays` map in this call will be judged against must
            // already be in force. `process_reach_overlay` resolves
            // `reach_map_spec` (two memoised geometry reads, no drop-cutter
            // work) and hands the walk to the Reach lane, so this stays a
            // cheap call on the request thread.
            self.controller.process_reach_overlay();
        }

        // 3. Properties tab — one-shot override consumed by the
        //    properties panel the next time it renders a selected
        //    toolpath (so it only shows once a toolpath is selected).
        let mut tab_applied: Option<&str> = None;
        if let Some(tab) = properties_tab {
            const VALID_TABS: &[&str] = &[
                "geometry",
                "feeds",
                "feeds_speeds",
                "linking",
                "heights",
                "dressup",
            ];
            if !VALID_TABS.contains(&tab) {
                return json_str(serde_json::json!({
                    "error": format!(
                        "Unknown properties_tab '{tab}'. Valid: geometry, feeds, linking, heights, dressup"
                    )
                }));
            }
            // Scope the override to its target toolpath (the one selected
            // above, or the pre-existing selection) so an intervening
            // render of another toolpath can't consume it.
            let Selection::Toolpath(target_id) = self.controller.state().selection else {
                return json_str(serde_json::json!({
                    "error": "properties_tab needs a toolpath — pass toolpath_index in this \
                              call or select a toolpath first"
                }));
            };
            self.controller.state_mut().gui.pending_toolpath_tab =
                Some((target_id, tab.to_owned()));
            tab_applied = Some(tab);
        }

        // 3b. Non-toolpath properties selection (machine / stock). These
        //     panels render in the Setup workspace's properties pane, so
        //     switch there if the caller didn't pick a workspace.
        let mut select_applied: Option<&str> = None;
        if let Some(sel) = select {
            let target = match sel {
                "machine" => Selection::Machine,
                "stock" => Selection::Stock,
                other => {
                    return json_str(serde_json::json!({
                        "error": format!("Unknown select '{other}'. Valid: machine, stock")
                    }));
                }
            };
            if workspace.is_none() {
                crate::ui::overlays::registry::switch_workspace(
                    self.controller.state_mut(),
                    Workspace::Setup,
                );
                workspace_applied = Some(workspace_key(Workspace::Setup));
            }
            self.controller.state_mut().selection = target;
            select_applied = Some(sel);
        }

        // 4. Modal.
        if let Some(m) = modal {
            match m {
                "none" => {
                    let events = self.controller.events_mut();
                    events.push(AppEvent::Ui(UiCommand::CloseFeedsModal(NoArgs)));
                    events.push(AppEvent::Ui(UiCommand::CloseOptimizeModal(NoArgs)));
                    events.push(AppEvent::Ui(UiCommand::CloseOptimizeProject(NoArgs)));
                    events.push(AppEvent::Ui(UiCommand::CloseExportWizard(NoArgs)));
                    events.push(AppEvent::Ui(UiCommand::CloseToolLibrary(NoArgs)));
                    let state = self.controller.state_mut();
                    state.show_preflight = false;
                    state.show_shortcuts = false;
                }
                "feeds_modal" | "optimize_modal" => {
                    let Selection::Toolpath(tp_id) = self.controller.state().selection else {
                        return json_str(serde_json::json!({
                            "error": format!(
                                "'{m}' needs a toolpath — pass toolpath_index in this call \
                                 or select a toolpath first"
                            )
                        }));
                    };
                    let event = if m == "feeds_modal" {
                        AppEvent::OpenFeedsModal(tp_id)
                    } else {
                        AppEvent::OpenOptimizeModal(tp_id)
                    };
                    self.controller.events_mut().push(event);
                }
                "export_wizard" => {
                    self.controller
                        .events_mut()
                        .push(AppEvent::Ui(UiCommand::OpenExportWizard(NoArgs)));
                }
                "tool_library" => {
                    self.controller
                        .events_mut()
                        .push(AppEvent::Ui(UiCommand::OpenToolLibrary(NoArgs)));
                }
                other => {
                    return json_str(serde_json::json!({
                        "error": format!(
                            "Unknown modal '{other}'. Valid: feeds_modal, optimize_modal, \
                             export_wizard, tool_library, none"
                        )
                    }));
                }
            }
        }

        // 5. Overlays (P6). Last, so the workspace switch above has already
        //    applied its defaults and the preconditions read the workspace
        //    the caller asked for. The registry is the single source of both
        //    the ids and the refusal strings, so this reply and the panel
        //    can never disagree.
        let overlay_report = overlays.map(|requested| {
            crate::ui::overlays::registry::apply_overlays(self.controller.state_mut(), requested)
        });

        // Echo the resulting view. Modal mutations route through the event
        // queue and land later this same frame, so echo the requested
        // targets plus the already-applied selection.
        let state = self.controller.state();
        let selected = match state.selection {
            Selection::Toolpath(id) => state
                .session
                .toolpath_configs()
                .iter()
                .enumerate()
                .find(|(_, tc)| tc.id == id)
                .map(|(index, tc)| {
                    serde_json::json!({
                        "index": index,
                        "id": id,
                        "name": tc.name,
                    })
                }),
            _ => None,
        };
        let selection_kind = match state.selection {
            Selection::Machine => "machine",
            Selection::Stock => "stock",
            Selection::Toolpath(_) => "toolpath",
            Selection::Tool(_) => "tool",
            _ => "other",
        };
        json_str(serde_json::json!({
            "ok": true,
            "workspace": workspace_applied.unwrap_or_else(|| workspace_key(state.workspace)),
            "selected_toolpath": selected,
            "selection": selection_kind,
            "select": select_applied,
            "properties_tab": tab_applied,
            "modal": modal,
            "overlays": overlay_report.map(|report| serde_json::json!({
                "applied": report.applied,
                "refused": report.refused,
            })),
            "note": "view changes render on the next frame; call screenshot_gui to capture",
        }))
    }
}
