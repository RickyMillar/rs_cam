//! One rule for the predecessor of a Rest operation (G-RESTBADGE).
//!
//! The Operations card badge (`ui::toolpath_panel::rest_badge`) and the
//! static validator (`ui::properties::validate_toolpath`) both answer the
//! question "does this Rest op have an operation to follow?". Until
//! 2026-09-10 each held its own copy of the rule. The badge looked for ANY
//! other toolpath in the setup with the previous tool, in any order and in
//! any enabled state, so a Rest card dragged above its roughing pass kept a
//! green `dep` while the validator refused to generate it (R05 §2, defect 2).
//!
//! The rule, in words: a predecessor is an EARLIER toolpath in the SAME
//! setup that is ENABLED, cuts the SAME model, and uses the Rest op's
//! configured PREVIOUS tool. This module is the only place that rule is
//! written down. Both callers build a [`RestCandidate`] list in plan order
//! and call [`rest_predecessors`].

use rs_cam_core::session::{ProjectSession, ToolpathConfig};

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

/// The candidates of one setup, in plan order (`SetupData::toolpath_indices`).
pub(crate) fn setup_candidates(session: &ProjectSession, setup_idx: usize) -> Vec<RestCandidate> {
    session
        .list_setups()
        .get(setup_idx)
        .map(|setup| {
            setup
                .toolpath_indices
                .iter()
                .filter_map(|&idx| session.get_toolpath_config(idx))
                .map(RestCandidate::from_config)
                .collect()
        })
        .unwrap_or_default()
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

/// [`rest_predecessors`] over the setup that owns `rest_id` in `session`.
///
/// `rest_model_id` is passed in rather than read from the session because
/// the inspector validates a DRAFT entry whose model may differ from the
/// stored config; the badge passes the stored model.
pub fn rest_predecessors_in_session(
    session: &ProjectSession,
    rest_id: ToolpathId,
    rest_model_id: ModelId,
    prev_tool_id: ToolId,
) -> Vec<ToolpathId> {
    let Some(setup_idx) = session.setup_of_toolpath_id(rest_id) else {
        return Vec::new();
    };
    rest_predecessors(
        &setup_candidates(session, setup_idx),
        rest_id,
        rest_model_id,
        prev_tool_id,
    )
}
