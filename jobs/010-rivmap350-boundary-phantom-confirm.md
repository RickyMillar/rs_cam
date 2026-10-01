---
id: 010
state: queued
commit: 7a7c867d
needs_ricky_ok: true
---
## Commands
On master 7a7c867d (contains G-BOUNDARYPHANTOM 7a7c867d: the adaptive3d
planner now stamps only what the boundary clip keeps). Release build, under
the MemoryMax=16G cap as before. Simulation only; do NOT run any G-code on
the machine.

1. rs_cam_cli project <Ricky's rivmap350.toml> --output-dir <dir> --resolution 0.25
   (add --sim-artifact only if the collision list needs it).
2. From summary.json (or the diagnostics), report rapid_collision_count per
   toolpath, and list every remaining rapid collision (toolpath, move index,
   start/end XYZ) for "3D Rough 8" and any other toolpath.
3. Any collision left on "3D Rough 8": re-simulate that op at 0.125 mm and
   say whether it persists.
4. Cycle time of "3D Rough 8" before (job 006/007 numbers) and after.

## Expected
"3D Rough 8": 0 rapid collisions (was 8 at 0.2/0.25/0.125).
Cause found on the small fixture: the planner stamped rings in the band
between the model outline and the containment line (outline inset by the
tool radius); the session clip then deleted them, and later entries rapided
down onto that phantom-cut floor.

## Also, if Ricky agrees
Push rivmap350.toml and its models to a branch (e.g. fixtures-rivmap350,
folder planning/fixtures/rivmap350/, relative model paths) so the lead can
test against the real project.
