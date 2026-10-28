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
