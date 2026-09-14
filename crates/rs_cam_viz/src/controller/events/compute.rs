use std::sync::Arc;

use rs_cam_core::dexel_stock::TriDexelStock;
use rs_cam_core::session::{AdoptSimulationArgs, Command, ForgetResultArgs};

use crate::compute::{ComputeBackend, ComputeError, ComputeMessage, ComputeRequest};
use crate::state::simulation::{SimulationResults, SimulationRunMeta};
use crate::state::toolpath::{ComputeStatus, StockSource, ToolpathId};

use super::super::AppController;

impl<B: ComputeBackend> AppController<B> {
    /// Fail a toolpath generation at *submit* time — i.e. before the
    /// request ever reaches the compute worker. Every precondition
    /// rejection inside `submit_toolpath_compute` (missing tool, failed
    /// validation, no 3D mesh, no prior simulated stock for rest
    /// machining, a `DerivedRestRegions` boundary whose source is
    /// missing/self-referential/ungenerated/regionless, ...) must funnel
    /// through here.
    ///
    /// Without this, an MCP `generate_toolpath` / `generate_all` caller
    /// hangs forever: `notify_mcp_toolpath_complete` was previously only
    /// invoked from `drain_compute_results`, which only ever runs for
    /// requests that actually made it to the compute worker. A submit-time
    /// early return produced no worker result, so nothing ever drained,
    /// and the MCP oneshot channel was never resolved (confirmed live: an
    /// MCP `generate_toolpath` call sat unresolved for ~9 hours while the
    /// GUI correctly showed the toolpath in `Error` state).
    ///
    /// Sets `rt.status = Error(msg)` (matching what `drain_compute_results`
    /// does for a compute `Err`) and resolves any pending MCP waiter for
    /// this toolpath with the same error. If no MCP request is pending
    /// (GUI-initiated generate), `notify_mcp_toolpath_complete` is a no-op,
    /// matching existing behavior.
    fn fail_toolpath_submit(&mut self, tp_id: ToolpathId, msg: impl Into<String>) {
        let msg = msg.into();
        let rt = self.state.gui.toolpath_rt_or_default(tp_id);
        rt.status = ComputeStatus::Error(msg);
        rt.result = None;
        // Keep the core cache in step with `rt.result` — see
        // `forget_core_result`.
        Self::forget_core_result(&mut self.state.session, tp_id);
        self.toolpath_completion_landed(tp_id);
    }

    /// A/M11 sibling of [`Self::fail_toolpath_submit`] for the one rejection
    /// that is **not** a failure: a rest op whose upstream simulated stock
    /// does not exist yet. It records `AwaitingPriorStock` rather than
    /// `Error`, so `generate_all`'s fixpoint loop can retry it after a
    /// simulation while genuine failures stay failed, and so it never inflates
    /// the error list an operator or agent has to triage.
    fn block_toolpath_submit(
        &mut self,
        tp_id: ToolpathId,
        block: rs_cam_core::compute::AwaitingPriorStock,
    ) {
        let rt = self.state.gui.toolpath_rt_or_default(tp_id);
        rt.status = ComputeStatus::AwaitingPriorStock(block);
        rt.result = None;
        // Keep the core cache in step with `rt.result` — see
        // `forget_core_result`. Load-bearing for the A/M11 ladder: a
        // blocked op that still had a stale core result would read as
        // "generated" to `PhantomPriorStockScan` and never be offered the
        // phantom prior-stock snapshot that unblocks it.
        Self::forget_core_result(&mut self.state.session, tp_id);
        self.toolpath_completion_landed(tp_id);
    }

    /// The operation whose simulated stock `tp_id` is waiting on: the nearest
    /// ENABLED toolpath before it in the same setup.
    ///
    /// A/M11 defect 1 — the old message named no operation, so the user could
    /// not tell a one-round wait from a four-round one. The distinction the
    /// message must carry is whether that upstream op has itself generated: if
    /// it has not, the wait is at least two rounds (generate it, simulate,
    /// then come back); if it has, one simulation is enough.
    fn prior_stock_blocker(
        &self,
        tp_id: ToolpathId,
        toolpath_name: &str,
    ) -> rs_cam_core::compute::AwaitingPriorStock {
        use rs_cam_core::compute::AwaitingPriorStock;

        let Some((tp_idx, _)) = self.state.session.find_toolpath_config_by_id(tp_id) else {
            return AwaitingPriorStock {
                blocking_toolpath_id: None,
                blocking_toolpath_index: None,
                message: format!(
                    "'{toolpath_name}' uses remaining stock (rest machining) but is no \
                     longer in the project."
                ),
            };
        };

        // Setup membership decides the stock chain: only ops in the same
        // setup contribute to the snapshot this op reads.
        let same_setup: Vec<usize> = self
            .state
            .session
            .list_setups()
            .iter()
            .find(|s| s.toolpath_indices.contains(&tp_idx))
            .map_or_else(|| (0..tp_idx).collect(), |s| s.toolpath_indices.clone());

        let upstream = same_setup
            .iter()
            .copied()
            .filter(|idx| *idx < tp_idx)
            .filter_map(|idx| {
                self.state
                    .session
                    .toolpath_configs()
                    .get(idx)
                    .filter(|tc| tc.enabled)
                    .map(|tc| (idx, tc))
            })
            .next_back();

        let Some((blocker_idx, blocker)) = upstream else {
            return AwaitingPriorStock {
                blocking_toolpath_id: None,
                blocking_toolpath_index: None,
                message: format!(
                    "'{toolpath_name}' uses remaining stock (rest machining) but is the \
                     first enabled operation in its setup — there is no prior operation \
                     to leave any stock behind. Set its stock source to fresh stock, or \
                     move it after the operation it is meant to follow."
                ),
            };
        };

        let blocker_name = blocker.name.clone();
        let blocker_id = blocker.id;
        let blocker_generated = self
            .state
            .gui
            .toolpath_rt
            .get(&blocker_id)
            .is_some_and(|rt| matches!(rt.status, ComputeStatus::Done));

        let message = if blocker_generated {
            format!(
                "'{toolpath_name}' is waiting on simulated stock after '{blocker_name}' \
                 (index {blocker_idx}). That operation is generated, so ONE simulation \
                 is enough: run a simulation, then regenerate. (Not falling back to \
                 fresh stock.)"
            )
        } else {
            format!(
                "'{toolpath_name}' is waiting on simulated stock after '{blocker_name}' \
                 (index {blocker_idx}), which has not generated yet. The cycle may need \
                 repeating: generate '{blocker_name}', run a simulation, then regenerate \
                 this operation — and if IT feeds a further rest op, again. \
                 `generate_all` with a simulation resolution does the whole ladder in \
                 one call. (Not falling back to fresh stock.)"
            )
        };

        AwaitingPriorStock {
            blocking_toolpath_id: Some(blocker_id),
            blocking_toolpath_index: Some(blocker_idx),
            message,
        }
    }

    /// Drop the CORE-side cached result for `tp_id`, keeping the two result
    /// caches in step when a generation does not produce one.
    ///
    /// Free-standing (`&mut ProjectSession`, not `&mut self`) so it can be
    /// called while `drain_compute_results` still holds a `&mut` borrow of
    /// `self.state.gui`'s runtime row — the same field-disjoint shape the
    /// `Ok` arm's `AdoptResult` call already relies on.
    ///
    /// Silent when there is nothing cached: "this toolpath has no result"
    /// is the state being established, not a condition to report.
    ///
    /// `Command::ForgetResult` is the `Err`-arm twin of the `AdoptResult`
    /// row the `Ok` arm takes. The function holds no `AppState`, so it
    /// stamps no `stale_since`; the caller owns the runtime row it is
    /// already writing.
    fn forget_core_result(session: &mut rs_cam_core::session::ProjectSession, tp_id: ToolpathId) {
        if let Some((tp_index, _)) = session.find_toolpath_config_by_id(tp_id) {
            let command = Command::ForgetResult(ForgetResultArgs { index: tp_index });
            let _ = session.apply(command);
        }
    }

    /// Submit one toolpath generation to the compute lane.
    ///
    /// WP11b: this used to resolve every generation input itself — the
    /// model geometry, the setup transform, the heights, the boundary
    /// regions, the prior stock, the reference tools, the machine envelope
    /// — and ship 27 fields to the worker. That was a second assembly of
    /// what `ProjectSession::resolve_generation_inputs` already produces
    /// (tracker row N12), and the two answered differently on eight counts.
    ///
    /// The submit now runs core's step (i), `ProjectSession::start`, ON THE
    /// FRAME LOOP, and ships what it hands back. What stays here is what is
    /// genuinely the GUI's: the notifications, the runtime row, and the two
    /// refusals whose OUTCOME differs from a plain error — a missing tool,
    /// and a rest operation still waiting on simulated stock.
    pub(crate) fn submit_toolpath_compute(&mut self, tp_id: ToolpathId) {
        let Some((tp_idx, tc)) = self.state.session.find_toolpath_config_by_id(tp_id) else {
            self.fail_toolpath_submit(tp_id, "Toolpath config not found".to_owned());
            return;
        };

        let toolpath_name = tc.name.clone();
        let stock_source = tc.stock_source;
        let tool_id_raw = tc.tool_id;
        let operation = tc.operation.clone();

        if !self
            .state
            .session
            .tools()
            .iter()
            .any(|t| t.id.0 == tool_id_raw)
        {
            self.push_notification(
                "Cannot generate: no tool assigned to this toolpath".into(),
                super::super::Severity::Warning,
            );
            self.fail_toolpath_submit(tp_id, "No tool assigned to this toolpath".to_owned());
            return;
        }

        // Run validation
        {
            let validation =
                crate::ui::properties::ToolpathValidationContext::from_session(&self.state.session);
            if let Some((_, tc)) = self.state.session.find_toolpath_config_by_id(tp_id) {
                let errs = crate::ui::properties::validate_toolpath_config(tc, &validation);
                if !errs.is_empty() {
                    self.fail_toolpath_submit(tp_id, errs.join("; "));
                    return;
                }
            }
        }

        // The two lateral-setup preconditions (no 3D part to register the
        // face against; keep-outs a vertical work plane cannot express).
        //
        // The resolver checks the same thing, through the same method, so
        // this door only decides WHERE the operator reads the refusal: as a
        // toast at submit time rather than as a lane error. The check and
        // both message strings have exactly one owner,
        // `ProjectSession::check_lateral_setup_support`; the end-to-end
        // sentry is `rs_cam_core/tests/lateral_setup_end_to_end.rs`.
        //
        // Scoped so the session borrow ends before `fail_toolpath_submit`
        // takes `&mut self`.
        let lateral_refusal = {
            let session = &self.state.session;
            let setup = session
                .list_setups()
                .iter()
                .find(|s| s.toolpath_indices.contains(&tp_idx));
            session
                .check_lateral_setup_support(setup, &operation)
                .err()
                .map(|e| e.to_string())
        };
        if let Some(msg) = lateral_refusal {
            self.push_notification(msg.clone(), super::super::Severity::Warning);
            self.fail_toolpath_submit(tp_id, msg);
            return;
        }

        // A/M11: a rest operation with no simulated snapshot is a SEQUENCING
        // state, not a failure — `generate_all`'s fixpoint retries it, and a
        // failure would stay failed. `start` refuses this case too, but with
        // a plain `OperationFailed`, which the ladder would read as a
        // failure. So this one precondition stays here, ahead of `start`,
        // and it is the only rest/boundary check that does.
        //
        // It now asks whether the snapshot EXISTS. It used to clone the
        // whole dexel grid onto the request; the handle carries the session's
        // own `Arc`.
        if stock_source == StockSource::FromRemainingStock
            && self.state.simulation.prior_stock_for(tp_id).is_none()
        {
            let block = self.prior_stock_blocker(tp_id, &toolpath_name);
            let notice = block.message.clone();
            self.block_toolpath_submit(tp_id, block);
            self.push_notification(notice, super::super::Severity::Warning);
            return;
        }

        // Update GUI runtime status
        let rt = self.state.gui.toolpath_rt_or_default(tp_id);
        rt.status = ComputeStatus::Computing;
        // "Stale" means "needs a submit" — this submit satisfies it. Leaving
        // the flag set let `process_auto_regen`'s 500ms sweep resubmit the
        // same id while it was still the lane's active job, which the
        // worker's resubmit-cancels-and-requeues rule turned into a
        // deterministic "generation cancelled" for any slow op right after
        // load_project (Back Rough, 2×, 2026-07-07).
        rt.stale_since = None;
        rt.result = None;
        rt.debug_trace = None;
        rt.semantic_trace = None;
        rt.debug_trace_path = None;

        // §22 ruling 3: a FRESH flag per submit. The lane's own flag is set
        // by `submit_toolpath` when a resubmit supersedes the active job and
        // is cleared only by the worker thread, so a flag borrowed from the
        // lane can already read `true` here — and `start` polls its flag.
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let handle = match self.state.session.start(
            rs_cam_core::session::Job::GenerateToolpath(
                rs_cam_core::session::GenerateToolpathArgs { index: tp_idx },
            ),
            &cancel,
        ) {
            Ok(rs_cam_core::session::JobHandle::GenerateToolpath(handle)) => *handle,
            // `start` answers the handle of the row it was handed, so this
            // arm cannot happen. It is spelled out rather than wildcarded
            // so a new `Job` row stops this submit compiling.
            Ok(other) => {
                let message = format!(
                    "the generate submit received the {} row's handle",
                    other.id().wire_name()
                );
                tracing::error!("{message}");
                self.fail_toolpath_submit(tp_id, message);
                return;
            }
            Err(error) => {
                // Every rest and boundary precondition refuses HERE now,
                // with core's own wording. The viz copies of those checks
                // are deleted (§22 ruling 6).
                let message = error.to_string();
                self.push_notification(
                    format!("'{toolpath_name}': {message}"),
                    super::super::Severity::Error,
                );
                self.fail_toolpath_submit(tp_id, message);
                return;
            }
        };

        let submit_outcome = self.compute.submit_toolpath(ComputeRequest {
            handle,
            viz: crate::compute::VizExtras {
                toolpath_id: tp_id,
                cancel,
            },
        });
        // G-REGEN-RACE: if this submit replaced the lane's active job for
        // the same toolpath, the `Cancelled` that job is about to return is
        // OUR doing and a replacement is already queued behind it. Record
        // that so `drain_compute_results` does not read it as the
        // toolpath's outcome. Recorded here rather than inferred later
        // because only the lane, under its own lock, knows which submit
        // won the race.
        //
        // The handle's revision cannot replace this set: `start` moves no
        // revision, so two Generate clicks with no edit between them produce
        // two handles carrying the SAME revision.
        if matches!(
            submit_outcome,
            crate::compute::ToolpathSubmitOutcome::SupersededActive
        ) {
            self.superseded_toolpaths.insert(tp_id);
        }
    }

    /// Mark every toolpath whose *enabled* boundary is `DerivedRestRegions`
    /// referencing `source_id` as stale, using the same `stale_since`
    /// mechanism `crate::state::stale::stamp_stale` gives every command
    /// route (WP28 deleted the MCP surface's own helper). A
    /// `DerivedRestRegions` boundary's
    /// clip depends entirely on the source toolpath's cached
    /// `rest_regions` — any regeneration of the source (regions changed,
    /// vanished, or newly appeared) or its removal invalidates every
    /// dependent's cached result just as surely as editing the dependent's
    /// own boundary config would, so this sweep is called from both the
    /// generation-completion handler (`drain_compute_results`, below) and
    /// `handle_remove_toolpath` (`controller/events/toolpath.rs`).
    pub(crate) fn mark_derived_rest_dependents_stale(&mut self, source_id: ToolpathId) {
        let dependent_ids: Vec<ToolpathId> = self
            .state
            .session
            .toolpath_configs()
            .iter()
            .filter(|tc| {
                tc.boundary.enabled
                    && matches!(
                        tc.boundary.source,
                        crate::state::toolpath::BoundarySource::DerivedRestRegions {
                            source_toolpath_id,
                        } if source_toolpath_id == source_id
                    )
            })
            .map(|tc| tc.id)
            .collect();
        if dependent_ids.is_empty() {
            return;
        }
        let now = std::time::Instant::now();
        for id in dependent_ids {
            self.state.gui.toolpath_rt_or_default(id).stale_since = Some(now);
        }
    }

    // SAFETY: tp_index from position() within setup.toolpaths, slice always in bounds
    #[allow(clippy::indexing_slicing)]
    pub(crate) fn drain_compute_results(&mut self) {
        for message in self.compute.drain_results() {
            match message {
                ComputeMessage::Toolpath(result) => {
                    let tp_id = result.toolpath_id;
                    // WP11b: read before the match below moves `result`.
                    let reply_revision = result.revision;
                    // G-REGEN-RACE — the supersede is not an outcome.
                    //
                    // The toolpath lane's submit rule is
                    // resubmit-cancels-and-requeues, so any second submit
                    // of a toolpath that is currently the lane's active job
                    // aborts that job and queues the replacement. Both the
                    // GUI's `process_auto_regen` sweep and MCP
                    // `generate_all` submit through this controller, and
                    // after a `load_project` (which marks every toolpath
                    // stale) the sweep is guaranteed to have a job in
                    // flight 500 ms later — so an agent's `generate_all`
                    // superseded its own work and then read the resulting
                    // `Cancelled` as a *failure of the toolpath*, reporting
                    // "`<name>`: generation cancelled" with `generated: 0`
                    // while the replacement it had just queued went on to
                    // succeed unobserved.
                    //
                    // The fix is here, at the one place that turns a lane
                    // message into a toolpath outcome: a superseded job has
                    // no outcome at all. The toolpath is still `Computing`,
                    // no MCP waiter resolves, no `generate_all` bucket
                    // moves, and the replacement's own result does all
                    // three. A cancel with no supersede behind it — the
                    // `cancel_generation` escape hatch, the GUI's cancel
                    // button — is untouched and still terminal.
                    let expected_supersede = self.superseded_toolpaths.remove(&tp_id);
                    if expected_supersede && matches!(result.result, Err(ComputeError::Cancelled)) {
                        continue;
                    }
                    let rt = self.state.gui.toolpath_rt_or_default(tp_id);
                    rt.debug_trace = result.debug_trace.clone();
                    rt.semantic_trace = result.semantic_trace.clone();
                    rt.debug_trace_path = result.debug_trace_path.clone();
                    match result.result {
                        Ok(computed) => {
                            rt.status = ComputeStatus::Done;
                            // Sync the core-side `session.results` cache so the
                            // single point of truth for "this toolpath has a
                            // fresh result" is `ProjectSession`, not the
                            // viz-side `gui.toolpath_rt`. Without this, the
                            // gcode export / load-report paths read stale-empty
                            // `session.results[idx]` after Apply (see
                            // `planning/F1_RCA.md`).
                            // G-LATERESULT (F2.4): the core cache is what
                            // `FreshnessState` reads for `Current`, so writing
                            // a late result into it is what made an edit
                            // silently lose. If an input moved while this job
                            // ran, the result is KEPT — `rt.result` below, so
                            // the viewport still draws it and the operator
                            // does not lose a long 3D generation — but the
                            // core slot stays empty, which derives
                            // `EditedSince` and puts STALE on every surface
                            // F2.2 wired.
                            //
                            // The auto-regen arm was already safe by a
                            // different route: a resubmit supersedes, and
                            // G-REGEN-RACE drops the abandoned result before
                            // it reaches here. Nothing supersedes on a 3D
                            // manual-regen operation, which is why the row
                            // names that arm.
                            //
                            // WP3: the refusal itself lives in core, at
                            // `Command::AdoptResult`. This door hands the
                            // stamp over and reads the answer; it does not
                            // compare revisions of its own any more.
                            if let Some((tp_index, _)) =
                                self.state.session.find_toolpath_config_by_id(tp_id)
                            {
                                // Honour the §6.E dual-representation
                                // invariant: drill ops carry both the
                                // DrillOp payload and the annotated
                                // toolpath. The worker already built the
                                // DrillOp into `computed.drill_op`;
                                // routing it through here so
                                // `session.results[idx].drill_op()`
                                // returns Some for drill TPs matches the
                                // `session.generate_toolpath` production
                                // path. Without this, drill_gates on the
                                // tool-load report never populate (the
                                // gate evaluator reads `result.drill_op()`).
                                let op_data = match &computed.drill_op {
                                    Some(drill_op_arc) => rs_cam_core::drill_op::OpData::DrillOp(
                                        Arc::clone(drill_op_arc),
                                        Arc::clone(&computed.annotated),
                                    ),
                                    None => rs_cam_core::drill_op::OpData::Toolpath(Arc::clone(
                                        &computed.annotated,
                                    )),
                                };
                                let core_result = rs_cam_core::session::ToolpathComputeResult {
                                    op_data,
                                    stats: computed.stats.clone(),
                                    // Debug + semantic traces stay viz-side
                                    // (Arc'd on `rt.debug_trace` /
                                    // `rt.semantic_trace` above). Core readers
                                    // currently only consume `annotated.spans`
                                    // from `session.results`; bloating the
                                    // cache with owned trace copies is wasted
                                    // work.
                                    debug_trace: None,
                                    semantic_trace: None,
                                };
                                // WP11b: the revision rides the REPLY,
                                // stamped from the handle `start`
                                // produced. `None` means a hand-built
                                // reply in a test, and reproduces the
                                // pre-WP3 accept-when-unstamped arm.
                                let revision = match reply_revision {
                                    Some(submitted) => submitted,
                                    None => self.state.session.toolpath_revision(tp_index),
                                };
                                let adopted = self.state.session.apply(
                                    rs_cam_core::session::Command::AdoptResult(
                                        rs_cam_core::session::AdoptResultArgs {
                                            index: tp_index,
                                            revision,
                                            result: Box::new(core_result),
                                        },
                                    ),
                                );
                                // The `Ok` effects are empty: a completion
                                // records an answer, it moves no revision.
                                // On a refusal the core slot stays empty,
                                // which derives `EditedSince`. `rt.result`
                                // below still keeps the geometry, so the
                                // viewport draws it and the operator does
                                // not lose a long 3D generation. That is
                                // what the deleted viz gate did, and the
                                // outcome is the same.
                                if let Err(e) = adopted {
                                    tracing::debug!(
                                        toolpath = tp_index,
                                        "compute result not adopted: {e}"
                                    );
                                }
                            }
                            rt.result = Some(computed);
                            // Any toolpath whose `DerivedRestRegions` boundary
                            // depends on this one just saw its source result
                            // replaced (rest_regions may have appeared,
                            // changed, or vanished) — force a regenerate so
                            // the dependent re-resolves against the fresh
                            // regions instead of clipping against a stale set.
                            self.mark_derived_rest_dependents_stale(tp_id);
                        }
                        Err(ComputeError::Cancelled) => {
                            rt.status = ComputeStatus::Pending;
                            rt.result = None;
                            Self::forget_core_result(&mut self.state.session, tp_id);
                        }
                        Err(ComputeError::Message(error)) => {
                            rt.status = ComputeStatus::Error(error);
                            rt.result = None;
                            // G-STICKYEMPTY: clear the CORE cache too, not
                            // just `rt.result`. `Ok` writes both caches
                            // (`AdoptResult`, above); a failure used to
                            // clear only the viz one, so the previous
                            // parameter set's toolpath stayed in
                            // `session.results[idx]` — exportable,
                            // simulatable, and counted as "generated" by
                            // `PhantomPriorStockScan`, which is what
                            // withholds a pending rest op's phantom
                            // prior-stock snapshot and leaves it unable to
                            // regenerate until the project is reloaded.
                            Self::forget_core_result(&mut self.state.session, tp_id);
                        }
                    }
                    self.pending_upload = true;

                    // U4 reconciliation: clear this toolpath from the
                    // pending list. When the list empties we kick the
                    // reconciliation sim — this fires once, after the
                    // last applied toolpath finishes regenerating.
                    self.state
                        .pending_reconciliation_for_ids
                        .retain(|id| *id != tp_id);
                    if self.state.pending_reconciliation_for_ids.is_empty()
                        && matches!(
                            self.state.optimize_project.as_ref().map(|v| &v.status),
                            Some(crate::state::OptimizeProjectStatus::Reconciling(_))
                        )
                    {
                        let _submitted = self.run_simulation_with_all();
                    }

                    // Roadmap F.2 — auto-verify after per-TP Apply. If
                    // this regen is for the just-applied candidate,
                    // clear the pending flag and kick a full project
                    // sim so the user sees the verified live verdict
                    // without having to click Run Simulation by hand.
                    if self.state.pending_apply_resim == Some(tp_id.0) {
                        self.state.pending_apply_resim = None;
                        let _submitted = self.run_simulation_with_all();
                    }

                    // Resolve whoever is waiting on this toolpath — the MCP
                    // waiter, the `generate_all` ladder, or neither.
                    self.toolpath_completion_landed(tp_id);
                }
                ComputeMessage::Simulation(result) => match result {
                    Ok(simulation) => {
                        // N12 item 10 — one simulation state. The GUI
                        // simulates on its own lane, so the session never
                        // saw this answer and `ProjectSession::start`
                        // refused every `FromRemainingStock` operation in
                        // the GUI process. The core-typed copy is built
                        // HERE, before the view state consumes the fields;
                        // the cut trace is attached below, after the
                        // modulation post-pass rewrites it.
                        let mut adopted = core_simulation_from_lane(&simulation);
                        if simulation.resolution_clamped {
                            self.push_notification(
                                "Sim resolution was coarsened to fit grid limits — \
                                 consider reducing stock size or increasing resolution"
                                    .to_owned(),
                                crate::controller::Severity::Warning,
                            );
                        }
                        if simulation.mesh.indices.is_empty() {
                            self.push_notification(
                                "Simulation produced an empty mesh — \
                                 try increasing resolution or check stock dimensions"
                                    .to_owned(),
                                crate::controller::Severity::Warning,
                            );
                        }
                        let boundaries: Vec<_> = simulation
                            .boundaries
                            .iter()
                            .map(|boundary| crate::state::simulation::ToolpathBoundary {
                                id: boundary.id,
                                name: boundary.name.clone(),
                                tool_name: boundary.tool_name.clone(),
                                start_move: boundary.start_move,
                                end_move: boundary.end_move,
                                direction: boundary.direction,
                            })
                            .collect();

                        let setup_boundaries = {
                            let mut sbs = Vec::new();
                            let mut last_setup_id = None;
                            for boundary in &boundaries {
                                let setup_id = self.setup_of_toolpath(boundary.id);
                                if setup_id != last_setup_id {
                                    if let Some(setup_id) = setup_id {
                                        let setup_name = self
                                            .state
                                            .session
                                            .list_setups()
                                            .iter()
                                            .find(|s| s.id == setup_id.0)
                                            .map(|s| s.name.clone())
                                            .unwrap_or_default();
                                        sbs.push(crate::state::simulation::SetupBoundary {
                                            setup_id,
                                            setup_name,
                                            start_move: boundary.start_move,
                                        });
                                    }
                                    last_setup_id = setup_id;
                                }
                            }
                            sbs
                        };

                        let checkpoints: Vec<_> = simulation
                            .checkpoints
                            .into_iter()
                            .map(|checkpoint| crate::state::simulation::SimCheckpoint {
                                boundary_index: checkpoint.boundary_index,
                                core: checkpoint,
                            })
                            .collect();

                        // F.4 — retain the per-toolpath (and phantom)
                        // prior-stock snapshots so the submit-time
                        // FromRemainingStock gate can look them up by id.
                        let prior_stocks = simulation.prior_stocks;

                        if !simulation.rapid_collisions.is_empty() {
                            tracing::warn!(
                                "{} rapid collisions detected",
                                simulation.rapid_collisions.len()
                            );
                        }
                        self.state.simulation.checks.rapid_collisions = simulation.rapid_collisions;
                        self.state.simulation.checks.rapid_collision_move_indices =
                            simulation.rapid_collision_move_indices;

                        self.state.simulation.playback.display_deviations = simulation.deviations;
                        self.state.simulation.playback.display_mesh = None;
                        self.state.simulation.playback.display_mesh_move = None;
                        self.state.simulation.playback.last_mesh_upload_at = None;
                        self.state.simulation.playback.tool_gpu_move = None;
                        self.state.simulation.playback.display_mesh_preview = false;
                        self.state.simulation.playback.scrub_drag_active = false;

                        let stock = self.state.session.stock_config();
                        let stock_bbox = rs_cam_core::geo::BoundingBox3 {
                            min: rs_cam_core::geo::P3::new(0.0, 0.0, 0.0),
                            max: rs_cam_core::geo::P3::new(stock.x, stock.y, stock.z),
                        };

                        self.state.simulation.results = Some(SimulationResults {
                            mesh: simulation.mesh,
                            total_moves: simulation.total_moves,
                            boundaries,
                            setup_boundaries,
                            checkpoints,
                            selected_toolpaths: None,
                            playback_data: simulation.playback_data,
                            stock_bbox,
                            cut_trace: simulation.cut_trace,
                            cut_trace_path: simulation.cut_trace_path,
                            column_grid_cell_mm: simulation.column_grid_cell_mm,
                            prior_stocks,
                        });

                        // F-039 — apply adaptive feed modulation to the
                        // just-completed sim trace (unified load model §10.6,
                        // option A). The async worker runs the dexel sim only;
                        // modulation needs session context (material / machine
                        // / vendor LUT) so it runs here on the main thread. The
                        // post-pass stamps `modulation_summaries` onto the trace
                        // — so the Feeds-tab "operating point" card + the
                        // tool-load report populate — and swaps the modulated
                        // toolpaths into `session.results`, which G-code export
                        // reads, so exported feeds are the optimized per-move
                        // schedule. That last clause was aspirational until
                        // 2026-08-22: the viz exporter read the worker's
                        // pre-modulation IR out of `gui.toolpath_rt` instead
                        // (G-MODEXPORT). `io::export::emitted_toolpaths` now
                        // resolves from `session.results`, so the claim holds
                        // on the GUI/MCP path as well as the CLI one — sentried
                        // by `tests/modulated_feeds_reach_gcode_g_modexport.rs`.
                        // Default-on in the GUI. Take the trace out
                        // and put it back so the session (results) and the
                        // viz-side cut_trace are borrowed disjointly.
                        {
                            let opts = rs_cam_core::session::SimulationOptions {
                                adaptive_feed_modulation: true,
                                modulation_strategy:
                                    rs_cam_core::feed_modulation::ModulationStrategy::ConstrainedMax,
                                modulation_aggressiveness: 1.0,
                                ..Default::default()
                            };
                            let mut cut_trace = self
                                .state
                                .simulation
                                .results
                                .as_mut()
                                .and_then(|r| r.cut_trace.take());
                            if cut_trace.is_some() {
                                self.state
                                    .session
                                    .modulate_simulation_trace(&mut cut_trace, &opts);
                                if let Some(results) = self.state.simulation.results.as_mut() {
                                    results.cut_trace = cut_trace;
                                }
                            }
                        }

                        // N12 item 10 — the session adopts the simulation
                        // the viewport just adopted, so `start` sees the
                        // prior stock a `FromRemainingStock` operation
                        // needs.
                        //
                        // The trace is attached HERE and not in
                        // `core_simulation_from_lane`. The modulation pass
                        // above rewrites the trace through `Arc::make_mut`,
                        // which COPIES while a second `Arc` exists — so a
                        // copy taken before that pass would leave the
                        // session reading the pre-modulation trace and the
                        // viewport reading the modulated one. Taken after
                        // it, the two share one `Arc`.
                        adopted.cut_trace = self
                            .state
                            .simulation
                            .results
                            .as_ref()
                            .and_then(|results| results.cut_trace.as_ref())
                            .map(Arc::clone);
                        let args = AdoptSimulationArgs {
                            result: Box::new(adopted),
                        };
                        let adopt = Command::AdoptSimulation(args);
                        // WP19: the `Ok` arm stays absent on purpose.
                        // Storing a simulation leaves the session's
                        // `Some`, so `stale` is empty and
                        // `simulation_cleared` is false.
                        if let Err(error) = self.state.session.apply(adopt) {
                            tracing::warn!("simulation not adopted into the session: {error}");
                        }

                        let inspect_target =
                            self.state.simulation.debug.pending_inspect_toolpath.take();
                        if let Some(move_index) = inspect_target.and_then(|toolpath_id| {
                            self.state
                                .simulation
                                .boundaries()
                                .iter()
                                .find(|boundary| boundary.id == toolpath_id)
                                .map(|boundary| boundary.start_move)
                        }) {
                            self.state.simulation.playback.current_move = move_index;
                            self.state.simulation.playback.playing = false;
                        } else {
                            self.state.simulation.playback.current_move = 0;
                            self.state.simulation.playback.playing = false;
                        }

                        let initial_stock = TriDexelStock::from_bounds(
                            &stock_bbox,
                            self.state.simulation.resolution,
                        );
                        self.state.simulation.playback.live_stock = Some(initial_stock);
                        self.state.simulation.playback.live_sim_move = 0;
                        // Unclaimed: this stock is in the global frame, and
                        // whether that is the right frame for the group the
                        // playhead lands in is a question only the new
                        // `playback_data` can answer (G-LATERALSCRUB). Leaving
                        // a previous run's group id here would let a lateral
                        // group inherit a global-frame stock unchallenged,
                        // because the group ORDINAL can match across runs
                        // while the frame does not.
                        self.state.simulation.playback.live_stock_group = None;

                        let prev_gen = self
                            .state
                            .simulation
                            .last_run
                            .as_ref()
                            .map_or(0, |m| m.sim_generation);
                        // G-LATESIM (F2.10): the counter as it stood at
                        // SUBMIT, not now. Stamping the live counter here
                        // recorded the run as having been made against every
                        // edit that landed while it ran, so an operator who
                        // changed a parameter mid-simulation was shown the
                        // old run as current evidence — with no "stale" chip,
                        // no Readiness warning and no dimmed readout.
                        //
                        // The result itself is KEPT, exactly as F2.4 keeps a
                        // late toolpath result: a simulation is minutes of
                        // work, and discarding it would leave the operator
                        // with nothing and no way to tell a cancelled run
                        // from one that never happened. Stored, and marked
                        // not-current.
                        let submitted_at = self
                            .state
                            .simulation
                            .submitted_edit_counter
                            .take()
                            .unwrap_or(self.state.gui.edit_counter);
                        self.state.simulation.last_run = Some(SimulationRunMeta {
                            sim_generation: prev_gen + 1,
                            last_sim_edit_counter: submitted_at,
                            // Recording preferences are runtime-only. This
                            // result carries the capture revision it was
                            // SUBMITTED with; a toggle while the worker ran
                            // still needs a re-run, and an unstamped late
                            // result reads stale, never current.
                            accepted_metric_options_revision: self
                                .state
                                .simulation
                                .submitted_metric_options_revision
                                .take(),
                        });

                        self.pending_upload = true;

                        // U4 reconciliation: if the rollup is in
                        // Reconciling state, populate per-row
                        // reconciled values from the new trace and
                        // transition to Reconciled.
                        self.maybe_finalize_reconciliation();

                        // Notify pending MCP simulation request
                        #[cfg(feature = "mcp")]
                        self.notify_mcp_simulation_complete();
                        // A/M11: if this simulation was the fixpoint loop's
                        // own, the blocked rest ops can now see their upstream
                        // stock — start the next round. Ungated since Phase O:
                        // the GUI's Generate All runs the same ladder.
                        self.resume_generate_all_after_simulation(None);
                    }
                    Err(ComputeError::Cancelled) => {
                        let _ = (
                            self.state.simulation.submitted_edit_counter.take(),
                            self.state
                                .simulation
                                .submitted_metric_options_revision
                                .take(),
                        );
                        #[cfg(feature = "mcp")]
                        self.notify_mcp_simulation_error("Simulation cancelled");
                        self.resume_generate_all_after_simulation(Some(
                            "the simulation was cancelled".to_owned(),
                        ));
                    }
                    Err(ComputeError::Message(error)) => {
                        let _ = (
                            self.state.simulation.submitted_edit_counter.take(),
                            self.state
                                .simulation
                                .submitted_metric_options_revision
                                .take(),
                        );
                        tracing::error!("Simulation failed: {error}");
                        self.push_notification(
                            format!("Simulation failed: {error}"),
                            super::super::Severity::Error,
                        );
                        #[cfg(feature = "mcp")]
                        self.notify_mcp_simulation_error(&error);
                        self.resume_generate_all_after_simulation(Some(error));
                    }
                },
                ComputeMessage::Collision(result) => match result {
                    Ok(collision) => {
                        let count = collision.report.collisions.len();
                        if count == 0 {
                            tracing::info!("No holder clearance issues detected");
                            self.push_notification(
                                "No holder clearance issues detected".into(),
                                super::super::Severity::Info,
                            );
                        } else {
                            let msg = format!(
                                "{} holder clearance issues, min safe stickout: {:.1} mm",
                                count, collision.report.min_safe_stickout
                            );
                            tracing::warn!("{msg}");
                            self.push_notification(msg, super::super::Severity::Warning);
                        }
                        self.state.simulation.checks.holder_collision_count = count;
                        self.state.simulation.checks.min_safe_stickout = if count > 0 {
                            Some(collision.report.min_safe_stickout)
                        } else {
                            None
                        };
                        // G-HOLDERSTALE (F2.12): record WHEN this verdict was
                        // asked for, from the submit stamp rather than from
                        // the live counter, so an edit made while the lane
                        // worked is not folded into the record of when the
                        // check ran. Same rule F2.10 gave the simulation.
                        //
                        // The result itself is KEPT and marked not-current,
                        // for F2.10's reason: it is still the only evidence
                        // there is, and an emptied row cannot be told from
                        // one that never ran.
                        //
                        // A result with NO stamp of its own cannot say when
                        // it was measured. The simulation falls back to the
                        // live counter there; this row does not, because a
                        // holder verdict is a safety claim and "I do not
                        // know when this was measured" is not one to make.
                        // `None` reads as "Not checked".
                        self.state.simulation.checks.checked_at_edit_counter = self
                            .state
                            .simulation
                            .submitted_collision_edit_counter
                            .take();
                        // G-HOLDERSCOPE (F2.13): and the population the check
                        // covered, from the same submit. A result with no
                        // stamp of its own leaves the EMPTY population, which
                        // `covers_the_job` refuses — an unstamped verdict
                        // cannot claim the job.
                        self.state.simulation.checks.checked_scope = self
                            .state
                            .simulation
                            .submitted_collision_scope
                            .take()
                            .unwrap_or_default();
                        // Extract MCP response data before moving ownership
                        #[cfg(feature = "mcp")]
                        let mcp_collision_count = collision.report.collisions.len();
                        #[cfg(feature = "mcp")]
                        let mcp_min_safe_stickout = collision.report.min_safe_stickout;
                        #[cfg(feature = "mcp")]
                        let mcp_is_clear = collision.report.is_clear();

                        self.state.simulation.checks.collision_report = Some(collision.report);
                        self.collision_positions = collision.positions;
                        self.pending_upload = true;

                        // Notify pending MCP collision request
                        #[cfg(feature = "mcp")]
                        self.notify_mcp_collision_complete(
                            mcp_collision_count,
                            mcp_min_safe_stickout,
                            mcp_is_clear,
                        );
                    }
                    Err(ComputeError::Cancelled) => {
                        // The stamp describes a check that produced nothing.
                        // Clearing it here keeps it from being read by a
                        // later arrival that had no submit of its own.
                        self.state.simulation.submitted_collision_edit_counter = None;
                        self.state.simulation.submitted_collision_scope = None;
                        #[cfg(feature = "mcp")]
                        self.notify_mcp_collision_error("Collision check cancelled");
                    }
                    Err(ComputeError::Message(error)) => {
                        self.state.simulation.submitted_collision_edit_counter = None;
                        self.state.simulation.submitted_collision_scope = None;
                        tracing::error!("Collision check failed: {error}");
                        self.push_notification(
                            format!("Collision check failed: {error}"),
                            super::super::Severity::Error,
                        );
                        #[cfg(feature = "mcp")]
                        self.notify_mcp_collision_error(&error);
                    }
                },
                ComputeMessage::Optimize(result) => {
                    self.handle_optimize_result(*result);
                }
                ComputeMessage::Reach(result) => {
                    self.handle_reach_map_result(*result);
                }
                ComputeMessage::Job(result) => {
                    self.handle_job_result(*result);
                }
            }
        }
    }

    /// Submit one core `Job` row's work step, and hold the MCP caller's
    /// oneshot until it answers (WP14a).
    ///
    /// The handle already holds every input, so the session stays here and
    /// stays usable: this is what stops the two slow reads blocking the
    /// frame loop.
    #[cfg(feature = "mcp")]
    pub(crate) fn submit_mcp_job(
        &mut self,
        handle: rs_cam_core::session::JobHandle,
        cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
        render: crate::mcp_bridge::McpJobRender,
        response_tx: tokio::sync::oneshot::Sender<crate::mcp_bridge::McpResponse>,
    ) {
        let id = crate::compute::JobRequestId(self.next_job_request_id);
        self.next_job_request_id = self.next_job_request_id.saturating_add(1);
        let Some(pending) = self.pending_mcp.as_mut() else {
            let _ = response_tx.send(crate::mcp_bridge::McpResponse {
                result: Err("MCP compute tracking not initialized".to_owned()),
            });
            return;
        };
        pending.jobs.insert(
            id,
            crate::mcp_bridge::PendingMcpJob {
                sender: response_tx,
                render,
            },
        );
        self.compute
            .submit_job(crate::compute::JobRequest { id, handle, cancel });
    }

    /// Submit one core `Job` row's work step the GUI started for ITSELF
    /// (WP14b).
    ///
    /// The id comes from the same counter `submit_mcp_job` draws from, so
    /// the two maps never key one submit twice. The flag is this submit's
    /// own: a close arm arms it, and the lane's shared FIFO flag is left
    /// alone so an MCP caller's job behind this one survives.
    ///
    /// **This is the ONE stamp site for `AppState::optimize_run` on the two
    /// `Job` rows** (WP24). The `target` parameter already names the kind
    /// and the id is in scope, so neither caller needs the id back. The
    /// caller must run `AppState::close_modals_for_exclusivity` BEFORE this
    /// call: that rule spares a RUNNING Optimize, so a stamp ahead of it
    /// would spare the previous settled modal instead of closing it.
    ///
    /// `progress` is what the candidate search publishes about itself
    /// (WP29). The Optimize submit builds one `Arc`, attaches it to the
    /// handle and passes the same `Arc` here, so the row reads what the
    /// worker writes. The tier-map preview runs no candidate ladder and
    /// passes `None`.
    pub(crate) fn submit_gui_job(
        &mut self,
        handle: rs_cam_core::session::JobHandle,
        cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
        target: crate::controller::GuiJobTarget,
        progress: Option<std::sync::Arc<rs_cam_core::tool_load::optimize::OptimizeProgress>>,
    ) {
        let id = crate::compute::JobRequestId(self.next_job_request_id);
        self.next_job_request_id = self.next_job_request_id.saturating_add(1);
        let kind = match &target {
            crate::controller::GuiJobTarget::OptimizeModal { toolpath_id } => {
                crate::state::OptimizeRunKind::Toolpath {
                    toolpath_id: *toolpath_id,
                }
            }
            crate::controller::GuiJobTarget::MultitoolPreview => {
                crate::state::OptimizeRunKind::MultitoolPreview
            }
        };
        self.state.optimize_run = Some(crate::state::OptimizeRun {
            kind,
            job_id: Some(id),
            started_at: std::time::Instant::now(),
            cancel_requested: false,
            progress,
        });
        self.gui_jobs.insert(
            id,
            crate::controller::PendingGuiJob {
                target,
                cancel: std::sync::Arc::clone(&cancel),
            },
        );
        self.compute
            .submit_job(crate::compute::JobRequest { id, handle, cancel });
    }

    /// Deliver one finished `Job` answer to whoever asked for it.
    ///
    /// There is no adopt step. The two read rows touch nothing, and
    /// `optimize_toolpath` ran on its handle's own cloned session, so the
    /// answer goes to the requester and the live session is untouched.
    ///
    /// **The GUI map is read FIRST and outside the feature gate.**
    /// `pending_mcp` is `None` in every build launched without `--mcp`,
    /// so a lookup behind the gate would never clear
    /// `AppState::optimize_run` and the progress row would stay up for
    /// good, with every Optimize entry point refused behind it.
    fn handle_job_result(&mut self, result: crate::compute::JobResult) {
        // Destructured here, above the feature gate: the answer is consumed
        // by value, which is what the gate's `needless_pass_by_value` rule
        // asks of a parameter this method takes by value.
        let crate::compute::JobResult { id, answer } = result;
        if let Some(job) = self.gui_jobs.remove(&id) {
            self.deliver_gui_job(&job.target, answer);
            return;
        }
        #[cfg(feature = "mcp")]
        {
            let Some(pending) = self.pending_mcp.as_mut() else {
                return;
            };
            let Some(job) = pending.jobs.remove(&id) else {
                // An answer whose caller went away. Not an error.
                return;
            };
            let reply = crate::mcp_bridge::render_job_answer(&job.render, &answer);
            let _ = job
                .sender
                .send(crate::mcp_bridge::McpResponse { result: Ok(reply) });
        }
        #[cfg(not(feature = "mcp"))]
        {
            // No MCP surface in this build, so nothing else is waiting.
            let _ = id;
            drop(answer);
        }
    }

    /// Land one GUI-started `Job` answer on the view that asked for it.
    ///
    /// `AppState::optimize_run` clears HERE for both targets, on every
    /// outcome — the answer, a cancel and a panic alike. It is the only
    /// place a GUI-started job can clear it, so an early return above this
    /// line leaves the progress row up and every Optimize entry point
    /// refused behind it.
    fn deliver_gui_job(
        &mut self,
        target: &crate::controller::GuiJobTarget,
        answer: Result<rs_cam_core::session::JobAnswer, crate::compute::ComputeError>,
    ) {
        use rs_cam_core::session::JobAnswer;

        self.state.optimize_run = None;
        match target {
            crate::controller::GuiJobTarget::OptimizeModal { toolpath_id } => {
                let status = match answer {
                    Ok(JobAnswer::OptimizeToolpath(outcome)) => {
                        crate::state::OptimizeRunStatus::Ready(*outcome)
                    }
                    Ok(other) => crate::state::OptimizeRunStatus::Failed(format!(
                        "the Optimize modal was answered by the {} row",
                        other.id().wire_name()
                    )),
                    Err(error) => crate::state::OptimizeRunStatus::Failed(error.to_string()),
                };
                self.land_optimize_outcome(*toolpath_id, status);
            }
            crate::controller::GuiJobTarget::MultitoolPreview => {
                let result = match answer {
                    Ok(JobAnswer::PreviewTierMap(preview)) => Ok(preview),
                    Ok(other) => Err(format!(
                        "the planner dialog was answered by the {} row",
                        other.id().wire_name()
                    )),
                    Err(error) => Err(error.to_string()),
                };
                self.handle_multitool_preview_result(result);
            }
        }
    }

    /// Write one per-toolpath Optimize status into the open modal.
    ///
    /// A modal reopened on a different toolpath while the job ran keeps
    /// its own state: the outcome names an operation the operator is no
    /// longer looking at.
    fn land_optimize_outcome(
        &mut self,
        toolpath_id: crate::state::toolpath::ToolpathId,
        status: crate::state::OptimizeRunStatus,
    ) {
        let Some(modal) = self.state.optimize_modal.as_mut() else {
            tracing::debug!(
                "Optimize result for tp {toolpath_id} arrived after modal was closed — discarded"
            );
            return;
        };
        if modal.toolpath_id != toolpath_id {
            tracing::debug!(
                "Optimize result for tp {toolpath_id} dropped — modal now open on tp {}",
                modal.toolpath_id
            );
            return;
        }
        modal.status = status;
    }

    /// Land a reach map on the viewport overlay (P5).
    ///
    /// Three outcomes, and they are deliberately not collapsed:
    ///
    /// * A result for a toolpath the overlay is no longer asking about is a
    ///   stale supersede. It is DROPPED, never shown — the operator has
    ///   already moved on, and the sweep has already asked for the new one.
    /// * A cancelled walk is DROPPED too, and it must not clear the key.
    ///   The Reach lane cancels only when a replacement has just been queued,
    ///   so the `Cancelled` is bookkeeping, exactly as G-REGEN-RACE is on the
    ///   toolpath lane. Clearing the key here instead makes the sweep submit a
    ///   third walk that cancels the second, whose `Cancelled` clears the key
    ///   again — a live-lock that never draws an overlay. The one case this
    ///   leaves behind, a lane cancelled with nothing queued behind it, is
    ///   recovered by `process_reach_overlay`'s idle-lane check.
    /// * A real error is kept and shown. A failed measurement must not read
    ///   as a clean part.
    ///
    /// `generation` is bumped only when the map is not the one this overlay
    /// last accepted (`ReachOverlayState::last_map`, NOT the current status —
    /// the scheduler sets `Computing` on submit, so the status never holds a
    /// map to compare against by the time a result lands). The reach memo
    /// returns the same `Arc` for a warm key, so re-selecting a toolpath
    /// rebuilds no GPU buffer.
    fn handle_reach_map_result(&mut self, result: crate::compute::ReachResult) {
        use crate::state::runtime::ReachStatus;

        if self.state.gui.reach_overlay.toolpath != Some(result.toolpath_id) {
            tracing::debug!(
                "Reach map for tp {} dropped — the overlay now follows a different selection",
                result.toolpath_id
            );
            return;
        }
        let overlay = &mut self.state.gui.reach_overlay;
        match result.result {
            Ok(map) => {
                let is_new_map = overlay
                    .last_map
                    .as_ref()
                    .is_none_or(|held| !Arc::ptr_eq(held, &map));
                if is_new_map {
                    overlay.generation = overlay.generation.saturating_add(1);
                }
                overlay.last_map = Some(Arc::clone(&map));
                overlay.colors = Some(result.colors);
                overlay.status = ReachStatus::Ready(map);
                self.pending_upload = true;
            }
            Err(ComputeError::Cancelled) => {
                tracing::debug!(
                    "Reach map for tp {} was superseded — a replacement walk is queued",
                    result.toolpath_id
                );
            }
            Err(ComputeError::Message(message)) => {
                overlay.colors = None;
                overlay.status = ReachStatus::Failed(message);
                self.pending_upload = true;
            }
        }
    }

    /// Stash the project rollup's report on `AppState`.
    ///
    /// The lane no longer carries a session back: WP14b gives it a CLONE,
    /// so the main thread never lost its own and there is nothing to
    /// restore. `AppState::optimize_run` still clears here, because the
    /// rollup is the one arm left on this lane; the two `Job` rows clear it
    /// in [`Self::deliver_gui_job`].
    fn handle_optimize_result(&mut self, result: crate::compute::OptimizeResult) {
        use crate::compute::OptimizeResultKind;

        self.state.optimize_run = None;

        match result.kind {
            OptimizeResultKind::Project { report } => {
                // Default-select every row that has a recommended
                // candidate. The user can flip individual rows before
                // clicking Apply selected.
                let row_selected: Vec<bool> = report
                    .per_toolpath
                    .iter()
                    .map(|(_, outcome)| outcome.first_safe().is_some())
                    .collect();
                if let Some(view) = self.state.optimize_project.as_mut() {
                    view.status = crate::state::OptimizeProjectStatus::Ready(report);
                    view.row_selected = row_selected;
                } else {
                    // Worker returned a result for a view that was
                    // already closed (cancelled) — discard.
                    tracing::debug!(
                        "Project optimize result arrived after view was closed — discarded"
                    );
                }
            }
        }
    }

    /// Land a tier-map preview on the planner dialog.
    ///
    /// A result that arrives after the dialog was vetoed is DISCARDED, not
    /// applied to a closed dialog: the operator already said no, and quietly
    /// re-arming the overlay behind them would be the veto failing. The
    /// session was never lent out either way — since WP14b the preview runs
    /// on the `Job` lane over its own handle.
    fn handle_multitool_preview_result(
        &mut self,
        result: Result<Box<rs_cam_core::session::MultitoolPreview>, String>,
    ) {
        let Some(planner) = self.state.multitool_planner.as_mut() else {
            tracing::debug!("Tier-map preview arrived with no planner state — discarded");
            return;
        };
        if !planner.open {
            planner.status = crate::state::multitool_planner::MultitoolPreviewStatus::Idle;
            tracing::debug!("Tier-map preview arrived after the dialog was closed — discarded");
            return;
        }
        match result {
            Ok(preview) => {
                let key = planner.requested_key.take();
                planner.previewed_key = key;
                planner.preview_generation = planner.preview_generation.saturating_add(1);
                planner.status =
                    crate::state::multitool_planner::MultitoolPreviewStatus::Ready(preview);
                self.state.viewport.show_tier_preview = true;
                self.pending_upload = true;
            }
            Err(message) => {
                planner.requested_key = None;
                planner.status =
                    crate::state::multitool_planner::MultitoolPreviewStatus::Failed(message);
            }
        }
    }

    /// U4: if the rollup is in `Reconciling` state and the latest sim
    /// result just landed, populate `reconciled_cycle_time_s` /
    /// `reconciled_verdict` per applied row and transition to
    /// `Reconciled`. Each row's reconciled values come from running
    /// the existing project load report against the new trace and
    /// reading per-toolpath cycle times directly off the trace.
    pub(crate) fn maybe_finalize_reconciliation(&mut self) {
        use rs_cam_core::tool_load::optimize::{OutcomeKind, ProjectOptimizeReport};

        let Some(view) = self.state.optimize_project.as_ref() else {
            return;
        };
        let crate::state::OptimizeProjectStatus::Reconciling(report) = &view.status else {
            return;
        };
        let Some(sim_results) = self.state.simulation.results.as_ref() else {
            return;
        };
        let Some(trace) = sim_results.cut_trace.as_deref() else {
            return;
        };

        // Compute per-toolpath verdicts off the new trace.
        let load_report = rs_cam_core::gcode::project_load_report(&self.state.session, Some(trace));
        // Map toolpath_id -> verdict for fast lookup. Index by id
        // (not toolpath_index) because the report uses ids.
        let mut verdict_by_id: std::collections::HashMap<
            rs_cam_core::ToolpathId,
            &rs_cam_core::tool_load::ToolpathLoadVerdict,
        > = std::collections::HashMap::new();
        for v in &load_report.per_toolpath {
            verdict_by_id.insert(v.toolpath_id, v);
        }

        // Walk the report and populate the first_safe candidate's
        // reconciled values for each row that has one.
        let mut updated_report = report.clone();
        for (tp_index, outcome) in updated_report.per_toolpath.iter_mut() {
            let toolpath_id = self
                .state
                .session
                .toolpath_configs()
                .get(*tp_index)
                .map(|tc| tc.id);
            let Some(toolpath_id) = toolpath_id else {
                continue;
            };
            let cycle = trace
                .toolpath_summaries
                .iter()
                .find(|s| s.toolpath_id == toolpath_id)
                .map(|s| s.total_runtime_s);
            let verdict = verdict_by_id.get(&toolpath_id).cloned().cloned();
            if outcome.kind == OutcomeKind::Ranked {
                // The applied candidate is `first_safe` per the rollup
                // Apply path. Find its index by re-running the same
                // selector, then update that candidate. We fall back
                // to candidate index 1 if the rollup logic disagrees
                // (e.g. the candidate set changed mid-flight, which
                // shouldn't happen since the report is immutable).
                let candidates = &mut outcome.candidates;
                let idx = ProjectOptimizeReport::first_safe_index(candidates).unwrap_or(0);
                if idx > 0
                    && let Some(c) = candidates.get_mut(idx)
                {
                    c.reconciled_cycle_time_s = cycle;
                    c.reconciled_verdict = verdict;
                }
            }
        }

        if let Some(view) = self.state.optimize_project.as_mut() {
            view.status = crate::state::OptimizeProjectStatus::Reconciled(updated_report);
        }
    }

    // ── A/M11: generate_all as a fixpoint over the rest-stock chain ──

    /// Start a `generate_all` from the MCP tool. When `fixpoint` is on this
    /// iterates generate -> simulate -> generate until nothing new appears;
    /// see [`crate::controller::generate_all::FixpointPlan`] for the
    /// termination argument.
    ///
    /// `simulation_resolution_mm` is refused rather than defaulted when the
    /// project needs it (A/M10): a silently chosen cell size changes
    /// collision counts and engagement, so the loop must not pick one.
    #[cfg(feature = "mcp")]
    pub(crate) fn mcp_start_generate_all(
        &mut self,
        fixpoint: bool,
        simulation_resolution_mm: Option<f64>,
        response_tx: tokio::sync::oneshot::Sender<crate::mcp_bridge::McpResponse>,
        progress_tx: Option<tokio::sync::mpsc::Sender<crate::mcp_bridge::ProgressUpdate>>,
    ) {
        use crate::controller::generate_all::{GenerateAllSink, generate_all_scope, plan_fixpoint};
        use crate::mcp_bridge::McpResponse;

        let scope = generate_all_scope(self.state.session.toolpath_configs());

        if scope.enabled.is_empty() {
            let _ = response_tx.send(McpResponse {
                result: Ok(rs_cam_mcp::server::text("No enabled toolpaths to generate")),
            });
            return;
        }

        let plan = match plan_fixpoint(fixpoint, &scope.rest_op_indices, simulation_resolution_mm) {
            Ok(plan) => plan,
            Err(missing) => {
                let bad = missing.supplied_clause();
                let _ = response_tx.send(McpResponse {
                    result: Ok(rs_cam_mcp::server::json_str(serde_json::json!({
                        "ok": false,
                        "error": format!(
                            "generate_all needs `simulation_resolution_mm`, and {bad}. This \
                             project has {} enabled rest-machining operation(s) (indices {:?}) \
                             whose stock comes from a simulation, so reaching a fully \
                             generated state requires running simulations between generate \
                             rounds. The resolution is NOT guessed: collision counts and \
                             engagement both move with cell size, so a silently chosen one \
                             would hand you verdicts you did not ask for. Pass the same \
                             resolution you will use for verification — well below the \
                             finishing tool's TIP radius (e.g. 0.1 for a 1 mm ball). To skip \
                             the ladder and get the old single-pass behaviour, pass \
                             `fixpoint: false`.",
                            missing.rest_op_indices.len(),
                            missing.rest_op_indices,
                        ),
                    }))),
                });
                return;
            }
        };

        // Checked before anything is queued: without the slot no individual
        // toolpath waiter can ever be resolved, so starting the run would
        // strand it.
        if self.pending_mcp.is_none() {
            let _ = response_tx.send(McpResponse {
                result: Err("MCP compute tracking not initialized".to_owned()),
            });
            return;
        }

        // MCP-only: `get_generation_debug_trace` needs the generator's
        // step-by-step output, and an agent has no other way to turn it on
        // mid-call. The GUI ladder leaves the operator's own
        // "capture generator trace" toggle alone.
        //
        // WP7: one `SetToolpathDebugOptions` per index, the same row
        // `mcp_generate_toolpath` and the GUI's own capture toggle take.
        // The row moves no revision and drops no result, because a debug
        // trace is an OUTPUT of a generation and never an input to one.
        //
        // WP19 `let _ =`: `Effects::stale` is therefore empty and
        // `simulation_cleared` is false. There is nothing to mirror.
        let count = self.state.session.toolpath_count();
        for index in 0..count {
            let Some(tc) = self.state.session.get_toolpath_config(index) else {
                continue;
            };
            if !tc.enabled || tc.debug_options.enabled {
                continue;
            }
            let mut debug_options = tc.debug_options;
            debug_options.enabled = true;
            let _ =
                self.state
                    .session
                    .apply(rs_cam_core::session::Command::SetToolpathDebugOptions(
                        rs_cam_core::session::SetToolpathDebugOptionsArgs {
                            index,
                            debug_options,
                        },
                    ));
        }

        self.start_generate_all(
            scope.enabled,
            plan,
            GenerateAllSink::Mcp {
                response_tx,
                progress_tx,
            },
        );
    }

    /// Arm the ladder and submit round 1. Shared entry point: the MCP tool and
    /// the GUI's Generate All differ only in [`GenerateAllSink`].
    pub(crate) fn start_generate_all(
        &mut self,
        ids: Vec<crate::state::toolpath::ToolpathId>,
        fixpoint: crate::controller::generate_all::FixpointPlan,
        sink: crate::controller::generate_all::GenerateAllSink,
    ) {
        use crate::controller::generate_all::{GenerateAllSink, PendingGenerateAll};

        let total = ids.len();
        match &sink {
            GenerateAllSink::Gui => {
                self.set_status(format!("Generate All: round 1, {total} operation(s)..."));
            }
            #[cfg(feature = "mcp")]
            GenerateAllSink::Mcp { progress_tx, .. } => {
                if let Some(tx) = progress_tx.as_ref() {
                    let _ = tx.try_send(crate::mcp_bridge::ProgressUpdate {
                        message: format!("Round 1: generating {total} toolpaths..."),
                        progress: 0.0,
                        total: Some(total as f64),
                    });
                }
            }
        }

        for &id in &ids {
            self.events.push(crate::ui::AppEvent::GenerateToolpath(id));
        }

        self.generate_all = Some(PendingGenerateAll::new(ids, fixpoint, sink));
    }

    /// Called whenever a round might have finished. Either advances the
    /// ladder (submitting the loop's own simulation) or resolves the caller.
    fn settle_generate_all_round(&mut self) {
        use crate::controller::generate_all::{GenerateAllSink, GenerateAllSummary};

        enum Next {
            Wait,
            Simulate(f64),
            Finish(Box<GenerateAllSummary>),
        }

        let next = {
            let Some(ga) = self.generate_all.as_mut() else {
                return;
            };
            if !ga.remaining.is_empty() || ga.fixpoint.awaiting_simulation {
                Next::Wait
            } else {
                // (a) something is blocked purely on sequencing, (b) the round
                // just finished produced at least one new op, and we are
                // inside the hard bound. All three, or we stop.
                let advance = ga.fixpoint.enabled
                    && !ga.blocked.is_empty()
                    && ga.fixpoint.completed_this_round > 0
                    && ga.fixpoint.round < ga.fixpoint.max_rounds;
                match (advance, ga.fixpoint.resolution_mm) {
                    (true, Some(res)) => {
                        ga.fixpoint.awaiting_simulation = true;
                        ga.fixpoint.simulations += 1;
                        Next::Simulate(res)
                    }
                    _ => Next::Finish(Box::new(ga.completed_summary())),
                }
            }
        };

        match next {
            Next::Wait => {}
            Next::Simulate(resolution) => {
                // G-RESNOTICE: say so before doing it. The write below is
                // unchanged and unconditional — only the operator's
                // knowledge of it is new.
                if let Some(notice) = crate::controller::generate_all::resolution_override_notice(
                    self.state.simulation.resolution,
                    self.state.simulation.auto_resolution,
                    resolution,
                ) {
                    self.push_notification(notice, super::super::Severity::Warning);
                }
                self.state.simulation.resolution = resolution;
                self.state.simulation.auto_resolution = false;
                if self.run_simulation_with_all_memoized(true) {
                    self.generate_all_progress(
                        "Simulating so the blocked rest operations can see their stock...",
                    );
                } else {
                    // Nothing to simulate — the completion we just armed will
                    // never drain. Unwind rather than hang.
                    self.resume_generate_all_after_simulation(Some(
                        "there was nothing to simulate, so the blocked operations can \
                         never see upstream stock"
                            .to_owned(),
                    ));
                }
            }
            Next::Finish(summary) => {
                // S5: the ladder is the only thing that asked the analysis
                // lane to retain a prefix snapshot, so it is the thing that
                // releases it. Without this the last round's snapshot would
                // sit on the lane until the next simulation consumed it.
                self.compute.clear_sim_prefix_cache();
                let Some(ga) = self.generate_all.take() else {
                    return;
                };
                match ga.sink {
                    GenerateAllSink::Gui => {
                        let severity = if summary.failed > 0 || summary.loop_error.is_some() {
                            super::super::Severity::Warning
                        } else {
                            super::super::Severity::Info
                        };
                        let message =
                            crate::controller::generate_all::generate_all_headline(&summary);
                        self.push_notification(message, severity);
                    }
                    #[cfg(feature = "mcp")]
                    GenerateAllSink::Mcp { response_tx, .. } => {
                        let _ = response_tx.send(crate::mcp_bridge::McpResponse {
                            result: Ok(crate::mcp_bridge::build_generate_all_response(&summary)),
                        });
                    }
                }
            }
        }
    }

    /// Hook from the simulation drain. Advances the ladder to the next round,
    /// or stops it when the simulation itself failed.
    pub(crate) fn resume_generate_all_after_simulation(&mut self, sim_error: Option<String>) {
        let retry: Vec<ToolpathId> = {
            let Some(ga) = self.generate_all.as_mut() else {
                return;
            };
            if !ga.fixpoint.awaiting_simulation {
                return;
            }
            ga.fixpoint.awaiting_simulation = false;
            if let Some(err) = sim_error {
                // Disable the loop as well as recording the error: leaving it
                // armed with a still-populated `blocked` list would re-arm the
                // same simulation forever.
                ga.fixpoint.enabled = false;
                ga.loop_error = Some(err);
                Vec::new()
            } else {
                ga.fixpoint.round += 1;
                ga.fixpoint.completed_this_round = 0;
                let retry: Vec<ToolpathId> = ga.blocked.drain(..).map(|(id, _)| id).collect();
                ga.remaining.clone_from(&retry);
                retry
            }
        };

        if retry.is_empty() {
            self.settle_generate_all_round();
            return;
        }
        let round = self.generate_all.as_ref().map_or(0, |ga| ga.fixpoint.round);
        self.generate_all_progress(&format!(
            "Round {round}: regenerating {} operation(s) that were waiting on upstream stock...",
            retry.len()
        ));
        for id in retry {
            self.events.push(crate::ui::AppEvent::GenerateToolpath(id));
        }
    }

    /// Report ladder progress on whichever surface started it, with the
    /// completion count the caller already knows.
    ///
    /// `progress` and `total` reach the MCP progress channel alone. The
    /// GUI sink writes the status line and reads the message only, so a
    /// build without the `mcp` feature never reads the two numbers.
    #[cfg_attr(not(feature = "mcp"), allow(unused_variables))]
    fn generate_all_progress_at(&mut self, message: &str, progress: f64, total: Option<f64>) {
        use crate::controller::generate_all::GenerateAllSink;

        let is_gui = match self.generate_all.as_ref() {
            None => return,
            Some(ga) => match &ga.sink {
                GenerateAllSink::Gui => true,
                #[cfg(feature = "mcp")]
                GenerateAllSink::Mcp { progress_tx, .. } => {
                    if let Some(tx) = progress_tx.as_ref() {
                        let _ = tx.try_send(crate::mcp_bridge::ProgressUpdate {
                            message: message.to_owned(),
                            progress,
                            total,
                        });
                    }
                    false
                }
            },
        };
        if is_gui {
            self.set_status(message.to_owned());
        }
    }

    /// Ladder progress at whatever the run has completed so far.
    fn generate_all_progress(&mut self, message: &str) {
        let done = self
            .generate_all
            .as_ref()
            .map_or(0.0, |ga| (ga.completed + ga.failed) as f64);
        self.generate_all_progress_at(message, done, None);
    }

    /// A toolpath generation reached a terminal state — resolve whoever is
    /// waiting on it.
    ///
    /// Every submit-time rejection and every drained compute result funnels
    /// through here, so the ladder cannot miss a completion (the failure that
    /// left an MCP `generate_toolpath` unresolved for ~9 hours).
    fn toolpath_completion_landed(&mut self, tp_id: crate::state::toolpath::ToolpathId) {
        #[cfg(feature = "mcp")]
        self.notify_mcp_toolpath_complete(tp_id);
        self.record_generate_all_completion(tp_id);
    }

    /// Fold one finished toolpath into the in-flight ladder, then let the
    /// round settle.
    fn record_generate_all_completion(&mut self, tp_id: crate::state::toolpath::ToolpathId) {
        // Read before the `&mut` borrow below: the ladder's error rows name
        // the operation, and the config is gone from the session on the one
        // path that reports "toolpath runtime not found".
        let tp_name = self
            .state
            .session
            .find_toolpath_config_by_id(tp_id)
            .map(|(_, tc)| tc.name.clone())
            .unwrap_or_else(|| format!("toolpath {}", tp_id.0));
        let rt_status = self
            .state
            .gui
            .toolpath_rt
            .get(&tp_id)
            .map(|rt| (rt.result.is_some(), rt.status.clone()));

        let mut progress: Option<(usize, usize)> = None;
        if let Some(ga) = self.generate_all.as_mut()
            && let Some(pos) = ga.remaining.iter().position(|id| *id == tp_id)
        {
            ga.remaining.remove(pos);
            match rt_status {
                Some((true, _)) => {
                    ga.completed += 1;
                    ga.fixpoint.completed_this_round += 1;
                }
                // A/M11: a sequencing block goes in its own bucket and does
                // NOT count as a failure. The fixpoint loop retries exactly
                // this set after a simulation; a genuine error is never
                // retried, which is what makes the loop terminate.
                Some((false, ComputeStatus::AwaitingPriorStock(b))) => {
                    ga.blocked.push((tp_id, b.message));
                }
                Some((false, ComputeStatus::Error(e))) => {
                    ga.failed += 1;
                    ga.errors.push((tp_id.0, e));
                }
                // Roadmap E.4 — "completed cleanly with zero moves" is a
                // config issue (depth/stock/model), not a thrown error.
                Some((false, ComputeStatus::Done)) => {
                    ga.failed += 1;
                    ga.errors.push((
                        tp_id.0,
                        format!(
                            "{tp_name}: completed with no moves — check depth, stock, or \
                             model assignment"
                        ),
                    ));
                }
                // Cancel resets status to `Pending`.
                Some((false, ComputeStatus::Pending)) => {
                    ga.failed += 1;
                    ga.errors
                        .push((tp_id.0, format!("{tp_name}: generation cancelled")));
                }
                Some((false, status @ (ComputeStatus::Computing | ComputeStatus::Disabled))) => {
                    ga.failed += 1;
                    ga.errors.push((
                        tp_id.0,
                        format!("{tp_name}: no result, status={}", status.label()),
                    ));
                }
                None => {
                    ga.failed += 1;
                    ga.errors
                        .push((tp_id.0, format!("{tp_name}: toolpath runtime not found")));
                }
            }
            progress = Some((
                ga.completed + ga.failed,
                ga.completed + ga.failed + ga.remaining.len(),
            ));
        }

        if let Some((current, total)) = progress {
            self.generate_all_progress_at(
                &format!("Completed {current}/{total}: {tp_name}"),
                current as f64,
                Some(total as f64),
            );
        }

        // A/M11: a finished round either advances the ladder or resolves the
        // caller. Outside the borrow above because advancing needs `&mut self`
        // to submit a simulation.
        self.settle_generate_all_round();
    }

    // ── MCP notification helpers ─────────────────────────────────────
    #[cfg(feature = "mcp")]
    fn notify_mcp_toolpath_complete(&mut self, tp_id: crate::state::toolpath::ToolpathId) {
        use crate::mcp_bridge::McpResponse;
        use rs_cam_mcp::server::json_str;

        if let Some(ref mut pending) = self.pending_mcp {
            // Check individual toolpath request
            if let Some(sender) = pending.toolpath.remove(&tp_id) {
                let rt = self.state.gui.toolpath_rt.get(&tp_id);
                let resp = match rt.and_then(|rt| rt.result.as_ref()) {
                    Some(result) => json_str(serde_json::json!({
                        "id": tp_id.0,
                        "move_count": result.stats.move_count,
                        "cutting_distance_mm": result.stats.cutting_distance,
                        "rapid_distance_mm": result.stats.rapid_distance,
                    })),
                    None => {
                        let status = rt.map(|rt| &rt.status);
                        let status_msg = status
                            .map(|status| match status {
                                ComputeStatus::Error(e) => format!("Error: {e}"),
                                // A/M11: blocked is not failed, and the reply
                                // says which operation it is waiting for.
                                ComputeStatus::AwaitingPriorStock(b) => b.message.clone(),
                                // The only way a drained result reaches this
                                // arm with `Pending` is the cancel path a few
                                // lines above (`Err(ComputeError::Cancelled)`
                                // resets status to `Pending`) — call it out
                                // by name instead of the generic fallback so
                                // an MCP caller waiting on `cancel_generation`
                                // sees an unambiguous outcome.
                                ComputeStatus::Pending => "Generation was cancelled".to_owned(),
                                ComputeStatus::Computing
                                | ComputeStatus::Done
                                | ComputeStatus::Disabled => {
                                    "Toolpath generation produced no result".to_owned()
                                }
                            })
                            .unwrap_or_else(|| "Toolpath not found".to_owned());
                        let blocked = status.and_then(ComputeStatus::blocked_on);
                        json_str(serde_json::json!({
                            "error": status_msg,
                            "status": status.map_or("Pending", ComputeStatus::label),
                            "awaiting_prior_stock": blocked.map(|b| serde_json::json!({
                                "blocking_toolpath_id": b.blocking_toolpath_id,
                                "blocking_toolpath_index": b.blocking_toolpath_index,
                            })),
                        }))
                    }
                };
                let _ = sender.send(McpResponse { result: Ok(resp) });
            }
        }
    }

    #[cfg(feature = "mcp")]
    fn notify_mcp_simulation_complete(&mut self) {
        use crate::mcp_bridge::McpResponse;
        use rs_cam_mcp::server::json_str;

        if let Some(ref mut pending) = self.pending_mcp
            && let Some(sender) = pending.simulation.take()
        {
            let resp = self.build_mcp_diagnostics();
            let _ = sender.send(McpResponse {
                result: Ok(json_str(resp)),
            });
        }
    }

    #[cfg(feature = "mcp")]
    fn notify_mcp_simulation_error(&mut self, error: &str) {
        use crate::mcp_bridge::McpResponse;
        use rs_cam_mcp::server::json_str;

        if let Some(ref mut pending) = self.pending_mcp
            && let Some(sender) = pending.simulation.take()
        {
            let _ = sender.send(McpResponse {
                result: Ok(json_str(serde_json::json!({"error": error}))),
            });
        }
    }

    #[cfg(feature = "mcp")]
    fn notify_mcp_collision_complete(
        &mut self,
        collision_count: usize,
        min_safe_stickout: f64,
        is_clear: bool,
    ) {
        use crate::mcp_bridge::McpResponse;
        use rs_cam_mcp::server::json_str;

        if let Some(ref mut pending) = self.pending_mcp
            && let Some(sender) = pending.collision.take()
        {
            let resp = json_str(serde_json::json!({
                "collision_count": collision_count,
                "min_safe_stickout_mm": min_safe_stickout,
                "is_clear": is_clear,
            }));
            let _ = sender.send(McpResponse { result: Ok(resp) });
        }
    }

    /// Build the MCP `get_diagnostics` JSON.
    ///
    /// **TD3 wave B-5 (`G-RESULTS`).** The per-toolpath rows come from the
    /// core [`rs_cam_core::session::ToolpathDiagnostic`] — the SAME record the
    /// CLI's `project` report publishes — with the GUI-only lane columns
    /// (`toolpath_index`, `status`, `error`, `awaiting_prior_stock`, `stale`)
    /// layered on top. A toolpath with no core diagnostic (not generated,
    /// failed, or awaiting upstream stock) still gets its lane row, because
    /// "cannot yet" is the answer an agent came for.
    ///
    /// The rows used to be hand-built from `gui.toolpath_rt` alone, under a
    /// doc comment claiming `session.results` was "only populated by the
    /// standalone MCP". That stopped being true at `d706c036`, when
    /// `drain_compute_results` started writing the worker's result through
    /// `ProjectSession::insert_result` (`Command::AdoptResult` since WP3) —
    /// but the read was never moved, so ten
    /// published channels (`op_kind`, `collision_count`,
    /// `rapid_collision_count`, and the seven generation-finding areas) were
    /// absent from the agent-facing wire while the CLI's carried them. An
    /// absent key is strictly worse than a `null`: it cannot say "not
    /// measured" and it cannot say "measured clean" (X-19).
    ///
    /// Sentried by `controller::results_parity_tests`.
    #[cfg(feature = "mcp")]
    pub fn build_mcp_diagnostics(&self) -> serde_json::Value {
        let session = &self.state.session;
        let gui = &self.state.gui;

        // One evidence view and one core diagnostics build, shared by the
        // per-toolpath rows, the project totals and the triage block below.
        let evidence = crate::app::mcp::viz_project_evidence(&self.state);
        let core_diagnostics = session.diagnostics_with_evidence(&evidence);
        let core_rows: std::collections::HashMap<rs_cam_core::ToolpathId, serde_json::Value> =
            core_diagnostics
                .per_toolpath
                .iter()
                .filter_map(|d| {
                    serde_json::to_value(d)
                        .ok()
                        .map(|value| (d.toolpath_id, value))
                })
                .collect();

        let mut per_toolpath = Vec::new();
        let mut runtime_errors = Vec::new();
        // A/M11 — sequencing blocks, kept apart from failures.
        let mut awaiting_prior_stock = Vec::new();
        for (index, tc) in session.toolpath_configs().iter().enumerate() {
            let tool_name = session
                .tools()
                .iter()
                .find(|t| t.id.0 == tc.tool_id)
                .map(|t| t.name.clone())
                .unwrap_or_default();
            if let Some(rt) = gui.toolpath_rt.get(&tc.id) {
                // A/M11: one taxonomy, read through the canonical resolver.
                // `runtime_errors` carries genuine failures ONLY — a disabled
                // op reports `Disabled` and a sequencing block reports
                // `AwaitingPriorStock` on its own channel, so an agent can
                // tell "cannot yet" from "cannot ever" without parsing prose.
                let status = ComputeStatus::effective(tc.enabled, &rt.status);
                if let Some(error) = status.error_text() {
                    runtime_errors.push(serde_json::json!({
                        "toolpath_index": index,
                        "toolpath_id": tc.id,
                        "name": tc.name,
                        "error": error,
                    }));
                }
                if let Some(block) = status.blocked_on() {
                    awaiting_prior_stock.push(serde_json::json!({
                        "toolpath_index": index,
                        "toolpath_id": tc.id,
                        "name": tc.name,
                        "blocking_toolpath_id": block.blocking_toolpath_id,
                        "blocking_toolpath_index": block.blocking_toolpath_index,
                        "message": block.message,
                    }));
                }
                // Start from the core diagnostic when this toolpath has one,
                // so every channel the CLI publishes is on the wire. The lane
                // columns are written on top afterwards and win on the keys
                // they share, which keeps the pre-B-5 values of those keys
                // byte-identical.
                let mut row = core_rows
                    .get(&tc.id)
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                // SAFETY: row is a JSON object — either a serialised
                // `ToolpathDiagnostic` (a struct) or the empty map above.
                #[allow(clippy::indexing_slicing)]
                {
                    row["toolpath_index"] = serde_json::json!(index);
                    row["toolpath_id"] = serde_json::json!(tc.id);
                    row["name"] = serde_json::json!(tc.name);
                    row["operation_type"] = serde_json::json!(tc.operation.label());
                    row["tool_name"] = serde_json::json!(tool_name);
                    row["status"] = serde_json::json!(status.label());
                    row["error"] = serde_json::json!(status.error_text());
                    row["awaiting_prior_stock"] =
                        serde_json::json!(status.blocked_on().map(|b| serde_json::json!({
                            "blocking_toolpath_id": b.blocking_toolpath_id,
                            "blocking_toolpath_index": b.blocking_toolpath_index,
                            "message": b.message,
                        })));
                    row["stale"] = serde_json::json!(rt.stale_since.is_some());
                    if let Some(ref result) = rt.result {
                        row["move_count"] = serde_json::json!(result.stats.move_count);
                        row["cutting_distance_mm"] =
                            serde_json::json!(result.stats.cutting_distance);
                        row["rapid_distance_mm"] = serde_json::json!(result.stats.rapid_distance);
                    }
                }
                per_toolpath.push(row);
            }
        }

        // LH-1: publish BOTH air-cut denominators under names that say which
        // is which. The legacy `air_cut_percentage` key keeps its
        // total-runtime value (the verdict rule below and every
        // `air_cut_high_threshold_pct` band are tuned against it); the
        // cutting-time reading - what `narrate_toolpath` prints for the same
        // seconds - ships beside it instead of contradicting it under one
        // name. See `MEASUREMENT_DOMAINS.md` LH-1.
        let (total_runtime_s, air_cut_pct, air_cut_pct_of_cutting, avg_engagement) =
            if let Some(ref sim_results) = self.state.simulation.results
                && let Some(ref ct) = sim_results.cut_trace
            {
                use rs_cam_core::simulation_cut::AirCutRatios;
                let s = &ct.summary;
                (
                    s.total_runtime_s,
                    s.air_cut_pct_of_total_runtime(),
                    s.air_cut_pct_of_cutting_time(),
                    s.average_engagement,
                )
            } else {
                (0.0, 0.0, 0.0, 0.0)
            };

        let rapid_collision_count = self.state.simulation.checks.rapid_collisions.len();

        // W5B-F4 (2026-08-21): 20 → 40. This bar and the GUI banner
        // (`ui/sim_diagnostics.rs`) are the same number on two surfaces; the
        // CLI shipped 40 on the same quantity, so the workspace carried two
        // project constants at once. 20 fired on every reference project the
        // repo has both before and after the swept-kernel flip, so it carried
        // no information. Interim constant; the recommended end state is to
        // derive this from the per-op offender list. Verdict flips: none.
        // `planning/perf_review_2026-08-19/DELTA_w5b_f4_aircut_DECISION.md` §5.6.
        let verdict = if rapid_collision_count > 0 {
            "WARNING: rapid collisions detected"
        } else if air_cut_pct > 40.0 {
            "WARNING: high air cutting (>40% of total runtime)"
        } else {
            "OK"
        };

        let mut resp = serde_json::json!({
            "total_runtime_s": total_runtime_s,
            "air_cut_percentage": air_cut_pct,
            "air_cut_pct_of_total_runtime": air_cut_pct,
            "air_cut_pct_of_cutting_time": air_cut_pct_of_cutting,
            "average_engagement": avg_engagement,
            // B-5: the holder-collision total, from the same evidence the
            // per-toolpath rows and the triage read. It was a literal `0`
            // here — a number that had never been measured, printed on the
            // surface an agent reads as a safety tally (X-VAC).
            "collision_count": core_diagnostics.collision_count,
            "rapid_collision_count": rapid_collision_count,
            "verdict": verdict,
            "per_toolpath": per_toolpath,
            "runtime_errors": runtime_errors,
            "awaiting_prior_stock": awaiting_prior_stock,
        });

        if let Some(ref sim_results) = self.state.simulation.results
            && let Some(ref ct) = sim_results.cut_trace
        {
            // SAFETY: resp is a known JSON object we just constructed
            #[allow(clippy::indexing_slicing)]
            {
                resp["semantic_summary_count"] = serde_json::json!(ct.semantic_summaries.len());
                resp["hotspot_count"] = serde_json::json!(ct.hotspots.len());
                resp["issue_count"] = serde_json::json!(ct.issues.len());
            }
        }

        // The page-one answer, from the SAME `ProjectSession::simulation_triage`
        // the GUI panel, the CLI report and narration read. An agent asking
        // "what should I act on?" reads `triage.safety` then `triage.actions`
        // and never has to know that `issue_count` above is a different
        // population from `air_cut_issue_count` — which is the confusion the
        // census documented and this field exists to end.
        // B-5: `evidence` and `core_diagnostics` were built once at the top
        // of this function; the triage reuses them instead of rebuilding the
        // project diagnostics a second time.
        let triage = self
            .state
            .session
            .simulation_triage_with_diagnostics(&evidence, &core_diagnostics);
        // SAFETY: resp is a known JSON object we just constructed.
        #[allow(clippy::indexing_slicing)]
        {
            // `Value::Null` is a constant, so the lazy form is the
            // `unnecessary_lazy_evaluations` lint.
            resp["triage"] = serde_json::to_value(&triage).unwrap_or(serde_json::Value::Null);
        }

        resp
    }

    #[cfg(feature = "mcp")]
    fn notify_mcp_collision_error(&mut self, error: &str) {
        use crate::mcp_bridge::McpResponse;
        use rs_cam_mcp::server::json_str;

        if let Some(ref mut pending) = self.pending_mcp
            && let Some(sender) = pending.collision.take()
        {
            let _ = sender.send(McpResponse {
                result: Ok(json_str(serde_json::json!({"error": error}))),
            });
        }
    }
}

/// Build the core simulation the session stores from the lane's answer.
///
/// N12 item 10. The two types carry ONE simulation. The viz one adds the
/// playback stream and the trace artifact path, which are the viewport's
/// own and which core carries no slot for; core's own
/// `ProjectSession::run_simulation` stores every other field.
///
/// What the copy SHARES, each behind an `Arc`: the per-toolpath
/// checkpoints and the prior stocks. What it COPIES: the display mesh,
/// the two deviation vectors, the boundary list and the rapid-collision
/// list. The mesh copy is the price of one simulation state, and it is
/// one mesh beside a per-toolpath checkpoint list the two already share.
///
/// The `cut_trace` slot is the one field this function does not fill. The
/// caller attaches the trace AFTER the feed-modulation post-pass runs,
/// because that pass rewrites the trace through `Arc::make_mut`. A second
/// `Arc` held at that moment makes `make_mut` copy the trace, and the two
/// surfaces then publish two different traces.
fn core_simulation_from_lane(
    simulation: &crate::compute::SimulationResult,
) -> rs_cam_core::compute::simulate::SimulationResult {
    rs_cam_core::compute::simulate::SimulationResult {
        mesh: simulation.mesh.clone(),
        total_moves: simulation.total_moves,
        deviations: simulation.deviations.clone(),
        column_deviations: simulation.column_deviations.clone(),
        boundaries: simulation
            .boundaries
            .iter()
            .map(|boundary| rs_cam_core::compute::simulate::SimBoundary {
                id: boundary.id,
                name: boundary.name.clone(),
                tool_name: boundary.tool_name.clone(),
                start_move: boundary.start_move,
                end_move: boundary.end_move,
                direction: boundary.direction,
            })
            .collect(),
        checkpoints: simulation.checkpoints.clone(),
        rapid_collisions: simulation.rapid_collisions.clone(),
        rapid_collision_move_indices: simulation.rapid_collision_move_indices.clone(),
        cut_trace: None,
        resolution_clamped: simulation.resolution_clamped,
        column_grid_cell_mm: simulation.column_grid_cell_mm,
        prior_stocks: simulation.prior_stocks.clone(),
    }
}
