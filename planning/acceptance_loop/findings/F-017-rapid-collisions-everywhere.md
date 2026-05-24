# F-017 — Rapid collisions in nearly every operation

- **Stage:** sim (root cause is path-planning, not the collision detector)
- **Severity:** medium
- **Status:** open — separate path-planning investigation
- **First found in:** round-01 (2026-05-24)
- **Effort:** L (multi-area investigation)
- **Linked PRs:** —
- **Source audits:** smoke-run evidence

## Evidence

Per-op `rapid_collision_count` from round-01 baseline:

| Case | Op | Rapid collisions |
|---|---|---:|
| AS001 | pocket | 36 |
| AS002 | adaptive (2D) | 70 |
| AS003 | profile | 1 |
| AS004 | face | 0 |
| AS005 | zigzag | 60 |
| AS007 | trace | 3 |
| AS008 | v_carve | 0 (own); 3 inherited from prior |
| AS009 | chamfer | 0 (own); 2 inherited |
| AS011 | drill | 0 |
| AS013 | adaptive3d | **1041** |
| AS014 | drop_cutter | 0 |
| AS015 | scallop | 52 |
| AS017 | horizontal_finish | 164 |

The collision detector clearly works — drill / face / drop_cutter
report 0 cleanly. But path generators for adaptive3d, horizontal_finish,
zigzag, and 2D adaptive are producing rapid moves that intersect
uncleared stock. AS013 with 1041 collisions on a single 207K-move
toolpath is severe.

## Acceptance test

1. **Round-level**: smoke verification — adaptive3d on terrain_small.stl
   produces ≤ 20 rapid collisions (down from 1041). horizontal_finish
   on stepped_block produces ≤ 10 (down from 164).
2. **Unit-level (where feasible)**: identify a single representative
   rapid-collision case in adaptive3d, add a regression test that
   reproduces, fix, test passes.

## Files

Likely starting points (needs investigation):

- `crates/rs_cam_core/src/adaptive3d/` — lift / bridge logic
- `crates/rs_cam_core/src/adaptive3d/clearing.rs` — recently modified
- `crates/rs_cam_core/src/adaptive3d/search.rs` — recently modified
- `crates/rs_cam_core/src/horizontal_finish.rs` — likely similar
- `planning/AGENTSEARCH_INVESTIGATION_LOG.md` — prior investigation
  notes on lift/bridge

## Fix shape

Investigation finding, not a single-line fix. Likely needs:

1. Reproduce a single collision in a unit test
2. Identify whether the lift function is bridging across previously-
   uncleared stock (memory hint:
   `peak_axial_doc_mm` reads ~12mm on lateral moves over uncleared
   stock — same root cause as F-002, maybe?)
3. Fix the lift / bridge logic to clear before transit OR force a
   retract to safe Z

## Risk

L. Multi-file investigation in performance-sensitive code. Likely
ties to known lift/bridge issues already tracked.

## Notes

- **Possibly related to F-002.** The same `peak_axial_doc_mm = 12.0`
  observation that drove the deflection false-positive may indicate
  rapids are passing over uncleared stock. If F-002's split lands
  first, this finding may become easier to diagnose.
- **Defer to a later round** unless the unification PRs are
  complete. The investigation depth is greater than the rest of the
  open queue.
- **Investigate in tandem with F-002 verification.** If F-002 fixes
  the measurement but the rapids are still going through stock,
  that's evidence of a real generator bug.
