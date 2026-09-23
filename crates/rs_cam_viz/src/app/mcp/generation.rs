//! The MCP generation surface: add and generate a toolpath, optimize it,
//! apply feeds, and plan or preview the multi-tool finishing ladder.
//!
//! Split out of `app/mcp.rs` (P4). Every item is a verbatim move; the
//! handlers stay reachable as `RsCamApp::mcp_*` because an inherent
//! `impl` compiles in any module of the crate.

use std::path::Path;

use rs_cam_mcp::server::{json_str, parse_operation_type};

use crate::app::RsCamApp;
use crate::mcp_bridge::{McpResponse, ProgressUpdate, mutation_error_json};
use crate::state::selection::Selection;

use super::simulation::build_per_depth_pass_summary;
use super::{MultitoolDials, parse_tier_strategies};

impl RsCamApp {
    /// v3.2 (2026-06-04): Combined-Suggest rationale for the toolpath
    /// at `index`. Returns a [`rs_cam_core::feeds::rationale::SuggestRationale`]
    /// payload as JSON describing every pass the orchestrator ran and
    /// why each parameter landed where it did (DPP back-off,
    /// chipload-target feed lift, runtime-stepover floor, etc.).
    ///
    /// Shares the GUI feeds modal's invocation via
    /// `ProjectSession::cutter_op_profile` (T10 dedup) so the agent and
    /// the operator see the same surface. Does *not* mutate the
    /// project — the agent decides whether to follow up with
    /// `set_toolpath_param`.
    pub(super) fn mcp_get_suggest_rationale(&self, index: usize) -> String {
        let state = self.controller.state();
        let Some(tc) = state.session.get_toolpath_config(index) else {
            return json_str(serde_json::json!({
                "error": format!("Toolpath index {index} not found")
            }));
        };
        let Some(profile) = state.session.cutter_op_profile(tc) else {
            return json_str(serde_json::json!({
                "error": format!("Tool {} for toolpath {} not found", tc.tool_id, tc.id)
            }));
        };
        match profile.feasibility {
            Ok(()) => {
                let rationale = rs_cam_core::feeds::rationale::SuggestRationale::from_warnings(
                    &profile.warnings,
                );
                json_str(serde_json::json!({
                    "toolpath_id": tc.id,
                    "toolpath_name": tc.name,
                    "rationale": serde_json::to_value(&rationale).unwrap_or_default(),
                }))
            }
            Err(e) => json_str(serde_json::json!({
                "toolpath_id": tc.id,
                "toolpath_name": tc.name,
                "error": format!("Suggest refused: {e}"),
            })),
        }
    }

    /// Strategy advisor (`STRATEGY_ADVISOR_2026-06-17`): plan each candidate
    /// clearing strategy for the Adaptive3d toolpath at `index` at its
    /// load-limited params and recommend the one with the minimum
    /// acceleration-aware wall-clock. Returns chosen strategy, the binding
    /// regime as the *why*, every candidate ranked by wall-clock, and the
    /// speed margin. Heavy (plans one toolpath per candidate) and runs
    /// synchronously, so the GUI is unresponsive while it computes. Does not
    /// mutate the project.
    /// Start the strategy advisor as a `Job`, and hold the caller's
    /// oneshot until the lane answers (WP14a).
    ///
    /// Step (i) runs here, on the frame loop: it resolves the generation
    /// inputs and captures every other session read, so a refusal still
    /// appears at submit time with core's own wording. Step (ii) — the
    /// candidate planning, simulation and modulation, which is tens of
    /// seconds — runs on the `Job` lane holding no session, so the GUI
    /// stays usable.
    pub(super) fn mcp_recommend_clearing_strategy(
        &mut self,
        index: usize,
        response_tx: tokio::sync::oneshot::Sender<McpResponse>,
    ) {
        // §22 ruling 3: a FRESH flag per submit. The lane maps "cancel this
        // job" onto it.
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let started = self.controller.state_mut().session.start(
            rs_cam_core::session::Job::RecommendClearingStrategy(
                rs_cam_core::session::RecommendClearingStrategyArgs { index },
            ),
            &cancel,
        );
        let handle = match started {
            Ok(handle) => handle,
            Err(e) => {
                let _ = response_tx.send(McpResponse {
                    result: Ok(json_str(serde_json::json!({
                        "error": format!("{e}"),
                    }))),
                });
                return;
            }
        };
        self.controller.submit_mcp_job(
            handle,
            cancel,
            crate::mcp_bridge::McpJobRender::StrategyRecommendation { index },
            response_tx,
        );
    }

    pub(super) fn mcp_get_tool_load_report(&self) -> String {
        let state = self.controller.state();
        // The cut trace is held in viz simulation state, not in
        // `session.simulation`. Pull it from there so chipload/power can be
        // evaluated against the active simulation run.
        let sim_trace = state
            .simulation
            .results
            .as_ref()
            .and_then(|r| r.cut_trace.as_deref());
        let report = rs_cam_core::gcode::project_load_report(&state.session, sim_trace);

        // Per-DepthPass MRR/feed/engagement histogram (S2.5). Keyed by
        // toolpath raw id, then a list of one entry per `SpanKind::DepthPass`
        // span, in span order (pass 0, pass 1, …). Lets agents distinguish
        // the high-DOC first pass from the steady-state passes for any
        // multi-pass operation without re-grouping the raw sample stream.
        let per_depth_pass = build_per_depth_pass_summary(state, sim_trace);

        // F5 — first-look summary so a single MCP read answers
        // "what's broken in this project?" without folding the
        // per-toolpath array.
        //
        // Roadmap F.11 — name resolver populates
        // `exceeds_breakdown[].toolpath_name` so agents don't need a
        // second `list_toolpaths()` round-trip to label each entry.
        let summary_value = serde_json::to_value(report.summary(|id| {
            state
                .session
                .toolpath_configs()
                .iter()
                .find(|tc| tc.id == id)
                .map(|tc| tc.name.clone())
        }))
        .unwrap_or(serde_json::Value::Null);
        let load_value = serde_json::to_value(&report).unwrap_or(serde_json::Value::Null);
        json_str(serde_json::json!({
            "summary": summary_value,
            "load_report": load_value,
            "per_depth_pass": per_depth_pass,
        }))
    }

    /// Run the optimizer on one toolpath as a `Job` (WP14b).
    ///
    /// Step (i) runs here, on the frame loop: it clones the session into
    /// the handle and takes the baseline cut trace off the session, so a
    /// refusal still appears at submit time with core's own wording.
    /// Step (ii) — the candidate search, which is one to two minutes —
    /// runs on the `Job` lane over that clone, so the GUI stays usable
    /// and nothing on screen is lent out.
    ///
    /// The CLIENT waits exactly as long as it did: the arm stores the
    /// oneshot and the drain answers it. No `timeout_s` is added, so the
    /// pinned wire snapshot does not move (§26 ruling 1).
    ///
    /// **The trace source changed.** It used to come from the VIEW's
    /// simulation slot. It now comes from the session, which the GUI
    /// drain adopts every simulation into. A session mutation clears that
    /// slot, so an `optimize_toolpath` issued after an edit refuses where
    /// it used to score against the older trace (§28 ruling 5).
    pub(super) fn mcp_optimize_toolpath(
        &mut self,
        index: usize,
        response_tx: tokio::sync::oneshot::Sender<McpResponse>,
    ) {
        // §22 ruling 3: a FRESH flag per submit. The lane maps "cancel
        // this job" onto it.
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let started = self.controller.state_mut().session.start(
            rs_cam_core::session::Job::OptimizeToolpath(
                rs_cam_core::session::OptimizeToolpathArgs { index },
            ),
            &cancel,
        );
        let handle = match started {
            Ok(handle) => handle,
            Err(e) => {
                let _ = response_tx.send(McpResponse {
                    result: Ok(json_str(serde_json::json!({
                        "error": format!("{e}"),
                    }))),
                });
                return;
            }
        };
        self.controller.submit_mcp_job(
            handle,
            cancel,
            crate::mcp_bridge::McpJobRender::OptimizeOutcome { index },
            response_tx,
        );
    }

    /// F3.1 — add a toolpath through the GUI's own path (PLAN.md §5).
    ///
    /// Calls `AppController::handle_add_toolpath`, the handler the Add
    /// menu item in `ui/toolpath_panel.rs` reaches through
    /// `AppEvent::AddToolpath`. That event's arm in
    /// `handle_internal_event` calls this handler and nothing else, so
    /// the two routes run one body; WP28 reads the handler directly
    /// because the arm returns `()` and the reply needs the command's
    /// `Effects`. None of that binding logic is reimplemented here: if
    /// the GUI path and a direct core add ever disagree, this tool shows
    /// the disagreement rather than hiding it.
    ///
    /// The handler returns `()` and reports a refusal ONLY by pushing a
    /// toast, so the refusal text is read back off the notification stack
    /// — the same stack `get_notifications` (F3.5) publishes, through the
    /// same `AppController::notifications` accessor.
    pub(super) fn mcp_add_toolpath_via_gui(
        &mut self,
        operation_type: &str,
        setup_index: Option<usize>,
    ) -> String {
        let op_type = match parse_operation_type(operation_type) {
            Ok(ot) => ot,
            Err(e) => {
                return self.mcp_mutation_error(format!("Error: {e}"), Some("operation_type"));
            }
        };

        // The GUI reads the target setup from the CURRENT SELECTION, so
        // honour `setup_index` the way a click does: select the setup,
        // then raise the event. Refuse an out-of-range index BEFORE
        // dispatching, so a typo never reaches the GUI path and never
        // produces a toast.
        if let Some(setup_index) = setup_index {
            let setups = self.controller.state().session.list_setups();
            let Some(setup) = setups.get(setup_index) else {
                let count = setups.len();
                return self.mcp_mutation_error(
                    format!("Error: setup index {setup_index} not found ({count} setups)"),
                    Some("setup_index"),
                );
            };
            let setup_id = crate::state::job::SetupId(setup.id);
            self.controller.state_mut().selection = Selection::Setup(setup_id);
        }

        let before = self.mcp_diagnostic_snapshot();
        let ids_before: std::collections::HashSet<rs_cam_core::ToolpathId> = self
            .controller
            .state()
            .session
            .toolpath_configs()
            .iter()
            .map(|tc| tc.id)
            .collect();
        let toasts_before = self.controller.notifications().len();

        // The Add menu's own handler, which `handle_internal_event`'s
        // `AppEvent::AddToolpath` arm calls and nothing else. It reports
        // the command's `Effects` so the reply can name the set the
        // setter dropped (WP28); the arm itself returns `()`.
        let effects = self.controller.handle_add_toolpath(op_type);

        // Whatever this call put on the stack. The GUI add path pushes at
        // most one, but read the tail rather than assuming a count.
        let pushed: Vec<serde_json::Value> = self
            .controller
            .notifications()
            .iter()
            .enumerate()
            .skip(toasts_before)
            .map(|(i, n)| Self::notification_json(i, n))
            .collect();

        let created = self
            .controller
            .state()
            .session
            .toolpath_configs()
            .iter()
            .enumerate()
            .find(|(_, tc)| !ids_before.contains(&tc.id))
            .map(|(index, tc)| {
                let session = &self.controller.state().session;
                let tool_name = session
                    .tools()
                    .iter()
                    .find(|t| t.id.0 == tc.tool_id)
                    .map(|t| t.name.clone());
                let model_name = session
                    .models()
                    .iter()
                    .find(|m| m.id == tc.model_id)
                    .map(|m| m.name.clone());
                let setup_index = session
                    .list_setups()
                    .iter()
                    .position(|s| s.toolpath_indices.contains(&index));
                serde_json::json!({
                    "index": index,
                    "id": tc.id.0,
                    "name": tc.name,
                    "operation": tc.operation.op_type().kind_str(),
                    "setup_index": setup_index,
                    "tool_id": tc.tool_id,
                    "tool_name": tool_name,
                    "model_id": tc.model_id,
                    "model_name": model_name,
                })
            });

        // The refusal text IS the toast. Nothing else carries it.
        let refusal = pushed
            .iter()
            .find(|n| n.get("severity").and_then(|s| s.as_str()) != Some("info"))
            .and_then(|n| n.get("message").and_then(|m| m.as_str()))
            .map(str::to_owned);

        match created {
            Some(created) => {
                // WP28: the set the `AddToolpath` command's setter
                // dropped. An append moves one revision, so this reads
                // the created index. `apply_quietly` already stamped it
                // on the view, and the operator route reads the same
                // answer. The old helper answered from a tag and named
                // EVERY toolpath.
                let stale: Vec<usize> = effects
                    .as_ref()
                    .map(|effects| effects.stale.iter().copied().collect())
                    .unwrap_or_default();
                let index = created.get("index").and_then(serde_json::Value::as_u64);
                self.mcp_mutation_result(
                    match index {
                        Some(i) => format!("Added toolpath at index {i} through the GUI add path."),
                        None => "Added toolpath through the GUI add path.".to_owned(),
                    },
                    serde_json::json!({
                        "created": created,
                        "refusal": serde_json::Value::Null,
                        "notifications": pushed,
                    }),
                    stale,
                    &before,
                )
            }
            None => {
                let summary = refusal.clone().unwrap_or_else(|| {
                    "The GUI add path created no toolpath and pushed no toast.".to_owned()
                });
                // Same refusal document every mutation uses, plus the two
                // fields that make this one diagnosable: nothing was
                // created, and here is exactly what the operator saw.
                let mut doc: serde_json::Value =
                    serde_json::from_str(&mutation_error_json(&summary, None))
                        .unwrap_or_else(|_| serde_json::json!({ "ok": false, "summary": summary }));
                if let Some(obj) = doc.as_object_mut() {
                    obj.insert("created".to_owned(), serde_json::Value::Null);
                    obj.insert(
                        "refusal".to_owned(),
                        refusal.map_or(serde_json::Value::Null, serde_json::Value::String),
                    );
                    obj.insert("notifications".to_owned(), serde_json::Value::Array(pushed));
                }
                json_str(doc)
            }
        }
    }

    /// Phase O — the machine-readable planner trigger. Validates, resolves the
    /// model, then hands off to the controller reconciler Phase U's dialog
    /// will share.
    pub(super) fn mcp_plan_multitool_finishing(
        &mut self,
        spec: &rs_cam_mcp::server::PlanMultitoolFinishingParam,
    ) -> String {
        let plan_spec = match self.multitool_plan_spec(
            spec.setup_index,
            spec.model_id,
            &spec.tool_ids,
            &MultitoolDials {
                cell_mm: spec.cell_mm,
                tolerance_mm: spec.tolerance_mm,
                margin_mm: spec.margin_mm,
                cusp_height_mm: spec.cusp_height_mm,
                coarseness: spec.coarseness,
                overlap_mm: spec.overlap_mm,
                max_regions_per_tier: spec.max_regions_per_tier,
                coarse_skips_fine_islands: spec.coarse_skips_fine_islands,
                monotone_cell_decomposition: spec.monotone_cell_decomposition,
                tier_strategies: spec.tier_strategies.clone(),
            },
        ) {
            Ok(plan_spec) => plan_spec,
            Err(message) => return Self::mcp_plan_error(&message),
        };

        let outcome = match self.controller.apply_multitool_plan(&plan_spec) {
            Ok(outcome) => outcome,
            Err(e) => return Self::mcp_plan_error(&e),
        };

        let session = &self.controller.state().session;
        let emitted: Vec<serde_json::Value> = outcome
            .toolpath_ids
            .iter()
            .filter_map(|id| {
                session
                    .find_toolpath_config_by_id(*id)
                    .map(|(index, tc)| (index, tc, *id))
            })
            .map(|(index, tc, id)| {
                serde_json::json!({
                    "index": index,
                    "id": id.0,
                    "name": tc.name,
                    "tier": tc.planner_origin.as_ref().map(|o| o.tier),
                    "tool_id": tc.tool_id,
                })
            })
            .collect();
        let replaced: Vec<usize> = outcome.replaced.iter().map(|id| id.0).collect();

        // Three-valued, and the wire says which: `null` = not measured (no
        // cached tier map for this ladder and these dials — planning builds
        // none), `[]` = measured and every band is inside the bound.
        let band_advisories: Option<Vec<String>> = outcome
            .band_advisories
            .as_ref()
            .map(|list| list.iter().map(ToString::to_string).collect());

        json_str(serde_json::json!({
            "ok": true,
            "plan_id": outcome.plan_id,
            "emitted": emitted,
            "replaced": replaced,
            "band_advisories": band_advisories,
            "note": "Nothing is generated yet — each tier resolves its islands lazily. Call \
                     generate_all with fixpoint on and a simulation_resolution_mm to run the \
                     whole coarse-to-fine rest-stock chain in one call. `band_advisories` \
                     is null when no tier map was cached for these dials — call \
                     preview_tier_map first to measure the overlap band; an empty list \
                     means measured and healthy.",
        }))
    }

    fn mcp_plan_error(message: &str) -> String {
        json_str(serde_json::json!({
            "ok": false,
            "error": format!("plan_multitool_finishing: {message}"),
        }))
    }

    fn mcp_preview_error(message: &str) -> String {
        json_str(serde_json::json!({
            "ok": false,
            "modified": false,
            "error": format!("preview_tier_map: {message}"),
        }))
    }

    /// The validation + dial resolution `plan_multitool_finishing` and
    /// `preview_tier_map` share.
    ///
    /// ONE site, because the two tools exist to describe the SAME job: an
    /// agent that previews at one set of defaults and plans at another is
    /// looking at a picture of a different job, and nothing on either surface
    /// would say so. The error strings carry no tool name — each caller
    /// prefixes its own.
    fn multitool_plan_spec(
        &self,
        setup_index: usize,
        model_id: Option<usize>,
        tool_ids: &[usize],
        dials: &MultitoolDials,
    ) -> Result<rs_cam_core::session::MultitoolPlanSpec, String> {
        let session = &self.controller.state().session;

        if setup_index >= session.list_setups().len() {
            return Err(format!(
                "setup_index {setup_index} does not exist — the project has {} setup(s).",
                session.list_setups().len()
            ));
        }
        if tool_ids.is_empty() {
            return Err(
                "tool_ids is empty — the ladder needs at least one tool, coarsest first."
                    .to_owned(),
            );
        }
        let known: Vec<usize> = session.tools().iter().map(|t| t.id.0).collect();
        if let Some(missing) = tool_ids.iter().copied().find(|id| !known.contains(id)) {
            return Err(format!(
                "tool_id {missing} does not match any tool in this project (have {known:?}). \
                 These are library tool ids, not indices."
            ));
        }

        // The setup names no model of its own, so "the setup's model" is the
        // project's model when there is exactly one. Two is ambiguous and is
        // refused rather than resolved by position.
        let model_id = match model_id {
            Some(id) => {
                if !session.models().iter().any(|m| m.id == id) {
                    let available: Vec<usize> = session.models().iter().map(|m| m.id).collect();
                    return Err(format!(
                        "model_id {id} does not exist — the project has {available:?}."
                    ));
                }
                id
            }
            None => match session.models() {
                [] => {
                    return Err(
                        "no model imported — the tier map is measured against one.".to_owned()
                    );
                }
                [only] => only.id,
                models => {
                    let available: Vec<usize> = models.iter().map(|m| m.id).collect();
                    return Err(format!(
                        "this project has {} models ({available:?}), so `model_id` is required \
                         — it is not guessed.",
                        models.len()
                    ));
                }
            },
        };

        // The remaining fields (raw close radius / min island area, rim
        // erosion) stay at their core derivation; `coarseness` is the one
        // knob the operator turns and it scales both.
        let island_defaults = rs_cam_core::maps::tier_islands::TierIslandParams::default();
        let islands = rs_cam_core::maps::tier_islands::TierIslandParams {
            coarseness: dials.coarseness.unwrap_or(island_defaults.coarseness),
            overlap_mm: dials.overlap_mm.unwrap_or(island_defaults.overlap_mm),
            max_regions_per_tier: dials
                .max_regions_per_tier
                .unwrap_or(island_defaults.max_regions_per_tier),
            ..island_defaults
        };

        // Unset dials fall back to the CORE's own defaults, never to numbers
        // copied here: a second copy is a second thing to drift.
        let plan_defaults = rs_cam_core::session::MultitoolPlanSpec::default();
        Ok(rs_cam_core::session::MultitoolPlanSpec {
            setup_index,
            model_id,
            tool_ids: tool_ids.to_vec(),
            cell_mm: dials.cell_mm.unwrap_or(plan_defaults.cell_mm),
            tolerance_mm: dials.tolerance_mm.unwrap_or(plan_defaults.tolerance_mm),
            margin_mm: dials.margin_mm.unwrap_or(plan_defaults.margin_mm),
            cusp_height_mm: dials.cusp_height_mm.unwrap_or(plan_defaults.cusp_height_mm),
            // B1 decision, not a dial: the raw drop-cutter residual is a
            // tool-CENTRE difference biased by R*(sec theta - 1), so a slope
            // both tools machine perfectly still reads as fine-tier
            // territory. Measured on wanaka200: raw claimed 71.6% of the
            // board for the fine tiers, compensated 22.0% — within 3% of the
            // stock-referenced truth. Stated here rather than inherited so a
            // change to the core default cannot silently move this surface.
            treatment: rs_cam_core::maps::tier_map::ResidualTreatment::SlopeCompensated,
            islands,
            coarse_skips_fine_islands: dials
                .coarse_skips_fine_islands
                .unwrap_or(plan_defaults.coarse_skips_fine_islands),
            // C2. Same `None` = the caller did not say = the CORE default
            // (off) rule as every dial above it.
            monotone_cell_decomposition: dials
                .monotone_cell_decomposition
                .unwrap_or(plan_defaults.monotone_cell_decomposition),
            tier_strategies: parse_tier_strategies(&dials.tier_strategies)?,
        })
    }

    /// Phase U item 2 — the agent-visible preview twin of the planner.
    ///
    /// A pure READ: it builds the tier map and its islands and reports them.
    /// Nothing is emitted, no parameter moves, no result is invalidated — the
    /// reply says `modified: false` and that is a statement about this
    /// function, not a hope. It takes `&self` so that stays true by type.
    /// Start the tier-map preview as a `Job`, and hold the caller's
    /// oneshot until the lane answers (WP14a).
    ///
    /// Step (i) runs here, on the frame loop: it resolves the dials, checks
    /// the SVG destination and captures the ladder and the geometry, so
    /// every refusal still appears at submit time. Step (ii) — the
    /// full-grid residual walk, tens of seconds per ladder tool — runs on
    /// the `Job` lane holding no session.
    ///
    /// The SVG write is NOT part of the job. It is a viz-side step the
    /// drain takes after the answer arrives.
    pub(super) fn mcp_preview_tier_map(
        &mut self,
        spec: &rs_cam_mcp::server::PreviewTierMapParam,
        response_tx: tokio::sync::oneshot::Sender<McpResponse>,
    ) {
        let plan_spec = match self.multitool_plan_spec(
            spec.setup_index,
            spec.model_id,
            &spec.tool_ids,
            &MultitoolDials {
                cell_mm: spec.cell_mm,
                tolerance_mm: spec.tolerance_mm,
                margin_mm: spec.margin_mm,
                cusp_height_mm: spec.cusp_height_mm,
                coarseness: spec.coarseness,
                overlap_mm: spec.overlap_mm,
                max_regions_per_tier: spec.max_regions_per_tier,
                // The skip dial moves tier 0's BOUNDARY, not the islands,
                // so the island preview does not change with it — but it
                // is threaded through (not swallowed) so the previewed
                // spec IS the planned spec, per the param's parity
                // contract and the dial-parity sentry.
                coarse_skips_fine_islands: spec.coarse_skips_fine_islands,
                // C2 is likewise an EMISSION dial — it changes what a
                // tier's shallow band emits, not the island map — and is
                // threaded through for the same dial-parity contract.
                monotone_cell_decomposition: spec.monotone_cell_decomposition,
                // Strategies do not move the island map; threaded (not
                // swallowed) for the dial-parity contract, same as the two
                // emission dials above.
                tier_strategies: spec.tier_strategies.clone(),
            },
        ) {
            Ok(plan_spec) => plan_spec,
            Err(message) => {
                Self::send_preview_error(response_tx, &message);
                return;
            }
        };

        // Checked BEFORE the walk: a residual map is tens of seconds of work,
        // and finding out afterwards that the destination does not exist
        // spends all of it to produce an error.
        if let Some(path) = spec.svg_path.as_deref()
            && let Err(message) = Self::validate_svg_out_path(path)
        {
            Self::send_preview_error(response_tx, &message);
            return;
        }

        let setup_index = plan_spec.setup_index;
        let model_id = plan_spec.model_id;
        // §22 ruling 3: a FRESH flag per submit. The walk polls it at grid-row
        // granularity, so Cancel is honest for the whole of it.
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let started = self.controller.state_mut().session.start(
            rs_cam_core::session::Job::PreviewTierMap(rs_cam_core::session::PreviewTierMapArgs {
                spec: Box::new(plan_spec),
            }),
            &cancel,
        );
        let handle = match started {
            Ok(handle) => handle,
            Err(e) => {
                Self::send_preview_error(response_tx, &e.to_string());
                return;
            }
        };
        self.controller.submit_mcp_job(
            handle,
            cancel,
            crate::mcp_bridge::McpJobRender::TierMapPreview {
                setup_index,
                model_id,
                svg_path: spec.svg_path.clone(),
            },
            response_tx,
        );
    }

    /// Answer a `preview_tier_map` refusal on the caller's own oneshot.
    fn send_preview_error(response_tx: tokio::sync::oneshot::Sender<McpResponse>, message: &str) {
        let _ = response_tx.send(McpResponse {
            result: Ok(Self::mcp_preview_error(message)),
        });
    }

    /// Refuse an SVG destination rather than creating a directory tree the
    /// operator did not ask for, or writing a `.svg` that is not one.
    fn validate_svg_out_path(path: &str) -> Result<(), String> {
        if path.is_empty() {
            return Err("svg_path is empty.".to_owned());
        }
        if !path.ends_with(".svg") {
            return Err(format!(
                "svg_path must end in `.svg` — got `{path}`. This writes an SVG, not a PNG or \
                 an HTML dump."
            ));
        }
        // A bare filename resolves against the GUI process's working
        // directory, which is rarely where the caller means.
        let Some(dir) = Path::new(path)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
        else {
            return Err(format!(
                "svg_path `{path}` names no directory. Pass an absolute path — a bare \
                 filename lands in the GUI process's working directory."
            ));
        };
        if !dir.is_dir() {
            return Err(format!(
                "svg_path's directory `{}` does not exist. It is not created — pass a \
                 directory that is already there.",
                dir.display()
            ));
        }
        Ok(())
    }

    /// `apply_feeds` — the agent's entry to the one application funnel
    /// (Checkpoint I-5, 2026-08-12).
    ///
    /// A-3's census §2f recorded that the MCP surface had **no** apply tool at
    /// all: `get_suggest_rationale` is read-only and says so,
    /// `set_spindle_strategy` states it mutates nothing, and the only agent
    /// write was `set_toolpath_param` — a raw operator write that is neither
    /// feeds-validated nor invariant-funnelled. An agent therefore held the
    /// old modal's contract with none of the modal's preview. This gives it
    /// the properties panel's contract instead, and unlike a GUI button it
    /// must name its scope.
    ///
    /// A refused pairing returns a mutation **error** carrying the engine's
    /// own refusal text. It must never read as a successful no-op: an agent
    /// that cannot distinguish "applied" from "declined" will re-simulate and
    /// conclude the recommendation did nothing.
    pub(super) fn mcp_apply_feeds(&mut self, index: usize, scope: &str) -> String {
        use rs_cam_core::feeds::suggest::ApplyScope;
        let before = self.mcp_diagnostic_snapshot();
        let parsed = match scope {
            "speeds" | "Speeds" => ApplyScope::Speeds,
            "cut_geometry" | "CutGeometry" | "cut" => ApplyScope::CutGeometry,
            "both" | "Both" => ApplyScope::Both,
            other => {
                return self.mcp_mutation_error(
                    format!(
                        "Error: unknown scope '{other}'. Expected 'speeds' (feed/plunge/RPM, \
                         does not change the cut), 'cut_geometry' (stepover/DOC, CHANGES THE \
                         CUT) or 'both'."
                    ),
                    Some("scope"),
                );
            }
        };
        let Some(toolpath_id) = self
            .controller
            .state()
            .session
            .toolpath_configs()
            .get(index)
            .map(|tc| tc.id)
        else {
            return self.mcp_mutation_error(
                format!("Error: toolpath index {index} not found"),
                Some("index"),
            );
        };
        let effects = match self
            .controller
            .apply_feeds_recommendation(toolpath_id, parsed)
        {
            Ok(effects) => effects,
            Err(why) => {
                return self.mcp_mutation_error(
                    format!("Error: nothing applied to toolpath {index} — Suggest refused: {why}"),
                    Some("index"),
                );
            }
        };
        // WP28: the funnel runs `ReplaceToolpathConfig` and stamps its
        // own `Effects::stale`, so the reply names the edited toolpath
        // AND the downstream results the stock chain dropped. The old
        // helper named the edited index alone (N15).
        let stale: Vec<usize> = effects.stale.iter().copied().collect();
        let applied = self
            .controller
            .state()
            .session
            .toolpath_configs()
            .get(index)
            .map(|tc| {
                serde_json::json!({
                    "scope": scope,
                    "changes_the_cut": !matches!(parsed, ApplyScope::Speeds),
                    "feed_rate": tc.operation.feed_rate(),
                    "plunge_rate": tc.operation.plunge_rate(),
                    "spindle_rpm": tc.operation.spindle_rpm(),
                    "stepover": tc.operation.stepover(),
                    "depth_per_pass": tc.operation.depth_per_pass(),
                })
            })
            .unwrap_or(serde_json::Value::Null);
        let cut_note = if matches!(parsed, ApplyScope::Speeds) {
            "Cut geometry (DOC/WOC) unchanged."
        } else {
            "CHANGED THE CUT (DOC/WOC) — re-simulate before trusting any gate verdict."
        };
        self.mcp_mutation_result(
            format!(
                "Applied Feeds recommendation to toolpath {index} with scope '{scope}'. \
                 {cut_note} Regenerate to apply."
            ),
            applied,
            stale,
            &before,
        )
    }

    pub(super) fn mcp_generate_toolpath(
        &mut self,
        index: usize,
        response_tx: tokio::sync::oneshot::Sender<McpResponse>,
    ) {
        // Find the toolpath ID from the index
        let session = &self.controller.state().session;
        let Some(tc) = session.toolpath_configs().get(index) else {
            let _ = response_tx.send(McpResponse {
                result: Ok(json_str(
                    serde_json::json!({"error": format!("Toolpath index {index} not found")}),
                )),
            });
            return;
        };
        let tp_id = tc.id;

        // MCP diagnostics depend on generation debug + semantic traces; enable
        // capture before queuing compute so get_generation_debug_trace and
        // narrate_toolpath have structured planner data.
        //
        // WP11b (§16 ruling 7): through the COMMAND surface, not
        // `toolpath_configs_mut()`. The write reached past every rule the
        // surface enforces, and the submit step reads the flag off the
        // config, so the two have to be the same door. The row moves no
        // revision and drops no result, which is why it can run immediately
        // before the generate.
        //
        // WP19 `let _ =`: `Effects::stale` is therefore empty and
        // `simulation_cleared` is false. There is nothing to mirror.
        let _ = self.controller.state_mut().session.apply(
            rs_cam_core::session::Command::SetToolpathDebugOptions(
                rs_cam_core::session::SetToolpathDebugOptionsArgs {
                    index,
                    debug_options: rs_cam_core::trace::debug_trace::ToolpathDebugOptions {
                        enabled: true,
                    },
                },
            ),
        );

        // Checked before the waiter is stored: a plan already running would
        // refuse this one, and nothing would ever resolve the oneshot. That
        // is the failure that left a `generate_toolpath` unresolved for
        // about nine hours.
        if self.controller.plan_is_busy() {
            let _ = response_tx.send(McpResponse {
                result: Ok(json_str(serde_json::json!({
                    "ok": false,
                    "error": "a generation plan is already running, or one is \
                              waiting on the operator to confirm a simulation \
                              resolution. Poll `generation_status` for its step, \
                              or call `cancel_generation` and try again.",
                }))),
            });
            return;
        }

        // Store the oneshot sender BEFORE the plan runs: a submit-time
        // refusal reaches a terminal state inside the call below, and the
        // waiter has to already be there to be resolved.
        if let Some(ref mut pending) = self.controller.pending_mcp {
            pending.toolpath.insert(tp_id, response_tx);
        } else {
            // If pending_mcp is None, respond immediately with error
            let _ = response_tx.send(McpResponse {
                result: Err("MCP compute tracking not initialized".to_owned()),
            });
            return;
        }

        // R6: by DIRECT CALL, not through `AppEvent`. `self.events` is
        // drained only by `handle_events` inside `draw_frame`, so an event
        // here would wait for a painted frame the MCP path cannot promise.
        // The waiter still resolves off `notify_mcp_toolpath_complete`, keyed
        // by id, so it reads this operation's own outcome and not the plan's.
        self.controller.handle_generate_toolpath(tp_id);
    }

    pub(super) fn mcp_generate_all(
        &mut self,
        fixpoint: Option<bool>,
        simulation_resolution_mm: Option<f64>,
        response_tx: tokio::sync::oneshot::Sender<McpResponse>,
        progress_tx: Option<tokio::sync::mpsc::Sender<ProgressUpdate>>,
    ) {
        // A/M11: the whole ladder lives on the controller, which owns the
        // compute lane, the simulation state and `pending_mcp` — the three
        // things a fixpoint loop has to coordinate. This is a thin adapter.
        self.controller.mcp_start_generate_all(
            fixpoint.unwrap_or(true),
            simulation_resolution_mm,
            response_tx,
            progress_tx,
        );
    }
}
