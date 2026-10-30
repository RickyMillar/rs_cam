//! W1 (toolpath inspector programme, 2026-10-30): THE run state of one
//! toolpath, and the next action it asks for.
//!
//! The GUI header and MCP `list_toolpaths` read [`toolpath_run_state`], so
//! the two surfaces give one word for one state. Before this function the
//! GUI built the word from its own compute record and gave two wrong
//! answers:
//!
//! - F1: "Ready" for an op with no tool and no feeds, which generation
//!   refuses. A current finding that refuses generation now gives
//!   [`RunState::NotReady`].
//! - F2: "Operation failed: no feeds data" after the feeds were set, until a
//!   regenerate. A refusal now belongs to the inputs that produced it: the
//!   session records it with the generation-input revision
//!   ([`ProjectSession::toolpath_revision_at`]), and an edit that moves the
//!   revision clears it.
//!
//! The order of the rule (the first arm that applies):
//!
//! 1. `enabled == false` gives [`RunState::Disabled`].
//! 2. A compute in flight (the caller's flag) gives [`RunState::Generating`].
//! 3. A current finding that refuses generation gives
//!    [`RunState::NotReady`]. A diagnostic list that cannot be built (a
//!    tool id that names no tool) gives `NotReady` with no finding.
//! 4. A refusal recorded at the current revision gives [`RunState::Failed`].
//! 5. A result gives [`RunState::Done`].
//! 6. No result, after a result an edit dropped, gives
//!    [`RunState::OutOfDate`].
//! 7. Else [`RunState::Ready`].

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use super::ProjectSession;
use crate::diagnostics::{Diagnostic, DiagnosticId, DiagnosticState, Severity, ids};
use crate::ids::ToolpathId;

/// Where one toolpath stands. [`toolpath_run_state`] is the one rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum RunState {
    /// The op is switched off; it does not go to G-code.
    Disabled,
    /// A current finding refuses generation. `blocking` names each one, in
    /// the order of the diagnostic list; it is empty when the list cannot
    /// be built.
    NotReady { blocking: Vec<DiagnosticId> },
    /// No finding refuses generation and the op has no result.
    Ready,
    /// A compute is in flight for the op.
    Generating,
    /// The result answers the current inputs.
    Done,
    /// The op had a result, and an edit of its inputs dropped it.
    OutOfDate,
    /// The last run refused, and no input changed since that run.
    Failed { reason: String },
}

/// The one action the state asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NextAction {
    /// Bind a tool: the op has none, or the wrong kind.
    ChooseTool,
    /// Fix the inputs a blocking finding names.
    FixInputs,
    Generate,
    Regenerate,
    Retry,
    None,
}

impl RunState {
    /// The stable word for the MCP wire and for tests.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::NotReady { .. } => "not_ready",
            Self::Ready => "ready",
            Self::Generating => "generating",
            Self::Done => "done",
            Self::OutOfDate => "out_of_date",
            Self::Failed { .. } => "failed",
        }
    }
}

/// The action that `state` asks for. A not-ready op with a tool finding
/// asks for a tool first (Suggest needs the tool to give the feeds); any
/// other blocking finding asks for a fix of the inputs.
#[must_use]
pub fn next_action(state: &RunState) -> NextAction {
    match state {
        RunState::NotReady { blocking } => {
            let tool_first = blocking.iter().any(|id| {
                id.as_str() == ids::PRECOND_NO_TOOL || id.as_str() == ids::COMPAT_TOOL_KIND_REFUSED
            });
            if tool_first {
                NextAction::ChooseTool
            } else {
                NextAction::FixInputs
            }
        }
        RunState::Ready => NextAction::Generate,
        RunState::OutOfDate => NextAction::Regenerate,
        RunState::Failed { .. } => NextAction::Retry,
        RunState::Disabled | RunState::Generating | RunState::Done => NextAction::None,
    }
}

/// True when the finding refuses generation while it is current: a
/// `Blocking` finding, or the tool-kind refusal (`Critical`, but the
/// generator's own prologue refuses on the same `tool_refusal`, W2.2).
#[must_use]
pub fn refuses_generation(d: &Diagnostic) -> bool {
    d.state == DiagnosticState::Current
        && (d.severity == Severity::Blocking || d.id.as_str() == ids::COMPAT_TOOL_KIND_REFUSED)
}

/// The run record of the session: what [`toolpath_run_state`] needs that
/// the result cache cannot say. Not saved: a load has no result and no
/// refusal.
#[derive(Debug, Clone, Default)]
pub(crate) struct RunRecord {
    /// The last refusal of each toolpath, with the generation-input
    /// revision it answered. Current while the toolpath is still at that
    /// revision.
    refusals: HashMap<ToolpathId, (u64, String)>,
    /// The toolpaths that gained a result and lost no run since. An edit
    /// drops the result and keeps the id here (out of date); a refused or
    /// cancelled run removes it.
    generated: HashSet<ToolpathId>,
}

impl ProjectSession {
    /// Record the refusal of the last run of the toolpath at `index`, at
    /// the current revision. `None` clears the record (a cancel).
    pub(crate) fn record_run_refusal(&mut self, index: usize, refusal: Option<String>) {
        let Some(id) = self.toolpath_configs.get(index).map(|tc| tc.id) else {
            return;
        };
        let revision = self.toolpath_revision_at(index);
        self.run_record.generated.remove(&id);
        match refusal {
            Some(reason) => {
                self.run_record.refusals.insert(id, (revision, reason));
            }
            None => {
                self.run_record.refusals.remove(&id);
            }
        }
    }

    /// Record that the toolpath at `index` gained a result.
    pub(crate) fn record_run_result(&mut self, index: usize) {
        let Some(id) = self.toolpath_configs.get(index).map(|tc| tc.id) else {
            return;
        };
        self.run_record.refusals.remove(&id);
        self.run_record.generated.insert(id);
    }

    /// The refusal of the last run of `id`, while no input changed since
    /// that run. `None` after an edit that moved the revision.
    #[must_use]
    pub fn current_run_refusal(&self, id: ToolpathId) -> Option<&str> {
        let (index, _) = self.find_toolpath_config_by_id(id)?;
        let (revision, reason) = self.run_record.refusals.get(&id)?;
        (*revision == self.toolpath_revision_at(index)).then_some(reason.as_str())
    }
}

/// THE run state of the toolpath `id`; the module doc gives the rule.
///
/// `in_flight` is true while a compute lane runs a generation for the
/// toolpath. The session does not own the worker lanes (the GUI and MCP
/// run them off the session), so the caller supplies it; the CLI door
/// [`ProjectSession::generate_toolpath`] holds `&mut self` and never
/// reads this while it runs.
///
/// `None` when no toolpath has the id.
#[must_use]
pub fn toolpath_run_state(
    session: &ProjectSession,
    id: ToolpathId,
    in_flight: bool,
) -> Option<RunState> {
    let (index, tc) = session.find_toolpath_config_by_id(id)?;
    if !tc.enabled {
        return Some(RunState::Disabled);
    }
    if in_flight {
        return Some(RunState::Generating);
    }
    // The session's own trace: no load gate is Blocking, so the trace
    // changes no blocking answer.
    // A list that cannot be built (a tool id that names no tool) is not
    // ready either: generation refuses the same input. The state names no
    // finding then.
    let Ok(findings) = session.diagnose_toolpath(index) else {
        return Some(RunState::NotReady {
            blocking: Vec::new(),
        });
    };
    let blocking: Vec<DiagnosticId> = findings
        .into_iter()
        .filter(refuses_generation)
        .map(|d| d.id)
        .collect();
    if !blocking.is_empty() {
        return Some(RunState::NotReady { blocking });
    }
    if let Some(reason) = session.current_run_refusal(id) {
        return Some(RunState::Failed {
            reason: reason.to_owned(),
        });
    }
    if session.result_at(index).is_some() {
        return Some(RunState::Done);
    }
    if session.run_record.generated.contains(&id) {
        return Some(RunState::OutOfDate);
    }
    Some(RunState::Ready)
}
