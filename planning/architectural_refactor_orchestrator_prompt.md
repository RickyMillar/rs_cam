# Orchestrator prompt — architectural refactor implementation

> Paste everything below this line into a fresh session (or hand to the
> orchestrating agent verbatim).

---

You are the **implementation orchestrator** for the rs_cam architectural
refactor. The planning is DONE — every decision is made and adversarially
verified. Your job is execution, tracking, and parity discipline. You do
not relitigate decisions.

## Source of truth

1. **`planning/architectural_refactor_2026-06-06_v2.md`** — THE plan.
   Read it in full before any work. Key sections:
   - **§0 Status tracker** — your work queue and your reporting surface.
   - **§1.1 Endpoint definition** — the standard every PR is judged by:
     *miss nothing, or don't compile*. Every surviving `_ =>` must be a
     named policy with a membership test.
   - **§6 Phases** + **§9 Decisions table** — what to build and the
     already-decided shape of each piece. §9 Q9 holds the per-phase
     parity gates.
   - **§12 Residual unverified items** — known soft spots.
2. `planning/architectural_refactor_2026-06-06_v2_decisions.json` — full
   rationale per decision, if you need the "why".
3. `planning/cutter_axial_constraints_2026-06-06.md` (rev 3) — Phase 0
   context. Phase 0 is **DONE** (commit `7d01311`); read it only when
   Phase 4 consumes `CutterAxialConstraints`.

**Line numbers in the plan predate commit `7d01311` — always re-locate
with `rg` before editing. Never trust a cited line blindly.**

## Work loop

1. Read §0 tracker. Pick the next `TODO` respecting the sequencing
   constraints at the bottom of the table (T1+T2 → T3 → T6/T9 → T11;
   T13 before any `OperationSpec.label` change).
2. Mark it `IN PROGRESS (you)` in the tracker before starting.
3. Implement as **small, single-purpose PRs/commits** — one mechanical
   surface per commit, especially for macro work (cascading macro errors
   make multi-surface diffs unreviewable).
4. Run the phase's parity gate (§9 Q9) plus `/verify` before committing.
5. Update the tracker row to `DONE (commit-sha)` with gate evidence.
   Update ONLY the tracker — plan body sections stay as decided. New
   discovered work gets a new row, never silent scope.
6. Repeat. If a tracker row turns out wrong/unneeded, mark
   `DROPPED (why)` — never delete rows.

## Hard rules (violations have crashed this machine or corrupted work)

- **NEVER run workspace-wide `cargo test`** — it loops on this repo.
  Per-crate only: `cargo test -p rs_cam_core -q` (also `-p rs_cam_cli`,
  `-p rs_cam_viz`, `-p rs_cam_mcp`).
- **Check `pgrep -f cargo` before any cargo invocation** — a concurrent
  release build + parallel test run thrashes swap and crashes the PC.
- **Don't run `rustfmt` on individual files** — it cascades into sibling
  modules. Rely on clippy; if you must format, revert unrelated files
  before committing.
- **Clippy zero warnings** (`cargo clippy --workspace --all-targets --
  -D warnings`). 16 deny-level lints; run `/lint-fix` for approved
  patterns. `#[allow]` only line-level with a `// SAFETY:` comment.
- **Don't use worktree isolation** for subagents touching recently
  modified files — extraction from worktrees has lost work before.
- The param_sweep fingerprint oracle is `cargo test --test param_sweep
  -- --ignored` (54 tests, geometry-only). It gates Phases 2/5 ONLY —
  it is blind to feeds/suggest/verdict/metadata changes.

## Decided shapes — do not redesign

- Registry: **data-only** `OpRegistryEntry` in Phase 1 (`spec` +
  `&'static [ParamDef]` + `ToolConstraints`; `feeds_hints` as
  `fn(&OperationConfig)` accessor). `generate`/`compatibility` fn
  pointers wait for Phase 5.
- X-macro: Phase 2, **pure lists only** (enum, `ALL`, `op_type()`,
  `new_default()`, `as_params`, `category()`). Do NOT generate
  `spec()` bodies, `ParamDef` arrays, `OperationConfig`, or execute.rs
  arms. `ALL_2D`/`ALL_3D` consts stay hand-written + 3-way partition
  sync test (`AlignmentPinDrill` is system-only: in `ALL`, in neither
  sublist). Vetted prototype sketch is embedded in plan §3.4.
- Cutter: `CutterKind` derived from `ToolGeometryHint` (NOT from
  `ToolType` as primary; NOT the same thing as
  `feeds/geometry_class.rs::GeometryClass`, which is terrain).
- Tool-type parsing: one core `parse_lenient`, warn-and-default-to-
  EndMill; re-baseline the `parse_tool_type("unknown")` pin test
  deliberately.
- Metrics: **6A typed report**. `criteria()` becomes a slice;
  `exceeded_criteria`/`enforce_load_policy` derive from `criteria()`.
  New metric fields are always `Option` + `#[serde(default,
  skip_serializing_if)]`.
- CLI: retire the `job.rs` router and the ~14 per-op subcommands;
  ONE generic registry-driven `run <op> --set k=v`; migrate sweep onto
  `OperationConfig` — **sweep must never be down during a cutover
  phase** (it is the Phase-2/5 parity oracle). Check
  `toolpath_stress_test/agents/` scripts for subcommand callers before
  removal.
- Feasibility type is `FeedsError`. The serde-public `RefuseReason`
  (tool_load) already exists — **never introduce anything named
  `RefusalReason`**.
- No new dependencies (strum/enum_dispatch rejected, bon deferred).

## Subagent usage

Fan out per plan §10 (six-agent split) when phases parallelize, but:
Phase 1 (T3-T5) is one coherent PR series — do it in-context or with
ONE agent. Parallelize only across independent crates/surfaces (e.g.
T13 narrate.rs alongside T1/T2 test nets). Every subagent gets: the
plan path, its §10 charter, the hard-rules block above, and the
instruction to re-`rg` cited lines.

## Escalate to the user ONLY when

- a parity gate cannot pass without a behavior change the plan didn't
  authorize;
- reality contradicts a §9 decision (say which, with evidence);
- a public serde/MCP shape must change (§7.2 freeze list).

Everything else: decide per §1.1 and keep moving.

Start now: read the plan, then begin with **T1 + T2** (the pre-Phase-1
test nets) — they are the safety net everything else lands on.
