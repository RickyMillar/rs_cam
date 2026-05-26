# F-025 — Z-frame mismatch on non-identity setups (face_up=Bottom etc.)

- **Stage:** substrate
- **Severity:** medium (smoke hasn't reproduced; theoretical from F-024 implementer)
- **Status:** open — stub finding, low priority until smoke surfaces it
- **First found in:** round-03 (2026-05-25), as a side-observation by F-024 implementer
- **Effort:** S–M (likely same shape as F-024 fix, opposite direction)
- **Linked PRs:** —
- **Source audits:** F-024 implementer's investigation note

## Evidence

F-024 fixed identity setups (`face_up=Top`, `z_rotation=Deg0`) by
returning `None` for both `local_stock_bbox` and `local_to_global`,
letting the existing fallback at `compute/simulate.rs:386` use the
world-frame `request.stock_bbox`.

The F-024 implementer noted during their investigation:

> "while investigating I noticed `transform_toolpath` for non-identity
> face_ups (e.g. `face_up=Bottom`) likely produces toolpath Z outside
> the local stock bbox too (cuts at local Z=-2 → global Z=stock_z+2,
> above the zero-rooted grid), but this is out of F-024's scope
> (which specifically targets identity setups) and the smoke suite
> hasn't called it out as a separate bar miss. Left as a future
> finding if it surfaces."

In other words: for non-identity setups, `transform_toolpath` rotates/
mirrors the toolpath into the local frame, but the local stock bbox
is still rooted at `(0,0,0)` while the cut-into-stock semantic
(commanded DOC = 2 mm) may not map cleanly to "stamped below the
local grid origin". The exact wrong-direction is opposite to F-024's
(cutter above the grid rather than below) but the same class of bug.

## When to revisit

- A smoke case using `face_up=Bottom` or `z_rotation != Deg0` (e.g.
  flipped-stock work, two-sided machining) lands in the acceptance
  matrix and triggers the same "axial engagement reads bigger than
  commanded DOC" symptom F-024 fixed for identity setups.
- A user reports deflection over-fire on flipped-stock work.

Currently the smoke matrix (`cases_agent_smoke.csv`) doesn't include
any flipped-stock cases, so this is a future-finding stub.

## Acceptance test (placeholder)

Same shape as F-024's:

1. Build a pocket toolpath via `ProjectSession` with `face_up=Bottom`
   (or `z_rotation=Deg90`) on a flipped stock setup. Assert per-sample
   `axial_engagement_mm <= commanded DOC + margin`.
2. Run the deflection gate. Assert `peak_mm < 0.2` on a 2 mm-DOC pass.

## Files

Same files as F-024:
- `crates/rs_cam_core/src/session/compute.rs` (the local_to_global
  decision)
- `crates/rs_cam_core/src/compute/transform.rs` (`effective_stock_bbox`)
- `crates/rs_cam_core/src/compute/simulate.rs:386` (fallback path)

## Fix shape (speculative)

The F-024 fix made the identity-setup case use the world bbox via
fallback. For non-identity setups, the world bbox still needs to be
applied through the inverse of the setup transform — i.e. either
build `local_stock_bbox` as `inverse(local_to_global) * request.stock_bbox`,
or apply the transform after stamping. The implementer can pick after
reproducing.

## Notes

- Low priority — no user-facing symptom currently. File a real fix
  PR if a smoke case (or user report) demonstrates the bug.
- F-024 implementer's full investigation context is in their agent
  output (commit `d82bd4d` author log).
