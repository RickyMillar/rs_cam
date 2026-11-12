# P29-8e design: the straight plunges of wanaka idx 17 under the Ramp style

Written 2026-11-13 on HEAD `0038733a5`. No product code changed. The prototype edit was restored
from byte copies (`sha256sum -c` OK on all four files, then `touch`). The probe test was deleted.
The lead reviews this note and rules on section 8.

## 0. The premise was wrong

The brief and four texts in the tree say: the 42 straight plunges of idx 17 are the re-plunges
after each tab. **They are not.** All 42 are the re-entries of the K1 obstacle step at the two
"Clamp X max" boxes of setup "3 Front". The tab descents of idx 17 are already ramps under the
Ramp style (16 fold ramps, section 2.3). Section 3 gives the proof.

These texts state the wrong cause. This note does not edit them; the lead decides (Q1):

| Text | Where |
|---|---|
| Row `U-TabReplunge` (`what`, `closes_in`) | `crates/rs_cam_core/src/limits/exemptions.rs:433-443` |
| "the re-plunge after a tab of a Profile is the plunge entry cut" | `crates/rs_cam_core/src/compute/catalog/cuts.rs:17`, `:328`, `:782` |
| "on a Profile with tabs the same cut holds the tab re-plunge at the op plunge rate" | `crates/rs_cam_core/src/feeds/entry_rule.rs:431-433` |
| Row 56 ("one at each end of a tab gap"), Q4, F5 | `planning/feeds_contract_2026-11-03/phases/P29-8_DESIGN.md:56`, `:464`, `:704` |
| Row 29 doc line | `crates/rs_cam_core/tests/the_exemption_list_only_shrinks_p29_3.rs:88` |

F5 is right in kind (under the style None the tab descent is a straight plunge at the cut feed),
but the emitted feed is 2500 mm/min, not 3165: the per-axis cap lowers it (section 2.3).

## 1. Reproduction

A throwaway test loaded `planning/rivmap_block_job/nz_wanaka_21_operator.toml` read-only through
`ProjectSession::load`, generated idx 17 through `ProjectSession::generate_toolpath`, and wrote
every move to a CSV in the scratchpad. All numbers in this section are measured.

Idx 17 is "Outline profile with tabs": Profile, outside, 6 mm flat end mill (tool 0), depth 25 in
passes of 1.25 (20 levels), feed 3165, plunge 1582, ramp feed 3165, 10610 rpm, 8 tabs 6 x 2 mm,
entry style Ramp at 3 degrees, setup "3 Front".

| Intent | Feed (mm/min) | Moves |
|---|---|---|
| `EntryApproach` | 791 | 19 |
| `EntryRamp` | 3165 | 511 |
| `EntryPlunge` | 1582 | 42 |

1750 moves in all. Cycle time by `machine::kinematics::compute_cycle_time` on
`MachineProfile::link_kinematics`: 720.00 s.

### 1.1 Where the 42 are

Every one of the 42 is at X 379.5 (the X max side of the ring). Each has the same frame:

1. `Retract` rapid to Z 35.5 at the end of a cut piece;
2. `Linking` rapid at Z 35.5 to the new Y;
3. `Linking` rapid down to Z 27.5;
4. `EntryPlunge` at 1582 down to the level;
5. `ClearingCut` (39 of 42), or the remainder of a tab ramp (move 1603), or a cut on the tab top
   (moves 1616, 1739).

| Levels | Plunges | Y of the plunge | Z to |
|---|---|---|---|
| 23.75 to 2.5 (18 levels) | 2 per level, 36 | 274.15 and 120.81 | the level |
| 1.25 (tab level) | 3 | 322.19, 274.15, 120.81 | 1.515, 1.25, 2.0 |
| 0.0 (tab level) | 3 | 264.65, 120.81, 111.31 | 0.0, 2.0, 0.0 |

On the levels 23.75 to 1.25 the cut pieces end at Y 322.19 and 168.85 and start again at 274.15 and
120.81: each gap is 48.04 mm of tool-centre path. On the level 0 the gaps are wider: the re-entries
are at Y 264.65 and 111.31 and a piece ends at 178.35, so each gap is 9.5 mm wider on each side
(section 2.5).

## 2. The cause

### 2.1 The obstacle step runs after the entry dressup

`JobPhases::run_stages` (`crates/rs_cam_core/src/session/compute.rs:757-758`) runs, in this order:

1. `apply_dressups` (the dressup pipeline; the Ramp entry step converts every vertical descent it
   sees, `dressup/mod.rs:478-482`);
2. `clip_and_split_entries`, which runs the boundary clip, then the K1 obstacle step
   (`session/compute.rs:1203`, `compute::obstacle::clip_annotated_to_obstacles`,
   `compute/obstacle.rs:1446`), then the entry-descent split (`optimize_entry_descents_annotated`);
3. `cap_feeds_at_axis_rates`.

The obstacle step calls the shared clip walk `geometry::boundary::clip_walk`
(`geometry/boundary.rs:389`). Where a cut leaves an obstacle zone, the walk emits a rapid to the
plane and a vertical descent tagged `EntryPlunge` (`geometry/boundary.rs:531-532`, and `:536` for a
t = 0 re-entry). The entry dressup ran before this step, so no entry rule ever sees that descent.
The Ramp style cannot reach it.

### 2.2 Why the feed is the op plunge rate

The walk sets the descent feed to `descent_feed = usable_plunge_rate.unwrap_or(source_feed)`
(`geometry/boundary.rs:517`). The session passes `Some(self.context.plunge_rate)`
(`session/compute.rs:1207`), the stored op plunge rate 1582. The entry style does not take part.
So the feed is 1582 under every style. Measured under the style None: 61 `EntryPlunge` at 1582
(19 pass starts and the same 42).

The entry-descent split then rapids down to Z 27.5 (frame step 3). That is the fresh stock top 25.5
plus `PLUNGE_CLEARANCE_MM` 2.0 (`toolpath.rs:26`); the arithmetic matches, the code path of the
split was not traced (reasoned).

### 2.3 The tab descents are a second family

`apply_tabs` (`dressup/tabs.rs:70`) runs inside `ProfilePass::emit_level`
(`compute/execute/clearing_2d.rs:729-730`), so before the dressup pipeline. Its descent after a tab
is a vertical move tagged `EntryPlunge` at the feed of the interrupted cut move
(`dressup/tabs.rs:215-219`, `:254-258`).

- Under Ramp, the entry step sees these descents and turns each into a fold ramp. Measured: 16
  `EntryRamp` groups that start from the `ClearingCut` at the tab top Z 2.0 (8 tabs x 2 levels
  below the tab top). No tab descent is a straight plunge.
- Under None, no entry step runs: only `TRANSFORM_ADAPTIVE_2D` sets `stock_aware_plunge`
  (`compute/catalog/capabilities.rs:787`, the one `rg` hit). Measured: 16
  `EntryPlunge` at 2500. The generator writes 3165; `cap_feeds_at_axis_rates` lowers it to the
  machine Z axis rate 2500 (`max_rate_xyz_mm_min = [15000, 15000, 2500]`, operator file lines 84-88).

### 2.4 Proof by removal

The four obstacles of the setup were removed in memory (`Command::RemoveObstacle`, file not saved)
and idx 17 was generated again. Measured: 0 `EntryPlunge`, 512 `EntryRamp`, 1539 moves. The tab
descents stay 16 fold ramps.

### 2.5 The obstacle geometry

Setup "3 Front", operator file lines 3204-3290: "Clamp X max 1" box X 366.5 to 396.5, Y 124.83 to
164.83, Z 0 to 29.5; "Clamp X max 2" Y 278.17 to 318.17. Clearance 1 mm plus the tool radius 3 mm
gives the tool-centre keep-out Y 120.83 to 168.83 and 274.17 to 322.17: the measured gaps of the
levels 23.75 to 1.25.

A second zone binds near the floor. The file states no machine head, so the head is the default
band of diameter 25 mm (`UNMODELLED_HEAD_DIAMETER_MM`, `machine/head.rs:185`, radius 12.5), and its
face is 30 mm (the stick-out of tool 0) above the tip. The grown clamp top is 29.5 + 1 = 30.5. So
with the tip below Z 0.5 the head reaches the grown clamp, and the keep-out grows by 12.5 - 3 =
9.5 mm on each side: Y 111.33 to 178.33 and 264.67 to 331.67 (reasoned arithmetic). The data match
it (measured): the level 0 gaps of section 1.1; a ramp at HEAD move 1742 cut short at Z 0.4946;
and under the style None two tab descents near the clamps stop at Z 0.5 (section 3.3).

## 3. What the descent is physically

A height map measured each descent: fresh stock top 25.5 (the emission stock box), cell 0.25 mm, a
flat disc of radius 3, every earlier move of idx 17 stamped before the descent is read. Measured.

### 3.1 The 42 obstacle re-entries

| Material under the disc | Bite into stock (mm) | Plunges |
|---|---|---|
| stock under the whole disc | 1.75 (first level, from the fresh top 25.5) | 2 |
| stock under the whole disc | 1.25 (one depth per pass) | 36 |
| stock under the whole disc | 0.50 (move 1616, onto the tab top) | 1 |
| stock under the whole disc | 0.495 (move 1747) | 1 |
| stock under the whole disc | 0.219 (move 1603, into a cut-short tab ramp) | 1 |
| air only | 0 (move 1739, onto the tab top) | 1 |

So 41 of 42 are full-width plunges into stock. The op itself removed only the material above the
previous level at that XY: each level re-enters at the same point, and the point is never cut at
the new level before the plunge. The first-level bite is 1.25 mm on the machine after op 12 faces
the top to 25 (reasoned: op 17 reads fresh stock).

Two of them (1603, 1747) exist only because the clamp clip cut a tab fold ramp short. Moves 1616
and 1739 descend onto a tab top: the tab at Y 120.75 to 132.75 lies inside the clamp keep-out
except 0.08 mm (section 7, F3).

Each plunge also feeds through air at 1582 from Z 27.5 down to the material: 626 mm in all, 23.7 s
(segment length over feed, no acceleration).

### 3.2 The vertical-rate limit

Measured through `feeds_result_for_toolpath(..).plunge.vertical_limit().bound(1582, rpm)`:

| rpm | Bound (mm/min) | Binding |
|---|---|---|
| 10610 (stored) | 1347.47 | `AxialChip`: G6 axial chip 0.0635 mm/tooth x rpm x 2 (row `amana-flat-hardwood-pocket-6000-2f-spektra`) |
| 24000 (Suggest) | 1582 | `Candidate` (the Z axis rate 2500 does not bind) |

At the stored rpm the 41 plunges into stock run 17 % over the bound (1582 / 1347.47).

### 3.3 The tab descents under the style None (measured, `none` run)

| Descent | Bite (mm) | Plunges |
|---|---|---|
| tab top 2.0 to level 1.25 | 0.75, full disc | 8 |
| tab top 2.0 to level 0 | 1.25, full disc, 0.75 air above | 6 |
| tab top 2.0 to 0.5 (the head zone of section 2.5 stops it; then a retract) | 0.75, full disc | 2 |

All 16 run at 2500, 1.86 x the bound 1347.47.

### 3.4 What this decides

The descents are not in air. A ramp or a bounded feed is the right fix; "nothing" is wrong. The
operator asked for a ramp on this op, so a ramp is the faithful answer under Ramp. Under None the
operator asked for a plunge, so only the feed can change.

## 4. The fix options

The rules of the P29 work hold: one rule; an entry number comes only from
`feeds::entry_rule::ResolvedEntry`; a descent into stock is a declared entry cut that the limits
judge.

### (a) A clip re-entry follows the configured entry style

One post-clip entry door, after the boundary clip and the obstacle step, before the entry-descent
split. It runs the shared `dressup::apply_entry` with the op's `ResolvedEntry` on the descents that
a clip invented, and on nothing else. It runs on a `EntryContract::Dressup` op with the style Ramp
or Helix only (the existing `effective_entry_style`). `RestEntryRamp` in
`optimize_entry_descents` is the precedent for a post-clip entry door.

Type shape (compiler-held, no move-shape guess):

- `clip_walk` returns the indices of the descents it emitted (`ClipReentries(Vec<usize>)`) in its
  `Transformed`, next to the mapping.
- `apply_entry` takes `EntrySites`: `EveryVerticalDescent` (the pipeline) or
  `Only(&ClipReentries)` (the door). The prototype used a shape test instead ("an `EntryPlunge`
  after a rapid"); the typed list removes that guess.
- The door takes the style and the plunge feed that the pipeline resolved
  (`PipelineRun::effective_entry_style` and `entry_plunge_rate` carried in the `Produced` record or
  in a `ResolvedPipelineEntry` value), and does not resolve them again on the dressed toolpath.
  The prototype resolved them again, and two rules drift: the plunge feed (791 in the pipeline,
  1582 in the door, F4), and `effective_entry_style`, whose `planner_made_entries` test reads the
  dressup's own ramps, so a non-centre-cutting tool under None would get no ramp at a clip
  re-entry in the door (reasoned from `dressup_apply.rs:688-718`).
- The door then runs `ObstacleToolView::check_toolpath` on its output as a debug assertion. A fold
  ramp walks only the cut that follows the re-entry, and that cut stops at the next retract, so it
  stays inside the allowed interval.
- Reorder (obstacle step before the entry dressup) is not chosen: a lead, a dogbone or a link
  from the later steps could enter an obstacle, and K1 says the obstacle step keeps every move out.
  The data also show why "entry then clip" is wrong: the clip cuts a ramp short (moves 1598 and
  1742 at HEAD).

### (b) A sloped tab flank

This option changes only the tab descents, not the 42. Under Ramp the tab descents already ramp.
Under None (F5) a sloped flank would replace the 16 plunges. The repo holds one source on tab
shape: `research/01_workflows.md:39` ("Tabs/bridges to hold pieces in place (rectangular,
triangular, or rounded)"). It prints no angle and no length. No FreeCAD, Kiri:Moto or Fusion text
in `research/` or `planning/` gives a tab ramp number (`rg` over `*.md` and `*.txt`). A flank angle
would be an underived default; the only angle with a source on this op is the entry ramp angle of
`ResolvedEntry`, which the style None does not have. Not chosen.

### (c) Keep the plunge, bound its feed by the one vertical-rate limit

The clip descent feed and the tab descent feed become
`VerticalRateLimit::bound(op plunge rate, op rpm)`. On idx 17 at 10610 rpm: 1582 to 1347 on the
42 (binding `AxialChip`), 2500 to 1347 on the 16 tab descents under None.

Type shape: `clip_walk` takes an `EntryPlungeFeed` (private field) in place of
`plunge_rate_mm_min: Option<f64>`. Only `ResolvedEntry` builds it, from the op plunge rule and its
`VerticalRateLimit`. `apply_tabs` takes the same type for its descent instead of the interrupted
move's feed. The descent stays the declared `Entry(Plunge)` cut (P29-8 8d); its value check reads
the same bound.

### Choice

**(a)**, with (c) as a second commit for every straight descent that stays (the style None, a fold
that has no run, the tab descent under None).

Why (a) first: the operator chose Ramp at 3 degrees on this op, and 41 of the 42 descents are
full-width plunges into stock. (c) alone keeps the plunge the operator did not ask for. (a) alone
leaves the style None 17 % (clip) and 86 % (tabs) over the bound, so (c) follows. The two are
disjoint on idx 17 under Ramp: after (a) no clip descent is left there.

## 5. Predictions for (a)

The prototype: the shape-test door of section 4 (a) in `session/compute.rs` after the obstacle
step, a `reentry_only` flag on `apply_entry`, and a re-export. The patch is
`/tmp/claude-1001/-home-ricky-personal-repos-rs-cam/ce846d39-41d4-4e44-be56-d5dd5295a423/scratchpad/p29_8e/prototype.patch`.
"Measured" means a run of the prototype; "reasoned" means not run.

### 5.1 Idx 17 (measured)

| | HEAD | Prototype |
|---|---|---|
| Moves | 1750 | 1840 |
| `EntryPlunge` at 1582 | 42 | 0 |
| `EntryRamp` at 3165 | 511 | 603 (+92) |
| `EntryApproach` | 19 at 791 | 19 at 791 + 40 at 1582 |
| Obstacle hits (`check_toolpath`, `JoinStart::Unknown`) | 0 | 0 |
| Cycle time (`compute_cycle_time`) | 720.00 s | 749.55 s (+29.55 s, +4.1 %) |

How each of the 42 changes:

- 39 (move 1603 included): a rapid to Z 26.0, a fed air lead (`EntryApproach`, 1582) down to the material plus the
  contact clearance, then 2 `EntryRamp` moves at 3165: out along the following cut and back. The
  first level ramps 21.47 mm out and drops 1.125 mm per leg: 3.0 degrees. A deep level ramps about
  18.7 mm out and back from about 0.7 mm above the previous level.
- 2: `EntryRamp` moves only: the two re-entries of the first level. The material top there is the
  fresh top 25.5, so the ramp starts at the rapid floor Z 26.0 and needs no air lead.
- 1: an air lead straight down to the tab top Z 2.0 (the descent is air only), a cut on the tab
  top, then the existing tab fold ramp (HEAD move 1739).

Why the time rises: each ramp adds about 2 x 19 to 21 mm at the ramp feed, and the air lead still
starts at Z 26.0. The lead starts high because the conservative material top over the entry disc
(`OwnStockReplay::material_top`) reads the uncut stock under the clamp, 25.5 (reasoned from
`rapid_to_entry_top`, `dressup/entry_descent.rs:452-468`; the 26.0 is measured). The air lead runs
at 1582 because the door's plunge rule reads the first linear feed of the dressed toolpath
(`PlungeFeedRule::FirstLinearFeedFraction`), not the 791 that the pipeline read (measured feed;
the cause is reasoned).

### 5.2 What the operator sees in the G-code (measured from the moves, reasoned for the post)

- No straight G1 Z plunge at F1582 at X 379.5 any more.
- At each clamp gap: G0 down to Z 26.0, a fed G1 Z descent at F1582 through air, then two G1 moves
  that descend along the outline at F3165 (3 degrees) and come back to the gap edge.
- About 30 s more on this op (720 s to 750 s by the kinematic integrator).
- The two uncut spans under the X max clamps stay uncut (section 7, F1).
- The cut itself does not move. Both move lists were stamped to the end on the height map of
  section 3 (cell 0.25 mm): the maximum difference of the two final maps is 0.0 mm, 0 cells
  (measured).

### 5.3 Other ops

| Scope | Result |
|---|---|
| Wanaka idx 0, 8, 12 (Dressup contract, style Ramp) | identical move lists (measured) |
| Wanaka idx 12 | holds 2 obstacle re-entry plunges (X 404.0 and 401.43; they go away with the obstacles, measured); the door leaves them unchanged (measured). Why (reasoned): the cut after the re-entry at X 401.43 is 2.6 mm long, and the fold needs a run of at least the tool radius, so it falls back to the plunge (`dressup/entry_descent.rs:750-764`); the run after the X 404.0 re-entry was not measured |
| Wanaka idx 4, 5, 6, 7, 9, 10, 11, 13 (style None) | no change: the door does not run under None (reasoned); their plunge counts at HEAD are in the scratchpad `head_*.csv` |
| Wanaka idx 1, 2, 14, 15, 16 (Planner contracts) | no change (reasoned; not run, they take minutes) |
| G-OPGOLDEN `every_op_keeps_its_toolpath_g_opgolden` | 46 changes, identical to `opgolden_known_46_rows.txt` (measured): no new row moves |
| FM1 `feeds_matrix_instrument_fm1` | 2 tests pass, 0 cells move (measured, 55 s) |
| Tracked project files with tabs | `nz_wanaka_21.toml` (tracked) and the untracked wanaka copies in `planning/rivmap_block_job/` and `planning/feeds_blind_review_2026-11-03/` hold the same clamps and a Ramp profile; their outline profile changes as idx 17 does (reasoned). `nz_south.toml` has tabs and no obstacle: no change unless a boundary clip re-enters a Ramp op (reasoned) |

The scope of the change is not "a file with tabs". It is a Dressup op with the style Ramp or
Helix whose path crosses an obstacle or re-enters a boundary.

May move, not run (a scan of the tracked `.toml` files for a Ramp or Helix op together with an
obstacle or an enabled boundary):

- test fixtures: `tests/fixtures/testpiece_mini_2026-10-20_c3808eb0/testpiece_mini.toml` (8 Ramp or
  Helix ops, 6 obstacles), `tests/fixtures/rivmap100_rough_2026-11-04_7effc804/rivmap100.toml`,
  `tests/fixtures/t3b_r10_scallop_islands_2026-09-08_e4d817e9.toml`, `tests/fixtures/test_job.toml`,
  `rs_cam_viz/tests/fixtures/stale_cards/face_and_rest_rough.toml` (boundaries);
- planning files: `planning/fixtures/rivmap100/*.toml` (the roughing benchmark), the wanaka200
  files, and the wanaka21 copies under `planning/feeds_blind_review_2026-11-03/`.

Many of these ops are Planner ops (3D Rough), which the door does not touch; the list is an upper
bound.

Sentries that pin a clip descent as `EntryPlunge` at the plunge rate, and may go red (not run):
`boundary_reentry_plunge_rate_g_boundaryplunge`, `boundary_clip_escape_f1`,
`skipped_boundary_offset_f8`, `isoclip_link_rapid_g_isocliprapid`, `entry_descent_profile_b2`,
`transform_provenance_fingerprints`, and the K1 obstacle tests (`a_manual_retract_rises_over_the_clamp_k1`,
`capability_link_moves_safety`). A boundary re-entry of a Ramp op becomes a ramp; each test that
reads one must be re-read before the commit.

### 5.4 Predictions for (c) (reasoned, not run)

- Idx 17 under Ramp after (a): no change (no clip descent left).
- Idx 17 under None: 42 clip plunges and 16 tab plunges at 1347 (from 1582 and 2500).
- Ops whose stored plunge rate is under the bound: no change. The other wanaka ops need one bound
  per op; not measured.

## 6. What changes for the operator's cut

Under the Ramp style the outline profile stops plunging a 6 mm end mill 1.25 mm into white oak at
each side of each X max clamp, on every level. It ramps down along the outline at 3 degrees
instead. The cut takes about 30 s longer. The part outline, the tabs and the uncut spans do not
move (the final height maps are equal, section 5.2, measured).

## 7. Findings outside the fix

- **F1. The clamps leave the outline uncut on the X max side.** The profile never cuts the two
  48 mm tool-centre gaps on any level, and on the level 0 it leaves two 67 mm gaps (the head zone,
  section 2.5). The setup frame has XY0 at the stock corner (setup 1 datum note,
  operator file line 587, "XY0 = stock corner"; setup 3 holds the pins datum and "Top clamps as in
  setup 1", line 3203; the pins at X 6.25 and 388.75 are 6.25 mm from each edge of a 395 mm stock),
  so the X max boxes (X 366.5 to 396.5) cover 10 mm of the part edge at X 376.5, and the X min boxes
  (X -38.5 to -8.5) lie 8.5 mm outside the stock. The two sides are not symmetric. After the op the
  part stays joined over the full height on about 2 x 42 mm of the X max edge, and over the bottom
  0.5 mm on about 2 x 61 mm (gap less the tool diameter), in addition to the 8 tabs (reasoned from
  the move list and the boxes). Operator question (Q4).
- **F2. The tab descent under None runs at 2500**, not at the cut feed 3165: the axis cap lowers
  it. It is 1.86 x the vertical bound.
- **F3. A tab sits under a clamp.** Tab placement (`plan_profile_tabs`) does not read the
  obstacles. The tab at Y 120.75 to 132.75 on the X max side lies in the clamp keep-out except
  0.08 mm, so it holds nothing that the clamp gap does not already hold; it only adds the
  re-entry onto the tab top (moves 1616, 1739).
- **F4. The door's plunge rule reads a different first feed.** `FirstLinearFeedFraction` gives
  791 in the pipeline and 1582 in a post-clip door on the same op (P29-8 O1). The P29-8 8c branch
  (a) (`OpPlungeRate`) would make the two equal.

## 8. Questions for the lead

- **Q1.** The stale texts of section 0: rewrite `U-TabReplunge` as the clip re-entry (who: the
  `Entry(Plunge)` cut; cause: `geometry/boundary.rs:517-536` after the dressups), and fix the
  `cuts.rs`, `entry_rule.rs` and test doc lines, in the commit of (a)? My choice: yes, in that
  commit; the row closes with (a) plus (c).
- **Q2.** (a) and then (c) as two commits, or (c) only? My choice: both, (a) first.
- **Q3.** The door covers boundary re-entries too (one rule for every clip). On a surface-riding
  rest op `RestEntryRamp` already ramps them. Keep `RestEntryRamp` as the rest arm of the same
  door, or fold it in? My choice: one door, `RestEntryRamp` is its rest arm (a later commit).
- **Q4.** F1: is the X max clamp position in the operator file right? If the clamps sit outside
  the part, every clip re-entry of idx 17 goes away and the outline is cut through.
- **Q5.** F3: should tab placement skip the obstacle keep-out? Not in scope here.
- **Q6.** The door takes the pipeline's resolved style and plunge feed (section 4 (a)), or the
  P29-8 8c branch (a) (`OpPlungeRate`) lands first so that the plunge rule has one answer on any
  toolpath? My choice: the door takes the resolved values; 8c (a) is still wanted for F4.

## 9. Lead rulings

Lead, 2026-11-13.

- **The cause stands.** The 42 plunges are the re-entries of the obstacle clip, not the tabs. My
  brief had the wrong premise.
- **Q1: yes.** The commit of (a) rewrites each wrong-cause text (`U-TabReplunge` and the lines the
  note lists).
- **Q2: (a), and (c) as a CHECK, not a clamp.** A stated plunge rate ships as stated, with a
  warning (the rule of a stated depth). So a straight plunge that remains is judged against
  `VerticalRateLimit::bound` on the record; its feed is not lowered at emission. One feed rule
  does change: a descent into stock never runs at the cut feed. The tab descent under style None
  reads the plunge feed of the entry rule (after 8c: the op plunge rate).
- **Q3: yes, one step after the clips for every clip re-entry** (obstacle and boundary), with the
  rest-stock arm. Each fixture op that moves is predicted before the code.
- **Q6: 8c lands first.** The new step reads the `ResolvedEntry` that the dressups resolved; it
  does not work a style or a feed out again.
- **Q4 and Q5 are the operator's** (the clamp boxes of setup "3 Front" cover 10 mm of the part
  edge; a tab sits under a clamp). The lead asks. Tab placement against obstacles is logged, not
  in P29.
- **The fix waits for the operator's go**: it moves the G-code of idx 17 (39 plunges become 3
  degree ramps, +29.5 s).
