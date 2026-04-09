//! CRUD mutation methods on [`ProjectSession`].

use crate::compute::config::{BoundaryConfig, DressupConfig, HeightsConfig};
use crate::compute::stock_config::StockConfig;
use crate::compute::tool_config::{ToolConfig, ToolId};
use crate::compute::transform::FaceUp;

use super::{ProjectPostConfig, ProjectSession, SessionError, SetupData, ToolpathConfig};
use crate::compute::transform::ZRotation;

impl ProjectSession {
    // ── Toolpath CRUD ─────────────────────────────────────────────

    /// Add a toolpath to the specified setup, returning its index in
    /// `toolpath_configs`.
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

        // Swap in the flat vec
        self.toolpath_configs.swap(from_index, to_index);

        // Update setup indices to reflect the swap
        for setup in &mut self.setups {
            for idx in &mut setup.toolpath_indices {
                if *idx == from_index {
                    *idx = to_index;
                } else if *idx == to_index {
                    *idx = from_index;
                }
            }
        }

        // Swap cached results
        let r_from = self.results.remove(&from_index);
        let r_to = self.results.remove(&to_index);
        if let Some(r) = r_from {
            self.results.insert(to_index, r);
        }
        if let Some(r) = r_to {
            self.results.insert(from_index, r);
        }

        self.simulation = None;
        Ok(())
    }

    /// Enable or disable a toolpath.
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
    pub fn set_dressup_config(
        &mut self,
        index: usize,
        dressups: DressupConfig,
    ) -> Result<(), SessionError> {
        let tc = self
            .toolpath_configs
            .get_mut(index)
            .ok_or(SessionError::ToolpathNotFound(index))?;
        tc.dressups = dressups;
        self.results.remove(&index);
        self.simulation = None;
        Ok(())
    }

    /// Replace the heights config for a toolpath, invalidating its cached result.
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
    pub fn add_model(&mut self, mut model: super::LoadedModel) -> usize {
        model.id = self.next_model_id;
        self.next_model_id += 1;
        let id = model.id;
        self.models.push(model);
        id
    }

    /// Remove a model by index.
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
    pub fn add_tool(&mut self, mut config: ToolConfig) -> usize {
        config.id = ToolId(self.next_tool_id);
        self.next_tool_id += 1;
        let idx = self.tools.len();
        self.tools.push(config);
        idx
    }

    /// Remove a tool by index. Errors if any toolpath still references it.
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

    // ── Global config ─────────────────────────────────────────────

    /// Replace the stock configuration, invalidating simulation.
    pub fn set_stock_config(&mut self, stock: StockConfig) {
        self.stock = stock;
        self.simulation = None;
    }

    /// Replace the post-processor configuration, invalidating simulation.
    pub fn set_post_config(&mut self, post: ProjectPostConfig) {
        self.post = post;
        self.simulation = None;
    }

    /// Replace the machine profile.
    pub fn set_machine(&mut self, machine: crate::machine::MachineProfile) {
        self.machine = machine;
    }

    /// Replace the full tools list.
    ///
    /// Invalidates simulation (tool geometry changes affect material removal).
    pub fn replace_tools(&mut self, tools: Vec<ToolConfig>) {
        self.tools = tools;
        // Update the next-ID counter so newly added tools don't collide.
        self.next_tool_id = self.tools.iter().map(|t| t.id.0 + 1).max().unwrap_or(0);
        self.simulation = None;
    }

    /// Replace a single toolpath configuration by its index in
    /// `toolpath_configs`, invalidating its cached result and simulation.
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

    /// Wholesale replace all setups and toolpath configs from an external
    /// source (e.g. GUI's `JobState`).  This is the bulk-sync path used by
    /// `sync_session_from_job`.
    ///
    /// The caller is responsible for building valid `SetupData` and
    /// `ToolpathConfig` vecs whose `toolpath_indices` are consistent.
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
