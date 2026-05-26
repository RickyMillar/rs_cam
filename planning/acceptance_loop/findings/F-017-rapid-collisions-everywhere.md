# F-017 — Rapid collisions in nearly every operation

- **Stage:** sim (root cause is path-planning, not the collision detector)
- **Severity:** medium → **low** (after F-024 reframe; see below)
- **Status:** **reframed in round-04** — most of the original collision
  counts were a symptom of F-024's Z-frame mismatch, NOT a real
  path-planning bug. Residual scope is much smaller.
- **First found in:** round-01 (2026-05-24)
- **Effort:** S–M (was L; reduced after F-024)
- **Linked PRs:** —
- **Source audits:** smoke-run evidence + round-04 F-024 verification

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

## Round-04 reframe (2026-05-25)

The F-002 hunch above was **correct in direction, wrong in scope**.
The root cause turned out to be F-024 (dexel stock grid Z-frame
mismatch). Round-04 smoke verified F-024 on AS001 and watched
rapid_collision_count drop from **36 → 0** alongside the deflection
fix. The collision-detector was telling the truth all along: its
"uncleared stock" view of the dexel grid was at the wrong Z relative
to the toolpath, so rapids that were actually safely above stock
read as cutting through it.

**Revised collision-counts (round-04, AS001 origin_z=-12, post-F-024)**:

| Case | Op | Round-01 | Round-04 v3 | Notes |
|---|---|---:|---:|---|
| AS001 | pocket | 36 | **0** | F-024 fix |
| AS013 | adaptive3d | 1041 → 844 (lift-bridge fix) | 844 | unchanged — origin_z=0, F-024 doesn't apply |

**The remaining scope of F-017 is the AS013-class problem only.**
Origin_z=-12 cases (most of the 2D op suite) are likely already
clean — needs full sweep to confirm in round-05.

If round-05's full sweep confirms all origin_z=-12 cases are 0–5
collisions and only origin_z=0 cases (terrain) show >100, then:

1. F-017 is closed at the auto_from_model scope; the residual
   AS013 problem becomes either:
   - Real path-planning bug in adaptive3d's lift/bridge specifically
     for auto-from-model stock (smaller scope), OR
   - A second F-024-class bug for auto_from_model stock (which IS
     `origin_z=0` but where the model extends ABOVE stock top, so
     the cutter may sometimes be above the dexel grid in the
     opposite direction).
2. F-017 acceptance test should be re-cut: the round-level bar
   becomes "AS013 ≤ 20 collisions" and the unit test should
   reproduce on terrain_small.stl specifically.

**Action for round-05:** when smoke runs, snapshot rapid_collision_count
on AS001-AS018. Use to either close F-017 or re-rank as the AS013-only
investigation it actually is.
