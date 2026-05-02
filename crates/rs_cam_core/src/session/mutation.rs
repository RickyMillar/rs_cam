//! CRUD mutation methods on [`ProjectSession`].

use tracing::instrument;

use crate::compute::catalog::OperationConfig;
use crate::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig};
use crate::compute::stock_config::{FixtureId, KeepOutId, StockConfig};
use crate::compute::tool_config::{ToolConfig, ToolId};
use crate::compute::transform::FaceUp;
use crate::enriched_mesh::FaceGroupId;

use super::{
    Fixture, KeepOutZone, ProjectPostConfig, ProjectSession, SessionError, SetupData,
    ToolpathConfig,
};
use crate::compute::transform::ZRotation;
use crate::geo::{BoundingBox3, P3};
use crate::polygon::Polygon2;

/// Compute a 3D bounding box from a slice of 2D polygons (SVG/DXF models).
/// Z extent is zero; `update_from_bbox` preserves stock Z for 2D models.
/// Returns `None` if all polygons are empty.
fn polygons_bbox(polygons: &[Polygon2]) -> Option<BoundingBox3> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for poly in polygons {
        for pt in poly
            .exterior
            .iter()
            .chain(poly.holes.iter().flat_map(|h| h.iter()))
        {
            if pt.x < min_x {
                min_x = pt.x;
            }
            if pt.y < min_y {
                min_y = pt.y;
            }
            if pt.x > max_x {
                max_x = pt.x;
            }
            if pt.y > max_y {
                max_y = pt.y;
            }
        }
    }
    if !min_x.is_finite() {
        return None;
    }
    Some(BoundingBox3 {
        min: P3::new(min_x, min_y, 0.0),
        max: P3::new(max_x, max_y, 0.0),
    })
}

impl ProjectSession {
    // ── Toolpath CRUD ─────────────────────────────────────────────

    /// Add a toolpath to the specified setup, returning its index in
    /// `toolpath_configs`.
    #[instrument(skip(self, config))]
    pub fn add_toolpath(
        &mut self,
        setup_index: usize,
        mut config: ToolpathConfig,
    ) -> Result<usize, SessionError> {
        let setup = self
            .setups
            .get_mut(setup_index)
            .ok_or(SessionError::SetupNotFound(setup_index))?;

        // Assign a fresh ID
        config.id = self.next_toolpath_id;
        self.next_toolpath_id += 1;

        let tp_index = self.toolpath_configs.len();
        self.toolpath_configs.push(config);
        setup.toolpath_indices.push(tp_index);

        // Adding a toolpath invalidates simulation
        self.simulation = None;

        Ok(tp_index)
    }

    /// Remove a toolpath by its index in `toolpath_configs`.
    ///
    /// Updates all setup `toolpath_indices` so that indices above the
    /// removed one are shifted down by one.
    #[instrument(skip(self))]
    pub fn remove_toolpath(&mut self, index: usize) -> Result<(), SessionError> {
        if index >= self.toolpath_configs.len() {
            return Err(SessionError::ToolpathNotFound(index));
        }

        self.toolpath_configs.remove(index);
        self.results.remove(&index);

        // Rebuild every setup's toolpath_indices: remove the index,
        // then decrement any index above it.
        for setup in &mut self.setups {
            setup.toolpath_indices.retain(|&i| i != index);
            for idx in &mut setup.toolpath_indices {
                if *idx > index {
                    *idx -= 1;
                }
            }
        }

        // Re-key cached results whose index shifted
        let mut new_results = std::collections::HashMap::new();
        for (k, v) in self.results.drain() {
            if k > index {
                new_results.insert(k - 1, v);
            } else {
                new_results.insert(k, v);
            }
        }
        self.results = new_results;

        self.simulation = None;
        Ok(())
    }

    /// Move a toolpath from `from_index` to `to_index` within the same setup.
    ///
    /// Both indices refer to positions in the session-level `toolpath_configs`
    /// vec. The toolpath must belong to the same setup as determined by the
    /// current `toolpath_indices`.
    #[instrument(skip(self))]
    pub fn reorder_toolpath(
        &mut self,
        from_index: usize,
        to_index: usize,
    ) -> Result<(), SessionError> {
        if from_index >= self.toolpath_configs.len() {
            return Err(SessionError::ToolpathNotFound(from_index));
        }
        if to_index >= self.toolpath_configs.len() {
            return Err(SessionError::ToolpathNotFound(to_index));
        }
        if from_index == to_index {
            return Ok(());
        }

        // Swap only the display order within the setup that owns both
        // toolpaths. `toolpath_configs` and the results cache are keyed by
        // stable global indices and must not move.
        for setup in &mut self.setups {
            let pos_from = setup.toolpath_indices.iter().position(|&i| i == from_index);
            let pos_to = setup.toolpath_indices.iter().position(|&i| i == to_index);
            if let (Some(pf), Some(pt)) = (pos_from, pos_to) {
                setup.toolpath_indices.swap(pf, pt);
                break;
            }
        }

        self.simulation = None;
        Ok(())
    }

    /// Enable or disable a toolpath.
    #[instrument(skip(self))]
    pub fn set_toolpath_enabled(
        &mut self,
        index: usize,
        enabled: bool,
    ) -> Result<(), SessionError> {
        let tc = self
            .toolpath_configs
            .get_mut(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        tc.enabled = enabled;
        self.simulation = None;
        Ok(())
    }

    /// Replace the dressup config for a toolpath, invalidating its cached result.
    #[instrument(skip(self, dressups))]
    pub fn set_dressup_config(
        &mut self,
        index: usize,
        mut dressups: DressupConfig,
    ) -> Result<(), SessionError> {
        let tc = self
            .toolpath_configs
            .get_mut(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        // Enforce the per-operation dressup invariant so incompatible
        // combinations can't be introduced via this API.
        dressups.normalize_for_op(tc.operation.op_type());
        tc.dressups = dressups;
        self.results.remove(&index);
        self.simulation = None;
        Ok(())
    }

    /// Update a single dressup field by merging a JSON patch onto the existing
    /// [`DressupConfig`]. Only the specified field is changed. Invalidates
    /// the cached result.
    #[instrument(skip(self, value))]
    pub fn set_dressup_field(
        &mut self,
        index: usize,
        key: &str,
        value: serde_json::Value,
    ) -> Result<(), SessionError> {
        let tc = self
            .toolpath_configs
            .get_mut(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        let mut merged = serde_json::to_value(&tc.dressups)
            .map_err(|e| SessionError::InvalidParam(format!("dressup serialize: {e}")))?;
        match merged.as_object_mut() {
            Some(obj) => {
                if !obj.contains_key(key) {
                    return Err(SessionError::InvalidParam(format!(
                        "unknown dressup field '{key}'"
                    )));
                }
                obj.insert(key.to_owned(), value);
            }
            None => {
                return Err(SessionError::InvalidParam(
                    "dressup config is not an object".to_owned(),
                ));
            }
        }
        let mut new_cfg: DressupConfig = serde_json::from_value(merged)
            .map_err(|e| SessionError::InvalidParam(format!("dressup patch: {e}")))?;
        // Enforce the per-operation dressup invariant on every patch.
        new_cfg.normalize_for_op(tc.operation.op_type());
        tc.dressups = new_cfg;
        self.results.remove(&index);
        self.simulation = None;
        Ok(())
    }

    /// Set the stock_source for a toolpath (Fresh vs FromRemainingStock).
    /// Invalidates the cached result.
    #[instrument(skip(self))]
    pub fn set_stock_source(
        &mut self,
        index: usize,
        source: crate::compute::config::StockSource,
    ) -> Result<(), SessionError> {
        let tc = self
            .toolpath_configs
            .get_mut(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        tc.stock_source = source;
        self.results.remove(&index);
        self.simulation = None;
        Ok(())
    }

    /// Replace the heights config for a toolpath, invalidating its cached result.
    #[instrument(skip(self, heights))]
    pub fn set_heights_config(
        &mut self,
        index: usize,
        heights: HeightsConfig,
    ) -> Result<(), SessionError> {
        let tc = self
            .toolpath_configs
            .get_mut(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        tc.heights = heights;
        self.results.remove(&index);
        self.simulation = None;
        Ok(())
    }

    /// Replace the boundary config for a toolpath, invalidating its cached result.
    #[instrument(skip(self, boundary))]
    pub fn set_boundary_config(
        &mut self,
        index: usize,
        boundary: BoundaryConfig,
    ) -> Result<(), SessionError> {
        let tc = self
            .toolpath_configs
            .get_mut(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        tc.boundary = boundary;
        self.results.remove(&index);
        self.simulation = None;
        Ok(())
    }

    // ── Model CRUD ────────────────────────────────────────────────

    /// Add a model and return its ID.
    ///
    /// When `stock.auto_from_model` is enabled, the stock dimensions are
    /// auto-updated from the new model's bounding box — mesh bbox for
    /// STL/STEP models, polygon bbox for SVG/DXF. Without this, MCP
    /// users who called `import_model` after loading a project saw the
    /// stock stay at its pre-import size (see F-13 in the April review).
    ///
    /// For 2D polygon models (zero Z extent), `update_from_bbox`
    /// preserves the existing stock Z dimension rather than collapsing
    /// it to 0.
    #[instrument(skip(self, model))]
    pub fn add_model(&mut self, mut model: super::LoadedModel) -> usize {
        model.id = self.next_model_id;
        self.next_model_id += 1;
        let id = model.id;

        // Compute bbox before moving the model into self.models.
        let auto_from_model = self.stock.auto_from_model;
        let model_bbox = if auto_from_model {
            model.mesh.as_ref().map(|mesh| mesh.bbox).or_else(|| {
                model
                    .polygons
                    .as_ref()
                    .and_then(|polys| polygons_bbox(polys))
            })
        } else {
            None
        };

        self.models.push(model);

        if let Some(bbox) = model_bbox {
            self.stock.update_from_bbox(&bbox);
        }

        id
    }

    /// Remove a model by index.
    #[instrument(skip(self))]
    pub fn remove_model(&mut self, index: usize) -> Result<(), SessionError> {
        if index >= self.models.len() {
            return Err(SessionError::MissingGeometry(format!(
                "Model index {index} not found"
            )));
        }
        self.models.remove(index);
        Ok(())
    }

    // ── Tool CRUD ─────────────────────────────────────────────────

    /// Add a tool and return its index in the tools vec.
    #[instrument(skip(self, config))]
    pub fn add_tool(&mut self, mut config: ToolConfig) -> usize {
        config.id = ToolId(self.next_tool_id);
        self.next_tool_id += 1;
        let idx = self.tools.len();
        self.tools.push(config);
        idx
    }

    /// Remove a tool by index. Errors if any toolpath still references it.
    #[instrument(skip(self))]
    pub fn remove_tool(&mut self, index: usize) -> Result<(), SessionError> {
        let tool = self
            .tools
            .get(index)
            .ok_or(SessionError::ToolNotFound(ToolId(index)))?;
        let tool_raw_id = tool.id.0;

        // Check no toolpaths reference this tool
        let in_use = self
            .toolpath_configs
            .iter()
            .any(|tc| tc.tool_id == tool_raw_id);
        if in_use {
            return Err(SessionError::ToolInUse(ToolId(tool_raw_id)));
        }

        self.tools.remove(index);
        Ok(())
    }

    // ── Setup CRUD ────────────────────────────────────────────────

    /// Add a new setup and return its index.
    #[instrument(skip(self))]
    pub fn add_setup(&mut self, name: String, face_up: FaceUp) -> usize {
        let id = self.next_setup_id;
        self.next_setup_id += 1;
        let idx = self.setups.len();
        self.setups.push(SetupData {
            id,
            name,
            face_up,
            z_rotation: ZRotation::default(),
            fixtures: Vec::new(),
            keep_out_zones: Vec::new(),
            toolpath_indices: Vec::new(),
        });
        idx
    }

    /// Remove a setup by index. Errors if the setup still has toolpaths.
    #[instrument(skip(self))]
    pub fn remove_setup(&mut self, index: usize) -> Result<(), SessionError> {
        let setup = self
            .setups
            .get(index)
            .ok_or(SessionError::SetupNotFound(index))?;
        if !setup.toolpath_indices.is_empty() {
            return Err(SessionError::SetupHasToolpaths(index));
        }
        self.setups.remove(index);
        Ok(())
    }

    // ── Cross-setup moves ─────────────────────────────────────────

    /// Move a toolpath from its current setup to a different setup.
    ///
    /// The cached toolpath result is invalidated because the setup transform
    /// may have changed (e.g. top → bottom orientation).
    #[instrument(skip(self))]
    pub fn move_toolpath_to_setup(
        &mut self,
        tp_index: usize,
        target_setup_index: usize,
    ) -> Result<(), SessionError> {
        if tp_index >= self.toolpath_configs.len() {
            return Err(SessionError::ToolpathNotFound(tp_index));
        }
        if target_setup_index >= self.setups.len() {
            return Err(SessionError::SetupNotFound(target_setup_index));
        }

        // Remove from whichever setup currently owns this toolpath
        for setup in &mut self.setups {
            setup.toolpath_indices.retain(|&i| i != tp_index);
        }

        // SAFETY: target_setup_index bounds-checked above
        #[allow(clippy::indexing_slicing)]
        self.setups[target_setup_index]
            .toolpath_indices
            .push(tp_index);

        self.results.remove(&tp_index);
        self.simulation = None;
        Ok(())
    }

    // ── Toolpath config updates ──────────────────────────────────

    /// Set the BREP face selection for a toolpath, invalidating its cached result.
    #[instrument(skip(self, face_ids))]
    pub fn set_face_selection(
        &mut self,
        index: usize,
        face_ids: Option<Vec<FaceGroupId>>,
    ) -> Result<(), SessionError> {
        let tc = self
            .toolpath_configs
            .get_mut(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        tc.face_selection = face_ids;
        self.results.remove(&index);
        self.simulation = None;
        Ok(())
    }

    /// Update the alignment pin drill holes for a toolpath.
    ///
    /// Errors if the toolpath's operation is not `AlignmentPinDrill`.
    #[instrument(skip(self, holes))]
    pub fn set_alignment_pin_drill_holes(
        &mut self,
        index: usize,
        holes: Vec<[f64; 2]>,
    ) -> Result<(), SessionError> {
        let tc = self
            .toolpath_configs
            .get_mut(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        match tc.operation {
            OperationConfig::AlignmentPinDrill(ref mut cfg) => {
                cfg.holes = holes;
            }
            _ => {
                return Err(SessionError::InvalidParam(
                    "Toolpath is not an AlignmentPinDrill operation".to_owned(),
                ));
            }
        }
        self.results.remove(&index);
        self.simulation = None;
        Ok(())
    }

    // ── Setup mutations ──────────────────────────────────────────

    /// Rename a setup. This is metadata-only and does not affect compute.
    #[instrument(skip(self))]
    pub fn rename_setup(&mut self, index: usize, name: String) -> Result<(), SessionError> {
        let setup = self
            .setups
            .get_mut(index)
            .ok_or(SessionError::SetupNotFound(index))?;
        setup.name = name;
        Ok(())
    }

    /// Add a fixture to a setup, invalidating all toolpath results in that setup.
    #[instrument(skip(self, fixture))]
    pub fn add_fixture(
        &mut self,
        setup_index: usize,
        fixture: Fixture,
    ) -> Result<(), SessionError> {
        let setup = self
            .setups
            .get_mut(setup_index)
            .ok_or(SessionError::SetupNotFound(setup_index))?;
        setup.fixtures.push(fixture);
        let indices: Vec<usize> = setup.toolpath_indices.clone();
        for &tp_idx in &indices {
            self.results.remove(&tp_idx);
        }
        self.simulation = None;
        Ok(())
    }

    /// Remove a fixture from a setup by its ID.
    #[instrument(skip(self))]
    pub fn remove_fixture(
        &mut self,
        setup_index: usize,
        fixture_id: FixtureId,
    ) -> Result<(), SessionError> {
        let setup = self
            .setups
            .get_mut(setup_index)
            .ok_or(SessionError::SetupNotFound(setup_index))?;
        setup.fixtures.retain(|f| f.id != fixture_id);
        let indices: Vec<usize> = setup.toolpath_indices.clone();
        for &tp_idx in &indices {
            self.results.remove(&tp_idx);
        }
        self.simulation = None;
        Ok(())
    }

    /// Add a keep-out zone to a setup, invalidating all toolpath results in that setup.
    #[instrument(skip(self, zone))]
    pub fn add_keep_out(
        &mut self,
        setup_index: usize,
        zone: KeepOutZone,
    ) -> Result<(), SessionError> {
        let setup = self
            .setups
            .get_mut(setup_index)
            .ok_or(SessionError::SetupNotFound(setup_index))?;
        setup.keep_out_zones.push(zone);
        let indices: Vec<usize> = setup.toolpath_indices.clone();
        for &tp_idx in &indices {
            self.results.remove(&tp_idx);
        }
        self.simulation = None;
        Ok(())
    }

    /// Remove a keep-out zone from a setup by its ID.
    #[instrument(skip(self))]
    pub fn remove_keep_out(
        &mut self,
        setup_index: usize,
        zone_id: KeepOutId,
    ) -> Result<(), SessionError> {
        let setup = self
            .setups
            .get_mut(setup_index)
            .ok_or(SessionError::SetupNotFound(setup_index))?;
        setup.keep_out_zones.retain(|z| z.id != zone_id);
        let indices: Vec<usize> = setup.toolpath_indices.clone();
        for &tp_idx in &indices {
            self.results.remove(&tp_idx);
        }
        self.simulation = None;
        Ok(())
    }

    // ── Invalidation helpers ─────────────────────────────────────
    //
    // For immediate-mode UI panels that mutate session fields in-place
    // (via `stock_mut()` / `machine_mut()` / `tools_mut()`), these methods
    // ensure the cache is properly cleared after the edit completes.

    /// Invalidate cached simulation after stock config was mutated in-place.
    #[instrument(skip(self))]
    pub fn invalidate_stock(&mut self) {
        self.simulation = None;
    }

    /// Invalidate cached simulation after machine profile was mutated in-place.
    #[instrument(skip(self))]
    pub fn invalidate_machine(&mut self) {
        self.simulation = None;
    }

    /// Invalidate cached results for all toolpaths that reference a given tool.
    #[instrument(skip(self))]
    pub fn invalidate_tool(&mut self, tool_id: usize) {
        for (idx, tc) in self.toolpath_configs.iter().enumerate() {
            if tc.tool_id == tool_id {
                self.results.remove(&idx);
            }
        }
        self.simulation = None;
    }

    // ── Global config ─────────────────────────────────────────────

    /// Replace the stock configuration, invalidating simulation.
    #[instrument(skip(self, stock))]
    pub fn set_stock_config(&mut self, stock: StockConfig) {
        self.stock = stock;
        self.simulation = None;
    }

    /// Update stock dimensions from a bounding box (used by `auto_from_model`
    /// to size the stock around imported geometry). Invalidates simulation.
    ///
    /// For 2D polygon bboxes (zero Z extent), [`StockConfig::update_from_bbox`]
    /// preserves the existing Z so attaching an SVG/DXF doesn't collapse stock
    /// thickness — see F-13 in the April 2026 review.
    #[instrument(skip(self))]
    pub fn update_stock_from_bbox(&mut self, bbox: &BoundingBox3) {
        self.stock.update_from_bbox(bbox);
        self.simulation = None;
    }

    /// Add an alignment pin, deduping against existing pins within 0.01mm.
    ///
    /// Returns `true` if the pin was added, `false` if a duplicate was skipped.
    #[instrument(skip(self))]
    pub fn add_alignment_pin(&mut self, x: f64, y: f64, diameter: f64) -> bool {
        const PIN_DEDUP_EPSILON_MM: f64 = 0.01;
        let exists = self
            .stock
            .alignment_pins
            .iter()
            .any(|p| (p.x - x).abs() < PIN_DEDUP_EPSILON_MM && (p.y - y).abs() < PIN_DEDUP_EPSILON_MM);
        if exists {
            return false;
        }
        self.stock
            .alignment_pins
            .push(crate::compute::stock_config::AlignmentPin::new(
                x, y, diameter,
            ));
        self.simulation = None;
        true
    }

    /// Remove an alignment pin by index. Returns `Err` if out of bounds.
    #[instrument(skip(self))]
    pub fn remove_alignment_pin(&mut self, index: usize) -> Result<(), SessionError> {
        if index >= self.stock.alignment_pins.len() {
            return Err(SessionError::InvalidParam(format!(
                "alignment pin index {index} out of range ({} pins)",
                self.stock.alignment_pins.len()
            )));
        }
        self.stock.alignment_pins.remove(index);
        self.simulation = None;
        Ok(())
    }

    /// Replace the post-processor configuration, invalidating simulation.
    #[instrument(skip(self, post))]
    pub fn set_post_config(&mut self, post: ProjectPostConfig) {
        self.post = post;
        self.simulation = None;
    }

    /// Replace the machine profile.
    #[instrument(skip(self, machine))]
    pub fn set_machine(&mut self, machine: crate::machine::MachineProfile) {
        self.machine = machine;
    }

    /// Replace the full tools list.
    ///
    /// Invalidates simulation (tool geometry changes affect material removal).
    #[instrument(skip(self, tools))]
    pub fn replace_tools(&mut self, tools: Vec<ToolConfig>) {
        self.tools = tools;
        // Update the next-ID counter so newly added tools don't collide.
        self.next_tool_id = self.tools.iter().map(|t| t.id.0 + 1).max().unwrap_or(0);
        self.simulation = None;
    }

    /// Replace a single toolpath configuration by its index in
    /// `toolpath_configs`, invalidating its cached result and simulation.
    #[instrument(skip(self, config))]
    pub fn replace_toolpath_config(
        &mut self,
        index: usize,
        config: ToolpathConfig,
    ) -> Result<(), SessionError> {
        let slot = self
            .toolpath_configs
            .get_mut(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        *slot = config;
        self.results.remove(&index);
        self.simulation = None;
        Ok(())
    }

    /// Apply an undo/redo snapshot of toolpath parameters to a toolpath at
    /// `index`. Restores the operation config, dressup config, and BREP face
    /// selection in one shot, invalidating the cached result and simulation.
    ///
    /// This is the session-API path used by the GUI undo stack to roll a
    /// toolpath back to a prior parameter state.
    #[instrument(skip(self, operation, dressups, face_selection))]
    pub fn apply_toolpath_param_snapshot(
        &mut self,
        index: usize,
        operation: OperationConfig,
        dressups: DressupConfig,
        face_selection: Option<Vec<FaceGroupId>>,
    ) -> Result<(), SessionError> {
        let tc = self
            .toolpath_configs
            .get_mut(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        tc.operation = operation;
        tc.dressups = dressups;
        tc.face_selection = face_selection;
        self.results.remove(&index);
        self.simulation = None;
        Ok(())
    }

    /// Wholesale replace all setups and toolpath configs from an external
    /// source (e.g. GUI's `JobState`).  This is the bulk-sync path used by
    /// `sync_session_from_job`.
    ///
    /// The caller is responsible for building valid `SetupData` and
    /// `ToolpathConfig` vecs whose `toolpath_indices` are consistent.
    #[instrument(skip(self, setups, toolpath_configs))]
    pub fn replace_setups_and_toolpaths(
        &mut self,
        setups: Vec<SetupData>,
        toolpath_configs: Vec<ToolpathConfig>,
    ) {
        self.setups = setups;
        self.toolpath_configs = toolpath_configs;
        // Update the next-ID counters.
        self.next_toolpath_id = self
            .toolpath_configs
            .iter()
            .map(|tc| tc.id + 1)
            .max()
            .unwrap_or(0);
        self.next_setup_id = self.setups.iter().map(|s| s.id + 1).max().unwrap_or(0);
        // Invalidate all cached results — the indices may have shifted.
        self.results.clear();
        self.simulation = None;
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use crate::compute::catalog::OperationConfig;
    use crate::compute::config::ToolpathStats;
    use crate::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig};
    use crate::compute::operation_configs::{AlignmentPinDrillConfig, PocketConfig};
    use crate::compute::stock_config::FixtureId;
    use crate::debug_trace::ToolpathDebugOptions;
    use crate::gcode::CoolantMode;
    use crate::session::{Fixture, FixtureKind, KeepOutZone, ToolpathComputeResult};

    fn make_session() -> ProjectSession {
        ProjectSession::new_empty()
    }

    fn make_tc(tool_id: usize, model_id: usize) -> ToolpathConfig {
        ToolpathConfig {
            id: 0,
            name: "test".to_owned(),
            enabled: true,
            operation: OperationConfig::Pocket(PocketConfig::default()),
            dressups: DressupConfig::default(),
            heights: HeightsConfig::default(),
            tool_id,
            model_id,
            pre_gcode: None,
            post_gcode: None,
            boundary: BoundaryConfig::default(),
            boundary_inherit: true,
            stock_source: crate::session::StockSource::Fresh,
            coolant: CoolantMode::Off,
            face_selection: None,
            feeds_auto: crate::compute::config::FeedsAutoMode::default(),
            debug_options: ToolpathDebugOptions::default(),
        }
    }

    fn make_tool() -> ToolConfig {
        ToolConfig::new_default(ToolId(0), crate::compute::tool_config::ToolType::EndMill)
    }

    fn fake_result() -> ToolpathComputeResult {
        ToolpathComputeResult {
            toolpath: Arc::new(crate::toolpath::Toolpath::new()),
            stats: ToolpathStats::default(),
            debug_trace: None,
            semantic_trace: None,
        }
    }

    // ── Toolpath CRUD ────────────────────────────────────────────

    #[test]
    fn add_toolpath_assigns_id_and_updates_setup() {
        let mut s = make_session();
        let tool_idx = s.add_tool(make_tool());
        assert_eq!(tool_idx, 0);

        let tc = make_tc(s.tools()[0].id.0, 0);
        let idx = s.add_toolpath(0, tc).unwrap();
        assert_eq!(idx, 0);
        assert_eq!(s.toolpath_configs()[0].id, 0);
        assert_eq!(s.list_setups()[0].toolpath_indices, vec![0]);

        let tc2 = make_tc(s.tools()[0].id.0, 0);
        let idx2 = s.add_toolpath(0, tc2).unwrap();
        assert_eq!(idx2, 1);
        assert_eq!(s.toolpath_configs()[1].id, 1);
        assert_eq!(s.list_setups()[0].toolpath_indices, vec![0, 1]);
    }

    #[test]
    fn add_toolpath_invalid_setup() {
        let mut s = make_session();
        let tc = make_tc(0, 0);
        let result = s.add_toolpath(99, tc);
        assert!(matches!(result, Err(SessionError::SetupNotFound(99))));
    }

    #[test]
    fn add_toolpath_invalidates_simulation() {
        let mut s = make_session();
        s.add_tool(make_tool());
        let tc = make_tc(s.tools()[0].id.0, 0);
        s.add_toolpath(0, tc).unwrap();
        // simulation is cleared (set to None) by add_toolpath
        assert!(s.simulation.is_none());
    }

    #[test]
    fn remove_toolpath_shifts_indices() {
        let mut s = make_session();
        s.add_tool(make_tool());
        let tool_id = s.tools()[0].id.0;

        s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
        s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
        s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
        assert_eq!(s.list_setups()[0].toolpath_indices, vec![0, 1, 2]);

        // Add cached result for index 2
        s.results.insert(2, fake_result());

        s.remove_toolpath(0).unwrap();

        // Setup indices shifted: [1, 2] → [0, 1]
        assert_eq!(s.list_setups()[0].toolpath_indices, vec![0, 1]);
        // Result for old index 2 should now be at index 1
        assert!(s.results.contains_key(&1));
        assert!(!s.results.contains_key(&2));
    }

    #[test]
    fn remove_toolpath_not_found() {
        let mut s = make_session();
        assert!(matches!(
            s.remove_toolpath(0),
            Err(SessionError::ToolpathNotFound(0))
        ));
    }

    #[test]
    fn reorder_toolpath_swaps() {
        let mut s = make_session();
        s.add_tool(make_tool());
        let tool_id = s.tools()[0].id.0;

        s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
        s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();

        let id_0 = s.toolpath_configs()[0].id;
        let id_1 = s.toolpath_configs()[1].id;

        s.reorder_toolpath(0, 1).unwrap();
        // Global config order is stable; only the setup's display order swaps.
        assert_eq!(s.toolpath_configs()[0].id, id_0);
        assert_eq!(s.toolpath_configs()[1].id, id_1);
        assert_eq!(s.list_setups()[0].toolpath_indices, vec![1, 0]);
    }

    #[test]
    fn set_toolpath_enabled_invalidates_sim() {
        let mut s = make_session();
        s.add_tool(make_tool());
        s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();
        s.set_toolpath_enabled(0, false).unwrap();
        assert!(!s.toolpath_configs()[0].enabled);
        assert!(s.simulation.is_none());
    }

    #[test]
    fn set_dressup_invalidates_result_and_sim() {
        let mut s = make_session();
        s.add_tool(make_tool());
        s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();
        s.results.insert(0, fake_result());

        s.set_dressup_config(0, DressupConfig::default()).unwrap();
        assert!(!s.results.contains_key(&0));
        assert!(s.simulation.is_none());
    }

    #[test]
    fn set_heights_invalidates_result_and_sim() {
        let mut s = make_session();
        s.add_tool(make_tool());
        s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();
        s.results.insert(0, fake_result());

        s.set_heights_config(0, HeightsConfig::default()).unwrap();
        assert!(!s.results.contains_key(&0));
        assert!(s.simulation.is_none());
    }

    #[test]
    fn set_boundary_invalidates_result_and_sim() {
        let mut s = make_session();
        s.add_tool(make_tool());
        s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();
        s.results.insert(0, fake_result());

        s.set_boundary_config(0, BoundaryConfig::default()).unwrap();
        assert!(!s.results.contains_key(&0));
        assert!(s.simulation.is_none());
    }

    // ── Tool CRUD ────────────────────────────────────────────────

    #[test]
    fn add_tool_returns_index_and_assigns_id() {
        let mut s = make_session();
        let idx = s.add_tool(make_tool());
        assert_eq!(idx, 0);
        let idx2 = s.add_tool(make_tool());
        assert_eq!(idx2, 1);
        assert_ne!(s.tools()[0].id, s.tools()[1].id);
    }

    #[test]
    fn remove_tool_in_use_errors() {
        let mut s = make_session();
        s.add_tool(make_tool());
        let tool_id = s.tools()[0].id.0;
        s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();

        let result = s.remove_tool(0);
        assert!(matches!(result, Err(SessionError::ToolInUse(_))));
    }

    #[test]
    fn remove_tool_not_in_use_succeeds() {
        let mut s = make_session();
        s.add_tool(make_tool());
        s.add_tool(make_tool());
        assert_eq!(s.tools().len(), 2);

        s.remove_tool(1).unwrap();
        assert_eq!(s.tools().len(), 1);
    }

    // ── Setup CRUD ───────────────────────────────────────────────

    #[test]
    fn add_setup_returns_index() {
        let mut s = make_session();
        // new_empty already creates setup 0
        assert_eq!(s.list_setups().len(), 1);

        let idx = s.add_setup("Setup 2".to_owned(), FaceUp::Bottom);
        assert_eq!(idx, 1);
        assert_eq!(s.list_setups().len(), 2);
        assert_eq!(s.list_setups()[1].name, "Setup 2");
        assert_eq!(s.list_setups()[1].face_up, FaceUp::Bottom);
    }

    #[test]
    fn remove_setup_with_toolpaths_errors() {
        let mut s = make_session();
        s.add_tool(make_tool());
        s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();

        let result = s.remove_setup(0);
        assert!(matches!(result, Err(SessionError::SetupHasToolpaths(0))));
    }

    #[test]
    fn remove_empty_setup_succeeds() {
        let mut s = make_session();
        s.add_setup("Extra".to_owned(), FaceUp::default());
        assert_eq!(s.list_setups().len(), 2);

        s.remove_setup(1).unwrap();
        assert_eq!(s.list_setups().len(), 1);
    }

    // ── Cross-setup move ─────────────────────────────────────────

    #[test]
    fn move_toolpath_to_setup_moves_and_invalidates() {
        let mut s = make_session();
        s.add_tool(make_tool());
        s.add_setup("Setup 2".to_owned(), FaceUp::Bottom);

        let tool_id = s.tools()[0].id.0;
        s.add_toolpath(0, make_tc(tool_id, 0)).unwrap();
        s.results.insert(0, fake_result());

        assert_eq!(s.list_setups()[0].toolpath_indices, vec![0]);
        assert!(s.list_setups()[1].toolpath_indices.is_empty());

        s.move_toolpath_to_setup(0, 1).unwrap();

        assert!(s.list_setups()[0].toolpath_indices.is_empty());
        assert_eq!(s.list_setups()[1].toolpath_indices, vec![0]);
        assert!(!s.results.contains_key(&0));
        assert!(s.simulation.is_none());
    }

    #[test]
    fn move_toolpath_invalid_target() {
        let mut s = make_session();
        s.add_tool(make_tool());
        s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();

        assert!(matches!(
            s.move_toolpath_to_setup(0, 99),
            Err(SessionError::SetupNotFound(99))
        ));
    }

    // ── Face selection ───────────────────────────────────────────

    #[test]
    fn set_face_selection_invalidates() {
        let mut s = make_session();
        s.add_tool(make_tool());
        s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();
        s.results.insert(0, fake_result());

        let faces = vec![
            crate::enriched_mesh::FaceGroupId(1),
            crate::enriched_mesh::FaceGroupId(3),
        ];
        s.set_face_selection(0, Some(faces)).unwrap();

        assert!(s.toolpath_configs()[0].face_selection.is_some());
        assert!(!s.results.contains_key(&0));
        assert!(s.simulation.is_none());
    }

    // ── Rename setup ─────────────────────────────────────────────

    #[test]
    fn rename_setup_changes_name() {
        let mut s = make_session();
        s.rename_setup(0, "New Name".to_owned()).unwrap();
        assert_eq!(s.list_setups()[0].name, "New Name");
    }

    #[test]
    fn rename_setup_not_found() {
        let mut s = make_session();
        assert!(matches!(
            s.rename_setup(99, "x".to_owned()),
            Err(SessionError::SetupNotFound(99))
        ));
    }

    // ── Fixture CRUD ─────────────────────────────────────────────

    #[test]
    fn add_fixture_invalidates_setup_toolpaths() {
        let mut s = make_session();
        s.add_tool(make_tool());
        s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();
        s.results.insert(0, fake_result());

        let fixture = Fixture {
            id: FixtureId(0),
            name: "Clamp 1".to_owned(),
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
        s.add_fixture(0, fixture).unwrap();

        assert_eq!(s.list_setups()[0].fixtures.len(), 1);
        assert!(!s.results.contains_key(&0));
        assert!(s.simulation.is_none());
    }

    #[test]
    fn remove_fixture_by_id() {
        let mut s = make_session();
        let fixture = Fixture {
            id: FixtureId(42),
            name: "Clamp".to_owned(),
            kind: FixtureKind::Clamp,
            enabled: true,
            origin_x: 0.0,
            origin_y: 0.0,
            origin_z: 0.0,
            size_x: 10.0,
            size_y: 10.0,
            size_z: 10.0,
            clearance: 1.0,
        };
        s.add_fixture(0, fixture).unwrap();
        assert_eq!(s.list_setups()[0].fixtures.len(), 1);

        s.remove_fixture(0, FixtureId(42)).unwrap();
        assert!(s.list_setups()[0].fixtures.is_empty());
    }

    // ── Keep-out CRUD ────────────────────────────────────────────

    #[test]
    fn add_keep_out_invalidates_setup_toolpaths() {
        let mut s = make_session();
        s.add_tool(make_tool());
        s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();
        s.results.insert(0, fake_result());

        let zone = KeepOutZone {
            id: crate::compute::stock_config::KeepOutId(0),
            name: "Zone 1".to_owned(),
            enabled: true,
            origin_x: 0.0,
            origin_y: 0.0,
            size_x: 20.0,
            size_y: 20.0,
        };
        s.add_keep_out(0, zone).unwrap();

        assert_eq!(s.list_setups()[0].keep_out_zones.len(), 1);
        assert!(!s.results.contains_key(&0));
        assert!(s.simulation.is_none());
    }

    #[test]
    fn remove_keep_out_by_id() {
        let mut s = make_session();
        let zone = KeepOutZone {
            id: crate::compute::stock_config::KeepOutId(7),
            name: "Zone".to_owned(),
            enabled: true,
            origin_x: 0.0,
            origin_y: 0.0,
            size_x: 10.0,
            size_y: 10.0,
        };
        s.add_keep_out(0, zone).unwrap();
        assert_eq!(s.list_setups()[0].keep_out_zones.len(), 1);

        s.remove_keep_out(0, crate::compute::stock_config::KeepOutId(7))
            .unwrap();
        assert!(s.list_setups()[0].keep_out_zones.is_empty());
    }

    // ── Alignment pin drill ──────────────────────────────────────

    #[test]
    fn set_alignment_pin_drill_holes() {
        let mut s = make_session();
        s.add_tool(make_tool());

        let tc = ToolpathConfig {
            operation: OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig::default()),
            ..make_tc(s.tools()[0].id.0, 0)
        };
        s.add_toolpath(0, tc).unwrap();
        s.results.insert(0, fake_result());

        let holes = vec![[10.0, 20.0], [90.0, 20.0]];
        s.set_alignment_pin_drill_holes(0, holes).unwrap();

        match &s.toolpath_configs()[0].operation {
            OperationConfig::AlignmentPinDrill(cfg) => {
                assert_eq!(cfg.holes.len(), 2);
                assert_eq!(cfg.holes[0], [10.0, 20.0]);
            }
            _ => panic!("Expected AlignmentPinDrill"),
        }
        assert!(!s.results.contains_key(&0));
    }

    #[test]
    fn set_alignment_pin_drill_holes_wrong_op() {
        let mut s = make_session();
        s.add_tool(make_tool());
        s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();

        let result = s.set_alignment_pin_drill_holes(0, vec![]);
        assert!(matches!(result, Err(SessionError::InvalidParam(_))));
    }

    // ── Invalidation helpers ─────────────────────────────────────

    #[test]
    fn invalidate_stock_clears_simulation() {
        let mut s = make_session();
        // simulation starts as None; invalidate should keep it None
        s.invalidate_stock();
        assert!(s.simulation.is_none());
    }

    #[test]
    fn invalidate_machine_clears_simulation() {
        let mut s = make_session();
        s.invalidate_machine();
        assert!(s.simulation.is_none());
    }

    #[test]
    fn invalidate_tool_clears_matching_results() {
        let mut s = make_session();
        s.add_tool(make_tool());
        s.add_tool(make_tool());
        let tool_id_0 = s.tools()[0].id.0;
        let tool_id_1 = s.tools()[1].id.0;

        s.add_toolpath(0, make_tc(tool_id_0, 0)).unwrap(); // idx 0
        s.add_toolpath(0, make_tc(tool_id_1, 0)).unwrap(); // idx 1
        s.add_toolpath(0, make_tc(tool_id_0, 0)).unwrap(); // idx 2

        s.results.insert(0, fake_result());
        s.results.insert(1, fake_result());
        s.results.insert(2, fake_result());

        s.invalidate_tool(tool_id_0);

        // Results for toolpaths using tool_id_0 (idx 0, 2) should be cleared
        assert!(!s.results.contains_key(&0));
        assert!(s.results.contains_key(&1)); // uses tool_id_1, unaffected
        assert!(!s.results.contains_key(&2));
    }

    // ── Global config ────────────────────────────────────────────

    #[test]
    fn set_stock_config_runs() {
        let mut s = make_session();
        s.set_stock_config(StockConfig::default());
        assert!(s.simulation.is_none());
    }

    #[test]
    fn replace_tools_updates_id_counter() {
        let mut s = make_session();
        let mut t1 = make_tool();
        t1.id = ToolId(5);
        let mut t2 = make_tool();
        t2.id = ToolId(10);
        s.replace_tools(vec![t1, t2]);

        // next_tool_id should be max(5, 10) + 1 = 11
        let new_idx = s.add_tool(make_tool());
        assert_eq!(s.tools()[new_idx].id, ToolId(11));
    }

    #[test]
    fn replace_toolpath_config_invalidates() {
        let mut s = make_session();
        s.add_tool(make_tool());
        s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();
        s.results.insert(0, fake_result());

        let new_tc = make_tc(s.tools()[0].id.0, 0);
        s.replace_toolpath_config(0, new_tc).unwrap();

        assert!(!s.results.contains_key(&0));
    }

    #[test]
    fn update_stock_from_bbox_invalidates_sim() {
        let mut s = make_session();
        let pad = s.stock_config().padding;
        let bbox = BoundingBox3 {
            min: P3::new(0.0, 0.0, 0.0),
            max: P3::new(120.0, 80.0, 25.0),
        };
        s.update_stock_from_bbox(&bbox);

        // Stock grew to fit bbox + padding (XY) and exact bbox + padding (Z).
        assert!((s.stock_config().x - (120.0 + 2.0 * pad)).abs() < 1e-6);
        assert!((s.stock_config().y - (80.0 + 2.0 * pad)).abs() < 1e-6);
        assert!((s.stock_config().z - (25.0 + pad)).abs() < 1e-6);
        // Simulation cache is cleared (mirrors the pattern in
        // set_toolpath_enabled_invalidates_sim).
        assert!(s.simulation.is_none());
    }

    #[test]
    fn apply_toolpath_param_snapshot_invalidates() {
        let mut s = make_session();
        s.add_tool(make_tool());
        s.add_toolpath(0, make_tc(s.tools()[0].id.0, 0)).unwrap();
        s.results.insert(0, fake_result());

        // Snapshot of "prior" state we want to restore.
        let snapshot_op =
            OperationConfig::AlignmentPinDrill(AlignmentPinDrillConfig::default());
        let snapshot_dress = DressupConfig::default();
        let snapshot_faces = Some(vec![crate::enriched_mesh::FaceGroupId(7)]);

        s.apply_toolpath_param_snapshot(
            0,
            snapshot_op,
            snapshot_dress,
            snapshot_faces.clone(),
        )
        .unwrap();

        match &s.toolpath_configs()[0].operation {
            OperationConfig::AlignmentPinDrill(_) => {}
            _ => panic!("operation snapshot not applied"),
        }
        assert_eq!(s.toolpath_configs()[0].face_selection, snapshot_faces);
        assert!(!s.results.contains_key(&0));
        assert!(s.simulation.is_none());
    }

    #[test]
    fn apply_toolpath_param_snapshot_not_found() {
        let mut s = make_session();
        let result = s.apply_toolpath_param_snapshot(
            99,
            OperationConfig::Pocket(PocketConfig::default()),
            DressupConfig::default(),
            None,
        );
        assert!(matches!(result, Err(SessionError::ToolpathNotFound(99))));
    }
}
