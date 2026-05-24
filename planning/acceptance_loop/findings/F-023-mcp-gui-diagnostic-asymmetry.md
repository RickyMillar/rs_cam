# F-023 — MCP / GUI diagnostic surface asymmetry: "Selected model missing"

- **Stage:** substrate
- **Severity:** high
- **Status:** landed
- **First found in:** round-02 (2026-05-25)
- **Effort:** S–M
- **Linked PRs:** —
- **Source audits:** live smoke evidence (see Evidence section)

## Evidence

During the round-02 smoke run on `test_data/ux_2d_pocket.toml`, the
following sequence reproduced a diagnostic surface asymmetry:

1. `inspect_model` reports one model in the project with `id: 1`
   (the SVG `demo_pocket.svg`).
2. `add_toolpath(setup_index=0, operation_type="pocket", tool_index=0, model_id=0, name="AS001-pocket")`
   returns:

   ```
   ok: true
   summary: "Added toolpath 0 (Pocket)."
   warnings: []
   gui_banners: []
   diagnostic_delta: [
     load.chipload.within (state: needs_simulation),
     load.power.within (state: needs_simulation),
     load.deflection.within (state: needs_simulation)
   ]
   ```

   The `model_id: 0` reference is invalid — no model with that id
   exists — but the mutation envelope is silent.

3. `get_toolpath_params(0)` reveals the broken reference:

   ```json
   "runtime": {
     "error": "Selected model is missing",
     "stale": false,
     "status": "Error"
   }
   ```

4. `get_toolpath_diagnostics(0)` returns only the three pre-sim
   load.* placeholders. **No diagnostic mentions the missing model.**

5. `get_project_diagnostics()` returns `[]`. **No diagnostic mentions
   the missing model.**

6. The GUI shows a "Selected model missing" banner on the Params tab
   for this toolpath. (User-flagged during the smoke run.)

7. **Confirmed re-test 2026-05-25 (post-F-015 rebuild)**: same
   `add_toolpath(model_id=0)` call after F-015 landed. GUI banner
   appears (user-confirmed in real-time). MCP envelope still returns:

   ```
   warnings: []
   gui_banners: []
   diagnostic_delta: [ only the three load.* placeholders ]
   ```

   F-015's plumbing now correctly carries blocking diagnostics into
   `diagnostic_delta`, `gui_banners`, AND `warnings` when the
   precondition adapter fires (verified by `rest` op without prior
   tool — all three surfaces populated with `precondition.rest_prev_tool_missing`).
   So the diagnostic-envelope plumbing **works**; the gap is that the
   model_id check is in a different code path — the viz-side
   `validate_toolpath` static check rather than the new
   `from_preconditions.rs` adapter or any sibling adapter under
   `crates/rs_cam_core/src/diagnostics/adapters/`.

   This **isolates the fix**: port the viz-side model_id-existence
   check into a new core-side adapter (`from_model_refs.rs` or merge
   into `from_static_checks.rs`) so the unified pipeline that F-015
   built can carry it.

Three things diverge:

- **Validation gap:** `add_toolpath` accepts an invalid `model_id`
  without rejecting or warning. The schema hint
  ("Model ID (raw numeric ID shown in toolpath configs, usually 0 for
  the first model)") nudges agents toward `0`, which is **wrong** for
  any project whose first model has id >= 1 (which is at least the
  `ux_2d_pocket.toml` case).
- **Surface gap:** the runtime's "Selected model is missing" error
  flows through `get_toolpath_params.runtime.error` and the GUI
  banner, but does **not** flow through the unified diagnostic
  stream (`get_toolpath_diagnostics`, `get_project_diagnostics`,
  `gui_banners` in the mutation envelope).
- **Discoverability gap:** an agent following the documented MCP
  workflow (add_toolpath → set params → generate) has no way to know
  the toolpath is wedged until `generate_toolpath` errors. The two
  diagnostic listings the schema advertises as the "recommended view"
  return nothing actionable.

## Why this matters (acceptance-loop framing)

The acceptance-loop principle is that **MCP and GUI must surface the
same signals** so an agent can audit what a user sees. Any banner the
GUI shows must also appear in the unified diagnostic stream. Otherwise
the smoke run cannot detect classes of failure that users hit on
day-one (this exact "Selected model missing" was the round-01 baseline
caveat on `ux_3d_terrain.toml`'s `Rivers (back) (copy)` toolpath —
re-surfaced now on a different template).

This is upstream of F-015 (op-precondition validation): F-015 covers
preconditions checked at add/param-set time. F-023 covers ongoing
runtime errors that the diagnostic stream silently drops on the
floor.

## Acceptance test

1. **Unit/integration test (validation gap)**:
   Build a project with one model whose id is non-zero. Call the
   session-level equivalent of `add_toolpath(model_id=0)`. The
   MutationResult must include at least one `Diagnostic` with
   `severity: blocking` (or equivalent) pointing at the invalid
   model_id, OR the call must return an error rather than `ok: true`.

2. **Unit/integration test (surface gap)**:
   After producing a toolpath whose `runtime.error = "Selected model
   is missing"`, call `get_toolpath_diagnostics(index)`. The returned
   `Vec<Diagnostic>` must contain at least one entry whose `message`
   or `id` references the missing model. Same for
   `get_project_diagnostics()`.

3. **Smoke verification**: re-run AS001 on round-NN. The
   `add_toolpath` envelope must surface the invalid `model_id=0`
   reference (warning or error), and the unified diagnostic stream
   must reflect any runtime model-binding failure.

## Files

- **`add_toolpath` MCP handler**: likely `crates/rs_cam_mcp/src/server.rs`
  (search for `add_toolpath`).
- **Session-level add path** (where the actual validation should live):
  `crates/rs_cam_core/src/session/mutation.rs` (or wherever
  toolpath-add lives).
- **Diagnostic stream builders**:
  - `get_toolpath_diagnostics` handler
  - `get_project_diagnostics` handler
  - `crates/rs_cam_core/src/diagnostics/diagnose.rs` (orchestrator
    after F-015 landed)
- **Runtime-error surface**: wherever `Toolpath::runtime.error` is
  set ("Selected model is missing" string) — promote it to a
  `Diagnostic` with a stable ID (e.g. `runtime.model_missing`).

## Fix shape

Two-part:

1. **Validation at add/mutate time** — `add_toolpath` and any model_id
   mutation should validate that the referenced model exists in the
   current project. If not, reject (returning a `Diagnostic` with
   `severity: blocking`) OR accept with a `blocking` diagnostic in the
   envelope (matching F-015's approach for preconditions).

2. **Runtime-error surfacing** — when a toolpath's `runtime.error`
   is non-null, the diagnostic stream must emit a `runtime.*`-id
   diagnostic with `severity: blocking` and `state: current`. This is
   a small adapter in `diagnostics/adapters/` — sibling to F-015's
   `from_preconditions.rs`, e.g. `from_runtime_errors.rs`.

## Risk

S–M. The runtime-error surface is a pure adapter add (sibling pattern
to F-015). The add-time validation may need a touch in
`session/mutation.rs` plus the MCP server handler — small but spans
two crates.

## Notes

- The misleading schema docstring on `add_toolpath`'s `model_id`
  param should be fixed in the same PR: drop "usually 0 for the
  first model" — IDs are assigned by the project, not 0-indexed.
- Likely-related class of issues: any toolpath field whose validity
  depends on cross-references (`tool_id`, `prev_tool_id`, surface
  model refs) probably has the same surface gap. The adapter should
  generalise across `runtime.error` reasons, not just model_missing.
- This finding was reported by the user during the round-02 smoke
  run when the GUI banner appeared but the MCP probes returned
  silence.
