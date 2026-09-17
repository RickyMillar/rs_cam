//! Construction of a [`ProjectSession`] for a test fixture or a caller that
//! builds fresh state.
//!
//! The builder carries TWO families of method, and a reader must not read
//! the pair as a duplicate.
//!
//! - The CHAINING methods ([`ProjectSessionBuilder::tool`],
//!   [`ProjectSessionBuilder::model`], [`ProjectSessionBuilder::setup`],
//!   [`ProjectSessionBuilder::toolpath`]) PIN the id the caller supplies.
//!   They take `self` by value and return `Self`. A fixture that names a
//!   tool by [`ToolConfig::id`] or a toolpath by [`ToolpathConfig::tool_id`]
//!   needs them.
//! - The ALLOCATING methods ([`ProjectSessionBuilder::add_tool`],
//!   [`ProjectSessionBuilder::add_model`], [`ProjectSessionBuilder::add_setup`],
//!   [`ProjectSessionBuilder::add_toolpath`]) ALLOCATE the id, exactly as the
//!   matching `ProjectSession` setter does. They take `&mut self` and return
//!   the created value, because a caller reads it.
//!
//! A fixture outside `rs_cam_core` builds a session here, and mutates one
//! through [`ProjectSession::apply`].
//!
//! # Invalidation
//!
//! The builder RE-DERIVES no invalidation. It builds fresh state, so there is
//! nothing to invalidate. Each allocating method delegates to the raw `_impl`
//! half, which carries the field write alone; the [`super::Effects`]
//! construction and the `invalidate_result_chain` call live in `with_effects`
//! and `try_with_effects`, which the builder never calls.

use crate::compute::stock_config::StockConfig;
use crate::compute::tool_config::ToolConfig;
use crate::compute::transform::FaceUp;
use crate::machine::MachineProfile;

use super::{
    LoadedModel, ProjectSession, SessionError, SetupData, ToolpathComputeResult, ToolpathConfig,
};

/// Builds a [`ProjectSession`] from parts the caller supplies.
///
/// # Contract of the chaining methods
///
/// [`Self::tool`], [`Self::model`], [`Self::setup`] and [`Self::toolpath`]
/// write every [`ToolConfig`], [`LoadedModel`], [`SetupData`] and
/// [`ToolpathConfig`] verbatim.
///
/// - They keep each supplied id. They never renumber.
/// - They keep the order of each list.
/// - They never call `StockConfig::update_from_bbox`, so a model never moves
///   the stock.
/// - [`Self::result`] stores a pre-inserted result as given and leaves the
///   revision map empty, so [`ProjectSession::toolpath_revision`] reads `0`.
/// - They validate no index and no reference. A toolpath that names an
///   absent tool reaches [`Self::build`] unchanged.
///
/// [`Self::build`] raises each id counter above every supplied id, so the
/// first later add does not collide with a supplied id.
///
/// # Contract of the allocating methods
///
/// [`Self::add_tool`], [`Self::add_model`], [`Self::add_setup`] and
/// [`Self::add_toolpath`] behave as the matching `ProjectSession` setter
/// does. Each one allocates the id and overwrites the supplied one, and
/// [`Self::add_model`] fits the stock when
/// [`StockConfig::auto_from_model`] is set. Each one returns the value the
/// setter reports in `Effects::created`, and the QUANTITY differs per
/// method: an index for a tool, a setup and a toolpath, and an ID for a
/// model.
///
/// # Order rules
///
/// [`ProjectSession::new_empty`] seeds one setup, "Setup 1". The first call
/// to [`Self::setup`] replaces that seed. Each later call appends.
/// [`Self::toolpath`] appends the new index to the LAST setup. Interleave the
/// two calls to fill more than one setup.
///
/// [`Self::add_setup`] always appends, and it marks the seed as written. Do
/// not mix [`Self::add_setup`] with [`Self::setup`] on one builder.
///
/// # Example
///
/// ```ignore
/// let session = ProjectSessionBuilder::new()
///     .tool(ToolConfig::new_default(ToolId(7), ToolType::EndMill))
///     .toolpath(config)
///     .build();
///
/// let mut builder = ProjectSessionBuilder::new().stock(stock);
/// let tool_index = builder.add_tool(tool);
/// let model_id = builder.add_model(model);
/// let session = builder.build();
/// ```
pub struct ProjectSessionBuilder {
    session: ProjectSession,
    /// True after [`Self::setup`] replaced the seeded setup, or after
    /// [`Self::add_setup`] appended beside it.
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
    pub fn post(mut self, post: crate::gcode::PostConfig) -> Self {
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

    /// Set the project name.
    pub fn name(mut self, name: String) -> Self {
        self.session.name = name;
        self
    }

    // ── The allocating methods ────────────────────────────────────────

    /// Append a tool the way [`ProjectSession::add_tool`] does. The session
    /// allocates the id, and this returns the tool's INDEX in the tool list.
    ///
    /// That index is NOT the tool's id. Read the id with [`Self::tools`].
    pub fn add_tool(&mut self, tool: ToolConfig) -> usize {
        self.session.add_tool_impl(tool)
    }

    /// Append a model the way [`ProjectSession::add_model`] does. This
    /// returns the model's ID, not its index.
    ///
    /// The call fits the stock to the model's bounding box when
    /// [`StockConfig::auto_from_model`] is set, which
    /// [`Self::model`] does not. `auto_from_model` DEFAULTS to true, so a
    /// fixture on a default stock must use this method and not
    /// [`Self::model`], or the stock silently keeps its default size.
    pub fn add_model(&mut self, model: LoadedModel) -> usize {
        self.session.add_model_impl(model)
    }

    /// Add a setup the way [`ProjectSession::add_setup`] does. This returns
    /// the setup's INDEX in the setup list.
    ///
    /// The call APPENDS. It never replaces the setup
    /// [`ProjectSession::new_empty`] seeds, so it leaves "Setup 1" in place
    /// at index 0. [`Self::setup`] replaces that seed on its first call.
    /// **Do not mix the two methods on one builder.** This method marks the
    /// seed as written, so a later [`Self::setup`] call appends rather than
    /// overwriting index 0.
    pub fn add_setup(&mut self, name: String, face_up: FaceUp) -> usize {
        self.setup_written = true;
        self.session.add_setup_impl(name, face_up)
    }

    /// Append a toolpath to one setup the way
    /// [`ProjectSession::add_toolpath`] does. This returns the toolpath's
    /// INDEX in the toolpath list.
    ///
    /// The call refuses when `setup_index` names no setup.
    pub fn add_toolpath(
        &mut self,
        setup_index: usize,
        config: ToolpathConfig,
    ) -> Result<usize, SessionError> {
        self.session.add_toolpath_impl(setup_index, config)
    }

    // ── The read accessors ────────────────────────────────────────────

    /// Every tool added so far, in order.
    pub fn tools(&self) -> &[ToolConfig] {
        self.session.tools()
    }

    /// Every model added so far, in order.
    pub fn models(&self) -> &[LoadedModel] {
        self.session.models()
    }

    /// Every setup added so far, in order. The session's own accessor is
    /// [`ProjectSession::list_setups`].
    pub fn setups(&self) -> &[SetupData] {
        self.session.list_setups()
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
