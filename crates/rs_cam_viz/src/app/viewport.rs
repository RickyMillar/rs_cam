use crate::interaction::input::InputAction;
use crate::render::{LineUniforms, MeshUniforms, ViewportCallback};
use crate::state::Workspace;
use crate::state::selection::Selection;
use crate::state::selection_highlight::HoverTarget;
use crate::ui::AppEvent;
use crate::ui_command::{SimJumpToMoveArgs, UiCommand};

use super::RsCamApp;

/// The pick mode of the selected toolpath, as the click and the hover read
/// it (geometry selection P4.5).
struct ActivePick {
    toolpath_id: crate::state::toolpath::ToolpathId,
    target: rs_cam_core::session::SelectionTarget,
    filter: crate::state::pick::PickFilter,
    /// The kinds the click tries first: the chips that are on, served and
    /// held (`PickFilter::effective_kinds`).
    kinds: Vec<rs_cam_core::selection::ItemKind>,
    /// The kinds the op can cut on the model, whatever the chips
    /// (`PickFilter::cut_kinds`, P4).
    cut_kinds: Vec<rs_cam_core::selection::ItemKind>,
    item_use: rs_cam_core::compute::catalog::ItemUse,
    held: std::collections::BTreeSet<rs_cam_core::selection::ItemKind>,
}

impl RsCamApp {
    /// The pick context of the viewport point `pos` (screen coordinates).
    fn pick_context(&self, pos: egui::Pos2) -> crate::interaction::picking::PickContext<'_> {
        let rect = self.viewport_rect;
        crate::interaction::picking::PickContext {
            camera: self.controller.state().camera.pose(),
            screen_x: pos.x - rect.min.x,
            screen_y: pos.y - rect.min.y,
            aspect: if rect.height() > 0.0 {
                rect.width() / rect.height()
            } else {
                1.0
            },
            vw: rect.width(),
            vh: rect.height(),
            visibility: &self.controller.state().overlays.model_visibility,
        }
    }

    /// The pick mode of the selected toolpath in the Toolpaths workspace,
    /// or `None` (P4.5).
    fn active_pick(&self) -> Option<ActivePick> {
        let state = self.controller.state();
        if state.workspace != Workspace::Toolpaths {
            return None;
        }
        let Selection::Toolpath(id) = *state.focus() else {
            return None;
        };
        let mode = state.gui.selection_ui.pick_mode(id)?;
        let (_, tc) = state.session().find_toolpath_config_by_id(id)?;
        let held = state
            .session()
            .models()
            .iter()
            .find(|m| m.id == tc.model_id)
            .map(crate::state::pick::held_kinds)
            .unwrap_or_default();
        let item_use = crate::state::pick::target_item_use(tc, mode.target);
        Some(ActivePick {
            toolpath_id: id,
            target: mode.target,
            kinds: mode.filter.effective_kinds(item_use, &held),
            cut_kinds: crate::state::pick::PickFilter::cut_kinds(item_use, &held),
            filter: mode.filter.clone(),
            item_use,
            held,
        })
    }

    /// The key of the item under `pos` in pick mode (P4.5), or `None`,
    /// with the kind whose chip is off when the item is of that kind (P4).
    /// A face that is not flat has no item: `Err` names it. The click and
    /// the hover both read it (`pick_mode_item`), so the hover predicts the
    /// click.
    fn pick_mode_key(
        &self,
        pick: &ActivePick,
        pos: egui::Pos2,
    ) -> Option<(
        Result<rs_cam_core::selection::MatchKey, String>,
        Option<rs_cam_core::selection::ItemKind>,
    )> {
        self.pick_mode_found(pick, pos)
            .map(|(key, chip_off, _)| (key, chip_off))
    }

    /// `pick_mode_key` with the STEP face of the hit when the hit is a
    /// face whose key is `Ok` (a pickable face). The hover reads the face.
    #[expect(
        clippy::type_complexity,
        reason = "one private tuple: the key, the chip-off kind and the face"
    )]
    fn pick_mode_found(
        &self,
        pick: &ActivePick,
        pos: egui::Pos2,
    ) -> Option<(
        Result<rs_cam_core::selection::MatchKey, String>,
        Option<rs_cam_core::selection::ItemKind>,
        Option<(
            crate::state::job::ModelId,
            rs_cam_core::geometry::enriched_mesh::FaceGroupId,
        )>,
    )> {
        let state = self.controller.state();
        let ctx = self.pick_context(pos);
        let found = crate::interaction::picking::pick_mode_item(
            &ctx,
            state.session(),
            pick.toolpath_id,
            &pick.filter,
            &pick.kinds,
            &pick.cut_kinds,
        )?;
        let face = match found.hit {
            crate::interaction::PickHit::ModelFace { model_id, face_id } => {
                Some((model_id, face_id))
            }
            _ => None,
        };
        let key = self.pick_hit_key(pick, found.hit)?;
        let face = face.filter(|_| key.is_ok());
        Some((key, found.chip_off, face))
    }

    /// The pick under the pointer, computed once per frame (B1). Every
    /// hover reader in `draw_viewport` reads it.
    ///
    /// The result is empty when the pointer is not over the viewport, and
    /// during a drag (an orbit or a tab drag). Then no pick runs and the
    /// hover clears. In pick mode the scene pick runs only when a model has
    /// an enriched mesh, because only the face hover reads it there.
    fn hover_result(
        &self,
        ui: &egui::Ui,
        response: &egui::Response,
        pick: Option<&ActivePick>,
    ) -> crate::interaction::picking::HoverResult {
        use crate::interaction::picking::HoverResult;
        if !response.hovered() || response.dragged() {
            return HoverResult::default();
        }
        let Some(pos) = ui.input(|i| i.pointer.hover_pos()) else {
            return HoverResult::default();
        };
        let state = self.controller.state();
        let found = pick.and_then(|pick| self.pick_mode_found(pick, pos));
        let pick_face = found.as_ref().and_then(|(_, _, face)| *face);
        let pick_mode = found.map(|(key, chip_off, _)| (key, chip_off));
        let has_enriched = state
            .session()
            .models()
            .iter()
            .any(|m| m.enriched_mesh.is_some());
        let scene = if pick.is_none() || has_enriched {
            crate::interaction::picking::pick(
                &self.pick_context(pos),
                state.session(),
                &state.gui,
                self.controller.collision_positions(),
                state.workspace,
                crate::state::viewport::ToolpathDrawFilter::from_state(state),
                None,
                crate::state::pin_view::active_display_setup(state.session(), state.focus())
                    .as_ref(),
            )
        } else {
            None
        };
        HoverResult {
            pos: Some(pos),
            pick_mode,
            pick_face,
            scene,
        }
    }

    /// The match key of a pick-mode hit, or `None` for a hit that is not
    /// an item. A face that is not flat has no item: `Err` names it.
    fn pick_hit_key(
        &self,
        pick: &ActivePick,
        hit: crate::interaction::PickHit,
    ) -> Option<Result<rs_cam_core::selection::MatchKey, String>> {
        use crate::interaction::PickHit;
        let state = self.controller.state();
        match hit {
            PickHit::Item { key, .. } => Some(Ok(key)),
            PickHit::ModelFace { model_id, face_id } => {
                let (_, tc) = state
                    .session()
                    .find_toolpath_config_by_id(pick.toolpath_id)?;
                Some(
                    state
                        .session()
                        .face_pick_key(tc, model_id, face_id)
                        .map_err(|refusal| refusal.to_string()),
                )
            }
            // P4.7: an STL face is the flat region under the hit. A cold
            // cache refuses `FacesNotReady`; the faces sweep
            // (`process_model_faces`) is already computing them.
            PickHit::MeshFace { model_id, point } => {
                let (_, tc) = state
                    .session()
                    .find_toolpath_config_by_id(pick.toolpath_id)?;
                Some(
                    state
                        .session()
                        .face_pick_key_at(tc, model_id, point)
                        .map_err(|refusal| refusal.to_string()),
                )
            }
            PickHit::CollisionMarker { .. }
            | PickHit::AlignmentPin { .. }
            | PickHit::Obstacle { .. }
            | PickHit::StockFace { .. }
            | PickHit::Toolpath { .. } => None,
        }
    }

    /// A click in pick mode (P4.5). It wins over every other pick: `true`
    /// when a pick mode is on, also when the click hits no item. The event
    /// comes from `pick_bar::click_event` (P3: a click toggles, a
    /// shift-click adds one item, a right-click adds the layer; P4: a click
    /// on an item whose chip is off turns the chip on and adds the item).
    fn handle_pick_mode_click(
        &mut self,
        click_pos: egui::Pos2,
        click: crate::ui::pick_bar::PickClick,
    ) -> bool {
        let Some(pick) = self.active_pick() else {
            return false;
        };
        match self.pick_mode_key(&pick, click_pos) {
            // E5: a right-click on an item opens the rule menu (its layer,
            // then the "Rule…" offers); the menu sends the edit.
            Some((Ok(key), chip_off)) if click == crate::ui::pick_bar::PickClick::Secondary => {
                let state = self.controller.state();
                let menu = state
                    .gui
                    .selection_ui
                    .mode
                    .as_ref()
                    .and_then(|mode| match mode {
                        crate::state::pick::ViewportMode::Pick(mode) => {
                            Some(crate::ui::pick_bar::PickMenu::new(
                                state.session(),
                                mode,
                                pick.item_use,
                                &pick.held,
                                key,
                                chip_off,
                            ))
                        }
                        crate::state::pick::ViewportMode::Tabs(_) => None,
                    });
                self.controller.state_mut().panels.pick_menu = menu;
            }
            Some((Ok(key), chip_off)) => {
                let event = self
                    .controller
                    .state_mut()
                    .gui
                    .selection_ui
                    .pick_mut()
                    .map(|mode| {
                        crate::ui::pick_bar::click_event(
                            mode,
                            pick.item_use,
                            &pick.held,
                            key,
                            chip_off,
                            click,
                        )
                    });
                if let Some(event) = event.flatten() {
                    self.controller.events_mut().push(event);
                }
            }
            Some((Err(refusal), _)) => self.controller.push_notification(
                format!("Could not pick the face: {refusal}"),
                crate::controller::Severity::Warning,
            ),
            None => {}
        }
        true
    }

    /// The toolpath whose tab editor is on and has a layout, in the
    /// Toolpaths workspace (geometry selection P4.6), or `None`.
    fn active_tab_edit(&self) -> Option<crate::state::toolpath::ToolpathId> {
        let state = self.controller.state();
        if state.workspace != Workspace::Toolpaths {
            return None;
        }
        let Selection::Toolpath(id) = *state.focus() else {
            return None;
        };
        state
            .gui
            .selection_ui
            .tab_edit(id)
            .filter(|mode| mode.answer().is_some())
            .map(|_| id)
    }

    /// The input actions of this frame (B2): read the sample, then run the
    /// one transition on `SelectionUi::drag`.
    fn viewport_input_actions(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
    ) -> Vec<InputAction> {
        use crate::interaction::input::{InputEnv, InputSample, step};
        let sample = InputSample::read(ui, response);
        // The tab test runs only on a primary drag start, as before.
        let press_on_tab = sample.started == Some(egui::PointerButton::Primary)
            && self
                .active_tab_edit()
                .zip(sample.press_origin)
                .is_some_and(|(id, origin)| self.tab_under(id, origin).is_some());
        // A C3a: the point under the press origin, only on a drag start.
        let drag_anchor = sample
            .started
            .and(sample.press_origin)
            .and_then(|origin| self.drag_anchor(origin));
        let env = InputEnv {
            // B3: the setting, read each frame (no GUI copy; Apply takes
            // effect on the next frame).
            scheme: self.controller.state().app_settings.display.mouse_scheme,
            pick_mode_on: self.active_pick().is_some(),
            press_on_tab,
            drag_anchor,
        };
        let drag = &mut self.controller.state_mut().gui.selection_ui.drag;
        step(drag, &sample, &env)
    }

    /// The world point under the screen point `pos` and its camera depth
    /// (A C3a): the nearest model surface hit, else the point at the target
    /// depth. `None` when the ray finds neither; the camera then uses the
    /// target. One brute-force model pick per drag start (design A Q8).
    fn drag_anchor(&self, pos: egui::Pos2) -> Option<crate::interaction::input::DragAnchor> {
        let state = self.controller.state();
        let camera = state.camera.pose();
        let ctx = self.pick_context(pos);
        let display = crate::state::pin_view::active_display_setup(state.session(), state.focus());
        let point = crate::interaction::picking::surface_hit(
            &ctx,
            state.session(),
            display.as_ref().map(|(frame, _)| frame),
        )
        .map(|p| nalgebra::Point3::new(p.x as f32, p.y as f32, p.z as f32))
        .or_else(|| camera.point_at_target_depth(ctx.screen_x, ctx.screen_y, ctx.vw, ctx.vh))?;
        let basis = camera.basis();
        Some(crate::interaction::input::DragAnchor {
            point: [point.x, point.y, point.z],
            depth: (point - basis.eye).dot(&basis.forward),
        })
    }

    /// The tab mark under the screen point `pos`, in the tab editor of
    /// `id` (P4.6).
    fn tab_under(&self, id: crate::state::toolpath::ToolpathId, pos: egui::Pos2) -> Option<usize> {
        let state = self.controller.state();
        let mode = state.gui.selection_ui.tab_edit(id)?;
        let scene = crate::interaction::picking::tab_scene(state.session(), id, mode)?;
        crate::interaction::picking::pick_tab(&self.pick_context(pos), &scene)
    }

    /// A primary click in the tab editor (P4.6). It wins over every other
    /// pick: `true` when the editor is on. A click on a loop (not on a tab)
    /// adds a hand tab at the nearest point of that loop.
    fn handle_tab_click(&mut self, click_pos: egui::Pos2) -> bool {
        let Some(id) = self.active_tab_edit() else {
            return false;
        };
        if self.tab_under(id, click_pos).is_some() {
            return true;
        }
        let hit = {
            let state = self.controller.state();
            state
                .gui
                .selection_ui
                .tab_edit(id)
                .and_then(|mode| mode.answer())
                .and_then(|answer| {
                    crate::interaction::picking::pick_tab_loop(
                        &self.pick_context(click_pos),
                        state.session(),
                        id,
                        &answer.loops,
                    )
                })
        };
        if let Some((loop_key, at)) = hit {
            self.controller.events_mut().push(AppEvent::EditTabs {
                toolpath_id: id,
                edit: rs_cam_core::session::TabEdit::Add { loop_key, at },
            });
        }
        true
    }

    /// A secondary click in the tab editor (P4.6): a click on a tab deletes
    /// it. `true` when the editor is on.
    fn handle_tab_secondary_click(&mut self, click_pos: egui::Pos2) -> bool {
        let Some(id) = self.active_tab_edit() else {
            return false;
        };
        if let Some(mark) = self.tab_under(id, click_pos) {
            self.controller.events_mut().push(AppEvent::EditTabs {
                toolpath_id: id,
                edit: rs_cam_core::session::TabEdit::Delete { mark },
            });
        }
        true
    }

    /// The tab drag of the tab editor (P4.6). A primary drag that starts on
    /// a tab moves it: the point snaps to the loop on each frame (GUI state
    /// only), and the release sends ONE `EditTabs` move (one undo step).
    /// `true` while the drag owns the pointer, so the camera does not orbit.
    fn handle_tab_drag(&mut self, ui: &egui::Ui, response: &egui::Response) -> bool {
        let Some(id) = self.active_tab_edit() else {
            return false;
        };
        if response.drag_started_by(egui::PointerButton::Primary)
            && let Some(origin) = ui.input(|i| i.pointer.press_origin())
            && let Some(mark) = self.tab_under(id, origin)
            && let Some(mode) = self
                .controller
                .state_mut()
                .gui
                .selection_ui
                .tab_edit_mut(id)
        {
            mode.drag = Some(crate::state::pick::TabDrag { mark, to: None });
        }
        let Some(drag) = self
            .controller
            .state()
            .gui
            .selection_ui
            .tab_edit(id)
            .and_then(|mode| mode.drag.clone())
        else {
            return false;
        };
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        if response.dragged_by(egui::PointerButton::Primary)
            && let Some(pos) = response.interact_pointer_pos()
        {
            let snapped = {
                use crate::interaction::picking::{loop_ring, pointer_on_plane, snap_to_loop};
                use crate::state::display_frame::{drawing_plane_z, toolpath_setup_frame};
                let state = self.controller.state();
                let session = state.session();
                state
                    .gui
                    .selection_ui
                    .tab_edit(id)
                    .and_then(|mode| mode.answer())
                    .and_then(|answer| answer.marks.get(drag.mark))
                    .zip(session.find_toolpath_config_by_id(id))
                    .and_then(|(mark, (_, tc))| {
                        let ring = loop_ring(session, tc, &mark.loop_key)?;
                        let frame = toolpath_setup_frame(session, id);
                        let z = drawing_plane_z(session, tc, frame.as_ref());
                        let p = pointer_on_plane(&self.pick_context(pos), z)?;
                        snap_to_loop(frame.as_ref(), session.stock_config(), &ring, p)
                            .map(|(at, _)| at)
                    })
            };
            if let Some(to) = snapped
                && drag.to != Some(to)
                && let Some(mode) = self
                    .controller
                    .state_mut()
                    .gui
                    .selection_ui
                    .tab_edit_mut(id)
                && let Some(d) = mode.drag.as_mut()
            {
                d.to = Some(to);
            }
        }
        if response.drag_stopped() {
            let last = self
                .controller
                .state_mut()
                .gui
                .selection_ui
                .tab_edit_mut(id)
                .and_then(|mode| mode.drag.take());
            if let Some(crate::state::pick::TabDrag { mark, to: Some(to) }) = last {
                self.controller.events_mut().push(AppEvent::EditTabs {
                    toolpath_id: id,
                    edit: rs_cam_core::session::TabEdit::Move { mark, to },
                });
            }
        }
        true
    }

    /// Click-to-select: find nearest toolpath to click position in screen space.
    fn handle_viewport_click(
        &mut self,
        click_pos: egui::Pos2,
        click: crate::ui::pick_bar::PickClick,
    ) {
        let rect = self.viewport_rect;
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return;
        }

        // P4.6: the tab editor wins the click, as the pick mode does.
        if self.handle_tab_click(click_pos) {
            return;
        }
        if self.handle_pick_mode_click(click_pos, click) {
            return;
        }
        // P4.9: a click closes an open popover; a click on an open path of
        // the selected model opens one.
        self.controller.state_mut().panels.drawing_popover = None;
        if self.handle_drawing_popover_click(click_pos) {
            return;
        }

        if self.handle_simulation_semantic_pick(click_pos) {
            return;
        }

        let ctx = crate::interaction::picking::PickContext {
            camera: self.controller.state().camera.pose(),
            screen_x: click_pos.x - rect.min.x,
            screen_y: click_pos.y - rect.min.y,
            aspect: rect.width() / rect.height(),
            vw: rect.width(),
            vh: rect.height(),
            visibility: &self.controller.state().overlays.model_visibility,
        };

        let workspace = self.controller.state().workspace;
        let drill_pick = self.active_drill_toolpath();

        let hit = {
            let state = self.controller.state();
            crate::interaction::picking::pick(
                &ctx,
                state.session(),
                &state.gui,
                self.controller.collision_positions(),
                workspace,
                crate::state::viewport::ToolpathDrawFilter::from_state(state),
                drill_pick,
                crate::state::pin_view::active_display_setup(state.session(), state.focus())
                    .as_ref(),
            )
        };

        // F4: the writes of a hit that are not the focus stay here (the
        // pin ring, the collision jump); the focus and the selection go
        // through the events of the one gesture translation.
        if let Some(hit) = &hit {
            self.pick_hit_side_effects(hit);
        }
        let events = {
            let state = self.controller.state();
            crate::interaction::intent::gesture_events(
                &crate::interaction::intent::ViewportGesture::Click { hit, click },
                &crate::interaction::intent::ViewContext {
                    workspace,
                    focus: state.focus(),
                    selection_ui: &state.gui.selection_ui,
                    session: state.session(),
                },
            )
        };
        for event in events {
            self.controller.events_mut().push(event);
        }
    }

    /// If the current selection is a drill / alignment-pin-drill toolpath in
    /// the Toolpaths workspace, return its id so the viewport offers drill
    /// target picking. Otherwise `None`.
    fn active_drill_toolpath(&self) -> Option<crate::state::toolpath::ToolpathId> {
        use rs_cam_core::compute::catalog::ItemUse;
        let state = self.controller.state();
        if state.workspace != Workspace::Toolpaths {
            return None;
        }
        let Selection::Toolpath(id) = *state.focus() else {
            return None;
        };
        let tc = state
            .session()
            .toolpath_configs()
            .iter()
            .find(|tc| tc.id == id)?;
        // F095: target picking is for an op that reads hole positions, so
        // the registry's item use decides it (today the same two drill ops
        // as `is_drill_kinematics`).
        matches!(tc.operation.capabilities().item_use, ItemUse::Holes { .. }).then_some(id)
    }

    fn handle_simulation_semantic_pick(&mut self, click_pos: egui::Pos2) -> bool {
        if self.controller.state().workspace != Workspace::Simulation
            || !self.controller.state().simulation.debug.enabled
        {
            return false;
        }

        let rect = self.viewport_rect;
        let Some((ray_origin, ray_dir)) = self.controller.state().camera.pose().unproject_ray(
            click_pos.x - rect.min.x,
            click_pos.y - rect.min.y,
            rect.width() / rect.height(),
            rect.width(),
            rect.height(),
        ) else {
            return false;
        };

        let target = {
            let parts = self.controller.state_mut().split_mut();
            parts.simulation.pick_semantic_item_with_ray(
                parts.gui,
                parts.session,
                &rs_cam_core::geo::P3::new(
                    ray_origin.x as f64,
                    ray_origin.y as f64,
                    ray_origin.z as f64,
                ),
                &rs_cam_core::geo::V3::new(ray_dir.x as f64, ray_dir.y as f64, ray_dir.z as f64),
            )
        };

        if let Some(target) = target {
            {
                let state = self.controller.state_mut();
                if let Some(item_id) = target.semantic_item_id {
                    state
                        .simulation
                        .pin_semantic_item(target.toolpath_id, item_id);
                }
                state.simulation.debug.focused_issue_index = None;
                state.simulation.debug.focused_hotspot = None;
            }
            self.controller
                .events_mut()
                .push(AppEvent::Ui(UiCommand::SimJumpToMove(SimJumpToMoveArgs {
                    move_index: target.move_index,
                })));
            return true;
        }

        false
    }

    /// B6 (design B §3.4): the release of a box in pick mode. The keys come
    /// from `picking::pick_items_in_rect` over the chips that are on; the
    /// event comes from `interaction::intent::gesture_events(BoxRelease)`:
    /// one `AddPicks` or `RemovePicks`, one undo step. No key sends nothing.
    /// A boundary whose source is not a selection gives a notice and no
    /// edit.
    fn handle_box_release(
        &mut self,
        boxed: egui::Rect,
        mode: crate::state::viewport_input::BoxMode,
        op: crate::state::viewport_input::BoxOp,
    ) {
        let Some(pick) = self.active_pick() else {
            return;
        };
        let state = self.controller.state();
        let boundary_not_selection = pick.target == rs_cam_core::session::SelectionTarget::Boundary
            && state
                .session()
                .find_toolpath_config_by_id(pick.toolpath_id)
                .is_some_and(|(_, tc)| {
                    !matches!(
                        tc.boundary.source,
                        rs_cam_core::compute::config::BoundarySource::Selection { .. }
                    )
                });
        if boundary_not_selection {
            self.controller.push_notification(
                crate::ui::pick_bar::BOX_NEEDS_SELECTION_BOUNDARY.to_owned(),
                crate::controller::Severity::Info,
            );
            return;
        }
        let origin = self.viewport_rect.min;
        let local = crate::interaction::picking::ScreenRect::from_corners(
            [
                f64::from(boxed.min.x - origin.x),
                f64::from(boxed.min.y - origin.y),
            ],
            [
                f64::from(boxed.max.x - origin.x),
                f64::from(boxed.max.y - origin.y),
            ],
        );
        let keys = crate::interaction::picking::pick_items_in_rect(
            &self.pick_context(boxed.center()),
            state.session(),
            pick.toolpath_id,
            &pick.filter,
            &pick.kinds,
            local,
            mode,
        );
        let events = crate::interaction::intent::gesture_events(
            &crate::interaction::intent::ViewportGesture::BoxRelease { keys, op },
            &crate::interaction::intent::ViewContext {
                workspace: state.workspace,
                focus: state.focus(),
                selection_ui: &state.gui.selection_ui,
                session: state.session(),
            },
        );
        for event in events {
            self.controller.events_mut().push(event);
        }
    }

    /// The writes of a scene hit that are not the focus (F4): a pin click
    /// rings the pin (P7), a stock click drops that ring, and a collision
    /// marker moves the playback. The focus itself is the `UiCommand::Select`
    /// event of `interaction::intent::gesture_events`.
    fn pick_hit_side_effects(&mut self, hit: &crate::interaction::PickHit) {
        use crate::interaction::PickHit;

        let workspace = self.controller.state().workspace;
        match (workspace, hit) {
            (Workspace::Setup | Workspace::Toolpaths, PickHit::AlignmentPin { pin_index }) => {
                // Pins are stock-level: a pin click opens the stock panel
                // with that pin's row marked (P7), and the viewport rings
                // the pin.
                self.controller.state_mut().panels.selected_pin = Some(*pin_index);
                self.controller.set_pending_upload();
            }
            (_, PickHit::StockFace { .. }) => {
                if self
                    .controller
                    .state_mut()
                    .panels
                    .selected_pin
                    .take()
                    .is_some()
                {
                    self.controller.set_pending_upload();
                }
            }
            (Workspace::Simulation, PickHit::CollisionMarker { index }) => {
                // Jump playback to the collision move. The markers are the
                // HOLDER report's positions (`collision_positions`), so the
                // index is a holder-event index. G-RAPIDFRAME: it used to
                // index the RAPID list, a different list, and the event's
                // move is local to the checked toolpath, so the located
                // list gives the run move.
                let move_idx = self
                    .controller
                    .state()
                    .simulation
                    .located_holder_collisions()
                    .get(*index)
                    .and_then(|hit| hit.global_move);
                if let Some(move_idx) = move_idx {
                    let pb = &mut self.controller.state_mut().simulation.playback;
                    pb.current_move = move_idx;
                    pb.playing = false;
                    self.pending_checkpoint_load = true;
                }
            }
            _ => {}
        }
    }

    pub(super) fn draw_viewport(&mut self, ui: &mut egui::Ui) {
        let lane_snapshots = self.controller.lane_snapshots();
        let workspace = self.controller.state().workspace;
        let projection = self.controller.state().camera.pose().projection;
        // Rest-depth heatmap legend info (threshold, peak rest mm) for the
        // currently selected toolpath, if it carries a `rest_grid`. **Any**
        // operation attaches one when its `rest_analysis.enabled` is set and
        // a mesh plus spatial index are present; the pencil `RestDepth` arm
        // and the UnifiedFinish claims pipeline attach their own. The
        // "only the pencil detector" claim this comment used to carry was
        // stale (audit §2c, fix §6.5).
        let selected_rest_grid_info: Option<(f64, f32)> =
            crate::ui::overlays::registry::rest_grid_info(self.controller.state());

        // P6: never hand a degenerate size to the 3D view. `available_size`
        // can arrive at zero once the resizable side panels have taken the
        // window, and a zero-width viewport renders — and screenshots — as
        // nothing at all. The viewport redesign removed the docked Overlays
        // column, so nothing else takes width here: the dock and the
        // catalogue float over the 3D view.
        let free = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(
                free.x.max(crate::ui::overlays::panel::MIN_VIEWPORT_WIDTH),
                free.y.max(1.0),
            ),
            egui::Sense::click_and_drag(),
        );

        self.viewport_rect = rect;
        // P4.6: the tab editor reads its layout again when the toolpath
        // inputs moved (never per frame).
        crate::state::tab_edit::refresh_layout(self.controller.state_mut());
        let dock = {
            let (state, events) = self.controller.state_and_events_mut();
            crate::ui::overlays::panel::draw_catalogue(ui.ctx(), state, projection, events, rect);
            // P4.5: the pick filter bar, at the top centre (L-4).
            crate::ui::pick_bar::draw(ui.ctx(), state, rect);
            crate::ui::viewport_overlay::draw(ui, state, projection, &lane_snapshots, events, rect)
        };
        if self.controller.state().viewport.show_tool_deflection {
            self.draw_sim_deflection_overlay(ui, rect);
        }

        // B2: one input sample and one transition per frame. The actions
        // apply below at the places the old reads were, so the frame order
        // does not change: the click, then the hover, then the right-click
        // and the tab drag, then the camera.
        let actions = self.viewport_input_actions(ui, &response);

        // Click-to-select toolpath in viewport. A click that only closed a
        // dock popover does not also select (MOCKUPS §12, Q1).
        for action in &actions {
            if let InputAction::Click { pos, modifiers } = *action
                && !dock.closed_popover_on_click
            {
                self.handle_viewport_click(pos, crate::ui::pick_bar::PickClick::primary(modifiers));
            }
        }

        // B1: one hover pick per frame. Every hover reader below reads
        // `hover`; no reader picks again.
        let pick = self.active_pick();
        let hover = self.hover_result(ui, &response, pick.as_ref());

        // P4.5: the item under the pointer in pick mode. The hover buffer
        // uploads on a change of the item only (`HoverDetector`), never per
        // frame.
        let hovered_item = match (&pick, &hover.pick_mode) {
            (Some(pick), Some((Ok(key), _))) => Some((pick, key.clone())),
            _ => None,
        };
        if hovered_item.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        // F4: ONE hover target per frame (`hover_target`): in pick mode a
        // pickable face lights, else the pick-mode item rings, else the
        // row of the Geometry tab list (GT-2); outside pick mode the STEP
        // face under the pointer in Toolpaths lights.
        let scene_face = match &hover.scene {
            Some(crate::interaction::picking::PickHit::ModelFace { model_id, face_id })
                if self.controller.state().workspace == Workspace::Toolpaths =>
            {
                Some((*model_id, *face_id))
            }
            _ => None,
        };
        let target = crate::state::selection_highlight::hover_target(
            pick.is_some(),
            hover.pick_face,
            hovered_item.as_ref().map(|(_, key)| key),
            self.controller.state().gui.selection_ui.row_hover.as_ref(),
            scene_face,
        );
        let before = self.hover_detector.hovered().cloned();
        if self.hover_detector.changed(target.clone()) {
            // The hover ring uploads when an item comes or goes; the face
            // state words (D3: `upload_gpu_data` reads
            // `HoverDetector::hovered_face`, a rewrite, no mesh build) when
            // a face comes or goes. The paired `request_repaint` fires on
            // the same change only (G-LV.1): a pointer at rest on a new
            // target makes no egui repaint of its own.
            let touches = |is: fn(&HoverTarget) -> bool| {
                before.as_ref().is_some_and(is) || target.as_ref().is_some_and(is)
            };
            if touches(HoverTarget::is_item) {
                self.hover_upload_pending = true;
            }
            if touches(HoverTarget::is_face) {
                self.controller.set_pending_upload();
            }
            ui.ctx().request_repaint();
        }
        if let Some((pick, key)) = &hovered_item {
            let state = self.controller.state();
            if let Some((_, tc)) = state.session().find_toolpath_config_by_id(pick.toolpath_id) {
                let tip = crate::ui::pick_bar::hover_text(state.session(), tc, pick.target, key);
                egui::Tooltip::always_open(
                    ui.ctx().clone(),
                    response.layer_id,
                    egui::Id::new("pick_item_tip"),
                    egui::PopupAnchor::Pointer,
                )
                .show(|ui| {
                    ui.label(tip);
                });
            }
        }

        // Span-path tooltip on toolpath hover. Shows the SpanKind path for
        // the closest sampled move so the user can read what part of the
        // operation they're pointing at (e.g. "DepthPass 3 / Region 2").
        // In pick mode the item tooltip above has the pointer (P4.5).
        if pick.is_none()
            && let Some(crate::interaction::picking::PickHit::Toolpath { id, move_index }) =
                &hover.scene
            && let Some(tip) = span_path_tooltip(self.controller.state(), *id, *move_index)
        {
            egui::Tooltip::always_open(
                ui.ctx().clone(),
                response.layer_id,
                egui::Id::new("toolpath_span_tip"),
                egui::PopupAnchor::Pointer,
            )
            .show(|ui| {
                ui.label(tip);
            });
        }

        // P4.6: a right-click on a tab deletes it; a drag that starts on a
        // tab moves it, and the camera does not orbit during that drag.
        // E5: in pick mode, a right-click on an item opens the rule menu.
        for action in &actions {
            if let InputAction::SecondaryClick { pos } = *action {
                self.controller.state_mut().panels.pick_menu = None;
                if !self.handle_tab_secondary_click(pos) {
                    let _ =
                        self.handle_pick_mode_click(pos, crate::ui::pick_bar::PickClick::Secondary);
                }
            }
        }
        if let Some(menu) = self.controller.state().panels.pick_menu.clone() {
            use crate::ui::pick_bar::PickMenuOutcome;
            let event = match crate::ui::pick_bar::show_pick_menu(&response, &menu) {
                PickMenuOutcome::Open => None,
                PickMenuOutcome::Closed => {
                    self.controller.state_mut().panels.pick_menu = None;
                    None
                }
                PickMenuOutcome::Layer => {
                    // P4: the layer row turns the chip-off kind on, as the
                    // right-click did.
                    if let (Some(kind), Some(mode)) = (
                        menu.chip_off,
                        self.controller.state_mut().gui.selection_ui.pick_mut(),
                    ) {
                        let _ = mode.filter.kinds.insert(kind);
                    }
                    Some(menu.layer_event())
                }
                PickMenuOutcome::Offer(row) => menu.offer_event(row),
            };
            if let Some(event) = event {
                self.controller.state_mut().panels.pick_menu = None;
                self.controller.events_mut().push(event);
            }
        }
        // §1.5: `step` claims a primary drag that starts on a tab before
        // any orbit (`InputEnv::press_on_tab`).
        let tab_drag = actions.contains(&InputAction::TabDrag);
        if tab_drag {
            let _ = self.handle_tab_drag(ui, &response);
        }
        if !tab_drag
            && response.hovered()
            && let Some(id) = self.active_tab_edit()
            && let Some(pos) = ui.input(|i| i.pointer.hover_pos())
            && self.tab_under(id, pos).is_some()
        {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
        }
        if self
            .tab_marks_detector
            .changed(crate::state::tab_edit::TabMarksKey::of(
                self.controller.state(),
            ))
        {
            self.tab_upload_pending = true;
            ui.ctx().request_repaint();
        }
        // The drag and the wheel (G-SCROLLLAYER: `InputSample::read` takes
        // the wheel only with the layer-aware hover answer).
        crate::interaction::input::apply_camera(
            &actions,
            &mut self.controller.state_mut().camera,
            rect,
        );
        // B6: the box. A release selects; a box in flight paints in egui
        // (nothing uploads during the drag).
        for action in &actions {
            if let InputAction::BoxRelease {
                rect: boxed,
                mode,
                op,
            } = *action
            {
                self.handle_box_release(boxed, mode, op);
            }
        }
        let aspect = if rect.height() > 0.0 {
            rect.width() / rect.height()
        } else {
            1.0
        };
        let view_proj = self.controller.state().camera.pose().view_proj(aspect);
        let eye = self.controller.state().camera.pose().eye();
        let ppp = ui.ctx().pixels_per_point();
        let state = self.controller.state();

        // P5 — is a reach map held for the toolpath that is selected RIGHT
        // NOW? A map held for a previous selection must never be drawn under
        // the new one, so the identity is compared rather than assumed.
        let reach_map_ready = match *state.focus() {
            Selection::Toolpath(id) => {
                state.gui.reach_overlay.toolpath == Some(id)
                    && state.gui.reach_overlay.ready_map().is_some()
            }
            _ => false,
        };

        // The By Area regions draw when their row is on and Ready. The row
        // precondition requires the Toolpaths workspace and a region map on
        // the selected toolpath, so this one answer gates the mesh and the
        // order labels.
        let area_regions_drawn = crate::ui::overlays::registry::area_regions_drawn(state);

        // D4: the eye of the line uniforms and the edge offset. The offset
        // floor is the STEP chord in model mm: the core constant times the
        // declared scale, the max over the visible STEP models.
        let camera = state.camera.pose();
        let chord = state
            .session()
            .models()
            .iter()
            .filter(|m| m.enriched_mesh.is_some())
            .filter(|m| state.overlays.model_visibility.is_visible(m.id))
            .map(|m| {
                rs_cam_core::io::STEP_CHORD_TOLERANCE
                    * m.units.as_ref().map_or(1.0, |u| u.scale_factor())
            })
            .fold(0.0_f64, f64::max) as f32;
        let (line_eye, edge_offset) = match camera.projection {
            crate::render::camera::ProjectionMode::Perspective => (
                [eye.x, eye.y, eye.z, 1.0],
                crate::render::EdgeOffset::perspective(camera.near, chord),
            ),
            crate::render::camera::ProjectionMode::Orthographic => {
                let dir = (camera.eye() - camera.target).normalize();
                (
                    [dir.x, dir.y, dir.z, 0.0],
                    crate::render::EdgeOffset::orthographic(camera.far, chord),
                )
            }
        };

        let callback = ViewportCallback {
            mesh_uniforms: MeshUniforms {
                view_proj,
                light_dir: [0.5, 0.3, 0.8],
                _pad0: 0.0,
                camera_pos: [eye.x, eye.y, eye.z],
                _pad1: 0.0,
            },
            line_uniforms: LineUniforms::new(view_proj, line_eye),
            // P6 — `show_model` replaces `RenderMode::Wireframe`, which drew
            // NOTHING: the crate has no wireframe pipeline, so the mode hid
            // an STL model and left a STEP one untouched (audit §3.1). The
            // flag now reaches both draw loops.
            has_mesh: state
                .session()
                .models()
                .iter()
                .any(|model| model.mesh.is_some())
                && state.viewport.show_model
                && state.workspace != Workspace::Simulation,
            show_model: state.viewport.show_model && state.workspace != Workspace::Simulation,
            display_mode: state.viewport.display_mode,
            edge_offset,
            // D §4: the outline draws while its row is on (the pass also
            // needs the model shaded, `record_scene_pass`).
            show_selection_outline: state.viewport.show_selection_outline,
            outline_uniforms: match camera.projection {
                crate::render::camera::ProjectionMode::Perspective => {
                    crate::render::outline::OutlineUniforms::perspective(
                        camera.near,
                        camera.far,
                        camera.fov,
                        rect.height() * ppp,
                    )
                }
                crate::render::camera::ProjectionMode::Orthographic => {
                    crate::render::outline::OutlineUniforms::orthographic(
                        camera.far,
                        camera.distance * (camera.fov * 0.5).tan(),
                        rect.height() * ppp,
                    )
                }
            },
            line_widths: crate::render::LineWidths {
                viewport_px: [rect.width() * ppp, rect.height() * ppp],
                pixels_per_point: ppp,
            },
            show_grid: state.viewport.show_grid,
            // P6 — the model clause is GONE from all three stock
            // consumers (audit §3.2, fix §6.3, first option). It used to
            // read `mesh.is_some()` on the box and the axes but not on the
            // solid block: one flag, three consumers, two preconditions. A
            // 2D model carries `mesh: None`, so on an SVG / DXF job — the
            // whole pocket, profile, v-carve and trace family — the box and
            // the axes never drew and the Stock checkbox did nothing. All
            // three buffers are built from `stock_config` alone, which
            // always exists, so the honest gate is the flag.
            show_stock: state.viewport.show_stock,
            // One buffer carries five overlays, each filtered into it at
            // upload time by its own flag, so the draw gate is "did any kind
            // contribute" (audit §3.3).
            show_line_marks: state.viewport.show_obstacles
                || state.viewport.show_alignment_pins
                || state.viewport.show_flip_axis
                || state.viewport.show_pin_ghost
                || state.viewport.show_datum,
            // The drawing buffer also carries the pick highlight of the
            // model edge items (UX-7, `drawing_draws`) and the hover ring,
            // so a STEP model with no 2D drawing draws them too.
            show_polygons: state.viewport.show_polygons
                && state
                    .session()
                    .models()
                    .iter()
                    .any(|m| m.items.has_drawing() || m.items.has_model_edges()),
            show_solid_stock: state.viewport.show_stock_solid,
            show_height_planes: state.viewport.show_height_planes
                && matches!(*state.focus(), Selection::Toolpath(_)),
            show_entry_markers: state.viewport.show_entry_markers,
            show_rest_heatmap: state.viewport.show_rest_heatmap
                && state.workspace == Workspace::Toolpaths
                && selected_rest_grid_info.is_some(),
            // Parallel to the rest heatmap and independent of it — both can
            // be up at once. Gated on the Toolpaths workspace for the same
            // reason: it is a planning overlay, not a verification one.
            // ALSO gated on the previewed setup being the ACTIVE one: the
            // tier map lives in its own setup's emission frame, so in any
            // other setup's display frame it renders wrongly shifted
            // (operator-observed: the front tier map floating offset beside
            // the flipped back-setup stock).
            show_tier_preview: state.viewport.show_tier_preview
                && state.workspace == Workspace::Toolpaths
                && state.multitool_planner.as_ref().is_some_and(|p| {
                    p.ready_preview().is_some() && state.active_setup_index() == Some(p.setup_index)
                }),
            // The By Area regions: the flag and the row precondition, which
            // requires the Toolpaths workspace and a region map on the
            // SELECTED toolpath (WP27: the selected toolpath only).
            show_area_regions: area_regions_drawn,
            // P5 — the reach overlay REPLACES the model draw, so its gate
            // also carries `show_model`: with the model hidden there is no
            // surface to re-colour, and drawing a coloured copy would
            // contradict the row the operator switched off. (Before P6 the
            // clause read `render_mode == Shaded`, the same intent expressed
            // through a mode whose other arm drew nothing at all.)
            //
            // `reach_map_ready` above already requires the selection to be a
            // toolpath and the held map to be that toolpath's.
            show_reach_overlay: state.viewport.show_reach_map
                && state.workspace == Workspace::Toolpaths
                && state.viewport.show_model
                && reach_map_ready,
            // P6 — the simulated stock gets a switch. Before this its gate
            // read the workspace and `has_results()` only, while a comment
            // claimed `show_stock` covered it (audit §2b, fix §6.8).
            show_sim_mesh: state.viewport.show_sim_stock
                && state.workspace == Workspace::Simulation
                && state.simulation.has_results(),
            sim_mesh_opacity: state.simulation.stock_opacity,
            show_cutting: state.viewport.show_cutting,
            show_rapids: state.viewport.show_rapids,
            show_collisions: state.viewport.show_collisions,
            toolpath_move_visibility: state
                .viewport
                .toolpath_move_visibility
                .iter()
                .map(|(id, v)| (*id, (v.show_cutting, v.show_rapids)))
                .collect(),
            // F2.2 — derived here from the one freshness model, never
            // stored. `EditedSince` alone: every other non-current state
            // has no drawable geometry to dim in the first place.
            stale_toolpaths: state
                .session()
                .toolpath_configs()
                .iter()
                .enumerate()
                .filter(|(index, _)| {
                    matches!(
                        crate::state::freshness::freshness_at(state.session(), &state.gui, *index),
                        Some(crate::state::freshness::FreshnessState::EditedSince)
                    )
                })
                .map(|(_, tc)| tc.id)
                .collect(),
            show_tool_model: state.workspace == Workspace::Simulation
                && state.simulation.has_results()
                && state.simulation.playback.tool_position.is_some(),
            toolpath_move_limits: if state.workspace == Workspace::Simulation
                && state.simulation.has_results()
                && state.simulation.playback.current_move < state.simulation.total_moves()
            {
                Some(
                    state
                        .simulation
                        .toolpath_move_limits(state.simulation.playback.current_move),
                )
            } else {
                None
            },
            show_origin_axes: state.viewport.show_origin_axes,
            viewport_width: (rect.width() * ppp) as u32,
            viewport_height: (rect.height() * ppp) as u32,
        };

        let cb = egui_wgpu::Callback::new_paint_callback(rect, callback);
        ui.painter().add(cb);
        // B6: the box paints after the scene callback, so it shows over
        // the 3D scene (a shape added before the callback is under it).
        if let crate::state::viewport_input::DragState::BoxSelect { anchor, to, .. } =
            self.controller.state().gui.selection_ui.drag
        {
            crate::ui::pick_bar::paint_box(&ui.painter_at(rect), anchor, to);
        }

        if area_regions_drawn {
            self.draw_area_region_labels(ui, rect);
        }
        if workspace != Workspace::Simulation {
            self.draw_pin_numbers(ui, rect);
        }

        // Draw orientation gizmo overlay (2D, on top of the 3D viewport)
        if self.controller.state().viewport.show_orientation_gizmo {
            self.draw_orientation_gizmo(ui, rect);
        }

        if workspace == Workspace::Simulation {
            // Pending-checkpoint indicator: when a backward jump is in
            // flight, the dexel mesh hasn't been rebuilt yet. Float a
            // pill at the top centre showing an animated spinner alongside
            // a "Loading next state…" label so the click is acknowledged
            // even though the viewport contents haven't changed yet.
            if self.pending_checkpoint_load {
                egui::Area::new(egui::Id::new("sim_loading_pill"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(egui::pos2(rect.center().x - 90.0, rect.min.y + 8.0))
                    .show(ui.ctx(), |ui| {
                        egui::Frame::default()
                            .fill(egui::Color32::from_rgba_unmultiplied(
                                crate::ui::tokens::TINT_INFO.r(),
                                crate::ui::tokens::TINT_INFO.g(),
                                crate::ui::tokens::TINT_INFO.b(),
                                220,
                            ))
                            .stroke(egui::Stroke::new(1.0_f32, crate::ui::tokens::INFO))
                            .corner_radius(6)
                            .inner_margin(egui::Margin::symmetric(10, 4))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.add(egui::Spinner::new().size(14.0));
                                    ui.label(
                                        egui::RichText::new("Loading next state…")
                                            .color(egui::Color32::WHITE),
                                    );
                                });
                            });
                    });
                // Force a redraw next frame so the spinner animates and so
                // the overlay disappears promptly once the load completes.
                ui.ctx().request_repaint();
            }

            // F6.1 viewport extension — bucket-derived hotspot pins
            // removed from the 3D viewport for the same reason as the
            // sim timeline (planning/OPTIMIZER_UX_DIALIN_FIXES.md F6).
            // `SimulationCutTrace.hotspots` is one aggregator per
            // `(toolpath_id, semantic_item_id)`, not a problem flag, so
            // painting them as orange pins reads as "danger everywhere"
            // when nearly all of them are routine reporting buckets.
            // The diagnostics-panel hotspot table still surfaces every
            // bucket for navigation; clicking a row there sets
            // `focused_hotspot` and seeks the playback. Viewport pins
            // for the Critical / Risky tier (gate trips) land with the
            // F6.2 graph-panel work.
            let _ = aspect;
            let active_overlay = {
                let state = self.controller.state_mut();
                if state.simulation.debug.enabled && state.simulation.debug.highlight_active_item {
                    state
                        .simulation
                        .active_semantic_item(&state.gui)
                        .and_then(|active| {
                            state
                                .simulation
                                .semantic_item_bbox_in_simulation(
                                    state.session(),
                                    active.toolpath_id,
                                    &active.item,
                                )
                                .map(|bbox| (active.item.label.clone(), bbox))
                        })
                } else {
                    None
                }
            };
            if let Some((label, bbox)) = active_overlay.as_ref() {
                self.draw_semantic_item_overlay(ui, rect, label, bbox);
            }
        }
        self.draw_drawing_popover(ui);
    }

    /// The popover of an open path (geometry selection P4.9): the two close
    /// acts of the model panel at the click. Each button sends the same
    /// `EditModelDrawing` event as the panel. The popover closes on an act,
    /// on Escape, and on the next viewport click.
    fn draw_drawing_popover(&mut self, ui: &egui::Ui) {
        use rs_cam_core::selection::DrawingEdit;
        use rs_cam_core::session::LedgerEdit;
        let Some(popover) = self.controller.state().panels.drawing_popover.clone() else {
            return;
        };
        let shape = self
            .controller
            .state()
            .session()
            .models()
            .iter()
            .find(|m| m.id == popover.model_id)
            .and_then(|m| {
                rs_cam_core::selection::open_shapes(&m.items)
                    .into_iter()
                    .find(|s| s.key.names_same_geometry(&popover.key))
            });
        let selected = *self.controller.state().focus() == Selection::Model(popover.model_id);
        let Some(shape) = shape.filter(|_| selected) else {
            self.controller.state_mut().panels.drawing_popover = None;
            return;
        };
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.controller.state_mut().panels.drawing_popover = None;
            return;
        }
        let mut edit = None;
        egui::Area::new(egui::Id::new("drawing_popover"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(popover.at[0], popover.at[1]))
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let layer = shape
                        .layer
                        .as_deref()
                        .map_or_else(|| "Open path".to_owned(), |l| format!("Open path on layer {l}"));
                    ui.label(format!("{layer}, gap {:.2} mm", shape.gap_mm));
                    if ui
                        .add(
                            crate::ui::components::Button::quiet(format!(
                                "Close with line ({:.2} mm)",
                                shape.gap_mm
                            ))
                            .enabled(shape.can_close_with_line),
                        )
                        .on_hover_text("Add a straight line from the end of the path to its start.")
                        .clicked()
                    {
                        edit = Some(DrawingEdit::CloseWithLine {
                            path: shape.key.clone(),
                        });
                    }
                    if ui
                        .add(
                            crate::ui::components::Button::quiet("Move ends together")
                                .enabled(shape.can_move_ends_together),
                        )
                        .on_hover_text(
                            "Move the end point of the path onto its start point. No line is added.",
                        )
                        .clicked()
                    {
                        edit = Some(DrawingEdit::MoveEndsTogether {
                            path: shape.key.clone(),
                        });
                    }
                });
            });
        if let Some(edit) = edit {
            self.controller.state_mut().panels.drawing_popover = None;
            self.controller
                .events_mut()
                .push(AppEvent::EditModelDrawing {
                    model_id: popover.model_id,
                    edit: LedgerEdit::Add(edit),
                });
        }
    }

    /// A click on an open path of the selected model opens its popover
    /// (geometry selection P4.9). Returns true when the click opened it.
    fn handle_drawing_popover_click(&mut self, click_pos: egui::Pos2) -> bool {
        let Selection::Model(model_id) = *self.controller.state().focus() else {
            return false;
        };
        let key = {
            let ctx = self.pick_context(click_pos);
            crate::interaction::picking::pick_open_path(
                &ctx,
                self.controller.state().session(),
                model_id,
            )
        };
        let Some(key) = key else {
            return false;
        };
        self.controller.state_mut().panels.drawing_popover =
            Some(crate::state::panels::DrawingPopover {
                model_id,
                key,
                at: [click_pos.x, click_pos.y],
            });
        true
    }

    /// Draw a small XYZ orientation gizmo in the top-right corner of the viewport.
    /// Uses the camera view matrix to rotate unit vectors, then draws 2D lines.
    fn draw_orientation_gizmo(&self, ui: &mut egui::Ui, viewport_rect: egui::Rect) {
        let gizmo_size = 50.0;
        let margin = 10.0;
        let gizmo_center = egui::pos2(
            viewport_rect.max.x - margin - gizmo_size * 0.5,
            viewport_rect.min.y + margin + gizmo_size * 0.5,
        );
        let axis_len = 20.0;

        let view = self.controller.state().camera.pose().view_matrix();
        let painter = ui.painter();

        // Background circle for readability
        painter.circle_filled(
            gizmo_center,
            gizmo_size * 0.5,
            egui::Color32::from_rgba_unmultiplied(
                crate::ui::tokens::DIAGRAM_CANVAS.r(),
                crate::ui::tokens::DIAGRAM_CANVAS.g(),
                crate::ui::tokens::DIAGRAM_CANVAS.b(),
                160,
            ),
        );

        // The X/Y/Z triad keeps its literals. It is a CATEGORY wearing hues
        // that read as DANGER and OK, which §2.6 forbids, but red/green/blue
        // for X/Y/Z is a CAD convention the operator reads without the
        // letters, and no scale in §2.9 carries three separable hues.
        // Ruling R22 makes the axis the one narrow exception: the hues stay,
        // and what the tokens buy is what `AUDIT.md` §2.10 actually
        // complained about — the X axis and error text used to be the SAME
        // constant, so one could not move without the other. They are
        // separate constants at different values now. An axis colour must
        // never appear in a panel.
        let axes: [(nalgebra::Vector3<f32>, egui::Color32, &str); 3] = [
            (
                nalgebra::Vector3::new(1.0, 0.0, 0.0),
                crate::ui::tokens::AXIS_X,
                "X",
            ),
            (
                nalgebra::Vector3::new(0.0, 1.0, 0.0),
                crate::ui::tokens::AXIS_Y,
                "Y",
            ),
            (
                nalgebra::Vector3::new(0.0, 0.0, 1.0),
                crate::ui::tokens::AXIS_Z,
                "Z",
            ),
        ];

        // Sort axes by depth (draw back-to-front)
        let mut axis_data: Vec<(f32, egui::Pos2, egui::Color32, &str)> = axes
            .iter()
            .map(|(axis, color, label)| {
                let rotated = view.transform_vector(axis);
                let screen_end =
                    gizmo_center + egui::vec2(rotated.x * axis_len, -rotated.y * axis_len);
                // Use Z for depth sorting (more negative = further back)
                (rotated.z, screen_end, *color, *label)
            })
            .collect();
        axis_data.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        for (_, end, color, label) in &axis_data {
            painter.line_segment([gizmo_center, *end], egui::Stroke::new(2.0_f32, *color));
            painter.text(
                *end,
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(10.0),
                *color,
            );
        }
    }

    /// Draw the order number of each By Area region at its anchor, and its
    /// filter box (F4: the planner cuts a region by this box, not by its
    /// cells), over the region mesh. The map is in the emission frame; the
    /// display shift of the selected toolpath's setup places it.
    fn draw_area_region_labels(&self, ui: &mut egui::Ui, viewport_rect: egui::Rect) {
        let state = self.controller.state();
        let Some(map) = crate::ui::overlays::registry::selected_area_regions(state) else {
            return;
        };
        let shift = match *state.focus() {
            Selection::Toolpath(id) => state
                .session()
                .setup_of_toolpath_id(id)
                .and_then(|idx| state.session().list_setups().get(idx))
                .map_or(rs_cam_core::geo::P3::new(0.0, 0.0, 0.0), |sd| {
                    crate::state::job::SetupFrame::of(sd)
                        .emission_to_display_shift(state.session().stock_config())
                }),
            _ => return,
        };
        let width = viewport_rect.width().max(1.0);
        let height = viewport_rect.height().max(1.0);
        let aspect = width / height;
        let z = (map.top_z + shift.z) as f32
            + rs_cam_core::maps::rest_heatmap_mesh::AREA_REGION_LIFT_MM;
        let project = |x: f64, y: f64| -> Option<egui::Pos2> {
            self.controller
                .state()
                .camera
                .pose()
                .project_to_screen(
                    [(x + shift.x) as f32, (y + shift.y) as f32, z],
                    aspect,
                    width,
                    height,
                )
                .map(|p| egui::pos2(viewport_rect.min.x + p[0], viewport_rect.min.y + p[1]))
        };
        let painter = ui.painter_at(viewport_rect);
        for region in &map.regions {
            let rgb = rs_cam_core::maps::rest_heatmap_mesh::area_region_color(region.order)
                .unwrap_or([1.0, 1.0, 1.0]);
            let color = crate::ui::tokens::from_linear_rgb(rgb);
            let [x0, y0, x1, y1] = region.bbox_xy;
            let corners = [
                project(x0, y0),
                project(x1, y0),
                project(x1, y1),
                project(x0, y1),
            ];
            if corners.iter().all(Option::is_some) {
                let points: Vec<egui::Pos2> = corners.iter().flatten().copied().collect();
                painter.add(egui::Shape::closed_line(
                    points,
                    egui::Stroke::new(1.0_f32, color),
                ));
            }
            if let Some(anchor) = project(region.anchor_xy[0], region.anchor_xy[1]) {
                let text = region.order.to_string();
                let font = egui::FontId::proportional(14.0);
                let galley = painter.layout_no_wrap(text, font, crate::ui::tokens::TEXT_STRONG);
                let pill =
                    egui::Rect::from_center_size(anchor, galley.size() + egui::vec2(8.0, 4.0));
                painter.rect_filled(pill, 4.0, crate::ui::tokens::SURFACE_RAISED);
                painter.rect_stroke(
                    pill,
                    4.0,
                    egui::Stroke::new(1.5_f32, color),
                    egui::StrokeKind::Inside,
                );
                painter.galley(
                    pill.center() - galley.size() * 0.5,
                    galley,
                    crate::ui::tokens::TEXT_STRONG,
                );
            }
        }
    }

    #[expect(
        clippy::indexing_slicing,
        reason = "edge indices are compile-time constants 0..7 into an 8-element projected array"
    )]
    fn draw_semantic_item_overlay(
        &self,
        ui: &mut egui::Ui,
        viewport_rect: egui::Rect,
        label: &str,
        bbox: &rs_cam_core::geo::BoundingBox3,
    ) {
        let width = viewport_rect.width().max(1.0);
        let height = viewport_rect.height().max(1.0);
        let aspect = width / height;
        let corners = [
            [bbox.min.x as f32, bbox.min.y as f32, bbox.min.z as f32],
            [bbox.max.x as f32, bbox.min.y as f32, bbox.min.z as f32],
            [bbox.max.x as f32, bbox.max.y as f32, bbox.min.z as f32],
            [bbox.min.x as f32, bbox.max.y as f32, bbox.min.z as f32],
            [bbox.min.x as f32, bbox.min.y as f32, bbox.max.z as f32],
            [bbox.max.x as f32, bbox.min.y as f32, bbox.max.z as f32],
            [bbox.max.x as f32, bbox.max.y as f32, bbox.max.z as f32],
            [bbox.min.x as f32, bbox.max.y as f32, bbox.max.z as f32],
        ];
        let projected: Vec<_> = corners
            .iter()
            .map(|corner| {
                self.controller
                    .state()
                    .camera
                    .pose()
                    .project_to_screen(*corner, aspect, width, height)
                    .map(|point| {
                        egui::pos2(
                            viewport_rect.min.x + point[0],
                            viewport_rect.min.y + point[1],
                        )
                    })
            })
            .collect();
        let edges = [
            (0usize, 1usize),
            (1, 2),
            (2, 3),
            (3, 0),
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 4),
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7),
        ];
        let color = crate::ui::tokens::ACCENT;
        let painter = ui.painter();
        for (start_idx, end_idx) in edges {
            if let (Some(start), Some(end)) = (projected[start_idx], projected[end_idx]) {
                painter.line_segment([start, end], egui::Stroke::new(1.5_f32, color));
            }
        }

        if let Some(anchor) = projected.iter().flatten().next().copied() {
            painter.text(
                anchor + egui::vec2(6.0, -6.0),
                egui::Align2::LEFT_BOTTOM,
                label,
                egui::FontId::proportional(12.0),
                color,
            );
        }
    }

    fn draw_sim_deflection_overlay(&self, ui: &mut egui::Ui, viewport_rect: egui::Rect) {
        let state = self.controller.state();
        if state.workspace != Workspace::Simulation || !state.simulation.has_results() {
            return;
        }
        let playback = &state.simulation.playback;
        if playback.tool_position.is_none() {
            return;
        }

        let panel_size = egui::vec2(178.0, 126.0);
        let panel_rect =
            egui::Rect::from_min_size(viewport_rect.min + egui::vec2(12.0, 12.0), panel_size);
        let painter = ui.painter_at(viewport_rect);
        painter.rect_filled(
            panel_rect,
            6.0,
            egui::Color32::from_rgba_unmultiplied(
                crate::ui::tokens::DIAGRAM_CANVAS.r(),
                crate::ui::tokens::DIAGRAM_CANVAS.g(),
                crate::ui::tokens::DIAGRAM_CANVAS.b(),
                220,
            ),
        );
        painter.rect_stroke(
            panel_rect,
            6.0,
            egui::Stroke::new(1.0_f32, crate::ui::tokens::BORDER),
            egui::StrokeKind::Middle,
        );

        let delta = playback.tool_deflection_mm;
        let color = deflection_ui_color(delta);
        let title = match delta {
            Some(mm) => format!("Tool deflection  δ={:.0} µm", mm * 1000.0),
            None => "Tool deflection  —".to_owned(),
        };
        painter.text(
            panel_rect.min + egui::vec2(8.0, 8.0),
            egui::Align2::LEFT_TOP,
            title,
            egui::FontId::proportional(12.0),
            color,
        );

        let stickout_mm = playback.tool_stickout.max(10.0) as f32;
        let body_top = panel_rect.min + egui::vec2(82.0, 32.0);
        let body_bottom = panel_rect.min + egui::vec2(82.0, 104.0);
        let px_per_mm = (body_bottom.y - body_top.y) / stickout_mm;
        let exaggerated_tip_offset = delta
            .map(|mm| (mm as f32 * DEFLECTION_OVERLAY_EXAGGERATION * px_per_mm).min(42.0))
            .unwrap_or(0.0);
        let deflected_tip = body_bottom + egui::vec2(exaggerated_tip_offset, 0.0);

        painter.line_segment(
            [body_top, body_bottom],
            egui::Stroke::new(1.0_f32, crate::ui::tokens::DIAGRAM_TOOL),
        );
        painter.line_segment([body_top, deflected_tip], egui::Stroke::new(2.0_f32, color));
        painter.circle_filled(body_top, 3.0, crate::ui::tokens::DIAGRAM_TOOL);
        painter.circle_filled(deflected_tip, 3.5, color);

        let cutter_len_px = (playback.tool_cutting_length as f32 * px_per_mm).clamp(12.0, 34.0);
        let cutter_top = body_bottom.y - cutter_len_px;
        let radius_px = (playback.tool_radius as f32 * px_per_mm).clamp(3.0, 10.0);
        let cutter_rect = egui::Rect::from_min_max(
            egui::pos2(body_bottom.x - radius_px, cutter_top),
            egui::pos2(body_bottom.x + radius_px, body_bottom.y),
        );
        painter.rect_stroke(
            cutter_rect,
            2.0,
            egui::Stroke::new(1.0_f32, crate::ui::tokens::DIAGRAM_INK),
            egui::StrokeKind::Middle,
        );

        let note = if delta.is_some() {
            format!("{DEFLECTION_OVERLAY_EXAGGERATION:.0}× exaggerated\nDirection illustrative")
        } else {
            "No cutting sample\nat playhead".to_owned()
        };
        painter.text(
            panel_rect.min + egui::vec2(8.0, 62.0),
            egui::Align2::LEFT_TOP,
            note,
            egui::FontId::proportional(11.0),
            crate::ui::theme::TEXT_MUTED,
        );
        painter.text(
            panel_rect.min + egui::vec2(8.0, 106.0),
            egui::Align2::LEFT_TOP,
            playback.tool_type_label.as_str(),
            egui::FontId::proportional(11.0),
            crate::ui::theme::TEXT_DIM,
        );
    }
}

const DEFLECTION_OVERLAY_EXAGGERATION: f32 = 200.0;

fn deflection_ui_color(deflection_mm: Option<f64>) -> egui::Color32 {
    let Some(delta) = deflection_mm else {
        return crate::ui::theme::TEXT_DIM;
    };
    if delta <= rs_cam_core::tool_load::deflection::WITHIN_BOUND_MM {
        crate::ui::theme::SUCCESS
    } else if delta <= rs_cam_core::tool_load::deflection::EXCEEDS_BOUND_MM {
        crate::ui::theme::WARNING
    } else {
        crate::ui::theme::ERROR
    }
}

/// Build a one-line tooltip describing the [`SpanKind`] path covering
/// `move_index` of `toolpath_id` (e.g. `"Operation › DepthPass 3 › Region 2"`).
/// Returns `None` when the toolpath has no spans, the spans are invalid,
/// or the path is empty (e.g. a rapid not inside any span).
fn span_path_tooltip(
    state: &crate::state::AppState,
    toolpath_id: crate::state::toolpath::ToolpathId,
    move_index: usize,
) -> Option<String> {
    use rs_cam_core::trace::toolpath_spans::{SpanKind, SpanPayload};
    let rt = state.gui.toolpath_rt.get(&toolpath_id)?;
    let result = rt.run.result()?;
    if !result.spans_valid() {
        return None;
    }
    let path = result.annotated.span_path_at(move_index);
    if path.is_empty() {
        return None;
    }
    let parts: Vec<String> = path
        .iter()
        .filter_map(|sid| result.spans().get(sid.0 as usize))
        .map(|span| match (&span.kind, &span.payload) {
            (
                SpanKind::DepthPass,
                Some(SpanPayload::DepthPass {
                    pass_index,
                    z_level,
                }),
            ) => {
                format!("DepthPass {} (z={:.2})", pass_index, z_level)
            }
            (SpanKind::Region, Some(SpanPayload::Region { region_id, .. })) => {
                format!("Region {}", region_id)
            }
            _ if !span.label.is_empty() => format!("{:?} {}", span.kind, span.label),
            _ => format!("{:?}", span.kind),
        })
        .collect();
    Some(parts.join(" › "))
}
