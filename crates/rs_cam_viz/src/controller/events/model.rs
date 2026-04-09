use rs_cam_core::compute::stock_config::AlignmentPin;
use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::session::{Fixture, FixtureKind, KeepOutZone};

use crate::compute::ComputeBackend;
use crate::state::job::{FlipAxis, ModelId, SetupId, ToolConfig};
use crate::state::selection::Selection;

use super::super::AppController;

impl<B: ComputeBackend> AppController<B> {
    // ── Tree / selection helpers ─────────────────────────────────────────

    pub(crate) fn handle_select(&mut self, selection: &Selection) {
        let old_setup = match &self.state.selection {
            Selection::Setup(id) => Some(*id),
            Selection::Fixture(id, _) | Selection::KeepOut(id, _) => Some(*id),
            Selection::Toolpath(tp_id) => self.setup_of_toolpath(*tp_id),
            _ => None,
        };
        let new_setup = match selection {
            Selection::Setup(id) => Some(*id),
            Selection::Fixture(id, _) | Selection::KeepOut(id, _) => Some(*id),
            Selection::Toolpath(tp_id) => self.setup_of_toolpath(*tp_id),
            _ => None,
        };
        if old_setup != new_setup {
            self.pending_upload = true;
        }
        self.state.selection = selection.clone();
    }

    /// Find the setup that owns a given toolpath ID.
    pub(crate) fn setup_of_toolpath(
        &self,
        tp_id: crate::state::toolpath::ToolpathId,
    ) -> Option<SetupId> {
        // Find which setup contains this toolpath by checking toolpath_indices
        let (tp_index, _) = self.state.session.find_toolpath_config_by_id(tp_id.0)?;
        self.state
            .session
            .list_setups()
            .iter()
            .find(|s| s.toolpath_indices.contains(&tp_index))
            .map(|s| SetupId(s.id))
    }

    pub(crate) fn handle_add_tool(&mut self, tool_type: crate::state::job::ToolType) {
        let tool = ToolConfig::new_default(crate::state::job::ToolId(0), tool_type);
        let idx = self.state.session.add_tool(tool);
        // The session assigned the ID — read it back.
        if let Some(tool) = self.state.session.tools().get(idx) {
            self.state.selection = Selection::Tool(tool.id);
        }
        self.state.gui.mark_edited();
    }

    pub(crate) fn handle_duplicate_tool(&mut self, tool_id: crate::state::job::ToolId) {
        if let Some(src) = self
            .state
            .session
            .tools()
            .iter()
            .find(|tool| tool.id == tool_id)
        {
            let mut duplicate = src.clone();
            duplicate.name = format!("{} (copy)", duplicate.name);
            let idx = self.state.session.add_tool(duplicate);
            if let Some(tool) = self.state.session.tools().get(idx) {
                self.state.selection = Selection::Tool(tool.id);
            }
            self.state.gui.mark_edited();
        }
    }

    pub(crate) fn handle_remove_tool(&mut self, tool_id: crate::state::job::ToolId) {
        let in_use = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .any(|tc| tc.tool_id == tool_id.0);
        if in_use {
            tracing::warn!(
                "Cannot remove tool {:?}: still referenced by one or more toolpaths",
                tool_id
            );
            self.push_notification(
                "Cannot remove tool: still referenced by one or more toolpaths".into(),
                super::super::Severity::Warning,
            );
        } else {
            self.state
                .session
                .tools_mut()
                .retain(|tool| tool.id != tool_id);
            if self.state.selection == Selection::Tool(tool_id) {
                self.state.selection = Selection::None;
            }
            self.state.gui.mark_edited();
        }
    }

    pub(crate) fn handle_add_setup(&mut self) {
        let idx = self
            .state
            .session
            .add_setup("".to_owned(), FaceUp::default());
        if let Some(setup) = self.state.session.list_setups().get(idx) {
            let id = SetupId(setup.id);
            // Rename with proper number
            if let Some(s) = self.state.session.setups_mut().get_mut(idx) {
                s.name = format!("Setup {}", s.id + 1);
            }
            self.state.selection = Selection::Setup(id);
        }
        self.state.gui.mark_edited();
    }

    pub(crate) fn handle_setup_two_sided(&mut self) {
        let has_flipped = self
            .state
            .session
            .list_setups()
            .iter()
            .any(|s| s.face_up == FaceUp::Bottom);
        if !has_flipped {
            let idx = self
                .state
                .session
                .add_setup("".to_owned(), FaceUp::Bottom);
            if let Some(s) = self.state.session.setups_mut().get_mut(idx) {
                s.name = format!("Setup {}", s.id + 1);
            }
        }
        {
            let stock = self.state.session.stock_mut();
            if stock.flip_axis.is_none() {
                stock.flip_axis = Some(FlipAxis::Horizontal);
            }
            if stock.alignment_pins.is_empty() {
                let margin = if stock.padding > 2.0 {
                    stock.padding / 2.0
                } else {
                    10.0_f64
                        .min(stock.x / 4.0)
                        .min(stock.y / 4.0)
                };
                let cy = stock.y / 2.0;
                let x_size = stock.x;
                stock
                    .alignment_pins
                    .push(AlignmentPin::new(margin, cy, 6.0));
                stock
                    .alignment_pins
                    .push(AlignmentPin::new(x_size - margin, cy, 6.0));
            }
        }
        self.pending_upload = true;
        self.state.gui.mark_edited();
        self.sync_alignment_pin_drill();
        self.state.selection = Selection::Stock;
    }

    pub(crate) fn handle_remove_setup(&mut self, setup_id: SetupId) {
        let setups = self.state.session.list_setups();
        if setups.len() > 1 {
            // Find index for removal
            if let Some(idx) = setups.iter().position(|s| s.id == setup_id.0) {
                // First remove all toolpaths belonging to this setup
                let tp_indices: Vec<usize> = setups
                    .get(idx)
                    .map(|s| s.toolpath_indices.clone())
                    .unwrap_or_default();
                // Remove in reverse order to preserve indices
                let mut sorted_indices = tp_indices;
                sorted_indices.sort_unstable();
                sorted_indices.reverse();
                for tp_idx in sorted_indices {
                    let _ = self.state.session.remove_toolpath(tp_idx);
                }
                // Now remove the setup
                let _ = self.state.session.remove_setup(idx);
            }
            match self.state.selection {
                Selection::Setup(id) if id == setup_id => {
                    self.state.selection = Selection::None;
                }
                Selection::Fixture(id, _) if id == setup_id => {
                    self.state.selection = Selection::None;
                }
                Selection::KeepOut(id, _) if id == setup_id => {
                    self.state.selection = Selection::None;
                }
                _ => {}
            }
            self.pending_upload = true;
            self.state.gui.mark_edited();
        }
    }

    pub(crate) fn handle_rename_setup(&mut self, setup_id: SetupId, name: String) {
        if let Some(setup) = self
            .state
            .session
            .setups_mut()
            .iter_mut()
            .find(|s| s.id == setup_id.0)
        {
            setup.name = name;
            self.state.gui.mark_edited();
        }
    }

    pub(crate) fn handle_add_fixture(&mut self, setup_id: SetupId) {
        // Generate a fixture ID based on existing max
        let max_id = self
            .state
            .session
            .list_setups()
            .iter()
            .flat_map(|s| s.fixtures.iter())
            .map(|f| f.id.0)
            .max()
            .map_or(0, |id| id + 1);
        let fixture_id = crate::state::job::FixtureId(max_id);

        if let Some(setup) = self
            .state
            .session
            .setups_mut()
            .iter_mut()
            .find(|s| s.id == setup_id.0)
        {
            setup.fixtures.push(Fixture {
                id: fixture_id,
                name: format!("Fixture {}", fixture_id.0 + 1),
                kind: FixtureKind::Clamp,
                enabled: true,
                origin_x: 0.0,
                origin_y: 0.0,
                origin_z: 0.0,
                size_x: 30.0,
                size_y: 15.0,
                size_z: 20.0,
                clearance: 3.0,
            });
            self.state.selection = Selection::Fixture(setup_id, fixture_id);
            self.pending_upload = true;
            self.state.gui.mark_edited();
        }
    }

    pub(crate) fn handle_remove_fixture(
        &mut self,
        setup_id: SetupId,
        fixture_id: crate::state::job::FixtureId,
    ) {
        if let Some(setup) = self
            .state
            .session
            .setups_mut()
            .iter_mut()
            .find(|s| s.id == setup_id.0)
        {
            setup.fixtures.retain(|f| f.id != fixture_id);
            if self.state.selection == Selection::Fixture(setup_id, fixture_id) {
                self.state.selection = Selection::Setup(setup_id);
            }
            self.pending_upload = true;
            self.state.gui.mark_edited();
        }
    }

    pub(crate) fn handle_add_keep_out(&mut self, setup_id: SetupId) {
        let max_id = self
            .state
            .session
            .list_setups()
            .iter()
            .flat_map(|s| s.keep_out_zones.iter())
            .map(|k| k.id.0)
            .max()
            .map_or(0, |id| id + 1);
        let keep_out_id = crate::state::job::KeepOutId(max_id);

        if let Some(setup) = self
            .state
            .session
            .setups_mut()
            .iter_mut()
            .find(|s| s.id == setup_id.0)
        {
            setup.keep_out_zones.push(KeepOutZone {
                id: keep_out_id,
                name: format!("Keep-Out {}", keep_out_id.0 + 1),
                enabled: true,
                origin_x: 0.0,
                origin_y: 0.0,
                size_x: 20.0,
                size_y: 20.0,
            });
            self.state.selection = Selection::KeepOut(setup_id, keep_out_id);
            self.pending_upload = true;
            self.state.gui.mark_edited();
        }
    }

    pub(crate) fn handle_remove_keep_out(
        &mut self,
        setup_id: SetupId,
        keep_out_id: crate::state::job::KeepOutId,
    ) {
        if let Some(setup) = self
            .state
            .session
            .setups_mut()
            .iter_mut()
            .find(|s| s.id == setup_id.0)
        {
            setup.keep_out_zones.retain(|k| k.id != keep_out_id);
            if self.state.selection == Selection::KeepOut(setup_id, keep_out_id) {
                self.state.selection = Selection::Setup(setup_id);
            }
            self.pending_upload = true;
            self.state.gui.mark_edited();
        }
    }

    // ── Model helpers ────────────────────────────────────────────────────

    pub(crate) fn handle_remove_model(&mut self, model_id: ModelId) {
        let in_use = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .any(|tc| tc.model_id == model_id.0);
        if in_use {
            tracing::warn!(
                "Cannot remove model {:?}: still referenced by one or more toolpaths",
                model_id
            );
            self.push_notification(
                "Cannot remove model: still referenced by one or more toolpaths".into(),
                super::super::Severity::Warning,
            );
        } else {
            self.state
                .session
                .models_mut()
                .retain(|m| m.id != model_id.0);
            let clear_selection = matches!(
                self.state.selection,
                Selection::Model(mid) | Selection::Face(mid, _) | Selection::Faces(mid, _)
                    if mid == model_id
            );
            if clear_selection {
                self.state.selection = Selection::None;
            }
            self.pending_upload = true;
            self.state.gui.mark_edited();
        }
    }

    // ── Stock / config helpers ───────────────────────────────────────────

    pub(crate) fn handle_stock_changed(&mut self) {
        let auto_from_model = self.state.session.stock_config().auto_from_model;
        if auto_from_model
            && let Some(bbox) = self
                .state
                .session
                .models()
                .iter()
                .find_map(|m| m.mesh.as_ref().map(|mesh| mesh.bbox))
        {
            self.state.session.stock_mut().update_from_bbox(&bbox);
        }
        self.pending_upload = true;
        self.state.gui.mark_edited();
        self.sync_alignment_pin_drill();
    }

    /// Create, update, or remove the auto-generated alignment pin drill toolpath.
    pub(crate) fn sync_alignment_pin_drill(&mut self) {
        use crate::state::toolpath::{AlignmentPinDrillConfig, OperationConfig};

        let has_pins = !self.state.session.stock_config().alignment_pins.is_empty();

        // Find existing pin drill toolpath across all setups.
        let existing: Option<(usize, usize)> = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .enumerate()
            .find(|(_, tc)| matches!(tc.operation, OperationConfig::AlignmentPinDrill(_)))
            .map(|(idx, tc)| (idx, tc.id));

        if has_pins && existing.is_none() {
            // Auto-create in first setup, but only if a tool exists.
            let first_tool_id = self.state.session.tools().first().map(|t| t.id.0);
            let first_setup_idx = if self.state.session.list_setups().is_empty() {
                None
            } else {
                Some(0)
            };
            if let (Some(setup_idx), Some(tool_id)) = (first_setup_idx, first_tool_id) {
                let model_id = self
                    .state
                    .session
                    .models()
                    .first()
                    .map(|m| m.id)
                    .unwrap_or(0);
                let holes: Vec<[f64; 2]> = self
                    .state
                    .session
                    .stock_config()
                    .alignment_pins
                    .iter()
                    .map(|p| [p.x, p.y])
                    .collect();
                let cfg = AlignmentPinDrillConfig {
                    holes,
                    ..Default::default()
                };
                let tc = rs_cam_core::session::ToolpathConfig {
                    id: 0, // will be assigned by session
                    name: "Pin Drill".to_owned(),
                    enabled: true,
                    operation: OperationConfig::AlignmentPinDrill(cfg),
                    dressups: crate::state::toolpath::DressupConfig::default(),
                    heights: crate::state::toolpath::HeightsConfig::default(),
                    tool_id,
                    model_id,
                    pre_gcode: None,
                    post_gcode: None,
                    boundary: crate::state::toolpath::BoundaryConfig::default(),
                    boundary_inherit: true,
                    stock_source: crate::state::toolpath::StockSource::Fresh,
                    coolant: rs_cam_core::gcode::CoolantMode::Off,
                    face_selection: None,
                    feeds_auto: crate::state::toolpath::FeedsAutoMode::default(),
                    debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
                };
                let _ = self.state.session.add_toolpath(setup_idx, tc);
            }
        } else if !has_pins {
            // Remove pin drill toolpath if pins were all deleted.
            if let Some((idx, _id)) = existing {
                let _ = self.state.session.remove_toolpath(idx);
            }
        } else if let Some((idx, _id)) = existing {
            // Pins exist and toolpath exists — update hole positions and mark stale.
            let new_holes: Vec<[f64; 2]> = self
                .state
                .session
                .stock_config()
                .alignment_pins
                .iter()
                .map(|p| [p.x, p.y])
                .collect();
            if let Some(tc) = self.state.session.toolpath_configs_mut().get_mut(idx)
                && let OperationConfig::AlignmentPinDrill(ref mut cfg) = tc.operation
            {
                cfg.holes = new_holes;
            }
            // Mark stale in GUI runtime
            if let Some((_, tc)) = self.state.session.find_toolpath_config_by_id(
                existing.map(|(_, id)| id).unwrap_or(0),
            )
                && let Some(rt) = self.state.gui.toolpath_rt.get_mut(&tc.id)
            {
                rt.result = None;
                rt.stale_since = Some(std::time::Instant::now());
            }
        }
    }
}
