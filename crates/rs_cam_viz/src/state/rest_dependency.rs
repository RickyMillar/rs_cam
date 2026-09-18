//! One rule for the predecessor of a Rest operation (G-RESTBADGE).
//!
//! The static validator (`ui::properties::validate_toolpath`) answers the
//! question "does this Rest op have an operation to follow?" for a DRAFT
//! entry whose model may differ from the stored config. The stored answer
//! is the `PrevTool` edge of `rs_cam_core::session::dependencies`, which
//! the Operations card connector reads (W3, 2026-09-19); the card badge
//! that used to read this module is gone.
//!
//! The rule, in words: a predecessor is an EARLIER toolpath in the SAME
//! setup that is ENABLED, cuts the SAME model, and uses the Rest op's
//! configured PREVIOUS tool. The caller builds a [`RestCandidate`] list in
//! plan order and calls [`rest_predecessors`].

use rs_cam_core::session::ToolpathConfig;

use crate::state::job::{ModelId, ToolId};
use crate::state::toolpath::ToolpathId;

/// The four fields of a toolpath the predecessor rule reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestCandidate {
    pub id: ToolpathId,
    pub tool_id: ToolId,
    pub model_id: ModelId,
    pub enabled: bool,
}

impl RestCandidate {
    /// Snapshot the rule's fields from a session toolpath config.
    pub fn from_config(tc: &ToolpathConfig) -> Self {
        Self {
            id: tc.id,
            tool_id: ToolId(tc.tool_id),
            model_id: ModelId(tc.model_id),
            enabled: tc.enabled,
        }
    }
}

/// Every toolpath that qualifies as the predecessor of the Rest op `rest_id`,
/// in plan order.
///
/// `candidates` is one setup's toolpath list in plan order. A candidate
/// qualifies when it sits BEFORE the Rest op in that list, is enabled, cuts
/// `rest_model_id`, and uses `prev_tool_id`. An empty result means the
/// validator refuses and the card reads `no dep`. If `rest_id` is not in the
/// list the result is empty: a Rest op cannot follow an operation in another
/// setup.
pub fn rest_predecessors(
    candidates: &[RestCandidate],
    rest_id: ToolpathId,
    rest_model_id: ModelId,
    prev_tool_id: ToolId,
) -> Vec<ToolpathId> {
    let Some(rest_pos) = candidates.iter().position(|c| c.id == rest_id) else {
        return Vec::new();
    };
    candidates
        .iter()
        .take(rest_pos)
        .filter(|c| c.enabled && c.tool_id == prev_tool_id && c.model_id == rest_model_id)
        .map(|c| c.id)
        .collect()
}
