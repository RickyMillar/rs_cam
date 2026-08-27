//! `generate_all` as a fixpoint over the rest-stock chain — the state and the
//! pure decisions, shared by every surface that starts one.
//!
//! A/M11 landed the ladder behind `#[cfg(feature = "mcp")]`, so the GUI's
//! Generate All stayed a single pass and told the operator to loop by hand.
//! Phase O's emitted tier chain (k rest-driven ops from one planner action)
//! makes that untenable, so the types live here — unconditionally compiled,
//! with the MCP response channels demoted to one variant of
//! [`GenerateAllSink`]. The loop itself is unchanged; only where the finished
//! summary is delivered differs between the two callers.

use rs_cam_core::session::ToolpathConfig;

use crate::state::toolpath::{StockSource, ToolpathId};

/// Which ops one `generate_all` covers, and which of them force the ladder.
pub struct GenerateAllScope {
    /// Every enabled toolpath, in project order — the round-1 submission set.
    pub enabled: Vec<ToolpathId>,
    /// 0-based project indices of the enabled ops whose stock comes from a
    /// simulation. Both the ladder's hard bound and what a refusal names.
    pub rest_op_indices: Vec<usize>,
}

/// Read the ladder's inputs off a project.
#[must_use]
pub fn generate_all_scope(configs: &[ToolpathConfig]) -> GenerateAllScope {
    GenerateAllScope {
        enabled: configs
            .iter()
            .filter(|tc| tc.enabled)
            .map(|tc| tc.id)
            .collect(),
        // Rest-dependent ops bound the ladder: a stock chain cannot be longer
        // than the number of links in it.
        rest_op_indices: configs
            .iter()
            .enumerate()
            .filter(|(_, tc)| tc.enabled && tc.stock_source == StockSource::FromRemainingStock)
            .map(|(idx, _)| idx)
            .collect(),
    }
}

/// The ladder must simulate between rounds and was handed no usable cell size.
///
/// Typed rather than pre-rendered because the two surfaces owe the operator
/// different remedies — an MCP caller passes an argument, a GUI operator
/// unticks a checkbox — while the reason is one thing.
#[derive(Debug)]
pub struct MissingResolution {
    /// 0-based project indices of the enabled rest ops that force a simulation.
    pub rest_op_indices: Vec<usize>,
    /// What the caller supplied, when it supplied something unusable.
    pub supplied: Option<f64>,
}

impl MissingResolution {
    /// The clause naming what was wrong with what arrived.
    #[must_use]
    pub fn supplied_clause(&self) -> String {
        self.supplied.map_or_else(
            || "it was not supplied".to_owned(),
            |r| format!("{r} is not a positive cell size"),
        )
    }
}

/// Decide the ladder for one `generate_all`.
///
/// The resolution is **refused, never defaulted** (A/M10) whenever the project
/// needs one: collision counts and engagement both move with cell size, so a
/// silently chosen one hands back verdicts nobody asked for.
///
/// # Errors
/// [`MissingResolution`] when the ladder is on, the project has enabled
/// rest-machining ops, and no positive cell size was supplied.
pub fn plan_fixpoint(
    fixpoint: bool,
    rest_op_indices: &[usize],
    simulation_resolution_mm: Option<f64>,
) -> Result<FixpointPlan, MissingResolution> {
    match (
        fixpoint,
        rest_op_indices.is_empty(),
        simulation_resolution_mm,
    ) {
        (false, _, _) => Ok(FixpointPlan::single_pass()),
        // Nothing depends on simulated stock, so no simulation will be run and
        // no resolution is needed.
        (true, true, _) => Ok(FixpointPlan::single_pass()),
        (true, false, Some(res)) if res > 0.0 => {
            Ok(FixpointPlan::looping(res, rest_op_indices.len()))
        }
        (true, false, supplied) => Err(MissingResolution {
            rest_op_indices: rest_op_indices.to_vec(),
            supplied,
        }),
    }
}

/// Where a finished `generate_all` reports to.
///
/// The MCP variant is gated because `mcp_bridge` is: everything else in this
/// module compiles either way, which is the point of re-homing it.
pub enum GenerateAllSink {
    /// GUI Generate All — progress on the status line, summary as a toast.
    Gui,
    #[cfg(feature = "mcp")]
    Mcp {
        response_tx: tokio::sync::oneshot::Sender<crate::mcp_bridge::McpResponse>,
        /// Optional channel for streaming per-toolpath progress to the client.
        progress_tx: Option<tokio::sync::mpsc::Sender<crate::mcp_bridge::ProgressUpdate>>,
    },
}

impl GenerateAllSink {
    #[cfg(feature = "mcp")]
    #[must_use]
    pub fn progress_tx(
        &self,
    ) -> Option<&tokio::sync::mpsc::Sender<crate::mcp_bridge::ProgressUpdate>> {
        match self {
            Self::Gui => None,
            Self::Mcp { progress_tx, .. } => progress_tx.as_ref(),
        }
    }
}

/// State for one in-flight "generate all", whichever surface started it.
pub struct PendingGenerateAll {
    pub remaining: Vec<ToolpathId>,
    pub completed: usize,
    pub failed: usize,
    /// Per-toolpath error messages for genuinely failed generations.
    pub errors: Vec<(usize, String)>,
    /// A/M11 — ops that could not generate *yet* because their upstream
    /// simulated stock does not exist. Deliberately NOT counted in `failed`:
    /// "cannot yet" and "cannot ever" are different states, and only the
    /// former is worth retrying.
    pub blocked: Vec<(ToolpathId, String)>,
    /// A/M11 — the fixpoint loop's own state.
    pub fixpoint: FixpointPlan,
    /// Set when the loop itself failed (e.g. its simulation errored), as
    /// distinct from any individual toolpath failing.
    pub loop_error: Option<String>,
    pub sink: GenerateAllSink,
}

impl PendingGenerateAll {
    /// Arm a run that has not submitted anything yet.
    #[must_use]
    pub fn new(remaining: Vec<ToolpathId>, fixpoint: FixpointPlan, sink: GenerateAllSink) -> Self {
        Self {
            remaining,
            completed: 0,
            failed: 0,
            errors: Vec::new(),
            blocked: Vec::new(),
            fixpoint,
            loop_error: None,
            sink,
        }
    }

    /// Freeze this run into the shape the renderers consume.
    #[must_use]
    pub fn completed_summary(&self) -> GenerateAllSummary {
        GenerateAllSummary {
            generated: self.completed,
            failed: self.failed,
            errors: self.errors.clone(),
            blocked: self
                .blocked
                .iter()
                .map(|(id, msg)| (id.0, msg.clone()))
                .collect(),
            rounds: self.fixpoint.round,
            simulations: self.fixpoint.simulations,
            loop_error: self.loop_error.clone(),
        }
    }
}

/// A/M11 — `generate_all` iterating to a fixpoint over the rest-stock chain.
///
/// The ladder: generate everything, simulate, regenerate whatever was blocked
/// only on missing upstream stock, repeat. Before this, a chain of `k`
/// dependent rest ops needed `k` manual sim->generate rounds and nothing told
/// the operator what `k` was.
///
/// **Termination.** A round only continues when (a) at least one op is
/// blocked *purely* on sequencing and (b) the previous round generated at
/// least one new op. Genuine failures record `Error` and are never retried,
/// so they cannot keep (a) true. An op reaches `Done` at most once per call,
/// so (b) can hold at most `enabled_count` times. On top of that the loop is
/// hard-bounded by [`Self::max_rounds`] = the number of rest-dependent ops
/// plus one, because a stock chain cannot be longer than that.
pub struct FixpointPlan {
    /// `false` = the pre-A/M11 single pass. The caller can always opt out.
    pub enabled: bool,
    /// Simulation cell size for the loop's own simulations, in mm.
    ///
    /// **Caller-specified, never defaulted** (A/M10). A silently chosen
    /// resolution is the resolution-mismatch trap: collision counts and
    /// engagement change with cell size, so a loop that picked its own would
    /// hand back verdicts nobody asked for. `None` is only legal alongside
    /// `enabled: false`; otherwise the call refuses at request time.
    pub resolution_mm: Option<f64>,
    /// 1-based; the first generate pass is round 1.
    pub round: usize,
    pub max_rounds: usize,
    /// Ops that reached `Done` in the current round — condition (b).
    pub completed_this_round: usize,
    /// How many simulations the loop ran.
    pub simulations: usize,
    /// True between submitting the loop's simulation and its completion.
    pub awaiting_simulation: bool,
}

impl FixpointPlan {
    /// A plan that does exactly what `generate_all` did before A/M11.
    #[must_use]
    pub fn single_pass() -> Self {
        Self {
            enabled: false,
            resolution_mm: None,
            round: 1,
            max_rounds: 1,
            completed_this_round: 0,
            simulations: 0,
            awaiting_simulation: false,
        }
    }

    #[must_use]
    pub fn looping(resolution_mm: f64, rest_dependent_ops: usize) -> Self {
        Self {
            enabled: true,
            resolution_mm: Some(resolution_mm),
            round: 1,
            max_rounds: rest_dependent_ops.saturating_add(1),
            completed_this_round: 0,
            simulations: 0,
            awaiting_simulation: false,
        }
    }
}

/// The outcome of one `generate_all` call, ready to render.
pub struct GenerateAllSummary {
    pub generated: usize,
    pub failed: usize,
    /// `(toolpath id, message)` for genuine failures.
    pub errors: Vec<(usize, String)>,
    /// A/M11 — `(toolpath id, message)` for ops still waiting on upstream
    /// simulated stock. Separate from `errors` on purpose: an agent must be
    /// able to tell "cannot yet" from "cannot ever" without parsing prose.
    pub blocked: Vec<(usize, String)>,
    /// How many internal generate rounds it took. 1 = no ladder was needed.
    pub rounds: usize,
    /// How many simulations the loop ran on the caller's behalf.
    pub simulations: usize,
    /// The loop itself failed (not an individual toolpath).
    pub loop_error: Option<String>,
}

/// The one-line account both surfaces give of a finished run.
#[must_use]
pub fn generate_all_headline(summary: &GenerateAllSummary) -> String {
    let mut headline = format!("Generated {} toolpaths", summary.generated);
    if summary.failed > 0 {
        headline.push_str(&format!(", {} failed", summary.failed));
    }
    if !summary.blocked.is_empty() {
        headline.push_str(&format!(
            ", {} still waiting on upstream simulated stock",
            summary.blocked.len()
        ));
    }
    headline.push_str(&format!(
        " (in {} generate round{}, {} simulation{})",
        summary.rounds,
        if summary.rounds == 1 { "" } else { "s" },
        summary.simulations,
        if summary.simulations == 1 { "" } else { "s" },
    ));
    if let Some(err) = &summary.loop_error {
        headline.push_str(&format!(". The fixpoint loop stopped early: {err}"));
    }
    headline
}
