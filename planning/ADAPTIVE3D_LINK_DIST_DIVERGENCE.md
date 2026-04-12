# adaptive3d: unify link distance constants

**Status:** backlog (surfaced April 2026 during adaptive remediation Package F)

## Problem

Two different link-distance constants in the adaptive3d module, each
with its own hardcoded multiplier:

| Location | Formula | Used by |
|---|---|---|
| `crates/rs_cam_core/src/adaptive3d/path.rs:267` | `max_stay_down_dist.unwrap_or_else(\|\| max(tool_radius*6.0, stepover*6.0))` | ClearZLevelContext.max_link_dist, consumed by AgentSearch via `clear_z_level()` at clearing.rs:1046 |
| `crates/rs_cam_core/src/adaptive3d/clearing.rs:501, 751` | `ctx.tool_radius * 3.0` | ContourParallel and Adaptive strategies' ring-to-ring link check |

**ContourParallel and Adaptive** use `tool_radius * 3.0` (hardcoded at
the call site, ignores `params.max_stay_down_dist`).

**AgentSearch** uses `tool_radius * 6.0` (via `max_link_dist` on
`ClearZLevelContext`, respects `params.max_stay_down_dist` when set).

So the two EDT-based strategies are **2× more conservative** about
linking than AgentSearch, and neither one honors the user-settable
`max_stay_down_dist` parameter that's already on `Adaptive3dParams`.

## Symptom

If a user sets `max_stay_down_dist = 20.0` hoping to reduce rapid
retract-rapid-plunge sequences on a ContourParallel or Adaptive run,
**nothing happens** — the param is silently ignored. Meanwhile the
same setting works for AgentSearch.

Also: Package E scaled `max_link_dist` in path.rs to account for
stepover, but because clearing.rs uses a separate hardcoded constant,
Package E's fix only benefits AgentSearch.

## Discovery

Caught during Package F investigation (F-5 z_blend collision fix).
I noticed the link decision in `clear_z_level_contour_parallel` used
`tool_radius * 3.0` while the equivalent code in `clear_z_level`
(AgentSearch) used the `ClearZLevelContext.max_link_dist` field, and
flagged it as an open follow-up in commit `0958284`.

## Recommended fix

1. Remove the two hardcoded `tool_radius * 3.0` constants from
   `clearing.rs:501` and `clearing.rs:751`.
2. Reference `ctx.max_link_dist` in both sites instead. Since `ctx`
   is the `ClearZLevelContext` and already has the field, this is
   a one-line swap.
3. Verify: the sweep_adaptive3d_* param_sweep fingerprints will
   change at stepover=3 and any variant that hits the
   tool_radius*3 vs tool_radius*6 boundary. Expect the collision
   count to drop for ContourParallel and Adaptive because they'll
   start honoring the larger, path-clearance-gated link distance.

## Open question

Is `tool_radius * 3.0` intentional for EDT strategies (because
contour-to-contour links are fundamentally closer than AgentSearch's
pass-to-pass links)? If so, the constants should still be documented
and derive from stepover like path.rs does post-Package E. If not,
they're just inconsistent magic numbers.

## Estimated effort

- ~1 hour: swap the two call sites, re-run param_sweep, accept the
  fingerprint delta with before/after MCP probe on Fixture 2.
- Optional: expose `max_stay_down_dist` as a user-facing parameter
  in the GUI/MCP config layer so users can override it from all
  three strategies, not just AgentSearch.

## Tests

- Add a sweep case for `max_stay_down_dist` under
  `sweep_adaptive3d_*` so the parameter's effect is regression-tested.

## Related

- Commit `0958284` — Package F z_blend fix (flags the divergence)
- Commit `0caeba9` — Package E max_link_dist stepover scaling (only affects the AgentSearch path)
- Review: F-5, F-6 in `planning/adaptive_review_2026-04.md`
