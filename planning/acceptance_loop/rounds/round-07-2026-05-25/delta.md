# Round-07 delta — 2026-05-25

**Vs baseline:** round-06 `rounds/round-06-2026-05-25/delta.md`.

**Smoke type:** F-027 + F-028 verification + F-024 stability check.
Two rebuild cycles in this round: F-028 needed a viz-path follow-up
(`c9e203d`) after an initial regression discovered during F-024
stability verification.

**Auditor:** autonomous Claude session.

**Implementer commits this round:**
- `b1f17fe` + `9821e72` — F-027 (adaptive3d planner stock-XY widening
  + border-clear inhibit)
- `e48d7df` + `bf63d06` — F-028 site 1 (heights.top_z anchored to
  ctx.stock_top_z; face op stock_top_z field; session/compute.rs world
  bbox for identity setups)
- `db1fb69` — F-028 test-tightening + defensive AS001 cross-check
- `c9e203d` + `95c5a94` — F-028 viz-path follow-up (the actual fix for
  the regression below: `submit_toolpath_compute` was passing
  `Some(transform_setup)` even for identity setups, causing toolpath
  generation to emit cuts in local frame while F-024's sim path
  re-interpreted them as world)

## Headline

**F-027 + F-028 verified. F-024 stability verified after one
regression-and-fix cycle.** Three of three round-05 deflection misses
fully resolved on the dimensions F-027/F-028 targeted; **deflection
bar still at 5/7 Within** because AS013 + AS015 carry an F-029
residual (interior-cell parity, opened by F-027 implementer during
landing).

| Case | Round-06 | Round-07 final | Resolved by |
|---|---|---|---|
| AS013 adaptive3d collisions | 2924 | **0** | F-027 |
| AS013 deflection.peak_mm | 0.576 Exceeds | 0.576 Exceeds | (F-029 residual, open) |
| AS015 scallop collisions | 52 | **0** | F-027 |
| AS015 deflection.peak_mm | 0.434 Exceeds | 0.434 Exceeds | (F-029 residual, open) |
| AS004 face collisions | 24 | **0** | F-028 |
| AS004 deflection.peak_mm | 0.243 Exceeds | **0.005 Within** | F-028 (49× reduction) |
| AS004 peak_axial_doc_mm | 11.42 (cmd 0.5) | **0.38** (cmd 0.5) | F-028 |
| AS001 pocket | 0.076 Within (round-05) | **0.076 Within** | F-024 + F-028 viz-path follow-up |
| AS003 profile | 0.076 Within (round-05) | **0.076 Within** | F-024 + F-028 viz-path follow-up |

## The F-028 regression-and-fix cycle

F-028 site 1 (commit `e48d7df`) landed clean cargo tests but the
auditor's MCP smoke on AS001 and AS003 showed `peak_axial = 0` and
96% air-cut — F-024's working setup was broken.

The follow-up agent's initial diagnosis (a fresh-session cargo test on
the same commit) said AS001 was fine. Re-running MCP smoke on a fresh
project load reproduced the regression. Tracing the MCP code path:

| Auditor cargo test path | MCP / GUI path |
|---|---|
| `session.run_simulation()` direct | `RunSimulation` event → controller → worker → core |
| Goes through F-028's site-1 `session/compute.rs::compute` fix | Bypasses site 1; uses viz-side request builders |
| z_level = -2, -4, -6 (world, correct) | z_level = 10, 8, 6 (local, broken) |

Root cause: `AppController::submit_toolpath_compute` always set
`transform_setup = Some(...)` for any setup, identity or not. The
toolpath generator then emitted cuts in local frame. F-024's
identity-setup sim fix (`local_to_global = None`) re-interpreted those
as world frame, putting the cutter above the world stock top.

The viz follow-up (`c9e203d`) added a one-line
`.filter(|s| s.needs_transform())` gate plus a `CapturingBackend`
regression test that drives the production `submit_toolpath_compute`
path and asserts `heights.top_z` + `stock_bbox` in world frame for
identity setups.

**This is the round-04 three-rebuild saga repeating, exactly.** Loop
learning escalated: write F-030 finding to retire the class.

## Verdict by finding

### Verified (closing this round)

| Finding | Resolution |
|---|---|
| **F-027** | AS013 collisions 2924 → 0; AS015 52 → 0. Boundary-cohort outliers eliminated per F-027's acceptance test. Deflection still Exceeds on both, but the surviving samples are now interior cohort (F-029). |
| **F-028** | AS004 deflection 0.243 → 0.005 Within (49× reduction). peak_axial 11.42 → 0.38 (matches commanded 0.5 mm DOC). Required 2 commits (site 1 + viz-path follow-up). F-024 stability preserved on AS001 + AS003 after viz-path follow-up. |

### Opened this round

| Finding | Trigger |
|---|---|
| **F-030** | Pattern recognised after F-028 viz follow-up: stock-frame + setup-transform handling is duplicated across 5 sites. F-024 needed 3 (sites 1-3); F-026 needed 1 (site 4 = load path); F-028 needed 2 (site 1 + new site 5 = viz controller gen). F-025 and F-029 will repeat the dance unless unified. **Architectural refactor**, not point fix; M-L effort; pickup blocked on F-029 + 7/7 deflection bar (the suite is its only regression net). Brief at `handoff_prompts/F-030-architecture-brief.md`. |

### Holding (residual)

| Finding | Status |
|---|---|
| **F-029** | Confirmed for AS013 + AS015. Round-08 implementer pickup is highest priority — closes the deflection bar to 7/7, unblocks F-030. |
| **F-017** | Cannot close until F-029 lands (3D-op collision surface waits on the same mechanism). |
| **F-025** | Untouched. Possibly subsumed by F-030. |

### Acceptance bars status

| Bar | Target | Round-06 | Round-07 | Status |
|---|---:|---|---|---|
| Sim chipload calibration (3D ops) | ≥ 95% | 2/2 | 2/2 stable | stable |
| Sim chipload calibration (2D ops) | ≥ 95% | 5/5 | 4/4 re-verified (AS001, AS003, AS004, AS005 — AS006 rest unmodeled-expected) | stable |
| **Sim deflection calibration** | ≥ 95% | 5/7 Within | **5/7 Within** (AS013 + AS015 still F-029 residual; AS004 + AS001 + AS003 Within) | **F-029 → 7/7** |
| Optimizer refusal correctness | 100% | not re-tested | not re-tested | stable; verify post-F-029 |
| Export gate | 100% | not tested | not tested | open |

## What's next

Round-08 priorities:

1. **F-029 implementer pickup** (top of queue; finding file has 4
   candidate root causes, recommends starting with link-move
   stamping fix).
2. After F-029 lands + MCP rebuild, round-08 smoke must show:
   - AS013 deflection < 0.2 mm Within
   - AS015 deflection < 0.2 mm Within
   - **7/7 Within** on the deflection bar
3. Once 7/7 confirmed, **F-030 architecture refactor unblocked**
   (precondition satisfied; brief at
   `handoff_prompts/F-030-architecture-brief.md`).
4. Full AS007-AS012, AS014, AS016-AS018 sweep deferred again — round
   has already burned context on the regression cycle and F-030
   handoff. Will batch into round-08 alongside F-029 verification.

## Loop process notes

- The F-028 regression cycle is the **second** instance of the round-04
  three-rebuild saga pattern. The implementer-contract update from
  round-04 ("test through the production entry point") is
  necessary-but-not-sufficient — F-028's site-1 test DID go through
  `ProjectSession::run_simulation`, but the MCP and the cargo test
  take different code paths after that. The contract needs sharpening:
  **"production entry point" for MCP-driven features means the
  controller event path, not `session.run_simulation()`**.
- F-030 is the structural fix for this. Loop docs (contract +
  autonomous_auditor.md) should not be further patched — they encode
  process for a duplicated-architecture problem that F-030 retires.
