use crate::compute::ComputeBackend;
use crate::state::job::{AlignmentPin, FaceUp, Fixture, FlipAxis, KeepOutZone, Setup, ToolConfig};
use crate::state::selection::Selection;

use super::super::AppController;

impl<B: ComputeBackend> AppController<B> {
    // ── Tree / selection helpers ─────────────────────────────────────────

    pub(crate) fn handle_select(&mut self, selection: &Selection) {
        let old_setup = match &self.state.selection {
            Selection::Setup(id) => Some(*id),
            Selection::Fixture(id, _) | Selection::KeepOut(id, _) => Some(*id),
            Selection::Toolpath(tp_id) => self.state.job.setup_of_toolpath(*tp_id),
            _ => None,
        };
        let new_setup = match selection {
            Selection::Setup(id) => Some(*id),
            Selection::Fixture(id, _) | Selection::KeepOut(id, _) => Some(*id),
            Selection::Toolpath(tp_id) => self.state.job.setup_of_toolpath(*tp_id),
            _ => None,
        };
        if old_setup != new_setup {
            self.pending_upload = true;
        }
        self.state.selection = selection.clone();
    }

    pub(crate) fn handle_add_tool(&mut self, tool_type: crate::state::job::ToolType) {
        let id = self.state.job.next_tool_id();
        let tool = ToolConfig::new_default(id, tool_type);
        self.state.selection = Selection::Tool(id);
        self.state.job.tools.push(tool);
        self.state.job.mark_edited();
    }

    pub(crate) fn handle_duplicate_tool(&mut self, tool_id: crate::state::job::ToolId) {
        if let Some(src) = self.state.job.tools.iter().find(|tool| tool.id == tool_id) {
            let mut duplicate = src.clone();
            let new_id = self.state.job.next_tool_id();
            duplicate.id = new_id;
            duplicate.name = format!("{} (copy)", duplicate.name);
            self.state.selection = Selection::Tool(new_id);
            self.state.job.tools.push(duplicate);
            self.state.job.mark_edited();
        }
    }

    pub(crate) fn handle_remove_tool(&mut self, tool_id: crate::state::job::ToolId) {
        let in_use = self
            .state
            .job
            .setups
            .iter()
            .flat_map(|setup| setup.toolpaths.iter())
            .any(|entry| entry.tool_id == tool_id);
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
            self.state.job.tools.retain(|tool| tool.id != tool_id);
            if self.state.selection == Selection::Tool(tool_id) {
                self.state.selection = Selection::None;
            }
            self.state.job.mark_edited();
        }
    }

    pub(crate) fn handle_add_setup(&mut self) {
        let id = self.state.job.next_setup_id();
        let name = format!("Setup {}", id.0 + 1);
        self.state.job.setups.push(Setup::new(id, name));
        self.state.selection = Selection::Setup(id);
        self.state.job.mark_edited();
    }

    pub(crate) fn handle_setup_two_sided(&mut self) {
        let has_flipped = self
            .state
            .job
            .setups
            .iter()
            .any(|s| s.face_up == FaceUp::Bottom);
        if !has_flipped {
            let id = self.state.job.next_setup_id();
            let mut setup = Setup::new(id, format!("Setup {}", id.0 + 1));
            setup.face_up = FaceUp::Bottom;
            self.state.job.setups.push(setup);
        }
        if self.state.job.stock.flip_axis.is_none() {
            self.state.job.stock.flip_axis = Some(FlipAxis::Horizontal);
        }
        if self.state.job.stock.alignment_pins.is_empty() {
            let margin = if self.state.job.stock.padding > 2.0 {
                self.state.job.stock.padding / 2.0
            } else {
                10.0_f64
                    .min(self.state.job.stock.x / 4.0)
                    .min(self.state.job.stock.y / 4.0)
            };
            let cy = self.state.job.stock.y / 2.0;
            self.state
                .job
                .stock
                .alignment_pins
                .push(AlignmentPin::new(margin, cy, 6.0));
            self.state.job.stock.alignment_pins.push(AlignmentPin::new(
                self.state.job.stock.x - margin,
                cy,
                6.0,
            ));
        }
        self.pending_upload = true;
        self.state.job.mark_edited();
        self.sync_alignment_pin_drill();
        self.state.selection = Selection::Stock;
    }

    pub(crate) fn handle_remove_setup(&mut self, setup_id: crate::state::job::SetupId) {
        if self.state.job.setups.len() > 1 {
            self.state.job.setups.retain(|setup| setup.id != setup_id);
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
            self.state.job.mark_edited();
        }
    }

    pub(crate) fn handle_rename_setup(
        &mut self,
        setup_id: crate::state::job::SetupId,
        name: String,
    ) {
        if let Some(setup) = self
            .state
            .job
            .setups
            .iter_mut()
            .find(|setup| setup.id == setup_id)
        {
            setup.name = name;
            self.state.job.mark_edited();
        }
    }

    pub(crate) fn handle_add_fixture(&mut self, setup_id: crate::state::job::SetupId) {
        let fixture_id = self.state.job.next_fixture_id();
        if let Some(setup) = self
            .state
            .job
            .setups
            .iter_mut()
            .find(|setup| setup.id == setup_id)
        {
            setup.fixtures.push(Fixture::new_default(fixture_id));
            self.state.selection = Selection::Fixture(setup_id, fixture_id);
            self.pending_upload = true;
            self.state.job.mark_edited();
        }
    }

    pub(crate) fn handle_remove_fixture(
        &mut self,
        setup_id: crate::state::job::SetupId,
        fixture_id: crate::state::job::FixtureId,
    ) {
        if let Some(setup) = self
            .state
            .job
            .setups
            .iter_mut()
            .find(|setup| setup.id == setup_id)
        {
            setup.fixtures.retain(|fixture| fixture.id != fixture_id);
            if self.state.selection == Selection::Fixture(setup_id, fixture_id) {
                self.state.selection = Selection::Setup(setup_id);
            }
            self.pending_upload = true;
            self.state.job.mark_edited();
        }
    }

    pub(crate) fn handle_add_keep_out(&mut self, setup_id: crate::state::job::SetupId) {
        let keep_out_id = self.state.job.next_keep_out_id();
        if let Some(setup) = self
            .state
            .job
            .setups
            .iter_mut()
            .find(|setup| setup.id == setup_id)
        {
            setup
                .keep_out_zones
                .push(KeepOutZone::new_default(keep_out_id));
            self.state.selection = Selection::KeepOut(setup_id, keep_out_id);
            self.pending_upload = true;
            self.state.job.mark_edited();
        }
    }

    pub(crate) fn handle_remove_keep_out(
        &mut self,
        setup_id: crate::state::job::SetupId,
        keep_out_id: crate::state::job::KeepOutId,
    ) {
        if let Some(setup) = self
            .state
            .job
            .setups
            .iter_mut()
            .find(|setup| setup.id == setup_id)
        {
            setup
                .keep_out_zones
                .retain(|keep_out| keep_out.id != keep_out_id);
            if self.state.selection == Selection::KeepOut(setup_id, keep_out_id) {
                self.state.selection = Selection::Setup(setup_id);
            }
            self.pending_upload = true;
            self.state.job.mark_edited();
        }
    }

    // ── Model helpers ────────────────────────────────────────────────────

    pub(crate) fn handle_remove_model(&mut self, model_id: crate::state::job::ModelId) {
        // Check if any toolpath references this model
        let in_use = self
            .state
            .job
            .setups
            .iter()
            .flat_map(|setup| setup.toolpaths.iter())
            .any(|entry| entry.model_id == model_id);
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
            self.state.job.models.retain(|model| model.id != model_id);
            let clear_selection = matches!(
                self.state.selection,
                Selection::Model(mid) | Selection::Face(mid, _) | Selection::Faces(mid, _)
                    if mid == model_id
            );
            if clear_selection {
                self.state.selection = Selection::None;
            }
            self.pending_upload = true;
            self.state.job.mark_edited();
        }
    }

    // ── Stock / config helpers ───────────────────────────────────────────

    pub(crate) fn handle_stock_changed(&mut self) {
        if self.state.job.stock.auto_from_model
            && let Some(bbox) = self
                .state
                .job
                .models
                .iter()
                .find_map(|m| m.mesh.as_ref().map(|mesh| mesh.bbox))
        {
            self.state.job.stock.update_from_bbox(&bbox);
        }
        self.pending_upload = true;
        self.state.job.mark_edited();
        self.sync_alignment_pin_drill();
    }

    /// Create, update, or remove the auto-generated alignment pin drill toolpath.
    pub(crate) fn sync_alignment_pin_drill(&mut self) {
        use crate::state::toolpath::{
            AlignmentPinDrillConfig, OperationConfig, ToolpathEntry, ToolpathEntryInit,
        };

        let has_pins = !self.state.job.stock.alignment_pins.is_empty();

        // Find existing pin drill toolpath across all setups.
        let existing = self
            .state
            .job
            .setups
            .iter()
            .flat_map(|s| s.toolpaths.iter().map(move |tp| (s.id, tp)))
            .find(|(_, tp)| matches!(tp.operation, OperationConfig::AlignmentPinDrill(_)))
            .map(|(sid, tp)| (sid, tp.id));

        if has_pins && existing.is_none() {
            // Auto-create in Setup 1 at index 0, but only if a tool exists.
            let first_tool_id = self.state.job.tools.first().map(|t| t.id);
            if let (Some(setup), Some(tool_id)) = (self.state.job.setups.first(), first_tool_id) {
                let setup_id = setup.id;
                let id = self.state.job.next_toolpath_id();
                let model_id = self
                    .state
                    .job
                    .models
                    .first()
                    .map(|m| m.id)
                    .unwrap_or(crate::state::job::ModelId(0));
                let holes: Vec<[f64; 2]> = self
                    .state
                    .job
                    .stock
                    .alignment_pins
                    .iter()
                    .map(|p| [p.x, p.y])
                    .collect();
                let cfg = AlignmentPinDrillConfig {
                    holes,
                    ..Default::default()
                };
                let entry = ToolpathEntry::from_init(ToolpathEntryInit::new(
                    id,
                    "Pin Drill".to_owned(),
                    tool_id,
                    model_id,
                    OperationConfig::AlignmentPinDrill(cfg),
                ));
                // Insert at index 0 (first operation in setup).
                if let Some(setup) = self.state.job.setups.iter_mut().find(|s| s.id == setup_id) {
                    setup.toolpaths.insert(0, entry);
                }
            }
        } else if !has_pins {
            // Remove pin drill toolpath if pins were all deleted.
            if let Some((_, tp_id)) = existing {
                self.state.job.remove_toolpath(tp_id);
            }
        } else if let Some((_, tp_id)) = existing {
            // Pins exist and toolpath exists — update hole positions and mark stale.
            let new_holes: Vec<[f64; 2]> = self
                .state
                .job
                .stock
                .alignment_pins
                .iter()
                .map(|p| [p.x, p.y])
                .collect();
            if let Some(tp) = self.state.job.find_toolpath_mut(tp_id) {
                if let OperationConfig::AlignmentPinDrill(ref mut cfg) = tp.operation {
                    cfg.holes = new_holes;
                }
                tp.result = None;
                tp.stale_since = Some(std::time::Instant::now());
            }
        }
    }
}
