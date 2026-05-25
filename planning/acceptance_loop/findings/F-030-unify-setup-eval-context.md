# F-030 — Unify SetupEvalContext across the 5 stock-frame entry points

- **Stage:** substrate / architecture
- **Severity:** medium (no user-facing bug; closes a class of bug that's surfaced 4× in this loop)
- **Status:** landed (commit `ba9a8fd`, 2026-05-26) — refactor landed
  against the 5/7 deflection bar with the user's explicit precondition
  override (the AS013/AS015 residual is sim-side, tracked as F-031,
  orthogonal to F-030's frame-handling scope). Auditor verification
  still required through round-08 MCP smoke; **F-030 does not claim
  `verified` until the smoke runs.**
- **First found in:** round-07 (2026-05-25), pattern recognised after F-028 needed a viz-path follow-up that mirrored the F-024 three-rebuild saga
- **Effort:** L (multi-file refactor across two crates, but well-bounded by an existing acceptance-bar suite that serves as regression)
- **Linked PRs:** `ba9a8fd`
- **Source audits:** round-04 delta (F-024 three-rebuild saga) + round-07 delta (F-028 viz-path follow-up)

## The pattern

Stock-frame + setup-transform decisions are **independently re-derived at 5 separate sites**. Every bug in this family has surfaced as "the fix lands at one site, but the production code path takes a different site that's still broken." Each finding's smoke-verification has exposed the next missing site:

| Site | File:fn | Touched by which finding |
|---|---|---|
| 1 (load) | `crates/rs_cam_core/src/session/project_file.rs::build_session_from_project` | F-026 |
| 2 (compute, direct API) | `crates/rs_cam_core/src/session/compute.rs::compute` | F-024 site 1, F-028 site 1 |
| 3 (viz worker sim) | `crates/rs_cam_viz/src/compute/worker/execute/mod.rs::build_core_simulation_request` | F-024 site 2 |
| 4 (viz controller sim) | `crates/rs_cam_viz/src/controller/events/simulation.rs::build_world_stock_bbox` | F-024 site 3 |
| 5 (viz controller gen) | `crates/rs_cam_viz/src/controller/events/compute.rs::submit_toolpath_compute` | F-028 viz follow-up (commit `c9e203d`) |

Each site:
- Independently checks `face_up == Top && z_rotation == Deg0` for identity-setup detection
- Independently constructs `stock_bbox` (world vs local zero-rooted)
- Independently constructs `HeightContext` / `transform_setup` / `local_to_global`
- Independently decides whether to honor `stock.auto_from_model`

The compiler can't tell you when sites diverge. The acceptance-loop's smoke does — but only after a `/mcp` rebuild + a full sweep.

## Why this matters

This loop has burned four rebuild cycles on what is effectively the same architectural bug:

- Round-04: F-024 needed three commits (sites 1, 2, 3) across three MCP rebuilds before AS001 deflection went 0.374 → 0.076.
- Round-05/06: F-026 needed a fifth site (load path) discovered only after F-024-style hotspot probing on auto_from_model projects.
- Round-07: F-028 needed two more passes — site 1 (heights resolution in core) + site 5 (transform_setup gating in viz controller) — because the viz controller's generation path bypassed F-028's site-1 fix.

Three findings (F-025 non-identity setups, F-027 boundary cohort, F-029 interior cohort) plus the long tail of "what about face_up=Bottom / z_rotation != Deg0 / flipped stock for two-sided work" are all downstream of the same architectural fault. Each will repeat the multi-site dance unless the fault is closed.

**This is the canonical "consolidate, don't patch" case** (per the project's standing preference at `/home/ricky/.claude-personal/projects/-home-ricky-personal-repos-rs-cam/memory/feedback_consolidate_dont_patch.md`).

## Proposed abstraction

Introduce `SetupEvalContext` (or extend the existing `HeightContext`) as the canonical carrier of every frame-derived value for a single (project, setup_index) tuple. Constructed once. Threaded through every entry point.

```rust
pub struct SetupEvalContext<'a> {
    /// World-frame stock bbox (single source of truth; honors stock.origin + auto_from_model)
    pub stock_world_bbox: Aabb3,

    /// Identity-or-not. None = identity (face_up=Top, z_rotation=Deg0).
    pub local_to_global: Option<Affine3>,

    /// HeightContext derived from stock_world_bbox + transform.
    pub heights_ctx: HeightContext,

    /// Dexel-grid bounds (today: == stock_world_bbox; future: enclose model peaks per F-026/F-027 family).
    pub dexel_world_bbox: Aabb3,

    /// Pre-resolved transform_setup (None for identity; matches F-028 viz follow-up filter).
    pub transform_setup: Option<&'a SetupTransform>,

    /// Pre-resolved stock material, workholding, padding — anything operations currently
    /// re-fetch from `session.stock_config()` ad hoc.
    pub stock_meta: StockMeta,
}
```

Every code path becomes:

```rust
let ctx = SetupEvalContext::build(&session, setup_index)?;
operation.evaluate(&ctx, ...)
```

Five entry points collapse to one. Operations consume `&SetupEvalContext`; they no longer touch raw stock dimensions, origins, or setup-rotation enums.

## Side effects (wins beyond this finding)

- **F-029 (interior-cell parity)** likely simplifies. Adaptive3d's planner currently bounds its `material_stock` by `mesh.bbox + tool_radius`; the simulator uses world stock bbox. If both consume the same `SetupEvalContext.dexel_world_bbox`, the mismatch disappears by construction. F-029 may reduce to "make adaptive3d consume the context" rather than a separate stamping fix.
- **F-025 (non-identity setups, face_up=Bottom etc.)** reduces to "centralize `local_to_global` construction in the context builder". F-028's `.filter(|s| s.needs_transform())` is half of this fix; the other half is making sure every site honors that filter.
- **F-017 (rapid collisions)** has historically been a symptom of frame mismatches. After F-030 it should not need its own investigation.
- **Future ops** can't accidentally break the frame contract — they take a `&SetupEvalContext` parameter, not raw fields, so omitting a check is a compile error.

## Constraints

- **Land F-029 first.** F-030 is too large to do safely against a moving deflection bar. After F-029, the AS001-AS018 acceptance suite passes — that becomes the regression net.
- **Use the existing acceptance suite as regression.** The smoke `cases_agent_smoke.csv` + the per-finding tests in `crates/rs_cam_core/tests/*_f0{24,26,27,28}*.rs` and `crates/rs_cam_viz/src/{compute/worker,controller}/tests.rs` are exactly the right shape for refactor regression. Add a new test only if the refactor exposes a frame case not yet covered.
- **One PR.** Atomic. Don't split into "introduce context, then migrate sites" — half-migrated state is more confusing than the current state.
- **Preserve F-024's safe_z behavior.** F-024 has a subtle "effective_safe_z reads from local bbox to floor at a conservatively-higher value, never below world stock top" that must survive the refactor. Cite: F-028 commit `e48d7df` body, paragraph on `effective_safe_z`.
- **Coordinate with the user on MCP.** The user rebuilds MCP between PRs; the refactor agent should not block on that. Cargo tests are the agent's signal; smoke verification is the auditor's job in a later round.

## Acceptance test

The existing `_f024.rs`, `_f026.rs`, `_f027.rs`, `_f028.rs` test files must continue to pass. **No new failure of any existing acceptance test.** Add tests if the refactor exposes a new contract — but the regression suite is already strong.

Round-09 (after F-030 lands) smoke run must show:
- AS001-AS018 verdicts byte-identical (or within rounding noise) to the post-F-029 round.
- The 5 sites listed above either deleted, or reduced to thin shims that delegate to `SetupEvalContext::build`.

## Risk

L. But bounded:

- **Mitigating**: the acceptance suite is in good shape after F-027 + F-028. Every site has at least one regression test that exercises it.
- **Risk**: there may be additional sites I haven't enumerated (CLI sim path? export gate? GCode emission?). The refactor agent should `grep` for `stock_bbox`, `effective_stock_bbox`, `face_up == FaceUp::Top`, `z_rotation == Rotation::Deg0`, `local_to_global`, `transform_setup`, and `auto_from_model` across the workspace at the start to enumerate sites.
- **Risk**: F-029's fix shape may not be known at refactor-start time. If F-029 turns out to need a frame-related change, do F-029 first, then refactor.

## Files (start by enumerating)

The 5 sites in the table above are the known sites. Run these greps at the start to confirm completeness:

```
rg "effective_stock_bbox|stock_bbox\(\)" --type rust crates/
rg "face_up.*=.*FaceUp::Top|z_rotation.*=.*Rotation::Deg0" --type rust crates/
rg "local_to_global|transform_setup|needs_transform" --type rust crates/
rg "auto_from_model" --type rust crates/
rg "HeightContext|HeightsConfig::resolve" --type rust crates/
```

## Notes

- The `controller/events/compute.rs:244-246` pattern (referenced in F-028's commit) and the new `.filter(|s| s.needs_transform())` (F-028 viz follow-up) are both seeds for the abstraction — the refactor can use them as starting points.
- The viz controller's `build_world_stock_bbox` helper (F-024 site 3) and the F-028 follow-up's `CapturingBackend` test in `controller/tests.rs` show how to test the controller path without driving a real worker thread; reuse the pattern.
- This finding deliberately does NOT include the architecture diagram. The refactor agent should produce one as part of the PR description, after enumerating sites.
- The fresh-session handoff brief lives at
  `planning/acceptance_loop/handoff_prompts/F-030-architecture-brief.md`.
