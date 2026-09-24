//! The MCP diagnostics surface: what the GUI knows went wrong, what a
//! toolpath narrates, and the span reads behind `inspect_spans`.
//!
//! Split out of `app/mcp.rs` (P4). Every item is a verbatim move; the
//! handlers stay reachable as `RsCamApp::mcp_*` because an inherent
//! `impl` compiles in any module of the crate.

use rs_cam_core::compute::config::ComputeStatus;

use rs_cam_mcp::response::{
    CappedArray, DEFAULT_MAX_DETAIL_SPANS, DEFAULT_MAX_TOP_LEVEL_SPANS, MAX_RESPONSE_BYTES,
    ResponseBudget, cap_json_values,
};
use rs_cam_mcp::server::{json_str, text};

use crate::app::RsCamApp;

impl RsCamApp {
    pub(super) fn mcp_get_diagnostics(&self) -> String {
        json_str(self.controller.build_mcp_diagnostics())
    }

    /// The tool a narration describes: the toolpath's OWN tool, or nothing.
    ///
    /// **B7 divergence 3 — a defect, fixed 2026-08-06.** The GUI narration
    /// used to append `.or_else(|| tools().first())` to this lookup, so when
    /// `tool_id` did not resolve it narrated with **another tool's
    /// geometry**: the diameter behind the large-arc threshold, the flute
    /// count behind every chipload sentence, and the cutter handed to
    /// `narrate_toolpath_with_context` itself. Nothing in the emitted text
    /// said so. Core's sibling narration has always errored instead
    /// (`session/compute.rs`, `SessionError::ToolNotFound`).
    ///
    /// A tool id that does not resolve is a broken project, not a routine
    /// state, so the honest answer is a refusal naming the id — which is
    /// what the caller emits. Exhibit:
    /// `narration_tool_lookup_refuses_where_the_parent_took_another_tool`.
    pub(super) fn narration_tool_for(
        tools: &[rs_cam_core::compute::tool_config::ToolConfig],
        tool_id: usize,
    ) -> Option<&rs_cam_core::compute::tool_config::ToolConfig> {
        tools.iter().find(|tool| tool.id.0 == tool_id)
    }

    pub(super) fn mcp_narrate_toolpath(&self, index: usize) -> String {
        let state = self.controller.state();
        let Some(tc) = state.session.get_toolpath_config(index) else {
            return format!("Error: Toolpath index {index} not found");
        };
        let Some(rt) = state.gui.toolpath_rt.get(&tc.id) else {
            return format!("Error: Toolpath {index} not generated. Run generate_toolpath first.");
        };
        let Some(result) = rt.result.as_ref() else {
            return format!("Error: Toolpath {index} not generated. Run generate_toolpath first.");
        };
        let Some(tool_config) = Self::narration_tool_for(state.session.tools(), tc.tool_id) else {
            return format!(
                "Error: toolpath {index} references tool id {} but no such tool is configured. Narration refuses rather than describing this toolpath with another tool's geometry.",
                tc.tool_id,
            );
        };

        let tool = rs_cam_core::compute::build_cutter(tool_config);
        let cut_trace = state
            .simulation
            .results
            .as_ref()
            .and_then(|sim| sim.cut_trace.as_deref());
        // B7 divergence 4: prefer the traces carried by the RESULT being
        // narrated, and only then the runtime's. The old order preferred
        // `rt.*`, so a narration could describe `result`'s move list using a
        // trace produced by a later generation. Core has no `rt` overlay and
        // has always read `result.*`; this makes the GUI agree, while still
        // falling back to `rt.*` for the paths that only populate there.
        let semantic_trace = result
            .semantic_trace
            .as_deref()
            .or(rt.semantic_trace.as_deref());
        let debug_trace = result.debug_trace.as_deref().or(rt.debug_trace.as_deref());
        // Checkpoint D Q2: narration reads the same measurability report the
        // gates and the triage do, so the MCP narration cannot publish an
        // air-cut percentage the gates have already declined to act on.
        //
        // B7 divergence 2: the cell size is the one the TRACE was measured
        // at (`SimulationResult::column_grid_cell_mm`), not the resolution
        // dial's current value — those are two different quantities, and the
        // measurability floors are cell-size dependent, so reading the dial
        // could return a different `NotMeasurable` verdict from core's on
        // identical evidence. `state.simulation.resolution` is what the next
        // simulation WILL use; it is not a property of this trace.
        let measurability = cut_trace.map(|trace| {
            rs_cam_core::stock::sim_measurability::MeasurabilityReport::from_trace(
                trace,
                state
                    .simulation
                    .results
                    .as_ref()
                    .map(|sim| sim.column_grid_cell_mm),
            )
        });
        let mut context = rs_cam_core::trace::narrate::ToolpathNarrationContext {
            measurability: measurability.as_ref(),
            toolpath_id: Some(tc.id),
            toolpath_name: Some(tc.name.as_str()),
            operation_label: Some(tc.operation.label()),
            operation_kind: Some(tc.operation.op_type()),
            // The deepest commanded bite (a step-ladder coarse step).
            depth_per_pass_mm: tc.operation.deepest_axial_step(),
            stepover_mm: tc.operation.stepover(),
            tool_diameter_mm: Some(tool_config.diameter),
            feed_rate_mm_min: Some(tc.operation.feed_rate()),
            spindle_rpm: Some(
                tc.operation
                    .spindle_rpm()
                    .unwrap_or(state.session.post_config().spindle_speed),
            ),
            flute_count: Some(tool_config.flute_count),
            // B7 divergence 1: the shared expression. Core used op-type
            // alone (blind to a generator that emits Drilling moves without
            // declaring a drill op type); this side used op-type OR ANY
            // Drilling move (which called a v-carve with a drilled entry a
            // drill cycle and suppressed its air-cut anomaly). The shared
            // helper is neither — see its doc.
            is_drill_cycle: rs_cam_core::trace::narrate::is_drill_cycle_for_narration(
                tc.operation.op_type(),
                &result.annotated.toolpath.moves,
            ),
            material: Some(&state.session.stock_config().material),
            // Every ToolpathStats-derived channel is filled by
            // `absorb_stats` below — the SAME join core's narration uses, so
            // a new finding cannot reach one narration and miss the other
            // (B7). The GUI worker fills `result.stats` from the core
            // generation findings; carrying them here is what puts those
            // figures in front of an agent narrating a live GUI toolpath.
            ..Default::default()
        };
        context.absorb_stats(&result.stats);

        let mut narration = rs_cam_core::trace::narrate::narrate_toolpath_with_context(
            result.annotated.as_ref(),
            semantic_trace,
            cut_trace,
            debug_trace,
            &tool,
            &context,
        );
        // Phase 4 — the two-sided kinematic sentence.
        //
        // Built by the session's single producer, which is the SAME call
        // that fills `ToolpathLoadVerdict::kinematic_utilization` in
        // `gcode::project_load_report`. Going through the load report here
        // would evaluate every gate on every toolpath to publish one
        // sentence; narration is a 4 ms call and must stay one.
        //
        // B7 divergence 4, again: the sentence measures `result`, the SAME
        // move list `narrate_toolpath_with_context` just described, handed
        // to `kinematic_utilization_of`. The by-index form would re-fetch
        // `session.results[index]` and could report a later generation's
        // motion under this narration's heading. One result source.
        if let Some(util) =
            state
                .session
                .kinematic_utilization_of(index, &result.annotated.toolpath, cut_trace)
            && let Some(sentence) = Self::kinematics_narration_sentence(&util)
        {
            narration.push_str("\n\n");
            narration.push_str(&sentence);
        }
        narration
    }

    /// The `kinematics` sentence appended to a narration, or `None` when
    /// nothing was measurable.
    ///
    /// Both sides are stated on equal footing: headroom (feed-bound time a
    /// feed rise converts) and over-command (machine-bound time, plus any
    /// plunge-class descent that outruns the operation's own plunge rate).
    /// Every reading is guarded on its own `Option` — an unmeasured value is
    /// omitted rather than printed as a zero, because a zero utilization and
    /// an unmeasured utilization are opposite facts.
    fn kinematics_narration_sentence(
        util: &rs_cam_core::machine::kinematic_utilization::ToolpathKinematicUtilization,
    ) -> Option<String> {
        let mut utilization: Option<f64> = None;
        if util.is_measured() {
            utilization = util.utilization;
        }
        let mut clauses: Vec<String> = Vec::new();
        if let Some(u) = utilization {
            clauses.push(format!("runs at {:.0}% of commanded feed", u * 100.0));
        }
        if let Some(b) = util.bindings.as_ref() {
            let mut feed = format!("{:.0}% feed-bound", b.feed_bound * 100.0);
            // The PRECOMPUTED field, never `headroom_estimate`: that method
            // re-solves every move, and this is an MCP handler path.
            if let Some(headroom) = util.headroom_at_1_30 {
                feed.push_str(&format!(
                    " (+{:.0}% at a x1.30 feed rise)",
                    headroom * 100.0
                ));
            }
            clauses.push(feed);
            clauses.push(format!(
                "{:.0}% machine-bound and will not move",
                b.machine_bound * 100.0
            ));
        }
        if util.plunge_is_measured()
            && let Some(ratio) = util.plunge.peak_ratio
        {
            clauses.push(format!(
                "plunge-class peak {:.1}x this op's own plunge rate ({} of {} \
                 vertical-dominant moves over 1x)",
                ratio, util.plunge.over_1x, util.plunge.population
            ));
        }
        if clauses.is_empty() {
            return None;
        }
        // Phase 3 — say WHICH feeds were read. The modulator runs after a
        // simulation, so before one this sentence describes the plan, not the
        // motion the post-processor emits. This handler narrates the WORKER's
        // move list, which the modulation post-pass never rewrites, so the
        // qualifier here is load-bearing (`feedback_measure_emitted_motion`).
        //
        // The Planned wording is THIS SURFACE's, not the shared
        // `FeedsProvenance::qualifier()`. The shared text ends "run a
        // simulation for emitted", which is true where it is read from
        // `session.results` (the CLI and the GUI) and FALSE here: this
        // handler narrates `state.gui.toolpath_rt`, the worker's
        // pre-modulation IR, which a simulation never rewrites. An operator
        // who follows that instruction runs a simulation and reads the same
        // number again. Narrating the emitted result is the open follow-up
        // (G-MODEXPORT class); until it lands, this surface states what it
        // reads and names the surface that does carry the emitted figure.
        let provenance = match util.feeds_provenance {
            rs_cam_core::machine::kinematic_utilization::FeedsProvenance::Planned => {
                "planned — this surface narrates the pre-modulation plan; the \
                 emitted reading is in get_tool_load_report after a simulation"
            }
            rs_cam_core::machine::kinematic_utilization::FeedsProvenance::Emitted => {
                util.feeds_provenance.qualifier()
            }
        };
        Some(format!(
            "kinematics: {} ({provenance}).",
            clauses.join("; ")
        ))
    }

    pub(super) fn mcp_get_generation_debug_trace(
        &self,
        index: usize,
        span_kind: Option<&str>,
        exit_reason: Option<&str>,
        max_yield_ratio: Option<f64>,
        max_spans: Option<usize>,
    ) -> String {
        let state = self.controller.state();
        let Some(tc) = state.session.get_toolpath_config(index) else {
            return json_str(
                serde_json::json!({"error": format!("Toolpath index {index} not found")}),
            );
        };
        let Some(rt) = state.gui.toolpath_rt.get(&tc.id) else {
            return json_str(serde_json::json!({
                "error": format!("Toolpath {index} not generated. Run generate_toolpath first.")
            }));
        };
        let Some(trace) = rt.debug_trace.as_ref() else {
            return json_str(serde_json::json!({
                "error": format!("Toolpath {index} has no debug trace — the operation generator didn't capture one.")
            }));
        };

        // span_kind accepts EITHER a generation-debug string (e.g.
        // "adaptive_pass", "z_level_clear", "preflight") OR a structural
        // SpanKind synonym in snake_case (e.g. "depth_pass", "entry"). The
        // latter expands to the set of debug-trace kinds that participate in
        // that structural span. This unifies the agent vocabulary across
        // get_cut_trace + inspect_spans + get_generation_debug_trace.
        let kind_filter: Box<dyn Fn(&str) -> bool> = match span_kind {
            None => Box::new(|_| true),
            Some(needle) => {
                let synonyms = expand_span_kind_synonyms(needle);
                Box::new(move |k: &str| synonyms.iter().any(|s| s == k))
            }
        };
        // CLI-04: the default is named in `rs_cam_mcp::response`. `0`
        // still means uncapped on this tool's wire.
        let limit = max_spans.unwrap_or(rs_cam_mcp::response::DEFAULT_MAX_DEBUG_TRACE_SPANS);
        let filtered: Vec<_> = trace
            .spans
            .iter()
            .filter(|s| kind_filter(s.kind.as_str()))
            .filter(|s| {
                exit_reason.is_none_or(|needle| {
                    s.exit_reason.as_deref().is_some_and(|r| r.contains(needle))
                })
            })
            .filter(|s| {
                max_yield_ratio
                    .is_none_or(|max_y| s.counters.get("yield_ratio").is_some_and(|&y| y <= max_y))
            })
            .collect();
        let total_matching = filtered.len();
        let visible_spans: Vec<_> = if limit == 0 {
            filtered.clone()
        } else {
            filtered.iter().copied().take(limit).collect()
        };
        // Each returned span is enriched with `span_kind_hint`: the
        // structural `SpanKind` (snake_case) that this generation-time span
        // contributes to, or null when there's no clean mapping (e.g.
        // op-internal "preflight", "widen_band").
        let visible: Vec<serde_json::Value> = visible_spans
            .iter()
            .map(|span| {
                let mut value =
                    serde_json::to_value(span).unwrap_or_else(|_| serde_json::json!({}));
                if let serde_json::Value::Object(map) = &mut value {
                    map.insert(
                        "span_kind_hint".into(),
                        serde_json::json!(map_debug_kind_to_span_kind(&span.kind)),
                    );
                }
                value
            })
            .collect();

        let pass_spans: Vec<_> = trace
            .spans
            .iter()
            .filter(|s| s.kind == "adaptive_pass")
            .collect();
        let mut passes_by_exit: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        let mut yield_sum = 0.0f64;
        let mut yield_count = 0usize;
        let mut low_yield_passes = 0usize;
        let mut looped_passes = 0usize;
        let mut idle_passes = 0usize;
        // Arc-quality aggregates across all adaptive_pass spans:
        let mut mean_delta_sum = 0.0f64;
        let mut mean_delta_count = 0usize;
        let mut sinuosity_sum = 0.0f64;
        let mut sinuosity_count = 0usize;
        let mut max_sinuosity = 0.0f64;
        let mut max_sinuosity_span_id: Option<u64> = None;
        let mut zigzag_passes = 0usize; // sign_flip_rate > 0.3
        // Engagement aggregates
        let mut engagement_sum = 0.0f64;
        let mut engagement_count = 0usize;
        let mut global_max_engagement = 0.0f64;
        let mut high_engagement_passes = 0usize; // max_engagement > 0.5
        let mut over_target_sum = 0.0f64;
        let mut target_frac_first: Option<f64> = None;
        let mut worst: Vec<(
            f64,
            u64,
            &rs_cam_core::trace::debug_trace::ToolpathDebugSpan,
        )> = Vec::new();
        let mut worst_arc: Vec<(f64, &rs_cam_core::trace::debug_trace::ToolpathDebugSpan)> =
            Vec::new();
        for span in &pass_spans {
            if let Some(reason) = span.exit_reason.as_deref() {
                *passes_by_exit.entry(reason.to_owned()).or_insert(0) += 1;
                if reason.contains("loop") {
                    looped_passes += 1;
                }
                if reason.contains("idle") {
                    idle_passes += 1;
                }
            }
            if let Some(&y) = span.counters.get("yield_ratio") {
                yield_sum += y;
                yield_count += 1;
                if y < 0.1 {
                    low_yield_passes += 1;
                }
                let steps = span.counters.get("step_count").copied().unwrap_or(0.0) as u64;
                worst.push((y, steps, span));
            }
            if let Some(&d) = span.counters.get("mean_angle_delta") {
                mean_delta_sum += d;
                mean_delta_count += 1;
                worst_arc.push((d, span));
            }
            if let Some(&s) = span.counters.get("sinuosity") {
                sinuosity_sum += s;
                sinuosity_count += 1;
                if s > max_sinuosity {
                    max_sinuosity = s;
                    max_sinuosity_span_id = Some(span.id);
                }
            }
            if span.counters.get("sign_flip_rate").copied().unwrap_or(0.0) > 0.3 {
                zigzag_passes += 1;
            }
            if let Some(&me) = span.counters.get("mean_engagement") {
                engagement_sum += me;
                engagement_count += 1;
            }
            if let Some(&mx) = span.counters.get("max_engagement") {
                if mx > global_max_engagement {
                    global_max_engagement = mx;
                }
                if mx > 0.5 {
                    high_engagement_passes += 1;
                }
            }
            if let Some(&ot) = span.counters.get("over_target_rate") {
                over_target_sum += ot;
            }
            if target_frac_first.is_none()
                && let Some(&tf) = span.counters.get("target_frac")
            {
                target_frac_first = Some(tf);
            }
        }
        worst.sort_by(|a, b| a.0.total_cmp(&b.0));
        let worst_json: Vec<_> = worst
            .iter()
            .take(10)
            .map(|(y, steps, span)| {
                serde_json::json!({
                    "id": span.id,
                    "label": span.label,
                    "exit_reason": span.exit_reason,
                    "yield_ratio": y,
                    "step_count": steps,
                    "idle_count": span.counters.get("idle_count").copied().unwrap_or(0.0),
                    "search_evaluations": span.counters.get("search_evaluations").copied().unwrap_or(0.0),
                    "z_level": span.z_level,
                    "xy_bbox": span.xy_bbox,
                })
            })
            .collect();
        worst_arc.sort_by(|a, b| b.0.total_cmp(&a.0)); // descending — worst first
        let worst_arc_json: Vec<_> = worst_arc
            .iter()
            .take(10)
            .map(|(d, span)| {
                serde_json::json!({
                    "id": span.id,
                    "label": span.label,
                    "exit_reason": span.exit_reason,
                    "mean_angle_delta": d,
                    "angle_delta_std": span.counters.get("angle_delta_std").copied().unwrap_or(0.0),
                    "sign_flip_rate": span.counters.get("sign_flip_rate").copied().unwrap_or(0.0),
                    "sinuosity": span.counters.get("sinuosity").copied().unwrap_or(0.0),
                    "step_count": span.counters.get("step_count").copied().unwrap_or(0.0),
                    "z_level": span.z_level,
                    "xy_bbox": span.xy_bbox,
                })
            })
            .collect();

        let mut response = serde_json::json!({
            "summary": {
                "schema_version": trace.schema_version,
                "toolpath_name": trace.toolpath_name,
                "operation_label": trace.operation_label,
                "span_count": trace.spans.len(),
                "hotspot_count": trace.hotspots.len(),
                "annotation_count": trace.annotations.len(),
                "dominant_span_kind": trace.summary.dominant_span_kind,
                "dominant_span_elapsed_us": trace.summary.dominant_span_elapsed_us,
            },
            "diagnostics": {
                "pass_count": pass_spans.len(),
                "passes_by_exit_reason": passes_by_exit,
                "low_yield_passes": low_yield_passes,
                "looped_passes": looped_passes,
                "idle_passes": idle_passes,
                "avg_yield_ratio": if yield_count > 0 { yield_sum / yield_count as f64 } else { 0.0 },
                "worst_yields": worst_json,
                "arc_quality": {
                    "avg_mean_angle_delta": if mean_delta_count > 0 { mean_delta_sum / mean_delta_count as f64 } else { 0.0 },
                    "avg_sinuosity": if sinuosity_count > 0 { sinuosity_sum / sinuosity_count as f64 } else { 0.0 },
                    "max_sinuosity": max_sinuosity,
                    "max_sinuosity_span_id": max_sinuosity_span_id,
                    "zigzag_passes": zigzag_passes,
                    "worst_arc_passes": worst_arc_json,
                },
                "engagement": {
                    "target_frac": target_frac_first,
                    "avg_mean_engagement": if engagement_count > 0 { engagement_sum / engagement_count as f64 } else { 0.0 },
                    "max_engagement": global_max_engagement,
                    "high_engagement_passes": high_engagement_passes,
                    "avg_over_target_rate": if engagement_count > 0 { over_target_sum / engagement_count as f64 } else { 0.0 },
                },
            },
            "hotspots": trace.hotspots,
            "annotations": trace.annotations,
        });
        // CLI-04: this response used to write `spans_returned` and
        // `spans_total_matching` by hand and say nothing at all about
        // truncation — an agent could not tell a short answer from a
        // complete one. `CappedArray` reports all four keys.
        let cap = if limit == 0 { usize::MAX } else { limit };
        let capped = CappedArray::from_parts(visible, total_matching, cap);
        if let serde_json::Value::Object(map) = &mut response {
            capped.insert_keys("spans", map);
            map.insert("spans".into(), capped.into_value());
        }
        json_str(response)
    }

    pub(super) fn mcp_inspect_spans(
        &self,
        index: usize,
        kind: Option<&str>,
        parent_id: Option<u32>,
        pass_index: Option<u32>,
        region_id: Option<u32>,
        max_spans: Option<usize>,
    ) -> String {
        let session = &self.controller.state().session;
        let gui = &self.controller.state().gui;
        let Some(tc) = session.toolpath_configs().get(index) else {
            return text(format!("Toolpath {index} not found."));
        };
        let Some(rt) = gui.toolpath_rt.get(&tc.id) else {
            return text(format!(
                "Toolpath {index} has no runtime entry. Run generate_toolpath first."
            ));
        };
        let Some(result) = rt.result.as_ref() else {
            return text(format!(
                "Toolpath {index} not generated. Run generate_toolpath first."
            ));
        };

        let n_moves = result.toolpath().moves.len();
        match build_inspect_spans_response(
            tc.id,
            index,
            &tc.name,
            tc.operation.label(),
            n_moves,
            result.spans(),
            result.spans_valid(),
            kind,
            parent_id,
            pass_index,
            region_id,
            max_spans,
        ) {
            Ok(value) => json_str(value),
            Err(msg) => json_str(serde_json::json!({ "error": msg })),
        }
    }

    pub(super) fn mcp_diagnostic_snapshot(&self) -> Vec<serde_json::Value> {
        let state = self.controller.state();
        let sim_trace = state
            .simulation
            .results
            .as_ref()
            .and_then(|r| r.cut_trace.as_deref());
        let mut values = Vec::new();
        let evidence = viz_project_evidence(state);
        values.extend(
            state
                .session
                .diagnose_project_with_evidence(&evidence)
                .into_iter()
                .filter_map(|diag| serde_json::to_value(diag).ok()),
        );
        for index in 0..state.session.toolpath_count() {
            if let Ok(diags) = state.session.diagnose_toolpath_with_trace(index, sim_trace) {
                values.extend(
                    diags
                        .into_iter()
                        .filter_map(|diag| serde_json::to_value(diag).ok()),
                );
            }
        }
        values.extend(self.mcp_runtime_error_diagnostics());
        values
    }

    fn mcp_runtime_error_diagnostics(&self) -> Vec<serde_json::Value> {
        let state = self.controller.state();
        state
            .session
            .toolpath_configs()
            .iter()
            .enumerate()
            .filter_map(|(index, tc)| {
                let rt = state.gui.toolpath_rt.get(&tc.id)?;
                // A/M11: only genuine failures. A disabled op reports
                // `Disabled` (no error text) and a sequencing block reports
                // `AwaitingPriorStock` — neither belongs in an error list an
                // agent has to triage.
                let error = ComputeStatus::effective(tc.enabled, &rt.status).error_text()?;
                Some(serde_json::json!({
                    "id": format!("runtime.generate_error.{}", tc.id),
                    "scope": { "kind": "toolpath", "id": tc.id },
                    "category": "state",
                    "severity": "blocking",
                    "confidence": "verified",
                    "state": "current",
                    "source": { "kind": "gui_runtime", "toolpath_index": index },
                    "message": error,
                }))
            })
            .collect()
    }

    /// PR-3: unified per-toolpath diagnostics. Calls
    /// [`ProjectSession::diagnose_toolpath`] which runs every
    /// adapter and applies supersession. The result is a flat list
    /// of [`rs_cam_core::diagnostics::Diagnostic`] suitable for
    /// the GUI params panel or any MCP consumer that wants a single
    /// canonical view.
    pub(super) fn mcp_get_toolpath_diagnostics(&self, index: usize) -> String {
        let state = self.controller.state();
        // The active sim trace lives on the viz-side state, not on
        // the core session — pass it explicitly so the load gates
        // see fresh evidence rather than `NeedsSimulation`.
        let sim_trace = state
            .simulation
            .results
            .as_ref()
            .and_then(|r| r.cut_trace.as_deref());
        match state.session.diagnose_toolpath_with_trace(index, sim_trace) {
            Ok(diagnostics) => {
                json_str(serde_json::to_value(&diagnostics).unwrap_or(serde_json::Value::Null))
            }
            Err(e) => json_str(serde_json::json!({"error": format!("{e}")})),
        }
    }

    /// PR-3: project-wide diagnostics (collisions, air-cut high,
    /// generated-empty, plunge stress). Builds a `ProjectEvidence`
    /// borrow view from viz-side simulation state — collisions
    /// and runtime/engagement readings reflect the latest run rather
    /// than the (always empty in GUI mode) core-session snapshot.
    pub(super) fn mcp_get_project_diagnostics(&self) -> String {
        let state = self.controller.state();
        let evidence = viz_project_evidence(state);
        let diagnostics = state.session.diagnose_project_with_evidence(&evidence);
        json_str(serde_json::to_value(&diagnostics).unwrap_or(serde_json::Value::Null))
    }
}

/// Build a `ProjectEvidence` borrow view from viz-side state so MCP
/// handlers can hand off to core diagnostics without GUI/MCP drift.
/// Pulls boundaries from `state.simulation.results`, rapid collisions
/// from `state.simulation.checks`, and the cut trace from the results
/// arc.
pub(crate) fn viz_project_evidence(
    state: &crate::state::AppState,
) -> rs_cam_core::session::ProjectEvidence<'_> {
    state.simulation.project_evidence()
}

/// Generation-debug span "kind" strings that participate in a given
/// structural `SpanKind`. When the input is itself a generation-debug kind
/// (e.g. "adaptive_pass"), the synonym set is just `{input}` so the filter
/// keeps backward compatibility with the existing string vocabulary.
///
/// Wave D3: the structural half of this used to be a list of string
/// literals with a catch-all fallback, so a NEW `SpanKind` variant silently
/// fell through to "treat as a literal debug-trace kind" and matched
/// nothing, with no compile error and no runtime complaint. It now parses
/// the input into the enum first and matches that EXHAUSTIVELY — adding a
/// variant to `SpanKind` breaks this build until someone says what it
/// expands to. Only genuinely unparseable input (a real debug-trace kind)
/// takes the literal path.
fn expand_span_kind_synonyms(span_kind: &str) -> Vec<String> {
    use rs_cam_core::trace::toolpath_spans::SpanKind;
    let Ok(kind) = parse_span_kind_filter(span_kind) else {
        // Not a structural kind at all — a literal debug-trace kind.
        return vec![span_kind.to_owned()];
    };
    match kind {
        // Structural SpanKind synonyms expand to the matching debug kinds.
        SpanKind::DepthPass => vec![
            "z_level_clear".to_owned(),
            "adaptive_pass".to_owned(),
            "z_level".to_owned(),
        ],
        SpanKind::Entry => vec!["entry_search".to_owned()],
        // These structural kinds have no debug-trace generators yet — an
        // empty set so the filter matches nothing rather than falsely
        // matching by string.
        SpanKind::Operation
        | SpanKind::Region
        | SpanKind::LeadOut
        | SpanKind::LinkBridge
        | SpanKind::DressupArtifact
        | SpanKind::GeometryRefit
        | SpanKind::WaterlineCleanup
        | SpanKind::RapidOrderBarrier => Vec::new(),
    }
}

/// Map a debug-trace span "kind" string back to the structural `SpanKind`
/// (snake_case) it most directly contributes to. Op-internal phases
/// (preflight, widen_band, etc.) have no structural equivalent and return
/// `None`.
fn map_debug_kind_to_span_kind(debug_kind: &str) -> Option<&'static str> {
    match debug_kind {
        "z_level_clear" | "adaptive_pass" | "z_level" => Some("depth_pass"),
        "entry_search" => Some("entry"),
        _ => None,
    }
}

/// Map an MCP `span_kind` string (snake_case) to the `SpanKind` enum.
///
/// Wave D3: the string table lives in core
/// ([`rs_cam_core::trace::toolpath_spans::SpanKind::as_key`], exhaustive) rather
/// than being transcribed here, so a new variant cannot be silently absent
/// from the agent vocabulary.
pub(super) fn parse_span_kind_filter(
    s: &str,
) -> Result<rs_cam_core::trace::toolpath_spans::SpanKind, String> {
    use rs_cam_core::trace::toolpath_spans::SpanKind;
    SpanKind::from_key(s).ok_or_else(|| {
        let known: Vec<&str> = SpanKind::ALL.iter().map(|k| k.as_key()).collect();
        format!(
            "unknown span_kind {s:?} — known kinds: {}",
            known.join(", ")
        )
    })
}

fn span_to_json(id: usize, s: &rs_cam_core::trace::toolpath_spans::Span) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "kind": s.kind.label(),
        "start_move": s.start_move,
        "end_move": s.end_move,
        "is_boundary": s.is_boundary(),
        "label": &*s.label,
        "payload": s.payload.as_ref().map(|p| format!("{p:?}")),
        // Wave D3: a `region` span is either a planner territory NODE or one
        // GENERATOR pass, and their `region_id`s index different tables.
        // Published as its own key so an agent never has to parse the label
        // (or the Debug-formatted payload) to tell them apart. `null` on
        // every non-region span.
        "region_role": s.region_role().map(|role| role.label()),
    })
}

/// Build the JSON response body for `inspect_spans`. Returns the response
/// envelope on success or an error message on bad filter input.
///
/// Default (no filter) returns a summary: `kind_counts` plus outermost spans
/// (Operation + DepthPass) under `top_level` with child counts. Setting any
/// of `kind`, `parent_id`, `pass_index`, `region_id` switches to detail mode
/// and returns matching spans under `spans`, with `total_matching` and
/// `truncated` reflecting `max_spans`.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_inspect_spans_response(
    toolpath_id: rs_cam_core::ToolpathId,
    toolpath_index: usize,
    name: &str,
    operation_label: &str,
    n_moves: usize,
    spans: &[rs_cam_core::trace::toolpath_spans::Span],
    spans_valid: bool,
    kind: Option<&str>,
    parent_id: Option<u32>,
    pass_index: Option<u32>,
    region_id: Option<u32>,
    max_spans: Option<usize>,
) -> Result<serde_json::Value, String> {
    use rs_cam_core::trace::toolpath_spans::{SpanKind, SpanPayload};

    let kind_filter = kind.map(parse_span_kind_filter).transpose()?;

    // Validate parent_id and resolve its move range.
    let parent_range: Option<(usize, usize)> = match parent_id {
        Some(pid) => {
            let parent = spans.get(pid as usize).ok_or_else(|| {
                format!("parent_id {pid} out of range (span_count={})", spans.len())
            })?;
            Some((parent.start_move, parent.end_move))
        }
        None => None,
    };

    // Per-kind tally for the summary row — always included.
    let mut kind_counts: std::collections::BTreeMap<&'static str, usize> =
        std::collections::BTreeMap::new();
    for s in spans {
        *kind_counts.entry(s.kind.label()).or_insert(0) += 1;
    }

    let filter_active =
        kind.is_some() || parent_id.is_some() || pass_index.is_some() || region_id.is_some();

    let mut response = serde_json::json!({
        "toolpath_id": toolpath_id,
        "toolpath_index": toolpath_index,
        "name": name,
        "operation": operation_label,
        "move_count": n_moves,
        "span_count": spans.len(),
        "spans_valid": spans_valid,
        "kind_counts": kind_counts,
    });

    if !filter_active {
        // Summary mode: outermost spans only (Operation + DepthPass), with
        // child counts of contained non-boundary spans (excluding self).
        //
        // Checkpoint L-3 gave this array the same cap as `span_summaries`
        // "for uniformity". **It is a latent bound, not a measured cost**:
        // B-1 measured summary mode on a 33,195-span Unified Finish at
        // **685 bytes** — `top_level` holds only Operation and DepthPass
        // spans, of which that fixture has few. Nothing here fixed an
        // observed problem. What it removes is an unbounded array whose
        // `child_count` is additionally an O(top_level x spans) nested scan,
        // so both the byte count and the scan are now bounded by the cap.
        //
        // CLI-04: the cap, the count and the `top_level_*` vocabulary all
        // come from `rs_cam_mcp::response` now. This branch used to write
        // the four keys by hand beside a `DEFAULT_MAX_TOP_LEVEL_SPANS` it
        // imported from there — the same words, spelled twice.
        let cap = max_spans.unwrap_or(DEFAULT_MAX_TOP_LEVEL_SPANS);
        let total_matching = spans
            .iter()
            .filter(|s| matches!(s.kind, SpanKind::Operation | SpanKind::DepthPass))
            .count();
        // A lazy iterator, not a collected Vec: `cap_json_values` stops at
        // the cap or the byte budget, so nothing beyond it is ever built
        // and the O(top_level x spans) child scan is bounded with it.
        let rows = spans
            .iter()
            .enumerate()
            .filter(|(_, s)| matches!(s.kind, SpanKind::Operation | SpanKind::DepthPass))
            .map(|(id, s)| {
                let child_count = spans
                    .iter()
                    .enumerate()
                    .filter(|(other_id, c)| {
                        *other_id != id
                            && !c.is_boundary()
                            && c.start_move >= s.start_move
                            && c.end_move <= s.end_move
                    })
                    .count();
                let mut v = span_to_json(id, s);
                if let serde_json::Value::Object(map) = &mut v {
                    map.insert("child_count".into(), serde_json::json!(child_count));
                }
                v
            });
        let mut budget = ResponseBudget::new(MAX_RESPONSE_BYTES);
        let top_level = cap_json_values(total_matching, rows, cap, &mut budget);

        if let serde_json::Value::Object(map) = &mut response {
            top_level.insert_keys("top_level", map);
            map.insert("top_level".into(), top_level.into_value());
            map.insert(
                "hint".into(),
                serde_json::json!(
                    "Pass kind, parent_id, pass_index, or region_id to retrieve detail spans."
                ),
            );
        }
        return Ok(response);
    }

    // Detail mode: collect matching spans, then truncate.
    let matching: Vec<(usize, &rs_cam_core::trace::toolpath_spans::Span)> = spans
        .iter()
        .enumerate()
        .filter(|(id, s)| {
            if let Some(want) = kind_filter
                && s.kind != want
            {
                return false;
            }
            if let Some((p_start, p_end)) = parent_range {
                // Child must be strictly different from parent and contained
                // within parent's range. Boundary spans at the parent's
                // start/end count as inside.
                if (*id as u32) == parent_id.unwrap_or(u32::MAX) {
                    return false;
                }
                if s.start_move < p_start || s.end_move > p_end {
                    return false;
                }
            }
            if let Some(want_pi) = pass_index {
                match &s.payload {
                    Some(SpanPayload::DepthPass { pass_index: pi, .. }) if *pi == want_pi => {}
                    _ => return false,
                }
            }
            if let Some(want_rid) = region_id {
                match &s.payload {
                    Some(SpanPayload::Region { region_id: rid, .. }) if *rid == want_rid => {}
                    _ => return false,
                }
            }
            true
        })
        .collect();

    // CLI-04: the same bounded-array primitive the summary branch uses.
    // The cap was the bare literal `50` here, 30 lines below a branch
    // that read its own cap from `rs_cam_mcp::response`; it is
    // `DEFAULT_MAX_DETAIL_SPANS` now, and the byte backstop applies to
    // this array too.
    let total_matching = matching.len();
    let cap = max_spans.unwrap_or(DEFAULT_MAX_DETAIL_SPANS);
    let mut budget = ResponseBudget::new(MAX_RESPONSE_BYTES);
    let capped = cap_json_values(
        total_matching,
        matching.into_iter().map(|(id, s)| span_to_json(id, s)),
        cap,
        &mut budget,
    );

    if let serde_json::Value::Object(map) = &mut response {
        // CLI-04 follow-up (2026-09-18): the four keys were `truncated`,
        // `total_matching`, `max_spans` and `returned` — bare words, while
        // the summary branch twelve lines up already published
        // `top_level_*` through the same primitive. One vocabulary now:
        // `spans_total_matching`, `spans_returned`, `spans_truncated`,
        // `spans_cap`. The `inspect_spans` tool description moved with it.
        capped.insert_keys("spans", map);
        map.insert("spans".into(), capped.into_value());
    }
    Ok(response)
}
