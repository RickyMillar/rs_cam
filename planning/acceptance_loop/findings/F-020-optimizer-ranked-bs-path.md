# F-020 — Optimizer Ranked-outcome BS-stepover path untested + Wanaka concern

- **Stage:** optimize
- **Severity:** high
- **Status:** open (needs test fixture before fix)
- **First found in:** Wanaka review (pre-round-01); confirmed
  untested by round-01 smoke
- **Effort:** M (test fixture + investigation; fix may be S if just
  add scallop gating)
- **Linked PRs:** —
- **Source audits:** Wanaka review + smoke-run evidence

## Evidence

The optimizer has four outcome variants per `OptimizeOutcome`:
`Ranked`, `MarginalSafe`, `TradeOff`, `NoSafeImprovement`, `Skipped`.

Round-01 smoke tested exactly two cases:
- AS011 drill → `Skipped` with reason `steady_state_samples_not_present` ✅
- AS015 scallop → `NoSafeImprovement` with reason
  `deflection_setup_locked` and a gold-standard narrative ✅

The remaining variants are untested. In particular, **`Ranked` was
never observed**.

The earlier Wanaka review (`planning/WANAKA_TOOLPATH_REVIEW.md`)
caught a `Ranked` outcome on 3D Finish 6 that proposed:

- stepover 0.30 mm → 1.00 mm (3.3× coarser)
- 70% cycle-time savings
- all load gates Within
- **no scallop / surface-quality gating** — would silently produce
  a roughed-out surface

This is the "BS path" — optimizer fakes a cycle-time improvement by
coarsening a finish-quality parameter that isn't load-modeled.

Round-01 smoke didn't reach `Ranked` because every finish case had
deflection EXCEEDS from F-002 (the optimizer correctly refused). Once
F-002 lands, deflection will pass and the optimizer should reach
`Ranked` — at which point the BS path is reachable.

## Acceptance test

1. **Fixture (must be in PR)**: build a case under `cases_agent_smoke.csv`
   or a new `cases_optimize_quality.csv` where:
   - finish op (scallop, drop_cutter, horizontal_finish)
   - load gates have headroom (deflection Within, power Within, chipload Within)
   - default stepover is fine-grained
2. **Test**: run optimizer on this case. Assert one of:
   - Returns `NoSafeImprovement` because surface quality regression
     prevents an improvement
   - Returns `Ranked` candidates that ALL preserve scallop_height ≤
     baseline (don't allow coarsening)
   - Returns `TradeOff` candidates that explicitly call out the
     quality regression
3. **Smoke verification (auditor's next round)**: this case must
   pass the test above. A `Ranked` candidate that silently coarsens
   stepover is a fail.

## Files

- `crates/rs_cam_core/src/optimize/` (likely dir — needs locating)
- `crates/rs_cam_core/src/tool_load/verdict.rs` — gate definitions
  that the optimizer scores against
- New: surface-quality gate (scallop_height vs requested quality tier
  from `planning/SUGGEST_SIM_OPTIMIZE_ACCEPTANCE.md`)

## Fix shape

Two-step:

1. **Test fixture first.** Land a test case (or smoke case) that
   reaches `Ranked`. Without the fixture, we can't confirm the bug
   exists in current code (it was observed in older Wanaka project,
   maybe not in current).
2. **If the BS path still exists:** add a surface-quality gate to
   the optimizer's candidate scoring. A finish-op candidate that
   coarsens stepover beyond the quality tier fails the gate.

## Risk

Medium. The fix shape depends on whether the BS path actually still
exists in current code. The recently-shipped MCP overhaul may have
addressed this incidentally — needs re-test.

## Notes

- **Blocked by F-002 verification.** Once F-002 lands and deflection
  stops over-firing, finish ops should reach `Ranked` outcomes for
  the first time in round-01's smoke evidence base.
- **Out of scope:** changing the optimizer's search strategy. Only
  the candidate gating.
- This is **the highest-leverage future audit topic** — the AS015
  refusal narrative shows the optimizer can be exemplary; we need
  to confirm it stays that way when it does propose improvements.
