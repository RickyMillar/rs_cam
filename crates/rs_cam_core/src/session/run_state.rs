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
//! 6. W2b: a rest op (`StockSource::FromRemainingStock`) with no current
//!    simulated snapshot gives [`RunState::Waiting`]. The GUI submit blocks
//!    it on the same rule (`snapshot_is_current`), so the GUI and MCP give
//!    one word.
//! 7. No result, after a result an edit dropped, gives
//!    [`RunState::OutOfDate`].
//! 8. Else [`RunState::Ready`].
//!
//! W2c: an AUTOMATIC submit (the GUI regenerate sweep after an edit, a
//! plan step that is not the plan's target) skips a toolpath for which
//! [`skips_automatic_generate`] is true. It records no refusal. An explicit
//! generate (the GUI button, MCP `generate_toolpath`) still refuses with
//! its reason.

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
    /// W2b: a rest op that waits on the simulated stock of the operations
    /// before it. `on` names the nearest enabled operation before it in the
    /// carve order ([`ProjectSession::carve_order_before`]); it is empty
    /// when no operation comes before it.
    Waiting { on: Vec<ToolpathId> },
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
            Self::Waiting { .. } => "waiting",
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
                id.as_str() == ids::PRECOND_NO_TOOL
                    || id.as_str() == ids::COMPAT_TOOL_KIND_CANNOT_COMPUTE
            });
            if tool_first {
                NextAction::ChooseTool
            } else {
                NextAction::FixInputs
            }
        }
        // W2b: a Generate runs the plan, which generates and simulates the
        // operations before a waiting rest op first.
        RunState::Ready | RunState::Waiting { .. } => NextAction::Generate,
        RunState::OutOfDate => NextAction::Regenerate,
        RunState::Failed { .. } => NextAction::Retry,
        RunState::Disabled | RunState::Generating | RunState::Done => NextAction::None,
    }
}

/// W2c: true when an AUTOMATIC submit skips the toolpath in `state`: the
/// op is switched off, or a current finding refuses generation. The GUI
/// regenerate sweep and every plan step that is not the plan's target read
/// this rule. An explicit generate does not read it and still refuses.
#[must_use]
pub fn skips_automatic_generate(state: &RunState) -> bool {
    matches!(state, RunState::Disabled | RunState::NotReady { .. })
}

/// True when the diagnostic refuses generation while it is current.
///
/// K6 (ruling Q6, Q10), feeds contract P14 (review M7): the row kind alone
/// decides. A current diagnostic whose `diag_ids!` row is
/// [`crate::diagnostics::RowKind::NotReady`] (a missing input, including
/// `compat.tool_kind_cannot_compute`, the prologue's tool refusal) refuses;
/// a finding never does, whatever its severity (a cut into the model is
/// `Severe` and refuses nothing, A5). An id with no row keeps the old rule
/// (a `Severe` one refuses): `DiagnosticId` also builds from a `String`, so
/// no test holds every produced id to a row.
#[must_use]
pub fn refuses_generation(d: &Diagnostic) -> bool {
    use crate::diagnostics::RowKind;
    if d.state != DiagnosticState::Current {
        return false;
    }
    match ids::row_kind(d.id.as_str()) {
        Some(RowKind::NotReady) => true,
        Some(RowKind::Finding(_)) => false,
        None => d.severity == Severity::Severe,
    }
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
    // W2b: the gate list (`diagnose_toolpath_gate`): the sources that can
    // refuse generation, without the load report, the feeds calculator and
    // Suggest, because the GUI header reads this state on every repaint.
    // No load gate, feeds hint or Suggest record refuses generation, so
    // the refusing subset is the same as the full list's.
    // A list that cannot be built (a tool id that names no tool) is not
    // ready either: generation refuses the same input. The state names no
    // finding then.
    let Ok(findings) = session.diagnose_toolpath_gate(index) else {
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
    if tc.stock_source == crate::compute::config::StockSource::FromRemainingStock
        && session.snapshot_is_current(id).is_err()
    {
        return Some(RunState::Waiting {
            on: waiting_on(session, id),
        });
    }
    if session.run_record.generated.contains(&id) {
        return Some(RunState::OutOfDate);
    }
    Some(RunState::Ready)
}

/// W2b: the operation a waiting rest op `id` waits on: the nearest enabled
/// toolpath before it in the carve order, which crosses setups. The GUI
/// submit names the same one (`prior_stock_blocker`). Empty when no enabled
/// toolpath comes before it.
fn waiting_on(session: &ProjectSession, id: ToolpathId) -> Vec<ToolpathId> {
    session
        .carve_order_before(id)
        .into_iter()
        .rev()
        .find_map(|index| {
            session
                .toolpath_configs()
                .get(index)
                .filter(|tc| tc.enabled)
                .map(|tc| tc.id)
        })
        .into_iter()
        .collect()
}
