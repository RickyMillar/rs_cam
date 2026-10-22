# Adaptive research — status

Lead session: Fable, started 2026-10-23 10:41 NZDT. **At gate G0** (all Phase 0, Phase 1 and Phase 4 inputs ready; no Phase 2 code started).

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
| Phase 0 baseline pages | DONE 13:00 NZDT: 9 columns (3 fixtures × 3 strategies), 15 rough ops, every generate finished, no refusal. Lead reviewed all three pages: HEAD stamps present, bars pass/fail/not-measured with instrument names, V1–V3 marked, N1–N9 named, N5 spurs captured mid-cycle. PNGs quantised by the lead (pngquant) to keep the planning tree small | `phase0/rivmap100.html`, `phase0/corne.html`, `phase0/wanaka200.html`, `phase0/PHASE0_NOTES.md` |
| Synthesis | done (lead) | `SYNTHESIS.md` |

## Disk

98 % full at start (19 GB free). The lead pruned 22 192 older-hash
duplicate files in `target/debug/deps` under the lane lock (10:53 NZDT);
63 GB free after. Peers were told. The next candidate is
`.claude/worktrees/demo-build/target` (6.2 GB, commit of 2026-09-25);
not taken without a ruling.

## G0 findings from the rivmap100 baseline (lead read of the page)

- **Two hard bars have no instrument through MCP for a 3D Adaptive op.**
  Bar 2 (coverage): every per-toolpath coverage field is null; the
  coverage code (`metrology/union_coverage.rs`) is test-support only.
  Bar 3 (gouge on emitted moves): `ModelGougeRule::Exempt` for 3D Adaptive
  in `compute/catalog/registry.rs`; the cut trace has only air_cut and
  low_engagement issues. Both bars read NOT MEASURED on every column.
  Operator decision needed: build the two instruments before Phase 2, or
  accept the bars as not measured for G1 (2D renders).
- **V1, V2, V3 all PRESENT on HEAD, on all three strategies.** The form has
  changed since June: V1 is entry circles (helix + concentric loops) at
  residue spots, not only in corners; V2 is overlapping entry circles that
  cut air, not corner-blend arcs. New: N1 "bead chain" (a line of helix
  entries 1–1.5 mm apart, each a full circle), N2 a fan of long fed links
  from one corner (ContourParallel), N3 entry stacking (AgentSearch).
- **Entries dominate time:** entry_s 2 364 of 3 263 s (ContourParallel),
  6 100 of 7 085 s (ContourSpiral). The spiral's 1 061 keep-down links
  with nested helix/plunge descents are the residue clean-up.
- **Peak axial DOC above the commanded 2.0 mm:** 3.0 mm (AgentSearch),
  2.5 mm (ContourSpiral) by `narrate_toolpath`. Not a hard bar as written;
  flagged for the operator.
- Bar 1 (0 rapid + 0 holder collisions) and Bar 4 (generation completes)
  PASS on all three rivmap100 columns.

## Phase 0 hard-bar table (from the pages; bars are pass/fail, no ranking)

| Fixture | Strategy | Bar 1 collisions | Bar 2 coverage | Bar 3a mesh gouge | Bar 3b stock-side plough/over-cut | Bar 4 generation |
|---|---|---|---|---|---|---|
| rivmap100 | ContourParallel | PASS 0/0 | NOT MEASURED | NOT MEASURED | 0 reported | PASS |
| rivmap100 | AgentSearch | PASS 0/0 | NOT MEASURED | NOT MEASURED | 0 reported | PASS |
| rivmap100 | ContourSpiral | PASS 0/0 | NOT MEASURED | NOT MEASURED | 0 reported | PASS |
| Corne | ContourParallel | PASS 0/0 | NOT MEASURED | NOT MEASURED | FAIL `link_cuts_stock` 1 + 3 | PASS |
| Corne | AgentSearch | PASS 0/0 | NOT MEASURED | NOT MEASURED | FAIL `crosses_standing_material` 4 (peak 15 mm) | PASS |
| Corne | ContourSpiral | PASS 0/0 | NOT MEASURED | NOT MEASURED | 0 reported | PASS |
| wanaka200 | ContourParallel | PASS 0/0 | NOT MEASURED | NOT MEASURED | FAIL `link_cuts_stock` CRITICAL (a link removes 11.26 mm) | PASS |
| wanaka200 | AgentSearch | PASS 0/0 | NOT MEASURED | NOT MEASURED | 0 reported | PASS |
| wanaka200 | ContourSpiral | PASS 0/0 | NOT MEASURED | NOT MEASURED | 0 reported | PASS |

V1–V3 on HEAD: all three present on every rivmap100 column; on Corne and
wanaka200 ContourParallel is free of V1 and V2 (V3 present on wanaka
front); AgentSearch and ContourSpiral show V3 everywhere and V1 as seam
loop chains (spiral). New defects named on the pages: N1 bead chains of
helix entries, N2 link fan from one corner, N3 entry stacking, N5 standing
spurs (AgentSearch only: Corne up to 16.5 mm, wanaka back 15.6 mm;
ContourParallel none; ContourSpiral nubs ≤ 3.4 mm), N7 band outside the
Corne wall left standing with holes (AgentSearch, ContourSpiral), N8 1 726
retracts on the wanaka back rough (ContourParallel, mostly 0.5 mm hops),
N9 narrow valleys left higher (rivmap100 AgentSearch, ContourSpiral).
Also: peak axial DOC above the commanded value on AgentSearch and
ContourSpiral (wanaka back 10.92 vs 5.46 mm). Corne ContourParallel cycle
time not reproducible (2 022 s then 1 564 s; the feed modulation reads the
previous simulation).

## Operator observation 2026-10-23 ~12:20 (Corne case, live GUI)

"As it's milling down it leaves tall spurs of material. These could snap
and cause issues; prominent ones about 7 min in." Recorded as new defect
N5 "standing spurs". The Phase 0 agent captures them mid-cycle on every
Corne column and checks the other fixtures. A spur that snaps is a safety
item, not a look item: it needs a measure (thin standing material between
passes) before Phase 2 candidates are judged.

## Findings for G0 that are not about the spiral

- **`planning/multitool_2026-08-23/wanaka200_mt1.toml` does not load at HEAD.**
  The loader refuses the Drill toolpath's retired key
  `setups[0].toolpaths[2].operation.params.depth` (Phase 4 agent). The
  Phase 0 wanaka200 column needs a corrected fixture file first (save-as a
  new name; the fixture itself is not changed).
- Lead session paused ~11:30–12:10 NZDT (usage limit); Phase 0 resumed and
  finished. The GUI restarted at 11:56 on the same binary (gui pid 84659).
- The worktree `.claude/worktrees/adaptive-head` (own target, ~4 GB) stays
  until Phase 0 ends; delete it with its target dir afterwards.
