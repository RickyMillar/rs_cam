# Round-02 delta — partial audit (test-level only)

**Date:** 2026-05-25
**Auditor:** post-compact Claude session
**Status:** PARTIAL — live MCP smoke deferred to round-03

## What landed since round-01 baseline

| Commit | Finding(s) | Verified at |
|---|---|---|
| 072c11a | F-001, F-002, F-003, F-007, F-008, F-013 | Commit inspection + test names referenced in commit body |
| 2a287c1 | F-016 | `cargo test --test drill_material_plumbing_f016` → 3/3 PASS; `drill_metrics::tests::chip_welding_threshold_per_material_family` → PASS |

Additionally **F-012** closes as a duplicate of F-003 (the
`feeds_result_for_toolpath` workholding=Medium hardcode lived inside
the singletons F-003 collapsed).

## What is NOT verified yet

The acceptance bars these findings claim to move are bar-level signals
that only the live smoke acceptance suite can confirm:

| Bar | Pre-fix (round-01) | Predicted by fix | Verified? |
|---|---|---|---|
| Sim chipload calibration (2D ops) | 0/7 Unmodeled | ≥ 95% modeled | **NO** — needs live MCP smoke |
| Sim deflection calibration | 4/13 (over-fires) | ≥ 95% | **NO** — needs live MCP smoke |
| AS011 hardwood chip-welding threshold | 8.0 (softwood) | per-material (5.0 hardwood) | unit-test verified; smoke confirms field surface |
| Suggest LUT consistency vs banner | inconsistent (3 singletons) | consistent (1 singleton) | smoke needed |

Live MCP was disconnected at audit time, so round-02 is held open for
re-run when `rs-cam` MCP is back. Round-03 will combine that smoke
verification with whatever F-015 (or other next-round-02 pickups)
land.

## Queue changes

- **F-022** unblocked (was "deferred — depends on F-003"). Re-ranked
  into open queue at low severity / M effort.
- **F-012** closed (subsumed by F-003).
- Acceptance bars table in STATE.md NOT refreshed yet — refresh
  requires the live smoke rerun.

## Next implementer recommended pickup

- **F-015** — Op-precondition static validation (rest, drill,
  project_curve). Medium severity / M effort. Unblocks AS006 next
  smoke. Independent of everything currently open.

Alternate parallel pickups: **F-018** (template regen, S) or
**F-021** (Suggest All paint-thrash, S) — both genuinely independent.

## Auditor notes

- The unification batch is large (one commit covers six findings +
  loop scaffolding). Future implementers should land one finding per
  PR per `implementer_contract.md` Step 3; the batch was tolerated
  because it landed alongside the loop bootstrap itself.
- Live MCP smoke should be the first thing the next auditor does once
  MCP reconnects. Re-running AS001–AS017 against round-01 baseline
  will produce the round-02 verified delta.
