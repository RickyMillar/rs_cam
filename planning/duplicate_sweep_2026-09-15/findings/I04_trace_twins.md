# I04 — trace twins (debug_trace ↔ semantic_trace ↔ simulation_cut)
Verdict: TRUE_DUP (sanitize_filename_component ×3) / DRIFTED_DUP (write_*_artifact ×3) / FALSE_POSITIVE (artifact structs)

## Evidence
- Copies and line ranges (current, content-verified against the 2026-09-15 index):
  - `crates/rs_cam_core/src/debug_trace.rs:551-568`, `semantic_trace.rs:1226-1243`, `simulation_cut.rs:1846-1863`:
    `sanitize_filename_component` — literal-diff: byte-identical except the
    empty-input fallback literal (`"toolpath_debug"` / `"toolpath_trace"` /
    `"simulation_cut_trace"`). Pairs: 0.9499, 0.9336.
  - `debug_trace.rs:530-548` `write_toolpath_debug_artifact` ↔
    `semantic_trace.rs:1205-1223` `write_toolpath_trace_artifact` (0.9329):
    identical bodies (create_dir_all → ms timestamp → `"{ts}_{sanitized}.json"`
    → pretty-json write) apart from fn/artifact names. Third member
    `simulation_cut.rs:1758-1787` `write_simulation_cut_artifact` shares the
    skeleton but diverged (below).
  - `debug_trace.rs:125-134` `ToolpathDebugArtifact` ↔
    `semantic_trace.rs:498-508` `ToolpathTraceArtifact` (0.9451): same six
    envelope fields, different payload members.
- Callers:
  - `write_toolpath_trace_artifact`: prod callers `crates/rs_cam_cli/src/main.rs:490`,
    `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:418`; artifacts built
    at `crates/rs_cam_cli/src/job.rs:717`, `crates/rs_cam_viz/src/compute/worker/helpers.rs:74`.
  - `write_simulation_cut_artifact`: one prod caller
    `crates/rs_cam_viz/src/compute/worker/execute/mod.rs:262` (documented at
    `crates/rs_cam_core/benches/hot_paths.rs:964`); prune contract in
    `simulation_cut.rs:1795-1805`.
  - `write_toolpath_debug_artifact` / `ToolpathDebugArtifact`: **no production
    caller** — workspace rg finds only the module's own test
    (`debug_trace.rs:669`). Legacy of the older debug-only format;
    `ToolpathTraceArtifact` is the shipped envelope (schema const shared,
    `semantic_trace.rs:521`).
- Sentries/tests pinning behavior:
  - `simulation_cut.rs:2932` `sanitize_filename_handles_special_chars` pins
    sanitization + the `"simulation_cut_trace"` fallback.
  - `simulation_cut.rs:2179-2192` artifact test pins unique-path-per-write
    (pid+seq fix for the 2026-08-27 same-millisecond collision).
  - `debug_trace.rs:669`, `semantic_trace.rs:1670` write-and-readback tests.
  - No `crates/rs_cam_core/tests/` sentry references these writers.

## Drift / differences
- Authoritative side: `simulation_cut.rs::write_simulation_cut_artifact`.
  Drift: it alone adds `pid + per-process seq` to the file name (real
  concurrency bug fix, comment at `simulation_cut.rs:1766-1772`); the two
  trace writers still hand out `"{ms}_{stem}.json"` names that can collide in
  the same millisecond. It also feeds `prune_simulation_cut_artifacts`
  (age-parse on the first `_`-field), which the trace writers never needed.

## Proposed cleanup
- home: new private `crates/rs_cam_core/src/artifact_io.rs` hosting
  `sanitize_filename_component(input, fallback)` + one
  `write_json_artifact(dir, stem, &Serialize, NamingPolicy)` (uniqueness flag
  preserves sim_cut's pid+seq and prune-parsable prefix). risk: **low**
  for the sanitize + debug/semantic writers (no wire/disk-format change);
  **med** for folding sim_cut in (concurrency + prune contract; G-SIMDUMP).
  Also delete dead `ToolpathDebugArtifact`/`write_toolpath_debug_artifact`
  (or formally deprecate; they are test-only).
- proof test: keep `simulation_cut.rs:2932` and the unique-path assert;
  add one unit test asserting all three fallbacks via the shared helper.
- Note: unrelated lookalike `crates/rs_cam_cli/src/sweep.rs:278`
  `sanitize_filename` differs in behavior (keeps `.`, no lowercase, no
  fallback) — leave alone.
