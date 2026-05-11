# F.1 RCA — Optimizer predicted verdict diverges from live re-sim

**Status:** Root cause identified. Fix design pending — scope is larger
than the roadmap anticipated (architectural cache reconciliation, not
just dressup-span boundary drift). See "Fix options" below for the
candidate directions; pick before opening the implementation PR.

**Repro:** wanaka_full_tuned.toml (`/home/ricky/Downloads/wanaka100/`),
TP10 3D Rough 6. Optimizer stage-2 candidate predicts
`deflection.peak_mm = 0.157` (Within). After Apply + manual Regenerate
+ Run Simulation in the GUI, live `peak_mm = 0.270` (Exceeds 0.200
bound). 70% prediction error.

---

## Verified cause: two desynchronized result caches

The codebase maintains two independent caches of per-toolpath compute
results that hold the same conceptual data but diverge after a
mutation:

- **`session.results: HashMap<usize, ToolpathComputeResult>`** —
  authoritative source for core-side reads (gcode export, MCP, load
  report). Lives on `ProjectSession`. Written only by
  `ProjectSession::generate_toolpath` at
  `crates/rs_cam_core/src/session/compute.rs:616`.
- **`gui.toolpath_rt[id].result: Option<ToolpathResult>`** — what
  every GUI surface (sim panels, properties, export wizard) reads
  from. Written by the GUI's threaded compute backend at
  `crates/rs_cam_viz/src/controller/events/compute.rs:340`.

The optimizer runs entirely in core via `evaluate_candidate`
(`crates/rs_cam_core/src/tool_load/optimize/candidate.rs:305-397`), so
its apply → regen → sim → verdict cycle reads and writes
`session.results` exclusively. Every cache it touches is fresh.

The live GUI path does not. `apply_optimize_candidate`
(`crates/rs_cam_viz/src/controller/events/mod.rs:332-430`) calls
`session.apply_toolpath_param_snapshot(...)`, which at
`crates/rs_cam_core/src/session/mutation.rs:98` invokes
`self.results.remove(&index)` — wiping the cached result for the
modified toolpath. The viz-side regen then writes to
`gui.toolpath_rt[id].result` (`compute.rs:340`) and **does not write
back to `session.results`**. There is no code path in viz that
populates `session.results` after a GUI-driven compute completes
(confirmed: `rg "session.results.insert" crates/rs_cam_viz/` returns
no hits).

After Apply + GUI Regenerate + GUI Run Simulation:
- `gui.toolpath_rt[id].result.annotated.spans` — populated and fresh.
- `session.results[idx]` — empty (still cleared from Apply).

`project_load_report` at `crates/rs_cam_core/src/gcode/mod.rs:228-230`
looks up spans via:

```rust
let spans: Option<&[Span]> = project
    .get_result(idx)
    .map(|r| r.annotated.spans.as_slice());
```

`project.get_result(idx)` reads `session.results`. For the just-applied
toolpath, that returns `None`, so `spans = None` is passed into the
deflection gate.

Inside `deflection::evaluate`
(`crates/rs_cam_core/src/tool_load/deflection.rs:143`):

```rust
let span_lookup = spans.map(SpanLookup::new);
// ...
if !super::locality::is_steady_state_for_gate(s, span_lookup.as_ref()) {
    // route to entry-spike track
}
```

When `span_lookup = None`, `is_steady_state_for_gate`
(`crates/rs_cam_core/src/tool_load/locality.rs:122-128`) returns
`true` for every sample — including high-force Entry-transient
samples that on the optimizer side were correctly routed to
`entry_peak_delta_mm`. Those transient samples now contaminate
`peak_delta_mm` directly, inflating the reported peak.

This is the **primary cause** of the 0.157 → 0.270 divergence.

## Secondary contributor: resolution mismatch

The optimizer hardcodes the simulation resolution for stage-2
evaluation at `policy.rs:602`:

```
refined_resolution_mm.value = 0.5
```

passed through `candidate.rs:446`:

```rust
search_policy().stages.refined_resolution_mm.value
```

into `SimulationOptions { resolution: 0.5, auto_resolution: false, ... }`.

The GUI defaults at `crates/rs_cam_viz/src/state/simulation.rs:595-596`
are `resolution: 0.25mm, auto_resolution: true`. With auto enabled the
runtime resolution is `min_tool_radius / 5.0` clamped `[0.02, 0.5]mm`
(`auto_resolution_for_tools` at `simulation.rs:309-333`). For TP10's
6mm flat EM that's `3.0 / 5.0 = 0.6` → clamps to `0.5` — same as the
optimizer. So for this specific tool the resolution does match. The
mismatch only bites smaller tooling (e.g. a 3mm end mill: optimizer
0.5mm, GUI 0.3mm — different per-sample dexel data).

Treat this as a latent secondary issue; not the proximate cause for
TP10 specifically but worth fixing in the same PR to prevent future
small-tool divergences.

## Verified ruled out

The roadmap's prior ruled-out list still holds:

- **Resolution.** Verified above for this specific TP — both at 0.5mm.
- **Sample attribution.** `s.toolpath_id == toolpath_id` filter at
  `deflection.rs:148` is identical for both paths.
- **Generation non-determinism.** Dressup pipeline is deterministic
  (no HashMap iteration, no RNG, no time-based ordering in
  `adaptive3d/clearing.rs`, `tsp.rs`, `dressup.rs`).
- **Cross-setup cascade.** Confirmed not relevant — span lookup
  failure is local to the modified toolpath.

## Fix options

Pick one before opening the implementation PR. Each has different
blast radius:

### Option A — GUI regen writes back to `session.results`

The GUI's threaded compute backend's `on_compute_result` callback
(`compute.rs:340`) currently stores into `gui.toolpath_rt`. Extend it
to also call `session.insert_result(idx, computed)` (new method on
`ProjectSession`). Both caches stay in sync.

**Pros:** Single fix point, restores invariant that `session.results`
is always populated for generated toolpaths. Fixes any other reader
that depends on `session.results` (gcode export already does — would
be incidentally fixed for any post-Apply export path).

**Cons:** Touches the GUI compute callback, requires new `ProjectSession`
mutation method, and means `session.results` now lifecycles through
viz code which CLAUDE.md guardrails warn against ("keep core library
independent from GUI concerns"). Mitigated by exposing the insert as
a typed core API.

### Option B — `project_load_report` falls back to span data on the trace

Each `SimulationSample` already carries `span_path`. When
`get_result(idx)` returns `None`, build a synthetic `SpanLookup` from
the trace's samples for that toolpath_id.

**Pros:** No viz/core coupling. Self-contained in core.

**Cons:** `span_path` is a flat ID encoding; reconstructing
`SpanLookup` (which exposes ancestry + kind) requires either
embedding more span metadata in samples or a fallback that gives less
precise classification. The Entry-spike routing logic depends on
ancestry (`is_steady_state_for_gate` checks parents), so a
reconstructed lookup may be a downgrade.

### Option C — Don't delete `session.results[idx]` on `apply_toolpath_param_snapshot`

Keep the stale result in place until the next `generate_toolpath`
overwrites it. Mark it stale via a flag.

**Pros:** Minimal change. Avoids the empty-window between Apply and
regen.

**Cons:** Stale result is dangerous — gcode export reads
`session.results` and would emit code for the pre-Apply params if a
user exports without regenerating. Currently the `.remove()` is a
safety: empty result forces a regen before export. Inverting this
trades one footgun for another. Would need export to gate on a
`fresh: bool` flag.

### Option D — Deflection gate uses `gui.toolpath_rt` via the trace

Same as B but at the gate level. `deflection::evaluate` already
receives `spans: Option<&[Span]>`. Restructure the caller (live path)
to source spans from `gui.toolpath_rt[id].result.annotated.spans`
when `session.results` is empty. This means `project_load_report`
needs a different signature for the live path vs the core/export
path.

**Pros:** No core/viz coupling — viz constructs the spans before
calling project_load_report.

**Cons:** Requires either two variants of `project_load_report` or a
new entry function that takes spans as an explicit map. The live
path's load report callers (`compute.rs:682`, `simulation.rs:658`,
`preflight.rs:153`) all need updating.

**Recommendation pending user input.** Option A is the cleanest
invariant restoration; Option D is the most respectful of the
core/viz separation guardrail. Option B is the most local but risks
classification accuracy. Option C is the smallest diff but trades
problems.

## Test plan

Repro test should:
1. Load `wanaka_full_tuned.toml` (or a synthetic minimal fixture that
   reproduces the same condition — 3D rough with a Waterline cleanup
   dressup where the deflection peak comes from an Entry-classified
   span).
2. Run `optimize_toolpath` on TP10, capture the stage-2 first-safe
   candidate's predicted `DeflectionVerdict::Within { peak_mm }`.
3. Apply via the GUI controller path (`apply_optimize_candidate`).
4. Drive GUI regen + sim through the threaded backend (or call the
   viz compute helpers directly).
5. Read the live verdict via `project_load_report(&session,
   Some(trace))`.
6. Assert the verdicts agree within a small tolerance.

Today this test should **FAIL** — the live verdict reports
`Exceeds`, the optimizer reports `Within`. Once the cache
reconciliation fix lands, the test should pass.

Test location candidate:
`crates/rs_cam_viz/tests/` — needs viz-side controller access to
exercise the `gui.toolpath_rt` path realistically. A core-only test
can't reproduce the bug because the bug lives at the
`session.results` ↔ `gui.toolpath_rt` boundary.

## Recommended PR sequencing change

Given the architectural scope:

- **PR 7 (this RCA):** RCA doc + failing repro test. No fix.
- **PR 7.1 (new):** Pick fix option and implement. Probably ~1-2 days
  given the cache-sync touchpoints; the original roadmap estimate of
  ~1-2 days was for instrumentation only.
- **PR 8 (F.3 unchanged):** Deflection gate lift-bridge exclusion —
  still needed regardless of the cache fix, because the lift-bridge
  problem (single-sample 6mm DOC in Waterline cleanup) trips the
  gate independently.
- **PR 9 (F.2 auto-verify):** Can be calibrated once F.1 fix lands,
  since divergences will then reflect real problems vs noise.

## Files referenced (verified against HEAD on master @ f62aa0b)

- `crates/rs_cam_core/src/tool_load/optimize/candidate.rs:305-397` —
  `evaluate_candidate`
- `crates/rs_cam_core/src/tool_load/optimize/policy.rs:602` —
  hardcoded `refined_resolution_mm = 0.5`
- `crates/rs_cam_core/src/session/mutation.rs:98` —
  `results.remove(&index)` inside `apply_toolpath_param_snapshot`
- `crates/rs_cam_core/src/session/compute.rs:616` — sole
  `session.results.insert` in core
- `crates/rs_cam_core/src/gcode/mod.rs:228-230` —
  `project.get_result(idx)` span lookup
- `crates/rs_cam_core/src/tool_load/deflection.rs:143-173` —
  `span_lookup` and entry-spike routing
- `crates/rs_cam_core/src/tool_load/locality.rs:122-128` —
  `is_steady_state_for_gate` defaults to `true` when `span_lookup = None`
- `crates/rs_cam_viz/src/controller/events/compute.rs:340` —
  `rt.result = Some(computed)` (only writes to `gui.toolpath_rt`)
- `crates/rs_cam_viz/src/controller/events/compute.rs:682` —
  reconciliation calls `project_load_report` against `session.results`
- `crates/rs_cam_viz/src/state/simulation.rs:595-596` — GUI default
  sim resolution / auto_resolution
