# F-007 — `DrillConfig::set_plunge_rate` is a silent no-op

- **Stage:** suggest
- **Severity:** high
- **Status:** landed
- **First found in:** round-01 (2026-05-24)
- **Effort:** S (<50 LOC)
- **Linked PRs:** —
- **Source audits:** suggest-pipeline

## Evidence

`crates/rs_cam_core/src/compute/operation_configs.rs:1165` —
`DrillConfig::set_plunge_rate` is a no-op. Whatever `suggest_params`
or `apply_feeds_result_to_op` writes is silently dropped. The
historic "plunge 527 > feed 385" default (caught and warned about by
the new `geom.plunge_exceeds_feed` rule) returns trivially on any
drill no matter what the suggest layer says.

This is also why F-003's invariant 1 (`plunge ≤ feed`) cannot be
enforced on drill until this fix lands.

## Smoke evidence

- AS006 rest baseline showed plunge=527 > feed=385 default values
  surfaced as a `geom.plunge_exceeds_feed` warning at add-toolpath
  time
- AS011 drill defaulted to `feed_rate=300` with no plunge_rate field
  exposed, so the silent no-op was not directly observable from MCP
  but is confirmed by reading the code

## Acceptance test

1. **Unit test**: `set_toolpath_param(index=N, param="plunge_rate",
   value=300.0)` on a drill toolpath. Generate G-code. Assert the
   per-peck plunge feed in the output is 300.0, not the hardcoded
   default.
2. **Unit test**: a drill toolpath where Suggest writes
   `plunge_rate = 250` must not generate G-code with `plunge_rate = 500`.
3. **Regression**: assert `DrillConfig::set_plunge_rate(250.0)`
   followed by `serde::to_value` shows `plunge_rate: 250.0` in the
   serialized form.

## Files

- `crates/rs_cam_core/src/compute/operation_configs.rs:1165` and the
  `DrillConfig` struct definition above it

## Fix shape

Read surrounding code first — the no-op may be historical because
drill peck cycles use a per-peck feed encoded elsewhere. Two options:

**Option A:** Add a `plunge_rate: f64` field to `DrillConfig`. The
`set_plunge_rate` impl writes it. Generate consumes it.

**Option B:** Unify with the existing per-peck encoding. The set_*
method becomes a real setter of the canonical feed.

Pick Option B if the per-peck encoding genuinely is the single source
of truth; otherwise Option A.

## Risk

Tiny. Single file. Worst case is needing a follow-up to migrate
existing TOMLs that had the wrong default.

## Notes

- **Blocks F-003's invariant 1 on drill.** If F-003 ships first, note
  the gap in the invariant test.
