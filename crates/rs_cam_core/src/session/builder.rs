//! Verbatim construction of a [`ProjectSession`] for a test fixture or a
//! caller that already holds every id.
//!
//! The CRUD doors ([`ProjectSession::add_tool`], [`ProjectSession::add_model`],
//! [`ProjectSession::add_toolpath`]) allocate the id themselves and overwrite
//! the one the caller supplied. A fixture that names a tool by
//! [`ToolConfig::id`] or a toolpath by [`ToolpathConfig::tool_id`] cannot use
//! those doors, so it reached for a `*_mut()` hatch instead. This builder is
//! the door for that case.

use crate::compute::stock_config::StockConfig;
use crate::compute::tool_config::ToolConfig;
use crate::machine::MachineProfile;

use super::{
    LoadedModel, ProjectPostConfig, ProjectSession, SetupData, ToolpathComputeResult,
    ToolpathConfig,
};

/// Builds a [`ProjectSession`] from parts the caller supplies.
///
/// # Contract
///
/// The builder writes every [`ToolConfig`], [`LoadedModel`], [`SetupData`]
/// and [`ToolpathConfig`] verbatim.
///
/// - It keeps each supplied id. It never renumbers.
/// - It keeps the order of each list.
/// - It never calls `StockConfig::update_from_bbox`, so a model never moves
///   the stock.
/// - It stores a pre-inserted result as given and leaves the revision map
///   empty, so [`ProjectSession::toolpath_revision`] reads `0`.
/// - It validates no index and no reference. A toolpath that names an absent
///   tool reaches [`Self::build`] unchanged.
///
/// [`Self::build`] raises each id counter above every supplied id, so the
/// first later `add_tool` / `add_model` / `add_toolpath` / `add_setup` does
/// not collide with a supplied id.
///
/// # Order rules
///
/// [`ProjectSession::new_empty`] seeds one setup, "Setup 1". The first call
/// to [`Self::setup`] replaces that seed. Each later call appends.
/// [`Self::toolpath`] appends the new index to the LAST setup. Interleave the
/// two calls to fill more than one setup.
///
/// # Example
///
/// ```ignore
/// let session = ProjectSessionBuilder::new()
///     .tool(ToolConfig::new_default(ToolId(7), ToolType::EndMill))
///     .toolpath(config)
///     .build();
/// ```
pub struct ProjectSessionBuilder {
    session: ProjectSession,
    /// True after [`Self::setup`] replaced the seeded setup.
    setup_written: bool,
}

impl Default for ProjectSessionBuilder {
    /// Same as [`Self::new`].
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectSessionBuilder {
    /// Start from an empty session.
    pub fn new() -> Self {
        Self {
            session: ProjectSession::new_empty(),
            setup_written: false,
        }
    }

    /// Set the stock configuration. The builder runs no bounding-box fit.
    pub fn stock(mut self, stock: StockConfig) -> Self {
        self.session.stock = stock;
        self
    }

    /// Set the machine profile.
    pub fn machine(mut self, machine: MachineProfile) -> Self {
        self.session.machine = machine;
        self
    }

    /// Set the post-processor configuration.
    pub fn post(mut self, post: ProjectPostConfig) -> Self {
        self.session.post = post;
        self
    }

    /// Append a tool. The builder keeps `tool.id`.
    pub fn tool(mut self, tool: ToolConfig) -> Self {
        self.session.tools.push(tool);
        self
    }

    /// Append a model. The builder keeps `model.id` and moves no stock.
    pub fn model(mut self, model: LoadedModel) -> Self {
        self.session.models.push(model);
        self
    }

    /// Add a setup. The first call replaces the seeded setup. A later call
    /// appends. The builder keeps `setup.toolpath_indices` as given.
    pub fn setup(mut self, setup: SetupData) -> Self {
        if self.setup_written {
            self.session.setups.push(setup);
        } else {
            self.setup_written = true;
            if let Some(seeded) = self.session.setups.first_mut() {
                *seeded = setup;
            } else {
                self.session.setups.push(setup);
            }
        }
        self
    }

    /// Append a toolpath configuration and list its index on the last setup.
    /// The builder keeps `config.id`.
    pub fn toolpath(mut self, config: ToolpathConfig) -> Self {
        let index = self.session.toolpath_configs.len();
        self.session.toolpath_configs.push(config);
        if let Some(setup) = self.session.setups.last_mut() {
            setup.toolpath_indices.push(index);
        }
        self
    }

    /// Store a computed result at a toolpath index. The revision map stays
    /// empty.
    pub fn result(mut self, index: usize, result: ToolpathComputeResult) -> Self {
        self.session.results.insert(index, result);
        self
    }

    /// Finish the session and raise every id counter above the supplied ids.
    pub fn build(mut self) -> ProjectSession {
        let next_tool_id = next_above(
            self.session.next_tool_id,
            self.session.tools.iter().map(|tool| tool.id.0),
        );
        let next_model_id = next_above(
            self.session.next_model_id,
            self.session.models.iter().map(|model| model.id),
        );
        let next_setup_id = next_above(
            self.session.next_setup_id,
            self.session.setups.iter().map(|setup| setup.id),
        );
        let next_toolpath_id = next_above(
            self.session.next_toolpath_id,
            self.session
                .toolpath_configs
                .iter()
                .map(|config| config.id.0),
        );
        self.session.next_tool_id = next_tool_id;
        self.session.next_model_id = next_model_id;
        self.session.next_setup_id = next_setup_id;
        self.session.next_toolpath_id = next_toolpath_id;
        self.session
    }
}

/// The first counter value above `current` and above every supplied id.
fn next_above(current: usize, ids: impl Iterator<Item = usize>) -> usize {
    ids.fold(current, |acc, id| acc.max(id.saturating_add(1)))
}
