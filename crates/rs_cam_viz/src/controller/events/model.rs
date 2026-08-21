use rs_cam_core::compute::transform::FaceUp;
use rs_cam_core::session::{Fixture, FixtureKind, KeepOutZone};

use crate::compute::ComputeBackend;
use crate::state::job::{FlipAxis, ModelId, SetupId, ToolConfig};
use crate::state::selection::Selection;

use super::super::AppController;

/// Stated reason when the pin diameter cannot be sized from a tool.
///
/// The hole has to match the dowel the keying geometry assumed, and the
/// hole is whatever the pin-drill cutter makes. With no tool in the
/// project there is no honest diameter — and a guess is exactly what the
/// old hardcoded `AlignmentPin::new(.., 6.0)` was.
const NO_PIN_TOOL_MESSAGE: &str = "Cannot place registration pins: no tool is defined, so \
     the pin diameter would be a guess. Add the drill you will use for the dowel holes, \
     then try again.";

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
        // `upload_gpu_data` (app/gpu_upload.rs) keys the height-plane and
        // rest-heatmap overlays off "whichever toolpath is currently
        // `Selection::Toolpath`", rebuilt only when `pending_upload` fires —
        // not every frame. The setup-changed check above catches switching
        // setups, but switching the selected *toolpath within the same
        // setup* (e.g. selecting a pencil rest-analysis toolpath right
        // after a scallop in the same setup) left both overlays showing
        // stale data from whichever toolpath was selected the last time
        // something else happened to set `pending_upload`. Any actual
        // selection change needs the same treatment.
        if old_setup != new_setup || self.state.selection != *selection {
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
        let (tp_index, _) = self.state.session.find_toolpath_config_by_id(tp_id)?;
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

    pub(crate) fn handle_add_tool_from_library(&mut self, mut tool: ToolConfig) {
        // The catalog tool's id is project-irrelevant; the session
        // reassigns it on insert. Reset to a sentinel first.
        tool.id = crate::state::job::ToolId(0);
        let idx = self.state.session.add_tool(tool);
        if let Some(tool) = self.state.session.tools().get(idx) {
            self.state.selection = Selection::Tool(tool.id);
        }
        self.state.gui.mark_edited();
    }

    // ── Tool Library modal ──────────────────────────────────────────────

    /// Load a fresh snapshot of every catalog into the modal state. Used
    /// both to open the modal and to refresh it after a mutation.
    fn load_tool_library_snapshot(&mut self) {
        let catalogs = rs_cam_core::tool_library::list_libraries()
            .into_iter()
            .filter_map(|name| {
                rs_cam_core::tool_library::load_library(&name)
                    .ok()
                    .map(|catalog| (name, catalog))
            })
            .collect();
        self.state.tool_library_modal = Some(crate::state::ToolLibraryModalState { catalogs });
    }

    /// Refresh the snapshot only if the modal is open (post-mutation).
    fn refresh_tool_library_snapshot(&mut self) {
        if self.state.tool_library_modal.is_some() {
            self.load_tool_library_snapshot();
        }
    }

    pub(crate) fn open_tool_library(&mut self) {
        self.state.close_modals_for_exclusivity();
        self.load_tool_library_snapshot();
    }

    /// Report a tool-library error to the user, prefixed with context.
    fn report_tool_library_error(
        &mut self,
        context: &str,
        err: &rs_cam_core::tool_library::ToolLibraryError,
    ) {
        tracing::error!("{context}: {err}");
        self.push_notification(format!("{context}: {err}"), super::super::Severity::Error);
    }

    pub(crate) fn delete_library_tool(&mut self, catalog: &str, index: usize) {
        if let Err(e) = rs_cam_core::tool_library::remove_tool_at(catalog, index) {
            self.report_tool_library_error("Delete tool failed", &e);
        }
        self.refresh_tool_library_snapshot();
    }

    pub(crate) fn update_library_tool(&mut self, catalog: &str, index: usize, tool: ToolConfig) {
        if let Err(e) = rs_cam_core::tool_library::update_tool_at(catalog, index, tool) {
            self.report_tool_library_error("Update tool failed", &e);
        }
        self.refresh_tool_library_snapshot();
    }

    pub(crate) fn move_library_tool(&mut self, from: &str, index: usize, to: &str) {
        if let Err(e) = rs_cam_core::tool_library::move_tool(from, index, to) {
            self.report_tool_library_error("Move tool failed", &e);
        }
        self.refresh_tool_library_snapshot();
    }

    pub(crate) fn create_tool_catalog(&mut self, name: &str) {
        if let Err(e) = rs_cam_core::tool_library::create_library(name) {
            self.report_tool_library_error("Create catalog failed", &e);
        }
        self.refresh_tool_library_snapshot();
    }

    pub(crate) fn delete_tool_catalog(&mut self, name: &str) {
        if let Err(e) = rs_cam_core::tool_library::delete_library(name) {
            self.report_tool_library_error("Delete catalog failed", &e);
        }
        self.refresh_tool_library_snapshot();
    }

    pub(crate) fn rename_tool_catalog(&mut self, old: &str, new: &str) {
        if let Err(e) = rs_cam_core::tool_library::rename_library(old, new) {
            self.report_tool_library_error("Rename catalog failed", &e);
        }
        self.refresh_tool_library_snapshot();
    }

    pub(crate) fn dedupe_tool_catalog(&mut self, name: &str) {
        if let Err(e) = rs_cam_core::tool_library::dedupe_library(name) {
            self.report_tool_library_error("Dedupe catalog failed", &e);
        }
        self.refresh_tool_library_snapshot();
    }

    // ── Machine library (snapshot model) ────────────────────────────────

    /// Import a library machine as a SNAPSHOT copy into the project's
    /// inline machine (no live link), then invalidate machine-dependent
    /// state — mirrors `MachineChanged`.
    pub(crate) fn import_machine_from_library(&mut self, name: &str) {
        match rs_cam_core::machine_library::load(name) {
            Ok(profile) => {
                *self.state.session.machine_mut() = profile;
                self.state.session.invalidate_machine();
                self.state.gui.mark_edited();
                self.set_status(format!("Imported machine '{name}' (snapshot copy)"));
            }
            Err(e) => self.report_machine_library_error("Import machine failed", &e),
        }
    }

    pub(crate) fn save_machine_to_library(&mut self, name: &str) {
        match rs_cam_core::machine_library::save(name, self.state.session.machine()) {
            Ok(path) => self.set_status(format!("Saved machine to {}", path.display())),
            Err(e) => self.report_machine_library_error("Save machine failed", &e),
        }
    }

    pub(crate) fn delete_machine_from_library(&mut self, name: &str) {
        match rs_cam_core::machine_library::delete(name) {
            Ok(()) => self.set_status(format!("Deleted machine '{name}' from library")),
            Err(e) => self.report_machine_library_error("Delete machine failed", &e),
        }
    }

    pub(crate) fn rename_machine_in_library(&mut self, old: &str, new: &str) {
        match rs_cam_core::machine_library::rename(old, new) {
            Ok(()) => self.set_status(format!("Renamed machine '{old}' → '{new}'")),
            Err(e) => self.report_machine_library_error("Rename machine failed", &e),
        }
    }

    fn report_machine_library_error(
        &mut self,
        context: &str,
        err: &rs_cam_core::machine_library::MachineLibraryError,
    ) {
        tracing::error!("{context}: {err}");
        self.push_notification(format!("{context}: {err}"), super::super::Severity::Error);
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
        let index = self
            .state
            .session
            .tools()
            .iter()
            .position(|t| t.id == tool_id);
        if let Some(idx) = index {
            match self.state.session.remove_tool(idx) {
                Ok(()) => {
                    if self.state.selection == Selection::Tool(tool_id) {
                        self.state.selection = Selection::None;
                    }
                    self.state.gui.mark_edited();
                }
                Err(rs_cam_core::session::SessionError::ToolInUse(_)) => {
                    tracing::warn!(
                        "Cannot remove tool {:?}: still referenced by one or more toolpaths",
                        tool_id
                    );
                    self.push_notification(
                        "Cannot remove tool: still referenced by one or more toolpaths".into(),
                        super::super::Severity::Warning,
                    );
                }
                Err(e) => {
                    self.push_notification(e.to_string(), super::super::Severity::Error);
                }
            }
        }
    }

    pub(crate) fn handle_add_setup(&mut self) {
        let next_id = self.state.session.list_setups().len() + 1;
        let name = format!("Setup {next_id}");
        let idx = self.state.session.add_setup(name, FaceUp::default());
        if let Some(setup) = self.state.session.list_setups().get(idx) {
            self.state.selection = Selection::Setup(SetupId(setup.id));
        }
        self.state.gui.mark_edited();
    }

    /// Diameter of the tool the pin-drill operation will actually run.
    ///
    /// The pin geometry is planned against the dowel, the dowel is the
    /// hole, and the hole is whatever this cutter makes. Before
    /// G-PINAUTO the placer hardcoded 6.0: against a Ø3 cutter that is a
    /// hole no 6 mm dowel ever sees, and against a large cutter it is a
    /// wall clearance computed for the wrong pin.
    ///
    /// The tool choice mirrors [`Self::sync_alignment_pin_drill`]: the
    /// existing pin-drill op's tool if there is one, else the first tool.
    /// If those two ever diverge, the hole stops matching the plan.
    fn pin_drill_tool_diameter(&self) -> Option<f64> {
        use crate::state::toolpath::OperationConfig;

        let tool_id = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .find(|tc| matches!(tc.operation, OperationConfig::AlignmentPinDrill(_)))
            .map(|tc| tc.tool_id)
            .or_else(|| self.state.session.tools().first().map(|t| t.id.0))?;
        self.state
            .session
            .tools()
            .iter()
            .find(|t| t.id.0 == tool_id)
            .map(|t| t.diameter)
            .filter(|d| *d > 0.0)
    }

    /// World-frame bbox of the first model that has one.
    ///
    /// Prefers the mesh bbox for 3D models and falls back to the 2D
    /// polygon bbox for SVG/DXF — see F-13 in the April review.
    fn first_model_bbox(&self) -> Option<rs_cam_core::geo::BoundingBox3> {
        self.state.session.models().iter().find_map(|m| {
            m.mesh.as_ref().map(|mesh| mesh.bbox).or_else(|| {
                crate::state::job::session_polygons_bbox(
                    m.polygons.as_deref().map(|v| v.as_slice()),
                )
            })
        })
    }

    pub(crate) fn handle_setup_two_sided(&mut self) {
        let has_flipped = self
            .state
            .session
            .list_setups()
            .iter()
            .any(|s| s.face_up == FaceUp::Bottom);
        if !has_flipped {
            let next_id = self.state.session.list_setups().len() + 1;
            let name = format!("Setup {next_id}");
            self.state.session.add_setup(name, FaceUp::Bottom);
        }

        // Key the pins to the flip the project actually programs, not to
        // an assumption. Only an in-plane flip keeps the XY footprint the
        // pins are dimensioned in; the edge-up orientations are refused
        // by the core placer rather than guessed at.
        let flip_face = self
            .state
            .session
            .list_setups()
            .iter()
            .map(|s| s.face_up)
            .find(|f| *f != FaceUp::Top)
            .unwrap_or(FaceUp::Bottom);

        let pin_diameter = self.pin_drill_tool_diameter();
        let model_bbox = self.first_model_bbox();
        let plan = match pin_diameter {
            Some(d) => self
                .state
                .session
                .stock_config()
                .plan_keyed_pins(flip_face, model_bbox.as_ref(), d)
                .map_err(|e| e.to_string()),
            // Sizing the pin from the tool is the point: with no tool
            // there is no honest diameter, and a guess is exactly what
            // the hardcoded 6.0 was.
            None => Err(NO_PIN_TOOL_MESSAGE.to_owned()),
        };

        let mut refusal: Option<String> = None;
        {
            let stock = self.state.session.stock_mut();
            if stock.alignment_pins.is_empty() {
                match plan {
                    Ok(pins) => stock.alignment_pins.extend(pins),
                    // Refuse rather than emit a placement that hangs off
                    // the blank or seats four ways: the operator would
                    // find out at the flip, with the part already cut.
                    Err(message) => refusal = Some(message),
                }
            }
            // `flip_axis` is a CACHE of the setup's face_up, never an
            // independent control — a stored axis that disagrees with the
            // setups is exactly how this shipped with a null axis beside a
            // Bottom setup. Every validation reads `face_up` instead.
            //
            // It is cached ONLY once pins exist for it to describe. On a
            // refusal it stays `None`, and that is load-bearing: the setup
            // panel suppresses its "Add alignment pins for this flip"
            // offer when a flip axis is set, so caching it here would
            // leave a flipped setup with zero pins reading as configured
            // and no affordance left to fix it.
            stock.flip_axis = if stock.alignment_pins.is_empty() {
                None
            } else {
                FlipAxis::from_face_up(flip_face)
            };
        }

        if let Some(message) = refusal {
            self.push_notification(message, super::super::Severity::Error);
        }

        // Whatever pins are stored now — freshly placed or pre-existing —
        // get judged against the flip they have to register.
        for warning in self
            .state
            .session
            .stock_config()
            .validate_pins_for_flip(flip_face)
            .warnings()
        {
            tracing::warn!("{warning}");
            self.push_notification(warning, super::super::Severity::Warning);
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
        if let Some(idx) = self
            .state
            .session
            .list_setups()
            .iter()
            .position(|s| s.id == setup_id.0)
        {
            let _ = self.state.session.rename_setup(idx, name);
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

        if let Some(idx) = self
            .state
            .session
            .list_setups()
            .iter()
            .position(|s| s.id == setup_id.0)
        {
            let fixture = Fixture {
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
            };
            let _ = self.state.session.add_fixture(idx, fixture);
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
        if let Some(idx) = self
            .state
            .session
            .list_setups()
            .iter()
            .position(|s| s.id == setup_id.0)
        {
            let _ = self.state.session.remove_fixture(idx, fixture_id);
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

        if let Some(idx) = self
            .state
            .session
            .list_setups()
            .iter()
            .position(|s| s.id == setup_id.0)
        {
            let zone = KeepOutZone {
                id: keep_out_id,
                name: format!("Keep-Out {}", keep_out_id.0 + 1),
                enabled: true,
                origin_x: 0.0,
                origin_y: 0.0,
                size_x: 20.0,
                size_y: 20.0,
            };
            let _ = self.state.session.add_keep_out(idx, zone);
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
        if let Some(idx) = self
            .state
            .session
            .list_setups()
            .iter()
            .position(|s| s.id == setup_id.0)
        {
            let _ = self.state.session.remove_keep_out(idx, keep_out_id);
            if self.state.selection == Selection::KeepOut(setup_id, keep_out_id) {
                self.state.selection = Selection::Setup(setup_id);
            }
            self.pending_upload = true;
            self.state.gui.mark_edited();
        }
    }

    // ── Model helpers ────────────────────────────────────────────────────

    pub(crate) fn handle_remove_model(&mut self, model_id: ModelId) {
        // Check if any toolpath still references this model
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
        } else if let Some(idx) = self
            .state
            .session
            .models()
            .iter()
            .position(|m| m.id == model_id.0)
        {
            let _ = self.state.session.remove_model(idx);
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
        if auto_from_model && let Some(bbox) = self.first_model_bbox() {
            self.state.session.update_stock_from_bbox(&bbox);
        } else {
            // No bbox to apply, but stock fields may still have been mutated
            // upstream — clear stale simulation just in case.
            self.state.session.invalidate_stock();
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
        let existing: Option<(usize, rs_cam_core::ToolpathId)> = self
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
                // F1 (2026-06-10): run the suggest funnel instead of raw
                // defaults — `..Default::default()` shipped the hardcoded
                // 300 mm/min feed, which on a Ø6 pin drill is exactly the
                // SolidWood rubbing floor (50 mm/min per mm Ø). Mirrors
                // the add-toolpath path in `events/toolpath.rs`. On a
                // suggest refusal, fall back to defaults rather than
                // skipping the auto-create.
                let stock_bbox = self.state.session.stock_bbox();
                let stock_padding = self.state.session.stock_config().padding;
                let stock_ctx = rs_cam_core::feeds::suggest::StockContext::from_stock_bbox(
                    stock_bbox,
                    stock_padding,
                );
                let suggested = self
                    .state
                    .session
                    .tools()
                    .iter()
                    .find(|t| t.id.0 == tool_id)
                    .and_then(|tool| {
                        rs_cam_core::feeds::suggest::suggest_params(
                            rs_cam_core::feeds::suggest::SuggestParamsInput {
                                op_type: crate::state::toolpath::OperationType::AlignmentPinDrill,
                                tool,
                                machine: self.state.session.machine(),
                                material: &self.state.session.stock_config().material,
                                workholding: self.state.session.stock_config().workholding_rigidity,
                                lut: rs_cam_core::feeds::embedded_vendor_lut(),
                                stock_ctx: &stock_ctx,
                                spindle_strategy: rs_cam_core::feeds::SpindleStrategy::default(),
                                context: rs_cam_core::feeds::suggest::SuggestContext::default(),
                            },
                        )
                        .ok()
                    });
                let (cfg, feeds_provenance) = match suggested {
                    Some(s) => match s.operation {
                        OperationConfig::AlignmentPinDrill(mut c) => {
                            c.holes = holes;
                            (c, s.provenance)
                        }
                        // suggest_params builds from the requested op_type,
                        // so this arm is unreachable; defaults keep it total.
                        _ => (
                            AlignmentPinDrillConfig {
                                holes,
                                ..Default::default()
                            },
                            rs_cam_core::feeds::FeedsProvenance::default(),
                        ),
                    },
                    None => {
                        tracing::warn!(
                            "pin-drill auto-create: suggest refused; falling back to defaults"
                        );
                        (
                            AlignmentPinDrillConfig {
                                holes,
                                ..Default::default()
                            },
                            rs_cam_core::feeds::FeedsProvenance::default(),
                        )
                    }
                };
                let tc = rs_cam_core::session::ToolpathConfig {
                    id: rs_cam_core::ToolpathId(0), // will be assigned by session
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
                    rest_analysis: crate::state::toolpath::RestAnalysisConfig::default(),
                    stock_source: crate::state::toolpath::StockSource::Fresh,
                    coolant: rs_cam_core::gcode::CoolantMode::Off,
                    face_selection: None,
                    debug_options: rs_cam_core::debug_trace::ToolpathDebugOptions::default(),
                    feeds_provenance,
                };
                let _ = self.state.session.add_toolpath(setup_idx, tc);
            }
        } else if !has_pins {
            // Remove pin drill toolpath if pins were all deleted.
            if let Some((idx, _id)) = existing {
                let _ = self.state.session.remove_toolpath(idx);
            }
        } else if let Some((idx, id)) = existing {
            // Pins exist and toolpath exists — update hole positions and mark stale.
            let new_holes: Vec<[f64; 2]> = self
                .state
                .session
                .stock_config()
                .alignment_pins
                .iter()
                .map(|p| [p.x, p.y])
                .collect();
            let _ = self
                .state
                .session
                .set_alignment_pin_drill_holes(idx, new_holes);
            // Mark stale in GUI runtime
            if let Some(rt) = self.state.gui.toolpath_rt.get_mut(&id) {
                rt.result = None;
                rt.stale_since = Some(std::time::Instant::now());
            }
        }
    }
}
