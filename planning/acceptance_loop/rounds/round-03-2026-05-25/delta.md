# Round-03 delta — 2026-05-25

**Vs baseline:** `round-02-2026-05-25/delta.md`
**Auditor:** autonomous Claude session
**Smoke type:** targeted probes (3 add_toolpath calls), not a full case sweep

## What changed since round-02

Three landings + one reframe + one new finding:

| Action | Finding | Commit | Notes |
|---|---|---|---|
| Landed | F-015 | `ba3f84d` | Smoke-verified late-round-02 (rest + project_curve preconditions emit blocking diagnostics through `diagnostic_delta` + `gui_banners` + `warnings`) |
| Landed | F-014 | `7410c58` | Pure docs cleanup — annotated stale dispatch-dup audit docs; archived `SERVICE_LAYER_OWNERSHIP_AUDIT.md` |
| Landed | F-023 | `dbe2c5d` | `from_model_refs` core-side adapter + MCP docstring fix |
| Reframed | F-002 | (no commit) | Round-02 reopen was wrong — see "Reframe" below |
| Opened | F-024 | (none) | Root cause of deflection over-fire isolated |

## Verifications

### F-023 — smoke-verified

Probe (post-rebuild): `add_toolpath(setup_index=0, operation_type="pocket", tool_index=0, model_id=0)` on `ux_2d_pocket.toml` (which has one model with `id: 1`).

Result envelope now carries on all three surfaces:

- `diagnostic_delta`: `{ id: "ref.model_missing", severity: "blocking", source: "static_validation", category: "geometry", message: "Selected model is missing — toolpath references model_id 0 but no loaded model has that id. Call \`inspect_model\` and set the toolpath's model to a valid \`id\` from the response." }`
- `gui_banners`: same message at `severity: blocking`
- `warnings`: same message at `level: blocking`

`get_toolpath_diagnostics(0)` also carries the diagnostic. The
**actionable fix message** is a nice touch — tells the agent exactly
which MCP call to make next.

### F-015 — smoke-verified late-round-02

Two precondition probes confirmed (rest without prior tool;
project_curve against mesh-only model). Both emit blocking
`precondition.*` diagnostics through all three surfaces. Drill
precondition is covered by F-015's existing integration tests.

## Reframe — F-002

The round-02 reopen of F-002 (claim: "linear-kinematics path stale
relative to fixed engagement-vector path") was **wrong**. The F-002
implementer reproduced the bug end-to-end and found:

- F-002's original split into `axial_engagement_mm` +
  `plunge_descent_mm` landed correctly in commit `072c11a` at
  `dexel_stock/simulation.rs:452-457`.
- Per-sample `axial_engagement_mm = 12.0` on **all four kinematics
  classes** (linear/helix/arc/plunge) for a 2mm-DOC pocket pass.
- Round-02's observed asymmetry (linear 12.0 vs arc/helix 0.0 in
  per-kinematics summary) is an artefact of
  `KinematicsAccumulator::observe`'s `if !sample.in_transit_span`
  filter. Arc/helix samples in pocket entry are transit-tagged;
  linear clearing cuts are not.

The deflection over-fire has a **different root cause**: a Z-frame
mismatch in the dexel stock grid. Tracked as F-024.

**The implementer refused to land a band-aid framing.** This is the
healthy operating mode for the loop — the implementer contract
empowered them to bounce a wrong framing back to the auditor with
file-and-line evidence rather than landing the wrong fix.

## New finding — F-024

[F-024](../../findings/F-024-dexel-stock-z-frame-mismatch.md) — Z-frame
mismatch in dexel stock grid for identity setups.

Quick summary (full details in finding file):

- `effective_stock_bbox` always roots local grid at `(0,0,0)..(stock_x, stock_y, stock_z)`.
- Identity setups (`face_up=Top`, `z_rotation=Deg0`) set
  `local_to_global = None` → no transform applied to toolpath before
  stamping.
- Toolpath emits Z in stock-top-relative frame (cut_depth = -2 etc).
- Cutter at Z=-2 is "below" the entire grid ray `(0, 10.5)`, so
  `removed_here` = full ray length → `axial_engagement_mm = 12.0`.
- Three candidate fix sites: `compute/simulate.rs:386`,
  `compute/transform.rs:293`, `session/compute.rs:1140-1146`.

This is the load-bearing fix for the deflection bar. AS001/AS002/AS013/AS015
all show `deflection.peak_mm` = round-01 value byte-identical because
the over-fire numbers are computed from "full stock height engaged".

## Acceptance bars status (unchanged from round-02)

No bar shifted in round-03; the smoke verifications were probes, not
case re-runs. The deflection bar remains blocked on F-024.

## Open queue (snapshot)

Top of queue: F-024 (high sev, M–L). F-020 (high sev, M, needs fixture).
Full queue in `STATE.md`.

## Implementer summary (loop health note)

- F-014: 1 PR landed, S effort matched estimate.
- F-023: 1 PR landed, S–M effort matched estimate.
- F-015: 1 PR landed prior round, S–M plumbing matched estimate.
- F-002 pickup: returned with blocker after ~22 min of investigation
  (correctly refused band-aid; rewrote the framing as F-024).

3 landings + 1 health-saving bounce-back across the round.
