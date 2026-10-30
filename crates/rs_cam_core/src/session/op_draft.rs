//! S11 parity: THE door that drafts a new toolpath (operator ruling
//! 2026-10-23).
//!
//! The GUI, MCP `add_toolpath` and the CLI job build a new op here, so the
//! same request gives the same numbers on every surface. Before S11 the
//! CLI job took the raw registry op, no stock defaults, no Suggest and
//! `DressupConfig::default()`.
//!
//! The draft is:
//!
//! 1. The model (Q7, [`ModelChoice`], [`ProjectSession::model_for_new_op`]):
//!    a stated model, or the rule: the models of
//!    the setup, the selected model when it serves the op, else the first
//!    setup model that is native to the op (a drawing for a 2D op, a mesh
//!    for a 3D op), else the first that serves it.
//! 2. The tool (R2, [`ToolChoice`]): a stated tool, or the first project
//!    tool that the op accepts ([`ProjectSession::tool_fit`]). When no tool
//!    fits, the draft has NO tool (`tool_id: None`). The op exists, shows
//!    [`ProjectSession::no_tool_state`] and refuses generation until the
//!    operator sets a tool ([`CHOOSE_A_TOOL_FIX`]).
//! 3. The registry op (`OperationConfig::new_default`) with the stock
//!    defaults (`feeds::suggest::apply_stock_defaults`), and Suggest under
//!    `ApplyScope::Both` (`feeds::suggest::suggest_params`). With no feeds
//!    data (`FeedsError::Unbacked`, ruling R1) or no tool, the op keeps no
//!    feeds: they are not set, and generation refuses until the operator
//!    sets them (S11).
//! 4. `DressupConfig::for_op`, `HeightsConfig::default`, and the boundary
//!    of [`draft_boundary`].
//!
//! The draft is a value: the caller sends it as `Command::AddToolpath`,
//! which runs the add-time floor rules (`part_outline_floor_due`,
//! `each_face_floor_due`, `each_hole_floor_due`). R-A: a Profile on a mesh
//! model drafts with side Outside and no tabs (the registry default); its
//! default selection takes the part outline only, and the add sets the
//! floor "Model bottom", depth 0.

use crate::compute::catalog::OperationConfig;
use crate::compute::catalog::{GeometryRequirement, OperationFamily, OperationType};
use crate::compute::config::{
    BoundaryConfig, DressupConfig, HeightsConfig, RestAnalysisConfig, StockSource,
};
use crate::feeds::{FeedsError, FeedsProvenance};
use crate::geometry::enriched_mesh::FaceGroupId;
use crate::ids::{ModelId, ToolId};

use super::{LoadedModel, ProjectSession, ToolConfig, ToolpathConfig};

/// R2: the fix that the no-tool refusal names. The Tool dropdown is in the
/// header of the toolpath inspector (W2b; MCP: `set_toolpath_tool`).
pub const CHOOSE_A_TOOL_FIX: &str = "Choose a tool in the header";

/// R2: the tool name that a list row shows for an op with no tool.
pub const NO_TOOL_NAME: &str = "No tool";

/// R2: how the draft chooses the tool of a new op.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolChoice {
    /// This tool. The draft refuses it when the op does not accept it.
    Exact(ToolId),
    /// The first project tool, in tool-list order, that the op accepts
    /// ([`ProjectSession::tool_fit`]). With none, the op has no tool.
    FirstFitting,
}

/// Q7: how the draft chooses the model of a new op.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelChoice {
    /// This model (P2.4: EXACT, no fallback). MCP `add_toolpath` with a
    /// `model_id` and a CLI job that names a model.
    Exact(ModelId),
    /// THE rule of [`ProjectSession::model_for_new_op`], with the model
    /// that the caller has selected (the Setup rail, the viewport, or MCP
    /// `add_toolpath_via_gui` `model_id`), if any.
    Rule {
        /// The selected model, or `None`.
        selected: Option<ModelId>,
    },
}

/// The request of a new toolpath draft.
#[derive(Debug, Clone)]
pub struct DraftRequest {
    /// The kind of op.
    pub op_type: OperationType,
    /// How the draft chooses the tool.
    pub tool: ToolChoice,
    /// How the draft chooses the model.
    pub model: ModelChoice,
    /// The setup that the op joins. Its "Models" list limits the model.
    pub setup_index: usize,
    /// The name of the op.
    pub name: String,
    /// Q7 (O-PICK): the items that the caller has picked in the viewport
    /// when it adds (a BREP face of a model). They become the selection of
    /// the op: each is one `SelectionEdit::AddPick`, so the picks replace
    /// the default. Under [`ModelChoice::Rule`] with no selected model, the
    /// model of the first pick is the selected model.
    pub picked_faces: Vec<(ModelId, FaceGroupId)>,
}

impl DraftRequest {
    /// A request for `op_type` in the setup `setup_index`, named `name`,
    /// with the first fitting tool and the model of the rule (no selected
    /// model).
    #[must_use]
    pub fn new(op_type: OperationType, setup_index: usize, name: impl Into<String>) -> Self {
        Self {
            op_type,
            tool: ToolChoice::FirstFitting,
            model: ModelChoice::Rule { selected: None },
            setup_index,
            name: name.into(),
            picked_faces: Vec::new(),
        }
    }

    /// The same request with the faces that the caller has picked.
    #[must_use]
    pub fn picks(mut self, picked_faces: Vec<(ModelId, FaceGroupId)>) -> Self {
        self.picked_faces = picked_faces;
        self
    }

    /// The same request with the stated tool ([`ToolChoice::Exact`]).
    #[must_use]
    pub fn tool(mut self, tool_id: ToolId) -> Self {
        self.tool = ToolChoice::Exact(tool_id);
        self
    }

    /// The same request with the stated model ([`ModelChoice::Exact`]).
    #[must_use]
    pub fn model(mut self, model_id: ModelId) -> Self {
        self.model = ModelChoice::Exact(model_id);
        self
    }

    /// The same request with the selected model of the caller
    /// ([`ModelChoice::Rule`]).
    #[must_use]
    pub fn selected_model(mut self, selected: Option<ModelId>) -> Self {
        self.model = ModelChoice::Rule { selected };
        self
    }
}

/// A drafted toolpath: the config to add, and the notes of the draft.
#[derive(Debug, Clone)]
pub struct ToolpathDraft {
    /// The config to send as `Command::AddToolpath`.
    pub config: ToolpathConfig,
    /// The `FeedsError::Unbacked` text when Suggest had no data for the
    /// tool, else `None`. The GUI shows it as a toast and MCP returns it as
    /// `feeds_refusal`.
    pub feeds_refusal: Option<String>,
    /// R2: the no-tool state ("No tool fits: needs a flat end mill") when
    /// the draft has no tool, else `None`.
    pub no_tool: Option<String>,
    /// Q7: the picked faces that did not become picks (a face with no Face
    /// item, or an op that holds no selection), with the reason.
    pub dropped_picks: Vec<String>,
}

/// Why a toolpath draft refuses.
#[derive(Debug, Clone)]
pub enum DraftRefusal {
    /// The tool id names no tool of the project.
    NoSuchTool(ToolId),
    /// The setup index names no setup.
    NoSuchSetup(usize),
    /// Suggest refuses the pairing of the tool and the op (the registry
    /// tool rule, or a stepover that the tool cannot give).
    Feeds(FeedsError),
}

impl std::fmt::Display for DraftRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSuchTool(id) => write!(f, "no tool has id {}", id.raw()),
            Self::NoSuchSetup(i) => write!(f, "no setup has index {i}"),
            Self::Feeds(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for DraftRefusal {}

/// The Suggest answer of one tool: the op, the feeds provenance and the
/// `Unbacked` text.
type Suggested = (OperationConfig, FeedsProvenance, Option<String>);

/// THE boundary of a new op (roadmap B.7, R2): a 3D op on a project with a
/// mesh model takes the model silhouette grown by one tool diameter
/// (`BoundaryConfig::for_3d_op`). With no tool there is no diameter, so the
/// op takes `BoundaryConfig::default()`; the first `SetToolpathTool` on it
/// applies this rule again (`set_toolpath_tool`).
pub(crate) fn draft_boundary(
    op_type: OperationType,
    has_mesh: bool,
    tool_diameter: Option<f64>,
) -> BoundaryConfig {
    match tool_diameter {
        Some(d) if op_type.spec().family == OperationFamily::ThreeD && has_mesh => {
            BoundaryConfig::for_3d_op(d)
        }
        _ => BoundaryConfig::default(),
    }
}

/// Q9: true when a Bore tool of `diameter` cuts every hole of `holes` with
/// one helix: each hole is `HoleFit::Larger` for the tool (THE hole fit,
/// `selection::hole_fit`, strict tolerance as the Bore reads it). So the
/// tool is smaller than the smallest hole, and at least half the largest.
/// True for no holes.
pub(crate) fn bore_tool_fits(diameter: f64, holes: &[f64]) -> bool {
    let size = crate::selection::hole_fit::DrillSize::strict(diameter);
    holes
        .iter()
        .all(|&d| crate::selection::hole_fit(Some(d), &size) == crate::selection::HoleFit::Larger)
}

/// Q9: the size limit of a Bore tool for `holes`, as words: "smaller than
/// Ø10.00 and at least Ø6.00" (one helix of a tool of D cuts a hole up to
/// 2 D). `None` for no holes.
pub(crate) fn bore_size_text(holes: &[f64]) -> Option<String> {
    let smallest = holes.iter().copied().min_by(f64::total_cmp)?;
    let largest = holes.iter().copied().max_by(f64::total_cmp)?;
    Some(format!(
        "smaller than Ø{smallest:.2} and at least Ø{:.2}",
        largest / 2.0
    ))
}

impl ProjectSession {
    /// R2: THE first-tool boundary rule. When the op had no tool
    /// (`old_tool` is `None`), `candidate` now has one, and its boundary is
    /// still the inherited default, the boundary becomes
    /// [`draft_boundary`] with the diameter of the new tool (the draft had
    /// no diameter). Both tool doors call it: `SetToolpathTool` (MCP, CLI)
    /// and `ReplaceToolpathConfig` (the GUI Tool combo).
    pub(crate) fn apply_first_tool_boundary(
        &self,
        old_tool: Option<ToolId>,
        candidate: &mut ToolpathConfig,
    ) {
        if old_tool.is_some()
            || !candidate.boundary_inherit
            || candidate.boundary != BoundaryConfig::default()
        {
            return;
        }
        let Some(diameter) = self.toolpath_tool(candidate).map(|t| t.diameter) else {
            return;
        };
        let has_mesh = self.models.iter().any(|m| m.mesh.is_some());
        candidate.boundary =
            draft_boundary(candidate.operation.op_type(), has_mesh, Some(diameter));
    }
}

impl LoadedModel {
    /// Q7: true when the op can read this model: a stock-based op reads any
    /// model; an `Items` op needs a selectable item; a `Mesh` op a mesh;
    /// an `ItemsAndMesh` op both. The GUI add menu reads the same rule
    /// over every model.
    pub fn serves(&self, op_type: OperationType) -> bool {
        let has_mesh = self.mesh.is_some();
        let has_items = self.items.has_selectable_item();
        op_type.is_stock_based()
            || match op_type.geometry_requirement() {
                GeometryRequirement::Items => has_items,
                GeometryRequirement::Mesh => has_mesh,
                GeometryRequirement::ItemsAndMesh => has_items && has_mesh,
            }
    }

    /// Q7: true when the model is the native input of the op: a drawing
    /// (items, no mesh) for an `Items` op, a mesh for a `Mesh` or an
    /// `ItemsAndMesh` op. A STEP part serves a 2D op, but a drawing of the
    /// same setup comes first.
    fn is_native_for(&self, op_type: OperationType) -> bool {
        self.serves(op_type)
            && match op_type.geometry_requirement() {
                GeometryRequirement::Items => self.mesh.is_none(),
                GeometryRequirement::Mesh | GeometryRequirement::ItemsAndMesh => true,
            }
    }
}

impl ProjectSession {
    /// THE draft of a new toolpath (see the module documentation).
    ///
    /// # Errors
    ///
    /// [`DraftRefusal::NoSuchSetup`] for an unknown setup index (under
    /// [`ModelChoice::Rule`]), [`DraftRefusal::NoSuchTool`] for an unknown stated tool, and
    /// [`DraftRefusal::Feeds`] when Suggest refuses a STATED tool for a
    /// reason other than missing feeds data. With
    /// [`ToolChoice::FirstFitting`] the draft refuses no tool: with none
    /// that fits, the op has no tool.
    pub fn draft_toolpath(&self, request: DraftRequest) -> Result<ToolpathDraft, DraftRefusal> {
        let DraftRequest {
            op_type,
            tool,
            model,
            setup_index,
            name,
            picked_faces,
        } = request;
        let model_id = match model {
            ModelChoice::Exact(id) => id,
            ModelChoice::Rule { selected } => self.model_for_new_op(
                op_type,
                setup_index,
                selected.or_else(|| picked_faces.first().map(|(m, _)| *m)),
            )?,
        };
        let chosen: Option<(&ToolConfig, Suggested)> = match tool {
            ToolChoice::Exact(id) => {
                let tool = self.tool_by_id(id).ok_or(DraftRefusal::NoSuchTool(id))?;
                let suggested = self
                    .suggest_for(op_type, tool, model_id)
                    .map_err(DraftRefusal::Feeds)?;
                Some((tool, suggested))
            }
            ToolChoice::FirstFitting => self.tools.iter().find_map(|t| {
                self.suggest_for(op_type, t, model_id)
                    .ok()
                    .map(|suggested| (t, suggested))
            }),
        };
        let has_mesh = self.models.iter().any(|m| m.mesh.is_some());
        let (tool_id, (operation, feeds_provenance, feeds_refusal)) = match chosen {
            Some((tool, suggested)) => (Some(tool.id), suggested),
            None => (
                None,
                (
                    crate::feeds::suggest::default_operation(op_type, &self.stock_context()),
                    FeedsProvenance::default(),
                    None,
                ),
            ),
        };
        let boundary = draft_boundary(
            op_type,
            has_mesh,
            tool_id
                .and_then(|id| self.tool_by_id(id))
                .map(|t| t.diameter),
        );
        let mut config = ToolpathConfig {
            shoe: Default::default(),
            id: crate::ToolpathId::placeholder(),
            name,
            enabled: true,
            operation,
            dressups: DressupConfig::for_op(op_type),
            heights: HeightsConfig::default(),
            tool_id,
            model_id,
            pre_gcode: None,
            post_gcode: None,
            boundary,
            boundary_inherit: true,
            rest_analysis: RestAnalysisConfig::default(),
            stock_source: StockSource::Fresh,
            coolant: crate::gcode::CoolantMode::Off,
            debug_options: crate::trace::debug_trace::ToolpathDebugOptions::default(),
            feeds_provenance,
            planner_origin: None,
        };
        let dropped_picks = self.apply_picks(&mut config, &picked_faces);
        // R2: the state of the op as it will be added (Q9: a Bore names the
        // size limit of its holes).
        let no_tool = self.no_tool_state_of(&config, None);
        Ok(ToolpathDraft {
            config,
            feeds_refusal,
            no_tool,
            dropped_picks,
        })
    }

    /// Q7 (O-PICK): each picked face becomes one `AddPick` on the op
    /// selection of `config` (the first pick replaces the default). The
    /// text of each pick that does not apply.
    fn apply_picks(
        &self,
        config: &mut ToolpathConfig,
        picked_faces: &[(ModelId, FaceGroupId)],
    ) -> Vec<String> {
        let mut dropped = Vec::new();
        let Some(op_default) = config.operation.default_selection() else {
            if !picked_faces.is_empty() {
                dropped.push(format!(
                    "{} holds no selection; the picked faces are not used",
                    config.operation.label()
                ));
            }
            return dropped;
        };
        for &(model_id, face) in picked_faces {
            let edit = self
                .face_pick_key(config, model_id, face)
                .map(crate::selection::SelectionEdit::AddPick);
            let applied = edit.and_then(|edit| match config.operation.selection_slot() {
                Some(slot) => slot.apply(edit, &op_default),
                None => Ok(()),
            });
            if let Err(refusal) = applied {
                dropped.push(refusal.to_string());
            }
        }
        dropped
    }

    /// Q7: THE model of a new op in the setup `setup_index`. The setup's
    /// "Models" list (empty = every model) gives the candidates, in project
    /// order. The rule:
    ///
    /// 1. `selected`, when it is a candidate and serves the op;
    /// 2. else the first candidate that is native to the op (a drawing for
    ///    a 2D op, a mesh for a 3D op);
    /// 3. else the first candidate that serves the op;
    /// 4. else the first candidate (the op refuses at generation and names
    ///    the missing geometry), or the placeholder id when the project has
    ///    no model.
    ///
    /// # Errors
    ///
    /// [`DraftRefusal::NoSuchSetup`] for an unknown setup index.
    pub fn model_for_new_op(
        &self,
        op_type: OperationType,
        setup_index: usize,
        selected: Option<ModelId>,
    ) -> Result<ModelId, DraftRefusal> {
        let setup = self
            .setups
            .get(setup_index)
            .ok_or(DraftRefusal::NoSuchSetup(setup_index))?;
        let candidates: Vec<&LoadedModel> = self
            .models
            .iter()
            .filter(|m| setup.model_ids.is_empty() || setup.model_ids.contains(&m.id))
            .collect();
        let picked = selected
            .and_then(|id| candidates.iter().find(|m| m.id == id))
            .filter(|m| m.serves(op_type))
            .or_else(|| candidates.iter().find(|m| m.is_native_for(op_type)))
            .or_else(|| candidates.iter().find(|m| m.serves(op_type)))
            .or_else(|| candidates.first());
        Ok(picked.map_or(ModelId::placeholder(), |m| m.id))
    }

    /// R2: THE acceptance door of a tool for an op of `op_type` on the model
    /// `model_id`. The add, "Add a Bore for these" and the GUI Tool combo
    /// read it. `Ok` when Suggest accepts the pairing, or has no feeds data
    /// for it (`Unbacked`, ruling R1).
    ///
    /// # Errors
    ///
    /// [`DraftRefusal::NoSuchTool`] for an unknown tool, and
    /// [`DraftRefusal::Feeds`] when Suggest refuses the pairing (the
    /// registry tool rule, or a stepover that the tool cannot give).
    pub fn tool_fit(
        &self,
        op_type: OperationType,
        tool_id: ToolId,
        model_id: ModelId,
    ) -> Result<(), DraftRefusal> {
        let tool = self
            .tool_by_id(tool_id)
            .ok_or(DraftRefusal::NoSuchTool(tool_id))?;
        self.suggest_for(op_type, tool, model_id)
            .map(|_| ())
            .map_err(DraftRefusal::Feeds)
    }

    /// R2: the no-tool state of the toolpath at `index`, or `None` when it
    /// has a tool (or the index names no toolpath). "No tool fits: needs a
    /// flat end mill" when no project tool fits the op, else "No tool set".
    /// Q9: a Bore with holes adds the size limit ("smaller than Ø10.00 and
    /// at least Ø6.00").
    /// The GUI row, the MCP reply, the diagnostics and the refusal read it.
    pub fn no_tool_state(&self, index: usize) -> Option<String> {
        let tc = self.toolpath_configs.get(index)?;
        self.no_tool_state_of(tc, Some(index))
    }

    /// [`Self::no_tool_state`] of the config `tc`. `index` is its place in
    /// the project, or `None` for a draft (no Drill before it).
    pub fn no_tool_state_of(&self, tc: &ToolpathConfig, index: Option<usize>) -> Option<String> {
        if tc.tool_id.is_some() {
            return None;
        }
        let op_type = tc.operation.op_type();
        let holes = if op_type == OperationType::Bore {
            self.bore_holes(tc, index)
        } else {
            Vec::new()
        };
        if let Some(limit) = bore_size_text(&holes) {
            let fits = self.tools.iter().any(|t| {
                bore_tool_fits(t.diameter, &holes)
                    && self.tool_fit(op_type, t.id, tc.model_id).is_ok()
            });
            return Some(if fits {
                "No tool set".to_owned()
            } else {
                format!("No tool fits: {} {limit}", op_type.needs_text())
            });
        }
        Some(self.no_tool_state_for(op_type, tc.model_id))
    }

    /// Q9: the diameters of the held holes that the Drill `drill` skips as
    /// larger than its tool (`HoleFit::Larger`, the holes of "Add a Bore
    /// for these"). Empty when the Drill skips no such hole.
    pub(crate) fn drill_bore_holes(&self, drill: &ToolpathConfig) -> Vec<f64> {
        let Ok(report) = self.selection_status(drill, super::SelectionTarget::Op) else {
            return Vec::new();
        };
        report
            .hole_items
            .iter()
            .filter(|item| {
                report
                    .hole_fits
                    .iter()
                    .any(|(key, fit)| *key == item.key && *fit == crate::selection::HoleFit::Larger)
            })
            .filter_map(|item| item.diameter)
            .collect()
    }

    /// Q9: the holes that the tool of the Bore `bore` must cut. The Bore
    /// and its Drill hold one selection (P6.7), so they are
    /// [`Self::drill_bore_holes`] of the Drill of the same model and
    /// selection (the nearest one before the Bore at `index`). With no such
    /// Drill (or no `index`) they are the held circles of the Bore.
    pub(crate) fn bore_holes(&self, bore: &ToolpathConfig, index: Option<usize>) -> Vec<f64> {
        let selection = bore.operation.selection();
        let before = index.map_or(0, |i| i.min(self.toolpath_configs.len()));
        let drill = self.toolpath_configs.iter().take(before).rev().find(|tc| {
            matches!(tc.operation, OperationConfig::Drill(_))
                && tc.model_id == bore.model_id
                && tc.operation.selection() == selection
        });
        if let Some(holes) = drill
            .map(|d| self.drill_bore_holes(d))
            .filter(|h| !h.is_empty())
        {
            return holes;
        }
        self.selection_status(bore, super::SelectionTarget::Op)
            .map(|report| {
                report
                    .hole_items
                    .iter()
                    .filter(|item| item.selected)
                    .filter_map(|item| item.diameter)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Q9: THE tool of a Bore that "Add a Bore for these" makes from the
    /// Drill `drill`: the first project tool that the Bore accepts
    /// ([`Self::tool_fit`]) and that fits every hole the Drill leaves
    /// ([`bore_tool_fits`]: smaller than the smallest hole, and large
    /// enough that one helix cuts the largest). `None` when no tool passes:
    /// the Bore then has no tool (R2).
    pub(crate) fn bore_tool_for(&self, drill: &ToolpathConfig) -> Option<ToolId> {
        let holes = self.drill_bore_holes(drill);
        self.tools
            .iter()
            .filter(|t| bore_tool_fits(t.diameter, &holes))
            .find(|t| {
                self.tool_fit(OperationType::Bore, t.id, drill.model_id)
                    .is_ok()
            })
            .map(|t| t.id)
    }

    /// The no-tool state of an op of `op_type` on `model_id` (no size
    /// limit).
    fn no_tool_state_for(&self, op_type: OperationType, model_id: ModelId) -> String {
        if self
            .tools
            .iter()
            .any(|t| self.tool_fit(op_type, t.id, model_id).is_ok())
        {
            "No tool set".to_owned()
        } else {
            format!("No tool fits: {}", op_type.needs_text())
        }
    }

    /// R2: the tool of `tc`, or the refusal: [`super::SessionError::NoTool`]
    /// with no tool, [`super::SessionError::ToolNotFound`] for an id that
    /// names no tool.
    pub(crate) fn tool_of(&self, tc: &ToolpathConfig) -> Result<&ToolConfig, super::SessionError> {
        let id = tc.tool_id.ok_or_else(|| super::SessionError::NoTool {
            toolpath: tc.id,
            state: self
                .toolpath_configs
                .iter()
                .position(|t| t.id == tc.id)
                .and_then(|index| self.no_tool_state(index))
                .unwrap_or_else(|| "No tool set".to_owned()),
        })?;
        self.tool_by_id(id)
            .ok_or(super::SessionError::ToolNotFound(id))
    }

    /// The Suggest context of the stock.
    fn stock_context(&self) -> crate::feeds::suggest::StockContext {
        crate::feeds::suggest::StockContext::from_stock_bbox(self.stock_bbox(), self.stock.padding)
    }

    /// Suggest for a new op of `op_type` with `tool` on `model_id`:
    /// `Unbacked` gives the registry and stock defaults with no feeds.
    fn suggest_for(
        &self,
        op_type: OperationType,
        tool: &ToolConfig,
        model_id: ModelId,
    ) -> Result<Suggested, FeedsError> {
        let stock_ctx = self.stock_context();
        // Q1: the bbox the runtime-sanity stepover back-off reads.
        let model_bbox = self.model_bbox_by_id(model_id);
        match crate::feeds::suggest::suggest_params(crate::feeds::suggest::SuggestParamsInput {
            op_type,
            tool,
            machine: &self.machine,
            material: &self.stock.material,
            lut: crate::feeds::embedded_vendor_lut(),
            stock_ctx: &stock_ctx,
            spindle_strategy: crate::feeds::SpindleStrategy::default(),
            context: crate::feeds::suggest::SuggestContext {
                model_bbox: model_bbox.as_ref(),
                ..crate::feeds::suggest::SuggestContext::default()
            },
        }) {
            Ok(s) => Ok((s.operation, s.provenance, None)),
            // Ruling R1 + S11: no feeds data. The op is added with the
            // registry and stock defaults and no feeds.
            Err(e @ FeedsError::Unbacked { .. }) => Ok((
                crate::feeds::suggest::default_operation(op_type, &stock_ctx),
                FeedsProvenance::default(),
                Some(e.to_string()),
            )),
            Err(e) => Err(e),
        }
    }
}

// ── Step 7.8: the template "Geodesic body + edges" ─────────────────────

/// The name of op 1 of [`ProjectSession::draft_geodesic_pair`].
pub const GEODESIC_BODY_NAME: &str = "Geodesic body";
/// The name of op 2 (the optional edge op).
pub const GEODESIC_EDGES_NAME: &str = "Geodesic edges";

/// Step 7.8 (operator ruling 2026-10-28, midday): what the template
/// "Geodesic body + edges" adds.
#[derive(Debug, Clone, Copy)]
pub struct GeodesicPairRequest {
    /// The setup that both ops join.
    pub setup_index: usize,
    /// The tool of op 1 (the GUI: the first fitting tool).
    pub tool: ToolChoice,
    /// The model of both ops.
    pub model: ModelChoice,
    /// The optional edge op (D-T2: the GUI always adds it; MCP `edges`).
    pub edges: bool,
    /// The tool of the edge op, or `None` for op 1's tool (D-T3). A tool
    /// larger than op 1's refuses at the command (D-T4, D53).
    pub edge_tool: Option<ToolId>,
}

impl GeodesicPairRequest {
    /// The GUI request: the first fitting tool, the model rule with the
    /// selected model, and the edge op with op 1's tool.
    #[must_use]
    pub fn new(setup_index: usize, selected_model: Option<ModelId>) -> Self {
        Self {
            setup_index,
            tool: ToolChoice::FirstFitting,
            model: ModelChoice::Rule {
                selected: selected_model,
            },
            edges: true,
            edge_tool: None,
        }
    }
}

/// The two drafts of the template. Send them as one
/// [`Command::AddGeodesicBodyAndEdges`](crate::session::Command::AddGeodesicBodyAndEdges).
#[derive(Debug, Clone)]
pub struct GeodesicPairDraft {
    /// Op 1, "Geodesic body".
    pub body: ToolpathDraft,
    /// Op 2, "Geodesic edges", or `None`.
    pub edges: Option<ToolpathDraft>,
}

impl ProjectSession {
    /// Step 7.8: draft the pair of the template "Geodesic body + edges"
    /// through THE draft door ([`Self::draft_toolpath`]), so each op takes
    /// the tool, model, Suggest values, dressups and boundary of any new
    /// 3D Rough. The template then sets only these fields:
    ///
    /// - op 1: `clearing_strategy = GeodesicAdaptive` (the ruling), and its
    ///   rest analysis on when the edge op exists (D44: the runner keeps
    ///   the regions only on demand);
    /// - op 2: `clearing_strategy = ContourParallel` (the ruling),
    ///   `contour_each_level = true` (C4; G-RESTDOC, D61/D68),
    ///   `stock_source = FromRemainingStock`, and the boundary
    ///   `DerivedRestRegions` with `Center` containment and offset 0 (the
    ///   D47 pair of the D53 sentry). The source id is op 1's, which the
    ///   command writes when it adds op 1; the draft holds a placeholder.
    ///   Its tool is op 1's tool unless the request states one: an equal
    ///   tool is the largest that D53 allows, and op 1's regions are the
    ///   centre positions of that tool (D48), so `Center` is exact.
    ///
    /// # Errors
    ///
    /// The refusals of [`Self::draft_toolpath`].
    pub fn draft_geodesic_pair(
        &self,
        request: GeodesicPairRequest,
    ) -> Result<GeodesicPairDraft, DraftRefusal> {
        use crate::compute::config::{BoundaryConfig, BoundaryContainment, BoundarySource};
        use crate::compute::operation_configs::ClearingStrategy;
        let GeodesicPairRequest {
            setup_index,
            tool,
            model,
            edges,
            edge_tool,
        } = request;
        let body_request = DraftRequest {
            op_type: OperationType::Adaptive3d,
            tool,
            model,
            setup_index,
            name: GEODESIC_BODY_NAME.to_owned(),
            picked_faces: Vec::new(),
        };
        let mut body = self.draft_toolpath(body_request)?;
        if let OperationConfig::Adaptive3d(cfg) = &mut body.config.operation {
            cfg.clearing_strategy = ClearingStrategy::GeodesicAdaptive;
        }
        body.config.rest_analysis.enabled = edges;
        if !edges {
            return Ok(GeodesicPairDraft { body, edges: None });
        }
        let edge_tool = match (edge_tool, body.config.tool_id) {
            (Some(id), _) | (None, Some(id)) => ToolChoice::Exact(id),
            // R2: op 1 has no tool, and the edge op has none either.
            (None, None) => ToolChoice::FirstFitting,
        };
        let edge_request = DraftRequest {
            op_type: OperationType::Adaptive3d,
            tool: edge_tool,
            model: ModelChoice::Exact(body.config.model_id),
            setup_index,
            name: GEODESIC_EDGES_NAME.to_owned(),
            picked_faces: Vec::new(),
        };
        let mut edge = self.draft_toolpath(edge_request)?;
        if let OperationConfig::Adaptive3d(cfg) = &mut edge.config.operation {
            cfg.clearing_strategy = ClearingStrategy::ContourParallel;
            // C4 (lead ruling at the 7.8 approval; source: G-RESTDOC,
            // D61/D68): this pair is the G-RESTDOC case (D61: a Corne edge
            // op cut 8.96 mm at a Depth/Pass of 6), so the template sets
            // the edge op's own value on. "Off by default" is the field's
            // default for any other op.
            cfg.contour_each_level = true;
        }
        edge.config.stock_source = crate::compute::config::StockSource::FromRemainingStock;
        edge.config.boundary = BoundaryConfig {
            enabled: true,
            source: BoundarySource::DerivedRestRegions {
                source_toolpath_id: crate::ToolpathId::placeholder(),
            },
            containment: BoundaryContainment::Center,
            offset: 0.0,
        };
        Ok(GeodesicPairDraft {
            body,
            edges: Some(edge),
        })
    }

    /// The radius (mm) of the built cutter of `tool_id`: the radius the
    /// containment uses (`ResolvedGenInputs::boundary_centre`). `None` for
    /// no tool or a tool that does not build.
    pub(crate) fn built_tool_radius_mm(&self, tool_id: Option<ToolId>) -> Option<f64> {
        use crate::tool::MillingCutter as _;
        tool_id
            .and_then(|id| self.tool_by_id(id))
            .and_then(|tool| super::session_cutter(tool).ok())
            .map(|def| def.diameter() / 2.0)
    }
}
