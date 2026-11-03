//! The header of the toolpath inspector: the status line (`HeaderRow`,
//! `draw_generate_row`) and its state words. The name row and the Tool row
//! call stay in `draw_toolpath_panel`; the Tool row is
//! `tab_badges::draw_tool_row`.

use crate::ui::components::{Notice, StatusLine};

/// W2b: the state words of the status line, one per core `RunState`.
pub(crate) const WORD_DISABLED: &str = "Disabled";
pub(crate) const WORD_NOT_READY: &str = "Not ready";
pub(crate) const WORD_READY: &str = "Ready";
pub(crate) const WORD_GENERATING: &str = "Generating";
pub(crate) const WORD_DONE: &str = "Done";
pub(crate) const WORD_OUT_OF_DATE: &str = "Out of date";
pub(crate) const WORD_FAILED: &str = "Failed";
pub(crate) const WORD_WAITING: &str = "Waiting";

/// What the status line of the header shows (FR-2 F22, W2b).
pub(in crate::ui::properties) struct HeaderRow<'a> {
    /// THE run state of the toolpath (core `toolpath_run_state`).
    pub run_state: Option<&'a rs_cam_core::session::RunState>,
    /// The GUI can submit a run: the project has a tool and no validator
    /// refusal stands.
    pub can_run: bool,
    /// The op does not regenerate on its own (the hover of "Ready").
    pub manual_generation: bool,
    /// The sentence of a blocked submit of a waiting rest op, when the GUI
    /// holds one (the hover of "Waiting").
    pub waiting_detail: Option<&'a str>,
    /// F20: the hover of the first blocking finding of the stored result
    /// (`placement::blocking_hover`).
    pub blocking: Option<&'a str>,
    /// The move count of the stored result.
    pub moves: Option<usize>,
    pub feed_stage: Option<rs_cam_core::session::FeedStage>,
    /// G-B: the moves of the stored result that carry a feed cap.
    pub caps: Option<crate::state::runtime::CapFigures>,
}

impl HeaderRow<'_> {
    /// True when the state word is the F20 safety signal "Done · BLOCKED".
    fn is_blocked(&self) -> bool {
        matches!(self.run_state, Some(rs_cam_core::session::RunState::Done))
            && self.blocking.is_some()
    }

    /// The move count text: "old: N moves" on a result that is out of date.
    fn moves_text(&self) -> Option<String> {
        let moves = self.moves?;
        Some(
            if matches!(
                self.run_state,
                Some(rs_cam_core::session::RunState::OutOfDate)
            ) {
                format!("old: {moves} moves")
            } else {
                format!("{moves} moves")
            },
        )
    }

    /// The word, its colour and its hover.
    fn word(&self) -> (&'static str, egui::Color32, String) {
        use crate::ui::tokens;
        use rs_cam_core::session::RunState;
        if self.is_blocked()
            && let Some(hover) = self.blocking
        {
            return (
                crate::ui::toolpath_panel::BLOCKED_HEADER,
                tokens::DANGER,
                hover.to_owned(),
            );
        }
        match self.run_state {
            Some(RunState::Disabled) => (
                WORD_DISABLED,
                tokens::TEXT_MUTED,
                "Disabled: this operation does not go to G-code.".to_owned(),
            ),
            Some(RunState::NotReady { .. }) => (
                WORD_NOT_READY,
                tokens::TEXT_MUTED,
                "Not ready: generation refuses until the marked inputs are set.".to_owned(),
            ),
            Some(RunState::Generating) => (
                WORD_GENERATING,
                tokens::TEXT_BODY,
                "Generating: the operation is in generation.".to_owned(),
            ),
            Some(RunState::Done) => (
                WORD_DONE,
                tokens::OK,
                "Done: the result answers the settings shown here.".to_owned(),
            ),
            Some(RunState::OutOfDate) => (
                WORD_OUT_OF_DATE,
                tokens::CAUTION,
                "Out of date: this operation generated, then one of its inputs changed. \
                 The path in the viewport and the figures on this panel are from that \
                 earlier generation."
                    .to_owned(),
            ),
            Some(RunState::Failed { .. }) => (
                WORD_FAILED,
                tokens::DANGER,
                "Failed: the last run refused. The issue row under this line gives the \
                 reason. An edit of the inputs clears it."
                    .to_owned(),
            ),
            Some(RunState::Waiting { .. }) => (
                WORD_WAITING,
                tokens::CAUTION,
                self.waiting_detail.map_or_else(
                    || {
                        "Waiting: this rest operation needs the simulated stock of the \
                         operations before it. Generate runs them and the simulation first."
                            .to_owned()
                    },
                    str::to_owned,
                ),
            ),
            Some(RunState::Ready) | None => (
                WORD_READY,
                tokens::TEXT_BODY,
                if self.manual_generation {
                    "Ready: this operation has no result yet, and it does not regenerate \
                     on its own. Press G, or click Generate."
                        .to_owned()
                } else {
                    "Ready: this operation has no result yet.".to_owned()
                },
            ),
        }
    }

    /// The one button of the state, or `None` for no button. W2b: a state
    /// that asks for a tool or for a fix of the inputs shows a disabled
    /// Generate; the red Tool row and the issue rows are the call to action.
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "projection: only the named variant(s) carry this (named: RunState::Done)"
    )]
    fn button(&self) -> Option<crate::ui::components::Button> {
        use crate::ui::components::Button;
        use rs_cam_core::session::{NextAction, RunState, next_action};
        let state = self.run_state.cloned().unwrap_or(RunState::Ready);
        let button = match next_action(&state) {
            NextAction::ChooseTool | NextAction::FixInputs => {
                return Some(Button::primary("Generate").enabled(false));
            }
            NextAction::Generate => Button::primary("Generate"),
            NextAction::Regenerate => Button::quiet("Regenerate"),
            NextAction::Retry => Button::quiet("Retry"),
            NextAction::None => match state {
                // A current result can run again on request.
                RunState::Done => Button::quiet("Regenerate"),
                _ => return None,
            },
        };
        Some(button.enabled(self.can_run))
    }
}

/// The status line of the header (FR-2 F22, W2b). The row never grows the
/// panel. True when its button was clicked.
///
/// From the right: the one button, the notes counter, then the move count
/// as faint meta (its hover holds the feed stage and the caps). The state
/// word takes the rest and truncates, except the F20 safety signal
/// "Done · BLOCKED": it takes its full width first and the move count drops.
pub(in crate::ui::properties) fn draw_generate_row(
    ui: &mut egui::Ui,
    row: &HeaderRow<'_>,
    notes: &[Notice],
) -> bool {
    let (word, colour, hover) = row.word();
    let figures = figures_hover(row);
    let hover = if figures.is_empty() {
        hover
    } else {
        format!("{hover}\n{figures}")
    };
    StatusLine::new(word, colour, hover)
        .keep_whole(row.is_blocked())
        .meta(row.moves_text().map(|text| (text, figures)))
        .button(row.button())
        .notes(notes)
        .show(ui)
}

/// The figures as one hover line, for a state word that may push them
/// out of the row.
fn figures_hover(row: &HeaderRow<'_>) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(text) = row.moves_text() {
        parts.push(text);
    }
    if let Some(caps) = row.caps {
        parts.push(caps.line());
    }
    if let Some(stage) = row.feed_stage {
        parts.push(stage.statement());
    }
    parts.join("\n")
}
