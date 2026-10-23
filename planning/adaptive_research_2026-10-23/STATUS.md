# Adaptive research — status

Lead session: Fable, started 2026-10-23 10:41 NZDT. **G0 PASSED 2026-10-23 13:30** with rulings below. Next: Phase 1.5 instruments, then Phase 2.

## Phase 1.5 + Phase 2 start (2026-10-23 13:45)

Branch `adaptive-instruments` in worktree `.claude/worktrees/adaptive-head`
(HEAD a23df1f8, own target). Agents edit there with no cargo; one verifier
runs the worktree lane script. The lead commits by pathspec on the branch
and cherry-picks onto master after the peer's default-removal commit lands
(it edits `compute/operation_configs.rs` widely; rebase on it).

Facts checked before briefing: `metrology::union_coverage` is LIVE (only
its evidence test is a gated target); `ToolpathStats::untouched_material_mm2`
is the scallop-cascade measure, not a rough measure, so the rough's coverage
is the union audit per op checkpoint; `ModelGougeRule::Exempt` on 3D ops
dates from UX-7b (6d5a5423, 2026-10-22), whose scope was 2.5D ops; the
Surface rule samples EMITTED moves at ≤ D/4 through the drop-cutter.

| Agent | Scope | Owns (files) |
|---|---|---|
| A coverage+thinstand | G-ROUGHCOV + G-THINSTAND: union audit per rough checkpoint against model + stock_to_leave, islands with height and min width, into SimulationTriage findings | `metrology/union_coverage.rs`, `stock/sim_triage.rs`, `session/compute/simulation.rs` (wiring), new sentry |
| B gouge | G-ROUGHGOUGE: Surface rule with the stock_to_leave allowance for 3D Rough (and other 3D ops that leave stock), on emitted moves; registry row change LAST | `compute/model_gouge.rs`, `compute/catalog/registry.rs` (one row), new sentry with an injected gouge |
| C candidate (a) | Phase 2 2D: geodesic field from the entry with islands as obstacles, wraps as level sets, one continuous path, behind a research flag; render beside the 2D baselines on `ux_2d_star` / `ux_2d_pocket` | new `adaptive/research_geodesic.rs` (or similar), a render harness; no edit to shipped paths |
| B result (14:20) | DONE, lead-reviewed by path: new `ModelGougeRule::Allowance { fix }`; limit = drop-cutter CL Z + (stock_to_leave_axial − chord_tolerance(tolerance, segment_merge_tolerance)), both read from the config the planner reads (no new constant; `OperationConfig::model_gouge_limit_offset_mm`); REG_ADAPTIVE3D row flipped; probe unchanged (real cutter − MODEL_GOUGE_TOL_MM); measured on the exported moves at generation; sentry `a_rough_cuts_no_deeper_than_its_leave_g_roughgouge` (4 tests). Deviation from the brief accepted: the Z tolerance is the op's own chord tolerance, not the 0.02 mm CAD rounding, because the RDP simplification may sit a chord that far below a draped point. Not built yet. | `P15_B_gouge.md` (worktree) |
| A result (14:35) | DONE (first pass): `metrology` is gated behind `test-support` at lib.rs (lead's "live" fact was wrong at the crate root), so the column walk and island flood moved to ungated `stock/standing_islands.rs`; the union audit calls it. Rough target = model + stock_to_leave_axial where there is model, mesh-bbox floor + leave where there is none (hole cores, outer band), raised to a pinned bottom_z, read only on tool-reachable columns. Bar: standing > leave + one cell (G-OVERCUTREF tolerance). Islands 4-connected with area, height, inscribed-disc width, peak; class safety when height > depth_per_pass + cell else action; abstains when cell > leave or flipped setup. Id `project.rough_leaves_islands`, field `triage.coverage`. Rulings given 14:40: add the viz tab row, append per-toolpath triage findings to `get_toolpath_diagnostics`, one narrate sentence. | `P15_A_coverage.md` (worktree) |
| C result (15:05) | DONE, not built: `adaptive/research_geodesic.rs` (research cfg) — geodesic distance from the entry disc (R about the EDT-max entry), islands as walls, the ONE `fast_sweep` solver made pub(crate) (+ converged flag), `iso_contour_where` filter; wraps = level sets one stepover apart (spiral.rs rule), ordered lower-level-first then nearest; wraps run through the shipped replay under `step_within_pass_load`, residue, wall pass, mop; NO trochoid insert (its values are underived). Harness `adaptive_geodesic_2d_renders_g1` writes SVGs for agent / contour_spiral / geodesic_a on star + pocket. Risks: field over-reads island corners (2.5 cells), kinks at meeting lines, 24-sweep cap, radial step between closed wraps (a continuous spiral `T/s − φ/2π` not built). Branch commits: de64e6ad (A), ce94b369 (B), b94f5e4d (C). | `P2_C_geodesic.md` (worktree) |
| D verifier | RUNNING 15:10: merge master into the branch, fmt, check, new sentries, folder sentries, viz --lib, mcp, clippy, magic-number scan, render harness | `P15_D_verify.md` (worktree) |
| (plan) D verifier | after A–C: fmt, clippy, named sentries, `rs_cam_core --lib adaptive`, viz `--lib`, mcp tests; `rg` for bare float literals in new code | — |

Operator 13:40: "sounds like it's worth trying some of these ideas" → Phase 2
candidate (a) starts in parallel with the instruments; (b) and (c) wait for G1.

## G1 ruling (operator, 2026-10-23 14:55, page `g1_2d_renders.html`)

"Geodesic looks good. Maybe spills a bit over the edges and also seems to
break down as it finds the other side of the circle; I would expect the
pattern to diffuse out of that opening better, instead it seems to have
broken down. The spiral still looks worse than contour parallel. Agent
search looks odd as always. The growth pattern does look a lot more like
the classic adaptive I wanted." Decision left to the lead: **candidate (a)
goes forward; (b) and (c) stay shelved unless (a) stalls.**

G1 defects to fix on (a) before 3D: G1-1 edge spill (wrap ends past the
region edge), G1-2 meeting-line breakdown behind the island (sawtooth
fragments instead of a continued front). Then the continuous spiral form
(no radial step, no plunge per wrap) and the link stage.

## G0 rulings (operator, 2026-10-23 13:25) and lead judgements

1. **Instruments first.** Operator: "instruments are important." Before
   any Phase 2 candidate: a coverage instrument for a 3D Rough through MCP
   (G-ROUGHCOV, start from `metrology/union_coverage.rs`), a gouge check of
   emitted moves against the model mesh for a rough (G-ROUGHGOUGE), and a
   thin-standing-material detector (G-THINSTAND, from `phase0/tools/spurs.py`).
2. **Defects filed now:** `DEFECTS.md` (G-LINKPLOUGH, G-SPUR, G-HOLECORE,
   G-OUTERBAND, G-AXIALDOC, G-WALLDIP, G-HOPRETRACT, G-SIMCYCLE).
3. **Trochoid cap 1.6 (lead judgement, operator: "no magic numbers").** The
   value is a tuned knee from the June wanaka200 sweep, not a derivation.
   Under the 2026-10-23 ruling it goes: `TROCHOID_CAP_MULT_3D` and
   `default_trochoid_cap_mult()` are removed when Phase 2 touches
   ContourSpiral; the spiral's engagement bound becomes the existing derived
   pass-load ceiling (`adaptive/search.rs` `step_within_pass_load`), which
   the plan already names as the bound for every candidate. Until then the
   baseline pages state 1.6. The peer's default-removal diff does not touch
   these two symbols (checked 13:20), so this research owns the removal.
4. **Compact spiral (lead judgement).** Arm STOPPED: 0 of 47 islands, and
   the 12 holeless islands refuse too. The module's own [REPO] constants
   (`max_chord_mm` 0.1, `closure_tol_mm` 0.05, backtrack 0.35 rad) are
   underived; they stay only because the module is research-only behind
   the `research` feature and nothing ships them. No productisation.

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

## G0 look (2026-10-23 13:15): operator and lead readings

**Operator (live GUI and the pages):** "AgentSearch looks like it makes a
nice spiral, then in odd shapes makes some very odd toolpaths (no clean
arcs). ContourSpiral looks almost as nice as parallel, but the arcs on
every corner look busy and potentially an artifact."

**Lead (all 9 columns, key views of each; zoom crops `phase0/img/lead_*.png`):**

- The spiral's "arcs on every corner" are NOT arcs. The Corne spiral G-code
  holds 11 G2/G3 in total. The busy shapes are chains of small closed
  polyline loops (1.5–2 mm) with a keep-down entry each, along the medial
  seam where the wraps' corners meet (`lead_corne_cs_corner_zoom.png`).
  They are the residue clean-up between wraps: V1 in its HEAD form (N1
  bead chain). The operator's artifact hunch is right.
- AgentSearch in the odd-shaped Corne region: irregular polyline walks with
  no structure, long diagonal fed links, scattered plunges
  (`lead_corne_as_right_zoom.png`). Matches the operator's reading.
- **New defect N10, hex-hole cores left standing (Corne).** The final stock
  top view shows a central pillar inside about a third of the hex holes on
  AgentSearch and inside 2 holes on ContourSpiral; ContourParallel cuts every
  hole clean (`lead_corne_as_stock_top_zoom.png`, `lead_corne_cs_...`,
  `lead_corne_cp_...`). The page did not name this. It is a finished-part
  defect and a coverage failure that bar 2 would have caught if it had an
  instrument.
- N7 confirmed on the zooms: AgentSearch and ContourSpiral leave the band
  outside the case wall standing with holes punched; ContourParallel clears it.
- rivmap100: ContourParallel links view is 104 retracts criss-crossing the
  part plus a fan of fed links from one corner (N2); AgentSearch has two big
  spirals and walks the rest; ContourSpiral has 1 061 keep-down clusters
  (the residue mop). Final floors of AgentSearch and ContourSpiral differ on
  0.2 % of pixels at this render; ContourParallel's floor shows rounder
  cusps (4 % of pixels differ from AgentSearch).
- wanaka200: ContourParallel back rough's links view is a solid wall of
  1 726 G0 hops (N8); AgentSearch back is one clean spiral with corner
  walks; ContourSpiral front has 4 414 keep-down links over the terrain.
  Final stocks look alike per fixture at this scale.
- Lead's answers to the three questions, per strategy (not a ranking):
  ContourParallel: links no (retract zigzags), path yes, part yes.
  AgentSearch: links partly, path no in odd shapes, part no (N5 spurs, N7
  band, N10 hole cores). ContourSpiral: links no (seam loop clusters), path
  yes except the seams, part partly (N7 band, 2 hole cores).

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
  geometry-selection lead confirms this is the intended break of 88d18baf
  (Drill `depth` retired for `floor`; old study TOMLs under planning/ are
  not migrated, no-legacy ruling). Phase 0 used a fixed copy
  `phase0/projects/wanaka200_base.toml`; the fixture itself is unchanged.
  `link_cuts_stock` on the default rough is not from that lead's packages;
  it is this research's to file.
- Lead session paused ~11:30–12:10 NZDT (usage limit); Phase 0 resumed and
  finished. The GUI restarted at 11:56 on the same binary (gui pid 84659).
- The worktree `.claude/worktrees/adaptive-head` (own target, ~4 GB) stays
  until Phase 0 ends; delete it with its target dir afterwards.
