# Progress

> **Finishing-strategy verdicts are SUPERSEDED (H4, 2026-08-04).** Every
> comparison of finish strategies in `planning/` — which strategy "wins",
> which has "nothing to do", the band-mix tables, the mm²/s efficiency
> figures — was measured through four instrument defects that are now
> fixed. Those verdicts are **void, not falsified**: the comparison could
> not have come out any other way. Before citing one, check
> `planning/review_2026-07-29/SUPERSEDED_CONCLUSIONS.md`.

> **Deleted planning material.** The structure purge of 2026-09-17 removed
> 1010 files from `planning/` and the root. A path below that is marked
> `(deleted 2026-09-17)` no longer exists in the tree. Retrieve any of them
> with `git show planning-pre-purge-2026-09-17:<path>`, and read
> `planning/DELETED_INDEX.md` for what each package decided and why it went.

## Where to carry on — hand-off 2026-09-25 (read this first)

The sessions that drove the work below (rs-cam-2f feeds, rs-cam-e2
roughing) ran out of tokens. Session rs-cam-13 landed what was in flight
and tidied the tree. **Master is at the head of this work; every
worktree except `.claude/worktrees/demo-build` (a GUI build, detached)
is merged and removed; nothing is pushed.** Old Claude sessions may
still be open in terminals, set to auto-continue on 2026-09-30: close
them before a new session works on master, or they act on stale state.

Landed 2026-09-25 (rs-cam-13): `851e8dfe` the Spektra sizes (24 rows,
G6 drill range 3.0-12.7 mm; EXTRAPOLATION_G6 §5.7), `4bee59ab` /
`ddb2b478` B6 one force line per material (EXTRAPOLATION_G7 §5),
`f1ec6283` the published power ceiling reads the final RPM.

**Fixed 2026-09-25 (cloud session, after the Wanaka models came into the
repo at `9a68c59`):** `suggest_feed_matches_final_geometry` was red only
because the models were missing (no bbox, so no stepover back-off).
`wanaka_suggest_integration` re-pinned to B6's force line (`bb3c802`;
bisect names `4bee59a`, an intended move). `rest_stock_identity_g_restres`
fixed in `95beefc`: since `1e17c3a` a ramp's first leg can lie above the
board, and the air-cut filter (it runs only after a simulation) turned it
into a round trip to safe Z, so a fresh op generated different moves
before and after a simulation. The filter now keeps a fed EntryRamp or
EntryHelix move.

**Fixed 2026-09-26 (cloud session):** `pill_writes_clamped_value_g_pillclamp`
is green (6/6). It was red from 2026-09-23 with three causes, each fixed
in the fixture: `0007528` (R1: Inlay allows only a V-bit; the Inlay
fixture takes the V-bit), `8702706` (R4 dial: the demo pocket funnel
writes 1.2 capped, then 0.85 x 0.75 load share, 0.825 snapped to
3.0 / 4 = 0.75 mm; ratio 5.6x; the test reads both stages from the
funnel warnings), `d5b7e34` (B4 key: the default 12.7 mm 90 degree V-bit
is 2.1x the 6 mm row and refuses; the fixture V-bit is 6.0 mm 90 degrees,
the printed key of `amana-vbit-softwood-trace-6000-2f`). Open question
for the operator: the GUI default V-bit (12.7 mm, 90 degrees) refuses on
VCarve in softwood since B4; 12.0 mm serves.

### Feeds / extrapolation stream (`planning/extrapolation_2026-09-24/`)

1. ~~The pillclamp red above.~~ Done 2026-09-26.
2. ~~G10 Part A~~ landed 2026-09-25 (`G10_PLAN.md` §3; RESULTS at its
   end). The
   plunge is a named fraction of the shipped side feed (`PLUNGE_RULES`),
   else the named material base; a ramp off the G6 claim holds its
   vertical rate at the plunge (Q2); the entry notes and the plunge basis
   are card lines and MCP `basis` keys. G-ENTRYREWRITE is closed: Suggest
   never writes the entry style (Q11). Sentries: the three `*_g10` core
   files and `the_card_names_every_entry_rule_g10`.
3. ~~G10 Part B~~ landed 2026-09-25 (`G10_PLAN.md` §6; RESULTS (Part B)
   at its end). A dressup `helix_radius` of None is the rule 0.3 x D
   (a saved 2.0 loads as an operator value, no format bump); every helix
   radius is capped at the flat bottom (Q6); the ramp angle, pitch and
   0.3 x D are named repo rules, one Adaptive3d rule for GUI and CLI
   (D4); a new 2D Adaptive starts Helix and an operator Ramp stays (D3);
   an entry never runs above the cut feed (`entry_feed_rate()`,
   G-RAMPCLAMP). Sentry
   `a_helix_leaves_no_core_and_a_ramp_never_outruns_the_feed_g10`.
   By Area WP3 can rebase.
4. B7 (RULINGS §B7): extend the deflection envelope to the 2D
   operations, then retire the 0.88 / 0.75 long-tool share.
5. The Optimize resolution parity gap
   (`tool_load/optimize/outcome.rs`, per-request auto resolution;
   ce6642be).
6. Operator owes: the B3 sim witness run, the bull literature cells,
   the hardwood V-bit 0.37x question, B5 open question 1 (Amana's
   printed 18 000 rpm against the 14 000 drill cap),
   `wanaka_suggest_integration` (ask before running; not run for B6).
7. B6 follow-ups (EXTRAPOLATION_G7 §5.4): a typed tool helix that
   selects the printed Curti helix row; a user line for Custom; a
   Kienzle form for aluminium; a printed line for plywood,
   particleboard, HDF. Check the FM1 sim DropCutter timing (about 15 %
   slower in the B6 runs on a loaded machine) HEAD against B6 on a
   quiet machine. The hardwood/plywood Spektra rows at 1/32, 1/16 and
   3/32 in are printed but not loaded (not in the ruling).

### Roughing / adaptive3d stream (rs-cam-e2's queue, block below)

1. ~~**G-PLANSIMGAP fix**~~ Done 2026-09-26: the 3D Rough planner applies
   the segment merge to each cut before it stamps it (RDP at max(op
   tolerance, merge tolerance)); the dressup merge stage moves nothing on
   a 3D Rough and the trace says "applied_by_planner". Merge stays on;
   entry clearance stays 0.5 mm. Sentry
   `adaptive3d_plan_matches_merged_path_g_plansimgap` (red before: 1382
   cells over 0.25 mm, max 2.812 mm; after: 0). Cost on the fixture: helix
   entries +6 % moves (they now start over the real top), fed length
   +5.0 % against the old merge. Landing record:
   `planning/entry_stock_awareness_2026-09-24/PLAN.md` RESULTS 4. The
   operator should see the rivmap100 time in the next GUI build.
   Measured 2026-09-25 on `planning/fixtures/rivmap100` (dpp 8, 0.5 mm):
   Global 778 -> 863 s, By Area 728 -> 813 s (+11 %), almost all entry
   time; volume unchanged. Accepted (operator delegated the call): a safe
   entry is not traded back; G10 Part B (helix radius, pitch and ramp
   rules) is the lever for the time.
2. ~~**G-RAPIDPLUNGETOL**~~ Landed 2026-09-25 (Option A, two-channel
   check): a low channel over the cells wholly inside the footprint, with
   only a float epsilon, ORed with the old high channel. Sentry
   `rapid_check_catches_shallow_plunges_g_rapidplungetol` (items 1-3 red
   before). rivmap100 0.5 / 0.25 mm: 0 rapid collisions before and after;
   no true strikes, no residue flags. Record:
   `planning/rapid_safety_2026-08-28/RAPIDPLUNGETOL_PLAN.md` RESULTS.
2a. **G-ENTRYORDER (2026-09-25): not a defect.** `pocket_lift_bridge_b1`'s
   3 rapid collisions at 1.0 mm cells (moves 114 / 268 / 423, Z-only rapids
   to -3.5 / -7.5 / -11.5 at (59.97, 18.60)) are coverage-union residue of
   `conservative_top`: the 1 mm high channel reads 0 / -4 / -8, one 4 mm
   level over the floor, and 4 mm > tau 2.207 cells = 2.2 mm. The stock
   there reads -4 / -8 / -12 at 0.5, 0.25, 0.1 and 0.05 mm, so the rapid
   floor is +0.5 over cleared material; 0 flags at 0.5 / 0.25 / 0.1 mm. The
   2.5D entry replay reads only moves emitted before each entry, and no op
   with an entry dressup is reordered after it. Bisected to 9887735 only
   because the measured floor went +2 -> +0.5 (the +2 floor cleared
   high - tau by 0.21 mm). Test now simulates at 0.5 mm (= Auto for its
   6 mm tool); new test `one_mm_cells_flag_only_the_known_coverage_union_residue`
   pins the 1.0 mm set. OPEN (queued with S2's sub-cell-aware clearance
   query): lower `conservative_top` once partial stamps together cover a
   cell.
3. By Area WP2 (the pocket tree: valleys are their own jobs, the high
   ground is one rest job; `planning/by_area_merge_tree_2026-09-25/`),
   WP3 (pocket minimum depth / area settings, after G10 Part B),
   WP5 / WP6 (measure Global against By Area; keep the faster order; if
   By Area is not faster for the same part, stop and report).
4. A GUI build for the operator after 1-3.
5. Smaller: does 2D Adaptive have the rapid-reorder defect; Global 36 s
   slower than By Area on one region; the sim cuts deeper than the
   planner stamps in 699 cells (safe direction); ring-order entries;
   2.5D helix beside a pocket wall has no containment check;
   G-MCPCUTROW; CLI job `strategy` silent fallback; the loader ignores
   the old `coarse_steps` key.
6. Operator owes: the By Area defaults (proposed 2 mm minimum depth,
   400 mm² minimum area: 3 valleys on rivmap100); segment merge off
   for 3D Rough if the planner cannot mirror it.

### Operator on-screen checks still open

- Rest stock identity (`planning/rest_stock_identity_2026-09-24/PLAN.md`
  "To check on screen"): resolution slider, WAIT rows, connector hover,
  confirm dialog, MCP too-coarse error, "regenerate <op> first".
- B6: the Feeds card force-line lines and hover, the stock panel
  "Force line:" row, plywood jobs now show no power figure.

## Current snapshot

`rs_cam` is now a desktop CAM application plus shared engine, not just an algorithm sandbox.

### Shipped surface

- 4-crate Rust workspace: core library, CLI, desktop GUI, and MCP server
- 23 GUI-exposed operations
- 14 direct CLI commands plus TOML job execution
- STL, SVG, DXF, and STEP import with BREP face selection
- 5 cutter families
- GRBL, grblHAL, LinuxCNC, and Mach3 post-processors
- feeds/speeds calculator with machine, material, and vendor-LUT inputs
- tri-dexel stock simulation (Z/X/Y grids, all 6 cardinal faces) and holder/shank collision checks
- typed GUI project persistence with missing-model warnings and editable-state round-trip
- dual-lane compute backend with lane-status reporting and active cancel support
- deterministic renderless `rs_cam_viz` regression harness in CI
- controller-first GUI architecture with canonical operation metadata and split compute/controller modules
- shared adaptive support module used by both 2D and 3D adaptive search/control code
- unified service layer: `ProjectSession` API in core, shared `execute_operation()` dispatch for all 23 ops
- MCP server (`rs_cam_mcp`) exposing `ProjectSession` tools for AI agent integration; the GUI embeds it (`--mcp`) and registers roughly 68 tools against the live session
- bounded typed simulation triage plus per-metric measurability abstention, consumed by GUI, MCP, CLI and narration through one contract
- machine kinematics as an analysis dimension: per-axis max rates (`$110/$111/$112`) in the machine model, a per-toolpath kinematic utilization instrument (utilization, feed-bound headroom, machine-bound share, plunge-class peak) on every simulation surface, and a geometric plunge guard in the feed modulator

## Roughing speed, entries and rest-stock identity — 2026-09-24/25 (session rs-cam-e2)

All on master, not pushed. Measured with `rs_cam_cli rough-score` (GUI-parity
instrument, 9699f0d5) on the rivmap100 demo copy.

- Step ladder (`planning/adaptive3d_step_ladder_roughing_2026-09-24/`): built,
  measured, REMOVED by operator ruling (9aa0994a..54ca58f6). Each Z level
  drapes to the surface, so a large plain Depth/Pass already cuts pockets in
  one pass (rivmap100: 2 mm 1379 s → 8 mm 586 s, same finished part). Kept:
  Fine Stepdown / Mill Shallow deleted; rest-stock roughs start at the real
  stock top; `--set` lists; unknown `order_by` refused.
- G-RESTRES / G-RESTSTALE / G-STALECARDS / G-MCPMODAL
  (`planning/rest_stock_identity_2026-09-24/`, f06a11b0..65584991): one stored
  project simulation resolution (Auto project-wide); each rest result records
  the stock it read and is dropped when it no longer matches; metrics-on and
  metrics-off carve the same stock; the simulation reads core results only;
  freshness is per operation ("regenerate <op> first"). Rule: GUI, MCP and CLI
  give identical numbers for the same project state.
- By Area regions overlay (772ab043..4f74849a): viewport Inspect row + MCP
  `inspect_spans.area_regions`. Finding: on terrain By Area finds ONE region.
- Entries (`planning/entry_stock_awareness_2026-09-24/`, 04006014..8ff81472):
  entries rapid to the planner's real stock top; keep-down link for all entry
  styles with a stock corridor check (the 8 mm straight plunge into stock is
  gone); helix/ramp take the full material depth and never helix air;
  new `ramp_feed_rate` (22 op configs, None = plunge rate) and
  `entry_clearance_mm` (default 0.5). dpp 8 helix: 1090 s default,
  734 s at ramp feed 2400, 580 s with pitch 2; plunge style ~370 s.
- Open, in order: (1) GUI build + operator look (overlay, resolution slider,
  WAIT rows/hover, regenerate message, entries); (2) By Area merge-tree
  measurement (priority flood in `surface/flow_accum.rs`, persistence h +
  min area) then Phase 3 (By Area uses the tree); (3) ring-order entries (needs
  a per-ring slope bite limit; trial 384 s); (4) 2.5D helix beside a pocket
  wall has no containment check; dome-slope steep drops; G-MCPCUTROW; CLI job
  `strategy` silent fallback. Feeds session owns: `ramp_feed_rate` value +
  Suggest write, G10 (entry params per tool), optimize-resolution parity gap.
- 2026-09-25: GUI release at 40f2b744 installed. Operator look found an
  entry defect: at a helix entry the tool feeds straight down through
  standing stock (rivmap100 span 59, Z 12 -> 7.8), then helixes in air;
  `project.entry_load` 448 samples, peak 6.08 mm. FIXED fc82d85f: the
  cause was the rapid-order dressup, which reordered 3D Rough runs after
  planning, so entries met stock the planner had seen cut. 3D Rough now
  refuses rapid reorder (as Face does); the GUI box greys out (984dec55).
  rivmap100 dpp 8: 580 -> 595 s; entry samples > 2x bite 448 -> 0; peak
  entry bite 6.08 -> 1.61 mm. Sentry
  `session_rough_keeps_the_planner_order_for_its_entries`. Seen on screen
  by the operator 2026-09-25 (GUI 984dec55): "looking very good"; MCP
  shows entry_load gone. The operator also saw a more logical cut order
  with no thin walls left standing: the reorder had split the planned
  ring sequence. Queue from the feeds session (043cece1, ramp-feed Suggest
  write): G-ENTRYREWRITE (pick_adaptive3d_entry_style writes entry_style
  on a scratch op that apply_feeds_subset never copies back, so the
  StrategyRewrote warning names a change that never ships); G-RAMPCLAMP
  (a hand-lowered feed can leave ramp_feed_rate above the feed). Both now
  owned by the feeds session (G10: Q11 removes the Suggest entry-style
  rewrite; Q6/Q8/Q9 helix radius cap, pitch and ramp-angle named rules),
  landing after By Area WP1; By Area WP2/WP3 rebase on it. Open: does 2D
  WP1 of By Area (e93cb4ce): a job cuts its cells, not its box; the
  material gate and level filter fixed for By Area. rivmap100 By Area
  595 -> 728 s: the fix adds the Z 0.5 level the old filter dropped.
  FOUND: Global has the same gate defect and never cuts Z 0.5 on
  rivmap100 (about 1200 mm³ left for the finish tool). FIXED 93ce4e29
  (Global gate counts the level grid; sentry
  adaptive3d_global_gate_drapes_below_floor). rivmap100 dpp 8 now: Global
  764.5 s / 48457 mm³, By Area 728.2 s / 48360 mm³ (1 region). Every
  earlier dpp 8 rivmap100 time (580/595 s) lacks the Z 0.5 level. Open:
  Global is 36 s slower than By Area on one region (probably the
  per-level waterline cleanup). The 1 Global rapid collision (move 6497)
  is a TRUE POSITIVE of G-PLANSIMGAP, not a check artefact (first read as
  an artefact, corrected the same day): the segment merge left a ~2.3 mm
  wall the planner never stamped, about 0.05 mm from the tool edge
  (0.21 mm vertical at a 0.1 mm replay). Not a gouge, but far under the
  0.5 mm the planner intends. Do not loosen the check.
  G-WLENTRYDISC (waterline_cleanup reads the entry floor over the tool
  radius only, the helix reaches 1.8 mm further) and G-PLANSIMGAP (planner
  stock under-reads the sim by up to 0.29 mm there; rapid reorder is off,
  so suspect segment merge / arc fitting). Probe:
  planning/entry_stock_awareness_2026-09-24/probes/probe_rapid6497.rs.
  G-WLENTRYDISC FIXED (waterline_cleanup reads the entry floor over the
  helix sweep; sentry waterline_helix_entry_reads_its_floor_over_the_helix_turns,
  red 2.83 mm before). rivmap100 Global 764.5 -> 778.4 s (helix takes the
  flank), By Area unchanged 728.2 s.
  G-PLANSIMGAP MEASURED: the cause is segment merge
  (dressup::condition::merge_linear_runs, 0.3 mm) after the planner;
  stamp_emitted_segment does not mirror it. Sim above planner: 4016 cells
  > 0.25 mm, max +2.34 mm (slopes amplify the lateral shift). Merge off:
  0 cells, every in-stock entry exactly 0.500 mm clear. Arc fitting, links,
  feed opt, boundary and cutter: no part. Evidence:
  planning/entry_stock_awareness_2026-09-24/plansimgap/. Fix options:
  planner mirrors the merge (structural), or merge off for adaptive3d
  (op tolerance simplifies; 9138 vs 8222 moves), or cap merge tol at op
  tol (mitigation). Do NOT raise entry clearance to cover it.
  Also open: before any dressup, the sim cuts deeper than the planner
  stamps in 699 cells (> 0.5 mm, min -2.0 mm): a second planner/emitter
  gap, in the safe direction.
  Rapid-check sentries (8d5930eb, no change to the check): side strikes,
  sub-cell slivers and rim walls are caught. OPEN G-RAPIDPLUNGETOL: the
  check subtracts 2.207 cells in Z, so a rapid tip 0.3 mm into material
  under the whole footprint is NOT flagged (1.10 mm at 0.5 mm cells,
  0.55 mm at 0.25 mm); a 1 mm plunge is missed at 0.5 mm cells.
  Adaptive have the same defect (it still allows the reorder)?
- 2026-09-25: By Area pocket tree measured
  (`planning/by_area_merge_tree_2026-09-25/`, 6f52d7a3): 3 valleys at
  h 2 mm / 400 mm² where By Area finds 1. Phase 3 blocker: the planner
  cuts a region by its box, not its cells. Operator has not chosen h/area.

## Extrapolation programme — 2026-09-24/25 (ruled; G1-G6 and B6 landed)

`planning/extrapolation_2026-09-24/`: ten gap groups. Phase 0 INVENTORY; Phases
1-2 fetch + trend (`EXTRAPOLATION_G1..G9`, verified rows in `fetch/<G>/`);
operator rulings in `RULINGS.md` (A1 tapered key = tip, A2 point mode, A3
visible family transfer, A4 "Wood" serves hardwood, B1-B8; G9 parked). Landed,
each with its §5 record: P1 G1 size (tip key, `feeds::extrapolation` size
claim, card + MCP basis; wanaka "3D Finish 6" ships), P2 G2 (Onsrud V-bit rows
for MDF/plywood, one Janka table, soft/hard cap), A2 G4 point mode (gate,
modulator, advisor), A3 G3 family transfer (tapered + bull), B5 G6 drill (the
2.5 multiplier deleted; flat end-mill plunge = side chip / Z), B4 G5 V-bit key
(printed key, Amana 18 000 rpm transcribed). Refusals on the FM1 matrix 512 ->
426. Visible: the Feeds card states every claim, cap, transfer and key.
2026-09-25: the Spektra sizes (G6 drill range 3.0-12.7 mm, 851e8dfe) and B6
(one printed force line per material: Curti 2021 for solid wood with FPL
densities, Goli 2018 for MDF, every other material refuses; EXTRAPOLATION_G7
§5) landed. Next: see the hand-off section at the top of this file. The operator owes: the B3 sim run, the bull literature
cells, the hardwood V-bit 0.37x question. Next-session prompt:
`planning/extrapolation_2026-09-24/PROMPT_NEXT.md`. Not pushed.

## Feeds matrix — 2026-09-23 (Phases 0-3 complete; Phase 4 in progress under the rulings)

`planning/feeds_matrix_2026-09-23/`: PLAN.md, EVIDENCE.md (230 findings, 97 confirmed
by a second fetch), RULINGS.md (the five rulings and what landed), DERATE_CHAIN_REVIEW.md,
FORMULA_BACKING.md, the FM1 matrix outputs. Landed: `FeedsSupport` per cell (1784881d),
the FM1 instrument (15f751c0), the printed vendor rows and re-graded seed rows (R5,
3885cbf..bab9a236, 827f383e), one published depth de-rate for the feed and the band (R3,
d8d8b0c7, aef54c83), R1 in both halves: Suggest refuses what the registry tool rule refuses
(0007528f, 2d786d0c) and refuses a formula-only wood cell FORMULA_BACKING_v2 calls CLUELESS
(94b80b24, 15b98aaa; the add doors still create the operation, without a recipe). Matrix on
15b98aaa: 498 of 960 ship, 462 refuse (192 tool rule + 270 CLUELESS). R2 smaller step LANDED 1ff9344a (no finish depth ceiling; one tapered engaged diameter; depth
diagnostics ids). Chart display LANDED 5199e06e. R4: spec 171ec9c4; WP1 + WP2a 7af7d76b; Q9 06c75ed5; Q6 c8b6b358; WP3 the dial LANDED
87027060 + dcb53a47 (0.75 gone, `aggressiveness` 0.85, size rule, composite lookup categories);
Q10 f82c1e70 + 674dbe73; Q8 b5675591; WP2b 63d7a5cd. R4 is CLOSED except WP5, which folds into the
extrapolation programme. Visible: flat-end pocket and adaptive cells at 1/8 in, 6 mm and 1/4 in read
a single printed value, so no band and a silent burn gate there. Not pushed.

## Simulation cut metrics — 2026-09-23 night (packages A–E landed; F in progress)

`planning/sim_cut_metrics_2026-09-23/PLAN.md`. Landed: A+G f1c62483 (one capture control;
`side_panel` helper, `g_panelfit`), B+D a1cbff0d (`tool_load::distribution`, `metric_guide`,
`g_cuthist`), C+E 22c3594c (cut-metric cards in the inspector, time-series drawer,
`g_cutcards`). §7 answers assumed (lines not shading; share of cutting time; one drawer
route; five metrics). Not seen on screen. Residual too-wide rows are ignored instrument arms.

## Viewport interaction redesign — 2026-09-23/24 night (phases 1–4 landed; mockups NOT operator-reviewed)

`planning/viewport_interaction_redesign/`: PHASE1_INVENTORY.md + MOCKUPS.md 171ec9c4 (NOT
operator-reviewed), phase 2 dock + catalogue 2594fec7 (`g_vpdock`; `Shift+O` retired; the
three strip commands rebuilt in the dock), phases 3–4 4eb46da8 (`g_vpstate`: seven live row
states, async intent protection, Compute vs Compute & show, Compute-rest confirm "may", real
legends incl. rest p95). Open: Q4 inspector reach checkbox bypasses `set_overlay`, Q5 deviation
render substitution in `app/gpu_upload.rs`; three legend colour sets mirrored from render
literals (move them into `render/colors.rs`). Not seen on screen.

## Generate ↔ simulate ↔ rest — 2026-09-18/19 (COMPLETE; MCP path and every indicator seen on screen, the GUI Auto click and the confirm modal not yet)

`planning/gen_sim_rest_ux_2026-09-18/`: `PLAN.md`, six implementation
briefs, `IMPLEMENTATION.md` (reconciliation, every landed commit, the
live look). The operator's brief: incremental simulations on Generate
All, one rest path, a minimal graphical dependency indicator, generating
vs simulating on the row, everything dependent goes stale on an edit.
The finding that explained the complaint: the GUI ladder refused to
start whenever the Simulation panel was on "Auto from tool size" and
submitted nothing. Five packages landed 2026-09-19 plus R2 and three
connector rounds: one pure edge function in core that the invalidation
walker, the card and the MCP wire all read; one core plan walk shared by
GUI, MCP and CLI, advanced off-frame (the 137 s stall was the ladder's
own frame-drained event push); the "Start from" row replacing four rest
controls; the rail connector, the simulating ring and the progress
button; one simulation-freshness truth (the core epoch; a late
simulation is refused, D7); one `awaiting_prior_stock` shape, an enum
for `set_stock_source`, `depends_on` on `list_toolpaths`, the CLI plan
driver. Gate on the merged tree: viz lib 412/0, core lib 2530/0,
nineteen viz binaries, cli, mcp, workspace clippy and fmt clean. Open:
R11/R12 MCP freshness wording, R13/D8 a core export refusal for a
blocked operation, the view-only isolation simulation, the confirm modal
unseen live. Not pushed.

## Design audit — 2026-09-17 (findings only; nothing landed)

Twelve read-only agents audited the regrouped tree for design patterns and
feature debt: `planning/design_audit_2026-09-17/` holds the brief, one file
per folder group (135 findings, each with a `file:line` anchor, a proposal,
the break it causes and the sentry that guards it) and `SYNTHESIS.md`: 20
defects ranked first, eight cross-cutting themes, a 25-row programme in five
waves, a not-now list and a verification list. The top defects: a failed
holder-collision check reads as zero collisions; MCP export drops the
machine-safety findings the GUI shows; `apply_tabs` strips move intent; an
unknown `face_up` loads as Top; a Post-tab edit never marks the project
dirty. All five waves landed overnight 2026-09-17/18: about 130 commits from
`1a0a1ea5` to `32f7ca82`, every wave gated (`WAVE*_GATE.md`), nine
sequential tail slots, no row landed without its sentry going red first.
`SYNTHESIS.md` carries the per-wave outcomes, the skips with cause and the
open operator items (STK-13, STK-12, FIN-08, the `tool_load/` lines, a
live GUI look).

## Structure programme — 2026-09-17 (navigable repository; COMPLETE)

The tree changed shape today. Read `planning/structure_2026-09-17/EXECUTION_SUMMARY.md`
for the full account; the durable facts:

- **Planning purge.** 1 010 files (1.15 GB) left `planning/` and the root.
  Nothing was archived. Every deleted file is at the annotated tag
  `planning-pre-purge-2026-09-17`: `git show planning-pre-purge-2026-09-17:<path>`.
  `planning/DELETED_INDEX.md` records what each deleted package decided and
  why it is obsolete. A `planning/…` path in a doc comment that no longer
  exists is expected, not a defect.
- **Core layout.** `crates/rs_cam_core/src/` holds `lib.rs` plus seven spine
  files (`geo`, `polygon`, `mesh`, `toolpath`, `ids`, `interrupt`,
  `measurement`); everything else sits in one of 24 folders (`ops/`,
  `finish/`, `geometry/`, `surface/`, `maps/`, `stock/`, `dressup/`, `io/`,
  `export/`, `trace/`, `machine/`, `util/` are new). No re-export shims: a
  moved module's path changed everywhere.
- **File splits.** The 19 production files over 3 000 lines (core and viz,
  including `session/compute.rs`, `compute/execute.rs`, `feeds/suggest.rs`,
  `app/mcp.rs`, `ui/properties/mod.rs`) are split into children as pure
  moves; the parent re-exports every public name, so callers did not change.
  Only `polygon.rs` (spine, ruled whole) remains over 3 000.
- **Instruction files.** Every module folder with an invariant carries its own
  `CLAUDE.md` (≤ 40 lines: file map, invariants, sentries, traps). The crate
  files are indexes. A rule lives in the one folder that owns it.
- **Also closed:** `feeds/` and `tool_load/` tech-debt hand-off list; all
  five red tests handed off by earlier programmes; the CLI smoke baseline is
  re-cut at `planning/toolpath_acceptance/baselines/2026-09-17.csv`
  (AS014's power verdict flagged for the operator).

History was rewritten with `git filter-repo` the same day (four blobs out;
HEAD tree unchanged). Old commit hashes resolve through
`planning/structure_2026-09-17/commit_map_2026-09-17.tsv`. Open: the
force-push of the rewritten history and the tag; the AS014 smoke verdict
(other session); the two `conformal_spiral` children (a separate session).

## Architecture consolidation — 2026-09-13 (close-out: every package landed; one core dev loop)

The operator answered four close-out questions (plan §27) and one UX
question (plan §28 ruling 1). Every open package landed on `master`:

- **WP22** (`077acf91`): the feed-optimisation pass caps a geometric
  plunge at the operation's `plunge_rate`, the same classifier the
  modulator's P3 guard reads. Measured red: 18 of 18 plunges at 2.00×
  on the sentry pocket. G-FEEDOPTPLUNGE is closed.
- **WP19** (`c566b24d`, `9d11ddc3`): the view mirrors
  `Effects::simulation_cleared`; a toolpath edit CLEARS the viewport
  simulation like every other edit (operator ruling; the F2.5 banner
  for toolpath edits ends); `mcp_stamp_stale` is merged into
  `state::stale::stamp_stale`, and a row it creates reads the catalog's
  `default_auto_regen`.
- **WP14b** (`04d09453`): `OptimizeToolpath` is a `Job` over a cloned
  session; the three `mem::replace` sites are gone; the clone on the P0
  fixture costs 0.07 ms. An `optimize_toolpath` issued after an edit that
  dropped the simulation refuses at submit.
- **WP15b** (`38951287` over five fix commits): `ProjectSessionBuilder`
  gains id-allocating `add_tool` / `add_model` / `add_setup` /
  `add_toolpath` and read accessors; 610 out-of-crate test sites moved
  to the builder or `apply`; the 59 `ProjectSession` setters are
  `pub(crate)`. The compile is the completeness proof.

**Independent completeness review** (`REVIEW_COMPLETENESS_2026-09-13.md`,
pin `2c593c7d`): INCOMPLETE, one blocker. The blocker became **WP23**
(`88aeb4d4`, `7e889892`; sentry `73c11d5b`): two view rows claimed a GUI
constructor that did not exist and are deleted; the view sentry now
censes constructors for every `gui: Reached` view row; and the viz
crate compiles without the `mcp` feature again. The review's other three
findings are ruled in plan §29. Open ledgers for the operator:
G-PERFGOLDEN2D, G-F036B-FLOOR, G-NOMCPTESTS.

**Closing core dev loop** (once, capped, at `0487287e`): 3863 passed /
4 failed / 272 ignored. All four reds are pre-existing: the f036b band
arm (red by design), the f036b floor arm (new ledger G-F036B-FLOOR),
`perf_golden_sim_metrics` (G-PERFGOLDEN2D, a re-bless is the operator's
call), and one stale surfaces arm, fixed at `b5250b71`. The heavy gate
did not run.

**Ledger close-out, same day** (plan §30 and §31; the operator answered
one question per ledger):

- **G-PERFGOLDEN2D**: the 2D perf golden is re-blessed at `457fddc2`;
  the moved fields are in that commit body.
- **WP25** (`b4dfb329`): the three MCP tests in
  `controller/results_parity_tests.rs` are gated on the `mcp` feature.
  The two store-half tests still compile in both configurations. The
  non-MCP all-targets viz clippy joins the lint gate. G-NOMCPTESTS is
  closed.
- **WP26** (`a21a4a24`): the f036b floor arm was an instrument defect,
  not a feeds change. Its proxy counted a move as modulated when the feed
  differed from nominal, and since WP11b (`80a9cf4d`) the
  feed-optimisation dressup writes per-move feeds on the same door. The
  arm now diffs the pre- and post-modulation IR per index and skips a
  `PlungeRate`-bound move. No feeds quantity moved. The binary reads
  9 passed / 1 failed; the band arm stays red by design. G-F036B-FLOOR
  is closed.
- **WP24** (`f405a657`; sentry `4efc88ee`): the full-screen Optimize
  placeholder is deleted. An Optimize run is `optimize_run:
  Option<OptimizeRun>` on the view state, the workspace bar draws a
  progress row with the run label, elapsed time and a Cancel
  (`UiCommand::CancelOptimizeRun`, a registry row constructed in the
  view), and the GUI stays usable during a run. A second Optimize request
  while one runs is refused with a toast. Not decided, recorded in the
  fix body: the modal exclusivity rule still spares a running Optimize,
  Apply is not blocked on a stale baseline, and the modal's Cancel
  discards the partial outcome while the row's Cancel keeps it. Nothing
  was verified on screen; the release GUI rebuild follows.

The work-package table held 30 rows, WP1 to WP26 all DONE, at that point.

**Evening of the same day, after a live smoke pass on the release build**
(STATUS.md "Live smoke pass" block; four ledgers G-MCPSIMMIRROR,
G-EXPORTEMPTYSETUP, G-FRESHNESSDISAGREE, G-DIRTYONLOAD, plus
G-OPTCANCELPARTIAL from the WP29 scout):

- **WP27** (`1a87e04c`; sentry `6bc2e506`): the viewport draws the SELECTED
  toolpath only by default (operator ruling, plan §32: many drawn toolpaths
  make the viewport slow). One `show_all_toolpaths` view flag, one Overlays
  registry row `all_toolpaths`, one bar button, one pure `toolpaths_to_draw`
  read by both the upload and the pick; the isolate pin wins. The Simulation
  workspace still draws every toolpath. Consequences: nothing selected draws
  no toolpath, so the viewport is empty after a load until a row is clicked,
  and a viewport click cannot select an undrawn toolpath.
- **WP29** (`8849572e`; sentry `2c8b4183`): an Optimize run reports its rung
  (three: feed/rpm, grid, refine) and its candidate done/total on the
  workspace-bar row and in the Optimize window (plan §33). The progress
  struct rides the job handle and the evaluation context, so no optimizer
  signature moved. No time-left figure: the whole-run total is unknown up
  front. The window's "keep partial results" sentence is untouched and
  ledgered as G-OPTCANCELPARTIAL, because both cancel routes discard them.

- **WP30** (`57841b05`): the toolchain moved to Rust 1.98.1 (operator ruling;
  pinned by `rust-toolchain.toml` at `7cad232a`). The new compiler raised
  156 findings, not the 14 a partial measurement had counted: 20 clippy
  findings in core, 132 `float_literal_f32_fallback` future-incompatibility
  warnings and 4 clippy findings in viz, none in CLI or MCP. All fixed, no
  `#[allow]`, no behaviour change; the workspace clippy gate and the
  no-default-features viz gate both exit 0 on 1.98.1.

The table holds 33 rows; WP28 (one GUI apply function that mirrors every
`Effects` field, closing G-MCPSIMMIRROR) is the open one.

## Architecture consolidation — 2026-09-12 (reviews and follow-ons)

Two independent Opus reviews ran after the sixteen packages landed
(`planning/arch_consolidation_2026-09-09/REVIEW_COMPLETENESS_2026-09-12.md`: complete with
residuals, no blocker, five majors; `REVIEW_TECH_DEBT_2026-09-12.md`: ten highs). The
findings became WP14-WP21 in `STATUS.md`; six landed the same day, red-first:

- WP16 `SetSetupName` reached from the GUI; WP20 prose pass (row count, twenty stale
  `Skip` reasons, plan §5 residuals, doubled "Save failed"); WP18 feed-optimisation refusal
  coverage restored and a vacuous WP12 sentry arm made to fire.
- WP17: a save or a post-config edit no longer clears the session simulation unless a
  motion-reaching post field changed; rest operations generate after a save again.
- WP21: the feed-optimisation pass no longer panics when half the nominal feed exceeds the
  dressup ceiling (a latent production panic since 2026-05; first exercised by the session
  generate door on 2026-09-11).
- WP14a: `RecommendClearingStrategy` and `PreviewTierMap` run as no-adopt `Job` rows on
  the compute lane instead of blocking the frame loop.
- WP15a: 24 rows so every public `ProjectSession` setter has one (registry 71 rows); the 48
  remaining direct setter calls in viz and CLI go through `apply`; a source-scan sentry
  keeps it so.

Open: WP14b (`OptimizeToolpath` as a `Job` over a cloned session; plan §24), WP19 (nineteen
viz sites still discard `Effects.stale`; merge the two stamp helpers), WP15b (setters to
`pub(crate)`, 656 test sites), and the ledgered G-FEEDOPTPLUNGE (the pass can overwrite a
plunge to twice its plunge rate; needs an operator decision). Final suite counts: viz 668,
CLI 32, MCP 29, core `session::` 156. The full heavy gate was not run (operator ruling).

## Architecture consolidation — 2026-09-11/12 (one command surface: implementation COMPLETE)

All sixteen work packages of `planning/arch_consolidation_2026-09-09/IMPLEMENTATION_PLAN.md`
landed on `master` between 2026-09-11 13:00 and 2026-09-12 06:00 NZST, each red-first with a
named sentry. The tracker is `STATUS.md` (table "Command surface — work packages").

- **The registry.** `rs_cam_core::session::command` holds one `for_each_command!` list with
  six columns (kind, id, wire name, payload, answer, surfaces) and a kind-split callback:
  `Command` rows (`apply` → `#[must_use] Effects { stale, simulation_cleared, revision:
  Option<u64>, created }`), `Query` rows (`query` → `QueryAnswer`), `Job` rows (`start` →
  handle; `execute_job` off-loop; `AdoptResult`). A second registry in viz
  (`ui_command.rs`) holds `UiCommand` / `UiQuery` rows. `SurfaceId` unions both.
- **One producer of the stale answer.** Every session mutation builds `Effects` through one
  combinator (`with_effects`). MCP replies read `Effects.stale` (N15 closed except two
  hand-written holdouts). `AdoptResult` refuses a completion whose revision moved.
- **One assembly of generation inputs.** `ResolvedGenInputs` is public with private fields
  and one producer; the GUI worker runs core's `execute_job` on a handle from `start`; the
  loose executor is crate-private. N12 items 1-10 closed (item 10, two simulation states,
  closed by `AdoptSimulation`).
- **No mutation hatch.** Of the eleven `*_mut` doors, six are deleted, three are
  `pub(crate)`, `insert_result` is `pub(crate)`, `wizard_mut` is gone (`WizardState` moved
  to viz). `cargo build -p rs_cam_viz` is the containment proof. Test fixtures build through
  `ProjectSessionBuilder`.
- **Doors.** MCP: 29 mutation variants collapsed to `McpRequestKind::Core(CoreRequest)` with
  one arm and a describe step; 18 view variants under `Ui` / `UiQuery`. GUI: the inspector
  applies `ReplaceToolpathConfig` each frame with the signature gate in core; the twelve
  egui draw sites and 34 non-egui sites write through rows; undo/redo restore through
  `RestoreToolpathSnapshot` (N14 closed); drill picks invalidate the chain (N6 closed); the
  five post-write notification events and `RemoveSetup` are deleted.
- **Gates.** Per package: sentries red then green, targeted suites, viz / CLI / MCP crate
  suites, fmt, clippy with `heavy-tests` `-D warnings`. Final counts: viz 660, CLI 32, MCP
  29, core `session::` 156. The full heavy core gate was NOT run (operator ruling
  2026-09-11: no large test gates). `f036b` stays red by design.
- **Residuals recorded in the plan §5 and STATUS:** three MCP holdouts (`apply_feeds`,
  `plan_multitool_finishing`, `export_gcode`) plus `load_project`; `compute_stale_set` /
  `MutationKind` kept for two callers; the strategy advisor's loose executor call and the
  14-argument `execute_operation` (test-only); `OptimizeToolpath`, `RecommendClearingStrategy`
  and `PreviewTierMap` are ruled `Job` (§14) but not yet moved; the `CloseOptimize*`
  `mem::replace`. Two independent Opus reviews (completeness, tech debt) are filed beside
  this block in `planning/arch_consolidation_2026-09-09/` when they complete.

## Architecture consolidation — 2026-09-10 (N1 DONE)

On `ui-fix-2026-09-09`, N1 now excludes disabled cached operations from core
checked G-code export, the door both CLI routes consume. A pre-fix reproduction
failed; the new sentry checks motion and phase metadata omission, retained
cache, an enabled control, and re-enable without regeneration. Review accepted.
No enabled missing/stale-result policy changed and no phase is complete.
N3/STEP units was already fixed by `be8b78c5`; it was not redone.

Focused export/core/GUI/CLI checks, workspace format and heavy-enabled Clippy
passed. Full heavy core: **3783 passed, 1 failed, 288 ignored**; the sole failure
is the documented F-036b all-F-word-median baseline (0.0214 vs [0.0320, 0.0550]),
not a green gate or a new N1 regression. Evidence and next-item ordering:
`arch_consolidation_2026-09-09/STATUS.md`. N1 is committed: the fix is
`3255a2ee` and the sentry is `b7bf5b33`. N2 is next.

## Architecture consolidation — 2026-09-10 late (N1, N2, N4, N5 DONE)

Three more pre-phase defects landed on `ui-fix-2026-09-09`, each as a sentry
commit that fails alone and a fix commit that carries the red output:

- N5 `f00f3bed` (sentry `4bdef194`): `set_toolpath_param` refuses `stepover`
  and `depth_per_pass` on an operation without the field. Before, it reported
  success, stamped manual provenance and staled the result chain. The three
  alias setters (Pencil, Waterline, RampFinish) still write. `rs_cam_cli run
  --set` on an unsupported field is now an error.
- N4 `84e43fff` (sentry `2dcfd14b`): the core diagnostics route resolves the
  toolpath's own `HeightsConfig`. Before, it projected heights onto the stock,
  so a pinned Top Z never reached MCP `get_toolpath_diagnostics` and three
  `geom.*` height checks could never fire there. The tautology parity test is
  replaced by a real ribbon-vs-session test.
- N2 `cff84815` (sentry `d36f5050`): the modulation retime republishes
  `toolpath_runtimes`, the summaries and the project total through one
  publisher on one clock. Before, the project total dropped every drill and
  the per-toolpath runtimes stayed pre-modulation.

Full heavy core after all four: **3793 passed, 1 failed, 288 ignored**; the one
failure is the F-036b instrument, red by design. Viz 606/0, CLI 31/0. New
rows N7–N11 and the checkpoint are in `arch_consolidation_2026-09-09/STATUS.md`.
No live MCP check ran this session; the server did not connect.

## Architecture consolidation — N9 diagnostic-context parity

N9 brings the GUI ribbon's Rest preconditions and model-reference findings
onto the same canonical contexts as session/MCP diagnosis. The production
`ToolpathPanelSnapshot` captures the entry and both contexts together; the
ribbon requires them rather than accepting an absent context. Existing GUI
load-verdict, feeds, generation-stat and height inputs remain unchanged.

The extended real ribbon-vs-session sentry reproduced **4 passed / 2 failed**
before the fix (missing Rest predecessor and dangling model), then passed all
6, including valid controls. Review accepted. Full viz **609/0**, MCP **29/0**,
format and heavy-enabled workspace Clippy pass. Full heavy core remains
**3793 passed, 1 failed, 288 ignored**, solely the same F-036b instrument at
0.0214 vs [0.0320, 0.0550]; it is not a green full gate. No live GUI/MCP check
was performed. Logs: `/tmp/rs_cam_n9_gates/`; current phase/decision status:
`arch_consolidation_2026-09-09/STATUS.md`.

## Architecture consolidation — 2026-09-11 (rulings, three fixes, the implementation plan)

Work moved to `master` (operator ruling; `ui-fix-2026-09-09` fast-forwarded in).
Landed red-first: P0-D1 coolant on the core export door (`ebfa657a`, sentry
`a96e3c4e`); N7 the modulation retime integrates only with a kinematics block
(`a20c0a93`, sentry `b6a98fe7`); N10 `angular_step` / `point_spacing` ranged
`greater_than(0.0)`, operator-authored (`5a5f9670`, sentry `598d2fc9`). Full heavy
core on the Phase 0 tree: **3801 passed, 1 failed, 288 ignored**; the one is F-036b,
red by design. Viz 618/0, CLI 31/0, MCP 29/0, fmt and clippy clean.

The one-command-surface ruling is ADOPTED
(`arch_consolidation_2026-09-09/RULING_ONE_COMMAND_SURFACE_DRAFT.md`) and its
executable sequencing is `IMPLEMENTATION_PLAN.md` (15 work packages, two review
rounds). Nothing of it is implemented. Q5 blocks WP7; WP1 can start.

## Older programme blocks

Every block before 2026-09-10 (2026-03 → 2026-09-08) lives in
`PROGRESS_HISTORY.md`, in the same shape. Read it only when a question
reaches that far back; the current snapshot above is the entry point.

## Known open work

- **F-034 cycle-time re-bench** — the 827 s Shapeoko wall-clock anchor is stale and the calibration test now flakes near its widened floor (0.30); re-measure per `planning/cycle_time_rebench.md`
- **enclosed-hole detection for adaptive3d** — optional "uncovered region not touching the grid border = hole, don't descend" mode on top of `SurfaceHeightmap.covered`
- **tri-dexel contour-tiling mesh** — full surface reconstruction for non-heightmap views (current side-grid mesh uses ray_top heightmap)
- emit per-operation manual pre/post G-code in export
- wire profile controller compensation (`G41` / `G42`)
- surface rapid-collision rendering and simulation deviation coloring
- expose workholding rigidity and vendor-LUT management in the GUI
- continue optional cleanup in `adaptive.rs` / `adaptive3d.rs`, but structural blockers are no longer the active tranche

## Verification

- `cargo run -q -p rs_cam_cli -- --help` succeeds
- `cargo fmt --check` passes
- per-crate `cargo test -q -p rs_cam_core` (and `-p rs_cam_cli`, `-p rs_cam_viz`, `-p rs_cam_mcp`) pass. Do **not** run `cargo test` at workspace scope on this repo — see `CLAUDE.md`
- `cargo clippy --workspace --all-targets -- -D warnings` passes

Update this file when the shipped surface or verification status changes materially.
