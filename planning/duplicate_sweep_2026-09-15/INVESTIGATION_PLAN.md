# Duplicate Sweep Investigation Plan — 2026-09-15

## What this is

A semantic duplicate sweep (Qwen3-Embedding-0.6B over the socraticode index,
`scripts/duplicate_sweep.py`, threshold cosine >= 0.92, production `src/`
chunks only — tests excluded) produced **59 cross-file duplicate-candidate
pairs** across `rs_cam_core`, `rs_cam_viz` and `rs_cam_cli`. Raw evidence:
`candidates.json` (this directory).

Similarity is **not proof**. This plan turns the candidates into verified
findings, one investigation per work item, so a cleanup plan can be written
from evidence rather than guesses.

Two pairs were already verified by hand and anchor the plan as real:
`ProjectFixtureSection` (core `session/project_file.rs` vs viz `io/project.rs`,
drifted `id` types) and `slugify` (byte-identical in two viz files).

## Investigation protocol (same for every item)

Each investigation is **read-only** and answers, in this order:

1. **Reproduce the similarity.** Read both regions at the given lines
   (line numbers are from the 2026-09-15 index — they may have drifted a few
   lines; re-locate by content if needed). Record a literal diff summary.
2. **Establish the use cases.** For each copy: who calls it
   (`codebase_symbol` / `codebase_impact` in socraticode, or `rg`), what
   behavior it must preserve, and which sentry/regression tests pin that
   behavior (check `crates/rs_cam_core/tests/` and crate CLAUDE.md rules).
3. **Decide the verdict** (exactly one):
   - `TRUE_DUP` — same purpose, same behavior, copy-paste. Merging is safe.
   - `DRIFTED_DUP` — same purpose, behavior has diverged. **Name the
     authoritative side and the drift** (which call sites depend on which
     behavior). This is the dangerous category — the drift is usually a
     bug in one of the two.
   - `SIBLING` — intentional parallel structure (e.g. 2D/3D variants of the
     same operation, per-surface UI of the same concept). Not debt; at most
     deserves a shared-harness extraction. Say why it is intentional.
   - `FALSE_POSITIVE` — similar shape only, different purpose. Say why.
4. **Propose the cleanup unit** (do not implement): where the single home
   should be (core module / shared helper / macro / UI widget), what breaks
   (sentries, snapshots, MCP wire types), risk level (low/med/high), and the
   smallest test that would prove the merge.

### Output format

One file per item: `planning/duplicate_sweep_2026-09-15/findings/I##_<slug>.md`:

```
# I## — <title>
Verdict: TRUE_DUP | DRIFTED_DUP | SIBLING | FALSE_POSITIVE
## Evidence
- <copies, lines, callers, sentries — with paths>
## Drift / differences
- <only for DRIFTED_DUP: authoritative side + what diverged>
## Proposed cleanup
- home: <path>  risk: <low|med|high>  proof test: <smallest test>
```

Keep each finding under ~60 lines. Evidence over prose.

## Work items

Grouped by file-family; an agent takes one item. Priority orders risk of
silent behavior divergence, not similarity score.

### I01 — Project-file I/O layer (P0, architectural) 
`rs_cam_core/src/session/project_file.rs` ↔ `rs_cam_viz/src/io/project.rs` ↔
`rs_cam_viz/src/controller/io.rs` ↔ `rs_cam_core/src/io.rs` — 9+ pairs, top
0.9828. Viz `io/project.rs` is a live parallel implementation of project
serialization (`load_project` called at `controller/io.rs:528`). Known drift:
`ProjectFixtureSection.id` is `Option<usize>` (core) vs `Option<FixtureId>` +
`skip_serializing_if` (viz); `infer_model_kind` exists twice with different
implementations. Also examine: model-path relink (G-MODELRELINK F4.3 in
core), legacy loaders (viz `load_legacy_project`), and whether the viz layer
violates "no parallel one-off flows" (workspace CLAUDE.md). Answer:
**which layer is authoritative, and what does it take for viz to delegate
to core?** Check MCP wire snapshots for project formats
(`rs_cam_viz/tests/snapshots/`).

### I02 — tier/reach twin family (P1)
`tier_map_cache.rs` ↔ `reach_map_cache.rs` (0.9535, plus a 3-way link to
`geom_cache.rs` 0.9384) and `tier_map.rs` ↔ `reach_map.rs` (0.9386, 0.9360).
Memo-cache scaffolding (stats/atomics/table/clear) is copy-paste; the maps
themselves may be SIBLING. Also `tier_map_cache.rs L257-278` ↔
`reach_map_cache.rs L198-219`. Consider a generic keyed-memo module as the
cleanup unit.

### I03 — dexel twins (P1)
`dexel_mesh.rs` ↔ `dexel_mesh_mc.rs` (0.9628, 0.9417, plus 2 more below
threshold). Midpoint-circle variant vs base dexel mesh — likely SIBLING with
shared harness. Verify what `mc` changes and whether the shared parts can
live in the base.

### I04 — trace twins (P1)
`debug_trace.rs` ↔ `semantic_trace.rs` (4 pairs, 0.9499 top) and both link to
`simulation_cut.rs` trace regions (0.9336). Determine whether these are two
front-ends over one underlying trace format, or independent formats that
happen to look alike.

### I05 — simulation state twins (P1)
`rs_cam_viz/src/state/simulation.rs` ↔ `rs_cam_core/src/compute/simulate.rs`
(3 pairs: 0.9367, 0.9247) and `state/simulation.rs L2919-3018` ↔
`simulation_cut.rs L1954-2053`. Likely viz mirroring core simulation results
for display — check whether the mirror is derived (good) or recomputed
(drift risk). Also `state/job.rs` ↔ `session/mod.rs` (0.9367, 0.9239) and
`session/mutation.rs` ↔ `state/job.rs` (0.9336): check against the
"apply(Command)" contract — viz state should not reimplement session
mutations.

### I06 — export surfaces (P2)
`rs_cam_viz/src/app/export.rs` ↔ `ui/export_wizard.rs` (`slugify`
byte-identical, 0.9757) and `rs_cam_viz/src/io/export.rs L344-360` ↔
`rs_cam_core/src/gcode/mod.rs L801-819` (0.9416 — possibly export-specific
G-code formatting duplicated in viz; if real, it belongs in core/export).

### I07 — optimize UI twins (P2)
`ui/optimize_modal.rs` ↔ `ui/optimize_project.rs` (2 pairs, 0.9527). Two
surfaces of the same optimizer workflow. Likely SIBLING + widget extraction.

### I08 — ops/config twins (P2)
- `project_curve.rs` ↔ `pencil.rs` L705-746 (0.9216) and
  `project_curve.rs L28-39` ↔ `compute/operation_configs.rs L1583-1596`
  (0.9434) — per-op config plumbing duplicated per operation.
- `drill_op.rs L160-182` ↔ `session/mod.rs L942-966` (0.9287).
- `compute/config.rs L1124-1182` ↔ `unified_finish.rs L1007-1035` (0.9517).
- `ramp_finish.rs` ↔ `spiral_finish.rs` (0.9529) — likely SIBLING strategies.
- `adaptive/path.rs` ↔ `adaptive3d/path.rs` (0.9815) — 2D/3D siblings;
  verify and mark, probably no action.

### I09 — feeds/inspection near-matches (P2, likely FALSE_POSITIVE-rich)
- `feeds/efficiency.rs L309-351` ↔ `feeds/force.rs L271-296` (0.9218) —
  caller/callee, flagged by the earlier agent as the intended "one model,
  shared" pattern. Confirm FALSE_POSITIVE/SIBLING and close.
- `feeds/vendor_normalize.rs L125-141` ↔
  `tool_load/optimize/context.rs L152-168` (0.9469).
- `tool_load/optimize/narrative.rs` ↔ `refusal.rs` (0.9271).

### I10 — small confirmed utilities (P2, quick wins)
`slugify` (byte-identical, viz x2 — confirmed TRUE_DUP already; needs only a
home decision), `ui/components/compare.rs` ↔ `ui/feeds/compare.rs` (0.9426),
`render/sim_render.rs` ↔ `render/mesh_render.rs` (0.9355),
`rs_cam_cli/sweep.rs` ↔ `rs_cam_cli/job.rs` (0.9224), benches
`classification.rs` ↔ `hot_paths.rs` (0.9261, bench harness only — lowest
priority).

### I11 — infer_model_kind & model-kind helpers (folded into I01)
(kept in I01; listed separately in candidates.json)

## Execution notes for the agent swarm

- One agent per item, **read-only investigation**. No code changes in this
  phase. `findings/I##_*.md` is the only write.
- Use socraticode (`codebase_symbol`, `codebase_impact`, `codebase_search`)
  first for callers; `rg` only for exact strings. The index is current
  (watcher active).
- Rust-specific: check `Cargo.toml` lint gates still apply to any proposed
  shared module (no `unsafe`, clippy-clean).
- Items I01, I02, I05 have architectural weight — if the verdict implies a
  workspace rule ("core independent of GUI", "apply(Command)", "no parallel
  flows"), say so explicitly in the finding.
- Independent items can run in parallel; I01 is the only one that touches
  persisted formats (project files, MCP snapshots) — flag migration
  concerns for it.

## After the investigations

1. Review pass over all findings (consistency of verdicts, missed pairs).
2. Author `CLEANUP_PLAN.md` from findings: each TRUE_DUP/DRIFTED_DUP becomes
   a work item with home, risk, proof test, sentry list, and sequencing
   (drift fixes first — they are live bug risk; merges second; sibling
   documentation last).
3. Cleanup executes against the repo quality gates (`cargo test -p <crate>
   -q`, clippy `-D warnings`, core sentries) — the cleanup plan should name
   the gate per item.

## Sweep hygiene

- Raw pair evidence: `candidates.json`. Full first sweep (incl. tests):
  the test-side clusters (sentry fixture duplication) are acknowledged but
  deliberately out of scope here; re-run
  `python3 scripts/duplicate_sweep.py` after cleanup to verify the pair
  count drops.
- Rerunning the sweep requires the local socraticode index (Qdrant on
  localhost:16333, collection `codebase_a3510da6358c`).
