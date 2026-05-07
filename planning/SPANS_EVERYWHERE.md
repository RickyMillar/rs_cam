# Spans Everywhere — Plumbing the new SpanKind backbone to GUI + MCP

After the toolpath_spans refactor (Phases 1–6, commits `6979026..0167c9a`), every dressup transform now produces and preserves a typed `AnnotatedToolpath { toolpath, spans, spans_valid }`. The structure is correct end-to-end *inside* the dressup pipeline — but the GUI/MCP currently strip it back to raw `Toolpath` at the worker boundary, so none of it reaches users or agents.

This plan plumbs spans through the rest of the system, retires the parallel "what-belongs-to-what" side-channels (`OperationAnnotations`, ad-hoc string taxonomies in debug trace), and unlocks span-typed analysis surfaces.

## Bottleneck

`crates/rs_cam_viz/src/compute/worker/execute/mod.rs:409-410`:
```rust
tp = apply_dressups(annotated, req, ...).toolpath;
//                                       ^^^^^^^^^ discards spans + spans_valid
```

Until viz state carries `AnnotatedToolpath` (not just `Toolpath`), every downstream win is blocked.

## Slice 1 — Foundation

Goal: spans flow end-to-end into viz state and become inspectable. Nothing visible to users yet, but everything else becomes buildable.

- **S1.1** Persist `AnnotatedToolpath` in `state::toolpath` runtime (replace `Toolpath` field).
- **S1.2** Stop calling `.toolpath` at the worker boundary in `worker/execute/mod.rs`.
- **S1.3** Add `inspect_spans(toolpath_id)` MCP tool returning `[{id, kind, start_move, end_move, payload}]`. Cheapest unlock: agents can ask "what is the structural anatomy of toolpath N?" without parsing 7k-move dumps.
- **S1.4** Add structural-spans row to `ui/sim_diagnostics.rs:741` ("Structural spans: {Operation, DepthPass×N, LinkBridge×K, …}") so the new model is visible to humans.

Acceptance: `inspect_spans(1)` on the wanaka Back Rough returns 1 Operation + 18 DepthPass + N RapidOrderBarrier spans matching the narration's Z-level structure.

## Slice 2 — Agent surface

Goal: MCP analysis tools become span-typed. Agents can ask "show me engagement for pass 4" or "did any LinkBridge trip the chipload gate?" as one-line queries.

- **S2.1** Stamp `span_id: Option<SpanId>` (or `span_path: Vec<SpanId>` for nesting) onto `SimulationCutSample`, `Issue`, `Hotspot` at sim time, using the `AnnotatedToolpath` already threaded through.
- **S2.2** Extend `get_cut_trace` MCP tool with `span_kind` / `span_id` / `pass_index` filter args.
- **S2.3** Migrate `narrate_toolpath` to read `SpanKind::DepthPass` directly instead of re-deriving via `Z_EPSILON_MM` clustering. Eliminates the wanaka phantom-passes artifact (`z=22.5` and `z=22.0` showing as separate things).
- **S2.4** Retire string-typed `"adaptive_pass"` / `"z_level_clear"` in `get_generation_debug_trace`; debug-trace spans carry a `SpanKind` (with a typed payload extension for op-internal kinds).
- **S2.5** Span-scoped `get_tool_load_report` — per-DepthPass MRR/feed-stress histograms surface the high-DOC pass-1 vs steady-state distinction (the wanaka 40 cm³/3mm DOC question becomes one MCP call).

Acceptance: validating wanaka Back Rough no longer requires narration prose parsing — `get_cut_trace(span_kind=DepthPass, pass_index=1)` returns the pass-1 sample set directly.

## Slice 3 — Human surface

Goal: GUI debug/metrics views become span-aware. Highest leverage for *interactive* debugging.

- **S3.1** Span-aware 3D renderer: `render/toolpath_render.rs` colors moves by `SpanKind` (Entry one hue, LinkBridge dashed, DressupArtifact muted, DepthPass tinted by index) instead of just `MoveType`.
- **S3.2** Hover tooltip: "DepthPass 3, Region 2" (currently shows only move type / index).
- **S3.3** "Jump to start of pass N" scrub controls in `sim_timeline.rs`. Generalize the existing `simulation.rs:858` `trace_target_for_span` plumbing from debug-trace spans to `AnnotatedToolpath` spans.
- **S3.4** SpanKind filter in sim view ("show only LinkBridges").
- **S3.5** Bucket chipload/engagement graphs by SpanKind so per-bucket statistics ("avg engagement on DepthPass moves vs LinkBridge moves") surface natively.

Acceptance: opening any wanaka toolpath in the GUI, the user can visually distinguish what `link_moves` added from what the planner generated, and scrub directly to "pass 4 entry."

## Slice 4 — Debt retirement

Goal: only one canonical "what belongs where" model in the codebase.

- **S4.1** Drop the `OperationAnnotations` side-channel. Currently read by `state/simulation.rs:1041,1208`, `ui/sim_diagnostics.rs:743`, `app/mcp.rs:904`. With spans persisted, annotations are redundant.
- **S4.2** Migrate `state/simulation.rs:858-885` `trace_target_for_span` to accept `AnnotatedToolpath` spans (currently only over debug-trace spans).
- **S4.3** Fold `narrate.rs::ZLevelSummary` into the `SpanKind::DepthPass` reader from S2.3.
- **S4.4** Fold `ui/sim_op_list.rs` semantic-kind tree alongside or under `SpanKind`.

Acceptance: `grep -r OperationAnnotations` returns zero hits in non-test viz code.

## Sequencing

S1 is load-bearing — nothing else builds without it.
S2 and S3 are independent of each other after S1.
S4 cleans up the parallel models exposed by S2/S3 and is partially incremental (e.g., S4.2 is required by S3.3).

Recommended order: **S1 → S2 → S4 (partial) → S3 → S4 (final)**. S2 first because the MCP-driven validation workflow (we just used this on wanaka) is where new analysis bottlenecks tend to surface earliest.

## Tracking

See task IDs starting from #59 in the workspace task list. Each slice has one umbrella task and one task per sub-item.
