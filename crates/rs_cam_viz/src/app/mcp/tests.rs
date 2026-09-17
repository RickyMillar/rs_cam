use super::diagnostics::build_inspect_spans_response;
use super::simulation::{CutTraceRequest, build_cut_trace_response};
use super::*;
use rs_cam_core::compute::tool_config::{ToolConfig, ToolId, ToolType};
use rs_cam_core::session::{AddToolpathArgs, Command, RemoveToolpathArgs};
use rs_cam_core::trace::toolpath_spans::{RegionSpanRole, Span, SpanKind, SpanPayload};

// ── B7 divergence 3: the wrong-tool fallback ────────────────────────
//
// Two tools with visibly different geometry, and a toolpath pointing at
// a THIRD id that does not exist. The parent revision's lookup is
// transcribed verbatim so the defect stays executable: checking out the
// parent was not available to this wave (the working tree is shared with
// another live lane), and a transcription keeps failing if anyone
// reintroduces the fallback, which a one-off checkout would not.

fn two_tool_fixture() -> Vec<ToolConfig> {
    let mut a = ToolConfig::new_default(ToolId(1), ToolType::EndMill);
    a.name = "Ø6 flat".to_owned();
    a.diameter = 6.0;
    a.flute_count = 2;
    let mut b = ToolConfig::new_default(ToolId(2), ToolType::BallNose);
    b.name = "Ø1 ball".to_owned();
    b.diameter = 1.0;
    b.flute_count = 4;
    vec![a, b]
}

/// The parent revision's lookup, transcribed from
/// `mcp_narrate_toolpath` at parent `88ce23a`.
fn parent_revision_tool_lookup(tools: &[ToolConfig], tool_id: usize) -> Option<&ToolConfig> {
    tools
        .iter()
        .find(|tool| tool.id.0 == tool_id)
        .or_else(|| tools.first())
}

/// **The exhibit.** On an unresolvable tool id the parent silently
/// returned the FIRST tool — a Ø6 2-flute end mill standing in for a Ø1
/// 4-flute ball nose. That diameter sets narration's large-arc threshold
/// and every tool-scaled hint; the flute count sits under every chipload
/// sentence; and the same `ToolConfig` builds the cutter handed to
/// `narrate_toolpath_with_context`. Nothing in the emitted text said the
/// numbers were about another tool.
#[test]
fn narration_tool_lookup_refuses_where_the_parent_took_another_tool() {
    let tools = two_tool_fixture();
    const MISSING_ID: usize = 99;

    let parent = parent_revision_tool_lookup(&tools, MISSING_ID)
        .expect("the parent revision always found *a* tool — that is the defect");
    assert_eq!(
        parent.name, "Ø6 flat",
        "the transcribed parent must reproduce the fallback, or this exhibit proves \
         nothing",
    );
    assert!(
        (parent.diameter - 1.0).abs() > 4.0 && parent.flute_count != 4,
        "the fixture must make the substitution VISIBLE — a fallback to a tool with the \
         same geometry would be harmless and would not demonstrate the defect",
    );

    assert!(
        crate::app::RsCamApp::narration_tool_for(&tools, MISSING_ID).is_none(),
        "the shipped lookup must refuse an unresolvable tool id rather than narrate \
         with another tool's geometry",
    );
}

/// Non-vacuity: the refusal is not a blanket one. A resolvable id still
/// returns its OWN tool, and not the first one.
#[test]
fn narration_tool_lookup_still_finds_the_toolpaths_own_tool() {
    let tools = two_tool_fixture();
    let found =
        crate::app::RsCamApp::narration_tool_for(&tools, 2).expect("tool id 2 is configured");
    assert_eq!(found.name, "Ø1 ball");
    assert!((found.diameter - 1.0).abs() < 1e-9);
}

// ══ TD3 B-2 — bounded reads (Checkpoint L) ═════════════════════════
//
// The read these sentries guard emitted a **60,517,035-byte** JSON-RPC
// line on 2026-08-08 (56,225,225 bytes of tool text) against a
// 1,588,883-sample trace — 16,245x the median MCP response in the same
// census. 94 % of it was `span_summaries`, which had no cap and no way
// to say it had been shortened. Full evidence:
// `planning/review_2026-08-08/GLV2_CRASH_CAPTURE.md`.

/// A session with `n` toolpaths, each carrying `spans_per_tp` spans and
/// one cut sample per span, plus the matching runtime results.
///
/// Returns the state and the ids the session actually assigned — the
/// ids are NOT the indices, which is the whole point of the L-5 sentry
/// below and is exactly what was measured on a real project.
fn cut_trace_fixture(
    n: usize,
    spans_per_tp: usize,
) -> (
    crate::state::AppState,
    Vec<rs_cam_core::ToolpathId>,
    rs_cam_core::stock::simulation_cut::SimulationCutTrace,
) {
    use rs_cam_core::stock::simulation_cut::{SimulationCutSample, SimulationCutTrace};
    use rs_cam_core::trace::toolpath_spans::{AnnotatedToolpath, Span, SpanId, SpanKind};

    let mut state = crate::state::AppState::default();
    let mut ids = Vec::new();
    let mut samples = Vec::new();

    for t in 0..n {
        let operation = crate::state::toolpath::OperationConfig::new_default(
            rs_cam_core::compute::catalog::OperationType::Pocket,
        );
        let op_type = operation.op_type();
        let config = rs_cam_core::session::ToolpathConfig {
            // Overwritten by `add_toolpath`, which assigns a fresh id —
            // which is precisely why ids and indices diverge.
            id: rs_cam_core::ToolpathId(0),
            name: format!("tp {t}"),
            enabled: true,
            operation,
            dressups: crate::state::toolpath::DressupConfig::for_op(op_type),
            heights: crate::state::toolpath::HeightsConfig::default(),
            tool_id: 0,
            model_id: 0,
            pre_gcode: None,
            post_gcode: None,
            boundary: crate::state::toolpath::BoundaryConfig::default(),
            boundary_inherit: true,
            stock_source: crate::state::toolpath::StockSource::default(),
            coolant: rs_cam_core::gcode::CoolantMode::Off,
            face_selection: None,
            debug_options: rs_cam_core::trace::debug_trace::ToolpathDebugOptions::default(),
            feeds_provenance: rs_cam_core::feeds::FeedsProvenance::default(),
            rest_analysis: crate::state::toolpath::RestAnalysisConfig::default(),
            planner_origin: None,
        };
        let index = state
            .session
            .apply(Command::AddToolpath(AddToolpathArgs {
                setup_index: 0,
                config: Box::new(config),
            }))
            .expect("setup 0 exists on a default session")
            .created
            .expect("the AddToolpath row reports the new toolpath index");
        let id = state
            .session
            .get_toolpath_config(index)
            .expect("just added")
            .id;
        ids.push(id);

        // One Operation span plus `spans_per_tp - 1` refit spans — the
        // shape that produced 33,195 entries for a single operation.
        let mut spans = vec![Span::new(0, spans_per_tp, SpanKind::Operation)];
        for s in 1..spans_per_tp {
            spans.push(Span::new(s, s + 1, SpanKind::GeometryRefit));
        }
        let annotated =
            AnnotatedToolpath::with_spans(rs_cam_core::toolpath::Toolpath::new(), spans);
        let mut rt = crate::state::runtime::ToolpathRuntime::new(false);
        rt.result = Some(crate::state::toolpath::ToolpathResult {
            annotated: std::sync::Arc::new(annotated),
            stats: rs_cam_core::compute::config::ToolpathStats::default(),
            debug_trace: None,
            semantic_trace: None,
            debug_trace_path: None,
            drill_op: None,
        });
        state.gui.toolpath_rt.insert(id, rt);

        // One cutting sample per span, so every span has a non-zero
        // `sample_count` and therefore earns a summary row.
        for s in 0..spans_per_tp {
            samples.push(SimulationCutSample {
                toolpath_id: id,
                move_index: s,
                sample_index: s,
                is_cutting: true,
                segment_time_s: 0.01,
                span_path: vec![SpanId(s as u32)],
                ..SimulationCutSample::test_fixture()
            });
        }
    }

    let trace = SimulationCutTrace {
        sample_step_mm: 0.1,
        samples,
        ..SimulationCutTrace::test_fixture()
    };
    (state, ids, trace)
}

fn request(
    toolpath_id: Option<usize>,
    caps: rs_cam_mcp::response::CutTraceCaps,
) -> CutTraceRequest<'static> {
    CutTraceRequest {
        toolpath_id,
        max_hotspots: None,
        max_issues: None,
        span_kind: None,
        span_id: None,
        pass_index: None,
        include_drill_samples: false,
        caps,
    }
}

/// **The exhibit.** Uncapped, the shipped builder emits every span
/// summary — that is the pre-fix behaviour, still reachable through
/// `CutTraceCaps::unbounded()` so the red state stays executable. With
/// the ruled defaults the same trace answers bounded, and says so.
#[test]
fn an_unfiltered_read_answers_bounded_instead_of_emitting_everything() {
    let (state, _ids, trace) = cut_trace_fixture(4, 300);
    let population = 4 * 300;

    // Red: the parent behaviour, reproduced rather than remembered.
    let before = build_cut_trace_response(
        &state,
        &trace,
        &request(None, rs_cam_mcp::response::CutTraceCaps::unbounded()),
    )
    .expect("no filter, nothing to refuse");
    assert_eq!(
        before["span_summaries"].as_array().map(Vec::len),
        Some(population),
        "the uncapped build must still emit the whole population, or this \
         exhibit is not showing the behaviour the wave replaced",
    );
    let before_bytes = serde_json::to_string(&before).unwrap().len();

    // Green: the shipped defaults.
    let after = build_cut_trace_response(
        &state,
        &trace,
        &request(None, rs_cam_mcp::response::CutTraceCaps::default()),
    )
    .expect("no filter, nothing to refuse");
    let after_bytes = serde_json::to_string(&after).unwrap().len();

    assert_eq!(
        after["span_summaries"].as_array().map(Vec::len),
        Some(rs_cam_mcp::response::DEFAULT_MAX_SPAN_SUMMARIES),
    );
    assert_eq!(
        after["span_summaries_total_matching"],
        serde_json::json!(population),
        "total_matching must be the PRE-CAP population — a count taken from \
         what was emitted would report a truncated array as complete",
    );
    assert_eq!(after["span_summaries_truncated"], serde_json::json!(true));
    assert_eq!(
        after["span_summaries_returned"],
        serde_json::json!(rs_cam_mcp::response::DEFAULT_MAX_SPAN_SUMMARIES),
    );
    assert!(
        after_bytes < before_bytes,
        "the bounded response must be smaller ({after_bytes} vs {before_bytes})",
    );
    assert!(
        after_bytes < rs_cam_mcp::response::MAX_RESPONSE_BYTES,
        "the bounded response must fit the 8 MiB backstop: {after_bytes} B",
    );
    assert_eq!(
        after["complete"],
        serde_json::json!(true),
        "an array shortened by its own item cap is still a COMPLETE \
         response; `complete` reports dropped sections, not truncation",
    );
    assert_eq!(after["sections_not_computed"], serde_json::json!([]));
}

/// **L-5, red-first.** Before this wave an id matching no toolpath got a
/// full skeleton with every array empty — indistinguishable from a
/// toolpath that genuinely produced no samples, and printed next to a
/// project-wide issue count that made it read as a measurement. It is
/// now refused, and the refusal names the valid ids.
#[test]
fn an_unmatched_toolpath_id_is_refused_and_names_the_valid_ids() {
    let (mut state, ids, trace) = cut_trace_fixture(3, 4);
    // Drop the first toolpath so ids and indices diverge — the measured
    // condition on the real project, where 4/5/6 were simultaneously
    // valid indices and valid ids of DIFFERENT toolpaths.
    let _ = state
        .session
        .apply(Command::RemoveToolpath(RemoveToolpathArgs { index: 0 }))
        .expect("index 0 exists");
    let live: Vec<usize> = (0..state.session.toolpath_count())
        .filter_map(|i| state.session.get_toolpath_config(i).map(|tc| tc.id.0))
        .collect();
    let dead = ids[0].0;
    assert!(
        !live.contains(&dead),
        "the fixture must actually orphan an id, or it proves nothing",
    );

    // Red: the parent returned a skeleton here. Assert the *shape* of
    // what made it dangerous — a well-formed, all-zero answer.
    let refusal =
        build_cut_trace_response(&state, &trace, &request(Some(dead), Default::default()))
            .expect_err("an id matching no toolpath is a caller error, not an empty result");
    assert!(
        refusal.contains("matches no toolpath"),
        "refusal must say what went wrong: {refusal}",
    );
    assert!(
        refusal.contains("NOT the index"),
        "refusal must correct the misreading that causes this: {refusal}",
    );
    for id in &live {
        assert!(
            refusal.contains(&id.to_string()),
            "refusal must list valid id {id}: {refusal}",
        );
    }

    // Non-vacuity: a VALID id still answers, and answers with data.
    let ok = build_cut_trace_response(
        &state,
        &trace,
        &request(Some(*live.first().unwrap()), Default::default()),
    )
    .expect("a valid id must still be served");
    assert!(
        ok["span_summaries"]
            .as_array()
            .is_some_and(|a| !a.is_empty()),
        "the refusal must not have become a blanket one",
    );
}

/// A section the byte backstop cannot fit is OMITTED and NAMED, never
/// emitted as an empty array — C25's standing rule, on the real
/// response builder rather than on the primitive.
#[test]
fn a_byte_starved_response_names_what_it_could_not_serve() {
    let (state, _ids, trace) = cut_trace_fixture(2, 200);
    let caps = rs_cam_mcp::response::CutTraceCaps {
        // Small enough that the always-kept scalars and `summary`
        // consume it outright, so `span_summaries` gets nothing.
        max_response_bytes: 200,
        ..Default::default()
    };
    let out = build_cut_trace_response(&state, &trace, &request(None, caps))
        .expect("a tight budget is not a refusal — it is a smaller answer");

    assert_eq!(
        out["complete"],
        serde_json::json!(false),
        "a response that dropped a section must not claim completeness",
    );
    let dropped = out["sections_not_computed"]
        .as_array()
        .expect("always present");
    assert!(
        !dropped.is_empty(),
        "the fixture must actually starve the budget, or it proves nothing",
    );
    for key in dropped {
        let name = key.as_str().expect("section names are strings");
        assert!(
            out.get(name).is_none(),
            "`{name}` did not fit and must be ABSENT; an empty array would be \
             indistinguishable from a real empty result (C25)",
        );
    }
    // Whatever the budget cost us, span_summaries still reports its true
    // population rather than a zero.
    assert_eq!(out["span_summaries_total_matching"], serde_json::json!(400),);
    assert!(
        out["span_summaries_returned"]
            .as_u64()
            .is_some_and(|r| r < 400),
        "the byte budget must bind here",
    );
    assert_eq!(out["span_summaries_truncated"], serde_json::json!(true));
}

/// Ordering is a documented contract, so a cap is reproducible: the same
/// request twice gives the same rows, and they are the leading ones.
#[test]
fn the_span_summary_cap_is_deterministic_and_documented() {
    let (state, _ids, trace) = cut_trace_fixture(3, 50);
    let caps = rs_cam_mcp::response::CutTraceCaps {
        span_summaries: 12,
        ..Default::default()
    };
    let a = build_cut_trace_response(&state, &trace, &request(None, caps)).unwrap();
    let b = build_cut_trace_response(&state, &trace, &request(None, caps)).unwrap();
    assert_eq!(
        a["span_summaries"], b["span_summaries"],
        "an unreproducible cap is not a contract",
    );
    let span_ids: Vec<u64> = a["span_summaries"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v["span_id"].as_u64())
        .collect();
    assert_eq!(
        span_ids,
        (0..12).collect::<Vec<u64>>(),
        "the cap must take the leading entries of the documented order: {}",
        rs_cam_mcp::response::ORDERING_SPAN_SUMMARIES,
    );
    assert_eq!(
        a["span_summaries_order"],
        serde_json::json!(rs_cam_mcp::response::ORDERING_SPAN_SUMMARIES),
        "the response must carry the order it was capped by",
    );
}

/// `inspect_spans` summary mode gets the same bound. Stated honestly:
/// this one is LATENT — B-1 measured summary mode at 685 bytes on a
/// 33,195-span operation, so nothing here fixed a measured cost.
#[test]
fn inspect_spans_summary_mode_is_capped_and_says_so() {
    use rs_cam_core::trace::toolpath_spans::{Span, SpanKind};
    let spans: Vec<Span> = (0..500)
        .map(|i| Span::new(i, i + 1, SpanKind::Operation))
        .collect();
    let out = build(&spans, None, None, None, None, Some(7));
    assert_eq!(out["top_level"].as_array().map(Vec::len), Some(7));
    assert_eq!(out["top_level_total_matching"], serde_json::json!(500));
    assert_eq!(out["top_level_returned"], serde_json::json!(7));
    assert_eq!(out["top_level_truncated"], serde_json::json!(true));
    assert_eq!(out["top_level_cap"], serde_json::json!(7));

    // Default cap, and an unbounded population under it reports clean.
    let small: Vec<Span> = (0..3)
        .map(|i| Span::new(i, i + 1, SpanKind::Operation))
        .collect();
    let out = build(&small, None, None, None, None, None);
    assert_eq!(out["top_level_truncated"], serde_json::json!(false));
    assert_eq!(
        out["top_level_cap"],
        serde_json::json!(rs_cam_mcp::response::DEFAULT_MAX_TOP_LEVEL_SPANS),
    );
}

/// Build a representative span tree:
/// - Operation 0..30
///   - DepthPass 0..15 (pass_index=0)
///     - Region 0..7 (region_id=0)
///     - Region 7..15 (region_id=1)
///   - DepthPass 15..30 (pass_index=1)
///     - Region 15..22 (region_id=2)
///     - Region 22..30 (region_id=3)
fn fixture_spans() -> Vec<Span> {
    vec![
        Span::new(0, 30, SpanKind::Operation),
        Span::new(0, 15, SpanKind::DepthPass).with_payload(SpanPayload::DepthPass {
            z_level: -2.0,
            pass_index: 0,
        }),
        Span::new(0, 7, SpanKind::Region).with_payload(SpanPayload::Region {
            region_id: 0,
            role: RegionSpanRole::GeneratorPass,
        }),
        Span::new(7, 15, SpanKind::Region).with_payload(SpanPayload::Region {
            region_id: 1,
            role: RegionSpanRole::GeneratorPass,
        }),
        Span::new(15, 30, SpanKind::DepthPass).with_payload(SpanPayload::DepthPass {
            z_level: -4.0,
            pass_index: 1,
        }),
        Span::new(15, 22, SpanKind::Region).with_payload(SpanPayload::Region {
            region_id: 2,
            role: RegionSpanRole::GeneratorPass,
        }),
        Span::new(22, 30, SpanKind::Region).with_payload(SpanPayload::Region {
            region_id: 3,
            role: RegionSpanRole::GeneratorPass,
        }),
    ]
}

fn build(
    spans: &[Span],
    kind: Option<&str>,
    parent_id: Option<u32>,
    pass_index: Option<u32>,
    region_id: Option<u32>,
    max_spans: Option<usize>,
) -> serde_json::Value {
    build_inspect_spans_response(
        rs_cam_core::ToolpathId(42),
        0,
        "tp",
        "pocket",
        30,
        spans,
        true,
        kind,
        parent_id,
        pass_index,
        region_id,
        max_spans,
    )
    .expect("valid filter")
}

#[test]
fn default_returns_summary_with_kind_counts_and_top_level() {
    let spans = fixture_spans();
    let v = build(&spans, None, None, None, None, None);

    assert_eq!(v["span_count"], 7);
    assert_eq!(v["move_count"], 30);
    assert_eq!(v["spans_valid"], true);
    // Spans array must NOT be present in summary mode.
    assert!(v.get("spans").is_none());

    let kc = &v["kind_counts"];
    assert_eq!(kc["Operation"], 1);
    assert_eq!(kc["DepthPass"], 2);
    assert_eq!(kc["Region"], 4);

    let top_level = v["top_level"].as_array().expect("top_level array");
    // Operation + 2 DepthPass = 3 entries, no Region leaves.
    assert_eq!(top_level.len(), 3);
    let kinds: Vec<&str> = top_level
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, vec!["Operation", "DepthPass", "DepthPass"]);

    // Operation has 6 contained spans (2 DepthPass + 4 Region).
    assert_eq!(top_level[0]["child_count"], 6);
    // Each DepthPass has 2 Region children.
    assert_eq!(top_level[1]["child_count"], 2);
    assert_eq!(top_level[2]["child_count"], 2);

    assert!(v["hint"].as_str().unwrap().contains("kind"));
}

#[test]
fn kind_filter_returns_only_matching_spans() {
    let spans = fixture_spans();
    let v = build(&spans, Some("depth_pass"), None, None, None, None);

    assert_eq!(v["total_matching"], 2);
    assert_eq!(v["truncated"], false);
    let arr = v["spans"].as_array().expect("spans array");
    assert_eq!(arr.len(), 2);
    for s in arr {
        assert_eq!(s["kind"], "DepthPass");
    }
}

#[test]
fn parent_id_filter_narrows_to_contained_children() {
    let spans = fixture_spans();
    // parent_id=1 is the first DepthPass (covers 0..15).
    let v = build(&spans, None, Some(1), None, None, None);
    let arr = v["spans"].as_array().expect("spans array");
    // Children: Region 0..7 (id=2) and Region 7..15 (id=3). The parent
    // itself is excluded.
    let ids: Vec<u64> = arr.iter().map(|s| s["id"].as_u64().unwrap()).collect();
    assert_eq!(ids, vec![2, 3]);
}

#[test]
fn parent_id_combined_with_kind_filters_correctly() {
    let spans = fixture_spans();
    // parent_id=0 (Operation), kind=region → all 4 regions.
    let v = build(&spans, Some("region"), Some(0), None, None, None);
    assert_eq!(v["total_matching"], 4);
}

#[test]
fn pass_index_filters_to_matching_depth_pass() {
    let spans = fixture_spans();
    let v = build(&spans, None, None, Some(1), None, None);
    let arr = v["spans"].as_array().expect("spans array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["kind"], "DepthPass");
    assert_eq!(arr[0]["start_move"], 15);
}

#[test]
fn region_id_filters_to_matching_region() {
    let spans = fixture_spans();
    let v = build(&spans, None, None, None, Some(2), None);
    let arr = v["spans"].as_array().expect("spans array");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["start_move"], 15);
    assert_eq!(arr[0]["end_move"], 22);
}

#[test]
fn max_spans_truncates_and_reports_total() {
    let spans = fixture_spans();
    let v = build(&spans, Some("region"), None, None, None, Some(2));
    assert_eq!(v["total_matching"], 4);
    assert_eq!(v["truncated"], true);
    assert_eq!(v["max_spans"], 2);
    let arr = v["spans"].as_array().expect("spans array");
    assert_eq!(arr.len(), 2);
}

#[test]
fn default_max_spans_is_50() {
    // Build 60 region spans.
    let mut spans = vec![Span::new(0, 60, SpanKind::Operation)];
    for i in 0..60 {
        spans.push(
            Span::new(i, i + 1, SpanKind::Region).with_payload(SpanPayload::Region {
                region_id: i as u32,
                role: RegionSpanRole::GeneratorPass,
            }),
        );
    }
    let v = build_inspect_spans_response(
        rs_cam_core::ToolpathId(1),
        0,
        "tp",
        "pocket",
        60,
        &spans,
        true,
        Some("region"),
        None,
        None,
        None,
        None,
    )
    .unwrap();
    assert_eq!(v["total_matching"], 60);
    assert_eq!(v["truncated"], true);
    assert_eq!(v["max_spans"], 50);
    assert_eq!(v["spans"].as_array().unwrap().len(), 50);
}

#[test]
fn invalid_kind_returns_error() {
    let spans = fixture_spans();
    let res = build_inspect_spans_response(
        rs_cam_core::ToolpathId(1),
        0,
        "tp",
        "pocket",
        30,
        &spans,
        true,
        Some("garbage"),
        None,
        None,
        None,
        None,
    );
    assert!(res.is_err());
}

#[test]
fn invalid_parent_id_returns_error() {
    let spans = fixture_spans();
    let res = build_inspect_spans_response(
        rs_cam_core::ToolpathId(1),
        0,
        "tp",
        "pocket",
        30,
        &spans,
        true,
        None,
        Some(999),
        None,
        None,
        None,
    );
    assert!(res.is_err());
}

/// `set_ui_view` documents these workspace keys — every key must
/// parse, every `Workspace` variant must round-trip through
/// `workspace_key` → `parse_workspace`, and unknown keys stay `None`.
#[test]
fn workspace_keys_round_trip() {
    // `Workspace::ALL`, not a hand-written list beside it: the list this
    // used to carry was a fourth copy of the same enumeration, and it is
    // exactly that kind of copy that left Readiness out of the Workspace
    // menu (G-WSMENU).
    for ws in Workspace::ALL {
        assert_eq!(parse_workspace(workspace_key(ws)), Some(ws));
    }
    // "sim" alias accepted on input (matches the RS_CAM_SCREENSHOT
    // env-var vocabulary).
    assert_eq!(parse_workspace("sim"), Some(Workspace::Simulation));
    assert_eq!(parse_workspace("not_a_workspace"), None);
}
