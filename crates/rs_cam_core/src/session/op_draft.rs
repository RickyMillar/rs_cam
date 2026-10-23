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
//! 1. The registry op (`OperationConfig::new_default`) with the stock
//!    defaults (`feeds::suggest::apply_stock_defaults`).
//! 2. Suggest under `ApplyScope::Both` (`feeds::suggest::suggest_params`).
//!    With no feeds data (`FeedsError::Unbacked`, ruling R1) the op keeps
//!    no feeds: they are not set, and generation refuses until the
//!    operator sets them (S11). Any other Suggest refusal refuses the draft:
//!    the tool does not fit the op.
//! 3. `DressupConfig::for_op`, `HeightsConfig::default`, and
//!    `BoundaryConfig::for_3d_op` on a 3D op when a model has a mesh.
//!
//! The draft is a value: the caller sends it as `Command::AddToolpath`,
//! which runs the add-time floor rules (`part_outline_floor_due`,
//! `each_face_floor_due`, `each_hole_floor_due`). R-A: a Profile on a mesh
//! model drafts with side Outside and no tabs (the registry default); its
//! default selection takes the part outline only, and the add sets the
//! floor "Model bottom", depth 0.

use crate::compute::catalog::OperationType;
use crate::compute::config::{
    BoundaryConfig, DressupConfig, HeightsConfig, RestAnalysisConfig, StockSource,
};
use crate::feeds::FeedsError;
use crate::ids::{ModelId, ToolId};

use super::{ProjectSession, ToolpathConfig};

/// A drafted toolpath: the config to add, and the Suggest refusal text
/// when the tool has no feeds data (the op then has no feeds).
#[derive(Debug, Clone)]
pub struct ToolpathDraft {
    /// The config to send as `Command::AddToolpath`.
    pub config: ToolpathConfig,
    /// The `FeedsError::Unbacked` text when Suggest had no data for the
    /// tool, else `None`. The GUI shows it as a toast and MCP returns it as
    /// `feeds_refusal`.
    pub feeds_refusal: Option<String>,
}

/// Why a toolpath draft refuses.
#[derive(Debug, Clone)]
pub enum DraftRefusal {
    /// The tool id names no tool of the project.
    NoSuchTool(ToolId),
    /// Suggest refuses the pairing of the tool and the op (the registry
    /// tool rule, or a stepover that the tool cannot give).
    Feeds(FeedsError),
}

impl std::fmt::Display for DraftRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSuchTool(id) => write!(f, "no tool has id {}", id.raw()),
            Self::Feeds(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for DraftRefusal {}

impl ProjectSession {
    /// THE draft of a new toolpath of `op_type` on the tool `tool_id` and
    /// the model `model_id`, named `name` (see the module documentation).
    ///
    /// # Errors
    ///
    /// [`DraftRefusal::NoSuchTool`] for an unknown tool id, and
    /// [`DraftRefusal::Feeds`] when Suggest refuses the pairing for a
    /// reason other than missing feeds data.
    pub fn draft_toolpath(
        &self,
        op_type: OperationType,
        tool_id: ToolId,
        model_id: ModelId,
        name: String,
    ) -> Result<ToolpathDraft, DraftRefusal> {
        let tool = self
            .tools
            .iter()
            .find(|t| t.id == tool_id)
            .ok_or(DraftRefusal::NoSuchTool(tool_id))?;
        let stock_ctx = crate::feeds::suggest::StockContext::from_stock_bbox(
            self.stock_bbox(),
            self.stock.padding,
        );
        // Q1: the bbox the runtime-sanity stepover back-off reads.
        let model_bbox = self.model_bbox_by_id(model_id);
        let (operation, feeds_provenance, feeds_refusal) =
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
                Ok(s) => (s.operation, s.provenance, None),
                // Ruling R1 + S11: no feeds data. The op is added with the
                // registry and stock defaults and no feeds.
                Err(e @ FeedsError::Unbacked { .. }) => (
                    crate::feeds::suggest::default_operation(op_type, &stock_ctx),
                    crate::feeds::FeedsProvenance::default(),
                    Some(e.to_string()),
                ),
                Err(e) => return Err(DraftRefusal::Feeds(e)),
            };
        // Roadmap B.7, R2: a 3D op on a mesh model takes the model
        // silhouette grown by one tool diameter.
        let has_mesh = self.models.iter().any(|m| m.mesh.is_some());
        let boundary = if operation.is_3d() && has_mesh {
            BoundaryConfig::for_3d_op(tool.diameter)
        } else {
            BoundaryConfig::default()
        };
        let config = ToolpathConfig {
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
        Ok(ToolpathDraft {
            config,
            feeds_refusal,
        })
    }
}
