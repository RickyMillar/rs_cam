---
id: 004
state: done
commit: 5b34e710
started: 2026-09-30T20:21:00Z
finished: 2026-09-30T20:31:42Z
---
## Summary

- Release build (rs_cam_cli + rs_cam_viz, cold release cache): OK, 5 min 44 s.
- Repro (rivmap100_memory_repro.toml --resolution 0.5, MemoryMax=10G):
  - exit 0, wall 4 min 58 s (user 726 s, 246 % CPU)
  - Maximum resident set size: 4 608 064 kB = 4.39 GiB (4.61 GB)
    (cloud: 4.59 GB, 514 s on 4 cores; pre-fix: abort at 9.54 GB)
  - collision_count 0, rapid_collision_count 0, collision_checks_failed 0
  - total_runtime_s 139 788.8 (38.8 h); verdict "WARNING: 82.0% air
    cutting of total runtime" (air 81.97 % of runtime, 85.44 % of cutting)
  - WARN lines: feed optimization skipped on '3D Rough ladder 8 > 2'
    (remaining stock) and 'Scallop Finish' (mesh surface); high air on
    '3D Rough ladder 8 > 2' 41 %, 'Face 5' 70 %.
  - Moves: rough 379 920; scallop 4 734 507 (generated at +158 s).
- target/release/rs_cam_gui is built in the runner worktree.

## Runner notes

- The repro writes diagnostics/ into the working directory: 7.7 GB, of
  which simulation.json is 8 175 040 462 bytes (the time -v "File system
  outputs" 15 973 080 blocks match). Peak RSS is fixed, but this file is a
  disk cost on every CLI run of a large project. It is kept on the runner's
  PC for now.

Logs on the runner's PC: /tmp/claude-1001/-home-ricky-personal-repos-rs-cam--claude-worktrees-bridge-cse-01X9bPqEnCfUkrbNprBjUs3d/84f1aa02-8de5-5da0-aff8-1fa9722b654e/scratchpad/job004_5b34e710.log, /tmp/claude-1001/-home-ricky-personal-repos-rs-cam--claude-worktrees-bridge-cse-01X9bPqEnCfUkrbNprBjUs3d/84f1aa02-8de5-5da0-aff8-1fa9722b654e/scratchpad/job004_repro.log
