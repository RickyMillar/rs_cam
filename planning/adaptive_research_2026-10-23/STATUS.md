# Adaptive research — status

Lead session: Fable, started 2026-10-23 10:41 NZDT. Next gate: **G0**.

## Baseline pin

- Phase 0 code state: HEAD `61a8d4b4` (the plan commit `df6de926` adds
  only planning files). The GUI is built from a detached worktree at this
  commit: `.claude/worktrees/adaptive-head`, own target dir.
- Reason: the main checkout holds a peer session's uncommitted work
  (335 files, started 09:48 NZDT, the underived-defaults removal, S11
  tests). The release binary in `target/release` was built at 01:19,
  before commit `44af62dc` changed crate code, so it is not HEAD.
- Each fixture page records the binary path, its mtime and the HEAD hash.

## Open questions for the operator

1. `TROCHOID_CAP_MULT_3D = 1.6` (`adaptive3d/clearing.rs`) and
   `default_trochoid_cap_mult()` (`compute/operation_configs.rs`) cite the
   June cap sweep, which lives only behind tag
   `planning-pre-purge-2026-09-17`. The peer's default-removal work may
   delete this default; ContourSpiral then refuses until the dial is set.
   The baseline uses 1.6 and says so.
2. Fixture name in PLAN.md: the iso-scallop project is
   `planning/multitool_2026-08-23/wanaka200_iso_scallop.toml` (no `_mt1_`).

## What ran

| Item | State | Output |
|---|---|---|
| Plan + prompt commit | done `df6de926` | |
| HEAD worktree + GUI build | done 10:59 NZDT; installed over `target/release/rs_cam_gui` by rename (mtime 1792706351.82, sha256 51cbb713…a438); `gui_restart` verified | |
| R1 FreeCAD Adaptive | done; FreeCAD commit be69b03e, LGPL-2.1, author Kresimir Tusek 2018 (no Freesteel lineage in the source); lead spot-check: cpp:4064-4066 retract penalty, cpp:3880 climb-only fail, cpp:1856 stepOverScaled = R·s, rs_cam path.rs:999 6R link range — all match; accepted | `R1_freecad_adaptive.md` |
| R2 Freesteel | done; source FOUND through the Wayback CDX index (live site 403/503); 19 posts read; lead spot-check: 5 quoted phrases found in 2 archived posts (2007-01-15, 2013-11-20) — accepted | `R2_freesteel.md` |
| R3 papers | done; Held 2014 full + 2009 secondary, Bieterman by abstract + patent, Sethian 1999 + Zhang 2005, libactp headers; lead spot-check: Held–de Lorenzo §1.3 quote verified by fetch; L7 label (iso-field solver) was wrong, sent back, fixed; accepted | `R3_papers.md` |
| Phase 4 refusal count | done: 0 of 47 wanaka200 small-tool islands accepted (45 LevelNotSingleLoop, 2 NotStarShaped); run 7.1 s at 0.4 mm cell; disk controls (two disks × four cusp radii) all accepted. **Arm stops per PLAN Phase 4 step 2.** Deviations: level sets from a grid EDT + marching squares (C1's closed form is disk-only); harness `tests/spiral_finish_compact_wanaka_g3.rs` built in the worktree, copied to main and committed unbuilt there | `PHASE4_refusals.md` |
| Phase 0 baseline pages | running (one agent, sequential on the GUI) | `phase0/` |
| Synthesis | done (lead) | `SYNTHESIS.md` |

## Disk

98 % full at start (19 GB free). The lead pruned 22 192 older-hash
duplicate files in `target/debug/deps` under the lane lock (10:53 NZDT);
63 GB free after. Peers were told. The next candidate is
`.claude/worktrees/demo-build/target` (6.2 GB, commit of 2026-09-25);
not taken without a ruling.

## Findings for G0 that are not about the spiral

- **`planning/multitool_2026-08-23/wanaka200_mt1.toml` does not load at HEAD.**
  The loader refuses the Drill toolpath's retired key
  `setups[0].toolpaths[2].operation.params.depth` (Phase 4 agent). The
  Phase 0 wanaka200 column needs a corrected fixture file first (save-as a
  new name; the fixture itself is not changed).
- Lead session hit its usage limit at ~11:30 NZDT with Phase 0 still
  running (one agent on the GUI; pages land in `phase0/`). Check
  `phase0/PHASE0_NOTES.md` and the three pages before the operator sees them:
  images from HEAD (gui pid 946291, binary mtime 1792706351.82), hard bars
  pass/fail, V1–V3 marked.
- The worktree `.claude/worktrees/adaptive-head` (own target, ~4 GB) stays
  until Phase 0 ends; delete it with its target dir afterwards.
