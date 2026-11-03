use super::job::{ModelId, ObstacleId, SetupId, ToolId};
use super::toolpath::ToolpathId;

/// What is currently selected in the project tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Selection {
    None,
    Stock,
    PostProcessor,
    Machine,
    Model(ModelId),
    Tool(ToolId),
    Setup(SetupId),
    /// K1: an obstacle of a setup (a fixture or a keep-out zone).
    Obstacle(SetupId, ObstacleId),
    Toolpath(ToolpathId),
}

impl Selection {
    /// The wire token of the focus kind: `none`, `stock`, `post`, `machine`,
    /// `model`, `tool`, `setup`, `obstacle` or `toolpath` (F2). A new variant
    /// does not compile until it has a token.
    #[must_use]
    pub fn wire_kind(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Stock => "stock",
            Self::PostProcessor => "post",
            Self::Machine => "machine",
            Self::Model(_) => "model",
            Self::Tool(_) => "tool",
            Self::Setup(_) => "setup",
            Self::Obstacle(..) => "obstacle",
            Self::Toolpath(_) => "toolpath",
        }
    }
}

/// The pick mode that is on, for the focus summary (F3).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PickSummary {
    /// The 0-based index of the toolpath whose selection the pick edits.
    pub toolpath_index: Option<usize>,
    /// The selection the pick edits (`op` or `boundary`).
    pub target: rs_cam_core::session::SelectionTarget,
    /// The kinds whose chip is on.
    pub kinds: Vec<rs_cam_core::selection::ItemKind>,
}

/// The view focus as one value (F3): what an agent cannot see on screen.
/// The `set_ui_view` echo and `get_ui_view` read it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FocusSummary {
    /// [`Selection::wire_kind`].
    pub kind: &'static str,
    /// The id of the focused item, when it has one (an obstacle:
    /// `"<setup id>/<obstacle id>"`).
    pub id: Option<String>,
    /// The 0-based index of the focused toolpath.
    pub toolpath_index: Option<usize>,
    /// The workspace wire token.
    pub workspace: &'static str,
    /// The pick mode that is on, if any.
    pub pick: Option<PickSummary>,
    /// The 0-based index of the toolpath whose selection the viewport
    /// lights (`highlight_toolpath`, L-9).
    pub highlighted_toolpath: Option<usize>,
}

/// The focus summary of `state`.
pub fn focus_summary(state: &super::AppState) -> FocusSummary {
    let focus = state.focus();
    let session = state.session();
    let index_of = |id: ToolpathId| session.toolpath_configs().iter().position(|tc| tc.id == id);
    let id = match focus {
        Selection::None | Selection::Stock | Selection::PostProcessor | Selection::Machine => None,
        Selection::Model(id) => Some(id.to_string()),
        Selection::Tool(id) => Some(id.to_string()),
        Selection::Setup(id) => Some(id.to_string()),
        Selection::Obstacle(setup, obstacle) => Some(format!("{setup}/{obstacle}")),
        Selection::Toolpath(id) => Some(id.to_string()),
    };
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "projection: only the named variant(s) carry this (named: Selection::Toolpath)"
    )]
    let toolpath_index = match focus {
        Selection::Toolpath(id) => index_of(*id),
        _ => None,
    };
    let pick = match state.gui.selection_ui.mode.as_ref() {
        Some(super::pick::ViewportMode::Pick(mode)) => Some(PickSummary {
            toolpath_index: index_of(mode.toolpath_id),
            target: mode.target,
            kinds: mode.filter.kinds.iter().copied().collect(),
        }),
        Some(super::pick::ViewportMode::Tabs(_)) | None => None,
    };
    FocusSummary {
        kind: focus.wire_kind(),
        id,
        toolpath_index,
        workspace: state.workspace.wire_key(),
        pick,
        highlighted_toolpath: super::selection_highlight::highlight_toolpath(state)
            .and_then(|tc| index_of(tc.id)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every focus kind has its own wire token.
    #[test]
    fn every_focus_kind_has_a_wire_token_f2() {
        let setup = SetupId::from_raw(1);
        let cases = [
            (Selection::None, "none"),
            (Selection::Stock, "stock"),
            (Selection::PostProcessor, "post"),
            (Selection::Machine, "machine"),
            (Selection::Model(ModelId::from_raw(1)), "model"),
            (Selection::Tool(ToolId::from_raw(1)), "tool"),
            (Selection::Setup(setup), "setup"),
            (
                Selection::Obstacle(setup, ObstacleId::from_raw(1)),
                "obstacle",
            ),
            (Selection::Toolpath(ToolpathId::from_raw(1)), "toolpath"),
        ];
        for (focus, token) in cases {
            assert_eq!(focus.wire_kind(), token, "{focus:?}");
        }
    }
}
